//! The Monte Carlo demo's headless tests (workspace hard rule), moved from
//! `examples/monte_carlo_web/main.rs` with the demo (gh:#1763), unchanged
//! but for the crate paths.

use super::*;
use petir::mathf::RealMath;
use engine::Tier;
use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::physics::track_output::TrackEvent;
use std::sync::OnceLock;
use table::{Loaded, Rung};
use triso::model;

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/triso_pebble_web_headless.csv");
const FIXTURE_N: usize = 40;
const FIXTURE_SEED: u64 = 1;

fn physics() -> &'static triso::sim::Physics {
    static P: OnceLock<triso::sim::Physics> = OnceLock::new();
    P.get_or_init(|| match load_native(Rung::Triso, Tier::Exact, |_, _| {}).expect("load nuclear data") {
        Loaded::Triso(l) => l.phys,
        _ => unreachable!(),
    })
}

fn godiva_physics() -> &'static godiva::sim::Physics {
    static P: OnceLock<godiva::sim::Physics> = OnceLock::new();
    P.get_or_init(|| match load_native(Rung::Godiva, Tier::Exact, |_, _| {}).expect("load nuclear data") {
        Loaded::Godiva(l) => l.phys,
        _ => unreachable!(),
    })
}

/// Covariance stripping must change nothing transport reads. Checked on
/// O-16 (39 of its 42 MB are covariances) and U-235 (whose stripped MF 35
/// is the fission-spectrum covariance — so the spectrum itself must be
/// untouched): cross sections bit-identical on a dense log grid, and
/// fission-energy samples bit-identical for the same seeds.
#[test]
fn stripping_covariances_changes_nothing_transport_reads() {
    for (label, tape) in [("O-16", "n-008_O_016-ENDF8.0.endf"), ("U-235", "n-092_U_235-ENDF8.0.endf")] {
        let path = njoy_outram_park_fork::reference_data::reference_endf(tape).expect("tape present");
        let raw = std::fs::read(path).unwrap();
        let stripped = tapes::strip_covariances(&raw);
        assert!(stripped.len() < raw.len() / 2, "{label}: stripping removed too little");
        let full = model::nuclide_from_bytes(&raw, label).unwrap();
        let thin = model::nuclide_from_bytes(&stripped, label).unwrap();
        assert_eq!(full.awr.to_bits(), thin.awr.to_bits(), "{label}: AWR");
        let one = Material {
            id: 1,
            name: label.into(),
            components: vec![NuclideComponent { nuclide_idx: 0, atom_density: 1.0 }],
            temperature: model::TEMPERATURE_K,
        };
        for k in 0..=2000 {
            let e = 1.0e-5 * 10f64.r_powf(12.3 * k as f64 / 2000.0); // 1e-5 eV .. 20 MeV
            let (a, b) = (one.macro_xs(e, std::slice::from_ref(&full)), one.macro_xs(e, std::slice::from_ref(&thin)));
            for (what, x, y) in [
                ("total", a.total, b.total), ("elastic", a.elastic, b.elastic),
                ("absorption", a.absorption, b.absorption), ("fission", a.fission, b.fission),
                ("nu_fission", a.nu_fission, b.nu_fission),
            ] {
                assert_eq!(x.to_bits(), y.to_bits(), "{label}: {what} at {e:e} eV: {x} vs {y}");
            }
        }
        if label == "U-235" {
            for (seed, e_in) in (0..200u64).zip([0.0253, 1.0, 1.0e3, 1.0e6].into_iter().cycle()) {
                let (mut s1, mut s2) = (seed, seed);
                let (x, y) = (full.sample_fission_energy(e_in, &mut s1), thin.sample_fission_energy(e_in, &mut s2));
                assert_eq!(x.to_bits(), y.to_bits(), "U-235 fission spectrum, seed {seed}");
            }
        }
    }
}

#[test]
fn the_2d_reduction_keeps_the_pebble_inventory_ratios() {
    // 8335 x 0.0455 / 2.5 = 151.7 -> 152 rods.
    assert_eq!(model::particles_2d(), 152);
    assert!((model::particle_r() - 0.0455).abs() < 1e-12);
    let p = model::half_pitch();
    let area_fraction = std::f64::consts::PI * model::PEBBLE_R.r_powi(2) / (2.0 * p).r_powi(2);
    assert!((area_fraction - model::CORE_FILLING_FRACTION).abs() < 1e-12);
}

