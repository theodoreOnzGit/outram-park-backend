// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification and crossover benchmarks for the hybrid-backend LDU kernels.
//!
//! # What is verified here
//!
//! 1. **Topology inversion** — the cell-gather index really is the face
//!    addressing turned inside out, and it detects a mesh that changed under it.
//! 2. **Bitwise parity with the serial oracle** — every product, residual,
//!    element-wise and reduction kernel is compared against
//!    [`LduMatrix::multiply`] / [`LduMatrix::residual`] or against its own serial
//!    backend, comparing `f64::to_bits()` rather than values, so a one-ulp drift
//!    fails the test.
//! 3. **Measured deviation from the flat-sum references** — the reductions are
//!    *not* bitwise equal to [`crate::krylov::vecops`]'s flat sums, and the size
//!    of that difference is measured and gated rather than assumed.
//! 4. **Crossover benchmarks** (`#[ignore]`d) — the absolute timings behind
//!    [`SPMV_MIN_CELLS`] and [`VECOP_MIN_ELEMENTS`].
//!
//! # A caveat on what these tests can prove
//!
//! With the `parallel` feature **off** — the default —
//! [`ComputeBackend::CpuMulti`] resolves to [`ComputeBackend::Serial`], so every
//! cross-backend test still passes but exercises one code path twice. The tests
//! that matter for the parallel path are only meaningful under
//! `--features parallel`, and both feature settings are run before this module is
//! called done.
//!
//! # Running the benchmarks
//!
//! ```text
//! cargo test -p outram-foam-basic-lib --lib --release --features parallel \
//!     ldu_matrix::parallel::tests::spmv_crossover_benchmark -- --ignored --nocapture
//! ```

use std::hint::black_box;
use std::sync::Arc;
use std::time::Instant;

use super::*;
use crate::krylov::vecops;

// ── Fixtures ──────────────────────────────────────────────────────────────────

/// xorshift64\* pseudorandom generator — fixed seed, no crate dependency, so
/// every test input is exactly reproducible run to run and machine to machine.
struct Rng(u64);

impl Rng {
    /// Seed the generator. The seed is forced odd, since xorshift64 has a fixed
    /// point at zero.
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A pseudorandom `f64` uniform on `[-1, 1)`.
    fn signed_unit(&mut self) -> f64 {
        let u = (self.next_u64() >> 11) as f64 / (1_u64 << 53) as f64;
        u.mul_add(2.0, -1.0)
    }

    /// A pseudorandom vector of `n` elements uniform on `[-1, 1)`.
    fn vector(&mut self, n: usize) -> Vec<f64> {
        (0..n).map(|_| self.signed_unit()).collect()
    }
}

/// Owner/neighbour addressing of a structured `nx * ny * nz` hexahedral mesh
/// with a 7-point stencil — the canonical finite-volume connectivity.
///
/// Faces are emitted in cell-major order with `owner < neighbour`, which is the
/// OpenFOAM convention [`LduMatrix`] documents.
fn structured_faces(nx: usize, ny: usize, nz: usize) -> (Vec<usize>, Vec<usize>) {
    let id = |i: usize, j: usize, k: usize| (k * ny + j) * nx + i;
    let mut owner = Vec::new();
    let mut neighbour = Vec::new();
    for k in 0..nz {
        for j in 0..ny {
            for i in 0..nx {
                let c = id(i, j, k);
                if i + 1 < nx {
                    owner.push(c);
                    neighbour.push(id(i + 1, j, k));
                }
                if j + 1 < ny {
                    owner.push(c);
                    neighbour.push(id(i, j + 1, k));
                }
                if k + 1 < nz {
                    owner.push(c);
                    neighbour.push(id(i, j, k + 1));
                }
            }
        }
    }
    (owner, neighbour)
}

/// A diagonally dominant 7-point-stencil matrix on a structured mesh, with
/// pseudorandom coefficients from a fixed seed.
///
/// `symmetric` chooses whether `lower == upper` (a Laplacian-like operator) or
/// the two differ (a convection-diffusion-like operator). Both are exercised,
/// because a bug that reads `upper` where it should read `lower` is invisible on
/// a symmetric matrix.
fn random_matrix(nx: usize, ny: usize, nz: usize, seed: u64, symmetric: bool) -> LduMatrix {
    let (owner, neighbour) = structured_faces(nx, ny, nz);
    let n_cells = nx * ny * nz;
    let mut m = LduMatrix::new(n_cells, owner, neighbour);
    let mut rng = Rng::new(seed);
    m.diag = (0..n_cells).map(|_| 6.0 + rng.signed_unit()).collect();
    m.upper = (0..m.n_internal_faces)
        .map(|_| -1.0 + 0.25 * rng.signed_unit())
        .collect();
    m.lower = if symmetric {
        m.upper.clone()
    } else {
        (0..m.n_internal_faces)
            .map(|_| -1.0 + 0.25 * rng.signed_unit())
            .collect()
    };
    m
}

