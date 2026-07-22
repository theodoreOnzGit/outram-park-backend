//! Random sphere packing — generating and querying explicit dispersion-fuel geometry.
//!
//! This is where the packed-particle geometry for a doubly-heterogeneous
//! TRISO/pebble layout is *generated* and *queried*. A pebble holds O(10⁴) TRISO
//! kernels scattered at random through a graphite matrix; this module produces
//! that random, non-overlapping set of kernel centres and answers the one question
//! the [`super::delta_tracking`] flight asks over and over: **"is this point inside
//! a fuel kernel or in the matrix?"**
//!
//! Despite living under [`super`], the packer itself is **application-neutral**: it
//! places equal-radius spheres in a cube. TRISO kernels are the motivating case and
//! supply the naming of [`PackedSpheres::is_inside_kernel`], but the same generator
//! serves any dispersion medium — B4C burnable-poison particles, for instance.
//!
//! # Relationship to [`crate::stochastic`]
//!
//! This module is the **explicit** end of the memory/fidelity spectrum: every inclusion
//! is stored, so point membership is exact and memory is O(N). The approximate models
//! that sample geometry instead of storing it — Chord Length Sampling and Semi-Implicit
//! CLS — live in [`crate::stochastic`], and treat what this module produces as their
//! reference solution via [`crate::stochastic::medium::RsaMedium`].
//!
//! (Named `sphere_packing` rather than `stochastic_media` so it is not confused with
//! that sibling module — the two would otherwise read as the same thing.)
//!
//! # What is implemented
//!
//! - [`pack_spheres`] — **Random Sequential Addition (RSA)**: drop kernel centres
//!   one at a time at uniform-random positions, rejecting any that would overlap an
//!   already-placed kernel, until the target packing fraction is reached. This is a
//!   direct port of OpenMC's `_random_sequential_pack` (the RSA half of
//!   `openmc.model.pack_spheres`, `openmc/model/triso.py:882` and `:1210`), using
//!   the same overlap-acceleration mesh so the nearest-neighbour test stays O(1)
//!   per trial rather than O(N).
//! - [`PackedSpheres`] — owns the packed kernel list plus a uniform spatial hash
//!   grid, and offers a fast [`PackedSpheres::is_inside_kernel`] point-membership
//!   test for the transport lookup.
//!
//! RSA saturates near a packing fraction of ~0.38 (the [`MAX_PF_RSA`] limit); above
//! that the trial-rejection loop becomes prohibitively slow. The higher-density
//! close-random-packing methods (Jodrey–Tory contraction, RSA–DEM / ODR–DEM
//! relaxation — [`PackingMethod::RsaDem`] / [`PackingMethod::OdrDem`], see the
//! [`super::references`] bibliography) are **not** implemented here; they remain a
//! follow-up for reaching pebble-bed-realistic (~0.6) packing fractions.
//!
//! # Provenance
//!
//! The RSA algorithm and its acceleration mesh mirror OpenMC (MIT):
//! `openmc/model/triso.py` — `_random_sequential_pack` (line 882),
//! `_RectangularPrism` container (line 253), `pack_spheres` (line 1210). The
//! high-packing-fraction DEM methods draw on Tan et al. — see [`super::references`].

use crate::geometry::position::Position;
use crate::rng::lcg::prn;
use std::collections::HashMap;

/// Maximum packing fraction Random Sequential Addition can reach in practice.
///
/// Mirrors OpenMC's `MAX_PF_RSP = 0.38` (`openmc/model/triso.py:20`). Above this,
/// RSA's reject-until-no-overlap loop effectively stops finding room for new
/// spheres; [`pack_spheres`] returns [`PackingError::PackingTooDense`]
/// rather than spinning forever.
pub const MAX_PF_RSA: f64 = 0.38;

/// One packed spherical particle (e.g. a TRISO kernel) in a stochastic medium.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    /// Centre \[cm\].
    pub center: Position,
    /// Radius \[cm\].
    pub radius: f64,
}

