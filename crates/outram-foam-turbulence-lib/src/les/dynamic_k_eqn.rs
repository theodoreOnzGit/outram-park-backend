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

//! Dynamic one-equation sub-grid kinetic-energy model `dynamicKEqn` (Kim & Menon 1995),
//! ported from OpenFOAM-v2506 (ESI) `LES/dynamicKEqn/dynamicKEqn.C` (gh:#1544, under #1377).
//!
//! `Ck` and `Ce` are not constants but come from the Germano identity, evaluated with the
//! `simple` filter (outer averaging) and the chosen test filter ([`LesFilter`], the family ported
//! in `outram_foam_basic_lib::fv_operators::les_filter`):
//!
//! * `LL = simple(dev(filter(sqr(U)) - sqr(filter(U))))`
//! * `MM = simple(-2 delta sqrt(max(KK, 0)) filter(D))`, `KK = 1/2 (filter(|U|^2) - |filter(U)|^2)`
//! * `Ck = simple(1/2 LL && MM) / (simple(|MM|^2) + VSMALL)`, clipped at zero as `(|Ck| + Ck)/2`
//! * `Ce = simple(nuEff (filter(|D|^2) - |filter(D)|^2)) / simple(KK^1.5 / (2 delta))`, clipped likewise
//! * `nut = Ck sqrt(k) delta`; the k equation has `G = 2 nut (gradU && devSymm(gradU))`,
//!   `SuSp((2/3) div U)` and `Sp(Ce sqrt(k)/delta)`.
//!
//! **Two `D` in upstream, both reproduced.** `correctNut()` (called by `validate()` and
//! at construction) passes `symm(grad U)` and an UNCLAMPED `KK`; `correct()` passes
//! `devSymm(grad U)` and `KK` clamped below at `SMALL`. [`DynamicKEqn::terms`] returns both.
//!
//! **Status: AI draft, terms only.** [`DynamicKEqn::terms`] is compared per cell against the real
//! ESI model on a frozen duct (`upstream_turb_gh1362/README.md`, slice 7). There is no `correct()`
//! here: it needs the wall-normal-corrected boundary gradient of `fvc::grad(U)` (the port's
//! `grad_vec` leaves it extrapolated, see the README), so the caller supplies the gradient with
//! its boundary faces.

use crate::les::pointwise::{
    as_calculated, calculated, extrapolate_boundary, ddot_st, ddot_t_st, dev_symm_st, dev_symm_tensor, boundary_values,
    mag_sqr_st, mag_sqr_vec, map1, map2, sqr_vec, symm_tensor, umax,
};
use crate::source_terms::{FrozenState, SourceTerms};
use outram_foam_basic_lib::fields::vol_field::VolField;
use outram_foam_basic_lib::fv_operators::les_filter::{simple_filter, FilterValue, LesFilter};
use outram_foam_basic_lib::prelude::{
    FvMesh, SymmTensor, Tensor, VolScalarField, VolVectorField,
};
use petir::mathf::RealMath;
use std::sync::Arc;

/// `SMALL` (double): the lower clamp of `KK` in `correct()`.
const SMALL: f64 = 1e-15;
/// `VSMALL` (double).
const VSMALL: f64 = 1e-300;

/// `dynamicKEqn` LES model (incompressible, single phase).
pub struct DynamicKEqn {
    pub mesh: Arc<FvMesh>,
    /// Sub-grid kinetic energy k [m^2/s^2].
    pub k: VolScalarField,
    /// Sub-grid eddy viscosity [m^2/s] with its boundary values (used in `nuEff`, `G`).
    pub nu_t: VolScalarField,
    /// Velocity [m/s]; its face values come from its boundary conditions.
    pub u: VolVectorField,
    /// Molecular kinematic viscosity [m^2/s].
    pub nu: VolScalarField,
    filter: LesFilter,
    delta: Vec<f64>,
}

