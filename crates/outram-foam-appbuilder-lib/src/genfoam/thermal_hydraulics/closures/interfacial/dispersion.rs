// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/dispersionModels/
//             {dispersionModel.{H,C}, constant/constantDispersion.{H,C}}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `interfacial::dispersion` — turbulent dispersion coefficient
//!
//! Port of GeN-Foam's `dispersionModel` run-time-selectable family. Only one
//! concrete model ships upstream (`dispersionModels::constant`), so this is
//! (currently) a single-variant closed enum, same situation as
//! [`super::virtual_mass::VirtualMassCoefficient`].
//!
//! ## The fluid1-vs-fluid2 sign resolution
//!
//! Upstream's constructor resolves *which* fixed value to store at
//! dictionary-read time, not at evaluation time:
//!
//! ```text
//! value_ = (pair.fluid1().name() == dispersedPhaseName) ? dict["value"] : 1 - dict["value"]
//! ```
//!
//! i.e. the dictionary always specifies the coefficient *for the named
//! dispersed phase*; if the pair's `fluid1` happens to be that phase the raw
//! value is kept, otherwise its complement is stored (the model always reports
//! "dispersion of `fluid1`", per `dispersionModel::value()`'s doc comment:
//! "Return dispersion value of fluid1_ (ALWAYS fluid1)"). This is a
//! dictionary-parse-time decision (which fluid was named), not per-cell
//! physics, so it is reproduced as a constructor argument
//! ([`TurbulentDispersion::constant_for_fluid1`]) rather than deferred into the
//! evaluation call.

use uom::si::f64::Ratio;
use uom::si::ratio::ratio;

/// Turbulent dispersion coefficient for `fluid1` of an `FFPair` —
/// dimensionless (upstream's `dispersionModel::value()` always reports the
/// coefficient for `fluid1`; the complementary coefficient for `fluid2` is
/// `1 - value`, computed by the caller if needed).
///
/// Closed enum port of GeN-Foam's `dispersionModel` family. See the module
/// docs for why [`TurbulentDispersion::Constant`] is currently the only
/// variant, and for the fluid1/fluid2 resolution convention.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TurbulentDispersion {
    /// A fixed dispersion coefficient for `fluid1`, resolved once from the
    /// case dictionary (see [`TurbulentDispersion::constant_for_fluid1`]).
    Constant {
        /// The coefficient value for `fluid1` (dimensionless).
        value_for_fluid1: f64,
    },
}

impl TurbulentDispersion {
    /// Build a [`TurbulentDispersion::Constant`] from the dictionary-declared
    /// `dispersedPhase` value, reproducing GeN-Foam's `fluid1`-vs-`fluid2`
    /// resolution exactly.
    ///
    /// `dict_value` is the raw `value` keyword from
    /// `dispersionModel { type constant; dispersedPhase <name>; value <v>; }`.
    /// `dispersed_phase_is_fluid1` is `true` when the pair's `fluid1` is the
    /// named `dispersedPhase`; in that case the stored coefficient (always
    /// reported "for `fluid1`") is `dict_value` unchanged, otherwise it is `1 -
    /// dict_value`.
    #[must_use]
    pub fn constant_for_fluid1(dict_value: Ratio, dispersed_phase_is_fluid1: bool) -> Self {
        let v = dict_value.get::<ratio>();
        Self::Constant {
            value_for_fluid1: if dispersed_phase_is_fluid1 {
                v
            } else {
                1.0 - v
            },
        }
    }

    /// The (already-resolved) dispersion coefficient for `fluid1` of the pair.
    #[must_use]
    pub fn value_for_fluid1(&self) -> Ratio {
        let Self::Constant { value_for_fluid1 } = *self;
        Ratio::new::<ratio>(value_for_fluid1)
    }

    /// The complementary dispersion coefficient for `fluid2` of the pair
    /// (`1 - value_for_fluid1`; not a distinct upstream field, just the
    /// arithmetic complement a caller needs for the other phase).
    #[must_use]
    pub fn value_for_fluid2(&self) -> Ratio {
        let Self::Constant { value_for_fluid1 } = *self;
        Ratio::new::<ratio>(1.0 - value_for_fluid1)
    }
}
