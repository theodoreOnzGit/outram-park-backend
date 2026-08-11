//! Bounds-checked `(p,s)` flash facade (bead `op-dt3.26`).
//!
//! The unchecked `(p,s)` entry points in
//! [`crate::interfaces::functional_programming::ps_flash_eqm`] **panic** on
//! out-of-range input. This module wraps them with an explicit
//! validity-envelope check performed **before** any panicking internal is
//! called, returning [`SteamTablesError`] instead.
//!
//! ## Panic-trace: every reachable `panic!` and the gate that excludes it
//!
//! Traced against the source on 2026-08-11. Every `(p,s)` entry point
//! starts by calling `ps_flash_region(p, s)`, which calls
//! `check_if_within_ps_validity_region(p, s)`; the region equations
//! themselves (`region_1_.../region_5_...`, `backward_eqn_ps_*`) contain no
//! `panic!`/`todo!`/`unwrap` at all, so the entire panic surface lives in
//! the router.
//!
//! | Panic site | Condition | Excluded by |
//! |---|---|---|
//! | `ps_flash_eqm/validity_range.rs:21` "p,s point is lower than acceptable pressure range" | `p < p_sat(273.15 K)` | pressure lower gate |
//! | `ps_flash_eqm/validity_range.rs:26` "p,s point is higher than acceptable pressure range" | `p > 100 MPa` | pressure upper gate |
//! | `ps_flash_eqm/validity_range.rs:40`, `:60` "outside pressure range" | re-check of the two above | same two gates |
//! | `ps_flash_eqm/mod.rs:828` "p,s point is outside pressure range" | same predicate | same two gates |
//! | `ps_flash_eqm/mod.rs:833` "p,s point below 273.15K" | `s < s(273.15 K, p)` | entropy lower gate |
//! | `ps_flash_eqm/mod.rs:837` "p,s point above 1073.15K" | `s > s(1073.15 K, p)` | entropy upper gate |
//! | `pt_flash_eqm/mod.rs:198` "entropy: two-phase (T,p) ... under-determined" | formerly reached *inside* `is_below_isotherm_t_273_15` when `p == p_sat(273.15 K)` **bit-for-bit** | **no longer reachable**: that check now evaluates its lower bound with the Region-1 forward equation `s_tp_1` (bead `op-znjx`), and this facade mirrors it |
//! | `boundaries_between_single_phase_regions.rs:20,23,60,63,106` "p in (p,s) point is outside validity range" | `p` below 16.529 MPa / above 100 MPa inside the `p >= 16.529 MPa` branch | unreachable: that branch is only entered when `16.529 MPa <= p <= 100 MPa` already holds |
//! | `boundaries_from_single_phase_regions_to_region_4_multiphase.rs:30,33,75,78` "entropy of p,s point is outside validity range" | `s` outside the Region-3/4 boundary entropy band | unreachable inside the gate: the Region-1 and Region-2 tests immediately above them already returned for every `s` outside that band |
//! | `boundaries_from_single_phase_regions_to_region_4_multiphase.rs:123,151` "pressure of p,s point is outside validity range" | `p` outside `[p_sat(273.15 K), p_sat(623.15 K)]` in the `p < 16.529 MPa` branch | pressure gate plus the branch condition |
//! | `ps_flash_eqm/mod.rs:64,154` `todo!("region 5 ps flash not implemented")` | `ps_flash_region` returning `Region5` | unreachable: `ps_flash_region` has no code path that returns `Region5` |
//!
//! The last four rows are "unreachable" arguments rather than direct gates,
//! so they were checked empirically as well — see the V&V note in
//! [`super::tests`]: a 273 609-point sweep of the accepted set found zero
//! surviving panics across all ten wrapped functions.
//!
//! ## The `p == p_sat(273.15 K)` trap — fixed 2026-08-11 (bead `op-znjx`)
//!
//! This used to be a real asymmetry with the `(p,h)` facade. The `(p,h)`
//! validity check dodges the trap by evaluating its lower enthalpy bound
//! with the Region-1 forward equation `h_tp_1` instead of the `(T,p)`
//! router (see the note in `ph_flash_eqm/validity_range.rs`); the `(p,s)`
//! check used to call `s_tp_eqm_single_phase(273.15 K, p)`, which routes to
//! Region 4 and panics at exactly `p = p_sat(273.15 K)`, so this facade
//! carried an **exclusive** pressure floor where the `(p,h)` one is
//! inclusive.
//!
//! `ps_flash_eqm/validity_range.rs::is_below_isotherm_t_273_15` now uses
//! `s_tp_1` for that bound, exactly as the `(p,h)` sibling uses `h_tp_1`.
//! The floor here is therefore **inclusive** again, matching `(p,h)`, and
//! [`check_ps_envelope`] mirrors the internal change by computing its own
//! lower entropy bound with `s_tp_1` too. (Precedent for this kind of
//! carve-out removal: bead `op-cv1c`, where fixing the region router's
//! half-open 100 MPa edge let this facade drop its matching carve-out.)
//!
//! ## No `catch_unwind`
//!
//! This facade is a bounds check, not an exception handler. Non-finite
//! input (`NaN`, `+/-inf`) is rejected explicitly up front, because the
//! internals' range comparisons are silently `false` for `NaN` and would
//! otherwise let `NaN` states through undetected.

