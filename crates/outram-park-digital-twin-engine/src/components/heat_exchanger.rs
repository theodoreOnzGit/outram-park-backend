//! Schematic heat exchanger: the physics wrapper.
//!
//! **Split 2026-11-05** (maintainer: "widget art can live in bugis, the
//! combination of logic and art can live inside the digital twin engine"):
//! the artwork, its kinds and constructions, the scalars and the approach
//! helpers ([`terminal_approaches`], [`approach_verdict`], ...) are
//! [`bugis::components::heat_exchanger`], re-exported here. This file holds
//! the widget that wraps a [`tampines::components::HeatExchanger`], and the
//! mapping between [`HeatExchangerKind`] and DWSIM's [`FlowArrangement`]
//! ([`HeatExchangerKindArrangement`]); it draws through [`HeatExchangerArt`].
//!
//! [`tampines::components::HeatExchanger`] holds a flow arrangement, a
//! heat-transfer area and an overall coefficient, and no fluid state (its
//! `calculate` returns `NotYetImplemented`). So [`HeatExchangerVisual::new`]
//! draws [`HeatExchangerArt::unpainted`] in the component's own arrangement,
//! labelled with its area and `U`, and paints no temperature. See the bugis
//! module for the full description.

pub use bugis::components::heat_exchanger::*;
use egui::{Pos2, Response, Ui, Vec2, Widget};
use outram_park_fork_dwsim_libs::heat_exchanger::lmtd::FlowArrangement;
use tampines::components::HeatExchanger;
use uom::si::f64::{Area, HeatTransfer, Power, TemperatureInterval};

/// The one-to-one mapping between [`HeatExchangerKind`] and the
/// [`FlowArrangement`] the physics libraries store and take (moved from the
/// bugis enum's inherent methods, 2026-11-05: bugis depends on no physics
/// crate). Bring it into scope to call `HeatExchangerKind::from_arrangement`
/// and `kind.arrangement()`.
pub trait HeatExchangerKindArrangement: Sized {
    /// The arrangement [`tampines::components::HeatExchanger`] stores, drawn as
    /// this kind.
    fn from_arrangement(arrangement: FlowArrangement) -> Self;

    /// This kind as the [`FlowArrangement`] the physics libraries take, so a
    /// caller can hand the studio's selection straight to a rating routine.
    fn arrangement(self) -> FlowArrangement;
}

impl HeatExchangerKindArrangement for HeatExchangerKind {
    fn from_arrangement(arrangement: FlowArrangement) -> Self {
        match arrangement {
            FlowArrangement::CounterCurrent => Self::CounterFlow,
            FlowArrangement::CoCurrent => Self::ParallelFlow,
        }
    }

    fn arrangement(self) -> FlowArrangement {
        match self {
            Self::CounterFlow => FlowArrangement::CounterCurrent,
            Self::ParallelFlow => FlowArrangement::CoCurrent,
        }
    }
}

/// Where a [`HeatExchangerVisual`] gets the state it renders.
///
/// Enum dispatch, not a trait object, per the workspace's mandatory "no trait
/// objects" Rust design rule.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeatExchangerVisualState {
    /// Backed by a [`tampines::components::HeatExchanger`] alone: its
    /// arrangement, area and coefficient are drawn or labelled; it has no
    /// fluid state, so no temperature is painted.
    Physics(HeatExchanger),
    /// Backed by caller-supplied scalars from the caller's own plant model,
    /// graded against the accompanying [`HeatExchangerDisplayRange`].
    Scalars(HeatExchangerScalars, HeatExchangerDisplayRange),
}

/// Visual representation of a two-stream recuperator: a [`HeatExchangerArt`]
/// built from a [`tampines::components::HeatExchanger`] ([`Self::new`]) or from
/// the caller's own scalar plant state ([`Self::from_scalars`]).
pub struct HeatExchangerVisual {
    state: HeatExchangerVisualState,
    art: HeatExchangerArt,
}

impl HeatExchangerVisual {
    /// Wrap a [`HeatExchanger`] with the given screen geometry.
    ///
    /// **Signature preserved** from the original placeholder widget. Draws
    /// [`HeatExchangerArt::unpainted`] in the component's own `arrangement`,
    /// labelled with its `area` and `overall_coefficient`.
    ///
    /// Defaults to [`HeatExchangerConstruction::ShellAndTube`], which the
    /// component says nothing about; change it with [`Self::with_construction`].
    pub fn new(physics: HeatExchanger, screen_position: Pos2, screen_vector: Vec2) -> Self {
        Self {
            art: HeatExchangerArt::unpainted(
                HeatExchangerKind::from_arrangement(physics.arrangement),
                screen_position,
                screen_vector,
            )
            .with_surface(physics.area, physics.overall_coefficient),
            state: HeatExchangerVisualState::Physics(physics),
        }
    }

