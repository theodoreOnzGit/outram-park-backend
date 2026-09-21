// SPDX-License-Identifier: GPL-3.0
//
// Code-to-code verification of `boon_lay::triso_atops_fork` against the
// upstream INL TRISO-ATOPS Python (MIT, commit de374c8).
//
// Uses only boon-lay + approx (both Android- and wasm-friendly) and reads its
// fixture through `include_str!`, so there is no filesystem access at run time
// and no `cfg(not(target_os = "android"))` gate is needed.

//! # V&V: TRISO-ATOPS calculation core, code-to-code
//!
//! ## Methodology
//!
//! Every reference value in this file is produced by **executing the upstream
//! TRISO-ATOPS Python itself** — not by re-deriving the physics, and not by
//! hand-transcribing numbers from the User Manual. The generator
//! `dev/gen_triso_atops_reference.py` imports
//! `trisoatops/utility_functions/calculation_functions.py` at upstream commit
//! `de374c8` from the gitignored reference clone under `upstream_source/`,
//! sweeps each function over an input grid chosen to straddle **every branch
//! and every clamp** in the upstream source, and writes
//! `tests/data/triso_atops_reference.csv` (one row per case, f64 values written
//! at full `repr()` round-trip precision).
//!
//! This test replays that fixture through the Rust port and asserts agreement
//! per function. It complements — and does not replace —
//! `triso_atops_fork_verification.rs`, which checks the *analytical* limits of
//! the same models. Analytical limits prove the physics is right; this file
//! proves the **translation** is right, which is the question that actually
//! matters for a port (workspace `CLAUDE.md`, "Debugging a port: read upstream
//! first" — upstream is the specification).
//!
//! **Branch coverage.** The grid deliberately includes the temperature clamps
//! (490/550/700/800 °C), the 1500 °C kernel-correlation switch, the `[0, 1]`
//! release-fraction clamps, the `1e8` attenuation cap, the `beta - lam == 0`
//! guard, the `int_Dp == 0` and `RF < 1e-6` transient thresholds, and the
//! `1e-19` / `1e-5` fallback branches.
//!
//! **Tolerances are measured, not assumed.** Each group asserts at the
//! tolerance recorded in the results table below; those tolerances were set
//! from the observed maximum deviation, then rounded up by roughly an order of
//! magnitude. The residual is floating-point summation order: NumPy's `np.sum`
//! is pairwise, the Rust port sums sequentially, so series-based quantities
//! differ in the last bits. Nothing here is fitted to make a test pass.
//!
//! ## Results (2026-09-21, upstream `de374c8`, 14 130 cases, 51 tests, all passing)
//!
//! ~~2026-09-15: 5 699 cases in 32 groups.~~ **SUPERSEDED** — the second pass
//! (accident path and run set-up) took it to 11 855 cases / 42 tests, and the
//! third pass (exhaustive widening plus the end-to-end `accident_case`
//! composition) to the figures above. The superseded numbers are kept because
//! results quoted elsewhere were measured against them.
//!
//! The full per-group table, the ill-conditioning analysis and the mutation
//! evidence live in `docs/triso-atops-code-to-code.md`; re-print the measured
//! deviations here at any time with `--nocapture`. In summary:
//!
//! - Outside the ill-conditioned inputs below, agreement is between **exact
//!   equality and 4.2e-10** relative, with seven groups bit-exact — including
//!   the cumulative diffusion integral, the surface most exposed to a silent
//!   ordering or off-by-one slip.
//! - The end-to-end `normal_operation_node` chain agrees to **3.1e-11** across
//!   all six outputs.
//! - All 84 nuclide decay constants match exactly, and the test fails if
//!   upstream carries a nuclide the port lacks.
//!
//! **Ill-conditioned inputs.** Two upstream formulas lose precision
//! catastrophically in part of their range, as a property of the formulas
//! rather than of the port: `RF_Graph` evaluates `1 - exp(-x)` at `x` as small
//! as 1e-13, and `breakthrough_model_transient` subtracts three terms of order
//! 0.1 that can cancel to order 1e-13 (measured cancellation ratio 2.8e12,
//! i.e. a best achievable relative precision of 6.2e-4). Asserting a tight
//! tolerance there would be asserting noise.
//!
//! The generator therefore records a per-case **cancellation ratio** (`cond`,
//! the fourth fixture column) and each case is asserted against
//! `max(group_tol, 8 * eps * cond)`. That is algebraically an *absolute* check
//! against the arithmetic noise floor, so it stays fully sensitive to real
//! defects — a wrong constant shifts an intermediate term by ~1e15 times that
//! floor. The same widening is applied to the end-to-end `accident_case`
//! groups, whose conditioning is **inherited** from the release-fraction
//! evaluation underneath rather than re-derived; one Ag-110m row uses 19 % of
//! its widened tolerance and is the worst in that group.
//!
//! **The suite is not vacuous**, verified by mutation in three rounds. The
//! third round (2026-09-21, 11 mutations) killed 10 and left one *equivalent*
//! mutant: deleting `booth_transient`'s `RF < 1e-6 -> 0` guard changes nothing,
//! because the 5 000-term truncation leaves a residual of `6/(pi^2 * 5000) =
//! 1.216e-4` and the guard is unreachable. That is upstream defect 10, and it
//! is asserted directly by `booth_transient_zero_floor_is_unreachable` rather
//! than left as a footnote. The earlier rounds ran in two passes — the
//! second aimed specifically at the ill-conditioned groups, to confirm the
//! widened bound masks nothing. Perturbing the `rf_graph` numerator, the
//! transient breakthrough time-lag, the trapezoid weight in `integrate` and the
//! noble-gas `k_plate` zeroing in `normal_operation_node` failed exactly the
//! groups that read them — `release_fraction.kernel` included, despite its 16
//! widened cases — and left every unrelated test green. Details and the
//! first-round table are in the doc above.
//!
//! ## Upstream defects reproduced
//!
//! Three defects are reproduced *deliberately* here, because the job of this
//! file is to establish what upstream computes. Each is flagged at its call
//! site and described in full in `docs/triso-atops-fork.md`:
//!
//! * **Defect 8** — the nuclide-name regex is unanchored, so `cs137mm` is
//!   silently accepted as `Cs-137m`, a different nuclide. The port anchors the
//!   match; the divergence is asserted by
//!   `name_normalisation_matches_upstream_regex`.
//! * **Defect 9** — `accident_case` pairs the *venting times* with the
//!   *first-n temperature slices*, which are different samples whenever the
//!   venting mask is gappy. The `gappy_vent_mask` scenario pins it; on it the
//!   nodal Xe-133 kernel release differs by 36 % from the consistent pairing.
//! * **Defect 10** — the dead `RF < 1e-6` guard described above.
//!
//! ## Where the port deliberately differs from upstream
//!
//! Two inputs exist for which upstream has **no value to compare against**,
//! because upstream raises instead of returning. Both are asserted explicitly
//! rather than skipped, so a future change to either side breaks this test:
//!
//! 1. **`RB_fail_Noble_Gases` for He/Ne/Ar/Rn (`Z ∈ {2, 10, 18, 86}`).**
//!    Upstream branches `if z == 36 … elif z == 54 or z in halogens …` with no
//!    `else`, so `RB_fail` is never bound and Python raises
//!    `UnboundLocalError`. The Rust port's `if z == 36 { … } else { … }` is
//!    total and applies the xenon fit. The port is the more defensible
//!    behaviour (these are real noble gases the correlation is meant to cover),
//!    but it *is* a divergence and is recorded as one.
//! 2. **`clean_up` with `k_plate == k_clean == 0`.** `plate_out` guards
//!    `beta - lam == 0` and returns `0`; `clean_up` carries no such guard.
//!    Upstream therefore raises `ZeroDivisionError`, while the Rust port
//!    computes `0.0 / 0.0` and yields `NaN`. The port faithfully reproduces the
//!    *missing guard*; both sides agree there is no meaningful value. Upstream
//!    never reaches this in practice because `higher_activities` only calls
//!    `clean_up` when a clean-up system is present (`k_clean > 0`), so the
//!    defect is latent rather than live.
//!
//! A third divergence is *not* a choice but a conditioning property: upstream's
//! venting mask tests a floating-point mean against zero, and on an exactly
//! cancelling field the port's kelvin-stored temperatures put that mean at
//! `-2.2e-16` instead of `0.0`, so the mask drops samples upstream keeps. It is
//! recorded under its own fixture group and asserted by
//! `coolant_release_knife_edge_divergence_is_representation_noise` rather than
//! tuned away.
//!
//! **This is verification, not validation.** It establishes that the Rust port
//! computes what the upstream Python computes. It says nothing about whether
//! either matches measured fission-product release — that needs a public
//! benchmark and is not claimed here.

