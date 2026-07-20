//! `mg-mode-part-ii` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `mg-mode-part-ii.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Generates MGXS from a continuous-energy run (mgxs.Library), inspects them (plot_xs), and re-runs in multigroup mode.
//!
//! **OpenMC API exercised.** mgxs.Library, plot_xs, MeshFilter, multigroup run
//!
//! **GAP (sharpened 2026-07-17).** A multigroup transport mode *does* now exist
//! ([`outram_mc_libs::physics::physics_mg::run_keff_mg`], exercised LIVE by
//! `mg_mode_part_i`), and a `RegularMesh`/`MeshFilter` now exists too (added for
//! `post_processing`). The remaining blocker for THIS notebook is the
//! **generate-from-CE step**: `mgxs.Library` collapses a continuous-energy run
//! into a multigroup library, which is the **njoy track's** responsibility
//! (self-shielded/flux-solved MGXS, bead op-6tz.6.3) and is not available here.
//! Without a way to produce the MGXS from a CE run, the notebook's
//! generate-then-re-run workflow cannot be reproduced. Tracked by op-6tz.15
//! (this crate) + op-6tz.6.3 (njoy). Kept `#[ignore]` with an `unimplemented!()`
//! body so it can never report a fake green.

/// GAP placeholder for the `mg-mode-part-ii` notebook. See the module docs.
#[test]
#[ignore = "MGXS-from-CE generation (mgxs.Library) is the njoy track (op-6tz.6.3); run_keff_mg + MeshFilter exist but the notebook's generate-then-run workflow does not (op-6tz.15)"]
fn mg_mode_generate_and_run() {
    unimplemented!("mg-mode-part-ii: MGXS-from-CE generation (mgxs.Library) unavailable — njoy track op-6tz.6.3 / op-6tz.15");
}
