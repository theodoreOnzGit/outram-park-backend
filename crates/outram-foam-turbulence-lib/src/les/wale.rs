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

//! WALE sub-grid-scale model (Nicoud & Ducros 1999), ported from
//! OpenFOAM-v2506 (ESI) `WALE.C` (gh:#1377, under #1376).
//!
//! Algebraic: `Sd = devSymm(gradU & gradU)`,
//! `k = sqr(Cw^2 delta/Ck) |Sd|^6 / (sqr(|S|^5 + |Sd|^{5/2}) + SMALL)`
//! (written with `pow(magSqr(symm(gradU)), 5/2)` and `pow(magSqrSd, 5/4)`),
//! `nut = Ck delta sqrt(k)`. Defaults `Ck = 0.094`, `Cw = 0.325`; `delta` is
//! the `cubeRootVol` width `1.0 * cbrt(V)`.
//!
//! **Status: AI draft.** [`Wale::source_terms`] is compared per cell against the
//! real ESI model on a frozen duct (`upstream_turb_gh1362/README.md`, slice 3);
//! `pow` and `cbrt` come from PETIR, so ulp differences against glibc are
//! counted and pinned there. No wall damping/van Driest delta, and `correct()`
//! is not compared against upstream.

use crate::k_epsilon_step::scalar_field;
use crate::source_terms::{self, FrozenState, SourceTerms};
use crate::traits::TurbulenceModel;
use outram_foam_basic_lib::fv_operators::fvm;
use outram_foam_basic_lib::prelude::{
    Field, FvMesh, FvVectorMatrix, Tensor, VolScalarField, VolVectorField,
};
use petir::mathf::RealMath;
use std::sync::Arc;

/// upstream double `SMALL`.
const U_SMALL: f64 = 1e-15;

/// Per-cell intermediates of upstream `WALE::k`.
#[derive(Debug, Clone, Copy)]
pub struct WaleParts {
    /// `magSqr(Sd)` [1/s^4].
    pub mag_sqr_sd: f64,
    /// `magSqr(symm(gradU))` [1/s^2].
    pub mag_sqr_s: f64,
    /// `pow(magSqr(symm(gradU)), 5/2)`.
    pub pow52: f64,
    /// `pow(magSqrSd, 5/4)`.
    pub pow54: f64,
    /// Sub-grid kinetic energy [m^2/s^2].
    pub k: f64,
    /// Eddy viscosity [m^2/s].
    pub nut: f64,
}

/// Upstream `WALE::k` and `correctNut` for one cell (translated line by line).
pub fn wale_cell(grad: &Tensor, delta: f64, ck: f64, cw: f64) -> WaleParts {
    let gg = source_terms::tensor_inner(grad, grad);
    let sd = source_terms::dev_symm(&gg);
    let mag_sqr_sd = source_terms::mag_sqr_symm6(&sd);
    let mag_sqr_s = source_terms::mag_sqr_symm6(&source_terms::symm6(grad));
    let pow52 = mag_sqr_s.r_powf(5.0 / 2.0);
    let pow54 = mag_sqr_sd.r_powf(5.0 / 4.0);
    let lead = (cw * cw * delta) / ck;
    let lead2 = lead * lead;
    let sum = pow52 + pow54;
    let pow3 = mag_sqr_sd * (mag_sqr_sd * mag_sqr_sd);
    let k = lead2 * (pow3 / ((sum * sum) + U_SMALL));
    let nut = ck * delta * k.sqrt();
    WaleParts {
        mag_sqr_sd,
        mag_sqr_s,
        pow52,
        pow54,
        k,
        nut,
    }
}

/// WALE LES model (incompressible, single phase).
pub struct Wale {
    pub mesh: Arc<FvMesh>,
    /// Sub-grid eddy viscosity [m^2/s].
    pub nu_sgs: VolScalarField,
    /// Sub-grid kinetic energy [m^2/s^2].
    pub k_sgs: VolScalarField,
    /// Velocity [m/s]; set by the solver before `correct()`.
    pub u: VolVectorField,
    /// Molecular kinematic viscosity [m^2/s].
    pub nu: VolScalarField,
    /// Turbulent Prandtl number for `alpha_eff`.
    pub prt: f64,
    ck: f64,
    cw: f64,
    delta: Vec<f64>,
}

impl Wale {
    /// Upstream defaults (`Ck = 0.094`, `Cw = 0.325`), `cubeRootVol` delta.
    pub fn new(mesh: Arc<FvMesh>) -> Self {
        let delta = (0..mesh.n_cells)
            .map(|c| 1.0 * mesh.cell_volumes[c].r_cbrt())
            .collect();
        Self {
            nu_sgs: VolScalarField::zeros("nuSgs", mesh.clone()),
            k_sgs: VolScalarField::zeros("k", mesh.clone()),
            u: VolVectorField::zero("U", mesh.clone()),
            nu: VolScalarField::uniform("nu", mesh.clone(), 1e-5),
            mesh,
            prt: 0.85,
            ck: 0.094,
            cw: 0.325,
            delta,
        }
    }

    /// The per-cell `cubeRootVol` filter width [m].
    pub fn delta(&self) -> &[f64] {
        &self.delta
    }

