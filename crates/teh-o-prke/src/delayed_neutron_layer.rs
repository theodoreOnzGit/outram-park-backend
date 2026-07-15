//! Reusable **delayed-neutron layer** — a reduced point-kinetics precursor
//! bank modelled as five independent first-order lags, one per delayed-neutron
//! group.
//!
//! # What this module is for
//!
//! The [`crate::nordheim_fuchs`] exact timestepper is a **prompt-only**
//! excursion model: it evolves reactor power and adiabatic fuel temperature
//! but carries **no delayed-neutron precursors**. Physically the delayed
//! neutrons are the reservoir that gives an operating reactor its inertia and
//! its long, controllable period: a chain that is *prompt* subcritical (net
//! reactivity `rho` below the delayed fraction `beta`) is held critical only
//! by neutrons emitted seconds-to-minutes later from decaying precursors.
//! Strip that reservoir out and a prompt-only model coupled to a thermal
//! feedback loop degenerates into a bang-bang relaxation oscillation — power
//! explodes whenever the prompt margin `rho - beta` goes positive and
//! collapses the instant it goes negative, driving the fuel temperature (hence
//! the reactivity) up and down without damping.
//!
//! This layer restores the delayed-neutron reservoir while **keeping
//! Nordheim-Fuchs for the prompt response**. It sits between the
//! prompt-excursion layer and the thermal-hydraulics layer as a
//! *delayed-neutron source* in a Lie-split point-kinetics update (see
//! "How to couple it" below).
//!
//! # The model it implements
//!
//! Point reactor kinetics in reactor-power form (power `P`, prompt neutron
//! generation time `Lambda`, net reactivity `rho`, delayed fraction
//! `beta = sum_i beta_i`):
//!
//! ```text
//!   dP/dt   = (rho - beta)/Lambda * P + S ,   S = sum_i lambda_i C_i
//!   dC_i/dt = beta_i/Lambda * P - lambda_i C_i
//! ```
//!
//! The **prompt** part `(rho - beta)/Lambda * P` (with adiabatic
//! fuel-temperature feedback folded into `rho`) is exactly what
//! [`crate::nordheim_fuchs::NordheimFuchsExactTimestepper::step`] advances.
//! This layer owns the **delayed** part: the precursor source
//! `S = sum_i lambda_i C_i`.
//!
//! Differentiating `S_i = lambda_i C_i` gives a **first-order lag** for each
//! group's source contribution:
//!
//! ```text
//!   tau_i dS_i/dt + S_i = (beta_i / Lambda) * P ,   tau_i = 1 / lambda_i
//! ```
//!
//! i.e. transfer function `S_i(s) / P(s) = (beta_i/Lambda) / (tau_i s + 1)`:
//! a lag of DC gain `beta_i/Lambda` and time constant `tau_i = 1/lambda_i`.
//! Each group is exactly one [`TransferFnFirstOrder`]. The total delayed
//! source is `S = sum_i S_i`, and over a timestep `dt` it injects a delayed
//! power increment `Delta P_delayed = S * dt` into the balance.
//!
//! At steady state each lag settles to `S_i = (beta_i/Lambda) P`, so
//! `S = (beta/Lambda) P` and the power equation forces `rho = 0` (delayed
//! critical) — the physically correct operating point, which the prompt-only
//! model could not reach (it sat at prompt critical, `rho = beta`, and rang).
//!
//! This is a deliberately reduced (pedagogical / real-time-simulator) model,
//! **not** a full spatially-resolved kinetics solve. It is intended for
//! education, capability building, and V&V demonstrations — not for
//! licensing, safety, or operational analysis.
//!
//! # How to couple it (Lie-split point kinetics)
//!
//! Per timestep `dt`, with the prompt-excursion layer as the prompt
//! propagator:
//!
//! 1. set the prompt layer's power to the current total power and its
//!    reactivity/feedback, then advance it one step → prompt power `P_p`
//!    (this applies the `(rho - beta)/Lambda * P` term and adiabatic
//!    feedback);
//! 2. `let dp_delayed = layer.advance(P_p, dt);` — updates the precursor
//!    lags and returns the delayed power increment `S * dt`;
//! 3. total power `P = P_p + dp_delayed`; feed `P` back to the prompt layer
//!    for the next step and to the thermal-hydraulics layer.
//!
//! The delayed increment keeps the reactor alive through the prompt-subcritical
//! operating regime and, because the `S_i` lag `P`, supplies the inertia that
//! damps the fuel-temperature feedback loop.
//!
//! # Standard delayed-neutron data
//!
//! [`DelayedNeutronLayer::u235_five_group`] bakes in a documented five-group
//! reduced set for thermal U-235 (see that constructor). For any other fuel or
//! group structure, build the layer from explicit `(beta_i, lambda_i)` pairs
//! with [`DelayedNeutronLayer::new`].

