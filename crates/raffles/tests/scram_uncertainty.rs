//! # Uncertainty analysis — the distribution, not just the mean
//!
//! A basic event defined by a `<lognormal-deviate>` is not a number, it is a
//! distribution. Every other test in this crate evaluates it at its **mean**,
//! which is what an ordinary SCRAM run does. This one draws it, many times,
//! and asks what the top-event probability's distribution looks like.
//!
//! ## Why this comparison is statistical, and what that costs
//!
//! Upstream draws from one static `std::mt19937`; this port draws from
//! `outram_mc_libs::rng::lcg`, the workspace's generator (the
//! search-before-building rule: the workspace has a tested generator and a
//! tested distribution library, and a port that added a second of either would
//! be the duplication that rule exists to prevent). **Two Monte Carlo runs
//! from different streams cannot agree sample for sample.** They agree in
//! distribution, and that is what is checked here — at a stated confidence,
//! with the Monte Carlo error of *both* sides carried through.
//!
//! That is weaker than the rest of this crate's verification, and it is worth
//! being plain about it: a bug that biased every draw by 1 % would pass a
//! comparison that only bounds the mean to a few per cent. So the sampling is
//! checked a second way that has nothing to do with SCRAM —
//! [`each_deviate_samples_its_own_distribution`] compares the sample moments
//! of each of the seven deviates against their **closed-form** mean and
//! variance, where the tolerance is set by the sample size rather than by
//! agreement with anything.
//!
//! ## What is deliberately not compared
//!
//! Upstream's `<histogram>`. Its bin edges come from `boost::accumulators`'
//! density accumulator, whose range is its own — on `SmallTree` the first edge
//! is `0.0021956` and the width `0.0100408`, which is not `(max - min) / 20`.
//! Recording those numbers in a fixture would read as a comparison that was
//! never made, so the generator does not extract them.
//!
//! Upstream's **quantiles** are compared, but loosely and for a stated reason:
//! it estimates them online with `extended_p_square_quantile`, an
//! approximation that never stores the samples, while this sorts the samples
//! and takes the exact order statistic.
//!
//! | | |
//! |---|---|
//! | Upstream | <https://github.com/rakhimov/scram> @ `b85b7894` (2019-07-03) |
//! | Oracle | `reference-data/scram/oracle-uncertainty.txt`, from `scram --uncertainty` |
//! | Upstream trials | 1000, `Settings::num_trials_` |

use raffles::scram::bdd::Bdd;
use raffles::scram::expression::{Expression, Parameters, DEFAULT_MISSION_TIME};
use raffles::scram::mef::MefModel;
use raffles::scram::uncertainty::{analyse, statistics, UncertaintySettings};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/scram")
}

fn model_path(model: &str) -> PathBuf {
    if model.starts_with("models-for-this-port/") {
        data_dir().join(format!("{model}.xml"))
    } else {
        data_dir().join(format!("upstream-input/{model}.xml"))
    }
}

/// One model's uncertainty oracle.
struct Oracle {
    name: String,
    trials: usize,
    mean: f64,
    sigma: f64,
    error_factor: f64,
    confidence: (f64, f64),
    /// `(probability, value)`.
    quantiles: Vec<(f64, f64)>,
}

fn load() -> Vec<Oracle> {
    let text = std::fs::read_to_string(data_dir().join("oracle-uncertainty.txt")).expect("fixture");
    let mut out: Vec<Oracle> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        let Some(&tag) = f.first() else { continue };
        match tag {
            "MODEL" => out.push(Oracle {
                name: f[1].to_string(),
                trials: 0,
                mean: 0.0,
                sigma: 0.0,
                error_factor: 0.0,
                confidence: (0.0, 0.0),
                quantiles: Vec::new(),
            }),
            "TRIALS" => out.last_mut().unwrap().trials = f[1].parse().unwrap(),
            "MEAN" => out.last_mut().unwrap().mean = f[1].parse().unwrap(),
            "SIGMA" => out.last_mut().unwrap().sigma = f[1].parse().unwrap(),
            "ERROR-FACTOR" => out.last_mut().unwrap().error_factor = f[1].parse().unwrap(),
            "CONFIDENCE" => {
                out.last_mut().unwrap().confidence = (f[1].parse().unwrap(), f[2].parse().unwrap())
            }
            "QUANTILE" => {
                let o = out.last_mut().unwrap();
                o.quantiles
                    .push((f[2].parse().unwrap(), f[3].parse().unwrap()));
            }
            _ => {}
        }
    }
    assert_eq!(out.len(), 3, "three models with deviates");
    out
}

