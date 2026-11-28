// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GAMG smoothers, pre-sweeps and the asymmetric path against the real binary's own GAMG (gh:#1936, README "Round 72")
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! **Methodology.** Same instrument as round 67 (`genfoam_gamg_vs_upstream.rs`): an instrumented COPY of ESI v2506's `GAMGSolver`
//! (`upstream_driver/gamg_dump/build.sh`) dumps, inside the real `GeN-Foam` run, the matrix, the agglomeration, the residual after
//! every V-cycle and the solution of one solve. **S1** the FFTF `p_rgh` solve 0 of step 1 (symmetric, the round 67 matrix, tolerance
//! 1e-14, `relTol 0`) with the smoother and sweep selections the port newly runs (`scripts/run_fftf_gamg_real.sh` with
//! `GAMG_SMOOTHER`); **S2** (second half of this file) the `TStruct` solve of the shipped legacy thermo-mechanics case (asymmetric).
//! Criterion: every V-cycle residual and the solution bit-identical. Controls: a different smoother or sweep count must differ.
//! **Results**: README "Round 72 results".

#![cfg(feature = "std")]
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::common::gamg::{face_area_pair_weights, gamg_dic_solve, pair_agglomerate, GamgControls, GamgSmoother};
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_basic_lib::prelude::LduMatrix;

fn refs() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361/refs")
}

fn gunzip_bytes(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let flg = bytes[3];
    let mut at = 10;
    if flg & 4 != 0 {
        at += 2 + usize::from(bytes[at]) + 256 * usize::from(bytes[at + 1]);
    }
    for bit in [8u8, 16u8] {
        if flg & bit != 0 {
            while bytes[at] != 0 {
                at += 1;
            }
            at += 1;
        }
    }
    if flg & 2 != 0 {
        at += 2;
    }
    decompress_to_vec(&bytes[at..bytes.len() - 8]).expect("inflate")
}

fn gunzip(path: &Path) -> String {
    String::from_utf8(gunzip_bytes(path)).expect("utf8")
}

/// The values of the line starting with `key n` (one line per key).
fn list(text: &str, key: &str) -> Vec<f64> {
    for l in text.lines() {
        let mut it = l.split_whitespace();
        if it.next() == Some(key) {
            let n: usize = it.next().expect("n").parse().expect("n");
            let v: Vec<f64> = it.map(|s| s.parse().expect("number")).collect();
            assert_eq!(v.len(), n, "{key}");
            return v;
        }
    }
    panic!("key {key} missing");
}

fn mesh_dir() -> Option<PathBuf> {
    let up = PathBuf::from(std::env::var("GENFOAM_UPSTREAM").unwrap_or_else(|_| "/home/user/upstream-build/GeN-Foam-src".to_string()));
    let p = up.join("Tutorials/reactorCases/2D_FFTF/rootCase/constant/fluidRegion/polyMesh");
    p.join("owner").is_file().then_some(p)
}

struct Fftf {
    a: LduMatrix,
    weights: Vec<f64>,
    source: Vec<f64>,
    psi0: Vec<f64>,
}

fn load_fftf() -> Option<Fftf> {
    let dir = mesh_dir()?;
    let m = gunzip(&refs().join("gamg_r67_fftf_step1_matrix.ref.txt.gz"));
    let lower: Vec<usize> = list(&m, "LOWER").into_iter().map(|v| v as usize).collect();
    let upper: Vec<usize> = list(&m, "UPPER").into_iter().map(|v| v as usize).collect();
    let diag = list(&m, "DIAG");
    let mut a = LduMatrix::new(diag.len(), lower, upper);
    a.diag = diag;
    a.upper = list(&m, "UPPERC");
    a.lower = a.upper.clone();
    let mesh = read_poly_mesh(&dir).expect("FFTF fluid polyMesh");
    let sf: Vec<[f64; 3]> = mesh.face_area_vectors[..a.n_internal_faces].iter().map(|v| [v.x, v.y, v.z]).collect();
    Some(Fftf { a, weights: face_area_pair_weights(&sf), source: list(&m, "SOURCE"), psi0: list(&m, "PSI0") })
}

