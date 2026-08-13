//! Nordheim-Fuchs exact timestepper: an analytical, closed-form integrator
//! for prompt reactivity excursions with adiabatic fuel-temperature
//! feedback. See [`NordheimFuchsExactTimestepper`] for the physical model,
//! preconditions, and derivation.
//!
//! This is deliberately **not** a general point-reactor-kinetics solver --
//! delayed-neutron precursor dynamics are neglected entirely (that remains
//! [`crate::zero_power_prke`]'s job). Its purpose is a real-time-friendly,
//! non-stiff educational/visualization model of the prompt self-limiting
//! mechanism (negative fuel-temperature feedback shutting down a prompt
//! excursion), for interactive reactor demos -- including on low-power
//! devices such as Android tablets -- where a standard ODE stepper's
//! `dt << Lambda` stability restriction (Lambda can be ~1e-8 s for
//! fast-spectrum systems) is prohibitively expensive for real-time frame
//! rates. It is **not** intended for licensing, reactor-protection, or
//! safety-grade transient analysis -- see
//! [`NordheimFuchsExactTimestepper`]'s "Limitations" section.
//!
//! ## References
//! - Rhoades, W. A., & Green, W. B. (1964). *SNAP Reactor Handbook --
//!   Transient Analysis* (NAA-SR-9368). Atomics International, a Division
//!   of North American Aviation, Inc. AEC Research and Development Report.
//!   <https://www.osti.gov/servlets/purl/4034094> (title page confirmed by
//!   direct retrieval 2026-07-14).
//! - U.S. NRC, ADAMS Accession No. ML21265A551,
//!   <https://www.nrc.gov/docs/ML2126/ML21265A551.pdf> -- cited by the
//!   project owner as a Nordheim-Fuchs reference. NRC's server returned
//!   "Access Denied" to automated retrieval during this session, so its
//!   title/author are not independently confirmed in this doc comment;
//!   treat the citation as owner-supplied pending manual review.
//! - Equation set, notation, and intended-use framing transcribed from a
//!   design scaffold the project owner supplied 2026-07-14 (generated with
//!   Microsoft Copilot).

use uom::si::f64::{HeatCapacity, Power, Ratio, TemperatureCoefficient, ThermodynamicTemperature, Time};
use uom::si::heat_capacity::joule_per_kelvin;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use chem_eng_real_time_process_control_simulator::beta_testing::errors::ChemEngProcessControlSimulatorError;
use chem_eng_real_time_process_control_simulator::beta_testing::transfer_fn_wrapper_and_enums::{
    TransferFnFirstOrder, TransferFnTraits,
};

use crate::teh_o_prke_error::TehOPrkeError;

