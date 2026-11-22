// SPDX-License-Identifier: GPL-3.0-only
//! Write the feature sets of upstream-mode `polyDualMesh` for a primal
//! `polyMesh` as id lists (`featureFaces.ids`, `featureEdges.ids` as point
//! pairs, `singleCellFeaturePoints.ids`, `multiCellFeaturePoints.ids`), in
//! `toc()` order, so they can be diffed with OpenFOAM's own `.obj` dumps
//! (gh:#1472).
//!
//! `dump_dual_features PRIMAL_POLYMESH_DIR OUT_DIR FEATURE_ANGLE [flags]`.
//! Example binary: reporting glue, exempt from the reached-by-a-test rule
//! (it calls the tested `upstream_feature_sets`).
use outram_foam_mesh::poly_dual_mesh::PolyMesh as Primal;
use outram_foam_mesh::poly_dual_mesh_upstream::{primal_edges, upstream_feature_sets, UpstreamOptions};
use std::io::Write;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, dir, out, angle, flags @ ..] = a.as_slice() else {
        eprintln!("usage: dump_dual_features PRIMAL_POLYMESH_DIR OUT_DIR FEATURE_ANGLE [flags]");
        std::process::exit(2);
    };
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
    let mut o = UpstreamOptions::new(angle.parse().unwrap());
    for f in flags {
        o = match f.as_str() {
            "splitAllFaces" => o.with_split_all_faces(),
            "concaveMultiCells" => o.with_concave_multi_cells(),
            other => panic!("unknown flag {other}"),
        };
    }
    let fs = upstream_feature_sets(&primal, &o).expect("feature sets");
    let edges = primal_edges(&primal);
    std::fs::create_dir_all(out).unwrap();
    let w = |name: &str| std::fs::File::create(format!("{out}/{name}")).unwrap();
    let mut f = w("featureFaces.ids");
    for i in &fs.faces {
        writeln!(f, "{i}").unwrap();
    }
    let mut f = w("featureEdges.ids");
    for &e in &fs.edges {
        writeln!(f, "{} {}", edges[e].0, edges[e].1).unwrap();
    }
    let mut f = w("singleCellFeaturePoints.ids");
    for &p in &fs.single_cell_points {
        writeln!(f, "{p}").unwrap();
    }
    let mut f = w("multiCellFeaturePoints.ids");
    for &p in &fs.multi_cell_points {
        writeln!(f, "{p}").unwrap();
    }
}
