// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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

//! Small symmetric second- and fourth-order tensor algebra in Voigt form.
//!
//! # What belongs in this module
//!
//! [`Voigt6`] (a symmetric second-order tensor as six numbers), [`Tensor4`] (a
//! minor-symmetric fourth-order tensor as a 6x6 matrix), their algebra, the
//! stress invariants the J2 model needs, and conversions to and from
//! [`outram_foam_basic_lib::primitives::SymmTensor`].
//!
//! # What does NOT belong here
//!
//! Constitutive laws (see [`crate::material`]), shape-function gradients (see
//! [`crate::element`]), and anything mesh-aware. This module is pure algebra on
//! six numbers.
//!
//! # Reuse
//!
//! The general-purpose symmetric-tensor type in this workspace is
//! [`outram_foam_basic_lib::primitives::SymmTensor`] (trace, deviator, double
//! inner product, invariants, eigenvalues). It is *not* duplicated here.
//! [`Voigt6`] exists alongside it because finite elements need the **flat
//! six-vector with engineering shear strain**: that is the form a B-matrix
//! produces, and the form in which a fourth-order constitutive tensor is a
//! plain 6x6 matrix. [`Voigt6::from_stress_tensor`] and friends convert
//! between the two.
//!
//! # THE CONVENTION, stated once (read this before using the module)
//!
//! Component order is
//!
//! `[0] = xx, [1] = yy, [2] = zz, [3] = yz, [4] = xz, [5] = xy`.
//!
//! - A **stress** `Voigt6` holds the tensor components directly:
//!   `[sigma_xx, sigma_yy, sigma_zz, sigma_yz, sigma_xz, sigma_xy]`, all in
//!   pascals.
//! - A **strain** `Voigt6` holds **engineering** shear:
//!   `[eps_xx, eps_yy, eps_zz, gamma_yz, gamma_xz, gamma_xy]` with
//!   `gamma_ij = 2 eps_ij`, all dimensionless (metre per metre).
//!
//! This asymmetry is the standard finite-element convention and it is chosen
//! deliberately: it makes `sigma_V = D eps_V` hold with `D` the ordinary 6x6
//! elasticity matrix, and it makes the internal-force integrand `B^T sigma`
//! correct with no stray factors of two. The price is that the same type means
//! two slightly different things depending on what it holds, so **every
//! function here that cares says which it expects in its name or its doc**.
//! Functions that do not care (addition, scaling) are safe for both.
//!
//! # Units
//!
//! Stress-like `Voigt6` values are in pascals (newton per square metre).
//! Strain-like values are dimensionless. [`Tensor4`] entries used as an
//! elasticity or algorithmic tangent are in pascals. The public API of the
//! crate is `uom`-typed at its boundary; this module works in bare SI `f64`
//! because a fourth-order tensor with mixed physical roles has no single `uom`
//! type.

use outram_foam_basic_lib::primitives::SymmTensor;

/// Number of independent components of a symmetric second-order tensor in three
/// dimensions.
pub const VOIGT: usize = 6;

/// A symmetric second-order tensor stored as six numbers in Voigt order.
///
/// See the module documentation for the component order and the
/// stress-versus-strain convention — it matters, and it is not restated on
/// every method.
///
/// # Units
///
/// Pascals when it holds a stress; dimensionless when it holds a strain.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Voigt6(pub [f64; VOIGT]);

impl Voigt6 {
    /// The zero tensor.
    pub const ZERO: Voigt6 = Voigt6([0.0; VOIGT]);

    /// The second-order identity `delta_ij`, in **stress** form
    /// (`[1, 1, 1, 0, 0, 0]`).
    ///
    /// Read as a strain this is a unit isotropic extension, which is also
    /// correct, because the shear components are zero and so the engineering
    /// doubling does not bite.
    pub const IDENTITY: Voigt6 = Voigt6([1.0, 1.0, 1.0, 0.0, 0.0, 0.0]);

    /// Construct from the six components in Voigt order.
    ///
    /// Units: pascals for a stress, dimensionless for a strain (with
    /// engineering shear in the last three slots).
    #[must_use]
    pub const fn new(xx: f64, yy: f64, zz: f64, yz: f64, xz: f64, xy: f64) -> Self {
        Voigt6([xx, yy, zz, yz, xz, xy])
    }

