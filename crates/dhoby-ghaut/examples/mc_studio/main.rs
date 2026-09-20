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
//! # Headless mode — required, not optional
//!
//! ```text
//! cargo run -p dhoby-ghaut --example mc_studio --release -- --headless [case]
//! ```
//!
//! Emits one CSV row per case on stdout, with a stable header and fixed
//! precision. Cases are `godiva`, `sphere-csg`, `box`, `cylinder`; with no
//! argument all four run.
//!
//! The workspace `CLAUDE.md` makes this a hard rule. It matters more here
//! than for a geometry tool: a criticality calculation returns a number
//! that looks plausible whatever it is, so "the GUI showed 1.0-ish" is not
//! evidence of anything. The headless path drives [`model`] directly and
//! prints `k_eff` and its standard deviation to fixed precision, which a
//! test can actually check.
//!
//! **Single-threaded on purpose.** The headless runs force
//! `ComputeType::CpuSingleThread` so the trace is byte-reproducible; the
//! GUI still offers the multi-threaded path. The Monte Carlo driver
//! documents its results as thread-count-independent, but a fixture should
//! not depend on that holding.
//!
//! Target-gated OFF Android (windowing GUI); the library stays headless.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Android build: no windowing GUI. An empty main keeps `cargo check` clean.
#[cfg(target_os = "android")]
fn main() {}

#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result<()> {
    // Reachable before anything touches eframe, so a machine with no
    // display can still run it.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--headless") {
        let case = args.iter().find(|a| !a.starts_with("--")).cloned();
        print!("{}", headless::run(case.as_deref()));
        return Ok(());
    }

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
pub mod model {
    //! The studio's model, with no GUI in it — so `--headless` and the
    //! tests can drive exactly what the GUI drives.

    use outram_blender::primitives::{cube, cylinder, uv_sphere};
    use outram_blender::sim::{
        csg_from_mesh, ComputeType, KeffSettings, MaterialSpec, McSimSetup, SimGeometry,
        ThreadCount,
    };

    /// Which geometry to author and run.
    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    pub enum GeomKind {
        /// The analytic bare-sphere driver — no CSG, fastest path.
        BareSphere,
        SphereCsg,
        Box,
        Cylinder,
    }

    impl GeomKind {
        pub fn label(self) -> &'static str {
            match self {
                GeomKind::BareSphere => "Bare sphere (fast driver)",
                GeomKind::SphereCsg => "Sphere (CSG)",
                GeomKind::Box => "Box (CSG)",
                GeomKind::Cylinder => "Cylinder (CSG)",
            }
        }

        pub fn slug(self) -> &'static str {
            match self {
                GeomKind::BareSphere => "godiva",
                GeomKind::SphereCsg => "sphere-csg",
                GeomKind::Box => "box",
                GeomKind::Cylinder => "cylinder",
            }
        }

        pub fn from_slug(s: &str) -> Option<Self> {
            Self::ALL.iter().copied().find(|k| k.slug() == s)
        }

        pub const ALL: [GeomKind; 4] = [
            GeomKind::BareSphere,
            GeomKind::SphereCsg,
            GeomKind::Box,
            GeomKind::Cylinder,
        ];
    }

    /// Everything the run needs, with no GUI state in it.
    #[derive(Clone, Debug)]
    pub struct Params {
        pub kind: GeomKind,
        pub radius: f64,
        pub box_dims: [f64; 3],
        pub cyl_radius: f64,
        pub cyl_height: f64,
        pub mat_name: String,
        pub mat_temp_k: f64,
        pub nuclides: Vec<(String, f64)>,
        pub n_particles: usize,
        pub n_inactive: usize,
        pub n_active: usize,
        /// Multi-threaded execution. The headless path forces this off so
        /// its trace is byte-reproducible.
        pub multithread: bool,
    }

    impl Default for Params {
        fn default() -> Self {
            // Godiva HEU at the ICSBEP critical radius.
            Params {
                kind: GeomKind::BareSphere,
                radius: 8.7407,
                box_dims: [10.0, 10.0, 10.0],
                cyl_radius: 7.0,
                cyl_height: 15.0,
                mat_name: "Godiva HEU".into(),
                mat_temp_k: 293.6,
                nuclides: vec![
                    ("U234".into(), 4.9184e-4),
                    ("U235".into(), 4.4994e-2),
                    ("U238".into(), 2.4984e-3),
                ],
                n_particles: 2000,
                n_inactive: 10,
                n_active: 30,
                multithread: false,
            }
        }
    }

    /// Assemble the backend simulation from `p`.
    pub fn build_setup(p: &Params) -> Result<McSimSetup, String> {
        let material = MaterialSpec {
            name: p.mat_name.clone(),
            temperature_k: p.mat_temp_k,
            nuclides: p
                .nuclides
                .iter()
                .filter(|(n, _)| !n.trim().is_empty())
                .cloned()
                .collect(),
        };
        let geometry = match p.kind {
            GeomKind::BareSphere => SimGeometry::BareSphere {
                radius_cm: p.radius,
            },
            GeomKind::SphereCsg => {
                csg_from_mesh(&uv_sphere(32, 16, p.radius), 0).map_err(|e| e.to_string())?
            }
            GeomKind::Box => {
                // The exporter fits an axis-aligned box, so size it by the
                // largest requested dimension.
                let s = p.box_dims[0].max(p.box_dims[1]).max(p.box_dims[2]);
                csg_from_mesh(&cube(s), 0).map_err(|e| e.to_string())?
            }
            GeomKind::Cylinder => csg_from_mesh(&cylinder(48, p.cyl_radius, p.cyl_height), 0)
                .map_err(|e| e.to_string())?,
        };
        let compute = if p.multithread {
            ComputeType::CpuMultiThread(ThreadCount::Auto)
        } else {
            ComputeType::CpuSingleThread
        };
        Ok(McSimSetup {
            geometry,
            materials: vec![material],
            settings: KeffSettings {
                n_particles: p.n_particles.max(1),
                n_inactive: p.n_inactive,
                n_active: p.n_active.max(1),
                compute,
                ..Default::default()
            },
        })
    }

    /// A finished run, reduced to what the GUI and the CSV both want.
    #[derive(Clone, Default, Debug)]
    pub struct Outcome {
        pub k_mean: f64,
        pub k_std: f64,
        pub k_by_gen: Vec<f64>,
    }

    /// Build and run — the whole model in one call, no GUI, no thread.
    pub fn run(p: &Params) -> Result<Outcome, String> {
        let setup = build_setup(p)?;
        setup
            .run()
            .map(|r| Outcome {
                k_mean: r.k_mean,
                k_std: r.k_std,
                k_by_gen: r.k_by_generation,
            })
            .map_err(|e| e.to_string())
    }
}

