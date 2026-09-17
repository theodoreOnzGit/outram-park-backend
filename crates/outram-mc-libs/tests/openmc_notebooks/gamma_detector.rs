//! `gamma-detector` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `gamma-detector.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Builds a gamma detector, sources decay photons (decay_photon_energy) from an activated material, and tallies the detector pulse-height/energy spectrum.
//!
//! **OpenMC API exercised.** data.decay_photon_energy, Source, stats.Discrete/Isotropic, photon transport, EnergyFilter
//!
//! **GAP.** The crate is neutron-only: no photon transport and no decay-photon source.
//! Tracked by bead op-6tz.19. This test is `#[ignore]`d with an
//! `unimplemented!()` body so it can never report a fake green; removing the
//! ignore before the API exists makes it fail loudly.

/// GAP placeholder for the `gamma-detector` notebook. See the module docs.
#[test]
#[ignore = "requires photon transport + decay-photon source (op-6tz.19)"]
fn gamma_detector_photon() {
    unimplemented!("gamma-detector: requires photon transport + decay-photon source (op-6tz.19)");
}
