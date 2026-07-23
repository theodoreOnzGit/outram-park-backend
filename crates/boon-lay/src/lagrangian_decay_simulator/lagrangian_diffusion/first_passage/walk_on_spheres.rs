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
use outram_mc_libs::rng::lcg::prn;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::{Area, DiffusionCoefficient, Length, Time};
use uom::ConstZero;

use crate::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::oorandom_rng::OoRng64;
use crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::sphere_fpt::{
    sample_first_passage_time, sample_uniform_direction,
};
use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::{
    TrisoCell, TrisoRegion,
};

/// Default diffusion coefficient used when a position falls outside every known
/// region (treated as a cracked/open layer). Matches the fallback the existing
/// Gaussian-step code uses.
const FALLBACK_DIFFUSION_M2_PER_S: f64 = 1e-6;

/// Outcome of a single Walk-on-Spheres hop within a [`TrisoCell`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HopOutcome {
    /// The walker advanced by one interface-free sphere and is still strictly
    /// inside the particle.
    Stepped,
    /// The walker reached the OPyC outer surface and is released from the
    /// particle.
    Released,
    /// The walker reached an interior layer interface. Resolving it (transmit
    /// vs. reflect via the interface rule) is handled by the multilayer phase;
    /// until then this outcome simply reports arrival.
    ReachedInterface,
}

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

    /// Perform one Walk-on-Spheres hop inside the walker's current [`TrisoCell`]
    /// shell.
    ///
    /// Sizes the hop with [`nearest_interface_distance`] so the sphere touches
    /// but never crosses the nearest interface, jumps to a uniform point on that
    /// sphere, and adds the sampled first-passage time to the walker's clock.
    ///
    /// `capture_eps` is the distance below which the walker is considered to
    /// have *reached* an interface rather than to still be hopping — it bounds
    /// the geometric approach to a boundary. Reaching the OPyC outer surface
    /// yields [`HopOutcome::Released`]; reaching an interior interface yields
    /// [`HopOutcome::ReachedInterface`] (resolved by the multilayer phase).
    ///
    /// Because the hop can never overshoot a thin layer, this is the step that
    /// removes the buffer-teleport failure of the single-Gaussian scheme (see
    /// `docs/buffer_clt_failure_analysis.md`).
    pub fn hop(&mut self, triso_cell: &TrisoCell, capture_eps: Length) -> HopOutcome {
        let Some(hop_radius) = nearest_interface_distance(triso_cell, self.position) else {
            // No containing shell: the walker is already outside the particle.
            return HopOutcome::Released;
        };

        if hop_radius <= capture_eps {
            // Within capture distance of the nearest interface. If that is the
            // outer particle surface, the walker is released; otherwise it has
            // arrived at an interior interface.
            if self.radius() + capture_eps >= triso_cell.get_opyc_radius() {
                return HopOutcome::Released;
            }
            return HopOutcome::ReachedInterface;
        }

        let diffusion_coefficient = triso_cell
            .try_get_diffusion_coefficient(self.position, self.nuclide)
            .unwrap_or_else(|| {
                DiffusionCoefficient::new::<square_meter_per_second>(FALLBACK_DIFFUSION_M2_PER_S)
            });

        let tau = sample_first_passage_time(&mut self.rng.0, hop_radius, diffusion_coefficient);
        self.time += tau;

        let dir = sample_uniform_direction(&mut self.rng.0);
        self.position = [
            self.position[0] + hop_radius * dir[0],
            self.position[1] + hop_radius * dir[1],
            self.position[2] + hop_radius * dir[2],
        ];

        HopOutcome::Stepped
    }

    /// Walk to the surface of a single homogeneous absorbing sphere and return
    /// the total first-passage (release) time.
    ///
    /// The walker hops within a sphere of radius `sphere_radius` filled with a
    /// medium of constant diffusion coefficient `diffusion_coefficient`, its
    /// surface a perfect sink. Hops shrink as the walker nears the surface; the
    /// walk stops once it is within `capture_eps` of the surface, at which point
    /// the accumulated `self.time` is the release time.
    ///
    /// This is the single-region case used to verify the engine against Crank's
    /// analytical sphere-release solution, and the cleanest demonstration that
    /// the method is timestep-free: with a buffer-like `D` it reproduces the
    /// release curve that the single-Gaussian step could only match with a
    /// vanishingly small `dt`.
    ///
    /// # Units
    ///
    /// `sphere_radius` and `capture_eps` are `Length`s; `diffusion_coefficient`
    /// is a `DiffusionCoefficient` (m^2/s); the returned release time is a
    /// `Time` (seconds).
    pub fn walk_to_absorbing_sphere(
        &mut self,
        sphere_radius: Length,
        diffusion_coefficient: DiffusionCoefficient,
        capture_eps: Length,
    ) -> Time {
        loop {
            let hop_radius = sphere_radius - self.radius();
            if hop_radius <= capture_eps {
                return self.time;
            }
            let tau = sample_first_passage_time(&mut self.rng.0, hop_radius, diffusion_coefficient);
            self.time += tau;
            let dir = sample_uniform_direction(&mut self.rng.0);
            self.position = [
                self.position[0] + hop_radius * dir[0],
                self.position[1] + hop_radius * dir[1],
                self.position[2] + hop_radius * dir[2],
            ];
        }
    }
}

