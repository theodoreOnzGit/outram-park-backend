//! **Fukushima Daiichi Unit 1 station-blackout demo** (AI draft, 2026-10-21).
//!
//! **MOVED AGAIN 2026-11-05** (maintainer: "if web examples live in dhoby
//! ghaut, and this causes the dependency cycle, let the web examples deploy
//! from digital twin engine instead") from `dhoby-ghaut` to
//! `outram-park-digital-twin-engine`, with its web page
//! (`web/fukushima/`): dhoby-ghaut's dev-dependency on this engine closed a
//! publish-time cycle with the engine's dependency on dhoby-ghaut. The
//! published URL (`demos/fukushima/`) is unchanged; `dhoby_ghaut::web_demo` paths
//! became `bugis::web_demo` (the module dhoby-ghaut re-exported).
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
//! cargo run --release -p outram-park-digital-twin-engine --example fukushima_web
//! cargo run --release -p outram-park-digital-twin-engine --example fukushima_web -- --headless 108000 600
//! cargo run --release -p outram-park-digital-twin-engine --example fukushima_web -- --compare   # BSAF table (gh:#1032)
//! cargo run --release -p outram-park-digital-twin-engine --example fukushima_web -- --headless 108000 600 --lumped-containment
//! cargo run --release -p outram-park-digital-twin-engine --example fukushima_web -- --bench-workers [skip] [steps]   # gh:#1099
//! crates/outram-park-digital-twin-engine/web/fukushima/build.sh      # the browser demo, into dist/
//! ```
//!
//! The browser build is published, password-gated, at `demos/fukushima/` on
//! the backend's Pages site (`scripts/build-pages.sh`). ~~Local only: this
//! demo is NOT published on any Pages site (maintainer, 2026-10-21).~~
//! **CHANGED 2026-10-27** at the maintainer's request ("put the fukushima sim
//! on web demo also ... so i can see it on my phone").
//!
//! Research, education and V&V only. Not validated; not for operation,
//! licensing, safety decisions or emergency response. See `sembawang::bwr_mark1::params`
//! for the provenance of every number, and each physics module for what it
//! leaves out.
//!
//! CLI and window glue: reached by no test (it starts an event loop or reads
//! `std::env::args`), per the workspace exception for `main`/CLI glue. What it
//! dispatches to is tested — `sembawang::bwr_mark1::headless_csv` by
//! `sembawang::bwr_mark1::tests::the_headless_run_matches_the_committed_fixture`, and the
//! engine behind the window by `engine::tests`.

#![cfg_attr(target_arch = "wasm32", allow(dead_code))]

#[cfg(not(target_os = "android"))]
mod app;
#[cfg(not(target_os = "android"))]
mod engine;
// The plant physics. ~~`mod physics;` (this example's own folder)~~ MOVED
// 2026-10-29 (gh:#1033, maintainer: "Physics libraries go to sembawang"): it
// is `sembawang::bwr_mark1`, its tests and the 30 h fixture with it; this
// demo only draws it and steps it on its workers.
#[cfg(not(target_os = "android"))]
use sembawang::bwr_mark1 as physics;

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
    // `--lumped-containment`: the explicit ablation of the containment
    // network, the plant before gh:#1099 (2026-11-02).
    let lumped = args.iter().any(|a| a == "--lumped-containment");
    let ablate = |m: physics::Mechanisms| if lumped { m.without_network_containment() } else { m };
    if args.iter().any(|a| a == "--compare") {
        let runs: Vec<(&str, physics::compare::Metrics)> =
            physics::compare::ablations().into_iter().map(|(n, m)| (n, physics::compare::measure(ablate(m)))).collect();
        print!("{}", physics::compare::report(&runs));
        return Ok(());
    }
    if let Some(i) = args.iter().position(|a| a == "--headless") {
        let num = |k: usize| args.get(i + k).and_then(|s| s.parse().ok());
        let steps = num(1).unwrap_or(3600);
        let every = num(2).unwrap_or(60);
        print!("{}", physics::headless_csv_with(ablate(physics::Mechanisms::default()), steps, every));
        return Ok(());
    }
    if let Some(i) = args.iter().position(|a| a == "--bench-workers") {
        let num = |k: usize| args.get(i + k).and_then(|s| s.parse().ok());
        let t = engine::native_workers::time_modes(num(1).unwrap_or(0), num(2).unwrap_or(3600));
        println!(
            "{t:?}\nsteps {} to {}: inline {:.3} ms/step, three workers {:.3} ms/step, speed-up {:.3}",
            t.skip,
            t.skip + t.steps,
            1e3 * t.inline_s / t.steps as f64,
            1e3 * t.three_s / t.steps as f64,
            t.inline_s / t.three_s
        );
        return Ok(());
    }
    eframe::run_native(
        "Fukushima Daiichi Unit 1",
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            bugis::Theme::default().apply(&cc.egui_ctx); // Gruvbox Dark (`bugis`)
            let ctx = cc.egui_ctx.clone();
            let link = bugis::web_demo::link::start_native(engine::Runner::default(), move || ctx.request_repaint());
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
            _ => bugis::web_demo::link::worker_main::<engine::Runner>(),
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
                    bugis::Theme::default().apply(&cc.egui_ctx); // Gruvbox Dark (`bugis`)
                    let link = bugis::web_demo::link::start_web(cc.egui_ctx.clone(), engine::web::WORKER_JS, engine::Event::Error)?;
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
