//! # Expressions, verified against SCRAM's computed basic-event probabilities
//!
//! A fault tree's leaves are rarely bare numbers. `HIPPS`, in upstream's own
//! `input/` suite, defines **every** basic event by a `<periodic-test>` or a
//! `<GLM>` expression — its probabilities appear nowhere in the model as
//! literals. SCRAM computes them and prints them in its report, which makes
//! that model an exact oracle for
//! [`raffles::scram::expression`].
//!
//! This is a stronger check than it looks. Every other test in this crate
//! takes those probabilities from the oracle and uses them; this one
//! *derives* them from the model's own parameters and must land on the same
//! values to six significant figures.
//!
//! | | |
//! |---|---|
//! | Upstream | <https://github.com/rakhimov/scram> @ `b85b7894` (2019-07-03) |
//! | Oracle | `reference-data/scram/oracle.txt`, the `EVENT` records for `HIPPS` |
//! | Mission time | SCRAM's default, 8760 h — `HIPPS` sets none |

use raffles::scram::expression::{Expression, Parameters, DEFAULT_MISSION_TIME};
use std::collections::HashMap;

/// SCRAM's computed value for each `HIPPS` basic event, from its report.
fn hipps_oracle() -> HashMap<String, f64> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/scram/oracle.txt");
    let text = std::fs::read_to_string(path).expect("oracle fixture");
    let mut out = HashMap::new();
    let mut in_hipps = false;
    for line in text.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.is_empty() {
            continue;
        }
        match f[0] {
            "MODEL" => in_hipps = f[1] == "HIPPS/HIPPS",
            "EVENT" if in_hipps => {
                out.insert(f[1].to_string(), f[2].parse().unwrap());
            }
            _ => {}
        }
    }
    assert_eq!(out.len(), 9, "HIPPS has nine basic events");
    out
}

/// **Methodology.** Rebuild each of `HIPPS`'s nine basic-event expressions
/// from the model's own literals and evaluate them, then compare against the
/// values SCRAM computed and reported.
///
/// The expressions, transcribed from `input/HIPPS/HIPPS.xml`:
///
/// | event | expression | arguments |
/// |---|---|---|
/// | `LogicSolverFailure` | `<GLM>` | gamma 0, lambda 1e-6, mu 0.1, mission time |
/// | `PSH1Failure`, `PSH2Failure`, `PSH3Failure` | `<periodic-test>` | lambda 7e-7, tau 720, theta 0, mission time |
/// | `PSHCCF` | `<periodic-test>` | lambda 3.5e-8, tau 720, theta 0, mission time |
/// | `SV1Failure`, `SV2Failure` | `<periodic-test>` | lambda 2.6e-6, tau 1440, theta 0, mission time |
/// | `SDV1Failure`, `SDV2Failure` | `<periodic-test>` | lambda 4.1e-6, tau 2160, theta 0, mission time |
///
/// The four-argument `<periodic-test>` is upstream's `InstantRepair` flavour.
/// Mission time is SCRAM's default 8760 h, which `HIPPS` does not override.
///
/// Tolerance is `5e-6` **relative**, the resolution of upstream's
/// 6-significant-figure report.
///
/// **Result** (2026-09-22): all nine reproduced.
///
/// | event | RAFFLES | SCRAM |
/// |---|---|---|
/// | `PSHCCF` | 4.199991e-6 | 4.199990e-6 |
/// | `PSH1Failure` (and 2, 3) | 8.399647e-5 | 8.399650e-5 |
/// | `LogicSolverFailure` | 9.999900e-6 | 9.999900e-6 |
/// | `SV1Failure` (and 2) | 3.119513e-4 | 3.119510e-4 |
/// | `SDV1Failure` (and 2) | 4.918790e-4 | 4.918790e-4 |
///
/// Note `PSH1Failure` at 8.4e-5 against a rate of 7e-7 over 8760 h: a plain
/// exponential would give 6.1e-3, seventy times higher. The periodic test is
/// doing real work here, and getting its sawtooth wrong would be visible.
#[test]
fn hipps_basic_event_probabilities_match_scrams_computed_values() {
    let oracle = hipps_oracle();
    let params = Parameters::new();
    let t = || Expression::MissionTime;
    let f = Expression::float;

    let periodic = |lambda: f64, tau: f64| {
        Expression::periodic_test_instant_repair(f(lambda), f(tau), f(0.0), t())
    };
    let cases: Vec<(&str, Expression)> = vec![
        (
            "LogicSolverFailure",
            Expression::glm(f(0.0), f(1e-6), f(0.1), t()),
        ),
        ("PSH1Failure", periodic(7e-7, 720.0)),
        ("PSH2Failure", periodic(7e-7, 720.0)),
        ("PSH3Failure", periodic(7e-7, 720.0)),
        ("PSHCCF", periodic(3.5e-8, 720.0)),
        ("SV1Failure", periodic(2.6e-6, 1440.0)),
        ("SV2Failure", periodic(2.6e-6, 1440.0)),
        ("SDV1Failure", periodic(4.1e-6, 2160.0)),
        ("SDV2Failure", periodic(4.1e-6, 2160.0)),
    ];
    assert_eq!(cases.len(), oracle.len());

    for (name, expression) in &cases {
        expression
            .validate(&params, DEFAULT_MISSION_TIME)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let ours = expression
            .evaluate(&params, DEFAULT_MISSION_TIME)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        let theirs = oracle[*name];
        println!("{name:<20} ours {ours:.6e}  scram {theirs:.6e}");
        assert!(
            (ours - theirs).abs() / theirs.abs().max(f64::MIN_POSITIVE) < 5e-6,
            "{name}: ours {ours}, SCRAM {theirs}"
        );
    }
    println!("reproduced {} computed probabilities", cases.len());
}

