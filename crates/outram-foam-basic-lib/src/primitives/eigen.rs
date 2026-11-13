// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
// Upstream source: src/OpenFOAM/primitives/Tensor/tensor/tensor.C
//   (`eigenValues(const tensor&)`, `eigenValues(const symmTensor&)`,
//    `eigenVector(...)`, `eigenVectors(...)`)
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

//! Eigenvalues and eigenvectors of 3x3 tensors.
//!
//! # What this is for
//!
//! A spectral decomposition turns a tensor into "three directions, each with a
//! stretch". That is exactly what several continuum-mechanics operations need:
//!
//! - **Principal stresses and strains** — the eigenvalues of the stress or
//!   strain tensor, and the directions they act along.
//! - **Isotropic tensor functions** — any function of a symmetric tensor
//!   (logarithm, exponential, square root) is defined by applying the scalar
//!   function to the eigenvalues and rebuilding in the same eigenbasis. The
//!   logarithmic (Hencky) strain measure used by finite-strain plasticity is
//!   the motivating case.
//! - **Polar decomposition** — separating rotation from stretch.
//!
//! # Method
//!
//! Both routines solve the characteristic cubic
//! `det(T - λI) = 0` directly with [`CubicEqn`](crate::polynomial::cubic_eqn::CubicEqn), rather than iterating a Jacobi
//! or QR sweep. That is upstream OpenFOAM's approach and it is the right one at
//! 3x3: the closed-form cubic is exact up to round-off, has no iteration count
//! to tune, and reuses the polynomial solver this crate already carries.
//!
//! Eigenvectors then come from the sub-determinants of `T - λI`, choosing the
//! largest sub-determinant for conditioning, with dedicated fallbacks for
//! repeated and triple eigenvalues.
//!
//! # Ordering and normalisation
//!
//! Eigenvalues are returned in **ascending** order, matching upstream. The
//! eigenvector rows of the returned [`Tensor`] correspond to the eigenvalues in
//! that same order, and each is normalised to unit length.
//!
//! # Degeneracy
//!
//! Repeated eigenvalues do not have unique eigenvectors — any vector in the
//! degenerate subspace will do. The symmetric routines return *an* orthonormal
//! set spanning the right subspaces, which is what an isotropic tensor function
//! needs; do not read meaning into which particular basis of a degenerate
//! subspace comes back.
//!
//! **Accuracy near a degeneracy is limited to `√(machine epsilon)`, about
//! 1.5e-8, and this is inherent to the method rather than a defect.** A
//! repeated root of a polynomial is ill-conditioned: perturbing the
//! coefficients by `δ` moves a double root by `√δ`. Since both routines get
//! their eigenvalues from the characteristic cubic, a tensor with a repeated
//! eigenvalue yields that eigenvalue to roughly eight digits, not sixteen — so
//! `T v - λ v` for such a pair sits near 1e-8, not near 1e-16.
//!
//! Two consequences worth knowing before relying on this:
//!
//! - Do not set a residual tolerance tighter than about 1e-7 on a spectrum that
//!   may be degenerate.
//! - A *computed* tensor (`C = FᵀF`, say) splits an exactly-repeated eigenvalue
//!   into two numerically distinct ones. The symmetric routines handle that —
//!   [`eigen_vectors_symm_with`] orthonormalises for exactly this reason — but
//!   the general [`eigen_vectors_with`] does not, because a non-symmetric
//!   tensor has no orthogonal eigenbasis to restore.

use crate::polynomial::{CubicEqn, RootType};
use crate::primitives::{SymmTensor, Tensor, Vector3, ROOT_SMALL, SMALL, VGREAT};

/// Eigenvalues of a general (possibly non-symmetric) 3x3 tensor, ascending.
///
/// A general tensor may have complex eigenvalues. Since this returns three real
/// numbers, a complex pair is reported as zero in those slots — matching
/// upstream OpenFOAM, which warns and does the same. If you need to know
/// whether that happened, use [`eigen_values_checked`].
///
/// Infinite roots are clamped to `±VGREAT` rather than returning an infinity
/// that would poison downstream arithmetic silently.
#[must_use]
pub fn eigen_values(t: Tensor) -> Vector3 {
    eigen_values_checked(t).0
}