/// Every field the model forms from a frozen state, with boundary values where upstream has them.
#[derive(Debug, Clone)]
pub struct DynamicTerms {
    /// `symm(grad U)` (the `D` of `correctNut()`).
    pub d_symm: VolField<SymmTensor>,
    /// `devSymm(grad U)` (the `D` of `correct()`).
    pub d_dev: VolField<SymmTensor>,
    /// `KK` unclamped (`correctNut()`).
    pub kk_nut: VolScalarField,
    /// `KK` clamped below at `SMALL` (`correct()`).
    pub kk: VolScalarField,
    /// `LL` (independent of `D`).
    pub ll: VolField<SymmTensor>,
    /// `MM` for `D = devSymm`, clamped `KK`.
    pub mm_dev: VolField<SymmTensor>,
    /// `MM` for `D = symm`, unclamped `KK`.
    pub mm_symm: VolField<SymmTensor>,
    /// `Ck(symm(grad U), KK unclamped)`.
    pub ck_symm: VolScalarField,
    /// `Ck(devSymm(grad U), KK clamped)`.
    pub ck_dev: VolScalarField,
    /// `Ce(devSymm(grad U), KK clamped)`.
    pub ce: VolScalarField,
    /// `nut` after `validate()`: `Ck_symm sqrt(k) delta`.
    pub nut_validate: Vec<f64>,
    /// `nut` after `correct()` with k unchanged: `Ck_dev sqrt(k) delta`.
    pub nut_after: Vec<f64>,
}

impl DynamicKEqn {
    /// `k` starts uniform at 1e-4, `nut` at zero, `nu` 1e-5, `delta = 1.0 cbrt(V)` (cubeRootVol).
    pub fn new(mesh: Arc<FvMesh>, filter: LesFilter) -> Self {
        let delta = (0..mesh.n_cells)
            .map(|c| 1.0 * mesh.cell_volumes[c].r_cbrt())
            .collect();
        Self {
            k: VolScalarField::uniform("k", mesh.clone(), 1e-4),
            nu_t: VolScalarField::zeros("nut", mesh.clone()),
            u: VolVectorField::zero("U", mesh.clone()),
            nu: VolScalarField::uniform("nu", mesh.clone(), 1e-5),
            mesh,
            filter,
            delta,
        }
    }

    /// The test filter.
    pub fn filter(&self) -> &LesFilter {
        &self.filter
    }

    fn delta_field(&self, over: &Option<Vec<f64>>) -> VolScalarField {
        let cells: Vec<f64> = match over {
            Some(v) => v.clone(),
            None => self.delta.clone(),
        };
        // upstream `LESdelta` builds `delta` with CALCULATED patches initialised to `small`
        // (`LESdelta.C`), and `correctBoundaryConditions` does nothing to a calculated patch:
        // its boundary value is SMALL = 1e-15, NOT the owner-cell value (found from the first
        // anisotropic comparison, README slice 7).
        let faces: Vec<Vec<f64>> = self
            .mesh
            .patches
            .iter()
            .map(|p| vec![SMALL; p.size])
            .collect();
        calculated("delta", &self.mesh, cells, faces)
    }

    /// `KK = 0.5*(filter(magSqr(U)) - magSqr(filter(U)))`, optionally `clamp_min(SMALL)`
    /// (cells and boundary faces, as `GeometricField::clamp_min`).
    fn kk(&self, u: &VolVectorField, clamp: bool) -> VolScalarField {
        let mag = map1(u, mag_sqr_vec);
        let f_mag = self.filter.apply(&mag);
        let f_u = self.filter.apply(u);
        let mag_f_u = map1(&f_u, mag_sqr_vec);
        let kk = map2(&f_mag, &mag_f_u, |a, b| 0.5 * (a - b));
        if clamp {
            map1(&kk, |x| if *x < SMALL { SMALL } else { *x })
        } else {
            kk
        }
    }

    /// Apply the `simple` filter to an expression whose value is built from a test-filter output
    /// by same-type operators. Upstream's `reuseTmp*` gives such an expression the PATCH TYPE of
    /// that output: `extrapolatedCalculated` for `simple` and `laplace` (their results come from
    /// `surfaceSum`/`surfaceIntegrate`), so the filter's `correctBoundaryConditions` replaces its
    /// stale boundary values with the owner-cell values first; `calculated` for `anisotropic`
    /// (new component-wise field), which keeps them. `nuEff*(...)` in `Ce` takes its type from
    /// the `nuEff()` tmp (calculated) and is NOT corrected.
    fn sf_entry<T: FilterValue>(&self, expr: VolField<T>) -> VolField<T> {
        match self.filter {
            LesFilter::Anisotropic(_) => simple_filter(&expr),
            _ => simple_filter(&extrapolate_boundary(&expr)),
        }
    }

