//! **The Monte Carlo demo** — one app, a rung of the tutorial ladder at a time
//! (gh:#520, #521), from `godiva`, a bare uranium sphere (Watch, and a true
//! Run k_eff), to `htr10`; the full list is the rung table below. Every
//! neutron is transported by outram-mc-libs on real ENDF/B-VIII.0 data that
//! the workspace's own NJOY port processes — in the browser, on the reader's
//! machine. Single-threaded; runs natively and in the browser
//! (`web/monte_carlo/`, published at `demos/monte-carlo/`).
//!
//! ```text
//! cargo run -p dhoby-ghaut --example monte_carlo_web --release [-- --rung godiva --mode run]
//! cargo run -p dhoby-ghaut --example monte_carlo_web --release -- --headless [n] [seed]
//! cargo run -p dhoby-ghaut --example monte_carlo_web --release -- --headless-godiva [n] [seed]
//! cargo run -p dhoby-ghaut --example monte_carlo_web --release -- --headless-keff [n inactive active seed] [--loose]
//! cargo run -p dhoby-ghaut --example monte_carlo_web --release -- --render-geometry <dir>
//! cargo run -p dhoby-ghaut --example monte_carlo_web --release -- --prepare-web-data <dir>
//! cargo test -p dhoby-ghaut --example monte_carlo_web --release
//! ```
//!
//! In the browser, `?rung=<name>&mode=<watch|run>` picks the rung and mode
//! ([`rungs`] is the table). Until 2026-10-04 this was the TRISO-only
//! `triso_pebble_web` example; its URL `demos/triso-pebble/` now redirects
//! here with `?rung=triso`.
//!
//! - **Physics**: outram-mc-libs continuous-energy transport, unmodified. The
//!   Godiva Run k_eff is its single-thread reference power iteration
//!   (`PowerIteration`, which `run_keff` is), stepped one generation at a time.
//! - **Data**: ENDF/B-VIII.0 tapes from `reference-data/endf/`, covariances
//!   stripped, processed by the workspace's NJOY port (RECONR + BROADR). The
//!   tier per rung and mode is in [`app`] (`tier_for`): Godiva Run k_eff at
//!   NJOY's tolerance 0.001, so its `k` is comparable with the record; the
//!   Watch modes at 0.01.
//! - **Geometry**: TRISO from IAEA-TECDOC-1382 ([`triso::model`]); Godiva from
//!   `outram_mc_libs::vv::godiva` ([`godiva::model`]).
//!
//! Education and research only, per the workspace `RESPONSIBLE_USE.md`. Not for
//! reactor operation, licensing, or safety-critical decisions.

#[cfg(not(target_os = "android"))]
mod anim;
#[cfg(not(target_os = "android"))]
mod app;
#[cfg(not(target_os = "android"))]
mod engine;
#[cfg(not(target_os = "android"))]
mod history;
#[cfg(not(target_os = "android"))]
mod keff;
#[cfg(not(target_os = "android"))]
#[macro_use]
mod rungs;
#[cfg(not(target_os = "android"))]
mod tapes;
#[cfg(not(target_os = "android"))]
mod xs;
#[cfg(not(target_os = "android"))]
mod raster;
#[cfg(not(target_os = "android"))]
mod sweep;
// The `htr10` rung's liberties toggle, lattice bed against DEM bed (gh:#787),
// and the baked beds it draws (shared with `dem_web`).
#[cfg(not(target_os = "android"))]
mod beds;
#[cfg(not(target_os = "android"))]
#[path = "../common/htr10_beds.rs"]
mod htr10_beds;
#[cfg(not(target_os = "android"))]
mod walkdemo;

// THE RUNG TABLE, in ladder order: one line per rung, `module: MarkerType`,
// for `examples/monte_carlo_web/<module>/mod.rs` (see `rungs.rs`). Adding a
// rung is adding its directory and one line here; nothing else changes.
#[cfg(not(target_os = "android"))]
rung_table! {
    godiva: Godiva,
    ugraphite: Ugraphite,
    lumped: Lumped,
    lct008: Lct008,
    triso: Triso,
    dhshort: DhShort,
    packing: Packing,
    htr10: Htr10,
}

/// Android stub: windowing GUIs are out of scope on Termux (the workspace
/// example rule — a blanked file gives "main function not found").
#[cfg(target_os = "android")]
fn main() {
    eprintln!("monte_carlo_web is a windowing GUI and is not built for Android.");
}

/// Read a reference tape from `reference-data/endf/` and strip its covariances
/// — the same bytes the browser gets once it has inflated the download.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn native_tape(tape: &str) -> Result<Vec<u8>, String> {
    let path = njoy_outram_park_fork::reference_data::reference_endf(tape)
        .ok_or_else(|| format!("reference tape {tape} is not present"))?;
    let raw = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(tapes::strip_covariances(&raw))
}

