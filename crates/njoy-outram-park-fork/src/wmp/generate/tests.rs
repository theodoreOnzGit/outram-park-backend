// SPDX-License-Identifier: AGPL-3.0-only
//! Unit tests of the WMP generator's pieces (gh:#1390). The differential
//! against OpenMC is `tests/wmp_generate_vs_openmc.rs`.

use super::npy::{cdiv, count_peaks, eigvals, interp, linspace, lstsq, pairwise_sum, HouseholderQr};
use super::vectfit::{evaluate, label_conjugate_poles, vectfit};
use super::*;

#[test]
fn interp_matches_numpy_rules() {
    let xp = [0.0, 10.0, 20.0];
    let fp = [1.0, 2.0, 4.0];
    let y = interp(&[-1.0, 0.0, 5.0, 10.0, 15.0, 20.0, 25.0], &xp, &fp);
    assert_eq!(y, vec![1.0, 1.0, 1.5, 2.0, 3.0, 4.0, 4.0]);
}

#[test]
fn linspace_matches_numpy_rules() {
    assert_eq!(linspace(1.0, 2.0, 1), vec![1.0]);
    let y = linspace(0.0, 1.0, 5);
    assert_eq!(y, vec![0.0, 0.25, 0.5, 0.75, 1.0]);
    // i*step + start, last set to stop
    let y = linspace(0.1, 0.7, 4);
    assert_eq!(y[1], 1.0 * ((0.7 - 0.1) / 3.0) + 0.1);
    assert_eq!(y[3], 0.7);
}

#[test]
fn pairwise_sum_blocks() {
    let v: Vec<f64> = (0..300).map(|i| (i as f64).sqrt()).collect();
    let naive: f64 = v.iter().sum();
    assert!((pairwise_sum(&v) - naive).abs() < 1e-9 * naive);
    assert_eq!(pairwise_sum(&[1.0, 2.0]), 3.0);
}

#[test]
fn peaks_count_plateaus_once() {
    assert_eq!(count_peaks(&[0.0, 1.0, 0.0, 2.0, 2.0, 0.0, 1.0]), 2);
    assert_eq!(count_peaks(&[0.0, 1.0]), 0);
}

#[test]
fn complex_division_is_smiths() {
    let z = cdiv(Cf64::new(3.0, 4.0), Cf64::new(0.0, 1.0));
    assert_eq!((z.re, z.im), (4.0, -3.0));
    let z = cdiv(Cf64::new(1.0, 0.0), Cf64::new(2.0, 0.0));
    assert_eq!((z.re, z.im), (0.5, 0.0));
}

#[test]
fn qr_and_lstsq_solve_a_fit() {
    // y = 1 + 2 t on t = 0..4, column-major [1, t]
    let t = [0.0, 1.0, 2.0, 3.0, 4.0];
    let mut a = vec![1.0; 5];
    a.extend_from_slice(&t);
    let y: Vec<f64> = t.iter().map(|x| 1.0 + 2.0 * x).collect();
    let x = lstsq(5, 2, &a, &[y], f64::EPSILON * 5.0).unwrap();
    assert!((x[0][0] - 1.0).abs() < 1e-13 && (x[0][1] - 2.0).abs() < 1e-13);
    let qr = HouseholderQr::new(5, 2, a);
    // |R00| = sqrt(5), sign by DLARFG: beta = -sign(alpha) norm
    assert!((qr.r(0, 0) + 5.0_f64.sqrt()).abs() < 1e-14);
    assert_eq!(qr.r(1, 0), 0.0);
}

#[test]
fn eigvals_pairs_positive_imaginary_first() {
    let ev = eigvals(2, &[0.0, 1.0, -1.0, 0.0]).unwrap();
    assert_eq!(ev[0].im, 1.0);
    assert_eq!(ev[1].im, -1.0);
}

#[test]
fn conjugate_labels() {
    let p = [Cf64::new(1.0, 0.0), Cf64::new(2.0, -1.0), Cf64::new(2.0, 1.0)];
    assert_eq!(label_conjugate_poles(&p).unwrap(), vec![0, 1, 2]);
    assert!(label_conjugate_poles(&[Cf64::new(2.0, 1.0)]).is_err());
}

