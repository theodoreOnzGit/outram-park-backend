// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to the per-cell registry fields that upstream's
// `offbeatLib/rheology/constitutiveLaws/misesPlasticity.C` and
// `offbeatLib/rheology/constitutiveLaws/creepModels/creepModel.C` create with
// `createOrLookup(...)` and carry across timesteps via `oldTime()`
// (`epsilonP`, `epsilonPEq`, `sigmaY`, `epsilonCreep`, `epsilonCreepEq`,
// `epsilonCreepPrimEq`).
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! The per-cell inputs and per-cell memory of the constitutive integration.
//!
//! A rate-independent elastic law needs no memory: hand it a strain, it hands
//! back a stress. Plasticity and creep are different — they are *path
//! dependent*. How much a piece of cladding has already yielded, and how much
//! it has already crept, changes what it does next. That history is
//! [`RheologyState`], and it is the caller's to store, one per cell.

use outram_foam_basic_lib::primitives::SymmTensor;

use crate::materials::MaterialState;
use crate::mechanics::LinearElastic;

/// Irradiation quantities the constitutive laws need that
/// [`MaterialState`] does not carry.
///
/// [`MaterialState`] describes what the material *is* (temperature,
/// composition, accumulated damage). This describes the **instantaneous
/// irradiation environment and microstructure** of one cell, which only the
/// creep correlations need. Keeping them apart means a purely elastic or
/// purely plastic case never has to invent values it does not use.
///
/// # Units — raw `f64`, strict SI
///
/// | Field | Unit | Upstream equivalent |
/// |---|---|---|
/// | [`fast_flux`](Self::fast_flux) | n/m²/s (E > 1 MeV) | `fastFlux` field, which is in **n/cm²/s** |
/// | [`fission_rate`](Self::fission_rate) | fissions/m³/s | `Q / 312e-13` in `MatproCreepModel.C` |
/// | [`grain_radius`](Self::grain_radius) | m | `grainRadius` field |
///
/// # Converting from a volumetric heat source
///
/// Upstream derives the fission rate from the volumetric heat source `Q`
/// \[W/m³\] as `F = Q / 3.12e-11`, i.e. one fission releases about 195 MeV of
/// locally deposited energy. Do the same at the call site:
/// `fission_rate = q_volumetric / 3.12e-11`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct IrradiationState {
    /// Fast-neutron flux \[n/m²/s\], conventionally E > 1 MeV.
    ///
    /// Drives irradiation creep, which is the mechanism that actually lets
    /// cladding creep down onto the pellet over months at temperatures far too
    /// low for thermal creep to matter. Zero outside a reactor.
    ///
    /// **Note the unit.** Upstream's `fastFlux` field is in n/cm²/s and
    /// `LimbackCreepModel.C` multiplies it by `1e4` to reach n/m²/s before
    /// applying the correlation constant. This port takes SI directly and so
    /// omits that factor.
    pub fast_flux: f64,

    /// Volumetric fission rate \[fissions/m³/s\].
    ///
    /// Drives irradiation-enhanced creep of the fuel (the athermal Sakai term
    /// and the fission-enhanced diffusional term in MATPRO). Zero outside a
    /// reactor. Typical LWR fuel at 20 kW/m: order 1e19.
    pub fission_rate: f64,

    /// Mean grain **radius** \[m\] of the fuel.
    ///
    /// Diffusional (Nabarro–Herring) creep goes as the inverse square of grain
    /// *diameter*, so this is a first-order input for fuel creep, not a
    /// correction. Typical as-fabricated UO2: 4–8 µm radius. Irrelevant to
    /// cladding, which may leave it zero.
    pub grain_radius: f64,
}

