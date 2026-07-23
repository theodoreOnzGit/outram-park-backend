//! Real-time TRISO diffusion driven by the Walk-on-Spheres first-passage engine.
//!
//! An `egui`/`eframe` demonstrator that animates an ensemble of fission-product
//! atoms diffusing out of a TRISO particle. Each frame advances every atom by a
//! user-set slice of simulated time using the timestep-free Walk-on-Spheres
//! engine (in parallel with `rayon`), then draws their positions and the live
//! release-fraction curve. Because the engine is event-driven — not
//! timestep-limited by the fast buffer layer — the animation runs smoothly
//! without the host lagging.
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
    use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::oorandom_rng::OoRng64;
    use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::walk_on_spheres::{
        sample_uniform_in_ball, WalkParams, WoSWalker,
    };
    use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::TrisoCell;
    use boon_lay::Nuclide;
    use egui_plot::{Legend, Line, Plot, PlotPoints, Points};
    use rayon::prelude::*;
    use uom::si::f64::Time;
    use uom::si::length::micrometer;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::time::second;
    use uom::si::f64::ThermodynamicTemperature;

    /// The live diffusion demonstrator state.
    pub struct RealtimeApp {
        cell: TrisoCell,
        params: WalkParams,
        nuclide: Nuclide,
        walkers: Vec<WoSWalker>,
        released: Vec<bool>,
        n_walkers: usize,
        sim_time_s: f64,
        /// Simulated seconds advanced per rendered frame.
        speed_s_per_frame: f64,
        running: bool,
        seed: u64,
        /// (simulated time [s], cumulative release fraction).
        release_history: Vec<[f64; 2]>,
        layer_radii_um: [f64; 5],
    }

    impl RealtimeApp {
        pub fn new() -> Self {
            let mut cell = TrisoCell::new_crp6_geometry();
            // A high temperature keeps the diffusion coefficients large enough
            // that release is visible within the demo's time span.
            cell.set_uniform_temperature(ThermodynamicTemperature::new::<degree_celsius>(1600.0));
            let layer_radii_um = [
                cell.get_fuel_radius().get::<micrometer>(),
                cell.get_buffer_radius().get::<micrometer>(),
                cell.get_ipyc_radius().get::<micrometer>(),
                cell.get_sic_radius().get::<micrometer>(),
                cell.get_opyc_radius().get::<micrometer>(),
            ];
            let mut app = Self {
                cell,
                params: WalkParams::crp6_default(),
                nuclide: Nuclide::Cs137,
                walkers: Vec::new(),
                released: Vec::new(),
                n_walkers: 3000,
                sim_time_s: 0.0,
                speed_s_per_frame: 5.0e4,
                running: false,
                seed: 0x0B00_1A47,
                release_history: Vec::new(),
                layer_radii_um,
            };
            app.reset();
            app
        }

        /// Rebuild the ensemble at the kernel with a fresh clock.
        fn reset(&mut self) {
            let fuel_radius = self.cell.get_fuel_radius();
            let mut master = OoRng64::from_u64(self.seed);
            self.walkers = (0..self.n_walkers)
                .map(|_| {
                    let start = sample_uniform_in_ball(&mut master.0, fuel_radius);
                    let child = OoRng64::from_u64(master.next_u64());
                    WoSWalker::new(start, self.nuclide, child)
                })
                .collect();
            self.released = vec![false; self.n_walkers];
            self.sim_time_s = 0.0;
            self.release_history = vec![[0.0, 0.0]];
        }

        /// Advance every still-contained atom to the next frame's target time.
        fn advance(&mut self) {
            let target = self.sim_time_s + self.speed_s_per_frame;
            let target_time = Time::new::<second>(target);

            let cell = &self.cell;
            let params = &self.params;
            let walkers = &mut self.walkers;
            let released = &mut self.released;
            walkers
                .par_iter_mut()
                .zip(released.par_iter_mut())
                .for_each(|(walker, is_released)| {
                    if !*is_released && !walker.diffuse_until(cell, params, target_time) {
                        *is_released = true;
                    }
                });

            self.sim_time_s = target;
            let released_count = self.released.iter().filter(|&&r| r).count();
            let fraction = released_count as f64 / self.n_walkers.max(1) as f64;
            self.release_history.push([self.sim_time_s, fraction]);
        }

        fn current_release_fraction(&self) -> f64 {
            self.release_history.last().map(|p| p[1]).unwrap_or(0.0)
        }

        fn circle_um(radius_um: f64) -> Vec<[f64; 2]> {
            (0..=72)
                .map(|k| {
                    let a = std::f64::consts::TAU * k as f64 / 72.0;
                    [radius_um * a.cos(), radius_um * a.sin()]
                })
                .collect()
        }
    }

    impl eframe::App for RealtimeApp {
        fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
            let ctx = ui.ctx().clone();

            if self.running {
                self.advance();
                ctx.request_repaint();
            }

            egui::TopBottomPanel::top("controls").show(&ctx, |ui| {
                ui.heading("Walk-on-Spheres real-time TRISO diffusion (Cs-137, 1600 \u{b0}C)");
                ui.horizontal(|ui| {
                    if ui
                        .button(if self.running { "\u{23f8} Pause" } else { "\u{25b6} Run" })
                        .clicked()
                    {
                        self.running = !self.running;
                    }
                    if ui.button("\u{21ba} Reset").clicked() {
                        self.reset();
                    }
                    ui.separator();
                    ui.label(format!("sim time: {:.3e} s", self.sim_time_s));
                    ui.label(format!(
                        "released: {:.1}%",
                        100.0 * self.current_release_fraction()
                    ));
                    ui.label(format!("atoms: {}", self.n_walkers));
                });
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut self.speed_s_per_frame, 1.0e3..=1.0e6)
                            .logarithmic(true)
                            .text("sim seconds / frame"),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.n_walkers, 500..=20000)
                            .text("atoms (applied on Reset)"),
                    );
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
                            plot_ui.line(Line::new(
                                "M(t)/M_inf",
                                PlotPoints::from(self.release_history.clone()),
                            ));
                        });
                    ui.separator();
                    ui.label(
                        "Each atom is walked to the OPyC surface with no timestep; \
                         the SiC layer's interface rule holds most of them back.",
                    );
                });

            egui::CentralPanel::default().show(&ctx, |ui| {
                let contained: Vec<[f64; 2]> = self
                    .walkers
                    .iter()
                    .zip(&self.released)
                    .filter(|(_, &r)| !r)
                    .map(|(w, _)| {
                        [w.position[0].get::<micrometer>(), w.position[1].get::<micrometer>()]
                    })
                    .collect();

                Plot::new("positions")
                    .legend(Legend::default())
                    .data_aspect(1.0)
                    .view_aspect(1.0)
                    .x_axis_label("x (\u{b5}m)")
                    .y_axis_label("y (\u{b5}m)")
                    .show(ui, |plot_ui| {
                        for &r in &self.layer_radii_um {
                            plot_ui
                                .line(Line::new("", PlotPoints::from(Self::circle_um(r))));
                        }
                        plot_ui.points(
                            Points::new("atoms (x-y slice)", PlotPoints::from(contained)).radius(1.5),
                        );
                    });
            });
        }
    }
}
