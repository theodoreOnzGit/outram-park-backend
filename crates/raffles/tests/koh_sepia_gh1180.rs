//! Differential tests of the SEPIA Kennedy–O'Hagan port (gh:#1180) against
//! upstream SEPIA's own outputs, committed in
//! `verification_and_validation/koh_sepia_gh1180/references/sepia_koh.json`
//! by `generate_references.py` (SEPIA `309890423a12`, numpy 1.26.4, scipy
//! 1.10.1). Methodology, predictions D1–D7 and results:
//! `verification_and_validation/koh_calibration_gh1180.md`.

use raffles::bayesian::koh::{pairwise_sum, KohData, KohForm, KohFree, KohModel, KohParams, KohPriors, KohSampleConfig};
use serde_json::Value;

fn reference() -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/koh_sepia_gh1180/references/sepia_koh.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

fn flat(v: &Value) -> Vec<f64> {
    match v {
        Value::Array(a) => a.iter().flat_map(flat).collect(),
        Value::Number(n) => vec![n.as_f64().unwrap()],
        _ => panic!("not numeric: {v}"),
    }
}

fn rows(v: &Value) -> Vec<Vec<f64>> {
    v.as_array().unwrap().iter().map(flat).collect()
}

fn data(r: &Value, with_obs: bool) -> KohData {
    let i = &r["inputs"];
    let (xo, yo) = if with_obs { (rows(&i["x_obs"]), flat(&i["y_obs"])) } else { (Vec::new(), Vec::new()) };
    KohData::new(&rows(&i["x_sim"]), &rows(&i["t_sim"]), &flat(&i["y_sim"]), &xo, &yo).unwrap()
}

fn params(v: &Value, form: KohForm) -> KohParams {
    let get = |k: &str| v.get(k).map(flat).unwrap_or_default();
    let s = |k: &str| get(k).first().copied().unwrap_or(f64::NAN);
    KohParams {
        theta: get("theta"),
        beta_u: get("betaU"),
        lam_uz: s("lamUz"),
        lam_ws: s("lamWs"),
        lam_wos: s("lamWOs"),
        beta_v: get("betaV"),
        lam_vz: if form == KohForm::WithDiscrepancy { s("lamVz") } else { 20.0 },
        lam_os: if form == KohForm::SimulationOnly { 1000.0 } else { s("lamOs") },
    }
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1e-300)
}

/// **D1.** SEPIA's transforms of the toy data (40 runs, 10 observations):
/// unit-cube x and t, the standardisation's mean and `ddof = 1` standard
/// deviation, `u`, `w`.
///
/// **Prediction:** bit-identical. **Result (2026-11-04):** bit-identical in
/// every number (asserted with `==`), once numpy's zero-started pairwise sum
/// was reproduced (`pairwise_sum`, itself checked on hand-made sums below).
#[test]
fn d1_transforms_are_bit_identical() {
    let r = reference();
    let t = &r["transforms"];
    let d = data(&r, true);
    assert_eq!(d.y_mean, t["orig_y_mean"].as_f64().unwrap());
    assert_eq!(d.y_sd, t["orig_y_sd"].as_f64().unwrap());
    assert_eq!(d.x_min, flat(&t["orig_x_min"]));
    assert_eq!(d.x_max, flat(&t["orig_x_max"]));
    assert_eq!(d.t_min, flat(&t["orig_t_min"]));
    assert_eq!(d.t_max, flat(&t["orig_t_max"]));
    assert_eq!(d.zt, rows(&t["zt"]));
    assert_eq!(d.x_obs, rows(&t["x_obs_trans"]));
    assert_eq!(d.u, flat(&t["u"]));
    assert_eq!(d.w, flat(&t["w"]));
    assert_eq!((d.m(), d.n(), d.p, d.q), (40, 10, 1, 2));
    // SigObs = inv(I + 1e-8 I): its diagonal, as SEPIA stores it.
    assert!(flat(&t["sig_obs_diag"]).iter().all(|v| *v == 1.0 / (1.0 + 1e-8)));
    // The unit maps, both ways.
    let tu = d.unit_t(&[0.3, 0.7]);
    let back = d.native_t(&tu);
    assert!((back[0] - 0.3).abs() < 1e-15 && (back[1] - 0.7).abs() < 1e-15);
    assert_eq!(d.unit_x(&[d.x_max[0]]), vec![1.0]);
    // pairwise_sum: the three regimes.
    assert_eq!(pairwise_sum(&[1.0, 2.0, 3.0]), 6.0);
    let v: Vec<f64> = (1..=20).map(f64::from).collect();
    assert_eq!(pairwise_sum(&v), 210.0);
    let w: Vec<f64> = (1..=300).map(f64::from).collect();
    assert_eq!(pairwise_sum(&w), 45150.0);
    // Refusals.
    assert!(KohData::new(&[vec![0.0]], &[vec![0.0]], &[1.0], &[], &[]).is_err());
    assert!(KohData::new(&[vec![0.0], vec![1.0]], &[vec![0.0]], &[1.0, 2.0], &[], &[]).is_err());
    assert!(KohModel::new(data(&r, true), KohForm::SimulationOnly, KohPriors::sepia_defaults(2)).is_err());
}