/// Process a rung's tapes natively, in order. `report` sees each job's label
/// and seconds.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn load_native(rung: table::Rung, tier: engine::Tier, mut report: impl FnMut(&str, f64)) -> Result<table::Loaded, String> {
    let jobs = rung.jobs_for(rung.tier(tier));
    let mut post = |e: engine::Event| {
        if let engine::Event::JobDone { index, secs, .. } = e {
            report(jobs[index].0, secs);
        }
    };
    engine::native_load(0, rung, tier, &mut post)
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn main() -> Result<(), String> {
    use engine::Tier;
    use rungs::Mode;
    use table::{Loaded, Rung};
    let args: Vec<String> = std::env::args().skip(1).collect();
    let arg = |i: usize| args.get(i).map(String::as_str);
    let num = |i: usize, d: u64| -> Result<u64, String> { arg(i).map_or(Ok(d), str::parse).map_err(|e| format!("argument {i}: {e}")) };
    let timing = |label: &str, s: f64| eprintln!("  {label:<16} {s:6.1} s");
    match arg(0) {
        Some("--headless") => {
            let (n, seed) = (num(1, 200)? as usize, num(2, 1)?);
            eprintln!("Processing ENDF/B-VIII.0 (tier {:?}, {} K):", triso::model::SPEED, triso::model::TEMPERATURE_K);
            let Loaded::Triso(l) = load_native(Rung::Triso, Tier::Loose, timing)? else { unreachable!() };
            print!("{}", triso::sim::headless_csv(&l.phys, n, seed));
            Ok(())
        }
        Some("--headless-godiva") => {
            let (n, seed) = (num(1, 200)? as usize, num(2, 1)?);
            let Loaded::Godiva(l) = load_native(Rung::Godiva, Tier::Loose, timing)? else { unreachable!() };
            let mut chain = godiva::sim::Chain::new(seed);
            println!("{}", history::CSV_HEADER);
            for _ in 0..n {
                println!("{}", history::csv_row(&chain.run_next(&l.phys)));
            }
            Ok(())
        }
        Some("--headless-keff") => {
            let loose = args.iter().any(|a| a == "--loose");
            let d = Rung::Godiva.run_default().ok_or("no run default")?;
            let cfg = keff::KeffConfig {
                n_particles: num(1, d.n_particles as u64)? as usize,
                n_inactive: num(2, d.n_inactive as u64)? as usize,
                n_active: num(3, d.n_active as u64)? as usize,
                seed: num(4, d.seed)?,
                ..d
            };
            let tier = if loose { Tier::Loose } else { Tier::Exact };
            eprintln!("Processing ENDF/B-VIII.0 for Godiva ({tier:?} tier, {} K):", godiva::model::TEMPERATURE_K);
            let t0 = std::time::Instant::now();
            let Loaded::Godiva(l) = load_native(Rung::Godiva, tier, timing)? else { unreachable!() };
            let phys = l.phys;
            eprintln!("  data ready in {:.1} s; {} neutrons x [{} + {}], seed {}", t0.elapsed().as_secs_f64(), cfg.n_particles, cfg.n_inactive, cfg.n_active, cfg.seed);
            let t = std::time::Instant::now();
            godiva::sim::headless_keff(&phys, cfg, |l| println!("{l}"));
            eprintln!("  transport: {:.1} s (single thread)", t.elapsed().as_secs_f64());
            Ok(())
        }
        Some("--xs-table") => {
            // Godiva's macroscopic cross sections, split the way
            // transport_history splits a collision, at a few energies: the
            // numbers the lesson's reaction-choice picture uses.
            let Loaded::Godiva(l) = load_native(Rung::Godiva, Tier::Exact, timing)? else { unreachable!() };
            let (m, n) = (&l.phys.data.material, &l.phys.data.nuclides);
            println!("E_eV,Sigma_t_per_cm,mfp_cm,fission,capture,inelastic_and_nxn,elastic");
            for e in [1.0e4, 1.0e5, 5.0e5, 1.0e6, 2.0e6, 5.0e6] {
                let (mut t, mut f, mut a, mut inel) = (0.0, 0.0, 0.0, 0.0);
                for c in &m.components {
                    let x = n[c.nuclide_idx].xs_at_energy(e, godiva::model::TEMPERATURE_K);
                    t += c.atom_density * x.total;
                    f += c.atom_density * x.fission;
                    a += c.atom_density * x.absorption;
                    inel += c.atom_density * (x.inelastic + x.n2n + x.n3n + x.mt5);
                }
                let el = t - a - inel;
                println!("{e:e},{t:.5},{:.3},{:.4},{:.4},{:.4},{:.4}", 1.0 / t, f / t, (a - f) / t, inel / t, el / t);
            }
            Ok(())
        }
        Some("--render-geometry") => {
            let dir = std::path::Path::new(arg(1).ok_or("--render-geometry needs an output directory")?);
            triso::render::render_all(dir)?;
            ugraphite::render::render_all(dir)?;
            lumped::render::render_all(dir)?;
            lct008::render::render_all(dir)?;
            htr10::render::render_all(dir)?;
            godiva::render::render_all(dir)
        }
        Some("--bake-htr10-core") => htr10::core::bake::bake_cli(&args),
        Some("--prepare-web-data") => {
            let dir = std::path::Path::new(arg(1).ok_or("--prepare-web-data needs an output directory")?);
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            println!("{:<34} {:>10} {:>10} {:>10}", "tape", "raw MB", "stripped", "on wire");
            let (mut raw_t, mut wire_t) = (0usize, 0usize);
            let mut done: Vec<&str> = Vec::new();
            for r in table::ALL {
                for &(_, tape) in r.jobs() {
                    if done.contains(&tape) {
                        continue; // shared between rungs (U-235, U-238)
                    }
                    done.push(tape);
                    let path = njoy_outram_park_fork::reference_data::reference_endf(tape).ok_or_else(|| format!("missing {tape}"))?;
                    let raw = std::fs::read(&path).map_err(|e| e.to_string())?;
                    let stripped = tapes::strip_covariances(&raw);
                    let wire = tapes::compress(&stripped);
                    std::fs::write(dir.join(tapes::wire_name(tape)), &wire).map_err(|e| e.to_string())?;
                    let mb = |n: usize| n as f64 / 1.0e6;
                    println!("{:<34} {:>10.2} {:>10.2} {:>10.2}", tape, mb(raw.len()), mb(stripped.len()), mb(wire.len()));
                    raw_t += raw.len();
                    wire_t += wire.len();
                }
            }
            // The HTR-10 core's tapes (gh:#786), from nee_soon's own plan.
            let (r, w) = htr10::core::bake::prepare_web_data(dir)?;
            (raw_t, wire_t) = (raw_t + r, wire_t + w);
            println!("total: {:.1} MB of tapes -> {:.1} MB downloaded", raw_t as f64 / 1e6, wire_t as f64 / 1e6);
            Ok(())
        }
        _ => {
            // `--rung <name> --mode <watch|run>`, the native twin of the URL.
            let (rung, mode): (Rung, Mode) = rungs::start(&dhoby_ghaut::web_demo::platform::query_pairs());
            let options = eframe::NativeOptions::default();
            eframe::run_native(
                "Monte Carlo demo",
                options,
                Box::new(move |cc| Ok(Box::new(app::McApp::new(cc, rung, mode)))),
            )
            .map_err(|e| e.to_string())
        }
    }
}

