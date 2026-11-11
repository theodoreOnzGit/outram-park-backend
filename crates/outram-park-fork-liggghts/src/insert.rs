// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// PORTED FROM UPSTREAM — provenance (see the crate NOTICE):
//   Upstream project : LIGGGHTS-PUBLIC (DCS Computing GmbH / JKU Linz)
//   Upstream files   : src/random_park.cpp (`RanPark`), src/random.cpp (seed
//                      rules), src/fix_insert.cpp (`FixInsert::pre_exchange`,
//                      `calc_ninsert_this`, `distribute_ninsert_this`,
//                      `generate_random_velocity`), src/fix_insert_pack.cpp
//                      (`calc_ninsert_this`, `calc_maxtry`, `x_v_omega`,
//                      `calc_insertion_properties`), src/fix_insert_rate_region.cpp,
//                      src/fix_particledistribution_discrete.cpp (weights, order,
//                      `randomize_list`), src/fix_template_sphere.cpp
//                      (`randomize_ptilist`, expectancies), src/particleToInsert.cpp
//                      (`check_near_set_x_v_omega`, `insert`), src/region.cpp
//                      (`generate_random*`, `volume_mc`, `match_cut`),
//                      src/region_block.cpp, src/region_cylinder.cpp
//                      (`inside`, `surface_interior`), src/region_neighbor_list_I.h
//                      (`hasOverlap`), src/domain_I.h (`dist_subbox_borders`)
//   Upstream commit  : 3d5c00f20519e6bb6eb6756f51f1ad36564e649d (2024-06-07)
//   Upstream licence : "GNU Public License, version 2 or later" -> used here
//                      under the "or later" option as GPL-3.0. `random_park.cpp`
//                      and the region files are LAMMPS originals, Copyright
//                      (2003) Sandia Corporation (GPL); the LIGGGHTS changes
//                      are Copyright 2012- DCS Computing GmbH, Linz and
//                      2009-2012 JKU Linz.
// Do not strip this block during refactors (workspace CLAUDE.md).
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # Particle insertion: `fix insert/pack`, `fix insert/rate/region`, templates, distributions
//!
//! gh:#1414 (child of #1405, differential issue #1406). A translation of
//! LIGGGHTS-PUBLIC's insertion machinery for **spheres**, so that a Rust pour
//! and a LIGGGHTS pour started from the same seeds place the same particles at
//! the same positions, bit for bit.
//!
//! | upstream | here |
//! |---|---|
//! | `RanPark` (Park–Miller minimal standard) | [`RanPark`] |
//! | `region block` / `region cylinder` (interior, `units box`) | [`InsertRegion`] |
//! | `fix particletemplate/sphere` (`radius constant`, `density constant`) | [`ParticleTemplateSphere`] |
//! | `fix particledistribution/discrete` (`/numberbased`) | [`ParticleDistributionDiscrete`] |
//! | `fix insert/pack`, `fix insert/rate/region` | [`InsertFix`] |
//!
//! ## Where it hooks into the time step
//!
//! `FixInsert::pre_exchange` runs on a step on which the fix asked for a
//! neighbour rebuild (`next_reneighbor == ntimestep`): after the integrator's
//! first half-step and before the force evaluation. New atoms are appended at
//! the end of the local list, get consecutive tags, and take the integrator's
//! *second* half-kick on that same step.
//! [`GranularSystem::add_insertion`](crate::granular_system::GranularSystem::add_insertion)
//! puts the call at exactly that point and forces the neighbour rebuild.
//!
//! ## What is translated, and what is deliberately not
//!
//! Translated: the RNG; the discrete distribution's weights, bubble-sorted
//! volume order and `randomize_list` (`random_distribute exact|uncorrelated`);
//! sphere templates with constant radius and density (the only styles upstream's
//! `particletemplate/sphere` accepts at this commit); `insert/pack`'s three
//! targets (`volumefraction_region`, `particles_in_region`, `mass_in_region`),
//! `ntry_mc`, `maxattempt`, `overlapcheck`, `all_in`, `vel constant|uniform|gaussian`,
//! `omega constant`, `insert_every`, `start`; `insert/rate/region`; the
//! Monte-Carlo region volume (`Region::volume_mc`), which also **consumes the
//! region's random stream** so the first insertion position is the same.
//!
//! Single process, orthogonal fixed box: `distribute_ninsert_this` reduces to
//! "all of them here" and the sub-domain is the box. Not translated:
//! multi-sphere/superquadric templates, `orientation random` (it draws a
//! quaternion per trial, so it would need `random_unit_quat`; rejected with an
//! error rather than silently skipping the draw), `set_property`, dynamic
//! regions, `side out`, `insert/stream` (gh:#1414 remainder).
//!
//! ## The overlap test
//!
//! Upstream asks a `RegionNeighborList` (a bin grid of cell `4 * maxrad`)
//! whether a trial position overlaps any particle already there, `rsq <=
//! (r + r_j)^2`. The answer is a boolean, so the bins only affect speed; this
//! module uses a uniform grid with the same predicate. Upstream loads only
//! atoms near the region (`is_nearby`); an atom that overlaps a trial that is
//! `all_in` the region is necessarily near it, so the sets agree for every
//! trial `all_in yes` can make (an `all_in no` trial centred in the region can
//! overlap an atom up to `r_j` outside, which `is_nearby` also keeps).

use crate::particle::{Particle, Vec3};
use crate::DemError;
use std::collections::BTreeMap;
use uom::si::f64::{Length, MassDensity, Velocity};
use uom::si::{length::meter, mass_density::kilogram_per_cubic_meter, velocity::meter_per_second};

const IA: i32 = 16807;
const IM: i32 = 2_147_483_647;
const IQ: i32 = 127_773;
const IR: i32 = 2836;
/// `SEED_OFFSET` of `fix_insert_pack.cpp`: the region's stream is reset to
/// `seed + 12`.
const SEED_OFFSET: i32 = 12;
/// `SMALL` of `region.cpp` / `SMALL_DMBRDR` of `domain.h`.
const SMALL: f64 = 1.0e-8;

/// LAMMPS' `RanPark`: the Park–Miller "minimal standard" generator
/// (`IA = 16807`, `IM = 2^31 - 1`, Schrage's method), with the polar Box–Muller
/// `gaussian()`.
///
/// Bit-identical to upstream by construction: pure integer arithmetic for the
/// state, one multiplication by `1/IM` for the output.
#[derive(Debug, Clone, PartialEq)]
pub struct RanPark {
    seed: i32,
    save: bool,
    second: f64,
}

