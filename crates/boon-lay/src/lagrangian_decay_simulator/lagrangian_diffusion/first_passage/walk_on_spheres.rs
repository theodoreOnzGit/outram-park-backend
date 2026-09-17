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
use crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::interface::does_transmit;
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
    /// The walker reached an interior layer interface and it was resolved this
    /// step (it transmitted into the neighbour or reflected back). The walk
    /// continues from the reinserted position on the next step.
    ReachedInterface,
}

/// Tunables for a multilayer Walk-on-Spheres walk.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WalkParams {
    /// Distance below which a hop is treated as having *reached* the nearest
    /// interface rather than continuing to shrink. Bounds the geometric
    /// approach to a boundary; smaller values are more accurate but take more
    /// hops. Choose it small relative to the thinnest layer.
    pub capture_eps: Length,
    /// After an interface is resolved, the walker is reinserted this many
    /// `capture_eps` away from the interface (on the chosen side) so the next
    /// hop is a genuine hop and not an immediate re-trigger. Must be `> 1`.
    pub reinsert_factor: f64,
    /// Partition/solubility ratio `K` at every interface (`1.0` = plain
    /// concentration continuity, the default TRISO assumption).
    pub partition_k: f64,
    /// Safety cap on the number of steps before a walk is abandoned (returns
    /// `None`), guarding against a walker that never escapes within budget.
    pub max_steps: u64,
}

impl WalkParams {
    /// Sensible defaults for a CRP-6-scale particle: `capture_eps = 10 nm`
    /// (~1/3500 of the thinnest, 35 µm, SiC layer), reinsert at `2 * eps`, unit
    /// partition, and a generous step cap.
    pub fn crp6_default() -> Self {
        Self {
            capture_eps: Length::new::<uom::si::length::nanometer>(10.0),
            reinsert_factor: 2.0,
            partition_k: 1.0,
            max_steps: 50_000_000,
        }
    }
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

    /// Advance one multilayer step: either a genuine Walk-on-Spheres hop within
    /// the current shell, or — if the walker has reached an interface — one
    /// transmission/reflection resolution.
    ///
    /// The hop is sized by [`shell_bounds`] so it never crosses an interface.
    /// When the walker comes within `params.capture_eps` of an interface, it is
    /// resolved with the interface rule ([`does_transmit`]): reaching the OPyC
    /// outer surface releases the walker; an interior interface transmits into
    /// the neighbour or reflects, and the walker is reinserted
    /// `reinsert_factor * capture_eps` from the interface on the chosen side.
    pub fn step_multilayer(&mut self, triso_cell: &TrisoCell, params: &WalkParams) -> HopOutcome {
        let Some((inner, outer)) = shell_bounds(triso_cell, self.position) else {
            return HopOutcome::Released;
        };
        let rho = self.radius();
        let dist_out = outer - rho;
        let dist_in = inner.map(|r_in| rho - r_in);

        let hop_radius = match dist_in {
            Some(di) if di < dist_out => di,
            _ => dist_out,
        };

        let d_current = self.diffusion_at(triso_cell, self.position);

        if hop_radius > params.capture_eps {
            // Genuine Walk-on-Spheres hop, entirely within this shell.
            let tau = sample_first_passage_time(&mut self.rng.0, hop_radius, d_current);
            self.time += tau;
            let dir = sample_uniform_direction(&mut self.rng.0);
            self.position = [
                self.position[0] + hop_radius * dir[0],
                self.position[1] + hop_radius * dir[1],
                self.position[2] + hop_radius * dir[2],
            ];
            return HopOutcome::Stepped;
        }

        // The walker has reached the nearest interface — decide which one.
        let at_outer = match dist_in {
            Some(di) => dist_out <= di,
            None => true,
        };

        let reinsert = params.capture_eps * params.reinsert_factor;

        if at_outer {
            // The outer particle surface is an absorbing sink: reaching it is
            // release. Interior outer boundaries transmit/reflect.
            if outer == triso_cell.get_opyc_radius() {
                return HopOutcome::Released;
            }
            let d_next = self.diffusion_at(triso_cell, self.point_at_radius(outer + reinsert));
            if does_transmit(&mut self.rng.0, d_current, d_next, params.partition_k) {
                self.set_radius_to(outer + reinsert);
            } else {
                self.set_radius_to(outer - reinsert);
            }
        } else {
            let inner_radius = inner.expect("inner interface implies an inner radius");
            let d_next =
                self.diffusion_at(triso_cell, self.point_at_radius(inner_radius - reinsert));
            if does_transmit(&mut self.rng.0, d_current, d_next, params.partition_k) {
                self.set_radius_to(inner_radius - reinsert);
            } else {
                self.set_radius_to(inner_radius + reinsert);
            }
        }

        HopOutcome::ReachedInterface
    }

