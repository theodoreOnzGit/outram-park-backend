//! Bounds-checked constructors for the object-oriented
//! [`TampinesSteamTableCV`] control volume (bead `op-dt3.26`).
//!
//! [`TampinesSteamTableCV`]'s `new_from_*` constructors are thin composers
//! over the functional-programming flashes, so they inherit every panic
//! those flashes have. The free functions here are **additive**: they gate
//! the same inputs with the validators in this module's siblings and then
//! call the existing constructor unchanged. No existing signature is
//! touched, and the struct's fields stay private — these wrappers add a
//! `Result` entry point, they do not re-implement the flash.
//!
//! ## Panic-trace: constructor to gate
//!
//! Traced against `interfaces/object_oriented_programming/mod.rs` on
//! 2026-08-11. Each row names the internal calls the constructor makes and
//! the validator that excludes their panics.
//!
//! | Constructor | Internal calls | Gate |
//! |---|---|---|
//! | `new_from_tp_quality(t, p, volume, x)` | `v_tp_eqm_two_phase`, `h_tp_eqm_two_phase`, `s_tp_eqm_two_phase` | [`check_tpx_envelope`] — see [`super::two_phase`] for that family's full panic trace |
//! | `new_from_tp_quality_1(t, p, volume)` | same, with `x = 1` | [`check_tpx_envelope`] at `x = 1` |
//! | `new_from_tp_quality_0(t, p, volume)` | same, with `x = 0` | [`check_tpx_envelope`] at `x = 0` |
//! | `new_from_ph(p, h, volume)` | `t_ph_eqm`, `v_ph_eqm`, `s_ph_eqm` | [`super::check_ph_envelope`] |
//! | `new_from_ps(p, s, volume)` | `t_ps_eqm`, `v_ps_eqm`, `h_ps_eqm` | [`check_ps_envelope`] — see [`super::ps`] |
//! | `new_from_sat_pressure_quality(p, x, volume)` | `sat_temp_4(p)`, then `new_from_tp_quality` | [`check_tpx_envelope`] at `(sat_temp_4(p), p, x)`; `sat_temp_4` itself is a closed-form correlation with no panic site |
//! | `new_from_sat_temp_quality(t, x, volume)` | `sat_pressure_4(t)`, then `new_from_tp_quality` | [`check_tpx_envelope`] at `(t, sat_pressure_4(t), x)`; `sat_pressure_4` is likewise panic-free |
//!
//! ## Not covered here
//!
//! - **`new_from_hs`** — it resolves pressure with `hs_flash_eqm::p_hs_eqm`
//!   first, so it inherits the `(h,s)` flash's panic surface, which this
//!   module does not yet gate. It is deliberately left out rather than
//!   given a gate that would miss those panics.
//! - **The setter methods and the getters** (`set_tpx`, `set_ph`,
//!   `set_ps`, `compress_isentropically`, `get_crit_pressure_and_massflux`,
//!   ...) — the state-changing ones inherit the same flash panics, and
//!   `get_crit_pressure_and_massflux` additionally reaches the choked-flow
//!   root finder, whose panic is a *convergence* failure that no input
//!   bounds check can exclude.
//!
//! ## No `catch_unwind`
//!
//! This facade is a bounds check, not an exception handler.

use uom::si::f64::*;

use crate::interfaces::object_oriented_programming::TampinesSteamTableCV;
use crate::region_4_vap_liq_equilibrium::{sat_pressure_4, sat_temp_4};

use super::ps::check_ps_envelope;
use super::two_phase::check_tpx_envelope;
use super::{check_ph_envelope, Result};

/// Checked [`TampinesSteamTableCV::new_from_tp_quality`]: builds a control
/// volume from a two-phase-aware `(T, p, x)` flash.
///
/// Inputs: `temperature` in K (valid 273.15-2273.15), `pressure` in Pa
/// (valid up to and including 100 MPa below 1073.15 K, 50 MPa above),
/// `volume` the fixed control-volume size in m^3 (not validated — any
/// finite volume is geometrically meaningful and no internal reads it
/// during the flash), and `x` the steam quality (vapour mass fraction,
/// valid in `[0, 1]` inclusive). Saturation-line `(T,p)` pairs are
/// accepted: resolving them with an explicit quality is the point of this
/// constructor.
pub fn try_new_from_tp_quality(
    temperature: ThermodynamicTemperature,
    pressure: Pressure,
    volume: Volume,
    x: f64,
) -> Result<TampinesSteamTableCV> {
    check_tpx_envelope(temperature, pressure, x)?;
    Ok(TampinesSteamTableCV::new_from_tp_quality(
        temperature,
        pressure,
        volume,
        x,
    ))
}

