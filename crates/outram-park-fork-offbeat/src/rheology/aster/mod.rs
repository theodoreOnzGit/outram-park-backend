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
//! **Verification-tested draft. Nothing here is validated.** Every test in
//! every module below is *verification* — independent transcription of
//! upstream's algebra, closed-form limits, invariants, and measured
//! convergence orders. Nothing has been compared against a cladding-creepdown
//! measurement or any reactor data, and no such agreement is claimed.
//!
//! Upstream's `astest` suite **is** available in the read-only clone (an
//! earlier revision of this note wrongly said it was absent — it was merely
//! outside the sparse checkout), and it lives in the GPL-3.0-or-later `src`
//! repository, so it is in scope under `DATA_POLICY.md`. Two of its cases are
//! now run as integration tests — `tests/astest_ssnv101a.rs` (Chaboche) and
//! `tests/astest_ssnv126a.rs` (`VENDOCHAB`) — against upstream's own **`VALE_CALC`**
//! computed values. That makes them *verification against a reference
//! implementation*: they show this port reproduces code_aster's arithmetic,
//! **not** that either code reproduces reality. Upstream's `VALE_REFE`
//! analytical/experimental references are deliberately never asserted here —
//! promoting a case to validation is the maintainer's call.
//!
//! Foundations:
//!
//! - [`catalogue`] — what upstream declares (229 behaviours).
//! - [`kinematics`] — the Mandel convention and the deformation gradient.
//! - [`integration`] — the scalar local solvers every law below shares.
//! - [`log_strain`] — the `GDEF_LOG` large-strain wrapper.
//! - [`hardening`] — the one isotropic-hardening curve every law above shares.
//!
//! Constitutive laws:
//!
//! | Module | Laws |
//! |---|---|
//! | [`viscoplastic`] | `NORTON`, `LEMAITRE`, `LEMAITRE_IRRA` |
//! | [`isotropic`] | `VMIS_ISOT_LINE`/`_PUIS` hardening, `NORTON_HOFF` |
//! | [`chaboche`] | `VMIS_CIN1/2_CHAB`, `VISC_CIN1/2_CHAB`, `VMIS/VISC_CIN2_MEMO` |
//! | [`viscochab`] | `VISCOCHAB` — the 27-variable rate system of `rkdcha.F90` |
//! | [`damage`] | `VENDOCHAB`, `VISC_ENDO_LEMA`, `ROUSS_PR`, `ROUSS_VISC`, `GTN`, `VISC_GTN`, `CRIT_RUPT` |
//! | [`metallurgy`] | `VISC_IRRA_LOG`, `GRAN_IRRA_LOG`, `IRRAD3M`, `META_LEMA_ANI` |
//! | [`fracture`] | linear-elastic fracture post-processing only — see below |
//!
//! Two limitations that change results and must not be discovered late:
//!
//! - [`fracture`] is roughly **80 % blocked**. The G-theta domain integral
//!   needs element shape functions, Gauss quadrature and crack-front ring
//!   topology, none of which this crate has. What is implemented is the
//!   closed-form subset.
//! - [`damage`]'s `GTN` is the **local** form only. Without `GRADVARI`
//!   nonlocal regularisation a structural run will localise into one element
//!   band and give mesh-dependent answers.
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
pub mod hardening;
pub mod integration;
pub mod isotropic;
pub mod kinematics;
pub mod log_strain;
pub mod metallurgy;
pub mod viscochab;
pub mod viscoplastic;

pub use catalogue::{AsterBehaviour, ALL};
pub use chaboche::{
    BackStress, ChabocheIncrement, ChabocheLaw, ChabocheLocalState, ChabocheParameters,
    ChabochePredictor, ChabocheState, ElasticModuli, StrainMemory, ThermoElasticStep,
};
pub use damage::{
    equivalent_stress, max_principal_stress, mean_stress, DamageOutcome, GtnIncrement,
    GtnNucleation, GtnOutcome, GtnParameters, GtnState, GursonTvergaardNeedleman,
    IsotropicElasticity, LemaitreChabocheIncrement, LemaitreChabocheLaw,
    LemaitreChabocheParameters, LemaitreChabocheState, NortonOverstress, RousselierIncrement,
    RousselierLaw, RousselierOutcome, RousselierParameters, RousselierState, RuptureCriterion,
    RuptureState, ViscousSinhParameters, LEMAITRE_CHABOCHE_DAMAGE_MAX,
};
pub use fracture::{
    equivalent_mode_i_factor, hat_smooth_front, irwin_energy_release_rate, irwin_mode_split,
    legendre_front_mode, legendre_front_mode_derivative, max_hoop_stress_kink_angle,
    near_tip_stress, scaled_hoop_stress, westergaard_unit_field, CrackOpeningMode, CrackPlaneState,
    CrackTipBasis, LinearElasticConstants, ModeEnergyRelease, NearTipField, PlanarCrackTipResult,
    StressIntensityFactors, MAX_LEGENDRE_FRONT_DEGREE,
};
pub use hardening::{
    IsotropicHardening, ASTER_POWER_LINEARISATION_STRAIN, SLOPE_SINGULARITY_OFFSET,
};
pub use integration::{
    brent, newton_perturbed, newton_safeguarded, perturbed_default, secant, LocalSolution,
    ScalarAlgorithm, SolverControl,
};
pub use isotropic::NortonHoffLimitAnalysis;
pub use kinematics::{hencky_strain, AsterVoigt, DeformationGradient};
pub use log_strain::LogarithmicStrain;
pub use metallurgy::{
    HillAnisotropy, Irrad3m, Irrad3mHardening, Irrad3mIncrement, Irrad3mParameters, Irrad3mState,
    IrradiationGrowthDirection, LogarithmicIrradiationLaw, LogarithmicIrradiationParameters,
    MetaLemaAni, MetaLemaAniIncrement, MetaLemaAniPhase, IRRAD3M_PROOF_STRAIN,
};
// NOTE: `viscochab`'s three consts — `ASTER_COEFFICIENT_NAMES`,
// `INTERNAL_VARIABLE_COUNT` and `ODE_EQUATION_COUNT` — are deliberately left
// module-qualified rather than flattened here. Each names a property of *one*
// law, and all three would collide the moment a second rate-system law lands.
// Reach them as `viscochab::ODE_EQUATION_COUNT`.
pub use viscochab::{
    ViscoplasticChabocheParameters, ViscoplasticChabocheRates, ViscoplasticChabocheState,
    ViscoplasticChabocheSystem, ViscoplasticChabocheWithMemory, RKDCHA_ALPHA2_USES_D1,
};
pub use viscoplastic::{
    deviator, von_mises_of_deviator, CreepIncrement, LemaitreParameters, NortonParameters,
    ViscoplasticLaw,
};
