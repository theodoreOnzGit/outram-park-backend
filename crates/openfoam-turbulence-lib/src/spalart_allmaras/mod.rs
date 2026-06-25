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

/// Spalart-Allmaras one-equation turbulence model (1992).
/// Common in aerospace applications (external aerodynamics, aerofoils).
///
/// C++ source: `src/TurbulenceModels/turbulenceModels/RAS/SpalartAllmaras/`
///
/// Single transport equation for the modified viscosity ν̃:
///   ∂ν̃/∂t + U·∇ν̃ = Cb1·S̃·ν̃ + (1/σ)∇·((ν+ν̃)∇ν̃) + Cb2/σ·|∇ν̃|² − Cw1·fw·(ν̃/d)²
///   ν_t = ν̃ · fv1    where fv1 = χ³/(χ³ + Cv1³),  χ = ν̃/ν
pub struct SpalartAllmaras {
    pub mesh: Arc<FvMesh>,
    /// Working variable ν̃ [m²/s] — NOT equal to ν_t directly.
    pub nu_tilde: VolScalarField,
    /// Turbulent kinematic viscosity ν_t = ν̃ · fv1 [m²/s].
    pub nu_t: VolScalarField,
}

// ── Spalart-Allmaras constants ────────────────────────────────────────────────
pub const CB1:  f64 = 0.1355;
pub const CB2:  f64 = 0.622;
pub const CV1:  f64 = 7.1;
pub const SIGMA: f64 = 2.0/3.0;
pub const KAPPA: f64 = 0.41;
pub const CW1:  f64 = CB1 / (KAPPA * KAPPA) + (1.0 + CB2) / SIGMA;  // ≈ 3.239
pub const CW2:  f64 = 0.3;
pub const CW3:  f64 = 2.0;

impl SpalartAllmaras {
    pub fn new(mesh: Arc<FvMesh>) -> Self {
        let nu_tilde = VolScalarField::zeros("nuTilda", mesh.clone());
        let nu_t     = VolScalarField::zeros("nut",     mesh.clone());
        Self { mesh, nu_tilde, nu_t }
    }
}

impl TurbulenceModel for SpalartAllmaras {
    fn div_dev_rho_reff(&self, _u: &VolVectorField) -> FvVectorMatrix {
        todo!("SpalartAllmaras::div_dev_rho_reff")
    }

    fn correct(&mut self) {
        todo!("SpalartAllmaras::correct — solve ν̃ transport equation, update ν_t = ν̃·fv1")
    }

    fn nu_t(&self) -> &VolScalarField { &self.nu_t }

    fn alpha_eff(&self, _alpha: &VolScalarField) -> VolScalarField {
        todo!("SpalartAllmaras::alpha_eff")
    }

    fn mu_eff_field(&self, _mu: &VolScalarField) -> VolScalarField {
        todo!("SpalartAllmaras::mu_eff_field")
    }
}
