//! `hexagonal-lattice` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `hexagonal-lattice.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Builds a hexagonal fuel-pin lattice (HexLattice) with a central and outer ring, wraps it in a HexagonalPrism cell, and plots the geometry.
//!
//! **OpenMC API exercised.** HexLattice, Universe, Cell, ZCylinder, model.HexagonalPrism, Plot.from_geometry
//!
//! **GAP.** HexLattice indexing is a stub (no get_indices) and there is no lattice transport or geometry plotting.
//! Tracked by bead op-6tz.11. This test is `#[ignore]`d with an
//! `unimplemented!()` body so it can never report a fake green; removing the
//! ignore before the API exists makes it fail loudly.

/// GAP placeholder for the `hexagonal-lattice` notebook. See the module docs.
#[test]
#[ignore = "requires hex-lattice indexing + CSG navigation (op-6tz.11 -> op-6tz.7)"]
fn hexagonal_lattice_geometry() {
    unimplemented!("hexagonal-lattice: requires hex-lattice indexing + CSG navigation (op-6tz.11 -> op-6tz.7)");
}