/// Everything the constitutive law needs about one cell for one timestep.
///
/// Bundled into one struct rather than passed as seven arguments, because the
/// workspace's human-interface rule cares more about how much a reader has to
/// hold in mind than about argument count in the abstract: a reader who has
/// seen this struct once can read every `correct` call in the module.
///
/// # Units — raw `f64`, strict SI
///
/// Stress in pascal, strain dimensionless, time in second, temperature in
/// kelvin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RheologyInputs {
    /// Isotropic elastic constants of this cell's material.
    pub elastic: LinearElastic,

    /// **Mechanical** strain \[-\] of this cell: the total strain
    /// `ε = ½(∇D + ∇Dᵀ)` **minus** the eigenstrain.
    ///
    /// This is the single most misread quantity in the module, so it is worth
    /// being blunt: do **not** pass the raw strain from the displacement solve.
    /// Thermal expansion, swelling, densification and relocation are
    /// stress-free deformations — they must already have been subtracted, just
    /// as upstream's `rheologyByMaterial::correct` forms
    /// `epsEl_ = epsilon - additionalStrain_ - epsTh_` before any constitutive
    /// law sees it. Passing the total strain instead makes a freely expanding,
    /// unconstrained pellet appear to be under enormous stress, and it will
    /// then dutifully creep.
    pub mechanical_strain: SymmTensor,

    /// Composition, temperature and irradiation history of this cell.
    pub material: MaterialState,

    /// Instantaneous irradiation environment and microstructure of this cell.
    pub irradiation: IrradiationState,

    /// Timestep \[s\] over which the inelastic increment accumulates.
    ///
    /// Must be non-negative. Zero is legal and means "no time passes": every
    /// creep model then returns a zero increment, and the law degenerates to
    /// rate-independent elasto-plasticity.
    pub dt: f64,

    /// Equivalent total strain rate \[1/s\], used only by
    /// [`YieldStressModel::Fraptran`](crate::rheology::YieldStressModel::Fraptran).
    ///
    /// Zircaloy's yield stress is strain-rate sensitive: pull it faster and it
    /// is stronger. The FRAPTRAN correlation floors the rate at 1e-3 /s, which
    /// is also the rate its reference data was taken at, so leaving this zero
    /// is safe and gives the quasi-static curve. Every other yield model
    /// ignores it.
    pub equivalent_strain_rate: f64,
}

impl RheologyInputs {
    /// Inputs for a rate-independent evaluation: no time passes, no
    /// irradiation, quasi-static strain rate.
    ///
    /// The convenient starting point for an elastic or elasto-plastic check,
    /// and for unit tests. Override [`dt`](Self::dt) and
    /// [`irradiation`](Self::irradiation) to bring creep into play.
    #[must_use]
    pub fn quasi_static(
        elastic: LinearElastic,
        mechanical_strain: SymmTensor,
        material: MaterialState,
    ) -> Self {
        Self {
            elastic,
            mechanical_strain,
            material,
            irradiation: IrradiationState::default(),
            dt: 0.0,
            equivalent_strain_rate: 0.0,
        }
    }
}

