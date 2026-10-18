// SPDX-License-Identifier: GPL-3.0

//! **Tutorial 7's model: one Şeker layer of the HTR-10 pebble bed, an
//! infinite stack of it, through the beginner API** (`outram_mc_libs::model`,
//! epic gh #925, tutorial list on #943). Shared by
//! `examples/tutorial_seker_layer.rs` and `tests/model_seker_layer.rs`, so the
//! lesson and the test that pins it run one set of statements.
//!
//! # The case
//!
//! Şeker & Çolak (2003), *HTR-10 full core first criticality analysis with
//! MCNP*, Nucl. Eng. Des. 222, 263-270, doi:10.1016/S0029-5493(03)00031-1,
//! pp. 266-267, as `nee_soon::htr10_rmc::bed::SekerCell::from_paper`
//! reconstructs it from the text and Fig. 3 (gh:#472; nothing fitted). For
//! 6 cm balls, with `d` the diameter:
//!
//! - a **hexagonal prism** tile, faces perpendicular to y, apothem
//!   `d cos 30 + d/2 = 8.196 cm` (pitch 16.392 cm), height `2 d sqrt(2/3) =
//!   9.798 cm` (two close-packed layer spacings);
//! - **13 balls per tile**: a basal "flower" (7 balls, the centre one and six
//!   pointing at the corners, tangent to the side faces beside them) on the
//!   bottom face, centred on it (so each is a hemisphere in this layer and
//!   the other half of the same ball in the layer below); and a central
//!   plane of 6: three `Inner` balls at `d/sqrt(3)` (90, 210, 330 degrees)
//!   and three `Outer` at `2d/sqrt(3)` (270, 30, 150 degrees), each crossing
//!   a side face into the neighbour (1.73 cm past it);
//! - the lattice shows the 10 pieces that enter the tile from its neighbours
//!   and from the layer above, so a tile draws pieces of 23 balls.
//!
//! The site table is `SekerCell::site_centre` (`bed.rs`), the expressions
//! ported with the same arithmetic, and `own_sites()` lists the 13 balls the
//! tile owns, in `SekerSite::ALL` order.
//!
//! # Fuel and moderator balls
//!
//! Şeker p.267: *"Fuel and moderator balls are selected in each layer such
//! that 0.57:0.43 ratio is established"*. The core applies a low-discrepancy
//! rule over the balls of the bed (`SekerBed::assign`: ordered by layer, then
//! distance from the axis squared, then angle; ball `n` is fuel when
//! `floor((n+1) f) > floor(n f)`, `f = 0.57`). [`fuel_flags`] applies that
//! same rule to the 13 balls of one tile: **7 fuel and 6 moderator** (0.538),
//! because a tile of 13 cannot hold 0.57 exactly (8 would be 0.615). The
//! same ball has the same identity in every piece, in every tile and in every
//! layer: that is what a lattice of one tile means.
//!
//! # What is stated, not derived
//!
//! - The fuel ball is tutorial 5's (the regular arm: 8335 TRISO particles,
//!   the HTR-10 core's lattice offset), its materials copied from there.
//!   The moderator ("dummy") ball is a graphite sphere of the shell's
//!   graphite, as `htr10_rmc` builds it.
//! - Helium is an ideal gas at 101.33 kPa and the model temperature, with the
//!   He-3 fraction of natural helium (1.343e-6, IUPAC), as
//!   `nee_soon::htr10_rmc::materials` has it. The pressure is **an
//!   assumption** there too (neither paper states it; gh:#426).
//! - 300.15 K everywhere (Li, Yu & Wei 2014's and Şeker's 27 C), the
//!   temperature of the crate's HTR-10 records.

#![allow(dead_code)]

use std::f64::consts::PI;

