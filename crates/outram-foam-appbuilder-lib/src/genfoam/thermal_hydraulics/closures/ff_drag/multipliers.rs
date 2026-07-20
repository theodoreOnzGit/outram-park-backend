// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/dragModels/
//             twoPhaseDragMultiplierModels/ (ChenKalish, constant, Kaiser74,
//             Kaiser88, KottowskiSavatteri, LockhartMartinelli, LottesFlinn,
//             LottesFlinnNguyen); base class twoPhaseDragMultiplierModel.C/.H
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (sradman@protonmail.com; EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # Two-phase friction/drag multiplier correlations
//!
//! Port of GeN-Foam's `twoPhaseDragMultiplierModels` family (base class
//! `twoPhaseDragMultiplierModel`). Each model maps the local two-phase flow
//! state — the "multiplier fluid" phase fraction and/or the local
//! Lockhart-Martinelli parameter `X` — to a **dimensionless two-phase
//! friction multiplier** `phi^2`, the factor by which a single-phase
//! (fluid-structure) drag is scaled up to account for the presence of the
//! second phase:
//!
//! ```text
//! Kd_total = phi^2 * Kd_single_phase
//! ```
//!
//! Assembling `Kd_total` from `phi^2` and the single-phase `Kd` tensor field
//! (`twoPhaseDragMultiplierModel::correctField`) is the solver's job and out
//! of scope here; this module ports the **pure-algebra `phi2(celli)`
//! correlations**.
//!
//! `X`, the Lockhart-Martinelli parameter, is itself computed upstream in
//! `FFPair::correct` from phase viscosities, densities, and flow qualities
//! (`X_i = (mu_i/mu_j)^0.1 * (Xflow_i/Xflow_j)^0.9 * sqrt(rho_j/rho_i)`) —
//! that computation belongs to the fluid-pair module, not this drag-closure
//! port, so `X` is taken here as an input (already assembled per cell).
//!
//! ## Single-phase clamp
//!
//! Every model except [`TwoPhaseDragMultiplier::LottesFlinnNguyen`] uses the
//! shared upstream `onePhase(celli)` guard: when the multiplier-fluid phase
//! fraction exceeds `0.9999`, `phi2 = 1` unconditionally (flow is
//! essentially single-phase, so no two-phase correction applies).
//! `LottesFlinnNguyen` instead uses its own inline threshold of `0.999`. This
//! is a genuine upstream inconsistency (not a typo we can safely "fix" without
//! changing behaviour), so it is preserved here rather than silently
//! unified — see [`TwoPhaseDragMultiplier::multiplier`].
//!
//! ## Model set (closed enum, no `dyn` dispatch)
//!
//! | Variant | Upstream | Reference form |
//! |---|---|---|
//! | [`TwoPhaseDragMultiplier::ChenKalish`] | `ChenKalish` | `phi2 = exp(2(0.0867 ln^2 X - 0.518 ln X + 1.59))` |
//! | [`TwoPhaseDragMultiplier::Constant`] | `constant` | fixed `phi2` |
//! | [`TwoPhaseDragMultiplier::Kaiser74`] | `Kaiser74` | `phi2 = 67.24 / X^1.1` |
//! | [`TwoPhaseDragMultiplier::Kaiser88`] | `Kaiser88` | `phi2 = exp(2(1.48 - 1.05 L + 0.09 L^2))`, `L = ln(sqrt(X))` |
//! | [`TwoPhaseDragMultiplier::KottowskiSavatteri`] | `KottowskiSavatteri` | `phi2 = 10^(2(0.1046 log10^2 X - 0.5098 log10 X + 0.6252))` |
//! | [`TwoPhaseDragMultiplier::LockhartMartinelli`] | `LockhartMartinelli` | Chisholm (1967) `phi2 = 1 + C/X + 1/X^2` |
//! | [`TwoPhaseDragMultiplier::LottesFlinn`] | `LottesFlinn` | `phi2 = alpha^-exp` (homogeneous void-fraction form) |
//! | [`TwoPhaseDragMultiplier::LottesFlinnNguyen`] | `LottesFlinnNguyen` | `phi2 = (1 - (1+X^0.8)^-0.378)^-exp` |
//!
//! `ChenKalish`, `Kaiser74`, `Kaiser88`, and `KottowskiSavatteri` clamp `X` to
//! `[0.07, 30]` before evaluating (matching upstream, which notes the
//! correlations are only valid in that range); `LockhartMartinelli` and
//! `LottesFlinnNguyen` use `X` unclamped, exactly as upstream.
//!
//! Evaluate with [`TwoPhaseDragMultiplier::multiplier`].
//!
//! ## References
//!
//! - D. Chisholm, "A theoretical basis for the Lockhart-Martinelli
//!   correlation for two-phase two-component flow", *Int. J. Heat Mass
//!   Transfer* 10, 1967, pp. 1767-1778.
//! - R.W. Lockhart, R.C. Martinelli, "Proposed correlation of data for
//!   isothermal two-phase, two-component flow in pipes", *Chem. Eng. Prog.*
//!   45, 1949, pp. 39-48.
//! - P.A. Lottes, W.S. Flinn, "A method of analysis of natural circulation
//!   boiling systems", *Nucl. Sci. Eng.* 1, 1956, pp. 461-476.

use uom::si::f64::Ratio;
use uom::si::ratio::ratio;

/// Upstream `onePhase(celli)` threshold used by every variant except
/// [`TwoPhaseDragMultiplier::LottesFlinnNguyen`].
const ONE_PHASE_THRESHOLD: f64 = 0.9999;

/// `LottesFlinnNguyen`'s own inline single-phase threshold (see the module
/// doc's "Single-phase clamp" section).
const LOTTES_FLINN_NGUYEN_ONE_PHASE_THRESHOLD: f64 = 0.999;

/// Two-phase friction/drag multiplier correlation `phi^2`.
///
/// Closed enum port of GeN-Foam's `twoPhaseDragMultiplierModel` run-time-
/// selectable family. Evaluate with
/// [`TwoPhaseDragMultiplier::multiplier`], which takes the multiplier-fluid
/// phase fraction and the Lockhart-Martinelli parameter `X` (both
/// dimensionless [`Ratio`]s) and returns `phi^2` as a [`Ratio`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TwoPhaseDragMultiplier {
    /// Chen & Kalish correlation, log-quadratic in `ln(X)`.
    ChenKalish,
    /// Fixed multiplier, read from the GeN-Foam `value` dictionary entry
    /// (no default upstream — must be supplied).
    Constant {
        /// The fixed `phi^2` value returned for any two-phase cell.
        phi2: f64,
    },
    /// Kaiser (1974) correlation `phi2 = 67.24 / X^1.1`, derived upstream
    /// from `phi = 8.2 / X^0.55` (i.e. `phi2 = phi^2 = 8.2^2 / X^1.1 =
    /// 67.24 / X^1.1`).
    Kaiser74,
    /// Kaiser (1988) correlation, log-quadratic in `ln(sqrt(X))`.
    Kaiser88,
    /// Kottowski-Savatteri correlation, log10-quadratic in `X`.
    KottowskiSavatteri,
    /// Lockhart & Martinelli (1949) / Chisholm (1967) correlation
    /// `phi2 = min(1 + C/X + 1/X^2, max_phi2)`.
    LockhartMartinelli {
        /// Chisholm's `C` coefficient. GeN-Foam dictionary default: `20`
        /// (turbulent-turbulent).
        c: f64,
        /// Clamp on the returned multiplier. GeN-Foam dictionary default:
        /// `1000` (the base class's `maxPhi2` entry).
        max_phi2: f64,
    },
    /// Lottes & Flinn (1956) homogeneous void-fraction form
    /// `phi2 = alpha^-exp`, where `alpha` is the multiplier-fluid phase
    /// fraction (not `X`).
    LottesFlinn {
        /// Exponent. GeN-Foam dictionary default: `2.0`.
        exp: f64,
    },
    /// Lottes-Flinn-Nguyen form `phi2 = (1 - (1 + X^0.8)^-0.378)^-exp`. Uses
    /// its own `0.999` single-phase threshold instead of the shared
    /// `0.9999` — see the module doc.
    LottesFlinnNguyen {
        /// Exponent. GeN-Foam dictionary default: `2.0`.
        exp: f64,
    },
}

