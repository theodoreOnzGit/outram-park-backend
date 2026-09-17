// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/dragModels/
//             FSDragCoefficientModels/  (Churchill, Colebrook, Rehme,
//             ReynoldsPower, Engel, modifiedEngel, BaxiDalleDonne, NoKazimi)
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `fs_drag` — fluid-structure wall-friction factor correlations
//!
//! Port of GeN-Foam's `FSDragCoefficientModels` family. Each model maps a local
//! [`ReynoldsNumber`] to a **Darcy friction factor** ([`DarcyFrictionFactor`]),
//! the `f` in the channel pressure gradient
//!
//! ```text
//! dp/dx = - f * rho * |U| * U / (2 * D_h)
//! ```
//!
//! In GeN-Foam this scalar is one component of the anisotropic fluid-structure
//! drag tensor `Kd` assembled per cell in the porous momentum equation
//! (`UEqn`); assembling `Kd` from these factors belongs to the solver bead
//! (op-p6p.7.11) and is out of scope here. This module ports the **correlations
//! themselves**, which are pure algebra in `Re` plus fixed geometric
//! coefficients — no mesh or field state — and are therefore independently
//! verifiable against published values and analytical limits.
//!
//! ## Model set (closed enum, no `dyn` dispatch)
//!
//! | Variant | Upstream | Intended flow geometry |
//! |---|---|---|
//! | [`FsWallFriction::Churchill`] | `Churchill` | Smooth/rough pipe, all-`Re` (Churchill 1977) |
//! | [`FsWallFriction::Colebrook`] | `Colebrook` | Power-law fit `f = (a·log10 Re + b)^c` |
//! | [`FsWallFriction::ReynoldsPower`] | `ReynoldsPower` | Generic power law `f = a·Re^b + c` |
//! | [`FsWallFriction::Rehme`] | `Rehme` | Wire-wrapped rod bundle (Rehme 1973) |
//! | [`FsWallFriction::Engel`] | `Engel` | Wire-wrapped rod bundle (Engel et al. 1979) |
//! | [`FsWallFriction::ModifiedEngel`] | `modifiedEngel` | Modified Engel bundle |
//! | [`FsWallFriction::BaxiDalleDonne`] | `BaxiDalleDonne` | Wire-wrapped bundle (Baxi & Dalle Donne) |
//! | [`FsWallFriction::NoKazimi`] | `NoKazimi` | Wire-wrapped bundle (No & Kazimi) |
//!
//! The bundle correlations ([`FsWallFriction::Rehme`], `BaxiDalleDonne`,
//! `NoKazimi`) precompute fixed coefficients from the wire-wrap geometry; use
//! the `*_from_geometry` constructors to reproduce GeN-Foam's derivation exactly.
//!
//! ## References
//!
//! - S.W. Churchill, "Friction-factor equation spans all fluid-flow regimes",
//!   *Chem. Eng.* 84(24), 1977, pp. 91-92.
//! - K. Rehme, "Pressure drop correlations for fuel element spacers",
//!   *Nucl. Technol.* 17, 1973, pp. 15-23.
//! - Y.S. Tang, R.D. Coffield, R.A. Markley / F.C. Engel, R.A. Markley,
//!   A.A. Bishop, "Laminar, transition, and turbulent parallel flow pressure
//!   drop across wire-wrap-spaced rod bundles", *Nucl. Sci. Eng.* 69, 1979.

use crate::genfoam::thermal_hydraulics::units::{DarcyFrictionFactor, ReynoldsNumber};
use uom::si::ratio::ratio;

#[cfg(test)]
mod tests;

