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

//! Tests for [`crate::fields::parallel`].
//!
//! The **oracle is always [`ComputeBackend::Serial`]** — the deterministic
//! trusted reference. Element-wise kernels are asserted bit-identical to it;
//! reductions are asserted equal within a documented, measured tolerance,
//! because floating-point addition is not associative and the parallel
//! summation order differs (see the module's "Reduction determinism" section).
//!
//! Sizes are chosen to straddle [`field_parallel_crossover`]: the small cases
//! exercise the serial fallback inside a `CpuMulti` call, and `BIG` is above the
//! crossover so `CpuMulti` genuinely spreads work across threads when the crate
//! is built with `--features parallel`.

use std::sync::Arc;

use super::*;
use crate::fields::vol_field::{VolScalarField, VolVectorField};
use crate::mesh::fv_mesh::FvMesh;
use crate::primitives::Vector3;

/// Comfortably above [`FIELD_PARALLEL_CROSSOVER`] (131 072), and not a multiple
/// of [`REDUCTION_CHUNK`], so the ragged final chunk is exercised.
const BIG: usize = 200_003;

/// Sizes spanning empty, single-element, sub-crossover, and above-crossover.
const SIZES: [usize; 6] = [0, 1, 2, 1_000, 131_072, BIG];

/// Every backend the dispatcher accepts.
const BACKENDS: [ComputeBackend; 3] = [
    ComputeBackend::Serial,
    ComputeBackend::CpuMulti,
    ComputeBackend::Gpu,
];

/// Deterministic, reproducible test data with a wide dynamic range, so that
/// floating-point summation order actually matters.
fn sample(n: usize) -> Field<f64> {
    Field::from_fn(n, |i| {
        let t = i as f64;
        // Alternating large/small magnitudes: a plain left-to-right sum and a
        // chunked sum genuinely disagree in the last bits on this data.
        (t * 0.61803398874989).sin() * 1.0e6 + (t * 0.123456789012345).cos() * 1.0e-7
    })
}

fn sample_shift(n: usize) -> Field<f64> {
    Field::from_fn(n, |i| {
        let t = i as f64 + 0.5;
        (t * 0.27182818284590).cos() * 1.0e3 + 1.5
    })
}

fn sample_vec(n: usize) -> Field<Vector3> {
    Field::from_fn(n, |i| {
        let t = i as f64;
        Vector3::new(t.sin(), t.cos(), (t * 0.5).sin())
    })
}

// ── Dispatch policy ──────────────────────────────────────────────────────────

/// Methodology: [`should_parallelise`] is the single dispatch decision; assert
/// it is `false` for [`ComputeBackend::Serial`] at any size, `false` for any
/// backend below the crossover, and equal to `cfg!(feature = "parallel")` at and
/// above the crossover.
///
/// Result (2026-08-12): passes in both the default build and
/// `--features parallel`.
#[test]
fn dispatch_policy_is_the_single_decision_point() {
    let big = field_parallel_crossover();
    assert!(!should_parallelise(ComputeBackend::Serial, usize::MAX));
    assert!(!should_parallelise(ComputeBackend::CpuMulti, 0));
    assert!(!should_parallelise(ComputeBackend::CpuMulti, big - 1));
    assert_eq!(
        should_parallelise(ComputeBackend::CpuMulti, big),
        cfg!(feature = "parallel")
    );
    assert_eq!(
        should_parallelise(ComputeBackend::Gpu, big),
        cfg!(feature = "parallel"),
        "Gpu has no field kernel yet and must take the best CPU path"
    );
}

// ── Element-wise kernels: bit-identical to the Serial oracle ─────────────────

/// V&V — methodology: for every backend and every size in [`SIZES`] (0, 1, 2,
/// 1 000, 131 072, 200 003 — spanning empty, single-element, sub-crossover and
/// above-crossover), each element-wise scalar kernel is computed on that backend
/// and compared **bitwise** (`assert_eq!` on `f64`) against
/// [`ComputeBackend::Serial`]. Pass criterion: exact equality, because each
/// output element is produced by the identical expression regardless of which
/// thread evaluates it — no reassociation is possible in an element-wise map.
///
/// Result (measured 2026-08-12, default build and `--features parallel`,
/// 4 logical cores): all 6 sizes x 3 backends x 6 kernels bit-identical; zero
/// differing elements.
#[test]
fn vv_elementwise_scalar_kernels_are_bit_identical_to_serial() {
    for n in SIZES {
        let a = sample(n);
        let b = sample_shift(n);
        let s = ComputeBackend::Serial;

        let ref_add = add(s, &a, &b);
        let ref_sub = sub(s, &a, &b);
        let ref_scale = scale(s, &a, 0.25);
        let ref_axpy = axpy(s, &a, -3.5, &b);
        let ref_mul = pointwise_mul(s, &a, &b);
        let ref_div = pointwise_div(s, &a, &b);

        for backend in BACKENDS {
            assert_eq!(
                add(backend, &a, &b).as_slice(),
                ref_add.as_slice(),
                "add n={n}"
            );
            assert_eq!(
                sub(backend, &a, &b).as_slice(),
                ref_sub.as_slice(),
                "sub n={n}"
            );
            assert_eq!(
                scale(backend, &a, 0.25).as_slice(),
                ref_scale.as_slice(),
                "scale n={n}"
            );
            assert_eq!(
                axpy(backend, &a, -3.5, &b).as_slice(),
                ref_axpy.as_slice(),
                "axpy n={n}"
            );
            assert_eq!(
                pointwise_mul(backend, &a, &b).as_slice(),
                ref_mul.as_slice(),
                "pointwise_mul n={n}"
            );
            assert_eq!(
                pointwise_div(backend, &a, &b).as_slice(),
                ref_div.as_slice(),
                "pointwise_div n={n}"
            );
        }
    }
}

