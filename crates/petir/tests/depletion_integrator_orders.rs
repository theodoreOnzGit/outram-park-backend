// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **V&V: observed order of convergence of PETIR's eight depletion
//! integrators against an exact solution** (gh:#950).
//!
//! # Methodology
//!
//! The three-nuclide constant-power problem in `depletion_common`: the flux
//! depends on the composition, the burnup matrices at different compositions
//! do not commute, and the exact solution is closed-form. Each integrator
//! runs [`DepletionIntegrator::integrate`] with CRAM48 over `T = 1 s` in `N`
//! uniform steps, `N = 4, 8, ..., 1024`. The error is the largest relative
//! error of the three densities at `T`; the observed order between `N` and
//! `2N` is `log2(e(N) / e(2N))`.
//!
//! The predicted orders were committed before this test was first run
//! (`verification_and_validation/depletion/petir_integrators_2026_10_17.md`,
//! section 1, commit `cf3ddd8849`).
//!
//! **Ablation:** CF4 with its two final exponentials applied in the order
//! upstream's DOCSTRING implies (the pre-move `outram-mc-libs` CF4), built
//! from the same PETIR pieces; the same harness with the CODE order is
//! checked to reproduce `Integrator::Cf4` bit for bit first.
//!
//! # Results
//!
//! See `RECORDED` below and the V&V record, section 3.

mod depletion_common;

use depletion_common::{exact, FixedRate, N0, SOURCE};
use petir::depletion::matrix_funcs::{cf4_f1, cf4_f2, cf4_f3, cf4_f4};
use petir::depletion::{
    CramSolver, DepSystemSolver, DepletionIntegrator, DepletionMatrix, Integrator,
    TransportOperator,
};

const T: f64 = 1.0;
const NS: [usize; 9] = [4, 8, 16, 32, 64, 128, 256, 512, 1024];

fn final_state(method: Integrator, n: usize) -> Vec<f64> {
    let mut op = FixedRate::default();
    let dt = T / n as f64;
    let mut integ =
        DepletionIntegrator::new(method, CramSolver::Cram48, vec![dt; n], vec![SOURCE; n]).unwrap();
    let recs = integ.integrate(&mut op, vec![N0.to_vec()], false).unwrap();
    recs.last().unwrap().n[0].clone()
}

fn clip(mut v: Vec<f64>) -> Vec<f64> {
    for d in &mut v {
        if *d < 0.0 {
            *d = 0.0;
        }
    }
    v
}

/// CF4 from PETIR's pieces, with the final pair in either order.
fn cf4_by_hand(n_steps: usize, docstring_order: bool) -> Vec<f64> {
    let mut op = FixedRate::default();
    let s = CramSolver::Cram48;
    let dt = T / n_steps as f64;
    let mut n = N0.to_vec();
    let m = |op: &FixedRate, r: &Vec<f64>| -> DepletionMatrix { op.form_matrix(r, 0).unwrap() };
    for _ in 0..n_steps {
        let r0 = op.evaluate(&[n.clone()], SOURCE).unwrap().rates;
        let n1 = clip(s.solve(&cf4_f1(&m(&op, &r0)), &n, dt, 1).unwrap());
        let r1 = op.evaluate(&[n1.clone()], SOURCE).unwrap().rates;
        let n2 = clip(s.solve(&cf4_f1(&m(&op, &r1)), &n, dt, 1).unwrap());
        let r2 = op.evaluate(&[n2], SOURCE).unwrap().rates;
        let n3 = clip(
            s.solve(&cf4_f2(&m(&op, &r0), &m(&op, &r2)).unwrap(), &n1, dt, 1)
                .unwrap(),
        );
        let r3 = op.evaluate(&[n3], SOURCE).unwrap().rates;
        let (a, b, c, d) = (m(&op, &r0), m(&op, &r1), m(&op, &r2), m(&op, &r3));
        let f3 = cf4_f3(&a, &b, &c, &d).unwrap();
        let f4 = cf4_f4(&a, &b, &c, &d).unwrap();
        let (first, second) = if docstring_order { (f4, f3) } else { (f3, f4) };
        let inter = clip(s.solve(&first, &n, dt, 1).unwrap());
        n = clip(s.solve(&second, &inter, dt, 1).unwrap());
    }
    n
}

