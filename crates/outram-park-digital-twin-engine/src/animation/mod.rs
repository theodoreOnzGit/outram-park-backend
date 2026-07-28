//! Tracer / travel-time animation.
//!
//! A "tracer" is a small visual marker that travels along a component's flow
//! path, giving an at-a-glance sense of flow direction and speed -- e.g. dots
//! drifting along a pipe, faster when mass flow is higher, running backwards
//! when flow reverses.
//!
//! ## Design intent
//!
//! [`FlowTracer`] is the trait a visual component implements to support this:
//! it exposes the mass flow driving the tracer's direction/speed and owns the
//! tracer's current position along the flow path, advanced once per animation
//! frame. [`TravelTime`] is a separate, smaller trait for components whose
//! end-to-end residence time matters for animation timing (a long pipe should
//! take visibly longer for a tracer to cross than a short one, at the same
//! flow velocity) -- kept separate from [`FlowTracer`] since not every
//! tracer-bearing component necessarily needs to expose a travel time (a
//! tank's "residence time" is a different calculation than a pipe's).
//!
//! [`TracerTrain`] is the concrete, reusable implementation of that motion:
//! evenly-spaced tracer marks sharing one phase, advanced by
//! [`TracerTrain::advance`]. [`residence_time_from_flow`] computes the
//! residence time that drives it from a component's fluid inventory and mass
//! flow.
//!
//! ## Where tracer state lives
//!
//! Tracer motion is *stateful across frames*, but the visual components in
//! [`crate::components`] are `egui::Widget`s consumed by value and rebuilt
//! every frame from the current physics snapshot. So a [`TracerTrain`] is
//! owned by the **application** (typically alongside its physics state, or in
//! the [`crate::app_scaffold`]-driven app struct), advanced once per frame,
//! and copied into the widget at build time -- not owned by the widget, which
//! would reset its phase to zero on every repaint.
//!
//! ## What belongs here / what does not
//!
//! - **Belongs here:** the tracer/travel-time trait contracts, the
//!   animation-frame update logic, and the residence-time kinematics.
//! - **Does NOT belong here:** the underlying flow-rate/fluid-inventory
//!   *physics* -- that comes from [`tampines`] (via whichever
//!   [`crate::components`] wrapper a tracer-bearing visual component
//!   composes); this module only turns that physics into on-screen motion.
//!   Rendering does not belong here either -- this module is deliberately
//!   `egui`-free so it keeps building for Android (see the crate root's
//!   module gating); the drawing lives with each visual component.
//!
//! ## No trait objects
//!
//! Per the workspace's mandatory Rust design rules, these traits are a
//! compiler-enforced contract on each concrete visual component, not a
//! dispatch mechanism -- callers should match on a concrete component type
//! or an enum wrapping the tracer-bearing components, never
//! `&dyn FlowTracer`/`&dyn TravelTime`.