impl RanPark {
    /// A generator with the given seed.
    ///
    /// Upstream's `Random` constructor additionally requires the seed to be
    /// `> 10000` and prime (an error unless the input script relaxes it); that
    /// rule is applied by [`RanPark::checked`], not here, because the region
    /// generator starts from the unchecked default `3012211`... which is prime
    /// anyway. The Park–Miller recurrence itself needs only `seed > 0`.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if `seed <= 0` (`"Invalid seed for Park random
    /// # generator"`).
    pub fn new(seed: i32) -> Result<Self, DemError> {
        if seed <= 0 {
            return Err(DemError::InvalidInput(format!(
                "Invalid seed for Park random # generator: {seed}"
            )));
        }
        Ok(Self { seed, save: false, second: 0.0 })
    }

    /// [`RanPark::new`] plus `Random::Random`'s rule that the seed is
    /// `> 10000` and prime.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if either condition fails.
    pub fn checked(seed: i32) -> Result<Self, DemError> {
        if seed < 10_000 || !is_prime(seed) {
            return Err(DemError::InvalidInput(format!(
                "Random number generation: the random seed must be > 10000 and prime, got {seed} \
                 (e.g. 15485863, 15485867, 32452843, 32452867)"
            )));
        }
        Self::new(seed)
    }

    /// The current state (`RanPark::state()`).
    #[must_use]
    pub fn state(&self) -> i32 {
        self.seed
    }

    /// `RanPark::reset(int)`.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if `seed <= 0`.
    pub fn reset(&mut self, seed: i32) -> Result<(), DemError> {
        if seed <= 0 {
            return Err(DemError::InvalidInput(format!(
                "Invalid seed for Park random # generator: {seed}"
            )));
        }
        self.seed = seed;
        self.save = false;
        Ok(())
    }

    /// `RanPark::uniform()`: uniform in `(0, 1)`.
    pub fn uniform(&mut self) -> f64 {
        let k = self.seed / IQ;
        self.seed = IA * (self.seed - k * IQ) - IR * k;
        if self.seed < 0 {
            self.seed += IM;
        }
        // `AM*seed` with `#define AM (1.0/IM)`.
        (1.0 / f64::from(IM)) * f64::from(self.seed)
    }

    /// `RanPark::gaussian()`: polar Box–Muller, one value per call (the second
    /// of each pair is kept).
    pub fn gaussian(&mut self) -> f64 {
        if !self.save {
            let (mut v1, mut v2, mut rsq);
            loop {
                v1 = 2.0 * self.uniform() - 1.0;
                v2 = 2.0 * self.uniform() - 1.0;
                rsq = v1 * v1 + v2 * v2;
                if rsq < 1.0 && rsq != 0.0 {
                    break;
                }
            }
            let fac = (-2.0 * rsq.ln() / rsq).sqrt();
            self.second = v1 * fac;
            self.save = true;
            v2 * fac
        } else {
            self.save = false;
            self.second
        }
    }
}

fn is_prime(n: i32) -> bool {
    if n < 2 {
        return false;
    }
    let mut i = 2;
    while i * i <= n {
        if n % i == 0 {
            return false;
        }
        i += 1;
    }
    true
}

/// Principal axis of a cylinder region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// Along x.
    X,
    /// Along y.
    Y,
    /// Along z.
    Z,
}

/// An interior insertion region (`region ... units box`, no `side out`, not
/// dynamic): `block` or `cylinder`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InsertRegion {
    /// `region ID block xlo xhi ylo yhi zlo zhi`.
    Block {
        /// Lower corner `[m]`.
        lo: Vec3,
        /// Upper corner `[m]`.
        hi: Vec3,
    },
    /// `region ID cylinder axis c1 c2 radius lo hi`.
    Cylinder {
        /// Cylinder axis.
        axis: Axis,
        /// First in-plane centre coordinate `[m]` (y for x-axis, x otherwise).
        c1: f64,
        /// Second in-plane centre coordinate `[m]` (z for x- and y-axis, y for z).
        c2: f64,
        /// Radius `[m]`.
        radius: f64,
        /// Lower end along the axis `[m]`.
        lo: f64,
        /// Upper end along the axis `[m]`.
        hi: f64,
    },
}

impl InsertRegion {
    /// `region ID block xlo xhi ylo yhi zlo zhi`.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if a lower bound exceeds its upper bound
    /// (`"Illegal region block command"`).
    pub fn block(lo: Vec3, hi: Vec3) -> Result<Self, DemError> {
        if lo.x > hi.x || lo.y > hi.y || lo.z > hi.z {
            return Err(DemError::InvalidInput("Illegal region block command".into()));
        }
        Ok(Self::Block { lo, hi })
    }

    /// `region ID cylinder axis c1 c2 radius lo hi`.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if `radius <= 0` (`"Illegal region cylinder
    /// command"`).
    pub fn cylinder(axis: Axis, c1: Length, c2: Length, radius: Length, lo: Length, hi: Length) -> Result<Self, DemError> {
        let radius = radius.get::<meter>();
        if radius <= 0.0 {
            return Err(DemError::InvalidInput("Illegal region cylinder command".into()));
        }
        Ok(Self::Cylinder {
            axis,
            c1: c1.get::<meter>(),
            c2: c2.get::<meter>(),
            radius,
            lo: lo.get::<meter>(),
            hi: hi.get::<meter>(),
        })
    }

    /// The bounding box `(lo, hi)` (`extent_*lo/hi`).
    #[must_use]
    pub fn extent(&self) -> (Vec3, Vec3) {
        match *self {
            Self::Block { lo, hi } => (lo, hi),
            Self::Cylinder { axis, c1, c2, radius, lo, hi } => match axis {
                Axis::X => (Vec3::new(lo, c1 - radius, c2 - radius), Vec3::new(hi, c1 + radius, c2 + radius)),
                Axis::Y => (Vec3::new(c1 - radius, lo, c2 - radius), Vec3::new(c1 + radius, hi, c2 + radius)),
                Axis::Z => (Vec3::new(c1 - radius, c2 - radius, lo), Vec3::new(c1 + radius, c2 + radius, hi)),
            },
        }
    }

    /// `Region::match` for an interior region: inside, or on the surface.
    #[must_use]
    pub fn contains(&self, p: Vec3) -> bool {
        match *self {
            Self::Block { lo, hi } => {
                p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y && p.z >= lo.z && p.z <= hi.z
            }
            Self::Cylinder { axis, c1, c2, radius, lo, hi } => {
                let (d1, d2, a) = Self::cyl_coords(axis, c1, c2, p);
                let dist = (d1 * d1 + d2 * d2).sqrt();
                dist <= radius && a >= lo && a <= hi
            }
        }
    }

