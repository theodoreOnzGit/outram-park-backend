// SPDX-License-Identifier: GPL-3.0

//! **Tutorial 6's model: LEU-COMP-THERM-008 case 1 built through the
//! beginner API** (`outram_mc_libs::model`, epic gh #925, tutorial list on
//! #943). Shared by `examples/tutorial_lct008.rs` and
//! `tests/model_lct008_tutorial.rs`, so the lesson and the test that pins it
//! to the V&V model run one set of statements. Pulled in with `#[path]`
//! beside `lct008_model.rs`, which it reads its cards through.
//!
//! The specification is the OpenMC model in `mit-crpg/benchmarks` (Paul
//! Romano, MIT licence) committed under
//! `verification_and_validation/icsbep/leu-comp-therm-008/`: atom densities
//! of three materials, the 15 x 15 pin maps of 22 assemblies and the 7 x 7
//! core map are **read from those cards**, not transcribed, as
//! `lct008_model.rs` reads them. What is written here, in the beginner API's
//! words, is the structure:
//!
//! ```text
//! pin tile    water box, pitch x pitch, holding a clad rod holding a fuel rod
//! assembly    water box, one 15 x 15 lattice of pin tiles
//! core        a finite cylinder (r 76.2 cm, |z| < 81.662 cm), one 7 x 7
//!             lattice of assembly tiles, vacuum outside
//! ```
//!
//! # What is the same as the V&V model, and what is stated
//!
//! The same cards, radii, pitches and temperature; the same 35 nuclides at
//! the same atom densities; the same pin maps. The pin radii are the
//! hand-read constants `lct008_model::{R_FUEL, R_CLAD}` (the V&V model's own
//! `PIN_SHELLS` transcription), the core's `R_CORE`, `Z_LO`, `Z_HI`.
//! **Only case 1** is built: case 8's pyrex rods (pin universe 3) are
//! refused with a message.
//!
//! # Simplifications, with their kind (tutorial 6 page)
//!
//! - One water component for every water region (pin background, assembly
//!   gap, core): a label for output, no physics.
//! - A one-layer lattice is extruded through its container's height, so a
//!   tile's own height is only a label: set to the core's. **Negligible**:
//!   the cards have no axial structure either (the lattice's z pitch is 1 cm
//!   with one layer, `lct008_model.rs`).
//! - Temperature one value, 293.6 K, everywhere. The cards give no
//!   `<temperature>`, so OpenMC's default applies (`lct008_model::TEMP_K`).

#![allow(dead_code)]

use super::lct008_model::{
    attr, block, elements, parse_materials, strip_comments, GEOMETRY_XML_CASE1,
    MATERIALS_XML_CASE1, R_CLAD, R_CORE, R_FUEL, TEMP_K, Z_HI, Z_LO,
};
use outram_mc_libs::model::prelude::*;
use std::collections::BTreeMap;
use uom::si::f64::{Length, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::thermodynamic_temperature::kelvin;

/// What the cards say, for the lesson to check the assembled geometry
/// against.
#[derive(Debug, Clone)]
pub struct Cards {
    /// Fuel pins in the core, summed over the core map (the benchmark's
    /// 4961 for case 1).
    pub fuel_pins: usize,
    /// Assemblies in the core map that hold pins (the rest are water).
    pub assemblies: usize,
    /// Distinct assembly lattices used.
    pub distinct_assemblies: usize,
    /// Pin pitch \[cm\].
    pub pin_pitch_cm: f64,
    /// Assembly pitch \[cm\].
    pub assembly_pitch_cm: f64,
    /// Nuclides in the model's three materials, after `keep`.
    pub nuclides: usize,
}

/// The built model and what the cards say about it.
pub struct Lct008 {
    /// The model: add nothing to it, assemble it.
    pub model: Model,
    /// The cards' own numbers.
    pub cards: Cards,
}

/// **The lesson's trap: which row of the cards is the top one.** OpenMC's
/// `<universes>` runs top row first (`openmc/lattice.py:382-388`); outram's CSG
/// lattice starts at the lower-left (`rect.rs:45-47`). The beginner API makes
/// the reader choose by the constructor's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowOrder {
    /// `Lattice::from_rows_top_down`: the first row of the cards is the top
    /// one. **Right** for these cards.
    TopDown,
    /// `Lattice::from_natural_order`: the first row of the cards is the
    /// bottom one. **Wrong** for these cards: a mirror image in y.
    Natural,
}