use uom::si::f64::*;
use uom::si::pressure::pascal;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

use crate::interfaces::functional_programming::ps_flash_eqm::{
    alpha_v_ps_eqm, cp_ps_eqm, cv_ps_eqm, h_ps_eqm, kappa_ps_eqm, kappa_t_ps_eqm,
    mass_flux_ps_eqm_throat, t_ps_eqm, v_ps_eqm, w_ps_wood_wallis, x_ps_flash,
};
use crate::interfaces::functional_programming::pt_flash_eqm::s_tp_eqm_single_phase;
use crate::interfaces::functional_programming::pt_flash_eqm::s_tp_eqm_two_phase;
use crate::region_1_subcooled_liquid::{s_tp_1, InversePressure};
use crate::region_4_vap_liq_equilibrium::{sat_pressure_4, sat_temp_4};

use super::two_phase::check_tpx_envelope;
use super::{Result, SteamTablesError, P_MAX_PASCAL, T_MIN_KELVIN, T_R5_LOWER_KELVIN};

/// Validates a `(p, s)` pair against the envelope the unchecked `(p,s)`
/// internals actually accept, returning `Ok(())` when every wrapped
/// `try_*_ps_*` function in this module is safe to call.
///
/// # Physical quantities and valid ranges
///
/// - `p` — absolute pressure, Pa. Valid in the **closed** interval
///   `[p_sat(273.15 K), 100 MPa]`, i.e. from the triple-point saturation
///   pressure (about 611.213 Pa) up to and including 100 MPa. Both edges
///   are inclusive, matching `is_outside_pressure_range`, which rejects
///   only `p < p_sat(273.15 K)` and `p > 100 MPa`. The lower edge was
///   exclusive until bead `op-znjx` removed the Region-4 trap documented
///   at module level.
/// - `s` — specific entropy, J/(kg K). Valid between the 273.15 K and
///   1073.15 K isotherm entropies **at that pressure**, both edges
///   inclusive. Each bound is evaluated with the same call the internal
///   check uses — `s_tp_1` on the 273.15 K isotherm (which lies wholly in
///   Region 1) and `s_tp_eqm_single_phase` on the 1073.15 K one — so the
///   accepted set is identical rather than merely similar.
///
/// Non-finite input is rejected first with
/// [`SteamTablesError::NonFinite`].
pub fn check_ps_envelope(p: Pressure, s: SpecificHeatCapacity) -> Result<()> {
    let p_pascal = p.get::<pascal>();
    let s_si = s.get::<joule_per_kilogram_kelvin>();

    if !p_pascal.is_finite() {
        return Err(SteamTablesError::NonFinite {
            quantity: "pressure",
            value: p_pascal,
            unit: "Pa",
        });
    }
    if !s_si.is_finite() {
        return Err(SteamTablesError::NonFinite {
            quantity: "specific entropy",
            value: s_si,
            unit: "J/(kg K)",
        });
    }

    // Both edges INCLUSIVE, matching `is_outside_pressure_range`. The lower
    // edge was exclusive until bead `op-znjx`: `is_below_isotherm_t_273_15`
    // used to evaluate `s_tp_eqm_single_phase` on the 273.15 K isotherm, and
    // the (T,p) router classifies exactly p == p_sat(273.15 K) as Region 4
    // (two-phase (T,p) is under-determined) and panicked. That check now
    // uses the Region-1 forward equation `s_tp_1`, so the triple-point
    // isobar is a perfectly ordinary accepted state.
    let p_min_pascal =
        sat_pressure_4(ThermodynamicTemperature::new::<kelvin>(T_MIN_KELVIN)).get::<pascal>();
    if p_pascal < p_min_pascal || p_pascal > P_MAX_PASCAL {
        return Err(SteamTablesError::OutOfRange {
            quantity: "pressure",
            value: p_pascal,
            min: p_min_pascal,
            max: P_MAX_PASCAL,
            unit: "Pa",
        });
    }

    // Entropy window at this pressure, computed with the SAME functions the
    // internal validity check uses so the accepted set is identical: the
    // Region-1 forward equation `s_tp_1` on the 273.15 K isotherm (which
    // lies wholly in Region 1 for every valid pressure — see the module
    // doc's `p_sat(273.15 K)` section) and the (T,p) router on the
    // 1073.15 K one. Both calls are panic-free inside the pressure bounds
    // above, including at the p_sat(273.15 K) edge.
    let t_min = ThermodynamicTemperature::new::<kelvin>(T_MIN_KELVIN);
    let t_max = ThermodynamicTemperature::new::<kelvin>(T_R5_LOWER_KELVIN);
    let s_min = s_tp_1(t_min, p).get::<joule_per_kilogram_kelvin>();
    let s_max = s_tp_eqm_single_phase(t_max, p).get::<joule_per_kilogram_kelvin>();
    if s_si < s_min || s_si > s_max {
        return Err(SteamTablesError::OutOfRange {
            quantity: "specific entropy",
            value: s_si,
            min: s_min,
            max: s_max,
            unit: "J/(kg K)",
        });
    }
    Ok(())
}