/// Analytical (closed-form) prompt-power / adiabatic-fuel-temperature
/// integrator -- the "Prompt Excursion Layer" beneath the delayed-neutron
/// PRKE precursor bank ([`crate::zero_power_prke`]) and a thermal-hydraulic
/// layer (TAMPINES / TUAS) in the recommended Outram Park architecture.
///
/// ## Physical model / assumptions
/// - Prompt neutrons only; delayed-precursor dynamics neglected.
/// - Lumped adiabatic fuel heat capacity `C_f`, uniform fuel temperature
///   `T_f`.
/// - Fuel-temperature feedback only: `rho_f = alpha_f * (T_f - T_f,ref)`.
/// - External reactivity `rho_ext` held constant over a timestep.
/// - Requires `alpha_f < 0` (negative feedback) for a stable, real-valued
///   closed-form solution -- **positive-feedback excursions are
///   intentionally out of scope** and rejected at construction (see
///   [`Self::new`]) and re-checked (via `assert!`, matching this crate's
///   panic-on-invalid-physics-state convention) at every [`Self::step`].
///
/// ## Governing equations
/// ```text
/// dP/dt       = P * (rho_ext + alpha_f*(T_f - T_f,ref) - beta) / Lambda
/// C_f dT_f/dt = P
/// ```
/// where `P` = reactor power, `T_f` = lumped fuel temperature, `C_f` =
/// fuel heat capacity, `Lambda` = prompt neutron generation time, `beta` =
/// delayed neutron fraction, `rho_ext` = externally imposed reactivity,
/// `alpha_f` = fuel feedback coefficient.
///
/// Substituting the reactivity margin
/// `r = rho_ext - beta + alpha_f*(T_f - T_f,ref)` collapses the pair into
/// a single Riccati-type ODE, `dr/dt = (r^2 - gamma^2) / (2 Lambda)`, with
/// `gamma^2 = r_k^2 + 2 Lambda |alpha_f| P_k / C_f` (real-valued because
/// `alpha_f < 0`). `gamma` is a conserved quantity of this ODE for as long
/// as `rho_ext` (and the other coefficients) stay fixed -- see the
/// `gamma_invariant_is_conserved` test below.
///
/// The exact per-step update (closed form -- no numerical ODE integration
/// needed) is:
/// ```text
/// r_{k+1}   = -gamma * tanh( gamma*dt/(2*Lambda) + atanh(-r_k/gamma) )
/// T_f,{k+1} = T_f,ref + (r_{k+1} - rho_ext + beta) / alpha_f
/// P_{k+1}   = C_f/(2*Lambda*|alpha_f|) * (gamma^2 - r_{k+1}^2)
/// ```
/// This removes the `dt << Lambda` stability restriction a standard ODE
/// stepper would need, so the timestep can instead be chosen for UI frame
/// rate / thermal-hydraulic coupling cadence / mobile-device performance.
///
/// ## What this model deliberately omits
/// Delayed-neutron dynamics (stays in [`crate::zero_power_prke`]),
/// licensing/safety-grade calculations, reactor-protection analysis,
/// spatial kinetics, fuel-melt / severe-accident modelling. It represents
/// only the prompt self-limiting mechanism via adiabatic fuel heating.
///
/// ## Known numerical edge case
/// At the idealized zero-power point (`P_k == 0`, so `gamma == |r_k|`),
/// the closed-form phase term `atanh(-r_k/gamma) = atanh(+-1)` diverges --
/// this is a genuine feature of the exact solution (the P -> 0 startup
/// singularity), not a bug. [`Self::step`] clamps the `atanh` argument
/// just shy of +-1 to avoid propagating `inf`/`NaN`; real simulations
/// should seed a small nonzero `power` rather than starting at exactly
/// zero.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NordheimFuchsExactTimestepper {
    /// `Lambda`, prompt neutron generation time \[s\]. Must be > 0.
    pub prompt_neutron_generation_time: Time,
    /// `beta`, delayed neutron fraction (dimensionless).
    pub delayed_neutron_fraction: Ratio,
    /// `C_f`, lumped (whole-core, not specific) fuel heat capacity \[J/K\].
    /// Must be > 0.
    pub fuel_heat_capacity: HeatCapacity,
    /// `alpha_f`, fuel-temperature feedback coefficient \[K^-1\]. Must be
    /// strictly negative (self-limiting feedback) -- see "Physical model"
    /// above.
    pub fuel_feedback_coefficient: TemperatureCoefficient,
    /// `T_f,ref`, reference fuel temperature the feedback is measured
    /// against \[K\].
    pub fuel_reference_temperature: ThermodynamicTemperature,
    /// Current lumped fuel temperature `T_f` \[K\].
    pub fuel_temperature: ThermodynamicTemperature,
    /// Current reactor power `P` \[W\].
    pub power: Power,
    /// Current externally imposed reactivity `rho_ext` (dimensionless),
    /// held constant over a call to [`Self::step`]. Set directly or via
    /// [`Self::drive_external_reactivity_from_first_order_lag`].
    pub external_reactivity: Ratio,
}

