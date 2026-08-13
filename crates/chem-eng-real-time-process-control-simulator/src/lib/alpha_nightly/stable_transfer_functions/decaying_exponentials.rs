//! Decaying-exponential modes of a critically damped or overdamped
//! second-order transfer function, advanced by an **exact** constant-time
//! recurrence.
//!
//! These are the terms a numerator zero contributes to a non-oscillatory
//! second-order block,
//!
//! ```text
//!         a1 s^2 + b s
//! G(s) = -------------------
//!         a2 s^2 + b2 s + c
//! ```
//!
//! whose response to an input increment `a_1` applied at `t_1`, writing
//! `s = t - t_1`, is
//!
//! - *overdamped* (two distinct real roots): `a_1 [M_a exp(-alpha s)
//!   + M_b exp(-beta s)]`
//! - *critically damped* (one repeated root `lambda`): `a_1 [M_a s
//!   exp(-lambda s) + M_b exp(-lambda s)]`
//!
//! # Why a recurrence
//!
//! A plain exponential mode scales by `exp(-alpha T)` over a step, and the
//! `s exp(-lambda s)` mode obeys
//!
//! ```text
//! (s + T) exp(-lambda (s + T))
//!     = exp(-lambda T) [ s exp(-lambda s) + T exp(-lambda s) ]
//! ```
//!
//! so carrying the plain mode alongside it closes the recurrence in two
//! numbers. Both are identities of the exponential, hence **exact** — not a
//! numerical approximation — for input held constant between calls. This is
//! the zero-order-hold (step-invariant) equivalent of the block; see the
//! module documentation of [`super::first_order_transfer_fn`] for the general
//! statement.
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
//! - Hochbruck, M. and Ostermann, A., "Exponential integrators", *Acta
//!   Numerica* — the numerical-ODE framing used by this workspace's
//!   `teh-o-prke` decay-heat integrator.
//!
//! # What this module replaced
//!
//! Until version 0.2.0 the block pushed one
//! [`DecaySecondOrderExponentialResponse`] per input change onto a `Vec` and
//! re-summed it on every call. See bead `op-fm5`.

use std::collections::VecDeque;

use uom::{
    si::{f64::*, frequency::hertz, ratio::ratio, time::second},
    ConstZero,
};

use super::first_order_transfer_fn::PendingStepInput;
use crate::alpha_nightly::errors::ChemEngProcessControlSimulatorError;

/// The decaying-exponential state of this block, one variant per root
/// structure.
///
/// An enum rather than a trait object, so that adding a root structure is a
/// compile error at every match site.
#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub(crate) enum SecondOrderExponentialModes {
    /// Two distinct real roots `-alpha` and `-beta`.
    Overdamped {
        /// `sum_k M_a du_k exp(-alpha s_k)` (dimensionless).
        alpha_mode: Ratio,
        /// `sum_k M_b du_k exp(-beta s_k)` (dimensionless).
        beta_mode: Ratio,
    },
    /// One repeated real root `-lambda` (so `alpha == beta == lambda`).
    CriticallyDamped {
        /// `sum_k M_a du_k s_k exp(-lambda s_k)` (dimensionless). This is the
        /// mode the block actually emits.
        t_alpha_mode: Ratio,
        /// `sum_k M_a du_k exp(-lambda s_k)` (hertz). Carried only so that
        /// `t_alpha_mode` can be advanced; `M_a` is a rate, which is why this
        /// companion is a `Frequency` and `t_alpha_mode` is dimensionless.
        alpha_rate_mode: Frequency,
        /// `sum_k M_b du_k exp(-lambda s_k)` (dimensionless).
        beta_mode: Ratio,
    },
}

