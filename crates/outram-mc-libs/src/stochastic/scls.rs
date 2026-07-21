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
//! # What is NOT implemented here
//!
//! The SCLS **transport driver**: promoting a freshly sampled chord into a stored
//! history with a consistent centre, and coupling the whole thing to the k-eigenvalue
//! loop. Until that exists [`SclsMedium::material_at`] returns
//! [`MediumError::NotImplemented`] rather than a half-answer. Tracked as bead
//! `op-eby.3`; the adaptive-radius extension is `op-eby.6`.
//!
//! **No accuracy claim is made here.** Whether SCLS actually recovers the explicit-RSA
//! answer is an empirical question that the benchmark suite (bead `op-eby.7`) must
//! measure before anything in this module may be described as validated.
//!
//! This module is **new work**, not an OpenMC port — upstream has no SCLS.

use crate::geometry::position::Position;
use crate::stochastic::medium::{MaterialId, MediumError};

/// Squared distance \[cm²\] between two points.
fn dist_sq(a: Position, b: Position) -> f64 {
    let dx = a.x - b.x;
    let dy = a.y - b.y;
    let dz = a.z - b.z;
    dx * dx + dy * dy + dz * dz
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

    /// Point membership — **not implemented**.
    ///
    /// Retained geometry alone cannot answer this (see [`Self::retained_material_at`]),
    /// and the sampling fallback is the not-yet-built SCLS driver. Returns
    /// [`MediumError::NotImplemented`] rather than reporting "matrix" for every
    /// unremembered point, which would silently bias every result toward the matrix.
    pub fn material_at(
        &mut self,
        _position: Position,
        _seed: &mut u64,
    ) -> Result<MaterialId, MediumError> {
        Err(MediumError::NotImplemented("SCLS"))
    }
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

    /// Point membership is honestly reported as unimplemented.
    #[test]
    fn material_at_reports_not_implemented() {
        let mut m = medium();
        let mut seed = 1u64;
        assert_eq!(
            m.material_at(Position::new(0.0, 0.0, 0.0), &mut seed),
            Err(MediumError::NotImplemented("SCLS"))
        );
    }
}
