use super::{OdeError, OdeSystem, OdeSolverConfig, adaptive_step, integrate_interval, normalize_error};

// Butcher tableau coefficients — Runge-Kutta-Fehlberg 4(5)
// Source: Foam::RKF45 constants
const C2: f64 = 1.0 / 4.0;
const C3: f64 = 3.0 / 8.0;
const C4: f64 = 12.0 / 13.0;
const C5: f64 = 1.0;
const C6: f64 = 1.0 / 2.0;

const A21: f64 = 1.0 / 4.0;
const A31: f64 = 3.0 / 32.0;
const A32: f64 = 9.0 / 32.0;
const A41: f64 = 1932.0 / 2197.0;
const A42: f64 = -7200.0 / 2197.0;
const A43: f64 = 7296.0 / 2197.0;
const A51: f64 = 439.0 / 216.0;
const A52: f64 = -8.0;
const A53: f64 = 3680.0 / 513.0;
const A54: f64 = -845.0 / 4104.0;
const A61: f64 = -8.0 / 27.0;
const A62: f64 = 2.0;
const A63: f64 = -3544.0 / 2565.0;
const A64: f64 = 1859.0 / 4104.0;
const A65: f64 = -11.0 / 40.0;

// 5th-order weights
const B1: f64 = 16.0 / 135.0;
const B3: f64 = 6656.0 / 12825.0;
const B4: f64 = 28561.0 / 56430.0;
const B5: f64 = -9.0 / 50.0;
const B6: f64 = 2.0 / 55.0;

// Error = 5th-order − 4th-order weights
const E1: f64 = 25.0 / 216.0 - B1;
const E3: f64 = 1408.0 / 2565.0 - B3;
const E4: f64 = 2197.0 / 4104.0 - B4;
const E5: f64 = -1.0 / 5.0 - B5;
const E6: f64 = -B6;

/// Runge-Kutta-Fehlberg 4(5) explicit solver with adaptive step size.
/// Maps to `Foam::RKF45`.
pub struct Rkf45 {
    pub config: OdeSolverConfig,
    dydx0: Vec<f64>,
    y_temp: Vec<f64>,
    k2: Vec<f64>,
    k3: Vec<f64>,
    k4: Vec<f64>,
    k5: Vec<f64>,
    k6: Vec<f64>,
    err: Vec<f64>,
    y_stage: Vec<f64>,
}

impl Rkf45 {
    pub fn new(n: usize, abs_tol: f64, rel_tol: f64) -> Self {
        let mut cfg = OdeSolverConfig::default();
        cfg.abs_tol = abs_tol;
        cfg.rel_tol = rel_tol;
        Self {
            config: cfg,
            dydx0: vec![0.0; n],
            y_temp: vec![0.0; n],
            k2: vec![0.0; n],
            k3: vec![0.0; n],
            k4: vec![0.0; n],
            k5: vec![0.0; n],
            k6: vec![0.0; n],
            err: vec![0.0; n],
            y_stage: vec![0.0; n],
        }
    }