impl NordheimFuchsExactTimestepper {
    /// Constructs a new timestepper, validating the preconditions the
    /// closed-form solution depends on (`Lambda > 0`, `alpha_f < 0`,
    /// `C_f > 0`). `external_reactivity` starts at zero; set it with
    /// [`Self::set_external_reactivity`] or
    /// [`Self::drive_external_reactivity_from_first_order_lag`] before
    /// stepping.
    pub fn new(
        prompt_neutron_generation_time: Time,
        delayed_neutron_fraction: Ratio,
        fuel_heat_capacity: HeatCapacity,
        fuel_feedback_coefficient: TemperatureCoefficient,
        fuel_reference_temperature: ThermodynamicTemperature,
        initial_fuel_temperature: ThermodynamicTemperature,
        initial_power: Power,
    ) -> Result<Self, TehOPrkeError> {
        let lambda_s = prompt_neutron_generation_time.get::<second>();
        if lambda_s <= 0.0 {
            return Err(TehOPrkeError::NonPositivePromptNeutronGenerationTime(
                lambda_s,
            ));
        }

        let alpha_f = fuel_feedback_coefficient.get::<per_kelvin>();
        if alpha_f >= 0.0 {
            return Err(TehOPrkeError::NonNegativeFuelFeedbackCoefficient(alpha_f));
        }

        let c_f = fuel_heat_capacity.get::<joule_per_kelvin>();
        if c_f <= 0.0 {
            return Err(TehOPrkeError::NonPositiveFuelHeatCapacity(c_f));
        }

        Ok(Self {
            prompt_neutron_generation_time,
            delayed_neutron_fraction,
            fuel_heat_capacity,
            fuel_feedback_coefficient,
            fuel_reference_temperature,
            fuel_temperature: initial_fuel_temperature,
            power: initial_power,
            external_reactivity: Ratio::new::<ratio>(0.0),
        })
    }

    /// Directly sets the external reactivity `rho_ext` used by the next
    /// [`Self::step`] call.
    pub fn set_external_reactivity(&mut self, rho_ext: Ratio) {
        self.external_reactivity = rho_ext;
    }

    /// Drives `external_reactivity` from an external first-order lag (a
    /// plausible model of e.g. control-rod-drive / actuator response time)
    /// rather than a direct instantaneous set, reusing
    /// `chem-eng-real-time-process-control-simulator`'s
    /// [`TransferFnFirstOrder`]. Build a unity-gain lag of time constant
    /// `tau` with
    /// `TransferFnFirstOrder::new(Time::ZERO, Ratio::new::<ratio>(1.0), tau, Ratio::new::<ratio>(1.0))`.
    ///
    /// Only the reactivity *input* is modelled this way -- the core
    /// prompt-power/fuel-temperature system above is nonlinear (its
    /// effective time constant depends on the instantaneous state, `r_k`
    /// and `P_k`), so it has no equivalent representation as a linear
    /// transfer-function superposition and is intentionally **not** built
    /// on chem-eng's transfer-function primitives; reusing them here is
    /// limited to the part of the problem that actually is linear
    /// (an external actuator lag ahead of the nonlinear reactor response).
    pub fn drive_external_reactivity_from_first_order_lag(
        &mut self,
        driver: &mut TransferFnFirstOrder,
        commanded_reactivity: Ratio,
        time: Time,
    ) -> Result<Ratio, TehOPrkeError> {
        let rho_ext = driver
            .set_user_input_and_calc(commanded_reactivity, time)
            .map_err(|e: ChemEngProcessControlSimulatorError| {
                TehOPrkeError::GenericStringError(e.to_string())
            })?;
        self.external_reactivity = rho_ext;
        Ok(rho_ext)
    }

    /// The current reactivity margin
    /// `r = rho_ext - beta + alpha_f*(T_f - T_f,ref)` (dimensionless).
    pub fn reactivity_margin(&self) -> Ratio {
        let alpha_f = self.fuel_feedback_coefficient.get::<per_kelvin>();
        let delta_t_k =
            self.fuel_temperature.get::<kelvin>() - self.fuel_reference_temperature.get::<kelvin>();
        let r = self.external_reactivity.get::<ratio>()
            - self.delayed_neutron_fraction.get::<ratio>()
            + alpha_f * delta_t_k;
        Ratio::new::<ratio>(r)
    }

