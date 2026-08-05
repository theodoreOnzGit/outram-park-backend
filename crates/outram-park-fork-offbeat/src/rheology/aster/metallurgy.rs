// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from code_aster (https://gitlab.com/codeaster/src)
//   Copyright (C) 1991 - 2026 - EDF R&D
//   Licence: GPL-3.0-or-later
//   Upstream commit: b504ea08c2f49575e04644cee2e39a63ea45c16e
//   Sources:
//     bibfor/comport/nmvpir.F90        -- VISC_IRRA_LOG / GRAN_IRRA_LOG driver
//                                         (legacy symbol `nmvpir`, `num_lc = 28`)
//     bibfor/algorith/granac.F90       -- irradiation-growth increment (`granac`)
//     bibfor/lc/lc0028.F90             -- dispatch for `num_lc = 28`
//     bibfor/lc/lc0030.F90             -- dispatch for `num_lc = 30` (IRRAD3M)
//     bibfor/algorith/irrmat.F90       -- IRRAD3M material preparation (`irrmat`)
//     bibfor/algorith/irrres.F90       -- IRRAD3M local residuals (`irrres`)
//     mfront/META_LEMA_ANI.mfront      -- META_LEMA_ANI (`num_lc = 58`)
//     bibfor/utilifor/lcnrts.F90       -- von Mises norm `sqrt(3/2 s:s)` (`lcnrts`)
//     bibfor/utilifor/lcdevi.F90       -- the deviator (`lcdevi`)
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Metallurgical and irradiation constitutive laws (bead `op-a7p.5`).
//!
//! # Why a separate module from [`viscoplastic`](super::viscoplastic)
//!
//! The laws in [`viscoplastic`](super::viscoplastic) are *isotropic* and driven
//! by *time*. The laws here break one or both of those assumptions, and each
//! break is the whole point of the law:
//!
//! | Law | What it breaks |
//! |---|---|
//! | [`LogarithmicIrradiationLaw`] | driven by **fluence**, not time; the rate does not depend on the clock at all |
//! | [`Irrad3m`] | adds **swelling** (a volumetric eigenstrain) and an **incubation threshold** before irradiation creep starts |
//! | [`MetaLemaAni`] | **anisotropic** — the equivalent stress is a Hill quadratic form, so the return direction is not the stress deviator |
//!
//! All three exist because cladding and vessel internals live in a neutron
//! flux, and a flux does things temperature alone does not.
//!
//! # Fluence and flux units — read this before using any law here
//!
//! Getting these wrong is the single most likely way to produce a confident
//! wrong answer from this module, because every quantity involved is a pure
//! number whose meaning lives entirely in a convention.
//!
//! **Upstream fixes no unit.** code_aster's `IRRA` is a user-supplied command
//! variable (`AFFE_VARC`), and every irradiation coefficient is read from the
//! user's own material record. Consistency between the two is the user's
//! responsibility, and nothing in the Fortran checks it.
//!
//! **This port therefore declares a convention per law and states it in the
//! parameter documentation**, because "unit-agnostic" is not a usable contract
//! for a Rust API:
//!
//! - [`LogarithmicIrradiationParameters`] — fast neutron fluence `Φ` in
//!   **n/m²** (E > 1 MeV), the SI form. A user carrying n/cm² instead must
//!   multiply both `primary_fluence_constant` and `secondary_compliance` by
//!   `1e4`; `primary_compliance` is unaffected. Using the wrong one changes the
//!   creep by a factor of `1e4`, and nothing in the arithmetic will complain.
//! - [`Irrad3mParameters`] — irradiation dose in **dpa** (displacements per
//!   atom), which is the convention `R5.03.13` uses for the 304/316 stainless
//!   internals this law was fitted to. dpa is *not* a fluence: it is a damage
//!   measure, and the conversion to n/m² is spectrum-dependent and is not
//!   performed here.
//!
//! Neither law uses a **flux** at all — both are driven by the *fluence
//! increment* over the step. That is a deliberate difference from
//! [`LemaitreIrradiation`](super::viscoplastic::ViscoplasticLaw::LemaitreIrradiation),
//! which does take a fast flux `φ̇` in n/(m²·s). If you are switching between
//! the two, that is the boundary at which a factor of the timestep gets lost.
//!
//! # Temperature
//!
//! Upstream passes temperature in **degrees Celsius** and adds `r8t0()`
//! (273.15) at each Arrhenius evaluation. This port takes **kelvin
//! throughout** — no conversion happens inside, and passing Celsius will give a
//! wildly wrong Arrhenius factor rather than an error.
//!
//! # What is ported and what is not
//!
//! Ported: `VISC_IRRA_LOG`, `GRAN_IRRA_LOG`, `IRRAD3M`, and the *mechanical*
//! half of `META_LEMA_ANI`. **Not** ported: the `ZIRC` / `ZIRC_META` phase
//! kinetics (upstream `bibfor/metallurgy/zedgar.F90`) — those are `PHASE`-type
//! state laws rather than mechanical ones, and [`MetaLemaAni`] takes the β-phase
//! fraction as an *input* precisely so the two can be ported independently.
//!
//! # Status
//!
//! **Verification only.** Every test in this module checks the port against
//! upstream's algebra, an analytical limit, or an invariant. None of it is
//! validation: no result here has been compared with reactor data or with
//! code_aster output, and per `RESPONSIBLE_USE.md` these laws remain untrusted
//! draft material until the maintainer reviews them.

use outram_foam_basic_lib::primitives::{SymmTensor, Vector3};

use crate::error::{OffbeatError, Result};
use crate::rheology::aster::integration::{brent, SolverControl};
use crate::rheology::aster::viscoplastic::{deviator, von_mises_of_deviator, CreepIncrement};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Scale every component of a symmetric tensor.
fn scale_tensor(t: SymmTensor, f: f64) -> SymmTensor {
    SymmTensor::new(f * t.xx, f * t.xy, f * t.xz, f * t.yy, f * t.yz, f * t.zz)
}

/// Rebuild a stress tensor from a deviator and a mean (hydrostatic) stress.
fn from_deviator_and_mean(s: SymmTensor, mean: f64) -> SymmTensor {
    SymmTensor::new(s.xx + mean, s.xy, s.xz, s.yy + mean, s.yz, s.zz + mean)
}

/// Reject a non-positive shear modulus.
fn check_shear_modulus(shear_modulus: f64) -> Result<()> {
    if !(shear_modulus > 0.0) {
        return Err(OffbeatError::Unphysical {
            quantity: "shear modulus",
            value: shear_modulus,
            unit: "Pa",
            reason: "must be strictly positive",
        });
    }
    Ok(())
}

/// Reject a non-positive absolute temperature.
fn check_temperature(temperature: f64) -> Result<()> {
    if !(temperature > 0.0) {
        return Err(OffbeatError::Unphysical {
            quantity: "temperature",
            value: temperature,
            unit: "K",
            reason: "must be strictly positive and expressed in kelvin; the \
                     Arrhenius factor divides by it",
        });
    }
    Ok(())
}

// ===========================================================================
// 1. Logarithmic irradiation creep — VISC_IRRA_LOG and GRAN_IRRA_LOG
// ===========================================================================

/// Parameters of the logarithmic irradiation-creep law.
///
/// # The physics, for a reader who has not met irradiation creep
///
/// A metal in a neutron flux creeps under stresses far below those that would
/// make it creep thermally. Neutrons knock atoms off their lattice sites,
/// producing vacancies and interstitials continuously; those point defects
/// migrate and are absorbed preferentially at dislocations whose orientation
/// suits the applied stress, and the material flows. The controlling variable
/// is therefore **accumulated damage** — fluence — not elapsed time. Two
/// specimens at the same stress and temperature, one irradiated and one not,
/// will creep by wildly different amounts over the same hour.
///
/// # Why "logarithmic"
///
/// The creep rate per unit fluence is
///
/// `dp/dΦ = σ_eq · exp(-Q/(R·T)) · (A·C_t / (1 + C_t·Φ) + B)`
///
/// which integrates in closed form to
///
/// `p = σ_eq · exp(-Q/(R·T)) · (A·ln(1 + C_t·Φ) + B·Φ)`
///
/// The first term saturates **logarithmically** — that is primary irradiation
/// creep, fast at first and slowing as the defect microstructure reaches a
/// steady state. The second is linear in fluence: secondary, steady-state
/// irradiation creep, which never saturates and is what dominates over a fuel
/// cycle.
///
/// # Linearity in stress, and why that matters
///
/// The law is **linear in `σ_eq`** — a stress exponent of exactly 1. That is a
/// genuine feature of irradiation creep at reactor stress levels, not a
/// simplification, and it is what makes the step integration closed-form
/// (upstream declares `algo_inte = ANALYTIQUE`): no local iteration is needed
/// at all. Contrast
/// [`LemaitreIrradiation`](super::viscoplastic::ViscoplasticLaw::LemaitreIrradiation),
/// whose exponent `n` is a free parameter and which therefore needs a root
/// find.
///
/// # Units
///
/// Fluence `Φ` in **n/m²** (fast, E > 1 MeV) by this port's convention — see
/// the module documentation for the n/cm² trap. The parameter units follow from
/// requiring `dp/dΦ · ΔΦ` to be dimensionless with `σ_eq` in pascal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogarithmicIrradiationParameters {
    /// Primary (saturating) creep amplitude `A` \[1/Pa\]. Upstream `A`.
    ///
    /// Multiplies the logarithmic term. Because the term saturates, `A` sets
    /// the *total* primary creep strain per unit stress, not a rate.
    pub primary_compliance: f64,
    /// Secondary (steady-state) creep compliance `B` \[1/(Pa·n/m²)\].
    /// Upstream `B`.
    ///
    /// Creep strain per unit stress per unit fluence, once primary creep has
    /// saturated. This is the parameter that dominates a full cycle.
    pub secondary_compliance: f64,
    /// Primary-creep fluence constant `C_t` \[1/(n/m²)\]. Upstream `CSTE_TPS`.
    ///
    /// Its reciprocal is the fluence at which primary creep is roughly half
    /// saturated, so `1/C_t` is the natural "primary creep dose". Must be
    /// non-negative; zero disables the primary term.
    pub primary_fluence_constant: f64,
    /// Activation temperature `Q/R` \[K\]. Upstream `ENER_ACT`.
    ///
    /// Enters as `exp(-Q/(R·T))` with `T` in kelvin. Note that irradiation
    /// creep is only weakly thermally activated compared with thermal creep, so
    /// this is typically a few thousand kelvin rather than tens of thousands.
    pub activation_temperature: f64,
}

