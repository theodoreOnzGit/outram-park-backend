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

//! No-op laminar "turbulence" closure — zero turbulent viscosity everywhere.
//!
//! **Partial.** `correct` (a no-op), `nu_t`, `alpha_eff`, and `mu_eff_field`
//! work, but [`LaminarModel::div_dev_rho_reff`] — the momentum stress term — is
//! still a `todo!()` and will panic if called, so the laminar closure cannot yet
//! be driven end-to-end through the trait.

use crate::traits::TurbulenceModel;
use outram_foam_basic_lib::prelude::{FvMesh, FvVectorMatrix, VolScalarField, VolVectorField};
use std::sync::Arc;

/// No-op turbulence model — laminar flow, zero turbulent stresses.
///
/// Physically ν_t ≡ 0 (no turbulent viscosity), so the effective viscosity and
/// thermal diffusivity equal their molecular values.
///
/// **Partial implementation:** `div_dev_rho_reff` is a `todo!()` stub that
/// panics if called (see the module docs).
///
/// C++ source: `src/TurbulenceModels/turbulenceModels/laminar/laminar.H`
pub struct LaminarModel {
    pub mesh: Arc<FvMesh>,
    /// Molecular kinematic viscosity [m²/s] (from fluid thermo); ν_t ≡ 0.
    nu: VolScalarField,
}

impl LaminarModel {
    /// Construct a laminar closure over `mesh` with molecular kinematic
    /// viscosity field `nu` [m²/s].
    pub fn new(mesh: Arc<FvMesh>, nu: VolScalarField) -> Self {
        Self { mesh, nu }
    }
}

impl TurbulenceModel for LaminarModel {
    /// Not implemented — panics (`todo!()`). The laminar momentum stress term
    /// (`−∇·(ν·dev(∇U + ∇Uᵀ))`) has not been assembled yet.
    fn div_dev_rho_reff(&self, _u: &VolVectorField) -> FvVectorMatrix {
        todo!("LaminarModel::div_dev_rho_reff")
    }

    /// No-op — laminar model has no transport equations to solve.
    fn correct(&mut self) {}

    fn nu_t(&self) -> &VolScalarField {
        // Laminar flow has ν_t ≡ 0. NOTE: this returns the stored *molecular*
        // viscosity field (`nu`), not a zeroed field — a known limitation; the
        // model does not currently keep a separate zero ν_t field. Callers that
        // need a true zero turbulent viscosity must not treat this as ν_t.
        &self.nu
    }

    fn alpha_eff(&self, alpha: &VolScalarField) -> VolScalarField {
        alpha.clone()
    }

    fn mu_eff_field(&self, mu: &VolScalarField) -> VolScalarField {
        mu.clone()
    }
}
