//! The Widget Studio application state and panels.
//!
//! Layout: a widget picker on the left, the widget under test on a canvas in
//! the centre, and its live controls plus a "what drives what" readout on the
//! right. The readout is the point of the studio — it names every physical
//! quantity feeding the rendering, so a widget that is secretly ignoring its
//! physics has nowhere to hide.

use egui::{Color32, Pos2, RichText, Vec2};
use outram_park_digital_twin_engine::components::{PipeVisual, TurbineFlowPath, TurbineVisual};
use tampines_steam_tables::steam_turbine_equations::generator::ThreePhaseElectricGeneratorTurbine;
use uom::si::angular_velocity::{radian_per_second, revolution_per_minute};
use uom::si::electric_potential::volt;
use uom::si::electrical_resistance::ohm;
use uom::si::f64::{
    AngularVelocity, ElectricalResistance, Power, ThermodynamicTemperature, Time, Torque,
};
use uom::si::power::megawatt;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::torque::newton_meter;

/// Which widget the studio is currently exercising.
///
/// Deliberately an enum rather than a registry of trait objects — the set of
/// widgets is closed and known at compile time, matching the workspace's
/// "no trait objects" rule. Adding a widget adds a variant, and the compiler
/// then points at every match that needs handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetUnderTest {
    SteamTurbine,
    Pipes,
}

impl WidgetUnderTest {
    /// Every widget the studio can show, in picker order.
    pub const ALL: &'static [Self] = &[Self::SteamTurbine, Self::Pipes];

    /// Human-readable name for the picker.
    pub fn label(self) -> &'static str {
        match self {
            Self::SteamTurbine => "Steam turbine",
            Self::Pipes => "Pipes (3 backends)",
        }
    }

    /// One-line statement of the widget's quality status, shown in the picker
    /// so the gallery doubles as an honest progress board.
    pub fn status(self) -> &'static str {
        match self {
            Self::SteamTurbine => "reworked — spins at omega from a real torque balance",
            Self::Pipes => "salt / steam-water HEM / helium, stacked",
        }
    }
}

/// The studio app.
pub struct WidgetStudio {
    /// Which widget is on the canvas.
    selected: WidgetUnderTest,

    // ── Turbine physics (a real model, advanced every frame) ──────────────────
    /// The generator/rotor model under test.
    generator: ThreePhaseElectricGeneratorTurbine,
    /// Simulation clock. Application-owned, because widgets are rebuilt every
    /// repaint and a widget-owned clock would reset each frame.
    simulation_time: Time,
    /// Drive torque applied to the rotor, in N*m — the user's main control.
    drive_torque_nm: f64,
    /// Electrical load resistance, in ohm. Lower resistance means a heavier
    /// electrical load, which brakes the rotor.
    load_resistance_ohm: f64,
    /// Simulation timestep, in seconds.
    timestep_s: f64,
    /// Whether the physics is advancing.
    running: bool,
    /// Simulated seconds per wall-clock second. 1.0 is real time.
    sim_speed: f64,
    /// Leftover simulated time not yet consumed by a whole timestep.
    time_debt_s: f64,
    /// Smoothed frame time in milliseconds, for the performance readout.
    frame_ms: f32,
    /// Substeps actually taken on the last frame (capped).
    last_substeps: u32,

    // ── Presentation ──────────────────────────────────────────────────────
    /// On-screen size of the widget under test, in points.
    widget_size: Vec2,
    /// Flow path drawn: double-flow (PWR standard) or single-flow.
    flow_path: TurbineFlowPath,

    // ── Pipes tab ─────────────────────────────────────────────────────────
    /// The three demonstration pipes, built once at startup (standing up a
    /// fluid array per frame would be wasteful and pointless — the widget is
    /// rebuilt each repaint, the physics is not).
    pipe_rows: Vec<crate::pipes::PipeRow>,
    /// Backends that could not be constructed, reported rather than faked.
    pipe_errors: Vec<String>,
}

impl Default for WidgetStudio {
    fn default() -> Self {
        let (pipe_rows, pipe_errors) = crate::pipes::build_rows();
        Self {
            selected: WidgetUnderTest::SteamTurbine,
            generator: ThreePhaseElectricGeneratorTurbine::new_250_megawatt_generator(),
            simulation_time: Time::new::<second>(0.0),
            // Enough torque to spin a 530,000 kg*m^2 rotor up over tens of
            // seconds, so spin-up is watchable rather than instantaneous.
            drive_torque_nm: 2.0e6,
            load_resistance_ohm: 10.0,
            timestep_s: 0.02,
            running: true,
            sim_speed: 1.0,
            time_debt_s: 0.0,
            frame_ms: 0.0,
            last_substeps: 0,
            widget_size: Vec2::new(520.0, 260.0),
            flow_path: TurbineFlowPath::default(),
            pipe_rows,
            pipe_errors,
        }
    }
}

