//! **The nuclear data demo** (gh:#529): one app, a rung of the nuclear data
//! track at a time. Real ENDF/B-VIII.0 tapes are processed by this
//! workspace's NJOY port, `njoy-outram-park-fork`, in a background thread
//! natively and a Web Worker in the browser (`web/nuclear_data/`, published
//! at `demos/nuclear-data/`), and drawn as they arrive:
//!
//! - `endf` — what is on a tape (sections, resonance-range flags);
//! - `reconr` — RECONR at tolerances 0.3 … 0.001, σ(E) refining on screen;
//! - `broadr` — a temperature slider over U-238's low resonances;
//! - `purr` — PURR's probability bands in the unresolved range;
//! - `thermr` — S(α,β) cross sections and emission (graphite, H in H₂O);
//! - `groupr` — group averages, dilute and self-shielded;
//! - `acer` — recorded results only (an ACE build takes minutes).
//!
//! ```text
//! cargo run -p dhoby-ghaut --example nuclear_data_web --release [-- --rung broadr]
//! cargo run -p dhoby-ghaut --example nuclear_data_web --release -- --headless <rung>
//! cargo run -p dhoby-ghaut --example nuclear_data_web --release -- --prepare-web-data <dir>
//! cargo test -p dhoby-ghaut --example nuclear_data_web --release
//! ```
//!
//! Built on `dhoby_ghaut::web_demo` (view, panel, the no-lag link, loading,
//! rungs); no physics of its own. Each rung's panel states its deliberate
//! liberties (coarser tolerances, a clipped energy window, fewer ladders) and
//! points to the lesson page with the recorded check.
//!
//! Education and research only, per the workspace `RESPONSIBLE_USE.md`. Not
//! for reactor operation, licensing, or safety-critical decisions.

#[cfg(not(target_os = "android"))]
mod app;
#[cfg(not(target_os = "android"))]
mod engine;
#[cfg(not(target_os = "android"))]
mod plot;
#[cfg(not(target_os = "android"))]
mod rungs;
/// Tape bytes on the wire (covariance stripping, zlib): shared with the Monte
/// Carlo demo rather than repeated, so both publish tapes the same way.
#[cfg(not(target_os = "android"))]
#[path = "../monte_carlo_web/tapes.rs"]
mod tapes;

/// Android stub: windowing GUIs are out of scope on Termux.
#[cfg(target_os = "android")]
fn main() {
    eprintln!("nuclear_data_web is a windowing GUI and is not built for Android.");
}

#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn main() -> Result<(), String> {
    use dhoby_ghaut::web_demo::{lesson, platform};
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--prepare-web-data") => {
            let dir = std::path::Path::new(args.get(1).ok_or("--prepare-web-data needs an output directory")?);
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            let mut done: Vec<&str> = Vec::new();
            for r in &rungs::RUNGS {
                for &tape in r.tapes {
                    if done.contains(&tape) {
                        continue;
                    }
                    done.push(tape);
                    let stripped = engine::native_tape(tape)?;
                    let wire = tapes::compress(&stripped);
                    std::fs::write(dir.join(tapes::wire_name(tape)), &wire).map_err(|e| e.to_string())?;
                    println!("{tape:<34} {:>8.2} MB stripped {:>8.2} MB on the wire", stripped.len() as f64 / 1e6, wire.len() as f64 / 1e6);
                }
            }
            Ok(())
        }
        Some("--headless") => {
            let rung: rungs::Rung = lesson::parse(args.get(1).map_or("reconr", String::as_str)).ok_or("unknown rung")?;
            headless(rung)
        }
        _ => {
            let rung: rungs::Rung = lesson::from_query(&platform::query_pairs());
            eframe::run_native("Nuclear data demo", eframe::NativeOptions::default(), Box::new(move |cc| Ok(Box::new(app::NdApp::new(cc, rung)))))
                .map_err(|e| e.to_string())
        }
    }
}

