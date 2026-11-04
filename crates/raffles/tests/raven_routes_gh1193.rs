// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! **Differential tests of the RAVEN routes against RAVEN itself** (gh:#1193).
//!
//! The references are RAVEN `devel` @ 8baaae3a run on fixed inputs with numpy
//! 1.26.4, scipy 1.14.1 and scikit-learn 1.3.2, by
//! `verification_and_validation/raven_routes_gh1193/generate_references.py`
//! (command and versions in that script, the JSON `meta` blocks and the V&V
//! record `verification_and_validation/raven_routes_gh1193.md`, which also
//! holds the predictions, written before the port, and the measured results).
//!
//! Criterion (root `CLAUDE.md`, "Code-to-code"): bit-identical preferred,
//! otherwise below 1e-8 relative with the cause explained. Each test prints
//! its bit-identical count and worst difference with `eprintln!`, which is
//! where the V&V record's numbers come from
//! (`cargo test --release -p raffles --test raven_routes_gh1193 -- --nocapture`).

use std::path::PathBuf;

use raffles::bayesian::{
    raven_mcmc, BoundHandling, Draws, IndependentPrior, LikelihoodForm, RavenMcmcConfig,
    RavenMcmcKind,
};
use raffles::distributions::{Distribution, Normal, Uniform};
use serde_json::Value;

fn reference(name: &str) -> Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("verification_and_validation/raven_routes_gh1193/references")
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

/// Running tally of a comparison: how many values were bit-identical, and
/// the worst relative (or absolute) difference among the rest.
#[derive(Default)]
struct Tally {
    n: usize,
    identical: usize,
    worst: f64,
}

impl Tally {
    fn rel(&mut self, ours: f64, theirs: f64) {
        self.n += 1;
        if ours.to_bits() == theirs.to_bits() {
            self.identical += 1;
        } else {
            let scale = theirs.abs().max(f64::MIN_POSITIVE);
            self.worst = self.worst.max((ours - theirs).abs() / scale);
        }
    }

    fn abs(&mut self, ours: f64, theirs: f64) {
        self.n += 1;
        if ours.to_bits() == theirs.to_bits() {
            self.identical += 1;
        } else {
            self.worst = self.worst.max((ours - theirs).abs());
        }
    }

    fn report(&self, what: &str) -> String {
        let s = format!(
            "{what}: {}/{} bit-identical, worst {:.3e}",
            self.identical, self.n, self.worst
        );
        eprintln!("{s}");
        s
    }
}

fn prior_of(spec: &Value) -> IndependentPrior {
    let marginals = spec
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let a = num(&p[1]);
            let b = num(&p[2]);
            match p[0].as_str().unwrap() {
                "uniform" => Distribution::Uniform(Uniform::new(a, b).unwrap()),
                _ => Distribution::Normal(Normal::new(a, b).unwrap()),
            }
        })
        .collect();
    IndependentPrior::new(marginals).unwrap()
}