impl LogarithmicIrradiationParameters {
    /// Creep compliance `C` \[1/Pa\] over one step, **exactly as upstream
    /// computes it**.
    ///
    /// The equivalent creep increment is then `Δp = C · σ_eq` with `σ_eq` the
    /// *end-of-step* equivalent stress.
    ///
    /// # Upstream expression
    ///
    /// `nmvpir.F90` computes
    ///
    /// ```text
    /// dp1 = exp(-ener/(tp + r8t0()))
    /// dp1 = dp1 * (a*ctps/(1 + ctps*irrap) + b) * (irrap - irram)
    /// ```
    ///
    /// This is a **right-endpoint rectangle rule** on the fluence integral: the
    /// rate is evaluated at the end-of-step fluence `Φ⁺` and multiplied by the
    /// whole increment `ΔΦ`. Because the primary term decreases with fluence,
    /// the rectangle sits below the curve and upstream therefore
    /// **under-predicts** primary creep on a coarse fluence step. See
    /// [`exact_creep_compliance`](Self::exact_creep_compliance) for the closed
    /// form this converges to, and the module tests for the measured
    /// first-order convergence.
    ///
    /// # Arguments
    ///
    /// - `fluence_start` — `Φ⁻` \[n/m²\], the fluence at the start of the step.
    /// - `fluence_increment` — `ΔΦ` \[n/m²\], non-negative. Upstream raises
    ///   `ALGORITH8_88` on a decreasing fluence.
    /// - `temperature` — `T` \[K\], strictly positive, **end-of-step** value
    ///   (upstream uses `tp`, not the mid-step temperature).
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a negative fluence, a negative fluence
    /// increment, or a non-positive temperature.
    pub fn creep_compliance(
        self,
        fluence_start: f64,
        fluence_increment: f64,
        temperature: f64,
    ) -> Result<f64> {
        self.check(fluence_start, fluence_increment, temperature)?;
        let fluence_end = fluence_start + fluence_increment;
        let arrhenius = (-self.activation_temperature / temperature).exp();
        let rate = self.primary_compliance * self.primary_fluence_constant
            / (1.0 + self.primary_fluence_constant * fluence_end)
            + self.secondary_compliance;
        Ok(arrhenius * rate * fluence_increment)
    }

    /// Creep compliance `C` \[1/Pa\] from the **exact** fluence integral.
    ///
    /// `C = exp(-Q/(R·T)) · [ A·ln((1 + C_t·Φ⁺)/(1 + C_t·Φ⁻)) + B·(Φ⁺ - Φ⁻) ]`
    ///
    /// This is not what upstream evaluates; it is the limit upstream's
    /// rectangle rule converges to as the step is refined, and it is provided
    /// so that convergence can be *measured* rather than asserted. It is also
    /// the honest choice for a caller taking large fluence steps, at the cost
    /// of no longer reproducing code_aster's numbers step for step.
    ///
    /// Arguments, units and errors are exactly as
    /// [`creep_compliance`](Self::creep_compliance).
    ///
    /// # Errors
    ///
    /// As [`creep_compliance`](Self::creep_compliance).
    pub fn exact_creep_compliance(
        self,
        fluence_start: f64,
        fluence_increment: f64,
        temperature: f64,
    ) -> Result<f64> {
        self.check(fluence_start, fluence_increment, temperature)?;
        let fluence_end = fluence_start + fluence_increment;
        let arrhenius = (-self.activation_temperature / temperature).exp();
        let ct = self.primary_fluence_constant;
        let primary = if ct > 0.0 {
            self.primary_compliance * ((1.0 + ct * fluence_end) / (1.0 + ct * fluence_start)).ln()
        } else {
            0.0
        };
        let secondary = self.secondary_compliance * fluence_increment;
        Ok(arrhenius * (primary + secondary))
    }

    /// Shared argument validation for the two compliance routines.
    fn check(self, fluence_start: f64, fluence_increment: f64, temperature: f64) -> Result<()> {
        check_temperature(temperature)?;
        if fluence_start < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "fluence at start of step",
                value: fluence_start,
                unit: "n/m^2",
                reason: "must not be negative",
            });
        }
        if fluence_increment < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "fluence increment",
                value: fluence_increment,
                unit: "n/m^2",
                reason: "must not be negative; fluence accumulates and cannot \
                         decrease (upstream ALGORITH8_88)",
            });
        }
        if self.primary_fluence_constant < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "primary fluence constant CSTE_TPS",
                value: self.primary_fluence_constant,
                unit: "1/(n/m^2)",
                reason: "must not be negative; a negative value makes the \
                         primary term diverge at finite fluence",
            });
        }
        Ok(())
    }
}

/// The direction along which irradiation growth acts, as upstream's `ANGL_REP`
/// pair of Euler angles.
///
/// # What irradiation growth is
///
/// Zirconium alloys are hexagonal and strongly textured after tube drawing.
/// Under irradiation they *change shape at constant volume with no applied
/// stress at all*: interstitials condense on prismatic planes and vacancies on
/// basal planes, so the crystal lengthens along one axis and thins along the
/// others. In a fuel assembly this elongates the rods and the guide tubes over
/// a cycle, and it is a design driver for the assembly hold-down springs.
///
/// It is a **stress-free eigenstrain**, like thermal expansion: it changes the
/// strain the elastic predictor sees, and does not itself relax.
///
/// # Angles
///
/// Both in **radians**. Upstream takes them from `AFFE_CARA_ELEM`'s `MASSIF`
/// keyword, in degrees, and converts before calling. `azimuth` is upstream's
/// `alpha`, `elevation` is upstream's `beta`; the intended growth direction is
/// `n = (cos α cos β, sin α cos β, -sin β)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IrradiationGrowthDirection {
    /// Rotation about the z axis, `α` \[rad\]. Upstream `ANGL_REP(1)`.
    pub azimuth: f64,
    /// Elevation out of the xy plane, `β` \[rad\]. Upstream `ANGL_REP(2)`.
    ///
    /// Upstream rejects a non-zero value in 2-D (`ALGORITH11_82`).
    pub elevation: f64,
}

impl IrradiationGrowthDirection {
    /// The unit vector growth is intended to act along.
    ///
    /// `n = (cos α cos β, sin α cos β, -sin β)`, which is a unit vector for any
    /// `α`, `β`. Five of upstream's six growth-tensor components agree with
    /// `n ⊗ n` built from this vector; the sixth does not — see
    /// [`strain_increment`](Self::strain_increment).
    #[must_use]
    pub fn unit_vector(self) -> Vector3 {
        let (sa, ca) = self.azimuth.sin_cos();
        let (sb, cb) = self.elevation.sin_cos();
        Vector3::new(ca * cb, sa * cb, -sb)
    }

    /// Growth strain increment tensor, **reproducing upstream `nmvpir.F90`
    /// verbatim, including an apparent defect**.
    ///
    /// # Upstream expression
    ///
    /// ```text
    /// degran(1) = depsgr*caa*caa*cba*cba          ! xx
    /// degran(2) = depsgr*saa*saa*sba*sba          ! yy   <-- see below
    /// degran(3) = depsgr*sba*sba                  ! zz
    /// degran(4) = depsgr*saa*caa*cba*cba*rac2     ! xy (Mandel-scaled)
    /// degran(5) = -depsgr*caa*sba*cba*rac2        ! xz
    /// degran(6) = -depsgr*saa*sba*cba*rac2        ! yz
    /// ```
    ///
    /// # The discrepancy, reproduced deliberately and not fixed
    ///
    /// Growth is uniaxial, so the tensor should be the rank-one dyad
    /// `Δε_g · n ⊗ n`. Components xx, zz, xy, xz and yz all match that dyad
    /// exactly for `n = (cos α cos β, sin α cos β, -sin β)`. Component **yy does
    /// not**: upstream writes `sin²α · sin²β` where the dyad requires
    /// `sin²α · cos²β`.
    ///
    /// The consequence is not subtle. The trace of a uniaxial growth tensor
    /// must be `Δε_g`, because a unit dyad has unit trace; upstream's trace is
    /// `Δε_g·(cos²α cos²β + sin²α sin²β + sin²β)`. At `α = π/2, β = 0` — growth
    /// along the y axis, a perfectly ordinary orientation — upstream's tensor is
    /// **identically zero** and the growth silently disappears.
    ///
    /// Per the workspace rule, this port reproduces upstream rather than
    /// quietly correcting it. [`strain_increment_rank_one`](Self::strain_increment_rank_one)
    /// gives the dyad the algebra implies, and a test in this module measures
    /// the disagreement so the maintainer can decide which one OFFBEAT should
    /// use.
    ///
    /// # Arguments
    ///
    /// - `growth_strain_increment` — `Δε_g` \[-\], the scalar growth strain
    ///   accumulated over the step. Upstream obtains it from a user-tabulated
    ///   `GRAN_FO(TEMP, IRRA)` function **in percent**, differences it across
    ///   the step, divides by 100 and clamps at zero (`granac.F90`); this port
    ///   takes the already-converted, already-clamped dimensionless value so
    ///   that no tabulated-function machinery is needed here.
    #[must_use]
    pub fn strain_increment(self, growth_strain_increment: f64) -> SymmTensor {
        let (sa, ca) = self.azimuth.sin_cos();
        let (sb, cb) = self.elevation.sin_cos();
        let g = growth_strain_increment;
        // Upstream stores components 4-6 with a sqrt(2) Mandel factor; this
        // port stores plain tensor components, so the sqrt(2) is dropped and
        // the shear entries are the tensor components themselves.
        SymmTensor::new(
            g * ca * ca * cb * cb,
            g * sa * ca * cb * cb,
            -g * ca * sb * cb,
            g * sa * sa * sb * sb,
            -g * sa * sb * cb,
            g * sb * sb,
        )
    }

    /// Growth strain increment as the rank-one dyad `Δε_g · n ⊗ n`.
    ///
    /// This is what a uniaxial stress-free eigenstrain along
    /// [`unit_vector`](Self::unit_vector) must be, and it differs from
    /// [`strain_increment`](Self::strain_increment) only in the yy component.
    /// Provided so the discrepancy documented there is testable and so a caller
    /// who wants the algebraically consistent tensor can have it explicitly
    /// rather than by accident.
    ///
    /// `growth_strain_increment` is `Δε_g` \[-\], as in
    /// [`strain_increment`](Self::strain_increment).
    #[must_use]
    pub fn strain_increment_rank_one(self, growth_strain_increment: f64) -> SymmTensor {
        scale_tensor(
            SymmTensor::from_outer(self.unit_vector()),
            growth_strain_increment,
        )
    }
}

