// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from code_aster (https://gitlab.com/codeaster/src)
//   Copyright (C) 1991 - 2026 - EDF R&D
//   Licence: GPL-3.0-or-later
//   Upstream commit: b504ea08c2f49575e04644cee2e39a63ea45c16e
//   Sources:
//     bibfor/algorith/rkdcha.F90  -- the 27 internal-variable rates (legacy symbol `rkdcha`)
//     bibfor/algorith/cvmmat.F90  -- the 25-coefficient parameter map (`cvmmat`)
//     bibfor/algorith/cvmres.F90  -- the implicit residual, used here as the
//                                    independent cross-check on `rkdcha` (`cvmres`)
//     bibfor/algorith/cvmcvx.F90  -- the viscoplastic threshold (`cvmcvx`)
//     bibfor/comport/calsig.F90   -- stress from total and inelastic strain (`calsig`)
//     bibfor/comport/lcdvin.F90   -- Runge-Kutta rate dispatch (`lcdvin`)
//     bibfor/comport/rdif01.F90   -- the Runge-Kutta driver (`rdif01`)
//     code_aster/Behaviours/viscochab.py -- the behaviour declaration
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! `VISCOCHAB` — unified viscoplasticity with two back stresses, static
//! recovery and a strain-memory surface.
//!
//! # What this law is for
//!
//! A reactor component held hot under load does three things at once that a
//! simple creep law cannot describe together: it flows at a rate set by how far
//! the stress exceeds a threshold (viscoplasticity), it *remembers* the
//! direction it was last loaded in, so reversing the load yields early
//! (kinematic hardening), and while it sits it slowly forgets — thermal
//! recovery erodes the hardening it built up. `VISCOCHAB` is EDF's model for
//! exactly that combination, which is why it is the law reached for on vessel
//! and piping steels under thermal-mechanical cycling and creep-fatigue holds.
//!
//! # Why this module looks nothing like [`chaboche`](super::chaboche)
//!
//! The `VMIS_*_CHAB` family in [`chaboche`](super::chaboche) is *rate
//! independent*: a yield surface plus a consistency condition, which collapses
//! to one scalar unknown per step and is solved by radial return. `VISCOCHAB`
//! has no consistency condition — the overstress drives an explicit flow rate,
//! and every internal variable evolves by its own differential equation. There
//! is nothing to collapse. Upstream reflects this by offering a `RUNGE_KUTTA`
//! integration path (`algo_inte`), and that is the path ported here: the 27
//! coupled rates of `rkdcha.F90`, integrated by
//! [`outram_foam_basic_lib::ode::OdeIntegrator`].
//!
//! # The state — 27 rates in a 28-slot vector
//!
//! Upstream declares `nb_vari = 28` (`viscochab.py`), of which 27 evolve. In
//! upstream storage order:
//!
//! | Slots | Symbol | Meaning | Unit |
//! |---|---|---|---|
//! | 1-6 | `evi` | viscoplastic strain `ε^vi` | - |
//! | 7-12 | `a1v` | first back-strain `α₁` | - |
//! | 13-18 | `a2v` | second back-strain `α₂` | - |
//! | 19-24 | `csi` | memory-surface centre `ξ` | - |
//! | 25 | `rayvi` | isotropic hardening `R` | Pa |
//! | 26 | `qcum` | memory-surface radius `q` | - |
//! | 27 | `evcum` | accumulated equivalent viscoplastic strain `p` | - |
//! | 28 | — | integration-state indicator, rate identically zero | - |
//!
//! All six-component tensors are in code_aster's Mandel convention — ordering
//! `(XX, YY, ZZ, XY, XZ, YZ)` with the shear entries scaled by `√2`, so that a
//! plain dot product *is* the tensor double contraction. Use
//! [`AsterVoigt`] to convert; constructing the
//! six numbers by hand without the scaling is the classic way to get this
//! wrong.
//!
//! # The equations, as upstream writes them
//!
//! With `s` the stress deviator, `X_i = (2/3)·C_i·α_i` the back stresses, and
//! `n̂` the unit direction of the effective deviator:
//!
//! - effective deviator `smx = s - (2/3)(C₁α₁ + C₂α₂)`, equivalent
//!   `J = √(3/2 · smx:smx)`
//! - overstress `F = J - R - K`; **no flow at all** when `F ≤ 0`
//! - flow rate `ṗ = (F/(K₀ + A_K·R))^N · exp(ALP·(F/(K₀+A_K·R))^(N+1))`
//! - `ε̇^vi = (3/2)·(smx/J)·ṗ`, so `√(2/3 · ε̇^vi:ε̇^vi) = ṗ` exactly
//! - `α̇_i = ε̇^vi − γ_i·[D_i·α_i + (1−D_i)(α_i·n̂)n̂]·ṗ − G_Xi·‖X_i‖^(M_i−1)·α_i`
//! - `γ_i = G_i0·(A_I + (1−A_I)·e^(−B·p))`
//! - `Ṙ = B·(Q(q) − R)·ṗ + G_R·sign(Q_R − R)·|Q_R − R|^(M_R)`
//! - memory surface: `q̇ = ETA·(n̂·n̂*)·ṗ` and `ξ̇ = √(3/2)(1−ETA)(n̂·n̂*)·ṗ·n̂*`,
//!   active only while `√(2/3 · ‖ε^vi − ξ‖²_vM) > q` and `n̂·n̂* > 0`
//!
//! # The implicit reference rate of 1 s⁻¹
//!
//! `ṗ = (F/(K₀ + A_K R))^N` is dimensionally a pure number, not a rate.
//! Upstream's implicit path makes the missing factor explicit — `cvmres.F90`
//! writes `Δp = Δt·(F/K)^N` — so the parameterisation carries an **implicit
//! reference rate of 1 s⁻¹**, and `K₀` is only a stress if time is measured in
//! seconds. The same applies to `G_R`, `G_X1` and `G_X2`, whose units
//! (`Pa^(1−M)/s`) absorb the time unit. Feeding this law a timestep in hours
//! and expecting hours out will be wrong by the ratio to the fourth or fifth
//! power, silently.
//!
//! # Two places where upstream's explicit and implicit paths disagree
//!
//! Both were found by transcribing `rkdcha.F90` and `cvmres.F90` side by side.
//! This port reproduces **`rkdcha.F90`**, because that is the routine the
//! `RUNGE_KUTTA` algorithm actually runs, and pins both differences with tests
//! rather than silently correcting them — see the workspace rule on upstream
//! defects.
//!
//! 1. **`rkdcha.F90` line 124 uses `(1 − D1)` in the `α₂` equation.**
//!    Upstream reads
//!
//!    ```text
//!    da1v(itens) = d1*a1v(itens)+(1.0d0-d1)*xna1v*petin(itens)
//!    da2v(itens) = d2*a2v(itens)+(1.0d0-d1)*xna2v*petin(itens)
//!    ```
//!
//!    The second line's `d2*a2v` establishes that this is the `α₂` equation, so
//!    the `(1.0d0-d1)` immediately after it is inconsistent with its own line
//!    and with the `α₁` line above. `cvmres.F90`'s `JF` block — the same
//!    physics, integrated implicitly — uses `(1.d0-d2)` there
//!    (`zz = zz*(1.d0-d2)*g20*ccin*dp*2.d0/3.d0`), and every other term in the
//!    two blocks maps one-to-one once `X_i = (2/3)C_iα_i` is substituted. The
//!    verdict recorded here is therefore **an upstream typo in the explicit
//!    path**. It is reproduced verbatim; [`RKDCHA_ALPHA2_USES_D1`] marks it and
//!    `rkdcha_alpha2_reuses_d1_upstream_typo` measures the resulting
//!    discrepancy.
//!
//! 2. **`rkdcha.F90` zeroes *every* rate when `F ≤ 0`,** including the static
//!    recovery of `R`. `cvmres.F90`'s `RF` keeps its recovery term
//!    `sgn·G_R·Δt·|Q_R − R|^(M_R)` regardless of whether `Δp` is zero. So the
//!    two upstream paths predict different behaviour during an elastic hold:
//!    explicit recovers nothing, implicit recovers. This is structural rather
//!    than a slip of a subscript, so no "typo" verdict is claimed — it is
//!    recorded and pinned by `elastic_branch_zeroes_every_rate`.
//!
//! # What is *not* ported
//!
//! - **`A_R` (coefficient 3).** `cvmcvx.F90` forms the threshold as
//!   `J − A_R·R − K`; `rkdcha.F90` forms it as `J − R − K`, i.e. it hard-codes
//!   `A_R = 1`. The explicit path is what is ported, so `A_R` is accepted,
//!   stored and ignored, exactly as upstream ignores it.
//! - **Thermal strain, damage coupling, orthotropic elasticity and the
//!   `C_PLAN` branch** of `calsig.F90`. [`ViscoplasticChabocheSystem`] ports
//!   the isothermal, isotropic, 3-D branch only.
//! - **The tangent operator.** Upstream offers `PERTURBATION` /
//!   `VERIFICATION` only for this law; nothing analytic exists to port.
//! - **Any Jacobian.** [`OdeSystem::jacobian`] is left at its panicking
//!   default, so this system must be integrated with an *explicit* stepper
//!   ([`OdeSolver::rkf45`] or [`OdeSolver::euler`]), matching upstream's
//!   `RUNGE_KUTTA` path. Selecting [`OdeSolver::rosenbrock23`] will panic.
//!
//! # Status
//!
//! **Verification-tested draft; not validated.** Every test here is an
//! independent check of the transcription — closed-form saturation limits,
//! tensor invariants, and the two upstream discrepancies above. Nothing has
//! been compared against code_aster output or against a measured creep-fatigue
//! curve, and no such agreement is claimed.