/// **MCMC, replayed on RAVEN's own draws.** For each of the four recorded
/// chains the Rust sampler consumes RAVEN's proposal increments and
/// acceptance uniforms, and the likelihood closure returns, per proposed
/// point, the very value RAVEN's run computed (looked up by the step, so the
/// comparison is of the sampler, not of the likelihood's arithmetic).
///
/// Compared per step: the proposed point and the current state (P2,
/// bit-identical predicted), the decision (P1), `alpha` (P3, absolute),
/// acceptance rate and scaling (P4), adaptive `lambda` (P5), and at the end
/// the ensemble mean and covariance (P5).
#[test]
fn mcmc_matches_raven_step_for_step() {
    let r = reference("mcmc.json");
    let (mut states, mut alpha, mut rates, mut lam, mut ens) = (
        Tally::default(),
        Tally::default(),
        Tally::default(),
        Tally::default(),
        Tally::default(),
    );
    let mut decisions = (0usize, 0usize);
    for case in r["cases"].as_array().unwrap() {
        let steps = case["steps"].as_array().unwrap();
        let prior = prior_of(&case["priors"]);
        let kind = if case["kind"] == "metropolis" {
            RavenMcmcKind::Metropolis
        } else {
            RavenMcmcKind::AdaptiveMetropolis {
                adaptive_interval: case["adaptive_interval"].as_u64().unwrap() as usize,
            }
        };
        let config = RavenMcmcConfig {
            kind,
            limit: case["limit"].as_u64().unwrap() as usize,
            burn_in: case["burn_in"].as_u64().unwrap() as usize,
            tune: case["tune"].as_bool().unwrap(),
            tune_interval: case["tune_interval"].as_u64().unwrap() as usize,
            initial: Some(vec1(&case["initial"])),
            likelihood: if case["log_likelihood_form"].as_bool().unwrap() {
                LikelihoodForm::Log
            } else {
                LikelihoodForm::Linear
            },
            bounds: BoundHandling::ClampAsRaven,
        };
        let increments: Vec<Vec<f64>> = steps[1..].iter().map(|s| vec1(&s["increment"])).collect();
        let uniforms: Vec<f64> = steps[1..].iter().map(|s| num(&s["uniform"])).collect();
        // RAVEN's likelihood value at each evaluated point, keyed by the
        // point's bits: the proposals are compared separately, so a lookup
        // miss would itself be a failure.
        let table: Vec<(Vec<u64>, f64)> = steps
            .iter()
            .map(|s| {
                (
                    vec1(&s["proposed"]).iter().map(|v| v.to_bits()).collect(),
                    num(&s["likelihood"]),
                )
            })
            .collect();
        let ll = |x: &[f64]| -> f64 {
            let key: Vec<u64> = x.iter().map(|v| v.to_bits()).collect();
            table
                .iter()
                .find(|(k, _)| *k == key)
                .map_or(f64::NAN, |(_, v)| *v)
        };
        let out = raven_mcmc(
            &prior,
            &config,
            &ll,
            Draws::Replay {
                increments,
                uniforms,
            },
        )
        .unwrap();
        assert_eq!(out.nan_log_ratios, 0, "{}: a likelihood lookup missed", case["name"]);
        for (ours, theirs) in out.steps.iter().zip(steps) {
            for (a, b) in ours.proposed.iter().zip(vec1(&theirs["proposed"])) {
                states.rel(*a, b);
            }
            for (a, b) in ours.current.iter().zip(vec1(&theirs["current"])) {
                states.rel(*a, b);
            }
            rates.rel(ours.scaling, num(&theirs["scaling"]));
            if let Some(a) = ours.alpha {
                alpha.abs(a, num(&theirs["alpha"]));
                rates.rel(ours.accept_rate, num(&theirs["accept_rate"]));
                decisions.0 += 1;
                if ours.accepted == theirs["accepted"].as_bool().unwrap() {
                    decisions.1 += 1;
                }
            }
            if let Some(l) = ours.lambda {
                lam.rel(l, num(&theirs["lambda"]));
            }
        }
        if let (Some(m), Some(c)) = (&out.ensemble_mean, &out.ensemble_cov) {
            for (a, b) in m.iter().zip(vec1(&case["ensemble_mean"])) {
                ens.rel(*a, b);
            }
            for (ra, rb) in c.iter().zip(vec2(&case["ensemble_cov"])) {
                for (a, b) in ra.iter().zip(rb) {
                    ens.rel(*a, b);
                }
            }
        }
    }
    eprintln!("MCMC decisions: {}/{} identical", decisions.1, decisions.0);
    states.report("MCMC proposed + current states");
    alpha.report("MCMC alpha (absolute)");
    rates.report("MCMC acceptance rate + scaling");
    lam.report("adaptive lambda");
    ens.report("adaptive ensemble mean + covariance");
    assert_eq!(decisions.0, decisions.1);
    assert_eq!(states.identical, states.n, "P2: states bit-identical");
    assert!(alpha.worst <= 1e-12, "P3");
    assert_eq!(rates.identical, rates.n, "P4");
    assert!(lam.worst <= 1e-12 && ens.worst <= 1e-12, "P5");
}

use raffles::surrogate::polynomial_chaos::special::{
    eval_hermitenorm, eval_legendre, gauss_rule, GaussRule,
};
use raffles::surrogate::polynomial_chaos::{
    IndexSetKind, PceVariable, PolynomialChaos, PolynomialChaosSpec, SparseGridKind,
};
use raffles::surrogate::{GaussianProcess, GpConfig, GpKernel, GpOptimizer, RavenKernel};

