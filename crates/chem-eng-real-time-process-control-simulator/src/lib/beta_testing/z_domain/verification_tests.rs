// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Kay Chen Ong (OUTRAM PARK workspace)
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, version 3 of the License.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with this program.  If not, see <https://www.gnu.org/licenses/>.

//! Verification tests for the z-domain module (`c2d`/`d2c` port).
//!
//! Every test documents its **methodology** (what is computed, the
//! reference it is judged against, inputs, pass criterion) and its
//! **results** (the actual measured numbers, with the date and build they
//! were taken on), per the workspace V&V documentation rule. The reference
//! in each case is an analytic closed form or an exact algebraic identity
//! — no Octave installation was available on the test machine, so no
//! number below is an Octave output; where upstream Octave ships an
//! equivalent test (`%!assert` blocks in `inst/@lti/c2d.m` / `d2c.m`,
//! round-trip at tolerance 1e-4), the corresponding test here checks the
//! same identity at a tighter tolerance.

use uom::si::f64::*;
use uom::si::ratio::ratio;
use uom::si::time::second;
use uom::si::angular_velocity::radian_per_second;
use uom::ConstZero;

use super::continuous_tf::ContinuousTransferFn;
use super::conversion::{C2dMethod, D2cMethod};
use super::cplx::Cplx;
use super::discrete_tf::DiscreteTransferFn;
use super::polynomial;
use super::ZDomainError;

use crate::beta_testing::stable_transfer_functions::first_order_transfer_fn::FirstOrderStableTransferFnNoZeroes;
use crate::beta_testing::stable_transfer_functions::second_order_transfer_fn::SecondOrderStableStepResponse;

/// Deterministic pseudo-random sequence in [-1, 1] (LCG; no rand dep).
fn lcg_sequence(n: usize) -> Vec<f64> {
    let mut state: u64 = 0x2545F4914F6CDD1D;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        // take the top 53 bits as a float in [0,1), map to [-1,1]
        let u = (state >> 11) as f64 / (1u64 << 53) as f64;
        out.push(2.0 * u - 1.0);
    }
    out
}

/// # Methodology
///
/// Zero-order-hold discretisation of the first-order lag
/// `G(s) = K_p/(tau_p s + 1)` has the exact closed-form coefficients
/// `y[n] = a y[n-1] + K_p (1 - a) u[n-1]` with `a = exp(-T/tau_p)`
/// (the step-invariant recurrence documented in
/// `stable_transfer_functions/first_order_transfer_fn.rs`). This test runs
/// `c2d` (Zoh) with `K_p = 2.5`, `tau_p = 0.8 s`, `T = 0.1 s` and compares
/// every produced `z^-1` coefficient against those closed forms evaluated
/// in this test. Pass criterion: max absolute coefficient deviation
/// <= 1e-14.
///
/// # Results
///
/// Measured 2026-08-11 (crate 0.2.0, rustc 1.97.0, release profile):
/// denominator `[1, -0.8824969025845955]`, numerator
/// `[0, 0.29375774353851136]`, max deviation from the closed form
/// **0.0** (bit-exact). `a = exp(-0.125) = 0.8824969025845955`.
#[test]
fn zoh_first_order_coefficients_match_analytic_closed_form() {
    let k_p = Ratio::new::<ratio>(2.5);
    let tau = Time::new::<second>(0.8);
    let t_samp = Time::new::<second>(0.1);

    let g = ContinuousTransferFn::first_order(k_p, tau).unwrap();
    let d = g.to_discrete(t_samp, C2dMethod::Zoh).unwrap();

    let a = (-0.1_f64 / 0.8).exp();
    let expected_num = [0.0, 2.5 * (1.0 - a)];
    let expected_den = [1.0, -a];

    let mut max_dev = 0.0_f64;
    for (got, want) in d
        .numerator_z_inverse()
        .iter()
        .zip(expected_num.iter())
        .chain(d.denominator_z_inverse().iter().zip(expected_den.iter()))
    {
        max_dev = max_dev.max((got - want).abs());
    }
    println!(
        "zoh first order: num = {:?}, den = {:?}, max_dev = {:e}",
        d.numerator_z_inverse(),
        d.denominator_z_inverse(),
        max_dev
    );
    assert_eq!(d.numerator_z_inverse().len(), 2);
    assert_eq!(d.denominator_z_inverse().len(), 2);
    assert!(max_dev <= 1e-14, "max_dev = {max_dev:e}");
}