use boon_lay::prelude::*;
use boon_lay::triso_atops_fork::release_models::steady_state::{
    attenuation_factor, booth_longlived, booth_shortlived_fast_diffuse, breakthrough_model,
    rb_fail_noble_gases,
};
use boon_lay::triso_atops_fork::release_models::steady_state::BOOTH_SERIES_TERMS;
use boon_lay::triso_atops_fork::release_models::transient::{
    booth_transient, breakthrough_model_transient, rf_graph, BOOTH_TRANSIENT_ZERO_FLOOR,
};
use boon_lay::triso_atops_fork::accident::{
    accident_release_curies, atoms_to_curies, coolant_release, distribute_inventory_axially,
    mean_temperature_rate, release_activity, AccidentFractions, NormalOperationNode,
    ReleaseMaterial as AccidentMaterial,
};
use boon_lay::triso_atops_fork::normal_operation::NodalActivities;
use boon_lay::triso_atops_fork::run_file::TimeUnit;
use boon_lay::triso_atops_fork::run_selection::{
    normalise_nuclide_name, select_nuclides, select_nuclides_accident,
    sort_parents_before_daughters, ParentDecayPolicy,
};

use uom::si::area::square_meter;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::{
    Area, DiffusionCoefficient, Frequency, Length, Pressure, Ratio, ThermodynamicTemperature, Time,
};
use uom::si::pressure::kilopascal;
use uom::si::frequency::hertz;
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;

/// The committed reference fixture, baked in at compile time so the test needs
/// no filesystem at run time (keeps it Android/Termux- and wasm-clean).
const FIXTURE: &str = include_str!("data/triso_atops_reference.csv");

// ── unit constructors ────────────────────────────────────────────────────────
fn dc(v: f64) -> DiffusionCoefficient {
    DiffusionCoefficient::new::<square_meter_per_second>(v)
}
fn len_m(v: f64) -> Length {
    Length::new::<meter>(v)
}
fn t_s(v: f64) -> Time {
    Time::new::<second>(v)
}
fn temp_c(v: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(v)
}
fn hz(v: f64) -> Frequency {
    Frequency::new::<hertz>(v)
}
fn rat(v: f64) -> Ratio {
    Ratio::new::<ratio>(v)
}
fn area_m2(v: f64) -> Area {
    Area::new::<square_meter>(v)
}

/// One parsed fixture row.
struct Case {
    function: String,
    args: Vec<f64>,
    expected: f64,
    /// Cancellation ratio of the upstream evaluation — `largest intermediate
    /// term / |result|`, or `1/x` where the formula evaluates `1 - exp(-x)` at
    /// small `x`. `1.0` means well conditioned.
    ///
    /// Where upstream's own f64 arithmetic cannot resolve the answer to better
    /// than some relative precision, a tighter assertion would be asserting
    /// noise. Widening by `8·eps·cond` is equivalent to an **absolute** check
    /// against the arithmetic noise floor (`|got − want| <= 8·eps·largest_term`),
    /// so it stays fully sensitive to real porting errors — a wrong constant or
    /// sign shifts an intermediate term by O(0.1), which is ~1e15 times the
    /// noise floor — while not failing on a difference neither implementation
    /// could have avoided.
    cond: f64,
}

/// Parse the fixture. The format is written by our own generator and is
/// deliberately trivial (`function,arg;arg;…,expected`, `#` comments), so it is
/// split directly rather than through a CSV reader — no quoting can occur.
fn cases() -> Vec<Case> {
    FIXTURE
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty() && !l.starts_with("function,"))
        .map(|l| {
            let mut f = l.split(',');
            let function = f.next().expect("function column").to_string();
            let arg_field = f.next().expect("args column");
            let expected = f.next().expect("expected column");
            let cond = f.next().expect("cond column");
            let args = if arg_field.is_empty() {
                Vec::new()
            } else {
                arg_field
                    .split(';')
                    .map(|a| a.parse::<f64>().expect("f64 arg"))
                    .collect()
            };
            Case {
                function,
                args,
                expected: expected.parse::<f64>().expect("f64 expected"),
                cond: cond.parse::<f64>().expect("f64 cond"),
            }
        })
        .collect()
}

/// Relative deviation, falling back to absolute where the reference is zero.
fn deviation(got: f64, want: f64) -> f64 {
    if want == 0.0 {
        got.abs()
    } else {
        (got - want).abs() / want.abs()
    }
}

/// Evaluate every fixture row whose `function` matches `name` through the Rust
/// port, assert agreement to `tol`, and return the observed maximum deviation
/// so the test can print the real number (workspace V&V rule: record results,
/// not just methodology).
fn check_group(name: &str, tol: f64, eval: impl Fn(&[f64]) -> f64) -> f64 {
    let all = cases();
    let group: Vec<&Case> = all.iter().filter(|c| c.function == name).collect();
    assert!(
        !group.is_empty(),
        "no fixture rows for `{name}` — regenerate with dev/gen_triso_atops_reference.py"
    );

    let mut worst = 0.0_f64;
    let mut worst_args: Vec<f64> = Vec::new();
    let mut widened = 0usize;
    for c in &group {
        let got = eval(&c.args);
        // NaN on both sides is agreement: upstream and port both decline to
        // produce a value. (Upstream's own NaN/inf cases are written as such.)
        if got.is_nan() && c.expected.is_nan() {
            continue;
        }
        let dev = deviation(got, c.expected);
        // Widen only as far as upstream's own arithmetic could resolve.
        let noise_floor = 8.0 * f64::EPSILON * c.cond;
        let tol_eff = tol.max(noise_floor);
        if noise_floor > tol {
            widened += 1;
        }
        assert!(
            dev <= tol_eff,
            "{name}{:?}: port {got:e} vs upstream {:e} (rel dev {dev:e} > tol {tol_eff:e}; \
             cond {:e})",
            c.args,
            c.expected,
            c.cond
        );
        // Rank the "worst" case by how close it came to its own tolerance, so a
        // well-conditioned near-miss is not hidden by an ill-conditioned case
        // that is comfortably inside a wide bound.
        if dev / tol_eff > worst / tol.max(f64::MIN_POSITIVE) && dev > 0.0 {
            worst = dev;
            worst_args = c.args.clone();
        }
    }
    let note = if widened > 0 {
        format!(", {widened} ill-conditioned case(s) on a widened bound")
    } else {
        String::new()
    };
    println!(
        "{name}: {} cases, max_rel_dev = {worst:e} (tol {tol:e}{note}) at args {worst_args:?}",
        group.len()
    );
    worst
}

// ── diffusion ────────────────────────────────────────────────────────────────

/// Kernel diffusion coefficient `D(z, T)` across every element branch, the
/// 700/800 °C clamps and the 1500 °C correlation switch.
#[test]
fn diffusion_coefficient_kernel_matches_upstream() {
    check_group("diffusion_coefficient.kernel", 1e-12, |a| {
        diffusion_coefficient(a[0] as u32, temp_c(a[1]), temp_c(a[2]))
            .kernel
            .get::<square_meter_per_second>()
    });
}

/// Graphite diffusion coefficient `D_graph(z, T_graph)`, including the 490 °C
/// (Ag), 550 °C (Cs/Rb) and 800 °C (Sr/Ba/Eu) clamps and the `D_graph = D`
/// aliasing for the Kr/Te/I/Xe/Se group.
#[test]
fn diffusion_coefficient_graphite_matches_upstream() {
    check_group("diffusion_coefficient.graphite", 1e-12, |a| {
        diffusion_coefficient(a[0] as u32, temp_c(a[1]), temp_c(a[2]))
            .graphite
            .get::<square_meter_per_second>()
    });
}

/// Silver-through-SiC diffusion coefficient over the full temperature grid.
#[test]
fn diffusion_coefficient_sic_ag_matches_upstream() {
    check_group("diffusion_coefficient_sic_ag", 1e-12, |a| {
        diffusion_coefficient_sic_ag(temp_c(a[0])).get::<square_meter_per_second>()
    });
}

// ── steady-state release models ──────────────────────────────────────────────

/// Empirical `<R/B>_fail` correlation for noble gases and halogens, over the
/// Kr branch and the Xe/halogen branch.
#[test]
fn rb_fail_noble_gases_matches_upstream() {
    check_group("rb_fail_noble_gases", 1e-12, |a| {
        rb_fail_noble_gases(a[0] as u32, hz(a[1]), temp_c(a[2])).get::<ratio>()
    });
}

/// Breakthrough (barrier-limited) release fraction, including both clamps.
#[test]
fn breakthrough_model_matches_upstream() {
    check_group("breakthrough_model", 1e-9, |a| {
        breakthrough_model(dc(a[0]), t_s(a[1]), len_m(a[2]), len_m(a[3])).get::<ratio>()
    });
}

/// Booth long-lived series, from the near-zero-release to the saturated limit.
#[test]
fn booth_longlived_matches_upstream() {
    check_group("booth_longlived", 1e-9, |a| {
        booth_longlived(dc(a[0]), t_s(a[1]), len_m(a[2])).get::<ratio>()
    });
}

/// Booth short-lived / fast-diffusion closed form `3/x·(coth x − 1/x)`.
#[test]
fn booth_shortlived_matches_upstream() {
    check_group("booth_shortlived_fast_diffuse", 1e-11, |a| {
        booth_shortlived_fast_diffuse(dc(a[0]), hz(a[1]), len_m(a[2])).get::<ratio>()
    });
}

