// Copyright (C) 2026 Theodore Ong and the outram-park contributors. GPL-3.0-only.

//! **DEM-settled pebble beds** — hand a bed settled by
//! `outram-park-fork-liggghts` to Monte Carlo transport.
//!
//! New work, not an OpenMC port: OpenMC has no granular solver and takes pebble
//! centres from a file (`openmc.model.pack_spheres` is RSA/CRP, which live in
//! [`super::sphere_packing`] and [`super::crp_packing`]). This module is the
//! seam between the two crates, added 2026-10-02 at the maintainer's direction
//! ("make liggghts a dependency of outram-mc so it is easier to model pebble
//! beds").
//!
//! # What it does
//!
//! [`DemBed::from_particles`] takes the particles of a settled DEM system and
//! returns their centres and the (common) radius **in this crate's length
//! convention, cm** — DEM works in SI metres, so every length is multiplied by
//! [`CM_PER_M`]. [`DemBed::from_granular_system`] and
//! [`DemBed::from_dem_simulation`] are the same call on liggghts' two engines.
//! Use `GranularSystem` for anything that must settle into a static bed: per
//! that crate's `CLAUDE.md` it is the LIGGGHTS-faithful engine (shear history),
//! verified against upstream LIGGGHTS on the HTR-10 core. `DemSimulation` has
//! no tangential spring and cannot hold a heap.
//!
//! Two checks are made and neither is hidden:
//!
//! - **Monodispersity is refused, not averaged.** Every particle must have the
//!   same radius to [`MONODISPERSE_REL_TOL`]; otherwise
//!   [`DemBedError::Polydisperse`] names the extremes. A pebble-bed core
//!   universe is built around one pebble radius, and quietly averaging a
//!   polydisperse bed would put fuel where there is none.
//! - **Soft-sphere overlap is measured and reported.** DEM contacts are
//!   *penetrations*: a settled Hertz/Hooke bed has every touching pair
//!   interpenetrating by a small amount set by the contact stiffness and the
//!   load. [`DemBed::max_overlap_cm`] / [`DemBed::max_relative_overlap`] /
//!   [`DemBed::overlapping_pairs`] report it. Nothing is shrunk, moved or
//!   clipped: whether to accept the overlap, shrink the CSG radius, or re-settle
//!   stiffer is the geometry builder's decision, and it should be made with the
//!   number in hand. The HTR-10 model once carried pebbles interpenetrating by
//!   1.1 cm unnoticed (gh:#309, #310), which is why the number is surfaced.
//!
//! # What it does NOT do (yet)
//!
//! - **It does not build a CSG geometry of the bed.** The output is centres and
//!   a radius, the same thing [`super::sphere_packing::Sphere`] carries; turning
//!   them into cells, a lattice, or a delta-tracked medium is the caller's job.
//!   ~~(see `nee_soon::htr10_rmc` for how the HTR-10 core does it from a centre
//!   list)~~ **CORRECTED 2026-10-05:** until today `nee_soon::htr10_rmc` built
//!   its core only from Şeker's lattice, never from a centre list; it now does,
//!   in `nee_soon::htr10_rmc::explicit_bed::assemble_explicit_triso_from_centres`
//!   (overlaps split by the bisector plane, drawn in
//!   `crates/nee_soon/verification_and_validation/htr10_dem_bed_images/`). The
//!   geometry must be drawn and shown per this crate's `CLAUDE.md` before any
//!   k-eff from it is reported.
//! - **It does not check the container.** It does not know the vessel; the
//!   caller does. The test below checks containment for its own cylinder.
//! - **No physics claim.** A DEM-settled bed is a *verification*-grade product
//!   of `outram-park-fork-liggghts` (cross-code agreement with LIGGGHTS, no
//!   experimental comparison — see that crate's `CLAUDE.md`). Converting it to
//!   cm adds nothing to that claim.

use std::collections::BTreeMap;

use outram_park_fork_liggghts::granular_system::GranularSystem;
use outram_park_fork_liggghts::particle::Particle;
use outram_park_fork_liggghts::simulation::DemSimulation;