/// # Methodology
///
/// `C2dMethod::Zoh` and the O(1) recurrence block
/// `FirstOrderStableTransferFnNoZeroes` implement the same step-invariant
/// mathematics (bead `op-fm5`), so stepping both against the identical
/// piecewise-constant input sequence must give the same output samples.
/// Inputs: `K_p = 2.5`, `tau_p = 0.8 s`, `T = 0.1 s`, 200 samples of a
/// deterministic LCG pseudo-random signal in [-1, 1], both systems
/// starting at rest. Pass criterion: max absolute output difference
/// <= 1e-12 (dimensionless).
///
/// # Results
///
/// Measured 2026-08-11 (crate 0.2.0, rustc 1.97.0, release profile):
/// max |y_discrete - y_block| = **1.21e-14** over 200 samples — agreement
/// at the double-precision rounding floor, confirming the two
/// implementations realise the same discretisation rather than two
/// parallel ones.
#[test]
fn zoh_first_order_reproduces_o1_recurrence_block() {
    let k_p = Ratio::new::<ratio>(2.5);
    let tau = Time::new::<second>(0.8);
    let t_samp = Time::new::<second>(0.1);

    let g = ContinuousTransferFn::first_order(k_p, tau).unwrap();
    let mut discrete = g.to_discrete(t_samp, C2dMethod::Zoh).unwrap();

    let mut block =
        FirstOrderStableTransferFnNoZeroes::new(k_p, tau, Ratio::ZERO, Ratio::ZERO, Time::ZERO)
            .unwrap();

    let inputs = lcg_sequence(200);
    let mut max_diff = 0.0_f64;
    for (n, &u) in inputs.iter().enumerate() {
        let t_now = Time::new::<second>(n as f64 * 0.1);
        let u_ratio = Ratio::new::<ratio>(u);
        let y_block = block
            .set_user_input_and_calc_output(t_now, u_ratio)
            .unwrap()
            .get::<ratio>();
        let y_disc = discrete.advance_one_sample(u_ratio).get::<ratio>();
        max_diff = max_diff.max((y_block - y_disc).abs());
    }
    println!("zoh vs recurrence block: max |diff| = {max_diff:e}");
    assert!(max_diff <= 1e-12, "max_diff = {max_diff:e}");
}