/// The headless driver: run cases and emit CSV, no window involved.
#[cfg(not(target_os = "android"))]
pub mod headless {
    use super::model::{self, GeomKind, Params};

    /// Stable CSV header; keep in lockstep with the row format.
    pub const CSV_HEADER: &str = "case,n_particles,n_inactive,n_active,k_mean,k_std,generations
";

    pub fn run_case(p: &Params) -> String {
        match model::run(p) {
            Ok(o) => format!(
                "{},{},{},{},{:.6},{:.6},{}
",
                p.kind.slug(),
                p.n_particles,
                p.n_inactive,
                p.n_active,
                o.k_mean,
                o.k_std,
                o.k_by_gen.len(),
            ),
            Err(e) => format!(
                "{},{},{},{},NaN,NaN,ERROR:{}
",
                p.kind.slug(),
                p.n_particles,
                p.n_inactive,
                p.n_active,
                e.replace(',', ";")
            ),
        }
    }

    /// Run `case`, or every case when `None`.
    pub fn run(case: Option<&str>) -> String {
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
            // Single-threaded so the trace is reproducible.
            let p = Params {
                kind,
                multithread: false,
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

    use outram_blender::math::Vec3;
    use outram_blender::mesh::Mesh;
    use outram_blender::primitives::{cube, cylinder, uv_sphere};

    // One definition of the geometry set and the run outcome, shared with
    // the headless path so the two cannot drift.
    use super::model::{self, GeomKind, Outcome, Params};

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

        /// Gather the current UI state into the shared [`Params`].
        fn params(&self) -> Params {
            Params {
                kind: self.kind,
                radius: self.radius,
                box_dims: self.box_dims,
                cyl_radius: self.cyl_radius,
                cyl_height: self.cyl_height,
                mat_name: self.mat_name.clone(),
                mat_temp_k: self.mat_temp_k,
                nuclides: self.nuclides.clone(),
                n_particles: self.n_particles,
                n_inactive: self.n_inactive,
                n_active: self.n_active,
                multithread: self.multithread,
            }
        }

        /// Spawn the run on a background thread so the GUI never blocks.
        ///
        /// The thread calls exactly the `model::run` the headless path
        /// calls; nothing about the calculation lives in the GUI.
        fn launch_run(&self) {
            let p = self.params();
            // Surface a setup error immediately rather than on the thread.
            if let Err(e) = model::build_setup(&p) {
                *self.run.write().unwrap() = RunStatus::Failed(e);
                return;
            }
            *self.run.write().unwrap() = RunStatus::Running;
            let slot = self.run.clone();
            std::thread::spawn(move || {
                let status = match model::run(&p) {
                    Ok(o) => RunStatus::Done(o),
                    Err(e) => RunStatus::Failed(e),
                };
                *slot.write().unwrap() = status;
            });
        }
    }

    impl eframe::App for McStudio {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            let status = self.run.read().unwrap().clone();
            let running = matches!(status, RunStatus::Running);

            egui::Panel::top("mc_top").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("MC Studio");
                    ui.label("· author geometry → set up → run a basic outram-mc criticality (offline demo)");
                    egui::global_theme_preference_buttons(ui);
                });
                ui.separator();
            });

            egui::Panel::right("mc_controls")
                .min_size(320.0)
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        self.controls_ui(ui, running);
                    });
                });

            egui::CentralPanel::default().show(ui, |ui| {
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

/// Regression tests for the headless path, called directly.
///
/// Run with `cargo test -p dhoby-ghaut --examples --release`.
#[cfg(all(test, not(target_os = "android")))]
mod tests {
    use super::headless;
    use super::model::{self, GeomKind, Params};

    /// Regenerate with
    /// `cargo run -p dhoby-ghaut --example mc_studio --release -- --headless`.
    const FIXTURE: &str = include_str!("../../tests/fixtures/mc_studio_cases.csv");

    /// Byte-reproducibility is what a committed fixture rests on. Note the
    /// headless path forces single-threaded execution precisely so this
    /// holds without depending on the driver's thread-independence.
    #[test]
    fn the_headless_run_is_deterministic() {
        assert_eq!(headless::run(None), headless::run(None));
    }

    #[test]
    fn the_headless_run_matches_the_committed_fixture() {
        assert_eq!(
            headless::run(None),
            FIXTURE,
            "the traces changed; if intended, regenerate the fixture and say \
             in the commit what moved and why"
        );
    }

    /// Bounds every case must stay inside. A harness check against
    /// divergence — **not** V&V. Nothing here validates the physics; it
    /// catches a run that has gone obviously wrong.
    #[test]
    fn every_case_returns_a_finite_positive_k_with_sane_statistics() {
        for kind in GeomKind::ALL {
            let p = Params {
                kind,
                multithread: false,
                ..Default::default()
            };
            let o = model::run(&p).unwrap_or_else(|e| panic!("{}: {e}", kind.slug()));
            let label = kind.slug();

            assert!(
                o.k_mean.is_finite() && o.k_mean > 0.0,
                "{label}: k_eff = {}",
                o.k_mean
            );
            assert!(
                o.k_mean < 2.0,
                "{label}: k_eff = {} is implausible for a bare HEU assembly \
                 of this size",
                o.k_mean
            );
            assert!(
                o.k_std.is_finite() && o.k_std > 0.0 && o.k_std < 0.05,
                "{label}: sigma = {}",
                o.k_std
            );
            assert_eq!(
                o.k_by_gen.len(),
                p.n_inactive + p.n_active,
                "{label}: generation count"
            );
        }
    }

    /// A genuine consistency check the studio gets for free: the analytic
    /// bare-sphere driver and the faceted-CSG path describe the same
    /// physical assembly, so they must agree.
    ///
    /// # Methodology
    ///
    /// Run `godiva` (analytic `SimGeometry::BareSphere` at r = 8.7407 cm)
    /// and `sphere-csg` (the same radius as a 32x16 UV sphere pushed
    /// through `csg_from_mesh`) at identical material and settings, and
    /// compare. The faceted sphere is slightly smaller than the true one,
    /// so it should read marginally lower, but well inside the combined
    /// statistical uncertainty.
    ///
    /// # Results (measured 2026-09-20, 2000 particles x 30 active gens)
    ///
    /// | case | k_eff | sigma |
    /// |---|---|---|
    /// | godiva (analytic) | 1.010876 | 0.005487 |
    /// | sphere-csg (faceted) | 1.009360 | 0.005880 |
    ///
    /// A difference of **152 pcm** against a combined sigma of ~800 pcm —
    /// consistent, and in the expected direction. This exercises the
    /// `csg_from_mesh` bridge against a path that does not use it.
    ///
    /// **Not a validation of either.** Both sit ~1100 pcm above the ICSBEP
    /// Godiva benchmark's critical k = 1.0000, which is about 2 sigma at
    /// this deliberately low statistics; `outram-mc-libs`' own declared
    /// 500 pcm bar is measured on its own case with proper statistics, not
    /// on this studio path. What this test checks is that the two agree
    /// with *each other*.
    #[test]
    fn the_analytic_and_csg_sphere_paths_agree() {
        let analytic = model::run(&Params {
            kind: GeomKind::BareSphere,
            multithread: false,
            ..Default::default()
        })
        .expect("analytic");
        let faceted = model::run(&Params {
            kind: GeomKind::SphereCsg,
            multithread: false,
            ..Default::default()
        })
        .expect("faceted");

        let diff = (analytic.k_mean - faceted.k_mean).abs();
        let combined = (analytic.k_std.powi(2) + faceted.k_std.powi(2)).sqrt();
        assert!(
            diff < 3.0 * combined,
            "analytic {} +/- {} vs faceted {} +/- {}: {} apart, more than 3 \
             combined sigma ({})",
            analytic.k_mean,
            analytic.k_std,
            faceted.k_mean,
            faceted.k_std,
            diff,
            combined
        );
    }
}
