// SPDX-License-Identifier: GPL-3.0

//! **A distribcell tally, end to end** — GitHub #261.
//!
//! The offset tables and the filter each have their own unit tests. This is
//! the one that runs **transport** and checks the two are actually connected:
//! that `FilterEvent::cell_instance` is set from the tables, that the filter
//! reads it, and that the per-instance bins end up distinguishable.
//!
//! # Why an end-to-end test and not just the units
//!
//! Because the failure this is guarding against is precisely a *disconnected*
//! unit. #261's first increment shipped eight filters that passed their own
//! tests and could not be put on a tally. The distribcell tables could pass
//! theirs and never be consulted by the transport, and every per-instance
//! tally would then come back with one populated bin — which looks like a
//! geometry with one instance rather than like a broken wiring.
//!
//! # The case
//!
//! A 1 × 4 × 1 lattice of identical tiles, each holding the same target cell,
//! so there are **four instances of one cell**. A point source sits in tile 0.
//! Flux is tallied with a `DistribcellFilter`.
//!
//! The tiles are void, so neutrons stream; with the source at one end the flux
//! must fall monotonically along the lattice and **every** instance must be
//! reached. Both halves matter: all-four-reached rules out a filter that only
//! ever sees instance 0, and monotone-decreasing rules out one that scrambles
//! the instance numbering.
//!
//! # Results, 2026-09-22
//!
//! Printed at run time.

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::distribcell::DistribcellOffsets;
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::lattice::{Lattice, RectLattice};
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{
    BoundaryType, SurfaceKind, XPlane, YPlane, ZPlane,
};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::physics::fixed_source::{
    run_fixed_source_traced, FixedSource, FixedSourceSettings,
};
use outram_mc_libs::tally::filter::FilterKind;
use outram_mc_libs::tally::filter_extra::DistribcellFilter;
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

const N_TILES: usize = 4;
const PITCH: f64 = 5.0;

/// Root universe: one cell filled by a 1 × 4 × 1 lattice; every tile is
/// universe 1, whose single cell (index 1) is the target.
fn lattice_geometry() -> Geometry {
    // The outer box, vacuum on every face.
    let half_y = 0.5 * PITCH * N_TILES as f64;
    let surfaces = vec![
        SurfaceKind::XPlane(XPlane { x0: -0.5 * PITCH, bc: BoundaryType::Vacuum }),
        SurfaceKind::XPlane(XPlane { x0: 0.5 * PITCH, bc: BoundaryType::Vacuum }),
        SurfaceKind::YPlane(YPlane { y0: -half_y, bc: BoundaryType::Vacuum }),
        SurfaceKind::YPlane(YPlane { y0: half_y, bc: BoundaryType::Vacuum }),
        SurfaceKind::ZPlane(ZPlane { z0: -0.5 * PITCH, bc: BoundaryType::Vacuum }),
        SurfaceKind::ZPlane(ZPlane { z0: 0.5 * PITCH, bc: BoundaryType::Vacuum }),
    ];
    let mut region = vec![
        RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Outside },
        RegionToken::HalfSpace { surface_idx: 1, sense: HalfSpaceSense::Inside },
        RegionToken::Intersection,
    ];
    for s in 2..6 {
        region.push(RegionToken::HalfSpace {
            surface_idx: s,
            sense: if s % 2 == 0 { HalfSpaceSense::Outside } else { HalfSpaceSense::Inside },
        });
        region.push(RegionToken::Intersection);
    }
    // The tile cell: everything (the lattice bounds it).
    let tile_region = vec![RegionToken::HalfSpace {
        surface_idx: 1,
        sense: HalfSpaceSense::Inside,
    }];
    Geometry {
        surfaces,
        cells: vec![
            Cell::fill(1, region, CellFill::Lattice(0), Position::ZERO),
            Cell::fill(2, tile_region, CellFill::Void, Position::ZERO),
        ],
        universes: vec![
            Universe { id: 0, cell_indices: vec![0] },
            Universe { id: 1, cell_indices: vec![1] },
        ],
        lattices: vec![Lattice::Rect(RectLattice {
            id: 1,
            n: [1, N_TILES, 1],
            lower_left: Position::new(-0.5 * PITCH, -half_y, -0.5 * PITCH),
            pitch: [PITCH, PITCH, PITCH],
            universes: vec![1; N_TILES],
            outer: None,
        })],
        root_universe: 0,
    }
}

