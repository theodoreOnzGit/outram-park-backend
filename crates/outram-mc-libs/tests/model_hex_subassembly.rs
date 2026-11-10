//! # A hexagonal subassembly through `Model::assemble` (gh #1327)
//!
//! [`HexSubassembly`] builds a duct, a coolant prism and 37 three-layer pins
//! (4 rings) with a smeared wire wrap; this test puts it through the
//! model's own checks and lowering and looks at what the solver sees.
//!
//! **Inputs: generic, not any reactor's.** Pin pitch 0.8 cm, fuel radius
//! 0.25, cladding 0.28 / 0.33, wire 0.1 cm with a 20 cm lead, duct 0.1 cm
//! clear of the outer wires, 0.2 cm wall, 10 cm tall. Uranium metal at
//! 20 wt% U-235 (19.0 g/cm3), sodium bond and coolant (0.022 atoms/b-cm),
//! an 88/12 wt% Fe/Cr steel (7.8 g/cm3) for cladding, duct and wire. U-Zr
//! is not used: the reference tapes hold no zirconium.
//!
//! **Checks and predictions** (made before the first run):
//!
//! 1. `Model::assemble` accepts it with a reflective boundary (its own
//!    exact enclosure and overlap checks over 1 + 1 + 37 x 3 shapes).
//! 2. **One pair of z-planes.** Every solid is the full height, so the
//!    duct, the coolant prism and the pins all end on the same two planes.
//!    Prediction: exactly 2 `ZPlane` surfaces. Before this change a
//!    `HexPrism` always made two new z-planes, which gives **6** (duct 2,
//!    coolant prism 2, the pins' shared 2): coincident boundaries a
//!    neutron could be lost between. The lowering now shares them, as it
//!    already did for finite cylinders.
//! 3. **Volume fractions of the assembled geometry** (cell lookup on a
//!    901 x 901 grid at mid-height, material by the cell found) against the
//!    builder's analytic fractions, to 2e-3 (grid step 0.0063 cm against
//!    radii of 0.25-0.33 cm); the wire is in the coolant, so the sampled
//!    coolant is analytic coolant + wire. And no grid point inside the duct
//!    is unlocated.
//!
//! # Results (2026-11-10, release)
//!
//! 1. Assembled.
//! 2. 2 z-planes. **With the lowering change reverted, 6**, as predicted
//!    (measured once, then restored).
//! 3. 608 401 points inside the duct, none unlocated; worst
//!    sampled-vs-analytic difference 2.9e-4 (fuel 0.26604 vs 0.26589, duct
//!    0.13707 vs 0.13736). The first run counted 85 unlocated points: all
//!    on the outer duct faces to 1e-6 cm, a sampling artefact (lookup on a
//!    face is ambiguous), so points within 1e-9 cm of them are now left
//!    out. Record: `verification_and_validation/hex_subassembly/README.md`.
//!
//! Needs the reference ENDF tapes (`reference-data/endf/`) for the
//! materials' nuclides and masses (headers only: nothing is processed); it
//! skips when they are absent. Pictures:
//! `examples/hex_subassembly_geometry.rs`.

use njoy_outram_park_fork::reference_data::reference_endf_dir;
use outram_mc_libs::geometry::cell::SurfaceToken;
use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::geometry::surface::SurfaceKind;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::model::{HexSubassembly, PinDesign, SubassemblyMaterials, WireWrap};
use uom::si::f64::{Length, MassDensity, Ratio, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::mass_density::gram_per_cubic_centimeter;
use uom::si::ratio::{percent, ratio};
use uom::si::thermodynamic_temperature::kelvin;

/// The generic 4-ring subassembly and its materials (shared with the
/// pictures example by copy; both are generic).
pub fn generic_subassembly() -> (HexSubassembly, SubassemblyMaterials) {
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
    // 7.8 g/cm3 x 0.88 x N_A / 55.845 and x 0.12 / 51.996, atoms/b-cm.
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

#[test]
fn hex_subassembly_assembles_with_shared_end_planes_and_its_fractions() {
    let dir = reference_endf_dir();
    if !dir.join("n-011_Na_023-ENDF8.0.endf").exists() {
        eprintln!("SKIP: reference ENDF tapes not present");
        return;
    }
    let data = NuclearData::from_folder(&dir).unwrap();
    let (sa, materials) = generic_subassembly();
    let mut model = Model::new(data);
    model.add(sa.build(materials).unwrap().boundary(Boundary::Reflective));
    model.set_temperature(Temperature::Uniform(
        ThermodynamicTemperature::new::<kelvin>(293.6),
    ));
    // 1.
    let a = model.assemble().unwrap();
    // 2.
    let zplanes = a
        .geometry
        .surfaces
        .iter()
        .filter(|s| matches!(s, SurfaceKind::ZPlane(_)))
        .count();
    println!("z-planes: {zplanes}");
    assert_eq!(zplanes, 2);
    // 3.
    let v = sa.volume_fractions();
    let f_out = 3.0_f64.sqrt() * 3.0 * 0.8 + 0.86 + 0.2 + 0.4;
    let (hx, hy) = (f_out / 3.0_f64.sqrt(), 0.5 * f_out);
    let m = 901;
    let mut counts = vec![0usize; a.resolved.len()];
    let (mut inside, mut lost) = (0usize, 0usize);
    let s = 0.5 * 3.0_f64.sqrt();
    let normals = [
        [0.0, 1.0],
        [s, 0.5],
        [-s, 0.5],
        [0.0, -1.0],
        [-s, -0.5],
        [s, -0.5],
    ];
    for i in 0..m {
        for j in 0..m {
            let x = -hx + (i as f64 + 0.5) * 2.0 * hx / m as f64;
            let y = -hy + (j as f64 + 0.5) * 2.0 * hy / m as f64;
            let in_duct = normals
                .iter()
                .all(|n| n[0] * x + n[1] * y < 0.5 * f_out - 1e-9);
            // Points within 1e-9 cm of the outer faces are left out: on
            // a face, cell lookup is ambiguous by construction (a first
            // run found 85 such grid points, all at the outer apothem to
            // 1e-6 cm).
            if !in_duct {
                continue;
            }
            inside += 1;
            match a
                .geometry
                .locate(
                    Position::new(x, y, 0.0),
                    Direction::new(1.0, 0.0, 0.0),
                    SurfaceToken::NONE,
                )
                .and_then(|p| p.material)
            {
                Some(k) => counts[k] += 1,
                None => lost += 1,
            }
        }
    }
    assert_eq!(lost, 0, "unlocated points inside the duct");
    let frac = |name: &str| -> f64 {
        a.resolved
            .iter()
            .zip(&counts)
            .filter(|(r, _)| r.name == name)
            .map(|(_, c)| *c)
            .sum::<usize>() as f64
            / inside as f64
    };
    let mut worst: f64 = 0.0;
    for (name, want) in [
        ("U metal 20%", v.fuel.get::<ratio>()),
        ("Na bond", v.bond.get::<ratio>()),
        ("clad", v.clad.get::<ratio>()),
        ("duct", v.duct.get::<ratio>()),
        (
            "Na + wire (smeared)",
            v.coolant.get::<ratio>() + v.wire.get::<ratio>(),
        ),
    ] {
        let got = frac(name);
        println!("{name:>22}: sampled {got:.5}, analytic {want:.5}");
        worst = worst.max((got - want).abs());
        assert!((got - want).abs() < 2e-3, "{name}: {got} vs {want}");
    }
    println!("worst |sampled - analytic| = {worst:.2e} over {inside} points");
}
