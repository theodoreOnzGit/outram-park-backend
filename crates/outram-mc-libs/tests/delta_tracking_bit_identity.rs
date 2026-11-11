//! **Bit-identity gate for every delta-tracking path** (gh:#599 audit, 2026-10-06).
//!
//! # Why this exists
//!
//! The delta-tracking code was refactored for readability (one module,
//! `physics::delta_tracking`, holding the majorant, the flight loop, the
//! tentative-collision scoring hook and the hand-off to surface tracking).
//! A readability refactor must not move a single bit of any answer. The
//! crate's existing fingerprint gate (`stats_move_fingerprints.rs`) covers the
//! explicit-TRISO `keff_delta` run but **not the hybrid CSG driver**, its
//! tally estimators, the parallel and progress paths, or the flight
//! primitives. This file covers all of them on one small model.
//!
//! # Methodology
//!
//! A reflective sphere, radius 5 cm, holding three concentric zones (fuel
//! r < 2, graphite 2 < r < 3, dilute "gas" graphite 3 < r < 5), LOW-tier
//! (`Nuclide::from_core`) U-235, U-238 and C. Every output `f64` is folded,
//! bit pattern by bit pattern, into a 64-bit FNV-1a hash:
//!
//! - hybrid CSG, single thread: no tally; a flux/total tally by cell ×
//!   material × 2 groups with the tentative-collision estimator; the same
//!   tally with the real-collision ablation; the progress-callback entry
//!   point; the multi-thread path (2 threads) with the tally;
//! - `run_keff_delta_seq_in`, `run_keff_delta_par_in` (2 threads) and
//!   `DeltaPowerIteration` on the same zones;
//! - the primitives: `Majorant::from_materials` / `bounding` / `at`,
//!   `track_to_collision`, `bounded_delta_flight_visiting` (with the visitor's
//!   arguments folded in).
//!
//! The pins were taken on the code BEFORE the refactor, so a pass after it is
//! the evidence that the refactor is behaviour-preserving. Print the values
//! with `DELTA_FP_PRINT=1`.
//!
//! # Results (2026-10-06, snrsi-arch-desktop, release)
//!
//! Recorded in the pins below; the `k` values are printed by the test. The
//! ~~pins are a per-host bit-identity gate: like `stats_move_fingerprints`
//! (GitHub #578), a host with a different libm may produce different bits,
//! in which case add that host's pin rather than loosening the comparison.~~
//! **SUPERSEDED 2026-10-20 (gh:#985):** the platform libm is no longer
//! called, so one pin holds for every host; see below.
//!
//! # Re-pinned 2026-11-11 (latest): OpenMC `scatter()` reaction order and single-cutoff level pick (gh:#1386, #1444 items a, b)
//!
//! `physics::scatter::ReactionOrder::OpenMc` is the CSG collision default: the
//! scattering reactions are walked elastic, MT5, (n,2n), (n,3n), other, levels
//! against one cutoff, and the discrete level reuses that cutoff (no extra
//! draw). Control: `OUTRAM_MC_LEGACY_REACTION_ORDER=1` reproduces every previous
//! pin (3 passed, 2026-11-11). The sphere drivers (`keff_delta_*`) are not wired
//! yet and are unchanged. Cause of the move: OpenMC reaction order (#1386);
//! sigma = old-new over the quadrature of the two standard errors.
//!
//! | case | old k | new k | sigma | old -> new fingerprint |
//! |---|---|---|---|---|
//! | `surface_seq` | 1.29102 +/- 0.04422 | 1.31480 +/- 0.02377 | 0.47 | `0x421af953c2c86352` -> `0x70a3fc7b0653f8ad` |
//! | `hybrid_seq_*` (4 pins, one k) | 1.28224 +/- 0.02684 | 1.22848 +/- 0.02420 | 1.49 | `0x612a86565c772cf0` -> `0x9d6a14eceb527ae2`; `0x98379999342903ee` -> `0x75c6be0e87920854`; `0x8522bc2194983adc` -> `0xd8a66a7d2479eeb4`; `0x59f808f27651cb38` -> `0xb3e8f993cc67b42c` |
//! | `hybrid_par2_tentative` | 1.26183 +/- 0.01395 | 1.27927 +/- 0.01850 | 0.75 | `0x43e85d46f7bdb5d1` -> `0x086fd9d726390639` |
//!
//! None exceeds 3 sigma: re-pinned, not a finding.
//!
//! # Re-pinned 2026-11-11 (later): the OpenMC-faithful collision kernels are the transport default (gh:#1386, #1444, #1456)
//!
//! Maintainer decision 2026-11-11: `physics::scatter::ScatterKernels::OpenMc`
//! (level inelastic, CM-to-lab, `rotate_angle`, `elastic_scatter` with the
//! target-velocity samplers, OpenMC's two-draw reaction ladder) replaces the older
//! kernels in transport; the old ones are `ScatterKernels::Legacy`
//! (`Nuclide::with_legacy_scatter_kernels` or `OUTRAM_MC_LEGACY_SCATTER_KERNELS=1`).
//! Control: with the legacy switch on, every fingerprint below reproduces the
//! previous pin exactly (so only the kernels moved them). Same streams, same
//! distributions, a different last bit and draw order, so `k` moves only by path
//! divergence. Measured on this machine (Intel Xeon, 4 vCPU shared, rustc 1.99.1,
//! release). sigma = old-new difference over the quadrature of the two printed
//! standard errors:
//!
//! | case | old k | new k | sigma |
//! |---|---|---|---|
//! | `surface_seq` | 1.31841 +/- 0.02358 | 1.29102 +/- 0.04422 | 0.55 |
//! | `hybrid_seq_*` (4 pins, one k) | 1.29572 +/- 0.02027 | 1.28224 +/- 0.02684 | 0.40 |
//! | `hybrid_par2_tentative` | 1.29103 +/- 0.02417 | 1.26183 +/- 0.01395 | 1.05 |
//! | `keff_delta_seq` | 1.24412 +/- 0.01350 | 1.24358 +/- 0.03208 | 0.02 |
//! | `keff_delta_par2` | 1.23920 +/- 0.02256 | 1.23575 +/- 0.00783 | 0.14 |
//! | `keff_delta_stepping` | per-generation k (fingerprint only) | | |
//!
//! Fingerprints old -> new: `surface_seq` `0x41960ccb3770903b` -> `0x421af953c2c86352`;
//! `hybrid_seq_no_tally` `0xacc3c32c58981f20` -> `0x612a86565c772cf0`;
//! `hybrid_seq_tentative` `0xe85b3aec28389b6f` -> `0x98379999342903ee`;
//! `hybrid_seq_real` `0xc30aa4bbdf19a23a` -> `0x8522bc2194983adc`;
//! `hybrid_seq_progress` `0x8b4986730a5f5879` -> `0x59f808f27651cb38`;
//! `hybrid_par2_tentative` `0xbb38c221486406cf` -> `0x43e85d46f7bdb5d1`;
//! `keff_delta_seq` `0xaf4c3a642ab7abfa` -> `0xa24251ff7a0c4d8c`;
//! `keff_delta_stepping` `0x72fc9deaaff62ecc` -> `0x2a764f3378416ab5`;
//! `keff_delta_par2` `0x4e48c6737f2d2a7b` -> `0xd05b9af98b9744f5`.
//! None exceeds ~1 sigma: re-pinned, not a finding. Unmoved: `majorants`,
//! `bounded_delta_flight_visiting`, `track_to_collision` (no collision kernel).
//!
//! # Re-pinned 2026-11-11: surface and lattice distances follow OpenMC `8f79d9d0` (gh:#1442, #1443, #701, #1386)
//!
//! The surface `distance` routines (planes, sphere, cylinders, cones, quadric,
//! tori) and `RectLattice::distance` were rewritten as line-by-line ports of
//! OpenMC's `surface.cpp` / `lattice.cpp` and are now bit-identical to a
//! compiled OpenMC (`surface_distance_vs_openmc_code_to_code.rs`,
//! `lattice_distance_vs_openmc_code_to_code.rs`). The old routines differed
//! from upstream in the last bits (e.g. cylinders: `a = u^2 + v^2` against
//! upstream's `a = 1 - u_axis^2`) and in the coincidence band (`1e-14` / `1e-10`
//! against `FP_COINCIDENT = 1e-12`). A re-measurement, not a tuning: no input
//! was changed to reach a number, and the new bits are the upstream-faithful
//! ones. Measured on this machine (Intel Xeon, 4 vCPU shared, rustc 1.99.1,
//! release); the random streams are unchanged, so `k` moves only by the
//! path divergence a last-bit distance change causes. Pins that did NOT move:
//! `keff_delta_seq`, `keff_delta_stepping`, `keff_delta_par2`, `majorants`,
//! `bounded_delta_flight_visiting`, `track_to_collision` (pebble spheres and
//! planes: already bit-identical to upstream).
//!
//! | case | old | new | k (new) |
//! |---|---|---|---|
//! | `surface_seq` | `0xced442c206c401d6` | `0x41960ccb3770903b` | 1.31841 +/- 0.02358 (old k not recorded) |
//! | `hybrid_seq_no_tally` | `0x26fa1f95baf5d18f` | `0xacc3c32c58981f20` | 1.29572 +/- 0.02027 |
//! | `hybrid_seq_tentative` | `0x156e5ccf26d1bdfd` | `0xe85b3aec28389b6f` | 1.29572 +/- 0.02027 |
//! | `hybrid_seq_real` | `0x99da6a61ef5d2918` | `0xc30aa4bbdf19a23a` | 1.29572 +/- 0.02027 |
//! | `hybrid_seq_progress` | `0x795ccb20e19aebe1` | `0x8b4986730a5f5879` | 1.29572 +/- 0.02027 |
//! | `hybrid_par2_tentative` | `0x06b91807b471d659` | `0xbb38c221486406cf` | 1.29103 +/- 0.02417 |
//!
//! The old `k` of these cases was not recorded when they were pinned (the
//! pins hold fingerprints), so the size of the move cannot be stated.

