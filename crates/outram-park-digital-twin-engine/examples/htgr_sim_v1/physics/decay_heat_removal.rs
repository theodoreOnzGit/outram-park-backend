//! Passive decay-heat removal path: core -> reflector -> RPV -> RCCS.
//!
//! ~~**This is a PLACEHOLDER, and the word is load-bearing.**~~ **CORRECTED
//! 2026-09-29 (gh:#389, #395, #396)** -- it is no longer a placeholder in any
//! of the senses that sentence meant: every conductance is derived from the
//! HTR-10 geometry and material data (since 2026-09-17), every heat capacity
//! is derived from the annulus geometry, published densities and `c_p(T)`
//! (since 2026-09-29, one labelled exception: the RPV wall thickness), and the
//! reflector and vessel are solved in **one implicit system with the bed**
//! (since 2026-09-29) rather than bolted onto its source. What it still is not
//! is listed under "What is NOT modelled" below.
//!
//! It exists so that `htgr_sim_v1` has *somewhere for decay heat to go* under
//! a loss of forced cooling. Before it, a workspace-wide grep for
//! `cavity cool`, `reactor cavity`, `RCCS` and `vessel cooling` returned zero
//! hits in every crate's `src/`, which `docs/reactor-scoping/htr10.md` records
//! as "the largest single gap on this slate": with the circulator tripped and
//! the secondary isolated the core could not cool, and the reactor never went
//! recritical, where the real HTR-10 returns to power at about 3000 s.
//!
//! ## What is modelled
//!
//! ~~Three lumped control volumes in **series** ... `UA_core_refl
//! UA_refl_rpv UA_rpv_rccs`~~ **CORRECTED 2026-09-29** -- two solid nodes
//! (reflector, RPV) behind the bed, joined by **five derived legs**, and the
//! RCCS as a fixed boundary:
//!
//! ```text
//!             leg 1 (Achenbach: ZBS + dispersion + wall film) leg 2a
//!  BED solid + helium ───────────── near-wall node ────────── REFLECTOR node
//!   (T_s, T_f)       see the next section                      (T_r)
//!                                                                  │ leg 2b (outer half of the graphite annulus)
//!                                                                  │ leg 3  (boronated annulus, conduction)
//!                                                                  │ leg 4  (reflector -> RPV gap, RADIATION)
//!                                                               RPV node (T_v)
//!                                                                  │ leg 5  (RPV -> cavity, RADIATION)
//!                                                                RCCS 50 degC (fixed)
//! ```
//!
//! **One implicit solve (2026-09-29, gh:#395).** The bed's solid and helium
//! rows and these two nodes are the four unknowns of one backward-Euler
//! system ([`super::pebble_bed::PebbleBedPorousMediaNode::step`]); the
//! bed-to-reflector conductance sits on the bed solid row's diagonal. ~~The
//! passive loss was computed against a held bed temperature and subtracted
//! from the bed's source (`net_core_source`)~~ -- the pattern the engine
//! `CLAUDE.md` names as a defect ("put transfer terms inside the control
//! volume's own balance, implicitly, not as an adjustment to its source"). The
//! temperature-dependent legs (`k_eff(T)`, IG-110 `k(T)`, the two radiative
//! legs) and the secant heat capacities are re-evaluated at the latest iterate
//! inside the step's fixed point, which converges them to 1e-9 K; the
//! radiative legs use the exact secant conductance `sigma A eps (T1^2 +
//! T2^2)(T1 + T2)`, so at convergence they carry exactly `sigma A eps (T1^4 -
//! T2^4)`.
//!
//! ## The bed -> reflector leg: which mechanisms, and where each comes from
//!
//! The maintainer's direction (2026-09-29): the bed must transmit heat to the
//! reflector "via convection and radiation especially", each mechanism on the
//! implicit diagonal, with nothing counted twice. The structure is taken
//! whole from **Achenbach, E., "Heat and Flow Characteristics of Packed
//! Beds", *Exp. Therm. Fluid Sci.* 10 (1995) 17-27** (the KFA Julich review
//! behind much of KTA 3102; proprietary, cited not redistributed), whose
//! pseudo-homogeneous bed has an effective conductivity and a wall boundary
//! condition:
//!
//! ```text
//! lambda_e = lambda_0 + lambda_k                          Achenbach eq. (28), p. 22
//! lambda_k / lambda_g = Pe / K_r                           eq. (29), p. 23 (Yagi et al.)
//! K_r = 8 [2 - (1 - 2 d/D)^2]                              eq. (30), p. 23 (Schlunder)
//! -lambda_e dT/dr |_w = alpha_w dT                         eq. (32), p. 25
//! Nu_w = alpha_w d / lambda_g = (1 - d/D) Re^0.61 Pr^(1/3) eq. (34), p. 25, 50 < Re < 2e4
//! ```
//!
//! and, below `Re ~ 100`, "the temperature difference `dT` vanishes, which is
//! equivalent to `alpha_w -> infinity`" (p. 25). In this lumped model that
//! becomes a small network, every branch on the implicit diagonal:
//!
//! ```text
//!   BED SOLID T_s --[ 8 pi lambda_0(T_s) H ]--+                        (1) stagnant: conduction + sphere radiation
//!                                             +-- NEAR-WALL node --[ alpha_w A_w  (x)  2 G_annulus ]-- REFLECTOR T_r
//!   BED HELIUM T_f --[ 8 pi lambda_k H ]------+                        (2) convection: flow dispersion + (3) wall film
//! ```
//!
//! 1. **Stagnant conduction and pebble radiation -- `lambda_0`, from the bed
//!    solid.** The VTB ZBS table ([`zbs_effective_conductivity`]) is exactly
//!    Achenbach's `lambda_0`, "the stagnant gas effective conductivity",
//!    which he lists as including "heat radiation solid-solid and through the
//!    void area to the next layer" (p. 23). So the pebble radiation is in this
//!    branch once, and **no separate `sigma eps A (T_s^4 - T_w^4)` surface
//!    leg is added** -- it would count the same photons twice. IAEA-TECDOC-1694
//!    records the PBMR-400 benchmark codes coupling the bed to the reflector
//!    the same way ("the effective pebble bed thermal conductivity correlation
//!    since conduction and radiation heat transfer mechanisms are taken into
//!    account in the correlation"). The VTB table is total-only, so the split
//!    between its two parts is not available, and per gh:#362 the analytic
//!    `tampines::pebble_bed::ZbsBed` is not substituted. `lambda_0` is
//!    evaluated at the live bed temperature every pass, which is how the
//!    `T^3` radiative growth enters the implicit solve.
//! 2. **Convection through the bed -- `lambda_k`, from the bed helium.** The
//!    flow-dispersion conductivity is carried by the gas, so it is driven by
//!    the helium node. `Pe = Re Pr` on the superficial velocity; it is
//!    proportional to the flow, so it **vanishes at LOFC by construction**.
//!    At the rated point it exceeds `lambda_0` several times over.
//! 3. **Convection at the wall -- `alpha_w`, eq. (34), in series.** The wall
//!    film sits between the near-wall bed and the reflector's inner surface,
//!    over `A_w = pi D H_bed`. Below `Re = 100` the paper's own statement
//!    applies (`alpha_w -> infinity`, no film resistance); this makes the leg
//!    step at `Re = 100` (a loop flow of about 0.19 kg/s, reached only during
//!    a circulator run-down), because the source gives no blend and none is
//!    invented.
//!
//! **Assumptions, stated.** (a) `Re = u d / nu` on the superficial velocity
//! of the whole loop flow (the same basis the bed's pebble film uses; the
//! paper's nomenclature says only "velocity"); (b) the two branches split the
//! pseudo-homogeneous bed's single temperature into the lumped node's solid
//! and helium temperatures by mechanism; (c) `8 pi lambda H` is the
//! mean-to-surface conductance of a uniformly heated cylinder, applied to
//! both branches; (d) the helium node is outlet-referenced (the well-mixed
//! bed), so the dispersion branch sees the outlet temperature over the whole
//! wall, which **over-states** it near the cold top of the bed.
//!
//! ## The riser leg: reflector -> cold helium in the side-reflector channels (gh:#397)
//!
//! Cold helium from the circulator rises through **20 channels** in the side
//! reflector ([S2] section 5, [S5] section 2, [S3] section 2; Jun et al. 2009
//! Table 1) on its way to the top plenum, and cools the reflector as it goes.
//! The leg is a heat-transfer path out of the reflector row of the bed's
//! implicit solve and into the primary loop's cold-return CV.
//!
//! **Geometry, all published or derived from published figures:**
//! diameter **0.08 m** and channel-centre radius 1.446 m (Jun, Lim & Lee
//! 2009, *Nucl. Eng. Technol.* 41(3), Table 1 "Diameter of cold helium flow
//! channel, m 0.08"); total channel volume **0.507681 m^3** ([S3] KENO VI,
//! Table VI); hence length `L = V / (20 pi d^2/4)` = **5.05 m** (derived,
//! consistent with the 4.70 m graphite annulus plus the channels' run into
//! the top and bottom reflectors). Flow: **3.846 / 4.32 = 89.0 %** of the loop
//! ([`crate::physics::primary_loop::RISER_FLOW_FRACTION`], Gao & Shi 2002
//! Table 1).
//!
//! **Heat transfer.** `h` from the Gnielinski correlation in its laminar /
//! transition / turbulent interpolated form (Gnielinski 2013, *Int. J. Heat
//! Mass Transfer* 63, 134-140), as implemented in `tuas`
//! (`gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing_bulk_fluid_prandtl`)
//! with the entrance correction at `L/d = 63` and the Churchill smooth-pipe
//! friction factor; Dittus-Boelter is the cross-check
//! (`tests::the_riser_leg_uses_gnielinski_and_vanishes_without_flow`). The
//! channel wall sits at the reflector temperature and the helium enters at
//! the cold-return CV's temperature, which is the classic isothermal-wall
//! channel, so the leg is the exact effectiveness form
//!
//! ```text
//! Q_riser = m_r c_p (1 - exp(-h A / (m_r c_p))) (T_r - T_cold),   A = 20 pi d L
//! ```
//!
//! which is bounded by the stream's capacity and **vanishes with the flow**
//! (a trip leaves it negligible; stagnant natural convection in the channels
//! is not modelled).
//!
//! **Assumptions, stated.** (a) The channel walls are at the lumped
//! reflector node's temperature: the graphite conduction between the node and
//! the channels at r = 1.446 m is neglected, which **over-states** the leg;
//! (b) `tuas`'s Gnielinski is the liquids form, used with `Pr_wall = Pr_bulk`
//! so its liquid viscosity-ratio correction is unity, and the gas
//! temperature-ratio correction is omitted; its laminar branch is the
//! uniform-heat-flux `Nu = 4.36`; (c) the riser helium properties are
//! evaluated at the cold-return temperature (the channel inlet).
//!
//! ## The boundary condition, and where the number comes from
//!
//! The RCCS is held at a **fixed 50 degC** throughout, which is what GAMMA+
//! did for the same transient:
//!
//! > "A temperature of 50 degC at the RCCS water cooling tube \[12\] was used
//! > as a fixed boundary condition through the transients."
//! > -- Jun, J.S., Lim, H.S., Lee, W.J. (2009), *The Benchmark Calculations of
//! > the GAMMA+ Code with the HTR-10 Safety Demonstration Experiments*,
//! > Nuclear Engineering and Technology **41**(3), 307-318, section 2.4.
//!
//! **This citation is second-hand and must not be presented otherwise.** Jun
//! et al. attribute the 50 degC to their own reference \[12\], which this
//! workspace does not hold.
//!
//! A fixed-temperature sink is a real modelling choice, not a shortcut: the
//! RCCS is a water-cooled panel with its own circulation, so its surface
//! temperature is very nearly independent of the heat it is receiving over
//! this transient's range. GAMMA+ found it sufficient.
//!
//! ## What is NOT modelled, and which way each error points
//!
//! - ~~**The `UA` values are not derived.** ... **No number this module
//!   produces is a prediction until they are.**~~ **CORRECTED** -- derived
//!   from geometry since 2026-09-17 (see the constants below).
//! - ~~**Radiation is carried on the LAST link only.**~~ **CORRECTED** -- the
//!   gap (leg 4) and the vessel-to-cavity leg (5) are `T^4` legs, and leg 1
//!   carries the ZBS sphere radiation.
//! - ~~**Bed-to-wall convection** -- pending literature~~ -- present since
//!   2026-09-29 (Achenbach 1995, above).
//! - ~~**The helium-to-reflector riser leg** -- not yet; it biases the
//!   reflector hot under forced flow~~ -- present since 2026-09-29 (gh:#397);
//!   see "The riser leg" below.
//! - **No natural circulation.** After the blower baffle closes the real core
//!   establishes a buoyancy-driven helium loop that Chen et al. (2009) section
//!   5 call an effective heat-transport mechanism alongside conduction and
//!   radiation. This model has no gravity term. Omitting it removes a
//!   transport path, so core temperatures here are an **upper bound**.
//! - **One lumped node per region.** The reflector is 1.0 m of graphite with a
//!   real internal gradient; one node under-states its thermal lag.
//! - **Radial only.** The top and bottom reflectors, and the axial conduction
//!   through them, are not in the chain, nor in the reflector node's capacity.
//! - **No core barrel, cavity or carbon brick as separate bodies.** The core
//!   vessel (3.82 m ID, Jun et al. 2009 Table 1) sits in the gap between the
//!   boronated brick and the RPV and is not modelled; the gap is radiation
//!   only.

