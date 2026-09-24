//! # The seven MEF random deviates, against SCRAM's own computed values
//!
//! A basic event's probability is often not a literal. Upstream's `SmallTree`
//! and `BSCU` define theirs through `<lognormal-deviate>` parameters, and
//! until 2026-09-22 they were the two models
//! [`raffles::scram::mef`] could not read at all.
//!
//! The other five deviates — uniform, the two-argument log-normal, gamma,
//! beta and histogram — appear in **no** upstream input model, so
//! `models-for-this-port/deviates.xml` uses all seven and SCRAM supplies the
//! answers in `reference-data/scram/oracle-deviates.txt`.
//!
//! ## What is checked here, and what is not
//!
//! A random deviate has two faces upstream. `value()` is its **mean** — the
//! number SCRAM's ordinary run computes, prints, and quantifies with — and
//! `DoSample()` draws from the distribution, which only the uncertainty
//! analysis calls. **This file checks the first.** Sampling is not
//! implemented yet and lands with the uncertainty analysis, where SCRAM's
//! `--uncertainty` output is the oracle; implementing it before then would be
//! implementing it with nothing to check it against.
//!
//! [`Expression::is_deviate`] — upstream's `IsDeviate()` — is here because it
//! is the hook that analysis will use, and because it is checkable now.
//!
//! | | |
//! |---|---|
//! | Upstream | <https://github.com/rakhimov/scram> @ `b85b7894` (2019-07-03) |
//! | Oracle | `reference-data/scram/oracle-deviates.txt`, from the compiled binary |
//! | Mission time | SCRAM's default, 8760 h |

use raffles::scram::expression::{Expression, Interval, Parameters, DEFAULT_MISSION_TIME};
use raffles::distributions::ContinuousDistribution1D;
use raffles::scram::mef::MefModel;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The `reference-data/scram` directory.
fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference-data/scram")
}

/// SCRAM's computed value for each basic event of a fixture record.
fn oracle_events(fixture: &str, model: &str) -> HashMap<String, f64> {
    let text = std::fs::read_to_string(data_dir().join(fixture)).expect("fixture");
    let mut out = HashMap::new();
    let mut inside = false;
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        match f.as_slice() {
            ["MODEL", name] => inside = *name == model,
            ["EVENT", name, value] if inside => {
                out.insert(name.to_string(), value.parse().unwrap());
            }
            ["END"] if inside => break,
            _ => {}
        }
    }
    assert!(!out.is_empty(), "{model} not found in {fixture}");
    out
}

/// Relative agreement, falling back to absolute near zero.
fn agrees(ours: f64, theirs: f64, tol: f64) -> bool {
    let scale = theirs.abs().max(1.0);
    (ours - theirs).abs() / scale < tol
}

fn value(e: &Expression) -> f64 {
    e.value().expect("evaluates")
}

fn float(v: f64) -> Expression {
    Expression::Float(v)
}

/// **Methodology.** Read `models-for-this-port/deviates.xml` — which uses all
/// seven deviates — through [`MefModel`], build its tree, and compare the
/// probability this crate computes for each basic event against the one SCRAM
/// printed. Tolerance `5e-6` relative, the resolution of upstream's
/// 6-significant-figure report.
///
/// Three of the deviates sit inside an `<exponential>` rather than standing as
/// a probability, and that is not presentation: upstream validates a basic
/// event's expression with `EnsureProbability`, which checks the whole
/// `interval()` against `[0, 1]`, and the gamma, beta and histogram domains
/// reach past 1.
///
/// **Results** (2026-09-22): all seven agree.
///
/// | event | deviate | this crate | SCRAM |
/// |---|---|---|---|
/// | `u` | uniform(0.1, 0.3) | 0.2 | 0.2 |
/// | `n` | normal(0.5, 0.01) | 0.5 | 0.5 |
/// | `l3` | lognormal(2e-5, EF 3, 95 %) as a rate | 0.160711 | 0.160711 |
/// | `l2` | lognormal(mu -12, sigma 0.5) as a rate | 0.0591672 | 0.0591672 |
/// | `g` | gamma(k 2, theta 1e-6) as a rate | 0.0173674 | 0.0173674 |
/// | `b` | beta(1, 999) as a rate | 0.999843 | 0.999843 |
/// | `hist` | histogram over three bins as a rate | 0.0147817 | 0.0147817 |
#[test]
fn every_deviate_value_matches_scrams() {
    let name = "models-for-this-port/deviates";
    let oracle = oracle_events("oracle-deviates.txt", name);
    let model = MefModel::from_file(&data_dir().join(format!("{name}.xml")))
        .expect("the deviates model reads");
    let tree = model.fault_tree("top").expect("tree");

    let mut checked = 0;
    for (i, event) in tree.basic_event_names().iter().enumerate() {
        let theirs = oracle[event];
        let ours = tree.probabilities()[i];
        println!("{event:<6} ours {ours:.9}  scram {theirs:.9}");
        assert!(
            agrees(ours, theirs, 5e-6),
            "{event}: ours {ours}, SCRAM {theirs}"
        );
        checked += 1;
    }
    assert_eq!(checked, 7, "the model uses all seven deviates");
}