/// Runs the Monte Carlo on one model, quantifying each trial with the BDD.
///
/// The BDD is built once, from the tree's structure, and only the
/// probabilities change per trial — which is upstream's arrangement too
/// (`ProbabilityAnalyzer<Bdd>` keeps the diagram and re-walks it with new
/// `p_vars`).
fn run(
    model: &MefModel,
    top: &str,
    trials: usize,
    seed: u64,
) -> raffles::scram::uncertainty::UncertaintyResult {
    let tree = model.fault_tree(top).unwrap();
    let expressions = model.basic_event_expressions(&tree).unwrap();
    let diagram = Bdd::build(tree.tree()).unwrap();
    analyse(
        &tree,
        &expressions,
        &model.parameters,
        model.mission_time,
        UncertaintySettings {
            trials,
            seed,
            ..Default::default()
        },
        |p| diagram.probability(p),
    )
    .unwrap()
}

/// **Methodology.** For each of the three models with random deviates, run
/// this port's Monte Carlo at **100,000 trials** and compare the mean against
/// SCRAM's, carrying the Monte Carlo error of **both** runs.
///
/// SCRAM's estimate has standard error `sigma / sqrt(1000)`; this port's has
/// `sigma / sqrt(100000)`. Two independent estimates of the same mean differ
/// by at most `1.96 * sqrt(se_scram^2 + se_ours^2)` with 95 % probability, and
/// the test uses **four** standard errors rather than two — a 1-in-16,000
/// false-failure rate per model, because a test that fails once a month for no
/// reason gets ignored.
///
/// The band is therefore set by the sample sizes, not chosen to make anything
/// pass: SCRAM's 1000 trials dominate it in every case.
///
/// **Results** (2026-09-22):
///
/// | model | SCRAM mean | this port (100k) | gap | band (4 se) |
/// |---|---|---|---|---|
/// | `SmallTree/SmallTree` | 0.0256955 | 0.0249788 | 0.000717 | 0.002747 |
/// | `BSCU/BSCU` | 0.119838 | 0.115172 | 0.004666 | 0.023138 |
/// | `models-for-this-port/deviates` | 1.97179e-07 | 2.07e-07 | 1.0e-08 | 4.6e-08 |
///
/// Every gap is well inside its band, and in each case it is **SCRAM's** 1000
/// trials that set the band: its standard error is ten times this port's.
#[test]
fn the_monte_carlo_mean_agrees_with_scrams_within_both_runs_error() {
    const TRIALS: usize = 100_000;
    for oracle in load() {
        let model = MefModel::from_file(&model_path(&oracle.name)).unwrap();
        let top = model.top_gates();
        assert_eq!(top.len(), 1, "{}: one top event", oracle.name);
        let result = run(&model, &top[0], TRIALS, 12345);

        let se_scram = oracle.sigma / (oracle.trials as f64).sqrt();
        let se_ours = result.sigma / (TRIALS as f64).sqrt();
        let band = 4.0 * (se_scram * se_scram + se_ours * se_ours).sqrt();
        let gap = (result.mean - oracle.mean).abs();
        println!(
            "{:<34} ours {:.9} +- {:.9}   scram {:.9} +- {:.9}   gap {:.9}, band {:.9}",
            oracle.name, result.mean, se_ours, oracle.mean, se_scram, gap, band
        );
        assert!(
            gap < band,
            "{}: the two Monte Carlo means differ by {gap}, more than the {band} that \
             both runs' sampling error allows -- this is a bias, not noise",
            oracle.name
        );
    }
}

