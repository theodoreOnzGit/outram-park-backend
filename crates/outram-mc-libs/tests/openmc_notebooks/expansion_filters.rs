//! `expansion-filters` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `expansion-filters.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Tallies functional-expansion moments of the flux (Legendre in z, Zernike in r-theta) using expansion filters.
//!
//! **OpenMC API exercised.** SpatialLegendreFilter, ZernikeFilter, ZernikeRadialFilter, Legendre, legendre_from_expcoef
//!
//! **GAP.** No functional-expansion tally filters exist.
//! Tracked by bead op-6tz.14. This test is `#[ignore]`d with an
//! `unimplemented!()` body so it can never report a fake green; removing the
//! ignore before the API exists makes it fail loudly.

/// GAP placeholder for the `expansion-filters` notebook. See the module docs.
#[test]
#[ignore = "requires Legendre/Zernike expansion filters (op-6tz.14 -> op-6tz.9)"]
fn expansion_moment_filters() {
    unimplemented!("expansion-filters: requires Legendre/Zernike expansion filters (op-6tz.14 -> op-6tz.9)");
}
