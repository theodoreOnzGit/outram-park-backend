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
//! physics; temperature-dependent fuel properties; the decay heat that
//! leaves with volatile fission products; boron carbide control-blade
//! reactions. Debris after the lower head fails leaves the model.

use super::params::*;
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
    g_fc: f64,
    a_rod: f64,
    a_zr: f64,
    zr_kg: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Core {
    nodes: Vec<Node>,
    debris_c: f64,
    debris_e: f64,
    debris_power_share: f64,
    lower_head_failed: bool,
    zr_oxidised_kg: f64,
    h2_total_kg: f64,
}

fn geom() -> NodeGeom {
    let n = NODES as f64;
    let dz = ACTIVE_LENGTH_M / AXIAL_NODES as f64;
    let rod_length = ASSEMBLIES * RODS_PER_ASSEMBLY * dz / RINGS as f64;
    NodeGeom {
        c_fuel: core_uo2_kg() / n * UO2_CP,
        c_clad: core_zr_kg() / n * ZR_CP,
        g_fc: fuel_to_clad_w_per_m_k() * rod_length,
        a_rod: core_rod_area_m2() / n,
        a_zr: core_zr_area_m2() / n,
        zr_kg: core_zr_kg() / n,
    }
}

/// Chopped-cosine axial shape, normalised to sum to 1.
fn axial_shape() -> Vec<f64> {
    let l = ACTIVE_LENGTH_M;
    let le = COSINE_EXTRAPOLATION * l;
    let raw: Vec<f64> = (0..AXIAL_NODES)
        .map(|j| {
            let z = (j as f64 + 0.5) / AXIAL_NODES as f64 * l;
            (std::f64::consts::PI * (z - 0.5 * l) / le).cos()
        })
        .collect();
    let s: f64 = raw.iter().sum();
    raw.iter().map(|r| r / s).collect()
}

/// Urbanic-Heidrick parabolic rate constant, (kg Zr/m^2)^2/s.
pub fn oxidation_rate(t_k: f64) -> f64 {
    if t_k < OXIDATION_MIN_K {
        return 0.0;
    }
    let low = |t: f64| UH_LOW_A * (-UH_LOW_B_K / t).exp();
    let high = |t: f64| UH_HIGH_A * (-UH_HIGH_B_K / t).exp();
    if t_k <= UH_LOW_MAX_K {
        low(t_k)
    } else if t_k >= UH_HIGH_MIN_K {
        high(t_k)
    } else {
        let w = (t_k - UH_LOW_MAX_K) / (UH_HIGH_MIN_K - UH_LOW_MAX_K);
        low(UH_LOW_MAX_K) + w * (high(UH_HIGH_MIN_K) - low(UH_LOW_MAX_K))
    }
}

/// Dittus-Boelter heat transfer coefficient for steam at `t_film` and `p`,
/// for a ring's steam flow `w` (kg/s) through its share of the flow area.
fn steam_htc(w: f64, t_film: f64, p_pa: f64) -> (f64, f64) {
    let t = ThermodynamicTemperature::new::<kelvin>(t_film.clamp(300.0, 2200.0));
    let p = Pressure::new::<pascal>(p_pa);
    let cp = cp_tp_eqm_single_phase(t, p).get::<joule_per_kilogram_kelvin>();
    let k = lambda_tp_eqm_single_phase(t, p).get::<watt_per_meter_kelvin>();
    let mu = mu_tp_eqm_single_phase(t, p).get::<pascal_second>();
    let dh = hydraulic_diameter_m();
    let area = core_flow_area_m2() / RINGS as f64;
    let re = (w / area) * dh / mu;
    let pr = mu * cp / k;
    // Laminar floor Nu = 4.36 (fully developed, uniform flux).
    let nu = (0.023 * re.powf(0.8) * pr.powf(0.4)).max(4.36);
    (nu * k / dh, cp)
}

impl Core {
    /// The core just after the scram: fuel at its full-power average
    /// temperature over the coolant, so the stored heat is in it.
    pub fn new(t_sat_k: f64) -> Self {
        let g = geom();
        let axial = axial_shape();
        let mut nodes = Vec::with_capacity(NODES);
        for ring in 0..RINGS {
            for j in 0..AXIAL_NODES {
                let q = THERMAL_POWER_W / RINGS as f64 * RING_FACTORS[ring] * axial[j];
                let t_clad = t_sat_k + q / (BOILING_H * g.a_rod);
                nodes.push(Node { t_fuel: t_clad + q / g.g_fc, t_clad, w_ox: 0.0, relocated: false });
            }
        }
        Self {
            nodes,
            debris_c: 0.0,
            debris_e: 0.0,
            debris_power_share: 0.0,
            lower_head_failed: false,
            zr_oxidised_kg: 0.0,
            h2_total_kg: 0.0,
        }
    }