/// # Methodology
///
/// The zero-order-hold equivalent is *exact* at the sample instants for an
/// input held constant between samples, so the discrete step response of
/// `c2d(G, T, Zoh)` must land on the continuous analytic step response of
/// `G(s) = K_p/(tau_p^2 s^2 + 2 zeta tau_p s + 1)` at every sample. The
/// analytic reference is `SecondOrderStableStepResponse` (the closed-form
/// struct retained in `stable_transfer_functions` exactly for this
/// purpose). Inputs: `K_p = 1.5`, `tau_p = 2 s`, unit step, `T = 0.25 s`,
/// 200 samples, three damping regimes: underdamped `zeta = 0.45`,
/// critically damped `zeta = 1` (exercising the repeated-eigenvalue
/// branch), overdamped `zeta = 1.2`. Pass criterion: max absolute
/// deviation <= 1e-10 (dimensionless) in each regime.
///
/// # Results
///
/// Measured 2026-08-11 (crate 0.2.0, rustc 1.97.0, release profile), max
/// |y_discrete[n] - y_analytic(nT)| over 200 samples:
/// underdamped (zeta = 0.45) **2.91e-14**, critically damped (zeta = 1)
/// **1.98e-14**, overdamped (zeta = 1.2) **3.31e-14** — all within a few
/// hundred ulp of the rounding floor, confirming step-invariance in all
/// three regimes (the critically damped case exercises the
/// repeated-eigenvalue branch).
#[test]
fn zoh_second_order_step_response_exact_at_samples() {
    let k_p = Ratio::new::<ratio>(1.5);
    let tau = Time::new::<second>(2.0);
    let t_samp = Time::new::<second>(0.25);

    for zeta_val in [0.45, 1.0, 1.2] {
        let zeta = Ratio::new::<ratio>(zeta_val);
        let g = ContinuousTransferFn::second_order(k_p, tau, zeta).unwrap();
        let mut d = g.to_discrete(t_samp, C2dMethod::Zoh).unwrap();

        let mut analytic = SecondOrderStableStepResponse::new(
            k_p,
            tau,
            zeta,
            Time::ZERO,
            Ratio::new::<ratio>(1.0),
            Time::ZERO,
        )
        .unwrap();

        let mut max_dev = 0.0_f64;
        for n in 0..200 {
            let y_disc = d
                .advance_one_sample(Ratio::new::<ratio>(1.0))
                .get::<ratio>();
            let t_now = Time::new::<second>(n as f64 * 0.25);
            let y_ref = analytic.calculate_response(t_now).get::<ratio>();
            max_dev = max_dev.max((y_disc - y_ref).abs());
        }
        println!("zoh second order, zeta = {zeta_val}: max_dev = {max_dev:e}");
        assert!(max_dev <= 1e-10, "zeta = {zeta_val}: max_dev = {max_dev:e}");
    }
}

/// # Methodology
///
/// The Tustin method is the trapezoidal integration rule, so the bilinear
/// image of the pure integrator `G(s) = 1/s` must be exactly
/// `H(z^-1) = (T/2)(1 + z^-1)/(1 - z^-1)`, coefficients computed in this
/// test from `T = 0.5 s`. Pass criterion: every coefficient equal to the
/// closed form within 1e-15.
///
/// # Results
///
/// Measured 2026-08-11 (crate 0.2.0, rustc 1.97.0, release profile):
/// numerator `[0.25, 0.25]`, denominator `[1, -1]`, max deviation
/// **0.0** (bit-exact).
#[test]
fn tustin_integrator_is_trapezoidal_rule() {
    let g = ContinuousTransferFn::new(vec![1.0], vec![0.0, 1.0]).unwrap();
    let t_samp = Time::new::<second>(0.5);
    let d = g.to_discrete(t_samp, C2dMethod::Tustin).unwrap();

    let expected_num = [0.25, 0.25];
    let expected_den = [1.0, -1.0];
    let mut max_dev = 0.0_f64;
    for (got, want) in d
        .numerator_z_inverse()
        .iter()
        .zip(expected_num.iter())
        .chain(d.denominator_z_inverse().iter().zip(expected_den.iter()))
    {
        max_dev = max_dev.max((got - want).abs());
    }
    println!(
        "tustin integrator: num = {:?}, den = {:?}, max_dev = {max_dev:e}",
        d.numerator_z_inverse(),
        d.denominator_z_inverse()
    );
    assert!(max_dev <= 1e-15, "max_dev = {max_dev:e}");
}