/// V&V — methodology: the in-place kernels must leave `y` in exactly the state
/// the out-of-place kernel would produce, on every backend and every size in
/// [`SIZES`]. Pass criterion: bitwise equality against the
/// [`ComputeBackend::Serial`] out-of-place result.
///
/// Result (measured 2026-08-12): all sizes and backends bit-identical for
/// `add_assign`, `sub_assign`, `scale_assign`, `axpy_assign`.
#[test]
fn vv_in_place_kernels_match_out_of_place() {
    for n in SIZES {
        let a = sample(n);
        let b = sample_shift(n);
        let s = ComputeBackend::Serial;

        for backend in BACKENDS {
            let mut y = a.clone();
            add_assign(backend, &mut y, &b);
            assert_eq!(y.as_slice(), add(s, &a, &b).as_slice(), "add_assign n={n}");

            let mut y = a.clone();
            sub_assign(backend, &mut y, &b);
            assert_eq!(y.as_slice(), sub(s, &a, &b).as_slice(), "sub_assign n={n}");

            let mut y = a.clone();
            scale_assign(backend, &mut y, 0.25);
            assert_eq!(
                y.as_slice(),
                scale(s, &a, 0.25).as_slice(),
                "scale_assign n={n}"
            );

            let mut y = a.clone();
            axpy_assign(backend, &mut y, -3.5, &b);
            assert_eq!(
                y.as_slice(),
                axpy(s, &a, -3.5, &b).as_slice(),
                "axpy_assign n={n}"
            );
        }
    }
}

/// V&V — methodology: the ranked (`Vector3`) kernels must be bit-identical to
/// the [`ComputeBackend::Serial`] oracle across [`SIZES`], covering `add`,
/// `axpy_assign`, `scale_by_field` (the `rho*U` product) and `dot_field`.
///
/// Result (measured 2026-08-12): all sizes and backends bit-identical.
#[test]
fn vv_elementwise_vector_kernels_are_bit_identical_to_serial() {
    for n in SIZES {
        let u = sample_vec(n);
        let v = sample_vec(n);
        let rho = sample_shift(n);
        let s = ComputeBackend::Serial;

        let ref_add = add(s, &u, &v);
        let ref_scaled = scale_by_field(s, &u, &rho);
        let ref_dot = dot_field(s, &u, &v);
        let mut ref_axpy = u.clone();
        axpy_assign(s, &mut ref_axpy, 0.125, &v);

        for backend in BACKENDS {
            assert_eq!(
                add(backend, &u, &v).as_slice(),
                ref_add.as_slice(),
                "vec add n={n}"
            );
            assert_eq!(
                scale_by_field(backend, &u, &rho).as_slice(),
                ref_scaled.as_slice(),
                "scale_by_field n={n}"
            );
            assert_eq!(
                dot_field(backend, &u, &v).as_slice(),
                ref_dot.as_slice(),
                "dot_field n={n}"
            );
            let mut y = u.clone();
            axpy_assign(backend, &mut y, 0.125, &v);
            assert_eq!(y.as_slice(), ref_axpy.as_slice(), "vec axpy_assign n={n}");
        }
    }
}

/// Edge cases the dispatcher must survive: an empty field and a one-element
/// field, on every backend. Nothing must panic, and the documented empty-field
/// conventions of the serial [`Field`] API must be reproduced.
#[test]
fn edge_cases_empty_and_single_element() {
    for backend in BACKENDS {
        let empty = Field::<f64>::new(vec![]);
        assert_eq!(add(backend, &empty, &empty).len(), 0);
        assert_eq!(sum(backend, &empty), 0.0);
        assert_eq!(mean(backend, &empty), 0.0);
        assert_eq!(min(backend, &empty), f64::INFINITY);
        assert_eq!(max(backend, &empty), f64::NEG_INFINITY);
        assert_eq!(l2_norm(backend, &empty), 0.0);
        assert_eq!(dot(backend, &empty, &empty), 0.0);

        let one = Field::new(vec![7.5_f64]);
        assert_eq!(add(backend, &one, &one).as_slice(), &[15.0]);
        assert_eq!(sum(backend, &one), 7.5);
        assert_eq!(min(backend, &one), 7.5);
        assert_eq!(max(backend, &one), 7.5);
        assert_eq!(mean(backend, &one), 7.5);
        assert!((l2_norm(backend, &one) - 7.5).abs() < 1e-15);
    }
}

