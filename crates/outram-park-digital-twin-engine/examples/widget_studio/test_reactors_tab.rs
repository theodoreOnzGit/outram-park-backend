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
use outram_park_digital_twin_engine::components::{Htr10ReactorSchematic, Htr10SteamGeneratorVisual};

/// HTR-10 feedwater temperature, degC. `docs/reactor-scoping/htr10-plant-data.md`
/// section 6, *Quoted* ([S2] Table 1 and [S5] section 1 agree).
const FEEDWATER_DEGC: f64 = 104.0;
/// HTR-10 steam outlet temperature, degC. Same sheet and sources, *Quoted*.
const STEAM_DEGC: f64 = 440.0;

/// Horizontal gap between the vessel and the steam generator, as a multiple
/// of the vessel width: the length of duct left visible between them. A
/// drawing choice; no source gives the duct length.
const DUCT_GAP_FRACTION: f32 = 0.6;
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
    ui.add(
        egui::Slider::new(&mut state.cold_plenum_residence_s, 0.2..=20.0)
            .text("cold plenum"),
    );
    ui.add(
        egui::Slider::new(&mut state.hot_duct_residence_s, 0.2..=20.0).text("duct, hot inner tube"),
    );
    ui.add(
        egui::Slider::new(&mut state.cold_duct_residence_s, 0.2..=20.0).text("duct, cold annulus"),
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
            "All six are DISPLAY CHOICES, not derived: the sheet gives no internal \
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

/// The vessel and the steam generator side by side, joined by the coaxial
/// duct: the full-size HTR-10 page only (the mini card on the Reactor vessels
/// gallery shows the vessel alone).
///
/// Both widgets report their connection points from the same layout code
/// that paints them (`Htr10ReactorSchematic::duct_port`,
/// `Htr10SteamGeneratorVisual::gas_port`). The steam generator is placed so
/// its gas port is level with the duct and `DUCT_GAP_FRACTION` vessel widths
/// clear of the vessel, and the duct is then extended to reach it. That runs
/// the hot inner tube to the foot of the steam generator's central riser,
/// where the hot gas enters.
///
/// Both vessels are drawn at the same height scale: each is 11 m tall on its
/// data sheet (`HTR10_SG_ASPECT_RATIO` is 2.6 m by 11 m). The steam generator
/// sits raised because its gas port is at its foot, while the duct leaves the
/// reactor at the hot plenum, about mid-height.
fn draw_plant(ui: &mut egui::Ui, state: &TestReactorsTab) {
    let reactor = state.visual();
    let reactor_size = reactor.size();
    let vessel_h = state.vessel_width / DRAWN_ASPECT_RATIO;
    let sg_size = egui::vec2(vessel_h * HTR10_SG_ASPECT_RATIO, vessel_h);

    // Helium enters hot (the reactor outlet) and leaves cold (the reactor
    // inlet): one loop, so the two widgets share temperatures and scale.
    let sg = Htr10SteamGeneratorVisual::new(
        sg_size,
        degc(state.min_temp_degc),
        degc(state.max_temp_degc),
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

    // Lay out in local coordinates, reactor at the origin.
    let reactor_local = egui::Rect::from_min_size(egui::Pos2::ZERO, reactor_size);
    let duct = reactor.duct_port(reactor_local);
    let sg_origin_local = egui::Rect::from_min_size(egui::Pos2::ZERO, sg_size);
    let gas = sg.gas_port(sg_origin_local);
    let sg_min = egui::pos2(
        reactor_local.right() + DUCT_GAP_FRACTION * state.vessel_width,
        duct.end.y - gas.y,
    );
    let sg_local = egui::Rect::from_min_size(sg_min, sg_size);
    let extension = (sg_min.x + gas.x) - duct.end.x;

    // Allocate the bounding box, then shift both into it.
    let top = sg_local.top().min(0.0);
    let bottom = sg_local.bottom().max(reactor_local.bottom());
    let (canvas, _response) = ui.allocate_exact_size(
        egui::vec2(sg_local.right(), bottom - top),
        egui::Sense::hover(),
    );
    let shift = canvas.min.to_vec2() - egui::vec2(0.0, top);

    // The steam generator first, so the duct is painted over its foot and
    // reads as entering it.
    ui.put(sg_local.translate(shift), state.sg_tracers.attach(sg));
    ui.put(
        reactor_local.translate(shift),
        reactor.with_duct_extension(extension),
    );
}
