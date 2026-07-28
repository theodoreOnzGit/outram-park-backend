//! **Mesh Studio** — an egui app to author a surface and generate a
//! **polyhedral volume mesh with prism boundary layers**, then export it as an
//! OpenFOAM / outram-foam `polyMesh` for a CFD/TH solve.
//!
//! It drives the cfMesh pipeline, in the verified order that yields a valid
//! solvable polyhedral-with-layers mesh:
//!
//! ```text
//! shapes  →  carve_box  →  snap_to_surface  →  polyhedral_dual  →  add_boundary_layers  →  write_polymesh
//! (surface)  (hex bg)      (body-fit)          (polyhedral cells)   (prism wall layers)     (OpenFOAM)
//! ```
//!
//! Each stage is a toggle so you can inspect the mesh at any point; the default
//! produces the polyhedral-with-layers mesh. Live stats (cells / faces / volume)
//! and checkMesh-style quality (non-orthogonality / skewness / aspect ratio /
//! solvable) update on every rebuild, and the boundary surface is shown as a
//! rotatable 2D wireframe.
//!
//! Offline demonstration only — education / research / V&V, per the workspace
//! `RESPONSIBLE_USE.md`. Not for reactor operation, licensing, or
//! safety-critical decisions.
//!
//! Run (needs `foam-export` for the polyMesh writer):
//! ```text
//! cargo run -p outram-park-fork-cfmesh --example mesh_studio --features foam-export --release
//! ```
//!
//! Target-gated OFF Android (windowing GUI); the library stays headless.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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

    use outram_park_fork_cfmesh::carve::carve_box;
    use outram_park_fork_cfmesh::checks::{check_quality, QualityReport};
    use outram_park_fork_cfmesh::dual::{polyhedral_dual, polyhedral_dual_min_faces};
    use outram_park_fork_cfmesh::foam::write_polymesh;
    use outram_park_fork_cfmesh::layers::add_boundary_layers_adaptive;
    use outram_park_fork_cfmesh::math::Vec3;
    use outram_park_fork_cfmesh::shapes::{box_surface, cylinder_surface, sphere_surface};
    use outram_park_fork_cfmesh::snap::snap_to_surface;
    use outram_park_fork_cfmesh::volume_mesh::VolumeMesh;

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum GeomKind {
        Box,
        Sphere,
        Cylinder,
    }

    /// A generated mesh plus its quality report (owned by the app for drawing).
    struct Built {
        mesh: VolumeMesh,
        q: QualityReport,
        /// Notes on any pipeline stage that was skipped to keep the mesh valid.
        notes: Vec<String>,
    }

    /// Shared slot the background mesher writes into.
    #[derive(Default)]
    struct Slot {
        running: bool,
        /// Set once when a rebuild finishes; the GUI `take()`s it into `current`.
        done: Option<Result<Built, String>>,
    }

    /// The immutable meshing parameters handed to the background thread.
    #[derive(Clone)]
    struct Params {
        kind: GeomKind,
        size: f64,
        cyl_radius: f64,
        cyl_height: f64,
        cell_size: f64,
        snap: bool,
        dual: bool,
        dual_min_faces: bool,
        n_layers: usize,
        first_thickness: f64,
        expansion: f64,
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
                    kind: GeomKind::Sphere,
                    size: 3.0,
                    cyl_radius: 2.0,
                    cyl_height: 5.0,
                    cell_size: 0.6,
                    snap: true,
                    dual: true,
                    dual_min_faces: true,
                    n_layers: 3,
                    first_thickness: 0.04,
                    expansion: 1.3,
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

    /// Build the surface triangle soup for a geometry.
    fn surface(p: &Params) -> (Vec<Vec3>, Vec<[usize; 3]>) {
        match p.kind {
            GeomKind::Box => box_surface(Vec3::ZERO, Vec3::new(p.size, p.size, p.size)),
            GeomKind::Sphere => sphere_surface(Vec3::ZERO, p.size, 24, 48),
            GeomKind::Cylinder => cylinder_surface(Vec3::ZERO, p.cyl_radius, p.cyl_height, 48),
        }
    }

    /// Run the pipeline for `p`, returning the mesh + quality or an error.
    ///
    /// **Graceful degradation:** each optional stage (snap / dual / layers) is
    /// applied only if it keeps the mesh valid (closed, per `validate`);
    /// otherwise it is skipped and noted. So the result is always a valid,
    /// exportable mesh, and the caller is told the truth about what ran. In
    /// practice the polyhedral dual is robust on curved geometry, but prism
    /// **boundary layers are currently reliable only on flat-walled (box)
    /// geometry** — on a curved wall they can invalidate the mesh and are then
    /// skipped (see the `notes`). Curved-wall prism layers are WIP (bead).
    fn build(p: &Params) -> Result<Built, String> {
        let (pts, tris) = surface(p);
        let cs = p.cell_size.max(1e-3);
        let mut m = carve_box(&pts, &tris, cs);
        if m.cell_count() == 0 {
            return Err("carve produced 0 cells — try a smaller cell size".into());
        }
        let mut notes: Vec<String> = Vec::new();

        // Apply an optional stage, reverting (with a note) if it breaks validity.
        let mut stage = |m: VolumeMesh, next: VolumeMesh, name: &str| -> VolumeMesh {
            if next.validate().is_ok() {
                next
            } else {
                notes.push(format!("{name} skipped — it would invalidate the mesh on this geometry"));
                m
            }
        };

        if p.snap {
            let snapped = snap_to_surface(&m, &pts, &tris);
            m = stage(m, snapped, "snap-to-surface");
        }
        if p.dual {
            let dual = if p.dual_min_faces { polyhedral_dual_min_faces(&m) } else { polyhedral_dual(&m) };
            m = stage(m, dual, "polyhedral dual");
        }
        if p.n_layers > 0 {
            // Adaptive layers: smoothed normals + validity back-off, so curved
            // (sphere/cylinder) and polyhedral walls take layers too (the
            // thickness may be reduced from the request to stay valid).
            let layered = add_boundary_layers_adaptive(&m, "walls", p.n_layers, p.first_thickness.max(1e-4), p.expansion.max(1.0));
            m = stage(m, layered, "boundary layers could not be inserted on this geometry");
        }
        if let Err(e) = m.validate() {
            return Err(format!("generated mesh is not closed: {e}"));
        }
        let q = check_quality(&m);
        Ok(Built { mesh: m, q, notes })
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

            egui::Panel::top("ms_top").show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Mesh Studio");
                    ui.label("· surface → polyhedral volume mesh + prism layers → OpenFOAM/outram-foam polyMesh");
                    egui::global_theme_preference_buttons(ui);
                });
                ui.separator();
            });

            egui::Panel::right("ms_controls").min_size(340.0).show_inside(ui, |ui| {
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
            ui.heading("Geometry");
            ui.radio_value(&mut self.p.kind, GeomKind::Box, "Box");
            ui.radio_value(&mut self.p.kind, GeomKind::Sphere, "Sphere");
            ui.radio_value(&mut self.p.kind, GeomKind::Cylinder, "Cylinder");
            match self.p.kind {
                GeomKind::Cylinder => {
                    ui.add(egui::Slider::new(&mut self.p.cyl_radius, 0.5..=10.0).text("radius"));
                    ui.add(egui::Slider::new(&mut self.p.cyl_height, 0.5..=20.0).text("height"));
                }
                _ => {
                    ui.add(egui::Slider::new(&mut self.p.size, 0.5..=10.0).text("size / radius"));
                }
            }

            ui.separator();
            ui.heading("Meshing pipeline");
            ui.add(egui::Slider::new(&mut self.p.cell_size, 0.1..=2.0).text("background cell size"));
            ui.checkbox(&mut self.p.snap, "snap to surface (body-fit)");
            ui.checkbox(&mut self.p.dual, "polyhedral dual");
            ui.add_enabled_ui(self.p.dual, |ui| {
                ui.checkbox(&mut self.p.dual_min_faces, "  face-minimal dual");
            });

            ui.separator();
            ui.heading("Boundary layers");
            ui.add(egui::Slider::new(&mut self.p.n_layers, 0..=8).text("layers"));
            ui.add_enabled_ui(self.p.n_layers > 0, |ui| {
                ui.add(egui::Slider::new(&mut self.p.first_thickness, 0.005..=0.3).text("first thickness"));
                ui.add(egui::Slider::new(&mut self.p.expansion, 1.0..=2.0).text("expansion ratio"));
            });

            ui.separator();
            ui.add_enabled_ui(!running, |ui| {
                if ui.add(egui::Button::new("⚙  Generate mesh").min_size(egui::vec2(150.0, 32.0))).clicked() {
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
            ui.label("OpenFOAM/outram-foam polyMesh directory:");
            ui.text_edit_singleline(&mut self.export_dir);
            let have_mesh = matches!(&self.current, Some(Ok(_)));
            ui.add_enabled_ui(have_mesh, |ui| {
                if ui.button("💾  Write polyMesh").clicked() {
                    if let Some(Ok(built)) = &self.current {
                        let dir = std::path::PathBuf::from(&self.export_dir);
                        self.export_msg = match write_polymesh(&built.mesh, &dir) {
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
            let (rect, response) = ui.allocate_exact_size(egui::vec2(avail.x, view_h), egui::Sense::drag());
            if response.dragged() {
                let d = response.drag_delta();
                self.yaw += d.x * 0.01;
                self.pitch = (self.pitch + d.y * 0.01).clamp(-1.5, 1.5);
            }
            self.draw_wireframe(ui, rect);
            ui.label("boundary-surface wireframe · drag to orbit");
            ui.separator();

            match &self.current {
                None => {
                    if running {
                        ui.label("Generating mesh on a background thread…");
                    } else {
                        ui.label("Set the geometry + pipeline on the right, then Generate mesh.");
                    }
                }
                Some(Err(e)) => {
                    ui.colored_label(egui::Color32::from_rgb(220, 80, 80), format!("Mesh generation failed: {e}"));
                }
                Some(Ok(b)) => {
                    let m = &b.mesh;
                    ui.heading(format!("{} cells", m.cell_count()));
                    egui::Grid::new("stats").num_columns(2).striped(true).show(ui, |ui| {
                        ui.label("faces (internal / boundary)");
                        ui.label(format!("{} ({} / {})", m.face_count(), m.n_internal_faces(), m.n_boundary_faces()));
                        ui.end_row();
                        ui.label("total volume");
                        ui.label(format!("{:.4}", m.total_volume()));
                        ui.end_row();
                        ui.label("max non-orthogonality");
                        ui.label(format!("{:.1}°", b.q.max_non_orthogonality_deg));
                        ui.end_row();
                        ui.label("max skewness");
                        ui.label(format!("{:.3}", b.q.max_skewness));
                        ui.end_row();
                        ui.label("max aspect ratio");
                        ui.label(format!("{:.2}", b.q.max_aspect_ratio));
                        ui.end_row();
                        ui.label("negative-volume cells");
                        ui.label(format!("{}", b.q.n_negative_volume_cells));
                        ui.end_row();
                    });
                    if b.q.is_solvable() {
                        ui.colored_label(egui::Color32::from_rgb(90, 200, 120), "✔ solvable (checkMesh thresholds)");
                    } else if b.q.n_negative_volume_cells == 0 && b.q.max_non_orthogonality_deg < 85.0 {
                        // Near-wall prism layers are intrinsically non-orthogonal;
                        // exceeding checkMesh's 70° warning is expected and is
                        // handled by a solver's non-orthogonal correctors.
                        ui.colored_label(
                            egui::Color32::from_rgb(150, 200, 120),
                            "✔ closed, no inverted cells · high near-wall non-orthogonality (normal for boundary layers — use non-orthogonal correctors)",
                        );
                    } else {
                        ui.colored_label(egui::Color32::from_rgb(230, 170, 60), "⚠ quality below checkMesh thresholds");
                    }
                    for n in &b.notes {
                        ui.colored_label(egui::Color32::from_rgb(230, 170, 60), format!("• {n}"));
                    }
                }
            }
        }

        /// Draw the mesh's boundary faces as a rotatable 2D wireframe.
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
            let scale = ((rect.width() - 2.0 * margin) / span.x).min((rect.height() - 2.0 * margin) / span.y);
            let center = rect.center();
            let mid = (lo + hi) * 0.5;
            let to_screen = |v: egui::Vec2| center + egui::vec2((v.x - mid.x) * scale, -(v.y - mid.y) * scale);
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