impl SecondOrderExponentialModes {
    /// A zeroed state for the given root structure.
    fn zeroed_for(exponent_type: DecayingExponentialType) -> Self {
        match exponent_type {
            DecayingExponentialType::Overdamped => Self::Overdamped {
                alpha_mode: Ratio::ZERO,
                beta_mode: Ratio::ZERO,
            },
            DecayingExponentialType::CriticallyDamped => Self::CriticallyDamped {
                t_alpha_mode: Ratio::ZERO,
                alpha_rate_mode: Frequency::ZERO,
                beta_mode: Ratio::ZERO,
            },
        }
    }

    /// True if this state belongs to the given root structure.
    fn matches(&self, exponent_type: DecayingExponentialType) -> bool {
        matches!(
            (self, exponent_type),
            (Self::Overdamped { .. }, DecayingExponentialType::Overdamped)
                | (
                    Self::CriticallyDamped { .. },
                    DecayingExponentialType::CriticallyDamped
                )
        )
    }

    /// The block's output contribution, dimensionless.
    fn output(&self) -> Ratio {
        match self {
            Self::Overdamped {
                alpha_mode,
                beta_mode,
            } => *alpha_mode + *beta_mode,
            Self::CriticallyDamped {
                t_alpha_mode,
                beta_mode,
                ..
            } => *t_alpha_mode + *beta_mode,
        }
    }

    /// Advances the modes over `time_step` seconds. `alpha` and `beta` are
    /// the decay rates in hertz.
    fn advance(&mut self, time_step: Time, alpha: Frequency, beta: Frequency) {
        let alpha_decay = (-(time_step * alpha).get::<ratio>()).exp();
        let beta_decay = (-(time_step * beta).get::<ratio>()).exp();

        match self {
            Self::Overdamped {
                alpha_mode,
                beta_mode,
            } => {
                *alpha_mode = *alpha_mode * alpha_decay;
                *beta_mode = *beta_mode * beta_decay;
            }
            Self::CriticallyDamped {
                t_alpha_mode,
                alpha_rate_mode,
                beta_mode,
            } => {
                // (s + T) exp(-lambda (s + T))
                //   = exp(-lambda T) [ s exp(-lambda s) + T exp(-lambda s) ]
                *t_alpha_mode = (*t_alpha_mode + time_step * *alpha_rate_mode) * alpha_decay;
                *alpha_rate_mode = *alpha_rate_mode * alpha_decay;
                *beta_mode = *beta_mode * beta_decay;
            }
        }
    }

    /// Injects an input increment that started acting `time_since_start`
    /// seconds ago.
    ///
    /// `alpha_magnitude` is `M_a du` — dimensionless in the overdamped case
    /// and a rate (hertz) in the critically damped case, which is why it is
    /// passed as a `Frequency` there. `beta_magnitude` is `M_b du`,
    /// dimensionless in both.
    fn inject_overdamped(
        &mut self,
        alpha_magnitude: Ratio,
        beta_magnitude: Ratio,
        time_since_start: Time,
        alpha: Frequency,
        beta: Frequency,
    ) {
        if let Self::Overdamped {
            alpha_mode,
            beta_mode,
        } = self
        {
            *alpha_mode += alpha_magnitude * (-(time_since_start * alpha).get::<ratio>()).exp();
            *beta_mode += beta_magnitude * (-(time_since_start * beta).get::<ratio>()).exp();
        }
    }

    /// Injects an input increment into the critically damped state; see
    /// [`Self::inject_overdamped`] for the unit convention.
    fn inject_critically_damped(
        &mut self,
        alpha_rate_magnitude: Frequency,
        beta_magnitude: Ratio,
        time_since_start: Time,
        alpha: Frequency,
        beta: Frequency,
    ) {
        if let Self::CriticallyDamped {
            t_alpha_mode,
            alpha_rate_mode,
            beta_mode,
        } = self
        {
            let alpha_decay = (-(time_since_start * alpha).get::<ratio>()).exp();
            *t_alpha_mode += time_since_start * alpha_rate_magnitude * alpha_decay;
            *alpha_rate_mode += alpha_rate_magnitude * alpha_decay;
            *beta_mode += beta_magnitude * (-(time_since_start * beta).get::<ratio>()).exp();
        }
    }
}

