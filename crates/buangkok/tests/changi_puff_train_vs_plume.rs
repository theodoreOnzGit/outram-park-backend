// SPDX-License-Identifier: GPL-3.0
//
// Independent check of changi::activity's puff-train dilution factor against
// (a) a quadrature of the same puff kernel and (b) buangkok's pyDOSEIA
// Gaussian plume, in the steady limit (gh:#380).

//! # V&V (gh:#380): `changi::activity::chi_over_q` puff train vs a Gaussian plume
//!
//! `changi::activity` is not a port, so until now its strongest evidence was
//! internal consistency (`changi/tests/activity_properties.rs`). This file
//! gives it two **independent** checks.
//!
//! **Scope limit, binding:** a verification of dispersion arithmetic. It is not
//! a dispersion assessment for any site (`RESPONSIBLE_USE.md`).
//!
//! ## What is being checked, and why it can be checked exactly
//!
//! `dilution_factors` normalises a release segment's time-integrated
//! concentration by the mass that segment emitted. Under a **constant wind** and
//! a **fixed stability class**, every puff follows the same history — it sits at
//! `u a` with travel distance `u a` at age `a` — so the per-unit-mass exposure
//! of every puff is the same, and the segment's `chi/Q` is exactly
//!
//! ```text
//! chi/Q = sum_{k=0}^{A/dt} K(u k dt) exp(-lambda k dt) dt
//! ```
//!
//! with `K` the unit-mass Gaussian puff kernel at the receptor and `A` the
//! puff lifetime (`RunConfig::puff_duration`). Two consequences:
//!
//! - It can be written out **without** the puff-train code, as a plain sum
//!   over ages of the closed-form kernel (Gate 1), and it converges to
//!   `integral_0^A K(u a) exp(-lambda a) da` as `dt -> 0` (reported).
//! - It does **not** depend on `puff_dt`. The issue's condition "puff spacing
//!   small compared with sigma" is needed for the *instantaneous*
//!   concentration to look like a plume, not for the *time-integrated* one,
//!   which is what `chi/Q` is. Checked directly (the third test).
//!
//! That continuum integral is **not** the Gaussian plume: a puff at `x_p`
//! carries `sigma(x_p)`, not `sigma(x_r)`. A Laplace expansion of
//! `integral N(s; sigma_y(x_r - s)) P(x_r - s) ds` (the kernel factors as a
//! crosswind Gaussian `N` times the plume `P` evaluated at the puff) gives,
//! to second order in `sigma_y / x`,
//!
//! ```text
//! (continuum puff train) / (plume at x_r)  =  1 + c2,
//! c2 = (sigma_y^2 P)'' / (2 P)   evaluated at x_r
//! ```
//!
//! So the plume comparison has a **predicted, derived** residual (Gate 2), not
//! a picked tolerance.
//!
//! ## Methodology
//!
//! - **Model under test:** `changi::activity::chi_over_q::dilution_factors`,
//!   `StabilitySource::Fixed`, default `EmissionPolicy::OnePuffPerEmission` and
//!   `AdvectionPolicy::LagrangianTrajectory`.
//! - **Independent reference 1:** the puff kernel written out here from its
//!   closed form (`changi/src/puff/concentration.rs` doc; checked against
//!   upstream R there), summed over ages, and integrated by `petir`'s GSL QAGS
//!   port. Both use `changi`'s `pasquill_gifford_sigmas` (code-to-code
//!   verified against upstream R, 10 182 cases).
//! - **Independent reference 2:** `buangkok::pydoseia::dispersion::
//!   master_equation_single_plume` (Hukkoo-Bapat eq. 2.5, code-to-code verified
//!   against pyDOSEIA), fed **changi's** sigmas at `x_r`, so both sides use one
//!   sigma parameterisation, as #380 asks. The two codes' own sigma fits differ
//!   (ISC/Martin in changi, BARC/AERB in pyDOSEIA); that difference is
//!   reported separately in the fourth test and plays no part in the gates.
//! - **Inputs (fixed before running):** wind 4 m/s along +x; release height
//!   H = 40 m; receptors on the ground-level centreline (y = z = 0) at 0.12,
//!   0.35, 0.8, 1.5, 2.5, 5.0, 8.5 km, chosen to sit >= 12 % away from every
//!   sigma_z fit breakpoint of every class; classes A-F; `sim_dt` = 0.5 s,
//!   `puff_dt` = 100 s; release segment [0, 300 s) (3 puffs), run long enough
//!   that each lives its full lifetime; lifetime A = 15 000 s (60 km of
//!   travel). A stable nuclide, and a Kr-89-like one (`t1/2` = 189 s, synthetic:
//!   only the arithmetic is under test).
//! - **Gate 1 (changi vs the independent age sum), bound 1e-11 relative,
//!   derived:** both sides evaluate the same ~3e4 kernel terms, in slightly
//!   different operation order, so they can differ only by rounding: at most
//!   ~3e4 x 2^-52 = 7e-12 relative even if every addition rounded the same
//!   way. This checks everything `dilution_factors` does (Lagrangian
//!   advection, age binning, segment attribution and mass normalisation, decay
//!   weights) against an implementation that shares none of that code.
//!   ~~**Gate 1 (changi vs quadrature), bound 1e-9 relative:** the uniform age
//!   sum of a smooth integrand converges exponentially (Poisson summation), so
//!   changi should equal the quadrature to ~1e-11.~~ **CORRECTED 2026-09-29,
//!   before any result was recorded:** this pre-registered gate **failed**
//!   (worst 2.1e-5, class A at 120 m) because its derivation was wrong.
//!   changi's sigma_z is a *binned* power law whose bins meet with small jumps
//!   and slope changes, so the integrand is only piecewise smooth, and a
//!   uniform sum then carries an Euler-Maclaurin error from every breakpoint.
//!   The age-sum comparison replaced it as the correctness gate; the
//!   sum-vs-quadrature difference is kept as a **measurement** of changi's
//!   time-step error (below), not a gate.
//! - **Quadrature:** `petir::integration::qags_with_status`, split at every
//!   sigma_z breakpoint and at the peak window, accepted only when GSL's
//!   summed `abserr` is <= 1e-11 of the value.
//! - **Gate 2 (continuum train vs plume) — pre-registered:** where the
//!   expansion is valid — `H <= sigma_z(x_r)` (not in the vertical Gaussian's
//!   tail, where `P` varies faster than any power) **and** no sigma_z fit
//!   breakpoint within `x_r +/- 3 sigma_y(x_r)` (the expansion needs sigma
//!   smooth across the kernel) — require `|(R - 1) - c2| <= 0.5 |c2| + 1e-4`,
//!   i.e. the leading term predicts the residual's sign and size. `c2` is
//!   evaluated by a central difference with `h = 1e-3 x_r`. Points outside the
//!   validity domain are reported, not gated.
//!
//! ## Results and interpretation (measured 2026-09-29, `develop` 90ac7f3f7)
//!
//! Run with `cargo test --release -p buangkok --test changi_puff_train_vs_plume
//! -- --nocapture` for the full tables; 42 points (6 classes x 7 distances),
//! each for a stable and a Kr-89-like nuclide.
//!
//! **Gate 1 — changi's puff train is correct.** Worst difference from the
//! independent age sum: **3.9e-16** relative, over all 84 cases, most of
//! them bit-identical. The advection, age binning, segment normalisation and
//! decay weighting in `dilution_factors` do what they are documented to do.
//!
//! **Time-step error (measured, not gated).** changi's age sum against the
//! continuum integral, at `sim_dt` = 0.5 s:
//!
//! | Where | Error | Halving `dt` |
//! |---|---|---|
//! | Class C (no sigma_z breakpoints) | <= 1e-14 (rounding) | no change |
//! | Far field, all classes | <= 1e-9 | ~2-4x smaller |
//! | Breakpoint near the peak (e.g. A 120 m, A 350 m, D 350 m) | 2.1e-5, 1.0e-6, 3.7e-7 | 2.7x, 3.7x, 3.9x smaller |
//!
//! Halving `dt` cuts the error ~4x where it is visible: it is second order,
//! so the **slope changes** of the binned sigma_z fits dominate, not their
//! small value jumps (which would give 2x). At <= 2.2e-5 it is far below the
//! model's own fidelity; it is recorded so nobody mistakes it for a defect.
//!
//! **Gate 2 — the puff train reproduces pyDOSEIA's plume, residual as
//! predicted.** 21 of 42 points fall in the pre-registered validity domain and
//! **all 21 pass**. The measured `R - 1` tracks the derived `c2` closely, e.g.
//!
//! | Class, x | `R - 1` (measured) | `c2` (predicted) |
//! |---|---|---|
//! | D, 5.0 km | -6.878e-4 | -6.876e-4 |
//! | C, 2.5 km | -8.934e-4 | -8.984e-4 |
//! | B, 0.8 km | -7.022e-3 | -7.118e-3 |
//! | A, 1.5 km | +5.393e-2 | +5.415e-2 |
//!
//! Where it is valid, the puff train and the plume differ by at most ~7 %
//! (class A, 1-3 km, where `sigma_y / x` ~ 0.19) and by <= 0.13 % for C-F
//! beyond ~2 km, and the difference is the physics of a puff carrying
//! `sigma(x_p)`, not an error. Outside the domain the residual is large and
//! expected: near the 40 m stack in stable air the plume at `x_r` sits deep in
//! its vertical tail (`exp(-H^2 / 2 sigma_z^2)`), and puffs that have travelled
//! further carry larger `sigma_z` — F at 120 m gives `R` ~ 1e9 on a
//! concentration of ~1e-41 s/m^3, i.e. two negligible numbers.
//!
//! **The two codes' own sigma fits** (fourth test, reported only): changi's
//! ISC/Martin fits and pyDOSEIA's BARC/AERB fits give sigma_y within 4-15 %
//! and sigma_z mostly within 4 %, so their plumes mostly agree to within
//! 10 % (up to ~30 % near the stack) over 0.1-10 km. The exceptions: **class A beyond ~5 km**, where changi caps
//! sigma_z at 5 km and pyDOSEIA does not (plumes differ 2.5-8x), and **stable
//! classes near the stack** (F at 120 m: 7e2x), where small sigma_z
//! differences are exponentiated by the elevated release. A `changi` ->
//! `buangkok` hand-off (#375) has to pick one parameterisation; this is the
//! size of that choice.
//!
//! **What this does not cover:** varying wind (the exact-sum argument needs a
//! constant one), `StabilitySource::FromWind`, multiple sources, deposition,
//! and anything about real-world accuracy: both references are the same
//! Pasquill-Gifford Gaussian physics, so this is verification, not
//! validation.

