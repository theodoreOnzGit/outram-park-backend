//! Reactor-kinetics slot for the HTGR scaffold.
//!
//! Composes two **real** workspace pieces, matching the recommended OUTRAM PARK
//! kinetics architecture (see `teh_o_prke::nordheim_fuchs`'s module doc):
//!
//! 1. **Prompt Excursion Layer** --
//!    [`teh_o_prke::nordheim_fuchs::NordheimFuchsExactTimestepper`]
//!    (re-exported as [`nee_soon::NordheimFuchsExactTimestepper`]). Closed-form
//!    prompt power + adiabatic fuel-temperature feedback.
//! 2. **Delayed-neutron precursor bank** --
//!    [`teh_o_prke::delayed_neutron_layer::DelayedNeutronLayer`], the reduced
//!    five-group U-235 precursor bank (five first-order transfer functions).
//!    This supplies the precursor inertia that a prompt-only model omits, so
//!    the kinetics is **not prompt-only** -- deliberately avoiding
//!    `fhr_sim_v2`'s prompt-only oscillation mistake by construction.
//!
//! ## Lie-split coupling (the important part)
//!
//! Each timestep the two layers are combined with an operator (Lie) split, per
//! the `DelayedNeutronLayer` coupling recipe:
//!
//! 1. Advance the prompt model -> prompt power `P_p`.
//! 2. `let increment = delayed.advance(P_p, dt);` -- the delayed power
//!    increment `S*dt` (`S = sum_i lambda_i C_i` is the delayed-neutron
//!    source).
//! 3. Total power `P = P_p + increment`, fed **back** into the prompt model
//!    (`prompt.power = P`) so the next step's prompt dynamics and the fuel
//!    heating both see the full point-kinetics power, not the prompt part
//!    alone. That feedback is the precursor inertia that damps the
//!    fuel-temperature feedback loop.
//!
//! ## Nodalisation
//!
//! **One node.** This is point kinetics: the whole core is a single amplitude,
//! with one lumped fuel temperature behind the feedback. There is no spatial
//! flux shape, so control-rod worth cannot depend on rod position or on which
//! of the ten side-reflector rods moves, and there is no way to represent a
//! local power peak or the axial power shape of a pebble bed. The obvious
//! refinement is a coarse axial nodal-diffusion solve, which is a different
//! crate's job (`bedok`), not this simulator's.
//!
//! **The fuel-temperature feedback is a separate node from
//! [`super::pebble_bed`], and the two are not coupled.** The prompt layer keeps
//! its own adiabatic fuel temperature for reactivity feedback, while the pebble
//! bed keeps the graphite temperature that the helium actually sees. They are
//! sized consistently -- the prompt layer's heat capacity is *taken from* the
//! pebble bed's graphite mass -- but they are separate states and will disagree
//! during a transient. Making the feedback read the pebble-bed temperature is
//! the natural next step and is deliberately not done here.
//!
//! This slot is wired to the real `teh-o-prke` API (bead `op-wqk.9.2`). What
//! remains scaffold-level is only the *plant-scale illustrative parameters*
//! below, not the kinetics wiring.

use nee_soon::NordheimFuchsExactTimestepper;
use teh_o_prke::delayed_neutron_layer::DelayedNeutronLayer;

use uom::si::f64::{Power, Ratio, Time};
use uom::si::power::{megawatt, watt};
use uom::si::ratio::ratio;
use uom::si::time::second;

/// The HTGR kinetics slot: prompt excursion layer + five-group delayed-neutron
/// bank, coupled by a Lie split (see the module doc).
///
/// Reactivity is driven in **dollars** (`rho/beta`) from the GUI and converted
/// to the prompt layer's dimensionless `rho_ext = dollars * beta`.
#[derive(Clone)]
pub struct HtgrKinetics {
    /// Nordheim-Fuchs prompt-excursion timestepper.
    pub prompt: NordheimFuchsExactTimestepper,
    /// Five-group U-235 delayed-neutron precursor bank.
    pub delayed: DelayedNeutronLayer,
    /// Prompt-layer power `P_p` from the most recent step (before the delayed
    /// increment is fed back), kept for display.
    prompt_power: Power,
    /// Delayed power increment `S*dt` from the most recent step, kept for
    /// display.
    delayed_increment: Power,
    /// Total reactor power `P = P_p + increment` from the most recent step.
    total_power: Power,
}

