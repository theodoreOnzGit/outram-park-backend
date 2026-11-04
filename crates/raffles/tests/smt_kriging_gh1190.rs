// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! **Differential tests of the SMT kriging port against SMT itself**
//! (gh:#1190).
//!
//! The references are SMT @ 151a3ff3c137 run on fixed inputs with numpy
//! 2.4.6, scipy 1.17.1 and scikit-learn 1.9.1, by
//! `verification_and_validation/smt_kriging_gh1190/generate_references.py`
//! (command and versions in that script, the JSON `meta` blocks and the V&V
//! record `verification_and_validation/smt_kriging_gh1190.md`, which also holds
//! the predictions, committed before the port, and the measured results).
//!
//! Criterion (root `CLAUDE.md`, "Code-to-code"): bit-identical preferred,
//! otherwise below 1e-8 relative with the cause explained. Each test prints
//! its bit-identical counts and worst differences with `eprintln!`, which is
//! where the V&V record's numbers come from
//! (`cargo test --release -p raffles --test smt_kriging_gh1190 -- --nocapture`).

use std::path::PathBuf;

use raffles::surrogate::smt::{
    Mfk, MfkConfig, SmtCorrelation, SmtKriging, SmtKrgConfig, SmtOptimizer, SmtPls,
    SmtRegression, SmtStarts, TrendVariance,
};
use raffles::surrogate::{GaussianProcess, GpConfig, GpKernel, GpOptimizer, RavenKernel};
use serde_json::Value;

fn reference(name: &str) -> Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/smt_kriging_gh1190/references")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(&p).expect("reference file")).expect("json")
}

fn num(v: &Value) -> f64 {
    v.as_f64().expect("number")
}

fn vec1(v: &Value) -> Vec<f64> {
    v.as_array().expect("array").iter().map(num).collect()
}

fn vec2(v: &Value) -> Vec<Vec<f64>> {
    v.as_array().expect("array").iter().map(vec1).collect()
}

/// Running tally: values compared, how many were bit-identical, and the worst
/// difference among the rest on the stated scale.
#[derive(Default)]
struct Tally {
    n: usize,
    identical: usize,
    worst: f64,
}

impl Tally {
    /// `|ours − theirs| / scale`, bit-identity counted separately.
    fn scaled(&mut self, ours: f64, theirs: f64, scale: f64) {
        self.n += 1;
        if ours.to_bits() == theirs.to_bits() {
            self.identical += 1;
        } else {
            self.worst = self.worst.max((ours - theirs).abs() / scale.abs().max(f64::MIN_POSITIVE));
        }
    }

    fn rel(&mut self, ours: f64, theirs: f64) {
        self.scaled(ours, theirs, theirs);
    }

    fn report(&self, what: &str) -> String {
        let s = format!("{what}: {}/{} bit-identical, worst {:.3e}", self.identical, self.n, self.worst);
        eprintln!("{s}");
        s
    }

    /// The code-to-code criterion.
    fn gate(&self, what: &str, limit: f64) {
        let s = self.report(what);
        assert!(self.worst < limit, "{s} (limit {limit:e})");
    }
}

fn corr(v: &Value) -> SmtCorrelation {
    match v.as_str().expect("corr") {
        "squar_exp" => SmtCorrelation::SquarExp,
        "abs_exp" => SmtCorrelation::AbsExp,
        "matern52" => SmtCorrelation::Matern52,
        "matern32" => SmtCorrelation::Matern32,
        other => panic!("{other}"),
    }
}

fn poly(v: &Value) -> SmtRegression {
    match v.as_str().unwrap_or("constant") {
        "constant" => SmtRegression::Constant,
        "linear" => SmtRegression::Linear,
        "quadratic" => SmtRegression::Quadratic,
        other => panic!("{other}"),
    }
}

fn max_abs(v: &[f64]) -> f64 {
    v.iter().fold(0.0_f64, |m, x| m.max(x.abs()))
}

fn config_of(case: &Value) -> SmtKrgConfig {
    let pls = match (case["model"].as_str(), case["n_comp"].as_u64()) {
        (Some("KPLS"), Some(n)) => SmtPls::Kpls { n_comp: n as usize },
        (Some("KPLSK"), Some(n)) => SmtPls::Kplsk { n_comp: n as usize },
        _ => SmtPls::None,
    };
    SmtKrgConfig {
        corr: corr(&case["corr"]),
        poly: poly(&case["poly"]),
        pls,
        ..SmtKrgConfig::smt_defaults()
    }
}

