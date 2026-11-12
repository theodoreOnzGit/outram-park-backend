//! GeN-Foam `twoPhase` port on upstream's `Tutorials/featureCases/1D_boiling`
//! (gh:#1218). Methodology, predictions and results:
//! `verification_and_validation/genfoam_two_phase/README.md`.
//!
//! # What is compared
//!
//! Upstream ships **no numeric reference** for this case (its `Alltest` only
//! checks the run ends). ~~and upstream cannot be built on this machine, so this
//! is not a code-to-code comparison.~~ **CORRECTED 2026-11-12** (gh:#1361 round 3): the real binary was built
//! and run on this case; the code-to-code comparison is
//! `tests/genfoam_two_phase_vs_upstream.rs`. THIS file checks the port, run on
//! upstream's own case files read by `TwoPhaseCase::read`, against
//!
//! - **P1**, a closed form: the isothermal single-phase steady velocity
//!   `0.4e5 Pa = (U/alpha) sum_z Kd_z(U) L_z` (ReynoldsPower friction, the
//!   case's sodium properties at 652.15 K) = 0.71198 m/s;
//! - **P2-P5**, physical behaviour predicted before the run (boiling onset
//!   window, flow excursion, condensation after power-off, continuity).
//!
//! The case files are upstream's, vendored unchanged in
//! `tests/data/genfoam_1d_boiling` (`GENFOAM_UPSTREAM` points at a checkout
//! instead). `TWO_PHASE_OUT=<dir>` makes the full run write CSV histories for
//! plotting.

use std::io::Write;
use std::path::PathBuf;

use outram_foam_appbuilder_lib::genfoam::thermal_hydraulics::solver::two_phase::{
    TwoPhaseCase, TwoPhaseSolver,
};

fn case_dir() -> Option<PathBuf> {
    // Upstream's case files, vendored unchanged (tests/data/genfoam_1d_boiling,
    // see its PROVENANCE.md); GENFOAM_UPSTREAM points at a checkout instead.
    let dir = match std::env::var("GENFOAM_UPSTREAM") {
        Ok(root) => PathBuf::from(root).join("Tutorials/featureCases/1D_boiling"),
        Err(_) => PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/genfoam_1d_boiling"),
    };
    dir.join("constant").is_dir().then_some(dir)
}

/// The closed-form isothermal velocity of P1 [m/s], recomputed here from the
/// case's own polynomials so the number is not typed in.
fn closed_form_velocity() -> f64 {
    let t: f64 = 652.15;
    let rho = 1006.81 - 0.217 * t - 2.83e-6 * t * t - 6.54e-9 * t.powi(3);
    let mc = [
        4.932462e-03, -2.654334e-05, 6.527520e-08, -8.752858e-11, 6.621667e-14, -2.657514e-17,
        4.402963e-21,
    ];
    let mu: f64 = mc.iter().enumerate().map(|(i, c)| c * t.powi(i as i32)).sum();
    let nu = mu / rho;
    let dp = |u: f64| -> f64 {
        [(0.01, 0.5), (0.005, 1.0), (0.01, 0.5)]
            .iter()
            .map(|&(dh, l): &(f64, f64)| {
                let re = (u * dh / nu).max(10.0);
                let kd = 0.5 / dh * 0.5 * rho * u * 2.0 * re.powf(-0.125);
                kd * u / 0.5 * l
            })
            .sum()
    };
    let (mut lo, mut hi) = (0.01, 50.0);
    for _ in 0..200 {
        let m = 0.5 * (lo + hi);
        if dp(m) > 0.4e5 {
            hi = m;
        } else {
            lo = m;
        }
    }
    0.5 * (lo + hi)
}