use tuas_boussinesq_solver::heat_transfer_correlations::heat_transfer_interactions::conductance::simple_radiation_conductance;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::solid_database::nuclear_graphite::nuclear_graphite_ig_110_thermal_conductivity_unirradiated;
use uom::si::area::square_meter;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::density::try_get_rho;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::specific_enthalpy::try_get_h;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::specific_heat_capacity::try_get_cp;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::{Material, SolidMaterial};
use uom::si::available_energy::joule_per_kilogram;
use uom::si::energy::joule;
use uom::si::f64::{Area, Energy, Mass, MassRate, Power, Pressure, ThermodynamicTemperature};
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::power::watt;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};

/// RCCS water-cooling-tube temperature held as a fixed boundary \[degC\].
///
/// **50 degC**, per Jun et al. (2009) section 2.4 -- see the module docs for
/// the full quotation and the caveat that the citation is second-hand.
pub const RCCS_BOUNDARY_TEMPERATURE_C: f64 = 50.0;

// ~~`DESIGN_BED_TEMPERATURE_K = 950.0` ("the conductances are sized at") and
// `DESIGN_PASSIVE_HEAT_LOSS_W = 206 000` ("the one measured quantity the three
// conductances are anchored to")~~ -- moved into the test module 2026-09-29:
// nothing is sized or anchored at either any more. 206 kW is only the V&V
// reference, and 950 K only the temperature that comparison is taken at.

// ---------------------------------------------------------------------------
// THE CHAIN, DERIVED FROM GEOMETRY -- NOT FITTED TO THE 206 kW ANCHOR
// ---------------------------------------------------------------------------
//
// **Superseded 2026-09-17.** ~~`UA_CORE_REFLECTOR_W_PER_K = 2500.0` and
// `UA_REFLECTOR_RPV_W_PER_K = 505.06`, apportioned by assumption so their
// SERIES value reproduced the 206 kW anchor.~~ **CORRECTED** -- that pair was
// a two-leg chain with the split invented and with **no radiation term on
// either leg**, which is backwards: radiation carries ~42 % of the resistance
// and the legs it acts on are the hottest ones.
//
// Every conductance below is now computed from the HTR-10 R-Z zone map
// (`reactor_model::htr10_rz_geometry`) and the published vessel dimensions,
// with the radiative legs evaluated at the live temperatures each step. The
// 206 kW figure is used **only as an independent check**, never as an input:
// see ~~`tests::the_derived_chain_agrees_with_the_published_surface_cooling_duty`,
// which measures **+7.5 %**~~ `tests::the_chain_opens_in_equilibrium_at_its_design_heat`
// (**CORRECTED 2026-09-29**: the test was renamed and the recorded values are
// +13.9 % on 2026-09-17 and +23.3 % since 2026-09-28), which is allowed to
// fail if the physics says so.
// Calibrating a free parameter until that check passed was considered and
// rejected by the maintainer -- it would have matched exactly and proved
// nothing. See the crate `CLAUDE.md`, "Physical correctness is the first
// priority in this simulator".

/// Radial boundaries of the chain \[m\], from the R-Z zone map's own radial
/// ticks and the published vessel bore.
///
/// - `0.90` -- pebble-bed outer radius (zone-map tick at 90 cm).
/// - `1.67793` -- outer radius of the **Graphite** zones (tick at 167.793 cm).
/// - `1.90` -- outer radius of the **Boronated** graphite (tick at 190 cm).
/// - `2.10` -- RPV bore, half of the published 4.2 m inner diameter.
const R_BED_OUTER_M: f64 = 0.90;
const R_GRAPHITE_OUTER_M: f64 = 1.67793;
const R_BORONATED_OUTER_M: f64 = 1.90;
const R_RPV_INNER_M: f64 = 2.10;

/// Axial extents \[m\] each leg acts over, measured from the zone map rather
/// than assumed.
///
/// - `H_BED_M` -- the core mean height from `htr10::design` (197 cm), the
///   span over which heat is actually generated.
/// - `H_GRAPHITE_M` -- the Graphite zones span z = 40..510 cm, so 4.70 m.
/// - `H_BORONATED_M` -- the Boronated zones span the full z = 0..610 cm.
///
/// **Assumption:** each annulus is treated as conducting radially over its own
/// full axial extent, with axial conduction neglected. That is why the graphite
/// leg gets 4.70 m rather than the 1.97 m of core it surrounds -- heat
/// generated over the core height spreads axially through the taller reflector
/// before turning radial. Neglecting the axial resistance makes this an
/// **upper** bound on the graphite leg's conductance.
const H_BED_M: f64 = 1.97;
const H_GRAPHITE_M: f64 = 4.70;
const H_BORONATED_M: f64 = 6.10;

/// ~~Thermal conductivity of the reflector graphite \[W/(m K)\]. **Assumed, not
/// sourced to an HTR-10 measurement.** ... 30 is a mid-range value.~~
///
/// **CHANGED 2026-09-28 (maintainer: graphite "wired correctly into
/// htgr_sim_v1").** The **graphite reflector** annulus now takes IG-110 from
/// `tuas_boussinesq_solver`
/// ([`nuclear_graphite_ig_110_thermal_conductivity_unirradiated`], the VTB
/// HTTR deck quadratic) at the live reflector temperature — see
/// [`ua_graphite_annulus_w_per_k`]. This constant now serves **only the
/// boronated annulus**, which is boronated carbon brick, **not** IG-110: no
/// conductivity for it exists in this workspace, so the 30 W/(m K) stays as an
/// **assumption needing a source** (it carries under 4 % of the chain's
/// resistance, so it is cheap, but it is still not data).
const BORONATED_CONDUCTIVITY_W_PER_M_K: f64 = 30.0;

/// Grey-surface emissivity used on both radiative legs \[-\].
///
/// **0.8**, the only emissivity transcribed anywhere in this workspace --
/// `tampines::pebble_bed::ZbsBed::htr10()`, itself the NEA PBMR-400 benchmark
/// assumption. Applied here to graphite *and* to oxidised vessel steel, which
/// is a convenience rather than a measurement: real values for both sit in the
/// 0.7-0.9 band, so the error is modest, but it is an assumption.
const SURFACE_EMISSIVITY: f64 = 0.8;

/// Radiating area-coefficient of the vessel into the reactor cavity
/// \[m^2\] -- the `A * F * epsilon` product TUAS's
/// [`simple_radiation_conductance`] expects, not a bare area.
///
/// # The RPV-to-RCCS link is RADIATION, so its conductance is not a constant
///
/// Across the cavity gap the vessel sees the cooler panels essentially
/// directly, and radiation dominates. Its conductance is
///
/// ```text
/// UA_rad = sigma * A * F * epsilon * (T_h^2 + T_c^2)(T_h + T_c)
/// ```
///
/// which **rises steeply with temperature** -- roughly as `T^3`. Treating it as
/// a fixed number, as this module first did, under-states heat removal while
/// the vessel is hot and over-states it once cool, which is exactly backwards
/// for a cooldown: the error is largest when the removal matters most.
///
/// This is computed rather than fitted:
///
/// - **Area** `pi * D * H = pi * 4.2 m * 11.1 m = 146.4 m^2`, the RPV lateral
///   surface, from the published inner diameter and height
///   (IAEA-TECDOC-1382, carried in `htr10::design`).
/// - **View factor** `F ~ 1` -- the vessel is enclosed by the cavity, so
///   essentially all of what leaves it arrives somewhere on the boundary.
/// - **Effective emissivity** `epsilon_eff = 1/(1/eps_vessel + 1/eps_cavity - 1)
///   = 1/(1/0.8 + 1/0.9 - 1) = 0.735` for oxidised steel against a painted
///   cavity liner. **These two emissivities are assumed, not sourced** --
///   TUAS's solid database carries no emissivity data at all, and the only
///   graphite emissivity anywhere in this workspace is a hardcoded 0.8 in
///   `ZbsBed::htr10()`.
///
/// Giving `146.4 * 1.0 * 0.735 = 107.6 m^2`.
const RPV_RADIATING_AREA_COEFF_M2: f64 = 107.6;

// ---------------------------------------------------------------------------
// HEAT CAPACITIES, DERIVED (gh:#396, 2026-09-29)
//
// ~~`REFLECTOR_CAPACITY_J_PER_K = 1.8e8` and `RPV_CAPACITY_J_PER_K = 6.0e7`,
// "order of magnitude, not derived from the published masses", constant in
// T~~ -- replaced by `m c_p(T)`, integrated as ENTHALPY `m (h(T') - h(T))`
// (secant capacity), from the same annulus geometry the conductances use.
// These capacities set every cooldown time constant, including #320's LOFC.
// ---------------------------------------------------------------------------

/// Graphite reflector material: IG-110, the HTTR / HTR-10 reflector grade in
/// `tuas` -- density 1770 kg/m^3 (NEA/NSC/DOC(2006)1 table 1.27), `c_p(T)`
/// from Butland & Maddison (1973/74), valid 300-2000 K. The same material the
/// graphite annulus conductance already uses.
const REFLECTOR_GRAPHITE: SolidMaterial = SolidMaterial::NuclearGraphiteIG110;