/// Checked [`TampinesSteamTableCV::new_from_tp_quality_1`]: builds a
/// control volume from a `(T, p)` flash with steam quality fixed at 1
/// (saturated vapour / dew point on the saturation line, ignored
/// elsewhere). Same `(T,p)` envelope as [`try_new_from_tp_quality`].
pub fn try_new_from_tp_quality_1(
    temperature: ThermodynamicTemperature,
    pressure: Pressure,
    volume: Volume,
) -> Result<TampinesSteamTableCV> {
    check_tpx_envelope(temperature, pressure, 1.0)?;
    Ok(TampinesSteamTableCV::new_from_tp_quality_1(
        temperature,
        pressure,
        volume,
    ))
}

/// Checked [`TampinesSteamTableCV::new_from_tp_quality_0`]: builds a
/// control volume from a `(T, p)` flash with steam quality fixed at 0
/// (saturated liquid / bubble point on the saturation line, ignored
/// elsewhere). Same `(T,p)` envelope as [`try_new_from_tp_quality`].
pub fn try_new_from_tp_quality_0(
    temperature: ThermodynamicTemperature,
    pressure: Pressure,
    volume: Volume,
) -> Result<TampinesSteamTableCV> {
    check_tpx_envelope(temperature, pressure, 0.0)?;
    Ok(TampinesSteamTableCV::new_from_tp_quality_0(
        temperature,
        pressure,
        volume,
    ))
}

/// Checked [`TampinesSteamTableCV::new_from_ph`]: builds a control volume
/// from a `(p, h)` flash.
///
/// Inputs: `p` in Pa (valid `[p_sat(273.15 K), 100 MPa]`, both edges
/// inclusive), `h` in J/kg (valid between the 273.15 K and 1073.15 K
/// isotherm enthalpies at that pressure), `volume` in m^3. See
/// [`super::check_ph_envelope`] for the exact bounds.
pub fn try_new_from_ph(
    p: Pressure,
    h: AvailableEnergy,
    volume: Volume,
) -> Result<TampinesSteamTableCV> {
    check_ph_envelope(p, h)?;
    Ok(TampinesSteamTableCV::new_from_ph(p, h, volume))
}

/// Checked [`TampinesSteamTableCV::new_from_ps`]: builds a control volume
/// from a `(p, s)` flash.
///
/// Inputs: `p` in Pa (valid `(p_sat(273.15 K), 100 MPa]` — note the
/// **exclusive** lower edge, see [`super::ps`]), `s` in J/(kg K) (valid
/// between the 273.15 K and 1073.15 K isotherm entropies at that
/// pressure), `volume` in m^3.
pub fn try_new_from_ps(
    p: Pressure,
    s: SpecificHeatCapacity,
    volume: Volume,
) -> Result<TampinesSteamTableCV> {
    check_ps_envelope(p, s)?;
    Ok(TampinesSteamTableCV::new_from_ps(p, s, volume))
}

/// Checked [`TampinesSteamTableCV::new_from_sat_pressure_quality`]: builds
/// a saturation-line control volume from saturation pressure and quality.
///
/// Inputs: `p` the saturation pressure in Pa, `x` the steam quality
/// (dimensionless, valid `[0, 1]` inclusive), `volume` in m^3. The
/// saturation temperature is looked up with `sat_temp_4(p)` and the
/// resulting `(T, p, x)` triple is validated with
/// [`check_tpx_envelope`] — so a pressure whose saturation temperature
/// falls outside `[273.15 K, 2273.15 K]` is rejected here rather than
/// panicking downstream.
pub fn try_new_from_sat_pressure_quality(
    p: Pressure,
    x: f64,
    volume: Volume,
) -> Result<TampinesSteamTableCV> {
    let t = sat_temp_4(p);
    check_tpx_envelope(t, p, x)?;
    Ok(TampinesSteamTableCV::new_from_sat_pressure_quality(
        p, x, volume,
    ))
}

/// Checked [`TampinesSteamTableCV::new_from_sat_temp_quality`]: builds a
/// saturation-line control volume from saturation temperature and quality.
///
/// Inputs: `t` the saturation temperature in K, `x` the steam quality
/// (dimensionless, valid `[0, 1]` inclusive), `volume` in m^3. The
/// saturation pressure is looked up with `sat_pressure_4(t)` and the
/// resulting `(T, p, x)` triple is validated with
/// [`check_tpx_envelope`].
pub fn try_new_from_sat_temp_quality(
    t: ThermodynamicTemperature,
    x: f64,
    volume: Volume,
) -> Result<TampinesSteamTableCV> {
    let p = sat_pressure_4(t);
    check_tpx_envelope(t, p, x)?;
    Ok(TampinesSteamTableCV::new_from_sat_temp_quality(
        t, x, volume,
    ))
}
