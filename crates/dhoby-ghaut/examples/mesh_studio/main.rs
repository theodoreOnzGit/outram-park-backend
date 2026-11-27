//! **Mesh Studio** — volume-mesh a closed surface through `outram-park-fork-cfmesh`'s
//! **tet → dual → boundary-layers** pipeline and look **inside** the result:
//! every pipeline stage, every boundary layer, and the cell quality.
//!
//! One app for desktop and browser (gh:#1767), on `bugis::web_demo`:
//!
//! ```text
//!   author surface       cfMesh, staged (gh:#1765)              cut + colour (gh:#1766)
//!   (blender primitive   carve → snap → tet → Delaunay        one polygon per cut cell,
//!    or procedural)  ──► → dual → smooth → layers      ──►     by layer index or quality
//!                        (every stage's mesh kept, layer cells numbered)
//! ```
//!
//! - [`model`]: authoring and meshing, no GUI (unchanged from the desktop app,
//!   now on the staged route; the finished mesh is bit-identical).
//! - [`section`]: cut-plane sections and the per-cell colourings.
//! - [`engine`]: meshing and cutting off the UI thread (a thread natively, a
//!   Web Worker in the browser), with a wire format tested natively.
//! - [`app`]: the page: main view, folding side panel, report dock.
//! - [`headless`]: the CSV sweep behind the committed fixture.
//!
//! **Untrusted AI-assisted draft pending human V&V.** Offline demonstration
//! only — education / research / V&V, per the workspace `RESPONSIBLE_USE.md`.
//! Not for reactor operation, licensing, or safety-critical decisions.
//!
//! # Running it
//!
//! ```text
//! cargo run -p dhoby-ghaut --example mesh_studio --release
//! cargo run -p dhoby-ghaut --example mesh_studio --release -- --shape sphere --n 3
//! crates/dhoby-ghaut/web/mesh_studio/build.sh       # the browser build
//! ```
//!
//! # Headless mode — required, not optional
//!
//! ```text
//! cargo run -p dhoby-ghaut --example mesh_studio --release -- --headless [case]
//! ```
//!
//! Emits one CSV row per case on stdout, with a stable header and fixed
//! precision, so a run diffs cleanly against a committed fixture. Pass a
//! case name (`box`, `sphere`, `cylinder`, `subdivided-cube`) to run just
//! that one, `sweep` for every case crossed with every cleanup mode; with no
//! argument it runs all four.
//!
//! The workspace `CLAUDE.md` makes this a hard rule, and the reason is
//! worth restating: an agent or a CI job cannot open a window, so without
//! a headless path the model can only be checked by a human watching it —
//! which means in practice it is not checked at all, and every claim about
//! what the studio does becomes unfalsifiable. The headless path drives
//! [`model`] directly: no window, no event loop, no spawned thread.
//!
//! Target-gated OFF Android (windowing GUI); the library stays headless.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(not(target_os = "android"))]
mod app;
#[cfg(not(target_os = "android"))]
mod engine;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
pub mod headless;
#[cfg(not(target_os = "android"))]
pub mod model;
#[cfg(not(target_os = "android"))]
mod section;
#[cfg(not(any(target_os = "android", target_arch = "wasm32")))]
mod tetgen_render;

/// Android build: no windowing GUI. An empty main keeps `cargo check` clean.
#[cfg(target_os = "android")]
fn main() {}

