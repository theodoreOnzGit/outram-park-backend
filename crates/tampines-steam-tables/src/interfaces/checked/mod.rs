//! Bounds-checked, `Result`-returning facade over the IAPWS-IF97 flash
//! internals (bead `op-t647`).
//!
//! The unchecked entry points in
//! [`super::functional_programming::pt_flash_eqm`] and
//! [`super::functional_programming::ph_flash_eqm`] **panic** on
//! out-of-range input (`region_fwd_eqn_single_phase` and
//! `check_if_within_ph_validity_region` both end in `panic!`). In a
//! transient simulation a single overshooting state kills the physics
//! thread. This module wraps the commonly used `(T,p)` and `(p,h)` flash
//! functions with an explicit validity-envelope check performed **before**
//! any panicking internal is called, returning
//! [`SteamTablesError`] instead of panicking.
//!
//! ## The envelope enforced here
//!
//! The bounds below are **read from this crate's own region router**
//! (`region_fwd_eqn_single_phase` in `pt_flash_eqm/mod.rs` and
//! `check_if_within_ph_validity_region` + `validity_range.rs` in
//! `ph_flash_eqm/`), verified against the source on 2026-08-11 — they are
//! the set of inputs the internals actually accept, which matches the
//! documented IAPWS-IF97 envelope with three sharp edges noted below.
//!
//! `(T,p)` single-phase flashes ([`try_h_tp_eqm_single_phase`] etc.):
//!
//! - `T` in `[273.15 K, 2273.15 K]` overall;
//! - `p` in `(0, 100 MPa]` for `T` in `[273.15 K, 623.15 K]`;
//! - `p` in `(0, 100 MPa)` — **exclusive** upper edge — for `T` in
//!   `(623.15 K, 1073.15 K]`: the internal region router's Region-2 and
//!   Region-3 match arms use half-open ranges (`..100e6`), so exactly
//!   100 MPa above 623.15 K has no matching arm and would panic;
//! - `p` in `(0, 50 MPa]` for `T` in `(1073.15 K, 2273.15 K]` (Region 5);
//! - a `(T, p)` pair lying **exactly** on the saturation line
//!   (`p == p_sat(T)` bit-for-bit, `T < 647.096 K`) is rejected with
//!   [`SteamTablesError::SaturatedTpUnderdetermined`]: the internals route
//!   it to Region 4, where a single-phase `(T,p)` flash is physically
//!   under-determined without a steam quality (see
//!   `REGION_4_TP_UNDERDETERMINED` in `pt_flash_eqm`).
//!
//! `(p,h)` flashes ([`try_t_ph_eqm`] etc.):
//!
//! - `p` in `[p_sat(273.15 K) ≈ 611.213 Pa, 100 MPa)` — the upper edge is
//!   **exclusive** because the internal 1073.15 K-isotherm bound helper
//!   (`is_above_isotherm_t_1073_15`) evaluates
//!   `h_tp_eqm_single_phase(1073.15 K, p)`, whose region router rejects
//!   exactly 100 MPa above 623.15 K (previous bullet), so *every* internal
//!   `(p,h)` call at exactly `p = 100 MPa` panics regardless of `h`;
//! - `h` in `[h(273.15 K, p), h(1073.15 K, p)]`, evaluated with the same
//!   Region-1 forward equation (`h_tp_1`) and single-phase `(T,p)` flash
//!   the internal validity check uses, so the accepted set is identical.
//!   `(p,h)` flashes into Region 5 (`T > 1073.15 K`) are unsupported by
//!   IAPWS-IF97 itself (no backward `(p,h)` correlation) and fall out of
//!   the `h` upper bound here.
//!
//! Non-finite input (`NaN`, `±inf`) is rejected up front with
//! [`SteamTablesError::NonFinite`] — the internals' range comparisons are
//! silently `false` for `NaN`, which would otherwise let `NaN` states flow
//! through undetected.
//!
//! ## No `catch_unwind`
//!
//! This facade uses **bounds-checking only** — no
//! `std::panic::catch_unwind`. Every reachable `panic!` site in the
//! wrapped call graph is an envelope violation (region-router fallthrough,
//! `(p,h)` validity check, Region-4/Region-5 dispatch arms), and all of
//! them are excluded by the checks above before the internals run.
//! Closed-form IAPWS polynomial evaluation inside the envelope does not
//! panic (it may lose accuracy near the critical point — see the crate
//! `CLAUDE.md` "Known accuracy pitfalls" — but degraded accuracy is
//! returned as a value, not a panic).

