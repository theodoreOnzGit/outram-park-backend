// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//! Tests of `regime_map_2d` and `by_regime` (gh:#1225): hash and generator against
//! reference vectors, the PSBT `preCHFTraceRegimeMap`, the concave-polygon test against an
//! independent winding-number test, and the `byRegime` accumulation.

use super::by_regime::ByRegimeModel;
use super::regime_map::RegimeInterpolationMode;
use super::regime_map_2d::{hash_little, hash_table_walk_order, Domain, Rand48, RegimeMap2D};

/// The `preCHFTraceRegimeMap` of `featureCases/1D_PSBT_SC/Phase_Ex1_12223`.
fn psbt_map() -> RegimeMap2D {
    let pts: Vec<(String, [f64; 2])> = [
        ("p0", [0.0, 0.0]),
        ("p1", [0.3, 0.0]),
        ("p2", [0.625, 0.0]),
        ("p3", [1.0, 0.0]),
        ("p4", [1.0, 6000.0]),
        ("p5", [0.625, 6000.0]),
        ("p6", [0.0, 6000.0]),
        ("p7", [0.3, 2000.0]),
        ("p8", [0.5, 2700.0]),
        ("p9", [0.625, 2700.0]),
    ]
    .iter()
    .map(|(n, p)| ((*n).to_string(), *p))
    .collect();
    let reg = |n: &str, l: &[&str]| (n.to_string(), l.iter().map(|s| (*s).to_string()).collect::<Vec<_>>());
    let regimes = vec![
        reg("dispersedBubble", &["p0", "p1", "p7", "p8", "p9", "p5", "p6"]),
        reg("capSlugBubble", &["p1", "p2", "p9", "p8", "p7"]),
        reg("annularMist", &["p2", "p3", "p4", "p5", "p9"]),
    ];
    RegimeMap2D::new(&pts, &regimes, 0.125, RegimeInterpolationMode::Linear).expect("map")
}

/// lookup3 test vectors of Bob Jenkins' `driver2` (`hashlittle`).
#[test]
fn hash_little_matches_the_lookup3_reference_vectors() {
    assert_eq!(hash_little(b"", 0), 0xdead_beef);
    assert_eq!(hash_little(b"", 0xdead_beef), 0xbd5b_7dde);
    assert_eq!(hash_little(b"Four score and seven years ago", 0), 0x1777_0551);
    assert_eq!(hash_little(b"Four score and seven years ago", 1), 0xcd62_8161);
}

/// First draws of `Foam::Random(69 (id + 1917))`, measured with a g++ -O3 program using the
/// same `Rand48` + `std::uniform_real_distribution<double>` definitions (printed at 17 digits).
#[test]
fn rand48_reproduces_the_gcc_draws() {
    let mut r = Rand48::new(69 * 1917);
    assert_eq!(r.sample01(), 0.044_378_883_114_365_004);
    assert_eq!(r.sample01(), 0.703_656_070_635_777_84);
    let mut r = Rand48::new(69 * 1918);
    assert_eq!(r.sample01(), 0.661_120_915_734_191_18);
    assert_eq!(r.sample01(), 0.824_591_485_997_200_89);
    let mut r = Rand48::new(69 * 1919);
    assert_eq!(r.sample01(), 0.277_862_948_819_678_6);
    assert_eq!(r.sample01(), 0.945_526_900_892_962_54);
}

#[test]
fn walk_order_is_a_permutation_in_hash_bucket_order() {
    let m = psbt_map();
    let mut names = m.walk_order_names();
    names.sort_unstable();
    assert_eq!(names, vec!["annularMist", "capSlugBubble", "dispersedBubble"]);
    let mut keyed: Vec<(usize, &str)> = m
        .regime_names()
        .iter()
        .map(|n| ((hash_little(n.as_bytes(), 0) as usize) & 127, n.as_str()))
        .collect();
    keyed.sort_by_key(|k| k.0);
    let expect: Vec<&str> = keyed.iter().map(|k| k.1).collect();
    assert_eq!(m.walk_order_names(), expect);
    assert_eq!(hash_table_walk_order(&[]), Vec::<usize>::new());
    // forcing growth past 0.8 * 128 keys keeps a permutation
    let many: Vec<String> = (0..200).map(|i| format!("regime{i}")).collect();
    let mut order = hash_table_walk_order(&many);
    order.sort_unstable();
    assert_eq!(order, (0..200).collect::<Vec<_>>());
}