/// Native: `--headless [case]` prints the CSV with no window at all;
/// otherwise the app opens. Start-up glue (exempt from the reaching-test rule).
#[cfg(all(not(target_os = "android"), not(target_arch = "wasm32")))]
fn main() -> eframe::Result<()> {
    // The headless path must be reachable before anything touches eframe,
    // or a machine with no display cannot run it at all.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(k) = args.iter().position(|a| a == "--tetgen-render") {
        // The TetGen review images (gh:#1794, #1795), into the given folder.
        let dir = args.get(k + 1).cloned().unwrap_or_else(|| ".".into());
        // Optional TetGen switches (default -pzA; stage 6, gh:#1795, draws -pq1.414a...zA).
        let sw = args.get(k + 2).cloned().unwrap_or_else(|| "pzA".into());
        match tetgen_render::render_all(std::path::Path::new(&dir), &sw) {
            Ok(lines) => lines.iter().for_each(|l| println!("{l}")),
            Err(e) => eprintln!("tetgen render failed: {e}"),
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--measure") {
        // One row of the gh:#1796 mesh-type table: the page's query keys
        // as `--key value`, e.g. `--measure --shape pipe --tets tetgen --n 3`.
        let q = bugis::web_demo::platform::query_pairs();
        println!("{}", headless::measure(&q));
        return Ok(());
    }
    if args.iter().any(|a| a == "--headless") {
        let case = args.iter().find(|a| !a.starts_with("--")).cloned();
        print!("{}", headless::run(case.as_deref()));
        return Ok(());
    }

    env_logger::init();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 820.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Mesh Studio — OUTRAM PARK (offline demo)",
        native_options,
        Box::new(|cc| Ok(Box::new(app::MeshApp::new(cc)))),
    )
}

/// In the browser this module runs twice: on the page, where it starts the
/// egui app, and in the Web Worker (`web/mesh_studio/worker.js`), where there
/// is no `window` and it serves the engine instead. Start-up glue (exempt).
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
            .get_element_by_id("mesh_canvas")
            .expect("no #mesh_canvas")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("#mesh_canvas is not a canvas");
        let started = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(move |cc| Ok(Box::new(app::MeshApp::new(cc)))),
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

#[cfg(all(test, not(target_os = "android")))]
mod tests {
    use super::headless;
    use super::model::{build, Cleanup, GeomKind, Params};

    /// The committed fixture, regenerated with
    /// `cargo run -p dhoby-ghaut --example mesh_studio --release -- --headless sweep`.
    const FIXTURE: &str = include_str!("../../tests/fixtures/mesh_studio_sweep.csv");

    /// Determinism is the property every committed fixture depends on:
    /// same config in, byte-identical trace out.
    #[test]
    fn the_headless_sweep_is_deterministic() {
        let a = headless::run_sweep();
        let b = headless::run_sweep();
        assert_eq!(a, b, "two runs of the same sweep must agree byte for byte");
    }

    /// And it must still match what was committed.
    #[test]
    fn the_headless_sweep_matches_the_committed_fixture() {
        let got = headless::run_sweep();
        assert_eq!(
            got, FIXTURE,
            "the sweep changed; if that is intended, regenerate the fixture \
             with `--headless sweep` and say in the commit what moved and why"
        );
    }

    /// Bounds the model must not leave. A harness check on the mesher's
    /// own reported quality, **not** physics V&V — it catches divergence,
    /// it does not validate anything.
    #[test]
    fn every_case_produces_a_valid_mesh_within_loose_bounds() {
        for kind in GeomKind::PRIMITIVES {
            for cleanup in Cleanup::ALL {
                let p = Params {
                    kind,
                    cleanup,
                    ..Default::default()
                };
                let b = build(&p).unwrap_or_else(|e| panic!("{kind:?}/{cleanup:?}: {e}"));
                let label = format!("{}/{}", kind.slug(), cleanup.slug());

                assert!(b.report.valid, "{label}: mesh reported invalid");
                assert_eq!(
                    b.report.n_negative_volume_cells, 0,
                    "{label}: inverted cells"
                );
                assert!(b.report.cell_count > 0, "{label}: empty mesh");
                assert!(
                    b.report.total_volume > 0.0 && b.report.total_volume.is_finite(),
                    "{label}: volume {}",
                    b.report.total_volume
                );
                // Non-orthogonality: boundary-layer meshes legitimately
                // exceed checkMesh's 70 deg warning near the wall, so the
                // bound here only catches outright divergence.
                assert!(
                    b.report.max_non_orthogonality_deg < 90.0,
                    "{label}: non-orthogonality {} deg",
                    b.report.max_non_orthogonality_deg
                );
                // Skewness: checkMesh's own limit is 4.0 and cfmesh
                // documents exceeding it near the wall (its reference case
                // records ~14). 30 is a divergence catch, not a standard.
                assert!(
                    b.report.max_skewness < 30.0,
                    "{label}: skewness {}",
                    b.report.max_skewness
                );
            }
        }
    }