/// A logarithmic irradiation law — fluence-driven creep, optionally with
/// irradiation growth.
///
/// Enum dispatch rather than trait objects, per the workspace rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LogarithmicIrradiationLaw {
    /// Fluence-driven creep alone.
    ///
    /// ASTER behaviour name: `VISC_IRRA_LOG` (`num_lc = 28`, 2 state variables
    /// `EPSPEQ`, `IRVECU`). Upstream: `bibfor/comport/nmvpir.F90` reached
    /// through `bibfor/lc/lc0028.F90` — legacy symbols `nmvpir`, `lc0028`.
    /// Integration: `ANALYTIQUE`, and this port keeps it closed-form.
    ///
    /// Intended by upstream for the *axial* creep of fuel assembly structures.
    Creep(LogarithmicIrradiationParameters),

    /// Fluence-driven creep plus irradiation growth.
    ///
    /// ASTER behaviour name: `GRAN_IRRA_LOG` (`num_lc = 28`, 3 state variables
    /// `EPSPEQ`, `IRVECU`, `EPSGRD`). Same upstream driver as
    /// [`Creep`](Self::Creep); the only difference is the extra stress-free
    /// growth eigenstrain, which is subtracted from the strain increment before
    /// the elastic predictor.
    CreepAndGrowth {
        /// The creep parameters — identical in form to
        /// [`Creep`](Self::Creep).
        creep: LogarithmicIrradiationParameters,
        /// The direction growth acts along.
        growth: IrradiationGrowthDirection,
    },
}

impl LogarithmicIrradiationLaw {
    /// The upstream ASTER behaviour name.
    #[must_use]
    pub const fn aster_name(self) -> &'static str {
        match self {
            Self::Creep(_) => "VISC_IRRA_LOG",
            Self::CreepAndGrowth { .. } => "GRAN_IRRA_LOG",
        }
    }

    /// The creep parameters, whichever variant this is.
    #[must_use]
    pub const fn creep_parameters(self) -> LogarithmicIrradiationParameters {
        match self {
            Self::Creep(p) => p,
            Self::CreepAndGrowth { creep, .. } => creep,
        }
    }

    /// The stress-free growth strain increment for this step, or zero for
    /// [`Creep`](Self::Creep).
    ///
    /// **Call this first and subtract the result from the total strain
    /// increment before forming the trial stress**, exactly as upstream does
    /// (`depsth(k) = deps(k) - epsthe - ... - degran(k)`). Growth does not
    /// depend on stress, so it can be resolved before the mechanical solve;
    /// [`integrate`](Self::integrate) deliberately does *not* apply it, because
    /// doing so would mean silently guessing which elastic moduli the caller
    /// used.
    ///
    /// `growth_strain_increment` is `Δε_g` \[-\] — see
    /// [`IrradiationGrowthDirection::strain_increment`] for where upstream gets
    /// it and for the defect this reproduces.
    #[must_use]
    pub fn growth_strain_increment(self, growth_strain_increment: f64) -> SymmTensor {
        match self {
            Self::Creep(_) => SymmTensor::ZERO,
            Self::CreepAndGrowth { growth, .. } => growth.strain_increment(growth_strain_increment),
        }
    }

    /// Integrate one step in closed form, returning the creep increment.
    ///
    /// # Why there is no iteration
    ///
    /// The rate is linear in stress, so the step equation closes algebraically.
    /// Writing `C` for the compliance from
    /// [`creep_compliance`](LogarithmicIrradiationParameters::creep_compliance)
    /// and `μ` for the shear modulus, the two statements
    ///
    /// - `Δp = C · σ_eq` (the flow rule, at the end-of-step stress), and
    /// - `σ_eq = σ_eq_trial - 3μ Δp` (the elastic return)
    ///
    /// combine to `σ_eq = σ_eq_trial / (1 + 3μC)` with no unknown left. That
    /// factor is upstream's `coef1`, and the deviator is simply scaled by it —
    /// which is why upstream declares `algo_inte = ANALYTIQUE` and why the
    /// returned [`CreepIncrement::iterations`] is always zero.
    ///
    /// # Arguments
    ///
    /// - `trial_stress` — the elastic-predictor stress \[Pa\], full tensor,
    ///   with the growth and thermal eigenstrains already removed.
    /// - `shear_modulus` — `μ` \[Pa\], strictly positive.
    /// - `fluence_start` — `Φ⁻` \[n/m²\]. Upstream keeps this as the internal
    ///   variable `IRVECU` rather than reading the external field directly, so
    ///   that a restart or a re-meshed fluence field cannot make the law jump;
    ///   pass the same internal counter here.
    /// - `fluence_increment` — `ΔΦ` \[n/m²\], non-negative.
    /// - `temperature` — `T` \[K\], strictly positive, end-of-step.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a non-positive shear modulus, a
    /// negative fluence or fluence increment, or a non-positive temperature.
    pub fn integrate(
        self,
        trial_stress: SymmTensor,
        shear_modulus: f64,
        fluence_start: f64,
        fluence_increment: f64,
        temperature: f64,
    ) -> Result<CreepIncrement> {
        check_shear_modulus(shear_modulus)?;
        let compliance = self.creep_parameters().creep_compliance(
            fluence_start,
            fluence_increment,
            temperature,
        )?;

        let s_trial = deviator(trial_stress);
        let eq_trial = von_mises_of_deviator(s_trial);
        let mean = trial_stress.tr() / 3.0;

        if !(eq_trial > 0.0) || compliance == 0.0 {
            return Ok(CreepIncrement {
                equivalent_increment: 0.0,
                strain_increment: SymmTensor::ZERO,
                stress: trial_stress,
                equivalent_stress: eq_trial,
                iterations: 0,
            });
        }

        let coef = 1.0 / (1.0 + 3.0 * shear_modulus * compliance);
        let eq = coef * eq_trial;
        let dp = compliance * eq;

        let s_new = scale_tensor(s_trial, coef);
        Ok(CreepIncrement {
            equivalent_increment: dp,
            // De = (3/2) Dp s / sigma_eq, in the (unrotated) trial direction.
            strain_increment: scale_tensor(s_trial, 1.5 * dp / eq_trial),
            stress: from_deviator_and_mean(s_new, mean),
            equivalent_stress: eq,
            iterations: 0,
        })
    }
}

// ===========================================================================
// 2. IRRAD3M — irradiation plasticity, creep and swelling of 304/316 steel
// ===========================================================================

/// Material parameters of `IRRAD3M`.
///
/// # What the law is for
///
/// The austenitic stainless internals of a PWR vessel — baffles, formers, the
/// bolts that hold them — sit in the highest neutron flux of any structural
/// component in the plant. Over decades they harden and embrittle, creep under
/// the bolt preload, and **swell**: voids nucleate and grow, and the steel gains
/// volume. `IRRAD3M` is EDF's model for that combination, and the three
/// mechanisms are genuinely coupled, because swelling changes the load which
/// changes the creep.
///
/// # Units
///
/// Dose in **dpa** throughout (see the module documentation). `ZETA_F` and
/// `ZETA_G` are dimensionless multipliers that default to 1 upstream and exist
/// to let a user scale the creep and swelling terms without re-fitting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Irrad3mParameters {
    /// Conventional yield strength `R_p0.2` \[Pa\]. Upstream `R02`.
    ///
    /// The 0.2 %-offset proof stress. Upstream pins the hardening curve to pass
    /// through it at the fixed plastic strain `p_e = 2e-3`.
    pub yield_strength: f64,
    /// Uniform elongation `ε_u` \[-\]. Upstream `EPSI_U`.
    ///
    /// The *true* plastic strain at the onset of necking, i.e. at the tensile
    /// maximum. Typically 0.2-0.4 for unirradiated austenitic steel and far
    /// smaller once irradiated.
    pub uniform_elongation: f64,
    /// Ultimate tensile strength `R_m` \[Pa\]. Upstream `RM`.
    ///
    /// The *engineering* UTS. Upstream converts it to a true stress with the
    /// standard `R_m·exp(ε_u)`, which is where the `exp` in the identification
    /// comes from.
    pub ultimate_strength: f64,
    /// Irradiation-creep compliance `A_i0` \[1/(Pa·dpa)\]. Upstream `AI0`.
    ///
    /// Converts accumulated stress-dose above the threshold into creep strain.
    pub creep_compliance: f64,
    /// Irradiation-creep incubation threshold `η_s` \[Pa·dpa\]. Upstream
    /// `ETAI_S`.
    ///
    /// Creep does not start until the accumulated stress-dose `η = ∫σ_eq dΦ`
    /// exceeds this. A threshold in `σ·Φ` rather than in `Φ` alone means a
    /// lightly loaded component may never start creeping at all — which is the
    /// point of modelling it.
    pub creep_threshold: f64,
    /// Saturated volumetric swelling rate `R_g0` \[1/dpa\]. Upstream `RG0`.
    ///
    /// The steady swelling rate reached once the incubation dose is passed. It
    /// is a **volumetric** rate; upstream divides by three to obtain the linear
    /// strain, and so does this port.
    pub swelling_rate: f64,
    /// Swelling transition sharpness `α` \[1/dpa\]. Upstream `ALPHA`.
    ///
    /// Controls how abruptly swelling switches on around
    /// [`swelling_onset_dose`](Self::swelling_onset_dose). Zero disables
    /// swelling entirely (upstream's `alpha > 0` guard).
    pub swelling_sharpness: f64,
    /// Swelling incubation dose `Φ₀` \[dpa\]. Upstream `PHI0`.
    ///
    /// The dose at which the logistic swelling rate reaches half its saturated
    /// value.
    pub swelling_onset_dose: f64,
    /// Post-irradiation softening factor `κ` \[-\]. Upstream `KAPPA`.
    ///
    /// Sets the initial plateau of the flow curve at `κ·R_p0.2`, below which
    /// the material flows at constant stress. Values below one represent an
    /// irradiated microstructure that yields locally before the bulk proof
    /// stress is reached.
    pub yield_plateau_factor: f64,
    /// Irradiation-creep scale factor `ζ_f` \[-\]. Upstream `ZETA_F`, default 1.
    pub creep_scale: f64,
    /// Swelling scale factor `ζ_g` \[-\]. Upstream `ZETA_G`, default 1.
    pub swelling_scale: f64,
}

/// The plastic strain at which upstream anchors the proof stress, `p_e = 2e-3`.
///
/// Hard-coded in `irrmat.F90` as `data pe/2.0d-3/` and **not** a user
/// parameter, despite the name `R02` implying 0.2 % — which is exactly this
/// value.
pub const IRRAD3M_PROOF_STRAIN: f64 = 2.0e-3;