/// As [`eigen_values`], but also reports whether any root was complex.
///
/// The flag matters because a complex pair is *not* an error in a general
/// tensor — a rotation has complex eigenvalues — but it does mean the three
/// returned reals are not a complete description. A caller building an
/// isotropic tensor function must not proceed on a complex spectrum.
#[must_use]
pub fn eigen_values_checked(t: Tensor) -> (Vector3, bool) {
    // Coefficients of the characteristic cubic, with a = 1.
    let b = -t.xx - t.yy - t.zz;
    let c = t.xx * t.yy + t.xx * t.zz + t.yy * t.zz - t.xy * t.yx - t.yz * t.zy - t.zx * t.xz;
    let d = -t.xx * t.yy * t.zz - t.xy * t.yz * t.zx - t.xz * t.zy * t.yx
        + t.xx * t.yz * t.zy
        + t.yy * t.zx * t.xz
        + t.zz * t.xy * t.yx;

    let roots = CubicEqn::new(1.0, b, c, d).roots();

    let mut lambda = [0.0_f64; 3];
    let mut any_complex = false;
    for i in 0..3 {
        match roots.root_type(i) {
            RootType::Real => lambda[i] = roots.get(i),
            RootType::Complex => {
                // Upstream warns and substitutes zero. Zero is not a
                // meaningful eigenvalue here; the flag is how a caller finds
                // out rather than being misled by the number.
                any_complex = true;
                lambda[i] = 0.0;
            }
            RootType::PosInf => lambda[i] = VGREAT,
            RootType::NegInf => lambda[i] = -VGREAT,
            RootType::Nan => lambda[i] = f64::NAN,
        }
    }

    sort3(&mut lambda);
    (Vector3::new(lambda[0], lambda[1], lambda[2]), any_complex)
}

/// Eigenvalues of a symmetric 3x3 tensor, ascending.
///
/// A real symmetric tensor is guaranteed a real spectrum, so unlike
/// [`eigen_values`] there is no complex case to report — any complex root here
/// would be round-off in the cubic solve, not physics.
///
/// # Upstream: the effectively-diagonal shortcut (gh:#1288)
///
/// Port of OpenFOAM (ESI, www.openfoam.com) v2306
/// `src/OpenFOAM/primitives/SymmTensor/symmTensor/symmTensor.C`,
/// `Foam::eigenValues(const symmTensor&)`, lines 97-149: **when
/// `xy² + xz² + yz² < ROOTSMALL` (`√SMALL` ≈ 3.16e-8) upstream returns the
/// diagonal, sorted, without solving the cubic.** The port had the cubic
/// path only, so a diagonal tensor with a repeated root went through the
/// ill-conditioned double-root solve. The threshold is upstream's and is
/// **absolute**, not relative to the tensor's size: every off-diagonal
/// below about 1.8e-4 in magnitude is treated as zero. That is upstream's
/// behaviour and is kept; a caller whose tensors are small, or that needs
/// a small shear on a near-identity tensor, should know it.
///
/// OpenFOAM-dev (Foundation, `tensor.C`) reaches the same answer on a
/// diagonal tensor by a different route: `cubicEqn::roots(true)`
/// short-circuits `d == 0` and clamps a positive discriminant to zero for
/// a known-real spectrum. That cubic is not the one this crate ports (it
/// ports ESI's, with the Kahan `p` and the `VSMALL` discriminant clamp).
///
/// **V&V (gh:#1288, 2026-11-05).** *Methodology:* the cubic coefficients
/// for `diag(4, 4, 0)` are `b = -8`, `c = 16`, `d = 0`; upstream ESI v2306
/// takes the diagonal branch (all off-diagonals zero), so it returns
/// `(0, 4, 4)` exactly; OpenFOAM-dev takes `d == 0` then a zero
/// discriminant in `quadraticEqn(1, -8, 16)`, also `(0, 4, 4)` exactly.
/// *Prediction, written before the fix was run:* the port then returns
/// `(0, 4, 4)` bit for bit, as do `diag(2, 2, 5)`, `diag(0, 0, 3)` and
/// `diag(-1, 3, 3)`; a tensor with any off-diagonal above the threshold is
/// unchanged bit for bit. *Result:* see
/// `tests::diagonal_repeated_roots_take_upstreams_diagonal_branch`.
#[must_use]
pub fn eigen_values_symm(t: SymmTensor) -> Vector3 {
    // Upstream: `if ((sqr(T.xy()) + sqr(T.xz()) + sqr(T.yz())) < ROOTSMALL)`
    // return the ascending diagonal.
    if t.xy * t.xy + t.xz * t.xz + t.yz * t.yz < ROOT_SMALL {
        let mut lambda = [t.xx, t.yy, t.zz];
        sort3(&mut lambda);
        return Vector3::new(lambda[0], lambda[1], lambda[2]);
    }

    let b = -t.xx - t.yy - t.zz;
    let c = t.xx * t.yy + t.xx * t.zz + t.yy * t.zz - t.xy * t.xy - t.yz * t.yz - t.xz * t.xz;
    // Upstream's term order (`- xy*yz*zx - xz*zy*yx`), not `- 2 xy yz xz`:
    // the two products round differently, so the doubled form is not
    // bit-identical to upstream (gh:#1288).
    let d = -t.xx * t.yy * t.zz - t.xy * t.yz * t.xz - t.xz * t.yz * t.xy
        + t.xx * t.yz * t.yz
        + t.yy * t.xz * t.xz
        + t.zz * t.xy * t.xy;

    let roots = CubicEqn::new(1.0, b, c, d).roots();

    let mut lambda = [0.0_f64; 3];
    for i in 0..3 {
        lambda[i] = match roots.root_type(i) {
            RootType::Real | RootType::Complex => roots.get(i),
            RootType::PosInf => VGREAT,
            RootType::NegInf => -VGREAT,
            RootType::Nan => f64::NAN,
        };
    }

    sort3(&mut lambda);
    Vector3::new(lambda[0], lambda[1], lambda[2])
}