/// **Methodology.** The two upstream models that needed the log-normal
/// deviate, read from their own XML and priced against `oracle.txt`.
///
/// They are the reason this chunk was next: `SmallTree` and `BSCU` were the
/// only two records in the whole fixture that `scram::mef` refused, and the
/// refusal was asserted so it could not drift. It is now gone.
///
/// **Results** (2026-09-22): `SmallTree`'s four events and `BSCU`'s eight all
/// agree to `5e-6` relative. `SmallTree`'s `e1 = 0.160711` comes from a
/// log-normal with mean `2e-5`, error factor 3 at the 95 % level, used as an
/// exponential's rate over 8760 h — three ported pieces in series.
#[test]
fn the_two_upstream_lognormal_models_are_no_longer_refused() {
    for name in ["SmallTree/SmallTree", "BSCU/BSCU"] {
        let oracle = oracle_events("oracle.txt", name);
        let model = MefModel::from_file(&data_dir().join(format!("upstream-input/{name}.xml")))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let top = model.top_gates();
        assert_eq!(top.len(), 1, "{name}: one top event");
        let tree = model.fault_tree(&top[0]).unwrap();
        for (i, event) in tree.basic_event_names().iter().enumerate() {
            let theirs = oracle[event];
            let ours = tree.probabilities()[i];
            println!("{name:<20} {event:<34} ours {ours:.9}  scram {theirs:.9}");
            assert!(
                agrees(ours, theirs, 5e-6),
                "{name} {event}: ours {ours}, SCRAM {theirs}"
            );
        }
    }
}

/// **Methodology.** Each deviate's `value()` against its own closed form,
/// independently of SCRAM, so that a formula error cannot hide behind a model
/// whose numbers happen to match.
///
/// These are the textbook means, not upstream's code rearranged:
/// `(min+max)/2`, `mean`, `exp(mu + sigma^2/2)`, `k * theta`,
/// `alpha / (alpha + beta)`, and the histogram's weighted mean of bin
/// midpoints.
///
/// **Results** (2026-09-22): every one agrees to `1e-12` relative.
#[test]
fn each_deviate_value_matches_its_closed_form() {
    let u = Expression::uniform_deviate(float(2.0), float(8.0));
    assert!((value(&u) - 5.0).abs() < 1e-12);

    let n = Expression::normal_deviate(float(3.5), float(0.25));
    assert!((value(&n) - 3.5).abs() < 1e-12);

    // The three-argument flavour's value is its own mean parameter, by
    // definition of the parametrisation.
    let l3 = Expression::lognormal_deviate(float(2.0e-5), float(3.0), float(0.95));
    assert!((value(&l3) - 2.0e-5).abs() < 1e-18);

    // The two-argument flavour's mean is exp(mu + sigma^2 / 2).
    let (mu, sigma) = (-12.0_f64, 0.5_f64);
    let l2 = Expression::lognormal_deviate_normal(float(mu), float(sigma));
    let expected = (mu + sigma * sigma / 2.0).exp();
    assert!((value(&l2) - expected).abs() / expected < 1e-12);

    let g = Expression::gamma_deviate(float(2.0), float(1.0e-6));
    assert!((value(&g) - 2.0e-6).abs() / 2.0e-6 < 1e-12);

    let b = Expression::beta_deviate(float(1.0), float(999.0));
    assert!((value(&b) - 1.0e-3).abs() / 1.0e-3 < 1e-12);

    // Bins [0, 1e-6], [1e-6, 2e-6], [2e-6, 5e-6] with weights 1, 3, 1.
    // Midpoints 0.5e-6, 1.5e-6, 3.5e-6; weighted mean 17e-6 / 10 = 1.7e-6.
    let h = Expression::Histogram {
        boundaries: vec![float(0.0), float(1.0e-6), float(2.0e-6), float(5.0e-6)],
        weights: vec![float(1.0), float(3.0), float(1.0)],
    };
    assert!((value(&h) - 1.7e-6).abs() / 1.7e-6 < 1e-12);
}

