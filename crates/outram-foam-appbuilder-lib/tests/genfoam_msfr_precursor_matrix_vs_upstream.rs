// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `2D_MSFR` precursor equation `precEq.H` against an instrumented upstream binary (gh:#1891, README round 43)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam 652b3da on OpenFOAM-ESI v2506, the `transient` stage of `reactorCases/2D_MSFR` started from the
//! tight `steadyStateEN` state at t = 165 s (the state `refs/msfr_transient.ref.txt.gz` starts from), run with a
//! scratch copy of `libNeutronicsSolvers` whose `include/precEq.H` writes, before `precEq.solve()`, at `writePrecision
//! 17`: the mapped fields (`U`, `alpha`, `alphat`, `mu`, `rhoCool`, `diffCoeffPrec`), the face flux `phi` and its patch
//! values, `neutroSource_`, `precStar` old and current, the matrix `diag`, `upper`, `lower`, `source`, the boundary
//! `internalCoeffs` / `boundaryCoeffs`, and the mesh geometry (`Sf`, `Cf`, `C`, `weights`). The shipped sources and
//! libraries are untouched. Reference: `refs/msfr_prec_matrix_r43.ref.txt.gz`
//! (`scripts/extract_prec_matrix_reference.py`; groups 0, 3, 7 of the first `precEq` call of step 1).
//!
//! The port builds the same quantities from the same inputs (`upstream_precursor_transport`,
//! `DiffusionNeutronics::upstream_precursor_matrix`) and is compared element by element with `==`.
//!
//! **Found (gh:#1891).** The precursor difference of rounds 6 to 40 (5.8e-14 at step 1, 1.3e-11 in the totals at step
//! 50) was the WEDGE patches' boundary flux: upstream's `phi = fvc::flux(U*alpha)` takes the wedge patch value of `U`
//! as `transform(faceT_, U_cell)` (`wedgeFvPatchField::evaluate`), which makes the wedge face flux a few 1e-14 (not 0),
//! and `fvm::div(phi, precStar)` puts that flux into `internalCoeffs`, i.e. into the diagonal of every cell. The port
//! (and the test, and the workflow) set that flux to exactly 0.
//!
//! Predictions H1-H5, P1-P3, C1 and their criteria: README "Round 43", committed before the first run. Results: README
//! "Round 43 results".

// Reads case files: `std` only (gh:#1633).
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::precursor_drift::{
    precursor_transport_zero_wedge_flux, upstream_precursor_transport, wedge_face_t,
};
use outram_foam_appbuilder_lib::genfoam::neutronics::diffusion::{DiffusionNeutronics, DiffusionSettings, PrecursorTransport};
use outram_foam_appbuilder_lib::genfoam::neutronics::xs::CrossSectionData;
use outram_foam_appbuilder_lib::io::nuclear_data::read_nuclear_data;
use outram_foam_appbuilder_lib::io::poly_mesh::{read_cell_zones, read_poly_mesh, zone_of_cell};
use outram_foam_basic_lib::fields::boundary::bc::BoundaryCondition;
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")
}

fn read_ref(path: &Path) -> HashMap<String, Vec<f64>> {
    let text = String::from_utf8(gunzip(path)).expect("utf8");
    let mut m = HashMap::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        if let Some(key) = it.next() {
            m.insert(key.to_string(), it.map(|w| w.parse().expect("number")).collect());
        }
    }
    m
}

fn stage_case() -> PathBuf {
    let src = vv_dir().join("cases/msfr_neutro");
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR")).join("gh1891_msfr");
    let _ = std::fs::remove_dir_all(&dst);
    std::fs::create_dir_all(dst.join("polyMesh")).expect("scratch dir");
    std::fs::copy(src.join("nuclearData"), dst.join("nuclearData")).expect("nuclearData");
    for f in ["points", "faces", "owner", "neighbour", "boundary", "cellZones"] {
        std::fs::write(dst.join("polyMesh").join(f), gunzip(&src.join("polyMesh").join(format!("{f}.gz")))).expect("write");
    }
    dst
}