use buangkok::pydoseia::dispersion::{
    master_equation_single_plume, sigma_y as pydoseia_sigma_y, sigma_z as pydoseia_sigma_z,
    Receptor as PlumeReceptor, StabilityClass as PlumeClass,
};
use changi::activity::chi_over_q::{dilution_factors, StabilitySource};
use changi::puff::dispersion::pasquill_gifford_sigmas;
use changi::puff::simulate::{
    constant_wind, AdvectionPolicy, EmissionPolicy, Receptor, RunConfig, Source,
};
use changi::puff::stability::StabilityClass;
use petir::integration::qags_with_status;
use uom::si::f64::{Frequency, Length, Time, Velocity};
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

const U: f64 = 4.0;
const H: f64 = 40.0;
const DISTANCES_M: [f64; 7] = [120.0, 350.0, 800.0, 1500.0, 2500.0, 5000.0, 8500.0];
const SIM_DT: f64 = 0.5;
const PUFF_DT: f64 = 100.0;
const SEGMENT_END: f64 = 300.0;
const LIFETIME: f64 = 15_000.0;
const KR89_HALF_LIFE_S: f64 = 189.0;

const GATE_1_REL: f64 = 1e-11;
/// Largest quadrature `abserr / value` accepted (see `continuum_train`).
const QUAD_ACCEPT: f64 = 1e-11;

