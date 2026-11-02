//! The Mark I containment and the reactor building. This is what the
//! **containment worker** runs.
//!
//! ## Model (AI draft; read the limitations)
//!
//! - **Drywell**: nitrogen, hydrogen and steam at one temperature, ideal
//!   gases (steam's energy referenced to IF97's saturated vapour at
//!   100 degC with a constant `c_v`; N2 and H2 with MELCOR's `c_v(T)`
//!   fits, [`super::gas`], since 2026-10-29); a lumped wall heat sink (steel and
//!   concrete) takes heat by convection, **implicitly**; steam above its
//!   saturation pressure at the gas temperature condenses to the sump.
//!   **Since 2026-10-29** (`Mechanisms::dw_wall_hs`) the wall's coefficient
//!   is MELCOR's natural convection and steam condenses on it by diffusion
//!   through the non-condensables, its latent heat into the wall
//!   ([`super::pool_surface::structure_surface`]); the fixed coefficient is
//!   the ablation. The wall's area and heat capacity are still \[A\].
//! - **Wetwell (torus)**: the pool, one temperature; ~~the gas space above it
//!   at the pool temperature, saturated with steam (an equilibrium
//!   assumption)~~ **since 2026-10-29** (`Mechanisms::ww_atmosphere`) the gas
//!   space has its own temperature and steam, exchanging mass and energy
//!   with the pool at its surface and with the bubbles leaving it
//!   ([`super::pool_surface`]: MELCOR CVH/FL RM §5.1 and §6.1); the old
//!   equilibrium is the ablation. ~~The torus shell is not a heat structure
//!   (stated).~~ The torus shell above the pool is a heat structure on it
//!   since 2026-10-29 (`Mechanisms::ww_shell`, its geometry \[A\], its outer
//!   face adiabatic). Cost: the 30 h headless run 7.8 -> 11.1 s.
//! - **Downcomers**: drywell gas pushed into the pool once the drywell
//!   exceeds the wetwell by the submergence head; the steam condenses in the
//!   pool, the nitrogen and hydrogen bubble through into the wetwell gas at
//!   the pool's temperature. (With the wetwell atmosphere, by MELCOR's bubble model: below
//!   about 5 K of subcooling part of the steam passes uncondensed.) **Vacuum breakers** the other way.
//! - **Relief-valve discharge** goes into the pool (quenchers), its hydrogen
//!   into the wetwell gas.
//! - **Wetwell vent** to the stack through a rupture disk, when the operators
//!   open it; **drywell head-flange leak** to the reactor building at high
//!   pressure; the **reactor building** is well mixed at atmospheric pressure
//!   and its hydrogen fraction is compared with the flammability limit and a
//!   strong-deflagration threshold.
//!
//! The downcomers, the vacuum breakers and the leaks are stiff at a 1 s
//! step: when an orifice could pass more than equalises the two volumes in a
//! step, the transfer is the equalising amount (the implicit answer of a
//! stiff path), never more. ~~(True as stated before 2026-10-30.)~~
//! **CORRECTED 2026-10-30:** the cap counted only the donor's pressure
//! fall, so it overshot the set point by about the ratio of the volumes,
//! and the vacuum breaker chattered about it every step, amplifying a
//! 1e-9 perturbation 1.5 times a step; with
//! `Mechanisms::two_sided_equalising` it counts the receiver's rise too
//! (`R T / V = p / n` for each gas) and lands on the set point. **Not in the model**: drywell spray, ~~wetwell
//! stratification,~~ pool level and swell, ~~pool scrubbing of aerosols (no
//! fission products are tracked yet),~~ the torus room, the reactor building's
//! own heat-up and ventilation, steam in the building, and the building's
//! pressure. ~~And the combustion itself (the explosion is a threshold, not
//! a calculation).~~ **CORRECTED 2026-10-30:** with
//! `Mechanisms::burn_dynamics` a burn runs MELCOR's completeness, flame
//! speed and rate (BUR RM §2.3 to §2.5); its pressure rise is reported as
//! an adiabatic bound, since the building is held at one atmosphere.
//! **CORRECTED 2026-10-30** (struck above; checked in the code): the pool
//! stratifies in two layers (`Mechanisms::pool_layers`), it scrubs aerosols
//! (`Mechanisms::pool_scrubbing`, SPARC-90 in [`super::sparc`]), and the
//! fission products are tracked ([`super::rn_volume`]).

use super::params::*;
use super::aerosol::Sections;
use super::gas::{h2_molar_enthalpy, ncg_c, ncg_u};
use super::containment_fp::{DowncomerFlow, FpHold, PoolSide, RnContext};
use super::rn_volume::RnFlow;
use super::fdi;
use super::sparc;
use super::pool_surface;
use super::cavity::Cavity;
use petir::mathf::RealMath;
use super::steam::p_sat_pa;
use super::wire::{Reader, Writer};

/// Steam: constant `c_v`, J/(kg K), and IF97 saturated-vapour internal
/// energy at 373.15 K, J/kg. \[A\] constant `c_v`.
const CV_STEAM: f64 = 1500.0;
const U_STEAM_REF: f64 = 2.5060e6;
const T_REF: f64 = 373.15;
/// Liquid water: constant `c_p`, J/(kg K). \[A\]
const CP_WATER: f64 = 4186.0;
/// Drywell structures: area, convective coefficient and heat capacity are
/// inputs since 2026-10-29 (`Params::dw_wall_area_m2`, `dw_wall_h`,
/// `dw_wall_c`; still \[A\] placeholders).
const T_RB: f64 = 303.15;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ContainmentSnapshot {
    pub dt_s: f64,
    /// Relief-valve discharge into the pool: steam kg/s, its enthalpy J/kg,
    /// hydrogen kg/s.
    pub srv_steam_kg_s: f64,
    pub srv_h: f64,
    pub srv_h2_kg_s: f64,
    /// RPV leak (and, after lower-head failure, the breach) into the drywell:
    /// steam kg/s, enthalpy J/kg, hydrogen kg/s, gas temperature K.
    pub leak_steam_kg_s: f64,
    pub leak_h: f64,
    pub leak_h2_kg_s: f64,
    pub leak_gas_k: f64,
    /// Operators' order to open the wetwell vent (the disk must still burst).
    pub vent_ordered: bool,
    /// After the lower head fails: the debris's decay power on the drywell
    /// floor, W, and the injected water that now pours through, kg/s (at
    /// the injection temperature).
    pub ex_vessel_w: f64,
    pub pedestal_inflow_kg_s: f64,
    /// Water draining from a failed vessel into the pedestal, kg/s, and its
    /// temperature, K (`Mechanisms::failed_vessel_drain`).
    pub vessel_drain_kg_s: f64,
    pub vessel_drain_k: f64,
    /// MELCOR's `c_v(T)` for N2 and H2 ([`super::gas`]); false is the
    /// constant `c_v` ablation.
    pub gas_cv_t: bool,
    /// Noble gas (Xe) arriving this step through the relief valves (into
    /// the wetwell gas) and through the leaks (into the drywell), as
    /// fractions of the core inventory (gh:#1040).
    pub xe_relief: f64,
    pub xe_leak: f64,
    /// Cesium (as vapour) and UO2 aerosol arriving this step through the
    /// relief valves and the leaks, kg (gh:#1047).
    pub cs_relief_kg: f64,
    pub cs_leak_kg: f64,
    pub uo2_relief_kg: f64,
    pub uo2_leak_kg: f64,
    /// Iodine (as I2 vapour) likewise, kg.
    pub i_relief_kg: f64,
    pub i_leak_kg: f64,
    /// Pool scrubbing by steam condensation at the vent exits (SPARC-90's
    /// DF_EC; `Mechanisms::pool_scrubbing`).
    pub pool_scrubbing: bool,
    /// MELCOR's `X_o` in that scrubbing, and the impaction and swarm-rise
    /// factors past the downcomers (`Mechanisms::scrubbing_saturation`,
    /// `Mechanisms::swarm_scrubbing`).
    pub scrubbing_saturation: bool,
    pub swarm_scrubbing: bool,
    /// Fission-product classes on the aerosol tracked by section
    /// (`Mechanisms::sectional_classes`).
    pub sectional_classes: bool,
    /// The iodine partition limit on the condensation scrubbing
    /// (`Mechanisms::iodine_partition`).
    pub iodine_partition: bool,
    /// The wetwell vent passes the gas mixture (`Mechanisms::mixture_vent`).
    pub mixture_vent: bool,
    /// Thermophoresis and diffusiophoresis onto the drywell's structures
    /// (`Mechanisms::phoresis`).
    pub phoresis: bool,
    /// The swarm rise's scrubbing of I2 vapour (`Mechanisms::iodine_swarm`).
    pub iodine_swarm: bool,
    /// The vessel's aerosol and fission products leaving this step through
    /// the relief valves and the leaks, by section (`Mechanisms::
    /// vessel_aerosol`); empty, the scalar `*_relief_kg` and `*_leak_kg`
    /// above carry them instead.
    pub rn_relief: RnFlow,
    pub rn_leak: RnFlow,
    /// Condensed classes add to their aerosol section's mass
    /// (`Mechanisms::condensate_mass`).
    pub condensate_mass: bool,
    /// Decay heat of the noble-gas, alkali and halogen classes for the
    /// whole core inventory, W (`fp_decay`; zero without
    /// `Mechanisms::fp_decay_follows`): each location takes its share of the
    /// class's inventory.
    pub q_noble_w: f64,
    pub q_alkali_w: f64,
    pub q_halogen_w: f64,
    /// And the chalcogens' (Te/Se), when tellurium is transported
    /// (`Mechanisms::te_transport`).
    pub q_chalcogen_w: f64,
    /// The wetwell atmosphere has its own temperature and steam, and
    /// exchanges mass and energy with the pool at its surface and with the
    /// bubbles leaving the pool (`Mechanisms::ww_atmosphere`).
    pub ww_atmosphere: bool,
    /// The drywell structures as MELCOR's heat-structure surface: natural
    /// convection and diffusion-limited condensation (`Mechanisms::
    /// dw_wall_hs`); false is the fixed coefficient `dw_wall_h`.
    pub dw_wall_hs: bool,
    /// The torus shell above the pool as a heat structure on the wetwell
    /// atmosphere (`Mechanisms::ww_shell`; needs `ww_atmosphere`).
    pub ww_shell: bool,
    /// Gas leaving the drywell (downcomers, flange leak) takes its enthalpy
    /// (`Mechanisms::outflow_enthalpy`); false is its internal energy, the
    /// drywell keeping its temperature.
    pub outflow_enthalpy: bool,
    /// The relief valves' hydrogen arrives with its enthalpy at
    /// `srv_gas_k`, the vessel's saturation temperature, as the vessel gave
    /// it up (`Mechanisms::relief_h2_enthalpy`; needs `ww_atmosphere`);
    /// false is at the pool's temperature.
    pub relief_h2_enthalpy: bool,
    pub srv_gas_k: f64,
    /// Water condensed in the drywell drains to its floor, the pedestal's
    /// pool (`Mechanisms::condensate_to_floor`); false is it leaving the
    /// model.
    pub condensate_to_floor: bool,
    /// Heat through the RPV's insulation into the drywell gas, W
    /// (`Mechanisms::rpv_wall`).
    pub rpv_wall_w: f64,
    /// Molten core-concrete interaction on the drywell floor
    /// (`Mechanisms::mcci`), and the debris there: the core's UO2 and
    /// metallic Zr, kg, and their temperature, K.
    pub mcci: bool,
    /// The drywell structures lose heat across the gap to the concrete
    /// around them, which conducts it to the reactor building
    /// (`Mechanisms::dw_wall_outer`).
    pub dw_wall_outer: bool,
    /// The torus pool in two layers, split at the downcomers' exits
    /// (`Mechanisms::pool_layers`).
    pub pool_layers: bool,
    /// The cavity debris melts over a range and conducts when solid
    /// (`Mechanisms::cavity_phases`).
    pub cavity_phases: bool,
    /// The building's burn starts by MELCOR's BUR criteria
    /// (`Mechanisms::bur_ignition`).
    pub bur_ignition: bool,
    /// Gas flows to the atmosphere are limited to the critical flow
    /// (`Mechanisms::gas_choking`).
    pub gas_choking: bool,
    /// The downcomers' and vacuum breakers' equalising cap counts both
    /// volumes (`Mechanisms::two_sided_equalising`).
    pub two_sided_equalising: bool,
    /// A burn runs MELCOR's completeness, flame speed and rate
    /// (`Mechanisms::burn_dynamics`).
    pub burn_dynamics: bool,
    /// Debris arriving in the pedestal falls through its water by MELCOR's
    /// low-pressure melt ejection (`Mechanisms::fdi_lpme`).
    pub fdi_lpme: bool,
    /// The vessel's pressure, Pa, for the ejection velocity.
    pub vessel_p_pa: f64,
    pub cav_uo2_kg: f64,
    pub cav_zr_kg: f64,
    pub cav_debris_k: f64,
    /// The drywell spray (gh:#1109; Unit 3's fire pumper): water, kg/s, at
    /// `spray_k`, K; with `spray_exchange` (`Mechanisms::containment_spray`)
    /// it reaches equilibrium with the drywell's atmosphere as it falls
    /// ([`spray_condensation`]), without it the water lands on the floor
    /// unchanged.
    pub spray_kg_s: f64,
    pub spray_k: f64,
    pub spray_exchange: bool,
    /// The plant's inputs (gh:#1033).
    pub prm: Params,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ContainmentResult {
    pub p_dw_pa: f64,
    pub p_ww_pa: f64,
    pub t_dw_k: f64,
    /// The pool's temperature, K: its mean over both layers with
    /// `pool_layers`.
    pub t_pool_k: f64,
    /// The pool's surface layer (above the downcomers' exits), K; the
    /// whole pool without `pool_layers`.
    pub t_pool_top_k: f64,
    pub dw_h2_kg: f64,
    pub ww_h2_kg: f64,
    pub rb_h2_fraction: f64,
    pub vent_flowing: bool,
    pub flange_leaking: bool,
    pub rb_flammable: bool,
    pub rb_exploded: bool,
    /// The building's Le Chatelier combustible fraction, `X_H2 + X_CO
    /// L_H2 / L_CO` (MELCOR BUR RM Eq 2-3), and its CO fraction.
    pub rb_burn_y: f64,
    pub rb_co_fraction: f64,
    /// A burn is running in the building (`burn_dynamics`).
    pub rb_burning: bool,
    /// The largest burn's pressure rise, Pa, as an adiabatic,
    /// constant-volume bound `Q / (2.5 V)` (no venting, no heat loss; the
    /// building itself is held at one atmosphere, stated).
    pub rb_burn_dp_pa: f64,
    /// Steam raised by debris falling through the pedestal's water, kg
    /// (`fdi_lpme`), and whether an ejection was fast enough for MELCOR's
    /// high-pressure model (flagged, still treated as low-pressure).
    pub fdi_steam_kg: f64,
    pub fdi_hpme: bool,
    pub vented_h2_kg: f64,
    pub vented_kg_s: f64,
    /// Noble gas (Xe) in the drywell and the wetwell gas, vented to the
    /// stack and leaked to the reactor building, fractions of the core
    /// inventory (gh:#1040).
    pub xe_dw: f64,
    pub xe_ww: f64,
    pub xe_vented: f64,
    pub xe_building: f64,
    /// Cesium (gh:#1047), kg: on the drywell's structures, in the wetwell
    /// pool (deposited on its surface), airborne in both, vented, and
    /// leaked to the building.
    pub cs_dw_structures_kg: f64,
    pub cs_ww_pool_kg: f64,
    pub cs_airborne_kg: f64,
    pub cs_vented_kg: f64,
    pub cs_building_kg: f64,
    /// Iodine likewise, kg.
    pub i_dw_structures_kg: f64,
    pub i_ww_pool_kg: f64,
    pub i_airborne_kg: f64,
    pub i_vented_kg: f64,
    pub i_building_kg: f64,
    /// All the tellurium that has reached the containment, kg (in it,
    /// vented and leaked): for its conservation check.
    pub te_kg: f64,
    /// The cavity (`mcci`): concrete ablated, kg; H2 and CO made there, kg;
    /// the debris temperature, K, and its metallic Zr, kg.
    pub cav_ablated_kg: f64,
    pub cav_h2_kg: f64,
    pub cav_co_kg: f64,
    pub cav_debris_k: f64,
    pub cav_zr_kg: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Containment {
    dw_n2: f64,
    dw_h2: f64,
    dw_steam_kg: f64,
    dw_t: f64,
    dw_wall_t: f64,
    ww_n2: f64,
    ww_h2: f64,
    pool_kg: f64,
    pool_t: f64,
    rb_h2: f64,
    disk_burst: bool,
    flange_open: bool,
    exploded: bool,
    vented_h2_kg: f64,
    /// Water on the drywell floor (pedestal), kg, and its temperature, K.
    pedestal_kg: f64,
    pedestal_t: f64,
    /// The fission products held (noble gas, cesium, iodine, tellurium,
    /// aerosol, what the pool scrubbed, what left): moved into
    /// [`FpHold`] on 2026-11-01 (gh:#1100) with unchanged arithmetic, so
    /// that the control-volume network carries the same physics.
    fp: FpHold,
    /// The wetwell atmosphere's steam, kg, and temperature, K
    /// (`ww_atmosphere`); without it the gas is at the pool's temperature,
    /// saturated, and these are unused.
    ww_steam_kg: f64,
    ww_t: f64,
    /// The torus shell's temperature above the pool, K (`ww_shell`).
    ww_shell_t: f64,
    /// Whether this step carries the wetwell atmosphere (not state: set
    /// from the snapshot at each step).
    ww_atm: bool,
    /// Whether this step carries the drywell structures' surface model (not
    /// state).
    dw_hs: bool,
    /// The debris on the drywell floor (`mcci`).
    cavity: Cavity,
    /// The concrete around the drywell, K, inner face first
    /// (`dw_wall_outer`).
    dw_slab: [f64; DW_SLAB_CELLS],
    /// The pool's water below the downcomers' exits, kg and K, once it is
    /// split off (`pool_layers`); `pool_kg` and `pool_t` are then the water
    /// above them.
    deep_kg: f64,
    deep_t: f64,
    layered: bool,
    /// CO and CO2 from the cavity, mol: tracers inside the nitrogen-like
    /// inventory of each volume (they move in its fractions on every path,
    /// as the noble gas does), and in the building with the drywell gas it
    /// has received, mol (which displaces its air, so its oxygen).
    co: Tracer,
    rb_in: f64,
    burn: Burn,
    /// As in [`ContainmentResult`].
    fdi_steam_kg: f64,
    fdi_hpme: bool,
}

/// The building's burn (`Mechanisms::burn_dynamics`): the time left, s
/// (zero when none is running), the Le Chatelier `Y` it ends at, the
/// oxygen burnt, mol (a deficit that leaves with the building's gas), the
/// running burn's heat, J, and the largest burn's pressure bound, Pa.
///
/// Shared with `containment_network::ContainmentNetwork` (gh:#1100), whose
/// building burns by the same [`bur_step`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Burn {
    pub(crate) left_s: f64,
    pub(crate) y_min: f64,
    pub(crate) o2_used: f64,
    pub(crate) q_j: f64,
    pub(crate) dp_max: f64,
}

impl Burn {
    /// Whether a burn is running.
    pub fn is_burning(&self) -> bool {
        self.left_s > 0.0
    }
    /// The heat of the running (or last) burn so far, J, at
    /// `h2_combustion_j_per_mol` and `co_combustion_j_per_mol` (MELCOR's
    /// heats of combustion as `params` holds them).
    pub fn heat_j(&self) -> f64 {
        self.q_j
    }
    /// The largest burn's adiabatic, constant-volume pressure bound, Pa.
    pub fn dp_max_pa(&self) -> f64 {
        self.dp_max
    }
}

/// One step of MELCOR's BUR burn in a volume of `volume_m3` (BUR RM §2.3 to
/// §2.5) holding `h2` and `co` mol in `n` mol of gas, the diluent
/// fraction `x_d` and oxygen fraction `x_o2`; **moved out of
/// `Containment::step_burn` on 2026-11-01 (gh:#1100), arithmetic
/// unchanged**, so `ContainmentNetwork` burns by the same code. A burn
/// starts when `ignites` and none is running: its completeness (Eqs 2-6 to
/// 2-9) sets the `Y` it ends at, by the completeness's Le Chatelier formula
/// `Y = X_H2 + X_CO XH2CC / XCOCC` (Eq 2-7), and its duration is `CDIM / V`
/// (Eq 2-20). Each step burns at `YRATE = (Y - Ymin) / (t0 + t_comb - t)`
/// (Eq 2-21), the H2 and CO in proportion (Eqs 2-22, 2-23), capped at what
/// is there and at the oxygen. Returns `(H2 burnt, CO burnt, started)`, mol
/// and whether a burn started this step; the caller removes the fuel (the
/// old model) or hands the rates to `tampines`, which closes the heat (the
/// network). `burn.q_j` and `burn.dp_max` are diagnostics: neither model
/// adds `q_j` as heat (the old building is held at one atmosphere; the
/// network's heat is `tampines`').
#[allow(clippy::too_many_arguments)]
pub(crate) fn bur_step(prm: &Params, burn: &mut Burn, ignites: bool, h2: f64, co: f64, n: f64, x_d: f64, x_o2: f64, volume_m3: f64, dt: f64) -> (f64, f64, bool) {
    let ratio = prm.bur_xh2cc / prm.bur_xcocc;
    let y = (h2 + co * ratio) / n;
    let mut started = false;
    if burn.left_s <= 0.0 && ignites {
        burn.y_min = (1.0 - bur_completeness(prm, y)) * y;
        burn.left_s = bur_cdim(volume_m3) / bur_flame_speed(y, x_d);
        burn.q_j = 0.0;
        started = true;
    }
    if burn.left_s <= 0.0 || y <= 0.0 {
        return (0.0, 0.0, started);
    }
    let h = dt.min(burn.left_s);
    let burnt = ((y - burn.y_min) / burn.left_s * h / y).clamp(0.0, 1.0);
    let mut d_h2 = h2 * burnt;
    let mut d_co = co * burnt;
    let o2 = x_o2.max(0.0) * n;
    if 0.5 * (d_h2 + d_co) > o2 {
        let f = o2 / (0.5 * (d_h2 + d_co));
        d_h2 *= f;
        d_co *= f;
    }
    burn.o2_used += 0.5 * (d_h2 + d_co);
    burn.q_j += d_h2 * prm.h2_combustion_j_per_mol + d_co * prm.co_combustion_j_per_mol;
    burn.dp_max = burn.dp_max.max(burn.q_j / (2.5 * volume_m3));
    burn.left_s -= h;
    (d_h2, d_co, started)
}

/// CO and CO2, mol, in the drywell, the wetwell gas and the building.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Tracer {
    dw: [f64; 2],
    ww: [f64; 2],
    rb: [f64; 2],
}