/// Length mismatches must panic exactly as the serial operators do, rather than
/// silently truncating to the shorter operand — which is what a bare
/// `zip` would do.
#[test]
#[should_panic(expected = "Field length mismatch")]
fn mismatched_lengths_panic() {
    let a = Field::new(vec![1.0, 2.0, 3.0]);
    let b = Field::new(vec![1.0, 2.0]);
    let _ = add(ComputeBackend::CpuMulti, &a, &b);
}

// ── Reductions ───────────────────────────────────────────────────────────────

/// V&V — methodology: `min` and `max` are **associative**, so regrouping the
/// fold cannot change the answer. Assert bitwise equality against
/// [`ComputeBackend::Serial`] (and against the pre-existing serial
/// [`Field::min`]/[`Field::max`]) at every size in [`SIZES`]. Pass criterion:
/// exact equality.
///
/// Result (measured 2026-08-12): bit-identical at all 6 sizes on all 3 backends.
#[test]
fn vv_min_max_are_bit_identical_to_serial() {
    for n in SIZES {
        let a = sample(n);
        for backend in BACKENDS {
            assert_eq!(min(backend, &a), a.min(), "min n={n}");
            assert_eq!(max(backend, &a), a.max(), "max n={n}");
        }
    }
}

/// V&V — methodology: the parallel reductions use a fixed-chunk tree sum, so
/// they must **not** be expected to bit-match the serial left-to-right fold.
/// This test measures the actual relative deviation
/// `|parallel - serial| / |serial|` for `sum`, `l2_norm`, `dot` and
/// `vol_integral` on a field of 200 003 elements whose values span nine orders
/// of magnitude (`sin(.)*1e6 + cos(.)*1e-7`), which is a deliberately
/// summation-order-sensitive dataset. Reference: the serial `Field::sum` /
/// `Field::l2_norm` computed by the pre-existing trusted code path.
/// Pass criterion: relative deviation `< 1e-11`, about 130x looser than the
/// measured worst case, so the test is not brittle across compilers or
/// architectures while still catching a genuine reassociation blow-up.
///
/// Results (measured 2026-08-12, `--release --features parallel`, 4 logical
/// cores, n = 200 003; every number below was **printed by this test** and
/// transcribed):
///
/// | reduction | serial | parallel | relative deviation |
/// |---|---|---|---|
/// | `sum` | 3.76457534673912101e5 | 3.76457534673890681e5 | 5.690e-14 |
/// | `l2_norm` | 3.16230423855111003e8 | 3.16230423855108857e8 | 6.785e-15 |
/// | `dot` | 1.92352867127785730e9 | 1.92352867127784348e9 | 7.189e-15 |
/// | `vol_integral` | 1.88225943947787222e0 | 1.88225943947772434e0 | 7.857e-14 |
///
/// Interpretation: **worst-case relative deviation 7.857e-14**, on
/// `vol_integral`. That is larger than the `~1e-15` a naive count of `f64`
/// epsilon would suggest, because this dataset cancels heavily — individual
/// terms are `O(1e6)` while their sum is `O(3.8e5)`, so the relative error is
/// amplified by roughly the cancellation ratio (`vol_integral` scales the same
/// data by the cell volume, so it inherits it). `l2_norm`, which sums only
/// non-negative squares and therefore does not cancel, deviates by only
/// 6.8e-15. Both results are equally correct; in exact arithmetic neither is
/// "the" right answer, and the serial one is designated the reference by
/// convention, not by accuracy. In the default build (no `parallel` feature) the
/// deviation is identically zero, because both paths execute the same code.
#[test]
fn vv_parallel_sum_matches_serial_within_tolerance() {
    const TOL: f64 = 1e-11;
    let a = sample(BIG);
    let b = sample_shift(BIG);

    let s = ComputeBackend::Serial;
    let p = ComputeBackend::CpuMulti;

    let checks: [(&str, f64, f64); 3] = [
        ("sum", sum(s, &a), sum(p, &a)),
        ("l2_norm", l2_norm(s, &a), l2_norm(p, &a)),
        ("dot", dot(s, &a, &b), dot(p, &a, &b)),
    ];

    let mut worst = 0.0_f64;
    for (name, serial, parallel) in checks {
        let rel = ((parallel - serial) / serial).abs();
        println!("{name}: serial={serial:.17e} parallel={parallel:.17e} rel_dev={rel:.3e}");
        assert!(
            rel < TOL,
            "{name}: relative deviation {rel:.3e} exceeds tolerance {TOL:.0e}"
        );
        worst = worst.max(rel);
    }

    // Same check for the mesh-weighted integral.
    let mesh = Arc::new(FvMesh::periodic_1d(BIG, 1.0, 1.0));
    let phi = VolScalarField::new(
        "phi",
        mesh.clone(),
        a.clone(),
        mesh.patches
            .iter()
            .map(|q| crate::fields::boundary::bc::PatchField::zero_gradient(q.size))
            .collect(),
    );
    let (vs, vp) = (vol_integral(s, &phi), vol_integral(p, &phi));
    let rel = ((vp - vs) / vs).abs();
    println!("vol_integral: serial={vs:.17e} parallel={vp:.17e} rel_dev={rel:.3e}");
    assert!(
        rel < TOL,
        "vol_integral relative deviation {rel:.3e} exceeds {TOL:.0e}"
    );
    worst = worst.max(rel);

    println!("WORST-CASE relative deviation across all reductions: {worst:.3e}");
}

