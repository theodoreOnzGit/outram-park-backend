// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! WGSL kernels for **closed-form polynomial roots** — bead `op-yvj.4.2`,
//! GitHub #11.
//!
//! # Why only the closed-form solvers are here
//!
//! [`super`] also offers `solve_bracketed_batch` and `solve_newton_batch`, and
//! those are **deliberately absent** from this module. They take the objective
//! as a Rust closure `F: Fn(f64) -> f64`, and a Rust closure cannot be shipped
//! to a WGSL shader — there is no mechanism to compile arbitrary host code into
//! a compute kernel. Putting them on the GPU would mean changing their public
//! signature to accept a WGSL source string instead of a closure, which is a
//! different (and much worse) API, so the iterative root finders stay
//! CPU-only. This is a property of the language boundary, not a gap to be
//! filled later.
//!
//! The closed-form solvers have no such problem: their input is a coefficient
//! array and their output is a root array. Pure data in, pure data out, which
//! is exactly the shape GitHub #11 identified as "branch-light and a good
//! first GPU target".
//!
//! # What is here, and what is not
//!
//! - [`linear_roots_batch`] — `b x + c = 0`. Trivially branch-free.
//! - [`quadratic_roots_batch`] — `a x^2 + b x + c = 0`, including the
//!   complex-conjugate and degenerate-`a` cases.
//! - **Cubic is not here.** `CubicEqn::roots` selects between a trigonometric
//!   and a Cardano branch, each with its own sub-cases and a
//!   cube-root-of-a-signed-quantity step. Every lane in a workgroup executes
//!   both sides of a divergent branch, so the lockstep advantage is lost
//!   exactly where the arithmetic is most `f32`-sensitive; the discriminant of
//!   a near-degenerate cubic loses more relative precision than the roots have
//!   to spare. Measuring a slower *and* less accurate kernel would not have
//!   made it worth shipping. `cubic_roots_batch` therefore stays CPU-only, and
//!   this paragraph is the reason rather than an omission.
//!
//! # Precision
//!
//! `f32`, as everywhere in [`crate::compute::gpu`]. One specific loss is worth
//! naming: [`crate::polynomial::quadratic_eqn::QuadraticEqn::roots`] computes
//! its discriminant with an **FMA-compensated** expression precisely to keep
//! accuracy when `b^2/4` and `a c` nearly cancel. WGSL's `fma` is not
//! guaranteed to be a fused single-rounding operation, so the GPU kernel uses
//! the plain expression and gives up that compensation. Well-separated roots
//! are unaffected; near-degenerate ones (discriminant close to zero) are where
//! the two paths diverge most.
//!
//! **The figures quoted on each function were measured on well-separated roots
//! only** — they are not a bound on the near-degenerate case, and should not be
//! read as one. Characterising the degenerate range is outstanding work.

use crate::compute::gpu::{bytes_to_u32_vec, context, f64_to_f32_bytes, u32_to_bytes};
use crate::polynomial::linear_eqn::LinearEqn;
use crate::polynomial::quadratic_eqn::QuadraticEqn;
use crate::polynomial::roots::{RootType, Roots};

/// `b x + c = 0`, one lane per equation.
///
/// Mirrors `LinearEqn::roots`: a vanishing `b` yields `±inf` by the sign of
/// `c`, or `NaN` when `c` vanishes too.
const LINEAR_WGSL: &str = r#"
@group(0) @binding(0) var<storage, read> coeffs: array<f32>;   // b, c interleaved
@group(0) @binding(1) var<storage, read_write> out: array<u32>; // value bits, tag
@group(0) @binding(2) var<uniform> params: vec4<u32>;

const T_REAL:    u32 = 0u;
const T_POS_INF: u32 = 2u;
const T_NEG_INF: u32 = 3u;
const T_NAN:     u32 = 4u;

// Matches the crate's VSMALL underflow guard.
const VSMALL: f32 = 1.0e-37;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= params.x) { return; }
    let b = coeffs[2u * i];
    let c = coeffs[2u * i + 1u];

    var value: f32 = 0.0;
    var tag: u32 = T_REAL;
    if (abs(b) < VSMALL) {
        if (abs(c) < VSMALL) {
            value = 0.0;
            tag = T_NAN;
        } else if (c > 0.0) {
            value = 0.0;
            tag = T_NEG_INF;
        } else {
            value = 0.0;
            tag = T_POS_INF;
        }
    } else {
        value = -c / b;
        tag = T_REAL;
    }
    out[2u * i]      = bitcast<u32>(value);
    out[2u * i + 1u] = tag;
}
"#;

