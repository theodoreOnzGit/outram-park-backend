//! **Delta (Woodcock) tracking**: everything the crate does to move a neutron
//! by rejection against a majorant instead of by finding surfaces.
//!
//! NEW WORK, not a port: OpenMC is pure surface tracking (a search of its
//! `src/`, `include/` and `openmc/` trees at `608a1c338` for "delta
//! tracking", "woodcock" and "majorant" finds nothing). The method is
//! Woodcock et al., "Techniques used in the GEM code for Monte Carlo
//! neutronics calculations in reactors and other systems of complex
//! geometry", ANL-7050 (1965), in the form Serpent uses: J. Leppänen,
//! "Performance of Woodcock delta-tracking in lattice physics applications
//! using the Serpent Monte Carlo reactor physics burnup calculation code",
//! Ann. Nucl. Energy 37 (2010) 715-722.
//!
//! # The algorithm
//!
//! ```text
//! at energy E, in a region whose materials are bounded by Σ_maj(E):
//!   loop
//!     s ← −ln ξ₁ / Σ_maj(E)            flight on the majorant
//!     r ← r + s·u                      no surface is looked for
//!     if r left the region: stop on its boundary, hand back
//!     m ← material at r                a point query, not a surface search
//!     if ξ₂ < Σ_t,m(E)/Σ_maj(E): real collision in m at r
//!     else: virtual collision, nothing happens, continue
//! ```
//!
//! # The invariants it rests on
//!
//! 1. **Majorant bound**: `Σ_maj(E) ≥ Σ_t,m(E)` for every material the region
//!    can present, at the temperature and density it has now
//!    ([`majorant`]). Under it the real collision sites have exactly the
//!    surface-tracking distribution, so delta tracking is **unbiased**; a
//!    loose bound costs only virtual collisions. Where it fails, the excess
//!    collisions are silently lost.
//! 2. **Memorylessness**: stopping a flight anywhere (a region boundary, the
//!    nearest surface) and re-sampling from there leaves the distribution of
//!    real collision sites unchanged. This is what makes a hand-off to and
//!    from surface tracking exact ([`handoff`]).
//! 3. **Energy is constant in flight**: `Σ_maj(E)` is read once per flight
//!    and again after every real collision that changes `E`.
//! 4. **Scoring draws nothing**: the tentative-collision tally hook consumes
//!    no random number, so tallies never perturb the histories.
//!
//! # Map
//!
//! - [`majorant`] — [`Majorant`] construction (bound by construction on
//!   pointwise, S(α,β) and multigroup data; sampled on multipole and URR
//!   band totals), its 1/v extrapolation below the grid, and
//!   [`Majorant::audit`].
//! - [`flight`] — the flight loop [`flight::fly`], its region contract
//!   [`flight::DeltaRegion`], and the public primitives built on it
//!   ([`bounded_delta_flight_visiting`] and family, [`track_to_collision`],
//!   [`sample_delta_distance`], [`classify_collision`]).
//! - [`handoff`] — the hybrid CSG driver's flight through a delta region,
//!   its hand-off to and from surface tracking, and the tally estimators
//!   valid inside a delta region ([`DeltaTallyEstimator`]).
//!
//! The drivers: [`crate::physics::transport_csg::run_keff_csg_hybrid`]
//! (delta tracking per region, surface tracking elsewhere) and
//! [`crate::pebble_beds::keff_delta`] (a whole reflective or vacuum domain,
//! delta-tracked throughout).
//!
//! Gathered here from `pebble_beds::delta_tracking` and
//! `physics::transport_csg` on 2026-10-06 (gh:#599 audit, readability
//! refactor). `pebble_beds::delta_tracking` re-exports everything at its old
//! path; behaviour is bit-identical (`tests/delta_tracking_bit_identity.rs`).

pub mod flight;
pub mod handoff;
pub mod majorant;

pub use flight::{
    bounded_delta_flight, bounded_delta_flight_urr, bounded_delta_flight_visiting,
    classify_collision, fly, sample_delta_distance, track_to_collision, Advance, BoundedRay,
    DeltaEvent, DeltaFlight, DeltaRegion, DeltaStep, FlightEnd, IntoSiteContent, SiteContent,
    SiteTotal, TentativeSite,
};
pub use handoff::DeltaTallyEstimator;
pub use majorant::{bounding_audit_line, Majorant, MajorantAudit};
