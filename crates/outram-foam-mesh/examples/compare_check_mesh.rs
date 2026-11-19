// SPDX-License-Identifier: GPL-3.0-only
//! Compare this crate's ESI-form `checkMesh` fields with the `dumpQuality`
//! probe of OpenFOAM.com v2506 (gh:#1364): field by field, index by index.
//!
//! ```text
//! compare_check_mesh CASE_DIR
//! ```
//! `CASE_DIR` holds `constant/polyMesh` and the probe's `quality/` directory
//! (`verification_and_validation/esi_v2506_differential_gh1364/dumpQuality`).
//! Prints, per field, how many values are bit-identical, the worst absolute
//! and relative difference and the index of the first difference, then the
//! summary numbers `checkMesh` prints. Example binary: reporting glue, exempt
//! from the reached-by-a-test rule (it calls only the tested `check_mesh` API;
//! `tests/check_mesh_esi_v2506.rs` pins the results).
use outram_foam_basic_lib::mesh::PatchKind;
use outram_foam_mesh::check_mesh::{esi_check_summary, esi_geometric_d, esi_quality_fields};
use outram_foam_mesh::snappy_hex_mesh::esi_snap::SnapMesh;

fn read_numbers(path: &std::path::Path) -> Vec<f64> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .split_whitespace()
        .map(|t| t.parse().unwrap())
        .collect()
}

fn report(name: &str, ours: &[f64], theirs: &[f64]) {
    if ours.len() != theirs.len() {
        println!("{name}: LENGTH ours {} theirs {}", ours.len(), theirs.len());
        return;
    }
    let (mut same, mut worst_abs, mut worst_rel, mut first) = (0usize, 0.0f64, 0.0f64, None);
    for (i, (a, b)) in ours.iter().zip(theirs).enumerate() {
        if a.to_bits() == b.to_bits() {
            same += 1;
        } else {
            if first.is_none() {
                first = Some(i);
            }
            let d = (a - b).abs();
            worst_abs = worst_abs.max(d);
            worst_rel = worst_rel.max(d / b.abs().max(1e-300));
        }
    }
    println!(
        "{name}: n {} bit-identical {same} worst abs {worst_abs:e} worst rel {worst_rel:e} first differing {first:?}",
        ours.len()
    );
}

fn flat(v: &[[f64; 3]]) -> Vec<f64> {
    v.iter().flat_map(|p| p.iter().copied()).collect()
}

fn main() {
    let case = std::env::args().nth(1).expect("usage: compare_check_mesh CASE_DIR");
    let case = std::path::Path::new(&case);
    let io = outram_foam_basic_lib::io::PolyMesh::read(case.join("constant/polyMesh").to_str().unwrap())
        .expect("read polyMesh");
    let mesh = SnapMesh {
        points: io.points.iter().map(|p| [p.x, p.y, p.z]).collect(),
        faces: io.faces.iter().map(|f| f.verts.clone()).collect(),
        owner: io.faces.iter().map(|f| f.owner).collect(),
        neighbour: io.faces.iter().take(io.n_internal_faces).map(|f| f.neighbour.unwrap()).collect(),
    };
    let geom0 = outram_foam_mesh::check_mesh::esi_geometry(&mesh);
    let empty: Vec<usize> = io
        .patches
        .iter()
        .filter(|p| p.kind == PatchKind::Empty)
        .flat_map(|p| p.start..p.start + p.size)
        .collect();
    let mesh_d = esi_geometric_d(&geom0.face_areas, &empty);
    let q = esi_quality_fields(&mesh, mesh_d);
    let up = case.join("quality");
    let meshd_up: Vec<f64> = read_numbers(&up.join("meshD"));
    println!("meshD ours {:?} theirs {:?}", mesh_d, meshd_up);
    let g = &q.geometry;
    report("faceCentres", &flat(&g.face_centres), &read_numbers(&up.join("faceCentres")));
    report("faceAreas", &flat(&g.face_areas), &read_numbers(&up.join("faceAreas")));
    report("cellCentres", &flat(&g.cell_centres), &read_numbers(&up.join("cellCentres")));
    report("cellVolumes", &g.cell_volumes, &read_numbers(&up.join("cellVolumes")));
    report("ortho", &q.orthogonality, &read_numbers(&up.join("ortho")));
    report("skew", &q.skewness, &read_numbers(&up.join("skew")));
    report("ownPyr", &q.own_pyramid, &read_numbers(&up.join("ownPyr")));
    report("neiPyr", &q.nei_pyramid, &read_numbers(&up.join("neiPyr")));
    report("openness", &q.openness, &read_numbers(&up.join("openness")));
    report("aspect", &q.aspect_ratio, &read_numbers(&up.join("aspect")));
    report("flatness", &q.flatness, &read_numbers(&up.join("flatness")));
    report("concavity", &q.concavity, &read_numbers(&up.join("concavity")));
    report("determinant", &q.determinant, &read_numbers(&up.join("determinant")));
    // The EXISTING dev-form `check_quality` on the same (ESI) geometry, for the ledger
    // of where it differs from ESI's numbers (gh:#1364).
    {
        use outram_foam_basic_lib::primitives::Vector3;
        use outram_foam_mesh::check_mesh::{check_quality, CheckGeometry, CheckOptions};
        let v = |a: &[f64; 3]| Vector3::new(a[0], a[1], a[2]);
        let geom = CheckGeometry {
            face_areas: g.face_areas.iter().map(v).collect(),
            face_centres: g.face_centres.iter().map(v).collect(),
            cell_centres: g.cell_centres.iter().map(v).collect(),
            cell_volumes: g.cell_volumes.clone(),
        };
        let pts: Vec<Vector3> = mesh.points.iter().map(v).collect();
        let nb: Vec<Option<usize>> =
            (0..mesh.faces.len()).map(|f| mesh.neighbour.get(f).copied()).collect();
        for (label, opts) in [("default", CheckOptions::default()), ("legacy", CheckOptions::foam_mesh_legacy())] {
            let r = check_quality(&pts, &mesh.faces, &mesh.owner, &nb, &geom, &opts);
            println!(
                "dev-form check_quality[{label}]: nonOrthMax {:.17e} nonOrthMean {:.17e} skewMax {:.17e} boundarySkewMax {:.17e} aspect {:.17e} aspectCmpt {:.17e} minFaceArea {:.17e} minVol {:.17e} minPyr {:.17e} minDet {:.17e}",
                r.max_non_orthogonality_deg,
                r.mean_non_orthogonality_deg,
                r.max_skewness,
                r.max_boundary_skewness,
                r.max_aspect_ratio,
                r.max_aspect_ratio_cmpt,
                r.min_face_area,
                r.min_cell_volume,
                r.min_face_pyramid_volume,
                r.min_cell_determinant
            );
        }
    }
    let s = esi_check_summary(&q);
    println!("summary {s:?}");
    println!(
        "text: Mesh non-orthogonality Max: {:.6} average: {:.6}; Max skewness = {:.6}; Max cell openness = {:.6}; Max aspect ratio = {:.6}; Min volume = {:.6}. Max volume = {:.6}.  Total volume = {:.6}; Minimum face area = {:.6}; Cell determinant minimum: {:.6} average: {:.6}",
        s.max_non_orthogonality_deg,
        s.avg_non_orthogonality_deg,
        s.max_skewness,
        s.max_openness,
        s.max_aspect_ratio,
        s.min_volume,
        s.max_volume,
        s.total_volume,
        s.min_face_area,
        s.min_determinant,
        s.avg_determinant
    );
}
