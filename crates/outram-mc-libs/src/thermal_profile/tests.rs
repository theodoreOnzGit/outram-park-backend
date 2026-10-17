// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Tests of the beginner thermal profile (gh:#949). Numbers in the doc
//! comments were measured by running these tests.

use outram_foam_basic_lib::steady_conduction::{
    RadialClosedForm, RadialGeometry, RadialLayer, ThermalBoundary,
};
use uom::si::f64::{Length, Power, ThermalConductivity, ThermodynamicTemperature};
use uom::si::length::{centimeter, meter};
use uom::si::power::watt;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::volumetric_power_density::watt_per_cubic_meter;

use super::*;
use crate::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use crate::geometry::surface::{BoundaryType, Sphere, SurfaceKind, ZCylinder, ZPlane};
use crate::geometry::universe::Universe;

fn sphere(r: f64, bc: BoundaryType) -> SurfaceKind {
    SurfaceKind::Sphere(Sphere {
        x0: 0.0,
        y0: 0.0,
        z0: 0.0,
        r,
        bc,
    })
}

fn half(s: usize, sense: HalfSpaceSense) -> RegionToken {
    RegionToken::HalfSpace {
        surface_idx: s,
        sense,
    }
}

/// The intersection of half-spaces, in the RPN the cells evaluate.
fn and(tokens: &[RegionToken]) -> Vec<RegionToken> {
    let mut out = vec![tokens[0]];
    for t in &tokens[1..] {
        out.push(*t);
        out.push(RegionToken::Intersection);
    }
    out
}