use crate::geometry::position::Position;
use crate::pebble_beds::sphere_packing::Sphere;

/// Centimetres per metre: DEM (SI) to this crate's length unit.
pub const CM_PER_M: f64 = 100.0;

/// Relative radius spread below which a bed counts as monodisperse.
///
/// DEM particles built from one radius carry bit-identical radii; this
/// tolerance only forgives a radius that has round-tripped through a text
/// file. It is not a knob for accepting a genuinely polydisperse bed.
pub const MONODISPERSE_REL_TOL: f64 = 1e-9;

/// Why a DEM bed could not be converted.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DemBedError {
    /// The system holds no particles.
    #[error("the DEM system holds no particles")]
    Empty,
    /// A particle's position or radius is not finite (a blown-up integration).
    #[error("particle {index} has a non-finite position or radius")]
    NonFinite {
        /// Index of the offending particle.
        index: usize,
    },
    /// The radii differ by more than [`MONODISPERSE_REL_TOL`].
    #[error(
        "bed is polydisperse: radii span {min_radius_m} m .. {max_radius_m} m \
         (relative spread {relative_spread:e} > {MONODISPERSE_REL_TOL:e})"
    )]
    Polydisperse {
        /// Smallest radius \[m\].
        min_radius_m: f64,
        /// Largest radius \[m\].
        max_radius_m: f64,
        /// `(max - min) / max`.
        relative_spread: f64,
    },
}

/// A settled, monodisperse DEM bed in this crate's units (cm).
#[derive(Debug, Clone, PartialEq)]
pub struct DemBed {
    spheres: Vec<Sphere>,
    radius_cm: f64,
    max_overlap_cm: f64,
    overlapping_pairs: usize,
}

impl DemBed {
    /// Convert DEM particles (SI) to pebble centres and radius (cm).
    ///
    /// Particle order is preserved, so index `i` here is particle `i` there.
    /// The common radius reported is the **largest** particle radius — within
    /// [`MONODISPERSE_REL_TOL`] of every other — so a CSG built from it never
    /// under-states a pebble.
    ///
    /// # Errors
    ///
    /// [`DemBedError::Empty`], [`DemBedError::NonFinite`] or
    /// [`DemBedError::Polydisperse`]; see the module docs for why the last is
    /// a refusal and not a warning.
    pub fn from_particles(particles: &[Particle]) -> Result<Self, DemBedError> {
        if particles.is_empty() {
            return Err(DemBedError::Empty);
        }
        let mut r_min = f64::INFINITY;
        let mut r_max = 0.0_f64;
        for (index, p) in particles.iter().enumerate() {
            let finite = p.radius.is_finite()
                && p.position.x.is_finite()
                && p.position.y.is_finite()
                && p.position.z.is_finite();
            if !finite {
                return Err(DemBedError::NonFinite { index });
            }
            r_min = r_min.min(p.radius);
            r_max = r_max.max(p.radius);
        }
        let relative_spread = (r_max - r_min) / r_max;
        if relative_spread > MONODISPERSE_REL_TOL {
            return Err(DemBedError::Polydisperse {
                min_radius_m: r_min,
                max_radius_m: r_max,
                relative_spread,
            });
        }

        let radius_cm = r_max * CM_PER_M;
        let spheres: Vec<Sphere> = particles
            .iter()
            .map(|p| Sphere {
                center: Position::new(
                    p.position.x * CM_PER_M,
                    p.position.y * CM_PER_M,
                    p.position.z * CM_PER_M,
                ),
                radius: radius_cm,
            })
            .collect();
        let (max_overlap_cm, overlapping_pairs) = measure_overlap(&spheres, radius_cm);
        Ok(Self {
            spheres,
            radius_cm,
            max_overlap_cm,
            overlapping_pairs,
        })
    }

    /// [`DemBed::from_particles`] on a `GranularSystem` — the
    /// LIGGGHTS-faithful engine, and the one to settle a bed with.
    ///
    /// # Errors
    ///
    /// As [`DemBed::from_particles`].
    pub fn from_granular_system(system: &GranularSystem) -> Result<Self, DemBedError> {
        Self::from_particles(system.particles())
    }