/// The assembled geometry, queried as the solver queries it.
#[test]
fn the_assembled_geometry_puts_every_material_where_it_belongs() {
    let c = model::particle_centres(model::LAYOUT_SEED);
    let g = model::build_geometry(&c);
    let dir = Direction::new(1.0, 0.0, 0.0);
    let at = |x: f64, y: f64| g.locate(Position::new(x, y, 0.0), dir, SurfaceToken::NONE).map(|p| p.material);
    let rp = model::particle_r();
    for (i, &(x, y)) in c.iter().enumerate() {
        assert!((x * x + y * y).sqrt() + rp < model::FUEL_ZONE_R, "particle {i} crosses the fuel-zone edge");
        for &(a, b) in &c[i + 1..] {
            assert!(((x - a).r_powi(2) + (y - b).r_powi(2)).sqrt() > 2.0 * rp, "particles overlap");
        }
        // Probe the middle of every layer along +x.
        let mids = [
            (0.0, model::MAT_KERNEL),
            ((model::KERNEL_R + model::buffer_r()) / 2.0, model::MAT_BUFFER),
            ((model::buffer_r() + model::ipyc_r()) / 2.0, model::MAT_IPYC),
            ((model::ipyc_r() + model::sic_r()) / 2.0, model::MAT_SIC),
            ((model::sic_r() + rp) / 2.0, model::MAT_OPYC),
        ];
        for (dx, m) in mids {
            assert_eq!(at(x + dx, y), Some(Some(m)), "particle {i}, expected {}", model::MATERIAL_NAMES[m]);
        }
    }
    assert_eq!(at(0.0, (model::FUEL_ZONE_R + model::PEBBLE_R) / 2.0), Some(Some(model::MAT_SHELL)));
    let p = model::half_pitch();
    assert_eq!(at(0.98 * p, 0.98 * p), Some(None), "cell corner is helium (void)");
    assert!(at(1.01 * p, 0.0).is_none(), "outside the reflective cell is outside the geometry");
}

/// The Godiva sphere as the solver sees it: HEU inside, no cell outside.
#[test]
fn the_godiva_geometry_is_one_uranium_sphere() {
    let g = godiva::model::build_geometry();
    let r = godiva::model::RADIUS_CM;
    let dir = Direction::new(0.0, 0.0, 1.0);
    let at = |x: f64, y: f64, z: f64| g.locate(Position::new(x, y, z), dir, SurfaceToken::NONE).map(|p| p.material);
    for p in [(0.0, 0.0, 0.0), (0.99 * r, 0.0, 0.0), (0.0, -0.99 * r, 0.0), (0.5 * r, 0.5 * r, 0.5 * r)] {
        assert_eq!(at(p.0, p.1, p.2), Some(Some(0)), "inside at {p:?}");
    }
    assert!(at(1.01 * r, 0.0, 0.0).is_none());
    assert_eq!(r, outram_mc_libs::vv::godiva::RADIUS_CM);
}

#[test]
fn the_headless_run_is_deterministic() {
    assert_eq!(triso::sim::headless_csv(physics(), 12, 7), triso::sim::headless_csv(physics(), 12, 7));
}

/// A harness check, NOT validation: every history must end in a physical
/// absorption inside the cell, with no lost or truncated tracks.
#[test]
fn every_history_ends_physically_inside_the_cell() {
    let phys = physics();
    let mut chain = triso::sim::Chain::new(3);
    let p = model::half_pitch();
    for _ in 0..60 {
        let h = chain.run_next(phys);
        assert!(matches!(history::outcome_name(h.outcome), "fission" | "capture"), "history {}: {}", h.index, history::outcome_name(h.outcome));
        assert_eq!(h.track.dropped_states, 0, "history {} truncated", h.index);
        assert!(h.track.states.len() >= 2);
        for s in &h.track.states {
            assert!(s.r.x.abs() <= p + 1e-9 && s.r.y.abs() <= p + 1e-9, "history {} left the cell", h.index);
            assert!(s.energy > 1.0e-6 && s.energy < 3.0e7, "history {}: energy {} eV", h.index, s.energy);
        }
        assert!(h.path_cm.is_finite() && h.path_cm > 0.0);
    }
}

/// The same for Godiva, where leaking out is the commonest ending.
#[test]
fn every_godiva_history_leaks_or_is_absorbed_inside_the_sphere() {
    let phys = godiva_physics();
    let mut chain = godiva::sim::Chain::new(5);
    let r = godiva::model::RADIUS_CM;
    let mut leaks = 0;
    for _ in 0..80 {
        let h = chain.run_next(phys);
        assert!(matches!(h.outcome, Some(TrackEvent::Leak | TrackEvent::Fission | TrackEvent::Absorption)), "history {}: {}", h.index, history::outcome_name(h.outcome));
        leaks += (h.outcome == Some(TrackEvent::Leak)) as usize;
        assert_eq!(h.track.dropped_states, 0);
        for s in &h.track.states {
            assert!((s.r.x * s.r.x + s.r.y * s.r.y + s.r.z * s.r.z).sqrt() <= r * (1.0 + 1e-9), "history {} outside", h.index);
        }
    }
    assert!(leaks > 10, "a bare fast sphere leaks about half its neutrons; got {leaks} of 80");
}