/// Fuel sphere (material 0, radius `r_fuel`) in a shell (material 1, out to
/// `r_shell`), in a coolant (material 2) out to 3 cm.
fn nested_spheres(r_fuel: f64, r_shell: f64) -> Geometry {
    use HalfSpaceSense::{Inside, Outside};
    Geometry {
        surfaces: vec![
            sphere(r_fuel, BoundaryType::Transmissive),
            sphere(r_shell, BoundaryType::Transmissive),
            sphere(3.0, BoundaryType::Vacuum),
        ],
        cells: vec![
            Cell::material(1, vec![half(0, Inside)], 0, 293.6),
            Cell::material(2, and(&[half(0, Outside), half(1, Inside)]), 1, 293.6),
            Cell::material(3, and(&[half(1, Outside), half(2, Inside)]), 2, 293.6),
        ],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0, 1, 2],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

fn box_of(half_width: f64, voxels_across: usize) -> ThermalMesh {
    ThermalMesh {
        lower: Position::new(-half_width, -half_width, -half_width),
        upper: Position::new(half_width, half_width, half_width),
        voxels_across,
    }
}

fn k3() -> ThermalMaterial {
    ThermalMaterial::Constant(ThermalConductivity::new::<watt_per_meter_kelvin>(3.0))
}

/// **V&V (verification), CSG sphere against the closed form, end to end.**
///
/// *Methodology.* A 1 cm fuel sphere (k = 3 W/(m K), constant) generating
/// 1257 W (q''' = 3e8 W/m^3 on the true volume), water-cooled preset
/// (h = 5000 W/(m^2 K) to 577.65 K, both from [`presets`]). The geometry is
/// a CSG sphere; the film weights come from the CSG surface normals found by
/// tracking (this module), not from a formula. Box +-1.25 cm, 40 voxels
/// across (0.0625 cm voxels). Reference: `RadialClosedForm` (mean surface
/// `T_a + q R/(3h)`, heated-volume mean `T_s + q R^2/(15 k)`).
///
/// *Prediction (before running).* Errors close to those of the analytic-
/// normal voxel sphere in `outram-foam-basic-lib` at a similar resolution
/// (~0.1-1 K on the surface, a few K on the mean); no face without a CSG
/// normal; energy imbalance below 1e-9.
///
/// *Result (2026-10-17).* PASS. 17 256 voxels; mean surface 777.27 K
/// against exact 777.65 K (-0.39 K); heated-volume mean 1439.1 K against
/// 1444.3 K (-5.2 K, 0.6 % of the 867 K rise above the surface); hottest
/// voxel centre 2431.4 K against the exact centre 2444.3 K (a voxel centre
/// sits off the true centre); every surface face found its CSG normal;
/// energy imbalance 7.6e-14. Gates: surface within 2 K, mean within 10 K.
#[test]
fn csg_sphere_matches_closed_form() {
    let geometry = nested_spheres(1.0, 2.0);
    let r_cm = 1.0;
    let q = 3.0e8;
    let volume = 4.0 / 3.0 * std::f64::consts::PI * (r_cm * 1e-2_f64).powi(3);
    let power = Power::new::<watt>(q * volume);
    let profile = ThermalProblem::uniform_heat(power)
        .removed_by(HeatRemoval::WaterCooled)
        .solve(
            &geometry,
            &[ThermalRegion::heated(0, k3())],
            &box_of(1.25, 40),
        )
        .unwrap();
    let removal = HeatRemoval::WaterCooled;
    let exact = RadialClosedForm::new(
        RadialGeometry::Sphere,
        &[RadialLayer {
            outer_radius: Length::new::<centimeter>(r_cm),
            conductivity: ThermalConductivity::new::<watt_per_meter_kelvin>(3.0),
            heat_source: uom::si::f64::VolumetricPowerDensity::new::<watt_per_cubic_meter>(q),
        }],
        ThermalBoundary::Convective {
            coefficient: removal.heat_transfer_coefficient().unwrap(),
            ambient: removal.reference_temperature(),
        },
    )
    .unwrap();
    let r_m = r_cm * 1e-2;
    let exact_surface = exact.surface_temperature().get::<kelvin>();
    let exact_mean = exact_surface + q * r_m * r_m / (15.0 * 3.0);
    let surf = profile.mean_surface_temperature.get::<kelvin>();
    let mean = profile.mean_heated_temperature.get::<kelvin>();
    println!(
        "CSG sphere: surface {surf:.3} K (exact {exact_surface:.3}), mean {mean:.3} K (exact {exact_mean:.3}), \
         peak {:.3} K (exact centre {:.3}), voxels {}, imbalance {:.2e}, faces without normal {}",
        profile.peak_temperature.get::<kelvin>(),
        exact.centre_temperature().get::<kelvin>(),
        profile.n_voxels(),
        profile.energy_balance.relative_imbalance(),
        profile.faces_without_csg_normal
    );
    assert_eq!(profile.kind, ProfileKind::EducationalBallparkEstimate);
    assert_eq!(profile.faces_without_csg_normal, 0);
    assert!(profile.energy_balance.relative_imbalance() < 1e-9);
    assert!(
        (surf - exact_surface).abs() < 2.0,
        "surface {surf} vs {exact_surface}"
    );
    assert!(
        (mean - exact_mean).abs() < 10.0,
        "mean {mean} vs {exact_mean}"
    );
    assert!(profile
        .temperature_at(Position::new(0.0, 0.0, 0.0))
        .is_some());
    assert!(profile
        .temperature_at(Position::new(1.2, 0.0, 0.0))
        .is_none());
    assert!(!profile.heat_removal.fully_cited);
}

/// A UO2 sphere in a steel shell, air-cooled: the nested, temperature-
/// dependent path runs, balances, finds every CSG normal, draws, and reports
/// the steel correlation leaving its 300-700 K range (it must: air cooling at
/// 10 W/(m^2 K) puts the surface far above 700 K at this power).
#[test]
fn nested_uo2_in_steel_air_cooled_runs_and_reports_out_of_range() {
    let geometry = nested_spheres(1.0, 1.3);
    let profile = ThermalProblem::uniform_heat(Power::new::<watt>(50.0))
        .removed_by(HeatRemoval::AirCooled)
        .solve(
            &geometry,
            &[
                ThermalRegion::heated(0, ThermalMaterial::UraniumDioxide),
                ThermalRegion::unheated(1, ThermalMaterial::StainlessSteel304L),
            ],
            &box_of(1.5, 24),
        )
        .unwrap();
    println!(
        "UO2 in steel, air: peak {:.1} K, heated mean {:.1} K, surface {:.1} K, imbalance {:.2e}",
        profile.peak_temperature.get::<kelvin>(),
        profile.mean_heated_temperature.get::<kelvin>(),
        profile.mean_surface_temperature.get::<kelvin>(),
        profile.energy_balance.relative_imbalance()
    );
    assert_eq!(profile.faces_without_csg_normal, 0);
    assert!(profile.energy_balance.relative_imbalance() < 1e-9);
    assert!(profile.peak_temperature > profile.mean_surface_temperature);
    assert!(profile.mean_temperature < profile.mean_heated_temperature);
    assert!(profile
        .conductivity_outside_range
        .contains(&ThermalMaterial::StainlessSteel304L));
    // Drawing: the slice has the grid's shape and the body is not grey.
    for colour in [SliceColour::Region, SliceColour::Temperature] {
        let img = profile.render_slice(SlicePlane::Xy { z: 0.0 }, colour, 2);
        assert_eq!((img.width, img.height), (48, 48));
        assert_ne!(img.get(24, 24), img.get(0, 0));
        let img = profile.render_slice(SlicePlane::Xz { y: 0.0 }, colour, 1);
        assert_eq!((img.width, img.height), (24, 24));
    }
}

/// A finite cylinder (pin) with flat ends, fixed surface temperature: plane
/// and cylinder surfaces both bound the body, the balance holds.
#[test]
fn finite_cylinder_fixed_surface_balances() {
    use HalfSpaceSense::{Inside, Outside};
    let geometry = Geometry {
        surfaces: vec![
            SurfaceKind::ZCylinder(ZCylinder {
                x0: 0.0,
                y0: 0.0,
                r: 0.5,
                bc: BoundaryType::Transmissive,
            }),
            SurfaceKind::ZPlane(ZPlane {
                z0: -1.0,
                bc: BoundaryType::Transmissive,
            }),
            SurfaceKind::ZPlane(ZPlane {
                z0: 1.0,
                bc: BoundaryType::Transmissive,
            }),
            sphere(5.0, BoundaryType::Vacuum),
        ],
        cells: vec![
            Cell::material(
                1,
                and(&[half(0, Inside), half(1, Outside), half(2, Inside)]),
                0,
                293.6,
            ),
            Cell::fill(
                2,
                and(&[half(0, Outside), half(3, Inside)]),
                CellFill::Void,
                Position::ZERO,
            ),
            Cell::fill(
                3,
                and(&[half(0, Inside), half(1, Inside), half(3, Inside)]),
                CellFill::Void,
                Position::ZERO,
            ),
            Cell::fill(
                4,
                and(&[half(0, Inside), half(2, Outside), half(3, Inside)]),
                CellFill::Void,
                Position::ZERO,
            ),
        ],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0, 1, 2, 3],
        }],
        lattices: vec![],
        root_universe: 0,
    };
    let t_s = ThermodynamicTemperature::new::<kelvin>(600.0);
    let profile = ThermalProblem::uniform_heat(Power::new::<watt>(100.0))
        .removed_by(HeatRemoval::FixedSurfaceTemperature(t_s))
        .solve(
            &geometry,
            &[ThermalRegion::heated(0, ThermalMaterial::UraniumDioxide)],
            &box_of(1.2, 24),
        )
        .unwrap();
    assert!(profile.energy_balance.relative_imbalance() < 1e-9);
    assert_eq!(profile.faces_without_csg_normal, 0);
    assert!((profile.mean_surface_temperature.get::<kelvin>() - 600.0).abs() < 1e-9);
    assert!(profile.peak_temperature > t_s);
    assert!(profile.heat_removal.fully_cited);
    assert!((profile.power.get::<watt>() - 100.0).abs() < 1e-12);
    assert!(profile.voxel_size > Length::new::<meter>(0.0));
}

