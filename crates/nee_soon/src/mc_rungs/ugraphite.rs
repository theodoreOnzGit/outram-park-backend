//! **The model of the `ugraphite` rung** (and the data of the `lumped`
//! rung): uranium and graphite at 296 K, ENDF/B-VIII.0 (gh:#1742; moved
//! 2026-11-25 from `dhoby-ghaut/examples/monte_carlo_web/ugraphite/model.rs`;
//! its data builder stays there).
//!
//! **Nothing is retyped here.** The tapes, the temperature and the
//! compositions are outram-mc-libs' [`outram_mc_libs::vv::ugraphite`], the
//! module `examples/ugraphite_four_factor.rs` (rung 2) and
//! `examples/lumped_ugraphite_kinf.rs` (rung 3) run, so the demo moves the
//! same atoms the recorded results were measured on.
//!
//! - **Composition shown** ([`mixture`]): rung 2's main case, the uranium and
//!   carbon of one HTR-10 fuel pebble smeared over the ball (17 wt% U-235,
//!   `N_C/N_U` = 767.2), whose recorded `k_inf` is 1.56777 ± 0.00081.
//! - **Geometry** ([`build_geometry`]): the same reflective cube
//!   (`homogeneous_cube`, half-width [`HALF_CM`]) the example uses, so a
//!   neutron reaching a wall comes back: an infinite medium.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::material::material::Material;
use outram_mc_libs::pebble_beds::fhr_pebble::homogeneous_cube;
use outram_mc_libs::vv::ugraphite as vv;

pub use vv::TEMPERATURE_K;

/// Half-width of the reflective cube, cm: `examples/ugraphite_four_factor.rs`'s
/// `HALF_CM`. Irrelevant to `k_inf`; it only sets how often a track meets a wall.
pub const HALF_CM: f64 = 50.0;

/// The tapes, `(label shown while loading, file)`: the five nuclides in
/// [`vv::TAPES`] order (the order [`vv::Mix::material`] indexes), then the
/// crystalline-graphite S(α,β) law.
pub const JOBS: [(&str, &str); 6] = [
    ("U-234", vv::TAPES[0].1),
    ("U-235", vv::TAPES[1].1),
    ("U-238", vv::TAPES[2].1),
    ("C-12", vv::TAPES[3].1),
    ("C-13", vv::TAPES[4].1),
    ("graphite S(α,β)", vv::GRAPHITE_TSL.0),
];

/// Index of U-235 in the nuclide list (for the Watch mode's birth spectrum).
pub const N_U235: usize = vv::Role::U235.tape_index();
/// Index of U-238 (the σ(E) panel, gh:#549). The uranium slots do not move
/// when the graphite law is bound; the carbon's do (`vv::RoleIndex`).
pub const N_U238: usize = vv::Role::U238.tape_index();

/// Rung 2's main case: one HTR-10 fuel pebble's uranium and carbon.
pub fn mixture() -> vv::Mix {
    vv::htr10_pebble_mix().0
}

/// The one material, at index 0.
pub fn materials() -> Vec<Material> {
    vec![mixture().material(1, "U(17 wt%) + graphite, HTR-10 pebble ratio")]
}

/// The reflective cube filled with material 0.
pub fn build_geometry() -> Geometry {
    homogeneous_cube(HALF_CM, 0, TEMPERATURE_K)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One material in one reflective cube, six jobs.
    #[test]
    fn the_cube_holds_the_pebble_mixture() {
        assert_eq!(materials().len(), 1);
        assert!(!build_geometry().cells.is_empty());
        assert_eq!(JOBS.len(), 6);
        assert!(N_U235 < N_U238 || N_U238 < N_U235);
    }
}