fn error(n: &[f64]) -> f64 {
    let e = exact(T);
    n.iter()
        .zip(e)
        .map(|(a, b)| ((a - b) / b).abs())
        .fold(0.0, f64::max)
}

fn orders(errors: &[f64]) -> Vec<f64> {
    errors.windows(2).map(|w| (w[0] / w[1]).log2()).collect()
}

fn table(label: &str, errors: &[f64]) -> Vec<f64> {
    let p = orders(errors);
    print!("{label:<22}");
    for e in errors {
        print!(" {e:9.2e}");
    }
    print!("\n{:<22}          ", "  observed order");
    for q in &p {
        print!(" {q:9.3}");
    }
    println!();
    p
}

#[test]
fn the_by_hand_cf4_harness_reproduces_the_integrator() {
    for n in [4, 16] {
        let a = cf4_by_hand(n, false);
        let b = final_state(Integrator::Cf4, n);
        for (x, y) in a.iter().zip(&b) {
            assert_eq!(
                x.to_bits(),
                y.to_bits(),
                "harness != Integrator::Cf4 at N={n}"
            );
        }
    }
}

#[test]
fn observed_orders_of_convergence() {
    println!("N = {NS:?}, T = {T} s, CRAM48; max relative error at T");
    let mut measured = Vec::new();
    for method in Integrator::ALL {
        let errs: Vec<f64> = NS.iter().map(|&n| error(&final_state(method, n))).collect();
        let p = table(method.name(), &errs);
        measured.push((method, p));
    }
    let errs: Vec<f64> = NS.iter().map(|&n| error(&cf4_by_hand(n, true))).collect();
    let p_doc = table("cf4 (docstring order)", &errs);

    // Gate: the order between N = 128 and N = 256 (index 5 of the order
    // list) against the PREDICTED order, within 0.1. That pair is in every
    // method's asymptotic range and above CF4's round-off floor (CF4 reaches
    // ~1.3e-12 at N = 512, after which its "order" is round-off).
    for (method, p) in &measured {
        let q = p[5];
        let want = f64::from(method.predicted_order());
        assert!(
            (q - want).abs() < 0.1,
            "{method:?}: observed order {q:.3} at N=128->256, predicted {want}"
        );
    }
    assert!(
        (p_doc[5] - 2.0).abs() < 0.1,
        "docstring-order CF4: {:.3}",
        p_doc[5]
    );
}

/// The orders measured on 2026-10-17 (N = 128 -> 256), pinned so that a
/// change to any integrator's arithmetic that moves its order shows here.
///
/// | integrator | predicted | observed (128->256) | error at N = 256 |
/// |---|---|---|---|
/// | predictor | 1 | 0.993 | 3.89e-3 |
/// | cecm | 2 | 1.995 | 9.30e-6 |
/// | cf4 | 4 | 3.981 | 1.67e-11 |
/// | celi | 2 | 2.014 | 1.34e-6 |
/// | epc_rk4 | 2 | 2.000 | 3.00e-6 |
/// | leqi | 3 | 3.009 | 8.44e-9 |
/// | si_celi | 2 | 1.999 | 6.46e-6 |
/// | si_leqi | 3 | 2.989 | 5.85e-8 |
/// | cf4, docstring order | 2 | 2.000 | 6.01e-6 |
const RECORDED: [(&str, f64); 8] = [
    ("predictor", 0.993),
    ("cecm", 1.995),
    ("cf4", 3.981),
    ("celi", 2.014),
    ("epc_rk4", 2.000),
    ("leqi", 3.009),
    ("si_celi", 1.999),
    ("si_leqi", 2.989),
];

#[test]
fn the_recorded_orders_reproduce() {
    for (name, want) in RECORDED {
        let method = Integrator::by_name(name).unwrap();
        let e: Vec<f64> = [128, 256]
            .iter()
            .map(|&n| error(&final_state(method, n)))
            .collect();
        let q = (e[0] / e[1]).log2();
        assert!(
            (q - want).abs() < 5e-4,
            "{name}: {q:.4} against the recorded {want}"
        );
    }
}
