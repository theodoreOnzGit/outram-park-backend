//! Verification of the crate's **single** golden-section maximiser,
//! [`golden_section_max_g`], and of the choke pressures the four HEM
//! choked-flow code paths that use it produce.
//!
//! # Why this file exists
//!
//! Bead `op-uyi3` recorded that this crate contained **four** copies of the same
//! golden-section loop — the factored-out `golden_section_max_g`, plus three
//! inline rewrites in `get_critical_pressure_and_mass_flux_subcooled_liquid_ph`,
//! `get_critical_pressure_and_mass_flux_ph_vle_dome` and
//! `dome_crossing_interior_choke` — and that **all four evaluated the objective
//! twice per iteration**, contradicting the defining property of golden section
//! (and contradicting the in-dome copy's own doc comment, which claimed one
//! evaluation per iteration while the code below it did two).
//!
//! The objective is an IAPWS-IF97 `(p,s)` flash, so the wasted evaluations are
//! not free. The three inline copies were deleted in favour of calls to
//! `golden_section_max_g`, and `golden_section_max_g` was rewritten to reuse the
//! retained probe.
//!
//! **Probe reuse is not bit-identical to recompute-both.** The retained probe
//! keeps its original abscissa rather than being recomputed as
//! `a + gr*(b - a)` on the contracted bracket, and those two are not the same
//! `f64`. This file is the measurement that decides whether that matters here,
//! so the claim rests on printed numbers rather than on an argument.
//!
//! # Methodology
//!
//! Three independent checks, all in this file:
//!
//! 1. [`golden_section_locates_a_known_maximum`] — hands
//!    `golden_section_max_g` a synthetic objective with an analytically known
//!    maximum on a pressure-scale bracket, and asserts the located abscissa
//!    against it. This verifies the search itself without any IF97 dependence.
//! 2. [`golden_section_costs_one_evaluation_per_iteration`] — wraps that same
//!    objective in a call counter and prints the total number of objective
//!    evaluations for a fixed bracket. Golden section's contraction constant
//!    makes one probe of the contracted bracket coincide with a probe of the
//!    previous one, so a converged search should cost `k + 3` evaluations for
//!    `k` iterations (two initial probes, one per iteration, one final
//!    evaluation at the returned midpoint), not `2k + 1`.
//! 3. [`choke_pressures_of_the_four_solver_paths`] — prints the choke pressure
//!    and mass flux to full `f64` precision for one stagnation state on each of
//!    the four code paths that used to carry a private copy of the loop, so a
//!    before/after diff of the printed values shows exactly how far any choke
//!    pressure moved. It asserts only that the results are finite and inside the
//!    bracket the solver searched; the quantitative gates live in the Marviken,
//!    Zaloudek and Moody test files.
//!
//! There is a fourth check,
//! [`probe_reuse_moves_the_choke_far_below_the_stopping_rule`], which is the one
//! that actually settles the question: it keeps a copy of the **retired**
//! recompute-both loop as an oracle and runs both forms side by side over 158
//! real IF97 searches, so the difference between them is a measured number
//! rather than a before/after diff of two separate runs.
//!
//! # Results (all measured 2026-08-13, release, this machine)
//!
//! ## Objective evaluations
//!
//! Printed by [`golden_section_costs_one_evaluation_per_iteration`] on the
//! bracket `[1.0e4, 1.0e7]` Pa with the 1 Pa stopping rule:
//!
//! | Form | Objective evaluations |
//! |---|---|
//! | recompute-both (before) | **69** |
//! | probe reuse (after) | **37** |
//!
//! a **1.86x** reduction, with both forms locating
//! `p* = 3000000.2020218512 Pa` — the same `f64`.
//!
//! ## Choke pressures on the four production paths
//!
//! Printed by [`choke_pressures_of_the_four_solver_paths`], before and after the
//! de-duplication and probe-reuse rewrite:
//!
//! | Path | Region | `p_crit` before [Pa] | `p_crit` after [Pa] | move [Pa] |
//! |---|---|---|---|---|
//! | in-dome | 4 | 3034765.9167458080 | 3034765.9167458080 | 0 |
//! | subcooled liquid | 1 | 185900.6056136014 | 185900.6056136014 | 0 |
//! | superheated vapour | 2 | 2736915.5240636854 | 2736915.5240636864 | 1e-9 |
//! | supercritical | 3 | 16165453.1841280889 | 16165453.1841280889 | 0 |
//!
//! Mass fluxes: `9283.2727450284`, `95150.6196490945` and `53802.7623693730`
//! kg/(m^2 s) unchanged; the superheated one moved from `5943.6212852178` to
//! `5943.6212852177`, i.e. in the last digit printed.
//!
//! ## Probe reuse against the retired form, on the real objective
//!
//! Printed by [`probe_reuse_moves_the_choke_far_below_the_stopping_rule`] over
//! 158 searches spanning all three bracket shapes (8 candidate states skipped as
//! landing in the wrong IF97 region or below the 273.15 K floor):
//!
//! | Quantity | Worst |
//! |---|---|
//! | `abs(dp_crit)` | **5.990479e-1 Pa** — against a 1 Pa stopping rule |
//! | `abs(dp_crit)/p` | **4.604111e-8** |
//! | `abs(dG_crit)/G` | **3.429708e-13** |
//!
//! worst case in-dome at `p0 = 20000000.0 Pa`, `h0 = 2119.244 kJ/kg`.
//!
//! ## The V&V gates
//!
//! | Suite | Before | After | Output |
//! |---|---|---|---|
//! | Marviken | 6 passed | 6 passed | byte-identical |
//! | Moody | 13 passed | 13 passed | byte-identical |
//! | Zaloudek | 88 passed, 1 ignored | 88 passed, 1 ignored | 14 lines of 6993 changed |
//!
//! Every one of those 14 Zaloudek lines is a move in `p_crit_calc` alone — the
//! printed `G_calc` is identical in all 14 — and the largest is **3 Pa** on
//! `10356.041 kPa` (`2.9e-7` relative); the other 13 are 1 Pa. Zaloudek's
//! choke-pressure tolerance is `0.005` relative, so the worst move is ~4 orders
//! of magnitude inside it. Marviken's headline numbers were unchanged: test 23
//! `mean|dev| = 12.6 %`, `max|dev| = 23.1 %` (n = 29); test 24
//! `mean|dev| = 48.6 %`, `max|dev| = 70.2 %` (n = 40, 31 outside the band) —
//! test 24 remains **NOT validated**, exactly as before, and this change neither
//! improved nor worsened it.
//!
//! # Limitations
//!
//! - **One machine, one run each.** Nothing here is averaged over repeats, and
//!   no wall-clock timing is reported at all, because none was measured. The
//!   1.86x figure is an *evaluation count*, not a speed-up: no end-to-end solver
//!   benchmark was run, so this file makes no claim about how much faster the
//!   choked-flow solvers got.
//! - The evaluation count is measured on a synthetic Gaussian objective, not on
//!   an IF97 flash, so it measures the *search*, not the cost of the flash.
//! - [`choke_pressures_of_the_four_solver_paths`] is four stagnation states, one
//!   per path — a regression probe, not a sweep. The sweeps are the Zaloudek
//!   (88 tests) and Moody (13 isobars) files.
//! - The oracle sweep covers the three *bracket shapes*, not the surrounding
//!   solver logic. In particular it does not exercise the near-saturation
//!   quality discriminator, the deep-subcooling escape, or the 1500-point coarse
//!   scan in `dome_crossing_interior_choke` — those are covered only indirectly,
//!   by the Zaloudek and Moody suites being unchanged.
//! - `p_crit` moving by up to 3 Pa is **not** proof that no downstream consumer
//!   cares. It is proof that the V&V gates in this crate do not, at their
//!   current tolerances.