    /// The raw six components, Voigt order.
    #[must_use]
    pub const fn as_array(&self) -> [f64; VOIGT] {
        self.0
    }

    /// Trace `A_xx + A_yy + A_zz`.
    ///
    /// For a stress this is `3 p_mean` \[Pa\]; for a strain it is the
    /// volumetric strain \[-\]. The shear slots do not enter, so the
    /// engineering convention is irrelevant here.
    #[must_use]
    pub fn trace(&self) -> f64 {
        self.0[0] + self.0[1] + self.0[2]
    }

    /// Deviatoric part of a **stress** tensor: `s = sigma - (tr sigma / 3) I`
    /// \[Pa\].
    ///
    /// Do not call this on an engineering-shear strain vector — use
    /// [`Voigt6::strain_deviator`], which halves the shears first.
    #[must_use]
    pub fn stress_deviator(&self) -> Voigt6 {
        let m = self.trace() / 3.0;
        Voigt6([
            self.0[0] - m,
            self.0[1] - m,
            self.0[2] - m,
            self.0[3],
            self.0[4],
            self.0[5],
        ])
    }

    /// Deviatoric part of an **engineering-shear strain** vector, returned in
    /// the same engineering convention \[-\].
    ///
    /// The shear slots are already deviatoric (a shear has no trace), so only
    /// the normal components change; the engineering factor passes through
    /// untouched.
    #[must_use]
    pub fn strain_deviator(&self) -> Voigt6 {
        let m = self.trace() / 3.0;
        Voigt6([
            self.0[0] - m,
            self.0[1] - m,
            self.0[2] - m,
            self.0[3],
            self.0[4],
            self.0[5],
        ])
    }

    /// Von Mises equivalent stress `q = sqrt(3/2 s:s)` \[Pa\], for a **stress**
    /// `Voigt6`.
    ///
    /// Zero for a purely hydrostatic state; equal to `|sigma_xx|` for uniaxial
    /// stress. This is the quantity compared against the yield stress in
    /// [`crate::material::J2LinearHardening`].
    #[must_use]
    pub fn von_mises(&self) -> f64 {
        let s = self.stress_deviator();
        (1.5 * s.stress_double_dot(&s)).sqrt()
    }

    /// Mean (hydrostatic) stress `tr(sigma) / 3` \[Pa\], tension positive.
    #[must_use]
    pub fn mean_stress(&self) -> f64 {
        self.trace() / 3.0
    }

    /// Double contraction `A : B` of two **stress-form** tensors \[Pa^2\].
    ///
    /// Shear components are counted twice because the tensor has both `ij` and
    /// `ji`: `A:B = sum_normal + 2 * sum_shear`.
    #[must_use]
    pub fn stress_double_dot(&self, other: &Voigt6) -> f64 {
        self.0[0] * other.0[0]
            + self.0[1] * other.0[1]
            + self.0[2] * other.0[2]
            + 2.0 * (self.0[3] * other.0[3] + self.0[4] * other.0[4] + self.0[5] * other.0[5])
    }

    /// Energy-conjugate product `sigma : eps` \[Pa\] = \[J/m^3\], where `self`
    /// is a **stress** and `strain` is an **engineering-shear strain**.
    ///
    /// The engineering doubling already supplies the factor of two on the
    /// shears, so this is the plain dot product of the six numbers — which is
    /// exactly why the finite-element convention is worth its awkwardness.
    #[must_use]
    pub fn work_with_strain(&self, strain: &Voigt6) -> f64 {
        (0..VOIGT).map(|i| self.0[i] * strain.0[i]).sum()
    }

    /// Frobenius norm of a **stress** tensor, `sqrt(sigma : sigma)` \[Pa\].
    #[must_use]
    pub fn stress_norm(&self) -> f64 {
        self.stress_double_dot(self).sqrt()
    }

    /// Convert from a [`SymmTensor`] holding a **stress** \[Pa\]: components
    /// map straight across.
    #[must_use]
    pub fn from_stress_tensor(t: SymmTensor) -> Self {
        Voigt6([t.xx, t.yy, t.zz, t.yz, t.xz, t.xy])
    }

