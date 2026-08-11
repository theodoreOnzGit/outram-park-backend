//! Verification and regression tests for the exact O(1) recurrences that
//! replaced the growing response vectors in this module tree (bead
//! `op-fm5`).
//!
//! # Methodology (shared by every `*_matches_analytic_superposition` test)
//!
//! Each transfer-function block is driven with a deterministic,
//! always-changing input sequence — one that moves by far more than the
//! block's 9-decimal-place "input unchanged" threshold on every step, so that
//! every step really does inject a new increment. In parallel the same
//! increments are accumulated into a list of `(start_time, increment)` pairs
//! and the output is recomputed the slow way, by superposing the block's
//! **closed-form analytic step response** (`FirstOrderResponse`,
//! `SecondOrderStableStepResponse`, `DecaySinusoidResponse`,
//! `DecaySecondOrderExponentialResponse`) over every increment. That
//! superposition is the *reference*: it is the same mathematics the pre-0.2.0
//! implementation evaluated, only without the retirement truncation.
//!
//! **Pass criterion:** the largest absolute difference between the recurrence
//! and the analytic superposition, over every step of the run, must be below
//! `1e-9` (dimensionless output units). That threshold is chosen to be well
//! below the `exp(-20) = 2.06e-9` residual that the pre-0.2.0 code used to
//! *discard* when it retired a response, so passing means the recurrence is
//! at least as faithful as what it replaced.
//!
//! **Results** are recorded in each test's own doc comment, together with the
//! date they were taken and the machine.
//!
//! # A note on what changed numerically
//!
//! The old implementation dropped a step response once `t/tau > 20`, adding
//! its full steady-state value to an offset and discarding the remaining
//! `exp(-20) = 2.061e-9` of it. The recurrence does not truncate. So the new
//! output differs from the old by at most `2.061e-9 * |K_p du|` per retired
//! increment — and in the direction of *more* accuracy, since the discarded
//! residual is now carried rather than thrown away.

use std::time::Instant;

use uom::si::f64::*;
use uom::si::frequency::hertz;
use uom::si::ratio::ratio;
use uom::si::time::second;
use uom::ConstZero;

use super::decaying_exponentials::{DecaySecondOrderExponentialResponse, DecayingSecondOrderExponential};
use super::decaying_sinusoid::{DecaySinusoidResponse, DecayingSinusoid, TransferFnSinusoidType};
use super::first_order_transfer_fn::{FirstOrderResponse, FirstOrderStableTransferFnNoZeroes};
use super::first_order_transfer_fn_with_zeroes::FirstOrderStableTransferFnForZeroes;
use super::second_order_transfer_fn::{
    SecondOrderStableStepResponse, SecondOrderStableTransferFnNoZeroes,
};
use crate::alpha_nightly::controllers::{AnalogController, ProportionalController};
use crate::alpha_nightly::transfer_fn_wrapper_and_enums::TransferFnTraits;

/// A deterministic, always-changing drive signal, dimensionless. Chosen so
/// consecutive samples differ by far more than 1e-9 at every timestep used
/// in these tests, so the block's "input unchanged" guard never fires.
fn drive_signal(time_seconds: f64) -> Ratio {
    Ratio::new::<ratio>((0.37 * time_seconds).sin() * 0.5 + 0.021 * time_seconds)
}

/// The absolute value of a dimensionless quantity, as a plain `f64`.
fn abs_ratio(value: Ratio) -> f64 {
    value.get::<ratio>().abs()
}

/// Verifies the first-order lag's recurrence against the closed-form
/// superposition of `FirstOrderResponse` step responses.
///
/// # Methodology
///
/// Block: `K_p / (tau_p s + 1)` with `K_p = 1.75` (dimensionless),
/// `tau_p = 1.75 s`, no dead time, starting from input 0 and output 0.
/// Driven for 2000 steps at `dt = 0.01 s` (20 s of simulated time, i.e.
/// 11.4 time constants) with the always-changing signal `drive_signal`.
/// Reference: `initial_value + sum_k FirstOrderResponse(K_p, tau_p, t_k,
/// du_k).calculate_response(t)` over every increment injected so far.
/// Pass criterion: max absolute difference below 1e-9 (dimensionless).
///
/// # Results
///
/// Measured 2026-08-11 on AMD Ryzen 5 5600 (rustc 1.97.0, `--release`):
/// **max absolute difference 6.33e-15** over 2000 steps, against a tolerance
/// of 1e-9 — nearly six orders of magnitude of margin, and consistent with
/// pure floating-point round-off (the output reaches order 10, so this is a
/// few ulp accumulated over 2000 steps). The recurrence is exact for
/// piecewise-constant input, as the zero-order-hold theory says it should be;
/// what is left is round-off, not discretisation error.
#[test]
fn first_order_recurrence_matches_analytic_superposition() {
    let process_gain = Ratio::new::<ratio>(1.75);
    let process_time = Time::new::<second>(1.75);
    let timestep = 0.01_f64;

    let mut block = FirstOrderStableTransferFnNoZeroes::new(
        process_gain,
        process_time,
        Ratio::ZERO,
        Ratio::ZERO,
        Time::ZERO,
    )
    .unwrap();

    let mut increments: Vec<(Time, Ratio)> = Vec::new();
    let mut previous_input = Ratio::ZERO;
    let mut worst_difference = 0.0_f64;

    for step in 0..2000usize {
        let time_seconds = step as f64 * timestep;
        let current_time = Time::new::<second>(time_seconds);
        let current_input = drive_signal(time_seconds);

        let recurrence_output = block
            .set_user_input_and_calc_output(current_time, current_input)
            .unwrap();

        increments.push((current_time, current_input - previous_input));
        previous_input = current_input;

        let mut analytic_output = Ratio::ZERO;
        for (start_time, increment) in increments.iter().copied() {
            let mut response = FirstOrderResponse::new(
                process_gain,
                process_time,
                start_time,
                increment,
                current_time,
            )
            .unwrap();
            analytic_output += response.calculate_response(current_time);
        }

        worst_difference = worst_difference.max(abs_ratio(recurrence_output - analytic_output));
    }

    println!("first order: max |recurrence - analytic| = {worst_difference:.3e}");
    assert!(
        worst_difference < 1e-9,
        "first-order recurrence drifted from the analytic superposition by {worst_difference:.3e}"
    );
}

