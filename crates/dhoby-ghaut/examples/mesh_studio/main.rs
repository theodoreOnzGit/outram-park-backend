//! **Mesh Studio** — a Blender-inspired egui app that authors a surface in
//! `outram-blender`, volume-meshes it through the `outram-park-fork-cfmesh`
//! **tet → dual → boundary-layers** pipeline, shows the mesh statistics and
//! per-stage degradation notes, and exports an OpenFOAM `polyMesh`.
//!
//! It drives the [`outram_blender::foam_mesh`] bridge:
//!
//! ```text
//!   author surface        foam_mesh::mesh_to_tet_dual         foam_mesh::export_polymesh
//!   (primitive or          (carve → snap → tet → Delaunay      (points/faces/owner/
//!    procedural mesh)  ──►  → dual → smooth → prism layers) ──►  neighbour/boundary)
//! ```
//!
//! The source is a blender-authored [`outram_blender::mesh::Mesh`] — a built-in
//! box / UV-sphere / cylinder primitive, or a *procedurally-authored* mesh (a
//! Catmull-Clark-subdivided cube) — so the app exercises the real
//! blender→cfmesh surface path, not cfmesh's own primitives. Edit the
//! [`TetDualOptions`] with sliders/checkboxes, hit **Generate**, and read the
//! [`TetDualReport`] (cell count, volume, validity, max non-orthogonality,
//! skewness, negative cells, and each `stage_notes` line so you see which stages
//! gracefully degraded). Meshing runs on a background thread so the GUI never
//! blocks; the boundary surface is a rotatable 2D wireframe.
//!
//! **Untrusted AI-assisted draft pending human V&V.** Offline demonstration only
//! — education / research / V&V, per the workspace `RESPONSIBLE_USE.md`. Not for
//! reactor operation, licensing, or safety-critical decisions.
//!
//! # Running it
//!
//! ```text
//! cargo run -p dhoby-ghaut --example mesh_studio --release
//! ```
//!
//! (The `foam-mesh` feature comes from this crate's own dependency line; it
//! no longer needs passing on the command line, and the crate is
//! `dhoby-ghaut`, not `outram-blender` — the studios moved on 2026-09-17.)
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
//! that one; with no argument it runs all four.
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

// Android build: no windowing GUI. An empty main keeps `cargo check` clean.
#[cfg(target_os = "android")]
fn main() {}

#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result<()> {
    // The headless path must be reachable before anything touches eframe,
    // or a machine with no display cannot run it at all.
    let args: Vec<String> = std::env::args().skip(1).collect();
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
        Box::new(|_cc| Ok(Box::new(app::MeshStudio::default()))),
    )
}

#[cfg(not(target_os = "android"))]
pub mod model {
    //! The studio's model, with no GUI in it.
    //!
    //! Everything here is callable from a test or from `--headless`. That
    //! separation is the whole point: the GUI module below owns only
    //! widgets and drawing, so nothing about what the studio *computes*
    //! depends on a window existing.

    use outram_blender::foam_mesh::{mesh_to_tet_dual, TetDualOptions, TetDualReport, VolumeMesh};
    use outram_blender::mesh::Mesh;
    use outram_blender::primitives::{cube, cylinder, uv_sphere};
    use outram_blender::subdivision::catmull_clark;