use std::cell::Cell;

use uom::ConstZero;
use uom::si::available_energy::kilojoule_per_kilogram;
use uom::si::f64::*;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::pressure::{megapascal, pascal};

use crate::interfaces::functional_programming::ph_flash_eqm::ph_flash_region;
use crate::prelude::functional_programming::ph_flash_eqm::s_ph_eqm;
use crate::steam_turbine_equations::choked_flow::bubble_point_pressure_from_entropy;
use crate::steam_turbine_equations::choked_flow::dew_point_pressure_from_entropy;
use crate::prelude::functional_programming::ps_flash_eqm::h_ps_eqm;
use crate::prelude::functional_programming::ps_flash_eqm::v_ps_eqm;
use crate::prelude::functional_programming::pt_flash_eqm::FwdEqnRegion;
use crate::steam_turbine_equations::choked_flow::get_critical_pressure_and_mass_flux_multiphase_ph;
use crate::steam_turbine_equations::choked_flow::golden_section_max_g;

/// A synthetic, strictly unimodal objective on the pressure axis with an
/// analytically known maximum at [`GAUSSIAN_P_STAR_PA`].
///
/// `G(p) = G_MAX * exp(-((p - p*)/w)^2)`, a Gaussian in pressure. It stands in
/// for the HEM energy-balance mass flux `G(p) = rho(p,s0)*sqrt(2*(h0 - h(p,s0)))`
/// — same shape (one interior peak, smooth, quadratic at the top) without the
/// IF97 dependence, so a failure here is a failure of the search and nothing
/// else.
///
/// # Units
///
/// `p_pa` is a pressure in pascals; the return is a `uom` [`MassFlux`] in
/// kg/(m^2 s).
fn gaussian_mass_flux(p_pa: f64) -> MassFlux {
    const WIDTH_PA: f64 = 1.5e6;
    const G_MAX: f64 = 12_000.0;
    let z = (p_pa - GAUSSIAN_P_STAR_PA) / WIDTH_PA;
    MassFlux::new::<kilogram_per_square_meter_second>(G_MAX * (-z * z).exp())
}

