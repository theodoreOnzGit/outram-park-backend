// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full notice.

//! Unit tests for the albedo (Robin) boundary condition.

use super::*;
use outram_foam_basic_lib::fv_operators::fvm::DeltaCoeff;
use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;
use std::sync::Arc;

/// A uniform 1-D bar of `n` cells over `[0, L]`, unit cross-section, with the
/// `right` (x = L) patch first and `left` (x = 0) second — the patch ordering
/// the basic-lib operator tests use.
fn bar(n: usize, length: f64) -> Arc<FvMesh> {
    let dx = length / n as f64;
    let n_int = n - 1;
    let mut owner = (0..n_int).collect::<Vec<_>>();
    let neighbour = (1..n).collect::<Vec<_>>();
    owner.push(n - 1);
    owner.push(0);
    Arc::new(
        FvMeshBuilder::new()
            .n_cells(n)
            .n_internal_faces(n_int)
            .owner(owner)
            .neighbour(neighbour)
            .patches(vec![
                BoundaryPatch::new("right", n_int, 1, PatchKind::Patch),
                BoundaryPatch::new("left", n_int + 1, 1, PatchKind::Patch),
            ])
            .cell_volumes(vec![dx; n])
            .cell_centres(
                (0..n)
                    .map(|i| Vector3::new((i as f64 + 0.5) * dx, 0.0, 0.0))
                    .collect(),
            )
            .face_area_vectors(
                (0..n_int)
                    .map(|_| Vector3::new(1.0, 0.0, 0.0))
                    .chain([Vector3::new(1.0, 0.0, 0.0), Vector3::new(-1.0, 0.0, 0.0)])
                    .collect(),
            )
            .face_centres(
                (0..n_int)
                    .map(|f| Vector3::new((f as f64 + 1.0) * dx, 0.0, 0.0))
                    .chain([Vector3::new(length, 0.0, 0.0), Vector3::new(0.0, 0.0, 0.0)])
                    .collect(),
            )
            .build()
            .unwrap(),
    )
}

fn weight_of(pf: &PatchField<f64>) -> Vec<f64> {
    match &pf.bc {
        BoundaryCondition::MixedField { value_fraction, .. } => value_fraction.as_slice().to_vec(),
        other => panic!("expected MixedField, got {other:?}"),
    }
}

/// `gamma = (1 - alpha)/(1 + alpha)/2`, with the two limits that fix the
/// convention: a perfect reflector gives 0, a vacuum gives 1/2.
///
/// The vacuum value is also upstream's default for the `gamma` keyword, which is
/// the cross-check that this is the same convention and not a factor of two off.
#[test]
fn the_albedo_coefficient_reproduces_its_two_limits() {
    assert!((gamma_from_albedo(0.0) - 0.5).abs() < 1e-15, "vacuum");
    assert!(gamma_from_albedo(1.0).abs() < 1e-15, "perfect reflector");
    // alpha = 1/3 -> gamma = (2/3)/(4/3)/2 = 1/4.
    assert!((gamma_from_albedo(1.0 / 3.0) - 0.25).abs() < 1e-15);
    // Monotone: more reflective means a smaller gamma.
    let mut previous = f64::INFINITY;
    for i in 0..=10 {
        let g = gamma_from_albedo(i as f64 / 10.0);
        assert!(g < previous, "gamma must decrease with albedo");
        previous = g;
    }
}

/// A perfect reflector is zero-gradient and a vacuum-limit albedo is Dirichlet
/// zero — checked on the Robin *weight*, which is what the matrix sees.
#[test]
fn the_limits_reduce_to_zero_gradient_and_to_fixed_value() {
    let mesh = bar(10, 1.0);
    let d = VolScalarField::uniform("D", mesh.clone(), 1.5e-2);

    let reflector = albedo_patch_field(
        &mesh,
        0,
        0.0,
        &d,
        AlbedoLinearisation::FaceValue,
        DeltaCoeff::Orthogonal,
    );
    assert_eq!(
        weight_of(&reflector),
        vec![0.0],
        "gamma = 0 -> zero gradient"
    );

    // gamma -> infinity is the absorbing limit; a large finite value must
    // approach w = 1 from below and never exceed it.
    let absorbing = albedo_patch_field(
        &mesh,
        0,
        1.0e12,
        &d,
        AlbedoLinearisation::FaceValue,
        DeltaCoeff::Orthogonal,
    );
    let w = weight_of(&absorbing)[0];
    assert!(
        w < 1.0 && w > 1.0 - 1e-9,
        "gamma -> inf -> fixed value, got {w}"
    );
}