/// `a x^2 + b x + c = 0`, one lane per equation.
///
/// Follows the same case split as the CPU solver — degenerate `a` falls back to
/// the linear root, a positive discriminant uses the sign-stable form that
/// avoids cancellation, a negative one emits the conjugate pair as
/// `(re, im)` both tagged complex, and a zero discriminant emits the repeated
/// root twice.
const QUADRATIC_WGSL: &str = r#"
@group(0) @binding(0) var<storage, read> coeffs: array<f32>;    // a, b, c
@group(0) @binding(1) var<storage, read_write> out: array<u32>;  // v0,t0,v1,t1
@group(0) @binding(2) var<uniform> params: vec4<u32>;

const T_REAL:    u32 = 0u;
const T_COMPLEX: u32 = 1u;
const T_POS_INF: u32 = 2u;
const T_NEG_INF: u32 = 3u;
const T_NAN:     u32 = 4u;

const VSMALL: f32 = 1.0e-37;

fn sgn(x: f32) -> f32 {
    if (x < 0.0) { return -1.0; }
    return 1.0;
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= params.x) { return; }
    let a = coeffs[3u * i];
    let b = coeffs[3u * i + 1u];
    let c = coeffs[3u * i + 2u];

    var v0: f32 = 0.0; var t0: u32 = T_REAL;
    var v1: f32 = 0.0; var t1: u32 = T_NAN;

    if (abs(a) < VSMALL) {
        // Degenerate: linear root in slot 0, NaN in slot 1.
        if (abs(b) < VSMALL) {
            if (abs(c) < VSMALL) { t0 = T_NAN; }
            else if (c > 0.0)    { t0 = T_NEG_INF; }
            else                 { t0 = T_POS_INF; }
        } else {
            v0 = -c / b;
            t0 = T_REAL;
        }
        t1 = T_NAN;
    } else {
        // No FMA compensation available — see the module docs.
        var discr = b * b / 4.0 - a * c;
        if (abs(discr) <= VSMALL) { discr = 0.0; }

        if (discr > 0.0) {
            let x = -b / 2.0 - sgn(b) * sqrt(discr);
            // roots of (-a x + x0) and (-x0 x + c), matching the CPU pairing
            v0 = x / a;
            t0 = T_REAL;
            v1 = c / x;
            t1 = T_REAL;
        } else if (discr < 0.0) {
            v0 = -b / 2.0 / a;
            t0 = T_COMPLEX;
            v1 = sgn(b) * sqrt(-discr) / a;
            t1 = T_COMPLEX;
        } else {
            let r = -(b / 2.0) / a;
            v0 = r; t0 = T_REAL;
            v1 = r; t1 = T_REAL;
        }
    }

    out[4u * i]      = bitcast<u32>(v0);
    out[4u * i + 1u] = t0;
    out[4u * i + 2u] = bitcast<u32>(v1);
    out[4u * i + 3u] = t1;
}
"#;

/// Decode a `u32` tag back to a [`RootType`]. Anything unexpected is `Nan`,
/// which is the conservative reading — a caller must never mistake a garbled
/// tag for a valid real root.
fn tag_to_type(tag: u32) -> RootType {
    match tag {
        0 => RootType::Real,
        1 => RootType::Complex,
        2 => RootType::PosInf,
        3 => RootType::NegInf,
        _ => RootType::Nan,
    }
}

