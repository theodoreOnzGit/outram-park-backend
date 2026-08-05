// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from code_aster (https://gitlab.com/codeaster/src)
//   Copyright (C) 1991 - 2026 - EDF R&D
//   Licence: GPL-3.0-or-later
//   Upstream commit: b504ea08c2f49575e04644cee2e39a63ea45c16e
//   Sources:
//     bibfor/nonlinear/nmhoff.F90  -- the NORTON_HOFF law (legacy symbol `nmhoff`)
//     bibfor/lc/lc0017.F90         -- NORTON_HOFF dispatch (`num_lc = 17`, `lc0017`)
//     bibfor/comport/nmisot.F90    -- isotropic-hardening radial return (`nmisot`)
//     bibfor/comport/ecpuis.F90    -- power-law hardening curve (`ecpuis`)
//     bibfor/nonlinear/nmcri2.F90  -- the power-law return residual (`nmcri2`)
//     bibfor/lc/lc0002.F90         -- VMIS_ISOT_* / VISC_ISOT_* dispatch (`lc0002`)
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Isotropic hardening laws and the Norton-Hoff limit-analysis regularisation.
//!
//! # What is in here, and what is deliberately not
//!
//! Two things that both reuse the scalar radial return, and are otherwise
//! unrelated:
//!
//! - [`IsotropicHardening`] — the hardening curve `R(p)` of code_aster's
//!   `VMIS_ISOT_*` / `VISC_ISOT_*` family, plus the scalar return that solves
//!   for the plastic multiplier against it. Rate-**in**dependent; see the
//!   warning below.
//! - [`NortonHoffLimitAnalysis`] — the `NORTON_HOFF` law, which despite its
//!   name is not a creep law at all but a regularisation used to compute
//!   **limit loads**.
//!
//! # Warning: `VISC_ISOT_*` is not rate-dependent through this path
//!
//! The name invites the assumption that `VISC_ISOT_LINE` and `VISC_ISOT_TRAC`
//! add a viscous overstress to `VMIS_ISOT_LINE`/`_TRAC`. Through upstream's
//! `nmisot` they do not. That subroutine's signature carries **no time
//! instants at all** — no `instam`, no `instap`, no timestep — so nothing in
//! it can depend on strain *rate*; it branches only on the trailing five
//! characters of the behaviour name (`_LINE`, `_PUIS`, `_TRAC`) to pick a
//! hardening curve, and `lc0002` routes both the `VMIS_` and the `VISC_`
//! spellings into it unchanged.
//!
//! This port therefore implements the rate-independent return that `nmisot`
//! actually performs, and does **not** invent a viscous term to justify the
//! prefix. `VISC_ISOT_NL` is a genuinely different law on a different path
//! (`lc0076`) and is not ported here.
//!
//! # The radial return, in one paragraph
//!
//! Take the elastic trial stress, and measure it with the von Mises equivalent
//! `σ_eq`. If that is below the current yield `R(p)`, the step was elastic and
//! nothing happens. If it is above, plastic flow must bring it back onto the
//! yield surface, and because the flow is deviatoric and isotropic it does so
//! along the trial deviator's own direction — hence *radial*. The only unknown
//! is how far: the plastic multiplier `Δp`, fixed by requiring the returned
//! stress to sit exactly on the surface,
//!
//! `R(p_m + Δp) + 3μ Δp - σ_eq^trial = 0`.
//!
//! That is upstream's `nmcri2` residual verbatim, where its `1.5*deuxmu` is
//! `1.5 × 2μ = 3μ`.

use crate::error::{OffbeatError, Result};
use crate::rheology::aster::integration::{brent, LocalSolution, SolverControl};
use crate::rheology::aster::kinematics::AsterVoigt;

/// Isotropic hardening curve `R(p)` — the radius of the von Mises yield
/// surface as a function of accumulated equivalent plastic strain.
///
/// ASTER behaviour names: the `_LINE` and `_PUIS` suffixes of
/// `VMIS_ISOT_LINE`, `VMIS_ISOT_PUIS`, `VISC_ISOT_LINE` (`num_lc = 2`).
/// Upstream: `bibfor/comport/nmisot.F90` (legacy symbol `nmisot`), with the
/// power-law curve in `bibfor/comport/ecpuis.F90` (`ecpuis`).
///
/// Enum dispatch rather than a trait object, per the workspace rule: the set of
/// hardening curves is closed, and adding one must force every `match` to be
/// revisited.
///
/// # Not ported
///
/// `_TRAC` — a hardening curve given as a **tabulated** traction curve read
/// from the material deck (upstream `rctrac`/`rcfonc`). It needs the material
/// data-table infrastructure rather than any new mechanics, so it is left out
/// deliberately rather than approximated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IsotropicHardening {
    /// Linear hardening, `R(p) = σ_y + H p`.
    ///
    /// Upstream's `_LINE` branch. The cheapest useful hardening model, and the
    /// only one whose radial return has a closed-form solution.
    Linear(LinearHardening),
    /// Power-law (Ramberg-Osgood style) hardening.
    ///
    /// Upstream's `_PUIS` branch, curve `ecpuis`.
    PowerLaw(PowerLawHardening),
}

