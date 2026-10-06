//! **The delta-tracking flight**: sample a distance on the majorant, move,
//! accept or reject on `Σ_t/Σ_maj`, and stop at the region boundary.
//!
//! [`fly`] is the one implementation of that loop. The hybrid CSG driver
//! reaches it through [`bounded_delta_flight_visiting`] (a [`BoundedRay`]
//! region) and [`super::handoff`]; the reflective-domain drivers of
//! [`crate::pebble_beds::keff_delta`] through their own [`DeltaRegion`].
//! [`track_to_collision`] is the minimal teaching form, kept for its callers.
//!
//! Moved here from `pebble_beds::delta_tracking` on 2026-10-06 (gh:#599
//! audit); the old path re-exports everything.

use crate::geometry::position::{stream, Direction, Position};
use crate::material::material::Material;
use crate::material::nuclide::Nuclide;
use crate::mathf::RealMath;
use crate::rng::lcg::prn;

use super::majorant::Majorant;

/// The outcome of one delta-tracking flight segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeltaEvent {
    /// A physical interaction — sample the actual reaction here.
    Real,
    /// A virtual (delta) collision — no physics; continue the flight.
    Virtual,
}

/// Sample a delta-tracking flight distance \[cm\] from an exponential on the
/// majorant: `s = −ln ξ / Σ_maj`.
///
/// This is the ordinary free-flight sample, but on the majorant rather than the
/// local Σ_t, which is what lets the neutron cross material boundaries without
/// stopping at them. `majorant` is Σ_maj \[cm⁻¹\] at the current energy; a
/// non-positive majorant yields an infinite flight (no collisions possible).
pub fn sample_delta_distance(majorant: f64, seed: &mut u64) -> f64 {
    if majorant <= 0.0 {
        return f64::INFINITY;
    }
    -prn(seed).max(f64::MIN_POSITIVE).r_ln() / majorant
}

/// Decide whether a delta-tracking collision is real or virtual by rejection on
/// the ratio `Σ_t(local)/Σ_maj`.
///
/// Accepts a [`DeltaEvent::Real`] with probability `sigma_t_local / majorant`
/// (the physical collision), else [`DeltaEvent::Virtual`]. `sigma_t_local` is the
/// true macroscopic total \[cm⁻¹\] of the material at the landing point;
/// `majorant` is the Σ_maj the flight was sampled on. A `majorant ≤ 0` degenerates
/// to `Virtual`. The caller must guarantee `sigma_t_local ≤ majorant` (that is the
/// majorant's whole contract); if it is violated the ratio saturates at 1 (always
/// real), which is the safe direction.
pub fn classify_collision(sigma_t_local: f64, majorant: f64, seed: &mut u64) -> DeltaEvent {
    if majorant <= 0.0 {
        return DeltaEvent::Virtual;
    }
    let p_real = (sigma_t_local / majorant).clamp(0.0, 1.0);
    if prn(seed) < p_real {
        DeltaEvent::Real
    } else {
        DeltaEvent::Virtual
    }
}

/// Where a delta-tracking flight ended.
#[derive(Debug, Clone, Copy)]
pub struct DeltaFlight {
    /// Position \[cm\] of the real collision (or of leakage — see `escaped`).
    pub position: Position,
    /// Total path length \[cm\] flown, including all virtual-collision segments.
    pub distance: f64,
    /// Number of virtual (delta) collisions rejected before the real one.
    pub virtual_collisions: u32,
    /// `true` if the neutron left the tracking region before a real collision
    /// (the `sigma_t_local` lookup returned `None`).
    pub escaped: bool,
}

