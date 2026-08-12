//! **MC Studio** — a Blender-inspired egui app to author geometry and set up +
//! run a *basic* outram-mc Monte Carlo (k-eigenvalue / criticality) simulation.
//!
//! It drives the [`outram_blender::sim`] backend: pick a geometry, define a
//! material, choose run settings, hit **Run**, and read `k_eff ± σ` with the
//! per-generation eigenvalue plot. Geometry is shown as a rotatable 2D wireframe
//! (orthographic, drag to orbit).
//!
//! Offline demonstration only — education / research / V&V, per the workspace
//! `RESPONSIBLE_USE.md`. **Not** for reactor operation, licensing, or
//! safety-critical decisions.
//!
//! Run (needs the `mc-export` feature → outram-mc-libs + the `sim` backend):
//! ```text
//! cargo run -p outram-blender --example mc_studio --features mc-export --release
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
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
        ..Default::default()
    };
    eframe::run_native(
        "MC Studio — OUTRAM PARK (offline demo)",
        native_options,
        Box::new(|_cc| Ok(Box::new(app::McStudio::default()))),
    )
}

#[cfg(not(target_os = "android"))]
mod app {
    use std::sync::{Arc, RwLock};

    use outram_blender::math::Vec3;
    use outram_blender::mesh::Mesh;
    use outram_blender::primitives::{cube, cylinder, uv_sphere};
    use outram_blender::sim::{
        csg_from_mesh, ComputeType, KeffSettings, MaterialSpec, McSimSetup, SimGeometry,
        ThreadCount,
    };

    /// Which geometry to author / run.
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum GeomKind {
        BareSphere,
        SphereCsg,
        Box,
        Cylinder,
    }

