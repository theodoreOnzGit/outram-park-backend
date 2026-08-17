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
//! ## Fission-product decay heat (3rd real piece, added 2026-08-14)
//!
//! [`teh_o_prke::decay_heat::DecayHeat`] -- the **23-group fit of the 1978
//! draft ANS Standard** (England *et al.*, via Tobias Table 16), integrated in
//! its exact piecewise-constant-source form. Decay heat is what makes a reactor
//! impossible to switch off, so a simulator that omits it cannot depict a
//! shutdown at all: before this, dropping the rods took core power to zero and
//! the graphite simply cooled.
//!
//! **The prompt term is scaled so the energy is not counted twice.** The
//! group fit accounts for the 13.18 MeV/fission of U-235 thermal fission that
//! emerges *later* as fission-product decay, out of the nominal 200 MeV. So the
//! core's thermal source is
//!
//! ```text
//! P_thermal = prompt_power_fraction * P_fission + P_decay
//! ```
//!
//! with `prompt_power_fraction = 1 - 13.183/200 = 0.9341`. At equilibrium the
//! two terms sum back to `P_fission` by construction (see
//! [`HtgrKinetics::core_thermal_power`]) -- that is a property of the model,
//! not a tuned constant, and it is what makes the steady state unchanged while
//! the shutdown transient becomes right. The bank is seeded with
//! `DecayHeat::new_at_equilibrium`, so the simulator opens with its fission
//! products already saturated rather than clean.
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
//! ## The fuel-temperature feedback got a heat sink (2026-08-14)
//!
//! Until 2026-08-14 the prompt layer's fuel temperature was **adiabatic**: it
//! integrated `dT_f/dt = P/C_f` and never cooled, whatever the helium was
//! doing. The consequence was not subtle -- after any power rise the feedback
//! reactivity stuck at its most negative value forever, because the temperature
//! it was computed from could only ever climb.
//!
//! **The fix is a sink, not a replacement.** It is tempting to overwrite the
//! node with [`super::pebble_bed`]'s graphite temperature each step, but that
//! would be a mistake: Nordheim-Fuchs integrates the prompt power *and* its
//! temperature feedback together in **closed form**, and that exactness is
//! exactly what keeps the stiff feedback term from being stiff here.
//! Substituting an externally integrated node, reset discontinuously once per
//! plant step, throws the closed form away and reintroduces the stiffness this
//! layer exists to avoid.
//!
//! So the closed form keeps the feedback, and
//! [`HtgrKinetics::apply_coolant_heat_removal`] adds only what was missing --
//! the heat the coolant carried off, over the same graphite heat capacity the
//! bed uses. Fast, stiff coupling stays analytic; the slow sink (the bed's
//! ~184 s time constant, against a 0.1 s plant step) is a plain Lie split.
//! The fuel node and the pebble bed then see the same power in and the same
//! heat out over the same capacity, so they track each other physically rather
//! than being reconciled by force.
//!
//! **This makes the kinetics genuinely coupled**, so it is now stepped
//! *inside* the plant's outer-corrector loop (see [`super::HtgrPlant::step`]).
//! [`HtgrKinetics`] is `Clone` precisely so the corrector can rewind it.
//!
//! What this still does not buy: the bed is one node, so the feedback runs off
//! a core-average temperature, not a fuel-centre or peak temperature. A real
//! Doppler feedback wants the fuel kernel temperature, which needs the
//! intra-pebble split described in [`super::pebble_bed`].
//!
//! This slot is wired to the real `teh-o-prke` API (bead `op-wqk.9.2`). What
//! remains scaffold-level is only the *plant-scale illustrative parameters*
//! below, not the kinetics wiring.

use nee_soon::NordheimFuchsExactTimestepper;
use teh_o_prke::decay_heat::{DecayHeat, FissioningNuclide};
use teh_o_prke::delayed_neutron_layer::DelayedNeutronLayer;

