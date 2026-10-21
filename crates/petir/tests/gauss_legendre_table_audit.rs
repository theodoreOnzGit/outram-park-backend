//! Independent audit of every one of the 928 tabulated Gauss-Legendre values
//! in [`petir::integration::gauss_legendre_tables`].
//!
//! # Why this exists
//!
//! Those tables were transcribed from `peroxide` 0.41.2. A transcription can
//! go wrong in ways an integration test will not catch: a wrong digit in one
//! node shifts the integral by less than the rule's own error at that order,
//! so the rule keeps looking plausible while being silently worse than
//! advertised. This checks every value against a construction that shares no
//! code and no data with the table.
//!
//! **It found three wrong node values.** See "Corrections" below; without this
//! audit they would have shipped.
//!
//! # Methodology
//!
//! `P_n` and `P_n'` are evaluated by **Bonnet's three-term recurrence on
//! values**, which is numerically stable at every order — deliberately not by
//! [`petir::poly::dense::legendre`], which expands the coefficients and loses
//! about a digit per two degrees, and so cannot audit orders above ~12.
//!
//! - **Nodes.** Each tabulated node is Newton-polished against `P_n` and must
//!   move by less than 1e-14. A node that is genuinely a root of `P_n` does
//!   not move.
//! - **Weights.** Each tabulated weight is compared against
//!   `w_k = 2 / ((1 - x_k^2) P_n'(x_k)^2)` evaluated at the **polished** node.
//!   Evaluating it at the tabulated node instead makes a bad node look like a
//!   bad weight, which it did on the first run of this audit — the weights
//!   were fine throughout.
//!
//! This is a genuine second source rather than a restatement: the recurrence
//! comes from the definition of `P_n`, the table came from `peroxide`.
//!
//! # Corrections applied to the ported table
//!
//! Three node values in upstream are wrong. Each appears twice, once per sign,
//! so six entries were corrected. All six replacements agree with the standard
//! published Gauss-Legendre tables **and** with this audit's Newton polish, to
//! every digit printed:
//!
//! | order | upstream | corrected | error |
//! |---|---|---|---|
//! | 11 | `0.519096129110681` | `0.5190961292068118` | 9.613e-11 |
//! | 12 | `0.36783149891818` | `0.3678314989981802` | 8.000e-11 |
//! | 12 | `0.125333408511469` | `0.1252334085114689` | **1.000e-04** |
//!
//! The third is the serious one and is a plain single-digit typo — a `2`
//! became a `3` in the fourth decimal, with every remaining digit correct. It
//! costs upstream's 12-point rule about ten significant figures: a caller
//! asking for Gauss-Legendre at order 12 gets roughly five correct digits
//! instead of fifteen.
//!
//! Every other value, at every other order, passes. This is reported upstream;
//! see the crate NOTICE.
//!
//! # Results
//!
//! With the corrections applied: **0 bad entries out of 928**, measured
//! 2026-09-15. Worst node movement under Newton polish **5.551e-16**, worst
//! weight relative deviation **3.169e-14**.

use petir::integration::gauss_legendre_tables::{gauss_legendre_table, MAX_ORDER, MIN_ORDER};

/// `P_n(x)` and `P_n'(x)` by Bonnet's recurrence, stable at every order.
///
/// `P_n' = n (x P_n - P_{n-1}) / (x^2 - 1)`, which is exact away from the
/// endpoints — and every Gauss-Legendre node is strictly interior.
fn legendre_value_and_derivative(n: usize, x: f64) -> (f64, f64) {
    if n == 0 {
        return (1.0, 0.0);
    }
    if n == 1 {
        return (x, 1.0);
    }
    let mut p0 = 1.0f64;
    let mut p1 = x;
    for k in 1..n {
        let kf = k as f64;
        let p2 = ((2.0 * kf + 1.0) * x * p1 - kf * p0) / (kf + 1.0);
        p0 = p1;
        p1 = p2;
    }
    let dp = (n as f64) * (x * p1 - p0) / (x * x - 1.0);
    (p1, dp)
}

