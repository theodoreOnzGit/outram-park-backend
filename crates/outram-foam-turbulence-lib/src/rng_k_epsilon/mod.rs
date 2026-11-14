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

//! RNG k-epsilon (Yakhot, Orszag, Thangam, Gatski & Speziale 1992), ported from
//! OpenFOAM-v2506 (ESI) `RNGkEpsilon.C` (gh:#1377, under #1376).
//!
//! `nut = Cmu k^2/epsilon`; epsilon source `(C1 - R) GbyNu Cmu k` with
//! `R = eta (1 - eta/eta0)/(1 + beta eta^3)`, `eta = sqrt(|GbyNu|) k/epsilon`, plus
//! `SuSp(((2/3) C1 - C3) div U)` and `Sp(C2 epsilon/k)`; k source `G` with
//! `SuSp((2/3) div U)` and `Sp(epsilon/k)`. Coefficients are upstream's defaults
//! (Cmu 0.0845, C1 1.42, C2 1.68, C3 0, sigmak = sigmaEps 0.71942, eta0 4.38,
//! beta 0.012).
//!
//! **Status: AI draft.** [`RngKEpsilon::source_terms`] is compared bit for bit
//! against the real ESI model on a frozen duct
//! (`verification_and_validation/upstream_turb_gh1362/README.md`, slice 3).
//! `correct()` is NOT compared against upstream: no wall functions, zero-gradient
//! walls, and `bound()` is a plain floor at `small` (upstream averages neighbours).

use crate::k_epsilon_step::{self, scalar_field, EpsilonSources, StepInputs};
use crate::source_terms::{self, FrozenState, SourceTerms};
use crate::traits::TurbulenceModel;
use outram_foam_basic_lib::fv_operators::fvm;
use outram_foam_basic_lib::prelude::{
    Field, FvMesh, FvVectorMatrix, SurfaceScalarField, VolScalarField, VolVectorField,
};
use std::sync::Arc;

/// RNG k-epsilon model (incompressible, single phase).
pub struct RngKEpsilon {
    pub mesh: Arc<FvMesh>,
    /// Turbulent kinetic energy k [m^2/s^2].
    pub k: VolScalarField,
    /// Dissipation rate epsilon [m^2/s^3].
    pub epsilon: VolScalarField,
    /// Turbulent kinematic viscosity [m^2/s].
    pub nu_t: VolScalarField,
    /// Velocity [m/s]; set by the solver before `correct()`.
    pub u: VolVectorField,
    /// Volumetric face flux [m^3/s]; set by the solver before `correct()`.
    pub phi: SurfaceScalarField,
    /// Molecular kinematic viscosity [m^2/s].
    pub nu: VolScalarField,
    /// Time step [s].
    pub dt: f64,
    /// Turbulent Prandtl number for `alpha_eff`.
    pub prt: f64,
    cmu: f64,
    c1: f64,
    c2: f64,
    c3: f64,
    sigma_k: f64,
    sigma_eps: f64,
    eta0: f64,
    beta: f64,
}

impl RngKEpsilon {
    /// Upstream defaults; fields start uniform (k = 1e-4, epsilon = 1e-4).
    pub fn new(mesh: Arc<FvMesh>) -> Self {
        let mut m = Self {
            k: VolScalarField::uniform("k", mesh.clone(), 1e-4),
            epsilon: VolScalarField::uniform("epsilon", mesh.clone(), 1e-4),
            nu_t: VolScalarField::zeros("nut", mesh.clone()),
            u: VolVectorField::zero("U", mesh.clone()),
            phi: SurfaceScalarField::zeros("phi", mesh.clone()),
            nu: VolScalarField::uniform("nu", mesh.clone(), 1e-5),
            mesh,
            dt: 1e-3,
            prt: 0.85,
            cmu: 0.0845,
            c1: 1.42,
            c2: 1.68,
            c3: 0.0,
            sigma_k: 0.71942,
            sigma_eps: 0.71942,
            eta0: 4.38,
            beta: 0.012,
        };
        m.correct_nut();
        m
    }