/// Analytic maximum of [`gaussian_mass_flux`], in pascals.
const GAUSSIAN_P_STAR_PA: f64 = 3.0e6;

/// The shared golden-section maximiser locates a known maximum.
///
/// # Methodology
///
/// Maximise [`gaussian_mass_flux`] over `[1.0e4, 1.0e7]` Pa — a bracket much
/// wider than the peak's own width, with the peak well inside it. The solver's
/// stopping rule is a 1 Pa bracket width, so the returned midpoint must lie
/// within 0.5 Pa of the analytic maximum for a unimodal objective.
///
/// Pass criterion: `abs(p* - 3.0e6) <= 0.5 Pa`.
///
/// # Results (measured 2026-08-13, release)
///
/// Located `p* = 3000000.2020218512 Pa` against the analytic `3000000.0000000000
/// Pa`, an error of `0.2020218512 Pa`, with
/// `G(p*) = 11999.9999999998 kg/(m^2 s)` against the analytic `12000.0`.
/// **Identical, to the last bit, before and after the probe-reuse rewrite.**
#[test]
fn golden_section_locates_a_known_maximum() {
    let (p_star, g_star) = golden_section_max_g(gaussian_mass_flux, 1.0e4, 1.0e7);
    let p_star_pa = p_star.get::<pascal>();
    let err_pa = (p_star_pa - GAUSSIAN_P_STAR_PA).abs();
    println!("golden_section_max_g on a Gaussian objective:");
    println!("   located p*  = {p_star_pa:.10} Pa");
    println!("   analytic p* = {GAUSSIAN_P_STAR_PA:.10} Pa");
    println!("   |error|     = {err_pa:.10} Pa");
    println!(
        "   G(p*)       = {:.10} kg/(m^2 s)",
        g_star.get::<kilogram_per_square_meter_second>()
    );
    assert!(
        err_pa <= 0.5,
        "golden section missed the analytic maximum by {err_pa} Pa, \
         which is more than half the 1 Pa stopping bracket"
    );
}

