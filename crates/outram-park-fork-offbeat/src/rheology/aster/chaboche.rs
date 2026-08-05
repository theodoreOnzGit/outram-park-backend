// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from code_aster (https://gitlab.com/codeaster/src)
//   Copyright (C) 1991 - 2026 - EDF R&D
//   Licence: GPL-3.0-or-later
//   Upstream commit: b504ea08c2f49575e04644cee2e39a63ea45c16e
//   Sources:
//     bibfor/lc/lc0004.F90         -- dispatch for `num_lc = 4` (`lc0004`)
//     bibfor/comport/nmchab.F90    -- the Chaboche stress update (`nmchab`)
//     bibfor/comport/nmcham.F90    -- material-parameter assembly (`nmcham`)
//     bibfor/comport/nmchdp.F90    -- bracketing + scalar solve driver (`nmchdp`)
//     bibfor/nonlinear/nmchcr.F90  -- the scalar residual in `Δp` (`nmchcr`)
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Chaboche kinematic-hardening elasto-(visco)plastic laws.
//!
//! # What kinematic hardening is, and why it needs a new state variable
//!
//! Every law in [`crate::rheology::aster::viscoplastic`] is *isotropic*: the
//! yield or flow condition depends on the stress only through the von Mises
//! equivalent of its deviator `s`, so the yield surface is a sphere in
//! deviatoric space centred on the origin, which can only grow or shrink.
//!
//! That cannot reproduce what metal actually does under reversed loading. Pull
//! a steel bar into plasticity, then push it back: it yields in compression
//! well before the tensile yield stress in magnitude. This is the **Bauschinger
//! effect**, and an isotropic model gets it exactly wrong — it predicts the
//! reverse yield stress to be *larger*, not smaller.
//!
//! Chaboche's answer is to let the yield surface **translate** as well as
//! resize. Its centre is a deviatoric tensor `X`, the **back stress**, and the
//! flow condition is written on `s - X` rather than on `s`:
//!
//! `f = ||s - X||_vm - R(p)`, with `||·||_vm = sqrt(3/2 · (·):(·))`
//!
//! `X` is a genuine tensorial internal variable with its own evolution law, and
//! that is the architectural break with the isotropic family: the local
//! integration is no longer a scalar problem with a fixed flow direction.
//!
//! # The Armstrong-Frederick evolution law
//!
//! Each back stress is stored as a dimensionless **back strain** `α`, from
//! which the stress-dimensioned back stress is recovered as `X = (2/3) C α`.
//! This is upstream's storage convention (`nmchab.F90` reconstructs
//! `X = C·α/1.5` when it checks radiality), and it is kept here so a
//! code_aster state vector can be read across without rescaling.
//!
//! `α` follows Armstrong-Frederick:
//!
//! `α̇ = ε̇_p - γ(p) · δ · α · ṗ`
//!
//! The first term is *linear* (Prager) hardening — the surface centre simply
//! follows the plastic strain. The second is **dynamic recovery**: a pull-back
//! toward the origin proportional to `α` itself and to the rate of plastic
//! flow. Their competition makes `α` saturate rather than grow without bound,
//! and saturation is what produces a closed, stable hysteresis loop instead of
//! ever-increasing stress amplitude.
//!
//! Under monotonic proportional loading the saturated value is exactly
//!
//! `||X||_vm → C / γ`
//!
//! which is the classical Armstrong-Frederick result and the sharpest
//! analytical reference available for verifying this port. It is pinned by
//! [`the_back_stress_saturates_at_c_over_gamma`](self) in the test module.
//!
//! Two back stresses (`VISC_CIN2_CHAB` and friends) are used because one
//! Armstrong-Frederick tensor gives a single exponential approach to
//! saturation, which fits either the sharp knee just after yield or the long
//! tail, but not both. A fast-saturating `α₁` plus a slow-saturating `α₂`
//! reproduces both, and the saturated equivalent stress is simply
//! `R_∞ + C₁/γ₁ + C₂/γ₂`.
//!
//! # The coupled local solve, and why it still collapses to one scalar
//!
//! The unknowns of the local problem are the scalar `Δp` **and** the tensors
//! `Δα₁`, `Δα₂` — nominally 13 coupled unknowns in 3-D. Solving that as a
//! 13-dimensional Newton system is what a naive port would do. code_aster does
//! not, and the reason is worth stating because it is the whole architecture of
//! this module.
//!
//! Integrate Armstrong-Frederick with a backward-Euler step:
//!
//! `α = α_m + Δε_p - γ δ Δp α`  →  `α = (α_m + Δε_p) / (1 + γ δ Δp)`
//!
//! The update is **affine in `Δε_p`**, so
//!
//! `X = (2/3) M (α_m + Δε_p)`, with `M(Δp) = C / (1 + γ δ Δp)`
//!
//! Substituting into the elastic-predictor relation `s = s_trial - 2μ Δε_p`:
//!
//! `s - X = [s_trial - (2/3) M α_m] - (2μ + (2/3) M) Δε_p`
//!
//! Write `ŝ = s_trial - (2/3) M₁ α_m1 - (2/3) M₂ α_m2` for the bracketed term.
//! Normality says `Δε_p` is parallel to `s - X`, so the equation above is a
//! statement that `s - X` is a positive multiple of `ŝ`: **the flow direction
//! is that of `ŝ`, and the tensorial problem is radial after all.** Taking von
//! Mises norms of both sides collapses it to one scalar equation:
//!
//! `||ŝ||_vm = R(p_m + Δp) + (3μ + M₁ n₁ + M₂ n₂) Δp  [+ K (Δp/Δt)^(1/n)]`
//!
//! which is exactly upstream's `nmchcr` residual. One unknown, one equation —
//! but note that `ŝ` itself depends on `Δp` through `M(Δp)`, so unlike the
//! isotropic radial return the flow *direction rotates during the solve*. That
//! is the substantive difference, and it is why the residual here must
//! recompute `ŝ` and its norm at every trial `Δp` rather than fixing a
//! direction once from the elastic predictor.
//!
//! The collapse is exact only when `δ = 1` (`n₁ = n₂ = 1`). For the non-radial
//! variants (`δ < 1`, upstream's `CIN2_NRAD` material keyword) upstream keeps
//! the same scalar equation and folds the non-radiality into the correction
//! factors `n₁`, `n₂` evaluated on the current direction; that approximation is
//! reproduced here rather than improved on.
//!
//! # What is covered
//!
//! | ASTER name | Back stresses | Rate-dependent | Strain memory | State vars |
//! |---|---|---|---|---|
//! | `VMIS_CIN1_CHAB` | 1 | no | no | 8 |
//! | `VMIS_CIN2_CHAB` | 2 | no | no | 14 |
//! | `VISC_CIN1_CHAB` | 1 | yes | no | 8 |
//! | `VISC_CIN2_CHAB` | 2 | yes | no | 14 |
//! | `VMIS_CIN2_MEMO` | 2 | no | yes | 28 |
//! | `VISC_CIN2_MEMO` | 2 | yes | yes | 28 |
//!
//! All six share `num_lc = 4` and dispatch through the same upstream routine,
//! which is why they are one enum here rather than six.
//!
//! **`VISCOCHAB` and `VISC_TAHERI` are not in this module.** See
//! [`why_viscochab_and_visc_taheri_are_absent`](self#why-viscochab-and-visc_taheri-are-absent).
//!
//! # Why `VISCOCHAB` and `VISC_TAHERI` are absent
//!
//! Both were in the original scope for this module and both were left out
//! deliberately, for different reasons.
//!
//! - **`VISCOCHAB`** (`num_lc = 32`, 28 state variables, upstream
//!   `bibfor/algorith/rkdcha.F90` and the `cvm*` family) *is* a Chaboche law
//!   with two back stresses, but it is not formulated as a yield surface with a
//!   consistency condition. It is a pure **rate system** — 27 coupled ODEs in
//!   the viscoplastic strain, two back strains, a memory-surface centre, an
//!   isotropic radius, a memory radius and the cumulated strain — with static
//!   thermal recovery terms on the back stresses. Upstream declares
//!   `algo_inte = ("NEWTON", "NEWTON_RELI", "RUNGE_KUTTA")` for it and
//!   integrates it as an ODE system. It therefore does *not* share this
//!   module's scalar-collapse architecture; forcing it in would have meant
//!   either a second, unrelated architecture in the same file or a distorted
//!   port. Its rate function is straightforward to transcribe and is the
//!   natural next tranche.
//! - **`VISC_TAHERI`** (`num_lc = 18`, upstream `bibfor/comport/nmtahe.F90`
//!   plus nine `nmta*` helpers) turned out not to be a kinematic-hardening law
//!   at all. It is Taheri's two-surface ratcheting model, whose unknowns are
//!   two *scalars* (`dp` and the surface radius `sp`, or `xi` and `sp`) solved
//!   by a 2x2 Newton with an explicit line search. There is no back-stress
//!   tensor and nothing of this module's architecture applies to it.
//!
//! # Convention
//!
//! Raw `f64` and [`SymmTensor`] throughout, with units stated in prose — the
//! same convention as [`crate::rheology::aster::viscoplastic`]. Upstream stores
//! its tensors as Mandel six-vectors (`XX, YY, ZZ, √2·XY, √2·XZ, √2·YZ`, see
//! [`crate::rheology::aster::kinematics::AsterVoigt`]); this port works with
//! [`SymmTensor`] directly, so the `√2` scaling `nmchab.F90` applies when it
//! reads and writes `vim`/`vip` has no counterpart here and no state variable
//! needs rescaling.
//!
//! # Status
//!
//! AI-assisted port, not yet reviewed by a human and not validated against
//! code_aster output or against experiment. The tests below are *verification*
//! — against closed-form limits of the model and against internal consistency
//! conditions — not validation. See `RESPONSIBLE_USE.md`.

