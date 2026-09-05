//! First-order stable transfer function with no zeroes, advanced by an
//! **exact** constant-time recurrence rather than by re-summing a history of
//! step responses.
//!
//! # Why a recurrence
//!
//! The block realises
//!
//! ```text
//!            K_p
//! G(s) = -----------
//!         tau_p s + 1
//! ```
//!
//! Driven by a signal that is held constant between calls (which is what a
//! time-stepping simulator produces), the continuous state-space form
//! `x' = A x + B u` has an *exact* discrete equivalent — not an approximation:
//!
//! ```text
//! x[k+1] = Phi x[k] + Gamma u[k]
//! Phi    = exp(A T)
//! Gamma  = integral_0^T exp(A s) ds * B
//! ```
//!
//! For a first-order lag (`A = -1/tau_p`) this collapses to the familiar
//! one-pole update
//!
//! ```text
//! y[n] = exp(-T/tau_p) y[n-1] + (1 - exp(-T/tau_p)) K_p u[n]
//! ```
//!
//! and it reproduces the continuous solution exactly at the sample instants
//! whenever the input is piecewise constant over each interval `T`. This
//! result is standard and is known as the **zero-order-hold (ZOH) equivalent**
//! or the **step-invariant transformation**; the searchable term for the
//! general case is *exact discretization*.
//!
//! # References
//!
//! The mathematics below is textbook material, not a novel derivation. Cited
//! for the reader who wants the derivation in full; nothing here is
//! reproduced from these texts.
//!
//! - Astrom, K. J. and Wittenmark, B., *Computer-Controlled Systems: Theory
//!   and Design*, Prentice Hall — the sampling-of-continuous-systems chapter
//!   derives `Phi` and `Gamma` above. The canonical citation for this crate's
//!   purpose.
//! - Seborg, D. E., Edgar, T. F., Mellichamp, D. A. and Doyle, F. J.,
//!   *Process Dynamics and Control*, Wiley — the chemical-engineering-native
//!   treatment.
//! - Franklin, G. F., Powell, J. D. and Workman, M. L., *Digital Control of
//!   Dynamic Systems*, Addison-Wesley.
//! - Ogata, K., *Discrete-Time Control Systems*, Prentice Hall.
//! - Hochbruck, M. and Ostermann, A., "Exponential integrators", *Acta
//!   Numerica* — the numerical-ODE framing (exact for piecewise-constant
//!   forcing), which is the framing used by this workspace's
//!   `teh-o-prke` decay-heat and delayed-neutron-precursor integrators.
//! - Oppenheim, A. V. and Schafer, R. W., *Discrete-Time Signal Processing*,
//!   Prentice Hall — the one-pole IIR filter view.
//! - Smith, J. O. III, *Introduction to Digital Filters*, W3K Publishing —
//!   the open-access option, freely available from Stanford CCRMA.
//!
//! No edition, year or page number is given for any of the above because
//! none was verified against a physical copy while writing this module.
//! Do not add one without checking it.
//!
//! # What this module replaced
//!
//! Until version 0.2.0 this block stored one [`FirstOrderResponse`] per input
//! change in a `Vec` and re-summed the whole vector on every call. Entries
//! were retired once `t/tau_p > 20`, so the vector saturated at
//! `N = 20 tau_p / dt` — but only if the run lasted that long. A first-order
//! lag with `tau_p = 80.8 s` stepped at 1 ms never gets there, and the vector
//! grew to over a million entries. Measured cost of the old scheme on a
//! filtered PID at `dt = 0.001 s` was 1176 us/step; see the crate-level
//! bookkeeping for bead `op-fm5`.
//!
//! A first-order lag carries **one** state variable, not a history of every
//! input it has ever seen.

use std::collections::VecDeque;

use uom::{
    si::{f64::*, ratio::ratio, time::second},
    ConstZero,
};

use crate::beta_testing::errors::ChemEngProcessControlSimulatorError;

/// An input increment that has been accepted but whose dead time has not yet
/// elapsed, so it has not started to act on the block.
///
/// Only ever non-empty when the block has a non-zero dead time (transport
/// delay). With no dead time the queue stays empty and the block's memory is
/// genuinely constant.
///
/// # Fields
///
/// - `start_time` — the simulation time at which this increment begins to
///   drive the block, i.e. the time the input arrived plus the dead time.
///   Units: seconds (SI, `uom` `Time`).
/// - `input_increment` — the *change* in the dimensionless input signal,
///   `u(t) - u(t_previous)`. Units: dimensionless (`uom` `Ratio`).
#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub struct PendingStepInput {
    /// Simulation time at which the increment starts to act (seconds).
    pub start_time: Time,
    /// Change in the dimensionless input signal (dimensionless).
    pub input_increment: Ratio,
}