/// step responses for transfer function of type
///
/// G(s) = (a1 s^2 + bs)/ (a2 s^2 + b2 s + c)
///
/// for the non-oscillatory (critically damped or overdamped) cases.
///
/// # Physical quantity
///
/// Maps a dimensionless input signal to a dimensionless output signal. The
/// block contributes decaying exponentials and nothing at steady state.
///
/// # State
///
/// Two or three numbers, held in [`SecondOrderExponentialModes`]. Cost is
/// O(1) in time and memory.
///
/// # Valid ranges and assumptions
///
/// - `alpha` and `beta` are decay rates in hertz; `alpha` must be strictly
///   positive (checked by the constructors) and `beta` should be too.
/// - `magnitude_alpha` is dimensionless in the overdamped case; in the
///   critically damped case it holds a **rate** whose numeric value is in
///   hertz, kept in a `Ratio` for backwards compatibility with the
///   pre-0.2.0 field layout. `magnitude_beta` is dimensionless in both.
/// - `exponent_type` must **not** be mutated after construction — if it is,
///   the mode state is silently reset to zero on the next call, which
///   discards the transient.
/// - `delay` is a dead time in seconds; zero is the common case.
/// - Simulation time must be non-decreasing across calls.
#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct DecayingSecondOrderExponential {
    /// `M_a`: dimensionless when overdamped, a hertz-valued rate when
    /// critically damped (see the struct documentation).
    pub(crate) magnitude_alpha: Ratio,
    /// `M_b`, dimensionless.
    pub(crate) magnitude_beta: Ratio,
    /// decay frequency of first root, (hertz); strictly positive.
    pub(crate) alpha: Frequency,
    /// decay frequency of second root (hertz), equal to `alpha` when there is
    /// only one root.
    pub(crate) beta: Frequency,
    /// Input at the previous call (dimensionless).
    pub(crate) previous_timestep_input: Ratio,
    /// Output offset; constant after construction because decaying
    /// exponentials contribute nothing at steady state (dimensionless).
    pub(crate) offset: Ratio,
    /// Dead time / transport delay (seconds).
    pub(crate) delay: Time,
    /// The decaying-exponential state.
    pub(crate) decaying_modes: SecondOrderExponentialModes,
    /// Simulation time of the previous call; `None` before the first call.
    pub(crate) last_update_time: Option<Time>,
    /// Increments accepted but not yet due; empty unless a dead time is set.
    pub(crate) pending_inputs: VecDeque<PendingStepInput>,
    /// choose whether it's a critically damped or
    /// overdamped system
    pub(crate) exponent_type: DecayingExponentialType,
}

/// Whether the block's denominator has two distinct real roots or one
/// repeated real root.
#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub enum DecayingExponentialType {
    /// two distinct roots
    Overdamped,
    /// two equal real roots
    CriticallyDamped,
}

impl DecayingSecondOrderExponential {
    /// constructor for new over damped system
    /// with two real roots
    ///
    /// # Parameters
    ///
    /// - `magnitude_alpha`, `magnitude_beta` — `M_a`, `M_b`, dimensionless.
    /// - `alpha`, `beta` — decay rates in hertz; `alpha` strictly positive.
    /// - `initial_input` — starting input, dimensionless.
    /// - `initial_value` — output offset, dimensionless.
    /// - `delay` — dead time in seconds.
    ///
    /// # Errors
    ///
    /// Returns
    /// [`ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction`]
    /// if `alpha` is not strictly positive.
    pub fn new_overdamped(
        magnitude_alpha: Ratio,
        magnitude_beta: Ratio,
        alpha: Frequency,
        beta: Frequency,
        initial_input: Ratio,
        initial_value: Ratio,
        delay: Time,
    ) -> Result<Self, ChemEngProcessControlSimulatorError> {
        // if damping factor is less than or equal
        // 0, should throw an error
        // or panic (i will use errors maybe later?)

        let exponent_type = DecayingExponentialType::Overdamped;

        if alpha.value <= 0.0 {
            return Err(
                ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction,
            );
        }
        Ok(DecayingSecondOrderExponential {
            magnitude_alpha,
            magnitude_beta,
            alpha,
            beta,
            previous_timestep_input: initial_input,
            offset: initial_value,
            delay,
            decaying_modes: SecondOrderExponentialModes::zeroed_for(exponent_type),
            last_update_time: None,
            pending_inputs: VecDeque::new(),
            exponent_type,
        })
    }