use uom::si::f64::*;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::available_energy::joule_per_kilogram;

use crate::region_1_subcooled_liquid::h_tp_1;
use crate::region_4_vap_liq_equilibrium::sat_pressure_4;

use super::functional_programming::ph_flash_eqm::{
    cp_ph_eqm, cv_ph_eqm, kappa_ph_eqm, lambda_ph_eqm, mu_ph_eqm, s_ph_eqm, t_ph_eqm, u_ph_eqm,
    v_ph_eqm, w_ph_wood_wallis, x_ph_flash,
};
use super::functional_programming::pt_flash_eqm::{
    cp_tp_eqm_single_phase, cv_tp_eqm_single_phase, h_tp_eqm_single_phase,
    kappa_tp_eqm_single_phase, s_tp_eqm_single_phase, u_tp_eqm_single_phase,
    v_tp_eqm_single_phase, w_tp_eqm_single_phase,
};
use crate::dynamic_viscosity::mu_tp_eqm_single_phase;
use crate::thermal_conductivity::lambda_tp_eqm_single_phase;

/// Minimum IAPWS-IF97 temperature, K (Regions 1-4 lower bound).
const T_MIN_KELVIN: f64 = 273.15;
/// Region 1-4 / Region 5 boundary isotherm, K.
const T_R5_LOWER_KELVIN: f64 = 1073.15;
/// Maximum IAPWS-IF97 temperature, K (Region 5 upper bound).
const T_MAX_KELVIN: f64 = 2273.15;
/// Region 1 / Region 3 boundary isotherm, K (above it the internal region
/// router's 100 MPa pressure edge becomes exclusive).
const T_R1_UPPER_KELVIN: f64 = 623.15;
/// Critical temperature, K — upper end of the saturation line; the exact
/// `p == p_sat(T)` Region-4 trap only exists below it.
const T_CRIT_KELVIN: f64 = 647.096;
/// Maximum IAPWS-IF97 pressure for Regions 1-4, Pa.
const P_MAX_PASCAL: f64 = 100.0e6;
/// Maximum IAPWS-IF97 pressure for Region 5, Pa.
const P_MAX_R5_PASCAL: f64 = 50.0e6;

/// Error type for the bounds-checked IF97 facade.
///
/// Every variant carries the offending raw SI value so a caller (or a log
/// line) can see exactly which input broke which bound without re-deriving
/// units: temperatures in kelvin, pressures in pascal, specific enthalpies
/// in J/kg.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SteamTablesError {
    /// An input was `NaN` or infinite. The unchecked internals cannot
    /// detect this (range comparisons on `NaN` are silently `false`), so
    /// it is rejected here before anything else.
    #[error("{quantity} = {value} is not a finite number")]
    NonFinite {
        /// Which input quantity was non-finite (e.g. `"temperature"`).
        quantity: &'static str,
        /// The offending raw value in the SI unit named by `unit`.
        value: f64,
        /// SI unit of `value` as prose (e.g. `"K"`, `"Pa"`, `"J/kg"`).
        unit: &'static str,
    },
    /// An input lies outside the IAPWS-IF97 validity envelope enforced by
    /// this facade (see the module-level doc for the exact bounds,
    /// including which edges are exclusive).
    #[error(
        "{quantity} = {value} {unit} is outside the IF97 validity range \
         [{min}, {max}] {unit} (see interfaces::checked module docs for \
         exclusive edges)"
    )]
    OutOfRange {
        /// Which input quantity broke the bound (e.g. `"pressure"`).
        quantity: &'static str,
        /// The offending raw value in the SI unit named by `unit`.
        value: f64,
        /// Lower bound of the valid range, same unit.
        min: f64,
        /// Upper bound of the valid range, same unit. May be an exclusive
        /// edge — see the module-level doc.
        max: f64,
        /// SI unit of `value`/`min`/`max` as prose (e.g. `"K"`, `"Pa"`).
        unit: &'static str,
    },
    /// The `(T, p)` pair lies exactly on the saturation line
    /// (`p == p_sat(T)` bit-for-bit, `T < 647.096 K`). A single-phase
    /// `(T,p)` flash cannot resolve a two-phase state — the steam quality
    /// is a free variable there. Use a `(p,h)` flash (which carries the
    /// quality) instead.
    #[error(
        "(T, p) = ({t_kelvin} K, {p_pascal} Pa) lies exactly on the \
         saturation line: a single-phase (T,p) flash is under-determined \
         in the two-phase region (steam quality is a free variable); use a \
         (p,h) flash instead"
    )]
    SaturatedTpUnderdetermined {
        /// Temperature of the rejected state, K.
        t_kelvin: f64,
        /// Pressure of the rejected state (equal to `p_sat(T)`), Pa.
        p_pascal: f64,
    },
}