/// The identified hardening curve of an [`Irrad3mParameters`] set.
///
/// # Why identification is needed at all
///
/// The user supplies three *tensile-test* numbers — proof stress, UTS, uniform
/// elongation — and the law needs a *flow curve* `σ_y(p)`. Upstream builds a
/// three-segment curve and fixes its free parameters by requiring it to pass
/// through both measured points:
///
/// - `σ_y(p_e) = R_p0.2` at `p_e = 2e-3`, and
/// - `σ_y(ε_u) = R_m · exp(ε_u)` — the true stress at necking.
///
/// With the power-law form `σ_y = K(p + p₀)^n` and the substitution
/// `p₀ = n - ε_u`, the second condition gives `K = R_m e^{ε_u} / n^n` outright
/// and the first collapses to the scalar equation
///
/// `1 - (R_m e^{ε_u}/R_p0.2) · (n - n₀)^n / n^n = 0`,  `n₀ = ε_u - p_e`
///
/// which upstream solves by dichotomy. That equation is the whole of the
/// identification, and it is fully checkable — see the module tests.
///
/// # The three segments
///
/// | Range | Flow stress | Meaning |
/// |---|---|---|
/// | `p < p_k` | `κ·R_p0.2` | a constant plateau — irradiated material flowing before bulk yield |
/// | `p_k ≤ p < p_e` | `a·(p - p_e) + σ(p_e)` | a straight line joining the plateau to the power law |
/// | `p ≥ p_e` | `K(p + p₀)^n` | the identified power law |
///
/// The line's slope `a` is the power law's own slope at `p_e`, so the curve is
/// `C¹` there and merely continuous at `p_k`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Irrad3mHardening {
    /// Power-law coefficient `K` \[Pa\]. Upstream `materf(7,2)`.
    pub coefficient: f64,
    /// Power-law exponent `n` \[-\]. Upstream `materf(8,2)`.
    pub exponent: f64,
    /// Power-law strain offset `p₀` \[-\]. Upstream `materf(9,2)`.
    pub strain_offset: f64,
    /// Flow stress at `p_e`, `σ(p_e)` \[Pa\]. Upstream `materf(16,2)` (`spe`).
    pub stress_at_proof_strain: f64,
    /// Slope of the power law at `p_e` \[Pa\]. Upstream `materf(13,2)`
    /// (`penpe`).
    pub slope_at_proof_strain: f64,
    /// Plastic strain at which the plateau ends, `p_k` \[-\]. Upstream
    /// `materf(14,2)` (`pk`).
    pub plateau_strain: f64,
    /// The plateau flow stress `κ·R_p0.2` \[Pa\].
    pub plateau_stress: f64,
    /// `true` if the identification fell back to upstream's default branch
    /// because the scalar equation had no root.
    ///
    /// Upstream then sets `n = ε_u`, `p₀ = 0` and `K = R_m e^{ε_u}/ε_u^{ε_u}`,
    /// which satisfies the UTS condition but **not** the proof-stress one — the
    /// identified curve no longer passes through `R_p0.2`. Surfaced here rather
    /// than hidden, because a silently mis-identified flow curve is exactly the
    /// kind of plausible-looking wrong answer this port is meant to avoid.
    pub used_fallback: bool,
}

impl Irrad3mHardening {
    /// Flow stress `σ_y(p)` \[Pa\] at accumulated plastic strain `p` \[-\].
    ///
    /// Non-decreasing on `p ≥ 0` for physically ordered parameters. Evaluating
    /// below zero is meaningless and returns the plateau.
    #[must_use]
    pub fn flow_stress(self, p: f64) -> f64 {
        if p < self.plateau_strain {
            self.plateau_stress
        } else if p < IRRAD3M_PROOF_STRAIN {
            self.slope_at_proof_strain * (p - IRRAD3M_PROOF_STRAIN) + self.stress_at_proof_strain
        } else {
            self.coefficient * (p + self.strain_offset).powf(self.exponent)
        }
    }

    /// The plastic strain at which the flow stress first reaches `sigma`
    /// \[Pa\] — the inverse of [`flow_stress`](Self::flow_stress).
    ///
    /// Returns zero for any stress at or below the plateau, since the curve is
    /// flat there and the inverse is not unique; that is the correct choice for
    /// a return map, where "no additional plastic strain is required" is the
    /// answer wanted.
    #[must_use]
    pub fn strain_at_flow_stress(self, sigma: f64) -> f64 {
        if sigma <= self.plateau_stress {
            0.0
        } else if sigma < self.stress_at_proof_strain && self.slope_at_proof_strain > 0.0 {
            IRRAD3M_PROOF_STRAIN
                + (sigma - self.stress_at_proof_strain) / self.slope_at_proof_strain
        } else if self.coefficient > 0.0 && self.exponent > 0.0 {
            (sigma / self.coefficient).powf(1.0 / self.exponent) - self.strain_offset
        } else {
            IRRAD3M_PROOF_STRAIN
        }
    }
}

impl Irrad3mParameters {
    /// Identify the three-segment hardening curve from the tensile data.
    ///
    /// Reproduces `irrmat.F90`'s construction. Upstream finds the exponent with
    /// a hand-rolled halving dichotomy; this port brackets the same root and
    /// hands it to [`brent`], which is the shared
    /// machinery the workspace already has and converges superlinearly on the
    /// same equation. The root is identical — only the path to it differs.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a non-positive proof stress, a
    /// non-positive UTS, or a non-positive uniform elongation.
    /// [`OffbeatError::ConstitutiveNotConverged`] if the bracketed root find
    /// fails.
    pub fn identify_hardening(self) -> Result<Irrad3mHardening> {
        if !(self.yield_strength > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "IRRAD3M proof stress R02",
                value: self.yield_strength,
                unit: "Pa",
                reason: "must be strictly positive; the identification \
                         normalises by it",
            });
        }
        if !(self.ultimate_strength > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "IRRAD3M ultimate strength RM",
                value: self.ultimate_strength,
                unit: "Pa",
                reason: "must be strictly positive",
            });
        }
        if !(self.uniform_elongation > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "IRRAD3M uniform elongation EPSI_U",
                value: self.uniform_elongation,
                unit: "-",
                reason: "must be strictly positive; it is the true plastic \
                         strain at necking",
            });
        }

        let pe = IRRAD3M_PROOF_STRAIN;
        let eu = self.uniform_elongation;
        let true_uts = self.ultimate_strength * eu.exp();
        let coeffa = true_uts / self.yield_strength;
        let n0 = eu - pe;

        // f(n) = 1 - coeffa*((n-n0)^n)/(n^n). Monotone decreasing on the
        // admissible branch, with limit 1 - coeffa*exp(-n0) as n -> infinity.
        let f = |n: f64| 1.0 - coeffa * ((n - n0).powf(n)) / (n.powf(n));
        let f_infinity = 1.0 - coeffa * (-n0).exp();

        let lower = if n0 >= 0.0 {
            n0 + pe / 1000.0
        } else {
            pe / 1000.0
        };
        let f_lower = f(lower);

        // Upstream's own no-root test, transcribed. `n0 == 0` is excluded
        // because the equation degenerates there.
        let no_root = f_lower * f_infinity > 0.0 || n0 == 0.0;

        let (exponent, strain_offset, used_fallback) = if no_root {
            (eu, 0.0, true)
        } else {
            // Grow an upper bound until the sign flips. f tends to f_infinity
            // < 0 monotonically, so this terminates; the cap is a guard, not an
            // expected exit.
            let mut upper = lower.max(1.0e-6);
            let mut found = false;
            for _ in 0..80 {
                upper *= 4.0;
                if f(upper) < 0.0 {
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(OffbeatError::ConstitutiveNotConverged {
                    cell: usize::MAX,
                    residual: f(upper).abs(),
                    iterations: 80,
                });
            }
            let solution = brent(
                f,
                (lower, upper),
                &SolverControl {
                    max_iter: 200,
                    residual_tol: 1.0e-13,
                    step_tol: 1.0e-16,
                },
            )?;
            (solution.root, solution.root - eu, false)
        };

        let (coefficient, stress_at_proof_strain, slope) = if exponent > 0.0 {
            let k = true_uts / exponent.powf(exponent);
            let base = pe + strain_offset;
            (
                k,
                k * base.powf(exponent),
                exponent * k * base.powf(exponent - 1.0),
            )
        } else {
            // Upstream's degenerate branch: n <= 0 gives a constant curve.
            (self.ultimate_strength, self.ultimate_strength, 0.0)
        };

        let plateau_stress = self.yield_plateau_factor * self.yield_strength;
        let (slope_at_proof_strain, plateau_strain) = if slope > 0.0 {
            (
                slope,
                pe - (stress_at_proof_strain - plateau_stress) / slope,
            )
        } else {
            (0.0, 0.0)
        };

        Ok(Irrad3mHardening {
            coefficient,
            exponent,
            strain_offset,
            stress_at_proof_strain,
            slope_at_proof_strain,
            plateau_strain,
            plateau_stress,
            used_fallback,
        })
    }

    /// Accumulated **linear** swelling strain at dose `dose` \[dpa\].
    ///
    /// # Closed form, and why it is exact
    ///
    /// The swelling rate per unit dose is logistic,
    ///
    /// `dε_g/dΦ = R_g0 / (3·(1 + exp(α(Φ₀ - Φ))))`
    ///
    /// — a saturating switch that turns on around the incubation dose `Φ₀` with
    /// sharpness `α`, and the factor three converts the volumetric rate `R_g0`
    /// into a linear strain in each direction. That integrates analytically:
    ///
    /// `ε_g(Φ) = R_g0 · ln((e^{αΦ₀} + e^{αΦ}) / (1 + e^{αΦ₀})) / (3α)`
    ///
    /// and upstream evaluates exactly this (`irrmat.F90`, `materd(19,2)`) rather
    /// than integrating numerically — so the swelling increment carries **no**
    /// discretisation error at all, unlike the creep terms. Note `ε_g(0) = 0`,
    /// which the closed form gives identically.
    ///
    /// Returns zero when `α ≤ 0`, matching upstream's guard: a zero sharpness
    /// would make the logistic degenerate and upstream treats it as "swelling
    /// disabled".
    ///
    /// The result is a **linear** strain \[-\]; the volumetric swelling is three
    /// times it.
    #[must_use]
    pub fn swelling_strain(self, dose: f64) -> f64 {
        let alpha = self.swelling_sharpness;
        if !(alpha > 0.0) {
            return 0.0;
        }
        let e0 = (alpha * self.swelling_onset_dose).exp();
        let ed = (alpha * dose).exp();
        self.swelling_rate * ((e0 + ed) / (1.0 + e0)).ln() / (3.0 * alpha)
    }

    /// Stress-free swelling strain increment tensor over one step.
    ///
    /// Purely volumetric — `Δε_g · I` — so it does not touch the stress
    /// deviator and therefore does not interact with the return map at all.
    /// **Subtract it from the total strain increment before forming the trial
    /// stress**, exactly as with thermal expansion; [`Irrad3m::integrate`]
    /// deliberately does not apply it, for the same reason
    /// [`LogarithmicIrradiationLaw::integrate`] does not apply growth.
    ///
    /// Upstream applies the scale factor `ζ_g` here and suppresses swelling
    /// entirely on a step with no dose increment (`if (dphi > 0)`), both of
    /// which are reproduced.
    ///
    /// - `dose_start`, `dose_end` — \[dpa\]. A non-increasing dose gives zero.
    #[must_use]
    pub fn swelling_strain_increment(self, dose_start: f64, dose_end: f64) -> SymmTensor {
        if !(dose_end > dose_start) {
            return SymmTensor::ZERO;
        }
        let dg = self.swelling_scale
            * (self.swelling_strain(dose_end) - self.swelling_strain(dose_start));
        SymmTensor::from_diag(dg, dg, dg)
    }
}

