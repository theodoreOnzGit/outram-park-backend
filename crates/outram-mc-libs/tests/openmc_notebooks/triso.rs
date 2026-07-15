//! `triso` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `triso.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Packs TRISO fuel particles into a pebble/compact (pack_spheres), builds a TRISO lattice, and runs a doubly-heterogeneous k-eff.
//!
//! **OpenMC API exercised.** model.TRISO, model.pack_spheres, model.create_triso_lattice, Universe, Cell, Sphere
//!
//! **GAP.** pebble_beds::stochastic_media (packing) and delta_tracking exist as pieces, but no assembled TRISO-in-lattice k-eff simulation.
//! Tracked by bead op-6tz.16. This test is `#[ignore]`d with an
//! `unimplemented!()` body so it can never report a fake green; removing the
//! ignore before the API exists makes it fail loudly.

/// GAP placeholder for the `triso` notebook. See the module docs.
#[test]
#[ignore = "requires assembled doubly-heterogeneous TRISO k-eff sim (op-6tz.16 -> op-6tz.7)"]
fn triso_doubly_heterogeneous_keff() {
    unimplemented!("triso: requires assembled doubly-heterogeneous TRISO k-eff sim (op-6tz.16 -> op-6tz.7)");
}
