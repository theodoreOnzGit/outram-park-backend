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
//! its Haaland pipe friction were removed, because none of them describe a
//! packed bed. As of 2026-08-12 the bed's share of the loop loss is computed by
//! the **KTA packed-bed correlation** from the workspace library; the rest of
//! the loop is carried as the published component sum. Read the "what is real"
//! and "what is still illustrative" sections below before quoting either.
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
//! the bed, so no local heat flux and no local Reynolds number: the KTA
//! friction factor is evaluated **once, at the bulk mean**, not integrated down
//! a bed whose helium actually runs 250 -> 700 degC. There is
//! no gas momentum equation, so the pressure drop cannot feed back on the flow
//! and there is **no natural circulation** -- with the circulator stopped this
//! model has no decay-heat removal path at all, which is precisely the HTR-10
//! behaviour a reader might most want and the one it cannot answer. There is no
//! separate reflector-channel leg, so the published cold-helium-rises-in-the-
//! side-reflector path is documented but not resolved.
//!
//! **The refinement path**: split the bed helium axially into the same stack of
//! control volumes as [`super::pebble_bed`], marching downward and exchanging
//! with the matching bed node -- one change buys the gradient *and* a place to
//! evaluate the friction factor node by node instead of once at the mean. Then
//! give the steam generator three zones
//! (economiser / evaporator / superheater) instead of one `UA`. A momentum
//! equation with a buoyancy term, needed for natural circulation, comes after
//! both.
//!
//! ## What is real
//!
//! - **The operating point is the published HTR-10 one, read from the
//!   library.** Every published figure comes from
//!   [`outram_park_digital_twin_engine::htr10::design::Htr10DesignPoint`] --
//!   the workspace's single citation-carrying transcription of
//!   IAEA-TECDOC-1382 -- rather than being re-typed here: 10 MWth, primary
//!   helium 3.0 MPa, 250 degC core inlet, 700 degC core outlet, 4.3 kg/s at
//!   full power.
//! - **The bed pressure drop is an evaluated KTA friction result.**
//!   [`bed_pressure_drop`] runs the KTA packed-bed correlation from
//!   [`outram_park_digital_twin_engine::htr10::kta`] on the published pebble
//!   diameter, bed porosity and bed height at the live helium density and
//!   viscosity: a Reynolds number is formed, a friction factor is computed, and
//!   nothing in that term is anchored to a target. The implementation is gated
//!   against the Virtual Test Bed's checked-in worked example -- 3493.17 Pa/m
//!   and 34.9317 kPa against the published gold 3493 Pa/m and 34.93 kPa -- in
//!   [`tests::kta_bed_drop_reproduces_the_vtb_gold_and_is_checked_against_htr10`].
//! - **The rest of the loop is the published component budget**, not an
//!   invention: side-reflector pass 0.7 + mixture plenums 6.1 + steam generator
//!   15.0 + hot gas duct 4.1 = 25.9 kPa at rated flow, from Gao & Shi (2002)
//!   Table 1, whose five components sum to the stated 27.2 kPa total.
//! - **Helium properties are real and temperature-dependent.** `c_p` and
//!   density come from the CoolProp-derived Helmholtz EOS
//!   ([`outram_park_fork_coolprop::state_pt`], helium from Ortiz-Vega et al.)
//!   and the dynamic viscosity the Reynolds number needs from the same crate's
//!   Arp-McCarty-Friend helium transport model, all evaluated at the loop
//!   pressure and the current bulk mean helium temperature and re-evaluated
//!   every step -- not frozen constants.
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
//! - **A real friction correlation is not a resolved bed.** The KTA term is an
//!   evaluated friction result, but it is evaluated **once, on one control
//!   volume**, at the bulk mean density and viscosity of a bed whose real
//!   helium spans 250 to 700 degC. There is still no momentum equation, so the
//!   computed drop **does not feed back on the flow** -- the circulator
//!   setpoint sets the flow outright, and the pressure drop is a reported
//!   consequence, not a constraint. There is still no natural circulation.
//! - **The loop drop is only part-computed.** 25.9 kPa of the roughly 26.4 kPa
//!   total is the published component sum being carried, scaled quadratically
//!   in flow; only about 0.5 kPa of it is computed. Agreement between the
//!   model's loop total and the published 27.2 kPa is therefore mostly
//!   bookkeeping. **Do not read the loop pressure drop as a hydraulic
//!   prediction.**
//! - **The computed bed drop disagrees with the published bed drop.** KTA over
//!   this bed gives 0.504 kPa at the rated point against Gao & Shi's 1.3 kPa
//!   for "pebble bed and bottom reflector" -- 39% of it. The bottom reflector
//!   is not modelled here at all, their calculation is nodalised where this one
//!   is not, and their bed flow is 87.3% of rated against the 86% conservative
//!   fraction used here. The gap is recorded rather than closed; see the V&V
//!   test for the full comparison.
//! - **The steam generator is one effectiveness-NTU lump.** The published unit
//!   is a once-through helical-tube module; there is no three-zone moving
//!   boundary, no helical correlation, and no tube geometry here. The `UA` is
//!   an illustrative value chosen to place the settled loop near the published
//!   250/700 degC end states.
//! - **Piping and plenum geometry is invented.** See the `ILLUSTRATIVE
//!   GEOMETRY` block below -- IAEA-TECDOC-1382 is a neutronics benchmark and
//!   carries no plant piping. Replacing these with sourced figures is tracked
//!   as bead `op-szmi.6`.
//! - The loop remains a **single lumped node**, not a nodalised fluid array:
//!   there is no axial helium temperature profile through the bed.
//!
//! This is a demonstration model, **not a validated HTR-10 primary-loop
//! model**.

