// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! # `2D_MSFR` `steadyStateTH`, turbulence OFF, against the real binary (gh:#1361 round 19; gh:#1837)
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## Methodology
//!
//! Upstream: GeN-Foam `652b3da0cc55` on OpenFOAM-ESI v2506, unmodified, `reactorCases/2D_MSFR`,
//! `steadyStateTH` stage replayed by `scripts/run_msfr_th_tight.sh` with `TURB=off` (`turbulence false`): `onePhase`,
//! `solveFluidMechanics true`, `momentumMode cellCentered`, `nCorrectors 6`, `nOuterCorrectors 5`; every linear solver
//! `tolerance 1e-14`, `relTol 0`, `p_rgh` `PCG`/`DIC`; `writePrecision 17`; first step `deltaT 1e-3`, then
//! `onePhase::maxDeltaT` (x1.2 while the Courant number is far below `maxCo 5`).
//! The reference (`refs/msfr_th_off_head.ref.txt.gz`) holds `U`, `p`, `p_rgh`, `T`, `alphaPhi` after each of the first 3
//! steps; `refs/msfr_th_off_log.ref.txt.gz` the log: every `DICPCG` initial residual and iteration count, the `h`
//! residual and the `T (avg min max)` line of every outer iteration.
//!
//! The port starts from the ROOT case (`cases/msfr_th`: `0/fluidRegion`, uniform `T = 900`, `U = 0`, `p = 1e5`; the
//! restart files upstream creates at construction are created the same way), runs its own `onePhase` loop with its
//! own time-step adjustment, and nothing of upstream's results is an input.
//! Pass criterion: `U`, `p`, `p_rgh`, `alphaPhi` to <= 1e-9 relative to the field's max-norm; `T` to <= 1e-9 relative
//! to 900 K AND the per-outer-iteration `T (avg min max)` lines to 1e-9 relative.
//!
//! Predictions: README, "Round 19", committed before the first run. Results: README, "Round 19 results".

// Reads case files: `std` only (gh:#1633).
#![cfg(feature = "std")]
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use miniz_oxide::inflate::decompress_to_vec;
use outram_foam_appbuilder_lib::genfoam::multi_region::coupled_loop::adjust_delta_t;
use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::one_phase_cc::OnePhaseCcSolver;

fn vv_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/genfoam_vs_upstream_gh1361")
}

fn gunzip(path: &Path) -> Vec<u8> {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    // RFC 1952 header: 10 bytes, then the optional FEXTRA/FNAME/FCOMMENT/FHCRC fields the flags announce
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
    decompress_to_vec(&bytes[at..bytes.len() - 8]).unwrap_or_else(|_| panic!("inflate {}", path.display()))
}

fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("mkdir");
    for e in std::fs::read_dir(src).expect("read_dir") {
        let e = e.expect("entry");
        let (s, d) = (e.path(), dst.join(e.file_name()));
        if s.is_dir() {
            copy_dir(&s, &d);
        } else {
            std::fs::copy(&s, &d).expect("copy");
        }
    }
}

/// Stage the root case and the polyMesh; returns `(case root, polyMesh dir)`.
fn stage(tag: &str) -> (PathBuf, PathBuf) {
    let tmp = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("gh1361_r19_{tag}"));
    let _ = std::fs::remove_dir_all(&tmp);
    let root = tmp.join("case");
    for d in ["constant", "system", "0"] {
        copy_dir(&vv_dir().join("cases/msfr_th").join(d), &root.join(d));
    }
    let poly = tmp.join("polyMesh");
    std::fs::create_dir_all(&poly).expect("mkdir");
    let src = vv_dir().join("cases/msfr_neutro/polyMesh");
    for f in ["points", "faces", "owner", "neighbour", "boundary", "cellZones"] {
        std::fs::write(poly.join(f), gunzip(&src.join(format!("{f}.gz")))).expect("polyMesh");
    }
    (root, poly)
}

fn read_ref(path: &Path) -> (HashMap<String, Vec<f64>>, Vec<String>) {
    let text = String::from_utf8(gunzip(path)).expect("utf8");
    let mut m = HashMap::new();
    let mut raw = Vec::new();
    for line in text.lines() {
        raw.push(line.to_string());
        let mut it = line.split_whitespace();
        if let Some(key) = it.next() {
            let rest: Vec<&str> = it.collect();
            if key == "F" {
                let name = format!("{} {}", rest[1], rest[0]);
                m.insert(name, rest[2..].iter().map(|w| w.parse().expect("number")).collect());
            } else if let Some(nums) = rest.iter().map(|w| w.parse::<f64>()).collect::<Result<Vec<f64>, _>>().ok() {
                m.insert(key.to_string(), nums);
            }
        }
    }
    (m, raw)
}

