//! Visual valve: the physics wrapper.
//!
//! Wraps [`tampines::components::Valve`] with screen geometry. Colours
//! itself by opening percentage (fully closed = grey, fully open = green) --
//! [`Valve::current_kv`] is real/working, so this is real data, not a
//! placeholder.
//!
//! **Split 2026-11-05** (maintainer: "widget art can live in bugis, the
//! combination of logic and art can live inside the digital twin engine"):
//! the drawing is [`bugis::components::valve::ValveArt`], re-exported here;
//! this file reads the opening from the physics and hands it over.

pub use bugis::components::valve::*;
use egui::{Pos2, Response, Ui, Vec2, Widget};
use tampines::components::Valve;
use uom::si::ratio::ratio;

/// Visual representation of a [`Valve`].
pub struct ValveVisual {
    /// The underlying physics component.
    pub physics: Valve,
    /// On-screen centre position.
    pub screen_position: Pos2,
    /// On-screen size.
    pub screen_vector: Vec2,
}

impl ValveVisual {
    /// Wrap a [`Valve`] with the given screen geometry.
    pub fn new(physics: Valve, screen_position: Pos2, screen_vector: Vec2) -> Self {
        Self {
            physics,
            screen_position,
            screen_vector,
        }
    }

    /// The drawing of this valve: its opening read off the physics, as the
    /// widget always read it (`opening_percent` in `ratio` units, over 100).
    pub fn art(&self) -> ValveArt {
        let opening = (self.physics.opening_percent.get::<ratio>() / 100.0) as f32;
        ValveArt::new(opening, self.screen_position, self.screen_vector)
    }
}

impl Widget for ValveVisual {
    /// Draws [`ValveVisual::art`].
    fn ui(self, ui: &mut Ui) -> Response {
        self.art().ui(ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_park_fork_dwsim_libs::valve::iec_60534::{
        OpeningCharacteristic, ValveFlowCoefficient,
    };
    use uom::si::f64::Ratio;

    /// The physics opening reaches the art unchanged, and the widget draws.
    #[test]
    fn the_valve_opening_comes_from_the_physics() {
        let valve = Valve::new(
            ValveFlowCoefficient(10.0),
            OpeningCharacteristic::Linear,
            Ratio::new::<ratio>(50.0),
        );
        let visual = ValveVisual::new(valve, Pos2::new(20.0, 20.0), Vec2::new(10.0, 10.0));
        assert!((visual.art().opening - 0.5).abs() < 1e-6);
        assert!(bugis::components::shapes_drawn_by(visual) > 0);
    }
}
