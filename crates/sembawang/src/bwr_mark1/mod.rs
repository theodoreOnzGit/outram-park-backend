//! The plant: Fukushima Daiichi Unit 1 after the earthquake's scram.
//!
//! A BWR/3 in a Mark I containment. This is the model the ~~`dhoby-ghaut`~~
//! `outram-park-digital-twin-engine` (since 2026-11-05)
//! `fukushima_web` demo draws and steps; it moved here from that example on
//! 2026-10-29 (gh:#1033, maintainer: "Physics libraries go to sembawang").
//! Where it stands against the BSAF Unit 1 bands: [`compare`].
//!
//! **Transcendentals go through PETIR** (`petir::mathf::RealMath`: `r_exp`,
//! `r_ln`, `r_powf`, `r_powi`, `r_cos`; `sqrt` stays, being correctly rounded
//! by IEEE-754), as everywhere in this crate (gh:#985), so native and wasm
//! runs take one route. Converted 2026-10-29; the measured effect is in
//! `verification_and_validation/bwr_mark1/petir-conversion.md`.
//!
//! Three parts, owned and advanced separately so they can run on three
//! threads (three Web Workers in the browser), as the CIET Educational
//! Simulator's browser demo runs its heat transfer and its two flow solves:
//!
//! | part | owns | runs on |
//! |---|---|---|
//! | [`Plant`] (this module) | the reactor pressure vessel's water and steam, the isolation condensers, the relief valves, injection, decay heat, the operators' sequence | the plant worker, which also paces the run |
//! | [`core::Core`] | the fuel and cladding temperatures, oxidation, hydrogen, relocation | the core worker |
//! | [`PlantContainment`]: ~~[`containment::Containment`]~~ **since 2026-11-02 (gh:#1099)** the control-volume network [`containment_network::ContainmentNetwork`], the lumped [`containment::Containment`] under `Mechanisms::without_network_containment` | drywell, wetwell, pool, vent, reactor building | the containment worker, which holds it between steps ([`ContainmentWorker`]) |
//!
//! Each step, [`Plant::begin`] reads the vessel at the start of the step and
//! hands each worker a **snapshot**; the core and the containment advance
//! from it; [`Plant::finish`] takes their results and closes the vessel's
//! mass and energy balance. The coupling is explicit across the step (each
//! part sees the others' start-of-step state), the same splitting CIET v2
//! uses for its flows. Because the snapshots and results cross as exact
//! `f64`s ([`wire`]), the three-worker step is bit-for-bit the one-worker
//! step ([`Plant::step_inline`]); `tests::three_workers_are_one_worker`
//! holds that. (Since gh:#1099 the network's state has no wire form, so the
//! containment worker keeps its containment and only the snapshot and the
//! result cross; the core still crosses with its whole state.)
//!
//! **Units 2 and 3 (gh:#1109).** [`Plant::for_unit`] builds Unit 2 or 3
//! from [`Params::unit2`]/[`Params::unit3`] and its own containment network;
//! in place of the isolation condensers it runs RCIC and HPCI
//! ([`turbine_pump`]) and, under [`Mode::Historical`], the unit's cited
//! sequence ([`units23`]). Every existing constructor is Unit 1, unchanged.
//!
//! **AI draft, not validated.** See [`params`] for where every number comes
//! from (many are placeholders) and each module's "Not in the model" list.
//! Research, education and V&V only: never for operation, licensing, safety
//! decisions or emergency response.

pub mod cavity;
pub mod compare;
pub mod containment;
pub mod containment_fp;
pub mod containment_network;
pub mod core;
pub mod decay;
pub mod fdi;
pub mod fp_decay;
pub mod gas;
pub mod internals;
pub mod msl;
pub mod params;
pub mod pool_surface;
pub mod reaction;
pub mod source;
pub mod sparc;
pub mod steam;
pub mod turbine_pump;
pub mod units23;

// Deprecated aliases (gh:#1335, 2026-11-11): these five modules are the
// MELCOR clean-room equations, moved unchanged to
// `outram_park_fork_offbeat::melcor`. The old paths
// (`sembawang::bwr_mark1::aerosol::Medium` and the rest) keep working for
// one release through these plain re-exports; new code should name the
// offbeat paths. The plant assembly that calls them stays here.
/// Deprecated alias of [`outram_park_fork_offbeat::melcor::aerosol`] (gh:#1335).
pub use outram_park_fork_offbeat::melcor::aerosol;
/// Deprecated alias of [`outram_park_fork_offbeat::melcor::iodine`] (gh:#1335).
pub use outram_park_fork_offbeat::melcor::iodine;
/// Deprecated alias of [`outram_park_fork_offbeat::melcor::rn_volume`] (gh:#1335).
pub use outram_park_fork_offbeat::melcor::rn_volume;
/// Deprecated alias of [`outram_park_fork_offbeat::melcor::vapour`] (gh:#1335).
pub use outram_park_fork_offbeat::melcor::vapour;
/// Deprecated alias of [`outram_park_fork_offbeat::melcor::wire`] (gh:#1335).
pub use outram_park_fork_offbeat::melcor::wire;

pub use units23::Unit;

use petir::mathf::RealMath;
use containment::{Containment, ContainmentResult, ContainmentSnapshot};
use self::core::{Core, CoreResult, CoreSnapshot};
use tampines_steam_tables::prelude::get_choked_flow_massrate_and_state_from_stagnation_properties_and_area;
use self::decay::{DecayParts, PlantDecay};
use params::*;
use steam::{Sat, SatTable};
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{Area, AvailableEnergy, Pressure};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;
use wire::{Reader, Writer};

/// The plant's time step, s.
pub const DT_S: f64 = 1.0;
/// Seconds of the accident the historical sequence runs to (30 h).
pub const SCENARIO_END_S: f64 = 30.0 * 3600.0;

/// Who drives the operator actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The recorded sequence of the plant's unit: Unit 1's ([`EVENTS`],
    /// INPO 11-005 timeline, \[U\] times); since gh:#1109 Units 2 and 3's
    /// from USNRC HRTD Table 7.1-2 ([`units23`], via [`Plant::for_unit`]).
    Historical,
    /// The reader's own actions.
    FreePlay,
}

/// The enthalpy above 298 K that the hydrogen leaving the vessel takes, W:
/// `to_relief` of it at saturation `t_sat`, the rest at the core exit gas
/// temperature. The constant-`c_p` ablation keeps the old operation order.
fn h2_exit_enthalpy(h2_out: f64, to_relief: f64, t_sat: f64, t_exit: f64, cp_t: bool) -> f64 {
    if cp_t {
        h2_out * (to_relief * gas::h2_enthalpy_above_298(t_sat) + (1.0 - to_relief) * gas::h2_enthalpy_above_298(t_exit))
    } else {
        h2_out * self::core::CP_H2_J_PER_KG_K * (to_relief * (t_sat - self::core::T_298_K) + (1.0 - to_relief) * (t_exit - self::core::T_298_K))
    }
}

/// Which modelled mechanisms are on. Every one is **on by default**: the
/// correct-physics-default rule (root `CLAUDE.md`). Turning one off is an
/// explicit ablation through a `without_*` builder, used by [`compare`] to
/// measure how much of each miss against BSAF a mechanism carries
/// (gh:#1032).
///
/// A core mechanism travels to the core worker in its snapshot
/// (`CoreSnapshot::t_dependent_props`); the others act in the plant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mechanisms {
    /// The RPV hot-gas leak to the drywell ([`HOT_LEAK_M2`], `[A]`).
    pub hot_leak: bool,
    /// Hydrogen blanketing of the isolation-condenser tubes (`[A]`).
    pub ic_h2_blanketing: bool,
    /// MATPRO temperature-dependent fuel and cladding heat capacities
    /// (OFFBEAT, gh:#1038); off is the constant `UO2_CP`/`ZR_CP` model.
    pub temperature_dependent_properties: bool,
    /// The lower-head wall: heated by the debris, failing on its inner
    /// temperature or by creep (gh:#1034); off is the debris-temperature
    /// criterion `LOWER_HEAD_FAILURE_K`.
    pub lower_head_wall: bool,
    /// MELCOR's rod collapse (gh:#1035): breach at `CLAD_BREACH_K`,
    /// collapse at `ROD_COLLAPSE_K` or by time-at-temperature damage; off
    /// is the 2100 K whole-node slump `ZR_MELT_K`.
    pub rod_collapse: bool,
    /// In-core radiation between rings and levels, and from the outer ring
    /// to the shroud (gh:#1037); off is no radiation at all.
    pub core_radiation: bool,
    /// The core sees the swollen two-phase mixture level, not the collapsed
    /// one (gh:#1036); off is the collapsed level.
    pub level_swell: bool,
    /// Decay heat from U-235, Pu-239 and U-238 fission products after a
    /// finite irradiation (gh:#1041); off is U-235 alone at equilibrium.
    pub decay_mix: bool,
    /// The ANS neutron-capture correction G(t) (gh:#1041).
    pub capture_correction: bool,
    /// U-239 and Np-239 decay heat (gh:#1041).
    pub actinide_decay: bool,
    /// Choked flow of the vessel's hydrogen-steam gas from its own molar
    /// mass (gh:#1037); off is the steam-only flux for any gas.
    pub mixture_choking: bool,
    /// MELCOR's temperature-dependent `c_v` for the containment's N2 and H2
    /// ([`gas`], 2026-10-29); off is the constant 20.8 / 20.5 J/(mol K).
    pub gas_cv_t: bool,
    /// MELCOR's gaseous-diffusion limit on Zr oxidation
    /// ([`core::diffusion_limited_rate`], 2026-10-29); off is the parabolic
    /// rate limited only by the steam supply.
    pub diffusion_limit: bool,
    /// Hydrogen's `c_p(T)` in the core and the vessel (MELCOR's fit,
    /// [`gas`], 2026-10-29); off is the constant `core::CP_H2_J_PER_KG_K`.
    pub hydrogen_cp_t: bool,
    /// Creep rupture of a main steam line under the relief flow's hot gas
    /// ([`msl`], MELCOR's PIPE-STR and LM-CREEP, 2026-10-29); off is no
    /// steam-line failure.
    pub msl_creep: bool,
    /// The steam dome's internals as a heat structure between the core and
    /// the steam lines, leaks and containment ([`internals`], 2026-10-29);
    /// off passes the core's exit gas straight through.
    pub dome_internals: bool,
    /// Lower-head penetration failure at the wall's temperature limit
    /// (`core::HeadWall`, MELCOR §6.3, 2026-10-29); off is the wall alone.
    pub penetrations: bool,
    /// After lower-head failure the vessel's gas (the hydrogen and steam
    /// made after it) leaves through the breach on the vessel's own
    /// pressure (2026-10-29); off is the vessel sealed at the drywell's
    /// pressure, nothing leaving.
    pub failed_vessel_outflow: bool,
    /// Pool scrubbing of aerosol and iodine where gas enters the pool (the
    /// relief valves' quenchers, the downcomers), by the steam's
    /// condensation: SPARC-90's DF_EC (2026-10-29). Off is MELCOR's
    /// default with pool interaction off: everything passes.
    pub pool_scrubbing: bool,
    /// Cesium and iodine vapour condensing on the dome internals in the
    /// vessel (2026-10-29); off is everything leaving the core reaching the
    /// containment.
    pub in_vessel_deposition: bool,
    /// The dome internals lose heat to the saturated dome by natural
    /// convection (2026-10-29); off is no heat loss once the flow stops.
    pub internals_heat_loss: bool,
    /// The condensation scrubbing stops where the pool is near saturation:
    /// MELCOR's `DF_EC = X_o / X_i` with `X_o` the non-condensable fraction
    /// of the gas in equilibrium with the pool at the vent depth (Eqs D-5,
    /// D-6; 2026-10-29). Off is `X_o = 1`: all the steam condenses.
    pub scrubbing_saturation: bool,
    /// Inertial impaction and swarm-rise scrubbing past the downcomer exits
    /// (SPARC-90's `DF_II` and `DF_SR`, [`sparc`]; 2026-10-29). Off is
    /// condensation scrubbing alone.
    pub swarm_scrubbing: bool,
    /// Cesium and iodine on the aerosol tracked by size section: condensed
    /// in proportion to each section's mass transfer, carried through
    /// agglomeration and deposited with their section (`rn_volume`;
    /// 2026-10-29). Off is the lumped model: each class shares the aerosol's
    /// volume distribution.
    pub sectional_classes: bool,
    /// The condensation scrubbing of iodine vapour limited by the pool's
    /// partition coefficient for I2, from SPARC-90's equilibrium chemistry
    /// with the pool's dissolved CsOH (MELCOR Eq D-7, Appendix F; [`iodine`];
    /// 2026-10-29). Off is iodine vapour scrubbed like the gas.
    pub iodine_partition: bool,
    /// The wetwell vent passes the wetwell's gas mixture through its
    /// orifice, the pool boiling to supply the steam, and the boiled-off
    /// water takes its sensible heat too (gh:#1053; 2026-10-29). Off is the
    /// vent sized for the non-condensables alone, with steam added in
    /// proportion and only latent heat leaving the pool.
    pub mixture_vent: bool,
    /// Thermophoresis (from the walls' heat flux) and diffusiophoresis
    /// (from the steam condensing on them) onto the drywell's structures,
    /// MELCOR's defaults (RN §2.4.2.2; 2026-10-29). Off is gravity and
    /// Brownian diffusion alone.
    pub phoresis: bool,
    /// The swarm rise past the downcomers scrubs I2 vapour by diffusion to
    /// the bubbles' walls, limited by the pool's partition (MELCOR Appendix
    /// F; [`sparc::iodine_pass`]; 2026-10-29). Off is iodine vapour scrubbed
    /// by condensation alone.
    pub iodine_swarm: bool,
    /// The vessel's gas as a sectional aerosol volume (`rn_volume`;
    /// 2026-10-29): the released aerosol and vapour agglomerate, condense
    /// and deposit on the dome internals (at their temperature, with
    /// thermophoresis from their heat flux) and settle on a floor of the
    /// vessel's cross-section; what leaves carries its size distribution
    /// into the containment. Off is the aerosol passing straight through
    /// and the vapour condensing on the internals alone.
    pub vessel_aerosol: bool,
    /// Cesium and iodine condensed on the aerosol add to their section's
    /// particle mass, as MELCOR's sectional mass does (RN §2.5.2; the area,
    /// the agglomeration and the deposition follow the section's total;
    /// 2026-10-29). Off is the condensate riding weightless on the UO2
    /// particles.
    pub condensate_mass: bool,
    /// The decay heat of the released noble gases, alkali metals and
    /// halogens leaves the core with them and heats wherever they are: the
    /// vessel, the drywell's gas and structures, the pool; what is vented
    /// or leaked takes its heat away (MELCOR's DCH by class, the shares from
    /// `fp_decay`'s summation; 2026-10-29). Chalcogens (Te) are released but
    /// not transported, so their heat stays in the core (stated). Off is all
    /// decay heat in the core.
    pub fp_decay_follows: bool,
    /// Tellurium (MELCOR's chalcogen class) released with the volatiles
    /// and transported like cesium and iodine, with its own vapour pressure
    /// (RN 7110), its decay heat following it (2026-10-29; needs
    /// `vessel_aerosol`). Off is tellurium staying in the core, heat and
    /// all.
    pub te_transport: bool,
    /// The lower-plenum debris oxidises while it is quenched, by the
    /// parabolic law over MELCOR's particulate area (COR §3.2.1,
    /// 2026-10-29). Off is debris that never oxidises.
    pub debris_oxidation: bool,
    /// Molten Zr candles down the rods once the oxide shell breaches and
    /// refreezes in the first cooler node below, or reaches the debris
    /// (MELCOR COR §3.1, simplified; 2026-10-29). Off is the metal staying
    /// in its node until the rod falls.
    pub candling: bool,
    /// MELCOR's falling-debris quench: the debris in the wet lower plenum
    /// gives the water `h A_p (T - T_sat)` at COR_LP's 100 W/(m^2 K) over
    /// its particle area, with the decay factor, and is then limited by
    /// Lipinski's dryout flux (COR §2.3.8; 2026-10-29). Off is the excess
    /// heat released over a 120 s time constant \[A\].
    pub debris_quench: bool,
    /// The lower head takes heat from the debris that touches it while the
    /// plenum holds water, and can fail then (as MELCOR's head conducts
    /// from the debris in contact; 2026-10-29). Off is the head boiling
    /// against the plenum's water while there is any.
    pub wet_head_contact: bool,
    /// The penetrations take the debris's heat over the part of their
    /// height it covers (MELCOR's `dz_d / dz_1`, COR §6.1), the rest
    /// cooled by the plenum's water while there is any (2026-10-29). Off is
    /// the penetration heated by the debris over all of it.
    pub penetration_coverage: bool,
    /// A failed vessel's water drains through the breach into the
    /// pedestal (2026-10-29; MELCOR opens a lower-plenum flow path at
    /// failure). Off is the water kept in the failed vessel.
    pub failed_vessel_drain: bool,
    /// The wetwell atmosphere has its own temperature and steam: MELCOR's
    /// pool-surface exchange (CVH/FL RM §5.1) and bubble model at the vents
    /// (§6.1, the efficiency `epsilon_z epsilon_T`), flashing, and the vent
    /// and vacuum breakers passing the atmosphere as it is (2026-10-29). Off
    /// is the gas at the pool's temperature, saturated.
    pub ww_atmosphere: bool,
    /// The drywell structures as MELCOR's heat-structure surface (HS RM
    /// §2.6.1, §2.7): natural convection and condensation by diffusion
    /// through the non-condensables, its latent heat into the wall
    /// (2026-10-29). Off is the fixed 20 W/(m^2 K) \[A\] and condensation
    /// only of supersaturated steam.
    pub dw_wall_hs: bool,
    /// The torus shell above the pool as a heat structure on the wetwell
    /// atmosphere: MELCOR's natural convection and condensation, its outer
    /// face adiabatic (2026-10-29; needs `ww_atmosphere`). Off is no
    /// structure in the wetwell gas.
    pub ww_shell: bool,
    /// Gas leaving the drywell through the downcomers and the flange leak
    /// takes its enthalpy, so the drywell cools as it blows down and the
    /// pool receives the flow work (MELCOR CVH/FL RM Eqs 6-5, 6-6;
    /// 2026-10-29). Off is the internal energy only, the drywell keeping its
    /// temperature.
    pub outflow_enthalpy: bool,
    /// The relief valves' hydrogen reaches the wetwell with its enthalpy
    /// at the vessel's saturation temperature, closing the energy the
    /// vessel gives up with it (2026-10-29; needs `ww_atmosphere`). Off is
    /// the hydrogen at the pool's temperature.
    pub relief_h2_enthalpy: bool,
    /// Water condensed in the drywell (on its structures, and steam above
    /// saturation) drains to the floor, the pedestal's pool, where debris
    /// can boil it again (2026-10-29; MELCOR drains a structure's film to
    /// its volume's pool). Off is the condensate leaving the model.
    pub condensate_to_floor: bool,
    /// The RPV wall (cylinder and top head, 0.16 m of steel \[A\]) as a
    /// lumped heat structure: it exchanges with the vessel's saturated
    /// contents while they hold water, and loses heat through its
    /// insulation to the drywell (2026-10-29, gh:#1037). Off is no wall.
    pub rpv_wall: bool,
    /// The dome internals radiate to the RPV wall (MELCOR's gray pair, HS
    /// RM Eq 2-96, with its default steel emissivity; 2026-10-29; needs
    /// `rpv_wall`). Off is the internals with no heat path once the gas
    /// stops flowing past them.
    pub internals_radiation: bool,
    /// The top of the core radiates to the dome internals above it, its
    /// axial boundary structure (MELCOR COR RM §2.1.2, `Fcell,a` =
    /// `axial_exchange`; 2026-10-29; needs `core_radiation`). Off is the top
    /// of the core exchanging nothing.
    pub core_top_radiation: bool,
    /// The core's radiation looks through emptied cells to the next
    /// standing component, radially (or to the shroud) and axially, as
    /// MELCOR's does (COR RM §2.1.2; 2026-10-29). Off is no view across an
    /// emptied cell.
    pub see_through: bool,
    /// Molten core-concrete interaction on the drywell floor: a reduced
    /// form of MELCOR's CAV package ([`cavity`]; CORCON basaltic concrete,
    /// quasi-steady ablation, the concrete's H2O and CO2 reduced by the
    /// debris's Zr; 2026-10-29, F8). Off is the dry debris's heat going
    /// straight into the drywell's structures.
    pub mcci: bool,
    /// The drywell structures lose heat across the gap to the concrete
    /// around them, a 1-D conduction slab to the reactor building
    /// (2026-10-29; MELCOR's heat structures conduct). Off is the lumped
    /// structures adiabatic outside.
    pub dw_wall_outer: bool,
    /// Relocating material rests on the BWR core plate until the plate
    /// yields (MELCOR's PLATEB, COR RM §5.2.3, Eqs 5-3, 5-13, 5-16;
    /// 2026-10-29); thickness and beam spacing \[A\]. Off is relocation
    /// straight to the lower plenum.
    pub core_plate: bool,
    /// The lowest uncovered core cell radiates down to the lower plenum's
    /// water (MELCOR COR RM §2.1.2, `FLPUP` = 0.25; 2026-10-29; needs
    /// `core_radiation`); the steam it raises rises through the core. Off is
    /// the bottom of the core exchanging nothing.
    pub bottom_radiation: bool,
    /// One core plate per radial ring, each holding its ring's relocating
    /// material and failing on its own load (MELCOR COR RM §5.2.3: the
    /// supporting structure is a component of each ring's cell; 2026-10-29;
    /// needs `core_plate`). Off is one plate under the whole core.
    pub ring_plates: bool,
    /// The torus pool in two layers split at the downcomers' exits: the
    /// downcomers (and every surface process) act on the water above them,
    /// the relief valves' quenchers on the water below; the layers mix when
    /// the lower is the hotter (2026-10-29; PPOOLEX, Li and Kudinov). Off is
    /// one well-mixed pool.
    pub pool_layers: bool,
    /// The debris held on a core plate oxidises in the steam rising past it,
    /// which then no longer reaches its ring's core (MELCOR COR §3.2.1,
    /// particulate debris; 2026-10-29; needs `core_plate` and
    /// `debris_oxidation`). Off is the held debris inert.
    pub plate_oxidation: bool,
    /// The steam the lower plenum's debris oxidation uses no longer rises
    /// into the core (2026-10-29, a conservation defect: the core could
    /// oxidise with it a second time). Off is the defect, kept as the
    /// ablation that reproduces the earlier runs.
    pub lp_steam_consumed: bool,
    /// The control blades, MELCOR's NS component in its BWR form (steel
    /// only, 2026-10-30): their steel at each cell's cladding temperature,
    /// oxidised by MELCOR's stainless-steel law (COR RM §2.5.6) in the steam
    /// the Zr leaves, failing at the steel's melting temperature and
    /// draining to the lower plenum, where it quenches. Their B4C is not
    /// modelled (MELCOR's default BWR model caps its oxidation at 2 %).
    /// Off is a core without blades.
    pub control_blades: bool,
    /// A control blade at the steel's melting temperature absorbs its heat
    /// of fusion (MELCOR's 2.68e5 J/kg) before it drains, and carries it to
    /// the lower plenum (2026-10-30; needs `control_blades`). Off drains it
    /// at once at its solid enthalpy.
    pub blade_fusion: bool,
    /// The cavity debris's phases (MELCOR CAV RM §3.3, §3.10; 2026-10-30;
    /// needs `mcci`): its oxide melts over an ideal-solution pseudo-binary
    /// range with latent heats, and its interior conducts when solid and
    /// convects when liquid. Off is one convecting debris without phase
    /// change.
    pub cavity_phases: bool,
    /// The reactor building's burn starts by MELCOR's BUR criteria for a
    /// volume without igniters (BUR RM §2.2: Le Chatelier with CO, oxygen,
    /// steam and CO2 inerting; 2026-10-30), CO and CO2 followed as tracers
    /// in the nitrogen-like gas. Off is the 8 % hydrogen threshold \[A\].
    pub bur_ignition: bool,
    /// The wetwell vent and the drywell head flange, which discharge to the
    /// atmosphere from several bar, pass at most MELCOR's critical gas flow
    /// (CVH/FL RM §6.3, Eq 6-13; 2026-10-30): a missing limit, the
    /// incompressible orifice passing about twice it at those ratios. Off
    /// is the orifice alone.
    pub gas_choking: bool,
    /// The control blades' steel oxidation held to MELCOR's gaseous-
    /// diffusion limit (COR RM §2.5.3, Eq 2-168, which binds every
    /// reaction; 2026-10-30; needs `control_blades` and `diffusion_limit`).
    /// Off leaves the steel's parabolic rate unlimited.
    pub blade_diffusion_limit: bool,
    /// The downcomers' and vacuum breakers' equalising cap counts the
    /// receiving volume's pressure rise as well as the donor's fall
    /// (2026-10-30, a numerical defect: the one-sided cap overshot the set
    /// point, and the vacuum breaker chattered about it every step,
    /// amplifying a 1e-9 perturbation 1.5 times a step). Off is the
    /// one-sided cap.
    pub two_sided_equalising: bool,
    /// The control blades' B4C reacts with steam by MELCOR's simple model
    /// (COR RM §2.5.9, MARCON 2.1B; at most 2 % of it; 2026-10-30; needs
    /// `control_blades`). Off is inert B4C.
    pub b4c_oxidation: bool,
    /// A building burn runs MELCOR's combustion completeness, flame speed
    /// and rate (BUR RM §2.3 to §2.5; 2026-10-30), consuming oxygen and
    /// making CO2, and reports its pressure rise as an adiabatic bound.
    /// Off is the burn as one step that removes all of the building's H2
    /// and CO and wrecks the building (no further inflow).
    pub burn_dynamics: bool,
    /// Debris leaving the failed vessel falls through the pedestal's water
    /// by MELCOR's low-pressure melt ejection (FDI RM §2.2; 2026-10-30):
    /// Chu's breakup, radiation or Bromley film boiling, all of the heat
    /// boiling water. Off is debris landing at the core's temperature.
    pub fdi_lpme: bool,
    /// Candling Zr carries UO2 down with it, 0.2 of its mass: MELCOR's
    /// default secondary transport (COR RM §3.1.4, COR_CMT; 2026-10-30).
    /// Off is the Zr candling alone.
    pub uo2_candling: bool,
    /// A molten lower-plenum pool (above the UO2-ZrO2 eutectic) convects to
    /// the head by MELCOR's ACOPO correlation (COR RM §2.4.2, Table 2-4;
    /// 2026-10-30). Off is the fixed debris-to-head coefficient.
    pub molten_pool: bool,
    /// The containment is `tampines`' control-volume network
    /// ([`containment_network::ContainmentNetwork`]: drywell, torus and
    /// reactor building as control volumes with IF97 water, V(z) tables,
    /// inertial flow paths and an implicit step), **the default since
    /// 2026-11-02** (maintainer decision on gh:#1099, "switch it"). Off is
    /// the lumped [`containment::Containment`], the model every result
    /// before that date was measured on. The network costs about 100 times
    /// the lumped model a step (the module docs give the measured timing).
    pub network_containment: bool,
    /// The drywell spray's droplets reach equilibrium with the atmosphere
    /// as they fall, condensing steam (gh:#1109,
    /// `containment::spray_condensation`; Unit 3's fire-pumper spray, none
    /// on Unit 1). Off is the spray's water landing on the floor unchanged.
    pub containment_spray: bool,
}