/// `max |a - b| / max |b|`.
fn rel_max(a: &[f64], b: &[f64]) -> f64 {
    assert_eq!(a.len(), b.len());
    let scale = b.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    a.iter().zip(b).fold(0.0_f64, |m, (x, y)| m.max((x - y).abs())) / scale
}

struct StepResult {
    u: f64,
    p: f64,
    p_rgh: f64,
    t: f64,
    alpha_phi: f64,
}

fn run_steps(solver: &mut OnePhaseCcSolver, steps: usize) -> (Vec<StepResult>, Vec<f64>) {
    let (rf, _raw) = read_ref(&vv_dir().join("refs/msfr_th_off_head.ref.txt.gz"));
    let mut dt = solver.case().delta_t;
    let mut results = Vec::new();
    let mut dts = Vec::new();
    let timeprint = rf["TIMEPRINT"].clone();
    for s in 0..steps {
        solver.begin_time_step(dt);
        let (_res, rep) = solver.correct_physics();
        let key = |name: &str| format!("{name} {}", timeprint[s]);
        let u: Vec<f64> = solver.velocity().iter().flat_map(|v| [v.x, v.y, v.z]).collect();
        let ni = rf[&key("alphaPhi")].len();
        let r = StepResult {
            u: rel_max(&u, &rf[&key("U")]),
            p: rel_max(solver.pressure(), &rf[&key("p")]),
            p_rgh: rel_max(solver.p_rgh().expect("flow"), &rf[&key("p_rgh")]),
            t: rel_max(solver.temperature(), &rf[&key("T")]),
            alpha_phi: rel_max(&solver.alpha_phi()[..ni], &rf[&key("alphaPhi")]),
        };
        println!(
            "step {} t={} dt={dt:e}: U {:.3e} p {:.3e} p_rgh {:.3e} T {:.3e} alphaPhi {:.3e}  T stats {:?}",
            s + 1,
            timeprint[s],
            r.u,
            r.p,
            r.p_rgh,
            r.t,
            r.alpha_phi,
            rep.t_stats
        );
        results.push(r);
        dts.push(dt);
        dt = adjust_delta_t(true, &[solver.max_delta_t()], 1e-300, dt).expect("adjustable");
    }
    (results, dts)
}

/// Steps 1-3 of `steadyStateTH` with `turbulence false` against the real binary.
#[test]
fn steady_state_th_turbulence_off_matches_the_real_binary() {
    let (root, poly) = stage("main");
    let mut solver = OnePhaseCcSolver::from_case_dir(&root, &poly).expect("rootCase");
    let (results, dts) = run_steps(&mut solver, 3);
    let (rf, _) = read_ref(&vv_dir().join("refs/msfr_th_off_head.ref.txt.gz"));
    // the time step sequence is upstream's
    for (i, d) in dts.iter().enumerate().skip(1) {
        assert!((d - rf["DT"][i - 1]).abs() <= 1e-15 * d, "dt {i}: {d} vs {}", rf["DT"][i - 1]);
    }
    for (i, r) in results.iter().enumerate() {
        assert!(r.u <= 1e-9, "step {} U {:e}", i + 1, r.u);
        assert!(r.p <= 1e-9, "step {} p {:e}", i + 1, r.p);
        assert!(r.p_rgh <= 1e-9, "step {} p_rgh {:e}", i + 1, r.p_rgh);
        assert!(r.t <= 1e-9, "step {} T {:e}", i + 1, r.t);
        assert!(r.alpha_phi <= 1e-9, "step {} alphaPhi {:e}", i + 1, r.alpha_phi);
    }
}