use outram_park_digital_twin_engine::htr10::design::{Htr10DesignPoint, Htr10FuelTemperatureLimits};
use outram_park_digital_twin_engine::htr10::kta;
use outram_park_fork_coolprop::{state_pt, viscosity, Fluid};
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{
    DynamicViscosity, Mass, MassDensity, MassRate, Power, Pressure, SpecificHeatCapacity,
    ThermodynamicTemperature, Time, Volume,
};
use uom::si::mass::kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volume::cubic_meter;

use super::pebble_bed;

// ---------------------------------------------------------------------------
// PUBLISHED HTR-10 OPERATING POINT -- READ FROM THE LIBRARY
//
// IAEA-TECDOC-1382, Table 4-1 and section 4.1, transcribed once in
// `outram_park_digital_twin_engine::htr10::design` with a citation per field.
// This module reads that struct rather than keeping its own copy.
// ---------------------------------------------------------------------------

/// The published HTR-10 design point, from the library's single transcription.
fn design() -> Htr10DesignPoint {
    pebble_bed::design()
}

/// Primary helium pressure \[Pa\]: 3.0 MPa (published, via [`design`]).
fn loop_pressure_pa() -> f64 {
    design().primary_pressure.get::<pascal>()
}

/// Published core inlet helium temperature \[K\]: 250 degC (phase-1 operation).
fn published_core_inlet_k() -> f64 {
    design().helium_inlet_phase1.get::<kelvin>()
}

/// Published core outlet helium temperature \[K\]: 700 degC (phase-1
/// operation).
fn published_core_outlet_k() -> f64 {
    design().helium_outlet_phase1.get::<kelvin>()
}

/// Helium temperature the non-bed loop loss is referenced at \[K\]: the
/// published 250 degC cold leg, where the circulator sits. The reference
/// density for the quadratic non-bed loss is evaluated here.
fn pressure_drop_reference_temperature_k() -> f64 {
    published_core_inlet_k()
}

// ---------------------------------------------------------------------------
// PUBLISHED HTR-10 PRIMARY-LOOP PRESSURE BUDGET
//
// Gao & Shi (2002), Nucl. Eng. Des. 218, 51-64, Table 1 (Proprietary tier --
// cited, not re-hosted; the same paper `Htr10FuelTemperatureLimits` cites).
// Reading recorded in `docs/reactor-scoping/htr10-plant-data.md` section 7.5:
//
// | Component                       | kPa  | at kg/s |
// |---------------------------------|------|---------|
// | Pebble bed and bottom reflector |  1.3 | 3.77    |
// | Coolant pass in side reflector  |  0.7 | 3.846   |
// | Flow mixture plenums            |  6.1 | 4.32    |
// | Steam generator                 | 15.0 | 4.32    |
// | Hot gas duct                    |  4.1 | 4.32    |
// | TOTAL                           | 27.2 | 4.32    |
//
// The bed term is now computed by KTA (see `bed_pressure_drop`); the other four
// are carried as the published sum.
// ---------------------------------------------------------------------------