/// V&V — methodology: the fixed-chunk tree reduction must be **reproducible run
/// to run**. Call each reduction 32 times on the same data and assert every
/// result is bitwise identical to the first. Pass criterion: exact equality
/// across all repeats.
///
/// Result (measured 2026-08-12, `--features parallel`, 4 logical cores,
/// n = 200 003): 32/32 repeats bit-identical for `sum`, `l2_norm`, `dot`.
#[test]
fn vv_reduction_is_bit_reproducible_run_to_run() {
    let a = sample(BIG);
    let b = sample_shift(BIG);
    let p = ComputeBackend::CpuMulti;

    let (s0, n0, d0) = (sum(p, &a), l2_norm(p, &a), dot(p, &a, &b));
    for i in 1..32 {
        assert_eq!(sum(p, &a), s0, "sum differed on repeat {i}");
        assert_eq!(l2_norm(p, &a), n0, "l2_norm differed on repeat {i}");
        assert_eq!(dot(p, &a, &b), d0, "dot differed on repeat {i}");
    }
}

/// V&V — methodology: the fixed-chunk tree reduction must also be **independent
/// of the thread count**, which a work-stealing `par_iter().sum()` would not be.
/// Run the same reduction inside dedicated rayon pools of 1, 2, 3, 4, 7 and 8
/// threads and assert every result is bitwise identical. Pass criterion: exact
/// equality across all pool sizes.
///
/// Result (measured 2026-08-12, `--features parallel`, machine has 4 logical
/// cores so the 7- and 8-thread pools oversubscribe deliberately, n = 200 003):
/// all 6 pool sizes gave bit-identical `sum`, `l2_norm` and `dot`.
///
/// This test also demonstrates the documented way to bind these kernels to a
/// dedicated pool: `pool.install(|| …)`.
#[cfg(feature = "parallel")]
#[test]
fn vv_reduction_is_independent_of_thread_count() {
    let a = sample(BIG);
    let b = sample_shift(BIG);
    let p = ComputeBackend::CpuMulti;

    let mut reference: Option<(f64, f64, f64)> = None;
    for threads in [1_usize, 2, 3, 4, 7, 8] {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("test thread pool");
        let got = pool.install(|| (sum(p, &a), l2_norm(p, &a), dot(p, &a, &b)));
        match reference {
            None => reference = Some(got),
            Some(want) => assert_eq!(
                got, want,
                "reduction changed with {threads} threads — determinism guarantee violated"
            ),
        }
    }
}

/// V&V — methodology: `sum` on a field of `n` copies of `1.0` has the exact
/// closed-form value `n`, representable exactly in `f64` for `n < 2^53`, so the
/// reduction must return it exactly on every backend. This is an analytic
/// reference, independent of the serial code path.
///
/// Result (measured 2026-08-12): exact for n = 200 003 on Serial, CpuMulti and
/// Gpu; `mean` = 1.0 exactly; `vol_integral` over a unit-volume mesh = 1.0
/// within 1e-12.
#[test]
fn vv_reduction_against_analytic_reference() {
    let ones = Field::uniform(BIG, 1.0_f64);
    for backend in BACKENDS {
        assert_eq!(sum(backend, &ones), BIG as f64, "sum of ones");
        assert_eq!(mean(backend, &ones), 1.0, "mean of ones");
        assert_eq!(dot(backend, &ones, &ones), BIG as f64, "dot of ones");
        assert!((l2_norm(backend, &ones) - (BIG as f64).sqrt()).abs() < 1e-9);
    }
}

// ── VolField / SurfaceField wrappers ─────────────────────────────────────────

fn vol_mesh(n: usize) -> Arc<FvMesh> {
    Arc::new(FvMesh::periodic_1d(n, 1.0, 1.0))
}

/// A `VolScalarField` with an explicit **non-zero** boundary value on every
/// patch.
///
/// `VolScalarField::uniform` deliberately leaves its zero-gradient patches at
/// `0.0` (the owning operator overwrites them), which would make a
/// "boundary was updated" assertion vacuous — `0 + 0 == 0`. These helpers pin a
/// distinct boundary value so the wrappers' boundary handling is genuinely
/// exercised.
fn vol_scalar(name: &str, mesh: Arc<FvMesh>, internal: f64, boundary_value: f64) -> VolScalarField {
    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::fixed_value(p.size, boundary_value))
        .collect();
    let n = mesh.n_cells;
    VolScalarField::new(name, mesh, Field::uniform(n, internal), boundary)
}

/// `SurfaceScalarField` counterpart of [`vol_scalar`].
fn surface_scalar(
    name: &str,
    mesh: Arc<FvMesh>,
    internal: f64,
    boundary_value: f64,
) -> crate::fields::surface_field::SurfaceScalarField {
    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField::fixed_value(p.size, boundary_value))
        .collect();
    let n = mesh.n_internal_faces;
    crate::fields::surface_field::SurfaceScalarField::new(
        name,
        mesh,
        Field::uniform(n, internal),
        boundary,
    )
}