/// The internal state `IRRAD3M` carries between steps.
///
/// Upstream stores seven internal variables (`EPSPEQ`, `SEUIL`, `EPEQIRRA`,
/// `GONF`, `INDIPLAS`, `IRRA`, `TEMP`); the four that actually *evolve* and are
/// read back by the residuals are gathered here. The remaining three are either
/// diagnostics or copies of command variables the caller already has.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Irrad3mState {
    /// Accumulated equivalent **plastic** strain `p` \[-\]. Upstream `EPSPEQ`.
    pub plastic_strain: f64,
    /// Accumulated stress-dose `η = ∫ σ_eq dΦ` \[Pa·dpa\]. Upstream `SEUIL`.
    ///
    /// The incubation variable for irradiation creep; creep begins once it
    /// passes [`Irrad3mParameters::creep_threshold`].
    pub creep_driver: f64,
    /// Accumulated equivalent **irradiation-creep** strain `p_i` \[-\].
    /// Upstream `EPEQIRRA`.
    ///
    /// Tracked separately from the plastic strain because only the plastic part
    /// hardens the material — irradiation creep does not move the flow curve.
    pub irradiation_creep_strain: f64,
    /// Accumulated **linear** swelling strain \[-\]. Upstream `GONF`.
    pub swelling_strain: f64,
}

impl Irrad3mState {
    /// The state at the end of a step, given what the step produced.
    ///
    /// Advancing the four variables by hand is easy to get subtly wrong — the
    /// plastic and irradiation-creep strains accumulate separately, and only
    /// the *plastic* one hardens the material — so the arithmetic is offered
    /// here rather than left to every caller.
    ///
    /// `swelling_increment` is the **linear** swelling strain `Δε_g` \[-\] for
    /// the step, i.e. any one diagonal component of
    /// [`Irrad3mParameters::swelling_strain_increment`]. It is passed in rather
    /// than recomputed because swelling is resolved *before* the mechanical
    /// step, not during it.
    #[must_use]
    pub fn advanced(self, increment: &Irrad3mIncrement, swelling_increment: f64) -> Self {
        Self {
            plastic_strain: self.plastic_strain + increment.plastic_increment,
            creep_driver: self.creep_driver + increment.creep_driver_increment,
            irradiation_creep_strain: self.irradiation_creep_strain
                + increment.irradiation_creep_increment,
            swelling_strain: self.swelling_strain + swelling_increment,
        }
    }
}

/// The `IRRAD3M` law: parameters plus the hardening curve identified from them.
///
/// ASTER behaviour name: `IRRAD3M` (`num_lc = 30`, 7 state variables).
/// Upstream: `bibfor/algorith/irrmat.F90` (material preparation),
/// `bibfor/algorith/irrres.F90` (local residuals), reached through
/// `bibfor/lc/lc0030.F90` and the generic `plasti` driver — legacy symbols
/// `irrmat`, `irrres`, `lc0030`. Integration: `NEWTON` upstream.
///
/// Constructed with [`new`](Self::new) so the identification happens once, not
/// once per integration point per step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Irrad3m {
    /// The user's material parameters.
    pub parameters: Irrad3mParameters,
    /// The hardening curve identified from them.
    pub hardening: Irrad3mHardening,
}

/// What one `IRRAD3M` step produced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Irrad3mIncrement {
    /// Equivalent **plastic** strain increment `Δp` \[-\], non-negative.
    pub plastic_increment: f64,
    /// Equivalent **irradiation-creep** strain increment `Δp_i` \[-\],
    /// non-negative.
    pub irradiation_creep_increment: f64,
    /// Increment of the stress-dose driver `Δη` \[Pa·dpa\].
    pub creep_driver_increment: f64,
    /// Combined inelastic strain increment tensor \[-\], `(Δp + Δp_i)·(3/2)s/σ_eq`.
    ///
    /// Deviatoric: neither plastic flow nor irradiation creep changes volume.
    /// Swelling is *not* included — it is a separate, stress-free eigenstrain
    /// obtained from
    /// [`Irrad3mParameters::swelling_strain_increment`].
    pub strain_increment: SymmTensor,
    /// Stress at the end of the step \[Pa\].
    pub stress: SymmTensor,
    /// Von Mises equivalent of [`stress`](Self::stress) \[Pa\].
    pub equivalent_stress: f64,
    /// Local-solver iterations used. Zero when the step was purely elastic.
    pub iterations: usize,
}

impl Irrad3m {
    /// Build the law, identifying the hardening curve once.
    ///
    /// # Errors
    ///
    /// As [`Irrad3mParameters::identify_hardening`].
    pub fn new(parameters: Irrad3mParameters) -> Result<Self> {
        let hardening = parameters.identify_hardening()?;
        Ok(Self {
            parameters,
            hardening,
        })
    }

    /// The upstream ASTER behaviour name.
    #[must_use]
    pub const fn aster_name(self) -> &'static str {
        "IRRAD3M"
    }

    /// Irradiation-creep driver increment and creep increment for a candidate
    /// end-of-step equivalent stress.
    ///
    /// Transcribes the branch structure of `irrres.F90`'s `RESIDU EN
    /// DEFORMATION D IRRADIATION` block. Exposed publicly because the branch a
    /// step lands in is the single most consequential thing about an `IRRAD3M`
    /// result — creep either runs or does not — and a caller diagnosing a
    /// component that stubbornly refuses to creep needs to see which branch
    /// fired without re-deriving it.
    ///
    /// # The three regimes
    ///
    /// 1. **Already past the threshold** (`η⁻ > η_s`): the whole stress-dose
    ///    increment produces creep, `Δp_i = A_i0 Δη`.
    /// 2. **Still below it** (`η⁺ ≤ η_s`): the driver accumulates, no creep.
    /// 3. **Crossing it this step**: only the part of `η` above the threshold
    ///    counts, `Δp_i = A_i0 (η⁺ - η_s)`, and upstream additionally corrects
    ///    the driver increment itself by
    ///    `-(σ⁺ - σ⁻)(η_s - η⁻)/(2σ⁻)`. That correction is reproduced verbatim;
    ///    see the module tests, which document that it makes `Δη` depend on the
    ///    threshold, so the driver is not a pure stress-dose integral on a
    ///    crossing step.
    ///
    /// The driver increment uses the **trapezoidal** rule in stress,
    /// `Δη = ζ_f (σ⁻ + σ⁺)ΔΦ/2` — second-order in the dose step, unlike the
    /// right-endpoint rule [`LogarithmicIrradiationLaw`] inherits.
    ///
    /// # Arguments
    ///
    /// - `equivalent_stress_start` — `σ_eq⁻` \[Pa\], start of step.
    /// - `equivalent_stress_end` — `σ_eq⁺` \[Pa\], the candidate end value.
    /// - `driver_start` — `η⁻` \[Pa·dpa\].
    /// - `dose_increment` — `ΔΦ` \[dpa\], non-negative.
    ///
    /// Returns `(Δη, Δp_i)` in \[Pa·dpa\] and \[-\].
    #[must_use]
    pub fn irradiation_creep_increment(
        self,
        equivalent_stress_start: f64,
        equivalent_stress_end: f64,
        driver_start: f64,
        dose_increment: f64,
    ) -> (f64, f64) {
        let p = self.parameters;
        let trapezoid = (equivalent_stress_start + equivalent_stress_end) * dose_increment * 0.5;

        if driver_start > p.creep_threshold {
            let d_eta = p.creep_scale * trapezoid;
            return (d_eta, p.creep_compliance * d_eta);
        }

        let provisional_end = driver_start + p.creep_scale * trapezoid;
        if provisional_end <= p.creep_threshold {
            return (p.creep_scale * trapezoid, 0.0);
        }
        if !(p.creep_scale * trapezoid > 0.0) {
            return (0.0, 0.0);
        }

        // Threshold crossed during this step.
        let mut aux = trapezoid;
        if equivalent_stress_start > 0.0 {
            aux -= (equivalent_stress_end - equivalent_stress_start)
                * (p.creep_threshold - driver_start)
                / (2.0 * equivalent_stress_start);
        }
        let d_eta = p.creep_scale * aux;
        let dpi = p.creep_compliance * (driver_start + d_eta - p.creep_threshold);
        (d_eta, dpi.max(0.0))
    }

    /// Integrate one step: plasticity and irradiation creep together.
    ///
    /// # Reduction to one scalar
    ///
    /// Both mechanisms flow in the same direction — the normal to the von Mises
    /// surface, `(3/2)s/σ_eq` — and swelling is purely volumetric, so the
    /// deviator shrinks without rotating and the whole coupled system collapses
    /// to one scalar unknown. Taking the end-of-step equivalent stress `x` as
    /// that unknown:
    ///
    /// - `Δp(x) = max(0, p_y⁻¹(x) - p⁻)` from the plastic consistency
    ///   condition, with `p_y⁻¹` the inverse flow curve;
    /// - `Δp_i(x)` from
    ///   [`irradiation_creep_increment`](Self::irradiation_creep_increment);
    /// - `x = σ_eq_trial - 3μ(Δp + Δp_i)` from the elastic return.
    ///
    /// The residual is monotone increasing in `x` and brackets on
    /// `[0, σ_eq_trial]` by construction, so
    /// [`brent`] — derivative-free, and immune to
    /// the two slope discontinuities the three-segment flow curve puts in the
    /// residual — is used rather than Newton. That is a deliberate departure
    /// from upstream's declared `NEWTON`: upstream iterates on the full
    /// `ndt + 4` system with a consistent Jacobian, which is not the same
    /// problem once it is reduced to a scalar.
    ///
    /// # Arguments
    ///
    /// - `trial_stress` — elastic-predictor stress \[Pa\], with the thermal
    ///   *and* swelling eigenstrains already removed.
    /// - `shear_modulus` — `μ` \[Pa\], strictly positive.
    /// - `state` — internal variables at the start of the step.
    /// - `equivalent_stress_start` — `σ_eq⁻` \[Pa\], the von Mises equivalent of
    ///   the converged stress at the start of the step. Needed because the
    ///   creep driver integrates stress trapezoidally over the step, so the
    ///   start value is not recoverable from the trial stress.
    /// - `dose_increment` — `ΔΦ` \[dpa\], non-negative.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a non-positive shear modulus or a
    /// negative dose increment; [`OffbeatError::ConstitutiveNotConverged`] if
    /// the local solve fails.
    pub fn integrate(
        self,
        trial_stress: SymmTensor,
        shear_modulus: f64,
        state: Irrad3mState,
        equivalent_stress_start: f64,
        dose_increment: f64,
    ) -> Result<Irrad3mIncrement> {
        check_shear_modulus(shear_modulus)?;
        if dose_increment < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "dose increment",
                value: dose_increment,
                unit: "dpa",
                reason: "must not be negative; irradiation damage accumulates",
            });
        }

        let s_trial = deviator(trial_stress);
        let eq_trial = von_mises_of_deviator(s_trial);
        let mean = trial_stress.tr() / 3.0;
        let three_mu = 3.0 * shear_modulus;

        let increments = |x: f64| {
            let dp = (self.hardening.strain_at_flow_stress(x) - state.plastic_strain).max(0.0);
            let (d_eta, dpi) = self.irradiation_creep_increment(
                equivalent_stress_start,
                x,
                state.creep_driver,
                dose_increment,
            );
            (dp, dpi, d_eta)
        };
        let residual = |x: f64| {
            let (dp, dpi, _) = increments(x);
            x - (eq_trial - three_mu * (dp + dpi))
        };

        let (x, iterations) = if !(eq_trial > 0.0) {
            (0.0, 0)
        } else if residual(0.0) >= 0.0 {
            // The step relaxes the deviator completely: creep alone would
            // over-shoot, so the end stress is pinned at zero.
            (0.0, 0)
        } else {
            let solution = brent(
                residual,
                (0.0, eq_trial),
                &SolverControl {
                    max_iter: 200,
                    residual_tol: 1.0e-12 * eq_trial.max(1.0),
                    step_tol: 1.0e-16 * eq_trial.max(1.0),
                },
            )?;
            (solution.root.clamp(0.0, eq_trial), solution.iterations)
        };

        let (dp, dpi, d_eta) = increments(x);
        let scale = if eq_trial > 0.0 { x / eq_trial } else { 0.0 };
        let strain_increment = if eq_trial > 0.0 {
            scale_tensor(s_trial, 1.5 * (dp + dpi) / eq_trial)
        } else {
            SymmTensor::ZERO
        };

        Ok(Irrad3mIncrement {
            plastic_increment: dp,
            irradiation_creep_increment: dpi,
            creep_driver_increment: d_eta,
            strain_increment,
            stress: from_deviator_and_mean(scale_tensor(s_trial, scale), mean),
            equivalent_stress: x,
            iterations,
        })
    }
}

