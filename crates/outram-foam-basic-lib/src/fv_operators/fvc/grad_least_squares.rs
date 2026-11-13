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

//! Least-squares cell gradient — `Foam::leastSquaresGrad`.
//!
//! Translated from `src/finiteVolume/finiteVolume/gradSchemes/leastSquaresGrad/`.
//!
//! # What this computes
//!
//! For each cell `P`, the gradient `g = grad(phi)|_P` that best fits the
//! first-order Taylor expansion to every neighbour `N` in the least-squares
//! sense: `g` minimises
//!
//! ```text
//!     sum_N  w_N^2 ( phi_N - phi_P - g . d_N )^2 ,      d_N = C_N - C_P
//! ```
//!
//! ~~with the inverse-distance weighting `w_N = 1 / |d_N|` that OpenFOAM uses~~
//! **CORRECTED 2026-11-11 (gh:#1362)**: upstream's `leastSquaresVectors` weights
//! each face by `magSf/|d|^2` (split `(1-w)` / `w` between its two cells), not
//! `1/|d|^2`; the formula below is the unweighted-by-area sketch, the exact
//! arithmetic is on [`grad_least_squares`]. The normal equations are the 3x3
//! symmetric system
//!
//! ```text
//!     M g = r ,   M = sum_N w_N^2 d_N (x) d_N ,   r = sum_N w_N^2 (phi_N - phi_P) d_N
//! ```
//!
//! # Why it exists alongside [`grad`](super::grad)
//!
//! [`grad`](super::grad) is the Gauss (divergence-theorem) gradient. It is exact
//! for a linear field only when the face interpolation weights land on the true
//! face centres — i.e. on an orthogonal, non-skewed mesh. The least-squares
//! gradient is **exact for any linear field on any mesh**, orthogonal or not, as
//! long as each cell has at least three non-coplanar neighbour directions.
//! That property is what makes it the right gradient to feed the explicit
//! non-orthogonal correction in
//! [`fvm::laplacian_corrected`](crate::fv_operators::fvm::laplacian_corrected):
//! a correction built from an inconsistent gradient just moves the error around.
//!
//! # Units
//!
//! `phi` carries some scalar physical quantity `[X]` (temperature `[K]`,
//! pressure `[Pa]`, ...); the returned vector field is its spatial gradient
//! `[X/m]`, since all mesh geometry is in metres. `uom` is not applied here
//! because the field types are `f64`-valued throughout Layers 2-3; the unit is
//! whatever the caller put into `phi`, divided by metres.
//!
//! # Rank deficiency (2-D and 1-D meshes)
//!
//! On a 2-D mesh — such as every mesh in this crate's tests that has no
//! `z`-normal faces — the neighbour directions span only a plane and `M` is
//! singular. Rather than failing, this implementation detects each Cartesian
//! direction that the neighbour set does not resolve (its diagonal entry of `M`
//! is negligible against the trace) and **pins the gradient component in that
//! direction to zero**, which is the physically correct answer for a mesh with
//! no extent in it. See [`grad_least_squares`] for the exact test used.

use crate::fields::boundary::bc::PatchField;
use crate::fields::field::Field;
use crate::fields::vol_field::{VolScalarField, VolVectorField};
use super::linear_weight;
use crate::matrix::SquareMatrix;
use crate::fv_operators::fvm::patch_delta;
use crate::primitives::Vector3;

/// A direction is treated as unresolved when its diagonal entry of the
/// least-squares normal matrix falls below this fraction of the matrix trace.
///
/// `1e-10` is far below any genuine anisotropy of a usable mesh (a cell 10^4
/// times longer in one direction still gives ~1e-8) but far above the rounding
/// noise of an exactly 2-D mesh, where the entry is identically zero.
const RANK_TOL: f64 = 1.0e-10;