    impl GeomKind {
        fn label(self) -> &'static str {
            match self {
                GeomKind::BareSphere => "Bare sphere (fast driver)",
                GeomKind::SphereCsg => "Sphere (CSG)",
                GeomKind::Box => "Box (CSG)",
                GeomKind::Cylinder => "Cylinder (CSG)",
            }
        }
    }

    /// A material preset the user can load into the editable table.
    struct Preset {
        name: &'static str,
        temperature_k: f64,
        nuclides: &'static [(&'static str, f64)],
    }

    const PRESETS: &[Preset] = &[
        Preset {
            name: "Godiva HEU",
            temperature_k: 293.6,
            nuclides: &[
                ("U234", 4.9184e-4),
                ("U235", 4.4994e-2),
                ("U238", 2.4984e-3),
            ],
        },
        Preset {
            name: "UO2 (3% enr.)",
            temperature_k: 900.0,
            nuclides: &[("U235", 6.9e-4), ("U238", 2.17e-2), ("O16", 4.48e-2)],
        },
        Preset {
            name: "Pu-239 metal",
            temperature_k: 293.6,
            nuclides: &[("Pu239", 3.93e-2)],
        },
    ];

    /// Result of a finished run (a small owned copy; `KeffResult` stays in the
    /// backend).
    #[derive(Clone, Default)]
    struct Outcome {
        k_mean: f64,
        k_std: f64,
        k_by_gen: Vec<f64>,
    }

    #[derive(Clone)]
    enum RunStatus {
        Idle,
        Running,
        Done(Outcome),
        Failed(String),
    }

    /// The app.
    pub struct McStudio {
        // Geometry.
        kind: GeomKind,
        radius: f64,
        box_dims: [f64; 3],
        cyl_radius: f64,
        cyl_height: f64,
        // Material (editable name/temperature + nuclide rows).
        mat_name: String,
        mat_temp_k: f64,
        nuclides: Vec<(String, f64)>,
        // Run settings.
        n_particles: usize,
        n_inactive: usize,
        n_active: usize,
        multithread: bool,
        // Viewport orbit.
        yaw: f32,
        pitch: f32,
        // Shared run state (background thread writes, GUI reads).
        run: Arc<RwLock<RunStatus>>,
    }

    impl Default for McStudio {
        fn default() -> Self {
            let p = &PRESETS[0];
            Self {
                kind: GeomKind::BareSphere,
                radius: 8.7407,
                box_dims: [10.0, 10.0, 10.0],
                cyl_radius: 7.0,
                cyl_height: 15.0,
                mat_name: p.name.to_string(),
                mat_temp_k: p.temperature_k,
                nuclides: p
                    .nuclides
                    .iter()
                    .map(|(n, d)| (n.to_string(), *d))
                    .collect(),
                n_particles: 2000,
                n_inactive: 20,
                n_active: 50,
                multithread: true,
                yaw: 0.6,
                pitch: 0.5,
                run: Arc::new(RwLock::new(RunStatus::Idle)),
            }
        }
    }

    impl McStudio {
        /// A preview mesh for the current geometry (drawn as a wireframe).
        fn preview_mesh(&self) -> Mesh {
            match self.kind {
                GeomKind::BareSphere | GeomKind::SphereCsg => uv_sphere(24, 12, self.radius),
                GeomKind::Box => cube(self.box_dims[0].max(self.box_dims[1]).max(self.box_dims[2])),
                GeomKind::Cylinder => cylinder(24, self.cyl_radius, self.cyl_height),
            }
        }

        /// Build the backend simulation from the current UI state.
        fn build_setup(&self) -> Result<McSimSetup, String> {
            let material = MaterialSpec {
                name: self.mat_name.clone(),
                temperature_k: self.mat_temp_k,
                nuclides: self
                    .nuclides
                    .iter()
                    .filter(|(n, _)| !n.trim().is_empty())
                    .cloned()
                    .collect(),
            };
            let geometry = match self.kind {
                GeomKind::BareSphere => SimGeometry::BareSphere {
                    radius_cm: self.radius,
                },
                GeomKind::SphereCsg => {
                    csg_from_mesh(&uv_sphere(32, 16, self.radius), 0).map_err(|e| e.to_string())?
                }
                GeomKind::Box => {
                    // A box CSG: use an axis-aligned cube sized by the max dim
                    // (the exporter fits an axis-aligned box).
                    let s = self.box_dims[0].max(self.box_dims[1]).max(self.box_dims[2]);
                    csg_from_mesh(&cube(s), 0).map_err(|e| e.to_string())?
                }
                GeomKind::Cylinder => {
                    csg_from_mesh(&cylinder(48, self.cyl_radius, self.cyl_height), 0)
                        .map_err(|e| e.to_string())?
                }
            };
            let compute = if self.multithread {
                ComputeType::CpuMultiThread(ThreadCount::Auto)
            } else {
                ComputeType::CpuSingleThread
            };
            Ok(McSimSetup {
                geometry,
                materials: vec![material],
                settings: KeffSettings {
                    n_particles: self.n_particles.max(1),
                    n_inactive: self.n_inactive,
                    n_active: self.n_active.max(1),
                    compute,
                    ..Default::default()
                },
            })
        }

        /// Spawn the run on a background thread so the GUI never blocks.
        fn launch_run(&self) {
            let setup = match self.build_setup() {
                Ok(s) => s,
                Err(e) => {
                    *self.run.write().unwrap() = RunStatus::Failed(e);
                    return;
                }
            };
            *self.run.write().unwrap() = RunStatus::Running;
            let slot = self.run.clone();
            std::thread::spawn(move || {
                let status = match setup.run() {
                    Ok(r) => RunStatus::Done(Outcome {
                        k_mean: r.k_mean,
                        k_std: r.k_std,
                        k_by_gen: r.k_by_generation,
                    }),
                    Err(e) => RunStatus::Failed(e.to_string()),
                };
                *slot.write().unwrap() = status;
            });
        }
    }

    impl eframe::App for McStudio {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            let status = self.run.read().unwrap().clone();
            let running = matches!(status, RunStatus::Running);

            egui::Panel::top("mc_top").show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("MC Studio");
                    ui.label("· author geometry → set up → run a basic outram-mc criticality (offline demo)");
                    egui::global_theme_preference_buttons(ui);
                });
                ui.separator();
            });

            egui::Panel::right("mc_controls")
                .min_size(320.0)
                .show_inside(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        self.controls_ui(ui, running);
                    });
                });

            egui::CentralPanel::default().show_inside(ui, |ui| {
                self.viewport_and_results_ui(ui, &status);
            });

            if running {
                ui.ctx().request_repaint(); // poll the background run
            }
        }
    }

    impl McStudio {
        fn controls_ui(&mut self, ui: &mut egui::Ui, running: bool) {
            ui.heading("Geometry");
            for k in [
                GeomKind::BareSphere,
                GeomKind::SphereCsg,
                GeomKind::Box,
                GeomKind::Cylinder,
            ] {
                ui.radio_value(&mut self.kind, k, k.label());
            }
            match self.kind {
                GeomKind::BareSphere | GeomKind::SphereCsg => {
                    ui.add(egui::Slider::new(&mut self.radius, 1.0..=30.0).text("radius [cm]"));
                }
                GeomKind::Box => {
                    ui.add(egui::Slider::new(&mut self.box_dims[0], 1.0..=40.0).text("size [cm]"));
                    self.box_dims[1] = self.box_dims[0];
                    self.box_dims[2] = self.box_dims[0];
                }
                GeomKind::Cylinder => {
                    ui.add(egui::Slider::new(&mut self.cyl_radius, 1.0..=25.0).text("radius [cm]"));
                    ui.add(egui::Slider::new(&mut self.cyl_height, 1.0..=60.0).text("height [cm]"));
                }
            }

            ui.separator();
            ui.heading("Material");
            ui.horizontal(|ui| {
                ui.label("preset:");
                for p in PRESETS {
                    if ui.button(p.name).clicked() {
                        self.mat_name = p.name.to_string();
                        self.mat_temp_k = p.temperature_k;
                        self.nuclides = p
                            .nuclides
                            .iter()
                            .map(|(n, d)| (n.to_string(), *d))
                            .collect();
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label("name:");
                ui.text_edit_singleline(&mut self.mat_name);
            });
            ui.add(egui::Slider::new(&mut self.mat_temp_k, 250.0..=2000.0).text("T [K]"));
            ui.label("nuclides (name, atoms/barn·cm):");
            let mut remove: Option<usize> = None;
            for (i, (name, dens)) in self.nuclides.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(name).desired_width(70.0));
                    ui.add(
                        egui::DragValue::new(dens)
                            .speed(1e-4)
                            .range(0.0..=1.0)
                            .max_decimals(6),
                    );
                    if ui.button("✕").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                self.nuclides.remove(i);
            }
            if ui.button("+ add nuclide").clicked() {
                self.nuclides.push(("".to_string(), 1e-2));
            }

            ui.separator();
            ui.heading("Run settings");
            ui.add(egui::Slider::new(&mut self.n_particles, 100..=20000).text("histories/gen"));
            ui.add(egui::Slider::new(&mut self.n_inactive, 0..=100).text("inactive gens"));
            ui.add(egui::Slider::new(&mut self.n_active, 1..=300).text("active gens"));
            ui.checkbox(&mut self.multithread, "multithreaded");

            ui.separator();
            ui.add_enabled_ui(!running, |ui| {
                if ui
                    .add(egui::Button::new("▶  Run").min_size(egui::vec2(120.0, 32.0)))
                    .clicked()
                {
                    self.launch_run();
                }
            });
            if running {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("running…");
                });
            }
        }

        fn viewport_and_results_ui(&mut self, ui: &mut egui::Ui, status: &RunStatus) {
            // ── Wireframe viewport (top ~55%). ────────────────────────────────
            let avail = ui.available_size();
            let view_h = (avail.y * 0.55).max(220.0);
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(avail.x, view_h), egui::Sense::drag());
            if response.dragged() {
                let d = response.drag_delta();
                self.yaw += d.x * 0.01;
                self.pitch = (self.pitch + d.y * 0.01).clamp(-1.5, 1.5);
            }
            self.draw_wireframe(ui, rect);
            ui.label("drag the viewport to orbit");

            ui.separator();

            // ── Results (bottom). ─────────────────────────────────────────────
            match status {
                RunStatus::Idle => {
                    ui.label("Set up a material + geometry on the right, then press Run.");
                }
                RunStatus::Running => {
                    ui.label("Simulation running on a background thread…");
                }
                RunStatus::Failed(e) => {
                    ui.colored_label(
                        egui::Color32::from_rgb(220, 80, 80),
                        format!("Run failed: {e}"),
                    );
                }
                RunStatus::Done(o) => {
                    ui.heading(format!("k_eff = {:.5}  ±  {:.5}", o.k_mean, o.k_std));
                    let pcm = (o.k_mean - 1.0) * 1e5;
                    ui.label(format!(
                        "{:+.0} pcm from critical  ·  {}",
                        pcm,
                        if o.k_mean > 1.0 {
                            "supercritical"
                        } else {
                            "subcritical"
                        }
                    ));
                    use egui_plot::{Line, Plot, PlotPoints};
                    let pts: Vec<[f64; 2]> = o
                        .k_by_gen
                        .iter()
                        .enumerate()
                        .map(|(i, &k)| [i as f64, k])
                        .collect();
                    Plot::new("k_by_gen").height(200.0).show(ui, |pui| {
                        pui.line(Line::new("k per active generation", PlotPoints::from(pts)));
                    });
                }
            }
        }

        /// Draw the current geometry's edges as a 2D orthographic wireframe.
        fn draw_wireframe(&self, ui: &egui::Ui, rect: egui::Rect) {
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 4.0, ui.visuals().extreme_bg_color);
            let mesh = self.preview_mesh();
            let pos = mesh.positions();
            if pos.is_empty() {
                return;
            }
            // Rotate + orthographic project every vertex, then autofit to rect.
            let proj: Vec<egui::Vec2> = pos.iter().map(|p| self.project(*p)).collect();
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
            let to_screen = |v: egui::Vec2| -> egui::Pos2 {
                center + egui::vec2((v.x - mid.x) * scale, -(v.y - mid.y) * scale)
            };
            let stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(120, 190, 255));
            for e in 0..mesh.edge_count() {
                if let Some(edge) = mesh.edge(outram_blender::mesh::EdgeId(e)) {
                    let a = to_screen(proj[edge.verts[0].0]);
                    let b = to_screen(proj[edge.verts[1].0]);
                    painter.line_segment([a, b], stroke);
                }
            }
        }

        /// Rotate a model-space point by yaw (around Y) then pitch (around X) and
        /// return its orthographic (x, y) — the 2D wireframe projection.
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
