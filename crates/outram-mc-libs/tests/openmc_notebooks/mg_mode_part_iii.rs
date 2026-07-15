//! `mg-mode-part-iii` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `mg-mode-part-iii.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Builds a spatially-varying MGXS library over a lattice and runs multigroup transport with mesh tallies.
//!
//! **OpenMC API exercised.** mgxs.Library, RectLattice, MeshFilter, multigroup run
//!
//! **GAP.** Multigroup transport is a stub.
//! Tracked by bead op-6tz.15. This test is `#[ignore]`d with an
//! `unimplemented!()` body so it can never report a fake green; removing the
//! ignore before the API exists makes it fail loudly.

/// GAP placeholder for the `mg-mode-part-iii` notebook. See the module docs.
#[test]
#[ignore = "requires multigroup transport mode (op-6tz.15)"]
fn mg_mode_spatial() {
    unimplemented!("mg-mode-part-iii: requires multigroup transport mode (op-6tz.15)");
}
