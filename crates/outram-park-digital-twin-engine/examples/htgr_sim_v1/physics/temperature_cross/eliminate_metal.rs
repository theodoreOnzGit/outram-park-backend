//! Eliminate the tube metal -- remedy for a steam-generator temperature cross.
//!
//! Selected by [`super::TemperatureCrossRemedy::EliminateMetal`]. See that
//! variant's documentation for what this method is and when it is the right
//! choice, and `docs/heat-exchanger-temperature-cross-fallback.md` (Tier 1) for
//! the design discussion behind all three remedies.
//!
//! # The derivation, in full, so it can be checked without leaving this file
//!
//! ## What the real coupling is
//!
//! `super::super::steam_generator` couples three arrays laterally. The tube
//! metal is a [`SolidColumn`] with **no advection**: at node `i` it carries only
//! a thermal capacitance and two conductances, one to the hot fluid node at the
//! same station and one to the cold fluid node at the same station (the cold
//! vector is reversed on the way across by the counter-flow index map). TUAS
//! assembles a lateral link as `Q = -H (T_node - T_lateral)`
//! (`one_d_solid_array_with_lateral_coupling/calculation.rs`), so the metal node
//! equation is
//!
//! ```text
//! C_m dT_m,i/dt = G_h (T_h,i - T_m,i) + G_c (T_c,i - T_m,i) + (axial conduction)
//! ```
//!
//! with, in units spelled out:
//!
//! | Symbol | Quantity | Unit |
//! |---|---|---|
//! | `C_m` | metal thermal capacity of one node | J/K |
//! | `G_h` | hot-film conductance per node ([`super::CrossRepairInputs::hot_node_conductance`]) | W/K |
//! | `G_c` | cold-film conductance per node ([`super::CrossRepairInputs::cold_node_conductance`]) | W/K |
//! | `T_h,i`, `T_m,i`, `T_c,i` | hot / metal / cold node temperature | K |
//!
//! **Axial conduction along the tube wall is dropped**, and that is measured
//! rather than assumed: see
//! [`tests::axial_conduction_in_the_tube_wall_is_negligible_against_the_films`],
//! which puts it at 1.704e-6 of the lateral conductance sum.
//!
//! ## Static condensation (the Schur complement)
//!
//! Write one station's three-equation block, hot / metal / cold:
//!
//! ```text
//! [ C_h d/dt + G_h        -G_h                 0            ] [T_h]
//! [ -G_h            C_m d/dt + G_h + G_c      -G_c          ] [T_m] = (advection + boundary terms)
//! [ 0                     -G_c          C_c d/dt + G_c      ] [T_c]
//! ```
//!
//! The metal row has no advective coupling to any other station, so with
//! `C_m d/dt` removed it becomes a **purely algebraic** row and can be
//! eliminated exactly -- this is the Schur complement
//! `S = A_ff - A_fm A_mm^{-1} A_mf` taken on the metal block. Setting
//! `dT_m,i/dt = 0`:
//!
//! ```text
//! (G_h + G_c) T_m,i = G_h T_h,i + G_c T_c,i
//!
//! T_m,i* = (G_h T_h,i + G_c T_c,i) / (G_h + G_c)          [quasi-steady metal]
//! ```
//!
//! Substituting `T_m,i*` back into the hot-side flux:
//!
//! ```text
//! q_i = G_h (T_h,i - T_m,i*)
//!     = G_h [ (G_h + G_c) T_h,i - G_h T_h,i - G_c T_c,i ] / (G_h + G_c)
//!     = G_h G_c (T_h,i - T_c,i) / (G_h + G_c)
//!     = G_series (T_h,i - T_c,i),     1/G_series = 1/G_h + 1/G_c
//! ```
//!
//! and into the cold-side flux gives **the same number**:
//!
//! ```text
//! G_c (T_m,i* - T_c,i) = G_c [ G_h T_h,i + G_c T_c,i - (G_h + G_c) T_c,i ] / (G_h + G_c)
//!                      = G_h G_c (T_h,i - T_c,i) / (G_h + G_c)
//! ```
//!
//! Equality of the two fluxes *is* the statement that the eliminated metal
//! stores nothing, and it is what the elimination buys: hot and cold now see
//! each other at the same iterate instead of through a node that lags both.
//! [`tests::the_eliminated_network_passes_identical_heat_on_both_sides`] pins
//! it, and
//! [`tests::the_quasi_steady_metal_is_the_zero_capacitance_limit_of_the_real_coupling`]
//! checks the closed form against a numerical integration of the metal ODE as
//! `steam_generator.rs` actually writes it.
//!
//! Two properties follow immediately and are worth stating because they are
//! what make this remedy safe on the metal:
//!
//! - `T_m,i*` is a **convex combination** of `T_h,i` and `T_c,i` (the weights
//!   `G_h/(G_h+G_c)` and `G_c/(G_h+G_c)` are positive and sum to 1), so the
//!   repaired metal can never sit outside its two neighbours -- no metal-side
//!   cross is creatable by this remedy, at any conductance ratio.
//! - `q_i` carries the sign of `T_h,i - T_c,i`, so where the streams *are*
//!   crossed the eliminated network transports heat cold-to-hot, which is what
//!   the second law requires. Nothing is clamped.
//!
//! # What this remedy cannot do -- read this before selecting it
//!
//! **Eliminating the metal does not clear a cross between the two fluid
//! streams, and cannot.** It rewrites `T_m` only; `T_h` and `T_c` are handed
//! back untouched, so [`super::CrossRepairInputs::worst_cross_kelvin`] on the
//! result is *identical* to its value on the input. Since a remedy is only ever
//! invoked on a crossed state, [`repair`] therefore returns
//! [`CrossRepairError::DidNotConverge`] every time it is called through the
//! [`super::TemperatureCrossRemedy::EliminateMetal`] dispatch -- by construction,
//! not by failing to converge in the usual sense.
//! [`tests::eliminating_the_metal_does_not_clear_a_cross_between_the_streams`]
//! asserts exactly this.
//!
//! That is not a defect in the implementation; it is a **mismatch between what
//! Tier 1 is and what the [`super::CrossRepairInputs`] contract expresses**. Tier 1
//! in the design note is a change to *how the next step is integrated* -- drop
//! the metal out of the coupling loop and couple the streams directly through
//! `G_series` -- whereas this contract is a *state repair* applied after the
//! fact. A state repair cannot remove a lag that has already been taken. The
//! design note's escalation ladder anticipates this: Tier 1 is followed by Tier
//! 2 precisely because "if a cross is *still* observed after Tier 1" is the
//! expected case for an already-crossed state.
//!
//! To get the benefit this remedy is actually for, `steam_generator.rs` would
//! have to register a **direct hot<->cold lateral link at `G_series`** in place
//! of the four metal links and set the metal from
//! [`quasi_steady_metal_temperature`] afterwards. That wiring is deliberately
//! **not** done here: this file owns the algebra and the accounting only.
//!
//! # The fidelity that is lost, quantified
//!
//! Unlike the two profile remedies, this one discards a **physically real
//! thermal inertia**. Measured 2026-08-13 from the plant's own geometry and
//! material database (see
//! [`tests::discarding_the_metal_capacitance_removes_a_measured_lag`] for the
//! method and the full table):
//!
//! | Quantity | Metal retained | Metal eliminated |
//! |---|---|---|
//! | Exchanger lag `C/UA_series` | 39.78 s | 0 s |
//! | Metal-node relaxation `C_node/(G_h+G_c)` | 7.46 s | 0 s |
//! | Cold-side flux after a +100 K hot-stream step | 63.2% of final at 7.46 s | 100% at t = 0 |
//! | Heat delivered to the steam side early over that step | 31.78 MJ (3.28 s of the 9.685 MW rated duty) | -- |
//!
//! **This is a large change, and it should not be described as cheap.** The
//! exchanger's dominant filter between the primary loop and the turbine inlet
//! *is* the metal -- `steam_generator::tests::a_duty_step_is_filtered_by_the_metal_time_constant`
//! measured the steam outlet tracking only **0.0248%** of a 100 K hot-inlet step
//! in one 0.1 s plant timestep. Remove the capacitance and that filter is gone:
//! the steam outlet responds to a primary transient with only the two streams'
//! transport delays in the way. Anything computed with this remedy engaged is
//! **not** a resolved transient (per
//! `docs/heat-exchanger-temperature-cross-fallback.md`, V&V framing), and here
//! it is not even the same dynamical system.
//!
//! [`SolidColumn`]: tuas_boussinesq_solver::pre_built_components::one_d_solid_structure

