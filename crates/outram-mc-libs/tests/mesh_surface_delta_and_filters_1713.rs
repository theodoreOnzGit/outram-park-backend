//! **Mesh-surface tallies under delta tracking and with other filters**
//! (gh:#1713). Methodology, predictions (written before the runs) and results:
//! `verification_and_validation/mesh_surface_delta_and_filters_1713/README.md`.
//!
//! - `hand_built_bin_index_*`: the flattened bin of a mesh-surface filter
//!   combined with single-bin filters, on the live scorer and on the history
//!   driver, against indices worked out by hand from OpenMC's `FilterBinIter`
//!   (`tally_scoring.cpp:130-141`, strides `tally.cpp:496-509`).
//! - `delta_and_surface_tracking_give_the_same_mesh_surface_currents`: the
//!   same model surface-tracked and delta-tracked; delta tracking is new work
//!   (OpenMC has none), so the check is the statistical agreement of the two.

#![cfg(feature = "std")] // runs transport and prints a report (gh:#1605)

use petir::mathf::RealMath;

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings};
use outram_mc_libs::physics::transport_csg::{run_keff_csg, run_keff_csg_hybrid, SourceBox};
use outram_mc_libs::tally::filter::{EnergyFilter, FilterEvent, FilterKind};
use outram_mc_libs::tally::filter_extra::{EnergyFunctionFilter, MeshSurfaceFilter};
use outram_mc_libs::tally::history_events::{
    mesh_surface_bins, score_history_event, score_mesh_surface_segment, EventHistory, HistoryEvent,
    HistoryEventKind,
};
use outram_mc_libs::tally::mesh::{MeshKind, RegularMesh};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

fn tally(filters: Vec<FilterKind>) -> Tally {
    let n: usize = filters.iter().map(|f| f.n_bins()).product();
    Tally {
        id: 1,
        name: "t".into(),
        filters,
        scores: vec![ScoreType::Current],
        bins: vec![TallyBin::default(); n],
        derivative: None,
    }
}

fn two_element_mesh() -> MeshSurfaceFilter {
    MeshSurfaceFilter::new(MeshKind::Regular(RegularMesh {
        lower_left: [-1.0, -1.0, -1.0],
        upper_right: [1.0, 1.0, 1.0],
        dimension: [2, 1, 1],
    }))
    .unwrap()
}

fn energy_2g() -> FilterKind {
    FilterKind::Energy(EnergyFilter { bins: vec![1.0, 2.0, 3.0] })
}

/// No surfaces: a flight ending in a collision never reads the geometry.
fn empty_geometry() -> Geometry {
    Geometry { surfaces: vec![], cells: vec![], universes: vec![], lattices: vec![], root_universe: 0 }
}

/// Score one flight `(-0.5, 0, 0) -> (0.5, 0, 0)` ending in a collision, at
/// `energy`, weight 0.75, through the live scorer and the history driver;
/// both outputs.
fn score_both(t: &Tally, energy: f64) -> (Vec<f64>, Vec<f64>) {
    let r0 = Position::new(-0.5, 0.0, 0.0);
    let u = Direction { u: 1.0, v: 0.0, w: 0.0 };
    let hist = EventHistory::at_birth(r0);
    let mut live = vec![0.0; t.bins.len()];
    score_mesh_surface_segment(&mut live, t, &empty_geometry(), &hist, 0, 0, None, energy, 0.75, None, true, r0, u, 1.0);
    let he = HistoryEvent {
        kind: HistoryEventKind::Segment { distance: 1.0 },
        ev: FilterEvent { energy, direction: u, weight: 0.75, position: Position::ZERO, ..Default::default() },
        track_end: Position::new(0.5, 0.0, 0.0),
        mesh_surface_span: Some((r0, Position::new(0.5, 0.0, 0.0))),
    };
    let mut drv = vec![0.0; t.bins.len()];
    score_history_event(t, &he, &mut drv).expect("supported combination");
    (live, drv)
}

/// The two faces the flight crosses (out of element 0 through +x, into
/// element 1 through -x), from the filter itself.
fn faces() -> Vec<usize> {
    let ev = FilterEvent {
        position_last: Position::new(-0.5, 0.0, 0.0),
        position: Position::new(0.5, 0.0, 0.0),
        direction: Direction { u: 1.0, v: 0.0, w: 0.0 },
        ..Default::default()
    };
    let f = two_element_mesh().matches(&ev);
    assert_eq!(f.len(), 2, "the internal face is crossed once, scored on both sides: {f:?}");
    f
}