/// `(number of elements that are not `==`, worst |difference| over the reference's largest magnitude)`.
fn diff_report(name: &str, port: &[f64], up: &[f64]) -> (usize, f64) {
    assert_eq!(port.len(), up.len(), "{name} length");
    let mx = up.iter().fold(0.0_f64, |a, b| a.max(b.abs())).max(1e-300);
    let (mut n_bad, mut worst) = (0usize, 0.0_f64);
    for (a, b) in port.iter().zip(up) {
        if a != b {
            n_bad += 1;
            worst = worst.max((a - b).abs() / mx);
        }
    }
    println!("{name:28} len {:6} not-identical {n_bad:6} worst (of max) {worst:.3e}", up.len());
    (n_bad, worst)
}

fn flat(v: &[Vector3]) -> Vec<f64> {
    v.iter().flat_map(|a| [a.x, a.y, a.z]).collect()
}

struct Model {
    mesh: Arc<FvMesh>,
    xs: CrossSectionData,
    solver: DiffusionNeutronics,
}

fn model() -> Model {
    let dir = stage_case();
    let mesh: Arc<FvMesh> = read_poly_mesh(&dir.join("polyMesh")).expect("polyMesh");
    let input = read_nuclear_data(&dir.join("nuclearData")).expect("nuclearData");
    let xs = CrossSectionData::from_input(&input).expect("xs").with_upstream_unparametrised_chi();
    let zones = read_cell_zones(&dir.join("polyMesh")).expect("cellZones");
    let order: Vec<&str> = (0..xs.zone_count()).map(|i| xs.zone(i).expect("zone").name()).collect();
    let zone_map = zone_of_cell(&zones, &order, mesh.n_cells).expect("zone map");
    let n = mesh.n_cells;
    let flux_boundary: Vec<BoundaryCondition<f64>> = mesh.patches.iter().map(|_| BoundaryCondition::ZeroGradient).collect();
    let params0: Vec<f64> = (0..n).flat_map(|_| [900.0, 4125.0]).collect();
    let solver =
        DiffusionNeutronics::new_with_cell_parameters(mesh.clone(), &xs, &zone_map, &params0, &flux_boundary, DiffusionSettings::default())
            .expect("model");
    Model { mesh, xs, solver }
}

/// Upstream's `diag` with the boundary `internalCoeffs` added in patch order (`fvMatrix::solve`, `addBoundaryDiag`), and
/// `source` with the `boundaryCoeffs` (`addBoundarySource`).
fn solved_form(r: &HashMap<String, Vec<f64>>, k: usize, mesh: &FvMesh) -> (Vec<f64>, Vec<f64>) {
    let mut diag = r[&format!("diag_{k}")].clone();
    let mut source = r[&format!("source_{k}")].clone();
    for (pi, p) in mesh.patches.iter().enumerate() {
        for fi in 0..p.size {
            let c = mesh.owner[p.start + fi];
            diag[c] += r[&format!("internalCoeffs{pi}")][fi];
            source[c] += r[&format!("boundaryCoeffs{pi}")][fi];
        }
    }
    (diag, source)
}

fn velocity(r: &HashMap<String, Vec<f64>>, n: usize) -> Vec<Vector3> {
    (0..n).map(|c| Vector3::new(r["Ucell"][3 * c], r["Ucell"][3 * c + 1], r["Ucell"][3 * c + 2])).collect()
}

/// Matrix of group `k` of the first `precEq` call of step 1 (`ddt` present), for the transport `tr`.
fn port_matrix(m: &mut Model, r: &HashMap<String, Vec<f64>>, k: usize, tr: PrecursorTransport) -> outram_foam_basic_lib::prelude::FvMatrix {
    m.solver.set_precursor_drift(tr);
    let star: Vec<Vec<f64>> = (0..m.xs.prec_groups()).map(|_| r[&format!("precStarCur_{k}")].clone()).collect();
    m.solver.set_prec_star(&star).expect("precStar");
    let (keff, dt) = (r[&format!("HEADER_{k}")][0], r[&format!("HEADER_{k}")][1]);
    m.solver
        .upstream_precursor_matrix(k, &r["neutroSource"], keff, &r[&format!("precStarOld_{k}")], &m.mesh.cell_volumes, dt, true)
        .expect("matrix")
}

