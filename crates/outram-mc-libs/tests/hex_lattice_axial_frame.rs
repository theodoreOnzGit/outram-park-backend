//! **A 3-D hex lattice must never report a tile boundary BEHIND the particle.**
//!
//! Regression gate for a port defect found 2026-09-17 while assembling the
//! HTR-10 core (`bn:op-867c`, gh #214).
//!
//! # The defect
//!
//! [`HexLattice::distance`] reconstructs the lattice-frame position from the
//! caller's tile-local one. It reconstructed **all three** components, but the
//! axial test at the end of that function compares `z` against
//! `+/- 0.5 * pitch[1]` — a **tile-local** half-height. Feeding it a
//! lattice-frame `z` makes the comparison wrong by the tile's own `z` offset,
//! so `(z0 - z) / u.w` comes out **negative** for every tile except the one
//! sitting at offset zero.
//!
//! OpenMC builds the hybrid — x,y lattice-frame, z tile-local — at the *call*
//! site instead (`src/geometry.cpp:459-467`), and guards the result with
//! `if (d_lat < 0) p.mark_as_lost(...)`. This port had neither.
//!
//! # Why it survived until now
//!
//! The error cancels **exactly** when the tile z-offset is zero, i.e.
//! `n_axial == 1`, and `from_rings_3d` had unit tests only — no integration
//! test and no example. Every existing test therefore sat on the one
//! configuration that hides it.
//!
//! # Why it was invisible in the answer
//!
//! A negative distance steps the neutron **backwards**, so it re-crosses the
//! same boundary forever until the per-history event budget kills it. A
//! budget-exhausted history is scored as a *leak*, so the neutron balance still
//! closes and `k` reports no error at all — the HTR-10 core returned
//! `k = 0.000000` with nothing in the output pointing at geometry.
//!
//! # Results (2026-09-17)
//!
//! Measured on the HTR-10 core model, negative distance-to-boundary counts:
//!
//! | `n_axial` | before | after |
//! |---|---|---|
//! | 1 | 0 | 0 |
//! | 2 | 8,199,697 | 0 |
//! | 20 | 17,498,719 (worst −7.7e3 cm) | 0 |
//!
//! and the core went from `k = 0.000000` (68–88 % of histories stuck) to
//! `k_eff = 0.707506 +/- 0.006010` with **zero** lost, stuck or negative.
//!
//! The tests below are self-contained and do not need that model.

use outram_mc_libs::geometry::lattice::{HexLattice, HexOrientation};
use outram_mc_libs::geometry::position::{Direction, Position};

/// Build a 3-D hex lattice of `n_axial` layers, one universe everywhere.
fn lattice(n_rings: usize, n_axial: usize, pitch: f64, height: f64) -> HexLattice {
    let levels: Vec<Vec<Vec<usize>>> = (0..n_axial)
        .map(|_| {
            (0..n_rings)
                .rev()
                .map(|ring| vec![1usize; if ring == 0 { 1 } else { 6 * ring }])
                .collect()
        })
        .collect();
    HexLattice::from_rings_3d(
        0,
        HexOrientation::Y,
        Position::new(0.0, 0.0, 0.0),
        pitch,
        height,
        &levels,
        Some(0),
    )
}

