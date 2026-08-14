//! Steam secondary loop.
//!
//! Models the Rankine secondary side as a closed cycle: feedwater is pumped
//! from the condenser hotwell to steam-generator pressure, the IHX duty from
//! the helium primary loop boils and superheats it, the steam drives a
//! turbine, and the exhaust is condensed against a cooling-water stream that
//! returns the condensate to the hotwell.
//!
//! Steam/water properties are **real** throughout -- every state is an
//! IAPWS-IF97 [`tampines::hem::HemSteamCv`] (`tampines-steam-tables`'
//! `TampinesSteamTableCV`) built from a genuine `(p,h)` / `(p,s)` /
//! saturation flash.
//!
//! ## Nodalisation
//!
//! **One control volume for the cycle, but the steam generator is no longer
//! part of it.** As of 2026-08-12 the water/steam side of the steam generator is
//! a resolved 8-node counter-flow exchanger living in
//! [`super::steam_generator`], driven by the primary loop. What remains here is
//! the *rest* of the Rankine cycle: the turbine, the condenser and the feed pump
//! are each a single state-to-state flash, and the steam-generator outlet is
//! reproduced from the exchanger's own tube-side duty by the enthalpy balance
//! `h_out = h_feed + Q_cold/m_dot`.
//!
//! That balance is now a **restatement of the exchanger's result**, not an
//! independent model of it: `Q_cold` is by construction
//! `m_dot (h_out - h_in)` on the resolved tube side, so the two agree to within
//! one step of feedwater-controller lag. The economiser / evaporator /
//! superheater zones and the boiling-front position are available from
//! [`super::primary_loop::HeliumPrimaryLoop::steam_generator_state`].
//!
//! Still absent: a water inventory, so the steam pressure cannot slide with
//! load, and there is no drum level to swing (bead `op-jyyp.9`).
//!
//! ## What is real
//!
//! - **The steam conditions are the published HTR-10 ones, read from the
//!   library.** Main steam at 4.0 MPa and 440 degC, 12.5 t/hr, feedwater at
//!   104 degC, from IAEA-TECDOC-1382 Table 4-1 -- taken from
//!   [`outram_park_digital_twin_engine::htr10::design::Htr10DesignPoint`],
//!   the workspace's single transcription of that table, rather than re-typed
//!   here. The controller's target steam enthalpy is now flashed from those
//!   published conditions through IF97 at each use instead of being carried as
//!   a rounded constant. The published unit is a **once-through modular
//!   helical-tube** steam generator in a **separate pressure vessel** from the
//!   reactor.
//! - **The cycle is closed.** Feedwater enthalpy is *computed*, not fixed:
//!   condensate is the saturated liquid at condenser pressure, and the feed
//!   pump adds real work `v dp / eta` on top of it. Changing the condenser
//!   pressure therefore moves the feedwater state, as it does in a real plant.
//! - **The condenser has an energy balance.** The duty it rejects,
//!   `m_dot (h_turbine_out - h_condensate)`, is carried by a cooling-water
//!   stream whose outlet temperature follows `Q/(m_cw c_p)`.
//! - **The steam-generator duty comes from a resolved exchanger.** The
//!   `ihx_duty` handed in is the heat the tube side of
//!   [`super::steam_generator::NodalisedCounterFlowSteamGenerator`] actually
//!   absorbed, evaluated from **local node temperatures** on both streams. It is
//!   therefore already bounded by the real, collapsing superheater pinch rather
//!   than by an assumed isothermal sink. The `max_absorbable_duty` backstop
//!   below is retained but no longer binds -- see
//!   [`tests::the_absorbable_duty_cap_no_longer_binds`] for the measured margin.
//! - **Feedwater flow is controlled, not fixed.** A first-order-lagged
//!   proportional law moves the feed flow toward whatever holds the target
//!   steam enthalpy at the current duty.
//! - **The secondary side takes operator commands.** [`SecondaryCommands`]
//!   carries the feedwater mode ([`FeedwaterCommand::Auto`] with an adjustable
//!   target steam temperature, or [`FeedwaterCommand::Manual`] with a directly
//!   commanded flow) and the condenser back-pressure. Every one of them is
//!   clamped to a stated, physically argued range before it reaches the model
//!   -- see each range constant's doc comment for where its limit comes from.
//! - **Turbine expansion** is an isentropic `(p,s)` flash de-rated by an
//!   adiabatic efficiency, with the exhaust quality from the outlet `(p,h)`
//!   flash, and the cycle's net power nets off the feed-pump work.
//!
//! ## What is still illustrative
//!
//! - **The secondary piping inventory is one number for three runs.** The
//!   residence time that drives the schematic's flow tracers is built from the
//!   whole transport path (steam line + exhaust duct + feedwater line), because
//!   the schematic shares one tracer train across all three. Giving each run its
//!   own train and its own residence time is the correct refinement -- see
//!   [`SteamSecondaryLoop::piping_inventory`].
//! - **Live steam pressure is held fixed.** A sliding-pressure or drum
//!   model (steam pressure responding to the boiling/withdrawal imbalance)
//!   needs a mass-and-energy inventory for the steam generator, which this
//!   single-node model does not carry. This is the main remaining
//!   simplification on the secondary side.
//! - **The feedwater temperature is not the published 104 degC.** There is no
//!   feedwater heater train here, so the feedwater state is the condensate at
//!   the (illustrative) condenser pressure plus the pump work, which lands near
//!   40 degC. The published 104 degC is recorded above but not reproduced.
//! - Condenser pressure, cooling-water inlet temperature and flow, the
//!   turbine and pump efficiencies, the flow limits, the operator command
//!   ranges, and the secondary **piping volumes** are **invented values, not
//!   design data** -- IAEA-TECDOC-1382 is
//!   a reactor-physics benchmark and carries no condenser or turbine detail.
//!   Replacing them with sourced figures is tracked as bead `op-szmi.6`.
//! - The published plant is a steam-supply unit feeding a turbine-generator for
//!   co-generation; the closed Rankine cycle with a condenser modelled here is
//!   a plausible balance-of-plant, not the HTR-10's own.
//!
//! This is a demonstration model, not a validated steam-cycle model.

use chem_eng_real_time_process_control_simulator::alpha_nightly::controllers::AnalogController;
use chem_eng_real_time_process_control_simulator::alpha_nightly::transfer_fn_wrapper_and_enums::TransferFnTraits;
use tampines::hem::HemSteamCv;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    AvailableEnergy, Mass, MassRate, Power, Pressure, Ratio, SpecificHeatCapacity,
    ThermodynamicTemperature, Time, Volume,
};
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::{kilopascal, megapascal};
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};
use uom::si::time::second;
use uom::si::volume::cubic_meter;

// ---------------------------------------------------------------------------
// PUBLISHED HTR-10 STEAM CONDITIONS -- READ FROM THE LIBRARY
//
// IAEA-TECDOC-1382 Table 4-1, transcribed once in
// `outram_park_digital_twin_engine::htr10::design::Htr10DesignPoint` with a
// citation per field and a V&V test that closes the steam-side energy balance
// against IAPWS-IF97 (9.9618 MW against the published 10 MWth, -0.38%). This
// module reads that struct; there is no second copy of the steam conditions.
// ---------------------------------------------------------------------------

/// Live steam pressure at the steam-generator outlet / turbine inlet: 4.0 MPa
/// (**published**, via [`super::pebble_bed::design`]). Held fixed -- see the
/// module docs.
fn steam_pressure() -> Pressure {
    super::pebble_bed::design().main_steam_pressure
}

/// The **published** steam-generator outlet temperature, 440 degC (713.15 K),
/// which is the default target the feedwater controller holds in
/// [`FeedwaterCommand::Auto`].
///
/// Read from the design point rather than typed here, so there is one
/// transcription of IAEA-TECDOC-1382 Table 4-1 in the workspace.
pub fn design_target_steam_temperature() -> ThermodynamicTemperature {
    super::pebble_bed::design().main_steam_temperature
}

/// Steam enthalpy at `target_temperature` and the live steam pressure -- the
/// setpoint the feedwater controller in [`FeedwaterCommand::Auto`] holds.
///
/// `target_temperature` is in kelvin (`uom`-typed) and **must already be
/// clamped** by [`clamp_target_steam_temperature`]: IF97's single-phase `(p,T)`
/// flash panics rather than erroring outside its range, and below the
/// saturation temperature at the steam pressure it would return a *liquid*
/// enthalpy, which the controller would chase with an absurd feed flow.
///
/// At the published 4.0 MPa / 440 degC design point this measures
/// **3307.9 kJ/kg** (2026-08-12), which is what the controller used to carry as
/// a hardcoded 3.307e6 J/kg.
fn target_steam_enthalpy_at(target_temperature: ThermodynamicTemperature) -> AvailableEnergy {
    use tampines_steam_tables::interfaces::functional_programming::pt_flash_eqm::h_tp_eqm_single_phase;
    h_tp_eqm_single_phase(target_temperature, steam_pressure())
}

/// Upper temperature bound of the IF97 industrial formulation \[K\].
///
/// IAPWS-IF97 regions 1, 2 and 4 are defined up to **800 degC (1073.15 K)**.
/// `tampines-steam-tables` PANICS rather than returning an error outside its
/// validity range, so the duty cap below must never ask it for an enthalpy
/// above this, even transiently.
const IF97_MAX_TEMPERATURE_K: f64 = 1073.15;

/// Greatest duty \[W\] the steam side can absorb without its outlet exceeding
/// the hot side that is heating it -- i.e. without a temperature cross.
///
/// # Why this exists, and why it is now a backstop rather than the fix
///
/// Until 2026-08-12 the steam generator was an effectiveness-NTU lump in
/// [`super::primary_loop`] pinching against the steam *saturation* temperature,
/// which guarded the helium side only. Superheat beyond saturation was therefore
/// unbounded, which is both a second-law violation and -- because IF97 panics out
/// of range -- the cause of the simulator's crash on a fast power rise. This cap
/// stopped the crash, but by clamping the steam outlet at the *helium inlet
/// temperature*, which is why the GUI then showed steam as hot as the helium.
///
/// **The duty handed in is now correct at source**, from the resolved exchanger
/// in [`super::steam_generator`], whose tube-side outlet can never exceed the
/// local helium temperature. This function is kept as a **backstop** -- cheap,
/// and the last line of defence for an IF97 range panic if a future change feeds
/// this loop a duty from somewhere else -- and
/// [`SteamSecondaryLoop::absorbable_duty_utilisation`] reports how far it is from
/// binding so its inactivity is measured rather than assumed.
///
/// # Method
///
/// The cold stream boils, so a `c_p * dT` cap is not defined across the phase
/// change. The cap is taken on **enthalpy**: the steam may at most reach the
/// hot-side inlet temperature, so
///
/// `Q_max = m_dot * (h(p, T_hot) - h_feed)`
///
/// evaluated at the steam pressure. `T_hot` is additionally clamped to
/// [`IF97_MAX_TEMPERATURE_K`], because the helium runs hotter than water
/// substance is tabulated -- at the HTR-10 design point it leaves the core near
/// 973 K, and an excursion can push it past 1073 K. That clamp makes the cap
/// *conservative* in exactly the regime where it matters, and means this
/// function can never itself trigger the panic it exists to prevent.
///
/// Returns 0.0 rather than a negative duty when the hot side is at or below the
/// feedwater state, so a cold plant transfers nothing instead of running the
/// steam generator backwards.
fn max_absorbable_duty(
    steam_pressure: Pressure,
    hot_side_inlet_temperature: ThermodynamicTemperature,
    mass_flow_kg_per_s: f64,
    feedwater_enthalpy_j_per_kg: f64,
) -> f64 {
    use tampines_steam_tables::interfaces::functional_programming::pt_flash_eqm::h_tp_eqm_single_phase;

    let t_cap_k = hot_side_inlet_temperature
        .get::<kelvin>()
        .min(IF97_MAX_TEMPERATURE_K);
    let t_cap = ThermodynamicTemperature::new::<kelvin>(t_cap_k);
    let h_at_hot_side = h_tp_eqm_single_phase(t_cap, steam_pressure).get::<joule_per_kilogram>();

    (mass_flow_kg_per_s * (h_at_hot_side - feedwater_enthalpy_j_per_kg)).max(0.0)
}

/// Nominal secondary mass flow the loop is seeded at: the published 12.5 t/hr
/// main steam flow, i.e. 3.4722 kg/s (via the design point, which carries the
/// conversion rather than a rounded 3.47).
fn nominal_secondary_flow() -> MassRate {
    super::pebble_bed::design().main_steam_mass_flow
}

// ---------------------------------------------------------------------------
// INVENTED PLACEHOLDERS -- balance-of-plant the published source does not carry
// (bead `op-szmi.6`)
// ---------------------------------------------------------------------------

/// **Design** condenser back-pressure \[MPa\], 7 kPa (**invented**), consistent
/// with the cooling-water inlet temperature below.
///
/// This is the plant's *design* value and deliberately **not** the live one:
/// the operator can move the running condenser pressure through
/// [`SecondaryCommands::condenser_pressure`], but
/// [`design_point_turbine_power`] keeps expanding against this constant,
/// because a machine rating that followed the plant's current back-pressure
/// would make the generator's own nameplate meaningless.
const DESIGN_CONDENSER_PRESSURE_MPA: f64 = 0.007;

/// **Lowest condenser back-pressure the operator may command \[kPa\], 4.0.**
///
/// A condenser cannot condense below its cooling water. The saturation
/// temperature at this pressure must therefore stay above
/// [`COOLING_WATER_INLET_K`] (298.15 K = 25 degC), with a real approach in
/// hand: measured against IAPWS-IF97 on 2026-08-13,
/// `T_sat(4.0 kPa) = 302.11 K = 28.96 degC`, a **3.96 K approach** to the
/// cooling-water inlet. That is already tight for a surface condenser, so it is
/// the floor. See [`tests::the_condenser_pressure_range_keeps_a_cooling_water_approach`].
const MIN_CONDENSER_PRESSURE_KPA: f64 = 4.0;

/// **Highest condenser back-pressure the operator may command \[kPa\], 30.0.**
///
/// A degraded vacuum -- air in-leakage, a fouled tube bundle, a failed ejector.
/// `T_sat(30 kPa)` is about 69 degC, so the cycle still condenses against 25 degC
/// cooling water, but the turbine's available enthalpy drop is heavily reduced.
/// Beyond this the low-pressure end of the expansion stops being a meaningful
/// Rankine cycle at all, so it is the ceiling rather than a plant limit.
///
/// For orientation, the engine's own condenser widget demonstrator
/// (`examples/widget_studio/condenser_tab.rs`) uses a 2-20 kPa knob; this range
/// is deliberately wider at the top so a loss-of-vacuum can be driven, and
/// narrower at the bottom because that widget has no cooling water to approach.
const MAX_CONDENSER_PRESSURE_KPA: f64 = 30.0;

/// **Lowest AUTO steam-temperature setpoint the operator may dial \[degC\],
/// 260.**
///
/// The setpoint is flashed through IF97's **single-phase** `(p,T)` routine, so
/// it has to stay in superheat at the 4.0 MPa steam pressure. Saturation there
/// is `T_sat(4.0 MPa) = 523.51 K = 250.36 degC` (measured, see
/// [`tests::saturation_temperature_matches_if97_reference`]), so this floor
/// keeps **about 10 K of superheat** in hand. Asking a once-through steam
/// generator to hold a saturated outlet is not a control objective anyway --
/// there is no outlet temperature to control once the stream is on the
/// saturation line.
const MIN_TARGET_STEAM_TEMPERATURE_C: f64 = 260.0;