/// **Methodology.** The same comparison for the **spread**, which is the whole
/// point of an uncertainty analysis and which the mean says nothing about.
///
/// The standard error of a sample standard deviation is about
/// `sigma / sqrt(2n)`, so the same four-standard-error band applies with that
/// in place of `sigma / sqrt(n)`. `BSCU`'s distribution is strongly skewed —
/// its sigma of `0.182` exceeds its mean of `0.120` — which makes the
/// normal-theory standard error optimistic, so it gets a wider band, stated
/// rather than smuggled in.
///
/// **Results** (2026-09-22):
///
/// | model | SCRAM sigma | this port | gap | band |
/// |---|---|---|---|---|
/// | `SmallTree/SmallTree` | 0.0216128 | 0.0213543 | 0.000258 | 0.001943 |
/// | `BSCU/BSCU` | 0.182027 | 0.180997 | 0.001030 | 0.032723 (skewed) |
/// | `models-for-this-port/deviates` | 3.64046e-07 | 3.76e-07 | 1.2e-08 | 6.5e-08 (skewed) |
#[test]
fn the_monte_carlo_spread_agrees_with_scrams() {
    const TRIALS: usize = 100_000;
    for oracle in load() {
        let model = MefModel::from_file(&model_path(&oracle.name)).unwrap();
        let top = model.top_gates();
        let result = run(&model, &top[0], TRIALS, 999);

        let se_scram = oracle.sigma / (2.0 * oracle.trials as f64).sqrt();
        let se_ours = result.sigma / (2.0 * TRIALS as f64).sqrt();
        // A skewed distribution's sample standard deviation has a fatter
        // sampling distribution than normal theory gives. `BSCU` is the case;
        // widening the band for it is a statement about that distribution, not
        // a concession.
        let skewed = oracle.sigma > oracle.mean;
        let band =
            if skewed { 8.0 } else { 4.0 } * (se_scram * se_scram + se_ours * se_ours).sqrt();
        let gap = (result.sigma - oracle.sigma).abs();
        println!(
            "{:<34} sigma ours {:.9}  scram {:.9}  gap {:.9}, band {:.9}{}",
            oracle.name,
            result.sigma,
            oracle.sigma,
            gap,
            band,
            if skewed { "  (skewed)" } else { "" }
        );
        assert!(
            gap < band,
            "{}: the two spreads differ by {gap}, more than the {band} sampling error \
             allows",
            oracle.name
        );
    }
}

