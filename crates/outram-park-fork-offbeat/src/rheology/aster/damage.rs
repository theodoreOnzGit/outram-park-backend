// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from code_aster (https://gitlab.com/codeaster/src)
//   Copyright (C) 1991 - 2026 - EDF R&D
//   Licence: GPL-3.0-or-later
//   Upstream commit: b504ea08c2f49575e04644cee2e39a63ea45c16e
//   Sources:
//     bibfor/lc/lc0031.F90            -- VENDOCHAB / VISC_ENDO_LEMA dispatch (`num_lc = 31`)
//     bibfor/comport/nmveei.F90       -- VENDOCHAB implicit driver (`nmveei`)
//     bibfor/nonlinear/nmvecd.F90     -- VENDOCHAB residuals and tangent (`nmvecd`)
//     bibfor/nonlinear/nmvexi.F90     -- damage-equivalent stress chi (`nmvexi`)
//     bibfor/algorith/rkdvec.F90      -- VENDOCHAB Runge-Kutta rates (`rkdvec`)
//     bibfor/comport/nmvend.F90       -- VISC_ENDO_LEMA reduced solve (`nmvend`)
//     bibfor/nonlinear/nmfend.F90     -- VISC_ENDO_LEMA scalar residual (`nmfend`)
//     bibfor/algorith/vecmat.F90      -- VENDOCHAB material-slot mapping (`vecmat`)
//     bibfor/lc/lc0030.F90            -- ROUSS_PR / ROUSS_VISC dispatch (`num_lc = 30`)
//     bibfor/comport/lcrous.F90       -- Rousselier local integration (`lcrous`)
//     bibfor/comport/rslphi.F90       -- Rousselier residual in the porosity increment (`rslphi`)
//     bibfor/comport/rslcvx.F90       -- Rousselier yield function (`rslcvx`)
//     bibfor/comport/rslmat.F90       -- ROUSS_PR material-slot mapping (`rslmat`)
//     bibfor/comport/rsvmat.F90       -- ROUSS_VISC material-slot mapping (`rsvmat`)
//     bibfor/lc/lc0075.F90            -- GTN / VISC_GTN dispatch (`num_lc = 75`)
//     bibfor/algorith/lcgtn_module.F90 -- GTN yield surface, coalescence, nucleation
//     bibfor/algorith/visc_norton_module.F90 -- Norton overstress used by VISC_GTN
//     bibfor/algorith/crirup.F90      -- CRIT_RUPT post-iteration criterion (`crirup`)
//     bibfor/algorith/rupmat.F90      -- CRIT_RUPT stiffness degradation (`rupmat`)
//     bibfor/algorith/fgequi.F90      -- equivalent quantities, principal stresses (`fgequi`)
//     bibfor/utilifor/lcnrts.F90      -- second-invariant norm (`lcnrts`)
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Damage and rupture laws — bead `op-a7p.4`, phase P3 of the code_aster port.
//!
//! # What these laws are for
//!
//! Everything in [`crate::rheology::aster::viscoplastic`] conserves the
//! material: stress relaxes, strain accumulates, but the solid stays the solid
//! it started as. The laws here do not. Each carries an internal variable that
//! records irreversible *loss of load-bearing material* — a scalar damage `D`
//! for the Lemaitre-Chaboche family, a void volume fraction (porosity) `f` for
//! the Rousselier and Gurson families — and each feeds that variable back into
//! the stress so the material softens as it degrades.
//!
//! That feedback is the whole point and the whole difficulty. It is what lets
//! the model predict *when a component fails* rather than merely how much it
//! deforms, and it is also what makes the local integration hard: the softening
//! branch is where the boundary-value problem loses ellipticity, and where a
//! local solve that quietly clamps its unknown will report a converged answer
//! that is not a solution of anything.
//!
//! # The three families, and how they differ
//!
//! | Family | State variable | Yield surface | Failure mechanism modelled |
//! |---|---|---|---|
//! | [`LemaitreChabocheLaw`] (`VENDOCHAB`, `VISC_ENDO_LEMA`) | scalar damage `D` | von Mises on the **effective** stress `sigma/(1-D)` | creep damage — cavitation under sustained high-temperature load |
//! | [`RousselierLaw`] (`ROUSS_PR`, `ROUSS_VISC`) | porosity `f` | von Mises **plus** an exponential in the mean stress | ductile rupture — growth and coalescence of voids |
//! | [`GursonTvergaardNeedleman`] (`GTN`, `VISC_GTN`) | porosity `f` + coalescence | von Mises **plus** a `cosh` in the mean stress | ductile rupture, with nucleation and an explicit coalescence stage |
//!
//! The Lemaitre-Chaboche family keeps a pressure-independent (von Mises) yield
//! surface and puts all the damage in a multiplicative `(1-D)` factor. The two
//! porous-plastic families instead make the **yield surface itself** depend on
//! the hydrostatic stress, because that is the physics: a void grows under
//! triaxial tension and closes under triaxial compression, so a porous solid
//! yields sooner in tension than in compression and its plastic flow is no
//! longer volume-preserving. That single change is why their local solve cannot
//! be the scalar radial return used for `NORTON` and `LEMAITRE` — the plastic
//! increment now has a volumetric component, which changes the porosity, which
//! moves the yield surface, so the equivalent plastic strain and the porosity
//! must be solved **together**.
//!
//! # Softening, and what this port does about it
//!
//! Every law here softens: past some point, more strain means less stress. Two
//! things go wrong there, and they are different things.
//!
//! 1. **Loss of ellipticity** of the boundary-value problem. Once the tangent
//!    modulus goes negative the *structural* problem is ill-posed — the
//!    solution localises into a band whose width is set by the mesh, not by the
//!    material. No local integrator can fix that; it needs a regularisation
//!    (nonlocal or gradient damage). Upstream's `VISC_GTN` supplies one via the
//!    `GRADVARI` modelisation; **this port does not** (see
//!    [`GursonTvergaardNeedleman`]).
//! 2. **Failure of the local solve** as `D -> 1` or `f -> f_R`. This *is* the
//!    integrator's business, and here the port is deliberate: when the local
//!    system has no solution in the admissible range, the integrator says so.
//!    It never clamps the unknown at the boundary and reports success.
//!
//! Concretely, each law has one place where upstream itself saturates rather
//! than converges, and this port reproduces that saturation **as an explicitly
//! reported state, not as a converged solve**:
//!
//! - [`LemaitreChabocheLaw`]: upstream caps damage at `D = 0.99`
//!   (`dammax` in `nmvecd.F90`) and raises alarm `ALGORITH8_67`. This port
//!   returns [`DamageOutcome::Saturated`] on the same condition, visible in
//!   [`LemaitreChabocheIncrement::outcome`].
//! - [`RousselierLaw`]: upstream declares the point broken once the porosity
//!   reaches `PORO_LIMI` and ramps the stress to zero. Reported as
//!   [`RousselierOutcome::Broken`]. A genuinely empty bracket in the porosity
//!   increment — upstream's "subdivide the step" exit — becomes
//!   [`OffbeatError::ConstitutiveNotConverged`].
//! - [`GursonTvergaardNeedleman`]: upstream stops at `dam >= dam_bkn`. Reported
//!   as [`GtnOutcome::Broken`]; a non-convergent staggered solve returns
//!   [`OffbeatError::ConstitutiveNotConverged`] instead.
//!
//! # Conventions
//!
//! Raw `f64` with units stated in prose, matching
//! [`crate::rheology::aster::viscoplastic`] — not `uom`. Tensors are
//! [`SymmTensor`] from `outram-foam-basic-lib`; the `sqrt(2)`-scaled Mandel
//! six-vector used at the code_aster interface lives in
//! [`crate::rheology::aster::kinematics::AsterVoigt`] and is not needed here,
//! because every contraction in this module is done tensorially.
//!
//! All laws here are **small-strain** (`DEFORMATION = PETIT`). Upstream also
//! offers `GDEF_LOG` for the Rousselier and GTN families; the logarithmic-strain
//! pre/post-processing that would wrap these laws is in
//! [`crate::rheology::aster::log_strain`] and is not applied here.
//!
//! # Status and what is *not* ported
//!
//! Ported, with tests: `VENDOCHAB`, `VISC_ENDO_LEMA`, `ROUSS_PR`, `ROUSS_VISC`,
//! `GTN` / `VISC_GTN` (local form only, see below), `CRIT_RUPT`.
//!
//! **Not ported.** The `ENDO_*` concrete-damage family (`ENDO_ISOT_BETON`,
//! `ENDO_ORTH_BETON`, `ENDO_SCALAIRE`, `ENDO_FISS_EXP`, ...) is untouched: it
//! is a different physical domain (quasi-brittle concrete, with unilateral
//! crack closure and, for several members, a nonlocal `GRAD_VARI` formulation),
//! and the workspace's target cases are metals. The `GTN` port covers the
//! **local** yield surface, coalescence, nucleation and return map but **not**
//! upstream's `GRADVARI` nonlocal regularisation, nor its bespoke
//! `SPECIFIQUE` reformulation in `(p, ts)` variables; see
//! [`GursonTvergaardNeedleman`] for the exact boundary.
//!
//! # Upstream defects found
//!
//! Two, both in `VENDOCHAB`, both documented by a test in this module's test
//! file rather than silently corrected:
//!
//! 1. **`nmvexi.F90` reads the wrong material slots.** It takes the
//!    multiaxiality weights `ALPHA_D` and `BETA_D` of the damage-equivalent
//!    stress from `mate(2,2)` and `mate(3,2)`, which `vecmat.F90` fills with
//!    `UN_SUR_M` and `UN_SUR_K` — the Lemaitre viscosity parameters. The
//!    correct slots are `mate(5,2)` and `mate(6,2)`, which is what the
//!    Runge-Kutta path (`rkdvec.F90`) uses.
//! 2. **The implicit path accumulates damage with no plasticity.**
//!    `nmvecd.F90` evaluates the damage-rate equation unconditionally, so a
//!    purely elastic step with a non-zero `chi` still damages the material. The
//!    Runge-Kutta path (`rkdvec.F90`) and the `VISC_ENDO_LEMA` path
//!    (`nmvend.F90`) both gate damage on the plasticity criterion.
//!
//! This port follows the Runge-Kutta semantics on both points — they are the
//! self-consistent ones and the ones that match the declared catalogue — while
//! keeping the **implicit backward-Euler discretisation** of the `NEWTON` path.
//! See [`LemaitreChabocheLaw`] for the reasoning.

use outram_foam_basic_lib::primitives::{eigen_values_symm, SymmTensor};

use crate::error::{OffbeatError, Result};
use crate::rheology::aster::catalogue::AsterBehaviour;
use crate::rheology::aster::integration::{brent, newton_safeguarded, SolverControl};
use crate::rheology::aster::viscoplastic::{deviator, von_mises_of_deviator};

// ===========================================================================
// Shared elastic and hardening description
// ===========================================================================

/// Isotropic linear elasticity, as the two moduli a return map actually needs.
///
/// # Why these two and not `E`, `nu`
///
/// Every return map in this module splits the stress into its deviatoric and
/// hydrostatic parts and treats them separately: the deviator scales with the
/// shear modulus `mu`, the mean stress with the bulk modulus `K`. Storing
/// `(mu, K)` therefore removes a conversion from the inner loop and makes the
/// two roles visible. Use [`IsotropicElasticity::from_young_poisson`] when the
/// data are tabulated the usual way.
///
/// # Units
///
/// Both moduli in pascal \[Pa\], both strictly positive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IsotropicElasticity {
    /// Shear modulus `mu = E / (2(1+nu))` \[Pa\], strictly positive.
    pub shear_modulus: f64,
    /// Bulk modulus `K = E / (3(1-2nu))` \[Pa\], strictly positive.
    ///
    /// Relates mean stress to volumetric strain by `sigma_m = K tr(eps)`.
    pub bulk_modulus: f64,
}

impl IsotropicElasticity {
    /// Build from Young's modulus \[Pa\] and Poisson's ratio \[-\].
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a non-positive Young's modulus, or a
    /// Poisson ratio outside `(-1, 0.5)` — the open interval on which an
    /// isotropic solid is thermodynamically stable. `nu = 0.5` is excluded
    /// because it makes the bulk modulus infinite; a genuinely incompressible
    /// material needs a mixed formulation, not this one.
    pub fn from_young_poisson(young: f64, poisson: f64) -> Result<Self> {
        if !(young > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "Young's modulus",
                value: young,
                unit: "Pa",
                reason: "must be strictly positive",
            });
        }
        if !(poisson > -1.0 && poisson < 0.5) {
            return Err(OffbeatError::Unphysical {
                quantity: "Poisson's ratio",
                value: poisson,
                unit: "-",
                reason: "must lie strictly inside (-1, 0.5) for a stable, \
                         compressible isotropic solid",
            });
        }
        Ok(Self {
            shear_modulus: young / (2.0 * (1.0 + poisson)),
            bulk_modulus: young / (3.0 * (1.0 - 2.0 * poisson)),
        })
    }

    /// Young's modulus `E = 9 K mu / (3K + mu)` \[Pa\].
    #[must_use]
    pub fn young(self) -> f64 {
        9.0 * self.bulk_modulus * self.shear_modulus
            / (3.0 * self.bulk_modulus + self.shear_modulus)
    }

    /// Poisson's ratio `nu = (3K - 2mu) / (2(3K + mu))` \[-\].
    #[must_use]
    pub fn poisson(self) -> f64 {
        (3.0 * self.bulk_modulus - 2.0 * self.shear_modulus)
            / (2.0 * (3.0 * self.bulk_modulus + self.shear_modulus))
    }

    /// Reject non-positive moduli.
    fn validate(self) -> Result<()> {
        if !(self.shear_modulus > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "shear modulus",
                value: self.shear_modulus,
                unit: "Pa",
                reason: "must be strictly positive",
            });
        }
        if !(self.bulk_modulus > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "bulk modulus",
                value: self.bulk_modulus,
                unit: "Pa",
                reason: "must be strictly positive",
            });
        }
        Ok(())
    }
}