/// **Methodology.** Closed forms and limiting cases for each formula, where
/// no upstream model exercises them. These would catch a defect that happened
/// to agree with `HIPPS`'s particular arguments.
///
/// **Result** (2026-09-22): all hold.
#[test]
fn each_formula_matches_its_closed_form() {
    let p = Parameters::new();
    let f = Expression::float;
    let at = |e: &Expression, t: f64| e.evaluate(&p, t).unwrap();

    // Exponential: 1 - exp(-lambda t), and 0 at t = 0.
    let e = Expression::exponential(f(1e-3), Expression::MissionTime);
    assert!((at(&e, 1000.0) - (1.0 - (-1.0f64).exp())).abs() < 1e-15);
    assert_eq!(at(&e, 0.0), 0.0);

    // GLM with gamma = 0 and mu = 0 degenerates to the exponential: with no
    // repair and no failure on demand there is nothing else left.
    let glm = Expression::glm(f(0.0), f(1e-3), f(0.0), Expression::MissionTime);
    let exp = Expression::exponential(f(1e-3), Expression::MissionTime);
    assert!((at(&glm, 500.0) - at(&exp, 500.0)).abs() < 1e-12);

    // GLM at t = 0 is the failure-on-demand probability alone.
    let glm = Expression::glm(f(0.25), f(1e-3), f(0.1), Expression::MissionTime);
    assert!((at(&glm, 0.0) - 0.25).abs() < 1e-12);

    // Weibull is exactly 0 before the shift, and with beta = 1 it is the
    // exponential with rate 1/alpha.
    let w = Expression::weibull(f(1000.0), f(1.0), f(100.0), Expression::MissionTime);
    assert_eq!(at(&w, 100.0), 0.0);
    assert!((at(&w, 1100.0) - (1.0 - (-1.0f64).exp())).abs() < 1e-15);

    // Periodic test with instant repair: a SAWTOOTH. Just before a test the
    // probability has built up over a full interval; just after, it is near
    // zero again. Upstream's `time_after_test ? : tau` makes landing exactly
    // on a test mean a full interval, not none.
    let pt = Expression::periodic_test_instant_repair(
        f(1e-3),
        f(100.0),
        f(0.0),
        Expression::MissionTime,
    );
    // Measured 2026-09-22: 0.095072 just before a test, 0.095163 exactly on
    // one, 0.000100 just after — a three-order-of-magnitude reset.
    let just_before = at(&pt, 99.9);
    let exactly_on = at(&pt, 100.0);
    let just_after = at(&pt, 100.1);
    println!(
        "sawtooth: {just_before:.6} at 99.9, {exactly_on:.6} at 100, {just_after:.6} at 100.1"
    );
    assert!(just_after < just_before / 100.0, "the test must reset it");
    assert!(
        (exactly_on - (1.0 - (-0.1f64).exp())).abs() < 1e-12,
        "landing on a test means a full interval elapsed"
    );
    // Before the first test it is a plain exponential.
    let pt_late = Expression::periodic_test_instant_repair(
        f(1e-3),
        f(100.0),
        f(500.0),
        Expression::MissionTime,
    );
    assert!((at(&pt_late, 200.0) - at(&exp, 200.0)).abs() < 1e-15);

    // Periodic test with a repair rate: a very fast repair should approach
    // the instant-repair answer, and a very slow one should stay above it,
    // since failures then persist across tests.
    let fast = Expression::periodic_test_instant_test(
        f(1e-3),
        f(1e6),
        f(100.0),
        f(0.0),
        Expression::MissionTime,
    );
    let slow = Expression::periodic_test_instant_test(
        f(1e-3),
        f(1e-9),
        f(100.0),
        f(0.0),
        Expression::MissionTime,
    );
    // Measured 2026-09-22: instant 0.04877058, fast repair 0.04877058
    // (indistinguishable), slow repair 0.29531187 — six times higher,
    // because failures then persist across every test.
    let (instant, fast_v, slow_v) = (at(&pt, 350.0), at(&fast, 350.0), at(&slow, 350.0));
    println!("instant {instant:.8}, fast repair {fast_v:.8}, slow repair {slow_v:.8}");
    assert!(
        (fast_v - instant).abs() < 1e-6,
        "fast repair ≈ instant repair"
    );
    assert!(slow_v > instant, "a slow repair cannot beat an instant one");
}