/// Drive a neutron from `start` along `direction` to its next **real** collision by
/// delta tracking, looping over virtual collisions internally.
///
/// At each step it samples a flight on `majorant.at(energy)`, advances, then asks
/// the caller "what is Σ_t at this point?" via `sigma_t_at`. That closure returns
/// `Some(sigma_t)` for a point inside the tracking region (looking up whichever
/// material — fuel kernel, matrix, pebble, coolant — actually occupies the point)
/// or `None` if the neutron has left the region (leakage). A real collision ends
/// the loop; a virtual one continues it.
///
/// This is deliberately generic over the geometry lookup (`impl Fn`, no trait
/// object) so the doubly-heterogeneous machinery — lattice/universe descent or a
/// stochastic-media membership test — supplies `sigma_t_at` without this core
/// depending on it.
///
/// # Parameters
/// - `start` / `direction` — the neutron's phase-space point.
/// - `energy` — incident energy \[eV\] (constant along the flight; scattering
///   changes it *after* a real collision, in the caller's transport loop).
/// - `majorant` — the delta-tracking bound (see [`Majorant`]).
/// - `max_virtual` — safety cap on virtual collisions before giving up (returns
///   `escaped = true`); guards against a pathologically loose majorant.
/// - `sigma_t_at` — local total Σ_t \[cm⁻¹\] lookup, `None` outside the region.
pub fn track_to_collision<F>(
    start: Position,
    direction: Direction,
    energy: f64,
    majorant: &Majorant,
    max_virtual: u32,
    seed: &mut u64,
    sigma_t_at: F,
) -> DeltaFlight
where
    F: Fn(Position) -> Option<f64>,
{
    let maj = majorant.at(energy);
    let mut pos = start;
    let mut total = 0.0;
    let mut virtuals = 0u32;

    loop {
        let s = sample_delta_distance(maj, seed);
        pos = pos + Position::new(direction.u * s, direction.v * s, direction.w * s);
        total += s;

        match sigma_t_at(pos) {
            None => {
                return DeltaFlight {
                    position: pos,
                    distance: total,
                    virtual_collisions: virtuals,
                    escaped: true,
                };
            }
            Some(sigma_t) => match classify_collision(sigma_t, maj, seed) {
                DeltaEvent::Real => {
                    return DeltaFlight {
                        position: pos,
                        distance: total,
                        virtual_collisions: virtuals,
                        escaped: false,
                    };
                }
                DeltaEvent::Virtual => {
                    virtuals += 1;
                    if virtuals >= max_virtual {
                        return DeltaFlight {
                            position: pos,
                            distance: total,
                            virtual_collisions: virtuals,
                            escaped: true,
                        };
                    }
                }
            },
        }
    }
}

// ---------------------------------------------------------------------------
// THE FLIGHT LOOP — one implementation, used by every delta-tracked driver
// ---------------------------------------------------------------------------

/// Where one sampled flight segment took the particle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Advance {
    /// The segment ended inside the region, at a **tentative collision site**.
    /// `direction` is the direction after the segment, which a reflecting
    /// domain ([`crate::pebble_beds::keff_delta::DeltaDomain`]) may have
    /// changed.
    To {
        /// The tentative site.
        position: Position,
        /// Direction on arrival.
        direction: Direction,
    },
    /// The region ended before the segment did. The particle stands exactly on
    /// the region's boundary; nothing beyond it is sampled.
    Exit {
        /// The boundary point.
        position: Position,
    },
}

/// What occupies a tentative collision site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteContent {
    /// A material, by index into the material table.
    Material(usize),
    /// A void cell: `Σ_t = 0`, so the site is always virtual.
    Void,
    /// The lookup could not place the point (outside the model, or the
    /// geometry and the flight disagree). The flight cannot continue
    /// honestly, so the history is lost and reported as such.
    Lost,
}

/// One tentative collision site, as handed to a flight's visitor before the
/// site is classified real or virtual.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TentativeSite {
    /// Where the site is.
    pub position: Position,
    /// The material there; `None` for a void cell.
    pub material: Option<usize>,
    /// The majorant `Σ_maj(E)` \[cm⁻¹\] the flight was sampled on.
    pub majorant: f64,
    /// The total `Σ_t` \[cm⁻¹\] the accept/reject step reads here (`0` in a
    /// void). `sigma_t > majorant` is a majorant violation: the accept
    /// probability is clamped to 1 and collisions are silently lost.
    pub sigma_t: f64,
}

