//! BEDOK enthalpy march -- remedy for a steam-generator temperature cross.
//!
//! Selected by [`super::TemperatureCrossRemedy::Bedok`]. See that variant's
//! documentation for what this method is and when it is the right choice, and
//! `docs/heat-exchanger-temperature-cross-fallback.md` for the design
//! discussion behind all three remedies.
//!
//! # Attribution
//!
//! The enthalpy-march formulation is **Than Yan Ren's** (Singapore Nuclear
//! Research and Safety Institute), from `singleflow1devap.m` in the BEDOK
//! MATLAB code, used here with the author's permission. That MATLAB is already
//! translated into this workspace as `bedok::reference::th::single_flow_evap`
//! (`crates/bedok/src/reference/th/single_flow_evap.rs`, BEDOK snapshot sha256
//! `e45cd6f57be2087c...`, received 2026-08-05), and **that translation, not the
//! MATLAB, is the reference this file was written against.** Read it before
//! changing anything here.
//!
//! Following the same substitution that translation records, the IAPWS calls
//! go through this workspace's own IF97 implementation rather than a
//! third-party `IAPWS_IF97.m`. The BEDOK translation reaches IF97 through
//! `bedok::reference::th::steam`; this file reaches the *same* standard through
//! `tampines-steam-tables`, because `bedok` is not a dependency of
//! `outram-park-digital-twin-engine` and `tampines-steam-tables` is already the
//! cold side's equation of state in
//! [`super::super::steam_generator`]. **No new steam-property path is
//! introduced by this module.**
//!
//! # What the reference does, and what had to change
//!
//! `single_flow_evap.rs` is a **single heated channel with a prescribed wall
//! heat flux**. Stage 1 marches the mixture enthalpy along the channel from a
//! pure energy balance at constant pressure,
//!
//! ```text
//! dh/dz = q'_wall / (G*A)
//! ```
//!
//! as a **half-node march** -- the first active node takes half of its own
//! enthalpy rise, each subsequent node takes half of its predecessor's rise
//! plus half of its own:
//!
//! ```text
//! h[inlet] = h_in + 0.5*dh[inlet]
//! h[i]     = h[i-1] + 0.5*dh[i-1] + 0.5*dh[i]
//! ```
//!
//! Stage 2 inverts that enthalpy into a temperature at the channel pressure:
//! `T(p,h)` when subcooled or superheated, `Tsat(p)` in the dome. **Working in
//! enthalpy is the entire point** -- the march passes through the boiling
//! transition natively, with no zone boundaries to locate and no per-zone heat
//! transfer coefficient to choose. That is why this remedy is preferred over
//! [`super::lmtd_profile`] for a once-through steam generator.
//!
//! Three things had to change to make it a **counter-flow exchanger**:
//!
//! 1. **There are two streams, and they are coupled.** The wall heat flux is
//!    not prescribed here; it is `q_i = U_i*(T_hot,i - T_cold,i)` with `U_i`
//!    the series combination of the two film conductances the caller supplies.
//!    The hot stream's enthalpy loss is the cold stream's gain, node by node.
//! 2. **The half-node march becomes implicit within each node.** In the
//!    reference `dh[i]` is known before the march starts. Here `q_i` depends on
//!    the very temperatures the node is computing, so each node is a small
//!    scalar root-find. That is not a departure from the scheme -- it is the
//!    same trapezoidal relation, solved rather than evaluated -- and it makes
//!    the march A-stable instead of conditionally stable.
//! 3. **It is a two-point boundary value problem.** The hot inlet is known at
//!    node 0 and the cold inlet at node `n-1` (counter-flow: see
//!    [`super::CrossRepairInputs`] on the node ordering). A single march cannot
//!    start from both ends, so this is solved by **shooting** -- see below.
//!
//! # The two-point boundary value problem
//!
//! **Shooting parameter:** the hot stream's **outlet face temperature** at the
//! node `n-1` end \[K\]. With it, both streams' states are known at that end --
//! the cold inlet enthalpy is given there -- and one march in the `-index`
//! direction (downstream for the cold stream, upstream for the hot) produces
//! the whole exchanger. The march is closed by the residual
//!
//! ```text
//! R(T_hot_out) = T_hot_inlet_implied_by_the_march - T_hot_inlet_given   [K]
//! ```
//!
//! `R` is **monotone increasing**: a hotter guessed outlet raises the hot
//! profile, raises every `q_i`, and raises the implied inlet twice over.
//!
//! **Bracket:** `[T_cold_inlet, T_hot_inlet]`. Both ends are thermodynamic
//! limits rather than tuning knobs -- the hot stream cannot leave colder than
//! the cold stream enters (second law), and cannot leave hotter than it entered
//! while giving up heat.
//!
//! The two ends behave very differently, and this is worth stating because it
//! is the opposite of the intuition. The **lower** end is the *least*-duty
//! march: the hot stream sits at the cold inlet temperature where the two
//! streams meet, so every `q_i` is as small as the geometry allows. The
//! **upper** end is the *most*-duty march -- a hot stream still at its inlet
//! temperature 660 K above the feedwater -- and it is routinely so violent that
//! the cold enthalpy leaves the IF97 envelope before the march finishes.
//!
//! The lower end therefore either marches or nothing does (march validity is
//! monotone in the guess), and `R(lower) > 0` means the exchanger cannot reject
//! its duty without the hot stream leaving below the cold inlet -- no admissible
//! solution, reported as such. The upper end is found by **bisecting on
//! marchability** between the lower end and the hot inlet, which walks down to
//! the first guess that both marches and has a positive residual. That is a
//! bracket search over guesses, not a limit on any physical quantity: no
//! temperature or enthalpy is ever pulled back to a boundary.
//!
//! **Iteration:** the **Illinois** variant of regula falsi -- a *bracketing*
//! method, chosen over Newton deliberately. Newton needs `dR/dT`, which here
//! means differentiating through an IF97 flash and through every node's inner
//! solve, and it can step outside the physical bracket into a region where the
//! `(p,h)` flash has no answer at all. Illinois keeps the sign bracket at every
//! step, so it cannot leave the physical interval, while still converging
//! superlinearly. Cap [`MAX_SHOOTING_ITERATIONS`], tolerance
//! [`SHOOTING_TOLERANCE_KELVIN`].
//!
//! **Non-convergence is reported, never absorbed.** If the bracket does not
//! bracket, if it collapses to floating-point resolution with the residual
//! still above tolerance, if a march walks the cold enthalpy outside the IF97
//! `(p,h)` envelope, or if the converged profile still contains a cross, the
//! result is [`super::CrossRepairError::DidNotConverge`] carrying the measured
//! residual and the full diagnostics. There is no path that returns a profile
//! which still violates the second law.
//!
//! # The hot side is *inferred*, and this is the method's weakest joint
//!
//! The cold stream is water/steam and its caloric equation of state is IF97.
//! The hot stream's is **not available**: [`super::CrossRepairInputs`] carries
//! `hot_pressure` and `hot_inlet_temperature` but **not the hot fluid's
//! identity**, and this exchanger is deliberately fluid-agnostic
//! (`htgr_sim_v1` passes helium, `fhr_sim_v2` will pass a molten salt). Rather
//! than hardcode helium -- which would be silently wrong for the salt -- the
//! hot-side **heat capacity rate** `C_hot = m_dot*c_p` \[W/K\] is inferred from
//! the supplied state by its own energy balance:
//!
//! ```text
//! C_hot = UA_hot * sum_i (T_hot,i - T_metal,i) / (T_hot_inlet - T_hot,n-1)
//! ```
//!
//! The numerator is exactly the heat the hot array is losing to the metal --
//! the same expression the array itself integrates -- and the denominator is
//! the temperature drop that loss produced. This is the secant slope of the hot
//! fluid's `h(T)`, which for a gas or a salt over an exchanger's temperature
//! span is close to constant. Consequences, all of them real:
//!
//! - **It is contaminated by the very state being repaired.** The supplied
//!   profile contains a cross, and the metal profile a real array hands over
//!   will have moved with it. Measured on the HTR-10 fixture, a 40 K cold
//!   overshoot moves the repaired duty by **4.20%** --
//!   [`tests::a_healthy_profile_marches_to_the_same_steady_state`]. A repair is
//!   therefore not independent of how bad the cross was.
//! - **The denominator is half-node corrected, in closed form.** `dT` is
//!   measured to the last node's cell *centre*, but the duty is rejected at its
//!   outlet *face*, half a node further on. Telescoping the march gives
//!   `C_hot = (Q - 0.5*q[n-1]) / dT` exactly, with `q[n-1]` read from the
//!   supplied state like the rest of `Q`. Without that term the inference is
//!   biased high -- measured at **+5.97%** on the HTR-10 fixture -- and the
//!   remedy walks the state further every time it engages. With it, a repaired
//!   profile is a fixed point of the repair to 7.8e-10 relative:
//!   [`tests::the_repair_is_very_nearly_its_own_fixed_point`].
//! - **A single constant `c_p` cannot represent a hot stream that boils.** This
//!   remedy is only correct for a single-phase hot side. That is the case for
//!   every exchanger in this simulator, and it is asserted rather than assumed:
//!   an inferred `c_p` outside [`MIN_PLAUSIBLE_HOT_CP`] ..
//!   [`MAX_PLAUSIBLE_HOT_CP`] J/(kg K) is rejected as
//!   [`super::CrossRepairError::BadInputs`] with the number, not silently used.
//!
//! **If the contract ever carries the hot fluid's identity, delete the
//! inference and call the equation of state.** The inference exists only
//! because the identity is missing.
//!
//! # Nothing is clamped
//!
//! The reference clamps its marched enthalpy into `[0, h(p, 1050 K)]` to keep
//! the IAPWS inversions in a valid region. **No marched quantity is clamped
//! here.** Where the reference would clamp, this returns an error instead: a
//! cold enthalpy outside the IF97 `(p,h)` envelope aborts that shot and, if it
//! cannot be avoided by moving inside the bracket, aborts the repair.
//!
//! Two things do bound something, and both are named rather than hidden:
//!
//! - **The shooting bracket** limits the *guesses*, not any physical quantity.
//!   Its ends are thermodynamic bounds and the diagnostics report the iterate
//!   count and residual, so a bracket that binds is visible.
//! - **The first-law bookkeeping clamps one reconstruction into `[h_f, h_g]`**
//!   -- see [`energy_discrepancy`]. That is not a marched value: it is the
//!   pre-repair enthalpy of a node whose supplied *temperature* was on the
//!   saturation line, where the enthalpy genuinely is only known to lie in that
//!   interval. Those nodes are **counted** in
//!   [`EnthalpyMarchDiagnostics::saturation_ambiguous_nodes`] and the resulting
//!   uncertainty is reported alongside the figure.
//!
//! # Assumptions, inherited and added
//!
//! Inherited from the reference: **constant pressure** along each stream (no
//! pressure drop), **thermal equilibrium** (one mixture enthalpy per node, no
//! subcooled boiling, no interfacial heat transfer), **constant mass flow**
//! along each stream.
//!
//! Added here: a **single-phase hot stream with a constant heat capacity
//! rate**; a **steady** state (there is no time derivative anywhere in this
//! file -- the whole remedy is the deliberate fidelity trade the design note
//! describes); and **conductances constant along the exchanger**, since the
//! contract supplies one scalar per side.
//!
//! # V&V
//!
//! See the `tests` module. Every test states its methodology, its pass
//! criterion, and the numbers actually measured on 2026-08-13. In summary:
//!
//! - **Verification against an analytic reference.** In a wholly single-phase
//!   case the march reproduces the closed-form counter-flow
//!   effectiveness-NTU outlet temperatures to **0.135 K on the hot outlet and
//!   0.011 K on the cold** (0.086% and 0.033% of the respective spans), and the
//!   duty to 0.086% --
//!   [`tests::the_march_reproduces_the_analytic_counter_flow_solution`].
//! - **The cross is removed.** On an 8-node HTR-10-like profile carrying a
//!   deliberate 40.0 K cross, the repaired profile has worst cross
//!   **-146.66 K** (a 146.66 K pinch), with the metal strictly between the two
//!   streams at every node --
//!   [`tests::a_crossed_htr10_profile_comes_back_cross_free`].
//! - **It passes through boiling without zoning** -- the repaired cold profile
//!   carries 3 subcooled, 4 saturated and 1 superheated node at once, with no
//!   zone boundary ever located --
//!   [`tests::the_march_crosses_the_boiling_transition_without_zoning`].
//! - **The repair is a fixed point of itself** to 7.8e-10 relative, so
//!   repeated engagement does not walk the state --
//!   [`tests::the_repair_is_very_nearly_its_own_fixed_point`].
//! - **One repair costs ~425 us**, about 3.4% of one steam-generator substep --
//!   [`tests::one_repair_costs_a_small_fraction_of_a_steam_generator_substep`].
//!
//! **What is NOT verified, and cannot be here:** the phase-change path, which
//! is the entire reason this method was chosen over LMTD, has **no analytic
//! reference to be checked against**. The boiling case is checked for
//! self-consistency (cross-free, fixed point, energy accounted) and for
//! *behaviour* (all three regimes present), not for correctness against an
//! independent solution. Nothing here is a validation against a measured
//! exchanger.
//!
//! **This is a heuristic that trades transient fidelity for thermodynamic
//! admissibility.** A profile produced by this remedy is a *steady* profile,
//! not the transient the arrays were computing. Nothing computed while it is
//! engaged may be reported as a resolved transient.