/// Every comparison of one fitted KRG/KPLS at SMT's θ against SMT's record,
/// added to the tallies; returns the model.
fn compare_fixed(case: &Value, t: &mut [Tally; 8], mean_scale: f64) -> SmtKriging {
    let x = vec2(&case["xt"]);
    let y = vec1(&case["yt"]);
    let cfg = SmtKrgConfig {
        theta0: vec1(&case["optimal_theta"]),
        optimizer: SmtOptimizer::None,
        ..config_of(case)
    };
    let m = SmtKriging::fit(&cfg, &x, &y).unwrap();
    let (off, sc, ym, ys) = m.standardisation();
    for (a, b) in off.iter().zip(vec1(&case["x_offset"])) {
        t[0].rel(*a, b);
    }
    for (a, b) in sc.iter().zip(vec1(&case["x_scale"])) {
        t[0].rel(*a, b);
    }
    t[0].rel(ym, num(&case["y_mean"]));
    t[0].rel(ys, num(&case["y_std"]));
    t[1].rel(m.reduced_likelihood(), num(&case["rlf"]));
    let (s2, beta, gamma) = m.gls();
    t[2].rel(s2, num(&case["sigma2"]));
    let (rb, rg) = (vec1(&case["beta"]), vec1(&case["gamma"]));
    for (a, b) in beta.iter().zip(&rb) {
        t[3].scaled(*a, *b, max_abs(&rb));
    }
    for (a, b) in gamma.iter().zip(&rg) {
        t[3].scaled(*a, *b, max_abs(&rg));
    }
    for ((xt, mm), vv) in vec2(&case["x_test"]).iter().zip(vec1(&case["mean"])).zip(vec1(&case["variance"])) {
        let (pm, pv) = m.predict(xt).unwrap();
        t[4].scaled(pm, mm, mean_scale);
        t[5].scaled(pv, vv, s2);
    }
    if !case["rlf_gradient"].is_null() {
        let rg = vec1(&case["rlf_gradient"]);
        let (_, g) = m.reduced_likelihood_gradient_at(m.theta()).unwrap().unwrap();
        for (a, b) in g.iter().zip(&rg) {
            t[6].scaled(*a, *b, max_abs(&rg));
        }
    }
    if let (Some(c), false) = (m.coeff_pls(), case["coeff_pls"].is_null()) {
        let rc = vec2(&case["coeff_pls"]);
        let s = rc.iter().map(|r| max_abs(r)).fold(0.0, f64::max);
        for (a, b) in c.iter().flatten().zip(rc.iter().flatten()) {
            t[7].scaled(*a, *b, s);
        }
    }
    m
}

/// **Methodology.** Every fixed-θ case of `krg.json` (SMT `hyper_opt="NoOp"`):
/// the port fitted at SMT's own `optimal_theta` (no optimiser), compared on
/// standardisation, reduced likelihood, σ², β and γ (relative to the largest
/// entry), the mean at 25 or 23 test points (relative to max |y_train|), the
/// variance (relative to σ²) and the likelihood gradient (relative to its
/// largest component). Gate: the 1e-8 code-to-code criterion. Then finding 1
/// of the V&V record: on the linear and quadratic trends, the exact
/// universal-kriging variance against SMT's (prediction P7: > 1e-3 relative
/// somewhere).
///
/// **Results**: in the V&V record, table "Differential table".
#[test]
fn krg_numerics_match_smt_at_fixed_theta() {
    let r = reference("krg.json");
    let mut t: [Tally; 8] = Default::default();
    let mut exact_gap = 0.0_f64;
    for case in r["cases"].as_array().unwrap().iter().filter(|c| c["kind"] == "fixed") {
        let scale = max_abs(&vec1(&case["yt"]));
        let m = compare_fixed(case, &mut t, scale);
        if case["poly"] != "constant" {
            let cfg = SmtKrgConfig {
                theta0: m.theta().to_vec(),
                optimizer: SmtOptimizer::None,
                trend_variance: TrendVariance::Exact,
                ..config_of(case)
            };
            let e = SmtKriging::fit(&cfg, &vec2(&case["xt"]), &vec1(&case["yt"])).unwrap();
            let s2 = m.gls().0;
            for (xt, vv) in vec2(&case["x_test"]).iter().zip(vec1(&case["variance"])) {
                exact_gap = exact_gap.max((e.predict(xt).unwrap().1 - vv).abs() / s2);
            }
        }
    }
    t[0].gate("KRG standardisation (relative)", 1e-15);
    t[1].gate("KRG reduced likelihood (relative)", 1e-8);
    t[2].gate("KRG sigma2 (relative)", 1e-8);
    t[3].gate("KRG beta, gamma (relative to largest)", 1e-8);
    t[4].gate("KRG mean (relative to max |y|)", 1e-8);
    t[5].gate("KRG variance (relative to sigma2)", 1e-8);
    t[6].gate("KRG likelihood gradient (relative to largest)", 1e-8);
    eprintln!("finding 1: exact trend variance vs SMT's, worst {exact_gap:.3e} of sigma2");
    assert!(exact_gap > 1e-3, "P7: SMT's diag(G) variance should differ from the exact one");
}

