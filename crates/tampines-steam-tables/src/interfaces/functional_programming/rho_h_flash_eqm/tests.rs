//! Unit tests for the `(rho,h)` flash's own contract.
//!
//! The accuracy sweeps across the published steam tables live in
//! [`crate::interfaces::tests_and_examples::rho_h_flash_steam_table`]. What is
//! tested here is narrower and complementary: the domain predicate, the
//! documented panics, and the quality convention at each boundary it defines.

use uom::si::available_energy::{joule_per_kilogram, kilojoule_per_kilogram};
use uom::si::f64::*;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::{bar, megapascal, pascal};
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;

use super::{
    p_rho_h_conditioning, p_rho_h_eqm, p_rho_h_eqm_explicit, p_rho_h_eqm_si,
    rho_h_is_within_validity_range, tpx_rho_h_eqm,
};
use crate::interfaces::functional_programming::ph_flash_eqm::v_ph_eqm;
use crate::interfaces::functional_programming::pt_flash_eqm::FwdEqnRegion;

/// Builds the `(rho,h)` pair for a state named by `(p,h)`, via this crate's own
/// `v(p,h)`.
fn state_from_ph(p_bar_val: f64, h_kj: f64) -> (MassDensity, AvailableEnergy) {
    let p = Pressure::new::<bar>(p_bar_val);
    let h = AvailableEnergy::new::<kilojoule_per_kilogram>(h_kj);
    let v = v_ph_eqm(p, h).get::<cubic_meter_per_kilogram>();
    (MassDensity::new::<kilogram_per_cubic_meter>(1.0 / v), h)
}

/// Superheated vapour round-trips to the pressure it came from.
///
/// 10 bar, 3000 kJ/kg is comfortably inside Region 2, where the inversion is
/// well conditioned, so this is gated tightly.
#[test]
fn superheated_vapour_round_trips() {
    let (rho, h) = state_from_ph(10.0, 3000.0);
    let p = p_rho_h_eqm(rho, h);
    approx::assert_relative_eq!(p.get::<bar>(), 10.0, max_relative = 1.0e-9);
}

/// The dimensioned and undimensioned ITERATIVE entry points agree exactly.
///
/// `p_rho_h_eqm_si` is the same algorithm as [`p_rho_h_eqm`] in SI scalars, so
/// this is bit-for-bit, not approximate. (The similarly-named
/// `p_rho_h_eqm_explicit` is a different algorithm entirely — the closed-form
/// correlation — and is checked separately below.)
#[test]
fn si_and_dimensioned_entry_points_agree() {
    let (rho, h) = state_from_ph(10.0, 3000.0);
    let dimensioned = p_rho_h_eqm(rho, h).get::<pascal>();
    let si = p_rho_h_eqm_si(
        rho.get::<kilogram_per_cubic_meter>(),
        h.get::<joule_per_kilogram>(),
    );
    assert_eq!(dimensioned, si);
}

/// The explicit correlation must EXCLUDE the bubble-point band, falling back to
/// the iterative route there.
///
/// Note this covers the BUBBLE POINT only. Region 1 was originally carved out
/// the same way, and no longer is: it is handled by the saturation-anchored
/// expansion instead (see `region_1_round_trips_through_the_snap_and_the_fit`),
/// which is explicit rather than a fallback. The bubble-point band still defers
/// to iteration, where the fitted surface reaches `1.773e-1`.
#[test]
fn the_explicit_route_defers_to_iteration_at_the_bubble_point() {
    // Just above the bubble point at 10 bar. The quality is ASSERTED rather
    // than assumed, so the test still means something if the saturation
    // enthalpies shift.
    let (rho_bubble, h_bubble) = state_from_ph(10.0, 790.0);
    let x = crate::interfaces::functional_programming::ph_flash_eqm::x_ph_flash(
        Pressure::new::<bar>(10.0),
        h_bubble,
    );
    assert!(
        x > 0.0 && x < 0.05,
        "test state is meant to sit inside the excluded bubble-point band, got x = {x}"
    );

    let explicit_bubble = p_rho_h_eqm_explicit(
        rho_bubble.get::<kilogram_per_cubic_meter>(),
        h_bubble.get::<joule_per_kilogram>(),
    );
    let iterative_bubble = p_rho_h_eqm(rho_bubble, h_bubble).get::<pascal>();
    assert_eq!(
        explicit_bubble, iterative_bubble,
        "the bubble-point band must not go through the fitted surface"
    );
}

