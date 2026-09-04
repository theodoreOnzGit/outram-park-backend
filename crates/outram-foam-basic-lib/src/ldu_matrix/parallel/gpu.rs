// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! WGSL compute kernels for the LDU sparse product and the vector operations —
//! bead `op-yvj.4.4`, GitHub #13 items 1 and 2.
//!
//! # What this is
//!
//! The GPU half of [`super`]. Every function here mirrors exactly one CPU
//! function in the parent module, takes the same arguments, and returns
//! `Option`: `None` means "the GPU could not do it, use the CPU path", never
//! "the answer is wrong". The parent module owns the fallback decision; this
//! module never decides policy.
//!
//! # Precision — `f32`, and it matters more here than anywhere else
//!
//! WGSL has no `f64`, so every kernel below computes in `f32` (see
//! [`crate::compute::gpu`]). For a **single** SpMV or axpy that is a ~1e-7
//! relative error and unremarkable. Inside a **Krylov iteration** it is not:
//! an `f32` residual cannot drive an `f64` solve below about `1e-6` relative,
//! so a BiCGStab loop that calls these kernels will stall at that tolerance
//! rather than converging to `1e-10`.
//!
//! That is why these kernels are **not** on the auto-select path.
//! [`super::effective_backend`] never returns [`ComputeBackend::Gpu`] on its
//! own; a caller gets a GPU kernel only by naming
//! [`ComputeBackend::Gpu`] explicitly, having read this paragraph. The
//! measured numbers are in the parent module's test section and in
//! `tests/hybrid_parity.rs`.
//!
//! # Determinism
//!
//! [`dot`] and [`norm_l1`] sum each workgroup's 64 lanes on the GPU and then
//! combine the per-workgroup partials **on the host, in ascending workgroup
//! order**. The result is therefore reproducible run to run for a fixed input
//! length — unlike a full GPU tree reduction — while still differing from the
//! serial `f64` sum because the arithmetic itself is `f32` and the
//! associativity differs. Reproducible is not the same as identical, and the
//! parity gates use a tolerance for exactly this reason.

use crate::compute::gpu::{bytes_to_f64_vec, context, f64_to_f32_bytes, u32_to_bytes, WORKGROUP_SIZE};
use crate::compute::ComputeBackend;
use crate::ldu_matrix::ldu_matrix::LduMatrix;

use super::LduTopology;

/// `y = A x` for an LDU matrix, one lane per cell.
///
/// The row run `[row_start[c], row_start[c + 1])` is walked serially inside the
/// lane, exactly as the CPU kernel walks it, so the per-cell summation order is
/// the same on both paths and only the arithmetic precision differs.
const SPMV_WGSL: &str = r#"
@group(0) @binding(0) var<storage, read> diag:       array<f32>;
@group(0) @binding(1) var<storage, read> lower:      array<f32>;
@group(0) @binding(2) var<storage, read> upper:      array<f32>;
@group(0) @binding(3) var<storage, read> x:          array<f32>;
@group(0) @binding(4) var<storage, read> row_start:  array<u32>;
@group(0) @binding(5) var<storage, read> entry_face: array<u32>;
// Column index of each entry, with the "uses upper" flag folded into the top
// bit so the shader needs one buffer instead of two.
@group(0) @binding(6) var<storage, read> entry_other_flagged: array<u32>;
@group(0) @binding(7) var<storage, read_write> y: array<f32>;
@group(0) @binding(8) var<uniform> params: vec4<u32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let c = gid.x;
    let n_cells = params.x;
    if (c >= n_cells) { return; }

    var acc: f32 = diag[c] * x[c];
    let start = row_start[c];
    let end   = row_start[c + 1u];
    for (var e: u32 = start; e < end; e = e + 1u) {
        let f       = entry_face[e];
        let flagged = entry_other_flagged[e];
        let other   = flagged & 0x7fffffffu;
        let coeff   = select(lower[f], upper[f], (flagged & 0x80000000u) != 0u);
        acc = acc + coeff * x[other];
    }
    y[c] = acc;
}
"#;