/// Checked temperature T (K) from a `(p,s)` flash (Regions 1-4; Region 5
/// is not implemented for this flash path and lies outside the entropy
/// window). Valid range: the `(p,s)` envelope in
/// [`check_ps_envelope`]. Agrees exactly with [`t_ps_eqm`] for in-range
/// input.
pub fn try_t_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> Result<ThermodynamicTemperature> {
    check_ps_envelope(p, s)?;
    Ok(t_ps_eqm(p, s))
}

/// Checked specific volume v (m^3/kg) from a `(p,s)` flash (two-phase
/// states return the quality-weighted mixture volume). Valid range: the
/// `(p,s)` envelope in [`check_ps_envelope`]. Agrees exactly with
/// [`v_ps_eqm`] for in-range input.
pub fn try_v_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> Result<SpecificVolume> {
    check_ps_envelope(p, s)?;
    Ok(v_ps_eqm(p, s))
}

/// Checked mass density rho (kg/m^3) from a `(p,s)` flash — the reciprocal
/// of [`try_v_ps_eqm`]. Valid range: the `(p,s)` envelope in
/// [`check_ps_envelope`].
pub fn try_rho_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> Result<MassDensity> {
    check_ps_envelope(p, s)?;
    Ok(v_ps_eqm(p, s).recip())
}

