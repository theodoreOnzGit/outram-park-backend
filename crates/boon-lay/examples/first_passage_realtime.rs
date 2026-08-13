//! Real-time TRISO diffusion driven by the Walk-on-Spheres first-passage engine.
//!
//! An `egui`/`eframe` demonstrator that animates an ensemble of fission-product
//! atoms diffusing out of a TRISO particle. A **background worker thread** owns
//! the ensemble and advances it with the selected compute backend
//! ([`boon_lay::compute::ComputeType`] — CPU single/multi-thread or the `wgpu`
//! GPU kernel), publishing a small snapshot through an `Arc<RwLock<…>>`. The UI
//! thread only reads the latest snapshot and renders, so it never blocks on the
//! compute and stays smooth even when the SiC-barrier bounce makes a frame
//! expensive. A tap cycles the backend; the panel shows the effective backend
//! and live GPU availability.
//!
//! GUI (egui/eframe) example — out of scope for Android, which has no windowing
//! stack. On Android the real entry point and its egui-using module are gated
//! out and replaced by a no-op `main`, so the example target still builds there.

#[cfg(not(target_os = "android"))]
fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1150.0, 820.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Boon Lay — Walk-on-Spheres real-time TRISO diffusion",
        native_options,
        Box::new(|_cc| Ok(Box::new(app::RealtimeApp::new()))),
    )
}

#[cfg(target_os = "android")]
fn main() {
    println!("first_passage_realtime is a desktop-only egui example; not built on Android.");
}

#[cfg(not(target_os = "android"))]
mod app {
    use boon_lay::compute::ComputeType;
    use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::live::{
        LiveEnsemble, Snapshot,
    };
    use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::walk_on_spheres::WalkParams;
        use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell;
    use boon_lay::Nuclide;
    use egui_plot::{Legend, Line, Plot, PlotPoints, Points};
    use std::sync::{Arc, RwLock};
    use std::thread;
    use std::time::Duration;
    use uom::si::f64::{ThermodynamicTemperature, Time};
    use uom::si::length::micrometer;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::time::second;

    const TEMPERATURE_C: f64 = 1600.0;
    const NUCLIDE: Nuclide = Nuclide::Cs137;
    const SEED: u64 = 0x0B00_1A47;
    const MAX_HISTORY: usize = 4000;

    /// State shared between the UI thread and the background compute worker.
    ///
    /// Per the workspace `CLAUDE.md`, shared mutable state uses `Arc<RwLock<…>>`,
    /// not channels. The UI writes the *controls* and reads the *published*
    /// fields; the worker does the reverse. Locks are held only briefly (never
    /// across the heavy advance), so the UI never stalls.
    struct Shared {
        // --- controls: UI writes, worker reads ---
        running: bool,
        speed_s_per_frame: f64,
        compute: ComputeType,
        n_walkers: usize,
        reset_requested: bool,
        rebuild_requested: bool,
        // --- published: worker writes, UI reads ---
        snapshot: Snapshot,
        release_history: Vec<[f64; 2]>,
        // --- static (set once) ---
        gpu_available: bool,
        gpu_name: String,
        layer_radii_um: [f64; 5],
    }

    /// The live diffusion demonstrator.
    pub struct RealtimeApp {
        shared: Arc<RwLock<Shared>>,
    }