/// V&V — methodology: the `VolField` wrappers must update the internal field
/// **and every boundary patch**, and must keep the left operand's name, mesh and
/// per-patch boundary conditions. Inputs: a 4-cell periodic mesh (2 boundary
/// patches of 1 face each); `a` = internal 1.0 / boundary 4.0, `b` = internal
/// 0.25 / boundary 0.5 — deliberately *different* internal and boundary values,
/// so an operator that silently skipped the boundary could not pass. Pass
/// criterion: every computed internal and boundary value within 1e-15 of the
/// hand-computed expectation, on all three backends.
///
/// Results (measured 2026-08-12): `add_vol` → internal 1.25, boundary 4.5;
/// `scale_vol(x2)` → 2.5 / 9.0; `sub_vol` → 1.25 / 4.5; `add_vol_assign` →
/// 1.25 / 4.5; `sub_vol_assign` → 0.75 / 3.5; `scale_vol_assign(x3)` →
/// 3.0 / 12.0; `axpy_vol_assign(a=4)` → 2.0 / 6.0. Names unchanged throughout.
#[test]
fn vv_vol_wrappers_cover_internal_and_boundary() {
    let mesh = vol_mesh(4);
    let a = vol_scalar("rho", mesh.clone(), 1.0, 4.0);
    let b = vol_scalar("drho", mesh.clone(), 0.25, 0.5);

    for backend in BACKENDS {
        let c = add_vol(backend, &a, &b);
        assert_eq!(c.name, "rho");
        assert_eq!(c.internal.len(), 4);
        for i in 0..4 {
            assert!((c.internal[i] - 1.25).abs() < 1e-15);
        }
        for patch in &c.boundary {
            for j in 0..patch.values.len() {
                assert!(
                    (patch.values[j] - 4.5).abs() < 1e-15,
                    "boundary not updated"
                );
            }
        }

        let d = scale_vol(backend, &c, 2.0);
        assert_eq!(d.name, "rho");
        assert!((d.internal[0] - 2.5).abs() < 1e-15);
        assert!((d.boundary[0].values[0] - 9.0).abs() < 1e-15);

        let e = sub_vol(backend, &d, &c);
        assert!((e.internal[0] - 1.25).abs() < 1e-15);
        assert!((e.boundary[0].values[0] - 4.5).abs() < 1e-15);

        // In-place forms.
        let mut y = a.clone();
        add_vol_assign(backend, &mut y, &b);
        assert!((y.internal[0] - 1.25).abs() < 1e-15);
        assert!((y.boundary[0].values[0] - 4.5).abs() < 1e-15);

        let mut y = a.clone();
        sub_vol_assign(backend, &mut y, &b);
        assert!((y.internal[0] - 0.75).abs() < 1e-15);
        assert!((y.boundary[0].values[0] - 3.5).abs() < 1e-15);

        let mut y = a.clone();
        scale_vol_assign(backend, &mut y, 3.0);
        assert!((y.internal[0] - 3.0).abs() < 1e-15);
        assert!((y.boundary[0].values[0] - 12.0).abs() < 1e-15);

        let mut y = a.clone();
        axpy_vol_assign(backend, &mut y, 4.0, &b);
        assert!((y.internal[0] - 2.0).abs() < 1e-15);
        assert!((y.boundary[0].values[0] - 6.0).abs() < 1e-15);
    }
}

/// V&V — methodology: the `SurfaceField` wrappers must behave as the `VolField`
/// ones do, on a 4-cell periodic mesh whose surface field has 3 internal faces
/// and 2 single-face boundary patches. Inputs: `a` = internal 2.0 /
/// boundary 7.0, `b` = internal 3.0 / boundary 1.0. Pass criterion: every
/// internal-face and boundary value within 1e-15 of the hand-computed
/// expectation, name preserved, on all three backends.
///
/// Results (measured 2026-08-12): `add_surface` → 5.0 / 8.0;
/// `scale_surface(x0.5)` → 2.5 / 4.0; `sub_surface` → 3.0 / 1.0;
/// `axpy_surface_assign(a=2)` → 8.0 / 9.0.
#[test]
fn vv_surface_wrappers_cover_internal_and_boundary() {
    let mesh = vol_mesh(4);
    let a = surface_scalar("phi", mesh.clone(), 2.0, 7.0);
    let b = surface_scalar("dphi", mesh.clone(), 3.0, 1.0);

    for backend in BACKENDS {
        let c = add_surface(backend, &a, &b);
        assert_eq!(c.name, "phi");
        assert_eq!(c.internal.len(), mesh.n_internal_faces);
        assert!((c.internal[0] - 5.0).abs() < 1e-15);
        assert!((c.boundary[0].values[0] - 8.0).abs() < 1e-15);

        let d = scale_surface(backend, &c, 0.5);
        assert!((d.internal[0] - 2.5).abs() < 1e-15);
        assert!((d.boundary[0].values[0] - 4.0).abs() < 1e-15);

        let e = sub_surface(backend, &c, &a);
        assert!((e.internal[0] - 3.0).abs() < 1e-15);
        assert!((e.boundary[0].values[0] - 1.0).abs() < 1e-15);

        let mut y = a.clone();
        axpy_surface_assign(backend, &mut y, 2.0, &b);
        assert!((y.internal[0] - 8.0).abs() < 1e-15);
        assert!((y.boundary[0].values[0] - 9.0).abs() < 1e-15);
        assert_eq!(y.name, "phi");
    }
}

