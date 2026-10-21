//! **Per-region temperature in the lowering** (gh #945, stage 1): which
//! temperature each shape takes, how materials and nuclide keys split by
//! temperature, the refusals, and per-component temperatures taken from a
//! `ThermalProfile` (#949). No nuclear data is processed here (header-only
//! library); the transport checks are `tests/region_temperature.rs`.

use super::dh_region::{DhRegion, Packing};
use super::isotope::isotopes::*;
use super::material::{atoms_per_barn_cm, Material};
use super::model::tests::fake_data;
use super::model::{Model, ModelError, Temperature};
use super::shape::Sphere;
use crate::geometry::cell::CellFill;
use crate::geometry::position::Position;
use crate::thermal_profile::{HeatRemoval, ThermalMaterial, ThermalMesh, ThermalProblem, ThermalRegion};
use uom::si::f64::{Length, Power, Ratio, ThermalConductivity, ThermodynamicTemperature};
use uom::si::length::centimeter;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

fn cm(x: f64) -> Length {
    Length::new::<centimeter>(x)
}

fn kelvins(t: f64) -> Temperature {
    Temperature::Uniform(ThermodynamicTemperature::new::<kelvin>(t))
}

fn heu() -> Material {
    Material::from_atom_densities([
        (U235, atoms_per_barn_cm(4.4994e-2)),
        (U238, atoms_per_barn_cm(2.4984e-3)),
    ])
    .named("HEU")
}

fn oxygen() -> Material {
    Material::from_atom_densities([(O16, atoms_per_barn_cm(0.05))]).named("O")
}

/// Material temperatures by cell, in cell order (`None` for fill cells).
fn cell_temperatures(a: &super::AssembledModel) -> Vec<Option<f64>> {
    a.geometry
        .cells
        .iter()
        .map(|c| match c.fill {
            CellFill::Material(m) => {
                assert_eq!(
                    c.temperature, a.materials[m].temperature,
                    "cell and material agree"
                );
                Some(a.materials[m].temperature)
            }
            _ => None,
        })
        .collect()
}

/// **Precedence**: a shape's own temperature, then its component's, then
/// its container's, then the model's. One material entry and one nuclide
/// key per (material, temperature); each nuclide key carries its
/// temperature.
#[test]
fn shapes_and_components_take_their_own_temperatures() {
    let mut m = Model::new(fake_data());
    m.set_temperature(kelvins(293.6));
    let hot = m.component("hot");
    let pinned = m.component("pinned");
    m.set_component_temperature(hot, kelvins(1200.0));
    m.set_component_temperature(pinned, kelvins(450.0));
    // A later setting for the same component replaces the first.
    m.set_component_temperature(pinned, kelvins(500.0));
    assert_eq!(m.component_temperature(pinned), Some(kelvins(500.0)));
    let inherits = Sphere::radius(cm(0.5)).filled_with(heu());
    let in_hot = Sphere::radius(cm(0.5))
        .centred_at(cm(1.5), cm(0.0), cm(0.0))
        .filled_with(heu())
        .in_component(hot);
    let own_beats_component = Sphere::radius(cm(0.5))
        .centred_at(cm(-1.5), cm(0.0), cm(0.0))
        .filled_with(heu())
        .in_component(pinned)
        .at_temperature(kelvins(600.0));
    assert_eq!(own_beats_component.own_temperature(), Some(kelvins(600.0)));
    let middle = Sphere::radius(cm(3.0))
        .filled_with(oxygen())
        .at_temperature(kelvins(900.0))
        .containing(inherits)
        .containing(in_hot)
        .containing(own_beats_component);
    m.add(
        Sphere::radius(cm(5.0))
            .filled_with(oxygen())
            .containing(middle),
    );
    let a = m.assemble().unwrap();
    // Cells inside out: the three spheres, the middle, the outer.
    assert_eq!(
        cell_temperatures(&a),
        vec![
            Some(900.0),
            Some(1200.0),
            Some(600.0),
            Some(900.0),
            Some(293.6)
        ]
    );
    assert_eq!(
        a.temperature_k, 293.6,
        "the model's temperature is the default"
    );
    assert_eq!(a.temperatures_k(), vec![293.6, 600.0, 900.0, 1200.0]);
    // HEU at 900, 1200, 600; O at 900 and 293.6: five materials.
    assert_eq!(a.materials.len(), 5);
    // Each (nuclide, T) once: U235/U238 at three temperatures, O16 at two.
    assert_eq!(a.nuclides.len(), 8);
    for mat in &a.materials {
        for c in &mat.components {
            assert_eq!(
                a.nuclides[c.nuclide_idx].temperature.get::<kelvin>(),
                mat.temperature,
                "every material indexes nuclides at its own temperature"
            );
        }
    }
    let at = |t: f64| {
        a.nuclides
            .iter()
            .filter(|k| k.temperature.get::<kelvin>() == t)
            .count()
    };
    assert_eq!((at(900.0), at(1200.0), at(600.0), at(293.6)), (3, 2, 2, 1));
}

