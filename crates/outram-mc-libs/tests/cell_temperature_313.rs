// SPDX-License-Identifier: GPL-3.0

//! GitHub #313: a cell's temperature selects the temperature its material is
//! looked up at, as OpenMC (`assign_temperatures`, `src/geometry_aux.cpp:170`;
//! `Particle::sqrtkT` from the cell, `src/geometry.cpp:203`).
//!
//! # Methodology
//!
//! Prediction recorded before running: (1) a geometry whose cells already
//! agree with their materials returns `None`, i.e. nothing changes by a bit;
//! (2) two cells sharing one material at 293.6 K, one declared at 1200 K, give
//! one appended material copy at 1200 K, with the other cell untouched;
//! (3) on an embedded multipole (`Core`) U-235, whose cross section depends on
//! temperature, the bound material's `macro_xs` equals a material built at
//! 1200 K bit for bit and differs from the 293.6 K one at some energy of a
//! resonance scan; (4) an invalid cell temperature is refused.
//!
//! Results: see the test run recorded on gh:#313.

use outram_mc_libs::geometry::cell::{Cell, CellFill, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::material::cell_temperature::{
    bind_cell_temperatures, cell_temperature_mismatches,
};
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;

fn sphere(r: f64) -> SurfaceKind {
    SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r, bc: BoundaryType::Vacuum })
}

fn inside(s: usize) -> Vec<RegionToken> {
    vec![RegionToken::HalfSpace { surface_idx: s, sense: HalfSpaceSense::Inside }]
}

fn material(t: f64) -> Material {
    Material {
        id: 1,
        name: "m".into(),
        components: vec![NuclideComponent { nuclide_idx: 0, atom_density: 0.02 }],
        temperature: t,
    }
}

/// Two nested-by-hand cells (regions need not partition for this test: only
/// the fill and temperature fields are read).
fn geometry(t_a: f64, t_b: f64) -> Geometry {
    Geometry {
        surfaces: vec![sphere(1.0), sphere(2.0)],
        cells: vec![
            Cell::material(1, inside(0), 0, t_a),
            Cell::material(2, inside(1), 0, t_b),
        ],
        universes: vec![Universe { id: 0, cell_indices: vec![0, 1] }],
        lattices: vec![],
        root_universe: 0,
    }
}

#[test]
fn agreeing_cells_and_materials_change_nothing() {
    let g = geometry(293.6, 293.6);
    let m = vec![material(293.6)];
    assert!(cell_temperature_mismatches(&g, &m).is_empty());
    assert!(bind_cell_temperatures(&g, &m).unwrap().is_none());
}

#[test]
fn a_hot_cell_gets_one_hot_copy_of_the_shared_material() {
    let g = geometry(293.6, 1200.0);
    let m = vec![material(293.6)];
    assert_eq!(cell_temperature_mismatches(&g, &m).len(), 1);
    let (g2, m2) = bind_cell_temperatures(&g, &m).unwrap().expect("one cell disagrees");
    assert_eq!(m2.len(), 2, "originals kept, one copy appended");
    assert_eq!(m2[0].temperature, 293.6);
    assert_eq!(m2[1].temperature, 1200.0);
    assert!(matches!(g2.cells[0].fill, CellFill::Material(0)));
    assert!(matches!(g2.cells[1].fill, CellFill::Material(1)));
    // Two cells at the same hot temperature share the one copy.
    let g3 = geometry(1200.0, 1200.0);
    let (g4, m4) = bind_cell_temperatures(&g3, &m).unwrap().unwrap();
    assert_eq!(m4.len(), 2);
    assert!(matches!(g4.cells[0].fill, CellFill::Material(1)));
    assert!(matches!(g4.cells[1].fill, CellFill::Material(1)));
}

#[test]
fn invalid_cell_temperatures_are_refused() {
    let m = vec![material(293.6)];
    for bad in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(bind_cell_temperatures(&geometry(293.6, bad), &m).is_err(), "{bad}");
    }
}

/// Transport reads the cell temperature through the material: on a multipole
/// nuclide the bound cell's cross section is the 1200 K material's, bit for
/// bit, and it is not the 293.6 K one.
#[test]
fn the_bound_cell_is_looked_up_at_its_own_temperature() {
    let nucs = vec![Nuclide::from_core("U235").expect("embedded U-235")];
    let g = geometry(293.6, 1200.0);
    let m = vec![material(293.6)];
    let (g2, m2) = bind_cell_temperatures(&g, &m).unwrap().unwrap();
    let CellFill::Material(hot) = g2.cells[1].fill else { panic!("material cell") };
    let reference_hot = material(1200.0);
    let mut any_differs = false;
    for i in 0..400 {
        let e = 1.0 + 0.05 * f64::from(i); // 1 to 21 eV, across the 6.67 and 20.9 eV resonances
        let bound = m2[hot].macro_xs(e, &nucs).total;
        assert_eq!(bound.to_bits(), reference_hot.macro_xs(e, &nucs).total.to_bits());
        if bound.to_bits() != m[0].macro_xs(e, &nucs).total.to_bits() {
            any_differs = true;
        }
    }
    assert!(any_differs, "1200 K and 293.6 K gave the same cross section: this harness cannot see temperature");
}