/// `[Energy, MeshSurface]`: bin `g * n_ms + face` (energy stride = n_ms).
/// `[MeshSurface, Energy]`: bin `face * 2 + g`. Energy 1.5 eV is group 0,
/// 2.5 eV group 1, 5 eV outside the filter and scores nothing. Every other bin
/// is zero; the scored value is the weight, 0.75, exactly. Prediction P4.
#[test]
fn hand_built_bin_index_energy_and_mesh_surface() {
    let n_ms = FilterKind::MeshSurface(two_element_mesh()).n_bins();
    assert_eq!(n_ms, 2 * 4 * 3, "two elements x 4 * n_dim face bins");
    let fs = faces();
    for (energy, g) in [(1.5, Some(0usize)), (2.5, Some(1)), (5.0, None)] {
        let a = tally(vec![energy_2g(), FilterKind::MeshSurface(two_element_mesh())]);
        let b = tally(vec![FilterKind::MeshSurface(two_element_mesh()), energy_2g()]);
        for (t, energy_first) in [(&a, true), (&b, false)] {
            let mut want = vec![0.0; t.bins.len()];
            if let Some(g) = g {
                for &f in &fs {
                    let index = if energy_first { g * n_ms + f } else { f * 2 + g };
                    want[index] = 0.75;
                }
            }
            let (live, drv) = score_both(t, energy);
            assert_eq!(live, want, "live, E = {energy}");
            assert_eq!(drv, want, "driver, E = {energy}");
        }
    }
}

/// `EnergyFunction x MeshSurface`: one bin, weight `y(E)`; at `E = 1.5` on the
/// line `y = 2E` the score is `0.75 * 3.0`. A second multi-match filter is
/// refused by the driver and scores nothing live.
#[test]
fn hand_built_bin_index_weights_and_refusals() {
    let ef = EnergyFunctionFilter::new(vec![1.0, 2.0], vec![2.0, 4.0]).unwrap();
    let t = tally(vec![FilterKind::EnergyFunction(ef), FilterKind::MeshSurface(two_element_mesh())]);
    let (live, drv) = score_both(&t, 1.5);
    let mut want = vec![0.0; t.bins.len()];
    for f in faces() {
        want[f] = 0.75 * 3.0;
    }
    assert_eq!(live, want);
    assert_eq!(drv, want);

    let two = tally(vec![FilterKind::MeshSurface(two_element_mesh()), FilterKind::MeshSurface(two_element_mesh())]);
    assert!(mesh_surface_bins(&two, &FilterEvent::default()).is_err());
    let r0 = Position::new(-0.5, 0.0, 0.0);
    let he = HistoryEvent {
        kind: HistoryEventKind::Segment { distance: 1.0 },
        ev: FilterEvent { energy: 1.5, ..Default::default() },
        track_end: Position::new(0.5, 0.0, 0.0),
        mesh_surface_span: Some((r0, Position::new(0.5, 0.0, 0.0))),
    };
    let mut out = vec![0.0; two.bins.len()];
    assert!(score_history_event(&two, &he, &mut out).is_err());
    let mut live = vec![0.0; two.bins.len()];
    let u = Direction { u: 1.0, v: 0.0, w: 0.0 };
    score_mesh_surface_segment(
        &mut live, &two, &empty_geometry(), &EventHistory::at_birth(r0), 0, 0, None, 1.5, 0.75, None, true, r0, u, 1.0,
    );
    assert!(live.iter().all(|&x| x == 0.0));
}

// ── Delta vs surface tracking ───────────────────────────────────────────────
// The three-shell reflective sphere of `tests/delta_tracking_audit.rs`
// (copied: test files share no code), LOW-tier U-235, U-238, C.

const T_COLD: f64 = 293.6;

fn nuclides() -> Vec<Nuclide> {
    ["U235", "U238", "C0"].iter().map(|n| Nuclide::from_core(n).expect("embedded evaluation")).collect()
}

fn materials() -> Vec<Material> {
    let comp = |nuclide_idx, atom_density| NuclideComponent { nuclide_idx, atom_density };
    vec![
        Material {
            id: 1,
            name: "fuel".into(),
            temperature: T_COLD,
            components: vec![comp(0, 4.0e-4), comp(1, 4.0e-3), comp(2, 5.0e-2)],
        },
        Material { id: 2, name: "graphite".into(), temperature: T_COLD, components: vec![comp(2, 8.0e-2)] },
        Material { id: 3, name: "dilute gas".into(), temperature: T_COLD, components: vec![comp(2, 1.0e-5)] },
    ]
}

