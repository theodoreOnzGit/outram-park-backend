// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Kay Chen Ong (OUTRAM PARK workspace)
//
// Ported from the GNU Octave control package ("LTI Syncope"),
// https://github.com/gnu-octave/pkg-control, control 4.2.2+,
// commit f39e2625bffc56864c3af746b4410bb71ce6bbb1:
//   inst/@lti/c2d.m, inst/@lti/d2c.m   (method dispatch and semantics)
//   inst/@tf/__c2d__.m                 (matched pole/zero branch)
//   inst/@tf/__d2c__.m                 (matched pole/zero branch)
//   inst/@ss/__c2d__.m                 (zoh / tustin / prewarp semantics)
//   inst/@ss/__d2c__.m                 (tustin / prewarp semantics)
// Upstream copyright: (C) 2009-2016 Lukas F. Reichlin; inst/@ss/__c2d__.m
// additionally (C) Torsten Lilge. All of the above upstream files are
// GPL-3.0-or-later — the GPLv3 side of the mixed-licence package.
//
// NOT ported: the SLICOT kernels the upstream state-space paths call
// (__sl_mb05nd__ matrix exponential, __sl_ab04md__ bilinear state-space
// transformation), which are the BSD-3-Clause side of the package. The
// zero-order-hold discretisation here is an independent closed-form
// implementation for system order <= 2 (spectral decomposition of the
// companion matrix), and the bilinear/Tustin substitution is carried out
// directly on the transfer-function polynomials. No BSD-3 material is
// reproduced in this file.
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, version 3 of the License.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Continuous <-> discrete transfer-function conversion (`c2d` / `d2c`).
//!
//! # Methods and their mathematics
//!
//! - **Zero-order hold** ([`C2dMethod::Zoh`]) — the step-invariant
//!   transformation: exact at the sample instants whenever the input is
//!   held constant over each sample interval, which is precisely what a
//!   time-stepping simulator produces. This is the same mathematics as the
//!   O(1) recurrences in `stable_transfer_functions` (see that module's
//!   docs and bead `op-fm5`); here it is computed for a general SISO
//!   transfer function of order <= 2 via the companion-form state-space
//!   `x' = A x + B u`, `Phi = exp(A T)`,
//!   `Gamma = (integral_0^T exp(A s) ds) B`.
//! - **Tustin / bilinear** ([`C2dMethod::Tustin`]) — the substitution
//!   `s = (2/T) (z - 1)/(z + 1)` (the trapezoidal integration rule),
//!   applied directly to the numerator and denominator polynomials. Works
//!   for any order.
//! - **Tustin with prewarping** ([`C2dMethod::TustinPrewarp`]) — bilinear
//!   with `beta = w0 / tan(w0 T / 2)` in place of `2/T`, so the frequency
//!   response is exact at the angular frequency `w0` (upstream
//!   `inst/@ss/__c2d__.m` uses the identical `beta`).
//! - **Matched pole/zero** ([`C2dMethod::MatchedPoleZero`]) — maps every
//!   pole and finite zero through `z = exp(s T)`, fills the excess zeros at
//!   `z = -1` (all but one), and matches the gain at DC (or, if a pole or
//!   zero sits on the imaginary axis near DC, at the first clear frequency)
//!   — a direct port of the matched branch of `inst/@tf/__c2d__.m`.
//!
//! `d2c` supports `Tustin`, `TustinPrewarp` (the inverse substitution
//! `z = (beta + s)/(beta - s)`) and `MatchedPoleZero` (`s = ln(z)/T`).
//! **`d2c` by zero-order hold is deliberately not ported**: upstream
//! computes it with a matrix logarithm (`logm`), a general dense-matrix
//! algorithm out of scope here — tracked as a follow-up bead rather than
//! half-implemented. The same applies to upstream's `foh` (first-order
//! hold) and `impulse` invariant methods on the `c2d` side.
//!
//! # References
//!
//! The zero-order-hold mathematics is the same textbook material cited in
//! `stable_transfer_functions/first_order_transfer_fn.rs` (Astrom &
//! Wittenmark; Seborg, Edgar, Mellichamp & Doyle; Franklin, Powell &
//! Workman; Ogata; Oppenheim & Schafer; Smith). As recorded there and in
//! bead `op-ia5j`, **those citations are unverified against physical
//! copies — no edition, year or page number is given, and none should be
//! added without checking.**

