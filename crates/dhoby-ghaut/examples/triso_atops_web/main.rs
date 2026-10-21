//! **The TRISO-ATOPS and fuel failure demo**: one app, a rung of the
//! boon-lay lesson ladder at a time (gh:#540, the demo phase of #531; lessons
//! at `tutorials/triso-atops/`). Built on the shared web-demo framework
//! `dhoby_ghaut::web_demo` (gh:#521): its view, side panel, rung table and
//! worker plumbing.
//!
//! ```text
//! cargo run -p dhoby-ghaut --example triso_atops_web --release [-- --rung failure]
//! cargo run -p dhoby-ghaut --example triso_atops_web --release -- --headless <rung>
//! cargo test -p dhoby-ghaut --example triso_atops_web --release
//! ```
//!
//! In the browser, `?rung=<name>` picks the rung ([`rungs::TABLE`]):
//! `triso`, `decay`, `walk`, `layers`, `failure`, `chemistry`, `release`,
//! `source-term`.
//!
//! - **Physics**: none written here; every number is a call into `boon-lay`
//!   (see [`engine`] for which call serves which rung).
//! - **Recorded, not live**: rung 8's source term (`sembawang`'s
//!   `htr10_air_ingress_kora_bound`) is too heavy for a phone; its numbers are
//!   the dispersion demo's recorded ones, shared by path so the two demos
//!   cannot disagree ([`recorded`]).
//! - **No lagging**: every calculation runs in a background thread natively
//!   and in a Web Worker in the browser; the UI thread only draws.
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`): not for reactor
//! operation, licensing, safety decisions or emergency response, and nothing
//! here is a source term for any facility.

#[cfg(not(target_os = "android"))]
mod app;
#[cfg(not(target_os = "android"))]
mod engine;
/// The dispersion demo's recorded capstone, the same file (rung 8); this demo
/// uses only the release table.
#[cfg(not(target_os = "android"))]
#[allow(dead_code)]
#[path = "../dispersion_web/recorded.rs"]
mod recorded;
#[cfg(not(target_os = "android"))]
mod rungs;

/// Android stub: windowing GUIs are out of scope on Termux (the workspace
/// example rule: a blanked file gives "main function not found").
#[cfg(target_os = "android")]
fn main() {
    eprintln!("triso_atops_web is a windowing GUI and is not built for Android.");
}

/// Natively: `--rung <name>`, the twin of the URL query; `--headless <rung>`
/// prints that rung's default calculation as CSV with no window.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--headless") {
        let name = args.get(i + 1).map(String::as_str).unwrap_or("failure");
        let rung = dhoby_ghaut::web_demo::lesson::parse::<rungs::Rung>(name)
            .ok_or_else(|| format!("unknown rung '{name}'"))?;
        return headless(rung);
    }
    let rung: rungs::Rung =
        dhoby_ghaut::web_demo::lesson::from_query(&dhoby_ghaut::web_demo::platform::query_pairs());
    eframe::run_native(
        "TRISO-ATOPS demo",
        eframe::NativeOptions::default(),
        Box::new(move |cc| Ok(Box::new(app::TrisoApp::new(cc, rung)))),
    )
    .map_err(|e| e.to_string())
}

/// One rung's default calculation (the app's default controls) as CSV:
/// `series,label,x,y` rows, then `scalar,index,value` rows.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn headless(rung: rungs::Rung) -> Result<(), String> {
    use engine::Request;
    let req = match rung {
        rungs::Rung::Triso => Request::Slice { id: 1, geometry: 0, cells: 200 },
        rungs::Rung::Decay => Request::Decay { id: 1, nuclide: 0, t_over_half: 1.0 },
        rungs::Rung::Walk => Request::WalkCheck { id: 1, temp_c: 1200.0 },
        rungs::Rung::Layers => Request::Layers { id: 1, nuclide: 0, temp_c: 1600.0 },
        rungs::Rung::Failure => Request::Failure {
            id: 1,
            irr_c: 776.0,
            hold_c: 1800.0,
            hours: 500.0,
            cursor_h: 500.0,
        },
        rungs::Rung::Chemistry => Request::Chemistry { id: 1, o2_kpa: 21.3, steam_kpa: 5.0, h2_kpa: 0.0 },
        rungs::Rung::Release => Request::Release {
            id: 1,
            irr_c: 776.0,
            hold_c: 1800.0,
            hours: 500.0,
            f_hm: 1.0e-4,
            k_plate: 7.5e-5,
            k_clean: 8.77e-5,
            // The app's default: HTR-10's 1 %/day primary-helium leakage
            // (Liu & Cao 2002, Section 2.4.1).
            k_leak: 0.01 / 86_400.0,
        },
        rungs::Rung::SourceTerm => {
            println!("nuclide,released_bq,core_bq");
            for (n, bq, core) in recorded::RELEASE {
                println!("{n},{bq:e},{core:e}");
            }
            return Ok(());
        }
    };
    let mut out = Vec::new();
    engine::Engine::default().serve(req, &mut |e| out.push(e));
    match out.pop() {
        Some(engine::Event::Frame(f)) if rung == rungs::Rung::Release => {
            // Every nuclide at the end of the hold.
            let (n, p) = engine::release_dims(&f);
            println!("nuclide,group,source_atoms_per_s,circulating_bq,plated_bq,hps_bq,leaked_atoms,graphite_bq");
            for i in 0..n {
                let lam = engine::release_header(&f, i, 1);
                let v = |k| engine::release_value(&f, i, p - 1, k);
                println!(
                    "{},{},{:e},{:e},{:e},{:e},{:e},{:e}",
                    f.names[i],
                    engine::GROUPS[f.tags[i] as usize],
                    v(engine::rel::S),
                    lam * v(engine::rel::C),
                    lam * v(engine::rel::P),
                    lam * v(engine::rel::H),
                    v(engine::rel::LEAKED),
                    lam * v(engine::rel::G)
                );
            }
            Ok(())
        }
        Some(engine::Event::Frame(f)) => {
            println!("kind,label,x,y");
            for s in &f.series {
                for (x, y) in s.xs.iter().zip(&s.ys) {
                    println!("series,\"{}\",{x:e},{y:e}", s.label);
                }
            }
            for (i, v) in f.scalars.iter().enumerate() {
                println!("scalar,{i},,{v:e}");
            }
            Ok(())
        }
        Some(engine::Event::Error(e)) => Err(e),
        None => Err("no answer".into()),
    }
}

/// In the browser this module runs twice: on the page, where it starts the
/// egui app, and in the Web Worker (`web/triso_atops/worker.js`), where there
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
            .get_element_by_id("triso_canvas")
            .expect("no #triso_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#triso_canvas is not a canvas");
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(move |cc| Ok(Box::new(app::TrisoApp::new(cc, rung)))),
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
