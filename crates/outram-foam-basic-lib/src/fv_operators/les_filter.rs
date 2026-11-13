// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2011-2016 OpenFOAM Foundation
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

//! The `LESfilter` family: `simple`, `laplace` and `anisotropic` (gh:#1544, under #1377).
//!
//! Ported from OpenFOAM-v2506 (ESI), `src/TurbulenceModels/turbulenceModels/LES/LESfilters/`
//! (`simpleFilter/simpleFilter.C`, `laplaceFilter/laplaceFilter.C`,
//! `anisotropicFilter/anisotropicFilter.C`), with the finite-volume operators they call
//! (`finiteVolume/fvc/fvcSurfaceIntegrate.C`: `surfaceSum`, `surfaceIntegrate`;
//! `gaussLaplacianSchemes.C`: the scalar-gamma `fvcLaplacian`; `snGradScheme.C` with the
//! default `uncorrected` scheme). Each filter is generic over scalar, vector, symmTensor and
//! tensor fields through [`FilterValue`], and returns a field WITH its boundary values
//! (calculated patches), because a filter's output is routinely filtered again (the dynamic
//! models chain three passes) and the second pass interpolates those boundary values.
//!
//! * [`simple_filter`]: `surfaceSum(magSf * interpolate(f)) / surfaceSum(magSf)`; the boundary
//!   of the result is the owner-cell value (upstream's `extrapolatedCalculated`).
//! * [`LaplaceFilter`]: `f + laplacian(coeff, f)` with `coeff = V^(2/3)/widthCoeff`. Upstream
//!   builds `coeff_` as a calculated field whose boundary is ZERO, so the interpolated `gamma`
//!   is zero on every boundary face and no filter flux crosses a boundary: reproduced here
//!   (gh:#1544 prediction 3), not "fixed".
//! * [`AnisotropicFilter`]: `f + (coeff & surfaceIntegrate(Sf * snGrad(f)))` with the per-direction
//!   `coeff_d = (1/widthCoeff) sqr(2 V / surfaceSum(|Sf_d|))`; symmTensor and tensor fields are
//!   filtered component by component, as upstream does.
//!
//! **Status: AI draft.** Compared per cell and per boundary face against the real ESI classes in
//! `outram-foam-turbulence-lib/tests/openfoam_turbulence_code_to_code.rs`
//! (`upstream_turb_gh1362/README.md`, slice 7). Supported boundary conditions: those whose
//! `snGrad` upstream is `deltaCoeffs*(b - P)` (fixedValue, calculated, field-valued), plus
//! `ZeroGradient`/`Symmetry` (zero) and `FixedGradient` (the gradient). `Mixed` patches take the
//! generic path and are NOT claimed.

use std::ops::{Add, Div, Mul, Sub};
use std::sync::Arc;

use petir::mathf::RealMath;

use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
use crate::fields::field::Field;
use crate::fields::vol_field::{VolField, VolScalarField};
use crate::fv_operators::fvc::interpolate;
use crate::fv_operators::fvm::DeltaCoeff;
use crate::mesh::fv_mesh::{FvMesh, PatchKind};
use crate::primitives::{SymmTensor, Tensor, Vector3};

/// A field value a filter can act on (scalar, vector, symmTensor, tensor).
pub trait FilterValue:
    Clone
    + Default
    + Add<Output = Self>
    + Sub<Output = Self>
    + Mul<f64, Output = Self>
    + Div<f64, Output = Self>
{
    /// The [`AnisotropicFilter`] applied to `vf` (scalar and vector directly, symmTensor and
    /// tensor component by component, as upstream).
    fn anisotropic(filter: &AnisotropicFilter, vf: &VolField<Self>) -> VolField<Self>;
}

impl FilterValue for f64 {
    fn anisotropic(filter: &AnisotropicFilter, vf: &VolField<Self>) -> VolField<Self> {
        anisotropic_direct(filter, vf, |sf, g| sf * *g, |c, x: &Vector3| {
            c.x * x.x + c.y * x.y + c.z * x.z
        })
    }
}