    /// Which blender-authored surface to volume-mesh.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum GeomKind {
        Box,
        Sphere,
        Cylinder,
        /// A procedurally-authored blender mesh: a cube run through one
        /// level of Catmull-Clark subdivision (a rounded, closed genus-0
        /// blob) — proof the bridge accepts an arbitrary authored surface,
        /// not just a primitive.
        SubdividedCube,
    }

    impl GeomKind {
        pub fn label(self) -> &'static str {
            match self {
                GeomKind::Box => "Box (primitive)",
                GeomKind::Sphere => "UV-sphere (primitive)",
                GeomKind::Cylinder => "Cylinder (primitive)",
                GeomKind::SubdividedCube => "Subdivided cube (procedural)",
            }
        }

        /// Short machine-readable name, used by `--headless` and by the CSV.
        pub fn slug(self) -> &'static str {
            match self {
                GeomKind::Box => "box",
                GeomKind::Sphere => "sphere",
                GeomKind::Cylinder => "cylinder",
                GeomKind::SubdividedCube => "subdivided-cube",
            }
        }

        pub fn from_slug(s: &str) -> Option<Self> {
            Self::ALL.iter().copied().find(|k| k.slug() == s)
        }

        pub const ALL: [GeomKind; 4] = [
            GeomKind::Box,
            GeomKind::Sphere,
            GeomKind::Cylinder,
            GeomKind::SubdividedCube,
        ];
    }

    /// Optional surface-cleanup stage, run between authoring and meshing.
    ///
    /// These are the mesh-quality operators ported from Blender in
    /// `outram-blender`. They matter here specifically: the cfmesh pipeline
    /// takes the surface as the truth about the geometry, and a warped or
    /// concave face has no single well-defined normal, centroid or area for
    /// it to work from.
    #[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
    pub enum Cleanup {
        /// Hand the authored surface over untouched.
        #[default]
        None,
        /// Split faces that are non-planar by more than 5 degrees
        /// ([`outram_blender::connect_nonplanar`]). Adds edges, moves
        /// nothing.
        SplitNonPlanar,
        /// Split concave faces into convex pieces
        /// ([`outram_blender::connect_concave`]). Adds edges, moves
        /// nothing.
        SplitConcave,
        /// Both splits, non-planar first.
        SplitBoth,
    }

    impl Cleanup {
        pub fn label(self) -> &'static str {
            match self {
                Cleanup::None => "None (author surface as-is)",
                Cleanup::SplitNonPlanar => "Split non-planar faces (>5 deg)",
                Cleanup::SplitConcave => "Split concave faces",
                Cleanup::SplitBoth => "Split non-planar, then concave",
            }
        }

        pub fn slug(self) -> &'static str {
            match self {
                Cleanup::None => "none",
                Cleanup::SplitNonPlanar => "nonplanar",
                Cleanup::SplitConcave => "concave",
                Cleanup::SplitBoth => "both",
            }
        }

        pub const ALL: [Cleanup; 4] = [
            Cleanup::None,
            Cleanup::SplitNonPlanar,
            Cleanup::SplitConcave,
            Cleanup::SplitBoth,
        ];
    }

    /// The immutable authoring + meshing parameters handed to the worker
    /// thread, or to the headless driver.
    #[derive(Clone, Debug)]
    pub struct Params {
        pub kind: GeomKind,
        pub size: f64,
        pub cyl_radius: f64,
        pub cyl_height: f64,
        pub sphere_lat: usize,
        pub sphere_lon: usize,
        pub cleanup: Cleanup,
        pub opts: TetDualOptions,
    }

    impl Default for Params {
        fn default() -> Self {
            Params {
                kind: GeomKind::Box,
                size: 2.0,
                cyl_radius: 2.0,
                cyl_height: 5.0,
                sphere_lat: 12,
                sphere_lon: 24,
                cleanup: Cleanup::None,
                opts: TetDualOptions {
                    cell_size: 0.5,
                    first_layer_thickness: 0.02,
                    ..Default::default()
                },
            }
        }
    }

    /// A generated volume mesh plus its report.
    pub struct Built {
        pub mesh: VolumeMesh,
        pub report: TetDualReport,
        /// Face count of the surface actually handed to the mesher, after
        /// any [`Cleanup`]. Recorded so the CSV shows what cleanup did.
        pub surface_faces: usize,
    }

    /// Author the blender surface [`Mesh`] for `p`, before cleanup.
    ///
    /// This is deterministic: the primitives and Catmull-Clark are pure
    /// functions of the parameters, with no clock and no RNG.
    pub fn author_surface(p: &Params) -> Mesh {
        match p.kind {
            GeomKind::Box => cube(p.size),
            GeomKind::Sphere => uv_sphere(p.sphere_lon.max(3), p.sphere_lat.max(2), p.size),
            GeomKind::Cylinder => cylinder(24, p.cyl_radius, p.cyl_height),
            // One Catmull-Clark level turns the 6-quad cube into a rounded,
            // still-closed genus-0 surface — a genuine authored
            // (non-primitive) mesh to feed the bridge.
            GeomKind::SubdividedCube => catmull_clark(&cube(p.size), 1),
        }
    }

    /// Apply the chosen [`Cleanup`] to an authored surface.
    pub fn clean_surface(surface: &Mesh, cleanup: Cleanup) -> Mesh {
        use outram_blender::connect_nonplanar::{connect_nonplanar, DEFAULT_ANGLE_LIMIT};
        match cleanup {
            Cleanup::None => surface.clone(),
            Cleanup::SplitNonPlanar => connect_nonplanar(surface, DEFAULT_ANGLE_LIMIT),
            Cleanup::SplitConcave => outram_blender::connect_concave::connect_concave(surface),
            Cleanup::SplitBoth => outram_blender::connect_concave::connect_concave(
                &connect_nonplanar(surface, DEFAULT_ANGLE_LIMIT),
            ),
        }
    }

    /// Author, clean, and volume-mesh — the whole model in one call, with
    /// no GUI and no thread.
    pub fn build(p: &Params) -> Result<Built, String> {
        let authored = author_surface(p);
        let surface = clean_surface(&authored, p.cleanup);
        let surface_faces = surface.face_count();
        mesh_to_tet_dual(&surface, &p.opts).map(|(mesh, report)| Built {
            mesh,
            report,
            surface_faces,
        })
    }
}