impl TentativeSite {
    /// Does the majorant fail to bound `Σ_t` here?
    #[inline]
    pub fn violates_majorant(&self) -> bool {
        self.sigma_t > self.majorant
    }
}

/// The geometry a delta flight runs through, as the flight loop needs it:
/// how a segment of length `s` moves the particle, and what is at a point.
///
/// A compile-time contract (generic, never `dyn`): the bounded CSG region of
/// the hybrid driver ([`BoundedRay`]) and the reflective domains of
/// [`crate::pebble_beds::keff_delta`] are its two implementors.
pub trait DeltaRegion {
    /// Move `s` \[cm\] from `r` along `u`, or report that the region ends
    /// first.
    fn advance(&self, r: Position, u: Direction, s: f64) -> Advance;
    /// What occupies `r`.
    fn content(&self, r: Position) -> SiteContent;
}

/// What a point lookup returns, read as a [`SiteContent`].
///
/// `Option<usize>` is the contract of the public [`bounded_delta_flight`]
/// family: `None` is a point that cannot be placed ([`SiteContent::Lost`]),
/// and there is no way to say "void". A lookup that can see a void returns
/// [`SiteContent`] itself (the hybrid driver does, gh:#719).
pub trait IntoSiteContent {
    /// The content this lookup result describes.
    fn into_site_content(self) -> SiteContent;
}

impl IntoSiteContent for Option<usize> {
    #[inline]
    fn into_site_content(self) -> SiteContent {
        match self {
            Some(m) => SiteContent::Material(m),
            None => SiteContent::Lost,
        }
    }
}

impl IntoSiteContent for SiteContent {
    #[inline]
    fn into_site_content(self) -> SiteContent {
        self
    }
}

/// A straight ray through a region bounded by `distance_to_exit`, with
/// `material_at` naming what is at a point (an `Option<usize>` or a
/// [`SiteContent`], see [`IntoSiteContent`]).
pub struct BoundedRay<D, M> {
    /// Distance along the direction from a point to where the region ends;
    /// `f64::INFINITY` if it does not end.
    pub distance_to_exit: D,
    /// What is at a point.
    pub material_at: M,
}

impl<D, M, C> DeltaRegion for BoundedRay<D, M>
where
    D: Fn(Position, Direction) -> f64,
    M: Fn(Position) -> C,
    C: IntoSiteContent,
{
    #[inline]
    fn advance(&self, r: Position, u: Direction, s: f64) -> Advance {
        let d_exit = (self.distance_to_exit)(r, u);
        if s >= d_exit {
            // Land EXACTLY on the boundary -- not past it -- so the caller can
            // re-locate unambiguously.
            Advance::Exit {
                position: stream(r, u, d_exit),
            }
        } else {
            Advance::To {
                position: stream(r, u, s),
                direction: u,
            }
        }
    }

    #[inline]
    fn content(&self, r: Position) -> SiteContent {
        (self.material_at)(r).into_site_content()
    }
}

/// Which macroscopic total the accept/reject step reads at a site.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteTotal {
    /// [`Material::macro_xs_total`]: the smooth total.
    Smooth,
    /// [`Material::macro_xs_total_urr`] on this history's URR stream: the
    /// **band** total the collision will use (GitHub #407).
    UrrBand(u64),
}

/// How one delta flight ended.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FlightEnd {
    /// A real collision: sample the reaction here.
    Collision {
        /// Where.
        position: Position,
        /// Direction on arrival.
        direction: Direction,
        /// The material collided in.
        material: usize,
        /// Virtual collisions rejected on the way.
        virtual_collisions: u32,
    },
    /// The region ended first; the particle is on its boundary.
    Exit {
        /// The boundary point.
        position: Position,
        /// Direction, unchanged.
        direction: Direction,
        /// Virtual collisions rejected inside the region.
        virtual_collisions: u32,
    },
    /// The history is lost: the virtual-collision budget ran out, or a site
    /// could not be placed ([`SiteContent::Lost`]).
    Lost {
        /// Virtual collisions rejected before the loss.
        virtual_collisions: u32,
    },
}

