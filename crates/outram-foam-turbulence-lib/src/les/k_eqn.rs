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

//! One-equation sub-grid-scale kinetic-energy model `kEqn` (Yoshizawa & Horiuti 1985),
//! ported from OpenFOAM-v2506 (ESI) `kEqn.C` (gh:#1544, under #1377 and #1376).
//!
//! Transport of the sub-grid kinetic energy `k`:
//! `ddt(k) + div(phi, k) - laplacian(nut + nu, k) == G - SuSp((2/3) div U, k) - Sp(Ce sqrt(k)/delta, k)`
//! with `G = nut (gradU && devTwoSymm(gradU))` and `nut = Ck sqrt(k) delta`.
//! Defaults `Ck = 0.094` (the model's), `Ce = 1.048` (`LESModel`'s), `cubeRootVol` delta.
//!
//! **Status: AI draft.** [`KEqn::source_terms`] is compared per cell against the real ESI
//! model on a frozen duct (`upstream_turb_gh1362/README.md`, slice 5). `correct()` is NOT
//! compared: it uses the port's own solver settings, a plain floor at `small` instead of
//! upstream's neighbour-averaging `bound()`, and no `fvOptions`.

use crate::k_epsilon_step::{scalar_field, U_SMALL};
use crate::source_terms::{self, FrozenState, SourceTerms};
use crate::traits::TurbulenceModel;
use outram_foam_basic_lib::fv_operators::{fvc, fvm};
use outram_foam_basic_lib::prelude::{
    Field, FvMesh, FvVectorMatrix, SolverSettings, SurfaceScalarField, VolScalarField,
    VolVectorField,
};
use petir::mathf::RealMath;
use std::sync::Arc;

/// `kEqn` LES model (incompressible, single phase).
pub struct KEqn {
    pub mesh: Arc<FvMesh>,
    /// Sub-grid kinetic energy k [m^2/s^2].
    pub k: VolScalarField,
    /// Sub-grid eddy viscosity [m^2/s].
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
    ck: f64,
    ce: f64,
    delta: Vec<f64>,
}

impl KEqn {
    /// Upstream defaults; k starts at 1e-4, `delta = 1.0 cbrt(V)`.
    pub fn new(mesh: Arc<FvMesh>) -> Self {
        let delta = (0..mesh.n_cells)
            .map(|c| 1.0 * mesh.cell_volumes[c].r_cbrt())
            .collect();
        let mut m = Self {
            k: VolScalarField::uniform("k", mesh.clone(), 1e-4),
            nu_t: VolScalarField::zeros("nut", mesh.clone()),
            u: VolVectorField::zero("U", mesh.clone()),
            phi: SurfaceScalarField::zeros("phi", mesh.clone()),
            nu: VolScalarField::uniform("nu", mesh.clone(), 1e-5),
            mesh,
            dt: 1e-3,
            prt: 0.85,
            ck: 0.094,
            ce: 1.048,
            delta,
        };
        m.correct_nut();
        m
    }

    /// The per-cell `cubeRootVol` width [m].
    pub fn delta(&self) -> &[f64] {
        &self.delta
    }

    /// `nut = Ck sqrt(k) delta` (upstream `correctNut`).
    pub fn correct_nut(&mut self) {
        let k = self.k.internal.as_slice();
        let v: Vec<f64> = (0..self.mesh.n_cells)
            .map(|c| (self.ck * k[c].sqrt()) * self.delta[c])
            .collect();
        self.nu_t.internal = Field::new(v);
    }

    /// Per-cell terms of the k equation on the CURRENT fields (`state` carries `grad_u`,
    /// `div_u`; `cell_length` overrides the width): `delta`, `nut`, `GbyNu`, `G`
    /// (`= nu_t GbyNu` with the STORED `nu_t`), `k_SuSp`, `k_Sp`.
    pub fn source_terms(&self, state: &FrozenState) -> SourceTerms {
        let n = self.mesh.n_cells;
        let k = self.k.internal.as_slice();
        let nut = self.nu_t.internal.as_slice();
        let delta: &[f64] = match &state.cell_length {
            Some(v) => v.as_slice(),
            None => &self.delta,
        };
        let mut cols: Vec<Vec<f64>> = (0..6).map(|_| Vec::with_capacity(n)).collect();
        for c in 0..n {
            let gbn = source_terms::g_by_nu(&state.grad_u[c]);
            cols[0].push(delta[c]);
            cols[1].push((self.ck * k[c].sqrt()) * delta[c]);
            cols[2].push(gbn);
            cols[3].push(nut[c] * gbn);
            cols[4].push((2.0 / 3.0) * state.div_u[c]);
            cols[5].push((self.ce * k[c].sqrt()) / delta[c]);
        }
        let names = ["delta", "nut", "GbyNu", "G", "k_SuSp", "k_Sp"];
        let mut out = SourceTerms::default();
        for (name, col) in names.iter().zip(cols) {
            out.push(name, col);
        }
        out
    }
}