/// The live k_eff run, at a tiny size: every generation's counts close
/// (leaked + captured + fissioned = followed), the console has one line
/// per generation, and a generation crosses the worker boundary bit for bit.
#[test]
fn the_godiva_keff_run_counts_close_and_crosses_the_worker_boundary() {
    let phys = godiva_physics();
    let cfg = keff::KeffConfig { n_particles: 200, n_inactive: 3, n_active: 4, seed: 9, point_source: true, want_sites: true };
    let mut k = godiva::sim::Keff::new(cfg);
    assert_eq!(k.initial_sites().len(), 200);
    let mut n = 0;
    while let Some(g) = k.step(phys) {
        let c = g.report.counts;
        assert_eq!(c.leaked + c.captured + c.fissions(), c.tracked);
        assert!(c.tracked >= 200);
        assert_eq!(engine::decode_generation(&engine::encode_generation(&g)).unwrap(), g);
        n += 1;
    }
    assert_eq!(n, 7);
    let mut lines = Vec::new();
    godiva::sim::headless_keff(phys, cfg, |l| lines.push(l.to_string()));
    assert_eq!(lines.iter().filter(|l| l.contains("/1 ")).count(), 7);
}

/// What crosses from the physics worker to the page is a flattened
/// history; it must come back bit for bit, or the browser would draw a
/// different neutron from the one transported.
#[test]
fn a_history_crosses_the_worker_boundary_bit_for_bit() {
    let mut chain = triso::sim::Chain::new(11);
    for _ in 0..8 {
        let h = chain.run_next(physics());
        let back = engine::decode_history(&engine::encode_history(&h)).expect("decode");
        assert_eq!(history::csv_row(&back), history::csv_row(&h));
        assert_eq!(back.track.states.len(), h.track.states.len());
        for (a, b) in back.track.states.iter().zip(&h.track.states) {
            assert_eq!(a, b, "history {}: a track state changed in transit", h.index);
        }
    }
}

