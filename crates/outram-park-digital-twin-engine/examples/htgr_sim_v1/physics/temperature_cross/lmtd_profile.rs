//! LMTD steady profile -- remedy for a steam-generator temperature cross.
//!
//! Selected by [`super::TemperatureCrossRemedy::Lmtd`]. See that variant's
//! documentation for what this method is and when it is the right choice, and
//! `docs/heat-exchanger-temperature-cross-fallback.md` for the design
//! discussion behind all three remedies.
//!
//! # What this computes
//!
//! A **steady-state, thermodynamically admissible** temperature profile for the
//! whole exchanger, from the two inlet boundary conditions and the installed
//! conductance, and writes it over both fluid arrays and the tube metal. It
//! does not advance the transient; it replaces it. Anything computed while this
//! remedy is engaged is **not** a resolved transient.
//!
//! # Why the exchanger has to be zoned, and what "zoning" means here
//!
//! The log-mean temperature difference is the exact closed-form solution of the
//! steady counter-flow energy equations **only** under three assumptions:
//! constant specific heat on each side, constant overall coefficient `U`, and
//! no phase change. A once-through steam generator violates all three: its cold
//! side is feedwater at (here) 4 MPa taken through the IAPWS-IF97 saturation
//! line to superheated steam, so its `c_p` is not merely variable but
//! *undefined* across the boiling boundary, where enthalpy rises at constant
//! temperature.
//!
//! A single-zone LMTD across that boundary does not fail loudly; it returns a
//! plausible number that is wrong, which is worse than a crash. So this
//! implementation splits the exchanger into up to **three zones in cold-side
//! specific enthalpy** -- subcooled (economiser), two-phase (evaporator),
//! superheat -- and applies LMTD **within each zone**, with each zone's own
//! terminal temperature differences and its own cold-side capacity rate.
//!
//! ```text
//!   xi = 0  (hot inlet / steam outlet)                 xi = 1  (hot outlet / feedwater inlet)
//!   |<----- superheat ----->|<--- two-phase --->|<----- subcooled ----->|
//!   helium ------------------------------------------------------------> (cools)
//!   steam <------------------------------------------------------------- water (heats)
//!            h > h_g              h_f < h < h_g            h < h_f
//! ```
//!
//! ## Locating the boundaries is the crux
//!
//! The zone *boundaries* are not free parameters. In cold-side enthalpy they
//! are pinned exactly, at the IF97 saturated-liquid and saturated-vapour
//! enthalpies `h_f(p)` and `h_g(p)`; nothing is fitted or guessed there. What
//! is unknown is **where those enthalpies sit in space**, which follows from
//! the duty split, which follows from the total duty -- the one scalar this
//! method actually solves for.
//!
//! The solve is a conductance balance. For a trial total duty `Q`:
//!
//! 1. the cold outlet enthalpy is `h_out = h_in + Q/m_cold`, so each zone's
//!    duty is fixed by the IF97 saturation enthalpies it spans;
//! 2. the hot-side temperature at every zone boundary follows by marching the
//!    hot energy balance from the hot inlet, `T_hot = T_hot,in - q/C_hot`;
//! 3. each zone's LMTD is then known from its two terminal temperature
//!    differences, so the conductance it *demands* is `UA_zone = Q_zone/LMTD`;
//! 4. `Q` is correct when the demanded conductances sum to the conductance the
//!    exchanger actually has, `sum(UA_zone) = UA_total`.
//!
//! Step 4's residual is monotone increasing in `Q` (more duty means larger zone
//! duties and smaller terminal differences, both of which raise the demand), so
//! a bisection on `Q` is unconditionally convergent. A `Q` that would drive any
//! terminal difference to zero or negative is *infeasible* -- it demands
//! infinite conductance -- and is treated as lying above the root, which is how
//! the internal pinch at the evaporator boundary is respected without being
//! special-cased.
//!
//! Zone endpoint states are therefore **exact**, not linearised: the cold
//! temperature at each boundary comes from a real IF97 `(p,h)` flash, so each
//! zone's capacity rate is the secant `C = m (h_2 - h_1)/(T_2 - T_1)` -- the
//! same construction DWSIM uses, and the one
//! [`outram_park_fork_dwsim_libs::heat_exchanger::ntu_effectiveness`]'s module
//! documentation asks a caller with flash access to supply. The only place a
//! constant-`c_p` approximation survives is the *shape* of the profile inside a
//! zone, not the zone's endpoints or its duty.
//!
//! ## LMTD or effectiveness-NTU in the two-phase zone?
//!
//! **LMTD, and the choice is not a compromise.** Inside the evaporator the cold
//! stream is isothermal at `T_sat`, so the capacity-rate ratio is `C_r = 0`, and
//! at `C_r = 0` the LMTD relation `Q = UA * LMTD` and the effectiveness relation
//! `Q = C_hot * (T_hot,in - T_sat) * (1 - exp(-NTU))` are the *same equation*
//! rearranged -- both exact, both independent of flow arrangement, with the
//! multipass correction `F = 1`. [`tests::two_phase_lmtd_and_effectiveness_ntu_are_the_same_equation`]
//! measures that identity rather than asserting it.
//!
//! The reason to write it as LMTD here is the direction of the unknown. The
//! brief for effectiveness-NTU is "given `UA`, find `Q`", and it is the right
//! tool when the outlet is what you are solving for. But in this formulation the
//! evaporator's duty is **not** unknown: it is pinned by the latent heat,
//! `Q_2ph = m_cold * (h_g - h_f)`. What the outer bisection needs from each zone
//! is the *conductance that duty consumes*, which LMTD gives by direct division
//! with no inner iteration. Using effectiveness-NTU here would mean solving
//! `1 - exp(-NTU) = Q_2ph/(C_hot * dT)` for `NTU` -- the same number, obtained by
//! inverting an exponential instead of dividing. The outlet-first objection to
//! LMTD does not apply, because the outlet is supplied by the outer solve.
//!
//! # Rebuilding the node profiles
//!
//! With the duty split known, each zone occupies a known share of the total
//! conductance, and -- because the contract's conductances are uniform per node
//! -- therefore a known share of the exchanger's length. Inside a zone the
//! steady counter-flow solution is a pure exponential in accumulated
//! conductance,
//!
//! ```text
//! dT(xi) = dT_a * exp(-kappa * (xi - xi_a)),   kappa = UA_total * (1/C_hot - 1/C_cold)
//! ```
//!
//! which is integrated in closed form to give the accumulated duty, hence the
//! hot temperature, at each node centre `xi_i = (i + 1/2)/n`. The cold node
//! temperature is then taken from the cold enthalpy at that station through a
//! real IF97 `(p,h)` flash rather than from the linearised zone `c_p`, so the
//! saturation plateau appears in the rebuilt profile as a plateau, at the right
//! temperature, without being imposed.
//!
//! The tube metal is set to its steady value, the conductance-weighted mean
//! `T_metal = (UA_hot * T_hot + UA_cold * T_cold) / (UA_hot + UA_cold)`, which
//! is the temperature at which the two film heat flows balance.
//!
//! # Where this method is NOT trustworthy
//!
//! Read this list before believing any number this remedy produces.
//!
//! - **The transient is gone.** The profile is the steady one for the current
//!   boundary conditions. The remedy fires *during* transients, so it destroys
//!   exactly the dynamics a digital twin exists to show. The tube metal's
//!   thermal lag -- the physically dominant time constant of this exchanger --
//!   is discarded outright, because the metal is *set*, not integrated.
//! - **The hot stream is assumed to be helium.** [`super::CrossRepairInputs`]
//!   carries a hot-side pressure but no fluid identity, and a capacity rate
//!   cannot be formed without one. This module therefore evaluates the hot side
//!   as helium through [`outram_park_fork_coolprop`], which is correct for
//!   `htgr_sim_v1` and **silently wrong for any other hot fluid**. It is a
//!   contract gap, not a modelling choice.
//! - **`U` is constant within a zone and uniform along it.** The contract
//!   supplies one hot-side and one cold-side film conductance per node, with no
//!   axial or regime dependence, so the large real variation of the boiling and
//!   the single-phase-gas heat transfer coefficients is not represented. Zoning
//!   removes the `c_p` error across the boiling boundary; it does not remove
//!   this one.
//! - **The profile shape inside a zone is linearised even though its endpoints
//!   are not.** Near a tight pinch the exponential shape and the true IF97 one
//!   can differ enough to reintroduce a small cross, which is detected and
//!   reported as [`CrossRepairError::DidNotConverge`] rather than shipped.
//! - **Pure single-pass counter-current is assumed**, `F = 1`. A real helical
//!   once-through bundle is not exactly that.
//! - **No pressure drop, no axial conduction, no metal capacitance, no
//!   inventory dynamics.** Both pressures are held at their supplied values.
//! - **Supercritical cold-side pressure is refused, not approximated.** Above
//!   the water critical pressure there is no saturation line to zone on, and the
//!   pseudo-critical `c_p` peak is precisely the regime LMTD handles worst.
//! - **The IF97 range binds.** A cold outlet beyond 1073.15 K is refused rather
//!   than clamped.
//! - **The energy audit is a proxy, not the true stored-energy jump** -- see
//!   [`audit_energy_discrepancy`] for exactly what it does and does not measure.
//!
//! Of the three remedies in this module this is the one with the most
//! assumptions between the inputs and the answer. It should be the last one
//! tried, not the first.

// The `temperature_cross` module is not dispatched from the exchanger yet --
// nothing outside tests calls `TemperatureCrossRemedy::apply` -- so every item
// in this file is reachable only from this file's own test module, and the
// dead-code lint fires on all of them. **Delete this attribute** when the
// remedy is wired into `super::super::steam_generator`; it is a scaffolding
// suppression, not a standing exemption, and leaving it in place after wiring
// would hide genuinely unused code.
#![allow(dead_code)]

use super::{CrossRepairError, CrossRepairInputs, CrossRepairOutcome};

use outram_park_fork_dwsim_libs::heat_exchanger::lmtd::{lmtd as dwsim_lmtd, FlowArrangement};
use tampines_steam_tables::interfaces::checked::{try_h_tp_eqm_single_phase, try_t_ph_eqm};
use tampines_steam_tables::prelude::TampinesSteamTableCV;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::energy::joule;
use uom::si::f64::*;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;
use uom::si::temperature_interval::kelvin as kelvin_interval;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

/// Upper temperature bound of the IAPWS-IF97 industrial formulation \[K\].
///
/// Regions 1, 2 and 4 are defined to 800 degC. The cold stream may not be
/// driven past this, and this module refuses rather than clamps -- a clamp here
/// would be a silent second-law-consistent-looking lie about the steam outlet.
/// Mirrors `super::super::secondary_loop`'s constant of the same value.
const IF97_MAX_TEMPERATURE_KELVIN: f64 = 1073.15;

/// Ideal-gas-limit isobaric specific heat of helium \[J/(kg K)\].
///
/// Used only if the CoolProp-derived Helmholtz flash fails to converge, which
/// it does not at HTGR conditions (3 MPa, 500-1000 K, where helium is within a
/// fraction of a percent of the ideal-gas limit). Same fallback value and same
/// justification as `super::super::primary_loop::helium_properties`.
const HELIUM_IDEAL_GAS_CP_J_PER_KG_K: f64 = 5193.0;

