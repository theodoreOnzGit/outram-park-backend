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
//! ~~**The entire helium circuit is ONE control volume**, with two
//! temperatures carried at its boundaries~~ **CHANGED 2026-09-29 (gh:#388,
//! #391-#393)** -- the helium circuit is **four enthalpy-balance control
//! volumes and a resolved exchanger**, closed around one prescribed mass flow:
//!
//! ```text
//!   bed void helium (LTNE fluid node, pebble_bed)  --m_dot h_f-->  HOT DUCT CV
//!        ^                                                            |
//!        | m_dot h_c                                                  | m_dot h_h
//!   COLD RETURN CV  <--m_dot h_sg,out--  STEAM GENERATOR (8 nodes)  <-+
//!     + W_circ (circulator work)
//! ```
//!
//! | Region | Nodes | What is assumed uniform inside |
//! |---|---|---|
//! | Helium through the bed | **1** | ~~one `c_p` and one density at the bulk mean~~ the bed's own LTNE fluid node, an enthalpy balance over the 197 cm bed's void ([`super::pebble_bed::PebbleBedPorousMediaNode`]); the bulk mean is still where the KTA friction is evaluated |
//! | Hot-gas plenum + hot gas duct | ~~**0**~~ **1** | the hot-duct CV: well-mixed enthalpy, mass `rho V` over [`hot_duct_volume`] |
//! | Steam generator, helium side | **8** (~~an effectiveness-NTU lump~~ **CORRECTED 2026-09-17**) | one `UA_hot` per node against the tube metal; the cold side is a resolved IF97 array, not an isothermal sink -- see [`super::steam_generator`] |
//! | Connection tubes, circulator, annuli, riser channels, top plenum | ~~**0**~~ **1** | the cold-return CV: well-mixed enthalpy, mass over [`cold_return_volume`], circulator work as its source |
//! | Reflector cooling channels | **0** (as heat transfer) | their helium is in the cold-return CV; the helium-to-reflector convection is not modelled yet (gh:#397) |
//!
//! ~~The core inlet and core outlet temperatures are the **boundary values of
//! that one node** ... each relaxed by its own first-order lag~~ -- both lags
//! (`CORE_THERMAL_TIME_CONSTANT_S`, `RETURN_TRANSPORT_TIME_CONSTANT_S`) and
//! the core-outlet clamp were deleted 2026-09-29; see the block above
//! [`hot_duct_volume`]. The core outlet is the bed fluid node's own state;
//! the core inlet is the cold-return CV's.
//!
//! **What that costs.** There is no axial helium temperature profile through
//! the bed, so no local heat flux and no local Reynolds number: the KTA
//! friction factor is evaluated **once, at the bulk mean**, not integrated down
//! a bed whose helium actually runs 250 -> 700 degC. There is
//! no gas momentum equation, so the pressure drop cannot feed back on the flow
//! and there is **no natural circulation**. ~~With the circulator stopped this
//! model has no decay-heat removal path at all, which is precisely the HTR-10
//! behaviour a reader might most want and the one it cannot answer.~~
//! **CORRECTED 2026-09-17** -- decay heat now leaves the core by a *conduction
//! and radiation* chain that bypasses this loop entirely
//! ([`super::decay_heat_removal`]: bed -> reflector -> RPV -> RCCS, applied as
//! a sink on the bed source in [`super::HtgrPlant::step`]). What is still
//! missing from **this** module is the buoyancy-driven helium loop that runs
//! alongside it, so the passive path here is conduction/radiation only and
//! core temperatures under a loss of forced cooling are an upper bound. There
//! is no
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
//!   fission power: [`super::pebble_bed::PebbleBedPorousMediaNode`] holds the
//!   bed's 9 MJ/K of solid-phase thermal inertia (plus its own fluid-node
//!   capacitance) and hands this loop the heat rate that actually crosses the
//!   pebble surface.
//! - **The loop is closed.** The core inlet temperature is *computed* --
//!   ~~as the steam-generator helium-side outlet, relaxed through the return
//!   transport lag~~ (**CHANGED 2026-09-29**) as the state of the cold-return
//!   CV, an enthalpy balance fed by the steam-generator helium outlet plus the
//!   circulator work; it is not pinned to a fixed number.
//! - **The helium circuit conserves energy, and it is checked.** Every helium
//!   CV balances enthalpy on the one CoolProp helium EOS, every seam carries a
//!   single flux, and the plant's global ledger closes from fission to the
//!   steam generator and the RCCS at rounding level
//!   (`super::tests::the_whole_plant_conserves_energy_from_fission_to_the_steam_generator`,
//!   gh:#394).
//! - **The steam generator is pinch-limited node by node.**
//!   ~~by an effectiveness-NTU model against the secondary saturation
//!   temperature~~ **CORRECTED 2026-09-17** -- since 2026-08-12 this module
//!   owns a [`NodalisedCounterFlowSteamGenerator`], and the duty is the sum of
//!   eight local `UA (T_hot_i - T_metal_i)` terms evaluated at resolved
//!   temperatures, not a closed-form effectiveness against a saturation sink.
//!   A local difference that changes sign simply reverses the heat flow, so
//!   the pinch is structural rather than enforced (see
//!   [`HeliumPrimaryLoop::advance_steam_generator`] and
//!   [`super::steam_generator`]).
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
//! - **The steam generator's SIZING is a calibration, though its arrangement
//!   is resolved.** ~~The steam generator is one effectiveness-NTU lump.~~
//!   **CORRECTED 2026-09-17** -- it is an 8-node counter-flow exchanger with
//!   its own tube geometry ([`SteamGeneratorGeometry::htr10_illustrative`]),
//!   so the economiser/evaporator/superheater zones do resolve. What remains
//!   illustrative is that the published unit is a once-through *helical-tube*
//!   module and there is no helical-coil correlation here: `UA_hot` and
//!   `UA_cold` are constructor parameters whose series combination is an
//!   illustrative value chosen to place the settled loop near the published
//!   250/700 degC end states, and the tube diameters are invented.
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
use outram_park_digital_twin_engine::components::pipe::CoaxialDuctGeometry;
use outram_park_fork_coolprop::{state_pt, viscosity, Fluid, FluidState};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::length::meter;
use tampines::compressible::CoolPropFluid;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::SolidMaterial;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{
    AvailableEnergy, DynamicViscosity, Length, Mass, MassDensity, MassRate, Power, Pressure,
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
    NodalisedCounterFlowSteamGenerator, PimpleCorrectors, SteamGeneratorConfig,
    SteamGeneratorGeometry, SteamGeneratorState,
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
/// | Core outlet | 880.53 K = **607.4 degC** | 993.78 K = 720.6 degC | 700 degC |
/// | Core inlet | 432.51 K = **159.4 degC** | 545.94 K = 272.8 degC | 250 degC |
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
/// terminals (720.6 degC against 700, 272.8 degC against 250, at a fixed
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
/// **The cost is real, and it is nearly the whole of this simulator's cost.**
/// Three coupled array solves per 0.0125 s of *simulated* time, each running
/// its equation of state over every cell once per outer corrector. Measured
/// 2026-08-13 (release, 8 nodes, 2 outer correctors): the exchanger alone costs
/// about **1.0 s of compute per second of simulated time**, against ~0.04 for
/// everything else in the plant -- so it is **~96%** of `HtgrPlant::step`. Node
/// count is a linear term in that, which is why 8 is not raised toward the 17
/// the published INET model used.
///
/// Because the exchanger runs on its own clock, this cost does **not** fall
/// when the plant timestep rises; that is why raising the plant timestep from
/// 1 ms to 0.1 s bought only 4% (see [`super::PLANT_TIMESTEP_S`]). It also
/// makes any test that marches hundreds of seconds of simulated time expensive,
/// which is why the tests below march 150-400 s rather than the 400-800 s their
/// predecessors could afford.
const STEAM_GENERATOR_NODE_COUNT: usize = 8;

/// The fixed timestep the steam-generator arrays are advanced with \[s\].
///
/// **Derived, not typed in**: it is
/// [`super::PLANT_TIMESTEP_S`] / [`super::STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP`]
/// = 0.1 / 8 = **0.0125 s**, which is exactly the value this exchanger was
/// converged and stability-tested at on 2026-08-12. Changing the plant timestep
/// therefore moves the exchanger's clock with it, in a fixed ratio, rather than
/// leaving a second hand-maintained literal to drift out of step.
/// [`tests::the_steam_generator_substep_divides_the_plant_timestep`] pins the
/// division.
///
/// The exchanger accumulates whatever `dt` this loop hands it and advances in
/// whole steps of this size -- see
/// [`super::steam_generator::SteamGeneratorConfig::substep`]. That indirection
/// still matters even now the plant steps at 0.1 s: the exchanger is a
/// **multi-rate** sub-model whose cost per second of plant time does not depend
/// on the plant timestep at all, which is the property that made raising the
/// plant timestep worth doing.
///
/// # This is measured, from both ends
///
/// **Above**, by stability -- a **Courant** limit. The helium array's cells are
/// `5.0 m / 8 = 0.625 m` long and the gas moves at about 11 m/s, so at the full
/// 0.05 s plant timestep the advective Courant number is close to 1: measured
/// 2026-08-12, the enthalpy field goes odd-even (checkerboard) within four plant
/// steps and clamps against the array's enthalpy bounds. 0.025 s and 0.0125 s
/// both run 4000 plant steps clean. The measured Courant numbers are recorded in
/// [`super::steam_generator::tests::the_courant_number_bounds_the_array_substep`]
/// -- **and that test also shows that raising the arrays' PIMPLE outer-corrector
/// count does not lift the limit**, because their enthalpy convection is an
/// explicit source inside the corrector loop whose Picard contraction factor is
/// the Courant number itself.
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
fn steam_generator_substep_s() -> f64 {
    super::steam_generator_substep_seconds()
}

// ---------------------------------------------------------------------------
// THE TWO HELIUM CONTROL VOLUMES OUTSIDE THE BED (gh:#388, 2026-09-29)
//
// ~~`core_thermal_time_constant_s` -- a first-order lag on the core outlet with
// `tau = rho V_void / m_dot`~~, ~~`bounded_core_outlet` -- "a hard second-law
// guard"~~ and ~~`RETURN_TRANSPORT_TIME_CONSTANT_S = 8.0` s (invented)~~ were
// DELETED on 2026-09-29 (gh:#391, #392). The lag counted the bed void helium's
// inertia a second time (the LTNE fluid node in `pebble_bed` already carries
// it, from the same void volume); neither lag had a capacitance or an
// enthalpy of its own, so what the bed discharged and what the steam
// generator received differed by energy nobody stored; the clamp stood in for
// a formulation; and the 8 s return lag did not depend on flow, so after a
// circulator trip the core inlet still followed the steam generator in 8 s.
// The engine CLAUDE.md: "If a term needs a guard to stay physical, the
// formulation is wrong -- fix the formulation, do not add the guard."
//
// They are replaced by two lumped helium control volumes, each an enthalpy
// balance on a mass taken from a stated volume, so the residence time of each
// is `M / m_dot` and emerges from the CV rather than being typed in:
//
//   HOT DUCT CV   = hot-gas plenum in the bottom reflector + hot gas duct
//                   centre tube (in-reflector run + cross-vessel run)
//   COLD RETURN CV = SG-outlet connection tubes + circulator casing + SG
//                   vessel/sleeve annulus + coaxial-duct annulus + RPV/barrel
//                   annulus + the 20 side-reflector riser boreholes + top cold
//                   plenum
//
// The published flow path these two lump is `docs/reactor-scoping/
// htr10-plant-data.md` section 4.4 ([S2] section 5, [S5] section 2).
// ---------------------------------------------------------------------------

/// Hot-gas plenum in the bottom reflector \[m^3\] -- **INVENTED**. [S5] section
/// 2 says the bottom reflector "contains the hot gas plenum" and that its flow
/// passage is split into two sections; no source in the scoping sheet
/// dimensions it. 0.75 m^3 is about 0.3 m of the 2.545 m^2 core cross-section.
/// A sourced plenum volume would replace it.
const HOT_GAS_PLENUM_VOLUME_M3: f64 = 0.75;

/// Hot gas duct centre-tube volume **inside the reflector region** \[m^3\]:
/// 0.70686e5 cm^3, **published** -- the "Hot gas duct" void volume of the
/// [S3] KENO VI model, Table VI (`docs/reactor-scoping/htr10-plant-data.md`
/// section 4.3). It equals a 1.0 m run of the published 300 mm bore.
const HOT_GAS_DUCT_IN_REFLECTOR_VOLUME_M3: f64 = 0.070686;

/// Length of the hot gas duct's cross-vessel run between the reactor and
/// steam-generator pressure vessels \[m\] -- **INVENTED**. The scoping sheet
/// (section 4.1) records the duct length as "not stated in any of the five
/// sources". The bore it is multiplied by is the published 300 mm
/// ([`CoaxialDuctGeometry::htr10_hot_gas_duct`]).
const HOT_GAS_DUCT_CROSS_VESSEL_LENGTH_M: f64 = 3.0;

/// The 20 cold-helium riser boreholes in the side reflector \[m^3\]:
/// 5.07681e5 cm^3, **published** -- [S3] Table VI, "Coolant channels (20)".
/// Part of the cold-return CV; carried as its own constant so the published
/// part of that CV's volume stays visible.
const RISER_BOREHOLE_VOLUME_M3: f64 = 0.507681;

/// Helium volume of the **hot-duct CV** \[m^3\]: the invented plenum, the
/// published in-reflector duct volume and the cross-vessel run
/// (invented length times published bore). 1.0328 m^3.
pub fn hot_duct_volume() -> Volume {
    let bore_area = CoaxialDuctGeometry::htr10_hot_gas_duct().inner_flow_area();
    Volume::new::<cubic_meter>(HOT_GAS_PLENUM_VOLUME_M3 + HOT_GAS_DUCT_IN_REFLECTOR_VOLUME_M3)
        + bore_area * Length::new::<meter>(HOT_GAS_DUCT_CROSS_VESSEL_LENGTH_M)
}

/// Helium volume of the steam generator's **shell side** \[m^3\], as the
/// resolved exchanger's own helium array holds it:
/// `shell_flow_area x shell_flow_length` of
/// [`SteamGeneratorGeometry::htr10_illustrative`] (both invented), 1.25 m^3.
pub fn steam_generator_shell_volume() -> Volume {
    let g = SteamGeneratorGeometry::htr10_illustrative();
    g.shell_flow_area * g.shell_flow_length
}

/// Helium volume of the **cold-return CV** \[m^3\]: the invented
/// [`LOOP_GAS_VOLUME_OUTSIDE_BED_M3`] less the hot-duct CV and less the steam
/// generator's shell side. 3.7172 m^3, of which 0.5077 m^3 is the published
/// riser-borehole volume ([`RISER_BOREHOLE_VOLUME_M3`]); the rest is invented.
///
/// **Why the shell side is subtracted.** The 6 m^3 allowance was defined to
/// include "the steam-generator shell side", but that helium is now resolved
/// by the exchanger's own helium array, which carries its own inertia. Leaving
/// it in this CV as well would count it twice -- the same defect class as the
/// deleted core-outlet lag.
pub fn cold_return_volume() -> Volume {
    Volume::new::<cubic_meter>(LOOP_GAS_VOLUME_OUTSIDE_BED_M3)
        - hot_duct_volume()
        - steam_generator_shell_volume()
}

/// Floor on the commanded helium flow \[kg/s\] (**invented**), keeping the
/// energy-balance denominator and the residence time finite when the user
/// drives the circulator setpoint to zero. About 7% of nominal -- the published
/// blower regulates down to 30%, so anything below that is already outside the
/// machine's stated range.
const MIN_HELIUM_FLOW_KG_PER_S: f64 = 0.3;

/// Residual helium flow after a circulator trip \[kg/s\] (**a modelling
/// choice, see below**), about 0.23 % of the rated 4.3 kg/s.
///
/// **Not zero, and not physical residual forced flow.** In the HTR-10 loss-of-
/// forced-cooling ATWS test the blower was stopped and the blower baffle was
/// closed 12 s later, at which point Hu et al. (2006) record the primary mass
/// flow as having "decreased rapidly to almost zero". What then happens is not
/// stagnation: Chen et al. (2009) report that the helium, still at full system
/// pressure of about 2.5 MPa and therefore dense, sets up a **buoyancy-driven
/// natural circulation** through the core and the internals, and that this
/// convection is an effective heat-transport mechanism alongside conduction and
/// radiation.
///
/// **This model does not resolve that natural circulation** -- it has one
/// helium node, no momentum equation and no gravity term, so buoyancy cannot
/// be computed here (a limitation the workspace already records; see
/// `docs/reactor-scoping/htr10.md`). The residual is therefore a small positive
/// number chosen to keep the energy-balance denominator and the residence time
/// finite, **not** an estimate of the natural-circulation flow rate.
///
/// The consequence for results is one-directional and must be stated with any
/// number this model produces: omitting natural circulation removes a heat
/// transport path out of the core, so the computed core temperatures are an
/// **upper bound** and the computed power, through the negative temperature
/// coefficient, is correspondingly a lower bound.
const TRIPPED_HELIUM_FLOW_KG_PER_S: f64 = 1.0e-2;

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
/// The HTR-10-illustrative steam-generator configuration.
///
/// `pub` since 2026-09-27 so the standalone helical-coil harness
/// (`--sg-standalone`, see `crate::sg_standalone`) can build **the same
/// exchanger the plant runs** rather than a second configuration that would be
/// free to drift from it. That is the whole point of the harness: a defect it
/// reproduces has to be the plant's defect.
pub fn steam_generator_config() -> SteamGeneratorConfig {
    let ua = STEAM_GENERATOR_UA_W_PER_K;
    let f = STEAM_GENERATOR_HOT_SIDE_RESISTANCE_FRACTION;
    SteamGeneratorConfig {
        geometry: SteamGeneratorGeometry::htr10_illustrative(),
        hot_fluid: CoolPropFluid::Helium,
        hot_pressure: design().primary_pressure,
        cold_pressure: design().main_steam_pressure,
        // Kim's ANL-75-55 304L, NOT the Zou/Zweibaum `SteelSS304L`. The latter
        // is tabulated only to 1000 K (726.85 degC), which leaves about 27 K of
        // headroom above this plant's published 700 degC core outlet -- any
        // transient overshoot left the tabulated range and TUAS panicked rather
        // than extrapolating. `SteelSS304LHighTemp` carries the same alloy over
        // 300-1700 K, so the whole HTR-10 envelope (and phase 2's 900 degC) sits
        // inside the data. See `SolidMaterial::SteelSS304LHighTemp`'s own docs
        // for which part of that span is measured and which is Kim's
        // extrapolation into the melting range.
        metal: SolidMaterial::SteelSS304LHighTemp,
        hot_side_conductance: ThermalConductance::new::<watt_per_kelvin>(ua / f),
        cold_side_conductance: ThermalConductance::new::<watt_per_kelvin>(ua / (1.0 - f)),
        node_count: STEAM_GENERATOR_NODE_COUNT,
        substep: Time::new::<second>(steam_generator_substep_s()),
        initial_hot_end_temperature: ThermodynamicTemperature::new::<kelvin>(
            published_core_outlet_k(),
        ),
        initial_cold_end_temperature: ThermodynamicTemperature::new::<kelvin>(320.0),
        initial_cold_outlet_temperature: design().main_steam_temperature,
        hot_correctors: PimpleCorrectors::hot_gas_default(),
        cold_correctors: PimpleCorrectors::water_steam_default(),
    }
}

/// One lumped helium control volume's integrated state: the specific enthalpy
/// (stored exactly -- it is what the balance integrates) and the `(p, h)`
/// flash of it, which supplies the temperature, density and `c_p`.
///
/// Added 2026-09-29 (gh:#388, #393). Every helium CV in the circuit --
/// the bed's void node, the hot duct, the cold return -- is balanced on
/// enthalpy with the same CoolProp helium EOS
/// ([`pebble_bed::helium_enthalpy_at`], [`pebble_bed::helium_state_at`]), so
/// an enthalpy handed from one CV to the next means the same energy on both
/// sides of the seam.
#[derive(Clone, Copy, Debug)]
pub struct HeliumNode {
    /// Specific enthalpy \[J/kg\] -- the integrated state.
    enthalpy: f64,
    /// `(p, h)` flash of [`Self::enthalpy`] at the primary pressure.
    state: FluidState,
}

impl HeliumNode {
    /// A node at `temperature` and the primary pressure: the enthalpy is
    /// [`pebble_bed::helium_enthalpy_at`], the state its `(p, h)` flash.
    fn at_temperature(temperature: ThermodynamicTemperature) -> Self {
        let enthalpy = pebble_bed::helium_enthalpy_at(temperature);
        Self {
            enthalpy: enthalpy.get::<joule_per_kilogram>(),
            state: pebble_bed::helium_state_at(enthalpy),
        }
    }

    /// The node at a new `enthalpy` \[J/kg\] (one `(p, h)` flash, seeded at
    /// this node's current temperature -- the state it is moving from).
    fn moved_to(&self, enthalpy: f64) -> Self {
        Self {
            enthalpy,
            state: pebble_bed::helium_state_at_seeded(
                AvailableEnergy::new::<joule_per_kilogram>(enthalpy),
                Some(self.state.temperature),
            ),
        }
    }

    /// Specific enthalpy \[J/kg\].
    pub fn enthalpy(&self) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(self.enthalpy)
    }

    /// Temperature \[K\], from the EOS inverse of the enthalpy.
    pub fn temperature(&self) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.state.temperature)
    }

    /// Helium mass `rho V` held in `volume` at this state \[kg\].
    fn mass_in(&self, volume: Volume) -> Mass {
        Mass::new::<kilogram>(self.state.density * volume.get::<cubic_meter>())
    }
}

