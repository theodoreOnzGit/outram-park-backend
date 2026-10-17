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

//! Verification of the steady conduction solve against the closed form
//! (gh:#949). Every number below was measured by running these tests; the
//! predictions were written before the first run.

use uom::si::area::square_meter;
use uom::si::f64::{
    Area, HeatTransfer, Length, ThermalConductivity, ThermodynamicTemperature,
    VolumetricPowerDensity,
};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::volumetric_power_density::watt_per_cubic_meter;

use super::*;

fn m(x: f64) -> Length {
    Length::new::<meter>(x)
}
fn kw(x: f64) -> ThermalConductivity {
    ThermalConductivity::new::<watt_per_meter_kelvin>(x)
}
fn q(x: f64) -> VolumetricPowerDensity {
    VolumetricPowerDensity::new::<watt_per_cubic_meter>(x)
}
fn tk(x: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(x)
}
fn h(x: f64) -> HeatTransfer {
    HeatTransfer::new::<watt_per_square_meter_kelvin>(x)
}

fn geometries() -> [RadialGeometry; 3] {
    [
        RadialGeometry::Slab {
            area: Area::new::<square_meter>(0.01),
        },
        RadialGeometry::Cylinder { height: m(0.5) },
        RadialGeometry::Sphere,
    ]
}

/// The two-layer body used for the nested-shell checks: a heated core of
/// radius 1 cm (k = 3, q''' = 3e8 W/m^3) inside an unheated 0.5 cm shell
/// (k = 20). Numbers are illustrative inputs for a numerics check, not
/// material data.
fn two_layers() -> [RadialLayer; 2] {
    [
        RadialLayer {
            outer_radius: m(0.01),
            conductivity: kw(3.0),
            heat_source: q(3.0e8),
        },
        RadialLayer {
            outer_radius: m(0.015),
            conductivity: kw(20.0),
            heat_source: q(0.0),
        },
    ]
}

fn convective() -> ThermalBoundary {
    ThermalBoundary::Convective {
        coefficient: h(5.0e3),
        ambient: tk(560.0),
    }
}

/// Solve `layers` on a radial mesh with `refine` cells per layer-centimetre
/// and return the max cell-centre error against the closed form (K), the
/// relative energy imbalance and the solution.
fn radial_error(
    geometry: RadialGeometry,
    layers: &[RadialLayer],
    surface: ThermalBoundary,
    cells_per_layer: usize,
) -> (f64, f64, SteadyConductionSolution) {
    let shells: Vec<RadialShell> = layers
        .iter()
        .map(|l| RadialShell {
            outer_radius: l.outer_radius,
            cells: cells_per_layer,
        })
        .collect();
    let mesh = radial_mesh(geometry, &shells).expect("mesh");
    let problem = SteadyConductionProblem::new(
        mesh.clone(),
        layers.iter().map(|l| l.heat_source).collect(),
        vec![
            PatchCondition::uniform(surface),
            PatchCondition::uniform(ThermalBoundary::Adiabatic),
        ],
    )
    .expect("problem");
    let k: Vec<ThermalConductivity> = layers.iter().map(|l| l.conductivity).collect();
    let sol = problem
        .solve(|z, _| k[z], &SteadyConductionSettings::default())
        .expect("solve");
    let exact = RadialClosedForm::new(geometry, layers, surface).expect("closed form");
    let err = (0..sol.n_cells())
        .map(|c| {
            let r = m(mesh.mesh.cell_centres[c].x);
            (sol.temperature(c).get::<kelvin>() - exact.temperature_at(r).unwrap().get::<kelvin>())
                .abs()
        })
        .fold(0.0_f64, f64::max);
    (err, sol.energy_balance.relative_imbalance(), sol)
}