use outram_foam_basic_lib::primitives::SymmTensor;

use crate::error::{OffbeatError, Result};
use crate::rheology::aster::catalogue::AsterBehaviour;
use crate::rheology::aster::integration::{brent, SolverControl};
use crate::rheology::aster::viscoplastic::{deviator, von_mises_of_deviator};

/// The all-zero symmetric tensor.
///
/// [`SymmTensor::new`] is not a `const fn` upstream, so this cannot be a
/// constant.
#[must_use]
#[inline]
fn zero_tensor() -> SymmTensor {
    SymmTensor::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
}

/// Upstream's `r8miem()` — the smallest positive normalised `f64`, used as the
/// "is this quantity indistinguishable from zero" threshold in `nmchcr.F90`.
const R8MIEM: f64 = f64::MIN_POSITIVE;

// ─────────────────────────────────────────────────────────────────────────────
// Elasticity
// ─────────────────────────────────────────────────────────────────────────────

/// Isotropic elastic moduli at one instant.
///
/// # Units and valid range
///
/// `young` is Young's modulus `E` \[Pa\], strictly positive. `poisson` is
/// Poisson's ratio `ν` \[-\], which must lie strictly inside `(-1, 1/2)` for
/// the bulk and shear moduli to be positive; `ν = 1/2` is incompressible and
/// makes `3K` infinite, so it is rejected rather than returned as an infinity.
///
/// # Why two instants are needed
///
/// code_aster evaluates the moduli at both ends of the timestep because `E` and
/// `ν` are temperature-dependent and the temperature changes across a step.
/// `nmchab.F90` rescales the incoming stress by the ratio of the two so that
/// the elastic strain implied by it is preserved when the modulus changes. See
/// [`ThermoElasticStep`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElasticModuli {
    /// Young's modulus `E` \[Pa\], strictly positive.
    pub young: f64,
    /// Poisson's ratio `ν` \[-\], in `(-1, 1/2)`.
    pub poisson: f64,
}

impl ElasticModuli {
    /// Build and validate a pair of isotropic moduli.
    ///
    /// # Arguments
    ///
    /// - `young` — `E` \[Pa\], strictly positive.
    /// - `poisson` — `ν` \[-\], strictly inside `(-1, 1/2)`.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a non-positive modulus, and
    /// [`OffbeatError::OutOfRange`] for a Poisson ratio outside `(-1, 1/2)`.
    pub fn new(young: f64, poisson: f64) -> Result<Self> {
        if !(young > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "Young's modulus",
                value: young,
                unit: "Pa",
                reason: "must be strictly positive",
            });
        }
        if !(poisson > -1.0) || !(poisson < 0.5) {
            return Err(OffbeatError::OutOfRange {
                quantity: "Poisson's ratio",
                value: poisson,
                low: -1.0,
                high: 0.5,
                unit: "-",
            });
        }
        Ok(Self { young, poisson })
    }

    /// Twice the shear modulus, `2μ = E/(1+ν)` \[Pa\].
    ///
    /// Named for upstream's `deuxmu`, which is the combination that actually
    /// appears in the deviatoric stress update — carrying `2μ` rather than `μ`
    /// avoids a factor-of-two slip at every use site.
    #[must_use]
    pub fn twice_shear_modulus(self) -> f64 {
        self.young / (1.0 + self.poisson)
    }

    /// Three times the bulk modulus, `3K = E/(1-2ν)` \[Pa\].
    ///
    /// Upstream's `troisk`; the combination that multiplies the volumetric
    /// strain increment directly.
    #[must_use]
    pub fn three_times_bulk_modulus(self) -> f64 {
        self.young / (1.0 - 2.0 * self.poisson)
    }
}

/// The thermo-elastic description of one timestep.
///
/// # What it carries
///
/// The elastic moduli at the start and end of the step (they differ when the
/// temperature changed), the isotropic thermal strain *increment* accumulated
/// over the step, and the step duration.
///
/// # Units
///
/// `thermal_strain_increment` is dimensionless \[-\] and is subtracted from all
/// three normal components of the total strain increment, exactly as upstream's
/// `depsth = deps - coef·kron`. `dt` is in seconds \[s\] and must be
/// non-negative; it is used only by the rate-dependent variants, where the
/// viscous overstress is `K (Δp/Δt)^(1/n)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThermoElasticStep {
    /// Moduli at the start of the step, upstream's `matel(1:2)`.
    pub start: ElasticModuli,
    /// Moduli at the end of the step, upstream's `matel(3:4)`.
    pub end: ElasticModuli,
    /// Isotropic thermal strain increment over the step \[-\], upstream's
    /// `coef` from `verift`.
    pub thermal_strain_increment: f64,
    /// Step duration `Δt` \[s\], non-negative.
    pub dt: f64,
}

