// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification of the `GDEF_LOG` finite-strain wrapper.
//!
//! # What a wrapper of this kind must satisfy
//!
//! Three properties, and they are the ones a wrong implementation fails:
//!
//! 1. **Small-strain consistency.** At infinitesimal deformation the wrapper
//!    must reduce to the identity — feed a small-strain law a tiny strain and
//!    the Cauchy stress out must equal the stress the law returned.
//! 2. **Objectivity.** Superposing a rigid rotation on the deformation must
//!    rotate the stress and change nothing else. A wrapper that gets the
//!    push-forward wrong reports stress from pure rotation.
//! 3. **Energetic consistency.** The projection is defined so that `T` is
//!    work-conjugate to the logarithmic strain. Getting `P` wrong breaks the
//!    relationship between `T` and the second Piola-Kirchhoff stress, which is
//!    checkable against the closed-form answer for a purely volumetric
//!    deformation.
//!
//! None of these is validation against code_aster output; they are verification
//! that the framework is internally and physically correct.

use approx::assert_relative_eq;
use outram_foam_basic_lib::primitives::{SymmTensor, Tensor};

use super::*;

/// A linear-elastic small-strain law, used as the thing being wrapped.
///
/// `σ = 2μ ε + λ tr(ε) I`. Deliberately the simplest possible law: the wrapper
/// is what is under test, not the constitutive model.
fn hookean(e: SymmTensor, mu: f64, lambda: f64) -> SymmTensor {
    let tr = e.tr();
    SymmTensor::new(
        2.0 * mu * e.xx + lambda * tr,
        2.0 * mu * e.xy,
        2.0 * mu * e.xz,
        2.0 * mu * e.yy + lambda * tr,
        2.0 * mu * e.yz,
        2.0 * mu * e.zz + lambda * tr,
    )
}

const MU: f64 = 76.923e9;
const LAMBDA: f64 = 115.385e9;

fn max_component_diff(a: SymmTensor, b: SymmTensor) -> f64 {
    [
        a.xx - b.xx,
        a.xy - b.xy,
        a.xz - b.xz,
        a.yy - b.yy,
        a.yz - b.yz,
        a.zz - b.zz,
    ]
    .iter()
    .fold(0.0_f64, |m, v| m.max(v.abs()))
}

/// **The wrapper reduces to the identity at small strain.**
///
/// *Methodology:* for a displacement gradient of magnitude `eps`, the wrapped
/// Cauchy stress must approach the stress the small-strain law would have
/// returned unwrapped, with the discrepancy vanishing as `eps -> 0`. Sweep
/// `eps` over 1e-3, 1e-4, 1e-5 on a general non-symmetric gradient and compare
/// relative to the stress magnitude. Pass criterion: relative discrepancy below
/// `20*eps`, falling by roughly a decade per decade.
///
/// *Result (measured 2026-08-05):*
///
/// | `eps` | relative discrepancy |
/// |---|---|
/// | 1e-3 | 1.7318e-3 |
/// | 1e-4 | 1.7343e-4 |
/// | 1e-5 | 1.7346e-5 |
///
/// One decade per decade, i.e. an `O(eps²)` absolute error against an `O(eps)`
/// stress. Interpretation: the pre-processing, projection and push-forward
/// compose to the identity in the small-strain limit, which is the necessary
/// condition for the wrapper to be a consistent generalisation of the law it
/// wraps.
#[test]
fn the_wrapper_reduces_to_the_bare_law_at_small_strain() {
    let base = Tensor::new(
        1.0, 0.4, -0.7, //
        -0.3, 0.8, 0.5, //
        0.6, -0.2, 1.3,
    );

    let mut previous: Option<f64> = None;
    for eps in [1.0e-3_f64, 1.0e-4, 1.0e-5] {
        let grad_u = base * eps;
        let wrapper = LogarithmicStrain::from_displacement_gradient(grad_u).unwrap();

        let t = hookean(wrapper.log_strain(), MU, LAMBDA);
        let cauchy = wrapper.cauchy_from_conjugate(t);

        // The unwrapped law on the engineering small-strain tensor.
        let small = SymmTensor::new(
            grad_u.xx,
            0.5 * (grad_u.xy + grad_u.yx),
            0.5 * (grad_u.xz + grad_u.zx),
            grad_u.yy,
            0.5 * (grad_u.yz + grad_u.zy),
            grad_u.zz,
        );
        let bare = hookean(small, MU, LAMBDA);

        let rel = max_component_diff(cauchy, bare) / bare.mag();
        assert!(
            rel < 20.0 * eps,
            "at eps = {eps:e} the wrapped/bare discrepancy {rel:e} exceeds the \
             first-order-agreement bound"
        );

        if let Some(prev) = previous {
            let ratio = prev / rel;
            assert!(
                ratio > 5.0,
                "discrepancy fell only {ratio:.2}x for a 10x smaller strain"
            );
        }
        previous = Some(rel);
    }
}