/// Void volume inside the graphite annulus \[m^3\], **published** ([S3] KENO
/// VI model, Table VI, `docs/reactor-scoping/htr10-plant-data.md` section
/// 4.3): the 20 coolant channels 5.07681e5 cm^3, the 13 control-rod /
/// irradiation channels 7.76484e5 cm^3 and the 7 KLAK channels 2.29410e5
/// cm^3. All three sit in the side reflector, so they are subtracted from
/// the annulus. **Assumption:** each lies wholly inside the 4.70 m graphite
/// annulus the chain uses.
const GRAPHITE_ANNULUS_CHANNEL_VOID_M3: f64 = 0.507681 + 0.776484 + 0.229410;

/// Boronated carbon brick density \[kg/m^3\]: **1590**, published -- [S3]
/// Table II ("B4C content 5 wt%, brick density 1.59 g/cm^3"), recorded in
/// `docs/reactor-scoping/htr10-plant-data.md` section 4.3.
const BORONATED_BRICK_DENSITY_KG_PER_M3: f64 = 1590.0;

/// RPV wall thickness \[m\] -- **INVENTED**. The scoping sheet records "wall
/// thicknesses of the RPV, SG vessel and hot gas duct vessel" as not stated
/// in any source, and no held document (JAERI-Conf 96-010, IAEA-TECDOC-1382,
/// Jun et al. 2009 Table 1, Chen et al. 2009, Hu et al. 2006) gives it.
/// 0.10 m is a plausibility choice only: at the published 3.5 MPa design
/// pressure and 2.1 m radius it gives a membrane hoop stress `p r / t =
/// 73.5 MPa`, inside what a pressure-vessel steel carries. A sourced
/// thickness replaces it; it scales the RPV capacity linearly.
const RPV_WALL_THICKNESS_M: f64 = 0.10;

/// RPV steel stand-in: `SteelSS304LHighTemp` (Kim, ANL-75-55, 300-1700 K),
/// the only steel in `tuas`'s solid database valid over the vessel's range.
/// **A substitution, stated:** the HTR-10 RPV is C-Mn-Si steel ([S4] section
/// 2.7); no carbon-steel property set exists in the workspace. Carbon and
/// austenitic steels differ in `c_p` by roughly 10-20 % over 300-600 K, so
/// this biases the vessel capacity by about that much. The steam generator
/// makes the same kind of substitution for its 2.25Cr1Mo tubes.
const RPV_STEEL: SolidMaterial = SolidMaterial::SteelSS304LHighTemp;

/// Pressure argument for the `tuas` solid property calls \[Pa\] -- solids
/// here are pressure independent; the value only satisfies the signature.
fn solid_property_pressure() -> Pressure {
    Pressure::new::<pascal>(101_325.0)
}

/// Density of a `tuas` solid \[kg/m^3\] at 300 K (the solids here are
/// treated as fixed-mass bodies; density only sets the mass).
fn solid_density(material: SolidMaterial) -> f64 {
    try_get_rho(
        Material::Solid(material),
        ThermodynamicTemperature::new::<kelvin>(300.0),
        solid_property_pressure(),
    )
    .unwrap_or_else(|e| panic!("{material:?} density: {e:?}"))
    .get::<kilogram_per_cubic_meter>()
}

/// Specific enthalpy of a `tuas` solid \[J/kg\] (only differences are used).
///
/// # Panics
///
/// Outside the material's coded window (IG-110 300-2000 K, Kim steel
/// 300-1700 K): fail loud, no stale fallback.
fn solid_enthalpy(material: SolidMaterial, t: ThermodynamicTemperature) -> f64 {
    try_get_h(Material::Solid(material), t, solid_property_pressure())
        .unwrap_or_else(|e| panic!("{material:?} enthalpy at {} K: {e:?}", t.get::<kelvin>()))
        .get::<joule_per_kilogram>()
}

/// Graphite mass of the side-reflector annulus \[kg\]: IG-110 density times
/// `pi (1.678^2 - 0.90^2) 4.70 m^3` less the published channel voids.
/// 28.1 m^3, about 49.7 t.
pub fn reflector_graphite_mass() -> Mass {
    let volume =
        std::f64::consts::PI * (R_GRAPHITE_OUTER_M.powi(2) - R_BED_OUTER_M.powi(2)) * H_GRAPHITE_M
            - GRAPHITE_ANNULUS_CHANNEL_VOID_M3;
    Mass::new::<kilogram>(solid_density(REFLECTOR_GRAPHITE) * volume)
}

/// Boronated carbon brick mass \[kg\]: the published 1590 kg/m^3 times
/// `pi (1.90^2 - 1.678^2) 6.10 m^3` = 15.2 m^3, about 24.2 t. Lumped into the
/// reflector node. **Assumption:** its `c_p(T)` is graphite's (Butland &
/// Maddison) -- it is carbon brick with 5 wt% B4C, and no property set for it
/// exists here.
pub fn boronated_brick_mass() -> Mass {
    let volume = std::f64::consts::PI
        * (R_BORONATED_OUTER_M.powi(2) - R_GRAPHITE_OUTER_M.powi(2))
        * H_BORONATED_M;
    Mass::new::<kilogram>(BORONATED_BRICK_DENSITY_KG_PER_M3 * volume)
}

/// RPV mass \[kg\]: the lateral shell only, `pi ((r + t)^2 - r^2) H` on the
/// published 4.2 m bore and 11.1 m height ([`htr10::design`]) and the
/// invented [`RPV_WALL_THICKNESS_M`], times the stand-in steel's density.
/// Heads are not included (they are not in the radial chain either).
pub fn rpv_mass() -> Mass {
    let design = super::pebble_bed::design();
    let r = 0.5 * design.rpv_inner_diameter.get::<uom::si::length::meter>();
    let h = design.rpv_height.get::<uom::si::length::meter>();
    let volume = std::f64::consts::PI * ((r + RPV_WALL_THICKNESS_M).powi(2) - r * r) * h;
    Mass::new::<kilogram>(solid_density(RPV_STEEL) * volume)
}

/// Secant heat capacity `m (h(T') - h(T)) / (T' - T)` \[J/K\] of a body of
/// `mass` made of `material`, from `t_old` to `t_new` -- what makes a
/// backward-Euler step on a `c_p(T)` body conserve its enthalpy exactly. The
/// tangent `m c_p(T)` when the two temperatures coincide.
fn secant_capacity(
    mass: Mass,
    material: SolidMaterial,
    t_old: ThermodynamicTemperature,
    t_new: ThermodynamicTemperature,
) -> f64 {
    let m = mass.get::<kilogram>();
    let dt = t_new.get::<kelvin>() - t_old.get::<kelvin>();
    if dt.abs() > 1.0e-9 {
        m * (solid_enthalpy(material, t_new) - solid_enthalpy(material, t_old)) / dt
    } else {
        m * try_get_cp(Material::Solid(material), t_old, solid_property_pressure())
            .unwrap_or_else(|e| panic!("{material:?} c_p: {e:?}"))
            .get::<joule_per_kilogram_kelvin>()
    }
}

/// Radiative conductance from the RPV to the fixed 50 degC RCCS \[W/K\].
fn ua_rpv_rccs_w_per_k(rpv: ThermodynamicTemperature) -> f64 {
    simple_radiation_conductance(
        Area::new::<square_meter>(RPV_RADIATING_AREA_COEFF_M2),
        rpv,
        CoreToRccsPath::rccs_boundary(),
    )
    .get::<watt_per_kelvin>()
}

/// Conductance of a cylindrical annulus in pure radial conduction \[W/K\],
/// `2 pi k H / ln(r_o / r_i)`.
fn annulus_conductance_w_per_k(k_w_per_m_k: f64, height_m: f64, r_i_m: f64, r_o_m: f64) -> f64 {
    2.0 * std::f64::consts::PI * k_w_per_m_k * height_m / (r_o_m / r_i_m).ln()
}

/// **Leg 1 -- pebble bed to its own outer surface \[W/K\].** Conduction AND
/// pebble-to-pebble radiation, because the ZBS effective conductivity already
/// carries both.
///
/// For a solid cylinder with uniform volumetric generation the volume-mean
/// temperature sits `Q / (8 pi k H)` above the surface, so the conductance
/// between the bed's *mean* temperature -- which is the only temperature the
/// one-node model has -- and its edge is `8 pi k_eff H`.
///
/// `k_eff` is [`zbs_effective_conductivity`], the VTB packed-bed table, and is
/// **strongly temperature dependent**: 11.9 W/(m K) at 300 K rising to 45.0 at
/// 2000 K, because the radiation contribution grows as `T^3`. Evaluating it at
/// the live bed temperature is the point -- a constant fitted at the design
/// point would understate removal by nearly a factor of two at LOFC peak
/// temperatures.
///
/// **Assumption:** the VTB table is for a *generic* pebble bed, not HTR-10
/// specifically (see `htr10::zbs`), and `tampines`' analytic `ZbsBed` is known
/// not to reproduce it. The table is used because it is the tested one.
///
/// **Two further limits, recorded 2026-09-28 (gh:#362, deferred by the
/// maintainer -- do not switch to the analytic `ZbsBed`):** (1) the VTB table
/// is built on a **constant** 26 W/(m K) solid conductivity (per the
/// 2026-09-28 MOOSE/VTB graphite survey,
/// `crates/tuas_boussinesq_solver/docs/moose-graphite-thermal-conductivity-survey.md`;
/// not re-checked in this change), so this leg does
/// **not** use `tuas`'s A3 graphite conductivity (the pebble interior does);
/// (2) the table ends at 2000 K and is **flat at 44.95 W/(m K) above it**, so
/// the bed-to-reflector leg stops growing with temperature (verified: the
/// lookup in `htr10::zbs::zbs_effective_conductivity` clamps to its last
/// entry) exactly where the
/// `T^3` radiation contribution should make it grow fastest -- above 2000 K
/// this leg **under-states** passive removal.
fn ua_bed_to_surface_w_per_k(bed: ThermodynamicTemperature) -> f64 {
    let k_eff = outram_park_digital_twin_engine::htr10::zbs::zbs_effective_conductivity(bed)
        .get::<uom::si::thermal_conductivity::watt_per_meter_kelvin>();
    8.0 * std::f64::consts::PI * k_eff * H_BED_M
}

/// **Leg 2 -- the graphite reflector annulus \[W/K\]**, `r = 0.90 -> 1.678 m`
/// over 4.70 m. Pure solid conduction; no radiation term belongs here.
///
/// **CHANGED 2026-09-28:** `k` is IG-110 (the HTTR / HTR-10 reflector grade
/// in `tuas`), **unirradiated**, evaluated at the lumped reflector node's
/// temperature, where it was an assumed constant 30 W/(m K). IG-110 gives
/// 42.5 W/(m K) at 600 K and 33.5 at 1000 K, so this leg is **more**
/// conductive than before at every temperature the reflector reaches.
///
/// Assumptions, stated: (1) unirradiated — HTR-10's safety tests ran early in
/// life, and no reflector fluence is tracked; irradiated graphite conducts
/// worse, so this is the conductive bound; (2) one temperature for the whole
/// annulus — the node's; (3) HTR-10's reflector is IG-110-class graphite per
/// `tuas`'s variant doc, not re-checked against the HTR-10 benchmark
/// specification here.
///
/// # Panics
///
/// Outside IG-110's coded 300-2000 K window — the reflector never approaches
/// 2000 K in any transient this simulator runs.
fn ua_graphite_annulus_w_per_k(reflector: ThermodynamicTemperature) -> f64 {
    let k = nuclear_graphite_ig_110_thermal_conductivity_unirradiated(reflector)
        .unwrap_or_else(|e| {
            panic!(
                "reflector graphite (IG-110) conductivity at {} K: {e:?}",
                reflector.get::<kelvin>()
            )
        })
        .get::<uom::si::thermal_conductivity::watt_per_meter_kelvin>();
    annulus_conductance_w_per_k(k, H_GRAPHITE_M, R_BED_OUTER_M, R_GRAPHITE_OUTER_M)
}

