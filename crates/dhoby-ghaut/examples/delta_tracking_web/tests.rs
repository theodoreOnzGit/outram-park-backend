//! Headless tests (workspace hard rule). The data-dependent ones process the
//! `triso` rung's ENDF/B-VIII.0 tapes once (about 30 s natively).

use super::*;
use crate::physics::{Birth, GenResult, Method, Physics, Run, RunConfig, Trace};
use crate::state::{ManyRun, MarkKind, Pair, Predict, SegKind};
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::physics::delta_tracking::flight::TentativeSite;
use outram_mc_libs::physics::track_output::{TrackEvent, TrackState};
use outram_mc_libs::physics::tracking_trace::{TraceCounts, TraceEvent};
use std::sync::OnceLock;

fn phys() -> &'static Physics {
    static P: OnceLock<Physics> = OnceLock::new();
    P.get_or_init(|| load_native(|_, _| {}).expect("process the triso rung's tapes"))
}

fn every_kind() -> Vec<TraceEvent> {
    let r = Position::new(0.1, -0.2, 0.3);
    let u = Direction::new(0.6, 0.8, 0.0);
    vec![
        TraceEvent::Start { r, u, e: 2.0e6 },
        TraceEvent::Located {
            r,
            cell: 433,
            material: Some(0),
            sigma_t: 0.247,
        },
        TraceEvent::Located {
            r,
            cell: 2,
            material: None,
            sigma_t: 0.0,
        },
        TraceEvent::Segment {
            xi: Some(0.844),
            d_collision: 0.689,
            d_boundary: 0.0113,
            surface: Some(438),
        },
        TraceEvent::Segment {
            xi: None,
            d_collision: f64::INFINITY,
            d_boundary: 1.5,
            surface: None,
        },
        TraceEvent::Flight {
            from: r,
            u,
            to: Position::new(0.5, 0.3, 0.3),
            xi: 0.844,
            distance: 0.61,
            majorant: 0.279,
            exited: false,
        },
        TraceEvent::Tentative {
            site: TentativeSite {
                position: r,
                material: Some(5),
                majorant: 0.279,
                sigma_t: 0.119,
            },
            xi: Some(0.429),
            real: false,
        },
        TraceEvent::Tentative {
            site: TentativeSite {
                position: r,
                material: None,
                majorant: 0.279,
                sigma_t: 0.0,
            },
            xi: None,
            real: false,
        },
        TraceEvent::Collision {
            r,
            material: 5,
            nuclide: 5,
            e: 3.0e6,
        },
        TraceEvent::State(TrackState {
            r,
            u,
            energy: 2.4e6,
            time: 1e-9,
            weight: 1.0,
            cell: usize::MAX,
            material: Some(5),
            event: TrackEvent::Scatter,
        }),
        TraceEvent::State(TrackState {
            r,
            u,
            energy: 0.03,
            time: 0.0,
            weight: 1.0,
            cell: 7,
            material: None,
            event: TrackEvent::Fission,
        }),
    ]
}

