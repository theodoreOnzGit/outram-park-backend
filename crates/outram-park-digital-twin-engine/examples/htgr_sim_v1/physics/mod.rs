//! HTGR plant physics backend -- **pebble-bed** core, HTR-10 scale.
//!
//! Orchestrates the four subsystems of a helium-cooled, graphite-moderated
//! **pebble-bed** HTGR into a single [`HtgrPlant`] that steps them in the
//! physical order they are coupled:
//!
//! 1. [`kinetics`] -- reactor power from the prompt excursion layer + a
//!    delayed-neutron precursor bank, wired to the real
//!    `teh_o_prke::DelayedNeutronLayer`.
//! 2. [`pebble_bed`] -- the fission power heats 5.3 t of graphite pebbles,
//!    which hand the helium whatever crosses the pebble surface. This is the
//!    core's thermal inertia and it is the slowest thing in the plant.
//! 3. [`primary_loop`] -- the helium carries that heat from the core outlet to
//!    the steam generator and returns cooled, closing the circuit.
//! 4. [`steam_generator`] -- a **resolved 8-node counter-flow exchanger**
//!    (helium <-> steel tube metal <-> water/steam) owned by the primary loop.
//!    This is the only spatially discretised part of the plant.
//! 5. [`secondary_loop`] -- the steam-generator duty drives a real IAPWS-IF97
//!    steam cycle (feed pump -> steam generator -> turbine -> condenser ->
//!    hotwell).
//!
//! ## This core used to be prismatic
//!
//! Until 2026-08-12 this simulator modelled a **prismatic-block** HTGR at
//! roughly 200 MWth with machined coolant channels. It now models a pebble bed
//! at the published HTR-10 operating point -- 10 MWth, helium at 3.0 MPa,
//! 250 degC in and 700 degC out at 4.3 kg/s, 27,000 spherical fuel elements in
//! a 1.8 m by 1.97 m bed, with **downward** flow through the bed and a
//! separate-vessel once-through helical steam generator. The whole plant was
//! rescaled with it, so every displayed magnitude is roughly twenty times
//! smaller than it was.
//!
//! ## Nodalisation of the whole plant
//!
//! Every subsystem here is **one control volume**. Nothing in this plant model
//! is spatially discretised:
//!
//! | Subsystem | Nodes | Consequence |
//! |---|---|---|
//! | Pebble bed | **1** | no axial or radial temperature profile, no peak fuel temperature |
//! | Helium circuit | **1** (two boundary temperatures) | no gradient through the bed, no natural circulation |
//! | Steam generator | **8 x 3** (helium / tube metal / water, counter-flow) | resolved zones and a real metal lag; 8 nodes is coarse |
//! | Secondary water/steam, outside the SG | **1** | fixed steam pressure, no drum or inventory dynamics |
//! | Neutronics | **1** (point kinetics) | no spatial flux shape, no rod-position-dependent worth |
//! | Reflector, barrel, cavity | **0** | the HTR-10 passive decay-heat path is absent entirely |
//!
//! **The steam generator is the exception, as of 2026-08-12**, and it is the
//! only part of this plant that is not one control volume. See
//! [`steam_generator`].
//!
//! Each module's own doc comment states what its single node lumps and what
//! refinement to reach for first. Read [`pebble_bed`] before quoting any core
//! temperature from this model.
//!
//! ## The loops are coupled both ways
//!
//! The loops are not run open-ended in sequence. Each step reads the
//! secondary's **feedwater state** (enthalpy and flow) *first* and hands it to
//! the primary as the steam generator's tube-side inlet, so:
//!
//! - the water entering the tubes limits how much heat the helium can shed, and
//! - the resulting helium-side outlet becomes the next core inlet.
//!
//! That closes the primary loop (the core inlet is a computed variable, not a
//! constant) and makes the steam generator's duty a *resolved* result rather
//! than a formula.
//!
//! Until 2026-08-12 what crossed here was the secondary's **saturation
//! temperature**, and the exchanger was an effectiveness-NTU lump pinching
//! against it as an isothermal sink. That is correct for an evaporator and wrong
//! for a once-through unit: as the steam superheats the real driving difference
//! collapses, and against a fixed sink it never did. Measured 2026-08-12, the
//! old model over-predicted the hot-end driving difference by **78%** (470.4 K
//! against the resolved 263.8 K), and the steam it produced was clamped at the
//! helium inlet temperature by a downstream second-law cap. See
//! [`steam_generator`].
//!
//! The pebble bed sits inside that loop: it reads the helium bulk mean
//! temperature and returns a heat rate, so a loss of heat removal backs up into
//! the graphite temperature.
//!
//! ## One timestep, and outer correctors around the couplings
//!
//! The whole simulator advances at a single constant, [`PLANT_TIMESTEP_S`] =
//! 0.1 s -- the application, the steam generator's derived sub-clock and every
//! whole-plant test all read it, so no two of them can drift apart. The plant
//! stepped at **1 ms** until 2026-08-13, and moving to 0.1 s is the change that
//! made the sequential (Lie-split) couplings above worth correcting: quantities
//! that were one millisecond stale are now one tenth of a second stale.
//!
//! [`HtgrPlant::step`] therefore runs [`PLANT_OUTER_CORRECTORS`] outer
//! correctors over the **cheap** subsystems -- rewinding them to the start of
//! the timestep and re-advancing them against improving estimates of the
//! coupled quantities -- while advancing the steam generator exactly once, on
//! the final corrector, with the converged boundary conditions. The exchanger
//! is outside the loop because it is **~96% of this plant's compute** (measured
//! 2026-08-13) and its three arrays carry spatial history that cannot be
//! rewound cheaply.
//!
//! **Note what raising the timestep did and did not buy.** It did not make the
//! simulator meaningfully faster: the exchanger runs on its own clock, so the
//! real-time ratio moved only from 0.492 to 0.514. What actually bought speed
//! was halving the exchanger's PIMPLE outer-corrector count, which the
//! measurements showed changes the settled duty by nothing. See
//! [`PLANT_TIMESTEP_S`] and
//! [`steam_generator::PimpleCorrectors`].
//!
//! ## Status
//!
//! The structure, the cross-crate wiring, the published HTR-10 operating point
//! and core geometry, and the thermophysical properties (helium via the
//! CoolProp-derived Helmholtz EOS, water/steam via IAPWS-IF97) are **real**.
//!
//! **Every published constant now comes from the library**, not from a copy
//! kept here: [`outram_park_digital_twin_engine::htr10::design::Htr10DesignPoint`]
//! is the single transcription of IAEA-TECDOC-1382 that the core geometry, the
//! helium operating point, the steam conditions and the nominal power are all
//! read from. Two copies of an operating point drift silently; there is now
//! one.
//!
//! **The pebble-bed friction is real** as of 2026-08-12: the KTA packed-bed
//! correlation ([`outram_park_digital_twin_engine::htr10::kta`]) is evaluated
//! by [`primary_loop::bed_pressure_drop`] and gated against the Virtual Test
//! Bed's published worked example (3493.17 Pa/m against the gold 3493 Pa/m).
//! **That makes the friction real; it does not make the nodalisation real.**
//! Every subsystem is still one control volume, the correlation is applied once
//! at the bulk mean rather than integrated down the bed, and the pressure drop
//! still cannot feed back on the flow.
//!
//! **The steam generator is spatially resolved** as of 2026-08-12: an 8-node
//! counter-flow exchanger coupling a helium array, a steel tube-metal column and
//! a water/steam array through real conductances ([`steam_generator`]). It
//! resolves the economiser / evaporator / superheater zones, carries a derived
//! **3184 kg** of tube metal with a **38 s** thermal time constant, and cannot
//! represent a temperature cross at any node. **That makes the arrangement real;
//! it does not make the sizing real** -- the `UA` is still an explicit
//! calibration and the tube diameters are still invented.
//!
//! What remains illustrative is every *closure and every dimension the
//! published sources do not carry* -- the pebble-to-helium heat-transfer
//! coefficient (measurably too low, see [`pebble_bed`]), graphite `c_p`, the
//! loop gas volume, the steam-generator `UA` and tube geometry, efficiencies,
//! inventories and controller constants. An effective bed conductivity now **exists** in the
//! workspace ([`outram_park_digital_twin_engine::htr10::zbs`]) but is
//! deliberately not in the heat path, because one control volume has no
//! internal gradient for it to act on. The live steam pressure is still held
//! fixed (see [`secondary_loop`]). Replacing the invented dimensions with
//! sourced ones is tracked as bead `op-szmi.6`.
//!
//! This is a demonstration model for the digital-twin engine, **not a
//! validated HTR-10 model**, and must not be used for any of the purposes
//! `RESPONSIBLE_USE.md` excludes.
//!
//! What belongs here: the plant orchestration and the physics->snapshot
//! projection. What does not: GUI/rendering (that is [`crate::app`]) and the
//! underlying physics kernels (those live in the workspace libraries this
//! composes).

pub mod control_rods;
pub mod kinetics;
pub mod pebble_bed;
pub mod primary_loop;
pub mod protection;
pub mod secondary_loop;
pub mod steam_generator;
/// Remedies for a steam-generator temperature cross -- see the module docs for
/// why they exist and why the default does nothing.
pub mod temperature_cross;
pub mod turbine_generator;