use outram_foam_basic_lib::ode::{OdeError, OdeIntegrator, OdeSolver, OdeSystem};

use crate::error::{OffbeatError, Result};
use crate::rheology::aster::kinematics::AsterVoigt;

/// Number of internal variables upstream declares for `VISCOCHAB`
/// (`viscochab.py`, `nb_vari = 28`).
pub const INTERNAL_VARIABLE_COUNT: usize = 28;

/// Number of internal variables that actually evolve — the size of the ODE
/// system. The 28th is an integration-state indicator whose rate `rkdcha.F90`
/// sets identically to zero (`dvin(nvi) = detat`, `detat = 0`).
pub const ODE_EQUATION_COUNT: usize = 27;

/// Whether this port reproduces the `(1 − D1)` that `rkdcha.F90` line 124 uses
/// in the **`α₂`** equation, where symmetry with line 122 and the implicit path
/// `cvmres.F90` both imply `(1 − D2)`.
///
/// Always `true`: upstream defects are reproduced, not silently corrected. See
/// the module documentation for the evidence and
/// `rkdcha_alpha2_reuses_d1_upstream_typo` for the measured size of the
/// difference. If a future upstream release fixes the line, this constant and
/// that test are the two places to change.
pub const RKDCHA_ALPHA2_USES_D1: bool = true;

/// The 25 `VISCOCHAB` material keywords, in the order upstream stores them in
/// `materf(1..25, 2)`.
///
/// `cvmmat.F90` fills those 25 slots from `nomc(4..28)`, so slot `i` here is
/// upstream's `coeft(i)` as read by `rkdcha.F90`. Kept verbatim — these are
/// what a code_aster deck contains and what the literature cites.
pub const ASTER_COEFFICIENT_NAMES: [&str; 25] = [
    "K_0", "A_K", "A_R", "K", "N", "ALP", "B", "M_R", "G_R", "MU", "Q_M", "Q_0", "QR_0", "ETA",
    "C1", "M_1", "D1", "G_X1", "G1_0", "C2", "M_2", "D2", "G_X2", "G2_0", "A_I",
];