const CLASSES: [(StabilityClass, PlumeClass, &str); 6] = [
    (StabilityClass::A, PlumeClass::A, "A"),
    (StabilityClass::B, PlumeClass::B, "B"),
    (StabilityClass::C, PlumeClass::C, "C"),
    (StabilityClass::D, PlumeClass::D, "D"),
    (StabilityClass::E, PlumeClass::E, "E"),
    (StabilityClass::F, PlumeClass::F, "F"),
];

/// changi's sigma_z fit breakpoints (km), per class, transcribed from
/// `changi/src/puff/dispersion.rs` `params` (upstream `class_params`). Used
/// only to decide Gate 2's validity domain. Class C is unbinned.
fn sigma_z_breakpoints_km(class: StabilityClass) -> &'static [f64] {
    match class {
        StabilityClass::A => &[0.1, 0.15, 0.2, 0.25, 0.3, 0.4, 0.5],
        StabilityClass::B => &[0.2, 0.4],
        StabilityClass::C => &[],
        StabilityClass::D => &[0.3, 1.0, 3.0, 10.0, 30.0],
        StabilityClass::E => &[0.1, 0.3, 1.0, 2.0, 4.0, 10.0, 20.0, 40.0],
        StabilityClass::F => &[0.2, 0.7, 1.0, 2.0, 3.0, 7.0, 15.0, 30.0, 60.0],
    }
}