/// **The delta-tracking flight loop** (Woodcock et al. 1965; the form of
/// Leppänen 2010, Ann. Nucl. Energy 37, 715-722). NEW WORK: OpenMC has no
/// delta tracking.
///
/// ```text
/// Σ_maj = majorant(E)                    (once: E is constant in flight)
/// repeat at most max_virtual times:
///     s ← −ln ξ / Σ_maj                  (one draw)
///     advance s through the region       (stop on its boundary: Exit)
///     look up the content of the site    (lost: Lost)
///     visit(site)                        (scoring hook; draws nothing)
///     ξ < Σ_t(site)/Σ_maj ?  Collision : virtual, continue   (one draw)
/// Lost (budget exhausted)
/// ```
///
/// # Invariants
///
/// - **Unbiased if and only if `Σ_maj ≥ Σ_t` at every site** (see
///   [`super::majorant`]). Where the bound fails the accept probability is
///   clamped to 1 and the excess collisions are silently lost.
/// - **Stopping on the region boundary is exact**, not an approximation: the
///   flight length is exponential and memoryless, so resuming on the far
///   side under any method and any majorant gives the same distribution of
///   real collision sites.
/// - **`visit` draws no random number**, so a run that scores tentative
///   sites takes exactly the same histories as one that does not.
/// - **A void site is always virtual** and draws no accept/reject variate.
///
/// `majorant` is `Σ_maj` at the flight's energy; a non-positive value is the
/// caller's to handle (nothing can collide, so there is no flight to
/// sample) and is treated here as a lost history.
#[allow(clippy::too_many_arguments)]
pub fn fly<R, V>(
    start: Position,
    direction: Direction,
    energy: f64,
    majorant: f64,
    region: &R,
    materials: &[Material],
    nuclides: &[Nuclide],
    total: SiteTotal,
    max_virtual: u32,
    seed: &mut u64,
    mut visit: V,
) -> FlightEnd
where
    R: DeltaRegion,
    V: FnMut(TentativeSite),
{
    if !(majorant > 0.0) {
        return FlightEnd::Lost {
            virtual_collisions: 0,
        };
    }
    let mut r = start;
    let mut u = direction;
    let mut virtual_collisions = 0_u32;
    for _ in 0..max_virtual {
        let s = sample_delta_distance(majorant, seed);
        match region.advance(r, u, s) {
            Advance::Exit { position } => {
                return FlightEnd::Exit {
                    position,
                    direction: u,
                    virtual_collisions,
                };
            }
            Advance::To {
                position,
                direction,
            } => {
                r = position;
                u = direction;
            }
        }
        let m = match region.content(r) {
            SiteContent::Material(m) => m,
            SiteContent::Void => {
                visit(TentativeSite {
                    position: r,
                    material: None,
                    majorant,
                    sigma_t: 0.0,
                });
                virtual_collisions += 1;
                continue;
            }
            // A point inside what the geometry says is the region that the
            // lookup cannot place is a geometry/query disagreement, not a
            // physical escape; continuing would bias the result.
            SiteContent::Lost => return FlightEnd::Lost { virtual_collisions },
        };
        let sigma_t = match total {
            SiteTotal::UrrBand(us) => materials[m].macro_xs_total_urr(energy, nuclides, us),
            SiteTotal::Smooth => materials[m].macro_xs_total(energy, nuclides),
        };
        visit(TentativeSite {
            position: r,
            material: Some(m),
            majorant,
            sigma_t,
        });
        match classify_collision(sigma_t, majorant, seed) {
            DeltaEvent::Real => {
                return FlightEnd::Collision {
                    position: r,
                    direction: u,
                    material: m,
                    virtual_collisions,
                }
            }
            DeltaEvent::Virtual => virtual_collisions += 1,
        }
    }
    FlightEnd::Lost { virtual_collisions }
}

