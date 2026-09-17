//! Bounds-checked two-phase `(T, p, x)` flash facade (bead `op-dt3.26`).
//!
//! The `*_tp_eqm_two_phase` family in
//! [`crate::interfaces::functional_programming::pt_flash_eqm::multiphase_flashing`]
//! is the quality-carrying counterpart of the single-phase `(T,p)` flashes:
//! it resolves saturation-line states that
//! [`super::check_tp_single_phase_envelope`] has to reject as
//! under-determined. These functions **panic** on out-of-envelope `(T,p)`,
//! and silently *clamp* an out-of-range steam quality; this module rejects
//! both before the internals run.
//!
//! ## Panic-trace: every reachable `panic!` and the gate that excludes it
//!
//! Traced against the source on 2026-08-11. Each `*_tp_eqm_two_phase`
//! function begins with `region_fwd_eqn_two_phase(t, p, x)` and then
//! `match`es on the returned region. Unlike the single-phase family, the
//! `Region4` arm is **implemented** in every one of these functions, so the
//! `REGION_4_TP_UNDERDETERMINED` panics do not apply here.
//!
//! | Panic site | Condition | Excluded by |
//! |---|---|---|
//! | `pt_flash_eqm/mod.rs:157` "t,p flashing at eqm out of bounds!" | `region_fwd_eqn_two_phase` falls through to `region_fwd_eqn_single_phase` (i.e. `(T,p)` is off the saturation line, or above the critical temperature/pressure) with `(T,p)` outside the IF97 envelope | the `(T,p)` envelope gate below: `T` in `[273.15 K, 2273.15 K]`, `0 < p <= 100 MPa` for `T <= 1073.15 K`, `0 < p <= 50 MPa` above it |
//! | `pt_flash_eqm/mod.rs:171,184,198,211,225,242,256,321,334,348` `REGION_4_TP_UNDERDETERMINED` | the single-phase `(T,p)` functions' Region-4 arms | **not reachable from this family** — these functions never call the single-phase property functions; they call `region_fwd_eqn_single_phase` (the router) only, and handle `Region4` themselves |
//!
//! The Region-3 arms call the near-saturation backward volume equations
//! (`v_tp_3c`, `v_tp_3r`, `v_tp_3s`, `v_tp_3t`, `v_tp_3u`, `v_tp_3x`,
//! `v_tp_3y`, `v_tp_3z`) and `h_rho_t_3`/`s_rho_t_3`/... at Region-3 points
//! that may lie far from the saturation line. Those are closed-form
//! polynomial evaluations containing no `panic!`, `todo!`, `unwrap` or
//! `expect` (verified by grep over `region_1_..` through `region_5_..` and
//! `backward_eqn_*`, which return zero hits), so they can lose accuracy but
//! cannot panic — and the result is discarded unless the point really is
//! near the saturation line.
//!
//! ## Steam quality is rejected, not clamped
//!
//! `region_fwd_eqn_two_phase` and each Region-3/Region-4 arm silently clamp
//! `x` into `[0, 1]`. That is a correctness hazard rather than a panic: a
//! caller passing `x = 1.7` gets a saturated-vapour answer with no
//! indication that the input was nonsense. The checked facade rejects
//! `x < 0` and `x > 1` with [`SteamTablesError::QualityOutOfRange`], while
//! **accepting `x = 0` and `x = 1` exactly** — those are the physically
//! meaningful bubble- and dew-point states the internals route to
//! Region 1/2 (below 623.15 K) or Region 3 (above it).
//!
//! `NaN` quality is worse: it survives every clamp (`NaN < 0.0` and
//! `NaN > 1.0` are both `false`), fails the `x == 0.0` / `x == 1.0`
//! bubble/dew tests, and propagates into the mixture weighting to return a
//! silent `NaN`. It is rejected explicitly as
//! [`SteamTablesError::NonFinite`].
//!
//! ## Difference from the single-phase `(T,p)` gate
//!
//! [`check_tpx_envelope`] is deliberately **not**
//! [`super::check_tp_single_phase_envelope`] plus a quality check: it omits
//! that gate's `SaturatedTpUnderdetermined` rejection. A `(T, p_sat(T))`
//! pair is exactly what this family exists to evaluate, and accepting it is
//! the whole point.
//!
//! ## No `catch_unwind`
//!
//! This facade is a bounds check, not an exception handler.

use uom::si::f64::*;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;