/// The 25 material coefficients of `VISCOCHAB`.
///
/// # Units and the implicit second
///
/// Stress-like coefficients are in pascal; exponents and fractions are
/// dimensionless. Three coefficients — `static_recovery_rate_r`,
/// `static_recovery_rate_x1`, `static_recovery_rate_x2` — carry
/// `Pa^(1−M)·s⁻¹`, and the flow rate itself carries an implicit `1 s⁻¹` (see
/// the module docs). **Time must be in seconds.**
///
/// # Field naming
///
/// Rust names are descriptive; the upstream keyword is given for every field so
/// a deck can be read across. Order matches
/// [`ASTER_COEFFICIENT_NAMES`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViscoplasticChabocheParameters {
    /// `K_0` — viscous drag stress at zero isotropic hardening \[Pa\].
    /// Strictly positive; it is the denominator of the flow rate.
    pub drag_stress: f64,
    /// `A_K` — how much the isotropic hardening `R` adds to the drag \[-\].
    /// Typically in `[0, 1]`. The effective drag is `K₀ + A_K·R`.
    pub drag_hardening_coupling: f64,
    /// `A_R` — multiplier on `R` in the threshold \[-\].
    ///
    /// **Stored but unused**: `rkdcha.F90` hard-codes `A_R = 1` while
    /// `cvmcvx.F90` honours it. Kept so a deck round-trips and so the
    /// difference is visible rather than lost.
    pub threshold_hardening_multiplier: f64,
    /// `K` — initial elastic threshold \[Pa\], non-negative. Flow occurs only
    /// where the effective equivalent stress exceeds `R + K`.
    pub initial_threshold: f64,
    /// `N` — Norton exponent of the flow rate \[-\]. Typically 3-20; larger
    /// means a sharper, more nearly rate-independent response.
    pub flow_exponent: f64,
    /// `ALP` — exponential-overstress coefficient \[-\], non-negative.
    ///
    /// Adds `exp(ALP·(F/K)^(N+1))` to the power law, letting the rate rise
    /// faster than any power at high overstress. Upstream skips the factor
    /// entirely when `ALP ≤ 1e-30`, and so does this port.
    pub exponential_flow_coefficient: f64,
    /// `B` — rate of isotropic saturation and of kinematic-recovery decay
    /// \[-\]. Enters both `Ṙ` and `γ_i(p)`.
    pub isotropic_rate: f64,
    /// `M_R` — exponent of the static (time) recovery of `R` \[-\].
    pub static_recovery_exponent_r: f64,
    /// `G_R` — coefficient of the static recovery of `R` \[Pa^(1−M_R)·s⁻¹\].
    /// Zero disables time recovery of the isotropic hardening.
    pub static_recovery_rate_r: f64,
    /// `MU` — controls how fast the asymptotic hardening `Q` follows the memory
    /// radius `q` \[-\], through `1 − exp(−2·MU·q)`.
    pub memory_saturation_rate: f64,
    /// `Q_M` — asymptotic isotropic hardening at a fully developed memory
    /// surface \[Pa\]. **Must be non-zero**: upstream divides by it when
    /// forming `Q_R`.
    pub hardening_saturation_max: f64,
    /// `Q_0` — asymptotic isotropic hardening at zero memory radius \[Pa\].
    pub hardening_saturation_min: f64,
    /// `QR_0` — amplitude of the recovery target offset \[Pa\], entering
    /// `Q_R = Q − QR_0·(1 − ((Q_M − Q)/Q_M)²)`.
    pub recovery_target_offset: f64,
    /// `ETA` — split of the memory-surface evolution between its radius `q` and
    /// its centre `ξ` \[-\], in `[0, 1]`. `ETA = 1` freezes the centre and, in
    /// the implicit path, disables the memory surface altogether.
    pub memory_split: f64,
    /// `C1` — modulus of the first back stress \[Pa\], through
    /// `X₁ = (2/3)·C1·α₁`.
    pub back_stress_modulus_1: f64,
    /// `M_1` — exponent of the static recovery of `X₁` \[-\].
    pub static_recovery_exponent_x1: f64,
    /// `D1` — split of the first back stress's dynamic recovery between its
    /// isotropic part `α₁` and its radial part `(α₁·n̂)n̂` \[-\], in `[0, 1]`.
    /// `D1 = 1` gives ordinary Armstrong-Frederick recovery.
    pub back_stress_recovery_split_1: f64,
    /// `G_X1` — coefficient of the static recovery of `X₁`
    /// \[Pa^(1−M_1)·s⁻¹\].
    pub static_recovery_rate_x1: f64,
    /// `G1_0` — dynamic-recovery coefficient `γ₁` at zero accumulated strain
    /// \[-\]. The saturated back stress is `C1/γ₁` in equivalent measure.
    pub dynamic_recovery_1: f64,
    /// `C2` — modulus of the second back stress \[Pa\].
    pub back_stress_modulus_2: f64,
    /// `M_2` — exponent of the static recovery of `X₂` \[-\].
    pub static_recovery_exponent_x2: f64,
    /// `D2` — split of the second back stress's dynamic recovery \[-\].
    ///
    /// **Reproduces an upstream defect**: `rkdcha.F90` applies `D2` to the
    /// `α₂` term but then uses `(1 − D1)`, not `(1 − D2)`, for the radial part.
    /// See [`RKDCHA_ALPHA2_USES_D1`].
    pub back_stress_recovery_split_2: f64,
    /// `G_X2` — coefficient of the static recovery of `X₂`
    /// \[Pa^(1−M_2)·s⁻¹\].
    pub static_recovery_rate_x2: f64,
    /// `G2_0` — dynamic-recovery coefficient `γ₂` at zero accumulated strain
    /// \[-\].
    pub dynamic_recovery_2: f64,
    /// `A_I` — floor of the dynamic-recovery decay \[-\], in `[0, 1]`.
    /// `γ_i(p) = G_i0·(A_I + (1 − A_I)·e^(−B·p))`, so `A_I = 1` freezes `γ_i`
    /// at `G_i0`.
    pub dynamic_recovery_floor: f64,
}