/// The headless driver: run cases and emit CSV, no window involved.
#[cfg(not(target_os = "android"))]
pub mod headless {
    use super::model::{build, GeomKind, Params};

    /// Stable CSV header. Keep this and the row format in lockstep — a
    /// committed fixture diffs against both.
    pub const CSV_HEADER: &str =
        "case,cleanup,surface_faces,cells,volume,valid,max_non_orth_deg,max_skewness,neg_vol_cells,stage_notes
";

    /// Run one case and format its row. Fixed precision so the output is
    /// byte-stable across runs and machines.
    pub fn run_case(p: &Params) -> String {
        match build(p) {
            Ok(b) => format!(
                "{},{},{},{},{:.6},{},{:.4},{:.6},{},{}
",
                p.kind.slug(),
                p.cleanup.slug(),
                b.surface_faces,
                b.report.cell_count,
                b.report.total_volume,
                b.report.valid,
                b.report.max_non_orthogonality_deg,
                b.report.max_skewness,
                b.report.n_negative_volume_cells,
                b.report.stage_notes.len(),
            ),
            Err(e) => format!(
                "{},{},0,0,0.000000,false,0.0000,0.000000,0,ERROR:{}\n",
                p.kind.slug(),
                p.cleanup.slug(),
                e.replace(',', ";")
            ),
        }
    }

    /// Every case crossed with every cleanup mode — the sweep that shows
    /// what the surface-quality operators are worth to the mesher.
    pub fn run_sweep() -> String {
        use super::model::Cleanup;
        let mut out = String::from(CSV_HEADER);
        for kind in GeomKind::ALL {
            for cleanup in Cleanup::ALL {
                let p = Params {
                    kind,
                    cleanup,
                    ..Default::default()
                };
                out.push_str(&run_case(&p));
            }
        }
        out
    }