// ---------------------------------------------------------------------------
// BOUNDED DELTA FLIGHT — the handoff half of hybrid tracking (bn:op-867c.3)
// ---------------------------------------------------------------------------

/// How a [`bounded_delta_flight`] ended.
///
/// NEW WORK, no OpenMC counterpart. The existing `delta_flight` in
/// [`super::keff_delta`] returns `Option<(Position, usize, Direction)>`, which
/// can express only "real collision" and "gone" — it cannot distinguish a
/// particle that **left the delta region** (and must continue under the
/// enclosing tracker) from one whose history was **lost**. Hybrid tracking
/// needs all three apart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeltaStep {
    /// A real collision inside the region. Sample the reaction here.
    Collision {
        /// Where the collision happened.
        position: Position,
        /// Material index at that point.
        material: usize,
        /// Direction on arrival (unchanged by the flight itself).
        direction: Direction,
        /// Virtual collisions rejected on the way. The cost of the majorant.
        virtual_collisions: u32,
    },
    /// The flight reached the region boundary. The particle sits **exactly on
    /// it**, and the caller continues with the enclosing tracking method.
    Exit {
        /// The boundary point, not a point beyond it.
        position: Position,
        /// Direction, unchanged.
        direction: Direction,
        /// Virtual collisions rejected inside the region.
        virtual_collisions: u32,
    },
    /// The virtual-collision budget ran out. **The history is lost**, and this
    /// variant exists so that is reported rather than silent.
    ///
    /// `keff_delta.rs`'s `delta_flight` signals this as a bare `None`, which
    /// the caller cannot tell from a legitimate exit — it shows up only as an
    /// unexplained leak. Counting it is half of `bn:op-867c.5`.
    Exhausted {
        /// The budget that was exhausted.
        virtual_collisions: u32,
    },
}

/// **Delta-track a flight inside a BOUNDED region, stopping at its boundary.**
///
/// The half of hybrid tracking that does not exist today. `delta_flight`
/// (`keff_delta.rs:453`) samples a distance, advances the **full** distance,
/// and only then looks up the material — so when the flight leaves the region
/// it returns a point already *past* the boundary, which is useless for a
/// handoff. This truncates at the boundary instead.
///
/// # Why truncating is EXACTLY unbiased, not an approximation
///
/// The flight length is exponential with rate `Σ_maj`, which is **memoryless**:
/// `P(s > a + b | s > a) = P(s > b)`. So cutting a sampled flight at the
/// boundary and resuming the sampling on the far side — under whatever method
/// and whatever majorant apply there — gives the same distribution of
/// interaction points as never having cut it. No weight correction, no
/// rejection term, nothing to get subtly wrong.
///
/// This is the property the whole hybrid design rests on, and it is why the
/// answer cannot depend on where the region boundaries are drawn. Only the
/// **cost** depends on that.
///
/// # Parameters
/// - `start`, `direction`, `energy` — the particle.
/// - `majorant` — bounding `Σ_t` over **this region's** materials, built with
///   [`Majorant::over_indices`]. It must bound every material
///   `material_at` can return inside the region: an under-bound majorant is a
///   **silent bias**, not a crash.
/// - `distance_to_exit` — distance along `direction` from a point to where the
///   region ends. `f64::INFINITY` means "not reached from here".
/// - `material_at` — material index at a point inside the region.
/// - `max_virtual` — budget before the history is declared lost.
/// - `seed` — the particle's RNG stream, advanced in place.
///
/// # Returns
/// [`DeltaStep`] — collision, exit, or exhaustion, each carrying the
/// virtual-collision count so the majorant's price is measurable rather than
/// assumed.
pub fn bounded_delta_flight<D, M>(
    start: Position,
    direction: Direction,
    energy: f64,
    majorant: &Majorant,
    materials: &[Material],
    nuclides: &[Nuclide],
    max_virtual: u32,
    distance_to_exit: D,
    material_at: M,
    seed: &mut u64,
) -> DeltaStep
where
    D: Fn(Position, Direction) -> f64,
    M: Fn(Position) -> Option<usize>,
{
    bounded_delta_flight_urr(
        start,
        direction,
        energy,
        majorant,
        materials,
        nuclides,
        max_virtual,
        distance_to_exit,
        material_at,
        seed,
        None,
    )
}

