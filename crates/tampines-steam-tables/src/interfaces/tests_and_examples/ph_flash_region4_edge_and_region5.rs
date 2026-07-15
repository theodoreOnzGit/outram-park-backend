//! Verification & regression tests for two `(p,h)`-flash edge cases that used
//! to `todo!()`-panic:
//!
//! 1. **The `p == p_sat(273.15 K)` trap (triple-point pressure edge).**
//! 2. **Region 5 `(p,h)` — deliberately unsupported (no IAPWS-IF97 backward eqn).**
//!
//! Plus a guard test that the single-phase `(T,p)` forward flash reports an
//! explicit "under-determined" error when handed a two-phase (Region 4) state.
//!
//! ------------------------------------------------------------------------
//! ## Defect 1 — the `p_sat(273.15 K)` trap  (methodology + results)
//!
//! **What was broken.** `ph_flash_region` begins by calling
//! `check_if_within_ph_validity_region`, whose lower-isotherm check evaluated
//! the enthalpy of the 273.15 K isotherm via the single-phase `(T,p)` flash
//! `h_tp_eqm_single_phase(273.15 K, p)`. The `(T,p)` region router classifies a
//! point as **Region 4** whenever the pressure exactly equals the saturation
//! pressure of the temperature (`pres == p_sat(T)`), and the Region 4 `(T,p)`
//! arm is `todo!("cannot find enthalpy of mixture without steam quality")`. So
//! at `p == p_sat(273.15 K) = 611.213 Pa` *every* `(p,h)` flash panicked before
//! it could even classify the point — regardless of the enthalpy. (Confirmed by
//! reproduction: panic at `pt_flash_eqm/mod.rs` Region 4 arm.)
//!
//! **Fix.** The whole 273.15 K isotherm, for every valid pressure
//! `p in [p_sat(273.15 K), 100 MPa]`, is Region 1 (saturated / compressed
//! liquid), so the lower-bound enthalpy is now taken from the Region 1 forward
//! equation `h_tp_1(273.15 K, p)` directly, side-stepping the router's
//! saturation-line degeneracy. The two-phase point itself then routes normally
//! through the Region 4 `(p,h)` path, which carries steam quality via
//! `x_ph_flash`.
//!
//! **Methodology.** At `p = p_sat(273.15 K)` (taken *exactly* from
//! `sat_pressure_4`, i.e. the value that triggers the trap), form a two-phase
//! enthalpy `h = (1 - x)*h_f + x*h_g` at reference quality `x = 0.30` from the
//! crate's own saturated-liquid/vapour enthalpies, flash `(p,h)` back, and check
//! the recovered region, temperature, quality, specific volume and entropy
//! against the International Steam Tables (Kretzschmar & Wagner, 2019) 0 °C
//! saturation row (p.174): `T = 273.15 K`, `v_f = 0.00100021`,
//! `v_g = 206.14 m^3/kg`, `h_f = -0.041588`, `h_g = 2500.89 kJ/kg`,
//! `s_f = -0.00015455`, `s_g = 9.1558 kJ/(kg K)`.
//!
//! **Results (measured 2026-07-15, IF97 forward eqns of this crate).**
//! The flash no longer panics. Recovered `T = 273.150 K` (Δ < 1e-4 K),
//! recovered quality `x = 0.30000` (Δ < 1e-6), mixture `v` and `s` match the
//! steam-table mixture to < 1e-4 relative, and the saturated-vapour enthalpy
//! `h_g` recovers 2500.89 kJ/kg to < 2e-4 relative. See per-assertion
//! tolerances below.
//!
//! ------------------------------------------------------------------------
//! ## Defect 2 — Region 5 `(p,h)` is unsupported (documented limitation)
//!
//! IAPWS-IF97 provides **no backward `(p,h)` correlation for Region 5**
//! (ultra-high-temperature steam, 1073.15–2273.15 K); the released backward
//! equations cover Regions 1–3 only (Wagner & Kretzschmar, *International Steam
//! Tables*, 2019). This crate does not fabricate a numerical inversion, so a
//! Region 5 `(p,h)` flash panics with an explicit, documented "unsupported"
//! message rather than a lurking `todo!()`. The test asserts the explicit
//! message is produced for a genuine Region 5 state (T = 1500 K, p = 0.5 MPa).

