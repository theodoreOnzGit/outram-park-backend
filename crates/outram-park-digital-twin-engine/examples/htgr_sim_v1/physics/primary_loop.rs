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
use tampines::compressible::CoolPropFluid;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::SolidMaterial;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{
    AvailableEnergy, DynamicViscosity, Mass, MassDensity, MassRate, Power, Pressure,
    SpecificHeatCapacity, ThermalConductance, ThermodynamicTemperature, Time, Volume,
};
use uom::si::thermal_conductance::watt_per_kelvin;
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
use super::steam_generator::{
    NodalisedCounterFlowSteamGenerator, SteamGeneratorConfig, SteamGeneratorGeometry,
    SteamGeneratorState,
};

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
///
/// # This number changed on 2026-08-12, and why that is not a free parameter
/// # being nudged
///
/// It was **1.0e5 W/K** while the steam generator was an effectiveness-NTU lump
/// pinching against an *isothermal saturation sink*. That formulation saw a
/// permanent ~450 K driving difference, so it needed a large `UA` to be
/// pinch-limited at all. The exchanger is now
/// [`super::steam_generator::NodalisedCounterFlowSteamGenerator`], which
/// evaluates the driving difference **node by node at local temperatures** --
/// and a real counter-flow exchanger with a collapsing superheater pinch is far
/// more effective per unit `UA` than the old formula implied. Carried over
/// unchanged, 1.0e5 W/K over-cools the helium. **Measured 2026-08-12** by
/// running [`tests::steam_generator_has_no_node_by_node_temperature_cross`]
/// with the constant set back to 1.0e5:
///
/// | | 1.0e5 W/K (carried over) | 4.26e4 W/K (re-calibrated) | Published |
/// |---|---|---|---|
/// | Core outlet | 880.53 K = **607.4 degC** | 993.89 K = 720.7 degC | 700 degC |
/// | Core inlet | 432.51 K = **159.4 degC** | 546.05 K = 272.9 degC | 250 degC |
/// | Steam outlet | 710.19 K = 437.0 degC | 700.81 K = 427.7 degC | 440 degC |
/// | Hot-end driving difference | 135.84 K | 263.80 K | -- |
///
/// So the old `UA` puts the whole helium circuit **92.6 K below** the published
/// core outlet and 90.6 K below the published core inlet -- a visibly wrong
/// plant, and worse than the defect being fixed.
///
/// **4.26e4 W/K is a re-calibration against the corrected physics, and it is a
/// calibration, not a prediction.** It is the `Q/LMTD` that places 10 MW across
/// the published terminal states (973 K / 525 K helium against 313 K / 713 K
/// water) in counter-flow: `LMTD = (260 - 212)/ln(260/212) = 234.8 K`, so
/// `UA = 10e6/234.8 = 4.26e4 W/K`. Nothing about the resulting agreement with
/// the published operating point is evidence of anything -- it was put there.
///
/// **It was not then nudged to close the remaining gap, and there is one.** The
/// closed-form `UA` above is derived for a *continuous* counter-flow exchanger
/// with terminal states; the model is an 8-node discretisation reading
/// cell-centre temperatures, and it lands about 20 K high on both helium
/// terminals (720.7 degC against 700, 272.9 degC against 250, at a fixed
/// 3.19 kg/s feed). That residual is reported, not tuned out: a second round of
/// fitting would only make the agreement less informative than it already is.
/// See [`tests::steam_generator_has_no_node_by_node_temperature_cross`] for the
/// measured design point and
/// [`super::steam_generator::SteamGeneratorGeometry::htr10_illustrative`] for
/// the geometry, which is *not* fitted to this number.
pub const STEAM_GENERATOR_UA_W_PER_K: f64 = 4.26e4;

/// Fraction of the steam generator's total thermal resistance placed on the
/// **hot (helium) side** (**invented**), 0.75.
///
/// The nodalised exchanger needs the overall `UA` split into a helium-to-metal
/// conductance and a metal-to-water conductance, because the metal sits between
/// them and its thermal mass is the point. Gas-side resistance dominating a
/// gas-to-boiling-water exchanger is the standard qualitative picture -- boiling
/// and high-velocity superheated steam are both far better at moving heat into a
/// tube wall than helium is at moving it out of a shell -- but **0.75 is not a
/// computed number**. No Dittus-Boelter, Gnielinski or helical-coil correlation
/// is evaluated anywhere in this model.
///
/// What the split *does* control physically is where the metal sits between the
/// two streams, and hence how a duty step is filtered. It does not change the
/// series `UA`.
const STEAM_GENERATOR_HOT_SIDE_RESISTANCE_FRACTION: f64 = 0.75;