/// **Methodology.** The arithmetic, boolean and conditional operators, and
/// the domain checks upstream's `Validate()` enforces.
///
/// **Result** (2026-09-22): all hold.
#[test]
fn the_operators_and_their_domain_checks_behave() {
    let f = Expression::float;
    let p = Parameters::new();
    let v = |e: Expression| e.evaluate(&p, 8760.0).unwrap();

    assert_eq!(v(Expression::Add(vec![f(1.0), f(2.0), f(3.0)])), 6.0);
    assert_eq!(v(Expression::Sub(vec![f(10.0), f(3.0), f(2.0)])), 5.0);
    assert_eq!(v(Expression::Mul(vec![f(2.0), f(3.0), f(4.0)])), 24.0);
    assert_eq!(v(Expression::Div(vec![f(24.0), f(3.0), f(2.0)])), 4.0);
    assert_eq!(v(Expression::Neg(f(3.0).into())), -3.0);
    assert_eq!(v(Expression::Abs(f(-3.0).into())), 3.0);
    assert_eq!(v(Expression::Min(vec![f(3.0), f(1.0), f(2.0)])), 1.0);
    assert_eq!(v(Expression::Max(vec![f(3.0), f(1.0), f(2.0)])), 3.0);
    assert_eq!(v(Expression::Mean(vec![f(1.0), f(2.0), f(6.0)])), 3.0);
    assert_eq!(v(Expression::Pow(f(2.0).into(), f(10.0).into())), 1024.0);
    assert_eq!(v(Expression::Sqrt(f(16.0).into())), 4.0);
    assert_eq!(v(Expression::Mod(f(17.0).into(), f(5.0).into())), 2.0);
    assert_eq!(v(Expression::Floor(f(1.7).into())), 1.0);
    assert_eq!(v(Expression::Ceil(f(1.2).into())), 2.0);
    assert_eq!(v(Expression::Round(f(1.5).into())), 2.0);
    assert_eq!(v(Expression::Trunc(f(-1.7).into())), -1.0);
    assert_eq!(
        v(Expression::Log(Expression::Exp(f(2.0).into()).into())),
        2.0
    );
    assert_eq!(v(Expression::Log10(f(1000.0).into())), 3.0);

    assert_eq!(v(Expression::Not(f(0.0).into())), 1.0);
    assert_eq!(v(Expression::And(vec![f(1.0), f(0.0)])), 0.0);
    assert_eq!(v(Expression::Or(vec![f(1.0), f(0.0)])), 1.0);
    assert_eq!(v(Expression::Eq(f(2.0).into(), f(2.0).into())), 1.0);
    assert_eq!(v(Expression::Lt(f(1.0).into(), f(2.0).into())), 1.0);
    assert_eq!(v(Expression::Gt(f(1.0).into(), f(2.0).into())), 0.0);
    assert_eq!(
        v(Expression::Ite {
            condition: f(1.0).into(),
            consequent: f(7.0).into(),
            alternate: f(9.0).into(),
        }),
        7.0
    );

    // Mission time and parameters.
    assert_eq!(v(Expression::MissionTime), 8760.0);
    let mut params = Parameters::new();
    params.insert("rate".to_string(), 1e-4);
    assert_eq!(
        Expression::Parameter("rate".to_string())
            .evaluate(&params, 8760.0)
            .unwrap(),
        1e-4
    );
    // An undefined parameter is refused, not defaulted.
    assert!(Expression::Parameter("ghost".to_string())
        .evaluate(&params, 8760.0)
        .is_err());

    // Division and modulus by zero are refused rather than returning inf/NaN.
    assert!(Expression::Div(vec![f(1.0), f(0.0)])
        .evaluate(&p, 8760.0)
        .is_err());
    assert!(Expression::Mod(f(1.0).into(), f(0.0).into())
        .evaluate(&p, 8760.0)
        .is_err());

    // Upstream's Validate() domains.
    let t = Expression::MissionTime;
    assert!(Expression::exponential(f(-1.0), t.clone())
        .validate(&p, 8760.0)
        .is_err());
    assert!(
        Expression::glm(f(0.0), f(0.0), f(0.1), t.clone())
            .validate(&p, 8760.0)
            .is_err(),
        "GLM needs a POSITIVE failure rate"
    );
    assert!(
        Expression::glm(f(1.5), f(1e-3), f(0.1), t.clone())
            .validate(&p, 8760.0)
            .is_err(),
        "gamma is a probability"
    );
    assert!(
        Expression::weibull(f(0.0), f(1.0), f(0.0), t.clone())
            .validate(&p, 8760.0)
            .is_err(),
        "Weibull needs a positive scale"
    );
    assert!(
        Expression::periodic_test_instant_repair(f(1e-3), f(0.0), f(0.0), t.clone())
            .validate(&p, 8760.0)
            .is_err(),
        "the time between tests must be positive"
    );
    assert!(Expression::exponential(f(1e-3), t)
        .validate(&p, 8760.0)
        .is_ok());
}