    /// `LL = simple(dev(filter(sqr(U)) - sqr(filter(U))))`.
    fn ll(&self, u: &VolVectorField) -> VolField<SymmTensor> {
        let sqr_u = map1(u, sqr_vec);
        let f_sqr = self.filter.apply(&sqr_u);
        let f_u = self.filter.apply(u);
        let sqr_f_u = map1(&f_u, sqr_vec);
        let diff = map2(&f_sqr, &sqr_f_u, |a, b| *a - *b);
        self.sf_entry(map1(&diff, dev_symm_st))
    }

    /// `MM = simple(-2.0*delta*sqrt(max(KK, 0))*filter(D))`.
    fn mm(
        &self,
        delta: &VolScalarField,
        d: &VolField<SymmTensor>,
        kk: &VolScalarField,
    ) -> VolField<SymmTensor> {
        let f_d = self.filter.apply(d);
        let m = map2(delta, kk, |dl, k| (-2.0 * dl) * umax(*k, 0.0).sqrt());
        let prod = map2(&f_d, &m, |s, m| *s * *m);
        self.sf_entry(prod)
    }

    /// `Ck` from `LL` and `MM`.
    fn ck(&self, ll: &VolField<SymmTensor>, mm: &VolField<SymmTensor>) -> VolScalarField {
        let lm = map2(ll, mm, |a, b| 0.5 * ddot_st(a, b));
        let num = simple_filter(&lm);
        let den0 = simple_filter(&map1(mm, mag_sqr_st));
        let den = map1(&den0, |x| *x + VSMALL);
        let ck = map2(&num, &den, |a, b| *a / *b);
        map1(&ck, |c| 0.5 * (c.abs() + *c))
    }

    /// `Ce(D, KK)` with `nuEff = nut + nu`.
    fn ce(
        &self,
        delta: &VolScalarField,
        d: &VolField<SymmTensor>,
        kk: &VolScalarField,
    ) -> VolScalarField {
        let nu_t = as_calculated(&self.nu_t);
        let nu = as_calculated(&self.nu);
        let nu_eff = map2(&nu_t, &nu, |a, b| *a + *b);
        let f_mag = self.filter.apply(&map1(d, mag_sqr_st));
        let mag_f_d = map1(&self.filter.apply(d), mag_sqr_st);
        let inner = map2(&f_mag, &mag_f_d, |a, b| *a - *b);
        let prod = map2(&nu_eff, &inner, |a, b| *a * *b);
        let num = simple_filter(&prod);
        let pw = map2(kk, delta, |k, dl| k.r_powf(1.5) / (2.0 * dl));
        let den = simple_filter(&pw);
        let ce = map2(&num, &den, |a, b| *a / *b);
        map1(&ce, |c| 0.5 * (c.abs() + *c))
    }

    /// All the fields of `validate()` and of one `correct()` on the frozen state: `state.grad_u`
    /// (cells) with `grad_u_faces` (the boundary faces of `fvc::grad(U)` in patch order, as the
    /// caller's gradient has them), `state.cell_length` overriding the width.
    pub fn terms(&self, state: &FrozenState, grad_u_faces: &[Tensor]) -> DynamicTerms {
        let mesh = &self.mesh;
        let faces: Vec<Vec<Tensor>> = mesh
            .patches
            .iter()
            .map(|p| {
                let b0 = p.start - mesh.n_internal_faces;
                grad_u_faces[b0..b0 + p.size].to_vec()
            })
            .collect();
        let grad = calculated("gradU", mesh, state.grad_u.clone(), faces);
        let d_symm = map1(&grad, symm_tensor);
        let d_dev = map1(&grad, dev_symm_tensor);
        let u = as_calculated(&self.u);
        let delta = self.delta_field(&state.cell_length);
        let kk_nut = self.kk(&u, false);
        let kk = self.kk(&u, true);
        let ll = self.ll(&u);
        let mm_symm = self.mm(&delta, &d_symm, &kk_nut);
        let mm_dev = self.mm(&delta, &d_dev, &kk);
        let ck_symm = self.ck(&ll, &mm_symm);
        let ck_dev = self.ck(&ll, &mm_dev);
        let ce = self.ce(&delta, &d_dev, &kk);
        let k = self.k.internal.as_slice();
        let nut_of = |ck: &VolScalarField| -> Vec<f64> {
            (0..mesh.n_cells)
                .map(|c| (ck.internal[c] * k[c].sqrt()) * delta.internal[c])
                .collect()
        };
        let nut_validate = nut_of(&ck_symm);
        let nut_after = nut_of(&ck_dev);
        DynamicTerms {
            d_symm,
            d_dev,
            kk_nut,
            kk,
            ll,
            mm_dev,
            mm_symm,
            ck_symm,
            ck_dev,
            ce,
            nut_validate,
            nut_after,
        }
    }

