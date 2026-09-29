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
//! Every subsystem here is **one control volume**, apart from the steam
//! generator. Nothing else in this plant model is spatially discretised:
//!
//! | Subsystem | Nodes | Consequence |
//! |---|---|---|
//! | Pebble bed | **1** volume, **2** temperatures (solid + helium, LTNE) | no axial or radial temperature profile, no peak fuel temperature |
//! | Helium circuit | **1** (two boundary temperatures) | no gradient through the bed, no natural circulation |
//! | Steam generator | **8 x 3** (helium / tube metal / water, counter-flow) | resolved zones and a real metal lag; 8 nodes is coarse |
//! | Secondary water/steam, outside the SG | **1** | fixed steam pressure, no drum or inventory dynamics |
//! | Neutronics | **1** (point kinetics) | no spatial flux shape, no rod-position-dependent worth |
//! | Reflector, barrel, cavity | **2** (reflector, RPV) | ~~the HTR-10 passive decay-heat path is absent entirely~~ **CORRECTED 2026-09-17** -- a lumped bed -> reflector -> RPV -> RCCS chain now exists ([`decay_heat_removal`]) and is stepped every plant step; barrel, carbon brick and cavity are folded into those two nodes, and the `UA` values are placeholders |
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
//! ## What the operator can command
//!
//! Everything an operator sets is carried in one value, [`PlantCommands`], and
//! nothing else reaches [`HtgrPlant::step`]. The primary side has the ganged
//! control-rod bank and the helium circulator; the secondary side has the
//! feedwater station -- **AUTO** on an adjustable steam-temperature setpoint or
//! **MANUAL** on a directly commanded flow -- and the condenser back-pressure.
//! Every field is clamped **inside the physics** to a stated, physically argued
//! range, so a slider, a test and a future OPC-UA write are held to the same
//! envelope. `tests::no_corner_of_the_command_envelope_crosses_or_clamps`
//! measures that no corner of it can drive a temperature cross or a Courant
//! breakdown.
//!
//! There is **no turbine load control**, because there is nothing to command:
//! the machine is islanded onto a fixed resistive load with no governor and no
//! throttle valve (see [`turbine_generator`]). Feedwater flow is what moves the
//! load in this plant.
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
pub mod atmospheric_dispersion;
pub mod decay_heat_removal;
pub mod dose_rate;
pub mod fission_product_release;
pub mod kinetics;
pub mod map_puff_model;
pub mod primary_loop;
pub mod protection;
pub mod reactor_model;
pub mod secondary_loop;
pub mod steam_generator;
/// Remedies for a steam-generator temperature cross -- see the module docs for
/// why they exist and why the default does nothing.
pub mod temperature_cross;
pub mod turbine_generator;

/// The former `physics::pebble_bed` module, migrated 2026-08-16 into
/// [`reactor_model::one_node`] -- now the geometry/correlation home for
/// every [`reactor_model::ReactorModelKind`] fidelity tier, and the site of
/// the real [`reactor_model::one_node::PebbleBedPorousMediaNode`] thermal
/// solve. Re-exported under its historical name because `kinetics`,
/// `primary_loop` and `secondary_loop` all read bed geometry and correlations
/// from it (`design()`, `bed_heat_capacity()`, `superficial_area()`, ...)
/// independent of which [`reactor_model::ReactorModelKind`] the plant's core
/// is currently running -- see [`reactor_model`]'s module doc comment for why
/// that geometry stays put rather than being duplicated per tier.
pub use reactor_model::one_node as pebble_bed;

use outram_park_digital_twin_engine::animation::residence_time_from_flow;
use outram_park_digital_twin_engine::app_scaffold::mark_component;
use uom::si::f64::{MassRate, Power, ThermodynamicTemperature, Time};
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::{megawatt, watt};
use uom::si::pressure::kilopascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

use crate::app::state::HtgrSnapshot;
use kinetics::{power_in_megawatts, HtgrKinetics};
use primary_loop::HeliumPrimaryLoop;
use protection::ReactorProtectionSystem;
use reactor_model::{ReactorModel, ReactorModelKind};
use secondary_loop::{SecondaryCommands, SteamSecondaryLoop};
use turbine_generator::TurbineGeneratorShaft;

/// **Control-rod bank insertion the simulator opens at**, ~~0.6035~~
/// ~~0.780927~~ ~~0.50~~ ~~0.40~~ ~~0.30~~ **0.45** (fraction, dimensionless).
///
/// This constant has had five values and the prose below had drifted behind
/// all of them, so the struck-through history is kept deliberately: a reader
/// who finds an old number quoted elsewhere needs to be able to date it.
///
/// # 0.40, set by the maintainer 2026-09-27 — and 0.30 BROKE THE PLANT
///
/// **0.30 was tried first, on the same day, and had to be backed off.** It made
/// the headless run *panic*: `tampines-steam-tables`' `ph_flash` raised
/// `p,h point below 273.15K` between plant step 200 and 500 (20–50 s of plant
/// time), while the same run at the previous 0.50 completed cleanly. The
/// opening excess at 0.30 is **+12.97 $**, and with the reactor protection
/// system disarmed by default the excursion drove the secondary side out of
/// IF97's validity range inside a minute.
///
/// That is the failure mode the reactivity table below predicts, arriving
/// exactly where it was predicted to: *"the opening state is ~13x prompt
/// critical"*. It is recorded rather than quietly skipped over, because it is
/// the evidence that the flag was not theoretical.
///
/// # ~~0.30, set by the maintainer 2026-09-27~~ — superseded the same day
///
/// **Maintainer direction, 2026-09-27: a hard-set constant, 0.30.** This is
/// the same kind of decision 0.50 was — an opening condition chosen for what
/// it lets the simulator demonstrate, not a value derived from a power target
/// or a criticality search. It continues in the direction 0.50 established:
/// *shallower*, so more bank travel is left in reserve.
///
/// **Measured consequence, 2026-09-27** (by
/// `tests::the_opening_rod_position_commands_a_known_reactivity`, at
/// `beta = 0.00650`):
///
/// | Insertion | External reactivity | | Relative to cold-clean critical |
/// |---|---|---|---|
/// | 0.604534 | 0 $ | 0 pcm | critical (the bisection's answer) |
/// | ~~0.6035~~ | ~~+0.0435 $~~ | ~~+28 pcm~~ | superseded |
/// | ~~0.50~~ | ~~+4.7294 $~~ | ~~+3074 pcm~~ | superseded |
/// | **0.30** | **+12.9676 $** | **+8428.9 pcm** | **~13x prompt critical** |
/// | 1.00 (full) | -6.9937 $ | -4545.9 pcm | shutdown authority retained |
///
/// The plant therefore opens far into the supercritical-cold-clean regime and
/// is held down by temperature feedback, not by the bank.
///
/// # THE OPENING STATE IS ~13x PROMPT CRITICAL -- flagged, not fixed
///
/// **Prompt critical is `+1 $` by definition.** At 0.30 the bank commands
/// `+12.97 $`, so the cold-clean opening condition is a **prompt excursion**
/// unless the negative temperature feedback is both very large and effectively
/// immediate. The 0.6035 constant's original doc said *"withdrawing the bank
/// by even ten percent from here is a prompt excursion"* -- this is
/// **thirty** percent shallower than that, and three hundred times its excess.
///
/// This was raised with the maintainer on 2026-09-27 and **0.30 was
/// reaffirmed as a hard-set constant.** It is recorded here rather than
/// silently adjusted, because a reader who finds a violent opening transient
/// needs to meet this number at the constant that causes it. Full insertion
/// still commands `-6.99 $`, so the bank can shut the core down -- the
/// shutdown demonstration 0.30 was chosen for does work.
///
/// **What is NOT established:** that the opening transient is bounded, or that
/// the plant reaches a steady state at all from here. The settled power at
/// 0.30 is **not measured** and is not 3 MW; see the note on 0.50 below, which
/// applies with much more force here. Re-measure before quoting any V&V
/// number recorded against the opening state.
///
/// # Where it sits relative to critical -- and the SIGN has changed
///
/// ~~"Very nearly the bank position at which this core is critical with no
/// external reactivity. Withdrawing the bank by even ten percent from here is
/// a prompt excursion."~~ **CHANGED 2026-09-17**, so the simulator opened
/// where the 15 Oct 2003 loss-of-forced-cooling ATWS test began rather than at
/// rated power.
///
/// ~~"Cold-clean critical is 0.604535; this sits far deeper, commanding
/// -5.5186 $ of external reactivity. The reactor is held at 3 MWth by the
/// bank, not by feedback, and the plant is *sub*critical in the cold-clean
/// sense."~~ **CORRECTED 2026-09-22 -- that is now backwards.** Cold-clean
/// critical is **0.604535** and the bank now opens at **0.50**, which is
/// *shallower*. The plant therefore starts **super**critical in the
/// cold-clean sense and is held down by feedback rather than by the bank --
/// the opposite of what the struck text describes, and the reason the opening
/// transient is a rise rather than a hold.
///
/// ~~**Why 0.50:** set by the maintainer so enough bank is left to insert that
/// a shutdown on an ATWS following a DLOFC or LOFC can actually be
/// demonstrated. Holding a power target was the *previous* criterion; it is
/// not this one. Confirmed 2026-09-22 that 0.50, not 0.55, is intended.~~
/// **SUPERSEDED 2026-09-27 by 0.30** -- the criterion is unchanged (bank in
/// reserve for an ATWS shutdown demonstration), only the value moved.
///
/// The **-5.5186 $** figure above belonged to 0.780927 and is not re-derived
/// here; see the struck table below for what it was and how it was found.
///
/// # ~~Measured, not chosen~~ CHOSEN, as of 2026-09-22 -- and that is the
/// # maintainer's call, not a defect
///
/// **CORRECTED 2026-09-22, again 2026-09-27.** The table below measured
/// **0.780927**. The constant is now **0.30**, set by the maintainer so the
/// simulator opens with enough bank withdrawn to demonstrate shutdown on an
/// ATWS after a DLOFC or LOFC. (On 2026-09-22 it was 0.50, confirmed intended
/// then; the commit that introduced it carried a note reading "i want 0.55"
/// and 0.50 is what was meant. On 2026-09-27 the maintainer set **0.30**,
/// asking for it as a hard-set constant.)
///
/// So the heading above is now wrong as written, and the table is kept
/// **struck through** rather than deleted because it is the provenance of the
/// number this replaced, and because it records the method that would have to
/// be re-run to re-derive a power target:
///
/// | Quantity | ~~Value~~ SUPERSEDED |
/// |---|---|
/// | ~~insertion~~ | ~~**0.780927**~~ |
/// | ~~settled power~~ | ~~**3.0428 MW** (target 3.0000)~~ |
/// | ~~external reactivity~~ | ~~**-5.5186 $**~~ |
/// | ~~helium flow~~ | ~~**1.290 kg/s** (30 % of rated)~~ -- the bisection above belongs to THIS flow |
///
/// **And the flow it was bisected at is no longer the opening flow.**
/// [`GUI_INITIAL_HELIUM_FLOW_KG_PER_S`] became `1.00` on 2026-09-27, so the
/// plant opens at **4.300 kg/s**. Every number in the struck table, including
/// the 3.0428 MW, belongs to 1.290 kg/s and **does not transfer to rated
/// flow**. Re-running the bisection is a ~34-minute job and was deliberately
/// not done here.
///
/// **The settled power at 0.30 is NOT 3 MW, and is not yet measured here.**
/// Less insertion is more reactivity, so it settles higher; by how much
/// depends on feedback and takes a long settle to find, which is why the
/// commit introducing it said "awaiting steady state (it takes very long)".
/// Marked **not re-checked** deliberately rather than left reading as though
/// the 3.0428 MW above still applied: a stale number that looks measured is
/// worse than an absent one.
///
/// Re-derive with `tests::report_the_rod_position_that_holds_three_megawatts`
/// (about 34 minutes -- it settles the plant 15 times) if a power target is
/// wanted again. Note that test bisects *for* 3 MW; it does not report the
/// power at a *given* insertion, which is the question 0.30 raises.
///
/// # The dollar figures here are ambiguous, and that is a real defect
///
/// The bisection reported `beta_eff = 0.006500`, because
/// `HtgrKinetics::delayed_neutron_fraction()` reads the **delayed** layer,
/// which is `DelayedNeutronLayer::u235_five_group()` at bare U-235's 0.0065 --
/// while the **prompt** layer uses HTR-10's published
/// [`kinetics::HtgrKinetics::HTR10_EFFECTIVE_DELAYED_FRACTION`] of 7.26e-3.
/// One physical quantity, two values, 11.7 % apart. Every dollar figure above
/// depends on which one is used; the pcm figures do not. Not fixed here.
///
pub const GUI_INITIAL_ROD_INSERTION: f64 = 0.45;

/// Fraction of rated helium flow the simulator opens at:
/// ~~0.30~~ **1.00** **CHANGED 2026-09-27**.
///
/// **Maintainer direction, 2026-09-27: the circulator default is the full
/// published 4.3 kg/s.** Because this constant is a *fraction* (see below),
/// that is expressed as **1.00**, not as 4.3.
///
/// **The name lies about the units and the call site cannot tell.** This is a
/// dimensionless *fraction*, not kg/s: it is consumed as
/// `GUI_INITIAL_HELIUM_FLOW_KG_PER_S * nominal_helium_flow()`, and
/// [`nominal_helium_flow`] is the published 4.3 kg/s. At the old 0.30 the
/// plant opened at **1.290 kg/s**, not 0.30 kg/s; at **1.00** it opens at the
/// full **4.300 kg/s**. Left named as-is rather than renamed in a change about
/// something else; read the multiplication, not the suffix.
///
/// # What the opening state is FOR has changed, and the pairing is no longer the binding constraint
///
/// **Maintainer, 2026-09-27, verbatim:** *"For the htgr_sim_v1, I indeed
/// wanted it to match the test. But for demo purposes, I just want to show
/// shutdown after LOFC. That is sufficient. The validation run can come
/// later."*
///
/// So the near-term acceptance criterion for this simulator is
/// **demonstrating that the reactor shuts itself down after a loss of forced
/// cooling.** Reproducing the published *initial condition* of the 15 October
/// 2003 HTR-10 LOFC/ATWS test is a **later validation goal, explicitly
/// deferred by the maintainer.**
///
/// ~~The HTR-10 loss-of-forced-cooling test began from part load, and
/// [`GUI_INITIAL_ROD_INSERTION`] was bisected against settled power AT this
/// flow. The two are a matched pair and must be changed together: at rated
/// flow the same bank position settles at a different power entirely, because
/// the bed temperature -- and therefore the feedback the bank is offsetting --
/// is different.~~ **SUPERSEDED 2026-09-27** -- not because it was wrong, but
/// because it was the right choice for a goal that is no longer the binding
/// one. The 0.30/1.29 kg/s pair was correct *for the validation run*; the
/// opening state no longer has to defend itself as the published test
/// condition, it has to defend that the demonstration works.
///
/// # Limitation, stated as a limitation
///
/// - The opening state is a **rated-flow demonstration starting point, not the
///   published LOFC initial condition.** Do not quote it as the latter.
/// - A validation run reproducing the 2003 test **will need the part-load pair
///   restored** -- this constant back to `0.30`, and
///   [`GUI_INITIAL_ROD_INSERTION`] re-bisected at that flow.
/// - [`GUI_INITIAL_ROD_INSERTION`] was bisected at the *old* 30 % flow, so it
///   **no longer holds any particular power**; it was also independently moved
///   to 0.30, a deliberately shallow, super-prompt opening state (maintainer's
///   call, gh:#318). The settled power at the opening state is therefore
///   unmeasured on both counts.
/// - The direction of this change is the one the demonstration wants: more
///   helium mass flow removes more heat from the bed for a given power, so the
///   core is better able to reject the heat the shallow bank admits.
///
/// Both values are the maintainer's call. Do not "restore the pair" on your
/// own initiative -- restore it when the validation run is the task.
pub const GUI_INITIAL_HELIUM_FLOW_KG_PER_S: f64 = 1.00;

/// **Every operator input the plant accepts, in one value.**
///
/// [`HtgrPlant::step`] takes this instead of a growing list of positional
/// arguments, so a reader can see the whole command surface in one place and
/// adding the next control does not change the signature again.
///
/// | Field | Commands | Range, and where the limit comes from |
/// |---|---|---|
/// | [`Self::control_rod_insertion_fraction`] | the ganged rod bank | `0..=1`, the mechanical stops ([`control_rods`]) |
/// | [`Self::helium_flow_setpoint`] | the primary circulator | 0.3 to 8.0 kg/s, the invented circulator capacity ([`primary_loop`]) |
/// | [`secondary.feedwater`](secondary_loop::SecondaryCommands::feedwater) | the feedwater station, AUTO or MANUAL | 260 to 540 degC setpoint / 0.3 to 12.0 kg/s flow ([`secondary_loop`]) |
/// | [`secondary.condenser_pressure`](secondary_loop::SecondaryCommands::condenser_pressure) | the condenser back-pressure | 4 to 30 kPa, bounded below by the cooling-water temperature ([`secondary_loop`]) |
///
/// **Every field is clamped inside the physics**, not by whatever produced it.
/// A slider, a test and a future OPC-UA write are all held to the same envelope,
/// which is the property that stops a control input from being able to drive
/// the model out of its correlations' validity ranges.
///
/// # What is deliberately not here
///
/// - **Reactivity.** The operator moves rods; reactivity is the consequence,
///   published back for display. See
///   [`HtgrPlant::external_reactivity_dollars`].
/// - **The protection system's arming and trip reset.** Those are latched
///   *state* owned by the physics thread rather than per-step demands, so they
///   are set through [`ReactorProtectionSystem`] directly -- see
///   [`crate::app`]'s physics loop.
/// - **Turbine load.** There is no governor and no throttle valve in this
///   plant, so there is nothing to command. See
///   [`turbine_generator`] for why, and
///   [`secondary_loop::FeedwaterCommand::Manual`] for what does the job
///   instead.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlantCommands {
    /// Control-rod bank insertion fraction, `0.0` fully withdrawn to `1.0`
    /// fully inserted (dimensionless).
    ///
    /// The ten HTR-10 side-reflector rods are ganged as one bank. The
    /// protection system can only ever drive this **deeper**, never lift it.
    pub control_rod_insertion_fraction: f64,
    /// Helium circulator mass-flow setpoint, in kg/s (`uom`-typed).
    ///
    /// Clamped by [`primary_loop`] to the circulator's illustrative
    /// 0.3 to 8.0 kg/s capacity, which brackets the published 4.3 kg/s
    /// operating point.
    pub helium_flow_setpoint: MassRate,
    /// Everything the operator commands on the **secondary** side: the
    /// feedwater mode and demand, and the condenser back-pressure.
    pub secondary: SecondaryCommands,
    /// Which accident scenario, if any, the plant is running.
    pub scenario: Scenario,
    /// Wind driving the atmospheric dispersion channel.
    ///
    /// A **command**, not plant state: the wind is weather, so the operator
    /// dials it in exactly as they would read it off a met mast. It reaches
    /// the plant the same way every other command does rather than being
    /// poked into the dispersion channel directly, so a headless run and the
    /// GUI drive it identically.
    pub meteorology: atmospheric_dispersion::Meteorology,
    /// What the Map tab wants of the dispersion **field**: a resolution (one
    /// cell per screen pixel) and a plume-clock offset (the fast-forward).
    ///
    /// A command, for the same reason the meteorology is one: it reaches the
    /// plant through the single one-way path every other operator input takes,
    /// so the GUI and a headless run drive the field identically rather than
    /// the GUI reaching into the channel. See
    /// [`atmospheric_dispersion::MapFieldRequest`], especially on why the
    /// plume clock may run ahead of the plant clock and what that must never
    /// be read as.
    pub map_field: atmospheric_dispersion::MapFieldRequest,
}

/// The accident scenario the plant is running.
///
/// **An enum rather than a pair of booleans, deliberately.** A circulator trip
/// and a secondary isolation are not independent switches an operator flips in
/// any combination -- they are stages of one event, and representing them
/// separately makes states expressible that the plant cannot be in (secondary
/// isolated while the blower runs at rated flow, say). Matching exhaustively
/// also means adding a scenario later is a compile error at every site that
/// must handle it, which is the whole point of this workspace's
/// enum-dispatch rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scenario {
    /// Normal operation. The circulator follows its commanded setpoint and the
    /// secondary circuit is connected.
    #[default]
    Normal,
    /// **Loss of forced cooling.** The helium circulator has tripped; the
    /// primary stays pressurised. The protection system isolates the secondary
    /// circuit [`SECONDARY_ISOLATION_DELAY_S`] after the trip, as it did in the
    /// HTR-10 test of 15 October 2003 (Hu et al. 2006 section 3).
    ///
    /// **Whether the rods move is NOT part of this scenario.** Leave the bank
    /// where it is and you have the ATWS the safety demonstration tested;
    /// arm the protection system and you have LOFC with scram. Keeping them
    /// separate is the point -- folding a scram into the scenario would make
    /// the inherent-shutdown case unrepresentable.
    Lofc,
}

/// Delay from a circulator trip to the protection system isolating the
/// secondary circuit \[s\].
///
/// **12 s, measured.** In the HTR-10 loss-of-forced-cooling ATWS test the
/// reactor protection system isolated the secondary circuit and closed the
/// blower baffle 12 s after initiation (Hu et al. 2006 section 3), after which
/// the primary flow was "almost zero".
///
/// This is not cosmetic: leaving the secondary connected during a LOFC keeps
/// feedwater pulling heat through a steam generator that has no helium-side
/// source, which drives the tube metal below the SS304L correlation's lower
/// bound. See [`primary_loop::HeliumPrimaryLoop::advance_steam_generator`].
pub const SECONDARY_ISOLATION_DELAY_S: f64 = 12.0;

/// Lowest helium flow the circulator can actually regulate at \[kg/s\].
///
/// The published blower turns down to about 30 %; below that it cannot hold a
/// setpoint, so a commanded flow under this floor is read as a STOPPED
/// machine rather than as a very small commanded flow. Mirrors
/// `primary_loop`'s own commanded-flow floor.
pub const CIRCULATOR_REGULATING_FLOOR_KG_PER_S: f64 = 0.3;