/// **D2, D3, D4.** Log-likelihood, log prior and log posterior at five
/// parameter sets in each of SEPIA's three forms, against upstream; and the
/// independent dense check (the joint MVN density of (u, w), not SEPIA).
///
/// **Predictions:** log-likelihood < 1e-10 relative, not bit-identical (D2);
/// log prior bit-identical, including the ρ cap of set 5 (D3); the dense
/// identity holds upstream to < 1e-9 and the port is within 1e-10 of both
/// (D4).
///
/// **Result (2026-11-04):** log-likelihood and log posterior within
/// **6.8e-10** relative of SEPIA (no_d set 4; every with_d and sim_only set
/// below 2.6e-11): **D2 missed** its 1e-10 at no_d sets 1 (1.4e-10) and 4.
/// The cause is SEPIA's own round-off, not the port's: at set 4 SEPIA is
/// 5.9e-10 from the independent dense density and the port 9.7e-11 (worst
/// port-vs-dense over all sets 9.7e-11), so the gap is upstream's LU
/// inverse of an ill-conditioned `SigW` (λ_WOs, λ_Ws large). Within the
/// workspace's 1e-8 code-to-code allowance, asserted at 1e-8. **D3 missed:**
/// the log prior is bit-identical at 9 of 15 sets and within 1e-14 at the
/// rest, because numpy 1.26.4's vectorised `exp`/`log` differ from glibc's
/// by an ulp on 20–24 % of arguments (measured on 200 000 each). **D4 met**
/// (SEPIA-vs-dense 5.9e-10 < 1e-9).
#[test]
fn d2_d4_log_likelihood_prior_and_posterior_match_sepia() {
    let r = reference();
    let mut worst = (0.0_f64, 0.0_f64, 0.0_f64);
    let mut bits = (0usize, 0usize);
    for (kind, form) in [("no_d", KohForm::NoDiscrepancy), ("with_d", KohForm::WithDiscrepancy), ("sim_only", KohForm::SimulationOnly)] {
        let q = 2;
        let model = KohModel::new(data(&r, form != KohForm::SimulationOnly), form, KohPriors::sepia_defaults(q)).unwrap();
        for (i, c) in r["log_lik"][kind].as_array().unwrap().iter().enumerate() {
            let p = params(&c["params"], form);
            let ll = model.log_lik(&p);
            let lp = model.log_prior(&p);
            let lpost = model.log_post(&p);
            let (sl, sp, spost) = (c["log_lik"].as_f64().unwrap(), c["log_prior"].as_f64().unwrap(), c["log_post"].as_f64().unwrap());
            let (e1, e2) = (rel(ll, sl), rel(lpost, spost));
            println!("{kind} set {}: log_lik {ll:.15e} vs SEPIA {sl:.15e} (rel {e1:.2e}); log_prior bits equal: {}; log_post rel {e2:.2e}", i + 1, lp == sp);
            worst.0 = worst.0.max(e1).max(e2);
            bits.1 += 1;
            if lp == sp {
                bits.0 += 1;
            }
            assert!(e1 < 1e-8 && e2 < 1e-8, "{kind} set {}", i + 1);
            assert!(rel(lp, sp) < 1e-14, "{kind} set {} prior {lp} vs {sp}", i + 1);
            if let Some(dense) = c.get("dense_log_lik").and_then(Value::as_f64) {
                println!("    dense: SEPIA rel {:.2e}, port rel {:.2e}", rel(sl, dense), rel(ll, dense));
                worst.1 = worst.1.max(rel(sl, dense));
                worst.2 = worst.2.max(rel(ll, dense));
                assert!(rel(sl, dense) < 1e-9 && rel(ll, dense) < 1e-9);
            }
        }
    }
    println!("worst port-vs-SEPIA rel {:.2e}; log prior bit-identical {}/{}; SEPIA-vs-dense {:.2e}; port-vs-dense {:.2e}", worst.0, bits.0, bits.1, worst.1, worst.2);
    // An emulator factor built at other hyperparameters is refused, not used.
    let model = KohModel::new(data(&r, true), KohForm::WithDiscrepancy, KohPriors::sepia_defaults(2)).unwrap();
    let p0 = KohParams::sepia_init(KohForm::WithDiscrepancy, 1, 2);
    let f = model.emulator_factor(&p0).unwrap();
    assert_eq!(model.log_lik_with(&p0, &f), model.log_lik(&p0));
    let mut p1 = p0.clone();
    p1.lam_uz = 2.0;
    assert_eq!(model.log_lik_with(&p1, &f), f64::NEG_INFINITY);
    let mut out = p0.clone();
    out.theta[0] = 1.5;
    assert_eq!(model.log_post(&out), f64::NEG_INFINITY);
}