/// Sample a point uniformly in the volume of a ball of radius `radius`, centred
/// on the origin, returned as a `[uom]` `Length` triple.
///
/// Uses `r = radius * U^(1/3)` for the radial coordinate (so the point is
/// volume-uniform, not radius-uniform) and an isotropic direction. This is the
/// birth distribution of a fission product created uniformly in a spherical
/// fuel kernel, and the initial condition for the Crank release comparison.
#[inline]
pub fn sample_uniform_in_ball(seed: &mut u64, radius: Length) -> [Length; 3] {
    let r = radius * prn(seed).cbrt();
    let dir = sample_uniform_direction(seed);
    [r * dir[0], r * dir[1], r * dir[2]]
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
    use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::release_fraction_analytical_solution::calculate_analytical_fraction_released;
    use approx::assert_relative_eq;
    use uom::si::diffusion_coefficient::square_meter_per_second;
    use uom::si::length::micrometer;
    use uom::si::time::second;

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

    /// The headline single-region verification: a Walk-on-Spheres ensemble in a
    /// homogeneous absorbing sphere reproduces Crank's analytical release curve
    /// for a uniform initial concentration — with **no timestep** and a
    /// buffer-like diffusion coefficient (the regime where the single-Gaussian
    /// step overshoots). This is a fast self-check; the full V&V record with
    /// tabulated numbers is written up separately.
    #[test]
    fn absorbing_sphere_release_matches_crank() {
        let radius = Length::new::<micrometer>(100.0);
        let diffusion = DiffusionCoefficient::new::<square_meter_per_second>(1e-8); // buffer-like
        let capture_eps = radius * 1e-3;
        let n = 4000usize;
        let mut master = OoRng64::from_u64(0x51ce_1234_5678_9abc);

        let mut release_times_s = Vec::with_capacity(n);
        for _ in 0..n {
            let start = sample_uniform_in_ball(&mut master.0, radius);
            let child = OoRng64::from_u64(master.next_u64());
            let mut walker = WoSWalker::new(start, Nuclide::Cs137, child);
            let t = walker.walk_to_absorbing_sphere(radius, diffusion, capture_eps);
            release_times_s.push(t.get::<second>());
        }

        for &t_s in &[0.05_f64, 0.1, 0.2, 0.4] {
            let mc = release_times_s.iter().filter(|&&t| t <= t_s).count() as f64 / n as f64;
            let crank = calculate_analytical_fraction_released(
                diffusion,
                radius,
                Time::new::<second>(t_s),
                200,
            );
            assert!(
                (mc - crank).abs() < 0.04,
                "t={t_s}s: MC release {mc:.3} vs Crank {crank:.3}"
            );
        }
    }

    /// A walker started just inside the OPyC outer surface is released on its
    /// next hop resolution.
    #[test]
    fn hop_releases_at_outer_surface() {
        let cell = TrisoCell::new_crp6_geometry();
        let capture_eps = Length::new::<micrometer>(0.5);
        // OPyC outer radius is 427.5 um; start 0.1 um inside it.
        let start = at(427.4);
        let mut walker = WoSWalker::new(start, Nuclide::Cs137, OoRng64::from_u64(7));
        assert_eq!(walker.hop(&cell, capture_eps), HopOutcome::Released);
    }
}