fn sigmas(class: StabilityClass, x: f64) -> (f64, f64) {
    let s = pasquill_gifford_sigmas(class, Length::new::<meter>(x)).expect("x > 0");
    (s.sigma_y.get::<meter>(), s.sigma_z.get::<meter>())
}

/// The unit-mass Gaussian puff at a ground-level centreline receptor `x_r`,
/// for a puff centred at `x_p` that has travelled `x_p`. Written from the
/// closed form, not by calling changi's kernel:
/// `C = 1 / ((2 pi)^{3/2} sy^2 sz) exp(-(x_r - x_p)^2 / (2 sy^2)) 2 exp(-H^2 / (2 sz^2))`.
fn puff_kernel(class: StabilityClass, x_p: f64, x_r: f64) -> f64 {
    if x_p <= 0.0 {
        return 0.0;
    }
    let (sy, sz) = sigmas(class, x_p);
    let s = x_r - x_p;
    let amp = 1.0 / ((2.0 * core::f64::consts::PI).powf(1.5) * sy * sy * sz);
    amp * (-0.5 * s * s / (sy * sy)).exp() * 2.0 * (-0.5 * H * H / (sz * sz)).exp()
}

/// Ages (s) at which a puff crosses one of `class`'s sigma_z breakpoints,
/// inside `(0, LIFETIME)`. The kernel jumps there (the power-law bins meet
/// only approximately).
fn breakpoint_ages(class: StabilityClass) -> Vec<f64> {
    sigma_z_breakpoints_km(class)
        .iter()
        .map(|b| b * 1000.0 / U)
        .filter(|a| *a > 0.0 && *a < LIFETIME)
        .collect()
}

/// `integral_0^A K(u a) exp(-lambda a) da`.
///
/// The integrand is only **piecewise** smooth: sigma_z is a binned power law
/// whose bins meet with small jumps, so the range is split at every
/// breakpoint, and at `a_r +/- 12 sigma_y / u` so the adaptive rule sees the
/// peak. Pieces inside the peak window are integrated to 1e-12 relative; the
/// rest carry almost nothing, and a *relative* tolerance on a near-zero
/// integral is unreachable (QAGS reports round-off), so they get an absolute
/// tolerance of 1e-13 times the peak window's value.
///
/// QAGS may still return `RoundOff` when it cannot *certify* 1e-12; its
/// estimate and `abserr` are GSL's honest output. The result is accepted only
/// if the summed `abserr` is within [`QUAD_ACCEPT`] of the total, which keeps
/// the quadrature two orders inside Gate 1's floor; anything worse fails.
fn continuum_train(class: StabilityClass, x_r: f64, lambda: f64) -> f64 {
    let f = |a: f64| puff_kernel(class, U * a, x_r) * (-lambda * a).exp();
    let (sy, _) = sigmas(class, x_r);
    let a_r = x_r / U;
    let w = 12.0 * sy / U;
    let lo = (a_r - w).max(0.0);
    let hi = (a_r + w).min(LIFETIME);

    let mut edges = vec![0.0, lo, hi, LIFETIME];
    edges.extend(breakpoint_ages(class));
    edges.sort_by(f64::total_cmp);
    edges.dedup();

    let integrate = |a: f64, b: f64, epsabs: f64| {
        let o = qags_with_status(f, a, b, epsabs, 1e-12, 2000);
        (o.integral.value, o.integral.abserr)
    };
    let in_peak = |p: &[f64]| p[0] >= lo && p[1] <= hi;
    let (mut peak, mut err) = (0.0, 0.0);
    for p in edges.windows(2).filter(|p| p[1] > p[0] && in_peak(p)) {
        let (v, e) = integrate(p[0], p[1], 0.0);
        peak += v;
        err += e;
    }
    let floor = 1e-13 * peak.abs();
    let mut total = peak;
    for p in edges.windows(2).filter(|p| p[1] > p[0] && !in_peak(p)) {
        let (v, e) = integrate(p[0], p[1], floor);
        total += v;
        err += e;
    }
    assert!(
        err <= QUAD_ACCEPT * total.abs(),
        "quadrature not trustworthy for {class:?} at {x_r} m: abserr {err:e} of {total:e}"
    );
    total
}

