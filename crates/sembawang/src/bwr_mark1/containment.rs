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
//! - **Wetwell (torus)**: the pool, one temperature; the gas space above it
//!   at the pool temperature, saturated with steam (an equilibrium
//!   assumption).
//! - **Downcomers**: drywell gas pushed into the pool once the drywell
//!   exceeds the wetwell by the submergence head; the steam condenses in the
//!   pool, the nitrogen and hydrogen bubble through into the wetwell gas at
//!   the pool's temperature. **Vacuum breakers** the other way.
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
use super::aerosol::{Facing, Medium, Sections, Surface};
use super::gas::{h2_molar_enthalpy, ncg_c, ncg_u};
use super::rn_volume::{RnSurface, RnVolume, CS, IODINE};
use super::vapour::BulkGas;
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
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
    /// The plant's inputs (gh:#1033).
    pub prm: Params,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ContainmentResult {
    pub p_dw_pa: f64,
    pub p_ww_pa: f64,
    pub t_dw_k: f64,
    pub t_pool_k: f64,
    pub dw_h2_kg: f64,
    pub ww_h2_kg: f64,
    pub rb_h2_fraction: f64,
    pub vent_flowing: bool,
    pub flange_leaking: bool,
    pub rb_flammable: bool,
    pub rb_exploded: bool,
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
}

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
        }
    }

    fn dw_moles(&self) -> f64 {
        self.dw_n2 + self.dw_h2 + self.dw_steam_kg / M_H2O
    }
    pub fn p_dw(&self, prm: &Params) -> f64 {
        self.dw_moles() * R_GAS * self.dw_t / prm.drywell_volume_m3
    }
    pub fn p_ww(&self, prm: &Params) -> f64 {
        (self.ww_n2 + self.ww_h2) * R_GAS * self.pool_t / prm.wetwell_gas_m3 + p_sat_pa(self.pool_t)
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
    fn set_pool_energy(&mut self, e: f64) {
        self.pool_t = 273.15 + e / (self.pool_kg * CP_WATER);
    }

    /// Advance one step. Pure function of `self` and `s`.
    pub fn step(&mut self, s: &ContainmentSnapshot) -> ContainmentResult {
        let prm = &s.prm;
        let dt = s.dt_s;
        let tv = s.gas_cv_t;
        // 1. Relief valves into the pool.
        let m = s.srv_steam_kg_s * dt;
        let e = self.pool_energy() + m * s.srv_h;
        self.pool_kg += m;
        self.set_pool_energy(e);
        self.ww_h2 += s.srv_h2_kg_s * dt / M_H2;
        // Noble gas is a trace non-condensable: it rides with the gas it
        // arrives in and moves in the same fraction as N2 and H2 on every
        // path below (gh:#1040).
        self.xe_ww += s.xe_relief;
        self.xe_dw += s.xe_leak;
        // Cesium and aerosol likewise: the relief flow's into the wetwell
        // gas (MELCOR's default with pool interaction off: gas below a pool
        // surface passes unchanged), the leaks' into the drywell.
        let rho_p = Medium::air(300.0, P_ATM, 1.8e-5).rho_p;
        self.ww_rn.add(s.uo2_relief_kg, rho_p, [s.cs_relief_kg, s.i_relief_kg]);
        self.dw_rn.add(s.uo2_leak_kg, rho_p, [s.cs_leak_kg, s.i_leak_kg]);
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
        if s.ex_vessel_w > 0.0 {
            let q = s.ex_vessel_w * dt;
            if self.pedestal_kg > 0.0 {
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
            } else {
                self.dw_wall_t += q / prm.dw_wall_c;
            }
        }
        // 3. Drywell wall heat sink, implicit in the two temperatures.
        let ha = prm.dw_wall_area_m2 * prm.dw_wall_h;
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
            let e = self.dw_energy(tv) - cond * CP_WATER * (self.dw_t - 273.15);
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
            let (moved, cv, ca) = self.dw_rn.take(f);
            self.ww_rn.receive(&moved, cv, ca);
            let e_dw = self.dw_energy(tv);
            self.dw_n2 -= dn2;
            self.dw_h2 -= dh2;
            self.dw_steam_kg -= dsteam;
            // The drywell gas keeps its temperature; the moved gas's energy
            // goes to the pool (steam condenses, N2/H2 cool to the pool).
            let e_moved = e_dw - self.dw_energy(tv);
            let e_pool = self.pool_energy() + e_moved - ncg_u(dn2, dh2, self.pool_t, tv);
            self.pool_kg += dsteam;
            self.set_pool_energy(e_pool);
            self.ww_n2 += dn2;
            self.ww_h2 += dh2;
        }
        let (p_dw, p_ww) = (self.p_dw(prm), self.p_ww(prm));
        let suck = p_ww - p_dw - prm.vacuum_breaker_pa;
        if suck > 0.0 {
            let n_ww = self.ww_n2 + self.ww_h2;
            let molar = (self.ww_n2 * M_N2 + self.ww_h2 * M_H2) / n_ww;
            let rho = p_ww * molar / (R_GAS * self.pool_t);
            let orifice = prm.discharge_coefficient * prm.vacuum_breaker_m2 * (2.0 * rho * suck).sqrt() * dt / molar;
            // At most the isothermal equalising amount of the non-condensables.
            let n = orifice.min(n_ww * suck / p_ww);
            let f = (n / n_ww).min(1.0);
            let (dn2, dh2) = (self.ww_n2 * f, self.ww_h2 * f);
            let dxe = self.xe_ww * f;
            self.xe_ww -= dxe;
            self.xe_dw += dxe;
            let (moved, cv, ca) = self.ww_rn.take(f);
            self.dw_rn.receive(&moved, cv, ca);
            self.ww_n2 -= dn2;
            self.ww_h2 -= dh2;
            let e = self.dw_energy(tv) + ncg_u(dn2, dh2, self.pool_t, tv);
            self.dw_n2 += dn2;
            self.dw_h2 += dh2;
            self.set_dw_energy(e, tv);
        }
        // 6. Wetwell vent through the rupture disk.
        let p_ww = self.p_ww(prm);
        if s.vent_ordered && p_ww >= prm.vent_rupture_disk_pa {
            self.disk_burst = true;
        }
        let mut vented_kg_s = 0.0;
        let vent_flowing = s.vent_ordered && self.disk_burst && p_ww > P_ATM;
        if vent_flowing {
            let n_g = self.ww_n2 + self.ww_h2;
            let p_nc = n_g * R_GAS * self.pool_t / prm.wetwell_gas_m3;
            let molar = (self.ww_n2 * M_N2 + self.ww_h2 * M_H2) / n_g.max(1e-30);
            let rho = p_ww * molar / (R_GAS * self.pool_t);
            let orifice = prm.discharge_coefficient * prm.vent_area_m2 * (2.0 * rho * (p_ww - P_ATM)).sqrt() * dt / molar;
            // Equalising amount of non-condensables, the steam partial
            // pressure being held by the pool.
            let n = orifice.min(n_g * ((p_ww - P_ATM) / p_nc).min(1.0));
            let f = n / n_g;
            let dh2 = self.ww_h2 * f;
            let dxe = self.xe_ww * f;
            self.xe_ww -= dxe;
            self.xe_vented += dxe;
            let (_, cv, ca) = self.ww_rn.take(f);
            self.cs_vented_kg += cv[CS] + ca[CS];
            self.i_vented_kg += cv[IODINE] + ca[IODINE];
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
            let (_, cv, ca) = self.dw_rn.take(f);
            self.cs_building_kg += cv[CS] + ca[CS];
            self.i_building_kg += cv[IODINE] + ca[IODINE];
            self.dw_n2 -= dn2;
            self.dw_h2 -= dh2;
            self.dw_steam_kg -= self.dw_steam_kg * f;
            // The building is well mixed at atmospheric pressure: incoming
            // non-condensables push out building air at its mixture fraction
            // (the steam condenses on the cold building).
            if !self.exploded {
                let n_rb = P_ATM * prm.reactor_building_m3 / (R_GAS * T_RB);
                let y = self.rb_h2 / n_rb;
                self.rb_h2 += dh2 - y * (dh2 + dn2);
            }
        }
        // 8. Fission products and aerosol: condensation, agglomeration and
        //    deposition, every 10 s of accumulated time (transport above
        //    moves them every step).
        self.rn_clock_s += dt;
        if self.rn_clock_s >= RN_STEP_S {
            let h = self.rn_clock_s;
            self.rn_clock_s = 0.0;
            self.step_rn(prm, h);
        }
        let n_rb = P_ATM * prm.reactor_building_m3 / (R_GAS * T_RB);
        let y_rb = self.rb_h2 / n_rb;
        if !self.exploded && y_rb >= prm.h2_explosion {
            self.exploded = true;
            self.rb_h2 = 0.0;
        }
        ContainmentResult {
            p_dw_pa: self.p_dw(prm),
            p_ww_pa: self.p_ww(prm),
            t_dw_k: self.dw_t,
            t_pool_k: self.pool_t,
            dw_h2_kg: self.dw_h2 * M_H2,
            ww_h2_kg: self.ww_h2 * M_H2,
            rb_h2_fraction: self.rb_h2 / n_rb,
            vent_flowing,
            flange_leaking: self.flange_open,
            rb_flammable: self.rb_h2 / n_rb >= prm.h2_flammable,
            rb_exploded: self.exploded,
            vented_h2_kg: self.vented_h2_kg,
            vented_kg_s,
            xe_dw: self.xe_dw,
            xe_ww: self.xe_ww,
            xe_vented: self.xe_vented,
            xe_building: self.xe_building,
            cs_dw_structures_kg: self.dw_rn.on_surface_kg.iter().map(|x| x[CS]).sum(),
            cs_ww_pool_kg: self.ww_rn.on_surface_kg[0][CS],
            cs_airborne_kg: self.dw_rn.vapour_kg[CS] + self.dw_rn.on_aerosol_kg[CS] + self.ww_rn.vapour_kg[CS] + self.ww_rn.on_aerosol_kg[CS],
            cs_vented_kg: self.cs_vented_kg,
            cs_building_kg: self.cs_building_kg,
            i_dw_structures_kg: self.dw_rn.on_surface_kg.iter().map(|x| x[IODINE]).sum(),
            i_ww_pool_kg: self.ww_rn.on_surface_kg[0][IODINE],
            i_airborne_kg: self.dw_rn.vapour_kg[IODINE] + self.dw_rn.on_aerosol_kg[IODINE] + self.ww_rn.vapour_kg[IODINE] + self.ww_rn.on_aerosol_kg[IODINE],
            i_vented_kg: self.i_vented_kg,
            i_building_kg: self.i_building_kg,
        }
    }
}