/// Checked specific enthalpy h (J/kg) from a `(p,s)` flash. Valid range:
/// the `(p,s)` envelope in [`check_ps_envelope`]. Agrees exactly with
/// [`h_ps_eqm`] for in-range input.
pub fn try_h_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> Result<AvailableEnergy> {
    check_ps_envelope(p, s)?;
    Ok(h_ps_eqm(p, s))
}

/// Checked steam quality x (dimensionless, bare `f64` to agree exactly with
/// the unchecked [`x_ps_flash`]): 0 for subcooled liquid, 1 for superheated
/// vapour, in `(0, 1)` inside the vapour-liquid dome. Valid range: the
/// `(p,s)` envelope in [`check_ps_envelope`].
pub fn try_x_ps_flash(p: Pressure, s: SpecificHeatCapacity) -> Result<f64> {
    check_ps_envelope(p, s)?;
    Ok(x_ps_flash(p, s))
}

/// Checked isobaric specific heat capacity cp (J/(kg K)) from a `(p,s)`
/// flash (two-phase states return a quality-interpolated estimate — see
/// [`cp_ps_eqm`]). Valid range: the `(p,s)` envelope in
/// [`check_ps_envelope`].
pub fn try_cp_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> Result<SpecificHeatCapacity> {
    check_ps_envelope(p, s)?;
    Ok(cp_ps_eqm(p, s))
}

/// Checked isochoric specific heat capacity cv (J/(kg K)) from a `(p,s)`
/// flash (two-phase states return a quality-interpolated estimate — see
/// [`cv_ps_eqm`]). Valid range: the `(p,s)` envelope in
/// [`check_ps_envelope`].
pub fn try_cv_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> Result<SpecificHeatCapacity> {
    check_ps_envelope(p, s)?;
    Ok(cv_ps_eqm(p, s))
}

/// Checked speed of sound w (m/s) from a `(p,s)` flash; in the two-phase
/// region this is the Wood/Wallis homogeneous-mixture sound speed. Valid
/// range: the `(p,s)` envelope in [`check_ps_envelope`]. Agrees exactly
/// with [`w_ps_wood_wallis`] for in-range input.
pub fn try_w_ps_wood_wallis(p: Pressure, s: SpecificHeatCapacity) -> Result<Velocity> {
    check_ps_envelope(p, s)?;
    Ok(w_ps_wood_wallis(p, s))
}

/// Checked isentropic exponent kappa (dimensionless `Ratio`) from a `(p,s)`
/// flash. Valid range: the `(p,s)` envelope in [`check_ps_envelope`].
/// Agrees exactly with [`kappa_ps_eqm`] for in-range input.
pub fn try_kappa_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> Result<Ratio> {
    check_ps_envelope(p, s)?;
    Ok(kappa_ps_eqm(p, s))
}

/// Checked isobaric cubic expansion coefficient alpha_v (1/K) from a
/// `(p,s)` flash. Valid range: the `(p,s)` envelope in
/// [`check_ps_envelope`]. Agrees exactly with [`alpha_v_ps_eqm`] for
/// in-range input.
pub fn try_alpha_v_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> Result<TemperatureCoefficient> {
    check_ps_envelope(p, s)?;
    Ok(alpha_v_ps_eqm(p, s))
}

/// Checked isothermal compressibility kappa_T (1/Pa) from a `(p,s)` flash.
/// Valid range: the `(p,s)` envelope in [`check_ps_envelope`]. Agrees
/// exactly with [`kappa_t_ps_eqm`] for in-range input.
pub fn try_kappa_t_ps_eqm(p: Pressure, s: SpecificHeatCapacity) -> Result<InversePressure> {
    check_ps_envelope(p, s)?;
    Ok(kappa_t_ps_eqm(p, s))
}