// ===========================================================================
// 3. META_LEMA_ANI — anisotropic Lemaitre creep with metallurgical phases
// ===========================================================================

/// A Hill quadratic form — the anisotropic replacement for von Mises.
///
/// # Why anisotropy is not optional for cladding
///
/// Zircaloy tubing is drawn and pilgered, which leaves the hexagonal grains
/// strongly textured: the basal poles point predominantly radially. The tube is
/// therefore *not* the same material in the hoop, axial and radial directions,
/// and a von Mises law — which assumes it is — gets the *direction* of creep
/// wrong even when it gets the magnitude right. For a cladding tube creeping
/// down onto a pellet under external coolant pressure, the direction is the
/// answer.
///
/// # The form
///
/// Hill 1948 replaces the von Mises equivalent with a general
/// pressure-insensitive quadratic,
///
/// `σ_H = sqrt(σ : M : σ)`
///
/// where `M` is a fourth-order tensor with the same symmetries and the same
/// null space (the hydrostatic direction) as `(3/2)P_dev`. Six independent
/// coefficients survive those constraints, and they are exactly the six
/// upstream tabulates.
///
/// # Coefficient meaning, and the von Mises check that pins it
///
/// Each field below is the corresponding **diagonal component of `M`** in the
/// material frame. Setting `M` to its isotropic value `(3/2)P_dev` gives
///
/// - normal components `M_xxxx = M_yyyy = M_zzzz = 3/2 · (1 - 1/3) = 1`
/// - shear components `M_xyxy = M_xzxz = M_yzyz = 3/2 · 1/2 = 3/4`
///
/// and [`VON_MISES`](Self::VON_MISES) carries exactly those numbers. That the
/// resulting `σ_H` reproduces `sqrt(3/2 s:s)` on a general stress state is the
/// check that fixes the convention beyond doubt, and it is a test in this
/// module rather than an assertion here.
///
/// # Expanded form
///
/// With `F = (M_xx + M_yy - M_zz)/2`, `G = (-M_xx + M_yy + M_zz)/2` and
/// `H = (M_xx - M_yy + M_zz)/2` — upstream's `H_F`, `H_G`, `H_H` — the
/// quadratic is
///
/// `σ_H² = F(σ_xx - σ_yy)² + G(σ_yy - σ_zz)² + H(σ_xx - σ_zz)² + 4(M_xy σ_xy² + M_xz σ_xz² + M_yz σ_yz²)`
///
/// which is manifestly zero on a hydrostatic stress, as a plastic-flow
/// potential for a metal must be.
///
/// # Units
///
/// All six coefficients are **dimensionless** ratios; `σ_H` carries the unit of
/// the stress passed in, i.e. pascal.
///
/// # The frame these are expressed in
///
/// The material frame, not the global one. For a cladding tube upstream names
/// the axes `(R, T, Z)` — radial, hoop, axial. This port takes the tensor in
/// whatever frame the caller works in and does not rotate; wiring the material
/// frame is the caller's job, exactly as `AFFE_CARA_ELEM/MASSIF` is upstream's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HillAnisotropy {
    /// `M_xxxx` \[-\] — upstream `M_RR_RR` for a tube.
    pub m_xx: f64,
    /// `M_yyyy` \[-\] — upstream `M_TT_TT` for a tube in 3-D.
    pub m_yy: f64,
    /// `M_zzzz` \[-\] — upstream `M_ZZ_ZZ` for a tube in 3-D.
    pub m_zz: f64,
    /// `M_xyxy` \[-\].
    ///
    /// Which upstream keyword lands here is not obvious — see
    /// [`from_aster_3d`](Self::from_aster_3d), which reproduces upstream's
    /// mapping and documents an apparent transposition in it.
    pub m_xy: f64,
    /// `M_xzxz` \[-\].
    pub m_xz: f64,
    /// `M_yzyz` \[-\].
    pub m_yz: f64,
}

impl HillAnisotropy {
    /// The isotropic case: `σ_H` reduces exactly to the von Mises equivalent.
    ///
    /// `(1, 1, 1, 3/4, 3/4, 3/4)` — the diagonal components of `(3/2)P_dev`.
    /// Use it as a sanity reference, and as the correct starting point for a
    /// texture-free material.
    pub const VON_MISES: Self = Self {
        m_xx: 1.0,
        m_yy: 1.0,
        m_zz: 1.0,
        m_xy: 0.75,
        m_xz: 0.75,
        m_yz: 0.75,
    };

    /// Build from upstream's six `META_LEMA_ANI` material keywords, using the
    /// **3-D** branch of `META_LEMA_ANI.mfront`.
    ///
    /// # Upstream mapping, transcribed
    ///
    /// ```text
    /// M1 = {F_MRR_RR, F_MTT_TT, F_MZZ_ZZ, F_MRZ_RZ, F_MRT_RT, F_MTZ_TZ};
    /// ```
    ///
    /// so the normal slots take `(RR, TT, ZZ) -> (xx, yy, zz)`, fixing the frame
    /// as `x = R`, `y = T`, `z = Z`.
    ///
    /// # An apparent transposition, reproduced and not fixed
    ///
    /// With `x = R, y = T, z = Z`, the `xy` shear slot *is* the `RT` shear, so
    /// it should receive `M_RT_RT`. Upstream gives it `M_RZ_RZ`, and gives the
    /// `xz` (= `RZ`) slot `M_RT_RT`. The two are exchanged. The same exchange
    /// appears in the axisymmetric branch, where the frame is `(R, Z, T)` and
    /// the single active shear slot — geometrically `RZ` — is given `M_RT_RT`.
    ///
    /// Because the exchange is consistent across both branches it may be a
    /// naming convention rather than a defect, and the `catalo/` directory that
    /// would settle it is not present in the read-only upstream clone this port
    /// was made from. So it is reproduced verbatim, flagged here, and pinned by
    /// a test. A caller who believes the geometric reading should build
    /// [`HillAnisotropy`] directly from the struct fields, which are named by
    /// tensor slot and carry no ambiguity at all.
    ///
    /// Arguments are the six upstream keywords, all dimensionless.
    #[must_use]
    pub const fn from_aster_3d(
        m_rr_rr: f64,
        m_tt_tt: f64,
        m_zz_zz: f64,
        m_rt_rt: f64,
        m_rz_rz: f64,
        m_tz_tz: f64,
    ) -> Self {
        Self {
            m_xx: m_rr_rr,
            m_yy: m_tt_tt,
            m_zz: m_zz_zz,
            // Upstream's slot order is {..., F_MRZ_RZ, F_MRT_RT, F_MTZ_TZ}.
            m_xy: m_rz_rz,
            m_xz: m_rt_rt,
            m_yz: m_tz_tz,
        }
    }

    /// The `(F, G, H)` triple of upstream's `H_F`, `H_G`, `H_H`.
    ///
    /// `F` weights `(σ_xx - σ_yy)²`, `G` weights `(σ_yy - σ_zz)²`, `H` weights
    /// `(σ_xx - σ_zz)²`. All dimensionless. Exposed because these, not the
    /// `M` components, are what the Hill literature tabulates.
    #[must_use]
    pub fn fgh(self) -> (f64, f64, f64) {
        (
            0.5 * (self.m_xx + self.m_yy - self.m_zz),
            0.5 * (-self.m_xx + self.m_yy + self.m_zz),
            0.5 * (self.m_xx - self.m_yy + self.m_zz),
        )
    }

    /// The contraction `M : σ` \[Pa\] — the gradient of `σ_H²/2`.
    ///
    /// Traceless for any `σ` and any coefficients, which is what makes the flow
    /// it generates volume-preserving. For [`VON_MISES`](Self::VON_MISES) it is
    /// exactly `(3/2)·dev(σ)`.
    #[must_use]
    pub fn contract(self, sigma: SymmTensor) -> SymmTensor {
        let (f, g, h) = self.fgh();
        let dxy = sigma.xx - sigma.yy;
        let dyz = sigma.yy - sigma.zz;
        let dxz = sigma.xx - sigma.zz;
        SymmTensor::new(
            f * dxy + h * dxz,
            2.0 * self.m_xy * sigma.xy,
            2.0 * self.m_xz * sigma.xz,
            -f * dxy + g * dyz,
            2.0 * self.m_yz * sigma.yz,
            -g * dyz - h * dxz,
        )
    }