/// Number of axial nodes in the steam generator (**invented**), 8.
///
/// Enough to resolve *that* the economiser, evaporator and superheater zones
/// exist and roughly where the boiling front sits -- at the design point the
/// water side shows one superheating node, about four nodes pinned on the
/// 523.5 K saturation plateau and two economiser nodes -- but far coarser than
/// the 17 water + 17 helium nodes the published Chinese INET transient model
/// used (`docs/reactor-scoping/htr10-plant-data.md` section 6). Read the axial
/// profile as an arrangement, not a converged solution.
///
/// The cost is real: three coupled array solves per 0.0125 s of *simulated*
/// time, **measured ~21 ms of wall clock per 0.05 s of simulated time**
/// (2026-08-12, release, 8 nodes). That is about 2.4x faster than real time, so
/// the GUI keeps up -- and because the exchanger runs on its own clock, the cost
/// does not rise when the plant is stepped at 1 ms instead of 50 ms. It does
/// make any test that marches thousands of plant steps expensive, which is why
/// the tests below march 150-400 s of simulated time rather than the 400-800 s
/// their predecessors could afford.
const STEAM_GENERATOR_NODE_COUNT: usize = 8;

/// The fixed timestep the steam-generator arrays are advanced with \[s\],
/// 0.0125 s.
///
/// The exchanger accumulates whatever `dt` this loop hands it and advances in
/// whole steps of this size -- see [`super::steam_generator::SteamGeneratorConfig::substep`].
/// That matters because the GUI steps this plant at **1 ms**, not at the 0.05 s
/// the tests use, and 1 ms is *below* the arrays' stability window.
///
/// # This is measured, from both ends
///
/// **Above**, by stability. The helium array's cells are ~0.6 m long and the gas
/// moves at ~10 m/s, so at the full 0.05 s plant timestep the advective Courant
/// number is close to 1: measured 2026-08-12, the enthalpy field goes odd-even
/// (checkerboard) within four plant steps and clamps against the array's
/// enthalpy bounds. 0.025 s and 0.0125 s both run 4000 plant steps clean.
///
/// **Below**, by a *different* instability -- this is a window, not a
/// "smaller is safer" limit. At 0.001 s the water side begins resolving its own
/// acoustic transient and the IF97 `(p,h)` flash goes out of range and panics.
///
/// **Within the window, by accuracy.** The three arrays are coupled explicitly
/// (Lie-split): each one's lateral conductance is evaluated against its
/// neighbours' *previous* sub-timestep temperatures, and the two fluid arrays
/// treat that source differently from the implicit solid, so the heat the metal
/// gives up and the heat the water takes agree only to `O(dt)`. Measured on the
/// steady-state stream energy balance
/// (`super::steam_generator::tests::energy_balance_closes_across_the_exchanger`,
/// 200 s at the design point):
///
/// | Sub-timestep | `Q_hot` | Closure `(Q_hot - Q_cold)/Q_hot` |
/// |---|---|---|
/// | 0.025 s | 9.8111 MW | **+1.72%** |
/// | 0.0125 s | 9.6719 MW | **+0.34%** |
/// | 0.00625 s | 9.6718 MW | **+0.35%** |
///
/// So 0.0125 s is where both the duty and the closure have converged; halving it
/// again buys nothing and doubles the cost. 0.025 s is visibly *not* converged
/// -- its duty is 1.4% high -- which is why the cheaper option was rejected.
const STEAM_GENERATOR_SUBSTEP_S: f64 = 0.0125;

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

/// Floor on the water/steam flow presented to the steam generator's tube side
/// \[kg/s\] (**invented**), matching the secondary loop's own flow floor.
///
/// The tube side is an advection-driven array: at zero flow it has no inlet
/// boundary to carry the feedwater state in, no residence time, and degenerates
/// into an axial-conduction problem that this plant model has no use for. The
/// secondary loop already clamps its own feed flow to the same value, so in
/// normal operation this floor never binds -- it exists so that a caller passing
/// literal zero still gets a well-posed exchanger rather than a stalled one.
const MIN_SECONDARY_FLOW_THROUGH_SG_KG_PER_S: f64 = 0.3;

