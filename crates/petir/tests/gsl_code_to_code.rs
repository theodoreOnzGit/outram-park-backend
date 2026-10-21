//! **V&V — code-to-code against GSL 2.8 actually compiled and run.**
//!
//! # Why this exists alongside `gsl_cheb_suite.rs`
//!
//! That file ports GSL's own assertions. This one goes further: GSL's
//! `cheb/{init,eval,deriv,integ}.c` were **compiled with gcc and executed**,
//! and their output is committed at
//! `reference-data/gsl/cheb-gsl-2.8-reference.txt` (with the driver beside it,
//! so it can be regenerated rather than trusted). This test replays that run.
//!
//! The difference matters. A ported test suite checks the port against the
//! properties upstream chose to assert. A code-to-code comparison checks it
//! against what upstream's compiled code *actually computes*, at points nobody
//! selected in advance — including the ones where the two might disagree.
//!
//! # Result (2026-09-14)
//!
//! ```text
//!   6114 values compared, 4707 bit-identical (77.0 %)
//!
//!   coefficients     worst |delta| = 3.79e-17
//!   eval             worst |delta| = 8.88e-16
//!   eval_n(7)        worst |delta| = 2.22e-16
//!   integ eval       worst |delta| = 3.33e-16
//!   deriv eval       worst |delta| = 1.65e-14
//! ```
//!
//! The derivative figure is not a defect: differentiation amplifies
//! coefficient error by `n^2`, and `1600 * 1e-17 = 1.6e-14`.
//!
//! # The residual is localised, not hand-waved
//!
//! [`gsl_coefficients_through_our_evaluator_are_bit_exact`] feeds **GSL's own
//! coefficients** through **our** `eval`/`eval_err`/`deriv`/`integ` and gets
//! `5025 / 5025` bit-identical, worst `|delta| = 0`. So the evaluation
//! arithmetic is a bit-exact transcription and the whole residual lives in the
//! coefficients — i.e. in `cos()`, where `libm` (musl, required by `no_std`)
//! and glibc differ in the last ulp. That is the deliberate trade recorded in
//! the workspace manifest.
//!
//! If this file ever fails, read the two tests together: the isolation test
//! staying green while the pipeline test reddens means something changed in
//! coefficient generation, not in evaluation.

use petir::ChebSeries;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Where the committed GSL output lives, relative to the crate.
fn reference() -> Option<PathBuf> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/gsl/cheb-gsl-2.8-reference.txt");
    p.exists().then_some(p)
}

/// The five cases the driver dumps. Must match `cheb_reference_driver.c`.
fn cases() -> Vec<(&'static str, usize, f64, f64, fn(f64) -> f64)> {
    let pi = std::f64::consts::PI;
    vec![
        ("sin40", 40, -pi, pi, f64::sin as fn(f64) -> f64),
        ("exp12", 12, -1.0, 2.0, f64::exp),
        ("runge24", 24, -1.0, 1.0, |x: f64| 1.0 / (1.0 + 25.0 * x * x)),
        ("sin1", 1, -5.0, 5.0, f64::sin),
        ("sin2", 2, -5.0, 5.0, f64::sin),
    ]
}

/// Bounds on `|ours - gsl|`, set just above the 2026-09-14 measurement. These
/// are absolute because the quantities are `O(1)`; a relative bound would be
/// meaningless on coefficients that are `1e-17` numerical noise.
fn tolerance(label: &str) -> f64 {
    match label {
        // n^2 amplification of coefficient error through differentiation.
        "deriv eval" => 1.0e-13,
        _ => 5.0e-15,
    }
}