/// Half-width \[K\] of the band around `T_sat` in which a node's temperature is
/// treated as not determining its enthalpy, for energy auditing only.
///
/// A two-phase node's enthalpy is a free variable at fixed `(T, p)` -- the IF97
/// `(T,p)` flash is under-determined there and `tampines-steam-tables` says so
/// explicitly (`SteamTablesError::SaturatedTpUnderdetermined`). Reading an
/// enthalpy off a temperature within this band would swing by the full latent
/// heat on a rounding difference, so the audit uses a fixed datum there
/// instead. See [`audit_energy_discrepancy`].
const SATURATION_AUDIT_BAND_KELVIN: f64 = 0.05;

/// Reference residence time \[s\] used to turn the streams' specific-enthalpy
/// change into a joule-valued audit figure.
///
/// **This is a stated assumption about inventory, not a measurement.**
/// [`super::CrossRepairInputs`] carries no node mass, no node volume and no
/// timestep, so a stored-energy jump in joules is not constructible from it.
/// The audit therefore assumes each stream's in-exchanger inventory equals one
/// second of its own mass flow. The reported energy scales **exactly linearly**
/// with this number, so a maintainer who knows the real inventory can rescale
/// it without re-running anything, and its sign and its trend over a run are
/// unaffected. See [`audit_energy_discrepancy`].
const AUDIT_REFERENCE_RESIDENCE_TIME_S: f64 = 1.0;

/// Largest node-wise cross \[K\] tolerated in the rebuilt profile before the
/// repair is declared a failure. Round-off scale, not a physical tolerance.
const CROSS_TOLERANCE_KELVIN: f64 = 1.0e-9;

/// Repair a crossed profile by imposing the zoned-LMTD steady profile.
///
/// Solves for the steady total duty that the installed conductance supports,
/// splits it across the subcooled / two-phase / superheat zones at the IF97
/// saturation enthalpies, and rebuilds all three node profiles from the closed-
/// form counter-flow solution within each zone. The tube metal is set to the
/// steady conductance-weighted mean of the two fluid temperatures.
///
/// Units: `inputs` temperatures are absolute (K), conductances are per node in
/// W/K, mass flows in kg/s, pressures in Pa, and the cold inlet boundary
/// condition is a specific enthalpy in J/kg. The returned profiles are absolute
/// temperatures (K) and the discrepancy is an energy (J) -- see
/// [`audit_energy_discrepancy`] for what that number actually measures.
///
/// **The result is a steady state, not a timestep.** See the module
/// documentation's "Where this method is NOT trustworthy" section before using
/// any number derived from it.
///
/// # Errors
///
/// - [`CrossRepairError::BadInputs`] for mismatched profile lengths, an empty
///   profile, a non-positive or non-finite mass flow, conductance, pressure or
///   temperature.
/// - [`CrossRepairError::DidNotConverge`] when the zone structure cannot be
///   determined (cold pressure at or above the water critical pressure), when
///   the hot inlet is not hotter than the cold inlet, when the steady solution
///   would drive the cold stream outside the IF97 validity range, when an IF97
///   flash rejects a state, or when the rebuilt profile still contains a cross
///   larger than round-off. **A caller must not read an error as "no repair
///   needed".**
pub fn repair(inputs: &CrossRepairInputs) -> Result<CrossRepairOutcome, CrossRepairError> {
    let node_count = validate(inputs)?;

    let saturation = ColdSaturation::at(inputs.cold_pressure)?;
    let context = SolveContext::build(inputs, saturation, node_count)?;
    let (zones, hot_capacity_rate_w_per_k) = solve_duty(&context)?;

    let (hot, cold, metal) = rebuild_profiles(
        inputs,
        &context,
        &zones,
        hot_capacity_rate_w_per_k,
        node_count,
    )?;

    let outcome = CrossRepairOutcome {
        energy_discrepancy: audit_energy_discrepancy(inputs, &hot, &cold, saturation)?,
        hot_temperatures: hot,
        metal_temperatures: metal,
        cold_temperatures: cold,
    };

    // NON-NEGOTIABLE: never hand back a profile that still violates the second
    // law. The zone endpoints are exact but the within-zone shape is not, so
    // near a tight pinch this can genuinely fail -- and when it does, saying so
    // is the only honest option.
    let worst = worst_cross_kelvin(&outcome.hot_temperatures, &outcome.cold_temperatures);
    if worst > CROSS_TOLERANCE_KELVIN {
        return Err(CrossRepairError::DidNotConverge(format!(
            "zoned-LMTD profile still crosses by {worst:.6e} K (zones: {}); \
             the within-zone linearisation cannot resolve this pinch",
            zones.describe()
        )));
    }

    Ok(outcome)
}

/// Largest amount \[K\] by which the cold stream exceeds the hot at any node.
///
/// Same measure as [`super::CrossRepairInputs::worst_cross_kelvin`], applied to
/// a candidate output rather than to the inputs. Zero or negative means no
/// cross.
fn worst_cross_kelvin(hot: &[ThermodynamicTemperature], cold: &[ThermodynamicTemperature]) -> f64 {
    hot.iter().zip(cold.iter()).fold(0.0_f64, |worst, (h, c)| {
        worst.max(c.get::<kelvin>() - h.get::<kelvin>())
    })
}

/// Reject inputs this remedy cannot use, and return the node count.
///
/// Checks the three profiles are the same non-zero length, that every supplied
/// temperature is finite and above absolute zero \[K\], and that both mass flows
/// \[kg/s\], both node conductances \[W/K\] and both pressures \[Pa\] are finite
/// and strictly positive.
fn validate(inputs: &CrossRepairInputs) -> Result<usize, CrossRepairError> {
    let node_count = inputs.hot_temperatures.len();
    if node_count == 0 {
        return Err(CrossRepairError::BadInputs(
            "the exchanger has no nodes".to_string(),
        ));
    }
    if inputs.metal_temperatures.len() != node_count || inputs.cold_temperatures.len() != node_count
    {
        return Err(CrossRepairError::BadInputs(format!(
            "profile lengths differ: hot {}, metal {}, cold {}",
            node_count,
            inputs.metal_temperatures.len(),
            inputs.cold_temperatures.len()
        )));
    }

    let finite_positive = |name: &str, value: f64| -> Result<(), CrossRepairError> {
        if !value.is_finite() || value <= 0.0 {
            return Err(CrossRepairError::BadInputs(format!(
                "{name} = {value} is not finite and positive"
            )));
        }
        Ok(())
    };

    finite_positive(
        "hot mass flow [kg/s]",
        inputs.hot_mass_flow.get::<kilogram_per_second>(),
    )?;
    finite_positive(
        "cold mass flow [kg/s]",
        inputs.cold_mass_flow.get::<kilogram_per_second>(),
    )?;
    finite_positive(
        "hot node conductance [W/K]",
        inputs.hot_node_conductance.get::<watt_per_kelvin>(),
    )?;
    finite_positive(
        "cold node conductance [W/K]",
        inputs.cold_node_conductance.get::<watt_per_kelvin>(),
    )?;
    finite_positive("hot pressure [Pa]", inputs.hot_pressure.get::<pascal>())?;
    finite_positive("cold pressure [Pa]", inputs.cold_pressure.get::<pascal>())?;
    finite_positive(
        "hot inlet temperature [K]",
        inputs.hot_inlet_temperature.get::<kelvin>(),
    )?;
    if !inputs
        .cold_inlet_enthalpy
        .get::<joule_per_kilogram>()
        .is_finite()
    {
        return Err(CrossRepairError::BadInputs(
            "cold inlet specific enthalpy [J/kg] is not finite".to_string(),
        ));
    }

    for (index, temperature) in inputs
        .hot_temperatures
        .iter()
        .chain(inputs.metal_temperatures.iter())
        .chain(inputs.cold_temperatures.iter())
        .enumerate()
    {
        let value = temperature.get::<kelvin>();
        if !value.is_finite() || value <= 0.0 {
            return Err(CrossRepairError::BadInputs(format!(
                "node temperature #{index} = {value} K is not finite and positive"
            )));
        }
    }

    Ok(node_count)
}

/// The cold side's saturation state at the operating pressure -- the two
/// enthalpies the zone boundaries sit on, and the temperature they sit at.
///
/// All from IAPWS-IF97: `T_sat` \[K\] from the Region 4 saturation line,
/// `h_f`/`h_g` \[J/kg\] from the saturated-liquid and saturated-vapour states at
/// that pressure.
#[derive(Clone, Copy, Debug)]
struct ColdSaturation {
    /// Saturation temperature \[K\] at the cold-side pressure.
    t_sat_kelvin: f64,
    /// Saturated-liquid specific enthalpy \[J/kg\] -- the subcooled/two-phase
    /// zone boundary.
    h_f_j_per_kg: f64,
    /// Saturated-vapour specific enthalpy \[J/kg\] -- the two-phase/superheat
    /// zone boundary.
    h_g_j_per_kg: f64,
}

impl ColdSaturation {
    /// Evaluate the saturation line at `pressure` \[Pa\].
    ///
    /// # Errors
    ///
    /// [`CrossRepairError::DidNotConverge`] at or above the water critical
    /// pressure (22.064 MPa), where there is no saturation line and therefore
    /// no zone structure to determine. Refusing is deliberate: the
    /// pseudo-critical `c_p` peak above the critical point is the single worst
    /// regime for a constant-`c_p` method, so a "one supercritical zone"
    /// fallback would be a confident wrong answer of exactly the kind this
    /// module exists to avoid.
    fn at(pressure: Pressure) -> Result<Self, CrossRepairError> {
        let t_sat = TampinesSteamTableCV::try_get_tsat(pressure).ok_or_else(|| {
            CrossRepairError::DidNotConverge(format!(
                "cold-side pressure {:.6e} Pa is at or above the water critical \
                 pressure: there is no saturation line, so the subcooled / \
                 two-phase / superheat zone structure is undefined",
                pressure.get::<pascal>()
            ))
        })?;

        let reference_volume = TampinesSteamTableCV::get_ref_vol();
        let h_f =
            TampinesSteamTableCV::new_from_sat_pressure_quality(pressure, 0.0, reference_volume)
                .get_specific_enthalpy()
                .get::<joule_per_kilogram>();
        let h_g =
            TampinesSteamTableCV::new_from_sat_pressure_quality(pressure, 1.0, reference_volume)
                .get_specific_enthalpy()
                .get::<joule_per_kilogram>();

        if !h_f.is_finite() || !h_g.is_finite() || h_g <= h_f {
            return Err(CrossRepairError::DidNotConverge(format!(
                "IF97 saturation enthalpies at {:.6e} Pa are not usable: \
                 h_f = {h_f:.6e} J/kg, h_g = {h_g:.6e} J/kg",
                pressure.get::<pascal>()
            )));
        }

        Ok(Self {
            t_sat_kelvin: t_sat.get::<kelvin>(),
            h_f_j_per_kg: h_f,
            h_g_j_per_kg: h_g,
        })
    }
}

