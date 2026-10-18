//! **End-to-end: a tallied reaction rate in the URR agrees with an independent
//! estimator of the same rate in the same run (GitHub #953 follow-up).**
//!
//! # Methodology
//!
//! `urr_tally_scoring_953.rs` checks the scoring function against the flight
//! and collision at one energy. This file asks the question that matters: on a
//! case with real unresolved-resonance flux, does the *tallied rate* equal what
//! the transport actually did?
//!
//! Fixture (all ENDF/B-VIII.0 at 293.6 K, URR probability tables on with the
//! cheap PURR settings of the sibling test; table values are not asserted):
//!
//! 1. `fixed_source_*`: a bare U-238 metal sphere (0.0482 /b.cm, 15 cm,
//!    vacuum), isotropic point source at 140 keV inside the 20-149 keV URR,
//!    analog. Everything absorbed between 25 and 145 keV is U-238 capture
//!    scored with a URR band. Per batch of independent histories (own seed) the
//!    same histories give (a) the **track-length** tally `Absorption` in the
//!    window, and (c) the **analog** count of capture (and fission) events
//!    whose energy lies in the window, read off the recorded tracks. They
//!    estimate the same rate, "absorptions in the window per source neutron".
//!    Compared per batch (paired, same histories) and as unpaired means.
//! 2. `delta_*`: a reflective 20 cm UO2 cube (no moderator) in a
//!    k-eigenvalue run. The flux-weighted mean `Sigma_a` in the window
//!    (`R = absorption / flux`) from surface-tracking track length, from the
//!    delta-tracking tentative-collision estimator and from the delta-tracking
//!    real-collision estimator (the last two on the same histories). The
//!    transport exposes no analog count of captures by energy in a
//!    k-eigenvalue run, so this test is the collision-estimator leg only.
//!
//! Pass criteria: (1) |track-length - analog| below 4 sigma, paired and
//! unpaired; (2) the three `R` agree pairwise within 4 sigma.
//!
//! **What each check can and cannot see.** Before #953 all three estimators
//! scored the same dilute function, so (2) passes on the old code too (see the
//! results); it checks that the estimators are mutually consistent, not that
//! they follow the transport. (1) is the discriminating check: the analog
//! count is the transport itself.
//!
//! # Prediction, written before the first run (2026-10-19, also on gh #953)
//!
//! Before #953 the track-length estimator scored the dilute `Sigma_a` while the
//! flight used the band `Sigma_t`, so at one energy the old score divided by
//! the true one is `Sa_dil * E[1/St_b] / E[Sa_b/St_b]` over bands `b`
//! (`band_to_dilute_ratio_across_the_window` prints it). A first guess of
//! +1.3 % (from the sibling test's band-mean ratio) was **revised before the
//! run**: that 0.98658 was 400-seed sampling noise (the band mean equals the
//! dilute value to 5 digits), and the flight-weighted ratio is 1.02 to 1.11
//! across the window (mean 1.048). Prediction: pre-#953 track length reads
//! HIGH by +3 to +8 % against the analog count; the current code agrees within
//! statistics.
//!
//! # Results (2026-10-19, release, i9-13900K, 4 of 16 logical cores pinned,
//! 64 GB, Linux 7.2.9, CPU only; test 1 about 60 s of which ~50 s is PURR
//! table construction, test 2 about 160 s)
//!
//! Current code: track length 0.42633 +- 0.00066, analog 0.42606 +- 0.00112
//! absorptions per source neutron in 25-145 keV (ratio 1.00064, paired
//! +0.19 sigma); 93.9 % of all absorptions fall in the window. Pre-#953 code
//! (parent commit, scratch worktree): track length 0.44585 +- 0.00071, same
//! analog count (bit-identical histories), ratio 1.0464, +13.3 sigma paired.
//! Test 2: see the README. Full record:
//! `verification_and_validation/urr_tally_scoring_953/README.md`.

use std::path::PathBuf;

use njoy_outram_park_fork::endf::tape::Tape;
use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::fixed_source::{run_fixed_source_traced, FixedSource, FixedSourceSettings};
use outram_mc_libs::physics::track_output::{TrackEvent, TrackRecorder};
use outram_mc_libs::tally::filter::{EnergyFilter, FilterKind};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