    /// Build a heat-exchanger visual from the caller's own scalar plant state
    /// ([`HeatExchangerArt::from_scalars`]).
    pub fn from_scalars(
        kind: HeatExchangerKind,
        screen_position: Pos2,
        screen_vector: Vec2,
        range: HeatExchangerDisplayRange,
        scalars: HeatExchangerScalars,
    ) -> Self {
        Self {
            state: HeatExchangerVisualState::Scalars(scalars, range),
            art: HeatExchangerArt::from_scalars(kind, screen_position, screen_vector, range, scalars),
        }
    }

    /// Draw a different flow arrangement with the same state.
    pub fn with_kind(mut self, kind: HeatExchangerKind) -> Self {
        self.art = self.art.with_kind(kind);
        self
    }

    /// Draw a different construction with the same state.
    pub fn with_construction(mut self, construction: HeatExchangerConstruction) -> Self {
        self.art = self.art.with_construction(construction);
        self
    }

    /// Label the exchanger with a known area and overall coefficient.
    pub fn with_surface(mut self, area: Area, overall_coefficient: HeatTransfer) -> Self {
        self.art = self.art.with_surface(area, overall_coefficient);
        self
    }

    /// Turn the internal component labels off — for thumbnails.
    pub fn without_labels(mut self) -> Self {
        self.art = self.art.without_labels();
        self
    }

    /// Which flow arrangement this visual draws.
    pub fn kind(&self) -> HeatExchangerKind {
        self.art.kind()
    }

    /// Which construction this visual draws.
    pub fn construction(&self) -> HeatExchangerConstruction {
        self.art.construction()
    }

    /// On-screen size of the box the artwork letterboxes into, in points.
    pub fn size(&self) -> Vec2 {
        self.art.size()
    }

    /// Where this visual gets its state.
    pub fn state(&self) -> &HeatExchangerVisualState {
        &self.state
    }

    /// The scalar state the artwork is drawn from, or `None` on the
    /// physics-backed path.
    pub fn scalars(&self) -> Option<HeatExchangerScalars> {
        match self.state {
            HeatExchangerVisualState::Physics(_) => None,
            HeatExchangerVisualState::Scalars(s, _) => Some(s),
        }
    }

    /// The heat-transfer area, if known.
    pub fn heat_transfer_area(&self) -> Option<Area> {
        self.art.heat_transfer_area()
    }

    /// The overall heat-transfer coefficient `U`, if known.
    pub fn overall_coefficient(&self) -> Option<HeatTransfer> {
        self.art.overall_coefficient()
    }

    /// The caller's duty, if supplied on the scalar path.
    pub fn duty(&self) -> Option<Power> {
        self.art.duty()
    }

    /// The two terminal approaches, or `None` on the physics-backed path
    /// ([`HeatExchangerArt::approaches`]).
    pub fn approaches(&self) -> Option<(TemperatureInterval, TemperatureInterval)> {
        self.art.approaches()
    }

    /// Whether the supplied temperatures are reachable in this arrangement,
    /// or `None` on the physics-backed path ([`HeatExchangerArt::verdict`]).
    pub fn verdict(&self) -> Option<ApproachVerdict> {
        self.art.verdict()
    }

    /// The drawing this widget paints.
    pub fn art(&self) -> &HeatExchangerArt {
        &self.art
    }
}