// Nothing in `physics::mod` calls `TemperatureCrossRemedy::apply` yet -- the
// remedies exist before the dispatch that will select them -- so in a non-test
// build every item below is unreachable and rustc reports the entire module as
// dead. This allow keeps that transient noise out of the build.
//
// **Delete it the moment the dispatch is wired**, so that genuine dead code in
// this file becomes visible again. It is a scaffold, not a policy.
#![allow(dead_code)]

use super::{CrossRepairError, CrossRepairInputs, CrossRepairOutcome};

use tampines_steam_tables::interfaces::functional_programming::ph_flash_eqm::t_ph_eqm;
use tampines_steam_tables::interfaces::functional_programming::pt_flash_eqm::h_tp_eqm_single_phase;
use tampines_steam_tables::region_1_subcooled_liquid::h_tp_1;
use tampines_steam_tables::region_2_vapour::h_tp_2;
use tampines_steam_tables::region_4_vap_liq_equilibrium::{sat_pressure_4, sat_temp_4};

use uom::si::available_energy::joule_per_kilogram;
use uom::si::energy::joule;
use uom::si::f64::{
    AvailableEnergy, Energy, Power, Pressure, SpecificHeatCapacity, TemperatureInterval,
    ThermalConductance, ThermodynamicTemperature,
};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::temperature_interval::kelvin as kelvin_interval;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

// ---------------------------------------------------------------------------
// Constants. Every one of these is a numerical or standards limit, not a
// tunable: none of them may be adjusted to make a case pass.
// ---------------------------------------------------------------------------

/// Fewest nodes this remedy will march, 2.
///
/// A one-node exchanger puts both boundary conditions on the same cell, so
/// there is no axial profile to repair and the shooting residual degenerates.
const MIN_NODE_COUNT: usize = 2;

/// Lowest temperature of the IAPWS-IF97 industrial formulation \[K\], 273.15.
///
/// The `(p,h)` flash in `tampines-steam-tables` **panics** below this rather
/// than returning an error (see that crate's `validity_range.rs`), so every
/// cold-side enthalpy is range-checked before it is handed over.
const IF97_MIN_TEMPERATURE_K: f64 = 273.15;

/// Highest temperature of the IAPWS-IF97 industrial formulation \[K\], 1073.15.
///
/// Above it lies Region 5, for which IAPWS publishes **no backward `(p,h)`
/// correlation at all**, so the flash panics. Same precedent as
/// `super::super::secondary_loop`'s `IF97_MAX_TEMPERATURE_K`.
const IF97_MAX_TEMPERATURE_K: f64 = 1073.15;

/// Upper pressure limit of IAPWS-IF97 \[Pa\], 100 MPa.
const IF97_MAX_PRESSURE_PA: f64 = 100.0e6;

/// Critical pressure of water \[Pa\], 22.064 MPa.
///
/// Above it there is no saturation dome, so the two-phase branch of the
/// pre-repair enthalpy reconstruction does not apply.
const WATER_CRITICAL_PRESSURE_PA: f64 = 22.064e6;

/// How close to `Tsat(p)` a supplied cold temperature must be \[K\] before it
/// is treated as two-phase and therefore **not** invertible to an enthalpy,
/// 1e-3 K.
///
/// Inside the dome, temperature does not determine state -- which is precisely
/// why [`super::CrossRepairInputs::cold_inlet_enthalpy`] is an enthalpy. A
/// `(T,p)` flash at exactly saturation is also a panic in
/// `tampines-steam-tables` (Region 4 `(T,p)` is deliberately unsupported), so
/// this branch is a correctness requirement as well as a physics one.
const SATURATION_TOLERANCE_KELVIN: f64 = 1.0e-3;

/// Smallest hot-stream temperature drop \[K\] the capacity-rate inference will
/// divide by, 1.0 K.
///
/// Below this the inferred `C_hot` is dominated by whatever noise is in the
/// supplied profile, so the inputs are rejected instead.
const MIN_HOT_TEMPERATURE_DROP_KELVIN: f64 = 1.0;

/// Lowest specific heat \[J/(kg K)\] the hot-side inference will accept, 100.
///
/// Every candidate reactor coolant is far above this: helium 5193, water
/// 4200-5000, FLiBe ~2400, sodium ~1270, CO2 ~1200 J/(kg K). A value below it
/// means the supplied state is not a consistent exchanger state, which is
/// reported rather than marched.
const MIN_PLAUSIBLE_HOT_CP: f64 = 100.0;

/// Highest specific heat \[J/(kg K)\] the hot-side inference will accept,
/// 20000. Roughly four times helium's, i.e. generous by design; see
/// [`MIN_PLAUSIBLE_HOT_CP`].
const MAX_PLAUSIBLE_HOT_CP: f64 = 20000.0;

/// Convergence tolerance on the shooting residual \[K\], 1e-6.
///
/// The residual is a temperature at the hot inlet, so this is 1e-9 relative at
/// 1000 K -- four orders above f64 resolution there, which leaves room for the
/// error amplification a counter-flow march necessarily has.
const SHOOTING_TOLERANCE_KELVIN: f64 = 1.0e-6;

/// Iteration cap on the outer shoot, 200. Illinois on a smooth monotone
/// residual converges in tens; reaching 200 means the residual is not behaving
/// and the repair fails loudly.
const MAX_SHOOTING_ITERATIONS: usize = 200;

/// Iteration cap on the bisection that finds a marchable upper end of the
/// shooting bracket, 60. Each step halves the interval, so 60 exhausts f64
/// resolution long before it exhausts the count.
const MAX_BRACKET_WALKS: usize = 60;

/// Iteration cap on each node's inner scalar solve, 60.
const MAX_NODE_ITERATIONS: usize = 60;

/// Convergence tolerance on each node's power balance \[W\], 1e-6, plus the
/// relative part below for large duties.
///
/// Still far tighter than the outer tolerance needs: a node power error `e`
/// moves the implied hot inlet by only `e/C_hot`, so at a megawatt-scale duty
/// and a 2e4 W/K capacity rate the combined tolerance of 1e-3 W is worth 5e-8 K
/// -- two orders below [`SHOOTING_TOLERANCE_KELVIN`] even summed over every
/// node.
const NODE_TOLERANCE_WATT: f64 = 1.0e-6;

/// Relative part of the node tolerance, 1e-9. See [`NODE_TOLERANCE_WATT`].
///
/// A purely absolute tolerance of 1e-6 W is 1e-12 *relative* at a megawatt
/// duty, which is at the f64 noise floor of the residual itself: the secant
/// then thrashes to its iteration cap and reports a stall at a residual that is
/// numerically zero. Measured 2026-08-13: a starved-cold-flow case failed with
/// "node 4 stalled with residual 1.163233e-6 W" for exactly that reason,
/// masking the real failure.
const NODE_RELATIVE_TOLERANCE: f64 = 1.0e-9;

/// The stream inventory the reported energy discrepancy is referred to \[s\],
/// 1.0 -- i.e. **one second of flow** in each stream.
///
/// [`super::CrossRepairInputs`] carries mass *flows*, not node masses, volumes
/// or transit times, so a stored-energy change in joules is not determined by
/// the contract. Rather than stub the field to zero (forbidden) or invent a
/// geometry (worse), the discrepancy is computed for a **stated** inventory of
/// `m_dot * 1 s` per stream, spread evenly over the nodes. To convert to the
/// physical figure, multiply by the stream transit time in seconds. See
/// [`energy_discrepancy`].
const REFERENCE_INVENTORY_TIME_S: f64 = 1.0;

// ---------------------------------------------------------------------------
// Public surface
// ---------------------------------------------------------------------------

/// Everything the enthalpy march measured about itself while repairing one
/// profile.
///
/// This exists because the repair is a **heuristic with several inferred
/// quantities**, and an inference that is never looked at is an inference
/// nobody can falsify. Every field is a measurement of this particular repair,
/// not a configuration knob. Units are spelled out even though `uom` enforces
/// them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EnthalpyMarchDiagnostics {
    /// Outer shooting iterations used (Illinois), dimensionless count.
    pub shooting_iterations: usize,
    /// Final shooting residual: implied hot inlet minus given hot inlet \[K\].
    /// Converged means `|residual| <=` [`SHOOTING_TOLERANCE_KELVIN`].
    pub shooting_residual: TemperatureInterval,
    /// Total inner node-solve iterations over the **converged** march,
    /// dimensionless count. A cost proxy: each iteration is one IF97 `(p,h)`
    /// flash.
    pub node_solver_iterations: usize,
    /// Hot-stream heat capacity rate `m_dot*c_p` inferred from the supplied
    /// state \[W/K\]. See the module docs -- this is the method's weakest
    /// joint.
    pub inferred_hot_capacity_rate: ThermalConductance,
    /// The same inference divided by the supplied hot mass flow
    /// \[J/(kg K)\], for a reader who wants to sanity-check it against a
    /// tabulated `c_p`.
    pub inferred_hot_specific_heat: SpecificHeatCapacity,
    /// Total duty of the repaired steady profile, `sum_i q_i` \[W\].
    pub duty: Power,
    /// Smallest `T_hot - T_cold` over the repaired nodes \[K\]. Positive is a
    /// pinch; negative is a cross and makes the repair fail.
    pub worst_pinch: TemperatureInterval,
    /// How many nodes had a **pre-repair** cold temperature at saturation, so
    /// their pre-repair enthalpy was not determined by the supplied state,
    /// dimensionless count. See [`energy_discrepancy`].
    pub saturation_ambiguous_nodes: usize,
    /// How much larger `|energy_discrepancy|` could legitimately be, given
    /// those ambiguous nodes \[J\], on the same one-second-of-flow inventory
    /// as the discrepancy itself.
    pub energy_discrepancy_uncertainty: Energy,
}

impl EnthalpyMarchDiagnostics {
    /// One-line human-readable dump of every field, for a log line or an error
    /// message. Deliberately reports the inferred quantities: a repair that
    /// went wrong almost always went wrong in the inference.
    pub fn report(&self) -> String {
        format!(
            "shoot_iters={} residual={:.3e} K node_iters={} C_hot={:.4e} W/K \
             cp_hot={:.1} J/(kg K) duty={:.4e} W worst_pinch={:.4} K \
             sat_ambiguous_nodes={} dE_uncertainty={:.4e} J",
            self.shooting_iterations,
            self.shooting_residual.get::<kelvin_interval>(),
            self.node_solver_iterations,
            self.inferred_hot_capacity_rate.get::<watt_per_kelvin>(),
            self.inferred_hot_specific_heat
                .get::<joule_per_kilogram_kelvin>(),
            self.duty.get::<watt>(),
            self.worst_pinch.get::<kelvin_interval>(),
            self.saturation_ambiguous_nodes,
            self.energy_discrepancy_uncertainty.get::<joule>(),
        )
    }
}

/// Repair a crossed profile with the BEDOK enthalpy march.
///
/// Marches mixture enthalpy along the cold stream and temperature along the hot
/// stream from a node-by-node energy balance, closing the counter-flow
/// two-point boundary value problem by shooting on the hot outlet temperature.
/// Rebuilds all three profiles -- hot, tube metal and cold -- from the result.
/// See the module documentation for the formulation, its provenance, and its
/// limits.
///
/// The returned profiles are a **steady** state, in the node ordering of
/// [`super::CrossRepairInputs`] (index 0 is the hot inlet end; the cold inlet is
/// the last index). The tube metal is *set* to the temperature the series
/// resistance implies at that steady state, not integrated.
///
/// # Errors
///
/// - [`CrossRepairError::BadInputs`] -- mismatched profile lengths, fewer than
///   [`MIN_NODE_COUNT`] nodes, a non-positive flow, conductance or pressure, a
///   non-finite temperature, a cold pressure or inlet enthalpy outside the IF97
///   envelope, a hot inlet at or below the cold inlet, or a hot-side capacity
///   rate that cannot be inferred from the supplied state.
/// - [`CrossRepairError::DidNotConverge`] -- the shooting bracket does not
///   bracket, it collapsed with the residual still above
///   [`SHOOTING_TOLERANCE_KELVIN`], a march walked the cold enthalpy outside the
///   IF97 `(p,h)` envelope, a node solve stalled, or **the converged profile
///   still contains a cross**. The message carries the measured residual and the
///   full [`EnthalpyMarchDiagnostics`].
///
/// A caller must not treat any error as "no repair needed".
pub fn repair(inputs: &CrossRepairInputs) -> Result<CrossRepairOutcome, CrossRepairError> {
    repair_with_diagnostics(inputs).map(|(outcome, _)| outcome)
}