/// Assert two `f64` slices are identical **bit for bit**, not merely close.
///
/// Comparing bit patterns rather than values is deliberate: `assert_eq!` on
/// `f64` would let `-0.0` pass for `0.0` and would reject `NaN == NaN`, and the
/// claim under test is exact reproduction, not numerical agreement.
fn assert_bitwise_eq(actual: &[f64], expected: &[f64], what: &str) {
    assert_eq!(actual.len(), expected.len(), "{what}: length mismatch");
    for (i, (a, e)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(
            a.to_bits(),
            e.to_bits(),
            "{what}: element {i} differs bitwise: {a:e} (0x{:016x}) vs {e:e} (0x{:016x})",
            a.to_bits(),
            e.to_bits()
        );
    }
}

/// Relative difference `|a - b| / max(|a|, |b|)`, or `0.0` when both are zero.
fn rel_diff(a: f64, b: f64) -> f64 {
    let scale = a.abs().max(b.abs());
    if scale == 0.0 {
        0.0
    } else {
        (a - b).abs() / scale
    }
}

/// Best per-call wall-clock time, in seconds, over `reps` samples of `iters`
/// back-to-back calls each.
///
/// Two deliberate choices:
///
/// - **`iters` calls per sample**, so a kernel that takes well under a
///   microsecond is still measured against a clock reading of milliseconds. A
///   single [`Instant::now`] pair around one sub-microsecond call measures mostly
///   the clock.
/// - **Best of, not mean of.** Every source of noise on a shared machine — and
///   this one may be running other builds — *adds* time, so the minimum is the
///   sample least contaminated by something other than the kernel.
fn best_per_call(reps: usize, iters: usize, mut f: impl FnMut()) -> f64 {
    let iters = iters.max(1);
    let mut best = f64::INFINITY;
    for _ in 0..reps {
        let start = Instant::now();
        for _ in 0..iters {
            f();
        }
        let per_call = start.elapsed().as_secs_f64() / iters as f64;
        if per_call < best {
            best = per_call;
        }
    }
    best
}

/// How many back-to-back calls to time in one sample, so every size does roughly
/// the same total amount of work (about 8 million element updates).
fn iters_for(work_items: usize) -> usize {
    (8_000_000 / work_items.max(1)).max(3)
}

/// Force the multi-CPU path regardless of size, for tests and benchmarks that
/// must exercise it below the production size floor.
const FORCE_PARALLEL: usize = 0;

// ── 1. Topology ───────────────────────────────────────────────────────────────

/// The cell-gather index inverts the face addressing exactly: each cell lists
/// every face incident on it, in ascending face order, with the correct
/// other-cell and upper/lower side.
#[test]
fn topology_inverts_face_addressing() {
    let m = random_matrix(4, 3, 2, 0xA11CE, true);
    let topo = LduTopology::from_matrix(&m);

    assert_eq!(topo.n_cells(), 24);
    assert_eq!(topo.n_internal_faces(), m.n_internal_faces);

    // Every face must appear exactly twice across all cells, once per side.
    let mut seen_upper = vec![0_usize; m.n_internal_faces];
    let mut seen_lower = vec![0_usize; m.n_internal_faces];
    let mut total_entries = 0;
    for c in 0..topo.n_cells() {
        let mut last_face: Option<usize> = None;
        for e in topo.row_start[c]..topo.row_start[c + 1] {
            let f = topo.entry_face[e];
            if let Some(prev) = last_face {
                assert!(prev < f, "cell {c}: faces are not in ascending order");
            }
            last_face = Some(f);
            if topo.entry_uses_upper[e] {
                assert_eq!(m.owner[f], c);
                assert_eq!(m.neighbour[f], topo.entry_other[e]);
                seen_upper[f] += 1;
            } else {
                assert_eq!(m.neighbour[f], c);
                assert_eq!(m.owner[f], topo.entry_other[e]);
                seen_lower[f] += 1;
            }
            total_entries += 1;
        }
        assert_eq!(
            topo.incident_face_count(c),
            topo.row_start[c + 1] - topo.row_start[c]
        );
    }
    assert_eq!(total_entries, 2 * m.n_internal_faces);
    assert!(seen_upper.iter().all(|&n| n == 1));
    assert!(seen_lower.iter().all(|&n| n == 1));
    assert!(topo.index_bytes() > 0);
}

/// `matches` accepts a reassembly of the same mesh and rejects a different one.
#[test]
fn topology_matches_detects_changed_addressing() {
    let m = random_matrix(3, 3, 3, 0xBEE5, true);
    let topo = LduTopology::from_matrix(&m);

    // Same addressing, different coefficients — must match.
    let mut reassembled = m.clone();
    reassembled.diag.iter_mut().for_each(|d| *d *= 3.0);
    assert!(topo.matches(&reassembled));

    // A different mesh — must not match.
    let other = random_matrix(4, 3, 3, 0xBEE5, true);
    assert!(!topo.matches(&other));

    // Same face count, one face rewired — must not match.
    let mut rewired = m.clone();
    rewired.neighbour[0] = rewired.owner[0];
    assert!(!topo.matches(&rewired));
}

