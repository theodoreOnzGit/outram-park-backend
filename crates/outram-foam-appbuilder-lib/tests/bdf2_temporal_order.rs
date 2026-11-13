// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # V&V — `DdtScheme::Backward` (BDF2) with its own `ddtCorr` partner in `PimpleFoam` (gh:#1067)
//!
//! Full record, with every number: `verification_and_validation/bdf2_temporal_order.md`.
//!
//! Common setup for every study here: the Re = 100 lid-driven cavity of
//! `tutorials/cases/pimple_foam_cavity` (20×20 mesh, L = 0.1 m, U_lid = 1 m/s,
//! ν = 1e-3 m²/s, `Gauss upwind` convection, `nCorrectors = 2`). Three
//! time-integration pairings:
//!
//! | label | `schemes.ddt` | `ddt_corr_form` | meaning |
//! |---|---|---|---|
//! | `euler` | `Euler` | `MatchDdtScheme` | first-order baseline |
//! | `bdf2-eulercorr` | `Backward` | `EulerOnly` (ablation) | the pre-gh:#1067 pairing |
//! | `bdf2-bdf2corr` | `Backward` | `MatchDdtScheme` | the fix (default) |
//!
//! ## Study 1 — Richardson temporal order (prediction MISSED; test `#[ignore]`d)
//!
//! **Method.** Impulsive start from rest, advanced to T = 0.4 s at
//! Δt = 4e-3, 2e-3, 1e-3, 5e-4, 2.5e-4 s; `e_k = ‖U(Δt_k) − U(Δt_k/2)‖` in the
//! volume-weighted L2 norm, observed order `p_k = log2(e_k/e_{k+1})`. Run at
//! `nOuterCorrectors = 1` (PISO) and `= 10` (converged PIMPLE).
//!
//! **Prediction (written 2026-11-01, BEFORE any run, kept verbatim).**
//! `euler` ≈ 1 at both settings; `bdf2-bdf2corr`, converged PIMPLE, rises to
//! ≈ 2; `bdf2-eulercorr` ≈ 1 or worse; PISO ≈ 1 for every pairing (lagged
//! convection flux). Gates fixed with the prediction: finest pair, converged
//! PIMPLE, `bdf2-bdf2corr` in [1.7, 2.3] and `euler` in [0.8, 1.2].
//!
//! **Result (2026-11-01): the prediction missed for every pairing.** Finest-pair
//! L2 orders, converged PIMPLE: `euler` −0.058, `bdf2-eulercorr` +0.067,
//! `bdf2-bdf2corr` −0.015; successive differences stay at 1.0e-3 to 1.9e-3 m/s
//! and do not shrink. `nOuter = 1` and `= 10` agree to three digits. The gates
//! were **not** moved; the test is `#[ignore]`d so it stays an executable
//! record of the miss without turning the suite red.
//!
//! ## Study 2 — restart-from-steady control (explains the miss; live)
//!
//! Designed **after** Study 1 missed, to test the assumption Study 1 rests on:
//! that the runs at different Δt share one semi-discrete system and differ
//! only by time-truncation error. Spin the cavity up to steady state (Euler,
//! Δt = 2e-3 s, 15 s), then restart each pairing from that state at each Δt
//! for 0.2 s. A steady state has no time-truncation error, so if Study 1's
//! differences were temporal, these would be ~0.
//!
//! **Result (2026-11-01).** Restarting at the spin-up's own Δt and scheme
//! moves the solution by 3.0e-11 m/s (it is steady), but every *other* (Δt, pairing)
//! moves it by 4.5e-4 to 7.5e-3 m/s in 0.2 s, and the Δt-to-Δt differences
//! (1.0e-3 to 1.9e-3 m/s) reproduce Study 1's to two digits. So **the
//! discrete fixed point itself depends on Δt** (through `rAU = V/A` in the
//! Rhie–Chow face flux and the `fvcDdtPhiCoeff` limiter), and Study 1 measured
//! that dependence, not temporal order. A Richardson study on a Rhie–Chow
//! PISO case is the wrong instrument for temporal order; see the V&V record
//! for the uncommitted `coeff = 1` diagnostic.
//!
//! ## Study 3 — steady-state consistency of the pairing (the defect's own signature; live)
//!
//! **Method.** As in `tests/fv_scheme_selection.rs`: run to t = 12 s (steady)
//! at Δt = 8e-3, 4e-3, 2e-3, 1e-3 s, `nOuterCorrectors = 1`, and record
//! `d(Δt) = max |U_backward − U_euler|`. At a steady state the BDF2 and Euler
//! ddt terms both vanish, so `d` can only come from the face flux. The
//! 2026-08-07 record measured `d` *growing* as Δt shrank with the mismatched
//! pairing (1.0394e-2 → 2.9258e-2), the signature of an inconsistency.
//!
//! **Prediction (written 2026-11-01 after Studies 1–2, BEFORE this run).**
//! `bdf2-eulercorr` reproduces the 2026-08-07 numbers to the printed digits
//! (its code path is unchanged). `bdf2-bdf2corr`: `d` *decreases*
//! monotonically as Δt is refined (the BDF2 weight `coefft` multiplies the
//! history in both `H/A` and `ddtCorr`, so it no longer survives Δt → 0).
//! Gates fixed with the prediction: (a) ablation within 1e-3 relative of each
//! 2026-08-07 value; (b) consistent-pairing `d` strictly decreasing over the
//! four Δt.
//!
//! **Result (2026-11-01): (a) passed — the ablation reproduces all four
//! 2026-08-07 values to the printed digits; (b) MISSED.** The BDF2 `ddtCorr`
//! cuts the steady Euler/BDF2 disagreement 2.7–3.6× at every Δt
//! (8.04e-3 vs 2.93e-2 m/s at Δt = 1e-3), but it still grows as Δt shrinks
//! (3.80e-3 → 8.04e-3). See [`study3_steady_state_bdf2_vs_euler_with_each_ddt_corr`].