/// Which packing-generation algorithm to use. See [`super::references`].
///
/// The set is closed and dispatched by `match` (no trait objects), per the
/// workspace design rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PackingMethod {
    /// Random Sequential Addition — reject-until-no-overlap insertion. Simple and
    /// parallelizable, but saturates around ~38 % packing fraction ([`MAX_PF_RSA`]).
    /// Implemented (see [`pack_spheres`]). Ref: [`super::references::TAN2024_RSA`].
    Rsa,
    /// Iterative RSA followed by Discrete-Element-Method relaxation, to reach the
    /// high packing fractions RSA alone cannot. **Not implemented.** Ref:
    /// [`super::references::TAN2026_RSA_DEM`].
    RsaDem,
    /// Coupled Ordered/Overlap-Driven-Relaxation with DEM. **Not implemented.**
    /// Ref: [`super::references::TAN2026_ODR_DEM`].
    OdrDem,
}

/// Parameters for generating a packed dispersion-fuel region in a cubic domain.
///
/// The domain is an axis-aligned cube centred at the origin with half-width
/// `domain_half_width`; centres are placed so every sphere lies fully inside it.
#[derive(Debug, Clone, Copy)]
pub struct PackingConfig {
    /// Particle radius \[cm\] (all particles equal-radius).
    pub particle_radius: f64,
    /// Target volumetric packing fraction (0…[`MAX_PF_RSA`] for [`PackingMethod::Rsa`]).
    pub packing_fraction: f64,
    /// Half-width \[cm\] of the cubic domain the particles pack into.
    pub domain_half_width: f64,
    /// Generation algorithm.
    pub method: PackingMethod,
    /// RNG seed for reproducibility.
    pub seed: u64,
}

/// Errors from stochastic-media generation.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PackingError {
    /// The requested method is not implemented yet (only [`PackingMethod::Rsa`] is).
    #[error("stochastic-media generator not implemented for {0:?} (only Rsa is)")]
    NotImplemented(PackingMethod),
    /// The target packing fraction exceeds what RSA can reach ([`MAX_PF_RSA`]).
    #[error("packing fraction {requested} exceeds the RSA limit {limit}")]
    PackingTooDense {
        /// The requested packing fraction.
        requested: f64,
        /// The RSA limit ([`MAX_PF_RSA`]).
        limit: f64,
    },
    /// A sphere could not be placed without overlap within the attempt budget —
    /// the domain is effectively saturated (try a lower packing fraction).
    #[error("could not place sphere {placed}/{target} within {attempts} attempts (domain saturated)")]
    PlacementFailed {
        /// How many spheres were placed before giving up.
        placed: usize,
        /// How many were requested.
        target: usize,
        /// The per-sphere attempt budget that was exhausted.
        attempts: usize,
    },
    /// The domain is too small to hold even one sphere (radius ≥ half-width).
    #[error("domain half-width {half_width} too small for radius {radius}")]
    DomainTooSmall {
        /// The domain half-width.
        half_width: f64,
        /// The sphere radius.
        radius: f64,
    },
}

impl PackingConfig {
    /// Generate the packed sphere list for this configuration.
    ///
    /// Dispatches on [`PackingConfig::method`]: [`PackingMethod::Rsa`] runs the
    /// implemented RSA packer ([`pack_spheres`]); the DEM methods return
    /// [`PackingError::NotImplemented`] (not ported — see the module docs).
    pub fn generate(&self) -> Result<Vec<Sphere>, PackingError> {
        match self.method {
            PackingMethod::Rsa => pack_spheres(
                self.particle_radius,
                self.domain_half_width,
                self.packing_fraction,
                self.seed,
            ),
            other => Err(PackingError::NotImplemented(other)),
        }
    }
}