const TEMP: f64 = 293.6;
/// The energy window the rates are restricted to \[eV\]: inside U-238's
/// 20-149 keV unresolved range, clear of both ends.
const WINDOW: [f64; 2] = [2.5e4, 1.45e5];

fn endf_path(file: &str) -> Option<PathBuf> {
    njoy_outram_park_fork::reference_data::reference_endf(file)
}

/// A nuclide with cheap probability tables (same settings as
/// `urr_tally_scoring_953.rs`); `None` when the tape is not in the checkout.
fn nuclide_with_tables(name: &str, file: &str, tables: bool) -> Option<Nuclide> {
    let p = endf_path(file)?;
    let t = uom::si::f64::ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(
        TEMP,
    );
    let nuc = Nuclide::from_endf_file(&p, name, t, 1.0e-3).expect("reconstructs");
    let nuc = nuc.without_urr_probability_tables();
    if !tables {
        return Some(nuc);
    }
    let tape = Tape::read_file(&p).expect("tape parses");
    let mat = tape.materials().first().copied().expect("one material");
    Some(
        nuc.with_urr_probability_tables(&tape, mat, TEMP, 20, 4, 500)
            .expect("tables"),
    )
}

fn u238_sphere(r_cm: f64) -> Geometry {
    Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: r_cm,
            bc: BoundaryType::Vacuum,
        })],
        cells: vec![Cell::material(
            1,
            vec![RegionToken::HalfSpace {
                surface_idx: 0,
                sense: HalfSpaceSense::Inside,
            }],
            0,
            TEMP,
        )],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

fn mean_se(x: &[f64]) -> (f64, f64) {
    let n = x.len() as f64;
    let m = x.iter().sum::<f64>() / n;
    let v = x.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (n - 1.0);
    (m, (v / n).sqrt())
}

/// At energy `e`: `(old/true score, dilute/band-mean)` for `Sigma_a`.
///
/// The old track-length score was `Sa_dil * d` with `d` flown on the band
/// total, `E[d | band] = 1/St_b`; the true rate is `E[Sa_b / St_b]`.
fn old_over_true(nuc: &Nuclide, e: f64) -> (f64, f64) {
    let n = 20_000;
    let dil = nuc.xs_at_energy(e, TEMP).absorption;
    let (mut inv_st, mut sa_over_st, mut sa) = (0.0, 0.0, 0.0);
    for k in 0..n {
        let xi = (k as f64 + 0.5) / n as f64;
        let b = nuc.xs_at_energy_urr(e, TEMP, xi);
        inv_st += 1.0 / b.total;
        sa_over_st += b.absorption / b.total;
        sa += b.absorption;
    }
    let (inv_st, sa_over_st, sa) = (inv_st / n as f64, sa_over_st / n as f64, sa / n as f64);
    (dil * inv_st / sa_over_st, dil / sa)
}

/// The size of the pre-#953 bias, computed from the tables alone.
#[test]
fn band_to_dilute_ratio_across_the_window() {
    let Some(u238) = nuclide_with_tables("U238", "n-092_U_238.endf", true) else {
        eprintln!("SKIP: ENDF tapes not in this checkout");
        return;
    };
    let mut ratios = vec![];
    for e in [3.0e4, 5.0e4, 7.0e4, 1.0e5, 1.3e5, 1.4e5] {
        assert!(u238.needs_urr_draw(e));
        let (flight, plain) = old_over_true(&u238, e);
        println!(
            "U-238 {e:>9.3e} eV: old/true (flight-weighted) {flight:.5}, dilute/band-mean {plain:.5}"
        );
        // The pre-#953 score was high by the flight weighting at every energy.
        assert!(
            flight > 1.01,
            "no flight-weighting bias at {e} eV: {flight}"
        );
        ratios.push(flight);
    }
    println!(
        "mean old/true over the six energies: {:.5}",
        ratios.iter().sum::<f64>() / 6.0
    );
}

struct Batch {
    track_length: f64,
    analog: f64,
    analog_all: f64,
}

