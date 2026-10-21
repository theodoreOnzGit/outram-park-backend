// SPDX-License-Identifier: GPL-3.0

//! **Code-to-code: reading upstream-format TRISO-ATOPS run files (#449).**
//!
//! # Methodology
//!
//! `dev/gen_upstream_run_file_reference.py` writes seven run files in
//! upstream's own format, with their CSVs, under
//! `tests/data/upstream_run_file/<case>/`. It runs **upstream's**
//! `run_functions.process_run_file` (commit `de374c8`) on each one, from that
//! directory, and records what upstream parsed in `expected.json`.
//!
//! The cases cover every input form `process_run_file` accepts:
//!
//! | Case | What it exercises |
//! |---|---|
//! | `csv_all` | every table as a CSV path, `read_profile` with header and index, `Times` as a path in hours, one accident CSV per ring |
//! | `inline` | inline arrays, headerless profiles, `Times` through `read_profile` in minutes, HPS off (so `k_clean` is not read) |
//! | `normal_only` | accident off (the accident constants are not read) |
//! | `wrong_unit`, `bad_time_unit` | upstream counts an error |
//! | `fraction_sum_over_one`, `fraction_sum_exactly_one` | the `check_run_file` fraction-sum checks (a sum of exactly 1 counts as an error in accident mode) |
//!
//! This test reads each case with `read_upstream_run_file`.
//!
//! **Pass criterion:**
//! - where upstream counted an error or raised, the port returns `Err`;
//! - otherwise the port returns `Ok`, and every constant (SI, times converted),
//!   toggle, node count, nuclide, inventory, profile, time and accident
//!   temperature equals upstream's **exactly** (the same decimal inputs, and
//!   the same single multiplication for the time units).
//!
//! # Results
//!
//! 2026-09-30: all 7 cases agree. There are 3 parsed cases (every value
//! bit-identical) and 4 rejected cases (`Err`, where upstream's `error_count`
//! is > 0). Upstream's `error_count` values (8 for a wrong unit) include its
//! `check_run_file` re-adding the running count, so only **whether** an error
//! occurred is compared, not how many.

use boon_lay::triso_atops_fork::run_file::upstream::read_upstream_run_file;
use serde_json::Value;
use std::path::PathBuf;

const CASES: [&str; 7] = [
    "csv_all",
    "inline",
    "normal_only",
    "wrong_unit",
    "bad_time_unit",
    "fraction_sum_over_one",
    "fraction_sum_exactly_one",
];

fn dir(case: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/upstream_run_file").join(case)
}

fn flat(v: &Value) -> Vec<f64> {
    match v {
        Value::Array(a) => a.iter().flat_map(flat).collect(),
        Value::Number(n) => vec![n.as_f64().unwrap()],
        _ => vec![],
    }
}

#[test]
fn every_case_agrees_with_upstream_process_run_file() {
    for case in CASES {
        let expected: Value =
            serde_json::from_str(&std::fs::read_to_string(dir(case).join("expected.json")).unwrap())
                .unwrap();
        let got = read_upstream_run_file(&dir(case).join("run.json"));
        let upstream_failed = !expected["raised"].is_null()
            || expected["error_count"].as_u64().is_some_and(|n| n > 0);
        if upstream_failed {
            assert!(got.is_err(), "{case}: upstream rejected it, the port accepted it: {got:?}");
            println!("{case}: rejected by both ({:?})", got.unwrap_err());
            continue;
        }
        let run = got.unwrap_or_else(|e| panic!("{case}: upstream accepted it, the port rejected it: {e:?}"));
        let up_const = flat(&expected["constants"]);
        assert_eq!(&run.constants[..up_const.len()], up_const.as_slice(), "{case}: constants");
        assert!(run.constants[up_const.len()..].iter().all(|x| *x == 0.0), "{case}: unread slots");
        assert_eq!(
            expected["options"],
            serde_json::json!([run.hps, run.accident]),
            "{case}: toggles"
        );
        assert_eq!(
            flat(&expected["react_config"]),
            vec![run.n_radial as f64, run.n_axial as f64],
            "{case}: node counts"
        );
        let nuc: Vec<String> =
            expected["nuclides"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        assert_eq!(run.nuclides, nuc, "{case}: nuclides");
        assert_eq!(flat(&expected["inventories"]), run.inventories.concat(), "{case}: inventories");
        assert_eq!(flat(&expected["core"]), run.core_temperatures.concat(), "{case}: core");
        assert_eq!(flat(&expected["graphite"]), run.graphite_temperatures.concat(), "{case}: graphite");
        assert_eq!(flat(&expected["times"]), run.times.clone().unwrap_or_default(), "{case}: times");
        let acc: Vec<f64> = run
            .accident_temperatures
            .clone()
            .map(|a| a.into_iter().flatten().flatten().collect())
            .unwrap_or_default();
        assert_eq!(flat(&expected["accident_temps"]), acc, "{case}: accident temperatures");
        println!("{case}: identical to upstream");
    }
}