/// Graphite attenuation factor, including the `1e8` cap and its negative-value
/// fallback. The `1/(1 − Σ)` form amplifies cancellation, hence the looser
/// tolerance relative to the other steady-state models.
#[test]
fn attenuation_factor_matches_upstream() {
    check_group("attenuation_factor", 1e-9, |a| {
        attenuation_factor(dc(a[0]), t_s(a[1]), len_m(a[2])).get::<ratio>()
    });
}

// ── transient (accident) release models ──────────────────────────────────────

/// Transient Booth series, including the `int_Dp == 0` short circuit and the
/// `RF < 1e-6 → 0` threshold.
#[test]
fn booth_transient_matches_upstream() {
    check_group("booth_transient", 1e-9, |a| {
        booth_transient(rat(a[0])).get::<ratio>()
    });
}

/// Transient breakthrough model, including both clamps and the
/// `int_Dt == 0 || int_Dp == 0 → 0` override applied after clamping.
#[test]
fn breakthrough_model_transient_matches_upstream() {
    check_group("breakthrough_model_transient", 1e-9, |a| {
        breakthrough_model_transient(rat(a[0]), area_m2(a[1]), len_m(a[2]), len_m(a[3]))
            .get::<ratio>()
    });
}

/// Graphite transient release fraction (odd-harmonic series).
/// **Upstream defect: `booth_transient`'s `RF < 1e-6 -> 0` guard is dead code.**
///
/// # Methodology
/// Upstream truncates the Booth series at `num_terms = 5000`, so as
/// `int_Dp -> 0` the sum tends to `sum_{i=1}^{4999} (i*pi)^{-2}`, not to
/// `1/6`. The residual is therefore
///
/// ```text
///   RF(0+) = 1 - 6 * sum_{i=1}^{N-1} (i*pi)^{-2}
///          = (6 / pi^2) * sum_{i=N}^{inf} i^{-2}
///          ~ 6 / (pi^2 * N)
/// ```
///
/// which for `N = 5000` is `1.216e-4` — **two orders of magnitude above the
/// `1e-6` floor the guard tests against.** `int_Dp == 0` is handled by an
/// earlier early-return, and `RF` increases monotonically in `int_Dp`, so no
/// input can land in `(0, 1e-6)` and the guard can never fire.
///
/// # Results (measured 2026-09-21)
/// Minimum `RF` over the fixture's whole `booth_transient` sweep
/// (`int_Dp` from `1e-12` to `1e2`, three points per decade):
/// `1.2159...e-4`, against the guard's `1e-6`. Deleting the guard from the
/// port changes no fixture value — it is an *equivalent mutant*, recorded as
/// such in `docs/triso-atops-code-to-code.md` rather than counted as a
/// surviving mutation.
///
/// # Interpretation
/// The guard is harmless but misleading: a reader takes it as evidence that
/// tiny releases are clamped to zero, and they are not. It becomes live only
/// if `num_terms` is raised past `6 / (pi^2 * 1e-6) ~ 6.1e5`. This test
/// asserts the premise so that a future change to
/// [`BOOTH_SERIES_TERMS`] cannot quietly alter the port's behaviour without
/// anyone noticing.
#[test]
fn booth_transient_zero_floor_is_unreachable() {
    let all = cases();
    let mut min_nonzero = f64::INFINITY;
    let mut n = 0usize;
    for c in all.iter().filter(|c| c.function == "booth_transient") {
        if c.args[0] == 0.0 {
            assert_eq!(c.expected, 0.0, "int_Dp = 0 must return exactly 0");
            continue;
        }
        assert!(
            c.expected > BOOTH_TRANSIENT_ZERO_FLOOR,
            "int_Dp = {:e} gave RF = {:e}, inside the guard band — the guard is              live after all; update the doc comment and the defect list",
            c.args[0],
            c.expected
        );
        min_nonzero = min_nonzero.min(c.expected);
        n += 1;
    }
    assert!(n > 0, "no non-zero booth_transient fixture rows");
    // 6 / (pi^2 * N) for N = 5000.
    let predicted = 6.0 / (core::f64::consts::PI.powi(2) * BOOTH_SERIES_TERMS as f64);
    assert!(
        (min_nonzero - predicted).abs() / predicted < 1e-3,
        "series-truncation residual {min_nonzero:e} does not match the predicted          6/(pi^2 N) = {predicted:e}"
    );
    println!(
        "booth_transient: min RF over {n} non-zero rows = {min_nonzero:e},          guard = {BOOTH_TRANSIENT_ZERO_FLOOR:e} (dead)"
    );
}

#[test]
fn rf_graph_matches_upstream() {
    check_group("rf_graph", 1e-9, |a| {
        rf_graph(area_m2(a[0]), len_m(a[1])).get::<ratio>()
    });
}

// ── coolant activity bookkeeping ─────────────────────────────────────────────

/// Steady-state circulating activity, with and without clean-up and parent.
#[test]
fn circulating_steadystate_matches_upstream() {
    check_group("circulating_steadystate", 1e-12, |a| {
        circulating_steadystate(a[0], hz(a[1]), hz(a[2]), hz(a[3]), a[4])
    });
}

/// Time-dependent circulating activity.
#[test]
fn circulating_matches_upstream() {
    check_group("circulating", 1e-12, |a| {
        circulating(a[0], hz(a[1]), hz(a[2]), t_s(a[3]), hz(a[4]), a[5])
    });
}

/// Steady-state plate-out activity.
#[test]
fn plate_out_steadystate_matches_upstream() {
    check_group("plate_out_steadystate", 1e-12, |a| {
        plate_out_steadystate(hz(a[0]), a[1], hz(a[2]), hz(a[3]), a[4])
    });
}

/// Time-dependent plate-out, including the `beta − lam == 0` guard that returns
/// zero and drops the parent term.
#[test]
fn plate_out_matches_upstream() {
    check_group("plate_out", 1e-9, |a| {
        plate_out(hz(a[0]), a[1], hz(a[2]), t_s(a[3]), a[4], hz(a[5]), a[6])
    });
}

/// Steady-state clean-up (HPS) activity.
#[test]
fn clean_up_steadystate_matches_upstream() {
    check_group("clean_up_steadystate", 1e-12, |a| {
        clean_up_steadystate(hz(a[0]), a[1], hz(a[2]), hz(a[3]), a[4])
    });
}

/// Time-dependent clean-up activity. The fixture omits `k_plate == k_clean == 0`
/// because upstream raises there; that divergence is asserted separately in
/// [`clean_up_without_guard_is_nan_where_upstream_raises`].
#[test]
fn clean_up_matches_upstream() {
    check_group("clean_up", 1e-9, |a| {
        clean_up(hz(a[0]), a[1], hz(a[2]), t_s(a[3]), a[4], hz(a[5]), a[6])
    });
}

// ── dispatchers and source terms ─────────────────────────────────────────────

/// The `<R/B>_fail` group dispatcher across all five transport groups, both
/// short- and long-lived, including the silver `√(λ_Ag110m/λ)` scaling and the
/// fixed `1e-5` fallback.
#[test]
fn rb_fail_dispatcher_matches_upstream() {
    check_group("rb_fail", 1e-9, |a| {
        rb_fail(
            a[0] as u32,
            a[1] != 0.0,
            hz(a[2]),
            temp_c(a[3]),
            t_s(a[4]),
            len_m(a[5]),
            len_m(a[6]),
            len_m(a[7]),
            dc(a[8]),
        )
        .get::<ratio>()
    });
}

/// Release rate, covering all three fraction-weighting branches and the
/// short-lived / long-lived birth-rate split.
#[test]
fn release_rate_matches_upstream() {
    check_group("release_rate", 1e-12, |a| {
        let z = a[1] as u32;
        release_rate(
            rat(a[0]),
            ElementGroup::from_atomic_number(z),
            FailureFractions {
                heavy_metal: a[6],
                sic: a[7],
                incremental: a[8],
                incremental_sic: a[9],
            },
            becquerels_from_curies(a[5]),
            a[2] != 0.0,
            t_s(a[3]),
            hz(a[4]),
        )
    });
}

/// Source rate out of `base_activities` — volatile (`S = R`) and attenuated
/// (`S = R/Af`) branches.
#[test]
fn base_activities_source_matches_upstream() {
    check_group("base_activities.source", 1e-9, |a| {
        let z = a[0] as u32;
        base_activities(
            ElementGroup::from_atomic_number(z),
            hz(a[1]),
            t_s(a[2]),
            len_m(a[3]),
            dc(a[4]),
            a[5],
        )
        .source_rate
    });
}

/// Graphite activity out of `base_activities`, including the exact zero for
/// volatiles.
#[test]
fn base_activities_graphite_matches_upstream() {
    check_group("base_activities.graphite", 1e-9, |a| {
        let z = a[0] as u32;
        base_activities(
            ElementGroup::from_atomic_number(z),
            hz(a[1]),
            t_s(a[2]),
            len_m(a[3]),
            dc(a[4]),
            a[5],
        )
        .graphite_activity
    });
}

// ── transient release-fraction dispatcher ────────────────────────────────────