/// **All shapes at the model's temperature assemble to exactly what the
/// model temperature alone gives**: the same cells, materials and nuclide
/// keys (the transport bit-for-bit check is `tests/region_temperature.rs`).
#[test]
fn every_shape_at_the_model_temperature_changes_nothing() {
    let build = |explicit: bool| {
        let mut m = Model::new(fake_data());
        m.set_temperature(kelvins(293.6));
        let t = |s: super::Shape| {
            if explicit {
                s.at_temperature(kelvins(293.6))
            } else {
                s
            }
        };
        let fuel = t(Sphere::radius(cm(1.0)).filled_with(heu()));
        let shell = t(Sphere::radius(cm(2.0))
            .filled_with(oxygen())
            .containing(fuel));
        m.add(shell);
        m.assemble().unwrap()
    };
    let (plain, explicit) = (build(false), build(true));
    assert_eq!(plain.nuclides, explicit.nuclides);
    assert_eq!(plain.materials.len(), explicit.materials.len());
    for (x, y) in plain.materials.iter().zip(&explicit.materials) {
        assert_eq!(
            (x.id, &x.name, x.temperature),
            (y.id, &y.name, y.temperature)
        );
        let c = |m: &crate::material::material::Material| {
            m.components
                .iter()
                .map(|c| (c.nuclide_idx, c.atom_density.to_bits()))
                .collect::<Vec<_>>()
        };
        assert_eq!(c(x), c(y));
    }
    for (x, y) in plain.geometry.cells.iter().zip(&explicit.geometry.cells) {
        assert_eq!((&x.region, &x.fill), (&y.region, &y.fill));
        assert_eq!(x.temperature.to_bits(), y.temperature.to_bits());
    }
    assert_eq!(plain.temperatures_k(), vec![293.6]);
}

/// Refusals: a non-positive shape or component temperature, and a
/// component of another model.
#[test]
fn bad_region_temperatures_are_refused() {
    let refused = |m: &Model, words: &str| match m.assemble() {
        Err(ModelError::BadValue(w)) => assert!(w.contains(words), "{w}"),
        other => panic!("{words}: {other:?}"),
    };
    let mut m = Model::new(fake_data());
    m.set_temperature(kelvins(293.6));
    m.add(
        Sphere::radius(cm(1.0))
            .filled_with(heu())
            .at_temperature(kelvins(0.0)),
    );
    refused(&m, "must be positive");

    let mut m = Model::new(fake_data());
    m.set_temperature(kelvins(293.6));
    let c = m.component("fuel");
    m.set_component_temperature(c, kelvins(f64::NAN));
    m.add(Sphere::radius(cm(1.0)).filled_with(heu()).in_component(c));
    refused(&m, "component fuel's");

    let mut other = Model::new(fake_data());
    let foreign = other.component("x");
    let mut m = Model::new(fake_data());
    m.set_temperature(kelvins(293.6));
    m.set_component_temperature(foreign, kelvins(600.0));
    assert_eq!(m.component_temperature(foreign), Some(kelvins(600.0)));
    m.add(Sphere::radius(cm(1.0)).filled_with(heu()));
    assert!(matches!(m.assemble(), Err(ModelError::ForeignComponent)));
}

