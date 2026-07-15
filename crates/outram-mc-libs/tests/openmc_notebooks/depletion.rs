//! `depletion` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `depletion.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Depletes a fuel pin over burnup steps with a coupled transport/transmutation operator and inspects nuclide inventories.
//!
//! **OpenMC API exercised.** deplete.CoupledOperator, deplete.PredictorIntegrator, deplete.Results, deplete.Chain, model.pin
//!
//! **GAP.** Depletion is explicitly out of scope in outram-mc (src/chain.cpp / transmutation matrix not ported); may land in a future crate.
//! Tracked by bead op-6tz.18. This test is `#[ignore]`d with an
//! `unimplemented!()` body so it can never report a fake green; removing the
//! ignore before the API exists makes it fail loudly.

/// GAP placeholder for the `depletion` notebook. See the module docs.
#[test]
#[ignore = "requires depletion / transmutation driver (op-6tz.18)"]
fn depletion_burnup() {
    unimplemented!("depletion: requires depletion / transmutation driver (op-6tz.18)");
}
