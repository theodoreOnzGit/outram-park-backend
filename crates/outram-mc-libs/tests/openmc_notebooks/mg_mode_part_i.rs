//! `mg-mode-part-i` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `mg-mode-part-i.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Creates a multigroup cross-section library (XSdata / Macroscopic / MGXSLibrary) and runs a multigroup k-eff on a simple lattice.
//!
//! **OpenMC API exercised.** XSdata, Macroscopic, MGXSLibrary, RectLattice, multigroup run
//!
//! **GAP.** physics_mg is a stub: no MGXS data types and no multigroup transport execution.
//! Tracked by bead op-6tz.15. This test is `#[ignore]`d with an
//! `unimplemented!()` body so it can never report a fake green; removing the
//! ignore before the API exists makes it fail loudly.

/// GAP placeholder for the `mg-mode-part-i` notebook. See the module docs.
#[test]
#[ignore = "requires multigroup transport mode + MGXS data types (op-6tz.15)"]
fn mg_mode_run() {
    unimplemented!("mg-mode-part-i: requires multigroup transport mode + MGXS data types (op-6tz.15)");
}
