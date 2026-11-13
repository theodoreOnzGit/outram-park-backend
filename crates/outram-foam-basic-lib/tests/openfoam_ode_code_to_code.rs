//! ODE-solver differential test against upstream OpenFOAM-v2506 (gh:#1362, row 6).
//!
//! # Method
//! `verification_and_validation/upstream_ode_gh1362/driver/odeDump.C` is
//! `applications/test/ODE/Test-ODE.C` transcribed onto the unmodified `libODE`
//! and run for Euler, RKF45 and Rosenbrock23: 15 single adaptive steps from
//! `x = 1` at `relTol = exp(-(i+1))`, then a solve to `x = 2` at `relTol 1e-4`.
//! This test runs the port's solvers on the same system from the upstream start
//! state and the upstream `relTol` bits, and compares bit patterns.
//!
//! # Criterion
//! Bit-identity; per solver the count of bit-identical values (of 15 x 7 step
//! values and 5 final values), the worst relative difference and the worst ulp
//! are printed (`--nocapture`) and PINNED.

use outram_foam_basic_lib::matrix::SquareMatrix;
use outram_foam_basic_lib::ode::integrator::OdeSolver;
use outram_foam_basic_lib::ode::{OdeError, OdeSystem};
use std::collections::BTreeMap;

const REF: &str =
    include_str!("../verification_and_validation/upstream_ode_gh1362/reference/ode_esi_v2506.txt");

struct TestOde;

impl OdeSystem for TestOde {
    fn n_eqns(&self) -> usize {
        4
    }

    fn derivatives(&self, x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        dydx[0] = -y[1];
        dydx[1] = y[0] - (1.0 / x) * y[1];
        dydx[2] = y[1] - (2.0 / x) * y[2];
        dydx[3] = y[2] - (3.0 / x) * y[3];
    }

    fn jacobian(&self, x: f64, y: &[f64], dfdx: &mut Vec<f64>, dfdy: &mut SquareMatrix) {
        dfdx[0] = 0.0;
        dfdx[1] = (1.0 / (x * x)) * y[1];
        dfdx[2] = (2.0 / (x * x)) * y[2];
        dfdx[3] = (3.0 / (x * x)) * y[3];

        dfdy.set(0, 0, 0.0);
        dfdy.set(0, 1, -1.0);
        dfdy.set(0, 2, 0.0);
        dfdy.set(0, 3, 0.0);

        dfdy.set(1, 0, 1.0);
        dfdy.set(1, 1, -1.0 / x);
        dfdy.set(1, 2, 0.0);
        dfdy.set(1, 3, 0.0);

        dfdy.set(2, 0, 0.0);
        dfdy.set(2, 1, 1.0);
        dfdy.set(2, 2, -2.0 / x);
        dfdy.set(2, 3, 0.0);

        dfdy.set(3, 0, 0.0);
        dfdy.set(3, 1, 0.0);
        dfdy.set(3, 2, 1.0);
        dfdy.set(3, 3, -3.0 / x);
    }
}

type Rows = BTreeMap<(String, String, usize), Vec<f64>>;

fn reference() -> Rows {
    let mut map = Rows::new();
    for line in REF.lines() {
        let mut it = line.split_whitespace();
        let solver = it.next().unwrap().to_string();
        let what = it.next().unwrap().to_string();
        let idx: usize = it.next().unwrap().parse().unwrap();
        let vals: Vec<f64> = it
            .map(|h| f64::from_bits(u64::from_str_radix(h, 16).unwrap()))
            .collect();
        map.insert((solver, what, idx), vals);
    }
    map
}

fn make(name: &str, rel_tol: f64) -> OdeSolver {
    // upstream absTol default SMALL = 1e-15
    let mut s = match name {
        "Euler" => OdeSolver::euler(4, 1e-15, rel_tol),
        "RKF45" => OdeSolver::rkf45(4, 1e-15, rel_tol),
        _ => OdeSolver::rosenbrock23(4, 1e-15, rel_tol),
    };
    s.config_mut().max_steps = 400_000;
    s
}

