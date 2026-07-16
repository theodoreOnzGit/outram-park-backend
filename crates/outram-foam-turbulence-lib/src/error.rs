// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

//! Error type for turbulence-model construction and transport solves.
//!
//! A single [`TurbulenceError`] enum covers the failure modes the closures can
//! report (field-shape mismatches, use-before-init, and non-physical negative
//! turbulence quantities).

use thiserror::Error;

/// Failure modes reported by the turbulence closures.
#[derive(Debug, Error)]
pub enum TurbulenceError {
    /// A field passed in does not match the mesh cell count (or another field's
    /// length). The payload describes the mismatch.
    #[error("field size mismatch: {0}")]
    FieldSizeMismatch(String),
    /// A model method was called before the model's state was initialised.
    #[error("turbulence model not initialised")]
    NotInitialised,
    /// A turbulence quantity (e.g. k, ε, ω, ν_t) went negative, which is
    /// non-physical. `field` names the quantity; `value` is the offending value.
    #[error("negative turbulent quantity: {field} = {value}")]
    NegativeField {
        /// Name of the turbulence field that went negative (e.g. `"k"`).
        field: &'static str,
        /// The offending (negative) value.
        value: f64,
    },
}