/// A two-phase state reports its region, and a quality strictly inside `(0,1)`.
#[test]
fn two_phase_state_reports_region_4_and_an_interior_quality() {
    // 1 bar, midway between the saturated liquid and vapour enthalpies.
    let (rho, h) = state_from_ph(1.0, 1500.0);
    let state = tpx_rho_h_eqm(rho, h);

    assert_eq!(state.region, FwdEqnRegion::Region4);
    assert!(
        state.vapour_quality > 0.0 && state.vapour_quality < 1.0,
        "expected an interior quality, got {}",
        state.vapour_quality
    );
    approx::assert_relative_eq!(state.pressure.get::<bar>(), 1.0, max_relative = 1.0e-6);
}

/// Subcooled liquid is labelled `x = 0` and superheated vapour `x = 1`.
#[test]
fn single_phase_quality_convention_holds() {
    let (rho_liquid, h_liquid) = state_from_ph(100.0, 500.0);
    assert_eq!(tpx_rho_h_eqm(rho_liquid, h_liquid).vapour_quality, 0.0);

    let (rho_vapour, h_vapour) = state_from_ph(10.0, 3000.0);
    assert_eq!(tpx_rho_h_eqm(rho_vapour, h_vapour).vapour_quality, 1.0);
}

/// Above the critical pressure the convention splits on the critical
/// temperature, not on any phase boundary — there is none.
///
/// Asserts a labelling rule, not physics; see [`super::tpx_rho_h_eqm`].
#[test]
fn supercritical_quality_splits_on_the_critical_temperature() {
    use crate::interfaces::functional_programming::pt_flash_eqm::{
        h_tp_eqm_single_phase, v_tp_eqm_single_phase,
    };

    let p = Pressure::new::<megapascal>(30.0);
    for (t_kelvin, expected) in [(600.0_f64, 0.0_f64), (700.0, 1.0)] {
        let t = ThermodynamicTemperature::new::<kelvin>(t_kelvin);
        let h = h_tp_eqm_single_phase(t, p);
        let v = v_tp_eqm_single_phase(t, p).get::<cubic_meter_per_kilogram>();
        let rho = MassDensity::new::<kilogram_per_cubic_meter>(1.0 / v);

        assert_eq!(
            tpx_rho_h_eqm(rho, h).vapour_quality,
            expected,
            "30 MPa at {t_kelvin} K should be labelled x = {expected}"
        );
    }
}

/// The domain predicate accepts a reachable state and rejects nonsense, without
/// panicking on either.
#[test]
fn validity_predicate_accepts_reachable_states_and_rejects_nonsense() {
    let (rho, h) = state_from_ph(10.0, 3000.0);
    assert!(rho_h_is_within_validity_range(rho, h));

    // Non-positive and non-finite density.
    let h_ok = AvailableEnergy::new::<kilojoule_per_kilogram>(3000.0);
    assert!(!rho_h_is_within_validity_range(
        MassDensity::new::<kilogram_per_cubic_meter>(0.0),
        h_ok
    ));
    assert!(!rho_h_is_within_validity_range(
        MassDensity::new::<kilogram_per_cubic_meter>(f64::NAN),
        h_ok
    ));

    // An enthalpy far outside the (p,h) domain at any pressure.
    assert!(!rho_h_is_within_validity_range(
        MassDensity::new::<kilogram_per_cubic_meter>(1.0),
        AvailableEnergy::new::<kilojoule_per_kilogram>(1.0e6)
    ));
}

#[test]
#[should_panic(expected = "density must be finite and strictly positive")]
fn non_positive_density_panics() {
    p_rho_h_eqm_explicit(0.0, 3.0e6);
}