/// **Highest AUTO steam-temperature setpoint the operator may dial \[degC\],
/// 540.**
///
/// Two independent limits, and this is under both:
///
/// - **Thermodynamic.** The steam can never leave hotter than the helium
///   heating it, and the published core outlet is 700 degC. A setpoint above
///   that is unreachable by construction; one near it is unreachable in
///   practice, because the exchanger's hot-end approach never collapses to
///   zero.
/// - **Metallurgical.** 540 degC is the conventional main-steam limit for
///   subcritical plant with ferritic tubing, and is the number a plant operator
///   would recognise as the top of the dial. This model's tube metal is
///   `SteelSS304LHighTemp`, tabulated far above it, so the metal is not what
///   binds here -- the limit is carried because it is the operationally
///   meaningful one, not because the model would fail above it.
///
/// A setpoint the plant cannot reach is not an error: the feedwater controller
/// simply drives the feed flow to [`MIN_SECONDARY_FLOW_KG_PER_S`] and the steam
/// settles wherever the exchanger can put it. What the operator sees then is a
/// controller at its stop, which is the correct depiction.
const MAX_TARGET_STEAM_TEMPERATURE_C: f64 = 540.0;

/// Feed-pump isentropic efficiency (**invented**), 0.75.
const FEED_PUMP_EFFICIENCY: f64 = 0.75;

/// Turbine adiabatic (isentropic) efficiency (**invented**), 0.85.
const TURBINE_EFFICIENCY: f64 = 0.85;

/// Feedwater-controller time constant \[s\] (**invented**), the first-order
/// lag on how fast the feed flow chases its target. This is the *pump and
/// valve* lag, not a controller tuning: it applies in MANUAL too.
const FEED_CONTROL_TIME_CONSTANT_S: f64 = 10.0;

/// Feedwater controller **proportional gain** `K_c` (dimensionless), acting on
/// the steam-temperature error normalised by [`STEAM_ERROR_SCALE_K`] and
/// producing an output scaled by [`FLOW_AUTHORITY_KG_PER_S`].
///
/// Sign convention: the loop is **reverse acting**. Steam that is too cold
/// means each kilogram is getting too little heat, so the correction is to
/// send *less* feedwater. The trim is therefore subtracted from the
/// feedforward demand.
///
/// Scale check, so this is not a magic number: at rated conditions
/// `h_steam = h_feed + Q/m`, so `dh/dm = -Q/m^2`. With `Q = 10 MW` and
/// `m ~ 3.5 kg/s` that is about -0.82 MJ/kg per (kg/s), and superheated steam
/// near 10 MPa has `c_p ~ 2.5 kJ/(kg K)`, giving roughly **-330 K per (kg/s)**.
/// So a 1 K error wants about 0.003 kg/s of trim for a deadbeat correction;
/// `K_p` is set a little above that so the proportional term alone closes most
/// of the gap in one lag time without overshooting.
const FEEDWATER_PROPORTIONAL_GAIN: f64 = 0.60;

/// Steam-temperature error \[K\] that maps to a controller input of 1.0.
///
/// `AnalogController` is dimensionless, so the error has to be normalised on
/// the way in and the output scaled on the way out. 100 K is a full-scale
/// excursion for this plant's steam temperature.
const STEAM_ERROR_SCALE_K: f64 = 100.0;

/// Feedwater flow \[kg/s\] commanded per unit of controller output --- the
/// loop's actuator authority.
///
/// Scale check, so neither this nor the gain is a magic number: at rated
/// conditions `h_steam = h_feed + Q/m`, so `dh/dm = -Q/m^2`. With `Q = 10 MW`
/// and `m ~ 3.2 kg/s` that is about -0.98 MJ/kg per (kg/s); superheated steam
/// near 5 MPa has `c_p ~ 2.5 kJ/(kg K)`, giving roughly **-390 K per (kg/s)**.
/// So a 100 K error (input 1.0) wants about 0.26 kg/s of correction for a
/// deadbeat move. `K_c * FLOW_AUTHORITY = 0.60 * 0.60 = 0.36 kg/s` is
/// deliberately above that: with the feedforward gone the controller has to
/// supply the whole demand, so it is tuned harder than a trim would be.
const FLOW_AUTHORITY_KG_PER_S: f64 = 0.60;

/// Feedwater controller **integral time** `T_i` \[s\] (**invented**). The
/// integral gain is `K_p / T_i`, the standard ISA form.
///
/// Chosen well above the 10 s pump lag so the integral cannot race the
/// actuator it is driving -- an integral time inside the actuator lag is the
/// classic way to turn a stable loop into an oscillating one. Shortened from
/// 40 s when the feedforward was removed: with no model term carrying the load
/// change, the integral has to do that work too.
const FEEDWATER_INTEGRAL_TIME_S: f64 = 25.0;

/// Minimum secondary mass flow \[kg/s\] (**invented**) -- a floor so the
/// enthalpy balance denominator and the residence time stay finite at zero
/// duty. About 14% of the published nominal flow.
///
/// **Raised from 0.3 to 0.5 on 2026-08-14.** At 0.3 kg/s the
/// MANUAL-at-the-pump-floor corner of the command envelope developed a
/// 0.0126 K temperature cross once the core's heat path was corrected (an
/// evaluated pebble-surface coefficient and a fuel-feedback node that actually
/// cools, both of which moved the settled operating point). The cross is a
/// statement about how far the command range may be pushed, not about the
/// assertion that catches it, so the range is narrowed here --- which is what
/// `super::super::physics::tests::no_corner_of_the_command_envelope_crosses_or_clamps`
/// asks a caller to do rather than loosen its tolerance.
const MIN_SECONDARY_FLOW_KG_PER_S: f64 = 0.5;

/// Maximum secondary mass flow \[kg/s\] (**invented** feed-pump capacity), a
/// generous 3.5x the published nominal flow.
const MAX_SECONDARY_FLOW_KG_PER_S: f64 = 12.0;

/// Cooling-water inlet temperature \[K\] (**invented**), ~25 degC.
const COOLING_WATER_INLET_K: f64 = 298.15;

/// Cooling-water mass flow \[kg/s\] (**invented**), sized for a ~10 K rise at
/// the nominal condenser duty of a 10 MWth plant.
const COOLING_WATER_FLOW_KG_PER_S: f64 = 200.0;

/// Cooling-water isobaric specific heat \[J/(kg K)\], liquid water near
/// ambient. Constant is appropriate here: `c_p` varies under 1% over the
/// ~10 K rise this stream sees.
const COOLING_WATER_CP_J_PER_KG_K: f64 = 4180.0;

// ---------------------------------------------------------------------------
// SECONDARY PIPING VOLUMES -- the transport path the schematic actually draws
//
// These replaced a single `SECONDARY_INVENTORY_KG = 2.0e3` on 2026-08-13. That
// constant was not wrong as a *plant* water inventory (a hotwell, a feed train
// and a deaerator really do hold tonnes), but it was the wrong quantity to
// drive a *pipe run's* residence time: most of it is vessel holdup that is not
// on the transport path at all. At the settled 3.13 kg/s feed flow it gave
// `tau = 2000/3.13 = 639 s`, so a tracer mark took over ten minutes to cross
// one connector and the secondary loop read as stagnant on screen.
//
// The primary loop has always done this the other way round -- `rho V` over the
// circuit's *gas* volume, `primary_loop::helium_inventory` -- which is why its
// tracers move. These constants put the secondary on the same footing: an
// invented pipe geometry, a real IF97 density, and a residence time that then
// responds to pressure, temperature and flow the way the primary's does.
//
// Each bore is chosen so the fluid velocity at the published 12.5 t/hr flow
// lands in the normal engineering range for that service. They are **invented
// balance-of-plant dimensions**, in the same sense as the condenser pressure
// and the turbine efficiency above (bead `op-szmi.6`); IAEA-TECDOC-1382 carries
// no piping detail.
// ---------------------------------------------------------------------------

/// Volume of the **main steam line**, steam-generator outlet nozzle to turbine
/// stop valve \[m^3\] (**invented**), 0.236.
///
/// A 30 m run of 0.100 m bore: `pi/4 * 0.100^2 * 30 = 0.2356 m^3`. At the
/// published 3.4722 kg/s and a 4.0 MPa / 440 degC specific volume of about
/// 0.0800 m^3/kg that is a steam velocity of **35 m/s**, which is the normal
/// range for a main steam line.
const MAIN_STEAM_LINE_VOLUME_M3: f64 = 0.236;

/// Volume of the **turbine exhaust duct** to the condenser \[m^3\]
/// (**invented**), 3.2.
///
/// A 5 m run of 0.90 m bore: `pi/4 * 0.90^2 * 5 = 3.18 m^3`. Exhaust at 7 kPa
/// and 0.895 quality has a specific volume near 18 m^3/kg, so the same mass
/// flow moves at roughly **100 m/s** -- low-pressure exhaust is bulky and fast,
/// which is why the duct is nearly ten times the steam line's bore and still
/// holds well under a kilogram.
const EXHAUST_DUCT_VOLUME_M3: f64 = 3.2;

/// Volume of the **condensate / feedwater line**, hotwell through the feed pump
/// to the steam-generator inlet nozzle \[m^3\] (**invented**), 0.047.
///
/// A 40 m run of 0.0386 m bore: `pi/4 * 0.0386^2 * 40 = 0.0468 m^3`. Liquid
/// water near 40 degC is about 992 kg/m^3, so the same mass flow moves at
/// roughly **3 m/s**, the conventional feedwater-line velocity.
///
/// **This is the leg that dominates the secondary residence time**, and
/// correctly so: it is the only one carrying liquid, and liquid is three orders
/// of magnitude denser than the steam in the other two.
const FEEDWATER_LINE_VOLUME_M3: f64 = 0.047;

// ---------------------------------------------------------------------------
// OPERATOR COMMANDS
// ---------------------------------------------------------------------------

/// How the feedwater flow is being commanded.
///
/// This is the secondary side's equivalent of the primary's control-rod
/// position: what the *operator* sets, with the plant's response left to the
/// model. Two modes, matched to the two things a feedwater station can be asked
/// to do.
///
/// The values carried here are **demands, not achieved states**. In both modes
/// the actual feed flow relaxes toward the demand over
/// [`FEED_CONTROL_TIME_CONSTANT_S`] and is clamped to the pump's capacity, so
/// neither mode can step the flow discontinuously.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FeedwaterCommand {
    /// **AUTO** -- the existing controller holds a steam-generator outlet
    /// temperature, which the operator dials.
    ///
    /// The control law is unchanged and is still *feedforward*: it sets the
    /// flow that would carry the offered duty up to the target steam enthalpy,
    /// `m_dot = Q / (h_target - h_feed)`. All this mode adds is that
    /// `h_target` is now flashed from an operator-set temperature instead of
    /// being pinned at the published 440 degC.
    ///
    /// **The known limitation comes with it.** Feedforward against a plant with
    /// a 38 s tube-metal lag limit-cycles (kopi-beans `op-tj10`), and moving the
    /// setpoint does not change that -- it moves the centre of the swing, not
    /// its existence. Replacing the law with real feedback is that bead's job,
    /// not this control's.
    Auto {
        /// Steam-generator outlet temperature to hold, in kelvin
        /// (`uom`-typed). Clamped to
        /// `[MIN_TARGET_STEAM_TEMPERATURE_C, MAX_TARGET_STEAM_TEMPERATURE_C]`
        /// = 260 to 540 degC by [`clamp_target_steam_temperature`].
        target_steam_temperature: ThermodynamicTemperature,
    },
    /// **MANUAL** -- the operator commands the feed flow directly and the
    /// steam temperature is left to go wherever the exchanger puts it.
    ///
    /// This is the mode that makes the plant diagnosable. With the controller
    /// out of the loop the steam outlet is a clean open-loop response to duty
    /// and flow, so the `op-tj10` limit cycle can be shown to be the
    /// controller's and not the exchanger's: hold the flow steady and the swing
    /// stops.
    ///
    /// It is also, in practice, this plant's **load control** -- see
    /// [`super::turbine_generator`] for why there is no governor to do the job
    /// properly.
    Manual {
        /// Feed mass-flow demand, in kg/s (`uom`-typed). Clamped to
        /// `[MIN_SECONDARY_FLOW_KG_PER_S, MAX_SECONDARY_FLOW_KG_PER_S]`
        /// = 0.3 to 12.0 kg/s -- the same invented feed-pump capacity the AUTO
        /// controller is held to, because it is the same pump.
        mass_flow_demand: MassRate,
    },
}

impl Default for FeedwaterCommand {
    /// AUTO at the **published** 440 degC steam-generator outlet temperature,
    /// which is exactly the behaviour this loop had before the mode existed.
    fn default() -> Self {
        let auto_control = false;
        if auto_control {
            return Self::Auto {
                target_steam_temperature: design_target_steam_temperature(),
            };
        }{
            return Self::Manual { 
                mass_flow_demand: MassRate::new::<kilogram_per_second>(10.0)
            };         
        }
    }
}

/// Every operator input the **secondary side** accepts.
///
/// Carried as one value so that adding the next secondary control does not
/// change [`SteamSecondaryLoop::step`]'s signature again. See
/// [`super::PlantCommands`] for the plant-wide set this is part of.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SecondaryCommands {
    /// Feedwater mode and its demand -- see [`FeedwaterCommand`].
    pub feedwater: FeedwaterCommand,
    /// Condenser back-pressure, in pascals (`uom`-typed).
    ///
    /// A **plant condition** as much as an operator input: it is set by the
    /// cooling water and the condenser's own health, and an operator changes it
    /// only indirectly. It is exposed because it sets the bottom of the cycle
    /// -- raising it costs turbine work, raises the exhaust temperature and
    /// warms the feedwater, all of which this model reproduces because the
    /// cycle is closed.
    ///
    /// Clamped to
    /// `[MIN_CONDENSER_PRESSURE_KPA, MAX_CONDENSER_PRESSURE_KPA]` = 4 to 30 kPa
    /// by [`clamp_condenser_pressure`].
    pub condenser_pressure: Pressure,
}

impl Default for SecondaryCommands {
    /// The plant's design condition: AUTO feedwater at the published 440 degC,
    /// and the design 7 kPa condenser back-pressure.
    fn default() -> Self {
        Self {
            feedwater: FeedwaterCommand::default(),
            condenser_pressure: Pressure::new::<megapascal>(DESIGN_CONDENSER_PRESSURE_MPA),
        }
    }
}

/// Clamp a commanded condenser back-pressure into
/// `[MIN_CONDENSER_PRESSURE_KPA, MAX_CONDENSER_PRESSURE_KPA]` (4 to 30 kPa).
///
/// Applied inside [`SteamSecondaryLoop::step`], so an out-of-range value from
/// *any* source -- a slider, a test, a future OPC-UA write -- is bounded by the
/// physics rather than by the widget that happened to produce it. A non-finite
/// input falls back to the design pressure rather than propagating a NaN
/// through the IF97 flashes.
pub fn clamp_condenser_pressure(commanded: Pressure) -> Pressure {
    let kpa = commanded.get::<kilopascal>();
    if !kpa.is_finite() {
        return Pressure::new::<megapascal>(DESIGN_CONDENSER_PRESSURE_MPA);
    }
    Pressure::new::<kilopascal>(kpa.clamp(MIN_CONDENSER_PRESSURE_KPA, MAX_CONDENSER_PRESSURE_KPA))
}