/// The puff train's age sum written out independently of changi:
/// `sum_{k=0}^{A/dt} K(u k dt) exp(-lambda k dt) dt`, the exposure one unit
/// puff delivers over its lifetime (module doc).
fn discrete_age_sum(class: StabilityClass, x_r: f64, lambda: f64, dt: f64) -> f64 {
    let n = (LIFETIME / dt).floor() as usize;
    (0..=n)
        .map(|k| {
            let a = k as f64 * dt;
            puff_kernel(class, U * a, x_r) * (-lambda * a).exp() * dt
        })
        .sum()
}

/// buangkok's pyDOSEIA plume, ground-level centreline, with changi's sigmas at `x`.
fn plume_with_changi_sigmas(class: StabilityClass, x: f64) -> f64 {
    let (sy, sz) = sigmas(class, x);
    let t = master_equation_single_plume(
        Length::new::<meter>(sy),
        Length::new::<meter>(sz),
        U,
        Length::new::<meter>(H),
        PlumeReceptor::GroundLevelCentreline,
    );
    t.pre_expo * t.expo
}

/// Leading-order Laplace prediction `c2 = (sigma_y^2 P)'' / (2 P)`.
fn c2_prediction(class: StabilityClass, x: f64) -> f64 {
    let g = |x: f64| {
        let (sy, _) = sigmas(class, x);
        sy * sy * plume_with_changi_sigmas(class, x)
    };
    let h = 1e-3 * x;
    let g2 = (g(x + h) - 2.0 * g(x) + g(x - h)) / (h * h);
    g2 / (2.0 * plume_with_changi_sigmas(class, x))
}

fn gate_2_applies(class: StabilityClass, x: f64) -> bool {
    let (sy, sz) = sigmas(class, x);
    let lo = (x - 3.0 * sy) / 1000.0;
    let hi = (x + 3.0 * sy) / 1000.0;
    H <= sz
        && !sigma_z_breakpoints_km(class)
            .iter()
            .any(|b| *b >= lo && *b <= hi)
}

fn run_config(sim_dt: f64, puff_dt: f64, segment_end: f64, lifetime: f64) -> RunConfig {
    RunConfig {
        sim_dt: Time::new::<second>(sim_dt),
        puff_dt: Time::new::<second>(puff_dt),
        output_dt: Time::new::<second>(sim_dt),
        // The last puff of the segment is emitted just before `segment_end`
        // and must live its whole lifetime.
        duration: Time::new::<second>(segment_end + lifetime),
        puff_duration: Time::new::<second>(lifetime),
        start_hour: 12,
        emission_policy: EmissionPolicy::OnePuffPerEmission,
        advection: AdvectionPolicy::LagrangianTrajectory,
    }
}