/// Convenience alias: every checked facade function returns
/// `Result<uom quantity, SteamTablesError>`.
pub type Result<T> = core::result::Result<T, SteamTablesError>;

/// Validates a `(T, p)` pair against the IF97 single-phase envelope the
/// unchecked `(T,p)` internals actually accept (module-level doc has the
/// full bound list). Returns `Ok(())` when every wrapped
/// `try_*_tp_eqm_single_phase` function is safe to call.
///
/// Inputs: `t` is a thermodynamic temperature (valid 273.15-2273.15 K),
/// `p` an absolute pressure (valid up to 100 MPa below 623.15 K, up to
/// but excluding 100 MPa from 623.15 K to 1073.15 K, up to 50 MPa above).
pub fn check_tp_single_phase_envelope(t: ThermodynamicTemperature, p: Pressure) -> Result<()> {
    let t_kelvin = t.get::<kelvin>();
    let p_pascal = p.get::<pascal>();

    if !t_kelvin.is_finite() {
        return Err(SteamTablesError::NonFinite {
            quantity: "temperature",
            value: t_kelvin,
            unit: "K",
        });
    }
    if !p_pascal.is_finite() {
        return Err(SteamTablesError::NonFinite {
            quantity: "pressure",
            value: p_pascal,
            unit: "Pa",
        });
    }
    if !(T_MIN_KELVIN..=T_MAX_KELVIN).contains(&t_kelvin) {
        return Err(SteamTablesError::OutOfRange {
            quantity: "temperature",
            value: t_kelvin,
            min: T_MIN_KELVIN,
            max: T_MAX_KELVIN,
            unit: "K",
        });
    }
    // Pressure ceiling depends on the temperature band; the lower edge is
    // exclusive at 0 everywhere (vacuum has no IF97 state).
    let (p_max, p_max_is_inclusive) = if t_kelvin > T_R5_LOWER_KELVIN {
        // Region 5 band (1073.15 K, 2273.15 K]: p <= 50 MPa.
        (P_MAX_R5_PASCAL, true)
    } else if t_kelvin > T_R1_UPPER_KELVIN {
        // (623.15 K, 1073.15 K]: the internal router's Region-2/Region-3
        // arms are half-open at 100 MPa, so the edge is EXCLUSIVE.
        (P_MAX_PASCAL, false)
    } else {
        // [273.15 K, 623.15 K]: Region 1 accepts exactly 100 MPa.
        (P_MAX_PASCAL, true)
    };
    let p_above_max = if p_max_is_inclusive {
        p_pascal > p_max
    } else {
        p_pascal >= p_max
    };
    if p_pascal <= 0.0 || p_above_max {
        return Err(SteamTablesError::OutOfRange {
            quantity: "pressure",
            value: p_pascal,
            min: 0.0,
            max: p_max,
            unit: "Pa",
        });
    }
    // Exact saturation-line hit below the critical temperature routes to
    // Region 4 inside the internals, whose single-phase (T,p) arms panic
    // (under-determined without steam quality) — reject it here instead.
    if t_kelvin < T_CRIT_KELVIN && p_pascal == sat_pressure_4(t).get::<pascal>() {
        return Err(SteamTablesError::SaturatedTpUnderdetermined { t_kelvin, p_pascal });
    }
    Ok(())
}

