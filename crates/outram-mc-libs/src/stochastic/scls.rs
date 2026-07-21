//! Semi-Implicit Chord Length Sampling (SCLS) — CLS with bounded geometric memory.
//!
//! Classical CLS ([`super::cls`]) is memoryless: every sampled chord is independent, so
//! a neutron that scatters backwards does not re-encounter the inclusion it just flew
//! through. That is the dominant error source in CLS, and it grows exactly where
//! dispersion-fuel problems live — optically thick inclusions and strong scattering.
//!
//! SCLS keeps the cheap chord sampling but **remembers recently-encountered
//! inclusions**. Sampled inclusions are promoted to stored [`ParticleHistory`] records
//! and consulted on subsequent flights, so a neutron re-crossing its own path sees the
//! same geometry twice — recovering the correlation CLS throws away, without ever
//! storing the full O(10⁴)-kernel packing an explicit model needs.
//!
//! Memory is bounded by the **Dynamic Inclusion Sphere** ([`InclusionSphere`]): a ball
//! that follows the neutron and defines what counts as "local". Histories inside are
//! retained; histories the neutron has flown away from are culled.
//!
//! ```text
//!            ┌──────── inclusion sphere (moves with neutron) ────────┐
//!  culled ○  │   ● retained    ● retained      ◉ neutron             │  ○ culled
//!            └───────────────────────────────────────────────────────┘
//!             R = λ_TMFP + R_largest
//! ```
//!
//! # Sphere radius
//!
//! ```text
//! R = λ_TMFP + R_largest
//! ```
//!
//! One transport mean free path is the distance over which the neutron's direction
//! decorrelates, so geometry beyond it is unlikely to be revisited before being
//! forgotten anyway; adding the largest inclusion radius guarantees that any inclusion
//! whose *body* could still intersect the local neighbourhood is retained even when its
//! centre sits just outside. See [`InclusionSphere::new`].
//!
//! # What is implemented here
//!
//! The retention machinery, which is self-contained and testable:
//!
//! - [`ParticleHistory`] / [`FlightSegment`] — the retained records (design doc §14).
//! - [`InclusionSphere`] — radius rule, containment, re-centring (design doc §15).
//! - [`SclsMedium::advance_to`] — move the neutron, re-centre the sphere, cull.
//! - [`SclsMedium::retained_material_at`] — exact answer *for retained geometry*.
//!
//! # The transport driver (bead `op-eby.3`, implemented)
//!
//! [`SclsMedium::material_at`] marches the flight and resolves each phase boundary:
//! matrix gaps are memoryless exponential chords, but a matrix→inclusion crossing first
//! tests the ray against every retained inclusion and **re-enters a remembered sphere**
//! when one is closer than the sampled gap — recovering the correlation classical CLS
//! discards. A genuinely new crossing spawns a real sphere with a correctly-distributed
//! impact parameter (`b = R√ξ`, giving the Cauchy mean chord `4R/3`) and remembers it;
//! inclusion exits are the exact geometric ray–sphere exit. Its occupancy still
//! converges to the packing fraction, and the memory is demonstrated by a re-crossing
//! test whose history count saturates instead of growing with the number of passes.
//! Coupling into the k-eigenvalue loop and the RSA-vs-CLS-vs-SCLS accuracy comparison is
//! the benchmark of bead `op-eby.7`; the adaptive-radius extension is `op-eby.6`.
//!
//! **No accuracy claim is made here.** Whether SCLS actually recovers the explicit-RSA
//! answer is an empirical question that the benchmark suite (bead `op-eby.7`) must
//! measure before anything in this module may be described as validated.
//!
//! This module is **new work**, not an OpenMC port — upstream has no SCLS.

use crate::geometry::position::{Direction, Position};
use crate::rng::lcg::prn;
use crate::stochastic::cls::sample_chord;
use crate::stochastic::medium::{MaterialId, MediumError};

/// Squared distance \[cm²\] between two points.
fn dist_sq(a: Position, b: Position) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    dx * dx + dy * dy + dz * dz
}

/// Distance from an interior point `p` along unit direction `u` to where the ray
/// **leaves** the sphere `(center, radius)` \[cm\]. Assumes `p` is inside; returns the
/// far (positive) root of `|p + t·u − center|² = R²`.
fn ray_sphere_exit(p: Position, u: Direction, center: Position, radius: f64) -> f64 {
    let oc = p - center;
    let b = u.dot_pos(oc); // u·(p − c)
    let c = oc.norm_sqr() - radius * radius;
    let disc = (b * b - c).max(0.0);
    -b + disc.sqrt()
}