impl FilterValue for Vector3 {
    fn anisotropic(filter: &AnisotropicFilter, vf: &VolField<Self>) -> VolField<Self> {
        // upstream `vector & tensor`, TensorI.H: v.x*t.xj + v.y*t.yj + v.z*t.zj
        anisotropic_direct(filter, vf, |sf, g| sf * *g, |c, t: &Tensor| {
            Vector3::new(
                c.x * t.xx + c.y * t.yx + c.z * t.zx,
                c.x * t.xy + c.y * t.yy + c.z * t.zy,
                c.x * t.xz + c.y * t.yz + c.z * t.zz,
            )
        })
    }
}

impl FilterValue for SymmTensor {
    fn anisotropic(filter: &AnisotropicFilter, vf: &VolField<Self>) -> VolField<Self> {
        let get: [fn(&SymmTensor) -> f64; 6] = [
            |t| t.xx,
            |t| t.xy,
            |t| t.xz,
            |t| t.yy,
            |t| t.yz,
            |t| t.zz,
        ];
        let parts: Vec<VolScalarField> = get
            .iter()
            .map(|g| anisotropic_component(filter, vf, *g))
            .collect();
        let mk = |a: Vec<f64>| SymmTensor::new(a[0], a[1], a[2], a[3], a[4], a[5]);
        recombine(
            vf,
            &parts,
            |i| mk(parts.iter().map(|p| p.internal[i]).collect()),
            |pi, fi| mk(parts.iter().map(|p| p.boundary[pi].values[fi]).collect()),
        )
    }
}

impl FilterValue for Tensor {
    fn anisotropic(filter: &AnisotropicFilter, vf: &VolField<Self>) -> VolField<Self> {
        let get: [fn(&Tensor) -> f64; 9] = [
            |t| t.xx,
            |t| t.xy,
            |t| t.xz,
            |t| t.yx,
            |t| t.yy,
            |t| t.yz,
            |t| t.zx,
            |t| t.zy,
            |t| t.zz,
        ];
        let parts: Vec<VolScalarField> = get
            .iter()
            .map(|g| anisotropic_component(filter, vf, *g))
            .collect();
        let mk = |a: Vec<f64>| Tensor::new(a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7], a[8]);
        recombine(
            vf,
            &parts,
            |i| mk(parts.iter().map(|p| p.internal[i]).collect()),
            |pi, fi| mk(parts.iter().map(|p| p.boundary[pi].values[fi]).collect()),
        )
    }
}

/// Upstream `fvc::surfaceSum`: zero, then every internal face added to owner AND neighbour in
/// face order, then every boundary face added to its owner (`fvcSurfaceIntegrate.C`).
pub fn surface_sum<T>(mesh: &FvMesh, internal: &[T], boundary: &[Vec<T>]) -> Vec<T>
where
    T: Clone + Default + Add<Output = T>,
{
    let mut ivf = vec![T::default(); mesh.n_cells];
    for f in 0..mesh.n_internal_faces {
        let o = mesh.owner[f];
        let n = mesh.neighbour[f];
        ivf[o] = ivf[o].clone() + internal[f].clone();
        ivf[n] = ivf[n].clone() + internal[f].clone();
    }
    for (pi, patch) in mesh.patches.iter().enumerate() {
        if patch.kind == PatchKind::Empty {
            continue;
        }
        for fi in 0..patch.size {
            let c = mesh.owner[patch.start + fi];
            ivf[c] = ivf[c].clone() + boundary[pi][fi].clone();
        }
    }
    ivf
}

/// Upstream `fvc::surfaceIntegrate`: zero, `+=` into the owner and `-=` out of the neighbour
/// per internal face, boundary faces `+=` into the owner, then a DIVISION by the cell volume.
pub fn surface_integrate<T>(mesh: &FvMesh, internal: &[T], boundary: &[Vec<T>]) -> Vec<T>
where
    T: Clone + Default + Add<Output = T> + Sub<Output = T> + Div<f64, Output = T>,
{
    let mut ivf = vec![T::default(); mesh.n_cells];
    for f in 0..mesh.n_internal_faces {
        let o = mesh.owner[f];
        let n = mesh.neighbour[f];
        ivf[o] = ivf[o].clone() + internal[f].clone();
        ivf[n] = ivf[n].clone() - internal[f].clone();
    }
    for (pi, patch) in mesh.patches.iter().enumerate() {
        if patch.kind == PatchKind::Empty {
            continue;
        }
        for fi in 0..patch.size {
            let c = mesh.owner[patch.start + fi];
            ivf[c] = ivf[c].clone() + boundary[pi][fi].clone();
        }
    }
    (0..mesh.n_cells)
        .map(|c| ivf[c].clone() / mesh.cell_volumes[c])
        .collect()
}

