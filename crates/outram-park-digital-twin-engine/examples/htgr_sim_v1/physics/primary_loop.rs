//! Helium primary loop, around a **pebble-bed** core.
//!
//! Models the primary circuit of a helium-cooled, graphite-moderated
//! **pebble-bed** HTGR at HTR-10 scale as a single lumped coolant node. Cold
//! helium is delivered to the top of the core, flows **downward** through the
//! pebble bed picking up the heat the graphite hands it, leaves through the hot
//! gas duct to the steam generator, and returns to the core inlet -- closing
//! the loop.
//!
//! This module used to model a **prismatic-block** core with machined coolant
//! channels. It no longer does: the channel geometry, its wall roughness and
//! its Haaland pipe friction have been removed, because none of them describe a
//! packed bed. What replaced them is documented honestly below -- in particular
//! the pressure drop is **not** a packed-bed friction correlation.
//!
//! ## Flow path (as built, published arrangement)
//!
//! Cold helium from the circulator rises through channels in the **side
//! reflector**, reverses at the top of the core, passes **down** through the
//! pebble bed into a hot gas plenum in the bottom reflector, and leaves through
//! the hot gas duct to the steam generator, which sits in a **separate
//! pressure vessel** alongside the reactor. The model is a single node, so it
//! carries the *direction* and the *end states* of that path, not its spatial
//! detail.
//!
//! ## Nodalisation -- read this first
//!
//! **The entire helium circuit is ONE control volume**, with two temperatures
//! carried at its boundaries rather than a mesh through it:
//!
//! | Region | Nodes | What is assumed uniform inside |
//! |---|---|---|
//! | Helium through the bed | **1** | one `c_p` and one density, both at the bulk mean `(T_in + T_out)/2` |
//! | Hot gas duct, plenums, SG shell, return leg | **0** | collapsed into one first-order transport lag |
//! | Steam generator, helium side | **1** (an effectiveness-NTU lump) | one `UA`, one isothermal cold side |
//! | Reflector cooling channels | **0** | not modelled |
//!
//! The core inlet and core outlet temperatures are the **boundary values of
//! that one node**, related by an energy balance, each relaxed by its own
//! first-order lag: the outlet by the gas thermal inertia
//! ([`CORE_THERMAL_TIME_CONSTANT_S`]), the inlet by the return transport lag
//! ([`RETURN_TRANSPORT_TIME_CONSTANT_S`]). Neither lag comes from a resolved
//! volume; both are stand-ins for one.
//!
//! **What that costs.** There is no axial helium temperature profile through
//! the bed, so no local heat flux, no local Reynolds number, and no place to
//! evaluate a bed friction or Nusselt correlation even if one existed. There is
//! no gas momentum equation, so the pressure drop cannot feed back on the flow
//! and there is **no natural circulation** -- with the circulator stopped this
//! model has no decay-heat removal path at all, which is precisely the HTR-10
//! behaviour a reader might most want and the one it cannot answer. There is no
//! separate reflector-channel leg, so the published cold-helium-rises-in-the-
//! side-reflector path is documented but not resolved.
//!
//! **The refinement path**: split the bed helium axially into the same stack of
//! control volumes as [`super::pebble_bed`], marching downward and exchanging
//! with the matching bed node -- one change buys both the gradient and a place
//! to put a real correlation. Then give the steam generator three zones
//! (economiser / evaporator / superheater) instead of one `UA`. A momentum
//! equation with a buoyancy term, needed for natural circulation, comes after
//! both.
//!
//! ## What is real
//!
//! - **The operating point is the published HTR-10 one**, from IAEA-TECDOC-1382
//!   (ingested at
//!   `crates/kovan-literature/generated/markdown/open/iaea-tecdoc-1382-part2.md`):
//!   10 MWth, primary helium 3.0 MPa, 250 degC core inlet, 700 degC core
//!   outlet, 4.3 kg/s at full power, and a circulator pressure head of
//!   0.06 MPa at that flow.
//! - **Helium properties are real and temperature-dependent.** `c_p` and
//!   density come from the CoolProp-derived Helmholtz EOS
//!   ([`outram_park_fork_coolprop::state_pt`], helium from Ortiz-Vega et al.)
//!   evaluated at the loop pressure and the current bulk mean helium
//!   temperature, re-evaluated every step -- not a frozen constant.
//! - **The core heat input now comes from the graphite**, not straight from the
//!   fission power: [`super::pebble_bed::PebbleBedCore`] holds the bed's 9 MJ/K
//!   of thermal inertia and hands this loop the heat rate that actually crosses
//!   the pebble surface.
//! - **The loop is closed.** The core inlet temperature is *computed* as the
//!   steam-generator helium-side outlet, relaxed through the return transport
//!   lag; it is not pinned to a fixed number.
//! - **The steam generator is pinch-limited** by an effectiveness-NTU model
//!   against the secondary saturation temperature, so its duty cannot exceed
//!   what the temperature difference and `UA` support (see [`Self::step`]).
//! - **The helium inventory is a real gas mass** `rho V` evaluated from the EOS
//!   density over the bed void volume derived from the published core geometry,
//!   plus an illustrative allowance for the rest of the circuit. That inventory
//!   is what sets the residence time driving the schematic's flow tracers.
//!
//! ## What is still illustrative
//!
//! - **The pressure drop is NOT a packed-bed correlation.** It is a quadratic
//!   loss `dp = dp_nominal (m_dot/m_dot_nom)^2 (rho_ref/rho)` anchored to the
//!   published 0.06 MPa circulator head. The quadratic form is the
//!   high-Reynolds limit that a packed-bed correlation tends toward, but no
//!   friction factor is evaluated: **KTA and Ergun are both still absent from
//!   this workspace**, and `docs/reactor-scoping/htr10.md` records that gap as
//!   open. Do not read the pressure drop as a bed friction result.
//! - **The steam generator is one effectiveness-NTU lump.** The published unit
//!   is a once-through helical-tube module; there is no three-zone moving
//!   boundary, no helical correlation, and no tube geometry here. The `UA` is
//!   an illustrative value chosen to place the settled loop near the published
//!   250/700 degC end states.
//! - **Piping and plenum geometry is invented.** See the `ILLUSTRATIVE
//!   GEOMETRY` block below -- IAEA-TECDOC-1382 is a neutronics benchmark and
//!   carries no plant piping. Replacing these with sourced figures is tracked
//!   as bead `op-szmi.6`.
//! - **Helium viscosity is not used at all** any more (the removed pipe
//!   friction was its only consumer), so the loop no longer carries the
//!   hardcoded viscosity constant it used to.
//! - The loop remains a **single lumped node**, not a nodalised fluid array:
//!   there is no axial helium temperature profile through the bed.
//!
//! This is a demonstration model, **not a validated HTR-10 primary-loop
//! model**.