/// Distance from an exterior point `p` along unit direction `u` to where the ray first
/// **enters** the sphere `(center, radius)` \[cm\], or `None` if the ray misses it or
/// only grazes behind `p`. `eps` skips a sphere the neutron is essentially already on
/// (e.g. the one it just exited), avoiding an immediate spurious re-entry.
fn ray_sphere_entry(
    p: Position,
    u: Direction,
    center: Position,
    radius: f64,
    eps: f64,
) -> Option<f64> {
    let oc = p - center;
    let b = u.dot_pos(oc);
    let c = oc.norm_sqr() - radius * radius;
    let disc = b * b - c;
    if disc < 0.0 {
        return None; // ray misses the sphere
    }
    let t = -b - disc.sqrt(); // near root
    if t > eps {
        Some(t)
    } else {
        None
    }
}

/// A unit vector perpendicular to `u`, at azimuth `phi` \[rad\] around it. Used to place
/// a freshly-sampled inclusion off the flight axis at the correct impact parameter.
fn perpendicular(u: Direction, phi: f64) -> Position {
    // Build one perpendicular e1 by crossing u with whichever axis is least aligned.
    let a = if u.u.abs() < 0.9 {
        Position::new(1.0, 0.0, 0.0)
    } else {
        Position::new(0.0, 1.0, 0.0)
    };
    let ux = Position::new(u.u, u.v, u.w);
    // e1 = normalize(a − (a·u)u)
    let e1 = {
        let proj = a.dot(ux);
        let v = a - ux * proj;
        v / v.norm()
    };
    // e2 = u × e1
    let e2 = Position::new(
        ux.y * e1.z - ux.z * e1.y,
        ux.z * e1.x - ux.x * e1.z,
        ux.x * e1.y - ux.y * e1.x,
    );
    e1 * phi.cos() + e2 * phi.sin()
}

/// One remembered inclusion — an inclusion the neutron has already encountered.
///
/// Design doc §14. Stored so that a later flight through the same neighbourhood sees
/// the same inclusion rather than re-sampling an independent one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleHistory {
    /// Inclusion centre \[cm\].
    pub center: Position,
    /// Inclusion radius \[cm\].
    pub radius: f64,
    /// Material occupying the inclusion.
    pub material_id: MaterialId,
}

impl ParticleHistory {
    /// A remembered inclusion at `center` \[cm\] with `radius` \[cm\].
    pub fn new(center: Position, radius: f64, material_id: MaterialId) -> Self {
        Self {
            center,
            radius,
            material_id,
        }
    }

    /// Whether `p` \[cm\] lies inside this inclusion.
    pub fn contains(&self, p: Position) -> bool {
        dist_sq(self.center, p) < self.radius * self.radius
    }
}

/// One remembered straight-line flight leg between collisions.
///
/// Design doc §14. Retaining traversed segments lets an SCLS driver detect when a
/// neutron re-enters ground it has already covered, which is where the memoryless
/// assumption does the most damage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlightSegment {
    /// Segment start \[cm\].
    pub start: Position,
    /// Segment end \[cm\].
    pub end: Position,
}

impl FlightSegment {
    /// A flight leg from `start` to `end` \[cm\].
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    /// Segment length \[cm\].
    pub fn length(&self) -> f64 {
        dist_sq(self.start, self.end).sqrt()
    }
}

/// The Dynamic Inclusion Sphere — the moving ball that bounds SCLS memory.
///
/// Design doc §15, the core SCLS innovation. Re-centred on the neutron at every
/// collision; anything it no longer covers is forgotten.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InclusionSphere {
    /// Current centre \[cm\] — tracks the neutron position.
    pub center: Position,
    /// Current radius \[cm\].
    pub radius: f64,
}

impl InclusionSphere {
    /// Build the sphere from the physics that sets its size.
    ///
    /// ```text
    /// R = λ_TMFP + R_largest
    /// ```
    ///
    /// # Parameters
    /// - `center` — initial centre \[cm\] (the neutron's position).
    /// - `transport_mfp` — transport mean free path λ_TMFP \[cm\], > 0. The scale over
    ///   which direction decorrelates, so the scale beyond which retained geometry
    ///   stops paying for itself.
    /// - `largest_inclusion_radius` — R_largest \[cm\], the biggest inclusion radius in
    ///   the medium. Included so an inclusion whose centre is just outside the ball but
    ///   whose body still reaches into it is not wrongly culled.
    ///
    /// Both length inputs are clamped at 0, so a degenerate input yields a
    /// zero-radius sphere rather than a negative one.
    pub fn new(center: Position, transport_mfp: f64, largest_inclusion_radius: f64) -> Self {
        let radius = transport_mfp.max(0.0) + largest_inclusion_radius.max(0.0);
        Self { center, radius }
    }