fn ref_cycles(name: &str) -> Vec<f64> {
    gunzip(&refs().join(format!("gamg_r72_s1_{name}_cycles.ref.txt.gz"))).lines().filter(|l| l.starts_with("VC ")).map(|l| l.split_whitespace().nth(3).expect("r").parse().expect("f")).collect()
}

fn ref_solution(name: &str) -> Vec<f64> {
    list(&gunzip(&refs().join(format!("gamg_r72_s1_{name}_solution.ref.txt.gz"))), "PSI")
}

fn ctl(smoother: GamgSmoother, n_pre: usize, n_post: usize, n_fin: usize, n_coarsest: usize) -> GamgControls {
    GamgControls {
        smoother,
        n_pre_sweeps: n_pre,
        n_post_sweeps: n_post,
        n_finest_sweeps: n_fin,
        n_cells_in_coarsest_level: n_coarsest,
        tolerance: 1e-14,
        rel_tol: 0.0,
        ..GamgControls::default()
    }
}

/// Port solve of the dumped system; returns `(cycles, psi)`.
fn solve(f: &Fftf, c: &GamgControls, max_iter: usize) -> (Vec<f64>, Vec<f64>) {
    let mut fwd = true;
    let agg = pair_agglomerate(f.a.n_cells, &f.a.owner, &f.a.neighbour, &f.weights, c, &mut fwd).expect("agglomerate");
    let mut psi = f.psi0.clone();
    let p = gamg_dic_solve(&f.a, &agg, &GamgControls { max_iter, ..*c }, &f.source, &mut psi).expect("solve");
    (p.cycles, psi)
}

fn first_difference(port: &[f64], up: &[f64]) -> Option<(usize, f64, f64)> {
    port.iter().zip(up).enumerate().find(|(_, (x, y))| x.to_bits() != y.to_bits()).map(|(k, (x, y))| (k + 1, *x, *y))
}

/// The four S1 selections: `(reference name, controls)`.
fn variants() -> Vec<(&'static str, GamgControls)> {
    vec![
        ("gs", ctl(GamgSmoother::GaussSeidel, 0, 2, 2, 10)),
        ("sgs", ctl(GamgSmoother::SymGaussSeidel, 0, 2, 2, 10)),
        ("dicgs", ctl(GamgSmoother::DicGaussSeidel, 1, 1, 1, 20)),
        ("dicpre", ctl(GamgSmoother::Dic, 2, 2, 2, 10)),
    ]
}

/// S1 criterion: every V-cycle residual (all 1 000, the real run also stopped at `maxIter 1000`) and the solution equal the real
/// binary's bit for bit, for each of the four selections.
#[test]
fn s1_every_new_smoother_and_pre_sweep_selection_reproduces_the_real_v_cycles() {
    let Some(f) = load_fftf() else {
        eprintln!("SKIP: upstream tutorials not found; set GENFOAM_UPSTREAM");
        return;
    };
    for (name, c) in variants() {
        let up = ref_cycles(name);
        let t0 = std::time::Instant::now();
        let (cycles, psi) = solve(&f, &c, 1000);
        let first = first_difference(&cycles, &up);
        let sol = ref_solution(name);
        let n_diff = psi.iter().zip(&sol).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
        let worst = cycles.iter().zip(&up).fold(0.0_f64, |m, (x, y)| m.max(((x - y) / y).abs()));
        println!("S1 {name}: {} cycles (upstream {}), first differing cycle {first:?}, worst relative residual difference {worst:e}, psi entries not bit-identical {n_diff}, {:?}", cycles.len(), up.len(), t0.elapsed());
        assert_eq!(cycles.len(), up.len(), "{name}: cycle count");
        assert!(first.is_none(), "{name}: {first:?}");
        assert_eq!(n_diff, 0, "{name}: psi");
    }
}