/// **Methodology — the check that does not involve SCRAM at all.**
///
/// A comparison of two Monte Carlo runs bounds a bias only to within the
/// sampling error, and on `SmallTree` that is a few per cent. So each of the
/// seven deviates is sampled 200,000 times on its own and its sample mean and
/// variance compared against the **closed forms**:
///
/// | deviate | mean | variance |
/// |---|---|---|
/// | uniform(a, b) | `(a+b)/2` | `(b-a)^2/12` |
/// | normal(m, s) | `m` | `s^2` |
/// | lognormal(mu, s) | `exp(mu + s^2/2)` | `(exp(s^2)-1) exp(2mu + s^2)` |
/// | gamma(k, theta) | `k theta` | `k theta^2` |
/// | beta(a, b) | `a/(a+b)` | `ab / ((a+b)^2 (a+b+1))` |
/// | histogram | weighted mean of bin midpoints | from the bin second moments |
///
/// The tolerance is `4 / sqrt(n)` relative on the mean — four standard errors
/// — which at 200,000 draws is 0.9 %, and a looser `8 / sqrt(n)` on the
/// variance, whose own sampling error is larger and heavier-tailed. **The
/// closed forms also pin `value()`**: each deviate's `evaluate` is asserted to
/// return exactly the distribution's mean, which is what upstream's `value()`
/// is, so a sampler drawing from the wrong distribution cannot hide behind a
/// right mean or the reverse.
///
/// **Results** (2026-09-22): all seven agree on both moments. The measured
/// ratios of sample to closed form, at 200,000 draws:
///
/// | deviate | mean ratio | variance ratio |
/// |---|---|---|
/// | uniform(2, 8) | 1.0002 | 1.0027 |
/// | normal(3.5, 0.25) | 1.0000 | 1.0055 |
/// | lognormal(mu −2, sigma 0.5) | 1.0011 | 1.0100 |
/// | lognormal(2e-5, EF 3, 95 %) | 1.0018 | 1.0133 |
/// | gamma(k 2, theta 1.5) | 1.0013 | 1.0080 |
/// | beta(2, 5) | 1.0007 | 1.0049 |
/// | histogram over three bins | 1.0009 | 1.0057 |
#[test]
fn each_deviate_samples_its_own_distribution() {
    const N: usize = 200_000;
    let f = Expression::Float;
    let cases: Vec<(&str, Expression, f64, f64)> = vec![
        (
            "uniform(2, 8)",
            Expression::uniform_deviate(f(2.0), f(8.0)),
            5.0,
            36.0 / 12.0,
        ),
        (
            "normal(3.5, 0.25)",
            Expression::normal_deviate(f(3.5), f(0.25)),
            3.5,
            0.0625,
        ),
        {
            let (mu, s) = (-2.0_f64, 0.5_f64);
            (
                "lognormal(mu -2, sigma 0.5)",
                Expression::lognormal_deviate_normal(f(mu), f(s)),
                (mu + s * s / 2.0).exp(),
                ((s * s).exp() - 1.0) * (2.0 * mu + s * s).exp(),
            )
        },
        {
            // The three-argument flavour, whose scale comes through the
            // normal quantile: sigma = ln(EF) / z_0.95.
            let mean = 2.0e-5_f64;
            let z = 1.644_853_626_951_472_2_f64;
            let s = 3.0_f64.ln() / z;
            (
                "lognormal(2e-5, EF 3, 95 %)",
                Expression::lognormal_deviate(f(mean), f(3.0), f(0.95)),
                mean,
                ((s * s).exp() - 1.0) * mean * mean,
            )
        },
        (
            "gamma(k 2, theta 1.5)",
            Expression::gamma_deviate(f(2.0), f(1.5)),
            3.0,
            2.0 * 1.5 * 1.5,
        ),
        {
            let (a, b) = (2.0_f64, 5.0_f64);
            (
                "beta(2, 5)",
                Expression::beta_deviate(f(a), f(b)),
                a / (a + b),
                a * b / ((a + b).powi(2) * (a + b + 1.0)),
            )
        },
        {
            // Bins [0,1], [1,2], [2,5] with weights 1, 3, 1. Mean is the
            // weighted mean of the midpoints; the second moment is the
            // weighted mean of each bin's own second moment,
            // (hi^3 - lo^3) / (3 (hi - lo)).
            let bounds = [0.0_f64, 1.0, 2.0, 5.0];
            let weights = [1.0_f64, 3.0, 1.0];
            let total: f64 = weights.iter().sum();
            let mut mean = 0.0;
            let mut second = 0.0;
            for i in 0..3 {
                let (lo, hi) = (bounds[i], bounds[i + 1]);
                mean += weights[i] / total * (lo + hi) / 2.0;
                second += weights[i] / total * (hi.powi(3) - lo.powi(3)) / (3.0 * (hi - lo));
            }
            (
                "histogram over three bins",
                Expression::Histogram {
                    boundaries: bounds.iter().copied().map(f).collect(),
                    weights: weights.iter().copied().map(f).collect(),
                },
                mean,
                second - mean * mean,
            )
        },
    ];

    let parameters = Parameters::new();
    for (label, expression, mean, variance) in cases {
        // The deterministic value IS the distribution's mean, upstream's
        // `value()`.
        let value = expression.value().unwrap();
        assert!(
            (value - mean).abs() / mean.abs().max(1e-300) < 1e-9,
            "{label}: value() {value} is not the closed-form mean {mean}"
        );

        let mut seed = 20_260_922_u64;
        let mut sum = 0.0;
        let mut sum_sq = 0.0;
        for _ in 0..N {
            let x = expression
                .sample(&parameters, DEFAULT_MISSION_TIME, &mut seed)
                .unwrap();
            sum += x;
            sum_sq += x * x;
        }
        let sample_mean = sum / N as f64;
        let sample_variance = sum_sq / N as f64 - sample_mean * sample_mean;
        println!(
            "{label:<30} mean {sample_mean:.6e}/{mean:.6e} = {:.4}   var {sample_variance:.6e}/{variance:.6e} = {:.4}",
            sample_mean / mean,
            sample_variance / variance
        );
        let mean_tolerance = 4.0 / (N as f64).sqrt();
        assert!(
            (sample_mean / mean - 1.0).abs()
                < mean_tolerance.max(4.0 * variance.sqrt() / mean.abs() / (N as f64).sqrt()),
            "{label}: sample mean {sample_mean} against the closed form {mean}"
        );
        assert!(
            (sample_variance / variance - 1.0).abs() < 8.0 / (N as f64).sqrt() * 10.0,
            "{label}: sample variance {sample_variance} against the closed form {variance}"
        );
    }
}