/// `y = alpha * x + y`, one lane per element. Purely elementwise, so the
/// result is exactly the `f32` rounding of the serial answer.
const AXPY_WGSL: &str = r#"
@group(0) @binding(0) var<storage, read> x: array<f32>;
@group(0) @binding(1) var<storage, read> y_in: array<f32>;
@group(0) @binding(2) var<storage, read_write> y_out: array<f32>;
@group(0) @binding(3) var<uniform> params: vec4<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= u32(params.y)) { return; }
    y_out[i] = params.x * x[i] + y_in[i];
}
"#;

/// `x = alpha * x`, one lane per element.
const SCALE_WGSL: &str = r#"
@group(0) @binding(0) var<storage, read> x: array<f32>;
@group(0) @binding(1) var<storage, read_write> out: array<f32>;
@group(0) @binding(2) var<uniform> params: vec4<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= u32(params.y)) { return; }
    out[i] = params.x * x[i];
}
"#;

/// Per-workgroup partial of `sum_i a_i * b_i`, reduced in shared memory.
///
/// Emits one `f32` per workgroup; the host combines them in workgroup order.
/// `params.y` selects the operand: `0` = `a[i] * b[i]` (dot), `1` = `abs(a[i])`
/// (L1 norm), so one shader serves both reductions.
const REDUCE_WGSL: &str = r#"
@group(0) @binding(0) var<storage, read> a: array<f32>;
@group(0) @binding(1) var<storage, read> b: array<f32>;
@group(0) @binding(2) var<storage, read_write> partials: array<f32>;
@group(0) @binding(3) var<uniform> params: vec4<u32>;

var<workgroup> scratch: array<f32, 64>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) gid: vec3<u32>,
        @builtin(local_invocation_id)  lid: vec3<u32>,
        @builtin(workgroup_id)         wid: vec3<u32>) {
    let i = gid.x;
    let n = params.x;
    var v: f32 = 0.0;
    if (i < n) {
        if (params.y == 0u) {
            v = a[i] * b[i];
        } else {
            v = abs(a[i]);
        }
    }
    scratch[lid.x] = v;
    workgroupBarrier();

    // Tree reduction within the workgroup: 64 -> 32 -> ... -> 1.
    var stride: u32 = 32u;
    loop {
        if (stride == 0u) { break; }
        if (lid.x < stride) {
            scratch[lid.x] = scratch[lid.x] + scratch[lid.x + stride];
        }
        workgroupBarrier();
        stride = stride / 2u;
    }
    if (lid.x == 0u) {
        partials[wid.x] = scratch[0];
    }
}
"#;