use chem_eng_real_time_process_control_simulator::beta_testing::transfer_fn_wrapper_and_enums::{
    TransferFnFirstOrder, TransferFnTraits,
};
use uom::si::f64::*;
use uom::si::frequency::hertz;
use uom::si::power::megawatt;
use uom::si::ratio::ratio;
use uom::si::time::second;
use uom::ConstZero;

use crate::teh_o_prke_error::TehOPrkeError;

/// Number of delayed-neutron groups this layer models. Fixed at five
/// first-order transfer functions (see the module documentation for why the
/// six-group standard data is reduced to five).
pub const NUM_DELAYED_GROUPS: usize = 5;

/// One delayed-neutron precursor group: its delayed fraction `beta_i`, its
/// decay constant `lambda_i`, and the first-order lag
/// `(beta_i/Lambda)/(tau_i s + 1)` (`tau_i = 1/lambda_i`) that produces this
/// group's delayed-neutron source contribution `S_i = lambda_i C_i` from the
/// reactor power.
#[derive(Debug, Clone)]
struct DelayedNeutronGroup {
    /// Delayed-neutron fraction `beta_i` (dimensionless) for this group.
    delayed_fraction: Ratio,
    /// Decay constant `lambda_i` \[s^-1\] for this group; the lag time
    /// constant is `tau_i = 1/lambda_i`.
    decay_constant: Frequency,
    /// First-order lag `(beta_i/Lambda) / (tau_i s + 1)` mapping the reactor
    /// power (in MW, carried as a dimensionless [`Ratio`] sample) to this
    /// group's delayed source contribution `S_i` (in MW/s).
    lag: TransferFnFirstOrder,
}

/// A reusable delayed-neutron layer: five first-order-lag precursor groups
/// that turn a prompt-only kinetics model into proper point kinetics by
/// supplying the delayed-neutron source `S = sum_i lambda_i C_i`.
///
/// Construct it with [`DelayedNeutronLayer::u235_five_group`] (documented
/// thermal-U-235 data) or [`DelayedNeutronLayer::new`] (arbitrary
/// `(beta_i, lambda_i)` data), then call [`DelayedNeutronLayer::advance`] once
/// per timestep with the prompt power to get the delayed power increment. See
/// the module-level documentation for the model and the coupling recipe.
#[derive(Debug, Clone)]
pub struct DelayedNeutronLayer {
    /// The five precursor groups (each a first-order lag).
    groups: [DelayedNeutronGroup; NUM_DELAYED_GROUPS],
    /// Prompt neutron generation time `Lambda` \[s\] used to scale each
    /// group's source gain `beta_i/Lambda`.
    prompt_generation_time: Time,
    /// Total delayed-neutron fraction `beta = sum_i beta_i`.
    total_delayed_fraction: Ratio,
    /// Running simulation time; the underlying transfer functions integrate
    /// against absolute time, so this is advanced by `dt` on every
    /// [`Self::advance`] call.
    elapsed_time: Time,
    /// Most recently computed delayed-neutron source `S = sum_i lambda_i C_i`
    /// \[expressed as a power rate, MW/s → stored as `Power`/`Time` via the
    /// increment; see [`Self::advance`]\]. Stored as the last power increment
    /// for reporting.
    last_delayed_increment: Power,
}

