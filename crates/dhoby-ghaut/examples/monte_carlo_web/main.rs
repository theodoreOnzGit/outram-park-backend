//! **The Monte Carlo demo** — one app, a rung of the tutorial ladder at a time
//! (gh:#520, #521), from `godiva`, a bare uranium sphere (Watch, and a true
//! Run k_eff), to `htr10`; the full list is the rung table below. Four of the
//! rungs are the ICSBEP benchmark cases the acceptance tests run (gh:#859):
//! `godiva`, `jemima`, `hst009` and `lct008`, each with its geometry drawn,
//! recorded tracks shipped with the page, and a Run k_eff at settings the
//! reader chooses. Every
//! neutron is transported by outram-mc-libs on real ENDF/B-VIII.0 data that
//! the workspace's own NJOY port processes — in the browser, on the reader's
//! machine. Runs natively and in the browser, the long runs and the data
//! processing on a pool of workers (gh:#905, #907, #908)
//! (`web/monte_carlo/`, published at `demos/monte-carlo/`).
//!
//! ```text
//! cargo run -p dhoby-ghaut --example monte_carlo_web --release [-- --rung godiva --mode run]
//! cargo run -p dhoby-ghaut --example monte_carlo_web --release -- --headless [n] [seed]
//! cargo run -p dhoby-ghaut --example monte_carlo_web --release -- --headless-godiva [n] [seed]
//! cargo run -p dhoby-ghaut --example monte_carlo_web --release -- --headless-keff [n inactive active seed]
//! cargo run -p dhoby-ghaut --example monte_carlo_web --release -- --headless-keff-rung <jemima|hst009|lct008> [n inactive active seed]
//! (~~`--loose`, the 0.01 tier~~: removed 2026-10-16, gh:#876 item 3; every
//! load is NJOY's 0.001.)
//! cargo run -p dhoby-ghaut --example monte_carlo_web --release -- --bake-tracks <rung> [n]
//! cargo run -p dhoby-ghaut --example monte_carlo_web --release -- --htr10-product-shas [threads]
//! cargo run -p dhoby-ghaut --example monte_carlo_web --release -- --render-geometry <dir>
//! cargo run -p dhoby-ghaut --example monte_carlo_web --release -- --prepare-web-data <dir>
//! cargo test -p dhoby-ghaut --lib --release mc_demo   (the tests moved with the app, gh:#1763)
//! ```
//!
//! In the browser, `?rung=<name>&mode=<watch|run>` picks the rung and mode
//! ([`rungs`] is the table). Until 2026-10-04 this was the TRISO-only
//! `triso_pebble_web` example; its URL `demos/triso-pebble/` now redirects
//! here with `?rung=triso`.
//!
//! - **Physics**: outram-mc-libs continuous-energy transport, unmodified.
//!   ~~The Godiva Run k_eff is its single-thread reference power iteration
//!   (`PowerIteration`, which `run_keff` is), stepped one generation at a
//!   time.~~ **Since 2026-10-16 (gh:#907, #908)** every benchmark rung's Run
//!   k_eff, Godiva's included, runs on a pool of workers through
//!   `transport_csg::distributed` (`run_keff_csg_par` bit for bit); Watch's
//!   whole generations still step the single-thread iteration.
//! - **Data**: ENDF/B-VIII.0 tapes from `reference-data/endf/`, covariances
//!   stripped, processed by the workspace's NJOY port (RECONR + BROADR). The
//!   tier per rung and mode is in [`app`] (`tier_for`): ~~Godiva Run k_eff at
//!   NJOY's tolerance 0.001, so its `k` is comparable with the record; the
//!   Watch modes at 0.01~~ **every rung and mode at NJOY's 0.001, one
//!   cached load** (maintainer, 2026-10-16, gh:#876 item 3); views that
//!   need no cross sections load none.
//! - **Geometry**: TRISO from IAEA-TECDOC-1382 ([`triso::model`]); Godiva from
//!   `outram_mc_libs::vv::godiva` ([`godiva::model`]).
//!
//! Education and research only, per the workspace `RESPONSIBLE_USE.md`. Not for
//! reactor operation, licensing, or safety-critical decisions.
//!
//! **Since gh:#1763 the app, its engine and every rung are the library's
//! `dhoby_ghaut::mc_demo`** (feature `mc_demo`), so `htgr_sim_v1`'s MC page
//! can draw the HTR-10 core itself. This file keeps the `main` functions:
//! the headless tools, the native window and the browser entry.

#[cfg(not(target_os = "android"))]
use dhoby_ghaut::mc_demo::{app, engine, rungs};
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
use dhoby_ghaut::mc_demo::{csgrung, godiva, history, hst009, htr10, jemima, keff, lct008, lumped, recorded, table, tapes, triso, ugraphite};