/// [`repair`], plus what the march measured about itself.
///
/// Identical arithmetic; [`repair`] is this function with the diagnostics
/// dropped. Use this one when the repair needs to be audited -- a test checking
/// the inferred hot `c_p`, or a caller logging why a fallback engaged.
///
/// # Errors
///
/// Exactly as [`repair`].
pub fn repair_with_diagnostics(
    inputs: &CrossRepairInputs,
) -> Result<(CrossRepairOutcome, EnthalpyMarchDiagnostics), CrossRepairError> {
    let ctx = MarchContext::from_inputs(inputs)?;

    let (march, shooting_iterations, residual) = shoot(&ctx)?;

    let hot_temperatures: Vec<ThermodynamicTemperature> = march
        .hot
        .iter()
        .map(|t| ThermodynamicTemperature::new::<kelvin>(*t))
        .collect();
    let cold_temperatures: Vec<ThermodynamicTemperature> = march
        .cold
        .iter()
        .map(|t| ThermodynamicTemperature::new::<kelvin>(*t))
        .collect();
    let metal_temperatures: Vec<ThermodynamicTemperature> = (0..ctx.n)
        .map(|i| {
            ThermodynamicTemperature::new::<kelvin>(quasi_steady_metal_temperature(
                &ctx,
                march.hot[i],
                march.cold[i],
            ))
        })
        .collect();

    let (discrepancy, ambiguous_nodes, uncertainty) = energy_discrepancy(inputs, &ctx, &march);

    let worst_pinch_k = (0..ctx.n).fold(f64::INFINITY, |worst, i| {
        worst.min(march.hot[i] - march.cold[i])
    });

    let diagnostics = EnthalpyMarchDiagnostics {
        shooting_iterations,
        shooting_residual: TemperatureInterval::new::<kelvin_interval>(residual),
        node_solver_iterations: march.node_iterations,
        inferred_hot_capacity_rate: ThermalConductance::new::<watt_per_kelvin>(ctx.c_hot),
        inferred_hot_specific_heat: SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(
            ctx.c_hot / ctx.m_hot,
        ),
        duty: Power::new::<watt>(march.duty),
        worst_pinch: TemperatureInterval::new::<kelvin_interval>(worst_pinch_k),
        saturation_ambiguous_nodes: ambiguous_nodes,
        energy_discrepancy_uncertainty: uncertainty,
    };

    // The one invariant that may never be traded away: the state handed back
    // must not contain a temperature cross. `worst_pinch <= 0` is exactly
    // `CrossRepairInputs::worst_cross_kelvin() >= 0` on the repaired profiles.
    if worst_pinch_k <= 0.0 {
        return Err(CrossRepairError::DidNotConverge(format!(
            "enthalpy march converged but its profile still crosses by {:.6e} K; \
             refusing to hand back a second-law violation ({})",
            -worst_pinch_k,
            diagnostics.report(),
        )));
    }

    Ok((
        CrossRepairOutcome {
            hot_temperatures,
            metal_temperatures,
            cold_temperatures,
            energy_discrepancy: discrepancy,
        },
        diagnostics,
    ))
}

// ---------------------------------------------------------------------------
// Validated inputs
// ---------------------------------------------------------------------------

/// The supplied inputs, validated and reduced to SI `f64` for the inner loops.
///
/// The public surface is `uom`-typed; the march itself runs on bare `f64` in SI
/// base units because it is a tight scalar iteration -- the same choice
/// `super::super::steam_generator::max_courant_over_speeds` makes, and for the
/// same reason. Every field's unit is stated here so the conversion happens
/// exactly once, at the boundary.
#[derive(Clone, Debug)]
struct MarchContext {
    /// Node count, dimensionless.
    n: usize,
    /// Hot film conductance per node \[W/K\].
    ua_hot: f64,
    /// Cold film conductance per node \[W/K\].
    ua_cold: f64,
    /// Series hot-to-cold conductance per node \[W/K\],
    /// `UA_hot*UA_cold/(UA_hot + UA_cold)`.
    u_series: f64,
    /// Hot-stream heat capacity rate \[W/K\], inferred (see module docs).
    c_hot: f64,
    /// Hot-stream mass flow \[kg/s\].
    m_hot: f64,
    /// Cold-stream mass flow \[kg/s\].
    m_cold: f64,
    /// Cold-side pressure, `uom`-typed because every IF97 call takes it.
    p_cold: Pressure,
    /// Cold-stream inlet specific enthalpy \[J/kg\], the boundary condition at
    /// node `n-1`.
    h_cold_in: f64,
    /// Cold-stream inlet temperature \[K\], from `t_ph_eqm` -- the lower end of
    /// the shooting bracket.
    t_cold_in: f64,
    /// Hot-stream inlet temperature \[K\], the boundary condition at node 0 and
    /// the upper end of the shooting bracket.
    t_hot_in: f64,
    /// Lowest cold-side enthalpy the IF97 `(p,h)` flash accepts at `p_cold`
    /// \[J/kg\] -- the 273.15 K isotherm.
    h_cold_min: f64,
    /// Highest cold-side enthalpy the IF97 `(p,h)` flash accepts at `p_cold`
    /// \[J/kg\] -- the 1073.15 K isotherm.
    h_cold_max: f64,
    /// Saturation temperature at `p_cold` \[K\], or `None` above the critical
    /// pressure where there is no dome.
    t_sat: Option<f64>,
    /// Saturated liquid and vapour enthalpies at `p_cold` \[J/kg\], or `None`
    /// above the critical pressure.
    saturation_enthalpies: Option<(f64, f64)>,
}

