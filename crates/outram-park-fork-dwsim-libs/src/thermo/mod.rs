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
//! ### Data substrate
//!
//! - [`component`] — the pure-compound constant-property data model
//!   ([`Component`]): critical properties, acentric factor, molar mass,
//!   ideal-gas heat-capacity coefficients. The shared substrate every other
//!   thermo module consumes. **Data substrate (this file's author).**
//!
//! ### Equations of state
//!
//! - [`cubic_eos`] — Peng-Robinson and SRK cubic EOS: compressibility solve,
//!   fugacity coefficients, enthalpy/entropy departures, van der Waals mixing.
//! - [`eos_variants`] — cubic-EOS refinements: the PRSV α-function and the
//!   Peneloux volume translation, composed on top of [`cubic_eos`].
//! - [`pr1978`] — Peng-Robinson 1978 (PR78), the ω-dependent α-slope refit.
//! - [`prsv2_full`] — full Peng-Robinson-Stryjek-Vera 2 (PRSV2): the
//!   three-parameter (κ₁, κ₂, κ₃) α-function with a complete Z / fugacity /
//!   departure / vapour-pressure surface.
//! - [`lkp`] — Lee-Kesler-Plöcker (LKP) three-parameter corresponding-states EOS.
//! - [`pr_lee_kesler`] — Peng-Robinson + Lee-Kesler enthalpy/entropy hybrid
//!   property package (PR fugacities, LKP caloric departures).
//!
//! ### Activity-coefficient / group-contribution models
//!
//! - [`activity`] — NRTL / UNIQUAC / Ideal (Raoult) liquid-phase activity
//!   coefficients.
//! - [`unifac`] — UNIFAC group-contribution activity coefficients.
//! - [`unifac_dortmund`] — modified UNIFAC (Dortmund) group-contribution
//!   activity coefficients (Weidlich & Gmehling temperature-dependent
//!   interaction + modified combinatorial term).
//! - [`unifac_lle`] — UNIFAC with the liquid-liquid-equilibrium (LLE)
//!   parameterised group-interaction table (same functional form, LLE `a_mn`).
//! - [`chao_seader_grayson`] — Chao-Seader and Grayson-Streed semi-empirical
//!   hydrocarbon K-value packages: regular-solution liquid activity × the
//!   Pitzer-form pure-liquid fugacity (ν⁰) ÷ a Redlich-Kwong vapour fugacity.
//! - [`electrolyte`] — aqueous-ionic (electrolyte) activity-coefficient tier.
//! - [`ideal_props`] — ideal-gas heat capacity / enthalpy / entropy from the
//!   [`Component`] Cp0 coefficients (the departure reference state).
//! - [`transport`] — transport-property correlations (viscosity, thermal
//!   conductivity, surface tension) and their phase-mixing rules.
//!
//! ### Flash algorithms
//!
//! - [`flash`] — isothermal-isobaric (TP) vapour-liquid-equilibrium flash via
//!   the Rachford-Rice / Nested-Loops method, with Wilson K-value initialisation.
//! - [`flash_insideout`] — Boston-Britt Inside-Out two-phase (VLE) PT flash.
//! - [`flash_insideout_3p`] — Boston-Fournier Inside-Out three-phase (VLLE)
//!   PT flash.
//! - [`flash_vlle`] — three-phase vapour-liquid-liquid equilibrium (VLLE)
//!   nested-loops PT flash.
//! - [`flash_lle`] — simple liquid-liquid equilibrium (LLE) isothermal split.
//! - [`flash_immiscible`] — nested-loops immiscible flash: an ordinary
//!   two-phase VLE on the water-free feed, plus a fully-immiscible component
//!   (e.g. water in a hydrocarbon system) partitioned by its own pure vapour
//!   pressure into a third, pure liquid phase.
//! - [`flash_sle`] — solid-liquid equilibrium (SLE) flash (ideal-solubility
//!   saturation with heat-of-fusion temperature dependence).
//! - [`flash_svlle`] — solid + three-phase (SVLLE) flash: precipitation coupled
//!   to the VLLE split.
//! - [`flash_single_comp`] — single-component (pure-fluid) saturation-shortcut
//!   flash.
//! - [`energy_flash`] — isenthalpic (PH) / energy flash: solve the temperature at
//!   which a mixture's total molar enthalpy meets a target `H` at fixed `P`.
//! - [`saturation`] — bubble-point / dew-point temperature & pressure of a
//!   multicomponent mixture, on top of the isothermal-isobaric VLE kernel.
//! - [`stability`] — phase-stability analysis via Michelsen's tangent-plane
//!   distance (TPD) criterion (single-/two-phase identification, flash init).
//!
//! ### Gibbs-minimisation & reacting equilibria
//!
//! - [`gibbs`] — Gibbs-energy-minimisation speciation flash (single gas phase,
//!   reacting mixture) under element / atom mass-balance constraints.
//! - [`gibbs_multiphase`] — multi-phase (N-phase) Gibbs-energy-minimisation
//!   flash: species distribution across several coexisting solution phases.
//! - [`electrolyte_svle`] — electrolyte SVLE flash: weak-electrolyte
//!   reaction-set speciation coupled to solid (Ksp) precipitation.
//! - [`sour_water`] — sour-water aqueous ionic-equilibrium speciation
//!   (H₂S / NH₃ / CO₂ / H₂O), built on the [`electrolyte_svle`] conventions.
//!
//! ### Specialised property packages
//!
//! - [`seawater`] — seawater thermophysical properties (density, specific heat,
//!   thermal conductivity, viscosity, vapour pressure, surface tension,
//!   boiling-point elevation) as a function of salinity and temperature
//!   (Sharqawy 2010 / Nayar 2016 correlations).
//! - [`black_oil`] — black-oil petroleum correlations: solution gas-oil ratio,
//!   bubble-point pressure, oil/gas/water formation volume factors, densities,
//!   and viscosities (Standing / Vazquez-Beggs / Beggs-Robinson /
//!   Dranchuk-Abou-Kassem).
//!
//! ### Property-package glue
//!
//! - [`property_package`] — glue that composes the cubic-EOS / ideal models into
//!   K-values and drives an EOS-consistent PT two-phase flash
//!   ([`property_package::PropertyPackageModel`], enum dispatch, no `dyn`).
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
//! This is a substantial slice of DWSIM's thermodynamics, though still not the
//! whole of it. What was once flagged as "future work" is now ported:
//!
//! - **Advanced EOS.** PR78 ([`pr1978`]), the full PRSV2 κ₁/κ₂/κ₃ α-function
//!   ([`prsv2_full`]), Lee-Kesler-Plöcker ([`lkp`]), and the PR + Lee-Kesler
//!   caloric hybrid ([`pr_lee_kesler`]) — on top of the earlier one-parameter
//!   PRSV α-function and Peneloux volume translation ([`eos_variants`]).
//! - **Inside-Out flashes.** Boston-Britt two-phase ([`flash_insideout`]) and
//!   Boston-Fournier three-phase ([`flash_insideout_3p`]).
//! - **Multi-phase / solid / electrolyte equilibria.** Three-phase VLLE
//!   ([`flash_vlle`]), LLE ([`flash_lle`]), SLE ([`flash_sle`]), SVLLE
//!   ([`flash_svlle`]), single-component shortcut ([`flash_single_comp`]),
//!   single- and multi-phase Gibbs minimisation ([`gibbs`], [`gibbs_multiphase`]),
//!   the electrolyte activity tier ([`electrolyte`]), the electrolyte SVLE
//!   speciation + Ksp solver ([`electrolyte_svle`]), and the sour-water package
//!   ([`sour_water`]).
//! - **Group-contribution activity.** Modified UNIFAC (Dortmund)
//!   ([`unifac_dortmund`]) and UNIFAC-LLE ([`unifac_lle`]).
//! - **Hydrocarbon & specialised packages.** Chao-Seader / Grayson-Streed
//!   hydrocarbon K-values ([`chao_seader_grayson`]), the nested-loops
//!   immiscible flash ([`flash_immiscible`]), the seawater property package
//!   ([`seawater`]), and the black-oil petroleum correlations ([`black_oil`]).
//!
//! **Still out of scope / future work:** the Mathias-Copeman and Twu α-variants,
//! and the CoolProp/steam-table external-property bridges. Everything here is
//! **verified, not benchmark-validated** — see `docs/port-scope.md` and epic
//! `op-qo2` for the remaining backlog.

pub mod activity;
pub mod black_oil;
pub mod chao_seader_grayson;
pub mod component;
pub mod cubic_eos;
pub mod electrolyte;
pub mod electrolyte_svle;
pub mod energy_flash;
pub mod eos_variants;
pub mod flash;
pub mod flash_immiscible;
pub mod flash_insideout;
pub mod flash_insideout_3p;
pub mod flash_lle;
pub mod flash_single_comp;
pub mod flash_sle;
pub mod flash_svlle;
pub mod flash_vlle;
pub mod gibbs;
pub mod gibbs_multiphase;
pub mod ideal_props;
pub mod lkp;
pub mod pr1978;
pub mod pr_lee_kesler;
pub mod property_package;
pub mod prsv2_full;
pub mod saturation;
pub mod seawater;
pub mod sour_water;
pub mod stability;
pub mod transport;
pub mod unifac;
pub mod unifac_dortmund;
pub mod unifac_lle;

pub use component::Component;