/// S1 controls: a wrong smoother, a wrong sweep count or a missing pre-smoothing differs from the real run at some cycle by more than
/// 1e-9 relative.
#[test]
fn s1_controls_the_wrong_smoother_or_sweep_count_differs_from_the_real_run() {
    let Some(f) = load_fftf() else {
        eprintln!("SKIP: upstream tutorials not found; set GENFOAM_UPSTREAM");
        return;
    };
    let worst = |name: &str, c: &GamgControls| -> f64 {
        let up = ref_cycles(name);
        let (cycles, _) = solve(&f, c, 30);
        cycles.iter().zip(&up).fold(0.0_f64, |m, (x, y)| m.max(((x - y) / y).abs()))
    };
    let mut ok = Vec::new();
    for (label, against, c) in [
        ("GaussSeidel run as symGaussSeidel", "gs", ctl(GamgSmoother::SymGaussSeidel, 0, 2, 2, 10)),
        ("symGaussSeidel run as GaussSeidel", "sgs", ctl(GamgSmoother::GaussSeidel, 0, 2, 2, 10)),
        ("GaussSeidel run as DIC", "gs", ctl(GamgSmoother::Dic, 0, 2, 2, 10)),
        ("DICGaussSeidel run as DIC", "dicgs", ctl(GamgSmoother::Dic, 1, 1, 1, 20)),
        ("DICGaussSeidel run as GaussSeidel", "dicgs", ctl(GamgSmoother::GaussSeidel, 1, 1, 1, 20)),
        ("nPreSweeps 2 run with nPreSweeps 0", "dicpre", ctl(GamgSmoother::Dic, 0, 2, 2, 10)),
        ("nPreSweeps 2 run with nPreSweeps 1", "dicpre", ctl(GamgSmoother::Dic, 1, 2, 2, 10)),
    ] {
        let w = worst(against, &c);
        println!("S1 control `{label}`: worst relative cycle residual difference over 30 cycles {w:e}");
        ok.push(w > 1e-9);
    }
    assert!(ok.iter().all(|&b| b), "{ok:?}");
}

// ---------------------------------------------------------------------------------------------------------------------------------
// S2: the shipped `TStruct` solve of the legacy thermo-mechanics case (asymmetric, `GaussSeidel`, `nCellsInCoarsestLevel 50`).
// Real run: `legacyTmDump` (GeN-Foam's own `legacyThermoMechanics`) with `TStruct` `solver GAMGdump` and the shipped options; the
// matrix, the agglomeration and the coarse matrices of solve 0 (step 1) are the real binary's own.
// ---------------------------------------------------------------------------------------------------------------------------------

use outram_foam_appbuilder_lib::genfoam::common::gamg::coarse_matrices;
use outram_foam_appbuilder_lib::genfoam::thermo_mechanics::legacy_fvm::pbicg::pbicg_dilu;

/// One dumped level: the restriction, the face restriction and the coarse `diag`, `upper`, `lower` of the real binary.
struct TmLevel {
    restrict: Vec<f64>,
    face_restrict: Vec<f64>,
    cdiag: Vec<f64>,
    cupper: Vec<f64>,
    clower: Vec<f64>,
}

struct TmDump {
    a: LduMatrix,
    source: Vec<f64>,
    psi0: Vec<f64>,
    levels: Vec<TmLevel>,
}

/// One scratch directory per call: the two S2 tests run concurrently in one process, and a directory named by the
/// process id alone let one test read `faces` while the other was rewriting it ("expected 60408 faces, parsed 0",
/// gates55, 2026-11-28).
static TM_MESH_CALL: AtomicUsize = AtomicUsize::new(0);

/// The legacy thermo-mechanics mesh of the repository case `cases/legacy_tm_esfr`, gunzipped into a scratch directory.
fn tm_mesh_dir() -> Option<PathBuf> {
    let src = refs().join("../cases/legacy_tm_esfr/constant/thermoMechanicalRegion/polyMesh");
    let call = TM_MESH_CALL.fetch_add(1, Ordering::SeqCst);
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1936_tm_polymesh_{}_{call}", std::process::id()));
    std::fs::create_dir_all(&dst).ok()?;
    for f in ["points", "faces", "owner", "neighbour", "cellZones", "boundary"] {
        let gz = src.join(format!("{f}.gz"));
        if gz.is_file() {
            std::fs::write(dst.join(f), gunzip_bytes(&gz)).ok()?;
        } else {
            std::fs::copy(src.join(f), dst.join(f)).ok()?;
        }
    }
    Some(dst)
}