/// # Methodology
///
/// `d2c(c2d(G, T, Tustin), Tustin)` must recover `G` exactly, because the
/// bilinear map is an involution-free exact change of variables. Upstream
/// Octave ships the same identity as a `%!assert` round-trip test in
/// `inst/@lti/d2c.m` at tolerance 1e-4 (state-space, via SLICOT); here the
/// identity is checked at the transfer-function level with
/// `G(s) = (3s + 2)/(4s^2 + 5s + 6)`, `T = 0.35 s`, comparing
/// leading-coefficient-normalised coefficient vectors. Pass criterion: max
/// deviation <= 1e-12.
///
/// # Results
///
/// Measured 2026-08-11 (crate 0.2.0, rustc 1.97.0, release profile):
/// max normalised-coefficient deviation = **8.88e-16** (round trip
/// recovered `(3s + 2)/(4s^2 + 5s + 6)` to the rounding floor).
#[test]
fn tustin_round_trip_recovers_continuous_coefficients() {
    let num = vec![2.0, 3.0]; // 2 + 3s
    let den = vec![6.0, 5.0, 4.0]; // 6 + 5s + 4s^2
    let g = ContinuousTransferFn::new(num.clone(), den.clone()).unwrap();
    let t_samp = Time::new::<second>(0.35);

    let d = g.to_discrete(t_samp, C2dMethod::Tustin).unwrap();
    let g2 = d.to_continuous(D2cMethod::Tustin).unwrap();

    let normalise = |p: &[f64], lead: f64| -> Vec<f64> { p.iter().map(|c| c / lead).collect() };
    let lead_orig = *den.last().unwrap();
    let lead_rt = *g2.denominator_ascending_s().last().unwrap();

    let num_orig = normalise(&num, lead_orig);
    let den_orig = normalise(&den, lead_orig);
    let num_rt = normalise(g2.numerator_ascending_s(), lead_rt);
    let den_rt = normalise(g2.denominator_ascending_s(), lead_rt);

    let mut max_dev = 0.0_f64;
    for (a, b) in num_orig
        .iter()
        .zip(num_rt.iter())
        .chain(den_orig.iter().zip(den_rt.iter()))
    {
        max_dev = max_dev.max((a - b).abs());
    }
    println!("tustin round trip: max_dev = {max_dev:e}");
    assert_eq!(num_rt.len(), num_orig.len());
    assert_eq!(den_rt.len(), den_orig.len());
    assert!(max_dev <= 1e-12, "max_dev = {max_dev:e}");
}

/// # Methodology
///
/// Bilinear prewarping at `w0` makes the discrete frequency response equal
/// the continuous one exactly at that angular frequency:
/// `H_d(exp(j w0 T)) = H_c(j w0)`. Inputs: the second-order test system
/// `K_p = 1`, `tau_p = 2 s`, `zeta = 0.5`, `w0 = 3 rad/s`, `T = 0.2 s`
/// (`w0 T = 0.6 < pi`). Both responses are evaluated by Horner's rule on
/// the stored polynomials. Pass criteria: complex deviation
/// `|H_d(e^{j w0 T}) - H_c(j w0)|` <= 1e-12 for the prewarped conversion,
/// and — as a sanity check that prewarping did something — deviation
/// > 1e-4 for plain Tustin at the same frequency.
///
/// # Results
///
/// Measured 2026-08-11 (crate 0.2.0, rustc 1.97.0, release profile):
/// prewarped deviation **6.99e-18**, plain-Tustin deviation **1.70e-3**
/// at `w0 = 3 rad/s` (|H_c(j w0)| = 2.816e-2 for reference).
#[test]
fn prewarp_matches_continuous_frequency_response_at_w0() {
    let g = ContinuousTransferFn::second_order(
        Ratio::new::<ratio>(1.0),
        Time::new::<second>(2.0),
        Ratio::new::<ratio>(0.5),
    )
    .unwrap();
    let t_samp = Time::new::<second>(0.2);
    let w0 = AngularVelocity::new::<radian_per_second>(3.0);

    let eval_continuous = |sys: &ContinuousTransferFn, w: f64| -> Cplx {
        let s = Cplx::new(0.0, w);
        polynomial::eval(sys.numerator_ascending_s(), s)
            / polynomial::eval(sys.denominator_ascending_s(), s)
    };
    // H_d(z) with coefficients ascending in z^-1: evaluate at z^-1 = 1/z.
    let eval_discrete = |sys: &DiscreteTransferFn, w: f64, t: f64| -> Cplx {
        let z_inv = Cplx::new(0.0, -w * t).exp(); // e^{-j w T}
        polynomial::eval(sys.numerator_z_inverse(), z_inv)
            / polynomial::eval(sys.denominator_z_inverse(), z_inv)
    };

    let h_c = eval_continuous(&g, 3.0);
    let d_pre = g
        .to_discrete(
            t_samp,
            C2dMethod::TustinPrewarp {
                prewarp_frequency: w0,
            },
        )
        .unwrap();
    let d_plain = g.to_discrete(t_samp, C2dMethod::Tustin).unwrap();

    let dev_pre = (eval_discrete(&d_pre, 3.0, 0.2) - h_c).abs();
    let dev_plain = (eval_discrete(&d_plain, 3.0, 0.2) - h_c).abs();
    println!(
        "prewarp: |H_c| = {:e}, dev_prewarp = {dev_pre:e}, dev_plain_tustin = {dev_plain:e}",
        h_c.abs()
    );
    assert!(dev_pre <= 1e-12, "dev_pre = {dev_pre:e}");
    assert!(
        dev_plain > 1e-4,
        "plain tustin unexpectedly exact: {dev_plain:e}"
    );
}