/// The critical (choked) flow of a gas through an orifice, mol/s: MELCOR's
/// sonic flux at the donor's stagnation state (CVH/FL RM §6.3, Eq 6-13),
/// `G = rho c_s (2/(g+1))^((g+1)/(2(g-1)))` with the multiplier at
/// MELCOR's nominal `g = 1.4` (0.5787), times the path's discharge
/// coefficient and area. The sound speed from the donor's own `g`:
/// diatomic gases at `c_v = 2.5 R`, steam at `3.15 R` (stated), by mole
/// fraction (`x_steam`); `molar` its molar mass, kg/mol.
///
/// **Since 2026-11-01 the code is `tampines::flow_path::critical_moles`**
/// (gh:#1094; `sembawang` depends on `tampines`), moved verbatim, so the
/// values are unchanged; this forwards to it.
pub(crate) fn critical_moles(cd: f64, area: f64, p: f64, t: f64, molar: f64, x_steam: f64) -> f64 {
    tampines::flow_path::critical_moles(cd, area, p, t, molar, x_steam)
}

/// MELCOR's BUR ignition test for a volume without igniters (BUR RM §2.2,
/// Eqs 2-3 to 2-5): Le Chatelier's `X_H2 + X_CO L_H2 / L_CO >= L_H2`,
/// oxygen at least `x_o2`, and steam and CO2 below `x_dil`.
pub(crate) fn bur_ignites(prm: &Params, x_h2: f64, x_co: f64, x_o2: f64, x_dil: f64) -> bool {
    x_h2 + x_co * prm.bur_xh2ign / prm.bur_xcoign >= prm.bur_xh2ign && x_o2 >= prm.bur_xo2ig && x_dil < prm.bur_xmscig
}

/// MELCOR's combustion completeness (BUR RM §2.3, the HECTR 1.5
/// correlation, Eqs 2-8 and 2-9): `23.4116 (Ymax - 0.03746)`, none below
/// the threshold, and clipped at 1 (a complete burn: by Eq 2-6, `CC = 1 -
/// Ymin / Ymax`, and `Ymin` cannot be negative).
pub(crate) fn bur_completeness(prm: &Params, y_max: f64) -> f64 {
    (prm.bur_cc_slope * (y_max - prm.bur_cc_y0)).clamp(0.0, 1.0)
}

/// MELCOR's flame speed, m/s (BUR RM §2.4, the HECTR 1.5 correlation,
/// Eqs 2-10 to 2-19, sensitivity coefficients 2200): `Vbase(Ymax)
/// Cdil(XD)`, `XD` the steam and CO2 fraction. Eq 2-18, printed with a
/// misplaced parenthesis, is read as the linear blend of Eq 2-17's and Eq
/// 2-19's `Cdil` across `0.2 < Ymax <= 0.3`.
pub(crate) fn bur_flame_speed(y_max: f64, x_d: f64) -> f64 {
    let v_base = if y_max <= 0.1 {
        59.2 * y_max + 1.792
    } else if y_max <= 0.2 {
        172.88 * y_max - 9.576
    } else if y_max <= 0.3 {
        50.0 * y_max + 15.0
    } else if y_max <= 0.4 {
        -50.0 * y_max + 45.0
    } else if y_max <= 0.6 {
        -75.0 * y_max + 55.0
    } else {
        -64.3 * y_max + 48.58
    };
    let lean = (1.0 - 4.53 * x_d + 5.37 * x_d * x_d).max(0.05);
    let rich = (1.0 - 1.29 * x_d).max(0.0);
    let c_dil = if y_max <= 0.2 {
        lean
    } else if y_max <= 0.3 {
        lean * (0.3 - y_max) / 0.1 + rich * (y_max - 0.2) / 0.1
    } else {
        rich
    };
    v_base * c_dil
}

/// MELCOR's default characteristic dimension of a volume, m (BUR UG,
/// record BUR_BRT, CDIM = DEF): the radius of the sphere of its volume,
/// `(3 V / 4 pi)^(1/3)`.
pub(crate) fn bur_cdim(volume_m3: f64) -> f64 {
    (3.0 * volume_m3 / (4.0 * std::f64::consts::PI)).r_cbrt()
}

/// Cells of the concrete slab around the drywell (`dw_wall_outer`).
pub(crate) const DW_SLAB_CELLS: usize = 10;

/// Saturation temperature at `p`, K (bisection on the IF97 saturation
/// pressure).
fn t_sat(p: f64) -> f64 {
    let (mut a, mut b) = (280.0, 640.0);
    for _ in 0..50 {
        let m = 0.5 * (a + b);
        if p_sat_pa(m) > p {
            b = m;
        } else {
            a = m;
        }
    }
    0.5 * (a + b)
}

fn steam_u(t: f64) -> f64 {
    U_STEAM_REF + CV_STEAM * (t - T_REF)
}

impl Containment {
    pub fn new(prm: &Params) -> Self {
        let dw_n2 = prm.initial_containment_pa * prm.drywell_volume_m3 / (R_GAS * prm.initial_drywell_k);
        let p_n2_ww = prm.initial_containment_pa - p_sat_pa(prm.initial_pool_k);
        let ww_n2 = p_n2_ww * prm.wetwell_gas_m3 / (R_GAS * prm.initial_pool_k);
        Self {
            dw_n2,
            dw_h2: 0.0,
            dw_steam_kg: 0.0,
            dw_t: prm.initial_drywell_k,
            dw_wall_t: prm.initial_drywell_k,
            ww_n2,
            ww_h2: 0.0,
            pool_kg: prm.pool_water_m3 * 995.0,
            pool_t: prm.initial_pool_k,
            rb_h2: 0.0,
            disk_burst: false,
            flange_open: false,
            exploded: false,
            vented_h2_kg: 0.0,
            pedestal_kg: 0.0,
            pedestal_t: prm.initial_drywell_k,
            fp: FpHold::new(prm.drywell_volume_m3, prm.wetwell_gas_m3),
            ww_steam_kg: p_sat_pa(prm.initial_pool_k) * prm.wetwell_gas_m3 * M_H2O / (R_GAS * prm.initial_pool_k),
            ww_t: prm.initial_pool_k,
            ww_shell_t: prm.initial_pool_k,
            ww_atm: false,
            dw_hs: false,
            cavity: Cavity::default(),
            dw_slab: [prm.initial_drywell_k; DW_SLAB_CELLS],
            deep_kg: 0.0,
            deep_t: prm.initial_pool_k,
            layered: false,
            co: Tracer::default(),
            rb_in: 0.0,
            burn: Burn::default(),
            fdi_steam_kg: 0.0,
            fdi_hpme: false,
        }
    }