impl ViscoplasticChabocheParameters {
    /// Read the 25 coefficients from an upstream `coeft(1..25)` array.
    ///
    /// The array is in the order of [`ASTER_COEFFICIENT_NAMES`], which is what
    /// `cvmmat.F90` produces from the deck. Provided so a `VISCOCHAB` material
    /// block can be transcribed positionally without hand-matching 25 names.
    #[must_use]
    pub const fn from_aster_coefficients(coeft: [f64; 25]) -> Self {
        Self {
            drag_stress: coeft[0],
            drag_hardening_coupling: coeft[1],
            threshold_hardening_multiplier: coeft[2],
            initial_threshold: coeft[3],
            flow_exponent: coeft[4],
            exponential_flow_coefficient: coeft[5],
            isotropic_rate: coeft[6],
            static_recovery_exponent_r: coeft[7],
            static_recovery_rate_r: coeft[8],
            memory_saturation_rate: coeft[9],
            hardening_saturation_max: coeft[10],
            hardening_saturation_min: coeft[11],
            recovery_target_offset: coeft[12],
            memory_split: coeft[13],
            back_stress_modulus_1: coeft[14],
            static_recovery_exponent_x1: coeft[15],
            back_stress_recovery_split_1: coeft[16],
            static_recovery_rate_x1: coeft[17],
            dynamic_recovery_1: coeft[18],
            back_stress_modulus_2: coeft[19],
            static_recovery_exponent_x2: coeft[20],
            back_stress_recovery_split_2: coeft[21],
            static_recovery_rate_x2: coeft[22],
            dynamic_recovery_2: coeft[23],
            dynamic_recovery_floor: coeft[24],
        }
    }

    /// The 25 coefficients back in upstream's `coeft(1..25)` order.
    #[must_use]
    pub const fn to_aster_coefficients(self) -> [f64; 25] {
        [
            self.drag_stress,
            self.drag_hardening_coupling,
            self.threshold_hardening_multiplier,
            self.initial_threshold,
            self.flow_exponent,
            self.exponential_flow_coefficient,
            self.isotropic_rate,
            self.static_recovery_exponent_r,
            self.static_recovery_rate_r,
            self.memory_saturation_rate,
            self.hardening_saturation_max,
            self.hardening_saturation_min,
            self.recovery_target_offset,
            self.memory_split,
            self.back_stress_modulus_1,
            self.static_recovery_exponent_x1,
            self.back_stress_recovery_split_1,
            self.static_recovery_rate_x1,
            self.dynamic_recovery_1,
            self.back_stress_modulus_2,
            self.static_recovery_exponent_x2,
            self.back_stress_recovery_split_2,
            self.static_recovery_rate_x2,
            self.dynamic_recovery_2,
            self.dynamic_recovery_floor,
        ]
    }

    /// Reject parameter sets the rate function cannot evaluate.
    ///
    /// Checks only what upstream's arithmetic actually requires: a strictly
    /// positive drag stress (it is a denominator), a non-zero `Q_M` (also a
    /// denominator), a non-negative threshold, and `D1`, `D2`, `ETA`, `A_I`
    /// inside `[0, 1]`, which is where they are physically meaningful.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] naming the offending coefficient.
    pub fn validate(&self) -> Result<()> {
        if !(self.drag_stress > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "VISCOCHAB K_0 (drag stress)",
                value: self.drag_stress,
                unit: "Pa",
                reason: "must be strictly positive; it divides the overstress",
            });
        }
        if self.hardening_saturation_max == 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "VISCOCHAB Q_M",
                value: self.hardening_saturation_max,
                unit: "Pa",
                reason: "must be non-zero; upstream forms (Q_M - Q)/Q_M",
            });
        }
        if self.initial_threshold < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "VISCOCHAB K (initial threshold)",
                value: self.initial_threshold,
                unit: "Pa",
                reason: "must not be negative",
            });
        }
        for (name, value) in [
            ("VISCOCHAB D1", self.back_stress_recovery_split_1),
            ("VISCOCHAB D2", self.back_stress_recovery_split_2),
            ("VISCOCHAB ETA", self.memory_split),
            ("VISCOCHAB A_I", self.dynamic_recovery_floor),
        ] {
            if !(0.0..=1.0).contains(&value) {
                return Err(OffbeatError::Unphysical {
                    quantity: name,
                    value,
                    unit: "-",
                    reason: "D1, D2, ETA and A_I are fractions and must lie in [0, 1]",
                });
            }
        }
        Ok(())
    }
}

/// The 27 evolving internal variables of `VISCOCHAB`.
///
/// Tensor fields are in code_aster's Mandel convention — see the module docs.
/// A pristine material is [`ViscoplasticChabocheState::undeformed`].
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ViscoplasticChabocheState {
    /// `ε^vi` — viscoplastic strain \[-\], upstream `vini(1:6)`. Deviatoric:
    /// viscoplastic flow preserves volume.
    pub viscoplastic_strain: AsterVoigt,
    /// `α₁` — first back-strain \[-\], upstream `vini(7:12)`. The back stress
    /// is `X₁ = (2/3)·C1·α₁` \[Pa\].
    pub back_strain_1: AsterVoigt,
    /// `α₂` — second back-strain \[-\], upstream `vini(13:18)`.
    pub back_strain_2: AsterVoigt,
    /// `ξ` — centre of the strain-memory surface \[-\], upstream
    /// `vini(19:24)`.
    pub memory_centre: AsterVoigt,
    /// `R` — isotropic hardening \[Pa\], upstream `vini(25)`. Adds to the
    /// elastic threshold and to the viscous drag.
    pub isotropic_hardening: f64,
    /// `q` — radius of the strain-memory surface \[-\], upstream `vini(26)`.
    /// Non-negative; grows only when the strain path leaves the surface.
    pub memory_radius: f64,
    /// `p` — accumulated equivalent viscoplastic strain \[-\], upstream
    /// `vini(27)`. Monotone non-decreasing.
    pub accumulated_strain: f64,
}