use outram_park_fork_coolprop::{state_pt, Fluid};
use uom::si::f64::{
    Mass, MassRate, Power, Pressure, SpecificHeatCapacity, ThermodynamicTemperature, Time, Volume,
};
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volume::cubic_meter;

use super::pebble_bed;

// ---------------------------------------------------------------------------
// PUBLISHED HTR-10 OPERATING POINT
// IAEA-TECDOC-1382, Table 4-1 and section 4.1. These are sourced figures.
// ---------------------------------------------------------------------------

/// Primary helium pressure \[Pa\]: 3.0 MPa (published).
const LOOP_PRESSURE_PA: f64 = 3.0e6;

/// Published core inlet helium temperature \[K\]: 250 degC.
const PUBLISHED_CORE_INLET_K: f64 = 523.15;

/// Published core outlet helium temperature \[K\]: 700 degC.
const PUBLISHED_CORE_OUTLET_K: f64 = 973.15;

/// Published circulator pressure head \[Pa\]: 0.06 MPa, quoted at 4.3 kg/s,
/// 3.0 MPa and 250 degC. Used as the anchor for the quadratic loop loss.
const NOMINAL_PRESSURE_DROP_PA: f64 = 6.0e4;

/// Helium temperature the circulator head is quoted at \[K\]: 250 degC
/// (published). The reference density for the quadratic loss is evaluated here.
const PRESSURE_DROP_REFERENCE_TEMPERATURE_K: f64 = 523.15;

// ---------------------------------------------------------------------------
// ILLUSTRATIVE GEOMETRY AND CLOSURES -- INVENTED PLACEHOLDERS, NOT PUBLISHED
//
// IAEA-TECDOC-1382 is a reactor-physics benchmark: it carries the core, the
// operating point and the vessel envelopes, but no primary piping, no hot gas
// duct bore, and no steam-generator tube geometry. Everything in this block is
// a plausible stand-in chosen to keep the model dimensionally sane and in the
// right numeric range. None of it is design data. Replacing these with sourced
// figures is tracked as bead `op-szmi.6`.
// ---------------------------------------------------------------------------

/// Helium-filled volume of the circuit **outside** the pebble bed \[m^3\]
/// (**invented**): the upper and lower plenums, the hot gas duct, the
/// steam-generator shell side and the circulator casing, lumped into one
/// number. The bed's own void volume is derived from the published core
/// geometry and added to this -- see [`Self::helium_inventory`].
const LOOP_GAS_VOLUME_OUTSIDE_BED_M3: f64 = 6.0;

/// Circulator isentropic/mechanical efficiency (**invented**), 0.80.
const CIRCULATOR_EFFICIENCY: f64 = 0.80;

