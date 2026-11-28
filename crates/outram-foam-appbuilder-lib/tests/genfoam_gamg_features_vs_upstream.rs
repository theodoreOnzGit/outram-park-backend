// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # The GAMG features of gh:#1946 against the real binary's own GAMG (README "Round 85")
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! **Methodology.** Round 72's instrument (`genfoam_gamg_smoothers_vs_upstream.rs`): an instrumented COPY of ESI v2506's `GAMGSolver`
//! (`upstream_driver/gamg_dump/build.sh`) dumps, inside the real run, the matrix, the agglomeration, the residual after every V-cycle and
//! the solution of one solve. **S1**: the FFTF `p_rgh` solve 0 of step 1 (symmetric, 18 538 cells; the dumped matrix equals round 67's
//! `gamg_r67_fftf_step1_matrix` line for line apart from the `TOL`/`NLEVELS` header, checked 2026-11-28), `tolerance 1e-14`, `relTol 0`,
//! upstream stopping at `maxIter 1000`, with ONE edit of the `smoother DIC;` entry each: `a1` `FDIC`, `a2` `nonBlockingGaussSeidel`, `a3`
//! `interpolateCorrection true`, `a4` `directSolveCoarsest true`, `a5` `mergeLevels 2`, `a6` `agglomerator algebraicPair`, `a7`
//! `updateInterval 10`, `a8` `cacheAgglomeration false` (solve 1, agglomerated again). **S2**: the asymmetric `TStruct` solve 0 of the
//! shipped legacy thermo-mechanics case (round 72's matrix, `tolerance 1e-14`, `relTol 0`, `maxIter 40`): `b1` `DILU`, `b2`
//! `DILUGaussSeidel`, `b3` `nonBlockingGaussSeidel`, `b4` `interpolateCorrection true`, `b5` `directSolveCoarsest true`, `b6`
//! `algebraicPair`, `b7` `mergeLevels 2`. Real runs: `scripts/run_gamg_features_real_r85.sh`; references `refs/gamg_r85_<v>_*`.
//!
//! **Criterion** (README round 85, fixed before any run): the agglomeration where dumped, every V-cycle residual and the solution
//! bit-identical. **Controls**: each variant run WITHOUT its feature must differ by more than 1e-9 relative at some cycle (reported, not
//! gated, where the README predicted no difference: `a1`, `a2`, `b3`).
//!
//! **Results** (2026-11-28, release): **all 15 variants bit-identical** to the real binary: every one of the 1 000 (S1) / 40 (S2) V-cycle
//! residuals and every solution entry (18 538 / 23 340), and the agglomeration of `a5` (5 levels), `a6`, `a7` (10), `a8` solve 1 (9 levels,
//! solve 0 had 10: `forward_` continued), `b6` (8), `b7` (4), 0 entries differing. Controls (feature off, worst relative cycle difference):
//! `a3` 0.85, `a4` 4.3e-8, `a5` 1.0, `a6` 0.97, `a7` 9.5, `a8` (cached agglomeration) 32, `b1` 1.1e5, `b2` 9.4e7, `b4` 2.1e-2, `b6` 0.47,
//! `b7` 0.81: all differ; `a1` 0 and `b3` 0 (predicted); `b5` 0 (NOT predicted: the real `directSolveCoarsest` cycles equal the
//! `PBiCGStab` coarsest ones bit for bit). Also: the real `a1` (FDIC) cycles equal round 67's DIC reference and the real `a2`
//! (nonBlockingGaussSeidel) cycles round 72's `gs`, line for line. README "Round 85 results".

#![cfg(feature = "std")]
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::common::gamg::{
    algebraic_pair_weights, face_area_pair_weights, face_area_pair_weights_at, gamg_dic_solve, pair_agglomerate, GamgAgglomeration, GamgControls, GamgSmoother,
};
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_basic_lib::prelude::LduMatrix;

fn refs() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361/refs")
}

fn gunzip_bytes(path: &Path) -> Vec<u8> {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => panic!("read {}: {e}", path.display()),
    };
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
    match decompress_to_vec(&bytes[at..bytes.len() - 8]) {
        Ok(v) => v,
        Err(e) => panic!("inflate {}: {e:?}", path.display()),
    }
}

fn gunzip(path: &Path) -> String {
    match String::from_utf8(gunzip_bytes(path)) {
        Ok(s) => s,
        Err(e) => panic!("utf8 {}: {e}", path.display()),
    }
}

