//! **The Godiva rung's model**: ICSBEP HEU-MET-FAST-001, a bare sphere of
//! highly enriched uranium metal, r = 8.7407 cm (gh:#1742; moved 2026-11-25
//! from `dhoby-ghaut/examples/monte_carlo_web/godiva/model.rs` and the
//! recorded result in `godiva/mod.rs`).
//!
//! **Nothing is retyped here.** Radius, temperature, the three nuclides,
//! their tapes and atom densities all come from
//! [`outram_mc_libs::vv::godiva`], the model
//! `outram-mc-libs/examples/godiva_keff_endf_local.rs` runs, i.e. the model
//! the recorded result was measured on.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use super::RecordedRun;
use outram_mc_libs::geometry::cell::{Cell, HalfSpaceSense, RegionToken};
use outram_mc_libs::geometry::geometry::Geometry;
use outram_mc_libs::geometry::surface::{BoundaryType, Sphere, SurfaceKind};
use outram_mc_libs::geometry::universe::Universe;
use outram_mc_libs::vv::godiva;

pub use godiva::{RADIUS_CM, TEMPERATURE_K};

/// The three tapes, `(label shown while loading, file)`, in
/// [`godiva::NUCLIDES`] order.
pub const JOBS: [(&str, &str); 3] = [
    ("U-234", godiva::NUCLIDES[0].1),
    ("U-235", godiva::NUCLIDES[1].1),
    ("U-238", godiva::NUCLIDES[2].1),
];

/// Index of U-235 in the nuclide list (for the Watch mode's birth spectrum).
pub const N_U235: usize = 1;

/// The recorded result for this model and data, to compare a reader's run
/// with: **0.99994 ± 0.00005 (−6 ± 5 pcm)**, 1024 seeds × 5000 histories ×
/// [40 inactive + 120 active], seed-to-seed sd 165 pcm,
/// `crates/outram-mc-libs/examples/godiva_keff_ensemble.rs` "Results
/// (2026-10-05)", commit `6faff1ed8` (#546), the number the Godiva lesson
/// quotes. Quoted, not re-measured here.
pub const RECORDED: RecordedRun = RecordedRun {
    label: "1024 seeds x 5000 x [40+120], 2026-10-05, godiva_keff_ensemble.rs at 6faff1ed8",
    k: 0.99994,
    sem: 0.00005,
    sd_one_run: 0.00165,
    histories_one_run: 5000.0 * 120.0,
};

/// The sphere as a CSG geometry, for the Watch mode's traced histories
/// (`run_fixed_source_traced` needs a `Geometry`; the Run mode's
/// `PowerIteration` builds its own sphere from the same radius). One sphere,
/// vacuum outside, one cell of material 0 at [`TEMPERATURE_K`].
pub fn build_geometry() -> Geometry {
    let surfaces = vec![SurfaceKind::Sphere(Sphere { x0: 0.0, y0: 0.0, z0: 0.0, r: RADIUS_CM, bc: BoundaryType::Vacuum })];
    let inside = vec![RegionToken::HalfSpace { surface_idx: 0, sense: HalfSpaceSense::Inside }];
    Geometry {
        surfaces,
        cells: vec![Cell::material(1, inside, 0, TEMPERATURE_K)],
        universes: vec![Universe { id: 0, cell_indices: vec![0] }],
        lattices: vec![],
        root_universe: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One sphere of the benchmark radius, one material cell.
    #[test]
    fn the_sphere_is_the_benchmark_sphere() {
        let g = build_geometry();
        assert_eq!((g.surfaces.len(), g.cells.len()), (1, 1));
        assert_eq!(RADIUS_CM, 8.7407);
        assert_eq!(JOBS[1].0, "U-235");
        assert_eq!(RECORDED.k, 0.99994);
    }
}
