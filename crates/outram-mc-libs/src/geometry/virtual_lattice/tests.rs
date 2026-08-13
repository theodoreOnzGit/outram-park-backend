//! Unit tests for the virtual lattice.
//!
//! The load-bearing test is [`traversal_agrees_with_brute_force_over_a_packing`]:
//! the accelerator is only useful if it returns *exactly* what the
//! unaccelerated scan returns. Everything else checks the bucket build and the
//! documented edge cases.
//!
//! These are **verification** tests (is it implemented correctly?), not
//! validation. No k-eigenvalue comparison against the upstream
//! `triso_virtual_lattice` regression case has been run.

use super::*;
use crate::geometry::position::Direction;
use crate::geometry::surface::{BoundaryType, Sphere};

/// Sphere at `(x, y, z)` with radius `r`, as a [`SurfaceKind`].
fn sphere(x: f64, y: f64, z: f64, r: f64) -> SurfaceKind {
    SurfaceKind::Sphere(Sphere {
        x0: x,
        y0: y,
        z0: z,
        r,
        bc: BoundaryType::Transmissive,
    })
}

/// A deterministic pseudo-random packing of `n` non-overlapping-ish spheres in
/// the unit-ish box `[0, 4)^3`. A plain LCG keeps the test reproducible without
/// pulling in the crate's RNG.
fn packing(n: usize, radius: f64) -> Vec<SurfaceKind> {
    let mut state: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((state >> 33) as f64) / ((1u64 << 31) as f64)
    };
    (0..n)
        .map(|_| {
            let x = next() * 4.0;
            let y = next() * 4.0;
            let z = next() * 4.0;
            sphere(x, y, z, radius)
        })
        .collect()
}

// ── bucket build ───────────────────────────────────────────────────────────

#[test]
fn a_small_sphere_registers_only_in_its_own_voxel() {
    // Grid 4x4x4 of unit voxels over [0,4)^3; sphere well inside voxel (1,2,3).
    let surfaces = vec![sphere(1.5, 2.5, 3.5, 0.2)];
    let (vl, report) = VirtualLattice::build([0.0; 3], [1.0; 3], [4, 4, 4], &[0], &surfaces);

    assert_eq!(report.registrations, 1, "one voxel only");
    assert!(report.unregistered.is_empty());
    assert_eq!(vl.surfaces_in_voxel([1, 2, 3]), &[0]);
    assert!(vl.surfaces_in_voxel([1, 2, 2]).is_empty());
}

#[test]
fn a_sphere_straddling_a_face_registers_in_both_voxels() {
    // Centre at x = 0.95 with radius 0.2 reaches into voxel i = 1 (x >= 1.0).
    let surfaces = vec![sphere(0.95, 0.5, 0.5, 0.2)];
    let (vl, report) = VirtualLattice::build([0.0; 3], [1.0; 3], [4, 4, 4], &[0], &surfaces);

    assert_eq!(report.registrations, 2);
    assert_eq!(vl.surfaces_in_voxel([0, 0, 0]), &[0]);
    assert_eq!(vl.surfaces_in_voxel([1, 0, 0]), &[0]);
}

#[test]
fn tangency_is_exclusive_matching_upstreams_strict_comparison() {
    // Centre exactly 0.25 from the x = 1.0 face, radius exactly 0.25: the
    // sphere touches but does not cross. Upstream's `sqrt(d2) < radius_` is
    // strict, so it is NOT registered in the neighbouring voxel.
    //
    // The coordinates here are deliberately dyadic (0.75, 0.25 — exact in
    // binary floating point). Exact tangency is only observable with exactly
    // representable values: with, say, centre 0.8 and radius 0.2, the gap
    // evaluates to 0.19999999999999996 < 0.2 and the sphere *does* register.
    // That is not a porting artefact — upstream's `sqrt` form takes the same
    // branch on the same inputs. Tangency is a measure-zero case in a real
    // packing; this test pins the comparison's strictness, not a physical
    // requirement.
    let surfaces = vec![sphere(0.75, 0.5, 0.5, 0.25)];
    let (vl, _) = VirtualLattice::build([0.0; 3], [1.0; 3], [4, 4, 4], &[0], &surfaces);

    assert_eq!(vl.surfaces_in_voxel([0, 0, 0]), &[0]);
    assert!(
        vl.surfaces_in_voxel([1, 0, 0]).is_empty(),
        "tangent sphere must not register in the neighbour"
    );
}