impl ThermoElasticStep {
    /// An isothermal step: the same moduli at both ends and no thermal strain.
    ///
    /// The common case in a verification test or an isothermal simulation.
    ///
    /// # Arguments
    ///
    /// - `moduli` — the (constant) elastic moduli.
    /// - `dt` — step duration \[s\], non-negative.
    #[must_use]
    pub fn isothermal(moduli: ElasticModuli, dt: f64) -> Self {
        Self {
            start: moduli,
            end: moduli,
            thermal_strain_increment: 0.0,
            dt,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// State
// ─────────────────────────────────────────────────────────────────────────────

/// The kinematic-hardening state: one or two dimensionless back strains.
///
/// # What it holds
///
/// `alpha1` and `alpha2` are the **back strains** `α₁`, `α₂` \[-\] —
/// dimensionless deviatoric tensors, upstream's `ALPHA*` and `ALPHA2*` state
/// variables. The stress-dimensioned back stress that shifts the yield surface
/// is recovered with [`BackStress::stress`] as `X = (2/3)(C₁ α₁ + C₂ α₂)`
/// \[Pa\].
///
/// `alpha2` is present but held at zero for the one-tensor laws
/// (`VMIS_CIN1_CHAB`, `VISC_CIN1_CHAB`); the law variant, not this struct,
/// decides how many are live.
///
/// # Assumptions
///
/// Both tensors should be deviatoric (`tr(α) = 0`). Nothing enforces it,
/// because the plastic strain increment that drives them is deviatoric by
/// construction, so a zero-initialised state stays deviatoric forever. Seeding
/// a non-deviatoric `α` is a caller error that will show up as a spurious
/// hydrostatic term in `X`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BackStress {
    /// First back strain `α₁` \[-\], deviatoric.
    pub alpha1: SymmTensor,
    /// Second back strain `α₂` \[-\], deviatoric. Zero for the one-tensor laws.
    pub alpha2: SymmTensor,
}

impl BackStress {
    /// The virgin state: both back strains zero, i.e. a yield surface centred
    /// on the origin.
    #[must_use]
    pub fn zero() -> Self {
        Self {
            alpha1: zero_tensor(),
            alpha2: zero_tensor(),
        }
    }

    /// The back stress `X = (2/3)(C₁ α₁ + C₂ α₂)` \[Pa\] — the centre of the
    /// yield surface in deviatoric stress space.
    ///
    /// # Arguments
    ///
    /// `c1`, `c2` are the kinematic-hardening moduli \[Pa\] *at the current
    /// accumulated plastic strain*, not their asymptotic values — see
    /// [`ChabocheLaw::kinematic_moduli`]. Passing the asymptotic `C_∞` when
    /// `k ≠ 1` gives a back stress that is wrong by the factor
    /// `1 + (k-1)e^{-wp}`.
    #[must_use]
    pub fn stress(self, c1: f64, c2: f64) -> SymmTensor {
        (self.alpha1 * c1 + self.alpha2 * c2) * (2.0 / 3.0)
    }

    /// The equivalent back stress `||X||_vm = sqrt(3/2 X:X)` \[Pa\].
    ///
    /// This is the quantity that saturates at `C/γ` under monotonic
    /// proportional loading for a single Armstrong-Frederick tensor, and at
    /// `C₁/γ₁ + C₂/γ₂` for two.
    #[must_use]
    pub fn equivalent_stress(self, c1: f64, c2: f64) -> f64 {
        von_mises_of_deviator(self.stress(c1, c2))
    }
}

/// The strain-memory-surface state of the `*_MEMO` variants.
///
/// # What the memory surface is for
///
/// Plain Chaboche saturates to a cyclic response that depends only on the
/// current strain, not on the strain amplitudes the material has already seen.
/// Real metals remember: a specimen cycled at large amplitude and then at small
/// amplitude hardens far more than one taken straight to the small amplitude.
///
/// Chaboche's memory surface models that with a surface in *plastic strain*
/// space, of radius `q` and centre `ξ`, that is dragged outward whenever the
/// plastic strain leaves it. The isotropic hardening then saturates toward a
/// level `Q(q)` set by how far the surface has been pushed, so the largest
/// amplitude ever reached is remembered.
///
/// # Units
///
/// `isotropic_increment` is in pascal \[Pa\] and is the *increment over `R₀`*,
/// matching upstream's state variable 15: the yield radius is
/// `R = R₀ + isotropic_increment`, **not** the `R_∞ + (R₀-R_∞)e^{-bp}`
/// expression the non-memory variants use. `memory_radius` and
/// `memory_centre` are dimensionless plastic strains \[-\], as is
/// `plastic_strain`.
///
/// All four fields are ignored by the variants without a memory surface.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct StrainMemory {
    /// Isotropic hardening accumulated on top of `R₀` \[Pa\]. Upstream `vim(15)`.
    pub isotropic_increment: f64,
    /// Memory-surface radius `q` \[-\]. Upstream `vim(16)`.
    pub memory_radius: f64,
    /// Memory-surface centre `ξ` in plastic-strain space \[-\]. Upstream
    /// `vim(17:22)`.
    pub memory_centre: SymmTensor,
    /// Accumulated plastic strain tensor `ε_p` \[-\]. Upstream `vim(23:28)`.
    ///
    /// Tracked only by the memory variants, because only they need the plastic
    /// strain *tensor* as opposed to its accumulated equivalent `p`.
    pub plastic_strain: SymmTensor,
}

impl StrainMemory {
    /// The virgin memory state: no accumulated hardening, a point-sized memory
    /// surface at the origin, and no plastic strain.
    #[must_use]
    pub fn zero() -> Self {
        Self {
            isotropic_increment: 0.0,
            memory_radius: 0.0,
            memory_centre: zero_tensor(),
            plastic_strain: zero_tensor(),
        }
    }
}

/// The complete internal state of a Chaboche law at one integration point.
///
/// # Correspondence with upstream's `vim`/`vip`
///
/// | Field | Upstream slot | Name |
/// |---|---|---|
/// | [`accumulated_plastic_strain`](Self::accumulated_plastic_strain) | `vim(1)` | `EPSPEQ` |
/// | [`local_iterations`](Self::local_iterations) | `vim(2)` | `INDIPLAS` |
/// | [`back_stress`](Self::back_stress)`.alpha1` | `vim(3:8)` | `ALPHA*` |
/// | [`back_stress`](Self::back_stress)`.alpha2` | `vim(9:14)` | `ALPHA2*` |
/// | [`memory`](Self::memory) | `vim(15:28)` | memory surface |
///
/// # Units
///
/// `accumulated_plastic_strain` is dimensionless \[-\] and non-decreasing.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ChabocheState {
    /// Accumulated equivalent plastic strain `p` \[-\], non-negative and
    /// non-decreasing.
    pub accumulated_plastic_strain: f64,
    /// Back strains — see [`BackStress`].
    pub back_stress: BackStress,
    /// Strain-memory state — see [`StrainMemory`]. Untouched by the variants
    /// without a memory surface.
    pub memory: StrainMemory,
    /// Local iterations used by the last step, and *de facto* the plasticity
    /// indicator: zero exactly when the step was elastic.
    ///
    /// # An upstream oddity, reproduced deliberately
    ///
    /// code_aster's catalogue names state variable 2 `INDIPLAS`, a plasticity
    /// indicator, and `nmchab.F90` reads it back as `plast` to select the
    /// tangent branch. Yet on output it writes `vip(2) = niter`, the local
    /// iteration count. The two happen to agree in effect — `niter` is zero
    /// exactly on an elastic step and at least one otherwise — so the overload
    /// is harmless, but the stored number is an iteration count and not a 0/1
    /// flag. This port stores the iteration count and names the field for what
    /// it actually is. See [`ChabocheIncrement::yielded`] for the honest flag.
    pub local_iterations: usize,
}