/// Vector fitting recovers a known rational function: two conjugate pole
/// pairs, two response vectors, started from displaced poles.
#[test]
fn vectfit_recovers_known_poles() {
    let truth = [Cf64::new(3.0, 0.2), Cf64::new(3.0, -0.2), Cf64::new(6.0, 0.5), Cf64::new(6.0, -0.5)];
    let res = vec![
        vec![Cf64::new(1.0, 0.5), Cf64::new(1.0, -0.5), Cf64::new(2.0, -1.0), Cf64::new(2.0, 1.0)],
        vec![Cf64::new(0.3, 0.1), Cf64::new(0.3, -0.1), Cf64::new(0.7, 0.2), Cf64::new(0.7, -0.2)],
    ];
    let s = linspace(1.0, 9.0, 400);
    let f = evaluate(&s, &truth, &res, &[]);
    let w = vec![vec![1.0; s.len()]; 2];
    let mut poles = vec![Cf64::new(2.0, -0.02), Cf64::new(2.0, 0.02), Cf64::new(8.0, -0.08), Cf64::new(8.0, 0.08)];
    let mut rms = f64::INFINITY;
    for _ in 0..20 {
        let r = vectfit(&f, &s, &poles, &w, 0, false, false).unwrap();
        poles = r.poles;
        rms = r.rms;
    }
    assert!(rms < 1e-10, "rms {rms}");
    for t in &truth {
        let d = poles.iter().map(|p| p.sub(*t)).map(|d| d.re.hypot(d.im)).fold(f64::INFINITY, f64::min);
        assert!(d < 1e-8, "pole {t:?} off by {d}");
    }
    assert!(vectfit(&f, &s, &poles, &w, 12, false, false).is_err());
    let none = vectfit(&f, &s, &[], &w, 0, false, false).unwrap();
    assert!(none.rms > 0.0);
}

/// The whole chain on a synthetic two-resonance "nuclide": fit, window,
/// and evaluate at 0 K against the pointwise input.
#[test]
fn synthetic_chain_reproduces_its_input() {
    let energy: Vec<f64> = linspace(1.0, 30.0, 600).into_iter().map(|x| x * x).collect();
    let bw = |e: f64, e0: f64, g: f64, a: f64| a * (g * g / 4.0) / ((e - e0) * (e - e0) + g * g / 4.0);
    let elastic: Vec<f64> = energy.iter().map(|&e| 4.0 + bw(e, 100.0, 2.0, 50.0) + bw(e, 500.0, 5.0, 20.0)).collect();
    let absorption: Vec<f64> =
        energy.iter().map(|&e| 0.1 / e.sqrt() + bw(e, 100.0, 2.0, 5.0) + bw(e, 500.0, 5.0, 1.0)).collect();
    let total: Vec<f64> = elastic.iter().zip(&absorption).map(|(a, b)| a + b).collect();
    let xs = PointwiseXs { name: "Syn1".into(), awr: 30.0, energy: energy.clone(), total, elastic, absorption, fission: None };
    let opts = VectfitOptions { orders: Some(vec![4, 6, 8, 10]), ..VectfitOptions::default() };
    let mp = vectfit_pointwise(&xs, Some(1), &opts).unwrap();
    assert_eq!(mp.mts, vec![2, 27]);
    let w = windowing(&mp, &WindowingOptions { n_win: Some(20), ..WindowingOptions::new(4) }).unwrap();
    assert_eq!(w.n_windows(), 20);
    assert!(w.poles_per_window() >= 0.0);
    let wmp = w.to_windowed_multipole();
    let mut worst = 0.0_f64;
    for (k, &e) in energy.iter().enumerate().skip(1).step_by(7) {
        if e >= w.e_max {
            break;
        }
        let x = wmp.evaluate(e, 0.0);
        worst = worst.max(((x.scatter - xs.elastic[k]) / xs.elastic[k]).abs());
    }
    assert!(worst < 2e-3, "worst relative miss {worst}");
    let searched = search_windowing(&mp, &SearchOptions { search_n_win: 3, search_cf_orders: vec![4, 3], ..SearchOptions::default() });
    assert!(searched.is_ok());
}