/// **D5, D6.** The predictive mean and covariance of (v, u) at x = 0, 0.27,
/// 0.5, 0.83, 1 for three parameter samples: against SEPIA's `uvPredSep`
/// (no discrepancy, upstream's default predictor) and its dense `uvPred`
/// (both forms; with δ, upstream's one block defect corrected as its own
/// comment says).
///
/// **Predictions:** mean < 1e-8 relative, covariance < 1e-8 of the largest
/// diagonal entry (D5); `uvPred` against `uvPredSep` < 1e-9 (D6).
/// **Result (2026-11-04):** met. Mean worst 5.0e-13 (no δ), 5.5e-13 (with
/// δ); covariance worst 1.0e-12 / 2.6e-13 of the largest diagonal entry;
/// upstream `uvPred` against `uvPredSep` 4.9e-13. `uvPredSep` with δ fails
/// upstream (`ValueError`, asserted).
#[test]
fn d5_d6_predictions_match_sepia() {
    let r = reference();
    let pr = &r["prediction"];
    let x_pred = rows(&r["inputs"]["x_pred"]);
    let mut worst_sep = 0.0_f64;
    for (kind, form) in [("no_d", KohForm::NoDiscrepancy), ("with_d", KohForm::WithDiscrepancy)] {
        let model = KohModel::new(data(&r, true), form, KohPriors::sepia_defaults(2)).unwrap();
        let xp: Vec<Vec<f64>> = x_pred.iter().map(|x| model.data.unit_x(x)).collect();
        let mus = pr[kind]["uvpred_mu"].as_array().unwrap();
        let sigmas = pr[kind]["uvpred_sigma"].as_array().unwrap();
        let (mut em, mut es) = (0.0_f64, 0.0_f64);
        for (s, sample) in pr["samples"].as_array().unwrap().iter().enumerate() {
            let p = params(sample, form);
            let got = model.predict(&p, &xp).unwrap();
            let mu = flat(&mus[s]);
            let sig = rows(&sigmas[s]);
            let scale = (0..sig.len()).map(|i| sig[i][i].abs()).fold(0.0, f64::max);
            for i in 0..mu.len() {
                em = em.max((got.mean[i] - mu[i]).abs() / mu[i].abs().max(1e-12));
                for j in 0..mu.len() {
                    es = es.max((got.cov[i][j] - sig[i][j]).abs() / scale);
                }
            }
            if kind == "no_d" {
                let sm = flat(&pr[kind]["uvpredsep_mu"].as_array().unwrap()[s]);
                worst_sep = worst_sep.max(mu.iter().zip(&sm).map(|(a, b)| (a - b).abs() / b.abs()).fold(0.0, f64::max));
            }
        }
        println!("{kind}: predictive mean worst rel {em:.2e}; covariance worst / max diag {es:.2e}");
        assert!(em < 1e-8 && es < 1e-8, "{kind}");
    }
    println!("upstream uvPred vs uvPredSep (no_d) mean worst rel {worst_sep:.2e}");
    assert!(worst_sep < 1e-9);
    let msg = pr["with_d"]["uvpredsep_status"].as_str().unwrap();
    assert!(msg.starts_with("ValueError"), "{msg}");
    let sim = KohModel::new(data(&r, false), KohForm::SimulationOnly, KohPriors::sepia_defaults(2)).unwrap();
    assert!(sim.predict(&KohParams::sepia_init(KohForm::SimulationOnly, 1, 2), &x_pred).is_err());
}

