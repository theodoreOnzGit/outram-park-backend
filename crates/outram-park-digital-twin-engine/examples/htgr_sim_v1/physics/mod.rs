//! HTGR plant physics backend -- **pebble-bed** core, HTR-10 scale.
//!
//! Orchestrates the four subsystems of a helium-cooled, graphite-moderated
//! **pebble-bed** HTGR into a single [`HtgrPlant`] that steps them in the
//! physical order they are coupled:
//!
//! 1. [`kinetics`] -- reactor power from the prompt excursion layer + a
//!    delayed-neutron precursor bank, wired to the real
//!    `teh_o_prke::DelayedNeutronLayer`.
//! 2. [`pebble_bed`] -- the fission power heats 5.3 t of graphite pebbles,
//!    which hand the helium whatever crosses the pebble surface. This is the
//!    core's thermal inertia and it is the slowest thing in the plant.
//! 3. [`primary_loop`] -- the helium carries that heat from the core outlet to
//!    the steam generator and returns cooled, closing the circuit.
//! 4. [`steam_generator`] -- a **resolved 8-node counter-flow exchanger**
//!    (helium <-> steel tube metal <-> water/steam) owned by the primary loop.
//!    This is the only spatially discretised part of the plant.
//! 5. [`secondary_loop`] -- the steam-generator duty drives a real IAPWS-IF97
//!    steam cycle (feed pump -> steam generator -> turbine -> condenser ->
//!    hotwell).
//!
//! ## This core used to be prismatic
//!
//! Until 2026-08-12 this simulator modelled a **prismatic-block** HTGR at
//! roughly 200 MWth with machined coolant channels. It now models a pebble bed
//! at the published HTR-10 operating point -- 10 MWth, helium at 3.0 MPa,
//! 250 degC in and 700 degC out at 4.3 kg/s, 27,000 spherical fuel elements in
//! a 1.8 m by 1.97 m bed, with **downward** flow through the bed and a
//! separate-vessel once-through helical steam generator. The whole plant was
//! rescaled with it, so every displayed magnitude is roughly twenty times
//! smaller than it was.
//!
//! ## Nodalisation of the whole plant
//!
//! Every subsystem here is **one control volume**. Nothing in this plant model
//! is spatially discretised:
//!
//! | Subsystem | Nodes | Consequence |
//! |---|---|---|
//! | Pebble bed | **1** | no axial or radial temperature profile, no peak fuel temperature |
//! | Helium circuit | **1** (two boundary temperatures) | no gradient through the bed, no natural circulation |
//! | Steam generator | **8 x 3** (helium / tube metal / water, counter-flow) | resolved zones and a real metal lag; 8 nodes is coarse |
//! | Secondary water/steam, outside the SG | **1** | fixed steam pressure, no drum or inventory dynamics |
//! | Neutronics | **1** (point kinetics) | no spatial flux shape, no rod-position-dependent worth |
//! | Reflector, barrel, cavity | **0** | the HTR-10 passive decay-heat path is absent entirely |
//!
//! **The steam generator is the exception, as of 2026-08-12**, and it is the
//! only part of this plant that is not one control volume. See
//! [`steam_generator`].
//!
//! Each module's own doc comment states what its single node lumps and what
//! refinement to reach for first. Read [`pebble_bed`] before quoting any core
//! temperature from this model.
//!
//! ## The loops are coupled both ways
//!
//! The loops are not run open-ended in sequence. Each step reads the
//! secondary's **feedwater state** (enthalpy and flow) *first* and hands it to
//! the primary as the steam generator's tube-side inlet, so:
//!
//! - the water entering the tubes limits how much heat the helium can shed, and
//! - the resulting helium-side outlet becomes the next core inlet.
//!
//! That closes the primary loop (the core inlet is a computed variable, not a
//! constant) and makes the steam generator's duty a *resolved* result rather
//! than a formula.
//!
//! Until 2026-08-12 what crossed here was the secondary's **saturation
//! temperature**, and the exchanger was an effectiveness-NTU lump pinching
//! against it as an isothermal sink. That is correct for an evaporator and wrong
//! for a once-through unit: as the steam superheats the real driving difference
//! collapses, and against a fixed sink it never did. Measured 2026-08-12, the
//! old model over-predicted the hot-end driving difference by **78%** (470.4 K
//! against the resolved 263.8 K), and the steam it produced was clamped at the
//! helium inlet temperature by a downstream second-law cap. See
//! [`steam_generator`].
//!
//! The pebble bed sits inside that loop: it reads the helium bulk mean
//! temperature and returns a heat rate, so a loss of heat removal backs up into
//! the graphite temperature.
//!
//! ## Status
//!
//! The structure, the cross-crate wiring, the published HTR-10 operating point
//! and core geometry, and the thermophysical properties (helium via the
//! CoolProp-derived Helmholtz EOS, water/steam via IAPWS-IF97) are **real**.
//!
//! **Every published constant now comes from the library**, not from a copy
//! kept here: [`outram_park_digital_twin_engine::htr10::design::Htr10DesignPoint`]
//! is the single transcription of IAEA-TECDOC-1382 that the core geometry, the
//! helium operating point, the steam conditions and the nominal power are all
//! read from. Two copies of an operating point drift silently; there is now
//! one.
//!
//! **The pebble-bed friction is real** as of 2026-08-12: the KTA packed-bed
//! correlation ([`outram_park_digital_twin_engine::htr10::kta`]) is evaluated
//! by [`primary_loop::bed_pressure_drop`] and gated against the Virtual Test
//! Bed's published worked example (3493.17 Pa/m against the gold 3493 Pa/m).
//! **That makes the friction real; it does not make the nodalisation real.**
//! Every subsystem is still one control volume, the correlation is applied once
//! at the bulk mean rather than integrated down the bed, and the pressure drop
//! still cannot feed back on the flow.
//!
//! **The steam generator is spatially resolved** as of 2026-08-12: an 8-node
//! counter-flow exchanger coupling a helium array, a steel tube-metal column and
//! a water/steam array through real conductances ([`steam_generator`]). It
//! resolves the economiser / evaporator / superheater zones, carries a derived
//! **3184 kg** of tube metal with a **38 s** thermal time constant, and cannot
//! represent a temperature cross at any node. **That makes the arrangement real;
//! it does not make the sizing real** -- the `UA` is still an explicit
//! calibration and the tube diameters are still invented.
//!
//! What remains illustrative is every *closure and every dimension the
//! published sources do not carry* -- the pebble-to-helium heat-transfer
//! coefficient (measurably too low, see [`pebble_bed`]), graphite `c_p`, the
//! loop gas volume, the steam-generator `UA` and tube geometry, efficiencies,
//! inventories and controller constants. An effective bed conductivity now **exists** in the
//! workspace ([`outram_park_digital_twin_engine::htr10::zbs`]) but is
//! deliberately not in the heat path, because one control volume has no
//! internal gradient for it to act on. The live steam pressure is still held
//! fixed (see [`secondary_loop`]). Replacing the invented dimensions with
//! sourced ones is tracked as bead `op-szmi.6`.
//!
//! This is a demonstration model for the digital-twin engine, **not a
//! validated HTR-10 model**, and must not be used for any of the purposes
//! `RESPONSIBLE_USE.md` excludes.
//!
//! What belongs here: the plant orchestration and the physics->snapshot
//! projection. What does not: GUI/rendering (that is [`crate::app`]) and the
//! underlying physics kernels (those live in the workspace libraries this
//! composes).