#[test]
fn petir_reproduces_gsl_2_8_output() {
    let Some(path) = reference() else {
        eprintln!("skipping: reference-data/gsl/ not present");
        return;
    };
    let text = std::fs::read_to_string(&path).expect("GSL reference is readable");

    let mut worst: BTreeMap<String, (f64, String)> = BTreeMap::new();
    let (mut n_cmp, mut exact) = (0usize, 0usize);

    for (tag, order, a, b, f) in cases() {
        let cs = ChebSeries::new(order, a, b, f).expect("init");
        let cd = cs.deriv();
        let ci = cs.integ();

        for line in text.lines() {
            let mut it = line.split_whitespace();
            let (t, kind) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
            if t != tag {
                continue;
            }
            let v: Vec<f64> = it.map(|s| s.parse().expect("numeric field")).collect();
            let mut check = |label: &str, ours: f64, theirs: f64, at: f64| {
                n_cmp += 1;
                if ours.to_bits() == theirs.to_bits() {
                    exact += 1;
                }
                let d = (ours - theirs).abs();
                let e = worst.entry(label.to_string()).or_insert((0.0, String::new()));
                if d > e.0 {
                    *e = (d, format!("{tag} at {at:.4e}"));
                }
            };
            match kind {
                "coeff" => check("coefficients", cs.coefficients()[v[0] as usize], v[1], v[0]),
                "eval" => {
                    let x = v[0];
                    check("eval", cs.eval(x), v[1], x);
                    let (r, err) = cs.eval_err(x);
                    check("eval_err result", r, v[2], x);
                    check("eval_err abserr", err, v[3], x);
                    check("deriv eval", cd.eval(x), v[4], x);
                    check("integ eval", ci.eval(x), v[5], x);
                }
                "evaln" => check("eval_n(7)", cs.eval_n(7, v[0]), v[1], v[0]),
                _ => {}
            }
        }
    }

    assert!(n_cmp > 6000, "only {n_cmp} values compared — reference truncated?");
    println!(
        "  vs GSL 2.8: {n_cmp} values, {exact} bit-identical ({:.1} %)",
        100.0 * exact as f64 / n_cmp as f64
    );
    for (label, (d, at)) in &worst {
        println!("    {label:<18} worst |delta| = {d:.3e}  ({at})");
        assert!(
            *d <= tolerance(label),
            "{label} differs from GSL by {d:.3e} at {at}, above the {:.1e} bound \
             (recorded 2026-09-14; see reference-data/gsl/README.md)",
            tolerance(label)
        );
    }
}

/// **Isolation:** GSL's own coefficients through our evaluator must be
/// *bit*-identical, which proves `eval`/`eval_err`/`deriv`/`integ` are exact
/// transcriptions and pins the pipeline's residual on `cos()` alone.
#[test]
fn gsl_coefficients_through_our_evaluator_are_bit_exact() {
    let Some(path) = reference() else {
        eprintln!("skipping: reference-data/gsl/ not present");
        return;
    };
    let text = std::fs::read_to_string(&path).expect("GSL reference is readable");

    let (mut n, mut exact) = (0usize, 0usize);
    for (tag, order, a, b, _f) in cases() {
        let mut gc = vec![0.0f64; order + 1];
        for line in text.lines() {
            let mut it = line.split_whitespace();
            if it.next() != Some(tag) || it.next() != Some("coeff") {
                continue;
            }
            let j: usize = it.next().unwrap().parse().unwrap();
            gc[j] = it.next().unwrap().parse().unwrap();
        }
        let cs = ChebSeries::from_coefficients(gc, a, b).expect("from_coefficients");
        let cd = cs.deriv();
        let ci = cs.integ();
        for line in text.lines() {
            let mut it = line.split_whitespace();
            if it.next() != Some(tag) || it.next() != Some("eval") {
                continue;
            }
            let v: Vec<f64> = it.map(|s| s.parse().unwrap()).collect();
            let (r, e) = cs.eval_err(v[0]);
            for (ours, theirs) in [
                (cs.eval(v[0]), v[1]),
                (r, v[2]),
                (e, v[3]),
                (cd.eval(v[0]), v[4]),
                (ci.eval(v[0]), v[5]),
            ] {
                n += 1;
                if ours.to_bits() == theirs.to_bits() {
                    exact += 1;
                }
            }
        }
    }
    println!("  GSL coefficients through our evaluator: {exact} / {n} bit-identical");
    assert_eq!(
        exact, n,
        "our eval/deriv/integ must be BIT-exact given identical coefficients — \
         {} of {n} differ. The pipeline test's residual is only attributable to \
         cos() while this holds.",
        n - exact
    );
}