#[test]
fn interior_points_belong_to_one_regime_and_the_weights_sum_to_one() {
    let m = psbt_map();
    let cases = [
        ((0.0, 1000.0), "dispersedBubble"),
        ((0.1, 100.0), "dispersedBubble"),
        ((0.45, 500.0), "capSlugBubble"),
        ((0.9, 5000.0), "annularMist"),
        ((0.2, 5900.0), "dispersedBubble"),
    ];
    for ((x, y), want) in cases {
        let w = m.regime_weights(x, y).expect("inside");
        assert_eq!(w[0].0, want, "({x}, {y})");
        let s: f64 = w.iter().map(|c| c.1).sum();
        assert!((s - 1.0).abs() < 1e-15, "weights {w:?}");
    }
}

/// On a shared boundary the distance is 0 so the coefficient is exactly 0.5 and the neighbour
/// takes the other 0.5; far from it (d >= 1) there is one regime.
#[test]
fn interpolation_band_straddles_a_shared_boundary() {
    let m = psbt_map();
    // the annularMist/capSlugBubble edge p2-p9 is x = 0.625 for y in [0, 2700]
    let w = m.regime_weights(0.625, 1000.0).expect("inside");
    assert_eq!(w.len(), 2);
    assert_eq!(w[0].1, 0.5);
    assert_eq!(w[1].1, 0.5);
    let w = m.regime_weights(0.45, 500.0).expect("inside");
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].1, 1.0);
    assert!(m.regime_weights(1.5, 100.0).is_err());
    assert!(m.regime_weights(-0.1, 100.0).is_err());
}

/// The concave `dispersedBubble` classification equals an independent winding-number test on a
/// grid (the ray of the translation is irrelevant away from vertices).
#[test]
fn concave_domain_classification_matches_a_winding_number_test() {
    let poly = [
        [0.0, 0.0],
        [0.3, 0.0],
        [0.3, 2000.0],
        [0.5, 2700.0],
        [0.625, 2700.0],
        [0.625, 6000.0],
        [0.0, 6000.0],
    ];
    let winding_inside = |x: f64, y: f64| -> bool {
        let mut wn = 0i32;
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let cross = (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0]);
            if a[1] <= y {
                if b[1] > y && cross > 0.0 {
                    wn += 1;
                }
            } else if b[1] <= y && cross < 0.0 {
                wn -= 1;
            }
        }
        wn != 0
    };
    let d = Domain::new(0, poly.to_vec());
    assert_eq!(d.sign(), 0, "the polygon is concave");
    let mut disagreements = 0;
    let mut inside = 0;
    for i in 0..97 {
        for j in 0..113 {
            let x = 0.0123 + f64::from(i) * 0.0107;
            let y = 17.3 + f64::from(j) * 53.1;
            let got = d.contains_point([x, y]);
            inside += usize::from(got);
            if got != winding_inside(x, y) {
                disagreements += 1;
            }
        }
    }
    assert!(inside > 1000 && inside < 97 * 113 - 1000, "the grid straddles the polygon ({inside})");
    assert_eq!(disagreements, 0);
}

#[test]
fn by_regime_model_follows_interpolate_value() {
    let names: Vec<String> = ["a", "b"].iter().map(|s| (*s).to_string()).collect();
    let m = ByRegimeModel::per_regime(&names, |n| Some(if n == "a" { 10.0 } else { 20.0 })).expect("models");
    assert_eq!(m.value(&[("a".to_string(), 1.0)], |v| *v), 10.0);
    assert_eq!(m.value(&[("a".to_string(), 0.25), ("b".to_string(), 0.75)], |v| *v), 0.25 * 10.0 + 0.75 * 20.0);
    assert_eq!(ByRegimeModel::Single(3.0).value(&[], |v| *v), 3.0);
    assert!(ByRegimeModel::<f64>::per_regime(&names, |_| None).is_err());
}
