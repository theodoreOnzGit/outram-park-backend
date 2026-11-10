//! # Pictures of a hexagonal subassembly (gh #1327, crate HARD RULE)
//!
//! Draws, from the **assembled** geometry (cell lookup per pixel, the
//! ported OpenMC plotter), the generic 4-ring subassembly of
//! `tests/model_hex_subassembly.rs`: 37 pins at 0.8 cm pitch, fuel 0.25 cm,
//! cladding 0.28 / 0.33 cm, a 0.1 cm wire (smeared into the coolant, so not
//! drawn), duct 0.1 cm clear of the outer wires with a 0.2 cm wall, 10 cm
//! tall. Generic numbers, not any reactor's design.
//!
//! Writes to `verification_and_validation/hex_subassembly/`:
//! - `hex_subassembly_xy.png`: the whole section at mid-height;
//! - `hex_subassembly_xz.png`: x-z through y = 0 (the centre row of pins);
//! - `hex_subassembly_pin_xy.png`: one corner pin and the duct beside it.
//!
//! Needs the reference ENDF tapes (headers only, nothing processed).
//!
//! ```text
//! cargo run --release -p outram-mc-libs --example hex_subassembly_geometry
//! ```
//!
//! `main` and the drawing glue are untested by design (CLI glue, workspace
//! rule exception); the subassembly they draw is tested in
//! `tests/model_hex_subassembly.rs` and `model::hex_subassembly::tests`.

use std::path::Path;

use njoy_outram_park_fork::reference_data::reference_endf_dir;
use outram_mc_libs::geometry::plot::PlotBasis;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::model::{HexSubassembly, PinDesign, SubassemblyMaterials, WireWrap};
use uom::si::f64::{Length, MassDensity, Ratio, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::mass_density::gram_per_cubic_centimeter;
use uom::si::ratio::percent;
use uom::si::thermodynamic_temperature::kelvin;

/// The generic subassembly (a copy of the test's; both generic).
fn generic_subassembly() -> (HexSubassembly, SubassemblyMaterials) {
    let cm = Length::new::<centimeter>;
    let pin = PinDesign {
        fuel_radius: cm(0.25),
        clad_inner_radius: cm(0.28),
        clad_outer_radius: cm(0.33),
    };
    let (p, s, rings) = (0.8, 0.1, 4);
    let f_in = 3.0_f64.sqrt() * (rings as f64 - 1.0) * p + 0.66 + 2.0 * s + 0.2;
    let sa =
        HexSubassembly::new(cm(f_in), cm(0.2), cm(10.0), cm(p), rings, pin).wire_wrap(WireWrap {
            diameter: cm(s),
            lead: cm(20.0),
        });
    let steel = || {
        Material::from_fractions(
            Basis::Weight,
            [
                (Element::Iron.natural(), Ratio::new::<percent>(88.0)),
                (Element::Chromium.natural(), Ratio::new::<percent>(12.0)),
            ],
        )
        .density(MassDensity::new::<gram_per_cubic_centimeter>(7.8))
    };
    let wire = Material::from_atom_densities([
        (Element::Iron.natural(), atoms_per_barn_cm(0.07402)),
        (Element::Chromium.natural(), atoms_per_barn_cm(0.01084)),
    ])
    .named("wire");
    let sodium = || Material::from_atom_densities([(Na23, atoms_per_barn_cm(0.022))]);
    let materials = SubassemblyMaterials {
        fuel: Material::fuel(
            HeavyMetal::Uranium(Enrichment::by_weight(
                FissileUranium::U235,
                Ratio::new::<percent>(20.0),
            )),
            FuelForm::Metallic,
        )
        .density(MassDensity::new::<gram_per_cubic_centimeter>(19.0))
        .named("U metal 20%"),
        bond: sodium().named("Na bond"),
        clad: steel().named("clad"),
        coolant: sodium().named("Na"),
        duct: steel().named("duct"),
        wire: Some(wire),
    };
    (sa, materials)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (sa, materials) = generic_subassembly();
    let mut model = Model::new(NuclearData::from_folder(&reference_endf_dir())?);
    model.add(sa.build(materials)?.boundary(Boundary::Reflective));
    model.set_temperature(Temperature::Uniform(
        ThermodynamicTemperature::new::<kelvin>(293.6),
    ));
    let a = model.assemble()?;
    let out =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/hex_subassembly");
    std::fs::create_dir_all(&out)?;
    let f_out = sa.volume_fractions(); // printed below with the pictures
    let width = 6.6;
    let corner = sa.pin_centres_cm()[0];
    for (img, file) in [
        (
            a.draw_window(
                PlotBasis::Xy,
                Position::new(0.0, 0.0, 0.0),
                [width, width],
                1000,
                &format!(
                    "hex subassembly, z = 0: {} pins, {} rings, pitch 0.8 cm, duct 0.2 cm wall",
                    sa.pin_count(),
                    sa.rings()
                ),
            ),
            "hex_subassembly_xy.png",
        ),
        (
            a.draw_window(
                PlotBasis::Xz,
                Position::new(0.0, 0.0, 0.0),
                [11.0, 11.0],
                1000,
                "hex subassembly, x-z at y = 0: the centre row of 7 pins, full height 10 cm",
            ),
            "hex_subassembly_xz.png",
        ),
        (
            a.draw_window(
                PlotBasis::Xy,
                Position::new(corner[0], corner[1], 0.0),
                [1.6, 1.6],
                800,
                &format!(
                    "one corner pin at ({:.3}, {:.3}) cm: fuel 0.25, bond to 0.28, clad to 0.33 cm",
                    corner[0], corner[1]
                ),
            ),
            "hex_subassembly_pin_xy.png",
        ),
    ] {
        let p = out.join(file);
        img.write_png(&p)?;
        println!("drew {}", p.display());
    }
    println!("analytic volume fractions: {f_out:?}");
    Ok(())
}
