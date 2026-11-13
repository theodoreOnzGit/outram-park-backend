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

//! Realizable k-epsilon (Shih, Liou, Shabbir, Yang & Zhu 1995), ported from
//! OpenFOAM-v2506 (ESI) `realizableKE.C` (gh:#1377, under #1376).
//!
//! `nut = rCmu k^2/epsilon` with the strain-dependent
//! `rCmu = 1/(A0 + As Us k/epsilon)`, `As = sqrt(6) cos(phis)`,
//! `phis = acos(clamp(sqrt(6) W, -1, 1))/3`, `W = 2 sqrt(2) (S.S):S/(|S| S2)`,
//! `Us = sqrt(S2/2 + |skew(gradU)|^2)`, with `S = devSymm(gradU)`,
//! `S2 = 2|S|^2`. Epsilon source `C1 |S| epsilon` with
//! `C1 = max(eta/(5 + eta), 0.43)`, `eta = |S| k/epsilon`; epsilon sink
//! `Sp(C2 epsilon/(k + sqrt(nu epsilon)))`; k source `G = nut (gradU && devTwoSymm(gradU))`
//! with `SuSp((2/3) div U)` and `Sp(epsilon/k)`. Defaults: A0 4.0, C2 1.9,
//! sigmak 1.0, sigmaEps 1.2.
//!
//! **Status: AI draft.** [`RealizableKE::source_terms`] is compared per cell
//! against the real ESI model on a frozen duct
//! (`verification_and_validation/upstream_turb_gh1362/README.md`, slice 3);
//! `acos` and `cos` come from PETIR, so ulp differences against glibc are
//! counted and pinned there. `correct()` is NOT compared against upstream: no
//! wall functions, zero-gradient walls, `bound()` a plain floor at `small`.

use crate::k_epsilon_step::{self, scalar_field, EpsilonSources, StepInputs};
use crate::source_terms::{self, FrozenState, SourceTerms};
use crate::traits::TurbulenceModel;
use outram_foam_basic_lib::fv_operators::fvm;
use outram_foam_basic_lib::prelude::{
    Field, FvMesh, FvVectorMatrix, SurfaceScalarField, Tensor, VolScalarField, VolVectorField,
};
use petir::mathf::RealMath;
use std::sync::Arc;

/// upstream double `SMALL`.
const U_SMALL: f64 = 1e-15;

/// Per-cell intermediates of upstream `realizableKE::rCmu`.
#[derive(Debug, Clone, Copy)]
pub struct RcmuParts {
    /// `S2 = 2 magSqr(devSymm(gradU))` [1/s^2].
    pub s2: f64,
    /// `magS = sqrt(S2)` [1/s].
    pub mag_s: f64,
    /// `W` (cosine of three times the invariant angle).
    pub w: f64,
    /// `phis = acos(clamp(sqrt(6) W, -1, 1))/3`.
    pub phis: f64,
    /// `As = sqrt(6) cos(phis)`.
    pub as_: f64,
    /// `Us = sqrt(S2/2 + magSqr(skew(gradU)))` [1/s].
    pub us: f64,
    /// `rCmu = 1/(A0 + As Us k/epsilon)`.
    pub rcmu: f64,
}

/// Realizable k-epsilon model (incompressible, single phase).
pub struct RealizableKE {
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
    a0: f64,
    c2: f64,
    sigma_k: f64,
    sigma_eps: f64,
}

/// Upstream `realizableKE::rCmu` for one cell (translated line by line).
pub fn rcmu_cell(grad: &Tensor, k: f64, epsilon: f64, a0: f64) -> RcmuParts {
    let s = source_terms::dev_symm(grad);
    let s2 = 2.0 * source_terms::mag_sqr_symm6(&s);
    let mag_s = s2.sqrt();
    let ss = source_terms::symm_inner_symm(&s, &s);
    let w = ((2.0 * 2.0_f64.sqrt()) * source_terms::tensor_ddot_symm(&ss, &s))
        / (mag_s * s2 + U_SMALL);
    let arg = 6.0_f64.sqrt() * w;
    let clamped = if arg > -1.0 { arg } else { -1.0 };
    let clamped = if clamped < 1.0 { clamped } else { 1.0 };
    let phis = (1.0 / 3.0) * clamped.r_acos();
    let as_ = 6.0_f64.sqrt() * phis.r_cos();
    let us = (s2 / 2.0 + source_terms::mag_sqr_skew(grad)).sqrt();
    let rcmu = 1.0 / (a0 + as_ * us * k / epsilon);
    RcmuParts {
        s2,
        mag_s,
        w,
        phis,
        as_,
        us,
        rcmu,
    }
}

