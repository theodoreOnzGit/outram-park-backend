//! User-added custom thermodynamic lines.
//!
//! Issue #26 asks for two overlapping things: "user-selectable isobars and
//! isotherms" (pick specific values rather than the fixed default set) and a
//! "custom thermodynamic line" control (isobar / isotherm / isentrope /
//! isenthalp / isochore at a user-chosen value). Both are the same mechanism
//! here — [`CustomLineType`] already includes isobar and isotherm, so adding
//! one at a chosen pressure or temperature *is* the "user-selectable" request,
//! without a second, parallel UI for the same idea.
//!
//! [`CustomLineType::Isentrope`] and [`CustomLineType::Isenthalp`] sweep
//! pressure through this crate's own `(p,s)`/`(p,h)` flashes
//! ([`curves::isentrope`], [`curves::isenthalp`]); [`CustomLineType::Isochore`]
//! has no such flash to reuse, so [`curves::isochore`] bisects pressure at
//! each temperature against the forward single-phase volume dispatcher — see
//! that function's doc comment for why.

use eframe::egui;
use uom::si::available_energy::kilojoule_per_kilogram;
use uom::si::f64::*;
use uom::si::pressure::bar;
use uom::si::specific_heat_capacity::kilojoule_per_kilogram_kelvin;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::degree_celsius;

use crate::curves;
use crate::data::{CustomLineMetadata, LayerKind, PlotLayer};
use crate::figure::{Rgb, SeriesStyle};

/// Which physical quantity a custom line holds constant.
///
/// An enum, not a trait object: the set of line types is closed, and adding a
/// sixth should be a compile error at every `match` site, per the workspace
/// Rust design rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomLineType {
    /// Constant pressure.
    Isobar,
    /// Constant temperature.
    Isotherm,
    /// Constant specific entropy.
    Isentrope,
    /// Constant specific enthalpy.
    Isenthalp,
    /// Constant specific volume.
    Isochore,
}

impl CustomLineType {
    /// All five, in the order the type selector shows them.
    pub const ALL: [CustomLineType; 5] = [
        CustomLineType::Isobar,
        CustomLineType::Isotherm,
        CustomLineType::Isentrope,
        CustomLineType::Isenthalp,
        CustomLineType::Isochore,
    ];

    /// Selector label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Isobar => "Isobar",
            Self::Isotherm => "Isotherm",
            Self::Isentrope => "Isentrope",
            Self::Isenthalp => "Isenthalp",
            Self::Isochore => "Isochore",
        }
    }

    /// Unit shown next to the slider/numeric input.
    pub fn unit_display(self) -> &'static str {
        match self {
            Self::Isobar => "bar",
            Self::Isotherm => "\u{00B0}C",
            Self::Isentrope => "kJ/(kg\u{00B7}K)",
            Self::Isenthalp => "kJ/kg",
            Self::Isochore => "m\u{00B3}/kg",
        }
    }

    /// Unit as written in the CSV export (plain ASCII, matching the crate's
    /// existing CSV column-naming convention).
    fn unit_csv(self) -> &'static str {
        match self {
            Self::Isobar => "bar",
            Self::Isotherm => "degC",
            Self::Isentrope => "kJ/(kg K)",
            Self::Isenthalp => "kJ/kg",
            Self::Isochore => "m3/kg",
        }
    }

    /// The CSV `custom_line_type` value.
    fn key_csv(self) -> &'static str {
        match self {
            Self::Isobar => "isobar",
            Self::Isotherm => "isotherm",
            Self::Isentrope => "isentrope",
            Self::Isenthalp => "isenthalp",
            Self::Isochore => "isochore",
        }
    }

    /// Sensible slider `(min, max)` bounds, in [`CustomLineType::unit_display`]
    /// (issue #26: "Use sensible defaults depending on line type"). Delegates
    /// to the `curves` module's own range constants, which are stated once
    /// there against the physical bounds every sweep in this tool is already
    /// clamped to.
    pub fn slider_range(self) -> (f64, f64) {
        match self {
            Self::Isobar => curves::CUSTOM_ISOBAR_RANGE_BAR,
            Self::Isotherm => curves::CUSTOM_ISOTHERM_RANGE_DEGC,
            Self::Isentrope => curves::CUSTOM_ISENTROPE_RANGE_KJ_PER_KG_K,
            Self::Isenthalp => curves::CUSTOM_ISENTHALP_RANGE_KJ_PER_KG,
            Self::Isochore => curves::CUSTOM_ISOCHORE_RANGE_M3_PER_KG,
        }
    }

    /// A reasonable starting value for the "Add line" control when this type
    /// is first selected: the slider range's midpoint, except isobar and
    /// isochore (both log-scaled physically) start near 1 bar / 1 m^3/kg
    /// rather than at a linear midpoint that would land implausibly high.
    pub fn default_value(self) -> f64 {
        match self {
            Self::Isobar => 10.0,
            Self::Isochore => 1.0,
            _ => {
                let (lo, hi) = self.slider_range();
                0.5 * (lo + hi)
            }
        }
    }
}

/// One user-added line: a type and the value that defines it, in
/// [`CustomLineType::unit_display`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CustomLine {
    /// Which quantity is held constant.
    pub line_type: CustomLineType,
    /// The value, in `line_type.unit_display()`.
    pub value: f64,
}