/// Number of equal-radius spheres a target packing fraction implies in a cube.
///
/// `N = floor(pf · V_cube / V_sphere)`, with `V_cube = (2·half_width)³` and
/// `V_sphere = (4/3)·π·radius³` — mirrors OpenMC
/// `int(pf*domain.volume//volume)` (`openmc/model/triso.py:1308`).
fn sphere_count(radius: f64, half_width: f64, packing_fraction: f64) -> usize {
    let v_cube = (2.0 * half_width).powi(3);
    let v_sphere = 4.0 / 3.0 * std::f64::consts::PI * radius.powi(3);
    (packing_fraction * v_cube / v_sphere).floor() as usize
}

/// Random Sequential Addition packing of equal spheres in a cubic domain.
///
/// Places `N = floor(pf · V_cube / V_sphere)` kernel centres one at a time at
/// uniform-random positions inside the cube (each centre kept at least `radius`
/// from every wall so the sphere is fully contained), rejecting any trial position
/// that would bring a new sphere within one diameter of an already-placed one.
/// Returns the accepted centres as [`Sphere`]s.
///
/// # Algorithm (ported from OpenMC, MIT)
///
/// Direct port of `_random_sequential_pack` (`openmc/model/triso.py:882`) with the
/// `_RectangularPrism` container's uniform placement and overlap mesh
/// (`openmc/model/triso.py:253`, `:396`). A uniform grid of cell size
/// `≈ 4·radius` overlays the domain; each placed centre is registered in every
/// grid cell within one diameter of it, so an overlap test for a trial point only
/// scans the spheres registered in the trial's own cell — O(1) instead of O(N).
/// The overlap predicate is squared-distance `< (2·radius)²`, exactly as upstream.
///
/// # Parameters
/// - `radius` — sphere radius \[cm\]; all spheres equal-radius.
/// - `half_width` — half-width \[cm\] of the axis-aligned cube (centred at origin).
/// - `packing_fraction` — target volumetric fraction; must be ≤ [`MAX_PF_RSA`].
/// - `seed` — RNG seed (uses the crate LCG [`prn`]) for a reproducible packing.
///
/// # Errors
/// - [`PackingError::PackingTooDense`] if `packing_fraction > MAX_PF_RSA`.
/// - [`PackingError::DomainTooSmall`] if a sphere cannot fit in the cube.
/// - [`PackingError::PlacementFailed`] if the domain saturates before all
///   `N` spheres are placed (the per-sphere attempt budget is exhausted).
pub fn pack_spheres(
    radius: f64,
    half_width: f64,
    packing_fraction: f64,
    seed: u64,
) -> Result<Vec<Sphere>, PackingError> {
    if packing_fraction > MAX_PF_RSA {
        return Err(PackingError::PackingTooDense {
            requested: packing_fraction,
            limit: MAX_PF_RSA,
        });
    }
    if radius >= half_width {
        return Err(PackingError::DomainTooSmall { half_width, radius });
    }

    let n_target = sphere_count(radius, half_width, packing_fraction);
    if n_target == 0 {
        return Ok(Vec::new());
    }

    // Uniform overlap-acceleration mesh: cell length ≈ 4·radius, at least one cell.
    // (OpenMC: cell_length = L / int(L / (4·radius)), openmc/model/triso.py:349.)
    let domain = 2.0 * half_width;
    let n_cells = ((domain / (4.0 * radius)).floor() as i64).max(1);
    let cell_length = domain / n_cells as f64;

    // Centres must stay ≥ radius from every wall (fully contained).
    let lo = -half_width + radius;
    let hi = half_width - radius;
    let span = hi - lo;
    let diameter = 2.0 * radius;
    let sqd = diameter * diameter;

    // mesh cell index of a coordinate (shifted so the domain starts at 0).
    let cell_of = |x: f64| ((x + half_width) / cell_length).floor() as i64;
    // Cells within one diameter of a coordinate — the {x−d, x, x+d} spread.
    let nearby = |x: f64| {
        let a = cell_of(x - diameter);
        let b = cell_of(x);
        let c = cell_of(x + diameter);
        let mut v = vec![a, b, c];
        v.sort_unstable();
        v.dedup();
        v
    };

    let mut spheres: Vec<Position> = Vec::with_capacity(n_target);
    // Grid cell (i,j,k) → indices into `spheres` whose centre is within one
    // diameter of that cell (mirrors OpenMC's `mesh` defaultdict).
    let mut mesh: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();

    let mut rng = seed;
    // Attempt budget scales with the target: near saturation each placement needs
    // many trials, but a hard cap keeps a too-dense request from hanging.
    let attempts_per_sphere = 10_000usize;

    for placed in 0..n_target {
        let mut trials = 0usize;
        loop {
            trials += 1;
            if trials > attempts_per_sphere {
                return Err(PackingError::PlacementFailed {
                    placed,
                    target: n_target,
                    attempts: attempts_per_sphere,
                });
            }
            let px = lo + span * prn(&mut rng);
            let py = lo + span * prn(&mut rng);
            let pz = lo + span * prn(&mut rng);
            let key = (cell_of(px), cell_of(py), cell_of(pz));
            let overlaps = mesh.get(&key).map(|idxs| {
                idxs.iter().any(|&q| {
                    let c = spheres[q];
                    let dx = px - c.x;
                    let dy = py - c.y;
                    let dz = pz - c.z;
                    dx * dx + dy * dy + dz * dz < sqd
                })
            }).unwrap_or(false);
            if overlaps {
                continue;
            }
            // Accept: register in every mesh cell within one diameter.
            let idx = spheres.len();
            spheres.push(Position::new(px, py, pz));
            for &i in &nearby(px) {
                for &j in &nearby(py) {
                    for &k in &nearby(pz) {
                        mesh.entry((i, j, k)).or_default().push(idx);
                    }
                }
            }
            break;
        }
    }

    Ok(spheres
        .into_iter()
        .map(|center| Sphere { center, radius })
        .collect())
}