fn load_tm() -> TmDump {
    let m = gunzip(&refs().join("gamg_r72_s2_tstruct_matrix.ref.txt.gz"));
    let lower: Vec<usize> = list(&m, "LOWER").into_iter().map(|v| v as usize).collect();
    let upper: Vec<usize> = list(&m, "UPPER").into_iter().map(|v| v as usize).collect();
    let diag = list(&m, "DIAG");
    let mut a = LduMatrix::new(diag.len(), lower, upper);
    a.diag = diag;
    a.upper = list(&m, "UPPERC");
    a.lower = list(&m, "LOWERC");
    let ag = gunzip(&refs().join("gamg_r72_s2_tstruct_agglom.ref.txt.gz"));
    // one text block per `LEVEL` line
    let mut levels = Vec::new();
    let mut block = String::new();
    let mut push = |block: &mut String, levels: &mut Vec<TmLevel>| {
        if !block.is_empty() {
            levels.push(TmLevel {
                restrict: list(block, "RESTRICT"),
                face_restrict: list(block, "FACERESTRICT"),
                cdiag: list(block, "CDIAG"),
                cupper: list(block, "CUPPERC"),
                clower: list(block, "CLOWERC"),
            });
            block.clear();
        }
    };
    for line in ag.lines() {
        if line.starts_with("LEVEL") {
            push(&mut block, &mut levels);
        } else if !line.starts_with("NLEVELS") {
            block.push_str(line);
            block.push('\n');
        }
    }
    push(&mut block, &mut levels);
    TmDump { source: list(&m, "SOURCE"), psi0: list(&m, "PSI0"), a, levels }
}

fn tm_ctl(tol: f64, rel: f64, max_iter: usize) -> GamgControls {
    GamgControls { smoother: GamgSmoother::GaussSeidel, tolerance: tol, rel_tol: rel, n_cells_in_coarsest_level: 50, max_iter, ..GamgControls::default() }
}

fn tm_weights(a: &LduMatrix) -> Option<Vec<f64>> {
    let mesh = read_poly_mesh(&tm_mesh_dir()?).expect("TM polyMesh");
    let sf: Vec<[f64; 3]> = mesh.face_area_vectors[..a.n_internal_faces].iter().map(|v| [v.x, v.y, v.z]).collect();
    Some(face_area_pair_weights(&sf))
}

fn ref_s2_cycles(tag: &str) -> Vec<f64> {
    gunzip(&refs().join(format!("gamg_r72_{tag}_tstruct_cycles.ref.txt.gz"))).lines().filter(|l| l.starts_with("VC 0 ")).map(|l| l.split_whitespace().nth(3).expect("r").parse().expect("f")).collect()
}

fn ref_s2_solution(tag: &str) -> Vec<f64> {
    list(&gunzip(&refs().join(format!("gamg_r72_{tag}_tstruct_solution.ref.txt.gz"))), "PSI")
}

/// S2 criterion: the agglomeration, all coarse matrices (`diag`, `upper`, `lower`), the V-cycles, the iteration count and the solution
/// of the real `TStruct` solve are bit-identical, at the shipped tolerance (1 cycle) and at 1e-14 over 40 cycles.
#[test]
fn s2_the_asymmetric_tstruct_solve_equals_the_real_binarys() {
    let d = load_tm();
    let Some(w) = tm_weights(&d.a) else {
        eprintln!("SKIP: upstream tutorials not found; set GENFOAM_UPSTREAM");
        return;
    };
    let c = tm_ctl(1e-6, 1e-3, 1000);
    let mut fwd = true;
    let agg = pair_agglomerate(d.a.n_cells, &d.a.owner, &d.a.neighbour, &w, &c, &mut fwd).expect("agglomerate");
    assert_eq!(agg.levels.len(), d.levels.len(), "levels");
    let mut agg_diff = 0usize;
    for (l, u) in agg.levels.iter().zip(&d.levels) {
        agg_diff += l.restrict.iter().zip(&u.restrict).filter(|(x, y)| **x as f64 != **y).count();
        agg_diff += l.face_restrict.iter().zip(&u.face_restrict).filter(|(x, y)| **x as f64 != **y).count();
    }
    let mats = coarse_matrices(&d.a, &agg);
    let mut mat_diff = 0usize;
    for (m, u) in mats.iter().zip(&d.levels) {
        mat_diff += m.diag.iter().zip(&u.cdiag).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
        mat_diff += m.upper.iter().zip(&u.cupper).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
        mat_diff += m.lower.iter().zip(&u.clower).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
    }
    println!("S2 levels {} (upstream {}), agglomeration entries differing {agg_diff}, coarse diag/upper/lower entries not bit-identical {mat_diff}", agg.levels.len(), d.levels.len());
    assert_eq!(agg_diff, 0);
    assert_eq!(mat_diff, 0);
    for (tag, ctl) in [("s2", c), ("s2t", tm_ctl(1e-14, 0.0, 40))] {
        let mut psi = d.psi0.clone();
        let p = gamg_dic_solve(&d.a, &agg, &ctl, &d.source, &mut psi).expect("solve");
        let up = ref_s2_cycles(tag);
        let sol = ref_s2_solution(tag);
        let n_diff = psi.iter().zip(&sol).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
        let fd = first_difference(&p.cycles, &up);
        println!("S2 {tag}: {} cycles (upstream {}), first differing cycle {fd:?}, psi entries not bit-identical {n_diff}", p.cycles.len(), up.len());
        assert_eq!(fd, None);
        assert_eq!(p.cycles.len(), up.len());
        assert_eq!(n_diff, 0);
    }
}