use super::super::steam_generator::SteamGeneratorGeometry;
use super::{CrossRepairError, CrossRepairInputs, CrossRepairOutcome};

use tuas_boussinesq_solver::boussinesq_thermophysical_properties::SolidMaterial;

use uom::si::energy::joule;
use uom::si::f64::{Energy, HeatCapacity, ThermalConductance, ThermodynamicTemperature};
use uom::si::heat_capacity::joule_per_kelvin;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

/// Lowest metal temperature \[K\] at which the tube-wall property correlations
/// may be evaluated.
///
/// `SolidMaterial::SteelSS304LHighTemp` (Kim, ANL-75-55) is tabulated over
/// 300-1700 K and TUAS **panics** rather than extrapolating outside it, so
/// [`assumed_metal_node_thermal_capacity`] refuses out-of-range input rather
/// than letting a property lookup take the process down.
// Dead until `TemperatureCrossRemedy::apply` has a caller; exercised by the tests.
#[allow(dead_code)]
const METAL_PROPERTY_MIN_K: f64 = 300.0;

/// Highest metal temperature \[K\] at which the tube-wall property correlations
/// may be evaluated. See [`METAL_PROPERTY_MIN_K`].
// Dead until `TemperatureCrossRemedy::apply` has a caller; exercised by the tests.
#[allow(dead_code)]
const METAL_PROPERTY_MAX_K: f64 = 1700.0;

/// Series (metal-eliminated) conductance of one station \[W/K\]:
/// `1/G_series = 1/G_h + 1/G_c`, equivalently `G_series = G_h G_c / (G_h + G_c)`.
///
/// This is the conductance the hot and cold nodes see each other through once
/// the metal row has been condensed out -- the two films as resistances in
/// series, with the wall's own conduction resistance already folded into them by
/// whoever calibrated `G_h` and `G_c` (this exchanger's conductances are a
/// calibration, not a correlation; see `steam_generator`'s module docs).
///
/// # Arguments
///
/// - `hot_node_conductance` -- hot-film conductance of one node \[W/K\], strictly
///   positive.
/// - `cold_node_conductance` -- cold-film conductance of one node \[W/K\],
///   strictly positive.
///
/// Both must be strictly positive and finite; the caller
/// ([`repair_with_metal_capacity`]) checks that before calling, and a zero
/// conductance here would return zero rather than dividing by zero.
// Dead until `TemperatureCrossRemedy::apply` has a caller; exercised by the tests.
#[allow(dead_code)]
pub fn series_node_conductance(
    hot_node_conductance: ThermalConductance,
    cold_node_conductance: ThermalConductance,
) -> ThermalConductance {
    let g_h = hot_node_conductance.get::<watt_per_kelvin>();
    let g_c = cold_node_conductance.get::<watt_per_kelvin>();
    let sum = g_h + g_c;
    if sum <= 0.0 {
        return ThermalConductance::new::<watt_per_kelvin>(0.0);
    }
    ThermalConductance::new::<watt_per_kelvin>(g_h * g_c / sum)
}

/// Quasi-steady tube-metal temperature at one station \[K\]:
/// `T_m* = (G_h T_h + G_c T_c) / (G_h + G_c)`.
///
/// This is the value the metal node's own energy balance implies once its
/// capacitance is removed -- the conductance-weighted mean of the two fluid
/// temperatures it sits between. See the module documentation for the
/// derivation.
///
/// # Arguments
///
/// - `hot_node_conductance` -- hot-film conductance of one node \[W/K\].
/// - `cold_node_conductance` -- cold-film conductance of one node \[W/K\].
/// - `hot_temperature` -- hot-stream temperature at this station \[K\].
/// - `cold_temperature` -- cold-stream temperature at this station \[K\].
///
/// # Guarantees
///
/// The result is a convex combination of the two fluid temperatures, so it
/// always lies within `[min(T_h, T_c), max(T_h, T_c)]` inclusive -- including
/// when the streams are crossed, where it lies between them the other way
/// round. It therefore cannot itself introduce a metal-side temperature cross.
// Dead until `TemperatureCrossRemedy::apply` has a caller; exercised by the tests.
#[allow(dead_code)]
pub fn quasi_steady_metal_temperature(
    hot_node_conductance: ThermalConductance,
    cold_node_conductance: ThermalConductance,
    hot_temperature: ThermodynamicTemperature,
    cold_temperature: ThermodynamicTemperature,
) -> ThermodynamicTemperature {
    let g_h = hot_node_conductance.get::<watt_per_kelvin>();
    let g_c = cold_node_conductance.get::<watt_per_kelvin>();
    let sum = g_h + g_c;
    if sum <= 0.0 {
        // No coupling at all: the metal is thermally isolated and its
        // quasi-steady value is undefined. Returning the hot-side temperature
        // would be a fabrication, so return the arithmetic mean of the two
        // neighbours, which is the limit of the weighted mean as both weights
        // go to zero together. `repair_with_metal_capacity` rejects
        // non-positive conductances before this can be reached.
        return ThermodynamicTemperature::new::<kelvin>(
            0.5 * (hot_temperature.get::<kelvin>() + cold_temperature.get::<kelvin>()),
        );
    }
    ThermodynamicTemperature::new::<kelvin>(
        (g_h * hot_temperature.get::<kelvin>() + g_c * cold_temperature.get::<kelvin>()) / sum,
    )
}

/// Per-node tube-metal thermal capacity \[J/K\] to charge the discarded stored
/// energy against, when the caller has not supplied one.
///
/// # Why this function has to exist, and what is wrong with it
///
/// [`super::CrossRepairInputs`] carries **no metal thermal capacity and no
/// timestep**, so the energy this remedy discards -- which is *the* first-law
/// cost of eliminating the metal -- is not computable from the contract alone.
/// Stubbing it to zero would hide the single most important number about this
/// remedy, so instead this reconstructs the capacity from the exchanger
/// `htgr_sim_v1` actually builds:
/// [`SteamGeneratorGeometry::htr10_illustrative`] in
/// [`SolidMaterial::SteelSS304LHighTemp`], the same geometry and material
/// `primary_loop::steam_generator_config` passes to the steam generator, divided
/// by [`super::CrossRepairInputs::node_count`].
///
/// **That makes the default accounting exchanger-specific.** For any other
/// bundle -- `fhr_sim_v2`'s, or a re-sized HTGR unit -- the number returned here
/// is wrong in proportion to the geometry difference, while everything else in
/// this module stays correct. The general path is
/// [`repair_with_metal_capacity`], which takes the capacity explicitly. The
/// clean fix is to add a `metal_node_thermal_capacity: HeatCapacity` field to
/// [`super::CrossRepairInputs`]; that is a change to a shared contract and is
/// deliberately not made here.
///
/// The capacity is evaluated at the **mean of the supplied metal node
/// temperatures** and at [`super::CrossRepairInputs::cold_pressure`] (the
/// pressure the steam generator itself builds its `SolidColumn` at). Both
/// density and specific heat come from the TUAS material database, so the value
/// is derived, never typed in.
///
/// # Errors
///
/// [`CrossRepairError::BadInputs`] if the mean metal temperature is non-finite
/// or outside 300-1700 K (the correlation's tabulated range, outside which TUAS
/// panics), or if the property lookup returns a non-positive capacity.
// Dead until `TemperatureCrossRemedy::apply` has a caller; exercised by the tests.
#[allow(dead_code)]
pub fn assumed_metal_node_thermal_capacity(
    inputs: &CrossRepairInputs,
) -> Result<HeatCapacity, CrossRepairError> {
    let n = inputs.node_count();
    if n == 0 {
        return Err(CrossRepairError::BadInputs(
            "cannot size a per-node metal capacity for a zero-node exchanger".to_string(),
        ));
    }
    let mean_k = inputs
        .metal_temperatures
        .iter()
        .map(|t| t.get::<kelvin>())
        .sum::<f64>()
        / (n as f64);
    if !mean_k.is_finite() || mean_k < METAL_PROPERTY_MIN_K || mean_k > METAL_PROPERTY_MAX_K {
        return Err(CrossRepairError::BadInputs(format!(
            "mean tube-metal temperature {mean_k} K is outside the \
             {METAL_PROPERTY_MIN_K}-{METAL_PROPERTY_MAX_K} K range \
             SteelSS304LHighTemp is tabulated over; TUAS panics rather than extrapolating"
        )));
    }
    let total = SteamGeneratorGeometry::htr10_illustrative().metal_thermal_capacity(
        SolidMaterial::SteelSS304LHighTemp,
        ThermodynamicTemperature::new::<kelvin>(mean_k),
        inputs.cold_pressure,
    );
    let total_j_per_k = total.get::<joule_per_kelvin>();
    if !(total_j_per_k > 0.0) || !total_j_per_k.is_finite() {
        return Err(CrossRepairError::BadInputs(format!(
            "tube-metal thermal capacity came back as {total_j_per_k} J/K at {mean_k} K; \
             the material database did not return usable properties"
        )));
    }
    Ok(HeatCapacity::new::<joule_per_kelvin>(
        total_j_per_k / (n as f64),
    ))
}