    /// The building's burn by MELCOR's BUR package (§2.3 to §2.5,
    /// `Mechanisms::burn_dynamics`). A burn starts when `ignites` and none
    /// is running: its completeness (Eqs 2-6 to 2-9) sets the `Y` it ends
    /// at, by the completeness's Le Chatelier formula `Y = X_H2 + X_CO
    /// XH2CC / XCOCC` (Eq 2-7), and its duration is `CDIM / V` (Eq 2-20).
    /// Each step burns at `YRATE = (Y - Ymin) / (t0 + t_comb - t)` (Eq
    /// 2-21), the H2 and CO in proportion (Eqs 2-22, 2-23), capped at what
    /// is there and at the oxygen; the CO2 made joins the building's tracer
    /// and the steam made condenses on the cold building (stated, as for the
    /// drywell's steam). MELCOR gets the pressure rise from its equation of
    /// state; the building here is held at one atmosphere, so the burn's
    /// heat is reported as an adiabatic, constant-volume bound, `Q / (c_v V
    /// / R)` at `c_v = 2.5 R` (air, \[A\]).
    /// `du` kg of UO2 and `dz` of Zr arriving at `s.cav_debris_k` fall
    /// through the pedestal's water (MELCOR FDI RM §2.2, see [`fdi`]); the
    /// heat they give up boils it at `h_g - h_pool` (Eq 2-20), the pool's
    /// temperature unchanged, capped at the pool; returns the temperature
    /// they land at, K.
    fn lpme(&mut self, prm: &Params, s: &ContainmentSnapshot, du: f64, dz: f64, tv: bool) -> f64 {
        let p = self.p_dw(prm);
        let f = lpme_fall(prm, s, du, dz, p, self.pedestal_kg, self.pedestal_t);
        self.fdi_hpme |= f.hpme;
        let (m, t_s, t_bot) = (f.steam_kg, f.t_sat_k, f.t_bottom_k);
        self.pedestal_kg -= m;
        let e = self.dw_energy(tv) + m * (steam_u(t_s) + R_GAS / M_H2O * t_s);
        self.dw_steam_kg += m;
        self.set_dw_energy(e, tv);
        self.fdi_steam_kg += m;
        t_bot
    }

    fn step_burn(&mut self, prm: &Params, ignites: bool, n_rb: f64, x_d: f64, x_o2: f64, dt: f64) {
        let (d_h2, d_co, started) = bur_step(prm, &mut self.burn, ignites, self.rb_h2, self.co.rb[0], n_rb, x_d, x_o2, prm.reactor_building_m3, dt);
        if started {
            self.exploded = true;
        }
        self.rb_h2 -= d_h2;
        self.co.rb[0] -= d_co;
        self.co.rb[1] += d_co;
    }

    /// The building's burn state (`burn_dynamics`; read-only, for comparing
    /// `ContainmentNetwork`'s burns with this model's, gh:#1100).
    pub fn building_burn(&self) -> Burn {
        self.burn
    }

    fn dw_moles(&self) -> f64 {
        self.dw_n2 + self.dw_h2 + self.dw_steam_kg / M_H2O
    }
    pub fn p_dw(&self, prm: &Params) -> f64 {
        self.dw_moles() * R_GAS * self.dw_t / prm.drywell_volume_m3
    }
    pub fn p_ww(&self, prm: &Params) -> f64 {
        if self.ww_atm {
            return self.ww_moles() * R_GAS * self.ww_t / prm.wetwell_gas_m3;
        }
        (self.ww_n2 + self.ww_h2) * R_GAS * self.pool_t / prm.wetwell_gas_m3 + p_sat_pa(self.pool_t)
    }
    /// The drywell structures' convective coefficient, W/(m^2 K), and
    /// condensation flux, kg/(m^2 s), by MELCOR's heat-structure surface
    /// ([`pool_surface::structure_surface`]).
    fn dw_wall_surface(&self, prm: &Params) -> (f64, f64) {
        let atm = pool_surface::Atmosphere { t_k: self.dw_t, p_pa: self.p_dw(prm), n_steam: self.dw_steam_kg / M_H2O, n_n2: self.dw_n2, n_h2: self.dw_h2 };
        pool_surface::structure_surface(&atm, self.dw_wall_t, prm.dw_wall_height_m)
    }
    /// The torus shell above the pool as MELCOR's heat-structure surface
    /// ([`pool_surface::structure_surface`]): steam condenses on it by
    /// diffusion, its latent heat into the shell, the condensate draining
    /// to the pool at the shell's temperature (no film kept), at most 90 %
    /// of the steam in a step; then the shell and the gas exchange heat by
    /// natural convection, implicitly in the two temperatures (as the
    /// drywell's wall). Its outer face is adiabatic (stated: the torus room
    /// is not modelled).
    fn ww_shell_step(&mut self, prm: &Params, tv: bool, dt: f64) {
        let atm = pool_surface::Atmosphere { t_k: self.ww_t, p_pa: self.p_ww(prm), n_steam: self.ww_steam_kg / M_H2O, n_n2: self.ww_n2, n_h2: self.ww_h2 };
        let (h, m_flux) = pool_surface::structure_surface(&atm, self.ww_shell_t, prm.ww_shell_height_m);
        let m = (m_flux * prm.ww_shell_area_m2 * dt).min(0.9 * self.ww_steam_kg);
        if m > 0.0 {
            let h_g = pool_surface::steam_h(self.ww_t);
            let h_l = CP_WATER * (self.ww_shell_t - 273.15);
            let e = self.ww_energy(tv) - m * h_g;
            self.ww_steam_kg -= m;
            self.set_ww_energy(e, tv);
            self.ww_shell_t += m * (h_g - h_l) / prm.ww_shell_c;
            let e_p = self.pool_energy() + m * h_l;
            self.pool_kg += m;
            self.set_pool_energy(e_p);
        }
        let k = prm.ww_shell_area_m2 * h * dt;
        let c_gas = self.ww_heat_capacity(tv);
        let q = k * (self.ww_t - self.ww_shell_t) / (1.0 + k / c_gas + k / prm.ww_shell_c);
        let e = self.ww_energy(tv) - q;
        self.set_ww_energy(e, tv);
        self.ww_shell_t += q / prm.ww_shell_c;
    }
    /// `q` J into the pedestal's water: it heats to saturation at the
    /// drywell's pressure, then boils, the steam into the drywell gas.
    fn boil_pedestal(&mut self, prm: &Params, tv: bool, q: f64) {
        let t_boil = t_sat(self.p_dw(prm));
        let to_boil = ((t_boil - self.pedestal_t) * self.pedestal_kg * CP_WATER).max(0.0);
        if q <= to_boil {
            self.pedestal_t += q / (self.pedestal_kg * CP_WATER);
        } else {
            self.pedestal_t = t_boil;
            let h_fg = 2.257e6;
            let m = ((q - to_boil) / h_fg).min(self.pedestal_kg);
            self.pedestal_kg -= m;
            let e = self.dw_energy(tv) + m * (steam_u(t_boil) + R_GAS / M_H2O * t_boil);
            self.dw_steam_kg += m;
            self.set_dw_energy(e, tv);
        }
    }
    /// The drywell structures' outer heat path (`dw_wall_outer`), explicit
    /// and stable at the plant step (the cells' diffusion time is hours):
    /// from the lumped structures across the gap, by the gap's gas
    /// conduction plus gray radiation between its faces, into a 1-D concrete
    /// slab of `DW_SLAB_CELLS` cells, and from its outer face to the reactor
    /// building's air at `T_RB`; each face resistance includes half a cell.
    fn dw_slab_step(&mut self, prm: &Params, dt: f64) {
        dw_slab_step(prm, &mut self.dw_wall_t, &mut self.dw_slab, dt);
    }
}

/// What debris falling through the pedestal's water does in one step
/// ([`lpme_fall`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LpmeFall {
    /// Pedestal water boiled, kg, at `t_sat_k`, by `dh_j_per_kg` each (`h_g(T_sat)
    /// − h_pool`, so the heat given up is their product).
    pub steam_kg: f64,
    pub t_sat_k: f64,
    pub dh_j_per_kg: f64,
    /// The debris's temperature on landing, K.
    pub t_bottom_k: f64,
    /// The ejection was fast enough for MELCOR's high-pressure model.
    pub hpme: bool,
}

/// `du` kg of UO2 and `dz` of Zr arriving at `s.cav_debris_k` fall through
/// `pool_kg` of water at `pool_t` K spread over the cavity, at drywell
/// pressure `p` (MELCOR FDI RM §2.2, see [`fdi`]): what they boil and the
/// temperature they land at. Moved out of `Containment::lpme` on 2026-11-01
/// (gh:#1100) so that `containment_network` couples the same code; the
/// arithmetic is unchanged (the fixture is byte-identical).
pub(crate) fn lpme_fall(prm: &Params, s: &ContainmentSnapshot, du: f64, dz: f64, p: f64, pool_kg: f64, pool_t: f64) -> LpmeFall {
    let t_s = t_sat(p);
    let rho_f = (du + dz) / (du / prm.uo2_density + dz / prm.zr_density);
    let c_tot = du * prm.uo2_cp + dz * prm.zr_cp;
    let rho_c = sparc::Pool::submerged(t_s, p, 0.0).rho_l();
    let u = fdi::ejection_velocity(prm.discharge_coefficient, s.vessel_p_pa - p, rho_f, prm.lp_bottom_level_m);
    let hpme = u > prm.fdi_hpme_velocity;
    let fall = fdi::Fall {
        rho_f,
        c_f: c_tot / (du + dz),
        sigma_f: prm.fdi_surface_tension,
        rho_c,
        t_c: pool_t,
        t_sat: t_s,
        p_pa: p,
        u,
        d0: prm.breach_diameter_m,
    };
    let depth = pool_kg / (rho_c * prm.cavity_area_m2);
    let t0 = s.cav_debris_k;
    let mut t_bot = fall.bottom_temperature(t0, depth).min(t0);
    let dh = pool_surface::steam_h(t_s) - CP_WATER * (pool_t - 273.15);
    let mut m = c_tot * (t0 - t_bot) / dh;
    if m > pool_kg {
        m = pool_kg;
        t_bot = t0 - m * dh / c_tot;
    }
    LpmeFall { steam_kg: m, t_sat_k: t_s, dh_j_per_kg: dh, t_bottom_k: t_bot, hpme }
}

/// The drywell structures' outer heat path (`dw_wall_outer`; see
/// `Containment::dw_slab_step`) on the structures at `wall_t` K and the
/// slab's cells. Moved out of `Containment` on 2026-11-01 (gh:#1100) so
/// that `containment_network` couples the same code; unchanged arithmetic.
pub(crate) fn dw_slab_step(prm: &Params, wall_t: &mut f64, slab: &mut [f64; DW_SLAB_CELLS], dt: f64) {
    let n = DW_SLAB_CELLS;
    let dx = prm.dw_concrete_m / n as f64;
    let area = prm.dw_wall_area_m2;
    let (tw, t0) = (*wall_t, slab[0]);
    let tm = 0.5 * (tw + t0);
    let h_gap = air_conductivity(tm) / prm.dw_gap_m + 4.0 * 5.670_374e-8 * tm * tm * tm / (2.0 / prm.dw_gap_emissivity - 1.0);
    let half = 0.5 * dx / prm.concrete_k;
    let q_in = area * (tw - t0) / (1.0 / h_gap + half);
    let q_out = area * (slab[n - 1] - T_RB) / (1.0 / prm.rb_wall_h + half);
    let c_cell = prm.concrete_density * prm.concrete_cp * dx * area;
    let g = prm.concrete_k / dx * area;
    let old = *slab;
    for i in 0..n {
        let mut q = 0.0;
        if i == 0 {
            q += q_in;
        } else {
            q += g * (old[i - 1] - old[i]);
        }
        if i == n - 1 {
            q -= q_out;
        } else {
            q -= g * (old[i] - old[i + 1]);
        }
        slab[i] = old[i] + q * dt / c_cell;
    }
    *wall_t -= q_in * dt / prm.dw_wall_c;
}