/// Everything the duty solve needs, in plain SI scalars.
///
/// Assembled once from the `uom`-typed contract so the bisection's inner loop
/// is not re-unwrapping quantities. Units are named on every field.
#[derive(Clone, Copy, Debug)]
struct SolveContext {
    /// Total installed conductance \[W/K\]: the per-node series combination of
    /// the two film conductances, times the node count.
    ua_total_w_per_k: f64,
    /// Hot-stream mass flow \[kg/s\].
    hot_mass_flow_kg_per_s: f64,
    /// Cold-stream mass flow \[kg/s\].
    cold_mass_flow_kg_per_s: f64,
    /// Hot-stream inlet temperature \[K\] (the boundary condition).
    t_hot_inlet_kelvin: f64,
    /// Hot-side pressure \[Pa\].
    hot_pressure_pa: f64,
    /// Cold-side pressure \[Pa\].
    cold_pressure_pa: f64,
    /// Cold inlet specific enthalpy \[J/kg\] (the boundary condition).
    h_cold_inlet_j_per_kg: f64,
    /// Cold inlet temperature \[K\], from the IF97 `(p,h)` flash of the above.
    t_cold_inlet_kelvin: f64,
    /// Cold-side saturation state at the operating pressure.
    saturation: ColdSaturation,
    /// Largest duty \[W\] the bisection may consider: the smaller of the
    /// terminal pinch limit and the IF97 range limit.
    q_bracket_upper_w: f64,
    /// True when [`Self::q_bracket_upper_w`] was set by the IF97 range rather
    /// than by the pinch, so a solve that runs into it can say which bound bit.
    bracket_is_if97_limited: bool,
}

impl SolveContext {
    /// Assemble the solve context, including the duty bracket.
    ///
    /// # Errors
    ///
    /// [`CrossRepairError::DidNotConverge`] if the hot inlet is not strictly
    /// hotter than the cold inlet (there is then no definite hot and cold
    /// stream, and the zoning has no meaning), or if an IF97 flash rejects one
    /// of the boundary states.
    fn build(
        inputs: &CrossRepairInputs,
        saturation: ColdSaturation,
        node_count: usize,
    ) -> Result<Self, CrossRepairError> {
        let ua_hot = inputs.hot_node_conductance.get::<watt_per_kelvin>();
        let ua_cold = inputs.cold_node_conductance.get::<watt_per_kelvin>();
        let ua_total = node_count as f64 / (1.0 / ua_hot + 1.0 / ua_cold);

        let hot_mass_flow = inputs.hot_mass_flow.get::<kilogram_per_second>();
        let cold_mass_flow = inputs.cold_mass_flow.get::<kilogram_per_second>();
        let t_hot_inlet = inputs.hot_inlet_temperature.get::<kelvin>();
        let h_cold_inlet = inputs.cold_inlet_enthalpy.get::<joule_per_kilogram>();
        let cold_pressure = inputs.cold_pressure;
        let t_cold_inlet = cold_temperature_kelvin(cold_pressure, h_cold_inlet)?;

        if t_hot_inlet <= t_cold_inlet {
            return Err(CrossRepairError::DidNotConverge(format!(
                "hot inlet {t_hot_inlet:.3} K is not hotter than the cold inlet \
                 {t_cold_inlet:.3} K, so there is no definite hot and cold \
                 stream to zone"
            )));
        }

        // Terminal pinch bound: the cold stream may at most be brought to the
        // hot inlet temperature, and never past the IF97 ceiling.
        let t_cap = t_hot_inlet.min(IF97_MAX_TEMPERATURE_KELVIN);
        let bracket_is_if97_limited = t_hot_inlet > IF97_MAX_TEMPERATURE_KELVIN;
        let h_cold_cap = try_h_tp_eqm_single_phase(
            ThermodynamicTemperature::new::<kelvin>(t_cap),
            cold_pressure,
        )
        .map_err(|e| {
            CrossRepairError::DidNotConverge(format!(
                "IF97 could not evaluate the cold-side enthalpy ceiling at \
                 {t_cap:.3} K, {:.6e} Pa: {e}",
                cold_pressure.get::<pascal>()
            ))
        })?
        .get::<joule_per_kilogram>();

        let q_bracket_upper = cold_mass_flow * (h_cold_cap - h_cold_inlet);
        if !(q_bracket_upper > 0.0) {
            return Err(CrossRepairError::DidNotConverge(format!(
                "no positive duty is admissible: the cold stream is already at \
                 or above the enthalpy the hot inlet could bring it to \
                 ({h_cold_inlet:.6e} J/kg against a ceiling of \
                 {h_cold_cap:.6e} J/kg)"
            )));
        }

        Ok(Self {
            ua_total_w_per_k: ua_total,
            hot_mass_flow_kg_per_s: hot_mass_flow,
            cold_mass_flow_kg_per_s: cold_mass_flow,
            t_hot_inlet_kelvin: t_hot_inlet,
            hot_pressure_pa: inputs.hot_pressure.get::<pascal>(),
            cold_pressure_pa: cold_pressure.get::<pascal>(),
            h_cold_inlet_j_per_kg: h_cold_inlet,
            t_cold_inlet_kelvin: t_cold_inlet,
            saturation,
            q_bracket_upper_w: q_bracket_upper,
            bracket_is_if97_limited,
        })
    }

    /// Cold-side pressure as a `uom` quantity, for the IF97 calls.
    fn cold_pressure(&self) -> Pressure {
        Pressure::new::<pascal>(self.cold_pressure_pa)
    }
}

/// Which of the three heat-transfer regimes a zone is in, on the cold side.
///
/// Ordered as they appear from the hot-inlet end of a counter-flow once-through
/// steam generator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ZoneKind {
    /// Cold side is superheated vapour: `h > h_g`.
    Superheat,
    /// Cold side is boiling at `T_sat`: `h_f <= h <= h_g`. LMTD is exact here.
    TwoPhase,
    /// Cold side is subcooled liquid (the economiser): `h < h_f`.
    Subcooled,
}

impl ZoneKind {
    /// Short human-readable name, for error messages.
    fn label(self) -> &'static str {
        match self {
            Self::Superheat => "superheat",
            Self::TwoPhase => "two-phase",
            Self::Subcooled => "subcooled",
        }
    }
}

/// One zone of the exchanger, fully resolved at a trial total duty.
///
/// "hi" always means the end at higher cold-side enthalpy -- the end nearer the
/// hot inlet, at smaller `xi` -- and "lo" the end nearer the feedwater inlet.
#[derive(Clone, Copy, Debug)]
struct Zone {
    /// Which regime the cold side is in through this zone.
    kind: ZoneKind,
    /// Heat duty of this zone \[W\].
    duty_w: f64,
    /// Cold temperature \[K\] at the zone's high-enthalpy end.
    t_cold_hi_kelvin: f64,
    /// Cold temperature \[K\] at the zone's low-enthalpy end.
    t_cold_lo_kelvin: f64,
    /// Hot temperature \[K\] at the zone's high-enthalpy end.
    t_hot_hi_kelvin: f64,
    /// Hot temperature \[K\] at the zone's low-enthalpy end.
    t_hot_lo_kelvin: f64,
    /// Duty \[W\] accumulated from the hot inlet up to this zone's
    /// high-enthalpy end.
    q_at_hi_end_w: f64,
    /// Conductance \[W/K\] this zone demands, `duty / LMTD`.
    ua_w_per_k: f64,
    /// Cold-side secant capacity rate \[W/K\], `duty/(T_hi - T_lo)`. `None` in
    /// the two-phase zone, where the cold stream is isothermal and the capacity
    /// rate is infinite.
    c_cold_w_per_k: Option<f64>,
}

/// The zones of one trial solution, in order from the hot-inlet end.
///
/// At most three; a zone with no duty is omitted rather than carried as a
/// zero-width entry, so a purely subcooled exchanger really does have one zone.
#[derive(Clone, Copy, Debug)]
struct ZoneSet {
    /// Backing storage; only the first [`Self::count`] entries are meaningful.
    zones: [Zone; 3],
    /// Number of live zones.
    count: usize,
    /// Total duty \[W\] across all zones.
    q_total_w: f64,
    /// Sum of the zones' demanded conductances \[W/K\].
    ua_demanded_w_per_k: f64,
}

impl ZoneSet {
    /// The live zones, hot-inlet end first.
    fn live(&self) -> &[Zone] {
        &self.zones[..self.count]
    }

    /// Zone structure as prose, e.g. `"superheat + two-phase + subcooled"`, for
    /// error messages.
    fn describe(&self) -> String {
        if self.count == 0 {
            return "none".to_string();
        }
        self.live()
            .iter()
            .map(|zone| zone.kind.label())
            .collect::<Vec<_>>()
            .join(" + ")
    }
}

/// Cold-stream temperature \[K\] at `(p, h)` from the IAPWS-IF97 `(p,h)` flash.
///
/// Uses the bounds-checked facade, so an out-of-envelope state comes back as an
/// error instead of a panic. In the two-phase region this correctly returns
/// `T_sat(p)`.
fn cold_temperature_kelvin(pressure: Pressure, h_j_per_kg: f64) -> Result<f64, CrossRepairError> {
    try_t_ph_eqm(
        pressure,
        AvailableEnergy::new::<joule_per_kilogram>(h_j_per_kg),
    )
    .map(|t| t.get::<kelvin>())
    .map_err(|e| {
        CrossRepairError::DidNotConverge(format!(
            "IF97 (p,h) flash failed at {:.6e} Pa, {h_j_per_kg:.6e} J/kg: {e}",
            pressure.get::<pascal>()
        ))
    })
}

/// Isobaric specific heat \[J/(kg K)\] of the hot stream at `(T, p)`.
///
/// **Assumes the hot stream is helium** -- see the module documentation; the
/// contract carries no fluid identity. Evaluated through the CoolProp-derived
/// Helmholtz EOS, falling back to the ideal-gas limit
/// [`HELIUM_IDEAL_GAS_CP_J_PER_KG_K`] if the flash fails to converge, which is
/// a cited physical bound rather than an invented number.
fn hot_specific_heat_j_per_kg_k(t_kelvin: f64, pressure_pa: f64) -> f64 {
    match outram_park_fork_coolprop::state_pt(
        outram_park_fork_coolprop::Fluid::Helium,
        t_kelvin,
        pressure_pa,
    ) {
        Ok(state) if state.cp.is_finite() && state.cp > 0.0 => state.cp,
        _ => HELIUM_IDEAL_GAS_CP_J_PER_KG_K,
    }
}

/// Specific enthalpy \[J/kg\] of the hot stream at `(T, p)`, or `None` if the
/// flash fails.
///
/// **Assumes the hot stream is helium**, as [`hot_specific_heat_j_per_kg_k`]
/// does. The value carries the EOS's own reference state; only differences of
/// it are used here, so that offset cancels.
fn hot_specific_enthalpy_j_per_kg(t_kelvin: f64, pressure_pa: f64) -> Option<f64> {
    match outram_park_fork_coolprop::state_pt(
        outram_park_fork_coolprop::Fluid::Helium,
        t_kelvin,
        pressure_pa,
    ) {
        Ok(state) if state.enthalpy.is_finite() => Some(state.enthalpy),
        _ => None,
    }
}

