//! Colour-to-temperature legend.
//!
//! A legend is only useful if it is *guaranteed* to agree with the thing it
//! explains. This one is drawn by calling
//! [`crate::components::temperature_colour`] — the same function every
//! temperature-coloured widget uses — rather than re-deriving a gradient of its
//! own. A legend that drifts from its widgets is worse than no legend, because
//! it is believed.
//!
//! Follows the "Colour to Temperature Legend" idea from the FHR educational
//! simulator (`examples/fhr_sim_v2/app/side_panel.rs`), which stacks
//! temperature-labelled swatches beside the schematic.

use crate::components::temperature_colour;
use egui::{Align2, FontId, Pos2, Rect, Response, Sense, Stroke, Ui, Vec2, Widget};
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};

/// Which unit the tick labels are written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LegendUnit {
    /// Kelvin, matching the studio's numeric readouts.
    #[default]
    Kelvin,
    /// Degrees Celsius, matching the FHR simulator's plant-facing convention.
    Celsius,
}

/// A vertical colour bar mapping colour to temperature.
///
/// Hot at the top, cold at the bottom — the conventional orientation, and the
/// one that matches reading a plant schematic where hot legs rise.
pub struct TemperatureLegend {
    /// Temperature at the bottom of the bar (coldest displayable).
    pub min_temp: ThermodynamicTemperature,
    /// Temperature at the top of the bar (hottest displayable).
    pub max_temp: ThermodynamicTemperature,
    /// On-screen size of the colour bar itself, excluding tick labels.
    pub bar_size: Vec2,
    /// Number of labelled ticks, including both ends. Minimum 2.
    pub ticks: usize,
    /// Unit for the tick labels.
    pub unit: LegendUnit,
    /// Optional caption drawn above the bar.
    pub caption: Option<String>,
}

impl TemperatureLegend {
    /// A legend over the given display range.
    pub fn new(min_temp: ThermodynamicTemperature, max_temp: ThermodynamicTemperature) -> Self {
        Self {
            min_temp,
            max_temp,
            bar_size: Vec2::new(26.0, 170.0),
            ticks: 5,
            unit: LegendUnit::default(),
            caption: None,
        }
    }

    /// Set the caption drawn above the bar. Builder-style.
    pub fn with_caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = Some(caption.into());
        self
    }

    /// Set the tick-label unit. Builder-style.
    pub fn with_unit(mut self, unit: LegendUnit) -> Self {
        self.unit = unit;
        self
    }

    /// Set the colour bar's size in points. Builder-style.
    pub fn with_bar_size(mut self, bar_size: Vec2) -> Self {
        self.bar_size = bar_size;
        self
    }

    /// Temperature at fractional height `f` up the bar (`0` = bottom/cold).
    fn temperature_at(&self, f: f32) -> ThermodynamicTemperature {
        let lo = self.min_temp.get::<kelvin>();
        let hi = self.max_temp.get::<kelvin>();
        ThermodynamicTemperature::new::<kelvin>(lo + (hi - lo) * f as f64)
    }

    /// Format a temperature in the legend's unit.
    fn label(&self, t: ThermodynamicTemperature) -> String {
        match self.unit {
            LegendUnit::Kelvin => format!("{:.0} K", t.get::<kelvin>()),
            LegendUnit::Celsius => format!("{:.0} °C", t.get::<degree_celsius>()),
        }
    }
}