impl Containment {
    /// Water arriving on the drywell floor, `kg` at `t_k`, mixed into the
    /// pedestal's pool (one pool for the floor and the pedestal, stated).
    fn to_pedestal(&mut self, kg: f64, t_k: f64) {
        let e = self.pedestal_kg * self.pedestal_t + kg * t_k;
        self.pedestal_kg += kg;
        self.pedestal_t = e / self.pedestal_kg;
    }
    fn ww_moles(&self) -> f64 {
        self.ww_n2 + self.ww_h2 + self.ww_steam_kg / M_H2O
    }
    /// The wetwell atmosphere's energy and heat capacity (as the drywell's).
    fn ww_energy(&self, tv: bool) -> f64 {
        ncg_u(self.ww_n2, self.ww_h2, self.ww_t, tv) + self.ww_steam_kg * steam_u(self.ww_t)
    }
    fn ww_heat_capacity(&self, tv: bool) -> f64 {
        ncg_c(self.ww_n2, self.ww_h2, self.ww_t, tv) + self.ww_steam_kg * CV_STEAM
    }
    /// The wetwell atmosphere's temperature from its energy, by Newton (the
    /// energy is monotonic in T).
    fn set_ww_energy(&mut self, e: f64, tv: bool) {
        for _ in 0..50 {
            let dt = (e - self.ww_energy(tv)) / self.ww_heat_capacity(tv);
            self.ww_t += dt;
            if dt.abs() < 1e-9 * self.ww_t {
                break;
            }
        }
    }
    /// Gas entering the pool below its surface (relief valves, downcomers),
    /// `dn2`, `dh2` mol and `m_s` kg of steam carrying `e_in` J, by MELCOR's
    /// bubble model (CVH/FL RM §6.1): with efficiency `epsilon`
    /// ([`pool_surface::bubble_efficiency`]) between no interaction
    /// (everything joins the atmosphere unchanged) and full interaction
    /// (the non-condensables leave at the pool's temperature with vapour at
    /// 0.99 of its saturation pressure; the rest of the steam and the energy
    /// stay in the pool), linearly.
    fn bubble_through(&mut self, prm: &Params, tv: bool, dn2: f64, dh2: f64, m_s: f64, e_in: f64) {
        let p_ww = self.p_ww(prm);
        let rho_l = sparc::Pool::submerged(self.pool_t, p_ww, prm.vent_submergence_pa).rho_l();
        let depth = prm.vent_submergence_pa / (rho_l * 9.81);
        let opening = (4.0 * prm.downcomer_area_m2 / (prm.downcomer_count * std::f64::consts::PI)).sqrt();
        let eps = pool_surface::bubble_efficiency(depth, opening, p_ww + prm.vent_submergence_pa, self.pool_t);
        let p_v = (0.99 * p_sat_pa(self.pool_t)).min(0.99 * p_ww);
        let m_v = (dn2 + dh2) * p_v / (p_ww - p_v) * M_H2O;
        let e_full = ncg_u(dn2, dh2, self.pool_t, tv) + m_v * steam_u(self.pool_t);
        let to_atm_steam = eps * m_v + (1.0 - eps) * m_s;
        let to_atm_e = eps * e_full + (1.0 - eps) * e_in;
        let e_a = self.ww_energy(tv) + to_atm_e;
        self.ww_n2 += dn2;
        self.ww_h2 += dh2;
        self.ww_steam_kg += to_atm_steam;
        self.set_ww_energy(e_a, tv);
        let e_p = self.pool_energy() + (e_in - to_atm_e);
        self.pool_kg += m_s - to_atm_steam;
        self.set_pool_energy(e_p);
    }
    /// The pool surface (MELCOR CVH/FL RM §5.1), flashing of a pool above
    /// the atmosphere's saturation temperature (its vapour leaving at
    /// once, as saturated steam: MELCOR holds bubbles up to a 0.4 void,
    /// not modelled), and rain-out of vapour above saturation in the
    /// atmosphere (its latent heat staying in the gas, as the drywell's).
    fn pool_surface_step(&mut self, prm: &Params, tv: bool, dt: f64) {
        let p_ww = self.p_ww(prm);
        let t_s = pool_surface::t_sat(p_ww);
        if self.pool_t > t_s {
            let h_fg = pool_surface::steam_h(t_s) - CP_WATER * (t_s - 273.15);
            let m = self.pool_kg * CP_WATER * (self.pool_t - t_s) / h_fg;
            let e_a = self.ww_energy(tv) + m * pool_surface::steam_h(t_s);
            self.ww_steam_kg += m;
            self.set_ww_energy(e_a, tv);
            self.pool_kg -= m;
            self.pool_t = t_s;
        }
        let rho = sparc::Pool::submerged(self.pool_t, p_ww, 0.0).rho_l();
        let ex = pool_surface::exchange(
            &pool_surface::Pool { t_k: self.pool_t, kg: self.pool_kg, rho, cp: CP_WATER },
            &pool_surface::Atmosphere { t_k: self.ww_t, p_pa: self.p_ww(prm), n_steam: self.ww_steam_kg / M_H2O, n_n2: self.ww_n2, n_h2: self.ww_h2 },
            &pool_surface::Surface {
                area_m2: prm.ww_pool_surface_m2,
                pool_depth_m: self.pool_kg / rho / prm.ww_pool_surface_m2,
                gas_depth_m: prm.wetwell_gas_m3 / prm.ww_pool_surface_m2,
            },
            dt,
        );
        let m = ex.evaporation_kg_s * dt;
        let h_f = CP_WATER * (self.pool_t - 273.15);
        let h_g = pool_surface::steam_h(self.ww_t);
        let e_p = self.pool_energy() - (m * h_f + ex.q_pool_w * dt);
        let e_a = self.ww_energy(tv) + (m * h_g - ex.q_atm_w * dt);
        self.pool_kg -= m;
        self.set_pool_energy(e_p);
        self.ww_steam_kg += m;
        self.set_ww_energy(e_a, tv);
        for _ in 0..4 {
            let keep = p_sat_pa(self.ww_t.min(640.0)) * prm.wetwell_gas_m3 * M_H2O / (R_GAS * self.ww_t);
            if self.ww_steam_kg <= keep {
                break;
            }
            let cond = self.ww_steam_kg - keep;
            let h_l = CP_WATER * (self.ww_t - 273.15);
            let e_a = self.ww_energy(tv) - cond * h_l;
            self.ww_steam_kg = keep;
            self.set_ww_energy(e_a, tv);
            let e_p = self.pool_energy() + cond * h_l;
            self.pool_kg += cond;
            self.set_pool_energy(e_p);
        }
    }
    fn dw_energy(&self, tv: bool) -> f64 {
        ncg_u(self.dw_n2, self.dw_h2, self.dw_t, tv) + self.dw_steam_kg * steam_u(self.dw_t)
    }
    fn dw_heat_capacity(&self, tv: bool) -> f64 {
        ncg_c(self.dw_n2, self.dw_h2, self.dw_t, tv) + self.dw_steam_kg * CV_STEAM
    }
    /// Drywell gas temperature from its energy. With constant `c_v` it is
    /// linear in T and solved exactly; with `c_v(T)`, by Newton from the
    /// current temperature (the energy is monotonic, `c_v > 0`).
    fn set_dw_energy(&mut self, e: f64, tv: bool) {
        if !tv {
            let c = self.dw_heat_capacity(false);
            let e0 = self.dw_steam_kg * (U_STEAM_REF - CV_STEAM * T_REF);
            self.dw_t = (e - e0) / c;
            return;
        }
        for _ in 0..50 {
            let dt = (e - self.dw_energy(true)) / self.dw_heat_capacity(true);
            self.dw_t += dt;
            if dt.abs() < 1e-9 * self.dw_t {
                break;
            }
        }
    }
    fn pool_energy(&self) -> f64 {
        self.pool_kg * CP_WATER * (self.pool_t - 273.15)
    }
    /// Exchange the two layers of the pool (`pool_layers`), so that what
    /// acts on "the pool" acts on the deep one, and back.
    fn swap_layers(&mut self) {
        std::mem::swap(&mut self.pool_kg, &mut self.deep_kg);
        std::mem::swap(&mut self.pool_t, &mut self.deep_t);
    }
    /// The torus pool in two layers (`pool_layers`): split, once, at the
    /// downcomers' exits (their submergence, `vent_submergence_pa`, as a
    /// depth, over the pool's surface area: a prismatic pool, stated); and
    /// mixed back to one temperature whenever the deep layer is the hotter
    /// (lighter) one, the stratification then being unstable. Between the
    /// layers nothing else passes (stated: conduction and entrainment
    /// neglected). The physics: steam condensing at a vent's exit heats and
    /// stirs the water above the exit while the water below stays cold
    /// (the PPOOLEX experiments; Li and Kudinov's effective heat and
    /// momentum source models of the Mark I torus).
    fn layer_pool(&mut self, prm: &Params) {
        if !self.layered {
            let depth = prm.vent_submergence_pa / (995.0 * 9.81);
            let top = (prm.ww_pool_surface_m2 * depth * 995.0).min(self.pool_kg);
            self.deep_kg = self.pool_kg - top;
            self.deep_t = self.pool_t;
            self.pool_kg = top;
            self.layered = true;
        }
        if self.deep_t > self.pool_t {
            let t = (self.pool_kg * self.pool_t + self.deep_kg * self.deep_t) / (self.pool_kg + self.deep_kg);
            self.pool_t = t;
            self.deep_t = t;
        }
    }
    fn set_pool_energy(&mut self, e: f64) {
        self.pool_t = 273.15 + e / (self.pool_kg * CP_WATER);
    }

