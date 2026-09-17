//! Second-order stable transfer function with no zeroes, advanced by an
//! **exact** constant-time recurrence rather than by re-summing a history of
//! step responses.
//!
//! # Why a recurrence
//!
//! The block realises
//!
//! ```text
//!                     K_p
//! G(s) = ------------------------------
//!         tau^2 s^2 + 2 tau zeta s + 1
//! ```
//!
//! Driven by a signal held constant between calls, the continuous state-space
//! form `x' = A x + B u` has an *exact* discrete equivalent — the
//! zero-order-hold (ZOH) equivalent, also called the step-invariant
//! transformation:
//!
//! ```text
//! x[k+1] = Phi x[k] + Gamma u[k]
//! Phi    = exp(A T)
//! Gamma  = integral_0^T exp(A s) ds * B
//! ```
//!
//! For a second order system `A` has two eigenvalues, so `exp(A T)` acts on a
//! **two-component** state. Concretely, the transient part of the step
//! response is a pair of modes whose advance over a step `T` is a fixed
//! linear map:
//!
//! - *underdamped* (`zeta < 1`): a decaying rotation. With `a = zeta/tau` and
//!   `omega = sqrt(1 - zeta^2)/tau`, the pair
//!   `(sum g_k e^{-a s_k} cos(omega s_k), sum g_k e^{-a s_k} sin(omega s_k))`
//!   advances by `exp(-a T)` times a rotation through `omega T` — angle
//!   addition, exactly.
//! - *critically damped* (`zeta = 1`): the pair `(sum g_k e^{-s_k/tau},
//!   sum g_k (s_k/tau) e^{-s_k/tau})` advances by `exp(-T/tau)` with the
//!   second picking up `(T/tau)` times the first — the standard shear that
//!   `t exp(-t)` obeys.
//! - *overdamped* (`zeta > 1`): two independent real exponentials with rates
//!   `r_1 = (zeta - sqrt(zeta^2-1))/tau` and
//!   `r_2 = (zeta + sqrt(zeta^2-1))/tau`, each simply scaled by `exp(-r T)`.
//!
//! Each is an identity of the exponential, so the update is exact at the
//! sample instants, not a numerical approximation, whenever the input is
//! piecewise constant over each interval.
//!
//! # References
//!
//! Textbook material; cited for the derivation, nothing reproduced from them.
//! Editions, years and page numbers are deliberately omitted because none was
//! verified against a physical copy — do not add one without checking it.
//!
//! - Astrom, K. J. and Wittenmark, B., *Computer-Controlled Systems: Theory
//!   and Design*, Prentice Hall — derives `Phi` and `Gamma` above.
//! - Seborg, D. E., Edgar, T. F., Mellichamp, D. A. and Doyle, F. J.,
//!   *Process Dynamics and Control*, Wiley — the chemical-engineering text,
//!   including the three damping regimes of the second-order step response.
//! - Franklin, G. F., Powell, J. D. and Workman, M. L., *Digital Control of
//!   Dynamic Systems*, Addison-Wesley.
//! - Ogata, K., *Discrete-Time Control Systems*, Prentice Hall.
//! - Hochbruck, M. and Ostermann, A., "Exponential integrators", *Acta
//!   Numerica* — the numerical-ODE framing.
//!
//! # What this module replaced
//!
//! Until version 0.2.0 the block pushed one [`SecondOrderStableStepResponse`]
//! per input change onto a `Vec` and re-summed it on every call. See bead
//! `op-fm5`.

use std::collections::VecDeque;

use uom::{
    si::{f64::*, ratio::ratio, time::second},
    ConstZero,
};

use super::first_order_transfer_fn::PendingStepInput;
use crate::beta_testing::errors::ChemEngProcessControlSimulatorError;