/// Isotropic hardening `R(p)` — the current radius of the yield surface.
///
/// # What it represents
///
/// `R(p)` is the flow stress \[Pa\] a material offers after accumulating
/// equivalent plastic strain `p` \[-\]. It is the quantity a tensile test
/// measures, and in code_aster it is normally supplied *point by point* as a
/// `TRACTION` curve read by `rsliso.F90`. Tabulated curves are a data-plumbing
/// concern rather than a physics one, so this port offers the closed-form
/// families instead and leaves the table for the caller to interpolate.
///
/// Enum dispatch, not trait objects, per the workspace rule.
///
/// # Units
///
/// Every stress-dimensioned field is in pascal \[Pa\]; `p` and every exponent
/// are dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IsotropicHardening {
    /// Perfect plasticity: `R(p) = sigma_y`, no hardening at all.
    ///
    /// The limiting case, and the one that makes a return map's bracket
    /// degenerate (the residual becomes exactly zero at its upper endpoint), so
    /// it is worth testing against explicitly.
    Perfect {
        /// Initial yield stress `sigma_y` \[Pa\], strictly positive.
        yield_stress: f64,
    },
    /// Linear hardening: `R(p) = sigma_y + H p`.
    Linear {
        /// Initial yield stress `sigma_y` \[Pa\], strictly positive.
        yield_stress: f64,
        /// Plastic modulus `H` \[Pa\]. Negative values describe linear
        /// softening and are permitted, but they make the local solve
        /// non-monotone — see the module documentation.
        modulus: f64,
    },
    /// Power-law (Ludwik) hardening: `R(p) = sigma_y + K p^n`.
    ///
    /// The classical fit for metals. Note `dR/dp` is infinite at `p = 0` for
    /// `n < 1`, which is real and not a coding error; [`Self::slope`] returns
    /// the slope at a small positive offset there rather than an infinity, so a
    /// Newton step stays finite.
    ///
    /// **Caution for the porous-plastic laws.** That infinite initial slope
    /// makes the *first* plastic increment of a virgin point genuinely
    /// ill-conditioned: with `n = 0.1` the flow stress climbs by `0.3 K` over a
    /// plastic strain of `1e-6`, so the local residual falls by order `K`
    /// across a porosity increment of order `1e-9` and any bracketed solver
    /// collapses its bracket to machine precision with the residual still
    /// large. That is a property of the curve, not of the return map. Prefer
    /// [`Self::EcroNl`] or [`Self::Linear`] — or an interpolated `TRACTION`
    /// table, which is what code_aster itself uses and which is piecewise
    /// linear and therefore finite-sloped throughout.
    PowerLaw {
        /// Initial yield stress `sigma_y` \[Pa\], strictly positive.
        yield_stress: f64,
        /// Hardening coefficient `K` \[Pa\], non-negative.
        coefficient: f64,
        /// Hardening exponent `n` \[-\], typically 0.05-0.5 for structural
        /// steels.
        exponent: f64,
    },
    /// code_aster's `ECRO_NL` nonlinear isotropic hardening — the form
    /// upstream's GTN law requires.
    ///
    /// `R(p) = R0 + RH p + R1 (1 - exp(-g1 p)) + R2 (1 - exp(-g2 p)) + RK (p + P0)^gm`
    ///
    /// Upstream: `f_ecro` in `bibfor/algorith/lcgtn_module.F90`, keywords
    /// `R0`, `RH`, `R1`, `GAMMA_1`, `R2`, `GAMMA_2`, `RK`, `P0`, `GAMMA_M`.
    /// The two saturating exponentials give the knee of the curve at small
    /// strain, the linear term the far-field slope, and the power term a
    /// tunable tail.
    EcroNl {
        /// `R0` — initial yield stress \[Pa\], strictly positive.
        r0: f64,
        /// `RH` — linear hardening modulus \[Pa\].
        rh: f64,
        /// `R1` — amplitude of the first saturating term \[Pa\].
        r1: f64,
        /// `GAMMA_1` — rate of the first saturating term \[-\].
        gamma_1: f64,
        /// `R2` — amplitude of the second saturating term \[Pa\].
        r2: f64,
        /// `GAMMA_2` — rate of the second saturating term \[-\].
        gamma_2: f64,
        /// `RK` — amplitude of the power term \[Pa\].
        rk: f64,
        /// `P0` — offset of the power term \[-\]; keeps it finite at `p = 0`.
        p0: f64,
        /// `GAMMA_M` — exponent of the power term \[-\]. Upstream defaults it
        /// to 1 when the keyword is absent.
        gamma_m: f64,
    },
}

impl IsotropicHardening {
    /// Flow stress `R(p)` \[Pa\] at accumulated equivalent plastic strain `p`.
    ///
    /// `p` must be non-negative; a negative argument is treated as zero, which
    /// is the physically meaningful extension (plastic strain never
    /// un-accumulates) rather than an error, because a Newton iterate can
    /// transiently overshoot below zero.
    #[must_use]
    pub fn value(self, p: f64) -> f64 {
        let p = p.max(0.0);
        match self {
            Self::Perfect { yield_stress } => yield_stress,
            Self::Linear {
                yield_stress,
                modulus,
            } => yield_stress + modulus * p,
            Self::PowerLaw {
                yield_stress,
                coefficient,
                exponent,
            } => yield_stress + coefficient * p.powf(exponent),
            Self::EcroNl {
                r0,
                rh,
                r1,
                gamma_1,
                r2,
                gamma_2,
                rk,
                p0,
                gamma_m,
            } => {
                r0 + rh * p
                    + r1 * (1.0 - (-gamma_1 * p).exp())
                    + r2 * (1.0 - (-gamma_2 * p).exp())
                    + rk * (p + p0).powf(gamma_m)
            }
        }
    }

    /// Hardening slope `dR/dp` \[Pa\] at accumulated equivalent plastic strain
    /// `p`.
    ///
    /// Supplied so the local solves can take a true Newton step. For the
    /// power-law family with an exponent below one the true slope diverges at
    /// the origin; this returns the slope at `p = 1e-12` instead, which is
    /// finite and keeps the safeguarded Newton in
    /// [`crate::rheology::aster::integration`] from proposing an infinite step.
    #[must_use]
    pub fn slope(self, p: f64) -> f64 {
        let p = p.max(0.0);
        match self {
            Self::Perfect { .. } => 0.0,
            Self::Linear { modulus, .. } => modulus,
            Self::PowerLaw {
                coefficient,
                exponent,
                ..
            } => {
                let q = if exponent < 1.0 { p.max(1.0e-12) } else { p };
                coefficient * exponent * q.powf(exponent - 1.0)
            }
            Self::EcroNl {
                rh,
                r1,
                gamma_1,
                r2,
                gamma_2,
                rk,
                p0,
                gamma_m,
                ..
            } => {
                let base = if gamma_m < 1.0 {
                    (p + p0).max(1.0e-12)
                } else {
                    p + p0
                };
                rh + r1 * gamma_1 * (-gamma_1 * p).exp()
                    + r2 * gamma_2 * (-gamma_2 * p).exp()
                    + rk * gamma_m * base.powf(gamma_m - 1.0)
            }
        }
    }
}

// ===========================================================================
// Stress invariants used by the damage-equivalent stresses
// ===========================================================================

/// Mean (hydrostatic) stress `sigma_m = tr(sigma)/3` \[Pa\].
///
/// Positive in tension. This is the invariant that drives void growth in the
/// Rousselier and Gurson families, and the reason their yield surfaces are not
/// pressure-independent.
#[must_use]
pub fn mean_stress(sigma: SymmTensor) -> f64 {
    sigma.tr() / 3.0
}

/// Von Mises equivalent stress `sigma_eq = sqrt(3/2 s:s)` of a **full** stress
/// tensor \[Pa\].
///
/// Convenience wrapper that takes the deviator first — unlike
/// [`von_mises_of_deviator`], which expects the deviator and inflates its
/// answer if given a stress with a hydrostatic part.
#[must_use]
pub fn equivalent_stress(sigma: SymmTensor) -> f64 {
    von_mises_of_deviator(deviator(sigma))
}

/// Largest principal stress `J0 = max(sigma_1, sigma_2, sigma_3)` \[Pa\].
///
/// Upstream's `calcj0`. This is the invariant that makes creep damage
/// *multiaxial*: cavities open on the planes normal to the greatest tension, so
/// a state with a large maximum principal stress damages faster than a
/// deviatorically equivalent state without one. Signed — a wholly compressive
/// state returns its largest (least negative) eigenvalue.
#[must_use]
pub fn max_principal_stress(sigma: SymmTensor) -> f64 {
    // `eigen_values_symm` returns the spectrum ascending, so the last component
    // is the greatest principal stress.
    eigen_values_symm(sigma).z
}

/// Scale a symmetric tensor by a scalar.
fn scaled(t: SymmTensor, f: f64) -> SymmTensor {
    SymmTensor::new(f * t.xx, f * t.xy, f * t.xz, f * t.yy, f * t.yz, f * t.zz)
}

/// Rebuild a full tensor from a deviator and a mean stress.
fn from_dev_and_mean(dev: SymmTensor, mean: f64) -> SymmTensor {
    SymmTensor::new(
        dev.xx + mean,
        dev.xy,
        dev.xz,
        dev.yy + mean,
        dev.yz,
        dev.zz + mean,
    )
}

/// Smallest strictly positive normalised `f64`, upstream's `r8miem()`.
const R8MIEM: f64 = f64::MIN_POSITIVE;

/// Floor applied to the isotropic-hardening variable `r` before it is raised to
/// `1/m`, upstream's `epsiec`.
///
/// **This is a physical regularisation, not a numerical guard, and the exact
/// value matters enormously.** The Lemaitre flow rate divides by
/// `s_c = K r^(1/m)`, which vanishes as `r → 0`, so a pristine point has an
/// unbounded initial rate. Upstream caps it at a definite strain:
/// `bibfor/algorith/rkdvec.F90` declares `parameter(epsiec = 1.d-8)` and
/// applies `if (ecrou .le. epsiec) ecrou = epsiec`.
///
/// Using `f64::MIN_POSITIVE` here instead — as this port originally did, with a
/// comment wrongly attributing it to upstream's `r8miem()` — changes `s_c` by
/// about thirty orders of magnitude. For `ssnv126a`'s material
/// (`K = 1450` MPa, `m = 9.8`): `1e-8` gives `s_c = 1450 × 0.1526 = 221` MPa,
/// while `2.2e-308` gives `s_c = 5.8e-29` MPa. The second makes the initial
/// flow rate astronomical, so the material relaxes to nothing within a single
/// step of any size and damage pins at its ceiling.
const HARDENING_FLOOR: f64 = 1.0e-8;

// ===========================================================================
// VENDOCHAB / VISC_ENDO_LEMA — Lemaitre-Chaboche damage-coupled viscoplasticity
// ===========================================================================

/// Upstream's saturation damage, `dammax` in `nmvecd.F90` and `nmveei.F90`.
///
/// Damage is capped here rather than at 1 because the effective stress
/// `sigma/(1-D)` and the damage rate's `(1-D)^(-K_D)` factor both blow up at
/// `D = 1`. 0.99 is upstream's choice; it is a numerical fence, not physics.
pub const LEMAITRE_CHABOCHE_DAMAGE_MAX: f64 = 0.99;

/// Material parameters of the Lemaitre-Chaboche damage-coupled viscoplastic
/// law.
///
/// # Where each one comes from
///
/// Upstream reads these from two keyword blocks and packs them into one array
/// (`vecmat.F90`): `LEMAITRE` supplies `N`, `UN_SUR_M`, `UN_SUR_K`, and
/// `VENDOCHAB` supplies `SY`, `ALPHA_D`, `BETA_D`, `R_D`, `A_D`, `K_D`. The
/// upstream names are given per field so a deck can be read across; the
/// reciprocals `UN_SUR_M` and `UN_SUR_K` are stored here as `m` and `k`
/// themselves, matching
/// [`LemaitreParameters`](crate::rheology::aster::viscoplastic::LemaitreParameters).
///
/// # The two rate equations
///
/// Isotropic hardening variable `r` \[-\] and damage `D` \[-\] evolve as
///
/// `dr/dt = ((sigma_eq/(1-D) - SY) / (K r^(1/m)))^n`
///
/// `dD/dt = (chi / A_D)^R_D * (1-D)^(-K_D)`
///
/// with `chi` the damage-equivalent stress built by
/// [`LemaitreChabocheLaw::damage_equivalent_stress`]. The accumulated
/// viscoplastic strain follows `dp/dt = (dr/dt)/(1-D)`: the *effective* section
/// carries the load, so a damaged material strains faster than its hardening
/// variable advances.
///
/// # Units
///
/// `k`, `yield_stress` and `damage_strength` in pascal \[Pa\]; `n`, `m`, the
/// two weights and the two damage exponents dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LemaitreChabocheParameters {
    /// Stress exponent `n` \[-\]. Upstream `N`. Must be strictly positive;
    /// upstream errors out (`ier = 11`) on `N <= 0`.
    pub n: f64,
    /// Strain-hardening exponent `m` \[-\]. Upstream stores its reciprocal as
    /// `UN_SUR_M`. Enters as `r^(1/m)`, so larger `m` means weaker hardening.
    pub m: f64,
    /// Viscosity reference stress `K` \[Pa\]. Upstream stores `1/K` as
    /// `UN_SUR_K`.
    pub k: f64,
    /// Yield stress `SY` \[Pa\] below which no viscoplastic flow occurs. The
    /// criterion is on the **effective** stress: flow begins when
    /// `sigma_eq/(1-D) > SY`.
    pub yield_stress: f64,
    /// `ALPHA_D` \[-\] — weight of the largest principal stress in `chi`.
    ///
    /// Turns creep damage multiaxial. Zero recovers a purely deviatoric damage
    /// driver. Upstream skips the eigenvalue solve entirely when this is at or
    /// below `1e-15`, which this port reproduces.
    pub principal_weight: f64,
    /// `BETA_D` \[-\] — weight of the trace (three times the mean stress) in
    /// `chi`. Makes damage grow faster under hydrostatic tension.
    pub trace_weight: f64,
    /// `R_D` \[-\] — exponent of the damage rate in `chi`.
    pub damage_exponent: f64,
    /// `A_D` \[Pa\] — damage strength; the `chi` at which the damage rate is
    /// one per second. Strictly positive.
    pub damage_strength: f64,
    /// `K_D` \[-\] — exponent of the `(1-D)` closure term. Positive values make
    /// damage accelerate as it accumulates, which is what produces the abrupt
    /// tertiary-creep knee.
    ///
    /// Upstream allows `K_D` to be supplied as a two-dimensional `NAPPE` in
    /// temperature and `chi`; this port takes a constant only.
    pub damage_closure_exponent: f64,
}

impl LemaitreChabocheParameters {
    /// Reject parameters upstream would reject, plus the ones that make the
    /// rate equations undefined.
    fn validate(self) -> Result<()> {
        if !(self.n > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "Lemaitre stress exponent N",
                value: self.n,
                unit: "-",
                reason: "must be strictly positive (upstream nmvecd returns ier = 11)",
            });
        }
        if self.m == 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "Lemaitre hardening exponent m",
                value: self.m,
                unit: "-",
                reason: "must be non-zero; the hardening term is r^(1/m)",
            });
        }
        if !(self.k > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "Lemaitre reference stress K",
                value: self.k,
                unit: "Pa",
                reason: "must be strictly positive",
            });
        }
        if !(self.damage_strength > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "damage strength A_D",
                value: self.damage_strength,
                unit: "Pa",
                reason: "must be strictly positive; the damage rate divides by it",
            });
        }
        if self.yield_stress < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "yield stress SY",
                value: self.yield_stress,
                unit: "Pa",
                reason: "must not be negative",
            });
        }
        Ok(())
    }
}

/// Internal state of a Lemaitre-Chaboche damage point, at one instant.
///
/// Mirrors upstream's ten internal variables `EPSPXX..EPSPYZ`, `EPSPEQ`,
/// `ECROISOT`, `ENDO`, `INDIPLAS` — the last of which (an iteration counter) is
/// not state and is not kept.
///
/// # Units
///
/// The strain tensor and all three scalars are dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LemaitreChabocheState {
    /// Viscoplastic strain tensor `eps_vp` \[-\]. Deviatoric: this law's flow
    /// is volume-preserving, unlike the porous-plastic laws below.
    pub viscoplastic_strain: SymmTensor,
    /// Accumulated equivalent viscoplastic strain `p` \[-\], upstream
    /// `EPSPEQ`. Grows as `dr/(1-D)`.
    pub equivalent_viscoplastic_strain: f64,
    /// Isotropic hardening variable `r` \[-\], upstream `ECROISOT`. Grows as
    /// `dr`; equals `p` only while the material is undamaged.
    pub hardening_variable: f64,
    /// Damage `D` \[-\], upstream `ENDO`, in `[0, 1)`. The load-bearing section
    /// is `(1-D)` of the nominal one.
    pub damage: f64,
}