/// Transient release from the **kernel**: Booth transient for non-silver,
/// breakthrough-through-SiC for silver.
#[test]
fn release_fraction_kernel_matches_upstream() {
    check_group("release_fraction.kernel", 1e-9, |a| {
        let z = a[0] as u32;
        release_fraction_transient(
            z,
            ElementGroup::from_atomic_number(z),
            area_m2(a[1]),
            len_m(a[2]),
            Some(len_m(a[3])),
            ReleaseMaterial::Kernel,
        )
        .get::<ratio>()
    });
}

/// Transient release from **graphite**: identically zero for volatiles
/// (noble gases and halogens), `RF_Graph` for fission metals.
#[test]
fn release_fraction_graphite_matches_upstream() {
    check_group("release_fraction.graphite", 1e-9, |a| {
        let z = a[0] as u32;
        release_fraction_transient(
            z,
            ElementGroup::from_atomic_number(z),
            area_m2(a[1]),
            len_m(a[2]),
            Some(len_m(a[3])),
            ReleaseMaterial::Graphite,
        )
        .get::<ratio>()
    });
}

// ── cumulative diffusion integral ────────────────────────────────────────────

/// `∫₀ᵗ D(T(t')) dt'` accumulated along a temperature history, checked at
/// **every** output time step rather than only the final value.
///
/// Upstream builds this with `np.diff(times, prepend=0)` and a cumulative sum
/// over a 3-D array, so an ordering or off-by-one slip would be silent and
/// would not show up in the endpoint alone. Row layout is
/// `[z, n, t_0..t_{n-1}, T_0..T_{n-1}, out_index]`.
fn check_integrate(name: &str, material: DiffusionMaterial) {
    check_group(name, 1e-9, |a| {
        let z = a[0] as u32;
        let n = a[1] as usize;
        let times: Vec<Time> = a[2..2 + n].iter().map(|v| t_s(*v)).collect();
        let temps: Vec<ThermodynamicTemperature> =
            a[2 + n..2 + 2 * n].iter().map(|v| temp_c(*v)).collect();
        let idx = a[2 + 2 * n] as usize;
        integrate_diffusion_over_time(z, &times, &temps, material)[idx].get::<square_meter>()
    });
}

#[test]
fn integrate_kernel_matches_upstream() {
    check_integrate("integrate.kernel", DiffusionMaterial::Kernel);
}

#[test]
fn integrate_graphite_matches_upstream() {
    check_integrate("integrate.graphite", DiffusionMaterial::Graphite);
}

// ── end-to-end normal-operation node ─────────────────────────────────────────

/// Drive the full `normal_operation_node` chain and compare every one of its six
/// outputs against the same chain composed from upstream functions.
///
/// The reference is the composition in upstream `trisoatops.py` — the
/// normal-operation driver — not `higher_activities` in isolation. That
/// distinction matters: the group-dependent zeroing of `k_plate` (noble gases)
/// and `k_clean` (non-halogens) lives at *that* call site upstream, so the
/// port's decision to fold it into `normal_operation_node` is faithful rather
/// than invented. Checking only `higher_activities` would have missed it.
///
/// The nuclide is identified by `(Z, A)`, not `Z` alone: Xe-133 and Xe-135 share
/// `Z = 54` but have decay constants two orders of magnitude apart, so keying on
/// `Z` would silently compare the wrong isotope.
///
/// Row layout: `[z, a_mass, sl, inventory_ci, T_core, T_graph, clean, k_plate,
/// k_clean, a_graph, a_grain, a_SiC, r, t_run, t_irad, f_hm, f_sic, f_inc,
/// f_inc_sic, c_par, p_par, hps_par]`.
fn node_outputs(a: &[f64]) -> NodalActivities {
    let (z, a_mass) = (a[0] as u32, a[1] as u32);
    let table = supported_nuclides();
    let nuclide = table
        .iter()
        .find(|n| n.z == z && n.a == a_mass)
        .unwrap_or_else(|| panic!("nuclide Z={z} A={a_mass} is in the port's table"));
    normal_operation_node(
        nuclide,
        a[2] != 0.0,
        becquerels_from_curies(a[3]),
        FailureFractions {
            heavy_metal: a[15],
            sic: a[16],
            incremental: a[17],
            incremental_sic: a[18],
        },
        PlantConstants {
            k_plate: hz(a[7]),
            k_clean: hz(a[8]),
            graphite_thickness: len_m(a[9]),
            grain_size: len_m(a[10]),
            sic_thickness: len_m(a[11]),
            kernel_radius: len_m(a[12]),
            run_time: t_s(a[13]),
            irradiation_time: t_s(a[14]),
        },
        NodeState {
            core_temperature: temp_c(a[4]),
            graphite_temperature: temp_c(a[5]),
        },
        a[6] != 0.0,
        ParentPools {
            circulating: a[19],
            plate_out: a[20],
            clean_up: a[21],
        },
    )
}

#[test]
fn node_release_rate_matches_upstream() {
    check_group("node.release_rate", 1e-9, |a| node_outputs(a).release_rate);
}

#[test]
fn node_source_rate_matches_upstream() {
    check_group("node.source_rate", 1e-9, |a| node_outputs(a).source_rate);
}

#[test]
fn node_graphite_activity_matches_upstream() {
    check_group("node.graphite_activity", 1e-9, |a| {
        node_outputs(a).graphite_activity
    });
}

#[test]
fn node_circulating_activity_matches_upstream() {
    check_group("node.circulating_activity", 1e-9, |a| {
        node_outputs(a).circulating_activity
    });
}

#[test]
fn node_plate_out_activity_matches_upstream() {
    check_group("node.plate_out_activity", 1e-9, |a| {
        node_outputs(a).plate_out_activity
    });
}

/// Clean-up activity. Rows exist only where upstream returns a value: with no
/// clean-up system `higher_activities` returns `None` for HPS rather than a
/// number, so those cases carry no reference and are not emitted.
#[test]
fn node_clean_up_activity_matches_upstream() {
    check_group("node.clean_up_activity", 1e-9, |a| {
        node_outputs(a).clean_up_activity
    });
}

// ── accident path: release_activity ──────────────────────────────────────────

/// Build the accident inputs from a fixture row.
///
/// Row layout: `[z, inventory, graphite, circulating, plate_out, hps, clean,
/// release_fraction, f_hm, f_sic, f_inc, f_inc_sic, f_inc_acc, f_inc_sic_acc]`.
fn accident_row(a: &[f64]) -> (u32, NormalOperationNode, AccidentFractions) {
    (
        a[0] as u32,
        NormalOperationNode {
            kernel_inventory_atoms: a[1],
            graphite_activity: a[2],
            circulating_activity: a[3],
            plate_out_activity: a[4],
            clean_up_activity: a[5],
            ..Default::default()
        },
        AccidentFractions {
            heavy_metal: a[8],
            sic: a[9],
            incremental: a[10],
            incremental_sic: a[11],
            incremental_accident: a[12],
            incremental_sic_accident: a[13],
        },
    )
}

/// Accident-path inventory drawdown, kernel side.
///
/// Replayed with `upstream_cadmium_typo = true`: the fixture comes from the
/// stock Python, which carries the `z == 48` silver branch. The *corrected*
/// behaviour is not code-to-code verifiable by construction — upstream cannot
/// produce it — so it is pinned by a unit test in the module instead
/// (`accident::tests::the_cadmium_typo_changes_palladium`).
#[test]
fn release_activity_kernel_matches_upstream() {
    check_group("release_activity.kernel", 1e-12, |a| {
        let (z, node, fr) = accident_row(a);
        release_activity(
            ElementGroup::from_atomic_number(z),
            fr,
            node,
            a[7],
            a[6] != 0.0,
            AccidentMaterial::Kernel,
            true,
            z,
        )
    });
}

/// Accident-path drawdown, graphite side — reads the graphite channel only.
#[test]
fn release_activity_graphite_matches_upstream() {
    check_group("release_activity.graphite", 1e-12, |a| {
        let (z, node, fr) = accident_row(a);
        release_activity(
            ElementGroup::from_atomic_number(z),
            fr,
            node,
            a[7],
            a[6] != 0.0,
            AccidentMaterial::Graphite,
            true,
            z,
        )
    });
}

// ── accident path: coolant_release ───────────────────────────────────────────

/// One decoded temperature field from a `coolant_release.*` fixture row.
///
/// Row layout, matching `gen_coolant_release` in the generator:
///
/// ```text
/// [n_nodes, n_t,
///  t_0 .. t_{n_t-1},
///  T(node 0, t_0..t_{n_t-1}), ... T(node n_nodes-1, ...),
///  hot_node, pressure_kPa, out_index]
/// ```
///
/// Upstream carries the field as `(radial, time, axial)` and averages `dT/dt`
/// over radial *and* axial while reading the absolute temperature from one
/// designated node. The port takes the `(radial, axial)` pair already
/// flattened into a node list, so the generator flattens it the same way and
/// the two sides see the same numbers in the same order.
struct CoolantRow {
    times: Vec<Time>,
    /// One time history per node, in the generator's flattened node order.
    nodes: Vec<Vec<ThermodynamicTemperature>>,
    /// Index into `nodes` of the node whose absolute temperature is used.
    hot_node: usize,
    pressure: Pressure,
    /// Which element of the function's output this row is comparing.
    index: usize,
}