/// The transient (decaying) part of a second-order step response, held as the
/// two modes appropriate to the damping regime.
///
/// One variant per regime rather than a trait object, so that adding a regime
/// is a compile error at every match site and so that go-to-definition works
/// on each variant.
///
/// In every variant the stored numbers are dimensionless (`uom` `Ratio`) and
/// are already scaled by the process gain and the input increment, i.e. they
/// are sums of `g_k = K_p du_k` weighted by the mode shape evaluated at the
/// time elapsed since increment `k` started acting.
#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub(crate) enum SecondOrderDecayingModes {
    /// `zeta < 1`: a decaying oscillation. Decay rate `a = zeta/tau`,
    /// oscillation frequency `omega = sqrt(1 - zeta^2)/tau`.
    Underdamped {
        /// `sum_k g_k exp(-a s_k) cos(omega s_k)` (dimensionless).
        cosine_mode: Ratio,
        /// `sum_k g_k exp(-a s_k) sin(omega s_k)` (dimensionless).
        sine_mode: Ratio,
    },
    /// `zeta == 1`: a repeated real root at `-1/tau`.
    CriticallyDamped {
        /// `sum_k g_k exp(-s_k/tau)` (dimensionless).
        exponential_mode: Ratio,
        /// `sum_k g_k (s_k/tau) exp(-s_k/tau)` (dimensionless).
        t_exponential_mode: Ratio,
    },
    /// `zeta > 1`: two distinct real roots, the slow one at
    /// `-r_1 = -(zeta - sqrt(zeta^2 - 1))/tau` and the fast one at
    /// `-r_2 = -(zeta + sqrt(zeta^2 - 1))/tau`.
    Overdamped {
        /// `sum_k g_k [r_2/(r_2 - r_1)] exp(-r_1 s_k)` (dimensionless).
        slow_mode: Ratio,
        /// `sum_k g_k [r_1/(r_2 - r_1)] exp(-r_2 s_k)` (dimensionless).
        fast_mode: Ratio,
    },
}

impl SecondOrderDecayingModes {
    /// A zeroed mode pair for the regime implied by `damping_factor`
    /// (dimensionless, must be strictly positive).
    fn zeroed_for(damping_factor: Ratio) -> Self {
        let zeta = damping_factor.get::<ratio>();
        if zeta < 1.0 {
            Self::Underdamped {
                cosine_mode: Ratio::ZERO,
                sine_mode: Ratio::ZERO,
            }
        } else if zeta == 1.0 {
            Self::CriticallyDamped {
                exponential_mode: Ratio::ZERO,
                t_exponential_mode: Ratio::ZERO,
            }
        } else {
            Self::Overdamped {
                slow_mode: Ratio::ZERO,
                fast_mode: Ratio::ZERO,
            }
        }
    }

    /// True if this mode pair belongs to the regime implied by
    /// `damping_factor`.
    fn matches_regime(&self, damping_factor: Ratio) -> bool {
        let zeta = damping_factor.get::<ratio>();
        match self {
            Self::Underdamped { .. } => zeta < 1.0,
            Self::CriticallyDamped { .. } => zeta == 1.0,
            Self::Overdamped { .. } => zeta > 1.0,
        }
    }

    /// The transient contribution to the output, which is subtracted from the
    /// accumulated steady state. Dimensionless.
    fn transient(&self, damping_factor: Ratio) -> Ratio {
        match self {
            Self::Underdamped {
                cosine_mode,
                sine_mode,
            } => {
                let zeta = damping_factor;
                let sqrt_one_minus_zeta_sq = (Ratio::new::<ratio>(1.0) - zeta * zeta).sqrt();
                *cosine_mode + *sine_mode * (zeta / sqrt_one_minus_zeta_sq).get::<ratio>()
            }
            Self::CriticallyDamped {
                exponential_mode,
                t_exponential_mode,
            } => *exponential_mode + *t_exponential_mode,
            Self::Overdamped {
                slow_mode,
                fast_mode,
            } => *slow_mode - *fast_mode,
        }
    }

    /// Advances the modes over `time_step` seconds — the `Phi = exp(A T)`
    /// half of the zero-order-hold equivalent.
    ///
    /// `process_time` is `tau` in seconds and `damping_factor` is `zeta`,
    /// dimensionless and strictly positive.
    fn advance(&mut self, time_step: Time, process_time: Time, damping_factor: Ratio) {
        match self {
            Self::Underdamped {
                cosine_mode,
                sine_mode,
            } => {
                let zeta = damping_factor.get::<ratio>();
                let time_ratio = (time_step / process_time).get::<ratio>();
                let decay = (-zeta * time_ratio).exp();
                let angle = (1.0 - zeta * zeta).sqrt() * time_ratio;
                let (sin_angle, cos_angle) = angle.sin_cos();

                // rotation by `angle`, shrunk by `decay` -- angle addition on
                // exp(-a s) {cos, sin}(omega s)
                let previous_cosine = *cosine_mode;
                let previous_sine = *sine_mode;
                *cosine_mode = (previous_cosine * cos_angle - previous_sine * sin_angle) * decay;
                *sine_mode = (previous_cosine * sin_angle + previous_sine * cos_angle) * decay;
            }
            Self::CriticallyDamped {
                exponential_mode,
                t_exponential_mode,
            } => {
                let time_ratio = (time_step / process_time).get::<ratio>();
                let decay = (-time_ratio).exp();

                // (s + T) exp(-(s+T)/tau) = exp(-T/tau) [ s exp(-s/tau)
                //                                       + T exp(-s/tau) ]
                *t_exponential_mode =
                    (*t_exponential_mode + *exponential_mode * time_ratio) * decay;
                *exponential_mode = *exponential_mode * decay;
            }
            Self::Overdamped {
                slow_mode,
                fast_mode,
            } => {
                let zeta = damping_factor.get::<ratio>();
                let root = (zeta * zeta - 1.0).sqrt();
                let time_ratio = (time_step / process_time).get::<ratio>();

                *slow_mode = *slow_mode * (-(zeta - root) * time_ratio).exp();
                *fast_mode = *fast_mode * (-(zeta + root) * time_ratio).exp();
            }
        }
    }