impl LemaitreChabocheState {
    /// The pristine state: no viscoplastic strain, no hardening, no damage.
    #[must_use]
    pub fn pristine() -> Self {
        Self {
            viscoplastic_strain: SymmTensor::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            equivalent_viscoplastic_strain: 0.0,
            hardening_variable: 0.0,
            damage: 0.0,
        }
    }
}

/// How a Lemaitre-Chaboche step ended.
///
/// Reported rather than hidden, because the difference between "the local
/// system was solved" and "the damage hit its numerical ceiling" is exactly the
/// difference between a result and an artefact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DamageOutcome {
    /// No viscoplastic flow: the effective equivalent stress stayed at or below
    /// `SY`. Damage does not advance either, following the Runge-Kutta
    /// (`rkdvec.F90`) and `VISC_ENDO_LEMA` (`nmvend.F90`) semantics; see the
    /// module documentation for the discrepancy with `nmvecd.F90`.
    Elastic,
    /// The coupled `(dr, D)` system was solved with `D` strictly below
    /// [`LEMAITRE_CHABOCHE_DAMAGE_MAX`]. This is the only outcome that is a
    /// genuine solution of the constitutive law.
    Converged,
    /// The damage equation had no root below [`LEMAITRE_CHABOCHE_DAMAGE_MAX`]:
    /// over this step the material would have damaged past upstream's ceiling.
    ///
    /// Upstream caps `D` at 0.99, zeroes the damage rate and raises alarm
    /// `ALGORITH8_67`; this port does the same *and says so here*. The returned
    /// stress and state are upstream's capped values and **are not** a solution
    /// of the rate equations — treat the point as failed, or re-run the step
    /// with a smaller `dt` and see whether the cap still binds.
    Saturated,
}

/// The result of integrating one Lemaitre-Chaboche step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LemaitreChabocheIncrement {
    /// Nominal (damaged) Cauchy stress at the end of the step \[Pa\]. This is
    /// what a load cell would read; the material's own sections feel
    /// `stress/(1-D)`.
    pub stress: SymmTensor,
    /// Effective von Mises equivalent stress `sigma_eq/(1-D)` \[Pa\] — the
    /// quantity the flow rule compares against `SY`.
    pub effective_equivalent_stress: f64,
    /// Damage-equivalent stress `chi` \[Pa\] at the end of the step.
    pub damage_equivalent_stress: f64,
    /// Updated internal state.
    pub state: LemaitreChabocheState,
    /// How the step ended.
    pub outcome: DamageOutcome,
    /// Whether upstream's overflow guard fired — that is, whether the power-law
    /// flow rate exceeded `0.1/dt` and was replaced by its tangent
    /// linearisation (`nmvecd.F90`, `etatf(2) = 'TANGENT'`, alarm
    /// `ALGORITH8_66`).
    ///
    /// When true, the returned increment comes from a **linearised** flow rule,
    /// not the power law. Upstream warns; so does this flag.
    pub rate_linearised: bool,
    /// Local iterations used by the outer damage solve.
    pub damage_iterations: usize,
    /// Local iterations used by the innermost hardening solve at the converged
    /// damage.
    pub flow_iterations: usize,
}

/// Lemaitre-Chaboche viscoplasticity coupled to isotropic damage.
///
/// # The model
///
/// Two coupled scalar rate equations, listed under
/// [`LemaitreChabocheParameters`], driving a von Mises flow rule written on the
/// **effective** stress `sigma/(1-D)`. Damage enters twice: it shrinks the
/// elastic stiffness (`sigma = (1-D) C : (eps - eps_vp)`), and it accelerates
/// the flow (`dp = dr/(1-D)`). The result is a creep curve with the
/// characteristic three stages — a decaying primary transient from the `r^(1/m)`
/// hardening, a quasi-steady secondary stage, and a tertiary runaway as `D`
/// grows and feeds back on itself.
///
/// # ASTER names and upstream provenance
///
/// - `VENDOCHAB` (`num_lc = 31`, 10 state variables), keywords `ELAS` +
///   `VENDOCHAB` + `LEMAITRE`, `algo_inte` `NEWTON` or `RUNGE_KUTTA`.
/// - `VISC_ENDO_LEMA` (`num_lc = 31`, 10 state variables), keywords `ELAS` +
///   `VISC_ENDO` + `LEMAITRE`, `algo_inte` `SECANTE`, `BRENT` or `DEKKER`.
///
/// Legacy symbols: `lc0031`, `nmveei`, `nmvecd`, `nmvexi` (implicit path);
/// `nmvprk`, `rkdvec` (Runge-Kutta path); `nmvend`, `nmfend`, `nmfedd`
/// (`VISC_ENDO_LEMA` reduced path). Documentation reference: R5.03.15.
///
/// # Which upstream path this port follows, and why it matters
///
/// Upstream has three integrators for one law, and they do not agree. This port
/// takes the **implicit backward-Euler discretisation** of the `NEWTON` path
/// with the **rate equations of the Runge-Kutta path**, for two reasons set out
/// in full in the module documentation:
///
/// 1. `nmvexi.F90` reads `ALPHA_D` and `BETA_D` from the material slots
///    `vecmat.F90` fills with `UN_SUR_M` and `UN_SUR_K`. `rkdvec.F90` reads the
///    correct slots. The port uses the parameters as declared.
/// 2. `nmvecd.F90` grows damage on elastic steps; `rkdvec.F90` and
///    `nmvend.F90` gate damage on the plasticity criterion. The port gates.
///
/// Both discrepancies are pinned by tests in this module's test file rather
/// than being corrected silently.
///
/// # Enum dispatch
///
/// The two variants differ only in how the damage driver `chi` is built, which
/// [`Self::damage_equivalent_stress`] shows in one place. Enum, not trait
/// objects, per the workspace rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LemaitreChabocheLaw {
    /// `VENDOCHAB` — the full multiaxial damage driver.
    ///
    /// `chi = ALPHA_D J0(sigma) + BETA_D tr(sigma) + (1 - ALPHA_D - BETA_D) sigma_eq(sigma)`
    ///
    /// evaluated on the **nominal** stress, with the closure exponent `K_D`
    /// active. Upstream: `nmvexi.F90` / `rkdvec.F90`.
    Vendochab(LemaitreChabocheParameters),

    /// `VISC_ENDO_LEMA` — the reduced driver: `chi = sigma_eq/(1-D)`, the
    /// **effective** equivalent stress, with `ALPHA_D`, `BETA_D` and `K_D` all
    /// absent from the material keyword block and therefore ignored.
    ///
    /// Upstream: `nmfend.F90`, where the damage increment is
    /// `dD = dt (sigma_eq/((1-D) A_D))^R_D`. Note this is *not* `VENDOCHAB`
    /// with the weights zeroed — the two differ by a factor `(1-D)^R_D`,
    /// because `VENDOCHAB` drives damage with the nominal stress and
    /// `VISC_ENDO_LEMA` with the effective one.
    ViscEndoLema(LemaitreChabocheParameters),
}

