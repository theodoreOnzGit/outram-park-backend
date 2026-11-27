// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # GAMG (`faceAreaPair`, `DIC`) against the real binary's own GAMG on the shipped 2D_FFTF `p_rgh` matrix (gh:#1934, README "Round 67")
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! **Methodology.** The reference is a dump taken INSIDE the real `GeN-Foam` (652b3da on ESI v2506) run of the shipped
//! `2D_FFTF/rootCase` steady state, at the first `p_rgh` solve of step 1, by an instrumented COPY of `GAMGSolver`
//! (`upstream_driver/gamg_dump`, `scripts/run_fftf_gamg_real.sh`, tight tolerances, `p_rgh` keeps `solver GAMG; smoother DIC`):
//! the fine matrix and source, the whole agglomeration, every coarse matrix, the residual after every V-cycle and the final solution
//! (`refs/gamg_r67_fftf_step1_*`). (a) the port builds the agglomeration from the FFTF mesh alone and must equal the dump exactly;
//! (b) the port solves the dumped system and must reproduce every V-cycle residual and the solution bit for bit. Controls: the old
//! algebraic `|upper|` weights, `magSf` weights, a `forward_` started `false`, one finest sweep, no scaling must each differ.
//! **Results**: README "Round 67 results".

#![cfg(feature = "std")]
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::common::gamg::{coarse_matrices, face_area_pair_weights, gamg_dic_solve, pair_agglomerate, GamgAgglomeration, GamgControls};
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_basic_lib::prelude::LduMatrix;

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> String {
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
    String::from_utf8(decompress_to_vec(&bytes[at..bytes.len() - 8]).expect("inflate")).expect("utf8")
}

/// `KEY n v1 .. vn` lines of a dump file (the header lines are skipped).
fn lists(text: &str) -> Vec<(String, Vec<String>)> {
    text.lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let key = it.next()?;
            let n: usize = it.next()?.parse().ok()?;
            let v: Vec<String> = it.map(str::to_string).collect();
            if v.len() == n && !key.starts_with("LEVEL") && key != "NCELLS" && key != "NLEVELS" {
                Some((key.to_string(), v))
            } else {
                None
            }
        })
        .collect()
}

fn ints(v: &[String]) -> Vec<i64> {
    v.iter().map(|s| s.parse().expect("int")).collect()
}

fn reals(v: &[String]) -> Vec<f64> {
    v.iter().map(|s| s.parse().expect("float")).collect()
}

fn mesh_dir() -> Option<PathBuf> {
    let up = PathBuf::from(std::env::var("GENFOAM_UPSTREAM").unwrap_or_else(|_| "/home/user/upstream-build/GeN-Foam-src".to_string()));
    let p = up.join("Tutorials/reactorCases/2D_FFTF/rootCase/constant/fluidRegion/polyMesh");
    p.join("owner").is_file().then_some(p)
}

struct Dump {
    lower: Vec<usize>,
    upper: Vec<usize>,
    diag: Vec<f64>,
    upper_c: Vec<f64>,
    source: Vec<f64>,
    psi0: Vec<f64>,
    levels: Vec<DumpLevel>,
    solution: Vec<f64>,
    cycles: Vec<f64>,
}

struct DumpLevel {
    n_coarse: usize,
    n_faces: usize,
    restrict: Vec<i64>,
    face_restrict: Vec<i64>,
    face_flip: Vec<i64>,
    clower: Vec<i64>,
    cupper: Vec<i64>,
    cdiag: Vec<f64>,
    cupper_c: Vec<f64>,
}