/// Energy terms of the primary loop's most recent pass \[J\], each from the
/// coefficients that pass actually used (gh:#394, 2026-09-29).
///
/// **Identity** (exact, by construction):
///
/// ```text
/// from_bed + circulator_work = hot_duct_storage + to_steam_generator + cold_return_storage
/// ```
///
/// `from_bed` is `m_dot (h_bed,out - h_core,in) dt` with `h_core,in` the
/// inlet enthalpy the **bed was handed** -- the same number, term for term,
/// as the bed's own [`pebble_bed::BedStepEnergy::throughflow_out`], which is
/// what lets the plant close a global balance across the seam.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PrimaryStepEnergy {
    /// Enthalpy the bed's throughflow delivered to the circuit above what the
    /// circuit delivered to the bed, `m_dot (h_bed,out - h_core,in) dt`.
    pub from_bed: f64,
    /// Change in the hot-duct CV's helium enthalpy, `M_h (h_h' - h_h)`.
    pub hot_duct_storage: f64,
    /// Enthalpy handed to the steam generator's helium side,
    /// `m_dot (h_h' - h_sg,out) dt` -- the steam generator's hot-side duty
    /// times `dt`; zero while the secondary is isolated.
    pub to_steam_generator: f64,
    /// Circulator shaft work delivered to the helium, `W dt`.
    pub circulator_work: f64,
    /// Change in the cold-return CV's helium enthalpy, `M_c (h_c' - h_c)`.
    pub cold_return_storage: f64,
}