//! # Re-pinned once, 2026-10-20: every transcendental on PETIR (gh:#985)
//!
//! gh:#985 routed every `exp`/`ln`/`powf`/trig call in the workspace through
//! `petir::mathf::RealMath`. That is a ROUTING change, not a physics change.
//! All twelve pins moved; old -> new, measured on host A (i9-13900K, Arch,
//! glibc 2.44), release, on the gh:#985 branch merged with develop
//! `8ad7795d6`:
//!
//! | case | old | new |
//! |---|---|---|
//! | `surface_seq` | `0xed6204ee28305a13` | `0xced442c206c401d6` |
//! | `hybrid_seq_no_tally` | `0xcddf64bd65c085f8` | `0x26fa1f95baf5d18f` |
//! | `hybrid_seq_tentative` | `0xebed4b21c8bb7584` | `0x156e5ccf26d1bdfd` |
//! | `hybrid_seq_real` | `0x642918b8289a2a7e` | `0x99da6a61ef5d2918` |
//! | `hybrid_seq_progress` | `0xae10287ccab10de5` | `0x795ccb20e19aebe1` |
//! | `hybrid_par2_tentative` | `0xb9eec0916b3bc703` | `0x06b91807b471d659` |
//! | `keff_delta_seq` | `0x488141d4cd3e5d19` | `0xaf4c3a642ab7abfa` |
//! | `keff_delta_stepping` | `0x5d5cd38cab27ce55` | `0x72fc9deaaff62ecc` |
//! | `keff_delta_par2` | `0x1cd08047447a42ce` | `0x4e48c6737f2d2a7b` |
//! | `majorants` | `0x04de1a0907930f6a` | `0xfe3bbd1a38537529` |
//! | `bounded_delta_flight_visiting` | `0xc87fe60e12b07a66` | `0xdbb25d2f8227a29b` |
//! | `track_to_collision` | `0xc89edf9971c39ad4` | `0xcdc41b8f380244cf` |
//!
//! New `k` values (single short runs, so their sigmas are large): surface
//! 1.34204 +/- 0.05157; hybrid sequential 1.30424 +/- 0.01652 (1 429 461
//! virtual collisions, the same bits for the no-tally, tentative, real and
//! progress paths, as before); hybrid 2 threads 1.26248 +/- 0.03154;
//! `keff_delta` sequential 1.24412 +/- 0.01350, 2 threads 1.23920 +/- 0.02256.
//!
//! **`majorants` moved too**, although the earlier partial conversion (mc-libs'
//! six functions only, recorded on gh:#985) left it at `0x04de1a0907930f6a`.
//! It has no trig of its own; its cross sections come from the embedded WMP
//! data, whose evaluation in `njoy-outram-park-fork` (Faddeeva, `exp`/`cos`/
//! `sin`) was routed to PETIR later in gh:#985. Attributed, not isolated.
//! **Not yet measured:** a second host. If one gives different bits, the
//! cause is a remaining platform call, not a reason to add a host pin.