/// Verifies the zero-bearing first-order block's recurrence against the
/// closed-form superposition it replaced.
///
/// # Methodology
///
/// Block: `K_p s tau_p / (tau_p s + 1)` with `K_p = 0.8` (dimensionless),
/// `tau_p = 2.5 s`, no dead time. Driven for 2000 steps at `dt = 0.01 s`.
/// Reference: `FirstOrderStableTransferFnForZeroes::reference_response`,
/// which superposes one `StepFunction` (gain `+K_p`) and one
/// `FirstOrderResponse` (gain `-K_p`) per increment — exactly the two vectors
/// the pre-0.2.0 implementation carried. Pass criterion: max absolute
/// difference below 1e-9 (dimensionless).
///
/// # Results
///
/// Measured 2026-08-11 on AMD Ryzen 5 5600 (rustc 1.97.0, `--release`):
/// **max absolute difference 1.69e-15** over 2000 steps, against a tolerance
/// of 1e-9. Round-off only.
#[test]
fn first_order_with_zeroes_recurrence_matches_analytic_superposition() {
    let process_gain = Ratio::new::<ratio>(0.8);
    let process_time = Time::new::<second>(2.5);
    let timestep = 0.01_f64;

    let mut block = FirstOrderStableTransferFnForZeroes::new(
        process_gain,
        process_time,
        Ratio::ZERO,
        Ratio::ZERO,
        Time::ZERO,
    )
    .unwrap();

    let mut increments: Vec<(Time, Ratio)> = Vec::new();
    let mut previous_input = Ratio::ZERO;
    let mut worst_difference = 0.0_f64;

    for step in 0..2000usize {
        let time_seconds = step as f64 * timestep;
        let current_time = Time::new::<second>(time_seconds);
        let current_input = drive_signal(time_seconds);

        let recurrence_output = block
            .set_user_input_and_calc_output(current_time, current_input)
            .unwrap();

        increments.push((current_time, current_input - previous_input));
        previous_input = current_input;

        let analytic_output = block.reference_response(&increments, current_time).unwrap();

        worst_difference = worst_difference.max(abs_ratio(recurrence_output - analytic_output));
    }

    println!("first order with zeroes: max |recurrence - analytic| = {worst_difference:.3e}");
    assert!(
        worst_difference < 1e-9,
        "zero-bearing recurrence drifted from the analytic superposition by {worst_difference:.3e}"
    );
}