/// The face value to fit against on boundary patch `pi`, local face `fi`, whose
/// owner cell is `owner`.
///
/// Derived from the **boundary-condition kind**, not blindly from
/// `PatchField::values`: for `ZeroGradient` / `Symmetry` / `Empty` the stored
/// `values` are zero until an operator writes them (see [`PatchField`]'s own
/// documentation), and reading them raw would inject a spurious gradient of
/// `-phi_P/|d|` at every wall of an otherwise uniform field. Those patches
/// instead return the owner cell's value, which is what a zero normal gradient
/// means. Dirichlet patches return their prescribed value; anything else falls
/// back to the stored face value, which is correct once the owning operator has
/// updated it.
fn boundary_face_value(phi: &VolScalarField, pi: usize, fi: usize, owner: usize) -> f64 {
    use crate::fields::boundary::bc::BoundaryCondition as Bc;
    match &phi.boundary[pi].bc {
        Bc::ZeroGradient | Bc::Symmetry | Bc::Empty => phi.internal[owner],
        Bc::FixedValue(v) => *v,
        Bc::FixedField(ff) => ff[fi],
        Bc::NoSlip => 0.0,
        _ => phi.boundary[pi].values[fi],
    }
}

/// A symmetric tensor `[xx, xy, xz, yy, yz, zz]` (upstream `symmTensor`).
type Sym = [f64; 6];

/// `sqr(d)` of a vector, upstream `symmTensor`.
fn sym_sqr(d: Vector3) -> Sym {
    [d.x * d.x, d.x * d.y, d.x * d.z, d.y * d.y, d.y * d.z, d.z * d.z]
}

/// `s*t` of a scalar and a symmetric tensor.
fn sym_scale(s: f64, t: &Sym) -> Sym {
    [s * t[0], s * t[1], s * t[2], s * t[3], s * t[4], s * t[5]]
}

/// `a += b`.
fn sym_add_assign(a: &mut Sym, b: &Sym) {
    for i in 0..6 {
        a[i] += b[i];
    }
}

/// Upstream `SymmTensor::det()`, term order preserved.
fn sym_det(a: &Sym) -> f64 {
    let (xx, xy, xz, yy, yz, zz) = (a[0], a[1], a[2], a[3], a[4], a[5]);
    xx * yy * zz + xy * yz * xz + xz * xy * yz - xx * yz * yz - xy * xy * zz - xz * yy * xz
}

/// Upstream `SymmTensor::adjunct()` divided by `detval` component-wise.
fn sym_adjunct_over(a: &Sym, detval: f64) -> Sym {
    let (xx, xy, xz, yy, yz, zz) = (a[0], a[1], a[2], a[3], a[4], a[5]);
    [
        (yy * zz - yz * yz) / detval,
        (xz * yz - xy * zz) / detval,
        (xy * yz - xz * yy) / detval,
        (xx * zz - xz * xz) / detval,
        (xy * xz - xx * yz) / detval,
        (xx * yy - xy * xy) / detval,
    ]
}

/// Upstream `SymmTensor::safeInv()` (ESI v2506 `SymmTensorI.H`): detects an
/// unresolved (2-D / 1-D) direction by `magSqr(diag) < SMALL * sum`, adds 1 to
/// that diagonal, inverts, and subtracts 1 again; a determinant below
/// `ROOTVSMALL` returns the zero tensor.
fn sym_safe_inv(a: &Sym) -> Sym {
    const SMALL: f64 = 1.0e-15;
    const ROOTVSMALL: f64 = 1.0e-150;
    let m_xx = a[0] * a[0];
    let m_yy = a[3] * a[3];
    let m_zz = a[5] * a[5];
    let threshold = SMALL * (m_xx + m_yy + m_zz);
    let small_xx = m_xx < threshold;
    let small_yy = m_yy < threshold;
    let small_zz = m_zz < threshold;
    if small_xx || small_yy || small_zz {
        let mut work = *a;
        if small_xx {
            work[0] += 1.0;
        }
        if small_yy {
            work[3] += 1.0;
        }
        if small_zz {
            work[5] += 1.0;
        }
        let detval = sym_det(&work);
        if detval.abs() < ROOTVSMALL {
            return [0.0; 6];
        }
        let mut inv = sym_adjunct_over(&work, detval);
        if small_xx {
            inv[0] -= 1.0;
        }
        if small_yy {
            inv[3] -= 1.0;
        }
        if small_zz {
            inv[5] -= 1.0;
        }
        return inv;
    }
    let detval = sym_det(a);
    if detval.abs() < ROOTVSMALL {
        return [0.0; 6];
    }
    sym_adjunct_over(a, detval)
}