use petir::mathf::RealMath;
use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::{
    bounded_delta_flight_visiting, track_to_collision, DeltaStep, Majorant,
};
use outram_mc_libs::pebble_beds::keff_delta::{
    run_keff_delta_par_in, run_keff_delta_seq_in, DeltaDomain, DeltaPowerIteration,
};
use outram_mc_libs::physics::compute::ThreadCount;
use outram_mc_libs::physics::keff::{ComputeType, KeffResult, KeffSettings};
use outram_mc_libs::physics::transport_csg::{
    run_keff_csg, run_keff_csg_hybrid, run_keff_csg_hybrid_with_progress, DeltaTallyEstimator,
    SourceBox,
};
use outram_mc_libs::rng::distributions::isotropic_direction;
use outram_mc_libs::tally::filter::{CellFilter, EnergyFilter, FilterKind, MaterialFilter};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

const TEMP: f64 = 293.6;
const R_OUT: f64 = 5.0;
const R_FUEL: f64 = 2.0;
const R_GRAPHITE: f64 = 3.0;

/// FNV-1a over a stream of `f64` bit patterns.
struct Fp(u64);

impl Fp {
    fn new() -> Self {
        Fp(0xcbf2_9ce4_8422_2325)
    }
    fn u(&mut self, x: u64) {
        for b in x.to_le_bytes() {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn f(&mut self, x: f64) {
        self.u(x.to_bits());
    }
    fn fs(&mut self, xs: &[f64]) {
        for &x in xs {
            self.f(x);
        }
    }
    fn keff(&mut self, r: &KeffResult) {
        self.f(r.k_mean);
        self.f(r.k_std);
        self.fs(&r.k_by_generation);
        self.fs(&r.entropy);
        self.u(r.virtual_collisions);
        self.u(r.collisions);
        self.u(r.lost_locate);
        self.u(r.stuck_events);
        self.u(r.leak_vacuum);
        self.u(r.leak_infinity);
        self.u(r.histories);
    }
    fn tally(&mut self, t: &Tally, n: u64) {
        for b in &t.bins {
            self.f(b.mean(n));
            self.f(b.rel_std_dev(n));
        }
    }
}

/// Print with `DELTA_FP_PRINT=1`; otherwise assert against the pinned value.
fn check(name: &str, got: u64, pinned: u64, detail: &str) {
    if std::env::var("DELTA_FP_PRINT").is_ok() {
        eprintln!("FINGERPRINT {name} = {got:#018x}   ({detail})");
        return;
    }
    assert_eq!(
        got, pinned,
        "{name} moved: {detail}; got {got:#018x}, pinned {pinned:#018x}"
    );
}

fn nuclides() -> Vec<Nuclide> {
    ["U235", "U238", "C0"]
        .iter()
        .map(|n| Nuclide::from_core(n).expect("embedded evaluation"))
        .collect()
}

fn materials() -> Vec<Material> {
    let comp = |nuclide_idx, atom_density| NuclideComponent {
        nuclide_idx,
        atom_density,
    };
    vec![
        Material {
            id: 1,
            name: "fuel".into(),
            temperature: TEMP,
            components: vec![comp(0, 1.0e-3), comp(1, 4.0e-3), comp(2, 5.0e-2)],
        },
        Material {
            id: 2,
            name: "graphite".into(),
            temperature: TEMP,
            components: vec![comp(2, 8.0e-2)],
        },
        Material {
            id: 3,
            name: "dilute gas".into(),
            temperature: TEMP,
            components: vec![comp(2, 1.0e-5)],
        },
    ]
}

/// The zone a point is in, for the `keff_delta` drivers.
fn zone(p: Position) -> Option<usize> {
    let r = p.norm();
    Some(if r < R_FUEL {
        0
    } else if r < R_GRAPHITE {
        1
    } else {
        2
    })
}

/// Reflective sphere holding universe 1's three concentric zones. `delta`
/// decides whether the root cell declares delta tracking; otherwise identical.
fn geometry(delta: bool) -> Geometry {
    let sphere = |r, bc| {
        SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r,
            bc,
        })
    };
    let hs = |surface_idx, sense| RegionToken::HalfSpace { surface_idx, sense };
    let surfaces = vec![
        sphere(R_OUT, BoundaryType::Reflective),
        sphere(R_FUEL, BoundaryType::Transmissive),
        sphere(R_GRAPHITE, BoundaryType::Transmissive),
    ];
    let cells = vec![
        Cell::material(10, vec![hs(1, HalfSpaceSense::Inside)], 0, TEMP),
        Cell::material(
            11,
            vec![
                hs(1, HalfSpaceSense::Outside),
                hs(2, HalfSpaceSense::Inside),
                RegionToken::Intersection,
            ],
            1,
            TEMP,
        ),
        Cell::material(12, vec![hs(2, HalfSpaceSense::Outside)], 2, TEMP),
        {
            let root = Cell::fill(
                1,
                vec![hs(0, HalfSpaceSense::Inside)],
                CellFill::Universe(1),
                Position::ZERO,
            );
            if delta {
                root.delta_tracked(0)
            } else {
                root
            }
        },
    ];
    Geometry {
        surfaces,
        cells,
        universes: vec![
            Universe {
                id: 0,
                cell_indices: vec![3],
            },
            Universe {
                id: 1,
                cell_indices: vec![0, 1, 2],
            },
        ],
        lattices: vec![],
        root_universe: 0,
    }
}

