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
//! ## Results (2026-09-15, upstream `de374c8`, 5 699 cases in 32 groups, all passing)
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
//! floor. 169 of 5 699 cases carry `cond > 1`; the rest are asserted at full
//! tightness.
//!
//! **The suite is not vacuous**, verified by mutation in two rounds — the
//! second aimed specifically at the ill-conditioned groups, to confirm the
//! widened bound masks nothing. Perturbing the `rf_graph` numerator, the
//! transient breakthrough time-lag, the trapezoid weight in `integrate` and the
//! noble-gas `k_plate` zeroing in `normal_operation_node` failed exactly the
//! groups that read them — `release_fraction.kernel` included, despite its 16
//! widened cases — and left every unrelated test green. Details and the
//! first-round table are in the doc above.
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
//! **This is verification, not validation.** It establishes that the Rust port
//! computes what the upstream Python computes. It says nothing about whether
//! either matches measured fission-product release — that needs a public
//! benchmark and is not claimed here.

use boon_lay::prelude::*;
use boon_lay::triso_atops_fork::release_models::steady_state::{
    attenuation_factor, booth_longlived, booth_shortlived_fast_diffuse, breakthrough_model,
    rb_fail_noble_gases,
};
use boon_lay::triso_atops_fork::release_models::transient::{
    booth_transient, breakthrough_model_transient, rf_graph,
};
use boon_lay::triso_atops_fork::normal_operation::NodalActivities;

use uom::si::area::square_meter;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::{
    Area, DiffusionCoefficient, Frequency, Length, Ratio, ThermodynamicTemperature, Time,
};
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
