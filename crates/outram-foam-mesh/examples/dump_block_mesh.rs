// SPDX-License-Identifier: GPL-3.0-only
//! Prints a numbering-independent digest of a blockMeshDict's mesh:
//! counts, total volume, and a hash of the set of face coordinate loops.
//! Used (gh:#1464) to show that the new upstream numbering leaves the geometry
//! unchanged: run before and after, compare lines.
//!
//! `cargo run --release -p outram-foam-mesh --example dump_block_mesh -- dict...`
use outram_foam_mesh::block_mesh::block_mesh_from_file;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn q(x: f64) -> i64 {
    (x * 1.0e9).round() as i64
}

fn main() {
    for path in std::env::args().skip(1) {
        let m = match block_mesh_from_file(&path) {
            Ok(m) => m,
            Err(e) => {
                println!("{path}: ERROR {e}");
                continue;
            }
        };
        let mut loops: Vec<Vec<(i64, i64, i64)>> = m
            .faces
            .iter()
            .map(|f| {
                let mut v: Vec<_> = f
                    .verts
                    .iter()
                    .map(|&i| (q(m.points[i].x), q(m.points[i].y), q(m.points[i].z)))
                    .collect();
                v.sort_unstable();
                v
            })
            .collect();
        loops.sort();
        let mut h = DefaultHasher::new();
        loops.hash(&mut h);
        let names: Vec<_> = m.patches.iter().map(|p| format!("{}:{}", p.name, p.size)).collect();
        println!(
            "{path}: points={} faces={} internal={} cells={} vol={:.12e} faceset={:016x} patches=[{}]",
            m.points.len(),
            m.faces.len(),
            m.n_internal_faces,
            m.n_cells,
            m.total_volume(),
            h.finish(),
            names.join(",")
        );
    }
}