/// `DICPCG` residuals and iteration counts, and the per-outer-iteration `T (avg min max)` of the log.
///
/// The first corrector of every outer iteration (initial residual 1e-6 to 1) is gated: residual to 1e-6 relative and
/// the iteration count to within one; so are correctors 2 and 3 of the first outer iteration of each step (the
/// residual is still above 1e-10 there, the iteration counts of upstream and port are equal; their residuals are held to
/// 1e-12 ABSOLUTE, the noise floor, which `d / 1e-6 <= 1e-6` encodes). From the 4th corrector
/// on the initial residual (~1e-12) is the round-off floor of the previous solve's tail: both codes start at ~3e-12
/// and need 69 to 150 iterations to reach 1e-14, a count that carries no information; only "both below 1e-10" is
/// gated there, and the state those solves converge to is gated to 1e-9 by the test above.
#[test]
fn pressure_solves_and_temperature_lines_follow_the_upstream_log() {
    let (root, poly) = stage("log");
    let mut solver = OnePhaseCcSolver::from_case_dir(&root, &poly).expect("rootCase");
    let text = String::from_utf8(gunzip(&vv_dir().join("refs/msfr_th_off_log.ref.txt.gz"))).expect("utf8");
    let mut dt = solver.case().delta_t;
    let mut worst_res = 0.0_f64;
    let mut worst_it = 0_i64;
    let mut worst_t = 0.0_f64;
    let mut max_floor = 0.0_f64;
    for step in 1..=3usize {
        solver.begin_time_step(dt);
        let (_r, rep) = solver.correct_physics();
        let solves = solver.pressure_solves().to_vec();
        assert_eq!(solves.len(), 30, "5 outer iterations x 6 correctors");
        for line in text.lines() {
            let w: Vec<&str> = line.split_whitespace().collect();
            if w[0] == "PRGH" && w[1].parse::<usize>().unwrap() == step {
                let (o, c) = (w[2].parse::<usize>().unwrap(), w[3].parse::<usize>().unwrap());
                let (ini, it) = (w[4].parse::<f64>().unwrap(), w[5].parse::<i64>().unwrap());
                let mine = solves.iter().find(|x| x.outer == o && x.corrector == c).expect("solve");
                if c == 1 || (o == 1 && c <= 3) {
                    worst_it = worst_it.max((mine.iterations as i64 - it).abs());
                    // a first corrector: relative; correctors 2-3 sit within ~1e-12 of the round-off floor: absolute
                    let d = (mine.initial_residual - ini).abs();
                    worst_res = worst_res.max(if c == 1 { d / ini } else { d / 1e-6 });
                } else if c >= 4 {
                    max_floor = max_floor.max(mine.initial_residual).max(ini);
                }
            }
            if w[0] == "TSTAT" && w[1].parse::<usize>().unwrap() == step {
                let o = w[2].parse::<usize>().unwrap();
                let (a, mn, mx) = rep.t_stats[o - 1];
                for (x, y) in [(a, w[3]), (mn, w[4]), (mx, w[5])] {
                    worst_t = worst_t.max((x - y.parse::<f64>().unwrap()).abs() / 900.0);
                }
            }
        }
        dt = adjust_delta_t(true, &[solver.max_delta_t()], 1e-300, dt).expect("adjustable");
    }
    println!(
        "gated correctors: worst initial-residual rel diff {worst_res:e}, worst iteration diff {worst_it}; \
         correctors >= 4: both below {max_floor:e}; worst T-line diff {worst_t:e}"
    );
    assert!(worst_res <= 1e-6, "{worst_res:e}");
    assert!(worst_it <= 1, "{worst_it}");
    assert!(max_floor <= 1e-10, "{max_floor:e}");
    assert!(worst_t <= 1e-9, "{worst_t:e}");
}

/// The lazy first-step old-time pressure (`fvc::ddt(p_)` creates `p.oldTime()` after the first solve) is what keeps
/// `T` within 3e-8 K of 900 K in upstream; the ablation (the initial 1e5 Pa as old pressure) heats the fluid by
/// ~`Delta p/(rho0 Cp)`.
#[test]
fn ablating_the_lazy_old_pressure_moves_t_by_millikelvin() {
    let (root, poly) = stage("lazy");
    let mut solver = OnePhaseCcSolver::from_case_dir(&root, &poly).expect("rootCase").without_first_step_lazy_old_pressure();
    solver.begin_time_step(1e-3);
    let (_r, rep) = solver.correct_physics();
    let (avg, mn, mx) = rep.t_stats[0];
    println!("ablated: T avg/min/max after outer iteration 1: {avg} {mn} {mx}");
    assert!((mx - mn) > 1e-4, "{}", mx - mn);
}

/// The solved flow refuses what is not ported: `turbulence true` (gh:#1838, gh:#1839).
#[test]
fn turbulence_true_is_refused_by_name() {
    let (root, poly) = stage("refuse");
    let p = root.join("constant/fluidRegion/turbulenceProperties");
    let text = std::fs::read_to_string(&p).expect("turbulenceProperties");
    std::fs::write(&p, text.replace("turbulence      false;", "turbulence      true;")).expect("write");
    let err = OnePhaseCcSolver::from_case_dir(&root, &poly).expect_err("must refuse").to_string();
    assert!(err.contains("turbulence true is not ported"), "{err}");
}