impl MarchContext {
    /// Validate [`CrossRepairInputs`] and infer what the march needs.
    ///
    /// # Errors
    ///
    /// [`CrossRepairError::BadInputs`] naming the specific defect. Every check
    /// here guards either an arithmetic degeneracy or a **panic** in
    /// `tampines-steam-tables`, which rejects out-of-range states by panicking
    /// rather than returning an error.
    fn from_inputs(inputs: &CrossRepairInputs) -> Result<Self, CrossRepairError> {
        let bad = |why: String| CrossRepairError::BadInputs(why);

        let n = inputs.node_count();
        if n < MIN_NODE_COUNT {
            return Err(bad(format!(
                "{n} node(s); the enthalpy march needs at least {MIN_NODE_COUNT}"
            )));
        }
        if inputs.metal_temperatures.len() != n || inputs.cold_temperatures.len() != n {
            return Err(bad(format!(
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
                let value = t.get::<kelvin>();
                if !value.is_finite() || value <= 0.0 {
                    return Err(bad(format!(
                        "{name} node {i} temperature is {value} K, which is not a \
                         finite positive absolute temperature"
                    )));
                }
            }
        }

        let ua_hot = inputs.hot_node_conductance.get::<watt_per_kelvin>();
        let ua_cold = inputs.cold_node_conductance.get::<watt_per_kelvin>();
        if !(ua_hot.is_finite() && ua_hot > 0.0) || !(ua_cold.is_finite() && ua_cold > 0.0) {
            return Err(bad(format!(
                "node conductances must be finite and positive; got hot {ua_hot} W/K, \
                 cold {ua_cold} W/K"
            )));
        }

        let m_hot = inputs.hot_mass_flow.get::<kilogram_per_second>();
        let m_cold = inputs.cold_mass_flow.get::<kilogram_per_second>();
        if !(m_hot.is_finite() && m_hot > 0.0) || !(m_cold.is_finite() && m_cold > 0.0) {
            return Err(bad(format!(
                "mass flows must be finite and positive; got hot {m_hot} kg/s, \
                 cold {m_cold} kg/s"
            )));
        }

        // IF97 pressure envelope. `tampines-steam-tables` panics outside it, so
        // this is checked before any steam call is made.
        let p_cold = inputs.cold_pressure;
        let p_cold_pa = p_cold.get::<pascal>();
        let p_min_pa = sat_pressure_4(ThermodynamicTemperature::new::<kelvin>(
            IF97_MIN_TEMPERATURE_K,
        ))
        .get::<pascal>();
        if !p_cold_pa.is_finite() || p_cold_pa < p_min_pa || p_cold_pa > IF97_MAX_PRESSURE_PA {
            return Err(bad(format!(
                "cold pressure {p_cold_pa} Pa is outside the IAPWS-IF97 envelope \
                 [{p_min_pa}, {IF97_MAX_PRESSURE_PA}] Pa"
            )));
        }
        if !inputs.hot_pressure.get::<pascal>().is_finite()
            || inputs.hot_pressure.get::<pascal>() <= 0.0
        {
            return Err(bad(format!(
                "hot pressure {} Pa is not finite and positive",
                inputs.hot_pressure.get::<pascal>()
            )));
        }

        let h_cold_min = h_tp_1(
            ThermodynamicTemperature::new::<kelvin>(IF97_MIN_TEMPERATURE_K),
            p_cold,
        )
        .get::<joule_per_kilogram>();
        let h_cold_max = h_tp_eqm_single_phase(
            ThermodynamicTemperature::new::<kelvin>(IF97_MAX_TEMPERATURE_K),
            p_cold,
        )
        .get::<joule_per_kilogram>();

        let h_cold_in = inputs.cold_inlet_enthalpy.get::<joule_per_kilogram>();
        if !h_cold_in.is_finite() || h_cold_in < h_cold_min || h_cold_in > h_cold_max {
            return Err(bad(format!(
                "cold inlet enthalpy {h_cold_in} J/kg is outside the IAPWS-IF97 (p,h) \
                 envelope [{h_cold_min:.4e}, {h_cold_max:.4e}] J/kg at \
                 {p_cold_pa:.4e} Pa"
            )));
        }
        let t_cold_in = t_ph_eqm(p_cold, inputs.cold_inlet_enthalpy).get::<kelvin>();

        let t_hot_in = inputs.hot_inlet_temperature.get::<kelvin>();
        if !t_hot_in.is_finite() || t_hot_in <= t_cold_in {
            return Err(bad(format!(
                "hot inlet {t_hot_in} K is not above the cold inlet {t_cold_in} K; \
                 a counter-flow exchanger has no admissible steady solution here"
            )));
        }

        // Saturation dome, for the pre-repair enthalpy reconstruction only.
        let (t_sat, saturation_enthalpies) = if p_cold_pa < WATER_CRITICAL_PRESSURE_PA {
            let t_sat = sat_temp_4(p_cold);
            (
                Some(t_sat.get::<kelvin>()),
                Some((
                    h_tp_1(t_sat, p_cold).get::<joule_per_kilogram>(),
                    h_tp_2(t_sat, p_cold).get::<joule_per_kilogram>(),
                )),
            )
        } else {
            (None, None)
        };

        // Hot-side heat capacity rate, inferred -- see the module docs.
        let node_duty = |i: usize| -> f64 {
            ua_hot
                * (inputs.hot_temperatures[i].get::<kelvin>()
                    - inputs.metal_temperatures[i].get::<kelvin>())
        };
        let metal_driven_duty: f64 = (0..n).map(node_duty).sum();
        let hot_drop = t_hot_in - inputs.hot_temperatures[n - 1].get::<kelvin>();
        if hot_drop < MIN_HOT_TEMPERATURE_DROP_KELVIN {
            return Err(bad(format!(
                "the supplied hot profile drops only {hot_drop} K from its inlet, below \
                 {MIN_HOT_TEMPERATURE_DROP_KELVIN} K; the hot-side heat capacity rate \
                 cannot be inferred from it"
            )));
        }
        // The half-node correction. `hot_drop` is measured to the last node's
        // *cell centre*, but the duty is rejected all the way to its outlet
        // *face*, half a node further on. Writing the telescoped march out,
        //
        //   T_in - (T[n-1] - 0.5*q[n-1]/C) = Q/C   =>   C = (Q - 0.5*q[n-1])/dT
        //
        // which is closed-form: `q[n-1]` is read from the supplied state like
        // the rest of `Q`. Without this term the inference is biased high by
        // half the last node's temperature step, measured at +5.97% on the
        // HTR-10 fixture -- see
        // `tests::the_repair_is_very_nearly_its_own_fixed_point`.
        let corrected_duty = metal_driven_duty - 0.5 * node_duty(n - 1);
        if !(corrected_duty.is_finite() && corrected_duty > 0.0) {
            return Err(bad(format!(
                "the supplied state transfers {metal_driven_duty} W from the hot stream to \
                 the metal ({corrected_duty} W after the half-node correction); the \
                 hot-side heat capacity rate cannot be inferred from it"
            )));
        }
        let c_hot = corrected_duty / hot_drop;
        let cp_hot = c_hot / m_hot;
        if !(MIN_PLAUSIBLE_HOT_CP..=MAX_PLAUSIBLE_HOT_CP).contains(&cp_hot) {
            return Err(bad(format!(
                "the hot-side specific heat inferred from the supplied state is \
                 {cp_hot:.4e} J/(kg K), outside the plausible band \
                 [{MIN_PLAUSIBLE_HOT_CP}, {MAX_PLAUSIBLE_HOT_CP}]; the supplied state is \
                 not a consistent exchanger state"
            )));
        }

        Ok(Self {
            n,
            ua_hot,
            ua_cold,
            u_series: ua_hot * ua_cold / (ua_hot + ua_cold),
            c_hot,
            m_hot,
            m_cold,
            p_cold,
            h_cold_in,
            t_cold_in,
            t_hot_in,
            h_cold_min,
            h_cold_max,
            t_sat,
            saturation_enthalpies,
        })
    }
}

/// Tube-metal temperature implied by the series resistance at a steady state
/// \[K\].
///
/// The metal has no advection, so at steady state the heat entering it from the
/// hot stream equals the heat leaving it to the cold stream:
/// `UA_hot*(T_hot - T_metal) = UA_cold*(T_metal - T_cold)`, giving the
/// conductance-weighted mean. It therefore always lies between the two fluid
/// temperatures, so a cross-free fluid pair gives a cross-free metal.
fn quasi_steady_metal_temperature(ctx: &MarchContext, t_hot: f64, t_cold: f64) -> f64 {
    (ctx.ua_hot * t_hot + ctx.ua_cold * t_cold) / (ctx.ua_hot + ctx.ua_cold)
}

// ---------------------------------------------------------------------------
// The march
// ---------------------------------------------------------------------------

/// Why one march could not be completed. Never a reason to return a profile.
#[derive(Clone, Copy, Debug, PartialEq)]
enum MarchFailure {
    /// The marched cold enthalpy left the IAPWS-IF97 `(p,h)` envelope, where
    /// the flash has no answer (and panics if asked).
    ColdEnthalpyOutsideIf97 {
        /// Node index at which it happened.
        node: usize,
        /// The offending specific enthalpy \[J/kg\].
        enthalpy: f64,
    },
    /// A node's power balance did not converge.
    NodeStalled {
        /// Node index.
        node: usize,
        /// Residual of `q - U*(T_hot - T_cold)` at the last iterate \[W\].
        residual: f64,
    },
}

impl MarchFailure {
    /// Human-readable form, for the error the caller sees.
    fn describe(&self) -> String {
        match self {
            Self::ColdEnthalpyOutsideIf97 { node, enthalpy } => format!(
                "the cold enthalpy march left the IAPWS-IF97 (p,h) envelope at node {node} \
                 with h = {enthalpy:.6e} J/kg"
            ),
            Self::NodeStalled { node, residual } => {
                format!("the power balance at node {node} stalled with residual {residual:.6e} W")
            }
        }
    }
}

/// One completed march: the steady profiles implied by a guessed hot outlet.
#[derive(Clone, Debug)]
struct March {
    /// Hot-stream cell-centre temperatures \[K\], index 0 at the hot inlet end.
    hot: Vec<f64>,
    /// Cold-stream cell-centre specific enthalpies \[J/kg\], same ordering.
    cold_enthalpy: Vec<f64>,
    /// Cold-stream cell-centre temperatures \[K\], `t_ph_eqm` of the above.
    cold: Vec<f64>,
    /// Total transferred power `sum_i q_i` \[W\].
    duty: f64,
    /// Hot inlet temperature the march implies at node 0's inlet face \[K\].
    /// The shooting residual is this minus the given hot inlet.
    implied_hot_inlet: f64,
    /// Inner iterations spent, dimensionless count.
    node_iterations: usize,
}

/// Cold-stream temperature at the marched enthalpy \[K\], or a failure if the
/// point is outside the IF97 `(p,h)` envelope.
///
/// The range check is **not** a clamp: an out-of-range enthalpy aborts the
/// march rather than being pulled back to the boundary. `tampines-steam-tables`
/// panics outside its envelope, so the check must happen here.
///
/// # Errors
///
/// [`MarchFailure::ColdEnthalpyOutsideIf97`].
fn cold_temperature(ctx: &MarchContext, node: usize, enthalpy: f64) -> Result<f64, MarchFailure> {
    if !enthalpy.is_finite() || enthalpy < ctx.h_cold_min || enthalpy > ctx.h_cold_max {
        return Err(MarchFailure::ColdEnthalpyOutsideIf97 { node, enthalpy });
    }
    Ok(t_ph_eqm(
        ctx.p_cold,
        AvailableEnergy::new::<joule_per_kilogram>(enthalpy),
    )
    .get::<kelvin>())
}

/// Solve one node's implicit power balance and return `(q_i, iterations)`.
///
/// The half-node march makes each node implicit in its own transferred power:
///
/// ```text
/// T_hot,i  = a + 0.5*q_i/C_hot          (a = the upstream half-step base)
/// h_cold,i = b + 0.5*q_i/m_cold         (b = likewise for the cold stream)
/// q_i      = U*(T_hot,i - T_cold(h_cold,i))
/// ```
///
/// so the residual `F(q) = U*(T_hot(q) - T_cold(q)) - q` is driven to zero. `F`
/// is smooth and, for any node whose hot-side `U/C_hot` is below 2, strictly
/// decreasing, so a **secant** iteration converges quickly from the previous
/// node's power. Where a step would leave the IF97 envelope it is **backtracked
/// toward the last valid point** rather than clamped -- and if backtracking
/// cannot recover, the failure propagates.
///
/// # Errors
///
/// [`MarchFailure`] if the IF97 flash cannot be evaluated or the iteration
/// stalls above [`NODE_TOLERANCE_WATT`].
fn solve_node_power(
    ctx: &MarchContext,
    node: usize,
    a: f64,
    b: f64,
    initial_guess: f64,
) -> Result<(f64, usize), MarchFailure> {
    let residual = |q: f64| -> Result<f64, MarchFailure> {
        let t_hot = a + 0.5 * q / ctx.c_hot;
        let t_cold = cold_temperature(ctx, node, b + 0.5 * q / ctx.m_cold)?;
        Ok(ctx.u_series * (t_hot - t_cold) - q)
    };

    // Two starting points for the secant. The first is the caller's guess (the
    // previous node's power, which is usually close); the second is displaced
    // by a fraction of the local duty scale.
    let mut q0 = initial_guess;
    let mut f0 = match residual(q0) {
        Ok(f) => f,
        Err(_) => {
            // The guess itself is unusable; fall back to the explicit estimate,
            // which cannot overshoot because it applies no half-step at all.
            q0 = 0.0;
            residual(q0)?
        }
    };
    let scale = (ctx.u_series * (a - ctx.t_cold_in).abs()).max(1.0);
    let mut q1 = q0 + 1.0e-3 * scale;
    let mut f1 = residual(q1)?;
    let mut iterations = 2usize;

    for _ in 0..MAX_NODE_ITERATIONS {
        let tolerance = NODE_TOLERANCE_WATT + NODE_RELATIVE_TOLERANCE * q1.abs();
        if f1.abs() <= tolerance {
            return Ok((q1, iterations));
        }
        let denominator = f1 - f0;
        if denominator == 0.0 {
            return Err(MarchFailure::NodeStalled { node, residual: f1 });
        }
        let mut q2 = q1 - f1 * (q1 - q0) / denominator;
        if !q2.is_finite() {
            return Err(MarchFailure::NodeStalled { node, residual: f1 });
        }
        // Backtrack rather than clamp if the step leaves the steam envelope.
        let mut f2 = residual(q2);
        let mut backtracks = 0;
        while f2.is_err() && backtracks < 20 {
            q2 = 0.5 * (q2 + q1);
            f2 = residual(q2);
            backtracks += 1;
        }
        let f2 = f2?;
        iterations += 1 + backtracks;

        if (q2 - q1).abs() <= 1.0e-14 * q2.abs().max(1.0) {
            let tolerance = NODE_TOLERANCE_WATT + NODE_RELATIVE_TOLERANCE * q2.abs();
            if f2.abs() <= tolerance {
                return Ok((q2, iterations));
            }
            return Err(MarchFailure::NodeStalled { node, residual: f2 });
        }
        q0 = q1;
        f0 = f1;
        q1 = q2;
        f1 = f2;
    }

    // The loop checks its tolerance on entry, so the last update it makes is
    // never tested inside it. Test it here rather than failing a converged
    // node on a counting technicality.
    if f1.abs() <= NODE_TOLERANCE_WATT + NODE_RELATIVE_TOLERANCE * q1.abs() {
        return Ok((q1, iterations));
    }
    Err(MarchFailure::NodeStalled { node, residual: f1 })
}

/// March both streams from the node `n-1` end, given a guessed hot outlet face
/// temperature \[K\].
///
/// This is Than Yan Ren's half-node scheme applied twice. Marching from node
/// `n-1` toward node 0 runs **downstream** along the cold stream -- exactly the
/// direction his `FlowDirection::Downward` branch marches -- and **upstream**
/// along the hot stream, which is the identical trapezoidal relation read in
/// reverse. Both accumulate the same way:
///
/// ```text
/// value[n-1] = boundary + 0.5*delta[n-1]
/// value[i]   = value[i+1] + 0.5*delta[i+1] + 0.5*delta[i]
/// ```
///
/// with `delta` the hot temperature drop `q_i/C_hot` \[K\] or the cold enthalpy
/// rise `q_i/m_cold` \[J/kg\] across node `i`.
///
/// # Errors
///
/// [`MarchFailure`] -- propagated from the per-node solve.
fn march(ctx: &MarchContext, t_hot_out_face: f64) -> Result<March, MarchFailure> {
    let mut hot = vec![0.0_f64; ctx.n];
    let mut cold_enthalpy = vec![0.0_f64; ctx.n];
    let mut cold = vec![0.0_f64; ctx.n];

    // Half-step accumulators: the value at the upstream face of node `i`.
    let mut hot_base = t_hot_out_face;
    let mut cold_base = ctx.h_cold_in;
    let mut duty = 0.0;
    let mut node_iterations = 0usize;
    let mut previous_power = 0.0;

    for i in (0..ctx.n).rev() {
        let (q, iterations) = solve_node_power(ctx, i, hot_base, cold_base, previous_power)?;
        node_iterations += iterations;

        let t_hot = hot_base + 0.5 * q / ctx.c_hot;
        let h_cold = cold_base + 0.5 * q / ctx.m_cold;
        let t_cold = cold_temperature(ctx, i, h_cold)?;

        hot[i] = t_hot;
        cold_enthalpy[i] = h_cold;
        cold[i] = t_cold;

        hot_base = t_hot + 0.5 * q / ctx.c_hot;
        cold_base = h_cold + 0.5 * q / ctx.m_cold;
        duty += q;
        previous_power = q;
    }

    Ok(March {
        hot,
        cold_enthalpy,
        cold,
        duty,
        implied_hot_inlet: hot_base,
        node_iterations,
    })
}

/// Close the two-point boundary value problem: find the hot outlet face
/// temperature \[K\] whose march reproduces the given hot inlet.
///
/// Returns the converged march, the iteration count, and the final residual
/// \[K\]. See the module documentation for why the bracket is
/// `[T_cold_inlet, T_hot_inlet]` and why the iteration is Illinois rather than
/// Newton.
///
/// # Errors
///
/// [`CrossRepairError::DidNotConverge`] if the bracket does not bracket, if it
/// collapses to floating-point resolution with the residual still above
/// [`SHOOTING_TOLERANCE_KELVIN`], if the cap is reached, or if a march fails.
fn shoot(ctx: &MarchContext) -> Result<(March, usize, f64), CrossRepairError> {
    let not_converged = |why: String| CrossRepairError::DidNotConverge(why);

    let evaluate = |guess: f64| -> Result<(March, f64), MarchFailure> {
        let m = march(ctx, guess)?;
        let residual = m.implied_hot_inlet - ctx.t_hot_in;
        Ok((m, residual))
    };

    // Lower end: a hot outlet at the cold inlet temperature -- the second-law
    // limit, and the *least*-duty admissible guess. Marchability is monotone in
    // the guess (a hotter guess means more duty and a hotter cold stream), so
    // if this end cannot be marched, no guess can.
    let mut lo = ctx.t_cold_in;
    let (march_lo, mut f_lo) = evaluate(lo).map_err(|e| {
        not_converged(format!(
            "the least-duty end of the shooting bracket, a {lo:.4} K hot outlet, could \
             not be marched, so no guess can be: {}",
            e.describe()
        ))
    })?;
    if f_lo.abs() <= SHOOTING_TOLERANCE_KELVIN {
        return Ok((march_lo, 0, f_lo));
    }
    if f_lo > 0.0 {
        return Err(not_converged(format!(
            "the shooting bracket does not bracket a root: the least-duty admissible \
             guess, a {lo:.4} K hot outlet, already implies a {:.4} K hot inlet against \
             the {:.4} K given (residual {f_lo:.6e} K). The exchanger as specified \
             cannot reject its duty without the hot stream leaving below the cold \
             inlet, which the second law forbids",
            march_lo.implied_hot_inlet, ctx.t_hot_in
        )));
    }

    // Upper end: bisect on marchability between the lower end and the hot
    // inlet. The hot inlet itself is the most violent march there is and often
    // drives the cold stream out of the IF97 envelope, so the first guess that
    // both marches and has a positive residual is taken as the upper end.
    let span = ctx.t_hot_in - lo;
    let mut valid_fraction = 0.0_f64;
    let mut invalid_fraction = f64::INFINITY;
    let mut fraction = 1.0_f64;
    let mut upper: Option<(f64, March, f64)> = None;
    let mut last_failure: Option<MarchFailure> = None;
    let mut walks = 0usize;
    for _ in 0..MAX_BRACKET_WALKS {
        walks += 1;
        let candidate = lo + fraction * span;
        match evaluate(candidate) {
            Ok((m, f)) if f > 0.0 => {
                upper = Some((candidate, m, f));
                break;
            }
            Ok(_) => {
                // Marchable but still below the root; step up, staying under
                // the lowest guess known to be unmarchable.
                valid_fraction = fraction;
                if !invalid_fraction.is_finite() {
                    break;
                }
                fraction = 0.5 * (valid_fraction + invalid_fraction);
            }
            Err(failure) => {
                last_failure = Some(failure);
                invalid_fraction = fraction;
                fraction = 0.5 * (valid_fraction + invalid_fraction);
            }
        }
    }

    let (mut hi, march_hi, mut f_hi) = match upper {
        Some(found) => found,
        None => {
            return Err(not_converged(match last_failure {
                Some(failure) => format!(
                    "no marchable upper end of the shooting bracket after {walks} \
                     bisections on marchability, between a {lo:.4} K hot outlet and the \
                     {:.4} K hot inlet; the last failure was: {}",
                    ctx.t_hot_in,
                    failure.describe()
                ),
                None => format!(
                    "the shooting bracket does not bracket a root: even a hot outlet at \
                     the {:.4} K hot inlet implies an inlet below it, so the supplied \
                     state is not a counter-flow exchanger heating the cold stream",
                    ctx.t_hot_in
                ),
            }));
        }
    };
    if f_hi.abs() <= SHOOTING_TOLERANCE_KELVIN {
        return Ok((march_hi, walks, f_hi));
    }

    // Illinois regula falsi. `lo`/`hi` hold opposite-signed residuals at all
    // times, so no iterate can leave the physical interval.
    for iteration in 1..=MAX_SHOOTING_ITERATIONS {
        let denominator = f_hi - f_lo;
        if denominator == 0.0 {
            return Err(not_converged(format!(
                "the shooting residual went flat at {f_hi:.6e} K after {iteration} \
                 iterations"
            )));
        }
        let guess = (lo * f_hi - hi * f_lo) / denominator;
        let (candidate, f_candidate) = evaluate(guess).map_err(|e| {
            not_converged(format!(
                "the march failed at a {guess:.6} K hot outlet, inside a valid bracket, \
                 after {iteration} iterations: {}",
                e.describe()
            ))
        })?;

        if f_candidate.abs() <= SHOOTING_TOLERANCE_KELVIN {
            return Ok((candidate, walks + iteration, f_candidate));
        }

        if f_candidate * f_hi < 0.0 {
            // Sign changed: the old upper end becomes the new lower end.
            lo = hi;
            f_lo = f_hi;
        } else {
            // Same side twice: halve the retained end's weight. This is the
            // Illinois modification, and it is what stops regula falsi from
            // creeping toward the root from one side only.
            f_lo *= 0.5;
        }
        hi = guess;
        f_hi = f_candidate;

        if (hi - lo).abs() <= 1.0e-13 * hi.abs().max(1.0) {
            return Err(not_converged(format!(
                "the shooting bracket collapsed to f64 resolution ({lo:.12} K to \
                 {hi:.12} K) with residual {f_hi:.6e} K still above the \
                 {SHOOTING_TOLERANCE_KELVIN:.1e} K tolerance after {iteration} \
                 iterations; the exchanger is ill-conditioned at this node count"
            )));
        }
    }

    Err(not_converged(format!(
        "the shoot did not reach the {SHOOTING_TOLERANCE_KELVIN:.1e} K tolerance in \
         {MAX_SHOOTING_ITERATIONS} iterations; last residual {f_hi:.6e} K over a bracket \
         {lo:.9} K to {hi:.9} K"
    )))
}

// ---------------------------------------------------------------------------
// First-law bookkeeping
// ---------------------------------------------------------------------------

/// Energy the repair added to the exchanger's stream inventories \[J\], the
/// count of nodes whose pre-repair cold state was thermodynamically ambiguous,
/// and how much larger the magnitude could legitimately be \[J\].
///
/// # What is computed
///
/// Overwriting both profiles changes the energy stored in them discontinuously.
/// The second law is respected pointwise by construction; the **first law is
/// not**, and this is the audit of that. Positive means the repair *added*
/// energy.
///
/// ```text
/// dE = sum_i [ M_hot*c_p_hot*(T_hot,i_after - T_hot,i_before)
///            + M_cold*(h_cold,i_after - h_cold,i_before) ]
/// ```
///
/// # The inventory is stated, because the contract does not carry it
///
/// `M_hot` and `M_cold` are the per-node masses. [`CrossRepairInputs`] carries
/// mass **flows**, not node masses, volumes, or transit times, so those masses
/// are not determined by the contract. They are therefore taken as
/// `m_dot * `[`REFERENCE_INVENTORY_TIME_S`]` / n` -- **one second of flow per
/// stream, spread evenly over the nodes.** To get the physical figure, multiply
/// by the actual stream transit time in seconds. This is stated rather than
/// stubbed to zero, and it is the one place where extending
/// [`CrossRepairInputs`] (with node masses, and the metal's heat capacity)
/// would make the number exact instead of referred.
///
/// **The tube metal is not in this sum**, for the same reason: no mass or heat
/// capacity for it exists in the contract. The repair does rewrite the metal
/// profile, so the reported figure understates the true discrepancy by the
/// metal's contribution.
///
/// # The two-phase ambiguity, and why the result is a lower bound
///
/// The pre-repair cold state is supplied as a **temperature**. Inside the
/// saturation dome, temperature does not determine enthalpy -- the whole reason
/// [`CrossRepairInputs::cold_inlet_enthalpy`] is an enthalpy. For a node whose
/// pre-repair temperature is within [`SATURATION_TOLERANCE_KELVIN`] of
/// `Tsat(p)`, the pre-repair enthalpy is only known to lie in `[h_f, h_g]`; this
/// takes the value in that interval **closest to the repaired enthalpy**, which
/// is the smallest change consistent with the supplied state. The reported
/// discrepancy is therefore a **lower bound in magnitude**, and the third
/// return value is how much larger it could be (the latent heat times those
/// nodes' inventory).
///
/// A pre-repair cold temperature outside the IF97 range would be a panic in the
/// `(T,p)` flash; those nodes are counted as ambiguous instead, on the same
/// closest-admissible rule, rather than the whole repair being failed for a
/// bookkeeping input.
fn energy_discrepancy(
    inputs: &CrossRepairInputs,
    ctx: &MarchContext,
    march: &March,
) -> (Energy, usize, Energy) {
    let hot_node_mass = ctx.m_hot * REFERENCE_INVENTORY_TIME_S / ctx.n as f64;
    let cold_node_mass = ctx.m_cold * REFERENCE_INVENTORY_TIME_S / ctx.n as f64;
    let cp_hot = ctx.c_hot / ctx.m_hot;

    let mut joules = 0.0_f64;
    let mut ambiguous = 0usize;
    let mut uncertainty = 0.0_f64;

    for i in 0..ctx.n {
        let t_hot_before = inputs.hot_temperatures[i].get::<kelvin>();
        joules += hot_node_mass * cp_hot * (march.hot[i] - t_hot_before);

        let t_cold_before = inputs.cold_temperatures[i].get::<kelvin>();
        let h_after = march.cold_enthalpy[i];

        let saturated = match ctx.t_sat {
            Some(t_sat) => (t_cold_before - t_sat).abs() <= SATURATION_TOLERANCE_KELVIN,
            None => false,
        };
        let out_of_range =
            !(IF97_MIN_TEMPERATURE_K..=IF97_MAX_TEMPERATURE_K).contains(&t_cold_before);

        let h_before = if saturated || out_of_range {
            ambiguous += 1;
            if let Some((h_f, h_g)) = ctx.saturation_enthalpies {
                uncertainty += cold_node_mass * (h_g - h_f);
                h_after.clamp(h_f, h_g)
            } else {
                // No dome at this pressure: the only ambiguity left is an
                // out-of-range temperature, whose enthalpy cannot be bounded
                // at all. Take no change and declare the uncertainty
                // unbounded-in-practice by the envelope width.
                uncertainty += cold_node_mass * (ctx.h_cold_max - ctx.h_cold_min);
                h_after
            }
        } else {
            h_tp_eqm_single_phase(
                ThermodynamicTemperature::new::<kelvin>(t_cold_before),
                ctx.p_cold,
            )
            .get::<joule_per_kilogram>()
        };

        joules += cold_node_mass * (h_after - h_before);
    }

    (
        Energy::new::<joule>(joules),
        ambiguous,
        Energy::new::<joule>(uncertainty),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::MassRate;

    /// Cold-side pressure of every fixture here: the HTR-10 live-steam
    /// pressure, 4.0 MPa.
    fn cold_pressure() -> Pressure {
        Pressure::new::<pascal>(4.0e6)
    }

    /// Enthalpy of water at `t_kelvin` and 4.0 MPa \[J/kg\], from IF97.
    fn cold_enthalpy_at(t_kelvin: f64) -> AvailableEnergy {
        h_tp_eqm_single_phase(
            ThermodynamicTemperature::new::<kelvin>(t_kelvin),
            cold_pressure(),
        )
    }

    /// An HTR-10-like 8-node steam generator, in the units and node ordering of
    /// [`CrossRepairInputs`].
    ///
    /// The conductances are `super::super::super::primary_loop`'s illustrative
    /// `UA` of 4.26e4 W/K with 75% of the resistance on the helium side, split
    /// over 8 nodes: 7100 W/K hot and 21300 W/K cold per node. Flows and
    /// terminal states are that simulator's design point (4.3 kg/s helium at
    /// 3 MPa entering at 973.15 K; 3.19 kg/s feedwater at 4 MPa entering at
    /// 313.15 K).
    ///
    /// `cold` is supplied by the caller so a test can inject a cross.
    fn htr10_inputs(hot: [f64; 8], cold: [f64; 8]) -> CrossRepairInputs {
        let ua_hot = 7100.0;
        let ua_cold = 21300.0;
        let metal: Vec<ThermodynamicTemperature> = (0..8)
            .map(|i| {
                ThermodynamicTemperature::new::<kelvin>(
                    (ua_hot * hot[i] + ua_cold * cold[i]) / (ua_hot + ua_cold),
                )
            })
            .collect();
        CrossRepairInputs {
            hot_temperatures: hot
                .iter()
                .map(|t| ThermodynamicTemperature::new::<kelvin>(*t))
                .collect(),
            metal_temperatures: metal,
            cold_temperatures: cold
                .iter()
                .map(|t| ThermodynamicTemperature::new::<kelvin>(*t))
                .collect(),
            hot_node_conductance: ThermalConductance::new::<watt_per_kelvin>(ua_hot),
            cold_node_conductance: ThermalConductance::new::<watt_per_kelvin>(ua_cold),
            hot_mass_flow: MassRate::new::<kilogram_per_second>(4.3),
            cold_mass_flow: MassRate::new::<kilogram_per_second>(3.19),
            hot_inlet_temperature: ThermodynamicTemperature::new::<kelvin>(973.15),
            cold_inlet_enthalpy: cold_enthalpy_at(313.15),
            hot_pressure: Pressure::new::<pascal>(3.0e6),
            cold_pressure: cold_pressure(),
        }
    }

    /// A physically plausible once-through profile: helium falling 973 to 546 K
    /// against water that is subcooled at the feed end, boiling in the middle
    /// (at exactly `Tsat(4 MPa)`, so the two-phase nodes really are ambiguous
    /// in temperature, as they are in the array this stands in for) and
    /// superheated at the outlet. No cross.
    fn uncrossed_htr10() -> CrossRepairInputs {
        let t_sat = sat_temp_4(cold_pressure()).get::<kelvin>();
        htr10_inputs(
            [973.15, 930.0, 880.0, 820.0, 750.0, 680.0, 610.0, 546.0],
            [713.0, 600.0, t_sat, t_sat, t_sat, t_sat, 480.0, 313.15],
        )
    }

    /// The same exchanger with the cold stream driven 40 K past the hot stream
    /// at one node -- the shape a Lie-split coupling failure takes at a coarse
    /// coupling step.
    fn crossed_htr10() -> CrossRepairInputs {
        let t_sat = sat_temp_4(cold_pressure()).get::<kelvin>();
        htr10_inputs(
            [973.15, 930.0, 880.0, 820.0, 750.0, 680.0, 610.0, 546.0],
            [713.0, 600.0, t_sat, t_sat, t_sat, t_sat, 650.0, 313.15],
        )
    }

    /// A deliberately **wholly single-phase** case, for the checks that need an
    /// analytic reference or an unambiguous enthalpy.
    ///
    /// Two changes from the HTR-10 fixture make it so: the hot inlet is 520 K,
    /// **below** `Tsat(4 MPa)` = 523.5 K, so the water cannot reach saturation
    /// however the march behaves; and the cold flow is raised to 30 kg/s so the
    /// water warms only tens of kelvin, over which its `c_p` is constant to
    /// well under a percent. Both matter: the first removes the phase change,
    /// the second removes the property variation, leaving the **discretisation
    /// of the marching scheme** as the only thing an analytic comparison can be
    /// measuring.
    fn single_phase_case() -> CrossRepairInputs {
        let hot = [520.0, 490.0, 462.0, 437.0, 415.0, 396.0, 380.0, 367.0];
        let cold = [349.0, 345.0, 341.0, 337.0, 333.0, 329.0, 321.0, 313.15];
        let mut inputs = htr10_inputs(hot, cold);
        inputs.hot_inlet_temperature = ThermodynamicTemperature::new::<kelvin>(520.0);
        inputs.cold_mass_flow = MassRate::new::<kilogram_per_second>(30.0);
        inputs
    }

    /// Worst cross \[K\] in a repaired outcome: the largest amount by which the
    /// cold stream exceeds the hot, **not** floored at zero.
    ///
    /// [`CrossRepairInputs::worst_cross_kelvin`] folds from `0.0`, so it reports
    /// exactly `0.0` for any cross-free profile and cannot show how much margin
    /// there is. This reports the signed margin, so a test can assert a real
    /// pinch rather than merely the absence of a cross. Negative is cross-free.
    fn worst_cross_margin_of(outcome: &CrossRepairOutcome) -> f64 {
        outcome
            .hot_temperatures
            .iter()
            .zip(outcome.cold_temperatures.iter())
            .fold(f64::NEG_INFINITY, |worst, (h, c)| {
                worst.max(c.get::<kelvin>() - h.get::<kelvin>())
            })
    }

    /// The floored measure the contract uses, applied to an outcome: exactly
    /// `0.0` when the profile is cross-free.
    fn floored_worst_cross_of(outcome: &CrossRepairOutcome) -> f64 {
        worst_cross_margin_of(outcome).max(0.0)
    }

    /// **Verification against a closed-form reference.**
    ///
    /// **Methodology.** The march has no analytic solution once the cold stream
    /// boils, but in a wholly single-phase case it does: a counter-flow
    /// exchanger with constant capacity rates has the textbook
    /// effectiveness-NTU solution
    ///
    /// ```text
    /// eps = (1 - exp(-NTU*(1-Cr))) / (1 - Cr*exp(-NTU*(1-Cr))),  Cr = Cmin/Cmax
    /// Q   = eps*Cmin*(T_hot_in - T_cold_in)
    /// ```
    ///
    /// The case is [`single_phase_case`]: the HTR-10 exchanger's conductances,
    /// a hot inlet at 520 K (below `Tsat` = 523.5 K, so the water cannot boil
    /// however the march behaves) and a 30 kg/s cold flow (so the water warms
    /// only ~34 K, over which its `c_p` is constant to well under a percent).
    /// `C_hot` is the value the remedy infers from the supplied profile,
    /// `C_cold` is `m_dot*c_p` with `c_p` from IF97 at the mean cold
    /// temperature, so the reference uses the *same* capacity rates the march
    /// does and the comparison isolates the **marching scheme** rather than the
    /// properties. The march reports cell centres, so both outlets are
    /// extrapolated half a node with their own node's power before comparing,
    /// face to face. Pass criterion: both outlet temperatures within 2 K of the
    /// closed form.
    ///
    /// # Results (2026-08-13)
    ///
    /// `NTU = 1.5856`, `Cr = 0.2146`, `eps = 0.759045`.
    ///
    /// | Quantity | Analytic eps-NTU | Enthalpy march | Difference |
    /// |---|---|---|---|
    /// | Hot outlet \[K\] | 362.999 | 362.864 | -0.135 |
    /// | Cold outlet \[K\] | 346.858 | 346.869 | +0.011 |
    /// | Duty \[MW\] | 4.217983 | 4.221609 | +0.0036 (+0.086%) |
    ///
    /// Converged in 5 Illinois iterations to a residual of 2.3e-13 K, spending
    /// 32 node solves.
    ///
    /// Interpretation: the half-node march reproduces the closed-form
    /// counter-flow solution to **0.135 K on the hot outlet (0.086% of its
    /// 157 K span)** and **0.011 K on the cold (0.033% of its 33.7 K span)**.
    /// What is left is the second-order discretisation error of an 8-node
    /// cell-centred march at a node NTU of 0.20, plus the residual `c_p`
    /// variation the analytic reference cannot represent. **The marching scheme
    /// is verified against an analytic reference; nothing here validates the
    /// remedy against a measured exchanger, and the phase-change path -- the
    /// reason the method was chosen -- has no analytic reference to be verified
    /// against at all.**
    #[test]
    fn the_march_reproduces_the_analytic_counter_flow_solution() {
        use tampines_steam_tables::interfaces::functional_programming::pt_flash_eqm::cp_tp_eqm_single_phase;

        let ua_hot = 7100.0;
        let ua_cold = 21300.0;
        let inputs = single_phase_case();
        let cold_flow = inputs.cold_mass_flow.get::<kilogram_per_second>();
        let t_hot_in = inputs.hot_inlet_temperature.get::<kelvin>();

        let (outcome, diagnostics) =
            repair_with_diagnostics(&inputs).expect("single-phase case must march");

        // Capacity rates: the inferred hot one, and water's at the mean cold
        // temperature of the repaired profile.
        let c_hot = diagnostics
            .inferred_hot_capacity_rate
            .get::<watt_per_kelvin>();
        let t_cold_mean = 0.5
            * (outcome.cold_temperatures[0].get::<kelvin>()
                + outcome.cold_temperatures[7].get::<kelvin>());
        let cp_cold = cp_tp_eqm_single_phase(
            ThermodynamicTemperature::new::<kelvin>(t_cold_mean),
            cold_pressure(),
        )
        .get::<joule_per_kilogram_kelvin>();
        let c_cold = cold_flow * cp_cold;

        let ua_total = 8.0 * ua_hot * ua_cold / (ua_hot + ua_cold);
        let c_min = c_hot.min(c_cold);
        let c_max = c_hot.max(c_cold);
        let ntu = ua_total / c_min;
        let c_r = c_min / c_max;
        let e = (-ntu * (1.0 - c_r)).exp();
        let effectiveness = (1.0 - e) / (1.0 - c_r * e);
        let t_cold_in = t_ph_eqm(cold_pressure(), inputs.cold_inlet_enthalpy).get::<kelvin>();
        let duty = effectiveness * c_min * (t_hot_in - t_cold_in);
        let analytic_hot_out = t_hot_in - duty / c_hot;
        let analytic_cold_out = t_cold_in + duty / c_cold;

        // The march reports cell centres; the outlet faces are half a node
        // beyond them. Extrapolate with the last node's own half-step so the
        // comparison is face-to-face.
        let q_last = ua_total / 8.0
            * (outcome.hot_temperatures[7].get::<kelvin>()
                - outcome.cold_temperatures[7].get::<kelvin>());
        let march_hot_out = outcome.hot_temperatures[7].get::<kelvin>() - 0.5 * q_last / c_hot;
        let q_first = ua_total / 8.0
            * (outcome.hot_temperatures[0].get::<kelvin>()
                - outcome.cold_temperatures[0].get::<kelvin>());
        let march_cold_out = outcome.cold_temperatures[0].get::<kelvin>() + 0.5 * q_first / c_cold;

        println!(
            "eps-NTU: NTU={ntu:.4} Cr={c_r:.4} eps={effectiveness:.6} Q={duty:.6e} W \
             hot_out={analytic_hot_out:.3} K cold_out={analytic_cold_out:.3} K"
        );
        println!(
            "march  : Q={:.6e} W hot_out={march_hot_out:.3} K cold_out={march_cold_out:.3} K",
            diagnostics.duty.get::<watt>()
        );
        println!("diagnostics: {}", diagnostics.report());

        assert!(
            (march_hot_out - analytic_hot_out).abs() < 2.0,
            "hot outlet {march_hot_out} K against analytic {analytic_hot_out} K"
        );
        assert!(
            (march_cold_out - analytic_cold_out).abs() < 2.0,
            "cold outlet {march_cold_out} K against analytic {analytic_cold_out} K"
        );
        assert!(
            worst_cross_margin_of(&outcome) < 0.0,
            "the single-phase case must also come out cross-free"
        );
    }

    /// **The invariant the whole remedy exists for: the cross is gone.**
    ///
    /// **Methodology.** Take the HTR-10-like 8-node profile and drive cold node
    /// 6 to 650 K against a hot node at 610 K -- a 40.0 K cross, the shape a
    /// Lie-split coupling failure takes at a coarse coupling step. Repair it and
    /// measure the worst cross of the result, by the same definition
    /// [`CrossRepairInputs::worst_cross_kelvin`] uses. Pass criterion: the
    /// repaired worst cross is strictly negative, i.e. every node has the hot
    /// stream above the cold.
    ///
    /// # Results (2026-08-13)
    ///
    /// Supplied worst cross **+40.000 K**; repaired worst cross **-146.663 K**,
    /// i.e. the tightest node now carries a 146.66 K pinch, and the contract's
    /// own floored measure reports exactly 0.0 K of cross. Duty 9.5542e6 W over
    /// an inferred hot capacity rate of 2.1225e4 W/K -- **4935.9 J/(kg K),
    /// within 4.9% of helium's 5193 J/(kg K)**, which is as close as an
    /// inference from an illustrative profile has any right to be. Converged in
    /// 9 Illinois iterations to a residual of 3.3e-07 K, spending 34 node
    /// solves. Energy discrepancy **-2.8539e6 J** per second of stream transit,
    /// with **4** nodes' pre-repair cold state on the saturation line and
    /// therefore ambiguous (uncertainty 2.7330e6 J on the same inventory).
    ///
    /// Repaired profile \[K\]:
    ///
    /// | node | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
    /// |---|---|---|---|---|---|---|---|---|
    /// | hot | 924.2 | 836.1 | 766.4 | 712.2 | 670.2 | 633.1 | 593.2 | 547.5 |
    /// | metal | 631.6 | 601.6 | 584.2 | 570.7 | 560.2 | 523.5 | 466.4 | 401.0 |
    /// | cold | 534.1 | 523.5 | 523.5 | 523.5 | 523.5 | 484.3 | 424.1 | 352.2 |
    ///
    /// Interpretation: the remedy does what it claims -- a second-law-violating
    /// profile is replaced by an admissible one with a wide margin, and the
    /// metal lands strictly between the two streams at every node. The negative
    /// discrepancy says the repair *removed* about 2.9 MJ per second of transit
    /// from the inventory, which is the injected 40 K cold overshoot being
    /// undone along with the fixture's whole profile being replaced by a steady
    /// one; a running total of that figure is what makes the fidelity trade
    /// auditable. Note the uncertainty is comparable to the discrepancy itself
    /// -- with four nodes in the dome, this figure is an order-of-magnitude
    /// audit, not a precise energy budget.
    #[test]
    fn a_crossed_htr10_profile_comes_back_cross_free() {
        let inputs = crossed_htr10();
        let supplied_cross = inputs.worst_cross_kelvin();
        assert!(
            supplied_cross > 0.0,
            "fixture must actually cross; got {supplied_cross} K"
        );

        let (outcome, diagnostics) =
            repair_with_diagnostics(&inputs).expect("the crossed fixture must be repairable");
        let repaired_cross = worst_cross_margin_of(&outcome);

        println!("supplied worst cross = {supplied_cross:.4} K");
        println!("repaired worst cross = {repaired_cross:.4} K (negative is a pinch)");
        println!(
            "repaired worst cross, floored as the contract measures it = {:.4} K",
            floored_worst_cross_of(&outcome)
        );
        println!(
            "energy discrepancy   = {:.4e} J per second of stream transit",
            outcome.energy_discrepancy.get::<joule>()
        );
        println!("diagnostics: {}", diagnostics.report());
        for i in 0..8 {
            println!(
                "  node {i}: hot {:8.3} K  metal {:8.3} K  cold {:8.3} K",
                outcome.hot_temperatures[i].get::<kelvin>(),
                outcome.metal_temperatures[i].get::<kelvin>(),
                outcome.cold_temperatures[i].get::<kelvin>(),
            );
        }

        assert!(
            repaired_cross < 0.0,
            "the repaired profile still crosses by {repaired_cross} K"
        );
        assert_eq!(
            floored_worst_cross_of(&outcome),
            0.0,
            "the contract's own measure must report no cross at all"
        );
        // The metal must lie between the two streams at every node, or it has
        // its own cross.
        for i in 0..8 {
            let hot = outcome.hot_temperatures[i].get::<kelvin>();
            let metal = outcome.metal_temperatures[i].get::<kelvin>();
            let cold = outcome.cold_temperatures[i].get::<kelvin>();
            assert!(
                cold < metal && metal < hot,
                "node {i}: metal {metal} K is not between cold {cold} K and hot {hot} K"
            );
        }
    }

    /// **The reason this method is preferred over LMTD: it needs no zoning.**
    ///
    /// **Methodology.** Repair the crossed HTR-10 profile and classify every
    /// repaired cold node against `Tsat(4 MPa)` = 523.504 K. The march never
    /// locates a zone boundary, never chooses a per-zone heat transfer
    /// coefficient, and never branches on phase: it marches enthalpy and asks
    /// IF97 for the temperature. Pass criterion: the repaired cold profile
    /// contains at least one subcooled node, at least one saturated node and at
    /// least one superheated node -- i.e. the boiling transition really is
    /// inside the marched domain and really was crossed without zoning.
    ///
    /// # Results (2026-08-13)
    ///
    /// Tsat = 523.508 K. The repaired cold profile is
    /// 534.10 / 523.51 / 523.51 / 523.51 / 523.51 / 484.30 / 424.07 / 352.19 K
    /// -- **3 subcooled nodes, 4 saturated nodes, 1 superheated node**, so all
    /// three regimes are present. Interpretation: the enthalpy formulation
    /// passes through the phase change natively. This is the property
    /// [`super::super::TemperatureCrossRemedy::Lmtd`] cannot have without being
    /// zoned first, and it is the concrete reason the design note names LMTD's
    /// zoning as the biggest technical risk of that remedy and not of this one.
    #[test]
    fn the_march_crosses_the_boiling_transition_without_zoning() {
        let (outcome, _) = repair_with_diagnostics(&crossed_htr10()).expect("repairable");
        let t_sat = sat_temp_4(cold_pressure()).get::<kelvin>();

        let mut subcooled = 0;
        let mut saturated = 0;
        let mut superheated = 0;
        for t in &outcome.cold_temperatures {
            let value = t.get::<kelvin>();
            if (value - t_sat).abs() <= SATURATION_TOLERANCE_KELVIN {
                saturated += 1;
            } else if value < t_sat {
                subcooled += 1;
            } else {
                superheated += 1;
            }
        }
        println!("Tsat = {t_sat:.3} K");
        println!(
            "cold profile = {:?}",
            outcome
                .cold_temperatures
                .iter()
                .map(|t| (t.get::<kelvin>() * 100.0).round() / 100.0)
                .collect::<Vec<f64>>()
        );
        println!("subcooled {subcooled}, saturated {saturated}, superheated {superheated}");

        assert!(subcooled >= 1, "no subcooled node in the repaired profile");
        assert!(saturated >= 1, "no saturated node in the repaired profile");
        assert!(
            superheated >= 1,
            "no superheated node in the repaired profile"
        );
    }

    /// **The repaired profile is a fixed point of the repair.**
    ///
    /// **Methodology.** Repair the crossed profile, feed the repaired profile
    /// back in as a fresh [`CrossRepairInputs`], and repair again. A steady
    /// solution must be its own repair: the second pass' inputs already satisfy
    /// the same energy balance, so anything that moves is an inconsistency in
    /// the method rather than physics. This is the sharpest available check on
    /// the **hot-side capacity-rate inference**, which is the one quantity the
    /// march cannot look up. Pass criterion: relative drift in the inferred
    /// capacity rate below 1e-7, and no node moving by more than 1e-5 K.
    ///
    /// **This test found a real defect.** The inference originally divided the
    /// hot duty by the drop to the last node's **cell centre**, half a node
    /// short of the outlet face where the duty is actually rejected. That
    /// biased it high, and this test measured the bias at **+5.97%** with node
    /// movements of 14.3 K (hot) and 38.7 K (cold) -- a remedy that walked the
    /// state further every time it engaged. The fix is the closed-form
    /// half-node correction `C = (Q - 0.5*q[n-1])/dT` now in
    /// [`MarchContext::from_inputs`]; it is exact rather than approximate,
    /// because `q[n-1]` is read from the supplied state like the rest of `Q`.
    ///
    /// # Results (2026-08-13, after the fix)
    ///
    /// Inferred hot capacity rate **2.1224515317e4 W/K** on the first pass and
    /// **2.1224515333e4 W/K** on the second -- a relative drift of
    /// **7.77e-10**. Largest node movement **1.80e-7 K** (hot) and
    /// **3.95e-7 K** (cold). Interpretation: the repair is a fixed point to
    /// within the shooting tolerance itself
    /// ([`SHOOTING_TOLERANCE_KELVIN`] = 1e-6 K), which is what remains once the
    /// bias is gone -- there is no systematic drift left to find. Repeated
    /// engagement of the remedy therefore does not walk the state anywhere.
    #[test]
    fn the_repair_is_very_nearly_its_own_fixed_point() {
        let inputs = crossed_htr10();
        let (first, first_diagnostics) = repair_with_diagnostics(&inputs).expect("repairable");

        let mut second_inputs = inputs.clone();
        second_inputs.hot_temperatures = first.hot_temperatures.clone();
        second_inputs.metal_temperatures = first.metal_temperatures.clone();
        second_inputs.cold_temperatures = first.cold_temperatures.clone();
        let (second, second_diagnostics) =
            repair_with_diagnostics(&second_inputs).expect("repairable a second time");

        let capacity_first = first_diagnostics
            .inferred_hot_capacity_rate
            .get::<watt_per_kelvin>();
        let capacity_second = second_diagnostics
            .inferred_hot_capacity_rate
            .get::<watt_per_kelvin>();
        let drift = (capacity_second - capacity_first) / capacity_first;

        let worst_hot = (0..8)
            .map(|i| {
                (second.hot_temperatures[i].get::<kelvin>()
                    - first.hot_temperatures[i].get::<kelvin>())
                .abs()
            })
            .fold(0.0_f64, f64::max);
        let worst_cold = (0..8)
            .map(|i| {
                (second.cold_temperatures[i].get::<kelvin>()
                    - first.cold_temperatures[i].get::<kelvin>())
                .abs()
            })
            .fold(0.0_f64, f64::max);

        println!(
            "C_hot pass 1 = {capacity_first:.10e} W/K, pass 2 = {capacity_second:.10e} W/K, \
             drift = {:.4e}",
            drift
        );
        println!("worst node movement: hot {worst_hot:.6e} K, cold {worst_cold:.6e} K");

        assert!(
            drift.abs() < 1.0e-7,
            "capacity-rate inference drifted by a relative {drift} between passes"
        );
        assert!(worst_hot < 1.0e-5, "hot profile moved {worst_hot} K");
        assert!(worst_cold < 1.0e-5, "cold profile moved {worst_cold} K");
    }

    /// **The first-law bookkeeping is computed, not stubbed.**
    ///
    /// **Methodology.** The `energy_discrepancy` field must record the change
    /// in the streams' stored energy that overwriting the profiles causes. Two
    /// checks: (1) on the crossed fixture the figure is non-zero and its sign
    /// says the repair removed energy, since the injected cross was a cold
    /// overshoot; (2) it is reproduced independently, from the outcome's own
    /// temperatures, by an inventory sum written out longhand in this test --
    /// so a future change that quietly returns zero, or drops a stream, fails
    /// here. Pass criterion: the independent sum agrees to 1e-6 relative.
    ///
    /// # Results (2026-08-13)
    ///
    /// **Single-phase case.** Reported **-1.064711e6 J** per second of stream
    /// transit; independently recomputed **-1.064268e6 J**; gap **442.14 J**,
    /// relative **4.15e-4**. That gap is not an error in either sum: the
    /// implementation differences the enthalpy it *marched*, while the longhand
    /// can only recover one from the node's temperature through the forward
    /// `(T,p)` equation, and IAPWS-IF97's backward `(p,h)` equations are
    /// correlations rather than exact inverses. Measured in the same test, the
    /// `(p,h) -> T -> (T,p)` round trip is worth up to **40.36 J/kg** per node
    /// and **596.49 J** over the whole inventory -- which brackets the observed
    /// 442 J. The tolerance is set at 1e-3 relative for that reason and no
    /// other.
    ///
    /// **Two-phase case.** -2.853886e6 J with **4** ambiguous nodes and an
    /// uncertainty of **2.7330e6 J**.
    ///
    /// Interpretation: the field carries a real number derived from both
    /// streams' node inventories. It is referred to a **stated**
    /// one-second-of-flow inventory because the contract carries no node masses
    /// -- see [`energy_discrepancy`] -- and it excludes the tube metal for the
    /// same reason, so it is a lower bound on the true first-law violation, not
    /// the whole of it. Where the pre-repair state is two-phase the uncertainty
    /// is of the same order as the figure itself, and the diagnostics say so
    /// rather than letting the number look more precise than it is.
    #[test]
    fn the_energy_discrepancy_is_computed_from_both_streams() {
        // Part 1: the single-phase case, where every enthalpy is determined by
        // its temperature and the longhand can be exact.
        let inputs = single_phase_case();
        let (outcome, diagnostics) = repair_with_diagnostics(&inputs).expect("repairable");
        let reported = outcome.energy_discrepancy.get::<joule>();

        let n = 8.0;
        let hot_node_mass =
            inputs.hot_mass_flow.get::<kilogram_per_second>() * REFERENCE_INVENTORY_TIME_S / n;
        let cold_node_mass =
            inputs.cold_mass_flow.get::<kilogram_per_second>() * REFERENCE_INVENTORY_TIME_S / n;
        let cp_hot = diagnostics
            .inferred_hot_specific_heat
            .get::<joule_per_kilogram_kelvin>();

        let mut expected = 0.0;
        for i in 0..8 {
            expected += hot_node_mass
                * cp_hot
                * (outcome.hot_temperatures[i].get::<kelvin>()
                    - inputs.hot_temperatures[i].get::<kelvin>());
            let h_after = h_tp_eqm_single_phase(outcome.cold_temperatures[i], cold_pressure())
                .get::<joule_per_kilogram>();
            let h_before = h_tp_eqm_single_phase(inputs.cold_temperatures[i], cold_pressure())
                .get::<joule_per_kilogram>();
            expected += cold_node_mass * (h_after - h_before);
        }

        println!("single-phase case:");
        println!("  reported   dE = {reported:.6e} J per second of stream transit");
        println!("  recomputed dE = {expected:.6e} J");
        println!(
            "  ambiguous nodes = {}, uncertainty = {:.4e} J",
            diagnostics.saturation_ambiguous_nodes,
            diagnostics.energy_discrepancy_uncertainty.get::<joule>()
        );

        // The longhand cannot be exact, and the size of the gap is worth
        // pinning rather than absorbing into a loose tolerance. The
        // implementation differences the enthalpy it *marched*; the longhand
        // can only recover an enthalpy from the node's temperature, through the
        // forward `(T,p)` equation. IAPWS-IF97's backward `(p,h)` equations are
        // correlations, not exact inverses of the forward ones, so that round
        // trip does not close. Measure it directly.
        let mut worst_round_trip = 0.0_f64;
        let mut round_trip_joules = 0.0_f64;
        for t in &outcome.cold_temperatures {
            let h_a = h_tp_eqm_single_phase(*t, cold_pressure());
            let t_b = t_ph_eqm(cold_pressure(), h_a);
            let h_b = h_tp_eqm_single_phase(t_b, cold_pressure()).get::<joule_per_kilogram>();
            let h_a = h_a.get::<joule_per_kilogram>();
            worst_round_trip = worst_round_trip.max((h_b - h_a).abs());
            round_trip_joules += cold_node_mass * (h_b - h_a).abs();
        }
        println!(
            "  IF97 (p,h)->T->(T,p) round trip: worst {worst_round_trip:.4} J/kg, \
             {round_trip_joules:.2} J over the inventory"
        );

        assert!(reported != 0.0, "the discrepancy was stubbed to zero");
        assert_eq!(
            diagnostics.saturation_ambiguous_nodes, 0,
            "a wholly single-phase case cannot have an ambiguous node"
        );
        let gap = (reported - expected).abs();
        let relative = gap / expected.abs().max(1.0);
        println!("  gap = {gap:.2} J, relative = {relative:.4e}");
        assert!(
            relative < 1.0e-3,
            "reported {reported} against longhand {expected}, relative {relative}"
        );

        // Part 2: the two-phase HTR-10 case, where four supplied cold nodes sit
        // exactly on the saturation line and their pre-repair enthalpy is
        // therefore not determined. The figure must still be produced, and the
        // ambiguity must be counted and bounded rather than hidden.
        let (boiling, boiling_diagnostics) =
            repair_with_diagnostics(&crossed_htr10()).expect("repairable");
        println!("two-phase case:");
        println!(
            "  dE = {:.6e} J, ambiguous nodes = {}, uncertainty = {:.4e} J",
            boiling.energy_discrepancy.get::<joule>(),
            boiling_diagnostics.saturation_ambiguous_nodes,
            boiling_diagnostics
                .energy_discrepancy_uncertainty
                .get::<joule>()
        );
        assert!(boiling.energy_discrepancy.get::<joule>() != 0.0);
        assert!(
            boiling_diagnostics.saturation_ambiguous_nodes > 0,
            "the boiling fixture's saturated nodes must be reported as ambiguous"
        );
        assert!(
            boiling_diagnostics
                .energy_discrepancy_uncertainty
                .get::<joule>()
                > 0.0,
            "an ambiguous node must carry a non-zero uncertainty"
        );
    }

    /// **A profile that cannot be repaired says so.**
    ///
    /// **Methodology.** Starve the cold stream: keep the HTR-10 exchanger
    /// exactly as it is but drop the feedwater flow from 3.19 kg/s to
    /// 0.02 kg/s, so no admissible steady profile exists -- the duty that
    /// geometry can pass would have to superheat the water clean out of the
    /// IF97 envelope. The hot-side inference is untouched by this (it reads the
    /// hot and metal profiles, not the cold flow), so the failure is the
    /// march's, not the validation's. Pass criterion: a
    /// [`CrossRepairError::DidNotConverge`] -- **not** a repaired profile, and
    /// specifically not a silently crossed one.
    ///
    /// # Results (2026-08-13)
    ///
    /// Returned `DidNotConverge`: "no marchable upper end of the shooting
    /// bracket after 60 bisections on marchability, between a 313.1603 K hot
    /// outlet and the 973.1500 K hot inlet; the last failure was: the power
    /// balance at node 4 stalled with residual 1.641749e-5 W".
    ///
    /// Interpretation: no admissible upper end exists, and the search says so
    /// after exhausting the interval rather than handing back a profile. The
    /// *proximate* failure reported is a node stall rather than an envelope
    /// breach, and that is itself informative: at 0.02 kg/s a few watts move
    /// the water's enthalpy by more than the whole subcooled range, so the node
    /// residual is near-vertical in `q` and the secant cannot close it. Both
    /// failure modes are the same physical fact -- the cold stream cannot carry
    /// this duty -- and either way the repair is refused rather than papered
    /// over by clamping the enthalpy at the envelope, which is exactly what the
    /// BEDOK reference does and what this adaptation deliberately does not. The
    /// caller must escalate.
    #[test]
    fn an_unrepairable_exchanger_reports_rather_than_returning_a_crossed_profile() {
        let mut inputs = crossed_htr10();
        inputs.cold_mass_flow = MassRate::new::<kilogram_per_second>(0.02);

        match repair(&inputs) {
            Err(CrossRepairError::DidNotConverge(why)) => println!("reported: {why}"),
            other => panic!("expected DidNotConverge, got {other:?}"),
        }
    }

    /// **Input validation refuses states the march cannot honestly consume.**
    ///
    /// **Methodology.** Four defects, each of which would otherwise either
    /// divide by something degenerate or reach a **panic** inside
    /// `tampines-steam-tables`: mismatched profile lengths, a non-positive mass
    /// flow, a hot inlet below the cold inlet, and a cold inlet enthalpy
    /// outside the IF97 `(p,h)` envelope. Pass criterion: each returns
    /// [`CrossRepairError::BadInputs`], and none panics.
    ///
    /// # Results (2026-08-13)
    ///
    /// All four returned `BadInputs` naming the specific defect and quoting the
    /// offending number. Interpretation: the remedy's failure surface is the
    /// error type, not the process. This matters more here than in most code
    /// because the steam tables signal an out-of-range state by panicking, and
    /// a panic inside a repair would take the simulator down at exactly the
    /// moment the repair was meant to keep it alive.
    #[test]
    fn bad_inputs_are_rejected_with_a_reason() {
        let base = crossed_htr10();

        let mut short = base.clone();
        short.cold_temperatures.pop();
        assert!(matches!(
            repair(&short),
            Err(CrossRepairError::BadInputs(_))
        ));

        let mut no_flow = base.clone();
        no_flow.cold_mass_flow = MassRate::new::<kilogram_per_second>(0.0);
        assert!(matches!(
            repair(&no_flow),
            Err(CrossRepairError::BadInputs(_))
        ));

        let mut cold_hot_side = base.clone();
        cold_hot_side.hot_inlet_temperature = ThermodynamicTemperature::new::<kelvin>(300.0);
        assert!(matches!(
            repair(&cold_hot_side),
            Err(CrossRepairError::BadInputs(_))
        ));

        let mut absurd_feed = base.clone();
        absurd_feed.cold_inlet_enthalpy = AvailableEnergy::new::<joule_per_kilogram>(9.9e7);
        match repair(&absurd_feed) {
            Err(CrossRepairError::BadInputs(why)) => println!("out-of-envelope feed: {why}"),
            other => panic!("expected BadInputs, got {other:?}"),
        }
    }

    /// **What one repair costs, because the design note requires it.**
    ///
    /// **Methodology.** `docs/heat-exchanger-temperature-cross-fallback.md`
    /// concern 6: "the real-time budget is the reason this exists, so it must
    /// be measured... if the fallback costs more than the substeps it saves, it
    /// is a net loss". Time 200 repairs of the crossed 8-node HTR-10 fixture in
    /// release mode and report the mean. The reference point is the exchanger
    /// itself, measured 2026-08-13 at **1.0 s of compute per second of
    /// simulated time** at 2 outer correctors, i.e. about 12.5 ms per 0.0125 s
    /// substep. Pass criterion: a mean below 20 ms, which is deliberately loose
    /// -- this is a regression guard against something catastrophic, not a
    /// benchmark, and it must not fail on a loaded machine.
    ///
    /// # Results (2026-08-13)
    ///
    /// **Mean 425 us per repair** over 200 repairs, from four consecutive runs
    /// measuring 429.1, 415.3, 435.9 and 421.0 us (release, on a workstation
    /// with two sibling cargo builds contending for it -- so read this as an
    /// order of magnitude, not a benchmark).
    ///
    /// Interpretation: a repair costs about **3.4% of one steam-generator
    /// substep** (12.5 ms), so engaging the remedy is cheap next to the
    /// sub-stepping it exists to avoid, and the design note's real-time
    /// argument survives its own measurement. The caveat is that this is per
    /// *repair*, not per plant step: a remedy firing on every substep would
    /// cost 80 repairs per second of simulated time, or ~34 ms/s -- still only
    /// ~3% of the exchanger, but no longer negligible. What dominates the cost
    /// is the same thing that dominates the exchanger's: IF97 flashes, one per
    /// node solve iteration.
    #[test]
    fn one_repair_costs_a_small_fraction_of_a_steam_generator_substep() {
        use std::time::Instant;

        let inputs = crossed_htr10();
        // Warm the property caches and confirm it repairs at all.
        repair(&inputs).expect("repairable");

        let repeats = 200;
        let start = Instant::now();
        for _ in 0..repeats {
            let outcome = repair(&inputs).expect("repairable");
            // Consume the result so the optimiser cannot elide the work.
            assert!(outcome.hot_temperatures[0].get::<kelvin>() > 0.0);
        }
        let mean = start.elapsed().as_secs_f64() / repeats as f64;

        println!("mean repair cost = {:.1} us", mean * 1.0e6);
        assert!(
            mean < 20.0e-3,
            "one repair took {mean} s, which is the wrong order of magnitude"
        );
    }

    /// **An already-healthy profile is repaired to the same steady state.**
    ///
    /// **Methodology.** The remedy is only invoked on a cross, but it must not
    /// depend on there being one: the march solves a steady exchanger, and a
    /// cross in the supplied state enters only through the inferred hot
    /// capacity rate. Repair the *uncrossed* HTR-10 fixture and compare with the
    /// repair of the crossed one, which differs only in the cold profile at
    /// node 6. Pass criterion: both are cross-free, and their duties differ by
    /// less than 5% -- the difference being the inference's exposure to the
    /// supplied cold profile, which is second-order because the inference reads
    /// the hot and metal profiles.
    ///
    /// # Results (2026-08-13)
    ///
    /// Uncrossed duty **9.972977e6 W**, crossed duty **9.554247e6 W**, a
    /// **4.20%** difference; worst cross -156.90 K and -146.66 K respectively,
    /// both comfortably cross-free.
    ///
    /// Interpretation: the presence of a cross moves the repaired steady state
    /// by about 4% in duty. That is the inference's exposure to the supplied
    /// profile made visible, and it is the honest size of the method's
    /// state-dependence -- the fixture's metal profile is built from the
    /// conductance-weighted mean, so injecting a cold overshoot moves the metal
    /// too, exactly as it would in the array this stands in for. **A repair is
    /// therefore not independent of how bad the cross was**, which is one more
    /// reason a remedy-engaged run is not a resolved transient.
    #[test]
    fn a_healthy_profile_marches_to_the_same_steady_state() {
        let (healthy, healthy_diagnostics) =
            repair_with_diagnostics(&uncrossed_htr10()).expect("repairable");
        let (crossed, crossed_diagnostics) =
            repair_with_diagnostics(&crossed_htr10()).expect("repairable");

        let healthy_duty = healthy_diagnostics.duty.get::<watt>();
        let crossed_duty = crossed_diagnostics.duty.get::<watt>();
        let difference = (crossed_duty - healthy_duty).abs() / healthy_duty;

        println!("uncrossed duty = {healthy_duty:.6e} W");
        println!(
            "crossed   duty = {crossed_duty:.6e} W  ({:.4}%)",
            100.0 * difference
        );
        println!(
            "uncrossed worst cross = {:.4} K",
            worst_cross_margin_of(&healthy)
        );
        println!(
            "crossed   worst cross = {:.4} K",
            worst_cross_margin_of(&crossed)
        );

        assert!(worst_cross_margin_of(&healthy) < 0.0);
        assert!(worst_cross_margin_of(&crossed) < 0.0);
        assert!(
            difference < 0.05,
            "duties differ by {}%",
            100.0 * difference
        );
    }
}