/// Log-mean temperature difference \[K\] of one zone, from its four terminal
/// temperatures \[K\].
///
/// Delegates to [`outram_park_fork_dwsim_libs::heat_exchanger::lmtd::lmtd`] in
/// its counter-current form -- this module does not re-derive LMTD -- but
/// guards the equal-end-differences case first. That case is a genuine `0/0` in
/// the DWSIM port (`(dt1 - dt2)/ln(dt1/dt2)` with `dt1 == dt2` evaluates to
/// `NaN`), and it is not a pathological input here: a balanced counter-flow
/// zone has exactly equal terminal differences. The limit is the arithmetic
/// mean, which is what the guard returns.
fn zone_lmtd_kelvin(
    t_hot_hi_kelvin: f64,
    t_hot_lo_kelvin: f64,
    t_cold_hi_kelvin: f64,
    t_cold_lo_kelvin: f64,
) -> f64 {
    let delta_hi = t_hot_hi_kelvin - t_cold_hi_kelvin;
    let delta_lo = t_hot_lo_kelvin - t_cold_lo_kelvin;
    if (delta_hi - delta_lo).abs() <= 1.0e-9 * delta_hi.abs().max(1.0) {
        return 0.5 * (delta_hi + delta_lo);
    }
    dwsim_lmtd(
        FlowArrangement::CounterCurrent,
        ThermodynamicTemperature::new::<kelvin>(t_hot_hi_kelvin),
        ThermodynamicTemperature::new::<kelvin>(t_hot_lo_kelvin),
        ThermodynamicTemperature::new::<kelvin>(t_cold_lo_kelvin),
        ThermodynamicTemperature::new::<kelvin>(t_cold_hi_kelvin),
    )
    .get::<kelvin_interval>()
}

/// Resolve the zone structure at a trial total duty and hot-side capacity rate.
///
/// Returns `Ok(None)` when the trial duty is **infeasible** -- some zone's
/// terminal temperature difference is zero or negative, so it would demand
/// infinite conductance. Infeasible always means "above the root", which is how
/// the internal evaporator pinch is handled without a special case.
///
/// Units: `q_total_w` in W, `c_hot_w_per_k` in W/K.
fn zones_at_duty(
    context: &SolveContext,
    q_total_w: f64,
    c_hot_w_per_k: f64,
) -> Result<Option<ZoneSet>, CrossRepairError> {
    let cold_pressure = context.cold_pressure();
    let h_in = context.h_cold_inlet_j_per_kg;
    let h_out = h_in + q_total_w / context.cold_mass_flow_kg_per_s;
    let saturation = context.saturation;

    // Enthalpy segments, hot-inlet end first (descending cold enthalpy).
    let segments = [
        (ZoneKind::Superheat, saturation.h_g_j_per_kg, f64::INFINITY),
        (
            ZoneKind::TwoPhase,
            saturation.h_f_j_per_kg,
            saturation.h_g_j_per_kg,
        ),
        (
            ZoneKind::Subcooled,
            f64::NEG_INFINITY,
            saturation.h_f_j_per_kg,
        ),
    ];

    // A zone narrower than this in specific enthalpy carries no meaningful duty
    // and is dropped; it would otherwise contribute a 0/0 LMTD.
    let enthalpy_epsilon = 1.0e-6 * (saturation.h_g_j_per_kg - saturation.h_f_j_per_kg);

    let placeholder = Zone {
        kind: ZoneKind::TwoPhase,
        duty_w: 0.0,
        t_cold_hi_kelvin: 0.0,
        t_cold_lo_kelvin: 0.0,
        t_hot_hi_kelvin: 0.0,
        t_hot_lo_kelvin: 0.0,
        q_at_hi_end_w: 0.0,
        ua_w_per_k: 0.0,
        c_cold_w_per_k: None,
    };
    let mut zones = [placeholder; 3];
    let mut count = 0_usize;
    let mut ua_demanded = 0.0_f64;

    let mut t_hot = context.t_hot_inlet_kelvin;
    let mut q_accumulated = 0.0_f64;

    for (kind, segment_lo, segment_hi) in segments {
        let hi = h_out.min(segment_hi);
        let lo = h_in.max(segment_lo);
        if hi - lo <= enthalpy_epsilon {
            continue;
        }

        let duty = context.cold_mass_flow_kg_per_s * (hi - lo);
        let t_cold_hi = cold_temperature_kelvin(cold_pressure, hi)?;
        let t_cold_lo = cold_temperature_kelvin(cold_pressure, lo)?;
        let t_hot_hi = t_hot;
        let t_hot_lo = t_hot - duty / c_hot_w_per_k;

        // Infeasible: this trial duty would need the cold stream to overtake
        // the hot somewhere in this zone.
        if t_hot_hi - t_cold_hi <= 0.0 || t_hot_lo - t_cold_lo <= 0.0 {
            return Ok(None);
        }

        let lmtd = zone_lmtd_kelvin(t_hot_hi, t_hot_lo, t_cold_hi, t_cold_lo);
        if !lmtd.is_finite() || lmtd <= 0.0 {
            return Ok(None);
        }

        let temperature_rise = t_cold_hi - t_cold_lo;
        let c_cold = if kind == ZoneKind::TwoPhase || temperature_rise <= 1.0e-9 {
            None
        } else {
            Some(duty / temperature_rise)
        };

        zones[count] = Zone {
            kind,
            duty_w: duty,
            t_cold_hi_kelvin: t_cold_hi,
            t_cold_lo_kelvin: t_cold_lo,
            t_hot_hi_kelvin: t_hot_hi,
            t_hot_lo_kelvin: t_hot_lo,
            q_at_hi_end_w: q_accumulated,
            ua_w_per_k: duty / lmtd,
            c_cold_w_per_k: c_cold,
        };
        ua_demanded += duty / lmtd;
        count += 1;

        t_hot = t_hot_lo;
        q_accumulated += duty;
    }

    Ok(Some(ZoneSet {
        zones,
        count,
        q_total_w,
        ua_demanded_w_per_k: ua_demanded,
    }))
}

/// Solve for the steady total duty, and with it the hot-side capacity rate.
///
/// Two nested loops:
///
/// - **Inner (bisection on duty).** The conductance demanded by the zone
///   structure is monotone increasing in duty, so bisecting on
///   `sum(UA_zone) - UA_total` converges unconditionally. Infeasible trials
///   (pinch violated) are treated as lying above the root.
/// - **Outer (hot capacity rate).** `C_hot = m_hot * c_p` needs a temperature at
///   which to evaluate `c_p`, and that temperature is an output. The loop
///   re-evaluates `C_hot` as the exact secant
///   `m_hot * (h(T_in) - h(T_out))/(T_in - T_out)` over the solved hot-side
///   span and re-solves, which makes the hot-side energy balance
///   endpoint-exact rather than constant-`c_p`. For helium at 3 MPa this barely
///   has to move: the eight-node HTR-10-like fixture converged to
///   `C_hot = 2.2323e4 W/K` at 4.3 kg/s (measured 2026-08-13), i.e. an
///   effective `c_p` of 5191.4 J/(kg K), within 0.03% of the 5193 J/(kg K)
///   ideal-gas limit.
///
/// Returns the converged zone structure and the hot capacity rate \[W/K\] it was
/// solved with.
///
/// # Errors
///
/// [`CrossRepairError::DidNotConverge`] if the installed conductance demands
/// more duty than the pinch or the IF97 range allows.
fn solve_duty(context: &SolveContext) -> Result<(ZoneSet, f64), CrossRepairError> {
    let mut c_hot = context.hot_mass_flow_kg_per_s
        * hot_specific_heat_j_per_kg_k(context.t_hot_inlet_kelvin, context.hot_pressure_pa);

    let mut solution = bisect_duty(context, c_hot)?;

    for _ in 0..4 {
        let t_hot_outlet = context.t_hot_inlet_kelvin - solution.q_total_w / c_hot;
        let refined = secant_hot_capacity_rate(context, t_hot_outlet).unwrap_or(c_hot);
        if (refined - c_hot).abs() <= 1.0e-9 * c_hot {
            c_hot = refined;
            break;
        }
        c_hot = refined;
        solution = bisect_duty(context, c_hot)?;
    }

    Ok((solution, c_hot))
}

/// Hot-side capacity rate \[W/K\] as the exact enthalpy secant over the solved
/// temperature span, `m_hot * (h(T_in) - h(T_out)) / (T_in - T_out)`.
///
/// Returns `None` if the span is too small to take a secant over, or if either
/// hot-side enthalpy flash fails; the caller then keeps its previous estimate.
fn secant_hot_capacity_rate(context: &SolveContext, t_hot_outlet_kelvin: f64) -> Option<f64> {
    let span = context.t_hot_inlet_kelvin - t_hot_outlet_kelvin;
    if !(span > 1.0e-6) {
        return None;
    }
    let h_in = hot_specific_enthalpy_j_per_kg(context.t_hot_inlet_kelvin, context.hot_pressure_pa)?;
    let h_out = hot_specific_enthalpy_j_per_kg(t_hot_outlet_kelvin, context.hot_pressure_pa)?;
    let capacity_rate = context.hot_mass_flow_kg_per_s * (h_in - h_out) / span;
    if capacity_rate.is_finite() && capacity_rate > 0.0 {
        Some(capacity_rate)
    } else {
        None
    }
}

/// Bisect the total duty \[W\] so the zones' demanded conductance equals the
/// installed conductance, at a fixed hot capacity rate `c_hot_w_per_k` \[W/K\].
///
/// The bracket is `[0, q_bracket_upper]`. Zero duty always demands zero
/// conductance, so the lower end is feasible by construction; the upper end is
/// the terminal pinch (or the IF97 ceiling), where the demand diverges.
///
/// # Errors
///
/// [`CrossRepairError::DidNotConverge`] when even the bracket's upper end
/// demands less conductance than the exchanger has -- i.e. the steady state the
/// installed `UA` implies lies outside the admissible range, which happens when
/// the IF97 ceiling rather than the pinch set the bracket.
fn bisect_duty(context: &SolveContext, c_hot_w_per_k: f64) -> Result<ZoneSet, CrossRepairError> {
    let ua_total = context.ua_total_w_per_k;
    let mut low = 0.0_f64;
    let mut high = context.q_bracket_upper_w;

    if let Some(at_high) = zones_at_duty(context, high, c_hot_w_per_k)? {
        if at_high.ua_demanded_w_per_k <= ua_total {
            let reason = if context.bracket_is_if97_limited {
                "the cold stream would have to leave the IAPWS-IF97 validity \
                 range (1073.15 K)"
            } else {
                "the steady duty the installed conductance implies exceeds the \
                 terminal pinch limit"
            };
            return Err(CrossRepairError::DidNotConverge(format!(
                "no admissible steady duty: at the bracket ceiling of \
                 {high:.6e} W the zones demand only \
                 {:.6e} W/K against the installed {ua_total:.6e} W/K -- {reason}",
                at_high.ua_demanded_w_per_k
            )));
        }
    }

    // 80 halvings take a bracket of order 1e8 W below 1e-15 W; the loop exits on
    // the width test long before that in practice.
    for _ in 0..80 {
        let middle = 0.5 * (low + high);
        match zones_at_duty(context, middle, c_hot_w_per_k)? {
            Some(trial) if trial.ua_demanded_w_per_k <= ua_total => low = middle,
            _ => high = middle,
        }
        if high - low <= 1.0e-12 * context.q_bracket_upper_w {
            break;
        }
    }

    zones_at_duty(context, low, c_hot_w_per_k)?.ok_or_else(|| {
        CrossRepairError::DidNotConverge(format!(
            "the bisected duty {low:.6e} W is not feasible, which should be \
             impossible: zero duty is always feasible and the bracket only ever \
             moves the feasible end upward"
        ))
    })
}