    /// `Region::match_cut`: `surface_interior(x, cut) != 0`, i.e. the point is
    /// inside the region and closer than `cut` to at least one inner surface.
    #[must_use]
    pub fn within_cut_of_surface(&self, p: Vec3, cut: f64) -> bool {
        match *self {
            Self::Block { lo, hi } => {
                if p.x < lo.x || p.x > hi.x || p.y < lo.y || p.y > hi.y || p.z < lo.z || p.z > hi.z {
                    return false;
                }
                p.x - lo.x < cut
                    || hi.x - p.x < cut
                    || p.y - lo.y < cut
                    || hi.y - p.y < cut
                    || p.z - lo.z < cut
                    || hi.z - p.z < cut
            }
            Self::Cylinder { axis, c1, c2, radius, lo, hi } => {
                let (d1, d2, a) = Self::cyl_coords(axis, c1, c2, p);
                let r = (d1 * d1 + d2 * d2).sqrt();
                if r > radius || a < lo || a > hi {
                    return false;
                }
                (radius - r < cut && r > 0.0) || a - lo < cut || hi - a < cut
            }
        }
    }

    fn cyl_coords(axis: Axis, c1: f64, c2: f64, p: Vec3) -> (f64, f64, f64) {
        match axis {
            Axis::X => (p.y - c1, p.z - c2, p.x),
            Axis::Y => (p.x - c1, p.z - c2, p.y),
            Axis::Z => (p.x - c1, p.y - c2, p.z),
        }
    }
}

/// `fix particletemplate/sphere SEED atom_type T density constant RHO radius
/// constant R` (the only radius/density styles upstream accepts for it).
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleTemplateSphere {
    /// Seed of the template's `random_insertion` stream (kept for the
    /// distinct-seeds rule; constants draw nothing).
    pub seed: i32,
    /// `atom_type` (1-based, as in the deck).
    pub atom_type: usize,
    radius: f64,
    density: f64,
    random_insertion: RanPark,
}

/// One particle to insert, as `ParticleToInsert` holds it for a single sphere.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleToInsert {
    /// Sphere radius `[m]`.
    pub radius: f64,
    /// Density `[kg/m^3]`.
    pub density: f64,
    /// `radius^3 * 4 * pi / 3`, in upstream's operation order `[m^3]`.
    pub volume: f64,
    /// `density * volume` `[kg]`.
    pub mass: f64,
    /// `atom_type` (1-based).
    pub atom_type: usize,
}

impl ParticleTemplateSphere {
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for an invalid seed (see [`RanPark::checked`]),
    /// `atom_type < 1`, or a non-positive radius or density.
    pub fn new(seed: i32, atom_type: usize, radius: Length, density: MassDensity) -> Result<Self, DemError> {
        let (radius, density) = (radius.get::<meter>(), density.get::<kilogram_per_cubic_meter>());
        if atom_type < 1 {
            return Err(DemError::InvalidInput("invalid atom type (must be >=1)".into()));
        }
        if radius <= 0.0 {
            return Err(DemError::InvalidInput("radius must be > 0".into()));
        }
        if density <= 0.0 {
            return Err(DemError::InvalidInput("density must be > 0".into()));
        }
        Ok(Self { seed, atom_type, radius, density, random_insertion: RanPark::checked(seed)? })
    }

    /// `volume_expect = cubic_expectancy * 4 * pi / 3` (upstream's order).
    #[must_use]
    pub fn volexpect(&self) -> f64 {
        (self.radius * self.radius * self.radius) * 4.0 * std::f64::consts::PI / 3.0
    }

    /// `mass_expect = E[density] * volume_expect`.
    #[must_use]
    pub fn massexpect(&self) -> f64 {
        self.density * self.volexpect()
    }

    /// `randomize_ptilist` for one entry.
    fn randomize_one(&self) -> ParticleToInsert {
        let radius = self.radius;
        let density = self.density;
        let volume = radius * radius * radius * 4.0 * std::f64::consts::PI / 3.0;
        ParticleToInsert { radius, density, volume, mass: density * volume, atom_type: self.atom_type }
    }
}

/// `fix particledistribution/discrete SEED N T1 W1 ...`
/// (`ParticleDistributionDiscrete::new(.., mass_based = true)`), or
/// `particledistribution/discrete/numberbased` (`mass_based = false`).
#[derive(Debug, Clone, PartialEq)]
pub struct ParticleDistributionDiscrete {
    random: RanPark,
    templates: Vec<ParticleTemplateSphere>,
    distweight: Vec<f64>,
    distorder: Vec<usize>,
    volexpect: f64,
    massexpect: f64,
    maxrad: f64,
}

impl ParticleDistributionDiscrete {
    /// Build the distribution; weights are normalised and, for `mass_based`,
    /// converted from mass % to number %, with upstream's arithmetic order.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for an invalid seed, no templates, a negative
    /// weight, or a template volume below `1e-12` (`volume_limit`).
    pub fn new(seed: i32, mass_based: bool, templates: Vec<(ParticleTemplateSphere, f64)>) -> Result<Self, DemError> {
        let random = RanPark::checked(seed)?;
        let nt = templates.len();
        if nt < 1 {
            return Err(DemError::InvalidInput("illegal number of templates".into()));
        }
        let (templates, mut distweight): (Vec<_>, Vec<_>) = templates.into_iter().unzip();
        if distweight.iter().any(|w| *w < 0.0) {
            return Err(DemError::InvalidInput("invalid weight".into()));
        }
        for t in &templates {
            if t.volexpect() < 1e-12 {
                return Err(DemError::InvalidInput(
                    "Volume expectancy too small (template volume < volume_limit 1e-12)".into(),
                ));
            }
        }
        let mut weightsum = 0.0;
        for w in &distweight {
            weightsum += *w;
        }
        for w in &mut distweight {
            *w /= weightsum;
        }
        if mass_based {
            for (w, t) in distweight.iter_mut().zip(&templates) {
                *w /= t.massexpect();
            }
            let mut weightsum = 0.0;
            for w in &distweight {
                weightsum += *w;
            }
            for w in &mut distweight {
                *w /= weightsum;
            }
        }
        let (mut volexpect, mut massexpect) = (0.0, 0.0);
        for (t, w) in templates.iter().zip(&distweight) {
            volexpect += t.volexpect() * *w;
            massexpect += t.massexpect() * *w;
        }
        // Bubble sort by insertion volume, descending (verbatim, including the
        // inner loop that ignores the shrinking `n`).
        let mut distorder: Vec<usize> = (0..nt).collect();
        let mut n = nt as i64;
        loop {
            let mut swapped = false;
            for i in 0..nt.saturating_sub(1) {
                if templates[distorder[i]].volexpect() < templates[distorder[i + 1]].volexpect() {
                    distorder.swap(i, i + 1);
                    swapped = true;
                }
            }
            n -= 1;
            if !(swapped && n > 0) {
                break;
            }
        }
        let maxrad = templates.iter().fold(0.0_f64, |m, t| if t.radius > m { t.radius } else { m });
        Ok(Self { random, templates, distweight, distorder, volexpect, massexpect, maxrad })
    }

    /// Number-based weights after normalisation (`distweight`).
    #[must_use]
    pub fn number_weights(&self) -> &[f64] {
        &self.distweight
    }

    /// Template indices from the largest to the smallest insertion volume
    /// (`distorder`).
    #[must_use]
    pub fn order(&self) -> &[usize] {
        &self.distorder
    }