fn num(s: &str) -> f64 {
    match s.parse::<f64>() {
        Ok(v) => v,
        Err(e) => panic!("number `{s}`: {e}"),
    }
}

/// The values of the first line starting with `key n` in `text`.
fn list(text: &str, key: &str) -> Vec<f64> {
    for l in text.lines() {
        let mut it = l.split_whitespace();
        if it.next() == Some(key) {
            let n = num(it.next().unwrap_or("x")) as usize;
            let v: Vec<f64> = it.map(num).collect();
            assert_eq!(v.len(), n, "{key}");
            return v;
        }
    }
    panic!("key {key} missing");
}

fn matrix_from(text: &str) -> (LduMatrix, Vec<f64>, Vec<f64>) {
    let lower: Vec<usize> = list(text, "LOWER").into_iter().map(|v| v as usize).collect();
    let upper: Vec<usize> = list(text, "UPPER").into_iter().map(|v| v as usize).collect();
    let diag = list(text, "DIAG");
    let mut a = LduMatrix::new(diag.len(), lower, upper);
    a.diag = diag;
    a.upper = list(text, "UPPERC");
    a.lower = if text.lines().any(|l| l.starts_with("LOWERC ")) { list(text, "LOWERC") } else { a.upper.clone() };
    (a, list(text, "SOURCE"), list(text, "PSI0"))
}

/// The per-level `RESTRICT` and `FACERESTRICT` lists of a dumped agglomeration.
fn dumped_agglomeration(name: &str) -> Vec<(Vec<f64>, Vec<f64>)> {
    let text = gunzip(&refs().join(name));
    let mut out = Vec::new();
    let mut block = String::new();
    let flush = |block: &mut String, out: &mut Vec<(Vec<f64>, Vec<f64>)>| {
        if !block.is_empty() {
            out.push((list(block, "RESTRICT"), list(block, "FACERESTRICT")));
            block.clear();
        }
    };
    for line in text.lines() {
        if line.starts_with("LEVEL") {
            flush(&mut block, &mut out);
        } else if !line.starts_with("NLEVELS") {
            block.push_str(line);
            block.push('\n');
        }
    }
    flush(&mut block, &mut out);
    out
}

/// Agglomeration entries that differ from the dump (and the level counts).
fn agglomeration_difference(agg: &GamgAgglomeration, dumped: &[(Vec<f64>, Vec<f64>)]) -> (usize, usize, usize) {
    let mut n = 0usize;
    for (l, (r, fr)) in agg.levels.iter().zip(dumped) {
        n += l.restrict.iter().zip(r).filter(|(x, y)| **x as f64 != **y).count();
        n += l.face_restrict.iter().zip(fr).filter(|(x, y)| **x as f64 != **y).count();
        n += l.restrict.len().abs_diff(r.len()) + l.face_restrict.len().abs_diff(fr.len());
    }
    (n, agg.levels.len(), dumped.len())
}

/// `VC <solve> <cycle> <residual>` lines of solve `solve`.
fn ref_cycles(v: &str, solve: usize) -> Vec<f64> {
    let tag = format!("VC {solve} ");
    gunzip(&refs().join(format!("gamg_r85_{v}_cycles.ref.txt.gz"))).lines().filter(|l| l.starts_with(&tag)).map(|l| num(l.split_whitespace().nth(3).unwrap_or("x"))).collect()
}

fn ref_solution(v: &str) -> Vec<f64> {
    list(&gunzip(&refs().join(format!("gamg_r85_{v}_solution.ref.txt.gz"))), "PSI")
}

fn upstream_tutorials() -> PathBuf {
    PathBuf::from(std::env::var("GENFOAM_UPSTREAM").unwrap_or_else(|_| "/home/user/upstream-build/GeN-Foam-src".to_string()))
}

/// The internal-face `Sf` of the FFTF fluid mesh (upstream tutorial).
fn fftf_sf(n_internal: usize) -> Option<Vec<[f64; 3]>> {
    let p = upstream_tutorials().join("Tutorials/reactorCases/2D_FFTF/rootCase/constant/fluidRegion/polyMesh");
    if !p.join("owner").is_file() {
        return None;
    }
    let mesh = match read_poly_mesh(&p) {
        Ok(m) => m,
        Err(e) => panic!("FFTF fluid polyMesh: {e}"),
    };
    Some(mesh.face_area_vectors[..n_internal].iter().map(|v| [v.x, v.y, v.z]).collect())
}

static TM_MESH_CALL: AtomicUsize = AtomicUsize::new(0);