/// **Methodology — the identity the log-normal's `Logarithmic` flavour rests
/// on, checked rather than assumed.**
///
/// Upstream computes its scale parameter as
///
/// ```cpp
/// double z = -std::sqrt(2) * boost::math::erfc_inv(2 * level_.value());
/// return std::log(ef_.value()) / z;
/// ```
///
/// This port has no `erfc_inv` and uses
/// [`raffles::distributions`]' standard normal quantile instead, on the
/// identity `-sqrt(2) * erfc_inv(2p) = Phi^-1(p)`. That substitution is the
/// kind of thing that is right in the algebra and wrong by a factor of two in
/// the code, so it is checked against the definition it claims to satisfy:
/// `Phi(z)` must return the level.
///
/// **Results** (2026-09-22): round-tripping `Phi(Phi^-1(p))` at the five
/// confidence levels MEF models use returns `p` to better than `1e-12`, and
/// the resulting `SmallTree` probability lands on SCRAM's `0.160711`.
#[test]
fn the_normal_quantile_substitution_is_the_identity_it_claims() {
    // Round-trip through the crate's own CDF, which is a separate routine.
    for level in [0.5, 0.9, 0.95, 0.975, 0.99] {
        let z = raffles::distributions::Normal::new(0.0, 1.0)
            .unwrap()
            .ppf(level)
            .unwrap();
        let back = raffles::distributions::Normal::new(0.0, 1.0)
            .unwrap()
            .cdf(z);
        assert!(
            (back - level).abs() < 1e-12,
            "Phi(Phi^-1({level})) = {back}"
        );
    }
    // z_0.95 = 1.6448536269514722 is the standard table value.
    let z = raffles::distributions::Normal::new(0.0, 1.0)
        .unwrap()
        .ppf(0.95)
        .unwrap();
    assert!(
        (z - 1.644_853_626_951_472_2).abs() < 1e-10,
        "z_0.95 = {z}, expected 1.6448536269514722"
    );

    // And the end-to-end consequence: SmallTree's e1.
    let rate = Expression::lognormal_deviate(float(2.0e-5), float(3.0), float(0.95));
    let p = Expression::exponential(rate, Expression::MissionTime)
        .evaluate(&Parameters::new(), DEFAULT_MISSION_TIME)
        .unwrap();
    assert!(
        agrees(p, 0.160711, 5e-6),
        "SmallTree e1 from the ported chain: {p}, SCRAM 0.160711"
    );
}

