//! **Step-level tracking events** (gh:#784): the observer hooks the rung-5
//! tutorial demo uses to show surface tracking and delta tracking step by
//! step, side by side.
//!
//! # What is checked, and why
//!
//! 1. **Observing changes nothing.** A traced power iteration
//!    (`CsgPowerIteration::step_traced`, `DeltaPowerIteration::step_traced`)
//!    gives the untraced one's generations bit for bit. The demo's whole
//!    claim is that it shows the histories the library runs, not a re-play.
//! 2. **The events tell the truth about the sampling.** In a delta trace,
//!    every flight's distance is `−ln ξ / Σ_maj` of the variate it reports,
//!    every tentative site is real exactly when `ξ < Σ_t/Σ_maj`, and every
//!    real site is followed by a collision. In a surface trace, every
//!    segment ends at the nearer of its two distances: a collision when
//!    `d_col < d_boundary`, a crossing otherwise.
//! 3. **A majorant too low is visible in the events**, and a bounding one
//!    shows no violation over the energies the histories visit.
//! 4. **The two methods agree within statistics** on the same three-zone
//!    reflective sphere (unbiasedness), measured, not assumed.
//!
//! # Model
//!
//! The three-zone reflective sphere of `delta_tracking_bit_identity.rs`
//! (fuel r < 2 cm, graphite to 3 cm, dilute graphite to 5 cm), LOW-tier
//! embedded U-235, U-238 and C (`Nuclide::from_core`), 293.6 K.
//!
//! # Results (2026-10-07, measured by this file; release build)
//!
//! Printed by `the_two_methods_agree_within_statistics` with
//! `--nocapture`; the pass criterion is |Δk| ≤ 4 σ_combined. The value
//! recorded on the first run is in that test's doc comment.

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::pebble_beds::delta_tracking::Majorant;
use outram_mc_libs::pebble_beds::keff_delta::{trace_delta_history, DeltaDomain, DeltaPowerIteration};
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::track_output::TrackEvent;
use outram_mc_libs::physics::tracking_trace::{TraceCounts, TraceEvent};
use outram_mc_libs::physics::transport_csg::{trace_csg_history, CsgPowerIteration, SourceBox};
use outram_mc_libs::rng::distributions::isotropic_direction;

const TEMP: f64 = 293.6;
const R_OUT: f64 = 5.0;
const R_FUEL: f64 = 2.0;
const R_GRAPHITE: f64 = 3.0;

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

/// The same three zones as CSG, surface-tracked throughout.
fn geometry() -> Geometry {
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
    Geometry {
        surfaces: vec![
            sphere(R_OUT, BoundaryType::Reflective),
            sphere(R_FUEL, BoundaryType::Transmissive),
            sphere(R_GRAPHITE, BoundaryType::Transmissive),
        ],
        cells: vec![
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
            Cell::material(
                12,
                vec![
                    hs(2, HalfSpaceSense::Outside),
                    hs(0, HalfSpaceSense::Inside),
                    RegionToken::Intersection,
                ],
                2,
                TEMP,
            ),
        ],
        universes: vec![Universe {
            id: 0,
            cell_indices: vec![0, 1, 2],
        }],
        lattices: vec![],
        root_universe: 0,
    }
}

fn majorant(mats: &[Material], nucs: &[Nuclide]) -> Majorant {
    let (lo, hi) = (1.0e-5_f64, 2.0e7_f64);
    let grid: Vec<f64> = (0..512)
        .map(|i| (lo.ln() + (hi.ln() - lo.ln()) * i as f64 / 511.0).exp())
        .collect();
    Majorant::over_indices(mats, &[0, 1, 2], nucs, &grid, 0.1)
}

fn settings(n: usize, inactive: usize, active: usize, seed: u64) -> KeffSettings {
    KeffSettings {
        n_particles: n,
        n_inactive: inactive,
        n_active: active,
        temperature_k: TEMP,
        seed,
        ..KeffSettings::default()
    }
}

fn source_box() -> SourceBox {
    SourceBox {
        lower: Position::new(-R_FUEL, -R_FUEL, -R_FUEL),
        upper: Position::new(R_FUEL, R_FUEL, R_FUEL),
    }
}

/// A neutron born in the fuel, the same for both methods.
fn birth(seed: &mut u64) -> (Position, Direction, f64) {
    let (u, v, w) = isotropic_direction(seed);
    (
        Position::new(0.3, -0.2, 0.1),
        Direction::new(u, v, w),
        2.0e6,
    )
}