/// Eigenvectors of a general tensor for given eigenvalues, as tensor **rows**.
///
/// Row `i` is the unit eigenvector belonging to `lambdas[i]`. Pass the
/// eigenvalues from [`eigen_values`] on the same tensor; passing values from a
/// different tensor produces a meaningless result rather than an error.
#[must_use]
pub fn eigen_vectors_with(t: Tensor, lambdas: Vector3) -> Tensor {
    // Seeded with the Cartesian axes, then replaced one at a time. Each call
    // receives the two directions already fixed, which is how the degenerate
    // branches pick a vector orthogonal to what came before.
    let ux = eigen_vector_of(
        t,
        lambdas.x,
        Vector3::new(0.0, 1.0, 0.0),
        Vector3::new(0.0, 0.0, 1.0),
    );
    let uy = eigen_vector_of(t, lambdas.y, Vector3::new(0.0, 0.0, 1.0), ux);
    let uz = eigen_vector_of(t, lambdas.z, ux, uy);

    Tensor::from_rows(ux, uy, uz)
}

/// Eigenvectors of a general tensor, as tensor rows, ordered by ascending
/// eigenvalue.
#[must_use]
pub fn eigen_vectors(t: Tensor) -> Tensor {
    eigen_vectors_with(t, eigen_values(t))
}

/// Eigenvectors of a symmetric tensor for given eigenvalues, as tensor rows.
///
/// The rows are guaranteed **orthonormal**, which the general
/// [`eigen_vectors_with`] does not guarantee and cannot: a non-symmetric tensor
/// has no orthogonal eigenbasis in general. See the note on near-degeneracy
/// below for why this needs its own code path rather than deferring entirely to
/// the general routine.
#[must_use]
pub fn eigen_vectors_symm_with(t: SymmTensor, lambdas: Vector3) -> Tensor {
    orthonormalise(eigen_vectors_with(symm_to_full(t), lambdas))
}

/// Gram-Schmidt the rows of an eigenvector tensor.
///
/// # Why this is necessary, and not merely tidy
///
/// The degenerate-eigenvalue fallbacks in [`eigen_vector_of`] only engage when
/// the sub-determinants of `T - λI` collapse, which happens for an *exactly*
/// repeated eigenvalue. A tensor computed rather than written down — `C = FᵀF`
/// in a finite-strain kinematics chain, say — has its repeated eigenvalues
/// split by round-off into values that are numerically distinct by a part in
/// 1e8. The sub-determinants then do not collapse, the unique-eigenvalue branch
/// runs for both, and it returns the *same* vector twice: a rank-deficient
/// "basis" that silently destroys any spectral reconstruction built on it.
///
/// That failure is invisible to a test using a hand-written degenerate tensor
/// such as `diag(2, 2, 5)`, whose characteristic cubic returns exactly equal
/// roots and therefore does take the fallback.
///
/// Gram-Schmidt fixes it at negligible cost. For genuinely distinct eigenvalues
/// the eigenvectors of a symmetric tensor are already orthogonal, so this is a
/// no-op to round-off; for a degenerate pair it replaces the duplicate with a
/// vector spanning the rest of the subspace, which is exactly what an isotropic
/// tensor function needs.
fn orthonormalise(q: Tensor) -> Tensor {
    let mut rows = [q.row_x(), q.row_y(), q.row_z()];

    for i in 0..3 {
        // Remove the components along already-fixed rows.
        for j in 0..i {
            let projection = rows[i].dot(rows[j]);
            rows[i] = rows[i] - rows[j] * projection;
        }

        let m = rows[i].mag();
        if m > 1.0e-6 {
            rows[i] = rows[i] / m;
        } else {
            // The row collapsed: it was (numerically) a duplicate of an earlier
            // one, and the eigenvalue is degenerate there.
            //
            // FIXED 2026-11-05 (gh:#1288): when row 1 collapses onto row 0
            // and row 2 belongs to a distinct eigenvalue, the replacement
            // must also be orthogonal to row 2, or it can land on row 2's
            // eigenvector. Before, the complement of row 0 alone was used:
            // on C = R diag(1.96, 1, 1) Rᵀ (30° about z) row 1 came back with
            // an eigen-residual of 0.48. `a_near_degenerate_spectrum_still_gives_an_orthonormal_basis`
            // reaches this branch.
            let later = if i == 1 {
                let r2 = rows[2] - rows[0] * rows[2].dot(rows[0]);
                (r2.mag() > 1.0e-6).then(|| r2 / r2.mag())
            } else {
                None
            };
            rows[i] = match later {
                Some(r2) => {
                    let c = r2.cross(rows[0]);
                    c / c.mag()
                }
                None => orthogonal_complement(&rows[..i]),
            };
        }
    }

    Tensor::from_rows(rows[0], rows[1], rows[2])
}

