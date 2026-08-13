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
//! - **Turbine expansion** is an isentropic `(p,s)` flash de-rated by an
//!   adiabatic efficiency, with the exhaust quality from the outlet `(p,h)`
//!   flash, and the cycle's net power nets off the feed-pump work.
//!
//! ## What is still illustrative
//!
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
//!   turbine and pump efficiencies, the flow limits, and the secondary
//!   inventory are **invented values, not design data** -- IAEA-TECDOC-1382 is
//!   a reactor-physics benchmark and carries no condenser or turbine detail.
//!   Replacing them with sourced figures is tracked as bead `op-szmi.6`.
//! - The published plant is a steam-supply unit feeding a turbine-generator for
//!   co-generation; the closed Rankine cycle with a condenser modelled here is
//!   a plausible balance-of-plant, not the HTR-10's own.
//!
//! This is a demonstration model, not a validated steam-cycle model.

use tampines::hem::HemSteamCv;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{
    AvailableEnergy, Mass, MassRate, Power, Pressure, SpecificHeatCapacity,
    ThermodynamicTemperature, Volume,
};
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::megapascal;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;
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

/// Target steam-generator outlet enthalpy the feedwater controller holds: the
/// IF97 enthalpy of steam at the **published** 4.0 MPa / 440 degC outlet
/// condition, evaluated live from `tampines-steam-tables` rather than carried
/// as a hardcoded 3.307e6 J/kg. Measured 3307.9 kJ/kg (2026-08-12).
fn target_steam_enthalpy() -> AvailableEnergy {
    use tampines_steam_tables::interfaces::functional_programming::pt_flash_eqm::h_tp_eqm_single_phase;
    let d = super::pebble_bed::design();
    h_tp_eqm_single_phase(d.main_steam_temperature, d.main_steam_pressure)
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

/// Condenser back-pressure \[MPa\], ~7 kPa (**invented**), consistent with the
/// cooling-water inlet temperature below.
const CONDENSER_PRESSURE_MPA: f64 = 0.007;

/// Feed-pump isentropic efficiency (**invented**), 0.75.
const FEED_PUMP_EFFICIENCY: f64 = 0.75;

/// Turbine adiabatic (isentropic) efficiency (**invented**), 0.85.
const TURBINE_EFFICIENCY: f64 = 0.85;

/// Feedwater-controller time constant \[s\] (**invented**), the first-order
/// lag on how fast the feed flow chases its target.
const FEED_CONTROL_TIME_CONSTANT_S: f64 = 10.0;

/// Minimum secondary mass flow \[kg/s\] (**invented**) -- a floor so the
/// enthalpy balance denominator and the residence time stay finite at zero
/// duty. About 9% of the published nominal flow.
const MIN_SECONDARY_FLOW_KG_PER_S: f64 = 0.3;

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

/// Secondary-side water/steam inventory \[kg\] (**invented**), used for the
/// residence time that drives the schematic's steam-line flow tracers.
const SECONDARY_INVENTORY_KG: f64 = 2.0e3;

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
    let p_cond = Pressure::new::<megapascal>(CONDENSER_PRESSURE_MPA);
    let v = Volume::new::<cubic_meter>(1.0);

    let inlet = HemSteamCv::new_from_ph(p_steam, target_steam_enthalpy(), v);
    let isentropic_outlet = HemSteamCv::new_from_ps(p_cond, inlet.get_specific_entropy(), v);
    let isentropic_drop = inlet.get_specific_enthalpy() - isentropic_outlet.get_specific_enthalpy();

    d.main_steam_mass_flow * isentropic_drop * TURBINE_EFFICIENCY
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
}

impl SteamSecondaryLoop {
    /// Construct the loop at its nominal operating point, with the condensate
    /// and feedwater states flashed from the real steam tables and the
    /// steam-generator outlet seeded at zero duty.
    pub fn new() -> Self {
        let steam_pressure = steam_pressure();
        let condenser_pressure = Pressure::new::<megapascal>(CONDENSER_PRESSURE_MPA);
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
    /// 1. **Feedwater controller.** The flow that would hold
    ///    [`target_steam_enthalpy`] at the current duty is
    ///    `Q/(h_target - h_feed)`; the actual flow relaxes toward it over
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
    pub fn step(
        &mut self,
        dt: uom::si::f64::Time,
        ihx_duty: Power,
        hot_side_inlet_temperature: ThermodynamicTemperature,
    ) {
        let q_w_uncapped = ihx_duty.get::<watt>().max(0.0);

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

        // 1. Feedwater controller: chase the flow that holds the target steam
        //    enthalpy at the current duty.
        let enthalpy_rise_target =
            (target_steam_enthalpy().get::<joule_per_kilogram>() - h_feed).max(1.0);
        // The controller chases the duty the primary is OFFERING, not the
        // capped duty: it is trying to raise flow until the steam side can
        // absorb it all. Using the capped value here would be circular (the cap
        // depends on flow) and would latch the controller at low flow.
        let target_flow = (q_w_uncapped / enthalpy_rise_target)
            .clamp(MIN_SECONDARY_FLOW_KG_PER_S, MAX_SECONDARY_FLOW_KG_PER_S);
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

    /// Water/steam inventory held in the secondary loop, used for the
    /// residence time driving the schematic's steam-line flow tracers.
    pub fn inventory(&self) -> Mass {
        Mass::new::<kilogram>(SECONDARY_INVENTORY_KG)
    }

    /// Turbine inlet temperature (= steam-generator outlet temperature).
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

    /// Hot-side (core outlet) temperature the steam generator sees in tests:
    /// the published HTR-10 core outlet of 700 degC (973.15 K).
    ///
    /// This is the temperature the second-law cap is taken against. Tests that
    /// exercise the loop at its design duty must supply a design hot side, or
    /// the cap would throttle them for reasons unrelated to what they check.
    fn nominal_hot_side() -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(973.15)
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
        loop_.step(dt(), Power::new::<megawatt>(10.0), nominal_hot_side());

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
            loop_.step(dt(), Power::new::<megawatt>(10.0), nominal_hot_side());
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
            low.step(dt(), Power::new::<megawatt>(3.0), nominal_hot_side());
            high.step(dt(), Power::new::<megawatt>(12.0), nominal_hot_side());
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
            loop_.step(dt(), Power::new::<megawatt>(10.0), nominal_hot_side());
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
            loop_.step(dt(), duty, nominal_hot_side());
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
                loop_.step(dt(), duty, nominal_hot_side());
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
            loop_.step(dt(), duty, nominal_hot_side());

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
                loop_.step(dt(), duty, nominal_hot_side());
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
            primary.step(
                dt(),
                power,
                pebble_bed::nominal_helium_flow(),
                feed_h,
                feed_flow,
            );
            secondary.step(
                dt(),
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
}