    /// Whether point `p` \[cm\] lies within the sphere.
    pub fn contains(&self, p: Position) -> bool {
        dist_sq(self.center, p) < self.radius * self.radius
    }

    /// Whether an inclusion of `radius` \[cm\] centred at `center` \[cm\] overlaps this
    /// sphere at all.
    ///
    /// This — not [`Self::contains`] — is the correct retention test: an inclusion whose
    /// centre is outside the ball can still have part of its body inside it, and culling
    /// it would punch a hole in the retained geometry.
    pub fn overlaps(&self, center: Position, radius: f64) -> bool {
        let reach = self.radius + radius;
        dist_sq(self.center, center) < reach * reach
    }

    /// Move the sphere to follow the neutron to `new_center` \[cm\]. Radius unchanged.
    pub fn recenter(&mut self, new_center: Position) {
        self.center = new_center;
    }
}

/// A semi-implicit CLS medium — chord statistics plus a bounded window of remembered
/// geometry.
///
/// Sits between [`super::cls::ClsMedium`] (no memory, O(1) storage) and
/// [`super::medium::RsaMedium`] (total memory, O(N) storage): memory is O(number of
/// inclusions within one inclusion-sphere volume), independent of how large the overall
/// medium is.
#[derive(Debug, Clone, PartialEq)]
pub struct SclsMedium {
    /// The underlying memoryless chord sampler.
    cls: super::cls::ClsMedium,
    /// The moving retention window.
    sphere: InclusionSphere,
    /// Remembered inclusions currently inside the window.
    histories: Vec<ParticleHistory>,
    /// Remembered flight legs currently inside the window.
    flights: Vec<FlightSegment>,
    /// Transient per-history flight state, `None` until the first [`Self::material_at`].
    flight: Option<SclsFlight>,
}

/// Transient flight state for SCLS phase reconstruction (see [`SclsMedium::material_at`]).
///
/// Unlike CLS, SCLS needs the current *inclusion* (when inside one) so it can compute a
/// geometric exit, and it derives the flight direction from successive query positions
/// so it can test the ray against retained inclusions.
#[derive(Debug, Clone, Copy, PartialEq)]
struct SclsFlight {
    /// Position of the previous `material_at` query \[cm\].
    last_position: Position,
    /// `true` if the neutron is currently inside an inclusion.
    in_inclusion: bool,
    /// The sphere `(center, radius)` the neutron is inside, when `in_inclusion`.
    current_sphere: Option<(Position, f64)>,
}

impl SclsMedium {
    /// Build an SCLS medium around a CLS sampler.
    ///
    /// - `cls` — the chord statistics to fall back on when no history covers a point.
    /// - `start` — the neutron's initial position \[cm\], the sphere's first centre.
    /// - `transport_mfp` — λ_TMFP \[cm\], see [`InclusionSphere::new`].
    ///
    /// R_largest is taken from the CLS medium's inclusion radius, since the scaffold
    /// assumes an equal-radius packing (as [`crate::pebble_beds::sphere_packing::pack_spheres`]
    /// generates). A polydisperse packing would pass the true maximum instead.
    pub fn new(cls: super::cls::ClsMedium, start: Position, transport_mfp: f64) -> Self {
        let sphere = InclusionSphere::new(start, transport_mfp, cls.inclusion_radius());
        Self {
            cls,
            sphere,
            histories: Vec::new(),
            flights: Vec::new(),
            flight: None,
        }
    }

    /// The underlying memoryless chord sampler.
    pub fn cls(&self) -> &super::cls::ClsMedium {
        &self.cls
    }

    /// The current retention window.
    pub fn sphere(&self) -> InclusionSphere {
        self.sphere
    }

    /// Currently remembered inclusions.
    pub fn histories(&self) -> &[ParticleHistory] {
        &self.histories
    }

    /// Currently remembered flight legs.
    pub fn flights(&self) -> &[FlightSegment] {
        &self.flights
    }

    /// Remember an inclusion the neutron has encountered.
    ///
    /// Ignored if it does not overlap the current window (it would be culled
    /// immediately anyway).
    pub fn remember_inclusion(&mut self, history: ParticleHistory) {
        if self.sphere.overlaps(history.center, history.radius) {
            self.histories.push(history);
        }
    }

    /// Remember a traversed flight leg.
    pub fn remember_flight(&mut self, segment: FlightSegment) {
        self.flights.push(segment);
    }

