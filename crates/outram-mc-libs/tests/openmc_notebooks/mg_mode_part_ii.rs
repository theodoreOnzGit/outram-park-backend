//! `mg-mode-part-ii` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `mg-mode-part-ii.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Generates MGXS from a continuous-energy run (mgxs.Library), inspects them (plot_xs), and re-runs in multigroup mode.
//!
//! **OpenMC API exercised.** mgxs.Library, plot_xs, MeshFilter, multigroup run
//!
//! **GAP.** Multigroup transport is a stub; MGXS generation is the njoy track's responsibility.
//! Tracked by bead op-6tz.15. This test is `#[ignore]`d with an
//! `unimplemented!()` body so it can never report a fake green; removing the
//! ignore before the API exists makes it fail loudly.

/// GAP placeholder for the `mg-mode-part-ii` notebook. See the module docs.
#[test]
#[ignore = "requires multigroup transport mode (op-6tz.15)"]
fn mg_mode_generate_and_run() {
    unimplemented!("mg-mode-part-ii: requires multigroup transport mode (op-6tz.15)");
}