/// [`bounded_delta_flight`] for a neutron carrying a URR stream seed: the real
/// `Σ_t` at each tentative site is the **band** total
/// ([`Material::macro_xs_total_urr`]), the one the collision will use, as in
/// OpenMC (GitHub #407). The majorant must then bound band totals, which every
/// constructor here does ([`Material::macro_xs_total_upper_bound`]). With
/// `urr_seed = None` it is `bounded_delta_flight` exactly.
#[allow(clippy::too_many_arguments)]
pub fn bounded_delta_flight_urr<D, M>(
    start: Position,
    direction: Direction,
    energy: f64,
    majorant: &Majorant,
    materials: &[Material],
    nuclides: &[Nuclide],
    max_virtual: u32,
    distance_to_exit: D,
    material_at: M,
    seed: &mut u64,
    urr_seed: Option<u64>,
) -> DeltaStep
where
    D: Fn(Position, Direction) -> f64,
    M: Fn(Position) -> Option<usize>,
{
    bounded_delta_flight_visiting(
        start,
        direction,
        energy,
        majorant,
        materials,
        nuclides,
        max_virtual,
        distance_to_exit,
        material_at,
        seed,
        urr_seed,
        |_, _, _| {},
    )
}

/// [`bounded_delta_flight_urr`] that also calls `visit(position, material,
/// majorant)` at **every tentative collision site** inside the region, virtual
/// and real alike (the real one last), before the site is classified.
///
/// NEW WORK, no OpenMC counterpart: OpenMC has no delta tracking, and its
/// MGXS module scores flux with the track-length estimator
/// (`openmc/mgxs/mgxs.py`, `estimator = 'tracklength'`), which a
/// delta-tracked flight cannot supply. This is what the **delta-tracking
/// collision estimator** needs (gh:#598). In a Woodcock-tracked region the
/// tentative sites are a Poisson process of rate `Σ_maj(E)` along the flight,
/// so their density is `φ(r,E)·Σ_maj(E)` everywhere in the region,
/// *including* the near-void helium between pebbles. Hence `w/Σ_maj` per site
/// is an unbiased flux estimator and `w·Σ_x(r)/Σ_maj` an unbiased
/// reaction-rate one. This is Serpent's collision flux estimator under delta
/// tracking (J. Leppänen, "Performance of Woodcock delta-tracking in lattice
/// physics applications using the Serpent Monte Carlo reactor physics burnup
/// calculation code", Ann. Nucl. Energy 37 (2010) 715-722). The
/// real-collision estimator `w/Σ_t` is unbiased too, but where `Σ_t ≪ Σ_maj`
/// (helium at 1 atm: `Σ_t/Σ_maj ~ 5e-5`) its support is almost never
/// sampled and each rare sample is enormous.
///
/// **The visitor draws no random numbers**, so a run that visits takes
/// exactly the same histories as one that does not.
#[allow(clippy::too_many_arguments)]
pub fn bounded_delta_flight_visiting<D, M, V>(
    start: Position,
    direction: Direction,
    energy: f64,
    majorant: &Majorant,
    materials: &[Material],
    nuclides: &[Nuclide],
    max_virtual: u32,
    distance_to_exit: D,
    material_at: M,
    seed: &mut u64,
    urr_seed: Option<u64>,
    mut visit: V,
) -> DeltaStep
where
    D: Fn(Position, Direction) -> f64,
    M: Fn(Position) -> Option<usize>,
    V: FnMut(Position, usize, f64),
{
    let maj = majorant.at(energy);

    // A non-positive majorant means nothing in this region can interact, so the
    // particle crosses it ballistically. Treat that as an exit rather than a
    // lost history: it is a legitimate (if degenerate) physical situation, e.g.
    // a void region.
    if !(maj > 0.0) {
        let d = distance_to_exit(start, direction);
        return DeltaStep::Exit {
            position: if d.is_finite() {
                stream(start, direction, d)
            } else {
                start
            },
            direction,
            virtual_collisions: 0,
        };
    }

    let region = BoundedRay {
        distance_to_exit,
        material_at,
    };
    let total = match urr_seed {
        Some(us) => SiteTotal::UrrBand(us),
        None => SiteTotal::Smooth,
    };
    let end = fly(
        start,
        direction,
        energy,
        maj,
        &region,
        materials,
        nuclides,
        total,
        max_virtual,
        seed,
        |site: TentativeSite| {
            // `BoundedRay` never reports a void, so `material` is present.
            if let Some(m) = site.material {
                visit(site.position, m, site.majorant);
            }
        },
    );
    DeltaStep::from(end)
}

