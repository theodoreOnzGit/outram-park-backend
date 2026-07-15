//! `capi` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `capi.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Drives a simulation through OpenMC's in-memory C-API: init, step batches (next_batch), and read/edit tallies, cells, and materials live.
//!
//! **OpenMC API exercised.** openmc.lib (init, simulation_init, next_batch, tallies, cells, materials, finalize)
//!
//! **GAP.** No in-memory run/introspection interface (batch stepping, live geometry/material edit).
//! Tracked by bead op-6tz.20. This test is `#[ignore]`d with an
//! `unimplemented!()` body so it can never report a fake green; removing the
//! ignore before the API exists makes it fail loudly.

/// GAP placeholder for the `capi` notebook. See the module docs.
#[test]
#[ignore = "requires in-memory run/introspection API (op-6tz.20)"]
fn capi_in_memory_run() {
    unimplemented!("capi: requires in-memory run/introspection API (op-6tz.20)");
}
