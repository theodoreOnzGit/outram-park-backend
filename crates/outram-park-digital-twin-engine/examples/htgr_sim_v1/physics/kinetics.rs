//! Reactor-kinetics slot for the HTGR scaffold.
//!
//! Composes two pieces, matching the recommended OUTRAM PARK kinetics
//! architecture (see `teh_o_prke::nordheim_fuchs`'s module doc):
//!
//! 1. **Prompt Excursion Layer** -- the *real*, already-validated
//!    [`teh_o_prke::nordheim_fuchs::NordheimFuchsExactTimestepper`]
//!    (re-exported as [`nee_soon::NordheimFuchsExactTimestepper`]). Closed-form
//!    prompt power + adiabatic fuel-temperature feedback. This is production
//!    code from the workspace, not a placeholder.
//!
//! 2. **Delayed-neutron precursor bank** -- a bank of first-order lags of the
//!    prompt power, one per delayed-neutron group, so the kinetics is **not
//!    prompt-only**. This deliberately avoids repeating `fhr_sim_v2`'s
//!    prompt-only oscillation mistake by construction: the delayed groups
//!    smooth the reactor's response with the precursor decay-constant time
//!    scales (seconds), not the prompt generation time (milliseconds).
//!
//! ## Status of the delayed layer -- PLACEHOLDER
//!
//! A parallel effort is adding a real `DelayedNeutronLayer` to `teh-o-prke`
//! (five first-order transfer functions built on
//! `chem-eng-real-time-process-control-simulator`'s `TransferFnFirstOrder`).
//! Until it lands, this module ships [`PlaceholderDelayedNeutronLayer`], a
//! local stand-in coded against the interface this scaffold *assumes* that
//! type will expose:
//!
//! ```text
//! // ASSUMED teh_o_prke::DelayedNeutronLayer interface (reconcile when it lands):
//! DelayedNeutronLayer::from_nuclide(nuclide: FissioningNuclideType,
//!                                   prompt_neutron_generation_time: Time)
//!     -> Result<Self, TehOPrkeError>;
//! DelayedNeutronLayer::new(beta_i: &[Ratio], lambda_i: &[Frequency])
//!     -> Result<Self, TehOPrkeError>;
//! fn step(&mut self, prompt_power: Power, time: Time) -> Result<Power, TehOPrkeError>;
//! fn delayed_power(&self) -> Power;             // total delayed-neutron power contribution
//! fn total_delayed_fraction(&self) -> Ratio;    // sum of beta_i
//! ```
//!
//! The placeholder reuses `teh-o-prke`'s real U-235 six-group delayed-fraction
//! and decay-constant tables and drives one `TransferFnFirstOrder` per group.
//! The assumed real layer may use five groups; that is a reconciliation
//! detail, not a structural one -- see bead `op-wqk.9.2`.

use chem_eng_real_time_process_control_simulator::beta_testing::transfer_fn_wrapper_and_enums::{
    TransferFnFirstOrder, TransferFnTraits,
};
use nee_soon::NordheimFuchsExactTimestepper;
use teh_o_prke::zero_power_prke::six_group_precursor_prke::six_group_constants::{
    new_decay_constant_array, new_u235_delayed_neutron_fraction_array,
};

use uom::si::f64::{Power, Ratio, Time};
use uom::si::power::{megawatt, watt};
use uom::si::ratio::ratio;
use uom::si::time::second;

/// A single delayed-neutron precursor group modelled as a first-order lag of
/// the prompt power.
///
/// Physically, precursor group `i` obeys
/// `dC_i/dt = (beta_i/beta) * P - lambda_i * C_i`; its delayed-neutron power
/// contribution therefore tracks the prompt power `P` through a first-order
/// lag of gain `beta_i/beta` and time constant `tau_i = 1/lambda_i`. That is
/// exactly what [`TransferFnFirstOrder`] represents, which is why the assumed
/// real `DelayedNeutronLayer` is described as "N first-order transfer
/// functions".
struct PrecursorGroupLag {
    /// The first-order lag `(beta_i/beta) / (tau_i s + 1)`, driven by the
    /// normalised prompt power (see [`PlaceholderDelayedNeutronLayer::step`]).
    lag: TransferFnFirstOrder,
    /// Fractional gain `beta_i / beta` of this group's contribution.
    fractional_gain: f64,
}

