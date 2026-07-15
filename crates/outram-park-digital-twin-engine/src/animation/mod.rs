//! Tracer / travel-time animation.
//!
//! **Status: scaffold only.** Trait definitions and design notes, no
//! implementation. Neither existing GUI example in this workspace
//! (`fhr_sim_v1`/`fhr_sim_v2`) animates flow at all -- both only do static
//! colour mapping (see [`crate::color_maps`]) -- so this is genuinely new
//! design work, not a port.
//!
//! ## Design intent
//!
//! A "tracer" is a small visual marker that travels along a component's
//! flow path, giving an at-a-glance sense of flow direction and speed --
//! e.g. dots drifting along a pipe, faster when mass flow is higher, running
//! backwards when flow reverses. [`FlowTracer`] is the trait a visual
//! component implements to support this: it exposes the mass flow driving
//! the tracer's direction/speed and owns the tracer's current position
//! along the flow path, advanced once per animation frame.
//!
//! [`TravelTime`] is a separate, smaller trait for components whose
//! end-to-end residence time matters for animation timing (e.g. a long pipe
//! should take visibly longer for a tracer to cross than a short one, at
//! the same flow velocity) -- kept separate from [`FlowTracer`] since not
//! every tracer-bearing component necessarily needs to expose a travel time
//! (a tank's "residence time" is a different calculation than a pipe's).
//!
//! ## What belongs here / what does not
//!
//! - **Belongs here:** the tracer/travel-time trait contracts, and (once
//!   implemented) the animation-frame update logic and rendering.
//! - **Does NOT belong here:** the underlying flow-rate/residence-time
//!   *physics* -- that comes from [`tampines`] (via whichever
//!   [`crate::components`] wrapper a tracer-bearing visual component
//!   composes); this module only turns that physics into on-screen motion.
//!
//! ## No trait objects
//!
//! Per the workspace's mandatory Rust design rules, these traits are a
//! compiler-enforced contract on each concrete visual component, not a
//! dispatch mechanism -- callers should match on a concrete component type
//! or an enum wrapping the (currently nonexistent) tracer-bearing
//! components, never `&dyn FlowTracer`/`&dyn TravelTime`.

use uom::si::f64::{MassRate, Time};

/// A visual tracer that moves along a component's flow path, its direction
/// and speed derived from mass flow.
///
/// Implementors own the tracer's current position; [`Self::advance`] is
/// called once per animation frame by the GUI's update loop (the
/// `app_scaffold` module that will own that loop does not exist yet --
/// see the workspace's `op-wqk.5` bead).
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

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::time::second;

    /// Sanity-checks that the trait shapes above are actually implementable
    /// and usable, since no real component implements them yet.
    struct MockPipeTracer {
        mass_flow: MassRate,
        position: f64,
        length: Time,
    }

    impl FlowTracer for MockPipeTracer {
        fn mass_flow(&self) -> MassRate {
            self.mass_flow
        }

        fn tracer_position(&self) -> f64 {
            self.position
        }

        fn advance(&mut self, dt: Time) {
            let speed = 1.0 / self.length.get::<second>();
            self.position = (self.position + speed * dt.get::<second>()).rem_euclid(1.0);
        }
    }

    impl TravelTime for MockPipeTracer {
        fn residence_time(&self) -> Time {
            self.length
        }
    }

    #[test]
    fn mock_tracer_advances_and_wraps() {
        let mut tracer = MockPipeTracer {
            mass_flow: MassRate::new::<kilogram_per_second>(1.0),
            position: 0.9,
            length: Time::new::<second>(1.0),
        };
        tracer.advance(Time::new::<second>(0.2));
        assert!((tracer.tracer_position() - 0.1).abs() < 1e-9);
        assert_eq!(tracer.residence_time(), Time::new::<second>(1.0));
    }
}