use uom::si::{angular_velocity::radian_per_second, f64::*, time::second};

use super::continuous_tf::ContinuousTransferFn;
use super::cplx::Cplx;
use super::discrete_tf::DiscreteTransferFn;
use super::polynomial;
use super::ZDomainError;

/// Continuous-to-discrete conversion method (`c2d`).
///
/// Enum dispatch per the workspace Rust design rules (no trait objects).
/// Upstream Octave method strings map as: `"zoh"`/`"std"` -> [`Self::Zoh`],
/// `"tustin"`/`"bilin"` -> [`Self::Tustin`], `"prewarp"` ->
/// [`Self::TustinPrewarp`], `"matched"` -> [`Self::MatchedPoleZero`].
/// Upstream's `"foh"` and `"impulse"` are not ported (see module docs).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum C2dMethod {
    /// Zero-order hold (step-invariant): exact at sample instants for
    /// piecewise-constant input. Requires a proper transfer function of
    /// order <= 2.
    Zoh,
    /// Bilinear (trapezoidal) transformation, `s = (2/T)(z-1)/(z+1)`.
    /// Any order.
    Tustin,
    /// Bilinear transformation with frequency prewarping: the discrete
    /// frequency response is exact at `prewarp_frequency`. Any order.
    TustinPrewarp {
        /// Angular frequency `w0` at which the response is matched, in
        /// rad/s (`uom` `AngularVelocity`). Must satisfy `0 < w0 < pi/T`.
        prewarp_frequency: AngularVelocity,
    },
    /// Matched pole/zero method (`z = exp(s T)` on poles and finite zeros,
    /// excess zeros at `z = -1`, gain matched at DC). Requires order <= 2.
    MatchedPoleZero,
}

/// Discrete-to-continuous conversion method (`d2c`).
///
/// Upstream's `"zoh"` inverse needs a matrix logarithm and is deliberately
/// absent (see module docs and the follow-up bead).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum D2cMethod {
    /// Inverse bilinear transformation, `z = (beta + s)/(beta - s)` with
    /// `beta = 2/T`. Any order.
    Tustin,
    /// Inverse bilinear transformation with prewarping at
    /// `prewarp_frequency` (rad/s); `beta = w0 / tan(w0 T / 2)`.
    TustinPrewarp {
        /// Angular frequency `w0` in rad/s; must satisfy `0 < w0 < pi/T`.
        prewarp_frequency: AngularVelocity,
    },
    /// Matched pole/zero method, `s = ln(z)/T`. Requires order <= 2 and no
    /// pole or zero at `z = 0`.
    MatchedPoleZero,
}

impl ContinuousTransferFn {
    /// Converts this continuous-time transfer function into its
    /// discrete-time equivalent with sample time `sample_time` — the
    /// Octave `c2d` function.
    ///
    /// `sample_time` is a physical time in seconds (strictly positive).
    /// See [`C2dMethod`] for the discretisation methods and their
    /// applicability; [`C2dMethod::Zoh`] and
    /// [`C2dMethod::MatchedPoleZero`] are limited to order <= 2 (a
    /// closed-form eigenvalue computation), while the Tustin variants work
    /// for any order.
    ///
    /// # Errors
    ///
    /// [`ZDomainError::NonPositiveSampleTime`],
    /// [`ZDomainError::UnsupportedOrder`],
    /// [`ZDomainError::ImproperTransferFunction`] (Zoh of a non-proper
    /// system), [`ZDomainError::InvalidPrewarpFrequency`], or
    /// [`ZDomainError::NonFinitePoleOrZero`] (matched method overflow).
    pub fn to_discrete(
        &self,
        sample_time: Time,
        method: C2dMethod,
    ) -> Result<DiscreteTransferFn, ZDomainError> {
        let t_s = sample_time.get::<second>();
        if t_s <= 0.0 {
            return Err(ZDomainError::NonPositiveSampleTime);
        }
        match method {
            C2dMethod::Zoh => c2d_zoh(self, sample_time),
            C2dMethod::Tustin => c2d_bilinear(self, sample_time, 2.0 / t_s),
            C2dMethod::TustinPrewarp { prewarp_frequency } => {
                let beta = prewarp_beta(prewarp_frequency, t_s)?;
                c2d_bilinear(self, sample_time, beta)
            }
            C2dMethod::MatchedPoleZero => c2d_matched(self, sample_time),
        }
    }
}