/// changi's segment-0 `chi/Q` at each receptor, for each decay constant.
fn changi_chi_over_q(
    class: StabilityClass,
    distances: &[f64],
    lambdas: &[f64],
    sim_dt: f64,
    puff_dt: f64,
    segment_end: f64,
    lifetime: f64,
) -> Vec<Vec<f64>> {
    let cfg = run_config(sim_dt, puff_dt, segment_end, lifetime);
    let n_steps = (cfg.duration.get::<second>() / sim_dt).floor() as usize + 1;
    let wind = constant_wind(
        Velocity::new::<meter_per_second>(U),
        Velocity::new::<meter_per_second>(0.0),
        n_steps,
    );
    let source = Source {
        x: Length::new::<meter>(0.0),
        y: Length::new::<meter>(0.0),
        height: Length::new::<meter>(H),
    };
    let receptors: Vec<Receptor> = distances
        .iter()
        .map(|x| Receptor {
            x: Length::new::<meter>(*x),
            y: Length::new::<meter>(0.0),
            z: Length::new::<meter>(0.0),
        })
        .collect();
    let bounds = [
        Time::new::<second>(0.0),
        Time::new::<second>(segment_end),
        cfg.duration,
    ];
    let factors = dilution_factors(
        &[source],
        &bounds,
        &wind,
        &receptors,
        &cfg,
        StabilitySource::Fixed(class),
    );
    (0..distances.len())
        .map(|r| {
            lambdas
                .iter()
                .map(|l| {
                    factors
                        .dilution(r, 0, Frequency::new::<hertz>(*l))
                        .seconds_per_cubic_meter()
                })
                .collect()
        })
        .collect()
}

/// **Gate 1: changi's puff train equals the independently written age sum;
/// the time-step error against the continuum is measured and reported.**
///
/// Results, 2026-09-29: see the module doc, "Results".
#[test]
fn gate_1_changi_train_equals_the_independent_age_sum() {
    let lambdas = [0.0, core::f64::consts::LN_2 / KR89_HALF_LIFE_S];
    let mut worst_gate: f64 = 0.0;
    let mut worst_dt: f64 = 0.0;
    let mut failures = Vec::new();
    println!(
        "{:>2} {:>7} {:>13} {:>10} {:>10} {:>10} {:>10} {:>10}",
        "cl", "x (m)", "changi", "vs sum", "dt err", "dt/2 err", "Kr89 sum", "Kr89 dt"
    );
    for (class, _, name) in CLASSES {
        let got = changi_chi_over_q(
            class,
            &DISTANCES_M,
            &lambdas,
            SIM_DT,
            PUFF_DT,
            SEGMENT_END,
            LIFETIME,
        );
        for (r, x) in DISTANCES_M.iter().enumerate() {
            let mut row = Vec::new();
            for (l, lambda) in lambdas.iter().enumerate() {
                let c = got[r][l];
                let sum = discrete_age_sum(class, *x, *lambda, SIM_DT);
                let q = continuum_train(class, *x, *lambda);
                assert!(c > 0.0 && sum > 0.0 && q > 0.0, "{name} {x} m");
                let gate = (c - sum).abs() / sum;
                let dt_err = (sum - q) / q;
                let half = (discrete_age_sum(class, *x, *lambda, 0.5 * SIM_DT) - q) / q;
                worst_gate = worst_gate.max(gate);
                worst_dt = worst_dt.max(dt_err.abs());
                if gate > GATE_1_REL {
                    failures.push(format!(
                        "{name} {x} m lambda {lambda:e}: changi {c:e} vs sum {sum:e}, rel {gate:e}"
                    ));
                }
                row.push((c, gate, dt_err, half));
            }
            println!(
                "{name:>2} {x:>7.0} {:>13.6e} {:>10.2e} {:>10.2e} {:>10.2e} {:>10.2e} {:>10.2e}",
                row[0].0, row[0].1, row[0].2, row[0].3, row[1].1, row[1].2
            );
        }
    }
    println!(
        "worst changi-vs-sum: {worst_gate:.3e} (gate {GATE_1_REL:e}); \
         worst time-step error vs continuum: {worst_dt:.3e}"
    );
    assert!(
        failures.is_empty(),
        "changi's puff train disagrees with the independent age sum:\n{}",
        failures.join("\n")
    );
}

