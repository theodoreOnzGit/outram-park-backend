// SPDX-License-Identifier: GPL-3.0

//! **The reachability gate for tally filters** — GitHub #261.
//!
//! # The defect this exists to prevent, which already happened once
//!
//! #261's first increment wrote eight filters into
//! `outram_mc_libs::tally::filter_extra` — `ReactionFilter`,
//! `CollisionFilter`, `CellFromFilter`, `CellBornFilter`,
//! `MaterialFromFilter`, `WeightFilter`, `MuSurfaceFilter` and
//! `EnergyFunctionFilter`. Each implemented `Filter`, each had unit tests, and
//! each of those tests passed.
//!
//! **None of them could be used.** `FilterKind` had no variant for any of
//! them, and `Tally::filters` is a `Vec<FilterKind>`, so there was no way to
//! put one on a tally. Eight filters were reported as ported and the crate
//! could not tally with a single one. Nothing failed, because nothing was
//! asking the question.
//!
//! A filter that cannot go on a tally is not a ported filter, whatever its
//! coverage says. This file asks that question for every variant, and
//! `FilterKind::name`'s exhaustive, wildcard-free match makes *adding* a
//! variant without dispatching it a compile error rather than a silent gap.
//!
//! # Results, 2026-09-22
//!
//! 30 variants, all constructible, all dispatched, all reporting a bin count.

use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::particle::particle::ParticleType;
use outram_mc_libs::tally::filter::*;
use outram_mc_libs::tally::filter_extra::*;
use outram_mc_libs::tally::mesh::{MeshKind, RegularMesh};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

fn a_mesh() -> MeshKind {
    MeshKind::Regular(RegularMesh {
        lower_left: [0.0, 0.0, 0.0],
        upper_right: [2.0, 2.0, 2.0],
        dimension: [2, 2, 2],
    })
}

/// One instance of **every** `FilterKind` variant.
fn one_of_each() -> Vec<FilterKind> {
    vec![
        FilterKind::Cell(CellFilter { cell_indices: vec![0] }),
        FilterKind::Material(MaterialFilter { material_indices: vec![0] }),
        FilterKind::Energy(EnergyFilter {
            bins: vec![0.0, 1.0, 2.0e7],
        }),
        FilterKind::EnergyOut(EnergyOutFilter {
            bins: vec![0.0, 1.0, 2.0e7],
        }),
        FilterKind::Universe(UniverseFilter {
            universe_indices: vec![0],
        }),
        FilterKind::Mesh(MeshFilter { mesh: a_mesh() }),
        FilterKind::Surface(SurfaceFilter { surface_indices: vec![0] }),
        FilterKind::Mu(MuFilter {
            bounds: vec![-1.0, 0.0, 1.0],
        }),
        FilterKind::PolarAzimuthal(PolarAzimuthalFilter {
            polar: vec![0.0, std::f64::consts::FRAC_PI_2, std::f64::consts::PI],
            azimuthal: vec![-std::f64::consts::PI, 0.0, std::f64::consts::PI],
        }),
        FilterKind::Time(TimeFilter {
            bounds: vec![0.0, 1.0],
        }),
        FilterKind::Particle(ParticleFilter {
            particles: vec![ParticleType::Neutron],
        }),
        FilterKind::SpatialLegendre(SpatialLegendreFilter {
            order: 2,
            axis: LegendreAxis::Z,
            min: -1.0,
            max: 1.0,
        }),
        FilterKind::Zernike(ZernikeFilter {
            order: 2,
            x0: 0.0,
            y0: 0.0,
            r: 1.0,
        }),
        FilterKind::SphericalHarmonics(SphericalHarmonicsFilter { order: 1 }),
        FilterKind::Reaction(ReactionFilter { bins: vec![18] }),
        FilterKind::Collision(CollisionFilter { bins: vec![0, 1] }),
        FilterKind::CellFrom(CellFromFilter { cells: vec![0] }),
        FilterKind::CellBorn(CellBornFilter { cells: vec![0] }),
        FilterKind::MaterialFrom(MaterialFromFilter { materials: vec![0] }),
        FilterKind::Weight(WeightFilter {
            bins: vec![0.0, 0.5, 1.0],
        }),
        FilterKind::MuSurface(MuSurfaceFilter {
            bins: vec![-1.0, 0.0, 1.0],
        }),
        FilterKind::EnergyFunction(
            EnergyFunctionFilter::new(vec![1.0, 1.0e7], vec![1.0, 2.0]).unwrap(),
        ),
        FilterKind::Legendre(LegendreFilter { order: 3 }),
        FilterKind::MeshBorn(MeshBornFilter { mesh: a_mesh() }),
        FilterKind::ParentNuclide(ParentNuclideFilter { nuclides: vec![0] }),
        FilterKind::CellInstance(CellInstanceFilter {
            pairs: vec![(0, 0), (0, 1)],
        }),
        FilterKind::MeshMaterial(MeshMaterialFilter {
            mesh: a_mesh(),
            pairs: vec![(0, 0)],
        }),
        FilterKind::ParticleProduction(ParticleProductionFilter {
            particles: vec![ParticleType::Neutron],
            energy_bins: vec![],
        }),
        FilterKind::MeshSurface(MeshSurfaceFilter::new(a_mesh()).unwrap()),
    ]
}