/// Clamp a commanded AUTO steam-temperature setpoint into
/// `[MIN_TARGET_STEAM_TEMPERATURE_C, MAX_TARGET_STEAM_TEMPERATURE_C]`
/// (260 to 540 degC).
///
/// Applied before the IF97 `(p,T)` flash, which **panics** rather than erroring
/// outside its validity range and which would silently return a *liquid*
/// enthalpy below the saturation temperature. A non-finite input falls back to
/// the published design setpoint.
pub fn clamp_target_steam_temperature(
    commanded: ThermodynamicTemperature,
) -> ThermodynamicTemperature {
    let c = commanded.get::<degree_celsius>();
    if !c.is_finite() {
        return design_target_steam_temperature();
    }
    ThermodynamicTemperature::new::<degree_celsius>(c.clamp(
        MIN_TARGET_STEAM_TEMPERATURE_C,
        MAX_TARGET_STEAM_TEMPERATURE_C,
    ))
}

/// Clamp a commanded feedwater mass flow into
/// `[MIN_SECONDARY_FLOW_KG_PER_S, MAX_SECONDARY_FLOW_KG_PER_S]`
/// (0.3 to 12.0 kg/s), the invented feed-pump capacity.
///
/// A non-finite input falls back to the pump's floor, which is the safe
/// direction: a stalled feed pump is a recoverable plant state, a NaN flow is
/// a dead physics thread.
pub fn clamp_feedwater_flow(commanded: MassRate) -> MassRate {
    let kg_s = commanded.get::<kilogram_per_second>();
    if !kg_s.is_finite() {
        return MassRate::new::<kilogram_per_second>(MIN_SECONDARY_FLOW_KG_PER_S);
    }
    MassRate::new::<kilogram_per_second>(
        kg_s.clamp(MIN_SECONDARY_FLOW_KG_PER_S, MAX_SECONDARY_FLOW_KG_PER_S),
    )
}

/// The operator-settable ranges, as plain `(min, max)` pairs in the units an
/// operator dials, so the GUI's sliders cannot drift from the clamps the
/// physics applies.
///
/// Returned rather than exported as constants so there is exactly one place a
/// range is written down. [`tests::the_gui_ranges_match_the_physics_clamps`]
/// checks that each pair really is the clamp's own boundary.
pub mod ranges {
    /// Condenser back-pressure \[kPa\]: 4.0 to 30.0.
    pub const CONDENSER_PRESSURE_KPA: (f64, f64) = (
        super::MIN_CONDENSER_PRESSURE_KPA,
        super::MAX_CONDENSER_PRESSURE_KPA,
    );
    /// AUTO steam-temperature setpoint \[degC\]: 260 to 540.
    pub const TARGET_STEAM_TEMPERATURE_C: (f64, f64) = (
        super::MIN_TARGET_STEAM_TEMPERATURE_C,
        super::MAX_TARGET_STEAM_TEMPERATURE_C,
    );
    /// MANUAL feedwater flow \[kg/s\]: 0.3 to 12.0.
    pub const FEEDWATER_FLOW_KG_PER_S: (f64, f64) = (
        super::MIN_SECONDARY_FLOW_KG_PER_S,
        super::MAX_SECONDARY_FLOW_KG_PER_S,
    );
}

/// Shaft power \[W\] the turbine delivers at the plant's **design point** --
/// the machine rating the turbine-generator in [`super::turbine_generator`] is
/// sized against.
///
/// # What this is
///
/// The same isentropic expansion [`SteamSecondaryLoop::step`] performs, but
/// evaluated once at the *published* HTR-10 main steam conditions rather than
/// at the live state: the published 12.5 t/hr of steam at 4.0 MPa / 440 degC
/// (IAEA-TECDOC-1382 Table 4-1, via the design point) expanded to the condenser
/// back-pressure and de-rated by [`TURBINE_EFFICIENCY`].
///
/// # Why it is a separate function
///
/// A generator has to be *rated* for something, and the rating must not move
/// when the plant moves -- a machine whose nameplate followed the load would
/// make its own speed meaningless. This is therefore a fixed property of the
/// design point, deliberately not read off the running plant.
///
/// # What is published and what is not
///
/// The steam conditions and flow are published; the condenser pressure and the
/// turbine efficiency it expands against are **invented** (see the module
/// docs), so the resulting rating is an illustrative balance-of-plant figure
/// and not an HTR-10 turbine-generator rating. IAEA-TECDOC-1382 carries no
/// turbine detail at all.
///
/// Measured 2026-08-12: **3.4333 MW** (3.433280 MW).
pub fn design_point_turbine_power() -> Power {
    let d = super::pebble_bed::design();
    let p_steam = d.main_steam_pressure;
    // The DESIGN back-pressure, deliberately -- not the live commanded one. See
    // `DESIGN_CONDENSER_PRESSURE_MPA`.
    let p_cond = Pressure::new::<megapascal>(DESIGN_CONDENSER_PRESSURE_MPA);
    let v = Volume::new::<cubic_meter>(1.0);

    let inlet = HemSteamCv::new_from_ph(
        p_steam,
        target_steam_enthalpy_at(design_target_steam_temperature()),
        v,
    );
    let isentropic_outlet = HemSteamCv::new_from_ps(p_cond, inlet.get_specific_entropy(), v);
    let isentropic_drop = inlet.get_specific_enthalpy() - isentropic_outlet.get_specific_enthalpy();

    d.main_steam_mass_flow * isentropic_drop * TURBINE_EFFICIENCY
}

/// The feedwater loop's controller: **pure feedback**, built on the
/// workspace's own process-control crate.
///
/// # Feedback only -- the feedforward was removed on 2026-08-14
///
/// This loop briefly carried a feedforward inverse model
/// (`m = Q/(h_target - h_feed)`) with a PI trim riding on top. That is a
/// common industrial arrangement, but it has a real cost: the feedforward is
/// an open-loop model of the steam generator, so the flow it commands is only
/// as good as that model. When the exchanger's actual behaviour departs from
/// `Q/Delta h` -- which it does, since the duty it can transfer depends on the
/// flow the feedforward is trying to set -- the feedforward is confidently
/// wrong and the trim spends its authority undoing it.
///
/// Pure feedback has no model to be wrong. It observes the steam temperature,
/// compares it with setpoint, and moves the feedwater flow until the error is
/// gone. It gives up the feedforward's instant response to a load change and
/// buys correctness in exchange.
///
/// # Built on `chem-eng-real-time-process-control-simulator`
///
/// The controller is that crate's
/// [`AnalogController::new_pi_controller`], not a hand-rolled loop. It is a
/// continuous-time PI in standard ISA form, `u = K_c (e + (1/T_i) integral e)`,
/// advanced by absolute simulation time through
/// [`TransferFnTraits::set_user_input_and_calc`].
///
/// **Known limitation, stated rather than worked around.** That controller has
/// no anti-windup, and this actuator saturates hard -- feedwater flow is
/// clamped to `[MIN_SECONDARY_FLOW_KG_PER_S, MAX_SECONDARY_FLOW_KG_PER_S]`.
/// While the pump sits on a stop the integrator keeps accumulating against an
/// error it cannot act on, and that shows up as overshoot on the way back.
/// Two things bound the damage here: the controller is rebuilt from scratch
/// whenever the station leaves AUTO (see [`FeedwaterController::reset`]), and
/// the demand is clamped at the actuator rather than inside the controller, so
/// the wound-up state is visible in [`FeedwaterController::last_output`]
/// instead of hidden. Adding conditional integration upstream in that crate
/// would remove the limitation properly, and is worth raising there.
#[derive(Debug, Clone)]
pub struct FeedwaterController {
    /// The PI controller itself.
    controller: AnalogController,
    /// Accumulated controller time, advanced by the plant timestep.
    elapsed: Time,
    /// Most recent steam-temperature error \[K\], kept for display.
    error_k: f64,
    /// Most recent controller output (dimensionless), kept for display.
    last_output: f64,
}

impl FeedwaterController {
    /// A fresh PI controller at [`FEEDWATER_PROPORTIONAL_GAIN`] and
    /// [`FEEDWATER_INTEGRAL_TIME_S`], with its integrator cleared.
    pub fn new() -> Self {
        Self {
            controller: AnalogController::new_pi_controller(
                Ratio::new::<uom::si::ratio::ratio>(FEEDWATER_PROPORTIONAL_GAIN),
                Time::new::<second>(FEEDWATER_INTEGRAL_TIME_S),
            )
            .expect("PI gains must satisfy the controller's preconditions"),
            elapsed: Time::new::<second>(0.0),
            error_k: 0.0,
            last_output: 0.0,
        }
    }

    /// Advance the controller by `dt` against a steam-temperature error and
    /// return the commanded feedwater flow \[kg/s\].
    ///
    /// The error is normalised by [`STEAM_ERROR_SCALE_K`] before it reaches the
    /// controller, because `AnalogController` works in dimensionless
    /// [`Ratio`]s; the output is scaled back out by [`FLOW_AUTHORITY_KG_PER_S`].
    ///
    /// The loop is **reverse acting**: steam below setpoint means each
    /// kilogram is getting too little heat, so the correction is *less*
    /// feedwater. That is why the output is subtracted from the nominal flow.
    ///
    /// If the controller reports an error the previous demand is held, which
    /// is the safe response for an actuator -- freezing beats jumping.
    pub fn command(&mut self, error_k: f64, dt: Time) -> f64 {
        self.error_k = error_k;
        self.elapsed += dt;

        let normalised = Ratio::new::<uom::si::ratio::ratio>(error_k / STEAM_ERROR_SCALE_K);
        match self
            .controller
            .set_user_input_and_calc(normalised, self.elapsed)
        {
            Ok(output) => self.last_output = output.get::<uom::si::ratio::ratio>(),
            Err(_) => { /* hold the previous output */ }
        }

        nominal_secondary_flow().get::<kilogram_per_second>()
            - FLOW_AUTHORITY_KG_PER_S * self.last_output
    }

    /// Most recent steam-temperature error \[K\] (`T_setpoint - T_steam`).
    pub fn error(&self) -> f64 {
        self.error_k
    }

    /// Most recent controller output (dimensionless). A large magnitude while
    /// the pump is on a stop is the visible symptom of windup.
    pub fn last_output(&self) -> f64 {
        self.last_output
    }

    /// Rebuild the controller from scratch, clearing its integrator.
    ///
    /// Used when the station leaves AUTO, so returning to AUTO does not dump
    /// an integral accumulated against a setpoint nobody was controlling to.
    /// A rebuild rather than a reset method because `AnalogController` exposes
    /// no way to zero its integrator in place.
    pub fn reset(&mut self) {
        let elapsed = self.elapsed;
        *self = Self::new();
        self.elapsed = elapsed;
    }
}

impl Default for FeedwaterController {
    fn default() -> Self {
        Self::new()
    }
}

/// Steam secondary-loop state.
pub struct SteamSecondaryLoop {
    steam_pressure: Pressure,
    condenser_pressure: Pressure,
    /// Reference (extensive) control-volume size for the `HemSteamCv` states;
    /// intensive flash results do not depend on it.
    reference_volume: Volume,

    // Computed cycle states (recomputed each step).
    /// Saturated-liquid condensate leaving the condenser hotwell.
    condensate: HemSteamCv,
    /// Feedwater entering the steam generator (condensate + pump work).
    feedwater_enthalpy: AvailableEnergy,
    /// Current secondary mass flow, moved by the feedwater controller.
    mass_flow: MassRate,
    steam_generator_outlet: HemSteamCv,
    turbine_inlet_temperature: ThermodynamicTemperature,
    turbine_power: Power,
    feed_pump_power: Power,
    steam_quality_after_turbine: f64,
    condenser_duty: Power,
    cooling_water_outlet_temperature: ThermodynamicTemperature,
    /// How close the offered duty came to [`max_absorbable_duty`] on the most
    /// recent step, as `Q_offered / Q_max`. See
    /// [`Self::absorbable_duty_utilisation`].
    absorbable_duty_utilisation: f64,
    /// Water/steam mass held in the secondary **piping** on the most recent
    /// step -- see [`Self::piping_inventory`].
    piping_inventory: Mass,
    /// Pure-feedback PI controller on the steam-temperature error. See
    /// [`FeedwaterController`].
    feedwater_controller: FeedwaterController,
}

impl SteamSecondaryLoop {
    /// Construct the loop at its nominal operating point, with the condensate
    /// and feedwater states flashed from the real steam tables and the
    /// steam-generator outlet seeded at zero duty.
    pub fn new() -> Self {
        let steam_pressure = steam_pressure();
        let condenser_pressure = Pressure::new::<megapascal>(DESIGN_CONDENSER_PRESSURE_MPA);
        let reference_volume = Volume::new::<cubic_meter>(1.0);

        // Saturated liquid in the hotwell, then the real feed-pump enthalpy rise.
        let condensate =
            HemSteamCv::new_from_sat_pressure_quality(condenser_pressure, 0.0, reference_volume);
        let feedwater_enthalpy =
            feedwater_enthalpy(&condensate, steam_pressure, condenser_pressure);

        let steam_generator_outlet =
            HemSteamCv::new_from_ph(steam_pressure, feedwater_enthalpy, reference_volume);
        let turbine_inlet_temperature = steam_generator_outlet.get_temperature();

        Self {
            steam_pressure,
            condenser_pressure,
            reference_volume,
            condensate,
            feedwater_enthalpy,
            mass_flow: nominal_secondary_flow(),
            steam_generator_outlet,
            turbine_inlet_temperature,
            turbine_power: Power::new::<watt>(0.0),
            feed_pump_power: Power::new::<watt>(0.0),
            steam_quality_after_turbine: 0.0,
            condenser_duty: Power::new::<watt>(0.0),
            cooling_water_outlet_temperature: ThermodynamicTemperature::new::<kelvin>(
                COOLING_WATER_INLET_K,
            ),
            absorbable_duty_utilisation: 0.0,
            // Seeded from the constructor's own states, so the first frame
            // reports a real inventory rather than zero (which would read as an
            // infinite flow speed on the schematic's tracers).
            piping_inventory: piping_inventory(
                &steam_generator_outlet,
                &steam_generator_outlet,
                &condensate,
            ),
            feedwater_controller: FeedwaterController::new(),
        }
    }

    /// The loop's **only integrated state**: the feedwater mass flow the
    /// controller is relaxing toward its target.
    ///
    /// Everything else this struct holds -- the condensate, the feedwater
    /// enthalpy, the steam-generator outlet, the turbine and condenser results
    /// -- is recomputed from scratch inside [`Self::step`] from the duty it is
    /// handed, so restoring this one scalar rewinds the loop exactly.
    ///
    /// Used by [`super::HtgrPlant::step`]'s outer-corrector loop with
    /// [`Self::restore_integrated_state`].
    pub fn integrated_state(&self) -> MassRate {
        self.mass_flow
    }

    /// Restore the mass flow saved by [`Self::integrated_state`], rewinding
    /// this loop to the start of the current plant timestep.
    pub fn restore_integrated_state(&mut self, mass_flow: MassRate) {
        self.mass_flow = mass_flow;
    }

    /// Saturation temperature at the live steam pressure, from the real IF97
    /// saturation line.
    ///
    /// **This stopped being a coupling variable on 2026-08-12.** It used to be
    /// handed to the primary loop as the isothermal cold-side temperature its
    /// effectiveness-NTU steam generator pinched against -- which is exactly the
    /// assumption that made a once-through unit's superheater invisible. The
    /// exchanger now resolves the cold side, so what crosses to the primary is
    /// the feedwater enthalpy and flow instead.
    ///
    /// It is kept because it is still a real, meaningful plant quantity (the
    /// boiling temperature the evaporator zone pins itself to -- see the
    /// evaporator plateau in
    /// `super::steam_generator::tests::the_cold_stream_resolves_all_three_zones`)
    /// and is a natural thing for the GUI to display.
    #[allow(dead_code)] // no longer a coupling variable -- see the doc comment
    pub fn saturation_temperature(&self) -> ThermodynamicTemperature {
        HemSteamCv::new_from_sat_pressure_quality(self.steam_pressure, 0.0, self.reference_volume)
            .get_temperature()
    }

