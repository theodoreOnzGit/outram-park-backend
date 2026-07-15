//! `tally-arithmetic` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `tally-arithmetic.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Scores several tallies (energy/cell/mesh-surface filters) then combines them with derived-tally algebra (+ - * /), slicing, and summation.
//!
//! **OpenMC API exercised.** Tally, EnergyFilter, CellFilter, MeshSurfaceFilter, derived-tally arithmetic, get_slice, summation
//!
//! **GAP.** Tally data structures exist but nothing scores them, and there is no derived-tally arithmetic layer.
//! Tracked by bead op-6tz.22. This test is `#[ignore]`d with an
//! `unimplemented!()` body so it can never report a fake green; removing the
//! ignore before the API exists makes it fail loudly.

/// GAP placeholder for the `tally-arithmetic` notebook. See the module docs.
#[test]
#[ignore = "requires transport-driven scoring + tally arithmetic (op-6tz.22 / op-6tz.9)"]
fn tally_arithmetic_derived() {
    unimplemented!("tally-arithmetic: requires transport-driven scoring + tally arithmetic (op-6tz.22 / op-6tz.9)");
}