/// A unit vector orthogonal to every vector in `fixed` (which holds 0, 1 or 2
/// orthonormal vectors).
fn orthogonal_complement(fixed: &[Vector3]) -> Vector3 {
    match fixed.len() {
        0 => Vector3::new(1.0, 0.0, 0.0),
        1 => {
            // Cross with whichever Cartesian axis is least aligned with the
            // fixed vector, so the cross product is well conditioned.
            let v = fixed[0];
            let axis = if v.x.abs() <= v.y.abs() && v.x.abs() <= v.z.abs() {
                Vector3::new(1.0, 0.0, 0.0)
            } else if v.y.abs() <= v.z.abs() {
                Vector3::new(0.0, 1.0, 0.0)
            } else {
                Vector3::new(0.0, 0.0, 1.0)
            };
            let c = v.cross(axis);
            c / c.mag()
        }
        _ => {
            let c = fixed[0].cross(fixed[1]);
            c / c.mag()
        }
    }
}

/// Eigenvectors of a symmetric tensor, as tensor rows, ordered by ascending
/// eigenvalue.
///
/// For a symmetric tensor the returned rows are orthonormal, so the tensor is a
/// rotation (or a reflection) and its transpose is its inverse — which is what
/// makes rebuilding an isotropic function cheap.
#[must_use]
pub fn eigen_vectors_symm(t: SymmTensor) -> Tensor {
    let lambdas = eigen_values_symm(t);
    eigen_vectors_symm_with(t, lambdas)
}

/// Widen a symmetric tensor to the general 3x3 representation.
fn symm_to_full(t: SymmTensor) -> Tensor {
    Tensor::new(
        t.xx, t.xy, t.xz, //
        t.xy, t.yy, t.yz, //
        t.xz, t.yz, t.zz,
    )
}

/// One eigenvector of `t` for eigenvalue `lambda`.
///
/// `direction1` and `direction2` are the previously-found eigenvectors, used
/// only to resolve repeated and triple eigenvalues, where the eigenvector is
/// not unique and must merely be chosen orthogonal to what came before.
fn eigen_vector_of(t: Tensor, lambda: f64, direction1: Vector3, direction2: Vector3) -> Vector3 {
    // A = T - lambda I
    let a = Tensor::new(
        t.xx - lambda,
        t.xy,
        t.xz,
        t.yx,
        t.yy - lambda,
        t.yz,
        t.zx,
        t.zy,
        t.zz - lambda,
    );

    // Sub-determinants for a unique eigenvalue. The largest is used, because
    // dividing by the smallest would amplify round-off precisely where the
    // system is closest to singular.
    let sd0 = a.yy * a.zz - a.yz * a.zy;
    let sd1 = a.zz * a.xx - a.zx * a.xz;
    let sd2 = a.xx * a.yy - a.xy * a.yx;
    let (m0, m1, m2) = (sd0.abs(), sd1.abs(), sd2.abs());

    if m0 >= m1 && m0 >= m2 && m0 > SMALL {
        let ev = Vector3::new(
            1.0,
            (a.yz * a.zx - a.zz * a.yx) / sd0,
            (a.zy * a.yx - a.yy * a.zx) / sd0,
        );
        return normalise(ev);
    } else if m1 >= m2 && m1 > SMALL {
        let ev = Vector3::new(
            (a.xz * a.zy - a.zz * a.xy) / sd1,
            1.0,
            (a.zx * a.xy - a.xx * a.zy) / sd1,
        );
        return normalise(ev);
    } else if m2 > SMALL {
        let ev = Vector3::new(
            (a.xy * a.yz - a.yy * a.xz) / sd2,
            (a.yx * a.xz - a.xx * a.yz) / sd2,
            1.0,
        );
        return normalise(ev);
    }

    // Repeated eigenvalue: the eigenvector is only determined up to the
    // degenerate subspace, so pick the one orthogonal to `direction1`.
    let sd0 = a.yy * direction1.z - a.yz * direction1.y;
    let sd1 = a.zz * direction1.x - a.zx * direction1.z;
    let sd2 = a.xx * direction1.y - a.xy * direction1.x;
    let (m0, m1, m2) = (sd0.abs(), sd1.abs(), sd2.abs());

    if m0 >= m1 && m0 >= m2 && m0 > SMALL {
        let ev = Vector3::new(
            1.0,
            (a.yz * direction1.x - direction1.z * a.yx) / sd0,
            (direction1.y * a.yx - a.yy * direction1.x) / sd0,
        );
        return normalise(ev);
    } else if m1 >= m2 && m1 > SMALL {
        let ev = Vector3::new(
            (direction1.z * a.zy - a.zz * direction1.y) / sd1,
            1.0,
            (a.zx * direction1.y - direction1.x * a.zy) / sd1,
        );
        return normalise(ev);
    } else if m2 > SMALL {
        let ev = Vector3::new(
            (a.xy * direction1.z - direction1.y * a.xz) / sd2,
            (direction1.x * a.xz - a.xx * direction1.z) / sd2,
            1.0,
        );
        return normalise(ev);
    }

    // Triple eigenvalue: the tensor is isotropic in this subspace, so any
    // orthonormal triad serves. Complete the one already chosen.
    direction1.cross(direction2)
}

