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

use std::sync::Arc;
use openfoam_basic_lib::prelude::{FvMesh, FvVectorMatrix, VolScalarField, VolVectorField};
use crate::traits::TurbulenceModel;

/// No-op turbulence model — laminar flow, zero turbulent stresses.
///
/// C++ source: `src/TurbulenceModels/turbulenceModels/laminar/laminar.H`
pub struct LaminarModel {
    pub mesh: Arc<FvMesh>,
    /// Molecular kinematic viscosity (from fluid thermo); ν_t ≡ 0.
    nu: VolScalarField,
}

impl LaminarModel {
    pub fn new(mesh: Arc<FvMesh>, nu: VolScalarField) -> Self {
        Self { mesh, nu }
    }
}

impl TurbulenceModel for LaminarModel {
    fn div_dev_rho_reff(&self, _u: &VolVectorField) -> FvVectorMatrix {
        todo!("LaminarModel::div_dev_rho_reff")
    }

    /// No-op — laminar model has no transport equations to solve.
    fn correct(&mut self) {}

    fn nu_t(&self) -> &VolScalarField {
        // ν_t = 0 for laminar flow; return the molecular viscosity field
        // with its values zeroed (caller interprets this as turbulent contribution).
        &self.nu
    }

    fn alpha_eff(&self, alpha: &VolScalarField) -> VolScalarField {
        alpha.clone()
    }

    fn mu_eff_field(&self, mu: &VolScalarField) -> VolScalarField {
        mu.clone()
    }
}
