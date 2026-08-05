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
//! convergence orders. Nothing has been compared against code_aster output, a
//! cladding-creepdown measurement, or any reactor data, and no such agreement
//! is claimed. Note also that upstream's `astest` suite is **absent** from the
//! clone this port was made from, so the reference oracle assumed by
//! `docs/code-aster-port-scoping.md` §7 was not available.
//!
//! Foundations:
//!
//! - [`catalogue`] — what upstream declares (229 behaviours).
//! - [`kinematics`] — the Mandel convention and the deformation gradient.
//! - [`integration`] — the scalar local solvers every law below shares.
//! - [`log_strain`] — the `GDEF_LOG` large-strain wrapper.
//!
//! Constitutive laws:
//!
//! | Module | Laws |
//! |---|---|
//! | [`viscoplastic`] | `NORTON`, `LEMAITRE`, `LEMAITRE_IRRA` |
//! | [`isotropic`] | `VMIS_ISOT_LINE`/`_PUIS` hardening, `NORTON_HOFF` |
//! | [`chaboche`] | `VMIS_CIN1/2_CHAB`, `VISC_CIN1/2_CHAB`, `VMIS/VISC_CIN2_MEMO` |
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
pub mod viscochab; // TEMPORARY-AGENT-WIRING: remove before hand-off
pub mod viscoplastic;

pub use catalogue::{AsterBehaviour, ALL};
pub use chaboche::{
    BackStress, ChabocheIncrement, ChabocheLaw, ChabocheLocalState, ChabocheParameters,
    ChabochePredictor, ChabocheState, ElasticModuli, StrainMemory, ThermoElasticStep,
};
// NOTE: `damage::IsotropicHardening` is deliberately NOT re-exported here.
// It collides with [`isotropic::IsotropicHardening`], and the two are genuinely
// different types: the `isotropic` one is the `VMIS_ISOT_*` / `VISC_ISOT_*`
// hardening curve driving the von Mises radial return, while the `damage` one
// is the curve set used by the Rousselier and GTN porous-plastic laws. Both
// names are defensible in their own module, so neither was renamed unilaterally
// — reach them as `damage::IsotropicHardening` and `isotropic::IsotropicHardening`.
// Consolidating them into one curve type is the real fix and is a design
// decision for the maintainer, not a mechanical rename.
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
pub use integration::{
    brent, newton_perturbed, newton_safeguarded, perturbed_default, secant, LocalSolution,
    ScalarAlgorithm, SolverControl,
};
pub use isotropic::{
    IsotropicHardening, LinearHardening, NortonHoffLimitAnalysis, PowerLawHardening,
};
pub use kinematics::{hencky_strain, AsterVoigt, DeformationGradient};
pub use log_strain::LogarithmicStrain;
pub use metallurgy::{
    HillAnisotropy, Irrad3m, Irrad3mHardening, Irrad3mIncrement, Irrad3mParameters, Irrad3mState,
    IrradiationGrowthDirection, LogarithmicIrradiationLaw, LogarithmicIrradiationParameters,
    MetaLemaAni, MetaLemaAniIncrement, MetaLemaAniPhase, IRRAD3M_PROOF_STRAIN,
};
pub use viscoplastic::{
    deviator, von_mises_of_deviator, CreepIncrement, LemaitreParameters, NortonParameters,
    ViscoplasticLaw,
};