/// Verifies the second-order lag's recurrence in **all three damping
/// regimes** against the closed-form superposition of
/// `SecondOrderStableStepResponse` step responses.
///
/// # Methodology
///
/// Block: `K_p / (tau^2 s^2 + 2 tau zeta s + 1)` with `K_p = 1.3`
/// (dimensionless) and `tau = 1.5 s`, run once per damping factor in
/// `{0.35, 1.0, 2.4, 1.0001, 1.000001, 50.0}` — covering underdamped,
/// exactly critically damped, ordinarily overdamped, **two near-critical
/// overdamped cases** and a heavily overdamped case. No dead time. 2000
/// steps at `dt = 0.01 s`. Reference: superposition of
/// `SecondOrderStableStepResponse` over every increment. Pass criterion: max
/// absolute difference below 1e-9 (dimensionless) in each regime.
///
/// The near-critical cases are included deliberately: the recurrence's
/// overdamped branch divides by `2 sqrt(zeta^2 - 1)`, which tends to zero as
/// `zeta` tends to 1 from above, so that is where the decomposition is worst
/// conditioned.
///
/// # Results
///
/// Measured 2026-08-11 on AMD Ryzen 5 5600 (rustc 1.97.0, `--release`):
///
/// | `zeta` | regime | max abs difference |
/// |---|---|---|
/// | 0.35 | underdamped | 9.05e-15 |
/// | 1.00 | critically damped | 5.11e-15 |
/// | 2.40 | overdamped | 6.94e-15 |
/// | 1.0001 | near-critical overdamped | 6.20e-14 |
/// | 1.000001 | near-critical overdamped | 8.55e-13 |
/// | 50.0 | heavily overdamped | 4.00e-15 |
///
/// All against a tolerance of 1e-9, so between three and six orders of
/// margin. The overdamped cases are checked specifically because the
/// recurrence there uses a two-real-exponential decomposition rather than the
/// `cosh`/`sinh` form of the analytic reference — agreement at round-off
/// confirms the two forms are algebraically identical, as derived in the
/// module documentation.
///
/// The near-critical results are the interesting ones. The error grows
/// roughly as `1/sqrt(zeta - 1)`, as the `1/(2 sqrt(zeta^2 - 1))` weighting
/// predicts: a 100-fold decrease in `zeta - 1` (1e-4 to 1e-6) cost about 14x
/// accuracy (6.20e-14 to 8.55e-13). That is a graceful degradation, it is
/// still three orders of magnitude inside tolerance at `zeta - 1 = 1e-6`, and
/// the analytic reference shares the same conditioning (its `sinh(b t)/b`
/// term is the same 0/0 limit), so this is a property of the problem rather
/// than a regression introduced by the recurrence. If a caller ever needs
/// `zeta` within about 1e-8 of 1, it should use the critically damped branch.
#[test]
fn second_order_recurrence_matches_analytic_superposition() {
    let process_gain = Ratio::new::<ratio>(1.3);
    let process_time = Time::new::<second>(1.5);
    let timestep = 0.01_f64;

    for damping_value in [0.35_f64, 1.0_f64, 2.4_f64, 1.0001_f64, 1.000001_f64, 50.0_f64] {
        let damping_factor = Ratio::new::<ratio>(damping_value);

        let mut block = SecondOrderStableTransferFnNoZeroes::new(
            process_gain,
            process_time,
            damping_factor,
            Ratio::ZERO,
            Ratio::ZERO,
            Time::ZERO,
        )
        .unwrap();

        let mut increments: Vec<(Time, Ratio)> = Vec::new();
        let mut previous_input = Ratio::ZERO;
        let mut worst_difference = 0.0_f64;

        for step in 0..2000usize {
            let time_seconds = step as f64 * timestep;
            let current_time = Time::new::<second>(time_seconds);
            let current_input = drive_signal(time_seconds);

            let recurrence_output = block
                .set_user_input_and_calc_output(current_time, current_input)
                .unwrap();

            increments.push((current_time, current_input - previous_input));
            previous_input = current_input;

            let mut analytic_output = Ratio::ZERO;
            for (start_time, increment) in increments.iter().copied() {
                let mut response = SecondOrderStableStepResponse::new(
                    process_gain,
                    process_time,
                    damping_factor,
                    start_time,
                    increment,
                    current_time,
                )
                .unwrap();
                analytic_output += response.calculate_response(current_time);
            }

            worst_difference = worst_difference.max(abs_ratio(recurrence_output - analytic_output));
        }

        println!(
            "second order zeta={damping_value}: max |recurrence - analytic| = {worst_difference:.3e}"
        );
        assert!(
            worst_difference < 1e-9,
            "second-order recurrence at zeta={damping_value} drifted by {worst_difference:.3e}"
        );
    }
}