/// The port's fit of `case` from `starts`, scored against SMT's record:
/// (θ, rlf, worst prediction gap relative to max |y|, held-out RMSE and 2σ
/// coverage if the case has true values).
fn fit_case(case: &Value, starts: SmtStarts) -> (Vec<f64>, f64, f64, Option<(f64, f64)>) {
    let cfg = SmtKrgConfig { starts, ..config_of(case) };
    let x = vec2(&case["xt"]);
    let y = vec1(&case["yt"]);
    let m = SmtKriging::fit(&cfg, &x, &y).unwrap();
    let scale = max_abs(&y);
    let xt = vec2(&case["x_test"]);
    let preds: Vec<(f64, f64)> = xt.iter().map(|p| m.predict(p).unwrap()).collect();
    let gap = preds.iter().zip(vec1(&case["mean"])).map(|(p, s)| (p.0 - s).abs() / scale).fold(0.0, f64::max);
    let score = (!case["y_test"].is_null()).then(|| score(&preds, &vec1(&case["y_test"])));
    (m.theta().to_vec(), m.reduced_likelihood(), gap, score)
}

/// Held-out RMSE and the fraction within ±2σ.
fn score(preds: &[(f64, f64)], truth: &[f64]) -> (f64, f64) {
    let n = truth.len() as f64;
    let rmse = (preds.iter().zip(truth).map(|(p, y)| (p.0 - y) * (p.0 - y)).sum::<f64>() / n).sqrt();
    let cov = preds.iter().zip(truth).filter(|(p, y)| (p.0 - *y).abs() <= 2.0 * p.1.max(0.0).sqrt()).count() as f64 / n;
    (rmse, cov)
}

fn theta_gap(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs() / y.abs()).fold(0.0, f64::max)
}