use uom::si::f64::{Mass, MassRate, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::time::second;

/// A visual tracer that moves along a component's flow path, its direction
/// and speed derived from mass flow.
///
/// Implementors own the tracer's current position; [`Self::advance`] is
/// called once per animation frame by the GUI's update loop (see
/// [`crate::app_scaffold`], which owns that loop). Most implementors will
/// delegate the motion itself to a [`TracerTrain`] field rather than
/// re-deriving the kinematics.
pub trait FlowTracer {
    /// Current mass flow rate driving this tracer's direction and speed.
    /// Positive is the component's defined forward direction; negative
    /// means the tracer should visibly run in reverse.
    fn mass_flow(&self) -> MassRate;

    /// Tracer position along the flow path, `[0, 1]` (`0` = inlet, `1` =
    /// outlet).
    fn tracer_position(&self) -> f64;

    /// Advance the tracer's position by one animation timestep `dt`,
    /// wrapping/resetting at the path's ends as the implementor sees fit
    /// (e.g. loop back to `0` after reaching `1`).
    fn advance(&mut self, dt: Time);
}

/// A component whose end-to-end residence time should influence tracer
/// animation timing (a tracer should visibly take longer to cross a
/// component with a longer travel time, at the same flow velocity).
pub trait TravelTime {
    /// Residence time for the current flow state -- how long a fluid parcel
    /// takes to traverse this component end-to-end.
    fn residence_time(&self) -> Time;
}

/// Mean residence time of a fluid parcel in a component holding
/// `fluid_inventory` kilograms of fluid and passing `mass_flow`.
///
/// This is the standard well-mixed/plug-flow residence-time identity
///
/// ```text
///     tau = m / m_dot
/// ```
///
/// where `m` is the fluid mass currently held in the component and `m_dot`
/// the mass flow through it. For a constant-area pipe this is equivalent to
/// `L / v` (length over bulk velocity), since `m = rho * A * L` and
/// `m_dot = rho * A * v`.
///
/// Only the **magnitude** of `mass_flow` is used -- a reversed flow takes
/// just as long to traverse the component as a forward one; direction is
/// [`TracerTrain::advance`]'s concern, not the travel time's.
///
/// ## Valid ranges and degenerate cases
///
/// `fluid_inventory` is expected non-negative and `mass_flow` non-zero. At
/// exactly zero flow the residence time is unbounded, which has no useful
/// finite representation; this returns [`infinite_residence_time`] there, and
/// [`TracerTrain::advance`] reads that as "no motion" (a stagnant component's
/// tracers should sit still, which is the physically honest display). A
/// negative `fluid_inventory` is not meaningful and likewise yields
/// [`infinite_residence_time`] rather than a negative travel time.
pub fn residence_time_from_flow(fluid_inventory: Mass, mass_flow: MassRate) -> Time {
    let m_dot = mass_flow.get::<kilogram_per_second>().abs();
    if !m_dot.is_finite() || m_dot == 0.0 {
        return infinite_residence_time();
    }
    let tau = fluid_inventory / MassRate::new::<kilogram_per_second>(m_dot);
    if tau.get::<second>() < 0.0 {
        return infinite_residence_time();
    }
    tau
}

/// The unbounded residence time of a component with no flow through it --
/// an infinite [`Time`], i.e. "a parcel never traverses this component".
///
/// `uom` has no `Time::INFINITY` associated constant, so this names the
/// value once rather than spelling `Time::new::<second>(f64::INFINITY)` at
/// each site. [`TracerTrain::advance`] treats it (and any other non-finite
/// residence time) as [`FlowDirection::Stagnant`].
pub fn infinite_residence_time() -> Time {
    Time::new::<second>(f64::INFINITY)
}

/// Which way a [`TracerTrain`] is currently moving along its flow path.
///
/// Enum, not a signed scalar, so that a `match` on flow direction is
/// exhaustive at every call site (per the workspace's enum-dispatch rule).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowDirection {
    /// Flow runs inlet -> outlet; tracers advance toward position `1`.
    Forward,
    /// Flow is reversed; tracers advance back toward position `0`.
    Reverse,
    /// No flow (or no finite residence time) -- tracers hold position.
    Stagnant,
}

/// A train of evenly-spaced tracer marks sharing a single phase, moving along
/// one component's flow path.
///
/// Rather than storing each mark's position independently (which lets them
/// drift apart through floating-point accumulation), the train stores one
/// `phase` in `[0, 1)` and derives mark `i` of `count` as
/// `(phase + i/count) mod 1`. The marks therefore stay exactly evenly spaced
/// for the lifetime of the animation.
///
/// ## Timing
///
/// [`Self::advance`] moves the phase at `1 / residence_time` of the path per
/// second, so a mark takes exactly one residence time to traverse the
/// component -- the animation is a direct readout of the physical travel
/// time, not a free-running decorative loop. Doubling the mass flow halves
/// the residence time (see [`residence_time_from_flow`]) and so visibly
/// doubles the tracer speed.
///
/// `Copy`, so an application can hold the authoritative train in its own
/// state and hand a copy to a per-frame widget without ceremony.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TracerTrain {
    /// Phase of the leading mark along the path, in `[0, 1)`.
    phase: f64,
    /// Number of evenly-spaced marks; always >= 1.
    count: usize,
    /// Direction of travel as of the last [`Self::advance`] call.
    direction: FlowDirection,
}

impl TracerTrain {
    /// A train of `count` evenly-spaced marks, starting at phase `0` and
    /// stagnant until first advanced.
    ///
    /// `count` is clamped to at least `1` -- a zero-mark train would render
    /// nothing and silently hide a flow the user is meant to see.
    pub fn new(count: usize) -> Self {
        Self {
            phase: 0.0,
            count: count.max(1),
            direction: FlowDirection::Stagnant,
        }
    }