impl Default for Mechanisms {
    fn default() -> Self {
        Self {
            hot_leak: true,
            ic_h2_blanketing: true,
            temperature_dependent_properties: true,
            lower_head_wall: true,
            rod_collapse: true,
            core_radiation: true,
            level_swell: true,
            decay_mix: true,
            capture_correction: true,
            actinide_decay: true,
            mixture_choking: true,
            gas_cv_t: true,
            diffusion_limit: true,
            hydrogen_cp_t: true,
            msl_creep: true,
            dome_internals: true,
            penetrations: true,
            failed_vessel_outflow: true,
            pool_scrubbing: true,
            in_vessel_deposition: true,
            internals_heat_loss: true,
            scrubbing_saturation: true,
            swarm_scrubbing: true,
            sectional_classes: true,
            iodine_partition: true,
            mixture_vent: true,
            phoresis: true,
            iodine_swarm: true,
            vessel_aerosol: true,
            condensate_mass: true,
            fp_decay_follows: true,
            te_transport: true,
            debris_oxidation: true,
            candling: true,
            debris_quench: true,
            wet_head_contact: true,
            penetration_coverage: true,
            failed_vessel_drain: true,
            ww_atmosphere: true,
            dw_wall_hs: true,
            ww_shell: true,
            outflow_enthalpy: true,
            relief_h2_enthalpy: true,
            condensate_to_floor: true,
            rpv_wall: true,
            internals_radiation: true,
            core_top_radiation: true,
            see_through: true,
            mcci: true,
            dw_wall_outer: true,
            core_plate: true,
            bottom_radiation: true,
            ring_plates: true,
            pool_layers: true,
            plate_oxidation: true,
            lp_steam_consumed: true,
            control_blades: true,
            blade_fusion: true,
            cavity_phases: true,
            bur_ignition: true,
            gas_choking: true,
            blade_diffusion_limit: true,
            two_sided_equalising: true,
            b4c_oxidation: true,
            burn_dynamics: true,
            fdi_lpme: true,
            uo2_candling: true,
            molten_pool: true,
            network_containment: true,
            containment_spray: true,
        }
    }
}

impl Mechanisms {
    /// No RPV hot-gas leak: the vessel loses gas only through the relief
    /// valves (and the breach once the lower head fails).
    pub fn without_hot_leak(mut self) -> Self {
        self.hot_leak = false;
        self
    }

    /// Constant fuel and cladding heat capacities (`UO2_CP`, `ZR_CP`, both
    /// `[A]`): the model before gh:#1038.
    pub fn without_temperature_dependent_properties(mut self) -> Self {
        self.temperature_dependent_properties = false;
        self
    }

    /// The lower head fails when dry debris exceeds `LOWER_HEAD_FAILURE_K`
    /// (`[A]`), with no wall: the model before gh:#1034.
    pub fn without_lower_head_wall(mut self) -> Self {
        self.lower_head_wall = false;
        self
    }

    /// A node slumps whole to the lower plenum when its cladding reaches
    /// `ZR_MELT_K` (`[U]`): the model before gh:#1035.
    pub fn without_rod_collapse(mut self) -> Self {
        self.rod_collapse = false;
        self
    }

    /// No radiation inside the core and no shroud heat structure: the model
    /// before gh:#1037.
    pub fn without_core_radiation(mut self) -> Self {
        self.core_radiation = false;
        self
    }

    /// The core sees the collapsed level: the model before gh:#1036.
    pub fn without_level_swell(mut self) -> Self {
        self.level_swell = false;
        self
    }

    /// U-235 fission products alone, at equilibrium: the decay heat before
    /// gh:#1041 (with the other two switches, the whole of it).
    pub fn without_decay_mix(mut self) -> Self {
        self.decay_mix = false;
        self
    }

    /// No neutron-capture correction (G = 1).
    pub fn without_capture_correction(mut self) -> Self {
        self.capture_correction = false;
        self
    }

    /// No U-239 or Np-239 decay heat.
    pub fn without_actinide_decay(mut self) -> Self {
        self.actinide_decay = false;
        self
    }

    /// The steam-only choked flux for the vessel gas, however much hydrogen
    /// it holds: the model before 2026-10-29.
    pub fn without_mixture_choking(mut self) -> Self {
        self.mixture_choking = false;
        self
    }

    /// Constant `c_v` for the containment's N2 (20.8) and H2 (20.5 J/(mol
    /// K)): the model before 2026-10-29.
    pub fn without_gas_cv_t(mut self) -> Self {
        self.gas_cv_t = false;
        self
    }

    /// No gaseous-diffusion limit on oxidation: the model before
    /// 2026-10-29.
    pub fn without_diffusion_limit(mut self) -> Self {
        self.diffusion_limit = false;
        self
    }

    /// Hydrogen at the constant `c_p` in the core and the vessel: the model
    /// before 2026-10-29.
    pub fn without_hydrogen_cp_t(mut self) -> Self {
        self.hydrogen_cp_t = false;
        self
    }

    /// No main-steam-line creep rupture: the model before 2026-10-29.
    pub fn without_msl_creep(mut self) -> Self {
        self.msl_creep = false;
        self
    }

    /// No dome internals: the core's exit gas reaches the steam lines, the
    /// leaks and the containment unchanged (the model before 2026-10-29).
    pub fn without_dome_internals(mut self) -> Self {
        self.dome_internals = false;
        self
    }

    /// The lower head fails by its wall alone, no penetration: the model
    /// before 2026-10-29.
    pub fn without_penetrations(mut self) -> Self {
        self.penetrations = false;
        self
    }

    /// A failed vessel passes nothing to the drywell: the model before
    /// 2026-10-29.
    pub fn without_failed_vessel_outflow(mut self) -> Self {
        self.failed_vessel_outflow = false;
        self
    }

    /// No pool scrubbing: everything entering the pool passes to the
    /// wetwell gas (the model before 2026-10-29, and MELCOR's default with
    /// pool interaction off).
    pub fn without_pool_scrubbing(mut self) -> Self {
        self.pool_scrubbing = false;
        self
    }

    /// No in-vessel deposition: the model before 2026-10-29.
    pub fn without_in_vessel_deposition(mut self) -> Self {
        self.in_vessel_deposition = false;
        self
    }

    /// The dome internals lose no heat once the flow past them stops: the
    /// model before 2026-10-29.
    pub fn without_internals_heat_loss(mut self) -> Self {
        self.internals_heat_loss = false;
        self
    }

    /// Condensation scrubbing as if all the steam condensed: the model
    /// before 2026-10-29.
    pub fn without_scrubbing_saturation(mut self) -> Self {
        self.scrubbing_saturation = false;
        self
    }

    /// No impaction or swarm-rise scrubbing: the model before 2026-10-29.
    pub fn without_swarm_scrubbing(mut self) -> Self {
        self.swarm_scrubbing = false;
        self
    }

    /// Each class on the aerosol lumped over its volume distribution: the
    /// model before 2026-10-29 (to round-off: the lumped operations are
    /// done section by section).
    pub fn without_sectional_classes(mut self) -> Self {
        self.sectional_classes = false;
        self
    }

    /// Iodine vapour scrubbed like the gas, no partition limit: the model
    /// before 2026-10-29.
    pub fn without_iodine_partition(mut self) -> Self {
        self.iodine_partition = false;
        self
    }

    /// The vent sized for the non-condensables alone: the model before
    /// 2026-10-29.
    pub fn without_mixture_vent(mut self) -> Self {
        self.mixture_vent = false;
        self
    }

    /// No phoresis onto the drywell's structures: the model before
    /// 2026-10-29.
    pub fn without_phoresis(mut self) -> Self {
        self.phoresis = false;
        self
    }

    /// No swarm scrubbing of iodine vapour: the model before 2026-10-29.
    pub fn without_iodine_swarm(mut self) -> Self {
        self.iodine_swarm = false;
        self
    }

    /// No aerosol dynamics in the vessel: the model before 2026-10-29.
    pub fn without_vessel_aerosol(mut self) -> Self {
        self.vessel_aerosol = false;
        self
    }

    /// Condensate weightless on the particles: the model before
    /// 2026-10-29.
    pub fn without_condensate_mass(mut self) -> Self {
        self.condensate_mass = false;
        self
    }

    /// All decay heat stays in the core: the model before 2026-10-29.
    pub fn without_fp_decay_follows(mut self) -> Self {
        self.fp_decay_follows = false;
        self
    }

    /// No tellurium transport: the model before 2026-10-29.
    pub fn without_te_transport(mut self) -> Self {
        self.te_transport = false;
        self
    }

    /// Debris that never oxidises: the model before 2026-10-29.
    pub fn without_debris_oxidation(mut self) -> Self {
        self.debris_oxidation = false;
        self
    }

    /// No candling: the model before 2026-10-29.
    pub fn without_candling(mut self) -> Self {
        self.candling = false;
        self
    }

    /// The 120 s quench: the model before 2026-10-29.
    pub fn without_debris_quench(mut self) -> Self {
        self.debris_quench = false;
        self
    }

    /// The head boils against the water while there is any: the model
    /// before 2026-10-29.
    pub fn without_wet_head_contact(mut self) -> Self {
        self.wet_head_contact = false;
        self
    }

    /// The penetration fully covered by any debris: the model before
    /// 2026-10-29.
    pub fn without_penetration_coverage(mut self) -> Self {
        self.penetration_coverage = false;
        self
    }

    /// A failed vessel keeps its water: the model before 2026-10-29.
    pub fn without_failed_vessel_drain(mut self) -> Self {
        self.failed_vessel_drain = false;
        self
    }

    /// The wetwell gas at the pool's temperature and saturated: the model
    /// before 2026-10-29.
    pub fn without_ww_atmosphere(mut self) -> Self {
        self.ww_atmosphere = false;
        self
    }

    /// The drywell structures with the fixed coefficient: the model before
    /// 2026-10-29.
    pub fn without_dw_wall_hs(mut self) -> Self {
        self.dw_wall_hs = false;
        self
    }

    /// No structure in the wetwell gas: the model before 2026-10-29.
    pub fn without_ww_shell(mut self) -> Self {
        self.ww_shell = false;
        self
    }

    /// Drywell outflows at internal energy: the model before 2026-10-29.
    pub fn without_outflow_enthalpy(mut self) -> Self {
        self.outflow_enthalpy = false;
        self
    }

    /// The relief valves' hydrogen at the pool's temperature: the model
    /// before 2026-10-29.
    pub fn without_relief_h2_enthalpy(mut self) -> Self {
        self.relief_h2_enthalpy = false;
        self
    }

    /// The drywell's condensate leaves the model: before 2026-10-29.
    pub fn without_condensate_to_floor(mut self) -> Self {
        self.condensate_to_floor = false;
        self
    }

    /// No RPV wall: the model before 2026-10-29.
    pub fn without_rpv_wall(mut self) -> Self {
        self.rpv_wall = false;
        self
    }

    /// The internals do not radiate to the wall: before 2026-10-29.
    pub fn without_internals_radiation(mut self) -> Self {
        self.internals_radiation = false;
        self
    }

    /// The top of the core exchanging nothing: before 2026-10-29.
    pub fn without_core_top_radiation(mut self) -> Self {
        self.core_top_radiation = false;
        self
    }

    /// No view across an emptied cell: before 2026-10-29.
    pub fn without_see_through(mut self) -> Self {
        self.see_through = false;
        self
    }

    /// No core-concrete interaction: before 2026-10-29.
    pub fn without_mcci(mut self) -> Self {
        self.mcci = false;
        self
    }

    /// The drywell structures adiabatic outside: before 2026-10-29.
    pub fn without_dw_wall_outer(mut self) -> Self {
        self.dw_wall_outer = false;
        self
    }

    /// No core-plate holdup: before 2026-10-29.
    pub fn without_core_plate(mut self) -> Self {
        self.core_plate = false;
        self
    }

    /// The bottom of the core exchanging nothing: before 2026-10-29.
    pub fn without_bottom_radiation(mut self) -> Self {
        self.bottom_radiation = false;
        self
    }

    /// One core plate under the whole core: before 2026-10-29.
    pub fn without_ring_plates(mut self) -> Self {
        self.ring_plates = false;
        self
    }

    /// One well-mixed torus pool: before 2026-10-29.
    pub fn without_pool_layers(mut self) -> Self {
        self.pool_layers = false;
        self
    }

    /// The debris on the core plate inert: before 2026-10-29.
    pub fn without_plate_oxidation(mut self) -> Self {
        self.plate_oxidation = false;
        self
    }

    /// The lower plenum's oxidation steam rising into the core as well:
    /// the defect before 2026-10-29.
    pub fn without_lp_steam_consumed(mut self) -> Self {
        self.lp_steam_consumed = false;
        self
    }

    /// A core without control blades: before 2026-10-30.
    pub fn without_control_blades(mut self) -> Self {
        self.control_blades = false;
        self
    }

    /// The blade drained at once at its solid enthalpy: before 2026-10-30.
    pub fn without_blade_fusion(mut self) -> Self {
        self.blade_fusion = false;
        self
    }

    /// The cavity debris convecting without phase change: before
    /// 2026-10-30.
    pub fn without_cavity_phases(mut self) -> Self {
        self.cavity_phases = false;
        self
    }

    /// The building exploding at 8 % hydrogen: before 2026-10-30.
    pub fn without_bur_ignition(mut self) -> Self {
        self.bur_ignition = false;
        self
    }

    /// The vent and the flange as incompressible orifices: before
    /// 2026-10-30.
    pub fn without_gas_choking(mut self) -> Self {
        self.gas_choking = false;
        self
    }

    /// The blades' steel oxidation without the diffusion limit: before
    /// 2026-10-30.
    pub fn without_blade_diffusion_limit(mut self) -> Self {
        self.blade_diffusion_limit = false;
        self
    }

    /// The one-sided equalising cap: before 2026-10-30.
    pub fn without_two_sided_equalising(mut self) -> Self {
        self.two_sided_equalising = false;
        self
    }

    /// Inert B4C: before 2026-10-30.
    pub fn without_b4c_oxidation(mut self) -> Self {
        self.b4c_oxidation = false;
        self
    }

    /// The burn as one step: before 2026-10-30.
    pub fn without_burn_dynamics(mut self) -> Self {
        self.burn_dynamics = false;
        self
    }

    /// Debris landing at the core's temperature: before 2026-10-30.
    pub fn without_fdi_lpme(mut self) -> Self {
        self.fdi_lpme = false;
        self
    }

    /// The Zr candling alone: before 2026-10-30.
    pub fn without_uo2_candling(mut self) -> Self {
        self.uo2_candling = false;
        self
    }

    /// The fixed debris-to-head coefficient for molten debris too: before
    /// 2026-10-30.
    pub fn without_molten_pool(mut self) -> Self {
        self.molten_pool = false;
        self
    }

    /// The lumped [`containment::Containment`] instead of the control-volume
    /// network: the plant before 2026-11-02 (gh:#1099). Every result pinned
    /// before the switch is reproduced under this ablation, bit for bit.
    pub fn without_network_containment(mut self) -> Self {
        self.network_containment = false;
        self
    }

    /// The spray's water reaching the floor without exchanging with the
    /// atmosphere: before gh:#1109 (no spray existed).
    pub fn without_containment_spray(mut self) -> Self {
        self.containment_spray = false;
        self
    }