    /// constructor for new critically damped system
    /// with two equal roots
    ///
    /// it will be in the form
    ///
    /// magnitude_alpha * t * exp (-alpha t)
    /// + magnitude_beta * exp (- beta t)
    ///
    /// magnitude_alpha is necessarily in a frequency unit
    /// and it will be converted into hertz before storage
    ///
    /// # Parameters
    ///
    /// - `magnitude_alpha` — `M_a`, a rate in hertz (it multiplies a time).
    /// - `magnitude_beta` — `M_b`, dimensionless.
    /// - `lambda` — the repeated root's decay rate in hertz; strictly
    ///   positive.
    /// - `initial_input` — starting input, dimensionless.
    /// - `initial_value` — output offset, dimensionless.
    /// - `delay` — dead time in seconds.
    ///
    /// # Errors
    ///
    /// Returns
    /// [`ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction`]
    /// if `lambda` is not strictly positive.
    pub fn new_critical(
        magnitude_alpha: Frequency,
        magnitude_beta: Ratio,
        lambda: Frequency,
        initial_input: Ratio,
        initial_value: Ratio,
        delay: Time,
    ) -> Result<Self, ChemEngProcessControlSimulatorError> {
        // if damping factor is less than or equal
        // 0, should throw an error
        // or panic (i will use errors maybe later?)

        let exponent_type = DecayingExponentialType::CriticallyDamped;
        let magnitude_alpha = Ratio::new::<ratio>(magnitude_alpha.get::<hertz>());

        // for critically damped systems, there is only one characteristic
        // damping frequency, which is lambda
        // therefore, the real part of the
        // two roots, alpha and beta are the same
        let alpha = lambda;
        let beta = lambda;

        if alpha.value <= 0.0 {
            return Err(
                ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction,
            );
        }
        Ok(DecayingSecondOrderExponential {
            magnitude_alpha,
            magnitude_beta,
            alpha,
            beta,
            previous_timestep_input: initial_input,
            offset: initial_value,
            delay,
            decaying_modes: SecondOrderExponentialModes::zeroed_for(exponent_type),
            last_update_time: None,
            pending_inputs: VecDeque::new(),
            exponent_type,
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
        // guard against an exponent type mutated behind our back
        if !self.decaying_modes.matches(self.exponent_type) {
            self.decaying_modes = SecondOrderExponentialModes::zeroed_for(self.exponent_type);
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

        Ok(self.offset + self.decaying_modes.output())
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
            .advance(time_step, self.alpha, self.beta);
    }

    /// Switches an input increment on, having already been acting for
    /// `time_since_start` seconds.
    fn activate(&mut self, input_increment: Ratio, time_since_start: Time) {
        let magnitude_alpha_times_user_input = self.magnitude_alpha * input_increment;
        let magnitude_beta_times_user_input = self.magnitude_beta * input_increment;

        match self.exponent_type {
            DecayingExponentialType::Overdamped => {
                self.decaying_modes.inject_overdamped(
                    magnitude_alpha_times_user_input,
                    magnitude_beta_times_user_input,
                    time_since_start,
                    self.alpha,
                    self.beta,
                );
            }
            DecayingExponentialType::CriticallyDamped => {
                self.decaying_modes.inject_critically_damped(
                    Frequency::new::<hertz>(magnitude_alpha_times_user_input.get::<ratio>()),
                    magnitude_beta_times_user_input,
                    time_since_start,
                    self.alpha,
                    self.beta,
                );
            }
        }
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
    /// (the widest variant) plus any queued-but-not-yet-due inputs.
    #[allow(dead_code)]
    pub(crate) fn state_size(&self) -> usize {
        3 + self.pending_inputs.len()
    }
}

/// Closed-form decaying-exponential response to a single input increment.
///
/// for decaying exponential responses, we have two main cases
/// the first is where there are two equal real roots. This is
/// critical damping
///
/// The second is where we have two real unequal roots. This is
/// overdamping.
///
/// # Role
///
/// Since version 0.2.0 this is the **analytic reference** the recurrence in
/// [`DecayingSecondOrderExponential`] is verified against; it no longer
/// drives simulations.
///
/// Units follow [`DecayingSecondOrderExponential`]: magnitudes dimensionless
/// (except `magnitude_alpha` in the critically damped case, which is a
/// hertz-valued rate), rates in hertz, times in seconds.
#[allow(dead_code)]
#[derive(Debug, PartialEq, PartialOrd, Clone, Copy)]
pub(crate) struct DecaySecondOrderExponentialResponse {
    magnitude_alpha_times_user_input: Ratio,
    magnitude_beta_times_user_input: Ratio,
    alpha: Frequency,
    beta: Frequency,
    start_time: Time,
    user_input: Ratio,
    current_time: Time,
    exponential_type: DecayingExponentialType,
}

impl Default for DecaySecondOrderExponentialResponse {
    /// default is a critically damped system with
    /// 1 / ( (s+1)^2 + 1)
    /// time in seconds,
    /// frequency in hertz
    fn default() -> Self {
        DecaySecondOrderExponentialResponse {
            magnitude_alpha_times_user_input: Ratio::new::<ratio>(1.0),
            magnitude_beta_times_user_input: Ratio::new::<ratio>(1.0),
            alpha: Frequency::new::<hertz>(1.0),
            beta: Frequency::new::<hertz>(1.0),
            start_time: Time::new::<second>(0.0),
            user_input: Ratio::new::<ratio>(1.0),
            current_time: Time::new::<second>(0.0),
            exponential_type: DecayingExponentialType::CriticallyDamped,
        }
    }
}

#[allow(dead_code)]
impl DecaySecondOrderExponentialResponse {
    /// constructor for new over damped system
    /// with two real roots
    ///
    /// # Errors
    ///
    /// Returns
    /// [`ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction`]
    /// if `alpha` is not strictly positive.
    pub fn new_overdamped(
        magnitude_alpha_times_user_input: Ratio,
        magnitude_beta_times_user_input: Ratio,
        alpha: Frequency,
        beta: Frequency,
        start_time: Time,
        user_input: Ratio,
        current_time: Time,
    ) -> Result<Self, ChemEngProcessControlSimulatorError> {
        // if damping factor is less than or equal
        // 0, should throw an error
        // or panic (i will use errors maybe later?)

        let exponential_type = DecayingExponentialType::Overdamped;

        if alpha.value <= 0.0 {
            return Err(
                ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction,
            );
        }
        Ok(DecaySecondOrderExponentialResponse {
            magnitude_alpha_times_user_input,
            magnitude_beta_times_user_input,
            alpha,
            beta,
            start_time,
            user_input,
            current_time,
            exponential_type,
        })
    }

    /// constructor for new over damped system
    /// with two equal roots
    ///
    /// it will be in the form
    ///
    /// magnitude_alpha * t * exp (-alpha t)
    /// + magnitude_beta * exp (- beta t)
    ///
    /// magnitude_alpha is necessarily in a frequency unit
    /// and it will be converted into hertz before storage
    ///
    /// # Errors
    ///
    /// Returns
    /// [`ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction`]
    /// if `alpha` is not strictly positive.
    pub fn new_critical(
        magnitude_alpha_times_user_input: Frequency,
        magnitude_beta_times_user_input: Ratio,
        alpha: Frequency,
        beta: Frequency,
        start_time: Time,
        user_input: Ratio,
        current_time: Time,
    ) -> Result<Self, ChemEngProcessControlSimulatorError> {
        // if damping factor is less than or equal
        // 0, should throw an error
        // or panic (i will use errors maybe later?)

        let exponential_type = DecayingExponentialType::CriticallyDamped;
        let magnitude_alpha_times_user_input =
            Ratio::new::<ratio>(magnitude_alpha_times_user_input.get::<hertz>());

        if alpha.value <= 0.0 {
            return Err(
                ChemEngProcessControlSimulatorError::UnstableDampingFactorForStableTransferFunction,
            );
        }
        Ok(DecaySecondOrderExponentialResponse {
            magnitude_alpha_times_user_input,
            magnitude_beta_times_user_input,
            alpha,
            beta,
            start_time,
            user_input,
            current_time,
            exponential_type,
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
        let at: Ratio = time_elapsed * self.alpha;
        let bt: Ratio = time_elapsed * self.beta;

        match self.exponential_type {
            DecayingExponentialType::Overdamped => {
                // need both alpha and beta to be more than 20
                if at > Ratio::new::<ratio>(20.0) && bt > Ratio::new::<ratio>(20.0) {
                    return true;
                }
            }
            DecayingExponentialType::CriticallyDamped => {
                // for critically damped systems we can represent it
                // using x exp (-x)
                //
                // if x > 23, generally we can ignore things
                //
                // However, this usually comes in the form:
                //
                // t exp (-lambda t)
                //
                // so lambda t = 23 doesn't quite cut it all the time
                // we must impose an additional constraint.
                // Consider rewriting x exp (-x)
                // 1/ lambda * (lambda t) exp (-lambda t)
                //
                // my tolerance initially was for
                // (lambda t) exp (- lambda t) \approx 1e-9
                //
                // Now I must also consider 1/lambda
                //
                // so just take the product,

                let inverse_lambda: Time = 1.0 / self.alpha;
                let lambda_t = at;

                // i'm not going to go into specifics... but this
                // will have to do

                let exponent_ratio: Ratio =
                    lambda_t * (-lambda_t.get::<ratio>()).exp() * inverse_lambda.get::<second>();

                let exponent_decayed: bool =
                    at > Ratio::new::<ratio>(23.0) && bt > Ratio::new::<ratio>(23.0);

                if exponent_decayed && exponent_ratio < Ratio::new::<ratio>(1e-10) {
                    return true;
                }
            }
        }

        return false;
    }

    /// Evaluates the decaying-exponential response at `simulation_time`,
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

        // for convenience, we calculate alpha t and beta t
        let alpha_t: Ratio = time_elapsed * self.alpha;
        let beta_t: Ratio = time_elapsed * self.beta;

        response = match self.exponential_type {
            DecayingExponentialType::CriticallyDamped => {
                // for two equal roots, also quite straightforward
                // magnitude_alpha * t * exp(-alpha t)
                // magnitude_beta * exp(-beta t)

                let t_exponential_response = self.magnitude_alpha_times_user_input
                    * time_elapsed.get::<second>()
                    * (-alpha_t.get::<ratio>()).exp();

                let exponential_response =
                    self.magnitude_beta_times_user_input * (-beta_t.get::<ratio>()).exp();

                t_exponential_response + exponential_response
            }
            DecayingExponentialType::Overdamped => {
                // for two unequal roots, also quite straightforward
                // magnitude_alpha * exp(-alpha t)
                // magnitude_beta * exp(-beta t)
                // user input part checks out

                self.magnitude_alpha_times_user_input * (-alpha_t.get::<ratio>()).exp()
                    + self.magnitude_beta_times_user_input * (-beta_t.get::<ratio>()).exp()
            }
        };

        return response;
    }

    /// steady state value
    /// of a decaying exponential is zero
    /// eventually
    pub fn steady_state_value(&self) -> Ratio {
        let response: Ratio = Ratio::ZERO;
        response
    }
}