#[test]
fn a_non_sphere_surface_is_reported_as_unregistered() {
    // Documented limitation: upstream's triso_in_mesh stubs every non-sphere
    // type to false, so a plane can never join a bucket.
    use crate::geometry::surface::XPlane;
    let surfaces = vec![SurfaceKind::XPlane(XPlane {
        x0: 2.0,
        bc: BoundaryType::Transmissive,
    })];
    let (vl, report) = VirtualLattice::build([0.0; 3], [1.0; 3], [4, 4, 4], &[0], &surfaces);

    assert_eq!(report.unregistered, vec![0]);
    assert_eq!(report.registrations, 0);
    assert!((0..vl.n_voxels()).all(|f| vl.buckets[f].is_empty()));
}

#[test]
fn a_sphere_outside_the_grid_is_reported_as_unregistered() {
    let surfaces = vec![sphere(99.0, 99.0, 99.0, 0.2)];
    let (_, report) = VirtualLattice::build([0.0; 3], [1.0; 3], [4, 4, 4], &[0], &surfaces);

    assert_eq!(report.unregistered, vec![0]);
    assert_eq!(report.registrations, 0);
}

#[test]
fn an_oversized_sphere_is_flagged_because_the_neighbourhood_scan_may_miss_voxels() {
    // Radius 0.9 > pitch/2 = 0.5: the 3x3x3 scan around the centre voxel is no
    // longer guaranteed to find every overlapped voxel. Upstream does not check
    // this; we report it.
    let surfaces = vec![sphere(2.0, 2.0, 2.0, 0.9)];
    let (_, report) = VirtualLattice::build([0.0; 3], [1.0; 3], [4, 4, 4], &[0], &surfaces);

    assert_eq!(report.radius_exceeds_pitch, vec![0]);
}

#[test]
#[should_panic(expected = "pitch must be strictly positive")]
fn a_zero_pitch_is_a_programming_error() {
    VirtualLattice::build([0.0; 3], [1.0, 0.0, 1.0], [4, 4, 4], &[], &[]);
}

// ── indexing ───────────────────────────────────────────────────────────────

#[test]
fn flat_index_is_x_fastest_matching_upstream() {
    let (vl, _) = VirtualLattice::build([0.0; 3], [1.0; 3], [4, 5, 6], &[], &[]);
    assert_eq!(vl.flat_index([1, 0, 0]), 1);
    assert_eq!(vl.flat_index([0, 1, 0]), 4);
    assert_eq!(vl.flat_index([0, 0, 1]), 20);
    assert_eq!(vl.n_voxels(), 120);
}

#[test]
fn indices_outside_the_grid_are_reported_then_clamped() {
    let (vl, _) = VirtualLattice::build([0.0; 3], [1.0; 3], [4, 4, 4], &[], &[]);
    let outside = Position::new(-3.5, 9.5, 2.5);

    assert_eq!(vl.indices_at(outside), [-4, 9, 2]);
    assert!(!vl.contains_indices(vl.indices_at(outside)));
    assert_eq!(vl.clamped_indices_at(outside), [0, 3, 2]);
}

// ── point location ─────────────────────────────────────────────────────────

#[test]
fn find_containing_locates_the_sphere_around_a_point() {
    let surfaces = vec![sphere(1.5, 1.5, 1.5, 0.3), sphere(2.5, 2.5, 2.5, 0.3)];
    let (vl, _) = VirtualLattice::build([0.0; 3], [1.0; 3], [4, 4, 4], &[0, 1], &surfaces);

    assert_eq!(
        vl.find_containing(Position::new(1.5, 1.5, 1.5), &surfaces),
        Some(0)
    );
    assert_eq!(
        vl.find_containing(Position::new(2.6, 2.5, 2.5), &surfaces),
        Some(1)
    );
}