impl ChabocheState {
    /// The virgin state: no plastic strain, no back stress, no memory.
    #[must_use]
    pub fn zero() -> Self {
        Self {
            accumulated_plastic_strain: 0.0,
            back_stress: BackStress::zero(),
            memory: StrainMemory::zero(),
            local_iterations: 0,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Parameters
// ─────────────────────────────────────────────────────────────────────────────

/// The material parameters of the Chaboche family.
///
/// One struct serves all six laws, mirroring upstream's single `mat(1:18)`
/// array assembled by `nmcham.F90`. Which fields are *read* is decided by the
/// [`ChabocheLaw`] variant, not by this struct: a one-tensor law ignores
/// `c2_asymptotic`/`gamma2_initial`, a rate-independent law ignores
/// `viscous_stress`/`viscous_exponent`, and a law without a memory surface
/// ignores the four `memory_*` fields. Populating an ignored field is harmless
/// but has no effect — the same contract upstream has.
///
/// # The two hardening mechanisms, and which parameters drive which
///
/// **Isotropic** (the surface grows): `r0`, `r_asymptotic`, `b` give
/// `R(p) = R_∞ + (R₀ - R_∞) e^{-b p}` \[Pa\]. With `R_∞ > R₀` the material
/// hardens cyclically; with `R_∞ < R₀` it softens, which is what tempered
/// martensitic steels actually do.
///
/// **Kinematic** (the surface translates): `c1_asymptotic`, `gamma1_initial`
/// and their `2` counterparts, modulated by `k`, `w` and `a_asymptotic`:
///
/// - `C_i(p) = C_i∞ · (1 + (k-1) e^{-w p})` \[Pa\]
/// - `γ_i(p) = γ_i0 · (a_∞ + (1-a_∞) e^{-b p})` \[-\]
///
/// Set `k = 1`, `w = 0`, `a_asymptotic = 1` for constant `C` and `γ`, which is
/// the textbook Armstrong-Frederick model and the configuration the analytical
/// saturation result `||X||_vm = C/γ` applies to.
///
/// # Units, verbatim upstream keyword names
///
/// | Field | Upstream keyword | Unit |
/// |---|---|---|
/// | [`r0`](Self::r0) | `R_0` | Pa |
/// | [`r_asymptotic`](Self::r_asymptotic) | `R_I` | Pa |
/// | [`b`](Self::b) | `B` | - |
/// | [`c1_asymptotic`](Self::c1_asymptotic) | `C_I` / `C1_I` | Pa |
/// | [`gamma1_initial`](Self::gamma1_initial) | `G_0` / `G1_0` | - |
/// | [`c2_asymptotic`](Self::c2_asymptotic) | `C2_I` | Pa |
/// | [`gamma2_initial`](Self::gamma2_initial) | `G2_0` | - |
/// | [`k`](Self::k) | `K` | - |
/// | [`w`](Self::w) | `W` | - |
/// | [`a_asymptotic`](Self::a_asymptotic) | `A_I` | - |
/// | [`delta1`](Self::delta1) / [`delta2`](Self::delta2) | `DELTA1` / `DELTA2` | - |
/// | [`viscous_exponent`](Self::viscous_exponent) | `N` (`LEMAITRE`) | - |
/// | [`viscous_stress`](Self::viscous_stress) | `1 / UN_SUR_K` (`LEMAITRE`) | Pa |
/// | [`memory_eta`](Self::memory_eta) | `ETA` (`MEMO_ECRO`) | - |
/// | [`memory_q0`](Self::memory_q0) | `Q_0` (`MEMO_ECRO`) | Pa |
/// | [`memory_qm`](Self::memory_qm) | `Q_M` (`MEMO_ECRO`) | Pa |
/// | [`memory_mu`](Self::memory_mu) | `MU` (`MEMO_ECRO`) | - |
///
/// Note that upstream's `LEMAITRE` keyword supplies `UN_SUR_K = 1/K` and this
/// struct stores `K` itself, the same inversion
/// [`crate::rheology::aster::viscoplastic::NortonParameters`] makes and for the
/// same reason: `K` is what the literature tabulates and the one with an
/// interpretable unit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChabocheParameters {
    /// Initial yield radius `R₀` \[Pa\], upstream `R_0`. Strictly positive.
    pub r0: f64,
    /// Asymptotic yield radius `R_∞` \[Pa\], upstream `R_I`. May be below `R₀`
    /// (cyclic softening).
    pub r_asymptotic: f64,
    /// Isotropic-saturation rate `b` \[-\], upstream `B`. Non-negative;
    /// upstream warns (`COMPOR1_84`) on a negative value.
    pub b: f64,
    /// Asymptotic first kinematic modulus `C₁∞` \[Pa\], upstream `C_I`/`C1_I`.
    pub c1_asymptotic: f64,
    /// Initial first dynamic-recovery coefficient `γ₁₀` \[-\], upstream
    /// `G_0`/`G1_0`. Zero gives linear (Prager) kinematic hardening.
    pub gamma1_initial: f64,
    /// Asymptotic second kinematic modulus `C₂∞` \[Pa\], upstream `C2_I`.
    /// Ignored by the one-tensor laws.
    pub c2_asymptotic: f64,
    /// Initial second dynamic-recovery coefficient `γ₂₀` \[-\], upstream
    /// `G2_0`. Ignored by the one-tensor laws.
    pub gamma2_initial: f64,
    /// Kinematic-modulus ratio `k` \[-\], upstream `K`. `C(0) = k C_∞`, so
    /// `k = 1` makes `C` constant.
    pub k: f64,
    /// Kinematic-modulus saturation rate `w` \[-\], upstream `W`. Non-negative;
    /// upstream warns (`COMPOR1_84`) on a negative value.
    pub w: f64,
    /// Asymptotic recovery ratio `a_∞` \[-\], upstream `A_I`. `γ(∞) = a_∞ γ₀`,
    /// so `a_∞ = 1` makes `γ` constant.
    pub a_asymptotic: f64,
    /// First non-radiality coefficient `δ₁` \[-\], upstream `DELTA1`
    /// (`CIN2_NRAD`). Must lie in `[0, 1]`; `1` is the ordinary radial model.
    pub delta1: f64,
    /// Second non-radiality coefficient `δ₂` \[-\], upstream `DELTA2`.
    pub delta2: f64,
    /// Viscous (Norton) exponent `n` \[-\], upstream `N` under `LEMAITRE`.
    /// Strictly positive; read only by the `VISC_*` variants.
    pub viscous_exponent: f64,
    /// Viscous drag stress `K` \[Pa\], the reciprocal of upstream's
    /// `UN_SUR_K`. Strictly positive; read only by the `VISC_*` variants.
    ///
    /// The overstress it produces is `K (Δp/Δt)^(1/n)`, so a large `K` means a
    /// strongly rate-sensitive material and `K → 0` recovers the
    /// rate-independent law.
    pub viscous_stress: f64,
    /// Memory-surface progression coefficient `η` \[-\], upstream `ETA`. In
    /// `[0, 1]`; read only by the `*_MEMO` variants.
    pub memory_eta: f64,
    /// Memory-surface initial saturation level `Q₀` \[Pa\], upstream `Q_0`.
    pub memory_q0: f64,
    /// Memory-surface maximum saturation level `Q_M` \[Pa\], upstream `Q_M`.
    pub memory_qm: f64,
    /// Memory-surface saturation rate `μ` \[-\], upstream `MU`.
    pub memory_mu: f64,
}

impl ChabocheParameters {
    /// A plain Armstrong-Frederick parameter set: constant `C` and `γ`, no
    /// viscosity, no memory surface.
    ///
    /// Sets `k = 1`, `w = 0`, `a_∞ = 1`, `δ₁ = δ₂ = 1`, and leaves the viscous
    /// and memory fields at values that the rate-independent, memory-free
    /// variants ignore. This is the configuration the closed-form saturation
    /// result `||X||_vm → C/γ` applies to exactly.
    ///
    /// # Arguments
    ///
    /// - `r0` — initial yield radius \[Pa\], strictly positive.
    /// - `c1` — kinematic modulus `C₁` \[Pa\], non-negative.
    /// - `gamma1` — dynamic recovery `γ₁` \[-\], non-negative. Zero gives
    ///   linear kinematic hardening.
    #[must_use]
    pub fn armstrong_frederick(r0: f64, c1: f64, gamma1: f64) -> Self {
        Self {
            r0,
            r_asymptotic: r0,
            b: 0.0,
            c1_asymptotic: c1,
            gamma1_initial: gamma1,
            c2_asymptotic: 0.0,
            gamma2_initial: 0.0,
            k: 1.0,
            w: 0.0,
            a_asymptotic: 1.0,
            delta1: 1.0,
            delta2: 1.0,
            viscous_exponent: 1.0,
            viscous_stress: 0.0,
            memory_eta: 0.0,
            memory_q0: 0.0,
            memory_qm: 0.0,
            memory_mu: 0.0,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// The law
// ─────────────────────────────────────────────────────────────────────────────

/// A Chaboche kinematic-hardening law.
///
/// Enum dispatch rather than trait objects, per the workspace rule — the six
/// variants are exactly the six code_aster behaviours that dispatch through
/// `lc0004.F90`, so the set is closed and known at compile time.
///
/// The variant selects three independent switches, matching upstream's
/// `nmcham.F90` decoding of the behaviour name:
///
/// - **one or two back stresses** (`CIN1` vs `CIN2`/`MEMO`), upstream `nbvar`;
/// - **rate-dependent or not** (`VISC_` vs `VMIS_`), upstream `visc`;
/// - **strain memory or not** (`_MEMO`), upstream `memo`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChabocheLaw {
    /// Rate-independent Chaboche with one back stress.
    ///
    /// ASTER behaviour name: `VMIS_CIN1_CHAB` (`num_lc = 4`, 8 state
    /// variables). Upstream: `bibfor/comport/nmchab.F90` via
    /// `bibfor/lc/lc0004.F90` — legacy symbols `nmchab`, `lc0004`.
    /// Integration: `SECANTE` or `BRENT`; this port uses
    /// [`brent`](crate::rheology::aster::integration::brent).
    ///
    /// Yield condition `||s - X||_vm = R(p)` with `X = (2/3) C₁ α₁`.
    VmisCin1Chab(ChabocheParameters),

    /// Rate-independent Chaboche with two back stresses.
    ///
    /// ASTER behaviour name: `VMIS_CIN2_CHAB` (`num_lc = 4`, 14 state
    /// variables). Upstream as [`Self::VmisCin1Chab`].
    ///
    /// The second Armstrong-Frederick tensor lets one saturate fast (the knee
    /// just past yield) and the other slowly (the long tail), which one tensor
    /// cannot do.
    VmisCin2Chab(ChabocheParameters),

    /// Rate-dependent Chaboche with one back stress.
    ///
    /// ASTER behaviour name: `VISC_CIN1_CHAB` (`num_lc = 4`, 8 state
    /// variables). Upstream as [`Self::VmisCin1Chab`]; the viscous branch is
    /// `nmchcr.F90`'s `rppmdp = rppmdp + kvi·(dp/dt)^(1/n)`.
    ///
    /// The yield condition is replaced by a Norton overstress relation:
    /// `||s - X||_vm = R(p) + K (ṗ)^(1/n)`. The stress may now exceed the
    /// yield radius, by an amount set by how fast the material is being
    /// strained.
    ViscCin1Chab(ChabocheParameters),

    /// Rate-dependent Chaboche with two back stresses.
    ///
    /// ASTER behaviour name: `VISC_CIN2_CHAB` (`num_lc = 4`, 14 state
    /// variables). The combination most often used for austenitic stainless
    /// steel at reactor temperatures.
    ViscCin2Chab(ChabocheParameters),

    /// Rate-independent Chaboche with two back stresses and a strain-memory
    /// surface.
    ///
    /// ASTER behaviour name: `VMIS_CIN2_MEMO` (`num_lc = 4`, 28 state
    /// variables). See [`StrainMemory`] for what the memory surface does.
    VmisCin2Memo(ChabocheParameters),

    /// Rate-dependent Chaboche with two back stresses and a strain-memory
    /// surface.
    ///
    /// ASTER behaviour name: `VISC_CIN2_MEMO` (`num_lc = 4`, 28 state
    /// variables). The fullest member of the family.
    ViscCin2Memo(ChabocheParameters),
}

impl ChabocheLaw {
    /// The material parameters this law was built with.
    #[must_use]
    pub const fn parameters(self) -> ChabocheParameters {
        match self {
            Self::VmisCin1Chab(p)
            | Self::VmisCin2Chab(p)
            | Self::ViscCin1Chab(p)
            | Self::ViscCin2Chab(p)
            | Self::VmisCin2Memo(p)
            | Self::ViscCin2Memo(p) => p,
        }
    }

    /// The corresponding entry in the generated behaviour catalogue.
    ///
    /// Use it to reach `num_lc`, the declared state-variable names, the
    /// supported deformations and the declared integration algorithms without
    /// duplicating them here.
    #[must_use]
    pub const fn behaviour(self) -> AsterBehaviour {
        match self {
            Self::VmisCin1Chab(_) => AsterBehaviour::VmisCin1Chab,
            Self::VmisCin2Chab(_) => AsterBehaviour::VmisCin2Chab,
            Self::ViscCin1Chab(_) => AsterBehaviour::ViscCin1Chab,
            Self::ViscCin2Chab(_) => AsterBehaviour::ViscCin2Chab,
            Self::VmisCin2Memo(_) => AsterBehaviour::VmisCin2Memo,
            Self::ViscCin2Memo(_) => AsterBehaviour::ViscCin2Memo,
        }
    }

    /// The upstream ASTER behaviour name, verbatim.
    #[must_use]
    pub const fn aster_name(self) -> &'static str {
        self.behaviour().aster_name()
    }

    /// How many Armstrong-Frederick back stresses are active — 1 or 2.
    ///
    /// Upstream's `nbvar`.
    #[must_use]
    pub const fn back_stress_count(self) -> usize {
        match self {
            Self::VmisCin1Chab(_) | Self::ViscCin1Chab(_) => 1,
            _ => 2,
        }
    }

    /// Whether the law carries a viscous overstress — upstream's `visc`.
    ///
    /// `true` for the `VISC_*` variants, whose flow condition is
    /// `||s - X||_vm = R + K (Δp/Δt)^(1/n)` rather than `= R`.
    #[must_use]
    pub const fn is_rate_dependent(self) -> bool {
        matches!(
            self,
            Self::ViscCin1Chab(_) | Self::ViscCin2Chab(_) | Self::ViscCin2Memo(_)
        )
    }

    /// Whether the law carries a strain-memory surface — upstream's `memo`.
    #[must_use]
    pub const fn has_strain_memory(self) -> bool {
        matches!(self, Self::VmisCin2Memo(_) | Self::ViscCin2Memo(_))
    }

    /// The kinematic moduli `(C₁, C₂)` \[Pa\] at accumulated plastic strain
    /// `p` \[-\].
    ///
    /// `C_i(p) = C_i∞ (1 + (k-1) e^{-w p})`, upstream's `cm`/`c2m` and
    /// `cp`/`c2p`. `C₂` is returned as zero for the one-tensor laws so callers
    /// need not branch.
    #[must_use]
    pub fn kinematic_moduli(self, p: f64) -> (f64, f64) {
        let m = self.parameters();
        let factor = 1.0 + (m.k - 1.0) * (-m.w * p).exp();
        let c2 = if self.back_stress_count() == 2 {
            m.c2_asymptotic
        } else {
            0.0
        };
        (m.c1_asymptotic * factor, c2 * factor)
    }

    /// The dynamic-recovery coefficients `(γ₁, γ₂)` \[-\] at accumulated
    /// plastic strain `p` \[-\].
    ///
    /// `γ_i(p) = γ_i0 (a_∞ + (1-a_∞) e^{-b p})`, upstream's `gammap`/`gamm2p`.
    #[must_use]
    pub fn recovery_coefficients(self, p: f64) -> (f64, f64) {
        let m = self.parameters();
        let factor = m.a_asymptotic + (1.0 - m.a_asymptotic) * (-m.b * p).exp();
        let g2 = if self.back_stress_count() == 2 {
            m.gamma2_initial
        } else {
            0.0
        };
        (m.gamma1_initial * factor, g2 * factor)
    }

    /// The isotropic yield radius `R(p)` \[Pa\] of the **non-memory** variants.
    ///
    /// `R(p) = R_∞ + (R₀ - R_∞) e^{-b p}`, upstream's `rpm`/`rpp` in the
    /// `memo == 0` branch. Starts at `R₀` and saturates at `R_∞`.
    ///
    /// The `*_MEMO` variants do **not** use this expression — their radius is
    /// `R₀ + R_v` with `R_v` an integrated state variable, see
    /// [`StrainMemory::isotropic_increment`].
    #[must_use]
    pub fn isotropic_radius(self, p: f64) -> f64 {
        let m = self.parameters();
        m.r_asymptotic + (m.r0 - m.r_asymptotic) * (-m.b * p).exp()
    }

    /// The yield radius at the start of a step, for either kind of variant.
    ///
    /// Dispatches between [`Self::isotropic_radius`] and the memory variants'
    /// `R₀ + R_v`.
    #[must_use]
    pub fn start_radius(self, state: ChabocheState) -> f64 {
        if self.has_strain_memory() {
            self.parameters().r0 + state.memory.isotropic_increment
        } else {
            self.isotropic_radius(state.accumulated_plastic_strain)
        }
    }

    // ── elastic predictor ────────────────────────────────────────────────────

    /// The elastic predictor for one step — everything the local solve needs
    /// that does not depend on `Δp`.
    ///
    /// Port of the first half of `nmchab.F90`: the thermal strain is removed,
    /// the incoming stress is rescaled for a change of elastic moduli, the
    /// trial deviator is formed, and the yield function is evaluated at
    /// `Δp = 0`.
    ///
    /// # Arguments
    ///
    /// - `state` — internal state at the start of the step.
    /// - `previous_stress` — Cauchy stress at the start of the step \[Pa\].
    /// - `strain_increment` — total (mechanical + thermal) small-strain
    ///   increment `Δε` \[-\] over the step.
    /// - `step` — moduli at both ends, thermal strain increment and `Δt`.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a negative `Δt`.
    pub fn elastic_predictor(
        self,
        state: ChabocheState,
        previous_stress: SymmTensor,
        strain_increment: SymmTensor,
        step: ThermoElasticStep,
    ) -> Result<ChabochePredictor> {
        if step.dt < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "timestep",
                value: step.dt,
                unit: "s",
                reason: "must not be negative",
            });
        }

        let two_mu_old = step.start.twice_shear_modulus();
        let three_k_old = step.start.three_times_bulk_modulus();
        let two_mu = step.end.twice_shear_modulus();
        let three_k = step.end.three_times_bulk_modulus();

        // Mechanical strain increment: total minus the isotropic thermal part.
        // The deviator is unaffected by the thermal term, which is isotropic.
        let volumetric = strain_increment.tr() / 3.0 - step.thermal_strain_increment;
        let deviatoric = deviator(strain_increment);

        // Rescale the incoming stress for a change of moduli, so that the
        // elastic strain it represents is preserved. Upstream's `sigmp`.
        let mean_old = previous_stress.tr() / 3.0;
        let rescaled_mean = (three_k / three_k_old) * mean_old;
        let rescaled_deviator = deviator(previous_stress) * (two_mu / two_mu_old);

        let trial_deviator = rescaled_deviator + deviatoric * two_mu;
        let mean_stress = rescaled_mean + three_k * volumetric;

        // Yield function at Dp = 0, using C(p_m) — upstream's `cm`, `c2m`.
        let (c1_start, c2_start) = self.kinematic_moduli(state.accumulated_plastic_strain);
        let shifted_start = trial_deviator - state.back_stress.stress(c1_start, c2_start);
        let radius_start = self.start_radius(state);
        let yield_function = von_mises_of_deviator(shifted_start) - radius_start;

        // Normalisation of the residual: upstream's `denom`, built from the
        // *asymptotic* moduli so that it does not move with Dp. It only sets
        // the residual's scale; it never changes the root.
        let m = self.parameters();
        let c2_inf = if self.back_stress_count() == 2 {
            m.c2_asymptotic
        } else {
            0.0
        };
        let normalisation = von_mises_of_deviator(
            trial_deviator - state.back_stress.stress(m.c1_asymptotic, c2_inf),
        );

        Ok(ChabochePredictor {
            trial_deviator,
            mean_stress,
            yield_function,
            twice_shear_modulus: two_mu,
            normalisation,
            start_state: state,
            dt: step.dt,
        })
    }