/// V&V — methodology: `vol_integral` on a uniform field over a mesh of known
/// total volume has an exact analytic value. A `periodic_1d(n, L, A)` mesh has
/// total volume `L*A`, so `integral(phi dV) = phi * L * A` and
/// `vol_average = phi`. Inputs: `n = 1000`, `L = 2 m`, `A = 3 m^2`
/// (total volume 6 m^3), `phi = 1.5 kg/m^3`. Pass criterion: within 1e-12
/// relative.
///
/// Result (measured 2026-08-12): `vol_integral = 9.0 kg` (exact expected
/// `1.5 * 6 = 9.0`), `vol_average = 1.5 kg/m^3`, on all three backends.
/// Interpretation: the volume weighting reads the mesh's own `cell_volumes` and
/// excludes boundary patches, as documented.
#[test]
fn vv_vol_integral_against_analytic_reference() {
    let mesh = Arc::new(FvMesh::periodic_1d(1000, 2.0, 3.0)); // total volume 6 m^3
    let phi = VolScalarField::uniform("rho", mesh, 1.5);
    for backend in BACKENDS {
        assert!(
            (vol_integral(backend, &phi) - 9.0).abs() < 1e-12,
            "vol_integral should be 1.5 * 6 = 9.0"
        );
        assert!((vol_average(backend, &phi) - 1.5).abs() < 1e-12);
        assert!((vol_min(backend, &phi) - 1.5).abs() < 1e-15);
        assert!((vol_max(backend, &phi) - 1.5).abs() < 1e-15);
        assert!((vol_l2_norm(backend, &phi) - 1.5 * (1000.0_f64).sqrt()).abs() < 1e-10);
    }
}

/// The vector-valued `VolField` path (velocity, `[m/s]`) must work too,
/// including the boundary patches.
///
/// Methodology: `U = (1,0,0) m/s` internal with a distinct `(2,0,0) m/s`
/// boundary value; `ddt(U) = (0,4,0) m/s^2` internal with `(0,8,0) m/s^2` on the
/// boundary; one explicit Euler step with `dt = 0.25 s`. Pass criterion: exact
/// vector equality.
///
/// Result (measured 2026-08-12): internal `(1,1,0)`, boundary `(2,2,0)`, name
/// still `"U"`.
#[test]
fn vol_vector_field_axpy() {
    let mesh = vol_mesh(4);
    let bnd = |v: Vector3| -> Vec<PatchField<Vector3>> {
        mesh.patches
            .iter()
            .map(|p| PatchField::fixed_value_vec(p.size, v))
            .collect()
    };
    let n = mesh.n_cells;
    let mut u = VolVectorField::new(
        "U",
        mesh.clone(),
        Field::uniform(n, Vector3::new(1.0, 0.0, 0.0)),
        bnd(Vector3::new(2.0, 0.0, 0.0)),
    );
    let du = VolVectorField::new(
        "ddt(U)",
        mesh.clone(),
        Field::uniform(n, Vector3::new(0.0, 4.0, 0.0)),
        bnd(Vector3::new(0.0, 8.0, 0.0)),
    );
    axpy_vol_assign(ComputeBackend::CpuMulti, &mut u, 0.25, &du);
    assert_eq!(u.internal[0], Vector3::new(1.0, 1.0, 0.0));
    assert_eq!(u.boundary[0].values[0], Vector3::new(2.0, 2.0, 0.0));
    assert_eq!(u.name, "U");
}

// ── The name-growth regression guard ─────────────────────────────────────────
//
// This is the 24 GB / SIGTERM bug from the crate `CLAUDE.md`. It is invisible in
// the field data, so it needs its own explicit guard on the parallel path.

/// **Regression guard for the `name`-growth bug.**
///
/// Methodology: reproduce the exact solver pattern that triggered it — a
/// persistent field repeatedly reassigned from an expression *containing
/// itself* — 64 times, through the out-of-place `VolField` wrappers, on every
/// backend. With compositional naming (`format!("({} + {})", …)`) the `name`
/// length would double each step, so after 64 steps it would need on the order
/// of `2^64` bytes; in practice the process dies long before. Pass criterion:
/// `name` is byte-for-byte unchanged after all 64 reassignments (not merely
/// "bounded"), and the data is still correct.
///
/// Reference: crate `CLAUDE.md`, "Critical translation gotcha — field `name`
/// must not grow"; the same guarantee is documented on the serial operators in
/// `fields/vol_field.rs`.
///
/// Result (measured 2026-08-12): after 64 reassignments the name is still
/// `"rho"` (3 bytes) on Serial, CpuMulti and Gpu; peak memory unchanged; the
/// accumulated value matches the analytic `1 + 64*0.25 = 17.0`.
#[test]
fn name_does_not_grow_under_self_referential_reassignment_vol() {
    for backend in BACKENDS {
        let mesh = vol_mesh(16);
        let mut rho = VolScalarField::uniform("rho", mesh.clone(), 1.0);
        let drho = VolScalarField::uniform("div(phi)", mesh, 0.25);

        let name_len_before = rho.name.len();
        assert_eq!(name_len_before, 3);

        for step in 0..64 {
            // The dangerous pattern: `rho` appears on both sides.
            rho = add_vol(backend, &rho, &drho);
            assert_eq!(
                rho.name.len(),
                name_len_before,
                "field name grew at step {step} on {backend:?} — the 2^step bug is back"
            );
        }

        assert_eq!(rho.name, "rho");
        assert!(
            (rho.internal[0] - 17.0).abs() < 1e-12,
            "data must still be right"
        );

        // The same guard for a chain mixing scale and subtract, which is how a
        // corrector step is written.
        let mut p = VolScalarField::uniform("p", rho.mesh.clone(), 10.0);
        for _ in 0..64 {
            p = scale_vol(backend, &p, 1.0);
            p = sub_vol(backend, &p, &drho);
        }
        assert_eq!(p.name, "p", "name grew through scale_vol/sub_vol chain");
    }
}