fn grid(order: RowOrder, tiles: Vec<Shape>, rows: Vec<Vec<usize>>) -> Lattice {
    match order {
        RowOrder::TopDown => Lattice::from_rows_top_down(tiles, rows),
        RowOrder::Natural => {
            let n = [rows.first().map_or(0, Vec::len), rows.len(), 1];
            Lattice::from_natural_order(tiles, n, rows.into_iter().flatten().collect())
        }
    }
}

/// One lattice of the cards: its size, pitch and universe map, top row first
/// (OpenMC's order, `openmc/lattice.py`).
struct CardLattice {
    n: (usize, usize),
    pitch: f64,
    rows: Vec<Vec<i32>>,
}

fn card_lattices(xml: &str) -> BTreeMap<i32, CardLattice> {
    let mut out = BTreeMap::new();
    for (a, body) in elements(xml, "lattice") {
        let id: i32 = attr(a, "id")
            .expect("lattice id")
            .parse()
            .expect("lattice id");
        let dim: Vec<usize> = attr(a, "dimension")
            .expect("lattice dimension")
            .split_whitespace()
            .map(|t| t.parse().expect("dimension"))
            .collect();
        let (nx, ny) = (dim[0], dim[1]);
        let pitch: Vec<f64> = block(body, "pitch");
        assert_eq!(pitch[0], pitch[1], "lattice {id} has a rectangular pitch");
        let flat: Vec<i32> = block(body, "universes");
        assert_eq!(
            flat.len(),
            nx * ny,
            "lattice {id}: entries against its size"
        );
        let rows = flat.chunks(nx).map(<[i32]>::to_vec).collect();
        out.insert(
            id,
            CardLattice {
                n: (nx, ny),
                pitch: pitch[0],
                rows,
            },
        );
    }
    out
}

/// **Build LEU-COMP-THERM-008 case 1** on `data`, with the nuclides `keep`
/// accepts (`|_| true` for the whole model: 35 nuclides). A test with a small
/// embedded library keeps the ones it has.
///
/// # Errors
/// A card the beginner model cannot hold (a pin universe other than the
/// water pin 1 and the fuel pin 2), or any [`ModelError`].
pub fn build(
    data: NuclearData,
    keep: impl Fn(&str) -> bool,
) -> Result<Lct008, Box<dyn std::error::Error>> {
    build_ordered(data, keep, RowOrder::TopDown, RowOrder::TopDown)
}