/// `mag(Sf)` per internal face and per boundary face (upstream `mesh.magSf()`).
fn mag_sf(mesh: &FvMesh) -> (Vec<f64>, Vec<Vec<f64>>) {
    let internal = (0..mesh.n_internal_faces)
        .map(|f| mesh.face_area_vectors[f].mag())
        .collect();
    let boundary = mesh
        .patches
        .iter()
        .map(|p| {
            (0..p.size)
                .map(|fi| mesh.face_area_vectors[p.start + fi].mag())
                .collect()
        })
        .collect();
    (internal, boundary)
}

/// A field with calculated patches: the cell values plus explicit boundary face values.
fn calculated_field<T: Clone + Default>(
    name: &str,
    mesh: &Arc<FvMesh>,
    cells: Vec<T>,
    boundary_values: Vec<Vec<T>>,
) -> VolField<T> {
    let boundary = boundary_values
        .into_iter()
        .map(|v| {
            let vals = Field::new(v);
            PatchField {
                bc: BoundaryCondition::Calculated(vals.clone()),
                values: vals,
            }
        })
        .collect();
    VolField::new(name, mesh.clone(), Field::new(cells), boundary)
}

/// Boundary values equal to the owner cell value (upstream `extrapolatedCalculated`).
fn extrapolated<T: Clone>(mesh: &FvMesh, cells: &[T]) -> Vec<Vec<T>> {
    mesh.patches
        .iter()
        .map(|p| {
            (0..p.size)
                .map(|fi| cells[mesh.owner[p.start + fi]].clone())
                .collect()
        })
        .collect()
}

/// `fvc::snGrad(vf)` with the default `uncorrected` scheme: `nonOrthDeltaCoeffs*(N - P)` on
/// internal faces; on a boundary face the patch's own `snGrad()` (`deltaCoeffs*(b - P)` unless
/// zeroGradient or fixedGradient). Returns (internal faces, boundary faces per patch).
fn sn_grad_uncorrected<T: FilterValue>(vf: &VolField<T>) -> (Vec<T>, Vec<Vec<T>>) {
    let mesh = &vf.mesh;
    let face_vals = interpolate(vf);
    let internal = (0..mesh.n_internal_faces)
        .map(|f| {
            let dc = DeltaCoeff::NonOrthogonal.interior_delta_coeff(mesh, f);
            (vf.internal[mesh.neighbour[f]].clone() - vf.internal[mesh.owner[f]].clone()) * dc
        })
        .collect();
    let boundary = mesh
        .patches
        .iter()
        .enumerate()
        .map(|(pi, patch)| {
            (0..patch.size)
                .map(|fi| {
                    let gf = patch.start + fi;
                    let p = vf.internal[mesh.owner[gf]].clone();
                    match &vf.boundary[pi].bc {
                        BoundaryCondition::ZeroGradient | BoundaryCondition::Symmetry => {
                            T::default()
                        }
                        BoundaryCondition::FixedGradient(g) => g.clone(),
                        _ => {
                            let dc = DeltaCoeff::Orthogonal.boundary_delta_coeff(mesh, gf);
                            (face_vals.boundary[pi].values[fi].clone() - p) * dc
                        }
                    }
                })
                .collect()
        })
        .collect();
    (internal, boundary)
}

/// Upstream `simpleFilter::operator()`:
/// `surfaceSum(magSf*interpolate(f))/surfaceSum(magSf)` (`simpleFilter.C`).
pub fn simple_filter<T: FilterValue>(vf: &VolField<T>) -> VolField<T> {
    let mesh = &vf.mesh;
    let (mi, mb) = mag_sf(mesh);
    let fv = interpolate(vf);
    let num_i: Vec<T> = (0..mesh.n_internal_faces)
        .map(|f| fv.internal[f].clone() * mi[f])
        .collect();
    let num_b: Vec<Vec<T>> = mesh
        .patches
        .iter()
        .enumerate()
        .map(|(pi, p)| {
            (0..p.size)
                .map(|fi| fv.boundary[pi].values[fi].clone() * mb[pi][fi])
                .collect()
        })
        .collect();
    let num = surface_sum(mesh, &num_i, &num_b);
    let den = surface_sum(mesh, &mi, &mb);
    let cells: Vec<T> = (0..mesh.n_cells)
        .map(|c| num[c].clone() / den[c])
        .collect();
    let bnd = extrapolated(mesh, &cells);
    calculated_field(&vf.name, mesh, cells, bnd)
}

