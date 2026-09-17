// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/XS/XS/include/readNuclearData.H
//                    and include/setNeutronicsVariables.H (the `xsVariables`
//                    dictionary + per-variable transform law)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Thomas Guilbaud (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # Feedback parameters (`xsVariables`) and their transform laws
//!
//! A GeN-Foam `nuclearData` file declares an `xsVariables` sub-dictionary that
//! names the feedback parameters the cross sections depend on and, for each, a
//! **transform law** applied to the raw physical value *before* it enters the
//! polyharmonic-spline interpolation. Choosing the law linearises the physical
//! dependence so a low-order spline fits well:
//!
//! - `lin`  — identity; use for quantities the XS vary linearly in
//!   (coolant density, expansion).
//! - `log`  — natural log; the classic **fast-spectrum Doppler** law, where
//!   resonance absorption varies as `ln(T_fuel)`.
//! - `sqrt` — square root; the **thermal-spectrum Doppler** law, `sqrt(T_fuel)`.
//!
//! The same law is applied both to the stored reference-state coordinates and to
//! the query point at evaluation time, so the interpolation is carried out
//! entirely in transformed space (see [`super::CrossSectionData`]).

/// The transform applied to a feedback parameter before interpolation.
///
/// Parsed from the `xsVariables` law keyword; see the module documentation for
/// the physical meaning of each.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableLaw {
    /// `lin` — identity, `x -> x`.
    Linear,
    /// `log` — natural logarithm, `x -> ln(x)` (fast-spectrum Doppler).
    Log,
    /// `sqrt` — square root, `x -> sqrt(x)` (thermal-spectrum Doppler).
    Sqrt,
}

impl VariableLaw {
    /// Parse a GeN-Foam law keyword (`"lin"`, `"log"`, `"sqrt"`).
    ///
    /// Returns `None` for any other string; the caller turns that into an
    /// [`super::XsError::UnknownVariableLaw`].
    #[must_use]
    pub fn from_keyword(keyword: &str) -> Option<Self> {
        match keyword {
            "lin" => Some(Self::Linear),
            "log" => Some(Self::Log),
            "sqrt" => Some(Self::Sqrt),
            _ => None,
        }
    }

    /// The GeN-Foam keyword string for this law.
    #[must_use]
    pub fn keyword(self) -> &'static str {
        match self {
            Self::Linear => "lin",
            Self::Log => "log",
            Self::Sqrt => "sqrt",
        }
    }

    /// Apply the transform to a raw physical parameter value.
    ///
    /// For `Log` and `Sqrt` the caller is responsible for supplying a positive
    /// value; upstream GeN-Foam clamps fields to a small floor when building the
    /// mesh fields (`max(x, 0.01)` for `log`), which is a spatial-field concern
    /// left to the field-materialisation layer, not this scalar transform.
    #[must_use]
    pub fn transform(self, value: f64) -> f64 {
        match self {
            Self::Linear => value,
            Self::Log => value.ln(),
            Self::Sqrt => value.sqrt(),
        }
    }
}

/// One feedback parameter declared in the `xsVariables` dictionary.
///
/// The order of these within [`super::CrossSectionData`] defines the layout of
/// every parameter vector passed to the interpolators and to the public
/// evaluation methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XsVariable {
    /// Variable name as it appears in the `nuclearData` state dictionaries
    /// (e.g. `"Tfuel"`, `"rhoCool"`, `"axExp"`).
    pub name: String,
    /// Transform law applied to this variable's value before interpolation.
    pub law: VariableLaw,
}

impl XsVariable {
    /// Construct a feedback variable from its name and law.
    #[must_use]
    pub fn new(name: impl Into<String>, law: VariableLaw) -> Self {
        Self {
            name: name.into(),
            law,
        }
    }
}