/// A stable first-order lag with no zeroes:
///
/// ```text
///            K_p
/// G(s) = -----------  exp(-c s)
///         tau_p s + 1
/// ```
///
/// where `c` is an optional dead time (transport delay).
///
/// # Physical quantity
///
/// Maps a dimensionless input signal `u(t)` to a dimensionless output signal
/// `y(t)`. Both are `uom` `Ratio`, i.e. dimensionless ratios — the block is
/// deliberately unit-agnostic so it can sit between any two scaled signals in
/// a control loop.
///
/// # State
///
/// The block keeps two numbers and (only when a dead time is set) a short
/// queue:
///
/// - `offset` — the output the block would settle to if the input were frozen
///   now and left forever. Every input increment that has begun to act has
///   already been added here, scaled by `K_p`.
/// - `decaying_mode` — the part of `offset` that has not yet been "earned",
///   i.e. `K_p * sum_k du_k exp(-(t - t_k)/tau_p)` over every increment `du_k`
///   that started acting at `t_k`. It decays geometrically and the output is
///   `offset - decaying_mode`.
///
/// So the output is exact at every sample instant, and evaluating it is
/// constant-time and constant-memory regardless of how long the simulation
/// runs or how often the input changes.
///
/// # Valid ranges and assumptions
///
/// - `process_time` (`tau_p`) must be strictly positive, in seconds. A
///   non-positive value is a non-stable (or algebraic) system and the
///   constructors reject it with
///   [`ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction`].
/// - `process_gain` (`K_p`) is dimensionless and unrestricted in sign.
/// - `delay` is a dead time in seconds. Zero (the default) is the common
///   case; a positive value buffers increments until they are due. A negative
///   value is accepted and treated as an increment that has already been
///   acting for `-delay` seconds, matching the pre-0.2.0 behaviour.
/// - **Simulation time passed to
///   [`Self::set_user_input_and_calc_output`] must be non-decreasing.** The
///   recurrence advances state forward only; a call with a time earlier than
///   the previous call holds the state rather than rewinding it. Calling
///   twice at the *same* time is fine and is idempotent.
#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct FirstOrderStableTransferFnNoZeroes {
    /// Steady-state gain `K_p` (dimensionless).
    pub(crate) process_gain: Ratio,
    /// Time constant `tau_p` (seconds); must be strictly positive.
    pub(crate) process_time: Time,
    /// Input at the previous call, used to form the increment (dimensionless).
    pub(crate) previous_timestep_input: Ratio,
    /// Accumulated steady-state output: what the output tends to as
    /// `t -> infinity` with the input frozen (dimensionless).
    pub(crate) offset: Ratio,
    /// Dead time / transport delay (seconds).
    pub(crate) delay: Time,

    /// The decaying part of the response, `K_p sum_k du_k exp(-(t-t_k)/tau_p)`
    /// (dimensionless). Output is `offset - decaying_mode`.
    pub(crate) decaying_mode: Ratio,
    /// Simulation time of the previous call; `None` before the first call.
    pub(crate) last_update_time: Option<Time>,
    /// Increments accepted but not yet due, ordered by `start_time`. Empty
    /// unless a dead time is set.
    pub(crate) pending_inputs: VecDeque<PendingStepInput>,
}