    /// Move the neutron to `new_position` \[cm\], re-centre the window, and cull.
    ///
    /// This is the per-collision update of design doc §15:
    ///
    /// ```text
    /// move neutron → update sphere centre → cull old histories → retain local ones
    /// ```
    ///
    /// Returns how many histories were culled, which is the quantity an adaptive-radius
    /// study (bead `op-eby.6`) would drive its feedback from.
    pub fn advance_to(&mut self, new_position: Position) -> usize {
        self.sphere.recenter(new_position);
        let before = self.histories.len();
        let sphere = self.sphere;
        self.histories
            .retain(|h| sphere.overlaps(h.center, h.radius));
        // A flight leg stays relevant while either endpoint is still local.
        self.flights
            .retain(|f| sphere.contains(f.start) || sphere.contains(f.end));
        before - self.histories.len()
    }

    /// Set the retention-window radius \[cm\] directly, then cull anything the resized
    /// window no longer covers. Used by the adaptive-radius extension ([`AdaptiveRadius`],
    /// bead `op-eby.6`) to replace the fixed `λ_TMFP + R_largest` rule with a
    /// locally-estimated radius. Clamped at 0.
    pub fn set_sphere_radius(&mut self, radius: f64) {
        self.sphere.radius = radius.max(0.0);
        let sphere = self.sphere;
        self.histories.retain(|h| sphere.overlaps(h.center, h.radius));
        self.flights
            .retain(|f| sphere.contains(f.start) || sphere.contains(f.end));
    }

    /// One adaptive step: feed the just-completed collision-to-collision `track_length`
    /// \[cm\] to `controller`, resize the retention window to the radius it returns, and
    /// cull. This is design-doc §17's variable-radius SCLS — the window grows where the
    /// neutron streams (long tracks → more geometry worth remembering) and shrinks where
    /// it collides often (short tracks → local memory suffices), instead of holding the
    /// static `λ_TMFP` a fixed-radius run assumes.
    pub fn adapt_radius(&mut self, controller: &mut AdaptiveRadius, track_length: f64) {
        let r = controller.update(track_length);
        self.set_sphere_radius(r);
    }

    /// Material at `p` \[cm\] **if remembered geometry already answers the question**.
    ///
    /// Returns `Some(inclusion_material)` when `p` falls inside a retained inclusion.
    /// Returns `None` when no history covers `p` — which does *not* mean "matrix", only
    /// "not remembered": the point may lie in an inclusion that was never sampled or has
    /// been culled. Resolving `None` requires sampling, which is the transport driver's
    /// job (bead `op-eby.3`).
    pub fn retained_material_at(&self, p: Position) -> Option<MaterialId> {
        self.histories
            .iter()
            .find(|h| h.contains(p))
            .map(|h| h.material_id)
    }

    /// Discard the in-progress flight so the next [`Self::material_at`] re-seeds the
    /// phase. Retained histories are *kept* — they are geometry, not per-flight state —
    /// so a new history in the same neighbourhood still benefits from the memory.
    pub fn begin_flight(&mut self) {
        self.flight = None;
    }

    /// The phase the reconstructed flight is currently in, or `None` before the first
    /// [`Self::material_at`]. `true` means inside an inclusion.
    pub fn in_inclusion(&self) -> Option<bool> {
        self.flight.map(|f| f.in_inclusion)
    }