/// Corrupt addressing is rejected at index-build time rather than producing a
/// silently wrong answer later.
#[test]
#[should_panic(expected = "must be exactly one of each per internal face")]
fn topology_rejects_mismatched_owner_neighbour_lengths() {
    let m = LduMatrix {
        n_cells: 3,
        n_internal_faces: 2,
        diag: vec![1.0; 3],
        lower: vec![0.0; 2],
        upper: vec![0.0; 2],
        owner: vec![0, 1],
        neighbour: vec![1],
    };
    let _ = LduTopology::from_matrix(&m);
}

/// An out-of-range cell index is rejected at index-build time.
#[test]
#[should_panic(expected = "but the matrix has only")]
fn topology_rejects_out_of_range_cell() {
    let m = LduMatrix::new(2, vec![0], vec![5]);
    let _ = LduTopology::from_matrix(&m);
}

// ── 2. Bitwise parity with the serial oracle ──────────────────────────────────

/// **Methodology.** For symmetric and asymmetric 7-point-stencil matrices on
/// meshes from 24 to 32 768 cells, compute `A x` through [`HybridLdu::spmv`] on
/// both backends — with the size floor forced to zero so the multi-CPU path is
/// genuinely taken — and compare against [`LduMatrix::multiply`], the crate's
/// pre-existing serial reference. **Pass criterion: bitwise equality of every
/// element** (`f64::to_bits`), not a tolerance.
///
/// **Result, 2026-08-12:** passes at every size and for both symmetry settings,
/// under default features and under `--features parallel`. **Interpretation:**
/// the cell-gather reformulation reproduces the serial face-scatter exactly, as
/// the ascending-face-order argument in the module documentation predicts, so
/// switching a solver to the multi-CPU backend cannot change its iteration
/// history.
#[test]
fn spmv_is_bitwise_identical_to_serial_reference() {
    for &(nx, ny, nz) in &[(4, 3, 2), (8, 8, 8), (32, 32, 32)] {
        for &symmetric in &[true, false] {
            let m = Arc::new(random_matrix(nx, ny, nz, 0xC0FFEE, symmetric));
            let ldu = HybridLdu::new(Arc::clone(&m));
            let mut rng = Rng::new(0xF00D);
            let x = rng.vector(m.n_cells);

            let reference = m.multiply(&x);
            let what = format!("spmv {nx}x{ny}x{nz} symmetric={symmetric}");

            let mut y = vec![0.0; m.n_cells];
            ldu.spmv_into_min(&x, &mut y, ComputeBackend::Serial, FORCE_PARALLEL);
            assert_bitwise_eq(&y, &reference, &format!("{what} serial"));

            ldu.spmv_into_min(&x, &mut y, ComputeBackend::CpuMulti, FORCE_PARALLEL);
            assert_bitwise_eq(&y, &reference, &format!("{what} cpu-multi"));

            // The public entry point, with the production size floor.
            assert_bitwise_eq(
                &ldu.spmv(&x, ComputeBackend::CpuMulti),
                &reference,
                &format!("{what} public"),
            );
        }
    }
}

/// The residual kernel is bitwise identical to [`LduMatrix::residual`] on both
/// backends.
///
/// **Methodology / result:** as
/// [`spmv_is_bitwise_identical_to_serial_reference`], for `r = b - A x` on a
/// 32x32x32 asymmetric matrix with pseudorandom `x` and `b`. Pass criterion is
/// bitwise equality; passed 2026-08-12 on both feature settings.
#[test]
fn residual_is_bitwise_identical_to_serial_reference() {
    let m = Arc::new(random_matrix(32, 32, 32, 0x5EED, false));
    let ldu = HybridLdu::new(Arc::clone(&m));
    let mut rng = Rng::new(0x1234);
    let x = rng.vector(m.n_cells);
    let b = rng.vector(m.n_cells);

    let reference = m.residual(&x, &b);
    let mut r = vec![0.0; m.n_cells];

    ldu.residual_into_min(&x, &b, &mut r, ComputeBackend::Serial, FORCE_PARALLEL);
    assert_bitwise_eq(&r, &reference, "residual serial");

    ldu.residual_into_min(&x, &b, &mut r, ComputeBackend::CpuMulti, FORCE_PARALLEL);
    assert_bitwise_eq(&r, &reference, "residual cpu-multi");

    assert_bitwise_eq(
        &ldu.residual(&x, &b, ComputeBackend::CpuMulti),
        &reference,
        "residual public",
    );
}