/// **Methodology.** The four KRG fit cases with SMT's defaults: the port's
/// bounded BFGS from (a) SMT's own recorded starts and (b) the port's own
/// starts (`SmtStarts::smt_default()`), against SMT's TNC answer and the
/// polished optimum (SMT's objective, TNC restarted with maxfun 10000).
/// Prediction P9: likelihood ≥ SMT's − 1e-9 and θ within 1e-4 of polished;
/// P10: (b) reaches the same optimum. Gate: the likelihood (an optimiser
/// that ends lower than SMT's TNC has failed); θ is reported.
///
/// **Results**: in the V&V record, "Fits".
#[test]
fn krg_fits_reach_smts_optimum() {
    let r = reference("krg.json");
    for case in r["cases"].as_array().unwrap().iter().filter(|c| c["kind"] == "fit") {
        let pol = vec1(&case["polished"]["theta"]);
        let (rlf_smt, rlf_pol) = (num(&case["rlf"]), num(&case["polished"]["rlf"]));
        for (label, starts) in [("SMT starts", SmtStarts::Explicit(vec2(&case["starts_log10"]))), ("own starts", SmtStarts::smt_default())] {
            let (th, rlf, gap, sc) = fit_case(case, starts);
            eprintln!(
                "{} [{label}]: theta {th:?} (SMT {:?}, polished {pol:?}, gap to polished {:.2e}); rlf {rlf:.12} (SMT {rlf_smt:.12}, polished {rlf_pol:.12}); prediction gap {gap:.2e}; held-out {sc:?}",
                case["name"],
                vec1(&case["optimal_theta"]),
                theta_gap(&th, &pol)
            );
            if num(&case["cond_r"]) <= 1e8 {
                assert!(rlf >= rlf_smt - 1e-9, "{}: likelihood below SMT's", case["name"]);
            }
        }
        let cfg = SmtKrgConfig { theta0: pol.clone(), optimizer: SmtOptimizer::None, ..config_of(case) };
        let at = SmtKriging::fit(&cfg, &vec2(&case["xt"]), &vec1(&case["yt"])).unwrap();
        eprintln!("{}: cond(R) at SMT's optimum {:.3e}; the port's likelihood AT the polished theta {:.12} (SMT {rlf_pol:.12})", case["name"], num(&case["cond_r"]), at.reduced_likelihood());
        if !case["y_test"].is_null() {
            let smt: Vec<(f64, f64)> = vec1(&case["mean"]).into_iter().zip(vec1(&case["variance"])).collect();
            eprintln!("{} SMT's own held-out (RMSE, 2σ coverage): {:?}", case["name"], score(&smt, &vec1(&case["y_test"])));
        }
    }
}

/// **Methodology.** `kpls.json`: KPLS at fixed θ (squar_exp and abs_exp,
/// n_comp 2, Rosenbrock 6-D, 40 points) compared as the KRG cases plus
/// `coeff_pls` (P11, P12); the KPLS and KPLSK fits from SMT's recorded starts
/// and the port's own (P13, P14).
///
/// **Results**: in the V&V record.
#[test]
fn kpls_and_kplsk_match_smt() {
    let r = reference("kpls.json");
    let mut t: [Tally; 8] = Default::default();
    for case in r["cases"].as_array().unwrap().iter().filter(|c| c["kind"] == "fixed") {
        compare_fixed(case, &mut t, max_abs(&vec1(&case["yt"])));
    }
    t[7].gate("KPLS coeff_pls (relative to largest)", 1e-8);
    t[0].gate("KPLS standardisation", 1e-15);
    t[1].gate("KPLS reduced likelihood", 1e-8);
    t[2].gate("KPLS sigma2", 1e-8);
    t[3].gate("KPLS beta, gamma", 1e-8);
    t[4].gate("KPLS mean", 1e-8);
    t[5].gate("KPLS variance", 1e-8);
    t[6].gate("KPLS likelihood gradient", 1e-8);
    for case in r["cases"].as_array().unwrap().iter().filter(|c| c["kind"] == "fit") {
        let explicit = if case["model"] == "KPLSK" {
            let mut s = vec2(&case["starts_log10_pls_pass"]);
            s.extend(vec2(&case["starts_log10_full_pass"]).into_iter().skip(1));
            s
        } else {
            vec2(&case["starts_log10"])
        };
        let smt: Vec<(f64, f64)> = vec1(&case["mean"]).into_iter().zip(vec1(&case["variance"])).collect();
        let smt_score = score(&smt, &vec1(&case["y_test"]));
        for (label, starts) in [("SMT starts", SmtStarts::Explicit(explicit.clone())), ("own starts", SmtStarts::smt_default())] {
            let (th, rlf, gap, sc) = fit_case(case, starts);
            eprintln!(
                "{} [{label}]: theta {th:?} (SMT {:?}); rlf {rlf:.10} (SMT {:.10}); prediction gap {gap:.2e}; held-out {sc:?} (SMT {smt_score:?})",
                case["name"],
                vec1(&case["optimal_theta"]),
                num(&case["rlf"])
            );
            if label == "SMT starts" {
                assert!(rlf >= num(&case["rlf"]) - 1e-6, "{}", case["name"]);
            }
        }
    }
}

fn mfk_config(case: &Value) -> MfkConfig {
    let o = &case["options"];
    MfkConfig {
        poly: poly(&o["poly"]),
        rho_regr: poly(&o["rho_regr"]),
        propagate_uncertainty: o["propagate_uncertainty"].as_bool().unwrap_or(true),
        ..MfkConfig::smt_defaults()
    }
}