/// Verifies the decaying-sinusoid block's recurrence, in both the sine and
/// cosine projections, against the closed-form superposition of
/// `DecaySinusoidResponse`.
///
/// # Methodology
///
/// Block: `M exp(-a t) {sin, cos}(omega t)` with `M = 0.9` (dimensionless),
/// `a = 0.4 Hz`, `omega = 2.1 Hz`, no dead time. 2000 steps at
/// `dt = 0.01 s`. Reference: superposition of `DecaySinusoidResponse` over
/// every increment. Pass criterion: max absolute difference below 1e-9
/// (dimensionless) for each projection.
///
/// # Results
///
/// Measured 2026-08-11 on AMD Ryzen 5 5600 (rustc 1.97.0, `--release`):
/// **sine 4.16e-16, cosine 4.86e-16** over 2000 steps, against a tolerance of
/// 1e-9. This is the test that exercises the decaying-rotation update, which
/// is the least obvious of the recurrences.
#[test]
fn decaying_sinusoid_recurrence_matches_analytic_superposition() {
    let magnitude = Ratio::new::<ratio>(0.9);
    let decay_frequency = Frequency::new::<hertz>(0.4);
    let omega = Frequency::new::<hertz>(2.1);
    let timestep = 0.01_f64;

    for sinusoid_type in [
        TransferFnSinusoidType::Sine,
        TransferFnSinusoidType::Cosine,
    ] {
        let mut block = match sinusoid_type {
            TransferFnSinusoidType::Sine => DecayingSinusoid::new_sine(
                magnitude,
                decay_frequency,
                Ratio::ZERO,
                Ratio::ZERO,
                Time::ZERO,
                omega,
            )
            .unwrap(),
            TransferFnSinusoidType::Cosine => DecayingSinusoid::new_cosine(
                magnitude,
                decay_frequency,
                Ratio::ZERO,
                Ratio::ZERO,
                Time::ZERO,
                omega,
            )
            .unwrap(),
        };

        let mut increments: Vec<(Time, Ratio)> = Vec::new();
        let mut previous_input = Ratio::ZERO;
        let mut worst_difference = 0.0_f64;

        for step in 0..2000usize {
            let time_seconds = step as f64 * timestep;
            let current_time = Time::new::<second>(time_seconds);
            let current_input = drive_signal(time_seconds);

            let recurrence_output = block
                .set_user_input_and_calc_output(current_time, current_input)
                .unwrap();

            increments.push((current_time, current_input - previous_input));
            previous_input = current_input;

            let mut analytic_output = Ratio::ZERO;
            for (start_time, increment) in increments.iter().copied() {
                let mut response = DecaySinusoidResponse::new(
                    magnitude,
                    decay_frequency,
                    start_time,
                    increment,
                    current_time,
                    omega,
                    sinusoid_type,
                )
                .unwrap();
                analytic_output += response.calculate_response(current_time);
            }

            worst_difference = worst_difference.max(abs_ratio(recurrence_output - analytic_output));
        }

        println!(
            "decaying sinusoid {sinusoid_type:?}: max |recurrence - analytic| = {worst_difference:.3e}"
        );
        assert!(
            worst_difference < 1e-9,
            "decaying-sinusoid recurrence ({sinusoid_type:?}) drifted by {worst_difference:.3e}"
        );
    }
}

/// Verifies the decaying-exponential block's recurrence, in both the
/// overdamped and critically damped root structures, against the closed-form
/// superposition of `DecaySecondOrderExponentialResponse`.
///
/// # Methodology
///
/// Overdamped: `M_a exp(-alpha t) + M_b exp(-beta t)` with `M_a = 0.6`,
/// `M_b = -0.25` (dimensionless), `alpha = 0.7 Hz`, `beta = 1.9 Hz`.
/// Critically damped: `M_a t exp(-lambda t) + M_b exp(-lambda t)` with
/// `M_a = 0.45 Hz` (a rate, since it multiplies a time), `M_b = 0.3`
/// (dimensionless), `lambda = 1.1 Hz`. Both with no dead time, 2000 steps at
/// `dt = 0.01 s`. Pass criterion: max absolute difference below 1e-9
/// (dimensionless).
///
/// # Results
///
/// Measured 2026-08-11 on AMD Ryzen 5 5600 (rustc 1.97.0, `--release`):
/// **overdamped 8.60e-16, critically damped 6.66e-16** over 2000 steps,
/// against a tolerance of 1e-9. The critically damped case is the one that
/// exercises the `t exp(-lambda t)` shear update, which needs a second
/// companion mode to close.
#[test]
fn decaying_exponential_recurrence_matches_analytic_superposition() {
    let timestep = 0.01_f64;

    // --- overdamped: two distinct real roots ---
    {
        let magnitude_alpha = Ratio::new::<ratio>(0.6);
        let magnitude_beta = Ratio::new::<ratio>(-0.25);
        let alpha = Frequency::new::<hertz>(0.7);
        let beta = Frequency::new::<hertz>(1.9);

        let mut block = DecayingSecondOrderExponential::new_overdamped(
            magnitude_alpha,
            magnitude_beta,
            alpha,
            beta,
            Ratio::ZERO,
            Ratio::ZERO,
            Time::ZERO,
        )
        .unwrap();

        let mut increments: Vec<(Time, Ratio)> = Vec::new();
        let mut previous_input = Ratio::ZERO;
        let mut worst_difference = 0.0_f64;

        for step in 0..2000usize {
            let time_seconds = step as f64 * timestep;
            let current_time = Time::new::<second>(time_seconds);
            let current_input = drive_signal(time_seconds);

            let recurrence_output = block
                .set_user_input_and_calc_output(current_time, current_input)
                .unwrap();

            increments.push((current_time, current_input - previous_input));
            previous_input = current_input;

            let mut analytic_output = Ratio::ZERO;
            for (start_time, increment) in increments.iter().copied() {
                let mut response = DecaySecondOrderExponentialResponse::new_overdamped(
                    magnitude_alpha * increment,
                    magnitude_beta * increment,
                    alpha,
                    beta,
                    start_time,
                    increment,
                    current_time,
                )
                .unwrap();
                analytic_output += response.calculate_response(current_time);
            }

            worst_difference = worst_difference.max(abs_ratio(recurrence_output - analytic_output));
        }

        println!(
            "decaying exponential overdamped: max |recurrence - analytic| = {worst_difference:.3e}"
        );
        assert!(
            worst_difference < 1e-9,
            "overdamped decaying-exponential recurrence drifted by {worst_difference:.3e}"
        );
    }

    // --- critically damped: one repeated real root ---
    {
        let magnitude_alpha_rate = Frequency::new::<hertz>(0.45);
        let magnitude_beta = Ratio::new::<ratio>(0.3);
        let lambda = Frequency::new::<hertz>(1.1);

        let mut block = DecayingSecondOrderExponential::new_critical(
            magnitude_alpha_rate,
            magnitude_beta,
            lambda,
            Ratio::ZERO,
            Ratio::ZERO,
            Time::ZERO,
        )
        .unwrap();

        let mut increments: Vec<(Time, Ratio)> = Vec::new();
        let mut previous_input = Ratio::ZERO;
        let mut worst_difference = 0.0_f64;

        for step in 0..2000usize {
            let time_seconds = step as f64 * timestep;
            let current_time = Time::new::<second>(time_seconds);
            let current_input = drive_signal(time_seconds);

            let recurrence_output = block
                .set_user_input_and_calc_output(current_time, current_input)
                .unwrap();

            increments.push((current_time, current_input - previous_input));
            previous_input = current_input;

            let mut analytic_output = Ratio::ZERO;
            for (start_time, increment) in increments.iter().copied() {
                let mut response = DecaySecondOrderExponentialResponse::new_critical(
                    magnitude_alpha_rate * increment.get::<ratio>(),
                    magnitude_beta * increment,
                    lambda,
                    lambda,
                    start_time,
                    increment,
                    current_time,
                )
                .unwrap();
                analytic_output += response.calculate_response(current_time);
            }

            worst_difference = worst_difference.max(abs_ratio(recurrence_output - analytic_output));
        }

        println!(
            "decaying exponential critically damped: max |recurrence - analytic| = {worst_difference:.3e}"
        );
        assert!(
            worst_difference < 1e-9,
            "critically damped decaying-exponential recurrence drifted by {worst_difference:.3e}"
        );
    }
}