    /// Advance one step. Pure function of `self` and `s`.
    pub fn step(&mut self, s: &ContainmentSnapshot) -> ContainmentResult {
        let prm = &s.prm;
        let dt = s.dt_s;
        let tv = s.gas_cv_t;
        self.ww_atm = s.ww_atmosphere;
        if s.pool_layers {
            self.layer_pool(prm);
        }
        self.dw_hs = s.dw_wall_hs;
        self.fp.begin_step(s);
        // 0. Decay heat of the fission products the containment holds
        //    (`fp_decay`): airborne in the drywell, into its gas; on its
        //    structures, into its walls; in the wetwell gas and the pool,
        //    into the pool.
        if let Some(q) = self.fp.decay_heat(prm, s) {
            let e = self.dw_energy(tv) + q.dw_gas_w * dt;
            self.set_dw_energy(e, tv);
            self.dw_wall_t += q.dw_wall_w * dt / prm.dw_wall_c;
            let e = self.pool_energy() + q.pool_w * dt;
            self.set_pool_energy(e);
        }
        // 0b. The RPV's insulation heats the drywell gas (`rpv_wall`).
        if s.rpv_wall_w != 0.0 {
            let e = self.dw_energy(tv) + s.rpv_wall_w * dt;
            self.set_dw_energy(e, tv);
        }
        // 1. Relief valves into the pool. With the wetwell atmosphere, by
        //    the bubble model; the hydrogen with its enthalpy at the
        //    vessel's saturation temperature (`relief_h2_enthalpy`), or at
        //    the pool's temperature without it.
        // With `pool_layers` the relief valves' quenchers, near the pool's
        // bottom, discharge into the deep layer.
        if self.layered {
            self.swap_layers();
        }
        let m = s.srv_steam_kg_s * dt;
        if self.ww_atm {
            let nh2 = s.srv_h2_kg_s * dt / M_H2;
            let e_h2 = if s.relief_h2_enthalpy { nh2 * h2_molar_enthalpy(s.srv_gas_k, tv) } else { ncg_u(0.0, nh2, self.pool_t, tv) };
            let e_in = m * s.srv_h + e_h2;
            self.bubble_through(prm, tv, 0.0, nh2, m, e_in);
        } else {
            let e = self.pool_energy() + m * s.srv_h;
            self.pool_kg += m;
            self.set_pool_energy(e);
            self.ww_h2 += s.srv_h2_kg_s * dt / M_H2;
        }
        if self.layered {
            self.swap_layers();
        }
        // The fission products arriving (`FpHold::arrive`): the relief
        // flow's scrubbed at the quenchers, the leaks' into the drywell.
        let side = PoolSide { t_k: self.pool_t, p_ww_pa: self.p_ww(prm), water_kg: self.pool_kg + self.deep_kg };
        self.fp.arrive(prm, s, side);
        // 2. RPV leak into the drywell.
        let m = s.leak_steam_kg_s * dt;
        let nh = s.leak_h2_kg_s * dt / M_H2;
        let e = self.dw_energy(tv) + m * s.leak_h + nh * h2_molar_enthalpy(s.leak_gas_k, tv);
        self.dw_steam_kg += m;
        self.dw_h2 += nh;
        self.set_dw_energy(e, tv);
        // 2b. Debris on the drywell floor: it heats and boils the water
        //     poured onto it; dry, its heat goes into the floor and walls
        //     (molten-core-concrete interaction is not modelled).
        if s.pedestal_inflow_kg_s > 0.0 {
            let m_in = s.pedestal_inflow_kg_s * dt;
            let e = self.pedestal_kg * self.pedestal_t + m_in * prm.injection_temperature_k;
            self.pedestal_kg += m_in;
            self.pedestal_t = e / self.pedestal_kg;
        }
        if s.vessel_drain_kg_s > 0.0 {
            let m_in = s.vessel_drain_kg_s * dt;
            let e = self.pedestal_kg * self.pedestal_t + m_in * s.vessel_drain_k;
            self.pedestal_kg += m_in;
            self.pedestal_t = e / self.pedestal_kg;
        }
        if s.mcci && s.ex_vessel_w > 0.0 {
            let du = (s.cav_uo2_kg - self.cavity.seen_uo2_kg).max(0.0);
            let dz = (s.cav_zr_kg - self.cavity.seen_zr_kg).max(0.0);
            let t_in = if s.fdi_lpme && self.pedestal_kg > 0.0 && du + dz > 0.0 {
                self.lpme(prm, s, du, dz, tv)
            } else {
                s.cav_debris_k
            };
            self.cavity.arrive(prm, s.cav_uo2_kg, s.cav_zr_kg, t_in);
        }
        if s.ex_vessel_w > 0.0 {
            let q = s.ex_vessel_w * dt;
            if s.mcci {
                // The debris attacks the concrete (`cavity`): its top boils
                // the water over it, or radiates to the structures when dry;
                // the concrete's gases enter the drywell at the ablation
                // temperature, CO and CO2 carried as nitrogen-like
                // non-condensables (stated, until F8's species).
                let water = (self.pedestal_kg > 0.0).then(|| {
                    let p = self.p_dw(prm);
                    (t_sat(p), p)
                });
                let f = self.cavity.step(prm, s.ex_vessel_w, self.dw_wall_t, water, dt, s.cavity_phases);
                if water.is_some() {
                    self.boil_pedestal(prm, tv, f.q_up_w * dt);
                } else {
                    self.dw_wall_t += f.q_up_w * dt / prm.dw_wall_c;
                }
                let t = f.gas_k;
                let n_c = f.co_mol + f.co2_mol;
                let e = self.dw_energy(tv)
                    + f.steam_kg * (steam_u(t) + R_GAS / M_H2O * t)
                    + f.h2_mol * h2_molar_enthalpy(t, tv)
                    + ncg_u(n_c, 0.0, t, tv)
                    + n_c * R_GAS * t;
                self.dw_steam_kg += f.steam_kg;
                self.dw_h2 += f.h2_mol;
                self.dw_n2 += n_c;
                self.co.dw[0] += f.co_mol;
                self.co.dw[1] += f.co2_mol;
                self.set_dw_energy(e, tv);
            } else if self.pedestal_kg > 0.0 {
                self.boil_pedestal(prm, tv, q);
            } else {
                self.dw_wall_t += q / prm.dw_wall_c;
            }
        }
        // 3. Drywell wall heat sink, implicit in the two temperatures. With
        //    `dw_wall_hs`, MELCOR's surface: its coefficient and its
        //    condensation from the start-of-step state; the condensate's
        //    latent heat goes into the wall, the water to the sump (no film
        //    is kept, stated), at most 90 % of the steam in a step (HS
        //    §2.7.3).
        let ha = if self.dw_hs {
            let (h, m_flux) = self.dw_wall_surface(prm);
            let m = (m_flux * prm.dw_wall_area_m2 * dt).min(0.9 * self.dw_steam_kg);
            if m > 0.0 {
                let h_g = pool_surface::steam_h(self.dw_t);
                let e = self.dw_energy(tv) - m * h_g;
                self.dw_steam_kg -= m;
                self.set_dw_energy(e, tv);
                self.dw_wall_t += m * (h_g - CP_WATER * (self.dw_wall_t - 273.15)) / prm.dw_wall_c;
                self.fp.dw_condensed_kg += m;
                if s.condensate_to_floor {
                    self.to_pedestal(m, self.dw_wall_t);
                }
            }
            prm.dw_wall_area_m2 * h
        } else {
            prm.dw_wall_area_m2 * prm.dw_wall_h
        };
        let c_gas = self.dw_heat_capacity(tv);
        let (tg, tw) = (self.dw_t, self.dw_wall_t);
        // q = ha (Tg' - Tw'); c_gas (Tg' - Tg) = -q dt; C_w (Tw' - Tw) = q dt.
        let k = ha * dt;
        let q = k * (tg - tw) / (1.0 + k / c_gas + k / prm.dw_wall_c);
        if tv {
            // c_gas is the slope at Tg: take the heat from the energy, so
            // the gas loses exactly what the wall gains.
            let e = self.dw_energy(tv) - q;
            self.set_dw_energy(e, tv);
        } else {
            self.dw_t = tg - q / c_gas;
        }
        self.dw_wall_t = tw + q / prm.dw_wall_c;
        // 3b. The structures' outer heat path (`dw_wall_outer`).
        if s.dw_wall_outer {
            self.dw_slab_step(prm, dt);
        }
        // 3c. The drywell spray (gh:#1109): its droplets condense steam as
        //     they fall ([`spray_condensation`]), and land on the floor.
        if s.spray_kg_s > 0.0 {
            let (m, t_out) = if s.spray_exchange {
                let p_s = self.dw_steam_kg / M_H2O * R_GAS * self.dw_t / prm.drywell_volume_m3;
                spray_condensation(s.spray_kg_s, s.spray_k, self.dw_t, p_s, self.dw_steam_kg, dt)
            } else {
                (0.0, s.spray_k)
            };
            if m > 0.0 {
                let e = self.dw_energy(tv) - m * dt * pool_surface::steam_h(self.dw_t);
                self.dw_steam_kg -= m * dt;
                self.set_dw_energy(e, tv);
                self.fp.dw_condensed_kg += m * dt;
            }
            self.to_pedestal((s.spray_kg_s + m) * dt, t_out);
        }
        // 4. Condensation of supersaturated steam (latent heat stays in the
        //    gas until the wall takes it): a few fixed-point passes.
        for _ in 0..4 {
            let p_s = self.dw_steam_kg / M_H2O * R_GAS * self.dw_t / prm.drywell_volume_m3;
            let p_sat = p_sat_pa(self.dw_t.min(640.0));
            if p_s <= p_sat {
                break;
            }
            let keep = p_sat * prm.drywell_volume_m3 * M_H2O / (R_GAS * self.dw_t);
            let cond = self.dw_steam_kg - keep;
            self.fp.dw_condensed_kg += cond;
            let e = self.dw_energy(tv) - cond * CP_WATER * (self.dw_t - 273.15);
            if s.condensate_to_floor {
                self.to_pedestal(cond, self.dw_t);
            }
            self.dw_steam_kg = keep;
            self.set_dw_energy(e, tv);
        }
        // 5. Downcomers (drywell -> pool) and vacuum breakers (wetwell -> drywell).
        let (p_dw, p_ww) = (self.p_dw(prm), self.p_ww(prm));
        let push = p_dw - p_ww - prm.vent_submergence_pa;
        if push > 0.0 {
            let n_dw = self.dw_moles();
            let molar_mass = (self.dw_n2 * M_N2 + self.dw_h2 * M_H2 + self.dw_steam_kg) / n_dw;
            let rho = p_dw * molar_mass / (R_GAS * self.dw_t);
            let orifice = prm.discharge_coefficient * prm.downcomer_area_m2 * (2.0 * rho * push).sqrt() * dt / molar_mass;
            // Equalising amount: drywell pressure down by `push` (isothermal);
            // with `two_sided_equalising` the wetwell's rise from the
            // non-condensables it receives is counted too (the steam
            // condensing in the pool, stated).
            let n_eq = if s.two_sided_equalising && self.ww_atm {
                let x_nc = (self.dw_n2 + self.dw_h2) / n_dw;
                push / (p_dw / n_dw + x_nc * p_ww / self.ww_moles())
            } else {
                n_dw * push / p_dw
            };
            let n = orifice.min(n_eq);
            let f = n / n_dw;
            let (dn2, dh2, dsteam) = (self.dw_n2 * f, self.dw_h2 * f, self.dw_steam_kg * f);
            for i in 0..2 {
                let d = self.co.dw[i] * f;
                self.co.dw[i] -= d;
                self.co.ww[i] += d;
            }
            // Its fission products, scrubbed by the pool (`FpHold::downcomer`).
            let side = PoolSide { t_k: self.pool_t, p_ww_pa: p_ww, water_kg: self.pool_kg + self.deep_kg };
            let flow = DowncomerFlow { f, n, dn2, dh2, dsteam_kg: dsteam, t_dw_k: self.dw_t, p_dw_pa: p_dw };
            self.fp.downcomer(prm, s, flow, side);
            let e_dw = self.dw_energy(tv);
            let t_dw = self.dw_t;
            self.dw_n2 -= dn2;
            self.dw_h2 -= dh2;
            self.dw_steam_kg -= dsteam;
            // The drywell gas keeps its temperature; the moved gas's energy
            // goes to the pool (steam condenses, N2/H2 cool to the pool).
            // With `outflow_enthalpy` it takes its flow work `n R T` too
            // (MELCOR's donor loses the enthalpy it moves, CVH/FL RM Eq
            // 6-6), and the drywell cools by it.
            let mut e_moved = e_dw - self.dw_energy(tv);
            if s.outflow_enthalpy {
                e_moved += n * R_GAS * t_dw;
                self.set_dw_energy(e_dw - e_moved, tv);
            }
            if self.ww_atm {
                self.bubble_through(prm, tv, dn2, dh2, dsteam, e_moved);
            } else {
                let e_pool = self.pool_energy() + e_moved - ncg_u(dn2, dh2, self.pool_t, tv);
                self.pool_kg += dsteam;
                self.set_pool_energy(e_pool);
                self.ww_n2 += dn2;
                self.ww_h2 += dh2;
            }
        }
        let (p_dw, p_ww) = (self.p_dw(prm), self.p_ww(prm));
        let suck = p_ww - p_dw - prm.vacuum_breaker_pa;
        if suck > 0.0 && self.ww_atm {
            // The atmosphere's mixture, steam with it, at its temperature;
            // it leaves with its enthalpy.
            let n_ww = self.ww_moles();
            let molar = (self.ww_n2 * M_N2 + self.ww_h2 * M_H2 + self.ww_steam_kg) / n_ww;
            let rho = p_ww * molar / (R_GAS * self.ww_t);
            let orifice = prm.discharge_coefficient * prm.vacuum_breaker_m2 * (2.0 * rho * suck).sqrt() * dt / molar;
            // Equalising: the wetwell down by `f p_ww`, and with
            // `two_sided_equalising` the drywell up by `f n_ww p_dw / n_dw`.
            let f_eq = if s.two_sided_equalising { suck / (p_ww + n_ww * p_dw / self.dw_moles()) } else { suck / p_ww };
            let f = (orifice / n_ww).min(f_eq);
            let (dn2, dh2, dsteam) = (self.ww_n2 * f, self.ww_h2 * f, self.ww_steam_kg * f);
            let e_out = f * (self.ww_energy(tv) + n_ww * R_GAS * self.ww_t);
            for i in 0..2 {
                let d = self.co.ww[i] * f;
                self.co.ww[i] -= d;
                self.co.dw[i] += d;
            }
            self.fp.ww_to_dw(f);
            let e_a = self.ww_energy(tv) - e_out;
            self.ww_n2 -= dn2;
            self.ww_h2 -= dh2;
            self.ww_steam_kg -= dsteam;
            self.set_ww_energy(e_a, tv);
            let e = self.dw_energy(tv) + e_out;
            self.dw_n2 += dn2;
            self.dw_h2 += dh2;
            self.dw_steam_kg += dsteam;
            self.set_dw_energy(e, tv);
        } else if suck > 0.0 {
            let n_ww = self.ww_n2 + self.ww_h2;
            let molar = (self.ww_n2 * M_N2 + self.ww_h2 * M_H2) / n_ww;
            let rho = p_ww * molar / (R_GAS * self.pool_t);
            let orifice = prm.discharge_coefficient * prm.vacuum_breaker_m2 * (2.0 * rho * suck).sqrt() * dt / molar;
            // At most the isothermal equalising amount of the non-condensables
            // (with `two_sided_equalising`, the drywell's rise counted).
            let n_eq = if s.two_sided_equalising && n_ww > 0.0 {
                let p_nc = (p_ww - p_sat_pa(self.pool_t).min(p_ww)).max(1.0);
                suck / (p_nc / n_ww + p_dw / self.dw_moles())
            } else {
                n_ww * suck / p_ww
            };
            let n = orifice.min(n_eq);
            // An empty wetwell gas moves nothing (0/0 otherwise; gh:#1047).
            let f = if n_ww > 0.0 { (n / n_ww).min(1.0) } else { 0.0 };
            let (dn2, dh2) = (self.ww_n2 * f, self.ww_h2 * f);
            for i in 0..2 {
                let d = self.co.ww[i] * f;
                self.co.ww[i] -= d;
                self.co.dw[i] += d;
            }
            self.fp.ww_to_dw(f);
            self.ww_n2 -= dn2;
            self.ww_h2 -= dh2;
            let e = self.dw_energy(tv) + ncg_u(dn2, dh2, self.pool_t, tv);
            self.dw_n2 += dn2;
            self.dw_h2 += dh2;
            self.set_dw_energy(e, tv);
        }
        // 5c. The pool surface (`ww_atmosphere`), and the torus shell
        //     above it (`ww_shell`).
        if self.ww_atm {
            self.pool_surface_step(prm, tv, dt);
            if s.ww_shell {
                self.ww_shell_step(prm, tv, dt);
            }
        }
        // 6. Wetwell vent through the rupture disk.
        let p_ww = self.p_ww(prm);
        if s.vent_ordered && p_ww >= prm.vent_rupture_disk_pa {
            self.disk_burst = true;
        }
        let mut vented_kg_s = 0.0;
        let vent_flowing = s.vent_ordered && self.disk_burst && p_ww > P_ATM;
        if vent_flowing && self.ww_atm {
            // The atmosphere as it is, through the orifice at its density,
            // at most the equalising amount; it leaves with its enthalpy,
            // and the pool gives steam only through its surface (and by
            // flashing) at the next step.
            let n_all = self.ww_moles();
            let molar = (self.ww_n2 * M_N2 + self.ww_h2 * M_H2 + self.ww_steam_kg) / n_all;
            let rho = p_ww * molar / (R_GAS * self.ww_t);
            let mut n_flow = prm.discharge_coefficient * prm.vent_area_m2 * (2.0 * rho * (p_ww - P_ATM)).sqrt() * dt / molar;
            if s.gas_choking {
                let x_s = self.ww_steam_kg / M_H2O / n_all;
                n_flow = n_flow.min(critical_moles(prm.discharge_coefficient, prm.vent_area_m2, p_ww, self.ww_t, molar, x_s) * dt);
            }
            let f = (n_flow / n_all).min((p_ww - P_ATM) / p_ww);
            let dh2 = self.ww_h2 * f;
            for i in 0..2 {
                self.co.ww[i] -= self.co.ww[i] * f;
            }
            self.fp.vent(f, f);
            self.vented_h2_kg += dh2 * M_H2;
            vented_kg_s = f * (self.ww_n2 * M_N2 + self.ww_h2 * M_H2 + self.ww_steam_kg) / dt;
            let e_a = self.ww_energy(tv) - f * (self.ww_energy(tv) + n_all * R_GAS * self.ww_t);
            self.ww_n2 -= self.ww_n2 * f;
            self.ww_h2 -= dh2;
            self.ww_steam_kg -= self.ww_steam_kg * f;
            self.set_ww_energy(e_a, tv);
        } else if vent_flowing && s.mixture_vent {
            // The vent passes the wetwell gas as it is, steam and
            // non-condensables, through its orifice at the mixture's
            // density, split by mole fraction; the pool boils to supply the
            // steam (gh:#1053). Before, the orifice was sized for the
            // non-condensables alone and steam added at x_s / (1 - x_s)
            // times that: about 20 times the orifice's capacity once the
            // wetwell is mostly steam, and nothing at all once the
            // non-condensables were gone.
            let n_g = self.ww_n2 + self.ww_h2;
            let v = prm.wetwell_gas_m3;
            let p_s = p_sat_pa(self.pool_t).min(p_ww);
            let x_s = p_s / p_ww;
            let n_sv = p_s * v / (R_GAS * self.pool_t);
            let molar = (self.ww_n2 * M_N2 + self.ww_h2 * M_H2 + n_sv * M_H2O) / (n_g + n_sv);
            let rho = p_ww * molar / (R_GAS * self.pool_t);
            let mut n_flow = prm.discharge_coefficient * prm.vent_area_m2 * (2.0 * rho * (p_ww - P_ATM)).sqrt() * dt / molar;
            if s.gas_choking {
                n_flow = n_flow.min(critical_moles(prm.discharge_coefficient, prm.vent_area_m2, p_ww, self.pool_t, molar, x_s) * dt);
            }
            // Non-condensables: their share, at most the equalising amount.
            let p_nc = p_ww - p_s;
            let n_nc = if n_g > 0.0 { (n_flow * (1.0 - x_s)).min(n_g * ((p_ww - P_ATM) / p_nc).min(1.0)) } else { 0.0 };
            let f = if n_g > 0.0 { n_nc / n_g } else { 0.0 };
            // Steam: its share, at most what boiling can give before the
            // pool's saturation pressure plus the remaining non-condensables
            // fall to the atmosphere's.
            let p_floor = (P_ATM - p_nc * (1.0 - f)).max(1.0e3);
            let h_fg = 2.257e6;
            let m_req = n_flow * x_s * M_H2O;
            // The cap binds only if the pool, having given `m_req`, would
            // be below the floor: one saturation pressure decides it, the
            // inverse (`t_sat`, a bisection) only when it binds.
            let t_after = self.pool_t - m_req * h_fg / (self.pool_kg * CP_WATER);
            let m_s = if p_sat_pa(t_after.max(280.0)) >= p_floor {
                m_req
            } else {
                m_req.min((self.pool_kg * CP_WATER * (self.pool_t - t_sat(p_floor)) / h_fg).max(0.0))
            };
            // The gas volume that left, for what rides in it.
            let f_gas = ((n_nc + m_s / M_H2O) / (n_g + n_sv)).min(1.0);
            let dh2 = self.ww_h2 * f;
            for i in 0..2 {
                self.co.ww[i] -= self.co.ww[i] * f;
            }
            self.fp.vent(f, f_gas);
            self.vented_h2_kg += dh2 * M_H2;
            vented_kg_s = (self.ww_n2 * f * M_N2 + dh2 * M_H2 + m_s) / dt;
            self.ww_n2 -= self.ww_n2 * f;
            self.ww_h2 -= dh2;
            // The boiled-off water leaves with its sensible heat as well as
            // its latent heat (before, only the latent heat left the pool).
            let e = self.pool_energy() - m_s * (CP_WATER * (self.pool_t - 273.15) + h_fg);
            self.pool_kg -= m_s;
            self.set_pool_energy(e);
        } else if vent_flowing {
            let n_g = self.ww_n2 + self.ww_h2;
            let p_nc = n_g * R_GAS * self.pool_t / prm.wetwell_gas_m3;
            let molar = (self.ww_n2 * M_N2 + self.ww_h2 * M_H2) / n_g.max(1e-30);
            let rho = p_ww * molar / (R_GAS * self.pool_t);
            let mut orifice = prm.discharge_coefficient * prm.vent_area_m2 * (2.0 * rho * (p_ww - P_ATM)).sqrt() * dt / molar;
            if s.gas_choking && n_g > 0.0 {
                orifice = orifice.min(critical_moles(prm.discharge_coefficient, prm.vent_area_m2, p_ww, self.pool_t, molar, 0.0) * dt);
            }
            // Equalising amount of non-condensables, the steam partial
            // pressure being held by the pool.
            let n = orifice.min(n_g * ((p_ww - P_ATM) / p_nc).min(1.0));
            // Once the non-condensables are all vented this was 0/0, and
            // the NaN took the wetwell's pressure with it: found 2026-10-29
            // at 73.5 h of the 500 h run, after which no downcomer flowed.
            let f = if n_g > 0.0 { n / n_g } else { 0.0 };
            let dh2 = self.ww_h2 * f;
            for i in 0..2 {
                self.co.ww[i] -= self.co.ww[i] * f;
            }
            self.fp.vent(f, f);
            self.vented_h2_kg += dh2 * M_H2;
            vented_kg_s = (self.ww_n2 * f * M_N2 + dh2 * M_H2) / dt;
            self.ww_n2 -= self.ww_n2 * f;
            self.ww_h2 -= dh2;
            // The steam that leaves with it boils off the pool.
            let x_s = p_sat_pa(self.pool_t) / p_ww;
            let steam_mol = n * x_s / (1.0 - x_s).max(1e-6);
            let m_s = (steam_mol * M_H2O).min(self.pool_kg * 1e-3);
            let h_fg = 2.257e6;
            let e = self.pool_energy() - m_s * h_fg;
            self.pool_kg -= m_s;
            self.set_pool_energy(e);
            vented_kg_s += m_s / dt;
        }
        // 7. Drywell head-flange leak to the reactor building.
        let p_dw = self.p_dw(prm);
        if p_dw >= prm.flange_leak_pa {
            self.flange_open = true;
        }
        if self.flange_open && p_dw > P_ATM {
            let n_dw = self.dw_moles();
            let molar = (self.dw_n2 * M_N2 + self.dw_h2 * M_H2 + self.dw_steam_kg) / n_dw;
            let rho = p_dw * molar / (R_GAS * self.dw_t);
            let mut orifice = prm.discharge_coefficient * prm.flange_leak_m2 * (2.0 * rho * (p_dw - P_ATM)).sqrt() * dt / molar;
            if s.gas_choking {
                let x_s = self.dw_steam_kg / M_H2O / n_dw;
                orifice = orifice.min(critical_moles(prm.discharge_coefficient, prm.flange_leak_m2, p_dw, self.dw_t, molar, x_s) * dt);
            }
            let n = orifice.min(n_dw * (p_dw - P_ATM) / p_dw);
            let f = n / n_dw;
            let dh2 = self.dw_h2 * f;
            let dn2 = self.dw_n2 * f;
            let dco = [self.co.dw[0] * f, self.co.dw[1] * f];
            self.co.dw[0] -= dco[0];
            self.co.dw[1] -= dco[1];
            self.fp.flange(f);
            self.dw_n2 -= dn2;
            self.dw_h2 -= dh2;
            self.dw_steam_kg -= self.dw_steam_kg * f;
            if s.outflow_enthalpy {
                let e = self.dw_energy(tv) - n * R_GAS * self.dw_t;
                self.set_dw_energy(e, tv);
            }
            // The building is well mixed at atmospheric pressure: incoming
            // non-condensables push out building air at its mixture fraction
            // (the steam condenses on the cold building).
            // (A building that has burnt stops taking gas without
            // `burn_dynamics`, where the explosion wrecks it; MELCOR's
            // building goes on.)
            if !self.exploded || s.burn_dynamics {
                let n_rb = P_ATM * prm.reactor_building_m3 / (R_GAS * T_RB);
                self.burn.o2_used -= self.burn.o2_used / n_rb * (dh2 + dn2);
                let y = self.rb_h2 / n_rb;
                self.rb_h2 += dh2 - y * (dh2 + dn2);
                for i in 0..2 {
                    let y_i = self.co.rb[i] / n_rb;
                    self.co.rb[i] += dco[i] - y_i * (dh2 + dn2);
                }
                let y_in = self.rb_in / n_rb;
                self.rb_in += (dh2 + dn2) * (1.0 - y_in);
            }
        }
        // 8. Fission products and aerosol: condensation, agglomeration and
        //    deposition, every 10 s of accumulated time (transport above
        //    moves them every step).
        if let Some(h) = self.fp.rn_due(dt) {
            self.step_rn(prm, h, s.phoresis);
        }
        let n_rb = P_ATM * prm.reactor_building_m3 / (R_GAS * T_RB);
        let y_rb = self.rb_h2 / n_rb;
        // The building's gas: its hydrogen, CO and CO2 fractions, and its
        // oxygen, air's 0.2095 less what the drywell gas has displaced
        // (its steam condensed on the cold building, stated).
        let x_co = self.co.rb[0] / n_rb;
        let x_co2 = self.co.rb[1] / n_rb;
        let x_o2 = 0.2095 * (1.0 - self.rb_in / n_rb).max(0.0) - self.burn.o2_used / n_rb;
        let ignites = if s.bur_ignition { bur_ignites(prm, y_rb, x_co, x_o2, x_co2) } else { y_rb >= prm.h2_explosion };
        let rb_burn_y = y_rb + x_co * prm.bur_xh2ign / prm.bur_xcoign;
        if s.burn_dynamics {
            self.step_burn(prm, ignites, n_rb, x_co2, x_o2, dt);
        } else if !self.exploded && ignites {
            self.exploded = true;
            self.rb_h2 = 0.0;
            self.co.rb[0] = 0.0;
        }
        if s.pool_layers {
            self.layer_pool(prm);
        }
        let fp = self.fp.report();
        ContainmentResult {
            p_dw_pa: self.p_dw(prm),
            p_ww_pa: self.p_ww(prm),
            t_dw_k: self.dw_t,
            t_pool_k: if self.layered { (self.pool_kg * self.pool_t + self.deep_kg * self.deep_t) / (self.pool_kg + self.deep_kg) } else { self.pool_t },
            t_pool_top_k: self.pool_t,
            dw_h2_kg: self.dw_h2 * M_H2,
            ww_h2_kg: self.ww_h2 * M_H2,
            rb_h2_fraction: self.rb_h2 / n_rb,
            vent_flowing,
            flange_leaking: self.flange_open,
            rb_flammable: self.rb_h2 / n_rb >= prm.h2_flammable,
            rb_exploded: self.exploded,
            rb_burn_y,
            rb_co_fraction: x_co,
            rb_burning: self.burn.left_s > 0.0,
            rb_burn_dp_pa: self.burn.dp_max,
            fdi_steam_kg: self.fdi_steam_kg,
            fdi_hpme: self.fdi_hpme,
            vented_h2_kg: self.vented_h2_kg,
            vented_kg_s,
            xe_dw: fp.xe_dw,
            xe_ww: fp.xe_ww,
            xe_vented: fp.xe_vented,
            xe_building: fp.xe_building,
            cs_dw_structures_kg: fp.cs_dw_structures_kg,
            cs_ww_pool_kg: fp.cs_ww_pool_kg,
            cs_airborne_kg: fp.cs_airborne_kg,
            cs_vented_kg: fp.cs_vented_kg,
            cs_building_kg: fp.cs_building_kg,
            i_dw_structures_kg: fp.i_dw_structures_kg,
            i_ww_pool_kg: fp.i_ww_pool_kg,
            i_airborne_kg: fp.i_airborne_kg,
            i_vented_kg: fp.i_vented_kg,
            i_building_kg: fp.i_building_kg,
            te_kg: fp.te_kg,
            cav_ablated_kg: self.cavity.ablated_kg,
            cav_h2_kg: self.cavity.h2_kg,
            cav_co_kg: self.cavity.co_kg,
            cav_debris_k: self.cavity.t_k,
            cav_zr_kg: self.cavity.zr_kg,
        }
    }
}