/// Rebuild the three node temperature profiles from the solved zone structure.
///
/// Node `i`'s centre sits at the fractional station `xi = (i + 1/2)/n` measured
/// from the hot-inlet end, and each zone occupies the share of the exchanger
/// equal to its share of the total conductance. Within a zone the steady
/// counter-flow solution is exponential in accumulated conductance, integrated
/// here in closed form.
///
/// The cold node temperature comes from an IF97 `(p,h)` flash of the local cold
/// enthalpy, not from the zone's linearised `c_p`, so the boiling plateau
/// appears at the right temperature without being imposed. The metal is the
/// steady conductance-weighted mean of the two fluid temperatures.
///
/// Returns `(hot, cold, metal)` profiles, all hot-inlet-first as the contract
/// requires.
fn rebuild_profiles(
    inputs: &CrossRepairInputs,
    context: &SolveContext,
    zones: &ZoneSet,
    c_hot_w_per_k: f64,
    node_count: usize,
) -> Result<
    (
        Vec<ThermodynamicTemperature>,
        Vec<ThermodynamicTemperature>,
        Vec<ThermodynamicTemperature>,
    ),
    CrossRepairError,
> {
    let ua_hot = inputs.hot_node_conductance.get::<watt_per_kelvin>();
    let ua_cold = inputs.cold_node_conductance.get::<watt_per_kelvin>();
    let ua_total = context.ua_total_w_per_k;
    let cold_pressure = context.cold_pressure();

    // Fractional station of each zone's high-enthalpy (hot-inlet-side) end.
    let mut zone_start_xi = [0.0_f64; 3];
    let mut accumulated = 0.0_f64;
    for (index, zone) in zones.live().iter().enumerate() {
        zone_start_xi[index] = accumulated / ua_total;
        accumulated += zone.ua_w_per_k;
    }

    let mut hot = Vec::with_capacity(node_count);
    let mut cold = Vec::with_capacity(node_count);
    let mut metal = Vec::with_capacity(node_count);

    for index in 0..node_count {
        let xi = (index as f64 + 0.5) / node_count as f64;

        // Accumulated duty from the hot inlet to this station.
        let q_at_xi = if zones.count == 0 {
            0.0
        } else {
            // Last zone owns everything past its start, so round-off at xi = 1
            // cannot fall off the end.
            let mut chosen = zones.count - 1;
            for candidate in 0..zones.count {
                let next_start = if candidate + 1 < zones.count {
                    zone_start_xi[candidate + 1]
                } else {
                    f64::INFINITY
                };
                if xi < next_start {
                    chosen = candidate;
                    break;
                }
            }
            let zone = zones.live()[chosen];
            let delta_xi = xi - zone_start_xi[chosen];
            let delta_hi = zone.t_hot_hi_kelvin - zone.t_cold_hi_kelvin;
            let inverse_c_cold = zone.c_cold_w_per_k.map_or(0.0, |c| 1.0 / c);
            let kappa = ua_total * (1.0 / c_hot_w_per_k - inverse_c_cold);
            let integral = if kappa.abs() * delta_xi < 1.0e-12 {
                ua_total * delta_hi * delta_xi
            } else {
                ua_total * delta_hi * (1.0 - (-kappa * delta_xi).exp()) / kappa
            };
            zone.q_at_hi_end_w + integral
        };

        let t_hot = context.t_hot_inlet_kelvin - q_at_xi / c_hot_w_per_k;
        let h_cold = context.h_cold_inlet_j_per_kg
            + (zones.q_total_w - q_at_xi) / context.cold_mass_flow_kg_per_s;
        let t_cold = cold_temperature_kelvin(cold_pressure, h_cold)?;
        let t_metal = (ua_hot * t_hot + ua_cold * t_cold) / (ua_hot + ua_cold);

        if !t_hot.is_finite() || !t_cold.is_finite() || !t_metal.is_finite() {
            return Err(CrossRepairError::DidNotConverge(format!(
                "rebuilt node {index} is not finite: hot {t_hot} K, cold \
                 {t_cold} K, metal {t_metal} K"
            )));
        }

        hot.push(ThermodynamicTemperature::new::<kelvin>(t_hot));
        cold.push(ThermodynamicTemperature::new::<kelvin>(t_cold));
        metal.push(ThermodynamicTemperature::new::<kelvin>(t_metal));
    }

    Ok((hot, cold, metal))
}

/// Specific enthalpy \[J/kg\] used for *energy auditing only*, from a cold-side
/// node temperature.
///
/// Within [`SATURATION_AUDIT_BAND_KELVIN`] of `T_sat` the temperature does not
/// determine the enthalpy -- steam quality is a free variable and IF97 rejects
/// the `(T,p)` flash outright there -- so a fixed datum of `h_f` is returned.
/// The **same** map is applied to the pre-repair and post-repair profiles, so a
/// node that stays inside the band contributes zero to the audit rather than a
/// latent-heat-sized artefact of a rounding difference.
fn cold_audit_enthalpy_j_per_kg(
    t_kelvin: f64,
    pressure: Pressure,
    saturation: ColdSaturation,
) -> Result<f64, CrossRepairError> {
    if (t_kelvin - saturation.t_sat_kelvin).abs() <= SATURATION_AUDIT_BAND_KELVIN {
        return Ok(saturation.h_f_j_per_kg);
    }
    try_h_tp_eqm_single_phase(ThermodynamicTemperature::new::<kelvin>(t_kelvin), pressure)
        .map(|h| h.get::<joule_per_kilogram>())
        .map_err(|e| {
            CrossRepairError::DidNotConverge(format!(
                "IF97 (T,p) flash failed while auditing a cold node at \
                 {t_kelvin:.3} K, {:.6e} Pa: {e}",
                pressure.get::<pascal>()
            ))
        })
}