/// Android stub: windowing GUIs are out of scope on Termux (the workspace
/// example rule — a blanked file gives "main function not found").
#[cfg(target_os = "android")]
fn main() {
    eprintln!("monte_carlo_web is a windowing GUI and is not built for Android.");
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
use dhoby_ghaut::mc_demo::load_native;

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
            let Loaded::Triso(l) = load_native(Rung::Triso, Tier::Exact, timing)? else { unreachable!() };
            print!("{}", triso::sim::headless_csv(&l.phys, n, seed));
            Ok(())
        }
        Some("--headless-godiva") => {
            let (n, seed) = (num(1, 200)? as usize, num(2, 1)?);
            let Loaded::Godiva(l) = load_native(Rung::Godiva, Tier::Exact, timing)? else { unreachable!() };
            let mut chain = godiva::sim::Chain::new(seed);
            println!("{}", history::CSV_HEADER);
            for _ in 0..n {
                println!("{}", history::csv_row(&chain.run_next(&l.phys)));
            }
            Ok(())
        }
        Some("--headless-keff") => {
            let d = Rung::Godiva.run_default().ok_or("no run default")?;
            let cfg = keff::KeffConfig {
                n_particles: num(1, d.n_particles as u64)? as usize,
                n_inactive: num(2, d.n_inactive as u64)? as usize,
                n_active: num(3, d.n_active as u64)? as usize,
                seed: num(4, d.seed)?,
                ..d
            };
            let tier = Tier::Exact;
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
            jemima::render::render_all(dir)?;
            hst009::render::render_all(dir)?;
            htr10::render::render_all(dir)?;
            godiva::render::render_all(dir)
        }
        Some("--render-pour") => {
            let csv = std::fs::read_to_string(arg(1).ok_or("--render-pour <csv> <fuel fraction> <dir>")?).map_err(|e| e.to_string())?;
            let f: f64 = arg(2).and_then(|v| v.parse().ok()).ok_or("--render-pour needs a fuel fraction")?;
            let dir = std::path::Path::new(arg(3).ok_or("--render-pour needs an output directory")?);
            println!("{}", htr10::core::pour::render_review(&csv, f, dir)?);
            Ok(())
        }
        Some("--bake-htr10-core") => htr10::core::bake::bake_cli(&args),
        Some("--htr10-product-shas") => htr10::core::bake::product_shas_cli(&args),
        Some("--bake-tracks") => {
            // The tracks a rung ships with the page (gh:#859, `recorded.rs`).
            let name = arg(1).ok_or("--bake-tracks needs a rung name")?;
            let rung = *table::ALL.iter().find(|r| r.info().name == name).ok_or_else(|| format!("no rung named {name}"))?;
            let n = num(2, recorded::DEFAULT_COUNT as u64)? as usize;
            let bytes = recorded::bake(rung, n, timing)?;
            let path = recorded::path(rung);
            std::fs::create_dir_all(path.parent().ok_or("no parent")?).map_err(|e| e.to_string())?;
            std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
            let states: usize = recorded::histories(&bytes)?.iter().map(|h| h.track.states.len()).sum();
            println!("{n} histories, {states} states, {:.1} kB -> {}", bytes.len() as f64 / 1e3, path.display());
            Ok(())
        }
        Some("--headless-keff-rung") => {
            // A benchmark rung's Run k_eff without a window (gh:#859):
            // `--headless-keff-rung <jemima|hst009|lct008> [n inactive active seed]`.
            let name = arg(1).ok_or("--headless-keff-rung needs a rung name")?;
            let rung = *table::ALL.iter().find(|r| r.info().name == name).ok_or_else(|| format!("no rung named {name}"))?;
            let d = rung.run_default().ok_or("this rung has no Run k_eff")?;
            let cfg = keff::KeffConfig {
                n_particles: num(2, d.n_particles as u64)? as usize,
                n_inactive: num(3, d.n_inactive as u64)? as usize,
                n_active: num(4, d.n_active as u64)? as usize,
                seed: num(5, d.seed)?,
                ..d
            };
            let tier = Tier::Exact;
            eprintln!("Processing ENDF/B-VIII.0 for {} ({tier:?} tier):", rung.info().title);
            let t0 = std::time::Instant::now();
            let loaded = load_native(rung, tier, timing)?;
            eprintln!("  data ready in {:.1} s; {} neutrons x [{} + {}], seed {}", t0.elapsed().as_secs_f64(), cfg.n_particles, cfg.n_inactive, cfg.n_active, cfg.seed);
            let t = std::time::Instant::now();
            match &loaded {
                Loaded::Jemima(l) => csgrung::headless_keff::<jemima::Jemima>(&l.phys, cfg, |l| println!("{l}")),
                Loaded::Hst009(l) => csgrung::headless_keff::<hst009::Hst009>(&l.phys, cfg, |l| println!("{l}")),
                Loaded::Lct008(l) => lct008::headless_keff(&l.phys, cfg, |l| println!("{l}")),
                _ => return Err(format!("--headless-keff-rung serves jemima, hst009 and lct008 (godiva: --headless-keff), not {name}")),
            };
            eprintln!("  transport: {:.1} s (single thread)", t.elapsed().as_secs_f64());
            Ok(())
        }
        Some("--prepare-web-data") => {
            let dir = std::path::Path::new(arg(1).ok_or("--prepare-web-data needs an output directory")?);
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            println!("{:<34} {:>10} {:>10} {:>10}", "tape", "raw MB", "stripped", "on wire");
            let (mut raw_t, mut wire_t) = (0usize, 0usize);
            let mut done: Vec<&str> = Vec::new();
            for r in table::ALL {
                for &(_, tape) in r.jobs() {
                    if tapes::is_generated(tape) {
                        continue; // a LEAPR law: nothing to publish
                    }
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
        renderer: eframe::Renderer::Wgpu,
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