use crate::interfaces::functional_programming::pt_flash_eqm::multiphase_flashing::{
    alpha_v_tp_eqm_two_phase, cp_tp_eqm_two_phase, cv_tp_eqm_two_phase, h_tp_eqm_two_phase,
    kappa_t_tp_eqm as kappa_t_tp_eqm_two_phase, kappa_tp_eqm_two_phase, s_tp_eqm_two_phase,
    u_tp_eqm_two_phase, v_tp_eqm_two_phase, w_tp_eqm_two_phase,
};
use crate::region_1_subcooled_liquid::InversePressure;

use super::{
    Result, SteamTablesError, P_MAX_PASCAL, P_MAX_R5_PASCAL, T_MAX_KELVIN, T_MIN_KELVIN,
    T_R5_LOWER_KELVIN,
};

/// Validates a `(T, p, x)` triple against the envelope the unchecked
/// two-phase `(T,p,x)` internals actually accept, returning `Ok(())` when
/// every wrapped `try_*_tp_eqm_two_phase` function is safe to call.
///
/// # Physical quantities and valid ranges
///
/// - `t` — thermodynamic temperature, K. Valid in `[273.15, 2273.15]`.
/// - `p` — absolute pressure, Pa. Valid in `(0, 100 MPa]` for `t` in
///   `[273.15 K, 1073.15 K]`, and in `(0, 50 MPa]` for `t` above
///   1073.15 K (IF97 Region 5). Both ceilings are **inclusive**; the floor
///   is exclusive at 0 (vacuum has no IF97 state).
/// - `x` — steam quality, i.e. vapour mass fraction, dimensionless. Valid
///   in `[0, 1]`, **both edges inclusive**. It only affects the answer for
///   `(T,p)` pairs on (or within `5e-4` relative pressure of) the
///   saturation line; elsewhere the underlying single-phase equations
///   ignore it.
///
/// Unlike [`super::check_tp_single_phase_envelope`] this check **accepts**
/// saturation-line `(T,p)` pairs — resolving them is what the two-phase
/// family is for.
///
/// Non-finite input is rejected first with
/// [`SteamTablesError::NonFinite`].
pub fn check_tpx_envelope(t: ThermodynamicTemperature, p: Pressure, x: f64) -> Result<()> {
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
    if !x.is_finite() {
        return Err(SteamTablesError::NonFinite {
            quantity: "steam quality",
            value: x,
            unit: "dimensionless",
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
    let p_max = if t_kelvin > T_R5_LOWER_KELVIN {
        P_MAX_R5_PASCAL
    } else {
        P_MAX_PASCAL
    };
    if p_pascal <= 0.0 || p_pascal > p_max {
        return Err(SteamTablesError::OutOfRange {
            quantity: "pressure",
            value: p_pascal,
            min: 0.0,
            max: p_max,
            unit: "Pa",
        });
    }
    // The internals CLAMP an out-of-range quality silently; reject instead.
    if !(0.0..=1.0).contains(&x) {
        return Err(SteamTablesError::QualityOutOfRange { x });
    }
    Ok(())
}

/// Checked specific enthalpy h (J/kg) from a two-phase-aware `(T,p,x)`
/// flash. Valid range: the `(T,p,x)` envelope in [`check_tpx_envelope`]
/// (saturation-line pairs accepted; `x` in `[0, 1]`). Agrees exactly with
/// [`h_tp_eqm_two_phase`] for in-range input.
pub fn try_h_tp_eqm_two_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
    x: f64,
) -> Result<AvailableEnergy> {
    check_tpx_envelope(t, p, x)?;
    Ok(h_tp_eqm_two_phase(t, p, x))
}

/// Checked specific internal energy u (J/kg) from a two-phase-aware
/// `(T,p,x)` flash. Valid range: the `(T,p,x)` envelope in
/// [`check_tpx_envelope`]. Agrees exactly with [`u_tp_eqm_two_phase`] for
/// in-range input.
pub fn try_u_tp_eqm_two_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
    x: f64,
) -> Result<AvailableEnergy> {
    check_tpx_envelope(t, p, x)?;
    Ok(u_tp_eqm_two_phase(t, p, x))
}

/// Checked specific entropy s (J/(kg K)) from a two-phase-aware `(T,p,x)`
/// flash. Valid range: the `(T,p,x)` envelope in [`check_tpx_envelope`].
/// Agrees exactly with [`s_tp_eqm_two_phase`] for in-range input.
pub fn try_s_tp_eqm_two_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
    x: f64,
) -> Result<SpecificHeatCapacity> {
    check_tpx_envelope(t, p, x)?;
    Ok(s_tp_eqm_two_phase(t, p, x))
}