/// **THE GATE.** A distribcell tally distinguishes the four instances.
#[test]
fn a_distribcell_tally_distinguishes_lattice_instances() {
    let geom = lattice_geometry();
    let offsets = DistribcellOffsets::build(&geom, 1).expect("offset tables");
    assert_eq!(
        offsets.n_instances, N_TILES,
        "a 1x{N_TILES}x1 lattice of a one-instance universe has {N_TILES} instances"
    );

    let mut tally = Tally {
        id: 1,
        name: "per-instance flux".into(),
        filters: vec![FilterKind::Distribcell(DistribcellFilter {
            cell_idx: 1,
            n_instances: offsets.n_instances,
        })],
        scores: vec![ScoreType::Flux],
        bins: vec![TallyBin::default(); offsets.n_instances],
    };

    // Source at the centre of tile 0 (the most negative y).
    let half_y = 0.5 * PITCH * N_TILES as f64;
    let src = FixedSource::Point {
        r: Position::new(0.0, -half_y + 0.5 * PITCH, 0.0),
        energy_ev: 2.0e6,
    };
    run_fixed_source_traced(
        &geom,
        &[],
        &[],
        &src,
        &FixedSourceSettings {
            n_particles: 20_000,
            n_batches: 10,
            ..Default::default()
        },
        Some(&mut tally),
        None,
        None,
        Some(&offsets),
    );

    let flux: Vec<f64> = tally.bins.iter().map(|b| b.sum).collect();
    println!("per-instance flux: {flux:?}");

    assert!(
        flux.iter().all(|&f| f > 0.0),
        "instance {:?} was never reached. A filter that only ever sees instance 0 \
         looks like a geometry with one instance rather than like broken wiring, \
         which is why this checks ALL of them: {flux:?}",
        flux.iter().position(|&f| f <= 0.0)
    );
    assert!(
        flux.windows(2).all(|w| w[0] > w[1]),
        "the flux must fall monotonically away from the source tile; {flux:?} does \
         not, which means the instance numbering is scrambled"
    );
    // Instance 0 holds the source, so it must dominate.
    let total: f64 = flux.iter().sum();
    assert!(
        flux[0] / total > 0.4,
        "the source tile holds only {:.1} % of the flux",
        100.0 * flux[0] / total
    );
}

/// Without the offset tables the transport sets no instance, and the filter
/// declines every event — **it must not silently bin everything into 0.**
#[test]
fn without_the_tables_a_distribcell_tally_scores_nothing() {
    let geom = lattice_geometry();
    let mut tally = Tally {
        id: 1,
        name: "per-instance flux".into(),
        filters: vec![FilterKind::Distribcell(DistribcellFilter {
            cell_idx: 1,
            n_instances: N_TILES,
        })],
        scores: vec![ScoreType::Flux],
        bins: vec![TallyBin::default(); N_TILES],
    };
    let half_y = 0.5 * PITCH * N_TILES as f64;
    run_fixed_source_traced(
        &geom,
        &[],
        &[],
        &FixedSource::Point {
            r: Position::new(0.0, -half_y + 0.5 * PITCH, 0.0),
            energy_ev: 2.0e6,
        },
        &FixedSourceSettings {
            n_particles: 2_000,
            n_batches: 5,
            ..Default::default()
        },
        Some(&mut tally),
        None,
        None,
        None, // no tables
    );
    let flux: Vec<f64> = tally.bins.iter().map(|b| b.sum).collect();
    println!("without tables: {flux:?}");
    assert!(
        flux.iter().all(|&f| f == 0.0),
        "with no instance available the filter must decline, not bin into 0: {flux:?}"
    );
}