/// Fluid-structure (wall) Darcy friction-factor correlation.
///
/// Closed enum port of GeN-Foam's `FSDragCoefficientModel` run-time-selectable
/// family. Evaluate with [`FsWallFriction::friction_factor`], which takes the
/// local [`ReynoldsNumber`] and returns the [`DarcyFrictionFactor`].
///
/// Each variant carries the parameters GeN-Foam reads from its `dragModel`
/// dictionary (or, for the bundle models, the coefficients derived once from the
/// wire-wrap geometry — see the `*_from_geometry` constructors).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FsWallFriction {
    /// Churchill (1977) all-regime correlation. `surface_roughness` is the
    /// relative roughness `epsilon = e / D_h` (dimensionless); use `0.0` for a
    /// hydraulically smooth wall.
    Churchill {
        /// Relative surface roughness `epsilon = e / D_h` (dimensionless).
        surface_roughness: f64,
    },
    /// Power-law-in-`log10(Re)` fit `f = (coeff · log10(Re) + constant)^exp`.
    /// (GeN-Foam names this model `Colebrook`; it is an explicit fit, not the
    /// implicit Colebrook-White equation.)
    Colebrook {
        /// Multiplier on `log10(Re)`.
        coeff: f64,
        /// Additive constant inside the power.
        constant: f64,
        /// Outer exponent.
        exp: f64,
    },
    /// Generic power law `f = coeff · Re^exp + constant`.
    ReynoldsPower {
        /// Leading coefficient.
        coeff: f64,
        /// Reynolds-number exponent (typically negative).
        exp: f64,
        /// Additive constant.
        constant: f64,
    },
    /// Rehme (1973) wire-wrapped-bundle correlation
    /// `f = a · (b1 / Re + b2 / Re^0.133)`. Build with
    /// [`FsWallFriction::rehme_from_geometry`].
    Rehme {
        /// Wetted-perimeter split factor `a` (dimensionless).
        a: f64,
        /// Laminar-branch coefficient `b1`.
        b1: f64,
        /// Turbulent-branch coefficient `b2`.
        b2: f64,
    },
    /// Engel et al. (1979) wire-wrapped-bundle correlation with a
    /// laminar/turbulent blend over `400 < Re < 5000`. No parameters.
    Engel,
    /// Modified Engel bundle correlation (different laminar/turbulent constants),
    /// same `400 < Re < 5000` blend. No parameters.
    ModifiedEngel,
    /// Baxi & Dalle Donne wire-wrapped-bundle correlation, blended over
    /// `400 < Re < 5000`. Build with
    /// [`FsWallFriction::baxi_dalle_donne_from_geometry`].
    BaxiDalleDonne {
        /// Laminar-branch coefficient `a`.
        a: f64,
        /// Turbulent-branch coefficient `b`.
        b: f64,
        /// Turbulent-branch coefficient `c`.
        c: f64,
    },
    /// No & Kazimi wire-wrapped-bundle correlation, blended over
    /// `400 < Re < 2600`. Build with
    /// [`FsWallFriction::no_kazimi_from_geometry`].
    NoKazimi {
        /// Laminar-branch coefficient `a`.
        a: f64,
        /// Turbulent-branch coefficient `b`.
        b: f64,
        /// Turbulent-branch coefficient `c`.
        c: f64,
    },
}

impl FsWallFriction {
    /// Evaluate the Darcy friction factor at the given Reynolds number.
    ///
    /// Faithful translation of each upstream model's `value(celli)` member.
    /// `re` must be positive (a physical Reynolds number); at `Re -> 0` the
    /// laminar branches diverge like `1/Re`, exactly as upstream.
    #[must_use]
    pub fn friction_factor(&self, re: ReynoldsNumber) -> DarcyFrictionFactor {
        DarcyFrictionFactor::new::<ratio>(self.value(re.get::<ratio>()))
    }

    /// Core scalar evaluation on the bare (dimensionless) Reynolds number.
    fn value(&self, re: f64) -> f64 {
        match *self {
            Self::Churchill { surface_roughness } => churchill(re, surface_roughness),
            Self::Colebrook {
                coeff,
                constant,
                exp,
            } => (coeff * re.log10() + constant).powf(exp),
            Self::ReynoldsPower {
                coeff,
                exp,
                constant,
            } => coeff * re.powf(exp) + constant,
            Self::Rehme { a, b1, b2 } => a * (b1 / re + b2 / re.powf(0.133)),
            Self::Engel => blend_400(re, 99.0 / re, 0.48 / re.powf(0.25), 5000.0),
            Self::ModifiedEngel => blend_400(re, 110.0 / re, 0.37 / re.powf(0.25), 5000.0),
            Self::BaxiDalleDonne { a, b, c } => {
                let ft = 0.316 * (b + c * re.powf(0.086)) / re.powf(0.25);
                blend_400(re, a / re, ft, 5000.0)
            }
            Self::NoKazimi { a, b, c } => {
                let ft = 0.316 * (b + c * re.powf(0.086)) / re.powf(0.25);
                blend_400(re, a / re, ft, 2600.0)
            }
        }
    }