impl LemaitreChabocheLaw {
    /// The upstream ASTER behaviour name.
    #[must_use]
    pub const fn aster_name(self) -> &'static str {
        match self {
            Self::Vendochab(_) => "VENDOCHAB",
            Self::ViscEndoLema(_) => "VISC_ENDO_LEMA",
        }
    }

    /// The catalogue entry this law corresponds to.
    #[must_use]
    pub const fn behaviour(self) -> AsterBehaviour {
        match self {
            Self::Vendochab(_) => AsterBehaviour::Vendochab,
            Self::ViscEndoLema(_) => AsterBehaviour::ViscEndoLema,
        }
    }

    /// The material parameters, whichever variant this is.
    #[must_use]
    pub const fn parameters(self) -> LemaitreChabocheParameters {
        match self {
            Self::Vendochab(p) | Self::ViscEndoLema(p) => p,
        }
    }

    /// Damage-equivalent stress `chi` \[Pa\] — the scalar that drives damage.
    ///
    /// # Arguments
    ///
    /// - `stress` — the **nominal** (damaged) Cauchy stress \[Pa\].
    /// - `damage` — the current damage `D` \[-\], in `[0, 1)`.
    ///
    /// # What the two variants compute
    ///
    /// [`Self::Vendochab`] returns the multiaxial combination
    /// `ALPHA_D J0 + BETA_D tr + (1 - ALPHA_D - BETA_D) sigma_eq`, all on the
    /// nominal stress. The weights sum to one by construction, so a uniaxial
    /// tensile state gives `chi = sigma` exactly whatever the split — a useful
    /// self-check, and the reason the weights are written this way rather than
    /// as three free coefficients.
    ///
    /// [`Self::ViscEndoLema`] returns `sigma_eq/(1-D)`.
    ///
    /// Upstream skips the eigenvalue solve when `ALPHA_D <= 1e-15`
    /// (`rkdvec.F90`), which this reproduces — not as an optimisation but
    /// because it is the branch upstream takes.
    #[must_use]
    pub fn damage_equivalent_stress(self, stress: SymmTensor, damage: f64) -> f64 {
        match self {
            Self::Vendochab(p) => {
                let j0 = if p.principal_weight <= 1.0e-15 {
                    0.0
                } else {
                    max_principal_stress(stress)
                };
                let j1 = stress.tr();
                let j2 = equivalent_stress(stress);
                p.principal_weight * j0
                    + p.trace_weight * j1
                    + (1.0 - p.principal_weight - p.trace_weight) * j2
            }
            Self::ViscEndoLema(_) => {
                let one_minus_d = (1.0 - damage).max(1.0 - LEMAITRE_CHABOCHE_DAMAGE_MAX);
                equivalent_stress(stress) / one_minus_d
            }
        }
    }

    /// Damage rate `dD/dt` \[1/s\] at a given driver and damage.
    ///
    /// `dD/dt = (chi/A_D)^R_D * (1-D)^(-K_D)`, with `chi` clamped at zero —
    /// upstream's `max(0, chi/A_D)` in `rkdvec.F90`, which matters because a
    /// compressive multiaxial state can drive `chi` negative and a negative
    /// base raised to a fractional power is not a number.
    ///
    /// [`Self::ViscEndoLema`] has no closure term (`K_D` absent from its
    /// keyword block), so the second factor is one.
    #[must_use]
    pub fn damage_rate(self, chi: f64, damage: f64) -> f64 {
        let p = self.parameters();
        let base = (chi / p.damage_strength).max(0.0);
        if base == 0.0 {
            return 0.0;
        }
        let growth = base.powf(p.damage_exponent);
        match self {
            Self::ViscEndoLema(_) => growth,
            Self::Vendochab(_) => {
                let one_minus_d = (1.0 - damage).max(1.0 - LEMAITRE_CHABOCHE_DAMAGE_MAX);
                growth * one_minus_d.powf(-p.damage_closure_exponent)
            }
        }
    }

    /// Viscoplastic hardening rate `dr/dt` \[1/s\], with upstream's overflow
    /// guard.
    ///
    /// # Arguments
    ///
    /// - `effective_equivalent_stress` — `sigma_eq/(1-D)` \[Pa\].
    /// - `hardening_variable` — `r` \[-\] at the end of the step.
    /// - `dt` — timestep \[s\], strictly positive.
    ///
    /// # The guard, and why it changes answers
    ///
    /// The bare law is `dr/dt = ((sigma_eff - SY)/(K r^(1/m)))^n`. With `n`
    /// around 5 and a stress a decade above `K`, that is `1e5` per second —
    /// enough to overflow, and certainly enough to wreck a Newton step.
    /// Upstream (`nmvecd.F90`) therefore replaces the power law by its **tangent
    /// linearisation** at `dr/dt = 0.1/dt` whenever the power law would exceed
    /// that value, i.e. whenever the step would accumulate more than 0.1 of
    /// hardening variable:
    ///
    /// `rate ~ n R*^((n-1)/n) x + (1-n) R*`, with
    /// `x = (sigma_eff - SY)/(K r^(1/m))` and `R* = 0.1/dt`.
    ///
    /// This is not a cosmetic clamp: past the switch the returned rate is a
    /// *different function*, linear in the overstress rather than a power of
    /// it, and upstream raises alarm `ALGORITH8_66` if the converged solution
    /// used it. The second return value reports the same thing.
    ///
    /// Returns `(rate, linearised)`. Zero rate at or below the yield stress.
    #[must_use]
    pub fn hardening_rate(
        self,
        effective_equivalent_stress: f64,
        hardening_variable: f64,
        dt: f64,
    ) -> (f64, bool) {
        let p = self.parameters();
        let overstress = effective_equivalent_stress - p.yield_stress;
        if !(overstress > 0.0) || !(dt > 0.0) {
            return (0.0, false);
        }

        // sc = K r^(1/m). Upstream floors `r` at `epsiec = 1e-8`, not at
        // `r8miem()` — see [`HARDENING_FLOOR`], where the difference and its
        // consequences are spelled out.
        let r = if hardening_variable <= HARDENING_FLOOR {
            HARDENING_FLOOR
        } else {
            hardening_variable
        };
        let sc = p.k * r.powf(1.0 / p.m);
        let inv_sc = if sc <= R8MIEM { 1.0 / R8MIEM } else { 1.0 / sc };

        let arg = p.n * (overstress.ln() - sc.ln());
        let switch = (0.1 / dt).ln();
        if arg > switch {
            // Tangent linearisation at rate* = 0.1/dt (upstream `etatf(2) = 'TANGENT'`).
            let rate_star = 0.1 / dt;
            let c1 = p.n * rate_star.powf((p.n - 1.0) / p.n);
            let c0 = (1.0 - p.n) * rate_star;
            (c1 * overstress * inv_sc + c0, true)
        } else {
            (arg.exp(), false)
        }
    }

    /// Integrate one timestep of the coupled damage-viscoplasticity system.
    ///
    /// # The reduction, and why it is two scalars rather than eight
    ///
    /// Upstream solves an 8x8 system in the six stress components plus the two
    /// rates. It does not have to. Damage here is isotropic and viscoplastic
    /// flow is von Mises, so the stress deviator keeps its trial direction and
    /// only shrinks, and the mean stress is unaffected by flow. Writing `mu`
    /// for the shear modulus, `dr` for the hardening increment, `D` for the
    /// end-of-step damage and `sigma_eff_tr` for the von Mises equivalent of
    /// the *effective* elastic predictor `2 mu (dev eps - eps_vp)`:
    ///
    /// `sigma_eff = sigma_eff_tr - 3 mu dr/(1-D)`
    ///
    /// which closes the system with the two rate equations. Two unknowns, two
    /// equations, and the tensorial part is recovered afterwards by scaling the
    /// trial deviator.
    ///
    /// The solve is nested rather than simultaneous: an outer bracketed
    /// [`brent`] on `D` over `[D_old, 0.99]`, with an inner safeguarded
    /// [`newton_safeguarded`] on `dr` over
    /// `[0, (1-D)(sigma_eff_tr - SY)/(3 mu)]` for each trial damage. Both
    /// brackets are guaranteed by construction — the inner residual is
    /// `-dt rate <= 0` at zero increment and `+dr > 0` where the overstress is
    /// exhausted — which is what makes the whole thing robust without a
    /// Jacobian of the coupled system.
    ///
    /// # Arguments
    ///
    /// - `elastic` — isotropic moduli.
    /// - `state` — internal state at the **start** of the step.
    /// - `total_strain` — total mechanical strain at the **end** of the step
    ///   \[-\]; any thermal part must already be subtracted, as upstream does
    ///   with `epsth`.
    /// - `dt` — timestep \[s\]. Zero is legal and returns an elastic step.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for invalid moduli, parameters, a negative
    /// timestep, or a starting damage outside `[0, 1]`;
    /// [`OffbeatError::ConstitutiveNotConverged`] if either local solve
    /// exhausts its iteration budget. Damage saturation is **not** an error —
    /// it is reported as [`DamageOutcome::Saturated`].
    pub fn integrate(
        self,
        elastic: IsotropicElasticity,
        state: LemaitreChabocheState,
        total_strain: SymmTensor,
        dt: f64,
    ) -> Result<LemaitreChabocheIncrement> {
        elastic.validate()?;
        let params = self.parameters();
        params.validate()?;
        if dt < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "timestep",
                value: dt,
                unit: "s",
                reason: "must not be negative",
            });
        }
        if !(0.0..=1.0).contains(&state.damage) {
            return Err(OffbeatError::Unphysical {
                quantity: "damage",
                value: state.damage,
                unit: "-",
                reason: "must lie in [0, 1]",
            });
        }

        // Upstream `nmveei.F90`: a state arriving at exactly D = 1 is pulled
        // back to the ceiling so the effective stress stays finite.
        let d_old = if state.damage >= 1.0 {
            LEMAITRE_CHABOCHE_DAMAGE_MAX
        } else {
            state.damage
        };

        let two_mu = 2.0 * elastic.shear_modulus;
        let three_mu = 3.0 * elastic.shear_modulus;

        // Effective (undamaged) elastic predictor. Viscoplastic flow is
        // deviatoric, so the effective mean stress is unaffected by it.
        let elastic_dev_strain = deviator(total_strain) - state.viscoplastic_strain;
        let s_eff_trial = scaled(elastic_dev_strain, two_mu);
        let eq_eff_trial = von_mises_of_deviator(s_eff_trial);
        let mean_eff = elastic.bulk_modulus * total_strain.tr();

        // Assemble the answer for a given (damage, hardening increment).
        let assemble = |damage: f64, dr: f64| -> (SymmTensor, f64, SymmTensor, f64) {
            let one_minus_d = 1.0 - damage;
            let dp = if one_minus_d > 0.0 {
                dr / one_minus_d
            } else {
                0.0
            };
            let eq_eff = (eq_eff_trial - three_mu * dp).max(0.0);
            let shrink = if eq_eff_trial > 0.0 {
                eq_eff / eq_eff_trial
            } else {
                0.0
            };
            let s_eff = scaled(s_eff_trial, shrink);
            let sigma = scaled(from_dev_and_mean(s_eff, mean_eff), one_minus_d);
            // Flow direction is the trial deviator's, normalised so its von
            // Mises equivalent is one: n = (3/2) s / sigma_eq.
            let dstrain = if eq_eff_trial > 0.0 {
                scaled(s_eff_trial, 1.5 * dp / eq_eff_trial)
            } else {
                SymmTensor::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
            };
            (sigma, eq_eff, dstrain, dp)
        };

        // Elastic step: no flow, and (per the Runge-Kutta semantics) no damage.
        if dt == 0.0 || eq_eff_trial <= params.yield_stress {
            let (sigma, eq_eff, _, _) = assemble(d_old, 0.0);
            let chi = self.damage_equivalent_stress(sigma, d_old);
            return Ok(LemaitreChabocheIncrement {
                stress: sigma,
                effective_equivalent_stress: eq_eff,
                damage_equivalent_stress: chi,
                state: LemaitreChabocheState {
                    damage: d_old,
                    ..state
                },
                outcome: DamageOutcome::Elastic,
                rate_linearised: false,
                damage_iterations: 0,
                flow_iterations: 0,
            });
        }

        // Inner solve: the hardening increment at a fixed trial damage.
        let solve_hardening = |damage: f64| -> Result<(f64, bool, usize)> {
            let one_minus_d = 1.0 - damage;
            let upper = one_minus_d * (eq_eff_trial - params.yield_stress) / three_mu;
            if !(upper > 0.0) {
                return Ok((0.0, false, 0));
            }
            let residual = |dr: f64| {
                let dp = dr / one_minus_d;
                let eq_eff = (eq_eff_trial - three_mu * dp).max(params.yield_stress);
                let (rate, _) = self.hardening_rate(eq_eff, state.hardening_variable + dr, dt);
                let value = dr - dt * rate;
                if value.is_finite() {
                    value
                } else {
                    // Only reachable at dr -> 0 from a pristine hardening
                    // variable, where the rate is genuinely unbounded. The sign
                    // is what the bracket needs; the magnitude is not.
                    -1.0e300
                }
            };
            let derivative = |dr: f64| {
                let h = 1.0e-8 * upper.max(1.0e-12);
                // Numerical slope: the analytic tangent of the guarded rate is
                // branch-dependent, and the bisection safeguard makes an
                // inexact one safe.
                let slope = (residual(dr + h) - residual(dr)) / h;
                if slope.is_finite() && slope != 0.0 {
                    slope
                } else {
                    1.0
                }
            };
            let solution = newton_safeguarded(
                residual,
                derivative,
                (0.0, upper),
                &SolverControl {
                    max_iter: 200,
                    residual_tol: 1.0e-14 * upper.max(1.0e-12),
                    step_tol: 1.0e-18,
                },
            )?;
            let dr = solution.root.clamp(0.0, upper);
            let dp = dr / one_minus_d;
            let eq_eff = (eq_eff_trial - three_mu * dp).max(params.yield_stress);
            let (_, linearised) = self.hardening_rate(eq_eff, state.hardening_variable + dr, dt);
            Ok((dr, linearised, solution.iterations))
        };

        // Outer solve: the damage equation.
        let damage_residual = |damage: f64| -> f64 {
            let dr = match solve_hardening(damage) {
                Ok((dr, _, _)) => dr,
                Err(_) => return f64::NAN,
            };
            let (sigma, _, _, _) = assemble(damage, dr);
            let chi = self.damage_equivalent_stress(sigma, damage);
            damage - d_old - dt * self.damage_rate(chi, damage)
        };

        let mut outcome = DamageOutcome::Converged;
        let mut damage_iterations = 0;

        // Bracket the *first* root above `d_old`, scanning upward.
        //
        // The residual `D - d_old - dt r(D)` is **negative at both ends** for
        // any realistic step, and that is not a sign of saturation. At
        // `D = d_old` it is `-dt r(d_old) < 0` because the rate is positive; at
        // the ceiling it is hugely negative because `r ∝ (1-D)^(-k)` diverges
        // there, with `k ~ 14.5` giving a factor of order `1e29`. In between,
        // the linear `D` term outruns the rate and the residual rises through
        // zero — that crossing is the physical root.
        //
        // An earlier version tested `damage_residual(ceiling) < 0` and declared
        // saturation. Because that test is satisfied for essentially every
        // timestep, it fired on the very first step of every problem: measured
        // on `ssnv126a`, damage pinned at the ceiling even for `dt = 1e-10` s,
        // and only below about `1e-20` s did the law return anything else.
        // Saturation must instead mean *no crossing exists*, which is what the
        // scan below actually determines.
        let scan_upper = LEMAITRE_CHABOCHE_DAMAGE_MAX;
        let mut bracket: Option<(f64, f64)> = None;
        if d_old < scan_upper {
            let mut lower = d_old;
            let mut f_lower = damage_residual(lower);
            // Geometric ladder: the root sits very close to `d_old` for a small
            // step and further out for a large one, so sampling has to be dense
            // near the start and coarse further up.
            let samples = 200;
            for i in 1..=samples {
                let f = f64::from(i) / f64::from(samples);
                let upper = d_old + (scan_upper - d_old) * f * f * f;
                let f_upper = damage_residual(upper);
                if f_lower.is_finite() && f_upper.is_finite() && f_lower * f_upper <= 0.0 {
                    bracket = Some((lower, upper));
                    break;
                }
                lower = upper;
                f_lower = f_upper;
            }
        }

        let damage = if d_old >= LEMAITRE_CHABOCHE_DAMAGE_MAX {
            outcome = DamageOutcome::Saturated;
            LEMAITRE_CHABOCHE_DAMAGE_MAX
        } else if let Some(bracket) = bracket {
            let solution = brent(
                damage_residual,
                bracket,
                &SolverControl {
                    max_iter: 200,
                    residual_tol: 1.0e-14,
                    step_tol: 1.0e-16,
                },
            )?;
            damage_iterations = solution.iterations;
            solution.root
        } else {
            // No crossing anywhere below the ceiling: the step genuinely cannot
            // be completed without passing it. Upstream's `dammax` branch.
            outcome = DamageOutcome::Saturated;
            LEMAITRE_CHABOCHE_DAMAGE_MAX
        };

        let (dr, rate_linearised, flow_iterations) = solve_hardening(damage)?;
        let (sigma, eq_eff, dstrain, dp) = assemble(damage, dr);
        let chi = self.damage_equivalent_stress(sigma, damage);

        Ok(LemaitreChabocheIncrement {
            stress: sigma,
            effective_equivalent_stress: eq_eff,
            damage_equivalent_stress: chi,
            state: LemaitreChabocheState {
                viscoplastic_strain: state.viscoplastic_strain + dstrain,
                equivalent_viscoplastic_strain: state.equivalent_viscoplastic_strain + dp,
                hardening_variable: state.hardening_variable + dr,
                damage,
            },
            outcome,
            rate_linearised,
            damage_iterations,
            flow_iterations,
        })
    }
}

// ===========================================================================
// ROUSS_PR / ROUSS_VISC — Rousselier porous plasticity
// ===========================================================================

/// Material parameters of the Rousselier porous-plastic law.
///
/// # Upstream keyword block
///
/// `ROUSSELIER`, read by `rslmat.F90` (`ROUSS_PR`) and `rsvmat.F90`
/// (`ROUSS_VISC`) in the order `D`, `SIGM_1`, `PORO_INIT`, `PORO_CRIT`,
/// `PORO_ACCE`, `PORO_LIMI`, `D_SIGM_EPSI_NORM`, then `AN` and `BETA` for
/// `ROUSS_PR` or `BETA` alone for `ROUSS_VISC` (which forbids nucleation).
///
/// # Units
///
/// `sigma_1` in pascal \[Pa\]; every porosity and every coefficient
/// dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RousselierParameters {
    /// `D` \[-\] — the amplitude of the void term in the yield function.
    /// Typically around 2 for structural steels. Strictly positive.
    pub d: f64,
    /// `SIGM_1` \[Pa\] — the stress scale of the exponential. Sets how strongly
    /// hydrostatic tension softens the material: the void term grows as
    /// `exp(sigma_m/SIGM_1)`, so a small `SIGM_1` makes the law violently
    /// triaxiality-sensitive. Strictly positive.
    pub sigma_1: f64,
    /// `PORO_INIT` \[-\] — the initial void volume fraction `f0`, the reference
    /// against which the reduced stress is defined. Typically `1e-4` to `1e-3`.
    pub initial_porosity: f64,
    /// `PORO_CRIT` \[-\] — porosity above which growth is artificially
    /// accelerated, standing in for coalescence.
    pub critical_porosity: f64,
    /// `PORO_ACCE` \[-\] — the acceleration factor applied past `PORO_CRIT`.
    ///
    /// Upstream *divides* the volumetric plastic increment by it in the
    /// mean-stress update while keeping the same porosity increment, so a
    /// larger value means the porosity reaches a given level for less
    /// hydrostatic relaxation. One disables acceleration. Strictly positive.
    pub acceleration: f64,
    /// `PORO_LIMI` \[-\] — the porosity at which the point is declared broken
    /// and its stress ramped to zero.
    pub limit_porosity: f64,
    /// `D_SIGM_EPSI_NORM` \[-\] — the rate at which a broken point sheds its
    /// stress, as a fraction of Young's modulus per unit equivalent strain
    /// increment.
    pub broken_unloading_slope: f64,
    /// `AN` \[-\] — strain-controlled nucleation rate, `f_total = f + AN p`.
    ///
    /// Only `ROUSS_PR` activates this; `lcrous.F90` forces it to zero for
    /// `ROUSS_VISC`, and [`RousselierLaw::nucleation_rate`] does the same.
    pub nucleation_rate: f64,
    /// `BETA` \[-\] — the split of plastic work between dissipated heat and
    /// energy stored in the microstructure. Enters the dissipation bookkeeping
    /// only, never the stress.
    pub stored_energy_fraction: f64,
}

impl RousselierParameters {
    /// Reject parameters that make the law undefined.
    fn validate(self) -> Result<()> {
        if !(self.d > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "Rousselier D",
                value: self.d,
                unit: "-",
                reason: "must be strictly positive",
            });
        }
        if !(self.sigma_1 > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "Rousselier SIGM_1",
                value: self.sigma_1,
                unit: "Pa",
                reason: "must be strictly positive; the void term divides by it",
            });
        }
        if !(self.acceleration > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "Rousselier PORO_ACCE",
                value: self.acceleration,
                unit: "-",
                reason: "must be strictly positive; the porosity rate divides by it",
            });
        }
        if !(0.0..1.0).contains(&self.initial_porosity) {
            return Err(OffbeatError::Unphysical {
                quantity: "Rousselier PORO_INIT",
                value: self.initial_porosity,
                unit: "-",
                reason: "must lie in [0, 1)",
            });
        }
        Ok(())
    }
}

/// The `VISC_SINH` viscous overstress that turns `ROUSS_PR` into `ROUSS_VISC`.
///
/// # The model
///
/// The yield function gains a rate-dependent term
///
/// `Phi_visc = Phi - SIGM_0 asinh( (dp/(dt EPSI_0))^(1/M) )`
///
/// so the material can sustain a stress above its rate-independent yield
/// surface, by an amount that grows logarithmically with the plastic strain
/// rate. The inverse hyperbolic sine is the classical high-stress creep form:
/// linear in the rate at low rates and logarithmic at high ones, which avoids
/// the unbounded stress a pure power law gives at high rate.
///
/// Upstream: `rslphi.F90`, keyword block `VISC_SINH` with `SIGM_0`, `EPSI_0`,
/// `M`.
///
/// # Units
///
/// `sigma_0` in pascal \[Pa\], `reference_strain_rate` in per second \[1/s\],
/// `exponent` dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViscousSinhParameters {
    /// `SIGM_0` \[Pa\] — the amplitude of the viscous overstress.
    pub sigma_0: f64,
    /// `EPSI_0` \[1/s\] — the reference plastic strain rate. Strictly positive.
    pub reference_strain_rate: f64,
    /// `M` \[-\] — the rate exponent. Strictly positive; larger means a weaker
    /// rate dependence.
    pub exponent: f64,
}

/// Internal state of a Rousselier point.
///
/// Mirrors upstream's five internal variables `EPSPEQ`, `POROSITE`, `DISSIP`,
/// `EBLOC`, `INDIPLAS`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RousselierState {
    /// Accumulated equivalent plastic strain `p` \[-\], upstream `EPSPEQ`.
    pub equivalent_plastic_strain: f64,
    /// Void volume fraction `f` \[-\], upstream `POROSITE`. Starts at
    /// [`RousselierParameters::initial_porosity`].
    pub porosity: f64,
    /// Plastic dissipation rate \[W/m^3\], upstream `DISSIP`. Bookkeeping only
    /// — it never re-enters the stress update.
    pub dissipation: f64,
    /// Energy stored in the microstructure \[J/m^3\], upstream `EBLOC`. Also
    /// bookkeeping only.
    pub blocked_energy: f64,
}