    /// The regression this fixture exists for.
    ///
    /// # What happened
    ///
    /// `connect_nonplanar` originally drove its recursion from one global
    /// work stack, which shuffled the output face order even when it split
    /// nothing. Nothing in the operator's own tests noticed — they checked
    /// face and vertex counts, not order — and the geometry was identical
    /// either way.
    ///
    /// Handing that shuffled surface to the cfmesh pipeline was not
    /// harmless. Measured on the Catmull-Clark cube:
    ///
    /// | | cells | max skewness |
    /// |---|---|---|
    /// | shuffled face order | 2837 | 23.735 |
    /// | input order preserved | 1632 | 0.901 |
    ///
    /// A 26x difference in skewness, and 74 % more cells, from nothing but
    /// the order the faces arrived in. The first reading of that sweep
    /// looked like "splitting non-planar faces wrecks the volume mesh",
    /// which would have been a wrong and rather memorable conclusion.
    ///
    /// This test pins the corrected behaviour: cleanup on a surface whose
    /// faces are already planar must change **nothing**, and cleanup that
    /// does split must not blow the skewness up.
    #[test]
    fn surface_cleanup_does_not_wreck_the_volume_mesh() {
        // Planar-faced primitives: cleanup must be a complete no-op.
        for kind in [GeomKind::Box, GeomKind::Sphere, GeomKind::Cylinder] {
            let base = build(&Params {
                kind,
                cleanup: Cleanup::None,
                ..Default::default()
            })
            .expect("base build");
            for cleanup in [
                Cleanup::SplitNonPlanar,
                Cleanup::SplitConcave,
                Cleanup::SplitBoth,
            ] {
                let c = build(&Params {
                    kind,
                    cleanup,
                    ..Default::default()
                })
                .expect("cleanup build");
                assert_eq!(
                    c.surface_faces,
                    base.surface_faces,
                    "{}/{:?}: cleanup should not touch an already-clean surface",
                    kind.slug(),
                    cleanup
                );
                assert_eq!(c.report.cell_count, base.report.cell_count);
                assert!((c.report.max_skewness - base.report.max_skewness).abs() < 1e-9);
            }
        }

        // The Catmull-Clark cube genuinely has warped quads, so splitting
        // them does something — and must stay sane while doing it.
        let base = build(&Params {
            kind: GeomKind::SubdividedCube,
            cleanup: Cleanup::None,
            ..Default::default()
        })
        .expect("base");
        let split = build(&Params {
            kind: GeomKind::SubdividedCube,
            cleanup: Cleanup::SplitNonPlanar,
            ..Default::default()
        })
        .expect("split");

        assert_eq!(
            split.surface_faces,
            2 * base.surface_faces,
            "quads -> triangle pairs"
        );
        assert!(
            (split.report.total_volume - base.report.total_volume).abs() < 1e-3,
            "splitting must not change the enclosed volume: {} vs {}",
            base.report.total_volume,
            split.report.total_volume
        );
        assert!(
            split.report.max_skewness < 2.0,
            "skewness after splitting should stay near the unsplit 0.67, got {} \
             (it was 23.7 with the face-ordering bug)",
            split.report.max_skewness
        );
    }

