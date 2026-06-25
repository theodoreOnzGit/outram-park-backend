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

use openfoam_basic_lib::prelude::{FvVectorMatrix, VolScalarField, VolVectorField};

/// Common interface for all RAS and LES turbulence models.
///
/// Mirrors `Foam::compressible::turbulenceModel` and its incompressible
/// counterpart. Use static dispatch (generics) — not `dyn TurbulenceModel` —
/// to match C++ template zero-overhead composition.
pub trait TurbulenceModel {
    /// Assemble the turbulent deviatoric stress divergence term for the
    /// momentum equation:  ∇·(−2 μ_eff · dev(symm(∇U))).
    ///
    /// Returns an `FvVectorMatrix` whose coefficients are added to the
    /// momentum predictor before solving.
    fn div_dev_rho_reff(&self, u: &VolVectorField) -> FvVectorMatrix;

    /// Recompute turbulence transport fields (k, ε/ω, ν_t/μ_t) by solving
    /// the turbulence transport equations for one time step.
    ///
    /// Called once per time step **after** the momentum and pressure correctors.
    fn correct(&mut self);

    /// Turbulent kinematic viscosity field ν_t (incompressible) or μ_t/ρ
    /// (compressible).  Length == `mesh.n_cells`.
    fn nu_t(&self) -> &VolScalarField;

    /// Effective thermal diffusivity field: α_eff = α + α_t.
    ///
    /// `alpha` is the molecular thermal diffusivity (= κ / Cp) passed in from
    /// the thermophysical model.
    fn alpha_eff(&self, alpha: &VolScalarField) -> VolScalarField;

    /// Effective dynamic viscosity field: μ_eff = μ + μ_t.
    ///
    /// `mu` is the molecular dynamic viscosity field from the thermophysical model.
    fn mu_eff_field(&self, mu: &VolScalarField) -> VolScalarField;
}
