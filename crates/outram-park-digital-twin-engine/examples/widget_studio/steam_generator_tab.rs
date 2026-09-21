//! Steam-generator gallery tab: all three architectures, side by side.
//!
//! Shows every variant of
//! [`outram_park_digital_twin_engine::components::steam_generator::SteamGeneratorKind`]
//! at once, driven by one shared set of temperatures and one shared water
//! level, so the architectures can be compared directly and the colour
//! response checked across all three in a single glance.
//!
//! The point of the gallery is the *difference* between the three: a vertical
//! U-tube generator recirculates and therefore needs separators, a downcomer
//! and a level; a horizontal VVER generator recirculates lying down, with the
//! free surface spanning the vessel; a helical-coil generator is once-through
//! and has no level to show at all — moving the level slider deliberately does
//! nothing to it.
//!
//! **These are illustrative schematics, not validated models and not design
//! drawings.** Only the overall envelope proportions come from published
//! dimensions; the internals are proportioned for legibility and represent no
//! specific licensed design.

use egui::{RichText, Vec2};
use outram_park_digital_twin_engine::components::Htr10SteamGeneratorVisual;
use outram_park_digital_twin_engine::components::steam_generator::{
    SteamGeneratorKind, SteamGeneratorScalars, SteamGeneratorVisual,
};
use outram_park_digital_twin_engine::animation::TracerTrain;
use uom::si::angle::degree;
use uom::si::f64::{Angle, MassRate, ThermodynamicTemperature, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::time::second;
use uom::si::thermodynamic_temperature::degree_celsius;

/// Studio state for the steam-generator gallery.
pub struct SteamGeneratorTab {
    /// Primary coolant entering the generator (hot leg), degrees Celsius.
    pub primary_inlet_degc: f64,
    /// Primary coolant leaving the generator (cold leg), degrees Celsius.
    pub primary_outlet_degc: f64,
    /// Secondary feedwater entering the generator, degrees Celsius.
    pub feedwater_degc: f64,
    /// Secondary steam leaving the generator, degrees Celsius.
    pub steam_degc: f64,
    /// Cold end of the colour scale, degrees Celsius.
    pub min_temp_degc: f64,
    /// Hot end of the colour scale, degrees Celsius.
    pub max_temp_degc: f64,
    /// Secondary water level, dimensionless `[0, 1]`. Ignored by the
    /// once-through helical-coil unit, which has no free surface.
    pub water_level_frac: f32,
    /// Width of one gallery card, in points. Height follows at a fixed ratio.
    pub cell_width: f32,
    /// Whether to draw the internal component labels.
    pub show_labels: bool,
    /// HTR-10 card: riser share of the vessel diameter, dimensionless.
    ///
    /// Defaults to the maintainer-specified 0.40. A drawing parameter, not a
    /// plant dimension.
    pub riser_fraction: f32,
    /// HTR-10 card: coil stroke angle from the horizontal, degrees.
    ///
    /// Defaults to the maintainer-specified 7 degrees (revised down from 20).
    pub coil_angle_degrees: f64,
}

impl Default for SteamGeneratorTab {
    /// Defaults sit at a plausible large-PWR operating point — about 320 degC
    /// hot leg, 288 degC cold leg, 226 degC feedwater and saturated steam near
    /// 285 degC (roughly 6.9 MPa) — with the colour scale spanning it so no
    /// region is pinned at either end. These are round illustrative numbers,
    /// not plant data.
    fn default() -> Self {
        Self {
            primary_inlet_degc: 320.0,
            primary_outlet_degc: 288.0,
            feedwater_degc: 226.0,
            steam_degc: 285.0,
            min_temp_degc: 180.0,
            max_temp_degc: 360.0,
            water_level_frac: 0.62,
            cell_width: 210.0,
            show_labels: true,
            riser_fraction: 0.40,
            coil_angle_degrees: 7.0,
        }
    }
}

impl SteamGeneratorTab {
    /// The scalars every card in the gallery is drawn from.
    fn scalars(&self) -> SteamGeneratorScalars {
        SteamGeneratorScalars {
            primary_inlet_temp: degc(self.primary_inlet_degc),
            primary_outlet_temp: degc(self.primary_outlet_degc),
            feedwater_temp: degc(self.feedwater_degc),
            steam_temp: degc(self.steam_degc),
            water_level_frac: self.water_level_frac,
        }
    }
}

/// Height of a gallery card as a multiple of its width.
///
/// The two vertical architectures are very slender (roughly 1 : 4.6), so a
/// near-square card would letterbox them down to a sliver. The card is
/// therefore deliberately tall, and the wide VVER unit is given a wider column
/// instead of a taller one — see [`WIDE_COLUMN_MULTIPLE`].
const CARD_ASPECT: f32 = 2.4;

/// How much wider the horizontal (VVER) column is than the others.
///
/// Its artwork is 3.46 : 1, so in a column of the standard width it would
/// letterbox to a thin strip. Widening the column instead keeps all three
/// cards legible without stretching any of them.
const WIDE_COLUMN_MULTIPLE: f32 = 1.9;

fn degc(value: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(value)
}

/// Right-panel controls for the gallery.
pub fn controls(
    ui: &mut egui::Ui,
    state: &mut SteamGeneratorTab,
    tracers: &mut Htr10Tracers,
) {
    ui.heading("Steam generators");
    ui.label(
        RichText::new(
            "Three architectures: vertical U-tube (Western PWR), horizontal \
             U-tube (VVER), helical coil (HTGR / integral PWR). Illustrative \
             geometry — not a validated model, and not any specific licensed \
             design.",
        )
        .small()
        .weak(),
    );
    ui.separator();

    ui.label(RichText::new("Primary side [°C]").strong());
    ui.add(egui::Slider::new(&mut state.primary_inlet_degc, 100.0..=800.0).text("inlet (hot leg)"));
    ui.add(
        egui::Slider::new(&mut state.primary_outlet_degc, 100.0..=800.0).text("outlet (cold leg)"),
    );
    ui.label(
        RichText::new(
            "The tube bundles grade from the inlet temperature at one leg to \
             the outlet at the other. That gradient is a display \
             interpolation, not a computed profile.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Secondary side [°C]").strong());
    ui.add(egui::Slider::new(&mut state.feedwater_degc, 50.0..=400.0).text("feedwater"));
    ui.add(egui::Slider::new(&mut state.steam_degc, 100.0..=600.0).text("steam"));

    ui.separator();
    ui.label(RichText::new("Water level").strong());
    ui.add(
        egui::Slider::new(&mut state.water_level_frac, 0.0..=1.0)
            .text("level fraction")
            .fixed_decimals(2),
    );
    ui.label(
        RichText::new(
            "0 = at the tubesheet / vessel floor, 1 = at the separator inlet / \
             vessel top. The helical-coil unit ignores this: it is \
             once-through, so it has no free surface and no level.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("HTR-10 general structure").strong());
    ui.label(
        RichText::new(
            "The fourth card is a different widget: one plant's internal \
             arrangement, not a generic architecture. Both numbers below are \
             DRAWING parameters specified by the maintainer, not plant \
             dimensions — the real coil pitch is recorded as Unknown.",
        )
        .small()
        .weak(),
    );
    ui.add(
        egui::Slider::new(&mut state.riser_fraction, 0.1..=0.8)
            .text("riser share of diameter")
            .fixed_decimals(2),
    );
    ui.add(
        egui::Slider::new(&mut state.coil_angle_degrees, 0.0..=60.0)
            .text("coil angle [deg]"),
    );

    ui.add_space(6.0);
    ui.label(RichText::new("HTR-10 tracers").strong());
    ui.label(
        RichText::new(
            "Three streams on two independent loops. Direction and speed are \
             read off these numbers — set a flow NEGATIVE to reverse that \
             stream, or to zero to stall it. Stall the feedwater and the coil \
             marks freeze while the gas keeps moving.",
        )
        .small()
        .weak(),
    );
    ui.add(
        egui::Slider::new(&mut tracers.primary_mass_flow_kg_per_s, -10.0..=10.0)
            .text("primary (helium) [kg/s]"),
    );
    ui.add(
        egui::Slider::new(&mut tracers.secondary_mass_flow_kg_per_s, -10.0..=10.0)
            .text("secondary (feedwater) [kg/s]"),
    );
    ui.label(
        RichText::new(
            "Design values 4.32 and 3.49 kg/s — both Quoted in \
             docs/reactor-scoping/htr10-plant-data.md section 6.",
        )
        .small()
        .weak(),
    );
    ui.add(
        egui::Slider::new(&mut tracers.riser_residence_s, 0.5..=40.0)
            .text("riser residence [s]"),
    );
    ui.add(
        egui::Slider::new(&mut tracers.shell_residence_s, 0.5..=60.0)
            .text("shell-side residence [s]"),
    );
    ui.add(
        egui::Slider::new(&mut tracers.coil_residence_s, 0.5..=60.0)
            .text("coil residence [s]"),
    );
    ui.add(
        egui::Slider::new(&mut tracers.nozzle_residence_s, 0.2..=20.0)
            .text("nozzle residence [s]"),
    );
    ui.label(
        RichText::new(
            "The three residence times are DISPLAY CHOICES, not derived: the \
             riser is a schematic device with no stated dimensions, so there \
             is no volume to divide a flow into. Shown as sliders rather than \
             hardcoded so that is visible.",
        )
        .small()
        .weak(),
    );

    ui.separator();
    ui.label(RichText::new("Colour scale [°C]").strong());
    ui.label(
        RichText::new(
            "Diverging map: blue at min, neutral white at the MIDPOINT, red at \
             max. The midpoint carries meaning, so set the range about a \
             reference rather than clamping to the extremes seen.",
        )
        .small()
        .weak(),
    );
    ui.add(egui::Slider::new(&mut state.min_temp_degc, 0.0..=400.0).text("min"));
    ui.add(egui::Slider::new(&mut state.max_temp_degc, 100.0..=900.0).text("max"));
    if state.max_temp_degc <= state.min_temp_degc {
        state.max_temp_degc = state.min_temp_degc + 1.0;
    }

    ui.separator();
    ui.label(RichText::new("Layout").strong());
    ui.add(egui::Slider::new(&mut state.cell_width, 140.0..=380.0).text("card width [pt]"));
    ui.checkbox(&mut state.show_labels, "show internal labels");

    ui.separator();
    ui.label(
        RichText::new(
            "Every card here is scalar-fed. The same widget also renders a live \
             tampines::components::SteamGenerator — SteamGeneratorVisual::new \
             colours the steam space from its secondary-side control volume.",
        )
        .small()
        .weak(),
    );
}

/// Draws all three architectures side by side.
pub fn draw(ui: &mut egui::Ui, state: &SteamGeneratorTab, tracers: &Htr10Tracers) {
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let card_h = state.cell_width * CARD_ASPECT;
            let caption_h = 62.0;
            let gap = 14.0;
            let scalars = state.scalars();

            ui.horizontal_top(|ui| {
                for kind in SteamGeneratorKind::ALL {
                    let card_w = if kind.native_aspect_ratio() > 1.0 {
                        state.cell_width * WIDE_COLUMN_MULTIPLE
                    } else {
                        state.cell_width
                    };

                    ui.vertical(|ui| {
                        ui.set_width(card_w);

                        // Reserve the card and draw the generator into its
                        // centre; the widget letterboxes to its own native
                        // proportions inside that box.
                        let (rect, _response) =
                            ui.allocate_exact_size(Vec2::new(card_w, card_h), egui::Sense::hover());
                        let generator = SteamGeneratorVisual::from_scalars(
                            *kind,
                            rect.center(),
                            Vec2::new(card_w - 16.0, card_h - 16.0),
                            degc(state.min_temp_degc),
                            degc(state.max_temp_degc),
                            scalars,
                        );
                        let generator = if state.show_labels {
                            generator
                        } else {
                            generator.without_labels()
                        };
                        ui.put(rect, generator);

                        // Caption.
                        ui.allocate_ui(Vec2::new(card_w, caption_h), |ui| {
                            ui.label(RichText::new(kind.label()).strong());
                            ui.label(RichText::new(kind.description()).small().weak());
                            ui.label(RichText::new(kind.circulation()).small().weak());
                            let level = if kind.has_water_level() {
                                format!(
                                    "water level: {:.0} %",
                                    state.water_level_frac.clamp(0.0, 1.0) * 100.0
                                )
                            } else {
                                "water level: none (once-through)".to_string()
                            };
                            ui.label(RichText::new(level).small().weak());
                        });
                    });
                    ui.add_space(gap);
                }

                // ── HTR-10 general-structure schematic ────────────────────
                //
                // A different widget, not a fourth `SteamGeneratorKind`: it
                // draws one plant's specific internal arrangement rather than
                // a generic architecture, so it does not belong in that enum.
                ui.vertical(|ui| {
                    let card_w = state.cell_width;
                    ui.set_width(card_w);

                    let (rect, _response) =
                        ui.allocate_exact_size(Vec2::new(card_w, card_h), egui::Sense::hover());
                    let htr10 = Htr10SteamGeneratorVisual::new(
                        Vec2::new(card_w - 16.0, card_h - 16.0),
                        degc(state.min_temp_degc),
                        degc(state.max_temp_degc),
                        degc(state.primary_inlet_degc),
                        degc(state.primary_outlet_degc),
                        degc(state.feedwater_degc),
                        degc(state.steam_degc),
                    )
                    .with_riser_diameter_fraction(state.riser_fraction)
                    .with_coil_angle(Angle::new::<degree>(state.coil_angle_degrees));
                    let htr10 = if state.show_labels {
                        htr10
                    } else {
                        htr10.without_labels()
                    };
                    // Trains are copied in here, not owned by the widget.
                    ui.put(rect, tracers.attach(htr10));

                    ui.allocate_ui(Vec2::new(card_w, caption_h), |ui| {
                        ui.label(RichText::new("HTR-10 (general structure)").strong());
                        ui.label(
                            RichText::new(
                                "Central hot gas riser flanked by two peripheral helical \
                                 bundles; coils drawn as short parallel strokes.",
                            )
                            .small()
                            .weak(),
                        );
                        ui.label(
                            RichText::new(format!(
                                "riser {:.0} % of diameter, coils at {:.0}° — drawing \
                                 parameters, not plant dimensions",
                                state.riser_fraction * 100.0,
                                state.coil_angle_degrees,
                            ))
                            .small()
                            .weak(),
                        );
                    });
                });
                ui.add_space(gap);
            });

            ui.add_space(gap);
            ui.separator();
            ui.label(
                RichText::new(
                    "Offline demonstration art. Not for nuclear facility operation, \
                     reactor control, safety-critical decision-making, or licensing.",
                )
                .small()
                .weak(),
            );
        });
}

// ── HTR-10 tracer state ─────────────────────────────────────────────────────

/// The three tracer trains on the HTR-10 card, plus the flows that drive them.
///
/// Kept beside the tab state rather than inside the widget because widgets are
/// rebuilt every repaint: a train living in one would reset its phase to zero
/// each frame. The app advances these once per frame in `step` and copies them
/// into the widget at build time.
pub struct Htr10Tracers {
    /// Primary (helium) loop mass flow, kg/s. Drives the riser and the shell
    /// side. HTR-10 design value 4.32 kg/s
    /// (`docs/reactor-scoping/htr10-plant-data.md` section 6, *Quoted*).
    pub primary_mass_flow_kg_per_s: f64,
    /// Secondary (feedwater/steam) loop mass flow, kg/s. Drives the coil.
    /// HTR-10 design value 3.49 kg/s (same sheet, section 6, *Quoted*).
    pub secondary_mass_flow_kg_per_s: f64,
    /// Residence time of helium climbing the riser, s.
    ///
    /// A **display choice**, not a derived quantity: the riser is a schematic
    /// device (see the widget's module docs) with no stated dimensions, so
    /// there is no volume to divide a flow into. It is exposed as a slider and
    /// labelled as such rather than being quietly hardcoded.
    pub riser_residence_s: f64,
    /// Residence time of helium descending the shell side, s. Also a display
    /// choice, and deliberately longer than the riser's — the shell side is a
    /// much larger volume at a lower velocity.
    pub shell_residence_s: f64,
    /// Residence time of water through the coil, s. Display choice.
    pub coil_residence_s: f64,
    /// Residence time through either water-side nozzle, s. Display choice,
    /// and deliberately short — a nozzle is a far smaller volume than the
    /// coil it feeds, so its marks should visibly outrun the coil's.
    pub nozzle_residence_s: f64,
    riser: TracerTrain,
    shell: TracerTrain,
    coil: TracerTrain,
    feedwater: TracerTrain,
    steam: TracerTrain,
}

impl Default for Htr10Tracers {
    fn default() -> Self {
        Self {
            primary_mass_flow_kg_per_s: 4.32,
            secondary_mass_flow_kg_per_s: 3.49,
            riser_residence_s: 3.0,
            shell_residence_s: 9.0,
            coil_residence_s: 14.0,
            nozzle_residence_s: 2.5,
            riser: TracerTrain::new(4),
            shell: TracerTrain::new(5),
            coil: TracerTrain::new(6),
            feedwater: TracerTrain::new(3),
            steam: TracerTrain::new(3),
        }
    }
}

impl Htr10Tracers {
    /// Advance all three trains by `dt`.
    ///
    /// Each gets its own residence time and **its own loop's** mass flow, so
    /// the primary and secondary sides animate independently: stall the
    /// feedwater and the coil marks freeze while the gas keeps moving.
    pub fn step(&mut self, dt: Time) {
        let primary = MassRate::new::<kilogram_per_second>(self.primary_mass_flow_kg_per_s);
        let secondary = MassRate::new::<kilogram_per_second>(self.secondary_mass_flow_kg_per_s);
        self.riser
            .advance(dt, Time::new::<second>(self.riser_residence_s), primary);
        self.shell
            .advance(dt, Time::new::<second>(self.shell_residence_s), primary);
        self.coil
            .advance(dt, Time::new::<second>(self.coil_residence_s), secondary);
        // Both water-side nozzles carry the same secondary flow as the coil,
        // so they stall and reverse with it — but on their own, much shorter
        // residence time.
        let nozzle_tau = Time::new::<second>(self.nozzle_residence_s);
        self.feedwater.advance(dt, nozzle_tau, secondary);
        self.steam.advance(dt, nozzle_tau, secondary);
    }

    /// Attach this frame's trains to a widget.
    pub fn attach(&self, visual: Htr10SteamGeneratorVisual) -> Htr10SteamGeneratorVisual {
        visual
            .with_riser_tracer(self.riser.clone())
            .with_shell_gas_tracer(self.shell.clone())
            .with_coil_water_tracer(self.coil.clone())
            .with_feedwater_tracer(self.feedwater.clone())
            .with_steam_tracer(self.steam.clone())
    }
}