    // ── the scalar residual ──────────────────────────────────────────────────

    /// Evaluate the local state at a trial plastic-strain increment `Δp`.
    ///
    /// This is the port of `nmchcr.F90`, and it is the heart of the module: it
    /// is where the tensorial back-stress problem is collapsed to one scalar.
    /// Upstream returns only the residual and leaves everything else in a
    /// `COMMON` block for `nmchab` to pick up afterwards; this returns the
    /// whole evaluation as a value, so the converged state is obtained by
    /// simply calling it once more at the root.
    ///
    /// # Arguments
    ///
    /// - `predictor` — the elastic predictor for the step.
    /// - `dp` — trial accumulated-plastic-strain increment `Δp` \[-\],
    ///   non-negative.
    ///
    /// # The residual
    ///
    /// [`ChabocheLocalState::residual`] is
    /// `(R + (3μ + M₁n₁ + M₂n₂)Δp + K(Δp/Δt)^(1/n) - ||ŝ||_vm) / denom`,
    /// dimensionless. It is negative at `Δp = 0` whenever the step yields and
    /// increases with `Δp`, so a bracket `[0, Δp_max]` is easy to build.
    ///
    /// Note the sign: upstream's `nmchcr` returns `-f` where `f` is the
    /// (normalised) yield function, i.e. the same increasing residual. The sign
    /// is kept identical so the two can be compared line by line.
    #[must_use]
    pub fn local_state(self, predictor: ChabochePredictor, dp: f64) -> ChabocheLocalState {
        let m = self.parameters();
        let state = predictor.start_state;
        let p_end = state.accumulated_plastic_strain + dp;

        let (c1, c2) = self.kinematic_moduli(p_end);
        let (gamma1, gamma2) = self.recovery_coefficients(p_end);

        // The implicit Armstrong-Frederick moduli: M = C / (1 + gamma*delta*Dp).
        let modulus1 = c1 / (1.0 + gamma1 * dp * m.delta1);
        let modulus2 = if self.back_stress_count() == 2 {
            c2 / (1.0 + gamma2 * dp * m.delta2)
        } else {
            0.0
        };

        // The shifted trial deviator. Its direction is the flow direction —
        // see the module documentation for why that is exact.
        let effective_deviator =
            predictor.trial_deviator - state.back_stress.stress(modulus1, modulus2);
        let effective_equivalent = von_mises_of_deviator(effective_deviator);

        // Unit-Frobenius-norm flow direction; upstream's `norm`.
        let flow_direction = if effective_equivalent > R8MIEM {
            effective_deviator * (1.5_f64.sqrt() / effective_equivalent)
        } else {
            zero_tensor()
        };
        let plastic_strain_increment = flow_direction * (dp * 1.5_f64.sqrt());

        // Isotropic hardening, and the memory surface if the variant has one.
        let (radius, memory) = if self.has_strain_memory() {
            self.memory_update(state, plastic_strain_increment, dp)
        } else {
            (self.isotropic_radius(p_end), state.memory)
        };

        // Non-radiality corrections. Upstream computes these only when the
        // corresponding delta differs from one (its `idelta` switch).
        let non_radial1 = if (m.delta1 - 1.0).abs() > f64::EPSILON {
            let beta = state.back_stress.alpha1.double_inner(flow_direction) / 1.5_f64.sqrt();
            (1.0 + gamma1 * m.delta1 * dp - gamma1 * (1.0 - m.delta1) * beta)
                / (1.0 + gamma1 * dp)
        } else {
            1.0
        };
        let non_radial2 = if self.back_stress_count() == 2
            && (m.delta2 - 1.0).abs() > f64::EPSILON
        {
            let beta = state.back_stress.alpha2.double_inner(flow_direction) / 1.5_f64.sqrt();
            (1.0 + gamma2 * m.delta2 * dp - gamma2 * (1.0 - m.delta2) * beta)
                / (1.0 + gamma2 * dp)
        } else {
            1.0
        };

        // The right-hand side of the collapsed scalar equation.
        let mut resisting = radius
            + (1.5 * predictor.twice_shear_modulus
                + modulus1 * non_radial1
                + modulus2 * non_radial2)
                * dp;
        if self.is_rate_dependent() && predictor.dt > 0.0 {
            resisting += m.viscous_stress * (dp / predictor.dt).powf(1.0 / m.viscous_exponent);
        }

        let raw = resisting - effective_equivalent;
        let residual = if predictor.normalisation <= R8MIEM {
            raw
        } else {
            raw / predictor.normalisation
        };

        ChabocheLocalState {
            increment: dp,
            effective_deviator,
            effective_equivalent,
            flow_direction,
            plastic_strain_increment,
            kinematic_modulus: [modulus1, modulus2],
            non_radial_factor: [non_radial1, non_radial2],
            recovery_coefficient: [gamma1, gamma2],
            isotropic_radius: radius,
            memory,
            residual,
        }
    }

