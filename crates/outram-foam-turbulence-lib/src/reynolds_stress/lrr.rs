// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com), RASModels/LRR/LRR.C (v2506)
// Copyright (C) 2011-2016 OpenFOAM Foundation
// Copyright (C) 2020 OpenCFD Ltd.
//
// This file is part of OUTRAM PARK. It is free software under the GNU General
// Public License v3 or later, WITHOUT ANY WARRANTY; see <https://www.gnu.org/licenses/>.

//! Launder-Reece-Rodi (1975) Reynolds-stress model, ported from OpenFOAM-v2506 (ESI)
//! `RASModels/LRR/LRR.C` (gh:#1544).
//!
//! `k = tr(R)/2`, `nut = Cmu k^2/epsilon`, `P = -twoSymm(R & gradU)`, `G = |tr P|/2`.
//! Epsilon equation: `ddt + div - laplacian(DepsilonEff) == Ceps1 G epsilon/k - Sp(Ceps2 epsilon/k)`.
//! R equation: `ddt + div - laplacian(DREff) + Sp(C1 epsilon/k) == P - (2/3)(1 - C1) epsilon I
//! - C2 dev(P)` plus, with `wallReflection` (the upstream DEFAULT, on here), the wall-reflection
//! term `(3 Cmu^0.75/kappa)(sqrt(k)/y) devSymm((n & reflect) n)` added through `fvMatrix::operator+=`
//! with `reflect = Cref1 R - (Cref2 C2 k/epsilon) dev(P)`. In wall cells `P` is first rescaled
//! (see [`super::wall_correct_production`]). Defaults: `Cmu 0.09`, `C1 1.8`, `C2 0.6`,
//! `Ceps1 1.44`, `Ceps2 1.92`, `Cs 0.25`, `Ceps 0.15`, `kappa 0.41`, `Cref1 0.5`, `Cref2 0.3`.
//!
//! **Status: AI draft, terms only**; see [`super`] for what is and is not compared.

use super::{
    dev_symm_tensor, effective_diffusivity, gen_g, production, push_symm, wall_correct_production,
    wall_face_cells, ReynoldsStressFrozen,
};
use crate::source_terms::SourceTerms;
use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;
use outram_foam_basic_lib::primitives::{SphericalTensor, SymmTensor, Vector3};
use petir::mathf::RealMath;
use std::sync::Arc;

/// LRR on a frozen field (terms only).
pub struct Lrr {
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
    /// `C1`, upstream 1.8.
    pub c1: f64,
    /// `C2`, upstream 0.6.
    pub c2: f64,
    /// `Ceps1`, upstream 1.44.
    pub ceps1: f64,
    /// `Ceps2`, upstream 1.92.
    pub ceps2: f64,
    /// `Cs`, upstream 0.25.
    pub cs: f64,
    /// `Ceps`, upstream 0.15.
    pub ceps: f64,
    /// `kappa`, upstream 0.41.
    pub kappa: f64,
    /// `Cref1`, upstream 0.5.
    pub cref1: f64,
    /// `Cref2`, upstream 0.3.
    pub cref2: f64,
    /// Wall-reflection term; upstream default `true`, and so here (disable it explicitly).
    pub wall_reflection: bool,
}

impl Lrr {
    /// Upstream defaults with wall reflection ON; `R`, `epsilon` and `nu` zero-sized until set.
    pub fn new(mesh: Arc<FvMesh>) -> Self {
        let n = mesh.n_cells;
        Self {
            mesh,
            r: vec![SymmTensor::ZERO; n],
            epsilon: vec![0.0; n],
            nu: vec![0.0; n],
            cmu: 0.09,
            c1: 1.8,
            c2: 0.6,
            ceps1: 1.44,
            ceps2: 1.92,
            cs: 0.25,
            ceps: 0.15,
            kappa: 0.41,
            cref1: 0.5,
            cref2: 0.3,
            wall_reflection: true,
        }
    }

    /// Explicit ablation of the wall-reflection term (a visible act, never the default).
    pub fn without_wall_reflection(mut self) -> Self {
        self.wall_reflection = false;
        self
    }

    /// `k = 0.5*tr(R)` per cell.
    pub fn k(&self) -> Vec<f64> {
        self.r.iter().map(|r| 0.5 * r.tr()).collect()
    }

    /// `nut = Cmu*sqr(k)/epsilon` per cell (`LRR::correctNut`).
    pub fn nut(&self) -> Vec<f64> {
        self.k()
            .iter()
            .zip(&self.epsilon)
            .map(|(k, e)| (self.cmu * (k * k)) / e)
            .collect()
    }

    /// Per-cell terms: `k`, `nut`, `P_*` (unrescaled), `G`, `DepsEff_*`, `DREff_*`, `eps_E`,
    /// `eps_Sp`, `Pw_*` (wall-rescaled), `R_Sp`, `R_Su_*` (the explicit R source without the
    /// reflection term), and with wall reflection `reflect_*`, `refl_coef`, `refl_*`.
    /// `state` needs `grad_u`, and `wall_distance`, `wall_normal` when `wall_reflection`.
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
        let r_sp: Vec<f64> = (0..n).map(|c| (self.c1 * self.epsilon[c]) / k[c]).collect();
        let iso = (2.0 / 3.0) * (1.0 - self.c1);
        let su: Vec<SymmTensor> = (0..n)
            .map(|c| {
                (pw[c] - SphericalTensor::new(iso * self.epsilon[c]))
                    - (self.c2 * dev_symm_tensor(pw[c]))
            })
            .collect();

        let mut out = SourceTerms::default();
        out.push("k", k.clone());
        out.push("nut", nut);
        push_symm(&mut out, "P", &p);
        out.push("G", g);
        push_symm(&mut out, "DepsEff", &deps);
        push_symm(&mut out, "DREff", &dr);
        out.push("eps_E", eps_e);
        out.push("eps_Sp", eps_sp);
        push_symm(&mut out, "Pw", &pw);
        out.push("R_Sp", r_sp);
        push_symm(&mut out, "R_Su", &su);
        if self.wall_reflection {
            let coef0 = (3.0 * self.cmu.r_powf(0.75)) / self.kappa;
            let reflect: Vec<SymmTensor> = (0..n)
                .map(|c| {
                    (self.cref1 * self.r[c])
                        - (((self.cref2 * self.c2) * (k[c] / self.epsilon[c]))
                            * dev_symm_tensor(pw[c]))
                })
                .collect();
            let coef: Vec<f64> = (0..n)
                .map(|c| coef0 * (k[c].sqrt() / state.wall_distance[c]))
                .collect();
            let refl: Vec<SymmTensor> = (0..n)
                .map(|c| {
                    let nn: Vector3 = state.wall_normal[c];
                    let nr = nn_dot_symm(nn, reflect[c]);
                    coef[c] * (nr * nn).dev_symm()
                })
                .collect();
            push_symm(&mut out, "reflect", &reflect);
            out.push("refl_coef", coef);
            push_symm(&mut out, "refl", &refl);
        }
        out
    }
}

/// Upstream `operator&(Vector, SymmTensor)`.
fn nn_dot_symm(v: Vector3, st: SymmTensor) -> Vector3 {
    Vector3::new(
        v.x * st.xx + v.y * st.xy + v.z * st.xz,
        v.x * st.xy + v.y * st.yy + v.z * st.yz,
        v.x * st.xz + v.y * st.yz + v.z * st.zz,
    )
}