/// Relative finite-difference step used inside `mass_flux_ps_eqm_throat`
/// (`dp = p * 1e-5`). Mirrored here so the checked wrapper can validate the
/// perturbed pressures the unchecked function will actually evaluate.
const MASS_FLUX_DP_RELATIVE: f64 = 1e-5;

/// Absolute pressure floor `mass_flux_ps_eqm_throat` clamps its downward
/// perturbation to, in Pa — `0.000_611_212_677 MPa * 1.01`, copied from the
/// unchecked function so the mirror stays exact.
const MASS_FLUX_P_FLOOR_PASCAL: f64 = 0.000_611_212_677e6 * 1.01;

/// Checked HEM critical mass flux G (kg/(m^2 s)) evaluated **at throat
/// conditions** `(p, s)` — not stagnation conditions. Valid range: a
/// **stricter** envelope than [`check_ps_envelope`], because the unchecked
/// [`mass_flux_ps_eqm_throat`] differentiates `v(p,s)` by finite difference
/// and therefore evaluates the `(p,s)` flash at *three* pressures.
///
/// # Panic-trace
///
/// Beyond the `(p,s)` panics listed at module level, the unchecked function
/// reaches them again at the perturbed pressures:
///
/// - `v_ps_eqm(p + p * 1e-5, s_adjusted)` — panics for `p` within
///   `1e-5` relative of 100 MPa (the step walks over the ceiling), and for
///   `s` within a step of the 1073.15 K isotherm (raising the pressure
///   lowers that isotherm's entropy). Excluded by re-running
///   [`check_ps_envelope`] at `p + dp`.
/// - `v_ps_eqm(max(p - p * 1e-5, 611.823 Pa), s_adjusted)` — same, at the
///   lower edge. Excluded by re-running [`check_ps_envelope`] at that
///   clamped pressure.
/// - `s_tp_eqm_two_phase(sat_temp_4(p), p, 0.0 | 1.0)`, used to locate the
///   bubble point for the near-saturation entropy adjustment. Excluded by
///   [`check_tpx_envelope`] at `(sat_temp_4(p), p, 0)`; the `x = 1` call is
///   the same `(T,p)` point and both qualities are inside `[0, 1]`.
///
/// The entropy actually passed to the perturbed flashes is the *adjusted*
/// entropy the unchecked function computes (it snaps entropies just below
/// the bubble point up to a quality of `1e-4`), reproduced here exactly so
/// the checked bound is the one the internals will use.
///
/// Units: `p` in Pa, `s` in J/(kg K), result in kg/(m^2 s).
pub fn try_mass_flux_ps_eqm_throat(p: Pressure, s: SpecificHeatCapacity) -> Result<MassFlux> {
    check_ps_envelope(p, s)?;

    // Mirror of the unchecked function's bubble-point entropy adjustment.
    let t_sat = sat_temp_4(p);
    check_tpx_envelope(t_sat, p, 0.0)?;
    let s_l = s_tp_eqm_two_phase(t_sat, p, 0.0);
    let s_v = s_tp_eqm_two_phase(t_sat, p, 1.0);
    let quality_at_bubble_point = 1e-4;
    let s_bubble_pt = s_l * (1.0 - quality_at_bubble_point) + quality_at_bubble_point * s_v;
    let ds = s_bubble_pt - s_l;
    let s_min = s_l - ds;
    let s_adjusted = if s >= s_min && s <= s_bubble_pt {
        s_bubble_pt
    } else {
        s
    };

    // Mirror of the finite-difference stencil.
    let dp = p * MASS_FLUX_DP_RELATIVE;
    let p_floor = Pressure::new::<pascal>(MASS_FLUX_P_FLOOR_PASCAL);
    let p_minus = if p - dp > p_floor { p - dp } else { p_floor };
    check_ps_envelope(p + dp, s_adjusted)?;
    check_ps_envelope(p_minus, s_adjusted)?;

    Ok(mass_flux_ps_eqm_throat(p, s))
}