    /// What the containment worker needs to build the plant's containment
    /// (gh:#1099).
    pub fn containment_choice(&self) -> ContainmentChoice {
        ContainmentChoice { network: self.network_containment, mcci: self.mcci, pool_layers: self.pool_layers, unit: Unit::One }
    }

    /// Isolation-condenser condensation unaffected by hydrogen in the vessel.
    pub fn without_ic_h2_blanketing(mut self) -> Self {
        self.ic_h2_blanketing = false;
        self
    }
}

/// What the operators (or the historical sequence) have set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Controls {
    pub ic_open: [bool; IC_TRAINS],
    pub dc_power: bool,
    /// Hold the relief valves open (manual depressurisation; needs DC).
    pub relief_held_open: bool,
    pub injection: bool,
    pub vent_ordered: bool,
}

impl Default for Controls {
    fn default() -> Self {
        Self { ic_open: [false; IC_TRAINS], dc_power: true, relief_held_open: false, injection: false, vent_ordered: false }
    }
}

/// One recorded moment of the Unit 1 accident, seconds after the scram
/// (14:46 JST, 11 March 2011). \[U\] Times are from the INPO 11-005 timeline
/// as recalled, not yet checked against the report.
///
/// **Partly checked 2026-10-29 (gh:#1044)** against the observed anchors in
/// BSAF Phase II (`crates/kovan-literature/derived/nea7525-bsaf-fukushima-daiichi-bands.md`
/// §1.1; NEA No. 7525, © OECD 2021, cited with acknowledgement):
/// - the reactor-building explosion, 15:36 on 12 March = 24.83 h, **agrees**
///   with BSAF's observed 24.83 h;
/// - the wetwell vent order, 14:30 = 23.73 h, is **consistent** with BSAF's
///   "manually vented ≈24 h" (vent closure 24.68 h, a BSAF boundary
///   condition this sequence does not have).
///
/// ~~The other five times (IC stop, IC cycling, station blackout, the three
/// injection changes) have no BSAF anchor and stay `[U]` until checked
/// against INPO 11-005, IAEA TV1 or the Japanese government report
/// (#1031).~~ **CORRECTED 2026-10-30 (gh:#1044, F13)**: checked against the
/// NRC's open training chapter, *Fukushima Dai-ichi Earthquake and Tsunami
/// Event* (USNRC HRTD, GE BWR/4 Advanced Technology, ch. 7.1, Table 7.1-2;
/// `reactor-literature/theodore-open-corpus/nrc/`):
/// - IC stop 15:03, vent 14:30, explosion 15:36 and seawater 19:04
///   **agree** to the minute;
/// - fresh-water injection: the NRC's 05:46, 15 h 00 min after the scram,
///   is the label's own time, but the event fired at 14 h 59 min (a
///   defect, label and code disagreeing). **Fixed to 15.0 h**;
/// - the station blackout: the NRC lists the loss of all AC and DC at about
///   15:41 (00:55); this model keeps INPO's 15:37 as recalled. The sources
///   disagree by 4 min, **not resolved** (INPO 11-005 is not held);
/// - IC cycling at 15:17 and the fresh water exhausted at 14:53: not in
///   the NRC table, still `[U]`.
#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub t_s: f64,
    pub label: &'static str,
    pub kind: EventKind,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EventKind {
    /// Operators stop both isolation condensers (cooldown above 55 degC/h).
    IcStopped,
    /// Operators cycle train A to hold the pressure in a band, until the
    /// station blackout.
    IcCycleA,
    /// Tsunami: AC and DC lost; the IC valves fail closed.
    StationBlackout,
    /// Fire-engine injection starts (fresh water, later seawater).
    InjectionOn,
    InjectionOff,
    /// The wetwell vent is opened.
    VentOpened,
    /// Recorded for comparison only: the model computes its own.
    Explosion,
    // ── Units 2 and 3 (gh:#1109; [`units23`]) ──
    /// RCIC started, the whole flow into the vessel (trips at L8 while DC
    /// lasts).
    RcicStart,
    /// RCIC started with the operators holding the level ([`turbine_pump::Drive::Throttled`]).
    RcicStartThrottled,
    /// RCIC lost for good.
    RcicLost,
    /// HPCI started with the operators holding the level.
    HpciStartThrottled,
    /// HPCI lost for good.
    HpciLost,
    /// DC power lost: the relief valves fall back to their spring set
    /// points, level control and the pumps' trip logic are gone.
    DcLost,
    /// AC lost, DC kept (Unit 3's station blackout): nothing modelled runs
    /// on AC, so recorded.
    AcLost,
    /// This many relief valves held open (by batteries brought in: needs
    /// no station DC).
    ReliefHeld(u8),
    /// The fire pumper sprays the drywell; and stops.
    SprayOn,
    SprayOff,
    /// The wetwell vent closes (the order is withdrawn).
    VentClosed,
    /// What the plant did itself (a trip, an explosion), for comparison
    /// only: never acted on.
    Recorded,
}

pub const EVENTS: [Event; 9] = [
    Event { t_s: 17.0 * 60.0, label: "15:03 Operators stop both IC trains (cooldown > 55 °C/h)", kind: EventKind::IcStopped },
    Event { t_s: 31.0 * 60.0, label: "15:17 Train A cycled to hold pressure", kind: EventKind::IcCycleA },
    Event { t_s: 51.0 * 60.0, label: "15:37 Tsunami: station blackout, IC valves fail closed", kind: EventKind::StationBlackout },
    Event { t_s: 15.0 * 3600.0, label: "05:46 (12 Mar) Fresh-water injection by fire engine", kind: EventKind::InjectionOn },
    Event { t_s: (23.0 * 60.0 + 44.0) * 60.0, label: "14:30 Wetwell vent opened", kind: EventKind::VentOpened },
    Event { t_s: (24.0 * 60.0 + 7.0) * 60.0, label: "14:53 Fresh water exhausted", kind: EventKind::InjectionOff },
    Event { t_s: (24.0 * 60.0 + 50.0) * 60.0, label: "15:36 Reactor building explosion (recorded)", kind: EventKind::Explosion },
    Event { t_s: (28.0 * 60.0 + 18.0) * 60.0, label: "19:04 Seawater injection", kind: EventKind::InjectionOn },
    Event { t_s: SCENARIO_END_S, label: "20:46 (12 Mar) End of the run", kind: EventKind::Explosion },
];

/// Pressure band operators held with train A before the blackout, Pa. \[A\]
/// (the recorded cycling kept it roughly between 6 and 7 MPa).
const IC_A_BAND: (f64, f64) = (6.0e6, 7.0e6);

/// The vessel's state derived from its mass and energy.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vessel {
    pub sat: Sat,
    pub x: f64,
    pub p_pa: f64,
    /// Collapsed level above the bottom of active fuel, m.
    pub level_m: f64,
    pub gas_m3: f64,
    pub h2_fraction: f64,
    /// True when no saturated mixture matches (boiled dry): an ideal-gas
    /// steam fallback, outside what the model represents.
    pub dry: bool,
}

/// Which containment the plant builds, and the two plant mechanisms the
/// network maps onto its own (gh:#1099): `mcci` onto
/// [`containment_network::NetworkMechanisms::cavity`] and `pool_layers`
/// onto its layered torus pool. The network reads every other containment
/// switch it carries from the snapshot, as the lumped model does; the
/// lumped model's own numerical and gas-model switches (`ww_atmosphere`,
/// `gas_cv_t`, `outflow_enthalpy`, `mixture_vent`, `two_sided_equalising`,
/// `gas_choking`, `condensate_to_floor`, `burn_dynamics`, `relief_h2_enthalpy`
/// excepted, which the network reads) have no counterpart on the network
/// and do nothing there (stated, not hidden).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContainmentChoice {
    pub network: bool,
    pub mcci: bool,
    pub pool_layers: bool,
    /// Which unit's network to build (gh:#1109); Unit 1 from
    /// [`Mechanisms::containment_choice`], the plant's own from
    /// [`Plant::containment_choice`].
    pub unit: Unit,
}

/// The containment model a plant runs, by enum dispatch (rust-design-rules:
/// no trait objects, no `Box`; the network is held by value).
#[derive(Clone, Debug)]
#[allow(clippy::large_enum_variant)] // owned by value, as the workspace's design rules ask
pub enum ContainmentModel {
    /// The control-volume network, the default since gh:#1099.
    Network(containment_network::ContainmentNetwork),
    /// The lumped model (`Mechanisms::without_network_containment`).
    Lumped(Containment),
    /// The network could not be built from these inputs; why. Its steps
    /// return the initial state and [`PlantContainment::error`] says so.
    Unbuilt(String),
}

/// The plant's containment (gh:#1099, 2026-11-02): either model behind one
/// step, `ContainmentSnapshot` in, the old [`ContainmentResult`] out
/// ([`containment_network::NetworkResult::to_containment_result`] maps the
/// network's), so the plant, the frame, the CSV and every consumer read
/// one type.
///
/// **State that cannot cross a worker boundary.** The network's state
/// (`tampines`' `CvNetwork`) has no wire form, so the containment worker
/// holds its containment between steps ([`ContainmentWorker`]); only the
/// snapshot and the result cross, as exact `f64`s.
///
/// **A network step that fails** (a Picard step that does not converge
/// after six halvings, a state outside IF97) commits nothing; the step
/// returns the last result unchanged and [`PlantContainment::error`]
/// reports the first failure with its time. The run then continues on a
/// frozen containment, so every caller that reports a result must check
/// it (the headless run and `compare::measure_with` panic on it).
#[derive(Clone, Debug)]
pub struct PlantContainment {
    model: ContainmentModel,
    prm: Params,
    last: ContainmentResult,
    error: Option<String>,
    t_s: f64,
}

impl PlantContainment {
    /// A fresh containment for `prm`, the model and the mapped switches
    /// from `choice`.
    pub fn new(prm: &Params, choice: ContainmentChoice) -> Self {
        let mut last = ContainmentResult::default();
        last.p_dw_pa = prm.initial_containment_pa;
        last.p_ww_pa = prm.initial_containment_pa;
        last.t_dw_k = prm.initial_drywell_k;
        last.t_pool_k = prm.initial_pool_k;
        last.t_pool_top_k = prm.initial_pool_k;
        let (model, error) = if choice.network {
            let mut mech = containment_network::NetworkMechanisms::default();
            mech.cavity = choice.mcci;
            mech.pool_layers = choice.pool_layers;
            match containment_network::ContainmentNetwork::with_config(prm, choice.unit.network_config(), mech) {
                Ok(n) => (ContainmentModel::Network(n), None),
                Err(e) => {
                    let m = format!("the containment network could not be built: {e}");
                    (ContainmentModel::Unbuilt(m.clone()), Some(m))
                }
            }
        } else {
            (ContainmentModel::Lumped(Containment::new(prm)), None)
        };
        Self { model, prm: *prm, last, error, t_s: 0.0 }
    }

    /// One step.
    pub fn step(&mut self, s: &ContainmentSnapshot) -> ContainmentResult {
        self.t_s += s.dt_s;
        match &mut self.model {
            ContainmentModel::Network(n) => match n.step(s) {
                Ok(r) => self.last = r.to_containment_result(&self.prm),
                Err(e) => {
                    if self.error.is_none() {
                        self.error = Some(format!("the containment network failed at t = {} s: {e}", self.t_s));
                    }
                }
            },
            ContainmentModel::Lumped(c) => self.last = c.step(s),
            ContainmentModel::Unbuilt(_) => {}
        }
        self.last
    }

    /// The first failure, if any (see the type's docs).
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Whether this is the control-volume network.
    pub fn is_network(&self) -> bool {
        matches!(self.model, ContainmentModel::Network(_))
    }

    /// The model, read-only.
    pub fn model(&self) -> &ContainmentModel {
        &self.model
    }
}

/// The containment worker (gh:#1099): it **holds** the plant's containment
/// between steps, because the network's state has no wire form. Each
/// request carries a run generation, the [`ContainmentChoice`] and the
/// step's snapshot (which carries the `Params`); a new generation rebuilds
/// the containment from them, so a reset or a changed choice needs no
/// separate message (and a message lost before the worker's handshake
/// loses nothing). The reply is a status (1 ok, 0 the containment has
/// failed, see [`PlantContainment::error`]) and the [`ContainmentResult`].
#[derive(Clone, Debug, Default)]
pub struct ContainmentWorker {
    generation: Option<u64>,
    held: Option<PlantContainment>,
}

impl ContainmentWorker {
    /// The request for one step of run `generation`.
    pub fn request(generation: u64, choice: ContainmentChoice, s: &ContainmentSnapshot) -> Vec<f64> {
        let mut w = Writer::default();
        w.f(&[generation as f64, b(choice.network), b(choice.mcci), b(choice.pool_layers), choice.unit.code()]);
        s.write(&mut w);
        w.0
    }

    /// Answer one request (never panics on a short message: the wire reader
    /// reads NaN past the end).
    pub fn handle(&mut self, msg: &[f64]) -> Vec<f64> {
        let mut r = Reader::new(msg);
        let g = r.f();
        let choice = ContainmentChoice { network: r.f() != 0.0, mcci: r.f() != 0.0, pool_layers: r.f() != 0.0, unit: Unit::from_code(r.f()) };
        let s = ContainmentSnapshot::read(&mut r);
        let generation = (g.is_finite() && g >= 0.0).then_some(g as u64);
        if generation.is_none() || generation != self.generation || self.held.is_none() {
            self.generation = generation;
            self.held = Some(PlantContainment::new(&s.prm, choice));
        }
        let mut w = Writer::default();
        if let Some(c) = self.held.as_mut() {
            let res = c.step(&s);
            w.f(&[b(c.error().is_none())]);
            res.write(&mut w);
        }
        w.0
    }

    /// The reply's result and whether the containment is still good.
    pub fn read_reply(v: &[f64]) -> (ContainmentResult, bool) {
        let mut r = Reader::new(v);
        let ok = r.f() == 1.0;
        (ContainmentResult::read(&mut r), ok)
    }

    /// The containment it holds, read-only.
    pub fn held(&self) -> Option<&PlantContainment> {
        self.held.as_ref()
    }
}

/// Everything the page shows, flattened (see [`Frame::write`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub t_s: f64,
    pub mode_free: bool,
    pub controls: Controls,
    pub vessel: Vessel,
    pub decay_w: f64,
    pub q_ic_w: f64,
    pub ic_shell_k: [f64; IC_TRAINS],
    pub ic_shell_kg: [f64; IC_TRAINS],
    pub relief_open: bool,
    pub w_relief_kg_s: f64,
    pub w_bypass_kg_s: f64,
    pub w_leak_kg_s: f64,
    pub w_injection_kg_s: f64,
    pub injected_kg: f64,
    pub rpv_h2_kg: f64,
    pub hot_leak_open: bool,
    /// The main steam line's wall temperature, K, creep damage, and whether
    /// it has ruptured.
    pub msl: msl::SteamLine,
    /// The dome internals' temperature and the gas leaving them, K.
    pub internals_k: f64,
    pub dome_gas_k: f64,
    /// Noble gas (Xe) in the vessel's gas, fraction of inventory (the
    /// containment's shares are in `containment`).
    pub xe_vessel: f64,
    /// Cesium in the vessel's gas, kg (gh:#1047).
    pub cs_vessel_kg: f64,
    pub i_vessel_kg: f64,
    /// Cesium and iodine on the dome internals, kg.
    pub on_internals_kg: [f64; 2],
    pub core: CoreResult,
    pub containment: ContainmentResult,
    /// RCIC's and HPCI's injection into the vessel, the turbines' steam
    /// from it, and the drywell spray, kg/s (gh:#1109; zero on Unit 1).
    pub w_rcic_kg_s: f64,
    pub w_hpci_kg_s: f64,
    pub w_turbine_kg_s: f64,
    pub w_spray_kg_s: f64,
}

/// Choked mass flux of saturated steam, kg/(m^2 s), tabulated against
/// pressure from the steam tables' isentropic converging-nozzle solution
/// (`get_choked_flow_massrate_and_state_from_stagnation_properties_and_area`,
/// stagnation just above the saturated-vapour enthalpy so the flash stays in
/// region 2). Built once, in the plant worker only.
#[derive(Clone, Debug)]
struct ChokedTable {
    ln_p: Vec<f64>,
    flux: Vec<f64>,
}

impl ChokedTable {
    fn new(sat: &SatTable) -> Self {
        // 80 points over 0.2 MPa to the original 9.5 MPa top, continued at
        // the same step to the saturation table's 20 MPa (see steam.rs).
        let ln_p = steam::log_grid(2.0e5, steam::P_GRID_TOP_PA, 80, steam::P_MAX_PA);
        let flux = ln_p
            .iter()
            .map(|l| {
                let p = l.r_exp();
                let h0 = sat.at(p).h_g + 1.0e3;
                let (m, _) = get_choked_flow_massrate_and_state_from_stagnation_properties_and_area(
                    Pressure::new::<pascal>(p),
                    AvailableEnergy::new::<joule_per_kilogram>(h0),
                    Area::new::<square_meter>(1.0),
                );
                m.get::<kilogram_per_second>()
            })
            .collect();
        Self { ln_p, flux }
    }
    /// Mass flux at stagnation pressure `p_pa`; below the table, scaled
    /// linearly with pressure (choked flux is nearly proportional to it).
    fn at(&self, p_pa: f64) -> f64 {
        let l = p_pa.r_ln();
        if l <= self.ln_p[0] {
            return self.flux[0] * p_pa / self.ln_p[0].r_exp();
        }
        let n = self.ln_p.len();
        let step = self.ln_p[1] - self.ln_p[0];
        let i = (((l - self.ln_p[0]) / step).floor() as usize).min(n - 2);
        let w = ((l - self.ln_p[i]) / step).min(1.0);
        self.flux[i] + w * (self.flux[i + 1] - self.flux[i])
    }
}

/// Choked mass flux of a hydrogen-steam gas with hydrogen mole fraction
/// `x_h2`, relative to pure steam at the same pressure and temperature:
/// `sqrt(M_mix / M_H2O)` (ideal gas, fixed gamma).
pub fn mixture_flux_factor(x_h2: f64) -> f64 {
    let x = x_h2.clamp(0.0, 1.0);
    ((x * M_H2 + (1.0 - x) * M_H2O) / M_H2O).sqrt()
}

/// The swollen two-phase mixture level above the bottom of active fuel, m,
/// from the collapsed one (gh:#1036), after MELCOR's CVH pool model (MELCOR
/// 2.2 Vol. 2, CVH/FL §5.1.3, Eqs 5-26 to 5-30): bubbles rise at
/// `bubble_rise_m_s`, so with the vapour flux through the pool top `J` the
/// mean void is `J / (2 v0)`, capped at `max_pool_void`, and the swollen pool
/// fills `V_liquid / (1 - alpha)`. `q_pool_w` W raise vapour of specific
/// volume `v_g` with latent heat `h_fg`; `J` is over the free cross-section
/// at the collapsed level.
pub fn swollen_level(prm: &Params, level_m: f64, q_pool_w: f64, h_fg: f64, v_g: f64) -> f64 {
    let z = level_m + prm.baf_m;
    let dz = 0.01;
    let area = (volume_below(prm, z + dz) - volume_below(prm, z - dz)) / (2.0 * dz);
    if area <= 0.0 || q_pool_w <= 0.0 {
        return level_m;
    }
    let j = q_pool_w / h_fg * v_g / area;
    let alpha = (j / (2.0 * prm.bubble_rise_m_s)).min(prm.max_pool_void);
    height_of_volume(prm, volume_below(prm, z) / (1.0 - alpha)) - prm.baf_m
}

/// Free volume of the vessel below height `z` above its bottom, m^3.
pub fn volume_below(prm: &Params, z: f64) -> f64 {
    let a = std::f64::consts::PI * prm.rpv_radius_m * prm.rpv_radius_m;
    let a_core = a - core_rod_area_per_height(prm) - prm.core_other_area_m2;
    let taf = prm.baf_m + prm.active_length_m;
    let z = z.clamp(0.0, prm.rpv_height_m);
    let lower = z.min(prm.baf_m) * a * prm.lower_plenum_free;
    let core = (z.min(taf) - prm.baf_m).max(0.0) * a_core;
    let upper = (z - taf).max(0.0) * a * prm.upper_free;
    lower + core + upper
}

/// Cross-section of fuel rods and channel walls, m^2.
fn core_rod_area_per_height(prm: &Params) -> f64 {
    prm.assemblies * (prm.rods_per_assembly * std::f64::consts::PI * prm.clad_outer_r_m * prm.clad_outer_r_m + 4.0 * (prm.channel_inner_width_m + prm.channel_wall_m) * prm.channel_wall_m)
}

/// Height (above the vessel bottom) at which the free volume below is `v`.
pub fn height_of_volume(prm: &Params, v: f64) -> f64 {
    let (mut a, mut b) = (0.0, prm.rpv_height_m);
    if v <= 0.0 {
        return 0.0;
    }
    if v >= volume_below(prm, b) {
        return b;
    }
    for _ in 0..60 {
        let m = 0.5 * (a + b);
        if volume_below(prm, m) > v {
            b = m;
        } else {
            a = m;
        }
    }
    0.5 * (a + b)
}