    /// Convert to a [`SymmTensor`] holding a **stress** \[Pa\].
    #[must_use]
    pub fn to_stress_tensor(self) -> SymmTensor {
        SymmTensor::new(self.0[0], self.0[5], self.0[4], self.0[1], self.0[3], self.0[2])
    }

    /// Convert from a [`SymmTensor`] holding a **strain** \[-\], doubling the
    /// shear components into the engineering convention.
    #[must_use]
    pub fn from_strain_tensor(t: SymmTensor) -> Self {
        Voigt6([t.xx, t.yy, t.zz, 2.0 * t.yz, 2.0 * t.xz, 2.0 * t.xy])
    }

    /// Convert to a [`SymmTensor`] holding a **strain** \[-\], halving the
    /// engineering shears back to tensor components.
    #[must_use]
    pub fn to_strain_tensor(self) -> SymmTensor {
        SymmTensor::new(
            self.0[0],
            0.5 * self.0[5],
            0.5 * self.0[4],
            self.0[1],
            0.5 * self.0[3],
            self.0[2],
        )
    }

    /// Scale every component by `k` (dimensionless multiplier).
    #[must_use]
    pub fn scaled(&self, k: f64) -> Voigt6 {
        let mut o = self.0;
        for v in o.iter_mut() {
            *v *= k;
        }
        Voigt6(o)
    }

    /// Component-wise sum. Valid for two stresses or two strains, never a mix.
    #[must_use]
    pub fn plus(&self, other: &Voigt6) -> Voigt6 {
        let mut o = self.0;
        for i in 0..VOIGT {
            o[i] += other.0[i];
        }
        Voigt6(o)
    }

    /// Component-wise difference. Valid for two stresses or two strains, never
    /// a mix.
    #[must_use]
    pub fn minus(&self, other: &Voigt6) -> Voigt6 {
        let mut o = self.0;
        for i in 0..VOIGT {
            o[i] -= other.0[i];
        }
        Voigt6(o)
    }

    /// Largest absolute component \[same units as the tensor\].
    #[must_use]
    pub fn abs_max(&self) -> f64 {
        self.0.iter().fold(0.0_f64, |m, v| m.max(v.abs()))
    }
}

/// A minor-symmetric fourth-order tensor stored as a 6x6 Voigt matrix `D`, such
/// that `sigma_V = D eps_V` with `eps_V` in **engineering** shear form.
///
/// # Why this mapping has no stray factors of two
///
/// For a fourth-order tensor `c` with minor symmetries, `sigma_ij =
/// c_ijkl eps_kl`. Summing `kl` over both `(k,l)` and `(l,k)` contributes a
/// factor two on each shear pair, which the engineering definition
/// `gamma_kl = 2 eps_kl` exactly cancels. So `D[I][J] = c_ijkl` for the index
/// maps `I <-> ij`, `J <-> kl` with no scaling at all — as long as `eps_V` is
/// engineering and `sigma_V` is not. The consequence, which is the usual
/// tripwire, is that [`Tensor4::identity_symmetric`] has `1/2` (not `1`) in its
/// last three diagonal entries.
///
/// # Units
///
/// Pascals, when used as an elasticity or algorithmic tangent modulus.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tensor4(pub [[f64; VOIGT]; VOIGT]);

impl Default for Tensor4 {
    fn default() -> Self {
        Tensor4::ZERO
    }
}

impl Tensor4 {
    /// The zero fourth-order tensor.
    pub const ZERO: Tensor4 = Tensor4([[0.0; VOIGT]; VOIGT]);

    /// Entry `D[i][j]` \[Pa\] when used as a modulus.
    #[must_use]
    pub fn get(&self, i: usize, j: usize) -> f64 {
        self.0[i][j]
    }

    /// The symmetric fourth-order identity `I_sym`, whose action is
    /// `I_sym : eps = eps`.
    ///
    /// In this Voigt convention it is `diag(1, 1, 1, 1/2, 1/2, 1/2)` — see the
    /// type documentation for why the halves are there and are not a bug.
    #[must_use]
    pub fn identity_symmetric() -> Tensor4 {
        let mut d = [[0.0; VOIGT]; VOIGT];
        for i in 0..3 {
            d[i][i] = 1.0;
        }
        for i in 3..VOIGT {
            d[i][i] = 0.5;
        }
        Tensor4(d)
    }