fn grid() -> Vec<f64> {
    let (lo, hi) = (1.0e-5_f64, 2.0e7_f64);
    (0..512)
        .map(|i| (lo.r_ln() + (hi.r_ln() - lo.r_ln()) * i as f64 / 511.0).r_exp())
        .collect()
}

fn majorant(mats: &[Material], nucs: &[Nuclide]) -> Majorant {
    Majorant::over_indices(mats, &[0, 1, 2], nucs, &grid(), 0.1)
}

fn settings(compute: ComputeType, est: DeltaTallyEstimator) -> KeffSettings {
    KeffSettings {
        n_particles: 400,
        n_inactive: 3,
        n_active: 5,
        temperature_k: TEMP,
        seed: 20_261_006,
        compute,
        delta_tally_estimator: est,
        ..KeffSettings::default()
    }
}

fn source() -> SourceBox {
    SourceBox {
        lower: Position::new(-R_OUT, -R_OUT, -R_OUT),
        upper: Position::new(R_OUT, R_OUT, R_OUT),
    }
}

/// Flux and total by cell (the three leaf cells) × material × 2 groups.
fn tally() -> Tally {
    let n = 3 * 3 * 2 * 2;
    Tally {
        id: 599,
        name: "zones".into(),
        filters: vec![
            FilterKind::Cell(CellFilter {
                cell_indices: vec![0, 1, 2],
            }),
            FilterKind::Material(MaterialFilter {
                material_indices: vec![0, 1, 2],
            }),
            FilterKind::Energy(EnergyFilter {
                bins: vec![1.0e-5, 0.625, 2.0e7],
            }),
        ],
        scores: vec![ScoreType::Flux, ScoreType::Total],
        bins: vec![TallyBin::default(); n],
    }
}

