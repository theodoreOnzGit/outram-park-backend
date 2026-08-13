//! Decaying-sinusoid modes of an underdamped second-order transfer function,
//! advanced by an **exact** constant-time recurrence.
//!
//! These are the terms a numerator zero contributes to an underdamped
//! second-order block,
//!
//! ```text
//!         a1 s^2 + b s
//! G(s) = ---------------
//!         a2 s^2 + b2 s + c
//! ```
//!
//! whose response to an input increment `a_1` applied at `t_1` is
//!
//! ```text
//! y(t) = u(t - t_1) a_1 M exp(-a (t - t_1)) sin(omega (t - t_1))
//! ```
//!
//! (or the same with `cos`), for magnitude `M`, decay rate `a` and
//! oscillation frequency `omega`.
//!
//! # Why a recurrence
//!
//! Superposing one such term per past input change is a sum of
//! `exp(-a s) {cos, sin}(omega s)`, and the pair
//!
//! ```text
//! C(t) = sum_k g_k exp(-a s_k) cos(omega s_k)
//! S(t) = sum_k g_k exp(-a s_k) sin(omega s_k)
//! ```
//!
//! advances over a timestep `T` by a decaying rotation,
//!
//! ```text
//! C(t + T) = exp(-a T) [ C(t) cos(omega T) - S(t) sin(omega T) ]
//! S(t + T) = exp(-a T) [ C(t) sin(omega T) + S(t) cos(omega T) ]
//! ```
//!
//! which is the angle-addition identity, hence **exact** — not a numerical
//! approximation — for input held constant between calls. It is the
//! zero-order-hold (step-invariant) equivalent of this block, specialised to
//! a complex-conjugate pole pair. See the module documentation of
//! [`super::first_order_transfer_fn`] for the general statement.
//!
//! # References
//!
//! Textbook material; nothing reproduced from these. Editions, years and page
//! numbers are deliberately omitted because none was verified against a
//! physical copy — do not add one without checking it.
//!
//! - Astrom, K. J. and Wittenmark, B., *Computer-Controlled Systems: Theory
//!   and Design*, Prentice Hall.
//! - Seborg, D. E., Edgar, T. F., Mellichamp, D. A. and Doyle, F. J.,
//!   *Process Dynamics and Control*, Wiley.
//! - Franklin, G. F., Powell, J. D. and Workman, M. L., *Digital Control of
//!   Dynamic Systems*, Addison-Wesley.
//! - Ogata, K., *Discrete-Time Control Systems*, Prentice Hall.
//! - Oppenheim, A. V. and Schafer, R. W., *Discrete-Time Signal Processing*,
//!   Prentice Hall — the two-pole resonator view of the same recurrence.
//! - Smith, J. O. III, *Introduction to Digital Filters*, W3K Publishing —
//!   open access, from Stanford CCRMA.
//!
//! # What this module replaced
//!
//! Until version 0.2.0 the block pushed one [`DecaySinusoidResponse`] per
//! input change onto a `Vec` and re-summed it on every call. See bead
//! `op-fm5`.

use std::collections::VecDeque;

use uom::{
    si::{f64::*, frequency::hertz, ratio::ratio, time::second},
    ConstZero,
};

use super::first_order_transfer_fn::PendingStepInput;
use crate::alpha_nightly::errors::ChemEngProcessControlSimulatorError;

/// step responses for transfer function of type
///
/// G(s) = (a1 s^2 + bs)/ (a2 s^2 + b2 s + c)
///
/// # Physical quantity
///
/// Maps a dimensionless input signal to a dimensionless output signal. The
/// block contributes a decaying oscillation and nothing at steady state.
///
/// # State
///
/// Two numbers, `cosine_mode` and `sine_mode`, holding
/// `sum_k g_k exp(-a s_k) {cos, sin}(omega s_k)` over every input increment
/// that has started acting. The output projects whichever one
/// `sinusoid_type` selects. Cost is O(1) in time and memory.
///
/// # Valid ranges and assumptions
///
/// - `a` is the decay rate in hertz (reciprocal seconds) and must be strictly
///   positive, otherwise the mode does not decay.
/// - `omega` is the oscillation frequency in hertz (radians per second, as
///   used here) and must be strictly positive.
/// - `magnitude` is dimensionless, any sign.
/// - `delay` is a dead time in seconds; zero is the common case.
/// - Simulation time must be non-decreasing across calls; a call at an
///   earlier time holds the state rather than rewinding it.
#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct DecayingSinusoid {
    /// Amplitude scaling `M` applied to each input increment (dimensionless).
    pub(crate) magnitude: Ratio,
    /// decay frequency or 1/decay time (hertz); strictly positive.
    pub(crate) a: Frequency,
    /// Input at the previous call (dimensionless).
    pub(crate) previous_timestep_input: Ratio,
    /// oscillation frequency (hertz); strictly positive.
    pub(crate) omega: Frequency,
    /// Output offset; constant after construction because a decaying
    /// sinusoid contributes nothing at steady state (dimensionless).
    pub(crate) offset: Ratio,
    /// Dead time / transport delay (seconds).
    pub(crate) delay: Time,
    /// `sum_k g_k exp(-a s_k) cos(omega s_k)` (dimensionless).
    pub(crate) cosine_mode: Ratio,
    /// `sum_k g_k exp(-a s_k) sin(omega s_k)` (dimensionless).
    pub(crate) sine_mode: Ratio,
    /// Simulation time of the previous call; `None` before the first call.
    pub(crate) last_update_time: Option<Time>,
    /// Increments accepted but not yet due; empty unless a dead time is set.
    pub(crate) pending_inputs: VecDeque<PendingStepInput>,
    /// choose whether it's a sine or cosine,
    pub(crate) sinusoid_type: TransferFnSinusoidType,
}