/// Steam-generator overall conductance `UA` \[W/K\] (**invented**), chosen so
/// the settled loop sits near the published 250 degC / 700 degC end states at
/// 10 MWth and 4.3 kg/s. It stands in for a once-through helical-tube module
/// whose tube diameter, wall, pitch and module count are all unknown here.
const STEAM_GENERATOR_UA_W_PER_K: f64 = 1.0e5;

/// Thermal-inertia time constant of the lumped helium node in the core \[s\]
/// (**invented**), giving the core-outlet temperature a visible first-order
/// transient rather than an instantaneous jump. This is the *gas* inertia; the
/// graphite's much larger inertia lives in
/// [`super::pebble_bed::PebbleBedCore`].
const CORE_THERMAL_TIME_CONSTANT_S: f64 = 5.0;

/// Transport lag from the steam-generator helium outlet round to the core inlet
/// \[s\] (**invented**), so a change in secondary heat removal reaches the core
/// inlet with a delay rather than instantly.
const RETURN_TRANSPORT_TIME_CONSTANT_S: f64 = 8.0;

/// Floor on the commanded helium flow \[kg/s\] (**invented**), keeping the
/// energy-balance denominator and the residence time finite when the user
/// drives the circulator setpoint to zero. About 7% of nominal -- the published
/// blower regulates down to 30%, so anything below that is already outside the
/// machine's stated range.
const MIN_HELIUM_FLOW_KG_PER_S: f64 = 0.3;

/// Ceiling on the commanded helium flow \[kg/s\] (**invented**), a generous
/// stand-in for the circulator's capacity at roughly 185% of the published
/// 4.3 kg/s. Without it, a control input scaled for the old 200 MWth prismatic
/// plant would command twenty times the nominal flow through a 10 MWth core.
const MAX_HELIUM_FLOW_KG_PER_S: f64 = 8.0;

/// Lumped helium primary-loop state.
pub struct HeliumPrimaryLoop {
    /// Isobaric specific heat of helium at the current bulk mean temperature
    /// (re-evaluated every step from the real EOS).
    c_p: SpecificHeatCapacity,
    /// Helium density at the current bulk mean temperature (real EOS).
    density: f64,
    /// Helium density at the conditions the circulator head is quoted at, used
    /// to scale the quadratic loop loss.
    reference_density: f64,
    /// Current (transient) core-inlet temperature -- the steam-generator helium
    /// outlet after the return transport lag.
    core_inlet_temperature: ThermodynamicTemperature,
    /// Current (transient) core-outlet helium temperature.
    core_outlet_temperature: ThermodynamicTemperature,
    /// Current helium mass flow (driven by the circulator setpoint).
    mass_flow: MassRate,
    /// Most recently computed steam-generator duty transferred to the secondary
    /// loop.
    ihx_duty: Power,
    /// Helium-side steam-generator outlet temperature (feeds the core inlet).
    ihx_outlet_temperature: ThermodynamicTemperature,
    /// Frictional pressure drop around the loop at the current flow.
    pressure_drop: Pressure,
    /// Circulator hydraulic power required to sustain that pressure drop.
    circulator_power: Power,
}

impl HeliumPrimaryLoop {
    /// Construct the loop at the published HTR-10 operating point:
    /// `nominal_flow` helium mass flow, core inlet seeded at 250 degC and core
    /// outlet at 700 degC, helium properties evaluated at their mean.
    ///
    /// Seeding at the published end states rather than at a single cold
    /// temperature means the simulator opens near its operating point instead
    /// of spending several minutes of simulated time warming up.
    pub fn new(nominal_flow: MassRate) -> Self {
        let inlet = ThermodynamicTemperature::new::<kelvin>(PUBLISHED_CORE_INLET_K);
        let outlet = ThermodynamicTemperature::new::<kelvin>(PUBLISHED_CORE_OUTLET_K);
        let (c_p, density) =
            helium_properties(0.5 * (PUBLISHED_CORE_INLET_K + PUBLISHED_CORE_OUTLET_K));
        let (_, reference_density) = helium_properties(PRESSURE_DROP_REFERENCE_TEMPERATURE_K);

        Self {
            c_p,
            density,
            reference_density,
            core_inlet_temperature: inlet,
            core_outlet_temperature: outlet,
            mass_flow: nominal_flow,
            ihx_duty: Power::new::<watt>(0.0),
            ihx_outlet_temperature: inlet,
            pressure_drop: Pressure::new::<pascal>(0.0),
            circulator_power: Power::new::<watt>(0.0),
        }
    }

