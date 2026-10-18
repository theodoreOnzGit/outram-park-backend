// SPDX-License-Identifier: GPL-3.0

//! **Tutorial 8's model: the full Şeker bed of HTR-10, stacked, inside the
//! core, through the beginner API** (`outram_mc_libs::model`, epic gh #925,
//! tutorial list on #943). Shared by `examples/tutorial_seker_bed.rs` and
//! `tests/model_seker_bed.rs`, so the lesson and the test that pins it run
//! one set of statements.
//!
//! # The case
//!
//! Tutorial 7 built one Şeker layer (a tile of 13 balls, repeated without
//! end). Here that tile is **stacked** in the plane and in height inside the
//! core cavity, and the balls that would cross the wall, the cone or the tube
//! are **rejected**:
//!
//! - the **cavity** is a [`Hopper`]: a cylinder of radius 90 cm, a cone 36.946
//!   cm tall narrowing to the 25 cm discharge tube, and the tube down to the
//!   bottom of the model (IAEA-TECDOC-1382 and Terry et al. 2005 Fig. 2, via
//!   `nee_soon::htr10_rmc::core_model`: `HTR10_CORE_RADIUS_CM`,
//!   `HTR10_CONUS_HEIGHT_CM`, `HTR10_DISCHARGE_TUBE_RADIUS_CM`,
//!   `HTR10_BOTTOM_REFLECTOR_CM`). Its floor, the top of the cone, is z = 0;
//! - the **bed** is loaded to Şeker & Çolak's height `9.798 N + 6` cm (`N`
//!   layers; the lowest ball's bottom to the highest ball's top), every ball
//!   whole, the cone and tube filled with graphite balls on the same lattice
//!   (Şeker p.267);
//! - the **fuel : moderator** split is 0.57 : 0.43 of the balls above the
//!   floor by the core's low-discrepancy rule, every ball in the cone and the
//!   tube moderator;
//! - around the cavity: the **reflector** of solid graphite (TECDOC Table 4-3
//!   zone 22) out to 167.793 cm, **boronated carbon bricks** (zone 17) to
//!   190 cm, an empty cavity (helium) above the bed up to z = 221.818 cm, 130
//!   cm of graphite above that, vacuum outside. The model is 610 cm tall
//!   (Terry et al. 2005 Fig. 2).
//!
//! # What this reflector leaves out (stated, not hidden)
//!
//! `nee_soon::htr10_rmc` builds the TECDOC-1382 reflector with its zone map
//! (82 zones with their own densities), the 20 coolant channels, the control-
//! rod borings and the withdrawn rods. **This tutorial's reflector is solid
//! graphite and boronated brick, with none of those.** The tutorial's bed is
//! nee_soon's; the shell around it is the simplified one, and tutorial 13
//! (the HTR-10 helpers) is where the full shell goes. Every one of those
//! omissions makes the reflector better than the real one, so k from this
//! model is expected **above** nee_soon's record (open, not measured: see
//! the page).

#![allow(dead_code)]