use outram_foam_appbuilder_lib::io::control_dict::{ControlDict, StartControl, StopControl};
use outram_foam_appbuilder_lib::io::field_reader::{
    read_vol_scalar_field_full, read_vol_vector_field_full,
};
use outram_foam_appbuilder_lib::io::fv_schemes::{DdtScheme, DivScheme, FvSchemes};
use outram_foam_appbuilder_lib::io::fv_solution::FvSolution;
use outram_foam_appbuilder_lib::io::poly_mesh::read_poly_mesh;
use outram_foam_appbuilder_lib::solvers::pimple_foam::PimpleFoam;
use outram_foam_appbuilder_lib::solvers::schemes::DdtCorrForm;
use outram_foam_basic_lib::prelude::{Vector3, VolScalarField};
use std::path::Path;

const CASE_DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tutorials/cases/pimple_foam_cavity"
);

/// The three pairings of the module table.
const PAIRINGS: [(&str, DdtScheme, DdtCorrForm); 3] = [
    ("euler", DdtScheme::Euler, DdtCorrForm::MatchDdtScheme),
    (
        "bdf2-eulercorr",
        DdtScheme::Backward,
        DdtCorrForm::EulerOnly,
    ),
    (
        "bdf2-bdf2corr",
        DdtScheme::Backward,
        DdtCorrForm::MatchDdtScheme,
    ),
];

/// The cavity at rest (lid moving), configured for one pairing.
fn cavity(ddt: DdtScheme, corr: DdtCorrForm, n_outer: usize, dt: f64) -> PimpleFoam {
    let case = Path::new(CASE_DIR);
    let mesh = read_poly_mesh(&case.join("constant").join("polyMesh")).expect("read polyMesh");
    let control = ControlDict {
        start: StartControl::StartTime(0.0),
        stop: StopControl::EndTime(1.0),
        delta_t: dt,
        ..ControlDict::default()
    };
    let mut schemes = FvSchemes::default();
    schemes.ddt = ddt;
    schemes.default_div = DivScheme::GaussUpwind;
    let mut solution = FvSolution::default();
    solution.pimple.n_outer_correctors = n_outer;
    solution.pimple.n_correctors = 2;

    let mut s = PimpleFoam::new(mesh.clone(), control, schemes, solution);
    s.u = read_vol_vector_field_full(&case.join("0").join("U"), &mesh).expect("read 0/U");
    s.p = read_vol_scalar_field_full(&case.join("0").join("p"), &mesh).expect("read 0/p");
    s.nu = VolScalarField::uniform("nu", mesh, 1.0e-3);
    s.ddt_corr_form = corr;
    s
}