    /// Run `case` (or every case when `None`) and return the whole CSV.
    pub fn run(case: Option<&str>) -> String {
        if case == Some("sweep") {
            return run_sweep();
        }
        let kinds: Vec<GeomKind> = match case {
            Some(name) => match GeomKind::from_slug(name) {
                Some(k) => vec![k],
                None => {
                    return format!(
                        "# unknown case {name:?}; known cases: {}\n",
                        GeomKind::ALL
                            .iter()
                            .map(|k| k.slug())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            },
            None => GeomKind::ALL.to_vec(),
        };
        let mut out = String::from(CSV_HEADER);
        for kind in kinds {
            let p = Params {
                kind,
                ..Default::default()
            };
            out.push_str(&run_case(&p));
        }
        out
    }
}

#[cfg(not(target_os = "android"))]
mod app {
    use std::sync::{Arc, RwLock};

    use super::model::{build, Built, Cleanup, GeomKind, Params};
    use outram_blender::foam_mesh::{export_polymesh, Vec3};

    /// Shared slot the background mesher writes into.
    #[derive(Default)]
    struct Slot {
        running: bool,
        /// Set once when a rebuild finishes; the GUI `take()`s it into `current`.
        done: Option<Result<Built, String>>,
    }

    pub struct MeshStudio {
        p: Params,
        yaw: f32,
        pitch: f32,
        export_dir: String,
        export_msg: String,
        slot: Arc<RwLock<Slot>>,
        current: Option<Result<Built, String>>,
    }

    impl Default for MeshStudio {
        fn default() -> Self {
            Self {
                p: Params::default(),
                yaw: 0.6,
                pitch: 0.5,
                export_dir: "/tmp/mesh_studio/polyMesh".into(),
                export_msg: String::new(),
                slot: Arc::new(RwLock::new(Slot::default())),
                current: None,
            }
        }
    }

    impl MeshStudio {
        fn launch_build(&self) {
            {
                let mut s = self.slot.write().unwrap();
                if s.running {
                    return;
                }
                s.running = true;
                s.done = None;
            }
            let p = self.p.clone();
            let slot = self.slot.clone();
            std::thread::spawn(move || {
                // Exactly the call `--headless` makes, so the GUI cannot
                // drift from what the headless path reports.
                let result = build(&p);
                let mut s = slot.write().unwrap();
                s.running = false;
                s.done = Some(result);
            });
        }

        /// Move a finished result out of the shared slot into app-owned state.
        fn poll(&mut self) {
            let taken = {
                let mut s = self.slot.write().unwrap();
                s.done.take()
            };
            if let Some(r) = taken {
                self.current = Some(r);
            }
        }
    }

    impl eframe::App for MeshStudio {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            self.poll();
            let running = self.slot.read().unwrap().running;

            egui::Panel::top("ms_top").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Mesh Studio");
                    ui.label("· blender surface → cfmesh tet→dual→layers → OpenFOAM polyMesh (offline demo)");
                    egui::global_theme_preference_buttons(ui);
                });
                ui.separator();
            });

            egui::Panel::right("ms_controls")
                .min_size(350.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.controls_ui(ui, running));
                });

            egui::CentralPanel::default().show(ui, |ui| self.center_ui(ui, running));

            if running {
                ui.ctx().request_repaint();
            }
        }
    }

    impl MeshStudio {
        fn controls_ui(&mut self, ui: &mut egui::Ui, running: bool) {
            ui.heading("Source surface (blender-authored)");
            for k in [
                GeomKind::Box,
                GeomKind::Sphere,
                GeomKind::Cylinder,
                GeomKind::SubdividedCube,
            ] {
                ui.radio_value(&mut self.p.kind, k, k.label());
            }
            match self.p.kind {
                GeomKind::Cylinder => {
                    ui.add(
                        egui::Slider::new(&mut self.p.cyl_radius, 0.5..=10.0).text("radius [m]"),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.p.cyl_height, 0.5..=20.0).text("height [m]"),
                    );
                }
                GeomKind::Sphere => {
                    ui.add(egui::Slider::new(&mut self.p.size, 0.5..=10.0).text("radius [m]"));
                    ui.add(
                        egui::Slider::new(&mut self.p.sphere_lon, 6..=64).text("longitude segs"),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.p.sphere_lat, 3..=32).text("latitude bands"),
                    );
                }
                _ => {
                    ui.add(egui::Slider::new(&mut self.p.size, 0.5..=10.0).text("size [m]"));
                }
            }

            ui.separator();
            ui.heading("Surface cleanup (before meshing)");
            ui.label(
                "Mesh-quality operators ported from Blender. A warped or concave \
                 face has no single well-defined normal, centroid or area, and the \
                 cfmesh pipeline takes the surface as the truth about the geometry. \
                 Both add edges only — no vertex moves, so the surface is unchanged.",
            );
            for c in Cleanup::ALL {
                ui.radio_value(&mut self.p.cleanup, c, c.label());
            }

            ui.separator();
            ui.heading("Meshing pipeline (TetDualOptions)");
            let o = &mut self.p.opts;
            ui.add(egui::Slider::new(&mut o.cell_size, 0.1..=2.0).text("background cell size [m]"));
            ui.checkbox(&mut o.snap, "snap to surface (body-fit)");
            ui.checkbox(&mut o.delaunay, "Delaunay flip-improve tets");
            ui.checkbox(&mut o.dual, "polyhedral dual");
            ui.add_enabled_ui(o.dual, |ui| {
                ui.checkbox(&mut o.dual_min_faces, "  face-minimal dual");
            });
            ui.add(
                egui::Slider::new(&mut o.smooth_passes, 0..=5).text("Laplacian smoothing passes"),
            );

            ui.separator();
            ui.heading("Boundary layers");
            ui.add(egui::Slider::new(&mut o.n_layers, 0..=8).text("prism layers"));
            ui.add_enabled_ui(o.n_layers > 0, |ui| {
                ui.add(
                    egui::Slider::new(&mut o.first_layer_thickness, 0.005..=0.3)
                        .text("first thickness [m]"),
                );
                ui.add(egui::Slider::new(&mut o.expansion, 1.0..=2.0).text("expansion ratio"));
            });

            ui.separator();
            ui.add_enabled_ui(!running, |ui| {
                if ui
                    .add(egui::Button::new("⚙  Generate mesh").min_size(egui::vec2(150.0, 32.0)))
                    .clicked()
                {
                    self.launch_build();
                }
            });
            if running {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("meshing…");
                });
            }

            ui.separator();
            ui.heading("Export");
            ui.label("OpenFOAM polyMesh directory:");
            ui.text_edit_singleline(&mut self.export_dir);
            let have_mesh = matches!(&self.current, Some(Ok(_)));
            ui.add_enabled_ui(have_mesh, |ui| {
                if ui.button("💾  Write polyMesh").clicked() {
                    if let Some(Ok(built)) = &self.current {
                        let dir = std::path::PathBuf::from(&self.export_dir);
                        self.export_msg = match export_polymesh(&built.mesh, &dir) {
                            Ok(()) => format!("wrote polyMesh → {}", dir.display()),
                            Err(e) => format!("export failed: {e}"),
                        };
                    }
                }
            });
            if !self.export_msg.is_empty() {
                ui.label(&self.export_msg);
            }
        }

        fn center_ui(&mut self, ui: &mut egui::Ui, running: bool) {
            let avail = ui.available_size();
            let view_h = (avail.y * 0.6).max(240.0);
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(avail.x, view_h), egui::Sense::drag());
            if response.dragged() {
                let d = response.drag_delta();
                self.yaw += d.x * 0.01;
                self.pitch = (self.pitch + d.y * 0.01).clamp(-1.5, 1.5);
            }
            self.draw_wireframe(ui, rect);
            ui.label("volume-mesh boundary-surface wireframe · drag to orbit");
            ui.separator();

            match &self.current {
                None => {
                    if running {
                        ui.label("Generating volume mesh on a background thread…");
                    } else {
                        ui.label("Pick a source surface + pipeline options on the right, then Generate mesh.");
                    }
                }
                Some(Err(e)) => {
                    ui.colored_label(
                        egui::Color32::from_rgb(220, 80, 80),
                        format!("Mesh generation failed: {e}"),
                    );
                }
                Some(Ok(b)) => {
                    let r = &b.report;
                    ui.heading(format!("{} cells", r.cell_count));
                    egui::Grid::new("stats")
                        .num_columns(2)
                        .striped(true)
                        .show(ui, |ui| {
                            ui.label("valid (closed, in-range)");
                            ui.label(if r.valid { "✔ yes" } else { "✘ no" });
                            ui.end_row();
                            ui.label("total volume [m³]");
                            ui.label(format!("{:.5}", r.total_volume));
                            ui.end_row();
                            ui.label("max non-orthogonality [°]");
                            ui.label(format!("{:.1}", r.max_non_orthogonality_deg));
                            ui.end_row();
                            ui.label("max skewness");
                            ui.label(format!("{:.3}", r.max_skewness));
                            ui.end_row();
                            ui.label("negative-volume cells");
                            ui.label(format!("{}", r.n_negative_volume_cells));
                            ui.end_row();
                        });

                    if r.valid && r.n_negative_volume_cells == 0 {
                        if r.max_non_orthogonality_deg < 70.0 {
                            ui.colored_label(
                                egui::Color32::from_rgb(90, 200, 120),
                                "✔ valid · within checkMesh non-orthogonality warning",
                            );
                        } else {
                            // Near-wall prism layers are intrinsically non-orthogonal;
                            // exceeding checkMesh's 70° warning is expected and is
                            // handled by a solver's non-orthogonal correctors.
                            ui.colored_label(
                                egui::Color32::from_rgb(150, 200, 120),
                                "✔ valid, no inverted cells · high near-wall non-orthogonality is normal for boundary layers (use non-orthogonal correctors)",
                            );
                        }
                    } else {
                        ui.colored_label(
                            egui::Color32::from_rgb(230, 170, 60),
                            "⚠ mesh below acceptance thresholds",
                        );
                    }

                    // Stage-degradation notes — the whole point of showing the
                    // report: the user sees exactly which stages were skipped.
                    if r.stage_notes.is_empty() {
                        ui.label("all requested pipeline stages ran (no degradation)");
                    } else {
                        ui.label("pipeline stage notes:");
                        for n in &r.stage_notes {
                            ui.colored_label(
                                egui::Color32::from_rgb(230, 170, 60),
                                format!("• {n}"),
                            );
                        }
                    }
                }
            }
        }

        /// Draw the volume mesh's boundary faces as a rotatable 2D wireframe.
        fn draw_wireframe(&self, ui: &egui::Ui, rect: egui::Rect) {
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
            let Some(Ok(b)) = &self.current else { return };
            let m = &b.mesh;
            if m.points.is_empty() {
                return;
            }
            let proj: Vec<egui::Vec2> = m.points.iter().map(|p| self.project(*p)).collect();
            let (mut lo, mut hi) = (proj[0], proj[0]);
            for p in &proj {
                lo = egui::vec2(lo.x.min(p.x), lo.y.min(p.y));
                hi = egui::vec2(hi.x.max(p.x), hi.y.max(p.y));
            }
            let span = (hi - lo).max(egui::vec2(1e-3, 1e-3));
            let margin = 24.0;
            let scale = ((rect.width() - 2.0 * margin) / span.x)
                .min((rect.height() - 2.0 * margin) / span.y);
            let center = rect.center();
            let mid = (lo + hi) * 0.5;
            let to_screen =
                |v: egui::Vec2| center + egui::vec2((v.x - mid.x) * scale, -(v.y - mid.y) * scale);
            let stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 200, 160));
            // Only boundary faces (neighbour == None) — the visible surface.
            for f in 0..m.face_count() {
                if m.neighbour[f].is_some() {
                    continue;
                }
                let ring = &m.faces[f];
                let k = ring.len();
                for i in 0..k {
                    let a = to_screen(proj[ring[i]]);
                    let c = to_screen(proj[ring[(i + 1) % k]]);
                    painter.line_segment([a, c], stroke);
                }
            }
        }

        /// Rotate a point (yaw about Y, then pitch about X) → orthographic (x, y).
        fn project(&self, p: Vec3) -> egui::Vec2 {
            let (cy, sy) = (self.yaw.cos() as f64, self.yaw.sin() as f64);
            let x1 = p.x * cy + p.z * sy;
            let z1 = -p.x * sy + p.z * cy;
            let y1 = p.y;
            let (cp, sp) = (self.pitch.cos() as f64, self.pitch.sin() as f64);
            let y2 = y1 * cp - z1 * sp;
            egui::vec2(x1 as f32, y2 as f32)
        }
    }
}

/// Regression tests for the headless path, called directly — never through
/// the GUI, which is the point.
///
/// Run with `cargo test -p dhoby-ghaut --examples --release`.
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
        for kind in GeomKind::ALL {
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
}
