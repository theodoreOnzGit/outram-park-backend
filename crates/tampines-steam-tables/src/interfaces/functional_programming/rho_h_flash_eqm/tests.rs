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

/// The explicit correlation must EXCLUDE compressed liquid and the bubble
/// point, falling back to the iterative route there.
///
/// This is the carve-out that stops `p_rho_h_eqm_explicit` returning the fitted
/// surface's worst numbers — measured at `3.185` relative in Region 1 (a factor
/// of four) and `1.773e-1` within 5 % quality of the bubble point. In those two
/// regimes the explicit entry point must agree with the accurate one EXACTLY,
/// because it is literally calling it.
#[test]
fn the_explicit_route_defers_to_iteration_in_liquid_and_at_the_bubble_point() {
    // Compressed liquid: 100 bar, 300 kJ/kg (about 70 degC, well subcooled).
    let (rho_liquid, h_liquid) = state_from_ph(100.0, 300.0);
    let explicit = p_rho_h_eqm_explicit(
        rho_liquid.get::<kilogram_per_cubic_meter>(),
        h_liquid.get::<joule_per_kilogram>(),
    );
    let iterative = p_rho_h_eqm(rho_liquid, h_liquid).get::<pascal>();
    assert_eq!(
        explicit, iterative,
        "compressed liquid must not go through the fitted surface"
    );

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