use outram_mc_libs::geometry::triso_particle::TrisoRadii;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::pebble_beds::htr10;
use uom::si::f64::{Length, MassDensity, Ratio, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::mass_density::gram_per_cubic_centimeter;
use uom::si::ratio::{part_per_million, ratio};
use uom::si::thermodynamic_temperature::kelvin;

/// Ball diameter \[cm\] (HTR-10's 6 cm pebble).
pub const BALL_DIAMETER_CM: f64 = 6.0;
/// Fuel-zone radius \[cm\] and whole TRISO particles per fuel ball (Li, Yu &
/// Wei 2014 Table 2; Şeker & Çolak p.266).
pub const FUEL_ZONE_R_CM: f64 = 2.5;
pub const PARTICLES_PER_PEBBLE: usize = 8335;
/// The HTR-10 core's TRISO lattice offset, in pitches (gh:#430).
pub const OFFSET: [f64; 3] = [0.13, 0.37, 0.71];
/// Şeker & Çolak p.267: the fuel-ball fraction of a layer.
pub const FUEL_BALL_FRACTION: f64 = 0.57;
/// Model temperature \[K\].
pub const T_K: f64 = 300.15;
/// Helium pressure \[Pa\]: an assumption (gh:#426).
pub const HELIUM_PRESSURE_PA: f64 = 101_330.0;
/// He-3 atom fraction of natural helium (IUPAC/CIAAW; Meija et al. 2016).
pub const HE3_FRACTION: f64 = 1.343e-6;
/// Şeker's quoted packing: 13 balls in the interior cell (nee_soon
/// `SekerCell::packing_fraction`, 0.6448).
pub const SEKER_INTERIOR_PACKING: f64 = 0.6448;

/// `SekerCell::from_paper`: hexagon apothem \[cm\].
pub fn apothem_cm() -> f64 {
    BALL_DIAMETER_CM * (PI / 6.0).cos() + 0.5 * BALL_DIAMETER_CM
}

/// Flat-to-flat pitch \[cm\] (`SekerCell::pitch`).
pub fn pitch_cm() -> f64 {
    2.0 * apothem_cm()
}

/// Layer height \[cm\]: two close-packed spacings
/// (`close_packed_layer_spacing(d) = d sqrt(2/3)`).
pub fn height_cm() -> f64 {
    2.0 * (BALL_DIAMETER_CM * (2.0_f64 / 3.0).sqrt())
}

/// A ball site the tile owns (`SekerSite`, restricted to the 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Site {
    /// Basal flower ball `k` (0 is the centre; 1..=6 point at the corners,
    /// from 0 degrees, 60 degrees apart), centred on the bottom face.
    Flower(u8),
    /// Central-plane ball `k` (0..3) at `d/sqrt(3)`, 90 + 120 k degrees.
    Inner(u8),
    /// Central-plane ball `k` (0..3) at `2d/sqrt(3)`, 270 + 120 k degrees.
    Outer(u8),
}

/// The 13 balls the tile owns, in `SekerSite::ALL` order (bottom flower,
/// inner, outer).
pub fn own_sites() -> Vec<Site> {
    let mut v: Vec<Site> = (0..7).map(Site::Flower).collect();
    v.extend((0..3).map(Site::Inner));
    v.extend((0..3).map(Site::Outer));
    v
}

/// Tile-local centre \[cm\] of `site` (`SekerCell::site_centre`).
pub fn site_centre(site: Site) -> [f64; 3] {
    let d = BALL_DIAMETER_CM;
    let h = height_cm();
    let polar = |rho: f64, deg: f64, z: f64| {
        let t = deg.to_radians();
        [rho * t.cos(), rho * t.sin(), z]
    };
    let s3 = 3.0_f64.sqrt();
    match site {
        Site::Flower(k) => {
            if k == 0 {
                [0.0, 0.0, -0.5 * h]
            } else {
                polar(d, 60.0 * f64::from(k - 1), -0.5 * h)
            }
        }
        Site::Inner(k) => polar(d / s3, 90.0 + 120.0 * f64::from(k), 0.0),
        Site::Outer(k) => polar(2.0 * d / s3, 270.0 + 120.0 * f64::from(k), 0.0),
    }
}

/// **Which of the 13 balls are fuel**: the core's low-discrepancy rule
/// (`SekerBed::assign`) at fraction `f`, on the tile's 13 balls ordered by
/// layer (the flower is the basal plane, the rest the central plane), then
/// distance from the axis squared, then angle (`atan2`), then site.
pub fn fuel_flags(f: f64) -> Vec<bool> {
    let sites = own_sites();
    // Distances squared are compared on a 1e-6 cm^2 grid: the six flower
    // balls (and the three `Inner` and three `Outer` balls) are exactly
    // equidistant from the axis, and without the grid their order would be
    // decided by the rounding of `cos` and `sin`, not by the angle the rule
    // names.
    let mut order: Vec<(i32, i64, f64, usize)> = sites
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let [x, y, _] = site_centre(*s);
            let layer = if matches!(s, Site::Flower(_)) { 0 } else { 1 };
            (
                layer,
                ((x * x + y * y) * 1.0e6).round() as i64,
                y.atan2(x),
                i,
            )
        })
        .collect();
    order.sort_by(|p, q| {
        p.0.cmp(&q.0)
            .then(p.1.cmp(&q.1))
            .then(p.2.total_cmp(&q.2))
            .then(p.3.cmp(&q.3))
    });
    let mut fuel = vec![false; sites.len()];
    for (n, &(_, _, _, i)) in order.iter().enumerate() {
        fuel[i] = ((n + 1) as f64 * f).floor() > (n as f64 * f).floor();
    }
    fuel
}