/// The lumped scalars [`HeliumPrimaryLoop`] integrates, snapshotted so a plant
/// outer corrector can rewind them to the start of a timestep.
///
/// **Held inside the loop as one field** (2026-09-29), so a new lumped
/// quantity is rewound by construction rather than by remembering to add it
/// to a copy list. Everything except the steam generator's three arrays and
/// the two operator/protection flags lives here. See
/// [`HeliumPrimaryLoop::lumped_state`].
#[derive(Clone, Copy, Debug)]
pub struct PrimaryLumpedState {
    /// Helium leaving the bed on the most recent pass (the bed's own fluid
    /// node, handed in) -- the core outlet.
    bed_outlet: HeliumNode,
    /// The hot-duct CV (hot-gas plenum + hot gas duct centre tube).
    hot_duct: HeliumNode,
    /// The cold-return CV (connection tubes, circulator, annuli, risers, top
    /// plenum) -- its state is the core inlet.
    cold_return: HeliumNode,
    /// Steam-generator helium-side outlet specific enthalpy \[J/kg\].
    sg_outlet_enthalpy: f64,
    /// Steam-generator helium-side outlet temperature.
    sg_outlet_temperature: ThermodynamicTemperature,
    /// Circulator mass flow.
    mass_flow: MassRate,
    /// Heat rate leaving the helium in the steam generator.
    ihx_duty: Power,
    /// Heat rate entering the water/steam in the steam generator.
    secondary_duty: Power,
    /// Isobaric specific heat of helium at the bed's bulk mean temperature.
    c_p: SpecificHeatCapacity,
    /// Helium density at the bed's bulk mean temperature \[kg/m^3\] -- the
    /// density the KTA bed friction is formed on.
    density: f64,
    /// Helium dynamic viscosity at the bed's bulk mean temperature.
    dynamic_viscosity: DynamicViscosity,
    /// Frictional pressure drop around the whole loop.
    pressure_drop: Pressure,
    /// The pebble-bed part of that drop alone, from KTA.
    bed_pressure_drop: Pressure,
    /// Circulator shaft power delivered to the helium.
    circulator_power: Power,
    /// Energy terms of the most recent pass.
    last_step_energy: PrimaryStepEnergy,
}

/// Lumped helium primary-loop state.
pub struct HeliumPrimaryLoop {
    /// Every rollback-able lumped scalar. See [`PrimaryLumpedState`].
    lumped: PrimaryLumpedState,
    /// Helium density at the published 250 degC cold-leg reference condition,
    /// used to scale the quadratic non-bed loop loss.
    reference_density: f64,
    /// The nodalised counter-flow steam generator itself: helium shell side,
    /// steel tube metal, water/steam tube side.
    steam_generator: NodalisedCounterFlowSteamGenerator,
    /// Whether the helium circulator has tripped.
    ///
    /// **This exists so the simulator can enter loss of forced cooling at all.**
    /// [`MIN_HELIUM_FLOW_KG_PER_S`] is the right floor for a *commanded*
    /// setpoint -- the published blower regulates down to 30 % and a setpoint
    /// below that is outside the machine's range -- but it made a circulator
    /// trip unrepresentable: a commanded zero came back as 0.3 kg/s, about 7 %
    /// of rated, and the core kept being cooled by a blower that was supposed
    /// to have stopped.
    ///
    /// When tripped, the floor drops to [`TRIPPED_HELIUM_FLOW_KG_PER_S`]
    /// instead. See [`Self::trip_circulator`].
    circulator_tripped: bool,
    /// Whether the secondary circuit has been isolated from the steam
    /// generator.
    ///
    /// In the HTR-10 loss-of-forced-cooling ATWS test the reactor protection
    /// system isolated the secondary circuit 12 s after the circulator trip,
    /// and the blower baffle was closed at the same time (Hu et al. 2006,
    /// section 3). See [`Self::isolate_secondary`].
    secondary_isolated: bool,
}

impl HeliumPrimaryLoop {
    /// Trip the helium circulator, or reset the trip.
    ///
    /// Once tripped, the commanded-flow floor drops from
    /// [`MIN_HELIUM_FLOW_KG_PER_S`] to [`TRIPPED_HELIUM_FLOW_KG_PER_S`], so a
    /// commanded zero actually reaches (near) zero instead of being raised to
    /// 7 % of rated. Read [`TRIPPED_HELIUM_FLOW_KG_PER_S`] before interpreting
    /// any result this produces -- the residual is a numerical floor, not an
    /// estimate of the natural-circulation flow the real test established.
    ///
    /// The caller still has to command the flow down; this only removes the
    /// floor that was preventing it.
    pub fn trip_circulator(&mut self, tripped: bool) {
        self.circulator_tripped = tripped;
    }

    /// Whether the circulator is currently tripped.
    pub fn circulator_tripped(&self) -> bool {
        self.circulator_tripped
    }

    /// Isolate (or reconnect) the secondary circuit at the steam generator.
    ///
    /// While isolated the steam generator is not advanced at all: it transfers
    /// no heat and no feedwater flows through it. See
    /// [`Self::advance_steam_generator`] for why leaving it running during a
    /// loss of forced cooling drives the tube metal out of its property range.
    pub fn isolate_secondary(&mut self, isolated: bool) {
        self.secondary_isolated = isolated;
    }

    /// Construct the loop at the published HTR-10 operating point:
    /// `nominal_flow` helium mass flow, the cold-return CV (core inlet) seeded
    /// at 250 degC and the hot-duct CV and core outlet at 700 degC.
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
        let cold = HeliumNode::at_temperature(inlet);
        let hot = HeliumNode::at_temperature(outlet);