    /// The strain-memory-surface update at a trial `Δp` — upstream's `memo`
    /// branch of `nmchcr.F90`.
    ///
    /// Returns the yield radius `R = R₀ + R_v` \[Pa\] and the updated memory
    /// state. The plastic strain is advanced by `plastic_strain_increment`; if
    /// the result lies outside the memory surface of radius `q` centred on `ξ`,
    /// the surface is dragged: `Δq = η·(excess)` and `ξ` moves toward the new
    /// plastic strain. The saturation level the isotropic hardening then chases
    /// is `Q(q) = Q_M + (Q₀ - Q_M) e^{-2μq}`, which rises from `Q₀` to `Q_M` as
    /// the surface grows — that is the memory.
    fn memory_update(
        self,
        state: ChabocheState,
        plastic_strain_increment: SymmTensor,
        dp: f64,
    ) -> (f64, StrainMemory) {
        let m = self.parameters();
        let plastic_strain = state.memory.plastic_strain + plastic_strain_increment;

        let offset = plastic_strain - state.memory.memory_centre;
        // Upstream's `grjeps = sqrt(1.5 * offset:offset)`, then `grjeps/1.5`
        // is the equivalent strain measure `sqrt(2/3 offset:offset)`.
        let equivalent_offset = (1.5 * offset.double_inner(offset)).sqrt();
        let excess = equivalent_offset / 1.5 - state.memory.memory_radius;

        let (radius_increment, centre_shift) = if excess <= 0.0 {
            (0.0, zero_tensor())
        } else {
            let dq = m.memory_eta * excess;
            let coefficient = m.memory_eta * state.memory.memory_radius + dq;
            let shift = if coefficient > R8MIEM {
                offset * ((1.0 - m.memory_eta) * dq / coefficient)
            } else {
                zero_tensor()
            };
            (dq, shift)
        };

        let memory_radius = state.memory.memory_radius + radius_increment;
        let memory_centre = state.memory.memory_centre + centre_shift;

        let saturation =
            m.memory_qm + (m.memory_q0 - m.memory_qm) * (-2.0 * m.memory_mu * memory_radius).exp();
        let previous = state.memory.isotropic_increment;
        let isotropic_increment =
            previous + m.b * (saturation - previous) * dp / (1.0 + m.b * dp);

        (
            m.r0 + isotropic_increment,
            StrainMemory {
                isotropic_increment,
                memory_radius,
                memory_centre,
                plastic_strain,
            },
        )
    }

