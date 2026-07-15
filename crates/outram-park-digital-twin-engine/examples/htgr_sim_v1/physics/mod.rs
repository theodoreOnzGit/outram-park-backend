//! HTGR plant physics backend (scaffold).
//!
//! Orchestrates the three subsystems of a helium-cooled, graphite-moderated
//! HTGR into a single [`HtgrPlant`] that steps them in the physical order they
//! are coupled:
//!
//! 1. [`kinetics`] -- reactor power from the prompt excursion layer + a
//!    delayed-neutron precursor bank (see that module for what is real vs a
//!    placeholder for the forthcoming `teh_o_prke::DelayedNeutronLayer`).
//! 2. [`primary_loop`] -- reactor power heats the helium coolant; the IHX
//!    rejects it to the secondary side.
//! 3. [`secondary_loop`] -- the IHX duty drives a real IAPWS-IF97 steam cycle
//!    (steam generator -> turbine -> condenser).
//!
//! This is a **scaffold**: the structure and cross-crate wiring are real, but
//! several correlations are first-cut placeholders (flagged per module and in
//! beads `op-wqk.9.1` / `.2` / `.3`). It is not a validated HTGR model.
//!
//! What belongs here: the plant orchestration and the physics->snapshot
//! projection. What does not: GUI/rendering (that is [`crate::app`]) and the
//! underlying physics kernels (those live in the workspace libraries this
//! composes).

pub mod kinetics;
pub mod primary_loop;
pub mod secondary_loop;

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

        // 2. Primary helium loop absorbs the power; IHX rejects it.
        self.primary.step(dt, reactor_power, helium_flow_setpoint);

        // 3. Secondary steam loop driven by the IHX duty.
        self.secondary.step(
            self.primary.ihx_duty(),
            self.primary.core_outlet_temperature(),
        );
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

        // Clock.
        s.sim_time_s = self.sim_time.get::<second>();
    }
}

impl Default for HtgrPlant {
    fn default() -> Self {
        Self::new()
    }
}
