//! **Delta tracking against surface tracking, step by step** (gh:#784): the
//! demo of rung 5 of the Monte Carlo tutorial (`tutorials/monte-carlo/triso.html`,
//! steps 3 and 4).
//!
//! The same neutron, in the same 2D TRISO cell (the `triso` rung's model,
//! shared with `monte_carlo_web` by path, not copied), transported two ways
//! side by side, one step per tap: surface tracking (every `locate`, every
//! distance-to-boundary query, every crossing) and delta tracking (every
//! flight on the majorant, every tentative collision with `Σ_t/Σ_maj` and the
//! variate that decided it). "Run many" runs `k∞` by both methods and shows
//! the cost per history and whether the two agree; "majorant too low" adds a
//! third run on a majorant scaled down, to show the silent bias. Everything
//! is outram-mc-libs' own code (see [`physics`]), on ENDF/B-VIII.0 data the
//! workspace's NJOY port processes in the browser.
//!
//! ```text
//! cargo run -p dhoby-ghaut --example delta_tracking_web --release
//! cargo run -p dhoby-ghaut --example delta_tracking_web --release -- --headless-trace [seed]
//! cargo run -p dhoby-ghaut --example delta_tracking_web --release -- --headless-run [n inactive active seed low_factor]
//! cargo run -p dhoby-ghaut --example delta_tracking_web --release -- --prepare-web-data <dir>
//! cargo test -p dhoby-ghaut --example delta_tracking_web --release
//! ```
//!
//! In the browser (`web/delta_tracking/`, published at `demos/delta-tracking/`)
//! the tapes are the Monte Carlo demo's (`../monte-carlo/data/`).
//!
//! Education and research only, per the workspace `RESPONSIBLE_USE.md`. Not
//! for reactor operation, licensing, or safety-critical decisions.

#[cfg(not(target_os = "android"))]
#[path = "../monte_carlo_web/triso/model.rs"]
mod model;
#[cfg(not(target_os = "android"))]
#[path = "../monte_carlo_web/tapes.rs"]
mod tapes;
// The processed nuclear data every demo shares, cached in the browser
// (gh:#818): this demo reads the products the Monte Carlo demo's `triso`
// rung made (same tapes, settings and keys), and the other way round.
#[cfg(not(target_os = "android"))]
#[path = "../common/processed_cache.rs"]
mod processed_cache;
#[cfg(not(target_os = "android"))]
mod physics;
#[cfg(not(target_os = "android"))]
mod wire;
#[cfg(not(target_os = "android"))]
mod state;
#[cfg(not(target_os = "android"))]
mod engine;
#[cfg(not(target_os = "android"))]
mod app;

/// Android stub: windowing GUIs are out of scope on Termux (the workspace
/// example rule; a blanked file gives "main function not found").
#[cfg(target_os = "android")]
fn main() {
    eprintln!("delta_tracking_web is a windowing GUI and is not built for Android.");
}

/// Read a reference tape from `reference-data/endf/` and strip its
/// covariances: the bytes the browser gets once it has inflated the download.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn native_tape(tape: &str) -> Result<Vec<u8>, String> {
    let path = njoy_outram_park_fork::reference_data::reference_endf(tape)
        .ok_or_else(|| format!("reference tape {tape} is not present"))?;
    let raw = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(tapes::strip_covariances(&raw))
}