#[test]
fn find_containing_returns_none_in_the_matrix_between_particles() {
    let surfaces = vec![sphere(1.5, 1.5, 1.5, 0.3)];
    let (vl, _) = VirtualLattice::build([0.0; 3], [1.0; 3], [4, 4, 4], &[0], &surfaces);

    // Inside the same voxel but outside the sphere.
    assert_eq!(
        vl.find_containing(Position::new(1.05, 1.05, 1.05), &surfaces),
        None
    );
}

#[test]
fn find_containing_is_strict_on_the_surface() {
    // Exactly on the sphere: d2 == r2, and the test is `<`, so not contained.
    let surfaces = vec![sphere(1.5, 1.5, 1.5, 0.3)];
    let (vl, _) = VirtualLattice::build([0.0; 3], [1.0; 3], [4, 4, 4], &[0], &surfaces);

    assert_eq!(
        vl.find_containing(Position::new(1.8, 1.5, 1.5), &surfaces),
        None
    );
}

#[test]
fn find_containing_agrees_with_a_brute_force_scan_over_a_packing() {
    let surfaces = packing(400, 0.12);
    let idx: Vec<usize> = (0..surfaces.len()).collect();
    let (vl, report) = VirtualLattice::build([0.0; 3], [0.5; 3], [8, 8, 8], &idx, &surfaces);
    assert!(report.radius_exceeds_pitch.is_empty());

    let mut probes = 0;
    let mut hits = 0;
    for a in 0..12 {
        for b in 0..12 {
            for c in 0..12 {
                let p = Position::new(
                    a as f64 * 4.0 / 12.0,
                    b as f64 * 4.0 / 12.0,
                    c as f64 * 4.0 / 12.0,
                );
                let fast = vl.find_containing(p, &surfaces);
                let slow = idx.iter().copied().find(|&si| {
                    let (ctr, r) = surfaces[si].sphere_centre_radius().unwrap();
                    let (dx, dy, dz) = (p.x - ctr[0], p.y - ctr[1], p.z - ctr[2]);
                    dx * dx + dy * dy + dz * dz < r * r
                });
                assert_eq!(fast, slow, "point location disagreed at {p:?}");
                probes += 1;
                if fast.is_some() {
                    hits += 1;
                }
            }
        }
    }
    assert_eq!(probes, 12 * 12 * 12);
    assert!(
        hits > 0,
        "test is vacuous if no probe ever lands inside a sphere"
    );
}

// ── ray traversal ──────────────────────────────────────────────────────────

#[test]
fn traversal_finds_a_sphere_straight_ahead() {
    // Sphere centred at x = 2.5, radius 0.25; ray from origin along +x enters
    // its near face at x = 2.25, so distance = 2.25.
    let surfaces = vec![sphere(2.5, 0.5, 0.5, 0.25)];
    let (vl, _) = VirtualLattice::build([0.0; 3], [1.0; 3], [4, 4, 4], &[0], &surfaces);

    let (d, si) = vl.distance(
        Position::new(0.0, 0.5, 0.5),
        Direction::new(1.0, 0.0, 0.0),
        &surfaces,
        usize::MAX,
        f64::INFINITY,
    );
    assert_eq!(si, 0);
    assert!((d - 2.25).abs() < 1e-12, "distance {d}");
}

#[test]
fn traversal_returns_infinity_when_the_ray_misses_everything() {
    let surfaces = vec![sphere(2.5, 3.5, 0.5, 0.25)];
    let (vl, _) = VirtualLattice::build([0.0; 3], [1.0; 3], [4, 4, 4], &[0], &surfaces);

    let (d, si) = vl.distance(
        Position::new(0.0, 0.5, 0.5),
        Direction::new(1.0, 0.0, 0.0),
        &surfaces,
        usize::MAX,
        f64::INFINITY,
    );
    assert_eq!(si, usize::MAX);
    assert_eq!(d, f64::INFINITY);
}

#[test]
fn traversal_on_an_empty_grid_returns_infinity() {
    let (vl, _) = VirtualLattice::build([0.0; 3], [1.0; 3], [4, 4, 4], &[], &[]);
    let (d, si) = vl.distance(
        Position::ZERO,
        Direction::new(1.0, 0.0, 0.0),
        &[],
        usize::MAX,
        f64::INFINITY,
    );
    assert_eq!((d, si), (f64::INFINITY, usize::MAX));
}