/// Validates a `(p, h)` pair against the envelope the unchecked `(p,h)`
/// internals actually accept: `p` in `[p_sat(273.15 K), 100 MPa)` (upper
/// edge exclusive — see the module-level doc for why exactly 100 MPa
/// panics internally) and `h` between the 273.15 K and 1073.15 K
/// isotherm enthalpies at that pressure. Returns `Ok(())` when every
/// wrapped `try_*_ph_*` function is safe to call.
///
/// Inputs: `p` is an absolute pressure (Pa), `h` a specific enthalpy
/// (J/kg); the valid `h` window is pressure-dependent and is reported in
/// the error when violated.
pub fn check_ph_envelope(p: Pressure, h: AvailableEnergy) -> Result<()> {
    let p_pascal = p.get::<pascal>();
    let h_joule_per_kg = h.get::<joule_per_kilogram>();

    if !p_pascal.is_finite() {
        return Err(SteamTablesError::NonFinite {
            quantity: "pressure",
            value: p_pascal,
            unit: "Pa",
        });
    }
    if !h_joule_per_kg.is_finite() {
        return Err(SteamTablesError::NonFinite {
            quantity: "specific enthalpy",
            value: h_joule_per_kg,
            unit: "J/kg",
        });
    }
    // Same lower bound the internal check uses: p_sat(273.15 K).
    let p_min_pascal =
        sat_pressure_4(ThermodynamicTemperature::new::<kelvin>(T_MIN_KELVIN)).get::<pascal>();
    // Upper edge EXCLUSIVE: at exactly 100 MPa the internal validity check
    // itself panics (module-level doc, "(p,h) flashes" bullet 1).
    if p_pascal < p_min_pascal || p_pascal >= P_MAX_PASCAL {
        return Err(SteamTablesError::OutOfRange {
            quantity: "pressure",
            value: p_pascal,
            min: p_min_pascal,
            max: P_MAX_PASCAL,
            unit: "Pa",
        });
    }
    // Enthalpy window at this pressure, computed with the SAME functions
    // the internal validity check uses so the accepted set is identical.
    // Both calls are panic-free for p inside the pressure bounds above.
    let t_min = ThermodynamicTemperature::new::<kelvin>(T_MIN_KELVIN);
    let t_max = ThermodynamicTemperature::new::<kelvin>(T_R5_LOWER_KELVIN);
    let h_min = h_tp_1(t_min, p).get::<joule_per_kilogram>();
    let h_max = h_tp_eqm_single_phase(t_max, p).get::<joule_per_kilogram>();
    if h_joule_per_kg < h_min || h_joule_per_kg > h_max {
        return Err(SteamTablesError::OutOfRange {
            quantity: "specific enthalpy",
            value: h_joule_per_kg,
            min: h_min,
            max: h_max,
            unit: "J/kg",
        });
    }
    Ok(())
}

// ===================== (T,p) single-phase facade =====================

/// Checked specific enthalpy h (J/kg) from a single-phase `(T,p)` flash.
/// Valid range: the `(T,p)` envelope in the module doc (T 273.15-2273.15 K
/// with the band-dependent pressure ceiling); exact saturation-line pairs
/// are rejected. Agrees exactly with
/// [`h_tp_eqm_single_phase`] for in-range input.
pub fn try_h_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Result<AvailableEnergy> {
    check_tp_single_phase_envelope(t, p)?;
    Ok(h_tp_eqm_single_phase(t, p))
}

/// Checked specific internal energy u (J/kg) from a single-phase `(T,p)`
/// flash. Valid range: the `(T,p)` envelope in the module doc. Agrees
/// exactly with [`u_tp_eqm_single_phase`] for in-range input.
pub fn try_u_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Result<AvailableEnergy> {
    check_tp_single_phase_envelope(t, p)?;
    Ok(u_tp_eqm_single_phase(t, p))
}

/// Checked specific entropy s (J/(kg K)) from a single-phase `(T,p)`
/// flash. Valid range: the `(T,p)` envelope in the module doc. Agrees
/// exactly with [`s_tp_eqm_single_phase`] for in-range input.
pub fn try_s_tp_eqm_single_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
) -> Result<SpecificHeatCapacity> {
    check_tp_single_phase_envelope(t, p)?;
    Ok(s_tp_eqm_single_phase(t, p))
}

/// Checked isobaric specific heat capacity cp (J/(kg K)) from a
/// single-phase `(T,p)` flash. Valid range: the `(T,p)` envelope in the
/// module doc. Agrees exactly with [`cp_tp_eqm_single_phase`] for
/// in-range input.
pub fn try_cp_tp_eqm_single_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
) -> Result<SpecificHeatCapacity> {
    check_tp_single_phase_envelope(t, p)?;
    Ok(cp_tp_eqm_single_phase(t, p))
}