    /// [`DemBed::from_particles`] on a `DemSimulation` (the stateless engine).
    ///
    /// # Errors
    ///
    /// As [`DemBed::from_particles`].
    pub fn from_dem_simulation(sim: &DemSimulation) -> Result<Self, DemBedError> {
        Self::from_particles(sim.particles())
    }

    /// Pebble centres and radius \[cm\], in DEM particle order.
    pub fn spheres(&self) -> &[Sphere] {
        &self.spheres
    }

    /// Consume the bed, returning the spheres \[cm\].
    pub fn into_spheres(self) -> Vec<Sphere> {
        self.spheres
    }

    /// Number of pebbles.
    pub fn len(&self) -> usize {
        self.spheres.len()
    }

    /// `true` if there are no pebbles (never, for a successfully built bed).
    pub fn is_empty(&self) -> bool {
        self.spheres.is_empty()
    }

    /// The common pebble radius \[cm\].
    pub fn radius_cm(&self) -> f64 {
        self.radius_cm
    }

    /// Largest pairwise interpenetration `2r - d` over all pairs \[cm\]; `0.0`
    /// when no two pebbles overlap.
    pub fn max_overlap_cm(&self) -> f64 {
        self.max_overlap_cm
    }

    /// [`DemBed::max_overlap_cm`] as a fraction of the pebble **diameter**.
    pub fn max_relative_overlap(&self) -> f64 {
        self.max_overlap_cm / (2.0 * self.radius_cm)
    }

    /// Number of pebble pairs that interpenetrate (`d < 2r`).
    pub fn overlapping_pairs(&self) -> usize {
        self.overlapping_pairs
    }
}