    /// The deviatoric projector `I_dev = I_sym - (1/3) I (x) I`, whose action
    /// on a strain returns its deviator.
    #[must_use]
    pub fn deviatoric_projector() -> Tensor4 {
        let mut d = Tensor4::identity_symmetric();
        for i in 0..3 {
            for j in 0..3 {
                d.0[i][j] -= 1.0 / 3.0;
            }
        }
        d
    }

    /// The dyadic (outer) product `a (x) b`, i.e. `D[I][J] = a[I] b[J]`, with
    /// both `a` and `b` in **stress** (tensor-component) form.
    ///
    /// Units: the product of `a`'s and `b`'s units.
    #[must_use]
    pub fn outer(a: &Voigt6, b: &Voigt6) -> Tensor4 {
        let mut d = [[0.0; VOIGT]; VOIGT];
        for i in 0..VOIGT {
            for j in 0..VOIGT {
                d[i][j] = a.0[i] * b.0[j];
            }
        }
        Tensor4(d)
    }

    /// Isotropic linear-elastic stiffness `C = K I(x)I + 2 mu I_dev` \[Pa\].
    ///
    /// # Arguments
    ///
    /// - `bulk_modulus` — `K` \[Pa\], strictly positive.
    /// - `shear_modulus` — `mu` \[Pa\], strictly positive.
    ///
    /// The familiar Lame form follows: `D[0][0] = K + 4 mu / 3 = lambda + 2 mu`,
    /// `D[0][1] = K - 2 mu / 3 = lambda`, `D[3][3] = mu`.
    #[must_use]
    pub fn isotropic(bulk_modulus: f64, shear_modulus: f64) -> Tensor4 {
        let mut d = Tensor4::deviatoric_projector().scaled(2.0 * shear_modulus);
        for i in 0..3 {
            for j in 0..3 {
                d.0[i][j] += bulk_modulus;
            }
        }
        d
    }

    /// Scale every entry by `k` (dimensionless).
    #[must_use]
    pub fn scaled(&self, k: f64) -> Tensor4 {
        let mut d = self.0;
        for row in d.iter_mut() {
            for v in row.iter_mut() {
                *v *= k;
            }
        }
        Tensor4(d)
    }

    /// Entry-wise sum.
    #[must_use]
    pub fn plus(&self, other: &Tensor4) -> Tensor4 {
        let mut d = self.0;
        for i in 0..VOIGT {
            for j in 0..VOIGT {
                d[i][j] += other.0[i][j];
            }
        }
        Tensor4(d)
    }

    /// Apply the tensor to an **engineering-shear strain** vector, returning a
    /// **stress** vector: `sigma_V = D eps_V` \[Pa\].
    #[must_use]
    pub fn apply(&self, strain: &Voigt6) -> Voigt6 {
        let mut out = [0.0; VOIGT];
        for i in 0..VOIGT {
            let mut s = 0.0;
            for j in 0..VOIGT {
                s += self.0[i][j] * strain.0[j];
            }
            out[i] = s;
        }
        Voigt6(out)
    }

    /// Largest absolute entry-wise difference from `other` \[Pa\]. Used by the
    /// tangent-verification tests.
    #[must_use]
    pub fn max_abs_diff(&self, other: &Tensor4) -> f64 {
        let mut m = 0.0_f64;
        for i in 0..VOIGT {
            for j in 0..VOIGT {
                m = m.max((self.0[i][j] - other.0[i][j]).abs());
            }
        }
        m
    }