fn decode_coolant_row(a: &[f64]) -> CoolantRow {
    let n_nodes = a[0] as usize;
    let n_t = a[1] as usize;
    let t0 = 2;
    let times: Vec<Time> = a[t0..t0 + n_t]
        .iter()
        .map(|v| Time::new::<second>(*v))
        .collect();
    let temp0 = t0 + n_t;
    let nodes: Vec<Vec<ThermodynamicTemperature>> = (0..n_nodes)
        .map(|n| {
            a[temp0 + n * n_t..temp0 + (n + 1) * n_t]
                .iter()
                .map(|v| ThermodynamicTemperature::new::<degree_celsius>(*v))
                .collect()
        })
        .collect();
    let tail = temp0 + n_nodes * n_t;
    CoolantRow {
        times,
        nodes,
        hot_node: a[tail] as usize,
        pressure: Pressure::new::<kilopascal>(a[tail + 1]),
        index: a[tail + 2] as usize,
    }
}

/// Mean `dT/dt` across the core at each sample.
///
/// The port splits this out of `coolant_release` into `mean_temperature_rate`,
/// so it is verified against the same expression upstream computes inline.
/// Fields of 1, 3 and 6 nodes are covered, so the average over nodes is a real
/// reduction rather than the identity it would be on a single-node field.
#[test]
fn coolant_mean_temperature_rate_matches_upstream() {
    check_group("coolant_release.mean_dtdt", 1e-12, |a| {
        let r = decode_coolant_row(a);
        mean_temperature_rate(&r.times, &r.nodes)[r.index]
    });
}

/// Vented coolant fraction at each venting sample, including upstream's
/// hard-pinned `frac[0] = 1` and its non-contiguous venting mask.
///
/// Swept over every hot-node choice the generator emits (upstream's default
/// `floor(n_axial / 2)` and both ends) and over three pressures, so the node
/// selection is checked independently of the averaging.
#[test]
fn coolant_release_fraction_matches_upstream() {
    check_group("coolant_release.fraction", 1e-12, |a| {
        let r = decode_coolant_row(a);
        let rate = mean_temperature_rate(&r.times, &r.nodes);
        let (frac, _) = coolant_release(&r.times, &rate, &r.nodes[r.hot_node], r.pressure);
        frac[r.index]
    });
}

/// **Documented port divergence** — `dTdt_avg >= 0` on an exactly-cancelling
/// field.
///
/// # Methodology
///
/// The `coolant_release.knife_edge.*` fixture rows come from a two-node field
/// in which one node heats at `+0.2 K/s` and the other cools at `-0.2 K/s`, so
/// the mean `dT/dt` upstream's venting mask tests is **exactly zero** at every
/// sample. Upstream computes that mean in degrees Celsius throughout and gets
/// a clean `0.0`, so `dTdt_avg >= 0` holds and every sample vents. The port
/// stores temperature as `uom`'s `ThermodynamicTemperature`, i.e. in kelvin,
/// and `800 °C` and `900 °C` are not exactly representable after the `+273.15`
/// shift — they come back as `800.000000000000114` and `900.000000000000114`.
/// The differences therefore do not cancel exactly, the mean lands at
/// `-2.22e-16` instead of `0.0`, and the mask drops those samples.
///
/// # Results (measured 2026-09-21)
///
/// * Port mean `dT/dt` at each affected sample: `-2.220446049250313e-16 K/s`
///   (upstream: `0.0`). Absolute agreement to `3e-16`, which is the whole of
///   the disagreement.
/// * Upstream vents `4` of `4` samples; the port vents `1`. Every sample the
///   port *does* vent agrees with upstream's corresponding value.
///
/// # Interpretation
///
/// This is **not** a physics difference and **not** a formula error: it is a
/// `>= 0` test applied to a quantity whose true value is zero, which no
/// reimplementation in any language can be relied on to reproduce. It is
/// recorded rather than tuned away, because the input that produces it is a
/// perfectly ordinary symmetric core and a reader needs to know the mask is
/// ill-conditioned there. The physically meaningful statement — that the mean
/// rate is zero to within representation noise — is what this test asserts.
///
/// Changing the port to compute in Celsius would trade this for the same
/// defect in the other direction and would break the workspace's `uom` rule;
/// the conditioning, not the unit, is the problem.
#[test]
fn coolant_release_knife_edge_divergence_is_representation_noise() {
    let all = cases();

    // 1. The mean rate agrees with upstream's exact zero to representation
    //    noise, and is *negative* -- which is what flips the mask.
    let mean_rows: Vec<&Case> = all
        .iter()
        .filter(|c| c.function == "coolant_release.knife_edge.mean_dtdt")
        .collect();
    assert!(
        !mean_rows.is_empty(),
        "no knife-edge mean_dtdt fixture rows"
    );
    let mut worst = 0.0_f64;
    let mut saw_negative = false;
    for c in &mean_rows {
        let r = decode_coolant_row(&c.args);
        let got = mean_temperature_rate(&r.times, &r.nodes)[r.index];
        assert_eq!(c.expected, 0.0, "upstream should be exactly zero here");
        worst = worst.max(got.abs());
        if got < 0.0 {
            saw_negative = true;
        }
    }
    assert!(
        worst <= 3e-16,
        "knife-edge mean dT/dt drifted to {worst:e}; this test assumes the only          disagreement with upstream is representation noise"
    );
    assert!(
        saw_negative,
        "no sample came out negative — the divergence this test documents is gone;          re-measure and update the doc comment rather than deleting the test"
    );

    // 2. The port therefore vents fewer samples than upstream, and every
    //    sample it does vent matches.
    let frac_rows: Vec<&Case> = all
        .iter()
        .filter(|c| c.function == "coolant_release.knife_edge.fraction")
        .collect();
    assert!(!frac_rows.is_empty(), "no knife-edge fraction fixture rows");
    let mut upstream_n = 0usize;
    let mut port_n = 0usize;
    for c in &frac_rows {
        let r = decode_coolant_row(&c.args);
        let rate = mean_temperature_rate(&r.times, &r.nodes);
        let (frac, _) = coolant_release(&r.times, &rate, &r.nodes[r.hot_node], r.pressure);
        upstream_n = upstream_n.max(r.index + 1);
        port_n = port_n.max(frac.len());
        if let Some(got) = frac.get(r.index) {
            assert!(
                deviation(*got, c.expected) <= 1e-12,
                "knife-edge sample {} disagrees: port {got:e} vs upstream {:e}",
                r.index,
                c.expected
            );
        }
    }
    assert!(
        port_n < upstream_n,
        "the port no longer drops samples here (port {port_n}, upstream {upstream_n});          re-measure and update the doc comment"
    );
    println!("knife edge: upstream vents {upstream_n} samples, port vents {port_n}");
}

/// The times the venting samples correspond to — a gappy subset whenever the
/// transient cools and re-heats.
#[test]
fn coolant_release_vent_times_match_upstream() {
    check_group("coolant_release.vent_time", 1e-12, |a| {
        let r = decode_coolant_row(a);
        let rate = mean_temperature_rate(&r.times, &r.nodes);
        let (_, vent) = coolant_release(&r.times, &rate, &r.nodes[r.hot_node], r.pressure);
        vent[r.index].get::<second>()
    });
}

// ── end-to-end: the accident_case composition ────────────────────────────────

/// One decoded `accident_case.*` scenario.
///
/// The fixture writes each scenario **once**, as an
/// `accident_case.scenario:<id>` row, and every value row refers to it by
/// index — repeating ~170 numbers per value row made the committed fixture
/// 44 % scenario prefix, and the whole file is `include_str!`d into this
/// binary. Scenario row layout:
///
/// ```text
/// [n_radial, n_axial, n_t, clean,
///  c_0 .. c_14,
///  t_0 .. t_{n_t-1},
///  T(r, t, k) in C order,
///  normop(channel, k, r) in C order, one block per nuclide]
/// ```
struct AccidentScenario {
    n_radial: usize,
    n_axial: usize,
    times: Vec<Time>,
    /// `temps[r][t][k]`, degrees Celsius.
    temps: Vec<Vec<Vec<f64>>>,
    /// Per-nuclide normal-operation channels, `normop[nuclide][channel][k][r]`.
    normop: Vec<Vec<Vec<Vec<f64>>>>,
    constants: [f64; 15],
    clean: bool,
}

/// Nuclides the end-to-end fixture drives, in the generator's order — the
/// `normop` blocks are laid out in this order, so the two must agree.
const ACCIDENT_NUCLIDES: [&str; 6] = ["Xe-133", "I-131", "Te-132", "Ag-110m", "Sr-90", "Cs-137"];