/// The internal-face `Sf` of the legacy thermo-mechanics mesh (`cases/legacy_tm_esfr`, gunzipped into a scratch directory unique per call).
fn tm_sf(n_internal: usize) -> Option<Vec<[f64; 3]>> {
    let src = refs().join("../cases/legacy_tm_esfr/constant/thermoMechanicalRegion/polyMesh");
    let call = TM_MESH_CALL.fetch_add(1, Ordering::SeqCst);
    let dst = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1946_tm_polymesh_{}_{call}", std::process::id()));
    if std::fs::create_dir_all(&dst).is_err() {
        return None;
    }
    for f in ["points", "faces", "owner", "neighbour", "cellZones", "boundary"] {
        let gz = src.join(format!("{f}.gz"));
        let ok = if gz.is_file() { std::fs::write(dst.join(f), gunzip_bytes(&gz)).is_ok() } else { std::fs::copy(src.join(f), dst.join(f)).is_ok() };
        if !ok {
            return None;
        }
    }
    let mesh = match read_poly_mesh(&dst) {
        Ok(m) => m,
        Err(e) => panic!("TM polyMesh: {e}"),
    };
    let _ = std::fs::remove_dir_all(&dst);
    Some(mesh.face_area_vectors[..n_internal].iter().map(|v| [v.x, v.y, v.z]).collect())
}

struct Case {
    a: LduMatrix,
    source: Vec<f64>,
    psi0: Vec<f64>,
    sf: Vec<[f64; 3]>,
}

fn fftf() -> Option<Case> {
    let (a, source, psi0) = matrix_from(&gunzip(&refs().join("gamg_r67_fftf_step1_matrix.ref.txt.gz")));
    let sf = fftf_sf(a.n_internal_faces)?;
    Some(Case { a, source, psi0, sf })
}

fn tstruct() -> Option<Case> {
    let (a, source, psi0) = matrix_from(&gunzip(&refs().join("gamg_r72_s2_tstruct_matrix.ref.txt.gz")));
    let sf = tm_sf(a.n_internal_faces)?;
    Some(Case { a, source, psi0, sf })
}

/// The weights the variant's agglomerator uses (`a6`/`b6`: `algebraicPair`; `a7`: `updateInterval 10` built at time index 1).
fn weights(v: &str, c: &Case) -> Vec<f64> {
    match v {
        "a6" => algebraic_pair_weights(&c.a, false),
        "b6" => algebraic_pair_weights(&c.a, true),
        "a7" => face_area_pair_weights_at(&c.sf, 10, 1),
        _ => face_area_pair_weights(&c.sf),
    }
}

fn s1_base() -> GamgControls {
    GamgControls { tolerance: 1e-14, rel_tol: 0.0, ..GamgControls::default() }
}

fn s2_base() -> GamgControls {
    GamgControls { smoother: GamgSmoother::GaussSeidel, tolerance: 1e-14, rel_tol: 0.0, max_iter: 40, n_cells_in_coarsest_level: 50, ..GamgControls::default() }
}

/// The port's controls of each variant (`with_feature`), or the default the variant edits (`!with_feature`, the control).
fn controls(v: &str, with_feature: bool) -> GamgControls {
    let s1 = s1_base();
    let s2 = s2_base();
    if !with_feature {
        return if v.starts_with('a') { s1 } else { s2 };
    }
    match v {
        "a1" => GamgControls { smoother: GamgSmoother::Fdic, ..s1 },
        "a2" => GamgControls { smoother: GamgSmoother::NonBlockingGaussSeidel, ..s1 },
        "a3" => GamgControls { interpolate_correction: true, ..s1 },
        "a4" => GamgControls { direct_solve_coarsest: true, ..s1 },
        "a5" => GamgControls { merge_levels: 2, ..s1 },
        "a6" | "a7" | "a8" => s1,
        "b1" => GamgControls { smoother: GamgSmoother::Dilu, ..s2 },
        "b2" => GamgControls { smoother: GamgSmoother::DiluGaussSeidel, ..s2 },
        "b3" => GamgControls { smoother: GamgSmoother::NonBlockingGaussSeidel, ..s2 },
        "b4" => GamgControls { interpolate_correction: true, ..s2 },
        "b5" => GamgControls { direct_solve_coarsest: true, ..s2 },
        "b7" => GamgControls { merge_levels: 2, ..s2 },
        _ => s2,
    }
}