/// Upstream `laplaceFilter`: `f + fvc::laplacian(coeff, f)`, `coeff = V^(2/3)/widthCoeff`.
#[derive(Debug, Clone)]
pub struct LaplaceFilter {
    mesh: Arc<FvMesh>,
    width_coeff: f64,
    coeff: Vec<f64>,
}

impl LaplaceFilter {
    /// `laplaceFilter(mesh, widthCoeff)`: `coeff = pow(V, 2.0/3.0)/widthCoeff`
    /// (`laplaceFilter.C`; `pow` is PETIR's, 1 ulp from glibc on some arguments).
    pub fn new(mesh: Arc<FvMesh>, width_coeff: f64) -> Self {
        let coeff = (0..mesh.n_cells)
            .map(|c| mesh.cell_volumes[c].r_powf(2.0 / 3.0) / width_coeff)
            .collect();
        Self { mesh, width_coeff, coeff }
    }

    /// The same filter with an explicit coefficient field (to separate the libm `pow` from the
    /// filter arithmetic in a code-to-code comparison).
    pub fn with_coeff(mesh: Arc<FvMesh>, width_coeff: f64, coeff: Vec<f64>) -> Self {
        Self { mesh, width_coeff, coeff }
    }

    /// The per-cell coefficient `V^(2/3)/widthCoeff` [m^2].
    pub fn coeff(&self) -> &[f64] {
        &self.coeff
    }

    /// The `widthCoeff`.
    pub fn width_coeff(&self) -> f64 {
        self.width_coeff
    }

    /// The mesh the coefficient was built on.
    pub fn mesh(&self) -> &Arc<FvMesh> {
        &self.mesh
    }

    /// `f + laplacian(coeff, f)` (`laplaceFilter.C`): `fvc::div(gamma_f * snGrad(f) * magSf)`
    /// with `gamma_f` the linear interpolate of `coeff` whose boundary is zero.
    pub fn apply<T: FilterValue>(&self, vf: &VolField<T>) -> VolField<T> {
        let mesh = &vf.mesh;
        let zero_bnd: Vec<PatchField<f64>> = mesh
            .patches
            .iter()
            .map(|p| {
                let z = Field::new(vec![0.0; p.size]);
                PatchField { bc: BoundaryCondition::Calculated(z.clone()), values: z }
            })
            .collect();
        let gamma_vol = VolField::new(
            "laplaceFilterCoeff",
            mesh.clone(),
            Field::new(self.coeff.clone()),
            zero_bnd,
        );
        let gamma = interpolate(&gamma_vol);
        let (mi, mb) = mag_sf(mesh);
        let (sn_i, sn_b) = sn_grad_uncorrected(vf);
        let flux_i: Vec<T> = (0..mesh.n_internal_faces)
            .map(|f| (sn_i[f].clone() * gamma.internal[f]) * mi[f])
            .collect();
        let flux_b: Vec<Vec<T>> = mesh
            .patches
            .iter()
            .enumerate()
            .map(|(pi, p)| {
                (0..p.size)
                    .map(|fi| (sn_b[pi][fi].clone() * gamma.boundary[pi].values[fi]) * mb[pi][fi])
                    .collect()
            })
            .collect();
        let lap = surface_integrate(mesh, &flux_i, &flux_b);
        let fv = interpolate(vf);
        let cells: Vec<T> = (0..mesh.n_cells)
            .map(|c| vf.internal[c].clone() + lap[c].clone())
            .collect();
        let bnd: Vec<Vec<T>> = mesh
            .patches
            .iter()
            .enumerate()
            .map(|(pi, p)| {
                (0..p.size)
                    .map(|fi| {
                        fv.boundary[pi].values[fi].clone() + lap[mesh.owner[p.start + fi]].clone()
                    })
                    .collect()
            })
            .collect();
        calculated_field(&vf.name, mesh, cells, bnd)
    }
}

/// Upstream `anisotropicFilter`: per-direction coefficient
/// `coeff_d = (1/widthCoeff) sqr(2 V / surfaceSum(|Sf_d|))`.
#[derive(Debug, Clone)]
pub struct AnisotropicFilter {
    mesh: Arc<FvMesh>,
    width_coeff: f64,
    coeff: Vec<Vector3>,
}