/// **Methodology.** Upstream's `Validate()` for each deviate, case by case, in
/// upstream's own order — the log-normal checks the level, then the error
/// factor, then the mean, and a test that asserted a different order would be
/// asserting this port's behaviour rather than upstream's.
///
/// **Results** (2026-09-22): all twelve refusals hold, and each valid case
/// passes.
#[test]
fn the_domain_checks_are_upstreams() {
    let p = Parameters::new();
    let ok = |e: &Expression| e.validate(&p, DEFAULT_MISSION_TIME).is_ok();
    let bad = |e: &Expression, expect: &str| {
        let err = e
            .validate(&p, DEFAULT_MISSION_TIME)
            .expect_err("must be refused")
            .to_string();
        assert!(err.contains(expect), "expected `{expect}`, got: {err}");
    };

    // Uniform: min >= max.
    assert!(ok(&Expression::uniform_deviate(float(1.0), float(2.0))));
    bad(
        &Expression::uniform_deviate(float(2.0), float(2.0)),
        "min value is more than max",
    );
    // Normal and the two-argument log-normal: sigma <= 0.
    assert!(ok(&Expression::normal_deviate(float(1.0), float(0.1))));
    bad(
        &Expression::normal_deviate(float(1.0), float(0.0)),
        "cannot be negative or zero",
    );
    bad(
        &Expression::lognormal_deviate_normal(float(-12.0), float(-1.0)),
        "cannot be negative or zero",
    );
    // Log-normal, logarithmic flavour: level, then error factor, then mean.
    assert!(ok(&Expression::lognormal_deviate(
        float(1e-5),
        float(3.0),
        float(0.95)
    )));
    bad(
        &Expression::lognormal_deviate(float(1e-5), float(3.0), float(1.0)),
        "is not within (0, 1)",
    );
    bad(
        &Expression::lognormal_deviate(float(1e-5), float(1.0), float(0.95)),
        "cannot be less than 1",
    );
    bad(
        &Expression::lognormal_deviate(float(0.0), float(3.0), float(0.95)),
        "cannot be negative or zero",
    );
    // A level of 1 is refused BEFORE the error factor is looked at, which is
    // upstream's order: both are invalid here and the level is what is named.
    bad(
        &Expression::lognormal_deviate(float(1e-5), float(0.5), float(1.0)),
        "confidence level",
    );
    // Gamma and beta: both parameters positive.
    assert!(ok(&Expression::gamma_deviate(float(2.0), float(1e-6))));
    bad(
        &Expression::gamma_deviate(float(0.0), float(1e-6)),
        "k shape parameter",
    );
    bad(
        &Expression::gamma_deviate(float(2.0), float(0.0)),
        "theta scale parameter",
    );
    bad(
        &Expression::beta_deviate(float(0.0), float(1.0)),
        "alpha shape parameter",
    );
    bad(
        &Expression::beta_deviate(float(1.0), float(-1.0)),
        "beta shape parameter",
    );
    // Histogram: the length invariant, negative weights, and ordering.
    let hist = |b: &[f64], w: &[f64]| Expression::Histogram {
        boundaries: b.iter().copied().map(float).collect(),
        weights: w.iter().copied().map(float).collect(),
    };
    assert!(ok(&hist(&[0.0, 1.0, 2.0], &[1.0, 1.0])));
    bad(
        &hist(&[0.0, 1.0], &[1.0, 1.0]),
        "not equal to the number of intervals",
    );
    bad(&hist(&[0.0, 1.0, 2.0], &[1.0, -1.0]), "cannot be negative");
    bad(&hist(&[0.0, 2.0, 1.0], &[1.0, 1.0]), "are not increasing");
}