fn agglomerate(c: &Case, w: &[f64], ctl: &GamgControls, forward: &mut bool) -> GamgAgglomeration {
    match pair_agglomerate(c.a.n_cells, &c.a.owner, &c.a.neighbour, w, ctl, forward) {
        Ok(a) => a,
        Err(e) => panic!("agglomerate: {e}"),
    }
}

fn solve(a: &LduMatrix, agg: &GamgAgglomeration, ctl: &GamgControls, source: &[f64], psi0: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let mut psi = psi0.to_vec();
    match gamg_dic_solve(a, agg, ctl, source, &mut psi) {
        Ok(p) => (p.cycles, psi),
        Err(e) => panic!("solve: {e}"),
    }
}

fn first_difference(port: &[f64], up: &[f64]) -> Option<(usize, f64, f64)> {
    port.iter().zip(up).enumerate().find(|(_, (x, y))| x.to_bits() != y.to_bits()).map(|(k, (x, y))| (k + 1, *x, *y))
}

fn worst_rel(port: &[f64], up: &[f64], n: usize) -> f64 {
    port.iter().zip(up).take(n).fold(0.0_f64, |m, (x, y)| m.max(((x - y) / y).abs()))
}

/// The variants whose agglomeration differs from the default's, so it is compared with the dump.
fn agglomeration_dumped(v: &str) -> bool {
    matches!(v, "a5" | "a6" | "a7" | "b6" | "b7")
}

/// One variant: `(cycles bit-identical, psi entries differing, agglomeration entries differing, control worst relative difference)`.
fn run_variant(v: &str, c: &Case) -> (Option<(usize, f64, f64)>, usize, usize, f64) {
    let ctl = controls(v, true);
    let w = weights(v, c);
    let mut fwd = true;
    let agg = agglomerate(c, &w, &ctl, &mut fwd);
    let agg_diff = if agglomeration_dumped(v) {
        let d = agglomeration_difference(&agg, &dumped_agglomeration(&format!("gamg_r85_{v}_agglom.ref.txt.gz")));
        println!("{v}: agglomeration levels {} (upstream {}), entries differing {}", d.1, d.2, d.0);
        d.0 + d.1.abs_diff(d.2)
    } else {
        0
    };
    let t0 = std::time::Instant::now();
    let (cycles, psi) = solve(&c.a, &agg, &ctl, &c.source, &c.psi0);
    let up = ref_cycles(v, 0);
    let sol = ref_solution(v);
    let first = if cycles.len() == up.len() { first_difference(&cycles, &up) } else { Some((0, cycles.len() as f64, up.len() as f64)) };
    let n_diff = psi.iter().zip(&sol).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
    // control: the same reference, the feature off (the default agglomeration and controls)
    let cc = controls(v, false);
    let mut f2 = true;
    let agg0 = agglomerate(c, &face_area_pair_weights(&c.sf), &cc, &mut f2);
    let n_ctl = if v.starts_with('a') { 30 } else { 40 };
    let (cyc0, _) = solve(&c.a, &agg0, &GamgControls { max_iter: n_ctl, ..cc }, &c.source, &c.psi0);
    let control = worst_rel(&cyc0, &up, n_ctl);
    println!(
        "{v}: {} cycles (upstream {}), first differing cycle {first:?}, worst relative cycle difference {:e}, psi entries not bit-identical {n_diff} of {}, {:?}; CONTROL (feature off) worst relative cycle difference over {n_ctl} cycles {control:e}",
        cycles.len(),
        up.len(),
        worst_rel(&cycles, &up, up.len()),
        sol.len(),
        t0.elapsed()
    );
    (first, n_diff, agg_diff, control)
}

/// S1: `a1`-`a7` against the real FFTF `p_rgh` solve. Criterion: bit-identical (agglomeration where it differs from the default, every
/// one of the 1 000 cycles, the solution); controls (feature off) differ by more than 1e-9, except `a1`/`a2` (predicted equal: reported).
#[test]
fn s1_round85_features_reproduce_the_real_fftf_solve() {
    let Some(c) = fftf() else {
        eprintln!("SKIP: upstream tutorials not found; set GENFOAM_UPSTREAM");
        return;
    };
    let mut fails = Vec::new();
    for v in ["a1", "a2", "a3", "a4", "a5", "a6", "a7"] {
        let (first, n_diff, agg_diff, control) = run_variant(v, &c);
        if first.is_some() || n_diff != 0 || agg_diff != 0 {
            fails.push(format!("{v}: first {first:?} psi {n_diff} agg {agg_diff}"));
        }
        if !matches!(v, "a1" | "a2") && control <= 1e-9 {
            fails.push(format!("{v}: control does not discriminate ({control:e})"));
        }
    }
    assert!(fails.is_empty(), "{fails:?}");
}