impl HtgrKinetics {
    /// Construct the kinetics slot with illustrative graphite-moderated
    /// pebble-bed parameters. **Not** any specific licensed design -- round,
    /// order-of-magnitude numbers only, per this workspace's data policy.
    ///
    /// - `Lambda = 1e-3 s` (thermal, graphite-moderated: larger prompt
    ///   generation time than a fast system) -- illustrative,
    /// - `beta = 0.0065` -- illustrative,
    /// - `C_f` = the pebble bed's own lumped graphite heat capacity,
    ///   [`super::pebble_bed::bed_heat_capacity`] (about 9.0 MJ/K). This is
    ///   *derived* from the published pebble count, diameter and graphite
    ///   density, so the feedback sees the same thermal mass the thermal
    ///   hydraulics does. It used to be a flat 1e8 J/K sized for the old
    ///   200 MWth prismatic plant, which at 10 MWth would have made the
    ///   temperature feedback almost inert.
    /// - `alpha_f = -4e-5 K^-1` (negative fuel-temperature feedback) --
    ///   illustrative,
    /// - reference/initial fuel temperature 900 K -- illustrative,
    /// - `reference_power` seeds the initial prompt power.
    ///
    /// The delayed bank is built with the **same** `Lambda`, so its per-group
    /// source gains `beta_i/Lambda` are consistent with the prompt layer.
    pub fn new_illustrative(reference_power: Power) -> Self {
        use uom::si::f64::{TemperatureCoefficient, ThermodynamicTemperature};
        use uom::si::temperature_coefficient::per_kelvin;
        use uom::si::thermodynamic_temperature::kelvin;

        let prompt_generation_time = Time::new::<second>(1.0e-3);

        let prompt = NordheimFuchsExactTimestepper::new(
            prompt_generation_time,
            Ratio::new::<ratio>(0.0065),
            super::pebble_bed::bed_heat_capacity(),
            TemperatureCoefficient::new::<per_kelvin>(-4.0e-5),
            ThermodynamicTemperature::new::<kelvin>(900.0),
            ThermodynamicTemperature::new::<kelvin>(900.0),
            reference_power,
        )
        .expect("illustrative HTGR kinetics parameters must satisfy NordheimFuchs preconditions");

        let delayed = DelayedNeutronLayer::u235_five_group(prompt_generation_time);

        Self {
            prompt,
            delayed,
            prompt_power: reference_power,
            delayed_increment: Power::new::<watt>(0.0),
            total_power: reference_power,
        }
    }

