// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/XS/ (fatal-error checks in
//                    include/readNuclearData.H and setNeutronicsConstants.H)
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

//! Error type for building and querying the cross-section data model.

use thiserror::Error;

/// Failure while constructing or evaluating a [`super::CrossSectionData`].
///
/// These correspond to the `FatalError` checks GeN-Foam performs while reading
/// a `nuclearData` dictionary, plus the numerical failure of the RBF solve.
/// Errors are always propagated — never papered over with a default value.
#[derive(Debug, Error)]
pub enum XsError {
    /// The `states` list was empty. At least the mandatory `reference` state is
    /// required.
    #[error("nuclearData has no states; a 'reference' state is mandatory")]
    NoStates,

    /// The first state was not named `reference`. GeN-Foam requires the
    /// reference state first so its values seed every interpolator.
    #[error("first nuclearData state must be 'reference', found '{found}'")]
    ReferenceStateNotFirst {
        /// The name actually found in first position.
        found: String,
    },

    /// A feedback variable declared in `xsVariables` had no value in the
    /// `reference` state (its reference value is mandatory).
    #[error("xsVariable '{name}' has no value in the reference state")]
    MissingReferenceVariable {
        /// The offending variable name.
        name: String,
    },

    /// An `xsVariables` law keyword was not one of `lin` / `log` / `sqrt`.
    #[error("unknown xsVariable law '{law}' for variable '{name}' (expected lin/log/sqrt)")]
    UnknownVariableLaw {
        /// The variable whose law failed to parse.
        name: String,
        /// The unrecognised law keyword.
        law: String,
    },

    /// A per-group or per-precursor data array had the wrong length.
    #[error("{field} for zone '{zone}' has length {found}, expected {expected}")]
    LengthMismatch {
        /// Which array (e.g. `"nuSigmaEff"`, `"Beta"`, `"scatteringMatrixP0"`).
        field: &'static str,
        /// The zone the array belongs to.
        zone: String,
        /// Length that was supplied.
        found: usize,
        /// Length that was required.
        expected: usize,
    },

    /// The reference state did not define the constant data (IV, Beta, lambda,
    /// discFactor, integralFlux, ...) for one of its zones.
    #[error("reference zone '{zone}' is missing its constant data block")]
    MissingZoneConstants {
        /// The zone lacking constants.
        zone: String,
    },

    /// A zone or energy-group index passed to an accessor was out of range.
    #[error("{what} index {index} out of range (have {count})")]
    IndexOutOfRange {
        /// What was indexed (`"zone"`, `"energy group"`, `"Legendre moment"`).
        what: &'static str,
        /// The offending index.
        index: usize,
        /// The number of available items.
        count: usize,
    },

    /// The polyharmonic-spline saddle-point solve failed (singular matrix,
    /// typically duplicate reference-state coordinates).
    #[error("polyharmonic-spline RBF matrix is singular: {detail}")]
    SingularRbfMatrix {
        /// Detail from the underlying dense solver.
        detail: String,
    },
}
