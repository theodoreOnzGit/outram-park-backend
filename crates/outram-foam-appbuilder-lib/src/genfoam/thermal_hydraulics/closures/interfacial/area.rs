// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/interfacialAreaModels/
//             {spherical/sphericalInterfacialArea.{H,C}, annular/annularInterfacialArea.{H,C},
//             NoKazimi/NoKazimiInterfacialArea.{H,C}, Schor/SchorInterfacialArea.{H,C}}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `interfacial::area` — interfacial area concentration models
//!
//! Port of GeN-Foam's `interfacialAreaModels` family: given the local raw
//! (unnormalized) phase fractions of a dispersed/continuous fluid-fluid pair,
//! return the interfacial area concentration `a_i` (interface area per unit
//! mixture volume, [`InterfacialAreaConcentration`]) — the multiplier every
//! fluid-fluid drag and heat-transfer closure scales by.
//!
//! ## Model set (closed enum, no `dyn` dispatch)
//!
//! | Variant | Upstream | Geometry assumed |
//! |---|---|---|
//! | [`InterfacialArea::Spherical`] | `spherical` | Dispersed phase is spherical bubbles/droplets of diameter `D` |
//! | [`InterfacialArea::Annular`] | `annular` | Dispersed phase is a cylindrical (annular) core/film |
//! | [`InterfacialArea::NoKazimi`] | `NoKazimi` | Vapour in a triangular-pitch wire-wrapped rod bundle |
//! | [`InterfacialArea::Schor`] | `Schor` | Vapour in a triangular-pitch rod bundle, 3-region blend |
//!
//! ## A modelling note: "raw" vs "pair-normalized" phase fraction
//!
//! Upstream, `spherical`/`annular` read `pair.alphaDispersed()`/
//! `pair.alphaContinuous()` (both **raw**, i.e. fractions of the *whole* mesh
//! cell, which in general may host more than two fluids) and internally form
//! `a = alphaDispersed / (alphaDispersed + alphaContinuous)` — GeN-Foam calls
//! this the pair-*normalized* fraction. `NoKazimi`/`Schor` instead read the raw
//! vapour fraction directly (`vapour_[celli]`) and separately fetch
//! `vapour_.normalized()[celli]`, GeN-Foam's own name for exactly that same
//! `a`.
//!
//! This port unifies the four models on one signature,
//! [`InterfacialArea::area_concentration`], which takes the pair's two **raw**
//! fractions (`alpha_dispersed`, `alpha_continuous`) and derives the
//! pair-normalized fraction internally as `alpha_dispersed / (alpha_dispersed +
//! alpha_continuous)` for every variant — i.e. it assumes the pair's two fluids
//! are the only phases present in the cell (no third non-interacting phase). A
//! call site with a genuine three-phase mixture must pre-normalize
//! `alpha_continuous` to exclude the third phase before calling. This
//! assumption is explicit and testable (see `tests.rs`); it is not a
//! fabrication of new physics, only a simplification of GeN-Foam's general
//! multi-fluid registry lookup into a pure two-argument function.
//!
//! `dispersed_diameter` is used only by [`InterfacialArea::Spherical`] /
//! [`InterfacialArea::Annular`] (upstream reads it per-cell from a
//! [`super::diameter::BubbleDiameter`]/[`super::diameter::FilmDiameter`]
//! closure); [`InterfacialArea::NoKazimi`] / [`InterfacialArea::Schor`] carry
//! their own **fixed** pin diameter as a variant field (rod-bundle geometry is
//! a case constant, not a per-cell field) and ignore the argument.
//!
//! ## References
//!
//! - N. Zuber, "On the dispersed two-phase flow in the laminar flow regime,"
//!   *Chem. Eng. Sci.* 19(11), 1964 — spherical `a_i = 6*alpha/D` limit.
//! - H.C. No & M.S. Kazimi correlation for rod-bundle interfacial area (as
//!   implemented upstream; no public citation given in the GeN-Foam source).
//! - Schor et al. rod-bundle interfacial area correlation with a three-region
//!   (low/transition/high void-fraction) blend (as implemented upstream).

use super::units::{interfacial_area_concentration, FluidDiameter, InterfacialAreaConcentration};
use uom::si::f64::Ratio;
use uom::si::length::meter;
use uom::si::ratio::ratio;

