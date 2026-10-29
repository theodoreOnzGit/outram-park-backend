//! The core: heat-up of the fuel once the water level falls, zirconium-steam
//! oxidation, hydrogen, and relocation. This is what the **core worker**
//! runs (in the browser a Web Worker of its own, as CIET's flow solves are).
//!
//! ## Model (AI draft; read the limitations)
//!
//! Three equal-area rings, centre outwards, times [`AXIAL_NODES`] axial
//! nodes over the active length. Each node is two lumps: the UO2 and the
//! Zircaloy (cladding plus its share of the channel boxes), with
//! - the decay power shared by ring factor and a chopped-cosine axial shape;
//! - fuel-to-clad conductance through the pellet and the gap;
//! - clad-to-coolant: **covered** (below the collapsed level), nucleate
//!   boiling to water at `T_sat`; **uncovered**, convection to the steam
//!   rising from the boil-off below, Dittus-Boelter with IF97 steam
//!   properties at the film temperature, applied in effectiveness form
//!   (`q = W c_p (T_clad - T_in)(1 - e^(-NTU))`), which cannot heat the steam
//!   past the cladding: the second law holds by construction, no guard;
//! - both lumps advanced **implicitly** together (a 2x2 solve per node), so
//!   any time step is stable;
//! - **temperature-dependent heat capacities** (gh:#1038): OFFBEAT's MATPRO
//!   UO2 and MATPRO Zircaloy correlations, reused, not re-entered. The 2x2
//!   solve takes the heat capacities at the start of the step; each lump's
//!   energy is then advanced by the heat that solve moved, and its
//!   temperature found by inverting the correlation's enthalpy, so energy is
//!   conserved exactly. `Mechanisms::without_temperature_dependent_properties`
//!   is the ablation (the constant `UO2_CP`/`ZR_CP` path, unchanged);
//! - oxidation by the parabolic Urbanic-Heidrick law above 1000 K, its heat
//!   into the cladding, limited by the steam that reaches the node (steam
//!   starvation); the steam it uses and the hydrogen it makes travel up;
//! - a node whose cladding reaches [`ZR_MELT_K`] relocates: its mass, energy
//!   and decay power move to a debris lump in the lower plenum, quenched
//!   while water is there; dry debris above [`LOWER_HEAD_FAILURE_K`] fails
//!   the lower head.
//!
//! **Not in the model** (each would change the answer): radiation between
//! rods, to the shroud and upwards (it spreads heat across the core above
//! about 1200 K); the two-phase swell of the mixture level (the collapsed
//! level uncovers the fuel early); candling, blockages, and molten-pool
//! physics; ~~temperature-dependent fuel properties~~ (heat capacities added
//! 2026-10-29, gh:#1038; UO2 conductivity is still constant, waiting for a
//! cited Unit 1 burnup, which MATPRO's conductivity depends on); the decay heat that
//! leaves with volatile fission products; boron carbide control-blade
//! reactions. Debris after the lower head fails leaves the model.

use petir::mathf::RealMath;
use super::params::*;
use outram_park_fork_offbeat::materials::properties::heat_capacity::HeatCapacityModel;
use outram_park_fork_offbeat::materials::MaterialState;
use std::sync::OnceLock;
use super::wire::{Reader, Writer};
use tampines_steam_tables::dynamic_viscosity::mu_tp_eqm_single_phase;
use tampines_steam_tables::prelude::cp_tp_eqm_single_phase;
use tampines_steam_tables::thermal_conductivity::lambda_tp_eqm_single_phase;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{Pressure, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

pub const RINGS: usize = 3;
pub const NODES: usize = RINGS * AXIAL_NODES;

/// What the plant hands the core at the start of a step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CoreSnapshot {
    pub dt_s: f64,
    pub p_pa: f64,
    pub t_sat_k: f64,
    pub h_fg: f64,
    /// Collapsed water level above the bottom of active fuel, m (negative:
    /// below the core; below `-BAF_M` the lower plenum is dry).
    pub level_m: f64,
    /// Decay power of the whole core, W.
    pub decay_w: f64,
    /// Temperature-dependent heat capacities on (the default) or the
    /// constant-property ablation (`Mechanisms`).
    pub t_dependent_props: bool,
    /// The plant's inputs (gh:#1033).
    pub prm: Params,
}