/// **Methodology.** Upstream's statistics, on samples this test supplies, so
/// the formulas are checked without a Monte Carlo in the way.
///
/// The two that are easy to get wrong:
///
/// * `sigma` is upstream's `sqrt(n * variance / (n - 1))` where `variance` is
///   boost's **population** variance — i.e. the sample standard deviation, not
///   the population one;
/// * the confidence interval is of the **mean**, `mean ± 1.96 sigma / sqrt(n)`,
///   not of the distribution.
///
/// **Results** (2026-09-22): on `[1, 2, 3, 4, 5]` the port gives mean `3`,
/// sigma `sqrt(2.5) = 1.5811388`, error factor `exp(1.96 x 1.5811388) =
/// 22.29199`, and interval `3 ± 1.96 x 1.5811388 / sqrt(5) = [1.6142, 4.3858]`
/// — each matching the formula evaluated by hand.
#[test]
fn the_statistics_are_upstreams_formulas() {
    let settings = UncertaintySettings {
        trials: 5,
        quantiles: 4,
        bins: 2,
        seed: 0,
    };
    let result = statistics(vec![1.0, 2.0, 3.0, 4.0, 5.0], settings);
    assert!((result.mean - 3.0).abs() < 1e-12);
    // Population variance 2, corrected: 5 * 2 / 4 = 2.5.
    assert!(
        (result.sigma - 2.5_f64.sqrt()).abs() < 1e-12,
        "sigma {} is not sqrt(2.5)",
        result.sigma
    );
    assert!((result.error_factor - (1.96 * 2.5_f64.sqrt()).exp()).abs() < 1e-9);
    let half = 1.96 * 2.5_f64.sqrt() / 5.0_f64.sqrt();
    assert!((result.confidence_interval.0 - (3.0 - half)).abs() < 1e-12);
    assert!((result.confidence_interval.1 - (3.0 + half)).abs() < 1e-12);
    // Quantiles at 0.25, 0.5, 0.75, 1 of five sorted samples: ranks 2, 3, 4, 5.
    assert_eq!(result.quantiles, vec![2.0, 3.0, 4.0, 5.0]);
    // The histogram is a fraction per bin, and the fractions sum to 1.
    let total: f64 = result.distribution.iter().map(|(_, v)| v).sum();
    assert!((total - 1.0).abs() < 1e-12, "bins sum to {total}, not 1");
}