/// Roots of `n` independent linear equations on the GPU.
///
/// # Returns
///
/// `Some(roots)` in input order, or `None` when there is no adapter, the batch
/// is empty, or the dispatch failed — in which case the caller runs
/// [`super::linear_roots_batch`] on a CPU backend instead.
///
/// # Precision
///
/// `f32`. A linear root is a single division, so the error is essentially one
/// rounding: **max relative deviation 1.30e-7** over 262 144 equations with
/// non-dyadic coefficients (Mesa Intel Graphics RPL-S, OpenGL backend,
/// 2026-09-03) — right at the `f32` epsilon floor of ~1.2e-7.
pub fn linear_roots_batch(eqns: &[LinearEqn]) -> Option<Vec<Roots<1>>> {
    let ctx = context()?;
    let n = eqns.len();
    if n == 0 || (n as u64) > ctx.max_lanes() {
        return None;
    }
    let mut coeffs = Vec::with_capacity(n * 2);
    for e in eqns {
        coeffs.push(e.a);
        coeffs.push(e.b);
    }
    let bytes = ctx
        .dispatch(
            "poly-linear-roots",
            LINEAR_WGSL,
            &[&f64_to_f32_bytes(&coeffs)],
            (n * 2 * 4) as u64,
            &u32_to_bytes(&[n as u32, 0, 0, 0]),
            n as u32,
        )
        .ok()?;
    let words = bytes_to_u32_vec(&bytes);
    if words.len() != n * 2 {
        return None;
    }
    Some(
        (0..n)
            .map(|i| {
                let v = f32::from_bits(words[2 * i]) as f64;
                Roots::<1>::new(tag_to_type(words[2 * i + 1]), v)
            })
            .collect(),
    )
}