impl DelayedNeutronLayer {
    /// Builds a delayed-neutron layer from explicit per-group data.
    ///
    /// # Parameters
    /// - `prompt_generation_time`: the prompt neutron generation time
    ///   `Lambda` \[s\] of the reactor whose precursors these are; it sets
    ///   each group's source gain `beta_i/Lambda`. Must be strictly positive.
    /// - `groups`: five `(beta_i, lambda_i)` pairs. `beta_i` is dimensionless
    ///   ([`Ratio`]); `lambda_i` is a [`Frequency`] (`s^-1`) and must be
    ///   strictly positive so the lag has a finite, stable time constant.
    ///
    /// # Errors
    /// Returns [`TehOPrkeError::NonPositivePromptNeutronGenerationTime`] if
    /// `Lambda <= 0`, or [`TehOPrkeError::NonPositiveDelayedDecayConstant`] if
    /// any `lambda_i <= 0`.
    pub fn new(
        prompt_generation_time: Time,
        groups: [(Ratio, Frequency); NUM_DELAYED_GROUPS],
    ) -> Result<Self, TehOPrkeError> {
        let lambda_gen = prompt_generation_time.get::<second>();
        if lambda_gen <= 0.0 {
            return Err(TehOPrkeError::NonPositivePromptNeutronGenerationTime(
                lambda_gen,
            ));
        }

        let mut total_delayed_fraction = Ratio::ZERO;
        let mut built: Vec<DelayedNeutronGroup> = Vec::with_capacity(NUM_DELAYED_GROUPS);
        for (beta_i, lambda_i) in groups.into_iter() {
            let lambda_hz = lambda_i.get::<hertz>();
            if lambda_hz <= 0.0 {
                return Err(TehOPrkeError::NonPositiveDelayedDecayConstant(lambda_hz));
            }
            let tau_i: Time = Time::new::<second>(1.0 / lambda_hz);

            // source gain beta_i / Lambda (units s^-1, carried as a
            // dimensionless transfer-function gain number)
            let source_gain = Ratio::new::<ratio>(beta_i.get::<ratio>() / lambda_gen);

            // lag G(s) = source_gain / (tau_i s + 1):
            //   a1 = 0 (no numerator zero), b1 = source_gain,
            //   a2 = tau_i,                  b2 = 1.
            let lag =
                TransferFnFirstOrder::new(Time::ZERO, source_gain, tau_i, Ratio::new::<ratio>(1.0))
                    .map_err(|e| TehOPrkeError::GenericStringError(e.to_string()))?;

            total_delayed_fraction += beta_i;
            built.push(DelayedNeutronGroup {
                delayed_fraction: beta_i,
                decay_constant: lambda_i,
                lag,
            });
        }

        let groups: [DelayedNeutronGroup; NUM_DELAYED_GROUPS] = built.try_into().map_err(|_| {
            TehOPrkeError::GenericStringError(
                "DelayedNeutronLayer::new: expected exactly 5 groups".to_string(),
            )
        })?;

        Ok(Self {
            groups,
            prompt_generation_time,
            total_delayed_fraction,
            elapsed_time: Time::ZERO,
            last_delayed_increment: Power::ZERO,
        })
    }

    /// Builds the layer with a **documented five-group reduced set for thermal
    /// U-235**, for a reactor of prompt neutron generation time `Lambda`.
    ///
    /// # Data provenance
    /// The six-group thermal-U-235 delayed-neutron data used elsewhere in this
    /// crate (`zero_power_prke::six_group_precursor_prke::six_group_constants`,
    /// half-lives 56.0, 23.0, 6.2, 2.3, 0.61, 0.23 s; group fractions 0.00021,
    /// 0.00142, 0.00128, 0.00257, 0.00075, 0.00027; total `beta = 0.0065`) is
    /// collapsed here to **five** groups by merging the two shortest-lived
    /// groups (half-lives 0.61 s and 0.23 s). The merged group keeps their
    /// combined fraction `beta = 0.00075 + 0.00027 = 0.00102` and takes a
    /// fraction-weighted effective decay constant
    /// `lambda = (0.00075*lambda_5 + 0.00027*lambda_6) / 0.00102`, preserving
    /// the total delayed fraction (`beta = 0.0065`) and the aggregate fast
    /// delayed-neutron emission rate. `lambda_i = ln(2) / t_half,i`.
    ///
    /// Resulting five groups (`beta_i`, `lambda_i` \[s^-1\]):
    ///
    /// | i | `beta_i`  | `t_half` \[s\] | `lambda_i` \[s^-1\] |
    /// |---|-----------|---------------|---------------------|
    /// | 1 | 0.00021   | 56.0          | 0.012378            |
    /// | 2 | 0.00142   | 23.0          | 0.030137            |
    /// | 3 | 0.00128   | 6.2           | 0.111799            |
    /// | 4 | 0.00257   | 2.3           | 0.301369            |
    /// | 5 | 0.00102   | (merged)      | 1.633286            |
    ///
    /// These are round, order-of-magnitude illustrative constants suitable for
    /// education and V&V, consistent with the workspace data policy — not
    /// design data for any specific licensed reactor.
    ///
    /// # Panics
    /// If `prompt_generation_time <= 0`.
    pub fn u235_five_group(prompt_generation_time: Time) -> Self {
        let groups = [
            (
                Ratio::new::<ratio>(0.00021),
                Frequency::new::<hertz>(0.012378),
            ),
            (
                Ratio::new::<ratio>(0.00142),
                Frequency::new::<hertz>(0.030137),
            ),
            (
                Ratio::new::<ratio>(0.00128),
                Frequency::new::<hertz>(0.111799),
            ),
            (
                Ratio::new::<ratio>(0.00257),
                Frequency::new::<hertz>(0.301369),
            ),
            (
                Ratio::new::<ratio>(0.00102),
                Frequency::new::<hertz>(1.633286),
            ),
        ];
        Self::new(prompt_generation_time, groups)
            .expect("DelayedNeutronLayer::u235_five_group: Lambda > 0 and all lambda_i > 0")
    }