/// **V&V (verification), exact offset on one layer.**
///
/// *Methodology.* One heated layer (R = 1 cm, k = 3 W/(m K),
/// q''' = 3e8 W/m^3) with a fixed 600 K surface, on `N = 5, 10, 20, 40`
/// equal cells, for the slab, the cylinder and the sphere. Reference:
/// [`RadialClosedForm`] at each cell centre.
///
/// *Prediction (written before the first run).* With equal cells and one
/// material, the discrete flux through every interior face equals the exact
/// enclosed heat when the exact temperatures are inserted (the mid-radii
/// straddle each face symmetrically), so interior temperature *differences*
/// are exact. The only error comes from the half-cell to the surface, where
/// the scheme takes the gradient as linear: it over-predicts every cell by the
/// same `q''' dr^2 / (8 (m+1) k)`, i.e. 1/8, 1/16 and 1/24 of `q dr^2/k` for
/// slab, cylinder and sphere. Pass: every cell's error equals that offset to
/// within 1e-9 K, and so the observed order is exactly 2.
///
/// *Result (2026-10-17).* PASS, prediction confirmed: every cell of all
/// twelve cases carries the predicted offset to within 1e-9 K (at N = 10:
/// 12.5 K slab, 6.25 K cylinder, 4.17 K sphere), and the energy imbalance is
/// below 1e-9 of the generated power.
#[test]
fn vv_single_layer_error_is_the_predicted_uniform_offset() {
    let layer = RadialLayer {
        outer_radius: m(0.01),
        conductivity: kw(3.0),
        heat_source: q(3.0e8),
    };
    let surface = ThermalBoundary::FixedTemperature(tk(600.0));
    for geometry in geometries() {
        for n in [5, 10, 20, 40] {
            let (_, imbalance, sol) = radial_error(geometry, &[layer], surface, n);
            let exact = RadialClosedForm::new(geometry, &[layer], surface).unwrap();
            let dr = 0.01 / n as f64;
            let mf = f64::from(geometry.exponent());
            let offset = 3.0e8 * dr * dr / (8.0 * (mf + 1.0) * 3.0);
            let mesh = radial_mesh(
                geometry,
                &[RadialShell {
                    outer_radius: m(0.01),
                    cells: n,
                }],
            )
            .unwrap();
            for c in 0..n {
                let r = m(mesh.mesh.cell_centres[c].x);
                let e = sol.temperature(c).get::<kelvin>()
                    - exact.temperature_at(r).unwrap().get::<kelvin>();
                assert!(
                    (e - offset).abs() < 1e-9,
                    "{geometry:?} N={n} cell {c}: error {e} K, predicted offset {offset} K"
                );
            }
            assert!(
                imbalance < 1e-9,
                "{geometry:?} N={n}: imbalance {imbalance}"
            );
        }
    }
}

/// **V&V (verification), convergence order on nested shells with a
/// convective surface.**
///
/// *Methodology.* [`two_layers`] (heated core in an unheated shell of
/// different conductivity) cooled by `h = 5000 W/(m^2 K)` to 560 K, on
/// `N = 4, 8, 16, 32, 64` equal cells per layer, slab / cylinder / sphere.
/// Error: max over cell centres of `|T_FV - T_exact|`. Observed order
/// `p = log2(e_N / e_2N)`.
///
/// *Prediction (written before the first run).* Second order: `p -> 2`
/// (between 1.9 and 2.1 on the finest pair). The material interface is a
/// face (series-resistance conductance, exact for a linear profile) and the
/// unheated shell's `1/r^m` flux is integrated with the face area, both
/// `O(dr^2)`.
///
/// *Result (2026-10-17).* PASS, prediction confirmed. Max error (K) at
/// N = 4, 8, 16, 32, 64 cells per layer:
///
/// | geometry | N=4 | N=8 | N=16 | N=32 | N=64 | p (every pair) |
/// |---|---|---|---|---|---|---|
/// | slab | 78.1 | 19.5 | 4.88 | 1.22 | 0.305 | 2.000 |
/// | cylinder | 39.6 | 9.90 | 2.48 | 0.619 | 0.155 | 2.000 |
/// | sphere | 27.0 | 6.74 | 1.68 | 0.421 | 0.105 | 2.000 |
///
/// (Peak rise in the sphere is about 1000 K, so N = 16 is already under
/// 0.2 %.) Energy imbalance `< 1e-9` everywhere. Gate: `1.9 <= p <= 2.1`
/// on the finest pair.
#[test]
fn vv_nested_shells_converge_at_second_order() {
    for geometry in geometries() {
        let mut errors = Vec::new();
        for n in [4, 8, 16, 32, 64] {
            let (err, imbalance, _) = radial_error(geometry, &two_layers(), convective(), n);
            assert!(
                imbalance < 1e-9,
                "{geometry:?} N={n}: imbalance {imbalance}"
            );
            errors.push((n, err));
        }
        for w in errors.windows(2) {
            let p = (w[0].1 / w[1].1).log2();
            println!(
                "{geometry:?}: N={} err={:.4e} K -> N={} err={:.4e} K, p={p:.3}",
                w[0].0, w[0].1, w[1].0, w[1].1
            );
        }
        let last = &errors[errors.len() - 2..];
        let p = (last[0].1 / last[1].1).log2();
        assert!((1.9..=2.1).contains(&p), "{geometry:?}: observed order {p}");
    }
}

