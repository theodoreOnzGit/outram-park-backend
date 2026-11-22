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

//! # `dump mesh/stl` against upstream LIGGGHTS (gh:#1416)
//!
//! ## Methodology
//!
//! Deck `reference-data/liggghts/in.mesh_dump_stl` (LIGGGHTS-PUBLIC
//! `3d5c00f2`): the rotating drum of `in.rotating_drum` (64 facets, rotating
//! about `y`, period 0.5 s, 20 000 steps of 10 us, `neighbor 0.003`) with
//! three `dump mesh/stl` commands: ASCII at steps 0, 10 000 and 20 000,
//! `binary` at 20 000, and `region` (a block, `z >= 0.0123`) at 0 and 20 000.
//! The port runs the same system and writes [`mesh_dump`] output from the
//! moving drum at the same steps. Pass criterion: the ASCII files are
//! identical text (so every `%g`-printed normal and node agrees to six
//! digits, including the `1e-17` components of the incrementally rotated
//! cached normals), the binary file identical bytes (every `f32` the same),
//! the region file identical text.
//!
//! `dump mesh/vtk`, `mesh/vtm` and `mesh/gran/vtk` are compiled only with
//! VTK; the compiled upstream has none of them (`lmp_serial -h` lists
//! `mesh/stl` only), so they could not be compared and are not ported.
//!
//! ## Prediction (written before the first run, 2026-11-11)
//!
//! Identical in all five files: the rotating drum is already bit-identical
//! (`rotating_drum_cross_code.rs`), so cached normals and nodes are the same
//! doubles, and the formatting is exact decimal rounding on both sides.
//!
//! ## Result
//!
//! See the assertion messages; recorded in the V&V note on gh:#1416.

use outram_park_fork_liggghts::granular::{GranularContactModel, GranularMaterial};
use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::mesh_dump::{mesh_facets, stl_ascii, stl_binary};
use outram_park_fork_liggghts::mesh_wall::{MeshWall, MovingBoundary, WallGeometry};
use outram_park_fork_liggghts::particle::{Particle, Vec3};
use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

fn path(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/liggghts")
        .join(name)
}

fn drum_dump_matches() {
    let Ok(stl) = std::fs::read_to_string(path("drum_cylinder.stl")) else {
        eprintln!("skipping: reference-data/liggghts/ not present");
        return;
    };
    let r = 0.004;
    let sphere = |x: f64, y: f64, z: f64| {
        Particle::new(
            Vec3::new(x, y, z),
            Vec3::zero(),
            Vec3::zero(),
            Mass::new::<kilogram>(4.0 * std::f64::consts::PI / 3.0 * r * r * r * 2500.0),
            Length::new::<meter>(r),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .expect("valid sphere")
    };
    let drum = MovingBoundary::rotating(
        WallGeometry::Mesh(MeshWall::from_ascii_stl(&stl).expect("valid drum stl")),
        Vec3::zero(),
        Vec3::new(0.0, 1.0, 0.0),
        0.5,
    );
    let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
    let mut sys = GranularSystem::new(
        vec![sphere(0.0, 0.0, -0.0440), sphere(0.009, 0.004, -0.0435)],
        vec![],
        GranularContactModel::hertz_history(material),
        Vec3::new(0.0, 0.0, -9.81),
        1.0e-5,
    )
    .expect("valid system")
    .with_moving_walls(vec![drum])
    .with_neighbor_skin(0.003);

    let mut last = 0;
    for step in [0_usize, 10_000, 20_000] {
        sys.run(step - last);
        last = step;
        let WallGeometry::Mesh(m) = &sys.moving_walls()[0].geometry else {
            panic!("mesh wall");
        };
        let all = mesh_facets(m, |_| true);
        let want = std::fs::read_to_string(path(&format!("mesh_dump_a_{step}.stl")))
            .expect("reference ascii dump");
        let got = stl_ascii(&all);
        assert_eq!(got.lines().count(), want.lines().count(), "step {step}");
        for (n, (g, w)) in got.lines().zip(want.lines()).enumerate() {
            assert_eq!(g, w, "ascii dump differs at step {step}, line {}", n + 1);
        }
        assert_eq!(got, want, "ascii dump at step {step}");

        if step == 0 || step == 20_000 {
            let part = mesh_facets(m, |c| c[2] >= 0.0123 && c[2] <= 0.2);
            let want = std::fs::read_to_string(path(&format!("mesh_dump_c_{step}.stl")))
                .expect("reference region dump");
            assert!(part.len() < all.len() && !part.is_empty());
            assert_eq!(stl_ascii(&part), want, "region dump at step {step}");
        }
        if step == 20_000 {
            let want = std::fs::read(path("mesh_dump_b_20000.stl")).expect("reference binary");
            let got = stl_binary(&all);
            assert_eq!(got.len(), want.len());
            assert_eq!(got, want, "binary dump");
        }
    }
}

/// `dump mesh/stl` (ASCII, binary, region) of a rotating drum: 3 ASCII, 2
/// region and 1 binary file identical to LIGGGHTS.
#[test]
fn mesh_stl_dump_matches_liggghts() {
    drum_dump_matches();
}