    /// Injects a step of magnitude `step_magnitude` (dimensionless, already
    /// `K_p du`) that started acting `time_since_start` seconds ago.
    ///
    /// At `time_since_start == 0` this adds exactly `step_magnitude` to the
    /// transient, so the output does not jump — a second-order lag responds
    /// with zero slope as well as zero value.
    fn inject(
        &mut self,
        step_magnitude: Ratio,
        time_since_start: Time,
        process_time: Time,
        damping_factor: Ratio,
    ) {
        let time_ratio = (time_since_start / process_time).get::<ratio>();
        match self {
            Self::Underdamped {
                cosine_mode,
                sine_mode,
            } => {
                let zeta = damping_factor.get::<ratio>();
                let decay = (-zeta * time_ratio).exp();
                let angle = (1.0 - zeta * zeta).sqrt() * time_ratio;
                let (sin_angle, cos_angle) = angle.sin_cos();
                *cosine_mode += step_magnitude * (decay * cos_angle);
                *sine_mode += step_magnitude * (decay * sin_angle);
            }
            Self::CriticallyDamped {
                exponential_mode,
                t_exponential_mode,
            } => {
                let decay = (-time_ratio).exp();
                *exponential_mode += step_magnitude * decay;
                *t_exponential_mode += step_magnitude * (time_ratio * decay);
            }
            Self::Overdamped {
                slow_mode,
                fast_mode,
            } => {
                let zeta = damping_factor.get::<ratio>();
                let root = (zeta * zeta - 1.0).sqrt();
                // r_1 tau = zeta - root, r_2 tau = zeta + root,
                // (r_2 - r_1) tau = 2 root
                let slow_weight = (zeta + root) / (2.0 * root);
                let fast_weight = (zeta - root) / (2.0 * root);
                *slow_mode += step_magnitude * (slow_weight * (-(zeta - root) * time_ratio).exp());
                *fast_mode += step_magnitude * (fast_weight * (-(zeta + root) * time_ratio).exp());
            }
        }
    }
}

/// second order system with transfer function
/// in the form
///
/// K_p / ( tau^2 s^2 + 2 * tau * zeta s + 1)
///
/// tau is process time
/// zeta is damping factor
/// K_p is process gain (dimensionless, be careful)
///
/// no zeroes are expected here in this transfer fn
///
/// # Physical quantity
///
/// Maps a dimensionless input signal to a dimensionless output signal.
///
/// # State
///
/// Constant: an accumulated steady state (`offset`) plus a two-component
/// [`SecondOrderDecayingModes`] carrying the transient. Output is
/// `offset - transient`. Evaluating a step is O(1) in time and memory
/// regardless of run length or how often the input changes.
///
/// # Valid ranges and assumptions
///
/// - `process_time` (`tau`) in seconds, and in practice strictly positive.
/// - `damping_factor` (`zeta`) dimensionless and strictly positive; the
///   constructor rejects `zeta <= 0` as non-stable. `zeta` must **not** be
///   mutated after construction — if it is, the mode pair is silently reset
///   to zero on the next call, which discards the transient.
/// - `process_gain` (`K_p`) dimensionless, any sign.
/// - `delay` is a dead time in seconds; zero is the common case.
/// - Simulation time must be non-decreasing across calls; a call at an
///   earlier time holds the state rather than rewinding it, and repeated
///   calls at the same time are idempotent.
#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct SecondOrderStableTransferFnNoZeroes {
    /// Steady-state gain `K_p` (dimensionless).
    pub(crate) process_gain: Ratio,
    /// Natural time constant `tau` (seconds).
    pub(crate) process_time: Time,
    /// Input at the previous call (dimensionless).
    pub(crate) previous_timestep_input: Ratio,
    /// Damping factor `zeta` (dimensionless, strictly positive).
    pub(crate) damping_factor: Ratio,
    /// Accumulated steady-state output (dimensionless).
    pub(crate) offset: Ratio,
    /// Dead time / transport delay (seconds).
    pub(crate) delay: Time,

    /// The transient part of the response.
    pub(crate) decaying_modes: SecondOrderDecayingModes,
    /// Simulation time of the previous call; `None` before the first call.
    pub(crate) last_update_time: Option<Time>,
    /// Increments accepted but not yet due; empty unless a dead time is set.
    pub(crate) pending_inputs: VecDeque<PendingStepInput>,
}