/// A packed set of equal-radius kernels plus a spatial hash grid for fast
/// point-membership — the geometry the delta-tracking flight queries.
///
/// Owns the [`Sphere`] list produced by [`pack_spheres`] and a uniform grid that
/// makes [`PackedSpheres::is_inside_kernel`] O(1): each kernel is registered in
/// every grid cell its body touches, so a membership query scans only the kernels
/// registered in the query point's own cell.
///
/// This type is deliberately *physics-free* — it answers "inside a kernel or not?"
/// and leaves the mapping to material indices (fuel vs matrix) to the caller, so it
/// can serve any two-material dispersion medium.
#[derive(Debug, Clone)]
pub struct PackedSpheres {
    spheres: Vec<Sphere>,
    half_width: f64,
    radius: f64,
    cell_length: f64,
    /// cell (i,j,k) → indices of kernels whose body touches that cell.
    grid: HashMap<(i64, i64, i64), Vec<u32>>,
}

impl PackedSpheres {
    /// Pack a cubic domain by RSA and build the membership grid in one step.
    ///
    /// Parameters match [`pack_spheres`]. See it for the algorithm, provenance, and
    /// error conditions.
    pub fn pack(
        radius: f64,
        half_width: f64,
        packing_fraction: f64,
        seed: u64,
    ) -> Result<Self, PackingError> {
        let spheres = pack_spheres(radius, half_width, packing_fraction, seed)?;
        Ok(Self::from_spheres(spheres, half_width, radius))
    }

