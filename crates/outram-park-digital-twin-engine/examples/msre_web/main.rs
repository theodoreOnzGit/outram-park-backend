//! **MSRE simulator for the "Museum – historical reactors" web demos**
//! (gh:#1569, phase 1 gh:#1570; AI draft, 2026-11-14).
//!
//! The Molten-Salt Reactor Experiment (ORNL, 1965-1969) at three fidelity
//! levels: TM-1070's linear frequency response (T-1), the same plant in time
//! with TM-1626's circulating precursors, pumps and radiator
//! (`msre::model::MsreModel`, driven through `msre::session`), and this
//! workspace's full-core Monte Carlo (MSRE N-1/N-3), whose coefficients the
//! plant can use; an in-browser Monte Carlo tab is phase 2 (gh:#1571).
//!
//! The maintainer's hobby project, out of curiosity, built in holidays and
//! free time; not particularly relevant to their work (LWRs and HTGRs). For
//! education.
//!
//! ```text
//! cargo run --release -p outram-park-digital-twin-engine --example msre_web
//! cargo run --release -p outram-park-digital-twin-engine --example msre_web -- --headless [end_s] [every_s]
//! cargo run --release -p outram-park-digital-twin-engine --example msre_web -- --prepare-web-data <dir>   # the k_eff tab's tapes
//! crates/outram-park-digital-twin-engine/web/msre/build.sh   # the browser demo, into dist/
//! ```
//!
//! `--headless` prints `msre::session::headless_csv` of the demonstration
//! transient (`msre::session::demo_script`: rods +10 pcm, fuel pump trip,
//! radiator doors closed), default 1200 s sampled every 10 s. The headless
//! rule's `[steps] [sample_every]` are plant seconds here.
//!
//! Published, password-gated, at `demos/msre/` on the backend's Pages site
//! (`scripts/build-pages.sh`), in the demos page's "Museum – historical
//! reactors" section.
//!
//! Research, education and V&V only. NOT VALIDATED. Never for operation,
//! licensing or safety decisions.
//!
//! CLI and window glue: reached by no test (it starts an event loop or reads
//! `std::env::args`), per the workspace exception for `main`/CLI glue. What
//! it dispatches to is tested: `msre::session::tests` (the headless CSV) and
//! `msre::web`'s tests.

#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

#[cfg(not(target_os = "android"))]
mod app;
// The Core k_eff tab's page (gh:#1571); its pool is the library's.
#[cfg(not(target_os = "android"))]
mod keff_tab;
// The engine and the k_eff worker live in the library since gh:#1578
// (`msre::web`); named here so the app's `crate::engine` / `crate::keff`
// paths read as before.
#[cfg(not(target_os = "android"))]
use outram_park_digital_twin_engine::msre::web::{engine, keff};

/// Android stub: windowing GUIs are out of scope on Termux.
#[cfg(target_os = "android")]
fn main() {
    eprintln!("msre_web is a windowing GUI and is not built for Android.");
}

/// Natively: the egui app with the engine on a thread, or `--headless
/// [end_s] [every_s]` for a CSV trace with no window.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn main() -> Result<(), String> {
    use outram_park_digital_twin_engine::msre::session::{demo_script, headless_csv, SessionConfig};
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--prepare-web-data") {
        let dir = args.get(i + 1).ok_or("--prepare-web-data needs an output directory")?;
        return keff::prepare_web_data(std::path::Path::new(dir));
    }
    if let Some(i) = args.iter().position(|a| a == "--headless") {
        let num = |k: usize, d: f64| args.get(i + k).and_then(|s| s.parse().ok()).unwrap_or(d);
        let csv = headless_csv(SessionConfig::default(), &demo_script(), num(1, 1200.0), num(2, 10.0)).map_err(|e| e.to_string())?;
        print!("{csv}");
        return Ok(());
    }
    eframe::run_native(
        "MSRE simulator",
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            bugis::Theme::default().apply(&cc.egui_ctx);
            let ctx = cc.egui_ctx.clone();
            let link = bugis::web_demo::link::start_native(engine::Runner::default(), move || ctx.request_repaint());
            engine::start_native_ticker(&link);
            Ok(Box::new(app::App::new(link)))
        }),
    )
    .map_err(|e| e.to_string())
}

/// In the browser the same module starts twice: on the page (the egui app)
/// and in the worker (the plant).
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    if js_sys::global().dyn_ref::<web_sys::DedicatedWorkerGlobalScope>().is_some() {
        bugis::web_demo::link::worker_main::<engine::Runner>();
        return;
    }
    // wgpu everywhere (workspace rule): WebGPU where the browser has it,
    // WebGL2 through wgpu where it does not.
    let web_options = eframe::WebOptions { renderer: eframe::Renderer::Wgpu, ..Default::default() };
    wasm_bindgen_futures::spawn_local(async move {
        let document = web_sys::window().expect("no window").document().expect("no document");
        let canvas = document
            .get_element_by_id("msre_canvas")
            .expect("no #msre_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#msre_canvas is not a canvas");
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| {
                    bugis::Theme::default().apply(&cc.egui_ctx);
                    let link = bugis::web_demo::link::start_web(cc.egui_ctx.clone(), engine::worker::WORKER_JS, engine::Event::Error)?;
                    Ok(Box::new(app::App::new(link)))
                }),
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