pub mod control_rods;
pub mod kinetics;
pub mod pebble_bed;
pub mod primary_loop;
pub mod protection;
pub mod secondary_loop;
pub mod steam_generator;
pub mod turbine_generator;

use outram_park_digital_twin_engine::animation::residence_time_from_flow;
use outram_park_digital_twin_engine::app_scaffold::mark_component;
use uom::si::f64::{MassRate, Power, ThermodynamicTemperature, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::{megawatt, watt};
use uom::si::pressure::kilopascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use crate::app::state::HtgrSnapshot;
use kinetics::{power_in_megawatts, HtgrKinetics};
use pebble_bed::PebbleBedCore;
use primary_loop::HeliumPrimaryLoop;
use protection::ReactorProtectionSystem;
use secondary_loop::SteamSecondaryLoop;
use turbine_generator::TurbineGeneratorShaft;

/// Nominal thermal power used to seed the kinetics and size the loops: 10 MWth
/// (published HTR-10 figure, IAEA-TECDOC-1382 Table 4-1), read from
/// [`outram_park_digital_twin_engine::htr10::design::Htr10DesignPoint`] rather
/// than re-typed here.
pub fn nominal_thermal_power() -> Power {
    pebble_bed::design().thermal_power
}

/// Nominal helium mass flow: 4.3 kg/s at full power (published, via the same
/// design point).
pub fn nominal_helium_flow() -> MassRate {
    pebble_bed::nominal_helium_flow()
}

/// The full plant model: kinetics + pebble-bed core + helium primary loop +
/// steam secondary loop, plus the running simulation clock.
pub struct HtgrPlant {
    /// Reactor kinetics slot (prompt excursion + delayed-neutron bank).
    pub kinetics: HtgrKinetics,
    /// Lumped pebble-bed core -- the graphite thermal inertia between the
    /// fission power and the helium.
    pub core: PebbleBedCore,
    /// Helium primary loop.
    pub primary: HeliumPrimaryLoop,
    /// Steam secondary loop.
    pub secondary: SteamSecondaryLoop,
    /// Turbine-generator rotor. Driven by the secondary loop's enthalpy-drop
    /// power through a real torque balance, so the schematic's turbine rotor
    /// turns at a computed shaft speed rather than an animation constant. See
    /// [`turbine_generator`] -- especially on why the speed lands near
    /// synchronous and why this is an islanded, ungoverned machine.
    pub shaft: TurbineGeneratorShaft,
    /// Reactor protection system. Trips on measurable signals and drives the
    /// rod bank in, so a prompt excursion terminates instead of running the
    /// model out of its property range. See [`protection`].
    pub protection: ReactorProtectionSystem,
    /// Accumulated simulation time.
    pub sim_time: Time,
    /// Heat rate crossing the pebble surface into the helium on the most recent
    /// step -- the core's *thermal* output, which lags the fission power by the
    /// graphite time constant.
    core_heat_to_helium: Power,
}

impl HtgrPlant {
    /// Construct the plant at the published HTR-10 operating point.
    pub fn new() -> Self {
        let nominal_power = nominal_thermal_power();
        Self {
            kinetics: HtgrKinetics::new_illustrative(nominal_power),
            core: PebbleBedCore::new(),
            primary: HeliumPrimaryLoop::new(nominal_helium_flow()),
            secondary: SteamSecondaryLoop::new(),
            shaft: TurbineGeneratorShaft::new(),
            protection: ReactorProtectionSystem::new(),
            sim_time: Time::new::<second>(0.0),
            core_heat_to_helium: Power::new::<watt>(0.0),
        }
    }

    /// Heat rate crossing the pebble surface into the helium on the most recent
    /// step. At steady state this equals the fission power; during a transient
    /// it lags it by the bed's ~184 s graphite time constant.
    #[allow(dead_code)] // snapshot candidate -- not yet wired into the app layer
    pub fn core_heat_to_helium(&self) -> Power {
        self.core_heat_to_helium
    }

    /// Lumped pebble (graphite) temperature -- a **bed average**, not a peak
    /// fuel temperature. See [`pebble_bed`] for why.
    #[allow(dead_code)] // snapshot candidate -- not yet wired into the app layer
    pub fn pebble_temperature(&self) -> ThermodynamicTemperature {
        self.core.temperature()
    }

    /// External reactivity in dollars currently commanded by the control-rod
    /// bank, for the given insertion fraction.
    ///
    /// The operator commands rod *position*; reactivity is what results. See
    /// [`control_rods`] for the published HTR-10 bank worth and cold clean
    /// excess this is derived from, and for what remains illustrative about it.
    pub fn external_reactivity_dollars(&self, control_rod_insertion_fraction: f64) -> f64 {
        control_rods::external_reactivity_dollars(
            control_rod_insertion_fraction,
            self.kinetics
                .delayed_neutron_fraction()
                .get::<uom::si::ratio::ratio>(),
        )
    }

    /// Advance the whole plant by one timestep `dt`, given the user control
    /// inputs (control-rod bank insertion fraction in `0..=1` and the helium
    /// pump flow setpoint).
    pub fn step(
        &mut self,
        dt: Time,
        control_rod_insertion_fraction: f64,
        helium_flow_setpoint: MassRate,
    ) {
        self.sim_time += dt;

        // 1. Kinetics -> reactor fission power. Rod position is converted to
        //    reactivity here rather than in the GUI so the physics owns the
        //    conversion and an OPC-UA write of a rod position gets the same
        //    treatment as a slider drag.
        // 0. Protection system. Evaluated on the PREVIOUS step's measured
        //    signals, before any reactivity is applied, so a trip cannot be
        //    outrun within a timestep. Its scram demand can only deepen the
        //    operator's rod command, never lift it.
        self.protection.update(
            dt,
            self.kinetics.total_power(),
            self.primary.core_outlet_temperature(),
        );
        let control_rod_insertion_fraction = self
            .protection
            .effective_rod_insertion(control_rod_insertion_fraction);

        let external_reactivity_dollars =
            self.external_reactivity_dollars(control_rod_insertion_fraction);
        // Each subsystem announces itself before stepping, so a panic anywhere
        // below is attributed to a piece of PLANT EQUIPMENT in the crash modal
        // rather than only to a source file. The whole plant runs on one
        // physics thread, so the thread name alone identifies nothing.
        mark_component("reactor kinetics (point kinetics + control rods)");
        self.kinetics.step(dt, external_reactivity_dollars);
        let reactor_power = self.kinetics.total_power();

        // 2. Pebble bed absorbs the fission power and hands the helium only
        //    what crosses the pebble surface. The bed reads the helium bulk
        //    mean temperature from the *previous* step, which is an explicit
        //    (Lie-split) coupling -- safe here because the bed's ~184 s time
        //    constant is four orders of magnitude above the timestep.
        let helium_bulk = self.primary.helium_bulk_temperature();
        mark_component("pebble-bed core (graphite pebbles)");
        self.core_heat_to_helium =
            self.core
                .step(dt, reactor_power, helium_bulk, self.primary.mass_flow());

        // 3. Primary helium loop carries that heat to the steam generator,
        //    which is now a RESOLVED counter-flow exchanger rather than an
        //    effectiveness-NTU lump. Reading the secondary's feedwater state
        //    *before* the primary step is what closes the
        //    primary<->secondary coupling: the water entering the tube side
        //    sets how cold the helium can get, and the resulting helium-side
        //    outlet becomes the next core inlet.
        //
        //    The feedwater enthalpy and flow are one step old, which is the
        //    same explicit (Lie-split) coupling the saturation temperature used
        //    to be handed over with. It is safe here because the feedwater
        //    controller's own time constant is 10 s against a 0.05 s step, so
        //    the flow moves well under a percent between reads.
        let feedwater_enthalpy = self.secondary.feedwater_enthalpy();
        let secondary_flow = self.secondary.mass_flow();
        mark_component("helium primary loop (circulator + hot gas duct)");
        self.primary.step(
            dt,
            self.core_heat_to_helium,
            helium_flow_setpoint,
            feedwater_enthalpy,
            secondary_flow,
        );

        // 4. Secondary steam loop, driven by the duty the steam generator's
        // TUBE SIDE actually absorbed -- not by the heat the helium gave up.
        // The two differ by the tube metal's stored-energy rate, which is
        // exactly the transient the metal exists to provide.
        //
        // The core outlet is still handed over as the hot-side inlet, because
        // `secondary_loop::max_absorbable_duty` is retained as a backstop. It
        // should no longer bind: the exchanger's own outlet can never exceed
        // the local helium temperature, so the enthalpy balance downstream is
        // already bounded. See
        // `secondary_loop::tests::the_absorbable_duty_cap_no_longer_binds`.
        mark_component("steam generator + secondary steam loop (IF97)");
        self.secondary.step(
            dt,
            self.primary.steam_generator_duty_to_secondary(),
            self.primary.core_outlet_temperature(),
        );

        // 5. Turbine-generator shaft. Driven by the SAME enthalpy-drop power
        // the secondary loop just computed -- `T = P/omega`, so there is one
        // turbine power in this plant, not two. The rotor's speed is what the
        // schematic's turbine widget draws its blades turning at.
        mark_component("turbine-generator shaft (torque balance)");
        self.shaft
            .step(dt, self.secondary.turbine_power(), self.sim_time);
    }

    /// Project the current plant state onto the shared [`HtgrSnapshot`],
    /// writing only the *output* fields and leaving the GUI-owned control
    /// inputs untouched.
    pub fn write_snapshot(&self, s: &mut HtgrSnapshot) {
        // Kinetics.
        s.reactor_power_mw = power_in_megawatts(self.kinetics.total_power());
        s.prompt_power_mw = power_in_megawatts(self.kinetics.prompt_power());
        s.delayed_power_mw = power_in_megawatts(self.kinetics.delayed_power());
        s.fuel_temperature_k = self.kinetics.prompt.fuel_temperature.get::<kelvin>();
        // The BED temperature, not the kinetics fuel node. The two differ: the
        // kinetics node is a lumped point-kinetics fuel temperature driving
        // reactivity feedback, while this is the graphite the helium flows
        // over. Drawing the kinetics node made the bed appear COOLER than the
        // gas leaving it, which is thermodynamically impossible.
        s.bed_temperature_k = self.pebble_temperature().get::<kelvin>();
        s.trip_reason = self.protection.trip_reason();
        s.scram_insertion_fraction = self.protection.scram_insertion();
        s.reactivity_margin_dollars = self.kinetics.reactivity_margin_dollars();
        s.delayed_neutron_fraction_pcm = self
            .kinetics
            .delayed_neutron_fraction()
            .get::<uom::si::ratio::ratio>()
            * 1.0e5;

        // Primary loop.
        s.core_inlet_temp_k = self.primary.core_inlet_temperature().get::<kelvin>();
        s.core_outlet_temp_k = self.primary.core_outlet_temperature().get::<kelvin>();
        s.helium_mass_flow_kg_per_s = self.primary.mass_flow().get::<kilogram_per_second>();
        s.ihx_duty_mw = self.primary.ihx_duty().get::<megawatt>();
        s.ihx_outlet_temp_k = self.primary.ihx_outlet_temperature().get::<kelvin>();
        s.helium_residence_time_s =
            residence_time_from_flow(self.primary.helium_inventory(), self.primary.mass_flow())
                .get::<second>();
        s.primary_pressure_drop_kpa = self.primary.pressure_drop().get::<kilopascal>();
        s.bed_pressure_drop_kpa = self.primary.bed_pressure_drop().get::<kilopascal>();
        s.circulator_power_mw = self.primary.circulator_power().get::<megawatt>();
        s.helium_cp_j_per_kg_k =
            self.primary
                .specific_heat()
                .get::<uom::si::specific_heat_capacity::joule_per_kilogram_kelvin>();

        // Secondary loop.
        s.steam_pressure_mpa = self
            .secondary
            .steam_pressure()
            .get::<uom::si::pressure::megapascal>();
        s.sg_steam_outlet_temp_k = self.secondary.turbine_inlet_temperature().get::<kelvin>();
        s.turbine_inlet_temp_k = self.secondary.turbine_inlet_temperature().get::<kelvin>();
        s.steam_enthalpy_j_per_kg = self
            .secondary
            .steam_generator_outlet()
            .get_specific_enthalpy()
            .get::<uom::si::available_energy::joule_per_kilogram>();
        s.turbine_power_mw = self.secondary.turbine_power().get::<megawatt>();
        s.steam_quality_after_turbine = self.secondary.steam_quality_after_turbine();
        s.condenser_pressure_kpa = self.secondary.condenser_pressure().get::<kilopascal>();
        s.secondary_mass_flow_kg_per_s = self.secondary.mass_flow().get::<kilogram_per_second>();
        s.secondary_residence_time_s =
            residence_time_from_flow(self.secondary.inventory(), self.secondary.mass_flow())
                .get::<second>();
        s.feedwater_enthalpy_j_per_kg = self
            .secondary
            .feedwater_enthalpy()
            .get::<uom::si::available_energy::joule_per_kilogram>();
        s.condensate_enthalpy_j_per_kg = self
            .secondary
            .condensate()
            .get_specific_enthalpy()
            .get::<uom::si::available_energy::joule_per_kilogram>();
        s.feed_pump_power_mw = self.secondary.feed_pump_power().get::<megawatt>();
        s.net_cycle_power_mw = self.secondary.net_power().get::<megawatt>();
        s.condenser_duty_mw = self.secondary.condenser_duty().get::<megawatt>();
        s.cooling_water_outlet_temp_k = self
            .secondary
            .cooling_water_outlet_temperature()
            .get::<kelvin>();

        // Turbine-generator shaft.
        s.shaft_speed_rad_per_s = self
            .shaft
            .angular_velocity()
            .get::<uom::si::angular_velocity::radian_per_second>();
        s.shaft_speed_rpm = self.shaft.speed_rpm();
        s.generator_electrical_power_mw = self.shaft.electrical_power().get::<megawatt>();
        s.generator_rating_mw = self.shaft.rated_shaft_power().get::<megawatt>();

        // Clock.
        s.sim_time_s = self.sim_time.get::<second>();
    }
}

impl Default for HtgrPlant {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::HtgrSnapshot;

    /// The control-rod insertion fraction the GUI opens with, from
    /// `crate::app::state::HtgrSnapshot::default` -- the bank position at which
    /// the core is critical with no external reactivity, from
    /// [`control_rods::critical_insertion_fraction`].
    ///
    /// Kept in step with the app default by
    /// [`the_test_rod_position_matches_the_gui_default`].
    const HTGR_GUI_INITIAL_ROD_INSERTION: f64 = 0.6035;

    /// The whole-plant test must open where the GUI opens. If the app's default
    /// rod position moves, this catches it -- withdrawing the bank by even ten
    /// percent from critical is a prompt excursion in this core, so the two must
    /// not be allowed to drift apart silently.
    #[test]
    fn the_test_rod_position_matches_the_gui_default() {
        let gui_default = HtgrSnapshot::default().control_rod_insertion_fraction;
        assert!(
            (gui_default - HTGR_GUI_INITIAL_ROD_INSERTION).abs() < 1e-9,
            "the GUI opens at rod insertion {gui_default}, this test uses \
             {HTGR_GUI_INITIAL_ROD_INSERTION}"
        );
    }

    /// V&V (regression): **the whole plant survives being stepped at the GUI's
    /// own 1 ms timestep**, end to end.
    ///
    /// # Why this exists
    ///
    /// [`the_whole_plant_steps_without_crossing_or_leaving_property_range`]
    /// steps at 0.05 s, which is a sane plant timestep and the one every loop
    /// test uses. `crate::app` does not: its physics thread runs
    /// `PHYSICS_DT_S = 1.0e-3 s`, ten sub-steps per 10 ms tick. Measured
    /// 2026-08-12, a whole green test suite at 0.05 s coexisted with a simulator
    /// that killed its physics thread within 30 s of launch, because 1 ms is
    /// below the steam-generator arrays' stability window. Nothing in the suite
    /// drove the plant at the rate the application does. This test does.
    ///
    /// # Methodology
    ///
    /// A fresh plant stepped 20 000 times at 1 ms -- 20 s of simulated time, the
    /// window in which the crash occurred -- at the GUI's opening rod position
    /// and flow. Asserted: no panic, no node-by-node cross, the tube metal
    /// inside `SteelSS304L`'s range, and the clock advanced.
    ///
    /// # Results (measured 2026-08-12)
    ///
    /// Passes after [`super::steam_generator::SteamGeneratorConfig::substep`]
    /// was made an accumulate-and-advance clock. Figures printed by the test.
    #[test]
    fn the_whole_plant_steps_at_the_gui_timestep() {
        let mut plant = HtgrPlant::new();
        let mut snapshot = HtgrSnapshot::default();
        // The GUI's plant timestep -- `crate::app::PHYSICS_DT_S`.
        let dt = Time::new::<second>(1.0e-3);
        let flow = nominal_helium_flow();
        let mut worst_metal_k = 0.0_f64;

        for i in 0..20_000 {
            plant.step(dt, HTGR_GUI_INITIAL_ROD_INSERTION, flow);
            let sg = plant.primary.steam_generator_state();
            assert!(
                sg.worst_node_cross_kelvin() <= 1e-6,
                "temperature cross at 1 ms plant step {i}"
            );
            for t in sg.metal_node_temperatures.iter() {
                worst_metal_k = worst_metal_k.max(t.get::<kelvin>());
            }
        }
        plant.write_snapshot(&mut snapshot);
        println!(
            "GUI-TIMESTEP RUN (20 s at 1 ms): power {:.4} MW, core outlet {:.2} K, \
             steam {:.2} K, peak tube metal {worst_metal_k:.2} K",
            snapshot.reactor_power_mw, snapshot.core_outlet_temp_k, snapshot.sg_steam_outlet_temp_k
        );
        assert!(
            worst_metal_k < 1000.0,
            "tube metal reached {worst_metal_k} K"
        );
        assert!(plant.sim_time.get::<second>() > 19.0);
    }

    /// V&V: **the whole plant runs, and its steam generator holds the second law
    /// at every node, through the path the GUI actually drives.**
    ///
    /// # Why this test exists
    ///
    /// Every other test in this simulator drives one subsystem, or at most the
    /// primary and secondary loops in isolation. [`HtgrPlant::step`] is the only
    /// thing the GUI calls, and until 2026-08-12 nothing exercised it: the
    /// kinetics, the protection system, the pebble bed, the steam generator and
    /// the turbine shaft were each tested apart and integrated only in
    /// production. The steam-generator rework put a stiff, panicking dependency
    /// (three coupled CFD-style arrays, an IF97 flash that panics out of range
    /// and a steel property table that panics above 1000 K) into that path,
    /// which is a good reason to cover it.
    ///
    /// # Methodology
    ///
    /// A fresh [`HtgrPlant`] is stepped 3000 times at 0.05 s (150 s of simulated
    /// time) with the control rods at the **critical insertion the GUI opens
    /// with** ([`HTGR_GUI_INITIAL_ROD_INSERTION`] = 0.6035, the same value
    /// `crate::app::state::HtgrSnapshot::default` carries) and the circulator at
    /// the published 4.3 kg/s. At every step:
    ///
    /// - the steam generator's node-by-node cross measure must be zero;
    /// - the steam-generator tube metal must stay inside `SteelSS304L`'s
    ///   tabulated range (below 1000 K), since TUAS panics rather than
    ///   extrapolating;
    /// - every snapshot field the GUI reads must be finite.
    ///
    /// The snapshot projection [`HtgrPlant::write_snapshot`] is exercised too, so
    /// a field wired to a removed accessor is caught here rather than on screen.
    ///
    /// # Results (measured 2026-08-12)
    ///
    /// 3000 steps, no panic, zero crosses. Settled figures printed by the test.
    ///
    /// **A note on the rod position, because it is not a free choice.** Stepping
    /// this plant at a 50% bank insertion instead -- 10% withdrawn from critical
    /// -- is a **prompt excursion**: measured 2026-08-12, reactor power reaches
    /// 1073 MW within 1 s, the bed reaches 2261 K, the core outlet 2355 K, and
    /// the run dies at 6 s when the steam generator's tube metal passes
    /// `SteelSS304L`'s 1000 K ceiling and TUAS panics. The reactor protection
    /// system would terminate that at its 750 degC core-outlet trip, but it is
    /// **disarmed by default** in this simulator. That is pre-existing behaviour
    /// and not a consequence of the steam-generator rework -- an excursion
    /// previously died in the IF97 flash at 1073 K instead -- but the metal
    /// ceiling now arrives first, so it is recorded here.
    ///
    /// # Interpretation
    ///
    /// This is an integration smoke test with second-law teeth, not a validation
    /// of the plant. It says the GUI's physics thread survives its own opening
    /// state and that the exchanger behaves inside the full coupled loop, where
    /// the pebble bed's 184 s graphite lag and the protection system are also in
    /// the path.
    #[test]
    fn the_whole_plant_steps_without_crossing_or_leaving_property_range() {
        let mut plant = HtgrPlant::new();
        let mut snapshot = HtgrSnapshot::default();
        let dt = Time::new::<second>(0.05);
        let flow = nominal_helium_flow();

        let mut worst_cross = 0.0_f64;
        let mut worst_metal_k = 0.0_f64;

        for i in 0..3000 {
            plant.step(dt, HTGR_GUI_INITIAL_ROD_INSERTION, flow);
            plant.write_snapshot(&mut snapshot);

            let sg = plant.primary.steam_generator_state();
            worst_cross = worst_cross.max(sg.worst_node_cross_kelvin());
            for t in sg.metal_node_temperatures.iter() {
                worst_metal_k = worst_metal_k.max(t.get::<kelvin>());
            }
            assert!(
                sg.worst_node_cross_kelvin() <= 1e-6,
                "temperature cross of {} K in the steam generator at plant step {i}",
                sg.worst_node_cross_kelvin()
            );
            for (name, v) in [
                ("reactor_power_mw", snapshot.reactor_power_mw),
                ("core_inlet_temp_k", snapshot.core_inlet_temp_k),
                ("core_outlet_temp_k", snapshot.core_outlet_temp_k),
                ("ihx_duty_mw", snapshot.ihx_duty_mw),
                ("ihx_outlet_temp_k", snapshot.ihx_outlet_temp_k),
                ("sg_steam_outlet_temp_k", snapshot.sg_steam_outlet_temp_k),
                ("turbine_power_mw", snapshot.turbine_power_mw),
                (
                    "secondary_mass_flow_kg_per_s",
                    snapshot.secondary_mass_flow_kg_per_s,
                ),
                ("bed_temperature_k", snapshot.bed_temperature_k),
                ("shaft_speed_rpm", snapshot.shaft_speed_rpm),
            ] {
                assert!(
                    v.is_finite(),
                    "snapshot field {name} went non-finite at step {i}"
                );
            }
        }

        println!(
            "WHOLE-PLANT RUN (150 s, rods at the 0.6035 critical insertion, 4.3 kg/s):\n  \
             reactor power   = {:.4} MW\n  \
             bed temperature = {:.2} K\n  \
             core outlet     = {:.2} K ({:.2} degC)\n  \
             core inlet      = {:.2} K ({:.2} degC)\n  \
             SG duty         = {:.4} MW\n  \
             steam outlet    = {:.2} K ({:.2} degC)\n  \
             turbine power   = {:.4} MW, shaft {:.1} rpm\n  \
             worst node cross over the run    = {worst_cross:.6} K\n  \
             peak tube-metal temperature      = {worst_metal_k:.2} K (SteelSS304L limit 1000 K)",
            snapshot.reactor_power_mw,
            snapshot.bed_temperature_k,
            snapshot.core_outlet_temp_k,
            snapshot.core_outlet_temp_k - 273.15,
            snapshot.core_inlet_temp_k,
            snapshot.core_inlet_temp_k - 273.15,
            snapshot.ihx_duty_mw,
            snapshot.sg_steam_outlet_temp_k,
            snapshot.sg_steam_outlet_temp_k - 273.15,
            snapshot.turbine_power_mw,
            snapshot.shaft_speed_rpm,
        );

        assert!(
            worst_cross <= 1e-6,
            "worst cross over the run {worst_cross} K"
        );
        assert!(
            worst_metal_k < 1000.0,
            "tube metal reached {worst_metal_k} K, outside SteelSS304L's tabulated range"
        );
        assert!(
            plant.sim_time.get::<second>() > 149.0,
            "the plant clock did not advance"
        );
    }
}
