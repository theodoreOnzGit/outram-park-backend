//! **The hand-off between surface tracking and a delta-tracked region** in the
//! hybrid CSG driver ([`crate::physics::transport_csg::run_keff_csg_hybrid`]),
//! and the tally estimators that work inside such a region.
//!
//! NEW WORK, no OpenMC counterpart (OpenMC is pure surface tracking).
//!
//! # Where the hand-off happens
//!
//! Every flight of the CSG history loop starts with `Geometry::locate`, which
//! reports the tracking method in force at the particle
//! (`GeometryPath::tracking`: the deepest `Cell::delta_tracked` /
//! `surface_tracked` declaration on the path). The ONLY thing that differs
//! between the two methods is how far the particle goes to its next real
//! collision and in which material; the collision physics, the boundary
//! crossing and the leak accounting downstream are shared. So the hand-off is
//! one function, [`fly_delta_region`], called in place of the surface-tracking
//! distance sample, which answers with a [`RegionFlight`]:
//!
//! - **into** a delta region: the particle is wherever `locate` found it,
//!   with its own energy and weight; nothing is converted;
//! - **inside**: [`super::flight::fly`] samples tentative sites on `Σ_maj(E)`
//!   and accepts them with `Σ_t/Σ_maj`;
//! - **out of** the region, or past the nearest surface: the particle streams
//!   to the nearest surface (`d_surface`) and crosses it with the ordinary
//!   surface-crossing code, then the next `locate` decides again.
//!
//! # A fact the estimators rely on: flights stop at the NEAREST surface
//!
//! The flight is sampled to the region's own exit
//! (`GeometryExt::distance_out_of_level` at the tracking level), but the
//! particle advances at most to `d_surface`, the nearest surface over **every**
//! coordinate level (`Geometry::distance_to_boundary`). A real collision past
//! it is discarded and re-sampled from the surface on the next flight
//! (memoryless, so unbiased). Two consequences:
//!
//! - every **scored** tentative site lies in the flight's starting leaf cell,
//!   so binning a `Cell`, `Universe` or distribcell filter by the starting
//!   cell is exact (`tests/delta_tracking_audit.rs`,
//!   `cell_filter_in_a_delta_region_matches_surface_track_length`, gh:#599);
//! - the hybrid driver **does not skip internal surfaces**: it stops at each
//!   one as surface tracking does and pays the virtual collisions on top.
//!   Measured 2.5x slower than surface tracking on a three-shell sphere
//!   (gh:#599 audit). Delta tracking here is correct, not fast.
//!
//! # Tally estimators inside a delta region (gh:#598)
//!
//! A track-length estimator is not available: a delta flight is not a segment
//! through one material. Two collision estimators are, chosen by
//! [`DeltaTallyEstimator`]:
//!
//! - [`DeltaTallyEstimator::TentativeCollision`] (default, Serpent's,
//!   Leppänen 2010): `w/Σ_maj` (flux) and `w·Σ_x/Σ_maj` (rates) at every
//!   tentative site, virtual and real, before `d_surface` — scored by
//!   [`fly_delta_region`]'s visitor;
//! - [`DeltaTallyEstimator::RealCollision`] (ablation, OpenMC's collision
//!   estimator `w/Σ_t` at real collisions) — [`score_real_collision`].
//!
//! Neither draws a random number, so `k` is bit-identical between them.
//!
//! ## History of this estimator, kept because it shaped decisions
//!
//! - `bn:op-ra9f`: the track-length estimator was first applied here against
//!   `path.material` (the material at the flight's START). Measured on the
//!   HTR-10 bed, 8-group: the tallied `k_inf` fell 4175 pcm below the run's
//!   own `k_eff/(1-L)` balance. Replaced by the real-collision estimator.
//! - ~~"The collision estimator is what OpenMC and Serpent use in
//!   delta-tracked regions"~~ **CORRECTED 2026-10-06 (gh:#598):** OpenMC has
//!   no delta tracking; Serpent scores at every tentative collision.
//! - gh:#598 defect: the real-collision score was passed through
//!   `score_track_length` with `1/Σ_t` as a "length"; an unstructured mesh
//!   filter split it across cells and dropped helium's `1/Σ_t ~ 5e4 cm`
//!   almost wholly outside the mesh. `score_collision_point` bins at the site.
//! - gh:#598, the guard on `d_surface`: scoring tentative sites past the
//!   nearest surface counted that stretch twice (it is re-flown from the
//!   surface). Measured before the guard: region `Σ_t` −1.9 % (15 σ) against
//!   surface track length (`tests/delta_collision_estimator.rs`).

use crate::geometry::cell::SurfaceToken;
use crate::geometry::crossing::GeometryExt;
use crate::geometry::geometry::{Geometry, GeometryPath};
use crate::geometry::position::{stream, Direction, Position};
use crate::material::material::Material;
use crate::material::nuclide::Nuclide;
use crate::tally::scoring::score_collision_point;
use crate::tally::tally::Tally;

use super::flight::{fly, BoundedRay, DeltaStep, SiteContent, SiteTotal, TentativeSite};
use super::majorant::Majorant;

