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
//! - [`property_package`] — glue that composes the cubic-EOS / ideal models into
//!   K-values and drives an EOS-consistent PT two-phase flash
//!   ([`property_package::PropertyPackageModel`], enum dispatch, no `dyn`).
//! - [`energy_flash`] — isenthalpic (PH) / energy flash: solve the temperature at
//!   which a mixture's total molar enthalpy meets a target `H` at fixed `P`.
//! - [`saturation`] — bubble-point / dew-point temperature & pressure of a
//!   multicomponent mixture, on top of the isothermal-isobaric VLE kernel.
//! - [`stability`] — phase-stability analysis via Michelsen's tangent-plane
//!   distance (TPD) criterion (single-/two-phase identification, flash init).
//! - [`transport`] — transport-property correlations (viscosity, thermal
//!   conductivity, surface tension) and their phase-mixing rules.
//! - [`eos_variants`] — cubic-EOS refinements: the PRSV α-function and the
//!   Peneloux volume translation, composed on top of [`cubic_eos`].
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
//! one-parameter PRSV α-function and the Peneloux volume translation are ported
//! ([`eos_variants`]); the long tail — Gibbs-minimisation and inside-out flashes,
//! 3-phase / electrolyte / solid equilibria, the LKP and PRSV2/Mathias-Copeman/Twu
//! α-variants, seawater/sour-water/black-oil packages — remains future work (see
//! `docs/port-scope.md`, epic `op-qo2`).

pub mod activity;
pub mod component;
pub mod cubic_eos;
pub mod electrolyte;
pub mod electrolyte_svle;
pub mod energy_flash;
pub mod eos_variants;
pub mod flash;
pub mod flash_insideout;
pub mod flash_insideout_3p;
pub mod flash_lle;
pub mod flash_single_comp;
pub mod flash_sle;
pub mod flash_vlle;
pub mod gibbs;
pub mod ideal_props;
pub mod lkp;
pub mod pr1978;
pub mod pr_lee_kesler;
pub mod property_package;
pub mod prsv2_full;
pub mod saturation;
pub mod stability;
pub mod transport;
pub mod unifac;

pub use component::Component;
