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

use std::collections::HashMap;
use std::path::Path;
use crate::error::AppBuilderError;

/// Parsed `system/fvSolution`.
#[derive(Debug, Clone)]
pub struct FvSolution {
    /// Per-field linear solver configuration, keyed by field name.
    pub solvers: HashMap<String, LinearSolverConfig>,
    /// PIMPLE / PISO outer-loop control parameters.
    pub pimple: PimpleControl,
    /// Under-relaxation factors, keyed by field name.
    pub relaxation_fields:    HashMap<String, f64>,
    pub relaxation_equations: HashMap<String, f64>,
}

/// Linear solver configuration for a single field (fvSolution::solvers.<field>).
#[derive(Debug, Clone)]
pub struct LinearSolverConfig {
    pub solver:         LinearSolverType,
    pub preconditioner: Option<String>,
    pub tolerance:      f64,
    pub rel_tol:        f64,
    pub max_iter:       usize,
    pub smoother:       Option<String>,
    pub n_sweep:        usize,
}

/// Linear solver algorithm.
#[derive(Debug, Clone, PartialEq)]
pub enum LinearSolverType {
    /// Preconditioned Conjugate Gradient (symmetric systems, e.g. pressure).
    Pcg,
    /// Preconditioned Bi-Conjugate Gradient Stabilised (asymmetric, e.g. U, T).
    PbicgStab,
    /// Generalised Algebraic Multi-Grid (large pressure systems).
    Gamg,
    /// Gauss-Seidel (smoother or stand-alone for simple problems).
    GaussSeidel,
    /// Diagonal preconditioner only.
    Diagonal,
    /// Smooth solver (iterative, for symmetric).
    SmoothSolver,
}

/// PIMPLE / PISO outer-corrector loop control.
#[derive(Debug, Clone)]
pub struct PimpleControl {
    /// Number of outer PIMPLE correctors (1 = PISO).
    pub n_outer_correctors: usize,
    /// Number of inner pressure correctors per outer corrector.
    pub n_correctors: usize,
    /// Non-orthogonal correctors for mesh skewness compensation.
    pub n_non_orthogonal_correctors: usize,
    /// Use consistent formulation (avoids rAU cell-size dependency).
    pub consistent: bool,
    /// Turbulence corrector at end of each outer loop.
    pub correct_phi: bool,
}

impl FvSolution {
    pub fn read(path: &Path) -> Result<Self, AppBuilderError> {
        let _ = path;
        todo!("FvSolution::read — parse system/fvSolution")
    }
}

impl Default for LinearSolverConfig {
    fn default() -> Self {
        Self {
            solver: LinearSolverType::PbicgStab,
            preconditioner: Some("DILU".into()),
            tolerance: 1e-6,
            rel_tol: 0.0,
            max_iter: 1000,
            smoother: None,
            n_sweep: 1,
        }
    }
}

impl Default for PimpleControl {
    fn default() -> Self {
        Self {
            n_outer_correctors: 1,
            n_correctors: 2,
            n_non_orthogonal_correctors: 0,
            consistent: false,
            correct_phi: false,
        }
    }
}

impl Default for FvSolution {
    fn default() -> Self {
        let mut solvers = HashMap::new();
        solvers.insert("p".into(), LinearSolverConfig {
            solver: LinearSolverType::Gamg,
            preconditioner: Some("GaussSeidel".into()),
            tolerance: 1e-6,
            rel_tol: 0.01,
            ..Default::default()
        });
        solvers.insert("U".into(), LinearSolverConfig::default());
        Self {
            solvers,
            pimple: PimpleControl::default(),
            relaxation_fields: HashMap::new(),
            relaxation_equations: HashMap::new(),
        }
    }
}
