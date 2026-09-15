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

//! Parser for OpenFOAM's `system/fvSchemes` — the per-operator numerical scheme
//! selection dictionary. Each scheme family (ddt, grad, div, laplacian, snGrad,
//! interpolation) is a typed enum on [`FvSchemes`], so rust-analyzer surfaces
//! every valid option on hover and an unknown scheme is a `Result` error.

use crate::error::AppBuilderError;
use std::path::Path;

/// Parsed `system/fvSchemes` — numerical scheme selection for each operator.
#[derive(Debug, Clone)]
pub struct FvSchemes {
    pub ddt: DdtScheme,
    pub default_grad: GradScheme,
    pub default_div: DivScheme,
    pub default_laplacian: LaplacianScheme,
    pub default_sn_grad: SnGradScheme,
    pub default_interpolation: InterpolationScheme,
}

/// Time-stepping scheme (ddtSchemes).
#[derive(Debug, Clone, PartialEq)]
pub enum DdtScheme {
    Euler,
    Backward,
    CrankNicolson(f64), // off-centring coefficient ψ ∈ [0,1]
    LocalEuler,
    SteadyState,
}

/// Gradient scheme (gradSchemes).
#[derive(Debug, Clone, PartialEq)]
pub enum GradScheme {
    GaussLinear,
    LeastSquares,
    FourthOrder,
}

/// Divergence / convection scheme (divSchemes).
#[derive(Debug, Clone, PartialEq)]
pub enum DivScheme {
    GaussLinear,
    GaussUpwind,
    GaussLinearUpwind(String), // e.g. "Gauss linearUpwind grad(U)"
    GaussVanLeer,
    GaussMUSCL,
    GaussLimitedLinear(f64),
}

/// Laplacian scheme (laplacianSchemes).
#[derive(Debug, Clone, PartialEq)]
pub enum LaplacianScheme {
    GaussLinearCorrected,
    GaussLinearUncorrected,
    GaussLinearLimited(f64), // limiter coefficient ∈ [0,1]
}

/// Surface-normal gradient scheme (snGradSchemes).
#[derive(Debug, Clone, PartialEq)]
pub enum SnGradScheme {
    Corrected,
    Uncorrected,
    Limited(f64),
}

/// Face interpolation scheme (interpolationSchemes).
#[derive(Debug, Clone, PartialEq)]
pub enum InterpolationScheme {
    Linear,
    Upwind(String), // e.g. "upwind phi"
    Harmonic,
}

impl FvSchemes {
    /// Parse a `system/fvSchemes` file from disk.
    ///
    /// **Not yet implemented — calling this panics (`todo!`).** No OpenFOAM
    /// dictionary parsing exists in this crate; see the sibling
    /// [`crate::io::control_dict::ControlDict::read`] and
    /// [`crate::io::fv_solution::FvSolution::read`], which are in the same
    /// state.
    ///
    /// Build the struct in Rust instead — [`FvSchemes::default`] documents what
    /// the solvers in this crate actually do, and
    /// [`crate::solvers::schemes`] documents which selections are honoured and
    /// which return [`AppBuilderError::UnsupportedScheme`].
    pub fn read(path: &Path) -> Result<Self, AppBuilderError> {
        let _ = path;
        todo!("FvSchemes::read — parse system/fvSchemes")
    }
}

impl Default for FvSchemes {
    /// The defaults describe **what the solvers in this crate actually do** when
    /// no `system/fvSchemes` is supplied, not what a typical OpenFOAM case
    /// writes.
    ///
    /// Notably `default_div` is [`DivScheme::GaussUpwind`], not `Gauss linear`.
    /// Convection in [`crate::solvers::pimple_foam::PimpleFoam`] is assembled by
    /// [`crate::solvers::schemes::div_vec_scheme`], whose bounded first-order
    /// upwind arm is the safe default on the coarse meshes the tutorials use;
    /// second-order central differencing is opted into by setting
    /// `schemes.default_div = DivScheme::GaussLinear`. This field previously
    /// said `GaussLinear` while every solver hardwired upwind and ignored the
    /// struct entirely — the value was a claim nothing honoured.
    ///
    /// `default_grad`, `default_laplacian`, `default_sn_grad` and
    /// `default_interpolation` are **still not consulted by any solver**; they
    /// are parsed and stored only. Do not read them as a statement about the
    /// discretisation in use.
    fn default() -> Self {
        Self {
            ddt: DdtScheme::Euler,
            default_grad: GradScheme::GaussLinear,
            default_div: DivScheme::GaussUpwind,
            default_laplacian: LaplacianScheme::GaussLinearCorrected,
            default_sn_grad: SnGradScheme::Corrected,
            default_interpolation: InterpolationScheme::Linear,
        }
    }
}