/// The plant part of the model (see the module docs).
#[derive(Clone, Debug)]
pub struct Plant {
    sat: SatTable,
    choked: ChokedTable,
    pub mode: Mode,
    pub mechanisms: Mechanisms,
    pub controls: Controls,
    /// Which unit (gh:#1109): its sequence, its containment network, and
    /// whether it has isolation condensers or turbine-driven pumps.
    unit: Unit,
    /// RCIC and HPCI ([`turbine_pump`]; never started on Unit 1).
    pumps: [turbine_pump::TurbinePump; 2],
    /// The fire pumper sprays the drywell (Units 2 and 3's sequences).
    spray_on: bool,
    /// Relief valves held open by batteries brought in (needs no station
    /// DC; Units 2 and 3's sequences).
    held_valves: u8,
    /// The inputs this plant runs with ([`Params`]); fixed at construction.
    prm: Params,
    t_s: f64,
    /// Vessel water and steam: mass, kg, internal energy, J; hydrogen, kg.
    m: f64,
    u: f64,
    h2: f64,
    decay: PlantDecay,
    /// Each element's share of the fission products' decay heat
    /// (`fp_decay`, the committed table).
    fp_shares: fp_decay::ElementShares,
    ic_shell_k: [f64; IC_TRAINS],
    ic_shell_kg: [f64; IC_TRAINS],
    ic_autostarted: bool,
    ic_a_cycling: bool,
    relief_open: bool,
    hot_leak_open: bool,
    msl: msl::SteamLine,
    internals: internals::DomeInternals,
    /// The RPV wall's temperature, K, and the heat it gave the vessel's
    /// contents in the last step, W (`rpv_wall`).
    rpv_wall_k: f64,
    q_rpv_wall_w: f64,
    /// The gas temperature after the internals, K, and the heat they took
    /// in the last step, W.
    dome_gas_k: f64,
    q_internals_w: f64,
    /// Noble gas (Xe) in the vessel's gas, fraction of the core inventory,
    /// and the core's released fraction at the last step (gh:#1040).
    xe_vessel: f64,
    fp_seen: f64,
    /// Cesium (vapour) and UO2 aerosol in the vessel's gas, kg, and the
    /// core's UO2 released at the last step, kg (gh:#1047). No in-vessel
    /// deposition (stated: this overestimates what reaches the
    /// containment).
    cs_vessel_kg: f64,
    i_vessel_kg: f64,
    uo2_vessel_kg: f64,
    /// Cesium and iodine condensed on the dome internals, kg
    /// (`in_vessel_deposition`).
    on_internals_kg: [f64; 2],
    /// The vessel's gas as an aerosol volume (`vessel_aerosol`), and the
    /// time since its last aerosol step, s.
    vessel_rn: rn_volume::RnVolume,
    vessel_rn_clock_s: f64,
    uo2_seen_kg: f64,
    /// The vessel's own solved pressure, Pa, before a failed head pins the
    /// shown pressure to the drywell's.
    p_solved: f64,
    injected_kg: f64,
    vessel: Vessel,
    last_core: CoreResult,
    last_containment: ContainmentResult,
    /// This step's flows, set by `begin`, used by `finish`.
    pending: Pending,
}

#[derive(Clone, Copy, Debug, Default)]
struct Pending {
    q_ic_w: f64,
    /// Steam to the turbine bypass before the MSIVs close, kg/s.
    w_bypass: f64,
    q_ic_train: [f64; IC_TRAINS],
    w_relief: f64,
    w_leak: f64,
    w_inj: f64,
    h2_out: f64,
    /// Noble gas leaving this step through the relief valves and the
    /// leaks, fractions of the core inventory.
    xe_relief: f64,
    xe_leak: f64,
    /// The shares of the vessel's gas leaving through the relief valves
    /// and the leaks this step (what carries every trace species).
    share_relief: f64,
    share_leak: f64,
    /// Decay heat of the fission products the vessel holds, W
    /// (`fp_decay_follows`).
    vessel_fp_w: f64,
    /// Water draining through a failed lower head, kg/s
    /// (`failed_vessel_drain`).
    w_drain: f64,
    /// Heat through the RPV's insulation to the drywell, W (`rpv_wall`).
    rpv_out_w: f64,
    /// The turbine-driven pumps (gh:#1109): water into the vessel, kg/s,
    /// and its enthalpy, J/kg; the turbines' inlet from the vessel, kg/s,
    /// its enthalpy, J/kg (the exhaust goes with the relief stream in the
    /// snapshot); each pump's injection, kg/s (RCIC, HPCI).
    w_pump_inj: f64,
    h_pump_inj: f64,
    w_turbine: f64,
    h_turbine: f64,
    w_pump_each: [f64; 2],
    /// The drywell spray, kg/s.
    w_spray: f64,
}

/// The core and containment snapshots of one step.
pub struct Snapshots {
    pub core: CoreSnapshot,
    pub containment: ContainmentSnapshot,
}

impl Plant {
    pub fn new(mode: Mode) -> Self {
        Self::with_mechanisms(mode, Mechanisms::default())
    }

    /// A fresh plant with some mechanisms ablated (see [`Mechanisms`]).
    pub fn with_mechanisms(mode: Mode, mechanisms: Mechanisms) -> Self {
        Self::with_params(mode, mechanisms, Params::default())
    }

    /// A fresh plant with its own inputs (gh:#1033), for a corrected or
    /// perturbed input without recompiling. [`Plant::parts`] then builds the
    /// core and containment from the same inputs, and every snapshot carries
    /// them to the workers.
    pub fn with_params(mode: Mode, mechanisms: Mechanisms, prm: Params) -> Self {
        Self::with_unit_params(Unit::One, mode, mechanisms, prm)
    }

    /// A fresh plant of `unit` with its own inputs ([`Unit::params`]) and
    /// containment network ([`Unit::network_config`]); under
    /// [`Mode::Historical`] it runs that unit's recorded sequence
    /// ([`units23`] for Units 2 and 3; gh:#1109).
    pub fn for_unit(unit: Unit, mode: Mode, mechanisms: Mechanisms) -> Self {
        Self::with_unit_params(unit, mode, mechanisms, unit.params())
    }

    /// [`Plant::for_unit`] with perturbed or corrected inputs.
    pub fn with_unit_params(unit: Unit, mode: Mode, mechanisms: Mechanisms, prm: Params) -> Self {
        let sat = SatTable::new();
        let choked = ChokedTable::new(&sat);
        let s = sat.at(prm.initial_pressure_pa);
        let v_total = volume_below(&prm, prm.rpv_height_m);
        let v_liq = volume_below(&prm, prm.baf_m + prm.active_length_m + prm.normal_level_above_taf_m);
        let (m_l, m_g) = (v_liq / s.v_f, (v_total - v_liq) / s.v_g);
        let parts = DecayParts {
            mixed_finite: mechanisms.decay_mix,
            capture: mechanisms.capture_correction,
            actinides: mechanisms.actinide_decay,
        };
        let decay = PlantDecay::new(&prm, parts);
        let mut p = Self {
            sat,
            choked,
            mode,
            mechanisms,
            controls: Controls::default(),
            unit,
            pumps: [turbine_pump::TurbinePump::new(turbine_pump::Pump::Rcic), turbine_pump::TurbinePump::new(turbine_pump::Pump::Hpci)],
            spray_on: false,
            held_valves: 0,
            prm,
            t_s: 0.0,
            m: m_l + m_g,
            u: m_l * s.u_f + m_g * s.u_g,
            h2: 0.0,
            decay,
            fp_shares: fp_decay::ElementShares::committed(),
            ic_shell_k: [prm.initial_pool_k; IC_TRAINS],
            ic_shell_kg: [prm.ic_shell_water_kg; IC_TRAINS],
            ic_autostarted: false,
            ic_a_cycling: false,
            relief_open: false,
            hot_leak_open: false,
            msl: msl::SteamLine::new(s.t_k),
            internals: internals::DomeInternals::new(s.t_k),
            rpv_wall_k: s.t_k,
            q_rpv_wall_w: 0.0,
            dome_gas_k: s.t_k,
            q_internals_w: 0.0,
            xe_vessel: 0.0,
            fp_seen: 0.0,
            cs_vessel_kg: 0.0,
            i_vessel_kg: 0.0,
            uo2_vessel_kg: 0.0,
            on_internals_kg: [0.0; 2],
            vessel_rn: rn_volume::RnVolume::new(1.0, containment::rn_sections(), 2),
            vessel_rn_clock_s: 0.0,
            uo2_seen_kg: 0.0,
            p_solved: 0.0,
            injected_kg: 0.0,
            vessel: Vessel::default(),
            last_core: CoreResult::default(),
            last_containment: ContainmentResult::default(),
            pending: Pending::default(),
        };
        p.vessel = p.solve_vessel();
        // What the page shows before the first step: the initial state.
        let t_sat = p.vessel.sat.t_k;
        p.last_core.exit_gas_k = t_sat;
        p.last_core.peak_clad_k = t_sat;
        p.last_core.debris_k = t_sat;
        p.last_core.clad_k = vec![t_sat; self::core::NODES];
        p.last_containment.p_dw_pa = prm.initial_containment_pa;
        p.last_containment.p_ww_pa = prm.initial_containment_pa;
        p.last_containment.t_dw_k = prm.initial_drywell_k;
        p.last_containment.t_pool_k = prm.initial_pool_k;
        p
    }

    /// A fresh core and containment matching a fresh plant: the
    /// containment network by default, the lumped model under
    /// `Mechanisms::without_network_containment` (gh:#1099).
    pub fn parts(&self) -> (Core, PlantContainment) {
        (Core::new(self.vessel.sat.t_k, &self.prm), PlantContainment::new(&self.prm, self.containment_choice()))
    }

    /// What the containment worker builds for this plant: the mechanisms'
    /// choice with this plant's unit.
    pub fn containment_choice(&self) -> ContainmentChoice {
        ContainmentChoice { unit: self.unit, ..self.mechanisms.containment_choice() }
    }

    /// Which unit this plant is.
    pub fn unit(&self) -> Unit {
        self.unit
    }

    /// The RCIC and HPCI pumps' states (never started on Unit 1).
    pub fn pumps(&self) -> &[turbine_pump::TurbinePump; 2] {
        &self.pumps
    }

    /// The inputs this plant runs with.
    pub fn params(&self) -> &Params {
        &self.prm
    }

    pub fn time_s(&self) -> f64 {
        self.t_s
    }

    fn solve_vessel(&self) -> Vessel {
        let prm = self.prm;
        let v_total = volume_below(&prm, prm.rpv_height_m);
        let v = v_total / self.m;
        let u = self.u / self.m;
        match self.sat.mixture(v, u) {
            Some((s, x)) => {
                let v_liq = (1.0 - x) * self.m * s.v_f;
                let gas = (v_total - v_liq).max(1e-6);
                let p_h2 = self.h2 / M_H2 * R_GAS * s.t_k / gas;
                let steam_mol = x * self.m / M_H2O;
                let h2_mol = self.h2 / M_H2;
                Vessel {
                    sat: s,
                    x,
                    p_pa: s.p_pa + p_h2,
                    level_m: height_of_volume(&prm, v_liq) - prm.baf_m,
                    gas_m3: gas,
                    h2_fraction: h2_mol / (h2_mol + steam_mol).max(1e-30),
                    dry: false,
                }
            }
            None => {
                // Boiled dry: superheated steam continued from the saturated
                // vapour (see `SatTable::superheated`), plus the hydrogen's
                // partial pressure. The flows use the saturation state at that
                // pressure; outside what the model was built for, and flagged.
                let (p_steam, t) = self.sat.superheated(v, u);
                let s = self.sat.at(p_steam.min(steam::P_MAX_PA));
                let p_h2 = self.h2 / M_H2 * R_GAS * t / v_total;
                let mol = self.m / M_H2O + self.h2 / M_H2;
                Vessel { sat: s, x: 1.0, p_pa: p_steam + p_h2, level_m: -prm.baf_m, gas_m3: v_total, h2_fraction: (self.h2 / M_H2) / mol, dry: true }
            }
        }
    }

    /// Units 2 and 3's sequence ([`units23`]): the rows due in this step.
    fn apply_units23_history(&mut self) {
        for e in units23::due(self.unit, self.t_s, DT_S) {
            use turbine_pump::Drive;
            match e.kind {
                EventKind::RcicStart => self.pumps[0].start(Drive::Injecting),
                EventKind::RcicStartThrottled => self.pumps[0].start(Drive::Throttled),
                EventKind::RcicLost => self.pumps[0].lose(),
                EventKind::HpciStartThrottled => self.pumps[1].start(Drive::Throttled),
                EventKind::HpciLost => self.pumps[1].lose(),
                EventKind::DcLost | EventKind::StationBlackout => self.controls.dc_power = false,
                EventKind::ReliefHeld(n) => self.held_valves = self.held_valves.max(n),
                EventKind::SprayOn => self.spray_on = true,
                EventKind::SprayOff => self.spray_on = false,
                EventKind::InjectionOn => self.controls.injection = true,
                EventKind::InjectionOff => self.controls.injection = false,
                EventKind::VentOpened => self.controls.vent_ordered = true,
                EventKind::VentClosed => self.controls.vent_ordered = false,
                EventKind::AcLost | EventKind::Recorded | EventKind::Explosion | EventKind::IcStopped | EventKind::IcCycleA => {}
            }
        }
    }

    /// The operators' actions from the historical sequence, at time `t`.
    fn apply_history(&mut self) {
        if self.unit != Unit::One {
            self.apply_units23_history();
            return;
        }
        let t = self.t_s;
        let at = |k: EventKind| EVENTS.iter().filter(|e| e.kind == k).map(|e| e.t_s).collect::<Vec<_>>();
        let crossed = |times: &[f64]| times.iter().any(|&e| t >= e && t < e + DT_S);
        if crossed(&at(EventKind::IcStopped)) {
            self.controls.ic_open = [false; IC_TRAINS];
        }
        if crossed(&at(EventKind::IcCycleA)) {
            self.ic_a_cycling = true;
        }
        if crossed(&at(EventKind::StationBlackout)) {
            self.controls.dc_power = false;
            self.controls.ic_open = [false; IC_TRAINS];
            self.ic_a_cycling = false;
        }
        if crossed(&at(EventKind::InjectionOn)) {
            self.controls.injection = true;
        }
        if crossed(&at(EventKind::InjectionOff)) {
            self.controls.injection = false;
        }
        if crossed(&at(EventKind::VentOpened)) {
            self.controls.vent_ordered = true;
        }
        if self.ic_a_cycling {
            let p = self.vessel.p_pa;
            if p > IC_A_BAND.1 {
                self.controls.ic_open[0] = true;
            } else if p < IC_A_BAND.0 {
                self.controls.ic_open[0] = false;
            }
        }
    }

