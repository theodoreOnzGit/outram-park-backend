//! The Widget Studio application state and panels.
//!
//! Layout: a widget picker on the left, the widget under test on a canvas in
//! the centre, and its live controls plus a "what drives what" readout on the
//! right. The readout is the point of the studio — it names every physical
//! quantity feeding the rendering, so a widget that is secretly ignoring its
//! physics has nowhere to hide.

use egui::{Color32, Pos2, RichText, Vec2};
use outram_park_digital_twin_engine::components::TurbineVisual;
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
}

impl WidgetUnderTest {
    /// Every widget the studio can show, in picker order.
    pub const ALL: &'static [Self] = &[Self::SteamTurbine];

    /// Human-readable name for the picker.
    pub fn label(self) -> &'static str {
        match self {
            Self::SteamTurbine => "Steam turbine",
        }
    }

    /// One-line statement of the widget's quality status, shown in the picker
    /// so the gallery doubles as an honest progress board.
    pub fn status(self) -> &'static str {
        match self {
            Self::SteamTurbine => "reworked — spins at omega from a real torque balance",
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

    // ── Presentation ──────────────────────────────────────────────────────
    /// On-screen size of the widget under test, in points.
    widget_size: Vec2,
}

impl Default for WidgetStudio {
    fn default() -> Self {
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
            widget_size: Vec2::new(520.0, 260.0),
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
        if self.running {
            self.step_physics();
            // Physics is advancing, so keep repainting even without input.
            ui.ctx().request_repaint();
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
            });

        egui::CentralPanel::default().show_inside(ui, |ui| match self.selected {
            WidgetUnderTest::SteamTurbine => self.turbine_canvas(ui),
        });
    }
}

impl WidgetStudio {
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
        ui.label(RichText::new("Presentation").strong());
        ui.add(egui::Slider::new(&mut self.widget_size.x, 120.0..=900.0).text("width [pt]"));
        ui.add(egui::Slider::new(&mut self.widget_size.y, 60.0..=500.0).text("height [pt]"));

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
            .at_time(self.simulation_time),
        );
    }
}
