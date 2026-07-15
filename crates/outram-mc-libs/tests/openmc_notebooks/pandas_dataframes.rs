//! `pandas-dataframes` notebook -> outram-mc verification (IGNORED -- gap).
//!
//! Notebook: `pandas-dataframes.ipynb`
//! (openmc-notebooks@`cf1e5db2cd77d53a4fa76ffd9af7ab638f468713`, MIT).
//!
//! **What the notebook does.** Exports tally results (with distribcell/mesh/energy filters) to pandas DataFrames for inspection and a Trigger-based convergence check.
//!
//! **OpenMC API exercised.** Tally.get_pandas_dataframe, DistribcellFilter, MeshFilter, EnergyFilter, Trigger
//!
//! **GAP.** No DataFrame export and no distribcell/mesh scoring.
//! Tracked by bead op-6tz.22. This test is `#[ignore]`d with an
//! `unimplemented!()` body so it can never report a fake green; removing the
//! ignore before the API exists makes it fail loudly.

/// GAP placeholder for the `pandas-dataframes` notebook. See the module docs.
#[test]
#[ignore = "requires tally DataFrame export atop scored tallies (op-6tz.22 -> op-6tz.9)"]
fn pandas_dataframe_export() {
    unimplemented!("pandas-dataframes: requires tally DataFrame export atop scored tallies (op-6tz.22 -> op-6tz.9)");
}