/// Newton-polish `x` onto the nearest root of `P_n`.
fn polish(n: usize, x: f64) -> f64 {
    let mut r = x;
    for _ in 0..60 {
        let (p, dp) = legendre_value_and_derivative(n, r);
        if dp == 0.0 {
            break;
        }
        let step = p / dp;
        r -= step;
        if step.abs() < 1e-17 {
            break;
        }
    }
    r
}

/// Every tabulated node is a root of `P_n`, and every tabulated weight is the
/// Gauss weight at that root.
#[test]
fn every_tabulated_value_is_correct() {
    let mut checked = 0usize;
    let mut worst_node = 0.0f64;
    let mut worst_weight = 0.0f64;

    for n in MIN_ORDER..=MAX_ORDER {
        let (nodes, weights) = gauss_legendre_table(n).expect("in range");
        assert_eq!(nodes.len(), n, "node count at order {n}");
        assert_eq!(weights.len(), n, "weight count at order {n}");

        for (k, (&x, &w)) in nodes.iter().zip(weights.iter()).enumerate() {
            checked += 2;
            let r = polish(n, x);
            let node_err = (r - x).abs();
            if node_err > worst_node {
                worst_node = node_err;
            }
            if node_err > 1e-14 {
                panic!("order {n} node [{k}] is not a root of P_{n}: tabulated {x:.17}, polished {r:.17}, error {node_err:e}");
            }

            let (_, dp) = legendre_value_and_derivative(n, r);
            let expect = 2.0 / ((1.0 - r * r) * dp * dp);
            let rel = ((w - expect) / expect).abs();
            if rel > worst_weight {
                worst_weight = rel;
            }
            if rel > 1e-12 {
                panic!("order {n} weight [{k}]: tabulated {w:.17}, expected {expect:.17}, relative {rel:e}");
            }
        }
    }

    assert_eq!(checked, 928, "expected 928 values, checked {checked}");
    assert!(worst_node < 1e-14, "worst node movement {worst_node:e}");
    assert!(
        worst_weight < 1e-12,
        "worst weight deviation {worst_weight:e}"
    );
}

/// Each table has the structure a Gauss-Legendre rule must have.
///
/// Nodes ascending, strictly inside `(-1, 1)`, symmetric about zero; weights
/// strictly positive, symmetric, and summing to 2 — the integral of 1 over
/// the reference interval.
///
/// # Results
///
/// Holds at every order 2 through 30. Worst weight-sum deviation from 2 is
/// **9.992e-15**, measured 2026-09-15.
#[test]
fn every_table_has_the_structure_a_gauss_rule_must_have() {
    let mut worst_sum = 0.0f64;
    for n in MIN_ORDER..=MAX_ORDER {
        let (nodes, weights) = gauss_legendre_table(n).expect("in range");

        for (a, b) in nodes.iter().zip(nodes.iter().skip(1)) {
            assert!(a < b, "order {n}: nodes are not ascending");
        }
        for x in nodes {
            assert!(x.abs() < 1.0, "order {n}: node {x} is not interior");
        }
        for (a, b) in nodes.iter().zip(nodes.iter().rev()) {
            assert!((a + b).abs() < 1e-15, "order {n}: nodes are not symmetric");
        }
        for w in weights {
            assert!(*w > 0.0, "order {n}: weight {w} is not positive");
        }
        for (a, b) in weights.iter().zip(weights.iter().rev()) {
            assert!(
                (a - b).abs() < 1e-15,
                "order {n}: weights are not symmetric"
            );
        }
        let s: f64 = weights.iter().sum();
        worst_sum = worst_sum.max((s - 2.0).abs());
    }
    assert!(
        worst_sum < 1e-13,
        "worst weight-sum deviation {worst_sum:e}"
    );
}

/// Orders outside the table return `None` rather than panicking.
#[test]
fn out_of_range_orders_return_none() {
    assert!(gauss_legendre_table(0).is_none());
    assert!(gauss_legendre_table(1).is_none());
    assert!(gauss_legendre_table(MAX_ORDER + 1).is_none());
    assert!(gauss_legendre_table(usize::MAX).is_none());
    for n in MIN_ORDER..=MAX_ORDER {
        assert!(gauss_legendre_table(n).is_some(), "order {n} missing");
    }
}