fn log_grid(n: usize) -> Vec<f64> {
    let (lo, hi) = (1.0e-5_f64, 2.0e7_f64);
    (0..n).map(|i| (lo.r_ln() + (hi.r_ln() - lo.r_ln()) * i as f64 / (n - 1) as f64).r_exp()).collect()
}

/// Reflective sphere r = 5 cm: fuel r < 2, graphite 2 < r < 3, gas outside.
/// `delta`: the root cell declares delta tracking against majorant 0.
fn shells(delta: bool) -> Geometry {
    let sphere = |r, bc| SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r, bc });
    let hs = |surface_idx, sense| RegionToken::HalfSpace { surface_idx, sense };
    let shell = vec![hs(1, HalfSpaceSense::Outside), hs(2, HalfSpaceSense::Inside), RegionToken::Intersection];
    let root = Cell::fill(1, vec![hs(0, HalfSpaceSense::Inside)], CellFill::Universe(1), Position::ZERO);
    Geometry {
        surfaces: vec![
            sphere(5.0, BoundaryType::Reflective),
            sphere(2.0, BoundaryType::Transmissive),
            sphere(3.0, BoundaryType::Transmissive),
        ],
        cells: vec![
            Cell::material(10, vec![hs(1, HalfSpaceSense::Inside)], 0, None::<f64>),
            Cell::material(11, shell, 1, None::<f64>),
            Cell::material(12, vec![hs(2, HalfSpaceSense::Outside)], 2, None::<f64>),
            if delta { root.delta_tracked(0) } else { root },
        ],
        universes: vec![Universe { id: 0, cell_indices: vec![3] }, Universe { id: 1, cell_indices: vec![0, 1, 2] }],
        lattices: vec![],
        root_universe: 0,
    }
}

fn mesh_tally() -> Tally {
    tally(vec![FilterKind::MeshSurface(
        MeshSurfaceFilter::new(MeshKind::Regular(RegularMesh {
            lower_left: [-3.0, -3.0, -3.0],
            upper_right: [3.0, 3.0, 3.0],
            dimension: [3, 3, 3],
        }))
        .unwrap(),
    )])
}