/// **Leg 3 -- the boronated graphite annulus \[W/K\]**, `r = 1.678 -> 1.90 m`
/// over 6.10 m. Thin and short-pathed, so it carries under 4 % of the chain's
/// resistance and its assumed conductivity barely matters.
fn ua_boronated_annulus_w_per_k() -> f64 {
    annulus_conductance_w_per_k(
        BORONATED_CONDUCTIVITY_W_PER_M_K,
        H_BORONATED_M,
        R_GRAPHITE_OUTER_M,
        R_BORONATED_OUTER_M,
    )
}

/// **Leg 4 -- the reflector-to-vessel gap \[W/K\], RADIATION.**
///
/// `r = 1.90 -> 2.10 m`. Two long concentric grey cylinders, for which the
/// effective emissivity is
///
/// ```text
/// 1 / eps_eff = 1/eps_1 + (r_1/r_2) (1/eps_2 - 1)
/// ```
///
/// giving `eps_eff = 0.677` at [`SURFACE_EMISSIVITY`] on both surfaces. The
/// area is the reflector's outer lateral surface, `2 pi r H`.
///
/// **Assumption:** the gap is treated as radiation only. Helium natural
/// convection across it is neglected, which makes this leg -- and therefore
/// the whole chain -- **conservative** (less heat removed, hotter core).
fn ua_gap_to_rpv_w_per_k(
    reflector: ThermodynamicTemperature,
    rpv: ThermodynamicTemperature,
) -> f64 {
    let eps_eff = 1.0
        / (1.0 / SURFACE_EMISSIVITY
            + (R_BORONATED_OUTER_M / R_RPV_INNER_M) * (1.0 / SURFACE_EMISSIVITY - 1.0));
    let area = 2.0 * std::f64::consts::PI * R_BORONATED_OUTER_M * H_BORONATED_M * eps_eff;
    simple_radiation_conductance(Area::new::<square_meter>(area), reflector, rpv)
        .get::<watt_per_kelvin>()
}

/// Achenbach's radial turbulent Peclet number `K_r = 8 [2 - (1 - 2 d/D)^2]`
/// (eq. (30), p. 23), for the HTR-10 pebble and bed diameters. 9.03.
fn radial_turbulent_peclet() -> f64 {
    let d = super::pebble_bed::pebble_diameter().get::<uom::si::length::meter>();
    let big_d = super::pebble_bed::core_diameter().get::<uom::si::length::meter>();
    8.0 * (2.0 - (1.0 - 2.0 * d / big_d).powi(2))
}

/// Reynolds number `u d / nu` of the bed on the superficial velocity, the
/// helium conductivity \[W/(m K)\] and Prandtl number, at the helium node's
/// temperature and the loop flow.
fn bed_flow_numbers(helium: ThermodynamicTemperature, mass_flow: MassRate) -> (f64, f64, f64) {
    let (k_g, prandtl, mu) = super::pebble_bed::helium_transport(helium);
    let mass_flux = mass_flow.get::<kilogram_per_second>().abs()
        / super::pebble_bed::superficial_area().get::<square_meter>();
    let d = super::pebble_bed::pebble_diameter().get::<uom::si::length::meter>();
    (mass_flux * d / mu, k_g, prandtl)
}

/// **Branch (1): bed solid -> near-wall node \[W/K\]**, `8 pi lambda_0 H`
/// with `lambda_0` the stagnant ZBS conductivity (conduction and sphere
/// radiation) at the bed temperature -- see [`ua_bed_to_surface_w_per_k`].
fn ua_solid_to_near_wall_w_per_k(bed: ThermodynamicTemperature) -> f64 {
    ua_bed_to_surface_w_per_k(bed)
}

/// **Branch (2): bed helium -> near-wall node \[W/K\]**, `8 pi lambda_k H`
/// with the flow-dispersion conductivity `lambda_k = lambda_g Pe / K_r`
/// (Achenbach eqs. (29)-(30), `Pe = Re Pr`). Zero at zero flow.
fn ua_helium_to_near_wall_w_per_k(helium: ThermodynamicTemperature, mass_flow: MassRate) -> f64 {
    let (re, k_g, prandtl) = bed_flow_numbers(helium, mass_flow);
    let lambda_k = k_g * re * prandtl / radial_turbulent_peclet();
    8.0 * std::f64::consts::PI * lambda_k * H_BED_M
}

/// **The wall film \[W/K\]**, `alpha_w A_w` with `alpha_w = Nu_w lambda_g /
/// d` from Achenbach eq. (34), `Nu_w = (1 - d/D) Re^0.61 Pr^(1/3)`, over the
/// bed's lateral wall `A_w = pi D H_bed`. `None` below `Re = 100`, where the
/// paper states `alpha_w -> infinity` (no film resistance). Above the
/// correlation's `Re = 2e4` top it is still applied, and says so here: the
/// HTR-10 bed reaches `Re ~ 4300` at the 8 kg/s circulator ceiling.
fn ua_wall_film_w_per_k(helium: ThermodynamicTemperature, mass_flow: MassRate) -> Option<f64> {
    let (re, k_g, prandtl) = bed_flow_numbers(helium, mass_flow);
    if re < 100.0 {
        return None;
    }
    let d = super::pebble_bed::pebble_diameter().get::<uom::si::length::meter>();
    let big_d = super::pebble_bed::core_diameter().get::<uom::si::length::meter>();
    let nu_w = (1.0 - d / big_d) * re.powf(0.61) * prandtl.powf(1.0 / 3.0);
    let alpha_w = nu_w * k_g / d;
    Some(alpha_w * std::f64::consts::PI * big_d * H_BED_M)
}

/// **Near-wall node -> reflector node \[W/K\]**: the wall film (when it has
/// a resistance) in series with the inner half of the graphite annulus.
///
/// **Assumption:** the reflector node carries a single volume-mean
/// temperature, so the annulus's conduction resistance is split evenly
/// either side of it -- half on the way in, half on the way out (hence
/// `2.0 *`). The two halves recombine to the full annulus.
fn ua_near_wall_to_reflector_w_per_k(
    helium: ThermodynamicTemperature,
    mass_flow: MassRate,
    reflector: ThermodynamicTemperature,
) -> f64 {
    let annulus_half = 2.0 * ua_graphite_annulus_w_per_k(reflector);
    match ua_wall_film_w_per_k(helium, mass_flow) {
        Some(film) => 1.0 / (1.0 / film + 1.0 / annulus_half),
        None => annulus_half,
    }
}

/// Conductance from the **lumped reflector node to the RPV** \[W/K\]: the
/// outer half of the graphite annulus, then the boronated annulus, then the
/// radiative gap.
fn ua_reflector_to_rpv_w_per_k(
    reflector: ThermodynamicTemperature,
    rpv: ThermodynamicTemperature,
) -> f64 {
    1.0 / (1.0 / (2.0 * ua_graphite_annulus_w_per_k(reflector))
        + 1.0 / ua_boronated_annulus_w_per_k()
        + 1.0 / ua_gap_to_rpv_w_per_k(reflector, rpv))
}

/// Riser channel count (published; see the module doc).
const RISER_CHANNEL_COUNT: f64 = 20.0;

/// Riser channel diameter \[m\]: **0.08**, Jun, Lim & Lee (2009) Table 1.
const RISER_CHANNEL_DIAMETER_M: f64 = 0.08;

/// Riser channel length \[m\], derived: the published total volume over the
/// published count and bore, `V / (20 pi d^2 / 4)` = 5.05 m.
fn riser_channel_length_m() -> f64 {
    crate::physics::primary_loop::RISER_BOREHOLE_VOLUME_M3
        / (RISER_CHANNEL_COUNT * std::f64::consts::PI * RISER_CHANNEL_DIAMETER_M.powi(2) / 4.0)
}

/// Riser helium flow \[kg/s\] from the loop flow.
fn riser_mass_flow_kg_s(loop_flow: MassRate) -> f64 {
    loop_flow.get::<kilogram_per_second>().abs() * crate::physics::primary_loop::RISER_FLOW_FRACTION
}

/// Riser Nusselt number and Reynolds number at the cold helium temperature
/// and loop flow: `tuas`'s interpolated Gnielinski (2013), `Pr_wall =
/// Pr_bulk`, Churchill smooth-pipe friction, `L/d` from the derived length.
fn riser_nusselt_and_reynolds(
    cold_helium: ThermodynamicTemperature,
    loop_flow: MassRate,
) -> (f64, f64, f64, f64) {
    let (k_g, prandtl, mu) = super::pebble_bed::helium_transport(cold_helium);
    let per_channel = riser_mass_flow_kg_s(loop_flow) / RISER_CHANNEL_COUNT;
    let re = 4.0 * per_channel / (std::f64::consts::PI * RISER_CHANNEL_DIAMETER_M * mu);
    let darcy = tuas_boussinesq_solver::fluid_mechanics_correlations::darcy(re.max(1.0), 0.0)
        .unwrap_or_else(|e| panic!("riser Churchill friction at Re {re}: {e:?}"));
    let nu = tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::pipe_correlations::gnielinski_correlation_interpolated_uniform_heat_flux_liquids_developing_bulk_fluid_prandtl(
        re.max(1.0),
        prandtl,
        prandtl,
        darcy,
        riser_channel_length_m() / RISER_CHANNEL_DIAMETER_M,
    );
    (nu, re, prandtl, k_g)
}

/// **The riser leg \[W/K\]**: `m_r c_p (1 - exp(-h A / (m_r c_p)))`, the
/// exact isothermal-wall channel conductance against the channel INLET
/// temperature (the cold-return CV). Zero at zero flow. See the module doc.
fn ua_reflector_to_risers_w_per_k(
    cold_helium: ThermodynamicTemperature,
    loop_flow: MassRate,
) -> f64 {
    let m_r = riser_mass_flow_kg_s(loop_flow);
    if !(m_r > 0.0) {
        return 0.0;
    }
    let (nu, _, _, k_g) = riser_nusselt_and_reynolds(cold_helium, loop_flow);
    let h = nu * k_g / RISER_CHANNEL_DIAMETER_M;
    let area = RISER_CHANNEL_COUNT
        * std::f64::consts::PI
        * RISER_CHANNEL_DIAMETER_M
        * riser_channel_length_m();
    let cp =
        super::pebble_bed::helium_specific_heat(cold_helium).get::<joule_per_kilogram_kelvin>();
    let capacity_rate = m_r * cp;
    capacity_rate * (1.0 - (-h * area / capacity_rate).exp())
}

