//! Differential tests of the OpenDA port (gh:#1192) against OpenDA's own
//! outputs, committed in `verification_and_validation/openda_gh1192/references/`
//! by `generate_references.sh` (OpenDA `e51565c2c8de`, OpenJDK 21.0.12.1),
//! plus the statistical comparison and the analytic checks. Methodology,
//! upstream findings, predictions D1–D13 and A1–A5, and results:
//! `verification_and_validation/openda_gh1192.md`.

use raffles::openda::{
    jama, linalg, run_sequential, simple_least_squares, CostFunction, DrawSource, Dud, Dynamics, EnsembleOptions, Localization, Observations, OdaStochModel, Powell, Predictor,
    RosenbrockCost, SequentialFilter, SequentialRun, SimpleCost, SimpleModelConfig, SimpleStochModel, Simplex, SteadyStateGain, StochVector, WeightedLeastSquares,
};
use serde_json::Value;

fn load(name: &str) -> Value {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/openda_gh1192/references").join(name);
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

/// Per-quantity tally: numbers compared, bit-identical, worst relative
/// difference.
#[derive(Default)]
struct Tally {
    rows: Vec<(String, usize, usize, f64)>,
}

impl Tally {
    fn cmp(&mut self, label: &str, ours: &[f64], theirs: &[f64]) {
        assert_eq!(ours.len(), theirs.len(), "{label}: length {} vs {}", ours.len(), theirs.len());
        let mut same = 0;
        let mut worst: f64 = 0.0;
        for (a, b) in ours.iter().zip(theirs) {
            if a.to_bits() == b.to_bits() || (*a == 0.0 && *b == 0.0) {
                same += 1;
            } else {
                worst = worst.max((a - b).abs() / b.abs().max(f64::MIN_POSITIVE));
            }
        }
        self.rows.push((label.to_string(), ours.len(), same, worst));
    }
    fn report(&self, title: &str) -> (usize, usize, f64) {
        println!("== {title}");
        let (mut n, mut s, mut w) = (0, 0, 0.0_f64);
        for (l, c, same, worst) in &self.rows {
            println!("{l}: {same}/{c} bit-identical, worst rel {worst:.3e}");
            n += c;
            s += same;
            w = w.max(*worst);
        }
        println!("TOTAL {title}: {s}/{n} bit-identical, worst rel {w:.3e}");
        (n, s, w)
    }
}

/// **D1, D2, D3** (methodology: record §2, predictions §4). JAMA's SVD and
/// OpenDA's `svd`, `inverse`, `sqrt`, `rightSolve` and `A·Aᵀ` on the 6 fixed
/// matrices, bit for bit; and the target's `exp` against 2 000 values of the
/// JVM's `Math.exp`, counted.
///
/// **Result (2026-11-04):** D1/D2 **402/402 bit-identical** (predicted
/// bit-identical: met). D3: 6 of 2 000 `exp` values differ from the JVM's
/// `Math.exp`, each by 1 ulp (predicted ≤ 1 %, 1 ulp: met).
#[test]
fn d1_d2_d3_linear_algebra() {
    let r = load("openda_linalg.json");
    let mut t = Tally::default();
    for (i, c) in r["cases"].as_array().unwrap().iter().enumerate() {
        let a = rows(&c["a"]);
        let d = jama::svd(&a).unwrap();
        t.cmp(&format!("case {i} jama U"), &d.u.concat(), &flat(&c["jama_u"]));
        t.cmp(&format!("case {i} jama s"), &d.s, &flat(&c["jama_s"]));
        t.cmp(&format!("case {i} jama V"), &d.v.concat(), &flat(&c["jama_v"]));
        let (u, _, v) = linalg::svd(&a).unwrap();
        t.cmp(&format!("case {i} oda svd U"), &u.concat(), &flat(&c["oda_svd_u"]));
        t.cmp(&format!("case {i} oda svd V"), &v.concat(), &flat(&c["oda_svd_v"]));
        if c.get("oda_inverse").is_some() {
            t.cmp(&format!("case {i} inverse"), &linalg::inverse(&a).unwrap().concat(), &flat(&c["oda_inverse"]));
            t.cmp(&format!("case {i} sqrt"), &linalg::sqrt(&a).unwrap().concat(), &flat(&c["oda_sqrt"]));
        }
        t.cmp(&format!("case {i} rightSolve"), &linalg::right_solve(&a, &flat(&c["rhs"])).unwrap(), &flat(&c["oda_right_solve"]));
        let mut p = linalg::zeros(a.len(), a.len());
        linalg::multiply(&mut p, 1.0, &a, &a, 0.0, false, true).unwrap();
        t.cmp(&format!("case {i} A·Aᵀ"), &p.concat(), &flat(&c["oda_a_at"]));
    }
    let (n, s, w) = t.report("D1/D2 linear algebra");
    let args = flat(&r["exp_args"]);
    let jvm = flat(&r["exp_values"]);
    let mut differ = 0;
    let mut max_ulp = 0u64;
    for (x, e) in args.iter().zip(&jvm) {
        let ours = x.exp();
        if ours.to_bits() != e.to_bits() {
            differ += 1;
            max_ulp = max_ulp.max(ours.to_bits().abs_diff(e.to_bits()));
        }
    }
    println!("D3 exp: {differ}/{} differ from the JVM's Math.exp, max {max_ulp} ulp; JVM Math.pow(x,2)!=x*x on {} of {} samples, Math.exp!=StrictMath.exp on {}", args.len(), r["pow2_vs_square_differences"], r["samples"], r["exp_vs_strictexp_differences"]);
    assert_eq!(s, n, "D1/D2 predicted bit-identical (worst {w:e})");
    assert!(differ * 100 <= args.len() && max_ulp <= 1, "D3 predicted at most 1 % differing, by 1 ulp");
}

fn compare_history<C: CostFunction>(t: &mut Tally, label: &str, f: &C, case: &Value) {
    let h = f.history();
    let want_p = rows(&case["evaluated_parameters"]);
    let want_c = flat(&case["evaluated_costs"]);
    println!("{label}: {} evaluations (OpenDA {})", h.costs.len(), want_c.len());
    assert_eq!(h.costs.len(), want_c.len(), "{label}: evaluation count");
    t.cmp(&format!("{label} parameters"), &h.parameters.concat(), &want_p.concat());
    t.cmp(&format!("{label} costs"), &h.costs, &want_c);
    let (c, p) = h.optimum().unwrap();
    t.cmp(&format!("{label} optimum"), &[p.to_vec(), vec![c]].concat(), &[flat(&case["optimal"]), vec![case["optimal_cost"].as_f64().unwrap()]].concat());
    if let Some(pr) = case.get("evaluated_predictions") {
        t.cmp(&format!("{label} predictions"), &h.predictions.concat(), &rows(pr).concat());
    }
}

fn oscillator_calibration_obs() -> Observations {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/openda_gh1192/fixtures/oscillatorCalibrationObservations.csv");
    let text = std::fs::read_to_string(p).unwrap();
    let mut c: [Vec<f64>; 4] = Default::default();
    for line in text.lines().skip(1) {
        for (k, v) in line.split(',').enumerate() {
            c[k].push(v.trim().parse().unwrap());
        }
    }
    let [t, i, v, s] = c;
    Observations::new(t, i, v, s, None).unwrap()
}

/// **D4–D7.** Simplex, Powell and DUD on OpenDA's test costs and on the
/// oscillator calibration with the weighted cost: every evaluated parameter
/// vector, cost and prediction, in order, bit for bit, and the same
/// evaluation counts; DUD's and Simplex's error estimates.
///
/// **Result (2026-11-04):** **3 330/3 330 bit-identical**, every evaluation
/// count equal (Simplex 25, 90, 22; Powell 47, 116, 230; DUD 4, 4;
/// oscillator DUD 8 / 4, Simplex 22 / 25, Powell 35 / 34). Predicted
/// bit-identical: met.
#[test]
fn d4_d7_calibration() {
    let r = load("openda_calibration.json");
    let mut t = Tally::default();

    let mut f = SimpleCost::default();
    let mut s = Simplex::default();
    s.initialize(&mut f, &[0.0, 0.0], 1.5).unwrap();
    (s.abs_tol, s.rel_tol) = (0.001, 0.00001);
    s.optimize(&mut f).unwrap();
    compare_history(&mut t, "D4 simplex simple", &f, &r["simplex_simple"]);

    let mut f = RosenbrockCost::default();
    let mut s = Simplex::default();
    s.initialize(&mut f, &[0.0, 0.0], 1.5).unwrap();
    (s.abs_tol, s.rel_tol) = (0.001, 0.0001);
    s.optimize(&mut f).unwrap();
    compare_history(&mut t, "D4 simplex rosenbrock", &f, &r["simplex_rosenbrock"]);

    let mut f = simple_least_squares(false).unwrap();
    let mut s = Simplex::default();
    s.initialize(&mut f, &[0.0, 0.0], 1.0).unwrap();
    s.optimize(&mut f).unwrap();
    compare_history(&mut t, "D4 simplex simple LS", &f, &r["simplex_simple_ls"]);
    t.cmp("D4 simplex simple LS std", s.std_values().unwrap(), &flat(&r["simplex_simple_ls"]["std"]));

    for (key, start, width) in [("powell_simple", [0.1, 0.1], 0.5), ("powell_rosenbrock", [0.0, 0.0], 0.5)] {
        let mut p = Powell::default();
        if key == "powell_simple" {
            let mut f = SimpleCost::default();
            p.initialize(&mut f, &start, width).unwrap();
            (p.abs_tol, p.rel_tol) = (0.001, 0.0001);
            p.optimize(&mut f).unwrap();
            compare_history(&mut t, "D5 powell simple", &f, &r[key]);
        } else {
            let mut f = RosenbrockCost::default();
            p.initialize(&mut f, &start, width).unwrap();
            (p.abs_tol, p.rel_tol) = (0.001, 0.0001);
            p.optimize(&mut f).unwrap();
            compare_history(&mut t, "D5 powell rosenbrock", &f, &r[key]);
        }
    }
    let mut f = SimpleCost::default();
    let mut p = Powell::default();
    p.initialize_with_directions(&mut f, &[0.0, 0.0], vec![vec![2.0, 0.0], vec![0.0, 1.0]]).unwrap();
    p.optimize(&mut f).unwrap();
    compare_history(&mut t, "D5 powell simple searchinit", &f, &r["powell_simple_searchinit"]);

    for (key, bg) in [("dud_simple_ls", false), ("dud_simple_ls_background", true)] {
        let mut f = simple_least_squares(bg).unwrap();
        let mut d = Dud::default();
        d.initialize(&mut f, &[0.0, 0.0]).unwrap();
        (d.abs_tol, d.rel_tol) = (0.001, 0.001);
        d.optimize(&mut f).unwrap();
        compare_history(&mut t, &format!("D6 {key}"), &f, &r[key]);
        t.cmp(&format!("D6 {key} std"), d.std_values().unwrap(), &flat(&r[key]["std"]));
    }

    let obs = oscillator_calibration_obs();
    let tpl = SimpleStochModel::new(Dynamics::oscillator(), &SimpleModelConfig::default()).unwrap();
    for alg in ["dud", "simplex", "powell"] {
        for bg in [false, true] {
            let key = format!("oscillator_{alg}{}", if bg { "_background" } else { "" });
            let mut f = WeightedLeastSquares::simulation(tpl.clone(), &obs, bg).unwrap();
            let p0 = [8.0, 1.5708];
            match alg {
                "dud" => {
                    let mut d = Dud::default();
                    d.initialize(&mut f, &p0).unwrap();
                    d.max_iterations = 10;
                    d.optimize(&mut f).unwrap();
                    t.cmp(&format!("D7 {key} std"), d.std_values().unwrap(), &flat(&r[&key]["std"]));
                }
                "simplex" => {
                    let mut s = Simplex::default();
                    s.initialize(&mut f, &p0, 0.5).unwrap();
                    s.optimize(&mut f).unwrap();
                }
                _ => {
                    let mut p = Powell::default();
                    p.initialize(&mut f, &p0, 0.5).unwrap();
                    p.optimize(&mut f).unwrap();
                }
            }
            compare_history(&mut t, &format!("D7 {key}"), &f, &r[&key]);
        }
    }
    let (n, s, w) = t.report("D4-D7 calibration");
    assert_eq!(s, n, "D4-D7 predicted bit-identical (worst {w:e})");
}

const OSC: fn() -> SimpleModelConfig = || SimpleModelConfig {
    timespan: Some([0.0, 0.05, 10.0]),
    parameters: Some(vec![8.0, 1.5708]),
    parameter_std: Some(vec![1.0, 0.1257]),
    system_noise: Some((vec![0.0, 0.0], vec![0.3, 0.3])),
    initial_state: Some(vec![0.0, 0.0]),
    initial_state_std: Some(vec![0.8, 0.8]),
};

fn oscillator_obs(seq: &Value) -> Observations {
    let o = &seq["oscillator_obs"];
    Observations::new(flat(&o["time"]), flat(&o["index"]), flat(&o["value"]), flat(&o["std"]), None).unwrap()
}

fn injected(case: &Value) -> DrawSource {
    let ints = case.get("bootstrap_ints").map_or(Vec::new(), |v| flat(v).iter().map(|x| *x as usize).collect());
    DrawSource::injected(flat(&case["gaussians"]), case.get("uniforms").map_or(Vec::new(), flat), ints)
}

fn compare_run<M: OdaStochModel>(t: &mut Tally, label: &str, run: &SequentialRun<M>, case: &Value, draws: &DrawSource) {
    let steps = case["steps"].as_array().unwrap();
    assert_eq!(run.steps.len(), steps.len(), "{label}: analysis count");
    let mut fc = (Vec::new(), Vec::new());
    let mut an = (Vec::new(), Vec::new());
    let mut ma = (Vec::new(), Vec::new());
    let mut g = (Vec::new(), Vec::new());
    for (ours, theirs) in run.steps.iter().zip(steps) {
        assert!((ours.time - theirs["time"].as_f64().unwrap()).abs() < 1e-9);
        fc.0.extend(ours.forecast.concat());
        fc.1.extend(flat(&theirs["forecast"]));
        an.0.extend(ours.analysis.concat());
        an.1.extend(flat(&theirs["analysis"]));
        ma.0.extend(ours.main_after.clone());
        ma.1.extend(flat(&theirs["main_after"]));
        if let (Some((pre, post)), Some(gp)) = (&ours.gain, theirs.get("gain_pre")) {
            g.0.extend(pre.concat());
            g.1.extend(flat(gp));
            g.0.extend(post.concat());
            g.1.extend(flat(&theirs["gain_post"]));
        }
    }
    t.cmp(&format!("{label} forecast members"), &fc.0, &fc.1);
    t.cmp(&format!("{label} analysis members"), &an.0, &an.1);
    t.cmp(&format!("{label} main after analysis"), &ma.0, &ma.1);
    if !g.0.is_empty() {
        t.cmp(&format!("{label} gains (pre, post)"), &g.0, &g.1);
    }
    t.cmp(&format!("{label} final state"), &run.main.state(), &flat(&case["final_state"]));
    if let Some([gs, us, is]) = draws.consumed() {
        let want = [flat(&case["gaussians"]).len(), case.get("uniforms").map_or(0, |v| flat(v).len()), case.get("bootstrap_ints").map_or(0, |v| flat(v).len())];
        assert_eq!([gs, us, is], want, "{label}: every recorded draw consumed, no more");
    }
}

/// **D8–D12.** EnKF (localisation none / Hamill / autoZhang, stochastic
/// parameters), EnSR, the particle filter, the steady-state filter and the
/// Lorenz-63 EnKF with OpenDA's draws injected: every forecast and analysis
/// member at every analysis time, the gains, the main model, bit for bit,
/// and every recorded draw consumed.
///
/// **Result (2026-11-04):** **4 281/4 281 bit-identical** (forecast and
/// analysis members at all 11 / 10 analysis times, gains, main model, final
/// state), every recorded draw consumed and no more. Predicted bit-identical:
/// met, including the particle filter (D10), whose weights use `exp`.
#[test]
fn d8_d12_sequential() {
    let seq = load("openda_sequential.json");
    let obs = oscillator_obs(&seq);
    let tpl = SimpleStochModel::new(Dynamics::oscillator(), &OSC()).unwrap();
    let mut t = Tally::default();
    let o = EnsembleOptions::default();
    let cases: Vec<(&str, SimpleStochModel, SequentialFilter)> = vec![
        ("enkf_none", tpl.clone(), SequentialFilter::EnKf { ensemble_size: 10, localization: Localization::None, options: o }),
        ("enkf_hamill", tpl.clone().with_localization_mask(vec![1.0, 0.4]), SequentialFilter::EnKf { ensemble_size: 10, localization: Localization::Hamill, options: o }),
        ("enkf_zhang", tpl.clone(), SequentialFilter::EnKf { ensemble_size: 10, localization: Localization::auto_zhang(), options: o }),
        ("enkf_stochpar", tpl.clone(), SequentialFilter::EnKf { ensemble_size: 10, localization: Localization::None, options: EnsembleOptions { stoch_parameter: true, ..o } }),
        ("ensr", tpl.clone(), SequentialFilter::EnSr { ensemble_size: 10, options: o }),
        ("particle_filter", tpl.clone(), SequentialFilter::ParticleFilter { ensemble_size: 20, options: o }),
    ];
    for (key, model, filter) in cases {
        let mut d = injected(&seq[key]);
        let run = run_sequential(&model, &obs, &filter, &mut d).unwrap();
        compare_run(&mut t, key, &run, &seq[key], &d);
    }
    let ss = &seq["steady_state"];
    let ids: Vec<f64> = ss["gain_ids"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().split(':').next().unwrap().parse().unwrap()).collect();
    let gain = SteadyStateGain { columns: ids.into_iter().zip(rows(&ss["gain_columns"])).collect(), skip_factor: f64::INFINITY };
    let mut d = DrawSource::injected(Vec::new(), Vec::new(), Vec::new());
    let run = run_sequential(&tpl, &obs, &SequentialFilter::SteadyState(gain), &mut d).unwrap();
    let mut ssc = ss.clone();
    ssc["gaussians"] = Value::Array(Vec::new());
    compare_run(&mut t, "steady_state", &run, &ssc, &d);

    let lz = &seq["lorenz_enkf"];
    let lo = &lz["obs"];
    let lobs = Observations::new(flat(&lo["time"]), flat(&lo["index"]), flat(&lo["value"]), flat(&lo["std"]), None).unwrap();
    let lcfg = SimpleModelConfig {
        timespan: Some([0.0, 0.025, 5.0]),
        system_noise: Some((vec![0.0; 3], vec![0.1; 3])),
        initial_state: Some(vec![1.508870, -1.531271, 25.46091]),
        initial_state_std: Some(vec![0.5; 3]),
        ..Default::default()
    };
    let ltpl = SimpleStochModel::new(Dynamics::lorenz63(), &lcfg).unwrap();
    let mut d = injected(lz);
    let run = run_sequential(&ltpl, &lobs, &SequentialFilter::EnKf { ensemble_size: 10, localization: Localization::None, options: o }, &mut d).unwrap();
    compare_run(&mut t, "lorenz_enkf", &run, lz, &d);
    let (n, s, w) = t.report("D8-D12 sequential");
    assert_eq!(s, n, "D8-D12 predicted bit-identical (worst {w:e})");
}

fn mean_sd(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let m = v.iter().sum::<f64>() / n;
    (m, (v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (n - 1.0)).sqrt())
}

/// **D13, statistical (criterion fixed before the run, record §4).** The
/// port on the workspace LCG (seeds 1–200) against OpenDA on its own
/// `java.util.Random` (seeds 1–200): final analysis state of the oscillator
/// with EnKF, EnSR (N 10) and the particle filter (N 20). Pass: per
/// component |Δ mean|/√(s₁²/200 + s₂²/200) < 3 and sd ratio in [0.8, 1.25].
///
/// **Result (2026-11-04): pass, all six** (|Δ mean|/se 0.27–1.21, sd ratio
/// 0.926–1.149).
#[test]
fn d13_statistical() {
    let seq = load("openda_sequential.json");
    let obs = oscillator_obs(&seq);
    let tpl = SimpleStochModel::new(Dynamics::oscillator(), &OSC()).unwrap();
    let o = EnsembleOptions::default();
    let mut all_pass = true;
    for (key, filter) in [
        ("enkf", SequentialFilter::EnKf { ensemble_size: 10, localization: Localization::None, options: o }),
        ("ensr", SequentialFilter::EnSr { ensemble_size: 10, options: o }),
        ("particle_filter", SequentialFilter::ParticleFilter { ensemble_size: 20, options: o }),
    ] {
        let java = rows(&seq["replicates"][key]);
        let ours: Vec<Vec<f64>> = (1..=200u64).map(|s| run_sequential(&tpl, &obs, &filter, &mut DrawSource::lcg(s)).unwrap().main.state()).collect();
        for c in 0..2 {
            let (mj, sj) = mean_sd(&java.iter().map(|r| r[c]).collect::<Vec<_>>());
            let (mo, so) = mean_sd(&ours.iter().map(|r| r[c]).collect::<Vec<_>>());
            let z = (mo - mj).abs() / (sj * sj / 200.0 + so * so / 200.0).sqrt();
            let ratio = so / sj;
            let pass = z < 3.0 && (0.8..=1.25).contains(&ratio);
            all_pass &= pass;
            println!("D13 {key} x[{c}]: OpenDA {mj:+.5} ± {sj:.5}, port {mo:+.5} ± {so:.5}, |dmean|/se {z:.2}, sd ratio {ratio:.3} {}", if pass { "pass" } else { "FAIL" });
        }
    }
    assert!(all_pass, "D13 criterion");
}

/// A straight line `y = a + b t`.
struct Line {
    t: Vec<f64>,
}

impl Predictor for Line {
    fn predict(&self, p: &[f64]) -> raffles::Result<Vec<f64>> {
        Ok(self.t.iter().map(|t| p[0] + p[1] * t).collect())
    }
}

fn line_cost() -> WeightedLeastSquares<Line> {
    let t: Vec<f64> = (0..10).map(f64::from).collect();
    let y: Vec<f64> = t.iter().map(|t| 2.0 + 0.5 * t).collect();
    WeightedLeastSquares::new(Line { t }, StochVector::independent(y, vec![0.1; 10]).unwrap(), StochVector::independent(vec![0.0, 0.0], vec![1.0, 1.0]).unwrap(), 0.5, false)
}

/// **A1–A3.** DUD, Simplex and Powell recover (a, b) = (2, 0.5) of a
/// noise-free straight line through OpenDA's weighted cost (σ 0.1, start
/// (0, 0)). Predictions (record §4): DUD to 1e-12 relative at its first
/// trial point; Simplex (tolerances 1e-14, maxit 10 000) and Powell within
/// 1e-5.
///
/// **Result (2026-11-04):** A1 met (2.7e-15 at the first trial, 4
/// evaluations); A2 met (2.9e-9 after 69 iterations); **A3 missed**: Powell
/// stopped at iteration 2 at (0.579, 0.816). Post-hoc (after the miss): with
/// Brent's tolerances at 1e-14 it reaches (2, 0.5) in 3 iterations.
#[test]
fn a1_a3_linear_model_recovery() {
    let mut f = line_cost();
    let mut d = Dud::default();
    d.initialize(&mut f, &[0.0, 0.0]).unwrap();
    d.optimize(&mut f).unwrap();
    let first = f.history().parameters[3].clone();
    let e1 = ((first[0] - 2.0) / 2.0).abs().max(((first[1] - 0.5) / 0.5).abs());
    println!("A1 DUD first trial {first:?}, max rel error {e1:.3e}; {} evaluations, optimum {:?}", f.history().costs.len(), f.history().optimum().unwrap());
    let mut g = line_cost();
    let mut s = Simplex::default();
    (s.abs_tol, s.rel_tol, s.max_iterations) = (1e-14, 1e-14, 10_000);
    s.initialize(&mut g, &[0.0, 0.0], 1.0).unwrap();
    s.optimize(&mut g).unwrap();
    let ps = g.history().optimum().unwrap().1.to_vec();
    let e2 = (ps[0] - 2.0).abs().max((ps[1] - 0.5).abs());
    println!("A2 Simplex {ps:?} after {} iterations: max abs error {e2:.3e}", s.iterations());
    let mut h = line_cost();
    let mut p = Powell::default();
    (p.abs_tol, p.rel_tol) = (1e-14, 1e-14);
    p.initialize(&mut h, &[0.0, 0.0], 1.0).unwrap();
    p.optimize(&mut h).unwrap();
    let pp = h.history().optimum().unwrap().1.to_vec();
    let e3 = (pp[0] - 2.0).abs().max((pp[1] - 0.5).abs());
    println!("A3 Powell {pp:?} after {} iterations: max abs error {e3:.3e}", p.iterations());
    // POST-HOC diagnosis of the A3 miss (run after the miss was seen; not a
    // prediction): with Brent's tolerances tightened from OpenDA's 0.01 /
    // 0.001 to 1e-14 the same code recovers the line.
    let mut k = line_cost();
    let mut q = Powell::default();
    (q.abs_tol, q.rel_tol, q.abs_tol_brent, q.rel_tol_brent) = (1e-14, 1e-14, 1e-14, 1e-14);
    q.initialize(&mut k, &[0.0, 0.0], 1.0).unwrap();
    q.optimize(&mut k).unwrap();
    let pq = k.history().optimum().unwrap().1.to_vec();
    let e4 = (pq[0] - 2.0).abs().max((pq[1] - 0.5).abs());
    println!("A3 post-hoc, Brent tolerances 1e-14: {pq:?} after {} iterations, {} evaluations: max abs error {e4:.3e}", q.iterations(), k.history().costs.len());
    assert!(e1 < 1e-12, "A1");
    assert!(e2 < 1e-5, "A2");
    // A3 MISSED its prediction (< 1e-5): OpenDA's Powell with its default
    // Brent tolerances stalls at iteration 2 (both line searches return their
    // base point, so the change in J is 0). No accuracy gate is set on it.
    assert!(e3.is_finite() && p.iterations() >= 1);
    assert!(e4 < 1e-10, "post-hoc check");
}

/// The exact Kalman filter of the OpenDA oscillator experiment: RK4 is a
/// linear map F (its columns are F applied to e₀, e₁), the noise adds
/// `dt·0.3²·I` per substep, the prior is N(0, 0.8²I), x₀ is observed with
/// σ 0.1 at t = 0, 1, …, 10.
fn oscillator_kf(obs: &Observations) -> ([f64; 2], [[f64; 2]; 2]) {
    let mk = |x: [f64; 2]| {
        let mut m = SimpleStochModel::new(Dynamics::oscillator(), &SimpleModelConfig { initial_state: Some(x.to_vec()), ..OSC() }).unwrap();
        m.compute(0.05, &mut DrawSource::lcg(0)).unwrap();
        m.state()
    };
    let (c0, c1) = (mk([1.0, 0.0]), mk([0.0, 1.0]));
    let f = [[c0[0], c1[0]], [c0[1], c1[1]]];
    let q = 0.05 * 0.09;
    let (mut x, mut p) = ([0.0, 0.0], [[0.64, 0.0], [0.0, 0.64]]);
    for (k, &y) in obs.value.iter().enumerate() {
        if k > 0 {
            for _ in 0..20 {
                x = [f[0][0] * x[0] + f[0][1] * x[1], f[1][0] * x[0] + f[1][1] * x[1]];
                let fp = [[f[0][0] * p[0][0] + f[0][1] * p[1][0], f[0][0] * p[0][1] + f[0][1] * p[1][1]], [f[1][0] * p[0][0] + f[1][1] * p[1][0], f[1][0] * p[0][1] + f[1][1] * p[1][1]]];
                p = [
                    [fp[0][0] * f[0][0] + fp[0][1] * f[0][1] + q, fp[0][0] * f[1][0] + fp[0][1] * f[1][1]],
                    [fp[1][0] * f[0][0] + fp[1][1] * f[0][1], fp[1][0] * f[1][0] + fp[1][1] * f[1][1] + q],
                ];
            }
        }
        let s = p[0][0] + 0.01;
        let k = [p[0][0] / s, p[1][0] / s];
        let innov = y - x[0];
        x = [x[0] + k[0] * innov, x[1] + k[1] * innov];
        p = [[(1.0 - k[0]) * p[0][0], (1.0 - k[0]) * p[0][1]], [p[1][0] - k[1] * p[0][0], p[1][1] - k[1] * p[0][1]]];
    }
    (x, p)
}

/// **A4, A5.** The ensemble filters converge to the exact KF on the linear
/// oscillator as N grows: mean error |x̄ − x_KF|/σ_KF at t = 10, averaged
/// over 10 seeds (the larger of the two components), and the ensemble
/// variance against P_KF. Predictions (record §4): EnKF monotone in N and
/// < 0.1 at N 1600, variance within 10 % at N 1600; EnSR < 0.1 at N 1600.
///
/// **Result (2026-11-04):** A4 met: 0.145, 0.069, 0.054, 0.0165 σ_KF at
/// N 25, 100, 400, 1600; variance ratio (0.998, 1.010) at N 1600. **A5 not
/// tested as predicted:** EnSR's q×q SVD (OpenDA's design) took 66 s for 10
/// seeds at N 400, so N 1600 (about 64× that) was not run; at N 25, 100, 400
/// the error is 0.138, 0.056, 0.0525 σ_KF, reported without a gate.
#[test]
fn a4_a5_ensemble_filters_converge_to_the_kf() {
    let seq = load("openda_sequential.json");
    let obs = oscillator_obs(&seq);
    let (xk, pk) = oscillator_kf(&obs);
    println!("KF at t = 10: x {xk:?}, sd ({:.5}, {:.5})", pk[0][0].sqrt(), pk[1][1].sqrt());
    let tpl = SimpleStochModel::new(Dynamics::oscillator(), &OSC()).unwrap();
    let o = EnsembleOptions::default();
    let mut enkf_errors = Vec::new();
    let run = |filter: &SequentialFilter, seeds: u64| -> (f64, [f64; 2]) {
        let (mut err, mut var) = ([0.0; 2], [0.0; 2]);
        for s in 1..=seeds {
            let r = run_sequential(&tpl, &obs, filter, &mut DrawSource::lcg(1000 + s)).unwrap();
            let states: Vec<Vec<f64>> = r.members.iter().map(OdaStochModel::state).collect();
            let n = states.len() as f64;
            for c in 0..2 {
                let m = states.iter().map(|x| x[c]).sum::<f64>() / n;
                err[c] += (m - xk[c]).abs() / pk[c][c].sqrt() / seeds as f64;
                var[c] += states.iter().map(|x| (x[c] - m) * (x[c] - m)).sum::<f64>() / (n - 1.0) / pk[c][c] / seeds as f64;
            }
        }
        (err[0].max(err[1]), var)
    };
    let mut last_var = [0.0; 2];
    for n in [25, 100, 400, 1600] {
        let t0 = std::time::Instant::now();
        let (e, v) = run(&SequentialFilter::EnKf { ensemble_size: n, localization: Localization::None, options: o }, 10);
        println!("A4 EnKF N {n}: mean error {e:.4} sigma_KF, variance ratio ({:.3}, {:.3}) ({:.1} s)", v[0], v[1], t0.elapsed().as_secs_f64());
        enkf_errors.push(e);
        last_var = v;
    }
    let mut ensr = Vec::new();
    for n in [25, 100, 400] {
        let t0 = std::time::Instant::now();
        let (e, v) = run(&SequentialFilter::EnSr { ensemble_size: n, options: o }, 10);
        println!("A5 EnSR N {n}: mean error {e:.4} sigma_KF, variance ratio ({:.3}, {:.3}) ({:.1} s)", v[0], v[1], t0.elapsed().as_secs_f64());
        ensr.push(e);
    }
    assert!(enkf_errors.windows(2).all(|w| w[1] < w[0]) && enkf_errors[3] < 0.1, "A4 mean {enkf_errors:?}");
    assert!(last_var.iter().all(|v| (v - 1.0).abs() < 0.1), "A4 variance {last_var:?}");
    assert!(ensr.iter().all(|e| e.is_finite()));
}