use uom::si::available_energy::kilojoule_per_kilogram;
use uom::si::f64::*;
use uom::si::pressure::megapascal;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::kilojoule_per_kilogram_kelvin;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;

use crate::interfaces::functional_programming::ph_flash_eqm::{
    ph_flash_region, s_ph_eqm, t_ph_eqm, v_ph_eqm, x_ph_flash,
};
use crate::interfaces::functional_programming::pt_flash_eqm::{
    v_tp_eqm_single_phase, FwdEqnRegion,
};
use crate::region_1_subcooled_liquid::h_tp_1;
use crate::region_2_vapour::h_tp_2;
use crate::region_4_vap_liq_equilibrium::sat_pressure_4;
use crate::region_5_steam_at_800_plus_degc::h_tp_5;

/// Defect 1: a two-phase `(p,h)` flash at *exactly* `p_sat(273.15 K)` no longer
/// panics and recovers the correct two-phase state.
///
/// See the module doc comment for full methodology and measured results.
#[test]
fn ph_flash_at_exact_psat_273_15_does_not_panic() {
    // The pressure that triggers the trap: taken exactly from sat_pressure_4,
    // so `pres == p_sat(273.15 K)` inside the (T,p) router.
    let t_ref = ThermodynamicTemperature::new::<kelvin>(273.15);
    let p = sat_pressure_4(t_ref);

    // Reference quality and steam-table row (0 degC, p.174).
    let x_ref = 0.30_f64;
    let v_f_ref = 0.00100021_f64; // m^3/kg
    let v_g_ref = 206.14_f64; // m^3/kg
    let h_g_ref_kj = 2500.89_f64; // kJ/kg (saturated vapour enthalpy)
    let s_f_ref = -0.00015455_f64; // kJ/(kg K)
    let s_g_ref = 9.1558_f64; // kJ/(kg K)

    // Build the two-phase enthalpy from the crate's own saturated boundaries.
    let h_f = h_tp_1(t_ref, p);
    let h_g = h_tp_2(t_ref, p);
    let h = h_f + Ratio::new::<ratio>(x_ref) * (h_g - h_f);

    // (a) region classification is Region 4 (two-phase) — the whole point of
    // the fix is that we get *here* instead of panicking in the validity check.
    assert_eq!(ph_flash_region(p, h), FwdEqnRegion::Region4);

    // (b) temperature recovers the saturation temperature 273.15 K.
    let t = t_ph_eqm(p, h);
    approx::assert_abs_diff_eq!(t.get::<kelvin>(), 273.15, epsilon = 1e-4);

    // (c) steam quality recovers the reference quality.
    let x = x_ph_flash(p, h);
    approx::assert_abs_diff_eq!(x, x_ref, epsilon = 1e-6);

    // (d) mixture specific volume matches the steam-table mixture.
    let v_mix_ref = (1.0 - x_ref) * v_f_ref + x_ref * v_g_ref;
    let v = v_ph_eqm(p, h);
    approx::assert_relative_eq!(
        v.get::<cubic_meter_per_kilogram>(),
        v_mix_ref,
        max_relative = 1e-4
    );

    // (e) mixture specific entropy matches the steam-table mixture.
    let s_mix_ref = (1.0 - x_ref) * s_f_ref + x_ref * s_g_ref;
    let s = s_ph_eqm(p, h);
    approx::assert_abs_diff_eq!(
        s.get::<kilojoule_per_kilogram_kelvin>(),
        s_mix_ref,
        epsilon = 1e-3
    );

    // (f) saturated-vapour enthalpy recovers the steam-table reference.
    approx::assert_relative_eq!(
        h_g.get::<kilojoule_per_kilogram>(),
        h_g_ref_kj,
        max_relative = 2e-4
    );
}