/// **The gate.** Sweep every tile and a fan of directions; no tile-local point
/// inside its own tile may report a negative distance to the tile boundary.
///
/// A negative here is never physical: the particle is inside the tile, so every
/// face is ahead of it along *some* component, and the minimum over faces is a
/// forward distance.
#[test]
fn a_3d_hex_lattice_never_reports_a_boundary_behind_the_particle() {
    const PITCH: f64 = 6.6106; // HTR-10 bed cell, flat-to-flat
    const HEIGHT: f64 = 4.899; // half the paper's prism, one ball per tile
    const N_RINGS: usize = 4;

    // n_axial = 1 is the configuration that HIDES the defect (offset zero), so
    // it is included deliberately: it must stay clean too.
    for n_axial in [1usize, 2, 5, 20] {
        let lat = lattice(N_RINGS, n_axial, PITCH, HEIGHT);
        let mut worst = f64::INFINITY;
        let mut worst_at = (0i32, 0i32, 0i32);

        for iz in 0..n_axial as i32 {
            for iy in 0..(2 * N_RINGS as i32 - 1) {
                for ix in 0..(2 * N_RINGS as i32 - 1) {
                    // Tile-local sample points, well inside the tile so the
                    // test is about the FRAME, not about edge tolerance.
                    for &(lx, ly, lz) in &[
                        (0.0, 0.0, 0.0),
                        (0.4, 0.3, 0.3 * HEIGHT),
                        (-0.5, 0.2, -0.35 * HEIGHT),
                        (0.1, -0.6, 0.45 * HEIGHT),
                    ] {
                        let r = Position::new(lx, ly, lz);
                        for &(uu, vv, ww) in &[
                            (0.0, 0.0, 1.0),
                            (0.0, 0.0, -1.0),
                            (0.6, 0.0, 0.8),
                            (-0.5, 0.5, -0.707_106_781_186_547_5),
                            (0.577_350_269_189_625_8, 0.577_350_269_189_625_8, 0.577_350_269_189_625_8),
                        ] {
                            let n: f64 = (uu * uu + vv * vv + ww * ww) as f64;
                            let n = n.sqrt();
                            let u = Direction::new(uu / n, vv / n, ww / n);
                            let (d, _) = lat.distance(r, u, [ix, iy, iz]);
                            if d < worst {
                                worst = d;
                                worst_at = (ix, iy, iz);
                            }
                        }
                    }
                }
            }
        }

        println!("n_axial = {n_axial:>2}: worst distance {worst:+.6e} cm at tile {worst_at:?}");
        assert!(
            worst >= 0.0,
            "n_axial = {n_axial}: HexLattice::distance returned {worst:e} cm at tile \
             {worst_at:?}. A NEGATIVE tile distance steps the particle backwards and it \
             oscillates until the event budget kills it -- and because a budget-exhausted \
             history is scored as a leak, the eigenvalue reports no error at all. The cause \
             is the axial branch comparing a LATTICE-frame z against the tile-local bound \
             `+/- 0.5 * pitch[1]`; z must stay tile-local (OpenMC src/geometry.cpp:459-467)."
        );
    }
}

/// The defect's signature was **`n_axial`-dependence**: correct at one layer,
/// wrong at more. Pin that the axial distance no longer depends on how many
/// layers the lattice happens to have, which is what the frame error caused.
///
/// Same tile-local point, same direction, same pitch and height — only the
/// layer count differs. A purely axial ray from the tile centre must travel
/// `HEIGHT/2` to the top face regardless.
#[test]
fn the_axial_distance_does_not_depend_on_the_layer_count() {
    const PITCH: f64 = 6.6106;
    const HEIGHT: f64 = 4.899;
    let r = Position::new(0.0, 0.0, 0.0);
    let u = Direction::new(0.0, 0.0, 1.0);

    let mut seen: Vec<(usize, f64)> = Vec::new();
    for n_axial in [1usize, 2, 5, 20] {
        let lat = lattice(3, n_axial, PITCH, HEIGHT);
        // Tile 1 in z wherever it exists, else tile 0 — the point is that a
        // NON-zero z index must behave like the zero one.
        let iz = if n_axial > 1 { 1 } else { 0 };
        let (d, _) = lat.distance(r, u, [2, 2, iz]);
        println!("n_axial = {n_axial:>2}, iz = {iz}: d = {d:.9} cm");
        seen.push((n_axial, d));
    }

    let expected = 0.5 * HEIGHT;
    for (n_axial, d) in seen {
        if n_axial == 1 {
            // A single-layer lattice is not 3-D, so there is no axial face to
            // cross and `inf` is the correct answer, not a defect. Asserted
            // explicitly rather than skipped, so the distinction stays visible.
            assert!(
                d.is_infinite(),
                "a 1-layer hex lattice has no axial faces; expected an infinite \
                 axial distance, got {d}"
            );
            continue;
        }
        assert!(
            (d - expected).abs() < 1.0e-9,
            "n_axial = {n_axial}: a +z ray from the tile centre travelled {d} cm to the \
             top face, expected {expected} cm (half the axial pitch). A layer-count \
             dependence here is the lattice-frame/tile-local z confusion returning."
        );
    }
}