/// Published bed-plus-bottom-reflector pressure drop \[Pa\]: 1.3 kPa at rated
/// flow (Gao & Shi 2002, Table 1). **Reference only** -- the model computes its
/// bed term from KTA and is checked against this figure, not anchored to it.
/// See [`tests::kta_bed_drop_reproduces_the_vtb_gold_and_is_checked_against_htr10`].
const PUBLISHED_BED_AND_BOTTOM_REFLECTOR_DROP_PA: f64 = 1.3e3;

/// Published total primary-loop resistance \[Pa\] at rated flow: 27.2 kPa
/// (Gao & Shi 2002, Table 1). **Reference only.**
const PUBLISHED_LOOP_TOTAL_DROP_PA: f64 = 27.2e3;

/// Published sum of the loop components this model does **not** resolve
/// \[Pa\]: side-reflector pass 0.7 + mixture plenums 6.1 + steam generator 15.0
/// + hot gas duct 4.1 = 25.9 kPa at rated flow (Gao & Shi 2002, Table 1). The
/// steam generator alone is 15.0 of it, so the bed is a small part of this loop
/// and a bed correlation cannot be expected to reproduce the loop head.
const PUBLISHED_NON_BED_DROP_AT_RATED_PA: f64 =
    PUBLISHED_LOOP_TOTAL_DROP_PA - PUBLISHED_BED_AND_BOTTOM_REFLECTOR_DROP_PA;

/// Circulator **design head** \[Pa\]: 0.6 bar (Qin Zhenya 1996, JAERI-Conf
/// 96-010 section 5, Open tier). This is the machine's stated capability, about
/// 2.2x the computed 27.2 kPa loop resistance -- a design margin. It is
/// deliberately **not** used as the loop's operating pressure drop; conflating
/// the two is the error this module used to make. Recorded so the distinction
/// stays visible.
///
/// Deliberately **not** referenced by the model -- it appears only in the
/// pressure-drop V&V test's reported comparison, which is the point.
#[allow(dead_code)]
const CIRCULATOR_DESIGN_HEAD_PA: f64 = 6.0e4;