impl Default for PlantCommands {
    /// The published HTR-10 operating point, with the feedwater station in AUTO
    /// at the published 440 degC steam temperature and the condenser at its
    /// design 7 kPa.
    ///
    /// The rod position is [`GUI_INITIAL_ROD_INSERTION`] and the flow is
    /// [`GUI_INITIAL_HELIUM_FLOW_KG_PER_S`] of rated.
    ///
    /// ~~The flow is 30 % of rated, the pair that holds the plant at the
    /// HTR-10 test's **3 MWth** initial condition. They are a PAIR: the
    /// insertion was bisected at that flow, so changing one without the other
    /// lands somewhere else.~~ **CORRECTED 2026-09-27**: the flow is now
    /// **100 % of rated (4.3 kg/s)** by maintainer direction and the insertion
    /// is 0.30 by maintainer direction (gh:#318). This opening state is a
    /// **demonstration starting point for LOFC self-shutdown**, not the HTR-10
    /// 3 MWth initial condition; the published initial condition is a deferred
    /// validation goal and the settled power here is unmeasured.
    fn default() -> Self {
        Self {
            control_rod_insertion_fraction: GUI_INITIAL_ROD_INSERTION,
            // A FRACTION of the published 4.3 kg/s, not kg/s: 1.00 since
            // 2026-09-27 (maintainer direction), i.e. full rated flow.
            helium_flow_setpoint: GUI_INITIAL_HELIUM_FLOW_KG_PER_S * nominal_helium_flow(),
            secondary: SecondaryCommands::default(),
            // The MAP's one puff-model configuration (maintainer direction
            // 2026-09-29; `map_puff_model`, gh:#384), so the plant's default
            // command and the GUI's opening command are the same meteorology.
            // `Meteorology::default()` stays the channel's own default for
            // its unit tests.
            meteorology: map_puff_model::map_meteorology(
                map_puff_model::default_speed_m_per_s(),
                0.0,
            ),
            map_field: atmospheric_dispersion::MapFieldRequest::default(),
            scenario: Scenario::Normal,
        }
    }
}

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
/// Each corrector rewinds the cheap lumped states -- [`ReactorModel`],
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
/// # Why 2 -- measured, then contradicted. Read this before quoting the table.
///
/// **The "converged at 2" claim below does not currently hold, and nothing
/// gates it (`op-21rt`, P1).** Re-measured 2026-08-14 on the same 60 s
/// transient, the third corrector moved the steam outlet by **+29.29 K** and
/// the duty by **-0.271%** -- against the `-0.0007 K` in the table. The sweep
/// that would have caught this is **printed, never asserted**: the test
/// [`tests::the_plant_outer_correctors_converge`] runs and passes, because its
/// two legs share this same corrector count and the error cancels between them.
/// So there is **no running check on corrector convergence in this workspace** —
/// not a weak one, none.
///
/// The 2 is therefore **the shipped value, not a verified one**. It is left at
/// 2 deliberately: nothing shows 3 is converged either (the sweep stops there),
/// and a 29 K jump from one extra Picard sweep while the duty barely moves
/// looks more like divergence or bistability on the steam side than
/// under-convergence. Raising it blindly would trade a known-unverified number
/// for an unknown-unverified one.
///
/// [`HtgrPlant::step_with_correctors`] exists so this can be swept; the sweep
/// lives in [`tests::the_plant_outer_correctors_converge`]. Measured
/// 2026-08-13 -- **historical, superseded by the above** -- at the end of that
/// test's 60 s flow-ramp transient:
///
/// | Correctors | Core outlet | Steam outlet | SG duty | Moved from the previous count by |
/// |---|---|---|---|---|
/// | 1 (plain Lie split) | 949.6218 K | 575.7429 K | 7.65765 MW | -- |
/// | **2 (shipped)** | **949.6679 K** | **575.5177 K** | **7.65733 MW** | dT_out +0.046 K, dT_steam -0.225 K, dQ -0.004% |
/// | 3 | 949.6666 K | 575.5170 K | 7.65732 MW | dT_out **-0.0013 K**, dT_steam **-0.0007 K**, dQ -0.0002% |
/// | *1 ms reference* | *949.669 K* | *574.696 K* | *7.65827 MW* | -- |
///
/// **The 1-corrector row is history, not a live reading.** That arm was removed
/// from the sweep on 2026-08-14 (maintainer's instruction): convergence is
/// established by the *last* step being small, so only 3-against-2 carries the
/// claim, and the extra 60 s plant-time run was a fifth of a slow test for a
/// number nothing asserted on. The row is kept because it is what shows the
/// second corrector does real work; re-derive it with
/// [`HtgrPlant::step_with_correctors`] if it is ever needed again.
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

/// The plant's **global energy ledger** \[J\] (gh:#394, 2026-09-29): every
/// term from each subsystem's own final-corrector bookkeeping, so the balance
/// from fission to the steam generator and the RCCS can be checked rather
/// than assumed.
///
/// ```text
/// residual = source + circulator_work
///          - (fuel + bed_solid + bed_helium + hot_duct + cold_return + passive) storage
///          - to_steam_generator - to_rccs
/// ```
///
/// **Boundary.** The fuel node, the bed's graphite and void helium, the
/// primary loop's hot-duct and cold-return CVs and the passive path's
/// reflector and RPV are inside; the steam generator is outside, and the
/// boundary on that side is the helium stream, `m_dot (h_hot duct - h_SG,out)`
/// -- the exchanger's hot-side duty. The exchanger's own three arrays are
/// coupled explicitly (Lie split), so its internal closure is `O(dt)` rather
/// than exact (+0.34 % at the design point; see
/// `primary_loop::steam_generator_substep_s`); putting it inside would make
/// this ledger report that known closure instead of the seams it exists to
/// check.
///
/// Every internal seam -- fuel to bed, bed to passive path, bed to hot duct,
/// hot duct to steam generator, steam generator to cold return, cold return
/// to bed -- carries one flux used identically on both sides, so the residual
/// is a statement about the seams and is expected at rounding level.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PlantEnergyLedger {
    /// Fission (prompt) and decay heat deposited in the fuel node.
    pub source: f64,
    /// Fuel-node stored-energy change.
    pub fuel_storage: f64,
    /// Bed graphite enthalpy change.
    pub bed_solid_storage: f64,
    /// Bed void-helium enthalpy change.
    pub bed_helium_storage: f64,
    /// Hot-duct CV enthalpy change.
    pub hot_duct_storage: f64,
    /// Cold-return CV enthalpy change.
    pub cold_return_storage: f64,
    /// Reflector + RPV stored-energy change.
    pub passive_storage: f64,
    /// Enthalpy handed from the helium to the steam generator.
    pub to_steam_generator: f64,
    /// Heat to the RCCS.
    pub to_rccs: f64,
    /// Circulator work delivered to the helium.
    pub circulator_work: f64,
    /// `source + circulator_work - storage - to_steam_generator - to_rccs`.
    pub residual: f64,
}

impl PlantEnergyLedger {
    /// Sum of every storage term.
    pub fn stored(&self) -> f64 {
        self.fuel_storage
            + self.bed_solid_storage
            + self.bed_helium_storage
            + self.hot_duct_storage
            + self.cold_return_storage
            + self.passive_storage
    }

    fn accumulate(&mut self, step: &PlantEnergyLedger) {
        self.source += step.source;
        self.fuel_storage += step.fuel_storage;
        self.bed_solid_storage += step.bed_solid_storage;
        self.bed_helium_storage += step.bed_helium_storage;
        self.hot_duct_storage += step.hot_duct_storage;
        self.cold_return_storage += step.cold_return_storage;
        self.passive_storage += step.passive_storage;
        self.to_steam_generator += step.to_steam_generator;
        self.to_rccs += step.to_rccs;
        self.circulator_work += step.circulator_work;
        self.residual += step.residual;
    }
}

/// The full plant model: kinetics + pebble-bed core + helium primary loop +
/// steam secondary loop, plus the running simulation clock.
pub struct HtgrPlant {
    /// Whether a **Map tab is on screen**, and so whether the dispersion
    /// field should be refreshed at map cadence (10 Hz) as well as at
    /// [`AtmosphericDispersionChannel::update`]'s 60 s physics throttle.
    ///
    /// **Off by default, and the GUI is what turns it on**
    /// ([`Self::with_live_map_field`]). Two independent reasons, either
    /// sufficient:
    ///
    /// 1. **Headless runs must not depend on the wall clock.** The 10 Hz
    ///    refresh is gated on `std::time::Instant`, so a headless run that
    ///    took it would produce a trace that depends on how fast the machine
    ///    is. `headless.rs` asserts `run(&cfg) == run(&cfg)`; this is the
    ///    field that would break it. A headless run has no map to draw, so it
    ///    loses nothing.
    /// 2. **A GUI run has a rendering adapter by construction.** `eframe`
    ///    does not start without one, so in the only mode that sets this flag
    ///    the field is affordable: measured 2026-09-25 at the live map's own
    ///    working point (120 puffs, 512 cells), **1.33 ms on the GPU and
    ///    19.6 ms pooled on 16 cores**. That is why this is a mode flag
    ///    rather than an adapter probe.
    ///
    /// ~~Even the CPU path would fit: 15.9 ms pooled at the simulator's
    /// 7 260-puff working point.~~ **RESTATED 2026-09-25.** That working
    /// point no longer exists -- the field is instantaneous now, so it sums
    /// 120 puffs rather than 7 260, and the cost is set by the map's
    /// resolution instead. The CPU path still fits, which is why
    /// [`atmospheric_dispersion::max_grid_cells`] rather than this flag is
    /// what adapts to it: an adapter probe picks the *resolution*, while this
    /// flag picks whether the 10 Hz refresh happens at all.
    ///
    /// The flag remains about determinism and about not doing work nobody is
    /// looking at, not about affordability.
    pub map_field_live: bool,
    /// Reactor kinetics slot (prompt excursion + delayed-neutron bank).
    pub kinetics: HtgrKinetics,
    /// Pebble-bed core -- the graphite thermal inertia between the fission
    /// power and the helium, at whichever fidelity tier is currently
    /// selected. See [`reactor_model`] for the tiers and
    /// [`Self::set_reactor_model_kind`] to switch between them.
    pub core: ReactorModel,
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
    /// Passive decay-heat path out to the RCCS. See [`decay_heat_removal`].
    pub decay_heat_path: decay_heat_removal::CoreToRccsPath,
    /// TRISO fission-product release. ~~"driven off the SAME resolved
    /// fuel-kernel temperature as the Doppler channel."~~ **CORRECTED
    /// 2026-09-28 (gh:#360)** -- only the kernel-above-node *offset* is shared.
    /// This channel is handed `peak_kernel_temperature()` (bed node + offset,
    /// solved at the start-of-step bed temperature, throttled to 1 s); the
    /// Doppler channel's implied kernel is the kinetics node `T_f` + `R P(t)`,
    /// and `T_f` was measured 235 K above the bed at t = 1500 s. Quasi-steady
    /// and stateless, so it sits outside the corrector loop -- see
    /// [`fission_product_release`].
    pub release: fission_product_release::TrisoAtopsReleaseChannel,
    /// Gaussian puff atmospheric dispersion, driven by the release channel's
    /// circulating pool. Quasi-steady like the release channel and far more
    /// expensive, so it is throttled harder and sits outside the corrector
    /// loop -- see [`atmospheric_dispersion`], whose binding scope limit
    /// (research/education/V&V only; ~~no dose quantity of any kind~~ -- since
    /// 2026-09-29 its output feeds the indicative dose rate in [`dose_rate`],
    /// never a dose to any real person) applies to everything it produces.
    pub dispersion: atmospheric_dispersion::AtmosphericDispersionChannel,
    /// Sim time at which the current scenario was first commanded, `None`
    /// under [`Scenario::Normal`]. Drives the protection system's
    /// secondary-isolation delay.
    scenario_started_at: Option<Time>,
    /// Heat rate crossing the pebble surface into the helium on the most recent
    /// step -- the core's *thermal* output, which lags the fission power by the
    /// graphite time constant.
    core_heat_to_helium: Power,
    /// Passive decay-heat loss from the bed to the reflector on the most
    /// recent step (final corrector).
    passive_heat_loss: Power,
    /// The global energy ledger of the most recent step. See
    /// [`PlantEnergyLedger`].
    last_step_energy: PlantEnergyLedger,
    /// The global energy ledger accumulated since construction.
    energy: PlantEnergyLedger,
}

impl HtgrPlant {
    /// Construct the plant at the published HTR-10 operating point.
    pub fn new() -> Self {
        let nominal_power = nominal_thermal_power();
        let core = ReactorModel::default(); // ReactorModelKind::OneNodePorousMedia as of 2026-08-17
        let primary = HeliumPrimaryLoop::new(nominal_helium_flow());
        // The passive path opens in equilibrium with the bed as it is seeded
        // -- the derived constructor (gh:#389; ~~`placeholder()`~~ deleted).
        // The bed seed itself is #387's business and unchanged here.
        // The bed seeds both of its phases at the same temperature
        // (`PebbleBedPorousMediaNode::new`), so that is the helium side too.
        let decay_heat_path = decay_heat_removal::CoreToRccsPath::new_at_steady_state(
            core.temperature(),
            core.temperature(),
            primary.core_inlet_temperature(),
            primary.mass_flow(),
        );
        Self {
            // Off by default: headless and every test get the deterministic,
            // no-wall-clock path. `with_live_map_field` is the GUI's opt-in.
            map_field_live: false,
            kinetics: HtgrKinetics::new_htr10_published(nominal_power),
            core,
            primary,
            secondary: SteamSecondaryLoop::new(),
            shaft: TurbineGeneratorShaft::new(),
            protection: ReactorProtectionSystem::new(),
            sim_time: Time::new::<second>(0.0),
            decay_heat_path,
            release: fission_product_release::TrisoAtopsReleaseChannel::new_htr10(),
            dispersion: atmospheric_dispersion::AtmosphericDispersionChannel::new(),
            scenario_started_at: None,
            core_heat_to_helium: Power::new::<watt>(0.0),
            passive_heat_loss: Power::new::<watt>(0.0),
            last_step_energy: PlantEnergyLedger::default(),
            energy: PlantEnergyLedger::default(),
        }
    }

    /// The global energy ledger of the most recent step \[J\]. See
    /// [`PlantEnergyLedger`].
    #[allow(dead_code)] // read by the conservation tests
    pub fn last_step_energy(&self) -> PlantEnergyLedger {
        self.last_step_energy
    }

    /// The global energy ledger accumulated since construction \[J\].
    pub fn energy_ledger(&self) -> PlantEnergyLedger {
        self.energy
    }