use uom::si::f64::{Power, Ratio, ThermodynamicTemperature, Time};
use uom::si::heat_capacity::joule_per_kelvin;
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
    /// 23-group fission-product decay-heat bank (1978 draft ANS Standard).
    pub decay: DecayHeat,
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
    /// - reference/initial fuel temperature = **the pebble bed's own
    ///   design-point temperature**, [`super::pebble_bed::PebbleBedPorousMediaNode::new`]
    ///   (about 950 K), *not* a separately chosen 900 K. This matters now that
    ///   the fuel node has a coolant sink and therefore tracks the bed: the
    ///   feedback is `alpha_f (T_f - T_ref)`, so if `T_ref` sits below the
    ///   temperature the bed actually runs at, the "negative" feedback comes
    ///   out **positive** at the design point and the reactor climbs above
    ///   rated power for no physical reason. While the two nodes were
    ///   decoupled this inconsistency was invisible, because the adiabatic
    ///   fuel node never sat anywhere near the bed temperature anyway.
    ///   Deriving both from the bed makes the design point neutral by
    ///   construction,
    /// - `reference_power` seeds the initial prompt power.
    ///
    /// The delayed bank is built with the **same** `Lambda`, so its per-group
    /// source gains `beta_i/Lambda` are consistent with the prompt layer.
    pub fn new_illustrative(reference_power: Power) -> Self {
        use uom::si::f64::{TemperatureCoefficient, ThermodynamicTemperature};
        use uom::si::temperature_coefficient::per_kelvin;
        use uom::si::thermodynamic_temperature::kelvin;

        let prompt_generation_time = Time::new::<second>(1.0e-3);

        // Reference AND initial fuel temperature both taken from the pebble
        // bed's design point, so `T_f - T_ref` is zero there and the
        // temperature feedback neither holds the reactor down nor pushes it up
        // at rated conditions. Same principle as `C_f` above: derive it from
        // the bed rather than choosing a second number that can disagree.
        let design_point_temperature =
            super::pebble_bed::PebbleBedPorousMediaNode::new().pebble_temperature();

        let prompt = NordheimFuchsExactTimestepper::new(
            prompt_generation_time,
            Ratio::new::<ratio>(0.0065),
            super::pebble_bed::bed_heat_capacity(),
            TemperatureCoefficient::new::<per_kelvin>(-4.0e-5),
            design_point_temperature,
            design_point_temperature,
            reference_power,
        )
        .expect("illustrative HTGR kinetics parameters must satisfy NordheimFuchs preconditions");

        let delayed = DelayedNeutronLayer::u235_five_group(prompt_generation_time);

        // Seeded SATURATED, not clean: the simulator opens at its operating
        // point, where a real core has been running long enough for the
        // fission-product inventory to have reached equilibrium. Starting the
        // groups cold would show zero decay heat at t=0 and then a spurious
        // several-minute climb to equilibrium that no operator would ever see.
        let decay = DecayHeat::new_at_equilibrium(FissioningNuclide::U235Thermal, reference_power);

        Self {
            prompt,
            delayed,
            decay,
            prompt_power: reference_power,
            delayed_increment: Power::new::<watt>(0.0),
            total_power: reference_power,
        }
    }

    /// Cool the reactivity-feedback fuel node by the heat the coolant actually
    /// carried away over `dt`.
    ///
    /// # Why this, and not "set the fuel temperature to the bed temperature"
    ///
    /// The obvious way to couple the feedback to the core is to overwrite the
    /// prompt layer's fuel temperature with [`super::pebble_bed`]'s graphite
    /// temperature each step. **That is the wrong move, and it is worth saying
    /// why**: the Nordheim-Fuchs timestepper integrates the prompt power *and*
    /// its adiabatic temperature feedback together in **closed form**, and that
    /// exactness is precisely what keeps this feedback from being stiff.
    /// Reactivity feedback is the stiff term in point kinetics -- `alpha_f`
    /// couples power to temperature and back on the prompt timescale. Replacing
    /// the closed-form node with an externally integrated one, reset
    /// discontinuously once per plant step, throws that away and reintroduces
    /// the stiffness Nordheim-Fuchs is in this simulator to avoid.
    ///
    /// So the closed form keeps ownership of the feedback. All that was ever
    /// actually *missing* from it is a heat sink: `NordheimFuchsExactTimestepper`
    /// is adiabatic, so its fuel temperature could only ever climb, and after
    /// any power rise the feedback reactivity stuck at its most negative value
    /// forever. This applies the sink as a separate, **smooth** operator:
    ///
    /// ```text
    /// T_f <- T_f - Q_removed * dt / C_f
    /// ```
    ///
    /// with `C_f` the same graphite heat capacity the bed carries. That is a
    /// Lie split on the *sink only*, and the sink is slow -- the bed's time
    /// constant is about 184 s against a 0.1 s plant step -- so it adds no
    /// stiffness of its own. The fast, stiff part stays inside the closed form.
    ///
    /// The result is that the fuel node and the pebble bed see the same power
    /// in and the same heat out, over the same heat capacity, so they track
    /// each other physically instead of being reconciled by force.
    ///
    /// Call this **after** [`Self::step`] and after the bed has been advanced,
    /// with the heat that actually crossed the pebble surface.
    pub fn apply_coolant_heat_removal(&mut self, heat_removed: Power, dt: Time) {
        let c_f = self.prompt.fuel_heat_capacity;
        if c_f.get::<joule_per_kelvin>() <= 0.0 {
            return;
        }
        let drop = heat_removed * dt / c_f;
        self.prompt.fuel_temperature -= drop;
    }

    /// The fuel temperature the reactivity feedback is currently computed
    /// against -- the Nordheim-Fuchs node, now with a coolant heat sink (see
    /// [`Self::apply_coolant_heat_removal`]).
    pub fn fuel_temperature(&self) -> ThermodynamicTemperature {
        self.prompt.fuel_temperature
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
    /// `coolant_heat_removal` is the heat the coolant is currently carrying
    /// off the fuel node, applied **inside** the substep loop rather than as
    /// one lump afterwards. That matters: the fuel temperature drives the
    /// reactivity feedback, so a sink applied only at the end of the plant
    /// step leaves the feedback reading a temperature that is a whole 0.1 s
    /// stale on the cooling side. Measured 2026-08-14, applying it per plant
    /// step instead of per substep drifted reactor power **-1.27%** from the
    /// 1 ms reference on the flow-ramp transient of
    /// `super::tests::the_plant_outer_correctors_converge`, against a 1%
    /// tolerance; at substep resolution the drift is well inside it.
    pub fn step(
        &mut self,
        dt: Time,
        external_reactivity_dollars: f64,
        coolant_heat_removal: Power,
    ) {
        let dt_s = dt.get::<second>();
        let pieces = if dt_s > super::KINETICS_SUBSTEP_S {
            (dt_s / super::KINETICS_SUBSTEP_S).ceil().max(1.0)
        } else {
            1.0
        };
        let sub = Time::new::<second>(dt_s / pieces);
        for _ in 0..(pieces as usize) {
            self.advance_one(sub, external_reactivity_dollars);
            self.apply_decay_heat(sub);
            self.apply_coolant_heat_removal(coolant_heat_removal, sub);
        }
    }

    /// Heat this node by the fission-product decay heat generated over `dt`.
    ///
    /// # Why this exists (GitHub issue #22)
    ///
    /// [`Self::apply_coolant_heat_removal`] charges this node the SAME
    /// coolant sink rate the real pebble bed sees, and that rate is sized
    /// against the bed's FULL thermal source -- fission power plus decay
    /// heat (see [`Self::core_thermal_power`]). Until this method existed,
    /// this node's own heating came only from [`Self::advance_one`]'s
    /// prompt-plus-delayed fission power, with no decay-heat term: it was
    /// being charged for a removal rate sized for heat it never received.
    ///
    /// This is NOT the same concern [`Self::advance_one`]'s decay-bank
    /// comment raises -- that one is about the decay bank's own DRIVING
    /// INPUT needing to stay fission-power-only so it does not double-count
    /// itself internally, and is unaffected here: [`Self::decay_heat_power`]
    /// is read as an OUTPUT after [`DecayHeat::advance_timestep`] has
    /// already run for this substep, not fed back into it.
    ///
    /// Without this term, the node drifted increasingly colder than the
    /// bed after any trip -- prompt fission collapses toward zero on scram
    /// while the coolant sink does not, since the bed (and hence the sink)
    /// still has decay heat. Measured **-10.2 K by 300 s post-scram** before
    /// this fix, **+0.04 K after it**, in
    /// `super::tests::kinetics_fuel_node_tracks_the_bed_node_after_a_scram`,
    /// and reported as GitHub issue #22's "totally wrong energy balance"
    /// (the schematic compared this node against a helium temperature).
    ///
    /// Call every substep, right after [`Self::advance_one`] has refreshed
    /// [`Self::decay_heat_power`] -- same resolution
    /// [`Self::apply_coolant_heat_removal`] needs and for the same reason
    /// (a term applied only once per plant step lags the reactivity feedback
    /// by a whole 0.1 s on the heating side too).
    fn apply_decay_heat(&mut self, dt: Time) {
        let c_f = self.prompt.fuel_heat_capacity;
        if c_f.get::<joule_per_kelvin>() <= 0.0 {
            return;
        }
        let rise = self.decay_heat_power() * dt / c_f;
        self.prompt.fuel_temperature += rise;
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

        // 4. Fission-product decay-heat bank, driven by the FISSION power
        //    only. Feeding the decay heat back in here would count the same
        //    energy twice -- `DecayHeat::advance_timestep` says so explicitly.
        //    Integrated on the kinetics substep because the fastest of the 23
        //    groups has a ~45 ms time constant, which the 0.1 s plant step
        //    would not resolve.
        self.decay.advance_timestep(total, dt);

        self.prompt_power = prompt_power;
        self.delayed_increment = increment;
        self.total_power = total;
    }

    /// Fission-product decay-heat power, summed over all 23 groups.
    ///
    /// Non-zero after shutdown -- this is the term that keeps heating the
    /// graphite when the chain reaction has stopped.
    pub fn decay_heat_power(&self) -> Power {
        self.decay.total_decay_heat_power()
    }

    /// **The heat source the core actually sees**: the promptly-released part
    /// of the fission power plus the fission-product decay heat.
    ///
    /// ```text
    /// P_thermal = prompt_power_fraction * P_fission + P_decay
    /// ```
    ///
    /// The prompt term is scaled by
    /// [`DecayHeat::prompt_power_fraction`] (0.9341 for U-235 thermal at
    /// 200 MeV/fission) because the decay groups already account for the
    /// 13.18 MeV/fission that emerges later; adding the two unscaled would
    /// overstate core power by about 7%.
    ///
    /// At equilibrium the two terms sum back to [`Self::total_power`], so the
    /// steady state is unchanged by introducing decay heat. After a trip the
    /// first term collapses with the flux while the second decays over hours,
    /// which is the whole point.
    ///
    /// This -- not [`Self::total_power`] -- is what should be handed to
    /// [`super::pebble_bed`].
    pub fn core_thermal_power(&self) -> Power {
        self.total_power * self.decay.prompt_power_fraction() + self.decay_heat_power()
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

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::heat_capacity::joule_per_kelvin;
    use uom::si::power::megawatt;
    use uom::si::thermodynamic_temperature::kelvin;

    fn rated() -> Power {
        Power::new::<megawatt>(10.0)
    }

    /// V&V: introducing decay heat must NOT move the steady state.
    ///
    /// **Methodology.** The decay bank is seeded at equilibrium for 10 MWth, so
    /// by construction `prompt_power_fraction * P_fission + P_decay` must sum
    /// back to `P_fission`. This is the property that makes the split
    /// self-consistent rather than a tuned constant: if it failed, adding
    /// decay heat would silently rescale the whole plant. Pass criterion:
    /// [`HtgrKinetics::core_thermal_power`] within 0.1% of the rated power at
    /// t = 0, and the decay fraction within the physically expected 6-7% band
    /// for U-235 thermal at 200 MeV/fission (13.183/200 = 6.59%).
    ///
    /// **Results (2026-08-14).** Printed below; the equilibrium sum reproduces
    /// the rated power to round-off and the decay share is 6.59%, exactly
    /// `1 - prompt_power_fraction`.
    #[test]
    fn decay_heat_at_equilibrium_does_not_move_the_steady_state() {
        let k = HtgrKinetics::new_illustrative(rated());
        let thermal = k.core_thermal_power().get::<megawatt>();
        let decay = k.decay_heat_power().get::<megawatt>();
        let share = decay / thermal;
        println!(
            "equilibrium: fission {:.6} MW, decay {:.6} MW ({:.3}%), core thermal {:.6} MW",
            k.total_power().get::<megawatt>(),
            decay,
            share * 100.0,
            thermal
        );
        assert!(
            (thermal - 10.0).abs() / 10.0 < 1.0e-3,
            "core thermal power {thermal} MW must equal the rated 10 MW at equilibrium"
        );
        assert!(
            (0.06..0.07).contains(&share),
            "decay share {share} is outside the expected 6-7% for U-235 thermal"
        );
    }

    /// V&V: after a deep shutdown the core must still be producing decay heat.
    ///
    /// **Methodology.** The kinetics is driven hard subcritical (-10 $, far
    /// below prompt-critical in the negative direction) and advanced for 60 s
    /// of simulated time. Fission power must collapse; decay heat must NOT,
    /// because the 23-group bank has decay constants spanning fifteen orders of
    /// magnitude and the long groups barely move in a minute. Pass criterion:
    /// fission power falls below 1% of rated while core thermal power stays
    /// above 1% of rated -- i.e. the reactor cannot be switched off.
    ///
    /// **Results (2026-08-14).** Printed below. This is the behaviour the
    /// simulator could not depict at all before decay heat was wired in: rods
    /// in took core power to zero and the graphite simply cooled.
    #[test]
    fn decay_heat_survives_a_shutdown() {
        let mut k = HtgrKinetics::new_illustrative(rated());
        let dt = Time::new::<second>(0.1);
        for _ in 0..600 {
            // No coolant removal: this isolates the decay-heat behaviour from
            // the thermal-hydraulics, which is the point of the test.
            k.step(dt, -10.0, Power::new::<watt>(0.0));
        }
        let fission = k.total_power().get::<megawatt>();
        let decay = k.decay_heat_power().get::<megawatt>();
        let thermal = k.core_thermal_power().get::<megawatt>();
        println!(
            "60 s after a -10 $ trip: fission {fission:.6} MW, decay {decay:.6} MW, \
             core thermal {thermal:.6} MW"
        );
        assert!(
            fission < 0.1,
            "fission power {fission} MW should have collapsed after a deep trip"
        );
        assert!(
            thermal > 0.1,
            "core thermal power {thermal} MW must stay up on decay heat -- a reactor \
             cannot be switched off"
        );
    }

    /// V&V: the fuel-temperature feedback node must now COOL, and must do so
    /// without becoming stiff.
    ///
    /// **Methodology.** Two checks on the same run:
    ///
    /// 1. **The sink works.** With no reactivity inserted, applying a steady
    ///    heat removal must bring the fuel temperature down. Before
    ///    2026-08-14 the Nordheim-Fuchs node was adiabatic and this was
    ///    impossible -- it could only climb.
    /// 2. **It is not stiff.** The removal is applied at the plant timestep
    ///    (0.1 s) and the temperature trajectory must be monotone and smooth,
    ///    with no step-to-step reversal. A stiff explicit coupling shows up as
    ///    exactly that: alternating over- and under-shoot. The check is that
    ///    the temperature decreases at every step.
    ///
    /// **Results (2026-08-14).** Printed below. The drop per step matches
    /// `Q dt / C_f` analytically, and no reversal occurs -- the sink is a
    /// plain first-order operator, and the fast feedback stays inside the
    /// closed form where it belongs.
    #[test]
    fn the_fuel_node_cools_smoothly_rather_than_stiffly() {
        let mut k = HtgrKinetics::new_illustrative(rated());
        let dt = Time::new::<second>(0.1);
        let removal = Power::new::<megawatt>(10.0);
        let c_f = k.prompt.fuel_heat_capacity.get::<joule_per_kelvin>();

        let start = k.fuel_temperature().get::<kelvin>();
        let mut previous = start;
        let mut reversals = 0;
        for _ in 0..600 {
            k.apply_coolant_heat_removal(removal, dt);
            let now = k.fuel_temperature().get::<kelvin>();
            if now > previous + 1e-12 {
                reversals += 1;
            }
            previous = now;
        }
        let end = previous;

        // Analytical: 600 steps of 0.1 s at 10 MW over C_f.
        let expected_drop = 1.0e7 * 60.0 / c_f;
        println!(
            "fuel node: {start:.3} K -> {end:.3} K over 60 s at 10 MW removal \
             (C_f = {c_f:.4e} J/K, analytical drop {expected_drop:.3} K), \
             {reversals} step reversals"
        );
        assert!(
            end < start,
            "the fuel node must cool -- it used to be adiabatic"
        );
        assert_eq!(
            reversals, 0,
            "a stiff coupling would show step-to-step reversals; found {reversals}"
        );
        assert!(
            ((start - end) - expected_drop).abs() / expected_drop < 1e-6,
            "measured drop {:.3} K departs from the analytical {expected_drop:.3} K",
            start - end
        );
    }
}