    /// Largest absolute entry \[Pa\].
    #[must_use]
    pub fn abs_max(&self) -> f64 {
        let mut m = 0.0_f64;
        for row in &self.0 {
            for v in row {
                m = m.max(v.abs());
            }
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The isotropic stiffness must reproduce the Lame constants exactly.
    #[test]
    fn isotropic_matches_lame() {
        let (e, nu) = (200.0e9, 0.3);
        let mu = e / (2.0 * (1.0 + nu));
        let lambda = e * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
        let k = lambda + 2.0 * mu / 3.0;
        let d = Tensor4::isotropic(k, mu);
        assert!((d.get(0, 0) - (lambda + 2.0 * mu)).abs() < 1e-3);
        assert!((d.get(0, 1) - lambda).abs() < 1e-3);
        assert!((d.get(3, 3) - mu).abs() < 1e-3);
        assert!(d.get(0, 3).abs() < 1e-12);
    }

    /// Uniaxial strain through the isotropic tensor gives the textbook answer,
    /// and pure shear gives `tau = mu gamma`.
    #[test]
    fn isotropic_action_on_strain() {
        let (e, nu) = (210.0e9, 0.3);
        let mu = e / (2.0 * (1.0 + nu));
        let lambda = e * nu / ((1.0 + nu) * (1.0 - 2.0 * nu));
        let d = Tensor4::isotropic(lambda + 2.0 * mu / 3.0, mu);

        let eps = Voigt6::new(1.0e-3, 0.0, 0.0, 0.0, 0.0, 0.0);
        let s = d.apply(&eps);
        assert!((s.0[0] - (lambda + 2.0 * mu) * 1.0e-3).abs() < 1.0);
        assert!((s.0[1] - lambda * 1.0e-3).abs() < 1.0);

        let gam = Voigt6::new(0.0, 0.0, 0.0, 0.0, 0.0, 2.0e-3);
        let t = d.apply(&gam);
        assert!((t.0[5] - mu * 2.0e-3).abs() < 1.0);
    }

    /// `I_sym` must be the identity on engineering strain, and `I_dev` must
    /// project out the trace.
    #[test]
    fn projectors_behave() {
        let eps = Voigt6::new(1.0, 2.0, 3.0, 0.4, 0.5, 0.6);
        let id = Tensor4::identity_symmetric().apply(&eps);
        // I_sym : eps is the *tensor* form of eps, so shears are halved.
        for i in 0..3 {
            assert!((id.0[i] - eps.0[i]).abs() < 1e-14);
        }
        for i in 3..6 {
            assert!((id.0[i] - 0.5 * eps.0[i]).abs() < 1e-14);
        }
        let dev = Tensor4::deviatoric_projector().apply(&eps);
        assert!(dev.trace().abs() < 1e-13, "trace {}", dev.trace());
    }

    /// Von Mises of a uniaxial stress is its magnitude; of a hydrostatic state,
    /// zero.
    #[test]
    fn von_mises_limits() {
        let uni = Voigt6::new(250.0e6, 0.0, 0.0, 0.0, 0.0, 0.0);
        assert!((uni.von_mises() - 250.0e6).abs() < 1e-3);
        let hyd = Voigt6::new(1.0e6, 1.0e6, 1.0e6, 0.0, 0.0, 0.0);
        assert!(hyd.von_mises() < 1e-6);
        // Pure shear tau: q = sqrt(3) tau.
        let sh = Voigt6::new(0.0, 0.0, 0.0, 0.0, 0.0, 100.0e6);
        assert!((sh.von_mises() - 3.0_f64.sqrt() * 100.0e6).abs() < 1e-2);
    }

    /// Round-tripping through `SymmTensor` must preserve both conventions.
    #[test]
    fn symm_tensor_round_trip() {
        let s = Voigt6::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
        assert_eq!(Voigt6::from_stress_tensor(s.to_stress_tensor()), s);
        assert_eq!(Voigt6::from_strain_tensor(s.to_strain_tensor()), s);
        // Deviator agrees with the shared SymmTensor implementation.
        let dev_here = s.stress_deviator().to_stress_tensor();
        let dev_shared = s.to_stress_tensor().dev();
        assert!((dev_here.xx - dev_shared.xx).abs() < 1e-14);
        assert!((dev_here.xy - dev_shared.xy).abs() < 1e-14);
    }

    /// `sigma : eps` must equal the plain dot product in this convention, and
    /// agree with the tensor-form double contraction.
    #[test]
    fn work_conjugacy() {
        let sig = Voigt6::new(10.0, 20.0, 30.0, 4.0, 5.0, 6.0);
        let eps = Voigt6::new(1e-3, 2e-3, 3e-3, 8e-4, 1e-3, 1.2e-3);
        let via_dot = sig.work_with_strain(&eps);
        let via_tensor = sig.to_stress_tensor().double_inner(eps.to_strain_tensor());
        assert!((via_dot - via_tensor).abs() < 1e-12 * via_dot.abs().max(1.0));
    }
}