/// Roots of `n` independent quadratic equations on the GPU.
///
/// # Returns
///
/// `Some(roots)` in input order, or `None` to signal "use the CPU path".
///
/// # Precision
///
/// `f32`, and **without** the CPU solver's FMA-compensated discriminant (see
/// the module docs). Measured over 262 144 equations with non-dyadic
/// coefficients and well-separated real roots: **max relative deviation
/// 2.85e-7, RMS 5.70e-8** (Mesa Intel Graphics RPL-S, OpenGL backend,
/// 2026-09-03).
///
/// Note what that measurement does *not* cover. Well-separated roots are the
/// easy case; the FMA compensation the CPU solver uses exists for the
/// near-double-root case, where `b^2/4` and `a c` nearly cancel. The gate is
/// therefore set at 1e-4 rather than at the measured value, and a batch full
/// of near-degenerate quadratics should be expected to do materially worse
/// than the figure above. Root **types** are asserted to match exactly on
/// both paths, which is the property a caller is most likely to depend on.
pub fn quadratic_roots_batch(eqns: &[QuadraticEqn]) -> Option<Vec<Roots<2>>> {
    let ctx = context()?;
    let n = eqns.len();
    if n == 0 || (n as u64) > ctx.max_lanes() {
        return None;
    }
    let mut coeffs = Vec::with_capacity(n * 3);
    for e in eqns {
        coeffs.push(e.a);
        coeffs.push(e.b);
        coeffs.push(e.c);
    }
    let bytes = ctx
        .dispatch(
            "poly-quadratic-roots",
            QUADRATIC_WGSL,
            &[&f64_to_f32_bytes(&coeffs)],
            (n * 4 * 4) as u64,
            &u32_to_bytes(&[n as u32, 0, 0, 0]),
            n as u32,
        )
        .ok()?;
    let words = bytes_to_u32_vec(&bytes);
    if words.len() != n * 4 {
        return None;
    }
    Some(
        (0..n)
            .map(|i| {
                let r0 = Roots::<1>::new(
                    tag_to_type(words[4 * i + 1]),
                    f32::from_bits(words[4 * i]) as f64,
                );
                let r1 = Roots::<1>::new(
                    tag_to_type(words[4 * i + 3]),
                    f32::from_bits(words[4 * i + 2]) as f64,
                );
                Roots::from_pair(r0, r1)
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compute::ComputeBackend;

    /// Non-dyadic coefficients, for the reason given in
    /// `examples/hybrid_gpu_report.rs`: dyadic values are exact in `f32` and
    /// would make the comparison measure nothing.
    fn linear_batch(n: usize) -> Vec<LinearEqn> {
        (0..n)
            .map(|i| {
                let t = i as f64;
                LinearEqn::new(1.0 + t / 3.0_f64.sqrt(), -2.0 - t / 7.0_f64.sqrt())
            })
            .collect()
    }

    /// Quadratics with well-separated real roots `r1 = 1 + i/97`,
    /// `r2 = -3 - i/131`, expanded to coefficient form.
    fn quadratic_batch(n: usize) -> Vec<QuadraticEqn> {
        (0..n)
            .map(|i| {
                let t = i as f64;
                let r1 = 1.0 + t / 97.0;
                let r2 = -3.0 - t / 131.0;
                let a = 1.0 + t / 1000.0;
                QuadraticEqn::new(a, -a * (r1 + r2), a * r1 * r2)
            })
            .collect()
    }

    #[test]
    fn linear_matches_the_serial_oracle() {
        if context().is_none() {
            eprintln!("no GPU adapter — skipping");
            return;
        }
        let n = 262_144;
        let eqns = linear_batch(n);
        let want = super::super::linear_roots_batch(&eqns, ComputeBackend::Serial);
        let got = linear_roots_batch(&eqns).expect("GPU linear roots");
        assert_eq!(got.len(), want.len());

        let mut worst = 0.0_f64;
        for (g, w) in got.iter().zip(&want) {
            assert_eq!(
                g.root_type(0),
                w.root_type(0),
                "root type must match exactly"
            );
            if g.root_type(0) == RootType::Real {
                worst = worst.max((g.get(0) - w.get(0)).abs() / w.get(0).abs().max(1.0));
            }
        }
        println!("[gpu] linear roots max rel = {worst:.3e}");
        assert!(worst < 1e-6, "linear roots deviated {worst:e}");
    }

    #[test]
    fn quadratic_matches_the_serial_oracle() {
        if context().is_none() {
            return;
        }
        let n = 262_144;
        let eqns = quadratic_batch(n);
        let want = super::super::quadratic_roots_batch(&eqns, ComputeBackend::Serial);
        let got = quadratic_roots_batch(&eqns).expect("GPU quadratic roots");

        let mut worst = 0.0_f64;
        let mut sq = 0.0_f64;
        let mut counted = 0usize;
        for (g, w) in got.iter().zip(&want) {
            for slot in 0..2 {
                assert_eq!(
                    g.root_type(slot),
                    w.root_type(slot),
                    "root type must match exactly in slot {slot}"
                );
                if g.root_type(slot) == RootType::Real {
                    let rel = (g.get(slot) - w.get(slot)).abs() / w.get(slot).abs().max(1.0);
                    worst = worst.max(rel);
                    sq += rel * rel;
                    counted += 1;
                }
            }
        }
        let rms = (sq / counted.max(1) as f64).sqrt();
        println!("[gpu] quadratic roots max rel = {worst:.3e}, RMS = {rms:.3e}");
        assert!(worst < 1e-4, "quadratic roots deviated {worst:e}");
    }

    #[test]
    fn complex_pairs_are_tagged_the_same_on_both_paths() {
        if context().is_none() {
            return;
        }
        // x^2 + b x + 5 with small b: discriminant negative → conjugate pair.
        let eqns: Vec<QuadraticEqn> = (0..1024)
            .map(|i| QuadraticEqn::new(1.0, (i as f64) / 1000.0, 5.0))
            .collect();
        let want = super::super::quadratic_roots_batch(&eqns, ComputeBackend::Serial);
        let got = quadratic_roots_batch(&eqns).expect("GPU quadratic roots");
        for (g, w) in got.iter().zip(&want) {
            assert_eq!(g.root_type(0), RootType::Complex);
            assert_eq!(g.root_type(0), w.root_type(0));
            assert_eq!(g.root_type(1), w.root_type(1));
        }
    }

    #[test]
    fn degenerate_leading_coefficient_matches_the_cpu_fallback() {
        if context().is_none() {
            return;
        }
        // a == 0 → the CPU solver drops to the linear root and tags slot 1 NaN.
        let eqns: Vec<QuadraticEqn> = (0..256)
            .map(|i| QuadraticEqn::new(0.0, 2.0 + (i as f64) / 13.0, -1.0))
            .collect();
        let want = super::super::quadratic_roots_batch(&eqns, ComputeBackend::Serial);
        let got = quadratic_roots_batch(&eqns).expect("GPU quadratic roots");
        for (g, w) in got.iter().zip(&want) {
            assert_eq!(g.root_type(0), w.root_type(0));
            assert_eq!(g.root_type(1), RootType::Nan);
            assert!((g.get(0) - w.get(0)).abs() < 1e-5);
        }
    }

    #[test]
    fn empty_batch_returns_none() {
        assert!(linear_roots_batch(&[]).is_none());
        assert!(quadratic_roots_batch(&[]).is_none());
    }
}