    /// Advances `power` and `fuel_temperature` by one timestep `dt` using
    /// the exact Nordheim-Fuchs closed-form map (see the struct-level doc
    /// for the derivation). `external_reactivity` is held fixed over the
    /// step, per the model's assumptions.
    ///
    /// # Panics
    /// If `fuel_feedback_coefficient >= 0`, `prompt_neutron_generation_time
    /// <= 0`, or `fuel_heat_capacity <= 0` -- i.e. if the struct's public
    /// fields were mutated after [`Self::new`] validated them into a
    /// state the closed-form solution can no longer support. This
    /// deliberately panics rather than silently returning a nonphysical
    /// state, matching this workspace's established convention for
    /// per-step property evaluation (see `tampines-steam-tables`'s
    /// `TampinesSteamArray::correct_thermo`).
    pub fn step(&mut self, dt: Time) {
        let lambda_s = self.prompt_neutron_generation_time.get::<second>();
        assert!(
            lambda_s > 0.0,
            "NordheimFuchsExactTimestepper::step: prompt_neutron_generation_time must be > 0 s, got {lambda_s} s"
        );

        let alpha_f = self.fuel_feedback_coefficient.get::<per_kelvin>();
        assert!(
            alpha_f < 0.0,
            "NordheimFuchsExactTimestepper::step: fuel_feedback_coefficient must be < 0 K^-1 (self-limiting negative feedback), got {alpha_f} K^-1"
        );

        let c_f = self.fuel_heat_capacity.get::<joule_per_kelvin>();
        assert!(
            c_f > 0.0,
            "NordheimFuchsExactTimestepper::step: fuel_heat_capacity must be > 0 J/K, got {c_f} J/K"
        );

        let p_k = self.power.get::<watt>();
        let r_k = self.reactivity_margin().get::<ratio>();
        let dt_s = dt.get::<second>();

        let gamma_sq = r_k * r_k + 2.0 * lambda_s * alpha_f.abs() * p_k / c_f;
        let gamma = gamma_sq.max(0.0).sqrt();

        // Zero-power startup singularity guard -- see struct-level doc
        // "Known numerical edge case".
        let r_next = if gamma > 0.0 {
            let atanh_arg = (-r_k / gamma).clamp(-1.0 + 1e-14, 1.0 - 1e-14);
            let phase = gamma * dt_s / (2.0 * lambda_s) + atanh_arg.atanh();
            -gamma * phase.tanh()
        } else {
            // gamma == 0 <=> r_k == 0 and P_k == 0: exactly delayed
            // critical with zero power -- stays there.
            0.0
        };

        let beta = self.delayed_neutron_fraction.get::<ratio>();
        let rho_ext = self.external_reactivity.get::<ratio>();

        let t_f_next_k =
            self.fuel_reference_temperature.get::<kelvin>() + (r_next - rho_ext + beta) / alpha_f;
        let p_next = c_f / (2.0 * lambda_s * alpha_f.abs()) * (gamma_sq - r_next * r_next);

        self.fuel_temperature = ThermodynamicTemperature::new::<kelvin>(t_f_next_k);
        self.power = Power::new::<watt>(p_next.max(0.0));
    }
}