    /// Advance the loop by `dt`.
    ///
    /// `core_heat_to_helium` is the heat rate crossing the **pebble surface**
    /// into the helium, as returned by
    /// [`super::pebble_bed::PebbleBedCore::step`] -- not the raw fission power.
    /// Routing the fission power through the graphite first is what gives the
    /// loop the bed's thermal inertia.
    ///
    /// `flow_setpoint` is the commanded circulator flow, clamped to the
    /// circulator's illustrative range. `secondary_sink_temperature` is the
    /// steam-side saturation temperature the steam generator pinches against.
    ///
    /// The step, in order:
    ///
    /// 1. Helium `c_p` and density are re-evaluated from the real EOS at the
    ///    current bulk mean temperature `(T_in + T_out)/2`.
    /// 2. Core energy balance: steady-state outlet `T_in + Q/(m_dot c_p)`,
    ///    with the displayed outlet relaxed toward it over
    ///    [`CORE_THERMAL_TIME_CONSTANT_S`].
    /// 3. Steam generator, effectiveness-NTU with one isothermal (boiling)
    ///    side, so `C_min = m_dot c_p` and `eps = 1 - exp(-UA/C_min)`:
    ///    `Q = eps * m_dot * c_p * (T_out - T_sink)`. This is the pinch limit --
    ///    the duty vanishes as the helium approaches the secondary saturation
    ///    temperature, and can never drive it below.
    /// 4. Helium leaves at `T_out - Q/(m_dot c_p)`, which the core inlet relaxes
    ///    toward over [`RETURN_TRANSPORT_TIME_CONSTANT_S`], closing the loop.
    /// 5. Quadratic loop pressure drop and circulator hydraulic power at the
    ///    current flow and density.
    pub fn step(
        &mut self,
        dt: Time,
        core_heat_to_helium: Power,
        flow_setpoint: MassRate,
        secondary_sink_temperature: ThermodynamicTemperature,
    ) {
        // Clamp the commanded flow to the circulator's range so the energy
        // balance denominator and the residence time never blow up, and so a
        // setpoint scaled for a different plant cannot drive this one.
        let flow_kg_s = flow_setpoint
            .get::<kilogram_per_second>()
            .clamp(MIN_HELIUM_FLOW_KG_PER_S, MAX_HELIUM_FLOW_KG_PER_S);
        self.mass_flow = MassRate::new::<kilogram_per_second>(flow_kg_s);

        // 1. Real helium properties at the current bulk mean temperature.
        let t_in_k = self.core_inlet_temperature.get::<kelvin>();
        let t_out_k = self.core_outlet_temperature.get::<kelvin>();
        let (c_p, density) = helium_properties(0.5 * (t_in_k + t_out_k));
        self.c_p = c_p;
        self.density = density;
        let c_p_j = self.c_p.get::<joule_per_kilogram_kelvin>();

        let dt_s = dt.get::<second>();
        let capacity_rate = flow_kg_s * c_p_j; // C_min = m_dot c_p [W/K]

        // 2. Core energy balance with first-order gas thermal inertia.
        let t_out_ss_k = t_in_k + core_heat_to_helium.get::<watt>() / capacity_rate;
        let alpha_core = (dt_s / CORE_THERMAL_TIME_CONSTANT_S).clamp(0.0, 1.0);
        let t_out_next_k = t_out_k + alpha_core * (t_out_ss_k - t_out_k);
        self.core_outlet_temperature = ThermodynamicTemperature::new::<kelvin>(t_out_next_k);

        // 3. Steam-generator duty, effectiveness-NTU against an isothermal
        //    secondary side.
        let t_sink_k = secondary_sink_temperature.get::<kelvin>();
        let ntu = STEAM_GENERATOR_UA_W_PER_K / capacity_rate;
        let effectiveness = 1.0 - (-ntu).exp();
        let duty_w = (effectiveness * capacity_rate * (t_out_next_k - t_sink_k)).max(0.0);
        self.ihx_duty = Power::new::<watt>(duty_w);

        // 4. Helium leaves the steam generator cooled by that duty; the core
        //    inlet relaxes toward it through the return transport lag.
        let t_ihx_out_k = t_out_next_k - duty_w / capacity_rate;
        self.ihx_outlet_temperature = ThermodynamicTemperature::new::<kelvin>(t_ihx_out_k);
        let alpha_return = (dt_s / RETURN_TRANSPORT_TIME_CONSTANT_S).clamp(0.0, 1.0);
        let t_in_next_k = t_in_k + alpha_return * (t_ihx_out_k - t_in_k);
        self.core_inlet_temperature = ThermodynamicTemperature::new::<kelvin>(t_in_next_k);

        // 5. Loop pressure drop and circulator hydraulic power.
        self.update_hydraulics(flow_kg_s);
    }