    /// Per-cell `delta`, `magSqrSd`, `magSqrS`, `pow52`, `pow54`, `k`, `nut` on
    /// `state.grad_u`, without touching the model state. `state.cell_length`
    /// overrides the filter width (to isolate `cbrt` from the model formula).
    pub fn source_terms(&self, state: &FrozenState) -> SourceTerms {
        let n = self.mesh.n_cells;
        let names = ["delta", "magSqrSd", "magSqrS", "pow52", "pow54", "k", "nut"];
        let mut cols: Vec<Vec<f64>> = names.iter().map(|_| vec![0.0; n]).collect();
        for c in 0..n {
            let delta = match &state.cell_length {
                Some(v) => v[c],
                None => self.delta[c],
            };
            let p = wale_cell(&state.grad_u[c], delta, self.ck, self.cw);
            cols[0][c] = delta;
            cols[1][c] = p.mag_sqr_sd;
            cols[2][c] = p.mag_sqr_s;
            cols[3][c] = p.pow52;
            cols[4][c] = p.pow54;
            cols[5][c] = p.k;
            cols[6][c] = p.nut;
        }
        let mut out = SourceTerms::default();
        for (name, col) in names.iter().zip(cols) {
            out.push(name, col);
        }
        out
    }
}

impl TurbulenceModel for Wale {
    fn div_dev_rho_reff(&self, u: &VolVectorField) -> FvVectorMatrix {
        let n = self.mesh.n_cells;
        let nu = self.nu.internal.as_slice();
        let nut = self.nu_sgs.internal.as_slice();
        let nu_eff = scalar_field(
            "nuEff",
            self.mesh.clone(),
            (0..n).map(|c| nu[c] + nut[c]).collect(),
        );
        fvm::laplacian_vec(&nu_eff, u, self.mesh.clone())
    }

    fn correct(&mut self) {
        let state = FrozenState {
            grad_u: source_terms::gauss_grad_u(&self.mesh, &self.u),
            ..FrozenState::default()
        };
        let t = self.source_terms(&state);
        self.k_sgs.internal = Field::new(t.get("k").unwrap().to_vec());
        self.nu_sgs.internal = Field::new(t.get("nut").unwrap().to_vec());
    }

    fn nu_t(&self) -> &VolScalarField {
        &self.nu_sgs
    }

    fn alpha_eff(&self, alpha: &VolScalarField) -> VolScalarField {
        let a = alpha.internal.as_slice();
        let nut = self.nu_sgs.internal.as_slice();
        let v = (0..self.mesh.n_cells)
            .map(|c| a[c] + nut[c] / self.prt)
            .collect();
        scalar_field("alphaEff", self.mesh.clone(), v)
    }

    fn mu_eff_field(&self, mu: &VolScalarField) -> VolScalarField {
        let m = mu.internal.as_slice();
        let nut = self.nu_sgs.internal.as_slice();
        let v = (0..self.mesh.n_cells).map(|c| m[c] + nut[c]).collect();
        scalar_field("muEff", self.mesh.clone(), v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::prelude::{BoundaryPatch, FvMeshBuilder, PatchKind, Vector3};

    fn vx(x: f64) -> Vector3 {
        Vector3::new(x, 0.0, 0.0)
    }

    fn line_mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(3)
                .n_internal_faces(2)
                .owner(vec![0, 1, 0, 2])
                .neighbour(vec![1, 2])
                .patches(vec![
                    BoundaryPatch::new("left", 2, 1, PatchKind::Patch),
                    BoundaryPatch::new("right", 3, 1, PatchKind::Patch),
                ])
                .cell_volumes(vec![8.0, 8.0, 8.0])
                .cell_centres(vec![vx(0.5), vx(1.5), vx(2.5)])
                .face_area_vectors(vec![vx(1.0), vx(1.0), vx(-1.0), vx(1.0)])
                .face_centres(vec![vx(1.0), vx(2.0), vx(0.0), vx(3.0)])
                .build()
                .unwrap(),
        )
    }

    /// The WALE viscosity vanishes in pure rotation-free uniform flow and in
    /// solid-body rotation (Sd = 0 for a purely skew gradient): the defining
    /// near-wall property of the model. Pure skew gradient: Sd = devSymm(W.W)
    /// is nonzero for rotation, so test the true zero case: no gradient.
    #[test]
    fn nut_is_zero_without_velocity_gradient() {
        let p = wale_cell(&Tensor::ZERO, 1.0, 0.094, 0.325);
        assert_eq!(p.nut, 0.0);
        assert_eq!(p.k, 0.0);
    }

    /// Pure shear dU/dy = s: gradU has a single off-diagonal entry, so gradU & gradU
    /// = 0 and Sd = 0, hence WALE gives nut = 0 (the model's laminar-shear property).
    #[test]
    fn pure_shear_gives_zero_nut() {
        let g = Tensor::new(0.0, 3.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let p = wale_cell(&g, 1.0, 0.094, 0.325);
        assert_eq!(p.mag_sqr_sd, 0.0);
        assert_eq!(p.nut, 0.0);
    }

    /// delta = cbrt(V): 8 m^3 cells give 2 m (to cbrt rounding); correct() on a
    /// strained field gives non-negative finite nut; trait accessors work.
    #[test]
    fn correct_gives_finite_nonnegative_nut() {
        let mut m = Wale::new(line_mesh());
        assert!(m.delta().iter().all(|d| (d - 2.0).abs() < 1e-14));
        m.u.internal = Field::new(vec![vx(1.0), vx(4.0), vx(9.0)]);
        m.correct();
        for c in 0..3 {
            assert!(m.nu_sgs.internal[c].is_finite() && m.nu_sgs.internal[c] >= 0.0);
            assert!(m.k_sgs.internal[c].is_finite());
        }
        let nu = m.nu.clone();
        assert_eq!(m.mu_eff_field(&nu).internal.len(), 3);
        assert_eq!(m.alpha_eff(&nu).internal.len(), 3);
        let u = m.u.clone();
        let _ = m.div_dev_rho_reff(&u);
        assert_eq!(m.nu_t().internal.len(), 3);
    }
}