/// The diagonal reciprocal agrees bitwise across backends and reports a singular
/// row as an infinity rather than hiding it.
#[test]
fn diagonal_reciprocal_agrees_and_reports_singular_rows() {
    let mut m = random_matrix(16, 16, 16, 0xD1A6, true);
    m.diag[7] = 0.0; // singular row
    m.diag[9] = f64::NAN; // poisoned row
    let m = Arc::new(m);
    let ldu = HybridLdu::new(Arc::clone(&m));

    let mut serial = vec![0.0; m.n_cells];
    let mut multi = vec![0.0; m.n_cells];
    ldu.diagonal_reciprocal_into_min(&mut serial, ComputeBackend::Serial, FORCE_PARALLEL);
    ldu.diagonal_reciprocal_into_min(&mut multi, ComputeBackend::CpuMulti, FORCE_PARALLEL);
    assert_bitwise_eq(&multi, &serial, "diagonal reciprocal");

    assert!(
        serial[7].is_infinite(),
        "zero diagonal must give an infinity"
    );
    assert!(serial[9].is_nan(), "NaN diagonal must propagate");
    assert_eq!(serial[0], 1.0 / m.diag[0]);
}

/// `axpy` is bitwise identical across backends *and* to
/// [`crate::krylov::vecops::axpy`], because it performs no reduction.
#[test]
fn axpy_is_bitwise_identical_across_backends_and_to_vecops() {
    let mut rng = Rng::new(0xAABB);
    let n = 100_000;
    let x = rng.vector(n);
    let y0 = rng.vector(n);
    let alpha = 0.375_f64;

    let mut reference = y0.clone();
    vecops::axpy(alpha, &x, &mut reference);

    let mut serial = y0.clone();
    axpy_min(
        alpha,
        &x,
        &mut serial,
        ComputeBackend::Serial,
        FORCE_PARALLEL,
    );
    assert_bitwise_eq(&serial, &reference, "axpy serial vs vecops");

    let mut multi = y0.clone();
    axpy_min(
        alpha,
        &x,
        &mut multi,
        ComputeBackend::CpuMulti,
        FORCE_PARALLEL,
    );
    assert_bitwise_eq(&multi, &reference, "axpy cpu-multi vs vecops");
}

/// Every reduction returns a bit-for-bit identical value on both backends.
///
/// This is the property that makes the blocked summation worth its cost: a
/// solver's residual history does not change when the backend does.
#[test]
fn reductions_are_bitwise_identical_across_backends() {
    let mut rng = Rng::new(0xC0DE);
    for &n in &[0_usize, 1, 1023, 1024, 1025, 100_000, 1_000_003] {
        let a = rng.vector(n);
        let b = rng.vector(n);

        let d_serial = dot_min(&a, &b, ComputeBackend::Serial, FORCE_PARALLEL);
        let d_multi = dot_min(&a, &b, ComputeBackend::CpuMulti, FORCE_PARALLEL);
        assert_eq!(d_serial.to_bits(), d_multi.to_bits(), "dot at n = {n}");

        let l1_serial = norm_l1_min(&a, ComputeBackend::Serial, FORCE_PARALLEL);
        let l1_multi = norm_l1_min(&a, ComputeBackend::CpuMulti, FORCE_PARALLEL);
        assert_eq!(
            l1_serial.to_bits(),
            l1_multi.to_bits(),
            "norm_l1 at n = {n}"
        );

        assert_eq!(
            norm_l2(&a, ComputeBackend::Serial).to_bits(),
            norm_l2(&a, ComputeBackend::CpuMulti).to_bits(),
            "norm_l2 at n = {n}"
        );
    }
}

/// The reductions agree with hand-computed values on inputs small enough to
/// check by eye, and behave on empty input.
#[test]
fn reductions_match_hand_computed_values() {
    for backend in [ComputeBackend::Serial, ComputeBackend::CpuMulti] {
        assert_eq!(dot(&[1.0, 2.0, 3.0], &[4.0, -5.0, 6.0], backend), 12.0);
        assert_eq!(norm_l2(&[3.0, 4.0], backend), 5.0);
        assert_eq!(norm_l1(&[1.0, -2.0, 3.0], backend), 6.0);
        assert_eq!(dot(&[], &[], backend), 0.0);
        assert_eq!(norm_l1(&[], backend), 0.0);
        assert_eq!(norm_l2(&[], backend), 0.0);
    }
}

// ── 3. Measured deviation from the flat-sum references ────────────────────────