fn load_dump() -> Dump {
    let r = vv_dir().join("refs");
    let m = gunzip(&r.join("gamg_r67_fftf_step1_matrix.ref.txt.gz"));
    let get = |text: &str, key: &str| lists(text).into_iter().find(|(k, _)| k == key).unwrap_or_else(|| panic!("{key}")).1;
    let lower = ints(&get(&m, "LOWER")).into_iter().map(|v| v as usize).collect();
    let upper = ints(&get(&m, "UPPER")).into_iter().map(|v| v as usize).collect();
    let a = gunzip(&r.join("gamg_r67_fftf_step1_agglom.ref.txt.gz"));
    let mut levels = Vec::new();
    let mut cur: Option<(usize, usize)> = None;
    let mut acc: Vec<(String, Vec<String>)> = Vec::new();
    let mut flush = |cur: &mut Option<(usize, usize)>, acc: &mut Vec<(String, Vec<String>)>, levels: &mut Vec<DumpLevel>| {
        if let Some((nc, nf)) = cur.take() {
            let g = |k: &str| acc.iter().find(|(kk, _)| kk == k).unwrap_or_else(|| panic!("{k}")).1.clone();
            levels.push(DumpLevel {
                n_coarse: nc,
                n_faces: nf,
                restrict: ints(&g("RESTRICT")),
                face_restrict: ints(&g("FACERESTRICT")),
                face_flip: ints(&g("FACEFLIP")),
                clower: ints(&g("CLOWER")),
                cupper: ints(&g("CUPPER")),
                cdiag: reals(&g("CDIAG")),
                cupper_c: reals(&g("CUPPERC")),
            });
            acc.clear();
        }
    };
    for line in a.lines() {
        if line.starts_with("LEVEL") {
            flush(&mut cur, &mut acc, &mut levels);
            let w: Vec<&str> = line.split_whitespace().collect();
            cur = Some((w[3].parse().expect("n"), w[5].parse().expect("n")));
        } else if let Some((k, v)) = lists(line).into_iter().next() {
            acc.push((k, v));
        }
    }
    flush(&mut cur, &mut acc, &mut levels);
    let cycles = gunzip(&r.join("gamg_r67_fftf_step1_cycles.ref.txt.gz"))
        .lines()
        .filter(|l| l.starts_with("VC 0 "))
        .map(|l| l.split_whitespace().nth(3).expect("res").parse().expect("f"))
        .collect();
    Dump {
        lower,
        upper,
        diag: reals(&get(&m, "DIAG")),
        upper_c: reals(&get(&m, "UPPERC")),
        source: reals(&get(&m, "SOURCE")),
        psi0: reals(&get(&m, "PSI0")),
        levels,
        solution: reals(&get(&gunzip(&r.join("gamg_r67_fftf_step1_solution.ref.txt.gz")), "PSI")),
        cycles,
    }
}

fn dump_matrix(d: &Dump) -> LduMatrix {
    let mut m = LduMatrix::new(d.diag.len(), d.lower.clone(), d.upper.clone());
    m.diag = d.diag.clone();
    m.upper = d.upper_c.clone();
    m.lower = d.upper_c.clone();
    m
}

fn tight() -> GamgControls {
    GamgControls { tolerance: 1e-14, rel_tol: 0.0, ..GamgControls::default() }
}

fn agglomerate_ffft(weights: &[f64], forward: bool, a: &LduMatrix) -> GamgAgglomeration {
    let mut fwd = forward;
    pair_agglomerate(a.n_cells, &a.owner, &a.neighbour, weights, &tight(), &mut fwd).expect("agglomerate")
}

/// Number of entries of `agg` that differ from the dump (levels count, then every array).
fn agglomeration_differences(agg: &GamgAgglomeration, d: &Dump) -> usize {
    let mut diff = agg.levels.len().abs_diff(d.levels.len());
    for (l, u) in agg.levels.iter().zip(&d.levels) {
        diff += l.n_cells.abs_diff(u.n_coarse) + l.n_faces.abs_diff(u.n_faces);
        let cmp = |a: &[i64], b: &[i64]| a.len().abs_diff(b.len()) + a.iter().zip(b).filter(|(x, y)| x != y).count();
        diff += cmp(&l.restrict.iter().map(|&v| v as i64).collect::<Vec<_>>(), &u.restrict);
        diff += cmp(&l.face_restrict.iter().map(|&v| v as i64).collect::<Vec<_>>(), &u.face_restrict);
        diff += cmp(&l.face_flip.iter().map(|&v| i64::from(v)).collect::<Vec<_>>(), &u.face_flip);
        diff += cmp(&l.lower.iter().map(|&v| v as i64).collect::<Vec<_>>(), &u.clower);
        diff += cmp(&l.upper.iter().map(|&v| v as i64).collect::<Vec<_>>(), &u.cupper);
    }
    diff
}

fn ffft_weights(dir: &Path, n_internal_expected: usize) -> Vec<f64> {
    let mesh = read_poly_mesh(dir).expect("read FFTF fluid polyMesh");
    let ni = mesh.n_internal_faces;
    assert_eq!(ni, n_internal_expected);
    let sf: Vec<[f64; 3]> = mesh.face_area_vectors[..ni].iter().map(|v| [v.x, v.y, v.z]).collect();
    face_area_pair_weights(&sf)
}

/// (a) criterion: the agglomeration of the FFTF fluid mesh equals the real binary's with ZERO differences, and every coarse matrix is
/// bit-identical.
#[test]
fn a_agglomeration_and_coarse_matrices_equal_the_real_binarys_bit_for_bit() {
    let Some(dir) = mesh_dir() else {
        eprintln!("SKIP: upstream tutorials not found; set GENFOAM_UPSTREAM");
        return;
    };
    let d = load_dump();
    let a = dump_matrix(&d);
    let w = ffft_weights(&dir, a.n_internal_faces);
    let agg = agglomerate_ffft(&w, true, &a);
    let diff = agglomeration_differences(&agg, &d);
    println!("(a) levels {} (upstream {}); agglomeration entries that differ: {diff}", agg.levels.len(), d.levels.len());
    let mats = coarse_matrices(&a, &agg);
    let mut worst = 0usize;
    for (m, u) in mats.iter().zip(&d.levels) {
        worst += m.diag.iter().zip(&u.cdiag).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
        worst += m.upper.iter().zip(&u.cupper_c).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
    }
    println!("(a) coarse diag/upper entries not bit-identical: {worst} over {} levels", mats.len());
    assert_eq!(diff, 0, "agglomeration differs");
    assert_eq!(worst, 0, "coarse matrices differ");
}

