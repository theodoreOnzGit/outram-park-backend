//! The zero-bearing half of a first-order transfer function, advanced by an
//! **exact** constant-time recurrence.
//!
//! A general first-order block `(a1 s + b1)/(a2 s + b2)` splits into a lag
//! with no zeroes (handled by
//! [`super::first_order_transfer_fn::FirstOrderStableTransferFnNoZeroes`])
//! plus the term contributed by the zero, which is what this module handles:
//!
//! ```text
//!                 /             1        \        K_p tau_p s
//! G(s) = K_p * |  1 -  ----------------  |  =  ---------------
//!                 \        tau_p s + 1   /       tau_p s + 1
//! ```
//!
//! Its response to an input increment `a_1` applied at `t_1` is a pure
//! decaying exponential,
//!
//! ```text
//! y(t) = u(t - t_1) a_1 K_p exp(-(t - t_1)/tau_p)
//! ```
//!
//! — the immediate step `a_1 K_p` minus the lag's `a_1 K_p [1 - exp(...)]`.
//! Superposing those over every past input therefore needs only **one**
//! accumulator, decayed geometrically once per timestep. This is the
//! zero-order-hold / step-invariant equivalent of the block and is exact (not
//! an approximation) for input held constant between calls; see the module
//! documentation of [`super::first_order_transfer_fn`] for the derivation
//! sketch and the full reference list (Astrom and Wittenmark, *Computer-
//! Controlled Systems: Theory and Design*, Prentice Hall; Seborg, Edgar,
//! Mellichamp and Doyle, *Process Dynamics and Control*, Wiley; and others).
//!
//! Until version 0.2.0 this block carried **two** growing vectors — one of
//! [`super::first_order_transfer_fn::FirstOrderResponse`] and one of
//! [`super::step_fn::StepFunction`] — and re-summed both on every call, so it
//! cost twice what the plain lag did. See bead `op-fm5`.

use std::collections::VecDeque;

use uom::si::f64::*;
use uom::si::ratio::ratio;
use uom::si::time::second;
use uom::ConstZero;

use super::first_order_transfer_fn::PendingStepInput;
use crate::beta_testing::errors::ChemEngProcessControlSimulatorError;

/// Transfer function in the form:
///
/// G(s) = K_p [1 - 1/(tau_p s + 1)]
///
/// This comes from:
/// G(s) =  K_p s / (tau_p s + 1)
///
/// # Physical quantity
///
/// Maps a dimensionless input signal to a dimensionless output signal. This
/// is the "derivative-ish" part of a first-order block: it responds
/// immediately to a change in input and then washes out with time constant
/// `tau_p`, contributing nothing at steady state.
///
/// # State
///
/// One number, `decaying_mode` = `K_p sum_k du_k exp(-(t - t_k)/tau_p)`,
/// summed over every input increment `du_k` that started acting at `t_k`.
/// The output is `offset + decaying_mode`, and `offset` never moves after
/// construction because this block has zero steady-state gain.
///
/// # Valid ranges and assumptions
///
/// - `process_time` (`tau_p`) in seconds, strictly positive.
/// - `process_gain` (`K_p`) dimensionless, any sign.
/// - `delay` is a dead time in seconds; zero is the common case.
/// - Simulation time must be non-decreasing across calls (see
///   [`super::first_order_transfer_fn::FirstOrderStableTransferFnNoZeroes`]).
#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct FirstOrderStableTransferFnForZeroes {
    /// Gain `K_p` of the zero term (dimensionless).
    pub(crate) process_gain: Ratio,
    /// Time constant `tau_p` (seconds); must be strictly positive.
    pub(crate) process_time: Time,
    /// Input at the previous call (dimensionless).
    pub(crate) previous_timestep_input: Ratio,
    /// Output offset; constant after construction because this block has no
    /// steady-state gain (dimensionless).
    pub(crate) offset: Ratio,
    /// Dead time / transport delay (seconds).
    pub(crate) delay: Time,

    /// `K_p sum_k du_k exp(-(t - t_k)/tau_p)` (dimensionless). The whole
    /// dynamic response of this block.
    pub(crate) decaying_mode: Ratio,
    /// Simulation time of the previous call; `None` before the first call.
    pub(crate) last_update_time: Option<Time>,
    /// Increments accepted but not yet due; empty unless a dead time is set.
    pub(crate) pending_inputs: VecDeque<PendingStepInput>,
}

impl Default for FirstOrderStableTransferFnForZeroes {
    /// default is:
    ///
    /// s / (s + 1)
    ///
    /// with initial user input of 0.0
    /// and initial user value of 0.0
    fn default() -> Self {
        FirstOrderStableTransferFnForZeroes {
            process_gain: Ratio::new::<ratio>(1.0),
            process_time: Time::new::<second>(1.0),
            previous_timestep_input: Ratio::new::<ratio>(0.0),
            offset: Ratio::new::<ratio>(0.0),
            delay: Time::new::<second>(0.0),
            decaying_mode: Ratio::ZERO,
            last_update_time: None,
            pending_inputs: VecDeque::new(),
        }
    }
}

