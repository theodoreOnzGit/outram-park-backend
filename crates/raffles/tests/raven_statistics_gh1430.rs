// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! **Differential test of raffles' sample statistics against RAVEN's
//! `BasicStatistics` PostProcessor** (gh:#1430, case 3).
//!
//! Reference: RAVEN's real `BasicStatistics._runLocal` (xarray 2024.6.0, numpy
//! 1.26.4, scipy 1.14.1; unweighted, `biased=False`) on a 10^4-row, 3-variable
//! dataset made only of exactly rounded IEEE operations, so this file rebuilds
//! the same bits (`statistics_dataset` mirrors `dataset()` in
//! `generate_statistics_reference.py`). Predictions, written before the run,
//! and measured results: `PREDICTIONS.md`, `RESULTS.md` and `README.md` in
//! `verification_and_validation/raven_distributions_gh1430/`.
//!
//! Compared: mean, unbiased variance, sigma, covariance (RAVEN's `cov_*`),
//! Pearson and Spearman. **Not compared, because raffles has no such
//! function** (a gap, recorded in the V&V README): skewness, kurtosis, median,
//! percentile (and the standard errors). `minimum` and `maximum` are trivial
//! and also not in the crate.

use std::path::PathBuf;

use raffles::sensitivity::{
    pearson_correlation, sample_mean, sample_variance, spearman_correlation,
};
use serde_json::Value;

const N: usize = 10_000;
const VARS: [&str; 3] = ["x", "y", "z"];

fn statistics_dataset() -> [Vec<f64>; 3] {
    let mut x = Vec::with_capacity(N);
    let mut y = Vec::with_capacity(N);
    let mut z = Vec::with_capacity(N);
    for i in 0..N {
        let a = ((i * 7919 + 13) % 10007) as f64 / 10007.0;
        let b = ((i * 104729 + 5) % 10009) as f64 / 10009.0;
        x.push(a * a + 0.5 * b);
        y.push(3.0 * a - b * b);
        z.push(b * b * b - 0.25 * a);
    }
    [x, y, z]
}

fn reference() -> Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/raven_distributions_gh1430/references/statistics.json");
    serde_json::from_str(&std::fs::read_to_string(&p).expect("reference file")).expect("json")
}

fn rel(ours: f64, theirs: f64) -> f64 {
    (ours - theirs).abs() / theirs.abs().max(f64::MIN_POSITIVE)
}

#[derive(Default)]
struct Tally {
    n: usize,
    identical: usize,
    worst: f64,
}

impl Tally {
    fn add(&mut self, ours: f64, theirs: f64) {
        self.n += 1;
        if ours.to_bits() == theirs.to_bits() {
            self.identical += 1;
        } else {
            self.worst = self.worst.max(rel(ours, theirs));
        }
    }
}