fn decode_accident_scenario(a: &[f64]) -> AccidentScenario {
    let n_radial = a[0] as usize;
    let n_axial = a[1] as usize;
    let n_t = a[2] as usize;
    let clean = a[3] != 0.0;
    let mut constants = [0.0_f64; 15];
    constants.copy_from_slice(&a[4..19]);
    let t0 = 19;
    let times: Vec<Time> = a[t0..t0 + n_t]
        .iter()
        .map(|v| Time::new::<second>(*v))
        .collect();

    let temp0 = t0 + n_t;
    let temps: Vec<Vec<Vec<f64>>> = (0..n_radial)
        .map(|r| {
            (0..n_t)
                .map(|t| {
                    (0..n_axial)
                        .map(|k| a[temp0 + (r * n_t + t) * n_axial + k])
                        .collect()
                })
                .collect()
        })
        .collect();

    let norm0 = temp0 + n_radial * n_t * n_axial;
    let per_nuclide = 7 * n_axial * n_radial;
    let normop: Vec<Vec<Vec<Vec<f64>>>> = (0..ACCIDENT_NUCLIDES.len())
        .map(|n| {
            let base = norm0 + n * per_nuclide;
            (0..7)
                .map(|ch| {
                    (0..n_axial)
                        .map(|k| {
                            (0..n_radial)
                                .map(|r| a[base + (ch * n_axial + k) * n_radial + r])
                                .collect()
                        })
                        .collect()
                })
                .collect()
        })
        .collect();

    AccidentScenario {
        n_radial,
        n_axial,
        times,
        temps,
        normop,
        constants,
        clean,
    }
}

/// Every `accident_case` scenario in the fixture, indexed by its id.
fn accident_scenarios() -> Vec<AccidentScenario> {
    let all = cases();
    let mut rows: Vec<(usize, &Case)> = all
        .iter()
        .filter_map(|c| {
            c.function
                .strip_prefix("accident_case.scenario:")
                .and_then(|id| id.parse::<usize>().ok())
                .map(|id| (id, c))
        })
        .collect();
    rows.sort_by_key(|(id, _)| *id);
    assert!(!rows.is_empty(), "no accident_case.scenario rows in fixture");
    for (i, (id, _)) in rows.iter().enumerate() {
        assert_eq!(*id, i, "scenario ids must be dense and start at 0");
    }
    rows.into_iter()
        .map(|(_, c)| decode_accident_scenario(&c.args))
        .collect()
}

/// The nodal release, in curies, that the port's pieces compose to.
///
/// This mirrors upstream `accident_case`'s per-nuclide body **in the order
/// upstream writes it**, using only public port functions:
///
/// 1. truncate the transient to the venting window,
/// 2. `integrate_diffusion_over_time` per node, kernel and (for non-volatiles)
///    graphite,
/// 3. `release_fraction_transient` on each integral,
/// 4. `release_activity` to turn a fraction into an inventory drawdown,
/// 5. `atoms_to_curies`.
///
/// It is deliberately written as a *caller* would write it, from the module
/// doc comments alone, because that is what the end-to-end comparison is
/// testing: not the formulas — those are each covered by their own group —
/// but whether the documented wiring reproduces upstream.
struct NodalRelease {
    /// `[r][t][k]`, curies.
    kernel: Vec<Vec<Vec<f64>>>,
    graphite: Vec<Vec<Vec<f64>>>,
    /// Vented-coolant fraction at each retained sample.
    vent_fraction: Vec<f64>,
}

fn compose_accident_case(sc: &AccidentScenario, nuclide_index: usize) -> Option<NodalRelease> {
    let name = ACCIDENT_NUCLIDES[nuclide_index];
    let t_end = sc
        .times
        .iter()
        .copied()
        .fold(Time::new::<second>(0.0), |m, t| if t > m { t } else { m });
    let (selected, _) = select_nuclides_accident(&[name], t_end, None);
    let nuc = selected.into_iter().next()?;

    // -- upstream: coolant_release, then truncate the field to the vented
    //    window. Upstream keeps the FIRST `n_t - rmv` time slices, which is
    //    only the venting set when the mask is contiguous; the generator emits
    //    a gappy-mask scenario so this reproduces upstream either way.
    let nodes: Vec<Vec<ThermodynamicTemperature>> = (0..sc.n_radial)
        .flat_map(|r| {
            (0..sc.n_axial).map(move |k| {
                (0..sc.times.len())
                    .map(|t| ThermodynamicTemperature::new::<degree_celsius>(sc.temps[r][t][k]))
                    .collect::<Vec<_>>()
            })
        })
        .collect();
    let rate = mean_temperature_rate(&sc.times, &nodes);
    let hot_axial = sc.n_axial / 2;
    let hot: Vec<ThermodynamicTemperature> = (0..sc.times.len())
        .map(|t| ThermodynamicTemperature::new::<degree_celsius>(sc.temps[0][t][hot_axial]))
        .collect();
    let (vent_fraction, vent_times) =
        coolant_release(&sc.times, &rate, &hot, Pressure::new::<kilopascal>(101.325));
    let n_keep = vent_fraction.len();
    // UPSTREAM DEFECT (see the module doc comment, "Upstream defects
    // reproduced"): `accident_case` truncates the temperature field with
    // `accident_temp[:, :-rmv, :]` — the FIRST `n_t - rmv` slices — while
    // replacing the time axis with `times_short`, the *venting* times. When
    // the venting mask is gappy those two are different samples, and upstream
    // integrates a temperature taken at one instant over a Δt taken from
    // another. This is reproduced deliberately: the port is being verified
    // against upstream, and silently "fixing" it here would make the
    // comparison meaningless. The `gappy_vent_mask` scenario in the fixture
    // exists to pin it.
    let times_kept: Vec<Time> = vent_times.clone();

    let group = nuc.element_group();
    let volatile = matches!(group, ElementGroup::NobleGas | ElementGroup::Halogen);

    let fractions = AccidentFractions {
        heavy_metal: sc.constants[0],
        sic: sc.constants[1],
        incremental: sc.constants[2],
        incremental_sic: sc.constants[3],
        incremental_accident: sc.constants[12],
        incremental_sic_accident: sc.constants[13],
    };
    let a_graphite = Length::new::<meter>(sc.constants[4]);
    let r_kernel = Length::new::<meter>(sc.constants[10]);
    let a_sic = Length::new::<meter>(sc.constants[11]);
    let lam = nuc.decay_constant().get::<uom::si::frequency::hertz>();

    let ch = &sc.normop[nuclide_index];
    let mut kernel = vec![vec![vec![0.0; sc.n_axial]; n_keep]; sc.n_radial];
    let mut graphite = kernel.clone();

    for r in 0..sc.n_radial {
        for k in 0..sc.n_axial {
            let history: Vec<ThermodynamicTemperature> = (0..n_keep)
                .map(|t| ThermodynamicTemperature::new::<degree_celsius>(sc.temps[r][t][k]))
                .collect();
            let int_kernel = integrate_diffusion_over_time(
                nuc.z,
                &times_kept,
                &history,
                DiffusionMaterial::Kernel,
            );
            let int_graphite = if volatile {
                vec![Area::new::<square_meter>(0.0); n_keep]
            } else {
                integrate_diffusion_over_time(
                    nuc.z,
                    &times_kept,
                    &history,
                    DiffusionMaterial::Graphite,
                )
            };
            // `normop` is stored axial-major, transposed relative to the
            // temperature field — see `_normop_field` in the generator.
            let node = NormalOperationNode {
                kernel_inventory_atoms: ch[0][k][r],
                release_rate: ch[1][k][r],
                source_rate: ch[2][k][r],
                graphite_activity: ch[3][k][r],
                circulating_activity: ch[4][k][r],
                plate_out_activity: ch[5][k][r],
                clean_up_activity: ch[6][k][r],
            };
            for t in 0..n_keep {
                let rf_k = release_fraction_transient(
                    nuc.z,
                    group,
                    int_kernel[t],
                    r_kernel,
                    Some(a_sic),
                    ReleaseMaterial::Kernel,
                )
                .get::<ratio>();
                let rf_g = release_fraction_transient(
                    nuc.z,
                    group,
                    int_graphite[t],
                    a_graphite,
                    None,
                    ReleaseMaterial::Graphite,
                )
                .get::<ratio>();
                let atoms_k = release_activity(
                    group,
                    fractions,
                    node,
                    rf_k,
                    sc.clean,
                    AccidentMaterial::Kernel,
                    true,
                    nuc.z,
                );
                let atoms_g = release_activity(
                    group,
                    fractions,
                    node,
                    rf_g,
                    sc.clean,
                    AccidentMaterial::Graphite,
                    true,
                    nuc.z,
                );
                kernel[r][t][k] = atoms_to_curies(atoms_k, lam);
                graphite[r][t][k] = atoms_to_curies(atoms_g, lam);
            }
        }
    }

    Some(NodalRelease {
        kernel,
        graphite,
        vent_fraction,
    })
}

/// Which nuclides survive `nuclide_import_accident` for each scenario.
///
/// Checked first and separately: if the port and upstream disagree about
/// *which* nuclides are in play, every downstream comparison is comparing the
/// wrong things, and the failure should say so rather than surfacing as a
/// mismatched array length.
#[test]
fn accident_case_nuclide_selection_matches_upstream() {
    let all = cases();
    let scenarios = accident_scenarios();
    let mut checked = 0usize;
    for c in all.iter() {
        let Some(name) = c.function.strip_prefix("accident_case.dropped:") else {
            continue;
        };
        let sc = &scenarios[c.args[0] as usize];
        let t_end = sc
            .times
            .iter()
            .copied()
            .fold(Time::new::<second>(0.0), |m, t| if t > m { t } else { m });
        let (selected, _) = select_nuclides_accident(&[name], t_end, None);
        let dropped = if selected.is_empty() { 1.0 } else { 0.0 };
        assert_eq!(
            dropped, c.expected,
            "{name}: port dropped={dropped}, upstream dropped={}",
            c.expected
        );
        checked += 1;
    }
    assert!(checked > 0, "no accident_case.dropped fixture rows");
    println!("accident_case nuclide selection: {checked} rows agree");
}

