//! **EBR-II simulator for the "Museum – historical reactors" web demos**
//! (gh:#1741, epic gh:#1735; AI draft, 2026-11-24).
//!
//! The Experimental Breeder Reactor II (Argonne, Idaho, 1964-1994), a
//! pool-type sodium-cooled fast reactor, in the two Shutdown Heat Removal
//! Tests of 1984 and 1986: SHRT-17, a protected loss of flow (pumps tripped
//! and scram), and SHRT-45R, the unprotected one (pumps tripped, no scram),
//! where the reactor shut itself down on its own feedback. The plant is the
//! library's `ebr2::model::Ebr2Model`, driven through `ebr2::session`, on
//! the library's engine `ebr2::web` (a Web Worker in the browser, a thread
//! natively). This example holds only `main`, the app and the drawing.
//!
//! ```text
//! cargo run --release -p outram-park-digital-twin-engine --example ebr2_web
//! cargo run --release -p outram-park-digital-twin-engine --example ebr2_web -- --headless [end_s] [every_s] [--shrt17]
//! cargo run --release -p outram-park-digital-twin-engine --example ebr2_web -- --dump-cvs [steps] [--shrt17]   # the twin's CV map (gh:#1749)
//! crates/outram-park-digital-twin-engine/web/ebr2/build.sh   # the browser demo, into dist/
//! ```
//!
//! `--headless` prints `ebr2::session::headless_csv` of the test as
//! published (SHRT-45R by default, `--shrt17` for SHRT-17), default 900 s
//! sampled every 10 s, at the library V&V record's 2 m thermal-centre
//! height. The headless rule's `[steps] [sample_every]` are plant seconds.
//!
//! Published, password-gated, at `demos/ebr2/` on the backend's Pages site
//! (`scripts/build-pages.sh`), in the demos page's "Museum – historical
//! reactors" section.
//!
//! Research, education and V&V only. NOT VALIDATED. Never for operation,
//! licensing or safety decisions.
//!
//! CLI and window glue: reached by no test (it starts an event loop or reads
//! `std::env::args`), per the workspace exception for `main`/CLI glue. What
//! it dispatches to is tested: `ebr2::session::tests` (the headless CSV) and
//! `ebr2::web`'s tests.

#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

#[cfg(not(target_os = "android"))]
mod app;
#[cfg(not(target_os = "android"))]
mod plant_view;
#[cfg(not(target_os = "android"))]
mod keff_tab;

/// Android stub: windowing GUIs are out of scope on Termux.
#[cfg(target_os = "android")]
fn main() {
    eprintln!("ebr2_web is a windowing GUI and is not built for Android.");
}

/// Natively: the egui app with the engine on a thread, or `--headless
/// [end_s] [every_s] [--shrt17]` for a CSV trace with no window.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn main() -> Result<(), String> {
    use outram_park_digital_twin_engine::ebr2::session::{demo_script, headless_csv, SessionConfig};
    use outram_park_digital_twin_engine::ebr2::shrt::ShrtTest;
    use outram_park_digital_twin_engine::ebr2::web as engine;
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--headless") {
        let num = |k: usize, d: f64| args.get(i + k).and_then(|s| s.parse().ok()).unwrap_or(d);
        let test = if args.iter().any(|a| a == "--shrt17") { ShrtTest::Shrt17 } else { ShrtTest::Shrt45r };
        let cfg = SessionConfig { test, ..Default::default() };
        let csv = headless_csv(cfg, &demo_script(), num(1, 900.0), num(2, 10.0)).map_err(|e| e.to_string())?;
        print!("{csv}");
        return Ok(());
    }
    // The multifidelity twin template's CV map (gh:#1744, #1749): the
    // control volumes after `steps` plant seconds of the test.
    if args.iter().any(|a| a == "--dump-cvs") {
        use outram_park_digital_twin_engine::ebr2::twin::{demo_script as twin_script, ebr2_twin};
        use uom::si::{f64::Time, time::second};
        let test = if args.iter().any(|a| a == "--shrt17") { ShrtTest::Shrt17 } else { ShrtTest::Shrt45r };
        let mut twin = ebr2_twin(SessionConfig { test, ..Default::default() }).map_err(|e| e.to_string())?;
        let one = Time::new::<second>(1.0);
        let out = twin.headless(&args, &twin_script(), one, Time::new::<second>(900.0)).map_err(|e| e.to_string())?;
        print!("{out}");
        return Ok(());
    }
    // The MC page's tapes, stripped and compressed, for the web build
    // (gh:#1762). CLI glue.
    if let Some(i) = args.iter().position(|a| a == "--prepare-web-data") {
        let dir = args.get(i + 1).ok_or("--prepare-web-data needs an output directory")?;
        engine::keff::prepare_web_data(std::path::Path::new(dir))?;
        return Ok(());
    }
    eframe::run_native(
        "EBR-II simulator",
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
    use outram_park_digital_twin_engine::ebr2::web as engine;
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
            .get_element_by_id("ebr2_canvas")
            .expect("no #ebr2_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#ebr2_canvas is not a canvas");
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
