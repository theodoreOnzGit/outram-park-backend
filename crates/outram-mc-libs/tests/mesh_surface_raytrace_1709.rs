// SPDX-License-Identifier: AGPL-3.0-only

//! **gh:#1709: the port of OpenMC's `raytrace_mesh` against the re-derivation
//! it replaced in the mesh-surface filter.**
//!
//! # Methodology
//!
//! `RegularMeshExt::surface_bins_crossed_openmc` ports
//! `StructuredMesh::raytrace_mesh` with its `SurfaceAggregator`
//! (`src/mesh.cpp:1312-1488`, OpenMC `8f79d9d0`). `surface_bins_crossed`
//! enumerates the plane crossings on the open interval instead. The two
//! differ only at ties: a track that ends (or starts) on a mesh plane.
//!
//! - **Off ties:** 20 000 straight tracks from a fixed LCG, end points drawn
//!   uniformly in a box larger than a 5 x 3 x 4 mesh (so tracks start and end
//!   inside, outside and across it), none within 1e-6 cm of a plane. Pass:
//!   the two give the same bins in the same order on every track.
//! - **At a tie:** a track ending exactly on an interior plane. Upstream stops
//!   when the distance travelled reaches the track length, so the crossing at
//!   the end is not counted; the re-derivation does not count it either (open
//!   interval). A track continuing past the plane counts it in both.
//!
//! **Prediction, before the run:** identical on every off-tie track.
//!
//! # Results (2026-11-22)
//!
//! Identical on all 20 000 off-tie tracks; both tie cases as stated. The
//! Experiment A mesh-surface comparison against OpenMC
//! (`tests/tally_filters_vs_openmc.rs`) went from 16 / 1 tie misses to 0 / 0
//! with this port and upstream's `r_last_current` span (see the V&V README).

// The deprecated port of OpenMC's mesh-surface raytrace (gh:#1715) is used here
// on purpose: it is the code-to-code oracle these tests pin or compare against.
#![allow(deprecated)]

use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::tally::mesh::{RegularMesh, RegularMeshExt};

fn mesh() -> RegularMesh {
    RegularMesh { lower_left: [-5.0, -3.0, -2.0], upper_right: [5.0, 3.0, 6.0], dimension: [5, 3, 4] }
}

fn dir(a: Position, b: Position) -> Direction {
    let d = [b.x - a.x, b.y - a.y, b.z - a.z];
    let n = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    Direction { u: d[0] / n, v: d[1] / n, w: d[2] / n }
}

/// Distance of `x` along axis `k` from the nearest plane of `m`.
fn off_plane(m: &RegularMesh, p: Position) -> f64 {
    let r = [p.x, p.y, p.z];
    let w = m.width();
    let mut best = f64::INFINITY;
    for k in 0..3 {
        for i in 0..=m.dimension[k] {
            best = best.min((r[k] - (m.lower_left[k] + i as f64 * w[k])).abs());
        }
    }
    best
}

#[test]
fn the_raytrace_port_matches_the_rederivation_off_ties() {
    let m = mesh();
    let mut s: u64 = 0x1709_2026_1122;
    let mut next = || {
        s = s.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
        (s >> 11) as f64 / (1u64 << 53) as f64
    };
    let point = |next: &mut dyn FnMut() -> f64| {
        Position::new(-8.0 + 16.0 * next(), -6.0 + 12.0 * next(), -5.0 + 14.0 * next())
    };
    let mut n = 0;
    let mut crossings = 0;
    while n < 20_000 {
        let a = point(&mut next);
        let b = point(&mut next);
        if off_plane(&m, a) < 1e-6 || off_plane(&m, b) < 1e-6 {
            continue;
        }
        let port = m.surface_bins_crossed_openmc(a, b, dir(a, b));
        let old = m.surface_bins_crossed(a, b);
        assert_eq!(port, old, "track {n}: {a:?} -> {b:?}");
        crossings += port.len();
        n += 1;
    }
    println!("20000 off-tie tracks identical, {crossings} surface bins");
    assert!(crossings > 20_000, "the tracks cross the mesh");
}

#[test]
fn a_track_ending_on_a_plane_does_not_count_that_plane() {
    let m = mesh();
    let u = Direction { u: 1.0, v: 0.0, w: 0.0 };
    // x planes at -5, -3, -1, 1, 3, 5; y = 0.5 and z = 1.0 are off planes.
    let a = Position::new(-4.0, 0.5, 1.0);
    let ends_on = Position::new(-1.0, 0.5, 1.0);
    let past = Position::new(-0.5, 0.5, 1.0);
    let on = m.surface_bins_crossed_openmc(a, ends_on, u);
    let beyond = m.surface_bins_crossed_openmc(a, past, u);
    // One plane crossed (x = -3): outward + inward.
    assert_eq!(on.len(), 2, "{on:?}");
    assert_eq!(beyond.len(), 4, "{beyond:?}");
    assert_eq!(on, m.surface_bins_crossed(a, ends_on));
    assert_eq!(beyond, m.surface_bins_crossed(a, past));
}