    /// Start a step: decide the flows from the start-of-step state and hand
    /// out the snapshots.
    pub fn begin(&mut self) -> Snapshots {
        let prm = self.prm;
        if self.mode == Mode::Historical {
            self.apply_history();
        }
        let v = self.vessel;
        let p = v.p_pa;
        let t_sat = v.sat.t_k;
        // Isolation condensers: automatic start on high pressure (needs DC);
        // Unit 1 alone has them.
        if self.unit.has_isolation_condenser() && self.controls.dc_power && !self.ic_autostarted && p >= prm.ic_autostart_pa {
            self.ic_autostarted = true;
            self.controls.ic_open = [true; IC_TRAINS];
        }
        let mut q_ic_train = [0.0; IC_TRAINS];
        for (i, q) in q_ic_train.iter_mut().enumerate() {
            if self.controls.ic_open[i] && self.ic_shell_kg[i] > 0.0 && !v.dry {
                // Hydrogen blankets the tubes: condensation scaled by the
                // steam's share of the gas. [A]
                let blanket = if self.mechanisms.ic_h2_blanketing { 1.0 - v.h2_fraction } else { 1.0 };
                *q = (prm.ic_ua_w_per_k * (t_sat - self.ic_shell_k[i])).max(0.0) * blanket;
            }
        }
        // Relief valves: relief set points with DC, spring safety set points
        // without; held open by hand (needs DC).
        let (open_at, close_at) = if self.controls.dc_power { (prm.relief_open_pa, prm.relief_close_pa) } else { (prm.safety_open_pa, prm.safety_close_pa) };
        if p >= open_at || (self.controls.dc_power && self.controls.relief_held_open) {
            self.relief_open = true;
        } else if p <= close_at {
            self.relief_open = false;
        }
        let p_dw = self.last_containment.p_dw_pa;
        // The steam tables' choked flux, scaled to the hydrogen-steam
        // mixture (2026-10-29, after MELCOR's gas choking: G = rho c
        // f(gamma) from the mixture's own density and sound speed, gamma
        // fixed; MELCOR 2.2 Vol. 2 CVH/FL §6.3). At the same p, T and gamma,
        // G goes as sqrt(M), so by sqrt(M_mix / M_H2O); gamma's change with
        // the mixture is not followed (stated).
        let flux = self.choked.at(p) * if self.mechanisms.mixture_choking { mixture_flux_factor(v.h2_fraction) } else { 1.0 };
        let w_relief = if self.relief_open {
            flux * prm.relief_throat_m2 * prm.relief_valves
        } else if self.held_valves > 0 && !self.last_core.lower_head_failed && p > self.last_containment.p_ww_pa + prm.vent_submergence_pa {
            // Valves held open by batteries brought in (Units 2 and 3),
            // while the vessel is above the pool's back-pressure at the
            // quenchers (the wetwell's plus the submergence head). Once the
            // lower head has failed, the vessel's gas leaves through the
            // breach (`share_leak`) and a held valve passes nothing more.
            // (Found 2026-11-02: without both limits a held valve drained a
            // failed, dry vessel to zero mass, and its state went NaN.)
            flux * prm.relief_throat_m2 * f64::from(self.held_valves).min(prm.relief_valves)
        } else {
            0.0
        };
        if self.mechanisms.hot_leak && self.last_core.exit_gas_k >= prm.hot_leak_gas_k {
            self.hot_leak_open = true;
        }
        if self.mechanisms.msl_creep {
            // Before the first containment step its result is the default.
            let (p_dw_now, t_dw) = if self.last_containment.t_dw_k > 0.0 {
                (p_dw, self.last_containment.t_dw_k)
            } else {
                (prm.initial_containment_pa, prm.initial_drywell_k)
            };
            let t_gas = if self.last_core.exit_gas_k > 0.0 { self.gas_out_k() } else { v.sat.t_k };
            self.msl.step(&prm, w_relief, t_gas, p, p_dw_now, t_dw, DT_S);
        }
        let leak_area = if self.hot_leak_open { prm.hot_leak_m2 } else { 0.0 }
            + if self.last_core.lower_head_failed { prm.breach_m2 } else { 0.0 }
            + if self.msl.failed { msl::SteamLine::break_area_m2(&prm) } else { 0.0 };
        // After the lower head fails the vessel shows the drywell's
        // pressure, but its gas still leaves through the breach on its own
        // (solved) pressure (`failed_vessel_outflow`).
        let p_leak = if self.mechanisms.failed_vessel_outflow && self.last_core.lower_head_failed { self.p_solved } else { p };
        let w_leak = if p_leak > p_dw {
            if p_leak == p { flux * leak_area } else { self.choked.at(p_leak) * leak_area }
        } else {
            0.0
        };
        let w_inj = if self.controls.injection && p < prm.fire_engine_shutoff_pa {
            prm.fire_engine_max_kg_s * (1.0 - (p - P_ATM).max(0.0) / (prm.fire_engine_shutoff_pa - P_ATM)).sqrt()
        } else {
            0.0
        };
        // Gas leaving carries hydrogen in its mass share of the gas space.
        let steam_gas = (v.x * self.m).max(1e-9);
        let h2_share = self.h2 / (self.h2 + steam_gas);
        // Cannot remove more gas than there is.
        let out = (w_relief + w_leak) * DT_S;
        let mut scale = if out > 0.0 { ((steam_gas + self.h2) / out).min(1.0) } else { 1.0 };
        // A dry vessel is all gas, with no water flashing to hold its
        // pressure up, so a path that could pass more in a step than
        // equalises it with the drywell passes the equalising amount
        // (isothermal), the containment's rule for its own stiff paths.
        // Without it a large break emptied the vessel in one step (m = 0,
        // infinite pressure; found 2026-10-29 with a ruptured steam line
        // and a perturbed input).
        if v.dry && out > 0.0 && p_leak > p_dw && p_dw > 0.0 {
            let equalising = (steam_gas + self.h2) * (p_leak - p_dw) / p_leak;
            scale = scale.min(equalising / out);
        }
        let (w_relief, w_leak) = (w_relief * scale, w_leak * scale);
        // The decay heat that has left the core with the released noble
        // gases, alkali metals and halogens (`fp_decay_follows`): their
        // classes' share of the fission products' heat, times the
        // fraction released.
        let (q_noble_w, q_alkali_w, q_halogen_w, q_chalcogen_w) = if self.mechanisms.fp_decay_follows {
            let (p_fp, t) = (self.decay.fp_power_w(), self.decay.t_s());
            let sh = &self.fp_shares;
            let te = if self.mechanisms.te_transport && self.mechanisms.vessel_aerosol { p_fp * sh.share(&["Te", "Se"], t) } else { 0.0 };
            (p_fp * sh.share(&["Xe", "Kr"], t), p_fp * sh.share(&["Cs", "Rb"], t), p_fp * sh.share(&["I", "Br"], t), te)
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };
        let released = self.last_core.fp_volatile_released;
        let decay_w = self.decay.power_w() - released * (q_noble_w + q_alkali_w + q_halogen_w + q_chalcogen_w);
        // The vessel's own share: its gas's noble gas, and the cesium and
        // iodine in its gas and on its internals.
        // A failed vessel's water drains through the breach into the
        // pedestal (`failed_vessel_drain`): an orifice of `breach_diameter_m`
        // under the vessel's own (solved) pressure over the drywell's plus
        // the liquid's own head, at most the liquid there is.
        let w_drain = if self.mechanisms.failed_vessel_drain && self.last_core.lower_head_failed && !v.dry {
            let m_l = ((1.0 - v.x) * self.m).max(0.0);
            let rho_l = 1.0 / v.sat.v_f;
            let head = (self.p_solved - p_dw).max(0.0) + rho_l * 9.81 * (v.level_m + prm.baf_m).max(0.0);
            let area = 0.25 * std::f64::consts::PI * prm.breach_diameter_m * prm.breach_diameter_m;
            (prm.discharge_coefficient * area * (2.0 * rho_l * head).sqrt()).min(m_l / DT_S)
        } else {
            0.0
        };
        let rpv_out_w = if self.mechanisms.rpv_wall { prm.msl_insulation_h * rpv_wall_area_m2(&prm) * (self.rpv_wall_k - self.last_containment.t_dw_k) } else { 0.0 };
        let vessel_fp_w = self.xe_vessel * q_noble_w
            + (self.cs_vessel_kg + self.on_internals_kg[0]) / prm.cs_inventory_kg * q_alkali_w
            + (self.i_vessel_kg + self.on_internals_kg[1]) / prm.i_inventory_kg * q_halogen_w
            + self.vessel_rn.total_kg(rn_volume::TE) / prm.te_inventory_kg * q_chalcogen_w;
        // Before the MSIVs close, the pressure regulator passes steam to the
        // turbine bypass.
        let w_bypass = if self.t_s < prm.msiv_closure_s && !v.dry {
            (prm.bypass_gain_kg_s_pa * (p - prm.initial_pressure_pa)).clamp(0.0, steam_gas / DT_S)
        } else {
            0.0
        };
        // The shares of the vessel's gas leaving this step (well mixed: by
        // mass equals by moles, the composition being uniform). Once the
        // lower head has failed the vessel is open to the drywell, so its
        // trace species pass to the drywell each step.
        let (share_relief, share_leak) = if self.last_core.lower_head_failed {
            (0.0, 1.0)
        } else {
            ((w_relief * DT_S / (steam_gas + self.h2)).min(1.0), (w_leak * DT_S / (steam_gas + self.h2)).min(1.0))
        };
        // The turbine-driven pumps (gh:#1109; Units 2 and 3 only): the L8
        // trip on the start-of-step level, then each pump's flows.
        let mut pf = [turbine_pump::PumpFlows::default(); 2];
        if self.unit != Unit::One {
            let level_taf = v.level_m - prm.active_length_m;
            let (dc, t_now) = (self.controls.dc_power, self.t_s);
            for pump in self.pumps.iter_mut() {
                pump.check_trip(&prm, level_taf, dc, t_now);
            }
            if self.pumps.iter().any(|x| x.drive != turbine_pump::Drive::Off) {
                let swollen = if self.mechanisms.level_swell && !v.dry {
                    swollen_level(&prm, v.level_m, self.last_core.q_pool_w, v.sat.h_g - v.sat.h_f, v.sat.v_g)
                } else {
                    v.level_m
                };
                let inp = turbine_pump::PumpInputs {
                    p_vessel_pa: v.sat.p_pa,
                    h_g: v.sat.h_g,
                    h_f: v.sat.h_f,
                    level_above_taf_m: level_taf,
                    swollen_above_taf_m: swollen - prm.active_length_m,
                    p_exhaust_pa: self.last_containment.p_ww_pa + prm.vent_submergence_pa,
                    p_suction_pa: P_ATM,
                    dc_power: dc,
                    dry: v.dry,
                };
                for (f, pump) in pf.iter_mut().zip(self.pumps.iter()) {
                    *f = pump.flows(&prm, &inp);
                }
            }
        }
        let w_pump_inj: f64 = pf.iter().map(|f| f.inject_kg_s).sum();
        let w_turbine: f64 = pf.iter().map(|f| f.turbine_kg_s).sum();
        // The flows' mass-weighted enthalpies (zero with no flow).
        let per = |e: f64, w: f64| if w > 0.0 { e / w } else { 0.0 };
        let h_pump_inj = per(pf.iter().map(|f| f.inject_kg_s * f.inject_h).sum(), w_pump_inj);
        let h_turbine = per(pf.iter().map(|f| f.turbine_kg_s * f.turbine_h).sum(), w_turbine);
        let h_exhaust = per(pf.iter().map(|f| f.turbine_kg_s * f.exhaust_h).sum(), w_turbine);
        // The fire pumper's drywell spray, on the engine's curve against the
        // drywell's pressure (the same pumper and curve as the injection,
        // [A]).
        let p_dw_spray = self.last_containment.p_dw_pa;
        let w_spray = if self.spray_on && p_dw_spray < prm.fire_engine_shutoff_pa {
            prm.fire_engine_max_kg_s * (1.0 - (p_dw_spray - P_ATM).max(0.0) / (prm.fire_engine_shutoff_pa - P_ATM)).sqrt()
        } else {
            0.0
        };
        self.pending = Pending {
            w_pump_inj,
            h_pump_inj,
            w_turbine,
            h_turbine,
            w_pump_each: [pf[0].inject_kg_s, pf[1].inject_kg_s],
            w_spray,
            w_bypass,
            q_ic_w: q_ic_train.iter().sum(),
            q_ic_train,
            w_relief,
            w_leak,
            w_inj,
            h2_out: (w_relief + w_leak) * h2_share,
            vessel_fp_w,
            w_drain,
            rpv_out_w,
            // The gas leaving takes the noble gas in its share of the gas
            // space (well mixed): that share by mass is the share by
            // moles, the composition being uniform.
            // Once the lower head has failed the vessel is open to the
            // drywell (its pressure is the drywell's, with no flow model),
            // so its noble gas passes to the drywell each step.
            xe_relief: self.xe_vessel * share_relief,
            xe_leak: self.xe_vessel * share_leak,
            share_relief,
            share_leak,
        };
        // The vessel's aerosol volume gives up the same shares
        // (`vessel_aerosol`), split between the two paths.
        let (rn_relief, rn_leak) = if self.mechanisms.vessel_aerosol {
            let out = (share_relief + share_leak).min(1.0);
            let (moved, vapour_kg, on_aerosol_kg) = self.vessel_rn.take(out);
            let all = rn_volume::RnFlow { moved, vapour_kg, on_aerosol_kg };
            if out > 0.0 { (all.scaled(share_relief / (share_relief + share_leak)), all.scaled(share_leak / (share_relief + share_leak))) } else { (all.scaled(0.0), all.scaled(0.0)) }
        } else {
            (rn_volume::RnFlow::default(), rn_volume::RnFlow::default())
        };
        let h_gas = v.sat.h_g;
        // The relief valves' steam, and the turbines' exhaust with it into
        // the pool (gh:#1109; the exhaust line ends under the pool, HRTD
        // Figure 7.1-5a): one stream, its enthalpy the flows' mean. Without
        // a turbine running, the relief stream alone, as before.
        let srv_steam = w_relief * (1.0 - h2_share);
        let (srv_steam, srv_h) = if w_turbine > 0.0 {
            let m = srv_steam + w_turbine;
            (m, (srv_steam * h_gas + w_turbine * h_exhaust) / m)
        } else {
            (srv_steam, h_gas)
        };
        // The level the core sees (gh:#1036): swollen by the vapour the last
        // step raised in the pool, or the collapsed level.
        let core_level = if self.mechanisms.level_swell && !v.dry {
            swollen_level(&prm, v.level_m, self.last_core.q_pool_w, v.sat.h_g - v.sat.h_f, v.sat.v_g)
        } else {
            v.level_m
        };
        Snapshots {
            core: CoreSnapshot { dt_s: DT_S, p_pa: v.sat.p_pa, t_sat_k: t_sat, h_fg: v.sat.h_g - v.sat.h_f, level_m: core_level, decay_w, t_dependent_props: self.mechanisms.temperature_dependent_properties, p_dw_pa: self.last_containment.p_dw_pa, lower_head_wall: self.mechanisms.lower_head_wall, rod_collapse: self.mechanisms.rod_collapse, core_radiation: self.mechanisms.core_radiation, diffusion_limit: self.mechanisms.diffusion_limit, hydrogen_cp_t: self.mechanisms.hydrogen_cp_t, penetrations: self.mechanisms.penetrations, debris_oxidation: self.mechanisms.debris_oxidation, candling: self.mechanisms.candling, debris_quench: self.mechanisms.debris_quench, wet_head_contact: self.mechanisms.wet_head_contact, penetration_coverage: self.mechanisms.penetration_coverage, core_top_radiation: self.mechanisms.core_top_radiation, upper_k: self.internals.t_k, see_through: self.mechanisms.see_through, core_plate: self.mechanisms.core_plate, bottom_radiation: self.mechanisms.bottom_radiation, ring_plates: self.mechanisms.ring_plates, plate_oxidation: self.mechanisms.plate_oxidation, lp_steam_consumed: self.mechanisms.lp_steam_consumed, control_blades: self.mechanisms.control_blades, blade_fusion: self.mechanisms.blade_fusion, blade_diffusion_limit: self.mechanisms.blade_diffusion_limit, b4c_oxidation: self.mechanisms.b4c_oxidation, uo2_candling: self.mechanisms.uo2_candling, molten_pool: self.mechanisms.molten_pool, prm },
            containment: ContainmentSnapshot {
                dt_s: DT_S,
                srv_steam_kg_s: srv_steam,
                srv_h,
                spray_kg_s: w_spray,
                spray_k: prm.injection_temperature_k,
                spray_exchange: self.mechanisms.containment_spray,
                srv_h2_kg_s: w_relief * h2_share,
                leak_steam_kg_s: w_leak * (1.0 - h2_share),
                leak_h: h_gas,
                leak_h2_kg_s: w_leak * h2_share,
                leak_gas_k: self.gas_out_k(),
                vent_ordered: self.controls.vent_ordered,
                ex_vessel_w: self.last_core.ex_vessel_w,
                pedestal_inflow_kg_s: if self.last_core.lower_head_failed { w_inj } else { 0.0 },
                vessel_drain_kg_s: self.pending.w_drain,
                vessel_drain_k: v.sat.t_k,
                gas_cv_t: self.mechanisms.gas_cv_t,
                xe_relief: self.pending.xe_relief,
                xe_leak: self.pending.xe_leak,
                cs_relief_kg: self.cs_vessel_kg * self.pending.share_relief,
                cs_leak_kg: self.cs_vessel_kg * self.pending.share_leak,
                uo2_relief_kg: self.uo2_vessel_kg * self.pending.share_relief,
                uo2_leak_kg: self.uo2_vessel_kg * self.pending.share_leak,
                i_relief_kg: self.i_vessel_kg * self.pending.share_relief,
                i_leak_kg: self.i_vessel_kg * self.pending.share_leak,
                pool_scrubbing: self.mechanisms.pool_scrubbing,
                scrubbing_saturation: self.mechanisms.scrubbing_saturation,
                swarm_scrubbing: self.mechanisms.swarm_scrubbing,
                sectional_classes: self.mechanisms.sectional_classes,
                iodine_partition: self.mechanisms.iodine_partition,
                mixture_vent: self.mechanisms.mixture_vent,
                phoresis: self.mechanisms.phoresis,
                iodine_swarm: self.mechanisms.iodine_swarm,
                condensate_mass: self.mechanisms.condensate_mass,
                q_noble_w,
                q_alkali_w,
                q_halogen_w,
                q_chalcogen_w,
                ww_atmosphere: self.mechanisms.ww_atmosphere,
                dw_wall_hs: self.mechanisms.dw_wall_hs,
                ww_shell: self.mechanisms.ww_shell,
                outflow_enthalpy: self.mechanisms.outflow_enthalpy,
                relief_h2_enthalpy: self.mechanisms.relief_h2_enthalpy,
                srv_gas_k: v.sat.t_k,
                condensate_to_floor: self.mechanisms.condensate_to_floor,
                rpv_wall_w: self.pending.rpv_out_w,
                mcci: self.mechanisms.mcci,
                dw_wall_outer: self.mechanisms.dw_wall_outer,
                pool_layers: self.mechanisms.pool_layers,
                cavity_phases: self.mechanisms.cavity_phases,
                bur_ignition: self.mechanisms.bur_ignition,
                gas_choking: self.mechanisms.gas_choking,
                two_sided_equalising: self.mechanisms.two_sided_equalising,
                burn_dynamics: self.mechanisms.burn_dynamics,
                fdi_lpme: self.mechanisms.fdi_lpme,
                vessel_p_pa: v.sat.p_pa,
                cav_uo2_kg: if self.last_core.lower_head_failed { self.last_core.debris_uo2_kg } else { 0.0 },
                cav_zr_kg: if self.last_core.lower_head_failed { self.last_core.debris_zr_metal_kg } else { 0.0 },
                cav_debris_k: self.last_core.debris_k,
                rn_relief: rn_relief.clone(),
                rn_leak: rn_leak.clone(),
                prm,
            },
        }
    }

    /// The vessel's aerosol volume through one plant step
    /// (`vessel_aerosol`): resized to the gas space, the core's new release
    /// added (aerosol into the smallest section, Cs and I as vapour), and
    /// every aerosol step its condensation, agglomeration and deposition.
    /// Its surfaces: a floor of the vessel's cross-section (the water
    /// surface, or the core plate and debris once dry: settling only, at
    /// the dome's saturation temperature or, dry, the gas's), and the dome
    /// internals at their temperature, with the heat-mass analogy's
    /// coefficient for vapour (as `in_vessel_deposition`) and
    /// thermophoresis from their heat flux, `h (T_gas - T_int)`. Without
    /// `in_vessel_deposition` the surfaces take no vapour. Its totals are
    /// mirrored into the scalar fields the frame reports.
    fn step_vessel_rn(&mut self, prm: &Params, v: &Vessel, core: &CoreResult) {
        let gas_m3 = v.gas_m3.max(1.0);
        let rho_p = aerosol::Medium::air(300.0, 1.013e5, 1.8e-5).rho_p;
        self.vessel_rn.set_volume(gas_m3);
        self.vessel_rn.condensate_mass = self.mechanisms.condensate_mass;
        let new = core.fp_volatile_released - self.fp_seen;
        // Tellurium shares the row too (`te_transport`; off, none is
        // carried and its heat stays in the core).
        let te = if self.mechanisms.te_transport { new * prm.te_inventory_kg } else { 0.0 };
        self.vessel_rn.add(core.uo2_released_kg - self.uo2_seen_kg, rho_p, [new * prm.cs_inventory_kg, new * prm.i_inventory_kg, te]);
        self.vessel_rn_clock_s += DT_S;
        if self.vessel_rn_clock_s >= containment::RN_STEP_S {
            let h = self.vessel_rn_clock_s;
            self.vessel_rn_clock_s = 0.0;
            let n = self.vessel_rn.sections.v.len();
            let t_g = self.dome_gas_k;
            let medium = aerosol::Medium::air(t_g, v.p_pa, containment::air_viscosity(t_g));
            let n_s = v.x * self.m / 0.018015;
            let n_h = self.h2 / 0.002016;
            let n_t = (n_s + n_h).max(1e-30);
            let gases = [(vapour::BulkGas::Steam, n_s / n_t), (vapour::BulkGas::H2, n_h / n_t)];
            let rho = (v.x * self.m + self.h2).max(1e-9) / gas_m3;
            let k = if self.mechanisms.in_vessel_deposition { prm.internals_h / (rho * 2000.0) } else { 0.0 };
            let t_int = self.internals.t_k;
            let grad_t = prm.internals_h * (t_g - t_int) / containment::air_conductivity(t_int);
            let thermo: Vec<f64> = (0..n).map(|i| medium.thermophoretic_velocity(self.vessel_rn.sections.diameter(i), grad_t, t_int)).collect();
            let floor_t = if v.dry { t_g } else { v.sat.t_k };
            let surfaces = [
                rn_volume::RnSurface {
                    surface: aerosol::Surface { area: std::f64::consts::PI * prm.rpv_radius_m * prm.rpv_radius_m, facing: aerosol::Facing::Floor, phoretic: vec![0.0; n] },
                    t_k: floor_t,
                    k_vapour: k,
                },
                rn_volume::RnSurface { surface: aerosol::Surface { area: prm.internals_area_m2, facing: aerosol::Facing::Wall, phoretic: thermo }, t_k: t_int, k_vapour: k },
            ];
            self.vessel_rn.step(&medium, &gases, &surfaces, h);
        }
        let r = &self.vessel_rn;
        self.cs_vessel_kg = r.vapour_kg[rn_volume::CS] + r.on_aerosol(rn_volume::CS);
        self.i_vessel_kg = r.vapour_kg[rn_volume::IODINE] + r.on_aerosol(rn_volume::IODINE);
        // The particles' mass less the condensed classes on them (zero
        // without `condensate_mass`, the condensate being weightless).
        let condensed = if r.condensate_mass { (0..rn_volume::N_CLASSES).map(|c| r.on_aerosol(c)).sum() } else { 0.0 };
        self.uo2_vessel_kg = (r.aerosol_kg(rho_p) - condensed).max(0.0);
        self.on_internals_kg = [0, 1].map(|c| r.on_surface_kg.iter().map(|x| x[c]).sum::<f64>());
        // (Tellurium is not mirrored into the frame; it is in `vessel_rn`.)
    }