impl TwoPhaseDragMultiplier {
    /// Evaluate the two-phase multiplier `phi^2` at the given local state.
    ///
    /// `phase_fraction` is the multiplier-fluid phase fraction (GeN-Foam's
    /// `mFluidPtr_->normalized()[celli]`); `x_lm` is the local
    /// Lockhart-Martinelli parameter (see the module doc). Faithful
    /// translation of each upstream model's `phi2(celli)` member, including
    /// the single-phase clamp.
    #[must_use]
    pub fn multiplier(&self, phase_fraction: Ratio, x_lm: Ratio) -> Ratio {
        let a = phase_fraction.get::<ratio>();
        let threshold = if matches!(self, Self::LottesFlinnNguyen { .. }) {
            LOTTES_FLINN_NGUYEN_ONE_PHASE_THRESHOLD
        } else {
            ONE_PHASE_THRESHOLD
        };
        if a > threshold {
            return Ratio::new::<ratio>(1.0);
        }
        Ratio::new::<ratio>(self.value(a, x_lm.get::<ratio>()))
    }

    /// Core scalar evaluation (two-phase branch only), unit-stripped to bare
    /// `f64`.
    fn value(&self, phase_fraction: f64, x_lm: f64) -> f64 {
        match *self {
            Self::ChenKalish => {
                let ln_x = x_lm.clamp(0.07, 30.0).ln();
                (2.0 * (0.0867 * ln_x * ln_x - 0.518 * ln_x + 1.59)).exp()
            }
            Self::Constant { phi2 } => phi2,
            Self::Kaiser74 => 67.24 / x_lm.clamp(0.07, 30.0).powf(1.1),
            Self::Kaiser88 => {
                let log_sqrt_x = x_lm.clamp(0.07, 30.0).sqrt().ln();
                (2.0 * (1.48 - 1.05 * log_sqrt_x + 0.09 * log_sqrt_x * log_sqrt_x)).exp()
            }
            Self::KottowskiSavatteri => {
                let log10_x = x_lm.clamp(0.07, 30.0).log10();
                10f64.powf(2.0 * (0.1046 * log10_x * log10_x - 0.5098 * log10_x + 0.6252))
            }
            Self::LockhartMartinelli { c, max_phi2 } => {
                (1.0 + c / x_lm + 1.0 / (x_lm * x_lm)).min(max_phi2)
            }
            Self::LottesFlinn { exp } => phase_fraction.max(1e-9).powf(-exp),
            Self::LottesFlinnNguyen { exp } => {
                (1.0 - (1.0 + x_lm.powf(0.8)).powf(-0.378)).powf(-exp)
            }
        }
    }
}