fn mfk_levels(case: &Value) -> Vec<(Vec<Vec<f64>>, Vec<f64>)> {
    case["x_levels"].as_array().unwrap().iter().zip(case["y_levels"].as_array().unwrap()).map(|(x, y)| (vec2(x), vec1(y))).collect()
}

/// **Methodology.** The three fixed-θ MFK cases (Forrester, 11 LF and 4 or 6
/// HF points): the port at SMT's per-level θ, compared level by level on the
/// reduced likelihood, σ², β, γ, then the mean, the variance of every level
/// and σ²_ρ at 21 points (P15). Gate: 1e-8.
///
/// **Results**: in the V&V record.
#[test]
fn mfk_numerics_match_smt_at_fixed_theta() {
    let r = reference("mfk.json");
    // [well-conditioned, ill-conditioned] (V&V addendum: cond(R) > 1e8 at
    // any level puts the whole case in the second, reported, not gated).
    let mut tallies: [[Tally; 6]; 2] = Default::default();
    for case in r["cases"].as_array().unwrap().iter().filter(|c| c["kind"] == "fixed") {
        let levels = mfk_levels(case);
        let lv = case["levels"].as_array().unwrap();
        let worst_cond = lv.iter().map(|l| num(&l["cond_r"])).fold(0.0, f64::max);
        let t = &mut tallies[usize::from(worst_cond > 1e8)];
        let cfg = MfkConfig {
            theta0: lv.iter().map(|l| vec1(&l["optimal_theta"])).collect(),
            optimizer: SmtOptimizer::None,
            ..mfk_config(case)
        };
        let m = Mfk::fit(&cfg, &levels).unwrap();
        let (xo, xs, ym, ys) = m.standardisation();
        for (a, b) in xo.iter().zip(vec1(&case["x_offset"])).chain(xs.iter().zip(vec1(&case["x_scale"]))) {
            t[0].rel(*a, b);
        }
        t[0].rel(ym, num(&case["y_mean"]));
        t[0].rel(ys, num(&case["y_std"]));
        let mut s2_top = 0.0;
        for (i, l) in lv.iter().enumerate() {
            let (_, rlf, s2, beta, gamma) = m.level(i).unwrap();
            eprintln!("{} level {i} (cond(R) {:.3e}): rlf {rlf} vs SMT {}, sigma2 {s2} vs SMT {}", case["name"], num(&l["cond_r"]), num(&l["rlf"]), num(&l["sigma2"]));
            if i == 0 && !case["level0_sigma2_unblocked_cholesky"].is_null() {
                let u = num(&case["level0_sigma2_unblocked_cholesky"]);
                eprintln!("  unblocked-Cholesky Python sigma2 {u}: port vs it {:.3e} relative; port vs SMT (LAPACK) {:.3e}", (s2 - u).abs() / u, (s2 - num(&l["sigma2"])).abs() / num(&l["sigma2"]));
                assert!((s2 - u).abs() / u < 1e-8, "the ill-conditioned gap is the factorisation order");
            }
            t[1].rel(rlf, num(&l["rlf"]));
            t[1].rel(s2, num(&l["sigma2"]));
            let (rb, rg) = (vec1(&l["beta"]), vec1(&l["gamma"]));
            for (a, b) in beta.iter().zip(&rb) {
                t[2].scaled(*a, *b, max_abs(&rb));
            }
            for (a, b) in gamma.iter().zip(&rg) {
                t[2].scaled(*a, *b, max_abs(&rg));
            }
            s2_top = s2;
        }
        let scale = max_abs(&levels[levels.len() - 1].1);
        let all = vec2(&case["variance_all_levels"]);
        let rhos: Vec<Vec<f64>> = case["sigma2_rho"].as_array().unwrap().iter().map(vec1).collect();
        for (k, xt) in vec2(&case["x_test"]).iter().enumerate() {
            let (pm, pv, pr) = m.predict_all_levels(xt).unwrap();
            t[3].scaled(pm, num(&case["mean"][k]), scale);
            for (a, b) in pv.iter().zip(&all[k]) {
                t[4].scaled(*a, *b, s2_top);
            }
            for (i, a) in pr.iter().enumerate() {
                t[5].scaled(*a, rhos[i][k], rhos[i].iter().fold(0.0_f64, |s, v| s.max(v.abs())));
            }
        }
    }
    let [t, ill] = &tallies;
    t[0].gate("MFK standardisation", 1e-15);
    t[1].gate("MFK per-level reduced likelihood and sigma2", 1e-8);
    t[2].gate("MFK per-level beta, gamma", 1e-8);
    t[3].gate("MFK mean (relative to max |y_HF|)", 1e-8);
    t[4].gate("MFK variance, all levels (relative to the top level's sigma2)", 1e-8);
    t[5].gate("MFK sigma2_rho (relative to largest)", 1e-8);
    ill[0].gate("[cond(R) > 1e8, reported] MFK standardisation", 1e-15);
    for (k, what) in ["likelihood and sigma2", "beta, gamma", "mean", "variance", "sigma2_rho"].iter().enumerate() {
        ill[k + 1].report(&format!("[cond(R) > 1e8, reported, not gated] MFK {what}"));
    }
}