    // ── the local solve ──────────────────────────────────────────────────────

    /// Integrate one timestep, returning the end-of-step stress and state.
    ///
    /// Port of `nmchab.F90`'s `RAPH_MECA` path. The consistent tangent operator
    /// (`nmchat.F90`, upstream's `RIGI_MECA_TANG`/`FULL_MECA`) is **not**
    /// ported — see the crate-level status note.
    ///
    /// # Arguments
    ///
    /// - `state` — internal state at the start of the step.
    /// - `previous_stress` — Cauchy stress at the start of the step \[Pa\].
    /// - `strain_increment` — total small-strain increment `Δε` \[-\].
    /// - `step` — moduli at both ends of the step, the thermal strain
    ///   increment, and `Δt` \[s\].
    /// - `control` — iteration budget and tolerances for the scalar solve. The
    ///   residual is normalised, so `residual_tol` is dimensionless;
    ///   upstream's default (`RESI_INTE`) is `1e-6`, and `1e-12` is easily
    ///   reachable here.
    ///
    /// # Assumptions
    ///
    /// Small strain (`DEFORMATION = PETIT`). The laws also declare
    /// `PETIT_REAC`, `GROT_GDEP` and — for the `VMIS_*` variants — `GDEF_LOG`,
    /// which are pre/post-processing wrappers around this same small-strain
    /// kernel; see [`crate::rheology::aster::log_strain`] for the `GDEF_LOG`
    /// wrapper.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a negative `Δt` or an unusable bracket,
    /// and [`OffbeatError::ConstitutiveNotConverged`] if the scalar solve fails
    /// within `control.max_iter`.
    pub fn integrate(
        self,
        state: ChabocheState,
        previous_stress: SymmTensor,
        strain_increment: SymmTensor,
        step: ThermoElasticStep,
        control: SolverControl,
    ) -> Result<ChabocheIncrement> {
        let predictor =
            self.elastic_predictor(state, previous_stress, strain_increment, step)?;

        // Elastic step: nothing flows, nothing hardens, the state is carried
        // through unchanged. Upstream's `seuil <= 0` branch.
        if predictor.yield_function <= 0.0 {
            let stress = predictor.trial_deviator + SymmTensor::from_diag(1.0, 1.0, 1.0) * predictor.mean_stress;
            return Ok(ChabocheIncrement {
                stress,
                state: ChabocheState {
                    local_iterations: 0,
                    ..state
                },
                plastic_strain_increment: zero_tensor(),
                equivalent_increment: 0.0,
                iterations: 0,
                yielded: false,
            });
        }

        let residual = |dp: f64| self.local_state(predictor, dp).residual;

        let upper = self.bracket_upper_bound(predictor, control.max_iter)?;
        let solution = brent(residual, (0.0, upper), &control)?;
        let dp = solution.root.max(0.0);

        let local = self.local_state(predictor, dp);

        // Back-strain update: alpha = (alpha_m + n*Deps_p)/(1 + gamma*delta*Dp),
        // written as upstream writes it so the two can be compared directly.
        let m = self.parameters();
        let alpha1 = if m.c1_asymptotic != 0.0 {
            let denominator = 1.0 + local.recovery_coefficient[0] * m.delta1 * dp;
            let increment = (local.plastic_strain_increment * local.non_radial_factor[0]
                - state.back_stress.alpha1
                    * (local.recovery_coefficient[0] * m.delta1 * dp))
                * (1.0 / denominator);
            state.back_stress.alpha1 + increment
        } else {
            state.back_stress.alpha1
        };
        let alpha2 = if self.back_stress_count() == 2 && m.c2_asymptotic != 0.0 {
            let denominator = 1.0 + local.recovery_coefficient[1] * m.delta2 * dp;
            let increment = (local.plastic_strain_increment * local.non_radial_factor[1]
                - state.back_stress.alpha2
                    * (local.recovery_coefficient[1] * m.delta2 * dp))
                * (1.0 / denominator);
            state.back_stress.alpha2 + increment
        } else {
            state.back_stress.alpha2
        };

        let deviatoric_stress =
            predictor.trial_deviator - local.plastic_strain_increment * predictor.twice_shear_modulus;
        let stress =
            deviatoric_stress + SymmTensor::from_diag(1.0, 1.0, 1.0) * predictor.mean_stress;

        Ok(ChabocheIncrement {
            stress,
            state: ChabocheState {
                accumulated_plastic_strain: state.accumulated_plastic_strain + dp,
                back_stress: BackStress { alpha1, alpha2 },
                memory: local.memory,
                local_iterations: solution.iterations,
            },
            plastic_strain_increment: local.plastic_strain_increment,
            equivalent_increment: dp,
            iterations: solution.iterations,
            yielded: true,
        })
    }

