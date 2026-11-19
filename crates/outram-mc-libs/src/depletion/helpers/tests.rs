//! Analytic tests for the helpers port (gh:#1624); predictions in each
//! doc comment, all met exactly (2026-11-17).

use super::*;

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

fn set(y: f64) -> Vec<(String, f64)> {
    vec![("Xe135".to_owned(), y), ("I135".to_owned(), 2.0 * y)]
}

/// U-235-like table: thermal 0.0253 eV, 500 keV, 14 MeV.
fn table() -> YieldTable {
    YieldTable { energies: vec![0.0253, 5.0e5, 1.4e7], sets: vec![set(1.0), set(3.0), set(7.0)] }
}

/// 3.2 GW at 200 MeV per source neutron: `3.2e9 / (2e8 · 1.602176634e-19)`
/// = 9.98641e19 source/s. Zero energy is refused.
#[test]
fn energy_normalisation() {
    let f = energy_normalization_factor(3.2e9, 2.0e8).unwrap();
    assert_eq!(f, 3.2e9 / (2.0e8 * JOULE_PER_EV));
    assert!(energy_normalization_factor(1.0, 0.0).is_err());
    assert_eq!(source_rate_factor(7.0), 7.0);
}

/// Q from the chain's first fission reaction; `energy += r · q`.
#[test]
fn chain_fission_q() {
    let chain = vec![
        ("U235".to_owned(), vec![("(n,gamma)".to_owned(), 6.5e6), ("fission".to_owned(), 1.93e8)]),
        ("Xe135".to_owned(), vec![("(n,gamma)".to_owned(), 7.9e6)]),
    ];
    let idx = vec!["Xe135".to_owned(), "U235".to_owned()];
    let q = fission_q_vector(&chain, &idx);
    assert_eq!(q, vec![0.0, 1.93e8]);
    assert_eq!(chain_fission_energy(1.0, &[5.0, 2.0], &q), 1.0 + 2.0 * 1.93e8);
}

/// Exact match, else nearest; a tie takes the first (Python `min`).
#[test]
fn constant_yields_pick_exact_then_nearest() {
    let t = table();
    assert_eq!(constant_yields(&t, 5.0e5).unwrap(), set(3.0));
    assert_eq!(constant_yields(&t, 1.0).unwrap(), set(1.0));
    let tie = YieldTable { energies: vec![1.0, 3.0], sets: vec![set(1.0), set(2.0)] };
    assert_eq!(constant_yields(&tie, 2.0).unwrap(), set(1.0));
}

/// Upstream defaults (cutoff 112 eV, 0.0253 eV, 500 keV) on the table:
/// both exact, so Split{1, 3}; 70 % thermal gives `0.7·1 + 0.3·3 = 1.6`.
/// A table entirely above the cutoff is the lowest set, constant.
#[test]
fn cutoff_helper() {
    let y = cutoff_yields(&table(), 112.0, 0.0253, 5.0e5).unwrap();
    assert_eq!(y, CutoffYields::Split { thermal: set(1.0), fast: set(3.0) });
    let w = cutoff_weighted(&y, 0.7, 0.3);
    assert!((w[0].1 - 1.6).abs() < 1e-15 && (w[1].1 - 3.2).abs() < 1e-15);
    let hi = YieldTable { energies: vec![5.0e5, 1.4e7], sets: vec![set(3.0), set(7.0)] };
    assert_eq!(cutoff_yields(&hi, 112.0, 0.0253, 1.0e6).unwrap(), CutoffYields::Constant(set(3.0)));
    // Not tabulated at 1 MeV: nearest at or above the cutoff is 500 keV.
    assert_eq!(cutoff_yields(&table(), 112.0, 0.0253, 1.0e6).unwrap(), CutoffYields::Split { thermal: set(1.0), fast: set(3.0) });
    assert!(cutoff_yields(&table(), 1.0e6, 0.0253, 5.0e5).is_err());
}

/// Average fission energy 250 keV: between 0.0253 eV and 500 keV,
/// `fast_frac = (2.5e5 - 0.0253)/(5e5 - 0.0253)`; clamped outside.
#[test]
fn averaged_helper_interpolates() {
    let t = table();
    let f = (2.5e5 - 0.0253) / (5.0e5 - 0.0253);
    let y = averaged_yields(&t, 2.5e5).unwrap();
    assert!((y[0].1 - ((1.0 - f) * 1.0 + f * 3.0)).abs() < 1e-15);
    assert_eq!(averaged_yields(&t, 1e-3).unwrap(), set(1.0));
    assert_eq!(averaged_yields(&t, 2e7).unwrap(), set(7.0));
    let y = averaged_yields(&t, 1.0e6).unwrap();
    let f = (1.0e6 - 5.0e5) / (1.4e7 - 5.0e5);
    assert!((y[0].1 - ((1.0 - f) * 3.0 + f * 7.0)).abs() < 1e-14);
}
