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

//! Lagrangian two-equation eddy-viscosity model `dynamicLagrangian` (Meneveau, Lund & Cabot 1996),
//! ported from OpenFOAM-v2506 (ESI) `LES/dynamicLagrangian/dynamicLagrangian.C`
//! (gh:#1544, under #1377).
//!
//! `nut = (flm/fmm) delta^2 |devSymm(grad U)|`, with the two Lagrangian-averaged fields `flm`
//! (`<L M>`) and `fmm` (`<M M>`) transported along the flow:
//!
//! * `L = dev(filter(sqr(U)) - sqr(filter(U)))`
//! * `M = 2 delta^2 (filter(|S| S) - 4 |Sf| Sf)`, `S = devSymm(grad U)`, `Sf = devSymm(grad(filter U))`
//! * `invT = (1/(theta delta)) (flm fmm)^(1/8)`, `theta = 1.5`
//! * `ddt(flm) + div(phi, flm) == invT (L && M) - Sp(invT, flm)` and the same for `fmm` with `M && M`.
//!
//! **Status: AI draft, terms only.** [`DynamicLagrangian::terms`] is compared per cell against the
//! real ESI model on a frozen duct (`upstream_turb_gh1362/README.md`, slice 7). There is no
//! `correct()` here (the two transport equations, `bound` and `correctNut` after the solves are not
//! ported), and no `TurbulenceModel` impl. The gradients are the port's own
//! [`grad_vec_corrected`], which is bit-identical to the real `fvc::grad` on the duct.

use crate::les::pointwise::{
    boundary_values, calculated, ddot_st, dev_symm_st, dev_symm_tensor, mag_sqr_st, map1, map2,
    sqr_vec,
};
use outram_foam_basic_lib::fields::vol_field::VolField;
use outram_foam_basic_lib::fv_operators::fvc::grad_vec_corrected;
use outram_foam_basic_lib::fv_operators::les_filter::LesFilter;
use outram_foam_basic_lib::prelude::{
    FvMesh, SymmTensor, VolScalarField, VolVectorField,
};
use petir::mathf::RealMath;
use std::sync::Arc;

/// `SMALL` (double): the boundary value of `delta` (`LESdelta.C`).
const SMALL: f64 = 1e-15;

/// `dynamicLagrangian` LES model (incompressible, single phase).
pub struct DynamicLagrangian {
    pub mesh: Arc<FvMesh>,
    /// `<L M>` [m^4/s^4]; its face values come from its boundary conditions.
    pub flm: VolScalarField,
    /// `<M M>` [m^4/s^4].
    pub fmm: VolScalarField,
    /// Velocity [m/s].
    pub u: VolVectorField,
    filter: LesFilter,
    /// The `theta` coefficient (upstream default 1.5).
    pub theta: f64,
    delta: Vec<f64>,
}

/// Every field the model forms in `correct()`, with boundary values where upstream has them.
#[derive(Debug, Clone)]
pub struct LagrangianTerms {
    /// `devSymm(grad U)`.
    pub s: VolField<SymmTensor>,
    /// `mag(S)`.
    pub mag_s: VolScalarField,
    /// `filter(U)`.
    pub uf: VolVectorField,
    /// `devSymm(grad(filter(U)))`.
    pub sf: VolField<SymmTensor>,
    /// `mag(Sf)`.
    pub mag_sf: VolScalarField,
    /// `L`.
    pub l: VolField<SymmTensor>,
    /// `M`.
    pub m: VolField<SymmTensor>,
    /// `invT` (the `Sp` coefficient of both equations).
    pub inv_t: VolScalarField,
    /// `L && M` (explicit source of the `flm` equation is `invT*LM`).
    pub lm: VolScalarField,
    /// `M && M` (explicit source of the `fmm` equation is `invT*MM`).
    pub mm: VolScalarField,
    /// `nut = (flm/fmm) delta^2 |devSymm(grad U)|`, cells.
    pub nut: Vec<f64>,
}

impl DynamicLagrangian {
    /// `flm`, `fmm` start uniform at 1e-4, `theta = 1.5`, `delta = 1.0 cbrt(V)` (cubeRootVol).
    pub fn new(mesh: Arc<FvMesh>, filter: LesFilter) -> Self {
        let delta = (0..mesh.n_cells)
            .map(|c| 1.0 * mesh.cell_volumes[c].r_cbrt())
            .collect();
        Self {
            flm: VolScalarField::uniform("flm", mesh.clone(), 1e-4),
            fmm: VolScalarField::uniform("fmm", mesh.clone(), 1e-4),
            u: VolVectorField::zero("U", mesh.clone()),
            mesh,
            filter,
            theta: 1.5,
            delta,
        }
    }

    /// The test filter.
    pub fn filter(&self) -> &LesFilter {
        &self.filter
    }

    /// `delta` as upstream holds it: calculated patches at `small` (see `dynamic_k_eqn`);
    /// `cell_length` overrides the cell values.
    fn delta_field(&self, cell_length: &Option<Vec<f64>>) -> VolScalarField {
        let cells = match cell_length {
            Some(v) => v.clone(),
            None => self.delta.clone(),
        };
        let faces = self.mesh.patches.iter().map(|p| vec![SMALL; p.size]).collect();
        calculated("delta", &self.mesh, cells, faces)
    }