    /// Finish the step with the core's and the containment's results.
    pub fn finish(&mut self, core: CoreResult, containment: ContainmentResult) {
        let prm = self.prm;
        let d = self.pending;
        let v = self.vessel;
        let h_g = self.steam_out_h();
        let h_inj = 4186.0 * (prm.injection_temperature_k - 273.15);
        let gas_out = d.w_relief + d.w_leak;
        let steam_out = gas_out - d.h2_out + d.w_bypass;
        // After the lower head fails the injected water pours through the
        // breach onto the drywell floor (the containment's pedestal).
        let w_inj_rpv = if self.last_core.lower_head_failed { 0.0 } else { d.w_inj };
        self.m += (w_inj_rpv - steam_out - core.steam_used_kg_s - d.w_drain) * DT_S;
        // Hydrogen leaving takes the enthalpy above 298 K that the core's
        // gas stream gave it, which reached the water through `q_fluid_w`
        // (gh:#1046): at saturation through the relief valves, at the core
        // exit gas temperature through the leak (as the containment
        // receives it). ~~Wetwell gas has no energy equation, so the relief
        // path's share is not delivered there (stated limitation).~~ Since
        // 2026-10-29 the wetwell atmosphere receives it
        // (`relief_h2_enthalpy`, with `ww_atmosphere`).
        let to_relief = if gas_out > 0.0 { d.w_relief / gas_out } else { 0.0 };
        let h2_enthalpy_out = h2_exit_enthalpy(d.h2_out, to_relief, v.sat.t_k, self.gas_out_k(), self.mechanisms.hydrogen_cp_t);
        self.u += (core.q_fluid_w - d.q_ic_w + w_inj_rpv * h_inj - steam_out * h_g - core.steam_used_kg_s * h_g - h2_enthalpy_out - d.w_drain * v.sat.h_f) * DT_S;
        // The turbine-driven pumps (gh:#1109): the turbines take steam (and
        // any carried-over water) at the inlet's enthalpy, the pumps return
        // water at the tank's enthalpy plus their shaft work.
        if d.w_turbine > 0.0 || d.w_pump_inj > 0.0 {
            self.m += (d.w_pump_inj - d.w_turbine) * DT_S;
            self.u += (d.w_pump_inj * d.h_pump_inj - d.w_turbine * d.h_turbine) * DT_S;
        }
        // The dome internals take heat from (or give it back to) the gas
        // leaving the core, out of the vessel's energy.
        if self.mechanisms.dome_internals {
            let cp_h2 = gas::cp_h2_per_kg(core.exit_gas_k, self.mechanisms.hydrogen_cp_t);
            let (t_out, q) = self.internals.step(&prm, core.exit_steam_kg_s, core.h2_kg_s, cp_h2, core.exit_gas_k, v.p_pa, DT_S);
            self.dome_gas_k = t_out;
            // And their natural convection to the saturated dome, which the
            // vessel takes back (`internals_heat_loss`).
            // Only while the vessel holds water: a dry dome is a trace of
            // superheated gas that takes almost no heat. Without this limit
            // (found 2026-10-29 in the no-radiation ablation) the internals'
            // heat kept a dry vessel's pressure above the drywell's, its gas
            // drained out through the leak until its mass reached zero, and
            // its pressure then ran to 227 MPa (gh:#1051).
            let q_loss = if self.mechanisms.internals_heat_loss && !v.dry { self.internals.lose(&prm, v.sat.t_k, DT_S) } else { 0.0 };
            self.q_internals_w = q - q_loss;
            self.u -= (q - q_loss) * DT_S;
        }
        // The RPV wall (`rpv_wall`): its insulation loses heat to the
        // drywell; inside, while the vessel holds water, it exchanges with
        // the saturated contents, by boiling (`boiling_h`) below the level
        // and above it when the wall is colder than the steam (condensing),
        // by natural convection (`internals_loss_h`) above it otherwise;
        // implicit, as the internals' loss. A dry vessel's gas takes nothing
        // from it (the #1051 limit the internals carry, stated).
        self.q_rpv_wall_w = 0.0;
        if self.mechanisms.rpv_wall {
            let area = rpv_wall_area_m2(&prm);
            let mass = area * prm.rpv_wall_m * 7850.0;
            let mut h = core::steel_h(self.rpv_wall_k) - d.rpv_out_w * DT_S / mass;
            self.rpv_wall_k = core::steel_t(h);
            if !v.dry {
                let wet = ((v.level_m + prm.baf_m) / prm.rpv_height_m).clamp(0.0, 1.0);
                let h_above = if self.rpv_wall_k < v.sat.t_k { prm.boiling_h } else { prm.internals_loss_h };
                let ha = area * (wet * prm.boiling_h + (1.0 - wet) * h_above);
                let t = self.rpv_wall_k;
                let c = mass * (core::steel_h(t + 0.5) - core::steel_h(t - 0.5));
                let t_new = (c * t + DT_S * ha * v.sat.t_k) / (c + DT_S * ha);
                let q = ha * (t_new - v.sat.t_k);
                h -= q * DT_S / mass;
                self.rpv_wall_k = core::steel_t(h);
                self.u += q * DT_S;
                self.q_rpv_wall_w = q;
            }
            // The internals radiate to the wall (`internals_radiation`):
            // MELCOR's gray pair (HS RM Eq 2-96, view factor 1, equal
            // areas), each surface at MELCOR's default steel emissivity
            // (COR C1102), at most what brings the two to one temperature
            // in the step.
            if self.mechanisms.internals_radiation {
                let (ti, tw) = (self.internals.t_k, self.rpv_wall_k);
                let e = 1.0 / (1.0 / core::steel_emissivity(ti) + 1.0 / core::steel_emissivity(tw) - 1.0);
                let q = 5.670_374e-8 * e * prm.internals_envelope_m2 * (ti.r_powi(4) - tw.r_powi(4));
                let ci = prm.internals_kg * (core::steel_h(ti + 0.5) - core::steel_h(ti - 0.5));
                let cw = mass * (core::steel_h(tw + 0.5) - core::steel_h(tw - 0.5));
                let q_max = (ti - tw).abs() / (1.0 / ci + 1.0 / cw) / DT_S;
                let q = q.clamp(-q_max, q_max);
                self.internals.heat(&prm, -q, DT_S);
                self.rpv_wall_k = core::steel_t(core::steel_h(self.rpv_wall_k) + q * DT_S / mass);
            }
        }
        // Onto the internals, which hold the vessel's deposit: heat put in
        // a dry vessel's few kilograms of gas would drive its pressure up
        // and drain it through the leak (the #1051 artefact; found when this
        // first went into the vessel's fluid). The gas takes it from the
        // internals by their coupling.
        self.internals.heat(&prm, d.vessel_fp_w, DT_S);
        // And the core's radiation up to them (`core_top_radiation`).
        self.internals.heat(&prm, core.q_upper_w, DT_S);
        self.h2 = (self.h2 + (core.h2_kg_s - d.h2_out) * DT_S).max(0.0);
        // Noble gas: out with the gas, in from the fuel.
        let out = (d.share_relief + d.share_leak).min(1.0);
        if self.mechanisms.vessel_aerosol {
            self.step_vessel_rn(&prm, &v, &core);
        } else {
        self.cs_vessel_kg = self.cs_vessel_kg * (1.0 - out) + (core.fp_volatile_released - self.fp_seen) * prm.cs_inventory_kg;
        // Iodine shares cesium's CORSOR-M row (Xe, Cs, I2, Te: one row).
        self.i_vessel_kg = self.i_vessel_kg * (1.0 - out) + (core.fp_volatile_released - self.fp_seen) * prm.i_inventory_kg;
        self.uo2_vessel_kg = self.uo2_vessel_kg * (1.0 - out) + (core.uo2_released_kg - self.uo2_seen_kg);
        // Cesium and iodine vapour condense on the dome internals at their
        // temperature (TRAP-MELT2's step, `vapour::condense`): the internals'
        // area, and a mass-transfer coefficient from their heat-transfer
        // coefficient by the heat-mass analogy, `k = h/(rho c_p)` (Lewis
        // number about 1, steam's c_p, stated). The aerosol passes through
        // the vessel without dynamics, so vapour onto in-vessel aerosol is
        // not modelled (stated). ~~Result (2026-10-29): the internals hold up
        // to 19.4 kg of cesium (5.45 h), then reach 1005 K and the deposit
        // re-evaporates and leaves; by 26 h none remains. The internals
        // lump has no heat-loss path once gas stops flowing past it (no
        // radiation to the vessel wall, no conduction): a stated artefact
        // that keeps them hot, and real internals would keep part of it.~~
        // **CORRECTED 2026-10-29:** on the current model the internals hold
        // 19.4 kg of cesium from about 8 h to 26 h and none re-evaporates;
        // since `internals_radiation` they also radiate to the RPV wall and
        // settle at about 812 K at 26 h (1332 K without it), where the
        // radiation balances their deposit's decay heat.
        if self.mechanisms.in_vessel_deposition {
            let gas_m3 = v.gas_m3.max(1.0);
            let rho = (v.x * self.m + self.h2).max(1e-9) / gas_m3;
            let k = prm.internals_h / (rho * 2000.0);
            let t_s = self.internals.t_k;
            for (c, class) in [vapour::FpClass::Cs, vapour::FpClass::I2].iter().enumerate() {
                let vap = if c == 0 { self.cs_vessel_kg } else { self.i_vessel_kg };
                let mut surf = [vapour::CondensingSurface { ak: prm.internals_area_m2 * k, c_sat: class.saturation_concentration(t_s), mass: self.on_internals_kg[c] }];
                // An empty surface above the gas's concentration takes no part.
                let mut left = vap;
                if surf[0].mass > 0.0 || surf[0].c_sat < vap / gas_m3 {
                    left = vapour::condense(vap / gas_m3, gas_m3, &mut surf, DT_S) * gas_m3;
                    if surf[0].mass < 0.0 {
                        left += surf[0].mass;
                        surf[0].mass = 0.0;
                    }
                }
                self.on_internals_kg[c] = surf[0].mass;
                if c == 0 {
                    self.cs_vessel_kg = left;
                } else {
                    self.i_vessel_kg = left;
                }
            }
        }
        }
        self.uo2_seen_kg = core.uo2_released_kg;
        self.xe_vessel = (self.xe_vessel - d.xe_relief - d.xe_leak).max(0.0) + (core.fp_volatile_released - self.fp_seen);
        self.fp_seen = core.fp_volatile_released;
        self.injected_kg += d.w_inj * DT_S;
        for i in 0..IC_TRAINS {
            let q = d.q_ic_train[i] * DT_S;
            if q <= 0.0 {
                continue;
            }
            let to_boil = (373.15 - self.ic_shell_k[i]).max(0.0) * self.ic_shell_kg[i] * 4186.0;
            if q <= to_boil {
                self.ic_shell_k[i] += q / (self.ic_shell_kg[i] * 4186.0);
            } else {
                self.ic_shell_k[i] = 373.15;
                self.ic_shell_kg[i] = (self.ic_shell_kg[i] - (q - to_boil) / 2.257e6).max(0.0);
            }
        }
        self.decay.advance(DT_S);
        self.t_s += DT_S;
        self.last_core = core;
        self.last_containment = containment;
        self.vessel = self.solve_vessel();
        self.p_solved = self.vessel.p_pa;
        // Once the lower head has failed the vessel is open to the drywell:
        // its pressure is the drywell's (no back-flow model; what is left
        // inside is a few tonnes of steam and gas). ~~Nothing leaves it
        // then.~~ Since 2026-10-29 its gas leaves through the breach on the
        // pressure it would have (`p_solved`), held at the drywell's by the
        // dry vessel's equalising cap; before, the gas made after the
        // failure (its steam; almost no hydrogen is made then) stayed in
        // the vessel.
        if self.last_core.lower_head_failed {
            self.vessel.p_pa = self.last_containment.p_dw_pa;
        }
    }

    /// Specific enthalpy of the steam leaving the vessel (and taken by
    /// oxidation), J/kg: saturated vapour's while there is water; once dry,
    /// the superheated gas's own, `u/m + p_steam V/m`. ~~h_g always~~
    /// **Corrected 2026-10-29**: steam leaving a dry vessel at h_g left the
    /// superheat behind, so the specific energy grew without bound as a
    /// break drained it (found when a perturbed input sent the pressure to
    /// infinity).
    fn steam_out_h(&self) -> f64 {
        let v = &self.vessel;
        if v.dry {
            self.u / self.m + v.sat.p_pa * volume_below(&self.prm, self.prm.rpv_height_m) / self.m
        } else {
            v.sat.h_g
        }
    }

    /// The temperature of the gas leaving the vessel through the steam
    /// lines and leaks, K: after the dome internals, or the core's exit gas
    /// without them.
    pub fn gas_out_k(&self) -> f64 {
        if self.mechanisms.dome_internals { self.dome_gas_k } else { self.last_core.exit_gas_k }
    }

    /// One whole step with the core and containment advanced here, in
    /// order: what the one-worker mode (and natively, the engine thread)
    /// runs.
    pub fn step_inline(&mut self, core: &mut Core, containment: &mut PlantContainment) {
        let s = self.begin();
        let c = core.step(&s.core);
        let k = containment.step(&s.containment);
        self.finish(c, k);
    }

    pub fn frame(&self) -> Frame {
        Frame {
            t_s: self.t_s,
            mode_free: self.mode == Mode::FreePlay,
            controls: self.controls,
            vessel: self.vessel,
            decay_w: self.decay.power_w(),
            q_ic_w: self.pending.q_ic_w,
            ic_shell_k: self.ic_shell_k,
            ic_shell_kg: self.ic_shell_kg,
            relief_open: self.relief_open,
            w_relief_kg_s: self.pending.w_relief,
            w_bypass_kg_s: self.pending.w_bypass,
            w_leak_kg_s: self.pending.w_leak,
            w_injection_kg_s: self.pending.w_inj,
            injected_kg: self.injected_kg,
            rpv_h2_kg: self.h2,
            hot_leak_open: self.hot_leak_open,
            msl: self.msl,
            internals_k: self.internals.t_k,
            dome_gas_k: self.gas_out_k(),
            xe_vessel: self.xe_vessel,
            cs_vessel_kg: self.cs_vessel_kg,
            i_vessel_kg: self.i_vessel_kg,
            on_internals_kg: self.on_internals_kg,
            core: self.last_core.clone(),
            containment: self.last_containment,
            w_rcic_kg_s: self.pending.w_pump_each[0],
            w_hpci_kg_s: self.pending.w_pump_each[1],
            w_turbine_kg_s: self.pending.w_turbine,
            w_spray_kg_s: self.pending.w_spray,
        }
    }
}

fn b(x: bool) -> f64 {
    if x { 1.0 } else { 0.0 }
}

impl Frame {
    pub fn write(&self, w: &mut Writer) {
        let c = &self.controls;
        let v = &self.vessel;
        w.f(&[
            self.t_s,
            b(self.mode_free),
            b(c.ic_open[0]),
            b(c.ic_open[1]),
            b(c.dc_power),
            b(c.relief_held_open),
            b(c.injection),
            b(c.vent_ordered),
            v.sat.p_pa,
            v.sat.t_k,
            v.x,
            v.p_pa,
            v.level_m,
            v.gas_m3,
            v.h2_fraction,
            b(v.dry),
            self.decay_w,
            self.q_ic_w,
            self.ic_shell_k[0],
            self.ic_shell_k[1],
            self.ic_shell_kg[0],
            self.ic_shell_kg[1],
            b(self.relief_open),
            self.w_relief_kg_s,
            self.w_bypass_kg_s,
            self.w_leak_kg_s,
            self.w_injection_kg_s,
            self.injected_kg,
            self.rpv_h2_kg,
            b(self.hot_leak_open),
            self.msl.wall_k,
            self.msl.damage,
            b(self.msl.failed),
            self.internals_k,
            self.dome_gas_k,
            self.xe_vessel,
            self.cs_vessel_kg,
            self.i_vessel_kg,
            self.on_internals_kg[0],
            self.on_internals_kg[1],
        ]);
        self.core.write(w);
        self.containment.write(w);
        w.f(&[self.w_rcic_kg_s, self.w_hpci_kg_s, self.w_turbine_kg_s, self.w_spray_kg_s]);
    }
    pub fn read(r: &mut Reader) -> Self {
        let t_s = r.f();
        let mode_free = r.f() != 0.0;
        let controls = Controls { ic_open: [r.f() != 0.0, r.f() != 0.0], dc_power: r.f() != 0.0, relief_held_open: r.f() != 0.0, injection: r.f() != 0.0, vent_ordered: r.f() != 0.0 };
        let mut sat = Sat { p_pa: r.f(), t_k: r.f(), v_f: 0.0, v_g: 0.0, u_f: 0.0, u_g: 0.0, h_f: 0.0, h_g: 0.0 };
        sat.v_f = f64::NAN;
        let vessel = Vessel { sat, x: r.f(), p_pa: r.f(), level_m: r.f(), gas_m3: r.f(), h2_fraction: r.f(), dry: r.f() != 0.0 };
        Self {
            t_s,
            mode_free,
            controls,
            vessel,
            decay_w: r.f(),
            q_ic_w: r.f(),
            ic_shell_k: [r.f(), r.f()],
            ic_shell_kg: [r.f(), r.f()],
            relief_open: r.f() != 0.0,
            w_relief_kg_s: r.f(),
            w_bypass_kg_s: r.f(),
            w_leak_kg_s: r.f(),
            w_injection_kg_s: r.f(),
            injected_kg: r.f(),
            rpv_h2_kg: r.f(),
            hot_leak_open: r.f() != 0.0,
            msl: msl::SteamLine { wall_k: r.f(), damage: r.f(), failed: r.f() != 0.0 },
            internals_k: r.f(),
            dome_gas_k: r.f(),
            xe_vessel: r.f(),
            cs_vessel_kg: r.f(),
            i_vessel_kg: r.f(),
            on_internals_kg: [r.f(), r.f()],
            core: CoreResult::read(r),
            containment: ContainmentResult::read(r),
            w_rcic_kg_s: r.f(),
            w_hpci_kg_s: r.f(),
            w_turbine_kg_s: r.f(),
            w_spray_kg_s: r.f(),
        }
    }
}

/// The CSV the headless mode prints: a stable header, fixed precision.
/// The RPV wall's inner area above the lower head, m^2: the cylinder and
/// a hemispherical top head (`rpv_wall`).
fn rpv_wall_area_m2(prm: &Params) -> f64 {
    let r = prm.rpv_radius_m;
    2.0 * std::f64::consts::PI * r * prm.rpv_height_m + 2.0 * std::f64::consts::PI * r * r
}

pub const CSV_HEADER: &str = "t_s,p_rpv_mpa,level_m,decay_mw,q_ic_mw,w_relief_kg_s,w_injection_kg_s,peak_clad_k,exit_gas_k,h2_kg,relocated,p_dw_mpa,p_ww_mpa,t_pool_k,rb_h2_fraction,flags";

pub fn csv_row(f: &Frame) -> String {
    let flags = (f.controls.dc_power as u32)
        | (f.controls.ic_open[0] as u32) << 1
        | (f.controls.ic_open[1] as u32) << 2
        | (f.relief_open as u32) << 3
        | (f.controls.injection as u32) << 4
        | (f.containment.vent_flowing as u32) << 5
        | (f.hot_leak_open as u32) << 6
        | (f.core.lower_head_failed as u32) << 7
        | (f.containment.flange_leaking as u32) << 8
        | (f.containment.rb_exploded as u32) << 9
        | (f.vessel.dry as u32) << 10
        | (f.msl.failed as u32) << 11;
    format!(
        "{:.0},{:.6},{:.4},{:.4},{:.4},{:.3},{:.3},{:.2},{:.2},{:.3},{:.4},{:.6},{:.6},{:.3},{:.5},{}",
        f.t_s,
        f.vessel.p_pa / 1e6,
        f.vessel.level_m,
        f.decay_w / 1e6,
        f.q_ic_w / 1e6,
        f.w_relief_kg_s,
        f.w_injection_kg_s,
        f.core.peak_clad_k,
        f.core.exit_gas_k,
        f.core.h2_total_kg,
        f.core.relocated,
        f.containment.p_dw_pa / 1e6,
        f.containment.p_ww_pa / 1e6,
        f.containment.t_pool_k,
        f.containment.rb_h2_fraction,
        flags
    )
}

/// The headless run: `steps` steps of the historical sequence, one CSV row
/// every `every` steps. Deterministic: no clock, no randomness, no I/O.
///
/// Every mechanism on, so the containment is the network (gh:#1099);
/// [`headless_csv_with`] runs any ablation.
pub fn headless_csv(steps: usize, every: usize) -> String {
    headless_csv_with(Mechanisms::default(), steps, every)
}