impl CustomLine {
    /// Builds this line into a [`PlotLayer`], computing it live from
    /// `tampines-steam-tables` via [`curves`]. Returns `None` if the crate
    /// could not evaluate a usable curve anywhere along the sweep (fewer than
    /// two points survive) — dropped, never fabricated, matching every other
    /// layer in this tool.
    pub fn build(&self, samples: usize, colour: Rgb) -> Option<PlotLayer> {
        let segments = match self.line_type {
            CustomLineType::Isobar => curves::isobar(Pressure::new::<bar>(self.value), samples),
            CustomLineType::Isotherm => curves::isotherm(
                ThermodynamicTemperature::new::<degree_celsius>(self.value),
                samples,
            ),
            CustomLineType::Isentrope => curves::isentrope(
                SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(self.value),
                samples,
            ),
            CustomLineType::Isenthalp => curves::isenthalp(
                AvailableEnergy::new::<kilojoule_per_kilogram>(self.value),
                samples,
            ),
            CustomLineType::Isochore => curves::isochore(
                SpecificVolume::new::<cubic_meter_per_kilogram>(self.value),
                samples,
            ),
        };
        if segments.iter().map(Vec::len).sum::<usize>() < 2 {
            return None;
        }
        Some(PlotLayer {
            label: format!(
                "Custom {} ({} = {:.4} {})",
                self.line_type.label().to_lowercase(),
                self.line_type.label(),
                self.value,
                self.line_type.unit_display()
            ),
            kind: LayerKind::ComputedCurve,
            provenance: "computed live from tampines-steam-tables (user-added custom line)"
                .to_string(),
            segments,
            style: SeriesStyle::Line {
                width: 1.1,
                dash: Some((5.0, 2.0)),
            },
            colour,
            show_in_legend: true,
            legend_group: "Custom lines",
            custom_line: Some(CustomLineMetadata {
                line_type: self.line_type.key_csv(),
                value: self.value,
                unit: self.line_type.unit_csv(),
            }),
        })
    }
}

/// The `egui::Color32`-space rotation custom lines cycle through, so several
/// added lines stay visually distinct from each other. Deliberately not the
/// same [`crate::figure::PALETTE`] entries used for the built-in overlay
/// layers, so a custom line never looks like it might be one of them.
const CUSTOM_LINE_COLOURS: [Rgb; 4] = [
    Rgb::new(0xe0, 0x3a, 0x8c), // magenta-pink
    Rgb::new(0x00, 0x9e, 0x7a), // teal-green
    Rgb::new(0xff, 0x8c, 0x00), // orange
    Rgb::new(0x6a, 0x3d, 0xd1), // violet
];

/// Colour for the `index`-th custom line, cycling through
/// [`CUSTOM_LINE_COLOURS`].
pub fn colour_for_index(index: usize) -> Rgb {
    CUSTOM_LINE_COLOURS[index % CUSTOM_LINE_COLOURS.len()]
}

/// The type-selector row plus the value slider/numeric-input row.
///
/// Returns `true` if the "Add line" button was clicked and the resulting
/// [`CustomLine`] is worth adding. Does not touch app state itself, so the
/// caller stays in control of *where* the new line is appended.
pub fn add_line_controls(
    ui: &mut egui::Ui,
    line_type: &mut CustomLineType,
    value: &mut f64,
) -> Option<CustomLine> {
    ui.horizontal_wrapped(|ui| {
        for candidate in CustomLineType::ALL {
            if ui
                .selectable_value(line_type, candidate, candidate.label())
                .clicked()
            {
                *value = candidate.default_value();
            }
        }
    });
    let (lo, hi) = line_type.slider_range();
    *value = value.clamp(lo, hi);
    ui.horizontal(|ui| {
        ui.add(egui::Slider::new(value, lo..=hi).text(line_type.unit_display()));
        ui.add(egui::DragValue::new(value).speed((hi - lo) / 200.0));
    });
    if ui.button("Add line").clicked() {
        Some(CustomLine {
            line_type: *line_type,
            value: *value,
        })
    } else {
        None
    }
}

/// Checks every [`CustomLineType`] builds a usable, correctly-tagged
/// [`PlotLayer`] at its own default value, and that the CSV metadata
/// ([`crate::data::CustomLineMetadata`]) survives the round trip issue #26
/// asks for ("include metadata such as: line_type, line_value... unit").
///
/// # Methodology
///
/// For each of the five line types, builds a [`CustomLine`] at
/// `line_type.default_value()` and asserts: the layer is produced (`Some`,
/// not dropped); it carries `custom_line = Some(...)` with `value` equal to
/// what was requested and `line_type`/`unit` matching this type's own
/// `key_csv`/`unit_csv`; and it has at least two points to actually draw.
///
/// # Result (measured 2026-08-20)
///
/// Passes for all five types.
#[cfg(test)]
#[test]
fn every_custom_line_type_builds_a_tagged_layer_at_its_default_value() {
    for line_type in CustomLineType::ALL {
        let line = CustomLine {
            line_type,
            value: line_type.default_value(),
        };
        let layer = line
            .build(60, Rgb::new(0, 0, 0))
            .unwrap_or_else(|| panic!("{line_type:?} produced no layer at its default value"));
        assert_eq!(layer.kind, LayerKind::ComputedCurve);
        assert!(
            layer.point_count() >= 2,
            "{line_type:?}: too few points to draw"
        );
        let metadata = layer
            .custom_line
            .unwrap_or_else(|| panic!("{line_type:?}: PlotLayer.custom_line was None"));
        assert_eq!(metadata.line_type, line_type.key_csv());
        assert_eq!(metadata.unit, line_type.unit_csv());
        assert!(
            (metadata.value - line.value).abs() < 1.0e-9,
            "{line_type:?}: metadata.value {} != requested {}",
            metadata.value,
            line.value
        );
    }
}