fn describe(r: &KeffResult) -> String {
    format!(
        "k = {:.17} +/- {:.17}, virtual {}",
        r.k_mean, r.k_std, r.virtual_collisions
    )
}

#[test]
fn hybrid_csg_paths_are_bit_identical() {
    let nucs = nuclides();
    let mats = materials();
    let maj = majorant(&mats, &nucs);
    let geom = geometry(true);
    let seq = ComputeType::CpuSingleThread;
    let par = ComputeType::CpuMultiThread(ThreadCount::Fixed(2));
    let n_active = 5_u64;

    // Surface tracking on the identical model: the unbiased reference.
    let surf = run_keff_csg(
        &geometry(false),
        &mats,
        &nucs,
        source(),
        &settings(seq, DeltaTallyEstimator::default()),
        None,
    );
    let mut fp = Fp::new();
    fp.keff(&surf);
    check("surface_seq", fp.0, 0x70a3fc7b0653f8ad, &describe(&surf));

    let a = run_keff_csg_hybrid(
        &geom,
        &mats,
        &nucs,
        std::slice::from_ref(&maj),
        None,
        source(),
        &settings(seq, DeltaTallyEstimator::default()),
        None,
    );
    let mut fp = Fp::new();
    fp.keff(&a);
    check("hybrid_seq_no_tally", fp.0, 0x9d6a14eceb527ae2, &describe(&a));

    for (name, est, pin) in [
        (
            "hybrid_seq_tentative",
            DeltaTallyEstimator::TentativeCollision,
            0x75c6be0e87920854_u64,
        ),
        (
            "hybrid_seq_real",
            DeltaTallyEstimator::RealCollision,
            0xd8a66a7d2479eeb4,
        ),
    ] {
        let mut t = tally();
        let r = run_keff_csg_hybrid(
            &geom,
            &mats,
            &nucs,
            std::slice::from_ref(&maj),
            None,
            source(),
            &settings(seq, est),
            Some(&mut t),
        );
        let mut fp = Fp::new();
        fp.keff(&r);
        fp.tally(&t, n_active);
        check(name, fp.0, pin, &describe(&r));
    }

    let mut ks = Vec::new();
    let r = run_keff_csg_hybrid_with_progress(
        &geom,
        &mats,
        &nucs,
        std::slice::from_ref(&maj),
        None,
        source(),
        &settings(seq, DeltaTallyEstimator::default()),
        None,
        |g| ks.push(g.k),
    );
    let mut fp = Fp::new();
    fp.keff(&r);
    fp.fs(&ks);
    check("hybrid_seq_progress", fp.0, 0xb3e8f993cc67b42c, &describe(&r));

    let mut t = tally();
    let r = run_keff_csg_hybrid(
        &geom,
        &mats,
        &nucs,
        std::slice::from_ref(&maj),
        None,
        source(),
        &settings(par, DeltaTallyEstimator::default()),
        Some(&mut t),
    );
    let mut fp = Fp::new();
    fp.keff(&r);
    fp.tally(&t, n_active);
    check("hybrid_par2_tentative", fp.0, 0x086fd9d726390639, &describe(&r));
}