/// **Exactly `count` fuel balls of the 13**, by the same rule: the fraction
/// is set midway between `count/13` and `(count+1)/13`, so `floor(13 f)` is
/// `count` with room for rounding. For the ablation that asks how much the
/// tile's 7:6 split (against the paper's 0.57) is worth.
pub fn fuel_flags_for_count(count: usize) -> Vec<bool> {
    fuel_flags((count as f64 + 0.5) / 13.0)
}

/// The materials of the layer. [`materials`] makes the real set; a test makes
/// its own with the nuclides it has.
#[derive(Clone)]
pub struct Materials {
    /// UO2 kernel.
    pub uo2: Material,
    /// Porous buffer carbon.
    pub buffer: Material,
    /// Pyrolytic carbon (inner and outer layers).
    pub pyc: Material,
    /// Silicon carbide.
    pub sic: Material,
    /// Matrix graphite (the fuel zone).
    pub matrix: Material,
    /// Shell graphite (fuel-free), also the moderator ball.
    pub shell: Material,
    /// Helium between the balls.
    pub helium: Material,
}

/// Helium: an ideal gas at `p` \[Pa\] and `t` \[K\] with natural He-3, as
/// `nee_soon::htr10_rmc::materials::ideal_gas_atom_density` (`N = p / k_B T`,
/// converted to atoms per barn-cm).
pub fn helium(t_k: f64) -> Material {
    const BOLTZMANN: f64 = 1.380_649e-23;
    let n = HELIUM_PRESSURE_PA / (BOLTZMANN * t_k) * 1.0e-30;
    Material::from_atom_densities([
        (He3, atoms_per_barn_cm(n * HE3_FRACTION)),
        (He4, atoms_per_barn_cm(n * (1.0 - HE3_FRACTION))),
    ])
    .named("helium")
}

/// The real materials: tutorial 5's, and helium at `t_k`.
pub fn materials(t_k: f64) -> Materials {
    let g_cc = MassDensity::new::<gram_per_cubic_centimeter>;
    let ppm = Ratio::new::<part_per_million>;
    let graphite_law = Moderator::Graphite(GraphiteKind::ReactorPorosity30);
    let boron_in_carbon = Contaminant::ppm(
        ppm(htr10::B_PPM_GRAPHITE),
        Basis::Atom,
        PpmOf::PerHostAtom,
        Placement::AddOn,
    );
    let carbon = |rho: f64, name: &str| {
        Material::from_fractions(
            Basis::Atom,
            [(Element::Carbon.natural(), Ratio::new::<ratio>(1.0))],
        )
        .density(g_cc(rho))
        .contaminant(Element::Boron.natural(), boron_in_carbon)
        .bound_in(graphite_law)
        .named(name)
    };
    let uo2 = Material::fuel(
        HeavyMetal::Uranium(Enrichment::by_weight(
            FissileUranium::U235,
            Ratio::new::<ratio>(htr10::ENRICHMENT_WT),
        )),
        FuelForm::Oxide,
    )
    .density(g_cc(htr10::RHO_UO2))
    .contaminant(
        Element::Boron.natural(),
        Contaminant::ppm(
            ppm(htr10::B_PPM_URANIUM),
            Basis::Atom,
            PpmOf::HeavyMetal,
            Placement::AddOn,
        ),
    )
    .named("UO2 kernel");
    let sic = Material::from_fractions(
        Basis::Atom,
        [
            (Element::Silicon.natural(), Ratio::new::<ratio>(0.5)),
            (Element::Carbon.natural(), Ratio::new::<ratio>(0.5)),
        ],
    )
    .density(g_cc(htr10::RHO_SIC))
    .bound_in(Moderator::SiliconCarbide)
    .named("silicon carbide");
    Materials {
        uo2,
        buffer: carbon(htr10::RHO_BUFFER, "buffer carbon"),
        pyc: carbon(htr10::RHO_PYC, "pyrolytic carbon"),
        sic,
        matrix: carbon(htr10::RHO_GRAPHITE, "matrix graphite"),
        shell: carbon(htr10::RHO_GRAPHITE, "shell graphite"),
        helium: helium(t_k),
    }
}

/// How the fuel ball is built.
#[derive(Clone)]
pub enum FuelBall {
    /// Tutorial 5's pebble: a shell around a fuel zone of 8335 TRISO
    /// particles in a regular lattice, delta-tracked.
    Triso,
    /// A sphere of one smeared material (the test's small stand-in for the
    /// bit-for-bit comparison, which tests the lattice, not the pebble).
    Smeared(Material),
}

/// The components of the layer, for the census.
#[derive(Debug, Clone, Copy)]
pub struct Components {
    pub kernel: Component,
    pub coatings: Component,
    pub matrix: Component,
    /// The shell of a fuel ball (counting these counts the fuel balls).
    pub shell: Component,
    /// A moderator ball (counting these counts the moderator balls).
    pub moderator: Component,
    pub helium: Component,
}

