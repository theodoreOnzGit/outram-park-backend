// SPDX-License-Identifier: GPL-3.0
//
// RustBCA copy — provenance
// -------------------------
// Upstream project : RustBCA — https://github.com/lcpp-org/RustBCA
// Upstream version : v3.0.0, commit a356280d564bbdfbe74b97f32db0776444da58a5
// Upstream source  : src/math.rs
// Authors          : Jon Drobny <drobny2@illinois.edu>, Jon Drobny
//                    <jdrobny@tae.com> (upstream Cargo.toml); no copyright line upstream
// Original license : GNU GPL v3 (upstream LICENSE copied beside this file)
// Local changes    : std::mem::swap -> core::mem::swap
//                    (full list: module docs in mod.rs)

use super::*;
/// This function finds an orthonormal basis from a unit vector n
/// it should avoid all numerical / consistency issues
/// Duff et al., JCGT 2017
/// http://jcgt.org/published/0006/01/01/
#[inline]
pub fn duff_orthonormal_basis(n: Vector) -> (Vector, Vector) {
    let sign = (1.0_f64).copysign(n.z);
    let a = -1.0_f64 / (sign + n.z);
    let b = n.x*n.y*a;
    let b1 = Vector::new(1.0 + sign*n.x*n.x*a, sign*b, -sign*n.x);
    let b2 = Vector::new(b, sign + n.y*n.y*a, -n.y);
    (b1, b2)
}
#[inline]
pub fn triangular_index(i: &mut usize, j: &mut usize) -> usize {
    if i < j {
        core::mem::swap(i, j);
    }
    (*i*(*i + 1)/2) + *j
}