#[test]
fn bad_problems_are_refused() {
    let geometry = nested_spheres(1.0, 2.0);
    let fuel = [ThermalRegion::heated(0, k3())];
    let p = Power::new::<watt>(10.0);
    // No heat removal stated.
    assert!(matches!(
        ThermalProblem::uniform_heat(p).solve(&geometry, &fuel, &box_of(1.5, 8)),
        Err(ThermalProfileError::InvalidInput(_))
    ));
    let wc = ThermalProblem::uniform_heat(p).removed_by(HeatRemoval::WaterCooled);
    // Nothing heated; a material twice; a bad box; negative power.
    assert!(wc
        .solve(
            &geometry,
            &[ThermalRegion::unheated(0, k3())],
            &box_of(1.5, 8)
        )
        .is_err());
    assert!(wc
        .solve(
            &geometry,
            &[fuel[0], ThermalRegion::unheated(0, k3())],
            &box_of(1.5, 8)
        )
        .is_err());
    assert!(wc.solve(&geometry, &fuel, &box_of(1.5, 0)).is_err());
    assert!(ThermalProblem::uniform_heat(Power::new::<watt>(-1.0))
        .removed_by(HeatRemoval::AirCooled)
        .solve(&geometry, &fuel, &box_of(1.5, 8))
        .is_err());
    // The box cuts the body.
    assert_eq!(
        wc.solve(&geometry, &fuel, &box_of(0.8, 8)).unwrap_err(),
        ThermalProfileError::BodyTouchesBox
    );
}

