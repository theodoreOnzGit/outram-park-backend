//! `tally-power-normalization` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `tally-power-normalization.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Runs a k-eff, tallies fission energy deposition (kappa-fission), and normalizes the flux/reaction rates to a target reactor power.
//!
//! **OpenMC API exercised.** Tally (heating / kappa-fission), CellFilter, StatePoint, power normalization
//!
//! **GAP.** No heating/kappa-fission tally and no StatePoint to normalize from.
//! Tracked by bead op-6tz.22. This test is `#[ignore]`d with an
//! `unimplemented!()` body so it can never report a fake green; removing the
//! ignore before the API exists makes it fail loudly.

/// GAP placeholder for the `tally-power-normalization` notebook. See the module docs.
#[test]
#[ignore = "requires kappa-fission tally + power normalization (op-6tz.22 -> op-6tz.9)"]
fn tally_power_normalization() {
    unimplemented!("tally-power-normalization: requires kappa-fission tally + power normalization (op-6tz.22 -> op-6tz.9)");
}