impl From<FlightEnd> for DeltaStep {
    fn from(end: FlightEnd) -> Self {
        match end {
            FlightEnd::Collision {
                position,
                direction,
                material,
                virtual_collisions,
            } => DeltaStep::Collision {
                position,
                material,
                direction,
                virtual_collisions,
            },
            FlightEnd::Exit {
                position,
                direction,
                virtual_collisions,
            } => DeltaStep::Exit {
                position,
                direction,
                virtual_collisions,
            },
            FlightEnd::Lost { virtual_collisions } => DeltaStep::Exhausted { virtual_collisions },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Woodcock tracking in a homogeneous medium must reproduce the true collision
    /// density regardless of how loose the majorant is: the first real collision
    /// distance is exponentially distributed with mean 1/Σ_t. Here Σ_t = 0.5 cm⁻¹
    /// (mean free path 2 cm) tracked under a 4× majorant.
    #[test]
    fn delta_tracking_recovers_true_mean_free_path() {
        let sigma_t = 0.5_f64;
        let maj = Majorant::uniform(4.0 * sigma_t); // deliberately loose
        let mut seed = 0x1234_5678u64;
        let n = 200_000usize;
        let mut sum = 0.0;
        let mut virtual_total = 0u64;
        for _ in 0..n {
            let f = track_to_collision(
                Position::new(0.0, 0.0, 0.0),
                Direction::new(1.0, 0.0, 0.0),
                1.0e6,
                &maj,
                10_000,
                &mut seed,
                |_p| Some(sigma_t), // homogeneous, infinite medium
            );
            assert!(!f.escaped);
            sum += f.distance;
            virtual_total += f.virtual_collisions as u64;
        }
        let mean = sum / n as f64;
        let expected = 1.0 / sigma_t; // = 2 cm
        assert!(
            (mean - expected).abs() < 0.03,
            "delta-tracked mean free path {mean} ≠ 1/Σ_t {expected}"
        );
        // With a 4× majorant ~3 of every 4 collisions should be virtual.
        let virtual_frac = virtual_total as f64 / (virtual_total as f64 + n as f64);
        assert!(
            (virtual_frac - 0.75).abs() < 0.02,
            "virtual fraction {virtual_frac} ≠ 1 − Σ_t/Σ_maj = 0.75"
        );
    }

    /// The real/virtual split matches the ratio Σ_t/Σ_maj over many draws.
    #[test]
    fn classify_matches_ratio() {
        let mut seed = 99u64;
        let (sigma_t, maj) = (0.3_f64, 1.0_f64);
        let n = 100_000;
        let reals = (0..n)
            .filter(|_| classify_collision(sigma_t, maj, &mut seed) == DeltaEvent::Real)
            .count();
        let frac = reals as f64 / n as f64;
        assert!((frac - 0.3).abs() < 0.01, "real fraction {frac} ≠ 0.3");
    }
}