/// The built layer.
pub struct SekerLayer {
    /// The model: assemble it.
    pub model: Model,
    /// Its components.
    pub components: Components,
    /// The 13 balls the tile owns.
    pub sites: Vec<Site>,
    /// Which of them are fuel.
    pub fuel: Vec<bool>,
}

fn cm(x: f64) -> Length {
    Length::new::<centimeter>(x)
}

/// Tutorial 4's TRISO particle, kernel outwards, one `.containing` each.
pub fn triso_particle(m: &Materials, c: &Components) -> Shape {
    let r = TrisoRadii::HTR10;
    let kernel = Sphere::radius(cm(r.kernel))
        .filled_with(m.uo2.clone())
        .in_component(c.kernel);
    let buffer = Sphere::radius(cm(r.buffer))
        .filled_with(m.buffer.clone())
        .in_component(c.coatings)
        .containing(kernel);
    let ipyc = Sphere::radius(cm(r.ipyc))
        .filled_with(m.pyc.clone())
        .in_component(c.coatings)
        .containing(buffer);
    let sic = Sphere::radius(cm(r.sic))
        .filled_with(m.sic.clone())
        .in_component(c.coatings)
        .containing(ipyc);
    Sphere::radius(cm(r.opyc))
        .filled_with(m.pyc.clone())
        .in_component(c.coatings)
        .containing(sic)
}

/// **A fuel ball centred at `centre`**, tutorial 5's pebble moved to its
/// place in the tile.
pub fn fuel_ball(m: &Materials, c: &Components, ball: &FuelBall, centre: [f64; 3]) -> Shape {
    let at = |r: f64| Sphere::radius(cm(r)).centred_at(cm(centre[0]), cm(centre[1]), cm(centre[2]));
    match ball {
        FuelBall::Triso => {
            let fuel_zone = DhRegion::new(at(FUEL_ZONE_R_CM), m.matrix.clone())
                .in_component(c.matrix)
                .with_particles(
                    triso_particle(m, c),
                    Packing::LatticeOfCount {
                        particles: PARTICLES_PER_PEBBLE,
                        offset: OFFSET,
                    },
                )
                .delta_tracking();
            at(0.5 * BALL_DIAMETER_CM)
                .filled_with(m.shell.clone())
                .in_component(c.shell)
                .containing(fuel_zone)
        }
        FuelBall::Smeared(mix) => at(0.5 * BALL_DIAMETER_CM)
            .filled_with(mix.clone())
            .in_component(c.shell),
    }
}

/// **The tile**: a hexagonal prism of helium holding the 13 balls it owns.
pub fn tile(m: &Materials, c: &Components, ball: &FuelBall, fuel: &[bool]) -> Shape {
    let mut tile = HexPrism::flat_to_flat(cm(pitch_cm()), cm(height_cm()))
        .filled_with(m.helium.clone())
        .in_component(c.helium);
    for (site, &is_fuel) in own_sites().iter().zip(fuel) {
        let centre = site_centre(*site);
        let b = if is_fuel {
            fuel_ball(m, c, ball, centre)
        } else {
            Sphere::radius(cm(0.5 * BALL_DIAMETER_CM))
                .centred_at(cm(centre[0]), cm(centre[1]), cm(centre[2]))
                .filled_with(m.shell.clone())
                .in_component(c.moderator)
        };
        tile = tile.containing(b);
    }
    tile
}

/// **The infinite stack of one layer**: the tile repeated by a hexagonal
/// lattice, in a prism of the tile's size, mirrored on every face.
pub fn layer(m: &Materials, c: &Components, ball: &FuelBall, fuel: &[bool]) -> Shape {
    HexPrism::flat_to_flat(cm(pitch_cm()), cm(height_cm()))
        .filled_with(m.helium.clone())
        .in_component(c.helium)
        .containing_lattice(Lattice::hex(tile(m, c, ball, fuel)))
        .boundary(Boundary::Reflective)
}

/// Build the whole model: components, the layer, the temperature.
pub fn build(
    data: NuclearData,
    m: &Materials,
    ball: &FuelBall,
    fuel: &[bool],
    t_k: f64,
) -> SekerLayer {
    let mut model = Model::new(data);
    let components = Components {
        kernel: model.component("Kernel"),
        coatings: model.component("Coatings"),
        matrix: model.component("Matrix"),
        shell: model.component("Fuel ball shell"),
        moderator: model.component("Moderator ball"),
        helium: model.component("Helium"),
    };
    model.add(layer(m, &components, ball, fuel));
    model.set_temperature(Temperature::Uniform(
        ThermodynamicTemperature::new::<kelvin>(t_k),
    ));
    SekerLayer {
        model,
        components,
        sites: own_sites(),
        fuel: fuel.to_vec(),
    }
}