use outram_mc_libs::model::prelude::*;
use uom::si::f64::{Length, Ratio, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

#[path = "seker_layer_model.rs"]
pub mod layer;
#[allow(unused_imports)]
pub use layer::{
    fuel_ball, height_cm, materials, own_sites, pitch_cm, site_centre, FuelBall, Materials, Site,
    BALL_DIAMETER_CM, FUEL_BALL_FRACTION, T_K,
};

/// HTR-10 active-core radius \[cm\] (`HTR10_CORE_RADIUS_CM`, 180 cm
/// diameter, IAEA-TECDOC-1382).
pub const CORE_RADIUS_CM: f64 = 90.0;
/// The cone's height \[cm\] (`HTR10_CONUS_HEIGHT_CM`, Terry et al. 2005 Fig.
/// 2: z = 351.818 to 388.764).
pub const CONE_HEIGHT_CM: f64 = 36.946;
/// The discharge tube's radius \[cm\] (`HTR10_DISCHARGE_TUBE_RADIUS_CM`).
pub const TUBE_RADIUS_CM: f64 = 25.0;
/// The tube's depth \[cm\], to the bottom of the model
/// (`HTR10_BOTTOM_REFLECTOR_CM = 610 - 388.764`).
pub const TUBE_DEPTH_CM: f64 = 221.236;
/// The core cavity's height \[cm\], from the floor of the bed to its top
/// (`HTR10_CORE_CAVITY_CM`, Terry 2005 Fig. 2: z = 130.0 to 351.818).
pub const CAVITY_HEIGHT_CM: f64 = 221.818;
/// Graphite above the cavity \[cm\] (`HTR10_AXIAL_REFLECTOR_CM`).
pub const TOP_REFLECTOR_CM: f64 = 130.0;
/// Outer radius of the graphite reflector \[cm\] (`HTR10_GRAPHITE_OUTER_CM`,
/// Terry 2005 Fig. 2).
pub const GRAPHITE_OUTER_CM: f64 = 167.793;
/// Outer radius of the boronated bricks \[cm\] (`HTR10_REFLECTOR_OUTER_CM`).
pub const REFLECTOR_OUTER_CM: f64 = 190.0;
/// The whole model's height \[cm\] (`HTR10_MODEL_HEIGHT_CM`).
pub const MODEL_HEIGHT_CM: f64 = 610.0;

/// Reflector graphite, TECDOC-1382 Table 4-3 zone 22: carbon and natural
/// boron in atoms per barn-cm (`nee_soon::htr10_rmc::reflector::ROWS`).
pub const ZONE_22_CARBON: f64 = 0.882418e-1;
/// See [`ZONE_22_CARBON`].
pub const ZONE_22_BORON: f64 = 0.473769e-6;
/// Boronated carbon brick, Table 4-3 zone 17.
pub const ZONE_17_CARBON: f64 = 0.765984e-1;
/// See [`ZONE_17_CARBON`].
pub const ZONE_17_BORON: f64 = 0.346349e-2;

/// Şeker & Çolak (2003) Table 3: balls in the bed at `N` layers, `1346 N +
/// 733` (`nee_soon::htr10_rmc::SEKER_BALLS_PER_LAYER`,
/// `SEKER_BALLS_EXTRA_PLANE`); 733 per basal plane and 613 per central plane.
pub fn seker_reference_balls(layers: usize) -> usize {
    1346 * layers + 733
}

/// Şeker's loading height \[cm\] for `N` layers: `9.798 N + 6`, from the
/// lowest ball's bottom to the highest ball's top (the exact tile height, not
/// the rounded 9.798).
pub fn loading_height_cm(layers: usize) -> f64 {
    height_cm() * layers as f64 + BALL_DIAMETER_CM
}

fn cm(x: f64) -> Length {
    Length::new::<centimeter>(x)
}

/// The reflector's two materials, the bound-graphite law of the pebbles.
#[derive(Clone)]
pub struct ReflectorMaterials {
    /// Solid reflector graphite (TECDOC zone 22).
    pub graphite: Material,
    /// Boronated carbon brick (TECDOC zone 17).
    pub brick: Material,
}

/// The reflector materials: carbon at the table's atom density with its
/// natural boron, bound in graphite as tutorial 7's pebbles are.
pub fn reflector_materials() -> ReflectorMaterials {
    let graphite_law = Moderator::Graphite(GraphiteKind::ReactorPorosity30);
    let make = |carbon: f64, boron: f64, name: &str| {
        Material::from_atom_densities([
            (Element::Carbon.natural(), atoms_per_barn_cm(carbon)),
            (Element::Boron.natural(), atoms_per_barn_cm(boron)),
        ])
        .bound_in(graphite_law)
        .named(name)
    };
    ReflectorMaterials {
        graphite: make(ZONE_22_CARBON, ZONE_22_BORON, "reflector graphite"),
        brick: make(ZONE_17_CARBON, ZONE_17_BORON, "boronated carbon brick"),
    }
}

/// The components of the core, for the census.
#[derive(Debug, Clone, Copy)]
pub struct Components {
    /// The pebble's components, tutorial 7's.
    pub layer: layer::Components,
    /// The solid graphite reflector.
    pub reflector: Component,
    /// The boronated bricks.
    pub bricks: Component,
}

/// The built core.
pub struct SekerBedCore {
    /// The model: assemble it.
    pub model: Model,
    /// Its components.
    pub components: Components,
    /// Şeker layers.
    pub layers: usize,
    /// The bed's top \[cm\] above the floor (z = 0).
    pub bed_top_cm: f64,
}

/// **The tile**: tutorial 7's hexagonal prism of helium holding the 13 balls
/// it owns, every one a *fuel* ball here (the stack puts moderator balls
/// where the rule says).
pub fn tile(m: &Materials, c: &Components, ball: &FuelBall) -> Shape {
    let mut tile = HexPrism::flat_to_flat(cm(pitch_cm()), cm(height_cm()))
        .filled_with(m.helium.clone())
        .in_component(c.layer.helium);
    for site in own_sites() {
        tile = tile.containing(fuel_ball(m, &c.layer, ball, site_centre(site)));
    }
    tile
}

/// **The moderator ball**: graphite, the shell's, the fuel ball's radius.
pub fn moderator_ball(m: &Materials, c: &Components) -> Shape {
    Sphere::radius(cm(0.5 * BALL_DIAMETER_CM))
        .filled_with(m.shell.clone())
        .in_component(c.layer.moderator)
}

/// **The bed**: the tile stacked in a hopper, `layers` Şeker layers tall.
pub fn cavity(m: &Materials, c: &Components, ball: &FuelBall, layers: usize) -> Shape {
    let bed = Lattice::hex_bed(
        tile(m, c, ball),
        moderator_ball(m, c),
        Ratio::new::<ratio>(FUEL_BALL_FRACTION),
        cm(loading_height_cm(layers)),
    );
    Hopper::new(cm(CORE_RADIUS_CM), cm(CAVITY_HEIGHT_CM))
        .conical_floor(cm(CONE_HEIGHT_CM), cm(TUBE_RADIUS_CM))
        .outlet_tube(cm(TUBE_DEPTH_CM))
        .filled_with(m.helium.clone())
        .in_component(c.layer.helium)
        .containing_lattice(bed)
}

/// **The core**: the cavity in the graphite reflector in the boronated
/// bricks, vacuum outside. z = 0 is the floor of the bed.
pub fn build(
    data: NuclearData,
    m: &Materials,
    refl: &ReflectorMaterials,
    ball: &FuelBall,
    layers: usize,
    t_k: f64,
) -> SekerBedCore {
    let mut model = Model::new(data);
    let components = Components {
        layer: layer::Components {
            kernel: model.component("Kernel"),
            coatings: model.component("Coatings"),
            matrix: model.component("Matrix"),
            shell: model.component("Fuel ball shell"),
            moderator: model.component("Moderator ball"),
            helium: model.component("Helium"),
        },
        reflector: model.component("Graphite reflector"),
        bricks: model.component("Boronated bricks"),
    };
    // From the inside out, one statement each.
    let cavity = cavity(m, &components, ball, layers);
    let z_top = CAVITY_HEIGHT_CM + TOP_REFLECTOR_CM;
    let z_bottom = z_top - MODEL_HEIGHT_CM;
    let z_mid = 0.5 * (z_top + z_bottom);
    let graphite = Cylinder::radius(cm(GRAPHITE_OUTER_CM))
        .height(cm(MODEL_HEIGHT_CM))
        .centred_at(cm(0.0), cm(0.0), cm(z_mid))
        .filled_with(refl.graphite.clone())
        .in_component(components.reflector)
        .containing(cavity);
    let bricks = Cylinder::radius(cm(REFLECTOR_OUTER_CM))
        .height(cm(MODEL_HEIGHT_CM))
        .centred_at(cm(0.0), cm(0.0), cm(z_mid))
        .filled_with(refl.brick.clone())
        .in_component(components.bricks)
        .containing(graphite);
    model.add(bricks);
    model.set_temperature(Temperature::Uniform(
        ThermodynamicTemperature::new::<kelvin>(t_k),
    ));
    SekerBedCore {
        model,
        components,
        layers,
        bed_top_cm: loading_height_cm(layers),
    }
}
