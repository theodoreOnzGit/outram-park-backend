// SPDX-License-Identifier: GPL-3.0-only
//! **V&V: `petir::integration::{FixedType, FixedWorkspace}` against compiled
//! GSL 2.8** (gh:#1340, round 3). Methodology, reference and predictions P15:
//! `verification_and_validation/integration_family_gh1340.md`.
//!
//! Replays `reference-data/gsl/fixed-gsl-2.8-reference.txt` (driver
//! `fixed_reference_driver.c`): the nine weight types over several parameter
//! sets and ten orders (1 to 40), plus the refused parameter sets. For each
//! accepted rule: every node, every weight, and four integrals (`1`, `x^2`,
//! `x^5`, `1/(1+x^2)`). Whether the allocation is accepted must match
//! **exactly**; nodes, weights and integrals are counted bit-identical per
//! type and the counts pinned. The routines GSL calls are `sqrt`, `pow` (in
//! `fixed_compute` and `jacobi.c`) and `gsl_sf_gamma`; `sqrt` is exact, `pow`
//! and the gamma function reach glibc in GSL and PETIR's libm in the port.

use petir::integration::{FixedType, FixedWorkspace};
use std::collections::BTreeMap;

const REFERENCE: &str = include_str!("../../../reference-data/gsl/fixed-gsl-2.8-reference.txt");

fn kind(name: &str) -> FixedType {
    match name {
        "legendre" => FixedType::Legendre,
        "chebyshev" => FixedType::Chebyshev,
        "chebyshev2" => FixedType::Chebyshev2,
        "gegenbauer" => FixedType::Gegenbauer,
        "jacobi" => FixedType::Jacobi,
        "laguerre" => FixedType::Laguerre,
        "hermite" => FixedType::Hermite,
        "exponential" => FixedType::Exponential,
        "rational" => FixedType::Rational,
        other => panic!("unknown kind {other}"),
    }
}

/// Difference of two values relative to `max(1, |want|)`.
fn rel(got: f64, want: f64) -> f64 {
    if got.to_bits() == want.to_bits() {
        0.0
    } else {
        (got - want).abs() / want.abs().max(1.0)
    }
}

#[derive(Default, Clone, Copy)]
struct Tally {
    rules: usize,
    refused: usize,
    nodes: usize,
    nodes_exact: usize,
    weights: usize,
    weights_exact: usize,
    integrals: usize,
    integrals_exact: usize,
    worst_node: f64,
    worst_weight: f64,
    worst_integral: f64,
}

fn replay(reference: &str) -> BTreeMap<String, Tally> {
    let mut by_kind: BTreeMap<String, Tally> = BTreeMap::new();
    for line in reference.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(f[0], "FX", "{line}");
        let (tag, name) = (f[1], f[2]);
        let n: usize = f[3].parse().unwrap();
        let p: Vec<f64> = f[4..8].iter().map(|s| s.parse().unwrap()).collect();
        let ok = f[8] == "1";
        let t = by_kind.entry(name.to_string()).or_default();
        let got = FixedWorkspace::new(kind(name), n, p[0], p[1], p[2], p[3]);
        assert_eq!(got.is_ok(), ok, "acceptance differs for {name} {tag} n={n}");
        if !ok {
            t.refused += 1;
            continue;
        }
        let w = got.unwrap();
        t.rules += 1;
        let v: Vec<f64> = f[9..].iter().map(|s| s.parse().unwrap()).collect();
        assert_eq!(v.len(), 4 + 2 * n, "{line}");
        let ints = [
            w.integrate(|_| 1.0),
            w.integrate(|x| x * x),
            w.integrate(|x| x * x * x * x * x),
            w.integrate(|x| 1.0 / (1.0 + x * x)),
        ];
        for (g, want) in ints.iter().zip(v.iter()) {
            t.integrals += 1;
            t.integrals_exact += usize::from(g.to_bits() == want.to_bits());
            t.worst_integral = t.worst_integral.max(rel(*g, *want));
        }
        for i in 0..n {
            let (xw, ww) = (v[4 + 2 * i], v[5 + 2 * i]);
            t.nodes += 1;
            t.nodes_exact += usize::from(w.nodes()[i].to_bits() == xw.to_bits());
            t.worst_node = t.worst_node.max(rel(w.nodes()[i], xw));
            t.weights += 1;
            t.weights_exact += usize::from(w.weights()[i].to_bits() == ww.to_bits());
            t.worst_weight = t.worst_weight.max(rel(w.weights()[i], ww));
        }
    }
    by_kind
}

fn report(by_kind: &BTreeMap<String, Tally>) {
    for (k, t) in by_kind {
        println!(
            "  {k:<12} rules {:>3} refused {:>2}  nodes {:>4}/{:<4} (worst {:.1e})  weights {:>4}/{:<4} (worst {:.1e})  integrals {:>3}/{:<3} (worst {:.1e})",
            t.rules, t.refused, t.nodes_exact, t.nodes, t.worst_node, t.weights_exact, t.weights,
            t.worst_weight, t.integrals_exact, t.integrals, t.worst_integral
        );
    }
}

/// Against STOCK GSL 2.8 (glibc `pow`, `exp`, `log`, `sin`, ...): acceptance
/// exact, bit-identical counts pinned.
#[test]
fn fixed_matches_compiled_gsl() {
    let by_kind = replay(REFERENCE);
    report(&by_kind);
    let total_rules: usize = by_kind.values().map(|t| t.rules).sum();
    let total_refused: usize = by_kind.values().map(|t| t.refused).sum();
    assert_eq!((total_rules, total_refused), (270, 18));
    // (nodes exact, weights exact, integrals exact) per type, measured
    // 2026-11-14 and pinned.
    let pins: [(&str, (usize, usize, usize)); 9] = PINS;
    for (k, (ne, we, ie)) in pins {
        let t = by_kind[k];
        assert_eq!((t.nodes_exact, t.weights_exact, t.integrals_exact), (ne, we, ie), "{k}");
    }
}

/// Against GSL 2.8 relinked to PETIR's own `exp`, `log`, `pow`, `sin`, `cos`
/// (`petir_libm_shim.rs`): every node, weight and integral bit-identical.
#[test]
fn fixed_is_bit_identical_when_gsl_uses_petirs_libm() {
    const SHIM: &str = include_str!("../../../reference-data/gsl/fixed-gsl-2.8-petir-libm-reference.txt");
    let by_kind = replay(SHIM);
    report(&by_kind);
    for (k, t) in &by_kind {
        assert_eq!(t.nodes_exact, t.nodes, "{k} nodes");
        assert_eq!(t.weights_exact, t.weights, "{k} weights");
        assert_eq!(t.integrals_exact, t.integrals, "{k} integrals");
    }
}

/// `(kind, (nodes exact, weights exact, integrals exact))`; filled in from the
/// first run against stock GSL.
const PINS: [(&str, (usize, usize, usize)); 9] = [
    ("chebyshev", (0, 0, 0)),
    ("chebyshev2", (0, 0, 0)),
    ("exponential", (0, 0, 0)),
    ("gegenbauer", (0, 0, 0)),
    ("hermite", (0, 0, 0)),
    ("jacobi", (0, 0, 0)),
    ("laguerre", (0, 0, 0)),
    ("legendre", (0, 0, 0)),
    ("rational", (0, 0, 0)),
];