/// **D7.** Sampler, compared statistically: raffles' TMCMC (population
/// 2 000, 20 moves per stage, seed 1180) on the WITH_D toy posterior, all 11
/// parameters free, against SEPIA's own MCMC (12 000 steps, 2 000 burn,
/// seed 1180), with the criterion fixed before the run (V&V record §3):
/// |Δmean| ≤ 3 √(se_S² + se_R²) per θ, se_R = sd_R √(5/N), and the sd ratio
/// in [0.8, 1.25].
///
/// **First run MISSED (2026-11-04), and the protocol was fixed, not the
/// criterion:** with 3 moves per stage, θ₁ = 0.3615 against SEPIA's 0.3823
/// (|Δ| 0.0208 against a bound of 0.0058). A diagnostic over seeds and move
/// counts showed the population's mean log posterior still rising with the
/// move count (101.1, 104.4 at 3 moves; 107.8, 106.9 at 10; 108.9 at 20 with
/// population 4 000): the Metropolis moves had not equilibrated the sharp
/// precisions in 11 dimensions. At 20 moves θ₁ = 0.3816, θ₂ = 0.6977.
///
/// **Result (2026-11-04), 20 moves:** met. θ₁ 0.38416 ± 0.03093 against
/// SEPIA 0.38232 ± 0.03082 (|Δ| 0.0019, bound 0.0054, sd ratio 1.004); θ₂
/// 0.69681 ± 0.01664 against 0.69755 ± 0.01705 (|Δ| 0.0007, bound 0.0030,
/// ratio 0.976). 12 stages, 52 s. (Truth of the toy: θ = 0.3, 0.7; the
/// θ₁ offset is the δ/θ confounding, shared by both samplers.)
#[test]
fn d7_tmcmc_samples_sepias_posterior() {
    let r = reference();
    let model = KohModel::new(data(&r, true), KohForm::WithDiscrepancy, KohPriors::sepia_defaults(2)).unwrap();
    let start = KohParams::sepia_init(KohForm::WithDiscrepancy, 1, 2);
    let cfg = KohSampleConfig { free: KohFree::All, population: 2000, chain_length: 20, seed: 1180 };
    let t0 = std::time::Instant::now();
    let post = model.sample(&start, &cfg).unwrap();
    let n = post.samples.len() as f64;
    let s = &r["mcmc"]["summary"];
    println!("TMCMC: {} stages, {:.1} s", post.stages, t0.elapsed().as_secs_f64());
    for k in 0..2 {
        let v: Vec<f64> = post.samples.iter().map(|p| p.theta[k]).collect();
        let m = v.iter().sum::<f64>() / n;
        let sd = (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0)).sqrt();
        let (sm, ssd, sse) = (flat(&s["theta"]["mean"])[k], flat(&s["theta"]["sd"])[k], flat(&s["theta"]["batch_means_se"])[k]);
        let se_r = sd * (5.0 / n).sqrt();
        let bound = 3.0 * (sse * sse + se_r * se_r).sqrt();
        println!("theta_{}: TMCMC {m:.5} ± {sd:.5}; SEPIA {sm:.5} ± {ssd:.5}; |Δmean| {:.5} vs bound {bound:.5}; sd ratio {:.3}", k + 1, (m - sm).abs(), sd / ssd);
        assert!((m - sm).abs() <= bound, "theta_{}", k + 1);
        assert!((0.8..=1.25).contains(&(sd / ssd)), "theta_{} sd ratio", k + 1);
    }
    let best = post.best();
    assert!(post.log_post.iter().all(|v| *v <= model.log_post(best)));
    // The plug-in route: the emulator held at the best sample.
    let cfg = KohSampleConfig { free: KohFree::CalibrationOnly, population: 500, chain_length: 2, seed: 7 };
    let modular = model.sample(best, &cfg).unwrap();
    assert!(modular.samples.iter().all(|p| p.beta_u == best.beta_u && p.lam_ws == best.lam_ws));
    let sim = KohModel::new(data(&r, false), KohForm::SimulationOnly, KohPriors::sepia_defaults(2)).unwrap();
    assert!(sim.sample(&KohParams::sepia_init(KohForm::SimulationOnly, 1, 2), &cfg).is_err());
}

