// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from code_aster (https://gitlab.com/codeaster/src)
//   Copyright (C) 1991 - 2026 - EDF R&D
//   Licence: GPL-3.0-or-later
//   Upstream commit: b504ea08c2f49575e04644cee2e39a63ea45c16e
//   Sources:
//     bibfor/comport/ecpuis.F90          -- the ECRO_PUIS curve (`ecpuis`)
//     bibfor/comport/nmisot.F90          -- VMIS_ISOT_* dispatch (`nmisot`)
//     bibfor/algorith/lcgtn_module.F90   -- the ECRO_NL curve (`f_ecro`)
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! The isotropic hardening curve `R(p)`, shared by every law that needs one.
//!
//! # Why this module exists
//!
//! Two modules had grown their own `IsotropicHardening` enum independently —
//! [`super::isotropic`] for the `VMIS_ISOT_*` radial return, and
//! [`super::damage`] for the Rousselier and GTN porous-plastic laws. They
//! collided by name, so neither could be re-exported alongside the other, and
//! a caller reaching for "the hardening curve" had to know which law it was
//! about to feed. This module replaced both; those two now import from here.
//!
//! The obvious reading was that one was a duplicate. It was not. Their
//! `PowerLaw` variants are **different physics**: the `isotropic` one is
//! upstream's `ecpuis`, `R = σ_y + σ_y (E p / (α σ_y))^(1/n)`, while the
//! `damage` one is Ludwik, `R = σ_y + K p^n`. Merging them into a single
//! variant would have silently replaced one curve with the other, so they are
//! kept apart as [`AsterPower`](IsotropicHardening::AsterPower) and
//! [`Ludwik`](IsotropicHardening::Ludwik) — the names say which is which where
//! `PowerLaw` did not.
//!
//! So this module is the **union**, not the intersection: every curve family
//! either module offered, each kept distinct, under one type that both use.
//! The consolidation was behaviour-preserving in the curves themselves; what
//! it did change is that both callers now get the other's guards (the `p ≥ 0`
//! clamp and [`SLOPE_SINGULARITY_OFFSET`], previously only in `damage`) and
//! that [`radial_return`](IsotropicHardening::radial_return) — which lives in
//! [`super::isotropic`], next to its `nmisot` provenance — now accepts all five
//! curve families rather than two.
//!
//! # The curve
//!
//! `R(p)` is the radius of the yield surface at accumulated equivalent plastic
//! strain `p`. Every law here needs two things from it — its value, and its
//! slope `dR/dp`, which the local Newton solves differentiate through.
//!
//! # Units
//!
//! Every stress-dimensioned parameter is in pascal \[Pa\]; `p` and every
//! exponent are dimensionless. `p` is non-negative by construction: it is an
//! accumulated measure.

use crate::error::{OffbeatError, Result};

/// Below this accumulated plastic strain the `AsterPower` curve is replaced by
/// its secant through the origin.
///
/// Upstream's `p0 = 1.d-10` in `ecpuis.F90`, reproduced exactly. The reason is
/// that `dR/dp ∝ p^(1/n - 1)` diverges as `p → 0` for any `n > 1`, so a Newton
/// step taken at `p = 0` would see an infinite slope. Upstream replaces the
/// curve below `p0` with the straight line joining the origin to `R(p0)`, which
/// is finite-sloped and continuous with the curve at `p0`.
///
/// Note the curve is **C0 but not C1** there: the secant slope comes out
/// exactly `n` times the curve's own slope at `p0`.
pub const ASTER_POWER_LINEARISATION_STRAIN: f64 = 1.0e-10;

/// The accumulated plastic strain at which a curve with a genuinely infinite
/// initial slope is evaluated instead of at `p = 0`.
///
/// [`IsotropicHardening::Ludwik`] with `n < 1` and
/// [`IsotropicHardening::EcroNl`] with `γm < 1` both have `dR/dp → ∞` as
/// `p → 0`. That divergence is **real physics, not a coding error** — the
/// Ludwik fit really does rise vertically out of the origin. It is nonetheless
/// useless to a Newton step, which would propose an infinite correction, so
/// [`slope`](IsotropicHardening::slope) reports the slope at this offset
/// instead of an infinity.
///
/// Unlike [`ASTER_POWER_LINEARISATION_STRAIN`] this has **no upstream
/// counterpart**: code_aster reaches these two curves only through the
/// bracketed solves of the porous-plastic laws, which never ask for a slope at
/// the origin. It is this port's own guard, and it changes only the *slope*,
/// never the curve — [`value`](IsotropicHardening::value) is exact everywhere.
pub const SLOPE_SINGULARITY_OFFSET: f64 = 1.0e-12;