    /// Expected volume per inserted particle (`vol_expect()`).
    #[must_use]
    pub fn vol_expect(&self) -> f64 {
        self.volexpect
    }

    /// Expected mass per inserted particle (`mass_expect()`).
    #[must_use]
    pub fn mass_expect(&self) -> f64 {
        self.massexpect
    }

    /// Largest radius over the templates (`max_r_bound()` for spheres) `[m]`.
    #[must_use]
    pub fn max_r_bound(&self) -> f64 {
        self.maxrad
    }

    fn seeds(&self) -> Vec<i32> {
        let mut s = vec![self.random.state()];
        s.extend(self.templates.iter().map(|t| t.random_insertion.state()));
        s
    }

    /// `randomize_list(ntotal, ., exact_number)`: the list of particles to
    /// insert, ordered large to small.
    ///
    /// `exact_number`: truncate the distribution so exactly `ntotal` are
    /// generated, spreading the remainder by `random->uniform()` draws;
    /// otherwise each template gets `int(ntotal * w + uniform())`.
    pub fn randomize_list(&mut self, ntotal: usize, exact_number: bool) -> Vec<ParticleToInsert> {
        let nt = self.templates.len();
        let ninsert = ntotal as f64;
        let mut parttogen = vec![0_usize; nt];
        if !exact_number {
            for i in 0..nt {
                parttogen[i] = (ninsert * self.distweight[i] + self.random.uniform()) as usize;
            }
        } else {
            let mut truncated = 0_usize;
            let mut remainder = vec![0.0; nt];
            for i in 0..nt {
                parttogen[i] = (ninsert * self.distweight[i]) as usize;
                truncated += parttogen[i];
                remainder[i] = ninsert * self.distweight[i] - parttogen[i] as f64;
            }
            let gap = ntotal - truncated;
            for _ in 0..gap {
                let r = self.random.uniform() * gap as f64;
                let mut j = 0;
                let mut rsum = remainder[0];
                while rsum < r && j < nt - 1 {
                    j += 1;
                    rsum += remainder[j];
                }
                parttogen[j] += 1;
            }
        }
        let mut out = Vec::with_capacity(parttogen.iter().sum());
        for &chosen in &self.distorder {
            for _ in 0..parttogen[chosen] {
                out.push(self.templates[chosen].randomize_one());
            }
        }
        out
    }
}

/// How much `fix insert/pack` aims to have in its region.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PackTarget {
    /// `volumefraction_region` (0 < f <= 1).
    VolumeFraction(f64),
    /// `particles_in_region`.
    Particles(usize),
    /// `mass_in_region` `[kg]`.
    Mass(f64),
}

/// `vel constant|uniform|gaussian`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InsertVelocity {
    /// `vel constant vx vy vz`.
    Constant(Vec3),
    /// `vel uniform vx vy vz dvx dvy dvz`: `v + dv * 2 * (u - 0.5)` per axis.
    Uniform {
        /// Mean `[m/s]`.
        v: Vec3,
        /// Half-width `[m/s]`.
        fluct: Vec3,
    },
    /// `vel gaussian vx vy vz sx sy sz`: `v + s * N(0,1)` per axis.
    Gaussian {
        /// Mean `[m/s]`.
        v: Vec3,
        /// Standard deviation `[m/s]`.
        fluct: Vec3,
    },
}

/// What the fix does each insertion step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InsertMode {
    /// `fix insert/pack`: top the region up to a target.
    Pack(PackTarget),
    /// `fix insert/rate/region`: a fixed number or mass per unit time.
    RateRegion {
        /// `nparticles N` / `mass M`; `None` for `INF`.
        total: Option<RateTotal>,
        /// `particlerate` (particles/s) or `massrate` (kg/s).
        rate: Rate,
        /// Time step `[s]` (`update->dt`), for `ninsert_per`.
        dt: f64,
    },
}

/// Total to insert for `insert/rate/region`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RateTotal {
    /// `nparticles N`.
    Particles(usize),
    /// `mass M` `[kg]`.
    Mass(f64),
}

/// Flow rate for `insert/rate/region`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rate {
    /// `particlerate` `[1/s]`.
    Particles(f64),
    /// `massrate` `[kg/s]`.
    Mass(f64),
}

/// A particle ready to be appended to the ensemble.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InsertedParticle {
    /// The particle (position, velocity, omega, radius, mass; 300 K).
    pub particle: Particle,
    /// `atom_type` (1-based).
    pub atom_type: usize,
    /// Density `[kg/m^3]`.
    pub density: f64,
}

/// A `fix insert/pack` or `fix insert/rate/region`.
#[derive(Debug, Clone, PartialEq)]
pub struct InsertFix {
    mode: InsertMode,
    region: InsertRegion,
    distribution: ParticleDistributionDiscrete,
    random: RanPark,
    region_random: RanPark,
    seed: i32,
    box_lo: Vec3,
    box_hi: Vec3,
    insert_every: u64,
    next_reneighbor: Option<u64>,
    most_recent_ins_step: Option<u64>,
    overlap_check: bool,
    all_in: bool,
    velocity: InsertVelocity,
    omega: Vec3,
    exact_number: bool,
    maxattempt: i64,
    ntry_mc: usize,
    check_dist_from_subdomain_border: bool,
    // derived at set-up
    region_volume: f64,
    region_volume_local: f64,
    ninsert_exists: bool,
    ninsert: i64,
    ninsert_per: f64,
    ninserted: i64,
    massinserted: f64,
    insertion_ratio: f64,
}