/// **A delta-tracked region at its own temperature**: the matrix and the
/// particles that set none take the region's; a particle with its own keeps
/// it; the region's majorant set holds every material at every temperature
/// present in it (lattice and random packings).
#[test]
fn a_dh_region_at_its_own_temperature() {
    for random in [false, true] {
        let mut m = Model::new(fake_data());
        m.set_temperature(kelvins(293.6));
        let kernel = Sphere::radius(cm(0.02)).filled_with(heu());
        let particle = Sphere::radius(cm(0.04))
            .filled_with(oxygen())
            .containing(kernel.at_temperature(kelvins(1500.0)));
        let packing = if random {
            Packing::Random {
                fraction: Ratio::new::<ratio>(0.05),
                seed: 3,
            }
        } else {
            Packing::Lattice {
                pitch: cm(0.2),
                offset: [0.5; 3],
            }
        };
        let zone = DhRegion::new(Sphere::radius(cm(0.5)), oxygen())
            .with_particles(particle, packing)
            .delta_tracking()
            .at_temperature(kelvins(1200.0));
        m.add(
            Sphere::radius(cm(0.6))
                .filled_with(oxygen())
                .containing(zone),
        );
        let a = m.assemble().unwrap();
        assert_eq!(a.delta_regions().len(), 1);
        let mut in_region: Vec<f64> = a.delta_regions()[0]
            .iter()
            .map(|&i| a.materials[i].temperature)
            .collect();
        in_region.sort_by(f64::total_cmp);
        in_region.dedup();
        assert_eq!(in_region, vec![1200.0, 1500.0], "random = {random}");
        assert_eq!(a.temperatures_k(), vec![293.6, 1200.0, 1500.0]);
        // Every material cell in the region is at 1200 K (matrix, coating)
        // or 1500 K (kernel); only the outer shell is at 293.6 K.
        let n_cold = cell_temperatures(&a)
            .iter()
            .filter(|t| **t == Some(293.6))
            .count();
        assert_eq!(n_cold, 1, "random = {random}");
    }
}

/// **Per-component temperatures from a solved `ThermalProfile`** (#949):
/// a heated sphere in a shell, the surface held at 500 K. The fuel's mean
/// is above the shell's, both above 500 K; each is set on its component and
/// the next assembly processes the materials there. A model with no
/// component in the conducting body is refused, naming the fix.
#[test]
fn components_take_their_mean_from_a_thermal_profile() {
    let mut m = Model::new(fake_data());
    m.set_temperature(kelvins(293.6));
    let fuel = m.component("fuel");
    let shell = m.component("shell");
    let build = |fuel_c, shell_c| {
        let mut f = Sphere::radius(cm(1.0)).filled_with(heu());
        let mut s = Sphere::radius(cm(1.3)).filled_with(oxygen());
        if let Some(c) = fuel_c {
            f = f.in_component(c);
        }
        if let Some(c) = shell_c {
            s = s.in_component(c);
        }
        s.containing(f)
    };
    m.add(build(Some(fuel), Some(shell)));
    let a = m.assemble().unwrap();
    let k = ThermalMaterial::Constant(ThermalConductivity::new::<watt_per_meter_kelvin>(3.0));
    let mesh = ThermalMesh {
        lower: Position::new(-1.5, -1.5, -1.5),
        upper: Position::new(1.5, 1.5, 1.5),
        voxels_across: 24,
    };
    let profile = ThermalProblem::uniform_heat(Power::new::<watt>(500.0))
        .removed_by(HeatRemoval::FixedSurfaceTemperature(
            ThermodynamicTemperature::new::<kelvin>(500.0),
        ))
        .solve(
            &a.geometry,
            &[ThermalRegion::heated(0, k), ThermalRegion::unheated(1, k)],
            &mesh,
        )
        .unwrap();
    let taken = m.take_temperatures_from(&profile).unwrap();
    assert_eq!(taken.len(), 2);
    assert_eq!((taken[0].0, taken[1].0), (fuel, shell));
    let (t_fuel, t_shell) = (taken[0].1.get::<kelvin>(), taken[1].1.get::<kelvin>());
    println!("thermal profile -> fuel {t_fuel:.2} K, shell {t_shell:.2} K");
    assert!(t_fuel > t_shell && t_shell > 500.0, "{t_fuel} {t_shell}");
    assert_eq!(
        m.component_temperature(fuel),
        Some(Temperature::Uniform(taken[0].1))
    );
    let hot = m.assemble().unwrap();
    assert_eq!(cell_temperatures(&hot), vec![Some(t_fuel), Some(t_shell)]);

    // No component in the body: refused.
    let mut bare = Model::new(fake_data());
    bare.set_temperature(kelvins(293.6));
    bare.add(build(None, None));
    match bare.take_temperatures_from(&profile) {
        Err(ModelError::BadValue(w)) => assert!(w.contains("in_component"), "{w}"),
        other => panic!("{other:?}"),
    }
}