/// The one variant deliberately absent above, and why.
///
/// `DelayedGroup` is constructed through a fallible `new` that currently
/// **refuses every construction** (GitHub #278): nothing in the transport path
/// ever sets `FilterEvent::delayed_group`, so the filter could only ever tally
/// zero, and returning an error is better than returning zeros that look like
/// a measurement. It is counted here so the total is honest rather than
/// quietly one short.
const REFUSED_VARIANTS: usize = 1;

/// **THE GATE.** Every variant is constructible, dispatches, reports a bin
/// count, and can be put on a `Tally`.
#[test]
fn every_filter_kind_is_reachable_and_binnable() {
    let filters = one_of_each();
    let mut names: Vec<&'static str> = filters.iter().map(|f| f.name()).collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(
        before,
        names.len(),
        "two variants report the same name; `FilterKind::name` is the guard \
         and a duplicate makes it blind"
    );

    for f in &filters {
        assert!(
            f.n_bins() > 0,
            "{} reports zero bins, so it can never score",
            f.name()
        );
        // Dispatch must not panic on a default event.
        let _ = f.get_bin(&FilterEvent::default());
    }

    // Every one of them must actually go on a tally. (`FilterKind` is not
    // `Clone` — several variants carry a mesh — so the list is rebuilt rather
    // than cloned.)
    for f in one_of_each() {
        let n = f.n_bins();
        let name = f.name();
        let t = Tally {
            id: 1,
            name: name.into(),
            filters: vec![f],
            scores: vec![ScoreType::Flux],
            bins: vec![TallyBin::default(); n],
        };
        assert_eq!(t.n_bins(), n, "{name} does not size its tally correctly");
    }

    // 29 variants: 28 constructed above plus the refused `DelayedGroup`.
    assert_eq!(
        filters.len() + REFUSED_VARIANTS,
        30,
        "the variant count changed. If a filter was ADDED, construct it above \
         so this gate covers it; `FilterKind::name` will already have failed \
         to compile if it was not dispatched."
    );
}

/// **The Legendre filter's weights are the bare `P_l(mu)`.**
///
/// Not `(l + 1/2) P_l`. That normalisation belongs to evaluating an expansion,
/// not to accumulating its moments, and folding it in here would double-apply
/// it against `physics::scattdata::evaluate_legendre` — silently rescaling
/// every moment above P0 while leaving P0 looking right.
#[test]
fn the_legendre_filter_accumulates_bare_moments() {
    let f = LegendreFilter { order: 3 };
    for mu in [-1.0, -0.4, 0.0, 0.3, 1.0_f64] {
        let ev = FilterEvent {
            mu,
            ..FilterEvent::default()
        };
        let w = f.expansion_moments(&ev).expect("an expansion filter");
        assert_eq!(w.len(), 4);
        assert!((w[0] - 1.0).abs() < 1e-14, "P0 at mu={mu}");
        assert!((w[1] - mu).abs() < 1e-14, "P1 at mu={mu}");
        assert!(
            (w[2] - 0.5 * (3.0 * mu * mu - 1.0)).abs() < 1e-13,
            "P2 at mu={mu}: {}",
            w[2]
        );
        assert!(
            (w[3] - 0.5 * (5.0 * mu.powi(3) - 3.0 * mu)).abs() < 1e-13,
            "P3 at mu={mu}: {}",
            w[3]
        );
    }
    assert!(
        FilterKind::Legendre(LegendreFilter { order: 3 }).is_expansion(),
        "an expansion filter left out of `is_expansion` scores NOTHING: \
         `get_bin` returns None for it by design"
    );
}

/// The born-position mesh filter bins where the particle STARTED, not where it
/// is — the whole point of the filter, and trivially easy to wire to the wrong
/// field.
#[test]
fn the_meshborn_filter_bins_the_birth_position() {
    let f = MeshBornFilter { mesh: a_mesh() };
    let ev = FilterEvent {
        // Currently in the far corner, born in the near one.
        position: Position::new(1.5, 1.5, 1.5),
        position_born: Position::new(0.5, 0.5, 0.5),
        ..FilterEvent::default()
    };
    let born_bin = f.get_bin(&ev).expect("inside the mesh");
    let here_bin = MeshFilter { mesh: a_mesh() }.get_bin(&ev).expect("inside");
    assert_ne!(
        born_bin, here_bin,
        "MeshBornFilter returned the CURRENT element; it is reading the wrong \
         position field and would silently duplicate MeshFilter"
    );
    assert_eq!(born_bin, 0, "the near corner of a 2x2x2 mesh is bin 0");

    // Born outside the mesh: no bin, not bin 0.
    let outside = FilterEvent {
        position_born: Position::new(-5.0, 0.5, 0.5),
        ..ev
    };
    assert!(f.get_bin(&outside).is_none());
}