/// **V&V (verification), temperature-dependent conductivity (Picard).**
///
/// *Methodology.* A sphere (R = 1 cm, q''' = 3e8 W/m^3) with `k(T) = a + b T`,
/// `a = 1`, `b = 0.004` W/(m K^2), surface fixed at 600 K. The Kirchhoff
/// transform gives the exact profile: `a (T - T_s) + b/2 (T^2 - T_s^2) =
/// q''' (R^2 - r^2) / 6`. Meshes N = 10, 20, 40, 80.
///
/// *Prediction (written before the first run).* Picard converges in a
/// handful of passes (k varies about 2x over the body), and the error is
/// second order (`p` between 1.9 and 2.1 on the finest pair).
///
/// *Result (2026-10-17).* Order confirmed only asymptotically; the first run
/// used N = 10..80 and its finest pair gave p = 1.89, **missing the 1.9
/// gate**. The protocol was extended to N = 320 (the gate was not moved):
///
/// | N | 10 | 20 | 40 | 80 | 160 | 320 |
/// |---|---|---|---|---|---|---|
/// | max error (K) | 5.15 | 1.67 | 0.483 | 0.130 | 0.0339 | 0.00865 |
/// | p vs previous | | 1.62 | 1.79 | 1.89 | 1.94 | 1.97 |
///
/// Picard took 12-13 passes to 1e-6 K; peak 1545.1 K (surface 600 K).
/// The coarse-mesh order is below 2 because `k` is evaluated at cell-centre
/// temperatures, a pre-asymptotic effect that fades as shown.
#[test]
fn vv_picard_on_linear_conductivity_matches_kirchhoff_solution() {
    let (a, b, ts, qv, r_out) = (1.0, 0.004, 600.0, 3.0e8, 0.01);
    let exact = |r: f64| {
        // b/2 T^2 + a T - (b/2 Ts^2 + a Ts + q(R^2 - r^2)/6) = 0
        let rhs = 0.5 * b * ts * ts + a * ts + qv * (r_out * r_out - r * r) / 6.0;
        (-a + (a * a + 2.0 * b * rhs).sqrt()) / b
    };
    let mut errors = Vec::new();
    for n in [10, 20, 40, 80, 160, 320] {
        let mesh = radial_mesh(
            RadialGeometry::Sphere,
            &[RadialShell {
                outer_radius: m(r_out),
                cells: n,
            }],
        )
        .unwrap();
        let problem = SteadyConductionProblem::new(
            mesh.clone(),
            vec![q(qv)],
            vec![
                PatchCondition::uniform(ThermalBoundary::FixedTemperature(tk(ts))),
                PatchCondition::uniform(ThermalBoundary::Adiabatic),
            ],
        )
        .unwrap();
        let sol = problem
            .solve(
                |_, t| kw(a + b * t.get::<kelvin>()),
                &SteadyConductionSettings::default(),
            )
            .unwrap();
        let err = (0..n)
            .map(|c| {
                (sol.temperature(c).get::<kelvin>() - exact(mesh.mesh.cell_centres[c].x)).abs()
            })
            .fold(0.0_f64, f64::max);
        println!(
            "Picard k(T): N={n} err={err:.4e} K passes={} peak={:.2} K",
            sol.picard_passes,
            sol.peak_temperature().1.get::<kelvin>()
        );
        assert!(sol.energy_balance.relative_imbalance() < 1e-9);
        assert!(
            sol.picard_passes < 60,
            "Picard took {} passes",
            sol.picard_passes
        );
        errors.push(err);
    }
    let p = (errors[errors.len() - 2] / errors[errors.len() - 1]).log2();
    assert!((1.9..=2.1).contains(&p), "observed order {p}");
}