fn ulp(a: f64, b: f64) -> u64 {
    if a.to_bits() == b.to_bits() {
        0
    } else if a.is_sign_negative() != b.is_sign_negative() {
        u64::MAX
    } else {
        a.to_bits().abs_diff(b.to_bits())
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
struct Tally {
    identical: usize,
    total: usize,
    worst_ulp: u64,
    worst_rel: f64,
}

impl Tally {
    fn add(&mut self, mine: f64, theirs: f64) {
        self.total += 1;
        let d = ulp(mine, theirs);
        if d == 0 {
            self.identical += 1;
        } else {
            self.worst_ulp = self.worst_ulp.max(d);
            let rel = ((mine - theirs) / theirs).abs();
            if rel > self.worst_rel {
                self.worst_rel = rel;
            }
        }
    }
}

/// (steps tally over x - xStart, dxNext, y0..y3; final tally over y0..y3, dxEst);
/// `esi` selects `integrate_esi_v2506` for the final solve.
fn run_solver(name: &str, refs: &Rows, esi: bool) -> (Tally, Tally, Tally, Tally) {
    let ode = TestOde;
    let y_start = refs[&(name.to_string(), "ystart".to_string(), 0)].clone();
    let (mut t_x, mut t_dx, mut t_y, mut t_final) =
        (Tally::default(), Tally::default(), Tally::default(), Tally::default());
    for i in 0..15usize {
        let row = &refs[&(name.to_string(), "step".to_string(), i)];
        let rel_tol = row[0];
        let mut s = make(name, rel_tol);
        let mut x = 1.0;
        let mut y = y_start.clone();
        let mut dx = 0.6;
        s.solve_step(&ode, &mut x, &mut y, &mut dx).expect("step");
        t_x.add(x - 1.0, row[1]);
        t_dx.add(dx, row[2]);
        for k in 0..4 {
            t_y.add(y[k], row[3 + k]);
        }
    }
    let fin = &refs[&(name.to_string(), "final".to_string(), 0)];
    let mut s = make(name, 1e-4);
    let mut y = y_start;
    let mut dx = 0.5;
    if esi {
        s.integrate_esi_v2506(&ode, 1.0, 2.0, &mut y, &mut dx).expect("integrate");
    } else {
        s.integrate(&ode, 1.0, 2.0, &mut y, &mut dx).expect("integrate");
    }
    for k in 0..4 {
        t_final.add(y[k], fin[k]);
    }
    t_final.add(dx, fin[4]);
    (t_x, t_dx, t_y, t_final)
}

#[test]
fn ode_vs_openfoam_report() {
    let refs = reference();
    for esi in [false, true] {
        println!("=== integrate{}", if esi { "_esi_v2506" } else { "" });
        for name in ["Euler", "RKF45", "Rosenbrock23"] {
            let (a, b, c, d) = run_solver(name, &refs, esi);
            println!("{name:13} x-xStart {a:?}");
            println!("{name:13} dxNext   {b:?}");
            println!("{name:13} y        {c:?}");
            println!("{name:13} final    {d:?}");
        }
    }
}

/// Upstream's Euler cannot reach x = 2 at relTol 1e-4 in the default 10000
/// steps (FatalError at x = 1.23094); the port reports `MaxStepsExceeded`.
#[test]
fn euler_default_max_steps_fails_like_upstream() {
    let refs = reference();
    let y_start = refs[&("Euler".to_string(), "ystart".to_string(), 0)].clone();
    let mut s = OdeSolver::euler(4, 1e-15, 1e-4);
    let mut y = y_start;
    let mut dx = 0.5;
    let r = s.integrate(&TestOde, 1.0, 2.0, &mut y, &mut dx);
    assert!(matches!(r, Err(OdeError::MaxStepsExceeded { .. })), "{r:?}");
}

/// Pins (measured 2026-11-13, release). The 15 single adaptive steps are
/// bit-identical for all three solvers (x advanced, dxNext, y0..y3: 15 + 15 + 60
/// values each). The final solve to x = 2 is bit-identical (5/5 values) only
/// through `integrate_esi_v2506`; the older `integrate` forms the truncated step
/// as `(x + dx).min(x_end) - x` and returns the post-truncation suggestion, and
/// differs (Euler 0/5, RKF45 3/5, Rosenbrock23 0/5).
#[test]
fn ode_vs_openfoam_pins() {
    let refs = reference();
    for name in ["Euler", "RKF45", "Rosenbrock23"] {
        let (a, b, c, d) = run_solver(name, &refs, true);
        assert_eq!((a.identical, a.total), (15, 15), "{name}");
        assert_eq!((b.identical, b.total), (15, 15), "{name}");
        assert_eq!((c.identical, c.total), (60, 60), "{name}");
        assert_eq!((d.identical, d.total), (5, 5), "{name} esi final");
    }
    for (name, expect) in [("Euler", 0usize), ("RKF45", 3), ("Rosenbrock23", 0)] {
        let (a, b, c, d) = run_solver(name, &refs, false);
        assert_eq!((a.identical, b.identical, c.identical), (15, 15, 60), "{name}");
        assert_eq!((d.identical, d.total), (expect, 5), "{name} old final");
    }
}