/// Parameters of linear isotropic hardening.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LinearHardening {
    /// Initial yield stress `σ_y` \[Pa\]. Upstream `SY` of `ECRO_LINE`.
    ///
    /// Must be positive; a zero or negative yield stress has no physical
    /// meaning and is rejected by [`IsotropicHardening::radial_return`].
    pub yield_stress: f64,
    /// Hardening modulus `H = dR/dp` \[Pa\]. Upstream `rprim`.
    ///
    /// Zero gives perfect plasticity. Negative values model *softening* and are
    /// permitted, but see the note on
    /// [`radial_return`](IsotropicHardening::radial_return) — softening steeper
    /// than `-3μ` destroys the uniqueness of the return.
    pub hardening_modulus: f64,
}

/// Parameters of power-law isotropic hardening.
///
/// The curve is
///
/// `R(p) = σ_y + σ_y · (E p / (α σ_y))^(1/n)`,
///
/// which is upstream `ecpuis` verbatim, with `unsurn = 1/n`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PowerLawHardening {
    /// Initial yield stress `σ_y` \[Pa\]. Must be positive.
    pub yield_stress: f64,
    /// Young's modulus `E` \[Pa\]. Must be positive.
    pub youngs_modulus: f64,
    /// Dimensionless coefficient `α` \[-\]. Upstream `alfafa`. Must be
    /// positive.
    pub alpha: f64,
    /// Hardening exponent `n` \[-\]. Upstream stores its reciprocal as
    /// `unsurn`. Must be positive; `n = 1` recovers linear hardening in `p`.
    pub exponent: f64,
}

/// Below this accumulated plastic strain the power-law curve is replaced by its
/// secant line through the origin.
///
/// Upstream's `p0 = 1.d-10`, reproduced exactly. The reason is that
/// `dR/dp ∝ p^(1/n - 1)` diverges as `p → 0` for any `n > 1`, so a Newton step
/// taken at `p = 0` would see an infinite slope. Upstream replaces the curve
/// below `p0` with the straight line joining the origin to `R(p0)`, which is
/// finite-sloped and continuous with the curve at `p0`.
const POWER_LAW_LINEARISATION_STRAIN: f64 = 1.0e-10;

impl IsotropicHardening {
    /// The yield radius `R(p)` \[Pa\] at accumulated equivalent plastic strain
    /// `p` \[-\].
    ///
    /// `p` must be non-negative; it is an accumulated (monotone) measure.
    #[must_use]
    pub fn yield_radius(&self, p: f64) -> f64 {
        match self {
            Self::Linear(h) => h.yield_stress + h.hardening_modulus * p,
            Self::PowerLaw(h) => {
                let (radius, _) = power_law_curve(*h, p);
                radius
            }
        }
    }

    /// The hardening slope `R'(p) = dR/dp` \[Pa\] at accumulated equivalent
    /// plastic strain `p` \[-\].
    #[must_use]
    pub fn hardening_slope(&self, p: f64) -> f64 {
        match self {
            Self::Linear(h) => h.hardening_modulus,
            Self::PowerLaw(h) => {
                let (_, slope) = power_law_curve(*h, p);
                slope
            }
        }
    }

    /// The initial yield stress `σ_y` \[Pa\].
    #[must_use]
    pub fn yield_stress(&self) -> f64 {
        match self {
            Self::Linear(h) => h.yield_stress,
            Self::PowerLaw(h) => h.yield_stress,
        }
    }

