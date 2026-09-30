// SPDX-License-Identifier: GPL-3.0

//! **Code-to-code: nuclide-name handling against upstream's
//! `nuclide_import_accident`** (#449).
//!
//! # Methodology
//!
//! `boon-lay/dev/gen_upstream_run_file_reference.py` runs upstream's
//! `calculation_functions.nuclide_import_accident` (commit `de374c8`) on ten
//! spellings and records the outcome in
//! `boon-lay/tests/data/upstream_run_file/nuclide_names.json`. The outcome is
//! one of three:
//! - **kept**, under its canonical name;
//! - **raised** (`KeyError`: parses, but is not in the table);
//! - **skipped** (does not parse; upstream only logs a warning).
//!
//! Each spelling is then run through `sembawang::accident::release::accident_release`
//! on a one-nuclide inventory. **Pass criterion:**
//! - *kept* → `Ok`, released under the **same canonical name**;
//! - *raised* → `Err`;
//! - *skipped* → `Err`. This is **stricter than upstream, deliberately**: a
//!   nuclide silently missing from a source term is the silent-zero class of
//!   defect (#446).
//!
//! # Results
//!
//! 2026-09-30: all 10 agree.
//! - kept: `Cs-137`, `cs137`, `CS-137`, `cs-137`, `Ag-110m`, `ag110m`;
//! - raised: `Xx-999`, `Cs-999`;
//! - skipped: `???`, `137`.
//!
//! Before #449, the unknown names were reported as "screened out" and
//! `cs137` failed the exact-name inventory lookup.

use sembawang::accident::release::{accident_release, PlantParameters};
use sembawang::inventory::CoreInventory;
use sembawang::scenario::TemperatureTransient;
use serde_json::Value;
use uom::si::f64::{ThermodynamicTemperature, Time};
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::second;

#[test]
fn nuclide_names_are_handled_as_upstream_does() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../boon-lay/tests/data/upstream_run_file/nuclide_names.json");
    let probe: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let transient = TemperatureTransient::from_ramp(
        ThermodynamicTemperature::new::<degree_celsius>(600.0),
        ThermodynamicTemperature::new::<degree_celsius>(1600.0),
        Time::new::<second>(1.0e4),
        Time::new::<second>(1.0e5),
        21,
        1,
        1,
    )
    .unwrap();
    let plant = PlantParameters::np_mhtgr_reference(1e-4, 1e-4, 0.0);
    let mut n = 0;
    for (name, up) in probe.as_object().unwrap() {
        let out = accident_release(&CoreInventory::unit(&[name.as_str()], 1, 1), &transient, &plant);
        match up["outcome"].as_str().unwrap() {
            "kept" => {
                let canonical = up["canonical"][0].as_str().unwrap();
                let out = out.unwrap_or_else(|e| panic!("{name}: upstream kept it as {canonical}; port: {e:?}"));
                assert_eq!(out.cumulative_final[0].0, canonical, "{name}");
            }
            "raised" | "skipped" => assert!(out.is_err(), "{name}: upstream {}, port accepted it", up["outcome"]),
            other => panic!("unknown outcome {other}"),
        }
        n += 1;
    }
    assert_eq!(n, 10);
}