impl DiscreteTransferFn {
    /// Converts this discrete-time transfer function back into a
    /// continuous-time equivalent — the Octave `d2c` function.
    ///
    /// See [`D2cMethod`]; the zero-order-hold inverse (matrix logarithm)
    /// is deliberately not available.
    ///
    /// # Errors
    ///
    /// [`ZDomainError::UnsupportedOrder`] (matched with order > 2),
    /// [`ZDomainError::MatchedPoleZeroAtOrigin`],
    /// [`ZDomainError::InvalidPrewarpFrequency`], or
    /// [`ZDomainError::ZeroDenominator`] if the inverse map degenerates.
    pub fn to_continuous(&self, method: D2cMethod) -> Result<ContinuousTransferFn, ZDomainError> {
        let t_s = self.sample_time().get::<second>();
        match method {
            D2cMethod::Tustin => d2c_bilinear(self, 2.0 / t_s),
            D2cMethod::TustinPrewarp { prewarp_frequency } => {
                let beta = prewarp_beta(prewarp_frequency, t_s)?;
                d2c_bilinear(self, beta)
            }
            D2cMethod::MatchedPoleZero => d2c_matched(self),
        }
    }
}

/// Computes the bilinear coefficient `beta = w0 / tan(w0 T / 2)` for
/// prewarping (identical formula to upstream `inst/@ss/__c2d__.m`).
/// Requires `0 < w0 < pi/T` so the tangent is finite and positive.
fn prewarp_beta(w0: AngularVelocity, t_s: f64) -> Result<f64, ZDomainError> {
    let w0 = w0.get::<radian_per_second>();
    let half_angle = w0 * t_s / 2.0;
    if !(w0 > 0.0) || half_angle >= std::f64::consts::FRAC_PI_2 {
        return Err(ZDomainError::InvalidPrewarpFrequency);
    }
    Ok(w0 / half_angle.tan())
}

// ---------------------------------------------------------------------------
// Tustin / bilinear — polynomial substitution, any order
// ---------------------------------------------------------------------------

/// `c2d` by bilinear substitution `s = beta (z - 1)/(z + 1)`:
/// with `n = max(deg num, deg den)`, each coefficient `c_k` of `s^k`
/// becomes `c_k beta^k (z-1)^k (z+1)^{n-k}` after clearing `(z+1)^n`.
fn c2d_bilinear(
    sys: &ContinuousTransferFn,
    sample_time: Time,
    beta: f64,
) -> Result<DiscreteTransferFn, ZDomainError> {
    let n = sys.order();
    let num_z = bilinear_substitute(sys.numerator_ascending_s(), n, beta, 1.0, -1.0);
    let den_z = bilinear_substitute(sys.denominator_ascending_s(), n, beta, 1.0, -1.0);
    // ascending-z -> descending-z is a reverse
    let num_desc: Vec<f64> = num_z.into_iter().rev().collect();
    let den_desc: Vec<f64> = den_z.into_iter().rev().collect();
    DiscreteTransferFn::from_z_descending_coefficients(num_desc, den_desc, sample_time)
}

