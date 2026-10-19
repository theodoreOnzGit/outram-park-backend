//! **A double-heterogeneity region: particles packed in a sphere** (gh #926,
//! tutorial 5 on #943).
//!
//! The maintainer's decisions (#943, 2026-10-17), which this module follows:
//!
//! - **A `DhRegion` is built as its own object**, from the outset: a sphere
//!   of matrix with particles packed in it. The code never detects double
//!   heterogeneity by itself.
//! - **Packing:** random (the workspace's existing RSA,
//!   [`crate::pebble_beds::sphere_packing::pack_spheres`] through
//!   [`crate::dh_universe::pack_in_ball`]) or a regular lattice, **both with
//!   whole-particle rejection** (no cut particle, ever). The census reports
//!   the achieved packing fraction against the requested one.
//! - **Its tracking is required**, set by hand: `.delta_tracking()`; a
//!   region without it is refused by `assemble()`. (The chord-length family
//!   behind `.advanced().tracking(..)`, and the conversions `ring_rpt()` and
//!   `naive_homogenisation()`, are decided on #943 but not built: the
//!   chord-length methods live in `DhUniverse`'s own transport, not in the
//!   CSG run this model lowers to.)
//!
//! ```
//! use outram_mc_libs::model::prelude::*;
//! use uom::si::{f64::{Length, Ratio}, length::centimeter, ratio::percent};
//! let cm = Length::new::<centimeter>;
//! let graphite = Material::from_atom_densities([(C12, atoms_per_barn_cm(0.08))]);
//! let fuel = Material::from_atom_densities([(U235, atoms_per_barn_cm(0.04))]);
//! let particle = Sphere::radius(cm(0.05)).filled_with(fuel);
//! let fuel_zone = DhRegion::new(Sphere::radius(cm(1.0)), graphite.clone())
//!     .with_particles(particle, Packing::Random { fraction: Ratio::new::<percent>(5.0), seed: 7 })
//!     .delta_tracking();
//! let pebble = Sphere::radius(cm(1.2)).filled_with(graphite).containing(fuel_zone);
//! assert_eq!(pebble.contained().len(), 1);
//! ```
//!
//! ## How it lowers
//!
//! Both packings lower to a CSG `RectLattice` covering the zone, in the
//! zone's cell, with the cell's tracking set to delta tracking against a
//! majorant over the materials the zone holds
//! ([`super::AssembledModel::majorants`]):
//!
//! - **Lattice**: one tile, a cube of matrix around the particle, repeated
//!   by [`super::Lattice::covering`] with whole-particle rejection. The same
//!   lowering as `Sphere::containing_lattice` (nee_soon's HTR-10 TRISO
//!   lattice, `htr10_rmc::core_shell::TrisoBuild`).
//! - **Random**: the particle centres from `pack_in_ball` (the packing
//!   `DhUniverse::pebble` uses, with the same arguments, so the same seed
//!   gives the same particles), placed in a tracking lattice by
//!   [`crate::pebble_beds::triso_lattice::create_triso_lattice`], a port of
//!   OpenMC's `openmc.model.create_triso_lattice`.

use std::sync::Arc;

use uom::si::f64::{Length, Ratio};

use super::material::Material;
use super::model::{AssembledModel, Temperature};
use super::shape::{Component, Shape, Sphere};
use crate::material::material::Material as LowMaterial;
use crate::material::nuclide::Nuclide;
use crate::physics::delta_tracking::Majorant;