/// Verifies that the recurrence stays exact when the timestep is **irregular**
/// — the capability the pre-0.2.0 superposition had by construction and which
/// a fixed-grid recurrence would have silently lost.
///
/// # Methodology
///
/// Same first-order lag as
/// [`first_order_recurrence_matches_analytic_superposition`]
/// (`K_p = 1.75`, `tau_p = 1.75 s`) but stepped with a timestep that varies
/// pseudo-randomly between roughly 0.002 s and 0.05 s from step to step, for
/// 2000 steps. Reference: the same analytic superposition, evaluated at the
/// actual (irregular) sample times. Pass criterion: max absolute difference
/// below 1e-9 (dimensionless).
///
/// # Results
///
/// Measured 2026-08-11 on AMD Ryzen 5 5600 (rustc 1.97.0, `--release`):
/// **max absolute difference 7.99e-15** over 2000 irregular steps, against a
/// tolerance of 1e-9 — the same round-off floor as the fixed-grid case
/// (6.33e-15). The recurrence uses the actual elapsed time on every call, so
/// an irregular grid costs it nothing in accuracy.
#[test]
fn first_order_recurrence_is_exact_on_an_irregular_time_grid() {
    let process_gain = Ratio::new::<ratio>(1.75);
    let process_time = Time::new::<second>(1.75);

    let mut block = FirstOrderStableTransferFnNoZeroes::new(
        process_gain,
        process_time,
        Ratio::ZERO,
        Ratio::ZERO,
        Time::ZERO,
    )
    .unwrap();

    let mut increments: Vec<(Time, Ratio)> = Vec::new();
    let mut previous_input = Ratio::ZERO;
    let mut worst_difference = 0.0_f64;
    let mut time_seconds = 0.0_f64;

    for step in 0..2000usize {
        // deterministic but irregular: roughly 0.002 s to 0.05 s
        let jitter = ((step as f64 * 12.9898).sin() * 43758.5453).fract().abs();
        let current_time = Time::new::<second>(time_seconds);
        let current_input = drive_signal(time_seconds);

        let recurrence_output = block
            .set_user_input_and_calc_output(current_time, current_input)
            .unwrap();

        increments.push((current_time, current_input - previous_input));
        previous_input = current_input;

        let mut analytic_output = Ratio::ZERO;
        for (start_time, increment) in increments.iter().copied() {
            let mut response = FirstOrderResponse::new(
                process_gain,
                process_time,
                start_time,
                increment,
                current_time,
            )
            .unwrap();
            analytic_output += response.calculate_response(current_time);
        }

        worst_difference = worst_difference.max(abs_ratio(recurrence_output - analytic_output));

        time_seconds += 0.002 + 0.048 * jitter;
    }

    println!("irregular grid: max |recurrence - analytic| = {worst_difference:.3e}");
    assert!(
        worst_difference < 1e-9,
        "first-order recurrence drifted on an irregular grid by {worst_difference:.3e}"
    );
}