/// `d2c` by the inverse substitution `z = (beta + s)/(beta - s)`:
/// with `n` the denominator degree in `z`, each coefficient `c_k` of `z^k`
/// becomes `c_k (beta + s)^k (beta - s)^{n-k}` after clearing
/// `(beta - s)^n`.
fn d2c_bilinear(
    sys: &DiscreteTransferFn,
    beta: f64,
) -> Result<ContinuousTransferFn, ZDomainError> {
    // z^-1-ascending coefficients of length L describe descending powers of
    // z of degree L-1 when multiplied through by z^(L-1); ascending-z is
    // the reverse of the z^-1 list padded to the denominator length.
    let len = sys
        .numerator_z_inverse()
        .len()
        .max(sys.denominator_z_inverse().len());
    let mut num_zinv = sys.numerator_z_inverse().to_vec();
    let mut den_zinv = sys.denominator_z_inverse().to_vec();
    num_zinv.resize(len, 0.0);
    den_zinv.resize(len, 0.0);
    let num_asc_z: Vec<f64> = num_zinv.into_iter().rev().collect();
    let den_asc_z: Vec<f64> = den_zinv.into_iter().rev().collect();

    let n = len - 1;
    // (beta + s)^k (beta - s)^{n-k}: reuse the same helper with the roles
    // of the two linear factors swapped relative to c2d.
    let num_s = bilinear_substitute_z(&num_asc_z, n, beta);
    let den_s = bilinear_substitute_z(&den_asc_z, n, beta);
    ContinuousTransferFn::new(num_s, den_s)
}

/// Expands `sum_k c_k beta^k (z + p)^k (z + m)^{n-k}` (ascending-z output).
/// For c2d Tustin, `p = -1` (factor `z - 1`) and `m = +1` (factor `z + 1`):
/// pass `plus = 1.0, minus = -1.0` meaning the constant terms of the two
/// linear factors are `minus` and `plus` respectively.
fn bilinear_substitute(coeffs: &[f64], n: usize, beta: f64, plus: f64, minus: f64) -> Vec<f64> {
    // factor_a = (z - 1) as [minus, 1]; factor_b = (z + 1) as [plus, 1]
    let factor_a = [minus, 1.0];
    let factor_b = [plus, 1.0];
    let mut acc: Vec<f64> = Vec::new();
    let mut beta_k = 1.0;
    for (k, &c) in coeffs.iter().enumerate() {
        let term = polynomial::mul(
            &polynomial::pow(&factor_a, k),
            &polynomial::pow(&factor_b, n - k),
        );
        acc = polynomial::add(&acc, &polynomial::scale(&term, c * beta_k));
        beta_k *= beta;
    }
    acc
}

/// Expands `sum_k c_k (beta + s)^k (beta - s)^{n-k}` (ascending-s output),
/// the inverse-bilinear analogue of [`bilinear_substitute`].
fn bilinear_substitute_z(coeffs: &[f64], n: usize, beta: f64) -> Vec<f64> {
    let plus_s = [beta, 1.0]; // beta + s
    let minus_s = [beta, -1.0]; // beta - s
    let mut acc: Vec<f64> = Vec::new();
    for (k, &c) in coeffs.iter().enumerate() {
        let term = polynomial::mul(
            &polynomial::pow(&plus_s, k),
            &polynomial::pow(&minus_s, n - k),
        );
        acc = polynomial::add(&acc, &polynomial::scale(&term, c));
    }
    acc
}

// ---------------------------------------------------------------------------
// Zero-order hold — closed-form state-space discretisation, order <= 2
// ---------------------------------------------------------------------------