fn fixed_source_batches(nuclides: &[Nuclide], n_batches: usize, per_batch: usize) -> Vec<Batch> {
    let mats = vec![Material {
        id: 1,
        name: "U238".into(),
        components: vec![NuclideComponent {
            nuclide_idx: 0,
            atom_density: 0.0482,
        }],
        temperature: TEMP,
    }];
    let geom = u238_sphere(15.0);
    let source = FixedSource::Point {
        r: Position::ZERO,
        energy_ev: 1.4e5,
    };
    (0..n_batches)
        .map(|b| {
            let mut tally = Tally {
                id: 953,
                name: "window".into(),
                filters: vec![FilterKind::Energy(EnergyFilter {
                    bins: WINDOW.to_vec(),
                })],
                scores: vec![ScoreType::Absorption],
                bins: vec![TallyBin::default(); 1],
            };
            let mut rec = TrackRecorder::new(4 * per_batch, 100_000);
            let settings = FixedSourceSettings {
                n_particles: per_batch,
                n_batches: 1,
                seed: 0x9E37_79B9_7F4A_7C15u64.wrapping_mul(b as u64 + 1) | 1,
                ..FixedSourceSettings::default()
            };
            let res = run_fixed_source_traced(
                &geom,
                &mats,
                nuclides,
                &source,
                &settings,
                Some(&mut tally),
                Some(&mut rec),
                None,
                None,
            );
            // Every history (source neutron or fission secondary) is recorded;
            // both estimators below count all of them per SOURCE neutron.
            assert_eq!(rec.dropped_tracks, 0);
            assert_eq!(rec.tracks.len(), res.total_histories);
            let (mut inw, mut all) = (0u64, 0u64);
            for t in &rec.tracks {
                assert_eq!(t.dropped_states, 0);
                for s in &t.states {
                    if matches!(s.event, TrackEvent::Absorption | TrackEvent::Fission) {
                        all += 1;
                        if s.energy >= WINDOW[0] && s.energy < WINDOW[1] {
                            inw += 1;
                        }
                    }
                }
            }
            let n = per_batch as f64;
            Batch {
                track_length: tally.bins[0].sum / n,
                analog: inw as f64 / n,
                analog_all: all as f64 / n,
            }
        })
        .collect()
}

/// **(a) track length against (c) analog capture, same histories.**
#[test]
fn fixed_source_track_length_equals_analog_capture_in_the_urr() {
    let Some(u238) = nuclide_with_tables("U238", "n-092_U_238.endf", true) else {
        eprintln!("SKIP: ENDF tapes not in this checkout");
        return;
    };
    assert!(u238.needs_urr_draw(WINDOW[0]) && u238.needs_urr_draw(WINDOW[1] - 1.0));
    let (n_batches, per_batch) = (40, 5000);
    let t0 = std::time::Instant::now();
    let b = fixed_source_batches(&[u238], n_batches, per_batch);
    let tl: Vec<f64> = b.iter().map(|x| x.track_length).collect();
    let an: Vec<f64> = b.iter().map(|x| x.analog).collect();
    let diff: Vec<f64> = b.iter().map(|x| x.track_length - x.analog).collect();
    let all: Vec<f64> = b.iter().map(|x| x.analog_all).collect();
    let (mt, st) = mean_se(&tl);
    let (ma, sa) = mean_se(&an);
    let (md, sd) = mean_se(&diff);
    let (mall, _) = mean_se(&all);
    let z_paired = md / sd;
    let z_unpaired = (mt - ma) / (st * st + sa * sa).sqrt();
    println!(
        "fixed source, {n_batches} x {per_batch} histories ({:.0} s):\n  \
         window absorptions per source neutron: track-length {mt:.5} +- {st:.5}, analog {ma:.5} +- {sa:.5}\n  \
         track-length / analog = {:.5}; difference {md:+.5} +- {sd:.5} (paired {z_paired:+.2} sigma, unpaired {z_unpaired:+.2} sigma)\n  \
         fraction of ALL absorptions inside the window: {:.4}",
        t0.elapsed().as_secs_f64(),
        mt / ma,
        ma / mall,
    );
    assert!(z_paired.abs() < 4.0 && z_unpaired.abs() < 4.0);
}

// ── 2. Collision estimators under delta tracking ─────────────────────────────

use outram_mc_libs::geometry::position::Position as P3;
use outram_mc_libs::geometry::surface::{XPlane, YPlane, ZPlane};
use outram_mc_libs::physics::delta_tracking::Majorant;
use outram_mc_libs::physics::keff::{ComputeType, KeffSettings};
use outram_mc_libs::physics::transport_csg::{
    run_keff_csg, run_keff_csg_hybrid, DeltaTallyEstimator, SourceBox,
};

