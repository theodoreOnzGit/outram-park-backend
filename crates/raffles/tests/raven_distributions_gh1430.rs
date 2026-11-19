// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! **Differential test of raffles' 1-D distributions against RAVEN's own**
//! (gh:#1430, case 1).
//!
//! References: RAVEN `ravenframework/Distributions1D.py` (the `Basic*` classes
//! over `scipy.stats`, plus RAVEN's own `LogNormal`), run by
//! `verification_and_validation/raven_distributions_gh1430/generate_references.py`
//! into `references/distributions.json` (18 cases: 12 untruncated, 6
//! truncated; versions in the JSON `meta`). Predictions, written before the
//! run, and measured results: `PREDICTIONS.md`, `RESULTS.md` and `README.md`
//! in that folder.
//!
//! Criterion (root `CLAUDE.md`, "Code-to-code"): bit-identical preferred,
//! otherwise below 1e-8 relative with the cause explained. Run with
//! `--nocapture` to print the per-case tallies.

use std::path::PathBuf;

use raffles::distributions::{
    Beta, ContinuousDistribution1D, Distribution, Exponential, Gamma, LogNormal, Normal,
    Triangular, Truncated, Uniform, Weibull,
};
use serde_json::Value;

fn reference() -> Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/raven_distributions_gh1430/references/distributions.json");
    serde_json::from_str(&std::fs::read_to_string(&p).expect("reference file")).expect("json")
}

/// A reference number; JSON `null` is an infinity or NaN (see the generator).
fn opt(v: &Value) -> Option<f64> {
    v.as_f64()
}

fn p(v: &Value, k: &str) -> f64 {
    v["params"][k].as_f64().expect("param")
}

/// The raffles distribution for one reference case.
enum Under {
    Plain(Distribution),
    Trunc(Truncated),
}

impl Under {
    fn pdf(&self, x: f64) -> f64 {
        match self {
            Self::Plain(d) => d.pdf(x),
            Self::Trunc(d) => d.pdf(x),
        }
    }
    fn cdf(&self, x: f64) -> f64 {
        match self {
            Self::Plain(d) => d.cdf(x),
            Self::Trunc(d) => d.cdf(x),
        }
    }
    fn ppf(&self, q: f64) -> f64 {
        match self {
            Self::Plain(d) => d.ppf(q).expect("ppf"),
            Self::Trunc(d) => d.ppf(q).expect("ppf"),
        }
    }
}

fn base(case: &Value) -> Distribution {
    let kind = case["kind"].as_str().expect("kind");
    match kind {
        "uniform" => Distribution::Uniform(Uniform::new(p(case, "lower"), p(case, "upper")).unwrap()),
        "normal" => Distribution::Normal(Normal::new(p(case, "mu"), p(case, "sigma")).unwrap()),
        "lognormal" => Distribution::LogNormal(
            LogNormal::new(p(case, "mu"), p(case, "sigma"), p(case, "low")).unwrap(),
        ),
        "triangular" => Distribution::Triangular(
            Triangular::new(p(case, "lower"), p(case, "apex"), p(case, "upper")).unwrap(),
        ),
        "exponential" => {
            Distribution::Exponential(Exponential::new(p(case, "lambda"), p(case, "low")).unwrap())
        }
        "weibull" => Distribution::Weibull(
            Weibull::new(p(case, "k"), p(case, "lambda"), p(case, "low")).unwrap(),
        ),
        "gamma" => Distribution::Gamma(
            Gamma::new(p(case, "alpha"), p(case, "beta"), p(case, "low")).unwrap(),
        ),
        "beta" => Distribution::Beta(
            Beta::new(p(case, "alpha"), p(case, "beta"), p(case, "low"), p(case, "high")).unwrap(),
        ),
        other => panic!("unknown kind {other}"),
    }
}

fn build(case: &Value) -> Under {
    let b = base(case);
    match case["trunc"].as_array() {
        None => Under::Plain(b),
        Some(t) => Under::Trunc(
            Truncated::new(b, t[0].as_f64().unwrap(), t[1].as_f64().unwrap()).unwrap(),
        ),
    }
}

/// Bit-identical count and worst relative difference.
#[derive(Default, Clone, Copy)]
struct Tally {
    n: usize,
    identical: usize,
    worst: f64,
    worst_at: f64,
    /// Worst absolute difference (units of the quantity).
    worst_abs: f64,
}

impl Tally {
    fn add(&mut self, at: f64, ours: f64, theirs: f64) {
        self.n += 1;
        self.worst_abs = self.worst_abs.max((ours - theirs).abs());
        if ours.to_bits() == theirs.to_bits() {
            self.identical += 1;
        } else {
            let scale = theirs.abs().max(f64::MIN_POSITIVE);
            let r = (ours - theirs).abs() / scale;
            if r > self.worst {
                self.worst = r;
                self.worst_at = at;
            }
        }
    }
    fn merge(&mut self, o: &Tally) {
        self.n += o.n;
        self.identical += o.identical;
        self.worst_abs = self.worst_abs.max(o.worst_abs);
        if o.worst > self.worst {
            self.worst = o.worst;
            self.worst_at = o.worst_at;
        }
    }
}

struct CaseResult {
    label: String,
    pdf: Tally,
    cdf: Tally,
    ppf: Tally,
    moments: Tally,
    /// Worst ppf disagreement seen through the CDF, in units of 2^-53 of
    /// probability: |F(ours) - F(theirs)| / 2^-53 (raffles' own F).
    ppf_prob_units: f64,
    /// RAVEN `LogNormal.std()` minus raffles' std, over `low` (1.0 = RAVEN
    /// adds `low`; see `Distributions1D.py:303`).
    raven_std_excess_over_low: Option<f64>,
}