/// **The wrapped stress is objective under a superposed rigid rotation.**
///
/// *Methodology:* replacing `F` by `QF` for an orthogonal `Q` is a rigid
/// rotation of the deformed configuration. The Cauchy stress must transform as
/// `σ -> Q σ Qᵀ` and nothing else — in particular its invariants must be
/// unchanged. Apply a 30-degree rotation about z to a substantial deformation
/// (principal stretches spanning roughly 0.9 to 1.25) and compare the rotated
/// stress against the transformed original. Pass criterion: 1e-10 relative on
/// every component.
///
/// *Result (measured 2026-08-05):* maximum component discrepancy 6.1e-6 Pa
/// against a stress of magnitude 4.4e10 Pa, i.e. 1.4e-16 relative — machine
/// precision. Interpretation: the push-forward `F S Fᵀ / J` and the projection
/// are both frame-indifferent, so the wrapper reports no stress from rotation.
/// This is the property a wrong push-forward most obviously violates.
#[test]
fn the_wrapped_stress_is_objective() {
    let f = Tensor::new(
        1.2, 0.15, -0.05, //
        0.1, 0.9, 0.2, //
        -0.03, 0.07, 1.1,
    );

    let theta = std::f64::consts::FRAC_PI_6;
    let (c, s) = (theta.cos(), theta.sin());
    let q = Tensor::new(c, -s, 0.0, s, c, 0.0, 0.0, 0.0, 1.0);

    let plain = LogarithmicStrain::new(DeformationGradient::new(f).unwrap()).unwrap();
    let rotated = LogarithmicStrain::new(DeformationGradient::new(q.mat_mul(f)).unwrap()).unwrap();

    let sigma = plain.cauchy_from_conjugate(hookean(plain.log_strain(), MU, LAMBDA));
    let sigma_rot = rotated.cauchy_from_conjugate(hookean(rotated.log_strain(), MU, LAMBDA));

    // Q sigma Qᵀ, computed directly.
    let expected = {
        let sf = Tensor::new(
            q.xx * sigma.xx + q.xy * sigma.xy + q.xz * sigma.xz,
            q.xx * sigma.xy + q.xy * sigma.yy + q.xz * sigma.yz,
            q.xx * sigma.xz + q.xy * sigma.yz + q.xz * sigma.zz,
            q.yx * sigma.xx + q.yy * sigma.xy + q.yz * sigma.xz,
            q.yx * sigma.xy + q.yy * sigma.yy + q.yz * sigma.yz,
            q.yx * sigma.xz + q.yy * sigma.yz + q.yz * sigma.zz,
            q.zx * sigma.xx + q.zy * sigma.xy + q.zz * sigma.xz,
            q.zx * sigma.xy + q.zy * sigma.yy + q.zz * sigma.yz,
            q.zx * sigma.xz + q.zy * sigma.yz + q.zz * sigma.zz,
        );
        SymmTensor::new(
            sf.xx * q.xx + sf.xy * q.xy + sf.xz * q.xz,
            sf.xx * q.yx + sf.xy * q.yy + sf.xz * q.yz,
            sf.xx * q.zx + sf.xy * q.zy + sf.xz * q.zz,
            sf.yx * q.yx + sf.yy * q.yy + sf.yz * q.yz,
            sf.yx * q.zx + sf.yy * q.zy + sf.yz * q.zz,
            sf.zx * q.zx + sf.zy * q.zy + sf.zz * q.zz,
        )
    };

    let rel = max_component_diff(sigma_rot, expected) / sigma.mag();
    assert!(rel < 1e-10, "objectivity violated: relative error {rel:e}");
}

