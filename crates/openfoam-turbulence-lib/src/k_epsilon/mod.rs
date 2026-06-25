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

/// Standard two-equation k-ε turbulence model (Jones & Launder 1972).
///
/// C++ source: `src/TurbulenceModels/turbulenceModels/RAS/kEpsilon/`
///
/// Transport equations:
///   ∂k/∂t + ∇·(Uk) − ∇·((ν + ν_t/σ_k)∇k) = G − ε
///   ∂ε/∂t + ∇·(Uε) − ∇·((ν + ν_t/σ_ε)∇ε) = C1ε·(ε/k)·G − C2ε·(ε²/k)
///   ν_t = Cμ · k² / ε
pub struct KEpsilon {
    pub mesh: Arc<FvMesh>,
    /// Turbulent kinetic energy [m²/s²]
    pub k: VolScalarField,
    /// Turbulent dissipation rate [m²/s³]
    pub epsilon: VolScalarField,
    /// Turbulent kinematic viscosity ν_t = Cμ k²/ε [m²/s]
    pub nu_t: VolScalarField,
    // Model coefficients
    c_mu:    f64,  // 0.09
    c1_eps:  f64,  // 1.44
    c2_eps:  f64,  // 1.92
    sigma_k: f64,  // 1.0
    sigma_e: f64,  // 1.3
}

impl KEpsilon {
    /// Standard Jones-Launder coefficients.
    pub fn new(mesh: Arc<FvMesh>) -> Self {
        let k       = VolScalarField::uniform("k",   mesh.clone(), 0.0);
        let epsilon = VolScalarField::uniform("epsilon", mesh.clone(), 1e-10);
        let nu_t    = VolScalarField::zeros("nut", mesh.clone());
        Self { mesh, k, epsilon, nu_t,
               c_mu: 0.09, c1_eps: 1.44, c2_eps: 1.92,
               sigma_k: 1.0, sigma_e: 1.3 }
    }
}

impl TurbulenceModel for KEpsilon {
    fn div_dev_rho_reff(&self, _u: &VolVectorField) -> FvVectorMatrix {
        todo!("KEpsilon::div_dev_rho_reff")
    }

    fn correct(&mut self) {
        todo!("KEpsilon::correct — solve k and epsilon transport equations")
    }

    fn nu_t(&self) -> &VolScalarField { &self.nu_t }

    fn alpha_eff(&self, alpha: &VolScalarField) -> VolScalarField {
        todo!("KEpsilon::alpha_eff — alpha + nu_t/Prt")
    }

    fn mu_eff_field(&self, mu: &VolScalarField) -> VolScalarField {
        todo!("KEpsilon::mu_eff_field — mu + rho*nu_t")
    }
}