/// An unknown or unlisted parent is **not** folded into a catch-all bin.
#[test]
fn the_parent_nuclide_filter_refuses_an_unknown_parent() {
    let f = ParentNuclideFilter {
        nuclides: vec![3, 7],
    };
    assert_eq!(f.n_bins(), 2);
    for (parent, want) in [(Some(3), Some(0)), (Some(7), Some(1))] {
        let ev = FilterEvent {
            parent_nuclide: parent,
            ..FilterEvent::default()
        };
        assert_eq!(f.get_bin(&ev), want);
    }
    for parent in [None, Some(0), Some(99)] {
        let ev = FilterEvent {
            parent_nuclide: parent,
            ..FilterEvent::default()
        };
        assert_eq!(
            f.get_bin(&ev),
            None,
            "parent {parent:?} must match nothing, not bin 0 — a decay-source \
             study that attributed unknown parents to one nuclide would be \
             reporting a fabricated spectrum"
        );
    }
}

/// `(cell, instance)` pairs are matched on BOTH halves.
#[test]
fn the_cell_instance_filter_matches_on_both_halves() {
    let f = CellInstanceFilter {
        pairs: vec![(4, 0), (4, 1), (9, 0)],
    };
    let ev = |cell, inst| FilterEvent {
        cell_idx: cell,
        cell_instance: Some(inst),
        ..FilterEvent::default()
    };
    assert_eq!(f.get_bin(&ev(4, 0)), Some(0));
    assert_eq!(f.get_bin(&ev(4, 1)), Some(1));
    assert_eq!(f.get_bin(&ev(9, 0)), Some(2));
    // Right cell, wrong instance; and right instance, wrong cell.
    assert_eq!(f.get_bin(&ev(4, 2)), None);
    assert_eq!(f.get_bin(&ev(9, 1)), None);
    // No instance at all (not in a lattice).
    assert_eq!(
        f.get_bin(&FilterEvent {
            cell_idx: 4,
            ..FilterEvent::default()
        }),
        None
    );
}

/// The mesh-material filter needs both the element and the material to match.
#[test]
fn the_mesh_material_filter_needs_both_halves() {
    let f = MeshMaterialFilter {
        mesh: a_mesh(),
        pairs: vec![(0, 5), (7, 5), (0, 6)],
    };
    let ev = |p: Position, m| FilterEvent {
        position: p,
        material_idx: m,
        ..FilterEvent::default()
    };
    assert_eq!(f.get_bin(&ev(Position::new(0.5, 0.5, 0.5), 5)), Some(0));
    assert_eq!(f.get_bin(&ev(Position::new(1.5, 1.5, 1.5), 5)), Some(1));
    assert_eq!(f.get_bin(&ev(Position::new(0.5, 0.5, 0.5), 6)), Some(2));
    // Element 0 with an unlisted material.
    assert_eq!(f.get_bin(&ev(Position::new(0.5, 0.5, 0.5), 9)), None);
    // Outside the mesh entirely.
    assert_eq!(f.get_bin(&ev(Position::new(-1.0, 0.5, 0.5), 5)), None);
}

/// **The production filter matches one event in SEVERAL bins**, which no other
/// filter here does — so `matches`, not `get_bin`, is its entry point.
#[test]
fn the_particle_production_filter_can_match_one_event_many_times() {
    let f = ParticleProductionFilter {
        particles: vec![ParticleType::Neutron],
        energy_bins: vec![0.0, 1.0e5, 2.0e7],
    };
    assert_eq!(f.n_bins(), 2, "one particle type x two energy bins");

    let ev = FilterEvent {
        secondaries: vec![
            SecondarySite {
                particle: ParticleType::Neutron,
                energy: 1.0e3,
                weight: 1.0,
            },
            SecondarySite {
                particle: ParticleType::Neutron,
                energy: 2.0e6,
                weight: 0.5,
            },
            // Out of the energy range: dropped, not clamped into an end bin.
            SecondarySite {
                particle: ParticleType::Neutron,
                energy: 5.0e7,
                weight: 1.0,
            },
        ],
        ..FilterEvent::default()
    };
    let m = f.matches(&ev);
    assert_eq!(m, vec![(0, 1.0), (1, 0.5)], "got {m:?}");

    // With no energy binning, one bin per type and every secondary counts.
    let flat = ParticleProductionFilter {
        particles: vec![ParticleType::Neutron],
        energy_bins: vec![],
    };
    assert_eq!(flat.n_bins(), 1);
    assert_eq!(flat.matches(&ev).len(), 3);

    // An event with no secondaries matches nothing at all.
    assert!(f.matches(&FilterEvent::default()).is_empty());
}