/// The shared golden-section maximiser costs about one objective evaluation per
/// iteration, not two.
///
/// # Methodology
///
/// Wrap [`gaussian_mass_flux`] in a [`Cell`] counter and run the same search as
/// [`golden_section_locates_a_known_maximum`]. Count every call. Recompute-both
/// costs `2k + 1` evaluations for `k` iterations; probe reuse costs `k + 3`.
/// The exact counts are printed rather than predicted.
///
/// # Results (measured 2026-08-13, release)
///
/// | Form | Evaluations |
/// |---|---|
/// | recompute-both (before the `op-uyi3` fix) | **69** |
/// | probe reuse (after) | **37** |
///
/// a **1.86x** reduction, with both forms locating the same
/// `p* = 3000000.2020218512 Pa`. `37 = k + 3` gives `k = 34` iterations, which
/// matches `ln(1/9.99e6)/ln(0.618) = 33.4` rounded up — so the loop really is
/// doing one evaluation per iteration and not merely fewer of them.
///
/// # Pass criterion
///
/// `<= 45` evaluations. That is comfortably above the measured 37 and
/// comfortably below the 69 of the form this replaced, so the test fails if the
/// probe reuse is ever undone — which is its whole purpose. It is deliberately
/// not an equality assertion, so that a change to the stopping rule does not
/// look like a regression in the evaluation *strategy*.
#[test]
fn golden_section_costs_one_evaluation_per_iteration() {
    let calls = Cell::new(0_u32);
    let counted = |p_pa: f64| {
        calls.set(calls.get() + 1);
        gaussian_mass_flux(p_pa)
    };
    let (p_star, _g_star) = golden_section_max_g(counted, 1.0e4, 1.0e7);
    let n = calls.get();
    println!(
        "GOLDEN_SECTION_EVALS on [1.0e4, 1.0e7] Pa: {n} (located p* = {:.10} Pa)",
        p_star.get::<pascal>()
    );
    assert!(
        n <= 45,
        "golden section used {n} objective evaluations; the probe-reuse form measured \
         37 and the recompute-both form it replaced measured 69, so this looks like a \
         regression to evaluating both interior probes every iteration"
    );
}

/// Choke pressure and mass flux on each of the four code paths that used to
/// carry a private copy of the golden-section loop.
///
/// # Methodology
///
/// One stagnation state per path, dispatched through the public
/// [`get_critical_pressure_and_mass_flux_multiphase_ph`] so the routing is the
/// production routing:
///
/// | Path | `(p0, h0)` |
/// |---|---|
/// | in-dome | 5 MPa, 2000 kJ/kg |
/// | subcooled liquid | 5 MPa, 500 kJ/kg |
/// | superheated vapour | 5 MPa, 3300 kJ/kg |
/// | supercritical | 23 MPa, 1900 kJ/kg |
///
/// The IF97 region each state actually lands in is printed rather than assumed.
/// Every result is printed to full `f64` precision. The point of the test is the
/// printed values, which are diffed across a change to the search; the
/// assertions are sanity only (finite, positive, at or below the stagnation
/// pressure). The quantitative V&V gates for these paths are the Marviken,
/// Zaloudek and Moody test files.
///
/// # Results (measured 2026-08-13, release)
///
/// | Path | Region | `p_crit` [Pa] | `G_crit` [kg/(m^2 s)] |
/// |---|---|---|---|
/// | in-dome | 4 | 3034765.9167458080 | 9283.2727450284 |
/// | subcooled liquid | 1 | 185900.6056136014 | 95150.6196490945 |
/// | superheated vapour | 2 | 2736915.5240636864 | 5943.6212852177 |
/// | supercritical | 3 | 16165453.1841280889 | 53802.7623693730 |
///
/// Before the `op-uyi3` de-duplication and probe-reuse rewrite, six of these
/// eight numbers were **bit-identical** to the values above. The two that moved
/// are both on the superheated-vapour path: `p_crit` was
/// `2736915.5240636854 Pa` (a `1e-9 Pa` move, `3.7e-16` relative) and `G_crit`
/// was `5943.6212852178` (a move in the last digit printed). See the module doc
/// for the full before/after table.
#[test]
fn choke_pressures_of_the_four_solver_paths() {
    let cases: [(&str, f64, f64); 4] = [
        ("in-dome           ", 5.0e6, 2000.0),
        ("subcooled-liquid  ", 5.0e6, 500.0),
        ("superheated-vapour", 5.0e6, 3300.0),
        ("supercritical     ", 23.0e6, 1900.0),
    ];

    println!("\n--- CHOKE_PROBE: choke pressures, four HEM solver paths ---");
    for (label, p0_pa, h0_kj_per_kg) in cases {
        let p0 = Pressure::new::<pascal>(p0_pa);
        let h0 = AvailableEnergy::new::<kilojoule_per_kilogram>(h0_kj_per_kg);
        let region = ph_flash_region(p0, h0);
        let (p_crit, g_crit) = get_critical_pressure_and_mass_flux_multiphase_ph(p0, h0);
        let p_crit_pa = p_crit.get::<pascal>();
        let g_crit_si = g_crit.get::<kilogram_per_square_meter_second>();
        println!(
            "CHOKE_PROBE {label} region={region:?} p0={p0_pa:.1} \
             p_crit={p_crit_pa:.10} G_crit={g_crit_si:.10}"
        );

        assert!(
            p_crit_pa.is_finite() && g_crit_si.is_finite(),
            "{label}: non-finite choke ({p_crit_pa}, {g_crit_si})"
        );
        assert!(
            p_crit_pa > 0.0 && p_crit_pa <= p0_pa,
            "{label}: choke pressure {p_crit_pa} Pa is outside (0, p0 = {p0_pa}]"
        );
        assert!(
            g_crit_si > 0.0,
            "{label}: non-positive critical mass flux {g_crit_si}"
        );
    }
}