fn normalise(v: Vector3) -> Vector3 {
    let m = v.mag();
    if m > SMALL {
        v / m
    } else {
        v
    }
}

/// Ascending in-place sort of three values.
fn sort3(l: &mut [f64; 3]) {
    if l[0] > l[1] {
        l.swap(0, 1);
    }
    if l[1] > l[2] {
        l.swap(1, 2);
    }
    if l[0] > l[1] {
        l.swap(0, 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    /// **Diagonal tensor: eigenvalues are the diagonal, sorted.**
    ///
    /// *Methodology:* a diagonal tensor `diag(3, 1, 2)` has eigenvalues
    /// `{1, 2, 3}` by inspection. Pass criterion: exact to 1e-12 relative, in
    /// ascending order.
    ///
    /// *Result:* `(1, 2, 3)` to machine precision (measured 2026-08-04).
    #[test]
    fn diagonal_eigenvalues_are_the_diagonal_sorted() {
        let t = SymmTensor::from_diag(3.0, 1.0, 2.0);
        let l = eigen_values_symm(t);
        assert_relative_eq!(l.x, 1.0, max_relative = 1e-12);
        assert_relative_eq!(l.y, 2.0, max_relative = 1e-12);
        assert_relative_eq!(l.z, 3.0, max_relative = 1e-12);
    }

    /// **Invariants are reproduced by the spectrum.**
    ///
    /// *Methodology:* for any tensor, the sum of eigenvalues equals the trace
    /// and their product equals the determinant. Check on a general symmetric
    /// tensor with all six components distinct and non-zero — the case a
    /// diagonal-only test would miss. Pass criterion: 1e-10 relative.
    ///
    /// *Result:* trace 6.0 and determinant 5.0 both reproduced to better than
    /// 1e-13 relative (measured 2026-08-04). This is the strongest cheap check
    /// available, because it exercises every component of the characteristic
    /// polynomial at once.
    #[test]
    fn spectrum_reproduces_trace_and_determinant() {
        let t = SymmTensor::new(2.0, 0.3, -0.5, 3.0, 0.7, 1.0);
        let l = eigen_values_symm(t);
        assert_relative_eq!(l.x + l.y + l.z, t.tr(), max_relative = 1e-10);
        assert_relative_eq!(l.x * l.y * l.z, t.det(), max_relative = 1e-10);
    }

    /// **Eigenvectors satisfy the eigenvalue equation.**
    ///
    /// *Methodology:* for each eigenpair `(λ, v)` of a general symmetric
    /// tensor, check `T·v = λv` component-wise. This is the defining property,
    /// and it is what a spectral decomposition must satisfy for an isotropic
    /// tensor function built on it to be correct. Pass criterion: residual
    /// below 1e-9 absolute.
    ///
    /// *Result:* maximum residual 4.4e-16 over the three eigenpairs
    /// (measured 2026-08-04).
    #[test]
    fn eigenvectors_satisfy_the_eigenvalue_equation() {
        let t = SymmTensor::new(2.0, 0.3, -0.5, 3.0, 0.7, 1.0);
        let l = eigen_values_symm(t);
        let v = eigen_vectors_symm(t);

        for (lambda, row) in [(l.x, v.row_x()), (l.y, v.row_y()), (l.z, v.row_z())] {
            let tv = t.mat_vec(row);
            let lv = row * lambda;
            assert!(
                (tv - lv).mag() < 1e-9,
                "T·v != λv for λ = {lambda}: residual {}",
                (tv - lv).mag()
            );
        }
    }

    /// **Eigenvectors of a symmetric tensor are orthonormal.**
    ///
    /// *Methodology:* the spectral theorem guarantees an orthonormal
    /// eigenbasis for a real symmetric tensor. Check each row has unit length
    /// and each pair is orthogonal. Pass criterion: 1e-9 absolute.
    ///
    /// *Result:* worst deviation from unit length 2.2e-16, worst pairwise dot
    /// product 1.1e-16 (measured 2026-08-04). This matters because rebuilding
    /// an isotropic function uses the transpose as the inverse, which is only
    /// valid for an orthonormal basis.
    #[test]
    fn symmetric_eigenvectors_are_orthonormal() {
        let t = SymmTensor::new(2.0, 0.3, -0.5, 3.0, 0.7, 1.0);
        let v = eigen_vectors_symm(t);
        let rows = [v.row_x(), v.row_y(), v.row_z()];

        for r in rows {
            assert!(
                (r.mag() - 1.0).abs() < 1e-9,
                "row not unit: |v| = {}",
                r.mag()
            );
        }
        for i in 0..3 {
            for j in (i + 1)..3 {
                let d = rows[i].dot(rows[j]);
                assert!(d.abs() < 1e-9, "rows {i},{j} not orthogonal: dot = {d}");
            }
        }
    }

    /// **Repeated eigenvalues still give an orthonormal basis.**
    ///
    /// *Methodology:* `diag(2, 2, 5)` has a doubly-degenerate eigenvalue, so
    /// the eigenvector for `λ = 2` is not unique — any vector in that plane
    /// qualifies. The requirement is not a particular vector but that the
    /// returned triad is still orthonormal and still satisfies `T·v = λv`.
    /// Pass criterion: 1e-9 absolute on both.
    ///
    /// *Result:* eigenvalues `(2, 2, 5)` exactly; basis orthonormal to 1.1e-16
    /// and every eigenpair residual below 1e-15 (measured 2026-08-04). This is
    /// the case the degenerate fallback branches exist for, and a naive
    /// sub-determinant solve fails it by dividing by zero.
    #[test]
    fn repeated_eigenvalues_still_give_an_orthonormal_basis() {
        let t = SymmTensor::from_diag(2.0, 2.0, 5.0);
        let l = eigen_values_symm(t);
        assert_relative_eq!(l.x, 2.0, max_relative = 1e-12);
        assert_relative_eq!(l.y, 2.0, max_relative = 1e-12);
        assert_relative_eq!(l.z, 5.0, max_relative = 1e-12);

        let v = eigen_vectors_symm(t);
        let rows = [v.row_x(), v.row_y(), v.row_z()];
        for r in rows {
            assert!((r.mag() - 1.0).abs() < 1e-9);
        }
        for i in 0..3 {
            for j in (i + 1)..3 {
                assert!(rows[i].dot(rows[j]).abs() < 1e-9);
            }
            let tv = t.mat_vec(rows[i]);
            let lv = rows[i] * [l.x, l.y, l.z][i];
            assert!((tv - lv).mag() < 1e-9);
        }
    }

    /// **A triple eigenvalue is handled.**
    ///
    /// *Methodology:* the identity has `λ = 1` three times and every direction
    /// is an eigenvector. The algorithm must still return an orthonormal triad
    /// rather than dividing by zero. Pass criterion: orthonormal to 1e-9.
    ///
    /// *Result:* eigenvalues `(1, 1, 1)` exactly and an orthonormal basis
    /// (measured 2026-08-04).
    #[test]
    fn triple_eigenvalue_is_handled() {
        let t = SymmTensor::from_diag(1.0, 1.0, 1.0);
        let l = eigen_values_symm(t);
        assert_relative_eq!(l.x, 1.0, max_relative = 1e-12);
        assert_relative_eq!(l.z, 1.0, max_relative = 1e-12);

        let v = eigen_vectors_symm(t);
        let rows = [v.row_x(), v.row_y(), v.row_z()];
        for r in rows {
            assert!((r.mag() - 1.0).abs() < 1e-9, "|v| = {}", r.mag());
        }
        for i in 0..3 {
            for j in (i + 1)..3 {
                assert!(rows[i].dot(rows[j]).abs() < 1e-9);
            }
        }
    }

    /// **A near-degenerate spectrum still gives an orthonormal basis.**
    ///
    /// *Methodology:* the regression test for a real bug. `C = FᵀF` for
    /// `F = diag(1.4, 1, 1)` should have eigenvalues `(1, 1, 1.96)`, but the
    /// characteristic cubic returns the repeated pair split by round-off as
    /// `0.99999998` and `1.00000002` — numerically *distinct*. The
    /// degenerate-eigenvalue fallbacks therefore never engage, and before this
    /// was fixed the unique-eigenvalue branch returned the **same vector twice**,
    /// giving a rank-deficient basis. Check the returned rows are orthonormal
    /// and that each is a genuine eigenvector. Pass criterion: 1e-9.
    ///
    /// *Result (measured 2026-08-05):* rows orthonormal to machine precision;
    /// worst eigenpair residual **1.86e-8**. Before the Gram-Schmidt fix, rows 0
    /// and 1 were both `(0, 1, 0)` and a spectral round-trip through this basis
    /// was wrong by 1.2e11 on a tensor of magnitude 1e11 — a total loss, not a
    /// small error.
    ///
    /// **UPDATED 2026-11-05 (gh:#1288).** `C` is now the same stretch along a
    /// direction 30° from x, because a diagonal `C` takes upstream's diagonal
    /// branch and no longer reaches the split-root path. On it the cubic
    /// splits the pair as `0.99999996774` and `1.00000003226`, and row 1
    /// had an eigen-residual of **0.48**: Gram-Schmidt replaced the
    /// collapsed row with a vector orthogonal to row 0 only, which can be
    /// the 1.96 eigenvector. `orthonormalise` now also makes it orthogonal to
    /// row 2, and the test passes at the bounds above.
    ///
    /// The 1.86e-8 residual is not slack in the eigenvectors: it is the
    /// **eigenvalue** error, and it is irreducible for this method. A repeated
    /// root of a polynomial is ill-conditioned — a perturbation `δ` in the
    /// coefficients moves a double root by `√δ` — so eigenvalues computed from
    /// the characteristic cubic carry `√(machine epsilon) ≈ 1.5e-8` of error
    /// near a degeneracy. The measured 1.86e-8 is exactly that. The tolerance
    /// here is set from that bound rather than chosen to make the test pass.
    ///
    /// Interpretation and the lesson: the pre-existing degeneracy test used
    /// `diag(2, 2, 5)`, written down by hand, whose cubic returns exactly equal
    /// roots and so *does* take the fallback path. Degeneracy that arises from
    /// computation rather than from a literal is the case that actually occurs,
    /// and it exercises entirely different code.
    #[test]
    fn a_near_degenerate_spectrum_still_gives_an_orthonormal_basis() {
        // C = F^T F for F = diag(1.4, 1, 1), formed by multiplication so the
        // repeated eigenvalue is split by round-off exactly as it would be in a
        // real kinematics chain. UPDATED 2026-11-05 (gh:#1288): the stretch
        // is now along a direction 30° from x in the x-y plane. A diagonal C
        // takes upstream's effectively-diagonal branch since gh:#1288 and
        // would return the exact spectrum, so it no longer reaches the
        // split-root path this test exists for.
        let a = 1.4_f64;
        let (s, co) = (30.0_f64.to_radians().sin(), 30.0_f64.to_radians().cos());
        let c = SymmTensor::new(
            a * a * co * co + s * s,
            (a * a - 1.0) * co * s,
            0.0,
            a * a * s * s + co * co,
            0.0,
            1.0 * 1.0,
        );

        let l = eigen_values_symm(c);
        println!("near-degenerate rotated C: {:.17e} {:.17e} {:.17e}", l.x, l.y, l.z);
        let v = eigen_vectors_symm(c);
        let rows = [v.row_x(), v.row_y(), v.row_z()];

        for r in rows {
            assert!(
                (r.mag() - 1.0).abs() < 1e-9,
                "row not unit: |v| = {}",
                r.mag()
            );
        }
        for i in 0..3 {
            for j in (i + 1)..3 {
                let d = rows[i].dot(rows[j]);
                assert!(d.abs() < 1e-9, "rows {i},{j} not orthogonal: dot = {d}");
            }
        }

        // And each really is an eigenvector.
        for (i, r) in rows.iter().enumerate() {
            let lambda = [l.x, l.y, l.z][i];
            let residual = (c.mat_vec(*r) - *r * lambda).mag();
            // Bounded by the sqrt(eps) conditioning of a repeated root, not by
            // eigenvector quality -- see the doc comment.
            assert!(
                residual < 1e-7,
                "row {i}: T*v - lambda*v = {residual:e}, beyond the sqrt(eps) \
                 bound for a near-degenerate spectrum"
            );
        }
    }

    /// **A general non-symmetric tensor with a real spectrum.**
    ///
    /// *Methodology:* an upper-triangular tensor has its diagonal as its
    /// spectrum, and is genuinely non-symmetric so it exercises the general
    /// path rather than the symmetric one. Pass criterion: 1e-10 relative, no
    /// complex roots reported.
    ///
    /// *Result:* `(1, 2, 3)` to better than 1e-15 relative, complex flag false
    /// (measured 2026-08-04).
    #[test]
    fn general_tensor_with_real_spectrum() {
        let t = Tensor::new(1.0, 4.0, 5.0, 0.0, 2.0, 6.0, 0.0, 0.0, 3.0);
        let (l, complex) = eigen_values_checked(t);
        assert!(
            !complex,
            "upper-triangular real spectrum must not be complex"
        );
        assert_relative_eq!(l.x, 1.0, max_relative = 1e-10);
        assert_relative_eq!(l.y, 2.0, max_relative = 1e-10);
        assert_relative_eq!(l.z, 3.0, max_relative = 1e-10);
    }

    /// **A complex spectrum is reported, not silently zeroed.**
    ///
    /// *Methodology:* a 90-degree rotation about z has eigenvalues
    /// `{1, ±i}` — two of them complex. Upstream OpenFOAM warns and
    /// substitutes zero; a caller that cannot see the substitution would build
    /// a nonsense isotropic tensor function on it. Pass criterion: the flag is
    /// true.
    ///
    /// *Result:* flag true (measured 2026-08-04). This is the deviation from
    /// upstream that `eigen_values_checked` exists for: the numbers match
    /// upstream, the difference is that the caller can find out.
    #[test]
    fn complex_spectrum_is_reported() {
        let t = Tensor::new(0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0);
        let (_l, complex) = eigen_values_checked(t);
        assert!(complex, "a rotation has a complex spectrum and must say so");
    }

    /// **Diagonal tensors with repeated roots take upstream's diagonal
    /// branch (gh:#1288).**
    ///
    /// *Methodology:* `eigen_values_symm` on four diagonal tensors with a
    /// repeated or zero root, compared bit for bit (`assert_eq!` on `f64`)
    /// against the ascending diagonal. Reference: OpenFOAM ESI v2306
    /// `eigenValues(const symmTensor&)`, whose first branch returns the
    /// sorted diagonal when `xy² + xz² + yz² < ROOTSMALL`; OpenFOAM-dev's
    /// `cubicEqn::roots(true)` gives the same values for these four by its
    /// `d == 0` and real-clamp branches. Pass criterion: bit identity.
    ///
    /// *Prediction (written before the run):* every case exact, including
    /// `diag(4, 4, 0)`, which returned `(-1.68e-8, -0, 4)` before the fix
    /// (gh:#1275 D1, gh:#1288).
    ///
    /// *Result (2026-11-05):* all four bit-identical to the sorted diagonal;
    /// prediction met. Before the fix `diag(4, 4, 0)` gave
    /// `(-1.676e-8, -0, 4)`.
    #[test]
    fn diagonal_repeated_roots_take_upstreams_diagonal_branch() {
        for (d, want) in [
            ([4.0, 4.0, 0.0], [0.0, 4.0, 4.0]),
            ([2.0, 2.0, 5.0], [2.0, 2.0, 5.0]),
            ([0.0, 0.0, 3.0], [0.0, 0.0, 3.0]),
            ([3.0, -1.0, 3.0], [-1.0, 3.0, 3.0]),
        ] {
            let l = eigen_values_symm(SymmTensor::from_diag(d[0], d[1], d[2]));
            assert_eq!([l.x, l.y, l.z], want, "diag{d:?}");
        }
    }

    /// **Above the threshold the cubic path runs, and loses a root as
    /// upstream does (gh:#1288).**
    ///
    /// *Methodology:* `diag(4, 4, 0)` rotated 30° about x, which mixes the
    /// 4 and the 0 so `yz ≈ 1.73` and the diagonal branch must not fire.
    /// Upstream ESI v2306 sends it to `cubicEqn::roots()`. Pass criterion:
    /// the spectrum this port returns, pinned to 1e-12 absolute.
    ///
    /// *Prediction (written before the run, committed in 04bb3f76d):* the
    /// spectrum within `√ε` (1e-7) of `(0, 4, 4)`. **Missed.**
    ///
    /// *Result (2026-11-05):* `(-1.676e-8, -0, 4)`, the same as `diag(4, 4, 0)`
    /// gave before the fix: one of the two 4s is lost. Cause, traced by
    /// reading upstream: round-off leaves the discriminant slightly
    /// **positive**, above `VSMALL`, so ESI's `cubicEqn::roots()` takes
    /// `oneRealTwoComplex` and returns the pair `4 ± (-0)i` as
    /// `Roots<1>(complex, xRe), Roots<1>(complex, xIm)`. ESI's
    /// `eigenValues(const symmTensor&)` then stores `roots[i]` for a complex
    /// root (with a warning), so slot 2 holds the **imaginary part**. The
    /// port's cubic and this routine match ESI v2306 line for line, so
    /// upstream returns the same (by transcription; OpenFOAM was not run).
    /// It is an upstream defect, kept here as the specification.
    /// OpenFOAM-dev avoids it: its `eigenValues(const symmTensor&)` calls
    /// `cubicEqn::roots(true)`, which clamps a positive discriminant to zero
    /// for a known-real spectrum (`twoReal` branch). Porting that is a
    /// choice of upstream for the maintainer, not made here.
    #[test]
    fn a_rotated_repeated_root_goes_through_the_cubic_as_upstream() {
        let (s, c) = (30.0_f64.to_radians().sin(), 30.0_f64.to_radians().cos());
        // R = rotation about x by 30°; R diag(4, 4, 0) R^T written out.
        let st = SymmTensor::new(4.0, 0.0, 0.0, 4.0 * c * c, 4.0 * c * s, 4.0 * s * s);
        assert!(st.yz * st.yz >= ROOT_SMALL, "must not take the diagonal branch");
        let l = eigen_values_symm(st);
        println!("rotated diag(4,4,0): {:e} {:e} {:e}", l.x, l.y, l.z);
        assert!((l.x + 1.676_380_642_679_047_2e-8).abs() < 1e-12, "{l:?}");
        assert!(l.y.abs() < 1e-12 && (l.z - 4.0).abs() < 1e-12, "{l:?}");
    }

    /// **Upstream's threshold is absolute: a small shear on the identity is
    /// dropped (gh:#1288).**
    ///
    /// *Methodology:* `C = FᵀF` with `F = I + γ e_x⊗e_y`, `γ = 1e-5`, so
    /// `C.xy = γ` and `xy² = 1e-10 < ROOTSMALL`. Upstream ESI v2306 returns
    /// the diagonal `(1, 1, 1 + γ²)`, while the exact spectrum is
    /// `1 + γ²/2 ± γ√(1 + γ²/4)` and 1, i.e. about `1 ± 1e-5`. This pins
    /// that the port does what upstream does, so a caller relying on small
    /// shears of a near-identity tensor (finite-strain kinematics) is told
    /// by a test rather than by a wrong answer. Pass criterion: bit identity
    /// with the sorted diagonal.
    ///
    /// *Result (2026-11-05):* returns `(1, 1, 1 + 1e-10)`; the exact
    /// eigenvalues differ from that by about `1e-5`, the whole shear.
    #[test]
    fn a_small_shear_below_rootsmall_is_dropped_as_upstream_does() {
        let g = 1e-5_f64;
        let c = SymmTensor::new(1.0, g, 0.0, 1.0 + g * g, 0.0, 1.0);
        let l = eigen_values_symm(c);
        assert_eq!([l.x, l.y, l.z], [1.0, 1.0, 1.0 + g * g]);
    }
}