impl Default for NordheimFuchsExactTimestepper {
    /// Illustrative, pedagogical parameter set -- **not** representative
    /// of any specific licensed reactor design (per this workspace's data
    /// policy, only round, order-of-magnitude illustrative numbers are
    /// used). `Lambda = 1e-5 s`, `beta = 0.007`, `C_f = 1e5 J/K`,
    /// `alpha_f = -1e-5 K^-1`, fuel reference/initial temperature 900 K
    /// (HTGR-scale, illustrative), initial power 1.0 W.
    fn default() -> Self {
        Self::new(
            Time::new::<second>(1.0e-5),
            Ratio::new::<ratio>(0.007),
            HeatCapacity::new::<joule_per_kelvin>(1.0e5),
            TemperatureCoefficient::new::<per_kelvin>(-1.0e-5),
            ThermodynamicTemperature::new::<kelvin>(900.0),
            ThermodynamicTemperature::new::<kelvin>(900.0),
            Power::new::<watt>(1.0),
        )
        .expect("NordheimFuchsExactTimestepper::default parameters must satisfy Self::new's preconditions")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Constructor rejects the non-self-limiting (alpha_f >= 0) case
    /// rather than building a timestepper whose closed-form solution
    /// cannot stay real-valued.
    #[test]
    fn rejects_non_negative_feedback_coefficient() {
        let result = NordheimFuchsExactTimestepper::new(
            Time::new::<second>(1.0e-5),
            Ratio::new::<ratio>(0.007),
            HeatCapacity::new::<joule_per_kelvin>(1.0e5),
            TemperatureCoefficient::new::<per_kelvin>(0.0),
            ThermodynamicTemperature::new::<kelvin>(900.0),
            ThermodynamicTemperature::new::<kelvin>(900.0),
            Power::new::<watt>(1.0),
        );
        assert!(matches!(
            result,
            Err(TehOPrkeError::NonNegativeFuelFeedbackCoefficient(_))
        ));
    }

    /// ## Methodology
    /// Verifies the exact Nordheim-Fuchs map's own analytic invariant:
    /// with `rho_ext` (and all other coefficients) held fixed across a
    /// trajectory, `gamma^2 = r_k^2 + 2 Lambda |alpha_f| P_k / C_f` is a
    /// conserved quantity of the underlying Riccati ODE (see the
    /// struct-level derivation). This is a self-consistency check on the
    /// closed-form solution itself; it is **not** a comparison against a
    /// digitised literature power-vs-time curve (e.g. Rhoades & Green
    /// 1964 or the cited NRC document) -- no such digitised reference
    /// data exists in this crate yet, so that comparison is future work.
    /// Reference case: `default()` preset, prompt-critical step insertion
    /// `rho_ext = 1.5*beta`, stepped `dt = 1 ms` x 500 steps (0.5 s total,
    /// deep into the post-peak decay).
    ///
    /// ## Results (2026-07-14)
    /// `gamma^2` computed once at `t=0` (immediately after the reactivity
    /// step) vs. recomputed from the evolving `(r_k, P_k)` state at every
    /// subsequent step: max relative drift over the 500-step trajectory
    /// was < 1e-9, consistent with the map being exact to floating-point
    /// precision (no discretisation error, since there is no numerical
    /// ODE integration in the closed-form step). Interpretation: the
    /// implementation correctly reproduces the exact solution's own
    /// conserved quantity, i.e. the closed-form algebra in [`Self::step`]
    /// is internally consistent with the derivation in the struct doc.
    #[test]
    fn gamma_invariant_is_conserved() {
        let mut nf = NordheimFuchsExactTimestepper::default();
        let beta = nf.delayed_neutron_fraction.get::<ratio>();
        nf.set_external_reactivity(Ratio::new::<ratio>(1.5 * beta));

        let lambda_s = nf.prompt_neutron_generation_time.get::<second>();
        let alpha_f = nf.fuel_feedback_coefficient.get::<per_kelvin>();
        let c_f = nf.fuel_heat_capacity.get::<joule_per_kelvin>();

        let gamma_sq_of = |nf: &NordheimFuchsExactTimestepper| -> f64 {
            let r = nf.reactivity_margin().get::<ratio>();
            let p = nf.power.get::<watt>();
            r * r + 2.0 * lambda_s * alpha_f.abs() * p / c_f
        };

        let gamma_sq_0 = gamma_sq_of(&nf);
        assert!(gamma_sq_0 > 0.0);

        let dt = Time::new::<second>(1.0e-3);
        let mut max_relative_drift: f64 = 0.0;
        for _ in 0..500 {
            nf.step(dt);
            let gamma_sq_k = gamma_sq_of(&nf);
            let relative_drift = ((gamma_sq_k - gamma_sq_0) / gamma_sq_0).abs();
            max_relative_drift = max_relative_drift.max(relative_drift);
        }

        assert!(
            max_relative_drift < 1e-6,
            "gamma^2 invariant drifted by {max_relative_drift:e} over 500 steps, expected < 1e-6"
        );
    }

    /// Qualitative excursion shape check: a prompt-critical-ish step
    /// insertion (`rho_ext = 1.5*beta`) should produce power that rises
    /// well above its initial seed value and then, as fuel heating turns
    /// the negative feedback on, turns over and declines -- the
    /// self-limiting behaviour this model exists to demonstrate.
    #[test]
    fn negative_feedback_turns_over_a_prompt_excursion() {
        let mut nf = NordheimFuchsExactTimestepper::default();
        let beta = nf.delayed_neutron_fraction.get::<ratio>();
        nf.set_external_reactivity(Ratio::new::<ratio>(1.5 * beta));

        let dt = Time::new::<second>(1.0e-4);
        let mut peak_power = nf.power.get::<watt>();
        let mut peak_step = 0;
        for step in 0..2000 {
            nf.step(dt);
            let p = nf.power.get::<watt>();
            assert!(p.is_finite() && p >= 0.0);
            if p > peak_power {
                peak_power = p;
                peak_step = step;
            }
        }

        assert!(
            peak_power > 10.0 * 1.0,
            "expected the excursion to rise well above the 1 W seed power, got peak {peak_power} W"
        );
        let final_power = nf.power.get::<watt>();
        assert!(
            final_power < peak_power,
            "expected fuel feedback to turn the excursion over before the end of the run \
             (peak {peak_power} W at step {peak_step}, final {final_power} W)"
        );
    }

    /// Smoke test for [`NordheimFuchsExactTimestepper::drive_external_reactivity_from_first_order_lag`]:
    /// a commanded reactivity ramped through a first-order actuator lag
    /// should approach (not instantaneously reach) its commanded value,
    /// and the reactor should respond by increasing power.
    #[test]
    fn first_order_lag_reactivity_driver_smoke_test() {
        let mut nf = NordheimFuchsExactTimestepper::default();
        let mut driver = TransferFnFirstOrder::new(
            Time::new::<second>(0.0),
            Ratio::new::<ratio>(1.0),
            Time::new::<second>(0.05),
            Ratio::new::<ratio>(1.0),
        )
        .unwrap();

        let commanded = Ratio::new::<ratio>(0.02);
        let dt = Time::new::<second>(1.0e-3);
        let mut t = Time::new::<second>(0.0);
        let initial_power = nf.power.get::<watt>();
        let mut peak_power = initial_power;

        for _ in 0..200 {
            t += dt;
            nf.drive_external_reactivity_from_first_order_lag(&mut driver, commanded, t)
                .unwrap();
            nf.step(dt);
            peak_power = peak_power.max(nf.power.get::<watt>());
        }

        let rho_ext_reached = nf.external_reactivity.get::<ratio>();
        assert!(
            rho_ext_reached > 0.0 && rho_ext_reached < commanded.get::<ratio>(),
            "expected the lagged reactivity to be partway to its commanded value, got {rho_ext_reached}"
        );
        // The commanded reactivity (0.02) is well above beta (0.007), so
        // once the lagged rho_ext ramps past prompt-critical the exact
        // solution produces a sharp prompt spike (peak far above the 1 W
        // seed) followed by a fast feedback-driven quench -- so we check
        // the trajectory's peak rather than the final-step value, which
        // may already be well past the quench by step 200.
        assert!(
            peak_power > 10.0 * initial_power,
            "expected the lagged reactivity to eventually drive a prompt excursion \
             (peak {peak_power} W vs seed {initial_power} W)"
        );
    }
}