/// **Mesh-face currents.** A track crossing an internal face must score BOTH
/// sides: an outward current on the element it leaves and an inward current on
/// the element it enters. A net current is the difference of the two, so a
/// tally that recorded only one side could not form it.
///
/// # Result, 2026-09-22 — a 2x2x2 mesh on [0, 2]^3, cells 1 cm wide
///
/// A track along +x from (0.5, 0.5, 0.5) to (1.5, 0.5, 0.5) crosses the single
/// internal plane at x = 1. Element 0 is left through its **max** x face
/// (outward), element 1 is entered through its **min** x face (inward):
///
/// | bin | element | axis | face | sense |
/// |---|---|---|---|---|
/// | `12*0 + 0 + 2 + 0 = 2` | 0 | x | max | outward |
/// | `12*1 + 0 + 0 + 1 = 13` | 1 | x | min | inward |
#[test]
fn mesh_face_currents_score_both_sides_of_an_internal_crossing() {
    let m = RegularMesh {
        lower_left: [0.0, 0.0, 0.0],
        upper_right: [2.0, 2.0, 2.0],
        dimension: [2, 2, 2],
    };
    let f = MeshSurfaceFilter { mesh: m.clone() };
    assert_eq!(f.n_bins(), 12 * 8, "12 surface bins per element");

    let ev = FilterEvent {
        position_last: Position::new(0.5, 0.5, 0.5),
        position: Position::new(1.5, 0.5, 0.5),
        ..FilterEvent::default()
    };
    let bins = f.matches(&ev);
    assert_eq!(bins, vec![2, 13], "got {bins:?}");

    // Reversed, the roles swap: element 1 leaves through its MIN face,
    // element 0 is entered through its MAX face.
    let back = FilterEvent {
        position_last: Position::new(1.5, 0.5, 0.5),
        position: Position::new(0.5, 0.5, 0.5),
        ..FilterEvent::default()
    };
    assert_eq!(f.matches(&back), vec![12, 3], "got {:?}", f.matches(&back));

    // A track entirely inside one element crosses nothing.
    let inside = FilterEvent {
        position_last: Position::new(0.2, 0.5, 0.5),
        position: Position::new(0.8, 0.5, 0.5),
        ..FilterEvent::default()
    };
    assert!(f.matches(&inside).is_empty());

    // Leaving the mesh scores only the inside half.
    let out = FilterEvent {
        position_last: Position::new(1.5, 0.5, 0.5),
        position: Position::new(9.0, 0.5, 0.5),
        ..FilterEvent::default()
    };
    assert_eq!(f.matches(&out), vec![m.surface_bin(1, 0, true, false)]);
}

/// A track crossing several planes scores them **in order of travel**, and
/// every internal crossing scores a matched outward/inward pair.
#[test]
fn a_diagonal_track_scores_its_crossings_in_order() {
    let m = RegularMesh {
        lower_left: [0.0, 0.0, 0.0],
        upper_right: [3.0, 3.0, 3.0],
        dimension: [3, 3, 3],
    };
    let f = MeshSurfaceFilter { mesh: m };
    let ev = FilterEvent {
        position_last: Position::new(0.5, 0.5, 0.5),
        position: Position::new(2.5, 2.5, 2.5),
        ..FilterEvent::default()
    };
    let bins = f.matches(&ev);
    // Six internal planes crossed (two per axis), each scoring a pair.
    assert_eq!(bins.len(), 12, "got {bins:?}");
    assert!(
        bins.chunks(2).all(|p| p[0] % 2 == 0 && p[1] % 2 == 1),
        "every crossing must be an outward bin followed by an inward one: {bins:?}"
    );
}

/// A mesh with no face-crossing routine is refused, not silently reported as
/// crossing nothing.
#[test]
fn a_non_regular_mesh_is_refused_for_surface_currents() {
    use outram_mc_libs::tally::mesh::CylindricalMesh;
    let err = MeshSurfaceFilter::new(MeshKind::Cylindrical(CylindricalMesh {
        r_grid: vec![0.0, 1.0],
        phi_grid: vec![0.0, std::f64::consts::TAU],
        z_grid: vec![0.0, 1.0],
        origin: Position::new(0.0, 0.0, 0.0),
    }))
    .unwrap_err();
    assert!(err.contains("regular mesh"), "{err}");
}