    /// Material occupying `position` \[cm\], reconstructed along the SCLS flight
    /// (bead `op-eby.3`).
    ///
    /// SCLS is CLS with bounded geometric memory. The reconstruction marches the
    /// straight segment from the previous query to `position` along its direction and
    /// resolves each phase boundary:
    ///
    /// - **Matrix → inclusion.** Sample an exponential matrix chord (the memoryless
    ///   Markov gap), but *first* test the ray against every retained inclusion: if one
    ///   lies closer than the sampled gap, the neutron **re-enters that remembered
    ///   sphere** instead of inventing an independent one — this is the correlation CLS
    ///   throws away. A genuinely new crossing spawns a real sphere, placed with a
    ///   correctly-distributed impact parameter (`b = R√ξ`) so its geometric chord has
    ///   the Cauchy mean `4R/3`, and remembers it (if it overlaps the window).
    /// - **Inclusion → matrix.** Exit is the geometric ray–sphere exit of the specific
    ///   sphere the neutron is in — exact, not resampled.
    ///
    /// After the march the retention window is re-centred on `position` and non-local
    /// histories are culled ([`Self::advance_to`]). Call [`Self::begin_flight`] between
    /// independent histories. `seed` is the crate LCG stream ([`prn`]), advanced in
    /// place. Never errors — the `Result` matches the shared
    /// [`super::medium::StochasticMedium::material_at`] contract.
    pub fn material_at(
        &mut self,
        position: Position,
        seed: &mut u64,
    ) -> Result<MaterialId, MediumError> {
        let mean_matrix = self.cls.mean_chord_matrix();
        let radius = self.cls.inclusion_radius();
        let inclusion_mat = self.cls.inclusion_material();
        let matrix_mat = self.cls.matrix_material();
        let pf = self.cls.packing_fraction();
        const EPS: f64 = 1e-12;

        // First query: seed the phase from the volume-fraction prior.
        let mut fl = match self.flight.take() {
            Some(f) => f,
            None => {
                let in_inclusion = prn(seed) < pf;
                // If seeded inside an inclusion, anchor a sphere on the birth point
                // (neutron at centre — a negligible one-off; later inclusions are
                // placed with the proper impact parameter).
                let current_sphere = in_inclusion.then(|| {
                    let h = ParticleHistory::new(position, radius, inclusion_mat);
                    self.remember_inclusion(h);
                    (position, radius)
                });
                self.flight = Some(SclsFlight {
                    last_position: position,
                    in_inclusion,
                    current_sphere,
                });
                return Ok(if in_inclusion { inclusion_mat } else { matrix_mat });
            }
        };

        let seg = position - fl.last_position;
        let len = seg.norm();
        if len > 0.0 {
            let u = Direction::from_unnormalised(seg.x, seg.y, seg.z);
            let u_pos = Position::new(u.u, u.v, u.w);
            let mut pos = fl.last_position;
            let mut t = 0.0;
            loop {
                if fl.in_inclusion {
                    let (center, r) = fl
                        .current_sphere
                        .expect("in_inclusion implies a current sphere");
                    let d_exit = ray_sphere_exit(pos, u, center, r);
                    if t + d_exit <= len {
                        t += d_exit;
                        pos = pos + u_pos * d_exit;
                        fl.in_inclusion = false;
                        fl.current_sphere = None;
                    } else {
                        break;
                    }
                } else {
                    // Memoryless matrix gap...
                    let d_sample = sample_chord(mean_matrix, seed);
                    // ...unless a remembered inclusion lies closer along the ray.
                    let mut d_incl = d_sample;
                    let mut hit: Option<(Position, f64)> = None;
                    for h in &self.histories {
                        if let Some(d) = ray_sphere_entry(pos, u, h.center, h.radius, EPS) {
                            if d < d_incl {
                                d_incl = d;
                                hit = Some((h.center, h.radius));
                            }
                        }
                    }
                    if t + d_incl <= len {
                        t += d_incl;
                        pos = pos + u_pos * d_incl;
                        fl.in_inclusion = true;
                        // Borrow of self.histories is released; safe to mutate now.
                        let sphere = match hit {
                            Some(existing) => existing,
                            None => {
                                let center = fresh_inclusion_center(pos, u, radius, seed);
                                self.remember_inclusion(ParticleHistory::new(
                                    center,
                                    radius,
                                    inclusion_mat,
                                ));
                                (center, radius)
                            }
                        };
                        fl.current_sphere = Some(sphere);
                    } else {
                        break;
                    }
                }
            }
            // Record the traversed leg for the retention bookkeeping.
            self.remember_flight(FlightSegment::new(fl.last_position, position));
        }

        fl.last_position = position;
        let result = if fl.in_inclusion { inclusion_mat } else { matrix_mat };
        self.flight = Some(fl);
        // Per design-doc §15: re-centre the window on the neutron and cull.
        self.advance_to(position);
        Ok(result)
    }
}

/// Adaptive inclusion-sphere radius controller (design doc §17, bead `op-eby.6`).
///
/// The fixed-radius SCLS ([`InclusionSphere::new`]) sizes the window from a *static*
/// transport mean free path `λ_TMFP` supplied up front. In a heterogeneous problem the
/// true local mean free path varies — long in optically-thin matrix, short in a dense
/// inclusion cluster — so a single `λ_TMFP` is either too small (forgetting geometry the
/// neutron will revisit) or too large (paying to remember geometry it never will).
///
/// This controller estimates the local mean free path online as an exponential moving
/// average of the observed collision-to-collision track lengths, and sets
///
/// ```text
/// R = clamp(<track> + R_largest, R_min, R_max)
/// ```
///
/// mirroring the fixed rule with `<track>` in place of the static `λ_TMFP`. The EMA
/// weight `alpha` in (0, 1] trades responsiveness (large) against stability (small).
///
/// This is a **research extension** whose accuracy/efficiency payoff is an empirical
/// question for the benchmark (`op-eby.7`); no claim is made here that it beats the
/// fixed-radius model.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdaptiveRadius {
    mean_track: f64,
    alpha: f64,
    largest_radius: f64,
    min_radius: f64,
    max_radius: f64,
}

