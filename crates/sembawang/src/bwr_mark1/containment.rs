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
//! stiff path), never more. **Not in the model**: drywell spray, wetwell
//! stratification, pool level and swell, pool scrubbing of aerosols (no
//! fission products are tracked yet), the torus room, the reactor building's
//! own heat-up and ventilation, steam in the building, and the combustion
//! itself (the explosion is a threshold, not a calculation).

use super::params::*;
use super::aerosol::{diffusiophoretic_velocity, Facing, Medium, Sections, Surface};
use super::gas::{h2_molar_enthalpy, ncg_c, ncg_u};
use super::rn_volume::{RnFlow, RnSurface, RnVolume, CS, IODINE, N_CLASSES, TE};
use super::vapour::{BulkGas, FpClass};
use super::iodine;
use super::sparc;
use super::pool_surface;
use super::cavity::Cavity;
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
    pub cav_uo2_kg: f64,
    pub cav_zr_kg: f64,
    pub cav_debris_k: f64,
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
    /// Noble gas (Xe), as in [`ContainmentResult`].
    xe_dw: f64,
    xe_ww: f64,
    xe_vented: f64,
    xe_building: f64,
    /// Fission products and aerosol in the drywell and the wetwell gas
    /// (gh:#1047), the time since their last aerosol step, s, and cesium
    /// vented and leaked to the building, kg.
    dw_rn: RnVolume,
    ww_rn: RnVolume,
    rn_clock_s: f64,
    cs_vented_kg: f64,
    cs_building_kg: f64,
    i_vented_kg: f64,
    i_building_kg: f64,
    /// Tellurium vented or leaked to the building, kg.
    te_out_kg: f64,
    /// The swarm-rise pass fractions per section at the downcomers, and
    /// their age, s: recomputed at most once per aerosol step (`sparc`
    /// costs about 0.1 ms a call), from the conditions of the step that
    /// recomputes them.
    swarm_pass: Vec<f64>,
    swarm_age_s: f64,
    /// The pool's partition coefficient for I2 (`iodine::partition`),
    /// updated at each aerosol step.
    iodine_h: f64,
    /// The swarm rise's pass fraction for I2 vapour at the downcomers, and
    /// its age, s: recomputed at most once per aerosol step, as the
    /// particles' are.
    iodine_swarm_pass: f64,
    iodine_swarm_age_s: f64,
    /// Steam condensed in the drywell since the last aerosol step, kg: on
    /// its walls, for their diffusiophoresis.
    dw_condensed_kg: f64,
    /// Scrubbed into the pool water at the vent exits (SPARC-90's DF_EC),
    /// kg: cesium, iodine, aerosol.
    pool_scrubbed: [f64; N_CLASSES + 1],
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
}

/// CO and CO2, mol, in the drywell, the wetwell gas and the building.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Tracer {
    dw: [f64; 2],
    ww: [f64; 2],
    rb: [f64; 2],
}

/// MELCOR's BUR ignition test for a volume without igniters (BUR RM §2.2,
/// Eqs 2-3 to 2-5): Le Chatelier's `X_H2 + X_CO L_H2 / L_CO >= L_H2`,
/// oxygen at least `x_o2`, and steam and CO2 below `x_dil`.
pub(crate) fn bur_ignites(prm: &Params, x_h2: f64, x_co: f64, x_o2: f64, x_dil: f64) -> bool {
    x_h2 + x_co * prm.bur_xh2ign / prm.bur_xcoign >= prm.bur_xh2ign && x_o2 >= prm.bur_xo2ig && x_dil < prm.bur_xmscig
}