    /// Advance the loop by `dt`, absorbing `ihx_duty` into the steam and
    /// expanding it through the turbine.
    ///
    /// The step, in order:
    ///
    /// 0. **Operator commands.** The condenser back-pressure is taken from
    ///    `commands` and clamped by [`clamp_condenser_pressure`].
    /// 1. **Feedwater controller or manual demand.** In
    ///    [`FeedwaterCommand::Auto`] the flow that would hold the commanded
    ///    steam temperature at the current duty is `Q/(h_target - h_feed)`; in
    ///    [`FeedwaterCommand::Manual`] the demand is the operator's number
    ///    directly. Either way the actual flow relaxes toward it over
    ///    [`FEED_CONTROL_TIME_CONSTANT_S`], clamped to the pump's range.
    /// 2. **Condensate and feed pump.** Condensate is the saturated liquid at
    ///    condenser pressure; the feedwater enthalpy adds the real pump work
    ///    `v (p_steam - p_cond) / eta`. Feed-pump power is `m_dot` times that
    ///    rise.
    /// 3. **Steam generator.** `h_steam = h_feed + Q_ihx/m_dot`, flashed at
    ///    the steam pressure.
    /// 4. **Turbine.** Isentropic `(p,s)` expansion to condenser pressure,
    ///    de-rated by [`TURBINE_EFFICIENCY`]; power `m_dot (h_in - h_out)`.
    /// 5. **Condenser.** Duty `m_dot (h_out - h_condensate)` carried by the
    ///    cooling water, whose outlet temperature follows `Q/(m_cw c_p)`.
    /// 6. **Piping inventory**, for the schematic's flow tracers -- see
    ///    [`Self::piping_inventory`].
    pub fn step(
        &mut self,
        dt: uom::si::f64::Time,
        commands: SecondaryCommands,
        ihx_duty: Power,
        hot_side_inlet_temperature: ThermodynamicTemperature,
    ) {
        let q_w_uncapped = ihx_duty.get::<watt>().max(0.0);

        // 0. Operator command: the condenser back-pressure. Clamped HERE, in
        //    the physics, rather than trusting whatever produced it -- a
        //    slider, a test, or a future OPC-UA write all get the same bound.
        //    It is applied before the condensate flash below, so the whole cold
        //    end of the cycle moves with it: condensate state, feed-pump work,
        //    turbine exhaust and condenser duty.
        self.condenser_pressure = clamp_condenser_pressure(commands.condenser_pressure);

        // 2a. Condensate + feed pump (needed before the controller target,
        //     since the target flow depends on the feedwater enthalpy).
        self.condensate = HemSteamCv::new_from_sat_pressure_quality(
            self.condenser_pressure,
            0.0,
            self.reference_volume,
        );
        self.feedwater_enthalpy = feedwater_enthalpy(
            &self.condensate,
            self.steam_pressure,
            self.condenser_pressure,
        );
        let h_feed = self.feedwater_enthalpy.get::<joule_per_kilogram>();
        let h_condensate = self
            .condensate
            .get_specific_enthalpy()
            .get::<joule_per_kilogram>();

        // 1. Feedwater demand -- from the controller in AUTO, from the operator
        //    in MANUAL.
        let target_flow = match commands.feedwater {
            FeedwaterCommand::Auto {
                target_steam_temperature,
            } => {
                let setpoint = clamp_target_steam_temperature(target_steam_temperature);
                // PURE FEEDBACK. No feedforward term: the controller observes
                // the steam temperature and moves the flow until the error is
                // gone. See `FeedwaterController` for why the inverse model
                // was removed.
                let error_k =
                    setpoint.get::<kelvin>() - self.turbine_inlet_temperature.get::<kelvin>();
                self.feedwater_controller
                    .command(error_k, dt)
                    .clamp(MIN_SECONDARY_FLOW_KG_PER_S, MAX_SECONDARY_FLOW_KG_PER_S)
            }
            // MANUAL: the operator's number IS the demand. It still goes
            // through the same clamp and the same first-order lag below,
            // because the pump and its control valve do not care which mode the
            // station is in -- only where the demand came from changes.
            //
            // The controller is rebuilt while in MANUAL so that returning to
            // AUTO does not dump a stale integral -- accumulated against a
            // setpoint nobody was controlling to -- into the demand.
            FeedwaterCommand::Manual { mass_flow_demand } => {
                self.feedwater_controller.reset();
                clamp_feedwater_flow(mass_flow_demand).get::<kilogram_per_second>()
            }
        };
        let alpha = (dt.get::<second>() / FEED_CONTROL_TIME_CONSTANT_S).clamp(0.0, 1.0);
        let flow_kg_s = self.mass_flow.get::<kilogram_per_second>();
        let flow_next = (flow_kg_s + alpha * (target_flow - flow_kg_s))
            .clamp(MIN_SECONDARY_FLOW_KG_PER_S, MAX_SECONDARY_FLOW_KG_PER_S);
        self.mass_flow = MassRate::new::<kilogram_per_second>(flow_next);

        // 2b. Feed-pump power at the settled flow.
        self.feed_pump_power = Power::new::<watt>(flow_next * (h_feed - h_condensate).max(0.0));

        // 2c. SECOND-LAW CAP on the duty the steam side is allowed to absorb.
        //
        // The primary's effectiveness-NTU pinch (see `primary_loop`) caps duty
        // against the *saturation* temperature at the steam pressure, and its
        // test checks the HELIUM side only: that the helium is never cooled
        // below the sink nor heated above its inlet. Nothing there constrains
        // the steam OUTLET, because the secondary is modelled as an isothermal
        // sink -- so the superheat computed below was previously unbounded.
        //
        // That was a real temperature cross, not a display artefact: with
        // helium near 1000 K and saturation at 523 K the pinch authorises a
        // large duty, and if the feedwater controller is still lagging at
        // MIN_SECONDARY_FLOW_KG_PER_S that duty superheats the steam far above
        // the helium that heated it. Because IF97 PANICS rather than returning
        // an error out of range, the symptom was a dead physics thread and a
        // restart modal rather than a visibly wrong number.
        //
        // Cap on ENTHALPY, not `c_p * dT` -- the stream boils, so a specific
        // heat is not defined across the phase change. This mirrors the fix
        // already made in fhr_sim_v2's `steam_generator_duty`.
        let q_max = max_absorbable_duty(
            self.steam_pressure,
            hot_side_inlet_temperature,
            flow_next,
            h_feed,
        );
        let q_w = q_w_uncapped.min(q_max);
        // How hard the backstop is being leaned on. 1.0 means it is binding.
        self.absorbable_duty_utilisation = if q_max > 0.0 {
            q_w_uncapped / q_max
        } else if q_w_uncapped > 0.0 {
            f64::INFINITY
        } else {
            0.0
        };

        // 3. Steam-generator outlet from the secondary energy balance.
        let h_steam = h_feed + q_w / flow_next;
        self.steam_generator_outlet = HemSteamCv::new_from_ph(
            self.steam_pressure,
            AvailableEnergy::new::<joule_per_kilogram>(h_steam),
            self.reference_volume,
        );
        self.turbine_inlet_temperature = self.steam_generator_outlet.get_temperature();

        // 4. Isentropic expansion to condenser pressure, de-rated by the
        //    adiabatic efficiency.
        let s_in: SpecificHeatCapacity = self.steam_generator_outlet.get_specific_entropy();
        let isentropic_outlet =
            HemSteamCv::new_from_ps(self.condenser_pressure, s_in, self.reference_volume);
        let h_in = h_steam;
        let h_out_isentropic = isentropic_outlet
            .get_specific_enthalpy()
            .get::<joule_per_kilogram>();
        let h_out = h_in - TURBINE_EFFICIENCY * (h_in - h_out_isentropic);

        let turbine_outlet = HemSteamCv::new_from_ph(
            self.condenser_pressure,
            AvailableEnergy::new::<joule_per_kilogram>(h_out),
            self.reference_volume,
        );
        self.steam_quality_after_turbine = turbine_outlet.get_quality();
        self.turbine_power = Power::new::<watt>((flow_next * (h_in - h_out)).max(0.0));

        // 5. Condenser energy balance onto the cooling-water stream.
        let condenser_duty_w = (flow_next * (h_out - h_condensate)).max(0.0);
        self.condenser_duty = Power::new::<watt>(condenser_duty_w);
        let cw_rise =
            condenser_duty_w / (COOLING_WATER_FLOW_KG_PER_S * COOLING_WATER_CP_J_PER_KG_K);
        self.cooling_water_outlet_temperature =
            ThermodynamicTemperature::new::<kelvin>(COOLING_WATER_INLET_K + cw_rise);

        // 6. Piping inventory, from the three real states this step just
        //    flashed. Not a plant quantity anything else consumes -- it exists
        //    so the schematic's secondary flow tracers move at the loop's real
        //    transport speed. See `Self::piping_inventory`.
        self.piping_inventory = piping_inventory(
            &self.steam_generator_outlet,
            &turbine_outlet,
            &self.condensate,
        );
    }

    /// The steam-generator secondary-side outlet state (real `HemSteamCv`).
    pub fn steam_generator_outlet(&self) -> HemSteamCv {
        self.steam_generator_outlet
    }

    /// Condensate (saturated liquid) state leaving the condenser hotwell.
    pub fn condensate(&self) -> HemSteamCv {
        self.condensate
    }

    /// Feedwater specific enthalpy entering the steam generator -- condensate
    /// plus real feed-pump work.
    pub fn feedwater_enthalpy(&self) -> AvailableEnergy {
        self.feedwater_enthalpy
    }

    /// Current secondary mass flow, as moved by the feedwater controller.
    pub fn mass_flow(&self) -> MassRate {
        self.mass_flow
    }

    /// Water/steam mass held in the secondary **piping** -- the transport path
    /// the schematic draws -- used for the residence time that drives its flow
    /// tracers.
    ///
    /// `rho V` summed over the three runs, with each `rho` taken from the real
    /// IAPWS-IF97 state this loop computed on its most recent step:
    ///
    /// | Run | Volume | Density from |
    /// |---|---|---|
    /// | main steam line | [`MAIN_STEAM_LINE_VOLUME_M3`] | the steam-generator outlet state |
    /// | turbine exhaust duct | [`EXHAUST_DUCT_VOLUME_M3`] | the turbine outlet state |
    /// | condensate/feedwater line | [`FEEDWATER_LINE_VOLUME_M3`] | the hotwell condensate state |
    ///
    /// # What this deliberately is NOT
    ///
    /// It is **not** the plant's water inventory. A hotwell, a feed train and a
    /// deaerator hold tonnes, and this model carries none of them; a residence
    /// time built from that mass describes how long water sits in *vessels*,
    /// not how long a parcel takes to cross a *pipe*. Until 2026-08-13 this
    /// function returned a flat invented 2000 kg, which at the settled feed
    /// flow made the schematic's secondary tracers take **639 s** to cross one
    /// run -- indistinguishable from stagnant. See the piping-volume constants
    /// for the reasoning and
    /// [`tests::the_secondary_piping_residence_time_is_a_real_transport_time`]
    /// for the measured before/after.
    ///
    /// # Two honest limitations
    ///
    /// - **One number drives three runs.** The schematic shares a single
    ///   `TracerTrain` across the steam line, the exhaust duct and the
    ///   feedwater line, so they are all drawn at this loop-wide figure. It is
    ///   dominated by the feedwater line (liquid, hence nearly all the mass),
    ///   so the *steam* runs are drawn slower than their own transport time.
    ///   Giving each run its own train and its own residence time is the
    ///   correct refinement and is not done here.
    /// - **Cold start reads high, correctly.** Before the steam generator makes
    ///   steam its outlet is compressed liquid, so the main-steam line's `rho V`
    ///   is briefly three orders of magnitude larger. That is a real statement
    ///   about a line full of water, not an artefact, and it decays as the plant
    ///   heats up.
    pub fn piping_inventory(&self) -> Mass {
        self.piping_inventory
    }

    /// Turbine inlet temperature (= steam-generator outlet temperature).
    /// The feedwater PI trim's state -- its most recent steam-temperature
    /// error and accumulated integral. Exposed so the GUI can show whether the
    /// controller is actually holding setpoint or sitting on a stop.
    pub fn feedwater_controller(&self) -> &FeedwaterController {
        &self.feedwater_controller
    }

    pub fn turbine_inlet_temperature(&self) -> ThermodynamicTemperature {
        self.turbine_inlet_temperature
    }

    /// Live steam pressure (held fixed -- see the module docs).
    pub fn steam_pressure(&self) -> Pressure {
        self.steam_pressure
    }

    /// Condenser back-pressure.
    pub fn condenser_pressure(&self) -> Pressure {
        self.condenser_pressure
    }

    /// Turbine mechanical power output.
    pub fn turbine_power(&self) -> Power {
        self.turbine_power
    }

    /// Feed-pump power drawn to raise the condensate to steam pressure.
    pub fn feed_pump_power(&self) -> Power {
        self.feed_pump_power
    }

    /// Net cycle power: turbine output less the feed-pump work.
    pub fn net_power(&self) -> Power {
        self.turbine_power - self.feed_pump_power
    }

    /// Heat rejected in the condenser to the cooling-water stream.
    pub fn condenser_duty(&self) -> Power {
        self.condenser_duty
    }

    /// Cooling-water outlet temperature from the condenser energy balance.
    pub fn cooling_water_outlet_temperature(&self) -> ThermodynamicTemperature {
        self.cooling_water_outlet_temperature
    }

    /// Steam quality at the turbine exhaust `[0, 1]`.
    pub fn steam_quality_after_turbine(&self) -> f64 {
        self.steam_quality_after_turbine
    }

    /// How close the offered steam-generator duty came to the second-law
    /// backstop [`max_absorbable_duty`] on the most recent step, as the
    /// dimensionless ratio `Q_offered / Q_max`.
    ///
    /// - **Below 1.0**: the backstop is inactive and the steam outlet is the
    ///   plain enthalpy balance `h_feed + Q/m_dot`. This is what normal
    ///   operation should look like now that the duty comes from a resolved
    ///   exchanger.
    /// - **At or above 1.0**: the backstop is binding and the reported steam
    ///   temperature is the clamp, not the physics. Before 2026-08-12 this was
    ///   routine, because the duty handed in came from an effectiveness-NTU lump
    ///   against an isothermal sink and was over-predicted; the clamp is what
    ///   made the steam read as hot as the helium.
    ///
    /// Measured margin: see
    /// [`tests::the_absorbable_duty_cap_no_longer_binds`].
    #[allow(dead_code)] // read by the V&V tests; snapshot candidate for the app layer
    pub fn absorbable_duty_utilisation(&self) -> f64 {
        self.absorbable_duty_utilisation
    }
}

/// Water/steam mass held in the secondary piping, `sum V_i / v_i` over the
/// three runs -- see [`SteamSecondaryLoop::piping_inventory`] for what it is
/// for and what it deliberately excludes.
///
/// Each argument is a real IAPWS-IF97 state and supplies the specific volume
/// \[m^3/kg\] of its run:
///
/// - `steam` -- the steam-generator outlet, filling [`MAIN_STEAM_LINE_VOLUME_M3`];
/// - `exhaust` -- the turbine outlet, filling [`EXHAUST_DUCT_VOLUME_M3`];
/// - `condensate` -- the hotwell saturated liquid, filling
///   [`FEEDWATER_LINE_VOLUME_M3`]. The feedwater is that condensate plus pump
///   work, so it is a few tenths of a kelvin warmer at 4.0 MPa rather than
///   7 kPa; compressed liquid water's density moves under 0.5% over that, which
///   is far inside the invented pipe geometry's own uncertainty.
///
/// A non-finite or non-positive specific volume contributes nothing rather than
/// poisoning the sum -- the result feeds a residence time, and an infinite
/// inventory would freeze the schematic's tracers with no explanation.
fn piping_inventory(steam: &HemSteamCv, exhaust: &HemSteamCv, condensate: &HemSteamCv) -> Mass {
    let mass_in = |volume_m3: f64, state: &HemSteamCv| {
        let v = state
            .get_specific_volume()
            .get::<cubic_meter_per_kilogram>();
        if v.is_finite() && v > 0.0 {
            volume_m3 / v
        } else {
            0.0
        }
    };
    Mass::new::<kilogram>(
        mass_in(MAIN_STEAM_LINE_VOLUME_M3, steam)
            + mass_in(EXHAUST_DUCT_VOLUME_M3, exhaust)
            + mass_in(FEEDWATER_LINE_VOLUME_M3, condensate),
    )
}

