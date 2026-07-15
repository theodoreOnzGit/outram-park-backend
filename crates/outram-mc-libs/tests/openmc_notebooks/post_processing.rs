//! `post-processing` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `post-processing.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Reads a StatePoint, extracts a RegularMesh flux tally, reshapes it, and plots spatial distributions.
//!
//! **OpenMC API exercised.** StatePoint, RegularMesh, MeshFilter, Tally, mesh reshaping
//!
//! **GAP.** No StatePoint persistence and no scored mesh tally to post-process.
//! Tracked by bead op-6tz.22. This test is `#[ignore]`d with an
//! `unimplemented!()` body so it can never report a fake green; removing the
//! ignore before the API exists makes it fail loudly.

/// GAP placeholder for the `post-processing` notebook. See the module docs.
#[test]
#[ignore = "requires StatePoint + mesh tally scoring (op-6tz.22 / op-6tz.13)"]
fn post_processing_statepoint() {
    unimplemented!("post-processing: requires StatePoint + mesh tally scoring (op-6tz.22 / op-6tz.13)");
}