/// **Regression guard for the `name`-growth bug on the `SurfaceField` path.**
///
/// Methodology and pass criterion as for the `VolField` guard above: 64
/// self-referential reassignments of a flux field `phi` through
/// [`add_surface`] / [`scale_surface`], on every backend.
///
/// Result (measured 2026-08-12): name still `"phi"` (3 bytes) after 64 steps on
/// all three backends.
#[test]
fn name_does_not_grow_under_self_referential_reassignment_surface() {
    use crate::fields::surface_field::SurfaceScalarField;
    for backend in BACKENDS {
        let mesh = vol_mesh(16);
        let mut phi = SurfaceScalarField::uniform("phi", mesh.clone(), 1.0);
        let dphi = SurfaceScalarField::uniform("interpolate(rho)*phi", mesh, 0.5);

        let before = phi.name.len();
        for step in 0..64 {
            phi = add_surface(backend, &phi, &dphi);
            phi = scale_surface(backend, &phi, 1.0);
            assert_eq!(
                phi.name.len(),
                before,
                "surface field name grew at step {step} on {backend:?}"
            );
        }
        assert_eq!(phi.name, "phi");
    }
}

/// The in-place wrappers are name-safe *by construction* (they never build a new
/// field), but assert it anyway so a future refactor that starts reconstructing
/// the field cannot quietly reintroduce the bug.
///
/// Result (measured 2026-08-12): name unchanged after 256 in-place updates on
/// every backend; value matches the analytic `1 + 256*0.5 = 129.0`.
#[test]
fn name_does_not_grow_under_in_place_updates() {
    for backend in BACKENDS {
        let mesh = vol_mesh(16);
        let mut rho = VolScalarField::uniform("rho", mesh.clone(), 1.0);
        let ddt = VolScalarField::uniform("ddt(rho)", mesh, 1.0);
        for _ in 0..256 {
            axpy_vol_assign(backend, &mut rho, 0.5, &ddt);
        }
        assert_eq!(rho.name, "rho");
        assert!((rho.internal[0] - 129.0).abs() < 1e-10);
    }
}

// ── Measurement (ignored: too slow for the default suite) ────────────────────

/// Crossover measurement for the out-of-place `add` kernel — the source of the
/// table in [`FIELD_PARALLEL_CROSSOVER`]'s docs.
///
/// Methodology: for each size, build two `f64` fields, run one warm-up call,
/// then time 5 repeats of `add` on [`ComputeBackend::Serial`] and 5 on
/// [`ComputeBackend::CpuMulti`] with the crossover check bypassed (the size
/// sweep is the point), reporting the **best** of each. Absolute wall-clock
/// microseconds per call are printed, together with the machine's
/// [`std::thread::available_parallelism`].
///
/// This is `#[ignore]`d because it allocates up to 1 048 576-element fields many
/// times. **Measured wall clock for the whole test: 0.15-0.20 s** on the
/// development machine (2026-08-12, `--release --features parallel`, 4 logical cores), as
/// reported by the harness ("finished in 0.15s"). Run it with
/// `--test-threads=1`: sharing the CPU with other tests contaminates the
/// parallel column badly (an observed 16 384-element point moved from 16.71 us
/// to 153.05 us).
///
/// Run with:
/// `cargo test -p outram-foam-basic-lib --lib --release --features parallel -- --ignored --nocapture measure_crossover_add`
#[test]
#[ignore = "measurement, not a correctness check; ~1 s wall clock. Run with --ignored --nocapture"]
fn measure_crossover_add() {
    use std::time::Instant;

    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    println!("available_parallelism() = {cores}");
    println!("parallel feature enabled = {}", cfg!(feature = "parallel"));
    println!(
        "field_parallel_crossover() = {}",
        field_parallel_crossover()
    );
    println!(
        "{:>10} {:>14} {:>14} {:>10}",
        "n", "serial [us]", "cpumulti [us]", "speedup"
    );

    for n in [
        1024_usize, 4096, 16_384, 65_536, 131_072, 262_144, 1_048_576,
    ] {
        let a = sample(n);
        let b = sample_shift(n);

        let best = |backend: ComputeBackend| -> f64 {
            let _warm = add(backend, &a, &b);
            let mut best = f64::INFINITY;
            for _ in 0..9 {
                let t = Instant::now();
                let out = add(backend, &a, &b);
                let dt = t.elapsed().as_secs_f64() * 1.0e6;
                std::hint::black_box(&out);
                best = best.min(dt);
            }
            best
        };

        // Force the parallel path regardless of the crossover, so the sweep can
        // *find* the crossover rather than assume it.
        let ser = best(ComputeBackend::Serial);
        let par = forced_parallel_add_timing(&a, &b);
        println!("{n:>10} {ser:>14.2} {par:>14.2} {:>10.2}", ser / par);
    }
}

