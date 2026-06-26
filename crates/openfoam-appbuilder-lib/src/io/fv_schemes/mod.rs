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
    pub fn read(path: &Path) -> Result<Self, AppBuilderError> {
        let _ = path;
        todo!("FvSchemes::read — parse system/fvSchemes")
    }
}

impl Default for FvSchemes {
    fn default() -> Self {
        Self {
            ddt: DdtScheme::Euler,
            default_grad: GradScheme::GaussLinear,
            default_div: DivScheme::GaussLinear,
            default_laplacian: LaplacianScheme::GaussLinearCorrected,
            default_sn_grad: SnGradScheme::Corrected,
            default_interpolation: InterpolationScheme::Linear,
        }
    }
}
