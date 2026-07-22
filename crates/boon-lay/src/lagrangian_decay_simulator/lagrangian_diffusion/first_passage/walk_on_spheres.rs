//! The Walk-on-Spheres walker and the geometry that sizes each hop.
//!
//! A [`WoSWalker`] is a single diffusing atom: a position, the nuclide it
//! currently is, its accumulated simulated time, and its own random-number
//! stream. The engine advances it by repeatedly (a) finding the largest sphere
//! centred on the walker that contains no layer interface, then (b) jumping to
//! a uniform point on that sphere while adding the corresponding first-passage
//! time (see [`super::sphere_fpt`]). Because the sphere touches — but never
//! crosses — the nearest interface, an atom can never teleport across a thin
//! layer the way the single-Gaussian step does (see
//! `docs/buffer_clt_failure_analysis.md`).
//!
//! This Phase-0 scaffold defines the walker type and the geometry helper
//! [`nearest_interface_distance`], which turns the concentric-sphere `TrisoCell`
//! into the hop radius `R`. The stochastic `hop` itself, the outer-surface
//! escape test, and the interface handling are added in the CPU-engine phases.

use fission_yields_data::prelude::Nuclide;
use uom::si::f64::{Area, Length, Time};
use uom::ConstZero;

use crate::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::oorandom_rng::OoRng64;
use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::{
    TrisoCell, TrisoRegion,
};

/// A single diffusing atom tracked by the Walk-on-Spheres engine.
///
/// # Fields
///
/// - `position` — Cartesian position `[x, y, z]` as [`uom`] `Length`s, measured
///   from the TRISO particle centre.
/// - `nuclide` — the atom's current identity; changes when it decays or
///   transmutes (handled in the depletion phase).
/// - `time` — accumulated simulated time (a [`uom`] `Time`) since the walk
///   began; each hop adds its first-passage time to this.
/// - `rng` — the walker's private RNG stream ([`OoRng64`], the workspace LCG).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WoSWalker {
    /// Cartesian position from the particle centre, in `uom` `Length`.
    pub position: [Length; 3],
    /// Current nuclide identity.
    pub nuclide: Nuclide,
    /// Accumulated simulated time since the walk began.
    pub time: Time,
    /// Private random-number stream for this walker.
    pub rng: OoRng64,
}

impl WoSWalker {
    /// Create a walker at an explicit position with a fresh time of zero.
    #[inline]
    pub fn new(position: [Length; 3], nuclide: Nuclide, rng: OoRng64) -> Self {
        Self {
            position,
            nuclide,
            time: Time::ZERO,
            rng,
        }
    }

    /// Create a walker at the particle centre (the origin) with time zero.
    ///
    /// This is the usual birth site for a fission product created uniformly in
    /// the kernel; a uniform-in-kernel sampler is added with the engine.
    #[inline]
    pub fn new_at_center(nuclide: Nuclide, rng: OoRng64) -> Self {
        Self::new([Length::ZERO, Length::ZERO, Length::ZERO], nuclide, rng)
    }

    /// Radial distance of the walker from the particle centre, `|position|`.
    #[inline]
    pub fn radius(&self) -> Length {
        radial_distance(self.position)
    }
}

/// Radial distance of a point from the particle centre, `sqrt(x^2+y^2+z^2)`.
#[inline]
pub fn radial_distance(position: [Length; 3]) -> Length {
    let r_squared: Area =
        position[0] * position[0] + position[1] * position[1] + position[2] * position[2];
    r_squared.sqrt()
}

/// Distance from `position` to the nearest layer interface of `triso_cell`.
///
/// This is the radius `R` of the largest interface-free sphere the walker may
/// hop across. For the fuel kernel the only bounding surface is the kernel
/// outer sphere, so `R` is the distance out to it. For any coating shell the
/// walker is bounded on both sides, and `R` is the smaller of the distance in
/// to the inner sphere and out to the outer sphere.
///
/// Returns `None` if the walker is already outside the particle (it has been
/// released and there is no containing shell).
///
/// # Units
///
/// `position` components and the returned distance are [`uom`] `Length`s.
#[inline]
pub fn nearest_interface_distance(triso_cell: &TrisoCell, position: [Length; 3]) -> Option<Length> {
    let rho = radial_distance(position);

    // (inner bounding radius, outer bounding radius) of the walker's shell.
    let (inner, outer): (Option<Length>, Option<Length>) = match triso_cell.get_triso_region(position)
    {
        TrisoRegion::Fuel => (None, Some(triso_cell.get_fuel_radius())),
        TrisoRegion::Buffer => (
            Some(triso_cell.get_fuel_radius()),
            Some(triso_cell.get_buffer_radius()),
        ),
        TrisoRegion::IPyC => (
            Some(triso_cell.get_buffer_radius()),
            Some(triso_cell.get_ipyc_radius()),
        ),
        TrisoRegion::SiC => (
            Some(triso_cell.get_ipyc_radius()),
            Some(triso_cell.get_sic_radius()),
        ),
        TrisoRegion::OPyC => (
            Some(triso_cell.get_sic_radius()),
            Some(triso_cell.get_opyc_radius()),
        ),
        TrisoRegion::Outside => return None,
    };

    let dist_out = outer.map(|r_out| r_out - rho);
    let dist_in = inner.map(|r_in| rho - r_in);

    match (dist_in, dist_out) {
        (Some(a), Some(b)) => Some(if a < b { a } else { b }),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use uom::si::length::micrometer;

    fn at(x_um: f64) -> [Length; 3] {
        [
            Length::new::<micrometer>(x_um),
            Length::ZERO,
            Length::ZERO,
        ]
    }

    #[test]
    fn fuel_center_hop_reaches_kernel_surface() {
        let cell = TrisoCell::new_crp6_geometry();
        // At the centre, the nearest interface is the kernel outer surface at
        // r_fuel = 212.5 um.
        let r = nearest_interface_distance(&cell, at(0.0)).unwrap();
        assert_relative_eq!(r.get::<micrometer>(), 212.5, max_relative = 1e-9);
    }

    #[test]
    fn buffer_midpoint_bounded_on_both_sides() {
        let cell = TrisoCell::new_crp6_geometry();
        // Buffer spans 212.5 -> 312.5 um; at 262.5 um both distances are 50 um.
        let r = nearest_interface_distance(&cell, at(262.5)).unwrap();
        assert_relative_eq!(r.get::<micrometer>(), 50.0, max_relative = 1e-6);
    }

    #[test]
    fn outside_particle_has_no_shell() {
        let cell = TrisoCell::new_crp6_geometry();
        assert!(nearest_interface_distance(&cell, at(500.0)).is_none());
    }
}
