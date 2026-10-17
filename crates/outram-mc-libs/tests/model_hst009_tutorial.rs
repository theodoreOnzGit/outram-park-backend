//! **Tutorial 3's model is the V&V model** (gh #943): HEU-SOL-THERM-009
//! case 1 built through the beginner API (`examples/tutorial_hst009.rs`)
//! lowers to exactly what `vv::hst009` hands the transport.
//!
//! # Methodology
//!
//! The tutorial's statements, repeated here (inside out, one `.containing`
//! per statement; the spec's atom densities from `vv::hst009::MATERIALS`,
//! the tank from the `Material::aluminium_1100()` preset), assembled against
//! the workspace's ENDF/B-VIII.0 headers (no nuclear data processed), and
//! compared with `vv::hst009::geometry()` and `vv::hst009::materials()`:
//!
//! - every surface: centre, radius and boundary condition, `==`;
//! - every cell: region tokens and fill, `==`;
//! - every transport material: the same nuclides (by name, through the
//!   assembled nuclide list) at the same atom densities, **to one ulp**.
//!   Bit identity was the intent (a V&V case moved onto the API keeps its
//!   k bit for bit, #943 acceptance) and is not reached: `AtomDensity`
//!   stores per m³ and the conversion is not one-to-one on `f64`
//!   (`model::material::tests::atom_density_round_trip_is_within_one_ulp`);
//! - H-1 carries the H in H2O law in the solution and the reflector, and
//!   nothing else carries a law.
//!
//! # Results (2026-10-18)
//!
//! Passes: three surfaces and three cells identical; of the 14 (nuclide,
//! density) pairs, 11 bit-identical and 3 one ulp off (U-236 and F-19 in
//! the solution, Mn-55 in the tank); H in H2O on H-1 only.

use outram_mc_libs::geometry::surface::SurfaceKind;
use outram_mc_libs::model::prelude::*;
use outram_mc_libs::vv::hst009;
use uom::si::f64::{Length, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::thermodynamic_temperature::kelvin;

/// The tutorial's model, statement for statement.
fn tutorial_model(data: NuclearData) -> Model {
    let mut model = Model::new(data);
    let solution_c = model.component("Fuel solution");
    let tank_c = model.component("Tank");
    let water_c = model.component("Reflector");
    let spec = |mat: usize, name: &str| {
        atoms_per_barn_cm(
            hst009::MATERIALS[mat]
                .1
                .iter()
                .find(|(n, _)| *n == name)
                .map_or(0.0, |(_, d)| *d),
        )
    };
    let solution = Material::from_atom_densities([
        (U234, spec(hst009::MAT_SOLUTION, "U234")),
        (U235, spec(hst009::MAT_SOLUTION, "U235")),
        (U236, spec(hst009::MAT_SOLUTION, "U236")),
        (U238, spec(hst009::MAT_SOLUTION, "U238")),
        (F19, spec(hst009::MAT_SOLUTION, "F19")),
        (O16, spec(hst009::MAT_SOLUTION, "O16")),
        (H1, spec(hst009::MAT_SOLUTION, "H1")),
    ])
    .bound_in(Moderator::LightWater)
    .named("uranium oxyfluoride solution");
    let tank = Material::aluminium_1100();
    let water = Material::from_atom_densities([
        (H1, spec(hst009::MAT_WATER, "H1")),
        (O16, spec(hst009::MAT_WATER, "O16")),
    ])
    .bound_in(Moderator::LightWater)
    .named("water reflector");
    let cm = Length::new::<centimeter>;
    let solution_ball = Sphere::radius(cm(hst009::R_SOLUTION))
        .filled_with(solution)
        .in_component(solution_c);
    let tank_ball = Sphere::radius(cm(hst009::R_TANK))
        .filled_with(tank)
        .in_component(tank_c)
        .containing(solution_ball);
    let reflector = Sphere::radius(cm(hst009::R_REFLECTOR))
        .filled_with(water)
        .in_component(water_c)
        .containing(tank_ball);
    model.add(reflector);
    model.set_temperature(Temperature::Uniform(
        ThermodynamicTemperature::new::<kelvin>(hst009::TEMPERATURE_K),
    ));
    model
}

#[test]
fn tutorial_hst009_lowers_to_the_vv_model() {
    let data =
        NuclearData::workspace_reference().expect("reference-data/endf is tracked in the repo");
    let a = tutorial_model(data).assemble().expect("assembles");
    let g = hst009::geometry();

    assert_eq!(a.geometry.surfaces.len(), g.surfaces.len());
    for (x, y) in a.geometry.surfaces.iter().zip(&g.surfaces) {
        match (x, y) {
            (SurfaceKind::Sphere(p), SurfaceKind::Sphere(q)) => {
                assert_eq!((p.x0, p.y0, p.z0, p.r, p.bc), (q.x0, q.y0, q.z0, q.r, q.bc))
            }
            other => panic!("not two spheres: {other:?}"),
        }
    }
    assert_eq!(a.geometry.cells.len(), g.cells.len());
    for (x, y) in a.geometry.cells.iter().zip(&g.cells) {
        assert_eq!(x.region, y.region);
        assert_eq!(x.fill, y.fill);
    }

    let vv = hst009::materials();
    assert_eq!(a.materials.len(), vv.len());
    let mut pairs = 0;
    let mut differing = Vec::new();
    for (m, (ours, theirs)) in a.materials.iter().zip(&vv).enumerate() {
        assert_eq!(
            ours.components.len(),
            theirs.components.len(),
            "material {m}"
        );
        for c in &theirs.components {
            let name = hst009::NUCLIDES[c.nuclide_idx].0;
            let mine = ours
                .components
                .iter()
                .find(|o| a.nuclides[o.nuclide_idx].isotope.name() == name)
                .unwrap_or_else(|| panic!("{name} missing from material {m}"));
            let ulps = (mine.atom_density.to_bits() as i64 - c.atom_density.to_bits() as i64).abs();
            if ulps != 0 {
                println!(
                    "{name} in material {m}: {:e} against the V&V model's {:e} ({ulps} ulp)",
                    mine.atom_density, c.atom_density
                );
                differing.push(name);
            }
            assert!(ulps <= 1, "{name} in material {m}: {ulps} ulp apart");
            pairs += 1;
        }
    }
    assert_eq!(pairs, 14);
    assert_eq!(
        differing,
        ["U236", "F19", "Mn55"],
        "the measured set; a change is a finding"
    );
    for k in &a.nuclides {
        assert_eq!(k.law.is_some(), k.isotope == H1, "{}", k.isotope.name());
    }
}