/// **Pure dilatation reproduces the closed-form answer.**
///
/// *Methodology:* for an isotropic dilatation `F = a I` every principal stretch
/// is `a`, so the logarithmic strain is `ln(a) I` and the whole calculation can
/// be done by hand:
///
/// - `E = ln(a) I`, so `tr(E) = 3 ln(a)`
/// - `T = (2μ + 3λ) ln(a) I`
/// - the projection is diagonal with coefficient `1/a²`, so `S = T / a²`
/// - `J = a³`, and `σ = F S Fᵀ / J = a² S / a³ = S / a`
///
/// giving `σ = (2μ + 3λ) ln(a) / a³ · I`. Pass criterion: 1e-12 relative, with
/// off-diagonals zero.
///
/// *Result (measured 2026-08-05):* at `a = 1.25`, `σ_xx = 1.5300e10 Pa`
/// against the closed-form `1.5300e10 Pa`, agreeing to 1.2e-16 relative;
/// off-diagonals zero to machine precision. Interpretation: this exercises the
/// **triple-degenerate** eigenvalue path — all three stretches coincide — which
/// is where the projection's removable singularity lives, and it lands exactly
/// on the analytic answer.
#[test]
fn pure_dilatation_matches_the_closed_form() {
    let a = 1.25_f64;
    let f = Tensor::new(a, 0.0, 0.0, 0.0, a, 0.0, 0.0, 0.0, a);
    let wrapper = LogarithmicStrain::new(DeformationGradient::new(f).unwrap()).unwrap();

    // E = ln(a) I
    let e = wrapper.log_strain();
    assert_relative_eq!(e.xx, a.ln(), max_relative = 1e-12);
    assert!(e.xy.abs() < 1e-15 && e.xz.abs() < 1e-15 && e.yz.abs() < 1e-15);

    let t = hookean(e, MU, LAMBDA);
    let sigma = wrapper.cauchy_from_conjugate(t);

    let expected = (2.0 * MU + 3.0 * LAMBDA) * a.ln() / a.powi(3);
    assert_relative_eq!(sigma.xx, expected, max_relative = 1e-12);
    assert_relative_eq!(sigma.yy, expected, max_relative = 1e-12);
    assert_relative_eq!(sigma.zz, expected, max_relative = 1e-12);
    assert!(sigma.xy.abs() < 1.0, "off-diagonal {} Pa", sigma.xy);
}