impl AdaptiveRadius {
    /// Build a controller.
    ///
    /// - `initial_mfp` — starting mean-free-path estimate \[cm\] (e.g. the fixed-radius
    ///   run's `λ_TMFP`), used until enough tracks have been observed.
    /// - `alpha` — EMA weight in (0, 1]; clamped into that range.
    /// - `largest_radius` — `R_largest` \[cm\], added to the mean-free-path estimate.
    /// - `min_radius` / `max_radius` — clamp bounds \[cm\] on the resulting radius.
    pub fn new(
        initial_mfp: f64,
        alpha: f64,
        largest_radius: f64,
        min_radius: f64,
        max_radius: f64,
    ) -> Self {
        Self {
            mean_track: initial_mfp.max(0.0),
            alpha: alpha.clamp(f64::MIN_POSITIVE, 1.0),
            largest_radius: largest_radius.max(0.0),
            min_radius: min_radius.max(0.0),
            max_radius: max_radius.max(min_radius.max(0.0)),
        }
    }

    /// The current radius estimate \[cm\] without folding in a new track.
    pub fn radius(&self) -> f64 {
        (self.mean_track + self.largest_radius).clamp(self.min_radius, self.max_radius)
    }

    /// The current EMA mean track length \[cm\].
    pub fn mean_track(&self) -> f64 {
        self.mean_track
    }

    /// Fold a new collision-to-collision `track_length` \[cm\] into the EMA and return the
    /// updated, clamped radius \[cm\]. Non-finite or negative tracks are ignored.
    pub fn update(&mut self, track_length: f64) -> f64 {
        if track_length.is_finite() && track_length >= 0.0 {
            self.mean_track = self.alpha * track_length + (1.0 - self.alpha) * self.mean_track;
        }
        self.radius()
    }
}