/// The **retired** recompute-both golden-section loop, kept here as a numerical
/// oracle and nowhere else.
///
/// This is a byte-for-byte copy of the loop that `golden_section_max_g` carried
/// before bead `op-uyi3`, and of the three inline copies that were deleted with
/// it: it recomputes *both* interior probes on every iteration instead of
/// carrying the retained one across. It exists solely so
/// [`probe_reuse_moves_the_choke_far_below_the_stopping_rule`] can measure the
/// difference between the two forms directly, on the real IAPWS-IF97 objective,
/// instead of inferring it from a rounded printout.
///
/// # This is not a fifth copy — do not call it from `src/`
///
/// It is `#[cfg(test)]`-only by virtue of living in this module, it is private,
/// and its single caller is the comparison test below. The production path has
/// exactly one golden-section implementation,
/// [`golden_section_max_g`]. If you find yourself wanting this function outside
/// this file, you want `golden_section_max_g`.
///
/// # Units
///
/// `a_pa`, `b_pa` are pressures in pascals; the return is
/// `(Pressure, MassFlux)`.
fn retired_recompute_both_golden_section(
    g_of_p: impl Fn(f64) -> MassFlux,
    a_pa: f64,
    b_pa: f64,
) -> (Pressure, MassFlux) {
    let gr = (5.0_f64.sqrt() - 1.0) / 2.0; // 0.618...
    let mut a = a_pa;
    let mut b = b_pa;
    let mut c = b - gr * (b - a);
    let mut d = a + gr * (b - a);
    for _ in 0..100 {
        if (b - a).abs() < 1.0 {
            break;
        } // 1 Pa bracket width
        let gc = g_of_p(c).get::<kilogram_per_square_meter_second>();
        let gd = g_of_p(d).get::<kilogram_per_square_meter_second>();
        if gc > gd {
            b = d; // peak is in [a, d]
        } else {
            a = c; // peak is in [c, b]
        }
        c = b - gr * (b - a);
        d = a + gr * (b - a);
    }
    let p_star = Pressure::new::<pascal>(0.5 * (a + b));
    (p_star, g_of_p(p_star.get::<pascal>()))
}