/// **Methodology.** SMT's default MFK fit on Forrester (4 HF, 11 LF) and its
/// single-fidelity KRG on the 4 HF points, both scored on 101 equispaced
/// held-out points (RMSE, 2σ coverage); the port from SMT's recorded starts
/// and from its own (P16, P17).
///
/// **Results**: in the V&V record, "Forrester".
#[test]
fn mfk_fit_and_the_forrester_study() {
    let r = reference("mfk.json");
    let cases = r["cases"].as_array().unwrap();
    let fit = cases.iter().find(|c| c["name"] == "mfk_forrester_fit").unwrap();
    let truth = vec1(&fit["y_test"]);
    let xt = vec2(&fit["x_test"]);
    let smt: Vec<(f64, f64)> = vec1(&fit["mean"]).into_iter().zip(vec1(&fit["variance"])).collect();
    let smt_score = score(&smt, &truth);
    let per_level: Vec<SmtStarts> = fit["starts_log10"].as_array().unwrap().iter().map(|s| SmtStarts::Explicit(vec2(s))).collect();
    for (label, starts) in [("SMT starts", per_level), ("own starts", vec![SmtStarts::smt_default()])] {
        let cfg = MfkConfig { starts, ..mfk_config(fit) };
        let m = Mfk::fit(&cfg, &mfk_levels(fit)).unwrap();
        let preds: Vec<(f64, f64)> = xt.iter().map(|x| m.predict(x).unwrap()).collect();
        let sc = score(&preds, &truth);
        for (i, l) in fit["levels"].as_array().unwrap().iter().enumerate() {
            let (th, rlf, ..) = m.level(i).unwrap();
            eprintln!("MFK [{label}] level {i} (cond(R) at SMT's optimum {:.3e}): theta {th:?} (SMT {:?}); rlf {rlf:.10} (SMT {:.10})", num(&l["cond_r"]), vec1(&l["optimal_theta"]), num(&l["rlf"]));
            if num(&l["cond_r"]) <= 1e8 {
                assert!(rlf >= num(&l["rlf"]) - 1e-9);
            }
        }
        eprintln!("MFK [{label}] held-out (RMSE, 2σ coverage) {sc:?}; SMT {smt_score:?}");
    }
    let k = cases.iter().find(|c| c["name"] == "krg_forrester_hf_only_fit").unwrap();
    let (th, rlf, gap, sc) = fit_case(k, SmtStarts::Explicit(vec2(&k["starts_log10"])));
    let ksmt: Vec<(f64, f64)> = vec1(&k["mean"]).into_iter().zip(vec1(&k["variance"])).collect();
    eprintln!("KRG HF only: theta {th:?} rlf {rlf} gap {gap:.2e} held-out {sc:?}; SMT {:?}", score(&ksmt, &truth));
}

