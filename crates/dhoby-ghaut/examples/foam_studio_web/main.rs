//! **Foam Studio** (gh:#1155, stage 1 of epic gh:#1153): put a cylinder or a
//! box in a 3-D viewport, move and scale it, tag its faces inlet, outlet or
//! wall, mesh it, and run OUTRAM PARK's pimpleFoam port on it in the
//! reader's browser, with velocity and pressure on two slices, the residual
//! history, a probe and the velocity profile against Hagen–Poiseuille.
//! Built on `bugis::web_demo` (panel, dock, worker link, field and residual
//! painting); published at `demos/foam-studio/` (`web/foam_studio/`).
//!
//! ```text
//! cargo run  -p dhoby-ghaut --example foam_studio_web --release
//! cargo run  -p dhoby-ghaut --example foam_studio_web --release -- --headless [cells] [re] [steady|transient]
//! cargo run  -p dhoby-ghaut --example foam_studio_web --release -- --render-mesh [out-dir]
//! cargo test -p dhoby-ghaut --example foam_studio_web --release
//! ```
//!
//! - **Physics**: none written here. Meshes from
//!   `outram_foam_mesh::block_primitives` (the pipe O-grid, the box block);
//!   the solve is `outram_foam_appbuilder_lib`'s `PimpleFoam` with
//!   `cases::laminar_pipe`'s boundary conditions. V&V record:
//!   `crates/outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155.md`.
//! - **Cells**: 20k by default (interactive), 50k at most as a batch run with
//!   a progress bar (maintainer decision, 2026-11-03).
//! - **Solve modes**: steady by pseudo-transient PISO at Co ≈ 4 (the
//!   default), or Co ≈ 1 to watch the profile develop.
//! - **No lag**: each worker message is a mesh or one chunk of time steps
//!   ([`engine`]); the page only draws.
//! - `--headless` runs the same engine natively and prints its progress
//!   (the engine CLAUDE.md headless rule; regression test in [`engine`]).
//! - `--render-mesh` writes the drawing rule's mesh images, from the
//!   `PolyMesh` the solver is built from.
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`): not for reactor
//! operation, licensing or safety decisions.

#[cfg(not(target_os = "android"))]
mod app;
#[cfg(not(target_os = "android"))]
mod engine;
#[cfg(not(target_os = "android"))]
mod render;

/// Android stub: windowing GUIs are out of scope on Termux (the workspace
/// example rule: a blanked file gives "main function not found").
#[cfg(target_os = "android")]
fn main() {
    eprintln!("foam_studio_web is a windowing GUI and is not built for Android.");
}

/// The mesh images of the drawing rule: for each design, a cross-section and
/// an axial slice of the mesh the solver gets, drawn through
/// `outram_blender`'s mesh plotter. CLI glue (file output); the meshes come
/// from the tested `engine::mesh_design`.
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn render_mesh(out: &std::path::Path) -> Result<(), String> {
    use outram_blender::csg::plot::slice::PlotBasis;
    use outram_blender::unstructured::convert::foam::from_poly_mesh;
    use outram_blender::unstructured::{render_mesh_slice_annotated, MeshColourBy, MeshSlice};
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let pipe = |cells: usize, length: f64| engine::Design {
        shape: engine::Shape::Cylinder { radius: 0.01, length, centre: [0.0, 0.0, 0.5 * length] },
        target_cells: cells,
        ..engine::Design::default()
    };
    let cases = [
        ("demo_20k", pipe(engine::DEFAULT_CELLS, 0.2), "DEMO PIPE"),
        ("vv_m3", pipe(17_280, 0.4), "VV PIPE L/D=20, M=3"),
        ("vv_m5", pipe(80_000, 0.4), "VV PIPE L/D=20, M=5"),
    ];
    for (tag, d, label) in cases {
        let (poly, _, _) = if tag.starts_with("vv_") {
            // The V&V grids are the similar family `PipeOGrid::level`, not the
            // demo's; m = 3 and 5.
            let lvl = if tag == "vv_m3" { 3 } else { 5 };
            let len = uom::si::f64::Length::new::<uom::si::length::meter>;
            let g = outram_foam_mesh::block_primitives::PipeOGrid::level(len(0.01), len(0.4), lvl);
            (g.mesh().map_err(|e| e.to_string())?, 2, 0.0)
        } else {
            engine::mesh_design(&d)?
        };
        let m = from_poly_mesh(&poly.to_foam_poly_mesh()).map_err(|e| format!("{e:?}"))?;
        let len_cm = match d.shape {
            engine::Shape::Cylinder { length, .. } => length * 100.0,
            engine::Shape::Box { size, .. } => size[2] * 100.0,
        };
        let shots = [
            (PlotBasis::Xy, [0.0, 0.0, 0.5 * len_cm + 0.013], [2.2, 2.2], 900, format!("{label}, {} CELLS, X-Y AT Z = {:.2} CM", m.n_cells(), 0.5 * len_cm + 0.013), "xy"),
            (PlotBasis::Xz, [0.0, 0.013, 2.0], [2.2, 4.4], 500, format!("{label}, X-Z AT Y = 0.013 CM, INLET END"), "xz_inlet"),
        ];
        for (basis, origin, width, px, title, view) in shots {
            let py = ((px as f64) * width[1] / width[0]).round() as usize;
            let sl = MeshSlice { basis, origin, width, pixels: [px, py], colour_by: MeshColourBy::Cell };
            let img = render_mesh_slice_annotated(&m, &sl, &title);
            let name = format!("{tag}_{view}.png");
            img.write_png(&out.join(&name)).map_err(|e| format!("{e:?}"))?;
            println!("wrote {name}: {} cells, volume {:.6e} m3", m.n_cells(), m.total_volume());
        }
    }
    Ok(())
}

/// Natively: the GUI, or a command-line mode. `main` and CLI glue: exempt
/// from the reaching-test rule; each mode calls tested code
/// ([`engine::headless`], [`engine::mesh_design`]).
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--headless") => {
            let cells = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(engine::DEFAULT_CELLS);
            let re = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(100.0);
            let mode = if args.get(3).map(String::as_str) == Some("transient") { engine::Mode::Transient } else { engine::Mode::Steady };
            let d = engine::Design { target_cells: cells, ..engine::Design::default() };
            for l in engine::headless(d, re, mode, 5000)? {
                println!("{l}");
            }
            Ok(())
        }
        Some("--render-mesh") => {
            let out = args.get(1).cloned().unwrap_or_else(|| {
                concat!(env!("CARGO_MANIFEST_DIR"), "/../outram-foam-appbuilder-lib/verification_and_validation/laminar_pipe_gh1155").to_string()
            });
            render_mesh(std::path::Path::new(&out))
        }
        _ => eframe::run_native(
            "Foam Studio",
            eframe::NativeOptions::default(),
            Box::new(move |cc| Ok(Box::new(app::FoamApp::new(cc)))),
        )
        .map_err(|e| e.to_string()),
    }
}

/// In the browser this module runs twice: on the page, where it starts the
/// egui app, and in the Web Worker (`web/foam_studio/worker.js`), where there
/// is no `window` and it serves the engine instead. Start-up glue (exempt).
#[cfg(target_arch = "wasm32")]
fn main() {
    use eframe::wasm_bindgen::JsCast as _;
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    if js_sys::global().dyn_ref::<web_sys::DedicatedWorkerGlobalScope>().is_some() {
        bugis::web_demo::link::worker_main::<engine::Engine>();
        return;
    }
    let web_options = eframe::WebOptions { renderer: eframe::Renderer::Wgpu, ..Default::default() };
    wasm_bindgen_futures::spawn_local(async move {
        let document = web_sys::window().expect("no window").document().expect("no document");
        let canvas = document
            .get_element_by_id("foam_canvas")
            .expect("no #foam_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#foam_canvas is not a canvas");
        let started = eframe::WebRunner::new().start(canvas, web_options, Box::new(move |cc| Ok(Box::new(app::FoamApp::new(cc))))).await;
        if let Some(el) = document.get_element_by_id("loading") {
            match started {
                Ok(()) => el.remove(),
                Err(e) => el.set_inner_html(&format!("<p>Failed to start: {e:?}</p>")),
            }
        }
    });
}