    /// Quadratic loop pressure drop and the circulator hydraulic power needed
    /// to sustain it.
    ///
    /// `dp = dp_nominal (m_dot/m_dot_nom)^2 (rho_ref/rho)`, anchored to the
    /// published 0.06 MPa circulator head at 4.3 kg/s, 3.0 MPa and 250 degC.
    /// The `m_dot^2/rho` scaling is the fully-turbulent form every loss
    /// correlation shares in that limit; the *coefficient* is the published
    /// head rather than an evaluated friction factor.
    ///
    /// **This is not a packed-bed friction correlation.** Neither KTA nor Ergun
    /// exists in this workspace -- see the module docs and
    /// `docs/reactor-scoping/htr10.md`.
    ///
    /// Circulator power is `m_dot dp / (rho eta)`.
    fn update_hydraulics(&mut self, flow_kg_s: f64) {
        let rho = self.density;
        if !(rho > 0.0) || !(self.reference_density > 0.0) {
            self.pressure_drop = Pressure::new::<pascal>(0.0);
            self.circulator_power = Power::new::<watt>(0.0);
            return;
        }

        let flow_ratio = flow_kg_s / pebble_bed::NOMINAL_HELIUM_FLOW_KG_PER_S;
        let dp =
            NOMINAL_PRESSURE_DROP_PA * flow_ratio * flow_ratio * (self.reference_density / rho);

        self.pressure_drop = Pressure::new::<pascal>(dp);
        self.circulator_power = Power::new::<watt>(flow_kg_s * dp / (rho * CIRCULATOR_EFFICIENCY));
    }

    /// Total helium-filled volume of the primary circuit: the bed void volume
    /// derived from the published core geometry, plus the illustrative
    /// allowance for the plenums, duct, steam-generator shell and circulator.
    pub fn gas_volume(&self) -> Volume {
        pebble_bed::bed_void_volume() + Volume::new::<cubic_meter>(LOOP_GAS_VOLUME_OUTSIDE_BED_M3)
    }

    /// Helium inventory held in the circuit, `rho V` from the real EOS density.
    pub fn helium_inventory(&self) -> Mass {
        Mass::new::<kilogram>(self.density * self.gas_volume().get::<cubic_meter>())
    }

    /// Current (transient) core-inlet helium temperature -- the steam-generator
    /// helium outlet after the return transport lag.
    pub fn core_inlet_temperature(&self) -> ThermodynamicTemperature {
        self.core_inlet_temperature
    }

    /// Current (transient) core-outlet helium temperature.
    pub fn core_outlet_temperature(&self) -> ThermodynamicTemperature {
        self.core_outlet_temperature
    }

    /// Bulk mean helium temperature in the core, `(T_in + T_out)/2` -- the
    /// coolant temperature the pebble bed exchanges heat with.
    pub fn helium_bulk_temperature(&self) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(
            0.5 * (self.core_inlet_temperature.get::<kelvin>()
                + self.core_outlet_temperature.get::<kelvin>()),
        )
    }

    /// Helium-side steam-generator outlet temperature.
    pub fn ihx_outlet_temperature(&self) -> ThermodynamicTemperature {
        self.ihx_outlet_temperature
    }

    /// Current helium mass flow.
    pub fn mass_flow(&self) -> MassRate {
        self.mass_flow
    }

    /// Most recently computed steam-generator duty transferred to the secondary
    /// loop.
    pub fn ihx_duty(&self) -> Power {
        self.ihx_duty
    }

    /// Isobaric specific heat of helium at the current bulk mean temperature.
    pub fn specific_heat(&self) -> SpecificHeatCapacity {
        self.c_p
    }

    /// Helium density at the current bulk mean temperature \[kg/m^3\], from the
    /// real EOS.
    #[allow(dead_code)] // snapshot candidate -- not yet wired into the app layer
    pub fn density(&self) -> f64 {
        self.density
    }

    /// Frictional pressure drop around the loop at the current flow.
    pub fn pressure_drop(&self) -> Pressure {
        self.pressure_drop
    }

    /// Circulator hydraulic power required to sustain [`Self::pressure_drop`].
    pub fn circulator_power(&self) -> Power {
        self.circulator_power
    }
}