/// **Gate 2: the continuum puff train vs buangkok's pyDOSEIA plume, against
/// the derived second-order prediction.**
///
/// Results, 2026-09-29: see the module doc, "Results".
#[test]
fn gate_2_train_vs_plume_matches_the_derived_residual() {
    println!(
        "{:>2} {:>7} {:>13} {:>13} {:>11} {:>11} {:>6} {:>6}",
        "cl", "x (m)", "train", "plume", "R - 1", "c2", "sy/x", "gated"
    );
    let mut failures = Vec::new();
    let mut gated = 0;
    for (class, _, name) in CLASSES {
        for x in DISTANCES_M {
            let train = continuum_train(class, x, 0.0);
            let plume = plume_with_changi_sigmas(class, x);
            let r1 = train / plume - 1.0;
            let c2 = c2_prediction(class, x);
            let (sy, _) = sigmas(class, x);
            let applies = gate_2_applies(class, x);
            println!(
                "{name:>2} {x:>7.0} {train:>13.6e} {plume:>13.6e} {r1:>11.3e} {c2:>11.3e} {:>6.3} {:>6}",
                sy / x,
                if applies { "yes" } else { "no" }
            );
            if applies {
                gated += 1;
                if (r1 - c2).abs() > 0.5 * c2.abs() + 1e-4 {
                    failures.push(format!("{name} {x} m: R-1 {r1:e}, c2 {c2:e}"));
                }
            }
        }
    }
    println!(
        "gated points: {gated} of {}",
        CLASSES.len() * DISTANCES_M.len()
    );
    assert!(
        gated > 0,
        "the validity domain is empty; the gate tests nothing"
    );
    assert!(
        failures.is_empty(),
        "residual not predicted by the second-order term:\n{}",
        failures.join("\n")
    );
}

/// **`chi/Q` does not depend on `puff_dt`.** The time-integrated dilution
/// factor is a per-puff exposure, so emitting every 1 s or every 50 s must give
/// the same answer to rounding.
#[test]
fn chi_over_q_is_independent_of_puff_spacing() {
    let distances = [350.0, 1500.0];
    let lambdas = [0.0];
    let dense = changi_chi_over_q(
        StabilityClass::D,
        &distances,
        &lambdas,
        1.0,
        1.0,
        100.0,
        3000.0,
    );
    let sparse = changi_chi_over_q(
        StabilityClass::D,
        &distances,
        &lambdas,
        1.0,
        50.0,
        100.0,
        3000.0,
    );
    for (r, x) in distances.iter().enumerate() {
        let (a, b) = (dense[r][0], sparse[r][0]);
        let rel = (a - b).abs() / a;
        println!("D {x:>6.0} m: puff_dt 1 s {a:.12e}, 50 s {b:.12e}, rel {rel:.2e}");
        assert!(rel < 1e-12, "{x} m: {a:e} vs {b:e}");
    }
}

/// **Reported, not gated: how far the two codes' sigma fits differ.**
///
/// changi uses the ISC/Martin (1976) Pasquill-Gifford fits; pyDOSEIA uses the
/// BARC/AERB power laws. This is a model-choice difference, not a defect in
/// either port; it is what a `changi` -> `buangkok` hand-off (#375) would have
/// to reconcile. Printed: sigma ratios, and the pyDOSEIA plume with its own
/// sigmas over the pyDOSEIA plume with changi's.
#[test]
fn report_the_sigma_fit_difference() {
    println!(
        "{:>2} {:>7} {:>10} {:>10} {:>12}",
        "cl", "x (m)", "sy c/p", "sz c/p", "plume p/c"
    );
    for (class, pclass, name) in CLASSES {
        for x in DISTANCES_M {
            let (sy_c, sz_c) = sigmas(class, x);
            let xl = Length::new::<meter>(x);
            let sy_p = pydoseia_sigma_y(pclass, xl).get::<meter>();
            let sz_p = pydoseia_sigma_z(pclass, xl).get::<meter>();
            let own = master_equation_single_plume(
                Length::new::<meter>(sy_p),
                Length::new::<meter>(sz_p),
                U,
                Length::new::<meter>(H),
                PlumeReceptor::GroundLevelCentreline,
            );
            let ratio = own.pre_expo * own.expo / plume_with_changi_sigmas(class, x);
            println!(
                "{name:>2} {x:>7.0} {:>10.3} {:>10.3} {ratio:>12.3e}",
                sy_c / sy_p,
                sz_c / sz_p
            );
            assert!(sy_p > 0.0 && sz_p > 0.0 && ratio.is_finite());
        }
    }
}