/// **V&V: blocked `dot` versus the flat-sum reference.**
///
/// **Methodology.** Two fixed-seed xorshift64\* pseudorandom vectors with
/// elements uniform on `[-1, 1)`, at lengths 1 024, 4 096, 16 384, 65 536,
/// 262 144, 1 048 576 and 4 194 304. Compute [`dot`] (blocked, block size
/// [`REDUCTION_BLOCK`]) and [`crate::krylov::vecops::dot`] (flat left-to-right)
/// on the same inputs and take the relative difference
/// `|a - b| / max(|a|, |b|)` and, because that measure is taken against a
/// heavily cancelled sum and overstates the error, also the conditioning-aware
/// `|a - b| / sum |a_i b_i|`. **Pass criteria: worst raw relative difference
/// `<= 1e-12` and worst conditioned difference `<= 1e-15`.**
///
/// **Result (measured 2026-08-12, printed by this test):** worst raw 2.3744e-13
/// at n = 4 194 304, worst conditioned 7.9476e-17; the full table is transcribed
/// onto [`dot`]. **Interpretation:** the difference
/// is summation reassociation at rounding level, not a defect. Blocked summation
/// is the two-level, more accurate form — a flat sum of `n` terms has error
/// growing like `n * eps`, a blocked sum like `(block + n/block) * eps` — so the
/// blocked result is if anything closer to the exact value. The two forms are
/// interchangeable for solver purposes, but they are not interchangeable
/// bit for bit, and this test exists to keep that fact measured rather than
/// assumed.
#[test]
fn dot_matches_flat_reference() {
    let mut worst = 0.0_f64;
    let mut worst_n = 0;
    let mut worst_conditioned = 0.0_f64;
    println!("dot: blocked (this module) vs flat (krylov::vecops)");
    println!(
        "{:>10}  {:>24}  {:>24}  {:>12}  {:>12}",
        "n", "blocked", "flat", "rel diff", "vs sum|a.b|"
    );
    let mut n = 1024_usize;
    while n <= 4_194_304 {
        let mut rng = Rng::new(0x51EED ^ n as u64);
        let a = rng.vector(n);
        let b = rng.vector(n);
        let blocked = dot(&a, &b, ComputeBackend::Serial);
        let flat = vecops::dot(&a, &b);
        let d = rel_diff(blocked, flat);
        // The raw relative difference is measured against a heavily cancelled
        // sum (terms of order 1, total of order sqrt(n)), so it overstates the
        // error. Scaling by the sum of absolute terms is the conditioning-aware
        // measure: it is the error relative to the size of the arithmetic done.
        let magnitude: f64 = a.iter().zip(b.iter()).map(|(x, y)| (x * y).abs()).sum();
        let conditioned = (blocked - flat).abs() / magnitude;
        println!("{n:>10}  {blocked:>24.17e}  {flat:>24.17e}  {d:>12.4e}  {conditioned:>12.4e}");
        if d > worst {
            worst = d;
            worst_n = n;
        }
        worst_conditioned = worst_conditioned.max(conditioned);
        n *= 4;
    }
    println!(
        "dot worst relative difference: {worst:.4e} at n = {worst_n}; \
         worst conditioned difference: {worst_conditioned:.4e}"
    );
    assert!(
        worst <= 1e-12,
        "blocked dot deviates from the flat reference by {worst:e} (gate 1e-12) at n = {worst_n}"
    );
    assert!(
        worst_conditioned <= 1e-15,
        "blocked dot deviates from the flat reference by {worst_conditioned:e} \
         relative to sum|a.b| (gate 1e-15)"
    );
}

/// **V&V: blocked `norm_l1` / `norm_l2` versus their flat-sum references.**
///
/// **Methodology.** As [`dot_matches_flat_reference`], comparing [`norm_l1`]
/// against a flat `sum abs(x_i)` and [`norm_l2`] against
/// [`crate::krylov::vecops::nrm2`], at length 4 194 304. **Pass criterion:
/// relative difference `<= 1e-12` for both.** **Result:** printed by this test;
/// see its output line. **Interpretation:** as for `dot` — reassociation only.
#[test]
fn norms_match_flat_references() {
    let n = 4_194_304;
    let mut rng = Rng::new(0x7071);
    let x = rng.vector(n);

    let blocked_l1 = norm_l1(&x, ComputeBackend::Serial);
    let flat_l1: f64 = x.iter().map(|v| v.abs()).sum();
    let d1 = rel_diff(blocked_l1, flat_l1);

    let blocked_l2 = norm_l2(&x, ComputeBackend::Serial);
    let flat_l2 = vecops::nrm2(&x);
    let d2 = rel_diff(blocked_l2, flat_l2);

    println!("norm_l1 blocked {blocked_l1:.17e} flat {flat_l1:.17e} rel diff {d1:.4e}");
    println!("norm_l2 blocked {blocked_l2:.17e} flat {flat_l2:.17e} rel diff {d2:.4e}");

    assert!(d1 <= 1e-12, "norm_l1 deviates by {d1:e} (gate 1e-12)");
    assert!(d2 <= 1e-12, "norm_l2 deviates by {d2:e} (gate 1e-12)");
}

