//! **The DEM pour demo** (gh:#787): HTR-10 pebbles pour into the vessel and
//! settle, live, in the reader's browser, with the packing fraction converging
//! as they do. Built on `dhoby_ghaut::web_demo` (view, side panel, worker
//! plumbing); published at `demos/dem/` (`web/dem/`).
//!
//! ```text
//! cargo run -p dhoby-ghaut --example dem_web --release [-- --n 4000]
//! cargo run -p dhoby-ghaut --example dem_web --release -- --headless <n> <steps>
//! cargo run -p dhoby-ghaut --example dem_web --release -- --bake-beds
//! cargo test -p dhoby-ghaut --example dem_web --release
//! ```
//!
//! - **Physics**: none written here. `outram_park_fork_liggghts::htr10_fill`,
//!   the LIGGGHTS port's `GranularSystem` (the engine verified against
//!   upstream LIGGGHTS on the HTR-10 core) in the published HTR-10 vessel at
//!   the gh:#216 settings (µ = 0.1, µ_r = 0, E = 5e8 Pa, dt = 35 µs), on one
//!   thread in a Web Worker ([`engine`]). The artwork-grade `DemSimulation`
//!   is not used.
//! - **Reduced count**: the full core holds 27 554 pebbles; the browser pours
//!   a few thousand (chosen on screen, and stated there). The full-size bed
//!   is the baked gh:#216 bed, shown as its own view.
//! - `--headless <n> <steps>` runs the same engine natively, single-threaded,
//!   and prints the cost per step: how the browser count was chosen.
//! - `--bake-beds` writes `examples/common/htr10_beds.zz`, the lattice and
//!   DEM beds of the `htr10` rung's liberties toggle ([`htr10_beds`]).
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`): not for reactor
//! operation, licensing or safety decisions.

#[cfg(not(target_os = "android"))]
mod app;
#[cfg(not(target_os = "android"))]
mod engine;
#[cfg(not(target_os = "android"))]
#[path = "../common/htr10_beds.rs"]
mod htr10_beds;

/// Android stub: windowing GUIs are out of scope on Termux (the workspace
/// example rule: a blanked file gives "main function not found").
#[cfg(target_os = "android")]
fn main() {
    eprintln!("dem_web is a windowing GUI and is not built for Android.");
}

/// Natively: the GUI, or one of the command-line modes. `main` and CLI glue:
/// exempt from the reaching-test rule; each mode calls tested code
/// ([`engine::Engine::serve`], [`htr10_beds::bake::bake`]).
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--bake-beds") => {
            let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
            let csv = std::fs::read_to_string(format!("{root}/{}", htr10_beds::bake::DEM_CSV))
                .map_err(|e| e.to_string())?;
            let (zz, report) = htr10_beds::bake::bake(&csv)?;
            let out = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/common/htr10_beds.zz");
            std::fs::write(out, &zz).map_err(|e| e.to_string())?;
            for l in report {
                println!("{l}");
            }
            println!("wrote {out}");
            Ok(())
        }
        Some("--headless") => {
            let n: usize = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(4000);
            let steps: usize = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(2000);
            let baked = htr10_beds::decode(htr10_beds::BAKED)?;
            let full: Vec<[f64; 3]> = baked
                .dem
                .centres
                .iter()
                .map(|c| [f64::from(c[0]), f64::from(c[1]), f64::from(c[2])])
                .collect();
            println!(
                "# full-size baked bed: vessel bulk phi {:.4}, whole-core phi {:.4}",
                engine::vessel_bulk_fraction(&full),
                baked.dem.stats().phi_whole_core
            );
            let mut e = engine::Engine::default();
            let mut last = None;
            e.serve(
                engine::Request::Start {
                    id: 1,
                    n,
                    seed: app::SEED,
                },
                &mut |ev| last = Some(ev),
            );
            let t0 = std::time::Instant::now();
            let chunk = 500usize;
            let mut done = 0usize;
            println!("# n {n}: step  t[s]  KE/E_drop  phi_whole_core  phi_vessel_bulk  surface[m]  in_core  ms/step");
            while done < steps {
                e.serve(
                    engine::Request::Step {
                        id: 1,
                        steps: chunk,
                    },
                    &mut |ev| last = Some(ev),
                );
                done += chunk;
                if let Some(engine::Event::Progress(s)) = &last {
                    println!(
                        "{} {:.3} {:.3e} {:.4} {:.4} {:.3} {} {:.3}",
                        s.steps,
                        s.time_s,
                        s.ke_ratio,
                        s.phi,
                        s.phi_bulk,
                        s.surface_m,
                        s.n_core,
                        s.ms_per_step
                    );
                    if s.settled || s.gave_up {
                        break;
                    }
                }
            }
            println!(
                "# {done} steps in {:.1} s wall ({:.3} ms/step, one thread)",
                t0.elapsed().as_secs_f64(),
                t0.elapsed().as_secs_f64() * 1e3 / done as f64
            );
            Ok(())
        }
        _ => eframe::run_native(
            "DEM pour demo",
            eframe::NativeOptions::default(),
            Box::new(move |cc| Ok(Box::new(app::DemApp::new(cc)))),
        )
        .map_err(|e| e.to_string()),
    }
}

/// In the browser this module runs twice: on the page, where it starts the
/// egui app, and in the Web Worker (`web/dem/worker.js`), where there is no
/// `window` and it serves the engine instead. Start-up glue (exempt).
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
            .get_element_by_id("dem_canvas")
            .expect("no #dem_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#dem_canvas is not a canvas");
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(move |cc| Ok(Box::new(app::DemApp::new(cc)))),
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