/// **Methodology.** `interval()` — the half of upstream's validation that only
/// bites when a deviate is present.
///
/// Upstream checks an argument twice: `EnsureNonNegative`/`EnsureWithin`
/// compare the *value* against the range, and then the whole `interval()`
/// against it. A normal deviate with a comfortable mean can still have a
/// domain reaching six sigma below zero, and upstream rejects it on the
/// second check. Without `interval()` that check cannot exist, which is why
/// it is ported here rather than left for the uncertainty analysis.
///
/// **Results** (2026-09-22):
///
/// | expression | interval |
/// |---|---|
/// | `Float(0.3)` | `[0.3, 0.3]` — upstream's default, a point |
/// | `exponential(...)` | `[0, 1]` — stated outright upstream |
/// | `uniform(0.1, 0.3)` | `[0.1, 0.3]` |
/// | `normal(0.5, 0.1)` | `[-0.1, 1.1]`, the six-sigma band — **not** a probability |
/// | `lognormal(mu -12, sigma 0.5)` | `(0, exp(-10.5)]` |
/// | `add(uniform(1,2), 10)` | `[11, 12]` — bounds propagate through the corners |
#[test]
fn intervals_are_upstreams_and_are_what_makes_the_domain_checks_bite() {
    let p = Parameters::new();
    let iv = |e: &Expression| e.interval(&p, DEFAULT_MISSION_TIME).unwrap();

    // The default: a point.
    let point = iv(&float(0.3));
    assert_eq!(point, Interval::closed(0.3, 0.3));
    assert!(point.is_probability());

    // The probability formulas state [0, 1] outright.
    let exp = Expression::exponential(float(1e-5), Expression::MissionTime);
    assert_eq!(iv(&exp), Interval::closed(0.0, 1.0));

    let u = iv(&Expression::uniform_deviate(float(0.1), float(0.3)));
    assert_eq!(u, Interval::closed(0.1, 0.3));
    assert!(
        u.is_probability(),
        "a uniform on [0.1, 0.3] is a probability"
    );

    // The case the whole mechanism exists for: a mean that looks fine, a
    // domain that is not a probability and is not even non-negative.
    let n = iv(&Expression::normal_deviate(float(0.5), float(0.1)));
    assert!((n.lower() - -0.1).abs() < 1e-12, "{n}");
    assert!((n.upper() - 1.1).abs() < 1e-12, "{n}");
    assert!(!n.is_probability(), "six sigma reaches outside [0, 1]");
    assert!(!n.is_non_negative(), "and below zero");

    // Log-normal: (0, exp(3 sigma + mu)].
    let l = iv(&Expression::lognormal_deviate_normal(
        float(-12.0),
        float(0.5),
    ));
    assert!(
        l.lower() == 0.0 && !l.contains(0.0),
        "left-open at zero: {l}"
    );
    assert!(
        (l.upper() - (-10.5f64).exp()).abs() / l.upper() < 1e-12,
        "{l}"
    );
    assert!(l.is_positive(), "a log-normal rate is strictly positive");

    // Bounds propagate through an operator by upstream's corner sweep.
    let sum = Expression::Add(vec![
        Expression::uniform_deviate(float(1.0), float(2.0)),
        float(10.0),
    ]);
    assert_eq!(iv(&sum), Interval::closed(11.0, 12.0));

    // And a histogram spans its own boundaries.
    let h = Expression::Histogram {
        boundaries: vec![float(0.0), float(1.0e-6), float(5.0e-6)],
        weights: vec![float(1.0), float(1.0)],
    };
    assert_eq!(iv(&h), Interval::closed(0.0, 5.0e-6));
}

/// **Methodology.** `is_deviate()` — upstream's `IsDeviate()`, which answers
/// yes for a deviate and otherwise asks its arguments.
///
/// This is the hook the uncertainty analysis will use to decide whether a
/// model needs sampling at all, so it must be true *through* an arbitrary
/// expression tree, not only at the leaf.
///
/// **Results** (2026-09-22): a bare float is not a deviate; a deviate is; an
/// exponential over a deviate rate is; an exponential over a literal rate is
/// not; and it holds through three levels of nesting.
#[test]
fn is_deviate_reaches_through_the_whole_expression() {
    assert!(!float(0.5).is_deviate());
    assert!(!Expression::exponential(float(1e-5), Expression::MissionTime).is_deviate());

    let deviate = Expression::lognormal_deviate(float(1e-5), float(3.0), float(0.95));
    assert!(deviate.is_deviate());

    let rate = Expression::exponential(deviate.clone(), Expression::MissionTime);
    assert!(rate.is_deviate(), "through one level");

    let nested = Expression::Add(vec![
        float(1.0),
        Expression::Mul(vec![float(2.0), Expression::Neg(std::sync::Arc::new(rate))]),
    ]);
    assert!(nested.is_deviate(), "through three levels");

    let no_deviate = Expression::Add(vec![
        float(1.0),
        Expression::Mul(vec![float(2.0), float(3.0)]),
    ]);
    assert!(!no_deviate.is_deviate());
}