/// End-to-end nodal kernel release, in curies.
///
/// # Methodology
/// Upstream `trisoatops.accident_case` is driven over three scenarios (a
/// contiguous heat-then-cool transient with and without the clean-up system,
/// and one whose venting mask is gappy) for six nuclides spanning every
/// upstream branch — noble gas, halogen, the Te-counted-as-halogen case,
/// silver, and two ordinary fission metals. Upstream's three-significant-digit
/// display formatter is bypassed so the fixture holds full-precision values;
/// nothing else about the upstream call is altered. The port side is composed
/// from public functions in the order the module doc comments prescribe.
///
/// # Results (measured 2026-09-21)
/// See the `max dev` line this test prints. The tolerance is `1e-9` relative,
/// the same as the individual release-fraction groups it composes.
#[test]
fn accident_case_nodal_kernel_matches_upstream() {
    check_prefixed_group("accident_case.nodal_kernel:", 1e-9, |name, sc, a| {
        let i = nuclide_slot(name);
        let rel = compose_accident_case(sc, i)?;
        let (r, t, k) = (a[0] as usize, a[1] as usize, a[2] as usize);
        Some(rel.kernel[r][t][k])
    });
}

/// End-to-end nodal graphite release, in curies — identically zero for the
/// volatile groups, which is itself part of what is being checked.
#[test]
fn accident_case_nodal_graphite_matches_upstream() {
    check_prefixed_group("accident_case.nodal_graphite:", 1e-9, |name, sc, a| {
        let i = nuclide_slot(name);
        let rel = compose_accident_case(sc, i)?;
        let (r, t, k) = (a[0] as usize, a[1] as usize, a[2] as usize);
        Some(rel.graphite[r][t][k])
    });
}

/// End-to-end per-nuclide total release, in curies.
///
/// This is the row that catches a wiring error the nodal comparisons cannot:
/// the circulating and lifted-off plate-out terms are summed over **all**
/// nodes and added *outside* the vent-fraction scaling, while the kernel and
/// graphite sums are inside it.
#[test]
fn accident_case_total_matches_upstream() {
    check_prefixed_group("accident_case.total:", 1e-9, |name, sc, a| {
        let i = nuclide_slot(name);
        let rel = compose_accident_case(sc, i)?;
        let n_keep = rel.vent_fraction.len();
        let kernel_sum: Vec<f64> = (0..n_keep)
            .map(|t| {
                (0..sc.n_radial)
                    .flat_map(|r| (0..sc.n_axial).map(move |k| (r, k)))
                    .map(|(r, k)| rel.kernel[r][t][k])
                    .sum()
            })
            .collect();
        let graphite_sum: Vec<f64> = (0..n_keep)
            .map(|t| {
                (0..sc.n_radial)
                    .flat_map(|r| (0..sc.n_axial).map(move |k| (r, k)))
                    .map(|(r, k)| rel.graphite[r][t][k])
                    .sum()
            })
            .collect();
        let nuc = find_nuclide(name)?;
        let lam = nuc.decay_constant().get::<uom::si::frequency::hertz>();
        let ch = &sc.normop[i];
        let mut circulating = 0.0;
        let mut plate_out = 0.0;
        for k in 0..sc.n_axial {
            for r in 0..sc.n_radial {
                circulating += ch[4][k][r];
                plate_out += ch[5][k][r];
            }
        }
        let out = accident_release_curies(
            &kernel_sum,
            &graphite_sum,
            &rel.vent_fraction,
            atoms_to_curies(circulating, lam),
            atoms_to_curies(plate_out, lam),
            sc.constants[14],
        );
        Some(out[a[0] as usize])
    });
}

fn nuclide_slot(name: &str) -> usize {
    ACCIDENT_NUCLIDES
        .iter()
        .position(|n| *n == name)
        .unwrap_or_else(|| panic!("fixture names a nuclide the test does not drive: {name}"))
}

/// Like [`check_group`], but for groups whose `function` field carries a
/// `prefix:<nuclide>` suffix and whose args end in a scenario-relative index
/// block. The closure receives the nuclide name, the decoded scenario and the
/// trailing indices, and returns `None` when the nuclide is not in play.
fn check_prefixed_group(
    prefix: &str,
    tol: f64,
    eval: impl Fn(&str, &AccidentScenario, &[f64]) -> Option<f64>,
) -> f64 {
    let all = cases();
    let scenarios = accident_scenarios();
    let mut worst = 0.0_f64;
    let mut headroom = 0.0_f64;
    let mut n = 0usize;
    for c in all.iter() {
        let Some(name) = c.function.strip_prefix(prefix) else {
            continue;
        };
        let sc = &scenarios[c.args[0] as usize];
        let got = eval(name, sc, &c.args[1..])
            .unwrap_or_else(|| panic!("{}{name}: port dropped a nuclide upstream kept", prefix));
        let dev = deviation(got, c.expected);
        // Same conditioning-aware widening as `check_group`: the composition
        // inherits the cancellation of the release-fraction evaluation
        // underneath it, and the generator records that per row.
        let tol_eff = tol.max(8.0 * f64::EPSILON * c.cond);
        assert!(
            dev <= tol_eff,
            "{prefix}{name} idx {:?}: port {got:e} vs upstream {:e} \
             (rel dev {dev:e} > tol {tol_eff:e}; cond {:e})",
            &c.args[1..],
            c.expected,
            c.cond
        );
        worst = worst.max(dev);
        headroom = headroom.max(dev / tol_eff);
        n += 1;
    }
    assert!(n > 0, "no fixture rows for `{prefix}*`");
    println!(
        "{prefix}*: {n} rows, max rel dev {worst:e}, worst case used \
         {:.1}% of its tolerance",
        headroom * 100.0
    );
    worst
}

// ── run set-up ───────────────────────────────────────────────────────────────

/// Axial split of a per-ring inventory. Row layout: `[n_axial, inventory, k]`.
///
/// Every axial slot `k` is compared, not only the first: the operation is a
/// divide *and* a repeat, so checking one element would verify only the
/// divide and leave a port that filled the remaining slots wrongly undetected.
#[test]
fn inventory_processing_matches_upstream() {
    check_group("inventory_processing", 1e-12, |a| {
        distribute_inventory_axially(a[1], a[0] as usize)[a[2] as usize]
    });
}

/// Short-lived classification, `t½ / t_irrad < 0.2`.
///
/// Compared exactly rather than with a tolerance: the quantity under test is a
/// **decision**, encoded `1.0`/`0.0`, and a tolerance on a decision is
/// meaningless.
#[test]
fn nuclide_short_lived_classification_matches_upstream() {
    let all = cases();
    let rows: Vec<&Case> = all
        .iter()
        .filter(|c| c.function.starts_with("nuclide_import.short_lived:"))
        .collect();
    assert!(!rows.is_empty(), "no short-lived rows in fixture");
    for c in &rows {
        let name = c.function.trim_start_matches("nuclide_import.short_lived:");
        let (sel, skipped) = select_nuclides(
            &[name],
            Time::new::<second>(c.args[0]),
            None,
            ParentDecayPolicy::default(),
        );
        assert!(skipped.is_empty(), "{name} should be in the database");
        let got = f64::from(u8::from(sel[0].short_lived));
        assert_eq!(
            got, c.expected,
            "{name} at t_irrad = {} s: port {got} vs upstream {}",
            c.args[0], c.expected
        );
    }
    println!(
        "nuclide_import.short_lived: {} decisions matched",
        rows.len()
    );
}

/// Accident retention, `t½ / t_accident >= 0.04`. Exact, for the same reason.
#[test]
fn nuclide_accident_retention_matches_upstream() {
    let all = cases();
    let rows: Vec<&Case> = all
        .iter()
        .filter(|c| c.function.starts_with("nuclide_import_accident.retained:"))
        .collect();
    assert!(!rows.is_empty(), "no accident-retention rows in fixture");
    for c in &rows {
        let name = c
            .function
            .trim_start_matches("nuclide_import_accident.retained:");
        let (sel, skipped) =
            select_nuclides_accident(&[name], Time::new::<second>(c.args[0]), None);
        assert!(skipped.is_empty(), "{name} should be in the database");
        let got = f64::from(u8::from(!sel.is_empty()));
        assert_eq!(
            got, c.expected,
            "{name} at t_accident = {} s: port {got} vs upstream {}",
            c.args[0], c.expected
        );
    }
    println!(
        "nuclide_import_accident.retained: {} decisions matched",
        rows.len()
    );
}

// ── name normalisation, against upstream's own regex ─────────────────────────