/// A voxel sphere of radius `r` cooled convectively, with or without the
/// film-area correction; returns (mean-T error, surface-T error, imbalance).
fn voxel_sphere(cells_across: usize, film_correction: bool) -> (f64, f64, f64) {
    let r = 0.01;
    let hh = 2.0 * r / cells_across as f64;
    let grid = VoxelGrid {
        origin: [m(-r - hh); 3],
        spacing: m(hh),
        counts: [cells_across + 2; 3],
    };
    let vm = voxel_mesh(&grid, |p| {
        let d2: f64 = p.iter().map(|x| x.get::<meter>().powi(2)).sum();
        (d2 <= r * r).then_some(0)
    })
    .unwrap();
    let mesh = vm.conduction.mesh.clone();
    let surface = convective();
    let condition = if film_correction {
        let fraction = (0..vm.n_surface_faces())
            .map(|i| {
                let f = mesh.patches[0].start + i;
                let c = mesh.face_centres[f];
                let n_true = [c.x, c.y, c.z];
                let norm = n_true.iter().map(|v| v * v).sum::<f64>().sqrt();
                let n_axis = vm.surface_normal[i];
                (0..3)
                    .map(|a| n_axis[a] * n_true[a] / norm)
                    .sum::<f64>()
                    .abs()
            })
            .collect();
        PatchCondition::with_film_area_fraction(surface, fraction)
    } else {
        PatchCondition::uniform(surface)
    };
    // Same total power as the exact sphere, so the voxel volume error does
    // not masquerade as a conduction error.
    let qv = 3.0e8;
    let exact_volume = 4.0 / 3.0 * std::f64::consts::PI * r * r * r;
    let voxel_volume: f64 = mesh.cell_volumes.iter().sum();
    let problem = SteadyConductionProblem::new(
        vm.conduction.clone(),
        vec![q(qv * exact_volume / voxel_volume)],
        vec![condition],
    )
    .unwrap();
    let sol = problem
        .solve(|_, _| kw(3.0), &SteadyConductionSettings::default())
        .unwrap();
    let exact = RadialClosedForm::new(
        RadialGeometry::Sphere,
        &[RadialLayer {
            outer_radius: m(r),
            conductivity: kw(3.0),
            heat_source: q(qv),
        }],
        surface,
    )
    .unwrap();
    // Exact volume mean of T_s + q(R^2 - r^2)/(6k) is T_s + q R^2/(15 k).
    let exact_mean = exact.surface_temperature().get::<kelvin>() + qv * r * r / (15.0 * 3.0);
    let mean = sol.volume_mean_temperature(&[]).unwrap().get::<kelvin>();
    let surf = sol.surface_mean_temperature(0).unwrap().get::<kelvin>();
    (
        (mean - exact_mean).abs(),
        (surf - exact.surface_temperature().get::<kelvin>()).abs(),
        sol.energy_balance.relative_imbalance(),
    )
}

/// **V&V (verification), any-shape voxel mesh on a sphere, with the
/// film-area correction ablated.**
///
/// *Methodology.* A sphere (R = 1 cm, k = 3, q''' = 3e8 W/m^3 rescaled so the
/// voxel body carries the exact sphere's power) cut from cubic voxels, 8, 16,
/// 32 and 64 across, cooled by `h = 5000 W/(m^2 K)` to 560 K. Compared with
/// the closed form: the volume-mean temperature (exact `T_s + q R^2/(15 k)`)
/// and the mean surface temperature (exact `T_a + q R/(3 h)` = 760 K).
///
/// *Prediction (written before the first run).* With the film-area fraction
/// `|n_axis . n_true|`, both errors fall at about first order (the staircase
/// moves the surface by up to one voxel). **Without** it the film's area is
/// the staircase's, which tends to 3/2 of the sphere's, so the film drop
/// (200 K exact) settles near 2/3 of itself and the surface error stalls near
/// 67 K instead of falling.
///
/// *Result (2026-10-17).* PASS. Errors (K) against the closed form:
///
/// | voxels across | 8 | 16 | 32 | 64 |
/// |---|---|---|---|---|
/// | corrected: volume-mean T | 3.48 | 11.3 | 5.13 | 1.60 |
/// | corrected: mean surface T | 5.28 | 1.76 | 0.725 | 0.104 |
/// | uncorrected: volume-mean T | 73.9 | 79.9 | 69.1 | 65.0 |
/// | uncorrected: mean surface T | 71.1 | 71.1 | 67.9 | 67.1 |
///
/// The uncorrected surface error stalls near 67 K as predicted (200 K x 1/3).
/// The corrected volume-mean error is **not monotone** (staircase noise:
/// which voxels fall inside changes irregularly with N), so no clean order
/// is claimed for it; the surface error falls by 50x over the range. Energy
/// imbalance <= 1.6e-12. Gate on the finest mesh: corrected surface < 6 K
/// and mean < 10 K, uncorrected surface > 40 K.
#[test]
fn vv_voxel_sphere_converges_only_with_the_film_area_correction() {
    let mut with = Vec::new();
    for n in [8, 16, 32, 64] {
        let (mean_err, surf_err, imbalance) = voxel_sphere(n, true);
        let (mean_raw, surf_raw, _) = voxel_sphere(n, false);
        println!(
            "voxel sphere N={n}: corrected mean err {mean_err:.3} K, surface err {surf_err:.3} K | \
             uncorrected mean err {mean_raw:.3} K, surface err {surf_raw:.3} K | imbalance {imbalance:.2e}"
        );
        assert!(imbalance < 1e-9);
        with.push((mean_err, surf_err, surf_raw));
    }
    let last = with[with.len() - 1];
    // Corrected: the finest mesh is within 3 % of the 200 K film drop and
    // 3 % of the 333 K centre rise; uncorrected: still off by > 40 K.
    assert!(last.1 < 6.0, "corrected surface error {} K", last.1);
    assert!(last.0 < 10.0, "corrected mean error {} K", last.0);
    assert!(last.2 > 40.0, "uncorrected surface error {} K", last.2);
    assert!(
        with[3].1 < with[1].1,
        "corrected surface error does not fall"
    );
}