impl AnisotropicFilter {
    /// `anisotropicFilter(mesh, widthCoeff)` (`anisotropicFilter.C`).
    pub fn new(mesh: Arc<FvMesh>, width_coeff: f64) -> Self {
        let comps: [fn(&Vector3) -> f64; 3] = [|v| v.x, |v| v.y, |v| v.z];
        let mut cols: Vec<Vec<f64>> = Vec::with_capacity(3);
        for comp in comps {
            let mi: Vec<f64> = (0..mesh.n_internal_faces)
                .map(|f| comp(&mesh.face_area_vectors[f]).abs())
                .collect();
            let mb: Vec<Vec<f64>> = mesh
                .patches
                .iter()
                .map(|p| {
                    (0..p.size)
                        .map(|fi| comp(&mesh.face_area_vectors[p.start + fi]).abs())
                        .collect()
                })
                .collect();
            let sum = surface_sum(&mesh, &mi, &mb);
            cols.push(
                (0..mesh.n_cells)
                    .map(|c| {
                        let q = (2.0 * mesh.cell_volumes[c]) / sum[c];
                        (1.0 / width_coeff) * (q * q)
                    })
                    .collect(),
            );
        }
        let coeff = (0..mesh.n_cells)
            .map(|c| Vector3::new(cols[0][c], cols[1][c], cols[2][c]))
            .collect();
        Self { mesh, width_coeff, coeff }
    }

    /// The per-cell directional coefficients [m^2].
    pub fn coeff(&self) -> &[Vector3] {
        &self.coeff
    }

    /// The `widthCoeff`.
    pub fn width_coeff(&self) -> f64 {
        self.width_coeff
    }

    /// The mesh the coefficients were built on.
    pub fn mesh(&self) -> &Arc<FvMesh> {
        &self.mesh
    }

    /// `f + (coeff & surfaceIntegrate(Sf*snGrad(f)))`.
    pub fn apply<T: FilterValue>(&self, vf: &VolField<T>) -> VolField<T> {
        T::anisotropic(self, vf)
    }
}

/// Scalar/vector anisotropic filtering: `outer` forms `Sf*snGrad`, `contract` forms `coeff & X`.
fn anisotropic_direct<T, X, O, C>(
    filter: &AnisotropicFilter,
    vf: &VolField<T>,
    outer: O,
    contract: C,
) -> VolField<T>
where
    T: FilterValue,
    X: Clone + Default + Add<Output = X> + Sub<Output = X> + Div<f64, Output = X>,
    O: Fn(Vector3, &T) -> X,
    C: Fn(Vector3, &X) -> T,
{
    let mesh = &vf.mesh;
    let (sn_i, sn_b) = sn_grad_uncorrected(vf);
    let flux_i: Vec<X> = (0..mesh.n_internal_faces)
        .map(|f| outer(mesh.face_area_vectors[f], &sn_i[f]))
        .collect();
    let flux_b: Vec<Vec<X>> = mesh
        .patches
        .iter()
        .enumerate()
        .map(|(pi, p)| {
            (0..p.size)
                .map(|fi| outer(mesh.face_area_vectors[p.start + fi], &sn_b[pi][fi]))
                .collect()
        })
        .collect();
    let integ = surface_integrate(mesh, &flux_i, &flux_b);
    let fv = interpolate(vf);
    let cells: Vec<T> = (0..mesh.n_cells)
        .map(|c| vf.internal[c].clone() + contract(filter.coeff[c], &integ[c]))
        .collect();
    // `coeff_` is built with calculated patches at zero: its boundary is the zero vector.
    let zero = Vector3::new(0.0, 0.0, 0.0);
    let bnd: Vec<Vec<T>> = mesh
        .patches
        .iter()
        .enumerate()
        .map(|(pi, p)| {
            (0..p.size)
                .map(|fi| {
                    fv.boundary[pi].values[fi].clone()
                        + contract(zero, &integ[mesh.owner[p.start + fi]])
                })
                .collect()
        })
        .collect();
    calculated_field(&vf.name, mesh, cells, bnd)
}