/// The conductances and secant capacities of the passive path at one iterate
/// of the bed's implicit solve -- what [`super::pebble_bed::PebbleBedPorousMediaNode::step`]
/// puts in its reflector and RPV rows and on the bed solid row's diagonal.
///
/// Conductances in W/K, capacities in J/K, all evaluated at the iterate
/// temperatures handed to [`CoreToRccsPath::coupling`].
#[derive(Clone, Copy, Debug)]
pub struct PassiveCoupling {
    /// Branch (1): bed solid -> near-wall node, stagnant ZBS.
    pub solid_to_near_wall: f64,
    /// Branch (2): bed helium -> near-wall node, flow dispersion.
    pub helium_to_near_wall: f64,
    /// Near-wall node -> reflector node: wall film in series with the inner
    /// half of the graphite annulus.
    pub near_wall_to_reflector: f64,
    /// Reflector node -> RPV: the outer half of the graphite annulus, the
    /// boronated annulus and the radiative gap, in series.
    pub reflector_to_rpv: f64,
    /// RPV -> RCCS, radiative.
    pub rpv_to_rccs: f64,
    /// Reflector -> riser helium (gh:#397), against
    /// [`Self::riser_helium_temperature_k`].
    pub reflector_to_risers: f64,
    /// Temperature of the helium entering the risers -- the cold-return CV's
    /// state as handed to the bed \[K\].
    pub riser_helium_temperature_k: f64,
    /// Reflector node secant capacity (graphite + boronated brick) from the
    /// start-of-step temperature to the iterate.
    pub reflector_capacity: f64,
    /// RPV secant capacity from the start-of-step temperature to the iterate.
    pub rpv_capacity: f64,
    /// The RCCS boundary temperature \[K\].
    pub rccs_temperature_k: f64,
}

/// The passive heat path from the pebble bed out to the RCCS: the reflector
/// and RPV nodes' state. Their energy balances are solved inside the bed's
/// implicit step; this type owns the legs' physics and the two temperatures.
#[derive(Clone, Copy, Debug)]
pub struct CoreToRccsPath {
    /// Bulk graphite reflector temperature (graphite annulus + boronated
    /// brick, one node).
    reflector_temperature: ThermodynamicTemperature,
    /// Bulk reactor-pressure-vessel temperature.
    rpv_temperature: ThermodynamicTemperature,
    // NOTE: the conductances are deliberately NOT stored. Four of the five
    // legs depend on temperature, so caching them would freeze the physics
    // at whatever state the path was constructed in. The masses are
    // recomputed from geometry too; they are a few multiplications.
    /// Heat rate leaving the pebble bed on the most recent step.
    heat_from_core: Power,
    /// Heat rate reaching the RCCS on the most recent step.
    heat_to_rccs: Power,
    /// Heat rate given to the riser helium on the most recent step (gh:#397)
    /// -- what the cold-return CV receives.
    heat_to_risers: Power,
}