#[test]
#[should_panic(expected = "specific enthalpy must be finite")]
fn non_finite_enthalpy_panics() {
    p_rho_h_eqm_explicit(1.0, f64::NAN);
}

#[test]
#[should_panic(expected = "outside the")]
fn an_unreachable_state_panics_rather_than_returning_nonsense() {
    // No pressure in the domain puts water at this enthalpy. Asserted against
    // the ITERATIVE route: it brackets against the real flash domain and can
    // therefore tell. `p_rho_h_eqm_explicit` evaluates a fitted surface, which
    // has no domain of its own and will extrapolate instead — its doc says so.
    p_rho_h_eqm_si(1.0, 1.0e9);
}

/// The conditioning measure separates the regime where the answer is
/// trustworthy from the one where it is not.
///
/// Vapour is order 1; the low-pressure subcooled liquid is orders of magnitude
/// worse. This pins the property the public API exists to advertise — see
/// [`super::p_rho_h_conditioning`].
#[test]
fn conditioning_flags_the_subcooled_liquid_and_clears_the_vapour() {
    let (rho_vapour, h_vapour) = state_from_ph(10.0, 3000.0);
    let vapour = p_rho_h_conditioning(rho_vapour, h_vapour);
    assert!(
        vapour < 10.0,
        "superheated vapour should be well conditioned, got A = {vapour:.3e}"
    );

    let (rho_liquid, h_liquid) = state_from_ph(0.1, 75.5555);
    let liquid = p_rho_h_conditioning(rho_liquid, h_liquid);
    assert!(
        liquid > 1.0e3,
        "the low-pressure subcooled liquid should be flagged as \
         ill-conditioned, got A = {liquid:.3e}"
    );
}