/// S2 controls: `scaleCorrection true`, `symGaussSeidel`, a pre-sweep, the symmetric coarsening of the asymmetric matrix and the
/// `PBiCG`/`DILU` substitute each differ from the real binary.
#[test]
fn s2_controls_each_wrong_choice_differs_from_the_real_tstruct_solve() {
    let d = load_tm();
    let Some(w) = tm_weights(&d.a) else {
        eprintln!("SKIP: upstream tutorials not found; set GENFOAM_UPSTREAM");
        return;
    };
    let up = ref_s2_cycles("s2t");
    let ct = tm_ctl(1e-14, 0.0, 40);
    let mut fwd = true;
    let agg = pair_agglomerate(d.a.n_cells, &d.a.owner, &d.a.neighbour, &w, &ct, &mut fwd).expect("agglomerate");
    let worst = |c: &GamgControls| {
        let mut psi = d.psi0.clone();
        let p = gamg_dic_solve(&d.a, &agg, c, &d.source, &mut psi).expect("solve");
        p.cycles.iter().zip(&up).fold(0.0_f64, |m, (x, y)| m.max(((x - y) / y).abs()))
    };
    let mut ok = Vec::new();
    for (label, c) in [
        ("scaleCorrection true", GamgControls { scale_correction: Some(true), ..ct }),
        ("symGaussSeidel", GamgControls { smoother: GamgSmoother::SymGaussSeidel, ..ct }),
        ("nPreSweeps 1", GamgControls { n_pre_sweeps: 1, ..ct }),
    ] {
        let x = worst(&c);
        println!("S2 control `{label}`: worst relative cycle residual difference {x:e}");
        ok.push(x > 1e-9);
    }
    let mut sym = d.a.clone();
    sym.lower = sym.upper.clone();
    let mats = coarse_matrices(&sym, &agg);
    let n_diff: usize = mats.iter().zip(&d.levels).map(|(m, u)| m.upper.iter().zip(&u.cupper).filter(|(x, y)| x.to_bits() != y.to_bits()).count()).sum();
    println!("S2 control `symmetric coarsening`: coarse upper entries differing {n_diff}");
    ok.push(n_diff > 0);
    let mut psi = d.psi0.clone();
    // at the SHIPPED tolerances (1e-6, relTol 1e-3), where the substitute was used before this round; at 1e-14 both solvers reach the
    // same solution to 4.3e-16 of max (measured 2026-11-27, post-run note in README "Round 72 results")
    let sub = pbicg_dilu(&d.a, &d.source, &mut psi, 1e-6, 1e-3, 1000, 0);
    let sol = ref_s2_solution("s2");
    let rel = psi.iter().zip(&sol).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs())) / sol.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    println!("S2 control `PBiCG/DILU substitute`: {} iterations, solution differs from the real GAMG solution by {rel:e} of max", sub.iterations);
    ok.push(rel > 1e-9);
    assert!(ok.iter().all(|&b| b), "{ok:?}");
}