use outram_park_digital_twin_engine::animation::residence_time_from_flow;
use outram_park_digital_twin_engine::app_scaffold::mark_component;
use uom::si::f64::{MassRate, Power, ThermodynamicTemperature, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::{megawatt, watt};
use uom::si::pressure::kilopascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use crate::app::state::HtgrSnapshot;
use kinetics::{power_in_megawatts, HtgrKinetics};
use pebble_bed::PebbleBedCore;
use primary_loop::HeliumPrimaryLoop;
use protection::ReactorProtectionSystem;
use secondary_loop::SteamSecondaryLoop;
use turbine_generator::TurbineGeneratorShaft;

/// **The plant timestep \[s\] -- the one step size this simulator runs at.**
///
/// Every consumer reads this constant. There is no second literal anywhere in
/// `htgr_sim_v1` that has to be kept in step with it by hand:
///
/// | Consumer | Reads it as |
/// |---|---|
/// | [`crate::app`]'s physics thread | `PHYSICS_DT_S`, and the wall-clock tick budget derived from it |
/// | [`primary_loop`]'s steam generator | [`steam_generator_substep_seconds`] = `PLANT_TIMESTEP_S` / [`STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP`] |
/// | [`kinetics`] | [`KINETICS_SUBSTEP_S`] = `PLANT_TIMESTEP_S` / 100 |
/// | every whole-plant test, and every loop test | [`plant_timestep`] |
///
/// # Why 0.1 s and not the 1 ms this used to be
///
/// The application drove the plant at **1 ms** until 2026-08-13, and could not
/// keep up with wall clock: `HtgrPlant::step` cost 2.09-2.45 ms of compute per
/// 1 ms of plant time, so the best achievable real-time ratio was **0.41-0.48**
/// (`crate::app`'s module doc records the measurement). Reducing the substeps
/// per GUI tick could not help -- it cuts plant time and compute in the same
/// proportion -- so the timestep itself had to grow.
///
/// The cost is dominated by [`steam_generator`], which runs on **its own fixed
/// clock** and therefore costs the same per second of plant time whatever the
/// plant timestep is. Everything else in the plant costs `1/dt` per second of
/// plant time. Raising the plant step from 1 ms to 0.1 s removes the 100x
/// oversampling of that "everything else" and leaves the exchanger's cost
/// untouched. See [`tests::the_whole_plant_steps_at_the_gui_timestep`] for the
/// measured result.
///
/// # What raising it actually bought -- much less than expected
///
/// **4%.** The real-time ratio went from 0.492 at 1 ms to 0.514 at 0.1 s. That
/// is the honest headline, and it contradicts the reasoning that motivated the
/// change: the plant timestep was never where the money was.
///
/// The 2x that *did* reach real time came from halving the exchanger's own
/// PIMPLE outer-corrector count, 4 to 2, which the measurements showed changes
/// the settled duty by nothing (see
/// [`steam_generator::PimpleCorrectors`]). The global timestep is still worth
/// having -- it is what makes [`PLANT_OUTER_CORRECTORS`] affordable, and it
/// removes a class of bug where the application ran at a rate no test covered
/// -- but it is not the speed-up.
///
/// # Why not larger still
///
/// 0.1 s is 1:1 with a 10 Hz GUI tick, which is a comfortable update rate for a
/// schematic, and it is 8 whole [`steam_generator`] substeps -- see
/// [`STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP`].
///
/// Beyond that, **there is nothing left to win**: at 0.1 s the exchanger is
/// already 96% of the cost and pays nothing for the plant step, so a larger
/// step would buy under 4% more while making every coupling worse. It would
/// also strand two genuine physical timescales that 0.1 s currently brackets --
/// the prompt-kinetics relaxation `Lambda/beta = 1e-3/0.0065 = 0.154 s`
/// ([`kinetics`], already sub-stepped for exactly this reason) and the 5 s core
/// gas inertia.
pub const PLANT_TIMESTEP_S: f64 = 0.1;

/// Steam-generator array sub-steps per plant timestep, **2**.
///
/// Reduced from 8 on 2026-08-13, once the helium side moved to implicit
/// energy convection ([`EnergyBalanceMode::Implicit`], set in
/// [`steam_generator`]). This is the single largest real-time lever in the
/// simulator, because the exchanger is ~96% of the plant's compute and its
/// cost is exactly linear in this number.
///
/// # Read this before treating 8 as meaningful
///
/// 8 is set by **one thing only**: the advective Courant limit of the
/// *explicit* enthalpy convection in the two fluid arrays. It is **not** an
/// accuracy requirement, and nothing about the plant, the exchanger geometry or
/// the physics asks for it -- at 0.0125 s the settled duty is already within
/// +0.003% of a 0.00625 s reference, so accuracy was satisfied long before the
/// stability limit was.
///
/// # Why 2, and what stops it being 1
///
/// Measured 2026-08-13 with the helium side implicit, 20 s of plant time,
/// isolated release runs of [`tests::the_whole_plant_steps_at_the_gui_timestep`]:
///
/// | sub-steps | `Co_hot` | real-time ratio | steam outlet |
/// |---|---|---|---|
/// | 8 | 0.222 | 1.027 | 692.99 K |
/// | 4 | 0.444 | 2.051 | 695.13 K |
/// | **2** | **0.888** | **4.101** | **698.23 K** |
/// | 1 | 1.776 | -- | **temperature cross** |
///
/// **1 fails on thermodynamics, not on the Courant number.** The helium array
/// is implicit and handles `Co = 1.776` without trouble; what breaks is the
/// second-law assertion in
/// [`tests::the_whole_plant_steps_at_the_gui_timestep`]
/// (`worst_node_cross_kelvin() <= 1e-6`). The exchanger is three separate
/// matrix systems -- hot array, tube metal, cold array -- coupled through
/// conductances evaluated at the previous iterate, so the *inter-array*
/// coupling stays explicit however implicit each array's own convection is. At
/// one exchange per 0.1 s the counter-flow arrangement cannot resolve and the
/// cold stream overtakes the hot.
///
/// Remedies for that are designed but not adopted by default -- see
/// `docs/heat-exchanger-temperature-cross-fallback.md` and the
/// `TemperatureCrossRemedy` selector.
///
/// # The accuracy this costs
///
/// Going 8 -> 2 moves the settled steam outlet **+5.24 K** (0.76% of a ~693 K
/// outlet). Accepted deliberately: this exchanger's tube geometry is invented
/// and its `UA` illustrative, so 0.76% on the steam terminal is well inside
/// the model's own uncertainty, and 4.1x real time is worth more to a
/// demonstration simulator than 5 K on a terminal temperature. If a study
/// needs the fidelity back, raise this constant -- the cost is linear and the
/// numbers above tell you exactly what you buy.
///
/// The exchanger is a **multi-rate sub-model**: it accumulates the plant
/// timestep and advances its three coupled arrays in whole steps of
/// `PLANT_TIMESTEP_S / STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP`. With
/// [`PLANT_TIMESTEP_S`] at 0.1 s that is exactly **0.0125 s**, the substep the
/// exchanger was converged and stability-tested at on 2026-08-12 -- so the
/// global-timestep change did not move it.
///
/// # This division is a Courant limit, and outer correctors cannot remove it
///
/// The hot (helium) array's cells are `5.0 m / 8 = 0.625 m` long and the gas
/// moves at about 11 m/s, so the advective Courant number is `~0.23` at
/// 0.0125 s and would be `~1.8` at the full 0.1 s plant step. Both fluid arrays
/// carry their enthalpy convection **explicitly** -- `fvc::div_limited` as a
/// source term, not an `fvm::div` matrix contribution -- inside their PIMPLE
/// outer-corrector loop, which makes that loop a Picard iteration whose
/// contraction factor is the cell Courant number. More outer correctors
/// therefore do **not** buy a larger array timestep: above `Co = 1` the
/// iteration diverges however many are used.
///
/// Measured values and the corrector sweep that establishes this are in
/// [`steam_generator::tests::the_courant_number_bounds_the_array_substep`].
/// The outer correctors this plant *does* use are at the coupling level
/// instead -- see [`PLANT_OUTER_CORRECTORS`].
pub const STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP: usize = 2;

/// **Plant-level outer correctors per plant timestep, 2.**
///
/// These are *not* the steam-generator arrays' own PIMPLE correctors (those are
/// [`steam_generator::PimpleCorrectors`], and they do not lift the Courant
/// limit -- see [`STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP`]). These are outer
/// correctors on the **couplings between subsystems**, and they exist because
/// the plant timestep grew a hundredfold.
///
/// # The couplings they correct
///
/// [`HtgrPlant::step`] runs its five subsystems in sequence, so two of the
/// quantities that cross between them are read one step **late** -- a Lie
/// (operator) split:
///
/// | Lagged quantity | Read by | Own timescale |
/// |---|---|---|
/// | helium bulk mean temperature | [`pebble_bed`] | 5 s gas inertia |
/// | feedwater enthalpy and flow | [`primary_loop`]'s exchanger | 10 s feed controller |
///
/// At the old 1 ms timestep the resulting `O(dt)` error was 1e-4 of the
/// relevant time constant and could be ignored. At 0.1 s it is 1e-2, and --
/// more importantly -- the steam generator's boundary conditions are
/// **zero-order held** across all 8 of its array substeps, so freezing the
/// hot-inlet temperature at its start-of-step value now freezes it for a tenth
/// of a second.
///
/// # How the loop works, and why the exchanger is outside it
///
/// Each corrector rewinds the cheap lumped states -- [`PebbleBedCore`],
/// [`primary_loop::PrimaryLumpedState`] and the secondary's feed flow -- to the
/// start of the timestep and re-advances them against the latest iterate of the
/// coupled quantities. The **steam generator is advanced exactly once**, on the
/// final corrector: its three arrays carry spatial history that cannot be
/// rewound cheaply, and at ~96% of the plant's compute re-running it would undo
/// the entire point. What the loop therefore buys is that the exchanger is
/// handed *converged, end-of-step* boundary conditions instead of
/// start-of-step ones -- the difference between an explicit and a
/// nearly-implicit coupling -- for the cost of re-running the other 4%.
///
/// Three subsystems sit **outside** the loop, each for its own reason:
///
/// - **The protection system**, because it is specified to evaluate the
///   *previous* step's measured signals so that a trip cannot be outrun within
///   a timestep. Correcting it against this step's own power is exactly the
///   behaviour it exists to prevent.
/// - **The kinetics** ([`HtgrKinetics`]), because it is not coupled to anything
///   the loop iterates -- it reads only the commanded reactivity and its own
///   adiabatic fuel temperature, so a second pass returns a bit-identical
///   answer. Its accuracy problem at 0.1 s is real but separate, and is solved
///   by sub-stepping instead: see [`KINETICS_SUBSTEP_S`]. `HtgrKinetics` is
///   `Clone` so it can be moved inside the loop if the documented refinement
///   (reactivity feedback reading the pebble-bed temperature) is ever made.
/// - **The turbine shaft**, because it is a pure consumer -- it reads the
///   converged turbine power and feeds nothing back.
///
/// # Why 2 -- measured, not chosen
///
/// [`HtgrPlant::step_with_correctors`] exists so this can be swept; the sweep
/// lives in [`tests::the_plant_outer_correctors_converge`]. Measured
/// 2026-08-13, at the end of that test's 60 s flow-ramp transient:
///
/// | Correctors | Core outlet | Steam outlet | SG duty | Moved from the previous count by |
/// |---|---|---|---|---|
/// | 1 (plain Lie split) | 949.6218 K | 575.7429 K | 7.65765 MW | -- |
/// | **2 (shipped)** | **949.6679 K** | **575.5177 K** | **7.65733 MW** | dT_out +0.046 K, dT_steam -0.225 K, dQ -0.004% |
/// | 3 | 949.6666 K | 575.5170 K | 7.65732 MW | dT_out **-0.0013 K**, dT_steam **-0.0007 K**, dQ -0.0002% |
/// | *1 ms reference* | *949.669 K* | *574.696 K* | *7.65827 MW* | -- |
///
/// Two things to read off it. First, **the loop has converged at 2**: the third
/// corrector moves the core outlet by 1.3 mK and the steam outlet by 0.7 mK --
/// 35x and 320x smaller respectively than the changes the second corrector
/// itself made. Second, **the second corrector does real work**: it takes the core outlet from 0.047 K below the 1 ms reference
/// to 0.0015 K below it -- essentially onto it -- and the steam outlet from
/// +1.05 K to +0.82 K.
///
/// The residual +0.82 K on steam is **not** something more correctors fix; it
/// is the exchanger's zero-order-held boundary conditions, and the third
/// corrector confirms that by moving it 0.7 mK.
///
/// The extra corrector costs ~2% of a plant step, because it re-runs only the
/// cheap 4%.
pub const PLANT_OUTER_CORRECTORS: usize = 2;

/// **Longest sub-timestep the point kinetics is integrated with \[s\]**,
/// 0.001 s -- a hundredth of [`PLANT_TIMESTEP_S`], and deliberately the same
/// 1 ms this whole simulator used to run at.
///
/// [`kinetics`] is the second multi-rate sub-model in this plant, for the same
/// reason as [`steam_generator`]: it has an internal timescale the plant
/// timestep does not resolve. The prompt layer relaxes on
/// `Lambda / beta = 1e-3 / 0.0065 = 0.154 s`, so at the 0.1 s plant step the
/// Lie split between the prompt layer and the precursor bank is being run at
/// `dt / tau = 0.65` -- and that split is only first-order accurate.
///
/// **Measured 2026-08-13**, on the flow-ramp transient of
/// [`tests::the_plant_outer_correctors_converge`]. That transient drives the
/// reactor deeply subcritical through its own temperature feedback, so the
/// power decays by a factor of 60 over 60 s and the split error compounds --
/// the hardest case in this plant for a first-order split:
///
/// | Kinetics substep | Reactor power at 60 s | vs the 1 ms reference |
/// |---|---|---|
/// | 0.1 s (none -- one piece per plant step) | 0.02703 MW | **-84.5%** |
/// | 0.01 s | 0.14071 MW | **-19.3%** |
/// | **0.001 s (shipped)** | **0.17428 MW** | **+0.0000%** |
/// | 1 ms (the reference itself) | 0.17428 MW | -- |
///
/// **The exact agreement at 1 ms is structural, not a convergence result, and
/// should not be read as one.** Because the kinetics is decoupled from
/// everything else in the plant, the shipped run's 100 substeps of 1 ms and the
/// reference run's 100 steps of 1 ms are literally the same sequence of
/// operations. What the row demonstrates is that the sub-stepping is wired
/// correctly and costs nothing -- **not** that 1 ms is itself a converged
/// kinetics timestep. Nothing here establishes that; it is simply the rate the
/// simulator ran at before, so this change is a no-regression on that axis.
///
/// Every other plant quantity agreed to well under a kelvin even without
/// sub-stepping -- see that test's results table.
///
/// **The plant outer correctors cannot fix it.** The kinetics depends only on
/// the commanded reactivity and its own adiabatic fuel temperature, not on
/// anything the corrector loop iterates, so re-running it converges nothing.
/// Sub-stepping is the remedy and it is nearly free -- point kinetics is one
/// `atanh` and five first-order transfer functions per substep, against three
/// coupled array solves for the exchanger.
///
/// A **maximum**, not a fixed count: a caller already stepping finer than this
/// pays nothing extra, so the 1 ms reference leg of the accuracy test is
/// unaffected.
///
/// # Cost
///
/// Measured 2026-08-13: the whole-plant real-time ratio was **1.021** at a
/// 0.01 s kinetics substep and **1.037** at 0.001 s -- the same number within
/// run-to-run scatter. Point kinetics is one `atanh` and five first-order
/// transfer functions per substep, so a hundred of them per plant step is lost
/// in the noise of one steam-generator array solve.
pub const KINETICS_SUBSTEP_S: f64 = PLANT_TIMESTEP_S / 100.0;

/// The plant timestep as a `uom` [`Time`].
#[allow(dead_code)] // read by the whole-plant tests and by `crate::app` via the raw constant
pub fn plant_timestep() -> Time {
    Time::new::<second>(PLANT_TIMESTEP_S)
}

/// The steam-generator array sub-timestep \[s\], derived from
/// [`PLANT_TIMESTEP_S`] and [`STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP`].
pub fn steam_generator_substep_seconds() -> f64 {
    PLANT_TIMESTEP_S / STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP as f64
}

/// Nominal thermal power used to seed the kinetics and size the loops: 10 MWth
/// (published HTR-10 figure, IAEA-TECDOC-1382 Table 4-1), read from
/// [`outram_park_digital_twin_engine::htr10::design::Htr10DesignPoint`] rather
/// than re-typed here.
pub fn nominal_thermal_power() -> Power {
    pebble_bed::design().thermal_power
}

/// Nominal helium mass flow: 4.3 kg/s at full power (published, via the same
/// design point).
pub fn nominal_helium_flow() -> MassRate {
    pebble_bed::nominal_helium_flow()
}

/// The full plant model: kinetics + pebble-bed core + helium primary loop +
/// steam secondary loop, plus the running simulation clock.
pub struct HtgrPlant {
    /// Reactor kinetics slot (prompt excursion + delayed-neutron bank).
    pub kinetics: HtgrKinetics,
    /// Lumped pebble-bed core -- the graphite thermal inertia between the
    /// fission power and the helium.
    pub core: PebbleBedCore,
    /// Helium primary loop.
    pub primary: HeliumPrimaryLoop,
    /// Steam secondary loop.
    pub secondary: SteamSecondaryLoop,
    /// Turbine-generator rotor. Driven by the secondary loop's enthalpy-drop
    /// power through a real torque balance, so the schematic's turbine rotor
    /// turns at a computed shaft speed rather than an animation constant. See
    /// [`turbine_generator`] -- especially on why the speed lands near
    /// synchronous and why this is an islanded, ungoverned machine.
    pub shaft: TurbineGeneratorShaft,
    /// Reactor protection system. Trips on measurable signals and drives the
    /// rod bank in, so a prompt excursion terminates instead of running the
    /// model out of its property range. See [`protection`].
    pub protection: ReactorProtectionSystem,
    /// Accumulated simulation time.
    pub sim_time: Time,
    /// Heat rate crossing the pebble surface into the helium on the most recent
    /// step -- the core's *thermal* output, which lags the fission power by the
    /// graphite time constant.
    core_heat_to_helium: Power,
}

impl HtgrPlant {
    /// Construct the plant at the published HTR-10 operating point.
    pub fn new() -> Self {
        let nominal_power = nominal_thermal_power();
        Self {
            kinetics: HtgrKinetics::new_illustrative(nominal_power),
            core: PebbleBedCore::new(),
            primary: HeliumPrimaryLoop::new(nominal_helium_flow()),
            secondary: SteamSecondaryLoop::new(),
            shaft: TurbineGeneratorShaft::new(),
            protection: ReactorProtectionSystem::new(),
            sim_time: Time::new::<second>(0.0),
            core_heat_to_helium: Power::new::<watt>(0.0),
        }
    }

    /// Heat rate crossing the pebble surface into the helium on the most recent
    /// step. At steady state this equals the fission power; during a transient
    /// it lags it by the bed's ~184 s graphite time constant.
    #[allow(dead_code)] // snapshot candidate -- not yet wired into the app layer
    pub fn core_heat_to_helium(&self) -> Power {
        self.core_heat_to_helium
    }

    /// Lumped pebble (graphite) temperature -- a **bed average**, not a peak
    /// fuel temperature. See [`pebble_bed`] for why.
    #[allow(dead_code)] // snapshot candidate -- not yet wired into the app layer
    pub fn pebble_temperature(&self) -> ThermodynamicTemperature {
        self.core.temperature()
    }

    /// External reactivity in dollars currently commanded by the control-rod
    /// bank, for the given insertion fraction.
    ///
    /// The operator commands rod *position*; reactivity is what results. See
    /// [`control_rods`] for the published HTR-10 bank worth and cold clean
    /// excess this is derived from, and for what remains illustrative about it.
    pub fn external_reactivity_dollars(&self, control_rod_insertion_fraction: f64) -> f64 {
        control_rods::external_reactivity_dollars(
            control_rod_insertion_fraction,
            self.kinetics
                .delayed_neutron_fraction()
                .get::<uom::si::ratio::ratio>(),
        )
    }

    /// Advance the whole plant by one timestep `dt`, given the user control
    /// inputs (control-rod bank insertion fraction in `0..=1` and the helium
    /// pump flow setpoint).
    ///
    /// `dt` should be [`plant_timestep`]; the model is only tested at that
    /// value and at the 1 ms it used to run at.
    ///
    /// # Structure: an outer-corrector loop around the cheap subsystems
    ///
    /// The five subsystems are coupled in sequence, which leaves two quantities
    /// read one step late (the helium bulk temperature the bed sees, and the
    /// feedwater state the exchanger sees). At 0.1 s that lag matters, so the
    /// cheap lumped subsystems are rewound and re-advanced
    /// [`PLANT_OUTER_CORRECTORS`] times per timestep against improving
    /// estimates, while the **steam generator is advanced exactly once**, on
    /// the final corrector, with the converged boundary conditions. See
    /// [`PLANT_OUTER_CORRECTORS`] for why the exchanger is outside the loop and
    /// why the protection system and turbine shaft are too.
    pub fn step(
        &mut self,
        dt: Time,
        control_rod_insertion_fraction: f64,
        helium_flow_setpoint: MassRate,
    ) {
        self.step_with_correctors(
            dt,
            control_rod_insertion_fraction,
            helium_flow_setpoint,
            PLANT_OUTER_CORRECTORS,
        );
    }

    /// [`Self::step`] with the outer-corrector count supplied explicitly.
    ///
    /// Exists so the corrector count can be **measured** rather than asserted:
    /// [`PLANT_OUTER_CORRECTORS`] is a compile-time constant, so without this
    /// entry point no test could show that the loop has converged at it. See
    /// [`tests::the_plant_outer_correctors_converge`].
    ///
    /// `n_outer` is clamped to at least 1. `n_outer == 1` reproduces the plain
    /// sequential Lie-split ordering this plant used before 2026-08-13,
    /// step-for-step.
    ///
    /// Prefer [`Self::step`] in application code -- a simulator whose corrector
    /// count varies between call sites is a simulator with two different
    /// physics.
    pub fn step_with_correctors(
        &mut self,
        dt: Time,
        control_rod_insertion_fraction: f64,
        helium_flow_setpoint: MassRate,
        n_outer: usize,
    ) {
        self.sim_time += dt;

        // 0. Protection system. Evaluated on the PREVIOUS step's measured
        //    signals, before any reactivity is applied, so a trip cannot be
        //    outrun within a timestep. Its scram demand can only deepen the
        //    operator's rod command, never lift it. Deliberately OUTSIDE the
        //    corrector loop: correcting it against this step's own power is
        //    exactly the "outrun the trip" behaviour it is specified to
        //    prevent.
        self.protection.update(
            dt,
            self.kinetics.total_power(),
            self.primary.core_outlet_temperature(),
        );
        let control_rod_insertion_fraction = self
            .protection
            .effective_rod_insertion(control_rod_insertion_fraction);

        // Rod position is converted to reactivity here rather than in the GUI
        // so the physics owns the conversion and an OPC-UA write of a rod
        // position gets the same treatment as a slider drag. It is constant
        // over the step, so it is computed once outside the loop.
        let external_reactivity_dollars =
            self.external_reactivity_dollars(control_rod_insertion_fraction);

        // 1. Kinetics -> reactor fission power. OUTSIDE the corrector loop, and
        //    provably so: the prompt layer and the precursor bank depend only
        //    on `external_reactivity_dollars` (constant over the step) and on
        //    their own state, including the prompt layer's *own* adiabatic fuel
        //    temperature. Nothing the loop iterates reaches them, so a second
        //    pass would return a bit-identical answer.
        //
        //    If the refinement this module's docs name -- the reactivity
        //    feedback reading `pebble_bed`'s graphite temperature instead of
        //    the prompt layer's separate node -- is ever made, the kinetics
        //    becomes genuinely coupled and MUST move inside the loop.
        //    `HtgrKinetics` is `Clone` so that it can.
        //
        //    Each subsystem announces itself before stepping, so a panic
        //    anywhere below is attributed to a piece of PLANT EQUIPMENT in the
        //    crash modal rather than only to a source file. The whole plant
        //    runs on one physics thread, so the thread name alone identifies
        //    nothing.
        mark_component("reactor kinetics (point kinetics + control rods)");
        self.kinetics.step(dt, external_reactivity_dollars);
        let reactor_power = self.kinetics.total_power();

        // Old-time snapshot of everything the corrector loop re-advances. All
        // of it is scalar, so this is cheap next to one exchanger substep.
        let core_at_step_start = self.core;
        let primary_at_step_start = self.primary.lumped_state();
        let secondary_at_step_start = self.secondary.integrated_state();

        // The coupling variables. Initialised to their start-of-step values --
        // which is exactly what a single-corrector (plain Lie-split) step would
        // use throughout, so `PLANT_OUTER_CORRECTORS == 1` reproduces the
        // pre-2026-08-13 sequencing.
        let mut helium_bulk = self.primary.helium_bulk_temperature();
        let mut feedwater_enthalpy = self.secondary.feedwater_enthalpy();
        let mut secondary_flow = self.secondary.mass_flow();

        let n_outer = n_outer.max(1);
        for corrector in 0..n_outer {
            if corrector > 0 {
                self.core = core_at_step_start;
                self.primary.restore_lumped_state(primary_at_step_start);
                self.secondary
                    .restore_integrated_state(secondary_at_step_start);
            }

            // 2. Pebble bed absorbs the fission power and hands the helium only
            //    what crosses the pebble surface. `helium_bulk` is the
            //    start-of-step value on the first corrector and the previous
            //    corrector's end-of-step value thereafter, so the coupling
            //    moves from explicit toward implicit as the loop iterates. The
            //    bed's ~184 s time constant makes this the least sensitive of
            //    the couplings either way.
            mark_component("pebble-bed core (graphite pebbles)");
            self.core_heat_to_helium =
                self.core
                    .step(dt, reactor_power, helium_bulk, self.primary.mass_flow());

            // 3a. Primary hot leg: helium properties and the core-outlet
            //     temperature. Cheap (one CoolProp flash), so it is inside the
            //     loop.
            mark_component("helium primary loop (circulator + hot gas duct)");
            self.primary
                .step_hot_leg(dt, self.core_heat_to_helium, helium_flow_setpoint);

            // 3b. The steam generator, ONCE, on the final corrector -- with the
            //     converged core-outlet temperature as its hot inlet and the
            //     converged feedwater state as its cold inlet. Its three arrays
            //     hold spatial history and cannot be rewound, and it is ~96% of
            //     this plant's compute, so it is advanced exactly once per
            //     timestep. On the earlier correctors the loop runs on the
            //     previous timestep's duty, which is the predictor.
            if corrector + 1 == n_outer {
                self.primary
                    .advance_steam_generator(dt, feedwater_enthalpy, secondary_flow);
            }

            // 3c. Primary return leg: the core inlet relaxes toward the
            //     exchanger's helium-side outlet, closing the circuit, and the
            //     loop hydraulics are updated.
            self.primary.close_return_leg(dt);

            // 4. Secondary steam loop, driven by the duty the steam generator's
            //    TUBE SIDE actually absorbed -- not by the heat the helium gave
            //    up. The two differ by the tube metal's stored-energy rate,
            //    which is exactly the transient the metal exists to provide.
            //
            //    The core outlet is still handed over as the hot-side inlet,
            //    because `secondary_loop::max_absorbable_duty` is retained as a
            //    backstop. It should no longer bind: the exchanger's own outlet
            //    can never exceed the local helium temperature, so the enthalpy
            //    balance downstream is already bounded. See
            //    `secondary_loop::tests::the_absorbable_duty_cap_no_longer_binds`.
            mark_component("steam generator + secondary steam loop (IF97)");
            self.secondary.step(
                dt,
                self.primary.steam_generator_duty_to_secondary(),
                self.primary.core_outlet_temperature(),
            );

            // Hand the improved coupling values to the next corrector.
            helium_bulk = self.primary.helium_bulk_temperature();
            feedwater_enthalpy = self.secondary.feedwater_enthalpy();
            secondary_flow = self.secondary.mass_flow();
        }

        // 5. Turbine-generator shaft. Driven by the SAME enthalpy-drop power
        // the secondary loop just computed -- `T = P/omega`, so there is one
        // turbine power in this plant, not two. The rotor's speed is what the
        // schematic's turbine widget draws its blades turning at. Outside the
        // corrector loop: it is a pure consumer, reading the converged turbine
        // power and feeding nothing back.
        mark_component("turbine-generator shaft (torque balance)");
        self.shaft
            .step(dt, self.secondary.turbine_power(), self.sim_time);
    }

    /// Project the current plant state onto the shared [`HtgrSnapshot`],
    /// writing only the *output* fields and leaving the GUI-owned control
    /// inputs untouched.
    pub fn write_snapshot(&self, s: &mut HtgrSnapshot) {
        // Kinetics.
        s.reactor_power_mw = power_in_megawatts(self.kinetics.total_power());
        s.prompt_power_mw = power_in_megawatts(self.kinetics.prompt_power());
        s.delayed_power_mw = power_in_megawatts(self.kinetics.delayed_power());
        s.fuel_temperature_k = self.kinetics.prompt.fuel_temperature.get::<kelvin>();
        // The BED temperature, not the kinetics fuel node. The two differ: the
        // kinetics node is a lumped point-kinetics fuel temperature driving
        // reactivity feedback, while this is the graphite the helium flows
        // over. Drawing the kinetics node made the bed appear COOLER than the
        // gas leaving it, which is thermodynamically impossible.
        s.bed_temperature_k = self.pebble_temperature().get::<kelvin>();
        s.trip_reason = self.protection.trip_reason();
        s.scram_insertion_fraction = self.protection.scram_insertion();
        s.reactivity_margin_dollars = self.kinetics.reactivity_margin_dollars();
        s.delayed_neutron_fraction_pcm = self
            .kinetics
            .delayed_neutron_fraction()
            .get::<uom::si::ratio::ratio>()
            * 1.0e5;

        // Primary loop.
        s.core_inlet_temp_k = self.primary.core_inlet_temperature().get::<kelvin>();
        s.core_outlet_temp_k = self.primary.core_outlet_temperature().get::<kelvin>();
        s.helium_mass_flow_kg_per_s = self.primary.mass_flow().get::<kilogram_per_second>();
        s.ihx_duty_mw = self.primary.ihx_duty().get::<megawatt>();
        s.ihx_outlet_temp_k = self.primary.ihx_outlet_temperature().get::<kelvin>();
        s.helium_residence_time_s =
            residence_time_from_flow(self.primary.helium_inventory(), self.primary.mass_flow())
                .get::<second>();
        s.primary_pressure_drop_kpa = self.primary.pressure_drop().get::<kilopascal>();
        s.bed_pressure_drop_kpa = self.primary.bed_pressure_drop().get::<kilopascal>();
        s.circulator_power_mw = self.primary.circulator_power().get::<megawatt>();
        s.helium_cp_j_per_kg_k =
            self.primary
                .specific_heat()
                .get::<uom::si::specific_heat_capacity::joule_per_kilogram_kelvin>();

        // Secondary loop.
        s.steam_pressure_mpa = self
            .secondary
            .steam_pressure()
            .get::<uom::si::pressure::megapascal>();
        s.sg_steam_outlet_temp_k = self.secondary.turbine_inlet_temperature().get::<kelvin>();
        s.turbine_inlet_temp_k = self.secondary.turbine_inlet_temperature().get::<kelvin>();
        s.steam_enthalpy_j_per_kg = self
            .secondary
            .steam_generator_outlet()
            .get_specific_enthalpy()
            .get::<uom::si::available_energy::joule_per_kilogram>();
        s.turbine_power_mw = self.secondary.turbine_power().get::<megawatt>();
        s.steam_quality_after_turbine = self.secondary.steam_quality_after_turbine();
        s.condenser_pressure_kpa = self.secondary.condenser_pressure().get::<kilopascal>();
        s.secondary_mass_flow_kg_per_s = self.secondary.mass_flow().get::<kilogram_per_second>();
        s.secondary_residence_time_s =
            residence_time_from_flow(self.secondary.inventory(), self.secondary.mass_flow())
                .get::<second>();
        s.feedwater_enthalpy_j_per_kg = self
            .secondary
            .feedwater_enthalpy()
            .get::<uom::si::available_energy::joule_per_kilogram>();
        s.condensate_enthalpy_j_per_kg = self
            .secondary
            .condensate()
            .get_specific_enthalpy()
            .get::<uom::si::available_energy::joule_per_kilogram>();
        s.feed_pump_power_mw = self.secondary.feed_pump_power().get::<megawatt>();
        s.net_cycle_power_mw = self.secondary.net_power().get::<megawatt>();
        s.condenser_duty_mw = self.secondary.condenser_duty().get::<megawatt>();
        s.cooling_water_outlet_temp_k = self
            .secondary
            .cooling_water_outlet_temperature()
            .get::<kelvin>();

        // Turbine-generator shaft.
        s.shaft_speed_rad_per_s = self
            .shaft
            .angular_velocity()
            .get::<uom::si::angular_velocity::radian_per_second>();
        s.shaft_speed_rpm = self.shaft.speed_rpm();
        s.generator_electrical_power_mw = self.shaft.electrical_power().get::<megawatt>();
        s.generator_rating_mw = self.shaft.rated_shaft_power().get::<megawatt>();

        // Clock.
        s.sim_time_s = self.sim_time.get::<second>();
    }
}

impl Default for HtgrPlant {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::HtgrSnapshot;

    /// The control-rod insertion fraction the GUI opens with, from
    /// `crate::app::state::HtgrSnapshot::default` -- the bank position at which
    /// the core is critical with no external reactivity, from
    /// [`control_rods::critical_insertion_fraction`].
    ///
    /// Kept in step with the app default by
    /// [`the_test_rod_position_matches_the_gui_default`].
    const HTGR_GUI_INITIAL_ROD_INSERTION: f64 = 0.6035;

    /// The whole-plant test must open where the GUI opens. If the app's default
    /// rod position moves, this catches it -- withdrawing the bank by even ten
    /// percent from critical is a prompt excursion in this core, so the two must
    /// not be allowed to drift apart silently.
    #[test]
    fn the_test_rod_position_matches_the_gui_default() {
        let gui_default = HtgrSnapshot::default().control_rod_insertion_fraction;
        assert!(
            (gui_default - HTGR_GUI_INITIAL_ROD_INSERTION).abs() < 1e-9,
            "the GUI opens at rod insertion {gui_default}, this test uses \
             {HTGR_GUI_INITIAL_ROD_INSERTION}"
        );
    }

    /// V&V (regression **and** the simulator's headline speed measurement):
    /// **the whole plant survives being stepped at the GUI's own timestep**,
    /// end to end, and this is where the real-time ratio is measured.
    ///
    /// # Why this exists
    ///
    /// `crate::app`'s physics thread drives `HtgrPlant::step` at
    /// [`PLANT_TIMESTEP_S`], and measured 2026-08-12 a whole green test suite
    /// at 0.05 s coexisted with a simulator that killed its physics thread
    /// within 30 s of launch, because the rate the application ran at was a
    /// rate nothing tested. Both sides now read the same constant, so they
    /// cannot diverge -- but the coverage is still worth having, because the
    /// steam generator's arrays are unstable **both** above and below a window
    /// and the plant timestep is what feeds them.
    ///
    /// It is also the timing harness: 20 s of plant time through the exact call
    /// the GUI makes, so its wall clock divided by 20 s is the compute cost per
    /// second of plant time, and the reciprocal is the achievable real-time
    /// ratio.
    ///
    /// # Methodology
    ///
    /// A fresh plant stepped over 20 s of simulated time -- the window in which
    /// the 2026-08-12 crash occurred -- at the GUI's opening rod position and
    /// flow, at [`plant_timestep`]. Asserted: no panic, no node-by-node cross,
    /// the tube metal inside `SteelSS304LHighTemp`'s range, the clock advanced,
    /// and the exchanger's hot array never clamped its enthalpy field (the
    /// fingerprint of a Courant breakdown -- stronger than "it did not panic").
    ///
    /// # Results
    ///
    /// **2026-08-12, at the then-current 1 ms timestep**: passed, in 41.79 s
    /// (quiescent) / 49.00 s (loaded) of wall clock for 20 s of plant time --
    /// **2.09-2.45 s of compute per second of plant time**, a real-time ratio
    /// of **0.41-0.48**.
    ///
    /// **2026-08-13, re-measured at 1 ms before any change**: 40.65 s wall,
    /// **2.0327** compute per plant second, ratio **0.492**.
    ///
    /// **2026-08-13, with the plant timestep raised to 0.1 s and nothing else
    /// changed**: **1.9469** compute per plant second, ratio **0.514**. The
    /// timestep alone bought only 4%, because the steam generator is ~96% of
    /// the cost and runs on its own clock (measured alone: 1.9585).
    ///
    /// **This measurement is a floor, not a ceiling.** It is taken with
    /// [`STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP`] = 8, and that 8 exists only
    /// to respect the Courant limit of the exchanger arrays' *explicit*
    /// enthalpy convection -- a limitation kopi-beans `op-j2oq` is removing by
    /// giving `TampinesSteamArray` an implicit `fvm::div` energy mode. When that
    /// lands the substep count should fall and this ratio should rise. **No
    /// figure is extrapolated here for what it would become**; that is being
    /// measured properly under `op-j2oq` and a guess would only be quoted back
    /// later as if it were data.
    ///
    /// **2026-08-13, shipped configuration** -- 0.1 s plant timestep,
    /// [`PLANT_OUTER_CORRECTORS`] = 2, [`KINETICS_SUBSTEP_S`] = 1 ms, exchanger
    /// arrays at 2 outer correctors instead of 4, measured with this test run
    /// **alone** (`--test-threads=1`): 19.29-19.36 s wall for 20 s of plant
    /// time over four runs, **0.9646-0.9677** compute per plant second,
    /// **real-time ratio 1.033-1.037**. The simulator reaches real time.
    ///
    /// Where that 2.02x came from, in order of size: **halving the exchanger's
    /// PIMPLE outer correctors from 4 to 2** (~1.98x, and it changed the
    /// settled duty by nothing measurable -- see
    /// [`steam_generator::PimpleCorrectors`]), then the plant timestep itself
    /// (~1.04x). The plant outer correctors and the kinetics sub-stepping are
    /// accuracy measures and cost under 2% between them.
    ///
    /// **Run it under load and you will not see 1.037.** The same test measured
    /// 0.562 with the rest of this suite running on the other eleven cores.
    ///
    /// # Interpretation
    ///
    /// The wall-clock figure printed here is a **measurement of the maintainer's
    /// workstation**, not a property of the model, and it moves with machine
    /// load. It is not asserted on -- a timing assertion would fail on a busy
    /// CI box for no physical reason. What is asserted is that the plant
    /// survives its own GUI timestep.
    #[test]
    fn the_whole_plant_steps_at_the_gui_timestep() {
        let mut plant = HtgrPlant::new();
        let mut snapshot = HtgrSnapshot::default();
        // The GUI's plant timestep -- the same constant `crate::app` reads.
        let dt = plant_timestep();
        let plant_seconds = 20.0_f64;
        let steps = (plant_seconds / PLANT_TIMESTEP_S).round() as usize;
        let flow = nominal_helium_flow();
        let mut worst_metal_k = 0.0_f64;

        let started = std::time::Instant::now();
        for i in 0..steps {
            plant.step(dt, HTGR_GUI_INITIAL_ROD_INSERTION, flow);
            let sg = plant.primary.steam_generator_state();
            assert!(
                sg.worst_node_cross_kelvin() <= 1e-6,
                "temperature cross at plant step {i}"
            );
            for t in sg.metal_node_temperatures.iter() {
                worst_metal_k = worst_metal_k.max(t.get::<kelvin>());
            }
        }
        let wall = started.elapsed().as_secs_f64();
        plant.write_snapshot(&mut snapshot);
        println!(
            "GUI-TIMESTEP RUN ({plant_seconds} s at {PLANT_TIMESTEP_S} s, \
             {PLANT_OUTER_CORRECTORS} plant outer correctors):\n  \
             power {:.4} MW, core outlet {:.2} K, steam {:.2} K, \
             peak tube metal {worst_metal_k:.2} K\n  \
             wall clock {wall:.3} s for {plant_seconds} s of plant time = \
             {:.4} compute per plant second, REAL-TIME RATIO {:.3}",
            snapshot.reactor_power_mw,
            snapshot.core_outlet_temp_k,
            snapshot.sg_steam_outlet_temp_k,
            wall / plant_seconds,
            plant_seconds / wall,
        );
        assert!(
            worst_metal_k < 1700.0,
            "tube metal reached {worst_metal_k} K, outside SteelSS304LHighTemp's range"
        );
        assert_eq!(
            plant.primary.steam_generator_enthalpy_clamp_events(),
            0,
            "the exchanger's hot array clamped its enthalpy field -- the fingerprint of a \
             Courant breakdown at this timestep"
        );
        assert!(plant.sim_time.get::<second>() > plant_seconds - PLANT_TIMESTEP_S);
    }

    /// V&V: **the plant outer-corrector loop has converged at
    /// [`PLANT_OUTER_CORRECTORS`]**, and one corrector is not enough at 0.1 s.
    ///
    /// # Methodology
    ///
    /// One transient, two questions. The transient is a circulator flow
    /// ramp-down: 4.3 kg/s held for 10 s, ramped linearly to 3.0 kg/s over the
    /// next 10 s, then held, read at 60 s. It exercises every lagged coupling
    /// at once -- it moves the helium bulk temperature the bed reads, the
    /// hot-inlet temperature the exchanger is handed, and (through the duty)
    /// the feedwater flow -- and it drives the reactor deeply subcritical
    /// through its own temperature feedback, which is the hardest case for the
    /// kinetics split.
    ///
    /// 1. **Has the corrector loop converged?** The plant is run at
    ///    [`plant_timestep`] with 1, 2 and 3 correctors via
    ///    [`HtgrPlant::step_with_correctors`], and each result is compared with
    ///    the one before it. (This is why that entry point exists:
    ///    [`PLANT_OUTER_CORRECTORS`] is a compile-time constant, so a claim
    ///    that it is converged is unfalsifiable without it.)
    /// 2. **How much accuracy does the 0.1 s step cost?** The shipped
    ///    configuration is compared against a **reference integration at
    ///    1 ms**, 100x finer.
    ///
    /// # Results (measured 2026-08-13)
    ///
    /// **Corrector sweep** at the 0.1 s plant timestep -- the full table is in
    /// [`PLANT_OUTER_CORRECTORS`]. The third corrector moves the core outlet by
    /// **-0.0013 K** and the steam outlet by **-0.0007 K** against the second,
    /// so the loop is converged at two.
    ///
    /// **Accuracy against the 1 ms reference.** State at 60 s, shipped
    /// configuration (0.1 s plant timestep, [`PLANT_OUTER_CORRECTORS`] = 2,
    /// [`KINETICS_SUBSTEP_S`] = 1 ms):
    ///
    /// | Quantity | dt = 1 ms | dt = 0.1 s | Difference |
    /// |---|---|---|---|
    /// | Reactor power | 0.17428 MW | 0.17428 MW | **+0.0000%** |
    /// | Core outlet | 949.669 K | 949.668 K | **-0.0015 K** |
    /// | Bed temperature | 896.644 K | 896.590 K | **-0.054 K** |
    /// | Steam outlet | 574.696 K | 575.518 K | **+0.822 K** |
    /// | SG duty | 7.65827 MW | 7.65733 MW | **-0.012%** |
    ///
    /// For contrast, the same comparison **before** the kinetics was made
    /// multi-rate (i.e. one kinetics piece per 0.1 s plant step): reactor power
    /// 0.02703 MW, **-84.5%**, with every other quantity already inside a
    /// kelvin. The whole of the accuracy loss at 0.1 s was in the kinetics, and
    /// none of it was in the thermal hydraulics.
    ///
    /// # Interpretation
    ///
    /// A 0.1 s step is **not** automatically as accurate as a 1 ms step, and
    /// this test says how much is given up rather than pretending the
    /// difference is zero.
    ///
    /// **The steam outlet is the one quantity that genuinely degrades**, by
    /// +0.82 K. It sits downstream of the exchanger's zero-order-held boundary
    /// conditions, which the outer correctors improve but do not remove: the
    /// hot inlet is still frozen across all 8 array substeps within a plant
    /// step, only now at the converged end-of-step value rather than the
    /// start-of-step one. Removing that residue would need the hot inlet ramped
    /// across the substeps, which is not implemented.
    ///
    /// **The reactor-power agreement is exact for a structural reason**, not
    /// because the coupling converged -- see [`KINETICS_SUBSTEP_S`]. Do not
    /// read it as evidence that the kinetics is converged at 1 ms.
    ///
    /// This test is **slow** -- the 1 ms reference leg alone is 60 s of plant
    /// time at ~2 s of compute per plant second.
    #[test]
    fn the_plant_outer_correctors_converge() {
        let reference = flow_ramp_transient(Time::new::<second>(1.0e-3), PLANT_OUTER_CORRECTORS);
        let shipped = flow_ramp_transient(plant_timestep(), PLANT_OUTER_CORRECTORS);

        // The corrector sweep: how much does each extra corrector move the
        // answer? This is what establishes that PLANT_OUTER_CORRECTORS is
        // converged rather than merely chosen.
        println!("PLANT OUTER-CORRECTOR SWEEP at dt = {PLANT_TIMESTEP_S} s (read at 60 s)");
        let mut previous: Option<TransientEndState> = None;
        for n in 1..=3_usize {
            let r = flow_ramp_transient(plant_timestep(), n);
            match &previous {
                None => println!(
                    "  {n} corrector : P = {:.5} MW, T_out = {:.4} K, T_bed = {:.4} K, \
                     T_steam = {:.4} K, Q = {:.5} MW",
                    r.power_mw, r.core_outlet_k, r.bed_k, r.steam_k, r.duty_mw
                ),
                Some(p) => println!(
                    "  {n} correctors: P = {:.5} MW, T_out = {:.4} K, T_bed = {:.4} K, \
                     T_steam = {:.4} K, Q = {:.5} MW  (moved from {} by dT_out = {:+.5} K, \
                     dT_steam = {:+.5} K, dQ = {:+.5}%)",
                    r.power_mw,
                    r.core_outlet_k,
                    r.bed_k,
                    r.steam_k,
                    r.duty_mw,
                    n - 1,
                    r.core_outlet_k - p.core_outlet_k,
                    r.steam_k - p.steam_k,
                    100.0 * (r.duty_mw - p.duty_mw) / p.duty_mw,
                ),
            }
            previous = Some(r);
        }

        println!(
            "FLOW-RAMP TRANSIENT (4.3 -> 3.0 kg/s over 10-20 s, read at 60 s)\n  \
             reference, dt = 1 ms  : P = {:.5} MW, T_out = {:.3} K, T_bed = {:.3} K, \
             T_steam = {:.3} K, Q = {:.5} MW\n  \
             shipped,   dt = {PLANT_TIMESTEP_S} s : P = {:.5} MW, T_out = {:.3} K, \
             T_bed = {:.3} K, T_steam = {:.3} K, Q = {:.5} MW\n  \
             difference            : dP = {:+.4}%, dT_out = {:+.4} K, dT_bed = {:+.4} K, \
             dT_steam = {:+.4} K, dQ = {:+.4}%",
            reference.power_mw,
            reference.core_outlet_k,
            reference.bed_k,
            reference.steam_k,
            reference.duty_mw,
            shipped.power_mw,
            shipped.core_outlet_k,
            shipped.bed_k,
            shipped.steam_k,
            shipped.duty_mw,
            100.0 * (shipped.power_mw - reference.power_mw) / reference.power_mw,
            shipped.core_outlet_k - reference.core_outlet_k,
            shipped.bed_k - reference.bed_k,
            shipped.steam_k - reference.steam_k,
            100.0 * (shipped.duty_mw - reference.duty_mw) / reference.duty_mw,
        );

        assert!(
            ((shipped.power_mw - reference.power_mw) / reference.power_mw).abs() < 0.01,
            "reactor power drifted {:+.3}% from the 1 ms reference",
            100.0 * (shipped.power_mw - reference.power_mw) / reference.power_mw
        );
        assert!(
            (shipped.core_outlet_k - reference.core_outlet_k).abs() < 0.5,
            "core outlet drifted {:+.3} K from the 1 ms reference",
            shipped.core_outlet_k - reference.core_outlet_k
        );
        assert!(
            (shipped.bed_k - reference.bed_k).abs() < 0.5,
            "bed temperature drifted {:+.3} K from the 1 ms reference",
            shipped.bed_k - reference.bed_k
        );
        assert!(
            (shipped.steam_k - reference.steam_k).abs() < 2.0,
            "steam outlet drifted {:+.3} K from the 1 ms reference",
            shipped.steam_k - reference.steam_k
        );
    }

    /// The end-of-transient state [`the_plant_outer_correctors_converge`]
    /// compares between timesteps.
    struct TransientEndState {
        power_mw: f64,
        core_outlet_k: f64,
        bed_k: f64,
        steam_k: f64,
        duty_mw: f64,
    }

    /// Run the circulator flow ramp-down transient at `dt` with `n_outer` plant
    /// outer correctors, and read the plant at 60 s.
    fn flow_ramp_transient(dt: Time, n_outer: usize) -> TransientEndState {
        let dt_s = dt.get::<second>();
        let mut plant = HtgrPlant::new();
        let steps = (60.0 / dt_s).round() as usize;
        for i in 0..steps {
            let t = i as f64 * dt_s;
            let flow_kg_s = if t < 10.0 {
                4.3
            } else if t < 20.0 {
                4.3 - 1.3 * (t - 10.0) / 10.0
            } else {
                3.0
            };
            plant.step_with_correctors(
                dt,
                HTGR_GUI_INITIAL_ROD_INSERTION,
                MassRate::new::<kilogram_per_second>(flow_kg_s),
                n_outer,
            );
        }
        let mut s = HtgrSnapshot::default();
        plant.write_snapshot(&mut s);
        TransientEndState {
            power_mw: s.reactor_power_mw,
            core_outlet_k: s.core_outlet_temp_k,
            bed_k: s.bed_temperature_k,
            steam_k: s.sg_steam_outlet_temp_k,
            duty_mw: s.ihx_duty_mw,
        }
    }

    /// V&V: **the exchanger keeps its Courant margin at the circulator's
    /// ceiling**, which is the worst case the operator can command.
    ///
    /// # Why this exists
    ///
    /// The exchanger's array outer-corrector count was halved from 4 to 2 on
    /// 2026-08-13 because the settled duty was measured identical at 1, 2, 3 and
    /// 4 (see [`steam_generator::PimpleCorrectors`]). That measurement was taken
    /// at the **nominal** 4.3 kg/s, where the hot-side Courant number is 0.222.
    /// The operator can drive the circulator to
    /// `MAX_HELIUM_FLOW_KG_PER_S` = 8.0 kg/s -- 1.86x nominal -- which raises
    /// the gas velocity and the Courant number roughly in proportion. Halving
    /// the correctors on the strength of a nominal-point measurement, without
    /// checking the worst point the GUI can reach, would be exactly the kind of
    /// unfalsified claim this workspace's rules forbid.
    ///
    /// # Methodology
    ///
    /// The plant is stepped over 100 s of simulated time at
    /// [`plant_timestep`] with the circulator commanded to 8.0 kg/s from the
    /// first step -- a step change, not a ramp, so the hot array is also given a
    /// flow transient rather than a settled profile. Asserted at every step: no
    /// node-by-node temperature cross, and **zero enthalpy clamp events on the
    /// hot array** over the whole run. The clamp counter is the direct
    /// instrument: it counts every occasion the array had to limit its enthalpy
    /// field against its own bounds, which is what a Courant breakdown produces
    /// before it produces a panic. A run that only passes because the clamp is
    /// catching it is not a stable run, and this test is written so that
    /// distinction cannot hide behind a green result.
    ///
    /// The measured Courant number at the settled high-flow state is printed.
    ///
    /// # Results (measured 2026-08-13)
    ///
    /// **Results (re-measured 2026-08-13, at 2 sub-steps with the helium side
    /// implicit).** 8.0 kg/s for 100 s: `Co_hot` = **1.4404**, `Co_cold` =
    /// 0.1657 at the 0.05 s array substep, **0 enthalpy clamp events over 1000
    /// plant steps**, and no temperature cross at any step.
    ///
    /// `Co_hot` is **above 1 and that is the point.** Under the previous
    /// explicit convection this operating point was unreachable -- the Picard
    /// corrector loop's contraction factor is the cell Courant number, so it
    /// would have diverged however many correctors were used.
    /// `EnergyBalanceMode::Implicit` removes that bound, and the zero clamp
    /// count is the evidence that it genuinely holds rather than merely not
    /// crashing: the array never needed its enthalpy field rescued at 1.86x
    /// nominal flow.
    ///
    /// # Interpretation
    ///
    /// This bounds the corrector reduction: 2 outer correctors are enough not
    /// just at the design point but at the most demanding flow the operator can
    /// command. It does **not** license raising the array substep -- that is a
    /// separate limit and is measured in
    /// [`steam_generator::tests::the_courant_number_bounds_the_array_substep`].
    #[test]
    fn the_exchanger_holds_its_courant_margin_at_maximum_circulator_flow() {
        let mut plant = HtgrPlant::new();
        let dt = plant_timestep();
        let steps = (100.0 / PLANT_TIMESTEP_S).round() as usize;
        // The circulator ceiling -- `primary_loop::MAX_HELIUM_FLOW_KG_PER_S`.
        let flow = MassRate::new::<kilogram_per_second>(8.0);

        for i in 0..steps {
            plant.step(dt, HTGR_GUI_INITIAL_ROD_INSERTION, flow);
            let sg = plant.primary.steam_generator_state();
            assert!(
                sg.worst_node_cross_kelvin() <= 1e-6,
                "temperature cross at high-flow plant step {i}"
            );
        }

        let clamps = plant.primary.steam_generator_enthalpy_clamp_events();
        let (co_hot, co_cold) = plant
            .primary
            .steam_generator_courant_numbers(steam_generator_substep_seconds());
        println!(
            "MAXIMUM CIRCULATOR FLOW (8.0 kg/s, 1.86x nominal, 100 s at {PLANT_TIMESTEP_S} s):\n  \
             Co_hot = {co_hot:.4}, Co_cold = {co_cold:.4} at the {:.4} s array substep \
             (nominal flow gives Co_hot = 0.222)\n  \
             hot-array enthalpy clamp events = {clamps} over {steps} plant steps",
            steam_generator_substep_seconds()
        );

        assert_eq!(
            clamps, 0,
            "the hot array clamped its enthalpy field {clamps} times at the circulator \
             ceiling. The exchanger is being held together by the clamp rather than being \
             stable there, and 2 outer correctors are not enough at this flow."
        );
        // NOTE: this deliberately no longer asserts `Co_hot < 1`.
        //
        // That assertion encoded the *explicit* convection stability bound. The
        // helium side now runs `EnergyBalanceMode::Implicit`, which has no such
        // bound -- the whole point of the change -- so at the 0.05 s substep and
        // the circulator ceiling `Co_hot` legitimately exceeds 1. Keeping the
        // old assertion would have failed a run that is behaving exactly as
        // designed.
        //
        // The clamp assertion above is the stronger evidence and is retained
        // unchanged: it checks the array's enthalpy field never had to be
        // rescued, which is a direct statement about stability rather than a
        // proxy for it. If the implicit scheme were struggling here, the clamp
        // would fire; it does not.
        //
        // Co is still measured and printed, because it is the number that says
        // how much of the margin the implicit treatment is actually spending.
        assert!(
            co_hot.is_finite() && co_hot > 0.0,
            "Co_hot = {co_hot} is not a usable measurement"
        );
    }

    /// V&V: the steam-generator array substep is a whole division of the plant
    /// timestep, so no simulated time is stranded in the exchanger's
    /// accumulator.
    ///
    /// **Methodology.** The exchanger accumulates whatever `dt` it is handed and
    /// advances in whole substeps, carrying any remainder. If the substep does
    /// not divide the plant timestep exactly, the number of array advances per
    /// plant step alternates and the exchanger's effective clock beats against
    /// the plant's -- a slow, hard-to-attribute error. Pin the exact division.
    ///
    /// **Results (2026-08-13, re-measured after the sub-step count fell 8 -> 2).**
    /// `PLANT_TIMESTEP_S = 0.1 s`, `STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP = 2`,
    /// substep 0.05 s, remainder 0.0 s exactly, and 2 whole array advances per
    /// plant step. Interpretation: the exchanger advances the same number of
    /// substeps every plant step, with nothing carried.
    ///
    /// The expected substep is **derived** from the two constants rather than
    /// written as a literal. The previous version pinned `0.0125` directly,
    /// which meant the test failed the moment the sub-step count changed --
    /// reporting a stale expectation rather than the exact-division property it
    /// exists to guard. Deriving it keeps the guard alive at any sub-step count.
    #[test]
    fn the_steam_generator_substep_divides_the_plant_timestep() {
        let substep = steam_generator_substep_seconds();
        let expected = PLANT_TIMESTEP_S / STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP as f64;
        assert!(
            (substep - expected).abs() < 1e-15,
            "substep is {substep} s, expected {expected} s from PLANT_TIMESTEP_S / \
             STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP"
        );
        let quotient = PLANT_TIMESTEP_S / substep;
        assert!(
            (quotient - quotient.round()).abs() < 1e-12,
            "the substep does not divide the plant timestep: {quotient} substeps per step"
        );
        assert_eq!(
            quotient.round() as usize,
            STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP
        );
    }

    /// V&V: **the whole plant runs, and its steam generator holds the second law
    /// at every node, through the path the GUI actually drives.**
    ///
    /// # Why this test exists
    ///
    /// Every other test in this simulator drives one subsystem, or at most the
    /// primary and secondary loops in isolation. [`HtgrPlant::step`] is the only
    /// thing the GUI calls, and until 2026-08-12 nothing exercised it: the
    /// kinetics, the protection system, the pebble bed, the steam generator and
    /// the turbine shaft were each tested apart and integrated only in
    /// production. The steam-generator rework put a stiff, panicking dependency
    /// (three coupled CFD-style arrays, an IF97 flash that panics out of range
    /// and a steel property table that panics above 1000 K) into that path,
    /// which is a good reason to cover it.
    ///
    /// # Methodology
    ///
    /// A fresh [`HtgrPlant`] is stepped over 150 s of simulated time at
    /// [`plant_timestep`] with the control rods at the **critical insertion the GUI opens
    /// with** ([`HTGR_GUI_INITIAL_ROD_INSERTION`] = 0.6035, the same value
    /// `crate::app::state::HtgrSnapshot::default` carries) and the circulator at
    /// the published 4.3 kg/s. At every step:
    ///
    /// - the steam generator's node-by-node cross measure must be zero;
    /// - the steam-generator tube metal must stay inside `SteelSS304L`'s
    ///   tabulated range (below 1000 K), since TUAS panics rather than
    ///   extrapolating;
    /// - every snapshot field the GUI reads must be finite.
    ///
    /// The snapshot projection [`HtgrPlant::write_snapshot`] is exercised too, so
    /// a field wired to a removed accessor is caught here rather than on screen.
    ///
    /// # Results (measured 2026-08-12)
    ///
    /// 2026-08-12 (3000 steps at 0.05 s): no panic, zero crosses.
    ///
    /// **Re-measured 2026-08-13** at the 0.1 s plant timestep with
    /// [`PLANT_OUTER_CORRECTORS`] = 2 and the exchanger at 2 outer correctors
    /// -- same 150 s of simulated time, 1500 steps:
    ///
    /// | Quantity | Value |
    /// |---|---|
    /// | Reactor power | 0.1990 MW |
    /// | Bed temperature | 815.53 K |
    /// | Core outlet | 830.15 K (557.00 degC) |
    /// | Core inlet | 531.54 K (258.39 degC) |
    /// | SG duty | 6.6493 MW |
    /// | Steam outlet | 699.99 K (426.84 degC) |
    /// | Turbine power / shaft | 1.9964 MW, 2294.2 rpm |
    /// | **Worst node cross over the run** | **0.000000 K** |
    /// | Peak tube metal | 847.75 K (SteelSS304LHighTemp limit 1700 K) |
    ///
    /// These are **not** the published operating point and are not meant to be:
    /// the rods sit at the critical insertion with no operator action, so over
    /// 150 s the plant's own negative temperature feedback walks the power down
    /// from 10 MWth. The quantities being asserted are the second-law and
    /// property-range ones, which hold throughout.
    ///
    /// **A note on the rod position, because it is not a free choice.** Stepping
    /// this plant at a 50% bank insertion instead -- 10% withdrawn from critical
    /// -- is a **prompt excursion**: measured 2026-08-12, reactor power reaches
    /// 1073 MW within 1 s, the bed reaches 2261 K, the core outlet 2355 K, and
    /// the run dies at 6 s when the steam generator's tube metal passes
    /// `SteelSS304L`'s 1000 K ceiling and TUAS panics. The reactor protection
    /// system would terminate that at its 750 degC core-outlet trip, but it is
    /// **disarmed by default** in this simulator. That is pre-existing behaviour
    /// and not a consequence of the steam-generator rework -- an excursion
    /// previously died in the IF97 flash at 1073 K instead -- but the metal
    /// ceiling now arrives first, so it is recorded here.
    ///
    /// # Interpretation
    ///
    /// This is an integration smoke test with second-law teeth, not a validation
    /// of the plant. It says the GUI's physics thread survives its own opening
    /// state and that the exchanger behaves inside the full coupled loop, where
    /// the pebble bed's 184 s graphite lag and the protection system are also in
    /// the path.
    #[test]
    fn the_whole_plant_steps_without_crossing_or_leaving_property_range() {
        let mut plant = HtgrPlant::new();
        let mut snapshot = HtgrSnapshot::default();
        let dt = plant_timestep();
        let steps = (150.0 / PLANT_TIMESTEP_S).round() as usize;
        let flow = nominal_helium_flow();

        let mut worst_cross = 0.0_f64;
        let mut worst_metal_k = 0.0_f64;

        for i in 0..steps {
            plant.step(dt, HTGR_GUI_INITIAL_ROD_INSERTION, flow);
            plant.write_snapshot(&mut snapshot);

            let sg = plant.primary.steam_generator_state();
            worst_cross = worst_cross.max(sg.worst_node_cross_kelvin());
            for t in sg.metal_node_temperatures.iter() {
                worst_metal_k = worst_metal_k.max(t.get::<kelvin>());
            }
            assert!(
                sg.worst_node_cross_kelvin() <= 1e-6,
                "temperature cross of {} K in the steam generator at plant step {i}",
                sg.worst_node_cross_kelvin()
            );
            for (name, v) in [
                ("reactor_power_mw", snapshot.reactor_power_mw),
                ("core_inlet_temp_k", snapshot.core_inlet_temp_k),
                ("core_outlet_temp_k", snapshot.core_outlet_temp_k),
                ("ihx_duty_mw", snapshot.ihx_duty_mw),
                ("ihx_outlet_temp_k", snapshot.ihx_outlet_temp_k),
                ("sg_steam_outlet_temp_k", snapshot.sg_steam_outlet_temp_k),
                ("turbine_power_mw", snapshot.turbine_power_mw),
                (
                    "secondary_mass_flow_kg_per_s",
                    snapshot.secondary_mass_flow_kg_per_s,
                ),
                ("bed_temperature_k", snapshot.bed_temperature_k),
                ("shaft_speed_rpm", snapshot.shaft_speed_rpm),
            ] {
                assert!(
                    v.is_finite(),
                    "snapshot field {name} went non-finite at step {i}"
                );
            }
        }

        println!(
            "WHOLE-PLANT RUN (150 s at {PLANT_TIMESTEP_S} s, rods at the 0.6035 critical \
             insertion, 4.3 kg/s):\n  \
             reactor power   = {:.4} MW\n  \
             bed temperature = {:.2} K\n  \
             core outlet     = {:.2} K ({:.2} degC)\n  \
             core inlet      = {:.2} K ({:.2} degC)\n  \
             SG duty         = {:.4} MW\n  \
             steam outlet    = {:.2} K ({:.2} degC)\n  \
             turbine power   = {:.4} MW, shaft {:.1} rpm\n  \
             worst node cross over the run    = {worst_cross:.6} K\n  \
             peak tube-metal temperature      = {worst_metal_k:.2} K \
             (SteelSS304LHighTemp limit 1700 K)\n  \
             hot-array enthalpy clamp events  = {} over {steps} plant steps",
            snapshot.reactor_power_mw,
            snapshot.bed_temperature_k,
            snapshot.core_outlet_temp_k,
            snapshot.core_outlet_temp_k - 273.15,
            snapshot.core_inlet_temp_k,
            snapshot.core_inlet_temp_k - 273.15,
            snapshot.ihx_duty_mw,
            snapshot.sg_steam_outlet_temp_k,
            snapshot.sg_steam_outlet_temp_k - 273.15,
            snapshot.turbine_power_mw,
            snapshot.shaft_speed_rpm,
            plant.primary.steam_generator_enthalpy_clamp_events(),
        );

        assert_eq!(
            plant.primary.steam_generator_enthalpy_clamp_events(),
            0,
            "the exchanger's hot array clamped its enthalpy field. A nonzero count here means \
             the run is being HELD TOGETHER by the clamp rather than being stable: the \
             enthalpy field is leaving the range spanned by its own boundary and initial \
             data, which is the fingerprint of a Courant breakdown. Do not relax this."
        );

        assert!(
            worst_cross <= 1e-6,
            "worst cross over the run {worst_cross} K"
        );
        assert!(
            worst_metal_k < 1700.0,
            "tube metal reached {worst_metal_k} K, outside SteelSS304LHighTemp's range"
        );
        assert!(
            plant.sim_time.get::<second>() > 149.0,
            "the plant clock did not advance"
        );
    }
}