/// S1 `a8`, `cacheAgglomeration false`: upstream deletes the agglomeration after solve 0 and agglomerates again for solve 1, from the
/// `forward_` solve 0's build left. Criterion: solve 0's cycles equal the real ones; solve 1's agglomeration (built by the port from the
/// continued `forward_`) equals the dumped one; solve 1's cycles and solution bit-identical. Control: solve 1 with solve 0's (cached)
/// agglomeration.
#[test]
fn s1_a8_cache_agglomeration_false_agglomerates_again_as_the_real_binary() {
    let Some(c) = fftf() else {
        eprintln!("SKIP: upstream tutorials not found; set GENFOAM_UPSTREAM");
        return;
    };
    let ctl = GamgControls { cache_agglomeration: false, ..s1_base() };
    let w = face_area_pair_weights(&c.sf);
    let mut fwd = true;
    let agg0 = agglomerate(&c, &w, &ctl, &mut fwd);
    let (cyc0, _) = solve(&c.a, &agg0, &ctl, &c.source, &c.psi0);
    let up0 = ref_cycles("a8", 0);
    let first0 = first_difference(&cyc0, &up0);
    let (a1, source1, psi1) = matrix_from(&gunzip(&refs().join("gamg_r85_a8_matrix.ref.txt.gz")));
    let agg1 = agglomerate(&Case { a: a1.clone(), source: Vec::new(), psi0: Vec::new(), sf: Vec::new() }, &w, &ctl, &mut fwd);
    let d = agglomeration_difference(&agg1, &dumped_agglomeration("gamg_r85_a8_agglom.ref.txt.gz"));
    let (cyc1, psi) = solve(&a1, &agg1, &ctl, &source1, &psi1);
    let up1 = ref_cycles("a8", 1);
    let first1 = if cyc1.len() == up1.len() { first_difference(&cyc1, &up1) } else { Some((0, cyc1.len() as f64, up1.len() as f64)) };
    let sol = ref_solution("a8");
    let n_diff = psi.iter().zip(&sol).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
    let (cyc_ctl, _) = solve(&a1, &agg0, &GamgControls { max_iter: 30, ..ctl }, &source1, &psi1);
    let control = worst_rel(&cyc_ctl, &up1, 30);
    println!(
        "a8: solve 0 first differing cycle {first0:?} ({} cycles); solve 0 levels {}, solve 1 levels {} (upstream {}), solve-1 agglomeration entries differing {}; solve 1: {} cycles (upstream {}), first differing {first1:?}, psi entries not bit-identical {n_diff}; CONTROL (solve 0's agglomeration kept) worst relative cycle difference {control:e}",
        cyc0.len(),
        agg0.levels.len(),
        d.1,
        d.2,
        d.0,
        cyc1.len(),
        up1.len()
    );
    assert_eq!(first0, None);
    assert_eq!((d.0, d.1), (0, d.2));
    assert_eq!(first1, None);
    assert_eq!(n_diff, 0);
    assert!(control > 1e-9, "the re-agglomeration must matter: {control:e}");
}

/// S2: `b1`-`b7` against the real asymmetric `TStruct` solve. Criterion as S1 (40 cycles); controls differ, except `b3` (predicted
/// equal: reported).
#[test]
fn s2_round85_features_reproduce_the_real_tstruct_solve() {
    let Some(c) = tstruct() else {
        eprintln!("SKIP: legacy thermo-mechanics mesh not found");
        return;
    };
    let mut fails = Vec::new();
    for v in ["b1", "b2", "b3", "b4", "b5", "b6", "b7"] {
        let (first, n_diff, agg_diff, control) = run_variant(v, &c);
        if first.is_some() || n_diff != 0 || agg_diff != 0 {
            fails.push(format!("{v}: first {first:?} psi {n_diff} agg {agg_diff}"));
        }
        // `b5` (POST-RUN, README round 85 results, prediction A15 MISSED for it): the real `directSolveCoarsest` cycles equal round 72's
        // `s2t` (PBiCGStab/DILU on the coarsest level) bit for bit, so its control cannot discriminate; reported, not gated
        if v != "b3" && v != "b5" && control <= 1e-9 {
            fails.push(format!("{v}: control does not discriminate ({control:e})"));
        }
    }
    assert!(fails.is_empty(), "{fails:?}");
}