    /// The fuel stack temperatures -- kernel (the kinetics fuel node), SiC
    /// layer, fuelled-zone matrix, peak kernel -- placed on the line between
    /// the bed and the fuel node by the bed's most recent resolved-pebble
    /// coupling (gh:#360, 2026-09-28). `None` only if the bed has never had a
    /// coupling, which the design-point seed rules out.
    pub fn fuel_stack_temperatures(&self) -> Option<pebble_bed::FuelStackTemperatures> {
        self.core
            .fuel_bed_coupling()
            .map(|c| c.stack(self.core.temperature(), self.kinetics.fuel_temperature()))
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

    /// Turn on the 10 Hz map-field refresh — **the GUI's opt-in**, and the
    /// only thing that should ever call this.
    ///
    /// See [`Self::map_field_live`] for why the discriminator is "is a map on
    /// screen" rather than "is a GPU present": a run that reaches `eframe`
    /// has an adapter by construction, and a run that does not has no map to
    /// refresh and must stay independent of the wall clock.
    ///
    /// Deliberately a consuming builder rather than a setter: turning this on
    /// mid-run would change a plant's timing behaviour partway through a
    /// trace, which is exactly the kind of state change `headless.rs`'s
    /// determinism assertion exists to forbid.
    pub fn with_live_map_field(mut self) -> Self {
        self.map_field_live = true;
        self
    }

    /// The pebble-bed fidelity tier currently selected. See
    /// [`Self::set_reactor_model_kind`] to change it.
    #[allow(dead_code)] // read by a future control-panel dropdown; see reactor_model module docs
    pub fn reactor_model_kind(&self) -> ReactorModelKind {
        self.core.kind()
    }

    /// Switch the pebble-bed fidelity tier, rebuilding [`Self::core`] fresh at
    /// `kind`'s own cold-start seed (see [`reactor_model`]'s module doc
    /// comment for why a switch rebuilds rather than converts state in
    /// place). A no-op if `kind` is already selected, so a GUI `ComboBox`
    /// following the `HeaterType` write-on-change-only pattern can call this
    /// unconditionally without resetting the bed on every repaint.
    #[allow(dead_code)] // write side of a future control-panel dropdown; see reactor_model module docs
    pub fn set_reactor_model_kind(&mut self, kind: ReactorModelKind) {
        if self.core.kind() != kind {
            self.core = ReactorModel::new(kind);
        }
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

    /// Advance the whole plant by one timestep `dt` under the operator's
    /// [`PlantCommands`].
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
    pub fn step(&mut self, dt: Time, commands: PlantCommands) {
        self.step_with_correctors(dt, commands, PLANT_OUTER_CORRECTORS);
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
    pub fn step_with_correctors(&mut self, dt: Time, commands: PlantCommands, n_outer: usize) {
        let PlantCommands {
            control_rod_insertion_fraction,
            helium_flow_setpoint,
            secondary: secondary_commands,
            scenario,
            meteorology: _,
            // Both of these are consumed by the dispersion block at the end
            // of this function, which reads them off `commands` directly, so
            // the corrector loop below never sees them. Named rather than
            // `..` so a new command field is a compile error here and has to
            // be routed deliberately.
            map_field: _,
        } = commands;

        // Apply the scenario BEFORE the sim clock advances, so `scenario_time`
        // below is measured from the step on which the trip was first
        // commanded rather than one step late.
        let scenario_started_at = match (scenario, self.scenario_started_at) {
            (Scenario::Lofc, None) => {
                // First step of the trip. The circulator stops following its
                // setpoint; the secondary stays connected for now.
                self.primary.trip_circulator(true);
                Some(self.sim_time)
            }
            (Scenario::Lofc, Some(t0)) => Some(t0),
            (Scenario::Normal, _) => {
                // Scenario cleared: put the plant back to normal operation.
                self.primary.trip_circulator(false);
                self.primary.isolate_secondary(false);
                None
            }
        };
        self.scenario_started_at = scenario_started_at;

        // The protection system isolates the secondary a fixed delay after the
        // trip. This is the PLANT acting, not the operator, which is why it
        // lives here rather than in a scenario script.
        if let Some(t0) = scenario_started_at {
            let elapsed = (self.sim_time - t0).get::<second>();
            if elapsed >= SECONDARY_ISOLATION_DELAY_S {
                self.primary.isolate_secondary(true);
            }
        }

        // A tripped circulator does not follow the operator's setpoint.
        let helium_flow_setpoint = match scenario {
            Scenario::Normal => helium_flow_setpoint,
            Scenario::Lofc => MassRate::new::<kilogram_per_second>(0.0),
        };

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

        // Old-time snapshot of everything the corrector loop re-advances. All
        // of it is scalar, so this is cheap next to one exchanger substep.
        //
        // The kinetics is in here as of 2026-08-14. It used to be stepped ONCE
        // above the loop, and that was provably sufficient while its
        // reactivity feedback ran off the prompt layer's own adiabatic fuel
        // temperature -- nothing the loop iterated could reach it. Now the
        // feedback reads `pebble_bed`'s graphite temperature (see
        // `kinetics`), so the kinetics depends on a quantity the loop
        // corrects, and it MUST be rewound and re-advanced with everything
        // else. `HtgrKinetics` is `Clone` precisely so this is possible; the
        // module docs anticipated this exact change.
        let kinetics_at_step_start = self.kinetics.clone();
        let core_at_step_start = self.core;
        let primary_at_step_start = self.primary.lumped_state();
        let secondary_at_step_start = self.secondary.integrated_state();
        // The passive path's reflector and vessel nodes are integrated state
        // like the rest, so they are rewound per corrector too. Until
        // 2026-09-22 they were not: with two correctors they advanced twice
        // per step while the rewound bed lost their heat once, creating
        // energy in the chain. See
        // `tests::the_passive_path_conserves_energy_through_a_full_plant_step`.
        let decay_heat_path_at_step_start = self.decay_heat_path;
        // The bed temperature the passive path is solved against: the
        // start-of-step value on the first corrector, then the previous
        // corrector's end-of-step value, the same predictor-corrector
        // treatment as the helium couplings below.
        //
        // (Named for the passive path until 2026-09-29; since the path became
        // rows of the bed's own solve, only the kinetics' fuel-to-bed coupling
        // reads this predictor.)
        let mut bed_temperature_estimate = self.core.temperature();

        // The coupling variables. Initialised to their start-of-step values --
        // which is exactly what a single-corrector (plain Lie-split) step would
        // use throughout, so `PLANT_OUTER_CORRECTORS == 1` reproduces the
        // pre-2026-08-13 sequencing.
        // The bed's coupling variable is the core INLET temperature, not the
        // bulk mean: an arithmetic-mean driving temperature closed against a
        // downstream `T_in + Q/(m c_p)` is what produced a second-law
        // violation above NTU ~ 2 in this plant's very first pebble-bed
        // closure (see `reactor_model::one_node`'s "History" note) -- the
        // current default (`reactor_model::ReactorModelKind::OneNodePorousMedia`,
        // `pebble_bed::PebbleBedPorousMediaNode::step`) instead gives the
        // helium its own implicit thermal node against this same inlet
        // boundary condition, never an arithmetic mean.
        //
        // CHANGED 2026-09-29 (gh:#393): an ENTHALPY, the cold-return CV's
        // state. The same number is handed to the bed and, on the final
        // corrector, discharged by the cold-return CV -- the seam's single
        // flux (see `primary_loop::HeliumPrimaryLoop::close_return_leg`).
        let mut core_inlet_enthalpy = self.primary.core_inlet_enthalpy();
        // Start-of-step readings for the global energy ledger.
        let fuel_ledger_at_step_start = self.kinetics.ledger();
        let passive_stored_at_step_start = self.decay_heat_path.stored_energy();
        let mut feedwater_enthalpy = self.secondary.feedwater_enthalpy();
        let mut secondary_flow = self.secondary.mass_flow();

        let n_outer = n_outer.max(1);
        for corrector in 0..n_outer {
            if corrector > 0 {
                self.kinetics = kinetics_at_step_start.clone();
                // NOTE: `core_heat_to_helium` is deliberately NOT rewound. It
                // is an OUTPUT of the bed step, not integrated state, and it
                // is the kinetics sink's coupling variable -- so it must carry
                // the previous corrector's converged value forward, exactly
                // like `helium_bulk` and `feedwater_enthalpy` below. Rewinding
                // it would pin the kinetics to the start-of-step removal on
                // every corrector and defeat the loop.
                self.core = core_at_step_start;
                self.primary.restore_lumped_state(primary_at_step_start);
                self.secondary
                    .restore_integrated_state(secondary_at_step_start);
                self.decay_heat_path = decay_heat_path_at_step_start;
            }

            // 0. The circulator flow for this step. Prescribed (there is no
            //    momentum equation), so it is set BEFORE the bed: the bed,
            //    both helium CVs and the steam generator must all carry the
            //    same m_dot (2026-09-29; the bed used to read the previous
            //    step's flow).
            self.primary.command_flow(helium_flow_setpoint);

            // 1. Kinetics -> reactor fission power. The reactivity feedback
            //    stays inside Nordheim-Fuchs's closed form (that exactness is
            //    what keeps the stiff feedback term non-stiff); what the
            //    kinetics now takes from the rest of the plant is the heat
            //    SINK on its fuel node, applied in step 2b below. See
            //    `kinetics::apply_coolant_heat_removal`.
            //
            //    Each subsystem announces itself before stepping, so a panic
            //    anywhere below is attributed to a piece of PLANT EQUIPMENT in
            //    the crash modal rather than only to a source file. The whole
            //    plant runs on one physics thread, so the thread name alone
            //    identifies nothing.
            mark_component("reactor kinetics (point kinetics + control rods)");
            // ~~The heat sink on the fuel node is the previous corrector's
            // coolant heat removal ... The kernel-above-node resistance ... so
            // the Doppler channel can follow the kernel~~ -- both replaced
            // 2026-09-28 (gh:#360) by the fuel-to-bed coupling below.
            // CHANGED 2026-09-28 (gh:#360): the fuel node's sink is conduction
            // to the BED through the resolved-pebble resistance, not the
            // bed's heat-to-helium. The bed temperature is the corrector's
            // current estimate (start-of-step on the first pass, the previous
            // corrector's end-of-step after) -- the same predictor-corrector
            // treatment every coupling here gets.
            self.kinetics.step(
                dt,
                external_reactivity_dollars,
                bed_temperature_estimate,
                self.core.fuel_bed_coupling().map(|c| c.resistance),
            );

            // 2. Pebble bed absorbs the core's THERMAL power -- the promptly
            //    released fission power plus fission-product decay heat, not
            //    the raw fission power (see `kinetics::core_thermal_power`) --
            //    and hands the helium only what crosses the pebble surface.
            //    `helium_bulk` is the start-of-step value on the first
            //    corrector and the previous corrector's end-of-step value
            //    thereafter, so the coupling moves from explicit toward
            //    implicit as the loop iterates. The bed's ~184 s time constant
            //    makes this the least sensitive of the couplings either way.
            mark_component("pebble-bed core (graphite pebbles)");
            // CHANGED 2026-09-28 (gh:#360): the bed's source is what the fuel
            // node conducted to it this step -- exactly the energy the fuel
            // gave up -- not the reactor's thermal power. Fission (prompt
            // share) and decay heat were deposited in the FUEL by the kinetics
            // step above.
            let heat_from_fuel = self.kinetics.heat_to_bed();

            // 2a. ~~The passive decay-heat path advanced BEFORE the bed against a
            //     held bed temperature, and its heat subtracted from the bed's
            //     source (`net_core_source = heat_from_fuel -
            //     passive_heat_loss`)~~ -- DELETED 2026-09-29 (gh:#395). That
            //     was the pattern the engine CLAUDE.md names as a defect: a
            //     transfer term applied as an adjustment to a CV's source
            //     outside its implicit solve. The reflector and RPV are now
            //     unknowns of the bed's own backward-Euler system, with the
            //     bed -> reflector legs on the diagonal (see
            //     `one_node::PebbleBedPorousMediaNode::step` and
            //     `decay_heat_removal`). Under forced flow the path is a few
            //     hundred kW; under LOFC it is the ONLY path out of the core.
            //     It is rewound per corrector with the rest of the state.
            self.core_heat_to_helium = self.core.step(
                dt,
                heat_from_fuel,
                heat_from_fuel,
                core_inlet_enthalpy,
                self.primary.mass_flow(),
                &mut self.decay_heat_path,
            );
            self.passive_heat_loss = self.decay_heat_path.heat_from_core();
            bed_temperature_estimate = self.core.temperature();

            // 3a. Primary hot-duct CV: the hot-gas plenum and the hot gas
            //     duct, an enthalpy balance on the bed's outflow.
            //     ~~A first-order lag on the core outlet plus a second-law
            //     clamp~~ -- deleted 2026-09-29 (gh:#391): the lag counted the
            //     bed helium's inertia twice and the clamp stood in for a
            //     formulation.
            mark_component("helium primary loop (circulator + hot gas duct)");
            self.primary
                .step_hot_duct(dt, self.core.helium_outlet_enthalpy());

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

            // 3c. Primary cold-return CV: loop hydraulics and circulator
            //     work, then the CV's enthalpy balance with the work as its
            //     source. It discharges exactly the enthalpy the bed was
            //     handed above, so the seam is one flux.
            //     ~~The core inlet relaxes toward the exchanger's helium-side
            //     outlet over an invented, flow-independent 8 s~~ -- deleted
            //     2026-09-29 (gh:#392).
            //     Its source includes the heat the side reflector gave the
            //     riser helium (gh:#397), computed in the bed's solve above.
            self.primary.close_return_leg(
                dt,
                core_inlet_enthalpy,
                self.decay_heat_path.heat_to_risers(),
            );

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
            //    The hot-side inlet it is handed for that backstop is the
            //    steam generator's own helium inlet, the hot-duct CV.
            self.secondary.step(
                dt,
                secondary_commands,
                self.primary.steam_generator_duty_to_secondary(),
                self.primary.hot_duct_temperature(),
            );

            // Hand the improved coupling values to the next corrector.
            core_inlet_enthalpy = self.primary.core_inlet_enthalpy();
            feedwater_enthalpy = self.secondary.feedwater_enthalpy();
            secondary_flow = self.secondary.mass_flow();
        }

        // The global energy ledger, from each subsystem's final-corrector
        // bookkeeping. See `PlantEnergyLedger`.
        {
            use uom::si::energy::joule;
            let dt_s = dt.get::<second>();
            let fuel = self.kinetics.ledger();
            let bed = self.core.last_step_energy();
            let primary = self.primary.last_step_energy();
            let mut step = PlantEnergyLedger {
                source: (fuel.deposited_prompt - fuel_ledger_at_step_start.deposited_prompt)
                    + (fuel.deposited_decay - fuel_ledger_at_step_start.deposited_decay),
                fuel_storage: fuel.stored - fuel_ledger_at_step_start.stored,
                bed_solid_storage: bed.solid_storage,
                bed_helium_storage: bed.fluid_storage,
                hot_duct_storage: primary.hot_duct_storage,
                cold_return_storage: primary.cold_return_storage,
                passive_storage: (self.decay_heat_path.stored_energy()
                    - passive_stored_at_step_start)
                    .get::<joule>(),
                to_steam_generator: primary.to_steam_generator,
                to_rccs: self.decay_heat_path.heat_to_rccs().get::<watt>() * dt_s,
                circulator_work: primary.circulator_work,
                residual: 0.0,
            };
            step.residual = step.source + step.circulator_work
                - step.stored()
                - step.to_steam_generator
                - step.to_rccs;
            self.last_step_energy = step;
            self.energy.accumulate(&step);
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

        // 6. TRISO fission-product release, at the converged fuel-kernel
        // temperature. OUTSIDE the corrector loop and after it, for the same
        // reason the shaft is: it is a pure consumer of converged state and
        // feeds nothing back into the plant. It is also quasi-steady and holds
        // no integrated state, so there is nothing for a corrector to rewind
        // and nothing gained by iterating it.
        //
        // It is handed the FUEL STACK, not the bed's temperature: the
        // inventory-averaged kernel (the kinetics fuel node), the SiC layer
        // for silver, and the fuelled-zone matrix for graphite hold-up
        // (CHANGED 2026-09-28, gh:#360 -- it used to take the peak kernel of a
        // start-of-step profile and the whole-ball bed average). The release
        // coefficients are Arrhenius, so the wrong temperature would not give
        // a slightly wrong answer, it would give a confident one. See
        // `fission_product_release`.
        mark_component("TRISO fission-product release (TRISO-ATOPS)");
        // The live primary pools' plate-out is per loop cycle, so it follows
        // the loop flow (gh:#399).
        self.release.set_primary_flow(self.primary.mass_flow());
        self.release
            .update(self.sim_time.get::<second>(), self.fuel_stack_temperatures());

        // 7. Atmospheric dispersion, driven by the release channel's
        // circulating pool. Last, and outside the corrector loop, for the same
        // reason as the two above: a pure consumer of converged state that
        // feeds nothing back. Throttled harder still (60 s against the release
        // channel's 1 s) because a puff run is the most expensive thing in
        // this plant and is quasi-steady -- nothing it reads can change faster.
        mark_component("atmospheric dispersion (Gaussian puff)");
        // Apply the commanded wind before evaluating. `set_meteorology` forces
        // a re-evaluation when the value actually changes, so an operator who
        // turns the wind sees the rose follow without waiting out the throttle.
        if commands.meteorology != self.dispersion.meteorology() {
            // The map's direction control only ROTATES the plume (maintainer
            // direction 2026-09-29; `map_puff_model`): rotate the marched
            // population to the new bearing first, so the plume is the one a
            // steady wind from there would have built rather than a kinked
            // Lagrangian response to a wind shift.
            let new_from = commands.meteorology.direction_from;
            if new_from != self.dispersion.meteorology().direction_from {
                self.dispersion.rotate_population_to(new_from);
            }
            self.dispersion.set_meteorology(commands.meteorology);
        }
        // The map field at 10 Hz, but ONLY when a map is actually on screen.
        // See `Self::map_field_live` for why this is the right discriminator
        // and not an adapter probe: a GUI run has a rendering adapter by
        // construction (eframe would not have started without one), so the
        // field costs 1.3 ms there (measured, 512 cells on the GPU), while a
        // headless run has no map to refresh and must stay free of
        // wall-clock-dependent state.
        //
        // `refresh_field` is a 10 Hz SCHEDULE, not merely a rate limit:
        // ~~it recomputes only when the meteorology actually changed~~ --
        // CORRECTED 2026-09-25, the field is instantaneous now, so the plume
        // clock advances it on every tick. The field also still updates
        // through `update`'s own 60 s throttle below either way. This gates
        // CADENCE, never the model: both paths compute the same numbers.
        // The map's own request -- resolution and plume clock -- travels with
        // the commands, same as the wind. Applied before the refresh below so
        // a resolution change or a fast-forward takes effect on this tick
        // rather than the next.
        if commands.map_field != self.dispersion.field_request() {
            self.dispersion.set_field_request(commands.map_field);
        }
        if self.map_field_live {
            self.dispersion
                .refresh_field(self.sim_time.get::<second>());
        }
        self.dispersion
            .update(self.sim_time.get::<second>(), &self.release);
    }

    /// Project the current plant state onto the shared [`HtgrSnapshot`],
    /// writing only the *output* fields and leaving the GUI-owned control
    /// inputs untouched.
    pub fn write_snapshot(&self, s: &mut HtgrSnapshot) {
        // Kinetics.
        s.reactor_power_mw = power_in_megawatts(self.kinetics.total_power());
        s.prompt_power_mw = power_in_megawatts(self.kinetics.prompt_power());
        s.delayed_power_mw = power_in_megawatts(self.kinetics.delayed_power());
        s.fuel_temperature_k = self.kinetics.fuel_temperature().get::<kelvin>();
        // The BED temperature, not the kinetics fuel node. The two differ: the
        // kinetics node is a lumped point-kinetics fuel temperature driving
        // reactivity feedback, while this is the graphite the helium flows
        // over. Drawing the kinetics node made the bed appear COOLER than the
        // gas leaving it, which is thermodynamically impossible.
        s.bed_temperature_k = self.pebble_temperature().get::<kelvin>();

        // The resolved fuel kernel, and what it is worth. `NAN` rather than a
        // fallback when the tier does not resolve one -- see the field docs.
        // CHANGED 2026-09-28 (gh:#360): the peak kernel follows the FUEL NODE
        // through the fuel-bed coupling (stack), so it exists on every tier
        // and in a burst, where the steady profile does not.
        let kernel = self.fuel_stack_temperatures().map(|st| st.peak_kernel);
        s.peak_kernel_temperature_k = kernel.map_or(f64::NAN, |t| t.get::<kelvin>());
        s.kernel_offset_k = kernel.map_or(f64::NAN, |t| {
            t.get::<kelvin>() - self.core.temperature().get::<kelvin>()
        });
        // CHANGED 2026-09-28 (gh:#360): the fuel (kernel) channel of the split
        // isothermal coefficient, on the fuel node itself.
        s.kernel_doppler_dollars = self.kinetics.fuel_feedback_reactivity_dollars();

        // TRISO fission-product release, on a UNIT-INVENTORY basis. See
        // `fission_product_release` -- these are Ci per Ci of core inventory
        // and are not a source term for any reactor.
        s.release_circulating_ci_per_ci = self.release.total_circulating();
        s.release_evaluated_at_kernel_k = self
            .release
            .evaluated_at_kernel()
            .map_or(f64::NAN, |t| t.get::<kelvin>());
        // The resolved pebble interior, published in full -- see the field
        // docs on why the GUI is not left to interpolate it.
        let profile = self.core.pebble_profile();
        s.pebble_surface_k = profile.map_or(f64::NAN, |p| p.surface.get::<kelvin>());
        s.pebble_zone_boundary_k =
            profile.map_or(f64::NAN, |p| p.fuelled_zone_boundary.get::<kelvin>());
        s.pebble_centre_k = profile.map_or(f64::NAN, |p| p.centre.get::<kelvin>());
        s.particle_sic_k = profile.map_or(f64::NAN, |p| {
            p.hottest_particle.silicon_carbide_outer.get::<kelvin>()
        });

        // Atmospheric dispersion. chi/Q is the quotable field; the two
        // activity fields are a transfer function -- see the snapshot's own
        // field docs and `atmospheric_dispersion`'s scope limit.
        // NOTE: `wind_speed_m_per_s` and `wind_from_deg` are deliberately NOT
        // written here. They are GUI-owned CONTROL INPUTS, like the rod
        // position and the flow setpoint -- `write_snapshot` writes output
        // fields only, and echoing a command back would overwrite whatever the
        // operator had just dialled in on the very next tick.
        if let Some(result) = self.dispersion.latest() {
            s.dispersion_evaluated_at_s = result.evaluated_at_s;
            s.stability_class = result.stability.map_or("", |c| c.letter());
            for (slot, r) in s.receptors.iter_mut().zip(result.receptors.iter()) {
                slot.bearing_deg = r.bearing_deg;
                slot.distance_m = r.distance_m;
                slot.chi_over_q = r.chi_over_q;
                slot.instantaneous_chi_over_q = r.instantaneous_chi_over_q;
                slot.air_bq_s_per_m3 = r.air_bq_s_per_m3;
                slot.ground_bq_per_m2 = r.ground_bq_per_m2;
                slot.air_bq_s_per_m3_absolute = r.air_bq_s_per_m3_absolute.unwrap_or(f64::NAN);
                slot.ground_bq_per_m2_absolute = r.ground_bq_per_m2_absolute.unwrap_or(f64::NAN);
                slot.ground_bq_per_m2_absolute_by_nuclide = r.ground_bq_per_m2_absolute_by_nuclide;
            }
            s.dispersion_source_rate_absolute_by_nuclide_bq_per_s = result
                .source_rate_absolute_by_nuclide_bq_per_s
                .map(|r| r.unwrap_or(f64::NAN));
            s.dispersion_source_rate_per_ci_bq_per_s = result.source_rate_per_ci_bq_per_s;
            s.dispersion_source_rate_absolute_bq_per_s =
                result.source_rate_absolute_bq_per_s.unwrap_or(f64::NAN);
            // Reuse the allocation across ticks: the grid is a fixed size and
            // this runs on every write.
            s.dispersion_grid.clear();
            s.dispersion_grid
                .extend(result.grid.chi_over_q.iter().map(|v| *v as f32));
            s.dispersion_grid_cells = result.grid.cells;
            s.dispersion_grid_half_width_m = result.grid.half_width_m;
            // The PLUME clock, which is the plant clock plus the operator's
            // fast-forward offset -- not the plant clock. The Map tab shows
            // both and says which is which; see `MapFieldRequest`.
            s.dispersion_grid_time_s = result.grid.plume_time_s;
        }

        for (slot, release) in s.release.iter_mut().zip(self.release.latest()) {
            slot.name = release.name;
            slot.release_rate = release.activities.release_rate;
            slot.graphite_activity = release.activities.graphite_activity;
            slot.circulating_activity = release.activities.circulating_activity;
            slot.plate_out_activity = release.activities.plate_out_activity;
            slot.clean_up_activity = release.activities.clean_up_activity;
        }
        s.trip_reason = self.protection.trip_reason();
        s.scram_insertion_fraction = self.protection.scram_insertion();
        s.reactivity_margin_dollars = self.kinetics.reactivity_margin_dollars();
        s.budget_external_dollars = self.kinetics.external_reactivity_dollars();
        s.budget_fuel_dollars = self.kinetics.fuel_feedback_reactivity_dollars();
        s.budget_moderator_dollars = self.kinetics.moderator_feedback_reactivity_dollars();
        s.budget_xenon_dollars = self.kinetics.xenon_reactivity_dollars();
        s.budget_net_dollars = self.kinetics.net_reactivity_dollars();
        s.kinetics_beta = self
            .kinetics
            .kinetics_delayed_neutron_fraction()
            .get::<uom::si::ratio::ratio>();
        s.decay_heat_mw = power_in_megawatts(self.kinetics.decay_heat_power());
        s.core_thermal_power_mw = power_in_megawatts(self.kinetics.core_thermal_power());
        let controller = self.secondary.feedwater_controller();
        s.steam_temperature_error_k = controller.error();
        s.feedwater_controller_output = controller.last_output();
        s.delayed_neutron_fraction_pcm = self
            .kinetics
            .delayed_neutron_fraction()
            .get::<uom::si::ratio::ratio>()
            * 1.0e5;

        // Primary loop.
        s.core_inlet_temp_k = self.primary.core_inlet_temperature().get::<kelvin>();
        s.core_outlet_temp_k = self.primary.core_outlet_temperature().get::<kelvin>();
        s.hot_duct_temp_k = self.primary.hot_duct_temperature().get::<kelvin>();
        s.helium_mass_flow_kg_per_s = self.primary.mass_flow().get::<kilogram_per_second>();
        s.ihx_duty_mw = self.primary.ihx_duty().get::<megawatt>();
        s.sg_secondary_duty_mw = self
            .primary
            .steam_generator_duty_to_secondary()
            .get::<megawatt>();
        s.passive_heat_loss_mw = self.passive_heat_loss.get::<megawatt>();
        s.riser_heat_mw = self.decay_heat_path.heat_to_risers().get::<megawatt>();
        s.reflector_temp_k = self.decay_heat_path.reflector_temperature().get::<kelvin>();
        s.rpv_temp_k = self.decay_heat_path.rpv_temperature().get::<kelvin>();
        let e = self.energy_ledger();
        s.energy_source_j = e.source;
        s.energy_stored_j = e.stored();
        s.energy_to_steam_generator_j = e.to_steam_generator;
        s.energy_to_rccs_j = e.to_rccs;
        s.energy_circulator_work_j = e.circulator_work;
        s.energy_residual_j = e.residual;
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
        s.secondary_residence_time_s = residence_time_from_flow(
            self.secondary.piping_inventory(),
            self.secondary.mass_flow(),
        )
        .get::<second>();
        s.secondary_piping_inventory_kg = self.secondary.piping_inventory().get::<kilogram>();
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

    /// **The map-field refresh must be OFF unless a GUI turned it on.**
    ///
    /// `AtmosphericDispersionChannel::refresh_field` is gated on
    /// `std::time::Instant`, so a plant that takes it produces a trace that
    /// depends on how fast the host is. `headless.rs` asserts
    /// `run(&cfg) == run(&cfg)`; this test pins the flag that would break
    /// that assertion, at the constructor every headless run and every test in
    /// this file goes through.
    ///
    /// The GUI's opt-in is pinned in the same test so the two cannot drift:
    /// if someone flips the default, one of the two halves fails.
    #[test]
    fn the_live_map_field_is_off_by_default_and_only_the_gui_builder_turns_it_on() {
        assert!(
            !HtgrPlant::new().map_field_live,
            "HtgrPlant::new() must not take the wall-clock-gated map refresh; \
             headless determinism depends on it"
        );
        assert!(
            HtgrPlant::new().with_live_map_field().map_field_live,
            "with_live_map_field() is the GUI's opt-in and must set the flag"
        );
    }
    use super::*;
    use crate::app::state::HtgrSnapshot;

    /// The control-rod insertion fraction the GUI opens with -- **a read of
    /// [`GUI_INITIAL_ROD_INSERTION`], not an independent literal.**
    ///
    /// It was an independent literal until 2026-08-13, kept in step with
    /// `crate::app::state::HtgrSnapshot::default` by a test. Both now read the
    /// one constant, so they cannot disagree; the test below is what pins the
    /// identity of the source.
    const HTGR_GUI_INITIAL_ROD_INSERTION: f64 = GUI_INITIAL_ROD_INSERTION;

    /// The command set the whole-plant tests drive: the plant's own design
    /// point, at the GUI's opening rod position.
    ///
    /// Every test in this module used to pass a rod position and a flow
    /// positionally; they now pass a [`PlantCommands`], and this helper builds
    /// the one they all shared so a test that wants a *different* command says
    /// so explicitly.
    fn design_commands() -> PlantCommands {
        PlantCommands {
            control_rod_insertion_fraction: HTGR_GUI_INITIAL_ROD_INSERTION,
            helium_flow_setpoint: nominal_helium_flow(),
            secondary: SecondaryCommands::default(),
            scenario: Scenario::Normal,
            meteorology: atmospheric_dispersion::Meteorology::default(),
            map_field: atmospheric_dispersion::MapFieldRequest::default(),
        }
    }

    /// The whole-plant test must open where the GUI opens, and where
    /// [`PlantCommands::default`] opens. Withdrawing the bank by even ten
    /// percent from critical is a prompt excursion in this core, so the three
    /// must not be allowed to drift apart silently.
    ///
    /// **Now an identity check rather than an agreement check** -- all three
    /// read [`GUI_INITIAL_ROD_INSERTION`], so this fails only if someone
    /// reintroduces a local literal.
    #[test]
    fn the_test_rod_position_matches_the_gui_default() {
        let gui_default = HtgrSnapshot::default().control_rod_insertion_fraction;
        assert!(
            (gui_default - HTGR_GUI_INITIAL_ROD_INSERTION).abs() < 1e-9,
            "the GUI opens at rod insertion {gui_default}, this test uses \
             {HTGR_GUI_INITIAL_ROD_INSERTION}"
        );
        assert!(
            (PlantCommands::default().control_rod_insertion_fraction
                - HTGR_GUI_INITIAL_ROD_INSERTION)
                .abs()
                < 1e-9
        );
    }

    /// V&V: **what external reactivity the rod position the simulator opens at
    /// actually commands, and whether the bank retains shutdown authority from
    /// there** -- both measured, neither trusted as a literal.
    ///
    /// # ~~"the opening rod position is the critical one"~~ -- REPLACED 2026-09-27
    ///
    /// This test was called `the_opening_rod_position_is_the_critical_one` and
    /// asserted `|critical - GUI_INITIAL_ROD_INSERTION| < 5.0e-3`. **It had
    /// been failing on `develop` since [`GUI_INITIAL_ROD_INSERTION`] moved to
    /// 0.50 on 2026-09-22** -- 0.105 of bank travel from the bisection's
    /// 0.604534, and `+4.7294 $` where the old assertion allowed `0.25 $`.
    /// Measured failing 2026-09-27 before this rewrite; the failure was
    /// already recorded in
    /// `the_kernel_doppler_channel_dominates_the_shutdown_transient`'s doc
    /// comment as "a pre-existing fragility".
    ///
    /// **The premise, not the threshold, was what had gone stale.** The
    /// maintainer set 0.50 (2026-09-22) and then 0.30 (2026-09-27) precisely
    /// so the bank opens *shallower* than critical, leaving travel in reserve
    /// to demonstrate an ATWS shutdown after a DLOFC or LOFC. A gate asserting
    /// the opening position *is* critical contradicts that decision outright,
    /// so relaxing its tolerance to accommodate 0.30 would have been moving a
    /// threshold to make a test pass -- forbidden. The gate is instead
    /// **replaced by one that asserts the criterion actually in force**, and
    /// the superseded numbers are kept above so a reader meeting the old name
    /// elsewhere can date it.
    ///
    /// # Methodology
    ///
    /// [`GUI_INITIAL_ROD_INSERTION`] is a `const`, so it cannot call
    /// [`control_rods::critical_insertion_fraction`] (a bisection returning an
    /// `Option`). This test closes that gap. At the kinetics' own
    /// delayed-neutron fraction it computes, from
    /// [`control_rods::external_reactivity_dollars`] alone (no plant is
    /// stepped):
    ///
    /// 1. the cold-clean critical insertion, by bisection;
    /// 2. the external reactivity the shipped opening position commands;
    /// 3. the external reactivity at **full** insertion, which is the
    ///    shutdown authority the bank still has from anywhere.
    ///
    /// Pass criteria, each one a restatement of the maintainer's own stated
    /// intent rather than a tolerance chosen to fit:
    ///
    /// - the opening position lies **inside the mechanical stops**, `0..=1`;
    /// - it is **shallower than critical**, i.e. the plant opens supercritical
    ///   in the cold-clean sense and is held down by temperature feedback, not
    ///   by the bank -- this is the deliberate design, and a regression that
    ///   pushed it deeper than critical would change what the simulator
    ///   demonstrates;
    /// - **the bank can still shut the core down from the opening position**:
    ///   full insertion commands strictly negative external reactivity, with
    ///   at least one dollar of margin. This is the property 0.30 was chosen
    ///   *for*, and the one worth gating.
    ///
    /// The `beta` ambiguity documented on [`GUI_INITIAL_ROD_INSERTION`] applies
    /// to every dollar figure here and not to the pcm ones; this test reads the
    /// **delayed** layer, `beta = 0.00650`.
    ///
    /// # Results (measured 2026-09-27, `beta = 0.00650`)
    ///
    /// | Quantity | Value |
    /// |---|---|
    /// | Cold-clean critical insertion (bisection) | **0.604534** |
    /// | Shipped opening insertion | **0.300000** |
    /// | External reactivity at the opening position | printed by this test |
    /// | External reactivity at full insertion | printed by this test |
    /// | ~~Shipped 0.50 / `+4.7294 $` / `+3074 pcm`~~ | ~~superseded 2026-09-27~~ |
    ///
    /// The exact opening figure at 0.30 is left to the test's own stdout rather
    /// than transcribed here, because a number copied into a doc comment by
    /// hand is the class of claim this workspace's rules exist to stop: run the
    /// test with `--nocapture` and read it. What *is* asserted above is the
    /// sign and the margin, which is what the design depends on.
    ///
    /// **Interpretation.** The simulator opens well above cold-clean critical
    /// and relies on negative temperature feedback to hold it -- deliberately,
    /// so that inserting the remaining 70 % of bank travel is a demonstrable
    /// shutdown. It follows that this opening state is **not** a near-critical
    /// initial condition and must not be described as one, and that a transient
    /// started here is sensitive to the feedback model in a way a
    /// near-critical start would not be. That sensitivity is real and is
    /// recorded, not gated away.
    #[test]
    fn the_opening_rod_position_commands_a_known_reactivity() {
        let beta = HtgrKinetics::new_htr10_published(nominal_thermal_power())
            .delayed_neutron_fraction()
            .get::<uom::si::ratio::ratio>();
        let critical = control_rods::critical_insertion_fraction(beta)
            .expect("the bank must be able to reach critical at the illustrative beta");
        let rho_open = control_rods::external_reactivity_dollars(GUI_INITIAL_ROD_INSERTION, beta);
        let rho_full = control_rods::external_reactivity_dollars(1.0, beta);
        println!(
            "OPENING ROD POSITION (beta = {beta:.5})\n  \
             shipped insertion        = {GUI_INITIAL_ROD_INSERTION:.6}\n  \
             cold-clean critical      = {critical:.6} (bisection)\n  \
             rho_ext at opening       = {rho_open:+.5} $ ({:+.1} pcm)\n  \
             rho_ext at full insertion = {rho_full:+.5} $ ({:+.1} pcm)\n  \
             bank travel left in reserve = {:.3} of full\n  \
             The plant opens SUPERCRITICAL in the cold-clean sense and is held \
             down by temperature feedback, by design -- see the doc comment.",
            rho_open * beta * 1.0e5,
            rho_full * beta * 1.0e5,
            1.0 - GUI_INITIAL_ROD_INSERTION,
        );

        assert!(
            (0.0..=1.0).contains(&GUI_INITIAL_ROD_INSERTION),
            "the opening insertion {GUI_INITIAL_ROD_INSERTION} is outside the mechanical stops"
        );
        assert!(
            GUI_INITIAL_ROD_INSERTION < critical,
            "the simulator is meant to open SHALLOWER than cold-clean critical so bank travel \
             is left in reserve for an ATWS shutdown demonstration; opening insertion is \
             {GUI_INITIAL_ROD_INSERTION} against a critical position of {critical}"
        );
        assert!(
            rho_open > 0.0,
            "opening shallower than critical must command POSITIVE external reactivity; \
             got {rho_open} $ at insertion {GUI_INITIAL_ROD_INSERTION}"
        );
        assert!(
            rho_full < -1.0,
            "the bank must retain real shutdown authority from the opening position: full \
             insertion commands {rho_full} $, which is not at least one dollar subcritical"
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
    /// **2026-08-14, on a different machine**: 2.175 s wall for the same 20 s of
    /// plant time -- **0.1087 compute per plant second, ratio 9.196**, about
    /// nine times the figure recorded the day before.
    ///
    /// **Do not read that as a speedup.** These numbers are machine
    /// measurements, and this workspace is worked from several hosts (the
    /// commits above come from at least three). Nothing here identifies which
    /// host produced which reading, so a ratio measured on one cannot be
    /// differenced against a ratio measured on another to attribute a code
    /// change -- and no attempt is made to. The honest statement is that this
    /// model runs anywhere between **0.5x and 9.2x real time** depending on the
    /// host and its load, and that **only same-host, same-load pairs may be
    /// compared**. If a future change wants to claim a speedup, it must re-run
    /// the before and after on one machine and say which.
    ///
    /// # Interpretation
    ///
    /// The wall-clock figure printed here is a **measurement of the maintainer's
    /// workstation**, not a property of the model, and it moves with machine
    /// load. It is not asserted on -- a timing assertion would fail on a busy
    /// CI box for no physical reason. What is asserted is that the plant
     /// **Where the plant step actually spends its time** -- an instrument,
    /// not an assertion.
    ///
    /// # Why this exists
    ///
    /// The simulator computes at roughly real time, which is what forces
    /// every scheduling decision in the application layer: the 100 ms
    /// `PHYSICS_TICK`, the map field's separate 10 Hz clock, the resolution
    /// ceiling in [`atmospheric_dispersion::max_grid_cells`], and the
    /// maintainer's 2026-09-25 decision to fast-forward the plume clock
    /// rather than the plant. All of those rest on *which* part of the step
    /// is expensive, and that had only ever been recorded in a doc comment
    /// (`crate::app`'s "Where the compute actually goes" table, 2026-08-13),
    /// with nothing in the repository able to reproduce it. A number that
    /// decides a design and cannot be re-measured is not evidence.
    ///
    /// # Methodology
    ///
    /// The whole plant is stepped over a fixed span of plant time at the
    /// shipped [`PLANT_TIMESTEP_S`], and two of the three channels suspected
    /// of dominating it are then timed **on their own**, driven with the same
    /// state the plant hands them:
    ///
    /// - the TRISO-ATOPS release channel
    ///   ([`fission_product_release`]), throttled in the plant to 1 s;
    /// - the Gaussian puff dispersion channel
    ///   ([`atmospheric_dispersion`]), throttled to 60 s.
    ///
    /// Each is timed at the rate the plant actually calls it over the run, so
    /// the figures are directly comparable shares of the same wall clock, not
    /// per-call costs that still have to be multiplied by a cadence.
    ///
    /// The step's two loops are then broken down the same way, one level
    /// deeper: [`primary_loop::HeliumPrimaryLoop::advance_steam_generator`],
    /// [`primary_loop::HeliumPrimaryLoop::step_hot_duct`] (~~`step_hot_leg`~~
    /// until 2026-09-29; the table below was measured on it) and
    /// [`secondary_loop::SteamSecondaryLoop::step`] are each timed on the
    /// plant in the state the run left it, with the arguments
    /// [`HtgrPlant::step_with_correctors`] passes them, and charged at
    /// [`PLANT_OUTER_CORRECTORS`] calls per plant step -- which is how often
    /// the corrector loop really calls them.
    ///
    /// This test asserts only that the run completes; the timings are printed.
    /// A wall-clock assertion would be a flaky test on a shared machine, and
    /// the point here is the *breakdown*, which is stable, rather than the
    /// absolute numbers, which are not.
    ///
    /// # Results (2026-09-25), 16 logical cores, run alone
    ///
    /// | Part | Share of the plant step |
    /// |---|---|
    /// | **`primary_loop::advance_steam_generator`** | **~90 %** (23.7 ms per call, once per step) |
    /// | `secondary_loop::step` (IF97) | 0.44 % (0.057 ms per call, twice per step) |
    /// | `primary_loop::step_hot_leg` (core energy balance) | 0.03 % |
    /// | Gaussian puff dispersion (`changi`) | 0.61 % |
    /// | TRISO-ATOPS release (`boon-lay`) | 0.08 % |
    ///
    /// Whole plant: **0.262 s of compute per second of plant time, real-time
    /// ratio 3.82**.
    ///
    /// **Interpretation.** The cost is one function: the steam generator's
    /// array model. The two channels that *look* expensive -- a TRISO release
    /// model and a puff dispersion run -- are together under 0.7 %, so moving
    /// either onto its own thread would buy nothing measurable. Neither is
    /// the secondary loop, despite doing the IF97 property work, nor the core
    /// energy balance.
    ///
    /// # The first run of this instrument charged the steam generator twice
    ///
    /// It reported **181 %** of the step for `advance_steam_generator` -- an
    /// impossible share, and the instrument's own arithmetic is what exposed
    /// it. The cause was the multiplicity assumed here, not the timing: the
    /// hot leg and the secondary loop run once per outer corrector, but the
    /// steam generator runs **once per plant step**, on the final corrector
    /// only, because its arrays hold spatial history that cannot be rewound
    /// (step 3b of [`HtgrPlant::step_with_correctors`], and
    /// [`PLANT_OUTER_CORRECTORS`] says why). Corrected to 1x, it reads ~90 %,
    /// which independently reproduces the ~96 % that `primary_loop`'s own doc
    /// comment has claimed since 2026-08-13 -- a claim that until now nothing
    /// in the repository could re-measure.
    #[test]
    fn where_the_plant_step_spends_its_time() {
        let plant_seconds = 20.0_f64;
        let steps = (plant_seconds / PLANT_TIMESTEP_S).round() as usize;
        let commands = design_commands();
        let dt = plant_timestep();

        let mut plant = HtgrPlant::new();
        let started = std::time::Instant::now();
        for _ in 0..steps {
            plant.step(dt, commands);
        }
        let whole = started.elapsed().as_secs_f64();

        // The TRISO release channel alone, called as often as the plant calls
        // it over the same span (its throttle is 1 s of plant time).
        // A REPRESENTATIVE kernel temperature, not the plant's own -- and that is
        // a fix, not a shortcut (2026-09-27).
        //
        // `plant.core.peak_kernel_temperature()` returns `Option`, and at the
        // shipped opening condition it is **`None`**: the excursion takes the bed
        // to ~2589 K within 8 s (gh:#318) and the pebble solver cannot resolve a
        // kernel there. A `None` kernel gives the release channel nothing, an
        // empty release channel makes `dispersion.update` return early, and both
        // this block's release and dispersion timers then charge 0.0000 s for
        // work that never happened -- which is exactly what they did, and what
        // was briefly read as "the dispersion is free".
        //
        // Charging a representative 1100 K instead measures the cost of the model
        // ACTUALLY RUNNING, which is what this breakdown is for. It is an upper
        // bound on the plant's own cost at a resolved kernel, and it is honest
        // about being a stand-in rather than the plant's state. The plant's real
        // thermal state is the subject of gh:#318, not of a timing breakdown.
        let kernel = ThermodynamicTemperature::new::<kelvin>(1100.0);
        let bed = plant.core.temperature();
        let stack = Some(pebble_bed::FuelStackTemperatures {
            kernel,
            silicon_carbide: kernel,
            fuelled_zone_matrix: bed,
            peak_kernel: kernel,
        });
        let release_calls = (plant_seconds
            / fission_product_release::RELEASE_EVALUATION_INTERVAL_S)
            .ceil() as usize;
        let mut release = fission_product_release::TrisoAtopsReleaseChannel::new_htr10();
        let started = std::time::Instant::now();
        for call in 0..release_calls {
            // A fresh time each call so the channel's own throttle does not
            // turn the loop into one evaluation and 19 early returns.
            release.update(
                call as f64 * fission_product_release::RELEASE_EVALUATION_INTERVAL_S,
                stack,
            );
        }
        let release_time = started.elapsed().as_secs_f64();

        // The dispersion channel alone, likewise -- charged in full for every
        // evaluation the throttle allows over the span, which if anything
        // overstates it.
        //
        // COUNTING the evaluations that actually ran is not bookkeeping, it is the
        // fix for a real defect -- CORRECTED 2026-09-27.
        //
        // `AtmosphericDispersionChannel::update` returns EARLY, doing nothing, when
        // the release channel has produced no source term yet. This loop had no
        // way to tell that apart from a fast evaluation, so it reported
        // `0.0000 s wall over N calls = 0.00 %` either way -- and that zero was
        // read on 2026-09-27 as evidence the dispersion side was free, which in
        // turn was used to argue a throttle could be removed. A standalone
        // measurement of the same call
        // (`atmospheric_dispersion::tests::what_one_dispersion_evaluation_costs`)
        // puts one evaluation at **26.8 ms**.
        //
        // So the number this block prints must be accompanied by proof that the
        // model ran. `update` already returns `bool` for exactly this; nobody was
        // reading it.
        let dispersion_calls = (plant_seconds
            / atmospheric_dispersion::DISPERSION_EVALUATION_INTERVAL_S)
            .ceil()
            .max(1.0) as usize;
        let mut dispersion = atmospheric_dispersion::AtmosphericDispersionChannel::new();
        let mut dispersion_evaluated = 0usize;
        let started = std::time::Instant::now();
        for call in 0..dispersion_calls {
            if dispersion.update(
                call as f64 * atmospheric_dispersion::DISPERSION_EVALUATION_INTERVAL_S,
                &release,
            ) {
                dispersion_evaluated += 1;
            }
        }
        let dispersion_time = started.elapsed().as_secs_f64();
        assert!(
            dispersion_evaluated > 0,
            "the dispersion channel never actually evaluated: {dispersion_calls} calls to \
             `update` all returned false, so the {dispersion_time:.4} s it was charged is the \
             cost of an EARLY RETURN and not of the model. That is how this block came to \
             report dispersion as 0.00 % of the plant step on 2026-09-27 while a standalone \
             measurement of one evaluation gave 26.8 ms. Check that the release channel here \
             has a source term."
        );

        // --- inside the plant step: the two loops, timed on their own ---
        //
        // Called on the plant in the state the run above left it, with the
        // same arguments `step_with_correctors` passes, and charged at the
        // multiplicity the plant actually uses. Timing one call and
        // multiplying is what makes these shares of the same wall clock as
        // `whole` above -- so the multiplicity has to be right, and it is not
        // the same for all three:
        //
        // - the hot leg and the secondary loop run once per OUTER CORRECTOR;
        // - the STEAM GENERATOR runs once per plant step, on the final
        //   corrector only (step 3b of `step_with_correctors`), because its
        //   three arrays hold spatial history that cannot be rewound.
        let per_corrector = PLANT_OUTER_CORRECTORS.max(1);
        let sg_per_step = 1usize;
        let inner_repeats = 200usize;
        let feedwater_enthalpy = plant.secondary.feedwater_enthalpy();
        let secondary_flow = plant.secondary.mass_flow();
        let core_outlet = plant.primary.core_outlet_temperature();
        let core_outlet_from_bed = plant.core.helium_outlet_enthalpy();
        let duty = plant.primary.steam_generator_duty_to_secondary();

        let started = std::time::Instant::now();
        for _ in 0..inner_repeats {
            plant
                .primary
                .advance_steam_generator(dt, feedwater_enthalpy, secondary_flow);
        }
        let sg_per_call = started.elapsed().as_secs_f64() / inner_repeats as f64;

        let started = std::time::Instant::now();
        for _ in 0..inner_repeats {
            plant.primary.step_hot_duct(dt, core_outlet_from_bed);
        }
        let hot_leg_per_call = started.elapsed().as_secs_f64() / inner_repeats as f64;

        let started = std::time::Instant::now();
        for _ in 0..inner_repeats {
            plant
                .secondary
                .step(dt, commands.secondary, duty, core_outlet);
        }
        let secondary_per_call = started.elapsed().as_secs_f64() / inner_repeats as f64;

        let sg_total = sg_per_call * sg_per_step as f64 * steps as f64;
        let hot_leg_total = hot_leg_per_call * per_corrector as f64 * steps as f64;
        let secondary_total = secondary_per_call * per_corrector as f64 * steps as f64;

        let share = |t: f64| 100.0 * t / whole;
        println!("PLANT STEP COST BREAKDOWN over {plant_seconds} s of plant time");
        println!("  steps taken                  {steps}");
        println!(
            "  WHOLE PLANT                  {whole:.4} s wall, \
             {:.4} s wall per plant second, real-time ratio {:.3}",
            whole / plant_seconds,
            plant_seconds / whole
        );
        println!(
            "  TRISO-ATOPS release channel  {release_time:.4} s wall over {release_calls} \
             calls = {:.2} % of the step",
            share(release_time)
        );
        println!(
            "  Gaussian puff dispersion     {dispersion_time:.4} s wall over \
             {dispersion_evaluated}/{dispersion_calls} calls that EVALUATED = {:.2} % of the \
             step",
            share(dispersion_time)
        );
        println!(
            "  everything else              {:.2} % -- kinetics, pebble bed, primary loop, \
             steam generator, secondary, shaft",
            100.0 - share(release_time) - share(dispersion_time)
        );
        println!("  INSIDE the two loops, at {per_corrector} outer correctors per plant step:");
        println!(
            "    primary: steam generator   {:.4} ms/call x {sg_per_step} x {steps} = {:.2} % \
             (primary_loop::advance_steam_generator)",
            sg_per_call * 1e3,
            share(sg_total)
        );
        println!(
            "    primary: hot leg + core    {:.4} ms/call x {per_corrector} x {steps} = {:.2} % \
             (primary_loop::step_hot_duct)",
            hot_leg_per_call * 1e3,
            share(hot_leg_total)
        );
        println!(
            "    secondary loop (IF97)      {:.4} ms/call x {per_corrector} x {steps} = {:.2} % \
             (secondary_loop::step)",
            secondary_per_call * 1e3,
            share(secondary_total)
        );

        assert!(
            plant.sim_time.get::<second>() > 0.0,
            "the instrument must actually have stepped the plant"
        );
    }

   /// survives its own GUI timestep.
    #[test]
    fn the_whole_plant_steps_at_the_gui_timestep() {
        let mut plant = HtgrPlant::new();
        let mut snapshot = HtgrSnapshot::default();
        // The GUI's plant timestep -- the same constant `crate::app` reads.
        let dt = plant_timestep();
        let plant_seconds = 20.0_f64;
        let steps = (plant_seconds / PLANT_TIMESTEP_S).round() as usize;
        let commands = design_commands();
        let mut worst_metal_k = 0.0_f64;

        let started = std::time::Instant::now();
        for i in 0..steps {
            plant.step(dt, commands);
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
    /// [`PLANT_OUTER_CORRECTORS`]**.
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
    ///    [`plant_timestep`] with 2 and 3 correctors via
    ///    [`HtgrPlant::step_with_correctors`], and the third is compared against
    ///    the second. (This is why that entry point exists:
    ///    [`PLANT_OUTER_CORRECTORS`] is a compile-time constant, so a claim
    ///    that it is converged is unfalsifiable without it.)
    ///
    ///    **The single-corrector arm was dropped on 2026-08-14** at the
    ///    maintainer's instruction. Convergence is established by the *last*
    ///    step of the sweep being small — 3 against 2 — and a one-corrector
    ///    reading says nothing about that. It was a 60 s plant-time run costing
    ///    roughly a fifth of this test for a number nothing asserted on. The
    ///    historical figure is kept in [`PLANT_OUTER_CORRECTORS`]' table.
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
    /// | Reactor power | 12.09893 MW | 12.17475 MW | **+0.6267%** |
    /// | Core outlet | 1138.079 K | 1138.247 K | **+0.168 K** |
    /// | Bed temperature | 887.556 K | 887.703 K | **+0.147 K** |
    /// | Steam outlet | 721.771 K | 720.196 K | **-1.576 K** |
    /// | SG duty | 10.36751 MW | 10.37003 MW | **+0.024%** |
    ///
    /// **Re-measured 2026-08-14**, after the kinetics gained a coolant heat
    /// sink on its fuel node and moved INSIDE this corrector loop. Two things
    /// changed and both are expected:
    ///
    /// - **The transient itself is different.** The old figures were taken
    ///   with an *adiabatic* fuel node, which could only heat and therefore
    ///   drove the reactor spuriously far subcritical -- 0.174 MW at 60 s.
    ///   With a sink the feedback recovers and the plant settles an order of
    ///   magnitude higher. The old numbers were an artefact of the missing
    ///   sink, not a better-resolved answer.
    /// - **Reactor power is no longer exact.** It used to agree to
    ///   **+0.0000%**, and that agreement was *structural*: the kinetics was
    ///   decoupled from everything the plant timestep governed, so both legs
    ///   integrated it identically. Now the fuel node's sink is a genuine
    ///   coupling, carried between correctors like every other, so reactor
    ///   power acquires a real timestep sensitivity: **+0.63%**, inside the 1%
    ///   tolerance. That is the honest cost of coupling the feedback, and it
    ///   is far smaller than the -84.5% the kinetics cost before it was made
    ///   multi-rate.
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
    ///
    /// **Re-measured 2026-09-28** (gh:#360 fuel node, tuas graphite), at 60 s:
    ///
    /// | | pre-change tree (same day) | now |
    /// |---|---|---|
    /// | reactor power, 0.1 s / 1 ms | 17.33672 / 17.34233 MW (**-0.032 %**) | 12.04375 / 12.00028 MW (**+0.362 %**) |
    /// | bed, 0.1 s | 1278.316 K (dT +0.294 K) | 1334.397 K (dT -0.166 K) |
    /// | core outlet, 0.1 s | 1188.217 K (dT +0.320 K) | 1240.142 K (dT -0.159 K) |
    /// | 2 -> 3 correctors | dQ +0.00070 % | dQ +0.00020 % |
    ///
    /// The power sensitivity to the plant step grew (0.03 % -> 0.36 %,
    /// still inside 1 %): the fuel node is now a fast node (`R C` = 0.275 s)
    /// coupled to a bed temperature held over the 0.1 s step, where the old
    /// node was a 9 MJ/K copy of the bed.
    /// # This test does not currently support its own name (`op-21rt`)
    ///
    /// It **runs**, and everything it asserts on **passes** — the 0.1 s and 1 ms
    /// legs agree to 0.27 K on steam. That is the problem, not the reassurance
    /// it looks like: **both legs run at `PLANT_OUTER_CORRECTORS = 2`**, so a
    /// corrector-count error is common-mode between them and cancels exactly. A
    /// timestep-refinement study structurally cannot detect an unconverged
    /// corrector loop.
    ///
    /// The sweep that *would* detect it is **printed and never asserted on**,
    /// which is why a `+29.29 K` move in the steam outlet between 2 and 3
    /// correctors (measured 2026-08-14) passes green.
    ///
    /// It was briefly `#[ignore]`d on 2026-08-14 and then put back the same day:
    /// skipping it did not measurably shorten the suite, so the run-time
    /// argument for hiding it did not survive measurement, and a test that
    /// prints the contradicting number every run is worth more than one that
    /// does not run at all.
    ///
    /// **Until `op-21rt` is resolved, do not cite this test — or
    /// [`PLANT_OUTER_CORRECTORS`]' "converged at 2" table — as evidence of
    /// corrector convergence.** Green here means the timestep is fine, and
    /// nothing more.
    #[test]
    #[ignore = "every htgr_sim_v1 test must finish under 1 minute (maintainer direction, 2026-09-27); measured 2026-09-27 as still running after 20 s in its own process. Settles the whole plant once per corrector count to show convergence -- several whole-plant settles by construction, and the plant runs at only ~4.5x real time."]
    fn the_plant_outer_correctors_converge() {
        let reference = flow_ramp_transient(Time::new::<second>(1.0e-3), PLANT_OUTER_CORRECTORS);
        let shipped = flow_ramp_transient(plant_timestep(), PLANT_OUTER_CORRECTORS);

        // The corrector sweep: how much does each extra corrector move the
        // answer? This is what establishes that PLANT_OUTER_CORRECTORS is
        // converged rather than merely chosen.
        println!("PLANT OUTER-CORRECTOR SWEEP at dt = {PLANT_TIMESTEP_S} s (read at 60 s)");
        let mut previous: Option<TransientEndState> = None;
        // Starts at 2, not 1: see the doc comment. Convergence is the SIZE OF
        // THE LAST STEP, so only 3-against-2 carries the claim.
        for n in 2..=3_usize {
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

    /// V&V: **energy is conserved across the bed -> passive path seam through
    /// a full plant step**, at the plant's own corrector count.
    ///
    /// # Methodology
    ///
    /// One plant step (0.1 s, [`PLANT_OUTER_CORRECTORS`] correctors) from the
    /// design state, with the chain seeded 150 K out of equilibrium so heat
    /// really moves on every pass. The bed is charged the final corrector's
    /// `heat_from_core`; the reflector and vessel must have gained exactly
    /// that less what reached the 50 degC RCCS, `dE = (q_core - q_rccs) dt`,
    /// to 1e-6 of the step's heat.
    ///
    /// **Why 1e-6, not 1e-9.** `stored_energy` is `C T` from 0 K, about
    /// 1.2e11 J, so the difference of two readings has a rounding floor of
    /// `eps C T ~ 3e-5 J`, about 2e-9 of this step's ~1.6e4 J (first run:
    /// the two sides agreed to every printed digit and still missed 1e-9).
    /// 1e-6 sits 3e4 times above that floor and 1e6 times below the defect
    /// it guards, which was the whole step's heat (100 %). A second check repeats it at 1, 2 and 4
    /// correctors, so a missing rewind cannot hide behind the default.
    ///
    /// # Results (2026-09-22)
    ///
    /// Written with the fix. Before it the chain was advanced once per
    /// corrector without a rewind, so at 2 correctors it stored about twice
    /// the heat the bed lost; with it the balance closes to rounding at every
    /// corrector count.
    #[test]
    fn the_passive_path_conserves_energy_through_a_full_plant_step() {
        use uom::si::energy::joule;
        use uom::si::power::watt;
        for n_outer in [1, PLANT_OUTER_CORRECTORS, 4] {
            let mut plant = HtgrPlant::new();
            // Start the chain OUT of equilibrium with the bed (seeded for a
            // bed 150 K hotter), so every corrector pass moves real heat. From
            // the design state the chain is balanced and a missing rewind
            // stores almost nothing extra at 2 correctors -- measured: it then
            // failed only at 4.
            plant.decay_heat_path = decay_heat_removal::CoreToRccsPath::new_at_steady_state(
                ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(
                    plant
                        .core
                        .temperature()
                        .get::<uom::si::thermodynamic_temperature::kelvin>()
                        + 150.0,
                ),
                ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(
                    plant
                        .core
                        .temperature()
                        .get::<uom::si::thermodynamic_temperature::kelvin>()
                        + 150.0,
                ),
                plant.primary.core_inlet_temperature(),
                plant.primary.mass_flow(),
            );
            let dt = Time::new::<second>(0.1);
            let before = plant.decay_heat_path.stored_energy().get::<joule>();
            plant.step_with_correctors(dt, design_commands(), n_outer);
            let after = plant.decay_heat_path.stored_energy().get::<joule>();
            let q_in = plant.decay_heat_path.heat_from_core().get::<watt>();
            // Out: the RCCS and (gh:#397) the riser helium.
            let q_out = plant.decay_heat_path.heat_to_rccs().get::<watt>()
                + plant.decay_heat_path.heat_to_risers().get::<watt>();
            let expected = (q_in - q_out) * dt.get::<second>();
            let scale = (q_in * dt.get::<second>()).abs().max(1.0);
            assert!(
                ((after - before) - expected).abs() / scale < 1e-6,
                "{n_outer} corrector(s): the chain stored {:.6e} J but the bed lost \
                 {:.6e} J net of the RCCS ({q_in:.1} W in, {q_out:.1} W out)",
                after - before,
                expected
            );
        }
    }

    /// V&V: the gh:#351 excursion (rods at 0.30, **+12.97 $**) now runs
    /// **through** 2000 K with a resolved fuel stack instead of dropping it
    /// (gh:#350, gh:#351; 2026-09-28).
    ///
    /// **Methodology.** Open the plant with the rod bank at 0.30 (the setting
    /// gh:#351 recorded reaching ~2589 K bed in 8 s), everything else at the
    /// GUI defaults including the protection system, and step 20 s at the
    /// plant timestep. Record the peak bed, fuel-node, SiC and peak-kernel
    /// temperatures and require (1) the fuel stack is present on every step,
    /// (2) the run completes (a 3000 K window breach would panic, fail-loud),
    /// and (3) the energy chain still closes (per-step residual as in
    /// [`the_core_side_energy_chain_conserves_energy`], 1e-6 of the step's
    /// source).
    ///
    /// **Everything above 2000 K here is EXTRAPOLATED** (graphite k, and every
    /// TRISO layer conductivity; see `NuclearGraphiteMatrixA3HighTemp` and
    /// `tampines::pebble_bed::triso::CorrelationWindow`). This pins that the
    /// model keeps a continuous energy balance and feedback there, not that
    /// the temperatures are validated.
    ///
    /// **Results (2026-09-28).** Peak fission power **2615.6 MW** (the
    /// +12.97 $ burst); fuel node peaks at **2429.9 K**, SiC at **1687.9 K**;
    /// the bed is still rising at 20 s (**1545.5 K**, power 100.7 MW); worst
    /// step energy residual 5.8e-12. The linearly placed **peak kernel reads
    /// 4324.9 K** -- above the 3000 K window and past UO2 melting (~3120 K):
    /// that number is a linear placement on the fuel-bed line
    /// ([`reactor_model::one_node::FuelStackTemperatures::peak_kernel`]), not
    /// a property evaluation, and is **not physical** at this excursion. The
    /// old model (gh:#351) instead put ~2589 K on the *bed* within 8 s because
    /// the whole thermal power went straight into the bed; now the prompt
    /// energy lands in the 0.27 MJ/K fuel node first and reaches the bed
    /// through the pebble conduction resistance.
    #[test]
    fn the_gh351_excursion_runs_through_2000_k_with_a_resolved_fuel_stack() {
        use uom::si::energy::joule;
        use uom::si::power::watt;
        let mut plant = HtgrPlant::new();
        let mut commands = design_commands();
        commands.control_rod_insertion_fraction = 0.30;
        let dt = Time::new::<second>(PLANT_TIMESTEP_S);
        let (mut bed_max, mut fuel_max, mut sic_max, mut peak_max, mut p_max) =
            (0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
        let mut t_at_bed_max = 0.0;
        let mut worst = 0.0f64;
        for i in 0..200 {
            let l0 = plant.kinetics.ledger();
            let r0 = plant.decay_heat_path.stored_energy().get::<joule>();
            plant.step(dt, commands.clone());
            let l1 = plant.kinetics.ledger();
            let r1 = plant.decay_heat_path.stored_energy().get::<joule>();
            let e = plant.core.last_step_energy();
            let source = (l1.deposited_prompt - l0.deposited_prompt)
                + (l1.deposited_decay - l0.deposited_decay);
            let res = source - (l1.stored - l0.stored) - e.solid_storage - e.fluid_storage
                - (r1 - r0)
                - e.throughflow_out
                - (plant.decay_heat_path.heat_to_rccs().get::<watt>()
                    + plant.decay_heat_path.heat_to_risers().get::<watt>())
                    * dt.get::<second>();
            worst = worst.max(res.abs() / source.abs().max(1.0));
            let stack = plant
                .fuel_stack_temperatures()
                .expect("the fuel stack must exist on every step");
            let bed = plant.core.temperature().get::<kelvin>();
            if bed > bed_max {
                bed_max = bed;
                t_at_bed_max = (i + 1) as f64 * PLANT_TIMESTEP_S;
            }
            fuel_max = fuel_max.max(stack.kernel.get::<kelvin>());
            sic_max = sic_max.max(stack.silicon_carbide.get::<kelvin>());
            peak_max = peak_max.max(stack.peak_kernel.get::<kelvin>());
            p_max = p_max.max(plant.kinetics.total_power().get::<watt>());
        }
        println!(
            "gh:#351 excursion (rods 0.30), 20 s: peak power {:.1} MW; peak bed {bed_max:.1} K at \
             {t_at_bed_max:.1} s; peak fuel node {fuel_max:.1} K, SiC {sic_max:.1} K, peak \
             kernel {peak_max:.1} K; at 20 s bed {:.1} K, power {:.3} MW; worst step energy \
             residual {worst:.2e}",
            p_max / 1e6,
            plant.core.temperature().get::<kelvin>(),
            plant.kinetics.total_power().get::<watt>() / 1e6
        );
        assert!(worst < 1e-6);
    }

    /// V&V: **the whole core-side energy chain conserves energy** -- fuel
    /// node, bed (graphite + void helium), reflector and vessel -- across a
    /// normal-operation run and a loss of forced cooling (gh:#360,
    /// 2026-09-28).
    ///
    /// # Methodology
    ///
    /// Control volume: the fuel node, the bed's solid and void-helium nodes,
    /// and the passive path's reflector and RPV nodes. Over 60 s under the
    /// GUI's opening commands and then 60 s of LOFC (circulator tripped,
    /// secondary isolated after 12 s), at the plant's own corrector count,
    /// accumulate every step
    ///
    /// ```text
    /// residual = [f_prompt P + P_decay] dt                    (sources, fuel ledger)
    ///          - dE_fuel - dE_bed,solid - dE_bed,helium - dE_refl - dE_rpv   (storage)
    ///          - m_dot c_p (T_out - T_in) dt - Q_RCCS dt       (losses)
    /// ```
    ///
    /// using each subsystem's own final-corrector bookkeeping (the fuel
    /// ledger, [`reactor_model::one_node::BedStepEnergy`], the passive path's
    /// `stored_energy` and `heat_to_rccs`). Pass: `|sum residual|` below 1e-6
    /// of the gross source energy.
    ///
    /// **It fails on the pre-2026-09-28 model, and by how much.** There the
    /// fuel node was heated by `P + P_decay` and cooled by the bed's
    /// heat-to-helium while the bed was ALSO heated by the thermal power, so
    /// the fuel node's storage `C_f dT_f` was energy counted twice. From the
    /// baseline headless trace of the same working tree (default commands,
    /// before this change), `T_f` rose 950.54 K -> 1307.4 K by t = 1500 s on a
    /// `C_f` of 8.98 MJ/K: a residual of **3.20 GJ**, **14.6 %** of the
    /// ~21.9 GJ of fission energy over that time (trace-integrated at 5 s
    /// samples) -- not a rounding question.
    ///
    /// # Results (2026-09-28)
    ///
    /// Sources 4.9505e9 J; storage: fuel 1.0774e8, bed 3.8771e9,
    /// reflector+RPV 5.9295e7; out: helium 8.7589e8, RCCS 3.0482e7. Net
    /// residual 3.3e-4 J = **6.7e-14** of the source energy (worst single
    /// step 2.0e-10).
    #[test]
    fn the_core_side_energy_chain_conserves_energy() {
        use uom::si::energy::joule;
        use uom::si::power::watt;
        let mut plant = HtgrPlant::new();
        let dt = Time::new::<second>(0.1);
        let dt_s = dt.get::<second>();
        let mut sources = 0.0;
        let mut residual = 0.0;
        let mut worst_step = 0.0f64;
        let mut totals = [0.0f64; 6];
        for i in 0..1200 {
            let mut commands = design_commands();
            if i >= 600 {
                commands.scenario = Scenario::Lofc;
            }
            let ledger0 = plant.kinetics.ledger();
            let refl0 = plant.decay_heat_path.stored_energy().get::<joule>();
            plant.step(dt, commands);
            let ledger1 = plant.kinetics.ledger();
            let refl1 = plant.decay_heat_path.stored_energy().get::<joule>();
            let bed = plant.core.last_step_energy();
            let source = (ledger1.deposited_prompt - ledger0.deposited_prompt)
                + (ledger1.deposited_decay - ledger0.deposited_decay);
            let fuel = ledger1.stored - ledger0.stored;
            let refl = refl1 - refl0;
            // The chain's exits: the RCCS and (gh:#397) the riser helium,
            // which leaves this CV set for the primary loop's cold return.
            let rccs = (plant.decay_heat_path.heat_to_rccs().get::<watt>()
                + plant.decay_heat_path.heat_to_risers().get::<watt>())
                * dt_s;
            let r = source - fuel - bed.solid_storage - bed.fluid_storage - refl
                - bed.throughflow_out
                - rccs;
            sources += source.abs();
            residual += r;
            worst_step = worst_step.max(r.abs() / source.abs().max(1.0));
            for (t, v) in totals.iter_mut().zip([
                source,
                fuel,
                bed.solid_storage + bed.fluid_storage,
                refl,
                bed.throughflow_out,
                rccs,
            ]) {
                *t += v;
            }
        }
        println!(
            "core-side energy chain over 60 s normal + 60 s LOFC: sources {:.6e} J; storage \
             fuel {:.6e}, bed {:.6e}, reflector+RPV {:.6e}; out: helium {:.6e}, RCCS {:.6e}; \
             net residual {residual:.3e} J = {:.3e} of the source energy (worst single step \
             {worst_step:.3e})",
            totals[0],
            totals[1],
            totals[2],
            totals[3],
            totals[4],
            totals[5],
            residual.abs() / sources
        );
        assert!(residual.abs() / sources < 1e-6);
    }

    /// V&V (gh:#394): **the whole plant conserves energy from fission to the
    /// steam generator and the RCCS** -- through normal operation and a
    /// circulator trip -- with every helium CV inside the ledger.
    ///
    /// # Methodology
    ///
    /// [`PlantEnergyLedger`] per plant step, over 60 s from the GUI's opening
    /// commands and then 60 s of LOFC (circulator tripped to the 0.01 kg/s
    /// floor; secondary isolated 12 s later), at the plant's own corrector
    /// count:
    ///
    /// ```text
    /// residual = [f_prompt P + P_decay] dt + W dt
    ///          - dE_fuel - dE_bed,solid - dE_bed,helium - dE_hot duct - dE_cold return
    ///          - dE_refl+RPV - m_dot (h_hot duct - h_SG,out) dt - Q_RCCS dt
    /// ```
    ///
    /// Pass: every step's residual below 1e-9 of that step's gross energy
    /// (the sum of the magnitudes of its terms), and the accumulated residual
    /// below 1e-9 of the accumulated source. Also asserted, per step: the bed
    /// -> hot-duct seam carries one flux (the bed's `throughflow_out` equals
    /// the primary loop's `from_bed` to 1e-12 relative); and after the trip
    /// the cold-return residence time `M_c/m_dot` exceeds 100 s (it was a
    /// fixed 8 s).
    ///
    /// The opening state is a slow transient, not a steady state -- this
    /// model does not settle inside a test budget -- so "normal operation"
    /// here is that transient; the ledger has no term that knows the
    /// difference.
    ///
    /// # Boundary: the steam generator is OUTSIDE this ledger (explicit, revisitable)
    ///
    /// **Decision recorded 2026-09-29, accepted by the maintainer, to be
    /// revisited** (gh:#394). The ledger's boundary on the steam-generator
    /// side is the helium stream, `m_dot (h_hot duct - h_SG,out)`; the
    /// exchanger's three arrays (helium, tube metal, water/steam) and the
    /// secondary cycle are outside it. The reason: those arrays are coupled
    /// explicitly (Lie split), so the exchanger's internal closure
    /// `Q_hot - Q_cold - dE_metal/dt` is `O(dt)` rather than exact -- +0.34 %
    /// at the design point (`primary_loop::steam_generator_substep_s`), i.e.
    /// tens of kW at rated duty. Inside this ledger that known closure would
    /// swamp the seams the ledger exists to check. **To revisit:** make the
    /// exchanger's lateral coupling implicit (or conservative-flux) and move
    /// the boundary to the secondary duty, as the approved plan originally
    /// specified.
    ///
    /// **Why this could not be asserted before 2026-09-29.** The hot leg was
    /// a first-order lag and the return leg an 8 s lag, neither with a mass or
    /// an enthalpy, so the energy between the bed's discharge and what the
    /// exchanger received, and between the exchanger's outlet and the core
    /// inlet, had no storage term to put in a ledger; circulator work was
    /// computed and discarded; and the bed read the previous step's flow while
    /// the exchanger read the new one. `the_core_side_energy_chain_conserves_energy`
    /// therefore stopped at the bed boundary.
    ///
    /// # Results (2026-09-29)
    ///
    /// | Run | accumulated residual | worst step (normal / trip) | bed -> hot-duct seam |
    /// |---|---|---|---|
    /// | stage (a), 2026-09-29 | -4.6e-4 J = **9.2e-14** of 4.95e9 J | 2.4e-11 / 1.2e-10 | 0 |
    /// | stage (b), 2026-09-29 (passive path in the bed's solve) | +1.5e-5 J = **3.1e-15** | 1.1e-11 / 9.0e-11 | 0 |
    /// | stage (c), 2026-09-29 (riser leg: reflector -> cold return) | +5.1e-4 J = **1.0e-13** | 1.2e-11 / 4.4e-11 | 0 |
    ///
    /// Stage (b) totals: source 4.9475e9 J, circulator work 3.155e6 J;
    /// storage fuel 1.078e8, bed graphite 3.876e9, bed helium 5.27e6, hot duct
    /// 2.09e6, cold return 1.99e6, reflector+RPV 5.06e7; out: steam generator
    /// 8.686e8, RCCS 3.80e7. After the trip the cold-return residence time
    /// reaches 1010.6 s at the 0.01 kg/s floor (it was a fixed 8 s).
    #[test]
    fn the_whole_plant_conserves_energy_from_fission_to_the_steam_generator() {
        let mut plant = HtgrPlant::new();
        let dt = Time::new::<second>(PLANT_TIMESTEP_S);
        let dt_s = dt.get::<second>();
        let mut worst_normal = 0.0f64;
        let mut worst_trip = 0.0f64;
        let mut worst_seam = 0.0f64;
        let mut max_residence_after_trip = 0.0f64;
        for i in 0..1200 {
            let mut commands = design_commands();
            if i >= 600 {
                commands.scenario = Scenario::Lofc;
            }
            plant.step(dt, commands);
            let e = plant.last_step_energy();
            let gross = e.source.abs()
                + e.circulator_work.abs()
                + e.fuel_storage.abs()
                + e.bed_solid_storage.abs()
                + e.bed_helium_storage.abs()
                + e.hot_duct_storage.abs()
                + e.cold_return_storage.abs()
                + e.passive_storage.abs()
                + e.to_steam_generator.abs()
                + e.to_rccs.abs();
            let r = e.residual.abs() / gross;
            if i < 600 {
                worst_normal = worst_normal.max(r);
            } else {
                worst_trip = worst_trip.max(r);
                max_residence_after_trip = max_residence_after_trip
                    .max(plant.primary.cold_return_residence_time().get::<second>());
            }
            let bed = plant.core.last_step_energy();
            let primary = plant.primary.last_step_energy();
            worst_seam = worst_seam.max(
                (bed.throughflow_out - primary.from_bed).abs() / bed.throughflow_out.abs().max(1.0),
            );
        }
        let total = plant.energy_ledger();
        println!(
            "WHOLE-PLANT ENERGY LEDGER, 60 s normal + 60 s LOFC (trip at 60 s, secondary \
             isolated at 72 s):\n  source {:.6e} J, circulator work {:.6e} J\n  storage: fuel \
             {:.6e}, bed graphite {:.6e}, bed helium {:.6e}, hot duct {:.6e}, cold return \
             {:.6e}, reflector+RPV {:.6e}\n  out: steam generator {:.6e}, RCCS {:.6e}\n  \
             accumulated residual {:.3e} J = {:.3e} of the source; worst step {:.3e} (normal), \
             {:.3e} (trip); bed->hot-duct seam worst {:.3e}; cold-return residence after trip \
             up to {:.1} s (flow {:.3} kg/s)",
            total.source,
            total.circulator_work,
            total.fuel_storage,
            total.bed_solid_storage,
            total.bed_helium_storage,
            total.hot_duct_storage,
            total.cold_return_storage,
            total.passive_storage,
            total.to_steam_generator,
            total.to_rccs,
            total.residual,
            total.residual.abs() / total.source.abs(),
            worst_normal,
            worst_trip,
            worst_seam,
            max_residence_after_trip,
            plant.primary.mass_flow().get::<kilogram_per_second>(),
        );
        let _ = dt_s;
        assert!(
            worst_normal < 1e-9,
            "normal-operation step residual {worst_normal:e}"
        );
        assert!(worst_trip < 1e-9, "trip step residual {worst_trip:e}");
        assert!(total.residual.abs() / total.source.abs() < 1e-9);
        assert!(worst_seam < 1e-12, "bed -> hot duct seam {worst_seam:e}");
        assert!(
            max_residence_after_trip > 100.0,
            "the cold-return residence time must grow as the flow falls: {max_residence_after_trip} s"
        );
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
                PlantCommands {
                    helium_flow_setpoint: MassRate::new::<kilogram_per_second>(flow_kg_s),
                    ..design_commands()
                },
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
    #[ignore = "every htgr_sim_v1 test must finish under 1 minute (maintainer direction, 2026-09-27); measured 2026-09-27 as still running after 20 s in its own process. Steps the WHOLE plant at the 8.0 kg/s top of the circulator envelope, checking the Courant margin each step; the per-step check is the coverage."]
    fn the_exchanger_holds_its_courant_margin_at_maximum_circulator_flow() {
        let mut plant = HtgrPlant::new();
        let dt = plant_timestep();
        let steps = (100.0 / PLANT_TIMESTEP_S).round() as usize;
        // The circulator ceiling -- `primary_loop::MAX_HELIUM_FLOW_KG_PER_S`.
        let commands = PlantCommands {
            helium_flow_setpoint: MassRate::new::<kilogram_per_second>(8.0),
            ..design_commands()
        };

        for i in 0..steps {
            plant.step(dt, commands);
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

    /// V&V: **no corner of the operator's command envelope can drive a
    /// temperature cross, a Courant breakdown or a property-range excursion.**
    ///
    /// # Why this exists
    ///
    /// The secondary-side controls added on 2026-08-13 gave the operator three
    /// new authorities over the plant -- the feedwater mode and demand, the AUTO
    /// steam-temperature setpoint, and the condenser back-pressure -- and every
    /// one of them can move the steam generator's cold-side boundary condition.
    /// Two of the corners are obviously dangerous a priori:
    ///
    /// - **Minimum feedwater flow** (0.3 kg/s, 9% of nominal) spreads the full
    ///   duty over a twelfth of the design water. That is exactly the regime
    ///   `secondary_loop::max_absorbable_duty` was written for, and before the
    ///   exchanger was resolved it was where the simulator crashed.
    /// - **Maximum feedwater flow** (12 kg/s, 3.5x nominal) raises the cold
    ///   array's advective Courant number in proportion, which is the failure
    ///   mode [`STEAM_GENERATOR_SUBSTEPS_PER_PLANT_STEP`] exists to bound.
    ///
    /// A control that can put the plant somewhere its own second-law assertion
    /// fires is a control whose **range** is wrong. This test is what decides
    /// that, and it is deliberately written so a failure points at the range
    /// rather than at the assertion.
    ///
    /// # Methodology
    ///
    /// Five corners of the envelope, each a fresh plant stepped over 60 s of
    /// simulated time at [`plant_timestep`] with the rods at
    /// [`GUI_INITIAL_ROD_INSERTION`] and the circulator at the published
    /// 4.3 kg/s, so the only thing varying between them is the secondary
    /// command. 60 s is **six feedwater time constants**, so the demand is
    /// genuinely reached rather than merely approached: at 30 s the MANUAL
    /// floor corner was still passing 0.456 kg/s on its way down from the
    /// loop's 3.47 kg/s seed, which is not the corner it claims to test.
    ///
    /// | # | Corner | What it stresses |
    /// |---|---|---|
    /// | 1 | MANUAL at the 0.3 kg/s pump floor | maximum superheat, the crash regime |
    /// | 2 | MANUAL at the 12.0 kg/s pump ceiling | maximum cold-side Courant number |
    /// | 3 | AUTO at the 540 degC setpoint ceiling | controller driven toward its low-flow stop |
    /// | 4 | AUTO at the 260 degC setpoint floor, condenser at its 30 kPa ceiling | controller at its high-flow stop, hottest feedwater |
    /// | 5 | AUTO at the published 440 degC, condenser at its 4 kPa floor | coldest feedwater, deepest expansion |
    ///
    /// Asserted at **every step** of every corner, with the same criteria the
    /// existing whole-plant tests use and no tolerance relaxed:
    ///
    /// - `worst_node_cross_kelvin() <= 1e-6` -- the second law, node by node;
    /// - every GUI-visible snapshot field finite;
    ///
    /// and once per corner:
    ///
    /// - `steam_generator_enthalpy_clamp_events() == 0` -- the fingerprint of a
    ///   Courant breakdown, which is stronger evidence than "it did not panic";
    /// - peak tube metal inside `SteelSS304LHighTemp`'s 1700 K range.
    ///
    /// # Results (measured 2026-08-13)
    ///
    /// **All five corners passed with zero node-by-node crosses and zero
    /// enthalpy clamp events.** Read at 60 s, with the peaks taken over the
    /// whole run:
    ///
    /// | Corner | Feed flow at 60 s | Peak steam | Peak tube metal | Worst cross | Clamps | Tracer `tau` |
    /// |---|---|---|---|---|---|---|
    /// | MANUAL at the pump floor | **0.308 kg/s** | **906.9 K** | **907.4 K** | 0.000000 K | 0 | 159.6 s |
    /// | MANUAL at the pump ceiling | 11.979 kg/s | 816.7 K | 843.9 K | 0.000000 K | 0 | 9.5 s |
    /// | AUTO at the 540 degC ceiling | 2.349 kg/s | 823.3 K | 847.8 K | 0.000000 K | 0 | 21.3 s |
    /// | AUTO at the 260 degC floor, condenser ceiling | 3.394 kg/s | 822.2 K | 846.3 K | 0.000000 K | 0 | 15.3 s |
    /// | AUTO published, condenser floor | 2.792 kg/s | 823.0 K | 847.3 K | 0.000000 K | 0 | 18.2 s |
    ///
    /// **The interesting corner is the first.** At the 0.3 kg/s pump floor the
    /// duty is spread over a twelfth of the design water, and the steam reaches
    /// **906.9 K against a 907.4 K tube metal** -- the exchanger's hot and cold
    /// ends have very nearly met, which is the plant against its own
    /// thermodynamic limit. It resolves that without a cross anywhere and
    /// without the hot array's enthalpy clamp firing once. Before 2026-08-12,
    /// when the exchanger was an effectiveness-NTU lump against an isothermal
    /// sink, this was the regime in which the simulator crashed.
    ///
    /// Peak tube metal over all five corners is 907.4 K against
    /// `SteelSS304LHighTemp`'s 1700 K ceiling -- **793 K of margin** -- so no
    /// operator command comes near the steel property range.
    ///
    /// The feed flows in AUTO are *below* their demands because the rods sit at
    /// the critical insertion with no operator action, so the plant's own
    /// negative temperature feedback walks the power (and hence the duty the
    /// controller is chasing) down over the 60 s. That is the same behaviour
    /// [`the_whole_plant_steps_without_crossing_or_leaving_property_range`]
    /// records, not a control defect.
    ///
    /// The secondary residence time -- the tracer speed -- spans **159.6 s** at
    /// the pump floor to **9.5 s** at the ceiling, a factor of 17, which is the
    /// schematic's flow animation responding across the whole authority the
    /// operator has.
    ///
    /// # Interpretation
    ///
    /// The ranges in [`secondary_loop::ranges`] are therefore *bounded by
    /// something measured*, not merely chosen. This does **not** say the plant
    /// is well-behaved everywhere inside the envelope -- only at its corners,
    /// and only over 30 s. It is a range-validity check, not a validation of the
    /// exchanger.
    #[test]
    #[ignore = "over the 1-minute headless budget (maintainer direction, 2026-09-27): settles or sweeps the WHOLE plant, which runs at ~4.5x real time, so this is minutes to tens of minutes. Run explicitly with --ignored when the transient itself is the subject."]
    fn no_corner_of_the_command_envelope_crosses_or_clamps() {
        use secondary_loop::{ranges, FeedwaterCommand};
        use uom::si::pressure::kilopascal;
        use uom::si::thermodynamic_temperature::degree_celsius;

        let manual = |kg_s: f64| FeedwaterCommand::Manual {
            mass_flow_demand: MassRate::new::<kilogram_per_second>(kg_s),
        };
        let auto = |degc: f64| FeedwaterCommand::Auto {
            target_steam_temperature: ThermodynamicTemperature::new::<degree_celsius>(degc),
        };
        let kpa = |v: f64| uom::si::f64::Pressure::new::<kilopascal>(v);

        let (flow_lo, flow_hi) = ranges::FEEDWATER_FLOW_KG_PER_S;
        let (set_lo, set_hi) = ranges::TARGET_STEAM_TEMPERATURE_C;
        let (cond_lo, cond_hi) = ranges::CONDENSER_PRESSURE_KPA;

        let corners: [(&str, SecondaryCommands); 5] = [
            (
                "MANUAL at the pump floor",
                SecondaryCommands {
                    feedwater: manual(flow_lo),
                    ..SecondaryCommands::default()
                },
            ),
            (
                "MANUAL at the pump ceiling",
                SecondaryCommands {
                    feedwater: manual(flow_hi),
                    ..SecondaryCommands::default()
                },
            ),
            (
                "AUTO at the setpoint ceiling",
                SecondaryCommands {
                    feedwater: auto(set_hi),
                    ..SecondaryCommands::default()
                },
            ),
            (
                "AUTO at the setpoint floor, condenser ceiling",
                SecondaryCommands {
                    feedwater: auto(set_lo),
                    condenser_pressure: kpa(cond_hi),
                },
            ),
            (
                "AUTO published, condenser floor",
                SecondaryCommands {
                    feedwater: FeedwaterCommand::default(),
                    condenser_pressure: kpa(cond_lo),
                },
            ),
        ];

        let dt = plant_timestep();
        let steps = (60.0 / PLANT_TIMESTEP_S).round() as usize;

        println!(
            "COMMAND-ENVELOPE CORNERS (60 s each at {PLANT_TIMESTEP_S} s, rods at \
             {GUI_INITIAL_ROD_INSERTION}, circulator at the published 4.3 kg/s):"
        );
        for (name, secondary) in corners {
            let mut plant = HtgrPlant::new();
            let mut snapshot = HtgrSnapshot::default();
            let commands = PlantCommands {
                secondary,
                ..design_commands()
            };
            let mut worst_cross = 0.0_f64;
            let mut worst_metal_k = 0.0_f64;
            let mut worst_steam_k = 0.0_f64;

            for i in 0..steps {
                plant.step(dt, commands);
                plant.write_snapshot(&mut snapshot);

                let sg = plant.primary.steam_generator_state();
                worst_cross = worst_cross.max(sg.worst_node_cross_kelvin());
                for t in sg.metal_node_temperatures.iter() {
                    worst_metal_k = worst_metal_k.max(t.get::<kelvin>());
                }
                worst_steam_k = worst_steam_k.max(snapshot.sg_steam_outlet_temp_k);

                assert!(
                    sg.worst_node_cross_kelvin() <= 1e-6,
                    "temperature cross of {} K at step {i} with the secondary commanded \
                     '{name}'. This is a statement about the COMMAND RANGE, not about the \
                     assertion: narrow the range in `secondary_loop::ranges` rather than \
                     loosening this.",
                    sg.worst_node_cross_kelvin()
                );
                for (field, v) in [
                    ("core_outlet_temp_k", snapshot.core_outlet_temp_k),
                    ("sg_steam_outlet_temp_k", snapshot.sg_steam_outlet_temp_k),
                    ("ihx_duty_mw", snapshot.ihx_duty_mw),
                    ("turbine_power_mw", snapshot.turbine_power_mw),
                    (
                        "secondary_mass_flow_kg_per_s",
                        snapshot.secondary_mass_flow_kg_per_s,
                    ),
                    (
                        "secondary_residence_time_s",
                        snapshot.secondary_residence_time_s,
                    ),
                    ("condenser_pressure_kpa", snapshot.condenser_pressure_kpa),
                ] {
                    assert!(
                        v.is_finite(),
                        "snapshot field {field} went non-finite at step {i} with '{name}'"
                    );
                }
            }

            let clamps = plant.primary.steam_generator_enthalpy_clamp_events();
            println!(
                "  {name:<46} feed {:.3} kg/s, steam peak {worst_steam_k:.1} K, \
                 SG duty {:.3} MW, metal peak {worst_metal_k:.1} K, \
                 cross {worst_cross:.6} K, clamps {clamps}, tau_2ry {:.2} s",
                snapshot.secondary_mass_flow_kg_per_s,
                snapshot.ihx_duty_mw,
                snapshot.secondary_residence_time_s,
            );

            assert_eq!(
                clamps, 0,
                "the exchanger's hot array clamped its enthalpy field {clamps} times with \
                 the secondary commanded '{name}'. The run is being held together by the \
                 clamp rather than being stable there, so this corner is outside what the \
                 command range may allow."
            );
            assert!(
                worst_metal_k < 1700.0,
                "tube metal reached {worst_metal_k} K with '{name}', outside \
                 SteelSS304LHighTemp's range"
            );
        }
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
    // NOT gated, deliberately: maintainer exception 2026-09-27 -- this is the
    // STABILITY gate, and a stability check that only runs when asked for is a
    // stability check nobody runs. It stays in the default tier whatever it costs.
    fn the_whole_plant_steps_without_crossing_or_leaving_property_range() {
        let mut plant = HtgrPlant::new();
        let mut snapshot = HtgrSnapshot::default();
        let dt = plant_timestep();
        let steps = (150.0 / PLANT_TIMESTEP_S).round() as usize;
        let commands = design_commands();

        let mut worst_cross = 0.0_f64;
        let mut worst_metal_k = 0.0_f64;

        for i in 0..steps {
            plant.step(dt, commands);
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

    /// **Second law: the helium can never leave the core hotter than the
    /// graphite heating it.**
    ///
    /// # Methodology
    ///
    /// The plant is run through the same flow-ramp transient as
    /// [`tests::the_plant_outer_correctors_converge`] -- 4.3 kg/s held, ramped
    /// to 3.0 kg/s, then held -- and `T_core_outlet <= T_bed` is asserted at
    /// **every** step, not just at the end. A ramp is the case that matters:
    /// the violation this guards against appears while the core is cooling
    /// faster than the gas lag follows, so a steady-state check would miss it
    /// entirely.
    ///
    /// # Why this test exists
    ///
    /// This was a real, shipped defect, and it had three independent causes
    /// stacked on each other. Recorded here because each one is a trap that
    /// could be reintroduced separately:
    ///
    /// 1. **An arithmetic-mean driving temperature.** The bed handed over
    ///    `Q = UA (T_bed - T_mean)` with `T_mean = (T_in + T_out)/2`, closed
    ///    downstream by `T_out = T_in + Q/(m c_p)`. Solving the pair gives
    ///    `T_out = [T_in(1 - NTU/2) + NTU T_bed]/(1 + NTU/2)`, which exceeds
    ///    `T_bed` for **NTU > 2**. This exchanger runs at NTU ~ 6.6, so the
    ///    core outlet came out ~196 K above the bed. Fixed by using the exact
    ///    effectiveness-NTU form for an isothermal wall.
    /// 2. **Two modules deriving the same outlet with different `c_p`.** With
    ///    the balance corrected, the bed evaluated `c_p` at the inlet and the
    ///    loop at its bulk mean; a sub-percent disagreement put the outlet
    ///    +2.6 K above the bed again. Fixed by having the bed *publish* the
    ///    outlet its own balance computed, rather than the loop re-deriving it.
    /// 3. **An invented 5 s gas thermal lag.** The helium holdup is about 3 kg
    ///    against 5,280 kg of graphite, so the real lag is under a second. A
    ///    5 s lag let the outlet trail above the bed on a cooldown by ~2.5 K.
    ///    ~~Fixed by deriving the lag from the gas holdup, with
    ///    `bounded_core_outlet` as a hard guard on the remainder.~~
    ///    **CHANGED 2026-09-29 (gh:#391):** the lag and its clamp are
    ///    **deleted**. The bed's LTNE fluid node already is the core helium's
    ///    inertia (the lag counted it twice), and the engine `CLAUDE.md`
    ///    forbids a guard in place of a formulation. The "core outlet" this
    ///    test reads is now the bed fluid node itself, whose implicit row makes
    ///    `T_f'` a weighted combination of `T_f`, `T_s'` and `T_in` -- so the
    ///    invariant must hold **on the formulation alone**.
    ///
    /// # Results (2026-08-14)
    ///
    /// No violation at any step. At the 60 s read point the reference leg
    /// gives `T_out = 905.147 K` against `T_bed = 905.560 K` -- the helium
    /// approaches the graphite closely, as it should at NTU ~ 6.6, without
    /// passing it.
    ///
    /// # Results (2026-09-29, clamp deleted)
    ///
    /// No violation at any step; worst `T_out - T_bed` over the run
    /// **-28.9479 K** (at step 0, from the seeds). Passes with no guard
    /// anywhere between the bed and the steam generator.
    #[test]
    fn the_helium_never_leaves_the_core_hotter_than_the_bed() {
        let mut plant = HtgrPlant::new();
        let dt = plant_timestep();
        let mut worst_excess_k = f64::NEG_INFINITY;
        let mut worst_step = 0usize;

        let steps = (60.0 / PLANT_TIMESTEP_S) as usize;
        for step in 0..steps {
            let t = step as f64 * PLANT_TIMESTEP_S;
            // 4.3 kg/s to 10 s, ramp to 3.0 kg/s over 10-20 s, then hold.
            let flow = if t < 10.0 {
                4.3
            } else if t < 20.0 {
                4.3 + (3.0 - 4.3) * (t - 10.0) / 10.0
            } else {
                3.0
            };
            let mut commands = PlantCommands::default();
            commands.helium_flow_setpoint = MassRate::new::<kilogram_per_second>(flow);
            plant.step(dt, commands);

            let excess = plant.primary.core_outlet_temperature().get::<kelvin>()
                - plant.pebble_temperature().get::<kelvin>();
            if excess > worst_excess_k {
                worst_excess_k = excess;
                worst_step = step;
            }
        }

        println!(
            "worst core-outlet excess over the bed: {worst_excess_k:+.4} K at step {worst_step} \
             ({:.1} s)",
            worst_step as f64 * PLANT_TIMESTEP_S
        );
        assert!(
            worst_excess_k <= 1.0e-9,
            "the helium left the core {worst_excess_k:+.4} K HOTTER than the graphite heating \
             it, at step {worst_step}. This is a second-law violation -- see this test's docs \
             for the three separate causes that produced it before."
        );
    }

    /// **V&V for GitHub issue #22 ("htgr sim v1 non-physical data"): the
    /// PebbleBedPorousMediaNode's own 2x2 implicit solve stays correctly
    /// ordered (helium never leaves hotter than the bed) through a full
    /// scram and extended cooldown, not just the short flow-ramp
    /// [`the_helium_never_leaves_the_core_hotter_than_the_bed`] already
    /// covers.**
    ///
    /// # Methodology
    ///
    /// Full scram (`control_rod_insertion_fraction = 1.0`) held from `t=0`,
    /// circulator held at nominal flow (the issue's own repro steps: "scram
    /// reactor and run cooling"), everything else at
    /// [`PlantCommands::default`]. Run 30 simulated minutes and track
    /// `T_f(bed fluid) - T_s(bed solid)` at every step -- this is the
    /// invariant [`PebbleBedPorousMediaNode::step`]'s own doc comment flags
    /// as "not proven bounded by construction" for the two-phase implicit
    /// solve (as opposed to the removed effectiveness-NTU `PebbleBedCore`,
    /// which was exactly bounded). Pass criterion: the worst excess stays
    /// at or below zero, same tolerance as the existing NTU test.
    ///
    /// `#[ignore]`d: ~400 s real time at this crate's measured near-1:1
    /// compute-per-plant-second (see `app` module doc comment) -- run
    /// explicitly, not as part of the default suite, same convention as
    /// TUAS's long natural-circulation tests.
    ///
    /// # Results (2026-08-17)
    ///
    /// No inversion at any of the 18,000 steps. Worst (least negative)
    /// `T_f - T_s` excess was **-5.4252 K at t=1799.9 s** -- the fluid node
    /// stayed measurably below the bed throughout, including through the
    /// steepest part of the transient (t=0 to ~600 s, where `T_s` fell from
    /// 949.9 K to 562.3 K and `T_in` fell even faster, 521.2 K to 370.5 K,
    /// because the default feedwater is MANUAL at a fixed 10.0 kg/s -- see
    /// [`SecondaryCommands`]'s default -- which does not scale down with the
    /// collapsing primary duty and so pulls the loop toward the feedwater
    /// temperature rather than holding the published 250 degC cold leg).
    ///
    /// # Interpretation
    ///
    /// The coupled two-phase balance itself is NOT the source of issue #22's
    /// "totally wrong energy balance, helium out hotter than fuel" report.
    /// That symptom traced instead to a GUI wiring bug: the schematic's
    /// "T_fuel" tag and the reactor-vessel colour were reading
    /// [`HtgrSnapshot::fuel_temperature_k`] (the kinetics reactivity-feedback
    /// node) instead of [`HtgrSnapshot::bed_temperature_k`] (this test's
    /// `T_s`) -- see
    /// [`kinetics_fuel_node_tracks_the_bed_node_after_a_scram`] for
    /// why those two diverge, and `app::schematic`/`app::panels` for the fix.
    /// The very low absolute temperatures this test also shows (well below
    /// the published 250 degC cold leg) are a separate, real observation --
    /// the fixed 10.0 kg/s MANUAL feedwater default overcooling a scrammed
    /// core -- but are not by themselves a second-law violation.
    #[test]
    #[ignore]
    fn reproduce_issue_22_scram_cooldown_energy_balance() {
        let mut plant = HtgrPlant::new();
        let dt = plant_timestep();

        let mut commands = PlantCommands::default();
        commands.control_rod_insertion_fraction = 1.0; // full scram, held
                                                       // Keep the circulator at nominal flow -- the issue's own repro steps
                                                       // say "scram reactor and run cooling", i.e. forced cooling stays on.

        let sim_minutes = 30.0;
        let steps = (sim_minutes * 60.0 / PLANT_TIMESTEP_S) as usize;
        let mut worst_inversion_k = f64::NEG_INFINITY;
        let mut worst_inversion_t = 0.0;

        for step in 0..steps {
            plant.step(dt, commands);

            let t_s = plant.pebble_temperature().get::<kelvin>();
            let t_f = plant.primary.core_outlet_temperature().get::<kelvin>();
            let t_out = plant.primary.core_outlet_temperature().get::<kelvin>();
            let t_in = plant.primary.core_inlet_temperature().get::<kelvin>();
            let inversion = t_f - t_s;
            if inversion > worst_inversion_k {
                worst_inversion_k = inversion;
                worst_inversion_t = step as f64 * PLANT_TIMESTEP_S;
            }

            if step % (600 * 10) == 0 {
                // every 600 s = 10 sim-minutes
                println!(
                    "t={:>7.1} s  T_s(bed)={:>8.3} K  T_f(bed fluid)={:>8.3} K  \
                     T_in(core inlet)={:>8.3} K  T_out(loop, lagged)={:>8.3} K  \
                     decay_heat={:>10.1} W  flow={:>6.3} kg/s",
                    step as f64 * PLANT_TIMESTEP_S,
                    t_s,
                    t_f,
                    t_in,
                    t_out,
                    plant
                        .kinetics
                        .decay_heat_power()
                        .get::<uom::si::power::watt>(),
                    plant.primary.mass_flow().get::<kilogram_per_second>(),
                );
            }
        }

        println!(
            "worst T_f - T_s inversion over {sim_minutes} sim-minutes: {worst_inversion_k:+.4} K \
             at t={worst_inversion_t:.1} s"
        );
        assert!(
            worst_inversion_k <= 1.0e-9,
            "the bed fluid node left the core {worst_inversion_k:+.4} K HOTTER than the bed \
             solid node at t={worst_inversion_t:.1} s during a scram+cooldown -- a second-law \
             violation in PebbleBedPorousMediaNode's own 2x2 solve. See this test's docs."
        );
    }

    /// **V&V for GitHub issue #22: [`HtgrKinetics::fuel_temperature`] (the
    /// Nordheim-Fuchs reactivity-feedback node) tracks
    /// [`HtgrPlant::pebble_temperature`] (the bed's own solid-phase node)
    /// within a fraction of a kelvin through a scram, instead of drifting
    /// apart without bound.**
    ///
    /// # Methodology
    ///
    /// Full scram held from `t=0`, nominal flow, otherwise
    /// [`PlantCommands::default`]. Every 30 s over 5 simulated minutes,
    /// compare [`HtgrKinetics::fuel_temperature`] against
    /// [`HtgrPlant::pebble_temperature`]. The two are separate state: this
    /// node's own thermal balance is driven by
    /// [`HtgrKinetics::apply_decay_heat`] (source) and
    /// [`HtgrKinetics::apply_coolant_heat_removal`] (sink, `core_heat_to_helium`
    /// -- the same rate the real bed sees). Before [`HtgrKinetics::apply_decay_heat`]
    /// existed, this node received only the sink and NOT the matching
    /// decay-heat source, so after a scram (prompt fission collapsing toward
    /// zero while the sink, driven by the bed's decay heat, did not) it
    /// drifted increasingly colder than the bed -- this test is what caught
    /// that. Pass criterion: the gap stays within a small band around zero
    /// (residual second-order effects only -- see Interpretation), not the
    /// growing-without-bound behaviour the fix removed.
    ///
    /// # Results
    ///
    /// **Before the fix (2026-08-17, pre-`apply_decay_heat`):**
    ///
    /// | t (s) | kinetics.fuel_temperature (K) | bed.pebble_temperature (K) | diff (K) |
    /// |---|---|---|---|
    /// | 0 | 950.000 | 950.000 | 0.000 |
    /// | 30 | 922.004 | 923.527 | -1.523 |
    /// | 120 | 838.863 | 843.731 | -4.868 |
    /// | 300 | 701.454 | 711.695 | -10.241 |
    ///
    /// **After the fix (2026-08-17, with `apply_decay_heat`):**
    ///
    /// | t (s) | kinetics.fuel_temperature (K) | bed.pebble_temperature (K) | diff (K) |
    /// |---|---|---|---|
    /// | 0 | 950.000 | 950.000 | 0.000 |
    /// | 30 | 923.554 | 923.527 | +0.027 |
    /// | 120 | 843.765 | 843.731 | +0.034 |
    /// | 300 | 711.734 | 711.695 | +0.039 |
    ///
    /// # Interpretation
    ///
    /// The fix (adding the missing decay-heat source term, symmetric with
    /// the existing coolant-removal sink) reduced the 5-minute-post-scram
    /// gap from -10.241 K to +0.039 K -- a ~99.6% reduction, and it stays
    /// small and roughly flat rather than continuing to grow. This is what
    /// made a scrammed core look like it was violating the second law on
    /// screen before this fix: the schematic's "T_fuel" tag and reactor-vessel
    /// colour read this node (see `app::schematic`, `app::panels`, now
    /// pointed at `bed_temperature_k` instead as a second, independent fix),
    /// so a growing negative gap against a helium temperature read as an
    /// impossible ordering. The remaining +0.03-0.04 K residual is expected,
    /// not a defect: this node integrates explicitly at
    /// [`super::KINETICS_SUBSTEP_S`] resolution against a one-step-stale
    /// coolant sink, while the bed's own [`super::pebble_bed::PebbleBedPorousMediaNode`]
    /// integrates implicitly at the plant timestep -- small, bounded
    /// discretisation differences, not an energy-accounting gap.
    ///
    /// # Re-measured 2026-09-28 (gh:#360 fuel-node rework)
    ///
    /// The fuel node is now the TRISO particles (0.27 MJ/K), heated by
    /// `f_prompt P + P_decay` and cooled only by conduction to the bed, so it
    /// opens `R P` = 10.25 K above the bed and, after the scram, sits
    /// `R P_decay` above it -- a **positive offset by construction**, not a
    /// residual:
    ///
    /// | t (s) | fuel (K) | bed (K) | diff (K) | pre-change tree, same day |
    /// |---|---|---|---|---|
    /// | 0 | 960.250 | 950.000 | +10.250 | 0.000 |
    /// | 30 | 924.061 | 923.463 | +0.598 | +0.772 |
    /// | 120 | 841.953 | 841.496 | +0.457 | +2.438 |
    /// | 300 | 701.599 | 701.279 | **+0.320** | **+3.799** |
    ///
    /// The pre-change column (run on a snapshot of the working tree before
    /// this change) shows the old node had already drifted from the +0.039 K
    /// recorded 2026-08-17 to +3.80 K by 300 s; the new offset matches
    /// `R_fb x P_decay` (1.03e-6 K/W x ~0.25 MW = 0.26 K plus the bed's own
    /// lag) and shrinks with the decay heat. The bed cools ~10 K further by
    /// 300 s than before (701.3 vs 709.3 K) because the passive loss and the
    /// helium now draw on the bed alone rather than on a bed plus a shadow
    /// copy.
    #[test]
    #[ignore]
    fn kinetics_fuel_node_tracks_the_bed_node_after_a_scram() {
        let mut plant = HtgrPlant::new();
        let dt = plant_timestep();

        let mut commands = PlantCommands::default();
        commands.control_rod_insertion_fraction = 1.0; // full scram, held

        let sim_minutes = 5.0;
        let steps = (sim_minutes * 60.0 / PLANT_TIMESTEP_S) as usize;
        let mut final_diff_k = 0.0;

        for step in 0..=steps {
            if step > 0 {
                plant.step(dt, commands);
            }
            let diff_k = plant.kinetics.fuel_temperature().get::<kelvin>()
                - plant.pebble_temperature().get::<kelvin>();
            final_diff_k = diff_k;
            if step % 300 == 0 {
                // every 30 s
                println!(
                    "t={:>6.1} s  kinetics.fuel_temperature={:>8.3} K  \
                     bed.pebble_temperature={:>8.3} K  diff={:>8.3} K  \
                     core_heat_to_helium={:>10.1} W  decay_heat={:>10.1} W",
                    step as f64 * PLANT_TIMESTEP_S,
                    plant.kinetics.fuel_temperature().get::<kelvin>(),
                    plant.pebble_temperature().get::<kelvin>(),
                    diff_k,
                    plant.core_heat_to_helium.get::<uom::si::power::watt>(),
                    plant
                        .kinetics
                        .decay_heat_power()
                        .get::<uom::si::power::watt>(),
                );
            }
        }

        assert!(
            (-5.0..=5.0).contains(&final_diff_k),
            "kinetics.fuel_temperature diverged from bed.pebble_temperature by \
             {final_diff_k:+.3} K after {sim_minutes} sim-minutes post-scram, outside the \
             [-5, 5] K band the apply_decay_heat fix holds it in (measured +0.039 K on \
             2026-08-17) -- something has reopened GitHub issue #22's energy-accounting gap. \
             See this test's docs."
        );
    }

    /// One row of a loss-of-forced-cooling trace.
    #[derive(Clone, Copy, Debug)]
    pub struct LofcSample {
        pub time_s: f64,
        pub fission_power_w: f64,
        pub decay_power_w: f64,
        pub fuel_temperature_k: f64,
        pub bed_temperature_k: f64,
    }

    /// Run the HTR-10 helium-circulator-trip ATWS scenario and return the trace.
    ///
    /// **Scenario, from Hu et al. (2006) section 3 and Chen et al. (2009)
    /// section 4.** The reactor is at the test's initial condition; at `t = 0`
    /// the helium circulator is tripped. **No control rod moves** -- that is
    /// what makes it an anticipated transient *without scram*, and it is the
    /// whole point of the test: the reactor must shut itself down on the
    /// negative temperature coefficient alone. The secondary circuit is
    /// isolated and the blower baffle closed 12 s after initiation, after which
    /// the primary flow is "almost zero".
    ///
    /// The protection system is left disarmed, because arming it would insert
    /// the rods and destroy the very thing under test.
    /// Feedwater demand scaled to a part-load helium flow.
    ///
    /// **Not cosmetic.** The steam generator is a counter-flow exchanger with a
    /// real tube-metal state: run it at 30 % helium flow while the feed pump is
    /// still delivering its full-power demand and the tube metal is driven down
    /// toward the feedwater temperature, out of the bottom of the SS304L
    /// property correlation at 300 K, and the run aborts. A plant at part load
    /// moves both sides down together, so the scenario does too.
    fn scaled_feedwater(helium_flow_kg_s: f64) -> secondary_loop::FeedwaterCommand {
        use uom::si::mass_rate::kilogram_per_second;
        let fraction = helium_flow_kg_s / pebble_bed::nominal_helium_flow_kg_per_s();
        // Default MANUAL demand is 10.0 kg/s at full helium flow.
        let demand = (10.0 * fraction).max(1.0);
        secondary_loop::FeedwaterCommand::Manual {
            mass_flow_demand: MassRate::new::<kilogram_per_second>(demand),
        }
    }

    /// Settle the plant at a given rod position and flow, and report the power
    /// it lands on. Used to search for the test's initial condition.
    fn settled_power_mw(rod_insertion: f64, flow_kg_s: f64, settle_s: f64, xenon_on: bool) -> f64 {
        use uom::si::mass_rate::kilogram_per_second;
        let dt = Time::new::<second>(PLANT_TIMESTEP_S);
        let mut plant = HtgrPlant::new();
        plant.protection.set_enabled(false);
        let mut c = PlantCommands::default();
        // Xenon must be present during the SEARCH, not added afterwards.
        // Seeding it after settling adds a dollar of negative reactivity the
        // rods never balanced, so the transient would start subcritical and
        // the two arms would not be comparable.
        if xenon_on {
            plant
                .kinetics
                .enable_xenon_at_equilibrium(plant.kinetics.total_power());
        }
        c.control_rod_insertion_fraction = rod_insertion;
        c.helium_flow_setpoint = MassRate::new::<kilogram_per_second>(flow_kg_s);
        c.secondary.feedwater = scaled_feedwater(flow_kg_s);
        let steps = (settle_s / PLANT_TIMESTEP_S).round() as usize;
        for _ in 0..steps {
            plant.step(dt, c.clone());
        }
        plant.kinetics.total_power().get::<megawatt>()
    }

    /// Find the rod insertion that settles the plant at `target_mw` for a given
    /// helium flow, by bisection.
    ///
    /// Deeper insertion means more negative external reactivity and therefore
    /// less power, so settled power is monotonically decreasing in insertion
    /// and bisection is well posed.
    fn rod_position_for(target_mw: f64, flow_kg_s: f64, settle_s: f64, xenon_on: bool) -> f64 {
        // Bracket DEEPER than the simulator's opening position only.
        //
        // The opening insertion is already near critical at full flow and
        // settles around 9 MW; any target below that is reached by inserting
        // further. Searching shallower is not merely wasteful, it is
        // destructive: with the protection system disabled (as it must be for
        // an ATWS scenario) a shallow bank is strongly supercritical, the power
        // runs away, and the secondary superheats past the 2273.15 K top of
        // IAPWS-IF97 Region 5, which aborts the run before the search can
        // converge.
        let shallowest = PlantCommands::default().control_rod_insertion_fraction;
        let (mut lo, mut hi) = (shallowest, 0.95_f64); // lo = shallower = more power
        for _ in 0..14 {
            let mid = 0.5 * (lo + hi);
            if settled_power_mw(mid, flow_kg_s, settle_s, xenon_on) > target_mw {
                lo = mid; // still too hot: insert deeper
            } else {
                hi = mid;
            }
        }
        0.5 * (lo + hi)
    }

    fn run_lofc_atws(duration_s: f64, settle_s: f64) -> Vec<LofcSample> {
        run_lofc_atws_at(duration_s, settle_s, None)
    }

    /// As [`run_lofc_atws_at`], with the Xe-135 channel switched on or off.
    ///
    /// Xenon is seeded at equilibrium for the settled power, because the real
    /// test was run on a core that had been operating and therefore had a
    /// saturated xenon inventory when the circulator tripped.
    fn run_lofc_atws_xenon(
        duration_s: f64,
        settle_s: f64,
        initial_condition: Option<(f64, f64)>,
        xenon_on: bool,
    ) -> Vec<LofcSample> {
        use uom::si::mass_rate::kilogram_per_second;

        let dt = Time::new::<second>(PLANT_TIMESTEP_S);
        let mut plant = HtgrPlant::new();
        plant.protection.set_enabled(false);

        let mut steady = PlantCommands::default();
        if let Some((rod, flow)) = initial_condition {
            steady.control_rod_insertion_fraction = rod;
            steady.helium_flow_setpoint = MassRate::new::<kilogram_per_second>(flow);
            steady.secondary.feedwater = scaled_feedwater(flow);
        }
        if xenon_on {
            plant
                .kinetics
                .enable_xenon_at_equilibrium(plant.kinetics.total_power());
        }
        let settle_steps = (settle_s / PLANT_TIMESTEP_S).round() as usize;
        for _ in 0..settle_steps {
            plant.step(dt, steady.clone());
        }

        // Xenon is seeded BEFORE settling (below), so by here the plant has
        // already reached a critical steady state WITH it present.
        let _ = xenon_on;

        let rod_position = steady.control_rod_insertion_fraction;
        let initial_fission = plant.kinetics.total_power().get::<watt>();

        plant.primary.trip_circulator(true);
        let mut tripped = steady.clone();
        tripped.control_rod_insertion_fraction = rod_position;
        tripped.helium_flow_setpoint = MassRate::new::<kilogram_per_second>(0.0);

        const SECONDARY_ISOLATION_TIME_S: f64 = 12.0;
        let isolation_step = (SECONDARY_ISOLATION_TIME_S / PLANT_TIMESTEP_S).round() as usize;

        let steps = (duration_s / PLANT_TIMESTEP_S).round() as usize;
        let sample_every = (1.0 / PLANT_TIMESTEP_S).round() as usize;
        let mut trace = Vec::with_capacity(steps / sample_every + 1);
        trace.push(LofcSample {
            time_s: 0.0,
            fission_power_w: initial_fission,
            decay_power_w: plant.kinetics.decay.total_decay_heat_power().get::<watt>(),
            fuel_temperature_k: plant.kinetics.prompt.fuel_temperature.get::<kelvin>(),
            bed_temperature_k: plant.core.temperature().get::<kelvin>(),
        });

        for i in 1..=steps {
            if i == isolation_step {
                plant.primary.isolate_secondary(true);
            }
            plant.step(dt, tripped.clone());
            if i % sample_every == 0 {
                trace.push(LofcSample {
                    time_s: i as f64 * PLANT_TIMESTEP_S,
                    fission_power_w: plant.kinetics.total_power().get::<watt>(),
                    decay_power_w: plant.kinetics.decay.total_decay_heat_power().get::<watt>(),
                    fuel_temperature_k: plant.kinetics.prompt.fuel_temperature.get::<kelvin>(),
                    bed_temperature_k: plant.core.temperature().get::<kelvin>(),
                });
            }
        }
        trace
    }

    /// Reduce a LOFC trace to the CRP-5 benchmark parameters of interest.
    ///
    /// Returns `(shutdown_to_1pct_s, recriticality_s, peak_fraction, peak_time_s)`.
    /// "Recriticality" is taken as the first sample after the power minimum at
    /// which the fission power has risen by 10 % above that minimum -- a
    /// threshold crossing rather than a true `k = 1` test, which this model
    /// cannot evaluate directly.
    fn lofc_metrics(trace: &[LofcSample]) -> (Option<f64>, Option<f64>, f64, f64) {
        let p0 = trace[0].fission_power_w;
        let shutdown = trace
            .iter()
            .find(|s| s.fission_power_w <= 0.01 * p0)
            .map(|s| s.time_s);

        let min_idx = trace
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.fission_power_w.total_cmp(&b.1.fission_power_w))
            .map(|(i, _)| i)
            .unwrap_or(0);
        let p_min = trace[min_idx].fission_power_w;

        let recrit = trace[min_idx..]
            .iter()
            .find(|s| s.fission_power_w > 1.10 * p_min.max(f64::MIN_POSITIVE))
            .map(|s| s.time_s);

        let peak = trace[min_idx..]
            .iter()
            .max_by(|a, b| a.fission_power_w.total_cmp(&b.fission_power_w))
            .copied()
            .unwrap_or(trace[0]);

        (shutdown, recrit, peak.fission_power_w / p0, peak.time_s)
    }

    /// **HTR-10 LOFC ATWS, with and without Xe-135 poisoning.**
    ///
    /// ## Why this pair exists
    ///
    /// Chen et al. (2009) attribute THERMIX's over-prediction of the
    /// recriticality peak -- **32.6 %** calculated against **24.7 % measured**
    /// -- to having **omitted xenon feedback**: xenon builds while the reactor
    /// is subcritical and adds negative reactivity, so leaving it out makes the
    /// return to power both earlier and stronger. Jun et al. (2009), whose
    /// GAMMA+ model *does* carry a xenon channel, land on **25 % at 4200 s**
    /// against the measured 25 % at 4400 s.
    ///
    /// So xenon is worth roughly **7 percentage points of peak power** in this
    /// transient, according to two independent codes. This test runs the same
    /// scenario twice, differing only in whether the xenon channel is active,
    /// and reports the four CRP-5 parameters for each.
    ///
    /// ## What this does and does not establish
    ///
    /// The **sign and the mechanism** are physics: the published I-135/Xe-135
    /// yields and decay constants set how fast xenon accumulates after
    /// shutdown, and accumulating a neutron absorber must delay and depress the
    /// return to power.
    ///
    /// The **magnitude is a model input**, set by
    /// [`kinetics::EQUILIBRIUM_XENON_WORTH_DOLLARS`], because converting a
    /// xenon inventory to reactivity needs HTR-10 absorption cross sections
    /// this workspace does not have. Treat the difference between the two runs
    /// as a **sensitivity study on that constant**, not as a prediction. See
    /// [`kinetics::XenonChannel`] for the full statement, including that
    /// omitting xenon burnout biases this channel toward over-stating xenon.
    ///
    /// **Results: printed by this test.**
    #[test]
    #[ignore = "over the 1-minute headless budget (maintainer direction, 2026-09-27): settles or sweeps the WHOLE plant, which runs at ~4.5x real time, so this is minutes to tens of minutes. Run explicitly with --ignored when the transient itself is the subject."]
    fn lofc_atws_with_and_without_xenon() {
        let flow_30pct = 0.30 * pebble_bed::nominal_helium_flow_kg_per_s();
        let settle_s = 200.0;
        // Search PER ARM. Each case must open critical at the test power in
        // its own right: with xenon present the rods sit shallower to offset
        // it. Sharing one rod position would start the xenon arm a dollar
        // subcritical and the comparison would measure that, not xenon.
        let rod_no_xe = rod_position_for(3.315, flow_30pct, settle_s, false);
        let rod_xe = rod_position_for(3.315, flow_30pct, settle_s, true);

        // 3 h, the window over which the test data reports recriticality and
        // the first power peak.
        let duration = 10_800.0;

        let without = run_lofc_atws_xenon(duration, settle_s, Some((rod_no_xe, flow_30pct)), false);
        let with = run_lofc_atws_xenon(duration, settle_s, Some((rod_xe, flow_30pct)), true);

        let (s_no, r_no, pk_no, pkt_no) = lofc_metrics(&without);
        let (s_yes, r_yes, pk_yes, pkt_yes) = lofc_metrics(&with);

        let fmt = |o: Option<f64>| match o {
            Some(t) => format!("{t:.0} s"),
            None => "not reached".to_string(),
        };

        println!("\n=== HTR-10 LOFC ATWS: Xe-135 on vs off ===");
        println!(
            "initial power        : {:.4} MW (test: 3.315 MW)",
            without[0].fission_power_w / 1.0e6
        );
        println!("rod insertion        : {rod_no_xe:.6} (no Xe), {rod_xe:.6} (with Xe), helium flow {flow_30pct:.3} kg/s");
        println!(
            "equilibrium Xe worth : {} $ (MODEL INPUT, not derived)",
            kinetics::EQUILIBRIUM_XENON_WORTH_DOLLARS
        );
        println!();
        println!(
            "{:<22} {:>14} {:>14}   {}",
            "parameter", "without Xe", "with Xe", "measured / published"
        );
        println!("{:-<80}", "");
        println!(
            "{:<22} {:>14} {:>14}   {}",
            "shutdown to 1 %",
            fmt(s_no),
            fmt(s_yes),
            "330 s [Chen]"
        );
        println!(
            "{:<22} {:>14} {:>14}   {}",
            "recriticality",
            fmt(r_no),
            fmt(r_yes),
            "~3000 s [Hu], 2900 s [GAMMA+]"
        );
        println!(
            "{:<22} {:>13.2}% {:>13.2}%   {}",
            "first peak",
            100.0 * pk_no,
            100.0 * pk_yes,
            "24.7 % [Chen test], 32.6 % [THERMIX, no Xe]"
        );
        println!(
            "{:<22} {:>14.0} {:>14.0}   {}",
            "peak time", pkt_no, pkt_yes, "4400 s test, 4200 s GAMMA+"
        );
        println!();
        println!(
            "xenon effect on peak : {:+.2} percentage points  (published expectation: about -7)",
            100.0 * (pk_yes - pk_no)
        );

        // The mechanism must have the right SIGN: adding a neutron absorber
        // cannot raise the return to power. The magnitude is not gated, since
        // it is a function of a model input.
        assert!(
            pk_yes <= pk_no + 1.0e-9,
            "xenon RAISED the recriticality peak ({:.3} % with, {:.3} % without). Xe-135 is a \
             neutron absorber accumulating while the reactor is subcritical; it can only \
             depress the return to power. This is a sign error, not a calibration issue.",
            100.0 * pk_yes,
            100.0 * pk_no
        );
    }

    /// As [`run_lofc_atws`], optionally starting from a specified
    /// `(rod_insertion, helium_flow_kg_s)` initial condition instead of the
    /// simulator's default opening commands.
    fn run_lofc_atws_at(
        duration_s: f64,
        settle_s: f64,
        initial_condition: Option<(f64, f64)>,
    ) -> Vec<LofcSample> {
        run_lofc_atws_ablated(duration_s, settle_s, initial_condition, None)
    }

    /// As [`run_lofc_atws_at`], with the kernel Doppler channel's fuel share
    /// overridden.
    ///
    /// `None` runs the shipped model; `Some(0.0)` reproduces the
    /// pre-2026-09-22 model exactly, which is what makes this the attribution
    /// tool for any LOFC change. See
    /// `KernelDopplerChannel::with_fuel_share` (removed 2026-09-28, gh:#360).
    fn run_lofc_atws_ablated(
        duration_s: f64,
        settle_s: f64,
        initial_condition: Option<(f64, f64)>,
        fuel_share: Option<f64>,
    ) -> Vec<LofcSample> {
        use uom::si::mass_rate::kilogram_per_second;

        let dt = Time::new::<second>(PLANT_TIMESTEP_S);
        let mut plant = HtgrPlant::new();
        plant.protection.set_enabled(false);
        if let Some(share) = fuel_share {
            plant.kinetics.set_fuel_share(share);
        }

        // Hold the opening commands while the plant settles, so the transient
        // is not launched on top of the startup excursion.
        let mut steady = PlantCommands::default();
        if let Some((rod, flow)) = initial_condition {
            steady.control_rod_insertion_fraction = rod;
            steady.helium_flow_setpoint = MassRate::new::<kilogram_per_second>(flow);
            steady.secondary.feedwater = scaled_feedwater(flow);
        }
        let settle_steps = (settle_s / PLANT_TIMESTEP_S).round() as usize;
        for _ in 0..settle_steps {
            plant.step(dt, steady.clone());
        }

        let rod_position = steady.control_rod_insertion_fraction;
        let initial_fission = plant.kinetics.total_power().get::<watt>();

        // t = 0: trip the circulator. Rods stay exactly where they were.
        plant.primary.trip_circulator(true);
        let mut tripped = steady.clone();
        tripped.control_rod_insertion_fraction = rod_position;
        tripped.helium_flow_setpoint = MassRate::new::<kilogram_per_second>(0.0);

        let steps = (duration_s / PLANT_TIMESTEP_S).round() as usize;
        let sample_every = (1.0 / PLANT_TIMESTEP_S).round() as usize; // 1 Hz
        let mut trace = Vec::with_capacity(steps / sample_every + 1);
        trace.push(LofcSample {
            time_s: 0.0,
            fission_power_w: initial_fission,
            decay_power_w: plant.kinetics.decay.total_decay_heat_power().get::<watt>(),
            fuel_temperature_k: plant.kinetics.prompt.fuel_temperature.get::<kelvin>(),
            bed_temperature_k: plant.core.temperature().get::<kelvin>(),
        });

        // The protection system isolated the secondary circuit and closed the
        // blower baffle 12 s after initiation (Hu et al. 2006 section 3).
        const SECONDARY_ISOLATION_TIME_S: f64 = 12.0;
        let isolation_step = (SECONDARY_ISOLATION_TIME_S / PLANT_TIMESTEP_S).round() as usize;

        for i in 1..=steps {
            if i == isolation_step {
                plant.primary.isolate_secondary(true);
            }
            plant.step(dt, tripped.clone());
            if i % sample_every == 0 {
                trace.push(LofcSample {
                    time_s: i as f64 * PLANT_TIMESTEP_S,
                    fission_power_w: plant.kinetics.total_power().get::<watt>(),
                    decay_power_w: plant.kinetics.decay.total_decay_heat_power().get::<watt>(),
                    fuel_temperature_k: plant.kinetics.prompt.fuel_temperature.get::<kelvin>(),
                    bed_temperature_k: plant.core.temperature().get::<kelvin>(),
                });
            }
        }
        trace
    }

    /// **ABLATION: what the fuel share of the isothermal coefficient is worth
    /// in a LOFC ATWS** (rewritten 2026-09-28, gh:#360).
    ///
    /// ~~What the kernel Doppler channel is worth ... `fuel_share = 0`
    /// reproduces the pre-2026-09-22 model exactly~~ -- that channel is gone
    /// (the fuel node is the kernel now), and so is the model `f = 0` used to
    /// reproduce. The 2026-09-22 table is kept below as the record of the old
    /// model; it is **not** comparable to the new one.
    ///
    /// **Methodology.** [`run_lofc_atws_ablated`] (same 200 s settle at the
    /// shipped opening commands, circulator trip, 12 s secondary isolation,
    /// 600 s) at fuel shares 0.25, the shipped 0.7117 and 1.0. Reported:
    /// settled `p0`, time to 1 % of `p0`, minimum fraction, peak fuel
    /// temperature.
    ///
    /// **Why not `f -> 0` any more.** The shipped opening rod position is
    /// worth **+7.05 $** (`report_the_rod_position_that_holds_three_megawatts`),
    /// a prompt-supercritical insertion. With the fuel node a real 0.27 MJ/K
    /// node, a vanishing fuel coefficient leaves that burst arrested only by
    /// the slow bed channel, and the fuel-to-bed conduction reaches ~36 kW per
    /// pebble within a second -- measured 2026-09-28 at `f = 1e-3`: the
    /// resolved pebble would need a **35 K** surface to carry it, and the run
    /// stops (fail-loud, no fallback). That is the model correctly refusing an
    /// unphysical configuration, not a defect to route around, so the ablation
    /// range starts at 0.25.
    ///
    /// # Results -- old model, 2026-09-22 (record only)
    ///
    /// | fuel share | settled `p0` | 1 % reached | min fraction | peak fuel |
    /// |---|---|---|---|---|
    /// | 0.0 (pre-2026-09-22) | 0.0896 MW | 383 s | 0.00109 | 1318.2 K |
    /// | shipped (0.7117) | 3.3652 MW | NOT REACHED | 0.13795 | 1213.7 K |
    ///
    /// (Re-run on the pre-change working tree on 2026-09-28, the shipped row
    /// read p0 13.3768 MW, NOT REACHED, min fraction 0.01672, peak fuel
    /// 1346.3 K -- the plant had moved since 2026-09-22 with the rod change to
    /// 0.45, gh:#318.)
    ///
    /// # Results -- new model (2026-09-28)
    ///
    /// | fuel share | settled `p0` | 1 % reached | min fraction | peak fuel | end (600 s) fission / fuel / bed |
    /// |---|---|---|---|---|---|
    /// | 0.25 | 16.2011 MW | NOT REACHED | 0.02789 | 1341.5 K | 1.7389 MW / 1315.4 K / 1313.0 K |
    /// | shipped (0.7117) | 16.1826 MW | NOT REACHED | 0.03608 | 1341.5 K | 1.8809 MW / 1319.0 K / 1316.3 K |
    /// | 1.0 | 16.0504 MW | NOT REACHED | 0.04208 | 1341.6 K | 1.9459 MW / 1321.1 K / 1318.4 K |
    ///
    /// **Interpretation.** The share is now a **weak** lever on this
    /// transient: across 0.25-1.0 the minimum fraction moves 2.8-4.2 % and the
    /// end state by ~5 K, because with the fuel a real node that tracks the bed
    /// within `R P` (~20 K at 16 MW, ~2 K at 2 MW) the two channels see almost
    /// the same temperature change over a slow LOFC; the split matters for the
    /// *prompt* response, not for where a 600 s transient goes. More fuel
    /// share -> slightly *less* deep a dip, since the fuel cools toward the bed
    /// as power falls (a positive insertion on the fuel channel). The plant
    /// does not reach 1 % at any share -- see
    /// [`lofc_atws_reactor_shuts_itself_down`].
    ///
    /// **Asserted:** only that the ablation is real (the settled power or the
    /// minimum fraction moves across the range). Whether the plant shuts down
    /// is [`lofc_atws_reactor_shuts_itself_down`]'s to report.
    #[test]
    #[ignore = "over the 1-minute headless budget (maintainer direction, 2026-09-27): settles or sweeps the WHOLE plant, which runs at ~4.5x real time, so this is minutes to tens of minutes. Run explicitly with --ignored when the transient itself is the subject."]
    fn the_kernel_doppler_channel_is_ablated_on_the_lofc_transient() {
        let mut rows = Vec::new();
        for share in [Some(0.25), None, Some(1.0)] {
            let trace = run_lofc_atws_ablated(600.0, 200.0, None, share);
            let p0 = trace[0].fission_power_w;
            let one_percent = trace
                .iter()
                .find(|s| s.fission_power_w <= 0.01 * p0)
                .map(|s| s.time_s);
            let min_frac = trace
                .iter()
                .map(|s| s.fission_power_w / p0)
                .fold(f64::INFINITY, f64::min);
            let peak_fuel = trace
                .iter()
                .map(|s| s.fuel_temperature_k)
                .fold(f64::NEG_INFINITY, f64::max);
            let last = trace.last().expect("non-empty");
            println!(
                "fuel_share {:<8}: p0 {:.4} MW, 1% at {}, min fraction {:.5}, peak fuel {:.1} K, \
                 end fission {:.4} MW, end fuel {:.1} K, end bed {:.1} K",
                match share {
                    Some(f) => format!("{f:.4}"),
                    None => "shipped".to_string(),
                },
                p0 / 1.0e6,
                match one_percent {
                    Some(t) => format!("{t:.0} s"),
                    None => "NOT REACHED".to_string(),
                },
                min_frac,
                peak_fuel,
                last.fission_power_w / 1.0e6,
                last.fuel_temperature_k,
                last.bed_temperature_k,
            );
            rows.push((p0, min_frac));
        }
        let moved = (rows[0].0 - rows[2].0).abs() / rows[1].0 > 1e-3
            || (rows[0].1 - rows[2].1).abs() > 1e-3;
        assert!(moved, "the fuel-share ablation must change the transient; rows {rows:?}");
    }

    /// **HTR-10 loss-of-forced-cooling ATWS: does the reactor shut itself down?**
    ///
    /// This is the first of the four CRP-5 benchmark parameters of interest
    /// (ICONE22-30088): **shutdown time**. Chen et al. (2009) section 5 report
    /// that both the measured and the THERMIX fission power fall **from 100 %
    /// to 1 % of the initial value within 330 s**.
    ///
    /// **This is a scoping measurement, not a validation.** The model is one
    /// lumped bed node with no natural circulation, no reflector/barrel/cavity
    /// path and no xenon, against a test whose reference analysis used a 2-D
    /// r-z conduction model with 44 material regions including the RCCS. The
    /// assertion is therefore only that the reactor **does** shut itself down
    /// on temperature feedback alone, which is the qualitative claim the test
    /// exists to demonstrate. The measured time is printed for comparison, not
    /// gated.
    ///
    /// # Results
    ///
    /// | run | settled `p0` | 1 % reached | min fraction | peak fuel | at 600 s: fission / fuel / bed |
    /// |---|---|---|---|---|---|
    /// | pre-change tree (2026-09-28) | 13.3768 MW | NOT REACHED | 0.0167 | 1346.3 K | 1.5551 MW / 1323.9 K / 1224.4 K |
    /// | **after gh:#360 fuel node + tuas graphite (2026-09-28)** | 16.1826 MW | **NOT REACHED** | 0.0361 | 1341.5 K | 1.8809 MW / 1319.0 K / 1316.3 K |
    /// | **helium CVs on enthalpy, circulator work (2026-09-29, gh:#388)** | 16.1364 MW | **NOT REACHED** | -- | 1341.9 K | 1.8741 MW / 1319.0 K / 1316.4 K |
    /// | **passive path in the bed's solve, Achenbach legs, derived capacities (2026-09-29, gh:#395/#396)** | 16.1468 MW | **NOT REACHED** | -- | 1343.7 K | 1.6827 MW / 1320.7 K / 1318.3 K |
    /// | **riser leg (2026-09-29, gh:#397)** | 16.1377 MW | **NOT REACHED** | -- | 1343.5 K | 1.6886 MW / 1320.8 K / 1318.4 K |
    ///
    /// **Stage (b), 2026-09-29:** fission at 600 s fell 10 % (1.8741 ->
    /// 1.6827 MW), but the outcome does not change. The reflector capacity is
    /// now derived (1.11e8 J/K, where the invented value was 1.8e8), so the
    /// reflector heats faster and the passive path saturates sooner.
    ///
    /// **2026-09-29:** re-measured before (`a79755763b`, reproducing the
    /// 2026-09-28 row exactly) and after the primary-circuit change. The
    /// outcome does not move: the return-leg residence time now grows to
    /// ~1000 s after the trip instead of staying at 8 s, but with the
    /// secondary isolated at 12 s the helium path carries almost nothing
    /// either way, and what decides this transient is the passive path and the
    /// reactivity reference (gh:#389, #387).
    ///
    /// **Still failing, and the failure is reported, not silenced.** The
    /// predicted consequence of removing the ~-4.5 $ of spurious feedback
    /// (gh:#360) held: the plant settles hotter and higher (bed ~1303 K at
    /// 16.2 MW, where the old model had a fuel node 100+ K above a drifting
    /// bed) and the LOFC dip is shallower (3.6 % against 1.7 %). The old
    /// model's "fuel 1323.9 K / bed 1224.4 K" at 600 s was the shadow node's
    /// 100 K drift; the new pair differs by `R P` (2.7 K). The shutdown is
    /// being tested from a +7.05 $ opening that settles at 16 MW and 1300 K,
    /// not from HTR-10's 3 MW test condition (gh:#318) -- see
    /// [`lofc_atws_at_the_published_test_condition`].
    #[test]
    #[ignore = "over the 1-minute headless budget (maintainer direction, 2026-09-27): settles or sweeps the WHOLE plant, which runs at ~4.5x real time, so this is minutes to tens of minutes. Run explicitly with --ignored when the transient itself is the subject."]
    fn lofc_atws_reactor_shuts_itself_down() {
        let trace = run_lofc_atws(600.0, 200.0);
        let p0 = trace[0].fission_power_w;
        assert!(p0 > 0.0, "initial fission power must be positive");

        let one_percent = trace
            .iter()
            .find(|s| s.fission_power_w <= 0.01 * p0)
            .map(|s| s.time_s);
        let peak_fuel = trace
            .iter()
            .map(|s| s.fuel_temperature_k)
            .fold(f64::NEG_INFINITY, f64::max);
        let last = trace.last().expect("trace is non-empty");

        println!("\n=== HTR-10 LOFC ATWS (circulator trip, no scram) ===");
        println!("initial fission power     : {:.4} MW", p0 / 1.0e6);
        match one_percent {
            Some(t) => println!("time to 1 % of initial    : {t:.0} s   (measured: 330 s)"),
            None => println!(
                "time to 1 % of initial    : NOT REACHED within {:.0} s",
                last.time_s
            ),
        }
        println!(
            "peak fuel temperature     : {:.1} K ({:.1} degC)",
            peak_fuel,
            peak_fuel - 273.15
        );
        println!(
            "at t = {:.0} s: fission {:.4} MW, decay {:.4} MW, fuel {:.1} K, bed {:.1} K",
            last.time_s,
            last.fission_power_w / 1.0e6,
            last.decay_power_w / 1.0e6,
            last.fuel_temperature_k,
            last.bed_temperature_k
        );

        assert!(
            one_percent.is_some(),
            "the reactor did not shut itself down: fission power never fell to 1 % of its \
             initial value within {:.0} s. The negative temperature coefficient is the only \
             mechanism acting here, so this failing means the feedback is not arresting the \
             transient at all.",
            last.time_s
        );
    }

    /// **HTR-10 LOFC ATWS at the published test initial condition.**
    ///
    /// The test was run at **30 % of rated power** (Hu et al. 2006 section 1):
    /// 3000 kW, 2.5 MPa, core inlet 212-215 degC, outlet 650 degC. The
    /// simulator's default opening commands sit near 9 MW, so a transient
    /// launched from them starts with roughly three times the stored energy and
    /// a different temperature margin, and its shutdown time is not comparable
    /// to the measured one.
    ///
    /// This test therefore searches for the rod position that settles the plant
    /// at 3 MW with the helium flow scaled to 30 % of the published 4.3 kg/s,
    /// and runs the transient from there.
    ///
    /// **Still not a validation.** One lumped bed node, no natural circulation
    /// (which the real test established once the baffle closed), no xenon, no
    /// reflector/barrel/cavity path. Reported for comparison, not gated.
    ///
    /// **Results: printed by this test.**
    #[test]
    #[ignore = "over the 1-minute headless budget (maintainer direction, 2026-09-27): settles or sweeps the WHOLE plant, which runs at ~4.5x real time, so this is minutes to tens of minutes. Run explicitly with --ignored when the transient itself is the subject."]
    fn lofc_atws_at_the_published_test_condition() {
        let flow_30pct = 0.30 * pebble_bed::nominal_helium_flow_kg_per_s();
        let settle_s = 200.0;
        let rod = rod_position_for(3.0, flow_30pct, settle_s, false);
        let settled = settled_power_mw(rod, flow_30pct, settle_s, false);

        println!("\n=== initial condition search ===");
        println!("helium flow (30 % of rated): {flow_30pct:.3} kg/s");
        println!("rod insertion found        : {rod:.6}");
        println!("settled power              : {settled:.4} MW   (target 3.0 MW)");

        let trace = run_lofc_atws_at(3600.0, settle_s, Some((rod, flow_30pct)));
        let p0 = trace[0].fission_power_w;
        let one_percent = trace
            .iter()
            .find(|s| s.fission_power_w <= 0.01 * p0)
            .map(|s| s.time_s);

        // First post-shutdown power peak: the first local maximum after the
        // power has bottomed out. Scan from the minimum onward.
        let min_idx = trace
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.fission_power_w.total_cmp(&b.1.fission_power_w))
            .map(|(i, _)| i)
            .unwrap_or(0);
        let (peak_idx, peak) = trace[min_idx..]
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.fission_power_w.total_cmp(&b.1.fission_power_w))
            .map(|(i, s)| (i + min_idx, *s))
            .unwrap_or((0, trace[0]));

        let peak_fuel = trace
            .iter()
            .map(|s| s.fuel_temperature_k)
            .fold(f64::NEG_INFINITY, f64::max);
        let last = trace.last().expect("non-empty");

        println!("\n=== HTR-10 LOFC ATWS at the test condition ===");
        println!(
            "initial fission power : {:.4} MW   (test: 3.0 MW)",
            p0 / 1.0e6
        );
        match one_percent {
            Some(t) => println!("shutdown to 1 %       : {t:.0} s          (test: 330 s)"),
            None => println!("shutdown to 1 %       : NOT REACHED"),
        }
        println!(
            "power minimum at      : {:.0} s, {:.5} MW",
            trace[min_idx].time_s,
            trace[min_idx].fission_power_w / 1.0e6
        );
        println!(
            "first peak after min  : {:.3} % of initial at {:.0} s   (test: 24.7 % at 4400 s)",
            100.0 * peak.fission_power_w / p0,
            peak.time_s
        );
        let _ = peak_idx;
        println!(
            "peak fuel temperature : {:.1} K ({:.1} degC)   (test analysis: 823-883 degC, limit 1230)",
            peak_fuel,
            peak_fuel - 273.15
        );
        println!(
            "at t = {:.0} s        : fission {:.5} MW, decay {:.5} MW, fuel {:.1} K, bed {:.1} K",
            last.time_s,
            last.fission_power_w / 1.0e6,
            last.decay_power_w / 1.0e6,
            last.fuel_temperature_k,
            last.bed_temperature_k
        );

        assert!(
            one_percent.is_some(),
            "the reactor did not shut itself down at the published test condition"
        );
    }

    /// **Where the bank must sit for the plant to settle at the HTR-10 LOFC
    /// test's 3 MWth initial condition**, measured headlessly.
    ///
    /// # Methodology
    ///
    /// Three positions are reported and they are NOT the same quantity:
    ///
    /// 1. **Cold clean critical** -- `control_rods::critical_insertion_fraction`,
    ///    a bisection on the Lamarsh S-curve against the published bank worth
    ///    (15.24 %dk/k, B31) and unrodded `k = 1.119747` (B21). No feedback, no
    ///    temperature, no xenon.
    /// 2. **The simulator's opening position**, `PlantCommands::default()`.
    /// 3. **The 3 MW operating position**, bisected on settled power at 30 %
    ///    helium flow -- a part-load flow this test sets explicitly, NOT the
    ///    simulator's opening flow (which became rated 4.3 kg/s on
    ///    2026-09-27). It differs from (1) because at 3 MW the bed sits far
    ///    from the design-point temperature, so the feedback the bank must
    ///    offset is different.
    ///
    /// **NOTE 2026-09-27:** (2) and (3) are therefore no longer comparable as
    /// they were. The opening position is a demonstration starting point at
    /// rated flow; the 3 MW position is the deferred validation condition at
    /// part load. This test remains the way to re-derive (3).
    ///
    /// Reference: the HTR-10 loss-of-forced-cooling ATWS safety demonstration
    /// of 15 Oct 2003 began near 3 MWth, about 30 % of the 10 MWth rating.
    ///
    /// **Caveat carried from `control_rods`:** the worth MAGNITUDE is published
    /// HTR-10 data, but the worth SHAPE is the generic Lamarsh cosine-flux
    /// S-curve, and HTR-10's rods sit in the side reflector rather than the
    /// core. The published B31 spread is 13.06-16.56 %dk/k across codes, so no
    /// position here is meaningful to better than roughly a quarter.
    /// **Re-measured 2026-09-28 (gh:#360 fuel node, tuas graphite):** the
    /// insertion holding 3 MWth at 1.290 kg/s moved **0.627597 -> 0.674014**
    /// (settled 3.0001 MW; external reactivity -0.9436 $ -> **-2.6649 $**).
    /// With the spurious ~-4.5 $ fuel-node drift gone, more rod is needed to
    /// hold the same power. Opening position still worth +7.0548 $.
    #[test]
    #[ignore = "over the 1-minute headless budget (maintainer direction, 2026-09-27): settles or sweeps the WHOLE plant, which runs at ~4.5x real time, so this is minutes to tens of minutes. Run explicitly with --ignored when the transient itself is the subject."]
    fn report_the_rod_position_that_holds_three_megawatts() {
        use uom::si::ratio::ratio as ratio_unit;

        let beta = HtgrKinetics::new_htr10_published(nominal_thermal_power())
            .delayed_neutron_fraction()
            .get::<ratio_unit>();

        let cold_clean = control_rods::critical_insertion_fraction(beta)
            .expect("the bank must be able to hold down the cold clean excess");
        let opening = PlantCommands::default().control_rod_insertion_fraction;

        let flow_30pct = 0.30 * nominal_helium_flow().get::<kilogram_per_second>();
        let settle_s = 1200.0;

        let rod_3mw = rod_position_for(3.0, flow_30pct, settle_s, false);
        let got = settled_power_mw(rod_3mw, flow_30pct, settle_s, false);

        println!("\n=== HTR-10 3 MWth initial condition: where the bank sits ===");
        println!("beta_eff                      : {beta:.6}");
        println!(
            "cold clean critical insertion : {cold_clean:.6}  (S-curve vs published bank worth)"
        );
        println!("simulator opening position    : {opening:.6}");
        println!("insertion holding 3 MWth      : {rod_3mw:.6}  at {flow_30pct:.3} kg/s helium");
        println!("  -> settled power            : {got:.4} MW   (target 3.0000 MW)");
        println!(
            "  -> external reactivity      : {:.4} $  (opening position: {:.4} $)",
            control_rods::external_reactivity_dollars(rod_3mw, beta),
            control_rods::external_reactivity_dollars(opening, beta),
        );

        assert!(
            (got - 3.0).abs() < 0.15,
            "the bisection must land within 0.15 MW of 3 MWth, got {got:.4} MW at \
             insertion {rod_3mw:.6}"
        );
        assert!(
            (0.0..=1.0).contains(&rod_3mw),
            "insertion must be physical, got {rod_3mw}"
        );
    }

    /// Settle the plant at a rod position and flow, then report the state it
    /// reaches -- power AND the two temperatures -- sampled along the way so a
    /// reader can see whether it actually converged or is still ringing.
    ///
    /// Returns `(power_mw, bed_k, fuel_k)` at the end of the settle.
    fn settled_state(rod_insertion: f64, flow_kg_s: f64, settle_s: f64) -> (f64, f64, f64) {
        use uom::si::mass_rate::kilogram_per_second;
        use uom::si::thermodynamic_temperature::kelvin as kelvin_unit;
        let dt = Time::new::<second>(PLANT_TIMESTEP_S);
        let mut plant = HtgrPlant::new();
        plant.protection.set_enabled(false);
        let mut c = PlantCommands::default();
        c.control_rod_insertion_fraction = rod_insertion;
        c.helium_flow_setpoint = MassRate::new::<kilogram_per_second>(flow_kg_s);
        c.secondary.feedwater = scaled_feedwater(flow_kg_s);
        let steps = (settle_s / PLANT_TIMESTEP_S).round() as usize;
        let mark = steps / 8;
        for i in 0..steps {
            plant.step(dt, c.clone());
            if mark > 0 && i % mark == 0 {
                println!(
                    "    t = {:7.1} s   P = {:9.4} MW   bed = {:8.3} K   fuel = {:8.3} K",
                    (i as f64) * PLANT_TIMESTEP_S,
                    plant.kinetics.total_power().get::<megawatt>(),
                    plant.core.temperature().get::<kelvin_unit>(),
                    plant.kinetics.fuel_temperature().get::<kelvin_unit>(),
                );
            }
        }
        (
            plant.kinetics.total_power().get::<megawatt>(),
            plant.core.temperature().get::<kelvin_unit>(),
            plant.kinetics.fuel_temperature().get::<kelvin_unit>(),
        )
    }

    /// **The steady bed temperature at the opening state**, which is the
    /// number the bed must be SEEDED at.
    ///
    /// ~~The simulator opens its bed at the rated-power design point
    /// (949.95 K), but [`GUI_INITIAL_ROD_INSERTION`] holds 3 MWth at 30 %
    /// flow, which sits far cooler.~~ **CORRECTED 2026-09-27**: the opening
    /// state is no longer the 3 MWth one. `GUI_INITIAL_ROD_INSERTION` is 0.30
    /// (gh:#318) and [`GUI_INITIAL_HELIUM_FLOW_KG_PER_S`] is 1.00, i.e. full
    /// 4.3 kg/s; neither was bisected against the other, so the settled power
    /// and bed temperature here are **unmeasured** and this test is the way to
    /// measure them. The seeding argument below still applies whichever
    /// direction the mismatch runs. Starting 300 K too hot makes the temperature feedback shut
    /// the reactor down outright -- a headless run on 2026-09-17 fell to
    /// 7e-9 MW by 900 s, then rang between 0.58 and 5.93 MW for the next
    /// 900 s. That is the model relaxing an initial condition nobody chose,
    /// not plant behaviour.
    ///
    /// # Results (2026-09-28, both at the shipped 0.45 rod / 4.3 kg/s, 6000 s)
    ///
    /// | model | power | bed | fuel node | fuel - bed |
    /// |---|---|---|---|---|
    /// | pre-change working tree | 5.9956 MW | 740.36 K | 1318.43 K | **578.1 K** |
    /// | **gh:#360 fuel node + tuas graphite** | **16.1212 MW** | **1303.39 K** | 1323.36 K | **19.97 K** (= `R P`) |
    /// | **helium CVs on enthalpy, circulator work (2026-09-29, gh:#388)** | **16.0742 MW** | **1303.43 K** | 1323.34 K | 19.91 K |
    /// | **passive path in the bed's implicit solve, Achenbach legs, derived capacities (2026-09-29, gh:#395/#396)** | **16.0693 MW** | **1303.43 K** | 1323.34 K | 19.91 K |
    /// | **riser leg (2026-09-29, gh:#397)** | **15.8803 MW** | **1303.60 K** | 1323.28 K | 19.68 K |
    ///
    /// **Riser leg:** -1.18 % in power. The side reflector now hands about
    /// 0.33 MW to the helium rising through its channels, which re-enters the
    /// core ~15 K warmer (core inlet 522.2 -> 537.4 K at 300 s in the
    /// baseline), so the same feedback balance holds at a lower power.
    ///
    /// **2026-09-29:** -0.29 % in power, +0.04 K in the bed. The circulator's
    /// 52 kW now reaches the helium instead of being discarded, so the same
    /// feedback balance needs ~47 kW less fission power. Before/after taken
    /// the same day on the same build otherwise (before: `a79755763b`).
    ///
    /// The old "steady state" was not one: the shadow fuel node was pinned
    /// near 1318 K by the reactivity balance while the bed kept cooling (the
    /// gh:#360 drift), so the -4.5 $-and-growing spurious feedback held power
    /// at 6 MW. With it gone the plant settles 2.7x higher in power and 563 K
    /// hotter in the bed, as gh:#360 predicted in sign. The opening rod is
    /// worth +7.05 $ (gh:#318), so this is a hot plant by construction. The
    /// bed to seed at is now **1303.39 K**; the seed has not been changed here
    /// (the maintainer owns the opening condition).
    #[test]
    #[ignore = "over the 1-minute headless budget (maintainer direction, 2026-09-27): settles or sweeps the WHOLE plant, which runs at ~4.5x real time, so this is minutes to tens of minutes. Run explicitly with --ignored when the transient itself is the subject."]
    fn report_the_steady_state_bed_temperature_at_the_opening_condition() {
        let flow =
            GUI_INITIAL_HELIUM_FLOW_KG_PER_S * nominal_helium_flow().get::<kilogram_per_second>();
        println!(
            "\n=== settling at insertion {GUI_INITIAL_ROD_INSERTION}, flow {flow:.3} kg/s ==="
        );
        let (p, bed, fuel) = settled_state(GUI_INITIAL_ROD_INSERTION, flow, 6000.0);
        println!("\nSTEADY STATE after 6000 s:");
        println!("  power {p:.4} MW   bed {bed:.4} K   fuel {fuel:.4} K");
        println!("  (seed the bed at {bed:.4} K)");
        assert!(p.is_finite() && bed.is_finite());
    }
}