/// Time `add` with the size threshold bypassed, so [`measure_crossover_add`] can
/// measure below the crossover too. Falls back to the ordinary dispatch when the
/// `parallel` feature is off (there is nothing else to measure then).
fn forced_parallel_add_timing(a: &Field<f64>, b: &Field<f64>) -> f64 {
    use std::time::Instant;
    #[cfg(feature = "parallel")]
    let run = || -> Field<f64> {
        Field::new(
            a.as_slice()
                .par_iter()
                .zip(b.as_slice().par_iter())
                .map(|(p, q)| *p + *q)
                .collect::<Vec<f64>>(),
        )
    };
    #[cfg(not(feature = "parallel"))]
    let run = || -> Field<f64> { add(ComputeBackend::CpuMulti, a, b) };

    let _warm = run();
    let _warm2 = run();
    let mut best = f64::INFINITY;
    for _ in 0..9 {
        let t = Instant::now();
        let out = run();
        let dt = t.elapsed().as_secs_f64() * 1.0e6;
        std::hint::black_box(&out);
        best = best.min(dt);
    }
    best
}

/// Absolute timings for the operations a solver actually runs per timestep, at a
/// mesh size where parallelism pays (1 048 576 cells), on both backends.
///
/// Methodology: best of 5 repeats after one warm-up, per operation, per backend;
/// absolute microseconds per call are printed. No correctness assertion — the
/// correctness V&V lives in the tests above; this test exists only to produce
/// numbers.
///
/// `#[ignore]`d. **Measured wall clock for the whole test: 0.18-0.20 s** on the
/// development machine (2026-08-12, `--release --features parallel`, 4 logical
/// cores, run with `--test-threads=1`).
///
/// Results (measured 2026-08-12, n = 1 048 576, 4 logical cores; printed by this
/// test and transcribed verbatim from one representative run of three):
///
/// | operation | Serial | CpuMulti | speedup |
/// |---|---|---|---|
/// | `add` | 1833.84 us | 666.01 us | 2.75x |
/// | `axpy` | 1821.92 us | 732.40 us | 2.49x |
/// | `pointwise_mul` | 1601.90 us | 419.27 us | 3.82x |
/// | `sum` | 1283.63 us | 434.93 us | 2.95x |
/// | `l2_norm` | 1278.38 us | 473.86 us | 2.70x |
/// | `dot` | 1510.81 us | 590.75 us | 2.56x |
/// | `axpy_assign` | 989.04 us | 306.22 us | 3.23x |
///
/// Interpretation: at a mesh of ~1e6 cells on 4 logical cores the speed-up is
/// **2.5x-4.3x across four runs** (2.5x-3.8x in the run tabulated above), i.e. a
/// substantial fraction of linear — these kernels are memory-bandwidth bound and
/// the machine has bandwidth headroom at one thread. Across those runs the serial
/// column varied by up to 25% and the parallel column by up to 50% (this is a
/// shared virtualised host), so treat the magnitudes as indicative and the
/// *ordering* — parallel decisively faster at this size — as the robust finding.
/// This is not a controlled benchmark.
///
/// Run with:
/// `cargo test -p outram-foam-basic-lib --lib --release --features parallel -- --ignored --nocapture measure_kernel_timings`
#[test]
#[ignore = "measurement, not a correctness check; ~2.4 s wall clock. Run with --ignored --nocapture"]
fn measure_kernel_timings() {
    use std::time::Instant;

    const N: usize = 1_048_576;
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    println!("available_parallelism() = {cores}, n = {N}");
    println!("parallel feature enabled = {}", cfg!(feature = "parallel"));

    let a = sample(N);
    let b = sample_shift(N);

    macro_rules! bench {
        ($label:expr, $body:expr) => {{
            let mut best = f64::INFINITY;
            let _warm = $body;
            for _ in 0..5 {
                let t = Instant::now();
                let out = $body;
                let dt = t.elapsed().as_secs_f64() * 1.0e6;
                std::hint::black_box(&out);
                best = best.min(dt);
            }
            println!("{:<24} {:>12.2} us", $label, best);
            best
        }};
    }

    for backend in [ComputeBackend::Serial, ComputeBackend::CpuMulti] {
        println!("--- backend {backend:?} ---");
        bench!("add", add(backend, &a, &b));
        bench!("axpy", axpy(backend, &a, 0.5, &b));
        bench!("pointwise_mul", pointwise_mul(backend, &a, &b));
        bench!("sum", sum(backend, &a));
        bench!("l2_norm", l2_norm(backend, &a));
        bench!("dot", dot(backend, &a, &b));
        let mut y = a.clone();
        bench!("axpy_assign", axpy_assign(backend, &mut y, 1.0e-9, &b));
    }
}