impl InsertFix {
    /// `fix ID all insert/pack|insert/rate/region seed S distributiontemplate D
    /// insert_every E region R <target> ...`, including upstream's set-up
    /// (`calc_insertion_properties`): the region's stream is reset to `seed +
    /// 12` and `ntry_mc` Monte-Carlo points are drawn to measure its volume.
    ///
    /// `insert_every = 0` is `insert_every once`. The first insertion is at step
    /// `start` (default 1). The box is the simulation box (the sub-domain on a
    /// single process).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for the errors upstream raises: invalid seeds,
    /// equal seeds among the fix, its distribution and its templates, a region
    /// outside the box, a failed volume estimate, `insert_every once` with
    /// `insert/rate/region`, a missing or doubled target.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        mode: InsertMode,
        seed: i32,
        distribution: ParticleDistributionDiscrete,
        region: InsertRegion,
        insert_every: u64,
        box_lo: Vec3,
        box_hi: Vec3,
    ) -> Result<Self, DemError> {
        let random = RanPark::checked(seed)?;
        let mut seeds = vec![random.state()];
        seeds.extend(distribution.seeds());
        let n = seeds.len();
        seeds.sort_unstable();
        seeds.dedup();
        if seeds.len() != n {
            return Err(DemError::InvalidInput(
                "Random number generation: all random seeds of the insert fix, its distribution and its \
                 templates must differ"
                    .into(),
            ));
        }
        let mut fix = Self {
            mode,
            region,
            distribution,
            random,
            region_random: RanPark::new(3_012_211)?,
            seed,
            box_lo,
            box_hi,
            insert_every,
            next_reneighbor: Some(1),
            most_recent_ins_step: None,
            overlap_check: true,
            all_in: false,
            velocity: InsertVelocity::Constant(Vec3::zero()),
            omega: Vec3::zero(),
            exact_number: true,
            maxattempt: 50,
            ntry_mc: 100_000,
            check_dist_from_subdomain_border: true,
            region_volume: 0.0,
            region_volume_local: 0.0,
            ninsert_exists: true,
            ninsert: 0,
            ninsert_per: 0.0,
            ninserted: 0,
            massinserted: 0.0,
            insertion_ratio: 0.0,
        };
        // `insert/pack` has no fixed total.
        if matches!(mode, InsertMode::Pack(_)) {
            fix.ninsert_exists = false;
        }
        Ok(fix)
    }

    /// `overlapcheck yes|no` (default yes).
    #[must_use]
    pub fn with_overlap_check(mut self, on: bool) -> Self {
        self.overlap_check = on;
        self
    }

    /// `all_in yes|no` (default no): keep whole particles inside the region.
    #[must_use]
    pub fn with_all_in(mut self, on: bool) -> Self {
        self.all_in = on;
        self
    }

    /// `vel ...` (default constant zero).
    #[must_use]
    pub fn with_velocity(mut self, v: InsertVelocity) -> Self {
        self.velocity = v;
        self
    }

    /// `vel constant` from uom velocities.
    #[must_use]
    pub fn with_constant_velocity(self, vx: Velocity, vy: Velocity, vz: Velocity) -> Self {
        self.with_velocity(InsertVelocity::Constant(Vec3::new(
            vx.get::<meter_per_second>(),
            vy.get::<meter_per_second>(),
            vz.get::<meter_per_second>(),
        )))
    }

    /// `omega constant wx wy wz` `[rad/s]`.
    #[must_use]
    pub fn with_omega(mut self, w: Vec3) -> Self {
        self.omega = w;
        self
    }

    /// `start N`: the first insertion step (default 1).
    #[must_use]
    pub fn with_start(mut self, step: u64) -> Self {
        self.next_reneighbor = Some(step);
        self
    }

    /// `maxattempt N` (default 50).
    #[must_use]
    pub fn with_maxattempt(mut self, n: usize) -> Self {
        self.maxattempt = n as i64;
        self
    }

    /// `ntry_mc N` (default 100000; upstream requires >= 1000).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if `n < 1000`.
    pub fn with_ntry_mc(mut self, n: usize) -> Result<Self, DemError> {
        if n < 1000 {
            return Err(DemError::InvalidInput("ntry_mc must be > 1000".into()));
        }
        self.ntry_mc = n;
        Ok(self)
    }

    /// `random_distribute exact|uncorrelated` (default exact).
    #[must_use]
    pub fn with_exact_number(mut self, exact: bool) -> Self {
        self.exact_number = exact;
        self
    }

    /// `check_dist_from_subdomain_border yes|no` (default yes).
    #[must_use]
    pub fn with_check_dist_from_subdomain_border(mut self, on: bool) -> Self {
        self.check_dist_from_subdomain_border = on;
        self
    }

    /// Run upstream's set-up (`FixInsert::setup` -> `calc_insertion_properties`).
    /// Call once, after the builder methods and before the first
    /// [`InsertFix::pre_exchange`]; [`GranularSystem::add_insertion`] does.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for the checks of `calc_insertion_properties`.
    ///
    /// [`GranularSystem::add_insertion`]: crate::granular_system::GranularSystem::add_insertion
    pub fn setup(&mut self) -> Result<(), DemError> {
        self.region_random.reset(self.seed + SEED_OFFSET)?;
        self.volume_mc()?;
        let (rlo, rhi) = self.region.extent();
        if !(self.in_domain(Vec3::new(rlo.x + SMALL, rlo.y + SMALL, rlo.z + SMALL))
            && self.in_domain(Vec3::new(rhi.x - SMALL, rhi.y - SMALL, rhi.z - SMALL)))
        {
            return Err(DemError::InvalidInput(
                "Insertion region extends outside simulation box and a fixed boundary is used. \
                 Please use non-fixed boundaries in this case only"
                    .into(),
            ));
        }
        match self.mode {
            InsertMode::Pack(target) => {
                if self.region_volume <= 0.0
                    || self.region_volume_local < 0.0
                    || (self.region_volume_local - self.region_volume) / self.region_volume > 1e-3
                {
                    return Err(DemError::InvalidInput("Fix insert: Region volume calculation with MC failed".into()));
                }
                let ok = match target {
                    PackTarget::VolumeFraction(f) => (0.0..=1.0).contains(&f) && f > 0.0,
                    PackTarget::Particles(n) => n > 0,
                    PackTarget::Mass(m) => m > 0.0,
                };
                if !ok {
                    return Err(DemError::InvalidInput(
                        "must define exactly one valid keyword out of 'volumefraction_region', \
                         'particles_in_region', and 'mass_in_region'"
                            .into(),
                    ));
                }
            }
            InsertMode::RateRegion { total, rate, dt } => {
                if self.region_volume <= 0.0
                    || self.region_volume_local < 0.0
                    || self.region_volume_local > self.region_volume + f64::EPSILON
                {
                    return Err(DemError::InvalidInput("Fix insert: Region volume calculation with MC failed".into()));
                }
                if self.insert_every == 0 {
                    return Err(DemError::InvalidInput("'insert_every' = once not allowed".into()));
                }
                let mass_expect = self.distribution.mass_expect();
                self.ninsert_exists = total.is_some();
                if let Some(t) = total {
                    self.ninsert = match t {
                        RateTotal::Particles(n) => n as i64,
                        RateTotal::Mass(m) => (m / mass_expect) as i64,
                    };
                }
                let nflowrate = match rate {
                    Rate::Particles(n) => n,
                    Rate::Mass(m) => m / mass_expect,
                };
                if nflowrate <= 0.0 {
                    return Err(DemError::InvalidInput("must define either 'massrate' or 'particlerate'".into()));
                }
                self.ninsert_per = nflowrate * ((self.insert_every as f64) * dt);
            }
        }
        Ok(())
    }

    fn in_domain(&self, p: Vec3) -> bool {
        p.x >= self.box_lo.x
            && p.x <= self.box_hi.x
            && p.y >= self.box_lo.y
            && p.y <= self.box_hi.y
            && p.z >= self.box_lo.z
            && p.z <= self.box_hi.z
    }

    /// `Domain::is_in_subdomain` on one process: the sub-domain is the box, padded
    /// by `SMALL_DMBRDR` where it touches the box (always, here).
    fn in_subdomain(&self, p: Vec3) -> bool {
        p.x >= self.box_lo.x - SMALL
            && p.x < self.box_hi.x + SMALL
            && p.y >= self.box_lo.y - SMALL
            && p.y < self.box_hi.y + SMALL
            && p.z >= self.box_lo.z - SMALL
            && p.z < self.box_hi.z + SMALL
    }

    /// `Region::volume_mc` (one process). Draws `3 * ntry_mc` numbers from the
    /// region's stream, which is what makes the first insertion position match.
    fn volume_mc(&mut self) -> Result<(), DemError> {
        let (lo, hi) = self.region.extent();
        let cut = self.distribution.max_r_bound();
        let (mut n_in_local, mut n_in_global) = (0_i64, 0_i64);
        for _ in 0..self.ntry_mc {
            let pos = Vec3::new(
                lo.x + self.region_random.uniform() * (hi.x - lo.x),
                lo.y + self.region_random.uniform() * (hi.y - lo.y),
                lo.z + self.region_random.uniform() * (hi.z - lo.z),
            );
            if !self.in_domain(pos) {
                continue;
            }
            if self.region.contains(pos) {
                n_in_global += 1;
                if self.in_subdomain(pos) && !(self.all_in && self.region.within_cut_of_surface(pos, cut)) {
                    n_in_local += 1;
                }
            }
        }
        if n_in_global == 0 {
            return Err(DemError::InvalidInput(
                "Unable to calculate region volume (region too small, outside the domain, or the particles \
                 too large for all_in yes)"
                    .into(),
            ));
        }
        let vol_bbox = (hi.x - lo.x) * (hi.y - lo.y) * (hi.z - lo.z);
        let n_test = self.ntry_mc as f64;
        let vol_global = (n_in_global as f64) / (n_test * 1.0) * vol_bbox;
        let mut vol_local = (n_in_local as f64) / n_test * vol_bbox;
        let vol_local_all = vol_local;
        if vol_local_all < 1.0e-10 {
            return Err(DemError::InvalidInput("Unable to calculate region volume (volume_limit)".into()));
        }
        vol_local *= vol_global / vol_local_all;
        self.region_volume = vol_global;
        self.region_volume_local = vol_local;
        Ok(())
    }

    /// Monte-Carlo volume of the region `[m^3]` (`region_volume`).
    #[must_use]
    pub fn region_volume(&self) -> f64 {
        self.region_volume
    }

    /// Largest radius this fix can insert `[m]` (`max_rad`): the neighbour cut
    /// uses it before any such particle exists.
    #[must_use]
    pub fn max_rad(&self) -> f64 {
        self.distribution.max_r_bound()
    }

    /// Particles inserted so far (`ninserted`).
    #[must_use]
    pub fn ninserted(&self) -> usize {
        self.ninserted as usize
    }

    /// Mass inserted so far `[kg]` (`massinserted`).
    #[must_use]
    pub fn massinserted(&self) -> f64 {
        self.massinserted
    }

    /// The step of the next insertion, if any (`next_reneighbor`).
    #[must_use]
    pub fn next_insertion_step(&self) -> Option<u64> {
        self.next_reneighbor
    }

    /// `Region::generate_random` / `generate_random_shrinkby_cut`, sub-domain =
    /// box.
    fn generate_random(&mut self, shrink_by: Option<f64>) -> Result<Vec3, DemError> {
        let (elo, ehi) = self.region.extent();
        let lo = Vec3::new(elo.x.max(self.box_lo.x), elo.y.max(self.box_lo.y), elo.z.max(self.box_lo.z));
        let hi = Vec3::new(ehi.x.min(self.box_hi.x), ehi.y.min(self.box_hi.y), ehi.z.min(self.box_hi.z));
        if lo.x >= hi.x || lo.y >= hi.y || lo.z >= hi.z {
            return Err(DemError::InvalidInput("Impossible to generate random points on wrong sub-domain".into()));
        }
        let diff = Vec3::new(hi.x - lo.x, hi.y - lo.y, hi.z - lo.z);
        if let Some(cut) = shrink_by {
            if (ehi.x - elo.x < 2.0 * cut) || (ehi.y - elo.y < 2.0 * cut) || (ehi.z - elo.z < 2.0 * cut) {
                return Err(DemError::InvalidInput(
                    "Impossible to generate random points within region - region too small (smaller than twice \
                     the particle cutoff)"
                        .into(),
                ));
            }
        }
        loop {
            let x = lo.x + self.region_random.uniform() * diff.x;
            let y = lo.y + self.region_random.uniform() * diff.y;
            let z = lo.z + self.region_random.uniform() * diff.z;
            let pos = Vec3::new(x, y, z);
            let ok = match shrink_by {
                None => self.region.contains(pos),
                Some(cut) => self.region.contains(pos) && !self.region.within_cut_of_surface(pos, cut),
            };
            if ok {
                return Ok(pos);
            }
        }
    }

    /// `Domain::dist_subbox_borders`.
    fn dist_subbox_borders(&self, p: Vec3) -> f64 {
        let dlo = (self.box_lo.x - p.x).abs().min((self.box_lo.y - p.y).abs()).min((self.box_lo.z - p.z).abs());
        let dhi = (self.box_hi.x - p.x).abs().min((self.box_hi.y - p.y).abs()).min((self.box_hi.z - p.z).abs());
        if dlo < dhi {
            dlo
        } else {
            dhi
        }
    }

    /// `FixInsert::generate_random_velocity` (draws from the fix's stream).
    fn generate_random_velocity(&mut self) -> Vec3 {
        match self.velocity {
            InsertVelocity::Constant(v) => v,
            InsertVelocity::Uniform { v, fluct } => {
                let x = v.x + fluct.x * 2.0 * (self.random.uniform() - 0.50);
                let y = v.y + fluct.y * 2.0 * (self.random.uniform() - 0.50);
                let z = v.z + fluct.z * 2.0 * (self.random.uniform() - 0.50);
                Vec3::new(x, y, z)
            }
            InsertVelocity::Gaussian { v, fluct } => {
                let x = v.x + fluct.x * self.random.gaussian();
                let y = v.y + fluct.y * self.random.gaussian();
                let z = v.z + fluct.z * self.random.gaussian();
                Vec3::new(x, y, z)
            }
        }
    }

    /// The number of particles to try this step (`calc_ninsert_this`).
    fn calc_ninsert_this(&mut self, atoms: &[Particle]) -> i64 {
        match self.mode {
            InsertMode::Pack(target) => {
                let four_pi_3 = 4.0 * std::f64::consts::PI / 3.0;
                let (mut np_region, mut vol_region, mut mass_region) = (0_i64, 0.0, 0.0);
                for a in atoms {
                    if self.region.contains(a.position) {
                        np_region += 1;
                        vol_region += four_pi_3 * a.radius * a.radius * a.radius;
                        mass_region += a.mass;
                    }
                }
                let mut n = match target {
                    PackTarget::VolumeFraction(f) => {
                        let n = ((f * self.region_volume - vol_region) / self.distribution.vol_expect()
                            + self.random.uniform()) as i64;
                        self.insertion_ratio = vol_region / (f * self.region_volume);
                        n
                    }
                    PackTarget::Particles(total) => {
                        self.insertion_ratio = np_region as f64 / total as f64;
                        total as i64 - np_region
                    }
                    PackTarget::Mass(total) => {
                        let n = ((total - mass_region) / self.distribution.mass_expect() + self.random.uniform()) as i64;
                        self.insertion_ratio = mass_region / total;
                        n
                    }
                };
                if n < 0 {
                    n = 0;
                }
                self.insertion_ratio = self.insertion_ratio.clamp(0.0, 1.0);
                n
            }
            InsertMode::RateRegion { .. } => {
                let mut n = (self.ninsert_per + self.random.uniform()) as i64;
                if self.ninsert_exists && self.ninserted + n > self.ninsert {
                    n = self.ninsert - self.ninserted;
                }
                n
            }
        }
    }

    /// `calc_maxtry`.
    fn calc_maxtry(&self, ninsert_this_local: i64) -> i64 {
        match self.mode {
            InsertMode::Pack(_) => {
                if self.insertion_ratio >= 1.0 {
                    ninsert_this_local * self.maxattempt
                } else {
                    ((ninsert_this_local * self.maxattempt) as f64 / (1.0 - self.insertion_ratio)) as i64
                }
            }
            InsertMode::RateRegion { .. } => ninsert_this_local * self.maxattempt,
        }
    }

    /// `FixInsert::pre_exchange`: on the scheduled step, generate the particles to
    /// insert and return them in insertion order (they take consecutive tags).
    /// Returns an empty list on any other step.
    ///
    /// `atoms` is the current local ensemble (all of it counts toward the region
    /// target and the overlap test, as upstream uses "all particles, not only
    /// those in the fix group").
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if upstream would stop with an error (region
    /// too small for `all_in`, a particle at the origin, an overflowing count).
    pub fn pre_exchange(&mut self, step: u64, atoms: &[Particle]) -> Result<Vec<InsertedParticle>, DemError> {
        if self.next_reneighbor != Some(step) || self.most_recent_ins_step == Some(step) {
            return Ok(Vec::new());
        }
        self.most_recent_ins_step = Some(step);
        let mut ninsert_this = self.calc_ninsert_this(atoms);
        if ninsert_this < -200_000 {
            return Err(DemError::InvalidInput(
                "overflow in particle number calculation: inserting too many particles in one step".into(),
            ));
        }
        let mut exact_number = self.exact_number;
        if self.ninsert_exists && self.ninserted + ninsert_this >= self.ninsert {
            ninsert_this = (self.ninsert - self.ninserted).max(0);
            exact_number = true;
        }
        // distribute_ninsert_this on one process
        let fraction_local = self.region_volume_local / self.region_volume;
        let ninsert_this_local: i64 = if !exact_number {
            (fraction_local * ninsert_this as f64 + self.random.uniform()) as i64
        } else {
            if fraction_local < 0.02 {
                return Err(DemError::InvalidInput("Internal error distributing particles to processes".into()));
            }
            ((fraction_local / fraction_local) * ninsert_this as f64) as i64
        };
        let list = self.distribution.randomize_list(ninsert_this_local.max(0) as usize, exact_number);
        let ninsert_this = list.len() as i64;
        if ninsert_this == 0 {
            if self.insert_every != 0 && (!self.ninsert_exists || self.ninserted < self.ninsert) {
                self.next_reneighbor = self.next_reneighbor.map(|n| n + self.insert_every);
            } else if self.insert_every == 0 {
                self.next_reneighbor = None;
            }
            return Ok(Vec::new());
        }
        let (inserted, ninserted_this_local) = self.x_v_omega(&list, atoms)?;
        let mut out = Vec::with_capacity(inserted.len());
        let mut mass_inserted_this = 0.0;
        for (pti, (pos, vel)) in list.iter().zip(&inserted) {
            mass_inserted_this += pti.mass;
            out.push(InsertedParticle {
                particle: Particle {
                    position: *pos,
                    velocity: *vel,
                    angular_velocity: self.omega,
                    mass: pti.mass,
                    radius: pti.radius,
                    temperature: 300.0,
                },
                atom_type: pti.atom_type,
                density: pti.density,
            });
        }
        self.ninserted += ninserted_this_local as i64;
        self.massinserted += mass_inserted_this;
        if self.insert_every != 0 && (!self.ninsert_exists || self.ninserted < self.ninsert) {
            self.next_reneighbor = self.next_reneighbor.map(|n| n + self.insert_every);
        } else {
            self.next_reneighbor = None;
        }
        Ok(out)
    }

    /// `x_v_omega`: place `list` (in order) in the region; returns the
    /// `(position, velocity)` of the ones that fitted and their count.
    fn x_v_omega(
        &mut self,
        list: &[ParticleToInsert],
        atoms: &[Particle],
    ) -> Result<(Vec<(Vec3, Vec3)>, usize), DemError> {
        let ninsert_this_local = list.len();
        let mut placed: Vec<(Vec3, Vec3)> = Vec::with_capacity(ninsert_this_local);
        if !self.overlap_check {
            for pti in list {
                let pos = if self.all_in { self.generate_random(Some(pti.radius))? } else { self.generate_random(None)? };
                let v = self.generate_random_velocity();
                if pos.x == 0.0 && pos.y == 0.0 && pos.z == 0.0 {
                    return Err(DemError::InvalidInput("FixInsertPack::x_v_omega() illegal position".into()));
                }
                placed.push((pos, v));
            }
            return Ok((placed, ninsert_this_local));
        }
        let mut grid = OverlapGrid::new(2.0 * self.distribution.max_r_bound().max(atoms.iter().fold(0.0, |m, a| a.radius.max(m))));
        for a in atoms {
            grid.insert(a.position, a.radius);
        }
        let maxtry = self.calc_maxtry(ninsert_this_local as i64);
        let mut ntry: i64 = 0;
        while ntry < maxtry && placed.len() < ninsert_this_local {
            let pti = list[placed.len()];
            let rbound = pti.radius;
            let mut nins = 0;
            while nins == 0 && ntry < maxtry {
                let pos = loop {
                    let pos = if self.all_in { self.generate_random(Some(rbound))? } else { self.generate_random(None)? };
                    ntry += 1;
                    if !(self.check_dist_from_subdomain_border && ntry < maxtry && self.dist_subbox_borders(pos) < rbound) {
                        break pos;
                    }
                };
                if ntry == maxtry {
                    break;
                }
                let v = self.generate_random_velocity();
                if !grid.has_overlap(pos, pti.radius) {
                    grid.insert(pos, pti.radius);
                    placed.push((pos, v));
                    nins = 1;
                }
            }
        }
        let n = placed.len();
        Ok((placed, n))
    }
}