/// Run a rung's default requests through the engine on this thread and print
/// each result's note and timing: the native timings the panel's notes and
/// the lesson pages quote.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn headless(rung: rungs::Rung) -> Result<(), String> {
    use dhoby_ghaut::web_demo::link::NativeEngine as _;
    use engine::{Event, NdEngine, Request};
    let mut e = NdEngine::default();
    let mut failed = None;
    let mut post = |ev: Event| match ev {
        Event::JobDone { index, secs, .. } => eprintln!("  tape {index} loaded in {secs:.1} s"),
        Event::Result { param, secs, n_points, note, .. } => println!("{param:>10} {n_points:>9} {secs:>8.2} s  {note}"),
        Event::Error(m) => failed = Some(m),
        _ => {}
    };
    e.handle(Request::Load { id: 1, rung }, &mut post);
    let reqs: Vec<Request> = match rung {
        rungs::Rung::Endf => vec![Request::Sections { job: 1 }],
        rungs::Rung::Reconr => engine::RECONR_TOLS.iter().map(|&tol| Request::Reconr { job: 1, tol }).collect(),
        rungs::Rung::Broadr => [0.0, 293.6, 900.0, 2500.0].iter().map(|&t| Request::Broadr { job: 1, temp_k: t }).collect(),
        rungs::Rung::Purr => vec![Request::Purr { job: 1, temp_k: 293.6, nladr: 8, nsamp: 2000 }],
        rungs::Rung::Thermr => vec![
            Request::Thermr { job: 1, material: 0, temp_k: 296.0, e_emit: 0.0253 },
            Request::Thermr { job: 1, material: 1, temp_k: 293.6, e_emit: 0.0253 },
        ],
        rungs::Rung::Groupr => vec![Request::Groupr { job: 1, ngroups: 30, sigma0: 1.0e10 }, Request::Groupr { job: 1, ngroups: 30, sigma0: 50.0 }],
        rungs::Rung::Acer => Vec::new(),
    };
    for r in reqs {
        e.handle(r, &mut post);
    }
    match failed {
        Some(m) => Err(m),
        None => Ok(()),
    }
}

/// In the browser this module runs twice: on the page (the egui app) and in
/// the Web Worker (`web/nuclear_data/worker.js`), where there is no `window`
/// and it runs the engine.
#[cfg(target_arch = "wasm32")]
fn main() {
    use dhoby_ghaut::web_demo::{lesson, platform};
    use eframe::wasm_bindgen::JsCast as _;
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    if js_sys::global().dyn_ref::<web_sys::DedicatedWorkerGlobalScope>().is_some() {
        dhoby_ghaut::web_demo::link::worker_main::<engine::NdEngine>();
        return;
    }
    let web_options = eframe::WebOptions { renderer: eframe::Renderer::Glow, ..Default::default() };
    let rung: rungs::Rung = lesson::from_query(&platform::query_pairs());
    wasm_bindgen_futures::spawn_local(async move {
        let document = web_sys::window().expect("no window").document().expect("no document");
        let canvas = document
            .get_element_by_id("nd_canvas")
            .expect("no #nd_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#nd_canvas is not a canvas");
        let started = eframe::WebRunner::new().start(canvas, web_options, Box::new(move |cc| Ok(Box::new(app::NdApp::new(cc, rung))))).await;
        if let Some(el) = document.get_element_by_id("loading") {
            match started {
                Ok(()) => el.remove(),
                Err(e) => el.set_inner_html(&format!("<p>Failed to start: {e:?}</p>")),
            }
        }
    });
}

// ─── Tests (headless) ────────────────────────────────────────────────────────

#[cfg(all(test, not(target_os = "android"), not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use dhoby_ghaut::web_demo::link::NativeEngine as _;
    use engine::{Event, NdEngine, Request};

    /// RECONR at a coarse tolerance and BROADR through the engine, as the UI
    /// asks for them: results arrive for the job asked, and broadening keeps
    /// the area under the capture resonances (the sum rule the rung shows).
    /// Skips when the U-238 tape is not in `reference-data/endf/`.
    #[test]
    fn the_engine_serves_reconr_and_broadr_and_broadening_keeps_the_area() {
        if njoy_outram_park_fork::reference_data::reference_endf(rungs::U238).is_none() {
            eprintln!("SKIP: {} not present", rungs::U238);
            return;
        }
        let mut e = NdEngine::default();
        let mut got: Vec<(u32, f64, usize, String)> = Vec::new();
        let mut post = |ev: Event| match ev {
            Event::Result { job, param, n_points, note, .. } => got.push((job, param, n_points, note)),
            Event::Error(m) => panic!("engine error: {m}"),
            _ => {}
        };
        e.handle(Request::Load { id: 1, rung: rungs::Rung::Reconr }, &mut post);
        e.handle(Request::Reconr { job: 7, tol: 0.3 }, &mut post);
        e.handle(Request::Broadr { job: 8, temp_k: 900.0 }, &mut post);
        assert_eq!(got.len(), 2);
        assert_eq!((got[0].0, got[0].1), (7, 0.3));
        assert!(got[0].2 > 1000, "RECONR at 0.3 still resolves U-238's resonances: {} points", got[0].2);
        let note = &got[1].3;
        let pct: f64 = note.rsplit('(').next().and_then(|s| s.split(' ').next()).and_then(|s| s.parse().ok()).expect("area change in the note");
        assert!(pct.abs() < 0.5, "area under capture moved {pct} % on broadening: {note}");
    }
}