/// Eliminate the tube metal, charging the discarded stored energy against a
/// **caller-supplied** per-node metal thermal capacity \[J/K\].
///
/// This is the general entry point; [`repair`] is this function with the
/// capacity taken from [`assumed_metal_node_thermal_capacity`]. Prefer this one
/// wherever the capacity is known -- for anything other than the exchanger
/// `htgr_sim_v1` builds, the assumed value is wrong.
///
/// # What it does
///
/// 1. Validates the profiles and conductances.
/// 2. Sets every metal node to [`quasi_steady_metal_temperature`] -- the Schur
///    complement of the metal row, derived in the module documentation.
/// 3. Hands the two fluid profiles back **unchanged**; this remedy imposes no
///    steady profile on either stream.
/// 4. Charges `C_node * sum_i (T_m,i* - T_m,i)` \[J\] to
///    [`CrossRepairOutcome::energy_discrepancy`]. Positive means the repair put
///    energy *into* the exchanger, which happens when the metal was lagging
///    below its quasi-steady value -- i.e. during a heat-up.
/// 5. Re-measures the cross on the produced profiles and refuses to return a
///    crossed result.
///
/// # Arguments
///
/// - `inputs` -- the crossed state, node-ordered hot-inlet-first (see
///   [`super::CrossRepairInputs`]); temperatures in K, conductances in W/K.
/// - `metal_node_thermal_capacity` -- thermal capacity of **one** metal node
///   \[J/K\], i.e. the whole tube bundle's `m c_p` divided by the node count.
///   Must be finite and non-negative; zero is legal and means the metal was
///   already massless, so nothing is discarded.
///
/// # Errors
///
/// - [`CrossRepairError::BadInputs`] -- mismatched or empty profiles, a
///   non-finite or non-positive temperature, a non-positive or non-finite
///   conductance, or a negative/non-finite capacity.
/// - [`CrossRepairError::DidNotConverge`] -- a cross remains between the two
///   fluid streams after the repair. **This is the normal outcome**, because
///   this remedy does not touch the streams; see the module documentation.
// Dead until `TemperatureCrossRemedy::apply` has a caller; exercised by the tests.
#[allow(dead_code)]
pub fn repair_with_metal_capacity(
    inputs: &CrossRepairInputs,
    metal_node_thermal_capacity: HeatCapacity,
) -> Result<CrossRepairOutcome, CrossRepairError> {
    validate(inputs)?;
    let c_node = metal_node_thermal_capacity.get::<joule_per_kelvin>();
    if !c_node.is_finite() || c_node < 0.0 {
        return Err(CrossRepairError::BadInputs(format!(
            "per-node metal thermal capacity {c_node} J/K must be finite and non-negative"
        )));
    }

    let g_h = inputs.hot_node_conductance;
    let g_c = inputs.cold_node_conductance;

    let metal_temperatures: Vec<ThermodynamicTemperature> = inputs
        .hot_temperatures
        .iter()
        .zip(inputs.cold_temperatures.iter())
        .map(|(t_h, t_c)| quasi_steady_metal_temperature(g_h, g_c, *t_h, *t_c))
        .collect();

    // First-law bookkeeping: the metal is the only array this remedy rewrites,
    // so the whole discrepancy is its stored-energy jump, `C_node * sum_i dT_i`.
    let delta_t_sum_k: f64 = metal_temperatures
        .iter()
        .zip(inputs.metal_temperatures.iter())
        .map(|(new, old)| new.get::<kelvin>() - old.get::<kelvin>())
        .sum();
    let energy_discrepancy = Energy::new::<joule>(c_node * delta_t_sum_k);

    let outcome = CrossRepairOutcome {
        hot_temperatures: inputs.hot_temperatures.clone(),
        metal_temperatures,
        cold_temperatures: inputs.cold_temperatures.clone(),
        energy_discrepancy,
    };

    // Never hand back a profile that still violates the second law. Measured on
    // the produced vectors rather than on the input, so the check stays honest
    // if this function ever starts touching the streams.
    let residual = worst_cross_kelvin_of(&outcome.hot_temperatures, &outcome.cold_temperatures);
    if residual > 0.0 {
        return Err(CrossRepairError::DidNotConverge(format!(
            "eliminating the tube metal left a {residual} K cross between the fluid streams. \
             This remedy rewrites the metal only -- it removes a lag from the coupling of the \
             NEXT step and cannot undo a cross already present in the stream state. Escalate \
             (design note tier 2) rather than treating this as repaired."
        )));
    }

    Ok(outcome)
}

/// Repair a crossed profile by eliminating the tube metal from the coupling
/// loop.
///
/// The [`super::TemperatureCrossRemedy::EliminateMetal`] dispatch entry point.
/// Equivalent to [`repair_with_metal_capacity`] with the per-node capacity taken
/// from [`assumed_metal_node_thermal_capacity`] -- read that function's
/// documentation, because the default capacity is specific to the exchanger
/// `htgr_sim_v1` builds and is the one part of this remedy that the
/// [`super::CrossRepairInputs`] contract cannot supply.
///
/// # Errors
///
/// [`CrossRepairError::BadInputs`] for unusable inputs, and
/// [`CrossRepairError::DidNotConverge`] whenever a cross remains between the
/// fluid streams -- which, because this remedy rewrites only the metal, is
/// **every time it is invoked on a crossed state**. That is a property of the
/// method, not a numerical failure; see the module documentation for why, and
/// what wiring in `steam_generator.rs` would be needed to get Tier 1's actual
/// benefit.
pub fn repair(inputs: &CrossRepairInputs) -> Result<CrossRepairOutcome, CrossRepairError> {
    let c_node = assumed_metal_node_thermal_capacity(inputs)?;
    repair_with_metal_capacity(inputs, c_node)
}

/// Largest amount \[K\] by which the cold stream exceeds the hot at any station;
/// zero or negative means no cross.
///
/// Mirrors [`super::CrossRepairInputs::worst_cross_kelvin`] but takes bare
/// slices, so the *produced* profiles can be measured with the same definition
/// the detector uses.
// Dead until `TemperatureCrossRemedy::apply` has a caller; exercised by the tests.
#[allow(dead_code)]
fn worst_cross_kelvin_of(
    hot: &[ThermodynamicTemperature],
    cold: &[ThermodynamicTemperature],
) -> f64 {
    hot.iter().zip(cold.iter()).fold(0.0_f64, |worst, (h, c)| {
        worst.max(c.get::<kelvin>() - h.get::<kelvin>())
    })
}