/// Place the centre of a freshly-sampled inclusion the neutron enters at `entry` \[cm\]
/// travelling along `u`. The impact parameter is sampled `b = R√ξ` (the correct
/// distribution for isotropic rays hitting a sphere, giving a mean geometric chord
/// `4R/3`), and the centre is offset off the flight axis by `b` at a uniform azimuth,
/// so the ray enters exactly at `entry` and exits at `entry + 2√(R²−b²)·u`.
fn fresh_inclusion_center(entry: Position, u: Direction, radius: f64, seed: &mut u64) -> Position {
    use std::f64::consts::PI;
    let b = radius * prn(seed).sqrt();
    let axial = (radius * radius - b * b).max(0.0).sqrt();
    let phi = 2.0 * PI * prn(seed);
    let w = perpendicular(u, phi);
    entry + Position::new(u.u, u.v, u.w) * axial + w * b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stochastic::cls::ClsMedium;

    fn medium() -> SclsMedium {
        let cls = ClsMedium::new(0.03, 0.2, MaterialId(1), MaterialId(0));
        SclsMedium::new(cls, Position::new(0.0, 0.0, 0.0), 1.0)
    }

    /// The sphere radius is exactly lambda_TMFP + R_largest.
    #[test]
    fn inclusion_sphere_radius_is_tmfp_plus_largest_radius() {
        let s = InclusionSphere::new(Position::new(0.0, 0.0, 0.0), 1.5, 0.03);
        assert!((s.radius - 1.53).abs() < 1e-15);

        // Degenerate inputs clamp rather than going negative.
        let d = InclusionSphere::new(Position::new(0.0, 0.0, 0.0), -1.0, -1.0);
        assert_eq!(d.radius, 0.0);
    }

    /// Retention uses body overlap, not centre containment: an inclusion whose centre is
    /// outside the ball but whose body reaches in must be kept.
    #[test]
    fn overlap_test_keeps_inclusions_straddling_the_boundary() {
        let s = InclusionSphere::new(Position::new(0.0, 0.0, 0.0), 1.0, 0.1);
        // radius = 1.1; a centre at 1.15 is outside...
        let straddler = Position::new(1.15, 0.0, 0.0);
        assert!(!s.contains(straddler), "centre is outside the ball");
        // ...but a body of radius 0.1 reaches to 1.05, inside the 1.1 ball.
        assert!(
            s.overlaps(straddler, 0.1),
            "body still overlaps, must be retained"
        );
        // A genuinely distant inclusion does not overlap.
        assert!(!s.overlaps(Position::new(5.0, 0.0, 0.0), 0.1));
    }

    /// Advancing culls exactly the histories the neutron has flown away from.
    #[test]
    fn advancing_culls_non_local_histories() {
        let mut m = medium();
        // Sphere radius = 1.0 + 0.03 = 1.03, centred at origin.
        m.remember_inclusion(ParticleHistory::new(
            Position::new(0.0, 0.0, 0.0),
            0.03,
            MaterialId(1),
        ));
        m.remember_inclusion(ParticleHistory::new(
            Position::new(1.0, 0.0, 0.0),
            0.03,
            MaterialId(1),
        ));
        assert_eq!(m.histories().len(), 2);

        // Fly far away; both histories fall outside the window.
        let culled = m.advance_to(Position::new(10.0, 0.0, 0.0));
        assert_eq!(culled, 2);
        assert!(m.histories().is_empty());
    }

    /// An inclusion outside the window is never stored in the first place.
    #[test]
    fn remembering_a_distant_inclusion_is_a_no_op() {
        let mut m = medium();
        m.remember_inclusion(ParticleHistory::new(
            Position::new(50.0, 0.0, 0.0),
            0.03,
            MaterialId(1),
        ));
        assert!(m.histories().is_empty());
    }

    /// Retained geometry answers exactly where it can, and honestly reports None where
    /// it cannot — never defaulting to "matrix".
    #[test]
    fn retained_lookup_distinguishes_unknown_from_matrix() {
        let mut m = medium();
        let c = Position::new(0.1, 0.0, 0.0);
        m.remember_inclusion(ParticleHistory::new(c, 0.03, MaterialId(1)));

        // Inside a remembered inclusion -> exact answer.
        assert_eq!(m.retained_material_at(c), Some(MaterialId(1)));
        // Not covered by any history -> "unknown", not "matrix".
        assert_eq!(m.retained_material_at(Position::new(0.5, 0.0, 0.0)), None);
    }

    /// Flight legs are retained while either endpoint stays local.
    #[test]
    fn flight_segments_are_culled_with_the_window() {
        let mut m = medium();
        m.remember_flight(FlightSegment::new(
            Position::new(0.0, 0.0, 0.0),
            Position::new(0.5, 0.0, 0.0),
        ));
        assert_eq!(m.flights().len(), 1);
        let seg = m.flights()[0];
        assert!((seg.length() - 0.5).abs() < 1e-15);

        m.advance_to(Position::new(20.0, 0.0, 0.0));
        assert!(m.flights().is_empty());
    }

    /// Ray–sphere entry/exit distances are exact for a known geometry.
    #[test]
    fn ray_sphere_intersection_distances_are_exact() {
        let u = Direction::new(1.0, 0.0, 0.0);
        // Sphere centred at x=1, radius 0.25: a +x ray from the origin enters at 0.75,
        // exits at 1.25.
        let c = Position::new(1.0, 0.0, 0.0);
        let entry = ray_sphere_entry(Position::ZERO, u, c, 0.25, 1e-12).unwrap();
        assert!((entry - 0.75).abs() < 1e-12, "entry {entry}");
        // From just inside (x=0.9) the exit is at 1.25 -> distance 0.35.
        let exit = ray_sphere_exit(Position::new(0.9, 0.0, 0.0), u, c, 0.25);
        assert!((exit - 0.35).abs() < 1e-12, "exit {exit}");
        // A ray that misses (offset in y beyond the radius) has no entry.
        assert!(ray_sphere_entry(Position::new(0.0, 1.0, 0.0), u, c, 0.25, 1e-12).is_none());
    }

    /// A fresh inclusion is placed so the ray enters exactly at the entry point and the
    /// centre sits one radius-scaled impact parameter off-axis: `|entry − centre| = R`.
    #[test]
    fn fresh_inclusion_center_puts_entry_on_the_surface() {
        let u = Direction::from_unnormalised(1.0, 1.0, 0.0);
        let entry = Position::new(0.2, -0.1, 0.3);
        let mut seed = 5u64;
        for _ in 0..1000 {
            let c = fresh_inclusion_center(entry, u, 0.03, &mut seed);
            // Entry lies on the sphere surface.
            assert!((entry.distance(c) - 0.03).abs() < 1e-12);
        }
    }

    /// SCLS also reproduces the packing fraction as the flight-occupancy statistic —
    /// bounded memory must not bias the volume fraction it converges to.
    #[test]
    fn inclusion_fraction_along_scls_flight_converges_to_packing_fraction() {
        let pf = 0.2;
        let cls = ClsMedium::new(0.03, pf, MaterialId(1), MaterialId(0));
        // Large TMFP so nothing is culled over the test flight.
        let mut m = SclsMedium::new(cls, Position::ZERO, 100.0);

        let mut seed = 20260721u64;
        let step = 0.01;
        let n = 300_000;
        let mut in_incl = 0usize;
        for i in 0..n {
            let x = i as f64 * step;
            if m.material_at(Position::new(x, 0.0, 0.0), &mut seed).unwrap() == MaterialId(1) {
                in_incl += 1;
            }
        }
        let frac = in_incl as f64 / n as f64;
        assert!((frac - pf).abs() < 0.03, "SCLS inclusion fraction {frac} vs pf {pf}");
    }

    /// The memory property: oscillating across a small region many times does **not**
    /// grow the history set without bound — the neutron re-enters remembered inclusions
    /// instead of inventing a fresh one on every pass, so the count saturates far below
    /// the number of crossings. A memoryless model would keep sampling new inclusions.
    #[test]
    fn re_crossing_reuses_remembered_inclusions() {
        let cls = ClsMedium::new(0.03, 0.2, MaterialId(1), MaterialId(0));
        // TMFP large enough that the whole oscillation region stays in the window.
        let mut m = SclsMedium::new(cls, Position::ZERO, 100.0);

        let mut seed = 777u64;
        let sweeps = 400;
        let pts = 40; // points per one-way sweep across [0, 0.4] cm
        for s in 0..sweeps {
            for i in 0..pts {
                // Triangle wave: forward on even sweeps, backward on odd.
                let frac = i as f64 / pts as f64;
                let x = if s % 2 == 0 { frac } else { 1.0 - frac } * 0.4;
                let _ = m.material_at(Position::new(x, 0.0, 0.0), &mut seed).unwrap();
            }
        }
        // 400 sweeps × 40 points = 16000 crossings of a ~0.4 cm region. Memoryless
        // sampling could spawn thousands of inclusions; memory keeps it bounded to the
        // handful that actually fit in the region (region/⟨ℓ⟩ ~ a few dozen).
        let n = m.histories().len();
        assert!(n < 500, "history count {n} should saturate, not grow with crossings");
        assert!(!m.histories().is_empty(), "some inclusions must have been remembered");
    }

    /// The adaptive controller's EMA converges to a constant track length, and the
    /// radius it reports is that plus R_largest, clamped to the bounds.
    #[test]
    fn adaptive_radius_ema_converges_and_clamps() {
        let mut c = AdaptiveRadius::new(0.1, 0.2, 0.03, 0.05, 5.0);
        for _ in 0..500 {
            c.update(1.0);
        }
        assert!((c.mean_track() - 1.0).abs() < 1e-6, "EMA -> 1.0, got {}", c.mean_track());
        assert!((c.radius() - 1.03).abs() < 1e-6, "R = track + R_largest");

        for _ in 0..1000 {
            c.update(0.001);
        }
        assert!((c.radius() - 0.05).abs() < 1e-9, "clamped at min_radius");

        for _ in 0..1000 {
            c.update(100.0);
        }
        assert!((c.radius() - 5.0).abs() < 1e-9, "clamped at max_radius");

        // Non-finite / negative tracks are ignored (no NaN poisoning).
        let before = c.mean_track();
        c.update(f64::NAN);
        c.update(-3.0);
        assert_eq!(c.mean_track(), before);
    }

    /// Driving `adapt_radius` with short tracks shrinks the window and culls histories
    /// the smaller window no longer covers.
    #[test]
    fn adapt_radius_resizes_window_and_culls() {
        let mut m = medium(); // sphere radius 1.0 + 0.03 = 1.03 at origin
        m.remember_inclusion(ParticleHistory::new(
            Position::new(0.8, 0.0, 0.0),
            0.03,
            MaterialId(1),
        ));
        assert_eq!(m.histories().len(), 1);

        let mut c = AdaptiveRadius::new(1.0, 0.5, 0.03, 0.1, 2.0);
        for _ in 0..50 {
            m.adapt_radius(&mut c, 0.001);
        }
        assert!(m.sphere().radius < 0.2, "window shrank, got {}", m.sphere().radius);
        assert!(m.histories().is_empty(), "distant inclusion culled by the smaller window");
    }

    /// The reconstruction is deterministic in the LCG stream.
    #[test]
    fn scls_flight_is_reproducible() {
        let run = || {
            let cls = ClsMedium::new(0.03, 0.25, MaterialId(1), MaterialId(0));
            let mut m = SclsMedium::new(cls, Position::ZERO, 5.0);
            let mut seed = 314u64;
            (0..500)
                .map(|i| m.material_at(Position::new(i as f64 * 0.02, 0.0, 0.0), &mut seed).unwrap())
                .collect::<Vec<_>>()
        };
        assert_eq!(run(), run());
    }
}