impl ViscoplasticChabocheState {
    /// The pristine state: no strain, no hardening, no memory.
    #[must_use]
    pub fn undeformed() -> Self {
        Self::default()
    }

    /// Unpack from the flat 27-element ODE state vector.
    ///
    /// # Panics
    ///
    /// If `y.len() < 27`.
    #[must_use]
    pub fn from_ode_state(y: &[f64]) -> Self {
        assert!(
            y.len() >= ODE_EQUATION_COUNT,
            "VISCOCHAB needs {ODE_EQUATION_COUNT} state entries, got {}",
            y.len()
        );
        let six = |o: usize| {
            AsterVoigt::from_components([y[o], y[o + 1], y[o + 2], y[o + 3], y[o + 4], y[o + 5]])
        };
        Self {
            viscoplastic_strain: six(0),
            back_strain_1: six(6),
            back_strain_2: six(12),
            memory_centre: six(18),
            isotropic_hardening: y[24],
            memory_radius: y[25],
            accumulated_strain: y[26],
        }
    }

    /// Pack into the flat 27-element ODE state vector, in upstream's order.
    #[must_use]
    pub fn to_ode_state(self) -> Vec<f64> {
        let mut y = Vec::with_capacity(ODE_EQUATION_COUNT);
        for v in [
            self.viscoplastic_strain,
            self.back_strain_1,
            self.back_strain_2,
            self.memory_centre,
        ] {
            y.extend_from_slice(&v.components());
        }
        y.push(self.isotropic_hardening);
        y.push(self.memory_radius);
        y.push(self.accumulated_strain);
        y
    }
}

/// Time derivatives of the 27 internal variables — the output of
/// [`ViscoplasticChabocheWithMemory::internal_variable_rates`].
///
/// Units are those of the matching [`ViscoplasticChabocheState`] field divided
/// by seconds.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ViscoplasticChabocheRates {
    /// `ε̇^vi` \[1/s\], upstream `devi`. Deviatoric.
    pub viscoplastic_strain_rate: AsterVoigt,
    /// `α̇₁` \[1/s\], upstream `da1v`.
    pub back_strain_1_rate: AsterVoigt,
    /// `α̇₂` \[1/s\], upstream `da2v`.
    pub back_strain_2_rate: AsterVoigt,
    /// `ξ̇` \[1/s\], upstream `dcsi`.
    pub memory_centre_rate: AsterVoigt,
    /// `Ṙ` \[Pa/s\], upstream `drayvi`.
    pub isotropic_hardening_rate: f64,
    /// `q̇` \[1/s\], upstream `dqcum`.
    pub memory_radius_rate: f64,
    /// `ṗ` \[1/s\], upstream `devcum`. Non-negative.
    pub accumulated_strain_rate: f64,
}

impl ViscoplasticChabocheRates {
    /// Write the rates into a flat 27-element derivative vector, in upstream's
    /// order — the layout [`ViscoplasticChabocheState::from_ode_state`] reads.
    ///
    /// # Panics
    ///
    /// If `dydx.len() < 27`.
    pub fn write_ode_derivatives(self, dydx: &mut [f64]) {
        assert!(
            dydx.len() >= ODE_EQUATION_COUNT,
            "VISCOCHAB needs {ODE_EQUATION_COUNT} derivative slots, got {}",
            dydx.len()
        );
        for (block, v) in [
            self.viscoplastic_strain_rate,
            self.back_strain_1_rate,
            self.back_strain_2_rate,
            self.memory_centre_rate,
        ]
        .into_iter()
        .enumerate()
        {
            dydx[6 * block..6 * block + 6].copy_from_slice(&v.components());
        }
        dydx[24] = self.isotropic_hardening_rate;
        dydx[25] = self.memory_radius_rate;
        dydx[26] = self.accumulated_strain_rate;
    }
}

/// Elasto-viscoplastic Lemaitre-Chaboche law with strain memory and static
/// recovery.
///
/// ASTER behaviour name: `VISCOCHAB` (`num_lc = 32`, 28 internal variables, 27
/// of them evolving). Upstream: `bibfor/algorith/rkdcha.F90` for the rates,
/// reached through `bibfor/comport/lcdvin.F90` and driven by
/// `bibfor/comport/rdif01.F90` — legacy symbols `rkdcha`, `lcdvin`, `rdif01`.
/// Integration: `RUNGE_KUTTA` (ported here), or `NEWTON` / `NEWTON_RELI` via
/// `cvmres`/`cvmjac` (not ported).
///
/// The law itself is stateless — it is the parameter set plus the rate
/// function. State lives in [`ViscoplasticChabocheState`], and integration in
/// [`ViscoplasticChabocheSystem`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViscoplasticChabocheWithMemory {
    /// The 25 material coefficients.
    pub parameters: ViscoplasticChabocheParameters,
}

impl ViscoplasticChabocheWithMemory {
    /// Build the law from its coefficients, validating them.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] — see
    /// [`ViscoplasticChabocheParameters::validate`].
    pub fn new(parameters: ViscoplasticChabocheParameters) -> Result<Self> {
        parameters.validate()?;
        Ok(Self { parameters })
    }

