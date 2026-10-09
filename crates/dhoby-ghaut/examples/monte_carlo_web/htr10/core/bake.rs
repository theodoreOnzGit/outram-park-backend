//! **Native side of the core view** (gh:#786): the tapes the browser
//! downloads, and the bake of the recorded tracks the view opens on.
//!
//! ```text
//! cargo run -p dhoby-ghaut --example monte_carlo_web --release -- --bake-htr10-core [particles generations threads]
//! ```
//!
//! The bake runs the **pool's own code** natively: every job through
//! [`super::run_job`] on `threads` threads (the page's role played by a
//! work queue), one [`super::CoreWorker`] assembling the model, and
//! `DistributedPowerIteration` reducing chunks transported on `threads`
//! threads. It prints the per-job, assembly and transport timings, and each
//! generation's k, which at the record's settings (10 000 neutrons, seed
//! 20260917, N = 12) can be set beside the recorded log's first generations
//! (`htr10_seker_2026_10_07_10k/logs/run_e8_N12.log`). It writes the traced
//! histories to `htr10/data/core_tracks.bin.z`.

use super::{jobs, native_tape, run_job, CoreWorker, RECORD_SEED};
use crate::keff::KeffConfig;
use dhoby_ghaut::web_demo::link::Floats;
use nee_soon::htr10_rmc::data::Htr10DataConfig;
use outram_mc_libs::physics::transport_csg::distributed::{ChunkResult, DistributedPowerIteration};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

/// Histories traced per generation in the bake (more than the live run's,
/// so the view has a variety to replay; tracing changes nothing else).
pub const BAKE_TRACED: usize = 12;

/// The committed track file.
pub fn tracks_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples/monte_carlo_web/htr10/data/core_tracks.bin.z")
}

/// Write every core tape the browser downloads into `dir` (covariances
/// stripped, zlib), skipping any already written by another rung. The rod
/// steel's nickel comes from the `reference-data/ace` submodule.
///
/// # Errors
///
/// A tape that is not in this checkout (for nickel:
/// `git submodule update --init reference-data/ace`).
pub fn prepare_web_data(dir: &std::path::Path) -> Result<(usize, usize), String> {
    let cfg = Htr10DataConfig::default();
    let (mut raw_t, mut wire_t) = (0usize, 0usize);
    for j in jobs()? {
        let Some(t) = j.tape(&cfg).map_err(|e| e.to_string())? else {
            continue;
        };
        let out = dir.join(crate::tapes::wire_name(t.file));
        if out.exists() {
            continue;
        }
        let stripped = native_tape(&t).map_err(|e| {
            format!("{e} (for nickel: git submodule update --init reference-data/ace)")
        })?;
        let wire = crate::tapes::compress(&stripped);
        std::fs::write(&out, &wire).map_err(|e| e.to_string())?;
        println!(
            "{:<34} {:>10.2} {:>10.2}   (htr10 core)",
            t.file,
            stripped.len() as f64 / 1e6,
            wire.len() as f64 / 1e6
        );
        raw_t += stripped.len();
        wire_t += wire.len();
    }
    Ok((raw_t, wire_t))
}

