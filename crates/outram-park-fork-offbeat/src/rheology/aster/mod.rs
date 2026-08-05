// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from code_aster (https://gitlab.com/codeaster/src)
//   Copyright (C) 1991 - 2026 - EDF R&D
//   Licence: GPL-3.0-or-later
//   Upstream commit: b504ea08c2f49575e04644cee2e39a63ea45c16e
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! code_aster constitutive laws.
//!
//! # What this is
//!
//! A port of the constitutive-law layer of [code_aster](https://gitlab.com/codeaster/src),
//! EDF's nonlinear structural and thermo-mechanical solver, into this crate's
//! existing rheology idiom. Plan and module inventory:
//! `docs/code-aster-port-scoping.md`; tracked as epic `op-a7p`.
//!
//! # Why it is worth porting
//!
//! code_aster was built by EDF to justify the integrity and remaining life of
//! its own reactor fleet, so its constitutive laws are the *nuclear* ones —
//! irradiation creep, Zircaloy anisotropy, vessel steels — rather than generic
//! mechanical-engineering fare. That gives one port two consumers:
//!
//! - **Fuel performance.** [`crate::rheology`] currently offers three
//!   constitutive laws. code_aster's catalogue carries `ZIRC`, `ZIRC_MECA`,
//!   `META_LEMA_ANI`, `LEMAITRE_IRRA`, `VISC_IRRA_LOG`, `GRAN_IRRA_LOG` and
//!   `IRRAD3M` — anisotropic and irradiation creep for cladding, which normal
//!   operation needs and this crate does not yet have.
//! - **Severe accident.** Creep rupture of a reactor lower head, the model
//!   `docs/melcor-scoping.md` phase 5 needs.
//!
//! # Status
//!
//! **P0 only: the catalogue and the kinematics.** No constitutive law is
//! implemented yet. [`catalogue`] records what upstream declares;
//! [`kinematics`] pins the two conventions every future law depends on.
//!
//! # Provenance
//!
//! code_aster is GPL-3.0-or-later, compatible with this workspace. Upstream is
//! **not** vendored — the port is made from a read-only clone kept outside the
//! working tree, and only upstream's `src` repository is used. Its `validation`
//! and `data` repositories carry material that may not be freely distributed
//! and are out of scope per `DATA_POLICY.md`.

pub mod catalogue;
pub mod chaboche;
pub mod damage;
pub mod fracture;
pub mod integration;
pub mod isotropic;
pub mod kinematics;
pub mod log_strain;
pub mod metallurgy;
pub mod viscoplastic;

pub use catalogue::{AsterBehaviour, ALL};
pub use chaboche::{
    BackStress, ChabocheIncrement, ChabocheLaw, ChabocheLocalState, ChabocheParameters,
    ChabochePredictor, ChabocheState, ElasticModuli, StrainMemory, ThermoElasticStep,
};
pub use fracture::{
    equivalent_mode_i_factor, hat_smooth_front, irwin_energy_release_rate, irwin_mode_split,
    legendre_front_mode, legendre_front_mode_derivative, max_hoop_stress_kink_angle,
    near_tip_stress, scaled_hoop_stress, westergaard_unit_field, CrackOpeningMode, CrackPlaneState,
    CrackTipBasis, LinearElasticConstants, ModeEnergyRelease, NearTipField, PlanarCrackTipResult,
    StressIntensityFactors, MAX_LEGENDRE_FRONT_DEGREE,
};
pub use integration::{
    brent, newton_perturbed, newton_safeguarded, perturbed_default, secant, LocalSolution,
    ScalarAlgorithm, SolverControl,
};
pub use isotropic::{
    IsotropicHardening, LinearHardening, NortonHoffLimitAnalysis, PowerLawHardening,
};
pub use kinematics::{hencky_strain, AsterVoigt, DeformationGradient};
pub use log_strain::LogarithmicStrain;
pub use viscoplastic::{
    deviator, von_mises_of_deviator, CreepIncrement, LemaitreParameters, NortonParameters,
    ViscoplasticLaw,
};