    /// Build a [`FsWallFriction::Rehme`] from wire-wrapped-bundle geometry,
    /// reproducing GeN-Foam's coefficient derivation exactly.
    ///
    /// Units: `pin_diameter`, `wire_diameter`, `wire_lead_length`, and
    /// `wetted_wrap_perimeter` in metres; `number_of_pins` dimensionless.
    #[must_use]
    pub fn rehme_from_geometry(
        number_of_pins: f64,
        pin_diameter: f64,
        wire_diameter: f64,
        wire_lead_length: f64,
        wetted_wrap_perimeter: f64,
    ) -> Self {
        let pt = pin_diameter + 1.0444 * wire_diameter;
        let wetted_pin_perimeter =
            number_of_pins * core::f64::consts::PI * (pin_diameter + wire_diameter);
        let ratio_pd = pt / pin_diameter;
        let b = ratio_pd.sqrt()
            + (7.6 * (pin_diameter + wire_diameter) * ratio_pd * ratio_pd / wire_lead_length)
                .powf(2.16);
        Self::Rehme {
            a: wetted_pin_perimeter / (wetted_pin_perimeter + wetted_wrap_perimeter),
            b1: 64.0 * b.sqrt(),
            b2: 0.0816 * b.powf(0.9335),
        }
    }

    /// Build a [`FsWallFriction::BaxiDalleDonne`] from wire-wrapped-bundle
    /// geometry (`pin_diameter`, `wire_diameter`, `wire_lead_length` in metres).
    #[must_use]
    pub fn baxi_dalle_donne_from_geometry(
        pin_diameter: f64,
        wire_diameter: f64,
        wire_lead_length: f64,
    ) -> Self {
        let pt = pin_diameter + 1.0444 * wire_diameter;
        let ratio_pd = pt / pin_diameter;
        Self::BaxiDalleDonne {
            a: (80.0 / (100.0 * wire_lead_length).sqrt()) * ratio_pd.powf(1.5),
            b: 1.034 / ratio_pd.powf(0.124),
            c: 29.7 * ratio_pd.powf(6.9)
                / (wire_lead_length / (pin_diameter + wire_diameter)).powf(2.239),
        }
    }

    /// Build a [`FsWallFriction::NoKazimi`] from wire-wrapped-bundle geometry
    /// (`pin_diameter`, `wire_diameter`, `wire_lead_length` in metres). Differs
    /// from [`FsWallFriction::baxi_dalle_donne_from_geometry`] only in the
    /// laminar coefficient (`32` vs `80`).
    #[must_use]
    pub fn no_kazimi_from_geometry(
        pin_diameter: f64,
        wire_diameter: f64,
        wire_lead_length: f64,
    ) -> Self {
        let pt = pin_diameter + 1.0444 * wire_diameter;
        let ratio_pd = pt / pin_diameter;
        Self::NoKazimi {
            a: (32.0 / (100.0 * wire_lead_length).sqrt()) * ratio_pd.powf(1.5),
            b: 1.034 / ratio_pd.powf(0.124),
            c: 29.7 * ratio_pd.powf(6.9)
                / (wire_lead_length / (pin_diameter + wire_diameter)).powf(2.239),
        }
    }
}

/// Churchill (1977) all-regime Darcy friction factor.
///
/// `f = 8 · [ (8/Re)^12 + 1/(A+B)^1.5 ]^(1/12)` with
/// `A = (-2.457 · ln[(7/Re)^0.9 + 0.27·eps])^16` and `B = (37530/Re)^16`.
/// The `(8/Re)^12` term recovers the laminar `f = 64/Re` limit; the `A`/`B`
/// terms recover the turbulent (rough-pipe) limit.
fn churchill(re: f64, surface_roughness: f64) -> f64 {
    let a = (-2.457 * ((7.0 / re).powf(0.9) + 0.27 * surface_roughness).ln()).powi(16);
    let b = (37530.0 / re).powi(16);
    8.0 * ((8.0 / re).powi(12) + 1.0 / (a + b).powf(1.5)).powf(1.0 / 12.0)
}

/// Laminar/turbulent blend used by the wire-wrapped-bundle correlations.
///
/// Matches GeN-Foam exactly: below `Re = 400` return the laminar branch `fl`;
/// above `re_hi` return the turbulent branch `ft`; in between blend with
/// `psi = clamp((Re-400)/(re_hi-400), 0, 1)` as
/// `f = sqrt(psi)·ft + sqrt(1-psi)·fl`.
fn blend_400(re: f64, fl: f64, ft: f64, re_hi: f64) -> f64 {
    if re > 400.0 {
        if re < re_hi {
            let psi = ((re - 400.0) / (re_hi - 400.0)).clamp(0.0, 1.0);
            psi.sqrt() * ft + (1.0 - psi).sqrt() * fl
        } else {
            ft
        }
    } else {
        fl
    }
}