    /// Total delayed-neutron fraction `beta = sum_i beta_i` (dimensionless).
    pub fn total_delayed_neutron_fraction(&self) -> Ratio {
        self.total_delayed_fraction
    }

    /// Prompt neutron generation time `Lambda` \[s\] this layer was built with.
    pub fn prompt_generation_time(&self) -> Time {
        self.prompt_generation_time
    }

    /// The per-group decay constants `lambda_i` \[s^-1\], in construction order.
    pub fn decay_constants(&self) -> [Frequency; NUM_DELAYED_GROUPS] {
        core::array::from_fn(|i| self.groups[i].decay_constant)
    }

    /// The per-group delayed-neutron fractions `beta_i` (dimensionless), in
    /// construction order.
    pub fn delayed_fractions(&self) -> [Ratio; NUM_DELAYED_GROUPS] {
        core::array::from_fn(|i| self.groups[i].delayed_fraction)
    }

    /// The delayed power increment `S * dt` produced by the most recent
    /// [`Self::advance`] call. Zero before the first call.
    pub fn last_delayed_increment(&self) -> Power {
        self.last_delayed_increment
    }

    /// Advances the precursor bank by one timestep `dt` and returns the
    /// **delayed power increment** `Delta P_delayed = S * dt` to add to the
    /// power balance this step, where `S = sum_i lambda_i C_i` is the
    /// delayed-neutron source.
    ///
    /// Each group's first-order lag `(beta_i/Lambda)/(tau_i s + 1)` is advanced
    /// with the current reactor power as input; their sum is the source `S`,
    /// and the returned increment is `S * dt`. See the module documentation
    /// for the Lie-split coupling recipe.
    ///
    /// # Parameters
    /// - `reactor_power`: the reactor power `P` driving precursor production
    ///   this step (in the recommended coupling, the prompt layer's freshly
    ///   advanced power `P_p`). Must be finite and non-negative for a
    ///   physical result.
    /// - `dt`: the timestep. The underlying lags integrate exactly for a
    ///   piecewise-constant input, so any positive `dt` is stable.
    pub fn advance(&mut self, reactor_power: Power, dt: Time) -> Power {
        self.elapsed_time += dt;
        let now = self.elapsed_time;

        // normalise power to a dimensionless sample (value = power in MW) for
        // the transfer functions, which operate on Ratio.
        let power_sample = Ratio::new::<ratio>(reactor_power.get::<megawatt>());

        // sum_i S_i, with S_i in MW/s (carried as a Ratio number)
        let mut source_mw_per_s = 0.0;
        for group in self.groups.iter_mut() {
            let s_i = group
                .lag
                .set_user_input_and_calc(power_sample, now)
                // a stable first-order lag with constant coefficients cannot
                // return an error for a finite input; surface any as a panic
                // rather than silently returning stale state (matches the
                // workspace per-step-evaluation convention).
                .expect("delayed-neutron first-order lag evaluated a finite input");
            source_mw_per_s += s_i.get::<ratio>();
        }

        // delayed power increment over the step: S * dt  [MW]
        let dt_s = dt.get::<second>();
        let increment = Power::new::<megawatt>(source_mw_per_s * dt_s);
        self.last_delayed_increment = increment;
        increment
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    /// U-235 illustrative prompt generation time used across this crate.
    fn lambda() -> Time {
        Time::new::<second>(2.31e-4)
    }

    /// The five-group U-235 dataset sums to the standard total delayed
    /// fraction `beta = 0.0065`.
    #[test]
    fn u235_five_group_total_beta_is_0_0065() {
        let layer = DelayedNeutronLayer::u235_five_group(lambda());
        assert_relative_eq!(
            layer.total_delayed_neutron_fraction().get::<ratio>(),
            0.0065,
            epsilon = 1e-9
        );
    }

    /// Methodology: hold the reactor power at a constant `P` and step the layer
    /// for many precursor time constants. Each group's source `S_i` is a
    /// first-order lag of `(beta_i/Lambda) P`, so at steady state
    /// `S = sum_i S_i -> (beta/Lambda) P` and the per-step delayed increment
    /// settles to `S * dt = (beta/Lambda) P dt`.
    ///
    /// Result (data 2026-07-15): with `P = 30 MW`, `Lambda = 2.31e-4 s`,
    /// `dt = 0.05 s`, stepped to `t = 1000 s` (> 12 x the longest time constant
    /// `tau_1 = 80.8 s`), the delayed increment settles to
    /// `(0.0065/2.31e-4) * 30 * 0.05 = 42.21 MW` within `< 0.1 %`.
    #[test]
    fn constant_power_source_settles() {
        let mut layer = DelayedNeutronLayer::u235_five_group(lambda());
        let p = Power::new::<megawatt>(30.0);
        let dt = Time::new::<second>(0.05);

        let mut inc = Power::ZERO;
        for _ in 0..20_000 {
            inc = layer.advance(p, dt);
        }

        let expected = (0.0065 / 2.31e-4) * 30.0 * 0.05;
        assert_relative_eq!(inc.get::<megawatt>(), expected, max_relative = 1e-3);
    }

    /// Methodology: after a step increase in power, the delayed source must
    /// *lag* — it cannot instantly reach its new steady value because the
    /// precursors take time to build up. Confirm the increment right after a
    /// power step is well below its eventual steady value.
    ///
    /// Result (data 2026-07-15): power stepped 0 -> 30 MW; one `dt = 0.05 s`
    /// after the step the delayed increment is a small fraction of the steady
    /// `42.21 MW`, confirming the reservoir builds up gradually (the inertia
    /// that damps the feedback loop).
    #[test]
    fn power_step_source_lags() {
        let mut layer = DelayedNeutronLayer::u235_five_group(lambda());
        let p = Power::new::<megawatt>(30.0);
        let dt = Time::new::<second>(0.05);

        let first = layer.advance(p, dt);
        let steady = (0.0065 / 2.31e-4) * 30.0 * 0.05;
        assert!(
            first.get::<megawatt>() < 0.5 * steady,
            "one step after a power step the delayed source {} MW should still \
             be well below its steady value {steady} MW",
            first.get::<megawatt>()
        );
    }

    /// Rejects a non-positive decay constant.
    #[test]
    fn rejects_non_positive_decay_constant() {
        let groups = [
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(0.01)),
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(0.03)),
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(0.1)),
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(0.3)),
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(0.0)),
        ];
        assert!(matches!(
            DelayedNeutronLayer::new(lambda(), groups),
            Err(TehOPrkeError::NonPositiveDelayedDecayConstant(_))
        ));
    }

    /// Rejects a non-positive prompt generation time.
    #[test]
    fn rejects_non_positive_lambda() {
        let groups = [
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(0.01)),
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(0.03)),
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(0.1)),
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(0.3)),
            (Ratio::new::<ratio>(0.001), Frequency::new::<hertz>(1.6)),
        ];
        assert!(matches!(
            DelayedNeutronLayer::new(Time::new::<second>(0.0), groups),
            Err(TehOPrkeError::NonPositivePromptNeutronGenerationTime(_))
        ));
    }
}
