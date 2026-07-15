//! `flux-spectrum` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `flux-spectrum.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Runs a PWR pin cell and tallies the neutron flux into a fine energy-group structure (CASMO/SHEM), then plots the spectrum.
//!
//! **OpenMC API exercised.** examples.pwr_pin_cell, EnergyFilter, mgxs.GROUP_STRUCTURES, Tally, StatePoint
//!
//! **GAP.** EnergyFilter exists as a data structure but no transport loop scores an energy-binned track-length flux tally.
//! Tracked by bead op-6tz.9. This test is `#[ignore]`d with an
//! `unimplemented!()` body so it can never report a fake green; removing the
//! ignore before the API exists makes it fail loudly.

/// GAP placeholder for the `flux-spectrum` notebook. See the module docs.
#[test]
#[ignore = "requires energy-binned flux tally driven by transport (op-6tz.9 -> op-6tz.8/.7)"]
fn flux_energy_spectrum() {
    unimplemented!("flux-spectrum: requires energy-binned flux tally driven by transport (op-6tz.9 -> op-6tz.8/.7)");
}
