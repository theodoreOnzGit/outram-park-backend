//! `candu` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `candu.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Builds a CANDU fuel-bundle cluster (concentric ZCylinder pins in rings) inside a pressure/calandria tube and tallies with a distribcell filter.
//!
//! **OpenMC API exercised.** ZCylinder, Cell, Universe, DistribcellFilter, Tally
//!
//! **GAP.** No cluster CSG navigation and no distribcell filter.
//! Tracked by bead op-6tz.11. This test is `#[ignore]`d with an
//! `unimplemented!()` body so it can never report a fake green; removing the
//! ignore before the API exists makes it fail loudly.

/// GAP placeholder for the `candu` notebook. See the module docs.
#[test]
#[ignore = "requires CSG descent + distribcell tally (op-6tz.11 / op-6tz.7)"]
fn candu_cluster_distribcell() {
    unimplemented!("candu: requires CSG descent + distribcell tally (op-6tz.11 / op-6tz.7)");
}
