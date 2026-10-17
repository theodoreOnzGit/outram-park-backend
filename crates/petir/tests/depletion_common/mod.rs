// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! The three-nuclide constant-power problem shared by the depletion V&V
//! tests, mirrored line for line in
//! `verification_and_validation/depletion/openmc_inputs/integrators_reference.py`.
//!
//! ```text
//!   A --(lambda_A)--> C          A --(capture, sigma_g phi)--> B
//!   A --(fission, sigma_f phi)--> 2 C          B --(lambda_B)--> C
//!   phi = S / (sigma_f N_A)       (fission rate S held constant: "power")
//! ```
//!
//! The burnup matrix depends on the composition through `phi(N_A)`, and its
//! decay and reaction parts do not commute (`lambda_A != lambda_B`), so the
//! integrators genuinely differ. Yet the exact solution is closed-form,
//! because `phi N_A = S / sigma_f` is constant:
//!
//! ```text
//!   N_A(t) = (N_A0 + c/lA) e^{-lA t} - c/lA,          c = (sigma_a/sigma_f) S
//!   N_B(t) = N_B0 e^{-lB t} + (b/lB)(1 - e^{-lB t}),  b = (sigma_g/sigma_f) S
//!   N_C(t) = N_C0 + lA int N_A + 2 S t + lB int N_B
//! ```

#![allow(dead_code)]

use petir::depletion::{DepletionMatrix, Keff, OperatorResult, TransportOperator};

pub const LAM_A: f64 = 0.5;
pub const LAM_B: f64 = 2.0;
pub const SIG_F: f64 = 1.0;
pub const SIG_G: f64 = 0.5;
pub const SOURCE: f64 = 0.3;
pub const N0: [f64; 3] = [1.0, 0.0, 0.0];

/// Per-atom reaction rates `[sigma_f phi, sigma_g phi]` at constant power.
pub fn rates_at(n_a: f64) -> Vec<f64> {
    let phi = SOURCE / (SIG_F * n_a);
    vec![SIG_F * phi, SIG_G * phi]
}

/// The burnup matrix from rates `[rf, rg]`.
pub fn matrix_from(rf: f64, rg: f64) -> DepletionMatrix {
    let mut a = DepletionMatrix::zeros(3);
    a.set(0, 0, -(LAM_A + rf + rg)).unwrap();
    a.set(1, 0, rg).unwrap();
    a.set(2, 0, LAM_A + 2.0 * rf).unwrap();
    a.set(1, 1, -LAM_B).unwrap();
    a.set(2, 1, LAM_B).unwrap();
    a
}

/// The fixed-rate operator; counts its evaluations.
#[derive(Default)]
pub struct FixedRate {
    pub calls: usize,
}

impl TransportOperator for FixedRate {
    type Rates = Vec<f64>;
    type Error = ();

    fn evaluate(&mut self, n: &[Vec<f64>], _source_rate: f64) -> Result<OperatorResult<Vec<f64>>, ()> {
        self.calls += 1;
        let (n_a, n_b) = (n[0][0], n[0][1]);
        let k = (2.4 * n_a) / (1.5 * n_a + 0.8 * n_b);
        Ok(OperatorResult { k: Keff { value: k, std_dev: 1.0e-3 * k }, rates: rates_at(n_a) })
    }

    fn form_matrix(&self, rates: &Vec<f64>, _material: usize) -> Result<DepletionMatrix, ()> {
        Ok(matrix_from(rates[0], rates[1]))
    }
}

/// The exact composition at time `t` from `N0`.
pub fn exact(t: f64) -> [f64; 3] {
    let sig_a = SIG_F + SIG_G;
    let c = sig_a / SIG_F * SOURCE;
    let b = SIG_G / SIG_F * SOURCE;
    let ea = (-LAM_A * t).exp();
    let eb = (-LAM_B * t).exp();
    let k = N0[0] + c / LAM_A;
    let na = k * ea - c / LAM_A;
    let nb = N0[1] * eb + b / LAM_B * (1.0 - eb);
    let int_a = k * (1.0 - ea) / LAM_A - c * t / LAM_A;
    let int_b = N0[1] * (1.0 - eb) / LAM_B + b / LAM_B * (t - (1.0 - eb) / LAM_B);
    let nc = N0[2] + LAM_A * int_a + 2.0 * SOURCE * t + LAM_B * int_b;
    [na, nb, nc]
}