/// # Methodology
///
/// The matched pole/zero method must (a) map every continuous pole and
/// finite zero through `z = exp(s T)`, (b) append excess zeros at
/// `z = -1` (all but one), and (c) match the DC gain,
/// `H_d(1) = H_c(0)` — the defining properties of the method as ported
/// from `inst/@tf/__c2d__.m`. Inputs: `G_1(s) = (s+1)/((s+2)(s+3))`
/// (one finite zero, no excess-zero padding: np - nz - 1 = 0) and
/// `G_2(s) = 1/((s+2)(s+3))` (padding adds one zero at -1), both at
/// `T = 0.1 s`. Roots of the produced polynomials are recovered with the
/// quadratic formula. Pass criteria: pole/zero deviations from
/// `exp(-0.1)`, `exp(-0.2)`, `exp(-0.3)`, `-1` each <= 1e-12; DC-gain
/// deviation <= 1e-12.
///
/// # Results
///
/// Measured 2026-08-11 (crate 0.2.0, rustc 1.97.0, release profile):
/// G_1: pole deviations {3.33e-16, 3.33e-16}, zero deviation **0.0**,
/// DC gain 0.16666666666666657 vs 1/6, deviation **8.33e-17**.
/// G_2: added zero at exactly -1.0 (deviation **0.0**), DC-gain deviation
/// **8.33e-17**.
#[test]
fn matched_maps_poles_zeros_and_matches_dc_gain() {
    let t_samp = Time::new::<second>(0.1);

    // G_1 = (s+1)/((s+2)(s+3)) = (1 + s)/(6 + 5s + s^2)
    let g1 = ContinuousTransferFn::new(vec![1.0, 1.0], vec![6.0, 5.0, 1.0]).unwrap();
    let d1 = g1.to_discrete(t_samp, C2dMethod::MatchedPoleZero).unwrap();

    // recover roots of the produced polynomials (ascending z form)
    let den_asc: Vec<f64> = d1.denominator_z_inverse().iter().rev().copied().collect();
    let num_asc: Vec<f64> = {
        // pad numerator to denominator length before reversing so powers align
        let mut n = d1.numerator_z_inverse().to_vec();
        n.resize(d1.denominator_z_inverse().len(), 0.0);
        polynomial::trim(n.into_iter().rev().collect())
    };
    let mut poles: Vec<f64> = polynomial::roots_deg_le_2(&den_asc)
        .unwrap()
        .iter()
        .map(|c| c.re)
        .collect();
    poles.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let zeros: Vec<f64> = polynomial::roots_deg_le_2(&num_asc)
        .unwrap()
        .iter()
        .map(|c| c.re)
        .collect();

    let expect_p = {
        let mut v = vec![(-0.2_f64).exp(), (-0.3_f64).exp()];
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v
    };
    let pole_dev: Vec<f64> = poles
        .iter()
        .zip(expect_p.iter())
        .map(|(a, b)| (a - b).abs())
        .collect();
    let zero_dev = (zeros[0] - (-0.1_f64).exp()).abs();

    // DC gain: H_d(1) vs H_c(0) = 1/6
    let h_d_dc = polynomial::eval(d1.numerator_z_inverse(), Cplx::real(1.0))
        / polynomial::eval(d1.denominator_z_inverse(), Cplx::real(1.0));
    let dc_dev1 = (h_d_dc.re - 1.0 / 6.0).abs();
    println!(
        "matched G1: pole_dev = {pole_dev:?}, zero_dev = {zero_dev:e}, dc = {}, dc_dev = {dc_dev1:e}",
        h_d_dc.re
    );
    assert!(pole_dev.iter().all(|d| *d <= 1e-12));
    assert!(zero_dev <= 1e-12);
    assert!(dc_dev1 <= 1e-12);

    // G_2 = 1/((s+2)(s+3)): np - nz - 1 = 1 zero added at z = -1
    let g2 = ContinuousTransferFn::new(vec![1.0], vec![6.0, 5.0, 1.0]).unwrap();
    let d2 = g2.to_discrete(t_samp, C2dMethod::MatchedPoleZero).unwrap();
    let num2_asc: Vec<f64> = {
        let mut n = d2.numerator_z_inverse().to_vec();
        n.resize(d2.denominator_z_inverse().len(), 0.0);
        polynomial::trim(n.into_iter().rev().collect())
    };
    let zeros2 = polynomial::roots_deg_le_2(&num2_asc).unwrap();
    assert_eq!(zeros2.len(), 1, "exactly one padded zero expected");
    let zero2_dev = (zeros2[0].re - (-1.0)).abs();
    let h_d2_dc = polynomial::eval(d2.numerator_z_inverse(), Cplx::real(1.0))
        / polynomial::eval(d2.denominator_z_inverse(), Cplx::real(1.0));
    let dc_dev2 = (h_d2_dc.re - 1.0 / 6.0).abs();
    println!("matched G2: added zero dev = {zero2_dev:e}, dc_dev = {dc_dev2:e}");
    assert!(zero2_dev <= 1e-12);
    assert!(dc_dev2 <= 1e-12);
}