/// **Gauss rules and polynomials against SciPy** (P6, P7): nodes and weights
/// of `roots_legendre` / `roots_hermitenorm` for n = 1..20, relative to the
/// largest node or weight of the rule (a node at 0 has no relative scale);
/// `eval_legendre` / `eval_hermitenorm` for n = 0..12 at ten points,
/// including the `|x| < 1e-5` Legendre series.
#[test]
fn gauss_rules_and_polynomials_match_scipy() {
    let r = reference("quadrature.json");
    let (mut nodes, mut weights, mut polys) = (Tally::default(), Tally::default(), Tally::default());
    for row in r["roots"].as_array().unwrap() {
        let n = row["n"].as_u64().unwrap() as usize;
        for (rule, xk, wk) in [
            (GaussRule::Legendre, "legendre_x", "legendre_w"),
            (GaussRule::HermiteNorm, "hermitenorm_x", "hermitenorm_w"),
        ] {
            let (x, w) = gauss_rule(rule, n).unwrap();
            let (xr, wr) = (vec1(&row[xk]), vec1(&row[wk]));
            let xs = xr.iter().fold(0.0_f64, |a, b| a.max(b.abs())).max(1.0);
            let ws = wr.iter().fold(0.0_f64, |a, b| a.max(b.abs()));
            for (a, b) in x.iter().zip(&xr) {
                nodes.abs(a / xs, b / xs);
            }
            for (a, b) in w.iter().zip(&wr) {
                weights.abs(a / ws, b / ws);
            }
        }
    }
    let pts = vec1(&r["eval_points"]);
    for row in r["evals"].as_array().unwrap() {
        let n = row["n"].as_u64().unwrap() as usize;
        for (p, (l, h)) in pts.iter().zip(vec1(&row["legendre"]).iter().zip(vec1(&row["hermitenorm"]))) {
            polys.rel(eval_legendre(n, *p), *l);
            polys.rel(eval_hermitenorm(n, *p), h);
        }
    }
    nodes.report("Gauss nodes (relative to the rule's largest)");
    weights.report("Gauss weights (relative to the rule's largest)");
    polys.report("eval_legendre + eval_hermitenorm");
    // Gate: the code-to-code criterion (V&V record); P6/P7 are scored there.
    assert!(nodes.worst < 1e-8 && weights.worst < 1e-8, "criterion");
    assert_eq!(polys.identical, polys.n, "P7 recurrence, bit-identical");
}

fn pce_spec(case: &Value) -> PolynomialChaosSpec {
    let variables = case["priors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let (a, b) = (num(&p[1]), num(&p[2]));
            match p[0].as_str().unwrap() {
                "uniform" => PceVariable::Uniform { lower: a, upper: b },
                _ => PceVariable::Normal { mean: a, std: b },
            }
        })
        .collect();
    PolynomialChaosSpec {
        variables,
        index_set: match case["index_set"].as_str().unwrap() {
            "TensorProduct" => IndexSetKind::TensorProduct,
            "TotalDegree" => IndexSetKind::TotalDegree,
            _ => IndexSetKind::HyperbolicCross,
        },
        order: case["order"].as_u64().unwrap() as usize,
        grid: if case["grid"] == "smolyak" {
            SparseGridKind::Smolyak
        } else {
            SparseGridKind::Tensor
        },
        importance: Some(vec1(&case["importance"])),
    }
}