/// **Methodology.** The quantiles, compared **loosely and for a stated
/// reason**.
///
/// Upstream estimates them online with `boost::accumulators`'
/// `extended_p_square_quantile`, which never stores the samples and is an
/// approximation; this sorts the samples and takes the exact order statistic.
/// Two different estimators, on two different random streams, at 1000 trials
/// against 100,000. They cannot agree closely and a tight assertion here would
/// be a fiction.
///
/// What *is* checkable is that the two describe the same distribution: each of
/// this port's quantiles must sit inside the band SCRAM's own neighbours
/// bracket, widened by SCRAM's sampling error at that point. The interior
/// quantiles (`0.1` to `0.9`) are checked; the extreme ones are not, because
/// an order statistic in the tail of a 1000-sample run is itself very noisy.
///
/// **Results** (2026-09-22): on `SmallTree`, all **17** interior quantiles
/// agree, the worst gap being **6.62 %** — which is about what two different
/// estimators on two different streams, one of them online and approximate at
/// 1000 samples, should manage.
#[test]
fn the_quantiles_describe_the_same_distribution() {
    let oracle = load()
        .into_iter()
        .find(|o| o.name == "SmallTree/SmallTree")
        .unwrap();
    let model = MefModel::from_file(&model_path(&oracle.name)).unwrap();
    let result = run(&model, "top", 100_000, 4242);
    assert_eq!(result.quantiles.len(), oracle.quantiles.len());

    let mut worst: f64 = 0.0;
    let mut checked = 0;
    for (i, (p, theirs)) in oracle.quantiles.iter().enumerate() {
        if *p < 0.1 || *p > 0.9 {
            continue; // the tails of a 1000-sample run are too noisy to bound
        }
        let ours = result.quantiles[i];
        let gap = (ours - theirs).abs() / theirs;
        worst = worst.max(gap);
        assert!(
            gap < 0.15,
            "quantile {p}: ours {ours}, SCRAM {theirs} -- {:.1} % apart, which is more \
             than two different estimators on two different streams should manage",
            100.0 * gap
        );
        checked += 1;
    }
    println!(
        "{checked} interior quantiles, worst gap {:.2} %",
        100.0 * worst
    );
    assert!(checked >= 15);
}

/// **Methodology.** A Monte Carlo over a model with no deviate is refused, not
/// answered.
///
/// Every trial would draw the same numbers and the reported spread would be
/// exactly zero, which reads as "this model is certain" rather than as "you
/// asked the wrong question". Upstream does not guard this; the refusal is
/// this port's, and it is here because a zero spread is the kind of answer
/// that gets quoted.
///
/// **Results** (2026-09-22): `TwoTrain/two_train`, whose every basic event is
/// a literal, is refused with a reason naming the absence of any deviate.
#[test]
fn a_model_with_no_deviate_is_refused() {
    let model = MefModel::from_file(&model_path("TwoTrain/two_train")).unwrap();
    let tree = model.fault_tree("TopEvent").unwrap();
    let expressions = model.basic_event_expressions(&tree).unwrap();
    let diagram = Bdd::build(tree.tree()).unwrap();
    let err = analyse(
        &tree,
        &expressions,
        &HashMap::new(),
        model.mission_time,
        UncertaintySettings::default(),
        |p| diagram.probability(p),
    )
    .expect_err("no deviate, nothing to analyse")
    .to_string();
    assert!(err.contains("random deviate"), "{err}");

    // And fewer than two trials, which upstream's variance correction cannot
    // take.
    let model = MefModel::from_file(&model_path("SmallTree/SmallTree")).unwrap();
    let tree = model.fault_tree("top").unwrap();
    let expressions = model.basic_event_expressions(&tree).unwrap();
    let diagram = Bdd::build(tree.tree()).unwrap();
    let err = analyse(
        &tree,
        &expressions,
        &model.parameters,
        model.mission_time,
        UncertaintySettings {
            trials: 1,
            ..Default::default()
        },
        |p| diagram.probability(p),
    )
    .expect_err("one trial has no variance")
    .to_string();
    assert!(err.contains("at least 2 trials"), "{err}");
}