/// `c2d` by zero-order hold for a proper SISO transfer function of order
/// <= 2, via the controllable-canonical (companion) state-space form and a
/// closed-form matrix exponential (spectral decomposition of the 2x2
/// companion matrix). Independent implementation — upstream reaches this
/// via the BSD-3 SLICOT `__sl_mb05nd__`, which is NOT ported.
fn c2d_zoh(
    sys: &ContinuousTransferFn,
    sample_time: Time,
) -> Result<DiscreteTransferFn, ZDomainError> {
    let t_s = sample_time.get::<second>();
    let num = sys.numerator_ascending_s();
    let den = sys.denominator_ascending_s();
    let deg_num = polynomial::degree(num);
    let deg_den = polynomial::degree(den);
    if !num.is_empty() && deg_num > deg_den {
        return Err(ZDomainError::ImproperTransferFunction);
    }
    let order = sys.order();
    if order > 2 {
        return Err(ZDomainError::UnsupportedOrder { order });
    }

    // Static gain: y = g u exactly, hold or no hold.
    if order == 0 {
        let g = sys.steady_state_gain();
        return DiscreteTransferFn::from_z_inverse_coefficients(vec![g], vec![1.0], sample_time);
    }

    // Monic denominator: divide num and den by the leading den coefficient.
    let lead = den[deg_den];
    let den_m = polynomial::scale(den, 1.0 / lead);
    let mut num_m = polynomial::scale(num, 1.0 / lead);
    num_m.resize(deg_den + 1, 0.0); // pad with zeros up to b_{deg_den}

    if order == 1 {
        // G(s) = (b1 s + b0)/(s + a0) = b1 + (b0 - b1 a0)/(s + a0)
        let a0 = den_m[0];
        let b0 = num_m[0];
        let b1 = num_m[1];
        let lambda = -a0;
        let phi = (lambda * t_s).exp();
        let gamma = phi1_real(lambda, t_s);
        let c = b0 - b1 * a0;
        let d = b1;
        // H_d(z) = c*gamma/(z - phi) + d
        let num_desc = vec![d, c * gamma - d * phi];
        let den_desc = vec![1.0, -phi];
        return DiscreteTransferFn::from_z_descending_coefficients(
            num_desc, den_desc, sample_time,
        );
    }

    // order == 2: companion form
    //   A = [[0, 1], [-a0, -a1]],  B = [0, 1]^T,
    //   C = [b0 - b2 a0, b1 - b2 a1],  D = b2
    let a0 = den_m[0];
    let a1 = den_m[1];
    let b2 = num_m[2];
    let c_vec = [num_m[0] - b2 * a0, num_m[1] - b2 * a1];
    let d = b2;
    let a_mat = [[0.0, 1.0], [-a0, -a1]];

    // Eigenvalues: roots of s^2 + a1 s + a0.
    let roots = polynomial::roots_deg_le_2(&[a0, a1, 1.0])
        .expect("degree 2 by construction");
    let (l1, l2) = (roots[0], roots[1]);

    // Distinct vs (near-)repeated eigenvalues. Near-repeated pairs are
    // collapsed to their mean to avoid catastrophic cancellation in the
    // spectral formula; the threshold is documented as an approximation.
    let scale_ref = l1.abs().max(l2.abs()).max(1.0 / t_s);
    let (phi_mat, gamma_col) = if (l1 - l2).abs() <= 1e-7 * scale_ref {
        let lambda = (l1 + l2) * 0.5;
        zoh_repeated(a_mat, lambda, t_s)
    } else {
        zoh_distinct(a_mat, l1, l2, t_s)
    };

    // Discrete transfer function from (Phi, Gamma, C, D):
    //   den_d(z) = z^2 - tr(Phi) z + det(Phi)
    //   C (zI - Phi)^{-1} Gamma
    //     = [ (C.Gamma) z + c1 (p12 g2 - p22 g1) + c2 (p21 g1 - p11 g2) ]
    //       / den_d(z)
    let (p11, p12, p21, p22) = (phi_mat[0][0], phi_mat[0][1], phi_mat[1][0], phi_mat[1][1]);
    let (g1, g2) = (gamma_col[0], gamma_col[1]);
    let tr = p11 + p22;
    let det = p11 * p22 - p12 * p21;
    let cg = c_vec[0] * g1 + c_vec[1] * g2;
    let q = c_vec[0] * (p12 * g2 - p22 * g1) + c_vec[1] * (p21 * g1 - p11 * g2);
    let num_desc = vec![d, cg - d * tr, q + d * det];
    let den_desc = vec![1.0, -tr, det];
    DiscreteTransferFn::from_z_descending_coefficients(num_desc, den_desc, sample_time)
}

/// `phi1(lambda, T) = (exp(lambda T) - 1)/lambda`, i.e.
/// `integral_0^T exp(lambda s) ds`, with a series fallback near
/// `lambda = 0` (covers integrating systems without a matrix inverse).
fn phi1_real(lambda: f64, t_s: f64) -> f64 {
    let x = lambda * t_s;
    if x.abs() < 1e-8 {
        t_s * (1.0 + x / 2.0 + x * x / 6.0)
    } else {
        ((x).exp() - 1.0) / lambda
    }
}