impl Widget for HeatExchangerVisual {
    /// Draws the [`HeatExchangerArt`].
    fn ui(self, ui: &mut Ui) -> Response {
        self.art.ui(ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use petir::mathf::RealMath;
    use outram_park_fork_dwsim_libs::heat_exchanger::lmtd::lmtd;
    use uom::si::area::square_meter;
    use uom::si::f64::ThermodynamicTemperature;
    use uom::si::heat_transfer::watt_per_square_meter_kelvin;
    use uom::si::power::kilowatt as kilowatt_unit;
    use uom::si::temperature_interval::kelvin as kelvin_interval;
    use uom::si::thermodynamic_temperature::degree_celsius;

    fn degc(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_celsius>(v)
    }

    fn range() -> HeatExchangerDisplayRange {
        HeatExchangerDisplayRange {
            min_temp: degc(20.0),
            max_temp: degc(220.0),
        }
    }

    /// Illustrative state for a liquid-liquid recuperator: hot stream cooling
    /// 180 -> 95 degC against a cold stream heating 40 -> 110 degC, 1 200 kW.
    /// Demonstration values only.
    fn scalars() -> HeatExchangerScalars {
        HeatExchangerScalars {
            hot_inlet_temp: degc(180.0),
            hot_outlet_temp: degc(95.0),
            cold_inlet_temp: degc(40.0),
            cold_outlet_temp: degc(110.0),
            duty: Some(Power::new::<kilowatt_unit>(1200.0)),
        }
    }

    /// The physics-backed path draws the arrangement, area and `U` the
    /// component really holds, and no fluid state.
    ///
    /// **Methodology.** Wrap a counter-current 240 m² / 850 W/(m²·K)
    /// `HeatExchanger` and a co-current one with [`HeatExchangerVisual::new`],
    /// and require: no scalars, approaches, verdict or duty; the drawn kind to
    /// follow the component's own arrangement for both; the area and
    /// coefficient to survive; a drawing. The neutral-colour half is bugis's
    /// `the_unpainted_path_draws_its_arrangement_but_paints_no_temperature`.
    ///
    /// **Result (2026-08-12, re-checked after the 2026-11-05 split):**
    /// `CounterFlow` for `CounterCurrent`, `ParallelFlow` for `CoCurrent`;
    /// 240 m² and 850 W/(m²·K) survived; every fluid-state accessor `None`.
    #[test]
    fn the_physics_path_draws_its_arrangement_but_paints_no_temperature() {
        let area = Area::new::<square_meter>(240.0);
        let u = HeatTransfer::new::<watt_per_square_meter_kelvin>(850.0);
        let visual = HeatExchangerVisual::new(
            HeatExchanger::new(FlowArrangement::CounterCurrent, area, u),
            Pos2::new(200.0, 200.0),
            Vec2::new(360.0, 200.0),
        );
        assert!(visual.scalars().is_none());
        assert!(visual.art().scalars().is_none());
        assert!(visual.approaches().is_none());
        assert!(visual.verdict().is_none());
        assert!(visual.duty().is_none());
        assert!(matches!(visual.state(), HeatExchangerVisualState::Physics(_)));
        assert_eq!(visual.kind(), HeatExchangerKind::CounterFlow);
        assert_eq!(visual.construction(), HeatExchangerConstruction::ShellAndTube);
        assert_eq!(visual.heat_transfer_area(), Some(area));
        assert_eq!(visual.overall_coefficient(), Some(u));
        assert_eq!(visual.size(), Vec2::new(360.0, 200.0));
        assert!(bugis::components::shapes_drawn_by(visual) > 0);

        let co_current = HeatExchangerVisual::new(
            HeatExchanger::new(FlowArrangement::CoCurrent, area, u),
            Pos2::ZERO,
            Vec2::new(360.0, 200.0),
        );
        assert_eq!(co_current.kind(), HeatExchangerKind::ParallelFlow);
    }

    /// The scalar path and the builders reach the art.
    #[test]
    fn the_scalar_path_and_builders_reach_the_art() {
        let area = Area::new::<square_meter>(240.0);
        let u = HeatTransfer::new::<watt_per_square_meter_kelvin>(850.0);
        let v = HeatExchangerVisual::from_scalars(
            HeatExchangerKind::ParallelFlow,
            Pos2::new(200.0, 200.0),
            Vec2::new(360.0, 200.0),
            range(),
            scalars(),
        )
        .with_kind(HeatExchangerKind::CounterFlow)
        .with_construction(HeatExchangerConstruction::ALL[HeatExchangerConstruction::ALL.len() - 1])
        .with_surface(area, u)
        .without_labels();
        assert_eq!(v.scalars(), Some(scalars()));
        assert_eq!(v.kind(), HeatExchangerKind::CounterFlow);
        assert_eq!(
            v.construction(),
            HeatExchangerConstruction::ALL[HeatExchangerConstruction::ALL.len() - 1]
        );
        assert_eq!(v.duty(), scalars().duty);
        assert_eq!(v.approaches(), v.art().approaches());
        assert_eq!(v.verdict(), v.art().verdict());
        assert!(bugis::components::shapes_drawn_by(v) > 0);
    }

    /// The end brackets must be labelled with the same two temperature
    /// differences the log-mean is formed from, or the drawing and the rating
    /// would be describing different machines.
    ///
    /// **Methodology.** [`terminal_approaches`] is the widget's own convention;
    /// [`outram_park_fork_dwsim_libs::heat_exchanger::lmtd::lmtd`] is the
    /// workspace's ported DWSIM rating formula. Take the illustrative operating
    /// point (hot 180 -> 95 degC, cold 40 -> 110 degC), form `(dt1, dt2)` from
    /// the widget's brackets for each arrangement, and require
    /// `(dt1 - dt2)/ln(dt1/dt2)` to reproduce `lmtd(...)` for that arrangement
    /// to within 1e-6 K. Then require the counter-flow brackets to be the
    /// specific pairing `(T_hot_in - T_cold_out, T_hot_out - T_cold_in)` and the
    /// parallel-flow brackets `(T_hot_in - T_cold_in, T_hot_out - T_cold_out)`.
    ///
    /// **Result (2026-08-12):** counter-flow brackets 70.0000 K (left) and
    /// 55.0000 K (right), reconstructing an LMTD of 62.1988 K against
    /// `lmtd()`'s 62.1988 K, agreeing to better than 1e-6 K; parallel-flow
    /// brackets 140.0000 K and -15.0000 K, whose ratio is negative — so no real
    /// log-mean exists, which is itself the correct answer for an operating
    /// point parallel flow cannot reach, and the pairing was checked term by
    /// term instead. Interpretation: the bracket labels are the log-mean's own
    /// two ends, so a reader can take the numbers off the picture and reproduce
    /// the rating.
    #[test]
    fn the_end_brackets_are_the_two_ends_of_the_log_mean() {
        let s = scalars();
        for kind in HeatExchangerKind::ALL {
            let (left, right) = terminal_approaches(
                *kind,
                s.hot_inlet_temp,
                s.hot_outlet_temp,
                s.cold_inlet_temp,
                s.cold_outlet_temp,
            );
            let (dt1, dt2) = (
                left.get::<kelvin_interval>(),
                right.get::<kelvin_interval>(),
            );
            println!("{kind:?}: dt1 = {dt1:.4} K, dt2 = {dt2:.4} K");
            if dt1 > 0.0 && dt2 > 0.0 && (dt1 - dt2).abs() > 1e-9 {
                let ours = (dt1 - dt2) / (dt1 / dt2).r_ln();
                let theirs = lmtd(
                    kind.arrangement(),
                    s.hot_inlet_temp,
                    s.hot_outlet_temp,
                    s.cold_inlet_temp,
                    s.cold_outlet_temp,
                )
                .get::<kelvin_interval>();
                println!("  reconstructed LMTD {ours:.4} K vs lmtd() {theirs:.4} K");
                assert!(
                    (ours - theirs).abs() < 1e-6,
                    "{kind:?} brackets do not reproduce the log-mean"
                );
            }
        }

        // Term-by-term pairing, which holds whether or not the log-mean is real.
        let (cl, cr) = terminal_approaches(
            HeatExchangerKind::CounterFlow,
            s.hot_inlet_temp,
            s.hot_outlet_temp,
            s.cold_inlet_temp,
            s.cold_outlet_temp,
        );
        assert!((cl.get::<kelvin_interval>() - 70.0).abs() < 1e-9);
        assert!((cr.get::<kelvin_interval>() - 55.0).abs() < 1e-9);

        let (pl, pr) = terminal_approaches(
            HeatExchangerKind::ParallelFlow,
            s.hot_inlet_temp,
            s.hot_outlet_temp,
            s.cold_inlet_temp,
            s.cold_outlet_temp,
        );
        assert!((pl.get::<kelvin_interval>() - 140.0).abs() < 1e-9);
        assert!((pr.get::<kelvin_interval>() + 15.0).abs() < 1e-9);
    }

    /// The arrangement must survive a round trip through the physics enum, so
    /// the studio's selection and a rating call cannot disagree.
    #[test]
    fn the_arrangement_round_trips_through_the_physics_enum() {
        for kind in HeatExchangerKind::ALL {
            assert_eq!(
                HeatExchangerKind::from_arrangement(kind.arrangement()),
                *kind
            );
        }
        assert_eq!(
            HeatExchangerKind::CounterFlow.arrangement(),
            FlowArrangement::CounterCurrent
        );
        assert_eq!(
            HeatExchangerKind::ParallelFlow.arrangement(),
            FlowArrangement::CoCurrent
        );
    }
}
