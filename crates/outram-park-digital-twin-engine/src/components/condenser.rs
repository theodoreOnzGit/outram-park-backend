//! Schematic surface condenser: the physics wrapper.
//!
//! **Split 2026-11-05** (maintainer: "widget art can live in bugis, the
//! combination of logic and art can live inside the digital twin engine"):
//! the artwork, its kinds, scalars, display range and layout helpers are
//! [`bugis::components::condenser`], re-exported here. This file holds the
//! widget that wraps a [`tampines::components::Condenser`], and draws through
//! [`CondenserArt`].
//!
//! [`Condenser`] stores an **operating pressure** and a **target outlet
//! quality** and nothing else, so the physics-backed path ([`CondenserVisual::new`])
//! draws [`CondenserArt::unpainted`]: the complete machine in neutral metal
//! tones, labelled with the operating pressure it really holds. The target
//! outlet quality is readable through [`CondenserVisual::target_outlet_quality`]
//! but is never painted. The state-driven path is
//! [`CondenserVisual::from_scalars`]. See the bugis module for the full
//! description of what each path paints.

pub use bugis::components::condenser::*;
use egui::{Pos2, Rect, Response, Ui, Vec2, Widget};
use tampines::components::Condenser;
use uom::si::f64::Pressure;

/// Where a [`CondenserVisual`] gets the state it renders.
///
/// Enum dispatch, not a trait object, per the workspace's mandatory "no trait
/// objects" Rust design rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CondenserVisualState {
    /// Backed by a [`tampines::components::Condenser`] alone.
    ///
    /// That component holds an operating pressure and a **target** outlet
    /// quality and nothing else — no fluid state, no cooling-water side, no
    /// hotwell level — so this path draws the complete machine in neutral metal
    /// tones and paints no temperature or quality anywhere.
    Physics(Condenser),
    /// Backed by caller-supplied scalars from the caller's own plant model,
    /// graded against the accompanying [`CondenserDisplayRange`].
    Scalars(CondenserScalars, CondenserDisplayRange),
}

/// Visual representation of a surface condenser, in one of two cooling-water
/// arrangements.
///
/// Built either from a [`tampines::components::Condenser`] ([`Self::new`],
/// which draws the machine neutrally because that component holds no fluid
/// state) or from the caller's own scalar plant state ([`Self::from_scalars`]).
///
/// The artwork letterboxes to [`CondenserKind::native_aspect_ratio`] inside the
/// box it is given, so it never stretches.
pub struct CondenserVisual {
    state: CondenserVisualState,
    art: CondenserArt,
}

impl CondenserVisual {
    /// Wrap a [`Condenser`] with the given screen geometry.
    ///
    /// **Signature preserved** from the original placeholder widget, so every
    /// existing call site keeps working unchanged.
    ///
    /// This is the physics-backed path: [`CondenserArt::unpainted`], labelled
    /// with the component's own `pressure`. Nothing is fabricated to fill the
    /// gap — for a coloured condenser, pass the state you actually have to
    /// [`Self::from_scalars`].
    ///
    /// Defaults to [`CondenserKind::TwoPass`]; change it with
    /// [`Self::with_kind`].
    pub fn new(physics: Condenser, screen_position: Pos2, screen_vector: Vec2) -> Self {
        Self {
            art: CondenserArt::unpainted(screen_position, screen_vector)
                .with_operating_pressure(physics.pressure),
            state: CondenserVisualState::Physics(physics),
        }
    }

    /// Build a condenser visual from the caller's own scalar plant state,
    /// graded against `range` ([`CondenserArt::from_scalars`]).
    pub fn from_scalars(
        kind: CondenserKind,
        screen_position: Pos2,
        screen_vector: Vec2,
        range: CondenserDisplayRange,
        scalars: CondenserScalars,
    ) -> Self {
        Self {
            state: CondenserVisualState::Scalars(scalars, range),
            art: CondenserArt::from_scalars(kind, screen_position, screen_vector, range, scalars),
        }
    }

    /// Draw a different cooling-water arrangement with the same state.
    pub fn with_kind(mut self, kind: CondenserKind) -> Self {
        self.art = self.art.with_kind(kind);
        self
    }

    /// Label the shell with a known operating (shell-side) pressure.
    ///
    /// Set automatically by [`Self::new`] from the component's own
    /// `pressure` field.
    pub fn with_operating_pressure(mut self, pressure: Pressure) -> Self {
        self.art = self.art.with_operating_pressure(pressure);
        self
    }

    /// Turn the internal component labels off — for thumbnails.
    pub fn without_labels(mut self) -> Self {
        self.art = self.art.without_labels();
        self
    }

    /// Which cooling-water arrangement this visual draws.
    pub fn kind(&self) -> CondenserKind {
        self.art.kind()
    }

    /// On-screen size of the box the artwork letterboxes into, in points.
    pub fn size(&self) -> Vec2 {
        self.art.size()
    }

    /// Where this visual gets its state.
    pub fn state(&self) -> &CondenserVisualState {
        &self.state
    }

    /// The scalar state the artwork is drawn from, or `None` on the
    /// physics-backed path — which has none, and says so rather than
    /// synthesising one.
    pub fn scalars(&self) -> Option<CondenserScalars> {
        match self.state {
            CondenserVisualState::Physics(_) => None,
            CondenserVisualState::Scalars(s, _) => Some(s),
        }
    }