/// Verifies that a **dead time** (transport delay) still behaves like the
/// analytic superposition, and that the pending-input queue is bounded by the
/// dead time rather than by the run length.
///
/// # Methodology
///
/// Same first-order lag (`K_p = 1.75`, `tau_p = 1.75 s`) but with a dead time
/// of 0.25 s, stepped at `dt = 0.01 s` for 2000 steps. Reference: the same
/// analytic superposition, with each increment's start time shifted by the
/// dead time. Pass criterion: max absolute difference below 1e-9
/// (dimensionless), and the queue of not-yet-due increments never exceeds
/// `ceil(dead_time/dt) + 1 = 26` entries no matter how long the run goes.
///
/// # Results
///
/// Measured 2026-08-11 on AMD Ryzen 5 5600 (rustc 1.97.0, `--release`):
/// **max absolute difference 6.11e-15**, and the pending queue peaked at
/// **26 entries** and stayed there for the rest of the run — flat, not
/// growing. 26 is the 25 timesteps of 0.01 s that fit inside the 0.25 s dead
/// time, plus the increment pushed on the current step before any becomes
/// due. That is the irreducible amount of information a transport delay has
/// to remember, and unlike the old response vector it is set by the dead time
/// rather than by the run length.
#[test]
fn first_order_recurrence_handles_dead_time_with_a_bounded_queue() {
    let process_gain = Ratio::new::<ratio>(1.75);
    let process_time = Time::new::<second>(1.75);
    let dead_time = Time::new::<second>(0.25);
    let timestep = 0.01_f64;

    let mut block = FirstOrderStableTransferFnNoZeroes::new(
        process_gain,
        process_time,
        Ratio::ZERO,
        Ratio::ZERO,
        dead_time,
    )
    .unwrap();

    let mut increments: Vec<(Time, Ratio)> = Vec::new();
    let mut previous_input = Ratio::ZERO;
    let mut worst_difference = 0.0_f64;
    let mut worst_queue_length = 0usize;

    for step in 0..2000usize {
        let time_seconds = step as f64 * timestep;
        let current_time = Time::new::<second>(time_seconds);
        let current_input = drive_signal(time_seconds);

        let recurrence_output = block
            .set_user_input_and_calc_output(current_time, current_input)
            .unwrap();

        increments.push((current_time + dead_time, current_input - previous_input));
        previous_input = current_input;

        let mut analytic_output = Ratio::ZERO;
        for (start_time, increment) in increments.iter().copied() {
            let mut response = FirstOrderResponse::new(
                process_gain,
                process_time,
                start_time,
                increment,
                current_time,
            )
            .unwrap();
            analytic_output += response.calculate_response(current_time);
        }

        worst_difference = worst_difference.max(abs_ratio(recurrence_output - analytic_output));
        worst_queue_length = worst_queue_length.max(block.pending_inputs.len());
    }

    println!(
        "dead time: max |recurrence - analytic| = {worst_difference:.3e}, \
         max pending queue = {worst_queue_length}"
    );
    assert!(
        worst_difference < 1e-9,
        "dead-time recurrence drifted from the analytic superposition by {worst_difference:.3e}"
    );
    // 0.25 s of dead time at 0.01 s per step is 25 steps of buffer; allow one
    // extra for the boundary. What matters is that this does not grow with
    // the 2000-step run length.
    assert!(
        worst_queue_length <= 26,
        "pending-input queue grew to {worst_queue_length}, which is more than the dead time needs"
    );
}