    /// The ASTER behaviour-name suffix this curve corresponds to.
    ///
    /// Returned verbatim, per the workspace naming rule: `"_LINE"` or
    /// `"_PUIS"`. These are the trailing five characters upstream's `nmisot`
    /// itself branches on, so a reader can match this port to that `if` chain
    /// directly.
    #[must_use]
    pub fn aster_name_suffix(&self) -> &'static str {
        match self {
            Self::Linear(_) => "_LINE",
            Self::PowerLaw(_) => "_PUIS",
        }
    }

    /// Solve the von Mises radial return for the plastic multiplier.
    ///
    /// # Arguments
    ///
    /// - `trial_equivalent_stress` — the von Mises equivalent `σ_eq` \[Pa\] of
    ///   the **elastic trial** deviatoric stress, i.e. `√(3/2 s:s)` computed
    ///   from the stress the step would reach with no plastic flow. Must be
    ///   non-negative.
    /// - `shear_modulus` — `μ` \[Pa\]. Must be positive. Upstream carries
    ///   `deuxmu = 2μ = E/(1+ν)` and writes `1.5*deuxmu`; this port takes `μ`
    ///   itself and writes `3μ`, which is the same number.
    /// - `accumulated_strain` — `p_m` \[-\], the accumulated equivalent plastic
    ///   strain at the start of the step. Must be non-negative.
    ///
    /// # Returns
    ///
    /// `None` if the step is elastic — `σ_eq ≤ R(p_m)`, upstream's
    /// `seuil ≤ 0` branch, which sets `dp = 0` and takes no iteration at all.
    /// Otherwise `Some(solution)` with the plastic multiplier `Δp` in
    /// `solution.root`.
    ///
    /// # Which solver, and why the linear case is not iterated
    ///
    /// Linear hardening admits the closed form
    /// `Δp = (σ_eq - σ_y - H p_m) / (H + 3μ)`, which upstream also uses rather
    /// than iterating. It is exact, so iterating it would only add rounding.
    /// The power law has no closed form and is bracketed with
    /// [`brent`](crate::rheology::aster::integration::brent) on upstream's
    /// `nmcri2` residual. The bracket `[0, σ_eq / (3μ)]` is guaranteed to
    /// straddle the root whenever the step is plastic: at `Δp = 0` the residual
    /// is `R(p_m) - σ_eq < 0`, and at the upper end the `3μΔp` term alone
    /// already equals `σ_eq` while `R ≥ σ_y > 0`, so the residual is positive.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] if any input is outside the ranges above,
    /// or if a hardening parameter is non-positive.
    /// [`OffbeatError::ConstitutiveNotConverged`] if the bracketed solve
    /// exhausts its iteration budget.
    ///
    /// # A note on softening
    ///
    /// A sufficiently negative `hardening_modulus` (`H ≤ -3μ`) makes the
    /// residual non-increasing in `Δp`, so the return has no unique solution —
    /// physically, the material sheds stress faster than the elastic unloading
    /// can follow. This port **rejects** that case rather than returning the
    /// formally-computed value, because the closed form still evaluates to a
    /// finite number there and would silently be wrong. Upstream does not guard
    /// it.
    pub fn radial_return(
        &self,
        trial_equivalent_stress: f64,
        shear_modulus: f64,
        accumulated_strain: f64,
        control: &SolverControl,
    ) -> Result<Option<LocalSolution>> {
        self.validate()?;
        check_non_negative(
            trial_equivalent_stress,
            "trial equivalent stress",
            "Pa",
            "a von Mises equivalent stress is a norm and cannot be negative",
        )?;
        check_non_negative(
            accumulated_strain,
            "accumulated equivalent plastic strain",
            "-",
            "accumulated plastic strain is monotone and cannot be negative",
        )?;
        if !(shear_modulus > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "shear modulus",
                value: shear_modulus,
                unit: "Pa",
                reason: "must be strictly positive",
            });
        }

        let three_mu = 3.0 * shear_modulus;
        let radius_at_start = self.yield_radius(accumulated_strain);

        // Upstream's `seuil = sieleq - rp`; `seuil <= 0` is the elastic branch.
        if trial_equivalent_stress - radius_at_start <= 0.0 {
            return Ok(None);
        }

        match self {
            Self::Linear(h) => {
                let denominator = h.hardening_modulus + three_mu;
                if denominator <= 0.0 {
                    return Err(OffbeatError::Unphysical {
                        quantity: "hardening modulus",
                        value: h.hardening_modulus,
                        unit: "Pa",
                        reason: "softening at or beyond -3*mu makes the radial \
                                 return non-unique",
                    });
                }
                let delta_p = (trial_equivalent_stress
                    - h.yield_stress
                    - h.hardening_modulus * accumulated_strain)
                    / denominator;
                Ok(Some(LocalSolution {
                    root: delta_p,
                    residual: self.return_residual(
                        delta_p,
                        trial_equivalent_stress,
                        three_mu,
                        accumulated_strain,
                    ),
                    iterations: 0,
                    bisection_steps: 0,
                }))
            }
            Self::PowerLaw(_) => {
                let upper = trial_equivalent_stress / three_mu;
                let solution = brent(
                    |dp| {
                        self.return_residual(
                            dp,
                            trial_equivalent_stress,
                            three_mu,
                            accumulated_strain,
                        )
                    },
                    (0.0, upper),
                    control,
                )?;
                Ok(Some(solution))
            }
        }
    }

    /// Upstream's `nmcri2` residual, `R(p_m + Δp) + 3μ Δp - σ_eq^trial`.
    ///
    /// Zero exactly when the returned stress lies on the yield surface.
    /// Exposed because a caller assembling a consistent tangent needs the same
    /// function, and because a test that cannot see the residual cannot show
    /// the return actually landed on the surface.
    #[must_use]
    pub fn return_residual(
        &self,
        delta_p: f64,
        trial_equivalent_stress: f64,
        three_shear_moduli: f64,
        accumulated_strain: f64,
    ) -> f64 {
        self.yield_radius(accumulated_strain + delta_p) + three_shear_moduli * delta_p
            - trial_equivalent_stress
    }

    fn validate(&self) -> Result<()> {
        match self {
            Self::Linear(h) => check_positive(h.yield_stress, "yield stress", "Pa"),
            Self::PowerLaw(h) => {
                check_positive(h.yield_stress, "yield stress", "Pa")?;
                check_positive(h.youngs_modulus, "Young's modulus", "Pa")?;
                check_positive(h.alpha, "power-law hardening coefficient alpha", "-")?;
                check_positive(h.exponent, "power-law hardening exponent n", "-")
            }
        }
    }
}