#[test]
fn traversal_normalises_a_non_unit_direction() {
    let surfaces = vec![sphere(2.5, 0.5, 0.5, 0.25)];
    let (vl, _) = VirtualLattice::build([0.0; 3], [1.0; 3], [4, 4, 4], &[0], &surfaces);
    let start = Position::new(0.0, 0.5, 0.5);

    let unit = vl.distance(
        start,
        Direction::new(1.0, 0.0, 0.0),
        &surfaces,
        usize::MAX,
        f64::INFINITY,
    );
    let scaled = vl.distance(
        start,
        Direction::new(7.0, 0.0, 0.0),
        &surfaces,
        usize::MAX,
        f64::INFINITY,
    );
    assert_eq!(unit.1, scaled.1);
}

#[test]
fn traversal_agrees_with_brute_force_over_a_packing() {
    // THE correctness property: the accelerator must return exactly what a
    // scan over every surface returns, for every ray.
    let surfaces = packing(300, 0.1);
    let idx: Vec<usize> = (0..surfaces.len()).collect();
    let (vl, report) = VirtualLattice::build([0.0; 3], [0.5; 3], [8, 8, 8], &idx, &surfaces);
    assert!(
        report.unregistered.is_empty(),
        "every sphere of the packing should be placed: {:?}",
        report.unregistered
    );

    // Rays from a spread of origins in a spread of directions, including
    // axis-aligned ones (where a direction component is exactly zero, the
    // degenerate case in the DDA setup) and diagonals.
    let dirs = [
        Direction::from_unnormalised(1.0, 0.0, 0.0),
        Direction::from_unnormalised(0.0, 1.0, 0.0),
        Direction::from_unnormalised(0.0, 0.0, 1.0),
        Direction::from_unnormalised(-1.0, 0.0, 0.0),
        Direction::from_unnormalised(0.0, -1.0, 0.0),
        Direction::from_unnormalised(1.0, 1.0, 0.0),
        Direction::from_unnormalised(1.0, 1.0, 1.0),
        Direction::from_unnormalised(-1.0, 2.0, -0.5),
        Direction::from_unnormalised(0.3, -0.7, 1.0),
    ];

    let mut compared = 0;
    let mut found = 0;
    for a in 0..5 {
        for b in 0..5 {
            for c in 0..5 {
                let origin = Position::new(a as f64 * 0.83, b as f64 * 0.83, c as f64 * 0.83);
                for &u in &dirs {
                    let fast = vl.distance(origin, u, &surfaces, usize::MAX, f64::INFINITY);
                    let slow = VirtualLattice::distance_brute_force(
                        origin,
                        u,
                        &surfaces,
                        &idx,
                        usize::MAX,
                    );
                    assert_eq!(
                        fast.1, slow.1,
                        "surface disagreed from {origin:?} along {u:?}: \
                         accelerated {fast:?} vs brute force {slow:?}"
                    );
                    if slow.1 != usize::MAX {
                        assert!(
                            (fast.0 - slow.0).abs() <= 1e-12 * slow.0.max(1.0),
                            "distance disagreed from {origin:?} along {u:?}: \
                             {} vs {}",
                            fast.0,
                            slow.0
                        );
                        found += 1;
                    }
                    compared += 1;
                }
            }
        }
    }
    assert_eq!(compared, 5 * 5 * 5 * dirs.len());
    assert!(found > 0, "test is vacuous if no ray ever hits a sphere");
}

#[test]
fn traversal_respects_the_coincident_flag_for_the_surface_it_sits_on() {
    // A particle sitting on a sphere must not re-report a zero-distance
    // crossing of that same sphere.
    let surfaces = vec![sphere(2.0, 0.5, 0.5, 0.5)];
    let (vl, _) = VirtualLattice::build([0.0; 3], [1.0; 3], [4, 4, 4], &[0], &surfaces);

    // Exactly on the near face (x = 1.5), heading inward.
    let on_face = Position::new(1.5, 0.5, 0.5);
    let (d, _) = vl.distance(
        on_face,
        Direction::new(1.0, 0.0, 0.0),
        &surfaces,
        0,
        f64::INFINITY,
    );
    assert!(
        d > 1e-9,
        "coincident surface re-reported a zero crossing: {d}"
    );
}