    /// The reactor shapes (gh:#1768) mesh, carry their named patches, and
    /// grow boundary layers where the measured defaults say they do.
    ///
    /// **Methodology.** Each shape at its default design (cell-centre dual,
    /// corner feature points, back-off layers, 3 layers asked), through the
    /// same `build` the page calls. The enclosed volume is compared with the
    /// volume of the input surface itself (`shapes::surface_volume`), which
    /// is the mesher's target; snapping to the facets is the only expected
    /// loss.
    ///
    /// **Results (measured 2026-11-26, release).** Pipe 11857 cells, layers
    /// 1568 / 1568 / 1568; fuel channel 22204 cells, layers 3567 / 3584 /
    /// 3529; pebble 10412 cells, **no layer** (the back-off refuses every
    /// thickness there; cfMesh's optimiser grows all three in 77 s, which is
    /// too slow for this test). With the per-corner median dual instead, none
    /// of the three grew a layer and the pebble's dual was refused.
    ///
    /// **Since gh:#1796 both tet sources** (the page's default became
    /// TetGen). The figures above are the carved source's. On TetGen's
    /// tetrahedra (2026-11-26): pipe 6826 cells, layers 1088 × 3; fuel
    /// channel 14313 cells, 2638 × 3; pebble 7186 cells, **no layer** with
    /// back-off. The TetGen volumes are the surface's to round-off (its
    /// boundary *is* the surface); the pebble keeps its `pebble` patch
    /// only because the cavity gets a TetGen hole point
    /// (`outram_park_fork_cfmesh::pipeline::tet_source::hole_points`;
    /// without one TetGen meshed the pebble's inside, found by this test).
    ///
    /// **CHANGED 2026-11-26 (gh:#1829):** the layers are grown first and
    /// need TetGen, so the carved source is meshed bare (patches and volume
    /// still checked) and the layered builds are TetGen's; on TetGen all
    /// three shapes, the pebble included, now carry all 3 layers (the
    /// figures above are the removed back-off route's).
    #[test]
    fn reactor_shapes_mesh_with_named_patches_and_wall_layers() {
        for tetgen in [false, true] {
            use super::engine::Design;
            use super::model::reactor_surface;
            for (kind, patches, layered) in [
                (GeomKind::Pipe, vec!["inlet", "outlet", "walls"], true),
                (
                    GeomKind::FuelChannel,
                    vec!["channel", "inlet", "outlet", "pin"],
                    true,
                ),
                (
                    GeomKind::Pebble,
                    vec!["channel", "inlet", "outlet", "pebble"],
                    true,
                ),
            ] {
                let d = Design {
                    kind,
                    cell_size: kind.default_cell_size(2.0),
                    dual: kind.default_dual(),
                    tetgen,
                    n_layers: if tetgen { 3 } else { 0 },
                    ..Design::default()
                };
                let p = d.params();
                let b = build(&p).unwrap_or_else(|e| panic!("{kind:?}: {e}"));
                assert!(
                    b.report.valid && b.report.n_negative_volume_cells == 0,
                    "{kind:?}"
                );
                let mut names: Vec<&str> = b.mesh.patches.iter().map(|q| q.name.as_str()).collect();
                names.sort_unstable();
                assert_eq!(names, patches, "{kind:?}: patches");
                let (pts, tris, _, _) = reactor_surface(kind, p.size);
                let v = outram_park_fork_cfmesh::shapes::surface_volume(&pts, &tris);
                let rel = (b.report.total_volume - v).abs() / v;
                assert!(
                    rel < 5e-3,
                    "{kind:?}: volume {} vs surface {v} (rel {rel:e})",
                    b.report.total_volume
                );
                assert_eq!(b.layer_index.unreached, 0, "{kind:?}");
                if layered && tetgen {
                    assert_eq!(
                        b.layer_index.max,
                        3,
                        "{kind:?}: layers {:?}",
                        b.layer_index.counts()
                    );
                }
            }
        }
    }
}