impl FirstOrderStableTransferFnForZeroes {
    /// Builds the zero term `K_p s / (tau_p s + 1)` with a dead time.
    ///
    /// # Parameters
    ///
    /// - `process_gain` — `K_p`, dimensionless.
    /// - `process_time` — `tau_p` in seconds; must be strictly positive.
    /// - `initial_input` — the input the block is considered to start from,
    ///   dimensionless.
    /// - `initial_value` — the output offset, dimensionless.
    /// - `delay` — dead time in seconds; `Time::ZERO` for none.
    ///
    /// # Errors
    ///
    /// Returns
    /// [`ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction`]
    /// if `process_time` is not strictly positive.
    pub fn new(
        process_gain: Ratio,
        process_time: Time,
        initial_input: Ratio,
        initial_value: Ratio,
        delay: Time,
    ) -> Result<Self, ChemEngProcessControlSimulatorError> {
        if process_time.get::<second>() <= 0.0 {
            return Err(
                ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction,
            );
        }
        Ok(FirstOrderStableTransferFnForZeroes {
            process_gain,
            process_time,
            previous_timestep_input: initial_input,
            offset: initial_value,
            delay,
            decaying_mode: Ratio::ZERO,
            last_update_time: None,
            pending_inputs: VecDeque::new(),
        })
    }

    /// Advances the block to `current_time`, applies `current_input`, and
    /// returns the output there.
    ///
    /// The transfer function is:
    ///
    /// K_p [1 - 1/(tau_p s + 1)]
    ///
    /// Input and output are dimensionless; `current_time` is an absolute
    /// simulation time in seconds and must be non-decreasing across calls.
    /// Cost is O(1) in time and memory.
    ///
    /// # Errors
    ///
    /// Infallible in practice; the `Result` is kept for API compatibility.
    pub fn set_user_input_and_calc_output(
        &mut self,
        current_time: Time,
        current_input: Ratio,
    ) -> Result<Ratio, ChemEngProcessControlSimulatorError> {
        self.advance_to(current_time);
        self.activate_due_inputs(current_time);

        // same 9-decimal-place guard as the pre-0.2.0 implementation
        let input_changed: bool = (current_input.get::<ratio>() * 1e9).round()
            - (self.previous_timestep_input.get::<ratio>() * 1e9).round()
            != 0.0;

        if input_changed {
            let input_increment = current_input - self.previous_timestep_input;
            let start_time = current_time + self.delay;

            if start_time <= current_time {
                self.activate(input_increment, current_time - start_time);
            } else {
                self.pending_inputs.push_back(PendingStepInput {
                    start_time,
                    input_increment,
                });
            }

            self.previous_timestep_input = current_input;
        }

        Ok(self.offset + self.decaying_mode)
    }

    /// Decays the accumulator from the previous call's time to `current_time`.
    /// A non-positive elapsed time is a no-op.
    fn advance_to(&mut self, current_time: Time) {
        let previous_time = match self.last_update_time {
            Some(time) => time,
            None => current_time,
        };
        self.last_update_time = Some(current_time);

        let time_step = current_time - previous_time;
        if time_step <= Time::ZERO {
            return;
        }

        let decay_factor = (-(time_step / self.process_time).get::<ratio>()).exp();
        self.decaying_mode = self.decaying_mode * decay_factor;
    }

    /// Switches an input increment on, having already been acting for
    /// `time_since_start` seconds.
    fn activate(&mut self, input_increment: Ratio, time_since_start: Time) {
        let step_magnitude = self.process_gain * input_increment;
        let decay_factor = (-(time_since_start / self.process_time).get::<ratio>()).exp();
        self.decaying_mode += step_magnitude * decay_factor;
    }

    /// Switches on every queued increment whose dead time has elapsed.
    fn activate_due_inputs(&mut self, current_time: Time) {
        while let Some(pending) = self.pending_inputs.front().copied() {
            if pending.start_time > current_time {
                break;
            }
            self.pending_inputs.pop_front();
            self.activate(pending.input_increment, current_time - pending.start_time);
        }
    }

    /// Number of state values carried, for the O(1) regression tests. One
    /// accumulator plus any queued-but-not-yet-due inputs, so exactly 1 when
    /// no dead time is set.
    #[allow(dead_code)]
    pub(crate) fn state_size(&self) -> usize {
        1 + self.pending_inputs.len()
    }

    /// Evaluates this block's response the slow, closed-form way, by
    /// superposing one [`super::step_fn::StepFunction`] and one
    /// [`super::first_order_transfer_fn::FirstOrderResponse`] per input
    /// increment.
    ///
    /// This is the **analytic reference** used by the verification tests to
    /// check the recurrence above; it is not used to drive simulations. See
    /// the module documentation for why.
    ///
    /// `increments` is a slice of `(start_time, input_increment)` pairs, in
    /// seconds and dimensionless respectively.
    #[cfg(test)]
    pub(crate) fn reference_response(
        &self,
        increments: &[(Time, Ratio)],
        simulation_time: Time,
    ) -> Result<Ratio, ChemEngProcessControlSimulatorError> {
        use super::first_order_transfer_fn::FirstOrderResponse;
        use crate::beta_testing::stable_transfer_functions::step_fn::StepFunction;

        let mut total = self.offset;
        for (start_time, input_increment) in increments.iter().copied() {
            let mut lag = FirstOrderResponse::new(
                -self.process_gain,
                self.process_time,
                start_time,
                input_increment,
                simulation_time,
            )?;
            let mut step = StepFunction::new(
                self.process_gain,
                start_time,
                input_increment,
                simulation_time,
            )?;
            total += lag.calculate_response(simulation_time);
            total += step.calculate_response(simulation_time);
        }
        Ok(total)
    }
}
