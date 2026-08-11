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
//! - `p` in `(0, 100 MPa]` for `T` in `[273.15 K, 1073.15 K]` — the
//!   100 MPa ceiling is **inclusive at every temperature in this band**.
//!   (Between this module landing on 2026-08-11 and the `op-cv1c` fix
//!   later the same day, the edge was exclusive above 623.15 K, because
//!   the internal region router's Region-2 and Region-3 arms used
//!   half-open ranges (`..100e6`) and exactly 100 MPa fell through to
//!   their `panic!`. The router now closes those arms with `..=100e6`, so
//!   the facade no longer has to carve the point out.)
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
//! - `p` in `[p_sat(273.15 K) ≈ 611.213 Pa, 100 MPa]` — the upper edge is
//!   **inclusive**, matching `is_outside_pressure_range`, which rejects
//!   only `p > 100 MPa`. (It was exclusive here until `op-cv1c`: the
//!   internal 1073.15 K-isotherm bound helper
//!   `is_above_isotherm_t_1073_15` evaluates
//!   `h_tp_eqm_single_phase(1073.15 K, p)`, so the router's half-open
//!   100 MPa edge made *every* `(p,h)` call at exactly 100 MPa panic
//!   regardless of `h`. With the router fixed that call succeeds.);
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
//! ## Other families covered by this facade
//!
//! Three more families were added by bead `op-dt3.26`; each lives in its
//! own submodule with its own panic-site-to-gate table, and each is
//! re-exported here so `interfaces::checked::*` remains the single import:
//!
//! - [`ps`] — the `(p,s)` flash family (11 functions plus the throat mass
//!   flux) and [`check_ps_envelope`]. Note its pressure floor is
//!   **exclusive** where the `(p,h)` one is inclusive: the `(p,s)` validity
//!   check evaluates `s_tp_eqm_single_phase` on the 273.15 K isotherm and
//!   so hits the Region-4 trap at exactly `p = p_sat(273.15 K)`.
//! - [`two_phase`] — the quality-carrying `(T,p,x)` family (11 functions)
//!   and [`check_tpx_envelope`]. This one **accepts** saturation-line
//!   `(T,p)` pairs, which the single-phase gate above must reject, and it
//!   rejects a steam quality outside `[0, 1]` that the internals would
//!   silently clamp.
//! - [`control_volume`] — additive `Result`-returning constructors for
//!   [`crate::interfaces::object_oriented_programming::TampinesSteamTableCV`].
//!
//! Two `(T,p)` and two `(p,h)` properties that the original facade did not
//! reach — the isobaric cubic expansion coefficient `alpha_v` and the
//! isothermal compressibility `kappa_T` — are wrapped below against the
//! existing validators.
//!
//! ## Known gaps (deliberately not gated)
//!
//! Nothing here half-gates: a family with a panic this facade cannot
//! exclude is left out entirely rather than given a check that would claim
//! a safety it does not provide.
//!
//! - **The `(h,s)` flash family** (`t_hs_eqm`, `p_hs_eqm`, `v_hs_eqm`,
//!   `x_hs_eqm`, `cp_hs_eqm`, `w_hs_eqm`, `kappa_hs_eqm`, `mu_hs_eqm`,
//!   `lambda_hs_eqm`, `tpvx_hs_flash_eqm`, `hs_flash_region`,
//!   `find_pressure_from_hs_region_4`) — 26 distinct `panic!`/`todo!`
//!   sites spread over nine entropy-band sub-routers, whose enthalpy
//!   bounds are themselves computed by `(p,s)` flashes, plus a bisection
//!   that panics when its bracket fails. Gating it faithfully means
//!   mirroring the whole band dispatch, not adding a bounds check.
//! - **`TampinesSteamTableCV::new_from_hs`** and the setter methods, for
//!   the same reason (see [`control_volume`]).
//! - **The choked-flow solvers** in
//!   [`crate::steam_turbine_equations::converging_diverging_nozzles`], and
//!   `TampinesSteamTableCV::get_crit_pressure_and_massflux` which calls
//!   them. Their panics — "unable to find bracket for critical mass flux
//!   root finding", the Joule-Thomson "bounds are same sign!" and "failed
//!   to converge" — are **iteration failures, not envelope violations**.
//!   No check on the stagnation state can exclude them without running the
//!   solver, so the honest fix is for those functions to return `Result`,
//!   not for a facade to pretend it can screen the input. The one piece
//!   that *is* an envelope check, the throat mass flux, is wrapped as
//!   [`ps::try_mass_flux_ps_eqm_throat`].
//! - **`pt_flash_metastable`** — its `mod.rs` is empty (zero lines); the
//!   metastable Region-2 equations live in
//!   `region_2_vapour::metastable_region_2` and are reached through
//!   `TampinesSteamTableCV::get_metastable_steam_*`, which already return
//!   `Option`. There is no panicking entry point in that module to gate.
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
//! returned as a value, not a panic). This was confirmed by grep over
//! every `region_*` and `backward_eqn_*` module on 2026-08-11: zero
//! `panic!`, `todo!`, `unimplemented!`, `unwrap` or `expect` sites. The
//! whole panic surface of this crate's property layer lives in the
//! `interfaces/` routers.