/// An isotropic hardening curve `R(p)`.
///
/// Enum dispatch rather than trait objects, per the workspace rule: the set of
/// curve families is closed, adding one forces every `match` to be revisited,
/// and rust-analyzer can navigate to each variant.
///
/// # Not covered
///
/// Tabulated `TRACTION` curves (upstream `rctrac`/`rsliso`). They need the
/// material data-table infrastructure rather than any new mechanics, so they
/// are left for a caller to interpolate rather than approximated here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IsotropicHardening {
    /// Perfect plasticity: `R(p) = σ_y`, no hardening at all.
    ///
    /// The limiting case, and the one that makes a return map's bracket
    /// degenerate — the residual becomes exactly zero at its upper endpoint —
    /// so it is worth testing against explicitly.
    Perfect {
        /// Initial yield stress `σ_y` \[Pa\], strictly positive.
        yield_stress: f64,
    },

    /// Linear hardening: `R(p) = σ_y + H p`.
    ///
    /// ASTER: the `_LINE` suffix of `VMIS_ISOT_LINE` / `VISC_ISOT_LINE`.
    /// The only family whose radial return has a closed form.
    Linear {
        /// Initial yield stress `σ_y` \[Pa\], strictly positive.
        yield_stress: f64,
        /// Plastic modulus `H` \[Pa\]. Negative values describe linear
        /// **softening** and are permitted, but they make the local solve
        /// non-monotone — see [`radial_return`](Self::radial_return).
        modulus: f64,
    },

    /// Ludwik power-law hardening: `R(p) = σ_y + K p^n`.
    ///
    /// The form the Rousselier and GTN porous-plastic laws use. **Distinct
    /// from [`AsterPower`](Self::AsterPower)** — see the module documentation.
    Ludwik {
        /// Initial yield stress `σ_y` \[Pa\], strictly positive.
        yield_stress: f64,
        /// Hardening coefficient `K` \[Pa\], non-negative.
        coefficient: f64,
        /// Hardening exponent `n` \[-\], typically 0.05–0.5 for structural
        /// steels.
        exponent: f64,
    },

    /// code_aster's `ECRO_PUIS` curve:
    /// `R(p) = σ_y + σ_y (E p / (α σ_y))^(1/n)`.
    ///
    /// ASTER: the `_PUIS` suffix of `VMIS_ISOT_PUIS`. Upstream: `ecpuis.F90`.
    /// **Distinct from [`Ludwik`](Self::Ludwik)** — this one carries Young's
    /// modulus and a dimensionless `α` inside the bracket, and is linearised
    /// below [`ASTER_POWER_LINEARISATION_STRAIN`].
    AsterPower {
        /// Initial yield stress `σ_y` \[Pa\], strictly positive.
        yield_stress: f64,
        /// Young's modulus `E` \[Pa\], strictly positive.
        youngs_modulus: f64,
        /// Dimensionless coefficient `α` \[-\], upstream `alfafa`. Strictly
        /// positive.
        alpha: f64,
        /// Hardening exponent `n` \[-\], strictly positive. Upstream stores its
        /// reciprocal as `unsurn`.
        exponent: f64,
    },

    /// code_aster's `ECRO_NL` nonlinear isotropic hardening, which GTN needs:
    ///
    /// `R(p) = R0 + RH p + R1(1 - e^(-γ₁p)) + R2(1 - e^(-γ₂p)) + RK(p + P0)^γm`
    ///
    /// Upstream: `f_ecro` in `lcgtn_module.F90`. The two saturating
    /// exponentials give the knee at small strain, the linear term the
    /// far-field slope, and the power term a tunable tail.
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
    /// Flow stress `R(p)` \[Pa\] at accumulated equivalent plastic strain `p`
    /// \[-\].
    ///
    /// `p` is an accumulated (monotone) measure and should be non-negative. A
    /// negative argument is **clamped to zero** rather than rejected, which is
    /// the physically meaningful extension — plastic strain never
    /// un-accumulates — and is needed because a Newton iterate driving one of
    /// the porous-plastic return maps can transiently overshoot below zero.
    #[must_use]
    pub fn value(self, p: f64) -> f64 {
        let p = p.max(0.0);
        match self {
            Self::Perfect { yield_stress } => yield_stress,
            Self::Linear {
                yield_stress,
                modulus,
            } => yield_stress + modulus * p,
            Self::Ludwik {
                yield_stress,
                coefficient,
                exponent,
            } => yield_stress + coefficient * p.powf(exponent),
            Self::AsterPower { .. } => self.aster_power_curve(p).0,
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
    /// `p` \[-\].
    ///
    /// The local Newton solves differentiate the curve, so this must be the
    /// exact derivative of [`value`](Self::value) — a mismatched slope
    /// degrades convergence silently rather than failing.
    ///
    /// Two variants have a genuinely infinite slope at the origin and are
    /// evaluated at [`SLOPE_SINGULARITY_OFFSET`] instead: `Ludwik` with
    /// `n < 1`, and `EcroNl` with `γm < 1`. `AsterPower` handles its own
    /// divergence differently, by upstream's secant linearisation below
    /// [`ASTER_POWER_LINEARISATION_STRAIN`]. As in [`value`](Self::value), a
    /// negative `p` is clamped to zero.
    #[must_use]
    pub fn slope(self, p: f64) -> f64 {
        let p = p.max(0.0);
        match self {
            Self::Perfect { .. } => 0.0,
            Self::Linear { modulus, .. } => modulus,
            Self::Ludwik {
                coefficient,
                exponent,
                ..
            } => {
                let q = if exponent < 1.0 {
                    p.max(SLOPE_SINGULARITY_OFFSET)
                } else {
                    p
                };
                coefficient * exponent * q.powf(exponent - 1.0)
            }
            Self::AsterPower { .. } => self.aster_power_curve(p).1,
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
                    (p + p0).max(SLOPE_SINGULARITY_OFFSET)
                } else {
                    p + p0
                };
                rh + r1 * gamma_1 * (-gamma_1 * p).exp()
                    + r2 * gamma_2 * (-gamma_2 * p).exp()
                    + rk * gamma_m * base.powf(gamma_m - 1.0)
            }
        }
    }

    /// The initial yield stress `R(0)` \[Pa\].
    #[must_use]
    pub fn yield_stress(self) -> f64 {
        match self {
            Self::Perfect { yield_stress }
            | Self::Linear { yield_stress, .. }
            | Self::Ludwik { yield_stress, .. }
            | Self::AsterPower { yield_stress, .. } => yield_stress,
            Self::EcroNl { r0, .. } => r0,
        }
    }

    /// The ASTER behaviour-name suffix this curve corresponds to, where one
    /// exists.
    ///
    /// Returned verbatim per the workspace naming rule, so a reader can match
    /// this port to upstream's `if` chain on the trailing characters directly.
    /// `Perfect` and `EcroNl` are not `VMIS_ISOT_*` suffixes and return `None`.
    #[must_use]
    pub fn aster_name_suffix(self) -> Option<&'static str> {
        match self {
            Self::Linear { .. } => Some("_LINE"),
            Self::AsterPower { .. } => Some("_PUIS"),
            Self::Perfect { .. } | Self::Ludwik { .. } | Self::EcroNl { .. } => None,
        }
    }

    /// The `ECRO_PUIS` curve and its slope, upstream `ecpuis`.
    ///
    /// Returns `(R(p), dR/dp)`. Below [`ASTER_POWER_LINEARISATION_STRAIN`] the
    /// curve is the secant through the origin, exactly as upstream does.
    fn aster_power_curve(self, p: f64) -> (f64, f64) {
        let Self::AsterPower {
            yield_stress,
            youngs_modulus,
            alpha,
            exponent,
        } = self
        else {
            unreachable!("aster_power_curve is only reachable from the AsterPower variant")
        };
        let inverse_n = 1.0 / exponent;
        let coefficient = youngs_modulus / (alpha * yield_stress);
        let p0 = ASTER_POWER_LINEARISATION_STRAIN;

        if p <= p0 {
            let radius_at_p0 = yield_stress * (coefficient * p0).powf(inverse_n) + yield_stress;
            let secant = (radius_at_p0 - yield_stress) / p0;
            (yield_stress + p * secant, secant)
        } else {
            let radius = yield_stress * (coefficient * p).powf(inverse_n) + yield_stress;
            let slope =
                inverse_n * yield_stress * coefficient * (coefficient * p).powf(inverse_n - 1.0);
            (radius, slope)
        }
    }

    /// Reject parameter sets that have no physical meaning.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] naming the offending quantity.
    pub fn validate(self) -> Result<()> {
        let positive = |value: f64, quantity: &'static str, unit: &'static str| -> Result<()> {
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
        };
        match self {
            Self::Perfect { yield_stress }
            | Self::Linear { yield_stress, .. }
            | Self::Ludwik { yield_stress, .. } => positive(yield_stress, "yield stress", "Pa"),
            Self::AsterPower {
                yield_stress,
                youngs_modulus,
                alpha,
                exponent,
            } => {
                positive(yield_stress, "yield stress", "Pa")?;
                positive(youngs_modulus, "Young's modulus", "Pa")?;
                positive(alpha, "power-law coefficient alpha", "-")?;
                positive(exponent, "power-law exponent n", "-")
            }
            Self::EcroNl { r0, .. } => positive(r0, "initial yield stress R0", "Pa"),
        }
    }
}

#[cfg(test)]
mod tests;
