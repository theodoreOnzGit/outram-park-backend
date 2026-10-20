// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! Code-to-code verification of `petir::interp::Interp::eval_integ` against
//! `gsl_interp_eval_integ` compiled from GSL 2.8 (commit `cf180cd7`).
//! Added 2026-10-20 for gh:#989, where `tuas_boussinesq_solver` needs the
//! integral of a cubic-spline specific-heat table to compute enthalpy.
//!
//! # Methodology
//!
//! `reference-data/gsl/interp_integ_reference_driver.c` builds GSL's
//! `interpolation/{interp,linear,cspline,accel}.c` with its tridiagonal solver
//! (`linalg/tridiag.c`) from the vendored tree (recipe in its header and in
//! `reference-data/gsl/README.md`) and prints `gsl_interp_eval_integ` at 17
//! significant digits for `gsl_interp_linear` and `gsl_interp_cspline` on two
//! tables:
//!
//! - **runge**: the 9-knot Runge table the `eval`/`eval_deriv` comparison in
//!   `gsl_numerics_code_to_code.rs` already uses, integrated over all 231
//!   ordered pairs `a <= b` of a 21-point grid on `[-1, 1]`. That reaches
//!   `a == b`, limits on knots and between them, a single segment (where
//!   `index_a == index_b`) and spans of every length.
//! - **thermo**: a non-uniform 11-knot table on `[200, 1200]` shaped like the
//!   specific-heat tables `tuas_boussinesq_solver` integrates, from 273.15 to
//!   41 temperatures and from the first knot to every knot.
//!
//! Each `I` row is compared bit for bit. Each `ERR` row (GSL returns `NaN`
//! with its error handler off: `a > b`, or a limit outside the table) must be
//! a `PetirError::Domain` here.
//!
//! Pass criterion: every `ERR` row refused, and worst relative difference
//! below `1e-12` (the bar the other `interp` comparison uses). The count of
//! bit-identical values is reported, not asserted.
//!
//! # Results (2026-10-20, x86_64 Linux, glibc, `gcc -O2`)
//!
//! ```text
//!   case                      values  bit-identical   worst rel. diff
//!   integ_runge_linear           231   231 (100.0 %)   0
//!   integ_runge_cspline          231   231 (100.0 %)   0
//!   integ_thermo_linear           52    52 (100.0 %)   0
//!   integ_thermo_cspline          52    52 (100.0 %)   0
//!   refusals (a > b, out of range)  10/10 refused as Domain
//! ```
//!
//! Interpretation: the port reproduces GSL's arithmetic exactly, as the
//! `eval`/`eval_deriv` comparison already did for the same coefficients.
//!
//! # Skipping
//!
//! Absent reference file means skip, not fail.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use petir::interp::{Interp, InterpMethod};
use petir::PetirError;

type Cases = BTreeMap<String, Vec<(String, Vec<f64>)>>;

fn load() -> Option<Cases> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/gsl/interp-integ-gsl-2.8-reference.txt");
    let text = fs::read_to_string(path).ok()?;
    let mut cases: Cases = BTreeMap::new();
    let mut current = String::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f.first().copied() {
            Some("CASE") => {
                current = f.get(1).copied().unwrap_or("?").to_string();
                cases.insert(current.clone(), Vec::new());
            }
            Some("END") => current.clear(),
            Some(tag) => {
                let nums: Vec<f64> = f
                    .iter()
                    .skip(1)
                    .filter_map(|v| v.parse::<f64>().ok())
                    .collect();
                if let Some(rows) = cases.get_mut(&current) {
                    rows.push((tag.to_string(), nums));
                }
            }
            None => {}
        }
    }
    Some(cases)
}

fn rel(a: f64, b: f64) -> f64 {
    let scale = a.abs().max(b.abs());
    if scale < 1e-300 {
        (a - b).abs()
    } else {
        (a - b).abs() / scale
    }
}

fn runge_table() -> (Vec<f64>, Vec<f64>) {
    let n = 9usize;
    let xa: Vec<f64> = (0..n)
        .map(|i| -1.0 + 2.0 * i as f64 / (n - 1) as f64)
        .collect();
    let ya = xa.iter().map(|&x| 1.0 / (1.0 + 25.0 * x * x)).collect();
    (xa, ya)
}

fn thermo_table() -> (Vec<f64>, Vec<f64>) {
    let xa = vec![
        200.0, 250.0, 273.15, 300.0, 350.0, 400.0, 500.0, 600.0, 800.0, 1000.0, 1200.0,
    ];
    let ya = xa
        .iter()
        .map(|&x| 500.0 + 0.4 * x - 1.5e-4 * x * x)
        .collect();
    (xa, ya)
}

#[test]
fn eval_integ_matches_compiled_gsl() {
    let Some(cases) = load() else {
        eprintln!("skipping: interp-integ-gsl-2.8-reference.txt not present");
        return;
    };
    let (rx, ry) = runge_table();
    let (tx, ty) = thermo_table();
    let mut refused = 0usize;
    let mut refusals = 0usize;
    for (name, method, xa, ya) in [
        ("integ_runge_linear", InterpMethod::Linear, &rx, &ry),
        ("integ_runge_cspline", InterpMethod::CubicSpline, &rx, &ry),
        ("integ_thermo_linear", InterpMethod::Linear, &tx, &ty),
        ("integ_thermo_cspline", InterpMethod::CubicSpline, &tx, &ty),
    ] {
        let rows = cases.get(name).unwrap_or_else(|| panic!("no case {name}"));
        let s = Interp::new(method, xa, ya).unwrap();
        let (mut compared, mut identical, mut worst) = (0usize, 0usize, 0.0_f64);
        for (tag, nums) in rows {
            let (Some(&a), Some(&b)) = (nums.first(), nums.get(1)) else {
                panic!("{name}: malformed row");
            };
            match tag.as_str() {
                "I" => {
                    let expected = *nums.get(2).expect("integral column");
                    let got = s.eval_integ(a, b).unwrap();
                    compared += 1;
                    if got.to_bits() == expected.to_bits() {
                        identical += 1;
                    } else {
                        worst = worst.max(rel(got, expected));
                    }
                }
                "ERR" => {
                    refusals += 1;
                    if s.eval_integ(a, b) == Err(PetirError::Domain) {
                        refused += 1;
                    }
                }
                other => panic!("{name}: unexpected tag {other}"),
            }
        }
        let pct = 100.0 * identical as f64 / compared as f64;
        eprintln!(
            "{name} vs GSL 2.8: {compared} values, {identical} bit-identical \
             ({pct:.1} %), worst relative difference {worst:e}"
        );
        assert!(compared > 0, "{name}: nothing compared");
        assert!(worst < 1e-12, "{name}: worst relative difference {worst:e}");
    }
    eprintln!("refusals: {refused}/{refusals} returned PetirError::Domain");
    assert_eq!(refused, refusals, "every GSL EDOM row must be refused");
}

/// NaN limits are refused (a documented deviation: GSL lets them through).
#[test]
fn eval_integ_refuses_nan_limits() {
    let (xa, ya) = thermo_table();
    let s = Interp::new(InterpMethod::CubicSpline, &xa, &ya).unwrap();
    assert_eq!(s.eval_integ(f64::NAN, 300.0), Err(PetirError::Domain));
    assert_eq!(s.eval_integ(300.0, f64::NAN), Err(PetirError::Domain));
}