use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::*;
use uom::si::pressure::pascal;
use uom::si::thermodynamic_temperature::kelvin;

use crate::region_1_subcooled_liquid::h_tp_1;
use crate::region_4_vap_liq_equilibrium::sat_pressure_4;

use super::functional_programming::ph_flash_eqm::{
    alpha_v_ph_eqm, cp_ph_eqm, cv_ph_eqm, kappa_ph_eqm, kappa_t_ph_eqm, lambda_ph_eqm, mu_ph_eqm,
    s_ph_eqm, t_ph_eqm, u_ph_eqm, v_ph_eqm, w_ph_wood_wallis, x_ph_flash,
};
use super::functional_programming::pt_flash_eqm::{
    alpha_v_tp_eqm_single_phase, cp_tp_eqm_single_phase, cv_tp_eqm_single_phase,
    h_tp_eqm_single_phase, kappa_t_tp_eqm, kappa_tp_eqm_single_phase, s_tp_eqm_single_phase,
    u_tp_eqm_single_phase, v_tp_eqm_single_phase, w_tp_eqm_single_phase,
};
use crate::dynamic_viscosity::mu_tp_eqm_single_phase;
use crate::region_1_subcooled_liquid::InversePressure;
use crate::thermal_conductivity::lambda_tp_eqm_single_phase;

/// The `(p,s)` flash family — see the module's own docs for its panic
/// trace and its **exclusive** pressure floor.
pub mod ps;
pub use ps::*;

/// The quality-carrying two-phase `(T,p,x)` flash family — see the
/// module's own docs for its panic trace.
pub mod two_phase;
pub use two_phase::*;

/// Additive `Result`-returning constructors for the object-oriented
/// `TampinesSteamTableCV` control volume.
pub mod control_volume;
pub use control_volume::*;

/// Minimum IAPWS-IF97 temperature, K (Regions 1-4 lower bound).
pub(crate) const T_MIN_KELVIN: f64 = 273.15;
/// Region 1-4 / Region 5 boundary isotherm, K.
pub(crate) const T_R5_LOWER_KELVIN: f64 = 1073.15;
/// Maximum IAPWS-IF97 temperature, K (Region 5 upper bound).
pub(crate) const T_MAX_KELVIN: f64 = 2273.15;
/// Critical temperature, K — upper end of the saturation line; the exact
/// `p == p_sat(T)` Region-4 trap only exists below it.
const T_CRIT_KELVIN: f64 = 647.096;
/// Maximum IAPWS-IF97 pressure for Regions 1-4, Pa.
pub(crate) const P_MAX_PASCAL: f64 = 100.0e6;
/// Maximum IAPWS-IF97 pressure for Region 5, Pa.
pub(crate) const P_MAX_R5_PASCAL: f64 = 50.0e6;

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
    /// The steam quality (vapour mass fraction) lies outside `[0, 1]`.
    ///
    /// This is not a panic the internals raise — the two-phase `(T,p,x)`
    /// functions silently **clamp** an out-of-range quality, so a caller
    /// passing `x = 1.7` gets a saturated-vapour answer with no signal that
    /// the input was nonsense. The checked facade rejects it instead.
    /// `x = 0` (bubble point) and `x = 1` (dew point) are valid and
    /// accepted.
    #[error(
        "steam quality x = {x} is outside [0, 1]; the unchecked two-phase \
         (T,p,x) functions would silently clamp it"
    )]
    QualityOutOfRange {
        /// The offending quality, dimensionless (vapour mass fraction).
        x: f64,
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
/// `p` an absolute pressure (valid up to and including 100 MPa from
/// 273.15 K to 1073.15 K, up to and including 50 MPa above 1073.15 K).
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
    // Pressure ceiling depends on the temperature band; both ceilings are
    // INCLUSIVE (IF97 is defined at exactly 100 MPa / 50 MPa — see the
    // module doc). The lower edge is exclusive at 0 everywhere (vacuum
    // has no IF97 state).
    let p_max = if t_kelvin > T_R5_LOWER_KELVIN {
        // Region 5 band (1073.15 K, 2273.15 K]: p <= 50 MPa.
        P_MAX_R5_PASCAL
    } else {
        // [273.15 K, 1073.15 K]: p <= 100 MPa, Region 1/2/3 alike.
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
    // Exact saturation-line hit below the critical temperature routes to
    // Region 4 inside the internals, whose single-phase (T,p) arms panic
    // (under-determined without steam quality) — reject it here instead.
    if t_kelvin < T_CRIT_KELVIN && p_pascal == sat_pressure_4(t).get::<pascal>() {
        return Err(SteamTablesError::SaturatedTpUnderdetermined { t_kelvin, p_pascal });
    }
    Ok(())
}

/// Validates a `(p, h)` pair against the envelope the unchecked `(p,h)`
/// internals actually accept: `p` in `[p_sat(273.15 K), 100 MPa]` (both
/// edges inclusive, matching `is_outside_pressure_range`) and `h` between
/// the 273.15 K and 1073.15 K isotherm enthalpies at that pressure.
/// Returns `Ok(())` when every wrapped `try_*_ph_*` function is safe to
/// call.
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
    // Upper edge INCLUSIVE: 100 MPa exactly is a valid IF97 pressure and
    // the internals accept it (module-level doc, "(p,h) flashes"
    // bullet 1).
    if p_pascal < p_min_pascal || p_pascal > P_MAX_PASCAL {
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
pub fn try_h_tp_eqm_single_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
) -> Result<AvailableEnergy> {
    check_tp_single_phase_envelope(t, p)?;
    Ok(h_tp_eqm_single_phase(t, p))
}

/// Checked specific internal energy u (J/kg) from a single-phase `(T,p)`
/// flash. Valid range: the `(T,p)` envelope in the module doc. Agrees
/// exactly with [`u_tp_eqm_single_phase`] for in-range input.
pub fn try_u_tp_eqm_single_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
) -> Result<AvailableEnergy> {
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
pub fn try_v_tp_eqm_single_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
) -> Result<SpecificVolume> {
    check_tp_single_phase_envelope(t, p)?;
    Ok(v_tp_eqm_single_phase(t, p))
}