/// **V&V: blocked `normalised_residual` versus
/// [`LduMatrix::normalised_residual`].**
///
/// **Methodology.** A fixed-seed pseudorandom, diagonally dominant
/// 7-point-stencil matrix on a 32x32x32 mesh (32 768 cells), with pseudorandom
/// `x` and `b` uniform on `[-1, 1)`. Compare
/// [`HybridLdu::normalised_residual`] (blocked reduction) against
/// [`LduMatrix::normalised_residual`] (flat sums) on the same inputs.
/// **Pass criterion: relative difference `<= 1e-13`.**
///
/// **Result (measured 2026-08-12, printed by this test):** recorded on
/// [`HybridLdu::normalised_residual`]. **Interpretation:** the convergence
/// measure a solver tests against a tolerance is unchanged for any practical
/// tolerance — solver tolerances in this crate are `1e-6` to `1e-12` on a
/// quantity whose backends differ in the sixteenth digit.
#[test]
fn normalised_residual_matches_flat_reference() {
    let m = Arc::new(random_matrix(32, 32, 32, 0x9911, true));
    let ldu = HybridLdu::new(Arc::clone(&m));
    let mut rng = Rng::new(0x2468);
    let x = rng.vector(m.n_cells);
    let b = rng.vector(m.n_cells);

    let blocked_serial =
        ldu.normalised_residual_min(&x, &b, ComputeBackend::Serial, FORCE_PARALLEL);
    let blocked_multi =
        ldu.normalised_residual_min(&x, &b, ComputeBackend::CpuMulti, FORCE_PARALLEL);
    let flat = m.normalised_residual(&x, &b);
    let d = rel_diff(blocked_serial, flat);

    println!(
        "normalised_residual blocked {blocked_serial:.17e} flat {flat:.17e} rel diff {d:.4e} \
         ({:.2} ulp)",
        (blocked_serial - flat).abs() / f64::EPSILON / blocked_serial.abs()
    );

    assert_eq!(
        blocked_serial.to_bits(),
        blocked_multi.to_bits(),
        "normalised_residual must be bitwise identical across backends"
    );
    assert!(
        d <= 1e-13,
        "normalised_residual deviates from the flat reference by {d:e} (gate 1e-13)"
    );
}

// ── 4. Dispatch behaviour ─────────────────────────────────────────────────────

/// The dispatch predicates report what will actually run, and never claim a
/// backend that is not available in this build.
#[test]
fn dispatch_predicates_are_honest() {
    // Below the floor, always serial.
    assert_eq!(
        spmv_backend_for(ComputeBackend::CpuMulti, SPMV_MIN_CELLS - 1),
        ComputeBackend::Serial
    );
    assert_eq!(
        vecop_backend_for(ComputeBackend::CpuMulti, VECOP_MIN_ELEMENTS - 1),
        ComputeBackend::Serial
    );

    // At or above the floor, multi-CPU exactly when the feature is compiled in.
    let expected = if cfg!(feature = "parallel") {
        ComputeBackend::CpuMulti
    } else {
        ComputeBackend::Serial
    };
    assert_eq!(
        spmv_backend_for(ComputeBackend::CpuMulti, SPMV_MIN_CELLS),
        expected
    );
    assert_eq!(
        vecop_backend_for(ComputeBackend::CpuMulti, VECOP_MIN_ELEMENTS),
        expected
    );

    // A GPU request never claims the GPU here — there is no GPU kernel yet.
    assert_ne!(
        spmv_backend_for(ComputeBackend::Gpu, 1 << 24),
        ComputeBackend::Gpu
    );
    assert_eq!(
        spmv_backend_for(ComputeBackend::Gpu, 1 << 24),
        spmv_backend_for(ComputeBackend::CpuMulti, 1 << 24)
    );

    // Explicit Serial stays serial at any size.
    assert_eq!(
        spmv_backend_for(ComputeBackend::Serial, 1 << 24),
        ComputeBackend::Serial
    );
}

/// `with_matrix` reuses the index for a reassembly and refuses a different mesh.
#[test]
fn with_matrix_reuses_index_and_refuses_a_different_mesh() {
    let m = Arc::new(random_matrix(8, 8, 8, 0x3333, true));
    let ldu = HybridLdu::new(Arc::clone(&m));

    let mut reassembled = (*m).clone();
    reassembled.diag.iter_mut().for_each(|d| *d += 1.0);
    let reassembled = Arc::new(reassembled);
    let ldu2 = ldu
        .with_matrix(Arc::clone(&reassembled))
        .expect("same mesh");

    // Same index object, shared not rebuilt.
    assert!(Arc::ptr_eq(ldu.topology(), ldu2.topology()));

    let mut rng = Rng::new(0x777);
    let x = rng.vector(m.n_cells);
    assert_bitwise_eq(
        &ldu2.spmv(&x, ComputeBackend::CpuMulti),
        &reassembled.multiply(&x),
        "spmv after with_matrix",
    );

    assert!(ldu
        .with_matrix(Arc::new(random_matrix(8, 8, 9, 0x3333, true)))
        .is_none());
}