/// **GaussPolynomialRom against RAVEN** (P8–P10). Per case: the index set
/// (exact), the grid points and weights in RAVEN's order, then the ROM
/// trained on RAVEN's own training outputs (so the comparison is of the
/// ROM, not of the test function's arithmetic): coefficients, mean,
/// variance, Sobol and total indices, partial variances and predictions.
/// Coefficients, Sobol indices and partial variances are compared
/// absolutely on the case's scale (the largest coefficient, 1, the variance),
/// because many are zero in exact arithmetic.
#[test]
fn polynomial_chaos_matches_raven() {
    let r = reference("pce.json");
    let (mut pts, mut wts, mut coef, mut moments, mut sens, mut preds) = (
        Tally::default(),
        Tally::default(),
        Tally::default(),
        Tally::default(),
        Tally::default(),
        Tally::default(),
    );
    for case in r["cases"].as_array().unwrap() {
        let spec = pce_spec(case);
        let set = spec.index_points().unwrap();
        let theirs: Vec<Vec<usize>> = case["index_points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p.as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as usize).collect())
            .collect();
        assert_eq!(set, theirs, "P8 {}", case["name"]);
        let g = spec.grid().unwrap();
        let gp = vec2(&case["grid_points"]);
        assert_eq!(g.points.len(), gp.len(), "{}", case["name"]);
        for (a, b) in g.points.iter().zip(&gp) {
            for (u, v) in a.iter().zip(b) {
                pts.rel(*u, *v);
            }
        }
        let gw = vec1(&case["grid_weights"]);
        let wscale = gw.iter().fold(0.0_f64, |a, b| a.max(b.abs()));
        for (a, b) in g.weights.iter().zip(&gw) {
            wts.abs(a / wscale, b / wscale);
        }
        // Train on RAVEN's grid and RAVEN's outputs.
        let y: Vec<Vec<f64>> = vec1(&case["training_y"]).into_iter().map(|v| vec![v]).collect();
        let pce = PolynomialChaos::fit(&spec, &gp, &y).unwrap();
        let tc: Vec<f64> = case["coefficients"].as_array().unwrap().iter().map(|e| num(&e[1])).collect();
        let cscale = tc.iter().fold(0.0_f64, |a, b| a.max(b.abs()));
        for (a, b) in pce.coefficients(0).iter().zip(&tc) {
            coef.abs(a / cscale, b / cscale);
        }
        moments.rel(pce.mean(0), num(&case["mean"]));
        moments.rel(pce.variance(0), num(&case["variance"]));
        let var = num(&case["variance"]);
        let ours = pce.sobol(0);
        let sob = case["sobol"].as_array().unwrap();
        let part = case["partial_variances"].as_array().unwrap();
        assert_eq!(ours.len(), sob.len());
        for ((s, idx, p), (ts, tp)) in ours.iter().zip(sob.iter().zip(part)) {
            let subset: Vec<usize> = ts[0].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as usize).collect();
            assert_eq!(*s, subset);
            sens.abs(*idx, num(&ts[1]));
            sens.abs(p / var, num(&tp[1]) / var);
        }
        for ((_, t), tt) in pce.total_sobol(0).iter().zip(case["total_sobol"].as_array().unwrap()) {
            sens.abs(*t, num(&tt[1]));
        }
        for (x, p) in vec2(&case["test_points"]).iter().zip(vec1(&case["predictions"])) {
            preds.rel(pce.predict(x).unwrap()[0], p);
        }
    }
    pts.report("PCE grid points");
    wts.report("PCE grid weights (relative to the largest)");
    coef.report("PCE coefficients (relative to the largest)");
    moments.report("PCE mean + variance");
    sens.report("PCE Sobol, total, partial/variance (absolute)");
    preds.report("PCE predictions");
    // Gate: the code-to-code criterion; P9/P10 are scored in the V&V record.
    for t in [&pts, &wts, &coef, &moments, &sens, &preds] {
        assert!(t.worst < 1e-8, "criterion");
    }
}

fn gp_config(case: &Value) -> GpConfig {
    let d = case["x"][0].as_array().unwrap().len();
    let aniso = case["anisotropic"].as_bool().unwrap();
    let kernel = match (case["kernel_name"].as_str(), case["custom"].as_str()) {
        (None, _) => GpKernel::raven_default(),
        (Some("RBF"), _) => GpKernel::raven(RavenKernel::Rbf, d, aniso),
        (Some("Matern"), _) => GpKernel::raven(RavenKernel::Matern, d, aniso),
        (Some("Custom"), Some("Constant*RBF")) => GpKernel::raven(RavenKernel::ConstantRbf, d, aniso),
        (Some("Custom"), Some("Constant*RBF+WhiteNoise")) => {
            GpKernel::raven(RavenKernel::ConstantRbfWhite, d, aniso)
        }
        other => panic!("kernel {other:?}"),
    };
    // Start from the reference's initial theta (the fixed cases set it).
    let kernel = match &case["kernel_initial"] {
        Value::Null => kernel,
        k => kernel.with_theta(&vec1(&k["theta"])).unwrap(),
    };
    GpConfig {
        kernel,
        alpha: num(&case["alpha"]),
        normalize_y: case["normalize_y"].as_bool().unwrap(),
        optimizer: if case["optimizer"].is_null() {
            GpOptimizer::None
        } else {
            GpOptimizer::MaximiseLml
        },
    }
}