    /// Hill equivalent stress `σ_H = sqrt(σ : M : σ)` \[Pa\].
    ///
    /// Clamped at zero before the square root, reproducing upstream's
    /// `sqrt(max(sig|(H*sig), 0.0))`. That clamp is not cosmetic: a
    /// user-supplied coefficient set need not make the quadratic form positive
    /// semi-definite, and upstream chose to return zero rather than a NaN.
    #[must_use]
    pub fn equivalent_stress(self, sigma: SymmTensor) -> f64 {
        sigma.double_inner(self.contract(sigma)).max(0.0).sqrt()
    }

    /// The flow direction `n = (M : σ)/σ_H` \[-\].
    ///
    /// The outward normal to the Hill surface, and the direction the creep
    /// strain increment points along. Returns the zero tensor at zero
    /// equivalent stress, where the normal is undefined.
    ///
    /// For [`VON_MISES`](Self::VON_MISES) this is `(3/2)s/σ_eq`, the radial
    /// return direction the isotropic laws in
    /// [`viscoplastic`](super::viscoplastic) use. **For anything else it is
    /// not parallel to the deviator**, which is the whole reason this law needs
    /// its own integrator.
    #[must_use]
    pub fn flow_direction(self, sigma: SymmTensor) -> SymmTensor {
        let eq = self.equivalent_stress(sigma);
        if !(eq > 0.0) {
            return SymmTensor::ZERO;
        }
        scale_tensor(self.contract(sigma), 1.0 / eq)
    }

    /// Linear blend `za·self + (1-za)·other`, coefficient by coefficient.
    ///
    /// Upstream mixes the α-phase and β-phase Hill matrices this way as the
    /// metallurgical fractions change. `za` is the α-phase fraction \[-\].
    #[must_use]
    pub fn blend(self, other: Self, za: f64) -> Self {
        let mix = |a: f64, b: f64| za * a + (1.0 - za) * b;
        Self {
            m_xx: mix(self.m_xx, other.m_xx),
            m_yy: mix(self.m_yy, other.m_yy),
            m_zz: mix(self.m_zz, other.m_zz),
            m_xy: mix(self.m_xy, other.m_xy),
            m_xz: mix(self.m_xz, other.m_xz),
            m_yz: mix(self.m_yz, other.m_yz),
        }
    }
}

/// Viscoplastic parameters for one metallurgical phase of `META_LEMA_ANI`.
///
/// # The flow rule these parametrise
///
/// Each phase contributes a viscous stress
///
/// `σ_v,i = γ_i · p^{m_i} · (ṗ)^{1/n_i}`,  `γ_i = a_i · exp(Q_i/(n_i·T))`
///
/// and the law is satisfied when `σ_H = Σ_i f_i σ_v,i` over the three phases.
/// Inverting a single phase gives `ṗ = (σ_H/(γ p^m))^{n}` — a Lemaitre law, with
/// `γ` in the role of the reference stress `K` and `m` the strain-hardening
/// exponent.
///
/// # Why `Q` is divided by `n`
///
/// Exactly as in
/// [`LemaitreIrradiation`](super::viscoplastic::ViscoplasticLaw::LemaitreIrradiation):
/// because the rate goes as `γ^{-n}`, the `1/n` cancels and the *rate* carries a
/// clean `exp(-Q/(R·T))`. Transcribing `γ` without it gives an Arrhenius
/// exponent `n` times too large — wrong by orders of magnitude, and invisible to
/// any dimensional check.
///
/// # Units
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetaLemaAniPhase {
    /// Reference-stress amplitude `a` \[Pa·s^{1/n}\]. Upstream `F1_A`, `F2_A`,
    /// `C_A`.
    ///
    /// The fractional-power time unit is not a typo: `γ` multiplies `ṗ^{1/n}`,
    /// so `a` must carry `s^{1/n}` for the product to be a stress.
    pub amplitude: f64,
    /// Strain-hardening exponent `m` \[-\]. Upstream `F1_M`, `F2_M`, `C_M`.
    ///
    /// Positive `m` raises the flow stress as strain accumulates, so the rate
    /// *falls* — primary creep. Note the sign convention is opposite to
    /// [`LemaitreParameters::m`](super::viscoplastic::LemaitreParameters::m),
    /// which enters as `p^{-n/m}`.
    pub hardening_exponent: f64,
    /// Stress exponent `n` \[-\]. Upstream `F1_N`, `F2_N`, `C_N`. Strictly
    /// positive.
    pub stress_exponent: f64,
    /// Activation temperature `Q/R` \[K\]. Upstream `F1_Q`, `F2_Q`, `C_Q`.
    pub activation_temperature: f64,
}

impl MetaLemaAniPhase {
    /// The reference stress `γ = a·exp(Q/(n·T))` \[Pa·s^{1/n}\] at temperature
    /// `temperature` \[K\].
    ///
    /// Increases as the material cools, which is what makes a cold phase
    /// stiffer against creep.
    #[must_use]
    pub fn reference_stress(self, temperature: f64) -> f64 {
        self.amplitude * (self.activation_temperature / (self.stress_exponent * temperature)).exp()
    }

    /// Viscous stress `σ_v = γ p^m ṗ^{1/n}` \[Pa\] of this phase alone.
    ///
    /// - `temperature` \[K\], strictly positive.
    /// - `accumulated_strain` — `p` \[-\], the end-of-step value (upstream runs
    ///   `θ = 1`).
    /// - `strain_rate` — `ṗ` \[1/s\], non-negative.
    ///
    /// Returns zero if either `p` or `ṗ` is non-positive, reproducing
    /// upstream's guards (`pm[i] = (p_ > 0.) ? ... : 0.` and the matching test
    /// on `dp`). Without them the law would evaluate `0^m` with a negative `m`.
    #[must_use]
    pub fn viscous_stress(
        self,
        temperature: f64,
        accumulated_strain: f64,
        strain_rate: f64,
    ) -> f64 {
        if !(accumulated_strain > 0.0) || !(strain_rate > 0.0) {
            return 0.0;
        }
        self.reference_stress(temperature)
            * accumulated_strain.powf(self.hardening_exponent)
            * strain_rate.powf(1.0 / self.stress_exponent)
    }
}

/// The `META_LEMA_ANI` law — anisotropic Lemaitre creep of Zircaloy with
/// metallurgical phase dependence.
///
/// ASTER behaviour name: `META_LEMA_ANI` (`num_lc = 58`). Declared upstream as
/// a `LoiComportementMFront`, so
/// [`AsterBehaviour::MetaLemaAni.is_mfront()`](super::catalogue::AsterBehaviour::is_mfront)
/// is `true` and the algorithm lives in `mfront/META_LEMA_ANI.mfront` rather
/// than in a Fortran subroutine. Upstream integration: `NEWTON_PERT` on the
/// full implicit system with a numerical Jacobian. Documentation: `R4.04.04`
/// (metallurgy) and `R4.04.05` (mechanics).
///
/// # What the law is for
///
/// Fuel cladding during a loss-of-coolant accident. On the temperature ramp the
/// tube passes through the α → β transformation of zirconium, and the two
/// phases creep utterly differently — β-Zr, body-centred cubic and hot, is
/// orders of magnitude softer and very much less anisotropic than textured
/// α-Zr. Ballooning and burst are therefore governed by *where in the
/// transformation the tube is* when the stress arrives, which is why a
/// mechanical law here has to carry metallurgy.
///
/// # The three phases
///
/// Upstream carries three parameter sets, blended by weights that depend on the
/// α fraction `Za = 1 - Zb`:
///
/// | Set | Upstream prefix | Active when |
/// |---|---|---|
/// | [`alpha`](Self::alpha) | `F1_` | `Za ≥ 0.99` — essentially pure α |
/// | [`mixed`](Self::mixed) | `F2_` | `0.1 ≤ Za ≤ 0.9` — the two-phase field |
/// | [`beta`](Self::beta) | `C_` | `Za ≤ 0.01` — essentially pure β |
///
/// with linear ramps across the narrow bands between. See
/// [`phase_weights`](Self::phase_weights).
///
/// # What is not here
///
/// The **kinetics of `Zb` itself**. Upstream integrates the β fraction as a
/// fourth state variable, with separate heating and cooling laws and a
/// rate-dependent transformation-onset temperature (`R4.04.04`, and the
/// standalone `ZIRC` / `ZIRC_META` behaviours in
/// `bibfor/metallurgy/zedgar.F90`). This port takes `Zb` as an **input** to
/// every method, so a caller can drive it from any phase model — including a
/// future port of `ZIRC` — without this law having an opinion. That is a real
/// gap, and it is stated rather than papered over.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetaLemaAni {
    /// Viscoplastic parameters of the α-phase set (upstream `F1_`).
    pub alpha: MetaLemaAniPhase,
    /// Viscoplastic parameters of the two-phase set (upstream `F2_`).
    pub mixed: MetaLemaAniPhase,
    /// Viscoplastic parameters of the β-phase set (upstream `C_`).
    pub beta: MetaLemaAniPhase,
    /// Hill coefficients of the α phase (upstream `F_M..`).
    pub alpha_anisotropy: HillAnisotropy,
    /// Hill coefficients of the β phase (upstream `C_M..`).
    ///
    /// β-Zr is cubic and close to isotropic, so this is usually near
    /// [`HillAnisotropy::VON_MISES`].
    pub beta_anisotropy: HillAnisotropy,
}

/// What one `META_LEMA_ANI` step produced.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetaLemaAniIncrement {
    /// Equivalent viscoplastic strain increment `Δp` \[-\], non-negative.
    pub equivalent_increment: f64,
    /// Viscoplastic strain increment tensor \[-\], `Δp · n` with `n` the **Hill**
    /// flow direction.
    ///
    /// Deviatoric — the Hill contraction is traceless — but *not* parallel to
    /// the stress deviator unless the material is isotropic.
    pub strain_increment: SymmTensor,
    /// Stress at the end of the step \[Pa\].
    pub stress: SymmTensor,
    /// Hill equivalent of [`stress`](Self::stress) \[Pa\].
    pub equivalent_stress: f64,
    /// Local-solver iterations used.
    pub iterations: usize,
}