    pub fn solve_step(
        &mut self,
        ode: &dyn OdeSystem,
        x: &mut f64,
        y: &mut Vec<f64>,
        dx_try: &mut f64,
    ) -> Result<(), OdeError> {
        let cfg = self.config.clone();
        let abs_tol = cfg.abs_tol;
        let rel_tol = cfg.rel_tol;

        let k2 = &mut self.k2;
        let k3 = &mut self.k3;
        let k4 = &mut self.k4;
        let k5 = &mut self.k5;
        let k6 = &mut self.k6;
        let err = &mut self.err;
        let y_stage = &mut self.y_stage;

        adaptive_step(
            &cfg,
            |x0, y0, dydx0, dx, y_out| {
                let n = y0.len();

                // Stage 2
                for i in 0..n { y_stage[i] = y0[i] + A21 * dx * dydx0[i]; }
                ode.derivatives(x0 + C2 * dx, y_stage, k2);

                // Stage 3
                for i in 0..n { y_stage[i] = y0[i] + dx * (A31 * dydx0[i] + A32 * k2[i]); }
                ode.derivatives(x0 + C3 * dx, y_stage, k3);

                // Stage 4
                for i in 0..n {
                    y_stage[i] = y0[i] + dx * (A41 * dydx0[i] + A42 * k2[i] + A43 * k3[i]);
                }
                ode.derivatives(x0 + C4 * dx, y_stage, k4);

                // Stage 5
                for i in 0..n {
                    y_stage[i] = y0[i]
                        + dx * (A51 * dydx0[i] + A52 * k2[i] + A53 * k3[i] + A54 * k4[i]);
                }
                ode.derivatives(x0 + C5 * dx, y_stage, k5);

                // Stage 6
                for i in 0..n {
                    y_stage[i] = y0[i]
                        + dx * (A61 * dydx0[i] + A62 * k2[i] + A63 * k3[i]
                              + A64 * k4[i] + A65 * k5[i]);
                }
                ode.derivatives(x0 + C6 * dx, y_stage, k6);

                // 5th-order solution
                for i in 0..n {
                    y_out[i] = y0[i]
                        + dx * (B1 * dydx0[i] + B3 * k3[i] + B4 * k4[i]
                              + B5 * k5[i] + B6 * k6[i]);
                }

                // Error = 5th − 4th
                for i in 0..n {
                    err[i] = dx
                        * (E1 * dydx0[i] + E3 * k3[i] + E4 * k4[i]
                         + E5 * k5[i] + E6 * k6[i]);
                }

                normalize_error(y0, y_out, err, abs_tol, rel_tol)
            },
            ode,
            x,
            y,
            &mut self.dydx0,
            &mut self.y_temp,
            dx_try,
        )
    }

    pub fn integrate(
        &mut self,
        ode: &dyn OdeSystem,
        x_start: f64,
        x_end: f64,
        y: &mut Vec<f64>,
        dx_est: &mut f64,
    ) -> Result<(), OdeError> {
        let cfg = self.config.clone();
        integrate_interval(
            &cfg,
            &mut |x, y, dx| self.solve_step(ode, x, y, dx),
            x_start,
            x_end,
            y,
            dx_est,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DecayOde;
    impl OdeSystem for DecayOde {
        fn n_eqns(&self) -> usize { 1 }
        fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
            dydx[0] = -y[0];
        }
    }

    struct VanDerPol { mu: f64 }
    impl OdeSystem for VanDerPol {
        fn n_eqns(&self) -> usize { 2 }
        fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
            dydx[0] = y[1];
            dydx[1] = self.mu * (1.0 - y[0] * y[0]) * y[1] - y[0];
        }
    }

    #[test]
    fn rkf45_exponential_decay() {
        // y' = -y, y(0) = 1  →  y(1) = e^{-1}
        let ode = DecayOde;
        let mut solver = Rkf45::new(1, 1e-8, 1e-6);
        let mut y = vec![1.0_f64];
        let mut dx = 0.1;
        solver.integrate(&ode, 0.0, 1.0, &mut y, &mut dx).unwrap();
        let expected = (-1.0_f64).exp();
        assert!(
            (y[0] - expected).abs() < 1e-6,
            "y={:.10}, expected={:.10}", y[0], expected
        );
    }

    #[test]
    fn rkf45_decay_to_5() {
        // y' = -y, y(0) = 1  →  y(5) = e^{-5}
        let ode = DecayOde;
        let mut solver = Rkf45::new(1, 1e-9, 1e-7);
        let mut y = vec![1.0_f64];
        let mut dx = 0.1;
        solver.integrate(&ode, 0.0, 5.0, &mut y, &mut dx).unwrap();
        let expected = (-5.0_f64).exp();
        assert!((y[0] - expected).abs() < 1e-7, "y={:.12}, expected={:.12}", y[0], expected);
    }

    #[test]
    fn rkf45_nonstiff_vdp() {
        // Mildly non-stiff Van der Pol (mu=0.5): y(0) = (2, 0), integrate to t=2
        // Just check it completes without error and the solution is bounded
        let ode = VanDerPol { mu: 0.5 };
        let mut solver = Rkf45::new(2, 1e-7, 1e-5);
        let mut y = vec![2.0_f64, 0.0];
        let mut dx = 0.1;
        solver.integrate(&ode, 0.0, 2.0, &mut y, &mut dx).unwrap();
        assert!(y[0].abs() < 3.0, "y0={}", y[0]);
        assert!(y[1].abs() < 3.0, "y1={}", y[1]);
    }
}