/// Checked isochoric specific heat capacity cv (J/(kg K)) from a
/// single-phase `(T,p)` flash. Valid range: the `(T,p)` envelope in the
/// module doc. Agrees exactly with [`cv_tp_eqm_single_phase`] for
/// in-range input.
pub fn try_cv_tp_eqm_single_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
) -> Result<SpecificHeatCapacity> {
    check_tp_single_phase_envelope(t, p)?;
    Ok(cv_tp_eqm_single_phase(t, p))
}

/// Checked specific volume v (m^3/kg) from a single-phase `(T,p)` flash.
/// Valid range: the `(T,p)` envelope in the module doc. Agrees exactly
/// with [`v_tp_eqm_single_phase`] for in-range input.
pub fn try_v_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Result<SpecificVolume> {
    check_tp_single_phase_envelope(t, p)?;
    Ok(v_tp_eqm_single_phase(t, p))
}

/// Checked mass density rho (kg/m^3) from a single-phase `(T,p)` flash —
/// the reciprocal of [`try_v_tp_eqm_single_phase`]. Valid range: the
/// `(T,p)` envelope in the module doc.
pub fn try_rho_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Result<MassDensity> {
    check_tp_single_phase_envelope(t, p)?;
    Ok(v_tp_eqm_single_phase(t, p).recip())
}

/// Checked speed of sound w (m/s) from a single-phase `(T,p)` flash.
/// Valid range: the `(T,p)` envelope in the module doc. Agrees exactly
/// with [`w_tp_eqm_single_phase`] for in-range input.
pub fn try_w_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Result<Velocity> {
    check_tp_single_phase_envelope(t, p)?;
    Ok(w_tp_eqm_single_phase(t, p))
}

/// Checked isentropic exponent kappa (dimensionless `Ratio`) from a
/// single-phase `(T,p)` flash. Valid range: the `(T,p)` envelope in the
/// module doc. Agrees exactly with [`kappa_tp_eqm_single_phase`] for
/// in-range input.
pub fn try_kappa_tp_eqm_single_phase(t: ThermodynamicTemperature, p: Pressure) -> Result<Ratio> {
    check_tp_single_phase_envelope(t, p)?;
    Ok(kappa_tp_eqm_single_phase(t, p))
}

/// Checked dynamic viscosity mu (Pa s, IAPWS R12-08 fast path without the
/// critical enhancement) from a single-phase `(T,p)` flash. Valid range:
/// the `(T,p)` envelope in the module doc. Agrees exactly with
/// [`mu_tp_eqm_single_phase`] for in-range input.
pub fn try_mu_tp_eqm_single_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
) -> Result<DynamicViscosity> {
    check_tp_single_phase_envelope(t, p)?;
    Ok(mu_tp_eqm_single_phase(t, p))
}

/// Checked thermal conductivity lambda (W/(m K), IAPWS R15-11) from a
/// single-phase `(T,p)` flash. Valid range: the `(T,p)` envelope in the
/// module doc. Agrees exactly with [`lambda_tp_eqm_single_phase`] for
/// in-range input.
pub fn try_lambda_tp_eqm_single_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
) -> Result<ThermalConductivity> {
    check_tp_single_phase_envelope(t, p)?;
    Ok(lambda_tp_eqm_single_phase(t, p))
}

// ========================= (p,h) facade ==============================

/// Checked temperature T (K) from a `(p,h)` flash (Regions 1-4; Region 5
/// has no IAPWS-IF97 backward `(p,h)` correlation and lies outside the
/// enthalpy window). Valid range: the `(p,h)` envelope in the module doc.
/// Agrees exactly with [`t_ph_eqm`] for in-range input.
pub fn try_t_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<ThermodynamicTemperature> {
    check_ph_envelope(p, h)?;
    Ok(t_ph_eqm(p, h))
}

/// Checked specific volume v (m^3/kg) from a `(p,h)` flash (two-phase
/// states return the quality-weighted mixture volume). Valid range: the
/// `(p,h)` envelope in the module doc. Agrees exactly with [`v_ph_eqm`]
/// for in-range input.
pub fn try_v_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<SpecificVolume> {
    check_ph_envelope(p, h)?;
    Ok(v_ph_eqm(p, h))
}