/// First-law bookkeeping for the profile overwrite \[J\]. Positive means the
/// repair added energy to the exchanger.
///
/// # What this measures, exactly
///
/// The mean specific enthalpy of each stream over its nodes, after the repair
/// minus before, times that stream's mass flow, times
/// [`AUDIT_REFERENCE_RESIDENCE_TIME_S`]:
///
/// ```text
/// E = tau * [ m_hot * (h_hot,after - h_hot,before) + m_cold * (h_cold,after - h_cold,before) ]
/// ```
///
/// The hot stream's enthalpies come from the CoolProp-derived helium EOS, the
/// cold stream's from IF97 through [`cold_audit_enthalpy_j_per_kg`].
///
/// # What it does NOT measure, and why not
///
/// It is **not** the true stored-energy jump, and it cannot be, for two
/// independent reasons that are both properties of
/// [`super::CrossRepairInputs`] rather than of this method:
///
/// 1. **There is no inventory in the contract.** No node mass, no node volume,
///    no timestep -- so no quantity with the dimensions of energy can be formed
///    from the inputs at all. The reference residence time supplies the missing
///    scale as a *stated assumption* (each stream's inventory equals one second
///    of its own flow), and the result scales exactly linearly with it.
/// 2. **The pre-repair cold profile is temperatures only, and the cold stream
///    boils.** A two-phase node's enthalpy is not recoverable from its
///    temperature, so the latent-heat content of the old profile is genuinely
///    unknown. Nodes inside the saturation band therefore contribute nothing:
///    a node that went from 10% to 90% quality registers as no change, and the
///    magnitude reported here is a **lower bound** on the cold side's true
///    contribution. The hot side, being single-phase, is exact up to the
///    inventory assumption.
///
/// Both would be fixed by carrying the cold *enthalpy* profile and a node
/// inventory (or the coupling timestep) in `CrossRepairInputs`. Until then this
/// number is an auditable, correctly-signed, exactly-rescalable proxy -- which
/// is worth more than a zero, and less than a measurement.
fn audit_energy_discrepancy(
    inputs: &CrossRepairInputs,
    hot_after: &[ThermodynamicTemperature],
    cold_after: &[ThermodynamicTemperature],
    saturation: ColdSaturation,
) -> Result<Energy, CrossRepairError> {
    let node_count = inputs.hot_temperatures.len() as f64;
    let hot_pressure_pa = inputs.hot_pressure.get::<pascal>();
    let cold_pressure = inputs.cold_pressure;

    let mut hot_change = 0.0_f64;
    for (before, after) in inputs.hot_temperatures.iter().zip(hot_after.iter()) {
        let t_before = before.get::<kelvin>();
        let t_after = after.get::<kelvin>();
        let delta = match (
            hot_specific_enthalpy_j_per_kg(t_before, hot_pressure_pa),
            hot_specific_enthalpy_j_per_kg(t_after, hot_pressure_pa),
        ) {
            (Some(h_before), Some(h_after)) => h_after - h_before,
            // Ideal-gas fallback on the same terms as the c_p fallback: a
            // physical bound, not an invented number.
            _ => HELIUM_IDEAL_GAS_CP_J_PER_KG_K * (t_after - t_before),
        };
        hot_change += delta;
    }
    hot_change /= node_count;

    let mut cold_change = 0.0_f64;
    for (before, after) in inputs.cold_temperatures.iter().zip(cold_after.iter()) {
        let h_before =
            cold_audit_enthalpy_j_per_kg(before.get::<kelvin>(), cold_pressure, saturation)?;
        let h_after =
            cold_audit_enthalpy_j_per_kg(after.get::<kelvin>(), cold_pressure, saturation)?;
        cold_change += h_after - h_before;
    }
    cold_change /= node_count;

    let joules = AUDIT_REFERENCE_RESIDENCE_TIME_S
        * (inputs.hot_mass_flow.get::<kilogram_per_second>() * hot_change
            + inputs.cold_mass_flow.get::<kilogram_per_second>() * cold_change);

    Ok(Energy::new::<joule>(joules))
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_park_fork_dwsim_libs::heat_exchanger::ntu_effectiveness;
    use uom::si::area::square_meter;
    use uom::si::heat_transfer::watt_per_square_meter_kelvin;
    use uom::si::power::watt;
    use uom::si::ratio::ratio;
    use uom::si::temperature_interval::kelvin as kelvin_interval_unit;
    use uom::si::thermal_conductance::watt_per_kelvin as watt_per_kelvin_unit;

    /// Kelvin shorthand.
    fn k(value: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(value)
    }

    /// Watts-per-kelvin shorthand.
    fn wk(value: f64) -> ThermalConductance {
        ThermalConductance::new::<watt_per_kelvin_unit>(value)
    }

    /// The same three-node crossed fixture the parent module's tests use: the
    /// middle node has the cold stream 5 K above the hot.
    ///
    /// Reproduced rather than shared because the parent's helper is private to
    /// its own test module. If the parent's numbers change, this one does not
    /// follow automatically -- it is a fixture, not a coupling.
    fn crossed_inputs() -> CrossRepairInputs {
        CrossRepairInputs {
            hot_temperatures: vec![k(900.0), k(700.0), k(600.0)],
            metal_temperatures: vec![k(800.0), k(690.0), k(580.0)],
            cold_temperatures: vec![k(690.0), k(705.0), k(500.0)],
            hot_node_conductance: wk(1.0e4),
            cold_node_conductance: wk(3.0e4),
            hot_mass_flow: MassRate::new::<kilogram_per_second>(4.3),
            cold_mass_flow: MassRate::new::<kilogram_per_second>(3.2),
            hot_inlet_temperature: k(973.15),
            cold_inlet_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(4.0e5),
            hot_pressure: Pressure::new::<pascal>(3.0e6),
            cold_pressure: Pressure::new::<pascal>(4.0e6),
        }
    }

    /// An eight-node exchanger at the HTR-10 illustrative design point, with
    /// the plant's own numbers: helium 4.3 kg/s at 3 MPa entering at 973.15 K,
    /// feedwater 3.4722 kg/s at 4 MPa entering at 104 degC, and the series
    /// conductance `super::super::primary_loop::STEAM_GENERATOR_UA_W_PER_K`
    /// (4.26e4 W/K) split 0.75/0.25 between the hot and cold films over 8
    /// nodes -- the contract's conductances are per node, so each is the
    /// plant's total divided by the node count.
    ///
    /// The supplied profiles are deliberately nonsense (a flat 700 K cold
    /// stream against a falling hot stream) so that most nodes are crossed --
    /// the repair must recover a sane profile from a badly wrong one, which is
    /// the situation the remedy exists for.
    fn htr10_like_crossed_inputs() -> CrossRepairInputs {
        let nodes = 8;
        let ua_total = 4.26e4_f64;
        let hot_fraction = 0.75_f64;
        let hot: Vec<ThermodynamicTemperature> =
            (0..nodes).map(|i| k(973.15 - 50.0 * i as f64)).collect();
        let cold: Vec<ThermodynamicTemperature> = (0..nodes).map(|_| k(700.0)).collect();
        let metal: Vec<ThermodynamicTemperature> =
            (0..nodes).map(|i| k(850.0 - 25.0 * i as f64)).collect();
        CrossRepairInputs {
            hot_temperatures: hot,
            metal_temperatures: metal,
            cold_temperatures: cold,
            hot_node_conductance: wk(ua_total / hot_fraction / nodes as f64),
            cold_node_conductance: wk(ua_total / (1.0 - hot_fraction) / nodes as f64),
            hot_mass_flow: MassRate::new::<kilogram_per_second>(4.3),
            cold_mass_flow: MassRate::new::<kilogram_per_second>(3.4722),
            hot_inlet_temperature: k(973.15),
            // Feedwater at 104 degC, 4 MPa.
            cold_inlet_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(
                try_h_tp_eqm_single_phase(k(377.15), Pressure::new::<pascal>(4.0e6))
                    .unwrap()
                    .get::<joule_per_kilogram>(),
            ),
            hot_pressure: Pressure::new::<pascal>(3.0e6),
            cold_pressure: Pressure::new::<pascal>(4.0e6),
        }
    }

    /// Methodology: apply the remedy to the parent module's three-node fixture,
    /// whose middle node is crossed by 5.0 K, and require the returned profile
    /// to contain no cross at all (`worst_cross_kelvin <= 1e-9 K`, i.e.
    /// round-off). This is the remedy's non-negotiable: a profile that still
    /// violates the second law must never be returned as a success.
    ///
    /// # Results (2026-08-13)
    ///
    /// Input worst cross **5.000 K**; output worst cross **0.000 K** (the
    /// measure is floored at zero, so this means "no node crossed"), with a
    /// **closest approach of 264.970 K** -- the repaired profile is nowhere
    /// near a pinch. Solved duty **6.926880 MW** against an installed
    /// conductance of **2.2500e4 W/K** and a hot capacity rate of
    /// **2.2323e4 W/K**.
    ///
    /// Zone structure **two-phase + subcooled**: at this fixture's conductance
    /// the exchanger never dries the steam out, so there is no superheat zone
    /// and the method declines to invent one. Node temperatures \[K\]:
    ///
    /// | node | hot | metal | cold |
    /// |---|---|---|---|
    /// | 0 | 903.62 | 618.54 | 523.51 |
    /// | 1 | 795.15 | 591.42 | 523.51 |
    /// | 2 | 710.07 | 511.34 | 445.10 |
    ///
    /// Cold nodes 0 and 1 sit on `T_sat(4 MPa) = 523.51 K`, i.e. the fixture's
    /// steam generator is still boiling at its outlet. Energy audit
    /// **-3.368029e6 J** at the stated one-second inventory assumption.
    ///
    /// Interpretation: the remedy does what it claims on the fixture the whole
    /// module is keyed to -- it converts a second-law-violating profile into an
    /// admissible one -- and the zone structure it finds is a consequence of
    /// the inputs rather than an assumption.
    #[test]
    fn the_lmtd_remedy_removes_the_cross_on_the_shared_fixture() {
        let inputs = crossed_inputs();
        assert!(
            inputs.worst_cross_kelvin() > 4.9,
            "fixture should be crossed; got {}",
            inputs.worst_cross_kelvin()
        );

        let outcome = repair(&inputs).expect("the three-node fixture must repair");
        let worst = worst_cross_kelvin(&outcome.hot_temperatures, &outcome.cold_temperatures);
        assert!(
            worst <= CROSS_TOLERANCE_KELVIN,
            "repaired profile still crosses by {worst} K"
        );

        let node_count = validate(&inputs).unwrap();
        let saturation = ColdSaturation::at(inputs.cold_pressure).unwrap();
        let context = SolveContext::build(&inputs, saturation, node_count).unwrap();
        let (zones, c_hot) = solve_duty(&context).unwrap();
        println!(
            "  zones = {}, Q = {:.6e} W, C_hot = {:.4e} W/K, UA_total = {:.4e} W/K",
            zones.describe(),
            zones.q_total_w,
            c_hot,
            context.ua_total_w_per_k
        );

        let approach = outcome
            .hot_temperatures
            .iter()
            .zip(outcome.cold_temperatures.iter())
            .fold(f64::INFINITY, |closest, (h, c)| {
                closest.min(h.get::<kelvin>() - c.get::<kelvin>())
            });
        println!(
            "3-node fixture: worst cross in {:.3} K, out {:.3} K; closest \
             approach {approach:.3} K",
            inputs.worst_cross_kelvin(),
            worst
        );
        for index in 0..outcome.hot_temperatures.len() {
            println!(
                "  node {index}: hot {:.2} K, metal {:.2} K, cold {:.2} K",
                outcome.hot_temperatures[index].get::<kelvin>(),
                outcome.metal_temperatures[index].get::<kelvin>(),
                outcome.cold_temperatures[index].get::<kelvin>()
            );
        }
        println!(
            "  energy discrepancy {:.6e} J",
            outcome.energy_discrepancy.get::<joule>()
        );
    }

    /// Methodology: apply the remedy to the eight-node HTR-10-like fixture,
    /// whose cold profile is a flat 700 K against a falling helium profile so
    /// that six of eight nodes are crossed. Require a cross-free output, both
    /// fluid profiles monotone in the physically required direction, and both
    /// inlet boundary conditions respected (no node hotter than the hot inlet,
    /// no cold node colder than the feedwater inlet).
    ///
    /// # Results (2026-08-13)
    ///
    /// Input worst cross **76.850 K**; output worst cross **0.000 K**. Solved
    /// duty **9.355218 MW**, zone structure
    /// **superheat + two-phase + subcooled** -- all three zones resolved, the
    /// superheater taking **7.4%** of the conductance, the evaporator
    /// **58.0%** and the economiser **34.5%**.
    ///
    /// Rebuilt node temperatures \[K\], hot inlet first:
    ///
    /// ```text
    /// hot  930.24  844.09  776.06  722.46  680.23  646.59  612.54  574.62
    /// cold 538.97  523.51  523.51  523.51  523.51  512.31  464.50  408.34
    /// ```
    ///
    /// Energy audit **-8.871154e6 J**.
    ///
    /// **The steady state this imposes is not the plant's operating point,
    /// even at the plant's own boundary conditions.** The zone terminals put
    /// the steam outlet at **639.31 K (366.2 degC)** and the helium return at
    /// **554.07 K (280.9 degC)**, against the published HTR-10 values of
    /// 440 degC and 250 degC -- so the duty is **6.4% below** the published
    /// 10 MWth, the steam is **73.8 K cold** and the helium return **30.9 K
    /// hot**. That is not a defect in the zoning; it is what a fixed-boundary
    /// steady solve gives for a conductance
    /// (`primary_loop::STEAM_GENERATOR_UA_W_PER_K`) that was calibrated
    /// against a *coupled transient with feedwater-flow feedback*. Engaging
    /// this remedy will therefore visibly move the plant state by tens of
    /// kelvin, which is a reason to count and surface every event.
    ///
    /// Note also the half-cell offset: the exchanger's terminal (face) states
    /// are 639.31 K steam and 554.07 K helium, while the outermost *cell
    /// centres* -- which is what the contract asks for and what the arrays
    /// store -- are 538.97 K and 574.62 K. The gap is large on the steam side
    /// because with 8 nodes the superheat zone occupies only about 0.6 of a
    /// cell, so this configuration barely resolves the superheater at all.
    ///
    /// Interpretation: on a realistic configuration the zoning recovers the
    /// three-zone structure a once-through steam generator must have and the
    /// profile is admissible everywhere, but the imposed steady state is a
    /// materially different plant state from the one the transient was in.
    #[test]
    fn the_lmtd_remedy_recovers_a_three_zone_profile_on_the_htr10_fixture() {
        let inputs = htr10_like_crossed_inputs();
        let before = inputs.worst_cross_kelvin();
        assert!(
            before > 50.0,
            "fixture should be badly crossed; got {before}"
        );

        let outcome = repair(&inputs).expect("the HTR-10-like fixture must repair");
        let after = worst_cross_kelvin(&outcome.hot_temperatures, &outcome.cold_temperatures);
        assert!(
            after <= CROSS_TOLERANCE_KELVIN,
            "repaired profile still crosses by {after} K"
        );

        let hot: Vec<f64> = outcome
            .hot_temperatures
            .iter()
            .map(|t| t.get::<kelvin>())
            .collect();
        let cold: Vec<f64> = outcome
            .cold_temperatures
            .iter()
            .map(|t| t.get::<kelvin>())
            .collect();
        let metal: Vec<f64> = outcome
            .metal_temperatures
            .iter()
            .map(|t| t.get::<kelvin>())
            .collect();

        for index in 1..hot.len() {
            assert!(
                hot[index] < hot[index - 1],
                "hot stream must cool along the flow: {hot:?}"
            );
            assert!(
                cold[index] <= cold[index - 1] + 1.0e-9,
                "cold stream must be coldest at the feedwater end: {cold:?}"
            );
        }
        for index in 0..hot.len() {
            assert!(
                hot[index] <= inputs.hot_inlet_temperature.get::<kelvin>() + 1.0e-9,
                "no hot node may exceed the hot inlet"
            );
            assert!(
                metal[index] < hot[index] && metal[index] > cold[index],
                "metal must sit between the two fluids at node {index}"
            );
        }

        println!("8-node HTR-10-like: worst cross in {before:.3} K, out {after:.3} K");
        println!("  hot  {hot:?}");
        println!("  cold {cold:?}");
        println!(
            "  steam outlet {:.2} K, helium outlet {:.2} K, energy discrepancy {:.6e} J",
            cold[0],
            hot[hot.len() - 1],
            outcome.energy_discrepancy.get::<joule>()
        );
    }

    /// Methodology: the load-bearing justification for using LMTD rather than
    /// effectiveness-NTU inside the evaporator. With one stream isothermal the
    /// capacity-rate ratio is zero, and the two methods are then algebraically
    /// the same relation. Take `UA = 5.0e4 W/K`, `C_hot = 2.2e4 W/K`, hot
    /// entering the zone at 800 K against `T_sat = 523.5 K`; compute the duty
    /// twice --
    ///
    /// - by DWSIM's effectiveness-NTU
    ///   ([`ntu_effectiveness::evaluate`]) with the cold capacity rate set to
    ///   `1.0e16 W/K` so `C_r -> 0`, and
    /// - by DWSIM's LMTD ([`dwsim_lmtd`] then
    ///   [`outram_park_fork_dwsim_libs::heat_exchanger::lmtd::duty`]) on the
    ///   hot outlet that duty implies.
    ///
    /// Pass criterion: the two duties agree to a relative `1e-9`, and the
    /// counter-current and co-current effectiveness agree to the same tolerance
    /// (the arrangement-independence that makes `F = 1`).
    ///
    /// # Results (2026-08-13)
    ///
    /// Effectiveness-NTU duty **5.456264e6 W**, LMTD duty **5.456264e6 W**,
    /// relative difference **1.332e-12**. Counter-current and co-current
    /// effectiveness differed by **1.278e-12** relative -- both residuals are
    /// the finite `C_r = 2.2e-12` of the stand-in cold capacity rate, not a
    /// disagreement between the methods. Interpretation: inside
    /// the two-phase zone the choice between the two methods is not a modelling
    /// decision at all -- they are one equation written two ways -- so using
    /// LMTD there costs nothing in accuracy, and the reason to prefer it is
    /// purely that the unknown in this formulation is the conductance a known
    /// duty consumes, which LMTD returns by division rather than by inverting
    /// an exponential.
    #[test]
    fn two_phase_lmtd_and_effectiveness_ntu_are_the_same_equation() {
        let ua = 5.0e4_f64;
        let c_hot = 2.2e4_f64;
        let t_hot_in = 800.0_f64;
        let t_sat = 523.5_f64;

        let counter = ntu_effectiveness::evaluate(
            FlowArrangement::CounterCurrent,
            wk(ua),
            wk(c_hot),
            wk(1.0e16),
            TemperatureInterval::new::<kelvin_interval_unit>(t_hot_in - t_sat),
        );
        let co = ntu_effectiveness::evaluate(
            FlowArrangement::CoCurrent,
            wk(ua),
            wk(c_hot),
            wk(1.0e16),
            TemperatureInterval::new::<kelvin_interval_unit>(t_hot_in - t_sat),
        );
        let arrangement_difference =
            (counter.effectiveness.get::<ratio>() - co.effectiveness.get::<ratio>()).abs()
                / counter.effectiveness.get::<ratio>();
        assert!(
            arrangement_difference < 1.0e-9,
            "at C_r = 0 effectiveness must not depend on arrangement; \
             relative difference {arrangement_difference:.3e}"
        );

        let q_ntu = counter.duty.get::<watt>();
        let t_hot_out = t_hot_in - q_ntu / c_hot;
        let mean_difference = zone_lmtd_kelvin(t_hot_in, t_hot_out, t_sat, t_sat);
        let q_lmtd = outram_park_fork_dwsim_libs::heat_exchanger::lmtd::duty(
            HeatTransfer::new::<watt_per_square_meter_kelvin>(ua),
            Area::new::<square_meter>(1.0),
            TemperatureInterval::new::<kelvin_interval_unit>(mean_difference),
            Ratio::new::<ratio>(1.0),
        )
        .get::<watt>();

        let relative = (q_lmtd - q_ntu).abs() / q_ntu;
        println!(
            "two-phase zone: Q(eps-NTU) = {q_ntu:.6e} W, Q(LMTD) = {q_lmtd:.6e} W, \
             relative difference {relative:.3e}, arrangement difference \
             {arrangement_difference:.3e}"
        );
        assert!(
            relative < 1.0e-9,
            "LMTD and eps-NTU must agree at C_r = 0; relative difference {relative:.3e}"
        );
    }

    /// Methodology: internal consistency of the solve. At the converged duty,
    /// the zone duties must sum to the total duty and the zones' demanded
    /// conductances must sum to the installed conductance -- the latter is the
    /// equation the bisection solves, so a residual here means the root find
    /// did not close. Pass criterion: both to a relative `1e-6` on the
    /// eight-node HTR-10-like fixture.
    ///
    /// # Results (2026-08-13)
    ///
    /// Duty residual **0.000e0** (exact -- the zone duties are constructed as a
    /// partition of the total). Conductance residual **1.540e-12** relative
    /// against the installed 4.26e4 W/K, i.e. the bisection closes to near
    /// round-off. Solved duty **9.355218 MW**, split as:
    ///
    /// | zone | duty \[MW\] | `UA` \[W/K\] | share | hot \[K\] | cold \[K\] |
    /// |---|---|---|---|---|---|
    /// | superheat | 1.1538 | 3.1614e3 | 7.4% | 973.15 -> 921.46 | 639.31 -> 523.50 |
    /// | two-phase | 5.9495 | 2.4727e4 | 58.0% | 921.46 -> 654.95 | 523.50 -> 523.48 |
    /// | subcooled | 2.2519 | 1.4711e4 | 34.5% | 654.95 -> 554.07 | 523.48 -> 377.15 |
    ///
    /// Interpretation: the conductance balance that locates the zone
    /// boundaries is genuinely satisfied, so the boundary positions are a
    /// solved result rather than an assumed split. Note how badly a single-zone
    /// LMTD would misrepresent this exchanger -- 58% of its conductance is
    /// spent boiling at constant temperature, which a single `c_p` cannot
    /// describe at all.
    #[test]
    fn zone_duties_and_conductances_close_on_their_balances() {
        let inputs = htr10_like_crossed_inputs();
        let node_count = validate(&inputs).unwrap();
        let saturation = ColdSaturation::at(inputs.cold_pressure).unwrap();
        let context = SolveContext::build(&inputs, saturation, node_count).unwrap();
        let (zones, _c_hot) = solve_duty(&context).unwrap();

        let duty_sum: f64 = zones.live().iter().map(|z| z.duty_w).sum();
        let duty_residual = (duty_sum - zones.q_total_w).abs() / zones.q_total_w;
        let conductance_residual =
            (zones.ua_demanded_w_per_k - context.ua_total_w_per_k).abs() / context.ua_total_w_per_k;

        println!(
            "zones = {}, Q = {:.6e} W, duty residual {duty_residual:.3e}, \
             UA residual {conductance_residual:.3e}",
            zones.describe(),
            zones.q_total_w
        );
        for zone in zones.live() {
            println!(
                "  {:<10} duty {:.4e} W, UA {:.4e} W/K ({:.1}% of total), \
                 hot {:.2} -> {:.2} K, cold {:.2} -> {:.2} K",
                zone.kind.label(),
                zone.duty_w,
                zone.ua_w_per_k,
                100.0 * zone.ua_w_per_k / context.ua_total_w_per_k,
                zone.t_hot_hi_kelvin,
                zone.t_hot_lo_kelvin,
                zone.t_cold_hi_kelvin,
                zone.t_cold_lo_kelvin
            );
        }

        assert!(duty_residual < 1.0e-6, "duty residual {duty_residual:.3e}");
        assert!(
            conductance_residual < 1.0e-6,
            "conductance residual {conductance_residual:.3e}"
        );
    }

    /// Methodology: the zone boundaries must be pinned to the IF97 saturation
    /// enthalpies, not fitted. Check that on the eight-node fixture the
    /// two-phase zone's two cold-side terminal temperatures are both the
    /// saturation temperature at 4 MPa. Reference: IAPWS-IF97 `T_sat(4.0 MPa)`,
    /// which `super::super::secondary_loop`'s own V&V test records as 523.50 K
    /// against the published 523.50 K. Pass criterion: within 0.05 K -- see the
    /// result below for why that is not 1e-6 K.
    ///
    /// # Results (2026-08-13)
    ///
    /// `T_sat(4 MPa) = 523.5075 K`. The two-phase zone's cold terminals came
    /// back as **523.4988 K** and **523.4841 K**, departing by **0.0087 K** and
    /// **0.0235 K**.
    ///
    /// That residual is an IF97 internal inconsistency, not a zoning error, and
    /// it is worth knowing about. The zone boundaries sit at exactly `h_f` and
    /// `h_g`, and at *exactly* those enthalpies the `(p,h)` region router
    /// classifies the state as Region 1 or Region 2 rather than Region 4, so
    /// the temperature comes from the backward equations `t_ph_1`/`t_ph_2`,
    /// which reproduce the Region 4 saturation line only to a few hundredths of
    /// a kelvin. Enthalpies *inside* the two-phase range do route to Region 4
    /// and return `sat_temp_4(p)` exactly -- which is why the rebuilt node
    /// temperatures in
    /// [`tests::the_lmtd_remedy_recovers_a_three_zone_profile_on_the_htr10_fixture`]
    /// read 523.5075 K to every digit.
    ///
    /// Interpretation: the boundary *enthalpies* are exact by construction and
    /// the boundary *temperatures* follow from a real flash to within 0.024 K,
    /// which is far below any temperature difference this method resolves. The
    /// only thing the solve has to find is where those fixed boundaries sit in
    /// space -- which is the claim the module documentation makes.
    #[test]
    fn the_two_phase_zone_sits_exactly_on_the_if97_saturation_line() {
        let inputs = htr10_like_crossed_inputs();
        let node_count = validate(&inputs).unwrap();
        let saturation = ColdSaturation::at(inputs.cold_pressure).unwrap();
        let context = SolveContext::build(&inputs, saturation, node_count).unwrap();
        let (zones, _c_hot) = solve_duty(&context).unwrap();

        let two_phase = zones
            .live()
            .iter()
            .find(|z| z.kind == ZoneKind::TwoPhase)
            .expect("the HTR-10-like fixture must have an evaporator");

        let departure_hi = (two_phase.t_cold_hi_kelvin - saturation.t_sat_kelvin).abs();
        let departure_lo = (two_phase.t_cold_lo_kelvin - saturation.t_sat_kelvin).abs();
        println!(
            "T_sat = {:.4} K; two-phase cold terminals {:.4} K and {:.4} K \
             (departures {departure_hi:.4} K and {departure_lo:.4} K)",
            saturation.t_sat_kelvin, two_phase.t_cold_hi_kelvin, two_phase.t_cold_lo_kelvin
        );
        assert!(
            departure_hi < 0.05,
            "hi terminal departs by {departure_hi} K"
        );
        assert!(
            departure_lo < 0.05,
            "lo terminal departs by {departure_lo} K"
        );
        assert!(
            two_phase.c_cold_w_per_k.is_none(),
            "the evaporator is isothermal"
        );
    }

    /// Methodology: a hot stream too cold to boil the feedwater must produce a
    /// single subcooled zone, not a fabricated evaporator. Feed the three-node
    /// fixture a 480 K hot inlet, below `T_sat(4 MPa) = 523.5 K`. Pass
    /// criterion: exactly one zone, of kind `Subcooled`, and a cross-free
    /// profile.
    ///
    /// # Results (2026-08-13)
    ///
    /// One zone, **subcooled**, duty **1.0680 MW**, cold outlet (node 0)
    /// **436.47 K** against `T_sat = 523.51 K` -- below saturation, as it must
    /// be, and the repaired profile is cross-free.
    /// Interpretation: the zone structure is discovered from the inputs. The
    /// method does not assume a steam generator is always boiling, which
    /// matters because the remedy will fire during exactly the low-power
    /// transients where it is not.
    #[test]
    fn a_hot_stream_below_saturation_yields_one_subcooled_zone() {
        let mut inputs = crossed_inputs();
        inputs.hot_inlet_temperature = k(480.0);
        inputs.hot_temperatures = vec![k(478.0), k(460.0), k(450.0)];
        inputs.cold_temperatures = vec![k(440.0), k(420.0), k(400.0)];
        inputs.metal_temperatures = vec![k(460.0), k(440.0), k(425.0)];

        let node_count = validate(&inputs).unwrap();
        let saturation = ColdSaturation::at(inputs.cold_pressure).unwrap();
        let context = SolveContext::build(&inputs, saturation, node_count).unwrap();
        let (zones, _c_hot) = solve_duty(&context).unwrap();

        println!(
            "sub-saturation case: zones = {}, Q = {:.4e} W",
            zones.describe(),
            zones.q_total_w
        );
        assert_eq!(
            zones.count,
            1,
            "expected one zone, got {}",
            zones.describe()
        );
        assert_eq!(zones.live()[0].kind, ZoneKind::Subcooled);

        let outcome = repair(&inputs).expect("a subcooled exchanger must repair");
        let cold_outlet = outcome.cold_temperatures[0].get::<kelvin>();
        println!(
            "  cold outlet {cold_outlet:.2} K against T_sat {:.2} K",
            saturation.t_sat_kelvin
        );
        assert!(cold_outlet < saturation.t_sat_kelvin);
        assert!(
            worst_cross_kelvin(&outcome.hot_temperatures, &outcome.cold_temperatures)
                <= CROSS_TOLERANCE_KELVIN
        );
    }

    /// Methodology: a cold side at or above the water critical pressure has no
    /// saturation line, so the subcooled / two-phase / superheat structure does
    /// not exist. The module's stated rule is that an undeterminable zone
    /// structure is refused rather than guessed. Set the cold pressure to
    /// 25 MPa (above `p_crit = 22.064 MPa`) and require
    /// [`CrossRepairError::DidNotConverge`].
    ///
    /// # Results (2026-08-13)
    ///
    /// `DidNotConverge("cold-side pressure 2.500000e7 Pa is at or above the
    /// water critical pressure: there is no saturation line, so the subcooled /
    /// two-phase / superheat zone structure is undefined")`. Interpretation:
    /// the method refuses the regime it cannot zone, instead of silently
    /// degrading to the single-zone LMTD that the design note calls the single
    /// biggest technical risk of this tier.
    #[test]
    fn a_supercritical_cold_side_is_refused_rather_than_zoned() {
        let mut inputs = crossed_inputs();
        inputs.cold_pressure = Pressure::new::<pascal>(2.5e7);
        match repair(&inputs) {
            Err(CrossRepairError::DidNotConverge(reason)) => {
                println!("supercritical refusal: {reason}");
                assert!(reason.contains("critical pressure"));
            }
            other => panic!("expected DidNotConverge, got {other:?}"),
        }
    }

    /// Methodology: unusable inputs must be rejected as
    /// [`CrossRepairError::BadInputs`] rather than producing a profile. Three
    /// cases: mismatched profile lengths, a zero cold mass flow, and an empty
    /// exchanger.
    ///
    /// # Results (2026-08-13)
    ///
    /// All three returned `BadInputs` with the offending quantity named:
    /// `"profile lengths differ: hot 3, metal 3, cold 2"`, `"cold mass flow
    /// [kg/s] = 0 is not finite and positive"`, and `"the exchanger has no
    /// nodes"`. Interpretation: the remedy cannot be handed a malformed
    /// exchanger and quietly return something plausible.
    #[test]
    fn unusable_inputs_are_rejected_as_bad_inputs() {
        let mut short = crossed_inputs();
        short.cold_temperatures.pop();
        match repair(&short) {
            Err(CrossRepairError::BadInputs(reason)) => println!("short profile: {reason}"),
            other => panic!("expected BadInputs, got {other:?}"),
        }

        let mut stalled = crossed_inputs();
        stalled.cold_mass_flow = MassRate::new::<kilogram_per_second>(0.0);
        match repair(&stalled) {
            Err(CrossRepairError::BadInputs(reason)) => println!("stalled flow: {reason}"),
            other => panic!("expected BadInputs, got {other:?}"),
        }

        let empty = CrossRepairInputs {
            hot_temperatures: Vec::new(),
            metal_temperatures: Vec::new(),
            cold_temperatures: Vec::new(),
            ..crossed_inputs()
        };
        match repair(&empty) {
            Err(CrossRepairError::BadInputs(reason)) => {
                println!("empty exchanger: {reason}");
            }
            other => panic!("expected BadInputs, got {other:?}"),
        }
    }

    /// Methodology: a hot inlet colder than the cold inlet gives no definite
    /// hot and cold stream, so the zoning has no meaning and the remedy must
    /// say so. Feed a 350 K hot inlet against feedwater whose IF97 temperature
    /// is warmer than that. Pass criterion: `DidNotConverge` naming the
    /// inversion.
    ///
    /// # Results (2026-08-13)
    ///
    /// `DidNotConverge("hot inlet 350.000 K is not hotter than the cold inlet
    /// 367.907 K, so there is no definite hot and cold stream to zone")` -- the
    /// 367.907 K being the IF97 temperature of the fixture's 4.0e5 J/kg
    /// feedwater at 4 MPa. Interpretation: the degenerate case is named rather
    /// than run backwards.
    #[test]
    fn an_inverted_exchanger_is_refused() {
        let mut inputs = crossed_inputs();
        inputs.hot_inlet_temperature = k(350.0);
        match repair(&inputs) {
            Err(CrossRepairError::DidNotConverge(reason)) => {
                println!("inverted exchanger: {reason}");
                assert!(reason.contains("not hotter"));
            }
            other => panic!("expected DidNotConverge, got {other:?}"),
        }
    }

    /// Methodology: the first-law audit must be a real computation, not a stub.
    /// Check three things on the eight-node fixture: the value is finite and
    /// non-zero; it scales exactly linearly with the stated inventory
    /// assumption (verified by recomputing at a doubled mass flow, which is the
    /// same linear scaling the reference residence time applies); and its sign
    /// agrees with the direction the profiles actually moved.
    ///
    /// # Results (2026-08-13)
    ///
    /// Discrepancy **-8.871154e6 J** on the eight-node fixture. The repair
    /// lowers the hot stream's node-mean by **74.80 K** and the cold stream's
    /// by **197.73 K** (the input cold profile being a fabricated flat 700 K,
    /// far above the admissible 408-539 K band), so energy is removed and the
    /// sign is negative. Doubling both mass flows scaled the figure to
    /// **-1.774231e7 J**, a factor of **2.000000**, confirming the exact linear
    /// dependence on the inventory assumption.
    ///
    /// Interpretation: the number is auditable and correctly signed, and a
    /// maintainer who learns the true node inventory can rescale it without
    /// re-running. It is **not** the true stored-energy jump -- see
    /// [`audit_energy_discrepancy`] for the two reasons that number is not
    /// constructible from the present contract.
    #[test]
    fn the_energy_audit_is_computed_and_scales_with_the_stated_inventory() {
        let inputs = htr10_like_crossed_inputs();
        let outcome = repair(&inputs).unwrap();
        let energy = outcome.energy_discrepancy.get::<joule>();
        assert!(energy.is_finite(), "audit must be finite");
        assert!(
            energy.abs() > 1.0,
            "audit must not be stubbed; got {energy} J"
        );

        let mean = |profile: &[ThermodynamicTemperature]| -> f64 {
            profile.iter().map(|t| t.get::<kelvin>()).sum::<f64>() / profile.len() as f64
        };
        let hot_mean_change = mean(&outcome.hot_temperatures) - mean(&inputs.hot_temperatures);
        let cold_mean_change = mean(&outcome.cold_temperatures) - mean(&inputs.cold_temperatures);
        println!("node-mean change: hot {hot_mean_change:+.2} K, cold {cold_mean_change:+.2} K");
        assert!(
            hot_mean_change < 0.0 && cold_mean_change < 0.0 && energy < 0.0,
            "both streams cooled, so the audit must be negative"
        );

        let mut doubled = htr10_like_crossed_inputs();
        doubled.hot_mass_flow = inputs.hot_mass_flow * 2.0;
        doubled.cold_mass_flow = inputs.cold_mass_flow * 2.0;
        let doubled_energy = audit_energy_discrepancy(
            &doubled,
            &outcome.hot_temperatures,
            &outcome.cold_temperatures,
            ColdSaturation::at(inputs.cold_pressure).unwrap(),
        )
        .unwrap()
        .get::<joule>();
        let scale = doubled_energy / energy;

        println!(
            "energy discrepancy {energy:.6e} J; at doubled flow \
             {doubled_energy:.6e} J (scale {scale:.6})"
        );
        assert!(
            (scale - 2.0).abs() < 1.0e-9,
            "the audit must scale linearly with the inventory assumption; got {scale}"
        );
    }

    /// Methodology: guard the counter-flow node ordering, the single easiest
    /// thing for a remedy to invert. The repaired hot profile must be hottest
    /// at index 0 (the hot inlet end) and the repaired cold profile must be
    /// hottest at index 0 too (the steam outlet, because the cold stream runs
    /// the other way and its inlet is the last index). Pass criterion: both
    /// hold on the eight-node fixture, and the cold outlet exceeds the cold
    /// inlet temperature.
    ///
    /// # Results (2026-08-13)
    ///
    /// Hot **930.24 K** at node 0 falling to **574.62 K** at node 7; cold
    /// **538.97 K** at node 0 falling to **408.34 K** at node 7, against a
    /// feedwater inlet of 377.15 K. Interpretation: the profile is oriented as the contract
    /// documents, so the repair is not silently writing a reversed exchanger
    /// back into the arrays.
    #[test]
    fn the_repaired_profile_keeps_the_counter_flow_orientation() {
        let inputs = htr10_like_crossed_inputs();
        let outcome = repair(&inputs).unwrap();
        let hot: Vec<f64> = outcome
            .hot_temperatures
            .iter()
            .map(|t| t.get::<kelvin>())
            .collect();
        let cold: Vec<f64> = outcome
            .cold_temperatures
            .iter()
            .map(|t| t.get::<kelvin>())
            .collect();
        let last = hot.len() - 1;

        println!(
            "hot {:.2} -> {:.2} K, cold {:.2} -> {:.2} K",
            hot[0], hot[last], cold[0], cold[last]
        );
        assert!(hot[0] > hot[last], "hot must fall from node 0");
        assert!(
            cold[0] > cold[last],
            "cold must be coldest at the last node"
        );

        let t_cold_inlet = cold_temperature_kelvin(
            inputs.cold_pressure,
            inputs.cold_inlet_enthalpy.get::<joule_per_kilogram>(),
        )
        .unwrap();
        assert!(
            cold[last] > t_cold_inlet,
            "the last cold node sits half a node inside the feedwater inlet, so \
             it must already be warmer than it"
        );
    }
}