/// [`headless_csv`] with `mechanisms` (gh:#1099: the lumped containment's
/// trace is `headless_csv_with(Mechanisms::default().without_network_containment(), ..)`).
///
/// # Panics
///
/// If the containment network fails (a frozen containment would print a
/// trace that looks valid; [`PlantContainment`]).
pub fn headless_csv_with(mechanisms: Mechanisms, steps: usize, every: usize) -> String {
    let mut plant = Plant::with_mechanisms(Mode::Historical, mechanisms);
    let (mut core, mut cont) = plant.parts();
    let mut out = String::from(CSV_HEADER);
    out.push('\n');
    for i in 0..steps {
        plant.step_inline(&mut core, &mut cont);
        if let Some(e) = cont.error() {
            panic!("{e}");
        }
        if (i + 1) % every.max(1) == 0 {
            out.push_str(&csv_row(&plant.frame()));
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The control blades' B4C over 6 h (`b4c_oxidation`, MELCOR COR RM
    /// §2.5.9): 97 x 4.7 kg, capped at 2 %, so at most 9.12 kg reacts.
    /// Results (2026-10-30): 8.862 kg, as CO 4.46, CO2 152.95 and CH4 2.98
    /// mol (the steam-rich core favours CO2), about 2.6 kg of H2; with UO2
    /// carried by candling Zr (the same day) 8.821 kg, CO 4.05, CO2 152.51,
    /// CH4 3.08 mol.
    ///
    /// **Since gh:#1099 on the lumped containment**, explicitly: these pins
    /// were measured on it and keep their value there.
    #[test]
    fn the_blades_b4c_reacts_up_to_melcors_cap() {
        let mut plant = Plant::with_mechanisms(Mode::Historical, Mechanisms::default().without_network_containment());
        let (mut core, mut cont) = plant.parts();
        for _ in 0..21600usize {
            plant.step_inline(&mut core, &mut cont);
        }
        let (kg, gases) = core.b4c_tally();
        assert!(kg <= 97.0 * 4.7 * 0.02 + 1e-9 && (kg - 8.821).abs() < 1e-3, "{kg}");
        assert!((gases[1] - 152.51).abs() < 0.01 && gases[0] < 5.0 && gases[2] < 3.5, "{gases:?}");
    }

    /// Debris falling through the pedestal's water (`fdi_lpme`, MELCOR FDI
    /// RM §2.2) over the first 8 h. Results (2026-10-30): the first debris,
    /// 20.1 t of UO2 and 6.5 t of Zr, leaves the head at 5.344 h at 1350 K,
    /// solid (below Zr's 2098 K), so it does not break up; at 1.49 m/s
    /// through 0.93 m of water it cools 2 K and raises 8.6 kg of steam.
    /// Later packets of 2.2 t arrive hotter (to 2044 K) and slower; 63.0
    /// kg in all by 7.14 h, after which the vessel sits below the drywell
    /// and MELCOR's Eq 6-34 gives no velocity. No ejection is fast enough
    /// for MELCOR's high-pressure model. Predicted 0.05-0.5 t, for molten
    /// debris near 2800 K (missed low: the debris that leaves is solid).
    /// With UO2 carried by candling Zr (the same day) the head fails at
    /// 5.484 h: 8.07 kg at once, 74.75 kg by 8 h.
    ///
    /// **Since gh:#1099 on the lumped containment**, explicitly (the pins'
    /// model); the network's FDI is `the_network_plant_over_30_hours`'s.
    #[test]
    fn the_debris_falls_through_the_pedestal_water() {
        let mut plant = Plant::with_mechanisms(Mode::Historical, Mechanisms::default().without_network_containment());
        let (mut core, mut cont) = plant.parts();
        let mut first = None;
        for i in 0..28800usize {
            plant.step_inline(&mut core, &mut cont);
            if first.is_none() && plant.frame().containment.fdi_steam_kg > 0.0 {
                first = Some(((i + 1) as f64 / 3600.0, plant.frame().containment.fdi_steam_kg));
            }
        }
        let (h, kg) = first.unwrap();
        assert!((h - 5.4844).abs() < 1e-3 && (kg - 8.07).abs() < 0.01, "{h} {kg}");
        let k = plant.frame().containment;
        assert!((k.fdi_steam_kg - 74.75).abs() < 0.01 && !k.fdi_hpme, "{k:?}");
    }

    /// The core and containment work from snapshots that cross a worker
    /// boundary as flat f64s; that gives the in-place step bit for bit.
    /// Methodology: run 2000 steps of the historical sequence in place, and
    /// again with every snapshot and result written to the wire format and
    /// read back on the other side (what the three-worker mode does); compare
    /// every field of the frame after every step with `==` on the f64s.
    ///
    /// **Since gh:#1099 (2026-11-02) the containment is the network**, whose
    /// state has no wire form: the containment worker holds it
    /// ([`ContainmentWorker`]) and only the snapshot and the result cross;
    /// the core still crosses with its whole state. Predicted before the
    /// run: bit-identical (the worker builds its network from the
    /// snapshot's round-tripped `Params`, the same bits). The lumped
    /// containment's state round trip is
    /// `three_workers_are_one_worker_on_the_lumped_containment`.
    #[test]
    fn three_workers_are_one_worker() {
        let f = one_worker_against_three(Params::default(), 2000);
        assert!(f.t_s == 2000.0);
    }

    /// The lumped containment (`without_network_containment`) still crosses
    /// with its whole state each step, bit for bit (the pre-gh:#1099
    /// protocol, kept for the ablation).
    #[test]
    fn three_workers_are_one_worker_on_the_lumped_containment() {
        let mech = Mechanisms::default().without_network_containment();
        let mut a = Plant::with_mechanisms(Mode::Historical, mech);
        let (mut ca, mut ka) = a.parts();
        let mut b = Plant::with_mechanisms(Mode::Historical, mech);
        let (mut cb, _) = b.parts();
        let mut kb = Containment::new(&Params::default());
        assert!(!ka.is_network());
        for _ in 0..2000 {
            a.step_inline(&mut ca, &mut ka);
            let s = b.begin();
            let mut w = Writer::default();
            s.containment.write(&mut w);
            let cont_snap = ContainmentSnapshot::read(&mut Reader::new(&w.0));
            let mut w = Writer::default();
            s.core.write(&mut w);
            let core_snap = CoreSnapshot::read(&mut Reader::new(&w.0));
            let (core_res, cont_res);
            (cb, core_res) = remote_core(&cb, &core_snap);
            (kb, cont_res) = remote_containment(&kb, &cont_snap);
            b.finish(core_res, cont_res);
            let (mut wa, mut wb) = (Writer::default(), Writer::default());
            a.frame().write(&mut wa);
            b.frame().write(&mut wb);
            assert!(wa.0.iter().zip(&wb.0).all(|(x, y)| x.to_bits() == y.to_bits()), "diverged at t = {}", a.frame().t_s);
        }
    }

    /// **The network is the default containment** (gh:#1099), pinned as
    /// `the_coupled_physics_is_on_by_default` pins its physics: a plant
    /// built the ordinary way gets the network, the ablation gets the
    /// lumped model, and the worker builds what the plant asks for.
    #[test]
    fn the_containment_network_is_the_default() {
        assert!(Mechanisms::default().network_containment);
        let p = Plant::new(Mode::Historical);
        let (_, k) = p.parts();
        assert!(k.is_network() && k.error().is_none());
        assert!(matches!(k.model(), ContainmentModel::Network(_)));
        let lumped = Plant::with_mechanisms(Mode::Historical, Mechanisms::default().without_network_containment());
        assert!(matches!(lumped.parts().1.model(), ContainmentModel::Lumped(_)));
        let mut p = p;
        let s = p.begin();
        let mut w = ContainmentWorker::default();
        let reply = w.handle(&ContainmentWorker::request(0, p.mechanisms.containment_choice(), &s.containment));
        assert!(ContainmentWorker::read_reply(&reply).1 && w.held().is_some_and(PlantContainment::is_network));
        // The plant's mechanisms reach the network's.
        let m = Mechanisms::default().without_mcci().without_pool_layers();
        let k = Plant::with_mechanisms(Mode::Historical, m).parts().1;
        let ContainmentModel::Network(n) = k.model() else { panic!("not the network") };
        assert!(!n.mechanisms().cavity && !n.mechanisms().pool_layers && n.pool_layer_count() == 1);
    }

    /// Changed inputs (gh:#1033) reach every part: a plant built with
    /// [`Plant::with_params`] reports them, they cross the wire unchanged,
    /// one worker still equals three with them, and the run differs from
    /// the default one (one input per part: the core's boiling coefficient,
    /// the containment's initial pressure, the plant's IC conductance). Not
    /// the drywell volume: in the first 2000 s nothing enters the drywell,
    /// and its nitrogen is set from the initial pressure, so its pressure
    /// does not depend on the volume (checked; correct, not a defect).
    #[test]
    fn changed_inputs_reach_every_part() {
        let d = Params::default();
        let prm = Params { boiling_h: 0.9 * d.boiling_h, initial_containment_pa: 1.05 * d.initial_containment_pa, ic_ua_w_per_k: 0.8 * d.ic_ua_w_per_k, ..d };
        let mut w = Writer::default();
        prm.write(&mut w);
        assert_eq!(Params::read(&mut Reader::new(&w.0)), prm);
        assert_eq!(Plant::with_params(Mode::Historical, Mechanisms::default(), prm).params(), &prm);
        let changed = one_worker_against_three(prm, 2000);
        let default = one_worker_against_three(d, 2000);
        assert_ne!(changed.core.peak_clad_k, default.core.peak_clad_k);
        assert_ne!(changed.containment.p_dw_pa, default.containment.p_dw_pa);
        assert_ne!(changed.vessel.p_pa, default.vessel.p_pa); // the ICs are off by 2000 s; their effect is in the vessel
    }

    /// Steps a plant inline and a second one through the worker wire, frame
    /// for frame bit-identical, and returns the last frame. The core's
    /// state crosses each step; the containment stays in its worker
    /// ([`ContainmentWorker`], gh:#1099).
    fn one_worker_against_three(prm: Params, steps: usize) -> Frame {
        let mut a = Plant::with_params(Mode::Historical, Mechanisms::default(), prm);
        let (mut ca, mut ka) = a.parts();
        let mut b = Plant::with_params(Mode::Historical, Mechanisms::default(), prm);
        let (mut cb, _) = b.parts();
        let mut kb = ContainmentWorker::default();
        for _ in 0..steps {
            a.step_inline(&mut ca, &mut ka);
            let s = b.begin();
            let mut w = Writer::default();
            s.core.write(&mut w);
            let core_snap = CoreSnapshot::read(&mut Reader::new(&w.0));
            // The core worker gets the state and the snapshot, steps a copy
            // of its own, and sends back the new state and the result; the
            // containment worker gets the request and answers with the
            // result, holding its containment.
            let core_res;
            (cb, core_res) = remote_core(&cb, &core_snap);
            let (cont_res, ok) = ContainmentWorker::read_reply(&kb.handle(&ContainmentWorker::request(0, b.mechanisms.containment_choice(), &s.containment)));
            assert!(ok, "{:?}", kb.held().and_then(PlantContainment::error));
            b.finish(core_res, cont_res);
            let (fa, fb) = (a.frame(), b.frame());
            let (mut wa, mut wb) = (Writer::default(), Writer::default());
            fa.write(&mut wa);
            fb.write(&mut wb);
            assert!(wa.0.iter().zip(&wb.0).all(|(x, y)| x.to_bits() == y.to_bits()), "diverged at t = {}", fa.t_s);
        }
        a.frame()
    }

    fn remote_core(c: &Core, s: &CoreSnapshot) -> (Core, CoreResult) {
        let mut w = Writer::default();
        c.write_state(&mut w);
        let mut there = Core::read_state(&mut Reader::new(&w.0));
        let res = there.step(s);
        let mut w = Writer::default();
        there.write_state(&mut w);
        res.write(&mut w);
        let mut r = Reader::new(&w.0);
        (Core::read_state(&mut r), CoreResult::read(&mut r))
    }

    fn remote_containment(c: &Containment, s: &ContainmentSnapshot) -> (Containment, ContainmentResult) {
        let mut w = Writer::default();
        c.write_state(&mut w);
        let mut there = Containment::read_state(&mut Reader::new(&w.0));
        let res = there.step(s);
        let mut w = Writer::default();
        there.write_state(&mut w);
        res.write(&mut w);
        let mut r = Reader::new(&w.0);
        (Containment::read_state(&mut r), ContainmentResult::read(&mut r))
    }

    /// The headless trace is deterministic: two runs, byte-identical.
    #[test]
    fn the_headless_run_is_deterministic() {
        assert_eq!(headless_csv(600, 60), headless_csv(600, 60));
    }

    /// The headless run of the whole recorded sequence (30 h, a row every
    /// 10 min) against the committed fixture, byte for byte. A regression
    /// harness, not a validation: it says the model has not changed. After a
    /// deliberate change, re-bless with
    /// `FUKUSHIMA_BLESS=1 cargo test --release -p sembawang --lib bwr_mark1`
    /// (~~`-p dhoby-ghaut --example fukushima_web`~~, before the model moved
    /// here on 2026-10-29, gh:#1033).
    ///
    /// **Since gh:#1099 (2026-11-02) the fixture is the network plant's**,
    /// re-blessed after the switch (set after the run, gh:#1099); the lumped
    /// plant's fixture, byte-identical to the one committed before, is
    /// `headless_30h_lumped.csv`
    /// (`the_lumped_headless_run_matches_its_fixture`). Gated behind
    /// `long-tests`: the network's 30 h take about 20 min.
    #[test]
    #[ignore = "no regression pin until the network model is checked (maintainer 2026-11-02: unnecessary to pin a regression when not even sure of the model yet; gh:#1110)"]
    fn the_headless_run_matches_the_committed_fixture() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/verification_and_validation/bwr_mark1/headless_30h.csv");
        let start = std::time::Instant::now();
        let now = headless_csv(108_000, 600);
        println!("30 h on the network: {:.1} s", start.elapsed().as_secs_f64());
        if std::env::var("FUKUSHIMA_BLESS").is_ok() {
            std::fs::write(path, &now).expect("write the fixture");
        }
        let fixture = std::fs::read_to_string(path).expect("the committed fixture");
        assert!(now == fixture, "the headless trace changed; re-bless only if the change is deliberate");
    }

    /// The lumped containment's 30 h trace (`without_network_containment`)
    /// against the fixture committed before gh:#1099, renamed
    /// `headless_30h_lumped.csv` and otherwise unchanged: the ablation
    /// reproduces the old default byte for byte. Re-bless with
    /// `FUKUSHIMA_BLESS=1` as above.
    #[test]
    fn the_lumped_headless_run_matches_its_fixture() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/verification_and_validation/bwr_mark1/headless_30h_lumped.csv");
        let start = std::time::Instant::now();
        let now = headless_csv_with(Mechanisms::default().without_network_containment(), 108_000, 600);
        println!("30 h on the lumped containment: {:.1} s", start.elapsed().as_secs_f64());
        if std::env::var("FUKUSHIMA_BLESS").is_ok() {
            std::fs::write(path, &now).expect("write the fixture");
        }
        let fixture = std::fs::read_to_string(path).expect("the committed fixture");
        assert!(now == fixture, "the lumped headless trace changed; re-bless only if the change is deliberate");
    }

    /// Physical bounds over the whole recorded sequence, every step: positive
    /// pressures below the vessel's design range, cladding between the
    /// coolant and 2600 K, a level inside the vessel, hydrogen never
    /// decreasing. A harness check that catches divergence, not physics V&V.
    /// On the lumped containment since gh:#1099, explicitly; the network
    /// plant's bounds are checked by `the_network_plant_over_30_hours`.
    #[test]
    fn the_whole_sequence_stays_in_physical_bounds() {
        let mut p = Plant::with_mechanisms(Mode::Historical, Mechanisms::default().without_network_containment());
        let (mut c, mut k) = p.parts();
        let mut h2 = 0.0;
        for _ in 0..108_000 {
            p.step_inline(&mut c, &mut k);
            let f = p.frame();
            // ~~below 9.5 MPa~~: since 2026-10-29 a debris quench can push
            // the vessel past the safety valves' capacity to about 10 MPa
            // (there is no vessel overpressure failure); the bound is the
            // saturation table's top, 20 MPa.
            assert!(f.vessel.p_pa > 5.0e4 && f.vessel.p_pa < steam::P_MAX_PA, "p {} at {}", f.vessel.p_pa, f.t_s);
            // ~~2600 K~~: since gh:#1035 rods stand to MELCOR's collapse
            // temperatures (ROD_COLLAPSE_MAX_K 3100 K), so the bound is the
            // enthalpy tables' 3200 K top.
            assert!(f.core.peak_clad_k < 3200.0, "clad {} at {}", f.core.peak_clad_k, f.t_s);
            assert!(f.vessel.level_m >= -BAF_M - 1e-9 && f.vessel.level_m <= RPV_HEIGHT_M, "level {}", f.vessel.level_m);
            assert!(f.containment.p_dw_pa > 5.0e4 && f.containment.p_ww_pa > 5.0e4);
            assert!(f.containment.t_pool_k > 273.0 && f.containment.t_pool_k < 450.0);
            assert!(f.core.h2_total_kg >= h2);
            h2 = f.core.h2_total_kg;
        }
    }

    /// The vessel starts where it was put: about 7 MPa, the normal level.
    #[test]
    fn the_vessel_starts_at_its_initial_state() {
        let p = Plant::new(Mode::Historical);
        assert!((p.vessel.p_pa - INITIAL_PRESSURE_PA).abs() < 1.0e4, "{}", p.vessel.p_pa);
        assert!((p.vessel.level_m - (ACTIVE_LENGTH_M + NORMAL_LEVEL_ABOVE_TAF_M)).abs() < 0.01, "{}", p.vessel.level_m);
    }

    /// Mass and energy conservation of the vessel over the first hour of
    /// the recorded sequence (IC, relief valves, bypass all acting).
    /// Methodology: every step, add up what `finish` was told left or
    /// entered (relief and bypass steam at h_g, IC heat, core heat, injection,
    /// oxidation steam) and compare with the vessel's own mass and energy.
    /// Result (2026-10-21): equal to round-off; the bound is 1e-9 relative.
    /// (The RPV wall's exchange is counted since it landed, 2026-10-29.)
    #[test]
    fn the_vessel_conserves_mass_and_energy() {
        let mut p = Plant::new(Mode::Historical);
        let (mut c, mut k) = p.parts();
        let (m0, u0) = (p.m, p.u);
        let (mut dm, mut du) = (0.0, 0.0);
        for _ in 0..3600 {
            let h_g = p.steam_out_h();
            let s = p.begin();
            let d = p.pending;
            let cr = c.step(&s.core);
            let kr = k.step(&s.containment);
            let steam_out = d.w_relief + d.w_leak - d.h2_out + d.w_bypass + cr.steam_used_kg_s;
            let w_inj = if p.last_core.lower_head_failed { 0.0 } else { d.w_inj };
            dm += (w_inj - steam_out) * DT_S;
            let gas = d.w_relief + d.w_leak;
            let share = if gas > 0.0 { d.w_relief / gas } else { 0.0 };
            let t_sat = p.vessel.sat.t_k;
            let h2_out = if p.mechanisms.hydrogen_cp_t {
                d.h2_out * (share * gas::h2_enthalpy_above_298(t_sat) + (1.0 - share) * gas::h2_enthalpy_above_298(p.gas_out_k()))
            } else {
                d.h2_out * self::core::CP_H2_J_PER_KG_K * (share * (t_sat - 298.15) + (1.0 - share) * (p.gas_out_k() - 298.15))
            };
            du += (cr.q_fluid_w - d.q_ic_w + w_inj * 4186.0 * (INJECTION_TEMPERATURE_K - 273.15) - steam_out * h_g - h2_out) * DT_S;
            p.finish(cr, kr);
            du -= p.q_internals_w * DT_S;
            du += p.q_rpv_wall_w * DT_S;
        }
        assert!(((p.m - m0) - dm).abs() <= 1e-9 * m0, "mass {} vs {}", p.m - m0, dm);
        assert!(((p.u - u0) - du).abs() <= 1e-9 * u0.abs(), "energy {} vs {}", p.u - u0, du);
    }

    /// A dry vessel with a large break is not emptied in one step (found
    /// 2026-10-29: m reached 0 and the pressure went infinite). With the
    /// steam line ruptured (0.20 m^2) a dry vessel of 80 kg at 5.7 MJ/kg
    /// (about 0.31 MPa, the state that failed) could pass about 81 kg in
    /// 1 s, more than it holds; the step passes at most
    /// the isothermal equalising amount, `n (p - p_dw) / p`, so the vessel
    /// keeps its mass and stays at or above the drywell.
    #[test]
    fn a_dry_vessel_is_not_emptied_by_a_large_break() {
        let mut p = Plant::new(Mode::Historical);
        let (mut c, mut k) = p.parts();
        for _ in 0..10 {
            p.step_inline(&mut c, &mut k);
        }
        p.m = 80.0;
        p.u = 80.0 * 5.7e6;
        p.h2 = 0.0;
        p.vessel = p.solve_vessel();
        p.msl.failed = true;
        let (p0, p_dw) = (p.vessel.p_pa, p.last_containment.p_dw_pa);
        assert!(p.vessel.dry && p0 > p_dw, "{:?} p_dw {p_dw}", p.vessel);
        let s = p.begin();
        let out = (p.pending.w_relief + p.pending.w_leak) * DT_S;
        assert!(out <= 80.0 * (p0 - p_dw) / p0 * (1.0 + 1e-12), "{out}");
        let cr = c.step(&s.core);
        let kr = k.step(&s.containment);
        p.finish(cr, kr);
        assert!(p.m > 0.0 && p.vessel.p_pa.is_finite(), "{} {}", p.m, p.vessel.p_pa);
    }

    /// A dry vessel blowing down through a break loses its energy with its
    /// mass (found 2026-10-29: steam leaving at h_g left the superheat
    /// behind, and the pressure ran to infinity as the mass drained). From
    /// the state that failed (28.5 kg holding 4.58e8 J, the steam line
    /// ruptured), 60 s of blowdown keep the pressure finite and never above
    /// where it started, and the steam leaves at `u/m + p V/m`.
    #[test]
    fn a_dry_vessel_blows_down_with_its_energy() {
        let mut p = Plant::new(Mode::Historical);
        let (mut c, mut k) = p.parts();
        for _ in 0..10 {
            p.step_inline(&mut c, &mut k);
        }
        p.m = 28.5;
        p.u = 4.58e8;
        p.h2 = 0.43;
        p.vessel = p.solve_vessel();
        p.msl.failed = true;
        assert!(p.vessel.dry, "{:?}", p.vessel);
        let v_total = volume_below(&p.prm, p.prm.rpv_height_m);
        assert_eq!(p.steam_out_h(), p.u / p.m + p.vessel.sat.p_pa * v_total / p.m);
        let p0 = p.vessel.p_pa;
        for _ in 0..60 {
            p.step_inline(&mut c, &mut k);
            assert!(p.vessel.p_pa.is_finite() && p.vessel.p_pa <= p0 * (1.0 + 1e-9), "{} vs {p0}", p.vessel.p_pa);
        }
    }

    /// Noble gas is conserved from the fuel to wherever it goes (gh:#1040):
    /// at every step of the 26 h sequence, the Xe in the vessel's gas, the
    /// drywell, the wetwell gas, vented and leaked to the building adds up
    /// to what the fuel has released, to 1e-12 of the inventory (the
    /// containment's share lags the vessel's by its one-step hand-over;
    /// once the lower head has failed the vessel's noble gas passes to the
    /// drywell, the vessel being open to it).
    ///
    /// **On the lumped containment since gh:#1099**, explicitly, so that the
    /// positions pinned below keep their meaning; the network plant's
    /// conservation is `the_network_plant_over_30_hours`'s.
    #[test]
    fn noble_gas_is_conserved_from_fuel_to_stack() {
        let mut p = Plant::with_mechanisms(Mode::Historical, Mechanisms::default().without_network_containment());
        let (mut c, mut k) = p.parts();
        let mut peak_ww = 0.0f64;
        for _ in 0..(26 * 3600) {
            p.step_inline(&mut c, &mut k);
            let f = p.frame();
            let k = &f.containment;
            let total = f.xe_vessel + k.xe_dw + k.xe_ww + k.xe_vented + k.xe_building;
            assert!((total - f.core.fp_volatile_released).abs() < 1e-12, "{} vs {} at {}", total, f.core.fp_volatile_released, f.t_s);
            peak_ww = peak_ww.max(k.xe_ww);
        }
        // And it does move: most of it ends up in the wetwell gas.
        // Where it is at 26 h: ~~all 0.8505 in the wetwell gas, carried
        // there by the downcomers; nothing vented and nothing in the
        // building, as neither the vent nor the flange leak opens in this
        // run~~ (true until the internals heat loss, 2026-10-29, flipped the
        // run into the early regime, where the vent flows at 23.73 h): all
        // 0.7989 released is vented to the stack, through the wetwell gas,
        // where it peaked before the vent opened. Since the mixture vent
        // (gh:#1053), which no longer passes about 20 times its orifice's
        // capacity, 1.4e-8 is still in the wetwell gas at 26 h (was under
        // 1e-9). Since the wetwell atmosphere (2026-10-29) the vent passes
        // only the gas the atmosphere holds (the pool no longer boils on
        // demand to feed it): at 26 h 9.9e-6 is still in the drywell and
        // 1.3e-5 in the wetwell gas (were under 1e-9 and 1.4e-8); with the
        // torus shell, which opens the vent 0.13 h later, 2.65e-5 and
        // 2.39e-5; with the core-top radiation (less heat reaching the
        // containment), 5.39e-5 and 3.55e-5. With a core plate per ring
        // (2026-10-29) the drywell peaks at 0.755 MPa, past the flange
        // leak's 0.75 MPa, and 9.15e-4 reaches the reactor building (7.6e-5
        // in the drywell, 4.1e-5 in the wetwell gas). With the pool in two
        // layers the flange opens about 17.5 h: 4.00e-3 in the building,
        // 7.85e-4 in the drywell (past the 5e-4 this test held, which was a
        // record of the earlier runs, not a bound), 9.2e-6 in the wetwell.
        // With the plate's debris oxidising: 4.60e-3, 7.77e-4 and 8.8e-6;
        // with the plenum's oxidation steam consumed: 4.64e-3, 8.54e-4, 1.1e-5;
        // with the control blades: 1.62e-2, 2.11e-4, 2.02e-4; with the
        // blades' heat of fusion: 1.31e-2, 1.82e-4, 1.69e-4; with the cavity
        // debris's phases (MCCI gas holding the drywell at the flange's
        // pressure from 16.7 h): 0.1120, 1.81e-3, 1.34e-3; with the vent and
        // flange choked: 0.0218, 6.35e-4, 4.01e-4; with the blades' diffusion
        // limit: 0.0217, 6.59e-4, 4.19e-4; with the two-sided equalising:
        // 0.0283, 7.84e-4, 5.01e-4; with the blades' B4C: 0.0263, 6.47e-4,
        // 4.61e-4; with the debris falling through the pedestal's water:
        // 0.0263, 6.75e-4, 4.84e-4; with UO2 carried by candling Zr: 0.0231, 4.34e-4,
        // 2.99e-4; with the fresh water from 15:00 h (F13): 0.0231, 4.57e-4,
        // 3.09e-4; with SPARC's atan and the burn's cbrt through PETIR
        // (round-off, the run diverging from 23.8 h): 0.02318, 4.94e-4,
        // 3.38e-4.
        let f = p.frame();
        let k = &f.containment;
        assert!(f.xe_vessel < 1e-9 && (k.xe_dw - 4.94e-4).abs() < 1e-5 && (k.xe_ww - 3.38e-4).abs() < 1e-5, "{f:?}");
        assert!((k.xe_building - 0.02318).abs() < 1e-5, "{f:?}");
        // The building at 26 h by MELCOR's BUR test (2026-10-30): Le
        // Chatelier's Y = 0.0714 (H2 0.0685, CO 0.0049), below the 0.10 of a
        // volume without igniters, so no burn; it passes the 0.07 of a
        // volume with igniters between 24 and 26 h (the explosion came at
        // 24.8 h; recorded, not adopted). With the vent and flange choked:
        // Y 0.0347, CO 0.0030 (half the gas reaches the building); 0.0345
        // with the blades' diffusion limit; 0.0416, CO 0.0036, with the
        // two-sided equalising; 0.0329, CO 0.0028, with the blades' B4C;
        // 0.0394, CO 0.0035, with UO2 carried by candling Zr.
        assert!((k.rb_burn_y - 0.0394).abs() < 1e-4 && (k.rb_co_fraction - 0.0035).abs() < 1e-4 && !k.rb_exploded, "{f:?}");
        // (0.7989 until the fission products' decay heat left the core with
        // them, which moves the release: 0.7967 since; 0.7977 since
        // tellurium is transported; 0.8109 since the debris oxidises;
        // 0.9911 since candling; 0.9978 since MELCOR's quench; 0.9955
        // since the head's wet contact; 0.9960 since the RPV wall; 0.9949
        // since MCCI, which opens the flange leak and sends 0.0012 to the
        // building; 0.9957 since the drywell's outer heat path, the flange
        // closed again, with 2.5e-4 still in the drywell and 1.5e-4 in the
        // wetwell gas at 26 h; 0.9958 since the core plate, with 3.8e-4 and
        // 2.6e-4; 0.9960 since the bottom of the core radiates, with 2.7e-4
        // and 2.6e-4; 0.9937 with a plate per ring, the flange open; 0.9898
        // with the pool in two layers; 0.9891 with the plate's debris
        // oxidising; 0.9889 with the plenum's oxidation steam consumed;
        // 0.9813 with the control blades; 0.9843 with their heat of
        // fusion; 0.8826 with the cavity debris's phases; 0.9749 with the
        // vent and flange choked; 0.9682 with the two-sided equalising; 0.9702 with the blades' B4C; 0.9735 with UO2 carried by candling Zr; 0.9733 with SPARC's atan through PETIR.)
        assert!((k.xe_vented - 0.9733).abs() < 1e-4 && peak_ww > 0.7, "{k:?} {peak_ww}");
    }

    /// Cesium is conserved from the fuel to wherever it goes (gh:#1047): at
    /// every step of 26 h, the vessel's, the containment's airborne, on the
    /// drywell's structures, in the wetwell pool, vented and leaked to the
    /// building add up to the released fraction times the inventory, to
    /// 1e-9 of it (with the dome internals' deposits since in-vessel
    /// deposition). Iodine and, since 2026-10-29, tellurium likewise.
    /// On the lumped containment since gh:#1099 (the network's: below).
    #[test]
    fn cesium_is_conserved_from_fuel_to_stack() {
        let mut p = Plant::with_mechanisms(Mode::Historical, Mechanisms::default().without_network_containment());
        let (mut c, mut k) = p.parts();
        for _ in 0..(26 * 3600) {
            p.step_inline(&mut c, &mut k);
            let f = p.frame();
            let k = &f.containment;
            let total = f.cs_vessel_kg + f.on_internals_kg[0] + k.cs_airborne_kg + k.cs_dw_structures_kg + k.cs_ww_pool_kg + k.cs_vented_kg + k.cs_building_kg;
            let released = f.core.fp_volatile_released * CS_INVENTORY_KG;
            assert!((total - released).abs() < 1e-9 * CS_INVENTORY_KG, "{total} vs {released} at {}", f.t_s);
            let total_i = f.i_vessel_kg + f.on_internals_kg[1] + k.i_airborne_kg + k.i_dw_structures_kg + k.i_ww_pool_kg + k.i_vented_kg + k.i_building_kg;
            let released_i = f.core.fp_volatile_released * I_INVENTORY_KG;
            assert!((total_i - released_i).abs() < 1e-9 * I_INVENTORY_KG, "I {total_i} vs {released_i} at {}", f.t_s);
            // Tellurium likewise (since `te_transport`): the vessel's
            // aerosol volume and everything that has reached the
            // containment.
            let total_te = p.vessel_rn.total_kg(rn_volume::TE) + k.te_kg;
            let released_te = f.core.fp_volatile_released * TE_INVENTORY_KG;
            assert!((total_te - released_te).abs() < 1e-9 * TE_INVENTORY_KG, "Te {total_te} vs {released_te} at {}", f.t_s);
        }
        let f = p.frame();
        let k = &f.containment;
        println!(
            "CS 26h internals {:.2} vessel {:.2} airborne {:.2} dw structures {:.2} ww pool {:.2} vented {:.2} building {:.2} of released {:.2} kg",
            f.on_internals_kg[0], f.cs_vessel_kg, k.cs_airborne_kg, k.cs_dw_structures_kg, k.cs_ww_pool_kg, k.cs_vented_kg, k.cs_building_kg, f.core.fp_volatile_released * CS_INVENTORY_KG
        );
        println!(
            "I 26h vessel {:.3} airborne {:.3} dw structures {:.3} ww pool {:.3} of released {:.3} kg",
            f.i_vessel_kg, k.i_airborne_kg, k.i_dw_structures_kg, k.i_ww_pool_kg, f.core.fp_volatile_released * I_INVENTORY_KG
        );
    }

    /// A dry vessel is not emptied by the dome internals' heat (gh:#1051):
    /// in the no-radiation ablation, where the vessel boils dry before its
    /// head fails, its mass stays above 100 kg through 26 h (2026-10-29:
    /// 188.8 kg at its lowest; 180.1 kg with the heat loss off; it reached
    /// zero, and then 227 MPa, before the heat loss was limited to a wet
    /// vessel).
    #[test]
    fn a_dry_vessel_is_not_emptied_by_the_internals() {
        // On the lumped containment since gh:#1099 (a vessel guard; its 26 h
        // on the network would cost about 18 min).
        let mut p = Plant::with_mechanisms(Mode::Historical, Mechanisms::default().without_core_radiation().without_network_containment());
        let (mut c, mut k) = p.parts();
        let mut min_m = f64::INFINITY;
        for _ in 0..(26 * 3600) {
            p.step_inline(&mut c, &mut k);
            min_m = min_m.min(p.m);
            assert!(p.vessel.p_pa.is_finite() && p.vessel.p_pa < 2.0e7, "{} at {}", p.vessel.p_pa, p.t_s);
        }
        assert!(min_m > 100.0, "{min_m}");
    }

    /// **The network plant over the whole sequence** (gh:#1099, 2026-11-02):
    /// at every step of 30 h with the network the default, the physical
    /// bounds of `the_whole_sequence_stays_in_physical_bounds`, noble gas,
    /// cesium, iodine and tellurium conserved from the fuel to wherever they
    /// go (the tolerances of the lumped tests, unchanged), and the network
    /// never failing. Prints where the fission products are at 26 h.
    ///
    /// Predicted before the run: every bound and every balance holds (the
    /// network carries the same `FpHold` with the same arithmetic, and its
    /// rides conserve by construction, `containment_network.md` case D);
    /// at 26 h most of the noble gas vented (the vent opens near 24 h) and
    /// little in the building (open loop the flange opened at 17.25 h and
    /// the building reached 1.74 % H2).
    ///
    /// **Results (2026-11-02; set after the run, gh:#1099):** every bound
    /// and balance held at every step, and the network never failed (held).
    /// At 26 h (lumped in brackets, from `noble_gas_is_conserved_from_fuel_to_stack`):
    /// noble gas vented 0.98265 \[0.9733\], in the building 0.014214
    /// \[0.02318\], drywell 4.2e-6, wetwell gas 2.0e-6 \[4.94e-4, 3.38e-4\];
    /// the building's Le Chatelier Y 0.04159 \[0.0394\], no burn (held);
    /// cesium on the drywell's structures 73.99 kg and in the pool 78.22 kg,
    /// 0.067 kg vented and 0.052 kg to the building; iodine 11.31 kg in the
    /// pool, 0.46 kg vented; the cavity 84.1 t of concrete ablated, 552 kg
    /// of H2 and 803 kg of CO. The head failed at high pressure (4.69 h,
    /// before the steam line's creep rupture, which never comes): `fdi_hpme`
    /// is flagged (still treated as low-pressure ejection, as in the lumped
    /// model). The flange opens between 15.33 and 15.5 h \[16.67-16.83 h\];
    /// the building reaches 3.9 % H2 at 26 h \[3.7 %\]. Prediction on the
    /// building (little H2) MISSED: the drywell, higher than predicted
    /// (1.00 MPa), leaks more. 1540 s with three other network runs beside
    /// it.
    #[test]
    #[cfg_attr(not(feature = "long-tests"), ignore = "long test (the 30 h network run, ~20 min); runs by default, skipped under --no-default-features")]
    fn the_network_plant_over_30_hours() {
        // The bounds and balances, at every step (see the doc above). The
        // values pinned at 26 h are in `the_network_plant_over_30_hours_pins`.
        let _ = network_30_hours_checked();
    }

    /// The values set after the run at 26 h (gh:#1099, d16ec33f0), split out
    /// on 2026-11-04 from [`the_network_plant_over_30_hours`] so its bounds and
    /// balances keep gating while the regression values wait for the network
    /// model check, like the two other network pins.
    #[test]
    #[ignore = "no regression pin until the network model is checked (maintainer 2026-11-02: unnecessary to pin a regression when not even sure of the model yet; gh:#1110). Set after the run at d16ec33f0; gh:#1114 and #1116-#1120 have moved the network since"]
    fn the_network_plant_over_30_hours_pins() {
        let f = network_30_hours_checked();
        let kr = &f.containment;
        assert!((kr.xe_vented - 0.98265).abs() < 1e-4 && (kr.xe_building - 0.014214).abs() < 1e-5 && kr.xe_dw < 1e-5 && kr.xe_ww < 1e-5, "{kr:?}");
        assert!((kr.rb_burn_y - 0.04159).abs() < 1e-4 && !kr.rb_exploded && kr.fdi_hpme, "{kr:?}");
        assert!((kr.cs_ww_pool_kg - 78.22).abs() < 0.01 && (kr.i_ww_pool_kg - 11.31).abs() < 0.01, "{kr:?}");
    }

    /// The 30 h historical network run with every bound and every
    /// fission-product balance asserted at every step; returns the 26 h frame.
    /// Reached by `the_network_plant_over_30_hours` and its `_pins` twin.
    fn network_30_hours_checked() -> Frame {
        let mut p = Plant::new(Mode::Historical);
        let (mut c, mut k) = p.parts();
        assert!(k.is_network());
        let mut h2 = 0.0;
        let mut at_26 = None;
        for i in 0..108_000usize {
            p.step_inline(&mut c, &mut k);
            assert!(k.error().is_none(), "{:?}", k.error());
            let f = p.frame();
            assert!(f.vessel.p_pa > 5.0e4 && f.vessel.p_pa < steam::P_MAX_PA, "p {} at {}", f.vessel.p_pa, f.t_s);
            assert!(f.core.peak_clad_k < 3200.0, "clad {} at {}", f.core.peak_clad_k, f.t_s);
            assert!(f.vessel.level_m >= -BAF_M - 1e-9 && f.vessel.level_m <= RPV_HEIGHT_M, "level {}", f.vessel.level_m);
            let kr = &f.containment;
            assert!(kr.p_dw_pa > 5.0e4 && kr.p_ww_pa > 5.0e4, "{kr:?}");
            assert!(kr.t_pool_k > 273.0 && kr.t_pool_k < 450.0, "{kr:?}");
            assert!(f.core.h2_total_kg >= h2);
            h2 = f.core.h2_total_kg;
            let xe = f.xe_vessel + kr.xe_dw + kr.xe_ww + kr.xe_vented + kr.xe_building;
            assert!((xe - f.core.fp_volatile_released).abs() < 1e-12, "Xe {xe} vs {} at {}", f.core.fp_volatile_released, f.t_s);
            let cs = f.cs_vessel_kg + f.on_internals_kg[0] + kr.cs_airborne_kg + kr.cs_dw_structures_kg + kr.cs_ww_pool_kg + kr.cs_vented_kg + kr.cs_building_kg;
            assert!((cs - f.core.fp_volatile_released * CS_INVENTORY_KG).abs() < 1e-9 * CS_INVENTORY_KG, "Cs {cs} at {}", f.t_s);
            let io = f.i_vessel_kg + f.on_internals_kg[1] + kr.i_airborne_kg + kr.i_dw_structures_kg + kr.i_ww_pool_kg + kr.i_vented_kg + kr.i_building_kg;
            assert!((io - f.core.fp_volatile_released * I_INVENTORY_KG).abs() < 1e-9 * I_INVENTORY_KG, "I {io} at {}", f.t_s);
            let te = p.vessel_rn.total_kg(rn_volume::TE) + kr.te_kg;
            assert!((te - f.core.fp_volatile_released * TE_INVENTORY_KG).abs() < 1e-9 * TE_INVENTORY_KG, "Te {te} at {}", f.t_s);
            if i + 1 == 26 * 3600 {
                println!("network 26 h: {f:?}");
                at_26 = Some(f);
            }
        }
        println!("network 30 h: {:?}", p.frame());
        at_26.expect("26 h reached")
    }

    /// The swollen level (gh:#1036): no vapour, no swell; a vapour flux J
    /// gives a mean void J/(2 v0), so the pool's volume grows by 1/(1 -
    /// alpha); and the void stops at 0.4 however hard the pool boils.
    #[test]
    fn the_swollen_level_follows_bubble_rise_and_its_cap() {
        let prm = Params::default();
        let (h_fg, v_g) = (1.5e6, 0.027);
        let level = 2.0;
        assert_eq!(swollen_level(&prm, level, 0.0, h_fg, v_g), level);
        let z = level + prm.baf_m;
        let area = (volume_below(&prm, z + 0.01) - volume_below(&prm, z - 0.01)) / 0.02;
        let q = 0.06 * area / v_g * h_fg; // J = 0.06 m/s, alpha = 0.1
        let swollen = swollen_level(&prm, level, q, h_fg, v_g) + prm.baf_m;
        assert!((volume_below(&prm, swollen) - volume_below(&prm, z) / 0.9).abs() < 1e-6 * volume_below(&prm, z));
        let capped = swollen_level(&prm, level, 100.0 * q, h_fg, v_g) + prm.baf_m;
        assert!((volume_below(&prm, capped) - volume_below(&prm, z) / 0.6).abs() < 1e-6 * volume_below(&prm, z));
    }

    /// The mixture's choked-flux factor: 1 for steam, `sqrt(M_H2/M_H2O)`
    /// (about 0.335) for hydrogen, falling between.
    #[test]
    fn hydrogen_lowers_the_choked_mass_flux() {
        assert_eq!(mixture_flux_factor(0.0), 1.0);
        assert!((mixture_flux_factor(1.0) - (M_H2 / M_H2O).sqrt()).abs() < 1e-12);
        assert!(mixture_flux_factor(0.5) < 1.0 && mixture_flux_factor(0.5) > mixture_flux_factor(0.9));
    }

    /// The level-volume map inverts.
    #[test]
    fn the_level_map_inverts() {
        let prm = Params::default();
        for z in [0.5, 4.9, 6.0, 8.6, 12.0, 19.0] {
            assert!((height_of_volume(&prm, volume_below(&prm, z)) - z).abs() < 1e-9, "{z}");
        }
    }
}