/// Largest overlap `2r - d` and the count of overlapping pairs, by a uniform
/// cell list of edge `2r` (only neighbouring cells can hold a touching pair).
/// The result does not depend on iteration order; a `BTreeMap` is used anyway
/// so the traversal itself is deterministic.
fn measure_overlap(spheres: &[Sphere], radius: f64) -> (f64, usize) {
    let diameter = 2.0 * radius;
    let cell = |v: f64| (v / diameter).floor() as i64;
    let mut grid: BTreeMap<(i64, i64, i64), Vec<usize>> = BTreeMap::new();
    for (i, s) in spheres.iter().enumerate() {
        let c = s.center;
        grid.entry((cell(c.x), cell(c.y), cell(c.z)))
            .or_default()
            .push(i);
    }
    let mut max_overlap = 0.0_f64;
    let mut pairs = 0usize;
    for (i, s) in spheres.iter().enumerate() {
        let c = s.center;
        let (cx, cy, cz) = (cell(c.x), cell(c.y), cell(c.z));
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(members) = grid.get(&(cx + dx, cy + dy, cz + dz)) else {
                        continue;
                    };
                    for &j in members {
                        if j <= i {
                            continue;
                        }
                        let o = spheres[j].center;
                        let d = ((c.x - o.x).powi(2) + (c.y - o.y).powi(2) + (c.z - o.z).powi(2))
                            .sqrt();
                        if d < diameter {
                            pairs += 1;
                            max_overlap = max_overlap.max(diameter - d);
                        }
                    }
                }
            }
        }
    }
    (max_overlap, pairs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_park_fork_liggghts::boundary::Boundary;
    use outram_park_fork_liggghts::contact::{ContactModel, HookeContact};
    use outram_park_fork_liggghts::particle::Vec3;
    use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
    use uom::si::length::meter;
    use uom::si::mass::kilogram;
    use uom::si::thermodynamic_temperature::kelvin;

    // The bed of `outram-park-fork-liggghts/examples/pebble_bed.rs`, three
    // layers instead of eight so it settles in about a second (release).
    const R_PEBBLE: f64 = 0.03; // m
    const R_CYL: f64 = 0.25; // m
    const DENSITY: f64 = 1750.0; // kg/m^3
    const SPACING: f64 = 2.0 * R_PEBBLE * 1.03; // m
    const LAYERS: usize = 3;
    const DT: f64 = 1.0e-4; // s
    const STEPS: usize = 10_000; // 1.0 s of settling

    fn pebble(x: f64, y: f64, z: f64, radius: f64) -> Particle {
        let mass = DENSITY * 4.0 / 3.0 * std::f64::consts::PI * radius.powi(3);
        Particle::new(
            Vec3::new(x, y, z),
            Vec3::zero(),
            Vec3::zero(),
            Mass::new::<kilogram>(mass),
            Length::new::<meter>(radius),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .expect("valid pebble")
    }

    /// Lattice seed inside the cylinder, the example's `seed_lattice`.
    fn seed() -> Vec<Particle> {
        let max_r = R_CYL - R_PEBBLE;
        let half = (max_r / SPACING).ceil() as i64 + 1;
        let mut v = Vec::new();
        for layer in 0..LAYERS {
            let z = R_PEBBLE * 1.03 + layer as f64 * SPACING;
            for ix in -half..=half {
                for iy in -half..=half {
                    let (x, y) = (ix as f64 * SPACING, iy as f64 * SPACING);
                    if (x * x + y * y).sqrt() <= max_r {
                        v.push(pebble(x, y, z, R_PEBBLE));
                    }
                }
            }
        }
        v
    }

    /// **Methodology.** Settle a small cylinder bed with liggghts'
    /// `DemSimulation` (Hooke contact, the parameters of its
    /// `examples/pebble_bed.rs`: `k_n = 1e5 N/m`, `gamma_n = 200 N s/m`,
    /// `k_t = 8e4 N/m`, `gamma_t = 200 N s/m`, `mu = 0.5`, `dt = 1e-4 s`),
    /// three seeded layers, 10 000 steps. Convert it and check: the count is
    /// preserved; every length is exactly `100x` the DEM value (bit-exact,
    /// since `x * 100.0` is what the converter does and the test recomputes
    /// it); every centre lies inside the container (radially within
    /// `R_cyl`, above the floor); and the reported overlap is consistent with
    /// a brute-force recount and small.
    ///
    /// The overlap bound (2 % of a diameter) is a sanity gate, not a physics
    /// claim. Its physical scale: a Hooke contact under one pebble's weight
    /// compresses by `m g / k_n = 0.198 kg * 9.81 / 1e5 N/m = 1.94e-5 m`
    /// (0.0194 mm), and the bottom contact of a three-high column carries two
    /// pebbles, so about `3.9e-3 cm` is expected there.
    ///
    /// **Result** (2026-10-02, release, 0.75 s for the whole test): **111
    /// pebbles**, settled to `KE = 1.7e-26 J`; radius exactly 3 cm; **max
    /// overlap 3.883e-3 cm = 6.47e-4 of a diameter**, matching the two-pebble
    /// load estimate above; **74 overlapping pairs** = 37 columns x 2 vertical
    /// contacts (the seed's 3 % lateral gap leaves no side contacts in an
    /// ordered lattice). The cell-list count and maximum equal a brute-force
    /// recount exactly.
    #[test]
    fn a_settled_cylinder_bed_converts_to_cm() {
        let particles = seed();
        let n = particles.len();
        let walls = vec![
            Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), R_CYL).expect("cylinder"),
            Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).expect("floor"),
        ];
        let contact = ContactModel::Hooke(
            HookeContact::new(1.0e5, 200.0, 8.0e4, 200.0, 0.5).expect("valid Hooke"),
        );
        let mut sim = DemSimulation::new(particles, walls, contact, Vec3::new(0.0, 0.0, -9.81), DT)
            .expect("valid simulation");
        sim.run(STEPS);
        let ke = sim.kinetic_energy();

        let bed = DemBed::from_dem_simulation(&sim).expect("monodisperse, finite bed");
        let brute = brute_force_overlap(bed.spheres(), bed.radius_cm());
        eprintln!(
            "dem_bed: {} pebbles, KE {ke:.3e} J, radius {} cm, max overlap {:.6e} cm \
             ({:.4e} of a diameter), {} overlapping pairs",
            bed.len(),
            bed.radius_cm(),
            bed.max_overlap_cm(),
            bed.max_relative_overlap(),
            bed.overlapping_pairs()
        );

        assert_eq!(bed.len(), n, "pebble count");
        assert_eq!(bed.radius_cm(), R_PEBBLE * 100.0);
        for (p, s) in sim.particles().iter().zip(bed.spheres()) {
            assert_eq!(s.center.x, p.position.x * 100.0);
            assert_eq!(s.center.y, p.position.y * 100.0);
            assert_eq!(s.center.z, p.position.z * 100.0);
            assert_eq!(s.radius, p.radius * 100.0);
            let rho = (s.center.x.powi(2) + s.center.y.powi(2)).sqrt();
            assert!(
                rho < R_CYL * 100.0,
                "centre outside the cylinder: rho = {rho} cm"
            );
            assert!(
                s.center.z > 0.0,
                "centre below the floor: z = {} cm",
                s.center.z
            );
        }
        assert!(ke < 1.0e-4, "bed did not settle: KE {ke:e} J");
        assert_eq!(bed.overlapping_pairs(), brute.1, "cell list missed a pair");
        assert_eq!(
            bed.max_overlap_cm(),
            brute.0,
            "cell list missed the worst pair"
        );
        assert!(
            bed.max_relative_overlap() < 0.02,
            "max overlap {} of a diameter",
            bed.max_relative_overlap()
        );
    }

    fn brute_force_overlap(s: &[Sphere], r: f64) -> (f64, usize) {
        let mut worst = 0.0_f64;
        let mut count = 0;
        for i in 0..s.len() {
            for j in (i + 1)..s.len() {
                let (a, b) = (s[i].center, s[j].center);
                let d = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2) + (a.z - b.z).powi(2)).sqrt();
                if d < 2.0 * r {
                    count += 1;
                    worst = worst.max(2.0 * r - d);
                }
            }
        }
        (worst, count)
    }

    /// **Methodology.** A polydisperse bed is refused with both extremes
    /// named, an empty one is refused, and a blown-up (NaN) particle is
    /// refused by index. Two pebbles overlapping by a known 1 cm report
    /// exactly that. `from_granular_system` is exercised on an unstepped
    /// system so the LIGGGHTS-faithful engine's path is covered too.
    ///
    /// **Result** (2026-10-02): all refusals as stated; overlap 1 cm, 1 pair.
    #[test]
    fn refusals_and_a_known_overlap() {
        assert_eq!(DemBed::from_particles(&[]), Err(DemBedError::Empty));

        let mixed = [pebble(0.0, 0.0, 0.1, 0.03), pebble(0.5, 0.0, 0.1, 0.031)];
        match DemBed::from_particles(&mixed) {
            Err(DemBedError::Polydisperse {
                min_radius_m,
                max_radius_m,
                ..
            }) => {
                assert_eq!((min_radius_m, max_radius_m), (0.03, 0.031));
            }
            other => panic!("expected Polydisperse, got {other:?}"),
        }

        let mut blown = [pebble(0.0, 0.0, 0.1, 0.03)];
        blown[0].position.x = f64::NAN;
        assert_eq!(
            DemBed::from_particles(&blown),
            Err(DemBedError::NonFinite { index: 0 })
        );

        // Centres 5 cm apart, diameter 6 cm: 1 cm overlap.
        let pair = vec![pebble(0.0, 0.0, 0.1, 0.03), pebble(0.05, 0.0, 0.1, 0.03)];
        let system = GranularSystem::new(
            pair,
            vec![],
            outram_park_fork_liggghts::granular::GranularContactModel::hertz_history(
                outram_park_fork_liggghts::granular::GranularMaterial::new(5.0e6, 0.3, 0.5, 0.3)
                    .expect("valid material"),
            ),
            Vec3::zero(),
            1.0e-6,
        )
        .expect("valid system");
        let bed = DemBed::from_granular_system(&system).expect("valid bed");
        assert_eq!(bed.overlapping_pairs(), 1);
        assert!(
            (bed.max_overlap_cm() - 1.0).abs() < 1e-12,
            "{}",
            bed.max_overlap_cm()
        );
        assert!((bed.max_relative_overlap() - 1.0 / 6.0).abs() < 1e-12);
    }
}
