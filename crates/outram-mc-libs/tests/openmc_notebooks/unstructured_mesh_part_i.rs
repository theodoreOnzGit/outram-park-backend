//! `unstructured-mesh-part-i` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `unstructured-mesh-part-i.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Tallies onto an unstructured (libMesh/MOAB) mesh overlaid on a PWR assembly.
//!
//! **OpenMC API exercised.** UnstructuredMesh, MeshFilter, lib._libmesh_enabled, examples.pwr_assembly
//!
//! **GAP.** No UnstructuredMesh type and no mesh tally scoring.
//! Tracked by bead op-6tz.17. This test is `#[ignore]`d with an
//! `unimplemented!()` body so it can never report a fake green; removing the
//! ignore before the API exists makes it fail loudly.

/// GAP placeholder for the `unstructured-mesh-part-i` notebook. See the module docs.
#[test]
#[ignore = "requires unstructured-mesh tallies (op-6tz.17)"]
fn unstructured_mesh_tally() {
    unimplemented!("unstructured-mesh-part-i: requires unstructured-mesh tallies (op-6tz.17)");
}
