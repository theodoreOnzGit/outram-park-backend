// SPDX-License-Identifier: GPL-3.0-only
//! The default surface-preserving dual against the upstream-mode dual of the
//! same primal `polyMesh` (gh:#1472): counts, enclosed volume against the
//! primal's, and the mesh-quality figures `assess_quality` reports.
//!
//! `compare_dual_modes PRIMAL_POLYMESH_DIR FEATURE_ANGLE`
//!
//! Example binary: reporting glue, exempt from the reached-by-a-test rule (it
//! calls only the tested dual constructions and `assess_quality`).
use outram_foam_mesh::mesh_quality::assess_quality;
use outram_foam_mesh::poly_dual_mesh::{poly_dual_mesh, DualMesh, PolyMesh as Primal};
use outram_foam_mesh::poly_dual_mesh_upstream::{poly_dual_mesh_upstream, UpstreamOptions};

fn report(name: &str, d: &DualMesh, primal_volume: f64, names: &[String]) {
    let io = d.to_foam_poly_mesh(names).expect("to polyMesh");
    let q = assess_quality(&io);
    println!(
        "{name:<10} cells {:>6} points {:>6} faces {:>6} (internal {:>6}) | volume {:.12} (primal {:.12}, rel {:+.2e}) | max non-ortho {:.2} deg, mean {:.2} deg, max skew {:.3}, max aspect {:.1}, neg-vol cells {}",
        d.n_cells,
        d.points.len(),
        d.faces.len(),
        io.n_internal_faces,
        d.total_volume(),
        primal_volume,
        (d.total_volume() - primal_volume) / primal_volume,
        q.max_non_ortho_deg,
        q.mean_non_ortho_deg,
        q.max_skewness,
        q.max_aspect_ratio,
        q.n_negative_volume_cells,
    );
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, dir, angle] = a.as_slice() else {
        eprintln!("usage: compare_dual_modes PRIMAL_POLYMESH_DIR FEATURE_ANGLE");
        std::process::exit(2);
    };
    let angle: f64 = angle.parse().unwrap();
    let io = outram_foam_basic_lib::io::PolyMesh::read(dir).expect("read primal");
    let primal = Primal {
        points: io.points.clone(),
        faces: io.faces.iter().map(|f| f.verts.clone()).collect(),
        owner: io.faces.iter().map(|f| f.owner).collect(),
        neighbour: io.faces.iter().take(io.n_internal_faces).map(|f| f.neighbour.unwrap()).collect(),
        n_internal_faces: io.n_internal_faces,
        patches: io.patches.clone(),
        n_cells: io.n_cells,
    };
    let names: Vec<String> = io.patches.iter().map(|p| p.name.clone()).collect();
    let pv = primal.total_volume();
    println!("primal     cells {:>6} points {:>6} faces {:>6} volume {pv:.12}", io.n_cells, io.points.len(), io.faces.len());
    report("default", &poly_dual_mesh(&primal, angle).expect("default"), pv, &names);
    report(
        "upstream",
        &poly_dual_mesh_upstream(&primal, &UpstreamOptions::new(angle)).expect("upstream"),
        pv,
        &names,
    );
}