/// The weight is built from the LOCAL diffusion coefficient, so two groups whose
/// `D` differ get different weights — the reason a scalar per-patch `Mixed`
/// cannot express this condition.
#[test]
fn the_weight_tracks_the_groups_own_diffusion_coefficient() {
    let mesh = bar(10, 1.0);
    let gamma = 0.1;
    // The MSFR tutorial's own extremes, 1.10e-2 and 2.37e-2 m.
    let fast = VolScalarField::uniform("D", mesh.clone(), 2.372010e-2);
    let slow = VolScalarField::uniform("D", mesh.clone(), 1.103830e-2);

    let w_fast = weight_of(&albedo_patch_field(
        &mesh,
        0,
        gamma,
        &fast,
        AlbedoLinearisation::FaceValue,
        DeltaCoeff::Orthogonal,
    ))[0];
    let w_slow = weight_of(&albedo_patch_field(
        &mesh,
        0,
        gamma,
        &slow,
        AlbedoLinearisation::FaceValue,
        DeltaCoeff::Orthogonal,
    ))[0];

    // delta = dx/2 = 0.05 m for a 10-cell unit bar.
    let expect = |d: f64| {
        let r = gamma * 0.05 / d;
        r / (1.0 + r)
    };
    assert!((w_fast - expect(2.372010e-2)).abs() < 1e-14);
    assert!((w_slow - expect(1.103830e-2)).abs() < 1e-14);
    assert!(
        w_slow > w_fast,
        "a smaller D means a more absorbing boundary: {w_slow} vs {w_fast}"
    );
    // The spread across the MSFR's own groups is more than a factor of two in
    // D, and the weights differ by well over a percent — not a rounding detail.
    assert!(
        (w_slow - w_fast).abs() / w_fast > 0.01,
        "weights differ by {:.3} %",
        100.0 * (w_slow - w_fast).abs() / w_fast
    );
}