    /// The upstream ASTER behaviour name, `"VISCOCHAB"`.
    #[must_use]
    pub const fn aster_name(self) -> &'static str {
        "VISCOCHAB"
    }

    /// The effective deviator `smx = dev(σ) − (2/3)(C₁α₁ + C₂α₂)` \[Pa\] and
    /// its von Mises equivalent `J = √(3/2 · smx:smx)` \[Pa\].
    ///
    /// Upstream `rkdcha.F90` lines 84-92. Exposed because `J − R − K` is the
    /// quantity that decides whether the material flows at all, and a caller
    /// debugging a stalled integration wants to see it.
    #[must_use]
    pub fn effective_deviator(
        self,
        stress: AsterVoigt,
        state: &ViscoplasticChabocheState,
    ) -> (AsterVoigt, f64) {
        let p = &self.parameters;
        let sig = stress.components();
        let a1 = state.back_strain_1.components();
        let a2 = state.back_strain_2.components();
        let mean = (sig[0] + sig[1] + sig[2]) / 3.0;

        let mut smx = [0.0_f64; 6];
        let mut sum_sq = 0.0_f64;
        for i in 0..6 {
            let mut v =
                sig[i] - (p.back_stress_modulus_1 * a1[i] + p.back_stress_modulus_2 * a2[i]) / 1.5;
            if i < 3 {
                v -= mean;
            }
            smx[i] = v;
            sum_sq += v * v;
        }
        (AsterVoigt::from_components(smx), (1.5 * sum_sq).sqrt())
    }

    /// The overstress `F = J − R − K` \[Pa\]. Flow occurs only where `F > 0`.
    ///
    /// Upstream `rkdcha.F90` line 94. Note that `A_R` does **not** appear —
    /// the explicit path hard-codes it to 1; see the module docs.
    #[must_use]
    pub fn overstress(self, stress: AsterVoigt, state: &ViscoplasticChabocheState) -> f64 {
        let (_, j) = self.effective_deviator(stress, state);
        j - state.isotropic_hardening - self.parameters.initial_threshold
    }

    /// Equivalent viscoplastic strain rate `ṗ` \[1/s\] for a given overstress.
    ///
    /// `ṗ = (F/(K₀ + A_K·R))^N`, multiplied by `exp(ALP·(F/(K₀+A_K·R))^(N+1))`
    /// when `ALP > 1e-30`. Returns zero for `F ≤ 0`. Upstream `rkdcha.F90`
    /// lines 105-108.
    ///
    /// Remember the implicit `1 s⁻¹` reference rate documented at module level.
    #[must_use]
    pub fn flow_rate(self, overstress: f64, isotropic_hardening: f64) -> f64 {
        if overstress <= 0.0 {
            return 0.0;
        }
        let p = &self.parameters;
        let reduced =
            overstress / (p.drag_stress + p.drag_hardening_coupling * isotropic_hardening);
        let mut rate = reduced.powf(p.flow_exponent);
        if p.exponential_flow_coefficient > 1.0e-30 {
            rate *= (p.exponential_flow_coefficient * reduced.powf(p.flow_exponent + 1.0)).exp();
        }
        rate
    }

    /// The 27 internal-variable rates at a given stress and state — the direct
    /// transcription of `rkdcha.F90`.
    ///
    /// # Arguments
    ///
    /// - `stress` — Cauchy stress `σ` \[Pa\] in Mandel convention. Held fixed
    ///   for this evaluation; the caller (or
    ///   [`ViscoplasticChabocheSystem`]) is responsible for recomputing it from
    ///   the elastic law as the state moves.
    /// - `state` — the 27 internal variables at the same instant.
    ///
    /// # Behaviour below threshold
    ///
    /// When `F ≤ 0` **every** rate is zero, including the static recovery of
    /// `R` — see discrepancy 2 in the module docs.
    #[must_use]
    pub fn internal_variable_rates(
        self,
        stress: AsterVoigt,
        state: &ViscoplasticChabocheState,
    ) -> ViscoplasticChabocheRates {
        let p = &self.parameters;
        let (smx, j) = self.effective_deviator(stress, state);
        let overstress = j - state.isotropic_hardening - p.initial_threshold;

        if overstress <= 0.0 {
            // rkdcha.F90 lines 95-104: the whole `if (critv .le. 0)` branch.
            return ViscoplasticChabocheRates::default();
        }

        let smx = smx.components();
        let a1 = state.back_strain_1.components();
        let a2 = state.back_strain_2.components();
        let evi = state.viscoplastic_strain.components();
        let csi = state.memory_centre.components();

        // Flow rate `devcum` and the two directions upstream keeps:
        //   `devi` = (3/2)(smx/J) p_dot   and   `petin` = sqrt(3/2) smx/J, a
        //   Euclidean unit vector in Mandel space.
        let p_dot = self.flow_rate(overstress, state.isotropic_hardening);
        let ccin = p.dynamic_recovery_floor
            + (1.0 - p.dynamic_recovery_floor)
                * (-p.isotropic_rate * state.accumulated_strain).exp();
        let gamma1 = p.dynamic_recovery_1 * ccin;
        let gamma2 = p.dynamic_recovery_2 * ccin;

        let mut petin = [0.0_f64; 6];
        let mut devi = [0.0_f64; 6];
        let mut na1 = 0.0_f64;
        let mut na2 = 0.0_f64;
        const SQRT_1_5: f64 = 1.224_744_871_391_589_0; // sqrt(1.5)
        for i in 0..6 {
            let dir = smx[i] / j;
            devi[i] = 1.5 * dir * p_dot;
            petin[i] = SQRT_1_5 * dir;
            na1 += a1[i] * petin[i];
            na2 += a2[i] * petin[i];
        }

        // Kinematic hardening — rkdcha.F90 lines 121-126.
        //
        // The `(1.0 - D1)` on the alpha_2 line is upstream's, not a slip in
        // transcription: see RKDCHA_ALPHA2_USES_D1 and the module docs.
        let d1 = p.back_stress_recovery_split_1;
        let d2 = p.back_stress_recovery_split_2;
        let radial_split_for_alpha2 = if RKDCHA_ALPHA2_USES_D1 { d1 } else { d2 };
        let mut da1 = [0.0_f64; 6];
        let mut da2 = [0.0_f64; 6];
        for i in 0..6 {
            let r1 = d1 * a1[i] + (1.0 - d1) * na1 * petin[i];
            da1[i] = devi[i] - gamma1 * r1 * p_dot;
            let r2 = d2 * a2[i] + (1.0 - radial_split_for_alpha2) * na2 * petin[i];
            da2[i] = devi[i] - gamma2 * r2 * p_dot;
        }

        // Static (time) recovery of the two back stresses — lines 127-145.
        // `grjx_i` is the von Mises equivalent of `X_i = (2/3) C_i alpha_i`.
        let mut norm_sq_1 = 0.0_f64;
        let mut norm_sq_2 = 0.0_f64;
        for i in 0..6 {
            norm_sq_1 += a1[i] * a1[i];
            norm_sq_2 += a2[i] * a2[i];
        }
        let x1_eq = p.back_stress_modulus_1 * (norm_sq_1 / 1.5).sqrt();
        if x1_eq > 1.0e-30 {
            let factor = x1_eq.powf(p.static_recovery_exponent_x1) / x1_eq;
            for i in 0..6 {
                da1[i] -= p.static_recovery_rate_x1 * factor * a1[i];
            }
        }
        let x2_eq = p.back_stress_modulus_2 * (norm_sq_2 / 1.5).sqrt();
        if x2_eq > 1.0e-30 {
            let factor = x2_eq.powf(p.static_recovery_exponent_x2) / x2_eq;
            for i in 0..6 {
                da2[i] -= p.static_recovery_rate_x2 * factor * a2[i];
            }
        }

        // Isotropic hardening — lines 150-158.
        let q_asym = p.hardening_saturation_min
            + (p.hardening_saturation_max - p.hardening_saturation_min)
                * (1.0 - (-2.0 * p.memory_saturation_rate * state.memory_radius).exp());
        let ratio = (p.hardening_saturation_max - q_asym) / p.hardening_saturation_max;
        let q_recovery = q_asym - p.recovery_target_offset * (1.0 - ratio * ratio);
        let gap = q_recovery - state.isotropic_hardening;
        // Fortran `sign(1.0d0, x)` is +1 at x = 0; `f64::signum` agrees.
        let r_dot = p.isotropic_rate * (q_asym - state.isotropic_hardening) * p_dot
            + p.static_recovery_rate_r
                * gap.signum()
                * gap.abs().powf(p.static_recovery_exponent_r);

        // Strain-memory surface — lines 159-183.
        let mut memory_sq = 0.0_f64;
        for i in 0..6 {
            let d = evi[i] - csi[i];
            memory_sq += d * d;
        }
        let memory_eq = (1.5 * memory_sq).sqrt();
        let mut q_dot = 0.0_f64;
        let mut dcsi = [0.0_f64; 6];
        if memory_eq / 1.5 - state.memory_radius > 0.0 {
            let scale = SQRT_1_5 / memory_eq;
            let mut petin2 = [0.0_f64; 6];
            let mut projection = 0.0_f64;
            for i in 0..6 {
                petin2[i] = scale * (evi[i] - csi[i]);
                projection += petin[i] * petin2[i];
            }
            if projection > 0.0 {
                q_dot = p.memory_split * projection * p_dot;
                let centre_scale = SQRT_1_5 * (1.0 - p.memory_split) * projection * p_dot;
                for i in 0..6 {
                    dcsi[i] = centre_scale * petin2[i];
                }
            }
        }

        ViscoplasticChabocheRates {
            viscoplastic_strain_rate: AsterVoigt::from_components(devi),
            back_strain_1_rate: AsterVoigt::from_components(da1),
            back_strain_2_rate: AsterVoigt::from_components(da2),
            memory_centre_rate: AsterVoigt::from_components(dcsi),
            isotropic_hardening_rate: r_dot,
            memory_radius_rate: q_dot,
            accumulated_strain_rate: p_dot,
        }
    }
}