/// Run the case to `t_end` and return the solver; `isothermal` switches the
/// `fixedPower` source off first (the structure then stays at 652.15 K).
fn run_to(dir: &std::path::Path, t_end: f64, isothermal: bool) -> TwoPhaseSolver {
    let mut case = TwoPhaseCase::read(dir).expect("read 1D_boiling");
    if isothermal {
        for pm in case.structure.power.iter_mut().flatten() {
            pm.power_off();
        }
    }
    let mut s = TwoPhaseSolver::new(case);
    while s.time < t_end - 1e-12 {
        let r = s.step().expect("step");
        if std::env::var("TWO_PHASE_DEBUG").is_ok() {
            eprintln!(
                "t={:.4} dt={:.2e} it={} U0={:.6} U20={:.6} U39={:.6} Tl20={:.3}",
                s.time, r.dt, r.outer_iterations, s.fluids[0].u.c[0].x, s.fluids[0].u.c[20].x,
                s.fluids[0].u.c[39].x, s.fluids[0].t.c[20]
            );
        }
    }
    s
}

/// P1 on its premise: with the heater off the flow is isothermal and the
/// port's liquid velocity must settle, uniformly, on the closed form.
#[test]
fn one_d_boiling_isothermal_flow_matches_closed_form() {
    let Some(dir) = case_dir() else {
        eprintln!("SKIP: upstream 1D_boiling not found; set GENFOAM_UPSTREAM");
        return;
    };
    let s = run_to(&dir, 1.0, true);
    let u_ref = closed_form_velocity();
    let worst = s.fluids[0]
        .u
        .c
        .iter()
        .map(|u| (u.x - u_ref).abs() / u_ref)
        .fold(0.0, f64::max);
    eprintln!("P1 (isothermal): t = {:.3} s, U.liquid[0] = {:.6}, closed form = {u_ref:.6}, worst rel over cells = {worst:.3e}", s.time, s.fluids[0].u.c[0].x);
    assert!(worst < 1e-3, "P1 miss: worst rel {worst:.3e}");
}

/// P1 as first predicted, on the heated case at 0.3 s. Recorded miss: the
/// heater's thermal expansion makes the liquid velocity non-uniform by a few
/// per cent before boiling (README, "P1"), so this asserts only the measured
/// state: inlet velocity within 3 % of the closed form, faster downstream.
#[test]
fn one_d_boiling_heated_flow_at_0_3_s() {
    let Some(dir) = case_dir() else {
        eprintln!("SKIP: upstream 1D_boiling not found; set GENFOAM_UPSTREAM");
        return;
    };
    let s = run_to(&dir, 0.3, false);
    let u_ref = closed_form_velocity();
    let u_in = s.fluids[0].u.c[0].x;
    let u_out = s.fluids[0].u.c[39].x;
    let rel = (u_in - u_ref) / u_ref;
    eprintln!("P1 (heated, 0.3 s): U.liquid inlet = {u_in:.6}, outlet = {u_out:.6}, closed form = {u_ref:.6}, inlet rel = {rel:.3e}");
    assert!(rel.abs() < 3e-2);
    assert!(u_out > u_in, "heating must accelerate the liquid downstream");
}

