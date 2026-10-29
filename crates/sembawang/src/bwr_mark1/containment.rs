//! The Mark I containment and the reactor building. This is what the
//! **containment worker** runs.
//!
//! ## Model (AI draft; read the limitations)
//!
//! - **Drywell**: nitrogen, hydrogen and steam at one temperature, ideal
//!   gases (steam's energy referenced to IF97's saturated vapour at
//!   100 degC with a constant `c_v`); a lumped wall heat sink (steel and
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
use super::steam::p_sat_pa;
use super::wire::{Reader, Writer};

/// Ideal-gas molar heat capacities, J/(mol K). \[A\] constant.
const CV_N2: f64 = 20.8;
const CV_H2: f64 = 20.5;
const CP_H2: f64 = CV_H2 + R_GAS;
/// Steam: constant `c_v`, J/(kg K), and IF97 saturated-vapour internal
/// energy at 373.15 K, J/kg. \[A\] constant `c_v`.
const CV_STEAM: f64 = 1500.0;
const U_STEAM_REF: f64 = 2.5060e6;
const T_REF: f64 = 373.15;
/// Liquid water: constant `c_p`, J/(kg K). \[A\]
const CP_WATER: f64 = 4186.0;
/// Drywell structures: area, m^2, convective coefficient, W/(m^2 K), heat
/// capacity, J/K. \[A\] all three placeholders (steel shell, concrete, the
/// pedestal); the real heat-sink inventory is in SAND2012-6173.
const DW_WALL_AREA_M2: f64 = 4000.0;
const DW_WALL_H: f64 = 20.0;
const DW_WALL_C: f64 = 5.0e8;
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
    pub fn new() -> Self {
        let dw_n2 = INITIAL_CONTAINMENT_PA * DRYWELL_VOLUME_M3 / (R_GAS * INITIAL_DRYWELL_K);
        let p_n2_ww = INITIAL_CONTAINMENT_PA - p_sat_pa(INITIAL_POOL_K);
        let ww_n2 = p_n2_ww * WETWELL_GAS_M3 / (R_GAS * INITIAL_POOL_K);
        Self {
            dw_n2,
            dw_h2: 0.0,
            dw_steam_kg: 0.0,
            dw_t: INITIAL_DRYWELL_K,
            dw_wall_t: INITIAL_DRYWELL_K,
            ww_n2,
            ww_h2: 0.0,
            pool_kg: POOL_WATER_M3 * 995.0,
            pool_t: INITIAL_POOL_K,
            rb_h2: 0.0,
            disk_burst: false,
            flange_open: false,
            exploded: false,
            vented_h2_kg: 0.0,
            pedestal_kg: 0.0,
            pedestal_t: INITIAL_DRYWELL_K,
        }
    }

    fn dw_moles(&self) -> f64 {
        self.dw_n2 + self.dw_h2 + self.dw_steam_kg / M_H2O
    }
    pub fn p_dw(&self) -> f64 {
        self.dw_moles() * R_GAS * self.dw_t / DRYWELL_VOLUME_M3
    }
    pub fn p_ww(&self) -> f64 {
        (self.ww_n2 + self.ww_h2) * R_GAS * self.pool_t / WETWELL_GAS_M3 + p_sat_pa(self.pool_t)
    }
    fn dw_energy(&self) -> f64 {
        (self.dw_n2 * CV_N2 + self.dw_h2 * CV_H2) * self.dw_t + self.dw_steam_kg * steam_u(self.dw_t)
    }
    /// Drywell gas temperature from its energy (linear in T: solved exactly).
    fn set_dw_energy(&mut self, e: f64) {
        let c = self.dw_n2 * CV_N2 + self.dw_h2 * CV_H2 + self.dw_steam_kg * CV_STEAM;
        let e0 = self.dw_steam_kg * (U_STEAM_REF - CV_STEAM * T_REF);
        self.dw_t = (e - e0) / c;
    }
    fn pool_energy(&self) -> f64 {
        self.pool_kg * CP_WATER * (self.pool_t - 273.15)
    }
    fn set_pool_energy(&mut self, e: f64) {
        self.pool_t = 273.15 + e / (self.pool_kg * CP_WATER);
    }

    /// Advance one step. Pure function of `self` and `s`.
    pub fn step(&mut self, s: &ContainmentSnapshot) -> ContainmentResult {
        let dt = s.dt_s;
        // 1. Relief valves into the pool.
        let m = s.srv_steam_kg_s * dt;
        let e = self.pool_energy() + m * s.srv_h;
        self.pool_kg += m;
        self.set_pool_energy(e);
        self.ww_h2 += s.srv_h2_kg_s * dt / M_H2;
        // 2. RPV leak into the drywell.
        let m = s.leak_steam_kg_s * dt;
        let nh = s.leak_h2_kg_s * dt / M_H2;
        let e = self.dw_energy() + m * s.leak_h + nh * CP_H2 * s.leak_gas_k;
        self.dw_steam_kg += m;
        self.dw_h2 += nh;
        self.set_dw_energy(e);
        // 2b. Debris on the drywell floor: it heats and boils the water
        //     poured onto it; dry, its heat goes into the floor and walls
        //     (molten-core-concrete interaction is not modelled).
        if s.pedestal_inflow_kg_s > 0.0 {
            let m_in = s.pedestal_inflow_kg_s * dt;
            let e = self.pedestal_kg * self.pedestal_t + m_in * INJECTION_TEMPERATURE_K;
            self.pedestal_kg += m_in;
            self.pedestal_t = e / self.pedestal_kg;
        }
        if s.ex_vessel_w > 0.0 {
            let q = s.ex_vessel_w * dt;
            if self.pedestal_kg > 0.0 {
                let t_boil = t_sat(self.p_dw());
                let to_boil = ((t_boil - self.pedestal_t) * self.pedestal_kg * CP_WATER).max(0.0);
                if q <= to_boil {
                    self.pedestal_t += q / (self.pedestal_kg * CP_WATER);
                } else {
                    self.pedestal_t = t_boil;
                    let h_fg = 2.257e6;
                    let m = ((q - to_boil) / h_fg).min(self.pedestal_kg);
                    self.pedestal_kg -= m;
                    let e = self.dw_energy() + m * (steam_u(t_boil) + R_GAS / M_H2O * t_boil);
                    self.dw_steam_kg += m;
                    self.set_dw_energy(e);
                }
            } else {
                self.dw_wall_t += q / DW_WALL_C;
            }
        }
        // 3. Drywell wall heat sink, implicit in the two temperatures.
        let ha = DW_WALL_AREA_M2 * DW_WALL_H;
        let c_gas = self.dw_n2 * CV_N2 + self.dw_h2 * CV_H2 + self.dw_steam_kg * CV_STEAM;
        let (tg, tw) = (self.dw_t, self.dw_wall_t);
        // q = ha (Tg' - Tw'); c_gas (Tg' - Tg) = -q dt; C_w (Tw' - Tw) = q dt.
        let k = ha * dt;
        let q = k * (tg - tw) / (1.0 + k / c_gas + k / DW_WALL_C);
        self.dw_t = tg - q / c_gas;
        self.dw_wall_t = tw + q / DW_WALL_C;
        // 4. Condensation of supersaturated steam (latent heat stays in the
        //    gas until the wall takes it): a few fixed-point passes.
        for _ in 0..4 {
            let p_s = self.dw_steam_kg / M_H2O * R_GAS * self.dw_t / DRYWELL_VOLUME_M3;
            let p_sat = p_sat_pa(self.dw_t.min(640.0));
            if p_s <= p_sat {
                break;
            }
            let keep = p_sat * DRYWELL_VOLUME_M3 * M_H2O / (R_GAS * self.dw_t);
            let cond = self.dw_steam_kg - keep;
            let e = self.dw_energy() - cond * CP_WATER * (self.dw_t - 273.15);
            self.dw_steam_kg = keep;
            self.set_dw_energy(e);
        }
        // 5. Downcomers (drywell -> pool) and vacuum breakers (wetwell -> drywell).
        let (p_dw, p_ww) = (self.p_dw(), self.p_ww());
        let push = p_dw - p_ww - VENT_SUBMERGENCE_PA;
        if push > 0.0 {
            let n_dw = self.dw_moles();
            let molar_mass = (self.dw_n2 * M_N2 + self.dw_h2 * M_H2 + self.dw_steam_kg) / n_dw;
            let rho = p_dw * molar_mass / (R_GAS * self.dw_t);
            let orifice = DISCHARGE_COEFFICIENT * DOWNCOMER_AREA_M2 * (2.0 * rho * push).sqrt() * dt / molar_mass;
            // Equalising amount: drywell pressure down by `push` (isothermal).
            let n = orifice.min(n_dw * push / p_dw);
            let f = n / n_dw;
            let (dn2, dh2, dsteam) = (self.dw_n2 * f, self.dw_h2 * f, self.dw_steam_kg * f);
            let e_dw = self.dw_energy();
            self.dw_n2 -= dn2;
            self.dw_h2 -= dh2;
            self.dw_steam_kg -= dsteam;
            // The drywell gas keeps its temperature; the moved gas's energy
            // goes to the pool (steam condenses, N2/H2 cool to the pool).
            let e_moved = e_dw - self.dw_energy();
            let e_pool = self.pool_energy() + e_moved - (dn2 * CV_N2 + dh2 * CV_H2) * self.pool_t;
            self.pool_kg += dsteam;
            self.set_pool_energy(e_pool);
            self.ww_n2 += dn2;
            self.ww_h2 += dh2;
        }
        let (p_dw, p_ww) = (self.p_dw(), self.p_ww());
        let suck = p_ww - p_dw - VACUUM_BREAKER_PA;
        if suck > 0.0 {
            let n_ww = self.ww_n2 + self.ww_h2;
            let molar = (self.ww_n2 * M_N2 + self.ww_h2 * M_H2) / n_ww;
            let rho = p_ww * molar / (R_GAS * self.pool_t);
            let orifice = DISCHARGE_COEFFICIENT * VACUUM_BREAKER_M2 * (2.0 * rho * suck).sqrt() * dt / molar;
            // At most the isothermal equalising amount of the non-condensables.
            let n = orifice.min(n_ww * suck / p_ww);
            let f = (n / n_ww).min(1.0);
            let (dn2, dh2) = (self.ww_n2 * f, self.ww_h2 * f);
            self.ww_n2 -= dn2;
            self.ww_h2 -= dh2;
            let e = self.dw_energy() + (dn2 * CV_N2 + dh2 * CV_H2) * self.pool_t;
            self.dw_n2 += dn2;
            self.dw_h2 += dh2;
            self.set_dw_energy(e);
        }
        // 6. Wetwell vent through the rupture disk.
        let p_ww = self.p_ww();
        if s.vent_ordered && p_ww >= VENT_RUPTURE_DISK_PA {
            self.disk_burst = true;
        }
        let mut vented_kg_s = 0.0;
        let vent_flowing = s.vent_ordered && self.disk_burst && p_ww > P_ATM;
        if vent_flowing {
            let n_g = self.ww_n2 + self.ww_h2;
            let p_nc = n_g * R_GAS * self.pool_t / WETWELL_GAS_M3;
            let molar = (self.ww_n2 * M_N2 + self.ww_h2 * M_H2) / n_g.max(1e-30);
            let rho = p_ww * molar / (R_GAS * self.pool_t);
            let orifice = DISCHARGE_COEFFICIENT * VENT_AREA_M2 * (2.0 * rho * (p_ww - P_ATM)).sqrt() * dt / molar;
            // Equalising amount of non-condensables, the steam partial
            // pressure being held by the pool.
            let n = orifice.min(n_g * ((p_ww - P_ATM) / p_nc).min(1.0));
            let f = n / n_g;
            let dh2 = self.ww_h2 * f;
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
        let p_dw = self.p_dw();
        if p_dw >= FLANGE_LEAK_PA {
            self.flange_open = true;
        }
        if self.flange_open && p_dw > P_ATM {
            let n_dw = self.dw_moles();
            let molar = (self.dw_n2 * M_N2 + self.dw_h2 * M_H2 + self.dw_steam_kg) / n_dw;
            let rho = p_dw * molar / (R_GAS * self.dw_t);
            let orifice = DISCHARGE_COEFFICIENT * FLANGE_LEAK_M2 * (2.0 * rho * (p_dw - P_ATM)).sqrt() * dt / molar;
            let n = orifice.min(n_dw * (p_dw - P_ATM) / p_dw);
            let f = n / n_dw;
            let dh2 = self.dw_h2 * f;
            let dn2 = self.dw_n2 * f;
            self.dw_n2 -= dn2;
            self.dw_h2 -= dh2;
            self.dw_steam_kg -= self.dw_steam_kg * f;
            // The building is well mixed at atmospheric pressure: incoming
            // non-condensables push out building air at its mixture fraction
            // (the steam condenses on the cold building).
            if !self.exploded {
                let n_rb = P_ATM * REACTOR_BUILDING_M3 / (R_GAS * T_RB);
                let y = self.rb_h2 / n_rb;
                self.rb_h2 += dh2 - y * (dh2 + dn2);
            }
        }
        let n_rb = P_ATM * REACTOR_BUILDING_M3 / (R_GAS * T_RB);
        let y_rb = self.rb_h2 / n_rb;
        if !self.exploded && y_rb >= H2_EXPLOSION {
            self.exploded = true;
            self.rb_h2 = 0.0;
        }
        ContainmentResult {
            p_dw_pa: self.p_dw(),
            p_ww_pa: self.p_ww(),
            t_dw_k: self.dw_t,
            t_pool_k: self.pool_t,
            dw_h2_kg: self.dw_h2 * M_H2,
            ww_h2_kg: self.ww_h2 * M_H2,
            rb_h2_fraction: self.rb_h2 / n_rb,
            vent_flowing,
            flange_leaking: self.flange_open,
            rb_flammable: self.rb_h2 / n_rb >= H2_FLAMMABLE,
            rb_exploded: self.exploded,
            vented_h2_kg: self.vented_h2_kg,
            vented_kg_s,
        }
    }
}

impl Default for Containment {
    fn default() -> Self {
        Self::new()
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
        ]);
    }
    pub fn read_state(r: &mut Reader) -> Self {
        Self {
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
        }
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
        ]);
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// At rest the containment stays at its initial pressure.
    #[test]
    fn an_idle_containment_holds_its_pressure() {
        let mut c = Containment::new();
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
        let mut c = Containment::new();
        let e0 = c.pool_energy();
        let s = ContainmentSnapshot { dt_s: 1.0, srv_steam_kg_s: 50.0, srv_h: 2.77e6, ..Default::default() };
        c.step(&s);
        assert!((c.pool_energy() - e0 - 50.0 * 2.77e6).abs() < 1.0, "{}", c.pool_energy() - e0);
    }
}
