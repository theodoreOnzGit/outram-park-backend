// SPDX-License-Identifier: GPL-3.0-only
//! Write the `blockMeshDict` of a [`PipeOGrid`] to a file, so
//! `draw_block_mesh` can draw the O-grid the generator now emits (gh:#1464:
//! the central square is n x n single-cell-wide blocks so that block faces
//! are shared topologically).
//!
//! ```text
//! cargo run --release -p outram-foam-mesh --example write_pipe_dict -- OUT n nr nz
//! ```
//! Example binary: file glue, exempt from the reached-by-a-test rule (the
//! generator itself is tested in `block_primitives`).
use outram_foam_mesh::block_primitives::PipeOGrid;
use uom::si::f64::Length;
use uom::si::length::meter;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, out, n, nr, nz] = a.as_slice() else {
        eprintln!("usage: write_pipe_dict OUT n nr nz");
        std::process::exit(2);
    };
    let g = PipeOGrid::new(
        Length::new::<meter>(0.01),
        Length::new::<meter>(0.2),
        n.parse().unwrap(),
        nr.parse().unwrap(),
        nz.parse().unwrap(),
    );
    std::fs::write(out, g.block_mesh_dict()).unwrap();
}
