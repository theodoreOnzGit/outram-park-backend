//! **Fukushima Daiichi Unit 1 station-blackout demo** (AI draft, 2026-10-21).
//!
//! A GE BWR/3 with isolation condensers in a Mark I containment, from the
//! earthquake's scram through the station blackout, core damage, hydrogen,
//! venting and the reactor-building explosion, after the recorded sequence or
//! the reader's own actions. `bwr_sim` is kept for modern BWRs (BWRX-300,
//! ABWR); this binary is the Fukushima plant only.
//!
//! ~~**Built against the `outram-park` metacrate only**: every backend crate is
//! reached through `outram_park::backend::*` (feature `fukushima`).~~
//! **MOVED 2026-10-27 (gh:#986, maintainer's choice)** from the `outram-park`
//! repository (`outram-park/src/fukushima_sim/`) to this crate, as the
//! `fukushima_web` example, so the backend's Pages site can publish it: the
//! `outram-park` repository has no Pages site. Every `outram_park::backend::`
//! path was a re-export of a backend crate, so the move only removed that
//! prefix; the model, the engine and its tests are unchanged.
//!
//! ```text
//! cargo run --release -p dhoby-ghaut --example fukushima_web
//! cargo run --release -p dhoby-ghaut --example fukushima_web -- --headless 108000 600
//! crates/dhoby-ghaut/web/fukushima/build.sh      # the browser demo, into dist/
//! ```
//!
//! The browser build is published, password-gated, at `demos/fukushima/` on
//! the backend's Pages site (`scripts/build-pages.sh`). ~~Local only: this
//! demo is NOT published on any Pages site (maintainer, 2026-10-21).~~
//! **CHANGED 2026-10-27** at the maintainer's request ("put the fukushima sim
//! on web demo also ... so i can see it on my phone").
//!
//! Research, education and V&V only. Not validated; not for operation,
//! licensing, safety decisions or emergency response. See `physics::params`
//! for the provenance of every number, and each physics module for what it
//! leaves out.
//!
//! CLI and window glue: reached by no test (it starts an event loop or reads
//! `std::env::args`), per the workspace exception for `main`/CLI glue. What it
//! dispatches to is tested — `physics::headless_csv` by
//! `physics::tests::the_headless_run_matches_the_committed_fixture`, and the
//! engine behind the window by `engine::tests`.

#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

#[cfg(not(target_os = "android"))]
mod app;
#[cfg(not(target_os = "android"))]
mod engine;
#[cfg(not(target_os = "android"))]
mod physics;

/// Android stub: windowing GUIs are out of scope on Termux (the workspace
/// example rule: a blanked file gives "main function not found").
#[cfg(target_os = "android")]
fn main() {
    eprintln!("fukushima_web is a windowing GUI and is not built for Android.");
}

/// Natively: the egui app with the engine on a thread, or `--headless
/// [steps] [every]` for a CSV trace with no window.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--headless") {
        let steps = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(3600);
        let every = args.get(i + 2).and_then(|s| s.parse().ok()).unwrap_or(60);
        print!("{}", physics::headless_csv(steps, every));
        return Ok(());
    }
    eframe::run_native(
        "Fukushima Daiichi Unit 1",
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            cc.egui_ctx.set_theme(egui::Theme::Dark);
            let ctx = cc.egui_ctx.clone();
            let link = dhoby_ghaut::web_demo::link::start_native(engine::Runner::default(), move || ctx.request_repaint());
            Ok(Box::new(app::App::new(link)))
        }),
    )
    .map_err(|e| e.to_string())
}

/// In the browser the same module starts four times: on the page (the egui
/// app), and in three workers told apart by their name: the plant worker
/// (unnamed) and the `core` and `containment` solver workers it starts.
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    if let Some(scope) = js_sys::global().dyn_ref::<web_sys::DedicatedWorkerGlobalScope>() {
        match scope.name().as_str() {
            "core" => engine::web::solver_main(0),
            "containment" => engine::web::solver_main(1),
            _ => dhoby_ghaut::web_demo::link::worker_main::<engine::Runner>(),
        }
        return;
    }
    // wgpu everywhere (maintainer, 2026-10-27: "get glow out, let's use wgpu
    // only"). The native build above and outram-park-backend were already
    // wgpu-only; this makes the browser the same renderer rather than the one
    // odd target. WebGPU where the browser has it, WebGL2 where it does not
    // (the `wgpu/webgl` feature in Cargo.toml), so coverage is unchanged.
    let web_options =
        eframe::WebOptions { renderer: eframe::Renderer::Wgpu, ..Default::default() };
    wasm_bindgen_futures::spawn_local(async move {
        let document = web_sys::window().expect("no window").document().expect("no document");
        let canvas = document
            .get_element_by_id("fukushima_canvas")
            .expect("no #fukushima_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#fukushima_canvas is not a canvas");
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| {
                    cc.egui_ctx.set_theme(egui::Theme::Dark);
                    let link = dhoby_ghaut::web_demo::link::start_web(cc.egui_ctx.clone(), engine::web::WORKER_JS, engine::Event::Error)?;
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
