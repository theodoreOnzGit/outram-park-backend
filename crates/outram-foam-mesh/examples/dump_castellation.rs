// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! gh:#1465/#1466: dump the castellated mesh of an STL as text for the ESI
//! v2506 differential (`verification_and_validation/snappy_hex_mesh_esi_v2506`).
//!
//! `dump_castellation STL xmin ymin zmin xmax ymax zmax nx ny nz level kx ky kz OUT`
//!
//! With a 15th argument `octree` only the refinement stage runs
//! ([`outram_foam_mesh::snappy_hex_mesh::refinement::refined_octree`]): `OUT` holds
//! the refined leaves (a 16th argument limits the iterations), and the per-iteration selected-cell counts are printed.
//!
//! Writes `OUT` with one line per cell, `level cx cy cz` (level recovered from
//! the cell volume), and prints patch face counts. Example binary: CLI glue,
//! exempt from the reached-by-a-test rule (it calls only the tested `castellate`).

use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_mesh::snappy_hex_mesh::stl::read_stl_zoned;
use outram_foam_mesh::snappy_hex_mesh::{
    castellate, read_stl, BackgroundMesh, Bounds, CastellationControls, TriangleSoup,
};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let f = |i: usize| a[i].parse::<f64>().expect("number");
    let u = |i: usize| a[i].parse::<usize>().expect("integer");
    // 18th argument `zoned`: upstream's reader (single precision, merged corners).
    let surface = if a.get(17).map(String::as_str) == Some("zoned") {
        TriangleSoup::from_zoned(&read_stl_zoned(&a[0]).expect("stl"))
    } else {
        read_stl(&a[0]).expect("stl")
    };
    let domain = Bounds::new(Vector3::new(f(1), f(2), f(3)), Vector3::new(f(4), f(5), f(6)));
    let bg = BackgroundMesh::uniform(domain, u(7), u(8), u(9));
    let v0 = (f(4) - f(1)) * (f(5) - f(2)) * (f(6) - f(3)) / (u(7) * u(8) * u(9)) as f64;
    let c = CastellationControls::new(bg, u(10), Vector3::new(f(11), f(12), f(13)))
        .with_surface_intersection_refinement();
    if a.get(15).map(String::as_str) == Some("octree") {
        let (cells, selected) = outram_foam_mesh::snappy_hex_mesh::refinement::refined_octree(
            &surface,
            &c,
            a.get(16).map_or(100, |v| v.parse().expect("max iterations")),
        );
        let mut text = String::new();
        for (level, cc) in &cells {
            text.push_str(&format!("{level} {:.12e} {:.12e} {:.12e}\n", cc.x, cc.y, cc.z));
        }
        std::fs::write(&a[14], text).expect("write");
        println!("octree cells {} selected per iteration {:?}", cells.len(), selected);
        return;
    }
    let m = castellate(&surface, &c).expect("castellate");
    let mut text = String::new();
    for (cc, vol) in m.fv_mesh.cell_centres.iter().zip(&m.fv_mesh.cell_volumes) {
        let level = ((v0 / vol).log2() / 3.0).round() as i64;
        text.push_str(&format!("{level} {:.12e} {:.12e} {:.12e}\n", cc.x, cc.y, cc.z));
    }
    std::fs::write(&a[14], text).expect("write");
    println!(
        "cells {} internal faces {} faces {} by level {:?}",
        m.n_cells(),
        m.fv_mesh.n_internal_faces,
        m.fv_mesh.n_faces,
        m.cells_by_level
    );
    for p in &m.fv_mesh.patches {
        println!("patch {} {}", p.name, p.size);
    }
}
