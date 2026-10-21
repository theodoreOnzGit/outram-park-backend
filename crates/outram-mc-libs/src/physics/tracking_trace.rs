//! **Step-level tracking events**: what surface tracking and delta (Woodcock)
//! tracking each do on the way from one collision to the next, reported one
//! step at a time (gh:#784).
//!
//! NEW WORK, no OpenMC counterpart. OpenMC's nearest equivalent is its track
//! file (`TrackState`, ported as [`crate::physics::track_output`]), which
//! records where a particle *was* at each event but not the questions the
//! tracker asked to get there: which random number set the flight, how far
//! the nearest surface was, whether a tentative collision was real. The
//! Monte Carlo tutorial's rung-5 demo contrasts the two tracking methods step
//! by step, and those questions are the contrast.
//!
//! # How it attaches
//!
//! An **observer**: a closure `FnMut(TraceEvent)` passed to the `*_traced`
//! entry points ([`crate::physics::delta_tracking::flight::fly_traced`],
//! [`crate::pebble_beds::keff_delta::trace_delta_history`] and
//! [`crate::pebble_beds::keff_delta::DeltaPowerIteration::step_traced`],
//! [`crate::physics::transport_csg::trace_csg_history`] and
//! [`crate::physics::transport_csg::CsgPowerIteration::step_traced`]). It is
//! generic, never `dyn` (the workspace rule), and every untraced entry point
//! passes a no-op, which the compiler removes.
//!
//! **Observing draws no random number and changes nothing**: a traced run
//! takes exactly the histories an untraced one takes, bit for bit. That is
//! pinned by `tests/tracking_trace.rs` and by the crate's existing
//! bit-identity gates (`delta_tracking_bit_identity.rs`,
//! `variance_reduction_is_bit_identical_when_analog.rs`), which run through
//! the same, now observer-carrying, functions.
//!
//! # The two event sequences
//!
//! ```text
//! surface tracking, per flight segment:
//!   Located   (Geometry::locate: which cell, which material, Σ_t)
//!   Segment   (ξ, d_col = −ln ξ / Σ_t, distance_to_boundary, which surface)
//!   then either State(SurfaceCrossing)    d_col ≥ d_boundary: move to it, cross
//!          or   Collision + State(...)    d_col < d_boundary: collide
//!
//! delta tracking, per flight on the majorant:
//!   Flight    (ξ, s = −ln ξ / Σ_maj; no surface is looked for)
//!   Tentative (material at the landing point, Σ_t/Σ_maj, ξ, real or virtual)
//!   after a real one: Collision + State(...)
//! ```

use crate::geometry::position::{Direction, Position};
use crate::physics::delta_tracking::flight::TentativeSite;
use crate::physics::track_output::{TrackEvent, TrackState};

