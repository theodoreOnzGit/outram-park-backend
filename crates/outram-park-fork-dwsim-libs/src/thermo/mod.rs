//! # Thermodynamics kernel (DWSIM Tier-1 port)
//!
//! The core thermodynamic kernel translated from DWSIM's `DWSIM.Thermodynamics`
//! (GPL-3.0): the compound data model, cubic equations of state, liquid-phase
//! activity-coefficient models, and the vapour-liquid-equilibrium flash. These
//! supply the fugacity coefficients, K-values, and enthalpy/entropy departures
//! that every equipment model ultimately needs.
//!
//! > **⚠️ Unverified until validated.** Early-stage translation, no human V&V.
//! > Not for nuclear facility operation, reactor control, safety-critical, or
//! > licensing decisions. Independent OUTRAM PARK fork, not the official DWSIM.
//!
//! ## Modules
//!
//! - [`component`] — the pure-compound constant-property data model
//!   ([`Component`]): critical properties, acentric factor, molar mass,
//!   ideal-gas heat-capacity coefficients. The shared substrate every other
//!   thermo module consumes. **Data substrate (this file's author).**
//! - [`cubic_eos`] — Peng-Robinson and SRK cubic EOS: compressibility solve,
//!   fugacity coefficients, enthalpy/entropy departures, van der Waals mixing.
//! - [`activity`] — NRTL / UNIQUAC / Ideal (Raoult) liquid-phase activity
//!   coefficients.
//! - [`unifac`] — UNIFAC group-contribution activity coefficients.
//! - [`ideal_props`] — ideal-gas heat capacity / enthalpy / entropy from the
//!   [`Component`] Cp0 coefficients (the departure reference state).
//! - [`flash`] — isothermal-isobaric (TP) vapour-liquid-equilibrium flash via
//!   the Rachford-Rice / Nested-Loops method, with Wilson K-value initialisation.
//!
//! ## Design (crate `CLAUDE.md`)
//!
//! Enum dispatch (no `dyn`) for the EOS / activity / flash model choices; `uom`
//! at public boundaries where practical, documented raw `f64` (SI) in the inner
//! EOS/flash arithmetic loops where `uom` overhead would fight the math (the
//! DWSIM-internal SI convention: Pa, K, J/mol, kg/m³).
//!
//! ## Honest scope
//!
//! This is the **core kernel**, not the whole of DWSIM's thermodynamics. The
//! long tail — Gibbs-minimisation and inside-out flashes, 3-phase / electrolyte
//! / solid equilibria, LKP and PRSV variants, seawater/sour-water/black-oil
//! packages — remains future work (see `docs/port-scope.md`, epic `op-qo2`).

pub mod activity;
pub mod component;
pub mod cubic_eos;
pub mod energy_flash;
pub mod flash;
pub mod ideal_props;
pub mod property_package;
pub mod stability;
pub mod unifac;

pub use component::Component;