/// **P2: delta tracking and surface tracking give the same mesh-surface
/// currents; P1: the tally does not move `k`.**
///
/// Methodology (fixed before running, README P2): the sphere above, 2000
/// particles x [5 inactive + 20 active], seed 3, one thread; surface tracking
/// (`run_keff_csg`) against the whole sphere delta-tracked
/// (`run_keff_csg_hybrid`, the audit's majorant over all three materials).
/// Tally: `Current` on a 3x3x3 regular mesh over `[-3, 3]^3` (324 face bins).
/// Per bin with a nonzero mean in both arms,
/// `z = (delta - surface) / sqrt(s_d^2 + s_s^2)` from the batch standard
/// deviations of the means; `chi2 = sum z^2` over those `N` bins. Pass:
/// `chi2 / N < 1.5`, `max |z| < 5`, the delta tally not all zero, and the
/// hybrid `k` with the tally bit-identical to the hybrid `k` without it.
///
/// Prediction (before running): passes, `chi2 / N` in 0.8-1.2. A double count
/// on the delta arm would give `chi2 / N` in the hundreds.
///
/// Results (2026-11-22): see the README.
#[test]
fn delta_and_surface_tracking_give_the_same_mesh_surface_currents() {
    let nucs = nuclides();
    let mats = materials();
    let maj = Majorant::over_indices(&mats, &[0, 1, 2], &nucs, &log_grid(1024), 0.1);
    let s = KeffSettings {
        n_particles: 2000,
        n_inactive: 5,
        n_active: 20,
        temperature_k: T_COLD,
        seed: 3,
        compute: ComputeType::CpuSingleThread,
        ..KeffSettings::default()
    };
    let n = 20_u64;
    let cube = SourceBox { lower: Position::new(-5.0, -5.0, -5.0), upper: Position::new(5.0, 5.0, 5.0) };
    let mut t_surf = mesh_tally();
    let surf = run_keff_csg(&shells(false), &mats, &nucs, cube, &s, Some(&mut t_surf));
    let mut t_delta = mesh_tally();
    let delta = run_keff_csg_hybrid(
        &shells(true),
        &mats,
        &nucs,
        std::slice::from_ref(&maj),
        None,
        cube,
        &s,
        Some(&mut t_delta),
    );
    let bare = run_keff_csg_hybrid(&shells(true), &mats, &nucs, std::slice::from_ref(&maj), None, cube, &s, None);
    assert_eq!(delta.k_mean.to_bits(), bare.k_mean.to_bits(), "a mesh-surface tally moved k");
    assert_eq!(delta.k_std.to_bits(), bare.k_std.to_bits(), "a mesh-surface tally moved sigma_k");

    let c = compare(&t_surf, &t_delta, n);
    println!(
        "k: surface {:.6} ± {:.6}, delta {:.6} ± {:.6}; mesh-surface current summed over faces: surface {:.6}, delta {:.6} (ratio {:.5})",
        surf.k_mean,
        surf.k_std,
        delta.k_mean,
        delta.k_std,
        c.sum_a,
        c.sum_b,
        c.sum_b / c.sum_a
    );
    println!(
        "delta vs surface: bins compared {} of {}, chi2 {:.2}, chi2/N {:.4}, max |z| {:.3}",
        c.n_bins,
        t_surf.bins.len(),
        c.chi2,
        c.chi2 / c.n_bins as f64,
        c.max_z
    );
    // Amendment 1 control: two independent surface-tracked arms, the same
    // statistic. Reported beside the gate, not gating.
    let mut t_ctrl = mesh_tally();
    let _ = run_keff_csg(&shells(false), &mats, &nucs, cube, &KeffSettings { seed: 4, ..s.clone() }, Some(&mut t_ctrl));
    let cc = compare(&t_surf, &t_ctrl, n);
    println!(
        "control, surface seed 3 vs surface seed 4: bins {}, chi2/N {:.4}, max |z| {:.3}",
        cc.n_bins,
        cc.chi2 / cc.n_bins as f64,
        cc.max_z
    );
    // `Cell [0, 1, 2] x MeshSurface` on the surface arm (same seed, so the same
    // transport: scoring draws nothing), summed over the three cells, against
    // the plain tally. Each batch's per-bin value is an integer count before
    // normalisation, so the sums agree to rounding of the normalisation only.
    let mut t_cell = tally(vec![
        FilterKind::Cell(outram_mc_libs::tally::filter::CellFilter { cell_indices: vec![0, 1, 2] }),
        mesh_tally().filters.remove(0),
    ]);
    let surf2 = run_keff_csg(&shells(false), &mats, &nucs, cube, &s, Some(&mut t_cell));
    assert_eq!(surf2.k_mean.to_bits(), surf.k_mean.to_bits());
    let n_ms = t_surf.bins.len();
    let mut per_cell = [0.0f64; 3];
    let mut worst = 0.0f64;
    for f in 0..n_ms {
        let sum: f64 = (0..3).map(|k| t_cell.bins[k * n_ms + f].sum).sum();
        for (k, pc) in per_cell.iter_mut().enumerate() {
            *pc += t_cell.bins[k * n_ms + f].sum;
        }
        let want = t_surf.bins[f].sum;
        worst = worst.max((sum - want).abs() / want.abs().max(1.0e-300));
    }
    println!("Cell x MeshSurface: summed over cells vs plain, worst relative difference {worst:.3e}; per cell {per_cell:?}");
    assert!(worst < 1.0e-12, "cell-summed mesh-surface current differs from the plain tally: {worst:.3e}");
    assert!(per_cell.iter().all(|&x| x > 0.0), "a cell never scored: {per_cell:?}");
    assert!(c.sum_b > 0.0, "the delta-tracked mesh-surface tally scored nothing");
    assert!(c.n_bins > 100, "too few bins scored in both arms: {}", c.n_bins);
    assert!(c.chi2 / (c.n_bins as f64) < 1.5, "chi2/N = {:.3}", c.chi2 / c.n_bins as f64);
    assert!(c.max_z < 5.0, "max |z| = {:.3}", c.max_z);
}

/// Per-bin comparison of two tallies' batch means.
struct Comparison {
    chi2: f64,
    n_bins: usize,
    max_z: f64,
    sum_a: f64,
    sum_b: f64,
}

fn compare(a: &Tally, b: &Tally, n: u64) -> Comparison {
    let mut c = Comparison { chi2: 0.0, n_bins: 0, max_z: 0.0, sum_a: 0.0, sum_b: 0.0 };
    for (ba, bb) in a.bins.iter().zip(&b.bins) {
        let (ma, mb) = (ba.mean(n), bb.mean(n));
        c.sum_a += ma;
        c.sum_b += mb;
        if ma == 0.0 || mb == 0.0 {
            continue;
        }
        let (sa, sb) = (ma * ba.rel_std_dev(n), mb * bb.rel_std_dev(n));
        let z = (mb - ma) / sa.r_hypot(sb).max(1.0e-300);
        c.chi2 += z * z;
        c.n_bins += 1;
        c.max_z = c.max_z.max(z.abs());
    }
    c
}
