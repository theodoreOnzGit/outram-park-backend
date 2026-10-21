//! The model of the `lumped` rung: a Wigner-Seitz cell of rung 3, a sphere of
//! natural uranium metal at the centre of a graphite sphere, white outer
//! boundary.
//!
//! **Nothing is retyped here.** The compositions (natural uranium metal at
//! 19.05 g/cm³, graphite at 1.73 g/cm³), the cell-average ratio
//! `N_C/N_U` = [`C_PER_U`] and the cell radius come from outram-mc-libs'
//! [`outram_mc_libs::vv::ugraphite`], and the cell is built with the same
//! constructor and arguments as `ws_cell` in
//! `examples/lumped_ugraphite_kinf.rs` (`fhr_pebble_geometry`: a uranium
//! ball, two graphite shells, white outer boundary). The tapes and their
//! processing are the `ugraphite` rung's ([`crate::ugraphite::model`]).

use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::surface::BoundaryType;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::pebble_beds::fhr_pebble::fhr_pebble_geometry;
use outram_mc_libs::vv::ugraphite as vv;

pub use crate::ugraphite::model::TEMPERATURE_K;

/// Cell-average carbon atoms per uranium atom: rung 3's fixed ratio (the
/// example's default `CU`).
pub const C_PER_U: f64 = 600.0;

/// The lump radius shown, cm: the scan's radius with the highest measured
/// `k_inf` at [`C_PER_U`] (0.95924 +/- 0.00211 at r = 0.3 cm,
/// `examples/lumped_ugraphite_kinf.rs`, results of 2026-10-05). A display
/// choice made after the scan: nothing is compared at it.
pub const LUMP_R_CM: f64 = 0.3;

/// The cell's outer radius, cm, for [`LUMP_R_CM`] at [`C_PER_U`].
pub fn cell_r() -> f64 {
    vv::cell_radius(LUMP_R_CM, C_PER_U)
}

/// Material 0 the lump, material 1 the graphite.
pub fn materials() -> Vec<Material> {
    vec![vv::uranium_metal().material(1, "natural U metal"), vv::graphite().material(2, "graphite")]
}

/// The cell, as `ws_cell(r, R)` in the example: lump (material 0) to `r`,
/// graphite (material 1) to `R` in two shells, white boundary at `R`.
pub fn build_geometry() -> Geometry {
    let (r, big_r) = (LUMP_R_CM, cell_r());
    fhr_pebble_geometry(0.0, r, 0.5 * (r + big_r), big_r, 0, 1, 1, BoundaryType::White, TEMPERATURE_K)
}