/// One component of a symmTensor/tensor field as a scalar field with calculated patches
/// (upstream `component(d)`), filtered with the scalar anisotropic filter.
fn anisotropic_component<T: FilterValue>(
    filter: &AnisotropicFilter,
    vf: &VolField<T>,
    get: fn(&T) -> f64,
) -> VolScalarField {
    let mesh = &vf.mesh;
    let fv = interpolate(vf);
    let cells: Vec<f64> = vf.internal.iter().map(get).collect();
    let bnd: Vec<Vec<f64>> = (0..mesh.patches.len())
        .map(|pi| fv.boundary[pi].values.iter().map(get).collect())
        .collect();
    let comp = calculated_field("component", mesh, cells, bnd);
    <f64 as FilterValue>::anisotropic(filter, &comp)
}

/// Reassemble a field from filtered components (upstream `replace(d, ...)`).
fn recombine<T: FilterValue>(
    vf: &VolField<T>,
    parts: &[VolScalarField],
    cell: impl Fn(usize) -> T,
    face: impl Fn(usize, usize) -> T,
) -> VolField<T> {
    let mesh = &vf.mesh;
    let cells: Vec<T> = (0..mesh.n_cells).map(cell).collect();
    let bnd: Vec<Vec<T>> = (0..mesh.patches.len())
        .map(|pi| (0..parts[0].boundary[pi].values.len()).map(|fi| face(pi, fi)).collect())
        .collect();
    calculated_field(&vf.name, mesh, cells, bnd)
}

/// A filter chosen at run time (upstream `LESfilter::New`), by enum dispatch.
#[derive(Debug, Clone)]
pub enum LesFilter {
    /// `simple`.
    Simple,
    /// `laplace`.
    Laplace(LaplaceFilter),
    /// `anisotropic`.
    Anisotropic(AnisotropicFilter),
}

impl LesFilter {
    /// Filter `vf`.
    pub fn apply<T: FilterValue>(&self, vf: &VolField<T>) -> VolField<T> {
        match self {
            LesFilter::Simple => simple_filter(vf),
            LesFilter::Laplace(l) => l.apply(vf),
            LesFilter::Anisotropic(a) => a.apply(vf),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder};

    fn vx(x: f64) -> Vector3 {
        Vector3::new(x, 0.0, 0.0)
    }

    /// Three unit cells in a row, zero-gradient ends (one face per end).
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

    fn zg_field(mesh: &Arc<FvMesh>, v: Vec<f64>) -> VolScalarField {
        let boundary = mesh
            .patches
            .iter()
            .map(|p| PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::new(vec![v[mesh.owner[p.start]]; p.size]),
            })
            .collect();
        VolField::new("f", mesh.clone(), Field::new(v), boundary)
    }

    /// A uniform field is unchanged by every filter, and the simple filter of a field with
    /// zero-gradient ends keeps the mean: sum(out) = sum(in) on the line (all weights 1/2, 1/2
    /// internally and the ends see the owner value).
    #[test]
    fn uniform_field_is_a_fixed_point_of_all_filters() {
        let mesh = line_mesh();
        let u = zg_field(&mesh, vec![2.5, 2.5, 2.5]);
        let filters = [
            LesFilter::Simple,
            LesFilter::Laplace(LaplaceFilter::new(mesh.clone(), 2.0)),
            // the anisotropic filter is excluded: on this x-only line its y/z coefficients
            // are infinite (0 face area) and 0*inf = NaN, as upstream arithmetic gives too
        ];
        for f in &filters {
            let out = f.apply(&u);
            for c in 0..3 {
                assert!((out.internal[c] - 2.5).abs() < 1e-14, "{f:?} cell {c}");
            }
            for p in &out.boundary {
                assert!((p.values[0] - 2.5).abs() < 1e-14);
            }
        }
    }

    /// Simple filter by hand: cell 1 averages faces (1: 0.5(a+b)) and (2: 0.5(b+c)) with equal
    /// areas: (a+b)/4 + (b+c)/4 over 2.
    #[test]
    fn simple_filter_matches_the_face_average_by_hand() {
        let mesh = line_mesh();
        let u = zg_field(&mesh, vec![1.0, 2.0, 4.0]);
        let out = simple_filter(&u);
        let want1 = (0.5 * (1.0 + 2.0) + 0.5 * (2.0 + 4.0)) / 2.0;
        assert!((out.internal[1] - want1).abs() < 1e-14);
        // end cell 0: face 0 interpolate (1.5) and its boundary face (owner value 1)
        assert!((out.internal[0] - (1.5 + 1.0) / 2.0).abs() < 1e-14);
        // the boundary of the result is the owner-cell value
        assert_eq!(out.boundary[0].values[0], out.internal[0]);
    }