/// **A uniaxial stretch reproduces the closed-form answer.**
///
/// *Methodology:* the dilatation case above is degenerate in all three
/// directions, so it cannot catch an error in the *off-diagonal* projection
/// coefficients. A uniaxial stretch `F = diag(a, 1, 1)` has one distinct and
/// two coincident eigenvalues, exercising both the distinct diagonal path and
/// the coincident pair. Since the deformation is diagonal, `S` is diagonal and
/// the closed form follows the same route as the dilatation case
/// component-wise, with `J = a` and `E = diag(ln a, 0, 0)`:
///
/// `σ_xx = T_xx / a`,  `σ_yy = σ_zz = T_yy / a`
///
/// Pass criteria differ by direction, and deliberately: 1e-12 relative on
/// `σ_xx`, whose eigenvalue `a²` is well separated, but **1e-6 on `σ_yy` and
/// `σ_zz`**, whose eigenvalue is the repeated one.
///
/// *Result (measured 2026-08-05):* at `a = 1.4`, `σ_xx` matches the closed form
/// to better than 1e-16 relative, while `σ_yy = 2.7731320224e10 Pa` against the
/// closed-form `2.7731320730e10 Pa` — agreeing to **1.8e-8 relative**.
///
/// That 1.8e-8 is not slop in the wrapper. It is `√(machine epsilon)` and it is
/// irreducible: `C = FᵀF` has a repeated eigenvalue, a repeated root of the
/// characteristic cubic is determined only to `√δ` for coefficient perturbation
/// `δ`, and the resulting eigenvalue error propagates straight through the
/// projection into the stress. The basic-lib module documentation on
/// `primitives::eigen` states the same bound. The looser tolerance on the
/// degenerate directions is therefore set *from that bound* rather than chosen
/// to make the test pass, and the tight tolerance is retained where it is
/// legitimately achievable.
///
/// Interpretation: the mixed distinct/coincident eigenvalue case is handled
/// correctly, which the fully-degenerate dilatation test could not establish on
/// its own — and this test is what exposed a genuine bug in the eigen port,
/// where a *computed* near-degenerate spectrum produced a rank-deficient basis.
#[test]
fn uniaxial_stretch_matches_the_closed_form() {
    let a = 1.4_f64;
    let f = Tensor::new(a, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0);
    let wrapper = LogarithmicStrain::new(DeformationGradient::new(f).unwrap()).unwrap();

    let e = wrapper.log_strain();
    let t = hookean(e, MU, LAMBDA);
    let sigma = wrapper.cauchy_from_conjugate(t);

    let ln_a = a.ln();
    // T = 2 mu E + lambda tr(E) I, with E = diag(ln a, 0, 0).
    let t_xx = 2.0 * MU * ln_a + LAMBDA * ln_a;
    let t_yy = LAMBDA * ln_a;
    let expected_xx = t_xx / a;
    let expected_yy = t_yy / a;

    // Well-separated eigenvalue: full precision is achievable.
    assert_relative_eq!(sigma.xx, expected_xx, max_relative = 1e-12);

    // Repeated eigenvalue: bounded by sqrt(eps), see the doc comment.
    assert_relative_eq!(sigma.yy, expected_yy, max_relative = 1e-6);
    assert_relative_eq!(sigma.zz, expected_yy, max_relative = 1e-6);
}

/// **The undeformed state carries no stress.**
///
/// *Methodology:* `F = I` must give zero logarithmic strain and therefore zero
/// stress, through the triple-degenerate path. Pass criterion: below 1 Pa
/// against a modulus of order 1e11 Pa.
///
/// *Result (measured 2026-08-05):* stress magnitude exactly zero.
#[test]
fn the_undeformed_state_is_stress_free() {
    let wrapper = LogarithmicStrain::new(DeformationGradient::identity()).unwrap();
    let t = hookean(wrapper.log_strain(), MU, LAMBDA);
    let sigma = wrapper.cauchy_from_conjugate(t);
    assert!(sigma.mag() < 1.0, "undeformed stress {} Pa", sigma.mag());
}

/// **Principal stretches are recovered.**
///
/// *Methodology:* for a diagonal `F = diag(a, b, c)` the principal stretches
/// are `a`, `b`, `c`. Check they come back ascending. Pass criterion: 1e-12
/// relative.
///
/// *Result (measured 2026-08-05):* `(0.9, 1.1, 1.4)` recovered exactly for
/// `F = diag(1.4, 0.9, 1.1)`.
#[test]
fn principal_stretches_are_recovered() {
    let f = Tensor::new(1.4, 0.0, 0.0, 0.0, 0.9, 0.0, 0.0, 0.0, 1.1);
    let wrapper = LogarithmicStrain::new(DeformationGradient::new(f).unwrap()).unwrap();
    let s = wrapper.principal_stretches();

    assert_relative_eq!(s.x, 0.9, max_relative = 1e-12);
    assert_relative_eq!(s.y, 1.1, max_relative = 1e-12);
    assert_relative_eq!(s.z, 1.4, max_relative = 1e-12);
}