impl CoreToRccsPath {
    /// The fixed RCCS boundary temperature.
    pub fn rccs_boundary() -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<degree_celsius>(RCCS_BOUNDARY_TEMPERATURE_C)
    }

    // ~~`placeholder()` -- "Construct the path with **placeholder**
    // conductances and capacities ... seeded so that the chain passes roughly
    // 206 kW ... **Replace these before quoting any cooldown result**"~~ --
    // DELETED 2026-09-29 (gh:#389): false since 2026-09-17 (every conductance
    // derived) and, since 2026-09-29, for the capacities too. The plant now
    // builds the path with `new_at_steady_state` at its own bed seed.

    /// Construct the path **already in equilibrium** with a given bed --
    /// the derived constructor. Every conductance is the geometry- and
    /// literature-derived leg set, every capacity the derived `m c_p(T)`; the
    /// only thing chosen here is the node temperatures, and they are not a
    /// choice: they are the steady state for a bed whose solid sits at
    /// `bed_solid`, whose helium sits at `bed_helium`, with riser helium
    /// entering at `cold_helium`, at loop flow `mass_flow`.
    ///
    /// # Why this is not optional
    ///
    /// The conductances fix how much heat the chain carries at a given
    /// core-to-sink difference, but they say nothing about where the
    /// intermediate temperatures sit. Start them anywhere else and the chain
    /// opens with a large transient -- the model relaxing an initial condition
    /// nobody chose.
    pub fn new_at_steady_state(
        bed_solid: ThermodynamicTemperature,
        bed_helium: ThermodynamicTemperature,
        cold_helium: ThermodynamicTemperature,
        mass_flow: MassRate,
    ) -> Self {
        let t_sink = Self::rccs_boundary().get::<kelvin>();
        let mut path = Self {
            reflector_temperature: ThermodynamicTemperature::new::<kelvin>(
                0.5 * (bed_solid.get::<kelvin>() + t_sink),
            ),
            rpv_temperature: ThermodynamicTemperature::new::<kelvin>(t_sink + 50.0),
            heat_from_core: Power::new::<watt>(0.0),
            heat_to_rccs: Power::new::<watt>(0.0),
            heat_to_risers: Power::new::<watt>(0.0),
        };
        // Steady state: the same network with no storage (dt -> infinity).
        path.solve_with_bed_held(bed_solid, bed_helium, cold_helium, mass_flow, None);
        path
    }

    /// Solve the network behind a **held** bed (solid at `bed_solid`, helium
    /// at `bed_helium`): the near-wall node (algebraic), the reflector and the
    /// RPV. `dt_s = Some(dt)` is one backward-Euler step from the stored
    /// state; `None` is the steady state. The legs and secant capacities are
    /// re-evaluated at each iterate until the node temperatures move less than
    /// 1e-10 K. The heat rates stored are from the final pass's conductances
    /// at its solution, so the balance is exact.
    fn solve_with_bed_held(
        &mut self,
        bed_solid: ThermodynamicTemperature,
        bed_helium: ThermodynamicTemperature,
        cold_helium: ThermodynamicTemperature,
        mass_flow: MassRate,
        dt_s: Option<f64>,
    ) {
        let (t_s, t_f) = (bed_solid.get::<kelvin>(), bed_helium.get::<kelvin>());
        let (t_r0, t_v0) = (
            self.reflector_temperature.get::<kelvin>(),
            self.rpv_temperature.get::<kelvin>(),
        );
        let (mut t_w, mut t_r, mut t_v) = (0.5 * (t_s + t_r0), t_r0, t_v0);
        let mut c = self.coupling(
            bed_solid,
            bed_helium,
            cold_helium,
            mass_flow,
            self.reflector_temperature,
            self.rpv_temperature,
        );
        for _ in 0..500 {
            let (a_r, a_v) = match dt_s {
                Some(dt) => (c.reflector_capacity / dt, c.rpv_capacity / dt),
                None => (0.0, 0.0),
            };
            let (gs, gf, gw) = (
                c.solid_to_near_wall,
                c.helium_to_near_wall,
                c.near_wall_to_reflector,
            );
            let (g2, g3, g_riser) = (c.reflector_to_rpv, c.rpv_to_rccs, c.reflector_to_risers);
            let mut m = outram_foam_basic_lib::prelude::SquareMatrix::new(3);
            m.set(0, 0, gs + gf + gw);
            m.set(0, 1, -gw);
            m.set(1, 0, -gw);
            m.set(1, 1, a_r + gw + g2 + g_riser);
            m.set(1, 2, -g2);
            m.set(2, 1, -g2);
            m.set(2, 2, a_v + g2 + g3);
            let rhs = [
                gs * t_s + gf * t_f,
                a_r * t_r0 + g_riser * c.riser_helium_temperature_k,
                a_v * t_v0 + g3 * c.rccs_temperature_k,
            ];
            let x = m
                .solve(&rhs)
                .expect("the passive-path matrix is diagonally dominant with a Dirichlet sink");
            let moved = (x[0] - t_w)
                .abs()
                .max((x[1] - t_r).abs())
                .max((x[2] - t_v).abs());
            (t_w, t_r, t_v) = (x[0], x[1], x[2]);
            if moved < 1.0e-10 {
                break;
            }
            c = self.coupling(
                bed_solid,
                bed_helium,
                cold_helium,
                mass_flow,
                ThermodynamicTemperature::new::<kelvin>(t_r),
                ThermodynamicTemperature::new::<kelvin>(t_v),
            );
        }
        self.commit(
            ThermodynamicTemperature::new::<kelvin>(t_r),
            ThermodynamicTemperature::new::<kelvin>(t_v),
            Power::new::<watt>(c.near_wall_to_reflector * (t_w - t_r)),
            Power::new::<watt>(c.rpv_to_rccs * (t_v - c.rccs_temperature_k)),
            Power::new::<watt>(c.reflector_to_risers * (t_r - c.riser_helium_temperature_k)),
        );
    }

    // ~~`ua_from_balance_point` -- identify the series conductance from Chen
    // et al.'s t = 310 s balance point~~ -- DELETED 2026-09-29: it had no
    // caller, and the chain is derived rather than identified. The
    // identification idea is recorded on gh:#389 should a comparison against
    // Chen et al.'s balance point be wanted as V&V.

    /// The legs and secant capacities at an iterate of the bed's implicit
    /// solve: bed solid at `bed_solid`, bed helium at `bed_helium`, riser
    /// inlet helium (the cold-return CV) at `cold_helium`, loop flow
    /// `mass_flow`, reflector at `reflector`, RPV at `rpv`. The
    /// capacities run from this path's stored (start-of-step) temperatures to
    /// the iterate, so a converged solve conserves enthalpy exactly.
    ///
    /// # Panics
    ///
    /// Outside the property windows (IG-110 300-2000 K, the Kim steel
    /// 300-1700 K).
    #[allow(clippy::too_many_arguments)]
    pub fn coupling(
        &self,
        bed_solid: ThermodynamicTemperature,
        bed_helium: ThermodynamicTemperature,
        cold_helium: ThermodynamicTemperature,
        mass_flow: MassRate,
        reflector: ThermodynamicTemperature,
        rpv: ThermodynamicTemperature,
    ) -> PassiveCoupling {
        let reflector_mass = reflector_graphite_mass() + boronated_brick_mass();
        PassiveCoupling {
            solid_to_near_wall: ua_solid_to_near_wall_w_per_k(bed_solid),
            helium_to_near_wall: ua_helium_to_near_wall_w_per_k(bed_helium, mass_flow),
            near_wall_to_reflector: ua_near_wall_to_reflector_w_per_k(
                bed_helium, mass_flow, reflector,
            ),
            reflector_to_rpv: ua_reflector_to_rpv_w_per_k(reflector, rpv),
            rpv_to_rccs: ua_rpv_rccs_w_per_k(rpv),
            reflector_to_risers: ua_reflector_to_risers_w_per_k(cold_helium, mass_flow),
            riser_helium_temperature_k: cold_helium.get::<kelvin>(),
            // Graphite and brick share the graphite c_p(T) (see
            // `boronated_brick_mass`), so one secant on the summed mass.
            reflector_capacity: secant_capacity(
                reflector_mass,
                REFLECTOR_GRAPHITE,
                self.reflector_temperature,
                reflector,
            ),
            rpv_capacity: secant_capacity(rpv_mass(), RPV_STEEL, self.rpv_temperature, rpv),
            rccs_temperature_k: Self::rccs_boundary().get::<kelvin>(),
        }
    }

    /// Store the solved end-of-step state and the two boundary heat rates
    /// the solve computed from its own final conductances.
    pub fn commit(
        &mut self,
        reflector: ThermodynamicTemperature,
        rpv: ThermodynamicTemperature,
        heat_from_core: Power,
        heat_to_rccs: Power,
        heat_to_risers: Power,
    ) {
        self.reflector_temperature = reflector;
        self.rpv_temperature = rpv;
        self.heat_from_core = heat_from_core;
        self.heat_to_rccs = heat_to_rccs;
        self.heat_to_risers = heat_to_risers;
    }

    /// Enthalpy held in the two solid nodes \[J\], `m h(T)` on each (graphite
    /// + brick on the reflector, steel on the RPV) from `tuas`'s datum. Only
    /// its *change* is meaningful; the plant's global energy ledger reads it
    /// every step (gh:#394). ~~`C T` with the constant capacities~~ since
    /// 2026-09-29.
    pub fn stored_energy(&self) -> Energy {
        let reflector_mass = (reflector_graphite_mass() + boronated_brick_mass()).get::<kilogram>();
        Energy::new::<joule>(
            reflector_mass * solid_enthalpy(REFLECTOR_GRAPHITE, self.reflector_temperature)
                + rpv_mass().get::<kilogram>() * solid_enthalpy(RPV_STEEL, self.rpv_temperature),
        )
    }

    /// Advance the reflector and RPV by `dt` against a **held** bed -- the
    /// chain on its own, for the component tests. The plant never calls this:
    /// there the two nodes are rows of the bed's own implicit solve. Same
    /// legs, same secant capacities, same fixed point. `mass_flow` zero gives
    /// the stagnant (LOFC) leg.
    #[cfg(test)]
    pub fn advance(
        &mut self,
        dt: uom::si::f64::Time,
        bed_solid: ThermodynamicTemperature,
        bed_helium: ThermodynamicTemperature,
        cold_helium: ThermodynamicTemperature,
        mass_flow: MassRate,
    ) -> Power {
        self.solve_with_bed_held(
            bed_solid,
            bed_helium,
            cold_helium,
            mass_flow,
            Some(dt.get::<uom::si::time::second>()),
        );
        self.heat_from_core
    }

    /// Graphite reflector bulk temperature.
    pub fn reflector_temperature(&self) -> ThermodynamicTemperature {
        self.reflector_temperature
    }

    /// Reactor-pressure-vessel bulk temperature.
    pub fn rpv_temperature(&self) -> ThermodynamicTemperature {
        self.rpv_temperature
    }

    /// Heat rate leaving the pebble bed on the most recent step -- the
    /// passive loss the plant snapshot and trace report.
    pub fn heat_from_core(&self) -> Power {
        self.heat_from_core
    }

    /// Heat rate reaching the RCCS boundary on the most recent step.
    pub fn heat_to_rccs(&self) -> Power {
        self.heat_to_rccs
    }

    /// Heat rate given to the helium rising through the side-reflector
    /// channels on the most recent step (gh:#397) -- the cold-return CV's
    /// riser source.
    pub fn heat_to_risers(&self) -> Power {
        self.heat_to_risers
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::Time;
    use uom::si::time::second;

    /// Pebble-bed temperature the 206 kW comparison is taken at \[K\] -- the
    /// simulator's illustrative design-point bed temperature, kept as the
    /// comparison's pre-registered instrument (gh:#389 item 3 records that it
    /// is not the model's own settled state).
    const DESIGN_BED_TEMPERATURE_K: f64 = 950.0;

    /// **206 kW**, the HTR-10 surface cooling system's quoted duty -- Hu, S.,
    /// Wang, R., Gao, Z. (2006), *Nucl. Eng. Des.* **236**, 677-680, section
    /// 2.1, attributed there to Liang (2003). The V&V reference only.
    const DESIGN_PASSIVE_HEAT_LOSS_W: f64 = 206_000.0;

    fn k(t: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(t)
    }

    fn flow(kg_s: f64) -> MassRate {
        MassRate::new::<kilogram_per_second>(kg_s)
    }

    /// The published 250 degC core inlet -- the riser helium's temperature in
    /// these component tests (IAEA-TECDOC-1382 via `htr10::design`).
    fn cold() -> ThermodynamicTemperature {
        crate::physics::pebble_bed::design().helium_inlet_phase1
    }

    fn design_path() -> CoreToRccsPath {
        CoreToRccsPath::new_at_steady_state(
            k(DESIGN_BED_TEMPERATURE_K),
            k(DESIGN_BED_TEMPERATURE_K),
            cold(),
            flow(0.0),
        )
    }

    /// **Energy must not appear or vanish in the chain.** Over one step the
    /// heat leaving the bed must equal the heat reaching the RCCS plus the
    /// **enthalpy** stored in the two nodes (`m h(T)`, since 2026-09-29), at
    /// zero flow and at the rated flow.
    ///
    /// **Result (2026-09-17):** closes to better than 1e-9 relative.
    /// **Re-measured 2026-09-29** on enthalpy with the derived capacities and
    /// the Achenbach legs: printed; pass criterion unchanged (1e-9).
    #[test]
    fn the_chain_conserves_energy_over_a_step() {
        for m_dot in [0.0, 4.3] {
            let mut path = design_path();
            let bed = ThermodynamicTemperature::new::<degree_celsius>(900.0);
            let dt = Time::new::<second>(0.1);
            let e0 = path.stored_energy().get::<joule>();
            let q_in = path
                .advance(dt, bed, bed, cold(), flow(m_dot))
                .get::<watt>();
            // Out: the RCCS AND (since 2026-09-29, gh:#397) the riser helium.
            let q_out = path.heat_to_rccs().get::<watt>() + path.heat_to_risers().get::<watt>();
            let stored = path.stored_energy().get::<joule>() - e0;
            let residual = (q_in - q_out) * 0.1 - stored;
            println!("flow {m_dot} kg/s: in {q_in:.3} W, out {q_out:.3} W (risers {:.3} W), stored {stored:.6e} J, residual {residual:.3e} J", path.heat_to_risers().get::<watt>());
            assert!(
                (residual / (q_in * 0.1).abs().max(1.0)).abs() < 1.0e-9,
                "energy is not conserved across the chain at {m_dot} kg/s"
            );
        }
    }

    /// **The RCCS is a fixed 50 degC boundary, fed by vessel radiation.**
    ///
    /// Methodology: the sink must be exactly 50 degC (Jun et al. 2009 section
    /// 2.4, second-hand -- see the module doc) and must not move however long
    /// it receives heat; the heat reaching it must be `sigma A (T_rpv^4 -
    /// T_rccs^4)` at the vessel's solved temperature (1e-6 relative).
    #[test]
    fn the_rccs_is_a_fixed_50_c_sink_fed_by_vessel_radiation() {
        let sink = CoreToRccsPath::rccs_boundary();
        assert!((sink.get::<degree_celsius>() - 50.0).abs() < 1e-12);
        assert!((sink.get::<kelvin>() - 323.15).abs() < 1e-9);

        let mut path = design_path();
        let hot = ThermodynamicTemperature::new::<degree_celsius>(900.0);
        for _ in 0..1000 {
            path.advance(Time::new::<second>(0.1), hot, hot, cold(), flow(0.0));
            assert_eq!(
                CoreToRccsPath::rccs_boundary(),
                sink,
                "the sink must not drift"
            );
        }
        let t_v = path.rpv_temperature().get::<kelvin>();
        let expected =
            5.670374419e-8 * RPV_RADIATING_AREA_COEFF_M2 * (t_v.powi(4) - 323.15_f64.powi(4));
        let got = path.heat_to_rccs().get::<watt>();
        assert!(
            (got - expected).abs() / expected < 1e-6,
            "vessel -> RCCS must be sigma A (T^4 - T_rccs^4): {got:.3} W vs {expected:.3} W"
        );
    }

    /// **The implicit scheme is stable at any step and relaxes to the steady
    /// state.** One step of 1e10 s from the design state with the bed held
    /// 200 K hotter: every node between bed and sink, the balance closed to
    /// 1e-9, and `q_in ~ q_out` (the step still stores `sum(C dT)/dt`, ~W).
    #[test]
    fn a_huge_step_lands_on_the_steady_state() {
        let mut path = design_path();
        let bed = k(DESIGN_BED_TEMPERATURE_K + 200.0);
        let e0 = path.stored_energy().get::<joule>();
        let q_in = path
            .advance(Time::new::<second>(1.0e10), bed, bed, cold(), flow(0.0))
            .get::<watt>();
        let q_out = path.heat_to_rccs().get::<watt>();
        let (t_r, t_v) = (
            path.reflector_temperature().get::<kelvin>(),
            path.rpv_temperature().get::<kelvin>(),
        );
        assert!(
            bed.get::<kelvin>() > t_r && t_r > t_v && t_v > 323.15,
            "{t_r} {t_v}"
        );
        assert!(
            (q_in - q_out).abs() / q_in < 1e-3,
            "not settled: {q_in} vs {q_out}"
        );
        let stored = path.stored_energy().get::<joule>() - e0;
        let residual = stored - (q_in - q_out) * 1.0e10;
        assert!(
            residual.abs() / (q_in * 1.0e10) < 1e-9,
            "residual {residual:e} J"
        );
    }

    /// **Heat must flow downhill**, and **reverse rather than over-cool**: a
    /// bed hotter than the chain loses heat to it; a bed colder than the
    /// reflector gains heat from it.
    #[test]
    fn heat_flows_downhill_and_reverses_rather_than_over_cooling() {
        let mut path = design_path();
        let hot = ThermodynamicTemperature::new::<degree_celsius>(900.0);
        path.advance(Time::new::<second>(0.1), hot, hot, cold(), flow(4.3));
        assert!(path.heat_from_core().get::<watt>() > 0.0);
        assert!(path.heat_to_rccs().get::<watt>() > 0.0);

        let mut path = design_path();
        let cold_bed = ThermodynamicTemperature::new::<degree_celsius>(100.0);
        let q = path
            .advance(
                Time::new::<second>(0.1),
                cold_bed,
                cold_bed,
                cold(),
                flow(0.0),
            )
            .get::<watt>();
        assert!(
            q < 0.0,
            "with the bed colder than the reflector heat must flow INTO it ({q} W)"
        );
    }

    /// **The chain must open in equilibrium, carrying its design heat and
    /// perturbing nothing.**
    ///
    /// This is the invariant whose violation broke the plant: seeded out of
    /// equilibrium, the first link alone passed about 1.66 MW out of a 3 MW
    /// core, the core over-cooled, and the secondary side was driven to the
    /// water triple point until the steam tables refused the flash.
    ///
    /// Two things are asserted, and they are of **different kinds** -- keep
    /// them apart:
    ///
    /// 1. **An identity.** The heat leaving the core must equal the heat
    ///    reaching the RCCS. In equilibrium nothing is being stored, so any
    ///    difference between the two ends is an initialisation error. Gated
    ///    tightly, at 0.1 %.
    /// 2. **A COMPARISON against published data.** The derived duty is checked
    ///    against Hu's 206 kW with a wide band. This is V&V, not a fit.
    ///
    /// # Methodology
    ///
    /// Every conductance in the chain is computed from the HTR-10 R-Z zone map
    /// and the published vessel bore -- five legs, three of them temperature
    /// dependent (`k_eff(T)` on the bed, Stefan-Boltzmann on the gap and on the
    /// vessel). **Nothing is tuned to reproduce the reference.** The comparison
    /// is therefore capable of failing, which is the only thing that makes it
    /// evidence.
    ///
    /// Reference: Hu, Wang, Gao (2006), *Nucl. Eng. Des.* **236**, 677-680,
    /// section 2.1, attributed there to Liang (2003) -- the HTR-10 surface
    /// cooling system's quoted duty, **206 kW**.
    ///
    /// Pass criterion: within **25 %**. That band is set by the dominant
    /// assumption, not by the answer: reflector graphite conductivity ~~is~~
    /// **was** taken as 30 W/(m K) and irradiated nuclear graphite spans
    /// roughly 20-60, while that annulus carries about a quarter of the chain's
    /// resistance. (Since 2026-09-28 the annulus is IG-110 from `tuas`; see
    /// the 2026-09-28 results for what that did to the band's rationale.)
    ///
    /// # Results (2026-09-17)
    ///
    /// | Quantity | Derived | Published | Difference |
    /// |---|---|---|---|
    /// | passive duty at 950 K bed | **234.5 kW** | 206 kW | **+13.9 %** |
    ///
    /// Intermediate nodes settle at **411.1 degC** (reflector) and
    /// **198.2 degC** (RPV), the latter under the 350 degC vessel limit the
    /// HTR-10 test procedure lists. Both ends carry the same heat to within
    /// machine precision.
    ///
    /// **Interpretation.** A 14 % agreement obtained without fitting is a
    /// genuine, if loose, corroboration of the chain: the geometry, the
    /// mechanisms and the material properties together reproduce an
    /// independently published duty. It is *not* a validation of HTR-10
    /// cooldown behaviour -- that needs the transient, and the transient
    /// depends on the heat capacities, which are still partly assumed (the RPV
    /// wall thickness is not published anywhere in this workspace). The
    /// derived value is **high**, i.e. this model removes heat slightly faster
    /// than the reference, which is **non-conservative** for a heat-up
    /// transient and must be stated wherever a peak temperature is quoted.
    ///
    /// # Results (2026-09-28) -- MOVED, and closer to the band edge
    ///
    /// | Quantity | 2026-09-17 | **2026-09-28** | Published |
    /// |---|---|---|---|
    /// | passive duty at 950 K bed | 234.5 kW (+13.9 %) | **254.0 kW (+23.3 %)** | 206 kW |
    /// | reflector node | 411.1 degC | 411.6 degC | -- |
    /// | RPV node | 198.2 degC | 205.6 degC | -- |
    ///
    /// **What moved it:** the graphite reflector annulus now takes IG-110 from
    /// `tuas` (unirradiated, at the reflector temperature, 40.2 W/(m K) at the
    /// 684.8 K node) instead of the assumed 30 W/(m K) -- predicted beforehand
    /// to raise the duty (higher `k` on a leg carrying ~25 % of the
    /// resistance), and it did, by +8.3 %. **Nothing was tuned back.** The
    /// comparison is now **worse**, and the model removes heat faster than the
    /// reference by nearly a quarter -- more non-conservative for a heat-up.
    ///
    /// **The 25 % band is left where it was, and its stated justification no
    /// longer holds.** It was set by the uncertainty of the assumed 30 W/(m K);
    /// that assumption is gone, replaced by a sourced but *unirradiated*
    /// conductivity (irradiated graphite conducts worse, which would lower the
    /// duty) and a still-assumed boronated-annulus 30 W/(m K). Loosening the
    /// band to keep the margin would be moving a gate to pass; it was not done.
    /// Candidate physics for the remaining +23 %: reflector irradiation, the
    /// flat-above-2000 K ZBS table is irrelevant here (950 K), the assumed
    /// emissivities on the two radiative legs, and whether Hu's 206 kW
    /// describes the same heat path.
    ///
    /// ~~Previously asserted equality with 206 kW to 0.1 %.~~ **CORRECTED
    /// 2026-09-17** -- that only held because the conductances had been fitted
    /// to produce it, so the test restated its own input.
    ///
    /// # Results (2026-09-29, gh:#395/#396) -- the gated instrument is unchanged; a forced-flow reading is added
    ///
    /// The **gated** comparison stays where it was pre-registered: the chain
    /// alone, in equilibrium with a 950 K bed, **stagnant** (no flow) -- the
    /// instrument chosen before any of today's changes, kept so the gate is
    /// not re-chosen after seeing a result. Stage (b) changed nothing on that
    /// path (the stagnant limit of the new bed -> reflector network is exactly
    /// the old leg 1 + inner annulus half, and steady states do not depend on
    /// capacities), so it must reproduce 254.0 kW; see the printed values.
    ///
    /// **Added, not gated: the same 950 K bed at the rated 4.3 kg/s.** Hu's
    /// 206 kW is a normal-operation duty, i.e. under forced flow, so this is
    /// the reading that describes the same state as the reference. With the
    /// Achenbach dispersion and wall-film legs now present, it is expected to
    /// be **higher** than the stagnant one (more conductance in the first leg
    /// of a series chain). Printed and recorded on gh:#395. The bed helium is
    /// taken at the solid's 950 K -- an instrument choice, stated: at 10 MW
    /// the real helium node sits tens of kelvin below the solid, which would
    /// lower this reading. The model's own settled passive loss is in the
    /// headless trace (`passive_loss_mw`) and is reported alongside.
    ///
    /// | Reading (2026-09-29) | Derived | Published | Difference |
    /// |---|---|---|---|
    /// | stagnant, 950 K bed (**gated**) | **254.0 kW**, reflector 411.6 degC, RPV 205.6 degC | 206 kW | **+23.3 %** (unchanged, as predicted) |
    /// | 4.3 kg/s, 950 K bed and helium (reported) | **316.5 kW**, reflector 463.0 degC, RPV 227.4 degC | 206 kW | **+53.6 %** |
    /// | the plant's own state at 300 s (headless trace) | **784.8 kW** at a 1303 K bed | 206 kW | -- (a 1303 K bed, not a design state) |
    ///
    /// # Results after the riser leg (2026-09-29, gh:#397) -- and which quantity is Hu's
    ///
    /// **Which quantity Hu's 206 kW is.** Hu, Wang & Gao (2006), section 2.1,
    /// list it among "the basic data used for the analysis": "(1) The power
    /// dissipated by surface cooling system: 206 kW (Liang, 2003)" -- the
    /// **RCCS duty**. The source does not say at which plant state, so the
    /// forced-flow reading is not more authoritative than the stagnant one; it
    /// is the reading for a different state, reported, not gated.
    ///
    /// **Both quantities, before and after the riser leg** (steady state):
    ///
    /// | Reading | stage (b): bed -> reflector | stage (b): RCCS | stage (c): bed -> reflector | stage (c): RCCS |
    /// |---|---|---|---|---|
    /// | stagnant, 950 K (gated) | 254.0 kW | 254.0 kW | 254.0 kW | 254.0 kW (+23.3 %) |
    /// | 4.3 kg/s, 950 K (reported) | 316.5 kW | 316.5 kW (+53.6 %) | 589.1 kW | **134.6 kW (-34.7 %)** |
    ///
    /// **The RCCS duty is the reading that counts**, because it is Hu's
    /// quantity. Until the riser leg the two columns are the same number at
    /// steady state (nothing leaves the chain between the bed and the RCCS),
    /// so the stage (b) "+53.6 %" -- ~~recorded as the chain duty~~ -- was
    /// the RCCS duty as well, not a different quantity; the comparison did
    /// not change instrument, the physics split the two numbers. With the
    /// risers, part of the bed -> reflector heat goes back into the helium
    /// (it is not lost from the plant), which is why the bed -> reflector
    /// duty no longer measures what Hu's figure measures. Both are printed:
    ///
    /// | Reading | bed -> reflector | to risers | **to RCCS (Hu's quantity)** | reflector / RPV |
    /// |---|---|---|---|---|
    /// | stagnant, 950 K (**gated**, bed->reflector) | 254.0 kW | 0 | **254.0 kW (+23.3 %)** | 411.6 / 205.6 degC |
    /// | 4.3 kg/s, 950 K, risers at 250 degC | 589.1 kW | 454.5 kW | **134.6 kW (-34.7 %)** | 295.9 / 153.0 degC |
    ///
    /// The gate is unchanged (at zero flow both quantities are the same
    /// 254.0 kW, and the riser leg vanishes). On forced flow the risers now
    /// hold the reflector near the cold helium, so **less** heat reaches the
    /// vessel and the RCCS than Hu's figure -- the sign has flipped from the
    /// stage (b) reading, and the -34.7 % is reported, not tuned. The riser
    /// leg's assumption (a) (channel walls at the node temperature) is the
    /// one that pushes this reading low.
    ///
    /// **Interpretation (stage (b) reading, superseded above).** On the
    /// reading that describes the same state as the reference (forced flow),
    /// the model removed heat about half as fast again as Hu's figure. Nothing was tuned to close that. Candidate
    /// physics, for the record: the dispersion branch sees the
    /// outlet-referenced helium over the whole wall (assumption (d) in the
    /// module doc, an over-statement); the riser leg (gh:#397, stage (c)) is
    /// not yet here, and it will cool the reflector and so raise this further;
    /// unirradiated reflector conductivity; the assumed emissivities; and
    /// whether Hu's 206 kW is the same heat path. The ungated reading is not
    /// promoted to a gate after seeing it.
    #[test]
    fn the_chain_opens_in_equilibrium_at_its_design_heat() {
        let bed = k(DESIGN_BED_TEMPERATURE_K);
        let rel = |a: f64, b: f64| (a - b).abs() / b.abs().max(1.0);

        // The pre-registered, GATED instrument: stagnant, 950 K.
        let path = design_path();
        let q_core = path.heat_from_core().get::<watt>();
        let q_sink = path.heat_to_rccs().get::<watt>();
        let discrepancy = 100.0 * (q_core / DESIGN_PASSIVE_HEAT_LOSS_W - 1.0);
        println!(
            "STAGNANT 950 K (gated): core {:.1} kW, RCCS {:.1} kW; reflector {:.1} degC, RPV {:.1} degC; \
             {discrepancy:+.1} % against 206 kW (derived, NOT fitted)",
            q_core / 1e3,
            q_sink / 1e3,
            path.reflector_temperature().get::<degree_celsius>(),
            path.rpv_temperature().get::<degree_celsius>(),
        );

        // Added, not gated: the same bed at the rated flow.
        let forced = CoreToRccsPath::new_at_steady_state(bed, bed, cold(), flow(4.3));
        let q_forced = forced.heat_from_core().get::<watt>();
        println!(
            "FORCED 950 K, 4.3 kg/s, risers at 250 degC (reported, not gated): core {:.1} kW \
             ({:+.1} % against 206 kW), of which {:.1} kW to the riser helium and {:.1} kW to the \
             RCCS; reflector {:.1} degC, RPV {:.1} degC",
            q_forced / 1e3,
            100.0 * (q_forced / DESIGN_PASSIVE_HEAT_LOSS_W - 1.0),
            forced.heat_to_risers().get::<watt>() / 1e3,
            forced.heat_to_rccs().get::<watt>() / 1e3,
            forced.reflector_temperature().get::<degree_celsius>(),
            forced.rpv_temperature().get::<degree_celsius>(),
        );

        assert!(
            rel(q_core, DESIGN_PASSIVE_HEAT_LOSS_W) < 0.25,
            "the geometry-derived chain must agree with the published HTR-10 \
             surface-cooling duty to 25 %: got {q_core:.1} W ({discrepancy:+.1} %). Do NOT fix \
             this by tuning a conductance until it passes."
        );
        assert!(
            rel(q_core, q_sink) < 1.0e-3,
            "in equilibrium the heat leaving the core must equal the heat reaching the RCCS"
        );
        assert!(
            q_forced >= q_core,
            "adding the flow-dispersion and riser legs cannot reduce the heat leaving the bed"
        );
    }

    /// V&V (gh:#396): **the reflector and RPV capacities are derived** --
    /// `m c_p(T)` from the annulus geometry, published densities and
    /// Butland-Maddison graphite / Kim steel `c_p(T)` -- and they replace the
    /// invented constants.
    ///
    /// # Methodology
    ///
    /// Recompute the masses independently from the stated inputs --
    /// graphite annulus `pi (1.67793^2 - 0.90^2) 4.70 - 1.513575 m^3` at
    /// 1770 kg/m^3; boronated brick `pi (1.90^2 - 1.67793^2) 6.10 m^3` at
    /// 1590; RPV shell `pi (2.20^2 - 2.10^2) 11.1 m^3` at the Kim steel's
    /// 300 K density -- and require the module's masses to match to 1e-12.
    /// Then print the capacities at the design-state node temperatures
    /// against the deleted constants (1.8e8 and 6.0e7 J/K), and require the
    /// secant capacity to reproduce the enthalpy change exactly.
    ///
    /// **Fails on the pre-change module**: the capacities were those two
    /// constants, independent of `T` and of any mass.
    ///
    /// # Results (2026-09-29)
    ///
    /// Masses: graphite **49.7 t**, boronated brick **24.2 t**, RPV
    /// **118.4 t** (Kim steel at 7894.2 kg/m^3). Capacities at the design-state
    /// nodes (reflector 684.7 K, RPV 478.8 K): reflector **1.1115e8 J/K**
    /// (where the invented value was 1.8e8: **-38 %**), RPV **6.3209e7 J/K**
    /// (where the invented value was 6.0e7: +5 %; this one rests on the
    /// invented 0.10 m wall). The secant capacity reproduces the enthalpy
    /// change to 1e-12. A smaller reflector capacity shortens every passive
    /// cooldown time constant it sets.
    #[test]
    fn the_reflector_and_rpv_capacities_are_derived() {
        let pi = std::f64::consts::PI;
        let graphite = 1770.0 * (pi * (1.67793f64.powi(2) - 0.81) * 4.70 - 1.513575);
        let brick = 1590.0 * pi * (3.61 - 1.67793f64.powi(2)) * 6.10;
        let steel_rho = solid_density(RPV_STEEL);
        let rpv = steel_rho * pi * (2.2f64.powi(2) - 2.1f64.powi(2)) * 11.1;
        let rel = |a: f64, b: f64| (a - b).abs() / b;
        assert!(rel(reflector_graphite_mass().get::<kilogram>(), graphite) < 1e-12);
        assert!(rel(boronated_brick_mass().get::<kilogram>(), brick) < 1e-12);
        assert!(rel(rpv_mass().get::<kilogram>(), rpv) < 1e-12);

        let path = design_path();
        let (t_r, t_v) = (path.reflector_temperature(), path.rpv_temperature());
        let c = path.coupling(
            k(DESIGN_BED_TEMPERATURE_K),
            k(DESIGN_BED_TEMPERATURE_K),
            cold(),
            flow(0.0),
            t_r,
            t_v,
        );
        println!(
            "masses: graphite {:.1} t, boronated brick {:.1} t, RPV {:.1} t (steel rho {steel_rho:.1}); \
             capacities at T_refl {:.1} K / T_rpv {:.1} K: reflector {:.4e} J/K (was 1.8e8 invented), \
             RPV {:.4e} J/K (was 6.0e7 invented)",
            graphite / 1e3,
            brick / 1e3,
            rpv / 1e3,
            t_r.get::<kelvin>(),
            t_v.get::<kelvin>(),
            c.reflector_capacity,
            c.rpv_capacity,
        );
        // The secant capacity is the enthalpy change, exactly.
        let t_new = k(t_r.get::<kelvin>() + 50.0);
        let secant = path
            .coupling(k(950.0), k(950.0), cold(), flow(0.0), t_new, t_v)
            .reflector_capacity
            * 50.0;
        let m = (reflector_graphite_mass() + boronated_brick_mass()).get::<kilogram>();
        let dh = m
            * (solid_enthalpy(REFLECTOR_GRAPHITE, t_new) - solid_enthalpy(REFLECTOR_GRAPHITE, t_r));
        assert!(rel(secant, dh) < 1e-12);
    }

    /// V&V (gh:#395): **the bed -> reflector network is Achenbach's**, and
    /// behaves the way its equations say.
    ///
    /// # Methodology
    ///
    /// 1. `K_r` equals `8 [2 - (1 - 2 d/D)^2]` (eq. (30)) for d = 0.06 m,
    ///    D = 1.8 m, recomputed here.
    /// 2. At the rated 4.3 kg/s and 750 K helium, the dispersion conductance
    ///    equals `8 pi H lambda_g Re Pr / K_r` and the wall film `(1 - d/D)
    ///    Re^0.61 Pr^(1/3) lambda_g / d x pi D H` (eq. (34)), both recomputed
    ///    from the helium transport properties.
    /// 3. At zero flow the dispersion branch is zero and the film has no
    ///    resistance (`Re < 100`, `alpha_w -> infinity`), so the leg is the
    ///    stagnant ZBS branch in series with the annulus half -- the old leg.
    /// 4. The dispersion conductance rises with flow.
    ///
    /// # Results (2026-09-29)
    ///
    /// Rated 4.3 kg/s, 750 K helium: `Re = 2688.0` (inside eq. (34)'s
    /// 50-2e4), `Pr = 0.6601`, `lambda_g = 0.2966 W/(m K)`, `K_r = 9.0311`.
    /// The dispersion conductivity `lambda_k = 58.28 W/(m K)` is **2.9 times**
    /// the stagnant ZBS `lambda_0 = 20.25`; `G_s = 1002.4 W/K`, `G_f = 2885.6
    /// W/K`, wall film `5728.7 W/K` (`alpha_w = 514.2 W/(m^2 K)`). At zero
    /// flow the leg reduces exactly to the pre-change leg.
    #[test]
    fn the_bed_to_reflector_legs_follow_achenbach() {
        let (d, big_d, h) = (0.06f64, 1.8f64, 1.97f64);
        let k_r = 8.0 * (2.0 - (1.0 - 2.0 * d / big_d).powi(2));
        assert!((radial_turbulent_peclet() - k_r).abs() < 1e-12);

        let he = k(750.0);
        let (kg, pr, mu) = crate::physics::pebble_bed::helium_transport(he);
        let area = std::f64::consts::PI * big_d * big_d / 4.0;
        let re = 4.3 / area * d / mu;
        let g_f = ua_helium_to_near_wall_w_per_k(he, flow(4.3));
        let expected_gf = 8.0 * std::f64::consts::PI * h * kg * re * pr / k_r;
        let film = ua_wall_film_w_per_k(he, flow(4.3)).expect("rated flow is above Re = 100");
        let expected_film = (1.0 - d / big_d) * re.powf(0.61) * pr.powf(1.0 / 3.0) * kg / d
            * std::f64::consts::PI
            * big_d
            * h;
        let g_s = ua_solid_to_near_wall_w_per_k(he);
        println!(
            "rated 4.3 kg/s, 750 K helium: Re = {re:.1}, Pr = {pr:.4}, lambda_g = {kg:.4} W/(m K), \
             K_r = {k_r:.4}; lambda_k = {:.3} W/(m K) vs lambda_0 = {:.3}; G_s = {g_s:.1} W/K, \
             G_f = {g_f:.1} W/K, wall film {film:.1} W/K (alpha_w = {:.1} W/(m^2 K))",
            kg * re * pr / k_r,
            outram_park_digital_twin_engine::htr10::zbs::zbs_effective_conductivity(he)
                .get::<uom::si::thermal_conductivity::watt_per_meter_kelvin>(),
            film / (std::f64::consts::PI * big_d * h),
        );
        assert!((g_f - expected_gf).abs() / expected_gf < 1e-12);
        assert!((film - expected_film).abs() / expected_film < 1e-12);

        assert!(ua_helium_to_near_wall_w_per_k(he, flow(0.0)) == 0.0);
        assert!(ua_wall_film_w_per_k(he, flow(0.0)).is_none());
        let refl = k(700.0);
        assert!(
            (ua_near_wall_to_reflector_w_per_k(he, flow(0.0), refl)
                - 2.0 * ua_graphite_annulus_w_per_k(refl))
            .abs()
                < 1e-9
        );
        assert!(
            ua_helium_to_near_wall_w_per_k(he, flow(2.0))
                < ua_helium_to_near_wall_w_per_k(he, flow(4.3))
        );
    }

    /// V&V (gh:#397): **the riser leg uses Gnielinski, is bounded by the
    /// stream, and vanishes without flow.**
    ///
    /// # Methodology
    ///
    /// 1. Geometry: the derived channel length `V/(20 pi d^2/4)` equals
    ///    0.507681 / (20 pi 0.08^2 / 4) m, recomputed here.
    /// 2. At the rated 4.3 kg/s and the published 250 degC inlet, print the
    ///    riser Reynolds number, the Gnielinski Nusselt number `tuas`
    ///    returns, and the Dittus-Boelter cross-check `0.023 Re^0.8 Pr^0.4`
    ///    (heating); require the two to agree within 25 % (Dittus-Boelter's
    ///    usual scatter band against Gnielinski in fully turbulent flow).
    /// 3. The leg never exceeds the stream capacity `m_r c_p` and is zero at
    ///    zero flow; it rises with flow.
    ///
    /// # Results (2026-09-29)
    ///
    /// At 4.3 kg/s and 523.15 K: `L = 5.050 m`, `Re = 103 867` (turbulent),
    /// `Pr = 0.6585`; **Nu_Gnielinski = 186.77**, Nu_Dittus-Boelter = 200.60
    /// (**+7.4 %**, inside the band); `h = 539.9 W/(m^2 K)`, `hA = 13.70 kW/K`
    /// against the stream's `m_r c_p = 19.87 kW/K` (NTU 0.690), so the leg is
    /// **9.90 kW/K** -- the largest conductance out of the reflector at power,
    /// about 5x the bed -> reflector leg it now competes with.
    #[test]
    fn the_riser_leg_uses_gnielinski_and_vanishes_without_flow() {
        let expected_l = 0.507681 / (20.0 * std::f64::consts::PI * 0.08f64.powi(2) / 4.0);
        assert!((riser_channel_length_m() - expected_l).abs() < 1e-12);

        let (nu, re, pr, k_g) = riser_nusselt_and_reynolds(cold(), flow(4.3));
        let db = tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::pipe_correlations::dittus_boelter_correlation(re, pr, true);
        let g = ua_reflector_to_risers_w_per_k(cold(), flow(4.3));
        let cp = crate::physics::pebble_bed::helium_specific_heat(cold())
            .get::<joule_per_kilogram_kelvin>();
        let capacity = riser_mass_flow_kg_s(flow(4.3)) * cp;
        let h = nu * k_g / RISER_CHANNEL_DIAMETER_M;
        let area =
            20.0 * std::f64::consts::PI * RISER_CHANNEL_DIAMETER_M * riser_channel_length_m();
        println!(
            "riser at 4.3 kg/s, 523.15 K: L = {:.3} m, Re = {re:.0}, Pr = {pr:.4}, Nu_Gnielinski = \
             {nu:.2}, Nu_Dittus-Boelter = {db:.2} ({:+.1} %), h = {h:.1} W/(m^2 K), hA = {:.1} W/K, \
             m_r c_p = {capacity:.1} W/K, NTU = {:.3}, G = {g:.1} W/K",
            riser_channel_length_m(),
            100.0 * (db / nu - 1.0),
            h * area,
            h * area / capacity,
        );
        assert!((db / nu - 1.0).abs() < 0.25);
        assert!(g > 0.0 && g <= capacity);
        assert_eq!(ua_reflector_to_risers_w_per_k(cold(), flow(0.0)), 0.0);
        assert!(ua_reflector_to_risers_w_per_k(cold(), flow(2.0)) < g);
    }
}