/// Real helium isobaric specific heat and density at temperature `t_k` \[K\]
/// and the loop pressure, from the CoolProp-derived Helmholtz EOS.
///
/// Falls back to the ideal-gas-limit helium values (`c_p = 5193 J/(kg K)`,
/// density from `p/(R_specific T)` with `R_specific = 2077 J/(kg K)`) if the
/// `(p, T)` density solve fails to converge -- helium at HTGR conditions is
/// close to ideal, so the fallback is a physically sane bound rather than a
/// fabricated number, and it keeps a GUI frame from panicking on a transient.
fn helium_properties(t_k: f64) -> (SpecificHeatCapacity, f64) {
    /// Ideal-gas-limit helium `c_p` \[J/(kg K)\].
    const IDEAL_CP: f64 = 5193.0;
    /// Specific gas constant of helium \[J/(kg K)\].
    const R_SPECIFIC: f64 = 2077.0;

    let t = if t_k.is_finite() && t_k > 1.0 {
        t_k
    } else {
        PUBLISHED_CORE_INLET_K
    };

    match state_pt(Fluid::Helium, t, LOOP_PRESSURE_PA) {
        Ok(state) if state.cp.is_finite() && state.cp > 0.0 && state.density > 0.0 => (
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(state.cp),
            state.density,
        ),
        _ => (
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(IDEAL_CP),
            LOOP_PRESSURE_PA / (R_SPECIFIC * t),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::power::megawatt;

    fn nominal_loop() -> HeliumPrimaryLoop {
        HeliumPrimaryLoop::new(MassRate::new::<kilogram_per_second>(
            pebble_bed::NOMINAL_HELIUM_FLOW_KG_PER_S,
        ))
    }

    fn nominal_flow() -> MassRate {
        MassRate::new::<kilogram_per_second>(pebble_bed::NOMINAL_HELIUM_FLOW_KG_PER_S)
    }

    fn sink(t_k: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(t_k)
    }

    /// Methodology: helium `c_p` from the ported Helmholtz EOS is compared
    /// against the ideal-gas-limit value `5R/2M = 5193 J/(kg K)`, which real
    /// helium approaches closely at HTR-10 conditions (3.0 MPa, 523-973 K).
    /// Pass criterion: within 10% of the ideal limit, and strictly positive
    /// density.
    ///
    /// Results (2026-08-12, CoolProp-fork helium EOS, Ortiz-Vega et al.), at
    /// the published 3.0 MPa:
    ///
    /// | T \[K\] | `c_p` \[J/(kg K)\] | vs ideal limit | `rho` \[kg/m^3\] |
    /// |---|---|---|---|
    /// | 523.15 | 5191.58 | -0.027% | 2.73999 |
    /// | 748.15 | 5191.45 | -0.030% | 1.92094 |
    /// | 973.15 | 5191.62 | -0.027% | 1.47878 |
    ///
    /// `c_p` sits just *below* the ideal-gas limit across the range and the
    /// density falls as `1/T` would suggest. The values differ from the 5193
    /// constant, which confirms the EOS path is live rather than silently
    /// falling through to the fallback.
    #[test]
    fn helium_properties_are_near_the_ideal_gas_limit() {
        for t_k in [523.15, 748.15, 973.15] {
            let (c_p, density) = helium_properties(t_k);
            let cp_val = c_p.get::<joule_per_kilogram_kelvin>();
            assert!(
                (cp_val - 5193.0).abs() / 5193.0 < 0.10,
                "helium c_p {cp_val} at {t_k} K is not within 10% of the ideal limit"
            );
            assert!(density > 0.0, "helium density must be positive at {t_k} K");
        }
    }

    /// Methodology: the published HTR-10 primary-side figures must be mutually
    /// consistent under a plain energy balance. The report states, separately,
    /// 10 MWth, a helium mass flow of 4.3 kg/s at full power, a 250 degC core
    /// inlet and a 700 degC core outlet. Those four numbers over-determine the
    /// loop: the core temperature rise implied by `Q/(m_dot c_p)`, using the
    /// EOS `c_p` at the 3.0 MPa loop pressure and the bulk mean temperature,
    /// must reproduce the published 450 K rise. Pass criterion: within 5%.
    ///
    /// Results (2026-08-12): `c_p = 5191.4511 J/(kg K)` at 3.0 MPa and the
    /// 748.15 K bulk mean, giving `dT = 10e6/(4.3 x 5191.4511) = 447.96 K`
    /// against the published `700 - 250 = 450 K` -- **-0.45%**.
    ///
    /// Interpretation: this is a real check on published data, and it passes.
    /// The four figures are consistent with each other and with a real helium
    /// equation of state to better than half a percent. It verifies the
    /// operating point transcribed into this module; it does not validate the
    /// model built on top of it.
    #[test]
    fn published_operating_point_closes_on_the_energy_balance() {
        let bulk_mean_k = 0.5 * (PUBLISHED_CORE_INLET_K + PUBLISHED_CORE_OUTLET_K);
        let (c_p, _) = helium_properties(bulk_mean_k);
        let rise = 1.0e7
            / (pebble_bed::NOMINAL_HELIUM_FLOW_KG_PER_S * c_p.get::<joule_per_kilogram_kelvin>());
        let published_rise = PUBLISHED_CORE_OUTLET_K - PUBLISHED_CORE_INLET_K;
        assert!(
            (rise - published_rise).abs() / published_rise < 0.05,
            "energy-balance rise {rise} K departs from the published {published_rise} K"
        );
    }

    /// Methodology: the effectiveness-NTU steam generator must respect the
    /// second law in both directions. The invariant is conditional on which way
    /// heat can flow, checked at every one of 4000 steps of 0.05 s at 10 MWth
    /// against a 523.5 K sink (the IF97 saturation temperature at the published
    /// 4.0 MPa steam pressure):
    ///
    /// - while the helium is **hotter** than the sink, the steam generator may
    ///   cool it but never past the sink: `T_sink <= T_sg_out <= T_out`;
    /// - while the helium is **colder** than the sink, it transfers nothing
    ///   rather than heating the helium backwards, so `T_sg_out == T_out`.
    ///
    /// Results (2026-08-12): the transfer branch held at every step. Seeded at
    /// the published end states, the loop starts already above the sink, so the
    /// no-transfer branch is not exercised on this run (it is on a cold start,
    /// which is why the branch is kept). After 400 s of simulated time at
    /// 10 MWth the loop settled at `T_in = 528.64 K` (255.5 degC),
    /// `T_out = 976.60 K` (703.5 degC), `T_sg_out = 528.64 K` -- a 5.14 K
    /// approach above the sink. Those settled end states sit within 6 K of the
    /// published 250 degC / 700 degC, but note the steam-generator `UA` was
    /// *chosen* to put them there, so this is a calibration, not a prediction.
    /// What is *not* calibrated is the 447.96 K rise between them, which is the
    /// energy balance on published figures -- see
    /// `published_operating_point_closes_on_the_energy_balance`.
    #[test]
    fn steam_generator_respects_the_pinch_in_both_directions() {
        let mut loop_ = nominal_loop();
        let t_sink = sink(523.5);
        let t_sink_k = t_sink.get::<kelvin>();

        for _ in 0..4000 {
            loop_.step(
                Time::new::<second>(0.05),
                Power::new::<megawatt>(10.0),
                nominal_flow(),
                t_sink,
            );
            let t_out = loop_.core_outlet_temperature().get::<kelvin>();
            let t_sg = loop_.ihx_outlet_temperature().get::<kelvin>();

            if t_out > t_sink_k {
                assert!(
                    t_sg >= t_sink_k - 1e-9,
                    "the steam generator cooled the helium ({t_sg} K) past the {t_sink_k} K sink"
                );
                assert!(
                    t_sg <= t_out + 1e-9,
                    "the steam generator heated the helium ({t_sg} K) above its inlet {t_out} K"
                );
            } else {
                assert!(
                    (t_sg - t_out).abs() < 1e-9,
                    "heat moved backwards: helium {t_out} K below the sink {t_sink_k} K \
                     but left the steam generator at {t_sg} K"
                );
            }
        }

        assert!(
            loop_.core_outlet_temperature().get::<kelvin>() > t_sink_k,
            "the loop should settle hotter than the secondary sink at load"
        );
    }

    /// The core inlet must be a computed loop variable, not a fixed constant:
    /// cutting secondary heat removal (raising the sink temperature) must raise
    /// the core inlet.
    #[test]
    fn core_inlet_responds_to_secondary_heat_removal() {
        let mut cold_sink = nominal_loop();
        let mut hot_sink = nominal_loop();
        for _ in 0..4000 {
            let dt = Time::new::<second>(0.05);
            let q = Power::new::<megawatt>(10.0);
            cold_sink.step(dt, q, nominal_flow(), sink(500.0));
            hot_sink.step(dt, q, nominal_flow(), sink(600.0));
        }
        assert!(
            hot_sink.core_inlet_temperature().get::<kelvin>()
                > cold_sink.core_inlet_temperature().get::<kelvin>(),
            "a hotter secondary sink must raise the core inlet temperature"
        );
    }

    /// Methodology: once the transient has settled, the core temperature rise
    /// must follow the energy balance `dT = Q/(m_dot c_p)` using the *live*
    /// helium `c_p`. Pass criterion: within 2% of that balance.
    ///
    /// Results (2026-08-12): at 10 MWth and 4.3 kg/s the measured rise was
    /// `976.6005 - 528.6371 = 447.9635 K`, against
    /// `10e6/(4.3 x 5191.4532) = 447.9635 K` from the balance (the live `c_p`
    /// at the settled 752.6 K bulk mean) -- agreement to better than 0.001%.
    #[test]
    fn core_temperature_rise_matches_the_energy_balance() {
        let mut loop_ = nominal_loop();
        let power = Power::new::<megawatt>(10.0);
        for _ in 0..8000 {
            loop_.step(
                Time::new::<second>(0.05),
                power,
                nominal_flow(),
                sink(523.5),
            );
        }

        let measured = loop_.core_outlet_temperature().get::<kelvin>()
            - loop_.core_inlet_temperature().get::<kelvin>();
        let expected = power.get::<watt>()
            / (loop_.mass_flow().get::<kilogram_per_second>()
                * loop_.specific_heat().get::<joule_per_kilogram_kelvin>());
        assert!(
            (measured - expected).abs() / expected < 0.02,
            "core rise {measured} K departs from the energy balance {expected} K"
        );
    }

    /// Methodology: the quadratic loop loss is anchored to the **published**
    /// circulator pressure head of 0.06 MPa, quoted at 4.3 kg/s, 3.0 MPa and
    /// 250 degC. Evaluated at exactly those conditions the model must return
    /// exactly that head; away from them it must scale as `m_dot^2/rho`, so
    /// the loss and the circulator power must both rise with flow. Pass
    /// criterion: within 1% of 0.06 MPa at the anchor conditions, and strict
    /// monotonicity between a 2.0 kg/s and a 6.0 kg/s case.
    ///
    /// Results (2026-08-12): at the anchor conditions the model returned
    /// **60.000 kPa** against the published 60 kPa -- exact by construction, so
    /// this checks the wiring, not the physics. At the settled nominal
    /// operating point the bulk mean helium is 752.6 K and therefore less dense
    /// than at the 250 degC anchor, so the same quadratic law gives
    /// `dp = 86.09 kPa`, 1.43x the published cold-leg head, with
    /// `W_circulator = 242.3 kW` -- 2.42% of the 10 MWth heat load, a plausible
    /// fraction for a gas-cooled primary circulator. Helium inventory at that
    /// point 15.19 kg, giving a 3.53 s loop residence time. The 6.0 kg/s case
    /// exceeded the 2.0 kg/s case on both measures.
    ///
    /// Interpretation: this verifies the anchor and the scaling shape. It is
    /// **not** a packed-bed friction result -- no KTA or Ergun correlation
    /// exists in this workspace (see the module docs).
    #[test]
    fn pressure_drop_is_anchored_to_the_published_circulator_head() {
        // At the anchor conditions the quadratic law must return the published
        // head exactly.
        let mut anchored = nominal_loop();
        anchored.density = anchored.reference_density;
        anchored.update_hydraulics(pebble_bed::NOMINAL_HELIUM_FLOW_KG_PER_S);
        let dp_anchor = anchored.pressure_drop().get::<pascal>();
        assert!(
            (dp_anchor - NOMINAL_PRESSURE_DROP_PA).abs() / NOMINAL_PRESSURE_DROP_PA < 0.01,
            "at the anchor conditions dp = {dp_anchor} Pa, not the published 60 kPa"
        );

        let mut slow = nominal_loop();
        let mut fast = nominal_loop();
        for _ in 0..400 {
            let dt = Time::new::<second>(0.05);
            let q = Power::new::<megawatt>(10.0);
            slow.step(
                dt,
                q,
                MassRate::new::<kilogram_per_second>(2.0),
                sink(523.5),
            );
            fast.step(
                dt,
                q,
                MassRate::new::<kilogram_per_second>(6.0),
                sink(523.5),
            );
        }
        assert!(slow.pressure_drop().get::<pascal>() > 0.0);
        assert!(fast.pressure_drop().get::<pascal>() > slow.pressure_drop().get::<pascal>());
        assert!(fast.circulator_power().get::<watt>() > slow.circulator_power().get::<watt>());
    }

    /// The helium inventory must be positive so the residence time driving the
    /// schematic's flow tracers is finite, and it must include the bed void
    /// volume derived from the published core geometry.
    #[test]
    fn helium_inventory_includes_the_bed_void_volume() {
        let loop_ = nominal_loop();
        assert!(loop_.helium_inventory().get::<kilogram>() > 0.0);

        let total = loop_.gas_volume().get::<cubic_meter>();
        let bed = pebble_bed::bed_void_volume().get::<cubic_meter>();
        assert!(bed > 1.9 && bed < 2.0, "bed void volume {bed} m^3 is off");
        assert!(
            (total - bed - LOOP_GAS_VOLUME_OUTSIDE_BED_M3).abs() < 1e-9,
            "the circuit gas volume must be the bed void plus the illustrative allowance"
        );
    }

    /// The circulator flow clamp must hold the commanded flow inside the
    /// machine's illustrative range in both directions -- in particular a
    /// setpoint left over from the old 200 MWth prismatic plant (85 kg/s) must
    /// not drive a 10 MWth core.
    #[test]
    fn commanded_flow_is_clamped_to_the_circulator_range() {
        let mut too_fast = nominal_loop();
        too_fast.step(
            Time::new::<second>(0.05),
            Power::new::<megawatt>(10.0),
            MassRate::new::<kilogram_per_second>(85.0),
            sink(523.5),
        );
        assert!(
            (too_fast.mass_flow().get::<kilogram_per_second>() - MAX_HELIUM_FLOW_KG_PER_S).abs()
                < 1e-9
        );

        let mut stopped = nominal_loop();
        stopped.step(
            Time::new::<second>(0.05),
            Power::new::<megawatt>(10.0),
            MassRate::new::<kilogram_per_second>(0.0),
            sink(523.5),
        );
        assert!(
            (stopped.mass_flow().get::<kilogram_per_second>() - MIN_HELIUM_FLOW_KG_PER_S).abs()
                < 1e-9
        );
    }
}