/// **Placeholder** delayed-neutron precursor bank -- a stand-in for the
/// forthcoming `teh_o_prke::DelayedNeutronLayer` (see the module doc for the
/// assumed interface and bead `op-wqk.9.2`).
///
/// Drives one [`TransferFnFirstOrder`] per delayed group with the normalised
/// prompt power, then sums the group contributions to a total delayed-neutron
/// power. Real data (U-235 six-group `beta_i` / `lambda_i`) is reused from
/// `teh-o-prke`; the *coupling* to the prompt layer is scaffold-level.
pub struct PlaceholderDelayedNeutronLayer {
    groups: Vec<PrecursorGroupLag>,
    /// Reference power the lag input/output are normalised against, so the
    /// dimensionless `TransferFnFirstOrder` carries `P/P_ref`.
    reference_power: Power,
    /// Total delayed-neutron fraction `beta = sum(beta_i)`.
    total_beta: f64,
    /// Wall-clock-independent simulation time handed to the transfer functions.
    time: Time,
    /// Most recently computed total delayed-neutron power contribution.
    delayed_power: Power,
}

impl PlaceholderDelayedNeutronLayer {
    /// Build the six-group U-235 precursor bank, normalised against
    /// `reference_power` (the plant's nominal thermal power).
    ///
    /// # Panics
    /// If `reference_power <= 0` (the normalisation denominator), or if any
    /// group's [`TransferFnFirstOrder`] rejects its time constant.
    pub fn new_u235_six_group(reference_power: Power) -> Self {
        assert!(
            reference_power.get::<watt>() > 0.0,
            "PlaceholderDelayedNeutronLayer: reference_power must be > 0 W"
        );

        let beta_i = new_u235_delayed_neutron_fraction_array();
        let lambda_i = new_decay_constant_array();
        let total_beta: f64 = beta_i.iter().map(|b| b.get::<ratio>()).sum();

        let groups = beta_i
            .iter()
            .zip(lambda_i.iter())
            .map(|(beta, lambda)| {
                // tau_i = 1 / lambda_i.
                let tau = Time::new::<second>(1.0 / lambda.value);
                let fractional_gain = beta.get::<ratio>() / total_beta;
                // Unity-form first-order lag with time constant tau: the
                // fractional gain is applied outside, so the lag itself is
                // unity gain -- new(a1=0, b1=1, a2=tau, b2=1).
                let lag = TransferFnFirstOrder::new(
                    Time::new::<second>(0.0),
                    Ratio::new::<ratio>(1.0),
                    tau,
                    Ratio::new::<ratio>(1.0),
                )
                .expect("unity-gain first-order lag with tau>0 is always valid");
                PrecursorGroupLag {
                    lag,
                    fractional_gain,
                }
            })
            .collect();

        Self {
            groups,
            reference_power,
            total_beta,
            time: Time::new::<second>(0.0),
            delayed_power: Power::new::<watt>(0.0),
        }
    }

    /// Advance every precursor group's lag by `dt`, driven by the current
    /// `prompt_power`, and recompute the total delayed-neutron power.
    ///
    /// Mirrors the assumed `DelayedNeutronLayer::step(prompt_power, time)`
    /// signature (returns the total delayed-neutron power contribution).
    pub fn step(&mut self, prompt_power: Power, dt: Time) -> Power {
        self.time += dt;
        let normalised_input =
            Ratio::new::<ratio>(prompt_power.get::<watt>() / self.reference_power.get::<watt>());

        let mut total_normalised = 0.0_f64;
        for group in &mut self.groups {
            // set_user_input_and_calc carries a dimensionless scalar (P/P_ref).
            let out = group
                .lag
                .set_user_input_and_calc(normalised_input, self.time)
                .expect("first-order lag evaluation should not fail for finite input");
            total_normalised += group.fractional_gain * out.get::<ratio>();
        }

        self.delayed_power =
            Power::new::<watt>(total_normalised * self.reference_power.get::<watt>());
        self.delayed_power
    }

    /// The most recently computed total delayed-neutron power contribution.
    pub fn delayed_power(&self) -> Power {
        self.delayed_power
    }

    /// Total delayed-neutron fraction `beta = sum(beta_i)` (dimensionless).
    pub fn total_delayed_fraction(&self) -> Ratio {
        Ratio::new::<ratio>(self.total_beta)
    }
}

/// The HTGR kinetics slot: prompt excursion layer + delayed-neutron bank.
///
/// The prompt layer is real (`teh-o-prke`); the delayed layer is the
/// placeholder above until `teh_o_prke::DelayedNeutronLayer` lands. Reactivity
/// is driven in **dollars** (reactivity divided by `beta`) from the GUI and
/// converted to the prompt layer's dimensionless `rho_ext = dollars * beta`.
pub struct HtgrKinetics {
    /// Real Nordheim-Fuchs prompt-excursion timestepper.
    pub prompt: NordheimFuchsExactTimestepper,
    /// Placeholder delayed-neutron precursor bank.
    pub delayed: PlaceholderDelayedNeutronLayer,
    /// Total reactor power = prompt + delayed (see [`Self::total_power`]).
    total_power: Power,
}