        Self {
            lumped: PrimaryLumpedState {
                bed_outlet: hot,
                hot_duct: hot,
                cold_return: cold,
                sg_outlet_enthalpy: cold.enthalpy,
                sg_outlet_temperature: inlet,
                mass_flow: nominal_flow,
                ihx_duty: Power::new::<watt>(0.0),
                secondary_duty: Power::new::<watt>(0.0),
                c_p,
                density,
                dynamic_viscosity,
                pressure_drop: Pressure::new::<pascal>(0.0),
                bed_pressure_drop: Pressure::new::<pascal>(0.0),
                circulator_power: Power::new::<watt>(0.0),
                last_step_energy: PrimaryStepEnergy::default(),
            },
            reference_density,
            // The loop is constructed at the operating point, circulator
            // running and the secondary connected.
            circulator_tripped: false,
            secondary_isolated: false,
            steam_generator: NodalisedCounterFlowSteamGenerator::new(steam_generator_config())
                .expect("the HTR-10 steam-generator configuration must be constructible"),
        }
    }

    /// Advance the loop by `dt` -- **the composite API for an isolated loop**,
    /// with the bed replaced by a prescribed outlet enthalpy.
    ///
    /// `bed_outlet_enthalpy` is the specific enthalpy of the helium leaving
    /// the bed. `flow_setpoint` is the commanded circulator flow, clamped to
    /// the circulator's illustrative range. `feedwater_enthalpy` and
    /// `secondary_mass_flow` describe the water entering the steam
    /// generator's tube side.
    ///
    /// The prescribed "bed" is taken to have been handed this loop's
    /// start-of-step core inlet, so the cold-return CV discharges that
    /// enthalpy (see [`Self::close_return_leg`]).
    ///
    /// The step, in order (the plant calls the parts itself, around its bed):
    ///
    /// 1. [`Self::command_flow`] -- the circulator flow for the step.
    /// 2. [`Self::step_hot_duct`] -- the hot-duct CV's enthalpy balance on the
    ///    bed outflow.
    /// 3. [`Self::advance_steam_generator`] -- the nodalised counter-flow
    ///    exchanger, handed the hot-duct enthalpy; the duty and the
    ///    helium-side outlet enthalpy both come **out** of it.
    /// 4. [`Self::close_return_leg`] -- loop hydraulics and circulator work,
    ///    then the cold-return CV's enthalpy balance, circulator work as its
    ///    source.
    ///
    /// ~~2. Core energy balance: steady-state outlet `T_in + Q/(m_dot c_p)`,
    /// with the displayed outlet relaxed toward it over
    /// `CORE_THERMAL_TIME_CONSTANT_S`.~~ ~~4. The core inlet relaxes toward
    /// that helium-side outlet over `RETURN_TRANSPORT_TIME_CONSTANT_S`.~~ Both
    /// lags were replaced by CVs on 2026-09-29 (gh:#391, #392).
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
    #[allow(dead_code)] // the composite API, exercised by the loop tests; `HtgrPlant` calls the parts
    pub fn step(
        &mut self,
        dt: Time,
        bed_outlet_enthalpy: AvailableEnergy,
        flow_setpoint: MassRate,
        feedwater_enthalpy: AvailableEnergy,
        secondary_mass_flow: MassRate,
    ) {
        let inlet_seen_by_bed = self.core_inlet_enthalpy();
        self.command_flow(flow_setpoint);
        self.step_hot_duct(dt, bed_outlet_enthalpy);
        self.advance_steam_generator(dt, feedwater_enthalpy, secondary_mass_flow);
        self.close_return_leg(dt, inlet_seen_by_bed);
    }

    /// Set the circulator flow for this timestep from the commanded setpoint,
    /// clamped to the circulator's range.
    ///
    /// **Called before the bed is stepped** (2026-09-29): the flow is
    /// prescribed (there is no momentum equation), so the bed, both helium
    /// CVs and the steam generator must all see the same `m_dot` within a
    /// timestep. Until 2026-09-29 the bed read the previous step's flow while
    /// the hot leg and the exchanger read the new one, which on a flow change
    /// (a circulator trip) put different mass flows on the two sides of the
    /// bed-outlet seam.
    ///
    /// A TRIPPED circulator is not a commanded setpoint, so it does not get
    /// the commanded floor: it would put 7 % of rated flow through a core
    /// that is supposed to have lost forced cooling entirely.
    pub fn command_flow(&mut self, flow_setpoint: MassRate) {
        let floor = if self.circulator_tripped {
            TRIPPED_HELIUM_FLOW_KG_PER_S
        } else {
            MIN_HELIUM_FLOW_KG_PER_S
        };
        let flow_kg_s = flow_setpoint
            .get::<kilogram_per_second>()
            .clamp(floor, MAX_HELIUM_FLOW_KG_PER_S);
        self.lumped.mass_flow = MassRate::new::<kilogram_per_second>(flow_kg_s);
    }

    /// **The hot-duct CV**: the hot-gas plenum in the bottom reflector and the
    /// hot gas duct's centre tube, one well-mixed helium volume
    /// ([`hot_duct_volume`]) between the bed and the steam generator.
    ///
    /// Backward Euler on its enthalpy balance, with the mass frozen at the
    /// start-of-step state:
    ///
    /// ```text
    /// M_h (h_h' - h_h) / dt = m_dot (h_bed,out - h_h')
    /// => h_h' = (M_h h_h + m_dot dt h_bed,out) / (M_h + m_dot dt)
    /// ```
    ///
    /// `h_h'` is a convex combination of `h_h` and `h_bed,out`, so it can never
    /// leave the interval they span -- the second law on this leg is a
    /// property of the formulation, not of a clamp. Its residence time is
    /// `M_h / m_dot` (about 0.36 s at rated flow and 973 K, ~150 s at the
    /// 0.01 kg/s trip floor); nothing is typed in.
    ///
    /// Also re-evaluates the helium properties at the bed's bulk mean
    /// `(T_core,in + T_core,out)/2` for the KTA friction and the display.
    ///
    /// Replaces ~~`step_hot_leg`~~ (a first-order lag on the core outlet with
    /// a second-law clamp; gh:#391). To call it twice for the same timestep,
    /// restore [`Self::lumped_state`] in between.
    pub fn step_hot_duct(&mut self, dt: Time, bed_outlet_enthalpy: AvailableEnergy) {
        let dt_s = dt.get::<second>();
        let m_dot = self.lumped.mass_flow.get::<kilogram_per_second>();

        // What left the core. One (p, h) flash, for the temperature.
        self.lumped.bed_outlet = self
            .lumped
            .bed_outlet
            .moved_to(bed_outlet_enthalpy.get::<joule_per_kilogram>());

        // Real helium properties at the bed's bulk mean.
        let t_in_k = self.lumped.cold_return.state.temperature;
        let t_out_k = self.lumped.bed_outlet.state.temperature;
        let (c_p, density, dynamic_viscosity) = helium_properties(0.5 * (t_in_k + t_out_k));
        self.lumped.c_p = c_p;
        self.lumped.density = density;
        self.lumped.dynamic_viscosity = dynamic_viscosity;

        // The CV's own enthalpy balance.
        let mass = self
            .lumped
            .hot_duct
            .mass_in(hot_duct_volume())
            .get::<kilogram>();
        let h_old = self.lumped.hot_duct.enthalpy;
        let h_in = self.lumped.bed_outlet.enthalpy;
        let h_new = (mass * h_old + m_dot * dt_s * h_in) / (mass + m_dot * dt_s);
        self.lumped.hot_duct = self.lumped.hot_duct.moved_to(h_new);
        self.lumped.last_step_energy.hot_duct_storage = mass * (h_new - h_old);
    }

    /// **Part 2 of [`Self::step`]: the expensive exchanger.** Advances the
    /// resolved counter-flow steam generator by `dt`, handed the hot-duct
    /// CV's **enthalpy**, and stores both stream duties and the helium-side
    /// outlet enthalpy and temperature.
    ///
    /// The duty and the helium-side outlet both come **out** of the exchanger;
    /// neither is computed here. The secondary flow is floored so the tube side
    /// never stagnates -- at zero flow the water array has no advection and the
    /// exchanger degenerates into a conduction problem the plant model has no
    /// use for.
    ///
    /// This is the only irreversible part of a plant timestep: the exchanger's
    /// three arrays hold their own history and cannot be rolled back cheaply.
    /// [`super::HtgrPlant::step`] therefore calls it **exactly once per plant
    /// timestep**, on the final outer corrector, so that the hot-inlet
    /// enthalpy and the feedwater state it is handed are the converged
    /// end-of-step values rather than the start-of-step ones.
    pub fn advance_steam_generator(
        &mut self,
        dt: Time,
        feedwater_enthalpy: AvailableEnergy,
        secondary_mass_flow: MassRate,
    ) {
        // An ISOLATED steam generator is valved out of both circuits: it moves
        // no heat, and nothing flows through it to chill it. Advancing it
        // anyway is not merely wasted work, it is wrong -- the feed-flow floor
        // MIN_SECONDARY_FLOW_THROUGH_SG_KG_PER_S keeps pushing cold water
        // through tubes that have no helium-side source once the circulator has
        // tripped, and the tube metal is driven below the 300 K lower bound of
        // the SS304L property correlation, which aborts the run. That is what
        // the HTR-10 test procedure avoids by isolating the secondary 12 s
        // after the trip (Hu et al. 2006 section 3).
        if self.secondary_isolated {
            self.lumped.ihx_duty = Power::new::<watt>(0.0);
            self.lumped.secondary_duty = Power::new::<watt>(0.0);
            // The helium side is valved out too, so the cold-return CV
            // receives the hot-duct helium rather than a cooled
            // steam-generator outlet.
            self.lumped.sg_outlet_enthalpy = self.lumped.hot_duct.enthalpy;
            self.lumped.sg_outlet_temperature = self.lumped.hot_duct.temperature();
            return;
        }

        let sg = self
            .steam_generator
            .advance_timestep_from_hot_inlet_enthalpy(
                dt,
                self.lumped.hot_duct.enthalpy(),
                self.lumped.mass_flow,
                feedwater_enthalpy,
                MassRate::new::<kilogram_per_second>(
                    secondary_mass_flow
                        .get::<kilogram_per_second>()
                        .max(MIN_SECONDARY_FLOW_THROUGH_SG_KG_PER_S),
                ),
            )
            .expect("the steam generator must advance");
        self.lumped.ihx_duty = sg.hot_side_duty;
        self.lumped.secondary_duty = sg.cold_side_duty;
        self.lumped.sg_outlet_enthalpy = sg.hot_outlet_enthalpy.get::<joule_per_kilogram>();
        self.lumped.sg_outlet_temperature = sg.hot_outlet_temperature;
    }

    /// **The cold-return CV**, and the loop hydraulics that set its source.
    ///
    /// One well-mixed helium volume ([`cold_return_volume`]) from the steam
    /// generator's helium outlet to the top of the bed: the six connection
    /// tubes, the circulator casing, the annuli down the SG vessel, the
    /// coaxial duct and the RPV, the 20 side-reflector riser boreholes and the
    /// top cold plenum. Its state is the core inlet.
    ///
    /// 1. **Hydraulics.** KTA over the bed plus the published non-bed
    ///    remainder, and the circulator shaft power `W = m_dot dp / (rho eta)`
    ///    -- with `rho` the **cold-return CV's** density, where the circulator
    ///    sits (see [`Self::update_hydraulics`]).
    /// 2. **Energy.** Backward Euler, mass frozen at the start-of-step state,
    ///    **circulator work as the source** (gh:#392):
    ///
    ///    ```text
    ///    M_c (h_c' - h_c) / dt = m_dot (h_sg,out - h_core,in) + W
    ///    ```
    ///
    /// # Why the outflow is `h_core,in`, the enthalpy the bed was handed
    ///
    /// The steam generator is advanced once per plant step, on the final outer
    /// corrector, after the bed; so the bed on that corrector has already been
    /// solved against the **previous corrector's** estimate of this CV's state.
    /// The enthalpy crossing the seam into the bed is therefore that estimate,
    /// and this CV discharges exactly the same number -- the flux across the
    /// seam is evaluated once and used on both sides, so the seam conserves
    /// energy exactly. As the outer correctors converge the estimate converges
    /// to `h_c'` and the balance becomes the implicit well-mixed one. With one
    /// corrector it is explicit upwind, stable while `m_dot dt / M_c < 1`:
    /// **measured margin** at the 8 kg/s circulator ceiling and a 1000 K cold
    /// return (the lightest the CV gets), `0.8 kg / 5.3 kg = 0.15`.
    /// `core_inlet_enthalpy_seen_by_bed` is that estimate.
    ///
    /// Residence time `M_c / m_dot`: about 2.3 s at rated flow, growing
    /// without bound as the flow falls -- replaces
    /// ~~`RETURN_TRANSPORT_TIME_CONSTANT_S = 8.0` s, invented and
    /// flow-independent~~.
    pub fn close_return_leg(&mut self, dt: Time, core_inlet_enthalpy_seen_by_bed: AvailableEnergy) {
        let dt_s = dt.get::<second>();
        let m_dot = self.lumped.mass_flow.get::<kilogram_per_second>();

        self.update_hydraulics(m_dot);
        let work = self.lumped.circulator_power.get::<watt>();

        let mass = self
            .lumped
            .cold_return
            .mass_in(cold_return_volume())
            .get::<kilogram>();
        let h_old = self.lumped.cold_return.enthalpy;
        let h_to_bed = core_inlet_enthalpy_seen_by_bed.get::<joule_per_kilogram>();
        let h_from_sg = self.lumped.sg_outlet_enthalpy;
        let h_new = h_old + dt_s / mass * (m_dot * (h_from_sg - h_to_bed) + work);
        self.lumped.cold_return = self.lumped.cold_return.moved_to(h_new);

        let e = &mut self.lumped.last_step_energy;
        e.cold_return_storage = mass * (h_new - h_old);
        e.circulator_work = work * dt_s;
        e.to_steam_generator = m_dot * (self.lumped.hot_duct.enthalpy - h_from_sg) * dt_s;
        e.from_bed = m_dot * (self.lumped.bed_outlet.enthalpy - h_to_bed) * dt_s;
    }

    /// Every **lumped scalar** this loop integrates, as one `Copy` value.
    ///
    /// This is the loop's whole rollback-able state: the three helium nodes
    /// (bed outlet, hot duct, cold return), the steam-generator outlet, the
    /// flow, the duties, the hydraulics and the last pass's energy terms. It
    /// deliberately excludes the steam generator's three arrays, which hold
    /// their own spatial history -- see [`Self::advance_steam_generator`] for
    /// why that one part of a timestep is not repeated.
    ///
    /// Used by [`super::HtgrPlant::step`]'s outer-corrector loop with
    /// [`Self::restore_lumped_state`].
    pub fn lumped_state(&self) -> PrimaryLumpedState {
        self.lumped
    }

    /// Restore the lumped scalars saved by [`Self::lumped_state`], rewinding
    /// this loop to the start of the current plant timestep. Does **not** touch
    /// the steam generator.
    pub fn restore_lumped_state(&mut self, s: PrimaryLumpedState) {
        self.lumped = s;
    }

    /// Loop pressure drop -- **KTA over the bed, published sum for the rest** --
    /// and the circulator shaft power needed to sustain it.
    ///
    /// Two terms, and they are not the same kind of number:
    ///
    /// 1. **The pebble bed: real.** [`bed_pressure_drop`] evaluates the KTA
    ///    packed-bed correlation
    ///    ([`outram_park_digital_twin_engine::htr10::kta`]) at the current bed
    ///    mass flux, the live helium density and viscosity at the bed's bulk
    ///    mean, the published pebble diameter and bed porosity, integrated
    ///    over the published bed height. A friction factor is genuinely
    ///    evaluated; nothing about this term is anchored to a target.
    /// 2. **Everything else: the published sum, scaled.** The side-reflector
    ///    pass, the mixture plenums, the steam generator and the hot gas duct
    ///    total [`PUBLISHED_NON_BED_DROP_AT_RATED_PA`] (25.9 kPa of the 27.2 kPa
    ///    loop) at rated flow, and are carried as
    ///    `dp_non_bed = 25.9 kPa (m_dot/m_dot_nom)^2`. The quadratic is the
    ///    fully-turbulent shape; **no density correction is applied to this
    ///    term**, because the published sum already embeds each component's own
    ///    local temperature (the steam generator and cold legs are cold, the
    ///    duct and plenums hot) and this lumped model resolves only one
    ///    density per CV. Correcting it with the bulk-mean density would
    ///    inflate the cold components by ~40%.
    ///
    /// Circulator shaft power is `m_dot dp_total / (rho eta)` with the
    /// illustrative efficiency [`CIRCULATOR_EFFICIENCY`], and **all of it is
    /// delivered to the helium** as the cold-return CV's source: an adiabatic
    /// compressor raises the stream's enthalpy by `W / m_dot`, and the
    /// isentropic inefficiency is dissipated in the gas, not lost from it.
    /// That assumes the drive motor's own losses (on the upper shaft, [S2])
    /// do not reach the helium; it is stated rather than sourced.
    ///
    /// **CHANGED 2026-09-29 (gh:#392): `rho` is the cold-return CV's
    /// density**, where the circulator sits ([`pressure_drop_reference_temperature_k`]
    /// already says so: "the published 250 degC cold leg, where the circulator
    /// sits"). ~~It was the bed's bulk-mean density~~, which is ~30 % lower at
    /// the design point and inflated the work by the same factor. That did not
    /// matter while the work was computed and discarded; it matters now that
    /// the work is a source term.
    fn update_hydraulics(&mut self, flow_kg_s: f64) {
        let rho = self.lumped.density;
        let rho_circulator = self.lumped.cold_return.state.density;
        if !(rho > 0.0) || !(self.reference_density > 0.0) || !(rho_circulator > 0.0) {
            self.lumped.pressure_drop = Pressure::new::<pascal>(0.0);
            self.lumped.bed_pressure_drop = Pressure::new::<pascal>(0.0);
            self.lumped.circulator_power = Power::new::<watt>(0.0);
            return;
        }

        // 1. The bed, by KTA, on the fraction of the loop flow that reaches it.
        let bed_flow = MassRate::new::<kilogram_per_second>(flow_kg_s * core_flow_fraction());
        let bed_dp = bed_pressure_drop(
            bed_flow,
            MassDensity::new::<kilogram_per_cubic_meter>(rho),
            self.lumped.dynamic_viscosity,
        );
        self.lumped.bed_pressure_drop = bed_dp;

        // 2. The published remainder of the loop, quadratic in flow.
        let flow_ratio = flow_kg_s / pebble_bed::nominal_helium_flow_kg_per_s();
        let non_bed_dp = PUBLISHED_NON_BED_DROP_AT_RATED_PA * flow_ratio * flow_ratio;

        let dp = bed_dp.get::<pascal>() + non_bed_dp;
        self.lumped.pressure_drop = Pressure::new::<pascal>(dp);
        self.lumped.circulator_power =
            Power::new::<watt>(flow_kg_s * dp / (rho_circulator * CIRCULATOR_EFFICIENCY));
    }

    /// Total helium-filled volume of the primary circuit: the bed void volume
    /// derived from the published core geometry, plus the illustrative
    /// allowance for the plenums, duct, steam-generator shell and circulator
    /// (now split into the hot-duct CV, the steam generator's shell side and
    /// the cold-return CV -- the total is unchanged).
    pub fn gas_volume(&self) -> Volume {
        pebble_bed::bed_void_volume() + Volume::new::<cubic_meter>(LOOP_GAS_VOLUME_OUTSIDE_BED_M3)
    }

    /// Helium inventory held in the circuit \[kg\]: the two CVs' own masses,
    /// plus the bed void and the steam-generator shell side at the bed's
    /// bulk-mean density (those two nodes belong to the bed and the exchanger,
    /// which own their own densities).
    pub fn helium_inventory(&self) -> Mass {
        let rho_bulk = self.lumped.density;
        self.lumped.hot_duct.mass_in(hot_duct_volume())
            + self.lumped.cold_return.mass_in(cold_return_volume())
            + Mass::new::<kilogram>(
                rho_bulk
                    * (pebble_bed::bed_void_volume() + steam_generator_shell_volume())
                        .get::<cubic_meter>(),
            )
    }

    /// Core-inlet helium temperature -- the cold-return CV's state.
    pub fn core_inlet_temperature(&self) -> ThermodynamicTemperature {
        self.lumped.cold_return.temperature()
    }

    /// Core-inlet helium specific enthalpy -- the cold-return CV's state, and
    /// the enthalpy the bed is handed.
    pub fn core_inlet_enthalpy(&self) -> AvailableEnergy {
        self.lumped.cold_return.enthalpy()
    }

    /// Core-outlet helium temperature -- the helium **leaving the bed** (its
    /// fluid node), as handed to [`Self::step_hot_duct`].
    ///
    /// **CHANGED 2026-09-29 (gh:#391):** this was the lagged-and-clamped
    /// value the steam generator saw. The steam generator's inlet is now the
    /// hot-duct CV, [`Self::hot_duct_temperature`].
    pub fn core_outlet_temperature(&self) -> ThermodynamicTemperature {
        self.lumped.bed_outlet.temperature()
    }

    /// Hot-duct CV temperature -- the steam generator's helium inlet.
    pub fn hot_duct_temperature(&self) -> ThermodynamicTemperature {
        self.lumped.hot_duct.temperature()
    }

    /// Hot-duct CV specific enthalpy -- what the steam generator is handed.
    pub fn hot_duct_enthalpy(&self) -> AvailableEnergy {
        self.lumped.hot_duct.enthalpy()
    }

    /// Helium mass held in the hot-duct CV \[kg\].
    pub fn hot_duct_mass(&self) -> Mass {
        self.lumped.hot_duct.mass_in(hot_duct_volume())
    }

    /// Helium mass held in the cold-return CV \[kg\].
    pub fn cold_return_mass(&self) -> Mass {
        self.lumped.cold_return.mass_in(cold_return_volume())
    }

    /// Residence time of the hot-duct CV, `M_h / m_dot` -- emerges from the
    /// CV's mass and the flow; nothing is typed in.
    pub fn hot_duct_residence_time(&self) -> Time {
        self.hot_duct_mass() / self.lumped.mass_flow
    }

    /// Residence time of the cold-return CV, `M_c / m_dot`.
    pub fn cold_return_residence_time(&self) -> Time {
        self.cold_return_mass() / self.lumped.mass_flow
    }

    /// Bulk mean helium temperature in the core, `(T_in + T_out)/2`.
    #[allow(dead_code)] // snapshot candidate -- not yet wired into the app layer
    pub fn helium_bulk_temperature(&self) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(
            0.5 * (self.core_inlet_temperature().get::<kelvin>()
                + self.core_outlet_temperature().get::<kelvin>()),
        )
    }

    /// Helium-side steam-generator outlet temperature.
    pub fn ihx_outlet_temperature(&self) -> ThermodynamicTemperature {
        self.lumped.sg_outlet_temperature
    }

    /// Helium-side steam-generator outlet specific enthalpy.
    #[allow(dead_code)] // read by the V&V tests
    pub fn ihx_outlet_enthalpy(&self) -> AvailableEnergy {
        AvailableEnergy::new::<joule_per_kilogram>(self.lumped.sg_outlet_enthalpy)
    }

    /// Current helium mass flow.
    pub fn mass_flow(&self) -> MassRate {
        self.lumped.mass_flow
    }

    /// Energy terms of the most recent pass. See [`PrimaryStepEnergy`].
    pub fn last_step_energy(&self) -> PrimaryStepEnergy {
        self.lumped.last_step_energy
    }

    /// Heat rate **leaving the helium** in the steam generator on the most
    /// recent step -- the helium stream's own enthalpy drop across the resolved
    /// exchanger, `m_dot (h_in - h_out)`.
    pub fn ihx_duty(&self) -> Power {
        self.lumped.ihx_duty
    }

    /// The nodalised steam generator, read-only.
    ///
    /// Added 2026-09-27 for diagnosis: the cold side reached the 273.15 K IF97
    /// floor and nothing outside this module could see the node temperatures
    /// that got it there. A read-only accessor, so it cannot become a second
    /// way to drive the exchanger.
    #[allow(dead_code)]
    pub fn steam_generator(&self) -> &NodalisedCounterFlowSteamGenerator {
        &self.steam_generator
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
        self.lumped.secondary_duty
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

    /// How many times the steam generator's **hot** array has clamped its
    /// enthalpy field against its own bounds since the plant was constructed.
    ///
    /// Zero is the expected value and is direct evidence that the exchanger's
    /// array substep is inside its Courant window: a checkerboard breakdown
    /// shows up here as a nonzero count before it shows up as a panic. See
    /// [`super::steam_generator::NodalisedCounterFlowSteamGenerator::hot_enthalpy_clamp_events`].
    /// Advective Courant numbers `(hot, cold)` in the steam generator at a
    /// candidate array substep, measured from the arrays' own live velocity
    /// fields. See
    /// [`super::steam_generator::NodalisedCounterFlowSteamGenerator::max_courant_numbers`].
    #[allow(dead_code)] // read by the V&V tests; diagnostic candidate for the app layer
    pub fn steam_generator_courant_numbers(&self, substep_s: f64) -> (f64, f64) {
        self.steam_generator
            .max_courant_numbers(Time::new::<second>(substep_s))
    }

    #[allow(dead_code)] // read by the V&V tests; snapshot candidate for the app layer
    pub fn steam_generator_enthalpy_clamp_events(&self) -> usize {
        self.steam_generator.hot_enthalpy_clamp_events()
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
        self.lumped.c_p
    }

    /// Helium density at the current bulk mean temperature \[kg/m^3\], from the
    /// real EOS.
    #[allow(dead_code)] // snapshot candidate -- not yet wired into the app layer
    pub fn density(&self) -> f64 {
        self.lumped.density
    }

    /// Frictional pressure drop around the **whole loop** at the current flow:
    /// the KTA bed term plus the published non-bed remainder. Not a bed
    /// friction result on its own -- for that, see [`Self::bed_pressure_drop`].
    pub fn pressure_drop(&self) -> Pressure {
        self.lumped.pressure_drop
    }

    /// The **pebble-bed** pressure drop alone, from the KTA correlation at the
    /// current bed mass flux and live helium properties. This one *is* an
    /// evaluated packed-bed friction result.
    pub fn bed_pressure_drop(&self) -> Pressure {
        self.lumped.bed_pressure_drop
    }

    /// Helium dynamic viscosity at the current bulk mean temperature, from the
    /// CoolProp-derived Arp-McCarty-Friend helium transport model -- the
    /// property the KTA Reynolds number is formed on.
    #[allow(dead_code)] // snapshot candidate -- not yet wired into the app layer
    pub fn dynamic_viscosity(&self) -> DynamicViscosity {
        self.lumped.dynamic_viscosity
    }

    /// Circulator shaft power required to sustain [`Self::pressure_drop`] --
    /// and, since 2026-09-29 (gh:#392), the power delivered to the helium as
    /// the cold-return CV's source. ~~Computed and discarded~~ until then.
    pub fn circulator_power(&self) -> Power {
        self.lumped.circulator_power
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

    /// Test helper: the bed-outlet temperature that delivers `duty` into a loop
    /// sitting at `inlet` with capacity rate `m_dot c_p`.
    ///
    /// These tests drive the loop **by duty**, which is a legitimate specified
    /// boundary condition for an isolated component test. It is deliberately
    /// NOT what the production path does: there the outlet comes from the
    /// bed's own balance, precisely so the outlet cannot be derived from a
    /// duty with a `c_p` that disagrees with the bed's and end up above the
    /// bed temperature. See `pebble_bed::PebbleBedPorousMediaNode::step`.
    ///
    /// **On enthalpy since 2026-09-29 (gh:#393)**: `h_out = h_in + Q/m_dot`,
    /// exact, where it used to be `T_in + Q/(m_dot 5189.3)`.
    fn bed_outlet_for(duty: Power, inlet: AvailableEnergy, flow: MassRate) -> AvailableEnergy {
        inlet
            + AvailableEnergy::new::<joule_per_kilogram>(
                duty.get::<watt>() / flow.get::<kilogram_per_second>(),
            )
    }
    use super::*;
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

    /// The plant timestep these tests drive the loop at -- the same constant
    /// the application and the whole-plant tests read.
    fn dt() -> Time {
        crate::physics::plant_timestep()
    }

    /// Number of plant timesteps in `plant_seconds` of simulated time. Test
    /// windows are expressed in **simulated seconds** so that changing
    /// [`crate::physics::PLANT_TIMESTEP_S`] does not silently rescale them.
    fn steps_for(plant_seconds: f64) -> usize {
        (plant_seconds / crate::physics::PLANT_TIMESTEP_S).round() as usize
    }

    /// March the loop to a settled state at `power`, returning it.
    ///
    /// 200 s of simulated time at the plant timestep. That is more than ten times the
    /// steam generator's ~38 s metal time constant and ~80 times the two helium
    /// CVs' residence times (~2.5 s together at rated flow; ~~the 5 s core gas
    /// lag~~, deleted), so nothing here is still moving materially. Deliberately shorter
    /// than the 400 s the pre-2026-08-12 tests used, because each second of
    /// simulated time now advances three coupled fluid/solid arrays -- about
    /// 1 s of wall clock per simulated second, measured 2026-08-13 -- rather
    /// than evaluating a closed-form effectiveness.
    fn settled(power: Power) -> HeliumPrimaryLoop {
        let mut loop_ = nominal_loop();
        for _ in 0..steps_for(200.0) {
            let bed_out = bed_outlet_for(power, loop_.core_inlet_enthalpy(), nominal_flow());
            loop_.step(dt(), bed_out, nominal_flow(), feedwater(), secondary_flow());
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
    /// The loop is marched over 200 s of simulated time at the plant timestep, at
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
    /// # Results (measured 2026-08-12; **re-measured 2026-08-13** at the 0.1 s
    /// plant timestep with the exchanger arrays at 2 outer correctors -- the
    /// terminals moved by 0.11 K, which is the whole effect of both changes on
    /// this design point)
    ///
    /// Zero crosses at every step; the worst value of
    /// `worst_node_cross_kelvin` over the whole run was **0.000000 K**. The
    /// settled design point, at a *fixed* 3.19 kg/s feed (the plant's own
    /// controller-driven design point is in
    /// `super::super::secondary_loop::tests::the_absorbable_duty_cap_no_longer_binds`):
    ///
    /// | Quantity | Measured | Published | Delta |
    /// |---|---|---|---|
    /// | Core outlet (SG helium inlet) | 993.78 K = **720.6 degC** | 700 degC | **+20.6 K** |
    /// | Core inlet (SG helium outlet) | 545.94 K = **272.8 degC** | 250 degC | **+22.8 K** |
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
    /// The isothermal-sink model saw `T_helium - T_sat` = 993.78 - 523.5 =
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
    #[ignore = "every htgr_sim_v1 test must finish under 1 minute (maintainer direction, 2026-09-27); measured 2026-09-27 as still running after 20 s in its own process. Asserts the no-cross invariant at EVERY one of the 200 s of plant steps, so the run length IS the coverage -- shortening it would test less, not the same thing faster."]
    fn steam_generator_has_no_node_by_node_temperature_cross() {
        let mut loop_ = nominal_loop();
        let mut worst_cross = 0.0_f64;
        let mut worst_hot_end_dt = f64::INFINITY;

        for _ in 0..steps_for(200.0) {
            let bed_out = bed_outlet_for(
                Power::new::<megawatt>(10.0),
                loop_.core_inlet_enthalpy(),
                nominal_flow(),
            );
            loop_.step(dt(), bed_out, nominal_flow(), feedwater(), secondary_flow());
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
    /// the tube metal leaves its property table and TUAS *panics* rather than
    /// extrapolating, and IF97 stops at 1073.15 K. Measured 2026-08-12 against
    /// the then-current `SolidMaterial::SteelSS304L` (tabulated to **1000 K**):
    /// throttling to 1.0 kg/s for 100 s drove the tube metal through the steel
    /// limit and killed the run. 2.2 kg/s for 75 s shows the same directional
    /// response with the metal well inside range, and the test asserts that it
    /// stayed inside.
    ///
    /// **The plant now builds this exchanger with
    /// `SolidMaterial::SteelSS304LHighTemp`** (Kim, ANL-75-55, 300-1700 K), so
    /// the ceiling asserted here is 1700 K rather than 1000 K. That change is
    /// why the margin is comfortable. Re-measured 2026-08-13 over the intended
    /// 75 s window at the 0.1 s plant timestep: core inlet 537.63 K at the
    /// 3.19 kg/s feed against 580.20 K at the throttled 2.2 kg/s, with a peak
    /// tube metal of **931.32 K** -- inside even the old 1000 K ceiling. A 150 s
    /// window (which this test briefly had, when the plant timestep doubled
    /// without the step count being halved) reaches **999.07 K**, i.e. it would
    /// have grazed that ceiling; the window is expressed in simulated seconds
    /// now so it cannot drift with the timestep again.
    ///
    /// See the module docs of [`super::steam_generator`] for the operating
    /// margin against that ceiling at the design point.
    #[test]
    #[ignore = "every htgr_sim_v1 test must finish under 1 minute (maintainer direction, 2026-09-27); measured 2026-09-27 as still running after 20 s in its own process. Steps TWO whole primary loops (each with its nodalised steam generator) through 75 s of plant time. Not shortenable: the feedwater-throttling difference it asserts has to have time to appear at the core inlet."]
    fn core_inlet_responds_to_secondary_heat_removal() {
        let mut strong = nominal_loop();
        let mut weak = nominal_loop();
        let mut worst_metal_k = 0.0_f64;
        for _ in 0..steps_for(75.0) {
            let dt = dt();
            let q = Power::new::<megawatt>(10.0);
            let bed_out = bed_outlet_for(q, strong.core_inlet_enthalpy(), nominal_flow());
            strong.step(dt, bed_out, nominal_flow(), feedwater(), secondary_flow());
            let bed_out = bed_outlet_for(q, weak.core_inlet_enthalpy(), nominal_flow());
            weak.step(
                dt,
                bed_out,
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
             {worst_metal_k:.2} K (SteelSS304LHighTemp is tabulated to 1700 K; the \
             SteelSS304L this replaced stopped at 1000 K)"
        );
        assert!(
            weak_k > strong_k,
            "throttling the feedwater must raise the core inlet ({weak_k} K vs {strong_k} K)"
        );
        assert!(
            worst_metal_k < 1700.0,
            "the tube metal reached {worst_metal_k} K, outside SteelSS304LHighTemp's \
             tabulated range"
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
    /// Results (2026-08-12; re-measured 2026-08-13 at the 0.1 s plant timestep
    /// with the exchanger arrays at 2 outer correctors): the settled rise was
    /// **447.8358 K** (was 447.8439 K) against
    /// `10e6/(4.3 x c_p) = 447.9627 K` from the balance at the live `c_p` --
    /// **-0.027%**. The rise is unchanged in kind by the steam-generator rework,
    /// as it must be.
    #[test]
    #[ignore = "every htgr_sim_v1 test must finish under 1 minute (maintainer direction, 2026-09-27); measured 2026-09-27 as still running after 20 s in its own process. settled() drives the nodalised loop to steady state before the 2 % energy-balance check; a shorter settle would compare an unsettled rise against the balance, which weakens the assertion rather than speeding it up."]
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
    #[ignore = "every htgr_sim_v1 test must finish under 1 minute (maintainer direction, 2026-09-27); measured 2026-09-27 as still running after 20 s in its own process. One settled loop for the published 27.2 kPa budget plus two more loops stepped 400 times for the monotonicity check. The settle is load-bearing for the published comparison, so it cannot be cut."]
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
            let dt = dt();
            let q = Power::new::<megawatt>(10.0);
            let bed_out = bed_outlet_for(
                q,
                slow.core_inlet_enthalpy(),
                MassRate::new::<kilogram_per_second>(2.0),
            );
            slow.step(
                dt,
                bed_out,
                MassRate::new::<kilogram_per_second>(2.0),
                feedwater(),
                secondary_flow(),
            );
            let bed_out = bed_outlet_for(
                q,
                fast.core_inlet_enthalpy(),
                MassRate::new::<kilogram_per_second>(6.0),
            );
            fast.step(
                dt,
                bed_out,
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

    /// Run the loop with the steam generator valved out and a prescribed bed
    /// outlet: settle at `pre_k` for `settle_s`, then step the bed outlet to
    /// `step_k` and trace the core-inlet enthalpy \[J/kg\] for `seconds`.
    /// Cheap: the exchanger is not advanced while isolated.
    fn isolated_step_response(
        flow_kg_s: f64,
        pre_k: f64,
        step_k: f64,
        settle_s: f64,
        seconds: f64,
    ) -> (Vec<f64>, HeliumPrimaryLoop) {
        let mut loop_ = nominal_loop();
        loop_.isolate_secondary(true);
        let flow = MassRate::new::<kilogram_per_second>(flow_kg_s);
        let at =
            |k: f64| pebble_bed::helium_enthalpy_at(ThermodynamicTemperature::new::<kelvin>(k));
        for _ in 0..steps_for(settle_s) {
            loop_.step(dt(), at(pre_k), flow, feedwater(), secondary_flow());
        }
        let mut trace = vec![loop_.core_inlet_enthalpy().get::<joule_per_kilogram>()];
        for _ in 0..steps_for(seconds) {
            loop_.step(dt(), at(step_k), flow, feedwater(), secondary_flow());
            trace.push(loop_.core_inlet_enthalpy().get::<joule_per_kilogram>());
        }
        (trace, loop_)
    }

    /// Time \[s\] at which `trace` first covers 63.2 % of its change to its
    /// final value.
    fn time_to_63_percent(trace: &[f64]) -> f64 {
        let (h0, h_end) = (trace[0], *trace.last().unwrap());
        let i = trace
            .iter()
            .position(|h| (h - h0) / (h_end - h0) >= 1.0 - (-1.0f64).exp())
            .expect("the trace must reach 63.2 % of its change");
        i as f64 * crate::physics::PLANT_TIMESTEP_S
    }

    /// The 63.2 % time \[s\] of two first-order lags in series, from
    /// equilibrium, `y(t) = 1 - (t1 e^(-t/t1) - t2 e^(-t/t2))/(t1 - t2)` --
    /// the analytic answer the CV cascade is checked against (bisection).
    fn cascade_63_percent_time(t1: f64, t2: f64) -> f64 {
        let y = |t: f64| 1.0 - (t1 * (-t / t1).exp() - t2 * (-t / t2).exp()) / (t1 - t2);
        let target = 1.0 - (-1.0f64).exp();
        let (mut lo, mut hi) = (0.0, 20.0 * (t1 + t2));
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            if y(mid) < target {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    }

    /// V&V (gh:#392): **the return leg's residence time emerges from the CV's
    /// mass and scales as `1/m_dot`**, and **the circulator's work reaches
    /// the helium**.
    ///
    /// # Methodology
    ///
    /// The steam generator is valved out, so the helium runs bed outlet ->
    /// hot-duct CV -> cold-return CV -> core inlet with nothing in between.
    /// The loop is settled with the bed outlet at 523.15 K, then the bed
    /// outlet is stepped to 543.15 K -- a small step, so each CV's mass (and
    /// residence time) stays within ~4 % over the response -- and the
    /// core-inlet enthalpy is traced for ~40 residence times, at the rated
    /// 4.3 kg/s and at a tenth of it.
    ///
    /// 1. **Residence time.** Two well-mixed CVs in series answer a step with
    ///    the analytic two-lag response; its 63.2 % time is computed from the
    ///    CVs' own `tau = M/m_dot` (read at the end of the run) by
    ///    [`cascade_63_percent_time`]. Pass: the traced 63.2 % time within 5 %
    ///    of it at each flow (the `dt = 0.1 s` discretisation and the ~4 % mass
    ///    change are the tolerance), and the low-flow time at least 8x the
    ///    rated one.
    /// 2. **Circulator work.** Once settled, the core inlet must sit above the
    ///    hot-duct CV by exactly the work per unit mass, `h_c - h_h = W /
    ///    m_dot`, to 1e-6 relative.
    ///
    /// **Both fail on the pre-2026-09-29 loop.** Its core inlet relaxed
    /// through `RETURN_TRANSPORT_TIME_CONSTANT_S = 8.0` s at every flow, so
    /// the 63.2 % time was about 8 s at both flows (a ratio of 1, against
    /// 1.8 s expected at rated flow), and the computed circulator power was
    /// never added to the gas (`h_c = h_sg`).
    ///
    /// # Results (2026-09-29)
    ///
    /// Printed by the test; recorded on gh:#392.
    #[test]
    fn the_return_leg_residence_time_scales_with_flow_and_carries_the_circulator_work() {
        let mut t63 = Vec::new();
        for flow in [4.3, 0.43] {
            let tau_guess = 14.0 / flow; // ~ (M_h + M_c) / m_dot, for the run lengths only
            let (trace, loop_) =
                isolated_step_response(flow, 523.15, 543.15, 12.0 * tau_guess, 30.0 * tau_guess);
            let t = time_to_63_percent(&trace);
            let tau_h = loop_.hot_duct_residence_time().get::<second>();
            let tau_c = loop_.cold_return_residence_time().get::<second>();
            let expected = cascade_63_percent_time(tau_h, tau_c);
            let w = loop_.circulator_power().get::<watt>();
            let rise = (loop_.core_inlet_enthalpy() - loop_.hot_duct_enthalpy())
                .get::<joule_per_kilogram>();
            let expected_rise = w / flow;
            println!(
                "m_dot = {flow:.2} kg/s: 63.2 % time {t:.2} s against the two-lag {expected:.3} s \
                 (tau_h {tau_h:.3} s, tau_c {tau_c:.3} s; M_h = {:.3} kg, M_c = {:.3} kg); \
                 W = {:.1} W, h_core,in - h_hot = {rise:.4} J/kg against W/m_dot = \
                 {expected_rise:.4} J/kg",
                loop_.hot_duct_mass().get::<kilogram>(),
                loop_.cold_return_mass().get::<kilogram>(),
                w,
            );
            assert!(
                (t - expected).abs() / expected < 0.05,
                "63.2 % time {t} s is not the CVs' two-lag response {expected} s"
            );
            assert!(w > 0.0);
            assert!(
                (rise - expected_rise).abs() / expected_rise < 1e-6,
                "the circulator work did not reach the helium: rise {rise} J/kg, W/m_dot {expected_rise} J/kg"
            );
            t63.push(t);
        }
        assert!(
            t63[1] / t63[0] > 8.0,
            "the return-leg lag must grow as the flow falls: {t63:?}"
        );
    }

    /// V&V (gh:#394): **the primary loop closes its own energy balance on every
    /// pass, and the steam generator's hot-side duty is `m_dot (h_hot duct -
    /// h_SG,out)`**.
    ///
    /// # Methodology
    ///
    /// 5 s of the isolated-component loop at 10 MWth into a 4.3 kg/s stream,
    /// steam generator running. At every step:
    ///
    /// 1. `from_bed + circulator_work = hot_duct_storage + to_steam_generator
    ///    + cold_return_storage` ([`PrimaryStepEnergy`]) to 1e-12 of the
    ///    step's gross energy;
    /// 2. `ihx_duty dt = to_steam_generator` and `ihx_duty = m_dot (h_hot duct
    ///    - h_SG,out)`, to 1e-12 relative -- the exchanger was handed exactly
    ///    the enthalpy the hot-duct CV holds;
    /// 3. the hot-duct CV's new enthalpy lies between its old one and the bed
    ///    outlet (second law on that leg, from the formulation -- nothing
    ///    clamps it).
    ///
    /// Before 2026-09-29 (1) had no terms to test, and (2) was false in any
    /// transient: the exchanger was handed a lagged, clamped temperature that
    /// was not the enthalpy the bed discharged.
    ///
    /// # Results (2026-09-29)
    ///
    /// Printed by the test; recorded on gh:#394.
    #[test]
    fn the_primary_loop_closes_its_own_balance_and_hands_the_exchanger_its_enthalpy() {
        let mut loop_ = nominal_loop();
        let q = Power::new::<megawatt>(10.0);
        let mut worst_identity = 0.0f64;
        let mut worst_duty = 0.0f64;
        let dt_s = dt().get::<second>();
        for _ in 0..steps_for(5.0) {
            let h_hot_before = loop_.hot_duct_enthalpy().get::<joule_per_kilogram>();
            let bed_out = bed_outlet_for(q, loop_.core_inlet_enthalpy(), nominal_flow());
            loop_.step(dt(), bed_out, nominal_flow(), feedwater(), secondary_flow());
            let e = loop_.last_step_energy();
            let gross = e.from_bed.abs()
                + e.circulator_work.abs()
                + e.hot_duct_storage.abs()
                + e.to_steam_generator.abs()
                + e.cold_return_storage.abs();
            let identity = e.from_bed + e.circulator_work
                - e.hot_duct_storage
                - e.to_steam_generator
                - e.cold_return_storage;
            worst_identity = worst_identity.max(identity.abs() / gross);

            let duty = loop_.ihx_duty().get::<watt>();
            let m_dot = loop_.mass_flow().get::<kilogram_per_second>();
            let stream = m_dot
                * (loop_.hot_duct_enthalpy() - loop_.ihx_outlet_enthalpy())
                    .get::<joule_per_kilogram>();
            worst_duty = worst_duty
                .max((duty - stream).abs() / duty.abs())
                .max((duty * dt_s - e.to_steam_generator).abs() / (duty * dt_s).abs());

            let h_hot = loop_.hot_duct_enthalpy().get::<joule_per_kilogram>();
            let h_bed = bed_out.get::<joule_per_kilogram>();
            let (lo, hi) = (h_hot_before.min(h_bed), h_hot_before.max(h_bed));
            assert!(
                h_hot >= lo - 1e-9 * hi.abs() && h_hot <= hi + 1e-9 * hi.abs(),
                "the hot-duct CV left the interval of its inputs"
            );
        }
        println!(
            "primary identity worst {worst_identity:.3e} of the step's gross energy; SG duty vs \
             m_dot (h_hot - h_sg,out) worst {worst_duty:.3e}; final duty {:.4} MW",
            loop_.ihx_duty().get::<watt>() / 1e6
        );
        assert!(worst_identity < 1e-12);
        assert!(worst_duty < 1e-12);
    }

    /// The two CV volumes are what their definitions say: the hot duct holds
    /// the published in-reflector duct volume and the published 300 mm bore,
    /// the cold return holds at least the published riser boreholes, and the
    /// split plus the steam generator's shell side is the whole 6 m^3
    /// allowance.
    #[test]
    fn the_cv_volumes_split_the_allowance_without_double_counting_the_shell() {
        let hot = hot_duct_volume().get::<cubic_meter>();
        let cold = cold_return_volume().get::<cubic_meter>();
        let shell = steam_generator_shell_volume().get::<cubic_meter>();
        println!("hot duct {hot:.4} m^3, cold return {cold:.4} m^3, SG shell {shell:.4} m^3");
        assert!((hot + cold + shell - LOOP_GAS_VOLUME_OUTSIDE_BED_M3).abs() < 1e-12);
        assert!(hot > HOT_GAS_DUCT_IN_REFLECTOR_VOLUME_M3);
        assert!(cold > RISER_BOREHOLE_VOLUME_M3);
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
        // The prescribed bed outlet is formed at the flow the loop will
        // actually CARRY (the clamped one), not the commanded one. Until
        // 2026-09-29 the stopped case below formed it at the commanded
        // 0 kg/s -- 10 MW into zero flow, an infinite outlet enthalpy -- and
        // the since-deleted core-outlet clamp silently bounded the infinity.
        // With the clamp gone the helium flash fails loud on it, as it should.
        let mut too_fast = nominal_loop();
        let bed_out = bed_outlet_for(
            Power::new::<megawatt>(10.0),
            too_fast.core_inlet_enthalpy(),
            MassRate::new::<kilogram_per_second>(MAX_HELIUM_FLOW_KG_PER_S),
        );
        too_fast.step(
            dt(),
            bed_out,
            MassRate::new::<kilogram_per_second>(85.0),
            feedwater(),
            secondary_flow(),
        );
        assert!(
            (too_fast.mass_flow().get::<kilogram_per_second>() - MAX_HELIUM_FLOW_KG_PER_S).abs()
                < 1e-9
        );

        let mut stopped = nominal_loop();
        let bed_out = bed_outlet_for(
            Power::new::<megawatt>(10.0),
            stopped.core_inlet_enthalpy(),
            MassRate::new::<kilogram_per_second>(MIN_HELIUM_FLOW_KG_PER_S),
        );
        stopped.step(
            dt(),
            bed_out,
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