impl WidgetStudio {
    /// Advance the physics by one timestep.
    ///
    /// Uses the model's own explicit torque balance
    /// (`ThreePhaseElectricGeneratorTurbine::advance_timestep`) — the studio
    /// adds no physics of its own, per this crate's "no new physics" rule.
    fn step_physics(&mut self) {
        let dt = Time::new::<second>(self.timestep_s);
        self.generator.advance_timestep(
            Torque::new::<newton_meter>(self.drive_torque_nm),
            ElectricalResistance::new::<ohm>(self.load_resistance_ohm),
            self.simulation_time,
            dt,
        );
        self.simulation_time += dt;
    }

    /// Reset the model to rest at t = 0.
    fn reset(&mut self) {
        self.generator = ThreePhaseElectricGeneratorTurbine::new_250_megawatt_generator();
        self.simulation_time = Time::new::<second>(0.0);
    }

    fn omega(&self) -> AngularVelocity {
        self.generator.get_omega()
    }

    fn power(&self) -> Power {
        self.generator.get_power(
            ElectricalResistance::new::<ohm>(self.load_resistance_ohm),
            self.simulation_time,
        )
    }
}

impl eframe::App for WidgetStudio {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Measure the real frame time and pace the physics off it, rather than
        // taking exactly one timestep per frame. One-step-per-frame ties
        // simulated time to frame rate, and any hitch then compounds: the app
        // falls behind, tries to catch up, and feels laggy.
        let dt_real = ui.input(|i| i.stable_dt).clamp(0.0, 0.25) as f64;
        self.frame_ms = 0.9 * self.frame_ms + 0.1 * (dt_real as f32 * 1000.0);

        if self.running {
            self.time_debt_s += dt_real * self.sim_speed;
            // Hard cap on catch-up work per frame. Without it a long stall
            // queues thousands of substeps and the UI locks solid.
            const MAX_SUBSTEPS: u32 = 8;
            let mut taken = 0;
            while self.time_debt_s >= self.timestep_s && taken < MAX_SUBSTEPS {
                self.step_physics();
                self.time_debt_s -= self.timestep_s;
                taken += 1;
            }
            // Whatever could not be simulated this frame is dropped rather than
            // carried, so the app degrades to slow-motion instead of to a
            // growing backlog.
            if taken == MAX_SUBSTEPS {
                self.time_debt_s = 0.0;
            }
            self.last_substeps = taken;

            // Pipe tracers run off wall-clock time directly (not substeps):
            // each mark must cross its run in exactly one residence time, and
            // that is a real-time claim, not a simulation-step one.
            crate::pipes::advance_tracers(
                &mut self.pipe_rows,
                Time::new::<second>(dt_real * self.sim_speed),
            );

            ui.ctx().request_repaint();
        } else {
            self.last_substeps = 0;
        }

        egui::Panel::left("picker").show_inside(ui, |ui| {
            ui.heading("Widgets");
            ui.label(
                RichText::new("Gallery for developing and QC-ing the visual component library.")
                    .small()
                    .weak(),
            );
            ui.separator();
            for w in WidgetUnderTest::ALL {
                ui.selectable_value(&mut self.selected, *w, w.label());
                ui.label(RichText::new(w.status()).small().weak());
                ui.add_space(6.0);
            }
            ui.separator();
            ui.label(
                RichText::new(
                    "Remaining widgets are tracked as op-wqk.14.1 … .14.10 and are added here as \
                     each is brought up to standard.",
                )
                .small()
                .weak(),
            );
        });

        egui::Panel::right("controls")
            .min_size(320.0)
            .show_inside(ui, |ui| match self.selected {
                WidgetUnderTest::SteamTurbine => self.turbine_controls(ui),
                WidgetUnderTest::Pipes => self.pipe_controls(ui),
            });

        egui::CentralPanel::default().show_inside(ui, |ui| match self.selected {
            WidgetUnderTest::SteamTurbine => self.turbine_canvas(ui),
            WidgetUnderTest::Pipes => {
                // Pipes are drawn to true scale, so a long run can exceed the
                // panel. Scroll rather than rescale: shrinking to fit would
                // break length being comparable between rows.
                egui::ScrollArea::horizontal()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        crate::pipes::draw(ui, &self.pipe_rows, &self.pipe_errors)
                    });
            }
        });
    }
}