/// **How the particles of a [`DhRegion`] are arranged.** Every arrangement
/// keeps whole particles only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Packing {
    /// **Random sequential addition** to a packing fraction, OpenMC's
    /// `pack_spheres` as ported in
    /// [`crate::pebble_beds::sphere_packing::pack_spheres`], through
    /// [`crate::dh_universe::pack_in_ball`]: generated in the cube around
    /// the zone, whole particles kept, the request rescaled until the
    /// fraction **inside the zone** is within 0.2 % of `fraction`. RSA jams
    /// near 38 % ([`crate::pebble_beds::sphere_packing::MAX_PF_RSA`]); a
    /// fraction above that is refused.
    Random {
        /// The packing fraction asked for (particle volume over zone
        /// volume).
        fraction: Ratio,
        /// The packing seed: the same seed gives the same particles.
        seed: u64,
    },
    /// **A simple-cubic lattice** of `pitch`, one particle centre at
    /// `offset` pitches from the zone's centre, whole-particle rejection.
    /// See [`super::Lattice::covering`].
    Lattice {
        /// The lattice pitch (centre to centre).
        pitch: Length,
        /// Offset of the grid, in pitches.
        offset: [f64; 3],
    },
    /// **A simple-cubic lattice placing exactly `particles` whole
    /// particles**, its pitch found by
    /// [`super::Cube::for_whole_particles_in_sphere`] (refused when no
    /// pitch gives that count at this offset).
    LatticeOfCount {
        /// The number of whole particles wanted.
        particles: usize,
        /// Offset of the grid, in pitches.
        offset: [f64; 3],
    },
}

/// The tracking of a [`DhRegion`]. Only delta tracking lowers to the CSG run
/// today (see the module docs for the rest of #943's enum).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DhTracking {
    /// Delta (Woodcock) tracking against a majorant over the region's
    /// materials: unbiased under any valid majorant.
    Delta,
}

/// What a [`Shape`] made from a [`DhRegion`] carries.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DhSpec {
    pub(crate) particles: Option<(Arc<Shape>, Packing)>,
    pub(crate) tracking: Option<DhTracking>,
}

impl DhSpec {
    /// The particle, if one was given.
    pub(crate) fn particle(&self) -> Option<&Shape> {
        self.particles.as_ref().map(|(p, _)| p.as_ref())
    }
}

/// **A double-heterogeneity region**: a sphere of matrix with particles
/// packed in it, and the tracking it is transported with. Put it in a shape
/// with `.containing(region)`. See the module docs.
#[derive(Debug, Clone, PartialEq)]
pub struct DhRegion {
    zone: Sphere,
    matrix: Material,
    component: Option<Component>,
    spec: DhSpec,
    temperature: Option<Temperature>,
}

impl DhRegion {
    /// A `zone` of `matrix`, with no particles yet.
    pub fn new(zone: Sphere, matrix: Material) -> Self {
        Self {
            zone,
            matrix,
            component: None,
            temperature: None,
            spec: DhSpec {
                particles: None,
                tracking: None,
            },
        }
    }

    /// Put the matrix in `component`. (The particle's layers keep their
    /// own components.)
    pub fn in_component(mut self, component: Component) -> Self {
        self.component = Some(component);
        self
    }

    /// **The particle and how it is packed.** `particle` is any shape whose
    /// outermost solid is a sphere centred at its own origin (a TRISO
    /// particle built kernel outwards, tutorial 4); the packing places the
    /// copies.
    pub fn with_particles(mut self, particle: Shape, packing: Packing) -> Self {
        self.spec.particles = Some((Arc::new(particle), packing));
        self
    }

    /// **Transport this region by delta (Woodcock) tracking**, against a
    /// majorant over every material in it. Required (#943): a region
    /// without a tracking call is refused by `assemble()`. Delta tracking
    /// is unbiased under any valid majorant; it changes cost, not the
    /// answer.
    pub fn delta_tracking(mut self) -> Self {
        self.spec.tracking = Some(DhTracking::Delta);
        self
    }

    /// **Give the region its own temperature** (gh #945, stage 1): the
    /// matrix and every particle that sets none of its own
    /// ([`Shape::at_temperature`] on the particle). The region's majorant
    /// ([`AssembledModel::majorants`]) is built over its materials at the
    /// temperatures they are processed at, so it bounds every temperature in
    /// the region.
    pub fn at_temperature(mut self, temperature: Temperature) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// The zone.
    pub fn zone(&self) -> Sphere {
        self.zone
    }

    /// The packing, if particles were given.
    pub fn packing(&self) -> Option<Packing> {
        self.spec.particles.as_ref().map(|(_, p)| *p)
    }
}

impl From<DhRegion> for Shape {
    fn from(r: DhRegion) -> Shape {
        let mut s = r.zone.filled_with(r.matrix);
        s.component = r.component;
        s.dh = Some(r.spec);
        s.temperature = r.temperature;
        s
    }
}