    /// The shell-side operating pressure, if one is known.
    ///
    /// `Some` on the physics path (read from [`Condenser::pressure`]) and on
    /// any visual given [`Self::with_operating_pressure`]; `None` otherwise.
    pub fn operating_pressure(&self) -> Option<Pressure> {
        self.art.operating_pressure()
    }

    /// The wrapped component's **target** outlet quality, dimensionless in
    /// `[0, 1]`, or `None` on the scalar path.
    ///
    /// Exposed for a caller that wants to display the set-point, and
    /// deliberately **never painted** by this widget: it is what the condenser
    /// is asked to achieve, not what the fluid is. Colouring the condensate by
    /// it would be fabricating state. The quantity the artwork actually paints
    /// is [`CondenserScalars::exhaust_quality`], which the caller supplies.
    pub fn target_outlet_quality(&self) -> Option<f64> {
        match self.state {
            CondenserVisualState::Physics(c) => Some(c.target_outlet_quality),
            CondenserVisualState::Scalars(..) => None,
        }
    }

    /// The drawing this widget paints.
    pub fn art(&self) -> &CondenserArt {
        &self.art
    }

    /// Port positions for a condenser of `kind` whose box is `box_rect`
    /// ([`CondenserArt::ports`]).
    pub fn ports(kind: CondenserKind, box_rect: Rect) -> CondenserPorts {
        CondenserArt::ports(kind, box_rect)
    }
}

impl Widget for CondenserVisual {
    /// Draws the [`CondenserArt`]: neutral on the physics-backed path,
    /// graded by the caller's scalars on the other.
    fn ui(self, ui: &mut Ui) -> Response {
        self.art.ui(ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::pressure::kilopascal;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::f64::ThermodynamicTemperature;

    /// The physics-backed path draws the unpainted machine, keeps the
    /// operating pressure as real stored state, and reads the target outlet
    /// quality without painting it.
    ///
    /// **Methodology.** Wrap a 6.6 kPa `Condenser` with target outlet quality
    /// 0.0 and require: no scalars, the pressure on the art, the set-point
    /// through its accessor, the default kind and size, and a drawing. The
    /// neutral-colour half of this check is bugis's
    /// `the_unpainted_path_paints_nothing_it_cannot_see` (the resolved drawing
    /// state is private to the art).
    ///
    /// **Result (2026-11-05, after the split):** as before the split
    /// (2026-08-12): `operating_pressure()` 6.6 kPa, `target_outlet_quality()`
    /// 0.0, `scalars()` `None`.
    #[test]
    fn the_physics_path_draws_unpainted_with_its_pressure() {
        let pressure = Pressure::new::<kilopascal>(6.6);
        let visual = CondenserVisual::new(
            Condenser::new(pressure, 0.0),
            Pos2::new(200.0, 200.0),
            Vec2::new(320.0, 220.0),
        );
        assert!(visual.scalars().is_none());
        assert!(visual.art().scalars().is_none());
        assert_eq!(visual.operating_pressure(), Some(pressure));
        assert_eq!(visual.target_outlet_quality(), Some(0.0));
        assert_eq!(visual.kind(), CondenserKind::TwoPass);
        assert_eq!(visual.size(), Vec2::new(320.0, 220.0));
        assert!(matches!(visual.state(), CondenserVisualState::Physics(_)));
        assert!(bugis::components::shapes_drawn_by(visual) > 0);
    }

    /// The scalar path hands the caller's state to the art unchanged, and the
    /// builders reach the art.
    #[test]
    fn the_scalar_path_and_builders_reach_the_art() {
        let degc = |v: f64| ThermodynamicTemperature::new::<degree_celsius>(v);
        let range = CondenserDisplayRange {
            min_temp: degc(10.0),
            max_temp: degc(310.0),
        };
        let scalars = CondenserScalars {
            exhaust_quality: 0.90,
            condensing_temp: degc(38.0),
            condensate_temp: degc(36.0),
            cooling_water_inlet_temp: degc(22.0),
            cooling_water_outlet_temp: degc(32.0),
            hotwell_level_frac: Some(0.45),
        };
        let pressure = Pressure::new::<kilopascal>(6.6);
        let visual = CondenserVisual::from_scalars(
            CondenserKind::TwoPass,
            Pos2::new(200.0, 200.0),
            Vec2::new(320.0, 220.0),
            range,
            scalars,
        )
        .with_kind(CondenserKind::ALL[CondenserKind::ALL.len() - 1])
        .with_operating_pressure(pressure)
        .without_labels();
        assert_eq!(visual.scalars(), Some(scalars));
        assert_eq!(visual.art().scalars(), Some(scalars));
        assert_eq!(visual.target_outlet_quality(), None);
        assert_eq!(visual.operating_pressure(), Some(pressure));
        assert_eq!(visual.kind(), CondenserKind::ALL[CondenserKind::ALL.len() - 1]);
        let box_rect = Rect::from_center_size(Pos2::new(200.0, 200.0), Vec2::new(320.0, 220.0));
        assert_eq!(
            CondenserVisual::ports(visual.kind(), box_rect),
            CondenserArt::ports(visual.kind(), box_rect)
        );
        assert!(bugis::components::shapes_drawn_by(visual) > 0);
    }
}