/// Complex counterpart of [`phi1_real`].
fn phi1_cplx(lambda: Cplx, t_s: f64) -> Cplx {
    let x = lambda * t_s;
    if x.abs() < 1e-8 {
        Cplx::real(t_s) * (Cplx::real(1.0) + x * 0.5 + x * x * (1.0 / 6.0))
    } else {
        ((x).exp() - Cplx::real(1.0)) / lambda
    }
}

/// `psi(lambda, T) = integral_0^T s exp(lambda s) ds
///  = (T exp(lambda T) - phi1(lambda, T))/lambda`, with the `lambda -> 0`
/// limit `T^2/2` handled by series. Used by the repeated-eigenvalue branch.
fn psi_real(lambda: f64, t_s: f64) -> f64 {
    let x = lambda * t_s;
    if x.abs() < 1e-8 {
        t_s * t_s * (0.5 + x / 3.0 + x * x / 8.0)
    } else {
        (t_s * x.exp() - phi1_real(lambda, t_s)) / lambda
    }
}

/// `Phi = exp(A T)` and `Gamma = (integral exp(A s) ds) B` for a real 2x2
/// `A` with distinct eigenvalues `l1 != l2` (possibly a conjugate pair),
/// via the spectral formula
/// `f(A) = [f(l1)(A - l2 I) - f(l2)(A - l1 I)] / (l1 - l2)`;
/// `B = [0, 1]^T` so `Gamma` is the second column. Imaginary parts cancel
/// for conjugate pairs and are discarded.
fn zoh_distinct(a: [[f64; 2]; 2], l1: Cplx, l2: Cplx, t_s: f64) -> ([[f64; 2]; 2], [f64; 2]) {
    let dl = l1 - l2;
    // entry-wise: m1 = A - l2 I, m2 = A - l1 I (complex 2x2)
    let m = |lam: Cplx| -> [[Cplx; 2]; 2] {
        [
            [Cplx::real(a[0][0]) - lam, Cplx::real(a[0][1])],
            [Cplx::real(a[1][0]), Cplx::real(a[1][1]) - lam],
        ]
    };
    let m1 = m(l2);
    let m2 = m(l1);
    let e1 = (l1 * t_s).exp();
    let e2 = (l2 * t_s).exp();
    let f1 = phi1_cplx(l1, t_s);
    let f2 = phi1_cplx(l2, t_s);

    let mut phi = [[0.0; 2]; 2];
    let mut gamma_mat = [[0.0; 2]; 2];
    for i in 0..2 {
        for j in 0..2 {
            phi[i][j] = ((e1 * m1[i][j] - e2 * m2[i][j]) / dl).re;
            gamma_mat[i][j] = ((f1 * m1[i][j] - f2 * m2[i][j]) / dl).re;
        }
    }
    // Gamma = gamma_mat * B with B = [0, 1]^T -> second column
    (phi, [gamma_mat[0][1], gamma_mat[1][1]])
}

/// [`zoh_distinct`] for a repeated real eigenvalue `lambda`:
/// `exp(A T) = exp(lambda T) (I + T (A - lambda I))` and
/// `integral exp(A s) ds = phi1(lambda) I + psi(lambda) (A - lambda I)`.
fn zoh_repeated(a: [[f64; 2]; 2], lambda: Cplx, t_s: f64) -> ([[f64; 2]; 2], [f64; 2]) {
    // a repeated root of a real quadratic is real
    let l = lambda.re;
    let n = [[a[0][0] - l, a[0][1]], [a[1][0], a[1][1] - l]]; // A - lambda I
    let e = (l * t_s).exp();
    let f = phi1_real(l, t_s);
    let p = psi_real(l, t_s);
    let mut phi = [[0.0; 2]; 2];
    let mut gamma_mat = [[0.0; 2]; 2];
    for i in 0..2 {
        for j in 0..2 {
            let ident = if i == j { 1.0 } else { 0.0 };
            phi[i][j] = e * (ident + t_s * n[i][j]);
            gamma_mat[i][j] = f * ident + p * n[i][j];
        }
    }
    (phi, [gamma_mat[0][1], gamma_mat[1][1]])
}