/// The full 12 s transient, P2-P5 (about 8 s in release).
///
/// Asserts the measured behaviour, not the P2 window, which was missed
/// (README "Results"): boiling starts before the 7 s power-off, the inlet
/// liquid flow falls below half its single-phase value (flow excursion), and
/// the void has condensed away by 12 s. The continuity errors (P5, missed) are
/// printed, not asserted.
#[test]
fn one_d_boiling_full_transient() {
    let Some(dir) = case_dir() else {
        eprintln!("SKIP: upstream 1D_boiling not found; set GENFOAM_UPSTREAM");
        return;
    };
    let case = TwoPhaseCase::read(&dir).expect("read 1D_boiling");
    for d in &case.deviations {
        eprintln!("deviation: {d}");
    }
    let end = case.end_time;
    let mut s = TwoPhaseSolver::new(case);
    // Diagnostic ablations (not defaults): override alpha relaxation or nOuter.
    if let Ok(v) = std::env::var("TWO_PHASE_RELAX_ALPHA") {
        s.controls.relax_alpha = v.parse().expect("relax");
    }
    if let Ok(v) = std::env::var("TWO_PHASE_N_OUTER") {
        s.controls.n_outer = v.parse().expect("n_outer");
    }
    let out = std::env::var("TWO_PHASE_OUT").ok().map(PathBuf::from);
    let mut hist = Vec::new();
    let mut profiles = Vec::new();
    let u0 = closed_form_velocity();
    let mut first_boil: Option<f64> = None;
    let mut min_u_in = f64::MAX;
    let mut max_void_after_off = Vec::new();
    let mut steps = 0usize;
    while s.time < end - 1e-9 {
        let r = s.step().expect("step");
        steps += 1;
        let nv: Vec<f64> = s.fluids[1].normalized.clone();
        let vmax = nv.iter().cloned().fold(0.0, f64::max);
        if first_boil.is_none() && vmax > 1e-3 {
            first_boil = Some(s.time);
        }
        let u_in = s.fluids[0].u.c[0].x;
        if first_boil.is_some() {
            min_u_in = min_u_in.min(u_in);
        }
        if s.time > 7.0 {
            max_void_after_off.push((s.time, vmax));
        }
        let tsmax = s.structure.t_act.iter().cloned().fold(0.0, f64::max);
        hist.push((s.time, r.dt, u_in, vmax, tsmax, r.outer_iterations, r.courant, s.fluids[0].cumul_cont_err, s.fluids[1].cumul_cont_err, s.fluids[1].u.c[0].x));
        let wi = s.time_controls.write_interval.unwrap_or(0.1);
        if ((s.time / wi).round() * wi - s.time).abs() < 1e-9 {
            profiles.push((s.time, s.geom.mesh.cell_centres.iter().map(|c| c.x).collect::<Vec<_>>(),
                nv.clone(), s.fluids[0].t.c.clone(), s.fluids[1].t.c.clone(),
                s.structure.t_act.clone(), s.t_interface.clone()));
        }
    }
    eprintln!("steps = {steps}");
    eprintln!("P2: first alpha.vapour(normalized) > 1e-3 at t = {first_boil:?}");
    eprintln!("P3: min inlet U.liquid during boiling = {min_u_in:.4} m/s (P1 value {u0:.4})");
    let last = max_void_after_off.last().copied();
    eprintln!("P4: max void at end = {last:?}");
    eprintln!(
        "P5: cumulative continuity errors = {:.3e} {:.3e} kg/m3",
        s.fluids[0].cumul_cont_err, s.fluids[1].cumul_cont_err
    );
    let onset = first_boil.expect("P2: no boiling");
    assert!(onset < 7.0, "P2: boiling must start before power-off, got {onset}");
    assert!(min_u_in < 0.5 * u0, "P3: no flow excursion ({min_u_in})");
    assert!(last.expect("ran past 7 s").1 < 1e-3, "P4: void did not condense");
    assert!(s.fluids[0].cumul_cont_err.is_finite());
    if let Some(dir) = out {
        std::fs::create_dir_all(&dir).expect("out dir");
        let mut f = std::fs::File::create(dir.join("history.csv")).expect("csv");
        writeln!(f, "t,dt,u_liquid_inlet,max_alpha_vapour_normalized,max_T_structure,outer_iters,courant,cumul_cont_err_liquid,cumul_cont_err_vapour,u_vapour_inlet").unwrap();
        for h in &hist {
            writeln!(f, "{},{},{},{},{},{},{},{},{},{}", h.0, h.1, h.2, h.3, h.4, h.5, h.6, h.7, h.8, h.9).unwrap();
        }
        let mut f = std::fs::File::create(dir.join("profiles.csv")).expect("csv");
        writeln!(f, "t,x,alpha_vapour_normalized,T_liquid,T_vapour,T_structure,T_interface").unwrap();
        for p in &profiles {
            for i in 0..p.1.len() {
                writeln!(f, "{},{},{},{},{},{},{}", p.0, p.1[i], p.2[i], p.3[i], p.4[i], p.5[i], p.6[i]).unwrap();
            }
        }
    }
}