/// What the page draws must be what the worker traced, bit for bit.
#[test]
fn every_message_crosses_the_worker_boundary_bit_for_bit() {
    for e in every_kind() {
        let back = wire::decode_event(&wire::encode_event(&e)).expect("decode");
        assert_eq!(
            format!("{back:?}"),
            format!("{e:?}"),
            "an event changed in transit"
        );
        assert!(!wire::describe(&e).is_empty());
    }
    let mut counts = TraceCounts::default();
    for e in every_kind() {
        counts.add(&e);
    }
    let t = Trace {
        events: every_kind(),
        counts,
        dropped: 3,
    };
    let back = wire::decode_trace(&wire::encode_trace(&t)).expect("trace");
    assert_eq!(back.counts, t.counts);
    assert_eq!(back.dropped, 3);
    assert_eq!(format!("{:?}", back.events), format!("{:?}", t.events));
    let g = GenResult {
        method: Method::DeltaLow,
        index: 4,
        active: true,
        k: 1.5,
        k_mean: Some((1.49, 0.02)),
        counts,
        secs: 0.25,
        n_particles: 200,
        last: false,
    };
    assert_eq!(wire::decode_gen(&wire::encode_gen(&g)).expect("gen"), g);
    let c = RunConfig {
        n_particles: 200,
        n_inactive: 5,
        n_active: 20,
        seed: 784,
        low_factor: Some(0.25),
    };
    assert_eq!(wire::decode_run(&wire::encode_run(&c)).expect("run"), c);
    let c = RunConfig {
        low_factor: None,
        ..c
    };
    assert_eq!(wire::decode_run(&wire::encode_run(&c)).expect("run"), c);
    assert!(wire::decode_event(&[9.0; wire::PER_EVENT]).is_err());
    for m in Method::ALL {
        assert_eq!(Method::from_code(m.code()).unwrap(), m);
    }
    assert_eq!(wire::energy(2.5e6), "2.500 MeV");
    assert_eq!(wire::length(0.0113), "113.0 µm");
    assert_eq!(wire::length(f64::INFINITY), "∞");
    assert_eq!(wire::material_name(physics::VOID), "helium (void)");
}

/// Same neutron, same seed: both trackers' first flight is sampled from the
/// same variate, scaled by Σt (surface) and Σmaj (delta), so the delta
/// flight is never the longer.
#[test]
fn both_panes_run_the_same_neutron_from_the_same_variate() {
    let p = phys();
    for seed in 1..=8 {
        let b = physics::birth(p, seed);
        assert_eq!(
            p.material_at(b.r),
            Some(model::MAT_KERNEL),
            "born in a kernel"
        );
        let s = physics::trace_surface(p, b);
        let d = physics::trace_delta(p, b, &p.majorant);
        let start = |t: &Trace| match t.events[0] {
            TraceEvent::Start { r, u, e } => (r, u, e),
            other => panic!("first event {other:?}"),
        };
        assert_eq!(start(&s), start(&d));
        let (xi_s, d_col, sig_t) = s
            .events
            .windows(2)
            .find_map(|w| match (w[0], w[1]) {
                (
                    TraceEvent::Located { sigma_t, .. },
                    TraceEvent::Segment {
                        xi: Some(x),
                        d_collision,
                        ..
                    },
                ) => Some((x, d_collision, sigma_t)),
                _ => None,
            })
            .expect("a first segment");
        let (xi_d, s_d, maj) = d
            .events
            .iter()
            .find_map(|e| {
                if let TraceEvent::Flight {
                    xi,
                    distance,
                    majorant,
                    ..
                } = e
                {
                    Some((*xi, *distance, *majorant))
                } else {
                    None
                }
            })
            .expect("a first flight");
        assert_eq!(
            xi_s.to_bits(),
            xi_d.to_bits(),
            "neutron {seed}: the first variate differs"
        );
        assert!(
            maj >= sig_t && s_d <= d_col * (1.0 + 1e-12),
            "neutron {seed}: Σmaj {maj} < Σt {sig_t}"
        );
        assert!(
            (s_d / d_col - sig_t / maj).abs() < 1e-9,
            "the flights differ by exactly Σt/Σmaj"
        );
        // Both histories end in an absorption: the cell is reflective.
        for t in [&s, &d] {
            assert!(
                matches!(t.events.last(), Some(TraceEvent::State(st)) if matches!(st.event, TrackEvent::Fission | TrackEvent::Absorption))
            );
            assert_eq!(t.counts.leaked_or_lost, 0);
            assert_eq!(t.dropped, 0);
        }
        assert_eq!(
            d.counts.majorant_violations, 0,
            "the bounding majorant is exceeded on neutron {seed}"
        );
    }
}