// ---------------------------------------------------------------------------
// Matched pole/zero — direct port of the GPL matched branches
// ---------------------------------------------------------------------------

/// `c2d` by the matched pole/zero method — a port of the `"matched"`
/// branch of upstream `inst/@tf/__c2d__.m` (GPL-3.0-or-later, Lukas F.
/// Reichlin), restricted to order <= 2 so poles and zeros come from the
/// quadratic formula instead of a general root finder.
fn c2d_matched(
    sys: &ContinuousTransferFn,
    sample_time: Time,
) -> Result<DiscreteTransferFn, ZDomainError> {
    let t_s = sample_time.get::<second>();
    let order = sys.order();
    if order > 2 {
        return Err(ZDomainError::UnsupportedOrder { order });
    }
    let num = sys.numerator_ascending_s();
    let den = sys.denominator_ascending_s();

    // zpk data: roots and the ratio of leading coefficients.
    let z_c = polynomial::roots_deg_le_2(num).expect("order checked");
    let p_c = polynomial::roots_deg_le_2(den).expect("order checked");
    let k_c = num.last().copied().unwrap_or(0.0) / den.last().copied().unwrap_or(1.0);

    // z = exp(s T) on poles and finite zeros (upstream: p_d = exp(p_c*tsam))
    let p_d: Vec<Cplx> = p_c.iter().map(|&p| (p * t_s).exp()).collect();
    let mut z_d: Vec<Cplx> = z_c.iter().map(|&z| (z * t_s).exp()).collect();
    if p_d.iter().any(|p| p.is_non_finite()) || z_d.iter().any(|z| z.is_non_finite()) {
        return Err(ZDomainError::NonFinitePoleOrZero);
    }

    // continuous zeros at infinity map to z = -1, except one
    // (upstream: z_d = vertcat (z_d, repmat (-1, np-nz-1, 1)))
    let np = p_c.len();
    let nz = z_c.len();
    for _ in 0..np.saturating_sub(nz + 1) {
        z_d.push(Cplx::real(-1.0));
    }

    // Gain matched at w_c = 0 (DC) unless a pole/zero sits within
    // tol = sqrt(eps) of j*w_c; then step w_c by 0.1/T until clear
    // (upstream loop, verbatim semantics).
    let tol = f64::EPSILON.sqrt();
    let mut w_c = 0.0_f64;
    while p_c
        .iter()
        .chain(z_c.iter())
        .any(|r| (*r - Cplx::new(0.0, w_c)).abs() < tol)
    {
        w_c += 0.1 / t_s;
    }
    let jw = Cplx::new(0.0, w_c);
    let w_d = Cplx::new(0.0, w_c * t_s).exp();
    let prod = |roots: &[Cplx], at: Cplx| -> Cplx {
        roots
            .iter()
            .fold(Cplx::real(1.0), |acc, &r| acc * (at - r))
    };
    // k_d = real (k_c * prod(jw - z_c)/prod(jw - p_c)
    //             * prod(w_d - p_d)/prod(w_d - z_d))
    let k_d = (Cplx::real(k_c) * prod(&z_c, jw) / prod(&p_c, jw) * prod(&p_d, w_d)
        / prod(&z_d, w_d))
    .re;

    // Rebuild descending-z polynomials from the discrete roots.
    let num_asc = polynomial::from_roots(&z_d, k_d);
    let den_asc = polynomial::from_roots(&p_d, 1.0);
    let num_desc: Vec<f64> = num_asc.into_iter().rev().collect();
    let den_desc: Vec<f64> = den_asc.into_iter().rev().collect();
    DiscreteTransferFn::from_z_descending_coefficients(num_desc, den_desc, sample_time)
}