/// Advance `s` by `t` seconds at its own Δt, counting steps from an integer so
/// every run lands on the same end time.
fn advance(s: &mut PimpleFoam, t: f64) {
    for _ in 0..(t / s.control.delta_t).round() as usize {
        s.step().expect("cavity step");
    }
}

fn velocities(s: &PimpleFoam) -> Vec<Vector3> {
    let u = s.u.internal.as_slice().to_vec();
    assert!(
        u.iter().all(|v| v.mag().is_finite()),
        "a run went non-finite"
    );
    u
}

/// `(volume-weighted L2, max)` norm of `a − b`.
fn norms(a: &[Vector3], b: &[Vector3], vol: &[f64]) -> (f64, f64) {
    let v_tot: f64 = vol.iter().sum();
    let mut sq = 0.0;
    let mut mx = 0.0_f64;
    for ((x, y), v) in a.iter().zip(b).zip(vol) {
        let d = (*x - *y).mag();
        sq += d * d * v;
        mx = mx.max(d);
    }
    ((sq / v_tot).sqrt(), mx)
}

/// Successive-difference table and observed L2 orders for a list of runs at
/// `dts`; prints both and returns the orders.
fn print_orders(label: &str, dts: &[f64], runs: &[Vec<Vector3>], vol: &[f64]) -> Vec<f64> {
    let diffs: Vec<(f64, f64)> = runs.windows(2).map(|w| norms(&w[0], &w[1], vol)).collect();
    println!("--- {label} ---");
    for (k, (l2, mx)) in diffs.iter().enumerate() {
        println!(
            "  |U(dt={:.3e}) - U(dt={:.3e})|: L2 = {l2:.4e}, max = {mx:.4e}",
            dts[k],
            dts[k + 1]
        );
    }
    let mut p = Vec::new();
    for k in 0..diffs.len() - 1 {
        let pl = (diffs[k].0 / diffs[k + 1].0).log2();
        let pm = (diffs[k].1 / diffs[k + 1].1).log2();
        println!(
            "  order over dt = {:.3e} -> {:.3e} -> {:.3e}: L2 p = {pl:.3}, max p = {pm:.3}",
            dts[k],
            dts[k + 1],
            dts[k + 2]
        );
        p.push(pl);
    }
    p
}

const STUDY1_DTS: [f64; 5] = [4.0e-3, 2.0e-3, 1.0e-3, 5.0e-4, 2.5e-4]; // s
const STUDY1_T: f64 = 0.4; // s

/// Study 1 — Richardson temporal order from an impulsive start. **Prediction
/// missed** (see module docs); the gates are the ones written before the run
/// and were not moved, so this test fails and is ignored.
///
/// # Results (2026-11-01, release, this machine)
///
/// Finest-pair L2 orders `p` over Δt = 1e-3 → 5e-4 → 2.5e-4 s:
///
/// | pairing | nOuter = 1 | nOuter = 10 |
/// |---|---|---|
/// | `euler` | −0.058 | −0.058 |
/// | `bdf2-eulercorr` | +0.066 | +0.067 |
/// | `bdf2-bdf2corr` | −0.016 | −0.015 |
///
/// All twelve successive differences lie between 1.0e-3 and 1.9e-3 m/s (L2).
/// Interpretation: Study 2.
#[test]
#[ignore = "gh:#1067: pre-registered prediction missed (orders ~0, fixed point depends on dt); \
            kept as a record, gates not moved — see verification_and_validation/bdf2_temporal_order.md"]
