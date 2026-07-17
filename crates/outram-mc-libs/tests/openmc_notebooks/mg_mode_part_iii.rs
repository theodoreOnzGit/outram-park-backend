//! `mg-mode-part-iii` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `mg-mode-part-iii.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Builds a spatially-varying MGXS library over a lattice and runs multigroup transport with mesh tallies.
//!
//! **OpenMC API exercised.** mgxs.Library, RectLattice, MeshFilter, multigroup run
//!
//! **GAP (sharpened 2026-07-17).** As with part-ii, a multigroup transport mode
//! ([`outram_mc_libs::physics::physics_mg::run_keff_mg`]) and a
//! `RegularMesh`/`MeshFilter` now exist. The blockers specific to THIS notebook
//! are (1) **spatially-varying MGXS generation** over a lattice — a per-region
//! `mgxs.Library` collapse from a CE run (njoy track, op-6tz.6.3), and (2) the
//! MG transport driver does not yet consume a lattice of distinct per-cell
//! multigroup materials with mesh tallies. Tracked by op-6tz.15 (this crate) +
//! op-6tz.6.3 (njoy). Kept `#[ignore]` with an `unimplemented!()` body so it can
//! never report a fake green.

/// GAP placeholder for the `mg-mode-part-iii` notebook. See the module docs.
#[test]
#[ignore = "spatially-varying MGXS-from-CE generation over a lattice (njoy op-6tz.6.3) + lattice MG driver not available; run_keff_mg + MeshFilter exist (op-6tz.15)"]
fn mg_mode_spatial() {
    unimplemented!("mg-mode-part-iii: spatial MGXS generation + lattice MG driver unavailable — op-6tz.6.3 / op-6tz.15");
}