/// Cells of the concrete slab around the drywell (`dw_wall_outer`).
const DW_SLAB_CELLS: usize = 10;

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
            xe_dw: 0.0,
            xe_ww: 0.0,
            xe_vented: 0.0,
            xe_building: 0.0,
            dw_rn: RnVolume::new(prm.drywell_volume_m3, rn_sections(), 2),
            ww_rn: RnVolume::new(prm.wetwell_gas_m3, rn_sections(), 1),
            rn_clock_s: 0.0,
            cs_vented_kg: 0.0,
            cs_building_kg: 0.0,
            i_vented_kg: 0.0,
            i_building_kg: 0.0,
            te_out_kg: 0.0,
            swarm_pass: vec![1.0; rn_sections().v.len()],
            swarm_age_s: RN_STEP_S,
            iodine_h: iodine::partition(INITIAL_POOL_K, 0.0, 0.0, 0.0),
            dw_condensed_kg: 0.0,
            iodine_swarm_pass: 1.0,
            iodine_swarm_age_s: RN_STEP_S,
            pool_scrubbed: [0.0; N_CLASSES + 1],
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
        }
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
        let n = DW_SLAB_CELLS;
        let dx = prm.dw_concrete_m / n as f64;
        let area = prm.dw_wall_area_m2;
        let (tw, t0) = (self.dw_wall_t, self.dw_slab[0]);
        let tm = 0.5 * (tw + t0);
        let h_gap = air_conductivity(tm) / prm.dw_gap_m + 4.0 * 5.670_374e-8 * tm * tm * tm / (2.0 / prm.dw_gap_emissivity - 1.0);
        let half = 0.5 * dx / prm.concrete_k;
        let q_in = area * (tw - t0) / (1.0 / h_gap + half);
        let q_out = area * (self.dw_slab[n - 1] - T_RB) / (1.0 / prm.rb_wall_h + half);
        let c_cell = prm.concrete_density * prm.concrete_cp * dx * area;
        let g = prm.concrete_k / dx * area;
        let old = self.dw_slab;
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
            self.dw_slab[i] = old[i] + q * dt / c_cell;
        }
        self.dw_wall_t -= q_in * dt / prm.dw_wall_c;
    }
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
        self.swarm_age_s += dt;
        self.iodine_swarm_age_s += dt;
        self.dw_rn.lumped = !s.sectional_classes;
        self.ww_rn.lumped = !s.sectional_classes;
        self.dw_rn.condensate_mass = s.condensate_mass;
        self.ww_rn.condensate_mass = s.condensate_mass;
        // 0. Decay heat of the fission products the containment holds
        //    (`fp_decay`): airborne in the drywell, into its gas; on its
        //    structures, into its walls; in the wetwell gas and the pool,
        //    into the pool.
        if s.q_noble_w + s.q_alkali_w + s.q_halogen_w + s.q_chalcogen_w > 0.0 {
            let cs = |kg: f64| kg / prm.cs_inventory_kg * s.q_alkali_w;
            let io = |kg: f64| kg / prm.i_inventory_kg * s.q_halogen_w;
            let te = |kg: f64| kg / prm.te_inventory_kg * s.q_chalcogen_w;
            let air = |v: &RnVolume, c: usize| v.vapour_kg[c] + v.on_aerosol(c);
            let held = |v: &RnVolume, c: usize| v.on_surface_kg.iter().map(|x| x[c]).sum::<f64>();
            let dw_gas = self.xe_dw * s.q_noble_w + cs(air(&self.dw_rn, CS)) + io(air(&self.dw_rn, IODINE)) + te(air(&self.dw_rn, TE));
            let dw_wall = cs(held(&self.dw_rn, CS)) + io(held(&self.dw_rn, IODINE)) + te(held(&self.dw_rn, TE));
            let pool = self.xe_ww * s.q_noble_w
                + cs(air(&self.ww_rn, CS) + held(&self.ww_rn, CS) + self.pool_scrubbed[CS])
                + io(air(&self.ww_rn, IODINE) + held(&self.ww_rn, IODINE) + self.pool_scrubbed[IODINE])
                + te(air(&self.ww_rn, TE) + held(&self.ww_rn, TE) + self.pool_scrubbed[TE]);
            let e = self.dw_energy(tv) + dw_gas * dt;
            self.set_dw_energy(e, tv);
            self.dw_wall_t += dw_wall * dt / prm.dw_wall_c;
            let e = self.pool_energy() + pool * dt;
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
        // Noble gas is a trace non-condensable: it rides with the gas it
        // arrives in and moves in the same fraction as N2 and H2 on every
        // path below (gh:#1040).
        self.xe_ww += s.xe_relief;
        self.xe_dw += s.xe_leak;
        // Cesium and aerosol likewise: the relief flow's into the wetwell
        // gas (MELCOR's default with pool interaction off: gas below a pool
        // surface passes unchanged), the leaks' into the drywell.
        let rho_p = Medium::air(300.0, P_ATM, 1.8e-5).rho_p;
        // The relief flow enters the pool at the quenchers: with scrubbing,
        // only the share of its volume left once the steam condenses passes
        // (SPARC-90's DF_EC: "aerosol particles and iodine vapors are
        // removed in proportion to the reduction in the volumetric flow
        // rate"; Vol. 2 RN §2.7.1, Eqs D-5, D-6). The quenchers' depth is
        // not known; the downcomers' is used. Impaction and the swarm rise
        // are applied at the downcomers only: the quenchers' geometry is
        // not known either, and the relief flow's aerosol, all in the
        // smallest section, would pass the swarm almost whole (0.98 in
        // `sparc`'s test).
        let pool = sparc::Pool::submerged(self.pool_t, self.p_ww(prm), prm.vent_submergence_pa);
        let x_o = if s.scrubbing_saturation { pool.x_o() } else { 1.0 };
        let pass = if s.pool_scrubbing { condensation_pass(nc_share(s.srv_h2_kg_s / M_H2, 0.0, s.srv_steam_kg_s / M_H2O), x_o) } else { 1.0 };
        let relief_m3 = (s.srv_h2_kg_s / M_H2 + s.srv_steam_kg_s / M_H2O) * dt * R_GAS * self.pool_t / (self.p_ww(prm) + prm.vent_submergence_pa);
        if s.rn_relief.moved.is_empty() {
            let pass_i = self.iodine_vapour_pass(&pool, s.pool_scrubbing && s.iodine_partition, pass, s.i_relief_kg, relief_m3);
            self.pool_scrubbed[CS] += s.cs_relief_kg * (1.0 - pass);
            self.pool_scrubbed[IODINE] += s.i_relief_kg * (1.0 - pass_i);
            self.pool_scrubbed[AEROSOL] += s.uo2_relief_kg * (1.0 - pass);
            self.ww_rn.add(s.uo2_relief_kg * pass, rho_p, [s.cs_relief_kg * pass, s.i_relief_kg * pass_i, 0.0]);
            self.dw_rn.add(s.uo2_leak_kg, rho_p, [s.cs_leak_kg, s.i_leak_kg, 0.0]);
        } else {
            // The vessel's aerosol arrives with its size distribution
            // (`vessel_aerosol`): the relief flow's is scrubbed section by
            // section by the condensation, its vapour likewise (iodine's
            // within the partition limit); the leaks' enters the drywell.
            let rf = &s.rn_relief;
            let pass_i = self.iodine_vapour_pass(&pool, s.pool_scrubbing && s.iodine_partition, pass, rf.vapour_kg[IODINE], relief_m3);
            let pass_v = [pass, pass_i, pass];
            for k in 0..N_CLASSES {
                self.pool_scrubbed[k] += rf.vapour_kg[k] * (1.0 - pass_v[k]) + rf.on_aerosol_kg[k].iter().sum::<f64>() * (1.0 - pass);
            }
            self.pool_scrubbed[AEROSOL] += rf.moved.iter().sum::<f64>() * rho_p * (1.0 - pass);
            let moved: Vec<f64> = rf.moved.iter().map(|m| m * pass).collect();
            let ca: [Vec<f64>; N_CLASSES] = std::array::from_fn(|k| rf.on_aerosol_kg[k].iter().map(|m| m * pass).collect());
            self.ww_rn.receive(&moved, std::array::from_fn(|k| rf.vapour_kg[k] * pass_v[k]), &ca);
            let lk = &s.rn_leak;
            if !lk.moved.is_empty() {
                self.dw_rn.receive(&lk.moved, lk.vapour_kg, &lk.on_aerosol_kg);
            }
        }
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
            self.cavity.arrive(prm, s.cav_uo2_kg, s.cav_zr_kg, s.cav_debris_k);
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
                self.dw_condensed_kg += m;
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
            self.dw_condensed_kg += cond;
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
            // Equalising amount: drywell pressure down by `push` (isothermal).
            let n = orifice.min(n_dw * push / p_dw);
            let f = n / n_dw;
            let (dn2, dh2, dsteam) = (self.dw_n2 * f, self.dw_h2 * f, self.dw_steam_kg * f);
            let dxe = self.xe_dw * f;
            self.xe_dw -= dxe;
            self.xe_ww += dxe;
            for i in 0..2 {
                let d = self.co.dw[i] * f;
                self.co.dw[i] -= d;
                self.co.ww[i] += d;
            }
            let (moved, cv, ca) = self.dw_rn.take(f);
            // Through the downcomers into the pool: scrubbed likewise by the
            // condensation, then each section by impaction and the swarm
            // rise (`sparc`), with the cesium and iodine on that section.
            let pool = sparc::Pool::submerged(self.pool_t, p_ww, prm.vent_submergence_pa);
            let x_o = if s.scrubbing_saturation { pool.x_o() } else { 1.0 };
            let pass = if s.pool_scrubbing { condensation_pass(nc_share(dh2, dn2, dsteam / M_H2O), x_o) } else { 1.0 };
            let rho_p = Medium::air(300.0, P_ATM, 1.8e-5).rho_p;
            let swarm: Vec<f64> = if s.pool_scrubbing && s.swarm_scrubbing && moved.iter().any(|&m| m > 0.0) {
                if self.swarm_age_s >= RN_STEP_S {
                    let n_v = prm.downcomer_count;
                    let inj = sparc::Injection {
                        vent: sparc::Vent::Downcomer,
                        diameter_m: (4.0 * prm.downcomer_area_m2 / (n_v * std::f64::consts::PI)).sqrt(),
                        nc_mol_s: (dn2 + dh2) / dt / n_v,
                        q_exit_m3_s: n * R_GAS * self.dw_t / p_dw / dt / n_v,
                    };
                    let d: Vec<f64> = (0..moved.len()).map(|k| self.dw_rn.sections.diameter(k)).collect();
                    self.swarm_pass = sparc::pass_fractions(&pool, &inj, &d, rho_p);
                    self.swarm_age_s = 0.0;
                }
                self.swarm_pass.clone()
            } else {
                vec![1.0; moved.len()]
            };
            let mut pass_i = self.iodine_vapour_pass(&pool, s.pool_scrubbing && s.iodine_partition, pass, cv[IODINE], n * R_GAS * self.dw_t / p_dw);
            // The swarm rise scrubs the I2 vapour too (MELCOR Appendix F;
            // `sparc::iodine_pass`), from the bubble's concentration once
            // the gas is in equilibrium with the pool at the vent exit.
            let n_nc = dn2 + dh2;
            if s.pool_scrubbing && s.iodine_swarm && cv[IODINE] > 0.0 && n_nc > 0.0 && self.iodine_swarm_age_s >= RN_STEP_S {
                let n_v = prm.downcomer_count;
                let inj = sparc::Injection {
                    vent: sparc::Vent::Downcomer,
                    diameter_m: (4.0 * prm.downcomer_area_m2 / (n_v * std::f64::consts::PI)).sqrt(),
                    nc_mol_s: n_nc / dt / n_v,
                    q_exit_m3_s: n * R_GAS * self.dw_t / p_dw / dt / n_v,
                };
                let p0 = p_ww + prm.vent_submergence_pa;
                let p_s = p_sat_pa(self.pool_t);
                let x_s = (p_s / p0).min(1.0);
                let gas_l = n_nc * R_GAS * self.pool_t / (p0 - p_s).max(1.0) * 1e3;
                let gases = [(BulkGas::Steam, x_s), (BulkGas::N2, (1.0 - x_s) * dn2 / n_nc), (BulkGas::H2, (1.0 - x_s) * dh2 / n_nc)];
                let (_, sum_aq, _) = self.pool_chemistry(&pool);
                let iv = sparc::IodineVapour {
                    d_gas: FpClass::I2.diffusivity(&gases, self.pool_t, p0),
                    d_liquid: sparc::i2_liquid_diffusivity(self.pool_t, p0),
                    h: self.iodine_h,
                    sum_aq,
                    c_gas: cv[IODINE] * pass_i / iodine::M_I2 / gas_l,
                };
                self.iodine_swarm_pass = sparc::iodine_pass(&pool, &inj, &iv);
                self.iodine_swarm_age_s = 0.0;
            }
            if s.pool_scrubbing && s.iodine_swarm && cv[IODINE] > 0.0 && n_nc > 0.0 {
                pass_i *= self.iodine_swarm_pass;
            }
            let pass_v = [pass, pass_i, pass];
            for k in 0..N_CLASSES {
                self.pool_scrubbed[k] += cv[k] * (1.0 - pass_v[k]) + ca[k].iter().zip(&swarm).map(|(m, p)| m * (1.0 - pass * p)).sum::<f64>();
            }
            self.pool_scrubbed[AEROSOL] += moved.iter().sum::<f64>() * rho_p * (1.0 - pass)
                + moved.iter().zip(&swarm).map(|(m, p)| m * pass * (1.0 - p)).sum::<f64>() * rho_p;
            let moved: Vec<f64> = moved.iter().zip(&swarm).map(|(m, p)| m * pass * p).collect();
            let ca: [Vec<f64>; N_CLASSES] = std::array::from_fn(|k| ca[k].iter().zip(&swarm).map(|(m, p)| m * pass * p).collect());
            self.ww_rn.receive(&moved, std::array::from_fn(|k| cv[k] * pass_v[k]), &ca);
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
            let f = (orifice / n_ww).min(suck / p_ww);
            let (dn2, dh2, dsteam) = (self.ww_n2 * f, self.ww_h2 * f, self.ww_steam_kg * f);
            let e_out = f * (self.ww_energy(tv) + n_ww * R_GAS * self.ww_t);
            let dxe = self.xe_ww * f;
            self.xe_ww -= dxe;
            self.xe_dw += dxe;
            for i in 0..2 {
                let d = self.co.ww[i] * f;
                self.co.ww[i] -= d;
                self.co.dw[i] += d;
            }
            let (moved, cv, ca) = self.ww_rn.take(f);
            self.dw_rn.receive(&moved, cv, &ca);
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
            // At most the isothermal equalising amount of the non-condensables.
            let n = orifice.min(n_ww * suck / p_ww);
            // An empty wetwell gas moves nothing (0/0 otherwise; gh:#1047).
            let f = if n_ww > 0.0 { (n / n_ww).min(1.0) } else { 0.0 };
            let (dn2, dh2) = (self.ww_n2 * f, self.ww_h2 * f);
            let dxe = self.xe_ww * f;
            self.xe_ww -= dxe;
            self.xe_dw += dxe;
            for i in 0..2 {
                let d = self.co.ww[i] * f;
                self.co.ww[i] -= d;
                self.co.dw[i] += d;
            }
            let (moved, cv, ca) = self.ww_rn.take(f);
            self.dw_rn.receive(&moved, cv, &ca);
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
            let n_flow = prm.discharge_coefficient * prm.vent_area_m2 * (2.0 * rho * (p_ww - P_ATM)).sqrt() * dt / molar;
            let f = (n_flow / n_all).min((p_ww - P_ATM) / p_ww);
            let dh2 = self.ww_h2 * f;
            let dxe = self.xe_ww * f;
            self.xe_ww -= dxe;
            self.xe_vented += dxe;
            for i in 0..2 {
                self.co.ww[i] -= self.co.ww[i] * f;
            }
            let (_, cv, ca) = self.ww_rn.take(f);
            self.cs_vented_kg += cv[CS] + ca[CS].iter().sum::<f64>();
            self.i_vented_kg += cv[IODINE] + ca[IODINE].iter().sum::<f64>();
            self.te_out_kg += cv[TE] + ca[TE].iter().sum::<f64>();
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
            let n_flow = prm.discharge_coefficient * prm.vent_area_m2 * (2.0 * rho * (p_ww - P_ATM)).sqrt() * dt / molar;
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
            let dxe = self.xe_ww * f;
            self.xe_ww -= dxe;
            self.xe_vented += dxe;
            for i in 0..2 {
                self.co.ww[i] -= self.co.ww[i] * f;
            }
            let (_, cv, ca) = self.ww_rn.take(f_gas);
            self.cs_vented_kg += cv[CS] + ca[CS].iter().sum::<f64>();
            self.i_vented_kg += cv[IODINE] + ca[IODINE].iter().sum::<f64>();
            self.te_out_kg += cv[TE] + ca[TE].iter().sum::<f64>();
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
            let orifice = prm.discharge_coefficient * prm.vent_area_m2 * (2.0 * rho * (p_ww - P_ATM)).sqrt() * dt / molar;
            // Equalising amount of non-condensables, the steam partial
            // pressure being held by the pool.
            let n = orifice.min(n_g * ((p_ww - P_ATM) / p_nc).min(1.0));
            // Once the non-condensables are all vented this was 0/0, and
            // the NaN took the wetwell's pressure with it: found 2026-10-29
            // at 73.5 h of the 500 h run, after which no downcomer flowed.
            let f = if n_g > 0.0 { n / n_g } else { 0.0 };
            let dh2 = self.ww_h2 * f;
            let dxe = self.xe_ww * f;
            self.xe_ww -= dxe;
            self.xe_vented += dxe;
            for i in 0..2 {
                self.co.ww[i] -= self.co.ww[i] * f;
            }
            let (_, cv, ca) = self.ww_rn.take(f);
            self.cs_vented_kg += cv[CS] + ca[CS].iter().sum::<f64>();
            self.i_vented_kg += cv[IODINE] + ca[IODINE].iter().sum::<f64>();
            self.te_out_kg += cv[TE] + ca[TE].iter().sum::<f64>();
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
            let orifice = prm.discharge_coefficient * prm.flange_leak_m2 * (2.0 * rho * (p_dw - P_ATM)).sqrt() * dt / molar;
            let n = orifice.min(n_dw * (p_dw - P_ATM) / p_dw);
            let f = n / n_dw;
            let dh2 = self.dw_h2 * f;
            let dn2 = self.dw_n2 * f;
            let dxe = self.xe_dw * f;
            self.xe_dw -= dxe;
            self.xe_building += dxe;
            let dco = [self.co.dw[0] * f, self.co.dw[1] * f];
            self.co.dw[0] -= dco[0];
            self.co.dw[1] -= dco[1];
            let (_, cv, ca) = self.dw_rn.take(f);
            self.cs_building_kg += cv[CS] + ca[CS].iter().sum::<f64>();
            self.i_building_kg += cv[IODINE] + ca[IODINE].iter().sum::<f64>();
            self.te_out_kg += cv[TE] + ca[TE].iter().sum::<f64>();
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
            if !self.exploded {
                let n_rb = P_ATM * prm.reactor_building_m3 / (R_GAS * T_RB);
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
        self.rn_clock_s += dt;
        if self.rn_clock_s >= RN_STEP_S {
            let h = self.rn_clock_s;
            self.rn_clock_s = 0.0;
            self.step_rn(prm, h, s.phoresis);
        }
        let n_rb = P_ATM * prm.reactor_building_m3 / (R_GAS * T_RB);
        let y_rb = self.rb_h2 / n_rb;
        // The building's gas: its hydrogen, CO and CO2 fractions, and its
        // oxygen, air's 0.2095 less what the drywell gas has displaced
        // (its steam condensed on the cold building, stated).
        let x_co = self.co.rb[0] / n_rb;
        let x_co2 = self.co.rb[1] / n_rb;
        let x_o2 = 0.2095 * (1.0 - self.rb_in / n_rb).max(0.0);
        let ignites = if s.bur_ignition { bur_ignites(prm, y_rb, x_co, x_o2, x_co2) } else { y_rb >= prm.h2_explosion };
        let rb_burn_y = y_rb + x_co * prm.bur_xh2ign / prm.bur_xcoign;
        if !self.exploded && ignites {
            self.exploded = true;
            self.rb_h2 = 0.0;
            self.co.rb[0] = 0.0;
        }
        if s.pool_layers {
            self.layer_pool(prm);
        }
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
            vented_h2_kg: self.vented_h2_kg,
            vented_kg_s,
            xe_dw: self.xe_dw,
            xe_ww: self.xe_ww,
            xe_vented: self.xe_vented,
            xe_building: self.xe_building,
            cs_dw_structures_kg: self.dw_rn.on_surface_kg.iter().map(|x| x[CS]).sum(),
            cs_ww_pool_kg: self.ww_rn.on_surface_kg[0][CS] + self.pool_scrubbed[CS],
            cs_airborne_kg: self.dw_rn.vapour_kg[CS] + self.dw_rn.on_aerosol(CS) + self.ww_rn.vapour_kg[CS] + self.ww_rn.on_aerosol(CS),
            cs_vented_kg: self.cs_vented_kg,
            cs_building_kg: self.cs_building_kg,
            i_dw_structures_kg: self.dw_rn.on_surface_kg.iter().map(|x| x[IODINE]).sum(),
            i_ww_pool_kg: self.ww_rn.on_surface_kg[0][IODINE] + self.pool_scrubbed[IODINE],
            i_airborne_kg: self.dw_rn.vapour_kg[IODINE] + self.dw_rn.on_aerosol(IODINE) + self.ww_rn.vapour_kg[IODINE] + self.ww_rn.on_aerosol(IODINE),
            i_vented_kg: self.i_vented_kg,
            i_building_kg: self.i_building_kg,
            te_kg: self.dw_rn.total_kg(TE) + self.ww_rn.total_kg(TE) + self.pool_scrubbed[TE] + self.te_out_kg,
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
fn nc_share(h2_mol: f64, n2_mol: f64, steam_mol: f64) -> f64 {
    let total = h2_mol + n2_mol + steam_mol;
    if total > 0.0 { (h2_mol + n2_mol) / total } else { 1.0 }
}

impl Containment {
    /// The pool water, L, and its dissolved iodine (as I2) and cesium (as
    /// CsOH), mol/L: what was scrubbed into it and what deposited on its
    /// surface from the wetwell gas.
    fn pool_chemistry(&self, pool: &sparc::Pool) -> (f64, f64, f64) {
        let litres = (self.pool_kg + self.deep_kg) / pool.rho_l() * 1e3;
        let i_kg = self.pool_scrubbed[IODINE] + self.ww_rn.on_surface_kg[0][IODINE];
        let cs_kg = self.pool_scrubbed[CS] + self.ww_rn.on_surface_kg[0][CS];
        (litres, i_kg / iodine::M_I2 / litres, cs_kg / iodine::M_CS / litres)
    }

    /// The share of iodine vapour (`i_kg` in `gas_m3` of entering gas) that
    /// passes the condensation, given the gas's share `pass`: MELCOR's
    /// partition limit (Eq D-7, with the pool's `iodine_h`), and no
    /// scrubbing at all when the pool's own iodine over the partition is
    /// at least the gas's (RN §2.7.1). `on` false is the gas's share.
    fn iodine_vapour_pass(&self, pool: &sparc::Pool, on: bool, pass: f64, i_kg: f64, gas_m3: f64) -> f64 {
        if !on || pass >= 1.0 || gas_m3 <= 0.0 {
            return pass;
        }
        let (_, s_pool, _) = self.pool_chemistry(pool);
        let c_in = i_kg / iodine::M_I2 / (gas_m3 * 1e3);
        if s_pool / self.iodine_h >= c_in {
            return 1.0;
        }
        let rho_v = p_sat_pa(self.pool_t) * M_H2O / (R_GAS * self.pool_t);
        let m = (self.iodine_h * rho_v / pool.rho_l()).min(1.0);
        // 1 / DF_EC,vap with DF_EC = 1 / pass, written without dividing
        // by `pass` (which is 0 for pure steam).
        if m >= 1.0 { pass } else { pass / (pass + m * (1.0 - pass)) }
    }
}

/// The share of a gas's volume that passes the condensation at a vent
/// exit: `1 / DF_EC = X_i / X_o` (Eqs D-5, D-6), `X_i` the gas's
/// non-condensable mole fraction and `X_o` the fraction once it is in
/// equilibrium with the pool. At most 1 (DF_EC is at least 1).
fn condensation_pass(x_i: f64, x_o: f64) -> f64 {
    if x_i >= x_o { 1.0 } else { x_i / x_o }
}

/// Index of the particle mass in `pool_scrubbed`, after the classes.
const AEROSOL: usize = N_CLASSES;

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

/// Specific heats for the heat-mass analogy, J/(kg K): steam, N2, H2. \[A\]
/// constant values near 400-500 K.
const CP_STEAM_GAS: f64 = 2000.0;
const CP_N2_GAS: f64 = 1040.0;
const CP_H2_GAS: f64 = 14_300.0;

impl Containment {
    /// The fission-product and aerosol step of both volumes (gh:#1047).
    /// Drywell: its floor and its structures (the wall heat sink's area and
    /// temperature), the vapour's mass-transfer coefficient from the
    /// sink's heat-transfer coefficient by the heat-mass analogy,
    /// `k = h/(rho c_p)` (Lewis number about 1, stated). Wetwell: the pool
    /// surface (a floor, at the pool temperature); its shell is not a
    /// deposition surface yet (stated). No phoretic deposition yet.
    fn step_rn(&mut self, prm: &Params, dt: f64, phoresis: bool) {
        let n = self.dw_rn.sections.v.len();
        // Drywell gas.
        let n_s = self.dw_steam_kg / M_H2O;
        let n_tot = (self.dw_n2 + self.dw_h2 + n_s).max(1e-30);
        let gases_dw = [(BulkGas::Steam, n_s / n_tot), (BulkGas::N2, self.dw_n2 / n_tot), (BulkGas::H2, self.dw_h2 / n_tot)];
        let p_dw = self.p_dw(prm);
        let m_dw = Medium::air(self.dw_t, p_dw, air_viscosity(self.dw_t));
        let mass = self.dw_steam_kg + self.dw_n2 * M_N2 + self.dw_h2 * M_H2;
        let cp = (self.dw_steam_kg * CP_STEAM_GAS + self.dw_n2 * M_N2 * CP_N2_GAS + self.dw_h2 * M_H2 * CP_H2_GAS) / mass.max(1e-30);
        let rho = mass / prm.drywell_volume_m3;
        let h_wall = if self.dw_hs { self.dw_wall_surface(prm).0 } else { prm.dw_wall_h };
        let k_w = h_wall / (rho * cp);
        // Phoresis onto the drywell's structures (MELCOR RN §2.4.2.2):
        // thermophoresis from the gradient of their heat flux, `grad T =
        // q'' / k_air` at the surface temperature (Eq 2-43 and the text
        // after it), and diffusiophoresis from the steam condensed on them
        // since the last aerosol step (Eqs 2-45, 2-46), spread over their
        // area. Both surfaces are at the wall's temperature.
        let phoretic = if phoresis {
            let q = h_wall * (self.dw_t - self.dw_wall_t);
            let grad_t = q / air_conductivity(self.dw_wall_t);
            let w_cond = self.dw_condensed_kg / (dt * (prm.dw_floor_m2 + prm.dw_wall_area_m2));
            let n_nc = (self.dw_n2 + self.dw_h2).max(1e-30);
            let m_nc = (self.dw_n2 * M_N2 + self.dw_h2 * M_H2) / n_nc;
            let rho_sat = p_sat_pa(self.dw_t.min(640.0)) * M_H2O / (R_GAS * self.dw_t);
            let v_dp = diffusiophoretic_velocity(w_cond, n_s / n_tot, M_H2O, m_nc, rho, rho_sat);
            (0..n).map(|k| m_dw.thermophoretic_velocity(self.dw_rn.sections.diameter(k), grad_t, self.dw_wall_t) + v_dp).collect()
        } else {
            vec![0.0; n]
        };
        self.dw_condensed_kg = 0.0;
        let dw_surfaces = [
            RnSurface { surface: Surface { area: prm.dw_floor_m2, facing: Facing::Floor, phoretic: phoretic.clone() }, t_k: self.dw_wall_t, k_vapour: k_w },
            RnSurface { surface: Surface { area: prm.dw_wall_area_m2, facing: Facing::Wall, phoretic }, t_k: self.dw_wall_t, k_vapour: k_w },
        ];
        self.dw_rn.step(&m_dw, &gases_dw, &dw_surfaces, dt);
        // The pool's iodine partition (`iodine`), for the scrubbing until
        // the next aerosol step.
        let pool = sparc::Pool::submerged(self.pool_t, self.p_ww(prm), prm.vent_submergence_pa);
        let (_, s_i, oh) = self.pool_chemistry(&pool);
        self.iodine_h = iodine::partition(self.pool_t, s_i, oh, 0.0);
        // Wetwell gas, saturated with steam at the pool temperature (or the
        // atmosphere's own, with `ww_atmosphere`).
        let p_ww = self.p_ww(prm);
        let (x_s, t_gas) = if self.ww_atm { (self.ww_steam_kg / M_H2O / self.ww_moles(), self.ww_t) } else { ((p_sat_pa(self.pool_t) / p_ww).min(1.0), self.pool_t) };
        let n_nc = (self.ww_n2 + self.ww_h2).max(1e-30);
        let gases_ww = [(BulkGas::Steam, x_s), (BulkGas::N2, (1.0 - x_s) * self.ww_n2 / n_nc), (BulkGas::H2, (1.0 - x_s) * self.ww_h2 / n_nc)];
        let m_ww = Medium::air(t_gas, p_ww, air_viscosity(t_gas));
        let ww_surfaces = [RnSurface {
            surface: Surface { area: prm.ww_pool_surface_m2, facing: Facing::Floor, phoretic: vec![0.0; n] },
            t_k: self.pool_t,
            k_vapour: k_w,
        }];
        self.ww_rn.step(&m_ww, &gases_ww, &ww_surfaces, dt);
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
            self.xe_dw,
            self.xe_ww,
            self.xe_vented,
            self.xe_building,
            self.rn_clock_s,
            self.cs_vented_kg,
            self.cs_building_kg,
            self.i_vented_kg,
            self.i_building_kg,
            self.te_out_kg,
            self.swarm_age_s,
            self.iodine_h,
            self.dw_condensed_kg,
            self.iodine_swarm_pass,
            self.iodine_swarm_age_s,
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
        w.f(&self.swarm_pass);
        w.f(&self.pool_scrubbed);
        self.dw_rn.write(w);
        self.ww_rn.write(w);
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
            xe_dw: r.f(),
            xe_ww: r.f(),
            xe_vented: r.f(),
            xe_building: r.f(),
            dw_rn: RnVolume::new(0.0, rn_sections(), 2),
            ww_rn: RnVolume::new(0.0, rn_sections(), 1),
            rn_clock_s: r.f(),
            cs_vented_kg: r.f(),
            cs_building_kg: r.f(),
            i_vented_kg: r.f(),
            i_building_kg: r.f(),
            te_out_kg: r.f(),
            swarm_pass: vec![],
            swarm_age_s: r.f(),
            iodine_h: r.f(),
            dw_condensed_kg: r.f(),
            iodine_swarm_pass: r.f(),
            iodine_swarm_age_s: r.f(),
            pool_scrubbed: [0.0; N_CLASSES + 1],
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
        };
        c.dw_slab = std::array::from_fn(|_| r.f());
        c.deep_kg = r.f();
        c.deep_t = r.f();
        c.layered = r.f() != 0.0;
        c.co = Tracer { dw: [r.f(), r.f()], ww: [r.f(), r.f()], rb: [r.f(), r.f()] };
        c.rb_in = r.f();
        c.swarm_pass = (0..rn_sections().v.len()).map(|_| r.f()).collect();
        c.pool_scrubbed = std::array::from_fn(|_| r.f());
        c.dw_rn.read_into(r);
        c.ww_rn.read_into(r);
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
            self.cav_uo2_kg,
            self.cav_zr_kg,
            self.cav_debris_k,
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
            cav_uo2_kg: r.f(),
            cav_zr_kg: r.f(),
            cav_debris_k: r.f(),
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