impl RousselierState {
    /// The initial state for a given material: undeformed, at the material's
    /// initial porosity.
    #[must_use]
    pub fn initial(params: RousselierParameters) -> Self {
        Self {
            equivalent_plastic_strain: 0.0,
            porosity: params.initial_porosity,
            dissipation: 0.0,
            blocked_energy: 0.0,
        }
    }
}

/// How a Rousselier step ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RousselierOutcome {
    /// The stress stayed inside the yield surface; no plastic flow, no void
    /// growth.
    Elastic,
    /// Plastic, but with a compressive mean stress or a zero starting porosity,
    /// so the yield surface degenerated to von Mises and the coupled solve
    /// reduced to a scalar return at frozen porosity.
    ///
    /// Upstream takes this branch explicitly (`lcrous.F90`, the `df2 < 0` and
    /// `fi == 0` tests) and this port reproduces it, because the coupled
    /// bracket in the porosity increment is genuinely empty there — voids
    /// cannot grow under compression.
    VonMises,
    /// The coupled `(dp, df)` system was solved.
    Coupled,
    /// The point was already broken on entry (`f_total >= PORO_LIMI`): the
    /// stress is being ramped to zero over a strain scale set by
    /// `D_SIGM_EPSI_NORM`, the porosity is pinned at one, and no constitutive
    /// solve was attempted. Upstream's "materiau casse" branch.
    Broken,
}

/// The result of integrating one Rousselier step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RousselierIncrement {
    /// Cauchy stress at the end of the step \[Pa\].
    pub stress: SymmTensor,
    /// Reduced equivalent stress `sigma_eq/rho` \[Pa\] at the end of the step,
    /// where `rho = (1 - f_total)/(1 - f0)` is the section-loss factor. This is
    /// the quantity the yield function compares against `R(p)`.
    pub reduced_equivalent_stress: f64,
    /// Reduced mean stress `sigma_m/rho` \[Pa\] at the end of the step.
    pub reduced_mean_stress: f64,
    /// Equivalent plastic strain increment `dp` \[-\] over the step.
    pub plastic_strain_increment: f64,
    /// Porosity increment `df` \[-\] over the step.
    pub porosity_increment: f64,
    /// Updated internal state.
    pub state: RousselierState,
    /// How the step ended.
    pub outcome: RousselierOutcome,
    /// Local iterations used.
    pub iterations: usize,
}

/// Rousselier's porous-plastic law for ductile rupture.
///
/// # The model
///
/// Rousselier's yield function, written on the **reduced stress**
/// `sigma_tilde = sigma/rho` with `rho = (1 - f_total)/(1 - f0)`:
///
/// `Phi = sigma_tilde_eq - R(p) + D SIGM_1 f exp(sigma_tilde_m / SIGM_1)`
///
/// The third term is what makes this a damage law rather than a plasticity law.
/// It is positive, so it *shrinks* the elastic domain; it is proportional to
/// the porosity, so an initially near-dense material behaves like von Mises and
/// progressively softens as voids grow; and it is exponential in the mean
/// stress, so the softening is enormously more aggressive under triaxial
/// tension than in shear. That last property is the model's whole reason for
/// existing: it is why a notched tensile bar fails at a fraction of the strain
/// of a smooth one, and why a crack tip — where triaxiality is highest — is
/// where ductile tearing initiates.
///
/// Void growth follows from normality. The mean-stress term contributes a
/// volumetric component to the plastic flow, and mass conservation of the
/// matrix turns that into
///
/// `df = f (1 - f) D exp(sigma_tilde_m/SIGM_1) PORO_ACCE dp`
///
/// which is upstream's `dp = df/(f (1-f) D exp(...) acc)` inverted
/// (`rslphi.F90`). The coupling runs both ways — porosity changes the yield
/// surface, the yield surface changes the plastic increment, the plastic
/// increment changes the porosity — which is why the local solve is on `df`
/// with `dp` eliminated, rather than the scalar return of a von Mises law.
///
/// # ASTER names and upstream provenance
///
/// - `ROUSS_PR` (`num_lc = 30`, 5 state variables), keywords `ELAS` +
///   `ROUSSELIER`, `algo_inte` `NEWTON_1D`.
/// - `ROUSS_VISC` (`num_lc = 30`, 5 state variables), keywords `ELAS` +
///   `ROUSSELIER` + `VISC_SINH`, `algo_inte` `NEWTON_1D`.
///
/// Legacy symbols: `lc0030`, `plasti`, `lcrous`, `rslphi`, `rslcvx`.
/// Documentation reference: R5.03.06.
///
/// # What this port does and does not reproduce
///
/// **Reproduced:** the reduced-stress formulation, the theta-method (upstream
/// recommends `PARM_THETA = 0.5`), the acceleration past `PORO_CRIT`,
/// strain-controlled nucleation `AN` for `ROUSS_PR`, the `VISC_SINH` overstress
/// for `ROUSS_VISC`, the broken-point stress ramp past `PORO_LIMI`, the
/// exponent guards at `sigma_m/SIGM_1 > 200` and `< -50`, and the dissipation /
/// stored-energy bookkeeping.
///
/// **Not reproduced:** upstream's hand-rolled Newton-with-chord-fallback on the
/// porosity increment, replaced by [`brent`] on the same bracket upstream
/// computes — same equation, same bracket, a solver with a convergence
/// guarantee instead of one without. The consistent tangent operator
/// (`lcotan`, `rsljpl`) is not ported; upstream itself defaults this law's
/// tangent to `PERTURBATION`.
///
/// **Deliberate addition:** an explicit elastic test. `lcrous.F90` assumes its
/// caller (`plasti.F90`, via `rslcvx.F90`) has already established that the
/// point is plastic, and its bracket test `phi1 < 0` — which an elastic point
/// satisfies — is reported as "strain increment too large, subdivide". This
/// port evaluates the yield function at zero increment first and returns
/// [`RousselierOutcome::Elastic`]: the same physics, and a far better
/// diagnostic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RousselierLaw {
    /// `ROUSS_PR` — rate-independent, with optional strain-controlled void
    /// nucleation through [`RousselierParameters::nucleation_rate`].
    Plastic(RousselierParameters),
    /// `ROUSS_VISC` — with the `VISC_SINH` viscous overstress. Upstream forces
    /// `AN = 0` for this variant, and so does this port.
    Viscous(RousselierParameters, ViscousSinhParameters),
}

impl RousselierLaw {
    /// The upstream ASTER behaviour name.
    #[must_use]
    pub const fn aster_name(self) -> &'static str {
        match self {
            Self::Plastic(_) => "ROUSS_PR",
            Self::Viscous(_, _) => "ROUSS_VISC",
        }
    }

    /// The catalogue entry this law corresponds to.
    #[must_use]
    pub const fn behaviour(self) -> AsterBehaviour {
        match self {
            Self::Plastic(_) => AsterBehaviour::RoussPr,
            Self::Viscous(_, _) => AsterBehaviour::RoussVisc,
        }
    }

    /// The Rousselier material parameters, whichever variant this is.
    #[must_use]
    pub const fn parameters(self) -> RousselierParameters {
        match self {
            Self::Plastic(p) | Self::Viscous(p, _) => p,
        }
    }

    /// The effective nucleation rate `AN` \[-\]: the declared value for
    /// [`Self::Plastic`], zero for [`Self::Viscous`].
    ///
    /// Upstream sets `ann = 0` unconditionally for `ROUSS_VISC` in
    /// `lcrous.F90`, and the catalogue documentation says so explicitly ("mot
    /// cle non active pour le modele ROUSSELIER et ROUSS_VISC").
    #[must_use]
    pub const fn nucleation_rate(self) -> f64 {
        match self {
            Self::Plastic(p) => p.nucleation_rate,
            Self::Viscous(_, _) => 0.0,
        }
    }

    /// The rate-independent Rousselier yield function \[Pa\].
    ///
    /// `Phi = sigma_tilde_eq - R(p) + D SIGM_1 f exp(sigma_tilde_m/SIGM_1)`
    ///
    /// # Arguments
    ///
    /// - `reduced_equivalent` — `sigma_tilde_eq` \[Pa\].
    /// - `reduced_mean` — `sigma_tilde_m` \[Pa\].
    /// - `porosity` — `f`, including any nucleated part \[-\].
    /// - `flow_stress` — `R(p)` \[Pa\].
    ///
    /// Negative means elastic, zero means on the surface, positive means
    /// inadmissible. Upstream caps the exponent's argument at 200
    /// (`rslcvx.F90`), which this reproduces — without it a moderately triaxial
    /// state overflows before the solver ever sees it.
    #[must_use]
    pub fn yield_function(
        self,
        reduced_equivalent: f64,
        reduced_mean: f64,
        porosity: f64,
        flow_stress: f64,
    ) -> f64 {
        let p = self.parameters();
        let arg = (reduced_mean / p.sigma_1).min(200.0);
        reduced_equivalent - flow_stress + p.d * p.sigma_1 * porosity * arg.exp()
    }

    /// The viscous overstress `SIGM_0 asinh((dp/(dt EPSI_0))^(1/M))` \[Pa\].
    ///
    /// Zero for [`Self::Plastic`], and zero at `dp = 0` for either variant.
    /// Upstream: `rslphi.F90`.
    #[must_use]
    pub fn viscous_overstress(self, plastic_increment: f64, dt: f64) -> f64 {
        match self {
            Self::Plastic(_) => 0.0,
            Self::Viscous(_, v) => {
                if plastic_increment <= 0.0 || dt <= 0.0 || v.reference_strain_rate <= 0.0 {
                    return 0.0;
                }
                let x = plastic_increment / (dt * v.reference_strain_rate);
                let power = x.powf(1.0 / v.exponent);
                v.sigma_0 * (power + (1.0 + power * power).sqrt()).ln()
            }
        }
    }

    /// Integrate one timestep of the coupled plasticity-porosity system.
    ///
    /// # The unknown, and why it is the porosity increment
    ///
    /// The two increments `dp` (equivalent plastic strain) and `df` (porosity)
    /// are tied by the normality condition on the mean-stress term, so only one
    /// is independent. Upstream eliminates `dp` and solves for `df`, which is
    /// the right choice: `df` has a *computable bracket*. The upper end is the
    /// porosity increment at which the reduced mean stress would drive the
    /// exponent's argument to `-50` (a fully relaxed pressure); the lower end
    /// is zero, or the increment that brings the argument down from `+200` when
    /// the elastic predictor is wildly triaxial. Both endpoints come straight
    /// from `lcrous.F90`, and this port keeps them.
    ///
    /// For each trial `df`, the accompanying `dp` follows in closed form (or
    /// from a quadratic when nucleation is active), the reduced stresses follow
    /// from the elastic predictor, and the yield function is evaluated. One
    /// scalar residual, one bracketed [`brent`].
    ///
    /// # Arguments
    ///
    /// - `elastic` — isotropic moduli.
    /// - `hardening` — the matrix flow curve `R(p)`; code_aster supplies this
    ///   as a `TRACTION` curve, see [`IsotropicHardening`].
    /// - `state` — internal state at the **start** of the step.
    /// - `stress_start` — Cauchy stress at the start of the step \[Pa\]. This
    ///   law is formulated incrementally (`DEFORMATION = PETIT`), like upstream.
    /// - `strain_increment` — total strain increment over the step \[-\].
    /// - `dt` — timestep \[s\]. Only [`Self::Viscous`] uses it.
    /// - `theta` — the theta-method parameter in `(0, 1]`. Upstream recommends
    ///   `0.5` for `ROUSS_VISC` ("semi-NEWTON_1D"); `1.0` is fully implicit.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for invalid moduli, parameters, `theta`, or
    /// a negative timestep. [`OffbeatError::ConstitutiveNotConverged`] when the
    /// bracket in `df` does not straddle a root — which upstream treats as
    /// "strain increment too large, subdivide the step" (`irtet = 1`) and which
    /// is the honest report here too, rather than a clamped answer.
    pub fn integrate(
        self,
        elastic: IsotropicElasticity,
        hardening: IsotropicHardening,
        state: RousselierState,
        stress_start: SymmTensor,
        strain_increment: SymmTensor,
        dt: f64,
        theta: f64,
    ) -> Result<RousselierIncrement> {
        elastic.validate()?;
        let mat = self.parameters();
        mat.validate()?;
        if dt < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "timestep",
                value: dt,
                unit: "s",
                reason: "must not be negative",
            });
        }
        if !(theta > 0.0 && theta <= 1.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "theta-method parameter",
                value: theta,
                unit: "-",
                reason: "must lie in (0, 1]",
            });
        }

        let ann = self.nucleation_rate();
        let f_start = state.porosity;
        let p_start = state.equivalent_plastic_strain;
        let f_total_start = f_start + ann * p_start;

        // ---- Broken point: upstream's "materiau casse" branch. -------------
        if f_total_start >= mat.limit_porosity {
            // `lcnrte` is the dual second-invariant norm sqrt(2/3 e:e) of the
            // strain increment; `lcnrts` is sqrt(3/2 s:s) applied to the *full*
            // stress, not its deviator. That is upstream's expression verbatim
            // — it is a magnitude for scaling the unloading, not a von Mises
            // stress, and reproducing it matters because it sets how fast a
            // broken point sheds load.
            let strain_norm = (2.0 / 3.0 * strain_increment.double_inner(strain_increment)).sqrt();
            let stress_norm = (1.5 * stress_start.double_inner(stress_start)).sqrt();
            let drop = mat.broken_unloading_slope * elastic.young() * strain_norm;
            let stress = if drop >= stress_norm || stress_norm == 0.0 {
                SymmTensor::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0)
            } else {
                scaled(stress_start, 1.0 - drop / stress_norm)
            };
            return Ok(RousselierIncrement {
                stress,
                reduced_equivalent_stress: 0.0,
                reduced_mean_stress: 0.0,
                plastic_strain_increment: 0.0,
                porosity_increment: 1.0 - f_start,
                state: RousselierState {
                    equivalent_plastic_strain: p_start,
                    porosity: 1.0,
                    dissipation: 0.0,
                    blocked_energy: 0.0,
                },
                outcome: RousselierOutcome::Broken,
                iterations: 0,
            });
        }

        let two_mu = 2.0 * elastic.shear_modulus;
        let three_mu = 1.5 * two_mu;
        let three_k = 3.0 * elastic.bulk_modulus;

        let deps_mean = strain_increment.tr() / 3.0;
        let deps_dev = deviator(strain_increment);

        // Reduced stress at the start of the step.
        let inv_rho_start = (1.0 - mat.initial_porosity) / (1.0 - f_total_start);
        let sig_reduced_start = scaled(stress_start, inv_rho_start);
        let reduced_mean_start = sig_reduced_start.tr() / 3.0;
        let reduced_dev_start = deviator(sig_reduced_start);

        // Elastic predictor on the reduced deviator, at the theta point and at
        // the end of the step. Upstream keeps both because the yield function is
        // enforced at t + theta dt while the state is updated fully implicitly.
        let predictor_theta = reduced_dev_start + scaled(deps_dev, two_mu * theta);
        let eq_predictor_theta = von_mises_of_deviator(predictor_theta);
        let predictor_end = reduced_dev_start + scaled(deps_dev, two_mu);
        let eq_predictor_end = von_mises_of_deviator(predictor_end);

        let acceleration = if f_total_start >= mat.critical_porosity {
            mat.acceleration
        } else {
            1.0
        };

        // Residual in the porosity increment, and the state it implies.
        // Returns (Phi, dp).
        let evaluate = |df: f64| -> (f64, f64) {
            let f_theta = f_start + theta * df;
            let one_minus_f = 1.0 - f_theta;
            let reduced_mean = reduced_mean_start
                + three_k * theta * (deps_mean - df / (3.0 * one_minus_f * acceleration));
            let arg = (reduced_mean / mat.sigma_1).min(200.0);
            let expo = mat.d * arg.exp();
            let denom = one_minus_f * expo * acceleration;

            let dp = if ann == 0.0 {
                if denom == 0.0 || f_theta == 0.0 {
                    0.0
                } else {
                    df / (f_theta * denom)
                }
            } else {
                // With nucleation the tie between df and dp is quadratic:
                // upstream's coeffa/coeffb/coeffc in `rslphi.F90`.
                let coeff_a = 2.0 * ann * theta;
                let coeff_b = f_theta + ann * p_start;
                let coeff_c = if denom == 0.0 { 0.0 } else { df / denom };
                if coeff_c <= R8MIEM {
                    0.0
                } else {
                    let disc = coeff_b * coeff_b + 2.0 * coeff_a * coeff_c;
                    (-coeff_b + disc.max(0.0).sqrt()) / coeff_a
                }
            };

            let p_theta = p_start + theta * dp;
            let reduced_eq = eq_predictor_theta - three_mu * theta * dp;
            let f_total_theta = f_theta + ann * p_theta;
            let phi = self.yield_function(
                reduced_eq,
                reduced_mean,
                f_total_theta,
                hardening.value(p_theta),
            ) - self.viscous_overstress(dp, dt);
            (phi, dp)
        };

        // ---- Elastic test. -------------------------------------------------
        let (phi_zero, _) = evaluate(0.0);
        if phi_zero <= 0.0 {
            let reduced_mean_end = reduced_mean_start + three_k * deps_mean;
            let stress = scaled(
                from_dev_and_mean(predictor_end, reduced_mean_end),
                1.0 / inv_rho_start,
            );
            return Ok(RousselierIncrement {
                stress,
                reduced_equivalent_stress: eq_predictor_end,
                reduced_mean_stress: reduced_mean_end,
                plastic_strain_increment: 0.0,
                porosity_increment: 0.0,
                state,
                outcome: RousselierOutcome::Elastic,
                iterations: 0,
            });
        }

        // ---- Bracket in df, straight from `lcrous.F90`. --------------------
        let mean_predictor = reduced_mean_start + three_k * deps_mean;
        let argmax = 200.0;
        let argmin = -50.0;
        let df_lo = if mean_predictor / mat.sigma_1 > argmax {
            (1.0 - f_start) * (mean_predictor - argmax * mat.sigma_1)
                / (three_k * theta / (3.0 * acceleration) + mean_predictor - argmax * mat.sigma_1)
        } else {
            0.0
        };
        let mut df_hi = (1.0 - f_start) * (mean_predictor - argmin * mat.sigma_1)
            / (three_k * theta / (3.0 * acceleration) + mean_predictor - argmin * mat.sigma_1);
        if f_start == 0.0 {
            // A pore-free material cannot grow porosity: upstream forces the
            // von Mises branch with the sentinel df2 = -10.
            df_hi = -10.0;
        }

        let mut outcome = RousselierOutcome::Coupled;
        let (dp, df, iterations) = if df_hi < 0.0 || df_hi > 1.0 - f_start {
            // Compression, or no porosity: the porosity bracket is empty and
            // upstream falls back to a von Mises return at frozen porosity.
            outcome = RousselierOutcome::VonMises;
            let reduced_mean = reduced_mean_start + three_k * theta * deps_mean;
            let residual = |dp: f64| {
                let p_theta = p_start + theta * dp;
                let reduced_eq = eq_predictor_theta - three_mu * theta * dp;
                self.yield_function(
                    reduced_eq,
                    reduced_mean,
                    f_start + ann * p_theta,
                    hardening.value(p_theta),
                ) - self.viscous_overstress(dp, dt)
            };
            let upper = residual(0.0).max(0.0) / (three_mu * theta) + 1.0e-30;
            let solution = brent(
                residual,
                (0.0, upper),
                &SolverControl {
                    max_iter: 200,
                    residual_tol: 1.0e-10 * mat.sigma_1,
                    step_tol: 1.0e-18,
                },
            )?;
            (solution.root.max(0.0), 0.0, solution.iterations)
        } else {
            let solution = brent(
                |df| evaluate(df).0,
                (df_lo, df_hi),
                &SolverControl {
                    max_iter: 300,
                    residual_tol: 1.0e-10 * mat.sigma_1,
                    step_tol: 1.0e-18,
                },
            )?;
            let (_, dp) = evaluate(solution.root);
            (dp.max(0.0), solution.root, solution.iterations)
        };

        // ---- End-of-step state and stress (theta = 1 for the update). ------
        let p_end = p_start + dp;
        let f_end = f_start + df;
        let f_total_end = f_end + ann * p_end;
        let rho_end = (1.0 - f_total_end) / (1.0 - mat.initial_porosity);

        let reduced_eq_end = eq_predictor_end - three_mu * dp;
        let reduced_mean_end =
            reduced_mean_start + three_k * (deps_mean - df / (3.0 * (1.0 - f_end) * acceleration));
        let shrink = if eq_predictor_end > 0.0 {
            reduced_eq_end / eq_predictor_end
        } else {
            0.0
        };
        let reduced_stress = from_dev_and_mean(scaled(predictor_end, shrink), reduced_mean_end);
        let stress = scaled(reduced_stress, rho_end);

        // ---- Dissipation bookkeeping (upstream note HT-26/04/027). ---------
        let (dissipation, blocked_energy) = if dt > 0.0 && f_end > 0.0 {
            let sigma_eq = rho_end * reduced_eq_end;
            let term1 = dp / dt * sigma_eq;
            let term2 = reduced_mean_end * rho_end * df / (1.0 - f_end) / acceleration / dt;
            let term3 = rho_end
                * mat.sigma_1
                * (mat.initial_porosity / f_end * rho_end).max(R8MIEM).ln()
                * df
                / dt;
            let ebloc =
                state.blocked_energy + ((1.0 - mat.stored_energy_fraction) * term1 + term3) * dt;
            if ebloc >= 0.0 {
                (mat.stored_energy_fraction * term1 + term2 - term3, ebloc)
            } else {
                (term1 + term2, 0.0)
            }
        } else {
            (state.dissipation, state.blocked_energy)
        };

        Ok(RousselierIncrement {
            stress,
            reduced_equivalent_stress: reduced_eq_end,
            reduced_mean_stress: reduced_mean_end,
            plastic_strain_increment: dp,
            porosity_increment: df,
            state: RousselierState {
                equivalent_plastic_strain: p_end,
                porosity: f_end,
                dissipation,
                blocked_energy,
            },
            outcome,
            iterations,
        })
    }
}