/// Which of the two quadrature modes this block emits.
#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub enum TransferFnSinusoidType {
    /// Emits `M a_1 exp(-a s) sin(omega s)`, which starts at zero.
    Sine,
    /// Emits `M a_1 exp(-a s) cos(omega s)`, which starts at `M a_1`.
    Cosine,
}

impl Default for DecayingSinusoid {
    /// default is:
    ///
    /// 1 / ( (s+1)^2 + 1)
    /// time in seconds,
    /// frequency in hertz
    fn default() -> Self {
        DecayingSinusoid {
            magnitude: Ratio::new::<ratio>(1.0),
            a: Frequency::new::<hertz>(1.0),
            previous_timestep_input: Ratio::ZERO,
            offset: Ratio::ZERO,
            delay: Time::new::<second>(0.0),
            cosine_mode: Ratio::ZERO,
            sine_mode: Ratio::ZERO,
            last_update_time: None,
            pending_inputs: VecDeque::new(),
            omega: Frequency::new::<hertz>(1.0),
            sinusoid_type: TransferFnSinusoidType::Sine,
        }
    }
}

impl DecayingSinusoid {
    /// Builds a sine mode `M a_1 exp(-a s) sin(omega s)`.
    ///
    /// # Parameters
    ///
    /// - `magnitude` — `M`, dimensionless.
    /// - `decay_frequency` — `a` in hertz; strictly positive.
    /// - `initial_input` — starting input, dimensionless.
    /// - `initial_value` — output offset, dimensionless.
    /// - `delay` — dead time in seconds.
    /// - `omega` — oscillation frequency in hertz; strictly positive.
    ///
    /// # Errors
    ///
    /// Returns
    /// [`ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction`]
    /// if `decay_frequency` or `omega` is not strictly positive.
    pub fn new_sine(
        magnitude: Ratio,
        decay_frequency: Frequency,
        initial_input: Ratio,
        initial_value: Ratio,
        delay: Time,
        omega: Frequency,
    ) -> Result<Self, ChemEngProcessControlSimulatorError> {
        // if damping factor is less than or equal
        // 0, should throw an error
        // or panic (i will use errors maybe later?)

        if decay_frequency.value <= 0.0 {
            return Err(
                ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction,
            );
        }

        if omega.value <= 0.0 {
            return Err(
                ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction,
            );
        }

        Ok(DecayingSinusoid {
            magnitude,
            a: decay_frequency,
            previous_timestep_input: initial_input,
            offset: initial_value,
            delay,
            cosine_mode: Ratio::ZERO,
            sine_mode: Ratio::ZERO,
            last_update_time: None,
            pending_inputs: VecDeque::new(),
            omega,
            sinusoid_type: TransferFnSinusoidType::Sine,
        })
    }

    /// Builds a cosine mode `M a_1 exp(-a s) cos(omega s)`.
    ///
    /// Parameters and errors are as for [`Self::new_sine`].
    pub fn new_cosine(
        magnitude: Ratio,
        decay_frequency: Frequency,
        initial_input: Ratio,
        initial_value: Ratio,
        delay: Time,
        omega: Frequency,
    ) -> Result<Self, ChemEngProcessControlSimulatorError> {
        if decay_frequency.value <= 0.0 {
            return Err(
                ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction,
            );
        }

        if omega.value <= 0.0 {
            return Err(
                ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction,
            );
        }

        Ok(DecayingSinusoid {
            magnitude,
            a: decay_frequency,
            previous_timestep_input: initial_input,
            offset: initial_value,
            delay,
            cosine_mode: Ratio::ZERO,
            sine_mode: Ratio::ZERO,
            last_update_time: None,
            pending_inputs: VecDeque::new(),
            omega,
            sinusoid_type: TransferFnSinusoidType::Cosine,
        })
    }

    /// Advances the block to `current_time`, applies `current_input`, and
    /// returns the output there.
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

        let sinusoid_output = match self.sinusoid_type {
            TransferFnSinusoidType::Sine => self.sine_mode,
            TransferFnSinusoidType::Cosine => self.cosine_mode,
        };