impl Default for FirstOrderStableTransferFnNoZeroes {
    /// default is:
    ///
    /// 1 / (s + 1)
    ///
    /// with initial user input of 0.0
    /// and initial user value of 0.0
    fn default() -> Self {
        FirstOrderStableTransferFnNoZeroes {
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

impl FirstOrderStableTransferFnNoZeroes {
    /// Builds a first-order lag `K_p / (tau_p s + 1)` with a dead time.
    ///
    /// # Parameters
    ///
    /// - `process_gain` — `K_p`, dimensionless.
    /// - `process_time` — `tau_p` in seconds; must be strictly positive.
    /// - `initial_input` — the input value the block is considered to have
    ///   been sitting at, dimensionless. Increments are measured from here.
    /// - `initial_value` — the output value the block starts at,
    ///   dimensionless.
    /// - `delay` — dead time in seconds; use `Time::ZERO` for none.
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
        Ok(FirstOrderStableTransferFnNoZeroes {
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

    /// Builds a unity-gain first-order filter `1 / (tau_p s + 1)` with no
    /// dead time.
    ///
    /// # Parameters
    ///
    /// - `process_time` — filter time constant `tau_p` in seconds; must be
    ///   strictly positive.
    /// - `initial_input` — starting input, dimensionless.
    /// - `initial_value` — starting output, dimensionless.
    ///
    /// # Errors
    ///
    /// Returns
    /// [`ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction`]
    /// if `process_time` is not strictly positive.
    pub fn new_filter(
        process_time: Time,
        initial_input: Ratio,
        initial_value: Ratio,
    ) -> Result<Self, ChemEngProcessControlSimulatorError> {
        if process_time.get::<second>() <= 0.0 {
            return Err(
                ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction,
            );
        }
        Ok(FirstOrderStableTransferFnNoZeroes {
            process_gain: Ratio::new::<ratio>(1.0),
            process_time,
            previous_timestep_input: initial_input,
            offset: initial_value,
            delay: Time::new::<second>(0.0),
            decaying_mode: Ratio::ZERO,
            last_update_time: None,
            pending_inputs: VecDeque::new(),
        })
    }

    /// Advances the block to `current_time`, applies `current_input`, and
    /// returns the output at that instant.
    ///
    /// Both the input and the returned output are dimensionless (`Ratio`);
    /// `current_time` is an absolute simulation time in seconds and must be
    /// non-decreasing across calls.
    ///
    /// Cost is O(1) in time and memory: one exponential and a handful of
    /// multiplies, independent of the number of steps taken so far.
    ///
    /// # Errors
    ///
    /// Infallible in practice; the `Result` is kept for API compatibility and
    /// so that future non-stable variants can report a failure.
    pub fn set_user_input_and_calc_output(
        &mut self,
        current_time: Time,
        current_input: Ratio,
    ) -> Result<Ratio, ChemEngProcessControlSimulatorError> {
        // 1. decay whatever is already in flight up to now
        self.advance_to(current_time);

        // 2. switch on any increment whose dead time has now elapsed
        self.activate_due_inputs(current_time);

        // 3. accept a new increment if the input moved.
        //
        // The 9-decimal-place comparison is kept from the pre-0.2.0
        // implementation so that the block ignores exactly the same
        // sub-nano changes it used to.
        let input_changed: bool = (current_input.get::<ratio>() * 1e9).round()
            - (self.previous_timestep_input.get::<ratio>() * 1e9).round()
            != 0.0;

        if input_changed {
            let input_increment = current_input - self.previous_timestep_input;
            let start_time = current_time + self.delay;

            if start_time <= current_time {
                // no dead time (or a negative one): acts immediately
                self.activate(input_increment, current_time - start_time);
            } else {
                self.pending_inputs.push_back(PendingStepInput {
                    start_time,
                    input_increment,
                });
            }

            self.previous_timestep_input = current_input;
        }

        // 4. y(t) = (steady state it is heading for) - (what has not decayed yet)
        Ok(self.offset - self.decaying_mode)
    }

    /// Decays the in-flight mode from the previous call's time to
    /// `current_time`.
    ///
    /// This is the `Phi = exp(A T)` half of the zero-order-hold equivalent.
    /// A non-positive elapsed time is a no-op, which makes repeated calls at
    /// the same simulation time idempotent.
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
    ///
    /// `time_since_start` is zero in the no-dead-time case; it is the sliver
    /// of a timestep by which a delayed increment became due early when a
    /// dead time is set.
    fn activate(&mut self, input_increment: Ratio, time_since_start: Time) {
        let step_magnitude = self.process_gain * input_increment;

        // where the output is now headed
        self.offset += step_magnitude;

        // how much of that has yet to be realised
        let decay_factor = (-(time_since_start / self.process_time).get::<ratio>()).exp();
        self.decaying_mode += step_magnitude * decay_factor;
    }

    /// Switches on every queued increment whose dead time has elapsed by
    /// `current_time`. The queue is in `start_time` order, so this stops at
    /// the first entry that is not yet due.
    fn activate_due_inputs(&mut self, current_time: Time) {
        while let Some(pending) = self.pending_inputs.front().copied() {
            if pending.start_time > current_time {
                break;
            }
            self.pending_inputs.pop_front();
            self.activate(pending.input_increment, current_time - pending.start_time);
        }
    }

    /// Number of state values the block is carrying, for regression tests
    /// that assert the cost of a step does not grow with the step index.
    ///
    /// This is 2 (the accumulated steady state and the decaying mode) plus
    /// one per queued-but-not-yet-due input, so it is exactly 2 whenever no
    /// dead time is set.
    #[allow(dead_code)]
    pub(crate) fn state_size(&self) -> usize {
        2 + self.pending_inputs.len()
    }
}

/// Closed-form step response of a first-order lag:
///
/// ```text
/// y(t) = u(t - t_1) K_p a_1 [1 - exp(-(t - t_1)/tau_p)]
/// ```
///
/// where `u(.)` is the Heaviside step, `t_1` the start time, `a_1` the input
/// increment and `K_p`, `tau_p` the gain and time constant.
///
/// # Role
///
/// This is the **analytic reference** the recurrence in
/// [`FirstOrderStableTransferFnNoZeroes`] is verified against; it is no
/// longer used to drive a simulation, because superposing one of these per
/// input change is what made the old implementation cost grow without bound.
/// It is retained because a closed form that can be evaluated at an arbitrary
/// time is exactly what a verification test needs.
///
/// Units: `process_gain` and `user_input` dimensionless; `process_time`,
/// `start_time` and `current_time` in seconds. `process_time` must be
/// strictly positive.
#[allow(dead_code)]
#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub struct FirstOrderResponse {
    process_gain: Ratio,
    process_time: Time,
    start_time: Time,
    user_input: Ratio,
    current_time: Time,
}

impl Default for FirstOrderResponse {
    fn default() -> Self {
        FirstOrderResponse {
            process_gain: Ratio::new::<ratio>(1.0),
            process_time: Time::new::<second>(1.0),
            start_time: Time::new::<second>(0.0),
            user_input: Ratio::new::<ratio>(1.0),
            current_time: Time::new::<second>(0.0),
        }
    }
}

#[allow(dead_code)]
impl FirstOrderResponse {
    /// Builds one step response.
    ///
    /// # Errors
    ///
    /// Returns
    /// [`ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction`]
    /// if `process_time` is not strictly positive.
    pub fn new(
        process_gain: Ratio,
        process_time: Time,
        start_time: Time,
        user_input: Ratio,
        current_time: Time,
    ) -> Result<Self, ChemEngProcessControlSimulatorError> {
        if process_time.get::<second>() <= 0.0 {
            return Err(
                ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction,
            );
        }
        Ok(FirstOrderResponse {
            process_gain,
            process_time,
            start_time,
            user_input,
            current_time,
        })
    }

    /// True once the elapsed time exceeds 20 time constants, at which point
    /// `exp(-20) = 2.06e-9` of the step is still outstanding.
    ///
    /// Retained for reference. The recurrence does **not** truncate, so it
    /// carries that residual instead of discarding it.
    pub fn is_steady_state(&self) -> bool {
        let time_elapsed = self.current_time - self.start_time;

        let time_ratio: f64 = time_elapsed.value / self.process_time.value;

        if time_ratio > 20.0 {
            return true;
        }

        return false;
    }

    /// Evaluates `a_1 K_p [1 - exp(-(t - t_1)/tau_p)]` at `simulation_time`,
    /// returning zero before the start time. Dimensionless.
    pub fn calculate_response(&mut self, simulation_time: Time) -> Ratio {
        // get the current time (t - t0)
        self.current_time = simulation_time;
        let time_elapsed = self.current_time - self.start_time;

        // first let's deal with the heaviside function

        let heaviside_on: bool = self.current_time >= self.start_time;

        // if the current time is before start time, no response
        // from this transfer function
        if !heaviside_on {
            return Ratio::ZERO;
        }

        let time_ratio: Ratio = time_elapsed / self.process_time;
        let exponent_ratio: f64 = -time_ratio.value;

        // otherwise, calculate as per normal

        // u1(t - t1) * Kp * [1-exp(- [t-t1] / tau])
        let response: Ratio = self.steady_state_value() * (1.0 - exponent_ratio.exp());

        return response;
    }

    /// The value this step response tends to, `a_1 K_p`. Dimensionless.
    pub fn steady_state_value(&self) -> Ratio {
        let response: Ratio = self.user_input * self.process_gain;
        response
    }
}