#[test]
fn traced_power_iterations_are_the_untraced_ones_bit_for_bit() {
    let (mats, nucs) = (materials(), nuclides());
    let geom = geometry();
    let s = settings(200, 2, 3, 20_261_007);

    let mut plain = CsgPowerIteration::new(&geom, &mats, &nucs, source_box(), &s);
    let mut traced = plain.clone();
    let mut counts = TraceCounts::default();
    while let Some(a) = plain.step(&geom, &mats, &nucs) {
        let b = traced
            .step_traced(&geom, &mats, &nucs, |e| counts.add(&e))
            .expect("same length");
        assert_eq!(a, b, "surface tracking: a traced generation differs");
    }
    assert!(traced.finished());
    assert!(
        counts.boundary_queries > counts.collisions && counts.crossings > 0 && counts.flights == 0
    );

    let maj = majorant(&mats, &nucs);
    let domain = DeltaDomain::Sphere { radius: R_OUT };
    let mut plain = DeltaPowerIteration::new(domain, &mats, &nucs, &zone, &s);
    let mut traced = plain.clone();
    let mut counts = TraceCounts::default();
    while let Some(a) = plain.step(&mats, &nucs, &maj, &zone) {
        let b = traced
            .step_traced(&mats, &nucs, &maj, &zone, |e| counts.add(&e))
            .expect("same length");
        assert_eq!(a, b, "delta tracking: a traced generation differs");
    }
    assert!(
        counts.tentative == counts.flights
            && counts.virtual_collisions > 0
            && counts.boundary_queries == 0
    );
    assert_eq!(
        counts.tentative - counts.virtual_collisions,
        counts.collisions
    );
}

#[test]
fn a_delta_trace_reports_the_sampling_it_did() {
    let (mats, nucs) = (materials(), nuclides());
    let maj = majorant(&mats, &nucs);
    let domain = DeltaDomain::Sphere { radius: R_OUT };
    let mut seed = 11_u64;
    let mut checked = 0usize;
    for _ in 0..200 {
        let (r, u, e) = birth(&mut seed);
        let mut evs = Vec::new();
        trace_delta_history(
            r,
            u,
            e,
            domain,
            &mats,
            &nucs,
            &maj,
            &zone,
            &mut seed,
            |ev| evs.push(ev),
        );
        assert!(matches!(evs[0], TraceEvent::Start { .. }));
        let mut at = r;
        for (i, ev) in evs.iter().enumerate() {
            match *ev {
                TraceEvent::Start { r, .. } => at = r,
                TraceEvent::Flight {
                    from,
                    xi,
                    distance,
                    majorant,
                    exited,
                    to,
                    ..
                } => {
                    assert_eq!(from, at, "a flight starts where the last one ended");
                    let want = -xi.max(f64::MIN_POSITIVE).ln() / majorant;
                    assert!(
                        (distance - want).abs() <= 1e-12 * want.max(1.0),
                        "s = -ln xi / maj: {distance} vs {want}"
                    );
                    assert!(!exited, "a reflective sphere never exits");
                    assert!(to.norm() <= R_OUT * (1.0 + 1e-12));
                    at = to;
                    checked += 1;
                }
                TraceEvent::Tentative { site, xi, real } => {
                    let xi = xi.expect("no void in this model");
                    let p = (site.sigma_t / site.majorant).clamp(0.0, 1.0);
                    assert_eq!(real, xi < p, "real iff xi < sigma_t / sigma_maj");
                    assert!(
                        !site.violates_majorant(),
                        "the bounding majorant is exceeded at {site:?}"
                    );
                    if real {
                        assert!(
                            matches!(evs[i + 1], TraceEvent::Collision { .. }),
                            "a real site is a collision"
                        );
                    }
                }
                TraceEvent::State(s) => at = s.r,
                TraceEvent::Collision { r, .. } => assert_eq!(r, at),
                TraceEvent::Located { .. } | TraceEvent::Segment { .. } => {
                    panic!("delta tracking does not locate")
                }
            }
        }
        assert!(matches!(
            evs.last(),
            Some(TraceEvent::State(s)) if matches!(s.event, TrackEvent::Fission | TrackEvent::Absorption)
        ));
    }
    assert!(checked > 1000);
}