/// The path-dependent memory of one cell, at the **start** of the current
/// timestep.
///
/// # Why this exists
///
/// Upstream stores each of these as a `volScalarField`/`volSymmTensorField` on
/// the OpenFOAM mesh registry and reaches for `.oldTime()` to get the
/// start-of-step value. That works, but which fields a law depends on is
/// invisible until you read its body. Here they are one struct the caller owns,
/// so the dependency is in the signature.
///
/// # Lifecycle
///
/// 1. Build one per cell with [`RheologyState::pristine`].
/// 2. Each timestep, call
///    [`ConstitutiveLaw::correct`](crate::rheology::ConstitutiveLaw::correct)
///    with the *current* state — it does not mutate it.
/// 3. Once the outer mechanics iteration has converged for that timestep, call
///    [`advance`](Self::advance) with the accepted
///    [`StressCorrection`] to roll the history forward. This is upstream's
///    `updateTotalFields` plus OpenFOAM's implicit `oldTime()` rotation.
///
/// Calling `advance` inside the outer iteration instead of after it
/// double-counts the inelastic strain — the classic way to make a creep model
/// silently over-predict.
///
/// # Units — raw `f64`, strict SI
///
/// Strains dimensionless, stresses in pascal.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RheologyState {
    /// Accumulated plastic strain tensor `ε_p` \[-\]. Deviatoric: plastic flow
    /// under a von Mises criterion is volume preserving.
    pub plastic_strain: SymmTensor,

    /// Accumulated equivalent plastic strain `ε_p,eq` \[-\], the scalar measure
    /// of "how much this material has yielded in total", irrespective of
    /// direction. Monotonically non-decreasing. Drives isotropic hardening.
    pub equivalent_plastic_strain: f64,

    /// Accumulated creep strain tensor `ε_c` \[-\]. Also deviatoric.
    pub creep_strain: SymmTensor,

    /// Accumulated equivalent creep strain `ε_c,eq` \[-\].
    /// Monotonically non-decreasing.
    pub equivalent_creep_strain: f64,

    /// Accumulated equivalent **primary** creep strain \[-\].
    ///
    /// Only [`CreepModel::Limback`](crate::rheology::CreepModel::Limback) uses
    /// this; every other model leaves it zero. Primary (transient) creep is the
    /// fast initial creep of a freshly loaded material, which saturates at a
    /// stress- and temperature-dependent value; the model needs to know how
    /// much of that saturation has already been consumed.
    pub equivalent_primary_creep_strain: f64,

    /// Current yield stress `σ_y` \[Pa\].
    ///
    /// A **diagnostic cache**, not an independent state variable: every yield
    /// model in this port is a pure function of
    /// [`equivalent_plastic_strain`](Self::equivalent_plastic_strain) and the
    /// material state, so this is recomputed rather than trusted. It is kept so
    /// that post-processing can plot the hardening history without re-running
    /// the model. Zero in a [`pristine`](Self::pristine) state.
    pub yield_stress: f64,
}

impl RheologyState {
    /// Unstrained, unyielded, uncrept material — the beginning-of-life state.
    ///
    /// Every field is zero. This is what to build a fresh case from.
    ///
    /// ```
    /// use outram_park_fork_offbeat::rheology::RheologyState;
    ///
    /// let s = RheologyState::pristine();
    /// assert_eq!(s.equivalent_plastic_strain, 0.0);
    /// assert_eq!(s.equivalent_creep_strain, 0.0);
    /// ```
    #[must_use]
    pub fn pristine() -> Self {
        Self::default()
    }

    /// Roll the history forward by an accepted increment.
    ///
    /// Corresponds to upstream `misesPlasticity::updateTotalFields` together
    /// with the `oldTime()` rotation OpenFOAM performs at the end of a
    /// timestep. Call it **once per converged timestep**, never inside the
    /// outer mechanics iteration.
    pub fn advance(&mut self, correction: &StressCorrection) {
        self.plastic_strain += correction.plastic_strain_increment;
        self.equivalent_plastic_strain += correction.equivalent_plastic_strain_increment;
        self.creep_strain += correction.creep_strain_increment;
        self.equivalent_creep_strain += correction.equivalent_creep_strain_increment;
        self.equivalent_primary_creep_strain +=
            correction.equivalent_primary_creep_strain_increment;
        self.yield_stress = correction.yield_stress;
    }

    /// Total accumulated inelastic (plastic + creep) strain \[-\].
    ///
    /// This is the quantity the mechanics layer needs as an additional
    /// eigenstrain if it wants to treat the inelastic strain explicitly;
    /// upstream exposes the same sum through `correctAdditionalStrain`.
    #[must_use]
    pub fn inelastic_strain(&self) -> SymmTensor {
        self.plastic_strain + self.creep_strain
    }
}

