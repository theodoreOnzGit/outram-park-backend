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

//! Smagorinsky (1963) LES sub-grid-scale model — **scaffold only**.
//!
//! The [`Smagorinsky`] struct and its constant exist, but the sub-grid
//! viscosity update is not implemented: `correct`, `div_dev_rho_reff`,
//! `alpha_eff`, and `mu_eff_field` are `todo!()` stubs that panic if called.
//! Only `nu_t()` (which returns the zero-initialised ν_sgs field) is callable.

use crate::traits::TurbulenceModel;
use outram_foam_basic_lib::prelude::{FvMesh, FvVectorMatrix, VolScalarField, VolVectorField};
use std::sync::Arc;

/// Smagorinsky LES sub-grid scale model (1963).
///
/// **Scaffold only** — every trait method except `nu_t()` is a `todo!()` that
/// panics if called. The struct documents the intended model:
///
/// C++ source: `src/TurbulenceModels/LES/Smagorinsky/`
///
/// Sub-grid viscosity:  ν_sgs = (Cs·Δ)² · |S|
///   where Cs ≈ 0.17 is the Smagorinsky constant,
///         Δ  = (cell_volume)^(1/3) is the filter width (grid scale),
///         |S| = sqrt(2 · symm(∇U) : symm(∇U)) is the strain-rate magnitude.
pub struct Smagorinsky {
    pub mesh: Arc<FvMesh>,
    /// Sub-grid-scale kinematic viscosity ν_sgs [m²/s].
    pub nu_sgs: VolScalarField,
    /// Smagorinsky constant Cs (default 0.17).
    cs: f64,
}

impl Smagorinsky {
    /// Construct a Smagorinsky model over `mesh` with the default constant
    /// Cs = 0.17 and ν_sgs initialised to zero.
    pub fn new(mesh: Arc<FvMesh>) -> Self {
        let nu_sgs = VolScalarField::zeros("nuSgs", mesh.clone());
        Self {
            mesh,
            nu_sgs,
            cs: 0.17,
        }
    }

    /// Builder override for the Smagorinsky constant Cs (dimensionless).
    pub fn with_cs(mut self, cs: f64) -> Self {
        self.cs = cs;
        self
    }
}

impl TurbulenceModel for Smagorinsky {
    fn div_dev_rho_reff(&self, _u: &VolVectorField) -> FvVectorMatrix {
        todo!("Smagorinsky::div_dev_rho_reff")
    }

    fn correct(&mut self) {
        todo!("Smagorinsky::correct — compute |S| per cell, update nu_sgs = (Cs·Δ)²·|S|")
    }

    fn nu_t(&self) -> &VolScalarField {
        &self.nu_sgs
    }

    fn alpha_eff(&self, _alpha: &VolScalarField) -> VolScalarField {
        todo!("Smagorinsky::alpha_eff")
    }

    fn mu_eff_field(&self, _mu: &VolScalarField) -> VolScalarField {
        todo!("Smagorinsky::mu_eff_field")
    }
}