/// Process the `triso` rung's tapes natively, reporting each job's seconds.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn load_native(mut report: impl FnMut(&str, f64)) -> Result<physics::Physics, String> {
    let mut b = model::DataBuilder::default();
    while let Some(job) = b.next_job() {
        let t = std::time::Instant::now();
        native_tape(job.tape)
            .and_then(|bytes| b.step(&bytes, &mut processed_cache::DataStore::off()))
            .map_err(|e| format!("{}: {e}", job.label))?;
        report(job.label, t.elapsed().as_secs_f64());
    }
    let t = std::time::Instant::now();
    let p = physics::Physics::new(b.finish()?);
    report("assemble + majorant", t.elapsed().as_secs_f64());
    Ok(p)
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let num = |i: usize, d: f64| -> Result<f64, String> {
        args.get(i).map_or(Ok(d), |s| {
            s.parse::<f64>().map_err(|e| format!("argument {i}: {e}"))
        })
    };
    let timing = |label: &str, s: f64| eprintln!("  {label:<22} {s:7.2} s");
    match args.first().map(String::as_str) {
        Some("--headless-trace") => {
            let phys = load_native(timing)?;
            let b = physics::birth(&phys, num(1, 1.0)? as u64);
            let s = physics::trace_surface(&phys, b);
            let d = physics::trace_delta(&phys, b, &phys.majorant);
            println!(
                "birth r = ({:.4}, {:.4}) cm, E = {:.4e} eV, seed {}",
                b.r.x, b.r.y, b.e, b.seed
            );
            println!("surface: {:?}", s.counts);
            println!("delta:   {:?}", d.counts);
            for (name, t) in [("surface", &s), ("delta", &d)] {
                println!("--- {name}, first 12 steps ---");
                for e in t.events.iter().take(12) {
                    println!("{}", wire::describe(e));
                }
            }
            Ok(())
        }
        Some("--prepare-web-data") => {
            // The `triso` rung's tapes as the Monte Carlo demo publishes them
            // (`monte_carlo_web --prepare-web-data` writes the same files for
            // every rung); for serving this demo without that one.
            let dir = std::path::Path::new(
                args.get(1)
                    .ok_or("--prepare-web-data needs an output directory")?,
            );
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            for job in model::JOBS {
                let wire = tapes::compress(&native_tape(job.tape)?);
                std::fs::write(dir.join(tapes::wire_name(job.tape)), &wire)
                    .map_err(|e| e.to_string())?;
                eprintln!("  {:<34} {:8.2} MB", job.tape, wire.len() as f64 / 1e6);
            }
            Ok(())
        }
        Some("--headless-run") => {
            let phys = load_native(timing)?;
            let low = num(5, 0.5)?;
            let cfg = physics::RunConfig {
                n_particles: num(1, 200.0)? as usize,
                n_inactive: num(2, 5.0)? as usize,
                n_active: num(3, 20.0)? as usize,
                seed: num(4, 784.0)? as u64,
                low_factor: (low > 0.0).then_some(low),
            };
            eprintln!(
                "majorant: {} points, {:.2} s",
                phys.majorant.len(),
                phys.majorant_secs
            );
            let mut run = physics::Run::new(&phys, cfg);
            let mut last = Vec::new();
            for &m in run.methods() {
                let (mut secs, mut counts, mut histories) = (
                    0.0,
                    outram_mc_libs::physics::tracking_trace::TraceCounts::default(),
                    0usize,
                );
                let mut fin = None;
                while let Some(g) = run.step(&phys, m) {
                    secs += g.secs;
                    counts.merge(&g.counts);
                    histories += g.n_particles;
                    fin = g.k_mean;
                }
                let (k, s) = fin.unwrap_or((f64::NAN, f64::NAN));
                let h = histories as f64;
                println!(
                    "{:<26} k = {k:.5} ± {s:.5}  {:8.1} µs/history  per history: {:.1} stops, {:.1} locates, {:.1} queries, {:.1} crossings, {:.1} flights, {:.1} virtual, {:.1} real, {:.3} violations",
                    m.label(),
                    1e6 * secs / h,
                    counts.stops() as f64 / h,
                    counts.locates as f64 / h,
                    counts.boundary_queries as f64 / h,
                    counts.crossings as f64 / h,
                    counts.flights as f64 / h,
                    counts.virtual_collisions as f64 / h,
                    counts.collisions as f64 / h,
                    counts.majorant_violations as f64 / h,
                );
                last.push((m, (k, s), secs / h));
            }
            let z = |a: usize, b: usize| physics::z_score(last[a].1, last[b].1);
            println!(
                "delta − surface: {:+.0} pcm ({:+.2} σ)",
                (last[1].1 .0 - last[0].1 .0) * 1e5,
                -z(0, 1)
            );
            if last.len() == 3 {
                println!(
                    "low − surface:   {:+.0} pcm ({:+.2} σ)",
                    (last[2].1 .0 - last[0].1 .0) * 1e5,
                    -z(0, 2)
                );
            }
            Ok(())
        }
        _ => {
            let options = eframe::NativeOptions::default();
            eframe::run_native(
                "Delta vs surface tracking",
                options,
                Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
            )
            .map_err(|e| e.to_string())
        }
    }
}

/// In the browser this module runs twice: on the page, where it starts the
/// egui app, and in the physics Web Worker (`web/delta_tracking/worker.js`),
/// where there is no `window` and it runs the engine instead.
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    if js_sys::global()
        .dyn_ref::<web_sys::DedicatedWorkerGlobalScope>()
        .is_some()
    {
        dhoby_ghaut::web_demo::link::worker_main::<engine::Engine>();
        return;
    }
    let web_options = eframe::WebOptions {
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    wasm_bindgen_futures::spawn_local(async move {
        let document = web_sys::window()
            .expect("no window")
            .document()
            .expect("no document");
        let canvas = document
            .get_element_by_id("dt_canvas")
            .expect("no #dt_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#dt_canvas is not a canvas");
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
            )
            .await;
        if let Some(el) = document.get_element_by_id("loading") {
            match started {
                Ok(()) => el.remove(),
                Err(e) => el.set_inner_html(&format!("<p>Failed to start: {e:?}</p>")),
            }
        }
    });
}

#[cfg(all(test, not(target_os = "android"), not(target_arch = "wasm32")))]
mod tests;