    /// `nut = Cmu k^2/epsilon` (upstream `correctNut`, no bound).
    pub fn correct_nut(&mut self) {
        let k = self.k.internal.as_slice();
        let e = self.epsilon.internal.as_slice();
        let v: Vec<f64> = (0..self.mesh.n_cells)
            .map(|c| self.cmu * (k[c] * k[c]) / e[c])
            .collect();
        self.nu_t.internal = Field::new(v);
    }

    /// Per-cell terms of the epsilon and k equations on the CURRENT fields
    /// (`state` carries `grad_u` and `div_u`), upstream v2506 arithmetic:
    /// `nut`, `GbyNu`, `G`, `eta`, `R`, `eps_E`, `eps_SuSp`, `eps_Sp`, `k_E`,
    /// `k_SuSp`, `k_Sp`. `G` uses the model's stored `nu_t` (upstream
    /// `G = nut*GbyNu`); the `nut` term is `Cmu k^2/epsilon` recomputed.
    pub fn source_terms(&self, state: &FrozenState) -> SourceTerms {
        let n = self.mesh.n_cells;
        let k = self.k.internal.as_slice();
        let eps = self.epsilon.internal.as_slice();
        let nut_stored = self.nu_t.internal.as_slice();
        let mut cols: Vec<Vec<f64>> = (0..11).map(|_| vec![0.0; n]).collect();
        for c in 0..n {
            let gbn = source_terms::g_by_nu(&state.grad_u[c]);
            let g = nut_stored[c] * gbn;
            let eta = gbn.abs().sqrt() * k[c] / eps[c];
            let eta3 = eta * (eta * eta);
            let r = (eta * (-eta / self.eta0 + 1.0)) / (self.beta * eta3 + 1.0);
            cols[0][c] = self.cmu * (k[c] * k[c]) / eps[c];
            cols[1][c] = gbn;
            cols[2][c] = g;
            cols[3][c] = eta;
            cols[4][c] = r;
            cols[5][c] = (self.c1 - r) * gbn * self.cmu * k[c];
            cols[6][c] = ((2.0 / 3.0) * self.c1 - self.c3) * state.div_u[c];
            cols[7][c] = self.c2 * eps[c] / k[c];
            cols[8][c] = g;
            cols[9][c] = (2.0 / 3.0) * state.div_u[c];
            cols[10][c] = eps[c] / k[c];
        }
        let names = [
            "nut", "GbyNu", "G", "eta", "R", "eps_E", "eps_SuSp", "eps_Sp", "k_E", "k_SuSp",
            "k_Sp",
        ];
        let mut out = SourceTerms::default();
        for (name, col) in names.iter().zip(cols) {
            out.push(name, col);
        }
        out
    }
}

impl TurbulenceModel for RngKEpsilon {
    fn div_dev_rho_reff(&self, u: &VolVectorField) -> FvVectorMatrix {
        let n = self.mesh.n_cells;
        let nu = self.nu.internal.as_slice();
        let nut = self.nu_t.internal.as_slice();
        let nu_eff = scalar_field(
            "nuEff",
            self.mesh.clone(),
            (0..n).map(|c| nu[c] + nut[c]).collect(),
        );
        fvm::laplacian_vec(&nu_eff, u, self.mesh.clone())
    }

    fn correct(&mut self) {
        let mesh = self.mesh.clone();
        let n = mesh.n_cells;
        let state = FrozenState {
            grad_u: source_terms::gauss_grad_u(&mesh, &self.u),
            div_u: source_terms::div_phi(&mesh, &self.phi),
            ..FrozenState::default()
        };
        let t = self.source_terms(&state);
        let nut = self.nu_t.internal.as_slice().to_vec();
        let nu = self.nu.internal.as_slice().to_vec();
        let src = EpsilonSources {
            e: t.get("eps_E").unwrap().to_vec(),
            susp: t.get("eps_SuSp").unwrap().to_vec(),
            sp: t.get("eps_Sp").unwrap().to_vec(),
        };
        let inp = StepInputs {
            mesh: &mesh,
            phi: &self.phi,
            dt: self.dt,
            eps_src: &src,
            g: t.get("G").unwrap(),
            div_u: &state.div_u,
            d_eps: (0..n).map(|c| nut[c] / self.sigma_eps + nu[c]).collect(),
            d_k: (0..n).map(|c| nut[c] / self.sigma_k + nu[c]).collect(),
        };
        k_epsilon_step::step(inp, &mut self.k, &mut self.epsilon);
        self.correct_nut();
    }