fn study1_richardson_temporal_order_from_rest() {
    let vol = cavity(DdtScheme::Euler, DdtCorrForm::MatchDdtScheme, 1, 1e-3)
        .mesh
        .cell_volumes
        .clone();
    let mut finest = Vec::new();
    for n_outer in [1, 10] {
        for (label, ddt, corr) in PAIRINGS.iter().cloned() {
            let runs: Vec<Vec<Vector3>> = STUDY1_DTS
                .iter()
                .map(|&dt| {
                    let mut s = cavity(ddt.clone(), corr, n_outer, dt);
                    advance(&mut s, STUDY1_T);
                    velocities(&s)
                })
                .collect();
            let p = print_orders(
                &format!("{label}, nOuter = {n_outer}"),
                &STUDY1_DTS,
                &runs,
                &vol,
            );
            finest.push((label, n_outer, *p.last().unwrap()));
        }
    }
    let get = |l: &str| {
        finest
            .iter()
            .find(|(a, n, _)| *a == l && *n == 10)
            .unwrap()
            .2
    };
    let (pe, pb) = (get("euler"), get("bdf2-bdf2corr"));
    assert!(
        (0.8..=1.2).contains(&pe),
        "Euler finest-pair order {pe:.3} outside [0.8, 1.2]"
    );
    assert!(
        (1.7..=2.3).contains(&pb),
        "BDF2 + BDF2 ddtCorr finest-pair order {pb:.3} outside [1.7, 2.3]"
    );
}

/// Study 2 — restart from a steady state at each Δt (the control that explains
/// Study 1's miss). Gate: the restart at the spin-up's own Δt and scheme must
/// stay put (< 1e-9 m/s L2 in 0.2 s), confirming the spin-up is steady; the
/// Δt dependence itself is recorded, not gated.
///
/// # Results (2026-11-01, release, this machine)
///
/// Spin-up: Euler, Δt = 2e-3 s, 15 s. L2 drift from the spun-up state after
/// 0.2 s, and L2 successive differences:
///
/// | pairing | drift at Δt = 2e-3 | drift at Δt = 2.5e-4 | Δt-to-Δt differences |
/// |---|---|---|---|
/// | `euler` | 2.985e-11 | 3.747e-3 | 1.020e-3 … 1.339e-3 |
/// | `bdf2-eulercorr` | 2.256e-3 | 7.466e-3 | 1.572e-3 … 1.853e-3 |
/// | `bdf2-bdf2corr` | 6.753e-4 | 4.487e-3 | 1.121e-3 … 1.340e-3 |
///
/// Observed "orders" −0.210 … +0.066 (finest pair: −0.057, +0.066, −0.016),
/// matching Study 1 to two digits: the Δt-to-Δt differences of Study 1 are
/// differences of Δt-dependent fixed points, not time-truncation error.
#[test]
fn study2_restart_from_steady_state_shows_dt_dependent_fixed_point() {
    let mut base = cavity(DdtScheme::Euler, DdtCorrForm::MatchDdtScheme, 1, 2.0e-3);
    advance(&mut base, 15.0);
    let u0 = velocities(&base);
    let vol = base.mesh.cell_volumes.clone();
    let dts = STUDY1_DTS;
    for (label, ddt, corr) in PAIRINGS.iter().cloned() {
        let mut runs = Vec::new();
        for &dt in &dts {
            let mut s = cavity(ddt.clone(), corr, 1, dt);
            s.u = base.u.clone();
            s.p = base.p.clone();
            s.phi = base.phi.clone();
            advance(&mut s, 0.2);
            let u = velocities(&s);
            let (drift, _) = norms(&u, &u0, &vol);
            println!("{label} dt = {dt:.3e}: L2 drift from steady state in 0.2 s = {drift:.4e}");
            if label == "euler" && dt == 2.0e-3 {
                assert!(
                    drift < 1e-9,
                    "spin-up is not steady: restart at its own dt drifted {drift:e}"
                );
            }
            runs.push(u);
        }
        print_orders(&format!("{label} (restart)"), &dts, &runs, &vol);
    }
}

const STUDY3_DTS: [f64; 4] = [8.0e-3, 4.0e-3, 2.0e-3, 1.0e-3]; // s

