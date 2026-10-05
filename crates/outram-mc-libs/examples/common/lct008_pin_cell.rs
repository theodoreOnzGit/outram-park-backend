// SPDX-License-Identifier: GPL-3.0

//! **LEU-COMP-THERM-008's pin in a reflective square cell**: an infinite
//! lattice of one rod, so its eigenvalue is `k_inf`.
//!
//! Shared by `lct008_pitch_sweep.rs` (the recorded sweep,
//! `verification_and_validation/tutorial_rung4/pitch_sweep.md`) and the
//! `dhoby-ghaut` Monte Carlo web demo's pitch slider (gh:#549), which pulls this
//! file in with `#[path]`, so the browser and the record transport one model.
//! Moved here unchanged from `lct008_pitch_sweep.rs` on 2026-10-05.
//!
//! Material slots are the LCT-008 case-1 order: water 0, fuel 1, clad 2. The
//! radii come from the caller (`lct008_model::R_FUEL`, `R_CLAD`): no gap, as
//! LCT-008's pin has none.

#![allow(dead_code)]

use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::position::Position;
use outram_mc_libs::geometry::surface::{BoundaryType, SurfaceKind, XPlane, YPlane, ZCylinder, ZPlane};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::physics::transport_csg::SourceBox;

/// Half-height of the reflective cell [cm]. With reflective z-planes the value
/// does not change k∞; it only bounds the initial source box.
pub const HALF_Z: f64 = 10.0;

/// The moderator-to-fuel volume ratio of a square pitch `pitch` [cm].
pub fn vm_over_vf(pitch: f64, r_fuel: f64, r_clad: f64) -> f64 {
    (pitch * pitch - std::f64::consts::PI * r_clad * r_clad) / (std::f64::consts::PI * r_fuel * r_fuel)
}

/// The initial-source box: the fuel's bounding box over the cell's height.
pub fn source_box(r_fuel: f64) -> SourceBox {
    SourceBox { lower: Position::new(-r_fuel, -r_fuel, -HALF_Z), upper: Position::new(r_fuel, r_fuel, HALF_Z) }
}

/// The reflective square pin cell of half-pitch `half` [cm], centred on the
/// origin: fuel to `r_fuel`, clad to `r_clad`, water outside, every cell at
/// `temp_k`.
pub fn pin_cell(half: f64, r_fuel: f64, r_clad: f64, temp_k: f64) -> Geometry {
    let hs = |surface_idx: usize, inside: bool| RegionToken::HalfSpace {
        surface_idx,
        sense: if inside { HalfSpaceSense::Inside } else { HalfSpaceSense::Outside },
    };
    let cyl = |r: f64| SurfaceKind::ZCylinder(ZCylinder { x0: 0.0, y0: 0.0, r, bc: BoundaryType::Transmissive });
    let refl = BoundaryType::Reflective;
    let surfaces = vec![
        cyl(r_fuel),                                          // 0
        cyl(r_clad),                                          // 1
        SurfaceKind::XPlane(XPlane { x0: -half, bc: refl }),  // 2
        SurfaceKind::XPlane(XPlane { x0: half, bc: refl }),   // 3
        SurfaceKind::YPlane(YPlane { y0: -half, bc: refl }),  // 4
        SurfaceKind::YPlane(YPlane { y0: half, bc: refl }),   // 5
        SurfaceKind::ZPlane(ZPlane { z0: -HALF_Z, bc: refl }), // 6
        SurfaceKind::ZPlane(ZPlane { z0: HALF_Z, bc: refl }), // 7
    ];
    // Every cell is bounded by the box, so nothing is defined outside it.
    let in_box = |mut r: Vec<RegionToken>| {
        for (s, inside) in [(2, false), (3, true), (4, false), (5, true), (6, false), (7, true)] {
            r.push(hs(s, inside));
            r.push(RegionToken::Intersection);
        }
        r
    };
    let cells = vec![
        Cell::material(1, in_box(vec![hs(0, true)]), 1, temp_k),
        Cell::material(2, in_box(vec![hs(0, false), hs(1, true), RegionToken::Intersection]), 2, temp_k),
        Cell::material(3, in_box(vec![hs(1, false)]), 0, temp_k),
    ];
    Geometry {
        surfaces,
        cells,
        universes: vec![Universe { id: 0, cell_indices: vec![0, 1, 2] }],
        lattices: vec![],
        root_universe: 0,
    }
}