    /// An upper bound on `Δp` that brackets the root — upstream's `nmchdp.F90`.
    ///
    /// # Method
    ///
    /// The first guess is the rate-independent estimate
    /// `Δp₀ = seuil / (3μ + C₁ + C₂)`, which is exact when the moduli are
    /// constant and there is no isotropic hardening. For a rate-dependent law
    /// the Norton estimate `Δt (seuil/K)^n` is also tried, and the larger of
    /// the two taken — but only when the Norton estimate is below one, because
    /// a large exponent makes it explode.
    ///
    /// The guess is then walked by factors of ten until the residual there is
    /// positive, so the bracket `[0, Δp_max]` is guaranteed to straddle the
    /// root. This is upstream's bracketing loop, reproduced rather than
    /// replaced, because the residual's stiffness in `Δp` for a large Norton
    /// exponent makes a fixed multiplier unreliable.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] if no positive-residual bound is found
    /// within the iteration budget — the same condition upstream reports as
    /// `ALGORITH6_79`.
    fn bracket_upper_bound(self, predictor: ChabochePredictor, max_iter: usize) -> Result<f64> {
        let m = self.parameters();
        let p = predictor.start_state.accumulated_plastic_strain;
        let (c1, c2) = self.kinematic_moduli(p);

        let mut upper = predictor.yield_function / (1.5 * predictor.twice_shear_modulus + c1 + c2);
        if self.is_rate_dependent() && predictor.dt > 0.0 && m.viscous_stress > 0.0 {
            let norton =
                predictor.dt * (predictor.yield_function / m.viscous_stress).powf(m.viscous_exponent);
            if norton < 1.0 {
                upper = upper.max(norton);
            }
        }
        if !(upper > 0.0) || !upper.is_finite() {
            upper = 1.0e-12;
        }

        // Walk up until the residual is positive.
        if self.local_state(predictor, upper).residual < 0.0 {
            for _ in 0..max_iter {
                upper *= 10.0;
                if self.local_state(predictor, upper).residual >= 0.0 {
                    return Ok(upper);
                }
            }
            return Err(OffbeatError::Unphysical {
                quantity: "Chaboche local-solve bracket",
                value: upper,
                unit: "-",
                reason: "the residual stayed negative over ten decades of the plastic \
                         increment, so no root was bracketed (upstream ALGORITH6_79)",
            });
        }

        // Already positive: tighten by walking down, then step back once so the
        // bound still straddles. Upstream's "verify dpmax is not too big" loop.
        for _ in 0..max_iter {
            let smaller = upper / 10.0;
            if self.local_state(predictor, smaller).residual < 0.0 {
                return Ok(upper);
            }
            upper = smaller;
        }
        Ok(upper)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Result types
// ─────────────────────────────────────────────────────────────────────────────

/// The elastic predictor of one Chaboche step.
///
/// Everything the local solve needs that does not depend on the unknown `Δp`.
/// Built by [`ChabocheLaw::elastic_predictor`] and consumed by
/// [`ChabocheLaw::local_state`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChabochePredictor {
    /// The trial (elastic-predictor) stress deviator `s_trial` \[Pa\].
    pub trial_deviator: SymmTensor,
    /// The end-of-step hydrostatic stress `tr(σ)/3` \[Pa\]. Plastic flow is
    /// deviatoric, so this is already final.
    pub mean_stress: f64,
    /// The yield function at `Δp = 0`, `||s_trial - X_m||_vm - R(p_m)` \[Pa\].
    /// Upstream's `seuil`. Non-positive means the step is elastic.
    pub yield_function: f64,
    /// `2μ` at the end of the step \[Pa\].
    pub twice_shear_modulus: f64,
    /// The scale the residual is divided by \[Pa\] — upstream's `denom`. Only
    /// sets the residual's magnitude; it cannot move the root.
    pub normalisation: f64,
    /// Internal state at the start of the step.
    pub start_state: ChabocheState,
    /// Step duration `Δt` \[s\].
    pub dt: f64,
}

/// The local state at one trial value of `Δp`.
///
/// Returned by [`ChabocheLaw::local_state`]. Upstream leaves the same
/// quantities in a `COMMON` block; returning them makes the converged state a
/// matter of one more evaluation rather than of shared mutable state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChabocheLocalState {
    /// The trial accumulated-plastic-strain increment `Δp` \[-\].
    pub increment: f64,
    /// The shifted trial deviator `ŝ = s_trial - (2/3)(M₁α₁ + M₂α₂)` \[Pa\].
    /// Its direction is the flow direction.
    pub effective_deviator: SymmTensor,
    /// `||ŝ||_vm` \[Pa\] — upstream's `seq`.
    pub effective_equivalent: f64,
    /// The flow direction, normalised so that `n:n = 1` \[-\]. Upstream's
    /// `norm`.
    pub flow_direction: SymmTensor,
    /// The plastic strain increment `Δε_p = sqrt(3/2)·Δp·n` \[-\], deviatoric,
    /// whose equivalent measure `sqrt(2/3 Δε_p:Δε_p)` is exactly `Δp`.
    pub plastic_strain_increment: SymmTensor,
    /// The implicit Armstrong-Frederick moduli `[M₁, M₂]` \[Pa\],
    /// `M_i = C_i/(1 + γ_i δ_i Δp)`. Upstream's `mp`, `m2p`.
    pub kinematic_modulus: [f64; 2],
    /// The non-radiality corrections `[n₁, n₂]` \[-\]; both are exactly one for
    /// the ordinary radial model (`δ = 1`).
    pub non_radial_factor: [f64; 2],
    /// The dynamic-recovery coefficients `[γ₁, γ₂]` \[-\] at `p_m + Δp`.
    pub recovery_coefficient: [f64; 2],
    /// The isotropic yield radius `R` \[Pa\] at `p_m + Δp`.
    pub isotropic_radius: f64,
    /// The memory-surface state at `p_m + Δp`; unchanged from the start for
    /// variants without one.
    pub memory: StrainMemory,
    /// The dimensionless residual — see [`ChabocheLaw::local_state`].
    pub residual: f64,
}

/// The outcome of integrating one Chaboche step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChabocheIncrement {
    /// End-of-step Cauchy stress \[Pa\].
    pub stress: SymmTensor,
    /// End-of-step internal state.
    pub state: ChabocheState,
    /// The plastic strain increment `Δε_p` \[-\], deviatoric.
    pub plastic_strain_increment: SymmTensor,
    /// The accumulated-plastic-strain increment `Δp` \[-\], non-negative.
    pub equivalent_increment: f64,
    /// Local-solver iterations used; zero on an elastic step.
    pub iterations: usize,
    /// Whether the step yielded.
    ///
    /// The honest flag upstream's `INDIPLAS` was meant to be — see
    /// [`ChabocheState::local_iterations`].
    pub yielded: bool,
}

#[cfg(test)]
mod tests;
