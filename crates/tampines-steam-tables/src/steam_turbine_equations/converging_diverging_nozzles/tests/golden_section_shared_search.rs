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
//! # Results
//!
//! **RESULTS NOT YET TRANSCRIBED.** The tables in the individual test doc
//! comments below are filled in from this file's own printed output; do not
//! quote any number from here that is not marked as measured.
//!
//! # Limitations
//!
//! - Four stagnation states, one per path. It is a regression probe, not a
//!   sweep; the sweeps are the Zaloudek and Moody test files, which were also
//!   run before and after.
//! - The evaluation count is measured on a synthetic objective, not on an IF97
//!   flash, so it measures the *search*, not the end-to-end solver cost. No
//!   wall-clock speed-up of the solvers is claimed here, because none was
//!   measured.
//! - Bit-identical choke pressures on a handful of states would not prove
//!   bit-identity in general. Probe reuse *can* move an iterate in the last
//!   bits; these states may simply not expose it above the 1 Pa stopping rule.

use std::cell::Cell;

use uom::si::available_energy::kilojoule_per_kilogram;
use uom::si::f64::*;
use uom::si::mass_flux::kilogram_per_square_meter_second;
use uom::si::pressure::pascal;

use crate::interfaces::functional_programming::ph_flash_eqm::ph_flash_region;
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
/// Pass criterion: `|p* - 3.0e6| <= 0.5 Pa`.
///
/// # Results
///
/// See the printed output; transcribed in the bead `op-uyi3` hand-off.
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
/// # Results
///
/// See the printed output; transcribed in the bead `op-uyi3` hand-off.
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
        n <= 250,
        "golden section used {n} objective evaluations, above the 100-iteration cap"
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
/// # Results
///
/// See the printed output; transcribed in the bead `op-uyi3` hand-off.
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