/// What the core hands back.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CoreResult {
    /// Heat into the vessel's water and steam this step, W (mean).
    pub q_fluid_w: f64,
    /// Steam used by oxidation, kg/s, and hydrogen made, kg/s.
    pub steam_used_kg_s: f64,
    pub h2_kg_s: f64,
    /// Oxidation heat, W (already inside the core's balance; for display).
    pub q_ox_w: f64,
    pub peak_clad_k: f64,
    pub exit_gas_k: f64,
    /// Zirconium oxidised so far, kg; hydrogen made so far, kg.
    pub zr_oxidised_kg: f64,
    pub h2_total_kg: f64,
    /// Fraction of the core's nodes relocated, and debris temperature.
    pub relocated: f64,
    pub debris_k: f64,
    pub lower_head_failed: bool,
    /// Decay power of the debris that has left through the failed lower
    /// head, W: it goes to the drywell floor.
    pub ex_vessel_w: f64,
    /// Cladding temperature of every node, ring-major, bottom to top, K
    /// (relocated nodes: NaN).
    pub clad_k: Vec<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Node {
    t_fuel: f64,
    t_clad: f64,
    /// Zr oxidised per unit area, kg/m^2 (the parabolic law's W).
    w_ox: f64,
    relocated: bool,
}

/// Constants of one node (every node in a ring is the same size).
#[derive(Clone, Copy, Debug)]
struct NodeGeom {
    c_fuel: f64,
    c_clad: f64,
    m_fuel: f64,
    g_fc: f64,
    a_rod: f64,
    a_zr: f64,
    zr_kg: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Core {
    nodes: Vec<Node>,
    /// Debris in the constant-property ablation: heat capacity, J/K, and
    /// energy `c T`, J.
    debris_c: f64,
    debris_e: f64,
    /// Debris with temperature-dependent properties: UO2 and Zr masses, kg,
    /// and enthalpy above [`H_REF_K`], J.
    debris_mf: f64,
    debris_mc: f64,
    debris_h: f64,
    debris_power_share: f64,
    lower_head_failed: bool,
    zr_oxidised_kg: f64,
    h2_total_kg: f64,
}

fn geom(prm: &Params) -> NodeGeom {
    let n = NODES as f64;
    let dz = prm.active_length_m / AXIAL_NODES as f64;
    let rod_length = prm.assemblies * prm.rods_per_assembly * dz / RINGS as f64;
    NodeGeom {
        c_fuel: prm.core_uo2_kg() / n * prm.uo2_cp,
        c_clad: prm.core_zr_kg() / n * prm.zr_cp,
        m_fuel: prm.core_uo2_kg() / n,
        g_fc: prm.fuel_to_clad_w_per_m_k() * rod_length,
        a_rod: prm.core_rod_area_m2() / n,
        a_zr: prm.core_zr_area_m2() / n,
        zr_kg: prm.core_zr_kg() / n,
    }
}

/// Chopped-cosine axial shape, normalised to sum to 1.
fn axial_shape(prm: &Params) -> Vec<f64> {
    let l = prm.active_length_m;
    let le = prm.cosine_extrapolation * l;
    let raw: Vec<f64> = (0..AXIAL_NODES)
        .map(|j| {
            let z = (j as f64 + 0.5) / AXIAL_NODES as f64 * l;
            (std::f64::consts::PI * (z - 0.5 * l) / le).r_cos()
        })
        .collect();
    let s: f64 = raw.iter().sum();
    raw.iter().map(|r| r / s).collect()
}

/// Reference temperature of the enthalpy tables, K.
const H_REF_K: f64 = 250.0;
/// Top of the enthalpy tables, K: above UO2's melting point (3120 K).
const H_TOP_K: f64 = 3200.0;
/// Table step, K.
const H_STEP_K: f64 = 1.0;

/// Specific heat capacity, J/(kg K), of one of the two core materials at
/// `t_k`, from OFFBEAT's MATPRO correlation. Outside a correlation's
/// stated window the value at the nearer edge is held: an extrapolation of
/// this model's (MATPRO Zircaloy is stated to 2099 K; a node relocates at
/// [`ZR_MELT_K`] = 2100 K, so the hold only matters in the debris).
fn cp_matpro(model: HeatCapacityModel, t_k: f64) -> f64 {
    let (lo, hi) = model.temperature_range();
    model.value(&MaterialState::fresh(t_k.clamp(lo, hi)))
}

/// Enthalpy above [`H_REF_K`], J/kg, tabulated every [`H_STEP_K`] by the
/// trapezoid rule on [`cp_matpro`]. Built once per worker.
struct Enthalpy {
    h: Vec<f64>,
}

impl Enthalpy {
    fn new(model: HeatCapacityModel) -> Self {
        let n = ((H_TOP_K - H_REF_K) / H_STEP_K) as usize + 1;
        let mut h = Vec::with_capacity(n);
        h.push(0.0);
        for i in 1..n {
            let (t0, t1) = (H_REF_K + (i - 1) as f64 * H_STEP_K, H_REF_K + i as f64 * H_STEP_K);
            h.push(h[i - 1] + 0.5 * (cp_matpro(model, t0) + cp_matpro(model, t1)) * H_STEP_K);
        }
        Self { h }
    }
    /// h(T), J/kg, linear between table points; clamped to the table.
    fn at(&self, t_k: f64) -> f64 {
        let x = ((t_k - H_REF_K) / H_STEP_K).clamp(0.0, (self.h.len() - 1) as f64);
        let i = (x.floor() as usize).min(self.h.len() - 2);
        self.h[i] + (x - i as f64) * (self.h[i + 1] - self.h[i])
    }
    /// T(h), K: the inverse of [`Enthalpy::at`] (h rises strictly with T).
    fn temperature(&self, h: f64) -> f64 {
        let i = match self.h.binary_search_by(|v| v.total_cmp(&h)) {
            Ok(i) => i.min(self.h.len() - 2),
            Err(0) => 0,
            Err(i) => (i - 1).min(self.h.len() - 2),
        };
        let w = ((h - self.h[i]) / (self.h[i + 1] - self.h[i])).clamp(0.0, 1.0);
        H_REF_K + (i as f64 + w) * H_STEP_K
    }
}

fn uo2_enthalpy() -> &'static Enthalpy {
    static T: OnceLock<Enthalpy> = OnceLock::new();
    T.get_or_init(|| Enthalpy::new(HeatCapacityModel::MatproUo2))
}