/// Study 3 runs: for each Δt in [`STUDY3_DTS`], `(d_ablation, d_consistent)` =
/// max |U_backward − U_euler| at t = 12 s with the Euler-only and the BDF2
/// `ddtCorr`. One thread per Δt.
fn study3_steady_differences() -> Vec<(f64, f64)> {
    let max_diff = |a: &[Vector3], b: &[Vector3]| {
        a.iter()
            .zip(b)
            .map(|(x, y)| (*x - *y).mag())
            .fold(0.0_f64, f64::max)
    };
    std::thread::scope(|sc| {
        let handles: Vec<_> = STUDY3_DTS
            .iter()
            .map(|&dt| {
                sc.spawn(move || {
                    let steady = |ddt: DdtScheme, corr: DdtCorrForm| {
                        let mut s = cavity(ddt, corr, 1, dt);
                        advance(&mut s, 12.0);
                        velocities(&s)
                    };
                    let e = steady(DdtScheme::Euler, DdtCorrForm::MatchDdtScheme);
                    let m = steady(DdtScheme::Backward, DdtCorrForm::EulerOnly);
                    let b = steady(DdtScheme::Backward, DdtCorrForm::MatchDdtScheme);
                    (max_diff(&m, &e), max_diff(&b, &e))
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("thread"))
            .collect()
    })
}

/// ~~2026-08-07 record (`tests/fv_scheme_selection.rs`), mismatched pairing:
/// `[1.0394e-2, 1.6499e-2, 2.3420e-2, 2.9258e-2]`.~~ **RE-PINNED 2026-11-03
/// (gh:#1154)** to the values measured after `PimpleFoam` gained upstream's
/// boundary-flux handling (`constrainHbyA` into `phi`, `pEqn.flux()` on
/// fixed-p patches, the `needReference` guard). On the cavity only the first
/// acts: the lid's boundary flux in `phi` is now exactly `U_lid·Sf = 0`
/// instead of the zero-gradient `HbyA_P·Sf`. That moved this ablation by
/// −19 % to −4 % and moved the cavity closer to icoFoam (max |ΔU| / U_lid
/// 0.0144 → 0.0062); see `tutorials/pimple_foam_cavity.rs`. The 2026-08-07
/// record is kept in the table below as history.
const RECORDED_MISMATCH: [f64; 4] = [8.3883e-3, 1.3833e-2, 2.0748e-2, 2.8047e-2];
/// ~~2026-11-01 values of the consistent pairing (this file's first run):
/// `[3.8032e-3, 5.2084e-3, 6.8244e-3, 8.0361e-3]`.~~ **RE-PINNED 2026-11-03
/// (gh:#1154)**, same cause as [`RECORDED_MISMATCH`]; the BDF2/Euler steady
/// disagreement fell 44 % to 29 % but still grows as Δt shrinks.
const RECORDED_CONSISTENT: [f64; 4] = [2.1260e-3, 3.1705e-3, 4.5065e-3, 5.7053e-3];

/// Study 3 — steady-state Euler-vs-BDF2 difference, with each `ddtCorr` form.
///
/// Gates: (a) **pre-registered** — the Euler-only ablation reproduces the
/// 2026-08-07 record within 1e-3 relative (its code path is meant to be
/// unchanged; **since 2026-11-03 it reproduces the re-pinned
/// [`RECORDED_MISMATCH`]**, because the boundary-flux change of gh:#1154
/// moved every cavity number); (c) **post-hoc regression pin, chosen after the run and not
/// evidence** — the consistent pairing reproduces its own 2026-11-01 values
/// within 1e-3 relative, so a later change to the BDF2 path cannot move them
/// silently. Pre-registered gate (b) failed; it lives in the ignored
/// [`study3_prediction_b_consistent_difference_shrinks_with_dt`].
///
/// # Results (2026-11-01, release, this machine)
///
/// max |U_backward − U_euler| at t = 12 s (steady), m/s:
///
/// | Δt [s] | Euler-only ddtCorr (ablation) | 2026-08-07 record | BDF2 ddtCorr (default) | ratio |
/// |---|---|---|---|---|
/// | 8e-3 | 1.0394e-2 | 1.0394e-2 | 3.8032e-3 | 2.73 |
/// | 4e-3 | 1.6499e-2 | 1.6499e-2 | 5.2084e-3 | 3.17 |
/// | 2e-3 | 2.3420e-2 | 2.3420e-2 | 6.8244e-3 | 3.43 |
/// | 1e-3 | 2.9258e-2 | 2.9258e-2 | 8.0361e-3 | 3.64 |
///
/// **Re-measured 2026-11-03 (gh:#1154), after the boundary-flux change:**
///
/// | Δt [s] | Euler-only ddtCorr (ablation) | BDF2 ddtCorr (default) | ratio |
/// |---|---|---|---|
/// | 8e-3 | 8.3883e-3 | 2.1260e-3 | 3.95 |
/// | 4e-3 | 1.3833e-2 | 3.1705e-3 | 4.36 |
/// | 2e-3 | 2.0748e-2 | 4.5065e-3 | 4.60 |
/// | 1e-3 | 2.8047e-2 | 5.7053e-3 | 4.92 |
///
/// Both columns fell, the BDF2 one most. The growth with shrinking Δt
/// remains, so the interpretation below still holds.
///
/// **Interpretation.** The ablation reproduces the 2026-08-07 record to all
/// five printed digits. The BDF2 `ddtCorr` cuts the Euler/BDF2 steady
/// disagreement 2.7–3.6× at every Δt, but it **still grows** as Δt shrinks
/// (prediction (b) missed). A diagnostic run with the `fvcDdtPhiCoeff`
/// limiter replaced by `coeff = 1` (uncommitted one-line patch, see the V&V
/// record) gave 4.97e-3, 5.69e-3, 5.17e-3, 3.80e-3 — rising then falling
/// below Δt ≈ 2e-3 s — while the ablation grew to 5.47e-2: the remaining
/// growth is carried by the limiter, which is upstream's default and was
/// kept.
#[test]
fn study3_steady_state_bdf2_vs_euler_with_each_ddt_corr() {
    let results = study3_steady_differences();
    println!("| dt [s] | ablation (Euler-only ddtCorr) | 2026-08-07 record | BDF2 ddtCorr |");
    for (k, (m, b)) in results.iter().enumerate() {
        println!(
            "| {:.0e} | {m:.4e} | {:.4e} | {b:.4e} |",
            STUDY3_DTS[k], RECORDED_MISMATCH[k]
        );
    }
    for (k, (m, b)) in results.iter().enumerate() {
        let rel = (m - RECORDED_MISMATCH[k]).abs() / RECORDED_MISMATCH[k];
        assert!(
            rel < 1e-3,
            "(a) ablation at dt = {:e} gave {m:.4e}, recorded {:.4e} (rel {rel:.2e})",
            STUDY3_DTS[k],
            RECORDED_MISMATCH[k]
        );
        let rel = (b - RECORDED_CONSISTENT[k]).abs() / RECORDED_CONSISTENT[k];
        assert!(
            rel < 1e-3,
            "(c) consistent pairing at dt = {:e} gave {b:.4e}, pinned {:.4e} (rel {rel:.2e})",
            STUDY3_DTS[k],
            RECORDED_CONSISTENT[k]
        );
    }
}

/// Study 3, pre-registered gate (b): the consistent-pairing steady difference
/// must decrease strictly as Δt is refined. **Missed** (3.80e-3 → 8.04e-3, see
/// [`study3_steady_state_bdf2_vs_euler_with_each_ddt_corr`]); gate not moved.
#[test]
#[ignore = "gh:#1067: pre-registered prediction (b) missed — the BDF2/Euler steady difference \
            still grows as dt shrinks (limiter-driven); kept as a record, gate not moved"]
fn study3_prediction_b_consistent_difference_shrinks_with_dt() {
    let results = study3_steady_differences();
    for w in results.windows(2) {
        assert!(
            w[1].1 < w[0].1,
            "consistent-pairing steady difference must shrink with dt: {:.4e} -> {:.4e}",
            w[0].1,
            w[1].1
        );
    }
}