/// Interfacial area concentration `a_i` correlation.
///
/// Closed enum port of GeN-Foam's `interfacialAreaModel` run-time-selectable
/// family. Evaluate with [`InterfacialArea::area_concentration`]. See the
/// module docs for the shared "raw vs. pair-normalized fraction" convention and
/// why `dispersed_diameter` is ignored by the rod-bundle variants.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InterfacialArea {
    /// Spherical bubble/droplet geometry: `a_i = 6*alpha/D` below `cutoff_alpha`,
    /// tapering to zero at `alpha = 1` above it (packing limit — spheres cannot
    /// fill space, so upstream forces `a_i -> 0` as the dispersed fraction
    /// approaches unity). The two branches are algebraically continuous at
    /// `alpha = cutoff_alpha` (the `(1-a)/(1-cutoff)` taper factor is exactly
    /// `1` there) — see `tests.rs`'s `spherical_cutoff_branch_is_continuous`.
    Spherical {
        /// Void/quality fraction above which the tapering correction applies
        /// (upstream default `0.9`).
        cutoff_alpha: f64,
    },
    /// Cylindrical (annular) core/film geometry: `a_i = 4*alpha/D` below
    /// `cutoff_alpha`, with a `(1 - sqrt(alpha))` taper above it.
    ///
    /// **Known upstream discontinuity, faithfully reproduced.** Unlike
    /// [`InterfacialArea::Spherical`]'s `(1-a)/(1-cutoff)` taper (which is
    /// exactly `1` at `a = cutoff_alpha`, so its two branches meet), annular's
    /// `(1-sqrt(a))/(1-cutoff)` taper does **not** reduce to `1` at
    /// `a = cutoff_alpha` — the value jumps by a factor of exactly
    /// `1/(1+sqrt(cutoff_alpha))` (about `0.513`, i.e. an abrupt ~49% drop, at
    /// the upstream default `cutoff_alpha = 0.9`) when crossing from the
    /// dilute branch into the tapered branch. This was verified against the
    /// upstream `.C` source (`annularInterfacialArea.C`) character-for-character
    /// — it is not a transcription error in this port, and is not "fixed"
    /// here per the workspace's guardrail against altering verified upstream
    /// behaviour without human sign-off. See `tests.rs`'s
    /// `annular_cutoff_branch_has_a_documented_jump_discontinuity` for the
    /// exact reproduction and a flag for human follow-up.
    Annular {
        /// Void/quality fraction above which the tapering correction applies
        /// (upstream default `0.9`).
        cutoff_alpha: f64,
    },
    /// No & Kazimi wire-wrapped triangular-pitch rod-bundle correlation.
    /// Build with [`InterfacialArea::no_kazimi_from_geometry`].
    NoKazimi {
        /// Precomputed geometry constant `A = 4*pi / (D*(2*sqrt(3)*(P/D)^2 - pi))`, 1/m.
        a_const: f64,
        /// Pin (rod) diameter, m.
        pin_diameter_m: f64,
    },
    /// Schor rod-bundle correlation with a three-region blend over the
    /// pair-normalized vapour fraction: `alpha_n < 0.55` (dilute), `0.55 <=
    /// alpha_n < 0.65` (linear transition), `alpha_n >= 0.65` (dense, tapered
    /// toward `cutoff_alpha`). Build with [`InterfacialArea::schor_from_geometry`].
    Schor {
        /// Pin diameter, m.
        pin_diameter_m: f64,
        /// Precomputed `A = 2*sqrt(3)*(P/D)^2` geometry constant (dimensionless).
        a_const: f64,
        /// Void fraction above which the dense-region taper applies (upstream
        /// default `0.957`).
        cutoff_alpha: f64,
        /// Minimum allowable `a_i` at large void fraction, 1/m (upstream
        /// default `0.0`).
        min_area_at_large_alpha: f64,
        /// Precomputed interpolation value at `alpha_n = 0.55`, 1/m.
        ia1: f64,
        /// Precomputed interpolation value at `alpha_n = 0.65`, 1/m.
        ia2: f64,
    },
}

/// GeN-Foam's `Schor` model hardcodes this seven-digit truncation of pi rather
/// than the full-precision value; reproduced exactly so the port matches
/// upstream bit-for-bit rather than merely "close enough" (see the
/// module-level V&V notes in `tests.rs`). `clippy::approx_constant` is
/// suppressed deliberately: this is not a typo of `core::f64::consts::PI`
/// (which [`InterfacialArea::no_kazimi_from_geometry`] uses instead, for the
/// same reason — upstream's own two source files make different choices) —
/// it is upstream's literal, reproduced on purpose.
#[allow(clippy::approx_constant)]
const SCHOR_PI: f64 = 3.141_592_7;

/// Schor's fixed low/high pair-normalized-void-fraction blend boundaries.
const SCHOR_ALPHA_1: f64 = 0.55;
const SCHOR_ALPHA_2: f64 = 0.65;

impl InterfacialArea {
    /// Evaluate the interfacial area concentration for the given pair state.
    ///
    /// `alpha_dispersed`/`alpha_continuous` are the **raw** (whole-cell) phase
    /// fractions of the dispersed and continuous fluid of this pair (see the
    /// module docs). `dispersed_diameter` is the dispersed phase's
    /// characteristic diameter (from a [`super::diameter::BubbleDiameter`]
    /// closure); ignored by the rod-bundle variants.
    #[must_use]
    pub fn area_concentration(
        &self,
        alpha_dispersed: Ratio,
        alpha_continuous: Ratio,
        dispersed_diameter: FluidDiameter,
    ) -> InterfacialAreaConcentration {
        interfacial_area_concentration(self.value(
            alpha_dispersed.get::<ratio>(),
            alpha_continuous.get::<ratio>(),
            dispersed_diameter.get::<meter>(),
        ))
    }