/// The `VISCOCHAB` rate system as a 27-equation
/// [`OdeSystem`], ready for
/// [`OdeIntegrator`].
///
/// # What the independent variable is
///
/// `x` is **time within the step**, running from `0` to
/// [`step_duration`](Self::step_duration) \[s\]. Upstream's driver `rdif01.F90`
/// uses the same convention and forms the total strain by linear interpolation,
/// `ε(x) = ε_start + Δε · x/Δt`; `calsig.F90` then gives the stress. Both are
/// reproduced here, isothermal and isotropic only (see the module docs for what
/// is left out).
///
/// # Why the stress is not part of the state
///
/// Under strain control the stress is a *function* of the state:
/// `σ = C:(ε(x) − ε^vi)`. Carrying it as an extra unknown would over-determine
/// the system; upstream recomputes it at every derivative evaluation, and so
/// does this port.
///
/// # Integrator choice
///
/// [`OdeSystem::jacobian`] is not implemented, so use
/// [`OdeSolver::rkf45`] or [`OdeSolver::euler`].
/// [`OdeSolver::rosenbrock23`] will panic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViscoplasticChabocheSystem {
    /// The constitutive law and its 25 coefficients.
    pub law: ViscoplasticChabocheWithMemory,
    /// Young's modulus `E` \[Pa\], strictly positive.
    pub young_modulus: f64,
    /// Poisson's ratio `ν` \[-\], in `(-1, 0.5)`.
    pub poisson_ratio: f64,
    /// Total strain at the start of the step \[-\], Mandel convention.
    pub total_strain_start: AsterVoigt,
    /// Total-strain increment over the step \[-\], Mandel convention.
    pub total_strain_increment: AsterVoigt,
    /// Step duration `Δt` \[s\], strictly positive.
    pub step_duration: f64,
}

