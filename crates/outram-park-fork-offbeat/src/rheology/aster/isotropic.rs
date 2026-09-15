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
//! - The scalar radial return that solves for the plastic multiplier against a
//!   hardening curve — code_aster's `VMIS_ISOT_*` / `VISC_ISOT_*` family.
//!   Rate-**in**dependent; see the warning below. It is implemented as an
//!   inherent method on [`IsotropicHardening`], which lives in
//!   [`super::hardening`] because every law in this port shares it. `_LINE` is
//!   [`IsotropicHardening::Linear`] and `_PUIS` is
//!   [`IsotropicHardening::AsterPower`]; the return also accepts the three
//!   further curve families that module carries.
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
use crate::rheology::aster::hardening::IsotropicHardening;
use crate::rheology::aster::integration::{brent, LocalSolution, SolverControl};
use crate::rheology::aster::kinematics::AsterVoigt;

// ── The radial return, on the shared hardening curve ─────────────────────────

/// The scalar von Mises radial return of `nmisot`, added to the shared
/// [`IsotropicHardening`] curve.
///
/// The curve itself lives in [`super::hardening`] because every law in this
/// port needs it; the *return map* below is specific to `VMIS_ISOT_*` /
/// `VISC_ISOT_*`, so it stays here with the rest of that law's provenance.
/// Rust gathers both inherent `impl` blocks onto one rustdoc page, so a reader
/// hovering the type still sees the whole API at once.
impl IsotropicHardening {
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
    /// # Which solver, and why two variants are not iterated
    ///
    /// [`Perfect`](IsotropicHardening::Perfect) and
    /// [`Linear`](IsotropicHardening::Linear) admit the closed form
    /// `Δp = (σ_eq - σ_y - H p_m) / (H + 3μ)` — with `H = 0` for `Perfect` —
    /// which upstream also uses rather than iterating. It is exact, so
    /// iterating it would only add rounding.
    ///
    /// The three nonlinear curves have no closed form and are bracketed with
    /// [`brent`] on upstream's `nmcri2` residual. The bracket
    /// `[0, σ_eq / (3μ)]` straddles the root whenever the step is plastic and
    /// the curve is non-decreasing: at `Δp = 0` the residual is
    /// `R(p_m) - σ_eq < 0`, and at the upper end the `3μΔp` term alone already
    /// equals `σ_eq` while `R ≥ 0`, so the residual is positive. `Ludwik` and
    /// `AsterPower` are non-decreasing by construction; `EcroNl` is too for any
    /// parameter set with non-negative amplitudes, and [`brent`] reports an
    /// unbracketed root rather than returning a wrong one if a softening set is
    /// supplied.
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
    /// A sufficiently negative `modulus` (`H ≤ -3μ`) makes the residual
    /// non-increasing in `Δp`, so the return has no unique solution —
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
        let radius_at_start = self.value(accumulated_strain);

        // Upstream's `seuil = sieleq - rp`; `seuil <= 0` is the elastic branch.
        if trial_equivalent_stress - radius_at_start <= 0.0 {
            return Ok(None);
        }

        match *self {
            Self::Perfect { yield_stress } => self.closed_form_return(
                yield_stress,
                0.0,
                trial_equivalent_stress,
                three_mu,
                accumulated_strain,
            ),
            Self::Linear {
                yield_stress,
                modulus,
            } => self.closed_form_return(
                yield_stress,
                modulus,
                trial_equivalent_stress,
                three_mu,
                accumulated_strain,
            ),
            Self::Ludwik { .. } | Self::AsterPower { .. } | Self::EcroNl { .. } => {
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

    /// The exact return for a curve of constant slope `H`, shared by `Perfect`
    /// (`H = 0`) and `Linear`.
    fn closed_form_return(
        &self,
        yield_stress: f64,
        modulus: f64,
        trial_equivalent_stress: f64,
        three_mu: f64,
        accumulated_strain: f64,
    ) -> Result<Option<LocalSolution>> {
        let denominator = modulus + three_mu;
        if denominator <= 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "hardening modulus",
                value: modulus,
                unit: "Pa",
                reason: "softening at or beyond -3*mu makes the radial \
                         return non-unique",
            });
        }
        let delta_p =
            (trial_equivalent_stress - yield_stress - modulus * accumulated_strain) / denominator;
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
        self.value(accumulated_strain + delta_p) + three_shear_moduli * delta_p
            - trial_equivalent_stress
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