/// Uniform grid answering "does a sphere overlap any inserted one"
/// (`RegionNeighborList::hasOverlap`: `rsq <= (r + r_j)^2`).
struct OverlapGrid {
    cell: f64,
    cells: BTreeMap<(i64, i64, i64), Vec<(Vec3, f64)>>,
}

impl OverlapGrid {
    fn new(cell: f64) -> Self {
        Self { cell: if cell > 0.0 { cell } else { 1.0 }, cells: BTreeMap::new() }
    }

    fn key(&self, p: Vec3) -> (i64, i64, i64) {
        ((p.x / self.cell).floor() as i64, (p.y / self.cell).floor() as i64, (p.z / self.cell).floor() as i64)
    }

    fn insert(&mut self, p: Vec3, r: f64) {
        let k = self.key(p);
        self.cells.entry(k).or_default().push((p, r));
    }

    fn has_overlap(&self, p: Vec3, r: f64) -> bool {
        let (cx, cy, cz) = self.key(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(v) = self.cells.get(&(cx + dx, cy + dy, cz + dz)) {
                        for (q, rq) in v {
                            let d0 = p.x - q.x;
                            let d1 = p.y - q.y;
                            let d2 = p.z - q.z;
                            let rsq = d0 * d0 + d1 * d1 + d2 * d2;
                            let radsum = r + rq;
                            if rsq <= radsum * radsum {
                                return true;
                            }
                        }
                    }
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tpl(seed: i32, r: f64) -> ParticleTemplateSphere {
        ParticleTemplateSphere::new(seed, 1, Length::new::<meter>(r), MassDensity::new::<kilogram_per_cubic_meter>(2500.0)).unwrap()
    }

    fn one_template_dist() -> ParticleDistributionDiscrete {
        ParticleDistributionDiscrete::new(15_485_867, true, vec![(tpl(15_485_863, 0.005), 1.0)]).unwrap()
    }

    #[test]
    fn park_miller_first_values_and_seed_rules() {
        // seed 1: the minimal-standard sequence starts 16807, 282475249
        let mut r = RanPark::new(1).unwrap();
        assert_eq!(r.uniform(), (1.0 / 2147483647.0) * 16807.0);
        assert_eq!(r.state(), 16807);
        r.uniform();
        assert_eq!(r.state(), 282_475_249);
        assert!(RanPark::new(0).is_err());
        assert!(RanPark::checked(15_485_863).is_ok());
        assert!(RanPark::checked(15_485_864).is_err());
        assert!(RanPark::checked(9_973).is_err());
        let mut g = RanPark::new(15_485_863).unwrap();
        let (a, b) = (g.gaussian(), g.gaussian());
        assert!(a.is_finite() && b.is_finite() && a != b);
        g.reset(7).unwrap();
        assert_eq!(g.state(), 7);
        assert!(g.reset(0).is_err());
    }

    #[test]
    fn distribution_weights_and_order() {
        // two templates, 70/30 by mass: number weights follow 1/mass
        let d = ParticleDistributionDiscrete::new(
            15_485_867,
            true,
            vec![(tpl(15_485_863, 0.005), 0.7), (tpl(49_979_687, 0.0035), 0.3)],
        )
        .unwrap();
        let w = d.number_weights();
        assert!((w[0] + w[1] - 1.0).abs() < 1e-15 && w[1] > w[0] * 0.3);
        assert_eq!(d.order(), &[0, 1]);
        assert!(d.vol_expect() > 0.0 && d.mass_expect() > 0.0 && d.max_r_bound() == 0.005);
        assert!(ParticleDistributionDiscrete::new(15_485_867, true, vec![]).is_err());
    }

    #[test]
    fn fix_setup_checks_and_scheduling() {
        let reg = InsertRegion::block(Vec3::new(-0.01, -0.01, 0.0), Vec3::new(0.01, 0.01, 0.05)).unwrap();
        let lo = Vec3::new(-0.1, -0.1, -0.1);
        let hi = Vec3::new(0.1, 0.1, 0.1);
        let mode = InsertMode::Pack(PackTarget::Particles(3));
        // the fix, distribution and template seeds must differ
        assert!(InsertFix::new(mode, 15_485_867, one_template_dist(), reg, 10, lo, hi).is_err());
        assert!(InsertFix::new(mode, 32_452_843, one_template_dist(), reg, 10, lo, hi)
            .unwrap()
            .with_ntry_mc(10)
            .is_err());
        let mut fix = InsertFix::new(mode, 32_452_843, one_template_dist(), reg, 10, lo, hi)
            .unwrap()
            .with_ntry_mc(2000)
            .unwrap();
        fix.setup().unwrap();
        assert!(fix.region_volume() > 0.0);
        assert_eq!(fix.next_insertion_step(), Some(1));
        let got = fix.pre_exchange(1, &[]).unwrap();
        assert_eq!(got.len(), 3);
        assert_eq!(fix.ninserted(), 3);
        assert!(fix.massinserted() > 0.0 && fix.max_rad() == 0.005);
        // nothing on a step that is not scheduled
        assert!(fix.pre_exchange(2, &[]).unwrap().is_empty());
        // target already met: the next scheduled step inserts nothing
        let atoms: Vec<Particle> = got.iter().map(|g| g.particle).collect();
        assert!(fix.pre_exchange(11, &atoms).unwrap().is_empty());
        // a region outside the box is refused at set-up
        let far = InsertRegion::block(Vec3::new(1.0, 1.0, 1.0), Vec3::new(2.0, 2.0, 2.0)).unwrap();
        let mut bad = InsertFix::new(mode, 32_452_843, one_template_dist(), far, 10, lo, hi).unwrap();
        assert!(bad.setup().is_err());
    }

    #[test]
    fn region_predicates() {
        let z = Length::new::<meter>(0.0);
        let c = InsertRegion::cylinder(Axis::Z, z, z, Length::new::<meter>(1.0), z, Length::new::<meter>(2.0)).unwrap();
        assert!(c.contains(Vec3::new(0.5, 0.0, 1.0)) && !c.contains(Vec3::new(1.5, 0.0, 1.0)));
        assert!(c.within_cut_of_surface(Vec3::new(0.95, 0.0, 1.0), 0.1));
        assert!(!c.within_cut_of_surface(Vec3::new(0.0, 0.0, 1.0), 0.1));
        assert!(InsertRegion::cylinder(Axis::X, z, z, z, z, Length::new::<meter>(1.0)).is_err());
        assert!(InsertRegion::block(Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 1.0)).is_err());
        let (lo, hi) = c.extent();
        assert_eq!((lo.z, hi.z, lo.x, hi.y), (0.0, 2.0, -1.0, 1.0));
    }
}