/// **GP numerics at fixed hyperparameters** (P11–P13): the four cases whose
/// hyperparameters cannot move (RAVEN's default kernel, and three with
/// `optimizer = None`). Mean relative to max abs(y); std absolute, on unit
/// prior variance (P12); LML relative; gradient relative to its largest
/// component.
#[test]
fn gp_numerics_match_sklearn_at_fixed_hyperparameters() {
    let r = reference("gp.json");
    let (mut mean, mut sd, mut lml, mut grad) =
        (Tally::default(), Tally::default(), Tally::default(), Tally::default());
    for case in r["cases"].as_array().unwrap() {
        let fixed = case["optimizer"].is_null() || case["kernel_name"].is_null();
        if !fixed {
            continue;
        }
        let x = vec2(&case["x"]);
        let y = vec1(&case["y"]);
        let ys = y.iter().fold(0.0_f64, |a, b| a.max(b.abs()));
        let gp = GaussianProcess::fit(&gp_config(case), &x, &y).unwrap();
        for ((t, m), s) in vec2(&case["test"]).iter().zip(vec1(&case["mean"])).zip(vec1(&case["std"])) {
            let (pm, ps) = gp.predict(t).unwrap();
            mean.abs(pm / ys, m / ys);
            sd.abs(ps, s);
        }
        lml.rel(gp.log_marginal_likelihood(), num(&case["lml"]));
        let theta = gp.kernel().theta();
        if !theta.is_empty() {
            let (_, g) = gp.log_marginal_likelihood_at(&theta).unwrap().unwrap();
            let tg = vec1(&case["lml_gradient"]);
            let gs = tg.iter().fold(0.0_f64, |a, b| a.max(b.abs()));
            for (a, b) in g.iter().zip(&tg) {
                grad.abs(a / gs, b / gs);
            }
        }
    }
    mean.report("GP mean, fixed hyperparameters (relative to max |y|)");
    sd.report("GP std, fixed hyperparameters (absolute)");
    lml.report("GP LML, fixed hyperparameters");
    grad.report("GP LML gradient (relative to its largest)");
    // Gate: the code-to-code criterion; P11-P13 are scored in the V&V record.
    for t in [&mean, &sd, &lml, &grad] {
        assert!(t.worst < 1e-8, "criterion");
    }
}

/// **GP fit against sklearn's L-BFGS-B** (P14, P15): the two cases whose
/// hyperparameters are optimised. Reported, per case: the fitted theta, the
/// LML at each optimum, and the prediction gap.
#[test]
fn gp_fit_reaches_sklearns_optimum() {
    let r = reference("gp.json");
    for case in r["cases"].as_array().unwrap() {
        if case["optimizer"].is_null() || case["kernel_name"].is_null() {
            continue;
        }
        let x = vec2(&case["x"]);
        let y = vec1(&case["y"]);
        let gp = GaussianProcess::fit(&gp_config(case), &x, &y).unwrap();
        let ours = gp.kernel().theta();
        let theirs = vec1(&case["kernel_fitted"]["theta"]);
        let lml_t = num(&case["lml"]);
        let lml_o = gp.log_marginal_likelihood();
        let ys = y.iter().fold(0.0_f64, |a, b| a.max(b.abs()));
        let mut gap: f64 = 0.0;
        for (t, m) in vec2(&case["test"]).iter().zip(vec1(&case["mean"])) {
            gap = gap.max((gp.predict(t).unwrap().0 - m).abs() / ys);
        }
        let exp = |v: &[f64]| v.iter().map(|t| t.exp()).collect::<Vec<_>>();
        let at_theirs = gp.log_marginal_likelihood_at(&theirs).unwrap();
        let at_ours = gp.log_marginal_likelihood_at(&ours).unwrap();
        eprintln!("  our LML and gradient at sklearn's theta: {at_theirs:?}; at ours: {at_ours:?}");
        eprintln!(
            "GP fit {}: ours {:?} (LML {lml_o}), sklearn {:?} (LML {lml_t}), rel LML gap {:.3e}, max mean gap {gap:.3e}",
            case["name"],
            exp(&ours),
            exp(&theirs),
            (lml_o - lml_t) / lml_t.abs()
        );
        if case["name"] == "constant_rbf_anisotropic_fit_2d" {
            // P15 missed, and the case cannot test a fit: amplitude on its
            // 1e5 bound over alpha 1e-10 makes the Gram matrix singular to
            // working precision, so the two codes' LML at the SAME theta
            // disagree far beyond round-off of a conditioned problem.
            let (ours_at_theirs, _) = at_theirs.unwrap();
            assert!(((ours_at_theirs - lml_t) / lml_t).abs() > 1e-3);
            assert!(gp.kernel().amplitude.unwrap().value / num(&case["alpha"]) > 1e14);
        } else {
            // P14 (rbf_fit_1d) and P16 (matern_fit_2d).
            for (o, t) in ours.iter().zip(&theirs) {
                assert!(((o - t).exp() - 1.0).abs() < 1e-4, "length scale");
            }
            assert!(((lml_o - lml_t) / lml_t).abs() < 1e-8, "LML");
        }
    }
}