    impl RealtimeApp {
        pub fn new() -> Self {
            let cell = TrisoCell::new_crp6_geometry();
            let layer_radii_um = [
                cell.get_fuel_radius().get::<micrometer>(),
                cell.get_buffer_radius().get::<micrometer>(),
                cell.get_ipyc_radius().get::<micrometer>(),
                cell.get_sic_radius().get::<micrometer>(),
                cell.get_opyc_radius().get::<micrometer>(),
            ];
            let (gpu_available, gpu_name) = match boon_lay::gpu::cached_context() {
                Some(ctx) => (true, ctx.adapter_name().to_string()),
                None => (false, String::new()),
            };

            let shared = Arc::new(RwLock::new(Shared {
                running: false,
                speed_s_per_frame: 5.0e4,
                compute: ComputeType::default(),
                n_walkers: 3000,
                reset_requested: false,
                rebuild_requested: false,
                snapshot: Snapshot::default(),
                release_history: vec![[0.0, 0.0]],
                gpu_available,
                gpu_name,
                layer_radii_um,
            }));

            let worker_shared = Arc::clone(&shared);
            thread::spawn(move || worker(worker_shared));

            Self { shared }
        }
    }

    /// Build a fresh ensemble of `n` atoms in the CRP-6 geometry at the demo
    /// temperature.
    fn build_ensemble(n: usize) -> LiveEnsemble {
        LiveEnsemble::new(
            TrisoCell::new_crp6_geometry(),
            WalkParams::crp6_default(),
            NUCLIDE,
            ThermodynamicTemperature::new::<degree_celsius>(TEMPERATURE_C),
            n,
            SEED,
        )
    }

    /// The compute worker: advance the ensemble with the selected backend and
    /// publish a snapshot each frame. Runs for the process lifetime.
    fn worker(shared: Arc<RwLock<Shared>>) {
        let n0 = shared.read().unwrap().n_walkers;
        let mut ensemble = build_ensemble(n0);
        shared.write().unwrap().snapshot = ensemble.snapshot();

        loop {
            let (running, speed, compute, n, reset, rebuild) = {
                let s = shared.read().unwrap();
                (
                    s.running,
                    s.speed_s_per_frame,
                    s.compute,
                    s.n_walkers,
                    s.reset_requested,
                    s.rebuild_requested,
                )
            };

            if reset || rebuild {
                ensemble = build_ensemble(n);
                let mut s = shared.write().unwrap();
                s.reset_requested = false;
                s.rebuild_requested = false;
                s.release_history = vec![[0.0, 0.0]];
                s.snapshot = ensemble.snapshot();
                continue;
            }

            if !running {
                thread::sleep(Duration::from_millis(16));
                continue;
            }

            let until = ensemble.sim_time() + Time::new::<second>(speed);
            ensemble.advance_frame(compute, until); // no lock held during the heavy work
            let snapshot = ensemble.snapshot();

            let mut s = shared.write().unwrap();
            s.release_history
                .push([snapshot.sim_time_s, snapshot.released_fraction]);
            if s.release_history.len() > MAX_HISTORY {
                let overflow = s.release_history.len() - MAX_HISTORY;
                s.release_history.drain(0..overflow);
            }
            s.snapshot = snapshot;
        }
    }

    /// Honest description of the effective backend, given the selection and live
    /// GPU availability (mirrors the `outram-mc-tui` idiom).
    fn gpu_status_line(compute: ComputeType, available: bool, name: &str) -> String {
        match (compute, available) {
            (ComputeType::Gpu, true) => format!("GPU active: {name}"),
            (ComputeType::Gpu, false) => "No GPU adapter — running on the CPU path".to_string(),
            (_, true) => format!("GPU available: {name} (not selected)"),
            (_, false) => "No GPU adapter on this device".to_string(),
        }
    }

    fn circle_um(radius_um: f64) -> Vec<[f64; 2]> {
        (0..=72)
            .map(|k| {
                let a = std::f64::consts::TAU * k as f64 / 72.0;
                [radius_um * a.cos(), radius_um * a.sin()]
            })
            .collect()
    }

    impl eframe::App for RealtimeApp {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            let ctx = ui.ctx().clone();
            // Repaint at a steady display rate; the worker drives the simulation
            // independently, so the UI never blocks on compute.
            ctx.request_repaint_after(Duration::from_millis(33));

