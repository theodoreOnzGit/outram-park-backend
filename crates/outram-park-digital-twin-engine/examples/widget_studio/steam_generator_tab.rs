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
use outram_park_digital_twin_engine::components::steam_generator::{
    SteamGeneratorKind, SteamGeneratorScalars, SteamGeneratorVisual,
};
use uom::si::f64::ThermodynamicTemperature;
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
pub fn controls(ui: &mut egui::Ui, state: &mut SteamGeneratorTab) {
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
pub fn draw(ui: &mut egui::Ui, state: &SteamGeneratorTab) {
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
