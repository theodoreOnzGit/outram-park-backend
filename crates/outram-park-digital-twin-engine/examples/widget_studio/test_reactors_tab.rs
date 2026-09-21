//! Test-reactors tab: plant-specific reactor vessels under development.
//!
//! Separate from the `Reactors` gallery on purpose. That tab shows the six
//! generic architectures from `docs/reactor-scoping/` side by side, all driven
//! by one shared operating point; this one is a **bench for vessels drawn to a
//! specific plant**, where each gets its own controls and its own flow
//! animation and is free to change shape between sessions.
//!
//! Currently one occupant: the simplified HTR-10 vessel
//! ([`outram_park_digital_twin_engine::components::Htr10ReactorSchematic`]),
//! the counterpart to the HTR-10 steam generator on the Steam generators tab.
//!
//! **Illustrative schematics, not validated models and not design drawings.**
//! Proportions that come from published dimensions are cited at their
//! constants in the widget; everything else is drawing.

use egui::{RichText, Vec2};
use outram_park_digital_twin_engine::animation::TracerTrain;
use outram_park_digital_twin_engine::components::Htr10ReactorSchematic;
use uom::si::f64::{MassRate, ThermodynamicTemperature, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;

/// Studio state for the HTR-10 vessel bench.
pub struct TestReactorsTab {
    /// Fuel (pebble) temperature, degrees Celsius.
    pub pebble_degc: f64,
    /// Cold helium entering the vessel, degrees Celsius. 250 at design.
    pub inlet_degc: f64,
    /// Hot helium in the bottom plenum, degrees Celsius. 700 at design.
    pub outlet_degc: f64,
    /// Graphite reflector bulk temperature, degrees Celsius.
    pub reflector_degc: f64,
    /// Pressure-vessel wall temperature, degrees Celsius.
    ///
    /// Sits near the **inlet** temperature in normal operation, not near the
    /// core's: the downcomer annulus washes the wall in 250 degC helium
    /// precisely to keep it there
    /// (`docs/reactor-scoping/htr10-plant-data.md` section 4.2).
    pub vessel_degc: f64,
    /// Cold end of the colour scale, degrees Celsius.
    pub min_temp_degc: f64,
    /// Hot end of the colour scale, degrees Celsius.
    pub max_temp_degc: f64,
    /// Control-rod insertion, dimensionless `[0, 1]`.
    pub rod_frac: f32,
    /// Drawn vessel width, in points. Height follows the native aspect.
    pub vessel_width: f32,
    /// Whether to draw the internal labels.
    pub show_labels: bool,

    /// Primary (helium) loop mass flow, kg/s. Drives all three coolant
    /// passes. HTR-10 design value 4.32 kg/s (same sheet, section 6,
    /// *Quoted*).
    pub primary_mass_flow_kg_per_s: f64,
    /// Residence time down the downcomer annulus, s. Display choice.
    pub downcomer_residence_s: f64,
    /// Residence time up the side-reflector boreholes, s. Display choice.
    pub riser_residence_s: f64,
    /// Residence time across the hot plenum, s. Display choice.
    pub plenum_residence_s: f64,
    /// Residence time across the upper cold plenum, s. Display choice.
    pub cold_plenum_residence_s: f64,

    downcomer: TracerTrain,
    riser: TracerTrain,
    plenum: TracerTrain,
    cold_plenum: TracerTrain,
}

impl Default for TestReactorsTab {
    /// HTR-10 normal full-power operation, from
    /// `docs/reactor-scoping/htr10-plant-data.md` section 6: helium 250 degC
    /// in / 700 degC out, 4.32 kg/s.
    fn default() -> Self {
        Self {
            pebble_degc: 850.0,
            inlet_degc: 250.0,
            outlet_degc: 700.0,
            reflector_degc: 620.0,
            vessel_degc: 265.0,
            min_temp_degc: 30.0,
            max_temp_degc: 930.0,
            rod_frac: 0.35,
            vessel_width: 220.0,
            show_labels: true,
            primary_mass_flow_kg_per_s: 4.32,
            downcomer_residence_s: 4.0,
            riser_residence_s: 3.0,
            plenum_residence_s: 1.5,
            cold_plenum_residence_s: 2.0,
            downcomer: TracerTrain::new(5),
            riser: TracerTrain::new(5),
            plenum: TracerTrain::new(3),
            cold_plenum: TracerTrain::new(3),
        }
    }
}

impl TestReactorsTab {
    /// Advance every train by `dt`.
    ///
    /// The three coolant passes take the **primary** mass flow, so they stall
    /// and reverse together.
    ///
    /// The fuel discharge tube carries NO tracer: pebbles are not coolant,
    /// they cross the core over weeks on a 5-pass route, and no source gives
    /// a throughput to derive a rate from. Animating it would have meant
    /// inventing one.
    pub fn step(&mut self, dt: Time) {
        let primary = MassRate::new::<kilogram_per_second>(self.primary_mass_flow_kg_per_s);
        self.downcomer
            .advance(dt, Time::new::<second>(self.downcomer_residence_s), primary);
        self.riser
            .advance(dt, Time::new::<second>(self.riser_residence_s), primary);
        self.plenum
            .advance(dt, Time::new::<second>(self.plenum_residence_s), primary);
        self.cold_plenum.advance(
            dt,
            Time::new::<second>(self.cold_plenum_residence_s),
            primary,
        );
    }

    /// Build this frame's widget with the trains copied in.
    pub fn visual(&self) -> Htr10ReactorSchematic {
        let v = Htr10ReactorSchematic::new(
            Vec2::new(self.vessel_width, self.vessel_width / (4.0 / 11.0)),
            degc(self.min_temp_degc),
            degc(self.max_temp_degc),
            degc(self.pebble_degc),
            degc(self.inlet_degc),
            degc(self.outlet_degc),
            degc(self.reflector_degc),
            degc(self.vessel_degc),
        )
        .with_control_rod_frac(self.rod_frac)
        .with_downcomer_tracer(self.downcomer.clone())
        .with_riser_tracer(self.riser.clone())
        .with_plenum_tracer(self.plenum.clone())
        .with_cold_plenum_tracer(self.cold_plenum.clone())
;
        if self.show_labels {
            v
        } else {
            v.without_labels()
        }
    }
}

fn degc(value: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(value)
}

/// Right-panel controls.
pub fn controls(ui: &mut egui::Ui, state: &mut TestReactorsTab) {
    ui.heading("Test reactors — HTR-10");
    ui.label(
        RichText::new(
            "A bench for plant-specific vessels, separate from the six-architecture \
             gallery. Illustrative schematic — not a validated model, and not a \
             design drawing.",
        )
        .small()
        .weak(),
    );
    ui.separator();

    ui.label(RichText::new("Temperatures [°C]").strong());
    ui.add(egui::Slider::new(&mut state.pebble_degc, 100.0..=1600.0).text("fuel / pebbles"));
    ui.add(egui::Slider::new(&mut state.inlet_degc, 50.0..=600.0).text("helium inlet"));
    ui.add(egui::Slider::new(&mut state.outlet_degc, 100.0..=1000.0).text("helium outlet"));
    ui.add(egui::Slider::new(&mut state.reflector_degc, 100.0..=1200.0).text("graphite reflector"));
    ui.add(egui::Slider::new(&mut state.vessel_degc, 50.0..=600.0).text("vessel wall"));
    ui.label(
        RichText::new(
            "The vessel wall sits near the INLET temperature, not the core's: the \
             downcomer annulus washes it in 250 °C helium to keep it there. Drag it \
             up and the drawing shows a vessel that is no longer being protected.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Control rods").strong());
    ui.add(
        egui::Slider::new(&mut state.rod_frac, 0.0..=1.0)
            .text("insertion fraction")
            .fixed_decimals(2),
    );
    ui.label(
        RichText::new(
            "All ten rods sit in the SIDE REFLECTOR — HTR-10 has no in-core rods, \
             which is why none is drawn entering the bed.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Flows").strong());
    ui.label(
        RichText::new(
            "The three coolant passes share the primary flow, so they stall and \
             reverse together. Set it negative and watch all three run backwards.",
        )
        .small()
        .weak(),
    );
    ui.add(
        egui::Slider::new(&mut state.primary_mass_flow_kg_per_s, -10.0..=10.0)
            .text("primary helium [kg/s]"),
    );
    ui.label(
        RichText::new(
            "The fuel discharge tube is deliberately NOT animated: pebbles are not \
             coolant, they cross the core over weeks on a 5-pass route, and no \
             source gives a throughput to derive a rate from.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Residence times [s]").strong());
    ui.add(egui::Slider::new(&mut state.downcomer_residence_s, 0.3..=30.0).text("downcomer"));
    ui.add(egui::Slider::new(&mut state.riser_residence_s, 0.3..=30.0).text("reflector risers"));
    ui.add(egui::Slider::new(&mut state.plenum_residence_s, 0.2..=20.0).text("hot plenum"));
    ui.add(
        egui::Slider::new(&mut state.cold_plenum_residence_s, 0.2..=20.0)
            .text("cold plenum"),
    );
    ui.label(
        RichText::new(
            "All four are DISPLAY CHOICES, not derived: the sheet gives no internal \
             volumes for these passes, so there is nothing to divide a flow into. \
             Sliders rather than hardcoded constants, so that is visible.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Colour scale [°C]").strong());
    ui.add(egui::Slider::new(&mut state.min_temp_degc, 0.0..=400.0).text("min"));
    ui.add(egui::Slider::new(&mut state.max_temp_degc, 200.0..=1600.0).text("max"));
    if state.max_temp_degc <= state.min_temp_degc {
        state.max_temp_degc = state.min_temp_degc + 1.0;
    }

    ui.separator();
    ui.label(RichText::new("Layout").strong());
    ui.add(egui::Slider::new(&mut state.vessel_width, 120.0..=420.0).text("vessel width [pt]"));
    ui.checkbox(&mut state.show_labels, "show internal labels");
}

/// Draw the vessel and the note beside it.
pub fn draw(ui: &mut egui::Ui, state: &TestReactorsTab) {
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.add(state.visual());
                ui.add_space(18.0);
                ui.vertical(|ui| {
                    ui.set_max_width(320.0);
                    ui.label(RichText::new("HTR-10 — three-pass helium path").strong());
                    ui.label(
                        RichText::new(
                            "1. DOWN the annulus between the pressure vessel and the core \
                             barrel, cooling the vessel wall.\n\
                             2. UP the 20 coolant boreholes in the side reflector.\n\
                             3. DOWN through the pebble bed into the hot plenum, then out \
                             sideways through the hot gas duct.",
                        )
                        .small(),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            "Nothing is drawn over the bed itself. The geometry says the \
                             flow goes down, and marks over the pebbles would obscure the \
                             one region worth seeing.",
                        )
                        .small()
                        .weak(),
                    );
                    ui.add_space(8.0);
                    ui.label(RichText::new("Proportions that are cited").strong());
                    ui.label(
                        RichText::new(
                            "Vessel 4 m × 11 m (both stated as bounds), core 1.8 m × 1.97 m, \
                             discharge tube 500 mm × 3.3 m, 20 coolant boreholes. Note the \
                             bed is under a fifth of the vessel height, and the discharge \
                             tube is longer than the bed is tall.",
                        )
                        .small()
                        .weak(),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new(
                            "Offline demonstration art. Not for nuclear facility operation, \
                             reactor control, safety-critical decision-making, or licensing.",
                        )
                        .small()
                        .weak(),
                    );
                });
            });
        });
}
