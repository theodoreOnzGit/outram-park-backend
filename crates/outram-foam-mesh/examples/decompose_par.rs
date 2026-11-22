// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! `decomposePar` for the geometric methods (gh:#1381): reads `<case>/constant/polyMesh`
//! and writes `<case>/processorN/constant/polyMesh/...` as ESI v2506 does.
//!
//! ```text
//! decompose_par <case> <simple|hierarchical> <nx> <ny> <nz> [order=xyz] [delta=0.001] [precision=17]
//! ```
//!
//! CLI glue (no test reaches `main`: argument parsing and printing only; the functions it
//! calls are tested in `tests/decompose_par_esi_v2506.rs`).

use outram_foam_basic_lib::io::{EsiWriteOptions, PolyMesh};
use outram_foam_mesh::decompose::{decompose_par, write_processor_cases, GeomDecomp, GeomMethod};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 6 {
        eprintln!("usage: decompose_par <case> <simple|hierarchical> <nx> <ny> <nz> [order] [delta] [precision]");
        std::process::exit(2);
    }
    let method = match a[2].as_str() {
        "simple" => GeomMethod::Simple,
        "hierarchical" => GeomMethod::Hierarchical,
        other => {
            eprintln!("method {other} is not ported (scotch, metis, kahip need external libraries)");
            std::process::exit(2);
        }
    };
    let n = [a[3].parse().expect("nx"), a[4].parse().expect("ny"), a[5].parse().expect("nz")];
    let mut dec = GeomDecomp::new(method, n);
    if let Some(o) = a.get(6) {
        dec = dec.with_order(o).expect("order must be three of x y z");
    }
    if let Some(d) = a.get(7) {
        dec = dec.with_delta(d.parse().expect("delta"));
    }
    let precision = a.get(8).map_or(17, |p| p.parse().expect("precision"));
    let case = std::path::Path::new(&a[1]);
    let mesh = PolyMesh::read(case.join("constant/polyMesh")).expect("read polyMesh");
    let d = decompose_par(&mesh, &dec).expect("decompose");
    write_processor_cases(case, &mesh, &d, &EsiWriteOptions::default().with_precision(precision)).expect("write");
    for (p, pr) in d.procs.iter().enumerate() {
        println!("processor {p}: {} cells, {} processor patches", pr.cells.len(), pr.n_processor_patches());
    }
}