    /// The fields of `correct()` on the CURRENT `u`, `flm`, `fmm`; `cell_length` overrides the
    /// width `delta` (the upstream `pow(V, 1/3)` value, to separate it from PETIR's `cbrt`).
    pub fn terms(&self, cell_length: &Option<Vec<f64>>) -> LagrangianTerms {
        let mesh = &self.mesh;
        let delta = self.delta_field(cell_length);
        let u = crate::les::pointwise::as_calculated(&self.u);
        let grad_u = grad_vec_corrected(&self.u);
        let s = map1(&grad_u, dev_symm_tensor);
        let mag_s = map1(&s, |t| mag_sqr_st(t).sqrt());
        let uf = self.filter.apply(&self.u);
        let sf = map1(&grad_vec_corrected(&uf), dev_symm_tensor);
        let mag_sf = map1(&sf, |t| mag_sqr_st(t).sqrt());
        let sqr_u = map1(&u, sqr_vec);
        let f_sqr = self.filter.apply(&sqr_u);
        let sqr_f_u = map1(&self.filter.apply(&self.u), sqr_vec);
        let l = map1(&map2(&f_sqr, &sqr_f_u, |a, b| *a - *b), dev_symm_st);
        // M = 2.0*sqr(delta)*(filter(magS*S) - 4.0*magSf*Sf)
        let mag_s_s = map2(&mag_s, &s, |m, t| *t * *m);
        let f_mag_s_s = self.filter.apply(&mag_s_s);
        let four_sf = map2(&mag_sf, &sf, |m, t| *t * (4.0 * *m));
        let diff = map2(&f_mag_s_s, &four_sf, |a, b| *a - *b);
        let two_delta_sq = map1(&delta, |d| 2.0 * (*d * *d));
        let m = map2(&two_delta_sq, &diff, |a, b| *b * *a);
        // invT = (1.0/(theta*delta))*pow(flm*fmm, 1.0/8.0)
        let flm = crate::les::pointwise::as_calculated(&self.flm);
        let fmm = crate::les::pointwise::as_calculated(&self.fmm);
        let inv_theta_delta = map1(&delta, |d| 1.0 / (self.theta * *d));
        let pw = map2(&flm, &fmm, |a, b| (*a * *b).r_powf(1.0 / 8.0));
        let inv_t = map2(&inv_theta_delta, &pw, |a, b| *a * *b);
        let lm = map2(&l, &m, ddot_st);
        let mm = map1(&m, |t| ddot_st(t, t));
        let nut: Vec<f64> = (0..mesh.n_cells)
            .map(|c| {
                ((flm.internal[c] / fmm.internal[c]) * (delta.internal[c] * delta.internal[c]))
                    * mag_s.internal[c]
            })
            .collect();
        LagrangianTerms { s, mag_s, uf, sf, mag_sf, l, m, inv_t, lm, mm, nut }
    }

    /// The boundary face values of `u` (any boundary condition), patch order.
    pub fn u_faces(&self) -> Vec<Vec<outram_foam_basic_lib::prelude::Vector3>> {
        boundary_values(&self.u)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::fields::boundary::bc::{BoundaryCondition, PatchField};
    use outram_foam_basic_lib::fields::field::Field;
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

    /// A uniform velocity has zero strain: `S`, `M`, `L`, `LM` and `nut` vanish, `invT` is
    /// finite and positive.
    #[test]
    fn uniform_velocity_has_no_strain_and_zero_nut() {
        let mesh = line_mesh();
        let mut m = DynamicLagrangian::new(mesh.clone(), LesFilter::Simple);
        let bnd = (0..2)
            .map(|_| PatchField {
                bc: BoundaryCondition::FixedField(Field::new(vec![vx(2.0)])),
                values: Field::new(vec![vx(2.0)]),
            })
            .collect();
        m.u = VolVectorField::new("U", mesh.clone(), Field::new(vec![vx(2.0); 3]), bnd);
        let t = m.terms(&None);
        for c in 0..3 {
            assert!(t.mag_s.internal[c].abs() < 1e-12);
            assert!(t.mag_sf.internal[c].abs() < 1e-12);
            assert!(t.nut[c].abs() < 1e-12);
            assert!(t.lm.internal[c].abs() < 1e-12);
            assert!(t.mm.internal[c].abs() < 1e-12);
            assert!(t.inv_t.internal[c] > 0.0 && t.inv_t.internal[c].is_finite());
        }
        assert_eq!(t.s.internal.len(), 3);
        assert_eq!(t.uf.internal.len(), 3);
        assert_eq!(t.sf.internal.len(), 3);
        assert_eq!(t.l.internal.len(), 3);
        assert_eq!(t.m.internal.len(), 3);
        assert!(matches!(m.filter(), LesFilter::Simple));
        assert_eq!(m.u_faces().len(), 2);
        assert_eq!(m.theta, 1.5);
    }
}