// ===========================================================================
// GTN / VISC_GTN — Gurson-Tvergaard-Needleman
// ===========================================================================

/// Nucleation of new voids, as the sum of upstream's three mechanisms.
///
/// # The three terms
///
/// Upstream's `Nucleation` in `lcgtn_module.F90` adds:
///
/// 1. **Chu-Needleman Gaussian** —
///    `0.5 FN [erf((k - PN)/(sqrt(2) SN)) + erf(PN/(sqrt(2) SN))]`.
///    Second-phase particles decohere over a narrow band of plastic strain
///    centred on `PN`; the cumulative Gaussian is the fraction that has done
///    so. The second `erf` shifts the curve so it starts from zero at zero
///    strain.
/// 2. **Ramp** — `min(C0 (k - KI)/(KF - KI), C0)`, clamped below at zero: a
///    linear onset between two strain thresholds, saturating at `C0`.
/// 3. **Linear tail** — `B0 max(p_cum - EPC, 0)`: unbounded nucleation past a
///    strain threshold.
///
/// # Units
///
/// All porosities and strains dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GtnNucleation {
    /// `NUCL_GAUSS_PORO` (`FN`) \[-\] — the total void fraction available to
    /// Gaussian nucleation. Zero disables the term.
    pub gaussian_porosity: f64,
    /// `NUCL_GAUSS_PLAS` (`PN`) \[-\] — the mean nucleation strain. Upstream
    /// defaults to 0.1.
    pub gaussian_mean_strain: f64,
    /// `NUCL_GAUSS_DEV` (`SN`) \[-\] — the standard deviation. Upstream
    /// defaults to 0.05. Must be strictly positive when
    /// `gaussian_porosity > 0`.
    pub gaussian_std_dev: f64,
    /// `NUCL_CRAN_PORO` (`C0`) \[-\] — the saturation of the ramp term.
    pub ramp_porosity: f64,
    /// `NUCL_CRAN_INIT` (`KI`) \[-\] — where the ramp starts. Upstream default
    /// 0.05.
    pub ramp_start: f64,
    /// `NUCL_CRAN_FIN` (`KF`) \[-\] — where the ramp saturates. Upstream
    /// default 0.15. Must exceed `ramp_start` when `ramp_porosity > 0`.
    pub ramp_end: f64,
    /// `NUCL_EPSI_PENTE` (`B0`) \[-\] — slope of the linear tail.
    pub linear_slope: f64,
    /// `NUCL_EPSI_INIT` (`EPC`) \[-\] — where the linear tail starts. Upstream
    /// default 0.8.
    pub linear_start: f64,
}

impl GtnNucleation {
    /// No nucleation at all — every mechanism switched off.
    ///
    /// The right default when the only voids of interest are the ones the
    /// material started with. The thresholds keep upstream's defaults so that
    /// switching a mechanism on later needs one field, not four.
    #[must_use]
    pub fn none() -> Self {
        Self {
            gaussian_porosity: 0.0,
            gaussian_mean_strain: 0.1,
            gaussian_std_dev: 0.05,
            ramp_porosity: 0.0,
            ramp_start: 0.05,
            ramp_end: 0.15,
            linear_slope: 0.0,
            linear_start: 0.8,
        }
    }

    /// Nucleated void volume fraction \[-\] at hardening variable `kappa` and
    /// cumulated plastic strain `cumulated_plastic_strain`, both dimensionless
    /// and non-negative.
    ///
    /// Monotone non-decreasing in both arguments — voids nucleate, they do not
    /// un-nucleate.
    #[must_use]
    pub fn porosity(self, kappa: f64, cumulated_plastic_strain: f64) -> f64 {
        let gaussian = if self.gaussian_porosity > 0.0 && self.gaussian_std_dev > 0.0 {
            let s = core::f64::consts::SQRT_2 * self.gaussian_std_dev;
            0.5 * self.gaussian_porosity
                * (erf((kappa - self.gaussian_mean_strain) / s)
                    + erf(self.gaussian_mean_strain / s))
        } else {
            0.0
        };
        let ramp = if self.ramp_porosity > 0.0 && self.ramp_end > self.ramp_start {
            (self.ramp_porosity / (self.ramp_end - self.ramp_start) * (kappa - self.ramp_start))
                .min(self.ramp_porosity)
                .max(0.0)
        } else {
            0.0
        };
        let linear = self.linear_slope * (cumulated_plastic_strain - self.linear_start).max(0.0);
        gaussian + ramp + linear
    }
}

/// Error function `erf(x)`, to about 1.5e-7 absolute.
///
/// Rust's standard library has no `erf` and this crate takes no numerics
/// dependency, so the Abramowitz and Stegun 7.1.26 rational-times-Gaussian
/// approximation is used. Its stated maximum absolute error is `1.5e-7`, which
/// is orders of magnitude below the uncertainty on any measured nucleation
/// parameter — but it is *not* machine precision, and a test demanding 1e-12 of
/// a nucleation porosity would be testing this approximation rather than the
/// physics.
///
/// Odd by construction: `erf(-x) = -erf(x)`.
fn erf(x: f64) -> f64 {
    const A1: f64 = 0.254_829_592;
    const A2: f64 = -0.284_496_736;
    const A3: f64 = 1.421_413_741;
    const A4: f64 = -1.453_152_027;
    const A5: f64 = 1.061_405_429;
    const P: f64 = 0.327_591_1;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + P * x);
    let y = 1.0 - ((((A5 * t + A4) * t + A3) * t + A2) * t + A1) * t * (-x * x).exp();
    sign * y
}

/// Material parameters of the Gurson-Tvergaard-Needleman law.
///
/// # Upstream keyword block
///
/// `GTN`, read by `Init` in `lcgtn_module.F90`: `Q1`, `Q2`, `PORO_INIT`,
/// `COAL_PORO`, `COAL_ACCE`, `PORO_RUPT`, then the eight nucleation keywords
/// carried by [`GtnNucleation`], then `ENDO_CRIT_VISC` and `ENDO_CRIT_RUPT`.
///
/// # Units
///
/// All dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GtnParameters {
    /// `Q1` \[-\] — Tvergaard's first correction. Multiplies the porosity
    /// everywhere it appears, so `1/Q1` is the porosity at which the material
    /// loses all strength. Typically 1.5. Strictly positive.
    pub q1: f64,
    /// `Q2` \[-\] — Tvergaard's second correction, inside the `cosh`. Scales
    /// the sensitivity to hydrostatic stress. Typically 1.0. Strictly positive.
    pub q2: f64,
    /// `PORO_INIT` (`f0`) \[-\] — the initial void volume fraction, and a floor
    /// on the growth porosity thereafter. Strictly positive: Gurson's surface
    /// with `f = 0` degenerates to von Mises and upstream asserts against it.
    pub initial_porosity: f64,
    /// `COAL_PORO` (`fc`) \[-\] — the porosity at which coalescence starts.
    /// Below it the effective porosity is the true one.
    pub coalescence_porosity: f64,
    /// Coalescence slope `hc = COAL_ACCE - 1` \[-\], non-negative.
    ///
    /// Past `fc`, Tvergaard and Needleman's effective porosity is
    /// `f* = f + hc (f - fc)`, so the material loses strength faster than the
    /// voids actually grow. This is the model's stand-in for the plastic
    /// collapse of the ligaments between voids, which the smooth Gurson surface
    /// cannot represent.
    pub coalescence_slope: f64,
    /// `PORO_RUPT` (`fR`) \[-\] — the porosity at which `f*` reaches `1/Q1` and
    /// the material carries nothing. Consistency requires
    /// `fR = fc + (1/Q1 - fc)/(1 + hc)`, which
    /// [`GtnParameters::rupture_porosity_from_slope`] computes.
    pub rupture_porosity: f64,
    /// Void nucleation.
    pub nucleation: GtnNucleation,
    /// `ENDO_CRIT_RUPT` \[-\] — the damage `D = Q1 f*` above which upstream
    /// declares the point broken and stops integrating. Upstream additionally
    /// caps it at `1 - sqrt(tolerance)`.
    pub broken_damage: f64,
}