/// Degenerate inputs — an empty matrix and a matrix with no internal faces — are
/// handled rather than panicking or producing garbage.
#[test]
fn degenerate_matrices_behave() {
    let empty = Arc::new(LduMatrix::new(0, vec![], vec![]));
    let ldu = HybridLdu::new(Arc::clone(&empty));
    assert!(ldu.spmv(&[], ComputeBackend::CpuMulti).is_empty());
    assert!(ldu.diagonal_reciprocal(ComputeBackend::CpuMulti).is_empty());
    // No cells: the residual norm degenerates to the unscaled (zero) L1 norm.
    assert_eq!(
        ldu.normalised_residual(&[], &[], ComputeBackend::Serial),
        0.0
    );

    let mut diag_only = LduMatrix::new(3, vec![], vec![]);
    diag_only.diag = vec![2.0, 3.0, 4.0];
    let diag_only = Arc::new(diag_only);
    let ldu = HybridLdu::new(Arc::clone(&diag_only));
    let x = vec![1.0, 1.0, 1.0];
    assert_bitwise_eq(
        &ldu.spmv(&x, ComputeBackend::CpuMulti),
        &diag_only.multiply(&x),
        "diagonal-only spmv",
    );
    assert_eq!(ldu.auto_backend().is_available(), true);
}

/// A length mismatch is caught by an assertion rather than silently truncating.
#[test]
#[should_panic(expected = "expected n_cells")]
fn spmv_rejects_a_wrong_length_input() {
    let m = Arc::new(random_matrix(2, 2, 2, 1, true));
    let ldu = HybridLdu::new(m);
    let _ = ldu.spmv(&[1.0, 2.0], ComputeBackend::Serial);
}

// ── 5. Crossover benchmarks (ignored) ─────────────────────────────────────────

/// Absolute serial-versus-multi-CPU timings for [`HybridLdu::spmv_into`], the
/// evidence behind [`SPMV_MIN_CELLS`].
///
/// `#[ignore]`d because it is a measurement, not a gate, and takes about 31 s
/// of wall clock (measured 2026-08-12 on 4 idle cores). Run with:
///
/// ```text
/// cargo test -p outram-foam-basic-lib --lib --release --features parallel \
///     spmv_crossover_benchmark -- --ignored --nocapture
/// ```
///
/// The multi-CPU column is measured with the size floor forced to zero, so the
/// parallel path runs even at sizes where production code would not use it —
/// which is the only way to locate the crossover. Under default features the two
/// columns measure the same code and the speed-up is 1.0 by construction; the
/// numbers transcribed onto [`SPMV_MIN_CELLS`] come from a `--features parallel`
/// run.
#[test]
#[ignore = "measurement, ~31 s; run explicitly with --ignored --nocapture"]
fn spmv_crossover_benchmark() {
    let cores = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    println!("available_parallelism = {cores}");
    println!(
        "parallel feature compiled in: {}",
        cfg!(feature = "parallel")
    );
    println!(
        "{:>9}  {:>10}  {:>12}  {:>12}  {:>9}",
        "cells", "faces", "serial (us)", "multi (us)", "speed-up"
    );

    for &n in &[8_usize, 10, 12, 14, 16, 18, 20, 25, 32, 40, 51, 64, 80] {
        let m = Arc::new(random_matrix(n, n, n, 0x9E3779B9, true));
        let ldu = HybridLdu::new(Arc::clone(&m));
        let mut rng = Rng::new(0x1357);
        let x = rng.vector(m.n_cells);
        let mut y = vec![0.0; m.n_cells];

        // Warm the caches and, under `--features parallel`, rayon's global pool,
        // so pool construction is not charged to the first timed call.
        ldu.spmv_into_min(&x, &mut y, ComputeBackend::CpuMulti, FORCE_PARALLEL);

        let iters = iters_for(m.n_cells);
        let serial = best_per_call(7, iters, || {
            ldu.spmv_into_min(&x, &mut y, ComputeBackend::Serial, FORCE_PARALLEL);
            black_box(y[0]);
        });
        let multi = best_per_call(7, iters, || {
            ldu.spmv_into_min(&x, &mut y, ComputeBackend::CpuMulti, FORCE_PARALLEL);
            black_box(y[0]);
        });

        println!(
            "{:>9}  {:>10}  {:>12.2}  {:>12.2}  {:>9.2}",
            m.n_cells,
            m.n_internal_faces,
            serial * 1e6,
            multi * 1e6,
            serial / multi
        );
    }
}