impl Default for SecondOrderStableTransferFnNoZeroes {
    /// default is:
    ///
    /// 1 / (s^2 + 2s + 1)
    /// where process time is 1 second
    /// the damping factor is 1.0 which makes it a critically
    /// damped system
    ///
    /// with initial user input of 0.0
    /// and initial user value of 0.0
    fn default() -> Self {
        let damping_factor = Ratio::new::<ratio>(1.0);
        SecondOrderStableTransferFnNoZeroes {
            process_gain: Ratio::new::<ratio>(1.0),
            process_time: Time::new::<second>(1.0),
            previous_timestep_input: Ratio::ZERO,
            offset: Ratio::ZERO,
            delay: Time::new::<second>(0.0),
            decaying_modes: SecondOrderDecayingModes::zeroed_for(damping_factor),
            last_update_time: None,
            pending_inputs: VecDeque::new(),
            damping_factor,
        }
    }
}

impl SecondOrderStableTransferFnNoZeroes {
    /// Builds `K_p / (tau^2 s^2 + 2 tau zeta s + 1)` with a dead time.
    ///
    /// # Parameters
    ///
    /// - `process_gain` — `K_p`, dimensionless.
    /// - `process_time` — `tau` in seconds.
    /// - `damping_factor` — `zeta`, dimensionless, strictly positive. Below
    ///   1 gives an oscillatory response, exactly 1 critical damping, above 1
    ///   two real modes.
    /// - `initial_input` — starting input, dimensionless.
    /// - `initial_value` — starting output, dimensionless.
    /// - `delay` — dead time in seconds; `Time::ZERO` for none.
    ///
    /// # Errors
    ///
    /// Returns
    /// [`ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction`]
    /// if `damping_factor` is not strictly positive, since a non-positive
    /// damping factor is an unstable or undamped system.
    pub fn new(
        process_gain: Ratio,
        process_time: Time,
        damping_factor: Ratio,
        initial_input: Ratio,
        initial_value: Ratio,
        delay: Time,
    ) -> Result<Self, ChemEngProcessControlSimulatorError> {
        // if damping factor is less than or equal 0, it is unstable

        if damping_factor.value <= 0.0 {
            return Err(
                ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction,
            );
        }

        Ok(SecondOrderStableTransferFnNoZeroes {
            process_gain,
            process_time,
            previous_timestep_input: initial_input,
            offset: initial_value,
            delay,
            decaying_modes: SecondOrderDecayingModes::zeroed_for(damping_factor),
            last_update_time: None,
            pending_inputs: VecDeque::new(),
            damping_factor,
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
        // guard against a damping factor mutated behind our back
        if !self.decaying_modes.matches_regime(self.damping_factor) {
            self.decaying_modes = SecondOrderDecayingModes::zeroed_for(self.damping_factor);
        }

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

        Ok(self.offset - self.decaying_modes.transient(self.damping_factor))
    }

    /// Decays the modes from the previous call's time to `current_time`.
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

        self.decaying_modes
            .advance(time_step, self.process_time, self.damping_factor);
    }

    /// Switches an input increment on, having already been acting for
    /// `time_since_start` seconds.
    fn activate(&mut self, input_increment: Ratio, time_since_start: Time) {
        let step_magnitude = self.process_gain * input_increment;
        self.offset += step_magnitude;
        self.decaying_modes.inject(
            step_magnitude,
            time_since_start,
            self.process_time,
            self.damping_factor,
        );
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

    /// Number of state values carried, for the O(1) regression tests. Three
    /// (accumulated steady state plus two modes) plus any
    /// queued-but-not-yet-due inputs.
    #[allow(dead_code)]
    pub(crate) fn state_size(&self) -> usize {
        3 + self.pending_inputs.len()
    }
}

/// Closed-form step response of a stable second-order system, in all three
/// damping regimes.
///
/// # Role
///
/// Since version 0.2.0 this is the **analytic reference** the recurrence in
/// [`SecondOrderStableTransferFnNoZeroes`] is verified against; it is no
/// longer used to drive simulations, because superposing one of these per
/// input change is what made the old cost grow without bound. It is retained
/// because a closed form evaluable at an arbitrary time is what a
/// verification test needs.
///
/// Units: `process_gain`, `user_input` and `damping_factor` dimensionless;
/// `process_time`, `start_time` and `current_time` in seconds.
/// `damping_factor` must be strictly positive.
#[allow(dead_code)]
#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub struct SecondOrderStableStepResponse {
    process_gain: Ratio,
    process_time: Time,
    start_time: Time,
    user_input: Ratio,
    current_time: Time,
    damping_factor: Ratio,
}

impl Default for SecondOrderStableStepResponse {
    /// default is a critically damped system with
    /// process time 1s,
    /// process gain 1.0 (dimensionless)
    fn default() -> Self {
        SecondOrderStableStepResponse {
            process_gain: Ratio::new::<ratio>(1.0),
            process_time: Time::new::<second>(1.0),
            start_time: Time::new::<second>(0.0),
            user_input: Ratio::new::<ratio>(1.0),
            current_time: Time::new::<second>(0.0),
            damping_factor: Ratio::new::<ratio>(1.0),
        }
    }
}

#[allow(dead_code)]
impl SecondOrderStableStepResponse {
    /// Builds one second-order step response.
    ///
    /// # Errors
    ///
    /// Returns
    /// [`ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction`]
    /// if `damping_factor` is not strictly positive.
    pub fn new(
        process_gain: Ratio,
        process_time: Time,
        damping_factor: Ratio,
        start_time: Time,
        user_input: Ratio,
        current_time: Time,
    ) -> Result<Self, ChemEngProcessControlSimulatorError> {
        // if damping factor is less than or equal 0,
        // return an error

        if damping_factor.value <= 0.0 {
            return Err(
                ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction,
            );
        }
        Ok(SecondOrderStableStepResponse {
            process_gain,
            process_time,
            start_time,
            user_input,
            current_time,
            damping_factor,
        })
    }