/// Checked isobaric specific heat capacity cp (J/(kg K)) from a
/// two-phase-aware `(T,p,x)` flash (two-phase states return a
/// quality-weighted mixture value). Valid range: the `(T,p,x)` envelope in
/// [`check_tpx_envelope`]. Agrees exactly with [`cp_tp_eqm_two_phase`] for
/// in-range input.
pub fn try_cp_tp_eqm_two_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
    x: f64,
) -> Result<SpecificHeatCapacity> {
    check_tpx_envelope(t, p, x)?;
    Ok(cp_tp_eqm_two_phase(t, p, x))
}

/// Checked isochoric specific heat capacity cv (J/(kg K)) from a
/// two-phase-aware `(T,p,x)` flash. Valid range: the `(T,p,x)` envelope in
/// [`check_tpx_envelope`]. Agrees exactly with [`cv_tp_eqm_two_phase`] for
/// in-range input.
pub fn try_cv_tp_eqm_two_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
    x: f64,
) -> Result<SpecificHeatCapacity> {
    check_tpx_envelope(t, p, x)?;
    Ok(cv_tp_eqm_two_phase(t, p, x))
}

/// Checked specific volume v (m^3/kg) from a two-phase-aware `(T,p,x)`
/// flash (two-phase states return the quality-weighted mixture volume).
/// Valid range: the `(T,p,x)` envelope in [`check_tpx_envelope`]. Agrees
/// exactly with [`v_tp_eqm_two_phase`] for in-range input.
pub fn try_v_tp_eqm_two_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
    x: f64,
) -> Result<SpecificVolume> {
    check_tpx_envelope(t, p, x)?;
    Ok(v_tp_eqm_two_phase(t, p, x))
}

/// Checked mass density rho (kg/m^3) from a two-phase-aware `(T,p,x)`
/// flash — the reciprocal of [`try_v_tp_eqm_two_phase`]. Valid range: the
/// `(T,p,x)` envelope in [`check_tpx_envelope`].
pub fn try_rho_tp_eqm_two_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
    x: f64,
) -> Result<MassDensity> {
    check_tpx_envelope(t, p, x)?;
    Ok(v_tp_eqm_two_phase(t, p, x).recip())
}

/// Checked speed of sound w (m/s) from a two-phase-aware `(T,p,x)` flash.
/// Valid range: the `(T,p,x)` envelope in [`check_tpx_envelope`]. Agrees
/// exactly with [`w_tp_eqm_two_phase`] for in-range input.
pub fn try_w_tp_eqm_two_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
    x: f64,
) -> Result<Velocity> {
    check_tpx_envelope(t, p, x)?;
    Ok(w_tp_eqm_two_phase(t, p, x))
}

/// Checked isentropic exponent kappa (dimensionless `Ratio`) from a
/// two-phase-aware `(T,p,x)` flash. Valid range: the `(T,p,x)` envelope in
/// [`check_tpx_envelope`]. Agrees exactly with [`kappa_tp_eqm_two_phase`]
/// for in-range input.
pub fn try_kappa_tp_eqm_two_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
    x: f64,
) -> Result<Ratio> {
    check_tpx_envelope(t, p, x)?;
    Ok(kappa_tp_eqm_two_phase(t, p, x))
}

/// Checked isobaric cubic expansion coefficient alpha_v (1/K) from a
/// two-phase-aware `(T,p,x)` flash. Valid range: the `(T,p,x)` envelope in
/// [`check_tpx_envelope`]. Agrees exactly with
/// [`alpha_v_tp_eqm_two_phase`] for in-range input.
pub fn try_alpha_v_tp_eqm_two_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
    x: f64,
) -> Result<TemperatureCoefficient> {
    check_tpx_envelope(t, p, x)?;
    Ok(alpha_v_tp_eqm_two_phase(t, p, x))
}

/// Checked isothermal compressibility kappa_T (1/Pa) from a
/// two-phase-aware `(T,p,x)` flash. Valid range: the `(T,p,x)` envelope in
/// [`check_tpx_envelope`]. Agrees exactly with
/// `multiphase_flashing::kappa_t_tp_eqm` for in-range input.
pub fn try_kappa_t_tp_eqm_two_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
    x: f64,
) -> Result<InversePressure> {
    check_tpx_envelope(t, p, x)?;
    Ok(kappa_t_tp_eqm_two_phase(t, p, x))
}