/// The power-law curve and its slope, upstream `ecpuis`.
///
/// Returns `(R(p), R'(p))` in \[Pa\] and \[Pa\] respectively. Below
/// [`POWER_LAW_LINEARISATION_STRAIN`] the curve is the secant line through the
/// origin, exactly as upstream does.
fn power_law_curve(h: PowerLawHardening, p: f64) -> (f64, f64) {
    let inverse_n = 1.0 / h.exponent;
    let coefficient = h.youngs_modulus / (h.alpha * h.yield_stress);
    let p0 = POWER_LAW_LINEARISATION_STRAIN;

    if p <= p0 {
        // Upstream: rp0 = sigy*(E/alfafa/sigy*p0)^(1/n) + sigy, then the
        // straight line from (0, sigy) to (p0, rp0).
        let radius_at_p0 = h.yield_stress * (coefficient * p0).powf(inverse_n) + h.yield_stress;
        let slope = (radius_at_p0 - h.yield_stress) / p0;
        (h.yield_stress + p * slope, slope)
    } else {
        let radius = h.yield_stress * (coefficient * p).powf(inverse_n) + h.yield_stress;
        let slope =
            inverse_n * h.yield_stress * coefficient * (coefficient * p).powf(inverse_n - 1.0);
        (radius, slope)
    }
}

// ── Norton-Hoff limit analysis ───────────────────────────────────────────────

/// The Norton-Hoff regularisation used for **limit-load** analysis.
///
/// ASTER behaviour name: `NORTON_HOFF` (`num_lc = 17`). Upstream:
/// `bibfor/nonlinear/nmhoff.F90` — legacy symbol `nmhoff`, dispatched by
/// `bibfor/lc/lc0017.F90` (`lc0017`).
///
/// # This is not a creep law, despite the name
///
/// The name is shared with Norton creep and the two are easy to confuse, but
/// they answer different questions. Norton creep asks *how fast does this
/// deform under load*. Norton-Hoff limit analysis asks *what is the largest
/// load this structure can carry at all* — and it gets there by solving a
/// sequence of nonlinear-elastic problems whose solutions converge onto the
/// rigid-perfectly-plastic collapse state. There is no accumulated strain, no
/// internal state, and no history: the stress is a pure function of the
/// current total strain.
///
/// # The law
///
/// `σ = A ‖ε‖^(m-2) ε`, with `A = σ_y (2/3)^(m/2)`,
///
/// where `‖ε‖` is the Euclidean norm of the strain in Mandel form — which,
/// because Mandel carries `√2` on the shears, equals the tensor Frobenius norm
/// `√(ε:ε)`. This is exactly why the port takes an [`AsterVoigt`] and not a
/// loose six-array: the identity holds in Mandel and fails in engineering
/// Voigt.
///
/// # The continuation parameter
///
/// The exponent is driven by a pseudo-time `t`:
///
/// `m = 1 + 10^(1-t)`.
///
/// At `t = 1`, `m = 2` and the law is **linear** — an ordinary Newtonian
/// solid. As `t` grows, `m → 1` and the stress magnitude tends to `A`
/// independent of how large the strain is, which is the **rigid-perfectly-
/// plastic** limit whose solution is the collapse load. So `t` is not physical
/// time; it is a homotopy parameter walking the problem from an easy linear
/// solve to the hard plastic one. Advancing it too fast is the usual reason a
/// limit-analysis run stops converging.
///
/// # Not ported
///
/// The consistent tangent `dsidep`. Upstream builds it in the same subroutine
/// (`coef·I + coef(m-2)/‖ε‖² · ε ⊗ ε`), but it is only consumed by an assembled
/// FE stiffness matrix, which this crate's mechanics solve does not yet take.
/// It is a small addition once that exists.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NortonHoffLimitAnalysis {
    /// Yield stress `σ_y` \[Pa\]. Upstream reads it as `SY` from the
    /// `ECRO_LINE` material block. Must be positive.
    pub yield_stress: f64,
}