    /// checks if the transfer function has more or less reached
    /// steady state,
    ///
    /// I consider this where the time elapsed is 23 times
    /// the process_time
    ///
    /// this is because 23 * exp(-23) is about 2e-9, it is tiny...
    /// this is because we need to consider the exponential of
    /// x exp(-x) for critically damped systems
    ///
    /// Retained for reference; the recurrence does not truncate, so it
    /// carries this residual rather than discarding it.
    pub fn is_steady_state(&self) -> bool {
        let time_elapsed = self.current_time - self.start_time;

        let time_ratio: f64 = time_elapsed.value / self.process_time.value;

        let damping_factor = self.damping_factor;
        // no unstable or undamped responses allowed
        if damping_factor.value <= 0.0 {
            todo!(
                "damping factor needs to be more than 0.0, \n
                also need to implement Result enum"
            )
        }

        if damping_factor.get::<ratio>() < 1.0 {
            // case 1: underdamped systems
            // (zeta * t/tau_p) > 20.0

            let underdamped_time_ratio = damping_factor * time_ratio;

            if underdamped_time_ratio.get::<ratio>() > 20.0 {
                return true;
            }
        } else if damping_factor.get::<ratio>() == 1.0 {
            // case 2: critically damped system
            // probably need to redo this bit
            if time_ratio > 23.0 {
                return true;
            }
        } else {
            // case 3: overdamped system
            let sqrt_zeta_sq_minus_one: Ratio =
                (damping_factor * damping_factor - Ratio::new::<ratio>(1.0)).sqrt();
            let zeta = damping_factor;

            let overdamped_time_ratio_one = (zeta - sqrt_zeta_sq_minus_one) * time_ratio;

            let overdamped_time_ratio_two = (zeta + sqrt_zeta_sq_minus_one) * time_ratio;

            let overdamped_mode_one_steady_state: bool =
                overdamped_time_ratio_one.get::<ratio>().abs() > 20.0;
            let overdamped_mode_two_steady_state: bool =
                overdamped_time_ratio_two.get::<ratio>().abs() > 20.0;

            if overdamped_mode_two_steady_state && overdamped_mode_one_steady_state {
                return true;
            }
        }

        return false;
    }