/// **Which collision estimator a tally uses inside a delta-tracked region**
/// (gh:#598). A track-length estimator is not available there (see the
/// module docs).
///
/// NEW WORK: OpenMC has no delta tracking. Serpent, which does, scores its
/// collision flux at every tentative collision (Leppänen 2010, cited at
/// [`super::flight::bounded_delta_flight_visiting`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DeltaTallyEstimator {
    /// **The default.** At every tentative collision site, virtual and real,
    /// score `w/Σ_maj` (flux) and `w·Σ_x(r)/Σ_maj` (reaction rates), with
    /// `Σ_x` of the material AT the site. The sites have density
    /// `φ·Σ_maj` everywhere in the region, so this samples the near-void
    /// helium between pebbles as often as the graphite. Costs one
    /// `Material::macro_xs` per tentative site while a tally is attached.
    #[default]
    TentativeCollision,
    /// **Ablation**: `w/Σ_t` and `w·Σ_x/Σ_t` at REAL collisions only, as
    /// OpenMC's collision estimator does (`tally_scoring.cpp`,
    /// `score_general_ce_nonanalog`). Unbiased, but where `Σ_t ≪ Σ_maj`
    /// (helium, `Σ_t/Σ_maj ~ 5e-5`) its support is almost never sampled, so
    /// a finite run sees a void's flux as rare enormous scores or not at all.
    RealCollision,
}

/// Virtual-collision budget for a delta-tracked region before the history is
/// declared lost. Matches `keff_delta.rs`'s `MAX_VIRTUAL` so the two paths
/// agree on what counts as a stuck history.
pub(crate) const MAX_VIRTUAL_COLLISIONS: u32 = 100_000;

/// The particle at the start of a flight: everything the region flight and
/// its scoring hook read, by value.
#[derive(Debug, Clone, Copy)]
pub(crate) struct FlightStart {
    /// Position \[cm\].
    pub r: Position,
    /// Direction.
    pub u: Direction,
    /// Energy \[eV\], constant over the flight.
    pub e: f64,
    /// Statistical weight.
    pub w: f64,
    /// Particle clock at the start of the flight \[s\].
    pub time_s: f64,
}

/// The tally bins a flight's scores land in that do not depend on the site:
/// the starting leaf cell, its universe and its distribcell instance. Exact
/// for every scored site (see the module docs).
#[derive(Debug, Clone, Copy)]
pub(crate) struct StartBins {
    /// Leaf cell index.
    pub cell: usize,
    /// Leaf universe index.
    pub universe: usize,
    /// Distribcell instance, if a filter needs one.
    pub instance: Option<usize>,
}

/// How a flight through a delta region ended, for the CSG history loop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum RegionFlightEnd {
    /// A real collision before the nearest surface: stream `distance` along
    /// the direction and collide in `material`.
    Collision {
        /// Distance from the flight's start \[cm\].
        distance: f64,
        /// The material at the collision site.
        material: usize,
    },
    /// No real collision before the nearest surface, or the region ended:
    /// stream to the nearest surface and cross it.
    StreamToSurface,
    /// The history is lost (virtual budget exhausted, or a site the geometry
    /// could not place). The caller scores it as a leak.
    Lost,
}

/// [`RegionFlightEnd`] and what the flight cost.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct RegionFlight {
    /// How it ended.
    pub end: RegionFlightEnd,
    /// Virtual collisions rejected: the measured price of the majorant.
    pub virtual_collisions: u32,
    /// Tentative sites where `Σ_t > Σ_maj` (gh:#721); must be zero.
    pub majorant_violations: u32,
}

/// Distance travelled from `r` along `u` to `p` \[cm\] (the projection, so a
/// point on the ray gives its path length).
#[inline]
fn travelled(r: Position, u: Direction, p: Position) -> f64 {
    (p.x - r.x) * u.u + (p.y - r.y) * u.v + (p.z - r.z) * u.w
}