/// Checked mass density rho (kg/m^3) from a `(p,h)` flash — the
/// reciprocal of [`try_v_ph_eqm`]. Valid range: the `(p,h)` envelope in
/// the module doc.
pub fn try_rho_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<MassDensity> {
    check_ph_envelope(p, h)?;
    Ok(v_ph_eqm(p, h).recip())
}

/// Checked specific internal energy u (J/kg) from a `(p,h)` flash. Valid
/// range: the `(p,h)` envelope in the module doc. Agrees exactly with
/// [`u_ph_eqm`] for in-range input.
pub fn try_u_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<AvailableEnergy> {
    check_ph_envelope(p, h)?;
    Ok(u_ph_eqm(p, h))
}

/// Checked specific entropy s (J/(kg K)) from a `(p,h)` flash. Valid
/// range: the `(p,h)` envelope in the module doc. Agrees exactly with
/// [`s_ph_eqm`] for in-range input.
pub fn try_s_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<SpecificHeatCapacity> {
    check_ph_envelope(p, h)?;
    Ok(s_ph_eqm(p, h))
}

/// Checked isobaric specific heat capacity cp (J/(kg K)) from a `(p,h)`
/// flash (two-phase states return a quality-interpolated estimate — see
/// [`cp_ph_eqm`]). Valid range: the `(p,h)` envelope in the module doc.
/// Agrees exactly with [`cp_ph_eqm`] for in-range input.
pub fn try_cp_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<SpecificHeatCapacity> {
    check_ph_envelope(p, h)?;
    Ok(cp_ph_eqm(p, h))
}

/// Checked isochoric specific heat capacity cv (J/(kg K)) from a `(p,h)`
/// flash (two-phase states return a quality-interpolated estimate — see
/// [`cv_ph_eqm`]). Valid range: the `(p,h)` envelope in the module doc.
/// Agrees exactly with [`cv_ph_eqm`] for in-range input.
pub fn try_cv_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<SpecificHeatCapacity> {
    check_ph_envelope(p, h)?;
    Ok(cv_ph_eqm(p, h))
}

/// Checked speed of sound w (m/s) from a `(p,h)` flash; in the two-phase
/// region this is the Wood/Wallis homogeneous-mixture sound speed. Valid
/// range: the `(p,h)` envelope in the module doc. Agrees exactly with
/// [`w_ph_wood_wallis`] for in-range input.
pub fn try_w_ph_wood_wallis(p: Pressure, h: AvailableEnergy) -> Result<Velocity> {
    check_ph_envelope(p, h)?;
    Ok(w_ph_wood_wallis(p, h))
}

/// Checked isentropic exponent kappa (dimensionless `Ratio`) from a
/// `(p,h)` flash. Valid range: the `(p,h)` envelope in the module doc.
/// Agrees exactly with [`kappa_ph_eqm`] for in-range input.
pub fn try_kappa_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<Ratio> {
    check_ph_envelope(p, h)?;
    Ok(kappa_ph_eqm(p, h))
}

/// Checked steam quality x (dimensionless, bare `f64` to agree exactly
/// with the unchecked [`x_ph_flash`]): 0 for subcooled liquid, 1 for
/// superheated vapour, in `(0, 1)` inside the vapour-liquid dome. Valid
/// range: the `(p,h)` envelope in the module doc.
pub fn try_x_ph_flash(p: Pressure, h: AvailableEnergy) -> Result<f64> {
    check_ph_envelope(p, h)?;
    Ok(x_ph_flash(p, h))
}

/// Checked dynamic viscosity mu (Pa s, IAPWS R12-08 fast path) from a
/// `(p,h)` flash (two-phase states use the HEM-mixture density). Valid
/// range: the `(p,h)` envelope in the module doc. Agrees exactly with
/// [`mu_ph_eqm`] for in-range input.
pub fn try_mu_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<DynamicViscosity> {
    check_ph_envelope(p, h)?;
    Ok(mu_ph_eqm(p, h))
}

/// Checked thermal conductivity lambda (W/(m K), IAPWS R15-11) from a
/// `(p,h)` flash. Valid range: the `(p,h)` envelope in the module doc.
/// Agrees exactly with [`lambda_ph_eqm`] for in-range input.
pub fn try_lambda_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<ThermalConductivity> {
    check_ph_envelope(p, h)?;
    Ok(lambda_ph_eqm(p, h))
}

#[cfg(test)]
mod tests;
