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

//! Shared boundary-condition helpers for the PISO/PIMPLE solvers.
//!
//! The linear solver and field arithmetic rebuild output fields with
//! zero-gradient boundaries, so the prescribed BC *types* (e.g. a moving-wall
//! lid or a fixed inlet velocity) are lost after every `solve()`/arithmetic.
//! These helpers re-apply a captured BC template — the equivalent of OpenFOAM's
//! `field.correctBoundaryConditions()`.

use openfoam_basic_lib::prelude::{BoundaryCondition, VolScalarField, VolVectorField, Vector3};

/// Re-apply a scalar boundary-condition template to a field. For `FixedValue`
/// the boundary face values are reset to the fixed value; other BC types have
/// their face values recomputed by the operators (via `interpolate`).
pub(crate) fn correct_bcs(field: &mut VolScalarField, bcs: &[BoundaryCondition<f64>]) {
    for (pf, bc) in field.boundary.iter_mut().zip(bcs) {
        pf.bc = bc.clone();
        if let BoundaryCondition::FixedValue(v) = bc {
            for x in pf.values.iter_mut() { *x = *v; }
        }
    }
}

/// Vector counterpart of [`correct_bcs`].
pub(crate) fn correct_bcs_vec(field: &mut VolVectorField, bcs: &[BoundaryCondition<Vector3>]) {
    for (pf, bc) in field.boundary.iter_mut().zip(bcs) {
        pf.bc = bc.clone();
        if let BoundaryCondition::FixedValue(v) = bc {
            for x in pf.values.iter_mut() { *x = *v; }
        }
    }
}

/// Capture the boundary-condition template (the BC type per patch) of a field,
/// to be re-applied after solves with [`correct_bcs`] / [`correct_bcs_vec`].
pub(crate) fn capture_bcs<T: Clone>(boundary: &[openfoam_basic_lib::prelude::PatchField<T>])
    -> Vec<BoundaryCondition<T>>
{
    boundary.iter().map(|pf| pf.bc.clone()).collect()
}