/// # Methodology
///
/// `d2c(c2d(G, T, MatchedPoleZero), MatchedPoleZero)` must recover `G`
/// when no excess zero is padded (the padding at `z = -1` is dropped again
/// on the way back, so the identity is exact only in the no-padding case).
/// Inputs: `G(s) = (s+1)/((s+2)(s+3))`, `T = 0.1 s`; comparison on
/// leading-coefficient-normalised coefficient vectors. Pass criterion: max
/// deviation <= 1e-9 (the matched gain matching goes through
/// transcendental evaluations, so bit-exactness is not expected).
///
/// # Results
///
/// Measured 2026-08-11 (crate 0.2.0, rustc 1.97.0, release profile): max
/// normalised-coefficient deviation = **4.44e-15** — well inside the
/// criterion.
#[test]
fn matched_round_trip_recovers_continuous_coefficients() {
    let num = vec![1.0, 1.0];
    let den = vec![6.0, 5.0, 1.0];
    let g = ContinuousTransferFn::new(num.clone(), den.clone()).unwrap();
    let t_samp = Time::new::<second>(0.1);

    let d = g.to_discrete(t_samp, C2dMethod::MatchedPoleZero).unwrap();
    let g2 = d.to_continuous(D2cMethod::MatchedPoleZero).unwrap();

    let lead = *g2.denominator_ascending_s().last().unwrap();
    let num_rt: Vec<f64> = g2
        .numerator_ascending_s()
        .iter()
        .map(|c| c / lead)
        .collect();
    let den_rt: Vec<f64> = g2
        .denominator_ascending_s()
        .iter()
        .map(|c| c / lead)
        .collect();

    let mut max_dev = 0.0_f64;
    for (a, b) in num
        .iter()
        .zip(num_rt.iter())
        .chain(den.iter().zip(den_rt.iter()))
    {
        max_dev = max_dev.max((a - b).abs());
    }
    println!("matched round trip: max_dev = {max_dev:e}");
    assert_eq!(num_rt.len(), num.len());
    assert_eq!(den_rt.len(), den.len());
    assert!(max_dev <= 1e-9, "max_dev = {max_dev:e}");
}

