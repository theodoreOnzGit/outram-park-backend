//! `cad-based-geometry` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `cad-based-geometry.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Loads a CAD/DAGMC geometry (DAGMCUniverse) and runs transport with mesh tallies over it.
//!
//! **OpenMC API exercised.** DAGMCUniverse, dagmc, RegularMesh, MeshFilter, Model, Plot, StatePoint
//!
//! **GAP.** No DAGMC/CAD geometry backend exists; the crate is pure-Rust CSG only.
//! Tracked by bead op-6tz.17. This test is `#[ignore]`d with an
//! `unimplemented!()` body so it can never report a fake green; removing the
//! ignore before the API exists makes it fail loudly.

/// GAP placeholder for the `cad-based-geometry` notebook. See the module docs.
#[test]
#[ignore = "requires DAGMC/CAD geometry backend (op-6tz.17)"]
fn cad_dagmc_geometry() {
    unimplemented!("cad-based-geometry: requires DAGMC/CAD geometry backend (op-6tz.17)");
}