/// Feedwater specific enthalpy: condensate enthalpy plus the real feed-pump
/// work `v (p_steam - p_cond) / eta`.
///
/// Uses the incompressible-liquid pump-work approximation, which is the
/// standard treatment for a feed pump: the condensate specific volume is
/// essentially constant over the compression, so the isentropic work is
/// `v dp`, divided by [`FEED_PUMP_EFFICIENCY`] for the actual work.
fn feedwater_enthalpy(
    condensate: &HemSteamCv,
    steam_pressure: Pressure,
    condenser_pressure: Pressure,
) -> AvailableEnergy {
    let h_condensate = condensate
        .get_specific_enthalpy()
        .get::<joule_per_kilogram>();
    let v = condensate
        .get_specific_volume()
        .get::<cubic_meter_per_kilogram>();
    let dp = (steam_pressure - condenser_pressure).get::<uom::si::pressure::pascal>();
    let pump_work = v * dp / FEED_PUMP_EFFICIENCY;
    AvailableEnergy::new::<joule_per_kilogram>(h_condensate + pump_work)
}

impl Default for SteamSecondaryLoop {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::Time;
    use uom::si::power::megawatt;

    fn dt() -> Time {
        Time::new::<second>(0.05)
    }

    /// The plant's design commands: AUTO feedwater at the published 440 degC
    /// and the design 7 kPa condenser back-pressure.
    ///
    /// Every pre-existing test in this module used to run implicitly at these
    /// conditions, because the loop had no operator input at all. Naming them
    /// keeps those tests measuring what they always measured.
    fn design_commands() -> SecondaryCommands {
        SecondaryCommands::default()
    }