/// numpy's `pairwise_sum` (`numpy/core/src/umath/loops_utils.h.src`): below 8
/// elements a plain loop; up to 128 eight accumulators strided by 8, combined
/// as `((r0+r1)+(r2+r3))+((r4+r5)+(r6+r7))` and the remainder added in order;
/// above 128 split at `n/2` rounded down to a multiple of 8.
fn numpy_pairwise_sum(a: &[f64]) -> f64 {
    let n = a.len();
    if n < 8 {
        let mut res = 0.0;
        for v in a {
            res += *v;
        }
        res
    } else if n <= 128 {
        let mut r = [a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]];
        let mut i = 8;
        while i < n - (n % 8) {
            for k in 0..8 {
                r[k] += a[i + k];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while i < n {
            res += a[i];
            i += 1;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        numpy_pairwise_sum(&a[..n2]) + numpy_pairwise_sum(&a[n2..])
    }
}

#[test]
fn sample_statistics_against_raven_basic_statistics() {
    let r = reference();
    let v = &r["values"];
    let data = statistics_dataset();
    let get = |k: &str| v[k].as_f64().unwrap_or_else(|| panic!("missing {k}"));

    let (mut mean, mut var, mut sigma, mut cov, mut pear, mut spear) = (
        Tally::default(),
        Tally::default(),
        Tally::default(),
        Tally::default(),
        Tally::default(),
        Tally::default(),
    );
    for (i, a) in VARS.iter().enumerate() {
        mean.add(sample_mean(&data[i]).unwrap(), get(&format!("mean_{a}")));
        let s2 = sample_variance(&data[i]).unwrap();
        var.add(s2, get(&format!("var_{a}")));
        sigma.add(s2.sqrt(), get(&format!("sigma_{a}")));
        for (j, b) in VARS.iter().enumerate() {
            let p = pearson_correlation(&data[i], &data[j]).unwrap();
            pear.add(p, get(&format!("pear_{a}_{b}")));
            let sp = spearman_correlation(&data[i], &data[j]).unwrap();
            spear.add(sp, get(&format!("spear_{a}_{b}")));
            // RAVEN's covariance: dot(centred_a, centred_b) * 1/(n-1), with
            // the diagonal overwritten by the variance.
            let ma = sample_mean(&data[i]).unwrap();
            let mb = sample_mean(&data[j]).unwrap();
            let c = if i == j {
                sample_variance(&data[i]).unwrap()
            } else {
                data[i]
                    .iter()
                    .zip(&data[j])
                    .map(|(p, q)| (p - ma) * (q - mb))
                    .sum::<f64>()
                    * (1.0 / (N as f64 - 1.0))
            };
            cov.add(c, get(&format!("cov_{a}_{b}")));
        }
    }
    // Localisation of the mean/variance mismatch: re-do the sums in numpy's
    // pairwise order. If that reproduces RAVEN bit for bit, the first
    // divergent operation is the summation order (raffles sums left to
    // right), not the formula.
    let mut pw_mean = Tally::default();
    let mut pw_var = Tally::default();
    for (i, a) in VARS.iter().enumerate() {
        let m = numpy_pairwise_sum(&data[i]) / N as f64;
        pw_mean.add(m, get(&format!("mean_{a}")));
        let sq: Vec<f64> = data[i].iter().map(|x| (x - m) * (x - m)).collect();
        let var_biased = numpy_pairwise_sum(&sq) / N as f64;
        pw_var.add(var_biased * (N as f64 / (N as f64 - 1.0)), get(&format!("var_{a}")));
    }
    eprintln!(
        "numpy-order mean: {}/{} bit-identical, worst {:.3e}; numpy-order variance: {}/{}, worst {:.3e}",
        pw_mean.identical, pw_mean.n, pw_mean.worst, pw_var.identical, pw_var.n, pw_var.worst
    );
    let rows = [
        ("mean", &mean),
        ("variance", &var),
        ("sigma", &sigma),
        ("covariance", &cov),
        ("pearson", &pear),
        ("spearman", &spear),
    ];
    for (name, t) in rows {
        eprintln!(
            "{name}: {}/{} bit-identical, worst rel {:.3e}",
            t.identical, t.n, t.worst
        );
    }
    // Pins, measured 2026-11-13 (RESULTS.md): exact bit-identical counts, and
    // bounds just above the measured worst. Not to be relaxed.
    assert_eq!((pw_mean.identical, pw_var.identical), (3, 3), "numpy-order sums reproduce RAVEN");
    let pins = [
        (&mean, 0, 3, 1e-14),
        (&var, 0, 3, 1e-14),
        (&sigma, 0, 3, 1e-14),
        (&cov, 0, 9, 1e-14),
        (&pear, 1, 9, 1e-14),
        (&spear, 7, 9, 1e-15),
    ];
    for (i, (t, identical, n, bound)) in pins.iter().enumerate() {
        assert_eq!((t.identical, t.n), (*identical, *n), "row {i} count");
        assert!(t.worst < *bound, "row {i} worst {}", t.worst);
    }
}