/// The claims rest on the bound: audited over the demo's own materials.
#[test]
fn the_demo_majorant_bounds_every_material() {
    let p = phys();
    let a = p.majorant.audit(
        &p.materials,
        &p.nuclides,
        physics::MAJ_E_MIN,
        physics::MAJ_E_MAX,
        100_000,
    );
    eprintln!(
        "majorant audit: worst Σt/Σmaj = {:.4} at {:.4e} eV in {}, {} energies",
        a.worst_ratio,
        a.energy_ev,
        wire::material_name(a.material),
        a.energies_checked
    );
    assert!(
        a.worst_ratio <= 1.0,
        "the demo's majorant is under the data: {a:?}"
    );
    let low = p.majorant.scaled(0.25);
    assert!(
        low.audit(
            &p.materials,
            &p.nuclides,
            physics::MAJ_E_MIN,
            physics::MAJ_E_MAX,
            1000
        )
        .worst_ratio
            > 1.0
    );
}

/// A trace laid out for drawing: one track piece per surface segment or
/// flight, the cursor reaches every event, the counts at the end are the
/// trace's own.
#[test]
fn a_traced_neutron_lays_out_and_steps() {
    let p = phys();
    let b: Birth = physics::birth(p, 3);
    let (s, d) = (
        physics::trace_surface(p, b),
        physics::trace_delta(p, b, &p.majorant),
    );
    let mut pair = Pair::new(3, 1.0, s.clone(), d.clone(), Physics::domain());
    let segs_s = pair
        .surface
        .segs
        .iter()
        .filter(|g| g.kind != SegKind::Flight)
        .count() as u64;
    assert_eq!(
        segs_s, s.counts.boundary_queries,
        "one piece of track per segment"
    );
    assert_eq!(pair.delta.segs.len() as u64, d.counts.flights);
    assert_eq!(
        pair.delta
            .marks
            .iter()
            .filter(|m| matches!(m.kind, MarkKind::Virtual))
            .count() as u64,
        d.counts.virtual_collisions
    );
    assert_eq!(pair.surface.counts(), TraceCounts::default());
    pair.step();
    assert!(matches!(
        pair.surface.current(),
        Some(TraceEvent::Start { .. })
    ));
    assert_eq!(pair.surface.position(), [b.r.x, b.r.y]);
    pair.next_collision();
    for pane in [&pair.surface, &pair.delta] {
        assert!(
            matches!(pane.current(), Some(TraceEvent::State(st)) if st.event != TrackEvent::SurfaceCrossing)
        );
        assert_eq!(pane.counts().collisions, 1);
    }
    pair.playing = true;
    assert!(
        pair.play(1.0, 5.0),
        "five steps a second for a second moves"
    );
    pair.to_end();
    assert!(pair.finished() && !pair.playing);
    assert_eq!(pair.surface.counts(), s.counts);
    assert_eq!(pair.delta.counts(), d.counts);
    let last = |t: &Trace| match t.events.last() {
        Some(TraceEvent::State(st)) => [st.r.x, st.r.y],
        _ => panic!("no end"),
    };
    assert_eq!(pair.surface.position(), last(&s));
    assert_eq!(pair.delta.position(), last(&d));
    pair.rewind();
    assert_eq!(pair.surface.cursor, 0);
    assert!(!pair.play(1.0, 5.0), "a paused pair does not move");
}

/// "Run many" at a tiny size: every method streams every generation in
/// turn, and the scheduler stops when all are done.
#[test]
fn run_many_streams_every_method_in_turn() {
    let p = phys();
    let cfg = RunConfig {
        n_particles: 12,
        n_inactive: 1,
        n_active: 2,
        seed: 9,
        low_factor: Some(0.25),
    };
    let mut run = Run::new(p, cfg);
    let mut many = ManyRun::new(cfg);
    let mut order = Vec::new();
    while let Some(m) = many.next_request() {
        assert!(
            many.next_request().is_none(),
            "one request in flight at a time"
        );
        order.push(m);
        let g = run.step(p, m).expect("a generation");
        assert!(g.k.is_finite() && g.secs >= 0.0 && g.counts.starts >= 12);
        many.receive(g);
    }
    assert_eq!(order.len(), 9);
    assert_eq!(&order[..3], &Method::ALL);
    assert!(many.all_done() && !many.running);
    for m in Method::ALL {
        let t = many.tally(m).unwrap();
        assert_eq!((t.generations, t.histories), (3, 36));
        assert!(t.k.is_some() && t.us_per_history() > 0.0);
    }
    assert!(
        many.tally(Method::DeltaLow)
            .unwrap()
            .counts
            .majorant_violations
            > 0
    );
    assert!(many.difference(Method::Delta, Method::Surface).is_some());
    assert!(
        run.step(p, Method::Surface).is_none(),
        "no generation is left"
    );
    assert_eq!(run.methods(), &Method::ALL);
}