/// Build the HTR-10 steam generator's configuration.
///
/// Everything plant-specific is assembled **here**, in the caller, so that
/// [`super::steam_generator`] stays a general exchanger that knows nothing about
/// the HTR-10 (or, for `fhr_sim_v2`, about a molten salt). The overall `UA` is
/// split into a helium-to-metal and a metal-to-water conductance by
/// [`STEAM_GENERATOR_HOT_SIDE_RESISTANCE_FRACTION`]; the two in series reproduce
/// [`STEAM_GENERATOR_UA_W_PER_K`] exactly, which
/// [`tests::the_conductance_split_reproduces_the_series_ua`] checks.
///
/// The arrays are seeded on linear station profiles between the published
/// terminal states -- helium 700 degC in, steam at the published 440 degC out,
/// and a 320 K cold end near the feedwater state -- so the exchanger opens at
/// approximately its operating arrangement rather than isothermal or crossed.
fn steam_generator_config() -> SteamGeneratorConfig {
    let ua = STEAM_GENERATOR_UA_W_PER_K;
    let f = STEAM_GENERATOR_HOT_SIDE_RESISTANCE_FRACTION;
    SteamGeneratorConfig {
        geometry: SteamGeneratorGeometry::htr10_illustrative(),
        hot_fluid: CoolPropFluid::Helium,
        hot_pressure: design().primary_pressure,
        cold_pressure: design().main_steam_pressure,
        metal: SolidMaterial::SteelSS304L,
        hot_side_conductance: ThermalConductance::new::<watt_per_kelvin>(ua / f),
        cold_side_conductance: ThermalConductance::new::<watt_per_kelvin>(ua / (1.0 - f)),
        node_count: STEAM_GENERATOR_NODE_COUNT,
        substep: Time::new::<second>(STEAM_GENERATOR_SUBSTEP_S),
        initial_hot_end_temperature: ThermodynamicTemperature::new::<kelvin>(
            published_core_outlet_k(),
        ),
        initial_cold_end_temperature: ThermodynamicTemperature::new::<kelvin>(320.0),
        initial_cold_outlet_temperature: design().main_steam_temperature,
    }
}

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
    /// Most recently computed heat rate **leaving the helium** in the steam
    /// generator.
    ihx_duty: Power,
    /// Most recently computed heat rate **entering the water/steam** in the
    /// steam generator. Differs from [`Self::ihx_duty`] by the rate of change of
    /// energy stored in the tube metal.
    secondary_duty: Power,
    /// Helium-side steam-generator outlet temperature (feeds the core inlet).
    ihx_outlet_temperature: ThermodynamicTemperature,
    /// The nodalised counter-flow steam generator itself: helium shell side,
    /// steel tube metal, water/steam tube side.
    steam_generator: NodalisedCounterFlowSteamGenerator,
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
            secondary_duty: Power::new::<watt>(0.0),
            ihx_outlet_temperature: inlet,
            pressure_drop: Pressure::new::<pascal>(0.0),
            bed_pressure_drop: Pressure::new::<pascal>(0.0),
            circulator_power: Power::new::<watt>(0.0),
            steam_generator: NodalisedCounterFlowSteamGenerator::new(steam_generator_config())
                .expect("the HTR-10 steam-generator configuration must be constructible"),
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
    /// circulator's illustrative range. `feedwater_enthalpy` and
    /// `secondary_mass_flow` describe the water entering the steam generator's
    /// tube side.
    ///
    /// The step, in order:
    ///
    /// 1. Helium `c_p` and density are re-evaluated from the real EOS at the
    ///    current bulk mean temperature `(T_in + T_out)/2`.
    /// 2. Core energy balance: steady-state outlet `T_in + Q/(m_dot c_p)`,
    ///    with the displayed outlet relaxed toward it over
    ///    [`CORE_THERMAL_TIME_CONSTANT_S`].
    /// 3. **Steam generator, nodalised counter-flow.** The core-outlet helium
    ///    enters the shell side of
    ///    [`super::steam_generator::NodalisedCounterFlowSteamGenerator`], the
    ///    feedwater enters the tube side at the opposite end, and the exchanger
    ///    is advanced. The duty is *not* a formula evaluated here -- it is the
    ///    helium stream's own enthalpy drop across a resolved exchanger, and the
    ///    helium-side outlet temperature comes back with it.
    /// 4. The core inlet relaxes toward that helium-side outlet over
    ///    [`RETURN_TRANSPORT_TIME_CONSTANT_S`], closing the loop.
    /// 5. Loop pressure drop -- KTA over the bed plus the published non-bed
    ///    component sum -- and circulator hydraulic power at the current flow
    ///    and density.
    ///
    /// # What changed on 2026-08-12
    ///
    /// Step 3 used to be an effectiveness-NTU lump against the steam-side
    /// **saturation temperature**, treated as an isothermal sink:
    ///
    /// ```text
    /// Q = (1 - exp(-UA/(m_dot c_p))) * m_dot * c_p * (T_core_out - T_sat)
    /// ```
    ///
    /// That is right for an evaporator and wrong for a **once-through** unit,
    /// which superheats. As the steam superheats the real driving difference
    /// collapses; against a fixed 523 K sink with helium near 973 K it never
    /// did, so the duty was over-predicted and the steam ran far too hot. The
    /// nodalised exchanger evaluates the driving difference at local node
    /// temperatures instead. See that module's docs for the full account.
    pub fn step(
        &mut self,
        dt: Time,
        core_heat_to_helium: Power,
        flow_setpoint: MassRate,
        feedwater_enthalpy: AvailableEnergy,
        secondary_mass_flow: MassRate,
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

        // 3. Steam generator: advance the resolved counter-flow exchanger. The
        //    duty and the helium-side outlet both come OUT of it; neither is
        //    computed here. The secondary flow is floored so the tube side never
        //    stagnates -- at zero flow the water array has no advection and the
        //    exchanger degenerates into a conduction problem the plant model has
        //    no use for.
        let sg = self
            .steam_generator
            .advance_timestep(
                dt,
                self.core_outlet_temperature,
                self.mass_flow,
                feedwater_enthalpy,
                MassRate::new::<kilogram_per_second>(
                    secondary_mass_flow
                        .get::<kilogram_per_second>()
                        .max(MIN_SECONDARY_FLOW_THROUGH_SG_KG_PER_S),
                ),
            )
            .expect("the steam generator must advance");
        self.ihx_duty = sg.hot_side_duty;
        self.secondary_duty = sg.cold_side_duty;

        // 4. The core inlet relaxes toward the steam generator's helium-side
        //    outlet through the return transport lag.
        let t_ihx_out_k = sg.hot_outlet_temperature.get::<kelvin>();
        self.ihx_outlet_temperature = sg.hot_outlet_temperature;
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

    /// Heat rate **leaving the helium** in the steam generator on the most
    /// recent step -- the helium stream's own enthalpy drop across the resolved
    /// exchanger, `m_dot (h_in - h_out)`.
    pub fn ihx_duty(&self) -> Power {
        self.ihx_duty
    }

    /// Heat rate **entering the water/steam** in the steam generator on the most
    /// recent step, `m_dot (h_out - h_in)` on the tube side. This is what the
    /// secondary cycle absorbs.
    ///
    /// It is **not** equal to [`Self::ihx_duty`] during a transient: the
    /// difference is the rate of change of energy stored in the tube metal. That
    /// gap is the physics the metal exists to provide, not a bookkeeping error;
    /// at steady state it closes.
    pub fn steam_generator_duty_to_secondary(&self) -> Power {
        self.secondary_duty
    }

    /// The nodalised steam generator's most recent state -- per-node
    /// temperatures on all three streams, both stream duties, and both outlets.
    ///
    /// The node vectors are in **hot-side index order** (element 0 at the helium
    /// inlet), so `hot_node_temperatures[i]` and `cold_node_temperatures[i]`
    /// are at the same physical station and their difference is the local
    /// driving temperature difference.
    #[allow(dead_code)] // read by the V&V tests; snapshot candidate for the app layer
    pub fn steam_generator_state(&self) -> &SteamGeneratorState {
        self.steam_generator.state()
    }

    /// Tube-metal thermal time constant \[s\] at `temperature`, `C_metal/UA`.
    /// The lag a duty step is filtered through before it reaches the steam
    /// outlet.
    #[allow(dead_code)] // read by the V&V tests; snapshot candidate for the app layer
    pub fn steam_generator_metal_time_constant(
        &self,
        temperature: ThermodynamicTemperature,
    ) -> Time {
        self.steam_generator.metal_time_constant(temperature)
    }

    /// Series overall conductance `UA` \[W/K\] of the steam generator.
    #[allow(dead_code)] // read by the V&V tests; snapshot candidate for the app layer
    pub fn steam_generator_overall_conductance(&self) -> ThermalConductance {
        self.steam_generator.overall_conductance()
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
    use uom::si::available_energy::joule_per_kilogram;
    use uom::si::power::megawatt;

    fn nominal_loop() -> HeliumPrimaryLoop {
        HeliumPrimaryLoop::new(pebble_bed::nominal_helium_flow())
    }

    fn nominal_flow() -> MassRate {
        pebble_bed::nominal_helium_flow()
    }

    /// Feedwater specific enthalpy the steam generator's tube side is driven
    /// with in these tests: the secondary loop's own settled feedwater state at
    /// the published 4.0 MPa, condensate at the 7 kPa condenser plus real pump
    /// work. Measured 168.73 kJ/kg (2026-08-12,
    /// `secondary_loop::tests::feedwater_enthalpy_is_condensate_plus_real_pump_work`).
    fn feedwater() -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(168.73e3)
    }

    /// Secondary mass flow the tests drive the tube side with: the settled feed
    /// flow at the plant's nominal 10 MW duty, 3.19 kg/s (measured in
    /// `secondary_loop`), against the published 12.5 t/hr = 3.47 kg/s.
    fn secondary_flow() -> MassRate {
        MassRate::new::<kilogram_per_second>(3.19)
    }

    /// March the loop to a settled state at `power`, returning it.
    ///
    /// 200 s of simulated time at 0.05 s. That is more than ten times the
    /// steam generator's ~38 s metal time constant and forty times the 5 s core
    /// gas lag, so nothing here is still moving materially. Deliberately shorter
    /// than the 400 s the pre-2026-08-12 tests used, because each plant step now
    /// advances three coupled fluid/solid arrays (~10.8 ms of wall clock per
    /// step) rather than evaluating a closed-form effectiveness.
    fn settled(power: Power) -> HeliumPrimaryLoop {
        let mut loop_ = nominal_loop();
        for _ in 0..4000 {
            loop_.step(
                Time::new::<second>(0.05),
                power,
                nominal_flow(),
                feedwater(),
                secondary_flow(),
            );
        }
        loop_
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

    /// V&V: **the steam generator has no temperature cross at any node**, and
    /// the helium side stays inside its own terminal states.
    ///
    /// # Why this test exists, and why it could not have existed before
    ///
    /// The steam generator used to be an effectiveness-NTU lump against an
    /// isothermal saturation sink. A lump has **one** temperature per side, so
    /// the only cross it could be asked about was a terminal one -- and its
    /// predecessor test asked exactly that, on the helium side only. Nothing
    /// constrained the steam, because the steam had no temperature in that
    /// model, only a saturation temperature that never moved.
    ///
    /// The exchanger is now resolved, so "no temperature cross" can be asked the
    /// way it should be: **at every station, is the hot stream still hotter than
    /// the cold stream it is heating?** That is a strictly stronger question
    /// than the terminal one -- a counter-flow exchanger can satisfy both outlet
    /// inequalities and still cross somewhere in the middle.
    ///
    /// # Methodology
    ///
    /// The loop is marched 4000 steps of 0.05 s (200 s of simulated time) at
    /// 10 MWth and the published 4.3 kg/s, with the tube side fed at the
    /// secondary's settled feedwater state (168.73 kJ/kg, 3.19 kg/s). At **every
    /// step** three things are asserted:
    ///
    /// 1. `SteamGeneratorState::worst_node_cross_kelvin() == 0` -- no station
    ///    anywhere has `T_cold,i > T_hot,i`;
    /// 2. the helium-side outlet lies between the tube-side inlet temperature
    ///    and the helium inlet, so the shell stream cannot leave hotter than it
    ///    arrived nor colder than the water it is heating;
    /// 3. every node temperature on all three streams is finite.
    ///
    /// **Nothing in the model clamps any of this.** The lateral heat term is
    /// `q_i = UA_i (T_up,i - T_down,i)` at local node temperatures; if the cold
    /// stream ever overtook the hot stream the term would simply change sign.
    /// The test measures a property, it does not police one.
    ///
    /// # Results (measured 2026-08-12)
    ///
    /// Zero crosses at every one of the 4000 steps; the worst value of
    /// `worst_node_cross_kelvin` over the whole run was **0.000000 K**. The
    /// settled design point, at a *fixed* 3.19 kg/s feed (the plant's own
    /// controller-driven design point is in
    /// `super::super::secondary_loop::tests::the_absorbable_duty_cap_no_longer_binds`):
    ///
    /// | Quantity | Measured | Published | Delta |
    /// |---|---|---|---|
    /// | Core outlet (SG helium inlet) | 993.89 K = **720.7 degC** | 700 degC | **+20.7 K** |
    /// | Core inlet (SG helium outlet) | 546.05 K = **272.9 degC** | 250 degC | **+22.9 K** |
    /// | SG duty, helium side | **9.9938 MW** | 10 MW | -0.06% |
    /// | SG duty, water side | **9.9223 MW** | 10 MW | -0.78% |
    /// | Steam outlet | 700.81 K = **427.7 degC** | 440 degC | **-12.3 K** |
    /// | Hot-end driving difference | **263.80 K** | -- | -- |
    ///
    /// Axial profile at that point (hot-inlet first, kelvin):
    ///
    /// ```text
    /// helium [964.60, 883.32, 809.74, 748.85, 702.85, 656.95, 620.22, 546.21]
    /// metal  [765.50, 613.38, 595.06, 579.80, 568.37, 520.72, 491.74, 390.66]
    /// water  [700.81, 523.50, 523.58, 523.52, 523.58, 475.80, 448.48, 339.00]
    /// ```
    ///
    /// The water row is the whole point: four nodes pinned on the 523.5 K
    /// saturation plateau with an economiser below and a superheater above.
    ///
    /// The **hot-end driving difference is the number this change is about**.
    /// The isothermal-sink model saw `T_helium - T_sat` = 993.89 - 523.5 =
    /// **470.4 K** there, and it never collapsed however hot the steam got. The
    /// resolved exchanger sees `T_helium - T_steam` = **263.80 K**, because the
    /// steam has superheated to 700.81 K by the time it reaches that end. The
    /// old model was over-predicting the driving difference at the hot end by
    /// **78%**.
    ///
    /// The ~21 K the helium terminals sit above published is the residual of the
    /// `UA` calibration against an 8-node discretisation; see
    /// [`STEAM_GENERATOR_UA_W_PER_K`]. It was **not** tuned out.
    ///
    /// # Interpretation
    ///
    /// The no-cross property is **structural**, so this test is a regression
    /// guard on the coupling wiring (in particular the counter-flow index map --
    /// getting it backwards would produce crosses immediately), not evidence
    /// that the exchanger is well-sized. The `UA` that sets the temperature
    /// *level* is a calibration; see [`STEAM_GENERATOR_UA_W_PER_K`].
    #[test]
    fn steam_generator_has_no_node_by_node_temperature_cross() {
        let mut loop_ = nominal_loop();
        let mut worst_cross = 0.0_f64;
        let mut worst_hot_end_dt = f64::INFINITY;

        for _ in 0..4000 {
            loop_.step(
                Time::new::<second>(0.05),
                Power::new::<megawatt>(10.0),
                nominal_flow(),
                feedwater(),
                secondary_flow(),
            );
            let sg = loop_.steam_generator_state();
            let cross = sg.worst_node_cross_kelvin();
            worst_cross = worst_cross.max(cross);
            worst_hot_end_dt = worst_hot_end_dt.min(sg.hot_end_driving_difference_kelvin());

            assert!(
                cross <= 1e-6,
                "temperature cross of {cross} K inside the steam generator: \
                 hot {:?} vs cold {:?}",
                sg.hot_node_temperatures
                    .iter()
                    .map(|t| t.get::<kelvin>())
                    .collect::<Vec<_>>(),
                sg.cold_node_temperatures
                    .iter()
                    .map(|t| t.get::<kelvin>())
                    .collect::<Vec<_>>()
            );

            let t_hot_in = loop_.core_outlet_temperature().get::<kelvin>();
            let t_hot_out = loop_.ihx_outlet_temperature().get::<kelvin>();
            assert!(
                t_hot_out <= t_hot_in + 1e-6,
                "the steam generator heated the helium: {t_hot_out} K out of {t_hot_in} K in"
            );
            for t in sg
                .hot_node_temperatures
                .iter()
                .chain(sg.metal_node_temperatures.iter())
                .chain(sg.cold_node_temperatures.iter())
            {
                assert!(
                    t.get::<kelvin>().is_finite(),
                    "a node temperature went non-finite"
                );
            }
        }

        let sg = loop_.steam_generator_state();
        println!(
            "SETTLED DESIGN POINT (10 MWth, 4.3 kg/s helium, 3.19 kg/s feed):\n  \
             core outlet (SG helium in)  = {:.2} K ({:.1} degC), published 700 degC\n  \
             core inlet  (SG helium out) = {:.2} K ({:.1} degC), published 250 degC\n  \
             SG duty helium side         = {:.4} MW\n  \
             SG duty water side          = {:.4} MW\n  \
             steam outlet                = {:.2} K ({:.1} degC), published 440 degC\n  \
             hot-end driving difference  = {:.2} K\n  \
             worst node cross over run   = {:.6} K\n  \
             UA (series)                 = {:.4e} W/K\n  \
             metal time constant         = {:.2} s\n  \
             hot   nodes = {:?}\n  metal nodes = {:?}\n  cold  nodes = {:?}",
            loop_.core_outlet_temperature().get::<kelvin>(),
            loop_.core_outlet_temperature().get::<kelvin>() - 273.15,
            loop_.core_inlet_temperature().get::<kelvin>(),
            loop_.core_inlet_temperature().get::<kelvin>() - 273.15,
            loop_.ihx_duty().get::<watt>() / 1.0e6,
            loop_.steam_generator_duty_to_secondary().get::<watt>() / 1.0e6,
            sg.cold_outlet_temperature.get::<kelvin>(),
            sg.cold_outlet_temperature.get::<kelvin>() - 273.15,
            sg.hot_end_driving_difference_kelvin(),
            worst_cross,
            loop_
                .steam_generator_overall_conductance()
                .get::<watt_per_kelvin>(),
            loop_
                .steam_generator_metal_time_constant(ThermodynamicTemperature::new::<kelvin>(600.0))
                .get::<second>(),
            sg.hot_node_temperatures
                .iter()
                .map(|t| (t.get::<kelvin>() * 100.0).round() / 100.0)
                .collect::<Vec<_>>(),
            sg.metal_node_temperatures
                .iter()
                .map(|t| (t.get::<kelvin>() * 100.0).round() / 100.0)
                .collect::<Vec<_>>(),
            sg.cold_node_temperatures
                .iter()
                .map(|t| (t.get::<kelvin>() * 100.0).round() / 100.0)
                .collect::<Vec<_>>(),
        );

        assert!(
            worst_cross <= 1e-6,
            "worst node cross over the run was {worst_cross} K"
        );
        assert!(
            worst_hot_end_dt.is_finite() && worst_hot_end_dt > 0.0,
            "the hot end must keep a positive driving difference (worst {worst_hot_end_dt} K)"
        );
        assert!(
            loop_.core_outlet_temperature().get::<kelvin>()
                > loop_.core_inlet_temperature().get::<kelvin>(),
            "the loop must settle with a hot leg above its cold leg"
        );
    }

    /// The core inlet must be a computed loop variable, not a fixed constant:
    /// making the secondary less able to remove heat -- here by throttling the
    /// feedwater flow through the steam generator from 3.19 to 2.2 kg/s -- must
    /// raise the core inlet.
    ///
    /// This replaces the old sink-temperature form of the same check, which is
    /// no longer expressible: the secondary is no longer an isothermal sink with
    /// a temperature to raise, it is a resolved stream with a flow and an inlet
    /// enthalpy.
    ///
    /// # Why the throttle is mild and the window short
    ///
    /// This is an **open-loop** run: 10 MWth goes into the helium regardless,
    /// the protection system is not in the path, and the feedwater controller is
    /// bypassed. Reduce the heat removal and the loop simply heats without
    /// bound. Two ceilings then arrive before anything interesting does --
    /// `SolidMaterial::SteelSS304L` is tabulated only to **1000 K** and TUAS
    /// *panics* rather than extrapolating past it, and IF97 stops at 1073.15 K.
    /// Measured 2026-08-12: throttling to 1.0 kg/s for 100 s drives the tube
    /// metal through the steel limit and kills the run. 2.2 kg/s for 75 s shows
    /// the same directional response with the metal well inside range, and the
    /// test asserts that it stayed inside. See the module docs of
    /// [`super::steam_generator`] for the operating margin against that ceiling
    /// at the design point.
    #[test]
    fn core_inlet_responds_to_secondary_heat_removal() {
        let mut strong = nominal_loop();
        let mut weak = nominal_loop();
        let mut worst_metal_k = 0.0_f64;
        for _ in 0..1500 {
            let dt = Time::new::<second>(0.05);
            let q = Power::new::<megawatt>(10.0);
            strong.step(dt, q, nominal_flow(), feedwater(), secondary_flow());
            weak.step(
                dt,
                q,
                nominal_flow(),
                feedwater(),
                MassRate::new::<kilogram_per_second>(2.2),
            );
            for t in weak.steam_generator_state().metal_node_temperatures.iter() {
                worst_metal_k = worst_metal_k.max(t.get::<kelvin>());
            }
        }
        let strong_k = strong.core_inlet_temperature().get::<kelvin>();
        let weak_k = weak.core_inlet_temperature().get::<kelvin>();
        println!(
            "core inlet after 75 s: 3.19 kg/s feed -> {strong_k:.2} K, 2.2 kg/s feed -> \
             {weak_k:.2} K; peak tube-metal temperature on the throttled run \
             {worst_metal_k:.2} K (SteelSS304L is tabulated to 1000 K)"
        );
        assert!(
            weak_k > strong_k,
            "throttling the feedwater must raise the core inlet ({weak_k} K vs {strong_k} K)"
        );
        assert!(
            worst_metal_k < 1000.0,
            "the tube metal reached {worst_metal_k} K, outside SteelSS304L's tabulated range"
        );
    }

    /// Methodology: once the transient has settled, the core temperature rise
    /// must follow the energy balance `dT = Q/(m_dot c_p)` using the *live*
    /// helium `c_p`. Pass criterion: within 2% of that balance.
    ///
    /// This is the one part of the loop the steam-generator `UA` calibration
    /// cannot touch: the rise is set by the power and the flow, whatever
    /// temperature level the exchanger settles the loop at.
    ///
    /// Results (2026-08-12): the settled rise was **447.8439 K** against
    /// `10e6/(4.3 x c_p) = 447.9627 K` from the balance at the live `c_p` --
    /// **-0.027%**. The rise is unchanged in kind by the steam-generator rework,
    /// as it must be.
    #[test]
    fn core_temperature_rise_matches_the_energy_balance() {
        let loop_ = settled(Power::new::<megawatt>(10.0));

        let measured = loop_.core_outlet_temperature().get::<kelvin>()
            - loop_.core_inlet_temperature().get::<kelvin>();
        let expected = Power::new::<megawatt>(10.0).get::<watt>()
            / (loop_.mass_flow().get::<kilogram_per_second>()
                * loop_.specific_heat().get::<joule_per_kilogram_kelvin>());
        println!(
            "settled core rise = {measured:.4} K against the energy balance {expected:.4} K \
             ({:+.3}%)",
            100.0 * (measured - expected) / expected
        );
        assert!(
            (measured - expected).abs() / expected < 0.02,
            "core rise {measured} K departs from the energy balance {expected} K"
        );
    }

    /// The overall `UA` the two per-side conductances present in series must be
    /// exactly [`STEAM_GENERATOR_UA_W_PER_K`], whatever
    /// [`STEAM_GENERATOR_HOT_SIDE_RESISTANCE_FRACTION`] is set to.
    ///
    /// This is what makes the resistance split a *placement* of the metal
    /// between the two streams rather than a second, hidden sizing knob.
    #[test]
    fn the_conductance_split_reproduces_the_series_ua() {
        let loop_ = nominal_loop();
        let ua = loop_
            .steam_generator_overall_conductance()
            .get::<watt_per_kelvin>();
        assert!(
            (ua - STEAM_GENERATOR_UA_W_PER_K).abs() / STEAM_GENERATOR_UA_W_PER_K < 1e-12,
            "series UA {ua} W/K does not reproduce {STEAM_GENERATOR_UA_W_PER_K} W/K"
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

        let rated = settled(Power::new::<megawatt>(10.0));
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
                feedwater(),
                secondary_flow(),
            );
            fast.step(
                dt,
                q,
                MassRate::new::<kilogram_per_second>(6.0),
                feedwater(),
                secondary_flow(),
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
            feedwater(),
            secondary_flow(),
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
            feedwater(),
            secondary_flow(),
        );
        assert!(
            (stopped.mass_flow().get::<kilogram_per_second>() - MIN_HELIUM_FLOW_KG_PER_S).abs()
                < 1e-9
        );
    }
}