/// # Methodology
///
/// The `filt` docstring example from upstream (`filt([0, 3], [1, 4, 2])`,
/// i.e. `H(z^-1) = 3 z^-1 / (1 + 4 z^-1 + 2 z^-2)`) is driven with a unit
/// impulse `u = [1, 0, 0, ...]` and the outputs are compared against the
/// difference equation `y[n] = 3 u[n-1] - 4 y[n-1] - 2 y[n-2]` evaluated
/// independently in this test. Pass criterion: exact equality (both sides
/// are the same finite-precision arithmetic) over 20 samples.
///
/// # Results
///
/// Measured 2026-08-11 (crate 0.2.0, rustc 1.97.0, release profile):
/// first four impulse-response samples `[0, 3, -12, 42]` as hand
/// computation gives; all 20 samples matched the reference recurrence
/// exactly (max deviation **0.0**).
#[test]
fn filt_docstring_example_impulse_response() {
    let t_samp = Time::new::<second>(1.0);
    let mut h = DiscreteTransferFn::from_z_inverse_coefficients(
        vec![0.0, 3.0],
        vec![1.0, 4.0, 2.0],
        t_samp,
    )
    .unwrap();

    let n_samples = 20;
    let mut u = vec![0.0; n_samples];
    u[0] = 1.0;

    // independent reference recurrence
    let mut y_ref = vec![0.0; n_samples];
    for n in 0..n_samples {
        let u_prev = if n >= 1 { u[n - 1] } else { 0.0 };
        let y1 = if n >= 1 { y_ref[n - 1] } else { 0.0 };
        let y2 = if n >= 2 { y_ref[n - 2] } else { 0.0 };
        y_ref[n] = 3.0 * u_prev - 4.0 * y1 - 2.0 * y2;
    }

    let mut max_dev = 0.0_f64;
    let mut first_four = Vec::new();
    for n in 0..n_samples {
        let y = h
            .advance_one_sample(Ratio::new::<ratio>(u[n]))
            .get::<ratio>();
        if n < 4 {
            first_four.push(y);
        }
        max_dev = max_dev.max((y - y_ref[n]).abs());
    }
    println!("filt example: first samples = {first_four:?}, max_dev = {max_dev:e}");
    assert_eq!(first_four, vec![0.0, 3.0, -12.0, 42.0]);
    assert!(max_dev == 0.0, "max_dev = {max_dev:e}");
}

/// # Methodology
///
/// Regression guard in the spirit of bead `op-fm5`: a discrete block's
/// state must not grow with the sample index. A second-order ZOH
/// discretisation (`K_p = 1.5`, `tau_p = 2 s`, `zeta = 0.45`,
/// `T = 0.01 s`) is stepped 10,000 times with a varying pseudo-random
/// input, checking `state_size()` after every step. Pass criterion:
/// `state_size() == 2` at every one of the 10,000 steps.
///
/// # Results
///
/// Measured 2026-08-11 (crate 0.2.0, rustc 1.97.0, release profile):
/// state size stayed exactly **2** for all 10,000 steps.
#[test]
fn discrete_block_state_size_does_not_grow() {
    let g = ContinuousTransferFn::second_order(
        Ratio::new::<ratio>(1.5),
        Time::new::<second>(2.0),
        Ratio::new::<ratio>(0.45),
    )
    .unwrap();
    let mut d = g
        .to_discrete(Time::new::<second>(0.01), C2dMethod::Zoh)
        .unwrap();
    let inputs = lcg_sequence(10_000);
    for &u in &inputs {
        let _ = d.advance_one_sample(Ratio::new::<ratio>(u));
        assert_eq!(d.state_size(), 2);
    }
    println!("state size constant at 2 over {} steps", inputs.len());
}