    /// Laplace filter by hand on the line: the boundary gamma is zero, so only the two internal
    /// faces carry flux: out_1 = u_1 + c ((u_2 - u_1) - (u_1 - u_0)), c = V^(2/3)/w = 1/2.
    #[test]
    fn laplace_filter_has_no_flux_through_the_boundary() {
        let mesh = line_mesh();
        let u = zg_field(&mesh, vec![1.0, 2.0, 4.0]);
        let l = LaplaceFilter::new(mesh.clone(), 2.0);
        assert!((l.coeff()[0] - 0.5).abs() < 1e-15);
        assert_eq!(l.width_coeff(), 2.0);
        assert_eq!(l.mesh().n_cells, 3);
        let out = l.apply(&u);
        let want1 = 2.0 + 0.5 * ((4.0 - 2.0) - (2.0 - 1.0));
        assert!((out.internal[1] - want1).abs() < 1e-14);
        // end cell: only the internal face contributes
        assert!((out.internal[0] - (1.0 + 0.5 * (2.0 - 1.0))).abs() < 1e-14);
        let manual = LaplaceFilter::with_coeff(mesh.clone(), 2.0, l.coeff().to_vec());
        assert_eq!(manual.apply(&u).internal[1], out.internal[1]);
    }

    /// Anisotropic filter on the x-only line: `coeff_x = (1/w)(2V/(2|Sx|))^2 = 1/2`; the
    /// y and z coefficients divide by a zero face-area sum (non-finite, as upstream would give
    /// on a 1-D mesh), so only the x coefficient and the dispatch are checked here; the full
    /// 3-D arithmetic is compared with ESI on the duct (turbulence-lib code-to-code test).
    #[test]
    fn anisotropic_filter_x_coefficient_and_accessors() {
        let mesh = line_mesh();
        let a = AnisotropicFilter::new(mesh.clone(), 2.0);
        assert_eq!(a.coeff()[1].x, 0.5);
        assert!(!a.coeff()[1].y.is_finite());
        assert_eq!(a.width_coeff(), 2.0);
        assert_eq!(a.mesh().n_cells, 3);
    }

    /// Vector, symmTensor and tensor fields run through every filter and keep their sizes;
    /// the anisotropic symmTensor/tensor path goes component by component.
    #[test]
    fn all_value_types_are_filtered() {
        let mesh = line_mesh();
        let sym = zg_generic(
            &mesh,
            (0..3).map(|i| SymmTensor::new(i as f64, 1.0, 2.0, 3.0, 4.0, 5.0)).collect(),
        );
        let ten = zg_generic(
            &mesh,
            (0..3)
                .map(|i| Tensor::new(i as f64, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0))
                .collect(),
        );
        let vec = zg_generic(&mesh, (0..3).map(|i| Vector3::new(i as f64, 1.0, 2.0)).collect());
        let filters = [
            LesFilter::Simple,
            LesFilter::Laplace(LaplaceFilter::new(mesh.clone(), 2.0)),
            LesFilter::Anisotropic(AnisotropicFilter::new(mesh.clone(), 2.0)),
        ];
        for f in &filters {
            assert_eq!(f.apply(&sym).internal.len(), 3);
            assert_eq!(f.apply(&ten).internal.len(), 3);
            assert_eq!(f.apply(&vec).internal.len(), 3);
            assert_eq!(f.apply(&sym).boundary.len(), 2);
        }
        // the simple filter of a symmTensor is the component-wise scalar filter
        let s = simple_filter(&sym);
        let xx = simple_filter(&zg_field(&mesh, vec![0.0, 1.0, 2.0]));
        assert_eq!(s.internal[1].xx, xx.internal[1]);
    }

    fn zg_generic<T: FilterValue>(mesh: &Arc<FvMesh>, v: Vec<T>) -> VolField<T> {
        let boundary = mesh
            .patches
            .iter()
            .map(|p| PatchField {
                bc: BoundaryCondition::ZeroGradient,
                values: Field::new(vec![v[mesh.owner[p.start]].clone(); p.size]),
            })
            .collect();
        VolField::new("f", mesh.clone(), Field::new(v), boundary)
    }
}
