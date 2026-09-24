// SPDX-License-Identifier: GPL-3.0

//! **The particle clock** — gh:#262 and gh:#261.
//!
//! # What was missing
//!
//! The transport loop had **no clock at all**. `FilterEvent::time` and
//! `TrackState::time` both existed, were both documented as "time since the
//! particle was born", and were both hardcoded **`0.0`**. So a `TimeFilter`
//! compiled, ran, and binned every event into whichever bin contains zero —
//! measuring nothing while looking like it worked — and every state in a track
//! file reported `t = 0`.
//!
//! `scoring.rs` said so in its own comment rather than leaving it to be
//! discovered, which is why this is a gap being closed rather than a bug.
//!
//! It also blocks IFP's generation time `Lambda` (#262 scope item 4): the
//! lineage records each ancestor's lifetime, and with no clock there is no
//! lifetime to record. **This does not by itself make IFP work** — `beta_eff`
//! additionally needs per-ancestor delayed groups, which nothing in this crate
//! samples (the reason `DelayedGroupFilter::new` refuses). One of two
//! prerequisites.
//!
//! # Results (2026-09-24)
//!
//! Printed by the tests. `v(1 MeV) = 1.383159e9 cm/s`, so a neutron born at the
//! centre of a 10 cm void ball reaches the boundary at **7.229825 ns** — and the
//! clock reports that to **0.000e0 relative**, i.e. exactly, because `d / v` is
//! computed the same way on both sides and there is no other term. A traced and
//! an untraced run agree **bit for bit** (`1.989552157226e4`), so the clock
//! perturbs nothing.
//!
//! (An earlier draft of this comment said "7.2327 ns ... 6.7e-16 relative".
//! Both were written before the test ran and neither was the measured value.)

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::physics::fixed_source::{
    run_fixed_source, run_fixed_source_traced, FixedSource, FixedSourceSettings,
};
use outram_mc_libs::physics::track_output::TrackRecorder;
use outram_mc_libs::tally::filter::{CellFilter, FilterKind};
use outram_mc_libs::tally::scoring::neutron_speed_cm_per_s;
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

const R: f64 = 10.0;
const E0: f64 = 1.0e6;

