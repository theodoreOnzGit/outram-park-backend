// SPDX-License-Identifier: GPL-3.0

//! **Cell temperature is the transport DEFAULT, and this test keeps it so**
//! (GitHub #313, 2026-11-14; the `correct_physics_is_default` pattern).
//!
//! # Methodology
//!
//! OpenMC resolves temperature at the cell (`Cell::sqrtkT_`,
//! `assign_temperatures`, `src/geometry_aux.cpp:170-198`): a cell with a
//! `temperature` overrides its material's, a cell without takes the
//! material's. Here `Cell::temperature` is `Option<f64>` and the `run_keff_csg*`
//! drivers apply it with no flag. A bare Godiva-composition HEU sphere
//! ... **A first version used bare Godiva and its 1200 K control was blind**
//! (three arms bit-identical at 1200 K and 293.6 K: a fast sphere has no
//! collision where the multipole temperature matters, the lesson of
//! `temperature_precedence.rs`), so the model is a lightly moderated one: the
//! embedded multipole `Core` nuclides U-235 (4.5e-3) and H-1 (3.0e-2
//! atoms/(b cm)) in a vacuum-bounded sphere of r = 12 cm, whose cross sections
//! depend on the material temperature. It is run through the ordinary entry
//! points; 300 histories x [2 inactive + 3 active], fixed seed.
//!
//! Predictions fixed before running (all about bits, so no statistics):
//!
//! 1. cell `Some(1200)` on a 293.6 K material  ==  cell `None` on a 1200 K
//!    material, bit for bit, on the sequential driver, the parallel driver and
//!    one `CsgPowerIteration::step` (the cell temperature really reaches the
//!    lookup, by every driver);
//! 2. cell `None` on a 293.6 K material  ==  the first geometry after
//!    `without_cell_temperatures()` (the ablation is an explicit act that
//!    restores the material's temperature);
//! 3. 1. and 2. differ in k (the harness can see 1200 K against 293.6 K).
//!
//! # Results (2026-11-14, release)
//!
//! See the assertions' printed k values and the note on gh:#313.

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::physics::compute::ThreadCount;
use outram_mc_libs::physics::keff::KeffSettings;
use outram_mc_libs::physics::transport_csg::{
    run_keff_csg_par, run_keff_csg_seq, CsgPowerIteration, SourceBox,
};

const R: f64 = 12.0;

fn nuclides() -> Vec<Nuclide> {
    vec![Nuclide::from_core("U235").unwrap(), Nuclide::from_core("H1").unwrap()]
}

fn heu(t: f64) -> Material {
    Material {
        id: 1,
        name: "U235 + H1".into(),
        temperature: t,
        components: vec![
            NuclideComponent { nuclide_idx: 0, atom_density: 4.5e-3 },
            NuclideComponent { nuclide_idx: 1, atom_density: 3.0e-2 },
        ],
    }
}

fn sphere(cell_t: Option<f64>) -> Geometry {
    Geometry {
        surfaces: vec![SurfaceKind::Sphere(Sphere {
            x0: 0.0,
            y0: 0.0,
            z0: 0.0,
            r: R,
            bc: BoundaryType::Vacuum,
        })],
        cells: vec![Cell::material(
            1,
            vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }],
            0,
            cell_t,
        )],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    }
}

fn settings() -> KeffSettings {
    KeffSettings { n_particles: 300, n_inactive: 2, n_active: 3, seed: 7, ..Default::default() }
}

fn source_box() -> SourceBox {
    let h = R / 3f64.sqrt();
    SourceBox { lower: Position::new(-h, -h, -h), upper: Position::new(h, h, h) }
}

fn k_seq(g: &Geometry, m: &Material) -> f64 {
    run_keff_csg_seq(
        g,
        std::slice::from_ref(m),
        &nuclides(),
        &[],
        None,
        source_box(),
        &settings(),
        None,
        &[],
        None,
    )
    .k_mean
}

fn k_par(g: &Geometry, m: &Material) -> f64 {
    run_keff_csg_par(
        g,
        std::slice::from_ref(m),
        &nuclides(),
        &[],
        None,
        source_box(),
        &settings(),
        None,
        &[],
        None,
        ThreadCount::Fixed(2),
    )
    .k_mean
}

fn k_step(g: &Geometry, m: &Material) -> f64 {
    let nucs = nuclides();
    let mats = std::slice::from_ref(m);
    let mut it = CsgPowerIteration::new(g, mats, &nucs, source_box(), &settings());
    it.step(g, mats, &nucs).expect("one generation").k
}

#[test]
fn constructors_default_a_cell_to_unset() {
    let fill = Cell::fill(1, vec![], CellFill::Universe(0), Position::ZERO);
    assert_eq!(fill.temperature, None);
    assert_eq!(Cell::material(1, vec![], 0, None).temperature, None);
    assert_eq!(Cell::material(1, vec![], 0, 600.0).temperature, Some(600.0));
}

#[test]
fn a_declared_cell_temperature_reaches_every_k_driver_by_default() {
    let hot_cell = sphere(Some(1200.0));
    let hot_material = sphere(None);
    let cold = heu(293.6);
    let hot = heu(1200.0);

    let (a_seq, b_seq) = (k_seq(&hot_cell, &cold), k_seq(&hot_material, &hot));
    let (a_par, b_par) = (k_par(&hot_cell, &cold), k_par(&hot_material, &hot));
    let (a_step, b_step) = (k_step(&hot_cell, &cold), k_step(&hot_material, &hot));
    println!("seq {a_seq:.6} vs {b_seq:.6}; par {a_par:.6} vs {b_par:.6}; step {a_step:.6} vs {b_step:.6}");
    assert_eq!(a_seq.to_bits(), b_seq.to_bits(), "sequential driver ignored the cell temperature");
    assert_eq!(a_par.to_bits(), b_par.to_bits(), "parallel driver ignored the cell temperature");
    assert_eq!(a_step.to_bits(), b_step.to_bits(), "CsgPowerIteration::step ignored the cell temperature");

    // The ablation is explicit, and restores the material's temperature.
    let ablated = hot_cell.without_cell_temperatures();
    assert!(ablated.cells.iter().all(|c| c.temperature.is_none()));
    let unset_seq = k_seq(&sphere(None), &cold);
    assert_eq!(k_seq(&ablated, &cold).to_bits(), unset_seq.to_bits());
    assert_eq!(k_par(&ablated, &cold).to_bits(), k_par(&sphere(None), &cold).to_bits());

    // The harness can see 1200 K against 293.6 K.
    assert_ne!(a_seq.to_bits(), unset_seq.to_bits(), "1200 K and 293.6 K gave one k");
}

#[test]
fn a_cell_that_agrees_with_its_material_changes_nothing() {
    let m = heu(293.6);
    assert_eq!(
        k_seq(&sphere(Some(293.6)), &m).to_bits(),
        k_seq(&sphere(None), &m).to_bits()
    );
}