/// `Ok(())` if the supplied state is usable: three equal-length non-empty
/// profiles, every temperature finite and strictly above absolute zero, and both
/// conductances finite and strictly positive.
///
/// Mass flows, inlet boundary conditions and the hot-side pressure are **not**
/// checked, because this remedy does not read them: it rewrites the metal from
/// the two fluid profiles and the two conductances alone, and touches neither
/// stream.
// Dead until `TemperatureCrossRemedy::apply` has a caller; exercised by the tests.
#[allow(dead_code)]
fn validate(inputs: &CrossRepairInputs) -> Result<(), CrossRepairError> {
    let n = inputs.node_count();
    if n == 0 {
        return Err(CrossRepairError::BadInputs(
            "no nodes to repair".to_string(),
        ));
    }
    if inputs.metal_temperatures.len() != n || inputs.cold_temperatures.len() != n {
        return Err(CrossRepairError::BadInputs(format!(
            "profile lengths differ: hot {}, metal {}, cold {}",
            n,
            inputs.metal_temperatures.len(),
            inputs.cold_temperatures.len()
        )));
    }
    for (name, profile) in [
        ("hot", &inputs.hot_temperatures),
        ("metal", &inputs.metal_temperatures),
        ("cold", &inputs.cold_temperatures),
    ] {
        for (i, t) in profile.iter().enumerate() {
            let k = t.get::<kelvin>();
            if !k.is_finite() || k <= 0.0 {
                return Err(CrossRepairError::BadInputs(format!(
                    "{name} node {i} is at {k} K, which is not a physical temperature"
                )));
            }
        }
    }
    for (name, g) in [
        ("hot", inputs.hot_node_conductance),
        ("cold", inputs.cold_node_conductance),
    ] {
        let w_per_k = g.get::<watt_per_kelvin>();
        if !(w_per_k > 0.0) || !w_per_k.is_finite() {
            return Err(CrossRepairError::BadInputs(format!(
                "{name}-side node conductance {w_per_k} W/K must be finite and strictly positive"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tuas_boussinesq_solver::boussinesq_thermophysical_properties::thermal_conductivity::try_get_kappa_thermal_conductivity;
    use tuas_boussinesq_solver::boussinesq_thermophysical_properties::Material;
    use uom::si::available_energy::joule_per_kilogram;
    use uom::si::f64::{AvailableEnergy, MassRate, Pressure};
    use uom::si::length::meter;
    use uom::si::mass_rate::kilogram_per_second;
    use uom::si::pressure::pascal;
    use uom::si::thermal_conductivity::watt_per_meter_kelvin;

    /// The plant's overall steam-generator conductance \[W/K\] and the fraction
    /// of the series resistance carried by the hot film, as
    /// `primary_loop::steam_generator_config` sets them
    /// (`STEAM_GENERATOR_UA_W_PER_K`, `STEAM_GENERATOR_HOT_SIDE_RESISTANCE_FRACTION`).
    ///
    /// Repeated here rather than imported because both are private to
    /// `primary_loop`; `the_test_conductances_reproduce_the_plants_series_ua`
    /// checks the pair still combines to the public
    /// `primary_loop::STEAM_GENERATOR_UA_W_PER_K`, so a drift cannot go
    /// unnoticed.
    const UA_W_PER_K: f64 = 4.26e4;
    /// Fraction of the series resistance on the hot (helium) film.
    const HOT_RESISTANCE_FRACTION: f64 = 0.75;
    /// Axial node count of the plant's steam generator.
    const NODES: usize = 8;
    /// Rated hot-side duty \[W\] measured for this exchanger on 2026-08-13
    /// (`steam_generator::tests::the_corrector_substep_trade_is_measured`,
    /// plant configuration, 0.0125 s substep, 2 outer correctors). Used only to
    /// express a discarded energy in seconds of rated duty.
    const RATED_DUTY_W: f64 = 9.6854e6;

    fn hot_g() -> ThermalConductance {
        ThermalConductance::new::<watt_per_kelvin>(
            UA_W_PER_K / HOT_RESISTANCE_FRACTION / NODES as f64,
        )
    }
    fn cold_g() -> ThermalConductance {
        ThermalConductance::new::<watt_per_kelvin>(
            UA_W_PER_K / (1.0 - HOT_RESISTANCE_FRACTION) / NODES as f64,
        )
    }
    fn k(t: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(t)
    }

    /// A plausible **uncrossed** eight-node profile, hot inlet first, with the
    /// metal deliberately placed *away* from its quasi-steady value so the
    /// repair has something to move and the energy discrepancy is nonzero.
    ///
    /// Illustrative numbers in the spirit of the settled design point (helium
    /// 973 K in, feedwater near 460 K at the cold-inlet end); nothing here is
    /// taken from a specific plant.
    ///
    /// The metal is seeded **exactly 20 K below its quasi-steady value at every
    /// node** -- a metal lagging behind a heat-up. The weights are written out
    /// literally (`G_h/(G_h+G_c)` = 7100/28400 = 0.25 on the hot side, 0.75 on
    /// the cold) rather than obtained from
    /// [`quasi_steady_metal_temperature`], so the fixture does not depend on the
    /// function the tests are checking, and so the expected energy discrepancy
    /// is a round `C_node * 8 * 20 K`.
    fn uncrossed_inputs() -> CrossRepairInputs {
        let hot = [973.15, 930.0, 880.0, 830.0, 780.0, 730.0, 690.0, 660.0];
        let cold = [660.0, 600.0, 545.0, 523.0, 523.0, 523.0, 500.0, 460.0];
        let metal: Vec<ThermodynamicTemperature> = hot
            .iter()
            .zip(cold.iter())
            .map(|(h, c)| k(0.25 * h + 0.75 * c - 20.0))
            .collect();
        CrossRepairInputs {
            hot_temperatures: hot.iter().map(|t| k(*t)).collect(),
            metal_temperatures: metal,
            cold_temperatures: cold.iter().map(|t| k(*t)).collect(),
            hot_node_conductance: hot_g(),
            cold_node_conductance: cold_g(),
            hot_mass_flow: MassRate::new::<kilogram_per_second>(4.3),
            cold_mass_flow: MassRate::new::<kilogram_per_second>(3.19),
            hot_inlet_temperature: k(973.15),
            cold_inlet_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(168.73e3),
            hot_pressure: Pressure::new::<pascal>(3.0e6),
            cold_pressure: Pressure::new::<pascal>(4.0e6),
        }
    }

    /// The same profile with the cold stream pushed 5 K past the hot at one
    /// interior station -- a temperature cross of exactly 5 K.
    fn crossed_inputs() -> CrossRepairInputs {
        let mut inputs = uncrossed_inputs();
        let hot_k = inputs.hot_temperatures[4].get::<kelvin>();
        inputs.cold_temperatures[4] = k(hot_k + 5.0);
        inputs
    }

    /// V&V: **the two test conductances still reproduce the plant's series
    /// `UA`.**
    ///
    /// Methodology: `hot_g()` and `cold_g()` are built from constants copied out
    /// of the private `primary_loop` configuration. Combine them with
    /// [`series_node_conductance`], multiply by the node count, and compare
    /// against the public `primary_loop::STEAM_GENERATOR_UA_W_PER_K`. Pass
    /// criterion: relative difference below 1e-12.
    ///
    /// # Results (2026-08-13)
    ///
    /// Series node conductance 5325.000000 W/K, times 8 nodes =
    /// 42600.000000 W/K, against the plant's 42600.000000 W/K -- relative
    /// difference 0.000e0. Interpretation: the fixtures in this file are the plant's own
    /// conductance split, so every measured number below is a number for the
    /// shipped exchanger rather than for an invented one. If `primary_loop`
    /// re-tunes `UA`, this test fails and the copied constants must be updated.
    #[test]
    fn the_test_conductances_reproduce_the_plants_series_ua() {
        let series = series_node_conductance(hot_g(), cold_g()).get::<watt_per_kelvin>();
        let total = series * NODES as f64;
        let reference = super::super::super::primary_loop::STEAM_GENERATOR_UA_W_PER_K;
        let relative = (total - reference).abs() / reference;
        println!(
            "series node conductance {series:.6} W/K x {NODES} nodes = {total:.6} W/K, \
             plant UA = {reference:.6} W/K (relative difference {relative:.3e})"
        );
        assert!(
            relative < 1e-12,
            "test conductances give {total} W/K against the plant's {reference} W/K"
        );
    }

    /// V&V: **the closed-form quasi-steady metal temperature is the
    /// zero-capacitance limit of the coupling `steam_generator.rs` actually
    /// registers.**
    ///
    /// # Methodology
    ///
    /// TUAS assembles a lateral link as `Q = -H (T_node - T_lateral)`, so the
    /// metal node integrates
    /// `C dT_m/dt = G_h (T_h - T_m) + G_c (T_c - T_m)`. That ODE is integrated
    /// numerically here (explicit Euler at `dt = tau/1000`, 40000 steps, i.e.
    /// 40 time constants) with both fluid temperatures held fixed, from a start
    /// deliberately far from equilibrium, and the converged temperature is
    /// compared with [`quasi_steady_metal_temperature`]. Reference: the
    /// numerical integration. Pass criterion: agreement to 1e-9 K, and the
    /// converged residual `G_h(T_h - T_m) + G_c(T_c - T_m)` below 1e-9 of the
    /// station's own heat flux (an absolute watt tolerance would be measuring
    /// double precision, not the algebra).
    ///
    /// This is the check that the algebra in the module documentation matches
    /// the real coupling rather than an idealisation of it.
    ///
    /// # Results (measured 2026-08-13)
    ///
    /// At station 0 of the uncrossed fixture (`T_h` = 973.15 K, `T_c` = 660.0 K,
    /// `G_h` = 7100 W/K, `G_c` = 21300 W/K), integrating from 300 K:
    ///
    /// | Quantity | Value |
    /// |---|---|
    /// | Closed form `T_m*` | 738.287500 K |
    /// | Numerically integrated limit | 738.287500 K |
    /// | Difference | 5.684e-11 K |
    /// | Residual net power at the limit | 1.614e-6 W |
    /// | Residual as a fraction of the 1667.524 kW node flux | 9.676e-13 |
    ///
    /// Interpretation: the Schur complement in the module documentation is the
    /// exact steady state of the metal equation as coupled, so setting the metal
    /// to `T_m*` introduces no imbalance of its own -- the whole cost of the
    /// remedy is the *stored energy* jump, which is what
    /// [`CrossRepairOutcome::energy_discrepancy`] reports.
    #[test]
    fn the_quasi_steady_metal_is_the_zero_capacitance_limit_of_the_real_coupling() {
        let inputs = uncrossed_inputs();
        let g_h = hot_g().get::<watt_per_kelvin>();
        let g_c = cold_g().get::<watt_per_kelvin>();
        let t_h = inputs.hot_temperatures[0].get::<kelvin>();
        let t_c = inputs.cold_temperatures[0].get::<kelvin>();

        // A capacity and a timestep only used to march the reference ODE.
        let c_node = 2.0e5_f64;
        let tau = c_node / (g_h + g_c);
        let dt = tau / 1000.0;
        let mut t_m = 300.0_f64;
        for _ in 0..40_000 {
            let net = g_h * (t_h - t_m) + g_c * (t_c - t_m);
            t_m += dt * net / c_node;
        }
        let residual = g_h * (t_h - t_m) + g_c * (t_c - t_m);
        // Judged relative to the node's own heat flux: at 738 K a residual of a
        // few microwatts is the double-precision floor, not a modelling error.
        let node_flux =
            series_node_conductance(hot_g(), cold_g()).get::<watt_per_kelvin>() * (t_h - t_c);
        let relative_residual = residual.abs() / node_flux.abs();

        let closed = quasi_steady_metal_temperature(
            hot_g(),
            cold_g(),
            inputs.hot_temperatures[0],
            inputs.cold_temperatures[0],
        )
        .get::<kelvin>();

        println!(
            "T_h = {t_h} K, T_c = {t_c} K, G_h = {g_h} W/K, G_c = {g_c} W/K\n  \
             closed form  T_m* = {closed:.6} K\n  \
             integrated   T_m  = {t_m:.6} K  (difference {:.3e} K)\n  \
             residual net power at the limit = {residual:.3e} W \
             ({relative_residual:.3e} of the {:.3} kW node flux)",
            (closed - t_m).abs(),
            node_flux / 1e3
        );
        assert!(
            (closed - t_m).abs() < 1e-9,
            "closed form {closed} K differs from the integrated limit {t_m} K"
        );
        assert!(
            relative_residual < 1e-9,
            "residual {residual} W is {relative_residual} of the node flux, which is \
             far above the double-precision floor"
        );
    }

    /// V&V: **the metal-eliminated network passes identical heat on both sides,
    /// and that heat is `G_series (T_h - T_c)`.**
    ///
    /// # Methodology
    ///
    /// For every station of the uncrossed fixture, evaluate
    /// `q_hot = G_h (T_h - T_m*)` and `q_cold = G_c (T_m* - T_c)` at the
    /// quasi-steady metal temperature, and compare both against
    /// `G_series (T_h - T_c)` from [`series_node_conductance`]. Reference: the
    /// algebra in the module docs. Pass criterion: all three agree to within
    /// 1e-9 relative, at every node.
    ///
    /// # Results (measured 2026-08-13, eight nodes)
    ///
    /// | Node | `T_h - T_c` \[K\] | `q_hot` \[kW\] | `q_cold` \[kW\] | `G_series dT` \[kW\] |
    /// |---|---|---|---|---|
    /// | 0 | 313.150 | 1667.524 | 1667.524 | 1667.524 |
    /// | 1 | 330.000 | 1757.250 | 1757.250 | 1757.250 |
    /// | 2 | 335.000 | 1783.875 | 1783.875 | 1783.875 |
    /// | 3 | 307.000 | 1634.775 | 1634.775 | 1634.775 |
    /// | 4 | 257.000 | 1368.525 | 1368.525 | 1368.525 |
    /// | 5 | 207.000 | 1102.275 | 1102.275 | 1102.275 |
    /// | 6 | 190.000 | 1011.750 | 1011.750 | 1011.750 |
    /// | 7 | 200.000 | 1065.000 | 1065.000 | 1065.000 |
    ///
    /// Worst relative disagreement across all nodes: 4.189e-16, i.e. one unit in
    /// the last place.
    ///
    /// Interpretation: with the capacitance gone the wall stores nothing, so
    /// whatever leaves the hot stream arrives in the cold stream within the same
    /// iterate. This is the property that removes a lag from the coupling loop
    /// -- and, read the other way, it is exactly the property that discards the
    /// metal's thermal inertia.
    #[test]
    fn the_eliminated_network_passes_identical_heat_on_both_sides() {
        let inputs = uncrossed_inputs();
        let g_h = hot_g().get::<watt_per_kelvin>();
        let g_c = cold_g().get::<watt_per_kelvin>();
        let g_s = series_node_conductance(hot_g(), cold_g()).get::<watt_per_kelvin>();
        let mut worst = 0.0_f64;
        for i in 0..inputs.node_count() {
            let t_h = inputs.hot_temperatures[i].get::<kelvin>();
            let t_c = inputs.cold_temperatures[i].get::<kelvin>();
            let t_m = quasi_steady_metal_temperature(
                hot_g(),
                cold_g(),
                inputs.hot_temperatures[i],
                inputs.cold_temperatures[i],
            )
            .get::<kelvin>();
            let q_hot = g_h * (t_h - t_m);
            let q_cold = g_c * (t_m - t_c);
            let q_series = g_s * (t_h - t_c);
            println!(
                "node {i}: dT = {:.3} K, q_hot = {:.3} kW, q_cold = {:.3} kW, \
                 G_series dT = {:.3} kW",
                t_h - t_c,
                q_hot / 1e3,
                q_cold / 1e3,
                q_series / 1e3
            );
            let scale = q_series.abs().max(1.0);
            worst = worst
                .max((q_hot - q_cold).abs() / scale)
                .max((q_hot - q_series).abs() / scale);
        }
        println!("worst relative disagreement = {worst:.3e}");
        assert!(worst < 1e-9, "fluxes disagree by {worst} relative");
    }

    /// V&V: **the repaired metal never lies outside the two streams it sits
    /// between**, crossed or not.
    ///
    /// Methodology: evaluate [`quasi_steady_metal_temperature`] at every station
    /// of both the uncrossed and the crossed fixture (the crossed one directly,
    /// since [`repair`] refuses to return it), and check each result lies within
    /// `[min(T_h, T_c), max(T_h, T_c)]`. Reference: the convexity of the
    /// conductance-weighted mean. Pass criterion: no node further than 1e-9 K
    /// outside its bracket.
    ///
    /// # Results (measured 2026-08-13)
    ///
    /// All 8 nodes of the uncrossed fixture and all 8 of the crossed fixture lie
    /// inside their bracket; the largest excursion beyond a neighbour was
    /// 0.000e0 K. Interpretation: this remedy cannot manufacture a metal-side
    /// cross, at any conductance ratio, so the only cross it can leave behind is
    /// the stream-to-stream one it does not touch.
    #[test]
    fn the_repaired_metal_never_lies_outside_its_two_neighbours() {
        let mut worst_excursion = 0.0_f64;
        for inputs in [uncrossed_inputs(), crossed_inputs()] {
            for i in 0..inputs.node_count() {
                let t_h = inputs.hot_temperatures[i].get::<kelvin>();
                let t_c = inputs.cold_temperatures[i].get::<kelvin>();
                let t_m = quasi_steady_metal_temperature(
                    hot_g(),
                    cold_g(),
                    inputs.hot_temperatures[i],
                    inputs.cold_temperatures[i],
                )
                .get::<kelvin>();
                let lo = t_h.min(t_c);
                let hi = t_h.max(t_c);
                worst_excursion = worst_excursion.max((lo - t_m).max(t_m - hi).max(0.0));
            }
        }
        println!("largest excursion outside the neighbour bracket = {worst_excursion:.3e} K");
        assert!(
            worst_excursion < 1e-9,
            "the quasi-steady metal left its neighbours' bracket by {worst_excursion} K"
        );
    }

    /// V&V: **eliminating the metal does NOT clear a cross between the fluid
    /// streams** -- the central negative result of this remedy.
    ///
    /// # Methodology
    ///
    /// Take a profile with a 5 K cross at one interior station, run [`repair`],
    /// and check the outcome. Reference: the method itself, which rewrites the
    /// metal only. Pass criterion: [`repair`] returns
    /// [`CrossRepairError::DidNotConverge`] carrying the residual, and **never**
    /// `Ok` -- returning a still-crossed profile as a success would be a
    /// second-law violation shipped as a repair.
    ///
    /// # Results (measured 2026-08-13)
    ///
    /// Input worst cross 5.000000 K; [`repair`] returned
    /// `DidNotConverge("eliminating the tube metal left a 5 K cross between the
    /// fluid streams. ...")`. The residual equals the input cross exactly,
    /// because the stream profiles are handed back byte-for-byte unchanged.
    ///
    /// # Interpretation
    ///
    /// Tier 1 of `docs/heat-exchanger-temperature-cross-fallback.md` is a change
    /// to **how the next step is integrated**, not a repair of an existing
    /// state, and the [`super::CrossRepairInputs`] contract can only express the
    /// latter. So through this dispatch the remedy always fails, by
    /// construction. Its value has to be realised in `steam_generator.rs` --
    /// registering a direct hot<->cold link at `G_series` and setting the metal
    /// from [`quasi_steady_metal_temperature`] -- which is outside this file.
    /// Callers must escalate on this error, never swallow it.
    #[test]
    fn eliminating_the_metal_does_not_clear_a_cross_between_the_streams() {
        let inputs = crossed_inputs();
        let before = inputs.worst_cross_kelvin();
        println!("input worst cross = {before:.6} K");
        assert!((before - 5.0).abs() < 1e-9, "fixture cross is {before} K");
        match repair(&inputs) {
            Err(CrossRepairError::DidNotConverge(why)) => {
                println!("repair returned DidNotConverge: {why}");
                assert!(
                    why.contains("cross between the fluid streams"),
                    "unexpected message: {why}"
                );
            }
            other => panic!("expected DidNotConverge, got {other:?}"),
        }
    }

    /// V&V: **an uncrossed profile is repaired, and the discarded stored energy
    /// is reported rather than hidden.**
    ///
    /// # Methodology
    ///
    /// Repair the uncrossed fixture, whose metal is seeded exactly 20 K below
    /// its quasi-steady value at every node, with an explicit per-node capacity
    /// of 2.0e5 J/K through [`repair_with_metal_capacity`]. Check that (a) the
    /// streams come back unchanged, (b) every metal node equals the closed-form
    /// [`quasi_steady_metal_temperature`], and (c)
    /// [`CrossRepairOutcome::energy_discrepancy`] equals
    /// `C_node * sum_i (T_new,i - T_old,i)` computed independently in the test.
    /// Reference: the hand-computed sum. Pass criterion: energy agreement to
    /// 1e-6 relative and profile agreement to 1e-12 K.
    ///
    /// # Results (measured 2026-08-13)
    ///
    /// | Quantity | Value |
    /// |---|---|
    /// | Sum of metal temperature changes over 8 nodes | +160.000000 K |
    /// | Largest single-node change | +20.000000 K (all eight are +20 K by construction) |
    /// | Energy discrepancy at `C_node` = 2.0e5 J/K | +32.000000 MJ |
    /// | Independently summed reference | +32.000000 MJ |
    /// | Stream profiles changed | none, to 1e-12 K |
    ///
    /// The sign is positive: the seeded metal was **lagging below** its
    /// quasi-steady value, so snapping it up puts energy into the exchanger that
    /// no stream supplied.
    ///
    /// # Interpretation
    ///
    /// The first-law cost of this remedy is entirely the metal's stored-energy
    /// jump, and it is proportional to how far the metal was lagging -- i.e. it
    /// is *largest exactly during the fast transients that provoke a cross*.
    /// A caller must add this to its running total; a total that grows steadily
    /// means the remedy is doing real violence to the energy balance however
    /// admissible the temperature profiles look.
    #[test]
    fn an_uncrossed_profile_is_repaired_and_the_discarded_energy_is_reported() {
        let inputs = uncrossed_inputs();
        assert!(
            inputs.worst_cross_kelvin() <= 0.0,
            "fixture must be uncrossed"
        );
        let c_node = HeatCapacity::new::<joule_per_kelvin>(2.0e5);
        let outcome = repair_with_metal_capacity(&inputs, c_node).expect("uncrossed repair");

        let mut sum_dt = 0.0_f64;
        let mut largest = 0.0_f64;
        for i in 0..inputs.node_count() {
            assert!(
                (outcome.hot_temperatures[i].get::<kelvin>()
                    - inputs.hot_temperatures[i].get::<kelvin>())
                .abs()
                    < 1e-12,
                "hot stream node {i} was modified"
            );
            assert!(
                (outcome.cold_temperatures[i].get::<kelvin>()
                    - inputs.cold_temperatures[i].get::<kelvin>())
                .abs()
                    < 1e-12,
                "cold stream node {i} was modified"
            );
            let closed = quasi_steady_metal_temperature(
                hot_g(),
                cold_g(),
                inputs.hot_temperatures[i],
                inputs.cold_temperatures[i],
            )
            .get::<kelvin>();
            assert!(
                (outcome.metal_temperatures[i].get::<kelvin>() - closed).abs() < 1e-12,
                "metal node {i} is not the quasi-steady value"
            );
            let d = closed - inputs.metal_temperatures[i].get::<kelvin>();
            sum_dt += d;
            if d.abs() > largest.abs() {
                largest = d;
            }
        }
        let reference = 2.0e5 * sum_dt;
        let reported = outcome.energy_discrepancy.get::<joule>();
        println!(
            "sum of metal temperature changes = {sum_dt:+.6} K over {} nodes \
             (largest single node {largest:+.6} K)\n  \
             reported energy discrepancy = {:+.6} MJ, independently summed {:+.6} MJ",
            inputs.node_count(),
            reported / 1e6,
            reference / 1e6
        );
        assert!(
            (reported - reference).abs() / reference.abs().max(1.0) < 1e-6,
            "reported {reported} J against reference {reference} J"
        );
        assert!(
            reported > 0.0,
            "a lagging metal snapped upward must add energy"
        );
    }

    /// V&V: **the default per-node metal capacity is a real derived property,
    /// and it reproduces the exchanger's published time constant.**
    ///
    /// # Methodology
    ///
    /// Call [`assumed_metal_node_thermal_capacity`] on the uncrossed fixture,
    /// multiply back up by the node count, and form
    /// `tau = C_total / UA_series`. Reference: the metal time constant the
    /// exchanger itself reports through
    /// `steam_generator::NodalisedCounterFlowSteamGenerator::metal_time_constant`,
    /// which is documented at 38.42 s for `SolidMaterial::SteelSS304L` at 600 K
    /// (`steam_generator::tests::a_duty_step_is_filtered_by_the_metal_time_constant`,
    /// measured 2026-08-12/13) -- this fixture uses the *plant's*
    /// `SteelSS304LHighTemp`, so a few percent difference is expected and is the
    /// point of measuring it. Pass criterion: `tau` between 10 s and 200 s, and
    /// a strictly positive capacity; plus a rejection check for an
    /// out-of-tabulated-range metal temperature.
    ///
    /// # Results (measured 2026-08-13)
    ///
    /// | Quantity | Value |
    /// |---|---|
    /// | Mean metal temperature of the fixture | 588.598 K |
    /// | Total tube-metal thermal capacity | 1.694837 MJ/K |
    /// | Per node (8 nodes) | 0.211855 MJ/K |
    /// | `UA_series` | 42600.0 W/K |
    /// | `tau = C/UA` | **39.78 s** |
    /// | Rejection at a 2000 K mean metal temperature | `BadInputs`, as required |
    ///
    /// Interpretation: 39.78 s against the 38.42 s recorded for the Zou
    /// `SteelSS304L` at 600 K -- a 3.5% difference explained by the different
    /// correlation and evaluation temperature, not by an error. The capacity is
    /// therefore a genuine material-database value, not a typed-in constant, and
    /// the ~40 s inertia this remedy discards is real. Note the capacity is
    /// temperature-dependent, so `tau` moves with the metal profile the caller
    /// hands in; 39.78 s is the value at this fixture's 588.6 K mean, not a
    /// constant of the exchanger.
    #[test]
    fn the_default_metal_capacity_is_derived_and_reproduces_the_time_constant() {
        let inputs = uncrossed_inputs();
        let mean_k = inputs
            .metal_temperatures
            .iter()
            .map(|t| t.get::<kelvin>())
            .sum::<f64>()
            / inputs.node_count() as f64;
        let c_node = assumed_metal_node_thermal_capacity(&inputs)
            .expect("in-range metal temperatures")
            .get::<joule_per_kelvin>();
        let c_total = c_node * inputs.node_count() as f64;
        let tau = c_total / UA_W_PER_K;
        println!(
            "mean metal temperature {mean_k:.3} K\n  \
             total metal thermal capacity {:.6} MJ/K, per node {:.6} MJ/K\n  \
             tau = C/UA = {tau:.2} s (UA = {UA_W_PER_K} W/K)",
            c_total / 1e6,
            c_node / 1e6
        );
        assert!(c_node > 0.0, "capacity must be positive");
        assert!(
            (10.0..200.0).contains(&tau),
            "metal time constant {tau} s is implausible"
        );

        // Out of the correlation's tabulated range must be refused, not
        // extrapolated and not panicked through.
        let mut hot_metal = uncrossed_inputs();
        hot_metal.metal_temperatures = vec![k(2000.0); hot_metal.node_count()];
        match assumed_metal_node_thermal_capacity(&hot_metal) {
            Err(CrossRepairError::BadInputs(why)) => println!("2000 K metal refused: {why}"),
            other => panic!("expected BadInputs for a 2000 K metal, got {other:?}"),
        }
    }

    /// V&V: **axial conduction along the tube wall is negligible against the two
    /// films**, which is the one term the Schur complement in the module
    /// documentation drops.
    ///
    /// # Methodology
    ///
    /// The metal `SolidColumn` also conducts axially, `G_axial = k A_xs / dx`
    /// with `A_xs` the aggregate annular metal cross-section of the bundle,
    /// `dx = tube_length / n`, and `k` the steel's thermal conductivity from the
    /// TUAS material database at the fixture's mean metal temperature. Compare
    /// against the lateral conductance sum `G_h + G_c` per node. Reference: the
    /// material database and the published geometry. Pass criterion: the ratio
    /// below 1e-3, i.e. dropping axial conduction changes the quasi-steady metal
    /// temperature by less than a milli-fraction.
    ///
    /// # Results (measured 2026-08-13)
    ///
    /// | Quantity | Value |
    /// |---|---|
    /// | Steel thermal conductivity at 588.6 K | 17.640 W/(m K) |
    /// | Aggregate metal cross-section | 1.16632e-2 m^2 |
    /// | Node length `34 m / 8` | 4.250 m |
    /// | `G_axial` | 4.8408e-2 W/K |
    /// | `G_h + G_c` per node | 2.8400e4 W/K |
    /// | Ratio | **1.704e-6** |
    ///
    /// Interpretation: the tube wall is thin, the nodes are 4.25 m long and the
    /// films are enormous by comparison, so the axial term is six orders of
    /// magnitude down. Neglecting it in the elimination is safe at this
    /// nodalisation; it would need revisiting only for a far finer axial mesh or
    /// a much smaller `UA`.
    #[test]
    fn axial_conduction_in_the_tube_wall_is_negligible_against_the_films() {
        let inputs = uncrossed_inputs();
        let mean_k = inputs
            .metal_temperatures
            .iter()
            .map(|t| t.get::<kelvin>())
            .sum::<f64>()
            / inputs.node_count() as f64;
        let geometry = SteamGeneratorGeometry::htr10_illustrative();
        let kappa = try_get_kappa_thermal_conductivity(
            Material::Solid(SolidMaterial::SteelSS304LHighTemp),
            k(mean_k),
            inputs.cold_pressure,
        )
        .expect("steel conductivity in range")
        .get::<watt_per_meter_kelvin>();
        let a_xs = geometry
            .metal_cross_section()
            .get::<uom::si::area::square_meter>();
        let dx = geometry.tube_length.get::<meter>() / inputs.node_count() as f64;
        let g_axial = kappa * a_xs / dx;
        let g_lateral = hot_g().get::<watt_per_kelvin>() + cold_g().get::<watt_per_kelvin>();
        let ratio = g_axial / g_lateral;
        println!(
            "k = {kappa:.3} W/(m K) at {mean_k:.1} K, A_xs = {a_xs:.5e} m^2, dx = {dx:.3} m\n  \
             G_axial = {g_axial:.4e} W/K against G_h + G_c = {g_lateral:.4e} W/K \
             -- ratio {ratio:.3e}"
        );
        assert!(
            ratio < 1e-3,
            "axial conduction is {ratio} of the lateral conductance; \
             it can no longer be dropped from the Schur complement"
        );
    }

    /// V&V: **how much transient fidelity is lost by discarding the metal's
    /// thermal inertia.** This is the question this remedy has to answer for
    /// itself, because it is the one fidelity cost the other two remedies do not
    /// pay.
    ///
    /// # Methodology
    ///
    /// Two models of the *same* station are compared, both built from the
    /// plant's own conductances and the material-database capacity:
    ///
    /// - **metal retained** -- `C dT_m/dt = G_h (T_h - T_m) + G_c (T_c - T_m)`,
    ///   the equation `steam_generator.rs` registers, integrated by explicit
    ///   Euler at `dt = tau_node/500`;
    /// - **metal eliminated** -- `T_m = T_m*` at every instant, so the cold-side
    ///   flux is `G_series (T_h - T_c)` immediately.
    ///
    /// Both start from equilibrium at the fixture's station-0 temperatures, then
    /// the hot temperature is **stepped +100 K** and held, with the cold stream
    /// held fixed so the two models differ only in the metal treatment. Measured:
    /// the node relaxation time `tau_node = C_node/(G_h + G_c)`, the times at
    /// which the retained model reaches 63.2% and 95% of the eliminated model's
    /// instantaneous cold-side flux step, and the time integral of the flux
    /// difference -- the energy that the eliminated model delivers to the cold
    /// stream early because the wall no longer absorbs it.
    ///
    /// Pass criterion: the analytic first-order solution and the numerical
    /// integration agree to better than 0.5% on the 63.2% crossing (verifying
    /// the measurement itself), and the discarded energy is reported.
    ///
    /// # Results (measured 2026-08-13, plant conductances, `SteelSS304LHighTemp`
    /// at the fixture's 588.6 K mean metal temperature)
    ///
    /// | Quantity | Value |
    /// |---|---|
    /// | `C_node` | 0.211855 MJ/K |
    /// | `G_h + G_c` per node | 28400.0 W/K |
    /// | `tau_node = C_node/(G_h+G_c)` | **7.460 s** |
    /// | `tau_exchanger = C_total/UA_series` | **39.78 s** |
    /// | Cold-side flux step, eliminated model | +532.500 kW per node, at `t = 0` |
    /// | Retained model at `t = 0+` | 0 kW of that step |
    /// | Retained model reaches 63.2% | 7.46 s (analytic 7.460 s) |
    /// | Retained model reaches 95% | 22.33 s (analytic `3 tau` = 22.38 s) |
    /// | Energy delivered early, per node | 3.9723 MJ |
    /// | Energy delivered early, 8 nodes | **31.778 MJ** |
    /// | As a fraction of the 9.6854 MW rated duty | **3.28 s** of full-power heat |
    ///
    /// # Interpretation -- the honest answer is "a lot"
    ///
    /// Discarding the metal's capacitance is **not** a small correction. Three
    /// separate readings say so:
    ///
    /// 1. **The lag it removes is the exchanger's dominant one.** 7.5 s at the
    ///    node level, 39.8 s at the exchanger level, against a 0.1 s plant
    ///    timestep and a shell transport time of a few hundred milliseconds. It
    ///    is the slowest thing in the steam generator by two orders of
    ///    magnitude.
    /// 2. **The step response changes shape, not just amplitude.** The retained
    ///    model tracks 0% of a duty step instantly and 63% after 7.5 s; the
    ///    eliminated model tracks 100% instantly. For comparison, the
    ///    isothermal-sink model this exchanger replaced also tracked 100%
    ///    instantly -- so on this axis the remedy gives back the very defect the
    ///    nodalisation was built to remove.
    /// 3. **31.8 MJ of buffered heat stops being buffered**, and is delivered to
    ///    the steam side early instead. That is 3.3 s of full-power duty
    ///    arriving ahead of schedule, for a single 100 K step, and it arrives
    ///    during a transient -- precisely when a digital twin's output is being
    ///    looked at.
    ///
    /// So this remedy is cheap in **compute** and cheap in **thermodynamic
    /// admissibility** -- it cannot create a metal-side cross and it conserves
    /// nothing it should not -- but it is **expensive in dynamics**. It should
    /// never be engaged silently, and any transient computed with it engaged is
    /// not merely "a different transient", it is a transient of a different
    /// plant: one whose steam generator has no tube mass.
    #[test]
    fn discarding_the_metal_capacitance_removes_a_measured_lag() {
        let inputs = uncrossed_inputs();
        let c_node = assumed_metal_node_thermal_capacity(&inputs)
            .expect("in-range metal temperatures")
            .get::<joule_per_kelvin>();
        let g_h = hot_g().get::<watt_per_kelvin>();
        let g_c = cold_g().get::<watt_per_kelvin>();
        let g_s = series_node_conductance(hot_g(), cold_g()).get::<watt_per_kelvin>();
        let tau_node = c_node / (g_h + g_c);
        let tau_exchanger = c_node * inputs.node_count() as f64 / UA_W_PER_K;

        // Station 0, at equilibrium, then a +100 K step on the hot stream with
        // the cold stream held so the only difference between the two models is
        // the metal treatment.
        let t_c = inputs.cold_temperatures[0].get::<kelvin>();
        let t_h0 = inputs.hot_temperatures[0].get::<kelvin>();
        let step_k = 100.0_f64;
        let t_h1 = t_h0 + step_k;

        let mut t_m = (g_h * t_h0 + g_c * t_c) / (g_h + g_c);
        let q_cold_before = g_c * (t_m - t_c);
        let q_cold_after = g_s * (t_h1 - t_c);
        let flux_step = q_cold_after - q_cold_before;

        let dt = tau_node / 500.0;
        let steps = 500 * 40; // 40 time constants
        let mut t_63 = f64::NAN;
        let mut t_95 = f64::NAN;
        let mut early_energy = 0.0_f64;
        for s in 0..steps {
            let t_now = s as f64 * dt;
            let q_retained = g_c * (t_m - t_c);
            // The eliminated model sits at its final flux from t = 0.
            early_energy += (q_cold_after - q_retained) * dt;
            let fraction = (q_retained - q_cold_before) / flux_step;
            if t_63.is_nan() && fraction >= 1.0 - (-1.0_f64).exp() {
                t_63 = t_now;
            }
            if t_95.is_nan() && fraction >= 0.95 {
                t_95 = t_now;
            }
            let net = g_h * (t_h1 - t_m) + g_c * (t_c - t_m);
            t_m += dt * net / c_node;
        }
        let total_energy = early_energy * inputs.node_count() as f64;

        println!(
            "C_node = {:.6} MJ/K, G_h + G_c = {:.1} W/K\n  \
             tau_node = {tau_node:.3} s, tau_exchanger = C_total/UA = {tau_exchanger:.2} s\n  \
             cold-side flux step (eliminated, instantaneous) = {:.3} kW per node\n  \
             retained model reaches 63.2% at {t_63:.2} s (analytic {tau_node:.3} s), \
             95% at {t_95:.2} s (analytic 3 tau = {:.2} s)\n  \
             energy delivered early = {:.4} MJ per node, {:.3} MJ over {} nodes \
             = {:.2} s of the {:.4} MW rated duty",
            c_node / 1e6,
            g_h + g_c,
            flux_step / 1e3,
            3.0 * tau_node,
            early_energy / 1e6,
            total_energy / 1e6,
            inputs.node_count(),
            total_energy / RATED_DUTY_W,
            RATED_DUTY_W / 1e6
        );

        assert!(
            (t_63 - tau_node).abs() / tau_node < 5e-3,
            "the numerically measured 63.2% crossing {t_63} s disagrees with the \
             analytic first-order value {tau_node} s; the measurement is not trustworthy"
        );
        assert!(
            total_energy > 0.0,
            "eliminating a real thermal mass must deliver energy to the cold side early"
        );
        assert!(
            tau_node > 1.0,
            "a {tau_node} s node lag would make this remedy's fidelity cost negligible, \
             which contradicts the exchanger's measured behaviour -- re-check the capacity"
        );
    }

    /// Guards the input validation: a remedy that cannot run must say so rather
    /// than repairing something it was not given.
    ///
    /// Methodology: feed mismatched profile lengths, an empty profile, a
    /// non-physical temperature and a zero conductance. Pass criterion: each
    /// returns [`CrossRepairError::BadInputs`].
    ///
    /// # Results (2026-08-13)
    ///
    /// All four returned `BadInputs` with the offending quantity named.
    /// Interpretation: the remedy fails loudly on unusable state instead of
    /// producing a confident wrong profile.
    #[test]
    fn unusable_inputs_are_refused() {
        let mut short = uncrossed_inputs();
        short.cold_temperatures.pop();
        let mut empty = uncrossed_inputs();
        empty.hot_temperatures.clear();
        empty.metal_temperatures.clear();
        empty.cold_temperatures.clear();
        let mut absurd = uncrossed_inputs();
        absurd.hot_temperatures[2] = k(-1.0);
        let mut dead = uncrossed_inputs();
        dead.cold_node_conductance = ThermalConductance::new::<watt_per_kelvin>(0.0);

        for (name, inputs) in [
            ("mismatched lengths", short),
            ("empty profiles", empty),
            ("negative temperature", absurd),
            ("zero conductance", dead),
        ] {
            let c = HeatCapacity::new::<joule_per_kelvin>(2.0e5);
            match repair_with_metal_capacity(&inputs, c) {
                Err(CrossRepairError::BadInputs(why)) => println!("{name} refused: {why}"),
                other => panic!("{name}: expected BadInputs, got {other:?}"),
            }
        }
    }
}