/// The non-condensable share of a gas's moles (H2, N2, steam), the share of
/// its volume that survives the steam's condensation in the pool (SPARC-90's
/// DF_EC is its inverse). No gas, nothing scrubbed.
/// The drywell spray in equilibrium with the atmosphere it falls through
/// (gh:#1109): the droplets, `w` kg/s at `t_in` K, heat to the
/// atmosphere's dew point (the saturation temperature at the steam's
/// partial pressure `p_steam`, Pa), at most the gas's `t_gas`, by
/// condensing steam on themselves. Returns the steam condensed, kg/s, and
/// the temperature of the water (spray and condensate) reaching the floor,
/// K. Energy: `m h_g(t_gas) = w c_p (T - t_in) + m c_p (T - 273.15)`, the
/// condensate leaving at `T`, so the atmosphere loses `m h_g` and the floor
/// gains it; at most 90 % of the steam `steam_kg` in a step of `dt` s (the
/// structures' limit), the temperature then from the balance.
///
/// **The equilibrium limit (stated, \[A\]):** MELCOR's SPR package follows a
/// droplet's heat and mass transfer as it falls (SPR RM, cited by package
/// only, not held); a drop that falls tens of metres reaches the
/// atmosphere's temperature, so equilibrium is the bound on the spray's
/// effect. The gas's own sensible heat to the droplets is not counted (in a
/// steam-rich drywell the latent heat dominates), nor is evaporation from
/// droplets hotter than the dew point.
pub fn spray_condensation(w: f64, t_in: f64, t_gas: f64, p_steam: f64, steam_kg: f64, dt: f64) -> (f64, f64) {
    if !(w > 0.0 && p_steam > 0.0 && steam_kg > 0.0) {
        return (0.0, t_in);
    }
    let t_eq = pool_surface::t_sat(p_steam).min(t_gas).max(t_in);
    let h_g = pool_surface::steam_h(t_gas);
    let m = w * CP_WATER * (t_eq - t_in) / (h_g - CP_WATER * (t_eq - 273.15));
    let cap = 0.9 * steam_kg / dt;
    if m <= cap {
        return (m.max(0.0), t_eq);
    }
    let t = (w * CP_WATER * t_in + cap * (h_g + CP_WATER * 273.15)) / (CP_WATER * (w + cap));
    (cap, t)
}

pub(crate) fn nc_share(h2_mol: f64, n2_mol: f64, steam_mol: f64) -> f64 {
    let total = h2_mol + n2_mol + steam_mol;
    if total > 0.0 { (h2_mol + n2_mol) / total } else { 1.0 }
}

/// The share of a gas's volume that passes the condensation at a vent
/// exit: `1 / DF_EC = X_i / X_o` (Eqs D-5, D-6), `X_i` the gas's
/// non-condensable mole fraction and `X_o` the fraction once it is in
/// equilibrium with the pool. At most 1 (DF_EC is at least 1).
pub(crate) fn condensation_pass(x_i: f64, x_o: f64) -> f64 {
    if x_i >= x_o { 1.0 } else { x_i / x_o }
}

/// Aerosol step, s (the transport steps every plant step).
pub(crate) const RN_STEP_S: f64 = 10.0;

/// MELCOR's default aerosol size range, 0.1 to 50 um (RN1_ASP), as 27
/// sections of particle-volume ratio 2 (0.1 to 40 um, the fallout section
/// at 50 um).
pub(crate) fn rn_sections() -> Sections {
    Sections::new(0.1e-6, 27, 2.0)
}

/// Air's thermal conductivity, W/(m K), by Sutherland's law with White's
/// constants for air (k0 = 0.0241 W/(m K) at T0 = 273 K, S = 194 K; F. M.
/// White, *Viscous Fluid Flow*, 3rd ed., Table 1-3). MELCOR evaluates
/// thermophoresis with air's conductivity at the surface (RN §2.4.2.2).
pub(crate) fn air_conductivity(t_k: f64) -> f64 {
    let r = t_k / 273.0;
    0.0241 * r * r.sqrt() * (273.0 + 194.0) / (t_k + 194.0)
}

/// Air's viscosity, Pa s, by Sutherland's law (1.458e-6 T^1.5/(T + 110.4),
/// the standard constants): MELCOR evaluates its aerosol coefficients with
/// air's properties (Vol. 2, RN §2.4.2.3, constraint c).
pub(crate) fn air_viscosity(t_k: f64) -> f64 {
    1.458e-6 * t_k * t_k.sqrt() / (t_k + 110.4)
}

impl Containment {
    /// The fission-product and aerosol step of both volumes (gh:#1047):
    /// since gh:#1100 [`FpHold::step_rn`], given this model's drywell gas,
    /// structures and wetwell gas (arithmetic unchanged).
    fn step_rn(&mut self, prm: &Params, dt: f64, phoresis: bool) {
        let h_wall = if self.dw_hs { self.dw_wall_surface(prm).0 } else { prm.dw_wall_h };
        let p_ww = self.p_ww(prm);
        let (x_s, t_gas) = if self.ww_atm { (self.ww_steam_kg / M_H2O / self.ww_moles(), self.ww_t) } else { ((p_sat_pa(self.pool_t) / p_ww).min(1.0), self.pool_t) };
        let c = RnContext {
            dw_steam_kg: self.dw_steam_kg,
            dw_n2: self.dw_n2,
            dw_h2: self.dw_h2,
            dw_t_k: self.dw_t,
            p_dw_pa: self.p_dw(prm),
            dw_gas_m3: prm.drywell_volume_m3,
            h_wall,
            dw_wall_t_k: self.dw_wall_t,
            pool: PoolSide { t_k: self.pool_t, p_ww_pa: p_ww, water_kg: self.pool_kg + self.deep_kg },
            ww_x_steam: x_s,
            ww_t_k: t_gas,
            ww_n2: self.ww_n2,
            ww_h2: self.ww_h2,
            pool_area_m2: prm.ww_pool_surface_m2,
        };
        self.fp.step_rn(prm, &c, dt, phoresis);
    }
}

impl Default for Containment {
    /// The containment with the default inputs ([`Params::default`]).
    fn default() -> Self {
        Self::new(&Params::default())
    }
}

