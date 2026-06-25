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

/// Standard two-equation k-ω turbulence model (Wilcox 1988).
///
/// C++ source: `src/TurbulenceModels/turbulenceModels/RAS/kOmega/`
///
/// Transport equations:
///   ∂k/∂t + ∇·(Uk) − ∇·((ν + σ_k ν_t)∇k) = G − β* k ω
///   ∂ω/∂t + ∇·(Uω) − ∇·((ν + σ_ω ν_t)∇ω) = α (ω/k) G − β ω²
///   ν_t = k / ω
pub struct KOmega {
    pub mesh: Arc<FvMesh>,
    /// Turbulent kinetic energy [m²/s²]
    pub k: VolScalarField,
    /// Specific dissipation rate ω [1/s]
    pub omega: VolScalarField,
    /// Turbulent kinematic viscosity ν_t = k/ω [m²/s]
    pub nu_t: VolScalarField,
    // Model coefficients (Wilcox 1988)
    alpha:   f64,  // 5/9  ≈ 0.5556
    beta:    f64,  // 3/40 = 0.075
    beta_st: f64,  // 9/100 = 0.09  (= Cμ in k-ε)
    sigma_k: f64,  // 0.5
    sigma_w: f64,  // 0.5
}

impl KOmega {
    /// Wilcox 1988 coefficients.
    pub fn new(mesh: Arc<FvMesh>) -> Self {
        let k     = VolScalarField::uniform("k",     mesh.clone(), 0.0);
        let omega = VolScalarField::uniform("omega",  mesh.clone(), 1.0);
        let nu_t  = VolScalarField::zeros("nut",  mesh.clone());
        Self { mesh, k, omega, nu_t,
               alpha: 5.0/9.0, beta: 0.075, beta_st: 0.09,
               sigma_k: 0.5, sigma_w: 0.5 }
    }
}

impl TurbulenceModel for KOmega {
    fn div_dev_rho_reff(&self, _u: &VolVectorField) -> FvVectorMatrix {
        todo!("KOmega::div_dev_rho_reff")
    }

    fn correct(&mut self) {
        todo!("KOmega::correct — solve k and omega transport equations")
    }

    fn nu_t(&self) -> &VolScalarField { &self.nu_t }

    fn alpha_eff(&self, _alpha: &VolScalarField) -> VolScalarField {
        todo!("KOmega::alpha_eff")
    }

    fn mu_eff_field(&self, _mu: &VolScalarField) -> VolScalarField {
        todo!("KOmega::mu_eff_field")
    }
}