    /// Advance the train by one animation timestep `dt`, given the
    /// component's current `residence_time` (from
    /// [`residence_time_from_flow`], or a [`TravelTime`] implementor) and
    /// `mass_flow`.
    ///
    /// The sign of `mass_flow` sets [`FlowDirection`]; its magnitude enters
    /// only through `residence_time`. Motion is skipped entirely -- leaving
    /// the phase where it was and the direction [`FlowDirection::Stagnant`]
    /// -- when the flow is zero, or when `residence_time` is zero, negative,
    /// or non-finite (e.g. [`infinite_residence_time`] from a stagnant
    /// component).
    /// Those are exactly the cases with no well-defined tracer speed, and
    /// freezing is the honest display for them.
    pub fn advance(&mut self, dt: Time, residence_time: Time, mass_flow: MassRate) {
        let m_dot = mass_flow.get::<kilogram_per_second>();
        let tau_s = residence_time.get::<second>();
        let dt_s = dt.get::<second>();

        let direction = if !m_dot.is_finite() || m_dot == 0.0 {
            FlowDirection::Stagnant
        } else if m_dot > 0.0 {
            FlowDirection::Forward
        } else {
            FlowDirection::Reverse
        };

        if direction == FlowDirection::Stagnant || !tau_s.is_finite() || tau_s <= 0.0 {
            self.direction = FlowDirection::Stagnant;
            return;
        }

        let signed = match direction {
            FlowDirection::Forward => 1.0,
            FlowDirection::Reverse => -1.0,
            FlowDirection::Stagnant => 0.0,
        };
        let delta = signed * dt_s / tau_s;
        if !delta.is_finite() {
            self.direction = FlowDirection::Stagnant;
            return;
        }

        self.phase = (self.phase + delta).rem_euclid(1.0);
        self.direction = direction;
    }

    /// Phase of the leading mark, in `[0, 1)`.
    pub fn phase(&self) -> f64 {
        self.phase
    }

    /// Number of marks in the train (always >= 1).
    pub fn count(&self) -> usize {
        self.count
    }

    /// Direction of travel as of the last [`Self::advance`] call.
    pub fn direction(&self) -> FlowDirection {
        self.direction
    }

