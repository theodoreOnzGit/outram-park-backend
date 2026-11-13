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

//! Per-cell source terms of the transport models on a FROZEN field, in two
//! forms, for the code-to-code comparison of gh:#1362 row 5.
//!
//! Each model's `source_terms(form, state)` returns the quantities its
//! `correct()` assembles (G, the explicit source, the implicit `Sp` coefficient,
//! the `SuSp` coefficient, `nut`, the blending functions...) as named per-cell
//! vectors, without solving anything:
//!
//! * [`SourceForm::Port`] is the arithmetic the port's own `correct()` uses
//!   (the default of the crate; unchanged).
//! * [`SourceForm::EsiV2506`] is the OpenFOAM-v2506 (ESI) form, written from the
//!   upstream `kEpsilon.C`, `kOmega.C`, `kOmegaSSTBase.C`,
//!   `SpalartAllmarasBase.C`, `Smagorinsky.C` with the upstream operation order
//!   (so a frozen-field comparison can be bit for bit). It is an OPTION: nothing
//!   in `correct()` uses it, no default moves. Whether to make it the default is
//!   a maintainer decision recorded on gh:#1362.
//!
//! The gradients are INPUTS ([`FrozenState`]): the port's own
//! `fvc::grad` ([`FrozenState::grad_u`] built by the caller) or upstream's, so a
//! difference in a model formula is separated from a difference in the Gauss
//! gradient ulp ordering (gh:#1489).

use outram_foam_basic_lib::prelude::{Tensor, Vector3};

/// Which arithmetic a source-term evaluation uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceForm {
    /// The port's own `correct()` arithmetic (crate default).
    Port,
    /// OpenFOAM-v2506 (ESI) arithmetic and coefficients.
    EsiV2506,
}

/// Frozen-field gradients and wall distance a source-term evaluation needs.
///
/// Fields a model does not use may stay empty.
#[derive(Debug, Clone, Default)]
pub struct FrozenState {
    /// Cell-centred `grad(U)` (one tensor per cell, `T_ij = d_i U_j`).
    pub grad_u: Vec<Tensor>,
    /// `div(phi)` per cell (upstream `divU`, the incompressible `fvc::div(phi)`).
    pub div_u: Vec<f64>,
    /// `grad(k)` per cell (SST cross diffusion).
    pub grad_k: Vec<Vector3>,
    /// `grad(omega)` per cell (SST cross diffusion).
    pub grad_omega: Vec<Vector3>,
    /// `grad(nuTilda)` per cell (SA `Cb2/sigma |grad nuTilda|^2`).
    pub grad_nu_tilda: Vec<Vector3>,
    /// Wall distance override [m]; `None` keeps the model's own.
    pub wall_distance: Option<Vec<f64>>,
    /// LES filter width `delta` override [m] (Smagorinsky); `None` keeps the
    /// model's own cube-root-of-volume width.
    pub cell_length: Option<Vec<f64>>,
}

/// Named per-cell results of a source-term evaluation.
#[derive(Debug, Clone, Default)]
pub struct SourceTerms {
    /// Named per-cell vectors, in evaluation order.
    pub terms: Vec<(String, Vec<f64>)>,
}

impl SourceTerms {
    /// Append a named term.
    pub fn push(&mut self, name: &str, values: Vec<f64>) {
        self.terms.push((name.to_string(), values));
    }

    /// The term called `name`, if present.
    pub fn get(&self, name: &str) -> Option<&[f64]> {
        self.terms
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_slice())
    }
}

/// `tr(T) = xx + yy + zz` (upstream order).
pub fn tr(t: &Tensor) -> f64 {
    t.xx + t.yy + t.zz
}