    /// Per-cell terms of the k equation of `correct()`: `Ck`, `Ce`, `KK`, `nut` (after
    /// `validate()` and after `correct()`), `G = 2 nut (gradU && devSymm(gradU))` with the STORED
    /// `nu_t`, `k_SuSp = (2/3) div U`, `k_Sp = Ce sqrt(k)/delta`.
    pub fn source_terms(&self, state: &FrozenState, grad_u_faces: &[Tensor]) -> SourceTerms {
        let t = self.terms(state, grad_u_faces);
        let n = self.mesh.n_cells;
        let delta = self.delta_field(&state.cell_length);
        let k = self.k.internal.as_slice();
        let nut = self.nu_t.internal.as_slice();
        let mut out = SourceTerms::default();
        out.push("KK", t.kk.internal.iter().cloned().collect());
        out.push("KK_nut", t.kk_nut.internal.iter().cloned().collect());
        out.push("Ck_symm", t.ck_symm.internal.iter().cloned().collect());
        out.push("Ck_dev", t.ck_dev.internal.iter().cloned().collect());
        out.push("Ce", t.ce.internal.iter().cloned().collect());
        out.push("nut_validate", t.nut_validate.clone());
        out.push("nut_after", t.nut_after.clone());
        out.push(
            "G",
            (0..n)
                .map(|c| (2.0 * nut[c]) * ddot_t_st(&state.grad_u[c], &t.d_dev.internal[c]))
                .collect(),
        );
        out.push("k_SuSp", (0..n).map(|c| (2.0 / 3.0) * state.div_u[c]).collect());
        out.push(
            "k_Sp",
            (0..n)
                .map(|c| (t.ce.internal[c] * k[c].sqrt()) / delta.internal[c])
                .collect(),
        );
        out
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

    /// A uniform velocity has no sub-grid energy: KK is clamped to SMALL (and zero unclamped),
    /// `Ck` and `Ce` are finite and non-negative, and the clamp only acts on the corrected form.
    #[test]
    fn uniform_velocity_gives_zero_kk_and_finite_coefficients() {
        let mesh = line_mesh();
        let mut m = DynamicKEqn::new(mesh.clone(), LesFilter::Simple);
        let cells = vec![vx(2.0); 3];
        let bnd = (0..2)
            .map(|_| PatchField {
                bc: BoundaryCondition::FixedField(Field::new(vec![vx(2.0)])),
                values: Field::new(vec![vx(2.0)]),
            })
            .collect();
        m.u = VolVectorField::new("U", mesh.clone(), Field::new(cells), bnd);
        let st = FrozenState {
            grad_u: vec![Tensor::ZERO; 3],
            div_u: vec![0.0; 3],
            ..FrozenState::default()
        };
        let t = m.terms(&st, &[Tensor::ZERO, Tensor::ZERO]);
        for c in 0..3 {
            assert!(t.kk_nut.internal[c].abs() < 1e-14);
            assert_eq!(t.kk.internal[c], SMALL);
            assert!(t.ck_dev.internal[c].is_finite() && t.ck_dev.internal[c] >= 0.0);
            assert!(t.ce.internal[c].is_finite() && t.ce.internal[c] >= 0.0);
        }
        let s = m.source_terms(&st, &[Tensor::ZERO, Tensor::ZERO]);
        assert_eq!(s.get("G").unwrap(), &[0.0, 0.0, 0.0][..]);
        assert_eq!(s.get("k_SuSp").unwrap().len(), 3);
        assert_eq!(m.u_faces().len(), 2);
        assert!(matches!(m.filter(), LesFilter::Simple));
        assert_eq!(t.nut_after.len(), 3);
        assert_eq!(t.nut_validate.len(), 3);
        assert_eq!(t.mm_dev.internal.len(), 3);
        assert_eq!(t.mm_symm.internal.len(), 3);
        assert_eq!(t.ll.internal.len(), 3);
        assert_eq!(t.d_symm.internal.len(), 3);
        assert_eq!(t.d_dev.internal.len(), 3);
        assert_eq!(t.ck_symm.internal.len(), 3);
    }
}
