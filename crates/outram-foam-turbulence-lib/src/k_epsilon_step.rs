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

//! One transient step of a k-epsilon pair with ESI-form sources (shared by
//! [`crate::realizable_ke`] and [`crate::rng_k_epsilon`], gh:#1377).
//!
//! Upstream order (`RNGkEpsilon.C`, `realizableKE.C` v2506): the epsilon
//! equation is assembled and solved first, epsilon is bounded, then the k
//! equation is assembled with the UPDATED epsilon (`Sp(epsilon/k)`) and solved,
//! and k is bounded. Deviation from upstream, recorded in the callers: upstream's
//! `bound()` averages neighbours below the limit; here a plain floor at
//! `small` is used.

use outram_foam_basic_lib::fv_operators::{fvc, fvm};
use outram_foam_basic_lib::prelude::{
    Field, FvMesh, PatchField, SolverSettings, SurfaceScalarField, VolScalarField,
};
use std::sync::Arc;

/// Upstream double-precision `small` (`kMin_`, `epsilonMin_`).
pub(crate) const U_SMALL: f64 = 1e-15;

/// Build a `VolScalarField` from per-cell values with zero-gradient boundaries.
pub(crate) fn scalar_field(name: &str, mesh: Arc<FvMesh>, vals: Vec<f64>) -> VolScalarField {
    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient(p.size))
        .collect();
    VolScalarField::new(name, mesh, Field::new(vals), boundary)
}

/// Explicit/implicit coefficients of the epsilon equation (per cell, per volume).
pub(crate) struct EpsilonSources {
    /// Explicit source on the right-hand side.
    pub e: Vec<f64>,
    /// `SuSp` coefficient (`- fvm::SuSp(c, epsilon)`); empty means none.
    pub susp: Vec<f64>,
    /// `Sp` coefficient (`- fvm::Sp(c, epsilon)`).
    pub sp: Vec<f64>,
}

/// Everything one k-epsilon step needs besides the two fields.
pub(crate) struct StepInputs<'a> {
    pub mesh: &'a Arc<FvMesh>,
    pub phi: &'a SurfaceScalarField,
    pub dt: f64,
    pub eps_src: &'a EpsilonSources,
    /// Production `G` (explicit source of the k equation).
    pub g: &'a [f64],
    /// `fvc::div(phi)` per cell.
    pub div_u: &'a [f64],
    /// Effective epsilon diffusivity per cell.
    pub d_eps: Vec<f64>,
    /// Effective k diffusivity per cell.
    pub d_k: Vec<f64>,
}

/// Solve epsilon then k for one step of size `inp.dt`.
pub(crate) fn step(inp: StepInputs, k: &mut VolScalarField, epsilon: &mut VolScalarField) {
    let mesh = inp.mesh;
    let n = mesh.n_cells;
    let settings = SolverSettings::default();
    let deps_f = fvc::interpolate(&scalar_field("DepsilonEff", mesh.clone(), inp.d_eps));
    let eps_old = epsilon.clone();
    let mut eps_eqn = fvm::ddt(epsilon, &eps_old, inp.dt)
        + fvm::div(inp.phi, epsilon)
        + fvm::laplacian(&deps_f, epsilon);
    for c in 0..n {
        let v = mesh.cell_volumes[c];
        eps_eqn.source[c] += v * inp.eps_src.e[c];
        eps_eqn.ldu.diag[c] += v * inp.eps_src.sp[c];
        if !inp.eps_src.susp.is_empty() {
            let s = inp.eps_src.susp[c];
            eps_eqn.ldu.diag[c] += v * s.max(0.0);
            eps_eqn.source[c] -= v * s.min(0.0) * eps_old.internal[c];
        }
    }
    let (eps_new, _) = eps_eqn.solve("epsilon", settings);
    *epsilon = eps_new;
    for v in epsilon.internal.as_mut_slice() {
        if *v < U_SMALL {
            *v = U_SMALL;
        }
    }

    let dk_f = fvc::interpolate(&scalar_field("DkEff", mesh.clone(), inp.d_k));
    let k_old = k.clone();
    let mut k_eqn = fvm::ddt(k, &k_old, inp.dt) + fvm::div(inp.phi, k) + fvm::laplacian(&dk_f, k);
    for c in 0..n {
        let v = mesh.cell_volumes[c];
        k_eqn.source[c] += v * inp.g[c];
        k_eqn.ldu.diag[c] += v * (epsilon.internal[c] / k_old.internal[c]);
        let s = (2.0 / 3.0) * inp.div_u[c];
        k_eqn.ldu.diag[c] += v * s.max(0.0);
        k_eqn.source[c] -= v * s.min(0.0) * k_old.internal[c];
    }
    let (k_new, _) = k_eqn.solve("k", settings);
    *k = k_new;
    for v in k.internal.as_mut_slice() {
        if *v < U_SMALL {
            *v = U_SMALL;
        }
    }
}
