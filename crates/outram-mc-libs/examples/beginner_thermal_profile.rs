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

//! Beginner thermal profile, drawn (gh:#949).
//!
//! A 1 cm UO2 sphere generating 500 W inside a 0.3 cm 304L steel shell,
//! water-cooled preset. Writes five PNGs to the directory given as the first
//! argument (default `target/beginner_thermal_profile`):
//!
//! - `csg_xy.png`: the CSG geometry as transport sees it (`render_material_slice`);
//! - `voxels_xy.png`, `voxels_xz.png`: the voxels the conduction solve used,
//!   by region (red fuel, blue steel, grey outside);
//! - `temperature_xy.png`, `temperature_xz.png`: the solved temperatures, blue
//!   at the coolant temperature to red at the peak.
//!
//! Run: `cargo run --release -p outram-mc-libs --example beginner_thermal_profile -- <dir>`.
//!
//! `main` is CLI glue (no test reaches it, by the workspace's exception); the
//! library calls it makes are tested in `thermal_profile::tests`.
//!
//! An educational ballpark estimate, not a thermal-hydraulic analysis.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::plot::{render_material_slice, PlotBasis, Rgb, SlicePlot};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::thermal_profile::{
    HeatRemoval, SliceColour, SlicePlane, ThermalMaterial, ThermalMesh, ThermalProblem,
    ThermalRegion,
};
use uom::si::f64::Power;
use uom::si::length::centimeter;
use uom::si::power::watt;
use uom::si::thermodynamic_temperature::kelvin;

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

fn main() {
    use HalfSpaceSense::{Inside, Outside};
    let out = std::path::PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "target/beginner_thermal_profile".into()),
    );
    std::fs::create_dir_all(&out).expect("create output directory");

    // Material 0: UO2 fuel; 1: steel shell; 2: water.
    let geometry = Geometry {
        surfaces: vec![
            sphere(1.0, BoundaryType::Transmissive),
            sphere(1.3, BoundaryType::Transmissive),
            sphere(2.0, BoundaryType::Vacuum),
        ],
        cells: vec![
            Cell::material(1, vec![half(0, Inside)], 0, 600.0),
            Cell::material(
                2,
                vec![half(0, Outside), half(1, Inside), RegionToken::Intersection],
                1,
                600.0,
            ),
            Cell::material(
                3,
                vec![half(1, Outside), half(2, Inside), RegionToken::Intersection],
                2,
                600.0,
            ),
        ],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0, 1, 2],
        }],
        lattices: vec![],
        root_universe: 0,
    };

    let palette = [
        (Rgb::new(214, 96, 77), "UO2 FUEL"),
        (Rgb::new(67, 147, 195), "304L STEEL"),
        (Rgb::new(200, 225, 245), "WATER"),
    ];
    let plot = SlicePlot {
        origin: Position::new(0.0, 0.0, 0.0),
        basis: PlotBasis::Xy,
        width: [4.0, 4.0],
        pixels: [400, 400],
        level: None,
        show_overlaps: false,
        meshlines: None,
    };
    let (_raw, img) = render_material_slice(
        &geometry,
        &plot,
        &palette,
        "UO2 sphere in steel, x-y at z = 0",
    );
    img.write_png(out.join("csg_xy.png")).expect("write png");

    let profile = ThermalProblem::uniform_heat(Power::new::<watt>(500.0))
        .removed_by(HeatRemoval::WaterCooled)
        .solve(
            &geometry,
            &[
                ThermalRegion::heated(0, ThermalMaterial::UraniumDioxide),
                ThermalRegion::unheated(1, ThermalMaterial::StainlessSteel304L),
            ],
            &ThermalMesh {
                lower: Position::new(-1.5, -1.5, -1.5),
                upper: Position::new(1.5, 1.5, 1.5),
                voxels_across: 32,
            },
        )
        .expect("thermal profile");

    for (name, plane, colour) in [
        (
            "voxels_xy.png",
            SlicePlane::Xy { z: 0.0 },
            SliceColour::Region,
        ),
        (
            "voxels_xz.png",
            SlicePlane::Xz { y: 0.0 },
            SliceColour::Region,
        ),
        (
            "temperature_xy.png",
            SlicePlane::Xy { z: 0.0 },
            SliceColour::Temperature,
        ),
        (
            "temperature_xz.png",
            SlicePlane::Xz { y: 0.0 },
            SliceColour::Temperature,
        ),
    ] {
        profile
            .render_slice(plane, colour, 12)
            .write_png(out.join(name))
            .expect("write png");
    }

    let k = |t: uom::si::f64::ThermodynamicTemperature| t.get::<kelvin>();
    println!("{:?}", profile.kind);
    println!(
        "heat removal: {:?}, h = {:?}, reference T = {:.2} K, fully cited: {}",
        profile.heat_removal.preset,
        profile.heat_removal.heat_transfer_coefficient,
        k(profile.heat_removal.reference_temperature),
        profile.heat_removal.fully_cited
    );
    println!(
        "{} voxels of {:.4} cm; peak {:.1} K, fuel mean {:.1} K, body mean {:.1} K, surface mean {:.1} K",
        profile.n_voxels(),
        profile.voxel_size.get::<centimeter>(),
        k(profile.peak_temperature),
        k(profile.mean_heated_temperature),
        k(profile.mean_temperature),
        k(profile.mean_surface_temperature)
    );
    println!(
        "energy balance: generated {:.6} W, removed {:.6} W, relative imbalance {:.2e}",
        profile.energy_balance.generated.get::<watt>(),
        profile.energy_balance.removed.get::<watt>(),
        profile.energy_balance.relative_imbalance()
    );
    println!(
        "faces without a CSG normal: {}; correlations outside their range: {:?}",
        profile.faces_without_csg_normal, profile.conductivity_outside_range
    );
    println!("images in {}", out.display());
}