/// Absolute serial-versus-multi-CPU timings for [`dot`] and [`axpy`], the
/// evidence behind [`VECOP_MIN_ELEMENTS`].
///
/// `#[ignore]`d for the same reason as [`spmv_crossover_benchmark`]; takes about
/// 7 s of wall clock (measured 2026-08-12 on 4 idle cores).
#[test]
#[ignore = "measurement, ~7 s; run explicitly with --ignored --nocapture"]
fn vecop_crossover_benchmark() {
    let cores = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    println!("available_parallelism = {cores}");
    println!(
        "parallel feature compiled in: {}",
        cfg!(feature = "parallel")
    );
    println!(
        "{:>10}  {:>13}  {:>13}  {:>9}  {:>14}  {:>14}  {:>9}",
        "n",
        "dot ser (us)",
        "dot mul (us)",
        "speed-up",
        "axpy ser (us)",
        "axpy mul (us)",
        "speed-up"
    );

    let mut n = 1024_usize;
    while n <= 4_194_304 {
        let mut rng = Rng::new(0xABCD ^ n as u64);
        let a = rng.vector(n);
        let b = rng.vector(n);
        let mut y = rng.vector(n);

        black_box(dot_min(&a, &b, ComputeBackend::CpuMulti, FORCE_PARALLEL));

        let iters = iters_for(n);
        let dot_ser = best_per_call(7, iters, || {
            black_box(dot_min(&a, &b, ComputeBackend::Serial, FORCE_PARALLEL));
        });
        let dot_mul = best_per_call(7, iters, || {
            black_box(dot_min(&a, &b, ComputeBackend::CpuMulti, FORCE_PARALLEL));
        });
        let axpy_ser = best_per_call(7, iters, || {
            axpy_min(1e-9, &a, &mut y, ComputeBackend::Serial, FORCE_PARALLEL);
            black_box(y[0]);
        });
        let axpy_mul = best_per_call(7, iters, || {
            axpy_min(1e-9, &a, &mut y, ComputeBackend::CpuMulti, FORCE_PARALLEL);
            black_box(y[0]);
        });

        println!(
            "{n:>10}  {:>13.2}  {:>13.2}  {:>9.2}  {:>14.2}  {:>14.2}  {:>9.2}",
            dot_ser * 1e6,
            dot_mul * 1e6,
            dot_ser / dot_mul,
            axpy_ser * 1e6,
            axpy_mul * 1e6,
            axpy_ser / axpy_mul
        );
        n *= 2;
    }
}

/// How the sparse product scales with worker-thread count, and confirmation that
/// the answer does not change with it.
///
/// Only meaningful under `--features parallel`; without it there is one pool of
/// one thread and the test simply reports the serial time. `#[ignore]`d
/// (measurement, about 4 s measured 2026-08-12 on 4 idle cores).
#[test]
#[ignore = "measurement, ~4 s; run explicitly with --ignored --nocapture"]
fn spmv_thread_scaling_benchmark() {
    let cores = std::thread::available_parallelism()
        .map(std::num::NonZeroUsize::get)
        .unwrap_or(1);
    println!("available_parallelism = {cores}");

    let m = Arc::new(random_matrix(64, 64, 64, 0x2244, true));
    let ldu = HybridLdu::new(Arc::clone(&m));
    let mut rng = Rng::new(0x9090);
    let x = rng.vector(m.n_cells);
    let mut y = vec![0.0; m.n_cells];

    ldu.spmv_into_min(&x, &mut y, ComputeBackend::Serial, FORCE_PARALLEL);
    let reference = y.clone();
    let iters = iters_for(m.n_cells);
    let serial = best_per_call(7, iters, || {
        ldu.spmv_into_min(&x, &mut y, ComputeBackend::Serial, FORCE_PARALLEL);
        black_box(y[0]);
    });
    assert_bitwise_eq(&y, &reference, "serial spmv is run-to-run deterministic");
    println!("cells = {}, faces = {}", m.n_cells, m.n_internal_faces);
    println!("{:>8}  {:>12}  {:>9}", "threads", "time (us)", "speed-up");
    println!("{:>8}  {:>12.2}  {:>9.2}", 1, serial * 1e6, 1.0);

    #[cfg(feature = "parallel")]
    for threads in [2_usize, 4, 8] {
        let pool = match rayon::ThreadPoolBuilder::new().num_threads(threads).build() {
            Ok(p) => p,
            Err(e) => {
                println!("{threads:>8}  pool build failed: {e}");
                continue;
            }
        };
        let t = pool.install(|| {
            ldu.spmv_into_min(&x, &mut y, ComputeBackend::CpuMulti, FORCE_PARALLEL);
            best_per_call(7, iters, || {
                ldu.spmv_into_min(&x, &mut y, ComputeBackend::CpuMulti, FORCE_PARALLEL);
                black_box(y[0]);
            })
        });
        assert_bitwise_eq(&y, &reference, "spmv is thread-count independent");
        println!("{threads:>8}  {:>12.2}  {:>9.2}", t * 1e6, serial / t);
    }
}
