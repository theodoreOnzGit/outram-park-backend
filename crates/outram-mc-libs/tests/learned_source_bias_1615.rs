// SPDX-License-Identifier: AGPL-3.0-only
//! **Learned-importance source bias: off by default, unbiased through transport**
//! (gh:#1615; module `outram_mc_libs::stats::learned_source_bias`).
//!
//! # Methodology (fixed before the run)
//!
//! A vacuum sphere of radius 6 cm, void inside, with a uniform box source
//! `[-3,3] x [-1,1] x [-1,1]` (analog `FixedSource::Box`). One mesh cell
//! covering the sphere carries a track-length flux tally; its value per
//! source particle is the mean chord length from the source distribution, an
//! analog-vs-biased comparison with no physics in the way. The biased source is
//! `with_learned_source_bias` with a 6-cell window set along x whose `lower`
//! rises `1, 2, 4, 8, 16, 32` (importance falls 32x across the box), so the
//! births are strongly redistributed and carry weights spanning that range.
//! N = 5 seeds per arm, 20 000 histories each; z = (biased - analog) /
//! sqrt(s_a^2 + s_b^2) at the maintainer's gate `|z| <= 2`, asserted at the
//! gross-defect 5 (a 2-sigma assertion would fail a correct build 5 % of the
//! time) and printed.
//!
//! # Results
//!
//! Printed with `--nocapture`; the shielded-room measurement is in
//! `verification_and_validation/stats_epic_493/497_learned_importance.md`.

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::physics::fixed_source::{run_fixed_source, FixedSource, FixedSourceSettings};
use outram_mc_libs::physics::variance_reduction::VarianceReduction;
use outram_mc_libs::physics::weight_windows::WeightWindows;
use outram_mc_libs::stats::learned_source_bias::with_learned_source_bias;
use outram_mc_libs::tally::filter::{FilterKind, MeshFilter};
use outram_mc_libs::tally::mesh::{MeshKind, RegularMesh};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

const N_BATCHES: usize = 10;
const N_PER_SEED: usize = 20_000;

fn void_sphere() -> Geometry {
    Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: 6.0,
            bc: BoundaryType::Vacuum,
        })],
        cells: vec![Cell::fill(
            1,
            vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }],
            CellFill::Void,
            Position::ZERO,
        )],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    }
}

fn analog_box() -> FixedSource {
    FixedSource::Box {
        lower: Position::new(-3.0, -1.0, -1.0),
        upper: Position::new(3.0, 1.0, 1.0),
        energy_ev: 2.0e6,
    }
}

fn tally() -> Tally {
    let mesh = RegularMesh {
        lower_left: [-6.0, -6.0, -6.0],
        upper_right: [6.0, 6.0, 6.0],
        dimension: [1, 1, 1],
    };
    Tally {
        id: 1,
        name: "chord".into(),
        filters: vec![FilterKind::Mesh(MeshFilter { mesh: MeshKind::Regular(mesh) })],
        scores: vec![ScoreType::Flux],
        bins: vec![TallyBin::default(); 1],
    }
}

/// Importance windows: 6 cells along x over `[-3, 3]`, `lower = 1,2,4,8,16,32`.
fn windows() -> WeightWindows {
    let mesh = RegularMesh {
        lower_left: [-3.0, -1.0, -1.0],
        upper_right: [3.0, 1.0, 1.0],
        dimension: [6, 1, 1],
    };
    let lower: Vec<f64> = (0..6).map(|i| 2.0_f64.powi(i)).collect();
    let upper: Vec<f64> = lower.iter().map(|l| 5.0 * l).collect();
    WeightWindows::new(mesh, vec![0.0, 2.0e7], lower, upper).unwrap()
}

fn arm(src: &FixedSource, seed0: u64) -> Vec<f64> {
    (0..5u64)
        .map(|s| {
            let mut t = tally();
            let settings = FixedSourceSettings {
                n_particles: N_PER_SEED,
                n_batches: N_BATCHES,
                seed: seed0 + 1_000_003 * s,
                ..FixedSourceSettings::default()
            };
            run_fixed_source(&void_sphere(), &[], &[], src, &settings, Some(&mut t));
            t.bins[0].mean(N_BATCHES as u64) / (N_PER_SEED / N_BATCHES) as f64
        })
        .collect()
}

fn mean_sem(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0);
    (m, (var / n).sqrt())
}

/// Off by default: the analog source is a plain `Box`, the default
/// `VarianceReduction` carries no windows, and only the explicit call changes
/// the source (to an `Independent` one).
#[test]
fn source_bias_is_analog_unless_attached() {
    assert!(matches!(analog_box(), FixedSource::Box { .. }));
    assert!(VarianceReduction::default().weight_windows.is_none());
    let (biased, rep) = with_learned_source_bias(&analog_box(), &windows()).unwrap();
    assert!(matches!(biased, FixedSource::Independent(_)));
    assert_eq!(rep.n_cells, 6);
    assert!(rep.weight_rel_variance > 0.1, "the bias must actually redistribute: {rep:?}");
}

/// The biased source, run through transport (weights carried by
/// `run_fixed_source`), gives the analog mean chord length.
#[test]
fn biased_source_transport_matches_analog() {
    let (biased, _) = with_learned_source_bias(&analog_box(), &windows()).unwrap();
    let a = arm(&analog_box(), 11);
    let b = arm(&biased, 22);
    let (ma, sa) = mean_sem(&a);
    let (mb, sb) = mean_sem(&b);
    let z = (mb - ma) / (sa * sa + sb * sb).sqrt();
    println!("mean chord: analog {ma:.5} +- {sa:.2e}, biased {mb:.5} +- {sb:.2e}, z {z:+.2} [|z|<=2: {}]", z.abs() <= 2.0);
    assert!(z.abs() <= 5.0, "gross bias guard: z = {z}");
    assert!((ma - mb).abs() / ma < 0.02, "means differ by more than 2 %: {ma} vs {mb}");
}
