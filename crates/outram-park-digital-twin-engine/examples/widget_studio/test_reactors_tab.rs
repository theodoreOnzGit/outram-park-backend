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

use egui::RichText;
use outram_park_digital_twin_engine::animation::{PebbleTransits, TracerTrain};
use outram_park_digital_twin_engine::components::htr10_reactor_schematic::{
    CORE_CAVITY_HEIGHT_CM, CRITICAL_BED_HEIGHT_CM, DRAWN_ASPECT_RATIO, EQUILIBRIUM_BED_HEIGHT_CM,
};
use outram_park_digital_twin_engine::components::htr10_steam_generator::HTR10_SG_ASPECT_RATIO;
use outram_park_digital_twin_engine::components::pipe_route::{route, PipeStream};
use outram_park_digital_twin_engine::components::{
    CondenserDisplayRange, CondenserKind, CondenserScalars, CondenserVisual, Htr10ReactorSchematic,
    Htr10SteamGeneratorVisual, PumpVisual, TurbineFlowPath, TurbineVisual,
};
use outram_park_digital_twin_engine::components::pump::PumpKind;

/// HTR-10 feedwater temperature, degC. `docs/reactor-scoping/htr10-plant-data.md`
/// section 6, *Quoted* ([S2] Table 1 and [S5] section 1 agree).
const FEEDWATER_DEGC: f64 = 104.0;
/// HTR-10 steam outlet temperature, degC. Same sheet and sources, *Quoted*.
const STEAM_DEGC: f64 = 440.0;

/// Horizontal gap between the vessel and the steam generator, as a multiple
/// of the vessel width: the length of duct left visible between them. A
/// drawing choice; no source gives the duct length.
const DUCT_GAP_FRACTION: f32 = 0.6;