    fn nu_t(&self) -> &VolScalarField {
        &self.nu_t
    }

    fn alpha_eff(&self, alpha: &VolScalarField) -> VolScalarField {
        let a = alpha.internal.as_slice();
        let nut = self.nu_t.internal.as_slice();
        let v = (0..self.mesh.n_cells)
            .map(|c| a[c] + nut[c] / self.prt)
            .collect();
        scalar_field("alphaEff", self.mesh.clone(), v)
    }

    fn mu_eff_field(&self, mu: &VolScalarField) -> VolScalarField {
        let m = mu.internal.as_slice();
        let nut = self.nu_t.internal.as_slice();
        let v = (0..self.mesh.n_cells).map(|c| m[c] + nut[c]).collect();
        scalar_field("muEff", self.mesh.clone(), v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::prelude::{BoundaryPatch, FvMeshBuilder, PatchKind, Tensor, Vector3};

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
                .cell_volumes(vec![1.0, 1.0, 1.0])
                .cell_centres(vec![vx(0.5), vx(1.5), vx(2.5)])
                .face_area_vectors(vec![vx(1.0), vx(1.0), vx(-1.0), vx(1.0)])
                .face_centres(vec![vx(1.0), vx(2.0), vx(0.0), vx(3.0)])
                .build()
                .unwrap(),
        )
    }

    /// nut = Cmu k^2/epsilon with the RNG Cmu = 0.0845 (RNGkEpsilon.C correctNut):
    /// k = 0.2, epsilon = 10 gives 0.0845 * 0.04 / 10 = 3.38e-4.
    #[test]
    fn nut_uses_rng_cmu() {
        let mut m = RngKEpsilon::new(line_mesh());
        m.k.internal = Field::new(vec![0.2; 3]);
        m.epsilon.internal = Field::new(vec![10.0; 3]);
        m.correct_nut();
        for c in 0..3 {
            assert!((m.nu_t.internal[c] - 0.0845 * 0.04 / 10.0).abs() < 1e-18);
        }
    }

    /// Zero strain: GbyNu = eta = R = 0, so the epsilon source and SuSp vanish.
    #[test]
    fn zero_strain_gives_zero_sources() {
        let m = RngKEpsilon::new(line_mesh());
        let st = FrozenState {
            grad_u: vec![Tensor::ZERO; 3],
            div_u: vec![0.0; 3],
            ..FrozenState::default()
        };
        let t = m.source_terms(&st);
        for name in ["GbyNu", "G", "eta", "R", "eps_E", "eps_SuSp", "k_E", "k_SuSp"] {
            assert!(t.get(name).unwrap().iter().all(|v| *v == 0.0), "{name}");
        }
        assert!(t.get("eps_Sp").unwrap().iter().all(|v| (*v - 1.68).abs() < 1e-12));
    }

    /// correct() on a shear field keeps k, epsilon positive and nut finite, and
    /// the trait accessors return per-cell fields.
    #[test]
    fn correct_keeps_fields_positive_and_finite() {
        let m0 = line_mesh();
        let mut m = RngKEpsilon::new(m0.clone());
        m.u.internal = Field::new(vec![vx(1.0), vx(2.0), vx(3.0)]);
        m.nu = VolScalarField::uniform("nu", m0, 1e-3);
        for _ in 0..5 {
            m.correct();
            for c in 0..3 {
                assert!(m.k.internal[c] > 0.0 && m.epsilon.internal[c] > 0.0);
                assert!(m.nu_t.internal[c].is_finite() && m.nu_t.internal[c] >= 0.0);
            }
        }
        let nu = m.nu.clone();
        assert_eq!(m.mu_eff_field(&nu).internal.len(), 3);
        assert_eq!(m.alpha_eff(&nu).internal.len(), 3);
        let u = m.u.clone();
        let _ = m.div_dev_rho_reff(&u);
        assert_eq!(m.nu_t().internal.len(), 3);
    }
}