    /// Advance the walker by pure diffusion until its simulated time reaches
    /// `until` or it is released from the OPyC surface.
    ///
    /// Returns `true` if the walker is still inside the particle at `until`,
    /// `false` if it was released first. This is the frame-stepping primitive for
    /// a real-time animation: call it each frame with the frame's target time,
    /// leaving the walker's position updated in place. Decay is not applied here
    /// (use [`WoSWalker::advance_until`] for the depleting walk).
    pub fn diffuse_until(
        &mut self,
        triso_cell: &TrisoCell,
        params: &WalkParams,
        until: Time,
    ) -> bool {
        for _ in 0..params.max_steps {
            if self.time >= until {
                return true;
            }
            if self.step_multilayer(triso_cell, params) == HopOutcome::Released {
                return false;
            }
        }
        true
    }

    /// Run the multilayer walk until the walker is released from the OPyC outer
    /// surface, returning the release time, or `None` if the step cap in
    /// `params` is hit first.
    ///
    /// This ignores decay — it is the pure-diffusion release-time driver used
    /// by the multilayer verification; decay and transmutation clocks are layered
    /// on in the depletion phase.
    pub fn walk_until_released(
        &mut self,
        triso_cell: &TrisoCell,
        params: &WalkParams,
    ) -> Option<Time> {
        for _ in 0..params.max_steps {
            if self.step_multilayer(triso_cell, params) == HopOutcome::Released {
                return Some(self.time);
            }
        }
        None
    }

    /// Diffusion coefficient of the walker's current nuclide at `position`,
    /// falling back to the cracked-layer default outside every known region.
    #[inline]
    fn diffusion_at(&self, triso_cell: &TrisoCell, position: [Length; 3]) -> DiffusionCoefficient {
        triso_cell
            .try_get_diffusion_coefficient(position, self.nuclide)
            .unwrap_or_else(|| {
                DiffusionCoefficient::new::<square_meter_per_second>(FALLBACK_DIFFUSION_M2_PER_S)
            })
    }

    /// The point at radial distance `target` from the centre, along the walker's
    /// current radial direction. Used to reinsert a walker just across (or back
    /// from) an interface. If the walker is at the exact centre, falls back to
    /// the `+x` axis so the direction is well defined.
    #[inline]
    fn point_at_radius(&self, target: Length) -> [Length; 3] {
        let rho = self.radius();
        if rho <= Length::ZERO {
            return [target, Length::ZERO, Length::ZERO];
        }
        let scale = (target / rho).get::<uom::si::ratio::ratio>();
        [
            self.position[0] * scale,
            self.position[1] * scale,
            self.position[2] * scale,
        ]
    }

