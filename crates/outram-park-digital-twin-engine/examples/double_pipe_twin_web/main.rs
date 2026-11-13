//! **The double-pipe high-fidelity → low-fidelity twin** (gh:#1182, rung 5
//! of epic gh:#1177): the TUAS 1-D double-pipe exchanger evaluated live,
//! corrected toward the conjugate-heat-transfer CFD by the Kennedy–O'Hagan
//! calibration of gh:#1180 (η + δ, with its interval), beside the CFD design
//! points (gh:#1178), the co-kriging sweep (gh:#1190) and the filtering
//! traces (gh:#1191). Built on `bugis::web_demo` (side panel, view,
//! zoom buttons, worker plumbing); published at `demos/double-pipe-twin/`
//! (`web/double_pipe_twin/`).
//!
//! **MOVED 2026-11-05** (maintainer: "if web examples live in dhoby
//! ghaut, and this causes the dependency cycle, let the web examples deploy
//! from digital twin engine instead") from `dhoby-ghaut` to
//! `outram-park-digital-twin-engine`, with its web page
//! (`web/double_pipe_twin/`): dhoby-ghaut's dev-dependency on this engine closed a
//! publish-time cycle with the engine's dependency on dhoby-ghaut. The
//! published URL (`demos/double-pipe-twin/`) is unchanged; `dhoby_ghaut::web_demo` paths
//! became `bugis::web_demo` (the module dhoby-ghaut re-exported).
//!
//! ```text
//! cargo run -p outram-park-digital-twin-engine --example double_pipe_twin_web --release
//! cargo run -p outram-park-digital-twin-engine --example double_pipe_twin_web --release -- --headless
//! cargo run -p outram-park-digital-twin-engine --example double_pipe_twin_web --release -- --generate-artefacts [dir]
//! cargo test -p outram-park-digital-twin-engine --example double_pipe_twin_web --release
//! ```
//!
//! - **Physics**: none written here. The live 1-D is the twin engine's
//!   `multifidelity::double_pipe::one_d::ua` (TUAS's nodal conductances);
//!   everything calibrated is a committed artefact ([`artefacts`]).
//! - **No lag**: each worker message is one slice of 1-D evaluations
//!   ([`engine`]); the KOH and MFK fits never run in the browser.
//! - `--headless` prints the default slice as CSV, deterministic (the engine
//!   CLAUDE.md headless rule; regression test in [`engine`]).
//! - `--generate-artefacts` regenerates the committed artefacts natively
//!   ([`generate`], ~20 min).
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`): not for reactor
//! operation, licensing or safety decisions. AI draft, not validated.

#[cfg(not(target_os = "android"))]
mod app;
#[cfg(not(target_os = "android"))]
mod artefacts;
#[cfg(not(target_os = "android"))]
mod engine;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
mod generate;

/// Android stub: windowing GUIs are out of scope on Termux (the workspace
/// example rule: a blanked file gives "main function not found").
#[cfg(target_os = "android")]
fn main() {
    eprintln!("double_pipe_twin_web is a windowing GUI and is not built for Android.");
}

/// Natively: the GUI, or a command-line mode. `main` and CLI glue: exempt
/// from the reaching-test rule; each mode calls tested code
/// ([`engine::headless_csv`], [`generate::run`]'s pieces).
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--headless") => {
            print!("{}", engine::headless_csv(engine::default_operating())?);
            Ok(())
        }
        Some("--generate-artefacts") => {
            let out = args.get(1).cloned().unwrap_or_else(|| {
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/verification_and_validation/double_pipe_twin_web_gh1182"
                )
                .to_string()
            });
            generate::run(&out).map(|_| ())
        }
        _ => eframe::run_native(
            "Double-pipe twin",
            eframe::NativeOptions::default(),
            Box::new(move |cc| Ok(Box::new(app::TwinApp::new(cc)))),
        )
        .map_err(|e| e.to_string()),
    }
}

/// In the browser this module runs twice: on the page, where it starts the
/// egui app, and in the Web Worker (`web/double_pipe_twin/worker.js`),
/// where there is no `window` and it serves the engine. Start-up glue
/// (exempt).
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    if js_sys::global()
        .dyn_ref::<web_sys::DedicatedWorkerGlobalScope>()
        .is_some()
    {
        bugis::web_demo::link::worker_main::<engine::Engine>();
        return;
    }
    let web_options = eframe::WebOptions {
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    wasm_bindgen_futures::spawn_local(async move {
        let document = web_sys::window()
            .expect("no window")
            .document()
            .expect("no document");
        let canvas = document
            .get_element_by_id("double_pipe_twin_canvas")
            .expect("no #double_pipe_twin_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#double_pipe_twin_canvas is not a canvas");
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(move |cc| Ok(Box::new(app::TwinApp::new(cc)))),
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