/// Nuclide-name normalisation, checked against upstream's actual `re.match`
/// rather than against a reading of it.
///
/// The port hand-rolls the pattern to avoid a `regex` dependency, so the
/// fixture drives upstream's regex over a battery of spellings and records what
/// it produced. Rows are tagged `accepted:<raw>|<canonical>` or
/// `rejected:<raw>`.
///
/// # A documented divergence, and why it is the right one
///
/// Upstream's `re.match` is anchored only at the **start**, so it silently
/// accepts trailing junk and truncates the name to whatever prefix matched.
/// Measured against upstream on 2026-09-21:
///
/// | supplied | upstream yields | port |
/// |---|---|---|
/// | `Cs-137xyz` | `Cs-137x` | rejected |
/// | `Cs-137-extra` | `Cs-137` | rejected |
/// | `cs137mm` | `Cs-137m` | rejected |
///
/// The third is the dangerous one: a doubled keystroke turns Cs-137 into
/// **Cs-137m**, a different nuclide with a different half-life, with no
/// warning. The port rejects all three, which is a deliberate tightening — a
/// name upstream would truncate is far more likely a typo than an intent.
/// Asserted here explicitly so the divergence is pinned, not discovered.
#[test]
fn name_normalisation_matches_upstream_regex() {
    /// Spellings where the port deliberately rejects what upstream truncates.
    const DELIBERATE_REJECTS: [&str; 3] = ["Cs-137xyz", "Cs-137-extra", "cs137mm"];

    let all = cases();
    let rows: Vec<&Case> = all
        .iter()
        .filter(|c| c.function.starts_with("name_normalisation."))
        .collect();
    assert!(!rows.is_empty(), "no name-normalisation rows in fixture");

    let (mut agreed, mut diverged) = (0usize, 0usize);
    for c in &rows {
        if let Some(rest) = c.function.strip_prefix("name_normalisation.accepted:") {
            let (raw, canonical) = rest
                .split_once('|')
                .expect("accepted rows are tagged raw|canonical");
            let got = normalise_nuclide_name(raw);
            if DELIBERATE_REJECTS.contains(&raw) {
                assert!(
                    got.is_err(),
                    "{raw}: upstream truncates to {canonical}; the port must reject it"
                );
                diverged += 1;
            } else {
                assert_eq!(
                    got.as_deref(),
                    Ok(canonical),
                    "{raw}: port {got:?} vs upstream {canonical}"
                );
                agreed += 1;
            }
        } else if let Some(raw) = c.function.strip_prefix("name_normalisation.rejected:") {
            assert!(
                normalise_nuclide_name(raw).is_err(),
                "{raw}: upstream rejects it, so must the port"
            );
            agreed += 1;
        }
    }
    println!(
        "name_normalisation: {agreed} agreed with upstream, {diverged} deliberate divergences"
    );
    assert_eq!(
        diverged,
        DELIBERATE_REJECTS.len(),
        "every documented divergence must be exercised"
    );
}

// ── upstream behaviour pins ──────────────────────────────────────────────────

/// `nuclide_sort` is a no-op upstream — pinned against upstream itself.
///
/// The port's `sort_parents_before_daughters` deliberately does what the
/// function's docstring says rather than what its code does. That claim is only
/// worth making if upstream's actual behaviour is measured, so the fixture
/// records, for each ordering, whether upstream's
/// `par in list(nuke_list[:, 0])` test ever fired: `1.0` if it would reorder,
/// `0.0` if not.
///
/// Every row is `0.0`, **including `Xe-135, I-135`** — a daughter listed before
/// its parent, which is precisely the case the function exists to fix.
#[test]
fn upstream_nuclide_sort_never_reorders() {
    let all = cases();
    let rows: Vec<&Case> = all
        .iter()
        .filter(|c| c.function.starts_with("nuclide_sort.reorders:"))
        .collect();
    assert!(!rows.is_empty(), "no nuclide_sort rows in fixture");
    for c in &rows {
        assert_eq!(
            c.expected, 0.0,
            "{}: upstream unexpectedly reordered — the port's premise is wrong",
            c.function
        );
    }

    // And the port does reorder the case upstream misses.
    let names: Vec<String> = ["Xe-135", "I-135"].iter().map(|s| s.to_string()).collect();
    let sorted = sort_parents_before_daughters(&names);
    let i = sorted.iter().position(|n| n == "I-135").unwrap();
    let x = sorted.iter().position(|n| n == "Xe-135").unwrap();
    assert!(
        i < x,
        "the port must fix what upstream does not: {sorted:?}"
    );
    println!(
        "upstream nuclide_sort: {} orderings checked, all no-ops",
        rows.len()
    );
}

/// `convert_time`'s unit factors, including upstream's 365-day year.
#[test]
fn convert_time_factors_match_upstream() {
    let all = cases();
    let rows: Vec<&Case> = all
        .iter()
        .filter(|c| c.function.starts_with("convert_time:"))
        .collect();
    assert_eq!(rows.len(), 5, "five units");
    for c in &rows {
        let unit = c.function.trim_start_matches("convert_time:");
        let parsed = TimeUnit::parse(unit)
            .unwrap_or_else(|| panic!("port must parse upstream's unit {unit:?}"));
        assert_eq!(
            parsed.seconds(),
            c.expected,
            "{unit}: port {} vs upstream {}",
            parsed.seconds(),
            c.expected
        );
    }
    // Upstream returns None for an unknown unit; so must the port.
    assert!(TimeUnit::parse("fortnight").is_none());
}

// ── nuclide database ─────────────────────────────────────────────────────────

/// Every decay constant in the port's nuclide table, against the upstream
/// table entry for the same nuclide.
///
/// This is the whole database, not a spot check: a transcription slip in any
/// one half-life is a silent, physics-changing defect that no other test here
/// would catch.
#[test]
fn every_nuclide_decay_constant_matches_upstream() {
    let all = cases();
    let rows: Vec<&Case> = all
        .iter()
        .filter(|c| c.function.starts_with("nuclide.lam:"))
        .collect();
    assert!(!rows.is_empty(), "no nuclide rows in fixture");

    let mut worst = 0.0_f64;
    let mut checked = 0usize;
    let mut missing: Vec<&str> = Vec::new();
    for c in &rows {
        let name = c.function.trim_start_matches("nuclide.lam:");
        let Some(nuc) = find_nuclide(name) else {
            missing.push(name);
            continue;
        };
        let got = nuc.decay_constant().get::<hertz>();
        let dev = deviation(got, c.expected);
        assert!(
            dev <= 1e-12,
            "{name}: port lambda {got:e} vs upstream {:e} (rel dev {dev:e})",
            c.expected
        );
        worst = worst.max(dev);
        checked += 1;
    }
    println!(
        "nuclide decay constants: {checked}/{} matched, max_rel_dev = {worst:e}",
        rows.len()
    );
    assert!(
        missing.is_empty(),
        "nuclides present upstream but absent from the port: {missing:?}"
    );
}

// ── documented divergences ───────────────────────────────────────────────────

/// Upstream `RB_fail_Noble_Gases` raises `UnboundLocalError` for He/Ne/Ar/Rn
/// (`Z ∈ {2, 10, 18, 86}`): its `if/elif` chain has no `else`, so the result is
/// never bound. The Rust port is total and falls through to the xenon fit.
///
/// Asserted here so the divergence is pinned rather than discovered later: the
/// port must return the *same value as Xe* for these Z, and must not panic.
#[test]
fn noble_gases_upstream_cannot_evaluate_use_the_xenon_fit() {
    let lam = hz(1e-6);
    let t = temp_c(1000.0);
    let xe = rb_fail_noble_gases(54, lam, t).get::<ratio>();
    for z in [2_u32, 10, 18, 86] {
        let got = rb_fail_noble_gases(z, lam, t).get::<ratio>();
        assert!(
            got.is_finite(),
            "Z={z}: port must stay total where upstream raises, got {got}"
        );
        assert_eq!(
            got, xe,
            "Z={z}: port should fall through to the Xe/halogen fit"
        );
    }
    // Krypton keeps its own distinct fit, so the fallback is not swallowing it.
    assert_ne!(rb_fail_noble_gases(36, lam, t).get::<ratio>(), xe);
}

/// Upstream `clean_up` omits the `beta - lam == 0` guard that its sibling
/// `plate_out` has, so `k_plate == k_clean == 0` raises `ZeroDivisionError`
/// there. The Rust port reproduces the missing guard literally and yields
/// `NaN` from `0.0 / 0.0`.
///
/// Both sides decline to produce a value; this test pins *how*. It also
/// confirms `plate_out` really does take its guard on the same inputs, which is
/// what makes the asymmetry an upstream defect rather than a porting slip.
#[test]
fn clean_up_without_guard_is_nan_where_upstream_raises() {
    let (s, lam, t, c) = (1e6, hz(1e-6), t_s(1e8), 1.0);
    let zero = hz(0.0);

    let hps = clean_up(zero, s, lam, t, c, zero, 0.0);
    assert!(
        hps.is_nan(),
        "clean_up with beta == lam should be NaN (upstream raises), got {hps}"
    );

    // plate_out, on the same inputs, takes its guard and returns exactly zero —
    // and drops the parent term while doing so.
    assert_eq!(plate_out(zero, s, lam, t, c, zero, 0.0), 0.0);
    assert_eq!(plate_out(zero, s, lam, t, c, zero, 1.23e4), 0.0);
}