/// Correlation spot checks against their own sources.
///
/// - Graphite matrix at 100 degC returns `k100` = 47.4 exactly (the
///   correlation's reference point; checks the degC conversion).
/// - 304L at 300 K: 14.27 W/(m K) from Graves; OpenStax Table 1.5 gives 14
///   for stainless steel near room temperature, an independent source 2 %
///   away.
/// - UO2 at 300 K: 8.43 W/(m K) from Eq. (3.2) at Bu = 0 (PNNL-31427 quotes
///   7.6 at 25 degC without saying for which density or model; recorded, not
///   gated).
#[test]
fn correlations_reproduce_their_sources() {
    let t = |k: f64| ThermodynamicTemperature::new::<kelvin>(k);
    let k = |m: ThermalMaterial, tk: f64| m.conductivity(t(tk)).get::<watt_per_meter_kelvin>();
    assert!((k(ThermalMaterial::GraphiteMatrixA3, 373.15) - 47.4).abs() < 1e-9);
    let ss = k(ThermalMaterial::StainlessSteel304L, 300.0);
    assert!((ss - 14.0).abs() / 14.0 < 0.03, "304L at 300 K = {ss}");
    let uo2 = k(ThermalMaterial::UraniumDioxide, 300.0);
    println!("UO2 at 300 K = {uo2:.3} W/(m K)");
    assert!((uo2 - 1.0 / (0.0452 + 2.46e-4 * 300.0)).abs() < 1e-6);
    assert!(k(ThermalMaterial::UraniumDioxide, 1500.0) < uo2);
    assert_eq!(k(ThermalMaterial::Aluminium, 500.0), 220.0);
    assert!((k(ThermalMaterial::Zircaloy, 300.0) - 12.682).abs() < 0.01);
    assert_eq!(k(ThermalMaterial::UraniumMetal, 300.0), 27.5);
    assert!(!ThermalMaterial::Zircaloy.is_cited() && ThermalMaterial::Aluminium.is_cited());
    assert!(ThermalMaterial::UraniumDioxide.validity_kelvin().is_none());
}