impl AssembledModel {
    /// **Per delta-tracked region** (a [`DhRegion`]): the indices into
    /// [`AssembledModel::materials`] it can present. A cell whose tracking is
    /// `TrackingMethod::Delta { majorant: i }` belongs to region `i`.
    pub fn delta_regions(&self) -> &[Vec<usize>] {
        &self.delta_regions
    }

    /// **The majorant of each delta-tracked region**, on `nuclides` (in
    /// [`AssembledModel::nuclides`] order): `Majorant::bounding` over the
    /// region's materials with the arguments `DhUniverse::keff` uses (4096
    /// log bins from 1e-4 eV to 20 MeV, 32 sub-samples, a 30 % margin, and
    /// every nuclide breakpoint since gh:#585). Over the region only, so a
    /// strong absorber elsewhere costs nothing here (`TrackingMethod`'s
    /// docs). Empty when the model has no delta-tracked region.
    ///
    /// **Temperature (gh #945, stage 1).** A material placed at two
    /// temperatures is two entries of [`AssembledModel::materials`], each
    /// with its own temperature and its own nuclides processed at it, so the
    /// majorant is the largest `Σ_t` over every temperature present in the
    /// region, at every energy. It is rebuilt on every run from the
    /// materials and the nuclides passed; no majorant outlives the
    /// temperatures it was built at, and the run refuses nuclides processed
    /// at another temperature than their key's (`RunError::NuclideTemperature`).
    /// Pinned by `tests/region_temperature.rs` (an audit at 1200 K, and the
    /// 293.6 K control that is under).
    ///
    /// **Cover (gh #945, stage 3).** Only when `advanced()` asked for it
    /// (`AdvancedModel::with_majorant_cover`) is the table replaced by its
    /// Chebyshev cover (`Majorant::chebyshev_cover`): `>=` the table
    /// everywhere, so the same bound, with a few coefficients per band in
    /// place of a few hundred thousand nodes, at the price of more virtual
    /// collisions. The default is the table.
    pub fn majorants(&self, nuclides: &[Nuclide]) -> Vec<Majorant> {
        self.delta_regions
            .iter()
            .map(|mats| {
                let subset: Vec<LowMaterial> =
                    mats.iter().map(|&m| self.materials[m].clone()).collect();
                let table = Majorant::bounding(&subset, nuclides, 1.0e-4, 2.0e7, 4096, 32, 0.3);
                match self.majorant_cover {
                    Some(s) => table.chebyshev_cover(s),
                    None => table,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::isotopes::{C12, U235};
    use crate::model::material::atoms_per_barn_cm;
    use uom::si::length::centimeter;
    use uom::si::ratio::ratio;

    /// The builder keeps what it is given, and becomes a shape carrying it.
    #[test]
    fn a_dh_region_becomes_a_shape_with_its_particles_and_tracking() {
        let cm = Length::new::<centimeter>;
        let g = Material::from_atom_densities([(C12, atoms_per_barn_cm(0.08))]);
        let f = Material::from_atom_densities([(U235, atoms_per_barn_cm(0.04))]);
        let p = Sphere::radius(cm(0.05)).filled_with(f);
        let packing = Packing::Random {
            fraction: Ratio::new::<ratio>(0.05),
            seed: 3,
        };
        let r = DhRegion::new(Sphere::radius(cm(1.0)), g.clone());
        assert!(r.packing().is_none());
        let r = r.with_particles(p.clone(), packing);
        assert_eq!(r.packing(), Some(packing));
        assert_eq!(r.zone().radius_cm(), 1.0);
        let s: Shape = r.clone().into();
        let spec = s.dh.as_ref().unwrap();
        assert_eq!(spec.particle(), Some(&p));
        assert!(spec.tracking.is_none(), "no tracking until asked");
        let s: Shape = r.delta_tracking().into();
        assert_eq!(s.dh.as_ref().unwrap().tracking, Some(DhTracking::Delta));
        let mut n = 0;
        s.visit(&mut |_| n += 1);
        assert_eq!(n, 2, "the zone and its particle");
    }
}