/// # Methodology
///
/// Error-path checks: each unsupported or invalid request must return its
/// documented `ZDomainError` variant rather than a wrong answer. Cases:
/// (a) ZOH of an order-3 system -> `UnsupportedOrder { order: 3 }`;
/// (b) ZOH of an improper system (numerator degree 2 over denominator
/// degree 1) -> `ImproperTransferFunction`;
/// (c) descending-z constructor with numerator degree above denominator
/// degree -> `AcausalSystem`;
/// (d) `filt`-style constructor with `a0 = 0` -> `AcausalSystem`;
/// (e) prewarp with `w0 T >= pi` -> `InvalidPrewarpFrequency`;
/// (f) matched `d2c` of a system with a zero at `z = 0` ->
/// `MatchedPoleZeroAtOrigin`.
///
/// # Results
///
/// Measured 2026-08-11 (crate 0.2.0, rustc 1.97.0, release profile): all
/// six cases returned exactly the documented error variant.
#[test]
fn error_paths_return_documented_variants() {
    let t_samp = Time::new::<second>(0.1);

    // (a) order 3
    let g3 = ContinuousTransferFn::new(vec![1.0], vec![1.0, 3.0, 3.0, 1.0]).unwrap();
    assert_eq!(
        g3.to_discrete(t_samp, C2dMethod::Zoh).unwrap_err(),
        ZDomainError::UnsupportedOrder { order: 3 }
    );

    // (b) improper
    let gi = ContinuousTransferFn::new(vec![1.0, 0.0, 1.0], vec![1.0, 1.0]).unwrap();
    assert_eq!(
        gi.to_discrete(t_samp, C2dMethod::Zoh).unwrap_err(),
        ZDomainError::ImproperTransferFunction
    );

    // (c) acausal descending-z
    assert_eq!(
        DiscreteTransferFn::from_z_descending_coefficients(
            vec![1.0, 0.0, 0.0],
            vec![1.0, 0.5],
            t_samp
        )
        .unwrap_err(),
        ZDomainError::AcausalSystem
    );

    // (d) a0 = 0
    assert_eq!(
        DiscreteTransferFn::from_z_inverse_coefficients(vec![1.0], vec![0.0, 1.0], t_samp)
            .unwrap_err(),
        ZDomainError::AcausalSystem
    );

    // (e) prewarp beyond Nyquist: w0 T = 40 * 0.1 = 4 > pi
    let g1 = ContinuousTransferFn::first_order(Ratio::new::<ratio>(1.0), Time::new::<second>(1.0))
        .unwrap();
    assert_eq!(
        g1.to_discrete(
            t_samp,
            C2dMethod::TustinPrewarp {
                prewarp_frequency: AngularVelocity::new::<radian_per_second>(40.0)
            }
        )
        .unwrap_err(),
        ZDomainError::InvalidPrewarpFrequency
    );

    // (f) matched d2c with a numerator zero at z = 0:
    // H = 3 z^-1 / (1 + 4 z^-1 + 2 z^-2) has num(z) = 3 z, zero at origin
    let h = DiscreteTransferFn::from_z_inverse_coefficients(
        vec![0.0, 3.0],
        vec![1.0, 4.0, 2.0],
        t_samp,
    )
    .unwrap();
    assert_eq!(
        h.to_continuous(D2cMethod::MatchedPoleZero).unwrap_err(),
        ZDomainError::MatchedPoleZeroAtOrigin
    );

    println!("all six error paths returned their documented variants");
}