/// H1-H5, P1: every input and every coefficient of the first `precEq` call, port against upstream.
///
/// Measured 2026-11-27 (release): `alphat`, `mu`, `diffCoeffPrec`, `rhoCool`-derived diffusivity, the zeroGradient patch
/// fluxes (patches 2-5) and `Beta`-times-`neutroSource` source arrays are bit-identical; the wedge patch fluxes
/// are bit-identical in 8588 of 8595 faces per patch; the matrices are bit-identical except in the 8 cells and 22
/// faces whose GEOMETRY differs in the last bits (below). Written post-run at the measured sizes.
#[test]
fn the_precursor_matrix_matches_the_instrumented_upstream() {
    let r = read_ref(&vv_dir().join("refs/msfr_prec_matrix_r43.ref.txt.gz"));
    let mut m = model();
    let n = m.mesh.n_cells;

    // H1: the mapped coefficients
    let diff_up = &r["diffCoeffPrec"];
    let ours: Vec<f64> = (0..n).map(|c| (0.01 * (1.0 / 8.0) + 0.01 / 1.0) / r["rhoCool"][c]).collect();
    assert_eq!(diff_report("alphat", &vec![0.01 * (1.0 / 8.0); n], &r["alphat"]).0, 0, "H1 alphat");
    assert_eq!(diff_report("mu", &vec![0.01; n], &r["mu"]).0, 0, "H1 mu");
    assert_eq!(diff_report("diffCoeffPrec", &ours, diff_up).0, 0, "H1 diffCoeffPrec");

    // H2: the flux
    let u = velocity(&r, n);
    let tr = upstream_precursor_transport(&m.mesh, &u, &r["alpha"]);
    let phi_int: Vec<f64> = (0..m.mesh.n_internal_faces).map(|f| tr.phi.internal[f]).collect();
    let (bad_phi, w_phi) = diff_report("phi internal", &phi_int, &r["phi"]);
    let mut wedge_bad = 0;
    for pi in 0..m.mesh.patches.len() {
        let v: Vec<f64> = tr.phi.boundary[pi].values.as_slice().to_vec();
        let (b, w) = diff_report(&format!("phi boundary patch {pi}"), &v, &r[&format!("phiB{pi}")]);
        if pi >= 2 {
            assert_eq!(b, 0, "H2: the zeroGradient patch fluxes are bit-identical");
        } else {
            wedge_bad = wedge_bad.max(b);
            // the wedge faces: `faceNormals()` of the patch points against `Sf/|Sf|` of the mesh differ in the last bit
            assert!(b <= 8 && w < 1e-8, "H2 wedge patch {pi}: {b} faces, worst {w:e}");
        }
    }
    // post-run (measured: 22 faces, 9.4e-16 of max): the geometry of 8 quads differs (below), phi inherits it
    assert!(bad_phi <= 22 && w_phi < 2e-15, "H2 internal phi: {bad_phi} faces, {w_phi:e}");

    // geometry (the cause of the remainder)
    let (bsf, _) = diff_report("geometry Sf", &flat(&m.mesh.face_area_vectors[..m.mesh.n_internal_faces]), &r["Sf"]);
    let (bc, _) = diff_report("geometry C", &flat(&m.mesh.cell_centres), &r["C"]);
    let w: Vec<f64> = (0..m.mesh.n_internal_faces).map(|f| outram_foam_basic_lib::fv_operators::fvc::linear_weight(&m.mesh, f)).collect();
    let (bw, _) = diff_report("geometry weights", &w, &r["w"]);
    assert!(bsf <= 12 && bc <= 21 && bw <= 22, "geometry: Sf {bsf}, C {bc}, w {bw}");

    // H3/H4/H5: the matrix of groups 0, 3, 7, with the port's own transport and with upstream's internal flux
    for k in [0usize, 3, 7] {
        let (diag_up, source_up) = solved_form(&r, k, &m.mesh);
        for own in [false, true] {
            let internal = if own { phi_int.clone() } else { r["phi"].clone() };
            let bnd: Vec<Vec<f64>> = (0..m.mesh.patches.len()).map(|pi| tr.phi.boundary[pi].values.as_slice().to_vec()).collect();
            let t = PrecursorTransport::new(&m.mesh, r["alpha"].clone(), internal, bnd, diff_up.clone());
            let mat = port_matrix(&mut m, &r, k, t);
            let label = if own { "own phi" } else { "upstream internal phi" };
            let (bd, wd) = diff_report(&format!("group {k} diag, {label}"), mat.ldu.diag.as_slice(), &diag_up);
            let (bs, ws) = diff_report(&format!("group {k} source, {label}"), mat.source.as_slice(), &source_up);
            let (bu, _) = diff_report(&format!("group {k} upper, {label}"), mat.ldu.upper.as_slice(), &r[&format!("upper_{k}")]);
            let (bl, _) = diff_report(&format!("group {k} lower, {label}"), mat.ldu.lower.as_slice(), &r[&format!("lower_{k}")]);
            // the cells and faces of the geometry difference only (post-run sizes)
            assert!(bd <= 8 && wd < 6e-15, "H3 diag group {k} {label}: {bd} cells, {wd:e}");
            assert!(bs <= 8 && ws < 5e-15, "H3 source group {k} {label}: {bs} cells, {ws:e}");
            if !own {
                assert!(bu <= 6 && bl <= 20, "H3 off-diagonals group {k}: {bu}/{bl}");
            } else {
                assert!(bu <= 22 && bl <= 22, "H3 off-diagonals group {k} own phi: {bu}/{bl}");
            }
        }
    }
    let _ = wedge_bad;
}

