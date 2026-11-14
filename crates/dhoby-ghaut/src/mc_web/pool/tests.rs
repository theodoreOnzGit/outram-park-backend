//! Tests of the pool's two state machines on their own (gh:#1572). The whole
//! protocol on a real model, bit for bit against `run_keff_csg_par` for any
//! number of workers, with restarts and dead workers, is pinned by the
//! `monte_carlo_web` example's `htr10::core::pool::tests`, which drive these
//! machines through the HTR-10 `PoolState`.

use super::*;

/// The dealer is longest-processing-time first, ties to the lower worker,
/// and gives every job exactly once.
#[test]
fn the_dealer_is_lpt_and_deals_every_job_once() {
    // Longest first: 0 (5) to worker 0; 2 (4) to 1; 3 (3) to 1 (4 < 5);
    // 4 (3) to 0 (5 < 7); 1 (1) to 1 (7 < 8).
    let d = schedule(&[5.0, 1.0, 4.0, 3.0, 3.0], 2);
    assert_eq!(d, vec![vec![0, 4], vec![2, 3, 1]]);
    let mut all: Vec<usize> = d.concat();
    all.sort();
    assert_eq!(all, vec![0, 1, 2, 3, 4]);
    assert_eq!(schedule(&[1.0, 2.0], 0), vec![vec![1, 0]], "at least one worker");
}

/// An entropy mesh survives its words.
#[test]
fn a_mesh_survives_its_words() {
    let m = RegularMesh { lower_left: [-1.5, -2.0, 0.0], upper_right: [1.5, 2.0, 7.25], dimension: [8, 9, 10] };
    let back = mesh_from_words(&mesh_words(&m));
    assert_eq!((back.lower_left, back.upper_right, back.dimension), (m.lower_left, m.upper_right, m.dimension));
}

/// The data relay: each worker is sent its jobs once; each product goes to
/// every other worker and never back to its maker; a worker is told to
/// assemble only once every product has reached it, and the relay reports
/// when all have been told.
#[test]
fn every_product_reaches_every_other_worker_before_assembly() {
    let mut d = DataRelay::new(3, vec!["a".into(), "b".into(), "c".into(), "d".into()], vec![4.0, 3.0, 2.0, 1.0]);
    let (first, all) = d.outbox(RELAY_WORDS_PER_FRAME, true);
    assert!(!all);
    let dealt: Vec<(usize, Vec<usize>)> = first
        .into_iter()
        .map(|o| match o {
            DataOut::Process { worker, jobs } => (worker, jobs),
            _ => panic!("only processing first"),
        })
        .collect();
    assert_eq!(dealt.len(), 3);
    let mut got = vec![std::collections::BTreeSet::new(); 3];
    let mut assembled = vec![false; 3];
    for (w, jobs) in &dealt {
        for &j in jobs {
            got[*w].insert(j);
            d.on_started(j);
            d.on_product(*w, j, 1.0, Floats::from_vec(vec![j as f64; 10]), Source::Processed);
        }
    }
    assert!(d.all_processed());
    assert!((d.processed() - 1.0).abs() < 1e-12);
    let mut all_sent = false;
    for _ in 0..20 {
        let (out, all) = d.outbox(15, true);
        for o in out {
            match o {
                DataOut::Relay { worker, job, data } => {
                    assert!(!assembled[worker], "a product after its worker's assembly");
                    assert_ne!(d.owner[job], worker, "a product went back to its maker");
                    assert_eq!(data.to_vec(), vec![job as f64; 10]);
                    assert!(got[worker].insert(job), "relayed twice");
                }
                DataOut::Assemble { worker } => {
                    assert_eq!(got[worker].len(), 4, "worker {worker} assembled without every product");
                    assembled[worker] = true;
                }
                DataOut::Process { .. } => panic!("processing sent twice"),
            }
        }
        all_sent |= all;
    }
    assert!(all_sent && assembled.iter().all(|&a| a));
}

/// A k_eff pool refuses a second start while a run is going, sends one
/// source request, discards a source for another epoch visibly, and a
/// failed worker's message for no run is only noted.
#[test]
fn the_run_protocol_is_guarded() {
    fn settings(cfg: &KeffConfig) -> KeffSettings {
        KeffSettings { n_particles: cfg.n_particles, n_inactive: cfg.n_inactive, n_active: cfg.n_active, seed: cfg.seed, ..KeffSettings::default() }
    }
    let cfg = KeffConfig { n_particles: 10, n_inactive: 1, n_active: 2, seed: 7, point_source: false, want_sites: false };
    let mut p: KeffPool<()> = KeffPool::new(2, "test data".into(), settings, 0);
    p.start_run(cfg, (), 0.0).unwrap();
    assert!(p.start_run(cfg, (), 0.0).is_err(), "a run is going");
    let out = p.outbox();
    assert_eq!(out.len(), 1);
    assert!(matches!(out[0], (0, RunReq::Source { epoch: 1, .. })));
    assert!(p.is_busy(0) && !p.is_busy(1));
    assert!(p.outbox().is_empty(), "one source request only");
    p.on_source(1, &Floats::from_vec(Vec::new()), 0, &[0.0; 9], 99, 0.0);
    assert_eq!(p.stale_discarded, 1);
    p.on_failed(1, "nothing to do", NO_RUN);
    assert!(p.log.iter().any(|l| l.contains("nothing to do")));
    assert!(p.going());
    p.stop_run();
    assert!(!p.going());
}
