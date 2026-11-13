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

//! Launder-Sharma low-Reynolds-number k-epsilon (Launder & Sharma 1974), ported from
//! OpenFOAM-v2506 (ESI) `LaunderSharmaKE.C` (gh:#1544, under #1377 and #1376).
//!
//! `nut = Cmu fMu k^2/epsilon`, `fMu = exp(-3.4/(1 + (k^2/(nu epsilon))/50)^2)`,
//! `f2 = 1 - 0.3 exp(-min((k^2/(nu epsilon))^2, 50))`. Epsilon equation sources:
//! `C1 G epsilon/k + E`, `SuSp(((2/3) C1 - C3) div U)`, `Sp(C2 f2 epsilon/k)`;
//! k equation: `G`, `SuSp((2/3) div U)`, `Sp((epsilon + D)/k)`, with the wall-damping terms
//! `E = 2 nu nut magSqrGradGrad(U)` and `D = 2 nu |grad(sqrt(k))|^2`.
//! Defaults `Cmu 0.09`, `C1 1.44`, `C2 1.92`, `C3 0`, `sigmak 1`, `sigmaEps 1.3`.
//!
//! **Status: AI draft, terms only.** [`LaunderSharmaKe::source_terms`] is compared per cell
//! against the real ESI model on a frozen duct (`upstream_turb_gh1362/README.md`, slice 6).
//! `fvc::magSqrGradGrad(U)` and `grad(sqrt(k))` are INPUTS ([`FrozenState`]): the port's
//! nested Gauss gradients are not compared with upstream's (gh:#1489). No `correct()` and no
//! [`crate::traits::TurbulenceModel`] impl yet.

use crate::source_terms::{self, FrozenState, SourceTerms};
use outram_foam_basic_lib::prelude::{FvMesh, VolScalarField};
use petir::mathf::RealMath;
use std::sync::Arc;

/// Launder-Sharma k-epsilon on a frozen field (terms only).
pub struct LaunderSharmaKe {
    pub mesh: Arc<FvMesh>,
    /// Turbulent kinetic energy k [m^2/s^2].
    pub k: VolScalarField,
    /// Dissipation rate epsilon [m^2/s^3].
    pub epsilon: VolScalarField,
    /// Eddy viscosity the model holds (`G` and `E` use this, as upstream's `nut_`).
    pub nu_t: VolScalarField,
    /// Molecular kinematic viscosity [m^2/s].
    pub nu: VolScalarField,
    cmu: f64,
    c1: f64,
    c2: f64,
    c3: f64,
}

impl LaunderSharmaKe {
    /// Upstream defaults; k = epsilon = 1e-4.
    pub fn new(mesh: Arc<FvMesh>) -> Self {
        Self {
            k: VolScalarField::uniform("k", mesh.clone(), 1e-4),
            epsilon: VolScalarField::uniform("epsilon", mesh.clone(), 1e-4),
            nu_t: VolScalarField::zeros("nut", mesh.clone()),
            nu: VolScalarField::uniform("nu", mesh.clone(), 1e-5),
            mesh,
            cmu: 0.09,
            c1: 1.44,
            c2: 1.92,
            c3: 0.0,
        }
    }