/// Process every job on `threads` threads and assemble one worker's model.
pub fn build_worker(layers: usize, threads: usize) -> Result<CoreWorker, String> {
    let all = jobs()?;
    let cfg = Htr10DataConfig::default();
    let next = AtomicUsize::new(0);
    let done: Mutex<Vec<(usize, Floats, f64)>> = Mutex::new(Vec::new());
    let errors: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let t0 = Instant::now();
    std::thread::scope(|s| {
        for _ in 0..threads.max(1) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                let Some(job) = all.get(i) else { break };
                let t = Instant::now();
                let r = job
                    .tape(&cfg)
                    .map_err(|e| e.to_string())
                    .and_then(|tape| tape.map(|t| native_tape(&t)).transpose())
                    .and_then(|bytes| run_job(i, bytes.as_deref()));
                match r {
                    Ok(f) => done
                        .lock()
                        .map(|mut d| d.push((i, f, t.elapsed().as_secs_f64())))
                        .unwrap_or(()),
                    Err(e) => errors
                        .lock()
                        .map(|mut v| v.push(format!("{}: {e}", job.label())))
                        .unwrap_or(()),
                }
            });
        }
    });
    if let Some(e) = errors.into_inner().map_err(|_| "poisoned")?.first() {
        return Err(e.clone());
    }
    let mut done = done.into_inner().map_err(|_| "poisoned")?;
    done.sort_by_key(|d| d.0);
    println!(
        "jobs on {threads} threads, {:.1} s wall:",
        t0.elapsed().as_secs_f64()
    );
    let mut w = CoreWorker::default();
    for (i, f, secs) in done {
        println!(
            "  {:<20} {secs:7.1} s  {:>9} words",
            all[i].label(),
            f.len()
        );
        w.store(i, f);
    }
    let mut tapes_s = 0.0;
    let mut rep = w.assemble(layers, |t| {
        let t0 = Instant::now();
        let b = native_tape(t);
        tapes_s += t0.elapsed().as_secs_f64();
        b
    })?;
    rep.tapes_s = tapes_s;
    rep.nuclides_s -= tapes_s;
    println!(
        "assembly (one worker): tapes {:.1} s, nuclides {:.1} s, core {:.2} s, majorant {:.1} s ({} nodes)",
        rep.tapes_s,
        rep.nuclides_s,
        rep.geometry_s,
        rep.majorant_s,
        w.model.as_ref().map_or(0, |m| m.majorant.len())
    );
    Ok(w)
}

/// Run `generations` generations of `cfg` on `w`, chunks on `threads`
/// threads; returns the traced tracks `(generation, index, track)` and each
/// generation's `k`.
pub fn run(
    w: &CoreWorker,
    cfg: KeffConfig,
    generations: usize,
    threads: usize,
    traced: usize,
) -> Result<
    (
        Vec<(usize, usize, outram_mc_libs::physics::track_output::Track)>,
        Vec<f64>,
    ),
    String,
> {
    let super::CoreEv::Source { sites, seed, mesh, .. } = w.source(&cfg, 1)? else {
        return Err("no source".into());
    };
    let sites =
        outram_mc_libs::physics::transport_csg::distributed::SourceSite::decode(&sites.to_vec())?;
    let mut it =
        DistributedPowerIteration::from_initial_source(&super::run_settings(&cfg), &sites, seed);
    let mesh = super::mesh_from_words(&mesh);
    let (mut tracks, mut ks) = (Vec::new(), Vec::new());
    for _ in 0..generations {
        if it.finished() {
            break;
        }
        let t = Instant::now();
        let chunks = it.chunks(threads.max(1), traced);
        let results: Vec<Result<ChunkResult, String>> = std::thread::scope(|s| {
            let hs: Vec<_> = chunks
                .iter()
                .map(|c| {
                    s.spawn(move || {
                        w.chunk(&cfg, &c.to_f64s())
                            .and_then(|v| ChunkResult::from_f64s(&v))
                    })
                })
                .collect();
            hs.into_iter()
                .map(|h| {
                    h.join()
                        .map_err(|_| "a chunk panicked".to_string())
                        .and_then(|r| r)
                })
                .collect()
        });
        let results = results.into_iter().collect::<Result<Vec<_>, _>>()?;
        let g = it.finish_generation(results, Some(&mesh))?;
        let secs = t.elapsed().as_secs_f64();
        println!(
            "  generation {}: k = {:.6}, entropy {:.4}, {} histories in {secs:.1} s on {threads} threads ({:.2} ms CPU per history), {} collisions, {} virtual",
            g.index,
            g.k,
            g.entropy.unwrap_or(f64::NAN),
            g.counts.histories,
            1e3 * secs * threads as f64 / g.counts.histories.max(1) as f64,
            g.counts.collisions,
            g.counts.virtual_collisions
        );
        ks.push(g.k);
        tracks.extend(g.tracks.into_iter().map(|(i, t)| (g.index, i, t)));
    }
    Ok((tracks, ks))
}