/// Inputs that have no steady state, or are malformed, are refused.
#[test]
fn bad_inputs_are_errors() {
    let mesh = radial_mesh(
        RadialGeometry::Sphere,
        &[RadialShell {
            outer_radius: m(0.01),
            cells: 4,
        }],
    )
    .unwrap();
    let adiabatic = PatchCondition::uniform(ThermalBoundary::Adiabatic);
    // Heated, nothing removes the heat.
    assert!(SteadyConductionProblem::new(
        mesh.clone(),
        vec![q(1.0)],
        vec![adiabatic.clone(), adiabatic.clone()]
    )
    .is_err());
    // Wrong counts.
    assert!(SteadyConductionProblem::new(
        mesh.clone(),
        vec![],
        vec![adiabatic.clone(), adiabatic.clone()]
    )
    .is_err());
    assert!(
        SteadyConductionProblem::new(mesh.clone(), vec![q(1.0)], vec![adiabatic.clone()]).is_err()
    );
    // Film fraction of the wrong length.
    let bad_film = PatchCondition::with_film_area_fraction(convective(), vec![0.5, 0.5]);
    assert!(SteadyConductionProblem::new(
        mesh.clone(),
        vec![q(1.0)],
        vec![bad_film, adiabatic.clone()]
    )
    .is_err());
    // A non-positive conductivity.
    let ok = SteadyConductionProblem::new(
        mesh.clone(),
        vec![q(1.0)],
        vec![PatchCondition::uniform(convective()), adiabatic],
    )
    .unwrap();
    assert!(matches!(
        ok.solve(|_, _| kw(0.0), &SteadyConductionSettings::default()),
        Err(SteadyConductionError::BadConductivity { .. })
    ));
    // Meshes.
    assert!(radial_mesh(RadialGeometry::Sphere, &[]).is_err());
    assert!(radial_mesh(
        RadialGeometry::Sphere,
        &[RadialShell {
            outer_radius: m(0.01),
            cells: 0
        }]
    )
    .is_err());
    let grid = VoxelGrid {
        origin: [m(0.0); 3],
        spacing: m(0.1),
        counts: [2, 2, 2],
    };
    assert!(voxel_mesh(&grid, |_| None).is_err());
    let vm = voxel_mesh(&grid, |_| Some(0)).unwrap();
    assert_eq!(vm.cell_at([m(0.15), m(0.05), m(0.05)]), Some(1));
    assert_eq!(vm.cell_at([m(0.25), m(0.05), m(0.05)]), None);
    assert_eq!(vm.n_surface_faces(), 24);
    // Closed form refuses an adiabatic surface and r outside the body.
    let layer = two_layers();
    assert!(
        RadialClosedForm::new(RadialGeometry::Sphere, &layer, ThermalBoundary::Adiabatic).is_err()
    );
    let cf = RadialClosedForm::new(RadialGeometry::Sphere, &layer, convective()).unwrap();
    assert!(cf.temperature_at(m(0.02)).is_err());
    assert!(cf.centre_temperature() > cf.surface_temperature());
}