/// (b) criterion: every V-cycle residual of the dumped solve, the iteration count and the final solution equal upstream's bit for bit.
#[test]
fn b_every_v_cycle_of_the_dumped_solve_equals_the_real_binarys() {
    let Some(dir) = mesh_dir() else {
        eprintln!("SKIP: upstream tutorials not found; set GENFOAM_UPSTREAM");
        return;
    };
    let d = load_dump();
    let a = dump_matrix(&d);
    let w = ffft_weights(&dir, a.n_internal_faces);
    let agg = agglomerate_ffft(&w, true, &a);
    let mut psi = d.psi0.clone();
    let t0 = std::time::Instant::now();
    let perf = gamg_dic_solve(&a, &agg, &tight(), &d.source, &mut psi).expect("solve");
    println!("(b) {} V-cycles in {:?}; upstream {}", perf.iterations, t0.elapsed(), d.cycles.len());
    let mut first = None;
    let mut worst_rel = 0.0_f64;
    for (k, (x, y)) in perf.cycles.iter().zip(&d.cycles).enumerate() {
        if x.to_bits() != y.to_bits() {
            first.get_or_insert((k + 1, *x, *y));
        }
        worst_rel = worst_rel.max(((x - y) / y).abs());
    }
    let psi_diff = psi.iter().zip(&d.solution).filter(|(x, y)| x.to_bits() != y.to_bits()).count();
    let psi_worst = psi.iter().zip(&d.solution).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs())) / d.solution.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    println!("(b) first differing cycle {first:?}; worst relative residual difference {worst_rel:e}; psi entries not bit-identical {psi_diff} (worst {psi_worst:e} of max|psi|)");
    println!("(b) initial residual {:e} (upstream 0.60661745654371468)", perf.initial_residual);
    assert_eq!(perf.iterations, d.cycles.len());
    assert!(first.is_none(), "cycles differ: {first:?}");
    assert_eq!(psi_diff, 0);
}

/// Controls: each wrong choice must change the agglomeration or the V-cycle residuals by more than 1e-9 relative at some cycle.
#[test]
fn controls_each_wrong_choice_differs_from_the_real_binary() {
    let Some(dir) = mesh_dir() else {
        eprintln!("SKIP: upstream tutorials not found; set GENFOAM_UPSTREAM");
        return;
    };
    let d = load_dump();
    let a = dump_matrix(&d);
    let w = ffft_weights(&dir, a.n_internal_faces);
    let mesh = read_poly_mesh(&dir).expect("mesh");
    let mag_sf: Vec<f64> = mesh.face_areas[..a.n_internal_faces].to_vec();
    let abs_upper: Vec<f64> = a.upper.iter().map(|v| v.abs()).collect();
    let cap = GamgControls { max_iter: 30, ..tight() };
    let compare = |agg: &GamgAgglomeration, ctl: &GamgControls| -> f64 {
        let mut psi = d.psi0.clone();
        let p = gamg_dic_solve(&a, agg, ctl, &d.source, &mut psi).expect("solve");
        p.cycles.iter().zip(&d.cycles).fold(0.0_f64, |m, (x, y)| m.max(((x - y) / y).abs()))
    };
    let good = agglomerate_ffft(&w, true, &a);
    let mut report = Vec::new();
    for (name, agg, ctl) in [
        ("weights |upper| (algebraic pairing)", agglomerate_ffft(&abs_upper, true, &a), cap),
        ("weights magSf (updateInterval 0 branch)", agglomerate_ffft(&mag_sf, true, &a), cap),
        ("forward_ started false", agglomerate_ffft(&w, false, &a), cap),
        ("nFinestSweeps 1", good.clone(), GamgControls { n_finest_sweeps: 1, ..cap }),
        ("scaleCorrection false", good.clone(), GamgControls { scale_correction: Some(false), ..cap }),
    ] {
        let diff = compare(&agg, &ctl);
        let agg_diff = agglomeration_differences(&agg, &d);
        println!("control `{name}`: agglomeration entries differing {agg_diff}, worst cycle residual difference over 30 cycles {diff:e}");
        report.push(diff > 1e-9);
    }
    assert!(report.iter().all(|&b| b), "a control did not differ: {report:?}");
}
