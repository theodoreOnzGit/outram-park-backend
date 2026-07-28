//! HTGR plant physics backend (scaffold).
//!
//! Orchestrates the three subsystems of a helium-cooled, graphite-moderated
//! HTGR into a single [`HtgrPlant`] that steps them in the physical order they
//! are coupled:
//!
//! 1. [`kinetics`] -- reactor power from the prompt excursion layer + a
//!    delayed-neutron precursor bank, wired to the real
//!    `teh_o_prke::DelayedNeutronLayer`.
//! 2. [`primary_loop`] -- reactor power heats the helium coolant; the IHX
//!    rejects it to the secondary side, pinch-limited by an
//!    effectiveness-NTU model.
//! 3. [`secondary_loop`] -- the IHX duty drives a real IAPWS-IF97 steam cycle
//!    (feed pump -> steam generator -> turbine -> condenser -> hotwell).
//!
//! ## The loops are coupled both ways
//!
//! The two loops are not run open-ended in sequence. Each step reads the
//! secondary's saturation temperature *first* and hands it to the primary as
//! the IHX's cold-side pinch, so:
//!
//! - the secondary's pressure limits how much heat the helium can shed, and
//! - the resulting IHX helium outlet becomes the next core inlet.
//!
//! That closes the primary loop (the core inlet is a computed variable, not
//! a constant) and makes the steam generator duty-limited rather than
//! absorbing whatever the primary offers.
//!
//! ## Status
//!
//! The structure, the cross-crate wiring, and the thermophysical properties
//! (helium via the CoolProp-derived Helmholtz EOS, water/steam via
//! IAPWS-IF97) are **real**. What remains illustrative is the *plant data* --
//! loop geometry, `UA` values, efficiencies, inventories and controller
//! constants are HTGR-scale stand-ins, not a specific design's numbers -- and
//! the live steam pressure is still held fixed (see [`secondary_loop`]).
//! This is a demonstration model for the digital-twin engine, **not a
//! validated HTGR model**, and must not be used for any of the purposes
//! `RESPONSIBLE_USE.md` excludes.
//!
//! What belongs here: the plant orchestration and the physics->snapshot
//! projection. What does not: GUI/rendering (that is [`crate::app`]) and the
//! underlying physics kernels (those live in the workspace libraries this
//! composes).

pub mod kinetics;
pub mod primary_loop;
pub mod secondary_loop;

use outram_park_digital_twin_engine::animation::residence_time_from_flow;
use uom::si::f64::{MassRate, Power, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::megawatt;
use uom::si::pressure::kilopascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use crate::app::state::HtgrSnapshot;
use kinetics::{power_in_megawatts, HtgrKinetics};
use primary_loop::HeliumPrimaryLoop;
use secondary_loop::SteamSecondaryLoop;

/// Nominal HTGR thermal power used to seed the kinetics and size the loops
/// (illustrative, ~200 MWth).
const NOMINAL_THERMAL_POWER_MW: f64 = 200.0;

/// Nominal helium mass flow (illustrative), sized so the core temperature rise
/// is HTGR-scale at nominal power.
const NOMINAL_HELIUM_FLOW_KG_PER_S: f64 = 85.0;

/// The full HTGR plant model: kinetics + helium primary loop + steam secondary
/// loop, plus the running simulation clock.
pub struct HtgrPlant {
    /// Reactor kinetics slot (prompt excursion + delayed-neutron bank).
    pub kinetics: HtgrKinetics,
    /// Helium primary loop.
    pub primary: HeliumPrimaryLoop,
    /// Steam secondary loop.
    pub secondary: SteamSecondaryLoop,
    /// Accumulated simulation time.
    pub sim_time: Time,
}

impl HtgrPlant {
    /// Construct the plant at its nominal operating point.
    pub fn new() -> Self {
        let nominal_power = Power::new::<megawatt>(NOMINAL_THERMAL_POWER_MW);
        Self {
            kinetics: HtgrKinetics::new_illustrative(nominal_power),
            primary: HeliumPrimaryLoop::new(MassRate::new::<kilogram_per_second>(
                NOMINAL_HELIUM_FLOW_KG_PER_S,
            )),
            secondary: SteamSecondaryLoop::new(),
            sim_time: Time::new::<second>(0.0),
        }
    }

    /// Advance the whole plant by one timestep `dt`, given the user control
    /// inputs (external reactivity in dollars and the helium pump flow
    /// setpoint).
    pub fn step(
        &mut self,
        dt: Time,
        external_reactivity_dollars: f64,
        helium_flow_setpoint: MassRate,
    ) {
        self.sim_time += dt;

        // 1. Kinetics -> reactor thermal power.
        self.kinetics.step(dt, external_reactivity_dollars);
        let reactor_power = self.kinetics.total_power();

        // 2. Primary helium loop absorbs the power; the IHX rejects it into
        //    the secondary side, pinch-limited against the steam saturation
        //    temperature. Reading that temperature *before* the primary step
        //    is what closes the primary<->secondary coupling: the secondary's
        //    pressure sets how cold the helium can get, and the resulting IHX
        //    outlet becomes the next core inlet.
        let secondary_sink = self.secondary.saturation_temperature();
        self.primary
            .step(dt, reactor_power, helium_flow_setpoint, secondary_sink);

        // 3. Secondary steam loop driven by that (already limited) IHX duty.
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