/// `y = A x` on the GPU. `None` when there is no adapter or the dispatch fails.
///
/// # Arguments
///
/// - `matrix` — the assembled LDU system; only `diag`, `lower`, `upper` are read.
/// - `topology` — the cell-gather index for `matrix`'s addressing.
/// - `x` — the operand, length `matrix.n_cells`, dimensionless.
/// - `y` — the destination, same length; overwritten in full.
///
/// # Returns
///
/// `Some(())` when the GPU produced `y`, `None` when the caller must run a CPU
/// path instead. On `None`, `y` is untouched.
///
/// # Precision
///
/// `f32` throughout — see the module docs. Measured against the serial `f64`
/// kernel by `examples/hybrid_gpu_report.rs` on a 1 048 576-cell 1-D Laplacian
/// with non-dyadic coefficients: **max relative deviation 2.81e-7, RMS
/// 6.43e-8** (Mesa Intel Graphics RPL-S, OpenGL backend, 2026-09-03).
pub fn spmv_into(
    matrix: &LduMatrix,
    topology: &LduTopology,
    x: &[f64],
    y: &mut [f64],
) -> Option<()> {
    let ctx = context()?;
    let n = matrix.n_cells;
    if n == 0 || x.len() != n || y.len() != n {
        return None;
    }
    if (n as u64) > ctx.max_lanes() {
        return None;
    }

    let row_start: Vec<u32> = topology.row_start.iter().map(|&v| v as u32).collect();
    let entry_face: Vec<u32> = topology.entry_face.iter().map(|&v| v as u32).collect();
    // Fold the side flag into the column index's top bit. Cell indices this
    // large (>= 2^31) would already exceed every other u32 buffer here, so the
    // bit is genuinely free.
    let entry_other: Vec<u32> = topology
        .entry_other
        .iter()
        .zip(&topology.entry_uses_upper)
        .map(|(&o, &upper)| (o as u32) | if upper { 0x8000_0000 } else { 0 })
        .collect();

    let params = u32_to_bytes(&[n as u32, 0, 0, 0]);
    let bytes = ctx
        .dispatch(
            "ldu-spmv",
            SPMV_WGSL,
            &[
                &f64_to_f32_bytes(&matrix.diag),
                &f64_to_f32_bytes(&matrix.lower),
                &f64_to_f32_bytes(&matrix.upper),
                &f64_to_f32_bytes(x),
                &u32_to_bytes(&row_start),
                &u32_to_bytes(&entry_face),
                &u32_to_bytes(&entry_other),
            ],
            (n * 4) as u64,
            &params,
            n as u32,
        )
        .ok()?;

    let out = bytes_to_f64_vec(&bytes);
    if out.len() != n {
        return None;
    }
    y.copy_from_slice(&out);
    Some(())
}

/// `y = alpha * x + y` on the GPU. `None` on any failure; `y` untouched then.
///
/// # Precision
///
/// Elementwise, so the result is the `f32` rounding of the exact answer:
/// **max relative deviation 1.11e-7, RMS 3.70e-8** over 1 048 576 non-dyadic
/// elements (Mesa Intel Graphics RPL-S, OpenGL backend, 2026-09-03).
pub fn axpy(alpha: f64, x: &[f64], y: &mut [f64]) -> Option<()> {
    let ctx = context()?;
    let n = x.len();
    if n == 0 || y.len() != n || (n as u64) > ctx.max_lanes() {
        return None;
    }
    let mut params = (alpha as f32).to_le_bytes().to_vec();
    params.extend_from_slice(&(n as f32).to_le_bytes());
    let bytes = ctx
        .dispatch(
            "ldu-axpy",
            AXPY_WGSL,
            &[&f64_to_f32_bytes(x), &f64_to_f32_bytes(y)],
            (n * 4) as u64,
            &params,
            n as u32,
        )
        .ok()?;
    let out = bytes_to_f64_vec(&bytes);
    if out.len() != n {
        return None;
    }
    y.copy_from_slice(&out);
    Some(())
}

/// `x = alpha * x` on the GPU. `None` on any failure; `x` untouched then.
pub fn scale(alpha: f64, x: &mut [f64]) -> Option<()> {
    let ctx = context()?;
    let n = x.len();
    if n == 0 || (n as u64) > ctx.max_lanes() {
        return None;
    }
    let mut params = (alpha as f32).to_le_bytes().to_vec();
    params.extend_from_slice(&(n as f32).to_le_bytes());
    let bytes = ctx
        .dispatch(
            "ldu-scale",
            SCALE_WGSL,
            &[&f64_to_f32_bytes(x)],
            (n * 4) as u64,
            &params,
            n as u32,
        )
        .ok()?;
    let out = bytes_to_f64_vec(&bytes);
    if out.len() != n {
        return None;
    }
    x.copy_from_slice(&out);
    Some(())
}

