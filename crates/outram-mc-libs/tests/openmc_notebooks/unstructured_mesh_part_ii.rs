//! `unstructured-mesh-part-ii` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `unstructured-mesh-part-ii.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Combines a DAGMC CAD geometry with an unstructured mesh tally and energy filter.
//!
//! **OpenMC API exercised.** UnstructuredMesh, DAGMCUniverse, MeshFilter, EnergyFilter, lib._dagmc_enabled
//!
//! **GAP.** No DAGMC geometry and no unstructured mesh tally.
//! Tracked by bead op-6tz.17. This test is `#[ignore]`d with an
//! `unimplemented!()` body so it can never report a fake green; removing the
//! ignore before the API exists makes it fail loudly.

/// GAP placeholder for the `unstructured-mesh-part-ii` notebook. See the module docs.
#[test]
#[ignore = "requires DAGMC + unstructured-mesh tallies (op-6tz.17)"]
fn unstructured_mesh_dagmc() {
    unimplemented!("unstructured-mesh-part-ii: requires DAGMC + unstructured-mesh tallies (op-6tz.17)");
}