/// One step of a tracked particle, as surface or delta tracking took it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TraceEvent {
    /// A particle begins: a source neutron, or a secondary popped from the
    /// history's own stack.
    Start {
        /// Position \[cm\].
        r: Position,
        /// Direction.
        u: Direction,
        /// Energy \[eV\].
        e: f64,
    },
    /// **Surface tracking**: one `Geometry::locate`, placing the particle in
    /// a cell, and the total cross section of what fills it.
    Located {
        /// Where the particle was located \[cm\].
        r: Position,
        /// Leaf cell index.
        cell: usize,
        /// Its material; `None` for a void cell.
        material: Option<usize>,
        /// `Σ_t` \[cm⁻¹\] there at the particle's energy (`0` in a void).
        sigma_t: f64,
    },
    /// **Surface tracking**: one flight segment's two distances. The
    /// segment ends at the nearer of the two.
    Segment {
        /// The uniform variate the collision distance was sampled from;
        /// `None` in a void, where no variate is drawn.
        xi: Option<f64>,
        /// `−ln ξ / Σ_t` \[cm\] (`∞` in a void).
        d_collision: f64,
        /// `Geometry::distance_to_boundary` \[cm\]: the nearest surface (or
        /// lattice tile edge) along the flight.
        d_boundary: f64,
        /// The surface index that distance belongs to; `None` for a lattice
        /// crossing or no surface ahead.
        surface: Option<usize>,
    },
    /// **Delta tracking**: one flight sampled on the majorant. No surface is
    /// looked for; the flight may reflect off a reflective domain's walls on
    /// the way (the arrival direction is in the following event's state).
    Flight {
        /// Where the flight started \[cm\].
        from: Position,
        /// Direction it started along.
        u: Direction,
        /// Where it landed \[cm\] (or the region boundary, see `exited`).
        to: Position,
        /// Its uniform variate.
        xi: f64,
        /// `s = −ln ξ / Σ_maj` \[cm\].
        distance: f64,
        /// `Σ_maj(E)` \[cm⁻¹\].
        majorant: f64,
        /// The region ended before the flight did; the particle stopped on
        /// its boundary and no tentative collision follows.
        exited: bool,
    },
    /// **Delta tracking**: one tentative collision site, classified.
    Tentative {
        /// The site: position, material, `Σ_maj`, and the `Σ_t` the
        /// accept/reject step read.
        site: TentativeSite,
        /// The accept/reject variate; `None` in a void, where the site is
        /// virtual without a draw.
        xi: Option<f64>,
        /// `true` if `ξ < Σ_t/Σ_maj`: a real collision.
        real: bool,
    },
    /// **Both**: a real collision, with the nuclide the collision chose.
    Collision {
        /// Where \[cm\].
        r: Position,
        /// Material index.
        material: usize,
        /// Nuclide index (into the run's nuclide list).
        nuclide: usize,
        /// Incident energy \[eV\].
        e: f64,
    },
    /// **Both**: an outcome, as the track file would record it (a scatter
    /// with the outgoing energy and direction, a fission, a capture, a
    /// surface crossing with the state after it, a leak, a lost history).
    State(TrackState),
}

/// The no-op observer every untraced entry point passes.
#[inline(always)]
pub fn no_trace(_: TraceEvent) {}

/// Running counts of [`TraceEvent`]s: the per-method cost counters of the
/// rung-5 demo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TraceCounts {
    /// Particles started (sources and secondaries).
    pub starts: u64,
    /// Surface tracking: `Geometry::locate` calls.
    pub locates: u64,
    /// Surface tracking: distance-to-boundary queries (one per segment).
    pub boundary_queries: u64,
    /// Surface tracking: surfaces crossed (reflections included).
    pub crossings: u64,
    /// Delta tracking: flights sampled on the majorant.
    pub flights: u64,
    /// Delta tracking: tentative collision sites (real and virtual).
    pub tentative: u64,
    /// Delta tracking: virtual (rejected) collisions.
    pub virtual_collisions: u64,
    /// Delta tracking: sites where `Σ_t > Σ_maj`, i.e. where the majorant
    /// failed to bound and collisions were silently lost.
    pub majorant_violations: u64,
    /// Real collisions (both methods).
    pub collisions: u64,
    /// Histories ending in fission.
    pub fissions: u64,
    /// Histories ending in capture.
    pub captures: u64,
    /// Histories ending in a leak or a lost particle.
    pub leaked_or_lost: u64,
}

impl TraceCounts {
    /// Count one event.
    pub fn add(&mut self, ev: &TraceEvent) {
        match ev {
            TraceEvent::Start { .. } => self.starts += 1,
            TraceEvent::Located { .. } => self.locates += 1,
            TraceEvent::Segment { .. } => self.boundary_queries += 1,
            TraceEvent::Flight { .. } => self.flights += 1,
            TraceEvent::Tentative { site, real, .. } => {
                self.tentative += 1;
                if !real {
                    self.virtual_collisions += 1;
                }
                if site.violates_majorant() {
                    self.majorant_violations += 1;
                }
            }
            TraceEvent::Collision { .. } => self.collisions += 1,
            TraceEvent::State(s) => match s.event {
                TrackEvent::SurfaceCrossing => self.crossings += 1,
                TrackEvent::Fission => self.fissions += 1,
                TrackEvent::Absorption => self.captures += 1,
                TrackEvent::Leak | TrackEvent::Lost => self.leaked_or_lost += 1,
                TrackEvent::Born | TrackEvent::Scatter | TrackEvent::Rouletted => {}
            },
        }
    }