    /// Core scalar evaluation on bare SI magnitudes.
    fn value(&self, alpha_d: f64, alpha_c: f64, d_disp: f64) -> f64 {
        match *self {
            Self::Spherical { cutoff_alpha } => {
                let (alpha_sum, a) = pair_normalize(alpha_d, alpha_c);
                alpha_sum
                    * if a < cutoff_alpha {
                        6.0 * a / d_disp
                    } else {
                        6.0 * a / d_disp * (1.0 - a) / (1.0 - cutoff_alpha)
                    }
            }
            Self::Annular { cutoff_alpha } => {
                let (alpha_sum, a) = pair_normalize(alpha_d, alpha_c);
                alpha_sum
                    * if a < cutoff_alpha {
                        4.0 * a / d_disp
                    } else {
                        4.0 * a / d_disp * (1.0 - a.sqrt()) / (1.0 - cutoff_alpha)
                    }
            }
            Self::NoKazimi { a_const, .. } => {
                let (_, a_n) = pair_normalize(alpha_d, alpha_c);
                a_const * alpha_d * ((1.0 - a_n) / 0.043).min(1.0)
            }
            Self::Schor {
                pin_diameter_m: d,
                a_const,
                cutoff_alpha,
                min_area_at_large_alpha,
                ia1,
                ia2,
            } => {
                let (_, alpha_n) = pair_normalize(alpha_d, alpha_c);
                let scale_factor = alpha_d / alpha_n.max(1e-9);
                let piecewise = if alpha_n < SCHOR_ALPHA_1 {
                    (4.0 / d) * (SCHOR_PI * alpha_n / (3.0 * (a_const - SCHOR_PI))).sqrt()
                } else if alpha_n < SCHOR_ALPHA_2 {
                    ((SCHOR_ALPHA_2 - alpha_n) * ia1 + (alpha_n - SCHOR_ALPHA_1) * ia2)
                        / (SCHOR_ALPHA_2 - SCHOR_ALPHA_1)
                } else {
                    let dense = (4.0 / d) * SCHOR_PI.sqrt() / (3.0 * (a_const - SCHOR_PI))
                        * ((1.0 - alpha_n) * a_const + SCHOR_PI * alpha_n).sqrt()
                        * ((1.0 - alpha_n) / (1.0 - cutoff_alpha)).min(1.0);
                    dense.max(min_area_at_large_alpha / scale_factor)
                };
                scale_factor * piecewise
            }
        }
    }

    /// Build [`InterfacialArea::NoKazimi`] from rod-bundle geometry, reproducing
    /// GeN-Foam's coefficient derivation exactly.
    ///
    /// Units: `pin_diameter_m`, `pin_pitch_m` in metres.
    #[must_use]
    pub fn no_kazimi_from_geometry(pin_diameter_m: f64, pin_pitch_m: f64) -> Self {
        let pd = pin_pitch_m / pin_diameter_m;
        let a_const = 4.0 * core::f64::consts::PI
            / (pin_diameter_m * (2.0 * 3.0_f64.sqrt() * pd * pd - core::f64::consts::PI));
        Self::NoKazimi {
            a_const,
            pin_diameter_m,
        }
    }

    /// Build [`InterfacialArea::Schor`] from rod-bundle geometry and the two
    /// tuning knobs GeN-Foam exposes (`cutoffAlpha`, `minInterfacialAreaAtLargeAlpha`),
    /// reproducing GeN-Foam's coefficient derivation exactly.
    ///
    /// Units: `pin_diameter_m`, `pin_pitch_m` in metres;
    /// `min_area_at_large_alpha` in 1/m. Pass GeN-Foam's defaults
    /// (`cutoff_alpha = 0.957`, `min_area_at_large_alpha = 0.0`) to match a
    /// dictionary that omits them.
    #[must_use]
    pub fn schor_from_geometry(
        pin_diameter_m: f64,
        pin_pitch_m: f64,
        cutoff_alpha: f64,
        min_area_at_large_alpha: f64,
    ) -> Self {
        let pd = pin_pitch_m / pin_diameter_m;
        let a_const = 2.0 * 3.0_f64.sqrt() * pd * pd;
        let ia1 = (4.0 / pin_diameter_m)
            * (SCHOR_PI * SCHOR_ALPHA_1 / (3.0 * (a_const - SCHOR_PI))).sqrt();
        let ia2 = (4.0 / pin_diameter_m) * SCHOR_PI.sqrt() / (3.0 * (a_const - SCHOR_PI))
            * ((1.0 - SCHOR_ALPHA_2) * a_const + SCHOR_PI * SCHOR_ALPHA_2).sqrt();
        Self::Schor {
            pin_diameter_m,
            a_const,
            cutoff_alpha,
            min_area_at_large_alpha,
            ia1,
            ia2,
        }
    }
}

/// Shared helper: `(alpha_dispersed + alpha_continuous, alpha_dispersed /
/// (alpha_dispersed + alpha_continuous))` — GeN-Foam's "pair sum" and
/// "pair-normalized fraction" (see the module-level modelling note).
fn pair_normalize(alpha_d: f64, alpha_c: f64) -> (f64, f64) {
    let alpha_sum = alpha_d + alpha_c;
    (alpha_sum, alpha_d / alpha_sum)
}
