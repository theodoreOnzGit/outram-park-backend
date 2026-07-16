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

//! Finite-volume operator layer: the explicit (`fvc`) and implicit (`fvm`)
//! operator namespaces, plus the global-continuity flux correction
//! (`adjust_phi`). Mirrors `src/finiteVolume/finiteVolume/{fvc,fvm}/` and
//! `src/finiteVolume/cfdTools/general/adjustPhi/` from OpenFOAM.

/// Global-continuity flux correction — see [`adjust_phi::adjust_phi`].
///
/// Usage mirrors `Foam::adjustPhi` from
/// `src/finiteVolume/cfdTools/general/adjustPhi/adjustPhi.C`.
pub(crate) mod adjust_phi;

/// Explicit finite-volume operators — return a new field.
///
/// Usage mirrors `Foam::fvc::` from `src/finiteVolume/finiteVolume/fvc/`.
pub(crate) mod fvc;

/// Implicit finite-volume operators — assemble into a sparse `FvMatrix`.
///
/// Usage mirrors `Foam::fvm::` from `src/finiteVolume/finiteVolume/fvm/`.
pub(crate) mod fvm;

// Flat re-exports of fvc::* and fvm::* are intentionally omitted: both
// modules export functions named `div` and `div_vec`, and a glob re-export
// of both in the same namespace is ambiguous. Callers use the qualified
// `fvc::` / `fvm::` prefixes, which are available via the module re-exports
// above.
pub(crate) use adjust_phi::*;