/// **Regression test for bead `op-fm5`**: the state a transfer-function block
/// carries must not grow with the step index.
///
/// This is the test that would have caught the original defect. Before
/// version 0.2.0 each of these blocks pushed one response object per input
/// change onto a `Vec` and re-summed it every step; with no dead time the
/// state is now a fixed handful of numbers instead.
///
/// # Methodology
///
/// Each of the five affected block types is driven for 100,000 steps at
/// `dt = 0.001 s` with an always-changing input — the same 1 ms timestep at
/// which the old code degraded to 1176 us/step. `state_size()` is sampled at
/// steps 100 and 99,999. Every block is given a time constant long enough
/// (>= 80 s, matching the `teh-o-prke` case that grew to 1.6 M entries) that
/// the old retirement horizon of `20 tau / dt` would never be reached inside
/// the run. Pass criterion: the state size at step 99,999 equals the state
/// size at step 100, exactly.
///
/// # Results
///
/// Measured 2026-08-11 on AMD Ryzen 5 5600 (rustc 1.97.0, `--release`):
/// every block reported an **identical state size at step 100 and step
/// 99,999**, namely `[2, 1, 3, 2, 3]` — 2 for the first-order lag, 1 for the
/// zero-bearing block, 3 for the second-order lag, 2 for the decaying
/// sinusoid and 3 for the decaying exponential. Under the pre-0.2.0
/// implementation the corresponding live-response counts would have been 101
/// and 100,000. The whole 100,000-step, five-block run now completes in well
/// under a second.
#[test]
fn block_state_size_does_not_grow_with_step_index() {
    let long_process_time = Time::new::<second>(80.8);
    let timestep = 0.001_f64;
    let total_steps = 100_000usize;
    let early_step = 100usize;
    let late_step = total_steps - 1;

    let mut first_order = FirstOrderStableTransferFnNoZeroes::new(
        Ratio::new::<ratio>(1.75),
        long_process_time,
        Ratio::ZERO,
        Ratio::ZERO,
        Time::ZERO,
    )
    .unwrap();
    let mut with_zeroes = FirstOrderStableTransferFnForZeroes::new(
        Ratio::new::<ratio>(0.8),
        long_process_time,
        Ratio::ZERO,
        Ratio::ZERO,
        Time::ZERO,
    )
    .unwrap();
    let mut second_order = SecondOrderStableTransferFnNoZeroes::new(
        Ratio::new::<ratio>(1.3),
        long_process_time,
        Ratio::new::<ratio>(0.35),
        Ratio::ZERO,
        Ratio::ZERO,
        Time::ZERO,
    )
    .unwrap();
    let mut sinusoid = DecayingSinusoid::new_sine(
        Ratio::new::<ratio>(0.9),
        Frequency::new::<hertz>(1.0 / 80.8),
        Ratio::ZERO,
        Ratio::ZERO,
        Time::ZERO,
        Frequency::new::<hertz>(2.1),
    )
    .unwrap();
    let mut exponential = DecayingSecondOrderExponential::new_overdamped(
        Ratio::new::<ratio>(0.6),
        Ratio::new::<ratio>(-0.25),
        Frequency::new::<hertz>(1.0 / 80.8),
        Frequency::new::<hertz>(1.0 / 100.0),
        Ratio::ZERO,
        Ratio::ZERO,
        Time::ZERO,
    )
    .unwrap();

    let mut early_sizes = [0usize; 5];
    let mut late_sizes = [0usize; 5];

    for step in 0..total_steps {
        let time_seconds = step as f64 * timestep;
        let current_time = Time::new::<second>(time_seconds);
        let current_input = drive_signal(time_seconds);

        first_order
            .set_user_input_and_calc_output(current_time, current_input)
            .unwrap();
        with_zeroes
            .set_user_input_and_calc_output(current_time, current_input)
            .unwrap();
        second_order
            .set_user_input_and_calc_output(current_time, current_input)
            .unwrap();
        sinusoid
            .set_user_input_and_calc_output(current_time, current_input)
            .unwrap();
        exponential
            .set_user_input_and_calc_output(current_time, current_input)
            .unwrap();

        let sizes = [
            first_order.state_size(),
            with_zeroes.state_size(),
            second_order.state_size(),
            sinusoid.state_size(),
            exponential.state_size(),
        ];

        if step == early_step {
            early_sizes = sizes;
        }
        if step == late_step {
            late_sizes = sizes;
        }
    }

    println!("state size at step {early_step}: {early_sizes:?}");
    println!("state size at step {late_step}: {late_sizes:?}");
    assert_eq!(
        early_sizes, late_sizes,
        "transfer-function state grew between step {early_step} and step {late_step}; \
         this is exactly the op-fm5 defect coming back"
    );
}