/// Aerosol step, s (the transport steps every plant step).
const RN_STEP_S: f64 = 10.0;

/// MELCOR's default aerosol size range, 0.1 to 50 um (RN1_ASP), as 27
/// sections of particle-volume ratio 2 (0.1 to 40 um, the fallout section
/// at 50 um).
fn rn_sections() -> Sections {
    Sections::new(0.1e-6, 27, 2.0)
}

/// Air's viscosity, Pa s, by Sutherland's law (1.458e-6 T^1.5/(T + 110.4),
/// the standard constants): MELCOR evaluates its aerosol coefficients with
/// air's properties (Vol. 2, RN §2.4.2.3, constraint c).
fn air_viscosity(t_k: f64) -> f64 {
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
    fn step_rn(&mut self, prm: &Params, dt: f64) {
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
        let k_w = prm.dw_wall_h / (rho * cp);
        let dw_surfaces = [
            RnSurface { surface: Surface { area: prm.dw_floor_m2, facing: Facing::Floor, phoretic: vec![0.0; n] }, t_k: self.dw_wall_t, k_vapour: k_w },
            RnSurface { surface: Surface { area: prm.dw_wall_area_m2, facing: Facing::Wall, phoretic: vec![0.0; n] }, t_k: self.dw_wall_t, k_vapour: k_w },
        ];
        self.dw_rn.step(&m_dw, &gases_dw, &dw_surfaces, dt);
        // Wetwell gas, saturated with steam at the pool temperature.
        let p_ww = self.p_ww(prm);
        let x_s = (p_sat_pa(self.pool_t) / p_ww).min(1.0);
        let n_nc = (self.ww_n2 + self.ww_h2).max(1e-30);
        let gases_ww = [(BulkGas::Steam, x_s), (BulkGas::N2, (1.0 - x_s) * self.ww_n2 / n_nc), (BulkGas::H2, (1.0 - x_s) * self.ww_h2 / n_nc)];
        let m_ww = Medium::air(self.pool_t, p_ww, air_viscosity(self.pool_t));
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
        ]);
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
        };
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
            b(self.gas_cv_t),
            self.xe_relief,
            self.xe_leak,
            self.cs_relief_kg,
            self.cs_leak_kg,
            self.uo2_relief_kg,
            self.uo2_leak_kg,
            self.i_relief_kg,
            self.i_leak_kg,
        ]);
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
            gas_cv_t: r.f() != 0.0,
            xe_relief: r.f(),
            xe_leak: r.f(),
            cs_relief_kg: r.f(),
            cs_leak_kg: r.f(),
            uo2_relief_kg: r.f(),
            uo2_leak_kg: r.f(),
            i_relief_kg: r.f(),
            i_leak_kg: r.f(),
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
            self.dw_h2_kg,
            self.ww_h2_kg,
            self.rb_h2_fraction,
            b(self.vent_flowing),
            b(self.flange_leaking),
            b(self.rb_flammable),
            b(self.rb_exploded),
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
        ]);
    }
    pub fn read(r: &mut Reader) -> Self {
        Self {
            p_dw_pa: r.f(),
            p_ww_pa: r.f(),
            t_dw_k: r.f(),
            t_pool_k: r.f(),
            dw_h2_kg: r.f(),
            ww_h2_kg: r.f(),
            rb_h2_fraction: r.f(),
            vent_flowing: r.f() != 0.0,
            flange_leaking: r.f() != 0.0,
            rb_flammable: r.f() != 0.0,
            rb_exploded: r.f() != 0.0,
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
