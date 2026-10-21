// SPDX-License-Identifier: GPL-3.0

//! **The ACE route reads MT=5's energy-dependent multiplicity** — GitHub #365
//! audit.
//!
//! # What was wrong
//!
//! An ACE reaction with `|TY| > 100` keeps its neutron yield `y(E)` in a TAB1
//! inside DLW (`openmc/data/reaction.py:1059-1062`). `Nuclide::from_ace` never
//! read it, so every ACE branch had an empty yield and `Nuclide::mt5_yield`
//! returned **1** at all energies, while the ENDF route used the MF=6 yield.
//! Every held table with MT=5 writes `TY = -101`. U-235's (n,anything) actually
//! emits ~2 neutrons at 10-12 MeV and U-238's ~0.07-0.12, so the error ran in
//! both directions.
//!
//! # Methodology
//!
//! Reference: OpenMC's reader of the same NJOY2016 tables
//! (`verification_and_validation/ace_route_physics/openmc_inputs/mt5_yield_reference.py`,
//! OpenMC 0.16.1.dev25, into `data/mt5_yield_openmc.csv`), at 6, 10, 12, 14,
//! 17 and 20 MeV on every table carrying MT=5. `mt5_yield(E)` must equal
//! OpenMC's `y(E)` to 1e-12 (absolute, the values are O(1)): both are lin-lin
//! interpolation of the same TAB1. Tables are the five-route study's
//! `target/five_route_keff/njoy/293.6K/`, skipped when absent.
//!
//! # Results (2026-09-29)
//!
//! 48 rows (8 nuclides x 6 energies) equal; e.g. U-235 y(10 MeV) = 1.98752 and
//! U-238 y(12 MeV) = 0.06682, both previously 1. Without the change the test
//! fails on the first U-235 row (checked by reverting the wiring).

use outram_mc_libs::material::nuclide::Nuclide;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[test]
fn mt5_yield_from_ace_matches_openmc() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let csv = root.join("verification_and_validation/ace_route_physics/data/mt5_yield_openmc.csv");
    let dir = root.join("../../target/five_route_keff/njoy/293.6K");
    if !dir.is_dir() {
        println!("{} absent: skipping", dir.display());
        return;
    }
    let text = std::fs::read_to_string(csv).expect("reference csv");
    let mut cache: BTreeMap<String, Nuclide> = BTreeMap::new();
    let mut n = 0;
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("nuclide"))
    {
        let f: Vec<&str> = line.split(',').collect();
        let (name, e, want): (&str, f64, f64) =
            (f[0], f[1].parse().unwrap(), f[2].parse().unwrap());
        let nuc = cache.entry(name.to_string()).or_insert_with(|| {
            Nuclide::from_ace_file(dir.join(format!("{name}.ace")), name).expect("from_ace_file")
        });
        let got = nuc.mt5_yield(e);
        assert!(
            (got - want).abs() < 1e-12,
            "{name} E = {e}: mt5_yield {got} vs OpenMC {want}"
        );
        n += 1;
    }
    println!("{n} MT=5 yields equal to OpenMC");
    assert!(n >= 30);
}