fn run_case(case: &Value) -> CaseResult {
    let d = build(case);
    let label = format!(
        "{}{} {}",
        case["kind"].as_str().unwrap(),
        if case["trunc"].is_array() { "[trunc]" } else { "" },
        case["params"]
    );
    let mut r = CaseResult {
        label,
        pdf: Tally::default(),
        cdf: Tally::default(),
        ppf: Tally::default(),
        moments: Tally::default(),
        ppf_prob_units: 0.0,
        raven_std_excess_over_low: None,
    };
    let xs = case["x"].as_array().unwrap();
    for (i, x) in xs.iter().enumerate() {
        let x = x.as_f64().unwrap();
        let pdf_ref = opt(&case["pdf"][i]);
        let ours = d.pdf(x);
        match pdf_ref {
            Some(t) => r.pdf.add(x, ours, t),
            None => assert!(ours.is_infinite() || ours.is_nan(), "pdf at {x}: {ours}"),
        }
        r.cdf.add(x, d.cdf(x), case["cdf"][i].as_f64().unwrap());
    }
    let ps = case["p"].as_array().unwrap();
    for (i, q) in ps.iter().enumerate() {
        let q = q.as_f64().unwrap();
        match opt(&case["ppf"][i]) {
            Some(t) => {
                let ours = d.ppf(q);
                r.ppf.add(q, ours, t);
                if q > 0.0 && q < 1.0 {
                    let units = (d.cdf(ours) - d.cdf(t)).abs() / 2f64.powi(-53);
                    r.ppf_prob_units = r.ppf_prob_units.max(units);
                }
            }
            None => assert!(d.ppf(q).is_infinite(), "ppf({q}) = {}", d.ppf(q)),
        }
    }
    if let Under::Plain(b) = &d {
        if let Some(m) = opt(&case["mean"]) {
            r.moments.add(0.0, b.mean(), m);
        }
        if let Some(s) = opt(&case["std"]) {
            if case["kind"] == "lognormal" && p(case, "low") != 0.0 {
                // RAVEN's LogNormal.std() returns sqrt(Var) + low: a defect
                // upstream (a shift does not change the spread). raffles
                // does not copy it; compare with the shift removed and
                // record the excess.
                let low = p(case, "low");
                r.raven_std_excess_over_low = Some((s - b.std_dev()) / low);
                r.moments.add(1.0, b.std_dev(), s - low);
            } else {
                r.moments.add(1.0, b.std_dev(), s);
            }
        }
    }
    r
}

#[test]
fn distributions_against_raven_distributions1d() {
    let reference = reference();
    let cases = reference["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 18);
    let mut tot = [Tally::default(); 4];
    let mut ppf_prob_units = Vec::new();
    let mut excess = Vec::new();
    for case in cases {
        let r = run_case(case);
        ppf_prob_units.push(r.ppf_prob_units);
        if let Some(e) = r.raven_std_excess_over_low {
            excess.push(e);
        }
        for (t, c) in tot.iter_mut().zip([&r.pdf, &r.cdf, &r.ppf, &r.moments]) {
            t.merge(c);
        }
        eprintln!(
            "{}\n   pdf {}/{} worst {:.2e} @ {:.4e} | cdf {}/{} worst {:.2e} @ {:.4e} | ppf {}/{} worst {:.2e} @ {:.4e} | mom {}/{} worst {:.2e} | ppf in prob units {:.2} | raven std excess/low {:?}",
            r.label,
            r.pdf.identical, r.pdf.n, r.pdf.worst, r.pdf.worst_at,
            r.cdf.identical, r.cdf.n, r.cdf.worst, r.cdf.worst_at,
            r.ppf.identical, r.ppf.n, r.ppf.worst, r.ppf.worst_at,
            r.moments.identical, r.moments.n, r.moments.worst,
            r.ppf_prob_units, r.raven_std_excess_over_low,
        );
    }
    for (name, t) in ["pdf", "cdf", "ppf", "moments"].iter().zip(tot.iter()) {
        eprintln!(
            "TOTAL {name}: {}/{} bit-identical, worst rel {:.3e} @ {:.4e}, worst abs {:.3e}",
            t.identical, t.n, t.worst, t.worst_at, t.worst_abs
        );
    }
    // Pins, measured 2026-11-13 (RESULTS.md). Counts are exact; bounds sit
    // just above the measured worst and are not to be relaxed.
    assert_eq!((tot[0].identical, tot[0].n), (179, 564), "pdf bit-identical count");
    assert_eq!((tot[1].identical, tot[1].n), (258, 567), "cdf bit-identical count");
    assert_eq!((tot[2].identical, tot[2].n), (255, 513), "ppf bit-identical count");
    assert_eq!((tot[3].identical, tot[3].n), (16, 24), "moments bit-identical count");
    assert!(tot[0].worst < 1e-14, "pdf rel {}", tot[0].worst);
    // CDF compared absolutely: the truncated CDF at a point 1e-8 above the
    // lower bound is a difference of two nearly equal CDFs, so a relative
    // bound would be set by conditioning, not by the port.
    assert!(tot[1].worst_abs < 1e-14, "cdf abs {}", tot[1].worst_abs);
    assert!(tot[2].worst < 1e-8, "ppf rel {}", tot[2].worst);
    assert!(tot[3].worst < 1e-14, "moments rel {}", tot[3].worst);
    let worst_prob_units = ppf_prob_units.iter().cloned().fold(0.0, f64::max);
    assert!(worst_prob_units <= 40.0, "ppf in prob units {worst_prob_units}");
    // The RAVEN LogNormal.std() defect: exactly `low` too large.
    assert_eq!(excess.len(), 1);
    assert!((excess[0] - 1.0).abs() < 1e-14, "excess {}", excess[0]);
}
