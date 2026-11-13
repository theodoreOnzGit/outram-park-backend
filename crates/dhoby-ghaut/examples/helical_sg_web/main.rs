//! **The helical-coil steam generator demo** (gh:#1157, part of epic
//! gh:#1153): one tube of a helical-coil once-through steam generator, water
//! from subcooled to superheated, a hot fluid falling past in counterflow.
//! Built on `dhoby_ghaut::web_demo` (view, side panel, worker plumbing);
//! published at `demos/helical-sg/` (`web/helical_sg/`).
//!
//! ```text
//! cargo run -p dhoby-ghaut --example helical_sg_web --release
//! cargo run -p dhoby-ghaut --example helical_sg_web --release -- --headless [cells]
//! cargo run -p dhoby-ghaut --example helical_sg_web --release -- --render-geometry [out.png]
//! cargo test -p dhoby-ghaut --example helical_sg_web --release
//! ```
//!
//! - **Physics**: none written here. `tampines::components::helical_coil_otsg`,
//!   the steady 1-D march with its cited coil, boiling, dryout and two-phase
//!   pressure-drop closures and IF97 water; V&V record
//!   `crates/tampines/verification_and_validation/helical_coil_otsg_gh1157.md`.
//! - **No lag**: each worker message is one march of the shooting iteration
//!   ([`engine`]); the page only draws.
//! - `--headless [cells]` prints the converged profile as CSV, deterministic
//!   (the engine CLAUDE.md headless rule; regression test in [`engine`]).
//! - `--render-geometry` writes the drawing rule's review image of the coil
//!   ([`render`]).
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`): not for reactor
//! operation, licensing or safety decisions.

#[cfg(not(target_os = "android"))]
mod app;
#[cfg(not(target_os = "android"))]
mod engine;
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
mod render;

/// Android stub: windowing GUIs are out of scope on Termux (the workspace
/// example rule: a blanked file gives "main function not found").
#[cfg(target_os = "android")]
fn main() {
    eprintln!("helical_sg_web is a windowing GUI and is not built for Android.");
}

/// Natively: the GUI, or a command-line mode. `main` and CLI glue: exempt
/// from the reaching-test rule; each mode calls tested code
/// ([`engine::headless_csv`], [`render::render`]).
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--headless") => {
            let mut p = engine::Params::default();
            if let Some(n) = args.get(1).and_then(|v| v.parse::<f64>().ok()) {
                p.nodes = n;
            }
            print!("{}", engine::headless_csv(p)?);
            Ok(())
        }
        Some("--render-geometry") => {
            let out = args.get(1).cloned().unwrap_or_else(|| {
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../tampines/verification_and_validation/helical_coil_otsg_gh1157_geometry.png"
                )
                .to_string()
            });
            let (img, counts) = render::render(&engine::Params::default());
            img.write_png(&out).map_err(|e| e.to_string())?;
            println!("wrote {out}; panel B pixels bore {} wall {} shell {}", counts[0], counts[1], counts[2]);
            Ok(())
        }
        _ => eframe::run_native(
            "Helical steam generator",
            eframe::NativeOptions::default(),
            Box::new(move |cc| Ok(Box::new(app::HelicalApp::new(cc)))),
        )
        .map_err(|e| e.to_string()),
    }
}

/// In the browser this module runs twice: on the page, where it starts the
/// egui app, and in the Web Worker (`web/helical_sg/worker.js`), where there
/// is no `window` and it serves the engine. Start-up glue (exempt).
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
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    wasm_bindgen_futures::spawn_local(async move {
        let document = web_sys::window()
            .expect("no window")
            .document()
            .expect("no document");
        let canvas = document
            .get_element_by_id("helical_sg_canvas")
            .expect("no #helical_sg_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#helical_sg_canvas is not a canvas");
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(move |cc| Ok(Box::new(app::HelicalApp::new(cc)))),
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
