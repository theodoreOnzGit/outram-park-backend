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
//!    the steam generator, which rejects it to the secondary side,
//!    pinch-limited by an effectiveness-NTU model.
//! 4. [`secondary_loop`] -- the steam-generator duty drives a real IAPWS-IF97
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
//! | Steam generator | **1** effectiveness-NTU lump | no economiser/evaporator/superheater zones |
//! | Secondary water/steam | **1** | fixed steam pressure, no drum or inventory dynamics |
//! | Neutronics | **1** (point kinetics) | no spatial flux shape, no rod-position-dependent worth |
//! | Reflector, barrel, cavity | **0** | the HTR-10 passive decay-heat path is absent entirely |
//!
//! Each module's own doc comment states what its single node lumps and what
//! refinement to reach for first. Read [`pebble_bed`] before quoting any core
//! temperature from this model.
//!
//! ## The loops are coupled both ways
//!
//! The loops are not run open-ended in sequence. Each step reads the
//! secondary's saturation temperature *first* and hands it to the primary as
//! the steam generator's cold-side pinch, so:
//!
//! - the secondary's pressure limits how much heat the helium can shed, and
//! - the resulting helium-side outlet becomes the next core inlet.
//!
//! That closes the primary loop (the core inlet is a computed variable, not
//! a constant) and makes the steam generator duty-limited rather than
//! absorbing whatever the primary offers. The pebble bed sits inside that
//! loop: it reads the helium bulk mean temperature and returns a heat rate, so
//! a loss of heat removal backs up into the graphite temperature.
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
//! What remains illustrative is every *closure and every dimension the
//! published sources do not carry* -- the pebble-to-helium heat-transfer
//! coefficient (measurably too low, see [`pebble_bed`]), graphite `c_p`, the
//! loop gas volume, the steam-generator `UA`, efficiencies, inventories and
//! controller constants. An effective bed conductivity now **exists** in the
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
pub mod secondary_loop;

use outram_park_digital_twin_engine::animation::residence_time_from_flow;
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
use secondary_loop::SteamSecondaryLoop;

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
        let external_reactivity_dollars =
            self.external_reactivity_dollars(control_rod_insertion_fraction);
        self.kinetics.step(dt, external_reactivity_dollars);
        let reactor_power = self.kinetics.total_power();

        // 2. Pebble bed absorbs the fission power and hands the helium only
        //    what crosses the pebble surface. The bed reads the helium bulk
        //    mean temperature from the *previous* step, which is an explicit
        //    (Lie-split) coupling -- safe here because the bed's ~184 s time
        //    constant is four orders of magnitude above the timestep.
        let helium_bulk = self.primary.helium_bulk_temperature();
        self.core_heat_to_helium =
            self.core
                .step(dt, reactor_power, helium_bulk, self.primary.mass_flow());

        // 3. Primary helium loop carries that heat to the steam generator,
        //    which rejects it into the secondary side, pinch-limited against
        //    the steam saturation temperature. Reading that temperature
        //    *before* the primary step is what closes the primary<->secondary
        //    coupling: the secondary's pressure sets how cold the helium can
        //    get, and the resulting helium-side outlet becomes the next core
        //    inlet.
        let secondary_sink = self.secondary.saturation_temperature();
        self.primary.step(
            dt,
            self.core_heat_to_helium,
            helium_flow_setpoint,
            secondary_sink,
        );

        // 4. Secondary steam loop driven by that (already limited) duty.
        self.secondary.step(dt, self.primary.ihx_duty());
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

        // Clock.
        s.sim_time_s = self.sim_time.get::<second>();
    }
}

impl Default for HtgrPlant {
    fn default() -> Self {
        Self::new()
    }
}