/// The shared reduction driver behind [`dot`] and [`norm_l1`].
///
/// `mode` is `0` for `sum(a_i * b_i)` and `1` for `sum(abs(a_i))`.
fn reduce(a: &[f64], b: &[f64], mode: u32, label: &'static str) -> Option<f64> {
    let ctx = context()?;
    let n = a.len();
    if n == 0 || (n as u64) > ctx.max_lanes() {
        return None;
    }
    let groups = (n as u32).div_ceil(WORKGROUP_SIZE).max(1);
    let params = u32_to_bytes(&[n as u32, mode, 0, 0]);
    let bytes = ctx
        .dispatch(
            label,
            REDUCE_WGSL,
            &[&f64_to_f32_bytes(a), &f64_to_f32_bytes(b)],
            (groups as u64) * 4,
            &params,
            n as u32,
        )
        .ok()?;
    let partials = bytes_to_f64_vec(&bytes);
    if partials.len() != groups as usize {
        return None;
    }
    // Combine on the host in ascending workgroup order: deterministic run to
    // run, and accumulated in f64 so only the per-workgroup partials carry
    // f32 error rather than the whole sum.
    Some(partials.iter().sum())
}

/// `sum_i a_i * b_i` on the GPU. `None` on any failure.
///
/// # Precision
///
/// Each workgroup's 64-lane tree reduction runs in `f32`; the per-workgroup
/// partials are then summed in `f64` on the host. Measured against the serial
/// kernel over 1 048 576 non-dyadic elements: **relative deviation 1.04e-11**
/// (Mesa Intel Graphics RPL-S, OpenGL backend, 2026-09-03) — far better than
/// the elementwise kernels because the `f32` rounding errors of a
/// same-signed-ish sum largely cancel rather than accumulating.
///
/// Do **not** read that figure as a general guarantee. Cancellation-heavy
/// inputs (a residual near convergence, where the terms nearly sum to zero)
/// will do far worse: an `f32` reduction has no defence against catastrophic
/// cancellation, and that is the reason this kernel is off auto-select.
pub fn dot(a: &[f64], b: &[f64]) -> Option<f64> {
    if a.len() != b.len() {
        return None;
    }
    reduce(a, b, 0, "ldu-dot")
}

/// `sum_i abs(x_i)` on the GPU. `None` on any failure.
///
/// No cancellation is possible (every term is non-negative), so this is the
/// best-behaved reduction of the set and the one figure here that is
/// trustworthy across input ranges: **relative deviation 2.84e-10** over
/// 1 048 576 non-dyadic elements (Mesa Intel Graphics RPL-S, OpenGL backend,
/// 2026-09-03).
pub fn norm_l1(x: &[f64]) -> Option<f64> {
    reduce(x, x, 1, "ldu-norm-l1")
}