    /// Advance the kinetics by one timestep with the Lie-split coupling.
    ///
    /// `external_reactivity_dollars` is the user-commanded reactivity in
    /// dollars (`rho/beta`); it is converted to the prompt layer's
    /// dimensionless `rho_ext = dollars * beta` and held constant over the
    /// step.
    ///
    /// # This is a multi-rate sub-model, like the steam generator
    ///
    /// `dt` is subdivided into whole pieces no longer than
    /// [`super::KINETICS_SUBSTEP_S`], and [`Self::advance_one`] is run on each.
    /// **This is not decoration -- it is the difference between a right and a
    /// wrong answer at the plant timestep.** The prompt layer relaxes on
    /// `Lambda / beta = 1e-3 / 0.0065 = 0.154 s`, which is the *only* timescale
    /// in this plant comparable to [`super::PLANT_TIMESTEP_S`] = 0.1 s. The
    /// Lie split between the prompt layer and the precursor bank is first-order
    /// accurate in the step, so at `dt / tau = 0.65` it is badly resolved.
    ///
    /// Measured 2026-08-13 on the flow-ramp transient of
    /// `super::tests::the_plant_outer_correctors_converge`, which drives the
    /// reactor deeply subcritical through its own temperature feedback -- the
    /// hardest case for this split, because the power is decaying fast:
    ///
    /// | Kinetics substep | Reactor power at 60 s | vs the 1 ms reference |
    /// |---|---|---|
    /// | 0.1 s (none -- one piece per plant step) | 0.02703 MW | **-84.5%** |
    /// | 0.01 s | 0.14071 MW | **-19.3%** |
    /// | **0.001 s (shipped)** | **0.17428 MW** | **+0.0000%** |
    ///
    /// The exact agreement in the last row is structural rather than a
    /// convergence result -- both runs then integrate the kinetics at 1 ms, and
    /// the kinetics is decoupled from everything the plant timestep governs.
    /// See [`super::KINETICS_SUBSTEP_S`].
    ///
    /// The plant's outer correctors cannot fix this, because the kinetics is
    /// **not coupled** to anything the corrector loop iterates (see
    /// [`super::HtgrPlant::step`]) -- it depends only on the commanded
    /// reactivity and its own adiabatic fuel temperature. Sub-stepping is the
    /// only remedy, and it is nearly free: one `atanh` and five first-order
    /// transfer-function updates per substep, against three coupled array
    /// solves for the steam generator.
    ///
    /// Subdividing to a **maximum** substep rather than a fixed count means a
    /// caller already stepping finer than [`super::KINETICS_SUBSTEP_S`] -- the
    /// 1 ms reference leg of the accuracy test, for instance -- pays nothing
    /// extra.
    pub fn step(&mut self, dt: Time, external_reactivity_dollars: f64) {
        let dt_s = dt.get::<second>();
        let pieces = if dt_s > super::KINETICS_SUBSTEP_S {
            (dt_s / super::KINETICS_SUBSTEP_S).ceil().max(1.0)
        } else {
            1.0
        };
        let sub = Time::new::<second>(dt_s / pieces);
        for _ in 0..(pieces as usize) {
            self.advance_one(sub, external_reactivity_dollars);
        }
    }

    /// One Lie-split kinetics substep. See [`Self::step`], which is the entry
    /// point callers should use -- it subdivides `dt` for accuracy.
    fn advance_one(&mut self, dt: Time, external_reactivity_dollars: f64) {
        let beta = self.prompt.delayed_neutron_fraction.get::<ratio>();
        self.prompt
            .set_external_reactivity(Ratio::new::<ratio>(external_reactivity_dollars * beta));

        // 1. Prompt substep -> P_p.
        self.prompt.step(dt);
        let prompt_power = self.prompt.power;

        // 2. Delayed substep -> increment S*dt.
        let increment = self.delayed.advance(prompt_power, dt);

        // 3. Total power, fed back into the prompt model so the next step's
        //    dynamics and the fuel heating see the full point-kinetics power.
        let total = prompt_power + increment;
        self.prompt.power = total;

        self.prompt_power = prompt_power;
        self.delayed_increment = increment;
        self.total_power = total;
    }

    /// Prompt-excursion-layer power `P_p` (before the delayed increment is
    /// added back).
    pub fn prompt_power(&self) -> Power {
        self.prompt_power
    }

    /// Delayed-neutron power increment `S*dt` from the most recent step.
    pub fn delayed_power(&self) -> Power {
        self.delayed_increment
    }

    /// Total reactor power `P = P_p + increment`.
    pub fn total_power(&self) -> Power {
        self.total_power
    }

    /// Effective total delayed-neutron fraction `beta = sum(beta_i)` reported
    /// by the delayed-neutron layer (dimensionless).
    pub fn delayed_neutron_fraction(&self) -> Ratio {
        self.delayed.total_delayed_neutron_fraction()
    }

    /// Current reactivity margin `r = rho_ext - beta + alpha_f*(T_f - T_ref)`
    /// expressed in **dollars** (`r/beta`), for display.
    pub fn reactivity_margin_dollars(&self) -> f64 {
        let beta = self.prompt.delayed_neutron_fraction.get::<ratio>();
        self.prompt.reactivity_margin().get::<ratio>() / beta
    }
}

/// Convenience: power in megawatts (for snapshot scalars / plots).
pub fn power_in_megawatts(p: Power) -> f64 {
    p.get::<megawatt>()
}