impl GtnParameters {
    /// The rupture porosity implied by `Q1`, `fc` and the coalescence slope.
    ///
    /// `fR = fc + (1/Q1 - fc) / (1 + hc)` — the porosity at which the effective
    /// porosity `f*` reaches `1/Q1`. Upstream derives it exactly this way when
    /// `COAL_ACCE` is given rather than `PORO_RUPT`.
    ///
    /// # Units
    ///
    /// All arguments and the result dimensionless.
    #[must_use]
    pub fn rupture_porosity_from_slope(q1: f64, coalescence_porosity: f64, slope: f64) -> f64 {
        coalescence_porosity + (1.0 / q1 - coalescence_porosity) / (1.0 + slope)
    }

    /// Reject parameters upstream asserts against.
    fn validate(self) -> Result<()> {
        if !(self.q1 > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "GTN Q1",
                value: self.q1,
                unit: "-",
                reason: "must be strictly positive",
            });
        }
        if !(self.q2 > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "GTN Q2",
                value: self.q2,
                unit: "-",
                reason: "must be strictly positive",
            });
        }
        if !(self.initial_porosity > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "GTN PORO_INIT",
                value: self.initial_porosity,
                unit: "-",
                reason: "must be strictly positive (upstream asserts f0 > 0)",
            });
        }
        if self.coalescence_slope < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "GTN coalescence slope",
                value: self.coalescence_slope,
                unit: "-",
                reason: "must not be negative",
            });
        }
        if self.initial_porosity > self.rupture_porosity {
            return Err(OffbeatError::Unphysical {
                quantity: "GTN PORO_INIT",
                value: self.initial_porosity,
                unit: "-",
                reason: "must not exceed the rupture porosity PORO_RUPT",
            });
        }
        Ok(())
    }
}

/// Internal state of a GTN point.
///
/// Upstream carries 25 internal variables for `VISC_GTN`, most of them
/// post-processing echoes of the stress and of intermediate equivalent
/// stresses. The six kept here are the ones the update actually needs.
///
/// # Units
///
/// All dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GtnState {
    /// Plastic strain tensor \[-\]. **Not** deviatoric: GTN's flow rule has a
    /// volumetric component, and that component is exactly what grows the
    /// voids.
    pub plastic_strain: SymmTensor,
    /// Hardening variable `kappa` \[-\], upstream `EPSPEQ`. Defined by the work
    /// equivalence `(1-f) R(kappa) dkappa = sigma : deps_p`, so it is the
    /// plastic strain of the *matrix*, not of the porous aggregate.
    pub hardening_variable: f64,
    /// Growth part of the porosity \[-\], upstream `poro_grow`. Floored at
    /// `f0`.
    pub growth_porosity: f64,
    /// Nucleated part of the porosity \[-\], upstream `PORO_NUC`.
    pub nucleation_porosity: f64,
    /// Coalescence contribution to the damage \[-\], upstream `dam_coal`. A
    /// ratchet: it never decreases.
    pub coalescence_damage: f64,
    /// Cumulated equivalent plastic strain `sqrt(2/3 deps_p : deps_p)`
    /// integrated over the history \[-\], upstream `EPCUM`. Drives the linear
    /// nucleation tail; distinct from `kappa`.
    pub cumulated_plastic_strain: f64,
}

impl GtnState {
    /// The initial state for a given material: undeformed, at `f0`, no
    /// coalescence.
    #[must_use]
    pub fn initial(params: GtnParameters) -> Self {
        Self {
            plastic_strain: SymmTensor::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
            hardening_variable: 0.0,
            growth_porosity: params.initial_porosity,
            nucleation_porosity: 0.0,
            coalescence_damage: 0.0,
            cumulated_plastic_strain: 0.0,
        }
    }

    /// Total porosity `f = f_growth + f_nucleation` \[-\].
    #[must_use]
    pub fn porosity(self) -> f64 {
        self.growth_porosity + self.nucleation_porosity
    }
}

/// How a GTN step ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GtnOutcome {
    /// The trial stress was inside the yield surface.
    Elastic,
    /// The staggered plastic solve converged.
    Plastic,
    /// The damage reached [`GtnParameters::broken_damage`]: upstream stops
    /// integrating and returns a zero stress. Not a converged solve — the point
    /// has failed.
    Broken,
}

/// The result of integrating one GTN step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GtnIncrement {
    /// Cauchy stress at the end of the step \[Pa\].
    pub stress: SymmTensor,
    /// Von Mises equivalent of [`Self::stress`] \[Pa\].
    pub equivalent_stress: f64,
    /// Mean stress of [`Self::stress`] \[Pa\].
    pub mean_stress: f64,
    /// Flow stress `sigma_star = R(kappa) + viscous overstress` \[Pa\] at the
    /// end of the step — the yield surface's size parameter.
    pub flow_stress: f64,
    /// Damage `D = Q1 f* + coalescence` \[-\] at the end of the step, capped at
    /// one.
    pub damage: f64,
    /// Updated internal state.
    pub state: GtnState,
    /// How the step ended.
    pub outcome: GtnOutcome,
    /// Outer (staggered) iterations used.
    pub iterations: usize,
}

/// Norton viscous overstress used by `VISC_GTN`.
///
/// `sigma_v = K (dkappa/dt)^(1/n)` \[Pa\] — the extra stress the matrix
/// sustains when it is being strained at a finite rate. Upstream:
/// `visc_norton_module.F90` (`dka_to_vsc`), which asserts `K > 0` and `n > 1`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NortonOverstress {
    /// `N` \[-\] — the Norton exponent. Upstream asserts `N > 1`.
    pub n: f64,
    /// `K` \[Pa s^(1/n)\] — the viscosity coefficient. Strictly positive.
    pub k: f64,
}

impl NortonOverstress {
    /// Overstress \[Pa\] for a hardening increment `dkappa` \[-\] over `dt`
    /// \[s\].
    ///
    /// Zero for a non-positive increment or timestep.
    #[must_use]
    pub fn stress(self, dkappa: f64, dt: f64) -> f64 {
        if dkappa <= 0.0 || dt <= 0.0 {
            return 0.0;
        }
        self.k * (dkappa / dt).powf(1.0 / self.n)
    }
}

/// Gurson-Tvergaard-Needleman porous plasticity.
///
/// # The yield surface
///
/// Upstream writes it (`f_g` in `lcgtn_module.F90`) as
///
/// `Phi = (sigma_eq/sigma_star)^2 + 2 D cosh(3 Q2 sigma_m / (2 sigma_star)) - 1 - D^2`
///
/// with `D = Q1 f* + coalescence damage` the Tvergaard damage and `sigma_star`
/// the matrix flow stress. Two limits make the shape clear:
///
/// - `D = 0`: the surface collapses to `sigma_eq = sigma_star`, plain von
///   Mises. A dense material is rate-independent J2 plasticity.
/// - `sigma_eq = 0`: the surface gives
///   `cosh(3 Q2 sigma_m/(2 sigma_star)) = (1 + D^2)/(2D)`, a **finite**
///   hydrostatic strength. That is the essential difference from J2 plasticity,
///   which has none: a porous solid yields under pure pressure, and that is
///   what drives ductile tearing at a crack tip.
///
/// Compared with [`RousselierLaw`], the `cosh` is symmetric in `sigma_m` where
/// Rousselier's `exp` is not — so GTN predicts void *collapse* in compression
/// with the same law that predicts growth in tension, and Rousselier needs a
/// separate branch for it.
///
/// # Coalescence
///
/// The effective porosity is Tvergaard and Needleman's
/// `f* = f + hc max(0, f - fc)` ([`Self::star_porosity`]). Below `fc` nothing
/// changes; above it the material loses strength `1 + hc` times faster than the
/// voids grow, reaching zero strength at `f = fR`. This is a phenomenological
/// stand-in for the plastic collapse of the intervoid ligaments, and it is the
/// mechanism that makes the failure abrupt rather than asymptotic.
///
/// # ASTER names and upstream provenance
///
/// - `GTN` (`num_lc = 75`, 25 state variables), keywords `ELAS` + `ECRO_NL` +
///   `GTN` (+ `NONLOCAL`), `algo_inte` `SPECIFIQUE`.
/// - `VISC_GTN` (`num_lc = 75`, 25 state variables), the same plus `NORTON`.
///
/// Legacy symbols: `lc0075`, `lcgtn_module`, `visc_norton_module`.
/// Documentation reference: R5.03.29.
///
/// # What this port does and does not reproduce — read this before using it
///
/// **Reproduced:** the yield surface, the Tvergaard-Needleman coalescence map,
/// all three nucleation mechanisms, the implicit growth law
/// `f_grow = (f_grow_old + tr(deps_p)(1 - f_nucl)) / (1 + tr(deps_p))`, the
/// porosity cap at `fR`, the coalescence ratchet, the `ECRO_NL` hardening
/// curve, the Norton overstress `K (dkappa/dt)^(1/n)` of `VISC_GTN`, and the
/// broken-point cutoff.
///
/// **Not reproduced, and this matters:**
///
/// - **The `GRADVARI` nonlocal regularisation.** Upstream's `VISC_GTN` is
///   normally used with a nonlocal damage variable (the `phi` and `r` fields in
///   `lcgtn_module.F90`), precisely because a local softening law gives
///   mesh-dependent answers. This port is **local only**. A structural
///   calculation with it will localise into one element band and the answer
///   will depend on the mesh. That is a property of the model as ported, not a
///   bug to be tuned away.
/// - **Upstream's `SPECIFIQUE` algorithm.** Upstream reformulates the local
///   problem in variables `(p, ts)` with bespoke bounds (`bnd_pmin`,
///   `bnd_pmax`) and a singular-state branch. This port uses a **staggered
///   scheme** instead: an inner bracketed [`brent`] on the plastic multiplier
///   at frozen damage and flow stress, wrapped in an outer fixed point on
///   `(damage, flow stress)`. Same equations, different iteration. It is
///   simpler and its inner bracket is provable; it converges more slowly, and
///   near `D -> 1` it can fail to converge at all — in which case it returns
///   [`OffbeatError::ConstitutiveNotConverged`] rather than a clamped answer.
/// - **The consistent tangent.** Upstream builds one; this port does not.
///   Upstream's own catalogue offers `PERTURBATION` for this law.
/// - **`theta`.** Upstream's theta-predictor on the porosity is not exposed;
///   this port is fully implicit (`theta = 1`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GursonTvergaardNeedleman {
    /// `GTN` — rate-independent.
    RateIndependent(GtnParameters),
    /// `VISC_GTN` — with a Norton overstress on the matrix flow stress.
    ///
    /// The flow stress becomes `sigma_star = R(kappa) + K (dkappa/dt)^(1/n)`.
    /// Keyword block `NORTON` with `N` and `K`.
    Viscous(GtnParameters, NortonOverstress),
}