/// Fraction of the loop mass flow that passes through the pebble bed itself
/// (dimensionless): the conservative 86% of Gao & Shi (2002), read from
/// [`Htr10FuelTemperatureLimits::min_core_flow_fraction`]. The remainder is
/// control-rod-tube, discharge-tube and gap bypass flow that never sees the
/// bed. Gao & Shi's own Table 1 lists 3.77 kg/s of 4.32 kg/s through the bed
/// (87.3%), so 86% is the conservative end of their own numbers.
fn core_flow_fraction() -> f64 {
    Htr10FuelTemperatureLimits::gao_shi_2002()
        .min_core_flow_fraction
        .get::<ratio>()
}

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
    /// Helium dynamic viscosity at the current bulk mean temperature (real
    /// transport model) -- the KTA Reynolds number consumes it.
    dynamic_viscosity: DynamicViscosity,
    /// Helium density at the published 250 degC cold-leg reference condition,
    /// used to scale the quadratic non-bed loop loss.
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
    /// Frictional pressure drop around the whole loop at the current flow: the
    /// KTA bed term plus the published non-bed remainder.
    pressure_drop: Pressure,
    /// The **pebble-bed** part of that drop alone, from the KTA correlation.
    bed_pressure_drop: Pressure,
    /// Circulator hydraulic power required to sustain the total pressure drop.
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
        let inlet = ThermodynamicTemperature::new::<kelvin>(published_core_inlet_k());
        let outlet = ThermodynamicTemperature::new::<kelvin>(published_core_outlet_k());
        let (c_p, density, dynamic_viscosity) =
            helium_properties(0.5 * (published_core_inlet_k() + published_core_outlet_k()));
        let (_, reference_density, _) = helium_properties(pressure_drop_reference_temperature_k());

        Self {
            c_p,
            density,
            dynamic_viscosity,
            reference_density,
            core_inlet_temperature: inlet,
            core_outlet_temperature: outlet,
            mass_flow: nominal_flow,
            ihx_duty: Power::new::<watt>(0.0),
            ihx_outlet_temperature: inlet,
            pressure_drop: Pressure::new::<pascal>(0.0),
            bed_pressure_drop: Pressure::new::<pascal>(0.0),
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
    /// 5. Loop pressure drop -- KTA over the bed plus the published non-bed
    ///    component sum -- and circulator hydraulic power at the current flow
    ///    and density.
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
        let (c_p, density, dynamic_viscosity) = helium_properties(0.5 * (t_in_k + t_out_k));
        self.c_p = c_p;
        self.density = density;
        self.dynamic_viscosity = dynamic_viscosity;
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

    /// Loop pressure drop -- **KTA over the bed, published sum for the rest** --
    /// and the circulator hydraulic power needed to sustain it.
    ///
    /// Two terms, and they are not the same kind of number:
    ///
    /// 1. **The pebble bed: real.** [`bed_pressure_drop`] evaluates the KTA
    ///    packed-bed correlation
    ///    ([`outram_park_digital_twin_engine::htr10::kta`]) at the current bed
    ///    mass flux, the live helium density and viscosity, the published
    ///    pebble diameter and bed porosity, integrated over the published bed
    ///    height. A friction factor is genuinely evaluated; nothing about this
    ///    term is anchored to a target.
    /// 2. **Everything else: the published sum, scaled.** The side-reflector
    ///    pass, the mixture plenums, the steam generator and the hot gas duct
    ///    total [`PUBLISHED_NON_BED_DROP_AT_RATED_PA`] (25.9 kPa of the 27.2 kPa
    ///    loop) at rated flow, and are carried as
    ///    `dp_non_bed = 25.9 kPa (m_dot/m_dot_nom)^2`. The quadratic is the
    ///    fully-turbulent shape; **no density correction is applied to this
    ///    term**, because the published sum already embeds each component's own
    ///    local temperature (the steam generator and cold legs are cold, the
    ///    duct and plenums hot) and this single-node model resolves only one
    ///    density. Correcting it with the bulk-mean density would inflate the
    ///    cold components by ~40%.
    ///
    /// Circulator power is `m_dot dp_total / (rho eta)` with the illustrative
    /// efficiency [`CIRCULATOR_EFFICIENCY`].
    fn update_hydraulics(&mut self, flow_kg_s: f64) {
        let rho = self.density;
        if !(rho > 0.0) || !(self.reference_density > 0.0) {
            self.pressure_drop = Pressure::new::<pascal>(0.0);
            self.bed_pressure_drop = Pressure::new::<pascal>(0.0);
            self.circulator_power = Power::new::<watt>(0.0);
            return;
        }

        // 1. The bed, by KTA, on the fraction of the loop flow that reaches it.
        let bed_flow = MassRate::new::<kilogram_per_second>(flow_kg_s * core_flow_fraction());
        let bed_dp = bed_pressure_drop(
            bed_flow,
            MassDensity::new::<kilogram_per_cubic_meter>(rho),
            self.dynamic_viscosity,
        );
        self.bed_pressure_drop = bed_dp;

        // 2. The published remainder of the loop, quadratic in flow.
        let flow_ratio = flow_kg_s / pebble_bed::nominal_helium_flow_kg_per_s();
        let non_bed_dp = PUBLISHED_NON_BED_DROP_AT_RATED_PA * flow_ratio * flow_ratio;

        let dp = bed_dp.get::<pascal>() + non_bed_dp;
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

    /// Frictional pressure drop around the **whole loop** at the current flow:
    /// the KTA bed term plus the published non-bed remainder. Not a bed
    /// friction result on its own -- for that, see [`Self::bed_pressure_drop`].
    pub fn pressure_drop(&self) -> Pressure {
        self.pressure_drop
    }

    /// The **pebble-bed** pressure drop alone, from the KTA correlation at the
    /// current bed mass flux and live helium properties. This one *is* an
    /// evaluated packed-bed friction result.
    pub fn bed_pressure_drop(&self) -> Pressure {
        self.bed_pressure_drop
    }

    /// Helium dynamic viscosity at the current bulk mean temperature, from the
    /// CoolProp-derived Arp-McCarty-Friend helium transport model -- the
    /// property the KTA Reynolds number is formed on.
    #[allow(dead_code)] // snapshot candidate -- not yet wired into the app layer
    pub fn dynamic_viscosity(&self) -> DynamicViscosity {
        self.dynamic_viscosity
    }

    /// Circulator hydraulic power required to sustain [`Self::pressure_drop`].
    pub fn circulator_power(&self) -> Power {
        self.circulator_power
    }
}

/// Pebble-bed pressure drop from the **KTA packed-bed correlation**
/// ([`outram_park_digital_twin_engine::htr10::kta`]), for helium at `density`
/// and `dynamic_viscosity` flowing through the bed at `bed_mass_flow`.
///
/// The chain is exactly the one the library's own V&V test exercises against
/// the Virtual Test Bed worked example:
///
/// 1. `G = mdot / A` over the bed's **superficial** (empty-cylinder)
///    cross-section, [`super::pebble_bed::superficial_area`] = 2.545 m^2;
/// 2. `Re = G D_h / mu` on the published 6.0 cm pebble diameter;
/// 3. `psi = 320/(Re/(1-eps)) + 6/(Re/(1-eps))^0.1` at the published bed
///    porosity eps = 0.39;
/// 4. `-dp/dx = psi ((1-eps)/eps^3) G^2 / (2 D_h rho)`, integrated over the
///    published 1.97 m mean bed height.
///
/// **What this is.** An evaluated friction result: the friction factor is
/// computed from a Reynolds number formed on live properties, not read off a
/// target. **What this is not.** A resolved bed. The single node supplies one
/// density and one viscosity for the whole bed, where the real helium runs
/// 250 -> 700 degC top to bottom; the correlation is applied once at the bulk
/// mean rather than integrated down an axial profile. It also covers the bed
/// only -- not the bottom reflector the published 1.3 kPa figure includes.
///
/// **Validity** (KTA, as stated by the VTB source): `Re/(1-eps)` from about 1
/// to 1e5 and porosities near random packing. At the HTR-10 rated point this
/// model sits near `Re/(1-eps)` = 3.8e3, inside that band.
pub fn bed_pressure_drop(
    bed_mass_flow: MassRate,
    density: MassDensity,
    dynamic_viscosity: DynamicViscosity,
) -> Pressure {
    let mass_flux = kta::superficial_mass_flux(bed_mass_flow, pebble_bed::superficial_area());
    let gradient = kta::kta_pressure_gradient(
        mass_flux,
        pebble_bed::pebble_diameter(),
        pebble_bed::bed_porosity(),
        density,
        dynamic_viscosity,
    );
    kta::pressure_drop_over_bed(gradient, pebble_bed::core_mean_height())
}

/// Real helium isobaric specific heat, density and dynamic viscosity at
/// temperature `t_k` \[K\] and the loop pressure, from the CoolProp-derived
/// Helmholtz EOS (Ortiz-Vega et al.) and its helium transport model (Arp,
/// McCarty & Friend, NIST TN-1334).
///
/// Falls back to the ideal-gas-limit helium values (`c_p = 5193 J/(kg K)`,
/// density from `p/(R_specific T)` with `R_specific = 2077 J/(kg K)`) if the
/// `(p, T)` density solve fails to converge -- helium at HTGR conditions is
/// close to ideal, so the fallback is a physically sane bound rather than a
/// fabricated number, and it keeps a GUI frame from panicking on a transient.
/// The viscosity fallback is the **KTA 3102.1** helium fit
/// `mu = 3.674e-7 T^0.7` Pa s (valid 0.1-10 MPa, 293-1773 K; recorded in
/// `docs/reactor-scoping/htr10-plant-data.md` section 7.3 from Gao & Shi 2002),
/// i.e. a cited correlation rather than a made-up number.
fn helium_properties(t_k: f64) -> (SpecificHeatCapacity, f64, DynamicViscosity) {
    /// Ideal-gas-limit helium `c_p` \[J/(kg K)\].
    const IDEAL_CP: f64 = 5193.0;
    /// Specific gas constant of helium \[J/(kg K)\].
    const R_SPECIFIC: f64 = 2077.0;

    /// KTA 3102.1 helium dynamic viscosity \[Pa s\] at temperature `t` \[K\].
    fn kta_helium_viscosity(t: f64) -> DynamicViscosity {
        DynamicViscosity::new::<pascal_second>(3.674e-7 * t.powf(0.7))
    }

    let t = if t_k.is_finite() && t_k > 1.0 {
        t_k
    } else {
        published_core_inlet_k()
    };
    let p = loop_pressure_pa();

    match state_pt(Fluid::Helium, t, p) {
        Ok(state) if state.cp.is_finite() && state.cp > 0.0 && state.density > 0.0 => {
            let mu = viscosity(Fluid::Helium, t, state.density)
                .map(DynamicViscosity::new::<pascal_second>)
                .unwrap_or_else(|| kta_helium_viscosity(t));
            (
                SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(state.cp),
                state.density,
                mu,
            )
        }
        _ => (
            SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(IDEAL_CP),
            p / (R_SPECIFIC * t),
            kta_helium_viscosity(t),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::power::megawatt;

    fn nominal_loop() -> HeliumPrimaryLoop {
        HeliumPrimaryLoop::new(pebble_bed::nominal_helium_flow())
    }

    fn nominal_flow() -> MassRate {
        pebble_bed::nominal_helium_flow()
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
            let (c_p, density, _) = helium_properties(t_k);
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
        let bulk_mean_k = 0.5 * (published_core_inlet_k() + published_core_outlet_k());
        let (c_p, _, _) = helium_properties(bulk_mean_k);
        let rise = 1.0e7
            / (pebble_bed::nominal_helium_flow_kg_per_s() * c_p.get::<joule_per_kilogram_kelvin>());
        let published_rise = published_core_outlet_k() - published_core_inlet_k();
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

    /// V&V: the KTA bed pressure drop reproduces the Virtual Test Bed gold, and
    /// what it then predicts for the HTR-10 bed against the published figure.
    ///
    /// **Methodology, part 1 (the gate).** The Virtual Test Bed generic
    /// pebble-bed tutorial, step 2 (Open tier, CC-BY-4.0;
    /// `reference-data/virtual_test_bed/doc/content/htgr/generic-pbr-tutorial/step2.md`,
    /// recorded in `docs/reactor-scoping/vtb-findings.md` section 5) works the
    /// KTA correlation at `D_h` = 0.06 m, eps = 0.39, rho = 8.628204 kg/m^3,
    /// mu = 1.991242e-5 Pa s, Re = 40125, and states the checked-in gold
    /// answers `dp/dx` = -3493 Pa/m and 34.93 kPa over the 10 m bed (Pronghorn
    /// itself computes 3.4933e4 Pa). This test drives the **same correlation
    /// chain [`bed_pressure_drop`] uses** -- `kta_pressure_gradient` ->
    /// `pressure_drop_over_bed` from
    /// [`outram_park_digital_twin_engine::htr10::kta`] -- with the tutorial's
    /// geometry substituted for the HTR-10's, taking the published Re as the
    /// flow input (`G = Re mu / D_h`). Pass criterion: gradient within 1 Pa/m
    /// of 3493 (source precision, 4 significant figures) and the 10 m drop
    /// within 0.01 kPa of 34.93 kPa.
    ///
    /// **Methodology, part 2 (the HTR-10 prediction, reported not asserted to
    /// a target).** [`bed_pressure_drop`] is then evaluated at the HTR-10 rated
    /// point: 86% of 4.3 kg/s through the 2.545 m^2 bed cross-section, helium
    /// at the 3.0 MPa loop pressure and the 748.15 K published bulk mean, over
    /// the published 1.97 m bed height. The comparator is Gao & Shi (2002)
    /// Table 1, which gives **1.3 kPa** for "pebble bed and bottom reflector"
    /// at rated flow. Pass criterion is deliberately loose (0.1-1.3 kPa, i.e.
    /// same order and not exceeding the published bed-plus-reflector figure) --
    /// the point is to record the disagreement, not to tune it away.
    ///
    /// **Results (recorded 2026-08-12).**
    ///
    /// | Case | Model | Reference | Delta |
    /// |---|---|---|---|
    /// | VTB gold, `dp/dx` | 3493.17 Pa/m | 3493 Pa/m | +0.005% |
    /// | VTB gold, 10 m drop | 34.9317 kPa | 34.93 kPa | +0.005% |
    /// | HTR-10 bed, rated | 0.5041 kPa | 1.3 kPa (bed + bottom reflector) | **-61.2%** |
    ///
    /// At the HTR-10 rated point the model evaluates `G` = 1.4532 kg/(m^2 s),
    /// `Re` = 2315.7, `Re/(1-eps)` = 3796.2 (inside the KTA validity band),
    /// `psi` = 2.7159, on helium at rho = 1.9209 kg/m^3 and mu = 3.7653e-5
    /// Pa s, giving `|dp/dx|` = 255.9 Pa/m over the 1.97 m bed.
    ///
    /// **Interpretation -- the gate passes, the plant comparison does not
    /// agree, and that is a finding, not a defect to tune out.** The
    /// correlation implementation is verified to the published gold's every
    /// quoted digit. Against the plant, KTA over the HTR-10 bed gives 0.504 kPa
    /// where Gao & Shi report 1.3 kPa, i.e. **39% of the published figure**.
    /// Three known differences, none of them quantified here: (1) their figure
    /// covers the bed **and the bottom reflector's** flow passages, which this
    /// model does not represent at all; (2) their calculation is nodalised, so
    /// the correlation is integrated down a bed running 250 -> 700 degC, while
    /// this single node applies it once at the bulk mean; (3) their bed flow is
    /// 3.77 kg/s of 4.32 kg/s (87.3%) against the 86% conservative fraction and
    /// 4.3 kg/s benchmark flow used here. No attempt is made to close the gap
    /// by adjusting anything -- see the module docs.
    #[test]
    fn kta_bed_drop_reproduces_the_vtb_gold_and_is_checked_against_htr10() {
        use uom::si::f64::{Length, Ratio};
        use uom::si::length::meter;
        use uom::si::pressure::kilopascal;

        // --- Part 1: the VTB gold, through the same correlation chain. ---
        let d_h = Length::new::<meter>(0.06);
        let mu_vtb = DynamicViscosity::new::<pascal_second>(1.991242e-5);
        let rho_vtb = MassDensity::new::<kilogram_per_cubic_meter>(8.628204);
        let eps_vtb = Ratio::new::<ratio>(0.39);
        let g_vtb = Ratio::new::<ratio>(40125.0) * mu_vtb / d_h;

        let gradient_vtb = kta::kta_pressure_gradient(g_vtb, d_h, eps_vtb, rho_vtb, mu_vtb);
        let drop_vtb = kta::pressure_drop_over_bed(gradient_vtb, Length::new::<meter>(10.0));
        println!(
            "VTB gold: |dp/dx| = {:.2} Pa/m (gold 3493), drop over 10 m = {:.4} kPa (gold 34.93)",
            gradient_vtb.value,
            drop_vtb.get::<kilopascal>()
        );
        assert!(
            (gradient_vtb.value - 3493.0).abs() < 1.0,
            "KTA gradient {} Pa/m misses the VTB gold 3493 Pa/m",
            gradient_vtb.value
        );
        assert!(
            (drop_vtb.get::<kilopascal>() - 34.93).abs() < 0.01,
            "KTA 10 m drop {} kPa misses the VTB gold 34.93 kPa",
            drop_vtb.get::<kilopascal>()
        );

        // --- Part 2: what that correlation says about the HTR-10 bed. ---
        let bulk_mean_k = 0.5 * (published_core_inlet_k() + published_core_outlet_k());
        let (_, rho, mu) = helium_properties(bulk_mean_k);
        let bed_flow = MassRate::new::<kilogram_per_second>(
            pebble_bed::nominal_helium_flow_kg_per_s() * core_flow_fraction(),
        );
        let flux = kta::superficial_mass_flux(bed_flow, pebble_bed::superficial_area());
        let re = kta::packed_bed_reynolds(flux, pebble_bed::pebble_diameter(), mu);
        let psi = kta::kta_friction_factor(re, pebble_bed::bed_porosity());
        let drop = bed_pressure_drop(
            bed_flow,
            MassDensity::new::<kilogram_per_cubic_meter>(rho),
            mu,
        );
        println!(
            "HTR-10 bed: G = {:.4} kg/(m^2 s), Re = {:.1}, Re/(1-eps) = {:.1}, psi = {:.4}, \
             rho = {:.4} kg/m^3, mu = {:.4e} Pa s, drop = {:.4} kPa (published bed + bottom \
             reflector 1.3 kPa)",
            flux.value,
            re.get::<ratio>(),
            re.get::<ratio>() / (1.0 - pebble_bed::bed_porosity().get::<ratio>()),
            psi.get::<ratio>(),
            rho,
            mu.get::<pascal_second>(),
            drop.get::<kilopascal>()
        );

        // The KTA validity band the source states: Re/(1-eps) from about 1 to
        // 1e5. If a future change pushes the bed outside it, fail loudly.
        let re_modified = re.get::<ratio>() / (1.0 - pebble_bed::bed_porosity().get::<ratio>());
        assert!(
            (1.0..=1.0e5).contains(&re_modified),
            "modified Reynolds number {re_modified} is outside the KTA validity band"
        );

        // Same order as the published figure, and below it -- this model covers
        // the bed only, not the bottom reflector the published figure includes.
        let drop_pa = drop.get::<pascal>();
        assert!(
            (100.0..=PUBLISHED_BED_AND_BOTTOM_REFLECTOR_DROP_PA).contains(&drop_pa),
            "KTA bed drop {drop_pa} Pa is not in the 0.1 kPa to published 1.3 kPa band"
        );
    }

    /// V&V: the loop pressure drop sits on the published loop budget, and both
    /// it and the circulator power rise with flow.
    ///
    /// **Methodology.** The model's loop drop is the KTA bed term plus the
    /// published non-bed remainder (25.9 kPa at rated flow, quadratic in flow:
    /// side reflector 0.7 + mixture plenums 6.1 + steam generator 15.0 + hot
    /// gas duct 4.1, Gao & Shi 2002 Table 1). Evaluated at the rated 4.3 kg/s
    /// the total must therefore land near the published 27.2 kPa loop
    /// resistance -- **not** near the 60 kPa circulator design head, which is a
    /// capability with margin, not an operating loss. Pass criteria: total
    /// within 10% of 27.2 kPa at rated flow; strict monotonicity of both drop
    /// and circulator power between a settled 2.0 kg/s and 6.0 kg/s case.
    ///
    /// **Results (recorded 2026-08-12).** At the settled rated point the model
    /// gives **26.407 kPa** against the published 27.2 kPa, **-2.9%** -- and
    /// the whole of that shortfall is the bed term (0.507 kPa computed by KTA
    /// at the settled bulk mean, against the 1.3 kPa published for bed plus
    /// bottom reflector, see
    /// `kta_bed_drop_reproduces_the_vtb_gold_and_is_checked_against_htr10`),
    /// since the other four components are carried at their published values by
    /// construction. Circulator hydraulic power at that point is **74.3 kW**,
    /// 0.74% of the 10 MWth heat load -- a plausible fraction for a gas-cooled
    /// primary circulator. For comparison the previous anchored-quadratic model
    /// reported 86.1 kPa and 242.3 kW at the same point, because it treated the
    /// 60 kPa circulator *design head* as the operating loss and then scaled it
    /// up by the hot/cold density ratio. The 6.0 kg/s case exceeded the
    /// 2.0 kg/s case on both measures.
    ///
    /// **Interpretation.** The agreement on the *total* is mostly bookkeeping:
    /// 25.9 of the 27.2 kPa is carried, not computed. What is computed is the
    /// bed term, and it disagrees with the published bed figure by a factor of
    /// 2.6 (see the other test). Read this test as "the loop budget is wired up
    /// correctly and scales sensibly", not as a validated loop hydraulic model.
    #[test]
    fn loop_pressure_drop_sits_on_the_published_budget_and_rises_with_flow() {
        use uom::si::pressure::kilopascal;

        let mut rated = nominal_loop();
        for _ in 0..8000 {
            rated.step(
                Time::new::<second>(0.05),
                Power::new::<megawatt>(10.0),
                nominal_flow(),
                sink(523.5),
            );
        }
        let total_kpa = rated.pressure_drop().get::<kilopascal>();
        let bed_kpa = rated.bed_pressure_drop().get::<kilopascal>();
        println!(
            "settled rated point: total dp = {:.3} kPa (published 27.2), of which bed (KTA) = \
             {:.3} kPa (published bed + bottom reflector 1.3); circulator = {:.1} kW; \
             circulator design head for reference {:.0} kPa",
            total_kpa,
            bed_kpa,
            rated.circulator_power().get::<watt>() / 1.0e3,
            CIRCULATOR_DESIGN_HEAD_PA / 1.0e3
        );
        let published_kpa = PUBLISHED_LOOP_TOTAL_DROP_PA / 1.0e3;
        assert!(
            (total_kpa - published_kpa).abs() / published_kpa < 0.10,
            "loop drop {total_kpa} kPa departs from the published {published_kpa} kPa budget"
        );
        assert!(bed_kpa > 0.0, "the KTA bed term must be positive at flow");

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
        assert!(
            fast.bed_pressure_drop().get::<pascal>() > slow.bed_pressure_drop().get::<pascal>()
        );
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