/// A reflective cube of side `a` holding material 0; `delta` delta-tracks it.
fn uo2_cube(a: f64, delta: bool) -> Geometry {
    let h = 0.5 * a;
    let bc = BoundaryType::Reflective;
    let surfaces = vec![
        SurfaceKind::XPlane(XPlane { x0: -h, bc }),
        SurfaceKind::XPlane(XPlane { x0: h, bc }),
        SurfaceKind::YPlane(YPlane { y0: -h, bc }),
        SurfaceKind::YPlane(YPlane { y0: h, bc }),
        SurfaceKind::ZPlane(ZPlane { z0: -h, bc }),
        SurfaceKind::ZPlane(ZPlane { z0: h, bc }),
    ];
    let hs = |i: usize, sense| RegionToken::HalfSpace {
        surface_idx: i,
        sense,
    };
    let mut region = vec![hs(0, HalfSpaceSense::Outside)];
    for (i, s) in [
        (1, HalfSpaceSense::Inside),
        (2, HalfSpaceSense::Outside),
        (3, HalfSpaceSense::Inside),
        (4, HalfSpaceSense::Outside),
        (5, HalfSpaceSense::Inside),
    ] {
        region.push(hs(i, s));
        region.push(RegionToken::Intersection);
    }
    let cell = Cell::material(1, region, 0, TEMP);
    Geometry {
        surfaces,
        cells: vec![if delta { cell.delta_tracked(0) } else { cell }],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

fn window_tally() -> Tally {
    Tally {
        id: 953,
        name: "window".into(),
        filters: vec![FilterKind::Energy(EnergyFilter {
            bins: WINDOW.to_vec(),
        })],
        scores: vec![ScoreType::Flux, ScoreType::Absorption],
        bins: vec![TallyBin::default(); 2],
    }
}

/// `(R, relative sigma)` with `R = absorption / flux` in the window (the
/// flux-weighted mean `Sigma_a`), sigmas of the two bins added in quadrature
/// (correlation ignored: conservative).
fn window_ratio(t: &Tally, n: u64) -> (f64, f64) {
    let (f, a) = (&t.bins[0], &t.bins[1]);
    let (rf, ra) = (f.rel_std_dev(n), a.rel_std_dev(n));
    (a.mean(n) / f.mean(n), (rf * rf + ra * ra).sqrt())
}

/// **(a) surface track length, (b) tentative-collision and real-collision
/// estimators, on a UO2 cube with real URR flux.**
///
/// Methodology: reflective cube of side 20 cm, UO2 at 10.4 g/cc, 5 % U-235
/// by atoms of U (N = 1.16e-3 U-235, 2.21e-2 U-238, 4.65e-2 O-16 /b.cm), no
/// moderator, so fission neutrons slow through the U-238 URR by inelastic and
/// elastic scattering. ENDF/B-VIII.0 at 293.6 K, URR tables on U-235 and U-238
/// (cheap PURR settings). k-eigenvalue, 2000 x (20 + 30), seed 1, single
/// thread. Tally: `Flux` and `Absorption` in 25-145 keV. The quantity compared
/// is `R = absorption / flux`, the flux-weighted mean `Sigma_a`, which needs
/// no source normalisation. Arms: surface tracking (track length); delta
/// tracking with the default tentative-collision estimator; delta tracking
/// with the real-collision estimator (same histories as the tentative arm, so
/// bit-identical `k`). Pass: all three `R` within 4 sigma pairwise (sigma of
/// the ratio from the two bins in quadrature, conservative), no majorant
/// violation, `k` of the two delta arms bit-identical.
#[test]
fn delta_collision_estimators_agree_with_track_length_in_the_urr() {
    let (Some(u235), Some(u238), Some(o16)) = (
        nuclide_with_tables("U235", "n-092_U_235-ENDF8.0.endf", true),
        nuclide_with_tables("U238", "n-092_U_238.endf", true),
        nuclide_with_tables("O16", "n-008_O_016-ENDF8.0.endf", false),
    ) else {
        eprintln!("SKIP: ENDF tapes not in this checkout");
        return;
    };
    let nucs = vec![u235, u238, o16];
    let mats = vec![Material {
        id: 1,
        name: "UO2".into(),
        temperature: TEMP,
        components: vec![
            NuclideComponent {
                nuclide_idx: 0,
                atom_density: 1.16e-3,
            },
            NuclideComponent {
                nuclide_idx: 1,
                atom_density: 2.21e-2,
            },
            NuclideComponent {
                nuclide_idx: 2,
                atom_density: 4.65e-2,
            },
        ],
    }];
    let a = 20.0;
    let grid: Vec<f64> = (0..4096)
        .map(|i| (1.0e-5_f64.ln() + (2.0e7_f64.ln() - 1.0e-5_f64.ln()) * i as f64 / 4095.0).exp())
        .collect();
    let maj = Majorant::over_indices(&mats, &[0], &nucs, &grid, 0.3);
    let h = 0.5 * a;
    let source = SourceBox {
        lower: P3::new(-h, -h, -h),
        upper: P3::new(h, h, h),
    };
    let settings = |est| KeffSettings {
        n_particles: 2000,
        n_inactive: 20,
        n_active: 30,
        temperature_k: TEMP,
        seed: 1,
        compute: ComputeType::CpuSingleThread,
        delta_tally_estimator: est,
        ..KeffSettings::default()
    };
    let n = 30_u64;
    let t0 = std::time::Instant::now();

    let mut t_surf = window_tally();
    let surf = run_keff_csg(
        &uo2_cube(a, false),
        &mats,
        &nucs,
        source,
        &settings(DeltaTallyEstimator::default()),
        Some(&mut t_surf),
    );
    let mut t_tent = window_tally();
    let tent = run_keff_csg_hybrid(
        &uo2_cube(a, true),
        &mats,
        &nucs,
        std::slice::from_ref(&maj),
        None,
        source,
        &settings(DeltaTallyEstimator::TentativeCollision),
        Some(&mut t_tent),
    );
    let mut t_real = window_tally();
    let real = run_keff_csg_hybrid(
        &uo2_cube(a, true),
        &mats,
        &nucs,
        std::slice::from_ref(&maj),
        None,
        source,
        &settings(DeltaTallyEstimator::RealCollision),
        Some(&mut t_real),
    );
    let (rs, rt, rr) = (
        window_ratio(&t_surf, n),
        window_ratio(&t_tent, n),
        window_ratio(&t_real, n),
    );
    println!(
        "k: surface {:.5} +- {:.5}, delta {:.5} +- {:.5}; majorant violations {}, virtual collisions {} ({:.0} s)",
        surf.k_mean,
        surf.k_std,
        tent.k_mean,
        tent.k_std,
        tent.majorant_violations,
        tent.virtual_collisions,
        t0.elapsed().as_secs_f64()
    );
    let z = |x: (f64, f64), y: (f64, f64)| {
        (x.0 - y.0).abs() / ((x.0 * x.1).powi(2) + (y.0 * y.1).powi(2)).sqrt()
    };
    println!(
        "window <Sigma_a>_phi [1/cm]: track length {:.6e} +- {:.2e} | tentative {:.6e} +- {:.2e} ({:+.2} %, {:.2} sigma) | real-collision {:.6e} +- {:.2e} ({:+.2} %, {:.2} sigma)",
        rs.0,
        rs.0 * rs.1,
        rt.0,
        rt.0 * rt.1,
        100.0 * (rt.0 / rs.0 - 1.0),
        z(rt, rs),
        rr.0,
        rr.0 * rr.1,
        100.0 * (rr.0 / rs.0 - 1.0),
        z(rr, rs),
    );
    println!(
        "real-collision vs tentative: {:+.2} %, {:.2} sigma",
        100.0 * (rr.0 / rt.0 - 1.0),
        z(rr, rt)
    );
    assert_eq!(
        tent.majorant_violations, 0,
        "the majorant failed to bound the URR bands"
    );
    assert_eq!(
        tent.k_mean.to_bits(),
        real.k_mean.to_bits(),
        "the estimator must not change the histories"
    );
    assert!(z(rt, rs) < 4.0 && z(rr, rs) < 4.0 && z(rr, rt) < 4.0);
}