/// A void ball of radius `R` with a vacuum boundary: a source neutron at the
/// centre streams exactly `R` cm and leaks. No collisions, so the flight time
/// is analytic and the clock has nowhere to hide.
fn void_ball() -> Geometry {
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

fn settings(n: usize) -> FixedSourceSettings {
    FixedSourceSettings {
        n_particles: n,
        n_batches: 5,
        seed: 20_260_924,
        ..Default::default()
    }
}

/// **The clock is analytically right.** A neutron born at the centre of a void
/// ball reaches the boundary after exactly `R / v(E)` seconds.
#[test]
fn the_clock_matches_the_analytic_flight_time() {
    let geom = void_ball();
    let src = FixedSource::Point {
        r: Position::ZERO,
        energy_ev: E0,
    };
    let mut recorder = TrackRecorder::new(4, 64);
    run_fixed_source_traced(
        &geom,
        &[],
        &[],
        &src,
        &settings(5),
        None,
        Some(&mut recorder),
        None,
        None,
    );

    let v = neutron_speed_cm_per_s(E0);
    let expect = R / v;
    println!(
        "v({E0:.0e} eV) = {v:.6e} cm/s; expected flight over {R} cm = {:.6} ns",
        expect * 1.0e9
    );

    let tracks = &recorder.tracks;
    assert!(!tracks.is_empty(), "no tracks captured");
    let mut checked = 0usize;
    for track in tracks {
        // Birth must be at t = 0; there is nowhere else for a source particle
        // to start.
        let first = track.states.first().expect("a born state");
        assert_eq!(
            first.time, 0.0,
            "a source particle must be born at t = 0, got {}",
            first.time
        );
        // The last state is at or beyond the boundary; its clock must be the
        // analytic flight time for the distance actually travelled.
        let last = track.states.last().expect("a last state");
        if last.time <= 0.0 {
            continue; // a track that never moved
        }
        let d = (last.r.x * last.r.x + last.r.y * last.r.y + last.r.z * last.r.z).sqrt();
        let want = d / v;
        let rel = (last.time - want).abs() / want;
        println!(
            "  travelled {d:.6} cm, clock {:.6} ns, analytic {:.6} ns, rel {rel:.3e}",
            last.time * 1.0e9,
            want * 1.0e9
        );
        assert!(
            rel < 1.0e-12,
            "clock {} s against analytic {want} s over {d} cm at {E0} eV: {rel:.3e} \
             relative. The clock is d/v by construction, so anything above \
             round-off means the distance or the energy used for the speed is \
             not the one actually flown.",
            last.time
        );
        checked += 1;
    }
    assert!(checked > 0, "no track advanced the clock, so nothing was checked");
    println!("checked {checked} tracks against d/v");
}

/// **Adding the clock must not move any answer.** It draws no random numbers
/// and feeds no decision, so every tallied result must be bit-identical to a
/// run without it — the same property `track_capture_does_not_perturb_the_run`
/// pins for track capture, and for the same reason: an instrument that changes
/// the thing being measured is worse than none.
#[test]
fn the_clock_does_not_perturb_the_run() {
    let geom = void_ball();
    let src = FixedSource::Point {
        r: Position::new(1.0, 0.0, 0.0),
        energy_ev: E0,
    };

    // The clock runs unconditionally, so "without it" cannot be built by a
    // flag. What CAN be checked is that two runs of the same inputs agree bit
    // for bit whether or not tracks are captured — capture is what reads the
    // clock, so if the clock's accumulation perturbed the stream this would
    // diverge.
    let mut plain = flux_tally();
    run_fixed_source(&geom, &[], &[], &src, &settings(2000), Some(&mut plain));

    let mut traced = flux_tally();
    let mut recorder = TrackRecorder::new(8, 64);
    run_fixed_source_traced(
        &geom,
        &[],
        &[],
        &src,
        &settings(2000),
        Some(&mut traced),
        Some(&mut recorder),
        None,
        None,
    );

    assert_eq!(
        plain.bins[0].sum.to_bits(),
        traced.bins[0].sum.to_bits(),
        "the tallied flux differs between a traced and an untraced run: {} vs {}",
        plain.bins[0].sum,
        traced.bins[0].sum
    );
    assert!(plain.bins[0].sum > 0.0, "the run scored nothing");
    println!(
        "traced and untraced runs agree bit for bit: {:.12e}",
        plain.bins[0].sum
    );
}

/// The clock is **monotone non-decreasing along a track**, and strictly
/// increasing wherever the particle moved. A clock that could go backwards
/// would put a later event in an earlier time bin.
#[test]
fn the_clock_never_runs_backwards() {
    let geom = void_ball();
    let src = FixedSource::Point {
        r: Position::new(2.0, 1.0, -1.0),
        energy_ev: E0,
    };
    let mut recorder = TrackRecorder::new(16, 64);
    run_fixed_source_traced(
        &geom,
        &[],
        &[],
        &src,
        &settings(64),
        None,
        Some(&mut recorder),
        None,
        None,
    );
    let mut steps = 0usize;
    for track in recorder.tracks {
        for w in track.states.windows(2) {
            assert!(
                w[1].time >= w[0].time,
                "the clock went backwards: {} -> {}",
                w[0].time,
                w[1].time
            );
            steps += 1;
        }
    }
    assert!(steps > 0, "no consecutive states to compare");
    println!("{steps} consecutive state pairs, clock monotone throughout");
}

/// **A `PolarAzimuthalFilter` on a track-length tally now bins on the real
/// direction** — gh:#261.
///
/// Before the flight direction was threaded, `FilterEvent::direction` was
/// defaulted for every track-length event, so this filter put all of them in
/// one bin. The check is structural and does not need a reference: an
/// **isotropic** point source in a void ball must spread its track length
/// **evenly over equal solid angle**, so equal-width bins in `cos(theta)` must
/// each receive the same share within statistics. A filter stuck on a default
/// direction puts 100 % in one bin and 0 % in the rest, which no amount of
/// statistics explains.
#[test]
fn a_polar_filter_on_a_track_length_tally_sees_the_real_direction() {
    use outram_mc_libs::tally::filter::PolarAzimuthalFilter;

    const N_POLAR: usize = 4;
    let edges: Vec<f64> = (0..=N_POLAR)
        .map(|i| -1.0 + 2.0 * i as f64 / N_POLAR as f64)
        .collect();
    let mut tally = Tally {
        id: 2,
        name: "polar flux".into(),
        filters: vec![FilterKind::PolarAzimuthal(PolarAzimuthalFilter {
            polar: edges,
            // One azimuthal bin: this test is about the polar axis.
            azimuthal: vec![-std::f64::consts::PI, std::f64::consts::PI],
        })],
        scores: vec![ScoreType::Flux],
        bins: vec![TallyBin::default(); N_POLAR],
    };
    let geom = void_ball();
    let src = FixedSource::Point {
        r: Position::ZERO,
        energy_ev: E0,
    };
    run_fixed_source(&geom, &[], &[], &src, &settings(20_000), Some(&mut tally));

    let total: f64 = tally.bins.iter().map(|b| b.sum).sum();
    assert!(total > 0.0, "the polar-filtered tally scored nothing");
    let shares: Vec<f64> = tally.bins.iter().map(|b| b.sum / total).collect();
    println!("polar cos(theta) shares over {N_POLAR} equal bins: {shares:?}");

    // Equal solid angle per equal-width cos(theta) bin, so each must hold ~1/N.
    let want = 1.0 / N_POLAR as f64;
    for (i, s) in shares.iter().enumerate() {
        assert!(
            (s - want).abs() < 0.02,
            "polar bin {i} holds {:.4} of the track length against {want:.4} expected \
             for an isotropic source. A share of 1.0 in one bin means the filter is \
             still binning on a defaulted direction.",
            s
        );
    }
}