fn zr_enthalpy() -> &'static Enthalpy {
    static T: OnceLock<Enthalpy> = OnceLock::new();
    T.get_or_init(|| Enthalpy::new(HeatCapacityModel::MatproZircaloy))
}

/// Temperature of debris holding `mf` kg of UO2 and `mc` kg of Zr with
/// enthalpy `h` J above [`H_REF_K`]: bisection on the mixture's h(T).
fn debris_temperature(mf: f64, mc: f64, h: f64) -> f64 {
    let (fu, zr) = (uo2_enthalpy(), zr_enthalpy());
    let (mut a, mut b) = (H_REF_K, H_TOP_K);
    for _ in 0..60 {
        let m = 0.5 * (a + b);
        if mf * fu.at(m) + mc * zr.at(m) < h {
            a = m;
        } else {
            b = m;
        }
    }
    0.5 * (a + b)
}

/// Urbanic-Heidrick parabolic rate constant, (kg Zr/m^2)^2/s.
pub fn oxidation_rate(prm: &Params, t_k: f64) -> f64 {
    if t_k < prm.oxidation_min_k {
        return 0.0;
    }
    let low = |t: f64| prm.uh_low_a * (-prm.uh_low_b_k / t).r_exp();
    let high = |t: f64| prm.uh_high_a * (-prm.uh_high_b_k / t).r_exp();
    if t_k <= prm.uh_low_max_k {
        low(t_k)
    } else if t_k >= prm.uh_high_min_k {
        high(t_k)
    } else {
        let w = (t_k - prm.uh_low_max_k) / (prm.uh_high_min_k - prm.uh_low_max_k);
        low(prm.uh_low_max_k) + w * (high(prm.uh_high_min_k) - low(prm.uh_low_max_k))
    }
}