/// `st & v` (symmetric tensor dotted into a vector), upstream term order.
fn sym_dot(st: &Sym, v: Vector3) -> Vector3 {
    Vector3::new(
        st[0] * v.x + st[1] * v.y + st[2] * v.z,
        st[1] * v.x + st[3] * v.y + st[4] * v.z,
        st[2] * v.x + st[4] * v.y + st[5] * v.z,
    )
}

/// `|d|^2` as upstream `magSqr(vector)`.
fn mag_sqr(d: Vector3) -> f64 {
    d.x * d.x + d.y * d.y + d.z * d.z
}

/// `s*v` of a scalar and a vector, component-wise.
fn vscale(s: f64, v: Vector3) -> Vector3 {
    Vector3::new(s * v.x, s * v.y, s * v.z)
}

/// Least-squares gradient of a cell-centred scalar field — `Foam::fv::leastSquaresGrad`.
///
/// Returns `grad(phi)` at every cell centre, in units of `[phi]/m`. The returned
/// field's boundary patches are zero-gradient placeholders (the same convention
/// [`grad`](super::grad) uses); only the internal values are meaningful.
///
/// # Algorithm (ESI v2506 `leastSquaresVectors.C` + `leastSquaresGrad.C`)
///
/// ~~Per-cell normal equations with weight `1/|d|^2`, solved by Gaussian
/// elimination, boundary distance `|Cf - C_P|`, unresolved directions pinned.~~
/// **CHANGED 2026-11-11 (gh:#1362, upstream is the specification)**: the port now
/// follows upstream operation for operation (differential row `grad leastSquares`).
///
/// 1. `dd = sum_faces (magSf/|d|^2) sqr(d)`, split between the two cells as
///    `(1-w) wdd` (owner) and `w wdd` (neighbour), `w` the linear face weight;
///    a non-coupled boundary face adds `(magSf/|d|^2) sqr(d)` with `d` the
///    PATCH-NORMAL `fvPatch::delta()`.
/// 2. `invDd = safeInv(dd)` per cell (adjunct over determinant; a direction the
///    stencil does not resolve gets `+1` on its diagonal before the inversion and
///    `-1` after, so a 2-D or 1-D mesh gives a zero component there, and a
///    determinant under `ROOTVSMALL` gives the zero tensor).
/// 3. Per-face vectors `p = (1-w) (magSf/|d|^2) (invDd_P & d)` and
///    `n = -w (magSf/|d|^2) (invDd_N & d)`; boundary `p = magSf (1/|d|^2) (invDd_P & d)`.
/// 4. `grad_P += p (phi_N - phi_P)`, `grad_N -= n (phi_N - phi_P)`, and for each
///    boundary face `grad_P += p (phi_b - phi_P)`.
///
/// The boundary face value `phi_b` is derived from the **boundary-condition
/// kind** (not blindly from `PatchField::values`, which are zero for
/// `ZeroGradient` / `Symmetry` / `Empty` until an operator writes them): the
/// prescribed value for `FixedValue` / `FixedField` / `NoSlip`, the owner cell's
/// own value for `ZeroGradient` / `Symmetry` / `Empty`, and the stored
/// `PatchField::values` entry for every other kind. Upstream reads the patch
/// field itself, which is the same number once the patch has been evaluated.
/// Cyclic and cyclicAMI seams are treated as ordinary boundary faces here
/// (upstream adds the neighbour-side value for coupled patches; not ported).
///
/// # Which gradient: this one or the face-centre one (maintainer decision 2026-11-12, gh:#1336 / gh:#1362)
///
/// Both operators are kept; the choice between them is **left open for a
/// future study**.
///
/// - **This function is the upstream translation: the default, and
///   bit-identical to OpenFOAM** (ESI v2506 `leastSquaresGrad`, differential row
///   `grad leastSquares`, `tests/operators_vs_openfoam.rs`).
/// - **[`grad_least_squares_face_centre`] (port-original) is the more accurate of
///   the two at non-orthogonal boundary cells**, and the reason is in the
///   boundary stencil. Upstream's boundary stencil uses the patch-normal
///   `fvPatch::delta()`, not `Cf - C_P`, so it is not exact for a linear field
///   there. The port-original form uses the true face centre and is exact.
///   Measured on the `amp = 0.5` wavy mesh (17.50 deg max non-orthogonality):
///   max error **3.067e-1** here against **1.823e-14** for the face-centre form
///   (`tests/non_orthogonal_laplacian.rs`; upstream figure in
///   `verification_and_validation/upstream_ops_gh1362/README.md`). Upstream's
///   figure is the faithful behaviour of OpenFOAM, not a defect of this port.
///
/// # Accuracy
///
/// **NOT exact for a linear field at non-orthogonal boundary cells**: that is
/// upstream's behaviour (above). ~~Exact (to rounding) for any linear field on
/// any mesh~~ **CORRECTED 2026-11-12**: that claim, and its 1.823e-14 figure, belong
/// to [`grad_least_squares_face_centre`]; it was written for the pre-gh:#1362
/// operator and the code of this function contradicts it (re-read against the
/// README's 3.067e-1 on the `amp = 0.5` mesh). Internal cells away from a
/// non-orthogonal boundary are unaffected.
///
/// # Example
///
/// ```rust
/// use std::sync::Arc;
/// use outram_foam_basic_lib::fields::vol_field::VolScalarField;
/// use outram_foam_basic_lib::fv_operators::fvc::grad_least_squares;
/// use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;
///
/// // A 1-D periodic ring is uniform, so any field constant on it has zero
/// // gradient — the least-squares fit must return exactly that.
/// let mesh = Arc::new(FvMesh::periodic_1d(8, 1.0, 1.0));
/// let p = VolScalarField::uniform("p", mesh, 2.5);
/// let g = grad_least_squares(&p);
/// assert!(g.internal[0].mag() < 1e-12);
/// ```
pub fn grad_least_squares(phi: &VolScalarField) -> VolVectorField {
    let mesh = phi.mesh.clone();
    let n = mesh.n_cells;

    // dd tensor accumulation (upstream `calcLeastSquaresVectors`)
    let mut dd: Vec<Sym> = vec![[0.0; 6]; n];
    for f in 0..mesh.n_internal_faces {
        let own = mesh.owner[f];
        let nei = mesh.neighbour[f];
        let d = mesh.cell_centres[nei] - mesh.cell_centres[own];
        let w = linear_weight(&mesh, f);
        let wdd = sym_scale(mesh.face_areas[f] / mag_sqr(d), &sym_sqr(d));
        sym_add_assign(&mut dd[own], &sym_scale(1.0 - w, &wdd));
        sym_add_assign(&mut dd[nei], &sym_scale(w, &wdd));
    }
    for patch in &mesh.patches {
        for fi in 0..patch.size {
            let gf = patch.start + fi;
            let d = patch_delta(&mesh, gf);
            let t = sym_scale(mesh.face_areas[gf] / mag_sqr(d), &sym_sqr(d));
            sym_add_assign(&mut dd[mesh.owner[gf]], &t);
        }
    }
    let inv_dd: Vec<Sym> = dd.iter().map(sym_safe_inv).collect();

    // gradient accumulation (upstream `leastSquaresGrad::calcGrad`), the per-face
    // vectors formed on the fly with upstream's arithmetic
    let mut g = vec![Vector3::ZERO; n];
    for f in 0..mesh.n_internal_faces {
        let own = mesh.owner[f];
        let nei = mesh.neighbour[f];
        let d = mesh.cell_centres[nei] - mesh.cell_centres[own];
        let w = linear_weight(&mesh, f);
        let mag_sf_by_mag_sqrd = mesh.face_areas[f] / mag_sqr(d);
        let own_ls = vscale((1.0 - w) * mag_sf_by_mag_sqrd, sym_dot(&inv_dd[own], d));
        let nei_ls = vscale(-w * mag_sf_by_mag_sqrd, sym_dot(&inv_dd[nei], d));
        let delta_vsf = phi.internal[nei] - phi.internal[own];
        g[own] = g[own] + vscale(delta_vsf, own_ls);
        g[nei] = g[nei] - vscale(delta_vsf, nei_ls);
    }
    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let gf = patch.start + fi;
            let owner = mesh.owner[gf];
            let d = patch_delta(&mesh, gf);
            let own_ls = vscale(
                mesh.face_areas[gf] * (1.0 / mag_sqr(d)),
                sym_dot(&inv_dd[owner], d),
            );
            let face_value = boundary_face_value(phi, pi, fi, owner);
            g[owner] = g[owner] + vscale(face_value - phi.internal[owner], own_ls);
        }
    }

    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient_vec(p.size))
        .collect();
    VolVectorField::new(
        crate::fv_operators::naming::derived_name("grad", &phi.name),
        mesh,
        Field::new(g),
        boundary,
    )
}