/// Secondary-loop pipe thickness, as a fraction of the vessel width. A drawing
/// choice.
const STEAM_PIPE_FRACTION: f32 = 0.035;
use uom::si::angular_velocity::revolution_per_minute;
use uom::si::f64::{AngularVelocity, MassRate, ThermodynamicTemperature, Time};
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
    /// Pebble-bed height, cm, measured up from zero core height.
    ///
    /// Two values worth knowing: 123.06 cm at first criticality (the B1
    /// benchmark) and 197 cm at the equilibrium full-power core.
    pub bed_height_cm: f32,
    /// Drawn vessel width, in points. Height follows the native aspect.
    pub vessel_width: f32,
    /// Whether to draw the internal labels.
    pub show_labels: bool,

    /// Primary (helium) loop mass flow, kg/s. Drives every coolant tracer (the
    /// three passes and both duct streams). HTR-10 design value 4.32 kg/s
    /// (same sheet, section 6, *Quoted*).
    pub primary_mass_flow_kg_per_s: f64,
    /// Residence time down the downcomer annulus, s. Display choice.
    pub downcomer_residence_s: f64,
    /// Residence time up the side-reflector boreholes, s. Display choice.
    pub riser_residence_s: f64,
    /// Residence time across the hot plenum, s. Display choice.
    pub plenum_residence_s: f64,
    /// Residence time across the upper cold plenum, s. Display choice.
    pub cold_plenum_residence_s: f64,
    /// Residence time along the coaxial duct's hot inner tube, s. Display
    /// choice: the bores are cited but the duct length is not, so there is no
    /// volume to derive it from.
    pub hot_duct_residence_s: f64,
    /// Residence time back along the coaxial duct's cold annulus, s. Display
    /// choice, for the same reason.
    pub cold_duct_residence_s: f64,
    /// Pneumatic lift gas flow up the refuelling chute, kg/s. Only its SIGN
    /// matters to the animation (up, stopped, or back down). Display choice:
    /// no source gives the lift flow.
    pub refuel_lift_flow_kg_per_s: f64,
    /// Time for one pebble to travel the whole refuelling chute, s. Display
    /// choice: no source gives the lift speed.
    pub refuel_transit_s: f64,
    /// Pebble discharge rate driving the defuelling animation, kg/s. Only its
    /// SIGN matters (out, stopped, or back up). Display choice: no source gives
    /// a discharge rate to use.
    pub defuel_flow_kg_per_s: f64,
    /// Time for one pebble to travel the whole defuelling route, s. Display
    /// choice.
    pub defuel_transit_s: f64,
    /// Pebbles that have completed the refuelling chute into the core.
    pub pebbles_added: usize,
    /// Pebbles that have left through the defuelling opening.
    pub pebbles_removed: usize,

    downcomer: TracerTrain,
    riser: TracerTrain,
    plenum: TracerTrain,
    cold_plenum: TracerTrain,
    hot_duct: TracerTrain,
    cold_duct: TracerTrain,
    /// Pebbles on their way up the refuelling chute, one per `[add pebble]`.
    pub refuel_pebbles: PebbleTransits,
    /// Pebbles on their way out of the defuelling route, one per
    /// `[remove pebble]`.
    pub defuel_pebbles: PebbleTransits,
    /// The steam generator's tracers (riser, shell, coil, nozzles), reused
    /// from the Steam generators tab. Its primary flow is kept equal to this
    /// tab's, so the whole loop stalls or reverses together.
    pub sg_tracers: crate::steam_generator_tab::Htr10Tracers,
    /// Residence time through the duct inlet's elbows inside the steam
    /// generator, s. Display choice: the elbows are a drawing device with no
    /// stated volume.
    pub sg_elbow_residence_s: f64,
    sg_hot_elbow: TracerTrain,
    sg_cold_elbow: TracerTrain,
    /// Turbine shaft speed, rpm. Display choice: drives the rotor's turning,
    /// no model behind it (the studio tests GUI, not physics).
    pub turbine_rpm: f64,
    /// Feed pump shaft speed, rpm. Display choice.
    pub pump_rpm: f64,
    /// Condenser condensing temperature, degC. Display choice: the plant sheet
    /// records no turbine or condenser data.
    pub condensing_degc: f64,
    /// Turbine exhaust steam quality, `[0, 1]`. Display choice.
    pub exhaust_quality: f64,
    /// Condenser cooling water in and out, degC. Display choices.
    pub cooling_water_in_degc: f64,
    pub cooling_water_out_degc: f64,
    /// Residence time along each secondary-loop pipe run, s. Display choice.
    pub secondary_pipe_residence_s: f64,
    /// Page clock, s, for the turbine and pump rotors (phase = speed x time).
    pub simulation_time_s: f64,
    sec_main_steam: TracerTrain,
    sec_exhaust: TracerTrain,
    sec_condensate: TracerTrain,
    sec_feed: TracerTrain,
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
            bed_height_cm: EQUILIBRIUM_BED_HEIGHT_CM,
            vessel_width: 220.0,
            show_labels: true,
            primary_mass_flow_kg_per_s: 4.32,
            downcomer_residence_s: 4.0,
            riser_residence_s: 3.0,
            plenum_residence_s: 1.5,
            cold_plenum_residence_s: 2.0,
            hot_duct_residence_s: 1.5,
            cold_duct_residence_s: 2.5,
            refuel_lift_flow_kg_per_s: 0.01,
            refuel_transit_s: 6.0,
            defuel_flow_kg_per_s: 0.01,
            defuel_transit_s: 8.0,
            pebbles_added: 0,
            pebbles_removed: 0,
            downcomer: TracerTrain::new(5),
            riser: TracerTrain::new(5),
            plenum: TracerTrain::new(3),
            cold_plenum: TracerTrain::new(3),
            hot_duct: TracerTrain::new(4),
            cold_duct: TracerTrain::new(4),
            refuel_pebbles: PebbleTransits::new(),
            defuel_pebbles: PebbleTransits::new(),
            sg_tracers: crate::steam_generator_tab::Htr10Tracers::default(),
            sg_elbow_residence_s: 2.0,
            sg_hot_elbow: TracerTrain::new(4),
            sg_cold_elbow: TracerTrain::new(4),
            turbine_rpm: 3000.0,
            pump_rpm: 2900.0,
            condensing_degc: 40.0,
            exhaust_quality: 0.90,
            cooling_water_in_degc: 25.0,
            cooling_water_out_degc: 35.0,
            secondary_pipe_residence_s: 3.0,
            simulation_time_s: 0.0,
            sec_main_steam: TracerTrain::new(4),
            sec_exhaust: TracerTrain::new(3),
            sec_condensate: TracerTrain::new(4),
            sec_feed: TracerTrain::new(4),
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
        self.hot_duct
            .advance(dt, Time::new::<second>(self.hot_duct_residence_s), primary);
        self.cold_duct
            .advance(dt, Time::new::<second>(self.cold_duct_residence_s), primary);
        self.pebbles_added += self.refuel_pebbles.advance(
            dt,
            Time::new::<second>(self.refuel_transit_s),
            MassRate::new::<kilogram_per_second>(self.refuel_lift_flow_kg_per_s),
        );
        // One primary loop: the steam generator's helium side follows the
        // reactor's flow.
        self.sg_tracers.primary_mass_flow_kg_per_s = self.primary_mass_flow_kg_per_s;
        self.sg_tracers.step(dt);
        let elbow_tau = Time::new::<second>(self.sg_elbow_residence_s);
        let primary_flow = MassRate::new::<kilogram_per_second>(self.primary_mass_flow_kg_per_s);
        self.sg_hot_elbow.advance(dt, elbow_tau, primary_flow);
        self.sg_cold_elbow.advance(dt, elbow_tau, primary_flow);
        // The secondary loop's pipes follow the secondary flow (3.49 kg/s at
        // design, from the plant sheet via the steam generator's tracers).
        self.simulation_time_s += dt.get::<second>();
        let secondary =
            MassRate::new::<kilogram_per_second>(self.sg_tracers.secondary_mass_flow_kg_per_s);
        let pipe_tau = Time::new::<second>(self.secondary_pipe_residence_s);
        for train in [
            &mut self.sec_main_steam,
            &mut self.sec_exhaust,
            &mut self.sec_condensate,
            &mut self.sec_feed,
        ] {
            train.advance(dt, pipe_tau, secondary);
        }
        self.pebbles_removed += self.defuel_pebbles.advance(
            dt,
            Time::new::<second>(self.defuel_transit_s),
            MassRate::new::<kilogram_per_second>(self.defuel_flow_kg_per_s),
        );
    }

    /// Build this frame's widget with the trains copied in.
    pub fn visual(&self) -> Htr10ReactorSchematic {
        self.visual_at_width(self.vessel_width)
    }

    /// The same widget at a caller-chosen vessel width, in points.
    ///
    /// Used for the mini copy on the Reactor vessels gallery. It shares this
    /// state, so its temperatures, rods and moving tracers match the full-size
    /// view exactly.
    pub fn visual_at_width(&self, vessel_width: f32) -> Htr10ReactorSchematic {
        let v = Htr10ReactorSchematic::new(
            Htr10ReactorSchematic::native_size(vessel_width),
            degc(self.min_temp_degc),
            degc(self.max_temp_degc),
            degc(self.pebble_degc),
            degc(self.inlet_degc),
            degc(self.outlet_degc),
            degc(self.reflector_degc),
            degc(self.vessel_degc),
        )
        .with_control_rod_frac(self.rod_frac)
        .with_bed_height_cm(self.bed_height_cm)
        .with_downcomer_tracer(self.downcomer.clone())
        .with_riser_tracer(self.riser.clone())
        .with_plenum_tracer(self.plenum.clone())
        .with_cold_plenum_tracer(self.cold_plenum.clone())
        .with_hot_duct_tracer(self.hot_duct.clone())
        .with_cold_duct_tracer(self.cold_duct.clone())
        .with_refuel_pebbles(self.refuel_pebbles.clone())
        .with_defuel_pebbles(self.defuel_pebbles.clone());
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
    ui.label(RichText::new("Core loading").strong());
    ui.add(
        egui::Slider::new(&mut state.bed_height_cm, 0.0..=CORE_CAVITY_HEIGHT_CM)
            .text("bed height [cm]")
            .fixed_decimals(1),
    );
    ui.horizontal(|ui| {
        if ui.button("first criticality").clicked() {
            state.bed_height_cm = CRITICAL_BED_HEIGHT_CM;
        }
        if ui.button("equilibrium").clicked() {
            state.bed_height_cm = EQUILIBRIUM_BED_HEIGHT_CM;
        }
    });
    ui.label(
        RichText::new(
            "Measured UP from zero core height (the top of the conus), which is how \
             the benchmark defines a loading. 123.06 cm is where HTR-10 first went \
             critical in B1; 197 cm is the equilibrium full-power average. Under-load \
             it and the gas space at the top of the cavity opens up — that space is \
             what B1 is about.",
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
    ui.add(egui::Slider::new(&mut state.cold_plenum_residence_s, 0.2..=20.0).text("cold plenum"));
    ui.add(
        egui::Slider::new(&mut state.hot_duct_residence_s, 0.2..=20.0).text("duct, hot inner tube"),
    );
    ui.add(
        egui::Slider::new(&mut state.cold_duct_residence_s, 0.2..=20.0).text("duct, cold annulus"),
    );
    ui.add(egui::Slider::new(&mut state.sg_elbow_residence_s, 0.2..=20.0).text("SG inlet elbows"));
    ui.add(
        egui::Slider::new(&mut state.secondary_pipe_residence_s, 0.2..=20.0)
            .text("secondary loop pipes"),
    );

    ui.separator();
    ui.label(RichText::new("Secondary loop (display choices)").strong());
    ui.add(egui::Slider::new(&mut state.turbine_rpm, 0.0..=3600.0).text("turbine [rpm]"));
    ui.add(egui::Slider::new(&mut state.pump_rpm, 0.0..=3600.0).text("feed pump [rpm]"));
    ui.add(egui::Slider::new(&mut state.condensing_degc, 20.0..=100.0).text("condensing [degC]"));
    ui.add(egui::Slider::new(&mut state.exhaust_quality, 0.5..=1.0).text("exhaust quality"));
    ui.add(
        egui::Slider::new(&mut state.cooling_water_in_degc, 5.0..=50.0)
            .text("cooling water in [degC]"),
    );
    ui.add(
        egui::Slider::new(&mut state.cooling_water_out_degc, 5.0..=60.0)
            .text("cooling water out [degC]"),
    );
    ui.label(
        RichText::new(
            "Steam 440 degC, feedwater 104 degC and the 3.49 kg/s flow are quoted plant values. \
             The plant sheet records NO turbine or condenser data, so everything above is a \
             display choice for testing the widgets. At 3000 rpm the rotor turns 50 times a \
             second, faster than the screen refreshes, so it may appear to crawl or reverse.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Pebble handling").strong());
    ui.horizontal(|ui| {
        if ui.button("add pebble").clicked() {
            state.refuel_pebbles.launch();
        }
        if ui.button("remove pebble").clicked() {
            state.defuel_pebbles.launch();
        }
    });
    ui.label(format!(
        "added {} · removed {} · in transit: {} up, {} out",
        state.pebbles_added,
        state.pebbles_removed,
        state.refuel_pebbles.len(),
        state.defuel_pebbles.len()
    ));
    ui.add(
        egui::Slider::new(&mut state.refuel_lift_flow_kg_per_s, -0.02..=0.02)
            .text("lift gas flow [kg/s]"),
    );
    ui.add(egui::Slider::new(&mut state.refuel_transit_s, 1.0..=30.0).text("lift transit [s]"));
    ui.add(
        egui::Slider::new(&mut state.defuel_flow_kg_per_s, -0.02..=0.02)
            .text("discharge rate [kg/s]"),
    );
    ui.add(
        egui::Slider::new(&mut state.defuel_transit_s, 1.0..=30.0).text("discharge transit [s]"),
    );
    ui.label(
        RichText::new(
            "[add pebble] lifts one pebble pneumatically up the refuelling chute into the \
             core; the chute is otherwise empty. [remove pebble] sends one, highlighted, down \
             the defuelling route and out of the opening. Each moves with the SIGN of its \
             driving flow (zero parks it) and crosses in its transit time. All four are \
             DISPLAY CHOICES: no source gives the lift flow, discharge rate or speeds. The \
             bed drawing does not change with the counts; it is representative.",
        )
        .small()
        .weak(),
    );
    ui.label(
        RichText::new(
            "All seven are DISPLAY CHOICES, not derived: the sheet gives no internal \
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
                draw_plant(ui, state);
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

/// The HTR-10 plant on the full-size page (the mini card on the Reactor vessels
/// gallery shows the vessel alone): the vessel and the steam generator joined
/// by the coaxial duct, and the secondary loop, the steam generator's steam out
/// to a single-flow turbine, its exhaust down into a condenser, the condensate
/// to a centrifugal feed pump, and the feed back up into the steam generator.
///
/// Every pipe runs between ports the widgets report from their own drawing
/// code (`steam_port`, `feedwater_port`, `TurbineVisual::ports`,
/// `CondenserVisual::ports`, `PumpVisual::centrifugal_ports`), and is drawn
/// with the engine's `pipe_route::route`: straight legs, a proper elbow at
/// each corner, and tracers following the secondary flow. No physics model runs
/// here: the studio tests widgets, so the turbine and pump turn at
/// display-choice speeds and the condenser takes display-choice temperatures.
///
/// Both widgets report their connection points from the same layout code
/// that paints them (`Htr10ReactorSchematic::duct_port`,
/// `Htr10SteamGeneratorVisual::gas_port`). The steam generator is placed so
/// its gas port is level with the duct and `DUCT_GAP_FRACTION` vessel widths
/// clear of the vessel, and the duct is then extended to reach it. The duct
/// stops at the steam generator's left wall; inside, the steam generator
/// draws the hot inner tube bending up into its central riser and the two
/// cold bands bending up into the left and right coil bundles
/// (`Htr10SteamGeneratorVisual::with_duct_inlet`).
///
/// Both vessels are drawn at the same height scale: each is 11 m tall on its
/// data sheet (`HTR10_SG_ASPECT_RATIO` is 2.6 m by 11 m). The steam generator
/// sits raised because its gas port is at its foot, while the duct leaves the
/// reactor at the hot plenum, about mid-height.
fn draw_plant(ui: &mut egui::Ui, state: &TestReactorsTab) {
    let reactor = state.visual();
    let reactor_size = reactor.size();
    let vw = state.vessel_width;
    let vessel_h = vw / DRAWN_ASPECT_RATIO;
    let sg_size = egui::vec2(vessel_h * HTR10_SG_ASPECT_RATIO, vessel_h);
    let (min_t, max_t) = (degc(state.min_temp_degc), degc(state.max_temp_degc));

    // Helium enters hot (the reactor outlet) and leaves cold (the reactor
    // inlet): one loop, so the two widgets share temperatures and scale.
    let sg = Htr10SteamGeneratorVisual::new(
        sg_size,
        min_t,
        max_t,
        degc(state.outlet_degc),
        degc(state.inlet_degc),
        degc(FEEDWATER_DEGC),
        degc(STEAM_DEGC),
    );
    let sg = if state.show_labels {
        sg
    } else {
        sg.without_labels()
    };

    // ── Primary side: reactor, duct and steam generator ────────────────────
    // Lay out in local coordinates, reactor at the origin.
    let reactor_local = egui::Rect::from_min_size(egui::Pos2::ZERO, reactor_size);
    let duct = reactor.duct_port(reactor_local);
    // The steam generator draws the duct's three streams turning up inside it,
    // at the duct's own band heights, so they meet the duct exactly.
    let sg = sg
        .with_duct_inlet(duct.outer_height, duct.inner_height)
        .with_duct_inlet_tracers(state.sg_hot_elbow.clone(), state.sg_cold_elbow.clone());
    let gas = sg.gas_port(egui::Rect::from_min_size(egui::Pos2::ZERO, sg_size));
    let sg_min = egui::pos2(
        reactor_local.right() + DUCT_GAP_FRACTION * vw,
        duct.end.y - gas.y,
    );
    let sg_local = egui::Rect::from_min_size(sg_min, sg_size);
    let extension = (sg_min.x + gas.x) - duct.end.x;

    // ── Secondary side: turbine, condenser, feed pump ──────────────────────
    // Each is placed from its own reported ports, so the pipes land on the
    // nozzles the widgets actually draw. Sizes and gaps are drawing choices.
    let steam_port = sg.steam_port(sg_local);
    let feed_port = sg.feedwater_port(sg_local);

    let turbine_size = egui::vec2(0.8 * vw, 0.3 * vw);
    let turbine_centre = egui::pos2(
        steam_port.x + 0.3 * vw + 0.5 * turbine_size.x,
        steam_port.y + 0.15 * vw + 0.5 * turbine_size.y,
    );
    let turbine_ports =
        TurbineVisual::ports(turbine_centre, turbine_size, TurbineFlowPath::SingleFlow);

    let condenser_kind = CondenserKind::TwoPass;
    let condenser_size = egui::vec2(0.6 * vw, 0.5 * vw);
    let condenser_box = egui::Rect::from_min_size(
        egui::pos2(
            turbine_ports.exhaust_out.x - 0.5 * condenser_size.x,
            turbine_ports.exhaust_out.y + 0.12 * vw,
        ),
        condenser_size,
    );
    let condenser_ports = CondenserVisual::ports(condenser_kind, condenser_box);

    // The pump sits below the feedwater nozzle (its discharge rises to it)
    // and left of the condensate line (its suction faces right).
    let pump_size = egui::vec2(0.32 * vw, 0.32 * vw);
    let probe =
        PumpVisual::centrifugal_ports(egui::Rect::from_min_size(egui::Pos2::ZERO, pump_size));
    let pump_top = (feed_port.y + 0.10 * vw).max(condenser_ports.condensate_out.y + 0.10 * vw);
    let pump_box = egui::Rect::from_min_size(
        egui::pos2(
            condenser_ports.condensate_out.x - 0.2 * vw - probe.suction.x,
            pump_top,
        ),
        pump_size,
    );
    let pump_ports = PumpVisual::centrifugal_ports(pump_box);

    // Pipe centrelines, corner to corner.
    let main_steam_path = [
        steam_port,
        egui::pos2(turbine_ports.steam_in.x, steam_port.y),
        turbine_ports.steam_in,
    ];
    let exhaust_path = [turbine_ports.exhaust_out, condenser_ports.steam_in];
    let condensate_path = [
        condenser_ports.condensate_out,
        egui::pos2(condenser_ports.condensate_out.x, pump_ports.suction.y),
        pump_ports.suction,
    ];
    let feed_path = [
        pump_ports.discharge,
        egui::pos2(pump_ports.discharge.x, feed_port.y),
        feed_port,
    ];

    // ── Canvas: the bounding box of everything, then shift it into place ───
    let turbine_rect = egui::Rect::from_center_size(turbine_centre, turbine_size);
    let bounds = [
        reactor_local,
        sg_local,
        turbine_rect,
        condenser_box,
        pump_box,
    ]
    .into_iter()
    .reduce(|a, b| a.union(b))
    .expect("five rects");
    let top = bounds.top().min(0.0);
    let (canvas, _response) = ui.allocate_exact_size(
        egui::vec2(bounds.right(), bounds.bottom() - top),
        egui::Sense::hover(),
    );
    let shift = canvas.min.to_vec2() - egui::vec2(0.0, top);
    let at = |p: egui::Pos2| p + shift;
    // Everything is drawn in a child area, so routed pipes and placed widgets
    // cannot push the surrounding panel layout around.
    let mut canvas_ui = ui.new_child(egui::UiBuilder::new().max_rect(canvas));
    let ui = &mut canvas_ui;

    // Pipes first, so each widget's nozzle is painted over the pipe's end.
    let secondary =
        MassRate::new::<kilogram_per_second>(state.sg_tracers.secondary_mass_flow_kg_per_s);
    let pipe = |temperature, tracer| PipeStream {
        temperature,
        mass_flow: secondary,
        residence_time: Time::new::<second>(state.secondary_pipe_residence_s),
        thickness: STEAM_PIPE_FRACTION * vw,
        tracer,
        min_temp: min_t,
        max_temp: max_t,
    };
    let condensing = degc(state.condensing_degc);
    for (stream, path) in [
        (
            pipe(degc(STEAM_DEGC), state.sec_main_steam),
            &main_steam_path[..],
        ),
        (pipe(condensing, state.sec_exhaust), &exhaust_path[..]),
        (pipe(condensing, state.sec_condensate), &condensate_path[..]),
        (pipe(degc(FEEDWATER_DEGC), state.sec_feed), &feed_path[..]),
    ] {
        let path: Vec<egui::Pos2> = path.iter().map(|p| at(*p)).collect();
        route(ui, &stream, &path, false, false);
    }

    // The duct now stops at the steam generator's left wall (its gas port);
    // inside, the steam generator draws the bends. Painted first, so the
    // duct's end sits over the wall.
    ui.put(sg_local.translate(shift), state.sg_tracers.attach(sg));
    // The extended reactor is wider (its box includes the duct), but its
    // vessel is anchored at the left of the box, so the origin is unchanged.
    let reactor = reactor.with_duct_extension(extension);
    let reactor_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, reactor.size());
    ui.put(reactor_rect.translate(shift), reactor);

    let time = Time::new::<second>(state.simulation_time_s);
    ui.add(
        TurbineVisual::from_scalars(
            AngularVelocity::new::<revolution_per_minute>(state.turbine_rpm),
            Some(degc(STEAM_DEGC)),
            at(turbine_centre),
            turbine_size,
            min_t,
            max_t,
        )
        .with_flow_path(TurbineFlowPath::SingleFlow)
        .at_time(time),
    );
    // No labels on this condenser: on the plant page they crowd the loop
    // (maintainer direction, 2026-09-22).
    ui.add(
        CondenserVisual::from_scalars(
            condenser_kind,
            at(condenser_box.center()),
            condenser_size,
            CondenserDisplayRange {
                min_temp: min_t,
                max_temp: max_t,
            },
            CondenserScalars {
                exhaust_quality: state.exhaust_quality,
                condensing_temp: condensing,
                condensate_temp: condensing,
                cooling_water_inlet_temp: degc(state.cooling_water_in_degc),
                cooling_water_outlet_temp: degc(state.cooling_water_out_degc),
                // No model supplies a hotwell level here; `None` draws it hatched
                // rather than inventing one.
                hotwell_level_frac: None,
            },
        )
        .without_labels(),
    );
    ui.add(PumpVisual::from_scalars(
        PumpKind::Centrifugal,
        at(pump_box.center()),
        pump_size,
        AngularVelocity::new::<revolution_per_minute>(state.pump_rpm),
        time,
        Some(degc(FEEDWATER_DEGC)),
        min_t,
        max_t,
    ));
}