impl WidgetStudio {
    /// Right-hand panel for the pipes tab: what each backend is and what it
    /// can represent.
    fn pipe_controls(&mut self, ui: &mut egui::Ui) {
        ui.heading("Pipes");
        ui.label(
            RichText::new(
                "One PipeVisual over three flow backends. Colour comes from the per-cell \
                 temperature profile the backend reports; the number of colour bands IS the \
                 finite-volume cell count.",
            )
            .small()
            .weak(),
        );
        ui.separator();

        ui.label(RichText::new("Flow").strong());
        ui.label(
            RichText::new(
                "Velocity sets the tracer speed. Each white mark crosses its pipe in exactly \
                 one residence time, tau = L/u; a negative velocity runs it backwards.",
            )
            .small()
            .weak(),
        );
        for i in 0..self.pipe_rows.len() {
            let name = self.pipe_rows[i].name;
            ui.add(
                egui::Slider::new(&mut self.pipe_rows[i].velocity_m_s, -30.0..=30.0)
                    .text(format!("{name} [m/s]")),
            );
        }

        ui.add_space(8.0);
        ui.label(RichText::new("What drives what").strong());
        egui::Grid::new("pipe_readout")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                for row in &self.pipe_rows {
                    let temps = PipeVisual::new(
                        row.pipe.clone(),
                        Pos2::ZERO,
                        Vec2::new(1.0, 0.0),
                        row.min_temp,
                        row.max_temp,
                    )
                    .cell_temperatures();
                    ui.label(row.name);
                    if temps.is_empty() {
                        ui.label(
                            RichText::new("no cells reported — drawn neutral grey")
                                .italics()
                                .color(Color32::from_rgb(200, 140, 60)),
                        );
                    } else {
                        let first = temps.first().map(|t| t.get::<kelvin>()).unwrap_or(0.0);
                        let last = temps.last().map(|t| t.get::<kelvin>()).unwrap_or(0.0);
                        let tau = crate::pipes::residence_time(row).get::<second>();
                        let tau_txt = if tau.is_finite() {
                            format!("tau {tau:.2} s")
                        } else {
                            "stagnant".to_string()
                        };
                        ui.label(format!(
                            "{} cells · {first:.1} → {last:.1} K · {tau_txt}",
                            temps.len()
                        ));
                    }
                    ui.end_row();
                }
            });

        ui.add_space(8.0);
        ui.label(
            RichText::new(
                "These pipes are static: their arrays are constructed but not stepped, because \
                 tampines::components::Pipe::step is not implemented yet. The colours are the \
                 arrays' real initial states, not a fabricated profile.",
            )
            .small()
            .weak(),
        );
    }

    /// Right-hand panel: the controls that drive the turbine, and the readout
    /// naming which quantity drives which part of the rendering.
    fn turbine_controls(&mut self, ui: &mut egui::Ui) {
        ui.heading("Steam turbine");
        ui.label(
            RichText::new(
                "Physics: ThreePhaseElectricGeneratorTurbine (tampines-steam-tables). \
                 Explicit torque balance; EMF, current and power read off omega.",
            )
            .small()
            .weak(),
        );
        ui.separator();

        ui.label(RichText::new("Drive").strong());
        ui.add(
            egui::Slider::new(&mut self.drive_torque_nm, 0.0..=1.0e7)
                .text("drive torque [N·m]")
                .custom_formatter(|v, _| format!("{:.3e}", v)),
        );
        ui.add(
            egui::Slider::new(&mut self.load_resistance_ohm, 0.5..=200.0)
                .logarithmic(true)
                .text("load resistance [Ω]"),
        );
        ui.label(
            RichText::new("Lower resistance = heavier electrical load = more braking torque.")
                .small()
                .weak(),
        );

        ui.add_space(8.0);
        ui.label(RichText::new("Integration").strong());
        ui.add(
            egui::Slider::new(&mut self.timestep_s, 0.001..=0.2)
                .logarithmic(true)
                .text("timestep [s]"),
        );
        ui.add(
            egui::Slider::new(&mut self.sim_speed, 0.1..=20.0)
                .logarithmic(true)
                .text("sim speed [x real time]"),
        );
        ui.horizontal(|ui| {
            let label = if self.running { "Pause" } else { "Run" };
            if ui.button(label).clicked() {
                self.running = !self.running;
            }
            if ui.button("Step").clicked() {
                self.step_physics();
            }
            if ui.button("Reset").clicked() {
                self.reset();
            }
        });

        ui.add_space(8.0);
        ui.label(RichText::new("Machine").strong());
        ui.horizontal(|ui| {
            ui.selectable_value(
                &mut self.flow_path,
                TurbineFlowPath::DoubleFlow,
                "double flow",
            );
            ui.selectable_value(
                &mut self.flow_path,
                TurbineFlowPath::SingleFlow,
                "single flow",
            );
        });
        ui.label(
            RichText::new(
                "Double flow is the default and the PWR standard for both HP and LP cylinders: \
                 saturated steam means large volumetric flow, and centre admission cancels most \
                 of the axial thrust.",
            )
            .small()
            .weak(),
        );

        ui.add_space(8.0);
        ui.label(RichText::new("Presentation").strong());
        ui.add(egui::Slider::new(&mut self.widget_size.x, 120.0..=900.0).text("width [pt]"));
        ui.add(egui::Slider::new(&mut self.widget_size.y, 60.0..=500.0).text("height [pt]"));

        ui.separator();
        ui.label(RichText::new("Performance").strong());
        let fps = if self.frame_ms > 0.0 { 1000.0 / self.frame_ms } else { 0.0 };
        ui.label(format!(
            "frame {:.1} ms  ({:.0} fps) · {} substep(s)/frame",
            self.frame_ms, fps, self.last_substeps
        ));
        ui.label(
            RichText::new(
                "Physics is paced off the real frame time, capped at 8 substeps, so a hitch \
                 degrades to slow motion rather than a growing backlog.",
            )
            .small()
            .weak(),
        );

        ui.separator();
        ui.label(RichText::new("What drives what").strong());
        egui::Grid::new("turbine_readout")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                let omega = self.omega();
                let rpm = omega.get::<revolution_per_minute>();

                ui.label("simulation time");
                ui.label(format!("{:.2} s", self.simulation_time.get::<second>()));
                ui.end_row();

                ui.label("omega → blade rotation");
                ui.label(format!(
                    "{:.3} rad/s  ({:.1} rpm)",
                    omega.get::<radian_per_second>(),
                    rpm
                ));
                ui.end_row();

                ui.label("rotor angle θ = ωt");
                let theta = TurbineVisual::new_generator(
                    self.generator.clone(),
                    Pos2::ZERO,
                    self.widget_size,
                    ThermodynamicTemperature::new::<kelvin>(300.0),
                    ThermodynamicTemperature::new::<kelvin>(900.0),
                )
                .at_time(self.simulation_time)
                .rotor_angle();
                ui.label(format!(
                    "{:.2} rad",
                    theta.get::<uom::si::angle::radian>()
                ));
                ui.end_row();

                ui.label("electrical power");
                ui.label(format!("{:.3} MW", self.power().get::<megawatt>()));
                ui.end_row();

                ui.label("phase-1 EMF");
                ui.label(format!(
                    "{:.1} V",
                    self.generator.get_emf_1(self.simulation_time).get::<volt>()
                ));
                ui.end_row();

                let admission = TurbineVisual::new_generator(
                    self.generator.clone(),
                    Pos2::ZERO,
                    self.widget_size,
                    ThermodynamicTemperature::new::<kelvin>(300.0),
                    ThermodynamicTemperature::new::<kelvin>(900.0),
                );
                let a0 = admission.stage_angles(0.0);
                let a1 = admission.stage_angles(1.0);

                ui.label("stage angles (admission)");
                ui.label(format!(
                    "stator {:.0}°, rotor {:.0}°, R={:.2}",
                    a0.stator_deg, a0.rotor_deg, a0.reaction
                ));
                ui.end_row();

                ui.label("stage angles (exhaust)");
                ui.label(format!(
                    "stator {:.0}°, rotor {:.0}°, R={:.2}",
                    a1.stator_deg, a1.rotor_deg, a1.reaction
                ));
                ui.end_row();

                ui.label("stage quality (placeholder)");
                ui.label(format!(
                    "x = {:.2} at admission → {:.2} at exhaust",
                    admission.stage_quality(0.0),
                    admission.stage_quality(1.0)
                ));
                ui.end_row();

                ui.label("casing colour");
                ui.label(
                    RichText::new("none — no steam path on this variant")
                        .italics()
                        .color(Color32::from_rgb(200, 140, 60)),
                );
                ui.end_row();
            });

        ui.add_space(6.0);
        ui.label(
            RichText::new(
                "The generator variant is electromechanical and has no steam state, so the casing \
                 renders neutral grey rather than a fabricated temperature colour. Colour returns \
                 when the turbine is coupled to a steam path (bead op-dt3.18).",
            )
            .small()
            .weak(),
        );
    }

    /// Centre panel: the widget under test, on a plain canvas.
    fn turbine_canvas(&mut self, ui: &mut egui::Ui) {
        ui.heading("Widget under test");
        ui.label(
            RichText::new(
                "Blades turn at θ = ω·t. Watch the white marker blade to read speed and direction.",
            )
            .small()
            .weak(),
        );
        ui.separator();

        let available = ui.available_rect_before_wrap();
        let centre = available.center();

        ui.add(
            TurbineVisual::new_generator(
                self.generator.clone(),
                centre,
                self.widget_size,
                ThermodynamicTemperature::new::<kelvin>(300.0),
                ThermodynamicTemperature::new::<kelvin>(900.0),
            )
            .at_time(self.simulation_time)
            .with_flow_path(self.flow_path),
        );
    }
}