    /// The geometry and sampling work, in "stops": a surface tracker stops
    /// at every segment end (a collision or a surface), a delta tracker at
    /// every tentative site. The comparable step count of the two methods.
    pub fn stops(&self) -> u64 {
        self.boundary_queries + self.tentative
    }

    /// Add another set of counts to this one.
    pub fn merge(&mut self, o: &TraceCounts) {
        self.starts += o.starts;
        self.locates += o.locates;
        self.boundary_queries += o.boundary_queries;
        self.crossings += o.crossings;
        self.flights += o.flights;
        self.tentative += o.tentative;
        self.virtual_collisions += o.virtual_collisions;
        self.majorant_violations += o.majorant_violations;
        self.collisions += o.collisions;
        self.fissions += o.fissions;
        self.captures += o.captures;
        self.leaked_or_lost += o.leaked_or_lost;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(event: TrackEvent) -> TraceEvent {
        TraceEvent::State(TrackState {
            r: Position::ZERO,
            u: Direction::new(1.0, 0.0, 0.0),
            energy: 1.0,
            time: 0.0,
            weight: 1.0,
            cell: 0,
            material: None,
            event,
        })
    }

    #[test]
    fn counts_classify_every_event_kind() {
        let site = |sigma_t: f64| TentativeSite {
            position: Position::ZERO,
            material: Some(0),
            majorant: 1.0,
            sigma_t,
        };
        let evs = [
            TraceEvent::Start {
                r: Position::ZERO,
                u: Direction::new(1.0, 0.0, 0.0),
                e: 1.0,
            },
            TraceEvent::Located {
                r: Position::ZERO,
                cell: 0,
                material: None,
                sigma_t: 0.0,
            },
            TraceEvent::Segment {
                xi: None,
                d_collision: f64::INFINITY,
                d_boundary: 1.0,
                surface: Some(0),
            },
            state(TrackEvent::SurfaceCrossing),
            TraceEvent::Flight {
                from: Position::ZERO,
                u: Direction::new(1.0, 0.0, 0.0),
                to: Position::ZERO,
                xi: 0.5,
                distance: 0.7,
                majorant: 1.0,
                exited: false,
            },
            TraceEvent::Tentative {
                site: site(0.2),
                xi: Some(0.9),
                real: false,
            },
            TraceEvent::Tentative {
                site: site(1.5),
                xi: Some(0.1),
                real: true,
            },
            TraceEvent::Collision {
                r: Position::ZERO,
                material: 0,
                nuclide: 0,
                e: 1.0,
            },
            state(TrackEvent::Scatter),
            state(TrackEvent::Fission),
            state(TrackEvent::Absorption),
            state(TrackEvent::Lost),
        ];
        let mut c = TraceCounts::default();
        for e in &evs {
            c.add(e);
        }
        assert_eq!(
            c,
            TraceCounts {
                starts: 1,
                locates: 1,
                boundary_queries: 1,
                crossings: 1,
                flights: 1,
                tentative: 2,
                virtual_collisions: 1,
                majorant_violations: 1,
                collisions: 1,
                fissions: 1,
                captures: 1,
                leaked_or_lost: 1,
            }
        );
        assert_eq!(c.stops(), 3);
        let mut d = c;
        d.merge(&c);
        assert_eq!(d.tentative, 4);
        assert_eq!(d.leaked_or_lost, 2);
        no_trace(evs[0]);
    }
}
