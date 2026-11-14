// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), RASModels/SSG/SSG.C (v2506)
// Copyright (C) 2011-2016 OpenFOAM Foundation
// Copyright (C) 2020 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK. It is free software under the GNU General
// Public License v3 or later, WITHOUT ANY WARRANTY; see <https://www.gnu.org/licenses/>.

//! Speziale-Sarkar-Gatski (1991) Reynolds-stress model, ported from OpenFOAM-v2506 (ESI)
//! `RASModels/SSG/SSG.C` (gh:#1544).
//!
//! With `b = dev(R)/(2k)`, `S = symm(gradU)`, `Omega = skew(gradU)`: epsilon equation as in
//! [`super::lrr::Lrr`]; R equation
//! `ddt + div - laplacian(DREff) + Sp((C1/2 epsilon + C1s/2 G)/k) == P
//!  - (1/3)((2 - C1) epsilon - C1s G) I + C2 epsilon dev(innerSqr(b))
//!  + k ((C3 - C3s |b|) dev(S) + C4 devTwoSymm(b & S) + C5 twoSymm(b & Omega))`.
//! There is no wall-reflection term. In wall cells `P` is rescaled first. Defaults: `Cmu 0.09`,
//! `C1 3.4`, `C1s 1.8`, `C2 4.2`, `C3 0.8`, `C3s 1.3`, `C4 1.25`, `C5 0.4`, `Ceps1 1.44`,
//! `Ceps2 1.92`, `Cs 0.25`, `Ceps 0.15`.
//!
//! **Status: AI draft, terms only**; see [`super`] for what is and is not compared.

use super::{
    dev_symm_tensor, effective_diffusivity, gen_g, production, push_symm, push_tensor,
    wall_correct_production, wall_face_cells, ReynoldsStressFrozen,
};
use crate::source_terms::SourceTerms;
use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;
use outram_foam_basic_lib::primitives::{SphericalTensor, SymmTensor, Tensor};
use std::sync::Arc;

/// SSG on a frozen field (terms only).
pub struct Ssg {
    /// The mesh (wall patches decide where `P` is rescaled).
    pub mesh: Arc<FvMesh>,
    /// Reynolds stress `R` per cell [m^2/s^2].
    pub r: Vec<SymmTensor>,
    /// Dissipation rate per cell [m^2/s^3].
    pub epsilon: Vec<f64>,
    /// Molecular kinematic viscosity per cell [m^2/s].
    pub nu: Vec<f64>,
    /// `Cmu`, upstream 0.09.
    pub cmu: f64,
    /// `C1`, upstream 3.4.
    pub c1: f64,
    /// `C1s`, upstream 1.8.
    pub c1s: f64,
    /// `C2`, upstream 4.2.
    pub c2: f64,
    /// `C3`, upstream 0.8.
    pub c3: f64,
    /// `C3s`, upstream 1.3.
    pub c3s: f64,
    /// `C4`, upstream 1.25.
    pub c4: f64,
    /// `C5`, upstream 0.4.
    pub c5: f64,
    /// `Ceps1`, upstream 1.44.
    pub ceps1: f64,
    /// `Ceps2`, upstream 1.92.
    pub ceps2: f64,
    /// `Cs`, upstream 0.25.
    pub cs: f64,
    /// `Ceps`, upstream 0.15.
    pub ceps: f64,
}

impl Ssg {
    /// Upstream defaults; `R`, `epsilon` and `nu` zero until set.
    pub fn new(mesh: Arc<FvMesh>) -> Self {
        let n = mesh.n_cells;
        Self {
            mesh,
            r: vec![SymmTensor::ZERO; n],
            epsilon: vec![0.0; n],
            nu: vec![0.0; n],
            cmu: 0.09,
            c1: 3.4,
            c1s: 1.8,
            c2: 4.2,
            c3: 0.8,
            c3s: 1.3,
            c4: 1.25,
            c5: 0.4,
            ceps1: 1.44,
            ceps2: 1.92,
            cs: 0.25,
            ceps: 0.15,
        }
    }

    /// `k = 0.5*tr(R)` per cell.
    pub fn k(&self) -> Vec<f64> {
        self.r.iter().map(|r| 0.5 * r.tr()).collect()
    }