    /// Build a membership grid over an already-generated packing.
    ///
    /// `radius` is the common kernel radius and `half_width` the cubic domain
    /// half-width the spheres were packed into. Each kernel is registered in every
    /// grid cell its body (centre ± radius) touches.
    pub fn from_spheres(spheres: Vec<Sphere>, half_width: f64, radius: f64) -> Self {
        // Cell length ≈ 2·radius keeps at most a handful of kernels per cell.
        let cell_length = (2.0 * radius).max(f64::MIN_POSITIVE);
        let cell_of = |x: f64| ((x + half_width) / cell_length).floor() as i64;
        let mut grid: HashMap<(i64, i64, i64), Vec<u32>> = HashMap::new();
        for (idx, s) in spheres.iter().enumerate() {
            let c = s.center;
            let (ix0, ix1) = (cell_of(c.x - radius), cell_of(c.x + radius));
            let (iy0, iy1) = (cell_of(c.y - radius), cell_of(c.y + radius));
            let (iz0, iz1) = (cell_of(c.z - radius), cell_of(c.z + radius));
            for i in ix0..=ix1 {
                for j in iy0..=iy1 {
                    for k in iz0..=iz1 {
                        grid.entry((i, j, k)).or_default().push(idx as u32);
                    }
                }
            }
        }
        Self { spheres, half_width, radius, cell_length, grid }
    }

    /// Is the point `p` \[cm\] inside any packed kernel?
    ///
    /// O(1): looks up only the kernels registered in `p`'s grid cell and tests each
    /// for `|p − centre| < radius`. A point in the matrix (between kernels) or
    /// outside the domain returns `false`.
    pub fn is_inside_kernel(&self, p: Position) -> bool {
        let cell_of = |x: f64| ((x + self.half_width) / self.cell_length).floor() as i64;
        let key = (cell_of(p.x), cell_of(p.y), cell_of(p.z));
        let r2 = self.radius * self.radius;
        self.grid.get(&key).map(|idxs| {
            idxs.iter().any(|&q| {
                let c = self.spheres[q as usize].center;
                let dx = p.x - c.x;
                let dy = p.y - c.y;
                let dz = p.z - c.z;
                dx * dx + dy * dy + dz * dz < r2
            })
        }).unwrap_or(false)
    }

    /// The packed kernels.
    pub fn spheres(&self) -> &[Sphere] {
        &self.spheres
    }

    /// Number of packed kernels.
    pub fn len(&self) -> usize {
        self.spheres.len()
    }

    /// Whether the packing is empty.
    pub fn is_empty(&self) -> bool {
        self.spheres.is_empty()
    }

    /// The cubic domain half-width \[cm\].
    pub fn half_width(&self) -> f64 {
        self.half_width
    }

    /// The common kernel radius \[cm\].
    pub fn radius(&self) -> f64 {
        self.radius
    }

    /// Realized volumetric packing fraction `N · V_sphere / V_cube`.
    ///
    /// This is the *achieved* fraction from the accepted spheres — it should match
    /// the requested target to within one sphere's worth of volume (the floor in
    /// [`sphere_count`]).
    pub fn packing_fraction(&self) -> f64 {
        let v_sphere = 4.0 / 3.0 * std::f64::consts::PI * self.radius.powi(3);
        let v_cube = (2.0 * self.half_width).powi(3);
        self.spheres.len() as f64 * v_sphere / v_cube
    }