/// `d2c` by the matched pole/zero method — a port of the `"matched"`
/// branch of upstream `inst/@tf/__d2c__.m` (GPL-3.0-or-later, Lukas F.
/// Reichlin), restricted to order <= 2.
///
/// Deviation from upstream, documented: upstream's frequency-avoidance
/// loop tests `abs([p_d; z_d_orig] - w_d) < tol` but never updates `w_d`
/// inside the loop, which cannot terminate once entered; this port
/// recomputes `w_d = exp(w_c T)` on every iteration, which is the evident
/// intent. Additionally, where a lone negative-real discrete pole maps to
/// a genuinely complex continuous pole (principal-branch logarithm), the
/// imaginary part of the expanded polynomial is discarded — the same
/// real-coefficient outcome Octave reaches via `real()` on the gain and
/// polynomial construction.
fn d2c_matched(sys: &DiscreteTransferFn) -> Result<ContinuousTransferFn, ZDomainError> {
    let t_s = sys.sample_time().get::<second>();

    // Ascending-z polynomials (reverse of the padded z^-1 form).
    let len = sys
        .numerator_z_inverse()
        .len()
        .max(sys.denominator_z_inverse().len());
    let mut num_zinv = sys.numerator_z_inverse().to_vec();
    let mut den_zinv = sys.denominator_z_inverse().to_vec();
    num_zinv.resize(len, 0.0);
    den_zinv.resize(len, 0.0);
    let num_asc_z = polynomial::trim(num_zinv.into_iter().rev().collect());
    let den_asc_z = polynomial::trim(den_zinv.into_iter().rev().collect());

    let order = polynomial::degree(&num_asc_z).max(polynomial::degree(&den_asc_z));
    if order > 2 {
        return Err(ZDomainError::UnsupportedOrder { order });
    }

    let z_d_orig = polynomial::roots_deg_le_2(&num_asc_z).expect("order checked");
    let p_d = polynomial::roots_deg_le_2(&den_asc_z).expect("order checked");
    let k_d = num_asc_z.last().copied().unwrap_or(0.0) / den_asc_z.last().copied().unwrap_or(1.0);

    // upstream: poles/zeros at z = 0 are rejected because log(0) = -Inf
    if p_d.iter().chain(z_d_orig.iter()).any(|r| r.abs() < f64::EPSILON) {
        return Err(ZDomainError::MatchedPoleZeroAtOrigin);
    }

    // upstream: z_d(abs (z_d+1) < sqrt (eps)) = [] — drop zeros at -1,
    // which are the images of continuous zeros at infinity
    let tol = f64::EPSILON.sqrt();
    let z_d: Vec<Cplx> = z_d_orig
        .iter()
        .copied()
        .filter(|z| (*z + Cplx::real(1.0)).abs() >= tol)
        .collect();

    // s = ln(z)/T (principal branch)
    let p_c: Vec<Cplx> = p_d.iter().map(|&p| p.ln() * (1.0 / t_s)).collect();
    let z_c: Vec<Cplx> = z_d.iter().map(|&z| z.ln() * (1.0 / t_s)).collect();

    // Gain matched at w_d = exp(w_c T) on the unit circle's real axis,
    // stepping w_c away from any discrete pole/zero (see the deviation
    // note in the doc comment).
    let mut w_c = 0.0_f64;
    let mut w_d = Cplx::real(1.0);
    while p_d
        .iter()
        .chain(z_d_orig.iter())
        .any(|r| (*r - w_d).abs() < tol)
    {
        w_c += 0.1 / t_s;
        w_d = Cplx::real((w_c * t_s).exp());
    }
    let w_c_pt = Cplx::real(w_c);
    let prod = |roots: &[Cplx], at: Cplx| -> Cplx {
        roots
            .iter()
            .fold(Cplx::real(1.0), |acc, &r| acc * (at - r))
    };
    // k_c = real (k_d * prod(w_d - z_d_orig)/prod(w_d - p_d)
    //             * prod(w_c - p_c)/prod(w_c - z_c))
    let k_c = (Cplx::real(k_d) * prod(&z_d_orig, w_d) / prod(&p_d, w_d) * prod(&p_c, w_c_pt)
        / prod(&z_c, w_c_pt))
    .re;

    let num_s = polynomial::from_roots(&z_c, k_c);
    let den_s = polynomial::from_roots(&p_c, 1.0);
    ContinuousTransferFn::new(num_s, den_s)
}