/// Upstream `gradU && devTwoSymm(gradU)` (`TensorI.H` v2506: `devTwoSymm` then
/// the Tensor-SymmTensor double inner product, evaluated in upstream order).
pub fn g_by_nu(t: &Tensor) -> f64 {
    let sph = (2.0 / 3.0) * tr(t);
    let sxx = 2.0 * t.xx - sph;
    let sxy = t.xy + t.yx;
    let sxz = t.xz + t.zx;
    let syy = 2.0 * t.yy - sph;
    let syz = t.yz + t.zy;
    let szz = 2.0 * t.zz - sph;
    t.xx * sxx
        + t.xy * sxy
        + t.xz * sxz
        + t.yx * sxy
        + t.yy * syy
        + t.yz * syz
        + t.zx * sxz
        + t.zy * syz
        + t.zz * szz
}

/// Upstream `2*magSqr(symm(gradU))` (`magSqr(SymmTensor)` in upstream order).
pub fn two_mag_sqr_symm(t: &Tensor) -> f64 {
    let sxy = 0.5 * (t.xy + t.yx);
    let sxz = 0.5 * (t.xz + t.zx);
    let syz = 0.5 * (t.yz + t.zy);
    let ms = t.xx * t.xx
        + 2.0 * (sxy * sxy)
        + 2.0 * (sxz * sxz)
        + t.yy * t.yy
        + 2.0 * (syz * syz)
        + t.zz * t.zz;
    2.0 * ms
}

/// Upstream `mag(skew(gradU))` (`magSqr` of the nine components in row order).
pub fn mag_skew(t: &Tensor) -> f64 {
    let a = 0.5 * (t.xy - t.yx);
    let b = 0.5 * (t.xz - t.zx);
    let c = 0.5 * (t.yz - t.zy);
    // components xx, xy, xz, yx, yy, yz, zx, zy, zz = 0, a, b, -a, 0, c, -b, -c, 0
    let mut ms = 0.0_f64;
    ms += a * a;
    ms += b * b;
    ms += a * a;
    ms += c * c;
    ms += b * b;
    ms += c * c;
    ms.sqrt()
}

/// Upstream vector `a & b` (`x*x + y*y + z*z` order).
pub fn dot(a: Vector3, b: Vector3) -> f64 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

/// Upstream `magSqr(vector)`.
pub fn mag_sqr(a: Vector3) -> f64 {
    a.x * a.x + a.y * a.y + a.z * a.z
}

/// Upstream `max(a, b)` for scalars (`(a > b) ? a : b`).
pub fn umax(a: f64, b: f64) -> f64 {
    if a > b {
        a
    } else {
        b
    }
}

/// Upstream `min(a, b)` for scalars (`(a < b) ? a : b`).
pub fn umin(a: f64, b: f64) -> f64 {
    if a < b {
        a
    } else {
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Tensor {
        Tensor::new(1.0, 0.5, -0.25, 0.1, 2.0, 0.3, -0.4, 0.2, -3.0)
    }

    #[test]
    fn g_by_nu_is_two_s_double_inner_s_minus_two_thirds_trace_squared() {
        let t = sample();
        let s2 = two_mag_sqr_symm(&t);
        let want = s2 - (2.0 / 3.0) * tr(&t) * tr(&t);
        assert!((g_by_nu(&t) - want).abs() < 1e-13, "{} {}", g_by_nu(&t), want);
    }

    #[test]
    fn mag_skew_matches_frobenius_norm_of_antisymmetric_part() {
        let t = sample();
        let a = 0.5 * (t.xy - t.yx);
        let b = 0.5 * (t.xz - t.zx);
        let c = 0.5 * (t.yz - t.zy);
        let want = (2.0 * (a * a + b * b + c * c)).sqrt();
        assert!((mag_skew(&t) - want).abs() < 1e-14);
    }

    #[test]
    fn source_terms_lookup_and_scalar_helpers() {
        let mut s = SourceTerms::default();
        s.push("G", vec![1.0, 2.0]);
        assert_eq!(s.get("G"), Some(&[1.0, 2.0][..]));
        assert!(s.get("missing").is_none());
        assert_eq!(umax(1.0, 2.0), 2.0);
        assert_eq!(umin(1.0, 2.0), 1.0);
        let v = Vector3::new(1.0, 2.0, 3.0);
        assert_eq!(dot(v, v), 14.0);
        assert_eq!(mag_sqr(v), 14.0);
    }
}