    /// Positions of every mark along the flow path, each in `[0, 1)`
    /// (`0` = inlet, `1` = outlet), evenly spaced by `1/count`.
    ///
    /// Yields exactly [`Self::count`] items, in mark order rather than sorted
    /// position order (the train wraps, so the leading mark is not always the
    /// nearest the inlet).
    pub fn positions(&self) -> impl Iterator<Item = f64> {
        let phase = self.phase;
        let count = self.count;
        (0..count).map(move |i| (phase + i as f64 / count as f64).rem_euclid(1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::mass::kilogram;

    fn kgs(v: f64) -> MassRate {
        MassRate::new::<kilogram_per_second>(v)
    }
    fn s(v: f64) -> Time {
        Time::new::<second>(v)
    }

    /// Methodology: the residence-time identity `tau = m / m_dot` is checked
    /// against a hand-computed case -- 10 kg of inventory passing 2 kg/s must
    /// take exactly 5 s to traverse. Pass criterion: exact to 1e-12 s.
    ///
    /// Results (2026-07-28): `tau = 5.000000000000 s`, error < 1e-12 s
    /// against the analytical 5 s. Reversed flow (-2 kg/s) gives the same
    /// 5 s, confirming magnitude-only use of the flow.
    #[test]
    fn residence_time_matches_analytical_identity() {
        let tau = residence_time_from_flow(Mass::new::<kilogram>(10.0), kgs(2.0));
        assert!((tau.get::<second>() - 5.0).abs() < 1e-12);

        let reversed = residence_time_from_flow(Mass::new::<kilogram>(10.0), kgs(-2.0));
        assert!((reversed.get::<second>() - 5.0).abs() < 1e-12);
    }

    #[test]
    fn residence_time_is_infinite_at_zero_flow_or_negative_inventory() {
        assert!(
            residence_time_from_flow(Mass::new::<kilogram>(10.0), kgs(0.0))
                .get::<second>()
                .is_infinite()
        );
        assert!(
            residence_time_from_flow(Mass::new::<kilogram>(-1.0), kgs(2.0))
                .get::<second>()
                .is_infinite()
        );
    }

    /// Methodology: a mark must take exactly one residence time to traverse
    /// the path. Advancing a 4 s-residence-time train by 4 s in one step (and
    /// again in 8 steps of 0.5 s) must return the phase to its start.
    ///
    /// Results (2026-07-28): both the single-step and the 8-substep
    /// integration returned phase 0 to within 1e-12, confirming the tracer
    /// speed is exactly `1/tau` of the path per second and that substepping
    /// does not accumulate drift.
    #[test]
    fn mark_crosses_path_in_exactly_one_residence_time() {
        let mut train = TracerTrain::new(1);
        train.advance(s(4.0), s(4.0), kgs(1.0));
        assert!(train.phase() < 1e-12 || (1.0 - train.phase()) < 1e-12);

        let mut substepped = TracerTrain::new(1);
        for _ in 0..8 {
            substepped.advance(s(0.5), s(4.0), kgs(1.0));
        }
        assert!(substepped.phase() < 1e-12 || (1.0 - substepped.phase()) < 1e-12);
    }

    #[test]
    fn doubling_flow_doubles_tracer_speed() {
        let inventory = Mass::new::<kilogram>(10.0);
        let mut slow = TracerTrain::new(1);
        let mut fast = TracerTrain::new(1);

        slow.advance(
            s(1.0),
            residence_time_from_flow(inventory, kgs(1.0)),
            kgs(1.0),
        );
        fast.advance(
            s(1.0),
            residence_time_from_flow(inventory, kgs(2.0)),
            kgs(2.0),
        );

        // tau_slow = 10 s -> phase 0.1; tau_fast = 5 s -> phase 0.2.
        assert!((slow.phase() - 0.1).abs() < 1e-12);
        assert!((fast.phase() - 0.2).abs() < 1e-12);
    }

    #[test]
    fn reverse_flow_runs_backwards_and_wraps() {
        let mut train = TracerTrain::new(1);
        train.advance(s(1.0), s(10.0), kgs(-1.0));
        assert_eq!(train.direction(), FlowDirection::Reverse);
        // Wrapped back around from 0.0 to 0.9.
        assert!((train.phase() - 0.9).abs() < 1e-12);
    }

    #[test]
    fn stagnant_cases_freeze_the_train() {
        // Zero flow.
        let mut zero_flow = TracerTrain::new(3);
        zero_flow.advance(s(1.0), s(10.0), kgs(0.0));
        assert_eq!(zero_flow.phase(), 0.0);
        assert_eq!(zero_flow.direction(), FlowDirection::Stagnant);

        // Infinite residence time (what zero flow yields via the helper).
        let mut infinite_tau = TracerTrain::new(3);
        infinite_tau.advance(s(1.0), infinite_residence_time(), kgs(1.0));
        assert_eq!(infinite_tau.phase(), 0.0);
        assert_eq!(infinite_tau.direction(), FlowDirection::Stagnant);

        // Zero residence time -- no well-defined speed.
        let mut zero_tau = TracerTrain::new(3);
        zero_tau.advance(s(1.0), s(0.0), kgs(1.0));
        assert_eq!(zero_tau.phase(), 0.0);
        assert_eq!(zero_tau.direction(), FlowDirection::Stagnant);
    }

    #[test]
    fn marks_stay_evenly_spaced_and_in_range() {
        let mut train = TracerTrain::new(4);
        train.advance(s(0.7), s(10.0), kgs(1.0));

        let positions: Vec<f64> = train.positions().collect();
        assert_eq!(positions.len(), 4);
        assert!(positions.iter().all(|p| (0.0..1.0).contains(p)));

        // Consecutive marks are 1/4 of the path apart (modulo the wrap).
        for i in 0..4 {
            let gap = (positions[(i + 1) % 4] - positions[i]).rem_euclid(1.0);
            assert!((gap - 0.25).abs() < 1e-12);
        }
    }

    #[test]
    fn count_is_clamped_to_at_least_one() {
        let train = TracerTrain::new(0);
        assert_eq!(train.count(), 1);
        assert_eq!(train.positions().count(), 1);
    }

    /// Sanity-checks that the [`FlowTracer`]/[`TravelTime`] trait shapes are
    /// implementable on top of [`TracerTrain`], which is how real visual
    /// components are expected to satisfy them.
    struct MockPipeTracer {
        mass_flow: MassRate,
        inventory: Mass,
        train: TracerTrain,
    }

    impl TravelTime for MockPipeTracer {
        fn residence_time(&self) -> Time {
            residence_time_from_flow(self.inventory, self.mass_flow)
        }
    }

    impl FlowTracer for MockPipeTracer {
        fn mass_flow(&self) -> MassRate {
            self.mass_flow
        }

        fn tracer_position(&self) -> f64 {
            self.train.phase()
        }

        fn advance(&mut self, dt: Time) {
            let tau = self.residence_time();
            self.train.advance(dt, tau, self.mass_flow);
        }
    }

    #[test]
    fn traits_compose_over_a_tracer_train() {
        let mut tracer = MockPipeTracer {
            mass_flow: kgs(2.0),
            inventory: Mass::new::<kilogram>(10.0),
            train: TracerTrain::new(2),
        };
        assert!((tracer.residence_time().get::<second>() - 5.0).abs() < 1e-12);

        tracer.advance(s(1.0));
        // 1 s of a 5 s traverse -> one fifth of the path.
        assert!((tracer.tracer_position() - 0.2).abs() < 1e-12);
        assert_eq!(tracer.mass_flow(), kgs(2.0));
    }
}