    /// The smallest centre-to-centre distance between any two kernels \[cm\], or
    /// `None` if fewer than two kernels. For a valid non-overlapping packing this
    /// must be `≥ 2·radius`; the test suite uses it to prove no overlaps.
    ///
    /// O(N²) — intended for verification on modest packings, not the hot loop.
    pub fn min_center_distance(&self) -> Option<f64> {
        let n = self.spheres.len();
        if n < 2 {
            return None;
        }
        let mut min = f64::INFINITY;
        for a in 0..n {
            for b in (a + 1)..n {
                let d = self.spheres[a].center.distance(self.spheres[b].center);
                if d < min {
                    min = d;
                }
            }
        }
        Some(min)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RSA must place the target count with no overlaps, all fully inside the
    /// domain, and hit the requested packing fraction to within one sphere.
    #[test]
    fn rsa_packs_without_overlap_at_target_fraction() {
        let radius = 0.05;
        let half = 1.0;
        let pf = 0.3;
        let packed = PackedSpheres::pack(radius, half, pf, 42).expect("packs");

        // Target count from the same floor formula.
        let expected_n = sphere_count(radius, half, pf);
        assert_eq!(packed.len(), expected_n, "placed sphere count");
        assert!(packed.len() > 50, "expected a non-trivial packing, got {}", packed.len());

        // No overlaps: closest centre pair ≥ one diameter (minus float slack).
        let dmin = packed.min_center_distance().unwrap();
        assert!(
            dmin >= 2.0 * radius - 1e-12,
            "overlap: min centre distance {dmin} < diameter {}",
            2.0 * radius
        );

        // Every kernel fully inside the cube.
        for s in packed.spheres() {
            for coord in [s.center.x, s.center.y, s.center.z] {
                assert!(
                    coord.abs() + radius <= half + 1e-12,
                    "kernel at {coord} pokes outside the domain half-width {half}"
                );
            }
        }

        // Realized packing fraction within one sphere's worth of the target.
        let v_sphere = 4.0 / 3.0 * std::f64::consts::PI * radius.powi(3);
        let one_sphere_pf = v_sphere / (2.0 * half).powi(3);
        assert!(
            (packed.packing_fraction() - pf).abs() <= one_sphere_pf + 1e-9,
            "realized pf {} not within one sphere of target {pf}",
            packed.packing_fraction()
        );
    }

    /// Membership: kernel centres are inside, far-from-any-kernel points are not,
    /// and points outside the domain are not.
    #[test]
    fn membership_matches_geometry() {
        let radius = 0.05;
        let half = 1.0;
        let packed = PackedSpheres::pack(radius, half, 0.25, 7).expect("packs");

        // Every centre is inside its own kernel.
        for s in packed.spheres() {
            assert!(packed.is_inside_kernel(s.center), "centre must be inside");
            // A point just outside this kernel along +x (and not inside a neighbour,
            // checked by brute force) is outside.
        }

        // A point well outside the domain is not inside any kernel.
        assert!(!packed.is_inside_kernel(Position::new(10.0, 10.0, 10.0)));

        // Brute-force cross-check of is_inside_kernel on a grid of probe points.
        let r2 = radius * radius;
        let probe = |p: Position| {
            packed.spheres().iter().any(|s| {
                let d = p - s.center;
                d.x * d.x + d.y * d.y + d.z * d.z < r2
            })
        };
        let mut rng = 12345u64;
        for _ in 0..2000 {
            let p = Position::new(
                -half + 2.0 * half * prn(&mut rng),
                -half + 2.0 * half * prn(&mut rng),
                -half + 2.0 * half * prn(&mut rng),
            );
            assert_eq!(
                packed.is_inside_kernel(p),
                probe(p),
                "grid membership disagrees with brute force at {p:?}"
            );
        }
    }

    /// A packing fraction above the RSA limit is rejected, not silently attempted.
    #[test]
    fn too_dense_is_rejected() {
        let e = pack_spheres(0.05, 1.0, 0.5, 1).unwrap_err();
        assert!(matches!(e, PackingError::PackingTooDense { .. }));
    }

    /// The DEM methods are honestly reported as not implemented.
    #[test]
    fn dem_methods_not_implemented() {
        let cfg = PackingConfig {
            particle_radius: 0.05,
            packing_fraction: 0.3,
            domain_half_width: 1.0,
            method: PackingMethod::RsaDem,
            seed: 1,
        };
        assert_eq!(
            cfg.generate(),
            Err(PackingError::NotImplemented(PackingMethod::RsaDem))
        );
    }

    /// `PackingConfig::generate` with the Rsa method produces the same packing as
    /// the free function (the config path just forwards).
    #[test]
    fn config_rsa_forwards_to_pack_spheres() {
        let cfg = PackingConfig {
            particle_radius: 0.05,
            packing_fraction: 0.2,
            domain_half_width: 1.0,
            method: PackingMethod::Rsa,
            seed: 99,
        };
        let via_config = cfg.generate().unwrap();
        let direct = pack_spheres(0.05, 1.0, 0.2, 99).unwrap();
        assert_eq!(via_config, direct);
    }
}