#[test]
fn keff_delta_paths_are_bit_identical() {
    let nucs = nuclides();
    let mats = materials();
    let maj = majorant(&mats, &nucs);
    let domain = DeltaDomain::Sphere { radius: R_OUT };
    let seq = settings(ComputeType::CpuSingleThread, DeltaTallyEstimator::default());

    let r = run_keff_delta_seq_in(domain, &mats, &nucs, &maj, zone, &seq);
    let mut fp = Fp::new();
    fp.keff(&r);
    check("keff_delta_seq", fp.0, 0xa24251ff7a0c4d8c, &describe(&r));

    let mut it = DeltaPowerIteration::new(domain, &mats, &nucs, &zone, &seq);
    let mut fp = Fp::new();
    while let Some(g) = it.step(&mats, &nucs, &maj, &zone) {
        fp.f(g.k);
        if let Some((m, s)) = g.k_mean {
            fp.f(m);
            fp.f(s);
        }
    }
    check("keff_delta_stepping", fp.0, 0x2a764f3378416ab5, "per-generation k");

    let r = run_keff_delta_par_in(domain, &mats, &nucs, &maj, zone, &seq, ThreadCount::Fixed(2));
    let mut fp = Fp::new();
    fp.keff(&r);
    check("keff_delta_par2", fp.0, 0xd05b9af98b9744f5, &describe(&r));
}

