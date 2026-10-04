//! **The dispersion demo**: one app, a rung of the dispersion lesson ladder at
//! a time (gh:#530; lessons at `deep-dives/dispersion/rungs/`). Built on the
//! shared web-demo framework `dhoby_ghaut::web_demo` (gh:#521): its view, side
//! panel, rung table and worker plumbing.
//!
//! ```text
//! cargo run -p dhoby-ghaut --example dispersion_web --release [-- --rung puffs]
//! cargo test -p dhoby-ghaut --example dispersion_web --release
//! ```
//!
//! In the browser, `?rung=<name>` picks the rung ([`rungs::TABLE`]):
//! `plume`, `sigmas`, `rise-wake`, `puffs`, `deposition`, `dose`, `capstone`.
//!
//! - **Physics**: none written here. `buangkok`'s pyDOSEIA plume, rise, wake
//!   and deposition velocities; `changi`'s Pasquill-Gifford sigmas, puff field
//!   kernel and decay transfer function; see [`engine`] for which call serves
//!   which rung.
//! - **Recorded, not live**: the capstone (`sembawang`'s
//!   `htr10_air_ingress_kora_bound`) is too heavy for a phone, so its numbers
//!   are shown with their provenance ([`recorded`]).
//! - **No lagging**: every calculation runs in a background thread natively
//!   and in a Web Worker in the browser; the UI thread only draws.
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`): not for emergency
//! planning or response, licensing or any safety decision, and no dose shown
//! here is a dose to a real person.

#[cfg(not(target_os = "android"))]
mod app;
#[cfg(not(target_os = "android"))]
mod engine;
#[cfg(not(target_os = "android"))]
mod recorded;
#[cfg(not(target_os = "android"))]
mod rungs;

/// Android stub: windowing GUIs are out of scope on Termux (the workspace
/// example rule: a blanked file gives "main function not found").
#[cfg(target_os = "android")]
fn main() {
    eprintln!("dispersion_web is a windowing GUI and is not built for Android.");
}

/// Natively: `--rung <name>`, the twin of the URL query.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn main() -> Result<(), String> {
    let rung: rungs::Rung =
        dhoby_ghaut::web_demo::lesson::from_query(&dhoby_ghaut::web_demo::platform::query_pairs());
    eframe::run_native(
        "Dispersion demo",
        eframe::NativeOptions::default(),
        Box::new(move |cc| Ok(Box::new(app::DispApp::new(cc, rung)))),
    )
    .map_err(|e| e.to_string())
}

/// In the browser this module runs twice: on the page, where it starts the
/// egui app, and in the Web Worker (`web/dispersion/worker.js`), where there
/// is no `window` and it serves the engine instead.
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
    let rung: rungs::Rung =
        dhoby_ghaut::web_demo::lesson::from_query(&dhoby_ghaut::web_demo::platform::query_pairs());
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
            .get_element_by_id("disp_canvas")
            .expect("no #disp_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#disp_canvas is not a canvas");
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(move |cc| Ok(Box::new(app::DispApp::new(cc, rung)))),
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
