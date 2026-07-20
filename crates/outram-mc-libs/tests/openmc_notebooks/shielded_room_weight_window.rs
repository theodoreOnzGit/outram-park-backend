//! `shielded_room_weight_window` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `shielded_room_weight_window.ipynb (ABSENT at pinned commit)`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Named in the outram-mc directive as a variance-reduction / weight-window shielding case, but this notebook is NOT present in openmc-notebooks at the pinned commit.
//!
//! **OpenMC API exercised.** weight windows / variance reduction (WeightWindows, WeightWindowGenerator) -- inferred from the directive name
//!
//! **GAP.** No weight-window or variance-reduction machinery exists, AND the source notebook is absent upstream, so its exact API surface cannot be confirmed.
//! Tracked by bead op-6tz.21. This test is `#[ignore]`d with an
//! `unimplemented!()` body so it can never report a fake green; removing the
//! ignore before the API exists makes it fail loudly.

/// GAP placeholder for the `shielded_room_weight_window` notebook. See the module docs.
#[test]
#[ignore = "requires weight windows / variance reduction; notebook also absent upstream (op-6tz.21)"]
fn shielded_room_weight_window() {
    unimplemented!("shielded_room_weight_window: requires weight windows / variance reduction; notebook also absent upstream (op-6tz.21)");
}