        Ok(self.offset + sinusoid_output)
    }

    /// Advances the quadrature pair from the previous call's time to
    /// `current_time` by a decaying rotation. A non-positive elapsed time is
    /// a no-op.
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

        let decay = (-(time_step * self.a).get::<ratio>()).exp();
        let angle = (time_step * self.omega).get::<ratio>();
        let (sin_angle, cos_angle) = angle.sin_cos();

        let previous_cosine = self.cosine_mode;
        let previous_sine = self.sine_mode;
        self.cosine_mode = (previous_cosine * cos_angle - previous_sine * sin_angle) * decay;
        self.sine_mode = (previous_cosine * sin_angle + previous_sine * cos_angle) * decay;
    }

    /// Switches an input increment on, having already been acting for
    /// `time_since_start` seconds.
    fn activate(&mut self, input_increment: Ratio, time_since_start: Time) {
        let step_magnitude = self.magnitude * input_increment;

        let decay = (-(time_since_start * self.a).get::<ratio>()).exp();
        let angle = (time_since_start * self.omega).get::<ratio>();
        let (sin_angle, cos_angle) = angle.sin_cos();

        self.cosine_mode += step_magnitude * (decay * cos_angle);
        self.sine_mode += step_magnitude * (decay * sin_angle);
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

    /// Number of state values carried, for the O(1) regression tests. Two
    /// modes plus any queued-but-not-yet-due inputs.
    #[allow(dead_code)]
    pub(crate) fn state_size(&self) -> usize {
        2 + self.pending_inputs.len()
    }
}

/// Closed-form decaying-sinusoid response to a single input increment.
///
/// # Role
///
/// Since version 0.2.0 this is the **analytic reference** the recurrence in
/// [`DecayingSinusoid`] is verified against; it no longer drives simulations.
///
/// Units: `magnitude` and `user_input` dimensionless; `a` and `omega` in
/// hertz; `start_time` and `current_time` in seconds. `a` must be strictly
/// positive.
#[allow(dead_code)]
#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub struct DecaySinusoidResponse {
    magnitude: Ratio,
    a: Frequency,
    start_time: Time,
    user_input: Ratio,
    current_time: Time,
    omega: Frequency,
    sinusoid_type: TransferFnSinusoidType,
}

impl Default for DecaySinusoidResponse {
    /// default is a critically damped system with
    /// 1 / ( (s+1)^2 + 1)
    /// time in seconds,
    /// frequency in hertz
    fn default() -> Self {
        DecaySinusoidResponse {
            magnitude: Ratio::new::<ratio>(1.0),
            a: Frequency::new::<hertz>(1.0),
            start_time: Time::new::<second>(0.0),
            user_input: Ratio::new::<ratio>(1.0),
            current_time: Time::new::<second>(0.0),
            omega: Frequency::new::<hertz>(1.0),
            sinusoid_type: TransferFnSinusoidType::Sine,
        }
    }
}

#[allow(dead_code)]
impl DecaySinusoidResponse {
    /// Builds one decaying-sinusoid response.
    ///
    /// # Errors
    ///
    /// Returns
    /// [`ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction`]
    /// if `a` is not strictly positive.
    pub fn new(
        magnitude: Ratio,
        a: Frequency,
        start_time: Time,
        user_input: Ratio,
        current_time: Time,
        omega: Frequency,
        sinusoid_type: TransferFnSinusoidType,
    ) -> Result<Self, ChemEngProcessControlSimulatorError> {
        // if damping factor is less than or equal
        // 0, should throw an error
        // or panic (i will use errors maybe later?)

        if a.value <= 0.0 {
            return Err(
                ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction,
            );
        }
        Ok(DecaySinusoidResponse {
            magnitude,
            a,
            start_time,
            user_input,
            current_time,
            omega,
            sinusoid_type,
        })
    }

    /// checks if the transfer function has more or less reached
    /// steady state,
    ///
    /// this is determined by exp(-at)
    /// if at is 20 or more, then we have reached steady state
    ///
    /// Retained for reference; the recurrence does not truncate.
    pub fn is_steady_state(&self) -> bool {
        let time_elapsed = self.current_time - self.start_time;

        //  (at) in exp(-at)
        let at: Ratio = time_elapsed * self.a;

        if at > Ratio::new::<ratio>(20.0) {
            return true;
        }

        return false;
    }

    /// Evaluates `M a_1 exp(-a s) {sin, cos}(omega s)` at `simulation_time`,
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

        let response: Ratio;
        let at: Ratio = time_elapsed * self.a;
        let at: f64 = at.get::<ratio>();
        let omega_t: Ratio = time_elapsed * self.omega;
        let omega_t: f64 = omega_t.get::<ratio>();

        response = match self.sinusoid_type {
            TransferFnSinusoidType::Sine => {
                self.user_input * self.magnitude * (-at).exp() * (omega_t).sin()
            }
            TransferFnSinusoidType::Cosine => {
                self.user_input * self.magnitude * (-at).exp() * (omega_t).cos()
            }
        };

        return response;
    }

    /// steady state value
    /// of a decaying sinusoid is zero
    pub fn steady_state_value(&self) -> Ratio {
        let response: Ratio = Ratio::ZERO;
        response
    }
}