    /// Per-cell terms: `fMu`, `f2`, `nut` (recomputed), `E`, `D`, `G`, `eps_E1`
    /// (`C1 G epsilon/k`, the explicit epsilon source apart from `E`), `eps_SuSp`,
    /// `eps_Sp`, `k_E` (= `G`), `k_SuSp`, `k_Sp`. `state` needs `grad_u`, `div_u`,
    /// `mag_sqr_grad_grad_u`, `grad_sqrt_k`.
    pub fn source_terms(&self, state: &FrozenState) -> SourceTerms {
        let n = self.mesh.n_cells;
        let k = self.k.internal.as_slice();
        let eps = self.epsilon.internal.as_slice();
        let nu = self.nu.internal.as_slice();
        let nut = self.nu_t.internal.as_slice();
        let names = [
            "fMu", "f2", "nut", "E", "D", "G", "eps_E1", "eps_SuSp", "eps_Sp", "k_E",
            "k_SuSp", "k_Sp",
        ];
        let mut cols: Vec<Vec<f64>> = names.iter().map(|_| Vec::with_capacity(n)).collect();
        for c in 0..n {
            let kk = k[c] * k[c];
            let ratio = kk / (nu[c] * eps[c]);
            let one_plus = 1.0 + ratio / 50.0;
            let f_mu = (-3.4 / (one_plus * one_plus)).r_exp();
            let r2 = ratio * ratio;
            let f2 = 1.0 - 0.3 * (-source_terms::umin(r2, 50.0)).r_exp();
            let nut_new = ((self.cmu * f_mu) * kk) / eps[c];
            let e = ((2.0 * nu[c]) * nut[c]) * state.mag_sqr_grad_grad_u[c];
            let d = (2.0 * nu[c]) * source_terms::mag_sqr(state.grad_sqrt_k[c]);
            let g = nut[c] * source_terms::g_by_nu(&state.grad_u[c]);
            let row = [
                f_mu,
                f2,
                nut_new,
                e,
                d,
                g,
                ((self.c1 * g) * eps[c]) / k[c],
                ((2.0 / 3.0) * self.c1 - self.c3) * state.div_u[c],
                ((self.c2 * f2) * eps[c]) / k[c],
                g,
                (2.0 / 3.0) * state.div_u[c],
                (eps[c] + d) / k[c],
            ];
            for (col, v) in cols.iter_mut().zip(row) {
                col.push(v);
            }
        }
        let mut out = SourceTerms::default();
        for (name, col) in names.iter().zip(cols) {
            out.push(name, col);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::prelude::{
        BoundaryPatch, FvMeshBuilder, PatchKind, Tensor, Vector3,
    };

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

    fn state() -> FrozenState {
        FrozenState {
            grad_u: vec![Tensor::new(0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0); 3],
            div_u: vec![0.0; 3],
            mag_sqr_grad_grad_u: vec![0.5; 3],
            grad_sqrt_k: vec![vx(0.1); 3],
            ..FrozenState::default()
        }
    }

    /// Damping functions lie in (0, 1]: fMu -> 1 at high turbulence Reynolds number
    /// (k^2/(nu eps) large) and f2 -> 1; both are strictly below 1 at low Re.
    #[test]
    fn damping_functions_tend_to_one_at_high_reynolds_number() {
        let mut m = LaunderSharmaKe::new(line_mesh());
        m.k = VolScalarField::uniform("k", m.mesh.clone(), 10.0);
        m.epsilon = VolScalarField::uniform("epsilon", m.mesh.clone(), 1.0);
        let hi = m.source_terms(&state());
        assert!(hi.get("fMu").unwrap()[0] > 0.99 && hi.get("f2").unwrap()[0] > 0.99);
        m.k = VolScalarField::uniform("k", m.mesh.clone(), 1e-3);
        m.epsilon = VolScalarField::uniform("epsilon", m.mesh.clone(), 1.0);
        let lo = m.source_terms(&state());
        assert!(lo.get("fMu").unwrap()[0] < 0.1 && lo.get("f2").unwrap()[0] < 0.75);
    }

    /// The k destruction is (epsilon + D)/k and the epsilon source is C1 G epsilon/k.
    #[test]
    fn k_sink_and_epsilon_source_follow_the_equations() {
        let mut m = LaunderSharmaKe::new(line_mesh());
        m.nu_t = VolScalarField::uniform("nut", m.mesh.clone(), 1e-3);
        let t = m.source_terms(&state());
        let (k, e) = (1e-4_f64, 1e-4_f64);
        let d = 2.0 * 1e-5 * (0.1 * 0.1);
        assert!((t.get("k_Sp").unwrap()[0] - (e + d) / k).abs() < 1e-9);
        let g = t.get("G").unwrap()[0];
        assert!((t.get("eps_E1").unwrap()[0] - 1.44 * g * e / k).abs() < 1e-12);
        assert_eq!(t.get("k_E").unwrap(), t.get("G").unwrap());
    }
}