/// In the browser this module runs twice: on the page, where it starts the
/// egui app, and in the physics Web Worker (`web/monte_carlo/worker.js`),
/// where there is no `window` and it runs the engine instead.
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    if js_sys::global().dyn_ref::<web_sys::DedicatedWorkerGlobalScope>().is_some() {
        dhoby_ghaut::web_demo::link::worker_main::<engine::McEngine>();
        return;
    }
    let web_options = eframe::WebOptions {
        // WebGL2 rather than WebGPU, which is still not universal.
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    let (rung, mode) = rungs::start(&dhoby_ghaut::web_demo::platform::query_pairs());
    wasm_bindgen_futures::spawn_local(async move {
        let document = web_sys::window().expect("no window").document().expect("no document");
        let canvas = document
            .get_element_by_id("mc_canvas")
            .expect("no #mc_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#mc_canvas is not a canvas");
        let started = eframe::WebRunner::new()
            .start(canvas, web_options, Box::new(move |cc| Ok(Box::new(app::McApp::new(cc, rung, mode)))))
            .await;
        if let Some(el) = document.get_element_by_id("loading") {
            match started {
                Ok(()) => el.remove(),
                Err(e) => el.set_inner_html(&format!("<p>Failed to start: {e:?}</p>")),
            }
        }
    });
}

// ─── Tests (headless; workspace hard rule) ───────────────────────────────────

#[cfg(all(test, not(target_os = "android"), not(target_arch = "wasm32")))]
mod tests {
    use super::*;
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
        P.get_or_init(|| match load_native(Rung::Triso, Tier::Loose, |_, _| {}).expect("load nuclear data") {
            Loaded::Triso(l) => l.phys,
            _ => unreachable!(),
        })
    }

    fn godiva_physics() -> &'static godiva::sim::Physics {
        static P: OnceLock<godiva::sim::Physics> = OnceLock::new();
        P.get_or_init(|| match load_native(Rung::Godiva, Tier::Loose, |_, _| {}).expect("load nuclear data") {
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
                let e = 1.0e-5 * 10f64.powf(12.3 * k as f64 / 2000.0); // 1e-5 eV .. 20 MeV
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
        let area_fraction = std::f64::consts::PI * model::PEBBLE_R.powi(2) / (2.0 * p).powi(2);
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
                assert!(((x - a).powi(2) + (y - b).powi(2)).sqrt() > 2.0 * rp, "particles overlap");
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
        let link = dhoby_ghaut::web_demo::link::start_native(engine::McEngine::default(), || {});
        link.send(engine::Request::Load { id: 7, rung: Rung::Triso, tier: Tier::Loose });
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
    }

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
}