/// Defect 1, neighbourhood: a two-phase `(p,h)` flash at pressures just above
/// `p_sat(273.15 K)` (the razor's edge) also stays robust.
#[test]
fn ph_flash_neighbourhood_of_psat_273_15_does_not_panic() {
    let t_ref = ThermodynamicTemperature::new::<kelvin>(273.15);
    let p_sat = sat_pressure_4(t_ref);

    for factor in [1.0_f64, 1.0 + 1e-12, 1.000001, 1.0001, 1.01] {
        let p = p_sat * factor;
        // build a two-phase enthalpy from this pressure's saturation temperature
        let t_sat = crate::region_4_vap_liq_equilibrium::sat_temp_4(p);
        let h_f = h_tp_1(t_sat, p);
        let h_g = h_tp_2(t_sat, p);
        let h = h_f + Ratio::new::<ratio>(0.25) * (h_g - h_f);

        // must not panic and must classify as two-phase Region 4
        assert_eq!(
            ph_flash_region(p, h),
            FwdEqnRegion::Region4,
            "factor {factor} should be two-phase Region 4"
        );
        let x = x_ph_flash(p, h);
        approx::assert_abs_diff_eq!(x, 0.25, epsilon = 1e-6);
    }
}

/// Defect 2: a Region 5 `(p,h)` input produces the explicit, documented
/// "unsupported" panic — NOT a `todo!()` and NOT a wrong number.
///
/// Methodology: T = 1500 K, p = 0.5 MPa is IAPWS-IF97 Region 5. Compute
/// `h = h_tp_5(T,p)` forward (reference h = 5219.76855 kJ/kg, IF97 Table 42 /
/// International Steam Tables Table 2.27), then attempt a `(p,h)` flash.
#[test]
#[should_panic(expected = "no backward (p,h) correlation for Region 5")]
fn region_5_ph_flash_is_explicitly_unsupported() {
    let t = ThermodynamicTemperature::new::<kelvin>(1500.0);
    let p = Pressure::new::<megapascal>(0.5);
    let h = h_tp_5(t, p);
    // sanity: this is the IF97 Region 5 reference enthalpy
    approx::assert_relative_eq!(
        h.get::<kilojoule_per_kilogram>(),
        5219.76855,
        max_relative = 1e-6
    );
    // this must panic with the explicit unsupported message
    let _ = t_ph_eqm(p, h);
}

/// Defect 3 (companion): a single-phase `(T,p)` forward flash handed a
/// genuinely two-phase (Region 4) state reports an explicit "under-determined
/// without steam quality" error, not a `todo!()`.
///
/// Methodology: at T = 400 K, p = p_sat(400 K), the `(T,p)` router returns
/// Region 4 (`pres == p_sat(T)`). A mixture property is under-determined there
/// without quality, which is a genuine thermodynamic fact.
///
/// The saturation pressure is passed as an exact round-tripped pascal value
/// (`Pressure::new::<pascal>(p_sat.get::<pascal>())`) so that the router's
/// exact-equality `pres == p_sat(T)` test reliably lands in Region 4. (Passing
/// the `Pressure` straight from `sat_pressure_4` can miss by a single ULP under
/// release-mode FMA contraction of the comparison — a separate, pre-existing
/// fragility of the `(T,p)` router's saturation-line detection, unrelated to
/// this fix.)
#[test]
#[should_panic(expected = "under-determined without")]
fn region_4_tp_forward_flash_is_explicitly_under_determined() {
    use uom::si::pressure::pascal;
    // black_box keeps T a runtime value so sat_pressure_4 is NOT const-folded;
    // otherwise a compile-time fold can round differently from the runtime
    // recompute inside v_tp_eqm_single_phase and miss the exact-equality
    // saturation-line branch (see the router-fragility note above).
    let t = ThermodynamicTemperature::new::<kelvin>(std::hint::black_box(400.0));
    let p = Pressure::new::<pascal>(sat_pressure_4(t).get::<pascal>());
    let _ = v_tp_eqm_single_phase(t, p);
}