            // One brief read lock to grab everything the frame needs.
            let (
                snapshot,
                release_history,
                layer_radii_um,
                gpu_available,
                gpu_name,
                running,
                mut speed,
                compute,
                mut n_walkers,
            ) = {
                let s = self.shared.read().unwrap();
                (
                    s.snapshot.clone(),
                    s.release_history.clone(),
                    s.layer_radii_um,
                    s.gpu_available,
                    s.gpu_name.clone(),
                    s.running,
                    s.speed_s_per_frame,
                    s.compute,
                    s.n_walkers,
                )
            };

            egui::TopBottomPanel::top("controls").show(&ctx, |ui| {
                ui.heading("Walk-on-Spheres real-time TRISO diffusion (Cs-137, 1600 \u{b0}C)");
                ui.horizontal(|ui| {
                    if ui
                        .button(if running {
                            "\u{23f8} Pause"
                        } else {
                            "\u{25b6} Run"
                        })
                        .clicked()
                    {
                        self.shared.write().unwrap().running = !running;
                    }
                    if ui.button("\u{21ba} Reset").clicked() {
                        self.shared.write().unwrap().reset_requested = true;
                    }
                    ui.separator();
                    ui.label(format!("sim time: {:.3e} s", snapshot.sim_time_s));
                    ui.label(format!(
                        "released: {:.1}%",
                        100.0 * snapshot.released_fraction
                    ));
                    ui.label(format!("atoms: {}", snapshot.n_total));
                });
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Slider::new(&mut speed, 1.0e3..=1.0e6)
                                .logarithmic(true)
                                .text("sim seconds / frame"),
                        )
                        .changed()
                    {
                        self.shared.write().unwrap().speed_s_per_frame = speed;
                    }
                    if ui
                        .add(
                            egui::Slider::new(&mut n_walkers, 500..=20000)
                                .text("atoms (rebuilds on change)"),
                        )
                        .changed()
                    {
                        let mut s = self.shared.write().unwrap();
                        s.n_walkers = n_walkers;
                        s.rebuild_requested = true;
                    }
                });
                ui.horizontal(|ui| {
                    if ui
                        .button(format!("\u{1f5a5} Backend: {}", compute.label()))
                        .on_hover_text("Click to cycle CPU single \u{2192} CPU multi \u{2192} GPU")
                        .clicked()
                    {
                        self.shared.write().unwrap().compute = compute.next();
                    }
                    ui.label(gpu_status_line(compute, gpu_available, &gpu_name));
                });
            });

            egui::SidePanel::right("release_panel")
                .min_width(360.0)
                .show(&ctx, |ui| {
                    ui.heading("Release fraction over time");
                    Plot::new("release_fraction")
                        .legend(Legend::default())
                        .view_aspect(1.2)
                        .x_axis_label("simulated time (s)")
                        .y_axis_label("released fraction")
                        .show(ui, |plot_ui| {
                            plot_ui
                                .line(Line::new("M(t)/M_inf", PlotPoints::from(release_history)));
                        });
                    ui.separator();
                    ui.label(
                        "The compute runs on a background thread; the UI only reads snapshots, \
                         so it stays smooth no matter how heavy a frame is. Each atom is walked \
                         to the OPyC surface with no timestep; the SiC interface holds most back.",
                    );
                });

            egui::CentralPanel::default().show(&ctx, |ui| {
                Plot::new("positions")
                    .legend(Legend::default())
                    .data_aspect(1.0)
                    .view_aspect(1.0)
                    .x_axis_label("x (\u{b5}m)")
                    .y_axis_label("y (\u{b5}m)")
                    .show(ui, |plot_ui| {
                        for &r in &layer_radii_um {
                            plot_ui.line(Line::new("", PlotPoints::from(circle_um(r))));
                        }
                        plot_ui.points(
                            Points::new(
                                "atoms (x-y slice)",
                                PlotPoints::from(snapshot.positions_xy_um),
                            )
                            .radius(1.5_f32),
                        );
                    });
            });
        }
    }
}