/// [`build`], with the row order named for the core's lattice and for the
/// assemblies' (the right one is [`RowOrder::TopDown`] for both; the others
/// draw the trap).
pub fn build_ordered(
    data: NuclearData,
    keep: impl Fn(&str) -> bool,
    core_order: RowOrder,
    assembly_order: RowOrder,
) -> Result<Lct008, Box<dyn std::error::Error>> {
    let cm = Length::new::<centimeter>;
    let xml = strip_comments(GEOMETRY_XML_CASE1);
    let lattices = card_lattices(&xml);
    // The assembly wrappers: universe -> lattice (a cell `universe=U fill=L`
    // whose fill is a lattice rather than a universe).
    let mut wrapper: BTreeMap<i32, i32> = BTreeMap::new();
    for (a, _) in elements(&xml, "cell") {
        let universe: i32 = attr(a, "universe").expect("cell universe").parse()?;
        if let Some(fill) = attr(a, "fill") {
            let fill: i32 = fill.parse()?;
            if lattices.contains_key(&fill) {
                wrapper.insert(universe, fill);
            }
        }
    }

    let mut model = Model::new(data);
    let fuel_c = model.component("Fuel");
    let clad_c = model.component("Cladding");
    let water_c = model.component("Water");

    // ── Materials: the cards' atom densities, in the cards' order ──────────
    let spec = parse_materials(MATERIALS_XML_CASE1);
    let mut nuclides = std::collections::BTreeSet::new();
    let mut material = |m: &super::lct008_model::MaterialSpec| {
        Material::from_atom_densities(
            m.nuclides
                .iter()
                .filter(|(name, _)| keep(name))
                .map(|(name, ao)| {
                    nuclides.insert(name.clone());
                    (
                        Isotope::from_name(name).expect("a card's nuclide name"),
                        atoms_per_barn_cm(*ao),
                    )
                })
                .collect::<Vec<_>>(),
        )
        .named(&m.name)
    };
    let water = material(&spec[0]).bound_in(Moderator::LightWater);
    let fuel = material(&spec[1]);
    let clad = material(&spec[2]);

    // ── The pin: a fuel rod in a clad rod in a water box ───────────────────
    let pin_pitch = lattices[&11].pitch;
    let assembly_pitch = lattices[&99].pitch;
    let height = cm(Z_HI - Z_LO);
    let fuel_rod = Cylinder::radius(cm(R_FUEL))
        .filled_with(fuel)
        .in_component(fuel_c);
    let clad_rod = Cylinder::radius(cm(R_CLAD))
        .filled_with(clad)
        .in_component(clad_c)
        .containing(fuel_rod);
    let fuel_pin = Cuboid::sides(cm(pin_pitch), cm(pin_pitch), height)
        .filled_with(water.clone())
        .in_component(water_c)
        .containing(clad_rod);
    let water_pin = Cuboid::sides(cm(pin_pitch), cm(pin_pitch), height)
        .filled_with(water.clone())
        .in_component(water_c);

    // ── An assembly per distinct 15 x 15 map, a water box for the rest ─────
    let core_map = &lattices[&99];
    let mut assembly_tiles = vec![
        Cuboid::sides(cm(assembly_pitch), cm(assembly_pitch), height)
            .filled_with(water.clone())
            .in_component(water_c),
    ];
    let mut tile_of_universe: BTreeMap<i32, usize> = BTreeMap::new();
    let (mut fuel_pins, mut assemblies) = (0, 0);
    for row in &core_map.rows {
        for &universe in row {
            let Some(&lattice_id) = wrapper.get(&universe) else {
                continue; // a water assembly (universe 999 fills with water)
            };
            assemblies += 1;
            let card = &lattices[&lattice_id];
            assert_eq!(card.n, (15, 15), "assembly lattice {lattice_id} is 15 x 15");
            assert_eq!(
                card.pitch, pin_pitch,
                "assembly lattice {lattice_id}'s pitch"
            );
            fuel_pins += card.rows.iter().flatten().filter(|&&u| u == 2).count();
            if tile_of_universe.contains_key(&universe) {
                continue;
            }
            let pin_rows: Result<Vec<Vec<usize>>, String> = card
                .rows
                .iter()
                .map(|r| {
                    r.iter()
                        .map(|&u| match u {
                            1 => Ok(0), // the water pin
                            2 => Ok(1), // the fuel pin
                            other => Err(format!(
                                "pin universe {other} (case 8's pyrex rod) is not in tutorial 6"
                            )),
                        })
                        .collect()
                })
                .collect();
            let pins = grid(
                assembly_order,
                vec![water_pin.clone(), fuel_pin.clone()],
                pin_rows?,
            );
            assembly_tiles.push(
                Cuboid::sides(cm(assembly_pitch), cm(assembly_pitch), height)
                    .filled_with(water.clone())
                    .in_component(water_c)
                    .containing_lattice(pins),
            );
            tile_of_universe.insert(universe, assembly_tiles.len() - 1);
        }
    }
    let distinct_assemblies = assembly_tiles.len() - 1;

    // ── The core: a 7 x 7 lattice of assembly tiles, cut by a cylinder ─────
    let core_rows: Vec<Vec<usize>> = core_map
        .rows
        .iter()
        .map(|r| {
            r.iter()
                .map(|u| tile_of_universe.get(u).copied().unwrap_or(0))
                .collect()
        })
        .collect();
    let core_lattice = grid(core_order, assembly_tiles, core_rows);
    let core = Cylinder::radius(cm(R_CORE))
        .height(height)
        .filled_with(water)
        .in_component(water_c)
        .containing_lattice(core_lattice);
    model.add(core);
    model.set_temperature(Temperature::Uniform(
        ThermodynamicTemperature::new::<kelvin>(TEMP_K),
    ));
    Ok(Lct008 {
        model,
        cards: Cards {
            fuel_pins,
            assemblies,
            distinct_assemblies,
            pin_pitch_cm: pin_pitch,
            assembly_pitch_cm: assembly_pitch,
            nuclides: nuclides.len(),
        },
    })
}