    /// Hot-side (core outlet) temperature the steam generator sees in tests:
    /// the published HTR-10 core outlet of 700 degC (973.15 K).
    ///
    /// This is the temperature the second-law cap is taken against. Tests that
    /// exercise the loop at its design duty must supply a design hot side, or
    /// the cap would throttle them for reasons unrelated to what they check.
    fn nominal_hot_side() -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(973.15)
    }

    /// The GUI's slider ranges must be the physics' own clamp boundaries.
    ///
    /// **Methodology.** [`ranges`] is what `crate::app::panels` builds its
    /// sliders from, and the clamp helpers are what the model applies. If those
    /// two ever disagreed, a slider would offer a value the physics silently
    /// overrode, and the operator would drag a control that stopped responding
    /// with no indication why. This asserts the identity in the only way that
    /// can fail: it clamps values *just outside* each advertised range and
    /// checks the result lands exactly on the advertised boundary, and clamps
    /// the boundaries themselves and checks they pass through unchanged.
    ///
    /// Also checks the NaN fallbacks, because a non-finite value from an
    /// OPC-UA write or a divide-by-zero upstream must not reach an IF97 flash.
    #[test]
    fn the_gui_ranges_match_the_physics_clamps() {
        let (lo, hi) = ranges::CONDENSER_PRESSURE_KPA;
        for (input, expected) in [(lo - 1.0, lo), (hi + 1.0, hi), (lo, lo), (hi, hi)] {
            let got =
                clamp_condenser_pressure(Pressure::new::<kilopascal>(input)).get::<kilopascal>();
            assert!(
                (got - expected).abs() < 1e-9,
                "condenser pressure {input} kPa clamped to {got}, expected {expected}"
            );
        }
        assert!(
            clamp_condenser_pressure(Pressure::new::<kilopascal>(f64::NAN))
                .get::<kilopascal>()
                .is_finite()
        );

        let (lo, hi) = ranges::TARGET_STEAM_TEMPERATURE_C;
        for (input, expected) in [(lo - 50.0, lo), (hi + 50.0, hi), (lo, lo), (hi, hi)] {
            let got = clamp_target_steam_temperature(
                ThermodynamicTemperature::new::<degree_celsius>(input),
            )
            .get::<degree_celsius>();
            assert!(
                (got - expected).abs() < 1e-9,
                "steam setpoint {input} degC clamped to {got}, expected {expected}"
            );
        }
        assert!(
            clamp_target_steam_temperature(ThermodynamicTemperature::new::<degree_celsius>(
                f64::NAN
            ))
            .get::<kelvin>()
            .is_finite()
        );

        let (lo, hi) = ranges::FEEDWATER_FLOW_KG_PER_S;
        for (input, expected) in [(lo - 1.0, lo), (hi + 1.0, hi), (lo, lo), (hi, hi)] {
            let got = clamp_feedwater_flow(MassRate::new::<kilogram_per_second>(input))
                .get::<kilogram_per_second>();
            assert!(
                (got - expected).abs() < 1e-9,
                "feed flow {input} kg/s clamped to {got}, expected {expected}"
            );
        }
        assert!(
            clamp_feedwater_flow(MassRate::new::<kilogram_per_second>(f64::NAN))
                .get::<kilogram_per_second>()
                .is_finite()
        );

        // The default command must sit inside every range it is subject to --
        // a default the operator cannot dial back to would be a trap.
        let d = SecondaryCommands::default();
        let p = d.condenser_pressure.get::<kilopascal>();
        assert!((ranges::CONDENSER_PRESSURE_KPA.0..=ranges::CONDENSER_PRESSURE_KPA.1).contains(&p));
        match d.feedwater {
            FeedwaterCommand::Auto {
                target_steam_temperature,
            } => {
                let c = target_steam_temperature.get::<degree_celsius>();
                assert!(
                    (ranges::TARGET_STEAM_TEMPERATURE_C.0..=ranges::TARGET_STEAM_TEMPERATURE_C.1)
                        .contains(&c),
                    "the published 440 degC setpoint must be dialable, got {c}"
                );
            }
            FeedwaterCommand::Manual { .. } => panic!("the default feedwater mode must be AUTO"),
        }
    }

    /// V&V: **the condenser-pressure range keeps a real approach to the cooling
    /// water at its floor, and stays a condenser at its ceiling.**
    ///
    /// # Methodology
    ///
    /// A condenser rejects heat into cooling water, so its saturation
    /// temperature has to stay above the cooling water's inlet -- otherwise the
    /// model would be transferring heat up a temperature gradient, and the
    /// condenser energy balance would hand the cooling-water stream a negative
    /// rise. Nothing in the loop's arithmetic prevents that; the *range* is what
    /// prevents it, which is why the range is what gets tested.
    ///
    /// The IAPWS-IF97 saturation temperature is evaluated at both ends of
    /// [`ranges::CONDENSER_PRESSURE_KPA`] through the same
    /// `HemSteamCv::new_from_sat_pressure_quality` flash the loop itself uses,
    /// and compared with [`COOLING_WATER_INLET_K`]. Pass criterion: at least a
    /// **2 K** approach at the floor (a real surface condenser would want more,
    /// but 2 K is the point below which the model stops being meaningful), and
    /// the ceiling still below the 4.0 MPa steam pressure's own saturation
    /// temperature so the cycle is still a cycle.
    ///
    /// # Results (measured 2026-08-13)
    ///
    /// | Commanded pressure | `T_sat` | Approach to the 298.15 K cooling water |
    /// |---|---|---|
    /// | 4.0 kPa (floor) | 302.11 K = 28.96 degC | **+3.96 K** |
    /// | 7.0 kPa (design) | 312.15 K = 39.00 degC | +14.00 K |
    /// | 30.0 kPa (ceiling) | 342.25 K = 69.10 degC | +44.10 K |
    ///
    /// The ceiling's 342.25 K is well below the 523.51 K boiling temperature in
    /// the steam generator, so there is still a cycle at the top of the range.
    ///
    /// **Interpretation.** The 4 kPa floor is defensible but tight: a 3.96 K
    /// approach is at the aggressive end for a surface condenser, and it is
    /// where the floor was placed deliberately, so a user can drive the cold end
    /// as far as the model honestly allows and no further.
    #[test]
    fn the_condenser_pressure_range_keeps_a_cooling_water_approach() {
        let reference_volume = Volume::new::<cubic_meter>(1.0);
        let t_sat_at = |kpa: f64| {
            HemSteamCv::new_from_sat_pressure_quality(
                Pressure::new::<kilopascal>(kpa),
                0.0,
                reference_volume,
            )
            .get_temperature()
            .get::<kelvin>()
        };

        let (lo, hi) = ranges::CONDENSER_PRESSURE_KPA;
        let design = DESIGN_CONDENSER_PRESSURE_MPA * 1.0e3;
        for kpa in [lo, design, hi] {
            println!(
                "CONDENSER PRESSURE {kpa:.1} kPa: T_sat = {:.2} K ({:.2} degC), \
                 approach to the {COOLING_WATER_INLET_K:.2} K cooling water = {:.2} K",
                t_sat_at(kpa),
                t_sat_at(kpa) - 273.15,
                t_sat_at(kpa) - COOLING_WATER_INLET_K,
            );
        }

        let approach_at_floor = t_sat_at(lo) - COOLING_WATER_INLET_K;
        assert!(
            approach_at_floor > 2.0,
            "at the {lo} kPa floor the condenser saturates only {approach_at_floor} K above \
             its cooling water; the floor is too low"
        );
        let t_sat_steam = SteamSecondaryLoop::new()
            .saturation_temperature()
            .get::<kelvin>();
        assert!(
            t_sat_at(hi) < t_sat_steam,
            "at the {hi} kPa ceiling the condenser saturates at {} K, at or above the \
             {t_sat_steam} K boiling temperature in the steam generator -- there is no \
             cycle left",
            t_sat_at(hi)
        );
    }

    /// V&V: **MANUAL feedwater holds the operator's flow regardless of duty,
    /// and AUTO does not.**
    ///
    /// # Methodology
    ///
    /// The whole value of a MANUAL mode is that it takes the controller out of
    /// the loop, so the test that matters is the *contrast*: two loops given the
    /// same pair of very different duties, one in AUTO and one in MANUAL.
    ///
    /// Each is marched 4000 steps of 0.05 s (200 s, i.e. 20 feedwater time
    /// constants) at 3 MW and again at 12 MW. In AUTO the settled flow must move
    /// with the duty; in MANUAL it must settle on the operator's demand and be
    /// **identical** at both duties. The manual demand used is 5.0 kg/s, chosen
    /// to sit between the two flows AUTO settles at so neither result can be
    /// mistaken for the other.
    ///
    /// # Results (measured 2026-08-13)
    ///
    /// | Mode | Settled flow at 3 MW | at 12 MW |
    /// |---|---|---|
    /// | AUTO | 0.9557 kg/s | 3.8227 kg/s |
    /// | MANUAL (demand 5.0) | **5.0000 kg/s** | **5.0000 kg/s** |
    ///
    /// MANUAL is identical to the last digit at a duty ratio of four; AUTO moves
    /// the flow by a factor of exactly that. Interpretation: MANUAL genuinely
    /// decouples the feed flow from the duty, which is what makes the `op-tj10`
    /// limit cycle reproducible on demand -- hold the flow, and the feedforward
    /// law has nothing left to oscillate.
    #[test]
    fn manual_feedwater_holds_the_commanded_flow_and_auto_does_not() {
        let manual = SecondaryCommands {
            feedwater: FeedwaterCommand::Manual {
                mass_flow_demand: MassRate::new::<kilogram_per_second>(5.0),
            },
            ..SecondaryCommands::default()
        };

        let settle = |commands: SecondaryCommands, mw: f64| {
            let mut loop_ = SteamSecondaryLoop::new();
            for _ in 0..4000 {
                loop_.step(
                    dt(),
                    commands,
                    Power::new::<megawatt>(mw),
                    nominal_hot_side(),
                );
            }
            loop_.mass_flow().get::<kilogram_per_second>()
        };

        let auto_low = settle(design_commands(), 3.0);
        let auto_high = settle(design_commands(), 12.0);
        let manual_low = settle(manual, 3.0);
        let manual_high = settle(manual, 12.0);
        println!(
            "FEEDWATER MODE CONTRAST (settled flow after 200 s):\n  \
             AUTO   at  3 MW = {auto_low:.4} kg/s, at 12 MW = {auto_high:.4} kg/s\n  \
             MANUAL at  3 MW = {manual_low:.4} kg/s, at 12 MW = {manual_high:.4} kg/s \
             (demand 5.0000 kg/s)"
        );

        assert!(
            auto_high > auto_low + 0.5,
            "AUTO must move the flow with the duty ({auto_high} vs {auto_low} kg/s)"
        );
        assert!(
            (manual_high - manual_low).abs() < 1e-9,
            "MANUAL must be independent of duty ({manual_high} vs {manual_low} kg/s)"
        );
        assert!(
            (manual_low - 5.0).abs() < 1e-3,
            "MANUAL settled at {manual_low} kg/s, not the commanded 5.0"
        );
    }

    /// V&V: **the AUTO setpoint actually moves the settled steam temperature,
    /// in the right direction and by roughly the right amount.**
    ///
    /// # Methodology
    ///
    /// A setpoint that is wired up but ineffective would look identical to one
    /// that is not wired up at all, so this measures the response rather than
    /// the plumbing. Three setpoints -- 300, 440 (published) and 500 degC -- each
    /// marched 4000 steps of 0.05 s at a fixed 10 MW duty, reading the settled
    /// turbine-inlet temperature and feed flow.
    ///
    /// The expected mechanism is indirect and worth stating, because it is the
    /// opposite of a thermostat: the controller does not heat the steam, it
    /// **throttles the water**. A higher setpoint means a larger target enthalpy
    /// rise, so `Q/(h_target - h_feed)` calls for *less* flow, and the same duty
    /// spread over less water gives hotter steam.
    ///
    /// Pass criteria: the settled steam temperature increases monotonically with
    /// the setpoint; the feed flow decreases monotonically; and at the published
    /// 440 degC setpoint the settled steam lands within 5 K of 440 degC, which
    /// is the accuracy a feedforward law against a lagging plant can be held to
    /// (it limit-cycles -- see `op-tj10`).
    ///
    /// # Results (measured 2026-08-13)
    ///
    /// | Setpoint | Settled steam | Settled feed flow |
    /// |---|---|---|
    /// | 300 degC | **300.00 degC** | 3.5805 kg/s |
    /// | 440 degC (published) | **439.99 degC** | 3.1856 kg/s |
    /// | 500 degC | **500.00 degC** | 3.0515 kg/s |
    ///
    /// The setpoint is tracked to **0.01 K** at all three points, and the feed
    /// flow falls monotonically as the setpoint rises, which is the mechanism
    /// described above.
    ///
    /// **Do not read that 0.01 K as controller quality.** This test hands the
    /// loop a *constant* duty with no exchanger behind it, which is the one
    /// situation a feedforward law handles perfectly -- `Q/(h_target - h_feed)`
    /// is then exactly right. Against the real exchanger's 38 s metal lag the
    /// same law limit-cycles (kopi-beans `op-tj10`), and the coupled measurement
    /// in [`the_absorbable_duty_cap_no_longer_binds`] shows the swing.
    #[test]
    fn the_auto_setpoint_moves_the_settled_steam_temperature() {
        let settle = |setpoint_c: f64| {
            let commands = SecondaryCommands {
                feedwater: FeedwaterCommand::Auto {
                    target_steam_temperature: ThermodynamicTemperature::new::<degree_celsius>(
                        setpoint_c,
                    ),
                },
                ..SecondaryCommands::default()
            };
            let mut loop_ = SteamSecondaryLoop::new();
            for _ in 0..4000 {
                loop_.step(
                    dt(),
                    commands,
                    Power::new::<megawatt>(10.0),
                    nominal_hot_side(),
                );
            }
            (
                loop_.turbine_inlet_temperature().get::<degree_celsius>(),
                loop_.mass_flow().get::<kilogram_per_second>(),
            )
        };

        let (t300, f300) = settle(300.0);
        let (t440, f440) = settle(440.0);
        let (t500, f500) = settle(500.0);
        println!(
            "AUTO SETPOINT SWEEP (10 MW duty, settled after 200 s):\n  \
             setpoint 300 degC -> steam {t300:.2} degC at {f300:.4} kg/s\n  \
             setpoint 440 degC -> steam {t440:.2} degC at {f440:.4} kg/s  (published)\n  \
             setpoint 500 degC -> steam {t500:.2} degC at {f500:.4} kg/s"
        );

        assert!(
            t300 < t440 && t440 < t500,
            "the settled steam temperature must rise with the setpoint \
             ({t300}, {t440}, {t500} degC)"
        );
        assert!(
            f300 > f440 && f440 > f500,
            "a higher steam setpoint must call for LESS feedwater \
             ({f300}, {f440}, {f500} kg/s)"
        );
        assert!(
            (t440 - 440.0).abs() < 5.0,
            "at the published setpoint the steam settled at {t440} degC"
        );
    }

    /// V&V: **raising the condenser back-pressure costs turbine work and warms
    /// the feedwater**, which is the closed cycle responding as a real one does.
    ///
    /// # Methodology
    ///
    /// The condenser pressure sets the bottom of the Rankine cycle. Raising it
    /// must, simultaneously:
    ///
    /// 1. **reduce turbine work**, because the isentropic expansion ends at a
    ///    higher pressure and so a higher enthalpy;
    /// 2. **raise the exhaust quality**, because the expansion line stops
    ///    further up into the two-phase dome;
    /// 3. **warm the feedwater**, because the hotwell condensate is the
    ///    saturated liquid at that pressure.
    ///
    /// All three follow from the same flash, and a model that got any of them
    /// backwards would be one where the condenser pressure was not really
    /// reaching the cycle. Each of the three pressures -- the 4 kPa floor, the
    /// 7 kPa design point and the 30 kPa ceiling -- is marched 4000 steps of
    /// 0.05 s at a fixed 10 MW duty in AUTO.
    ///
    /// # Results (measured 2026-08-13)
    ///
    /// | Back-pressure | Turbine power | Exhaust quality | Feedwater enthalpy |
    /// |---|---|---|---|
    /// | 4 kPa (floor) | **3.2804 MW** | 0.8810 | 126.75 kJ/kg |
    /// | 7 kPa (design) | **3.1499 MW** | 0.8951 | 168.73 kJ/kg |
    /// | 30 kPa (ceiling) | **2.7591 MW** | 0.9366 | 294.64 kJ/kg |
    ///
    /// All three quantities move monotonically and in the textbook direction.
    /// Over the full commandable range the turbine loses **0.52 MW, 16% of its
    /// output**, which is the right order for a 4.0 MPa cycle losing its vacuum:
    /// the condenser command is a real cycle boundary condition, not a display
    /// value.
    ///
    /// Note the exhaust quality *improves* as the back-pressure rises -- less
    /// expansion means less moisture -- so a degraded vacuum is not uniformly
    /// bad for the machine, only for its output. The model reproduces that
    /// trade-off.
    #[test]
    fn raising_the_condenser_pressure_costs_turbine_work() {
        let settle = |kpa: f64| {
            let commands = SecondaryCommands {
                condenser_pressure: Pressure::new::<kilopascal>(kpa),
                ..SecondaryCommands::default()
            };
            let mut loop_ = SteamSecondaryLoop::new();
            for _ in 0..4000 {
                loop_.step(
                    dt(),
                    commands,
                    Power::new::<megawatt>(10.0),
                    nominal_hot_side(),
                );
            }
            (
                loop_.turbine_power().get::<watt>() / 1.0e6,
                loop_.steam_quality_after_turbine(),
                loop_.feedwater_enthalpy().get::<joule_per_kilogram>() / 1.0e3,
            )
        };

        let (lo, hi) = ranges::CONDENSER_PRESSURE_KPA;
        let (p_lo, x_lo, h_lo) = settle(lo);
        let (p_design, x_design, h_design) = settle(7.0);
        let (p_hi, x_hi, h_hi) = settle(hi);
        println!(
            "CONDENSER PRESSURE SWEEP (10 MW duty, settled after 200 s):\n  \
             {lo:>4.1} kPa -> turbine {p_lo:.4} MW, exhaust quality {x_lo:.4}, \
             feedwater {h_lo:.2} kJ/kg\n  \
             {:>4.1} kPa -> turbine {p_design:.4} MW, exhaust quality {x_design:.4}, \
             feedwater {h_design:.2} kJ/kg  (design)\n  \
             {hi:>4.1} kPa -> turbine {p_hi:.4} MW, exhaust quality {x_hi:.4}, \
             feedwater {h_hi:.2} kJ/kg",
            7.0
        );

        assert!(
            p_lo > p_design && p_design > p_hi,
            "turbine work must fall as the back-pressure rises \
             ({p_lo}, {p_design}, {p_hi} MW)"
        );
        assert!(
            x_lo < x_design && x_design < x_hi,
            "exhaust quality must rise as the back-pressure rises \
             ({x_lo}, {x_design}, {x_hi})"
        );
        assert!(
            h_lo < h_design && h_design < h_hi,
            "the feedwater must warm as the back-pressure rises \
             ({h_lo}, {h_design}, {h_hi} kJ/kg)"
        );
    }

    /// V&V: **the secondary residence time is a transport time, and the
    /// schematic's flow tracers therefore move.**
    ///
    /// # Why this exists
    ///
    /// The maintainer reported on 2026-08-13 that "the flow animations for the
    /// secondary loop is not working". They were not broken; they were about
    /// forty times too slow to see. The tracer speed is
    /// `1 / residence_time` of a run per second, and the residence time was
    /// built from a flat invented **2000 kg** "secondary inventory", giving
    /// `tau = 2000 / 3.13 = 639 s` -- over ten minutes for one traversal.
    ///
    /// The defect was the *quantity*, not the number: 2000 kg is a plausible
    /// plant water inventory, but most of it is vessel holdup that is not on the
    /// transport path a pipe run depicts. See
    /// [`SteamSecondaryLoop::piping_inventory`].
    ///
    /// # Methodology
    ///
    /// The loop is settled at the design 10 MW duty in AUTO, then the piping
    /// inventory and the residence time `m / m_dot` are read. Two properties are
    /// checked, neither of which is "it looks right":
    ///
    /// 1. **Magnitude.** The residence time must be a plausible pipe transport
    ///    time -- asserted between 1 s and 60 s. The lower bound matters as much
    ///    as the upper: a sub-second traversal would be a blur, and would mean
    ///    the piping volumes had been shrunk to suit the animation.
    /// 2. **Inverse scaling with flow.** Halving the flow must roughly double
    ///    the residence time, because `tau = m/m_dot` and the piping mass is set
    ///    by the fluid *states*, not by the flow. Checked at two MANUAL feed
    ///    flows a factor of two apart. The product `tau * m_dot` is the piping
    ///    mass, so it should be *nearly* invariant -- but not exactly, and the
    ///    tolerance says so: at 2 kg/s the same 10 MW duty superheats the steam
    ///    further than at 4 kg/s, thinning it, so the main steam line holds less.
    ///    Pass criterion **10%**, with the measured residual recorded below.
    ///
    /// 3. **Comparison with the primary loop.** The primary's tracers are the
    ///    ones that visibly work, and they are driven by the same
    ///    `residence_time_from_flow` over a `rho V` inventory. The secondary's
    ///    figure must land in a broadly similar band -- asserted within a factor
    ///    of ten of the primary's at its own nominal point. Outside that, one of
    ///    the two loops is being animated on a different kind of quantity again.
    /// 4. **Response to plant state.** The whole reason for deriving the
    ///    inventory rather than picking a constant is that it must move with
    ///    pressure and temperature. At a *fixed* MANUAL feed flow, a higher duty
    ///    makes the steam hotter and thinner, so the steam line must hold less
    ///    and the residence time must fall. A constant inventory cannot do this,
    ///    so this leg is what distinguishes the two fixes.
    ///
    /// **No visual claim is made.** This test cannot see the screen and does not
    /// pretend to; it measures the number the tracer speed is computed from.
    ///
    /// # Results (measured 2026-08-13)
    ///
    /// | | Before (flat 2000 kg) | After (derived `rho V`) |
    /// |---|---|---|
    /// | Piping inventory at the 10 MW design point | 2000 kg | **49.82 kg** |
    /// | Residence time at the settled 3.1856 kg/s | **627.8 s** | **15.64 s** |
    ///
    /// A **40x** speed-up, from over ten minutes per traversal to about sixteen
    /// seconds -- the same order as the primary loop's, which is the one that
    /// visibly works.
    ///
    /// **Inverse scaling with flow**, at a fixed 10 MW duty:
    ///
    /// | MANUAL flow | Piping inventory | `tau` |
    /// |---|---|---|
    /// | 2.00 kg/s | 48.93 kg | 24.46 s |
    /// | 4.00 kg/s | 51.99 kg | 13.00 s |
    ///
    /// `tau` ratio **1.882** against an ideal 2.000; equivalently `tau * m_dot`
    /// moves **-5.9%**. That residual is the steam density doing its job: the
    /// slower flow leaves the steam generator hotter and thinner. A model in
    /// which the number were exactly 2.000 would be one where the inventory had
    /// gone back to being a constant.
    ///
    /// **Primary-loop comparator**, at its own published operating point:
    /// **15.28 kg** of helium at 4.30 kg/s gives `tau = 3.55 s`. The secondary's
    /// 15.64 s is **4.4x** that -- the same order, and correctly the slower of
    /// the two, because the secondary's transport path is dominated by a liquid
    /// line. Before this change the ratio was **177x**.
    ///
    /// **Response to plant state**, at a fixed 3.00 kg/s MANUAL flow -- the
    /// property a constant inventory cannot have:
    ///
    /// | Duty | Steam temperature | Mass in the steam line | Piping total | `tau` |
    /// |---|---|---|---|---|
    /// | 6 MW | 523.5 K (saturated) | **7.40 kg** | 54.31 kg | 18.10 s |
    /// | 11 MW | 942.7 K | **2.20 kg** | 49.01 kg | 16.34 s |
    ///
    /// The steam line holds **3.4x less mass** when the steam is 419 K hotter,
    /// at exactly the same mass flow, and the residence time falls with it. A
    /// flat constant would have given two identical rows.
    ///
    /// # Interpretation
    ///
    /// The tracer is now driven by a transport time that responds to pressure,
    /// temperature and flow, in the same way the primary's already did. What
    /// this does **not** establish is that the animation reads well on screen --
    /// that is a visual judgement this test cannot make.
    #[test]
    fn the_secondary_piping_residence_time_is_a_real_transport_time() {
        let settle_at = |commands: SecondaryCommands| {
            let mut loop_ = SteamSecondaryLoop::new();
            for _ in 0..4000 {
                loop_.step(
                    dt(),
                    commands,
                    Power::new::<megawatt>(10.0),
                    nominal_hot_side(),
                );
            }
            let m = loop_.piping_inventory().get::<kilogram>();
            let m_dot = loop_.mass_flow().get::<kilogram_per_second>();
            (m, m_dot, m / m_dot)
        };

        let (mass, flow, tau) = settle_at(design_commands());
        // The superseded constant, kept in the printout so the change is
        // legible rather than asserted against a number nobody can see.
        const OLD_SECONDARY_INVENTORY_KG: f64 = 2.0e3;
        println!(
            "SECONDARY TRACER RESIDENCE TIME (10 MW, AUTO, settled after 200 s):\n  \
             piping inventory = {mass:.2} kg at {flow:.4} kg/s -> tau = {tau:.2} s\n  \
             the superseded flat {OLD_SECONDARY_INVENTORY_KG:.0} kg inventory would give \
             tau = {:.1} s at the same flow",
            OLD_SECONDARY_INVENTORY_KG / flow
        );

        assert!(
            (1.0..=60.0).contains(&tau),
            "the secondary residence time is {tau} s; outside 1-60 s it is either too slow \
             to see moving or too fast to read, and in both cases it is no longer a \
             plausible pipe transport time"
        );

        // Inverse scaling with flow: tau * m_dot is the piping mass, which the
        // flow itself does not set.
        let slow = SecondaryCommands {
            feedwater: FeedwaterCommand::Manual {
                mass_flow_demand: MassRate::new::<kilogram_per_second>(2.0),
            },
            ..SecondaryCommands::default()
        };
        let fast = SecondaryCommands {
            feedwater: FeedwaterCommand::Manual {
                mass_flow_demand: MassRate::new::<kilogram_per_second>(4.0),
            },
            ..SecondaryCommands::default()
        };
        let (m_slow, f_slow, tau_slow) = settle_at(slow);
        let (m_fast, f_fast, tau_fast) = settle_at(fast);
        println!(
            "  at {f_slow:.2} kg/s: {m_slow:.2} kg -> tau = {tau_slow:.2} s\n  \
             at {f_fast:.2} kg/s: {m_fast:.2} kg -> tau = {tau_fast:.2} s  \
             (ratio {:.3}, ideal 2.000)",
            tau_slow / tau_fast
        );
        assert!(
            tau_slow > tau_fast,
            "halving the flow must lengthen the residence time"
        );
        // 3. The primary loop, as the sanity comparator: same
        //    `residence_time_from_flow` over the same kind of `rho V`
        //    inventory, at its own published operating point.
        let primary = super::super::primary_loop::HeliumPrimaryLoop::new(
            super::super::pebble_bed::nominal_helium_flow(),
        );
        let primary_mass = primary.helium_inventory().get::<kilogram>();
        let primary_flow = primary.mass_flow().get::<kilogram_per_second>();
        let primary_tau = primary_mass / primary_flow;
        println!(
            "  PRIMARY comparator: {primary_mass:.2} kg of helium at {primary_flow:.2} kg/s \
             -> tau = {primary_tau:.2} s"
        );
        assert!(
            tau < primary_tau * 10.0 && tau > primary_tau / 10.0,
            "the secondary tracer time {tau} s is more than a factor of ten from the \
             primary's {primary_tau} s; the two loops are being animated on different \
             kinds of quantity again"
        );

        // 4. Response to plant state at a FIXED flow. A constant inventory
        //    cannot produce this, which is the point of deriving it.
        let at_duty = |mw: f64| {
            let commands = SecondaryCommands {
                feedwater: FeedwaterCommand::Manual {
                    mass_flow_demand: MassRate::new::<kilogram_per_second>(3.0),
                },
                ..SecondaryCommands::default()
            };
            let mut loop_ = SteamSecondaryLoop::new();
            for _ in 0..4000 {
                loop_.step(
                    dt(),
                    commands,
                    Power::new::<megawatt>(mw),
                    nominal_hot_side(),
                );
            }
            // The steam line's own mass, computed here from the same constant
            // and the same live IF97 state `piping_inventory` uses, so the leg
            // that actually responds is visible rather than buried in the sum.
            let v_steam = loop_
                .steam_generator_outlet()
                .get_specific_volume()
                .get::<cubic_meter_per_kilogram>();
            (
                loop_.turbine_inlet_temperature().get::<kelvin>(),
                MAIN_STEAM_LINE_VOLUME_M3 / v_steam,
                loop_.piping_inventory().get::<kilogram>(),
                loop_.piping_inventory().get::<kilogram>()
                    / loop_.mass_flow().get::<kilogram_per_second>(),
            )
        };
        let (t_cool, m_line_cool, m_cool, tau_cool) = at_duty(6.0);
        let (t_hot, m_line_hot, m_hot, tau_hot) = at_duty(11.0);
        println!(
            "  STATE RESPONSE at a fixed 3.00 kg/s MANUAL flow:\n    \
              6 MW: steam {t_cool:.1} K, steam line holds {m_line_cool:.2} kg, \
             piping total {m_cool:.2} kg -> tau = {tau_cool:.2} s\n    \
             11 MW: steam {t_hot:.1} K, steam line holds {m_line_hot:.2} kg, \
             piping total {m_hot:.2} kg -> tau = {tau_hot:.2} s"
        );
        assert!(
            t_hot > t_cool,
            "the higher duty must give hotter steam at the same flow"
        );
        assert!(
            m_line_hot < m_line_cool,
            "hotter, thinner steam must leave LESS mass in the steam line \
             ({m_line_hot} vs {m_line_cool} kg)"
        );
        assert!(
            tau_hot < tau_cool,
            "the residence time must respond to plant state, not just to flow \
             ({tau_hot} vs {tau_cool} s). A constant inventory would give exactly \
             equal values here, which is the fix this test exists to rule out."
        );

        let ratio = (tau_slow * f_slow) / (tau_fast * f_fast);
        assert!(
            (ratio - 1.0).abs() < 0.10,
            "tau * m_dot moved by {:.1}% between the two flows. It is the piping mass, \
             which the flow does not set directly -- a few percent is the steam density \
             responding to the different duty per unit flow (measured -5.9% on \
             2026-08-13), but a large move means the residence time has stopped being \
             `mass / flow`",
            100.0 * (ratio - 1.0)
        );
    }

    /// Methodology: the saturation temperature at the **published** HTR-10 main
    /// steam pressure of 4.0 MPa is compared against the IAPWS-IF97 saturation
    /// line, `T_sat(4.0 MPa) = 250.35 degC = 523.50 K`. This is the pinch
    /// temperature the primary loop's steam generator is limited by, so an
    /// error here would silently mis-size the whole coupling -- and at HTR-10
    /// scale it does double duty, because the published 250 degC core inlet
    /// sits almost exactly on this saturation temperature. Pass criterion:
    /// within 1 K of the reference.
    ///
    /// Results (2026-08-12, tampines-steam-tables IF97):
    /// `T_sat(4.0 MPa) = 523.5075 K` against the 523.50 K reference --
    /// agreement to 0.008 K. The coupling temperature handed to the primary
    /// loop is the real saturation line, not an assumed constant.
    ///
    /// This test previously checked 10 MPa / 584.15 K, from when the simulator
    /// modelled a ~200 MWth prismatic plant with an invented steam pressure.
    /// The pressure is now the published 4.0 MPa, so the reference moved with
    /// it; the check itself is unchanged in kind.
    #[test]
    fn saturation_temperature_matches_if97_reference() {
        let loop_ = SteamSecondaryLoop::new();
        let t_sat = loop_.saturation_temperature().get::<kelvin>();
        assert!(
            (t_sat - 523.50).abs() < 1.0,
            "T_sat(4.0 MPa) = {t_sat} K departs from the IF97 reference 523.50 K"
        );
    }

    /// Methodology: feedwater enthalpy must be computed, not fixed -- it has
    /// to exceed the condensate enthalpy by exactly the incompressible pump
    /// work `v dp / eta`, and the feed pump must draw positive power. Pass
    /// criterion: the rise matches `v dp / eta` to 1e-9 relative.
    ///
    /// Results (2026-08-12, at the published 4.0 MPa steam pressure):
    /// `h_condensate = 163.37 kJ/kg` (saturated liquid at 7 kPa) and
    /// `h_feed = 168.73 kJ/kg`, a 5.36 kJ/kg pump rise matching `v dp / eta` to
    /// round-off, drawing 17.1 kW at the settled 3.19 kg/s feed flow.
    #[test]
    fn feedwater_enthalpy_is_condensate_plus_real_pump_work() {
        let mut loop_ = SteamSecondaryLoop::new();
        loop_.step(
            dt(),
            design_commands(),
            Power::new::<megawatt>(10.0),
            nominal_hot_side(),
        );

        let h_feed = loop_.feedwater_enthalpy().get::<joule_per_kilogram>();
        let h_cond = loop_
            .condensate()
            .get_specific_enthalpy()
            .get::<joule_per_kilogram>();
        assert!(
            h_feed > h_cond,
            "feed pump must raise the condensate enthalpy"
        );
        assert!(loop_.feed_pump_power().get::<watt>() > 0.0);

        // The rise must equal v*dp/eta to within round-off.
        let v = loop_
            .condensate()
            .get_specific_volume()
            .get::<cubic_meter_per_kilogram>();
        let dp = (loop_.steam_pressure() - loop_.condenser_pressure())
            .get::<uom::si::pressure::pascal>();
        let expected = v * dp / FEED_PUMP_EFFICIENCY;
        assert!((h_feed - h_cond - expected).abs() / expected < 1e-9);
    }

    /// Methodology: the condenser energy balance must close -- the duty it
    /// rejects must equal the cooling-water stream's enthalpy pickup,
    /// `m_cw c_p (T_out - T_in)`. Pass criterion: agreement to 1e-6 relative.
    ///
    /// Results (2026-08-12): at the plant's nominal 10 MW steam-generator duty
    /// the condenser rejected 6.868 MW into the cooling water, raising it
    /// 8.215 K above its 298.15 K inlet. The two sides agreed to well under
    /// 1e-6 relative -- round-off. The balance closes.
    #[test]
    fn condenser_energy_balance_closes_onto_the_cooling_water() {
        let mut loop_ = SteamSecondaryLoop::new();
        for _ in 0..2000 {
            loop_.step(
                dt(),
                design_commands(),
                Power::new::<megawatt>(10.0),
                nominal_hot_side(),
            );
        }

        let duty = loop_.condenser_duty().get::<watt>();
        assert!(duty > 0.0, "condenser must reject heat at load");

        let rise = loop_.cooling_water_outlet_temperature().get::<kelvin>() - COOLING_WATER_INLET_K;
        assert!(rise > 0.0, "cooling water must heat up");

        let carried = COOLING_WATER_FLOW_KG_PER_S * COOLING_WATER_CP_J_PER_KG_K * rise;
        assert!(
            (carried - duty).abs() / duty < 1e-6,
            "condenser duty {duty} W does not match the cooling-water pickup {carried} W"
        );
    }

    /// The feedwater controller must move the flow with the duty rather than
    /// holding a fixed value, and must stay inside the pump's range.
    #[test]
    fn feedwater_flow_tracks_duty_and_stays_in_range() {
        let mut low = SteamSecondaryLoop::new();
        let mut high = SteamSecondaryLoop::new();
        for _ in 0..4000 {
            low.step(
                dt(),
                design_commands(),
                Power::new::<megawatt>(3.0),
                nominal_hot_side(),
            );
            high.step(
                dt(),
                design_commands(),
                Power::new::<megawatt>(12.0),
                nominal_hot_side(),
            );
        }

        let low_flow = low.mass_flow().get::<kilogram_per_second>();
        let high_flow = high.mass_flow().get::<kilogram_per_second>();
        assert!(
            high_flow > low_flow,
            "higher duty must call for more feedwater ({high_flow} vs {low_flow} kg/s)"
        );
        for f in [low_flow, high_flow] {
            assert!((MIN_SECONDARY_FLOW_KG_PER_S..=MAX_SECONDARY_FLOW_KG_PER_S).contains(&f));
        }
    }

    /// Net power must be the turbine output less the feed-pump work, and the
    /// cycle must be a net producer at load.
    #[test]
    fn net_power_nets_off_the_feed_pump() {
        let mut loop_ = SteamSecondaryLoop::new();
        for _ in 0..2000 {
            loop_.step(
                dt(),
                design_commands(),
                Power::new::<megawatt>(10.0),
                nominal_hot_side(),
            );
        }
        let net = loop_.net_power().get::<watt>();
        let expected = loop_.turbine_power().get::<watt>() - loop_.feed_pump_power().get::<watt>();
        assert!((net - expected).abs() < 1e-6);
        assert!(
            net > 0.0,
            "the cycle should be a net power producer at load"
        );
    }

    /// Methodology: the cycle's first law must not be violated -- the turbine
    /// work extracted can never exceed the heat added by the steam generator.
    /// Pass criterion: `W_turbine < Q_ihx` at a representative load.
    ///
    /// Results (2026-08-12): at the nominal 10 MW steam-generator duty the
    /// turbine produced 3.149 MW, a thermal efficiency of 31.5% (net 3.132 MW,
    /// 31.3%, after the 17.1 kW feed pump), at a settled feed flow of
    /// 3.187 kg/s against the published 12.5 t/hr = 3.47 kg/s main steam flow.
    /// The turbine inlet settled at **712.77 K = 439.6 degC**, against the
    /// published 440 degC main steam temperature -- 0.4 K, which confirms the
    /// target enthalpy constant was taken at the right state. The efficiency is
    /// plausible for a 4.0 MPa Rankine cycle rejecting to 7 kPa with an
    /// 0.85-efficient turbine, and sits below the 40.4% Carnot bound between
    /// `T_sat(4.0 MPa) = 523.5 K` and the 312 K condenser. Exhaust quality
    /// 0.895.
    #[test]
    fn turbine_work_never_exceeds_heat_input() {
        let mut loop_ = SteamSecondaryLoop::new();
        let duty = Power::new::<megawatt>(10.0);
        for _ in 0..2000 {
            loop_.step(dt(), design_commands(), duty, nominal_hot_side());
        }
        let w = loop_.turbine_power().get::<watt>();
        let q = duty.get::<watt>();
        assert!(w < q, "turbine work {w} W exceeds heat input {q} W");
        assert!(w > 0.0);
    }

    /// The steam side can never leave the hot side hotter than it arrived --
    /// no temperature cross, across the whole reachable duty range.
    ///
    /// **Why this test exists.** The simulator used to crash on a fast power
    /// rise, and the cause was a genuine second-law violation, not a display
    /// artefact. The primary loop's effectiveness-NTU pinch caps duty against
    /// the steam *saturation* temperature and its own test asserts only on the
    /// HELIUM side (`T_sink <= T_sg_out <= T_out`). Nothing constrained the
    /// steam OUTLET, because the secondary is modelled as an isothermal sink.
    /// With helium near 973 K and saturation at 523 K the pinch authorises a
    /// large duty, and while the first-order feedwater controller is still
    /// lagging near `MIN_SECONDARY_FLOW_KG_PER_S` that duty superheats the
    /// steam far above the helium that heated it. Because IF97 **panics**
    /// rather than returning an error outside its range, the symptom was a
    /// dead physics thread and a restart modal.
    ///
    /// **Methodology.** Two parts.
    ///
    /// 1. *The regime that used to crash.* Slam a cold-started loop -- feed
    ///    flow still at its floor -- with duties from 1 to 40 MW, well past the
    ///    ~32 MW at which the feed controller saturates against
    ///    `MAX_SECONDARY_FLOW_KG_PER_S`. Step each 200 times. Assert the
    ///    turbine inlet temperature never exceeds the 973.15 K hot side, and
    ///    that nothing panics.
    /// 2. *The pre-fix formula is retained and shown to violate.* Recompute
    ///    `h_feed + Q/m_dot` with no cap over the same sweep and count how many
    ///    points exceed the hot-side enthalpy. This is kept permanently so the
    ///    test cannot silently degrade into asserting something trivially true.
    ///
    /// **Results (2026-08-12, measured).** Part 1: the capped model held at
    /// every one of the 40 duties x 200 steps. Peak turbine inlet temperature
    /// reached **973.1477 K** against a 973.15 K hot side -- it saturates
    /// against the cap to within 3 mK, which is the cap binding as designed --
    /// and never exceeded it. No panic anywhere in the sweep. Part 2: the
    /// uncapped formula exceeded the hot-side enthalpy at **28 of the 40** duty
    /// points on the first step, i.e. at every duty above roughly 13 MW while
    /// the feed flow is still at its floor.
    ///
    /// **Part 3 added 2026-08-12 -- the cap must now pass with MARGIN, not by
    /// clamping.** Parts 1 and 2 deliberately slam the loop with synthetic
    /// duties far outside anything a real exchanger can produce, so the cap
    /// *does* bind there and part 1 saturates against it to within 3 mK. That
    /// proves the backstop works; it says nothing about whether the plant is
    /// relying on it. Part 3 asks the question that matters: over the duty range
    /// a resolved 10 MWth steam generator can actually deliver, and at the
    /// settled feed flow, how far is the cap from binding? Pass criterion: peak
    /// utilisation `Q_offered/Q_max` strictly below 1, and the steam outlet at
    /// least 100 K below the hot side.
    ///
    /// **Part 3 results (measured 2026-08-12).** Over 8 to 12 MW at the settled
    /// feed flow, marched 2000 steps each: peak utilisation **0.9242** and peak
    /// steam **850.56 K** against the 973.15 K hot side -- inactive, but only
    /// just, because these are *synthetic* duties handed straight in with no
    /// exchanger between them and the water. See
    /// [`tests::the_absorbable_duty_cap_no_longer_binds`] for the closed-loop
    /// version, which is the one that answers "does the plant lean on the
    /// backstop in normal operation" -- and which measures a larger margin
    /// precisely because the duty there comes from a real exchanger.
    ///
    /// **Interpretation.** The crash was the severe form of a temperature
    /// cross; a mild overshoot would merely have shown steam hotter than the
    /// helium heating it. The cap removes both by construction rather than by
    /// widening a tolerance. Note this bounds the *outlet* only. The internal
    /// temperature profile is no longer unmodelled, though -- it is resolved in
    /// [`super::super::steam_generator`], and its node-by-node no-cross property
    /// is checked in
    /// `super::super::primary_loop::tests::steam_generator_has_no_node_by_node_temperature_cross`.
    #[test]
    fn no_duty_can_drive_a_temperature_cross_on_the_steam_side() {
        let hot_side_k = nominal_hot_side().get::<kelvin>();
        let mut worst_capped_k = 0.0_f64;

        for mw in 1..=40 {
            let mut loop_ = SteamSecondaryLoop::new();
            let duty = Power::new::<megawatt>(mw as f64);
            for _ in 0..200 {
                loop_.step(dt(), design_commands(), duty, nominal_hot_side());
                let t_steam = loop_.turbine_inlet_temperature().get::<kelvin>();
                assert!(
                    t_steam <= hot_side_k + 1e-6,
                    "steam left the generator at {t_steam} K, hotter than the \
                     {hot_side_k} K helium that heated it, at {mw} MW"
                );
                worst_capped_k = worst_capped_k.max(t_steam);
            }
        }
        assert!(
            worst_capped_k > 700.0,
            "sweep never approached the cap ({worst_capped_k} K); it is not \
             exercising the regime it claims to"
        );

        // Part 2: the pre-fix formula, retained so this test keeps its teeth.
        let mut violations = 0;
        for mw in 1..=40 {
            let mut loop_ = SteamSecondaryLoop::new();
            let duty = Power::new::<megawatt>(mw as f64);
            loop_.step(dt(), design_commands(), duty, nominal_hot_side());

            let h_feed = loop_.feedwater_enthalpy().get::<joule_per_kilogram>();
            let flow = loop_.mass_flow().get::<kilogram_per_second>();
            let h_uncapped = h_feed + duty.get::<watt>() / flow;
            let h_ceiling = h_feed
                + max_absorbable_duty(steam_pressure(), nominal_hot_side(), flow, h_feed) / flow;
            if h_uncapped > h_ceiling {
                violations += 1;
            }
        }
        assert!(
            violations > 20,
            "the uncapped formula violated at only {violations} of 40 points; \
             if this drops, the cap may no longer be doing anything"
        );

        // Part 3: at duties a real 10 MWth exchanger can produce, the cap must
        // be inactive with margin.
        let mut worst_utilisation = 0.0_f64;
        let mut worst_steam_k = 0.0_f64;
        for mw in 8..=12 {
            let mut loop_ = SteamSecondaryLoop::new();
            let duty = Power::new::<megawatt>(mw as f64);
            for _ in 0..2000 {
                loop_.step(dt(), design_commands(), duty, nominal_hot_side());
                worst_utilisation = worst_utilisation.max(loop_.absorbable_duty_utilisation());
                worst_steam_k =
                    worst_steam_k.max(loop_.turbine_inlet_temperature().get::<kelvin>());
            }
        }
        println!(
            "part 3, 8-12 MW at settled flow: peak Q_offered/Q_max = {worst_utilisation:.4}, \
             peak steam = {worst_steam_k:.2} K against a {hot_side_k:.2} K hot side"
        );
        assert!(
            worst_utilisation < 1.0,
            "the absorbable-duty cap still binds at realistic duties \
             (peak utilisation {worst_utilisation})"
        );
        assert!(
            worst_steam_k < hot_side_k - 100.0,
            "steam reached {worst_steam_k} K, within 100 K of the {hot_side_k} K hot side"
        );
    }

    /// V&V: **the second-law backstop no longer binds in normal plant
    /// operation**, measured on the coupled primary + secondary loops.
    ///
    /// # Why this is the test that matters
    ///
    /// `max_absorbable_duty` was introduced as a crash fix, and it worked: it
    /// turned a dead physics thread into a running simulator. But it worked by
    /// **clamping the steam outlet at the helium inlet temperature**, so the
    /// price of not crashing was a plainly wrong number on screen -- steam as hot
    /// as the helium heating it. A backstop that is load-bearing is not a
    /// backstop, it is the model.
    ///
    /// The duty is now produced by the resolved counter-flow exchanger in
    /// [`super::super::steam_generator`], whose tube-side outlet cannot exceed
    /// the local helium temperature by construction. This test measures whether
    /// that made the clamp genuinely inactive, rather than assuming it.
    ///
    /// # Methodology
    ///
    /// The helium primary loop and this secondary loop are stepped together for
    /// 6000 steps of 0.05 s (300 s of simulated time) at 10 MWth and the
    /// published 4.3 kg/s, exactly as [`super::super::HtgrPlant::step`] wires
    /// them: the secondary's feedwater state drives the exchanger's tube side,
    /// and the exchanger's tube-side duty drives this loop. The maximum of
    /// [`SteamSecondaryLoop::absorbable_duty_utilisation`] is recorded, and the
    /// steam temperature is compared against the helium entering the exchanger.
    ///
    /// Both are reported twice: over the **whole run**, and over **normal
    /// operation**, defined as everything after the first 10 s. The distinction
    /// is not a convenience -- the exchanger is seeded on a linear arrangement
    /// and spends its first second or so shedding the difference between that
    /// arrangement and the real one, which is a start-up artefact of the seed
    /// rather than a plant behaviour.
    ///
    /// Pass criteria: the backstop never binds at all (whole-run utilisation
    /// strictly below 1.0); in normal operation it is below **0.9**, a stated
    /// 10% margin rather than merely "did not quite bind"; and the steam stays
    /// 100 K (whole run) / 150 K (normal operation) below the core-outlet helium.
    ///
    /// # Results (measured 2026-08-12)
    ///
    /// | Measure | Whole run | After 10 s |
    /// |---|---|---|
    /// | Peak `Q_offered/Q_max` | **0.9088** | **0.8807** |
    /// | Closest steam-to-helium approach | **147.5 K** | **193.1 K** |
    ///
    /// The backstop is therefore inactive throughout, with **12% margin in
    /// normal operation** and 9% even through start-up. Before this change it
    /// was routinely binding, which is what put steam as hot as the helium on
    /// screen.
    ///
    /// **Settled plant design point at 300 s** -- this is the plant's own, with
    /// the feedwater controller in the loop:
    ///
    /// | Quantity | Measured | Published | Delta |
    /// |---|---|---|---|
    /// | Steam outlet | 714.40 K = **441.25 degC** | 440 degC | **+1.25 K** |
    /// | Feed flow | **3.1269 kg/s** | 3.4722 kg/s | -9.9% |
    /// | Core outlet | 994.81 K = **721.66 degC** | 700 degC | +21.7 K |
    /// | Core inlet | 547.12 K = **273.97 degC** | 250 degC | +24.0 K |
    /// | Turbine power | **3.096 MW** | -- (invented BOP) | -- |
    ///
    /// The steam temperature is the maintainer-visible symptom, and it is fixed:
    /// **441.25 degC against a published 440 degC**. Before this change the
    /// steam read as hot as the helium heating it, because the backstop below
    /// was clamping it there. The ~22 K the *helium* terminals sit above
    /// published is the `UA` calibration's residual against an 8-node
    /// discretisation; see
    /// [`super::super::primary_loop::STEAM_GENERATOR_UA_W_PER_K`]. It was not
    /// tuned out.
    ///
    /// **This is the plant's own design point** -- the feedwater controller is
    /// in the loop here, where
    /// `super::super::primary_loop::tests::steam_generator_has_no_node_by_node_temperature_cross`
    /// holds the feed flow fixed at 3.19 kg/s and lands 12 K lower.
    ///
    /// **The settled state is a slow limit cycle, not a fixed point.** The steam
    /// outlet swings roughly 698-718 K with a period near 100 s, because the
    /// feedwater controller is a *feedforward* law -- it sets flow from the duty
    /// being offered, `Q/(h_target - h_feed)` -- acting on an exchanger with a
    /// 38 s metal lag. The published 440 degC sits inside that swing. This
    /// oscillation is a property of the controller, not of the exchanger, and it
    /// was invisible before because the secondary had no thermal inertia to
    /// oscillate against. Replacing the feedforward law with real feedback on
    /// steam temperature is the obvious follow-up.
    ///
    /// # Interpretation
    ///
    /// A margin here means the steam temperature on screen is the exchanger's
    /// answer, not a clamp's. It does **not** mean the exchanger is validated --
    /// its `UA` is an explicit calibration and its geometry is part invented.
    #[test]
    fn the_absorbable_duty_cap_no_longer_binds() {
        use super::super::pebble_bed;
        use super::super::primary_loop::HeliumPrimaryLoop;

        let mut primary = HeliumPrimaryLoop::new(pebble_bed::nominal_helium_flow());
        let mut secondary = SteamSecondaryLoop::new();
        let power = Power::new::<megawatt>(10.0);

        let mut worst_utilisation = 0.0_f64;
        let mut worst_approach_k = f64::INFINITY;
        let mut settled_worst_utilisation = 0.0_f64;
        let mut settled_worst_approach_k = f64::INFINITY;

        for _i in 0..6000 {
            let feed_h = secondary.feedwater_enthalpy();
            let feed_flow = secondary.mass_flow();
            // Drive the primary by a specified bed-outlet temperature, the
            // boundary condition `step_hot_leg` now takes. The equivalent of
            // the old `power` argument at this flow and inlet.
            let bed_out = ThermodynamicTemperature::new::<kelvin>(
                primary.core_inlet_temperature().get::<kelvin>()
                    + power.get::<watt>() / (pebble_bed::nominal_helium_flow_kg_per_s() * 5189.3),
            );
            primary.step(
                dt(),
                bed_out,
                pebble_bed::nominal_helium_flow(),
                feed_h,
                feed_flow,
            );
            secondary.step(
                dt(),
                design_commands(),
                primary.steam_generator_duty_to_secondary(),
                primary.core_outlet_temperature(),
            );

            let util = secondary.absorbable_duty_utilisation();
            let approach = primary.core_outlet_temperature().get::<kelvin>()
                - secondary.turbine_inlet_temperature().get::<kelvin>();
            worst_utilisation = worst_utilisation.max(util);
            worst_approach_k = worst_approach_k.min(approach);
            // "Normal operation" excludes the first 10 s, during which the
            // exchanger is still shedding the arrangement it was seeded with.
            if _i >= 200 {
                settled_worst_utilisation = settled_worst_utilisation.max(util);
                settled_worst_approach_k = settled_worst_approach_k.min(approach);
            }
        }

        println!(
            "COUPLED RUN (10 MWth, 4.3 kg/s helium, 300 s of simulated time):\n  \
             peak Q_offered/Q_max, whole run   = {worst_utilisation:.4} (1.0 = the backstop binds)\n  \
             peak Q_offered/Q_max, after 10 s  = {settled_worst_utilisation:.4}\n  \
             closest steam-to-helium approach, whole run  = {worst_approach_k:.2} K\n  \
             closest steam-to-helium approach, after 10 s = {settled_worst_approach_k:.2} K\n  \
             settled steam outlet        = {:.2} K ({:.2} degC), published 440 degC\n  \
             settled feed flow           = {:.4} kg/s, published 3.4722 kg/s\n  \
             settled turbine power       = {:.4} MW\n  \
             settled core outlet         = {:.2} K ({:.2} degC), published 700 degC\n  \
             settled core inlet          = {:.2} K ({:.2} degC), published 250 degC",
            secondary.turbine_inlet_temperature().get::<kelvin>(),
            secondary.turbine_inlet_temperature().get::<kelvin>() - 273.15,
            secondary.mass_flow().get::<kilogram_per_second>(),
            secondary.turbine_power().get::<watt>() / 1.0e6,
            primary.core_outlet_temperature().get::<kelvin>(),
            primary.core_outlet_temperature().get::<kelvin>() - 273.15,
            primary.core_inlet_temperature().get::<kelvin>(),
            primary.core_inlet_temperature().get::<kelvin>() - 273.15,
        );

        assert!(
            worst_utilisation < 1.0,
            "the absorbable-duty backstop BOUND during the run: peak utilisation \
             {worst_utilisation}"
        );
        assert!(
            settled_worst_utilisation < 0.9,
            "the absorbable-duty backstop is still load-bearing in normal operation: \
             peak utilisation {settled_worst_utilisation} (want < 0.9)"
        );
        assert!(
            worst_approach_k > 100.0,
            "the steam came within {worst_approach_k} K of the helium heating it"
        );
        assert!(
            settled_worst_approach_k > 150.0,
            "in normal operation the steam came within {settled_worst_approach_k} K of the \
             helium heating it"
        );
    }

    /// V&V: the **pure-feedback** loop must hold setpoint without a
    /// feedforward term, and must not oscillate.
    ///
    /// # Methodology
    ///
    /// The loop is settled in AUTO at the design duty and the steam-temperature
    /// error is measured. With the feedforward removed there is nothing but the
    /// PI controller closing the loop, so any failure to reach setpoint is the
    /// controller's. Two checks: the settled |error| must be under 2 K, and the
    /// feed flow must not reverse direction more than a handful of times over
    /// the settling window (an integral time inside the actuator lag is the
    /// classic way to make a loop hunt).
    ///
    /// Then the duty is stepped 10 -> 6 MW and the loop must re-settle. This is
    /// the case a feedforward would have handled instantly and pure feedback
    /// has to work for, so it is the honest test of the trade.
    ///
    /// # Results (2026-08-14)
    ///
    /// | Condition | Steam error | Flow | Reversals |
    /// |---|---|---|---|
    /// | Settled AUTO, 10 MW | **+0.000 K** | 3.1856 kg/s | **0** |
    /// | After a 10 -> 6 MW load change | **+0.000 K** | 1.9113 kg/s | -- |
    ///
    /// Controller output at the design point: +0.4777.
    ///
    /// **Interpretation.** Pure feedback holds setpoint *better* than the
    /// feedforward-plus-trim arrangement it replaced, which settled at
    /// -0.024 K: with no open-loop model term to fight, the integrator drives
    /// the error to zero outright. It also absorbs a 40% load change without a
    /// standing offset. The cost is dynamic, not steady-state -- there is no
    /// model term to move the flow the instant the duty changes, so the
    /// approach is slower than a feedforward's would have been. For a
    /// demonstration plant with a 10 s pump lag and a 184 s core, that is not
    /// a cost worth a wrong model.
    #[test]
    fn the_feedback_only_loop_holds_setpoint() {
        let mut loop_ = SteamSecondaryLoop::new();
        let mut reversals = 0;
        let mut previous_flow = loop_.mass_flow().get::<kilogram_per_second>();
        let mut previous_delta = 0.0_f64;
        for _ in 0..12000 {
            loop_.step(
                dt(),
                design_commands(),
                Power::new::<megawatt>(10.0),
                nominal_hot_side(),
            );
            let flow = loop_.mass_flow().get::<kilogram_per_second>();
            let delta = flow - previous_flow;
            if delta * previous_delta < 0.0 && delta.abs() > 1e-6 {
                reversals += 1;
            }
            previous_delta = delta;
            previous_flow = flow;
        }
        let settled_error = loop_.feedwater_controller().error();
        println!(
            "feedback-only, settled at 10 MW: error {settled_error:+.3} K, \
             output {:+.4}, flow {:.4} kg/s, {reversals} reversals",
            loop_.feedwater_controller().last_output(),
            loop_.mass_flow().get::<kilogram_per_second>()
        );
        assert!(
            settled_error.abs() < 2.0,
            "pure feedback left a {settled_error:+.3} K standing error"
        );
        assert!(
            reversals < 40,
            "feed flow reversed {reversals} times -- the loop is hunting"
        );

        // Load change: the case a feedforward would have caught instantly.
        for _ in 0..12000 {
            loop_.step(
                dt(),
                design_commands(),
                Power::new::<megawatt>(6.0),
                nominal_hot_side(),
            );
        }
        let after_load_change = loop_.feedwater_controller().error();
        println!(
            "after a 10 -> 6 MW load change: error {after_load_change:+.3} K, flow {:.4} kg/s",
            loop_.mass_flow().get::<kilogram_per_second>()
        );
        assert!(
            after_load_change.abs() < 5.0,
            "the loop did not recover from a load change: {after_load_change:+.3} K"
        );
    }

    /// MANUAL must rebuild the controller, so returning to AUTO does not dump
    /// an integral accumulated against a setpoint nobody was controlling to.
    #[test]
    fn manual_rebuilds_the_controller() {
        let mut loop_ = SteamSecondaryLoop::new();
        for _ in 0..2000 {
            loop_.step(
                dt(),
                design_commands(),
                Power::new::<megawatt>(4.0),
                nominal_hot_side(),
            );
        }
        assert!(
            loop_.feedwater_controller().last_output().abs() > 0.0,
            "AUTO should have driven the controller somewhere to begin with"
        );

        let manual = SecondaryCommands {
            feedwater: FeedwaterCommand::Manual {
                mass_flow_demand: MassRate::new::<kilogram_per_second>(3.0),
            },
            ..SecondaryCommands::default()
        };
        loop_.step(
            dt(),
            manual,
            Power::new::<megawatt>(4.0),
            nominal_hot_side(),
        );
        assert_eq!(
            loop_.feedwater_controller().last_output(),
            0.0,
            "MANUAL must rebuild the controller with a cleared integrator"
        );
    }
}
