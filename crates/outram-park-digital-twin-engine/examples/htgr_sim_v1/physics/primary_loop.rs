//! Helium primary loop (scaffold).
//!
//! Models a **helium-cooled, graphite-moderated prismatic-block HTGR** primary
//! loop as a single lumped coolant node: reactor thermal power heats helium
//! flowing through the core (core inlet -> core outlet), and an intermediate
//! heat exchanger (IHX) rejects that heat to the steam secondary loop.
//!
//! ## What is real vs placeholder
//!
//! - **Real:** `uom`-typed energy balance `Q = m_dot * c_p * dT`, a
//!   first-order thermal-inertia relaxation of the core-outlet temperature
//!   toward its steady-state value (so the display is visibly transient), and
//!   the IHX duty as `m_dot * c_p * (T_out - T_return)`.
//! - **Placeholder** (see bead `op-wqk.9.1`): constant helium `c_p` (no
//!   temperature/pressure dependence), a fixed core-inlet temperature, no
//!   friction / pressure-drop / pumping-power model, and no real fluid-array
//!   backend. A v2 should pull real helium properties and a pressure-drop
//!   correlation (e.g. `tuas` or `tampines`'s `CompressibleFluidArray`).

use uom::si::f64::{MassRate, Power, SpecificHeatCapacity, ThermodynamicTemperature, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

/// Helium isobaric specific heat, treated as constant (placeholder).
/// Illustrative value ~5193 J/(kg K).
const HELIUM_CP_J_PER_KG_K: f64 = 5193.0;

/// Fixed core-inlet helium temperature (placeholder), ~573 K (300 degC) --
/// typical prismatic-HTGR core inlet, illustrative only.
const CORE_INLET_TEMPERATURE_K: f64 = 573.0;

/// IHX cold-side helium return temperature the duty is measured against
/// (placeholder), taken equal to the core inlet in this single-node model.
const IHX_RETURN_TEMPERATURE_K: f64 = CORE_INLET_TEMPERATURE_K;

/// Thermal-inertia time constant of the lumped helium node (placeholder),
/// giving the displayed core-outlet temperature a visible first-order
/// transient rather than an instantaneous jump.
const THERMAL_TIME_CONSTANT_S: f64 = 5.0;

/// Lumped helium primary-loop state.
pub struct HeliumPrimaryLoop {
    /// Constant isobaric specific heat of helium (placeholder).
    c_p: SpecificHeatCapacity,
    /// Fixed core-inlet temperature (placeholder).
    core_inlet_temperature: ThermodynamicTemperature,
    /// Current helium mass flow (driven by the pump setpoint).
    mass_flow: MassRate,
    /// Current (transient) core-outlet helium temperature.
    core_outlet_temperature: ThermodynamicTemperature,
    /// Most recently computed IHX duty transferred to the secondary loop.
    ihx_duty: Power,
}

impl HeliumPrimaryLoop {
    /// Construct the loop at a nominal operating point: `nominal_flow` helium
    /// mass flow, core outlet seeded at the fixed core inlet temperature.
    pub fn new(nominal_flow: MassRate) -> Self {
        Self {
            c_p: SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(HELIUM_CP_J_PER_KG_K),
            core_inlet_temperature: ThermodynamicTemperature::new::<kelvin>(
                CORE_INLET_TEMPERATURE_K,
            ),
            mass_flow: nominal_flow,
            core_outlet_temperature: ThermodynamicTemperature::new::<kelvin>(
                CORE_INLET_TEMPERATURE_K,
            ),
            ihx_duty: Power::new::<watt>(0.0),
        }
    }

    /// Advance the loop by `dt`, absorbing reactor thermal power `reactor_power`
    /// with the helium flow commanded by `flow_setpoint`.
    ///
    /// Steady-state core outlet is `T_in + Q/(m_dot c_p)`; the displayed
    /// outlet relaxes toward it with [`THERMAL_TIME_CONSTANT_S`]. The IHX duty
    /// is the enthalpy the helium carries above the return temperature.
    pub fn step(&mut self, dt: Time, reactor_power: Power, flow_setpoint: MassRate) {
        // Clamp the commanded flow to a small positive floor so the energy
        // balance denominator never vanishes.
        let flow_kg_s = flow_setpoint.get::<kilogram_per_second>().max(1.0);
        self.mass_flow = MassRate::new::<kilogram_per_second>(flow_kg_s);

        let c_p = self.c_p.get::<joule_per_kilogram_kelvin>();
        let q_w = reactor_power.get::<watt>();
        let t_in_k = self.core_inlet_temperature.get::<kelvin>();

        // Steady-state core-outlet temperature from the energy balance.
        let t_out_ss_k = t_in_k + q_w / (flow_kg_s * c_p);

        // First-order relaxation toward the steady state (thermal inertia).
        let dt_s = dt.get::<second>();
        let alpha = (dt_s / THERMAL_TIME_CONSTANT_S).clamp(0.0, 1.0);
        let t_out_k = self.core_outlet_temperature.get::<kelvin>();
        let t_out_next_k = t_out_k + alpha * (t_out_ss_k - t_out_k);
        self.core_outlet_temperature = ThermodynamicTemperature::new::<kelvin>(t_out_next_k);

        // IHX duty: heat carried by the helium above the return temperature.
        let duty_w = (flow_kg_s * c_p * (t_out_next_k - IHX_RETURN_TEMPERATURE_K)).max(0.0);
        self.ihx_duty = Power::new::<watt>(duty_w);
    }

    /// Fixed core-inlet helium temperature (placeholder).
    pub fn core_inlet_temperature(&self) -> ThermodynamicTemperature {
        self.core_inlet_temperature
    }

    /// Current (transient) core-outlet helium temperature.
    pub fn core_outlet_temperature(&self) -> ThermodynamicTemperature {
        self.core_outlet_temperature
    }

    /// Current helium mass flow.
    pub fn mass_flow(&self) -> MassRate {
        self.mass_flow
    }

    /// Most recently computed IHX duty transferred to the secondary loop.
    pub fn ihx_duty(&self) -> Power {
        self.ihx_duty
    }
}
