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
//! Run (needs the `foam-mesh` feature → the cfmesh bridge + its polyMesh writer):
//! ```text
//! cargo run -p outram-blender --example mesh_studio --features foam-mesh --release
//! ```
//!
//! Target-gated OFF Android (windowing GUI); the library stays headless.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Android build: no windowing GUI. An empty main keeps `cargo check` clean.
#[cfg(target_os = "android")]
fn main() {}

#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result<()> {
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
mod app {
    use std::sync::{Arc, RwLock};

    use outram_blender::foam_mesh::{
        export_polymesh, mesh_to_tet_dual, TetDualOptions, TetDualReport, Vec3, VolumeMesh,
    };
    use outram_blender::mesh::Mesh;
    use outram_blender::primitives::{cube, cylinder, uv_sphere};
    use outram_blender::subdivision::catmull_clark;

    /// Which blender-authored surface to volume-mesh.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum GeomKind {
        Box,
        Sphere,
        Cylinder,
        /// A procedurally-authored blender mesh: a cube run through one level of
        /// Catmull-Clark subdivision (a rounded, closed genus-0 blob) — proof the
        /// bridge accepts an arbitrary authored surface, not just a primitive.
        SubdividedCube,
    }

    impl GeomKind {
        fn label(self) -> &'static str {
            match self {
                GeomKind::Box => "Box (primitive)",
                GeomKind::Sphere => "UV-sphere (primitive)",
                GeomKind::Cylinder => "Cylinder (primitive)",
                GeomKind::SubdividedCube => "Subdivided cube (procedural)",
            }
        }
    }

    /// A generated volume mesh plus its report (owned by the app for drawing).
    struct Built {
        mesh: VolumeMesh,
        report: TetDualReport,
    }

    /// Shared slot the background mesher writes into.
    #[derive(Default)]
    struct Slot {
        running: bool,
        /// Set once when a rebuild finishes; the GUI `take()`s it into `current`.
        done: Option<Result<Built, String>>,
    }

    /// The immutable authoring + meshing parameters handed to the worker thread.
    #[derive(Clone)]
    struct Params {
        kind: GeomKind,
        size: f64,
        cyl_radius: f64,
        cyl_height: f64,
        sphere_lat: usize,
        sphere_lon: usize,
        opts: TetDualOptions,
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
                p: Params {
                    kind: GeomKind::Box,
                    size: 2.0,
                    cyl_radius: 2.0,
                    cyl_height: 5.0,
                    sphere_lat: 12,
                    sphere_lon: 24,
                    opts: TetDualOptions {
                        cell_size: 0.5,
                        first_layer_thickness: 0.02,
                        ..Default::default()
                    },
                },
                yaw: 0.6,
                pitch: 0.5,
                export_dir: "/tmp/mesh_studio/polyMesh".into(),
                export_msg: String::new(),
                slot: Arc::new(RwLock::new(Slot::default())),
                current: None,
            }
        }
    }

    /// Author the blender surface [`Mesh`] for the current parameters. This is
    /// the surface the bridge triangulates and hands to the cfmesh pipeline.
    fn author_surface(p: &Params) -> Mesh {
        match p.kind {
            GeomKind::Box => cube(p.size),
            GeomKind::Sphere => uv_sphere(p.sphere_lon.max(3), p.sphere_lat.max(2), p.size),
            GeomKind::Cylinder => cylinder(24, p.cyl_radius, p.cyl_height),
            // One Catmull-Clark level turns the 6-quad cube into a rounded,
            // still-closed genus-0 surface — a genuine authored (non-primitive)
            // mesh to feed the bridge.
            GeomKind::SubdividedCube => catmull_clark(&cube(p.size), 1),
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
                let surface = author_surface(&p);
                let result = mesh_to_tet_dual(&surface, &p.opts)
                    .map(|(mesh, report)| Built { mesh, report });
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

            egui::Panel::top("ms_top").show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Mesh Studio");
                    ui.label("· blender surface → cfmesh tet→dual→layers → OpenFOAM polyMesh (offline demo)");
                    egui::global_theme_preference_buttons(ui);
                });
                ui.separator();
            });

            egui::Panel::right("ms_controls")
                .min_size(350.0)
                .show_inside(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.controls_ui(ui, running));
                });

            egui::CentralPanel::default().show_inside(ui, |ui| self.center_ui(ui, running));

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