/// Probe reuse moves the located choke pressure by less than the search's own
/// stopping-bracket width, measured directly against the retired form on the
/// real HEM objective.
///
/// This is the measurement that decides whether the `op-uyi3` probe-reuse
/// rewrite was safe, because probe reuse is *not* bit-identical to
/// recompute-both: the retained probe keeps its original abscissa, and
/// recomputing `a + gr*(b - a)` on the contracted bracket gives a different
/// `f64` in the last bits. Running the two forms side by side in one process
/// measures that difference; diffing two separate test runs only shows whether
/// it crossed a print-rounding boundary.
///
/// # Methodology
///
/// All three HEM choked-flow solvers maximise the same energy-balance objective
/// along the isentrope through the stagnation state,
///
/// ```text
/// G(p) = rho(p, s0) * sqrt( 2 * (h0 - h(p, s0)) )
/// ```
///
/// and differ only in the bracket they hand the search. This test rebuilds that
/// objective and runs **both** forms — the shipped [`golden_section_max_g`] with
/// probe reuse, and [`retired_recompute_both_golden_section`], the form it
/// replaced — over every bracket shape the crate uses:
///
/// | Stagnation family | Bracket | Solver it stands for |
/// |---|---|---|
/// | in-dome (Region 4) | `[p_min, p0]` | `get_critical_pressure_and_mass_flux_ph_vle_dome` |
/// | subcooled (Region 1) | `[p_min, p_bubble]` | `get_critical_pressure_and_mass_flux_subcooled_liquid_ph` |
/// | superheated (Region 2) | `[p_dew, p0]` | vapour sonic stretch of `..._superheated_vapour_ph` |
/// | superheated (Region 2) | `[p_min, p_dew]` | condensing stretch of the same solver |
///
/// with `p0` log-spaced from 1 MPa to 20 MPa (12 values) and, at each `p0`, `h0`
/// placed by quality relative to the saturation enthalpies: 5 fractions inside
/// the dome (`x0` = 0.15 … 0.85), 3 subcooled (`h_f - 0.05/0.20/0.50 * h_fg`)
/// and 3 superheated (`h_g + 0.05/0.20/0.50 * h_fg`). States that do not land in
/// the intended IF97 region, or that fall below the 273.15 K floor where
/// `ph_flash_region` refuses to answer, are skipped and counted rather than
/// forced.
///
/// The test reports the **worst** absolute and relative deviation in the located
/// choke pressure and in the mass flux there, over every search.
///
/// # Pass criteria
///
/// * worst `abs(dp_crit)/p < 1e-5` — 500x tighter than the tightest V&V gate
///   this search feeds (Zaloudek's `0.005` relative choke pressure), and ~200x
///   above the measured worst, so it is a regression detector rather than a
///   pinned value.
/// * worst `abs(dG_crit)/G < 1e-9` — near a maximum `dG/dp` is ~0, so a visible
///   move in `G` would mean the two forms landed in *different basins*, which is
///   a correctness failure rather than a rounding difference.
///
/// **Neither bound may be widened to make a future change pass.** Widening the
/// first past `0.005` would mean the search disagrees with itself by more than
/// the V&V tolerance it feeds.
///
/// # Results (measured 2026-08-13, release)
///
/// 158 searches compared, 8 candidate states skipped.
///
/// | Quantity | Worst |
/// |---|---|
/// | `abs(dp_crit)` | **5.990479e-1 Pa** (the stopping rule is 1 Pa) |
/// | `abs(dp_crit)/p` | **4.604111e-8** |
/// | `abs(dG_crit)/G` | **3.429708e-13** |
///
/// worst case in-dome at `p0 = 20000000.0 Pa`, `h0 = 2119.244 kJ/kg`.
///
/// So the two forms agree to within the resolution the stopping rule gives the
/// answer in the first place — but they are *not* identical, and the module doc
/// records the one place a larger move showed up in production: 3 Pa on a
/// 10.356 MPa Zaloudek choke, still `2.9e-7` relative.
///
/// # Limitations
///
/// This sweeps the *search*, not the solvers around it: the near-saturation
/// quality discriminator, the deep-subcooling escape and the 1500-point coarse
/// scan of `dome_crossing_interior_choke` are not exercised here. Their
/// insensitivity is evidenced only indirectly, by the Marviken/Moody outputs
/// being byte-identical and Zaloudek moving in 14 lines of 6993.
#[test]
fn probe_reuse_moves_the_choke_far_below_the_stopping_rule() {
    let p_min = Pressure::new::<megapascal>(0.000_611_212_677 * 1.01);

    let mut n_cases = 0_u32;
    let mut n_skipped = 0_u32;
    let mut worst_dp_pa = 0.0_f64;
    let mut worst_dp_rel = 0.0_f64;
    let mut worst_dg_rel = 0.0_f64;

    /// Lowest stagnation enthalpy this sweep will hand to an IF97 flash.
    ///
    /// `ph_flash_region` panics with "p,h point below 273.15K" below the IF97
    /// lower temperature limit, and the deepest subcooling steps below reach it
    /// at low `p0` where `h_fg` is large. Skipping such states keeps the sweep
    /// inside the formulation's validity range instead of catching a panic.
    ///
    /// Units: J/kg. 100 kJ/kg is roughly `h_f` at 297 K, comfortably above the
    /// 273.15 K floor.
    const H0_FLOOR_J_PER_KG: f64 = 100.0e3;

    let compare = |label: &str,
                   p0_pa: f64,
                   h0: AvailableEnergy,
                   lo_pa: f64,
                   hi_pa: f64,
                   n_cases: &mut u32,
                   worst_dp_pa: &mut f64,
                   worst_dp_rel: &mut f64,
                   worst_dg_rel: &mut f64,
                   worst_dp_state: &mut (String, f64, f64)| {
        if hi_pa - lo_pa <= 1.0 {
            return;
        }
        let s0 = s_ph_eqm(Pressure::new::<pascal>(p0_pa), h0);
        let g_of_p = |p_pa: f64| -> MassFlux {
            let p = Pressure::new::<pascal>(p_pa);
            let h = h_ps_eqm(p, s0);
            let ke = h0 - h;
            if ke < AvailableEnergy::ZERO {
                return MassFlux::ZERO;
            }
            let rho = v_ps_eqm(p, s0).recip();
            rho * (2.0 * ke).sqrt()
        };

        let (p_new, g_new) = golden_section_max_g(&g_of_p, lo_pa, hi_pa);
        let (p_old, g_old) = retired_recompute_both_golden_section(&g_of_p, lo_pa, hi_pa);

        let dp_pa = (p_new.get::<pascal>() - p_old.get::<pascal>()).abs();
        let dp_rel = dp_pa / p_old.get::<pascal>();
        let g_old_si = g_old.get::<kilogram_per_square_meter_second>();
        let dg_rel = if g_old_si.abs() > 0.0 {
            (g_new.get::<kilogram_per_square_meter_second>() - g_old_si).abs() / g_old_si.abs()
        } else {
            0.0
        };

        *n_cases += 1;
        if dp_pa > *worst_dp_pa {
            *worst_dp_pa = dp_pa;
            *worst_dp_rel = dp_rel;
            *worst_dp_state = (label.to_string(), p0_pa, h0.get::<kilojoule_per_kilogram>());
        }
        *worst_dg_rel = worst_dg_rel.max(dg_rel);
    };

    let mut worst_dp_state = (String::from("none"), 0.0_f64, 0.0_f64);

    for i in 0..12 {
        let f = i as f64 / 11.0;
        let p0_pa = 1.0e6 * (20.0_f64).powf(f); // 1 MPa .. 20 MPa, log-spaced
        let p0 = Pressure::new::<pascal>(p0_pa);

        // saturation enthalpies at p0, to place h0 by quality relative to the dome
        let tsat = crate::region_4_vap_liq_equilibrium::sat_temp_4(p0);
        let h_f =
            crate::prelude::functional_programming::pt_flash_eqm::h_tp_eqm_two_phase(tsat, p0, 0.0);
        let h_g =
            crate::prelude::functional_programming::pt_flash_eqm::h_tp_eqm_two_phase(tsat, p0, 1.0);
        let h_fg = h_g - h_f;

        // ── in-dome (Region 4): bracket [p_min, p0] ─────────────────────────
        for x0 in [0.15_f64, 0.30, 0.50, 0.70, 0.85] {
            let h0 = h_f + h_fg * x0;
            if ph_flash_region(p0, h0) != FwdEqnRegion::Region4 {
                n_skipped += 1;
                continue;
            }
            compare(
                "in-dome",
                p0_pa,
                h0,
                p_min.get::<pascal>(),
                p0_pa,
                &mut n_cases,
                &mut worst_dp_pa,
                &mut worst_dp_rel,
                &mut worst_dg_rel,
                &mut worst_dp_state,
            );
        }

        // ── subcooled liquid (Region 1): bracket [p_min, p_bubble] ──────────
        for sub in [0.05_f64, 0.20, 0.50] {
            let h0 = h_f - h_fg * sub;
            if h0.get::<uom::si::available_energy::joule_per_kilogram>() < H0_FLOOR_J_PER_KG {
                n_skipped += 1;
                continue;
            }
            if ph_flash_region(p0, h0) != FwdEqnRegion::Region1 {
                n_skipped += 1;
                continue;
            }
            let s0 = s_ph_eqm(p0, h0);
            let p_bubble = bubble_point_pressure_from_entropy(s0);
            compare(
                "subcooled",
                p0_pa,
                h0,
                p_min.get::<pascal>(),
                p_bubble.get::<pascal>(),
                &mut n_cases,
                &mut worst_dp_pa,
                &mut worst_dp_rel,
                &mut worst_dg_rel,
                &mut worst_dp_state,
            );
        }

        // ── superheated vapour (Region 2): both brackets the solver uses,
        //    the single-phase [p_dew, p0] and the condensing [p_min, p_dew] ──
        for sup in [0.05_f64, 0.20, 0.50] {
            let h0 = h_g + h_fg * sup;
            if ph_flash_region(p0, h0) != FwdEqnRegion::Region2 {
                n_skipped += 1;
                continue;
            }
            let s0 = s_ph_eqm(p0, h0);
            let p_dew = dew_point_pressure_from_entropy(s0);
            compare(
                "superheated-vapour-stretch",
                p0_pa,
                h0,
                p_dew.get::<pascal>(),
                p0_pa,
                &mut n_cases,
                &mut worst_dp_pa,
                &mut worst_dp_rel,
                &mut worst_dg_rel,
                &mut worst_dp_state,
            );
            compare(
                "superheated-condensing-stretch",
                p0_pa,
                h0,
                p_min.get::<pascal>(),
                p_dew.get::<pascal>(),
                &mut n_cases,
                &mut worst_dp_pa,
                &mut worst_dp_rel,
                &mut worst_dg_rel,
                &mut worst_dp_state,
            );
        }
    }

    println!("\n--- PROBE_REUSE_ORACLE: probe reuse vs the retired recompute-both form ---");
    println!("  searches compared: {n_cases} ({n_skipped} states skipped, wrong IF97 region)");
    println!("  worst |dp_crit|      = {worst_dp_pa:.6e} Pa  (stopping rule is 1 Pa)");
    println!("  worst |dp_crit|/p    = {worst_dp_rel:.6e}");
    println!("  worst |dG_crit|/G    = {worst_dg_rel:.6e}");
    println!(
        "  worst case: {} at p0 = {:.1} Pa, h0 = {:.3} kJ/kg",
        worst_dp_state.0, worst_dp_state.1, worst_dp_state.2
    );

    assert!(
        n_cases >= 80,
        "expected at least 80 comparison searches across the three bracket shapes, got {n_cases}"
    );
    assert!(
        worst_dp_rel < 1.0e-5,
        "probe reuse moved the located choke pressure by a relative {worst_dp_rel} \
         ({worst_dp_pa} Pa). The bound is 1e-5, which is still 500x tighter than the \
         tightest V&V gate this search feeds (Zaloudek's 0.005 relative choke pressure). \
         Do not widen it -- re-examine the rewrite."
    );
    assert!(
        worst_dg_rel < 1.0e-9,
        "probe reuse moved the critical mass flux by a relative {worst_dg_rel}; \
         near a maximum dG/dp is ~0, so any visible move in G means the two forms \
         landed in different basins, not merely different last bits"
    );
}