impl ViscoplasticChabocheSystem {
    /// Assemble a strain-driven `VISCOCHAB` system for one step.
    ///
    /// # Arguments
    ///
    /// - `law` — the constitutive law.
    /// - `young_modulus` — `E` \[Pa\], strictly positive.
    /// - `poisson_ratio` — `ν` \[-\], in `(-1, 0.5)`; `0.5` is excluded because
    ///   the bulk term divides by `1 − 2ν`.
    /// - `total_strain_start`, `total_strain_increment` — the imposed strain
    ///   path \[-\], linearly interpolated across the step.
    /// - `step_duration` — `Δt` \[s\], strictly positive.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a non-positive modulus or duration, or
    /// a Poisson ratio outside `(-1, 0.5)`.
    pub fn new(
        law: ViscoplasticChabocheWithMemory,
        young_modulus: f64,
        poisson_ratio: f64,
        total_strain_start: AsterVoigt,
        total_strain_increment: AsterVoigt,
        step_duration: f64,
    ) -> Result<Self> {
        if !(young_modulus > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "Young's modulus",
                value: young_modulus,
                unit: "Pa",
                reason: "must be strictly positive",
            });
        }
        if !(poisson_ratio > -1.0 && poisson_ratio < 0.5) {
            return Err(OffbeatError::Unphysical {
                quantity: "Poisson's ratio",
                value: poisson_ratio,
                unit: "-",
                reason: "must lie in (-1, 0.5); the bulk term divides by 1 - 2nu",
            });
        }
        if !(step_duration > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "step duration",
                value: step_duration,
                unit: "s",
                reason: "must be strictly positive; the strain path divides by it",
            });
        }
        Ok(Self {
            law,
            young_modulus,
            poisson_ratio,
            total_strain_start,
            total_strain_increment,
            step_duration,
        })
    }

    /// Cauchy stress \[Pa\] at time `x` \[s\] within the step, for a given
    /// viscoplastic strain.
    ///
    /// The isothermal, isotropic, 3-D branch of `calsig.F90`:
    /// `σ = 2μ·ε_el + λ·tr(ε_el)·I` with `ε_el = ε(x) − ε^vi`, `2μ = E/(1+ν)`
    /// and `λ = ν·2μ/(1−2ν)`.
    #[must_use]
    pub fn stress_at(&self, x: f64, viscoplastic_strain: AsterVoigt) -> AsterVoigt {
        let fraction = x / self.step_duration;
        let e0 = self.total_strain_start.components();
        let de = self.total_strain_increment.components();
        let evi = viscoplastic_strain.components();

        let mut elastic = [0.0_f64; 6];
        for i in 0..6 {
            elastic[i] = e0[i] + de[i] * fraction - evi[i];
        }
        let two_mu = self.young_modulus / (1.0 + self.poisson_ratio);
        let trace = elastic[0] + elastic[1] + elastic[2];
        let bulk = self.poisson_ratio * two_mu * trace / (1.0 - 2.0 * self.poisson_ratio);

        let mut sigma = [0.0_f64; 6];
        for i in 0..6 {
            sigma[i] = two_mu * elastic[i] + if i < 3 { bulk } else { 0.0 };
        }
        AsterVoigt::from_components(sigma)
    }

    /// Integrate one step from `state`, returning the state at `Δt`.
    ///
    /// Uses [`OdeIntegrator::typed`] — the system is owned by value, so nothing
    /// here borrows and no lifetime parameter appears.
    ///
    /// # Arguments
    ///
    /// - `state` — internal variables at the start of the step.
    /// - `solver` — an **explicit** stepper; see the type-level note.
    /// - `initial_step` — first sub-step to attempt \[s\]. A tenth of
    ///   [`step_duration`](Self::step_duration) is a reasonable default; the
    ///   adaptive controller takes over from there.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::ConstitutiveNotConverged`] if the adaptive controller
    /// gives up — either the tolerance became unreachable or the sub-step
    /// budget ran out. The `cell` field is zero: this call knows nothing about
    /// mesh position.
    pub fn integrate_step(
        self,
        state: ViscoplasticChabocheState,
        solver: OdeSolver,
        initial_step: f64,
    ) -> Result<ViscoplasticChabocheState> {
        let duration = self.step_duration;
        let mut integrator = OdeIntegrator::typed(self, solver);
        let mut y = state.to_ode_state();
        let mut dx = initial_step;
        match integrator.integrate(0.0, duration, &mut y, &mut dx) {
            Ok(()) => Ok(ViscoplasticChabocheState::from_ode_state(&y)),
            Err(OdeError::StepSizeUnderflow) => Err(OffbeatError::ConstitutiveNotConverged {
                cell: 0,
                residual: f64::NAN,
                iterations: 0,
            }),
            Err(OdeError::MaxStepsExceeded(n)) => Err(OffbeatError::ConstitutiveNotConverged {
                cell: 0,
                residual: f64::NAN,
                iterations: n,
            }),
        }
    }
}

impl OdeSystem for ViscoplasticChabocheSystem {
    fn n_eqns(&self) -> usize {
        ODE_EQUATION_COUNT
    }

    fn derivatives(&self, x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        let state = ViscoplasticChabocheState::from_ode_state(y);
        let stress = self.stress_at(x, state.viscoplastic_strain);
        let rates = self.law.internal_variable_rates(stress, &state);
        if dydx.len() < ODE_EQUATION_COUNT {
            dydx.resize(ODE_EQUATION_COUNT, 0.0);
        }
        rates.write_ode_derivatives(dydx);
    }
}

#[cfg(test)]
mod tests;