/// **Methodology.** The SMT route against the RAVEN/sklearn route
/// (`smt_kriging_gh1190.md`, "Cross-check"). (1) At fixed hyperparameters, on
/// `branin_squar_exp`: RAVEN's GP with `ConstantKernel(σ²) * RBF(ℓ)`,
/// `ℓ_i = s_i/√(2θ_i)`, `alpha = σ² nugget`, fitted to `y − m₀` with
/// `m₀ = y_mean + y_std β`, must give SMT's mean, and SMT's variance minus
/// its GLS term (P18). (2) Each route's default fit on the Branin 20 points,
/// held-out RMSE and coverage on the 100 points (P19).
///
/// **Results**: in the V&V record, "Cross-check".
#[test]
fn smt_and_raven_routes_cross_check() {
    let r = reference("krg.json");
    let cases = r["cases"].as_array().unwrap();
    let case = cases.iter().find(|c| c["name"] == "branin_squar_exp").unwrap();
    let x = vec2(&case["xt"]);
    let y = vec1(&case["yt"]);
    let theta = vec1(&case["optimal_theta"]);
    let smt = SmtKriging::fit(&SmtKrgConfig::at_theta(theta.clone()), &x, &y).unwrap();
    let (_, sc, ym, ys) = smt.standardisation();
    let (s2, beta, _) = smt.gls();
    let m0 = ym + ys * beta[0];
    let kernel = GpKernel {
        amplitude: Some(raffles::surrogate::gaussian_process::Hyper::fixed(s2)),
        length_scale: sc.iter().zip(&theta).map(|(s, t)| s / (2.0 * t).sqrt()).collect(),
        length_scale_bounds: None,
        ..GpKernel::raven(RavenKernel::ConstantRbf, 2, true)
    };
    let cfg = GpConfig { kernel, alpha: s2 * 100.0 * f64::EPSILON, normalize_y: false, optimizer: GpOptimizer::None };
    let yc: Vec<f64> = y.iter().map(|v| v - m0).collect();
    let gp = GaussianProcess::fit(&cfg, &x, &yc).unwrap();
    let scale = max_abs(&y);
    let (mut dm, mut dv, mut gls_max) = (0.0_f64, 0.0_f64, 0.0_f64);
    for xt in vec2(&case["x_test"]) {
        let (sm, sv) = smt.predict(&xt).unwrap();
        let (rm, rs) = gp.predict(&xt).unwrap();
        // SMT's GLS term at this point: its variance minus the simple-kriging one.
        dm = dm.max(((rm + m0) - sm).abs() / scale);
        let gls = sv - rs * rs;
        gls_max = gls_max.max(gls / s2);
        dv = dv.max(gls.min(0.0).abs() / s2);
    }
    eprintln!("cross-check at mapped hyperparameters: mean gap {dm:.3e} of max|y|; SMT var − RAVEN var in [0, {gls_max:.3e}] σ² (negative part {dv:.3e})");
    assert!(dm < 1e-9, "P18 mean");
    assert!(dv < 1e-9, "P18: SMT's variance is RAVEN's plus a non-negative GLS term");
    // (2) Default fits.
    let fit = cases.iter().find(|c| c["name"] == "fit_branin_squar_exp").unwrap();
    let xt = vec2(&fit["x_test"]);
    let truth = vec1(&fit["y_test"]);
    let s = SmtKriging::fit(&SmtKrgConfig::smt_defaults(), &x, &y).unwrap();
    let sp: Vec<(f64, f64)> = xt.iter().map(|p| s.predict(p).unwrap()).collect();
    let g = GaussianProcess::fit(&GpConfig::raven_with_kernel(GpKernel::raven(RavenKernel::ConstantRbf, 2, true)), &x, &y).unwrap();
    let gp_preds: Vec<(f64, f64)> = xt.iter().map(|p| {
        let (m, sd) = g.predict(p).unwrap();
        (m, sd * sd)
    }).collect();
    let mean_t = truth.iter().sum::<f64>() / truth.len() as f64;
    let sd_t = (truth.iter().map(|v| (v - mean_t) * (v - mean_t)).sum::<f64>() / truth.len() as f64).sqrt();
    let (a, b) = (score(&sp, &truth), score(&gp_preds, &truth));
    eprintln!("default fits on Branin, held-out (RMSE, 2σ coverage): SMT KRG {a:?}, RAVEN Constant*RBF {b:?}; held-out std {sd_t:.4}; RAVEN kernel {:?}", g.kernel());
    assert!(a.0 < 0.25 * sd_t, "P19 SMT");
    eprintln!("P19 RAVEN below 25 % of std: {}", b.0 < 0.25 * sd_t);
}