impl TurbulenceModel for KEqn {
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

    /// One transient k step with the ESI-form sources, then `nut` from the new k.
    fn correct(&mut self) {
        let n = self.mesh.n_cells;
        let state = FrozenState {
            grad_u: source_terms::gauss_grad_u(&self.mesh, &self.u),
            div_u: source_terms::div_phi(&self.mesh, &self.phi),
            ..FrozenState::default()
        };
        let t = self.source_terms(&state);
        let g = t.get("G").unwrap().to_vec();
        let susp = t.get("k_SuSp").unwrap().to_vec();
        let sp = t.get("k_Sp").unwrap().to_vec();
        let nut = self.nu_t.internal.as_slice();
        let nu = self.nu.internal.as_slice();
        let d_k: Vec<f64> = (0..n).map(|c| nut[c] + nu[c]).collect();
        let dk_f = fvc::interpolate(&scalar_field("DkEff", self.mesh.clone(), d_k));
        let k_old = self.k.clone();
        let mut eqn = fvm::ddt(&self.k, &k_old, self.dt)
            + fvm::div(&self.phi, &self.k)
            + fvm::laplacian(&dk_f, &self.k);
        for c in 0..n {
            let v = self.mesh.cell_volumes[c];
            eqn.source[c] += v * g[c];
            eqn.ldu.diag[c] += v * sp[c];
            eqn.ldu.diag[c] += v * susp[c].max(0.0);
            eqn.source[c] -= v * susp[c].min(0.0) * k_old.internal[c];
        }
        let (k_new, _) = eqn.solve("k", SolverSettings::default());
        self.k = k_new;
        for v in self.k.internal.as_mut_slice() {
            if *v < U_SMALL {
                *v = U_SMALL;
            }
        }
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

    /// nut = Ck sqrt(k) delta at construction; k_Sp = Ce sqrt(k)/delta.
    #[test]
    fn nut_and_dissipation_follow_k_and_delta() {
        let m = KEqn::new(line_mesh());
        let d = m.delta()[0];
        assert!((m.nu_t.internal[0] - 0.094 * 1e-2 * d).abs() < 1e-15);
        let st = FrozenState {
            grad_u: vec![outram_foam_basic_lib::prelude::Tensor::ZERO; 3],
            div_u: vec![0.0; 3],
            ..FrozenState::default()
        };
        let t = m.source_terms(&st);
        assert!((t.get("k_Sp").unwrap()[0] - 1.048 * 1e-2 / d).abs() < 1e-15);
        assert_eq!(t.get("G").unwrap(), &[0.0, 0.0, 0.0][..]);
    }

    /// correct() on a strained field keeps k and nut finite and non-negative and
    /// the trait accessors work.
    #[test]
    fn correct_keeps_k_positive_and_finite() {
        let mut m = KEqn::new(line_mesh());
        m.u.internal = Field::new(vec![vx(1.0), vx(4.0), vx(9.0)]);
        m.correct();
        for c in 0..3 {
            assert!(m.k.internal[c].is_finite() && m.k.internal[c] > 0.0);
            assert!(m.nu_t.internal[c].is_finite() && m.nu_t.internal[c] >= 0.0);
        }
        let nu = m.nu.clone();
        assert_eq!(m.mu_eff_field(&nu).internal.len(), 3);
        assert_eq!(m.alpha_eff(&nu).internal.len(), 3);
        let u = m.u.clone();
        let _ = m.div_dev_rho_reff(&u);
        assert_eq!(m.nu_t().internal.len(), 3);
    }
}