/// **Regression test for bead `op-fm5`**, at the level a user actually feels
/// it: the wall-clock cost of one filtered-PID step must not grow with the
/// step index.
///
/// # Methodology
///
/// `AnalogController::new_filtered_pid_controller(1.75, 1.75 s, 80.8 s, 1.0)`
/// — the shipped TUAS gains, but with a derivative filter time constant of
/// 80.8 s, matching the `teh-o-prke` case whose retirement horizon
/// (`20 * 80.8 / 0.001` = 1.6 M steps) is never reached. Stepped at
/// `dt = 0.001 s`, the timestep at which the old implementation degraded
/// worst. A 5,000-step warm-up is discarded, then the mean cost of a
/// 2,000-step window is measured at step 5,000 and again at step 100,000.
/// A bare `ProportionalController` — a pure gain, which the old code also
/// routed through the same machinery — is checked the same way.
/// Pass criterion: the late window costs no more than **5x** the early
/// window. The factor is deliberately loose because this is a wall-clock
/// measurement on a shared machine; the defect it guards against was a
/// factor of 20 to 40, and unbounded.
///
/// # Results
///
/// Measured 2026-08-11 on AMD Ryzen 5 5600 (rustc 1.97.0, `--release`):
///
/// | block | step 5,000 | step 100,000 | ratio |
/// |---|---|---|---|
/// | filtered PID, `tau_d = 80.8 s` | 0.279 us/step | 0.279 us/step | 1.00 |
/// | bare P controller | 0.105 us/step | 0.104 us/step | 1.00 |
///
/// (Taken while the machine was under heavy concurrent load, which is why the
/// absolute figures are above the 0.17 us/step measured on an idle machine.
/// The ratio, which is what the test asserts, is unaffected.)
///
/// Flat, as an O(1) recurrence must be. For comparison, the pre-0.2.0 code
/// measured on the same machine took 58.2 us/step at step 1,000 and
/// 2329.5 us/step at step 79,000 for the same PID configuration — a ratio of
/// 40 and still climbing. Wall-clock figures are specific to this CPU; the
/// machine-independent claim is the ratio.
#[test]
fn pid_step_cost_does_not_grow_with_step_index() {
    let timestep = 0.001_f64;

    let mut controller = AnalogController::new_filtered_pid_controller(
        Ratio::new::<ratio>(1.75),
        Time::new::<second>(1.75),
        Time::new::<second>(80.8),
        Ratio::new::<ratio>(1.0),
    )
    .unwrap();
    let mut proportional = AnalogController::P(
        ProportionalController::new(Ratio::new::<ratio>(1.75)).unwrap(),
    );

    let window = 2_000usize;
    // Each checkpoint is measured as the MINIMUM of several repeated
    // windows. Wall-clock timing on a shared machine is noisy in one
    // direction only -- contention can add time, never remove it -- so the
    // minimum is a far more stable estimator than a single sample.
    let repeats = 5usize;
    let early_checkpoint = 5_000usize;
    let late_checkpoint = 100_000usize;

    let mut early_pid_seconds = 0.0_f64;
    let mut late_pid_seconds = 0.0_f64;
    let mut early_p_seconds = 0.0_f64;
    let mut late_p_seconds = 0.0_f64;
    let mut sink = 0.0_f64;

    let mut step = 0usize;
    while step < late_checkpoint + window {
        let at_early = step == early_checkpoint;
        let at_late = step == late_checkpoint;

        if at_early || at_late {
            let mut pid_seconds = f64::INFINITY;
            let mut p_seconds = f64::INFINITY;

            for _ in 0..repeats {
                let pid_start = Instant::now();
                for offset in 0..window {
                    let time_seconds = (step + offset) as f64 * timestep;
                    sink += controller
                        .set_user_input_and_calc(
                            drive_signal(time_seconds),
                            Time::new::<second>(time_seconds),
                        )
                        .unwrap()
                        .get::<ratio>();
                }
                pid_seconds = pid_seconds.min(pid_start.elapsed().as_secs_f64());

                let p_start = Instant::now();
                for offset in 0..window {
                    let time_seconds = (step + offset) as f64 * timestep;
                    sink += proportional
                        .set_user_input_and_calc(
                            drive_signal(time_seconds),
                            Time::new::<second>(time_seconds),
                        )
                        .unwrap()
                        .get::<ratio>();
                }
                p_seconds = p_seconds.min(p_start.elapsed().as_secs_f64());

                step += window;
            }

            if at_early {
                early_pid_seconds = pid_seconds;
                early_p_seconds = p_seconds;
            } else {
                late_pid_seconds = pid_seconds;
                late_p_seconds = p_seconds;
            }
        } else {
            let time_seconds = step as f64 * timestep;
            sink += controller
                .set_user_input_and_calc(
                    drive_signal(time_seconds),
                    Time::new::<second>(time_seconds),
                )
                .unwrap()
                .get::<ratio>();
            sink += proportional
                .set_user_input_and_calc(
                    drive_signal(time_seconds),
                    Time::new::<second>(time_seconds),
                )
                .unwrap()
                .get::<ratio>();
            step += 1;
        }
    }

    let early_pid_us = early_pid_seconds * 1e6 / window as f64;
    let late_pid_us = late_pid_seconds * 1e6 / window as f64;
    let early_p_us = early_p_seconds * 1e6 / window as f64;
    let late_p_us = late_p_seconds * 1e6 / window as f64;

    println!(
        "PID us/step: {early_pid_us:.4} @{early_checkpoint} -> {late_pid_us:.4} @{late_checkpoint} \
         (ratio {:.2})",
        late_pid_us / early_pid_us
    );
    println!(
        "P   us/step: {early_p_us:.4} @{early_checkpoint} -> {late_p_us:.4} @{late_checkpoint} \
         (ratio {:.2})",
        late_p_us / early_p_us
    );
    println!("(sink {sink:.3})");

    assert!(
        late_pid_us < 5.0 * early_pid_us,
        "filtered PID step cost grew from {early_pid_us:.4} us to {late_pid_us:.4} us per step; \
         this is the op-fm5 defect coming back"
    );
    assert!(
        late_p_us < 5.0 * early_p_us,
        "proportional controller step cost grew from {early_p_us:.4} us to {late_p_us:.4} us \
         per step; this is the op-fm5 defect coming back"
    );
}
