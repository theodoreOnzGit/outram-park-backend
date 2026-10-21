//! Woodcock (delta) tracking — **moved to [`crate::physics::delta_tracking`]**
//! on 2026-10-06 (gh:#599 audit, readability refactor).
//!
//! Everything that lived here (the [`Majorant`] and its constructors, the
//! flight primitives, [`bounded_delta_flight_visiting`] and its family) is
//! re-exported at this path unchanged, so existing callers compile and run
//! bit-identically (`tests/delta_tracking_bit_identity.rs`). New code should
//! name `physics::delta_tracking`, whose module docs state the algorithm, its
//! invariants and its references (Woodcock 1965; Leppänen 2010).

pub use crate::physics::delta_tracking::flight::{
    bounded_delta_flight, bounded_delta_flight_urr, bounded_delta_flight_visiting,
    classify_collision, sample_delta_distance, track_to_collision, DeltaEvent, DeltaFlight,
    DeltaStep,
};
pub use crate::physics::delta_tracking::majorant::{bounding_audit_line, Majorant, MajorantAudit};