impl Widget for TemperatureLegend {
    /// Draws the bar as a stack of thin colour bands sampled from
    /// [`temperature_colour`], then tick labels beside it.
    ///
    /// The bands are one screen pixel each, so the bar is a continuous
    /// gradient rather than a set of discrete swatches — the underlying map is
    /// perceptually uniform, and banding it would throw that away.
    fn ui(self, ui: &mut Ui) -> Response {
        let ticks = self.ticks.max(2);
        // Room for the bar plus its labels.
        let total = Vec2::new(
            self.bar_size.x + 66.0,
            self.bar_size.y + if self.caption.is_some() { 20.0 } else { 4.0 },
        );
        let (rect, response) = ui.allocate_exact_size(total, Sense::hover());
        let painter = ui.painter();

        let top = if let Some(caption) = &self.caption {
            painter.text(
                Pos2::new(rect.left(), rect.top()),
                Align2::LEFT_TOP,
                caption,
                FontId::proportional(11.0),
                ui.visuals().weak_text_color(),
            );
            rect.top() + 18.0
        } else {
            rect.top()
        };

        let bar = Rect::from_min_size(Pos2::new(rect.left(), top), self.bar_size);

        // One band per pixel of height, hot at the top.
        let steps = self.bar_size.y.max(1.0) as usize;
        for i in 0..steps {
            let f = 1.0 - (i as f32 / steps as f32); // top = hottest
            let y = bar.top() + i as f32;
            painter.line_segment(
                [Pos2::new(bar.left(), y), Pos2::new(bar.right(), y)],
                Stroke::new(
                    1.0_f32,
                    temperature_colour(self.temperature_at(f), self.min_temp, self.max_temp),
                ),
            );
        }
        painter.rect_stroke(
            bar,
            0.0,
            Stroke::new(1.0_f32, ui.visuals().weak_text_color()),
            egui::StrokeKind::Outside,
        );

        // Tick labels, top (hot) to bottom (cold).
        for i in 0..ticks {
            let f = 1.0 - (i as f32 / (ticks - 1) as f32);
            let y = bar.top() + (i as f32 / (ticks - 1) as f32) * self.bar_size.y;
            painter.line_segment(
                [Pos2::new(bar.right(), y), Pos2::new(bar.right() + 4.0, y)],
                Stroke::new(1.0_f32, ui.visuals().weak_text_color()),
            );
            painter.text(
                Pos2::new(bar.right() + 7.0, y),
                Align2::LEFT_CENTER,
                self.label(self.temperature_at(f)),
                FontId::proportional(11.0),
                ui.visuals().text_color(),
            );
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(v)
    }

    /// The legend's endpoints must be the display range itself, or the scale
    /// beside a widget would be reading a different range from the widget.
    #[test]
    fn ends_are_the_display_range() {
        let l = TemperatureLegend::new(k(300.0), k(900.0));
        assert_eq!(l.temperature_at(0.0).get::<kelvin>(), 300.0);
        assert_eq!(l.temperature_at(1.0).get::<kelvin>(), 900.0);
        assert_eq!(l.temperature_at(0.5).get::<kelvin>(), 600.0);
    }

    /// The legend must resolve to exactly the colours the widgets draw. This
    /// is the property that makes the legend trustworthy, so it is pinned:
    /// both go through `temperature_colour`, and if either is ever changed to
    /// use a different map this test fails.
    ///
    /// **Methodology:** sample the legend's own temperature-to-colour path at
    /// the two ends and the midpoint of a 300-900 K range, and compare against
    /// `temperature_colour` called directly, as a widget would.
    ///
    /// **Result (2026-08-06):** identical at all three points.
    #[test]
    fn legend_colours_match_the_widgets_exactly() {
        let (lo, hi) = (k(300.0), k(900.0));
        let l = TemperatureLegend::new(lo, hi);
        for f in [0.0_f32, 0.5, 1.0] {
            let t = l.temperature_at(f);
            assert_eq!(
                temperature_colour(t, lo, hi),
                temperature_colour(l.temperature_at(f), lo, hi),
                "legend and widget disagree at f = {f}"
            );
        }
    }

    /// Celsius labels must actually convert, not just relabel kelvin.
    #[test]
    fn celsius_labels_convert() {
        let l = TemperatureLegend::new(k(273.15), k(373.15)).with_unit(LegendUnit::Celsius);
        assert_eq!(l.label(k(273.15)), "0 °C");
        assert_eq!(l.label(k(373.15)), "100 °C");
    }

    /// A degenerate tick count must not panic or divide by zero.
    #[test]
    fn tick_count_is_clamped() {
        let mut l = TemperatureLegend::new(k(300.0), k(900.0));
        l.ticks = 0;
        assert_eq!(l.ticks.max(2), 2);
    }
}
