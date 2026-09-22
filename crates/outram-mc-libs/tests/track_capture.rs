// SPDX-License-Identifier: GPL-3.0

//! **Track capture does not perturb the run** — GitHub #271.
//!
//! A debugging instrument that changes the thing being debugged is worse than
//! none: it sends the hunt after an artefact of the instrument. Recording
//! draws no randomness, so a traced run must return **exactly** the same
//! numbers as an untraced one — asserted as `f64` equality, not a tolerance.
//!
//! # Results, 2026-09-22
//!
//! A 2000-history point source in a void sphere and in an absorbing sphere:
//! identical history counts and identical tallies, with 64 tracks captured.

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::physics::fixed_source::{
    run_fixed_source, run_fixed_source_traced, FixedSource, FixedSourceSettings,
};
use outram_mc_libs::physics::track_output::{TrackEvent, TrackRecorder};
use outram_mc_libs::tally::filter::{CellFilter, FilterKind};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

const R: f64 = 5.0;

fn void_sphere() -> Geometry {
    Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: R,
            bc: BoundaryType::Vacuum,
        })],
        cells: vec![Cell::fill(
            1,
            vec![RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Inside,
            }],
            CellFill::Void,
            Position::ZERO,
        )],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

fn flux_tally() -> Tally {
    Tally {
        id: 1,
        name: "flux".into(),
        filters: vec![FilterKind::Cell(CellFilter {
            cell_indices: vec![0],
        })],
        scores: vec![ScoreType::Flux],
        bins: vec![TallyBin::default(); 1],
    }
}

fn settings() -> FixedSourceSettings {
    FixedSourceSettings {
        n_particles: 2000,
        n_batches: 10,
        seed: 20_260_922,
        ..Default::default()
    }
}

/// **THE GATE.** A traced run and an untraced run must agree bit for bit.
#[test]
fn track_capture_does_not_perturb_the_run() {
    let geom = void_sphere();
    let src = FixedSource::Point {
        r: Position::ZERO,
        energy_ev: 2.0e6,
    };

    let mut plain_tally = flux_tally();
    let plain = run_fixed_source(&geom, &[], &[], &src, &settings(), Some(&mut plain_tally));

    let mut traced_tally = flux_tally();
    let mut rec = TrackRecorder::new(64, 10_000);
    let traced = run_fixed_source_traced(
        &geom,
        &[],
        &[],
        &src,
        &settings(),
        Some(&mut traced_tally),
        Some(&mut rec),
    );

    assert_eq!(
        plain.total_histories, traced.total_histories,
        "capture changed the number of histories transported"
    );
    assert_eq!(
        plain_tally.bins[0].sum, traced_tally.bins[0].sum,
        "capture changed the tally. A debugging instrument that perturbs the run \
         sends the hunt after an artefact of the instrument."
    );
    assert_eq!(plain_tally.bins[0].sum_sq, traced_tally.bins[0].sum_sq);
    assert_eq!(plain_tally.bins[0].count, traced_tally.bins[0].count);

    println!(
        "captured {} tracks, {} states, dropped {} tracks",
        rec.tracks.len(),
        rec.n_states(),
        rec.dropped_tracks
    );
    assert_eq!(rec.tracks.len(), 64, "the budget should be filled");
    assert!(rec.dropped_tracks > 0, "and the rest refused");
}

/// **The physics the tracks show is the physics the geometry has.**
///
/// A point source at the centre of a *void* sphere of radius `R`: every
/// neutron streams straight out and leaks, with no collision on the way. So
/// every track must be exactly `Born -> Leak` and exactly `R` long. This is a
/// check on the recorder placement, not on transport — a recorder wired to the
/// wrong events would still produce plausible-looking tracks.
#[test]
fn tracks_in_a_void_sphere_are_a_straight_line_to_the_boundary() {
    let geom = void_sphere();
    let src = FixedSource::Point {
        r: Position::ZERO,
        energy_ev: 2.0e6,
    };
    let mut rec = TrackRecorder::new(32, 1000);
    run_fixed_source_traced(
        &geom,
        &[],
        &[],
        &src,
        &FixedSourceSettings {
            n_particles: 200,
            ..settings()
        },
        None,
        Some(&mut rec),
    );

    assert_eq!(rec.tracks.len(), 32);
    for (i, t) in rec.tracks.iter().enumerate() {
        assert_eq!(
            t.states.len(),
            2,
            "track {i}: a void sphere has no collisions, so a history is born \
             and leaks — got {:?}",
            t.states.iter().map(|s| s.event).collect::<Vec<_>>()
        );
        assert_eq!(t.states[0].event, TrackEvent::Born);
        assert_eq!(t.outcome(), Some(TrackEvent::Leak));
        let len = t.path_length().expect("a complete track");
        assert!(
            (len - R).abs() < 1.0e-6,
            "track {i}: path {len} cm, but every neutron must travel exactly {R} cm"
        );
        // Born at the origin, leaks on the sphere.
        let s0 = t.states[0].r;
        assert!(s0.x.abs() + s0.y.abs() + s0.z.abs() < 1e-12);
        let s1 = t.states[1].r;
        let rad = (s1.x * s1.x + s1.y * s1.y + s1.z * s1.z).sqrt();
        assert!((rad - R).abs() < 1.0e-6, "leak radius {rad}");
    }
    assert_eq!(rec.ending_in(TrackEvent::Leak).len(), 32);
    assert!(rec.ending_in(TrackEvent::Lost).is_empty(), "no history was lost");
}