    /// Advance one step. Pure function of `self` and `s`.
    pub fn step(&mut self, s: &CoreSnapshot) -> CoreResult {
        let g = geom();
        let axial = axial_shape();
        let dt = s.dt_s;
        let dz = ACTIVE_LENGTH_M / AXIAL_NODES as f64;
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
        if self.debris_c > 0.0 {
            if self.lower_head_failed {
                ex_vessel = p_debris;
            } else if s.level_m > -BAF_M {
                let excess = (self.debris_e - self.debris_c * s.t_sat_k).max(0.0);
                let release = excess * (1.0 - (-dt / DEBRIS_QUENCH_S).exp());
                self.debris_e -= release;
                q_quench = p_debris + release / dt;
            } else {
                self.debris_e += p_debris * dt;
                if self.debris_e / self.debris_c > LOWER_HEAD_FAILURE_K {
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
                let q = s.decay_w / RINGS as f64 * RING_FACTORS[ring] * axial[j];
                let z_mid = (j as f64 + 0.5) * dz;
                let covered = s.level_m >= z_mid;
                // Oxidation (explicit in T, uncovered only, steam-limited).
                let mut q_ox = 0.0;
                if !covered {
                    let k = oxidation_rate(node.t_clad);
                    if k > 0.0 && w_steam > 0.0 {
                        let w_max = g.zr_kg / g.a_zr;
                        let w_new = (node.w_ox * node.w_ox + k * dt).sqrt().min(w_max);
                        let mut dzr = (w_new - node.w_ox) * g.a_zr;
                        let steam_per_zr = 2.0 * M_H2O / M_ZR;
                        let dzr_steam = w_steam * dt / steam_per_zr;
                        dzr = dzr.min(dzr_steam);
                        let n = &mut self.nodes[idx];
                        n.w_ox += dzr / g.a_zr;
                        q_ox = dzr * OXIDATION_HEAT_J_PER_KG_ZR / dt;
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
                    (BOILING_H * g.a_rod, s.t_sat_k, 0.0)
                } else if w_steam > 0.0 {
                    let (h, cp) = steam_htc(w_steam, 0.5 * (node.t_clad + t_steam), s.p_pa);
                    let ntu = h * g.a_rod / (w_steam * cp);
                    (w_steam * cp * (1.0 - (-ntu).exp()), t_steam, cp)
                } else {
                    (0.0, t_steam, 0.0)
                };
                // Implicit two-lump update:
                //   cf (Tf' - Tf) = dt [q - G (Tf' - Tc')]
                //   cc (Tc' - Tc) = dt [G (Tf' - Tc') - hA (Tc' - Tcool) + q_ox]
                let n = self.nodes[idx];
                let (cf, cc, gfc) = (g.c_fuel, g.c_clad, g.g_fc);
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
                let m = &mut self.nodes[idx];
                m.t_fuel = tf;
                m.t_clad = tc;
                peak = peak.max(tc);
                if tc >= ZR_MELT_K {
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
            self.debris_c += g.c_fuel + g.c_clad;
            self.debris_e += g.c_fuel * n.t_fuel + g.c_clad * n.t_clad;
            self.debris_power_share += share;
        }
        let debris_k = if self.debris_c > 0.0 { self.debris_e / self.debris_c } else { s.t_sat_k };
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
        w.f(&[self.dt_s, self.p_pa, self.t_sat_k, self.h_fg, self.level_m, self.decay_w]);
    }
    pub fn read(r: &mut Reader) -> Self {
        Self { dt_s: r.f(), p_pa: r.f(), t_sat_k: r.f(), h_fg: r.f(), level_m: r.f(), decay_w: r.f() }
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
        let a = axial_shape();
        assert!((a.iter().sum::<f64>() - 1.0).abs() < 1e-12);
        assert!(a[AXIAL_NODES / 2] > a[0] && a[AXIAL_NODES / 2] > a[AXIAL_NODES - 1]);
    }

    /// A covered core at decay power puts all of it into the water (energy
    /// conservation of the node balance at steady state) and stays within a
    /// few kelvin of saturation.
    #[test]
    fn a_covered_core_hands_its_decay_heat_to_the_water() {
        let mut c = Core::new(560.0);
        let s = CoreSnapshot { dt_s: 1.0, p_pa: 7.0e6, t_sat_k: 560.0, h_fg: 1.5e6, level_m: 8.0, decay_w: 20.0e6 };
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
        let mut c = Core::new(560.0);
        // Start from equilibrium at saturation so the stored heat is gone.
        for n in c.nodes.iter_mut() {
            n.t_fuel = 560.0;
            n.t_clad = 560.0;
        }
        let g = geom();
        let e0: f64 = c.nodes.iter().map(|n| g.c_fuel * n.t_fuel + g.c_clad * n.t_clad).sum();
        let s = CoreSnapshot { dt_s: 1.0, p_pa: 7.0e6, t_sat_k: 560.0, h_fg: 1.5e6, level_m: -1.0, decay_w: 10.0e6 };
        for _ in 0..100 {
            let r = c.step(&s);
            assert_eq!(r.h2_kg_s, 0.0);
        }
        let e1: f64 = c.nodes.iter().map(|n| g.c_fuel * n.t_fuel + g.c_clad * n.t_clad).sum();
        assert!(((e1 - e0) - 10.0e6 * 100.0).abs() < 1e-6 * 1e9, "{}", e1 - e0);
    }

    /// The oxidation rate is continuous across its branches and rises with T.
    #[test]
    fn the_oxidation_rate_is_continuous_and_rising() {
        let mut last = 0.0;
        for t in (1000..2500).step_by(5) {
            let k = oxidation_rate(t as f64);
            assert!(k >= last, "{t}");
            last = k;
        }
        let a = oxidation_rate(UH_LOW_MAX_K);
        let b = oxidation_rate(UH_LOW_MAX_K + 1e-9);
        assert!((a - b).abs() < 1e-9 * a);
    }
}