#[cfg(test)]
mod unit_tests {
    use super::*;

    /// gh:#1109: a Unit 2 or 3 plant builds its own unit's containment
    /// network (`NetworkConfig::unit2()`/`unit3()`) from its own inputs, the
    /// unit crosses the containment worker's wire, and Unit 1 (every
    /// existing constructor) stays Unit 1 with the IC and no pumps.
    #[test]
    fn each_unit_builds_its_own_network_and_the_unit_crosses_the_wire() {
        for unit in [Unit::Two, Unit::Three] {
            let p = Plant::for_unit(unit, Mode::Historical, Mechanisms::default());
            assert_eq!((p.unit(), *p.params()), (unit, unit.params()));
            let (_, c) = p.parts();
            match c.model() {
                ContainmentModel::Network(n) => assert_eq!(n.config(), unit.network_config()),
                other => panic!("{unit:?}: {other:?}"),
            }
            // The worker builds the same unit from the request alone.
            let mut wk = ContainmentWorker::default();
            let mut q = Plant::for_unit(unit, Mode::Historical, Mechanisms::default());
            let s = q.begin();
            let reply = wk.handle(&ContainmentWorker::request(1, q.containment_choice(), &s.containment));
            assert!(ContainmentWorker::read_reply(&reply).1);
            match wk.held().unwrap().model() {
                ContainmentModel::Network(n) => assert_eq!(n.config(), unit.network_config()),
                other => panic!("{unit:?}: {other:?}"),
            }
        }
        let p = Plant::new(Mode::Historical);
        assert_eq!((p.unit(), p.containment_choice().unit), (Unit::One, Unit::One));
        assert_eq!(Mechanisms::default().containment_choice().unit, Unit::One);
        assert!(p.pumps().iter().all(|x| x.drive == turbine_pump::Drive::Off));
    }

    /// One worker equals three for Unit 2 on the lumped containment, past
    /// RCIC's start (the pump's flows and the merged exhaust cross the wire
    /// in the snapshot and the frame).
    #[test]
    fn three_workers_are_one_worker_for_unit2() {
        let mech = Mechanisms::default().without_network_containment();
        let mut a = Plant::for_unit(Unit::Two, Mode::Historical, mech);
        let (mut ca, mut ka) = a.parts();
        let mut b = Plant::for_unit(Unit::Two, Mode::Historical, mech);
        let (mut cb, _) = b.parts();
        let mut wk = ContainmentWorker::default();
        for _ in 0..900 {
            a.step_inline(&mut ca, &mut ka);
            let s = b.begin();
            let mut w = Writer::default();
            s.core.write(&mut w);
            let core_snap = CoreSnapshot::read(&mut Reader::new(&w.0));
            let c = cb.step(&core_snap);
            let reply = wk.handle(&ContainmentWorker::request(7, b.containment_choice(), &s.containment));
            let (k, ok) = ContainmentWorker::read_reply(&reply);
            assert!(ok);
            b.finish(c, k);
        }
        let (fa, fb) = (a.frame(), b.frame());
        assert!(fa.w_rcic_kg_s > 0.0 && fa.w_turbine_kg_s > 0.0);
        let (mut wa, mut wb) = (Writer::default(), Writer::default());
        fa.write(&mut wa);
        fb.write(&mut wb);
        assert_eq!(wa.0.iter().map(|x| x.to_bits()).collect::<Vec<_>>(), wb.0.iter().map(|x| x.to_bits()).collect::<Vec<_>>());
        // And the frame survives its own wire.
        let back = Frame::read(&mut Reader::new(&wa.0));
        assert_eq!((back.w_rcic_kg_s, back.w_turbine_kg_s, back.w_spray_kg_s), (fa.w_rcic_kg_s, fa.w_turbine_kg_s, fa.w_spray_kg_s));
    }
}