/// `--bake-htr10-core [particles generations threads]`.
pub fn bake_cli(args: &[String]) -> Result<(), String> {
    let num = |i: usize, d: usize| {
        args.get(i)
            .map_or(Ok(d), |s| s.parse::<usize>().map_err(|e| e.to_string()))
    };
    let (n, gens, threads) = (num(1, 10_000)?, num(2, 2)?, num(3, 5)?);
    let layers = super::super::LADDER_N as usize;
    println!("HTR-10 core bake: N = {layers}, {n} neutrons x {gens} generations, seed {RECORD_SEED}, {threads} threads");
    println!(
        "hardware: {}",
        outram_mc_libs::perf_report::HardwareInfo::detect().headline()
    );
    let t0 = Instant::now();
    let w = build_worker(layers, threads)?;
    println!("data ready in {:.1} s", t0.elapsed().as_secs_f64());
    let cfg = KeffConfig {
        n_particles: n,
        n_inactive: gens,
        n_active: 0,
        seed: RECORD_SEED,
        point_source: false,
        want_sites: false,
    };
    let cfg = KeffConfig {
        n_inactive: gens.saturating_sub(1).max(1),
        n_active: 1,
        ..cfg
    };
    let (tracks, ks) = run(&w, cfg, gens, threads, BAKE_TRACED)?;
    let thinned: Vec<_> = tracks
        .into_iter()
        .map(|(g, i, mut t)| {
            t.states.retain(|s| {
                s.event != outram_mc_libs::physics::track_output::TrackEvent::SurfaceCrossing
            });
            (g, i, t)
        })
        .collect();
    let bytes = super::screen::encode_tracks(&thinned);
    std::fs::write(tracks_path(), &bytes).map_err(|e| e.to_string())?;
    println!(
        "wrote {} ({} tracks, {:.0} kB); k by generation {ks:?}; total {:.1} s",
        tracks_path().display(),
        thinned.len(),
        bytes.len() as f64 / 1e3,
        t0.elapsed().as_secs_f64()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole native pipeline at a tiny size: the pool's jobs on two
    /// threads, one worker's assembly, two generations of 40 neutrons on two
    /// threads give the same k as on one (the reduction is the run's, not
    /// the threads'), and traced tracks. Opt-in: processes every tape
    /// (about a minute on 2 threads).
    #[test]
    #[ignore = "processes all 36 HTR-10 jobs (minutes); run with --ignored"]
    fn the_native_pipeline_builds_the_core_and_runs_it() {
        let w = build_worker(12, 2).expect("worker");
        let m = w.model.as_ref().expect("model");
        assert_eq!(m.nuclides.len(), 38);
        let cfg = KeffConfig {
            n_particles: 40,
            n_inactive: 1,
            n_active: 1,
            seed: RECORD_SEED,
            point_source: false,
            want_sites: false,
        };
        let (t2, k2) = run(&w, cfg, 2, 2, 3).expect("run");
        let (_, k1) = run(&w, cfg, 2, 1, 0).expect("run");
        assert_eq!(
            k1.iter().map(|k| k.to_bits()).collect::<Vec<_>>(),
            k2.iter().map(|k| k.to_bits()).collect::<Vec<_>>()
        );
        assert_eq!(t2.len(), 6);
        let dir = std::env::temp_dir().join("htr10_core_prepare_test");
        let _ = std::fs::create_dir_all(&dir);
        let (raw, wire) = prepare_web_data(&dir).expect("prepare");
        assert!(wire < raw || raw == 0);
    }
}