impl GursonTvergaardNeedleman {
    /// The upstream ASTER behaviour name.
    #[must_use]
    pub const fn aster_name(self) -> &'static str {
        match self {
            Self::RateIndependent(_) => "GTN",
            Self::Viscous(_, _) => "VISC_GTN",
        }
    }

    /// The catalogue entry this law corresponds to.
    #[must_use]
    pub const fn behaviour(self) -> AsterBehaviour {
        match self {
            Self::RateIndependent(_) => AsterBehaviour::Gtn,
            Self::Viscous(_, _) => AsterBehaviour::ViscGtn,
        }
    }

    /// The GTN material parameters, whichever variant this is.
    #[must_use]
    pub const fn parameters(self) -> GtnParameters {
        match self {
            Self::RateIndependent(p) | Self::Viscous(p, _) => p,
        }
    }

    /// Tvergaard-Needleman effective porosity `f* = f + hc max(0, f - fc)`
    /// \[-\].
    ///
    /// Equal to `f` below the coalescence threshold and steeper above it. This
    /// is the porosity the yield surface sees; the state variable is the true
    /// `f`.
    #[must_use]
    pub fn star_porosity(self, porosity: f64) -> f64 {
        let p = self.parameters();
        porosity + p.coalescence_slope * (porosity - p.coalescence_porosity).max(0.0)
    }

    /// The GTN yield function \[-\], dimensionless.
    ///
    /// `Phi = (sigma_eq/sigma_star)^2 + 2 D cosh(3 Q2 sigma_m/(2 sigma_star)) - 1 - D^2`
    ///
    /// # Arguments
    ///
    /// - `equivalent_stress` — `sigma_eq` \[Pa\], non-negative.
    /// - `mean_stress` — `sigma_m` \[Pa\], signed.
    /// - `flow_stress` — `sigma_star` \[Pa\], strictly positive.
    /// - `damage` — `D = Q1 f*` plus coalescence \[-\], in `[0, 1]`.
    ///
    /// Negative means elastic, zero on the surface, positive inadmissible.
    /// Unlike [`RousselierLaw::yield_function`] this is dimensionless, because
    /// the surface is written in squared-stress ratios; scale it by
    /// `sigma_star^2` if a stress-dimensioned residual is wanted.
    #[must_use]
    pub fn yield_function(
        self,
        equivalent_stress: f64,
        mean_stress: f64,
        flow_stress: f64,
        damage: f64,
    ) -> f64 {
        let p = self.parameters();
        let q = equivalent_stress / flow_stress;
        let arg = 1.5 * p.q2 * mean_stress / flow_stress;
        // cosh overflows around |arg| = 710; past that the point is far outside
        // any admissible surface and the sign is all a solver needs.
        let cosh = if arg.abs() > 700.0 {
            f64::MAX / 4.0
        } else {
            arg.cosh()
        };
        q * q + 2.0 * damage * cosh - 1.0 - damage * damage
    }

    /// Viscous overstress \[Pa\] for a hardening increment `dkappa` \[-\] over
    /// `dt` \[s\]; zero for the rate-independent variant.
    #[must_use]
    pub fn overstress(self, dkappa: f64, dt: f64) -> f64 {
        match self {
            Self::RateIndependent(_) => 0.0,
            Self::Viscous(_, n) => n.stress(dkappa, dt),
        }
    }

    /// Integrate one timestep.
    ///
    /// # The algorithm, stated plainly
    ///
    /// Normality on the GTN surface gives a plastic increment with both a
    /// deviatoric and a volumetric part:
    ///
    /// `deps_p = dl [ 3 s/sigma_star^2 + D Q2 sinh(3 Q2 sigma_m/(2 sigma_star))/sigma_star I ]`
    ///
    /// so the deviator still returns radially (its direction is the trial
    /// direction) but the mean stress relaxes too, and that relaxation is what
    /// changes the porosity. Three quantities are therefore coupled: the
    /// plastic multiplier `dl`, the damage `D` (through the porosity), and the
    /// flow stress `sigma_star` (through the hardening variable).
    ///
    /// This port stages them:
    ///
    /// 1. **Innermost**, at fixed `(D, sigma_star, dl)`: solve the scalar,
    ///    strictly monotone equation for the mean stress
    ///    `sigma_m - sigma_m_trial + 3 K dl D Q2 sinh(3 Q2 sigma_m/(2 sigma_star))/sigma_star = 0`
    ///    by [`brent`] on `[min(0, sigma_m_trial), max(0, sigma_m_trial)]` — a
    ///    bracket that always works because the `sinh` term has the sign of
    ///    `sigma_m`.
    /// 2. **Inner**, at fixed `(D, sigma_star)`: solve `Phi(dl) = 0` by
    ///    [`brent`]. `Phi` decreases monotonically in `dl` (both the equivalent
    ///    and the mean stress relax towards zero) and tends to `-(1-D)^2 < 0`,
    ///    so the bracket is again constructible.
    /// 3. **Outer**: update the porosity, the hardening variable and hence
    ///    `(D, sigma_star)` from that solution, and repeat until both settle.
    ///
    /// The outer loop is a fixed point, not a Newton iteration, and it is the
    /// part that can fail. It does so exactly where the physics is degenerate —
    /// as `D` approaches one, softening outruns the fixed point's contraction —
    /// and when it does this returns
    /// [`OffbeatError::ConstitutiveNotConverged`]. It never clamps `D` and
    /// reports success.
    ///
    /// # Arguments
    ///
    /// - `elastic` — isotropic moduli.
    /// - `hardening` — the matrix flow curve `R(kappa)`; use
    ///   [`IsotropicHardening::EcroNl`] to match upstream's `ECRO_NL`.
    /// - `state` — internal state at the **start** of the step.
    /// - `total_strain` — total mechanical strain at the **end** of the step
    ///   \[-\], thermal part already removed.
    /// - `dt` — timestep \[s\]. Only [`Self::Viscous`] uses it.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for invalid moduli, parameters, timestep or
    /// a non-positive flow stress; [`OffbeatError::ConstitutiveNotConverged`]
    /// if the staggered loop or either bracketed solve fails.
    pub fn integrate(
        self,
        elastic: IsotropicElasticity,
        hardening: IsotropicHardening,
        state: GtnState,
        total_strain: SymmTensor,
        dt: f64,
    ) -> Result<GtnIncrement> {
        elastic.validate()?;
        let mat = self.parameters();
        mat.validate()?;
        if dt < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "timestep",
                value: dt,
                unit: "s",
                reason: "must not be negative",
            });
        }

        let two_mu = 2.0 * elastic.shear_modulus;
        let three_k = 3.0 * elastic.bulk_modulus;

        // Elastic predictor. Plastic strain is *not* deviatoric here, so the
        // mean part carries a plastic contribution too.
        let elastic_strain = total_strain - state.plastic_strain;
        let s_trial = scaled(deviator(elastic_strain), two_mu);
        let eq_trial = von_mises_of_deviator(s_trial);
        let mean_trial = elastic.bulk_modulus * elastic_strain.tr();

        let damage_start =
            (state.coalescence_damage + mat.q1 * self.star_porosity(state.porosity())).min(1.0);

        // Already broken.
        if damage_start >= mat.broken_damage {
            return Ok(GtnIncrement {
                stress: SymmTensor::new(0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
                equivalent_stress: 0.0,
                mean_stress: 0.0,
                flow_stress: hardening.value(state.hardening_variable),
                damage: 1.0,
                state,
                outcome: GtnOutcome::Broken,
                iterations: 0,
            });
        }

        let sigma_star_start = hardening.value(state.hardening_variable);
        if !(sigma_star_start > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "GTN matrix flow stress R(kappa)",
                value: sigma_star_start,
                unit: "Pa",
                reason: "must be strictly positive; the yield surface divides by it",
            });
        }

        // Elastic test.
        if dt == 0.0
            || self.yield_function(eq_trial, mean_trial, sigma_star_start, damage_start) <= 0.0
        {
            let stress = from_dev_and_mean(s_trial, mean_trial);
            return Ok(GtnIncrement {
                stress,
                equivalent_stress: eq_trial,
                mean_stress: mean_trial,
                flow_stress: sigma_star_start,
                damage: damage_start,
                state,
                outcome: GtnOutcome::Elastic,
                iterations: 0,
            });
        }

        // Innermost: the mean stress at a given multiplier, damage and flow
        // stress. Strictly increasing in sigma_m, so the bracket is exact.
        let solve_mean = |dlambda: f64, damage: f64, sigma_star: f64| -> Result<f64> {
            if mean_trial == 0.0 || damage == 0.0 || dlambda == 0.0 {
                return Ok(mean_trial);
            }
            let residual = |sm: f64| {
                let arg = 1.5 * mat.q2 * sm / sigma_star;
                let sinh = if arg.abs() > 700.0 {
                    arg.signum() * f64::MAX / 4.0
                } else {
                    arg.sinh()
                };
                sm - mean_trial + three_k * dlambda * damage * mat.q2 * sinh / sigma_star
            };
            let bracket = if mean_trial > 0.0 {
                (0.0, mean_trial)
            } else {
                (mean_trial, 0.0)
            };
            let solution = brent(
                residual,
                bracket,
                &SolverControl {
                    max_iter: 200,
                    residual_tol: 1.0e-12 * sigma_star,
                    step_tol: 1.0e-18,
                },
            )?;
            Ok(solution.root)
        };

        // Inner: the plastic multiplier at a given damage and flow stress.
        let solve_multiplier = |damage: f64, sigma_star: f64| -> Result<(f64, f64, f64)> {
            let scale = sigma_star * sigma_star / (6.0 * elastic.shear_modulus);
            let stress_at = |dl: f64| -> Result<(f64, f64)> {
                let eq = eq_trial / (1.0 + dl / scale);
                let sm = solve_mean(dl, damage, sigma_star)?;
                Ok((eq, sm))
            };
            let residual = |dl: f64| match stress_at(dl) {
                Ok((eq, sm)) => self.yield_function(eq, sm, sigma_star, damage),
                Err(_) => f64::NAN,
            };
            // Upper bound: enough multiplier to drive the equivalent stress to a
            // thousandth of the flow stress, at which point the residual is
            // essentially -(1-D)^2 <= 0.
            let hi = scale * ((eq_trial / (1.0e-3 * sigma_star)).max(2.0) - 1.0);
            let solution = brent(
                residual,
                (0.0, hi),
                &SolverControl {
                    max_iter: 300,
                    residual_tol: 1.0e-14,
                    step_tol: 1.0e-20,
                },
            )?;
            let dl = solution.root.max(0.0);
            let (eq, sm) = stress_at(dl)?;
            Ok((dl, eq, sm))
        };

        // Outer staggered fixed point on (damage, flow stress).
        let mut damage = damage_start;
        let mut sigma_star = sigma_star_start;
        let mut result = None;
        let mut iterations = 0;
        for iteration in 1..=200_usize {
            iterations = iteration;
            let (dlambda, eq, sm) = solve_multiplier(damage, sigma_star)?;

            let arg = 1.5 * mat.q2 * sm / sigma_star;
            let sinh = if arg.abs() > 700.0 {
                arg.signum() * f64::MAX / 4.0
            } else {
                arg.sinh()
            };
            let volumetric_flow = damage * mat.q2 * sinh / sigma_star;
            let trace_dp = 3.0 * dlambda * volumetric_flow;

            // Plastic strain increment from normality.
            let shrink = if eq_trial > 0.0 { eq / eq_trial } else { 0.0 };
            let s_new = scaled(s_trial, shrink);
            let dev_dp = scaled(s_new, 3.0 * dlambda / (sigma_star * sigma_star));
            let dstrain = from_dev_and_mean(dev_dp, trace_dp / 3.0);

            // Work equivalence: (1-f) sigma_star dkappa = sigma : deps_p.
            let f_old = state.porosity();
            let work =
                dlambda * (2.0 * eq * eq / (sigma_star * sigma_star) + 3.0 * sm * volumetric_flow);
            let dkappa = (work / ((1.0 - f_old).max(1.0e-12) * sigma_star)).max(0.0);
            let kappa = state.hardening_variable + dkappa;

            // Porosity update (implicit growth law + nucleation).
            let epcum =
                state.cumulated_plastic_strain + (2.0 / 3.0 * dstrain.double_inner(dstrain)).sqrt();
            let mut f_nucl = mat
                .nucleation
                .porosity(kappa, epcum)
                .max(state.nucleation_porosity);
            let mut f_grow = ((state.growth_porosity + trace_dp * (1.0 - f_nucl))
                / (1.0 + trace_dp))
                .max(mat.initial_porosity);
            if f_nucl + f_grow > mat.rupture_porosity {
                let previous = state.porosity();
                let excess = f_nucl + f_grow - previous;
                if excess > 0.0 {
                    let coef = ((mat.rupture_porosity - previous) / excess).clamp(0.0, 1.0);
                    f_nucl = coef * f_nucl + (1.0 - coef) * state.nucleation_porosity;
                    f_grow = coef * f_grow + (1.0 - coef) * state.growth_porosity;
                }
            }
            let f_new = (f_nucl + f_grow).min(mat.rupture_porosity);
            let coalescence_damage = state
                .coalescence_damage
                .max(mat.q1 * (self.star_porosity(f_new) - f_new));
            let damage_new = (coalescence_damage + mat.q1 * f_new).min(1.0);
            let sigma_star_new = hardening.value(kappa) + self.overstress(dkappa, dt);

            let converged = (damage_new - damage).abs() <= 1.0e-12
                && (sigma_star_new - sigma_star).abs() <= 1.0e-12 * sigma_star_new.max(1.0);

            damage = damage_new;
            sigma_star = sigma_star_new;

            if converged {
                result = Some(GtnIncrement {
                    stress: from_dev_and_mean(s_new, sm),
                    equivalent_stress: eq,
                    mean_stress: sm,
                    flow_stress: sigma_star,
                    damage,
                    state: GtnState {
                        plastic_strain: state.plastic_strain + dstrain,
                        hardening_variable: kappa,
                        growth_porosity: f_grow,
                        nucleation_porosity: f_nucl,
                        coalescence_damage,
                        cumulated_plastic_strain: epcum,
                    },
                    outcome: if damage >= mat.broken_damage {
                        GtnOutcome::Broken
                    } else {
                        GtnOutcome::Plastic
                    },
                    iterations: iteration,
                });
                break;
            }
        }

        result.ok_or(OffbeatError::ConstitutiveNotConverged {
            cell: usize::MAX,
            residual: f64::NAN,
            iterations,
        })
    }
}

// ===========================================================================
// CRIT_RUPT — element rupture criterion
// ===========================================================================

/// Parameters of the `CRIT_RUPT` rupture criterion.
///
/// # Units
///
/// `critical_stress` in pascal \[Pa\]; `stiffness_divisor` dimensionless.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuptureCriterion {
    /// `SIGM_C` \[Pa\] — the critical maximum principal stress. When the
    /// element-averaged stress state's largest principal stress exceeds this,
    /// the element is declared broken.
    pub critical_stress: f64,
    /// `COEF` \[-\] — the factor by which a broken element's Young's modulus is
    /// **divided**. Upstream: `e = e/coef` in `rupmat.F90`, so a large `COEF`
    /// means a nearly-zero residual stiffness. Strictly positive.
    pub stiffness_divisor: f64,
}

/// The six internal variables `CRIT_RUPT` appends to the host law's.
///
/// Upstream: `EPSPVIT`, `EDISS`, `EDISSCUM`, `PDISS`, `PDISSCUM`, `CRITRUPT`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RuptureState {
    /// `EPSPVIT` — equivalent plastic strain rate `dp/dt` \[1/s\].
    pub plastic_strain_rate: f64,
    /// `EDISS` — energy dissipated over this step, `dp sigma_eq` \[J/m^3\].
    pub dissipated_energy: f64,
    /// `EDISSCUM` — cumulated dissipated energy \[J/m^3\].
    pub cumulated_dissipated_energy: f64,
    /// `PDISS` — dissipated power, `dp/dt sigma_eq` \[W/m^3\].
    pub dissipated_power: f64,
    /// `PDISSCUM` — cumulated dissipated power \[W/m^3\]. Upstream sums the
    /// per-step powers rather than integrating them, so this is a running sum
    /// of rates and not an energy; the name is upstream's and the behaviour is
    /// reproduced as found.
    pub cumulated_dissipated_power: f64,
    /// `CRITRUPT` — the rupture flag. Once true it stays true: upstream
    /// re-asserts it on every subsequent step ("la maille etait deja cassee.
    /// elle le reste").
    pub broken: bool,
}

impl RuptureState {
    /// The initial state: nothing dissipated, nothing broken.
    #[must_use]
    pub fn pristine() -> Self {
        Self {
            plastic_strain_rate: 0.0,
            dissipated_energy: 0.0,
            cumulated_dissipated_energy: 0.0,
            dissipated_power: 0.0,
            cumulated_dissipated_power: 0.0,
            broken: false,
        }
    }
}

impl RuptureCriterion {
    /// The upstream ASTER behaviour name.
    #[must_use]
    pub const fn aster_name(self) -> &'static str {
        "CRIT_RUPT"
    }

    /// The catalogue entry this corresponds to.
    #[must_use]
    pub const fn behaviour(self) -> AsterBehaviour {
        AsterBehaviour::CritRupt
    }

    /// Evaluate the criterion for one element over one step.
    ///
    /// # What it is, and what it is not
    ///
    /// `CRIT_RUPT` is not a constitutive law — its catalogue `lc_type` is
    /// `UTILITAIRE` and its `num_lc` is zero. It is a **post-iteration hook**
    /// (`POST_ITER = 'CRIT_RUPT'`) that runs after whatever real law the
    /// element uses, averages the stress over the element's Gauss points, and
    /// tests the largest principal stress of that average against `SIGM_C`.
    /// Once the test trips, the element's Young's modulus is divided by `COEF`
    /// (see [`Self::degraded_young_modulus`]) and its stress is zeroed —
    /// upstream's crude but effective element-death scheme.
    ///
    /// The averaging is deliberate and is why this takes an element-mean stress
    /// rather than a point stress: testing point by point would kill an element
    /// on a single hot Gauss point, which is a mesh artefact rather than a
    /// physical failure.
    ///
    /// # Arguments
    ///
    /// - `element_mean_stress` — the stress averaged over the element's Gauss
    ///   points \[Pa\].
    /// - `plastic_strain_increment` — the element-averaged equivalent plastic
    ///   strain increment over the step \[-\], non-negative.
    /// - `dt` — the timestep \[s\], strictly positive.
    /// - `previous` — the state at the start of the step.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a non-positive timestep or a
    /// non-positive `COEF`.
    pub fn evaluate(
        self,
        element_mean_stress: SymmTensor,
        plastic_strain_increment: f64,
        dt: f64,
        previous: RuptureState,
    ) -> Result<RuptureState> {
        if !(dt > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "timestep",
                value: dt,
                unit: "s",
                reason: "must be strictly positive; the criterion divides by it",
            });
        }
        if !(self.stiffness_divisor > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "CRIT_RUPT COEF",
                value: self.stiffness_divisor,
                unit: "-",
                reason: "must be strictly positive; the degraded modulus divides by it",
            });
        }

        let sigma_eq = equivalent_stress(element_mean_stress);
        let principal = max_principal_stress(element_mean_stress);
        let dp = plastic_strain_increment;

        Ok(RuptureState {
            plastic_strain_rate: dp / dt,
            dissipated_energy: dp * sigma_eq,
            cumulated_dissipated_energy: previous.cumulated_dissipated_energy + dp * sigma_eq,
            dissipated_power: dp * sigma_eq / dt,
            cumulated_dissipated_power: previous.cumulated_dissipated_power + dp * sigma_eq / dt,
            broken: previous.broken || principal > self.critical_stress,
        })
    }

    /// Young's modulus \[Pa\] to use for an element, given whether it is broken.
    ///
    /// Upstream `rupmat.F90`: `E / COEF` once broken, unchanged otherwise. The
    /// companion action — zeroing the broken element's stress — is the caller's
    /// to apply, because it belongs to the host law's state, not to this
    /// criterion's.
    #[must_use]
    pub fn degraded_young_modulus(self, young: f64, broken: bool) -> f64 {
        if broken {
            young / self.stiffness_divisor
        } else {
            young
        }
    }
}

#[cfg(test)]
mod tests;