/// Checked mass density rho (kg/m^3) from a single-phase `(T,p)` flash —
/// the reciprocal of [`try_v_tp_eqm_single_phase`]. Valid range: the
/// `(T,p)` envelope in the module doc.
pub fn try_rho_tp_eqm_single_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
) -> Result<MassDensity> {
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

/// Checked isobaric cubic expansion coefficient alpha_v (1/K) from a
/// single-phase `(T,p)` flash — the thermal expansivity
/// `(1/v)(dv/dT)_p`. Valid range: the `(T,p)` envelope in the module doc.
/// Agrees exactly with [`alpha_v_tp_eqm_single_phase`] for in-range input.
///
/// Panic-trace: `alpha_v_tp_eqm_single_phase` calls
/// `region_fwd_eqn_single_phase` (fallthrough `panic!` at
/// `pt_flash_eqm/mod.rs:157`, excluded by the `(T,p)` envelope gate) and
/// panics on its Region-4 arm (`pt_flash_eqm/mod.rs:334`, excluded by the
/// exact-saturation-line rejection). The per-region kernels
/// `alpha_v_tp_1/2/3/5` are panic-free.
pub fn try_alpha_v_tp_eqm_single_phase(
    t: ThermodynamicTemperature,
    p: Pressure,
) -> Result<TemperatureCoefficient> {
    check_tp_single_phase_envelope(t, p)?;
    Ok(alpha_v_tp_eqm_single_phase(t, p))
}

/// Checked isothermal compressibility kappa_T (1/Pa) from a single-phase
/// `(T,p)` flash — `-(1/v)(dv/dp)_T`. Valid range: the `(T,p)` envelope in
/// the module doc. Agrees exactly with [`kappa_t_tp_eqm`] for in-range
/// input.
///
/// Panic-trace: identical to [`try_alpha_v_tp_eqm_single_phase`], with the
/// Region-4 arm at `pt_flash_eqm/mod.rs:348`.
pub fn try_kappa_t_tp_eqm(t: ThermodynamicTemperature, p: Pressure) -> Result<InversePressure> {
    check_tp_single_phase_envelope(t, p)?;
    Ok(kappa_t_tp_eqm(t, p))
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

/// Checked isobaric cubic expansion coefficient alpha_v (1/K) from a
/// `(p,h)` flash (two-phase states return a quality-interpolated
/// estimate). Valid range: the `(p,h)` envelope in the module doc. Agrees
/// exactly with [`alpha_v_ph_eqm`] for in-range input.
///
/// Panic-trace: `alpha_v_ph_eqm` calls `t_ph_eqm` and `ph_flash_region`,
/// whose panics (`ph_flash_eqm/mod.rs:833,837,846` and the Region-5 arm at
/// `:51`) are all excluded by [`check_ph_envelope`]. Its Region-4 arm
/// evaluates `alpha_v_tp_1`/`alpha_v_tp_2` directly at `(T_sat, p)`,
/// bypassing the `(T,p)` router, so it adds no new panic site.
pub fn try_alpha_v_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<TemperatureCoefficient> {
    check_ph_envelope(p, h)?;
    Ok(alpha_v_ph_eqm(p, h))
}

/// Checked isothermal compressibility kappa_T (1/Pa) from a `(p,h)` flash
/// (two-phase states return a quality-interpolated estimate). Valid range:
/// the `(p,h)` envelope in the module doc. Agrees exactly with
/// [`kappa_t_ph_eqm`] for in-range input.
///
/// Panic-trace: identical to [`try_alpha_v_ph_eqm`].
pub fn try_kappa_t_ph_eqm(p: Pressure, h: AvailableEnergy) -> Result<InversePressure> {
    check_ph_envelope(p, h)?;
    Ok(kappa_t_ph_eqm(p, h))
}

#[cfg(test)]
mod tests;