/// Dittus-Boelter heat transfer coefficient for steam at `t_film` and `p`,
/// for a ring's steam flow `w` (kg/s) through its share of the flow area.
fn steam_htc(prm: &Params, w: f64, t_film: f64, p_pa: f64) -> (f64, f64) {
    let t = ThermodynamicTemperature::new::<kelvin>(t_film.clamp(300.0, 2200.0));
    let p = Pressure::new::<pascal>(p_pa);
    let cp = cp_tp_eqm_single_phase(t, p).get::<joule_per_kilogram_kelvin>();
    let k = lambda_tp_eqm_single_phase(t, p).get::<watt_per_meter_kelvin>();
    let mu = mu_tp_eqm_single_phase(t, p).get::<pascal_second>();
    let dh = prm.hydraulic_diameter_m();
    let area = prm.core_flow_area_m2() / RINGS as f64;
    let re = (w / area) * dh / mu;
    let pr = mu * cp / k;
    // Laminar floor Nu = 4.36 (fully developed, uniform flux).
    let nu = (0.023 * re.r_powf(0.8) * pr.r_powf(0.4)).max(4.36);
    (nu * k / dh, cp)
}

impl Core {
    /// The core just after the scram: fuel at its full-power average
    /// temperature over the coolant, so the stored heat is in it.
    pub fn new(t_sat_k: f64, prm: &Params) -> Self {
        let g = geom(prm);
        let axial = axial_shape(prm);
        let mut nodes = Vec::with_capacity(NODES);
        for ring in 0..RINGS {
            for j in 0..AXIAL_NODES {
                let q = prm.thermal_power_w / RINGS as f64 * prm.ring_factors[ring] * axial[j];
                let t_clad = t_sat_k + q / (prm.boiling_h * g.a_rod);
                nodes.push(Node { t_fuel: t_clad + q / g.g_fc, t_clad, w_ox: 0.0, relocated: false });
            }
        }
        Self {
            nodes,
            debris_c: 0.0,
            debris_e: 0.0,
            debris_mf: 0.0,
            debris_mc: 0.0,
            debris_h: 0.0,
            debris_power_share: 0.0,
            lower_head_failed: false,
            zr_oxidised_kg: 0.0,
            h2_total_kg: 0.0,
        }
    }