    /// Evaluates the second-order step response at `simulation_time`,
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

        // time ratio is t/tau
        let time_ratio: Ratio = time_elapsed / self.process_time;
        let steady_state_value: Ratio = self.steady_state_value();

        // need to calculate second order response
        // which means we need the damping factor or something
        let damping_factor = self.damping_factor;

        // no unstable or undamped responses allowed
        if damping_factor.value <= 0.0 {
            todo!(
                "damping factor needs to be more than 0.0, \n
                also need to implement Result enum"
            )
        }

        let response: Ratio;

        if damping_factor.get::<ratio>() < 1.0 {
            // case 1: underdamped

            let sqrt_one_minus_zeta_sq: Ratio =
                (Ratio::new::<ratio>(1.0) - damping_factor * damping_factor).sqrt();
            // first, cos term
            // cos ( sqrt(1-zeta^2)/tau * t)

            let omega_t_term: Ratio =
                sqrt_one_minus_zeta_sq * time_ratio.get::<uom::si::ratio::ratio>();

            let cosine_term: Ratio = Ratio::new::<ratio>(omega_t_term.get::<ratio>().cos());

            // next, sine term,
            // zeta / (1 - zeta^2) * sin ( sqrt(1 - zeta^2)/ tau * t)

            let sine_term =
                damping_factor / sqrt_one_minus_zeta_sq * omega_t_term.get::<ratio>().sin();

            // now we need 1 - exp(- zeta * t/tau) *
            // [ cos term + sine term ]

            let cosine_and_sine_term: Ratio = cosine_term + sine_term;

            // exp(- zeta * t/tau) * [ cos term + sine term ]
            let exponential_term: Ratio =
                (-damping_factor * time_ratio.get::<uom::si::ratio::ratio>()).exp()
                    * cosine_and_sine_term;

            let scaled_response = Ratio::new::<ratio>(1.0) - exponential_term;

            // a_0 * K_p *exp(- zeta * t/tau) * [ cos term + sine term ]
            response = steady_state_value * scaled_response;
        } else if damping_factor.get::<ratio>() == 1.0 {
            // case 2: critical damping
            //
            // a_0 K_p
            // {
            // 1 - [1 + t/tau] exp (- t/tau)
            // }

            let one_plus_t_over_tau = 1.0 + time_ratio.get::<uom::si::ratio::ratio>();

            let exponential_term =
                (-time_ratio.get::<uom::si::ratio::ratio>()).exp() * one_plus_t_over_tau;

            let scaled_response = 1.0 - exponential_term;

            response = steady_state_value * scaled_response;
        } else {
            // case 3: overdamped

            let sqrt_zeta_sq_minus_one: Ratio =
                (damping_factor * damping_factor - Ratio::new::<ratio>(1.0)).sqrt();

            // first, cosh term
            // cosh ( sqrt(zeta^2-1)/tau * t)

            let omega_t_term: Ratio =
                sqrt_zeta_sq_minus_one * time_ratio.get::<uom::si::ratio::ratio>();

            let cosh_term = Ratio::new::<ratio>(omega_t_term.get::<ratio>().cosh());

            // next, sinh term,
            // zeta / (1 - zeta^2) * sinh ( sqrt(zeta^2 - 1)/ tau * t)

            let sinh_term =
                damping_factor / sqrt_zeta_sq_minus_one * omega_t_term.get::<ratio>().sinh();

            // now we need 1 - exp(- zeta * t/tau) *
            // [ cosh term + sinh term ]

            let cosh_term_plus_sinh_term = cosh_term + sinh_term;

            // exp(- zeta * t/tau) * [ cos term + sine term ]
            let exponential_term: Ratio =
                (-damping_factor * time_ratio.get::<uom::si::ratio::ratio>()).exp()
                    * cosh_term_plus_sinh_term;

            let scaled_response = Ratio::new::<ratio>(1.0) - exponential_term;

            // a_0 * K_p *exp(- zeta * t/tau) * [ cos term + sine term ]
            response = steady_state_value * scaled_response;
        }

        return response;
    }

    /// The value this step response tends to, `a_1 K_p`. Dimensionless.
    pub fn steady_state_value(&self) -> Ratio {
        let response: Ratio = self.user_input * self.process_gain;
        response
    }
}