impl MetaLemaAni {
    /// The upstream ASTER behaviour name.
    #[must_use]
    pub const fn aster_name(self) -> &'static str {
        "META_LEMA_ANI"
    }

    /// Blending weights `(f_α, f_mixed, f_β)` for the α fraction `za` \[-\].
    ///
    /// Transcribes the `f[0]`, `f[1]`, `f[2]` ladder of the MFront `@Integrator`
    /// block. The three always sum to one — a partition of unity, which is what
    /// makes the blended viscous stress a genuine interpolation rather than an
    /// arbitrary sum, and which is pinned by a test.
    ///
    /// The ramps are deliberately narrow (`0.9 → 0.99` and `0.01 → 0.1`): the
    /// single-phase parameter sets are meant to hold right up to the edges of
    /// the transformation, with only a thin blending band.
    #[must_use]
    pub fn phase_weights(za: f64) -> (f64, f64, f64) {
        let f_alpha = if za >= 0.99 {
            1.0
        } else if za >= 0.9 {
            (za - 0.9) / 0.09
        } else {
            0.0
        };
        let f_beta = if za >= 0.1 {
            0.0
        } else if za >= 0.01 {
            (0.1 - za) / 0.09
        } else {
            1.0
        };
        let f_mixed = if za >= 0.99 {
            0.0
        } else if za >= 0.9 {
            1.0 - (za - 0.9) / 0.09
        } else if za >= 0.1 {
            1.0
        } else if za >= 0.01 {
            1.0 - (0.1 - za) / 0.09
        } else {
            0.0
        };
        (f_alpha, f_mixed, f_beta)
    }

    /// The Hill coefficients in force at β-phase fraction `beta_fraction`
    /// \[-\], on `[0, 1]`.
    ///
    /// Upstream blends linearly in the α fraction across the two-phase field
    /// and snaps to the pure sets outside `0.01 ≤ Za ≤ 0.99`.
    ///
    /// # A small discontinuity, reproduced
    ///
    /// The snap is not continuous with the blend: approaching `Za = 0.99` from
    /// below gives `0.99·M_α + 0.01·M_β`, while at `Za = 0.99` exactly the
    /// result jumps to `M_α`. The jump is `0.01·(M_α - M_β)` — small, but a
    /// genuine discontinuity in the yield surface and hence in the consistent
    /// tangent. The same happens at `Za = 0.01`. Reproduced rather than
    /// smoothed, and measured in a test.
    #[must_use]
    pub fn anisotropy_at(self, beta_fraction: f64) -> HillAnisotropy {
        let za = 1.0 - beta_fraction;
        if za >= 0.99 {
            self.alpha_anisotropy
        } else if za >= 0.01 {
            self.alpha_anisotropy.blend(self.beta_anisotropy, za)
        } else {
            self.beta_anisotropy
        }
    }

    /// The blended viscous stress `σ_v = Σ_i f_i γ_i p^{m_i} ṗ^{1/n_i}` \[Pa\].
    ///
    /// This is the quantity the Hill equivalent stress must equal at
    /// convergence.
    ///
    /// - `beta_fraction` — `Zb` \[-\] on `[0, 1]`.
    /// - `temperature` \[K\], strictly positive.
    /// - `accumulated_strain` — `p` \[-\], end-of-step.
    /// - `strain_rate` — `ṗ` \[1/s\], non-negative.
    ///
    /// Monotone non-decreasing in both `p` and `ṗ` for non-negative exponents,
    /// which is what guarantees the step residual has a unique root.
    #[must_use]
    pub fn viscous_stress(
        self,
        beta_fraction: f64,
        temperature: f64,
        accumulated_strain: f64,
        strain_rate: f64,
    ) -> f64 {
        let (fa, fm, fb) = Self::phase_weights(1.0 - beta_fraction);
        fa * self
            .alpha
            .viscous_stress(temperature, accumulated_strain, strain_rate)
            + fm * self
                .mixed
                .viscous_stress(temperature, accumulated_strain, strain_rate)
            + fb * self
                .beta
                .viscous_stress(temperature, accumulated_strain, strain_rate)
    }

    /// Integrate one step with an **anisotropic** return.
    ///
    /// # Why the isotropic radial return does not work here
    ///
    /// For von Mises flow the increment points along the stress deviator, so the
    /// deviator shrinks without rotating and one scalar suffices. Under Hill it
    /// points along `M : σ`, which is *not* parallel to `dev(σ)`, so the stress
    /// direction changes during the step and the tensorial problem does not
    /// obviously reduce.
    ///
    /// # The reduction that does work
    ///
    /// It reduces anyway, exactly, because the elastic operator and the flow
    /// direction share a structure. The step equation is
    ///
    /// `σ + 2μ Δp (M:σ)/σ_H = σ_trial`
    ///
    /// — the bulk term drops out because `M : σ` is traceless. Introduce the
    /// single scalar `β = 2μ Δp / σ_H`, and it becomes **linear** in `σ`:
    ///
    /// `(I + β M) : σ = σ_trial`
    ///
    /// which inverts in closed form (a 3×3 symmetric solve on the normal
    /// components; the three shears are scalar). Everything else follows:
    /// `σ_H(β)` from the inverted stress, `Δp(β) = β σ_H(β)/(2μ)`, and the flow
    /// rule `σ_H(β) = σ_v(Δp(β)/Δt)` closes the problem in one unknown.
    ///
    /// The residual is monotone decreasing — more `β` means more relaxation and
    /// more strain, so the Hill stress falls while the viscous stress rises — so
    /// it has exactly one root, and [`brent`] finds
    /// it on a bracket grown from zero. This is not upstream's algorithm
    /// (`NEWTON_PERT` on the full 7-unknown implicit system); it is an exact
    /// reduction of the same equations, and it is cheaper and more robust.
    ///
    /// # Arguments
    ///
    /// - `trial_stress` — elastic-predictor stress \[Pa\], with the thermal
    ///   eigenstrain already removed.
    /// - `shear_modulus` — `μ` \[Pa\], strictly positive.
    /// - `beta_fraction` — `Zb` \[-\] on `[0, 1]`, end-of-step.
    /// - `temperature` \[K\], strictly positive, end-of-step.
    /// - `accumulated_strain` — `p` \[-\] at the *start* of the step.
    /// - `dt` — timestep \[s\]. Zero is legal and yields no creep.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a non-positive shear modulus, a
    /// non-positive temperature, a negative timestep, or a β fraction outside
    /// `[0, 1]`; [`OffbeatError::ConstitutiveNotConverged`] if no bracket can be
    /// found or the local solve fails.
    pub fn integrate(
        self,
        trial_stress: SymmTensor,
        shear_modulus: f64,
        beta_fraction: f64,
        temperature: f64,
        accumulated_strain: f64,
        dt: f64,
    ) -> Result<MetaLemaAniIncrement> {
        check_shear_modulus(shear_modulus)?;
        check_temperature(temperature)?;
        if dt < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "timestep",
                value: dt,
                unit: "s",
                reason: "must not be negative",
            });
        }
        if !(0.0..=1.0).contains(&beta_fraction) {
            return Err(OffbeatError::Unphysical {
                quantity: "beta phase fraction",
                value: beta_fraction,
                unit: "-",
                reason: "must lie in [0, 1]; it is a volume fraction",
            });
        }

        let hill = self.anisotropy_at(beta_fraction);
        let eq_trial = hill.equivalent_stress(trial_stress);

        if dt == 0.0 || !(eq_trial > 0.0) {
            return Ok(MetaLemaAniIncrement {
                equivalent_increment: 0.0,
                strain_increment: SymmTensor::ZERO,
                stress: trial_stress,
                equivalent_stress: eq_trial,
                iterations: 0,
            });
        }

        let two_mu = 2.0 * shear_modulus;
        // For a candidate beta, the relaxed stress, its Hill equivalent, and
        // the creep increment it implies.
        let state_at = |beta: f64| {
            let sigma = relax_under_hill(hill, trial_stress, beta);
            let eq = hill.equivalent_stress(sigma);
            let dp = beta * eq / two_mu;
            (sigma, eq, dp)
        };
        let residual = |beta: f64| {
            let (_, eq, dp) = state_at(beta);
            eq - self.viscous_stress(beta_fraction, temperature, accumulated_strain + dp, dp / dt)
        };

        // beta is dimensionless and of order 2*mu*Dp/sigma_H, i.e. O(1) for a
        // typical creep step. Grow a bracket from well below that.
        let mut upper = 1.0e-8;
        let mut bracketed = false;
        for _ in 0..80 {
            if residual(upper) < 0.0 {
                bracketed = true;
                break;
            }
            upper *= 4.0;
        }
        if !bracketed {
            return Err(OffbeatError::ConstitutiveNotConverged {
                cell: usize::MAX,
                residual: residual(upper).abs(),
                iterations: 80,
            });
        }

        let solution = brent(
            residual,
            (0.0, upper),
            &SolverControl {
                max_iter: 300,
                residual_tol: 1.0e-10 * eq_trial,
                step_tol: 1.0e-18,
            },
        )?;

        let (stress, eq, dp) = state_at(solution.root.max(0.0));
        Ok(MetaLemaAniIncrement {
            equivalent_increment: dp,
            strain_increment: scale_tensor(hill.flow_direction(stress), dp),
            stress,
            equivalent_stress: eq,
            iterations: solution.iterations,
        })
    }
}

/// Solve `(I + β M) : σ = σ_trial` for `σ`.
///
/// The normal components couple through a symmetric 3×3 matrix and the three
/// shears are independent scalars, because the Hill tensor is block diagonal in
/// the material frame. Solved by the explicit adjugate; the matrix is
/// `I + β·(a positive semi-definite form)` and is therefore non-singular for
/// `β ≥ 0` whenever the coefficients are physical.
///
/// Falls back to the trial stress if the 3×3 block turns out singular, which can
/// only happen for a coefficient set that is not positive semi-definite.
fn relax_under_hill(hill: HillAnisotropy, trial: SymmTensor, beta: f64) -> SymmTensor {
    let (f, g, h) = hill.fgh();

    // Normal block: I + beta*N with
    //   N = [[F+H, -F, -H], [-F, G+F, -G], [-H, -G, H+G]]
    let a11 = 1.0 + beta * (f + h);
    let a22 = 1.0 + beta * (g + f);
    let a33 = 1.0 + beta * (h + g);
    let a12 = -beta * f;
    let a13 = -beta * h;
    let a23 = -beta * g;

    let c11 = a22 * a33 - a23 * a23;
    let c12 = a13 * a23 - a12 * a33;
    let c13 = a12 * a23 - a13 * a22;
    let det = a11 * c11 + a12 * c12 + a13 * c13;
    if det.abs() < f64::MIN_POSITIVE {
        return trial;
    }
    let c22 = a11 * a33 - a13 * a13;
    let c23 = a12 * a13 - a11 * a23;
    let c33 = a11 * a22 - a12 * a12;

    let (b1, b2, b3) = (trial.xx, trial.yy, trial.zz);
    let xx = (c11 * b1 + c12 * b2 + c13 * b3) / det;
    let yy = (c12 * b1 + c22 * b2 + c23 * b3) / det;
    let zz = (c13 * b1 + c23 * b2 + c33 * b3) / det;

    SymmTensor::new(
        xx,
        trial.xy / (1.0 + 2.0 * beta * hill.m_xy),
        trial.xz / (1.0 + 2.0 * beta * hill.m_xz),
        yy,
        trial.yz / (1.0 + 2.0 * beta * hill.m_yz),
        zz,
    )
}

#[cfg(test)]
mod tests;