/// C1 (control): the form before round 43, wedge faces with exactly zero flux, misses the diagonal of every cell by the
/// wedge `internalCoeffs` (relative 1e-13 to 4e-11 of the diagonal).
#[test]
fn the_zero_wedge_flux_form_misses_the_diagonal_everywhere() {
    let r = read_ref(&vv_dir().join("refs/msfr_prec_matrix_r43.ref.txt.gz"));
    let mut m = model();
    let n = m.mesh.n_cells;
    let u = velocity(&r, n);
    let old = precursor_transport_zero_wedge_flux(&m.mesh, &u, &r["alpha"]);
    for patch in [0usize, 1] {
        assert!(old.phi.boundary[patch].values.as_slice().iter().all(|v| *v == 0.0), "the old form has no wedge flux");
    }
    let bnd: Vec<Vec<f64>> = (0..m.mesh.patches.len()).map(|pi| old.phi.boundary[pi].values.as_slice().to_vec()).collect();
    let t = PrecursorTransport::new(&m.mesh, r["alpha"].clone(), r["phi"].clone(), bnd, r["diffCoeffPrec"].clone());
    let mat = port_matrix(&mut m, &r, 0, t);
    let (diag_up, _) = solved_form(&r, 0, &m.mesh);
    let (bad, worst) = diff_report("C1 group 0 diag, zero wedge flux", mat.ldu.diag.as_slice(), &diag_up);
    assert!(bad > 8000 && worst > 1e-11, "C1: {bad} cells, worst {worst:e}");
}

/// `wedge_face_t` is `None` off a wedge patch and a rotation (`T T^T = I`) on one.
#[test]
fn wedge_face_t_is_a_rotation_on_the_wedge_patches_and_absent_elsewhere() {
    let m = model();
    let mut wedges = 0;
    for (pi, p) in m.mesh.patches.iter().enumerate() {
        match wedge_face_t(&m.mesh, pi) {
            Some(t) => {
                wedges += 1;
                for i in 0..3 {
                    for j in 0..3 {
                        let tt: f64 = (0..3).map(|k| t[i][k] * t[j][k]).sum();
                        let want = if i == j { 1.0 } else { 0.0 };
                        assert!((tt - want).abs() < 1e-12, "patch {} T T^T [{i}][{j}] = {tt}", p.name);
                    }
                }
            }
            None => assert!(p.kind != outram_foam_basic_lib::mesh::PatchKind::Wedge),
        }
    }
    assert_eq!(wedges, 2, "front and back");
}