    /// Move the walker radially to `target` (same direction, new radius).
    #[inline]
    fn set_radius_to(&mut self, target: Length) {
        self.position = self.point_at_radius(target);
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
    let (inner, outer) = shell_bounds(triso_cell, position)?;
    let rho = radial_distance(position);
    let dist_out = outer - rho;
    match inner.map(|r_in| rho - r_in) {
        Some(dist_in) => Some(if dist_in < dist_out {
            dist_in
        } else {
            dist_out
        }),
        None => Some(dist_out),
    }
}

/// The inner and outer bounding-sphere radii of the shell containing
/// `position`.
///
/// Returns `(inner, outer)` where `inner` is `None` for the fuel kernel (which
/// has no inner boundary) and `outer` is the shell's outer radius. Returns
/// `None` if the walker is outside the particle. This is the geometry the
/// Walk-on-Spheres step uses to size hops and to identify which interface a
/// walker has reached.
#[inline]
pub fn shell_bounds(
    triso_cell: &TrisoCell,
    position: [Length; 3],
) -> Option<(Option<Length>, Length)> {
    let bounds = match triso_cell.get_triso_region(position) {
        TrisoRegion::Fuel => (None, triso_cell.get_fuel_radius()),
        TrisoRegion::Buffer => (
            Some(triso_cell.get_fuel_radius()),
            triso_cell.get_buffer_radius(),
        ),
        TrisoRegion::IPyC => (
            Some(triso_cell.get_buffer_radius()),
            triso_cell.get_ipyc_radius(),
        ),
        TrisoRegion::SiC => (
            Some(triso_cell.get_ipyc_radius()),
            triso_cell.get_sic_radius(),
        ),
        TrisoRegion::OPyC => (
            Some(triso_cell.get_sic_radius()),
            triso_cell.get_opyc_radius(),
        ),
        TrisoRegion::Outside => return None,
    };
    Some(bounds)
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
        [Length::new::<micrometer>(x_um), Length::ZERO, Length::ZERO]
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

    /// **The decisive interface-rule check.** A single ergodic walker in a
    /// two-region reflecting sphere (inner `D_in`, outer `D_out`, interface at
    /// `a`, reflecting wall at `b`) must, at equilibrium with unit partition,
    /// spend a fraction of its *time* in each region equal to that region's
    /// *volume* fraction — i.e. the concentration is uniform — regardless of the
    /// diffusivity contrast. This is what fixes the transmission rule: it holds
    /// for the linear `p = D2/(D1+D2)` rule and fails for the `sqrt(D)` rule.
    #[test]
    fn interface_rule_gives_uniform_equilibrium_density() {
        use crate::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::interface::does_transmit;
        use uom::si::length::{meter, micrometer, nanometer};
        use uom::si::ratio::ratio;
        use uom::si::time::second;

        let a = Length::new::<micrometer>(50.0);
        let b = Length::new::<micrometer>(100.0);
        let d_in = DiffusionCoefficient::new::<square_meter_per_second>(1e-8);
        let d_out = DiffusionCoefficient::new::<square_meter_per_second>(1e-9); // 10x contrast
        let eps = Length::new::<nanometer>(20.0);
        let reinsert = eps * 3.0;
        let k = 1.0;

        let vol_frac_in = (a.get::<meter>() / b.get::<meter>()).powi(3); // (50/100)^3 = 0.125

        let set_radius = |pos: &mut [Length; 3], target: Length| {
            let rho = radial_distance(*pos);
            if rho <= Length::ZERO {
                *pos = [target, Length::ZERO, Length::ZERO];
                return;
            }
            let scale = (target / rho).get::<ratio>();
            *pos = [pos[0] * scale, pos[1] * scale, pos[2] * scale];
        };

        let mut seed = 0x1357_9bdf_2468_ace0_u64;
        let mut pos: [Length; 3] = [a * 0.5, Length::ZERO, Length::ZERO];
        let mut t_in = 0.0_f64;
        let mut t_out = 0.0_f64;

        for _ in 0..3_000_000_u64 {
            let rho = radial_distance(pos);
            let in_inner = rho < a;
            let d_here = if in_inner { d_in } else { d_out };
            let (inner_r, outer_r): (Option<Length>, Length) =
                if in_inner { (None, a) } else { (Some(a), b) };
            let dist_out = outer_r - rho;
            let dist_in = inner_r.map(|r| rho - r);
            let hop_radius = match dist_in {
                Some(di) if di < dist_out => di,
                _ => dist_out,
            };

            if hop_radius > eps {
                let tau = sample_first_passage_time(&mut seed, hop_radius, d_here).get::<second>();
                if in_inner {
                    t_in += tau;
                } else {
                    t_out += tau;
                }
                let dir = sample_uniform_direction(&mut seed);
                pos = [
                    pos[0] + hop_radius * dir[0],
                    pos[1] + hop_radius * dir[1],
                    pos[2] + hop_radius * dir[2],
                ];
            } else {
                let at_outer = match dist_in {
                    Some(di) => dist_out <= di,
                    None => true,
                };
                if at_outer && outer_r == b {
                    // Reflecting outer wall (no absorption).
                    set_radius(&mut pos, b - reinsert);
                } else if at_outer {
                    // Interface at `a`, currently inside going out.
                    if does_transmit(&mut seed, d_here, d_out, k) {
                        set_radius(&mut pos, a + reinsert);
                    } else {
                        set_radius(&mut pos, a - reinsert);
                    }
                } else {
                    // Interface at `a`, currently outside going in.
                    if does_transmit(&mut seed, d_here, d_in, k) {
                        set_radius(&mut pos, a - reinsert);
                    } else {
                        set_radius(&mut pos, a + reinsert);
                    }
                }
            }
        }

        let measured_in = t_in / (t_in + t_out);
        println!(
            "interface equilibrium: measured inner time-fraction {measured_in:.4}, \
             volume fraction {vol_frac_in:.4}, abs error {:.4}",
            (measured_in - vol_frac_in).abs()
        );
        assert!(
            (measured_in - vol_frac_in).abs() < 0.02,
            "equilibrium inner time fraction {measured_in:.4} vs volume fraction {vol_frac_in:.4} \
             (uniform density) — transmission rule is wrong if these disagree"
        );
    }

    /// A walker started in the fuel kernel transmits outward across at least one
    /// interior interface within a bounded number of steps, exercising the
    /// multilayer machinery end to end.
    #[test]
    fn multilayer_walk_transmits_across_interfaces() {
        let cell = TrisoCell::new_crp6_geometry();
        let params = WalkParams {
            capture_eps: Length::new::<uom::si::length::nanometer>(20.0),
            reinsert_factor: 2.0,
            partition_k: 1.0,
            max_steps: 500_000,
        };
        let mut walker = WoSWalker::new_at_center(Nuclide::Cs137, OoRng64::from_u64(0x2024));
        let fuel_radius = cell.get_fuel_radius();
        let mut left_kernel = false;
        let mut saw_interface = false;
        for _ in 0..params.max_steps {
            match walker.step_multilayer(&cell, &params) {
                HopOutcome::ReachedInterface => saw_interface = true,
                HopOutcome::Released => break,
                HopOutcome::Stepped => {}
            }
            if walker.radius() > fuel_radius {
                left_kernel = true;
            }
            if left_kernel && saw_interface {
                break;
            }
        }
        assert!(saw_interface, "walker never reached an interface");
        assert!(
            left_kernel,
            "walker never transmitted out of the fuel kernel"
        );
    }
}