impl RealizableKE {
    /// Upstream defaults; fields start uniform (k = 1e-4, epsilon = 1e-4), U = 0.
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
            a0: 4.0,
            c2: 1.9,
            sigma_k: 1.0,
            sigma_eps: 1.2,
        };
        let g = source_terms::gauss_grad_u(&m.mesh, &m.u);
        m.correct_nut(&g);
        m
    }

    /// `nut = rCmu k^2/epsilon` on the given `grad(U)` (upstream `correctNut`).
    pub fn correct_nut(&mut self, grad_u: &[Tensor]) {
        let k = self.k.internal.as_slice();
        let e = self.epsilon.internal.as_slice();
        let v: Vec<f64> = (0..self.mesh.n_cells)
            .map(|c| rcmu_cell(&grad_u[c], k[c], e[c], self.a0).rcmu * (k[c] * k[c]) / e[c])
            .collect();
        self.nu_t.internal = Field::new(v);
    }

    /// Per-cell terms of the epsilon and k equations on the CURRENT fields
    /// (`state` carries `grad_u` and `div_u`), upstream v2506 arithmetic:
    /// `S2`, `magS`, `W`, `phis`, `As`, `Us`, `rCmu`, `nut` (recomputed),
    /// `eta`, `C1`, `G` (from the stored `nu_t`), `eps_E`, `eps_Sp`, `k_E`,
    /// `k_SuSp`, `k_Sp`. There is no epsilon `SuSp` term in this model.
    pub fn source_terms(&self, state: &FrozenState) -> SourceTerms {
        let n = self.mesh.n_cells;
        let k = self.k.internal.as_slice();
        let eps = self.epsilon.internal.as_slice();
        let nu = self.nu.internal.as_slice();
        let nut_stored = self.nu_t.internal.as_slice();
        let names = [
            "S2", "magS", "W", "phis", "As", "Us", "rCmu", "nut", "eta", "C1", "G", "eps_E",
            "eps_Sp", "k_E", "k_SuSp", "k_Sp",
        ];
        let mut cols: Vec<Vec<f64>> = names.iter().map(|_| vec![0.0; n]).collect();
        for c in 0..n {
            let p = rcmu_cell(&state.grad_u[c], k[c], eps[c], self.a0);
            let eta = p.mag_s * k[c] / eps[c];
            let c1 = {
                let r = eta / (5.0 + eta);
                if r > 0.43 {
                    r
                } else {
                    0.43
                }
            };
            let g = nut_stored[c] * source_terms::g_by_nu(&state.grad_u[c]);
            let nu_limited = if nu[c] > 0.0 { nu[c] } else { 0.0 };
            cols[0][c] = p.s2;
            cols[1][c] = p.mag_s;
            cols[2][c] = p.w;
            cols[3][c] = p.phis;
            cols[4][c] = p.as_;
            cols[5][c] = p.us;
            cols[6][c] = p.rcmu;
            cols[7][c] = p.rcmu * (k[c] * k[c]) / eps[c];
            cols[8][c] = eta;
            cols[9][c] = c1;
            cols[10][c] = g;
            cols[11][c] = c1 * p.mag_s * eps[c];
            cols[12][c] = self.c2 * eps[c] / (k[c] + (nu_limited * eps[c]).sqrt());
            cols[13][c] = g;
            cols[14][c] = (2.0 / 3.0) * state.div_u[c];
            cols[15][c] = eps[c] / k[c];
        }
        let mut out = SourceTerms::default();
        for (name, col) in names.iter().zip(cols) {
            out.push(name, col);
        }
        out
    }
}

impl TurbulenceModel for RealizableKE {
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
            susp: Vec::new(),
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
        self.correct_nut(&state.grad_u);
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
                .cell_volumes(vec![1.0, 1.0, 1.0])
                .cell_centres(vec![vx(0.5), vx(1.5), vx(2.5)])
                .face_area_vectors(vec![vx(1.0), vx(1.0), vx(-1.0), vx(1.0)])
                .face_centres(vec![vx(1.0), vx(2.0), vx(0.0), vx(3.0)])
                .build()
                .unwrap(),
        )
    }

    /// Zero strain: W = 0, phis = pi/6, As = sqrt(6) cos(pi/6) = 3/sqrt(2),
    /// Us = 0, so rCmu = 1/A0 = 0.25 exactly in value (to rounding).
    #[test]
    fn rcmu_at_rest_is_one_over_a0() {
        let p = rcmu_cell(&Tensor::ZERO, 0.2, 10.0, 4.0);
        assert_eq!(p.s2, 0.0);
        assert_eq!(p.w, 0.0);
        assert!((p.phis - std::f64::consts::PI / 6.0).abs() < 1e-15);
        assert!((p.as_ - 3.0 / 2.0_f64.sqrt()).abs() < 1e-14);
        assert!((p.rcmu - 0.25).abs() < 1e-15);
    }

    /// Realizability: for pure shear (traceless, W = 0) rCmu stays between 0 and 1/A0.
    #[test]
    fn rcmu_is_bounded_in_shear() {
        let shear = Tensor::new(0.0, 5.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        let p = rcmu_cell(&shear, 1.0, 1.0, 4.0);
        assert!(p.rcmu > 0.0 && p.rcmu < 0.25, "{}", p.rcmu);
        assert!(p.w.abs() < 1e-12);
    }

    /// Source terms at rest: G, eps_E and k_SuSp vanish; C1 takes its 0.43 floor.
    #[test]
    fn source_terms_at_rest() {
        let m = RealizableKE::new(line_mesh());
        let st = FrozenState {
            grad_u: vec![Tensor::ZERO; 3],
            div_u: vec![0.0; 3],
            ..FrozenState::default()
        };
        let t = m.source_terms(&st);
        for name in ["G", "eps_E", "k_SuSp", "magS"] {
            assert!(t.get(name).unwrap().iter().all(|v| *v == 0.0), "{name}");
        }
        assert!(t.get("C1").unwrap().iter().all(|v| *v == 0.43));
    }

    /// correct() on a shear field keeps k, epsilon positive and nut finite.
    #[test]
    fn correct_keeps_fields_positive_and_finite() {
        let m0 = line_mesh();
        let mut m = RealizableKE::new(m0.clone());
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