#[test]
fn delta_primitives_are_bit_identical() {
    let nucs = nuclides();
    let mats = materials();
    let maj = majorant(&mats, &nucs);
    let bounding = Majorant::bounding(&mats, &nucs, 1.0e-5, 2.0e7, 256, 8, 0.05);

    let mut fp = Fp::new();
    fp.u(maj.len() as u64);
    fp.u(bounding.len() as u64);
    for i in 0..2000 {
        let e = 1.0e-7 * (3.0e7_f64 / 1.0e-7).r_powf(i as f64 / 1999.0);
        fp.f(maj.at(e));
        fp.f(bounding.at(e));
    }
    check("majorants", fp.0, 0xfe3bbd1a38537529, "Majorant::at on 2000 energies");

    // Flights from random points in random directions at random energies,
    // through the three zones, bounded by the reflective sphere's radius.
    let mut seed = 20_261_006_u64;
    let mut fp = Fp::new();
    let mut fp_track = Fp::new();
    for _ in 0..2000 {
        let r0 = Position::new(
            -1.5 + 3.0 * outram_mc_libs::rng::lcg::prn(&mut seed),
            -1.5 + 3.0 * outram_mc_libs::rng::lcg::prn(&mut seed),
            -1.5 + 3.0 * outram_mc_libs::rng::lcg::prn(&mut seed),
        );
        let (dx, dy, dz) = isotropic_direction(&mut seed);
        let u = Direction::new(dx, dy, dz);
        let e = 1.0e-3 * (1.0e10_f64).r_powf(outram_mc_libs::rng::lcg::prn(&mut seed));
        let exit = |p: Position, d: Direction| {
            let b = p.x * d.u + p.y * d.v + p.z * d.w;
            let c = p.norm_sqr() - R_OUT * R_OUT;
            -b + (b * b - c).max(0.0).sqrt()
        };
        let mut visits = Vec::new();
        let urr_seed = seed ^ 0x5eed;
        let step = bounded_delta_flight_visiting(
            r0,
            u,
            e,
            &maj,
            &mats,
            &nucs,
            100_000,
            exit,
            zone,
            &mut seed,
            Some(urr_seed),
            |p, m, s| visits.push((p, m, s)),
        );
        for (p, m, s) in visits {
            fp.fs(&[p.x, p.y, p.z, s]);
            fp.u(m as u64);
        }
        match step {
            DeltaStep::Collision {
                position,
                material,
                virtual_collisions,
                ..
            } => {
                fp.u(1);
                fp.fs(&[position.x, position.y, position.z]);
                fp.u(material as u64);
                fp.u(u64::from(virtual_collisions));
            }
            DeltaStep::Exit {
                position,
                virtual_collisions,
                ..
            } => {
                fp.u(2);
                fp.fs(&[position.x, position.y, position.z]);
                fp.u(u64::from(virtual_collisions));
            }
            DeltaStep::Exhausted { virtual_collisions } => {
                fp.u(3);
                fp.u(u64::from(virtual_collisions));
            }
        }
        let flight = track_to_collision(r0, u, e, &maj, 100_000, &mut seed, |p| {
            (p.norm() < R_OUT)
                .then(|| zone(p).map(|m| mats[m].macro_xs_total(e, &nucs)))
                .flatten()
        });
        fp_track.fs(&[
            flight.position.x,
            flight.position.y,
            flight.position.z,
            flight.distance,
        ]);
        fp_track.u(u64::from(flight.virtual_collisions));
        fp_track.u(u64::from(flight.escaped));
    }
    check("bounded_delta_flight_visiting", fp.0, 0xdbb25d2f8227a29b, "2000 flights");
    check("track_to_collision", fp_track.0, 0xcdc41b8f380244cf, "2000 flights");
}