/// The result of integrating a constitutive law over one timestep in one cell.
///
/// Carries the corrected stress **and** every internal variable that advanced,
/// because the caller — not this module — owns the state and decides when to
/// commit it.
///
/// # Units — raw `f64`, strict SI
///
/// Stresses in pascal, strains dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StressCorrection {
    /// Corrected Cauchy stress `σ` \[Pa\], **tension positive**, matching the
    /// convention of [`MechanicsSolver::stress`](crate::mechanics::MechanicsSolver::stress).
    ///
    /// `σ = K tr(ε_el) I + 2μ dev(ε_el)`, with `ε_el` the elastic strain left
    /// after the inelastic increments have been removed.
    pub stress: SymmTensor,

    /// Elastic strain `ε_el` \[-\] after this step's plastic and creep
    /// increments were removed. The strain that actually carries stress.
    pub elastic_strain: SymmTensor,

    /// Plastic strain increment `Δε_p` \[-\] over this step. Deviatoric.
    pub plastic_strain_increment: SymmTensor,

    /// Equivalent plastic strain increment `Δε_p,eq` \[-\]. Non-negative.
    pub equivalent_plastic_strain_increment: f64,

    /// Creep strain increment `Δε_c` \[-\] over this step. Deviatoric.
    pub creep_strain_increment: SymmTensor,

    /// Equivalent creep strain increment `Δε_c,eq` \[-\]. Non-negative.
    pub equivalent_creep_strain_increment: f64,

    /// Equivalent **primary** creep strain increment \[-\]. Non-negative, and
    /// zero for every model but
    /// [`CreepModel::Limback`](crate::rheology::CreepModel::Limback). Already
    /// included in
    /// [`equivalent_creep_strain_increment`](Self::equivalent_creep_strain_increment);
    /// reported separately only so the split can be post-processed.
    pub equivalent_primary_creep_strain_increment: f64,

    /// Yield stress `σ_y` \[Pa\] at the **end** of the step, i.e. evaluated at
    /// the updated equivalent plastic strain.
    pub yield_stress: f64,

    /// Whether this cell yielded plastically during the step.
    ///
    /// Upstream reports the same thing through its `activeYield` field and
    /// prints the percentage of yielding cells each step; a sudden jump is the
    /// usual first sign that a timestep is too large.
    pub yielding: bool,

    /// Local iterations the constitutive integration took (creep Newton plus
    /// plastic return map).
    ///
    /// A healthy step is a handful. A count near the cap in many cells means
    /// the timestep is too large for the creep rate, which is what
    /// [`CreepTimeStepControl`](crate::rheology::CreepTimeStepControl) exists
    /// to prevent.
    pub iterations: usize,
}

impl StressCorrection {
    /// Von Mises equivalent stress \[Pa\] of the corrected stress,
    /// `q = sqrt(3/2 · dev(σ):dev(σ))`.
    ///
    /// The scalar a yield criterion is written against. After a converged
    /// return mapping it can never exceed the yield stress by more than the
    /// local Newton tolerance.
    #[must_use]
    pub fn von_mises_stress(&self) -> f64 {
        von_mises(self.stress)
    }

    /// Hydrostatic (mean) stress \[Pa\], `tr(σ)/3`. Negative under compression.
    #[must_use]
    pub fn hydrostatic_stress(&self) -> f64 {
        self.stress.tr() / 3.0
    }
}

/// Von Mises equivalent stress \[Pa\] of a stress tensor,
/// `q = sqrt(3/2 · dev(σ):dev(σ))`.
///
/// Free function because both the yield criterion and the creep laws need it
/// and neither owns it. Note that
/// [`SymmTensor::mag_sqr`](outram_foam_basic_lib::primitives::SymmTensor::mag_sqr)
/// is the full double contraction `σ:σ` (off-diagonals counted twice), matching
/// OpenFOAM's `magSqr`, so no extra factor is needed here.
#[must_use]
pub fn von_mises(stress: SymmTensor) -> f64 {
    (1.5 * stress.dev().mag_sqr()).sqrt()
}

/// Equivalent (von Mises) measure of a **strain** tensor,
/// `sqrt(2/3 · dev(ε):dev(ε))`.
///
/// The work-conjugate partner of [`von_mises`]: the factor is 2/3 for strain
/// where it is 3/2 for stress, so that `σ_eq · ε_eq` is the plastic work per
/// unit volume in uniaxial tension.
#[must_use]
pub fn equivalent_strain(strain: SymmTensor) -> f64 {
    ((2.0 / 3.0) * strain.dev().mag_sqr()).sqrt()
}