impl NortonHoffLimitAnalysis {
    /// Build the law from its single material parameter.
    #[must_use]
    pub fn new(yield_stress: f64) -> Self {
        Self { yield_stress }
    }

    /// The ASTER behaviour name, verbatim.
    #[must_use]
    pub fn aster_name(&self) -> &'static str {
        "NORTON_HOFF"
    }

    /// The exponent `m = 1 + 10^(1-t)` \[-\] at pseudo-time `t` \[-\].
    ///
    /// `t = 1` gives exactly `m = 2` (linear); larger `t` drives `m` toward 1
    /// (rigid-plastic). Values `t < 1` give `m > 2` and walk *away* from the
    /// plastic limit, which is legal arithmetic but not a useful continuation
    /// direction.
    #[must_use]
    pub fn exponent(pseudo_time: f64) -> f64 {
        1.0 + 10.0_f64.powf(1.0 - pseudo_time)
    }

    /// Stress \[Pa\] from total strain, at pseudo-time `t` \[-\].
    ///
    /// `strain` is the total strain in Mandel form (dimensionless). There is no
    /// history argument because the law has none.
    ///
    /// # The zero-strain and linear branches
    ///
    /// Upstream computes `coef = A ‖ε‖^(m-2)` but takes a separate branch when
    /// `t = 1` **or** `‖ε‖ = 0`, setting `coef = A`. The second condition is
    /// not cosmetic: for `m < 2` the exponent `m-2` is negative, so `0^(m-2)`
    /// is an infinity, and the branch is what keeps a zero-strain point from
    /// producing a NaN stress. This port reproduces both conditions. Upstream
    /// additionally disables floating-point exception trapping around the whole
    /// routine (`matfpe(-1)`), which is a fair signal that it expected trouble
    /// here.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] if the yield stress is not positive.
    pub fn stress(&self, strain: AsterVoigt, pseudo_time: f64) -> Result<AsterVoigt> {
        check_positive(self.yield_stress, "yield stress", "Pa")?;

        let m = Self::exponent(pseudo_time);
        // A = sy * sqrt(2/3)^m, upstream `am = sy*rac23**m`.
        let amplitude = self.yield_stress * (2.0_f64 / 3.0).sqrt().powf(m);
        let norm = strain.norm();

        // Upstream `line = inst .eq. 1 .or. epsno .eq. 0.d0`.
        let coefficient = if pseudo_time == 1.0 || norm == 0.0 {
            amplitude
        } else {
            amplitude * norm.powf(m - 2.0)
        };

        // Mandel scaling is linear, so scaling the six-vector componentwise is
        // the same as scaling the tensor — no re-derivation of the sqrt(2)
        // factors is needed or wanted here.
        let c = strain.components();
        Ok(AsterVoigt::from_components([
            coefficient * c[0],
            coefficient * c[1],
            coefficient * c[2],
            coefficient * c[3],
            coefficient * c[4],
            coefficient * c[5],
        ]))
    }
}

// ── Shared validation ────────────────────────────────────────────────────────

fn check_positive(value: f64, quantity: &'static str, unit: &'static str) -> Result<()> {
    if value > 0.0 {
        Ok(())
    } else {
        Err(OffbeatError::Unphysical {
            quantity,
            value,
            unit,
            reason: "must be strictly positive",
        })
    }
}

fn check_non_negative(
    value: f64,
    quantity: &'static str,
    unit: &'static str,
    reason: &'static str,
) -> Result<()> {
    if value >= 0.0 {
        Ok(())
    } else {
        Err(OffbeatError::Unphysical {
            quantity,
            value,
            unit,
            reason,
        })
    }
}

#[cfg(test)]
mod tests;