    /// `nut = Cmu*sqr(k)/epsilon` per cell (`SSG::correctNut`).
    pub fn nut(&self) -> Vec<f64> {
        self.k()
            .iter()
            .zip(&self.epsilon)
            .map(|(k, e)| (self.cmu * (k * k)) / e)
            .collect()
    }

    /// Per-cell terms: `k`, `nut`, `P_*`, `G`, `DepsEff_*`, `DREff_*`, `eps_E`, `eps_Sp`, `Pw_*`,
    /// `b_*`, `S_*`, `Omega_*`, `magb`, `R_Sp`, `t1_*`, `t2_*`, `R_Su_*`. `state` needs `grad_u`.
    pub fn source_terms(&self, state: &ReynoldsStressFrozen) -> SourceTerms {
        let n = self.mesh.n_cells;
        let k = self.k();
        let nut = self.nut();
        let p: Vec<SymmTensor> = (0..n).map(|c| production(self.r[c], &state.grad_u[c])).collect();
        let g: Vec<f64> = p.iter().map(|p| gen_g(*p)).collect();
        let deps: Vec<SymmTensor> = (0..n)
            .map(|c| effective_diffusivity(self.ceps, k[c], self.epsilon[c], self.r[c], self.nu[c]))
            .collect();
        let dr: Vec<SymmTensor> = (0..n)
            .map(|c| effective_diffusivity(self.cs, k[c], self.epsilon[c], self.r[c], self.nu[c]))
            .collect();
        let eps_e: Vec<f64> = (0..n)
            .map(|c| ((self.ceps1 * g[c]) * self.epsilon[c]) / k[c])
            .collect();
        let eps_sp: Vec<f64> = (0..n).map(|c| (self.ceps2 * self.epsilon[c]) / k[c]).collect();
        let mut pw = p.clone();
        wall_correct_production(&mut pw, &g, &wall_face_cells(&self.mesh));

        let b: Vec<SymmTensor> = (0..n)
            .map(|c| dev_symm_tensor(self.r[c]) / (2.0 * k[c]))
            .collect();
        let s: Vec<SymmTensor> = (0..n).map(|c| state.grad_u[c].symm()).collect();
        let omega: Vec<Tensor> = (0..n).map(|c| state.grad_u[c].skew()).collect();
        let magb: Vec<f64> = b.iter().map(|b| b.mag()).collect();
        let r_sp: Vec<f64> = (0..n)
            .map(|c| ((self.c1 / 2.0) * self.epsilon[c] + (self.c1s / 2.0) * g[c]) / k[c])
            .collect();
        let t1: Vec<SymmTensor> = (0..n)
            .map(|c| (self.c2 * self.epsilon[c]) * dev_symm_tensor(b[c].inner_sqr()))
            .collect();
        let t2: Vec<SymmTensor> = (0..n)
            .map(|c| {
                let a = (self.c3 - self.c3s * magb[c]) * dev_symm_tensor(s[c]);
                let bb = self.c4 * (b[c] * s[c]).dev_two_symm();
                let cc = self.c5 * (b[c] * omega[c]).two_symm();
                k[c] * ((a + bb) + cc)
            })
            .collect();
        let su: Vec<SymmTensor> = (0..n)
            .map(|c| {
                let sph = (1.0 / 3.0)
                    * ((2.0 - self.c1) * self.epsilon[c] - self.c1s * g[c]);
                ((pw[c] - SphericalTensor::new(sph)) + t1[c]) + t2[c]
            })
            .collect();

        let mut out = SourceTerms::default();
        out.push("k", k);
        out.push("nut", nut);
        push_symm(&mut out, "P", &p);
        out.push("G", g);
        push_symm(&mut out, "DepsEff", &deps);
        push_symm(&mut out, "DREff", &dr);
        out.push("eps_E", eps_e);
        out.push("eps_Sp", eps_sp);
        push_symm(&mut out, "Pw", &pw);
        push_symm(&mut out, "b", &b);
        push_symm(&mut out, "S", &s);
        push_tensor(&mut out, "Omega", &omega);
        out.push("magb", magb);
        out.push("R_Sp", r_sp);
        push_symm(&mut out, "t1", &t1);
        push_symm(&mut out, "t2", &t2);
        push_symm(&mut out, "R_Su", &su);
        out
    }
}