/// **Port-original** face-centre least-squares gradient (NOT upstream's
/// `leastSquaresGrad`; see [`grad_least_squares`] for that), kept because it is
/// exact for a linear field on any mesh, boundary cells included, which
/// `tests/non_orthogonal_laplacian.rs` and
/// [`solve_laplacian_non_orthogonal`](crate::fv_operators::fvm::solve_laplacian_non_orthogonal)
/// rely on (gh:#1362).
///
/// Differences from upstream: weight `1/|d|^2` (upstream `magSf/|d|^2` split by the
/// face weight), the boundary stencil at the true face centre `Cf - C_P`
/// (upstream: the patch-NORMAL `fvPatch::delta()`, which makes upstream inexact
/// for a linear field at non-orthogonal boundary cells), a Gaussian-elimination
/// solve, and unresolved directions pinned to zero.
///
/// **Which one to use (maintainer decision 2026-11-12, gh:#1336 / gh:#1362):** both
/// are kept and the choice is left open for a future study. This is the
/// **more accurate** of the pair at non-orthogonal boundary cells: **1.823e-14**
/// on the `amp = 0.5` wavy mesh (17.50 deg) against **3.067e-1** for
/// [`grad_least_squares`], because upstream's boundary stencil uses the
/// patch-normal `delta()` and is not exact for a linear field there. It is NOT the
/// default and NOT bit-identical to OpenFOAM; [`grad_least_squares`] is the
/// upstream translation and the default. Measured in
/// `tests/non_orthogonal_laplacian.rs` and
/// `verification_and_validation/upstream_ops_gh1362/README.md`.
///
/// Original description follows.
///
/// Least-squares gradient of a cell-centred scalar field — `Foam::leastSquaresGrad`.
///
/// Returns `grad(phi)` at every cell centre, in units of `[phi]/m`. The returned
/// field's boundary patches are zero-gradient placeholders (the same convention
/// [`grad`](super::grad) uses); only the internal values are meaningful.
///
/// # Stencil
///
/// Each cell's fit uses:
/// - every internal-face neighbour, at `d = C_N - C_P`, and
/// - every boundary face of the cell, at `d = Cf - C_P` with the face value
///   implied by that patch's **boundary condition**: the prescribed value for
///   `FixedValue` / `FixedField` / `NoSlip`, the owner cell's own value for
///   `ZeroGradient` / `Symmetry` / `Empty` (a zero normal gradient), and the
///   stored `PatchField::values` entry for every other kind. Including the
///   boundary faces is what keeps the fit well posed for a cell in a corner of
///   the domain, and matches OpenFOAM's treatment of a `fixedValue` patch.
///   Reading `values` unconditionally would be wrong for `ZeroGradient`, whose
///   stored values are zero until an operator writes them.
///
/// # Accuracy
///
/// **Exact (to rounding) for any linear field on any mesh**, provided the
/// boundary face values are themselves the exact field values there — this is
/// the property the non-orthogonal Laplacian correction depends on. Measured on
/// a wavy mesh of 17.50 degrees max non-orthogonality
/// (`tests/non_orthogonal_laplacian.rs`, 2026-08-07): max error **1.823e-14**
/// against the analytic gradient, versus **1.845e-02** for
/// [`grad`](super::grad) on the identical field and mesh. On a curved field it
/// is second-order accurate like the Gauss gradient (not measured here).
///
/// # Degenerate cells
///
/// A cell whose neighbour directions do not span 3-D (a 2-D or 1-D mesh, or a
/// cell with fewer than three faces) yields a zero gradient component in each
/// unresolved Cartesian direction; see the module documentation. A cell with no
/// faces at all yields a zero gradient.
///
/// # Example
///
/// ```rust
/// use std::sync::Arc;
/// use outram_foam_basic_lib::fields::vol_field::VolScalarField;
/// use outram_foam_basic_lib::fv_operators::fvc::grad_least_squares_face_centre;
/// use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;
///
/// // A 1-D periodic ring is uniform, so any field constant on it has zero
/// // gradient — the least-squares fit must return exactly that.
/// let mesh = Arc::new(FvMesh::periodic_1d(8, 1.0, 1.0));
/// let p = VolScalarField::uniform("p", mesh, 2.5);
/// let g = grad_least_squares_face_centre(&p);
/// assert!(g.internal[0].mag() < 1e-12);
/// ```
pub fn grad_least_squares_face_centre(phi: &VolScalarField) -> VolVectorField {
    let mesh = phi.mesh.clone();
    let n = mesh.n_cells;

    // Normal-equation accumulators, one symmetric 3x3 and one 3-vector per cell.
    // Stored as [xx, xy, xz, yy, yz, zz] to keep the allocation small.
    let mut m = vec![[0.0_f64; 6]; n];
    let mut r = vec![Vector3::ZERO; n];

    let accumulate =
        |cell: usize, d: Vector3, dphi: f64, m: &mut Vec<[f64; 6]>, r: &mut Vec<Vector3>| {
            let d2 = d.mag_sqr();
            if d2 < 1.0e-300 {
                return;
            }
            // OpenFOAM's inverse-distance weight: w = 1/|d|, so w^2 = 1/|d|^2.
            let w2 = 1.0 / d2;
            let e = &mut m[cell];
            e[0] += w2 * d.x * d.x;
            e[1] += w2 * d.x * d.y;
            e[2] += w2 * d.x * d.z;
            e[3] += w2 * d.y * d.y;
            e[4] += w2 * d.y * d.z;
            e[5] += w2 * d.z * d.z;
            r[cell] = r[cell] + d * (w2 * dphi);
        };

    // Internal faces contribute to both sides of the face.
    for f in 0..mesh.n_internal_faces {
        let o = mesh.owner[f];
        let nb = mesh.neighbour[f];
        let d = mesh.cell_centres[nb] - mesh.cell_centres[o];
        let dphi = phi.internal[nb] - phi.internal[o];
        accumulate(o, d, dphi, &mut m, &mut r);
        accumulate(nb, -d, -dphi, &mut m, &mut r);
    }

    // Boundary faces: the "neighbour" is the face centre carrying the patch value.
    for (pi, patch) in mesh.patches.iter().enumerate() {
        for fi in 0..patch.size {
            let gf = patch.start + fi;
            let owner = mesh.owner[gf];
            let d = mesh.face_centres[gf] - mesh.cell_centres[owner];
            let face_value = boundary_face_value(phi, pi, fi, owner);
            let dphi = face_value - phi.internal[owner];
            accumulate(owner, d, dphi, &mut m, &mut r);
        }
    }

    // Solve the 3x3 normal equations per cell, pinning unresolved directions.
    let mut g = vec![Vector3::ZERO; n];
    for c in 0..n {
        let e = m[c];
        let trace = e[0] + e[3] + e[5];
        if trace < 1.0e-300 {
            continue; // cell has no usable neighbour directions
        }
        let mut a = SquareMatrix::new(3);
        a.set(0, 0, e[0]);
        a.set(0, 1, e[1]);
        a.set(0, 2, e[2]);
        a.set(1, 0, e[1]);
        a.set(1, 1, e[3]);
        a.set(1, 2, e[4]);
        a.set(2, 0, e[2]);
        a.set(2, 1, e[4]);
        a.set(2, 2, e[5]);
        let mut rhs = [r[c].x, r[c].y, r[c].z];

        // Rank repair: a direction the stencil does not resolve gets a unit
        // diagonal and a zero right-hand side, i.e. gradient component = 0.
        for (k, diag) in [e[0], e[3], e[5]].into_iter().enumerate() {
            if diag <= RANK_TOL * trace {
                for j in 0..3 {
                    a.set(k, j, 0.0);
                    a.set(j, k, 0.0);
                }
                a.set(k, k, 1.0);
                rhs[k] = 0.0;
            }
        }

        if let Ok(sol) = a.solve(&rhs) {
            g[c] = Vector3::new(sol[0], sol[1], sol[2]);
        }
        // On a singular system not caught by the rank repair (a genuinely
        // degenerate stencil), leave the gradient at zero rather than emitting
        // NaN into the solution field.
    }

    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::zero_gradient_vec(p.size))
        .collect();
    VolVectorField::new(
        crate::fv_operators::naming::derived_name("grad", &phi.name),
        mesh,
        Field::new(g),
        boundary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fields::boundary::bc::PatchField;
    use crate::mesh::fv_mesh::{BoundaryPatch, FvMesh, FvMeshBuilder, PatchKind};
    use approx::assert_relative_eq;
    use std::sync::Arc;

    /// Two cells along x with a wall patch at each end — the same fixture
    /// `fvc::grad`'s own tests use.
    fn unit_mesh() -> Arc<FvMesh> {
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(2)
                .n_internal_faces(1)
                .owner(vec![0, 1, 0])
                .neighbour(vec![1])
                .patches(vec![
                    BoundaryPatch::new("right", 1, 1, PatchKind::Wall),
                    BoundaryPatch::new("left", 2, 1, PatchKind::Wall),
                ])
                .cell_volumes(vec![0.5, 0.5])
                .cell_centres(vec![
                    Vector3::new(0.25, 0.0, 0.0),
                    Vector3::new(0.75, 0.0, 0.0),
                ])
                .face_area_vectors(vec![
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(-1.0, 0.0, 0.0),
                ])
                .face_centres(vec![
                    Vector3::new(0.5, 0.0, 0.0),
                    Vector3::new(1.0, 0.0, 0.0),
                    Vector3::new(0.0, 0.0, 0.0),
                ])
                .build()
                .unwrap(),
        )
    }

    #[test]
    fn uniform_field_has_zero_gradient() {
        let m = unit_mesh();
        let p = VolScalarField::uniform("p", m, 3.0);
        let g = grad_least_squares(&p);
        assert!(g.internal[0].mag() < 1e-12);
        assert!(g.internal[1].mag() < 1e-12);
    }

    #[test]
    fn linear_field_recovered_exactly_in_1d() {
        // p = x, with the exact boundary values at x = 1 and x = 0.
        let m = unit_mesh();
        let boundary = vec![
            PatchField::fixed_value(1, 1.0), // right, x = 1
            PatchField::fixed_value(1, 0.0), // left,  x = 0
        ];
        let p = VolScalarField::new("p", m, Field::new(vec![0.25, 0.75]), boundary);
        let g = grad_least_squares(&p);
        for c in 0..2 {
            assert_relative_eq!(g.internal[c].x, 1.0, epsilon = 1e-12);
            // The mesh has no y/z extent, so those components must be pinned to
            // exactly zero by the rank repair, not left as NaN.
            assert_eq!(g.internal[c].y, 0.0);
            assert_eq!(g.internal[c].z, 0.0);
        }
    }

    #[test]
    fn periodic_ring_uniform_field_has_zero_gradient() {
        let mesh = Arc::new(FvMesh::periodic_1d(8, 1.0, 1.0));
        let p = VolScalarField::uniform("p", mesh, 2.5);
        let g = grad_least_squares(&p);
        for c in 0..8 {
            assert!(g.internal[c].mag() < 1e-12, "cell {c}: {:?}", g.internal[c]);
        }
    }
}