/// The scheduler, the predictions and the z-score, with no data.
#[test]
fn the_page_state_needs_no_physics() {
    let cfg = RunConfig {
        n_particles: 10,
        n_inactive: 0,
        n_active: 1,
        seed: 1,
        low_factor: None,
    };
    let mut many = ManyRun::new(cfg);
    assert_eq!(many.methods, vec![Method::Surface, Method::Delta]);
    assert_eq!(many.next_request(), Some(Method::Surface));
    many.failed();
    assert!(
        many.next_request().is_none(),
        "a failed run does not ask again"
    );
    let one = |m: Method| GenResult {
        method: m,
        index: 0,
        active: true,
        k: 1.0,
        k_mean: Some((1.0 + m.code() * 0.01, 0.0)),
        counts: TraceCounts::default(),
        secs: 0.1,
        n_particles: 10,
        last: true,
    };
    let mut fresh = ManyRun::new(cfg);
    fresh.receive(one(Method::Surface));
    fresh.receive(one(Method::Delta));
    assert!(fresh.all_done() && !fresh.running);
    assert!(
        fresh.difference(Method::Delta, Method::Surface).is_none(),
        "no σ yet, so no z-score"
    );
    let mut p = Predict::new("q?", &["a", "b"]);
    assert!(p.verdict().is_none());
    p.chosen = Some(1);
    assert_eq!(p.verdict().as_deref(), Some("You predicted: b."));
    assert!((physics::z_score((1.0, 0.003), (1.005, 0.004)) + 1.0).abs() < 1e-12);
    assert_eq!(engine::job_labels().len(), engine::job_weights().len());
    assert_eq!(*engine::job_labels().last().unwrap(), "majorant");
}

/// The GUI's engine thread (the native twin of the browser's worker) loads,
/// traces the neutron the headless functions trace, and runs a generation.
#[test]
fn the_engine_thread_loads_traces_and_runs() {
    use engine::{Event, Request};
    let link = dhoby_ghaut::web_demo::link::start_native(engine::Engine::default(), || {});
    link.send(Request::Load { id: 1 });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(900);
    let (mut ready, mut traced, mut gens, mut jobs) = (false, false, 0, 0);
    while !(traced && gens == 2) {
        assert!(
            std::time::Instant::now() < deadline,
            "engine thread timed out"
        );
        for e in link.drain() {
            match e {
                Event::JobDone { .. } => jobs += 1,
                Event::Ready { points, .. } => {
                    assert!(points > 1000);
                    ready = true;
                    link.send(Request::Trace {
                        seed: 5,
                        factor: 1.0,
                    });
                    link.send(Request::RunStart(RunConfig {
                        n_particles: 8,
                        n_inactive: 0,
                        n_active: 1,
                        seed: 3,
                        low_factor: None,
                    }));
                    link.send(Request::RunStep(Method::Surface));
                    link.send(Request::RunStep(Method::Delta));
                }
                Event::Trace {
                    seed,
                    surface,
                    delta,
                    ..
                } => {
                    let b = physics::birth(phys(), seed);
                    assert_eq!(surface, physics::trace_surface(phys(), b));
                    assert_eq!(delta, physics::trace_delta(phys(), b, &phys().majorant));
                    traced = true;
                }
                Event::Gen(g) => {
                    assert!(g.last && g.k.is_finite());
                    gens += 1;
                }
                Event::Error(m) => panic!("engine: {m}"),
                Event::JobStarted { .. } => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(ready);
    assert_eq!(jobs, engine::job_labels().len());
}