/// **V&V — the discretised albedo reproduces the analytic Robin face value.**
///
/// ## Methodology
///
/// For a single boundary face with owner-cell value `phi_c`, the condition
/// `-D dphi/dn = gamma phi` discretised one-sidedly has the exact face value
/// `phi_f = phi_c/(1 + gamma delta/D)`. The `mixed` form this module emits
/// evaluates `phi_f = (1 - w) phi_c`. The test asserts the two agree to 1e-15
/// relative over a sweep of `gamma` spanning eight orders of magnitude, and over
/// two mesh resolutions so `delta` changes.
///
/// ## Results (measured 2026-09-15)
///
/// Worst relative difference **3.24e-13**, across `gamma` in
/// `{1e-4, 1e-2, 0.1, 0.5, 1, 10, 1e4}` and `n` in `{10, 40}`.
///
/// **Where that 3.24e-13 comes from, since it is not zero.** It is cancellation
/// in the test's own reconstruction, not error in the condition. At
/// `gamma = 1e4` on the 40-cell bar, `r = gamma delta/D = 8333`, so
/// `w = r/(1+r)` is within `1.2e-4` of one and forming `1 - w` in double
/// precision discards about four significant digits: `1e-16 / 1.2e-4 ~ 1e-12`,
/// which is what is measured. The analytic form `1/(1+r)` has no such
/// subtraction.
///
/// This does **not** reach the linear system. `fvm::laplacian` uses `w` directly
/// on the diagonal (`diag += w coeff`) and `(1 - w)` only as a multiplier on
/// `refGrad`, which this condition sets to zero. The lossy quantity is therefore
/// never assembled; it appears only when the face value is reconstructed for
/// interpolation, and only for albedo coefficients far beyond the physical range
/// (`gamma <= 1/2` for any real surface — `1e4` is in the sweep to probe the
/// absorbing limit, not because a reactor has one).
///
/// The tolerance is set at `1e-12` for that reason rather than at machine
/// epsilon. Within the physical range `gamma <= 1/2` the agreement is at
/// round-off: **1.02e-15** worst case, which is a couple of ulp on a value of
/// `phi_c = 3.7` and is asserted separately at `5e-16 phi_c`.
#[test]
fn the_discretised_weight_matches_the_analytic_robin_face_value() {
    let phi_c = 3.7;
    let d_value = 1.5e-2;
    let mut worst = 0.0_f64;
    for n in [10usize, 40] {
        let mesh = bar(n, 1.0);
        let delta = 0.5 / n as f64; // half a cell
        let d = VolScalarField::uniform("D", mesh.clone(), d_value);
        for gamma in [1e-4, 1e-2, 0.1, 0.5, 1.0, 10.0, 1e4] {
            let w = weight_of(&albedo_patch_field(
                &mesh,
                0,
                gamma,
                &d,
                AlbedoLinearisation::FaceValue,
                DeltaCoeff::Orthogonal,
            ))[0];
            let discretised = (1.0 - w) * phi_c;
            let analytic = phi_c / (1.0 + gamma * delta / d_value);
            let rel = (discretised - analytic).abs() / analytic;
            worst = worst.max(rel);
        }
    }
    // Separately: within the physical range of gamma the reconstruction is not
    // lossy, so hold that part to round-off.
    let mut worst_physical = 0.0_f64;
    for n in [10usize, 40] {
        let mesh = bar(n, 1.0);
        let delta = 0.5 / n as f64;
        let d = VolScalarField::uniform("D", mesh.clone(), d_value);
        for gamma in [0.0, 1e-4, 1e-2, 0.1, 0.25, 0.5] {
            let w = weight_of(&albedo_patch_field(
                &mesh,
                0,
                gamma,
                &d,
                AlbedoLinearisation::FaceValue,
                DeltaCoeff::Orthogonal,
            ))[0];
            let analytic = phi_c / (1.0 + gamma * delta / d_value);
            worst_physical = worst_physical.max(((1.0 - w) * phi_c - analytic).abs() / analytic);
        }
    }
    println!(
        "albedo face value: worst relative difference {worst:.3e} over the full \
         sweep, {worst_physical:.3e} over the physical range gamma <= 1/2"
    );
    assert!(
        worst_physical < 5e-16 * phi_c,
        "within the physical albedo range the face value differs from the \
         analytic Robin result by {worst_physical:.3e}"
    );
    assert!(
        worst < 1e-12,
        "the discretised albedo face value differs from the analytic Robin \
         result by {worst:.3e}"
    );
}

/// A non-positive diffusion coefficient is treated as vacuum rather than
/// producing an infinity — the conservative direction, since it cannot inflate
/// `k_eff`.
#[test]
fn a_non_physical_diffusion_coefficient_degrades_to_vacuum() {
    let mesh = bar(10, 1.0);
    let zero = VolScalarField::uniform("D", mesh.clone(), 0.0);
    assert_eq!(
        weight_of(&albedo_patch_field(
            &mesh,
            0,
            0.1,
            &zero,
            AlbedoLinearisation::FaceValue,
            DeltaCoeff::Orthogonal,
        )),
        vec![1.0]
    );
    let negative = VolScalarField::uniform("D", mesh.clone(), -1.0);
    assert_eq!(
        weight_of(&albedo_patch_field(
            &mesh,
            0,
            0.1,
            &negative,
            AlbedoLinearisation::FaceValue,
            DeltaCoeff::Orthogonal,
        )),
        vec![1.0]
    );
}