/// Whether the GPU path in this module can run right now — a live adapter and
/// the `gpu` feature. Used by [`super::effective_backend`] so a `Gpu` request
/// degrades rather than failing.
#[must_use]
pub fn available() -> bool {
    ComputeBackend::Gpu.is_available() && context().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// A 1-D Laplacian on `n` cells: `diag = 2`, off-diagonals `-1`.
    fn laplacian(n: usize) -> (LduMatrix, LduTopology) {
        let owner: Vec<usize> = (0..n - 1).collect();
        let neighbour: Vec<usize> = (1..n).collect();
        let mut m = LduMatrix::new(n, owner, neighbour);
        m.diag.iter_mut().for_each(|d| *d = 2.0);
        m.lower.iter_mut().for_each(|v| *v = -1.0);
        m.upper.iter_mut().for_each(|v| *v = -1.0);
        let t = LduTopology::from_matrix(&m);
        (m, t)
    }

    fn max_rel(got: &[f64], want: &[f64]) -> f64 {
        got.iter()
            .zip(want)
            .map(|(g, w)| (g - w).abs() / w.abs().max(1.0))
            .fold(0.0_f64, f64::max)
    }

    #[test]
    fn spmv_matches_the_serial_oracle_within_f32() {
        if context().is_none() {
            eprintln!("no GPU adapter — skipping");
            return;
        }
        let n = 32_768;
        let (m, t) = laplacian(n);
        let x: Vec<f64> = (0..n).map(|i| ((i % 17) as f64) * 0.25 - 2.0).collect();

        let hybrid = super::super::HybridLdu::new(Arc::new(m.clone()));
        let want = hybrid.spmv(&x, ComputeBackend::Serial);

        let mut got = vec![0.0; n];
        spmv_into(&m, &t, &x, &mut got).expect("GPU spmv");

        let rel = max_rel(&got, &want);
        assert!(
            rel < 1e-5,
            "max relative deviation {rel:e} exceeds the f32 budget"
        );
    }

    #[test]
    fn axpy_matches_the_serial_oracle_within_f32() {
        if context().is_none() {
            return;
        }
        let n = 100_000;
        let x: Vec<f64> = (0..n).map(|i| (i as f64) * 1e-3).collect();
        let y0: Vec<f64> = (0..n).map(|i| ((i % 7) as f64) - 3.0).collect();

        let mut want = y0.clone();
        super::super::axpy(2.5, &x, &mut want, ComputeBackend::Serial);

        let mut got = y0.clone();
        axpy(2.5, &x, &mut got).expect("GPU axpy");

        let rel = max_rel(&got, &want);
        assert!(rel < 1e-6, "max relative deviation {rel:e}");
    }

    #[test]
    fn scale_matches_the_serial_oracle_within_f32() {
        if context().is_none() {
            return;
        }
        let n = 50_000;
        let x0: Vec<f64> = (0..n).map(|i| (i as f64) * 0.5 - 100.0).collect();

        let mut want = x0.clone();
        super::super::scale(-1.5, &mut want, ComputeBackend::Serial);

        let mut got = x0.clone();
        scale(-1.5, &mut got).expect("GPU scale");

        assert!(max_rel(&got, &want) < 1e-6);
    }

    #[test]
    fn dot_and_norm_l1_match_within_the_stated_tolerance() {
        if context().is_none() {
            return;
        }
        let n = 1_000_000;
        let a: Vec<f64> = (0..n).map(|i| ((i % 13) as f64) * 0.1).collect();
        let b: Vec<f64> = (0..n).map(|i| ((i % 11) as f64) * 0.2).collect();

        let want_dot = super::super::dot(&a, &b, ComputeBackend::Serial);
        let got_dot = dot(&a, &b).expect("GPU dot");
        let rel = (got_dot - want_dot).abs() / want_dot.abs().max(1.0);
        assert!(rel < 1e-6, "dot relative deviation {rel:e}");

        let want_l1 = super::super::norm_l1(&a, ComputeBackend::Serial);
        let got_l1 = norm_l1(&a).expect("GPU norm_l1");
        let rel = (got_l1 - want_l1).abs() / want_l1.abs().max(1.0);
        assert!(rel < 1e-6, "norm_l1 relative deviation {rel:e}");
    }

    #[test]
    fn reductions_are_reproducible_run_to_run() {
        if context().is_none() {
            return;
        }
        let n = 200_000;
        let a: Vec<f64> = (0..n).map(|i| ((i % 29) as f64) * 0.37 - 5.0).collect();
        let first = dot(&a, &a).expect("GPU dot");
        for _ in 0..4 {
            assert_eq!(
                dot(&a, &a).expect("GPU dot"),
                first,
                "host-side ordered combine must make the reduction reproducible"
            );
        }
    }

    #[test]
    fn empty_and_mismatched_inputs_return_none_rather_than_panicking() {
        assert!(dot(&[], &[]).is_none());
        assert!(dot(&[1.0], &[1.0, 2.0]).is_none());
        assert!(norm_l1(&[]).is_none());
        let mut y = vec![0.0; 3];
        assert!(axpy(1.0, &[], &mut y).is_none());
    }
}