/// **One flight of the hybrid driver inside a delta-tracked region.**
///
/// - `path` — the located path at the flight's start; its
///   `tracking_level` names the region.
/// - `majorant` — that region's majorant.
/// - `d_surface` — distance to the nearest surface over every level.
/// - `tentative` — `Some((tally, batch))` to score the tentative-collision
///   estimator into `batch`; `None` scores nothing.
/// - `seed`, `urr_seed` — the transport stream (advanced) and the history's
///   URR stream (read).
#[allow(clippy::too_many_arguments)]
pub(crate) fn fly_delta_region(
    geom: &Geometry,
    path: &GeometryPath,
    majorant: &Majorant,
    materials: &[Material],
    nuclides: &[Nuclide],
    p: FlightStart,
    d_surface: f64,
    bins: StartBins,
    tentative: Option<(&Tally, &mut [f64])>,
    seed: &mut u64,
    urr_seed: u64,
) -> RegionFlight {
    let FlightStart { r, u, e, w, time_s } = p;
    // The region's OWN extent, not the nearest surface -- a bed is full of
    // internal surfaces the tracker exists to cross.
    let exit_at = geom.distance_out_of_level(path, path.tracking_level);
    let (tally, batch) = match tentative {
        Some((t, b)) => (Some(t), b),
        None => (None, &mut [][..]),
    };
    // A non-positive majorant: nothing in the region can interact at this
    // energy, so the particle streams to the nearest surface (gh:#722 note:
    // no tentative site exists, so the tentative estimator scores nothing).
    let maj = majorant.at(e);
    if !(maj > 0.0) {
        return RegionFlight {
            end: RegionFlightEnd::StreamToSurface,
            virtual_collisions: 0,
            majorant_violations: 0,
        };
    }
    // The tentative-collision scoring hook. ONLY sites before `d_surface` are
    // scored (module docs); it draws no random number. A void site scores its
    // flux `w/Σ_maj` with no material, as surface tracking's track length
    // scores a void cell.
    let mut majorant_violations = 0_u32;
    let visit = |site: TentativeSite| {
        // Counted, never clamped silently (gh:#721). Draws nothing.
        if site.violates_majorant() {
            majorant_violations += 1;
        }
        let Some(t) = tally else { return };
        if !(site.majorant > 0.0) {
            return;
        }
        let d = travelled(r, u, site.position);
        if !(d < d_surface) {
            return;
        }
        let mxs = site.material.map(|m| materials[m].macro_xs(e, nuclides));
        score_collision_point(
            batch,
            t,
            bins.cell,
            site.material.unwrap_or(usize::MAX),
            bins.universe,
            e,
            1.0 / site.majorant,
            site.position,
            mxs.as_ref(),
            w,
            bins.instance,
            time_s + crate::physics::transport_csg::flight_time(d, e),
            u,
        );
    };
    let region = BoundedRay {
        // `exit_at` is measured from `r`; convert a probe point back to the
        // remaining distance along the ray.
        distance_to_exit: |q: Position, _d: Direction| exit_at - travelled(r, u, q),
        // A void cell is a site with `Σ_t = 0`, always virtual. ~~`None`, i.e.
        // a lost history~~ until gh:#719 (2026-10-06): every neutron that
        // sampled a site in a void cell was scored as a leak.
        material_at: |q: Position| match geom.locate(q, u, SurfaceToken::NONE) {
            None => SiteContent::Lost,
            Some(located) => located
                .material
                .map_or(SiteContent::Void, SiteContent::Material),
        },
    };
    let end = fly(
        r,
        u,
        e,
        maj,
        &region,
        materials,
        nuclides,
        SiteTotal::UrrBand(urr_seed),
        MAX_VIRTUAL_COLLISIONS,
        seed,
        visit,
    );
    let step = DeltaStep::from(end);
    match step {
        DeltaStep::Collision {
            position,
            material,
            virtual_collisions,
            ..
        } => {
            let distance = travelled(r, u, position);
            RegionFlight {
                // A real collision past the nearest surface is discarded: the
                // particle stops on the surface and the next flight re-samples
                // (memoryless, so unbiased).
                end: if distance < d_surface {
                    RegionFlightEnd::Collision { distance, material }
                } else {
                    RegionFlightEnd::StreamToSurface
                },
                virtual_collisions,
                majorant_violations,
            }
        }
        // Left the region: stream to the nearest surface and let the
        // enclosing method take over on the next `locate`.
        DeltaStep::Exit {
            virtual_collisions, ..
        } => RegionFlight {
            end: RegionFlightEnd::StreamToSurface,
            virtual_collisions,
            majorant_violations,
        },
        DeltaStep::Exhausted { virtual_collisions } => RegionFlight {
            end: RegionFlightEnd::Lost,
            virtual_collisions,
            majorant_violations,
        },
    }
}

/// **The real-collision estimator** (`DeltaTallyEstimator::RealCollision`,
/// the ablation): `w/Σ_t` and `w·Σ_x/Σ_t` at a real collision `distance`
/// along the flight in `material`, binned at the site (OpenMC's collision
/// estimator, `tally_scoring.cpp`, `score_general_ce_nonanalog`). Unbiased,
/// but where `Σ_t ≪ Σ_maj` (helium, `Σ_t/Σ_maj ~ 5e-5`) its support is almost
/// never sampled.
#[allow(clippy::too_many_arguments)]
pub(crate) fn score_real_collision(
    tally: &Tally,
    batch: &mut [f64],
    materials: &[Material],
    nuclides: &[Nuclide],
    p: FlightStart,
    bins: StartBins,
    distance: f64,
    material: usize,
) {
    let FlightStart { r, u, e, w, time_s } = p;
    let mxs = materials[material].macro_xs(e, nuclides);
    if mxs.total > 0.0 {
        score_collision_point(
            batch,
            tally,
            bins.cell,
            material,
            bins.universe,
            e,
            1.0 / mxs.total,
            stream(r, u, distance),
            Some(&mxs),
            w,
            bins.instance,
            time_s + crate::physics::transport_csg::flight_time(distance, e),
            u,
        );
    }
}