fn b(x: bool) -> f64 {
    if x { 1.0 } else { 0.0 }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // used by the browser's solver workers (and the tests)
impl Containment {
    /// The whole state, for a stateless worker to step.
    pub fn write_state(&self, w: &mut Writer) {
        w.f(&[
            self.dw_n2,
            self.dw_h2,
            self.dw_steam_kg,
            self.dw_t,
            self.dw_wall_t,
            self.ww_n2,
            self.ww_h2,
            self.pool_kg,
            self.pool_t,
            self.rb_h2,
            b(self.disk_burst),
            b(self.flange_open),
            b(self.exploded),
            self.vented_h2_kg,
            self.pedestal_kg,
            self.pedestal_t,
            self.fp.xe_dw,
            self.fp.xe_ww,
            self.fp.xe_vented,
            self.fp.xe_building,
            self.fp.rn_clock_s,
            self.fp.cs_vented_kg,
            self.fp.cs_building_kg,
            self.fp.i_vented_kg,
            self.fp.i_building_kg,
            self.fp.te_out_kg,
            self.fp.swarm_age_s,
            self.fp.iodine_h,
            self.fp.dw_condensed_kg,
            self.fp.iodine_swarm_pass,
            self.fp.iodine_swarm_age_s,
            self.ww_steam_kg,
            self.ww_t,
            self.ww_shell_t,
            self.cavity.uo2_kg,
            self.cavity.zr_kg,
            self.cavity.oxide_kg,
            self.cavity.t_k,
            self.cavity.seen_uo2_kg,
            self.cavity.seen_zr_kg,
            self.cavity.ablated_kg,
            self.cavity.h2_kg,
            self.cavity.co_kg,
            self.cavity.zro2_kg,
        ]);
        w.f(&self.dw_slab);
        w.f(&[self.deep_kg, self.deep_t, b(self.layered)]);
        w.f(&[self.co.dw[0], self.co.dw[1], self.co.ww[0], self.co.ww[1], self.co.rb[0], self.co.rb[1], self.rb_in]);
        w.f(&[self.burn.left_s, self.burn.y_min, self.burn.o2_used, self.burn.q_j, self.burn.dp_max]);
        w.f(&[self.fdi_steam_kg, b(self.fdi_hpme)]);
        w.f(&self.fp.swarm_pass);
        w.f(&self.fp.pool_scrubbed);
        self.fp.dw_rn.write(w);
        self.fp.ww_rn.write(w);
    }
    pub fn read_state(r: &mut Reader) -> Self {
        let mut c = Self {
            dw_n2: r.f(),
            dw_h2: r.f(),
            dw_steam_kg: r.f(),
            dw_t: r.f(),
            dw_wall_t: r.f(),
            ww_n2: r.f(),
            ww_h2: r.f(),
            pool_kg: r.f(),
            pool_t: r.f(),
            rb_h2: r.f(),
            disk_burst: r.f() != 0.0,
            flange_open: r.f() != 0.0,
            exploded: r.f() != 0.0,
            vented_h2_kg: r.f(),
            pedestal_kg: r.f(),
            pedestal_t: r.f(),
            // The fission products' scalars, in their wire order (the
            // fields' former order in `Containment`).
            fp: {
                let mut fp = FpHold::new(0.0, 0.0);
                fp.xe_dw = r.f();
                fp.xe_ww = r.f();
                fp.xe_vented = r.f();
                fp.xe_building = r.f();
                fp.rn_clock_s = r.f();
                fp.cs_vented_kg = r.f();
                fp.cs_building_kg = r.f();
                fp.i_vented_kg = r.f();
                fp.i_building_kg = r.f();
                fp.te_out_kg = r.f();
                fp.swarm_pass = vec![];
                fp.swarm_age_s = r.f();
                fp.iodine_h = r.f();
                fp.dw_condensed_kg = r.f();
                fp.iodine_swarm_pass = r.f();
                fp.iodine_swarm_age_s = r.f();
                fp
            },
            ww_steam_kg: r.f(),
            ww_t: r.f(),
            ww_shell_t: r.f(),
            ww_atm: false,
            dw_hs: false,
            cavity: Cavity {
                uo2_kg: r.f(),
                zr_kg: r.f(),
                oxide_kg: r.f(),
                t_k: r.f(),
                seen_uo2_kg: r.f(),
                seen_zr_kg: r.f(),
                ablated_kg: r.f(),
                h2_kg: r.f(),
                co_kg: r.f(),
                zro2_kg: r.f(),
            },
            dw_slab: [0.0; DW_SLAB_CELLS],
            deep_kg: 0.0,
            deep_t: 0.0,
            layered: false,
            co: Tracer::default(),
            rb_in: 0.0,
            burn: Burn::default(),
            fdi_steam_kg: 0.0,
            fdi_hpme: false,
        };
        c.dw_slab = std::array::from_fn(|_| r.f());
        c.deep_kg = r.f();
        c.deep_t = r.f();
        c.layered = r.f() != 0.0;
        c.co = Tracer { dw: [r.f(), r.f()], ww: [r.f(), r.f()], rb: [r.f(), r.f()] };
        c.rb_in = r.f();
        c.burn = Burn { left_s: r.f(), y_min: r.f(), o2_used: r.f(), q_j: r.f(), dp_max: r.f() };
        c.fdi_steam_kg = r.f();
        c.fdi_hpme = r.f() != 0.0;
        c.fp.swarm_pass = (0..rn_sections().v.len()).map(|_| r.f()).collect();
        c.fp.pool_scrubbed = std::array::from_fn(|_| r.f());
        c.fp.dw_rn.read_into(r);
        c.fp.ww_rn.read_into(r);
        c
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // used by the browser's solver workers (and the tests)
impl ContainmentSnapshot {
    pub fn write(&self, w: &mut Writer) {
        w.f(&[
            self.dt_s,
            self.srv_steam_kg_s,
            self.srv_h,
            self.srv_h2_kg_s,
            self.leak_steam_kg_s,
            self.leak_h,
            self.leak_h2_kg_s,
            self.leak_gas_k,
            b(self.vent_ordered),
            self.ex_vessel_w,
            self.pedestal_inflow_kg_s,
            self.vessel_drain_kg_s,
            self.vessel_drain_k,
            b(self.gas_cv_t),
            self.xe_relief,
            self.xe_leak,
            self.cs_relief_kg,
            self.cs_leak_kg,
            self.uo2_relief_kg,
            self.uo2_leak_kg,
            self.i_relief_kg,
            self.i_leak_kg,
            b(self.pool_scrubbing),
            b(self.scrubbing_saturation),
            b(self.swarm_scrubbing),
            b(self.sectional_classes),
            b(self.iodine_partition),
            b(self.mixture_vent),
            b(self.phoresis),
            b(self.iodine_swarm),
            b(self.condensate_mass),
            self.q_noble_w,
            self.q_alkali_w,
            self.q_halogen_w,
            self.q_chalcogen_w,
            b(self.ww_atmosphere),
            b(self.dw_wall_hs),
            b(self.ww_shell),
            b(self.outflow_enthalpy),
            b(self.relief_h2_enthalpy),
            self.srv_gas_k,
            b(self.condensate_to_floor),
            self.rpv_wall_w,
            b(self.mcci),
            b(self.dw_wall_outer),
            b(self.pool_layers),
            b(self.cavity_phases),
            b(self.bur_ignition),
            b(self.gas_choking),
            b(self.two_sided_equalising),
            b(self.burn_dynamics),
            b(self.fdi_lpme),
            self.vessel_p_pa,
            self.cav_uo2_kg,
            self.cav_zr_kg,
            self.cav_debris_k,
            self.spray_kg_s,
            self.spray_k,
            b(self.spray_exchange),
        ]);
        self.rn_relief.write(w);
        self.rn_leak.write(w);
        self.prm.write(w);
    }
    pub fn read(r: &mut Reader) -> Self {
        Self {
            dt_s: r.f(),
            srv_steam_kg_s: r.f(),
            srv_h: r.f(),
            srv_h2_kg_s: r.f(),
            leak_steam_kg_s: r.f(),
            leak_h: r.f(),
            leak_h2_kg_s: r.f(),
            leak_gas_k: r.f(),
            vent_ordered: r.f() != 0.0,
            ex_vessel_w: r.f(),
            pedestal_inflow_kg_s: r.f(),
            vessel_drain_kg_s: r.f(),
            vessel_drain_k: r.f(),
            gas_cv_t: r.f() != 0.0,
            xe_relief: r.f(),
            xe_leak: r.f(),
            cs_relief_kg: r.f(),
            cs_leak_kg: r.f(),
            uo2_relief_kg: r.f(),
            uo2_leak_kg: r.f(),
            i_relief_kg: r.f(),
            i_leak_kg: r.f(),
            pool_scrubbing: r.f() != 0.0,
            scrubbing_saturation: r.f() != 0.0,
            swarm_scrubbing: r.f() != 0.0,
            sectional_classes: r.f() != 0.0,
            iodine_partition: r.f() != 0.0,
            mixture_vent: r.f() != 0.0,
            phoresis: r.f() != 0.0,
            iodine_swarm: r.f() != 0.0,
            condensate_mass: r.f() != 0.0,
            q_noble_w: r.f(),
            q_alkali_w: r.f(),
            q_halogen_w: r.f(),
            q_chalcogen_w: r.f(),
            ww_atmosphere: r.f() != 0.0,
            dw_wall_hs: r.f() != 0.0,
            ww_shell: r.f() != 0.0,
            outflow_enthalpy: r.f() != 0.0,
            relief_h2_enthalpy: r.f() != 0.0,
            srv_gas_k: r.f(),
            condensate_to_floor: r.f() != 0.0,
            rpv_wall_w: r.f(),
            mcci: r.f() != 0.0,
            dw_wall_outer: r.f() != 0.0,
            pool_layers: r.f() != 0.0,
            cavity_phases: r.f() != 0.0,
            bur_ignition: r.f() != 0.0,
            gas_choking: r.f() != 0.0,
            two_sided_equalising: r.f() != 0.0,
            burn_dynamics: r.f() != 0.0,
            fdi_lpme: r.f() != 0.0,
            vessel_p_pa: r.f(),
            cav_uo2_kg: r.f(),
            cav_zr_kg: r.f(),
            cav_debris_k: r.f(),
            spray_kg_s: r.f(),
            spray_k: r.f(),
            spray_exchange: r.f() != 0.0,
            rn_relief: RnFlow::read(r),
            rn_leak: RnFlow::read(r),
            prm: Params::read(r),
        }
    }
}

impl ContainmentResult {
    pub fn write(&self, w: &mut Writer) {
        w.f(&[
            self.p_dw_pa,
            self.p_ww_pa,
            self.t_dw_k,
            self.t_pool_k,
            self.t_pool_top_k,
            self.dw_h2_kg,
            self.ww_h2_kg,
            self.rb_h2_fraction,
            b(self.vent_flowing),
            b(self.flange_leaking),
            b(self.rb_flammable),
            b(self.rb_exploded),
            self.rb_burn_y,
            self.rb_co_fraction,
            b(self.rb_burning),
            self.rb_burn_dp_pa,
            self.fdi_steam_kg,
            b(self.fdi_hpme),
            self.vented_h2_kg,
            self.vented_kg_s,
            self.xe_dw,
            self.xe_ww,
            self.xe_vented,
            self.xe_building,
            self.cs_dw_structures_kg,
            self.cs_ww_pool_kg,
            self.cs_airborne_kg,
            self.cs_vented_kg,
            self.cs_building_kg,
            self.i_dw_structures_kg,
            self.i_ww_pool_kg,
            self.i_airborne_kg,
            self.i_vented_kg,
            self.i_building_kg,
            self.te_kg,
            self.cav_ablated_kg,
            self.cav_h2_kg,
            self.cav_co_kg,
            self.cav_debris_k,
            self.cav_zr_kg,
        ]);
    }
    pub fn read(r: &mut Reader) -> Self {
        Self {
            p_dw_pa: r.f(),
            p_ww_pa: r.f(),
            t_dw_k: r.f(),
            t_pool_k: r.f(),
            t_pool_top_k: r.f(),
            dw_h2_kg: r.f(),
            ww_h2_kg: r.f(),
            rb_h2_fraction: r.f(),
            vent_flowing: r.f() != 0.0,
            flange_leaking: r.f() != 0.0,
            rb_flammable: r.f() != 0.0,
            rb_exploded: r.f() != 0.0,
            rb_burn_y: r.f(),
            rb_co_fraction: r.f(),
            rb_burning: r.f() != 0.0,
            rb_burn_dp_pa: r.f(),
            fdi_steam_kg: r.f(),
            fdi_hpme: r.f() != 0.0,
            vented_h2_kg: r.f(),
            vented_kg_s: r.f(),
            xe_dw: r.f(),
            xe_ww: r.f(),
            xe_vented: r.f(),
            xe_building: r.f(),
            cs_dw_structures_kg: r.f(),
            cs_ww_pool_kg: r.f(),
            cs_airborne_kg: r.f(),
            cs_vented_kg: r.f(),
            cs_building_kg: r.f(),
            i_dw_structures_kg: r.f(),
            i_ww_pool_kg: r.f(),
            i_airborne_kg: r.f(),
            i_vented_kg: r.f(),
            i_building_kg: r.f(),
            te_kg: r.f(),
            cav_ablated_kg: r.f(),
            cav_h2_kg: r.f(),
            cav_co_kg: r.f(),
            cav_debris_k: r.f(),
            cav_zr_kg: r.f(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **V&V (gh:#1096, item 3): the drywell wall on this model's inputs,
    /// through `tampines`' heat-structure coupling, reproduces this model's
    /// numbers.** Methodology: the containment is stepped from
    /// `Params::default()` for 1800 s at 1 s with 20 kg/s of steam and
    /// 0.02 kg/s of hydrogen leaking into the drywell (`dw_wall_hs` on), then
    /// the drywell's start-of-step wall exchange is computed two ways: (a) as
    /// step 3 of [`Containment::step`] does (`dw_wall_surface`, the 90 %
    /// limit, the latent heat `m (h_g − c_p (T_w − 273.15))`), and (b) by
    /// `tampines::components::wall_condensation::WallCondensation` on the
    /// containment's convention (`without_real_steam`) over a one-cell
    /// `HeatStructure` slab of the wall's area (width `dw_wall_area_m2`, 1 m
    /// high) at the wall's temperature, the drywell's gas as a `WallGas` of
    /// the same moles. **Prediction (before the run):** the coefficient and
    /// the condensed mass bit-identical (the same kernel, the same operation
    /// order for `m_flux A dt`); the latent heat to round-off (< 1e-14
    /// relative; the product is formed in another order). The sensible heat
    /// is not compared: the containment exchanges it implicitly in the two
    /// temperatures with the gas's capacity, the coupling explicitly through
    /// the slab's half cell (stated). **Result (2026-11-01), held:** at
    /// t = 300, 600, ..., 1800 s (T_dw 405.4 to 489.1 K, p_dw 0.178 to
    /// 0.266 MPa, wall 341.6 to 402.5 K) `h` (6.39 to 7.36 W/(m2 K)) and the
    /// condensed mass (16.07, 14.85, 10.15, 2.16, 0, 0 kg in the step) are
    /// bit-identical, and the latent heat differs by 0 (better than the
    /// predicted 1e-14). Record:
    /// `crates/tampines/verification_and_validation/wall_condensation.md`.
    #[test]
    fn the_drywell_wall_through_tampines_reproduces_the_containment() {
        use tampines::components::heat_structure::{ConductionZone, HeatStructure, StructureShape, StructureSide};
        use tampines::components::wall_condensation::{WallCondensation, WallGas};
        use uom::si::amount_of_substance::mole;
        use uom::si::f64::{AmountOfSubstance, Length, Pressure, ThermalConductivity, ThermodynamicTemperature, Time, VolumetricHeatCapacity};
        use uom::si::heat_transfer::watt_per_square_meter_kelvin;
        use uom::si::length::meter;
        use uom::si::mass_rate::kilogram_per_second;
        use uom::si::power::watt;
        use uom::si::pressure::pascal;
        use uom::si::thermal_conductivity::watt_per_meter_kelvin;
        use uom::si::thermodynamic_temperature::kelvin;
        use uom::si::time::second;
        use uom::si::volumetric_heat_capacity::joule_per_cubic_meter_kelvin;

        let prm = Params::default();
        let mut c = Containment::new(&prm);
        let snap = ContainmentSnapshot {
            dt_s: 1.0,
            leak_steam_kg_s: 20.0,
            leak_h: 2.8e6,
            leak_h2_kg_s: 0.02,
            leak_gas_k: 550.0,
            gas_cv_t: true,
            dw_wall_hs: true,
            prm,
            ..Default::default()
        };
        let mut worst_latent: f64 = 0.0;
        let mut checked = 0;
        for k in 0..1800 {
            c.step(&snap);
            if k % 300 != 299 {
                continue;
            }
            let dt = 1.0;
            // (a) The containment's own step 3.
            let (h_a, m_flux) = c.dw_wall_surface(&prm);
            let m_a = (m_flux * prm.dw_wall_area_m2 * dt).min(0.9 * c.dw_steam_kg);
            let lat_a = m_a * (pool_surface::steam_h(c.dw_t) - CP_WATER * (c.dw_wall_t - 273.15));
            // (b) tampines.
            let wall = HeatStructure::new(
                StructureShape::Slab { width: Length::new::<meter>(prm.dw_wall_area_m2) },
                &[ConductionZone {
                    thickness: Length::new::<meter>(0.02),
                    n_cells: 1,
                    conductivity: ThermalConductivity::new::<watt_per_meter_kelvin>(50.0),
                    volumetric_heat_capacity: VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(3.9e6),
                }],
                1,
                Length::new::<meter>(1.0),
                ThermodynamicTemperature::new::<kelvin>(c.dw_wall_t),
            )
            .unwrap();
            let gas = WallGas {
                temperature: ThermodynamicTemperature::new::<kelvin>(c.dw_t),
                pressure: Pressure::new::<pascal>(c.p_dw(&prm)),
                steam: AmountOfSubstance::new::<mole>(c.dw_steam_kg / M_H2O),
                nitrogen: AmountOfSubstance::new::<mole>(c.dw_n2),
                hydrogen: AmountOfSubstance::new::<mole>(c.dw_h2),
            };
            let x = WallCondensation::new()
                .without_real_steam()
                .exchange(&wall, StructureSide::Outer, &gas, Length::new::<meter>(prm.dw_wall_height_m), Time::new::<second>(dt))
                .unwrap();
            let h_b = x.heat_transfer_coefficient[0].get::<watt_per_square_meter_kelvin>();
            let m_b = x.condensation_rate.get::<kilogram_per_second>() * dt;
            let lat_b = x.latent_power_to_wall.get::<watt>() * dt;
            let rel = if lat_a != 0.0 { ((lat_b - lat_a) / lat_a).abs() } else { lat_b.abs() };
            worst_latent = worst_latent.max(rel);
            println!(
                "t {:5} s: T_dw {:.3} K, p_dw {:.1} Pa, T_wall {:.3} K, steam {:.1} kg; h {h_a:.6} / {h_b:.6} W/(m2 K); condensed {m_a:.6} / {m_b:.6} kg; latent {lat_a:.6e} / {lat_b:.6e} J ({rel:.1e})",
                k + 1,
                c.dw_t,
                c.p_dw(&prm),
                c.dw_wall_t,
                c.dw_steam_kg
            );
            assert_eq!(h_a.to_bits(), h_b.to_bits());
            assert_eq!(m_a.to_bits(), m_b.to_bits());
            assert!(rel < 1e-14, "{rel}");
            checked += 1;
            if m_a > 0.0 {
                assert!(lat_a > 0.0);
            }
        }
        println!("worst latent-heat difference {worst_latent:.2e} over {checked} states");
        assert_eq!(checked, 6);
    }

    /// MELCOR's BUR ignition test (BUR RM Eqs 2-3 to 2-5) by hand: 10 %
    /// hydrogen ignites, 9.9 % does not; 6 % H2 with 6.68 % CO is Le
    /// Chatelier's `0.06 + 0.0668 x 0.10/0.167` = 0.10, so ignites; 4.9 %
    /// oxygen or 55 % diluent does not.
    #[test]
    fn the_building_ignites_by_melcors_bur_criteria() {
        let prm = Params::default();
        assert!(bur_ignites(&prm, 0.10, 0.0, 0.2, 0.0));
        assert!(!bur_ignites(&prm, 0.099, 0.0, 0.2, 0.0));
        assert!(bur_ignites(&prm, 0.06, 0.0669, 0.2, 0.0) && !bur_ignites(&prm, 0.06, 0.0667, 0.2, 0.0));
        assert!(!bur_ignites(&prm, 0.2, 0.0, 0.049, 0.0));
        assert!(!bur_ignites(&prm, 0.2, 0.0, 0.2, 0.55) && bur_ignites(&prm, 0.2, 0.0, 0.2, 0.549));
    }

    /// MELCOR's burn (BUR RM §2.3 to §2.5) by hand, in the 26,000 m^3
    /// building (1.0449e6 mol at 1 atm and 303.15 K; CDIM 18.378 m):
    /// - 12 % H2, no diluent: `V = 172.88 x 0.12 - 9.576` = 11.170 m/s,
    ///   `t_comb` = 1.6453 s, the completeness 1.93 clipped to 1. At a 1 s
    ///   step it burns 1/1.6453 of the H2, then the rest; 1.2539e5 mol
    ///   gives 3.032e10 J, a bound of `Q / (2.5 V)` = 0.4665 MPa, and uses
    ///   6.27e4 mol of oxygen.
    /// - 6 % H2: the completeness `23.4116 (0.06 - 0.03746)` = 0.5277, so
    ///   it ends at 2.834 %, over `18.378 / 5.344` = 3.439 s.
    /// - `Cdil` at 20 % diluent: `1 - 0.906 + 0.2148` = 0.3088; at 30 %
    ///   `Y`, `1 - 1.29 x 0.2` = 0.742.
    #[test]
    fn the_building_burns_by_melcors_bur_rates() {
        let prm = Params::default();
        assert!((bur_cdim(26_000.0) - 18.378).abs() < 1e-3);
        assert!((bur_flame_speed(0.12, 0.0) - 11.1696).abs() < 1e-9);
        assert!((bur_flame_speed(0.12, 0.2) / 11.1696 - 0.3088).abs() < 1e-9);
        assert!((bur_flame_speed(0.35, 0.2) / (-50.0 * 0.35 + 45.0) - 0.742).abs() < 1e-9);
        // Eq 2-18 joins Eqs 2-17 and 2-19 at both ends.
        let lean = bur_flame_speed(0.2, 0.2) / (172.88 * 0.2 - 9.576);
        assert!((bur_flame_speed(0.200_000_1, 0.2) / (50.0 * 0.200_000_1 + 15.0) - lean).abs() < 1e-5);
        assert!((bur_completeness(&prm, 0.06) - 0.5277).abs() < 1e-4 && bur_completeness(&prm, 0.03) == 0.0 && bur_completeness(&prm, 0.12) == 1.0);
        let n_rb = P_ATM * prm.reactor_building_m3 / (R_GAS * T_RB);
        assert!((n_rb / 1.0449e6 - 1.0).abs() < 1e-4, "{n_rb}");
        let mut c = Containment::new(&prm);
        c.rb_h2 = 0.12 * n_rb;
        c.step_burn(&prm, true, n_rb, 0.0, 0.2095, 1.0);
        assert!(c.exploded && (c.burn.left_s - 0.6453).abs() < 1e-4, "{:?}", c.burn);
        assert!((c.rb_h2 / (0.12 * n_rb) - (1.0 - 1.0 / 1.6453)).abs() < 1e-4);
        c.step_burn(&prm, true, n_rb, 0.0, 0.2095, 1.0);
        assert!(c.rb_h2.abs() < 1e-6 && c.burn.left_s.abs() < 1e-12, "{} {:?}", c.rb_h2, c.burn);
        assert!((c.burn.q_j / 3.032e10 - 1.0).abs() < 1e-3 && (c.burn.dp_max / 0.4665e6 - 1.0).abs() < 1e-3, "{:?}", c.burn);
        assert!((c.burn.o2_used / (0.06 * n_rb) - 1.0).abs() < 1e-9);
        // Over: a further step burns nothing, though the criterion holds.
        let mut lean = Containment::new(&prm);
        lean.rb_h2 = 0.06 * n_rb;
        for _ in 0..4 {
            lean.step_burn(&prm, true, n_rb, 0.0, 0.2095, 1.0);
        }
        assert!((lean.rb_h2 / n_rb - (1.0 - 0.5277) * 0.06).abs() < 1e-5, "{}", lean.rb_h2 / n_rb);
        assert!(lean.burn.left_s == 0.0, "{:?}", lean.burn);
        let before = lean.rb_h2;
        lean.step_burn(&prm, false, n_rb, 0.0, 0.2095, 1.0);
        assert_eq!(lean.rb_h2, before);
    }

    /// The critical flow (MELCOR Eq 6-13) by hand: nitrogen at 0.8 MPa and
    /// 450 K (rho 5.99 kg/m^3, c_s 432.4 m/s) through 5e-4 m^2 with Cd 0.6:
    /// `0.6 x 5e-4 x 5.99 x 432.4 x 0.5787` = 0.4497 kg/s, 16.05 mol/s,
    /// about half the incompressible orifice's 0.88 kg/s at that ratio.
    #[test]
    fn a_leak_to_the_atmosphere_chokes() {
        let n = critical_moles(0.6, 5e-4, 0.8e6, 450.0, M_N2, 0.0);
        assert!((n * M_N2 / 0.4497 - 1.0).abs() < 2e-3, "{}", n * M_N2);
        let rho = 0.8e6 * M_N2 / (R_GAS * 450.0);
        let incompressible = 0.6 * 5e-4 * (2.0 * rho * (0.8e6 - 1.0133e5)).sqrt();
        assert!(incompressible / (n * M_N2) > 1.9 && incompressible / (n * M_N2) < 2.0, "{incompressible}");
    }

    /// At rest the containment stays at its initial pressure.
    #[test]
    fn an_idle_containment_holds_its_pressure() {
        let mut c = Containment::new(&Params::default());
        let s = ContainmentSnapshot { dt_s: 1.0, ..Default::default() };
        let mut r = c.step(&s);
        for _ in 0..1000 {
            r = c.step(&s);
        }
        assert!((r.p_dw_pa - INITIAL_CONTAINMENT_PA).abs() < 2.0e3, "{}", r.p_dw_pa);
        assert!((r.p_ww_pa - INITIAL_CONTAINMENT_PA).abs() < 2.0e3, "{}", r.p_ww_pa);
    }

    /// Relief-valve steam into the pool: the pool's energy rises by exactly
    /// the steam's enthalpy (the pool balance conserves energy).
    #[test]
    fn relief_steam_heats_the_pool_by_its_enthalpy() {
        let mut c = Containment::new(&Params::default());
        let e0 = c.pool_energy();
        let s = ContainmentSnapshot { dt_s: 1.0, srv_steam_kg_s: 50.0, srv_h: 2.77e6, ..Default::default() };
        c.step(&s);
        assert!((c.pool_energy() - e0 - 50.0 * 2.77e6).abs() < 1.0, "{}", c.pool_energy() - e0);
    }

    /// Air's conductivity by White's Sutherland constants against the
    /// tabulated values (Incropera's Table A.4: 0.0263 W/(m K) at 300 K,
    /// 0.0667 at 1000 K): 0.02625 and 0.06612, within 1 %.
    #[test]
    fn air_conductivity_matches_the_tables() {
        assert!((air_conductivity(300.0) / 0.0263 - 1.0).abs() < 0.01, "{}", air_conductivity(300.0));
        assert!((air_conductivity(1000.0) / 0.0667 - 1.0).abs() < 0.01, "{}", air_conductivity(1000.0));
    }
}

#[cfg(test)]
mod spray_tests {
    use super::*;

    /// The spray's equilibrium (gh:#1109) closes its energy balance: the
    /// steam it condenses at `h_g(T_gas)` heats the spray from `T_in` and
    /// leaves as condensate at the same temperature; capped at 90 % of the
    /// steam, the temperature comes from the balance. A dry atmosphere
    /// condenses nothing. Prediction before the run: 4 kg/s at 293 K in a
    /// drywell at 420 K with 0.3 MPa of steam (dew point about 407 K)
    /// condenses about 4 x 4186 x 114 / (2.586e6 - 4186 x 134) = 0.94 kg/s.
    /// **Result (2026-11-02): 0.8597 kg/s at 406.68 K. The prediction MISSED
    /// by 9 %**: it took h_g as an IF97 enthalpy (2.586 MJ/kg), where the
    /// containment's `steam_h(420 K)` is 2.770 MJ/kg (its `u + RT/M` with
    /// a constant `c_v`); with that, the hand value is 0.860.
    #[test]
    fn the_spray_closes_its_energy_balance() {
        let (w, t_in, t_gas) = (4.0, 293.15, 420.0);
        let (m, t) = spray_condensation(w, t_in, t_gas, 0.3e6, 5000.0, 1.0);
        let h_g = pool_surface::steam_h(t_gas);
        println!("spray: {m:.4} kg/s condensed, water at {t:.2} K");
        assert!((m * h_g - (w * CP_WATER * (t - t_in) + m * CP_WATER * (t - 273.15))).abs() < 1e-6 * m * h_g);
        assert!((t - pool_surface::t_sat(0.3e6)).abs() < 1e-9 && (0.85..1.05).contains(&m), "{m} {t}");
        // Capped: 0.5 kg of steam, 0.45 of it in a 1 s step.
        let (m, t) = spray_condensation(w, t_in, t_gas, 0.3e6, 0.5, 1.0);
        assert!((m - 0.45).abs() < 1e-12 && t < pool_surface::t_sat(0.3e6));
        assert!((m * h_g - (w * CP_WATER * (t - t_in) + m * CP_WATER * (t - 273.15))).abs() < 1e-6 * m * h_g);
        assert_eq!(spray_condensation(w, t_in, t_gas, 0.0, 0.0, 1.0), (0.0, t_in));
    }

    /// The lumped containment takes the spray: with the exchange its
    /// droplets condense drywell steam and land with it on the floor, so
    /// less steam goes down the downcomers to the pool; without it
    /// (`Mechanisms::without_containment_spray`) the water lands unchanged
    /// and nothing else differs from no spray. Water is conserved either
    /// way. (The drywell's pressure and steam are not the measure: the
    /// downcomers' equalising cap holds the drywell at the wetwell's plus
    /// the submergence, so after one step the sprayed drywell holds 0.17
    /// kg *more* steam at the same pressure; found 2026-11-02 when the
    /// first form of this test asserted less.)
    #[test]
    fn the_lumped_drywell_takes_the_spray() {
        let prm = Params::unit3();
        let base = ContainmentSnapshot { dt_s: 1.0, leak_steam_kg_s: 20.0, leak_h: 2.8e6, leak_gas_k: 500.0, prm, ..Default::default() };
        let mut warm = Containment::new(&prm);
        for _ in 0..600 {
            warm.step(&base);
        }
        let (mut a, mut b, mut c) = (warm.clone(), warm.clone(), warm);
        let spray = ContainmentSnapshot { spray_kg_s: 4.0, spray_k: 293.15, spray_exchange: true, ..base.clone() };
        a.step(&spray);
        b.step(&ContainmentSnapshot { spray_exchange: false, ..spray.clone() });
        c.step(&base);
        println!("drywell steam after one step: spray {:.2} kg, landing unchanged {:.2} kg, none {:.2} kg", a.dw_steam_kg, b.dw_steam_kg, c.dw_steam_kg);
        let water = |k: &Containment| k.dw_steam_kg + k.pedestal_kg + k.pool_kg + k.ww_steam_kg;
        println!("pool after one step: spray {:.3} kg, unchanged {:.3} kg; floor {:.3} / {:.3} kg", a.pool_kg, b.pool_kg, a.pedestal_kg, b.pedestal_kg);
        assert!(a.pool_kg < b.pool_kg && a.pedestal_kg > b.pedestal_kg + 0.5, "{} {}", a.pedestal_kg, b.pedestal_kg);
        assert!(b.dw_steam_kg == c.dw_steam_kg && b.pool_kg == c.pool_kg && (b.pedestal_kg - c.pedestal_kg - 4.0).abs() < 1e-9);
        assert!((water(&a) - water(&b)).abs() < 1e-6 * water(&a) && (water(&b) - water(&c) - 4.0).abs() < 1e-6 * water(&b));
    }
}