#[test]
fn a_surface_trace_ends_every_segment_at_the_nearer_distance() {
    let (mats, nucs) = (materials(), nuclides());
    let geom = geometry();
    let mut seed = 12_u64;
    for _ in 0..200 {
        let (r, u, e) = birth(&mut seed);
        let mut evs = Vec::new();
        trace_csg_history(r, u, e, &geom, &mats, &nucs, &mut seed, |ev| evs.push(ev));
        assert!(matches!(evs[0], TraceEvent::Start { .. }));
        for (i, ev) in evs.iter().enumerate() {
            match *ev {
                TraceEvent::Located {
                    sigma_t, material, ..
                } => {
                    assert!(sigma_t > 0.0 && material.is_some());
                    assert!(
                        matches!(evs[i + 1], TraceEvent::Segment { .. }),
                        "every locate is followed by a query"
                    );
                }
                TraceEvent::Segment {
                    xi,
                    d_collision,
                    d_boundary,
                    surface,
                } => {
                    let xi = xi.expect("no void");
                    assert!(d_collision > 0.0 && d_boundary > 0.0 && surface.is_some());
                    assert!(xi > 0.0 && xi < 1.0);
                    match evs[i + 1] {
                        TraceEvent::Collision { .. } => assert!(d_collision < d_boundary),
                        TraceEvent::State(s) => {
                            assert_eq!(s.event, TrackEvent::SurfaceCrossing);
                            assert!(d_collision >= d_boundary);
                        }
                        other => panic!("a segment ended in {other:?}"),
                    }
                }
                TraceEvent::Flight { .. } | TraceEvent::Tentative { .. } => {
                    panic!("surface tracking has no majorant")
                }
                _ => {}
            }
        }
    }
}

/// A majorant scaled below the bound shows in the events as violations,
/// and is the only thing that does: the run completes without complaint.
#[test]
fn a_majorant_too_low_shows_as_violations() {
    let (mats, nucs) = (materials(), nuclides());
    let maj = majorant(&mats, &nucs);
    let low = maj.scaled(0.3);
    for e in [1.0e-3, 0.0253, 1.0, 6.67, 1.0e3, 1.0e6] {
        assert_eq!(low.at(e), 0.3 * maj.at(e));
    }
    let domain = DeltaDomain::Sphere { radius: R_OUT };
    let count = |m: &Majorant| {
        let mut c = TraceCounts::default();
        let mut seed = 13_u64;
        for _ in 0..200 {
            let (r, u, e) = birth(&mut seed);
            trace_delta_history(r, u, e, domain, &mats, &nucs, m, &zone, &mut seed, |ev| {
                c.add(&ev)
            });
        }
        c
    };
    let (good, bad) = (count(&maj), count(&low));
    assert_eq!(good.majorant_violations, 0);
    assert!(
        bad.majorant_violations > 0,
        "a 0.3x majorant must be exceeded somewhere"
    );
    assert_eq!(
        bad.leaked_or_lost, 0,
        "and nothing else reports it: no history is lost"
    );
}

/// **Unbiasedness, measured.** The same eigenvalue problem by surface
/// tracking (`CsgPowerIteration`) and by delta tracking
/// (`DeltaPowerIteration`), 2000 neutrons × [10 + 40] each, seed 784.
/// Pass: |Δk| ≤ 4 σ_combined (4 rather than 3 so an honest build does not
/// fail one run in 370).
///
/// Recorded 2026-10-07 (release build, one thread; no timing is quoted, so
/// the host matters only for libm bits): surface k = 1.28482 ± 0.00386, delta k = 1.28727 ± 0.00438,
/// Δ = −245 ± 584 pcm (−0.42 σ). Deterministic for the seed, so a re-run
/// prints the same digits.
#[test]
fn the_two_methods_agree_within_statistics() {
    let (mats, nucs) = (materials(), nuclides());
    let geom = geometry();
    let s = settings(2000, 10, 40, 784);
    let mut it = CsgPowerIteration::new(&geom, &mats, &nucs, source_box(), &s);
    let mut surf = None;
    while let Some(g) = it.step(&geom, &mats, &nucs) {
        surf = g.k_mean;
    }
    let maj = majorant(&mats, &nucs);
    let mut it = DeltaPowerIteration::new(
        DeltaDomain::Sphere { radius: R_OUT },
        &mats,
        &nucs,
        &zone,
        &s,
    );
    let mut delta = None;
    while let Some(g) = it.step(&mats, &nucs, &maj, &zone) {
        delta = g.k_mean;
    }
    let ((ks, ss), (kd, sd)) = (surf.expect("surface k"), delta.expect("delta k"));
    let sigma = (ss * ss + sd * sd).sqrt();
    let z = (ks - kd) / sigma;
    eprintln!(
        "three-zone sphere: surface k = {ks:.5} ± {ss:.5}, delta k = {kd:.5} ± {sd:.5}, \
         Δ = {:+.0} ± {:.0} pcm ({z:+.2} σ)",
        (ks - kd) * 1e5,
        sigma * 1e5
    );
    assert!(
        z.abs() <= 4.0,
        "surface and delta tracking disagree by {z:.2} sigma"
    );
}