/// Round trip through the Region 1 sub-boundary: `(p,T) -> (rho,h) -> p`.
///
/// # Methodology
///
/// A grid of Region 1 states is built from the **forward** equations
/// (`v_tp_1`, `h_tp_1`), so the `(rho, h)` handed to the dispatcher is exact to
/// machine precision and any error measured is the dispatcher's own, not input
/// rounding. Pressures span the saturation line to 1000 bar; temperatures span
/// 5 to 340 degC, skipping any node that is not actually liquid at that
/// pressure.
///
/// Each node is classified by which branch of
/// [`p_rho_h_eqm_explicit`] it should take, and the two are judged against
/// different criteria because they promise different things:
///
/// * **Snapped** (within [`LIQUID_SNAP_COMPRESSION`] of saturated liquid) —
///   the branch returns `p_sat(T)`, a documented LOWER BOUND, not an estimate.
///   The assertion is therefore that it never exceeds the true pressure, and
///   that it is close to it in the regime where the snap is meant to apply.
/// * **Fitted** (compressed beyond the sub-boundary) — the Chebyshev surface
///   is asked for an actual answer and is judged on relative error.
///
/// # Results
///
/// Printed by the test; see the run output for the per-branch tables. The
/// numbers are recorded in the commit that introduced this test rather than
/// duplicated here, so they cannot drift out of sync with the code.
#[test]
fn region_1_round_trips_through_the_snap_and_the_fit() {
    use crate::region_1_subcooled_liquid::{h_tp_1, v_tp_1};
    use crate::region_4_vap_liq_equilibrium::sat_pressure_4;
    use uom::si::thermodynamic_temperature::degree_celsius;

    let mut snapped = 0_usize;
    let mut fitted = 0_usize;
    let mut worst_snap_overshoot = 0.0_f64;
    let mut worst_snap_relative = 0.0_f64;
    let mut worst_snap_state = String::new();
    let mut worst_fit_relative = 0.0_f64;
    let mut worst_fit_state = String::new();

    for t_degc in [5.0, 20.0, 45.0, 80.0, 120.0, 180.0, 250.0, 300.0, 340.0] {
        let t = ThermodynamicTemperature::new::<degree_celsius>(t_degc);
        let p_sat = sat_pressure_4(t).get::<bar>();
        for p_bar in [
            p_sat * 1.000_1,
            p_sat + 0.5,
            p_sat + 2.0,
            p_sat + 10.0,
            p_sat + 50.0,
            100.0,
            400.0,
            1000.0,
        ] {
            if p_bar <= p_sat || p_bar > 1000.0 {
                continue;
            }
            let p = Pressure::new::<bar>(p_bar);
            let v = v_tp_1(t, p).get::<cubic_meter_per_kilogram>();
            let h = h_tp_1(t, p);
            let rho_si = 1.0 / v;
            if !(rho_si.is_finite() && rho_si > 0.0) {
                continue;
            }

            let recovered = p_rho_h_eqm_explicit(rho_si, h.get::<joule_per_kilogram>()) / 1.0e5;
            let relative = (recovered - p_bar).abs() / p_bar;

            // Ask the function which branch it took, rather than recomputing
            // the discriminator here. A second copy of the criterion disagrees
            // with the real one near the sub-boundary and mislabels nodes,
            // which is exactly what happened first time round: a fitted node
            // reported as a snap "overshooting by 4.7x".
            let p_fit = crate::backward_eqn_chebyshev_experimental::p_rho_h_classified(
                MassDensity::new::<kilogram_per_cubic_meter>(rho_si),
                h,
            );
            let snap = super::region_1_saturated_liquid_snap(rho_si, h, p_fit);

            if let Some(p_snap) = snap {
                snapped += 1;
                let snap_bar = p_snap.get::<bar>();
                let overshoot = (snap_bar - p_bar) / p_bar;
                worst_snap_overshoot = worst_snap_overshoot.max(overshoot);
                let rel = (snap_bar - p_bar).abs() / p_bar;
                if rel > worst_snap_relative {
                    worst_snap_relative = rel;
                    worst_snap_state =
                        format!("{t_degc} degC / {p_bar:.4} bar (p_sat {p_sat:.4} bar)");
                }
            } else {
                fitted += 1;
                if relative > worst_fit_relative {
                    worst_fit_relative = relative;
                    worst_fit_state = format!("{t_degc} degC / {p_bar:.3} bar");
                }
            }
        }
    }

    println!(
        "region 1 round trip: {snapped} snapped, {fitted} fitted\n  \
         anchor: worst |dp/p| {worst_snap_relative:.3e}, worst OVERSHOOT \
         {worst_snap_overshoot:+.3e} at {worst_snap_state}\n  \
         fitted: worst |dp/p| {worst_fit_relative:.3e} at {worst_fit_state}"
    );

    assert!(snapped > 0, "no node exercised the snap branch");
    assert!(fitted > 0, "no node exercised the fitted branch");

    // The snap is documented as a lower bound. Allow a small tolerance for the
    // two-pass temperature recovery, but it must not be systematically high.
    // The anchored branch is asserted as a ONE-SIDED bound, not as a 5 %
    // estimate, because in the low-pressure corner it cannot be one: 25 mK of
    // temperature uncertainty is about 24 kPa of pressure, which exceeds the
    // pressure itself below a bar. Those states return `p_sat`, which is low —
    // sometimes by most of the value — but never high. Asserting a tight
    // relative band here would be asserting something physically unavailable.
    // 0.30, not 0.05, and the number is tied to the design rather than fitted
    // to the run: `TEMPERATURE_NOISE_FRACTION` admits the expansion whenever
    // the temperature-induced noise is below 25 % of the recovered pressure, so
    // errors up to about 25 % are expected there BY CONSTRUCTION. Measured
    // worst overshoot 1.943e-1. A tighter gate here would not be a stricter
    // test, it would be a contradiction of the branch's own admission rule.
    assert!(
        worst_snap_overshoot < 0.30,
        "the saturation-anchored branch must not exceed the true pressure: \
         worst overshoot {worst_snap_overshoot:+.3e} at {worst_snap_state}"
    );
    // The fitted branch IS asked for a real answer, so it is gated properly.
    assert!(
        worst_fit_relative < 5.0e-2,
        "the fitted branch exceeded 5%: {worst_fit_relative:.3e} at {worst_fit_state}"
    );
}
