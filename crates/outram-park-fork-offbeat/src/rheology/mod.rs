// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream `offbeatLib/rheology/` in full:
// `rheology.C/H`, `rheologyByMaterial.C/H` and `constitutiveLaws/`
// (`constitutiveLaw`, `elasticity`, `misesPlasticity`, `misesPlasticCreep`,
// `yieldStressModels/`, `creepModels/`).
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Constitutive laws — what makes fuel behave like fuel rather than a spring.
//!
//! # What this layer is for
//!
//! [`crate::mechanics`] solves equilibrium `∇·σ = 0` for a linear-elastic
//! material loaded by an eigenstrain. That is a spring: unload it and it
//! returns to where it started, and it never yields, never relaxes, never
//! creeps. Real fuel and real cladding do all three, and this module is where
//! that happens.
//!
//! Upstream calls it from `smallStrain.C` after the displacement solve has
//! converged: hand it the strain, and it hands back the **corrected stress**
//! plus the internal variables that advanced. This port keeps that contract,
//! per cell, in [`ConstitutiveLaw::correct`].
//!
//! # The two mechanisms, for a reader with no fuel-performance background
//!
//! Assume continuum mechanics; assume nothing about reactors.
//!
//! **Plasticity** is a *threshold* phenomenon. Below the yield stress `σ_y`
//! deformation is elastic and reversible. Reach `σ_y` and the material flows
//! irreversibly, at whatever rate it takes to keep the stress *on* the yield
//! surface — you cannot push a perfectly plastic material past `σ_y` no matter
//! how hard you strain it. Because yielding rearranges dislocations, `σ_y`
//! itself usually rises as flow accumulates: **work hardening**. In a fuel rod
//! this matters during power ramps and during pellet–cladding mechanical
//! interaction, where a swollen pellet pushes hard enough on the tube to yield
//! it.
//!
//! **Creep** is a *rate* phenomenon with no threshold. Hold any stress for long
//! enough and the material keeps deforming, at a rate set by stress and
//! (usually exponentially) by temperature. Over a laboratory test this is
//! invisible; over the 3–5 years a fuel rod spends in a reactor it dominates.
//! Two flavours matter:
//!
//! - **Thermal creep** — diffusion and dislocation climb, `∝ exp(−Q/RT)`. This
//!   is what makes a 1500 K fuel pellet slowly conform to the tube around it.
//! - **Irradiation creep** — fast neutrons continuously knock atoms off their
//!   lattice sites, and the resulting point defects relieve local stress as
//!   they migrate. Its rate depends on the neutron **flux**, is nearly
//!   independent of temperature, and is roughly linear in stress. This is the
//!   mechanism that actually lets 600 K cladding — far too cold for thermal
//!   creep to do anything measurable — creep *down* onto the pellet under
//!   coolant pressure over months. Predicting when the fuel/cladding gap closes
//!   is not possible without it.
//!
//! # What `correct` computes
//!
//! Everything rests on one additive split of the **mechanical** strain, i.e.
//! the total strain with the stress-free eigenstrain already removed:
//!
//! `ε_mech = ε_el + ε_p + ε_c`
//!
//! and one isotropic stress–strain relation in the elastic part alone:
//!
//! `σ = K tr(ε_el) I + 2μ dev(ε_el)`
//!
//! Per timestep, per cell, in this order (matching upstream's
//! `misesPlasticCreep`):
//!
//! 1. Subtract the plastic and creep strain accumulated in previous converged
//!    steps, giving the elastic **trial** strain and its trial stress.
//! 2. **Creep.** Solve `Δε_c = Δt · ε̇_c(q_trial − 3μ Δε_c)` by Newton for the
//!    equivalent creep increment, then flow along the deviatoric stress
//!    direction (Prandtl–Reuss).
//! 3. **Plasticity.** Radial-return the creep-reduced trial stress onto the
//!    yield surface, solving
//!    `|s| − 2μΔλ − sqrt(2/3) σ_y(ε_p,eq + sqrt(2/3)Δλ) = 0` by Newton.
//! 4. Assemble `σ` from what elastic strain is left.
//!
//! Creep is done **before** plasticity because creep relaxes stress: doing it
//! afterwards leaves more of the increment to be taken up plastically and
//! over-predicts plastic strain.
//!
//! Neither Newton iteration is allowed to fail quietly. Both report
//! [`OffbeatError::ConstitutiveNotConverged`](crate::error::OffbeatError::ConstitutiveNotConverged)
//! rather than returning a stress that sits outside the yield surface.
//!
//! # How the mechanics layer must drive this
//!
//! The contract is not obvious and getting it wrong produces plausible-looking
//! nonsense, so it is spelled out:
//!
//! 1. **Subtract the eigenstrain first.** [`RheologyInputs::mechanical_strain`]
//!    is the strain from the displacement solve **minus** thermal expansion,
//!    swelling, densification and relocation. Passing the total strain makes a
//!    freely expanding, unconstrained pellet appear to be under enormous stress
//!    — and it will then dutifully creep.
//! 2. **Own the state.** Keep one [`RheologyState`] per cell. Pass it by
//!    reference to `correct`, which does not mutate it.
//! 3. **Iterate, then commit.** `correct` may be called many times per timestep
//!    inside the outer mechanics corrector loop, each time from the *same*
//!    start-of-step [`RheologyState`]. Only once that loop has converged, call
//!    [`RheologyState::advance`] once. Advancing inside the loop double-counts
//!    the inelastic strain — the classic way to make a creep model silently
//!    over-predict.
//! 4. **Feed the corrected stress back.** The corrected stress is *softer* than
//!    the elastic one, so the displacement field that produced it is no longer
//!    an equilibrium field. Upstream restores equilibrium by re-solving with
//!    the inelastic strain treated as an additional eigenstrain (its
//!    `correctAdditionalStrain`); [`RheologyState::inelastic_strain`] is the
//!    quantity to hand back for that.
//! 5. **Limit the timestep.** Creep integration is implicit in stress but
//!    explicit in state. [`CreepTimeStepControl`] turns last step's increments
//!    into a bound on the next step.
//!
//! # Scope of this port
//!
//! **Implemented:** small-strain elasticity, von Mises plasticity with
//! isotropic hardening, and plasticity plus creep; three yield-stress models
//! (constant, tabulated hardening, FRAPTRAN Zircaloy); four creep models (none,
//! Norton power law, Limbäck Zircaloy, MATPRO fuel); per-material-zone
//! dispatch.
//!
//! **Not implemented, and why:**
//!
//! | Upstream | Reason |
//! |---|---|
//! | `hyperElasticity`, `neoHookeanElasticity`, `neoHookeanMisesPlasticity`, `hyperElasticMisesPlasticCreep` | Large-strain (total/updated Lagrangian) laws. [`crate::mechanics`] is small-strain only and does not produce a deformation gradient, so these would have nothing to consume. |
//! | `GPLS_Hydrogen`, `fastNeutronGPLS` | Anisotropic (Hill) viscoplastic laws for reactivity-initiated-accident transients. They need a Hill tensor, a normalised axial coordinate and an RIA-specific strain-rate range that no other part of this port supplies. |
//! | `LimbackCreepModelLOCA` | Loss-of-coolant variant covering the Zircaloy α→β phase transformation. Needs a phase-fraction field this port does not have. |
//! | `MalyginMOXCreepModel`, `RoutbortFastMOXCreepModel`, `TobbeCreepModel`, `TobbeDINCreepModel`, `TobbeAIM1CreepModel`, `ZhangHastelloyCreepModel`, `MonolithicSiCCreepModel`, `PARFUMEBufferCreepModel`, `PARFUMEPyCCreepModel` | Additional material-specific correlations with the same structure as [`CreepModel::Matpro`]. Deferred on effort, not on capability: each is a new enum variant plus a rate function. |
//! | `constantCreepPrincipalStress`, `correlationCreepPrincipalStress` | Need a principal-stress decomposition and per-direction creep, which the isotropic Prandtl–Reuss flow used here cannot express. |
//! | `planeStress`, `modifiedPlaneStrain`, `solvePressureEqn` in `rheologyByMaterial` | Mesh- and case-level features rather than constitutive laws: they need the axial slice mapper, the gap-gas model and the momentum-matrix diagonal, all of which live outside this module. |
//!
//! # Status
//!
//! **AI-assisted draft, unreviewed.** Per `RESPONSIBLE_USE.md` this is
//! untrusted material until a human has inspected it. The tests establish
//! self-consistency and agreement with closed-form plasticity and creep
//! solutions; none is a validation against experiment, and none may be
//! described as one.

pub mod aster;

mod by_material;
mod creep;
mod law;
mod state;
mod yield_stress;

pub use by_material::{Rheology, RheologyByMaterial};
pub use creep::{CreepIncrement, CreepModel, CreepTimeStepControl, ZircaloyCladType};
pub use law::ConstitutiveLaw;
pub use state::{
    equivalent_strain, von_mises, IrradiationState, RheologyInputs, RheologyState, StressCorrection,
};
pub use yield_stress::{HardeningCurve, YieldStressModel};

#[cfg(test)]
mod tests;