impl HtgrKinetics {
    /// Construct the kinetics slot with illustrative graphite-moderated HTGR
    /// parameters. **Not** any specific licensed design -- round,
    /// order-of-magnitude numbers only, per this workspace's data policy.
    ///
    /// - `Lambda = 1e-3 s` (thermal, graphite-moderated: larger prompt
    ///   generation time than a fast system),
    /// - `beta = 0.0065`, `C_f = 1e8 J/K` (whole-core lumped),
    /// - `alpha_f = -4e-5 K^-1` (negative fuel-temperature feedback),
    /// - reference/initial fuel temperature 900 K,
    /// - `reference_power` seeds both the initial prompt power and the delayed
    ///   bank's normalisation.
    pub fn new_illustrative(reference_power: Power) -> Self {
        use uom::si::f64::{HeatCapacity, TemperatureCoefficient, ThermodynamicTemperature};
        use uom::si::heat_capacity::joule_per_kelvin;
        use uom::si::temperature_coefficient::per_kelvin;
        use uom::si::thermodynamic_temperature::kelvin;

        let prompt = NordheimFuchsExactTimestepper::new(
            Time::new::<second>(1.0e-3),
            Ratio::new::<ratio>(0.0065),
            HeatCapacity::new::<joule_per_kelvin>(1.0e8),
            TemperatureCoefficient::new::<per_kelvin>(-4.0e-5),
            ThermodynamicTemperature::new::<kelvin>(900.0),
            ThermodynamicTemperature::new::<kelvin>(900.0),
            reference_power,
        )
        .expect("illustrative HTGR kinetics parameters must satisfy NordheimFuchs preconditions");

        let delayed = PlaceholderDelayedNeutronLayer::new_u235_six_group(reference_power);

        Self {
            prompt,
            delayed,
            total_power: reference_power,
        }
    }

    /// Advance the kinetics by one timestep.
    ///
    /// `external_reactivity_dollars` is the user-commanded reactivity in
    /// dollars (`rho/beta`); it is converted to the prompt layer's
    /// dimensionless `rho_ext = dollars * beta` and held constant over the
    /// step.
    ///
    /// The delayed layer is then advanced with the resulting prompt power and
    /// the total power is recomputed.
    pub fn step(&mut self, dt: Time, external_reactivity_dollars: f64) {
        let beta = self.prompt.delayed_neutron_fraction.get::<ratio>();
        self.prompt
            .set_external_reactivity(Ratio::new::<ratio>(external_reactivity_dollars * beta));
        self.prompt.step(dt);

        let delayed_power = self.delayed.step(self.prompt.power, dt);

        // TODO(op-dt3, op-wqk.9.2): scaffold-level combination. The real
        // DelayedNeutronLayer + a proper PRKE coupling should replace this
        // additive prompt+delayed sum; here the two are summed only to give a
        // single illustrative "total power" trace that is visibly NOT
        // prompt-only.
        self.total_power = self.prompt.power + delayed_power;
    }

    /// Prompt-excursion-layer power (the Nordheim-Fuchs power).
    pub fn prompt_power(&self) -> Power {
        self.prompt.power
    }

    /// Delayed-neutron layer's tracked lagged power contribution.
    pub fn delayed_power(&self) -> Power {
        self.delayed.delayed_power()
    }

    /// Total reactor power (prompt + delayed), in the scaffold combination.
    pub fn total_power(&self) -> Power {
        self.total_power
    }

    /// Effective total delayed-neutron fraction `beta = sum(beta_i)` reported
    /// by the delayed-neutron layer (dimensionless).
    pub fn delayed_neutron_fraction(&self) -> Ratio {
        self.delayed.total_delayed_fraction()
    }

    /// Current reactivity margin `r = rho_ext - beta + alpha_f*(T_f - T_ref)`
    /// expressed in **dollars** (`r/beta`), for display.
    pub fn reactivity_margin_dollars(&self) -> f64 {
        let beta = self.prompt.delayed_neutron_fraction.get::<ratio>();
        self.prompt.reactivity_margin().get::<ratio>() / beta
    }
}

/// Convenience: total power in megawatts (for snapshot scalars / plots).
pub fn power_in_megawatts(p: Power) -> f64 {
    p.get::<megawatt>()
}