    /// Advance one step. Pure function of `self` and `s`.
    pub fn step(&mut self, s: &CoreSnapshot) -> CoreResult {
        let prm = &s.prm;
        let g = geom(prm);
        let axial = axial_shape(prm);
        let dt = s.dt_s;
        let dz = prm.active_length_m / AXIAL_NODES as f64;
        let mut q_fluid = 0.0;
        let mut steam_used = 0.0;
        let mut h2 = 0.0;
        let mut q_ox_total = 0.0;
        let mut peak: f64 = s.t_sat_k;
        let mut exit_sum = 0.0;
        let mut exit_w = 0.0;
        let mut newly_relocated = Vec::new();
        // Debris in the lower plenum (relocated in earlier steps): quenched
        // while the plenum holds water, giving its stored heat over a
        // quench time and its decay power at once; the steam it raises
        // rises through the core. Dry, it heats up until the lower head
        // fails; after that its power leaves the vessel (ex-vessel).
        let p_debris = s.decay_w * self.debris_power_share;
        let mut q_quench = 0.0;
        let mut ex_vessel = 0.0;
        let tdp = s.t_dependent_props;
        if tdp && self.debris_mf + self.debris_mc > 0.0 {
            if self.lower_head_failed {
                ex_vessel = p_debris;
            } else if s.level_m > -prm.baf_m {
                let h_sat = self.debris_mf * uo2_enthalpy().at(s.t_sat_k) + self.debris_mc * zr_enthalpy().at(s.t_sat_k);
                let excess = (self.debris_h - h_sat).max(0.0);
                let release = excess * (1.0 - (-dt / prm.debris_quench_s).r_exp());
                self.debris_h -= release;
                q_quench = p_debris + release / dt;
            } else {
                self.debris_h += p_debris * dt;
                if debris_temperature(self.debris_mf, self.debris_mc, self.debris_h) > prm.lower_head_failure_k {
                    self.lower_head_failed = true;
                }
            }
        } else if !tdp && self.debris_c > 0.0 {
            if self.lower_head_failed {
                ex_vessel = p_debris;
            } else if s.level_m > -prm.baf_m {
                let excess = (self.debris_e - self.debris_c * s.t_sat_k).max(0.0);
                let release = excess * (1.0 - (-dt / prm.debris_quench_s).r_exp());
                self.debris_e -= release;
                q_quench = p_debris + release / dt;
            } else {
                self.debris_e += p_debris * dt;
                if self.debris_e / self.debris_c > prm.lower_head_failure_k {
                    self.lower_head_failed = true;
                }
            }
        }
        q_fluid += q_quench;
        // Steam from below the core, when the level is below it.
        let w_below = if s.level_m <= 0.0 { q_quench / s.h_fg / RINGS as f64 } else { 0.0 };
        for ring in 0..RINGS {
            // Steam rising in this ring: from below, plus the boil-off of
            // its covered nodes.
            let mut w_steam = w_below;
            let mut t_steam = s.t_sat_k;
            for j in 0..AXIAL_NODES {
                let idx = ring * AXIAL_NODES + j;
                let node = self.nodes[idx];
                if node.relocated {
                    continue;
                }
                let q = s.decay_w / RINGS as f64 * prm.ring_factors[ring] * axial[j];
                let z_mid = (j as f64 + 0.5) * dz;
                let covered = s.level_m >= z_mid;
                // Oxidation (explicit in T, uncovered only, steam-limited).
                let mut q_ox = 0.0;
                if !covered {
                    let k = oxidation_rate(prm, node.t_clad);
                    if k > 0.0 && w_steam > 0.0 {
                        let w_max = g.zr_kg / g.a_zr;
                        let w_new = (node.w_ox * node.w_ox + k * dt).sqrt().min(w_max);
                        let mut dzr = (w_new - node.w_ox) * g.a_zr;
                        let steam_per_zr = 2.0 * M_H2O / M_ZR;
                        let dzr_steam = w_steam * dt / steam_per_zr;
                        dzr = dzr.min(dzr_steam);
                        let n = &mut self.nodes[idx];
                        n.w_ox += dzr / g.a_zr;
                        q_ox = dzr * prm.oxidation_heat_j_per_kg_zr / dt;
                        let used = dzr * steam_per_zr / dt;
                        w_steam -= used;
                        steam_used += used;
                        h2 += dzr * 2.0 * M_H2 / M_ZR / dt;
                        self.zr_oxidised_kg += dzr;
                    }
                }
                q_ox_total += q_ox;
                // Clad-to-coolant conductance and coolant temperature.
                let (ha, t_cool, cp) = if covered {
                    (prm.boiling_h * g.a_rod, s.t_sat_k, 0.0)
                } else if w_steam > 0.0 {
                    let (h, cp) = steam_htc(prm, w_steam, 0.5 * (node.t_clad + t_steam), s.p_pa);
                    let ntu = h * g.a_rod / (w_steam * cp);
                    (w_steam * cp * (1.0 - (-ntu).r_exp()), t_steam, cp)
                } else {
                    (0.0, t_steam, 0.0)
                };
                // Implicit two-lump update:
                //   cf (Tf' - Tf) = dt [q - G (Tf' - Tc')]
                //   cc (Tc' - Tc) = dt [G (Tf' - Tc') - hA (Tc' - Tcool) + q_ox]
                let n = self.nodes[idx];
                let (cf, cc, gfc) = if tdp {
                    (
                        g.m_fuel * cp_matpro(HeatCapacityModel::MatproUo2, n.t_fuel),
                        g.zr_kg * cp_matpro(HeatCapacityModel::MatproZircaloy, n.t_clad),
                        g.g_fc,
                    )
                } else {
                    (g.c_fuel, g.c_clad, g.g_fc)
                };
                let a11 = cf / dt + gfc;
                let a12 = -gfc;
                let b1 = cf / dt * n.t_fuel + q;
                let a21 = -gfc;
                let a22 = cc / dt + gfc + ha;
                let b2 = cc / dt * n.t_clad + ha * t_cool + q_ox;
                let det = a11 * a22 - a12 * a21;
                let tf = (b1 * a22 - a12 * b2) / det;
                let tc = (a11 * b2 - a21 * b1) / det;
                let q_out = ha * (tc - t_cool);
                q_fluid += q_out;
                if covered {
                    w_steam += q_out.max(0.0) / s.h_fg;
                } else if w_steam > 0.0 && cp > 0.0 {
                    t_steam += q_out / (w_steam * cp);
                }
                // With temperature-dependent properties, advance each lump's
                // enthalpy by the heat the solve moved and invert for its
                // temperature (exact energy conservation); otherwise the
                // solve's temperatures are the answer.
                let (tf, tc) = if tdp {
                    let ef = g.m_fuel * uo2_enthalpy().at(n.t_fuel) + dt * (q - gfc * (tf - tc));
                    let ec = g.zr_kg * zr_enthalpy().at(n.t_clad) + dt * (gfc * (tf - tc) - q_out + q_ox);
                    (uo2_enthalpy().temperature(ef / g.m_fuel), zr_enthalpy().temperature(ec / g.zr_kg))
                } else {
                    (tf, tc)
                };
                let m = &mut self.nodes[idx];
                m.t_fuel = tf;
                m.t_clad = tc;
                peak = peak.max(tc);
                if tc >= prm.zr_melt_k {
                    newly_relocated.push((idx, q / s.decay_w.max(1.0)));
                }
            }
            exit_sum += w_steam * t_steam;
            exit_w += w_steam;
        }
        // Relocation to the lower plenum.
        for (idx, share) in newly_relocated {
            let n = &mut self.nodes[idx];
            n.relocated = true;
            if tdp {
                self.debris_mf += g.m_fuel;
                self.debris_mc += g.zr_kg;
                self.debris_h += g.m_fuel * uo2_enthalpy().at(n.t_fuel) + g.zr_kg * zr_enthalpy().at(n.t_clad);
            } else {
                self.debris_c += g.c_fuel + g.c_clad;
                self.debris_e += g.c_fuel * n.t_fuel + g.c_clad * n.t_clad;
            }
            self.debris_power_share += share;
        }
        let debris_k = if tdp && self.debris_mf + self.debris_mc > 0.0 {
            debris_temperature(self.debris_mf, self.debris_mc, self.debris_h)
        } else if !tdp && self.debris_c > 0.0 {
            self.debris_e / self.debris_c
        } else {
            s.t_sat_k
        };
        self.h2_total_kg += h2 * dt;
        let relocated = self.nodes.iter().filter(|n| n.relocated).count() as f64 / NODES as f64;
        CoreResult {
            q_fluid_w: q_fluid,
            steam_used_kg_s: steam_used,
            h2_kg_s: h2,
            q_ox_w: q_ox_total,
            peak_clad_k: peak,
            exit_gas_k: if exit_w > 0.0 { exit_sum / exit_w } else { s.t_sat_k },
            zr_oxidised_kg: self.zr_oxidised_kg,
            h2_total_kg: self.h2_total_kg,
            relocated,
            debris_k,
            lower_head_failed: self.lower_head_failed,
            ex_vessel_w: ex_vessel,
            clad_k: self.nodes.iter().map(|n| if n.relocated { f64::NAN } else { n.t_clad }).collect(),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // used by the browser's solver workers (and the tests)
impl Core {
    /// The whole state, for a stateless worker to step.
    pub fn write_state(&self, w: &mut Writer) {
        for n in &self.nodes {
            w.f(&[n.t_fuel, n.t_clad, n.w_ox, if n.relocated { 1.0 } else { 0.0 }]);
        }
        w.f(&[
            self.debris_c,
            self.debris_e,
            self.debris_mf,
            self.debris_mc,
            self.debris_h,
            self.debris_power_share,
            if self.lower_head_failed { 1.0 } else { 0.0 },
            self.zr_oxidised_kg,
            self.h2_total_kg,
        ]);
    }
    pub fn read_state(r: &mut Reader) -> Self {
        let nodes = (0..NODES).map(|_| Node { t_fuel: r.f(), t_clad: r.f(), w_ox: r.f(), relocated: r.f() != 0.0 }).collect();
        Self {
            nodes,
            debris_c: r.f(),
            debris_e: r.f(),
            debris_mf: r.f(),
            debris_mc: r.f(),
            debris_h: r.f(),
            debris_power_share: r.f(),
            lower_head_failed: r.f() != 0.0,
            zr_oxidised_kg: r.f(),
            h2_total_kg: r.f(),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // used by the browser's solver workers (and the tests)
impl CoreSnapshot {
    pub fn write(&self, w: &mut Writer) {
        w.f(&[self.dt_s, self.p_pa, self.t_sat_k, self.h_fg, self.level_m, self.decay_w, if self.t_dependent_props { 1.0 } else { 0.0 }]);
        self.prm.write(w);
    }
    pub fn read(r: &mut Reader) -> Self {
        Self {
            dt_s: r.f(),
            p_pa: r.f(),
            t_sat_k: r.f(),
            h_fg: r.f(),
            level_m: r.f(),
            decay_w: r.f(),
            t_dependent_props: r.f() != 0.0,
            prm: Params::read(r),
        }
    }
}

impl CoreResult {
    pub fn write(&self, w: &mut Writer) {
        w.f(&[
            self.q_fluid_w,
            self.steam_used_kg_s,
            self.h2_kg_s,
            self.q_ox_w,
            self.peak_clad_k,
            self.exit_gas_k,
            self.zr_oxidised_kg,
            self.h2_total_kg,
            self.relocated,
            self.debris_k,
            if self.lower_head_failed { 1.0 } else { 0.0 },
            self.ex_vessel_w,
        ]);
        w.vec(&self.clad_k);
    }
    pub fn read(r: &mut Reader) -> Self {
        Self {
            q_fluid_w: r.f(),
            steam_used_kg_s: r.f(),
            h2_kg_s: r.f(),
            q_ox_w: r.f(),
            peak_clad_k: r.f(),
            exit_gas_k: r.f(),
            zr_oxidised_kg: r.f(),
            h2_total_kg: r.f(),
            relocated: r.f(),
            debris_k: r.f(),
            lower_head_failed: r.f() != 0.0,
            ex_vessel_w: r.f(),
            clad_k: r.vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The axial shape sums to one and peaks mid-core.
    #[test]
    fn the_axial_shape_is_normalised_and_peaked() {
        let a = axial_shape(&Params::default());
        assert!((a.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!(a[AXIAL_NODES / 2] > a[0] && a[AXIAL_NODES / 2] > a[AXIAL_NODES - 1]);
    }

    /// A covered core at decay power puts all of it into the water (energy
    /// conservation of the node balance at steady state) and stays within a
    /// few kelvin of saturation.
    #[test]
    fn a_covered_core_hands_its_decay_heat_to_the_water() {
        let mut c = Core::new(560.0, &Params::default());
        let s = CoreSnapshot { dt_s: 1.0, p_pa: 7.0e6, t_sat_k: 560.0, h_fg: 1.5e6, level_m: 8.0, decay_w: 20.0e6, t_dependent_props: false, prm: Params::default() };
        let mut r = c.step(&s);
        for _ in 0..3000 {
            r = c.step(&s);
        }
        assert!((r.q_fluid_w - 20.0e6).abs() < 20.0e6 * 1e-6, "{}", r.q_fluid_w);
        assert!(r.peak_clad_k < 562.0 && r.h2_kg_s == 0.0, "{}", r.peak_clad_k);
    }

    /// An uncovered core with no steam heats adiabatically: the energy it
    /// gains is the decay power times the time (no oxidation without steam).
    #[test]
    fn a_dry_core_without_steam_heats_adiabatically() {
        let mut c = Core::new(560.0, &Params::default());
        // Start from equilibrium at saturation so the stored heat is gone.
        for n in c.nodes.iter_mut() {
            n.t_fuel = 560.0;
            n.t_clad = 560.0;
        }
        let g = geom(&Params::default());
        let e0: f64 = c.nodes.iter().map(|n| g.c_fuel * n.t_fuel + g.c_clad * n.t_clad).sum();
        let s = CoreSnapshot { dt_s: 1.0, p_pa: 7.0e6, t_sat_k: 560.0, h_fg: 1.5e6, level_m: -1.0, decay_w: 10.0e6, t_dependent_props: false, prm: Params::default() };
        for _ in 0..100 {
            let r = c.step(&s);
            assert_eq!(r.h2_kg_s, 0.0);
        }
        let e1: f64 = c.nodes.iter().map(|n| g.c_fuel * n.t_fuel + g.c_clad * n.t_clad).sum();
        assert!(((e1 - e0) - 10.0e6 * 100.0).abs() < 1e-6 * 1e9, "{}", e1 - e0);
    }

    /// The same dry, steamless heat-up with temperature-dependent heat
    /// capacities: the core's enthalpy (from the MATPRO tables) rises by
    /// exactly the decay power times the time. The enthalpy update and its
    /// inversion are what make this exact; a temperature-only update with
    /// varying `c_p` would not be.
    #[test]
    fn a_dry_core_conserves_enthalpy_with_temperature_dependent_properties() {
        let mut c = Core::new(560.0, &Params::default());
        for n in c.nodes.iter_mut() {
            n.t_fuel = 560.0;
            n.t_clad = 560.0;
        }
        let g = geom(&Params::default());
        let h = |c: &Core| -> f64 {
            c.nodes.iter().map(|n| g.m_fuel * uo2_enthalpy().at(n.t_fuel) + g.zr_kg * zr_enthalpy().at(n.t_clad)).sum()
        };
        let e0 = h(&c);
        let s = CoreSnapshot { dt_s: 1.0, p_pa: 7.0e6, t_sat_k: 560.0, h_fg: 1.5e6, level_m: -1.0, decay_w: 10.0e6, t_dependent_props: true, prm: Params::default() };
        for _ in 0..100 {
            c.step(&s);
        }
        let gained = h(&c) - e0;
        // Bound: the table inversion is linear between 1 K points, so the
        // round trip is exact to round-off; 1e-9 relative.
        assert!((gained - 1.0e9).abs() < 1e-9 * 1.0e9 * 10.0, "{gained}");
    }

    /// MATPRO's heat capacities as reused: positive over the core's range,
    /// UO2 rising with temperature, and the enthalpy table inverting.
    #[test]
    fn the_matpro_tables_are_positive_and_invert() {
        for t in (300..3100).step_by(50) {
            let t = t as f64;
            assert!(cp_matpro(HeatCapacityModel::MatproUo2, t) > 0.0);
            assert!(cp_matpro(HeatCapacityModel::MatproZircaloy, t) > 0.0);
            for e in [uo2_enthalpy(), zr_enthalpy()] {
                assert!((e.temperature(e.at(t)) - t).abs() < 1e-9, "{t}");
            }
        }
        assert!(cp_matpro(HeatCapacityModel::MatproUo2, 1500.0) > cp_matpro(HeatCapacityModel::MatproUo2, 600.0));
        let t = debris_temperature(2.0, 1.0, 2.0 * uo2_enthalpy().at(1400.0) + zr_enthalpy().at(1400.0));
        assert!((t - 1400.0).abs() < 1e-6, "{t}");
    }

    /// The oxidation rate is continuous across its branches and rises with T.
    #[test]
    fn the_oxidation_rate_is_continuous_and_rising() {
        let mut last = 0.0;
        for t in (1000..2500).step_by(5) {
            let k = oxidation_rate(&Params::default(), t as f64);
            assert!(k >= last, "{t}");
            last = k;
        }
        let a = oxidation_rate(&Params::default(), UH_LOW_MAX_K);
        let b = oxidation_rate(&Params::default(), UH_LOW_MAX_K + 1e-9);
        assert!((a - b).abs() < 1e-9 * a);
    }
}