/// The GUI's engine thread (the native twin of the browser's worker) must
/// produce exactly the headless sequence for the GUI's chain seed.
#[test]
fn the_engine_thread_runs_the_headless_sequence() {
    let link = crate::web_demo::link::start_native(engine::McEngine::default(), || {});
    link.send(engine::Request::Load { id: 7, rung: Rung::Triso, tier: Tier::Exact });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(900);
    let (mut ready, mut got) = (false, Vec::new());
    while got.len() < 5 {
        assert!(std::time::Instant::now() < deadline, "engine thread timed out");
        for e in link.drain() {
            match e {
                engine::Event::Ready { id } => {
                    assert_eq!(id, 7);
                    ready = true;
                    link.send(engine::Request::Run { n: 5, animate: true });
                }
                engine::Event::History { h, .. } => got.push(history::csv_row(&h)),
                engine::Event::Error(m) => panic!("engine: {m}"),
                _ => {}
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    assert!(ready);
    let want: Vec<String> =
        triso::sim::headless_csv(physics(), 5, engine::CHAIN_SEED).lines().skip(1).map(str::to_owned).collect();
    assert_eq!(got, want);

    // gh:#818: the engine keeps what it processed. Loading the rung again
    // (a rung switched away from and back to) reads every product back,
    // says so per job, and gives the same histories; the cache reports
    // its size and clears.
    let wait = |want_ready: u32| {
        let (mut sources, mut info, mut ready) = (Vec::new(), Vec::new(), false);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(900);
        while !(ready && !info.is_empty()) {
            assert!(std::time::Instant::now() < deadline, "engine thread timed out");
            for e in link.drain() {
                match e {
                    engine::Event::JobDone { source, .. } => sources.push(source),
                    engine::Event::Ready { id } if id == want_ready => {
                        ready = true;
                        link.send(engine::Request::CacheInfo);
                    }
                    engine::Event::CacheInfo(m) => info.push(m),
                    engine::Event::Error(m) => panic!("engine: {m}"),
                    _ => {}
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        (sources, info)
    };
    link.send(engine::Request::Load { id: 8, rung: Rung::Triso, tier: Tier::Exact });
    let (sources, info) = wait(8);
    assert_eq!(sources.len(), triso::model::JOBS.len());
    assert!(sources.iter().all(|s| matches!(s, crate::web_demo::data_cache::Source::Cached { .. })), "{sources:?}");
    assert!(info[0].contains("11 products"), "{info:?}");
    link.send(engine::Request::Run { n: 5, animate: true });
    let mut again = Vec::new();
    while again.len() < 5 {
        for e in link.drain() {
            if let engine::Event::History { h, .. } = e {
                again.push(history::csv_row(&h));
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(again, want, "the same neutrons from the cached data");
    link.send(engine::Request::ClearCache);
    let mut cleared = None;
    while cleared.is_none() {
        for e in link.drain() {
            if let engine::Event::CacheInfo(m) = e {
                cleared = Some(m);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(cleared.is_some_and(|m| m.ends_with("empty")));
}

/// The TRISO rung's first 40 Watch histories, pinned. **Re-blessed
/// 2026-10-18 (gh:#932)** when the demos' data moved onto njoy's
/// `process`: histories 0-3 are unchanged, history 4 differs first in
/// the 7th figure of its path length (2.708451e2 -> 2.708453e2 cm) and
/// the chain decorrelates after it (40 histories: 25 fissions / 15
/// captures before, 20 / 20 after, a draw, not a trend).
///
/// **Re-blessed 2026-11-20 (gh:#1675)** on develop `c49f5cf29`. Before
/// re-blessing, a path-limited `git bisect` found every commit after
/// 8349661c1 that moves this trace. Each round used the previous
/// mover's own trace as its reference, and the trace at `c49f5cf29` is
/// byte-identical to the last mover's. There are eight movers, each a
/// deliberate default change or bug fix, so none is a regression:
/// 1. 457d4ca97: resonance scattering defaults to RVS (gh:#1311).
/// 2. 3134e3134 with its build fix f95d4473e: OpenMC collision kernels
///    are the default (gh:#1386).
/// 3. bb2199b66: OpenMC reaction order and single-cutoff level pick
///    (gh:#1444 a, b).
/// 4. 2235f1684: per-collision fission banking (gh:#1444 c).
/// 5. 0b502abfa: fixed-source seeding fix (gh:#1493).
/// 6. 323d5d933: OpenMC cross-section details (gh:#463).
/// 7. d6bcf02d3: per-material S(a,b) in this rung's loader (gh:#1581).
/// 8. 44f365bbb: S(a,b) emission tables on THERMR's calcem grid
///    (gh:#550).
///
/// History 0 went from capture after 85 scatters and 517.4 cm to
/// capture after 82 scatters and 367.5 cm. Over the 40 histories,
/// fissions / captures went from 20 / 20 to 22 / 18, a draw, not a
/// trend. This trace pins reproducibility only; it is not a physics
/// check.
#[test]
fn the_headless_run_matches_the_committed_fixture() {
    let got = triso::sim::headless_csv(physics(), FIXTURE_N, FIXTURE_SEED);
    if std::env::var_os("TRISO_BLESS").is_some() {
        std::fs::write(FIXTURE, &got).unwrap();
    }
    let want = std::fs::read_to_string(FIXTURE).expect("fixture missing: rerun with TRISO_BLESS=1");
    assert_eq!(got, want, "headless trace differs from {FIXTURE}");
}

/// Every rung's lesson link is a page path, names are unique, and the URL
/// parser falls back sensibly: no rung opens the first (Godiva); a rung
/// without Run ignores mode=run.
/// The four benchmark rungs of the acceptance tests (gh:#859) each have
/// what the issue asks of them, read from the rung table: a Run k_eff
/// with one default run size for all four (the campaign's own for three;
/// smaller than the campaign's for the lattice), a recorded result read
/// from the five-route summary (route 4), and recorded tracks.
#[test]
fn the_benchmark_rungs_have_run_settings_a_record_and_shipped_tracks() {
    let expect = [
        (Rung::Godiva, (5000, 40, 120), 0.99994),
        (Rung::Jemima, (5000, 40, 120), 0.996092),
        // HST-009 carries U-236 since 2026-10-15 (was 1.001975 without it).
        (Rung::Hst009, (5000, 40, 120), 1.000717),
        // The lattice's record is the full 35-nuclide model since
        // 2026-10-15 (`lct008-full`); it was the 11-nuclide 1.002360,
        // then 1.001178 (8 seeds at [250 + 400]).
        (Rung::Lct008, (5000, 40, 120), 1.001594),
    ];
    // Benchmark k and its uncertainty, as tabulated by Mosteller
    // (LA-UR-10-06230) and van der Marck (Nucl. Data Sheets 107, 2006);
    // the maintainer confirmed the four rows on 2026-10-13.
    let bench = [(1.0000, 0.0010), (1.0000, 0.0030), (0.9990, 0.0043), (1.0007, 0.0016)];
    // Active histories of one seed of each record (the lattice's is the
    // campaign's 10 000 x 400, not the demo's default).
    let recorded = [600_000.0, 600_000.0, 600_000.0, 1_200_000.0];
    for (((r, size, k), hist), bench) in expect.into_iter().zip(recorded).zip(bench) {
        let name = r.info().name;
        let d = r.run_default().unwrap_or_else(|| panic!("{name} has no Run k_eff"));
        assert_eq!((d.n_particles, d.n_inactive, d.n_active, d.seed), (size.0, size.1, size.2, 1), "{name}");
        let rec = r.reference().unwrap_or_else(|| panic!("{name} has no record"));
        assert_eq!(rec.k, k, "{name}");
        assert!(rec.sem > 0.0 && rec.sem < 0.0005 && rec.sd_one_run > rec.sem && rec.experiment.is_some(), "{name}");
        assert_eq!(rec.histories_one_run, hist, "{name}");
        assert_eq!(rec.experiment.map(|(_, k, s)| (k, s)), Some(bench), "{name}");
        assert!(r.recorded_tracks().is_some(), "{name} ships no tracks");
        // Every recorded k is printed with its date and commit, and the
        // reference code with its versions (maintainer, 2026-10-14).
        let code = rec.code.unwrap_or_else(|| panic!("{name} has no reference-code k"));
        // Every route 1 was re-run on 2026-10-15 on OpenMC d7d3284a1.
        let (openmc, date) = ("OpenMC 0.16.1-dev25 at d7d3284a1 on NJOY2016 2016.79 at ac5adf5", "2026-10-15");
        assert!(code.label.contains(openmc) && code.label.ends_with(date), "{name}: {}", code.label);
        assert!(code.sem > 0.0 && (code.k - rec.k).abs() < 0.002, "{name}: {} against {}", code.k, rec.k);
        assert!(rec.label.contains(" at ") && rec.label.contains("2026-"), "{name}: {}", rec.label);
        let lines = keff::reference_lines(&rec);
        assert_eq!(lines.len(), 3, "{name}: experiment, reference code, record");
        assert!(lines[1].starts_with(" Reference code (OpenMC") && lines[2].starts_with(" Recorded ("), "{name}: {lines:?}");
    }
    assert!(keff::build_stamp().starts_with("computed now by"));
    for r in [beds::LATTICE_K, beds::RANDOM_K] {
        assert!(r.record.starts_with("2026-10-0") && r.record.contains(", at "), "{}", r.record);
    }
    // Route 1 is OpenMC on NJOY2016 data, the acceptance check's reference.
    assert_eq!(keff::five_route_row("jemima", "route1"), Some((0.995938, 0.000174, 0.000987)));
    assert!(keff::five_route_row("jemima", "route9").is_none());
    // A rung that ships none says so.
    assert!(Rung::Triso.recorded_tracks().is_none());
}

/// **Methodology.** Each benchmark rung's shipped track file
/// (`<rung>/data/tracks.bin.z`, written by `--bake-tracks`) is decoded as
/// the page decodes it. Every history must be complete (born first,
/// ended by a leak, a capture or a fission, nothing dropped) and every
/// state must lie inside the rung's model: within the radius the picture
/// is fitted to. ~~**Result (2026-10-13):** Godiva 60 histories / 266
/// states, Jemima 60 / 1381, HEU-SOL-THERM-009 30 / 3185, LEU-COMP-THERM-008
/// 30 / 2288; none truncated, none outside.~~ **Result (2026-10-16, files
/// re-baked at NJOY's 0.001, gh:#876 item 3; the 2026-10-13 files were at
/// 0.01):** Godiva 60 / 266 and Jemima 60 / 1381, unchanged (fast systems:
/// the same histories); HEU-SOL-THERM-009 30 / 2618 and
/// LEU-COMP-THERM-008 30 / 2112, changed by the thermal data; none
/// truncated, none outside.
#[test]
fn the_shipped_tracks_are_whole_histories_inside_their_geometry() {
    for (r, n, states) in [(Rung::Godiva, 60, 266), (Rung::Jemima, 60, 1381), (Rung::Hst009, 30, 2618), (Rung::Lct008, 30, 2112)] {
        let name = r.info().name;
        let h = recorded::histories(r.recorded_tracks().expect("shipped")).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!((h.len(), h.iter().map(|x| x.track.states.len()).sum::<usize>()), (n, states), "{name}");
        let reach = r.half_extent() * (1.0 + 1e-9);
        for x in &h {
            assert_eq!(x.track.states[0].event, TrackEvent::Born, "{name} history {}", x.index);
            assert!(matches!(x.outcome, Some(TrackEvent::Leak | TrackEvent::Fission | TrackEvent::Absorption)), "{name} history {}: {}", x.index, history::outcome_name(x.outcome));
            assert_eq!(x.track.dropped_states, 0);
            for s in &x.track.states {
                assert!((s.r.x * s.r.x + s.r.y * s.r.y).sqrt() <= reach, "{name} history {}: a state at {:?}", x.index, s.r);
            }
        }
    }
    assert!(recorded::histories(b"not a track file").is_err());
    assert!(recorded::path(Rung::Jemima).ends_with("jemima/data/tracks.bin.z"));
}

fn jemima_loaded() -> &'static std::sync::Mutex<csgrung::Loaded<jemima::Jemima>> {
    static P: OnceLock<std::sync::Mutex<csgrung::Loaded<jemima::Jemima>>> = OnceLock::new();
    P.get_or_init(|| match load_native(Rung::Jemima, Tier::Exact, |_, _| {}).expect("load nuclear data") {
        Loaded::Jemima(l) => std::sync::Mutex::new(l),
        _ => unreachable!(),
    })
}

/// **Methodology.** The Jemima rung on the three ENDF/B-VIII.0 tapes at
/// NJOY's tolerance 0.001 (~~the Watch tier, 0.01~~ until 2026-10-16, gh:#876 item 3): 60 chained histories, then a small
/// headless Run k_eff (1000 neutrons × [10 inactive + 30 active], seed 1)
/// run twice. Checks: every history ends in a leak, a capture or a
/// fission inside the assembly; the run is deterministic; its `k` is
/// within 5 of its own σ of the recorded 0.99609 ± 0.00027 (five-route
/// route 4). A harness check, not V&V: a different run size from the
/// record. ~~**Result (2026-10-13):** `k = 0.99954 ± 0.00851`, 0.4 σ from
/// the record; all 60 histories ended properly.~~ (at 0.01) **Result
/// (2026-10-16, at 0.001, gh:#876 item 3):** `k = 0.98455 ± 0.00799`,
/// −1.4 σ from the record; all 60 histories ended properly. The two
/// results are three days of code apart, so their difference is not
/// the tolerance's alone.
#[test]
fn the_jemima_rung_tracks_neutrons_and_runs_its_keff() {
    use rungs::{LoadedRung as _, McRung as _};
    let mut l = jemima_loaded().lock().expect("lock");
    let m = outram_mc_libs::vv::jemima::R_OUT * (1.0 + 1e-9);
    for _ in 0..60 {
        let h = l.run_next();
        assert!(matches!(h.outcome, Some(TrackEvent::Leak | TrackEvent::Fission | TrackEvent::Absorption)), "history {}: {}", h.index, history::outcome_name(h.outcome));
        assert_eq!(h.track.dropped_states, 0);
        for s in &h.track.states {
            assert!((s.r.x * s.r.x + s.r.y * s.r.y).sqrt() <= m && s.r.z >= -1e-9 && s.r.z <= outram_mc_libs::vv::jemima::Z_TOP * (1.0 + 1e-9));
        }
    }
    let cfg = keff::KeffConfig { n_particles: 1000, n_inactive: 10, n_active: 30, seed: 1, point_source: false, want_sites: false };
    let mut a = Vec::new();
    let (k, s) = csgrung::headless_keff::<jemima::Jemima>(&l.phys, cfg, |line| a.push(line.to_string()));
    let mut b = Vec::new();
    assert_eq!(csgrung::headless_keff::<jemima::Jemima>(&l.phys, cfg, |line| b.push(line.to_string())), (k, s));
    assert_eq!(a, b);
    assert_eq!(a.len(), 2 + 40 + 5, "header, 40 generations, blank, result, experiment, reference code, record");
    assert!(a[a.len() - 4].contains("computed now by"), "{}", a[a.len() - 4]);
    assert!(a[a.len() - 2].contains("OpenMC 0.16.1-dev25 at d7d3284a1 on NJOY2016 2016.79 at ac5adf5"), "{}", a[a.len() - 2]);
    assert!(a[a.len() - 1].contains("2026-10-15, at 093d8590cf"), "{}", a[a.len() - 1]);
    eprintln!("  Jemima 1000 x [10 + 30], tolerance 0.001: k = {k:.5} +/- {s:.5}");
    let rec = jemima::Jemima::reference().expect("record");
    assert!((k - rec.k).abs() < 5.0 * s, "k {k} +/- {s} against the record {}", rec.k);

    // Watch, whole generations: a point source at the middle of the core
    // that spreads, one generation per step.
    let w = keff::KeffConfig { n_particles: 300, n_inactive: 2, n_active: 2, seed: 1, point_source: true, want_sites: true };
    let sites = l.keff_start(w).expect("start");
    assert_eq!(sites.len(), 300);
    assert!(sites.iter().all(|p| p[0].abs() < 0.06 && p[1].abs() < 0.06), "a point source starts at the axis");
    // While the run is going, a drawn neutron is born at a site of its
    // source: here, still the point at the axis.
    let h = l.run_next();
    assert!(h.from_fission && h.birth.x.abs() < 0.06 && h.birth.y.abs() < 0.06, "born at {:?}", h.birth);
    let mut n = 0;
    while let Some(g) = l.keff_step() {
        assert_eq!((g.report.index, g.report.active), (n, n >= 2));
        // 5 x 5 x 5 mesh on the whole assembly: at most log2(125) bits.
        let h = g.report.entropy.expect("a CSG generation reports its source entropy");
        assert!(h > 0.0 && h <= 125f64.r_log2() && g.report.counts.tracked == 0, "entropy {h}");
        assert!((g.report.production - g.report.k * 300.0).abs() < 1e-9);
        assert!(g.sites.iter().any(|p| p[0].abs() > 1.0), "the source has spread");
        n += 1;
    }
    assert_eq!(n, 4);
    assert!(l.keff_finished());
}

/// **Methodology.** The `natu` rung (Lesson 1's random walk, gh:#911) on
/// Jemima's three ENDF/B-VIII.0 tapes at NJOY's tolerance 0.001 (~~the Watch tier, 0.01~~ until 2026-10-16, gh:#876 item 3):
/// 200 chained histories, each checked to start with a birth, end in a
/// leak, a capture or a fission, drop no state, and stay inside the bar.
/// The outcome counts are printed and recorded here, since the lesson
/// describes what a reader will see. A harness check, not V&V.
/// **Result (2026-10-16, on e9b5484608 plus the change adding the rung):**
/// all 200 ended properly inside the bar: 140 leaked, 37 captured, 23
/// fission; 12.6 scatters per history on average.
/// Re-run at 0.001 the same day (gh:#876 item 3): the same counts.
#[test]
fn the_natural_uranium_rung_tracks_whole_histories_inside_the_bar() {
    use rungs::LoadedRung as _;
    let mut l = match load_native(Rung::Natu, Tier::Exact, |_, _| {}).expect("load nuclear data") {
        Loaded::Natu(l) => l,
        _ => unreachable!(),
    };
    let eps = 1e-9;
    let (mut leak, mut capture, mut fission, mut scatters) = (0, 0, 0, 0);
    for _ in 0..200 {
        let h = l.run_next();
        assert_eq!(h.track.states[0].event, TrackEvent::Born);
        assert_eq!(h.track.dropped_states, 0);
        match h.outcome {
            Some(TrackEvent::Leak) => leak += 1,
            Some(TrackEvent::Absorption) => capture += 1,
            Some(TrackEvent::Fission) => fission += 1,
            o => panic!("history {}: {}", h.index, history::outcome_name(o)),
        }
        scatters += h.scatters;
        for s in &h.track.states {
            assert!(s.r.x.abs() <= natu::HALF_X + eps && s.r.y.abs() <= natu::HALF_YZ + eps && s.r.z.abs() <= natu::HALF_YZ + eps, "history {}: {:?}", h.index, s.r);
        }
    }
    eprintln!("  natu, 200 histories: {leak} leaked, {capture} captured, {fission} fission; {:.1} scatters per history", scatters as f64 / 200.0);
    assert!(leak > 0 && capture > 0 && fission > 0, "all three ends seen");
}

/// **Methodology.** The HEU-SOL-THERM-009 rung on its eleven tapes and
/// the H in H₂O law at the Watch tier: 40 chained histories, then a
/// small headless Run k_eff (1000 × [10 + 30], seed 1). Checks: every
/// history ends properly inside the reflector, most reach thermal
/// energies (a water-moderated solution), and `k` is within 5 of its own
/// σ of the recorded 1.00198 ± 0.00032 (five-route route 4). A harness
/// check, not V&V. Needs about half a minute of data processing, so it
/// is opt-in like the crate's other twelve-tape tests. **Result
/// (2026-10-13):** 28 of 40 histories thermalised; `k = 1.01633 ±
/// 0.00718`, 2.0 σ above the record, in 1.7 s (i9-13900K, one thread,
/// 62 GB, Linux, CPU only, shared machine). The σ of 30 correlated
/// generations is itself uncertain, and 10 inactive generations from a
/// uniform start is short; this run cannot say more than "no gross
/// error".
#[test]
#[ignore = "processes 11 ENDF tapes and LEAPR (minutes); run with --ignored"]
fn the_hst009_rung_thermalises_and_runs_its_keff() {
    use rungs::{LoadedRung as _, McRung as _};
    let Loaded::Hst009(mut l) = load_native(Rung::Hst009, Tier::Exact, |_, _| {}).expect("load") else { unreachable!() };
    let reach = outram_mc_libs::vv::hst009::R_REFLECTOR * (1.0 + 1e-9);
    let (mut thermal, n) = (0, 40);
    for _ in 0..n {
        let h = l.run_next();
        assert!(matches!(h.outcome, Some(TrackEvent::Leak | TrackEvent::Fission | TrackEvent::Absorption)), "history {}: {}", h.index, history::outcome_name(h.outcome));
        assert_eq!(h.track.dropped_states, 0);
        assert!(h.track.states.iter().all(|s| (s.r.x * s.r.x + s.r.y * s.r.y + s.r.z * s.r.z).sqrt() <= reach));
        thermal += h.thermalised() as usize;
    }
    assert!(thermal > n / 2, "a water-moderated solution thermalises most neutrons; got {thermal} of {n}");
    let cfg = keff::KeffConfig { n_particles: 1000, n_inactive: 10, n_active: 30, seed: 1, point_source: false, want_sites: false };
    let t = std::time::Instant::now();
    let (k, s) = csgrung::headless_keff::<hst009::Hst009>(&l.phys, cfg, |_| {});
    eprintln!("  HST-009 1000 x [10 + 30], Watch tier: k = {k:.5} +/- {s:.5} in {:.1} s (one thread); {thermal} of {n} histories thermalised", t.elapsed().as_secs_f64());
    let rec = hst009::Hst009::reference().expect("record");
    assert!((k - rec.k).abs() < 5.0 * s, "k {k} +/- {s} against the record {}", rec.k);
}

/// **Methodology.** The LEU-COMP-THERM-008 rung's whole-core Run k_eff
/// (gh:#859) on its twelve tapes at the Watch tier, 2000 neutrons ×
/// [20 inactive + 20 active], seed 1, through the page's own
/// `keff_start` / `keff_step`. Checks: one generation per step, in
/// order, to the last; the source box finds fuel; `k` is within 0.03 of
/// the recorded 1.00236 (a harness check with a loose bound: 20
/// inactive generations do not converge this core's source, which is
/// why the rung's default is the campaign's 250). **Result
/// (2026-10-13):** `k = 0.99288 ± 0.00443`, 40 generations in 3.6 s
/// (i9-13900K, one thread, 62 GB, Linux, CPU only, shared machine):
/// 950 pcm below the record, 2.1 of its own σ, as expected of a source
/// that has not converged.
#[test]
#[ignore = "processes 35 ENDF tapes and LEAPR (minutes); run with --ignored"]
fn the_lct008_rung_runs_a_whole_core_keff() {
    use rungs::LoadedRung as _;
    let Loaded::Lct008(mut l) = load_native(Rung::Lct008, Tier::Exact, |_, _| {}).expect("load") else { unreachable!() };
    let cfg = keff::KeffConfig { n_particles: 2000, n_inactive: 20, n_active: 20, seed: 1, point_source: false, want_sites: true };
    let sites = l.keff_start(cfg).expect("start");
    assert_eq!(sites.len(), 2000);
    assert!(sites.iter().all(|p| ((p[0] * p[0] + p[1] * p[1]) as f64).sqrt() < lct008::model::R_CORE));
    let t = std::time::Instant::now();
    let (mut n, mut last) = (0, None);
    while let Some(g) = l.keff_step() {
        assert_eq!(g.report.index, n);
        n += 1;
        last = g.report.k_mean;
    }
    let (k, s) = last.expect("active generations");
    eprintln!("  LCT-008 core 2000 x [20 + 20], Watch tier: k = {k:.5} +/- {s:.5} in {:.1} s (one thread)", t.elapsed().as_secs_f64());
    assert_eq!(n, 40);
    assert!(l.keff_finished());
    assert!((k - 1.00236).abs() < 0.03, "k {k} +/- {s}");
    let b = lct008::source_box();
    assert!(b.lower.z < b.upper.z && b.upper.x == lct008::model::R_CORE);
    let (k2, _) = lct008::headless_keff(&l.phys, keff::KeffConfig { n_particles: 500, n_inactive: 2, n_active: 3, seed: 1, point_source: false, want_sites: false }, |_| {});
    assert!(k2 > 0.8 && k2 < 1.2, "{k2}");
}

#[test]
fn the_rung_table_parses_urls() {
    let q = |s: &[(&str, &str)]| s.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect::<Vec<_>>();
    assert_eq!(rungs::start(&q(&[])), (Rung::Godiva, rungs::Mode::Watch));
    assert_eq!(rungs::start(&q(&[("rung", "godiva"), ("mode", "run")])), (Rung::Godiva, rungs::Mode::Run));
    assert_eq!(rungs::start(&q(&[("rung", "triso"), ("mode", "run")])), (Rung::Triso, rungs::Mode::Watch));
    for (i, r) in table::ALL.iter().enumerate() {
        let info = r.info();
        assert!(info.lesson.ends_with(".html") && !info.lesson.starts_with('/'));
        assert!(table::ALL[..i].iter().all(|o| o.info().name != info.name), "duplicate rung name {}", info.name);
        // A code walk's demo (gh:#785) may need no nuclear data.
        assert!((!r.jobs().is_empty() || r.walk_demo().is_some()) && r.half_extent() > 0.0);
    }
}
