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
//!   temperature found by inverting the correlation's enthalpy (which since
//!   gh:#1035 includes the heats of fusion, so a node pays 225 kJ per kg of
//!   Zr at 2098 K), so energy is
//!   conserved exactly. `Mechanisms::without_temperature_dependent_properties`
//!   is the ablation (the constant `UO2_CP`/`ZR_CP` path, unchanged);
//! - oxidation by the parabolic Urbanic-Heidrick law above ~~1000 K~~ 1100 K (MELCOR's TOXMIN), its heat
//!   into the cladding, limited by the steam that reaches the node (steam
//!   starvation) and, since 2026-10-29, by MELCOR's gaseous diffusion of
//!   that steam through the hydrogen (`diffusion_limited_rate`); the steam it uses and the hydrogen it makes travel up.
//!   The heat is Hess's law at the node (gh:#1046): the 298 K heat of
//!   reaction, plus the consumed steam's enthalpy above 298 K vapour, minus
//!   heating the new hydrogen to the stream, which then carries it (steam
//!   plus hydrogen heat capacity). A steam-starved node does not heat the
//!   hydrogen passing it;
//! - a node collapses by MELCOR's criteria (gh:#1035): once its metal is
//!   gone (oxidised below [`MIN_METAL_M`], or its shell breached at
//!   [`CLAD_BREACH_K`]) it accumulates time-at-temperature damage and falls
//!   at [`ROD_COLLAPSE_K`] or when the damage reaches 1; any rod falls at
//!   [`ROD_COLLAPSE_MAX_K`]. ~~Molten Zr that MELCOR would candle downward
//!   stays in the node and keeps oxidising (stated simplification).~~
//!   **Since 2026-10-29** molten Zr candles: once the shell breaches it
//!   flows down and refreezes in the first node below Zr's melting point
//!   (`candling`; simplified, see the candling pass). ~~a
//!   node whose cladding reaches [`ZR_MELT_K`] relocates~~ (that is the
//!   `without_rod_collapse` ablation). A fallen node relocates: its mass, energy
//!   and decay power move to a debris lump in the lower plenum, quenched
//!   while water is there; ~~dry debris above [`LOWER_HEAD_FAILURE_K`] fails
//!   the lower head~~ (since gh:#1034 that is only the
//!   `without_lower_head_wall` ablation): dry debris heats the lower-head
//!   wall (`HeadWall`), which fails on its inner node reaching
//!   [`HEAD_FAILURE_WALL_K`] or by Larson-Miller creep.
//!
//! **Not in the model** (each would change the answer): ~~radiation between
//! rods, to the shroud and upwards (it spreads heat across the core above
//! about 1200 K);~~ (added 2026-10-29, gh:#1037: ring-to-ring, level-to-level
//! and outer ring to a shroud lump; still missing: the upper-plenum
//! structures, the view through emptied cells, the downcomer side of the
//! shroud); the two-phase swell of the mixture level (the collapsed
//! level uncovers the fuel early); ~~candling,~~ (simplified candling added
//! 2026-10-29) blockages, and molten-pool physics; ~~temperature-dependent fuel properties~~ (heat capacities added
//! 2026-10-29, gh:#1038; UO2 conductivity is still constant, waiting for a
//! cited Unit 1 burnup, which MATPRO's conductivity depends on); the decay heat that
//! leaves with volatile fission products; boron carbide control-blade
//! reactions. Debris after the lower head fails leaves the model.

use petir::mathf::RealMath;
use super::params::*;
use outram_park_fork_offbeat::materials::properties::heat_capacity::HeatCapacityModel;
use outram_park_fork_offbeat::materials::MaterialState;
use tuas_boussinesq_solver::boussinesq_thermophysical_properties::solid_database::ss_304_l_high_temp as ss304;
use uom::si::f64::AvailableEnergy;
use uom::si::mass_density::kilogram_per_cubic_meter;
use std::sync::OnceLock;
use super::gas::{cp_h2_per_kg, h2_enthalpy_above_298};
use super::wire::{Reader, Writer};
use tampines_steam_tables::dynamic_viscosity::mu_tp_eqm_single_phase;
use tampines_steam_tables::prelude::{cp_tp_eqm_single_phase, h_tp_eqm_single_phase};
use uom::si::available_energy::joule_per_kilogram;
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
    /// Drywell pressure, Pa: the outside of the lower head, for its creep
    /// stress (gh:#1034).
    pub p_dw_pa: f64,
    /// The lower-head wall on (the default) or the debris-temperature
    /// ablation (`Mechanisms`).
    pub lower_head_wall: bool,
    /// MELCOR's rod-collapse criteria (the default) or the 2100 K
    /// whole-node slump (`Mechanisms`).
    pub rod_collapse: bool,
    /// In-core radiation and the shroud (the default) or none
    /// (`Mechanisms`).
    pub core_radiation: bool,
    /// MELCOR's gaseous-diffusion limit on oxidation (the default) or the
    /// parabolic rate limited only by steam supply (`Mechanisms`).
    pub diffusion_limit: bool,
    /// Hydrogen's `c_p(T)` from MELCOR's fit (the default) or the constant
    /// [`CP_H2_J_PER_KG_K`] (`Mechanisms`).
    pub hydrogen_cp_t: bool,
    /// Lower-head penetration failure (the default) or the wall alone
    /// (`Mechanisms`).
    pub penetrations: bool,
    /// Oxidation of the lower-plenum debris (`Mechanisms::debris_oxidation`).
    pub debris_oxidation: bool,
    /// Molten Zr candles down the rods (`Mechanisms::candling`).
    pub candling: bool,
    /// MELCOR's falling-debris quench and dryout limit
    /// (`Mechanisms::debris_quench`).
    pub debris_quench: bool,
    /// The lower head takes heat from the debris under water too
    /// (`Mechanisms::wet_head_contact`).
    pub wet_head_contact: bool,
    /// The penetration heated over the debris-covered part of its height
    /// only, the rest cooled by the fluid (`Mechanisms::penetration_coverage`).
    pub penetration_coverage: bool,
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
    /// Steam leaving the top of the core, kg/s (with the hydrogen,
    /// `h2_kg_s`, at `exit_gas_k`).
    pub exit_steam_kg_s: f64,
    /// Volatile fission products (Xe, Cs, I) released from the fuel so
    /// far, fraction of the core inventory (gh:#1040).
    pub fp_volatile_released: f64,
    /// UO2 released from the fuel so far, kg: the seed aerosol (gh:#1047).
    pub uo2_released_kg: f64,
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
    /// The lower-head wall's inner temperature, K, and its creep strain
    /// (gh:#1034; zero strain and the initial temperature until debris
    /// arrives, and in the ablation).
    pub head_inner_k: f64,
    pub head_strain: f64,
    /// Heat into the water this step, W: covered nodes and the debris
    /// quench. The vapour it raises inside the pool sets the swell of the
    /// mixture level (gh:#1036).
    pub q_pool_w: f64,
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
    /// Time-at-temperature damage of the rod, `sum dt/L` (gh:#1035).
    damage: f64,
    /// Fraction of the node's volatile fission products (Xe, Cs, I) still
    /// in its fuel (gh:#1040, `volatile_release_rate`).
    fp_left: f64,
    /// Fraction of the node's UO2 still in its fuel (CORSOR-M's UO2 row;
    /// the aerosol seed, gh:#1047).
    uo2_left: f64,
    /// Zr in the node, kg, metal and oxide counted as Zr: the geometry's
    /// share, less what has candled away, plus what has refrozen here
    /// (`candling`).
    zr_kg: f64,
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
    head: HeadWall,
    /// The core shroud, one steel lump per axial level, K (gh:#1037).
    shroud_k: [f64; AXIAL_NODES],
    /// Volatile fission products in the lower-plenum debris, and released
    /// from the fuel so far, as fractions of the core's inventory
    /// (gh:#1040).
    debris_fp: f64,
    fp_released: f64,
    /// UO2 in the in-vessel debris and released so far, fractions of the
    /// core's UO2.
    debris_uo2: f64,
    /// Zr in the lower-plenum debris already oxidised, kg (as Zr): the
    /// relocated nodes' oxide, and what the debris oxidises since
    /// (`debris_oxidation`).
    debris_zr_ox: f64,
    /// MELCOR's quench decay factor (`debris_quench`): 1 while debris is
    /// arriving, decaying with `quench_tau_s` once it stops.
    quench_f: f64,
    uo2_released: f64,
}

/// Through-wall nodes of the lower-head wall (inner face first).
pub const WALL_NODES: usize = 5;

/// The lower head under the debris (gh:#1034), after the public description
/// of MELCOR's COR lower-head model (MELCOR 2.2 Vol. 2, §6.1 and §6.3): one
/// representative 1-D column of the wall, `WALL_NODES` nodes through
/// `lower_head_thickness_m` of Type 304L steel (Kim, ANL-75-55, via tuas),
/// explicit conduction, the debris on the inner face through
/// `debris_to_head_w_m2_k` (water instead while the plenum is wet), the
/// outer face adiabatic. It fails when the inner node reaches
/// `head_failure_wall_k` or its Larson-Miller creep strain reaches
/// `creep_fail_strain`.
///
/// Simplified, and stated: one column stands for the whole wetted wall (the
/// wall the debris spreads onto later is taken as already that hot); the
/// outer face loses nothing (MELCOR's dry-cavity default is 10 W/(m^2 K),
/// about 4 % of the debris power here); no penetrations; the wall stays at
/// its initial temperature until debris arrives; no wall melting.
#[derive(Clone, Copy, Debug, PartialEq)]
struct HeadWall {
    t_k: [f64; WALL_NODES],
    strain: f64,
    /// A lower-head penetration (instrument or control-rod-drive housing),
    /// one lumped node heated by the same medium as the wall's inner face,
    /// K (gh:#1034, MELCOR's penetration failure).
    pen_k: f64,
}

pub(crate) fn steel_h(t_k: f64) -> f64 {
    ss304::steel_304_l_high_temp_specific_enthalpy_kim(ThermodynamicTemperature::new::<kelvin>(t_k)).get::<joule_per_kilogram>()
}

pub(crate) fn steel_t(h: f64) -> f64 {
    ss304::steel_304_l_high_temp_temp_from_specific_enthalpy_kim(AvailableEnergy::new::<joule_per_kilogram>(h)).get::<kelvin>()
}

/// Thermal conductivity, W/(m K), held at the correlation's 1700 K ceiling
/// above it (the wall fails well below).
fn steel_k(t_k: f64) -> f64 {
    let t = ThermodynamicTemperature::new::<kelvin>(t_k.min(1700.0));
    ss304::steel_304_l_high_temp_thermal_conductivity_kim(t).map_or(f64::NAN, |k| k.get::<watt_per_meter_kelvin>())
}

/// Density at 300 K, kg/m^3: the wall's mass is fixed at it.
pub(crate) fn steel_rho() -> f64 {
    ss304::steel_304_l_high_temp_density_kim(ThermodynamicTemperature::new::<kelvin>(300.0)).map_or(f64::NAN, |r| r.get::<kilogram_per_cubic_meter>())
}

/// The debris bed in the hemispherical head: the wall area it touches, m^2,
/// its height, m, and its bulk density, kg/m^3. A spherical cap of radius
/// `rpv_radius_m` holding the UO2 and Zr at their densities with
/// `debris_porosity`; full when it would overflow the hemisphere.
fn debris_cap(prm: &Params, mf: f64, mc: f64) -> (f64, f64, f64) {
    let r = prm.rpv_radius_m;
    let v = (mf / prm.uo2_density + mc / prm.zr_density) / (1.0 - prm.debris_porosity);
    let cap = |h: f64| std::f64::consts::PI * h * h * (3.0 * r - h) / 3.0;
    let (mut a, mut b) = (0.0, r);
    if cap(b) <= v {
        a = r;
    } else {
        for _ in 0..60 {
            let m = 0.5 * (a + b);
            if cap(m) > v {
                b = m;
            } else {
                a = m;
            }
        }
    }
    let h = if a == r { r } else { 0.5 * (a + b) };
    (2.0 * std::f64::consts::PI * r * h, h, if v > 0.0 { (mf + mc) / v } else { 0.0 })
}

impl HeadWall {
    fn new(t_k: f64) -> Self {
        Self { t_k: [t_k; WALL_NODES], strain: 0.0, pen_k: t_k }
    }

    /// Advance the wall one step with its inner face exchanging with a
    /// medium at `t_in` through `h_in`, W/(m^2 K), over `area`; returns the
    /// heat taken from that medium, W.
    fn step(&mut self, prm: &Params, t_in: f64, h_in: f64, area: f64, dt: f64) -> f64 {
        self.step_with(prm, t_in, h_in, area, dt, None)
    }

    /// [`Self::step`] with the penetration's own boundary (MELCOR COR §6.1,
    /// `penetration_coverage`): `Some((cover, t_w, h_w))` heats it from the
    /// medium over the fraction `cover` of its height that the debris
    /// covers (MELCOR's `dz_d / dz_1`) and cools the rest to the fluid at
    /// `t_w` through `h_w` (zero for a dry plenum). `None` is the medium
    /// over all of it.
    fn step_with(&mut self, prm: &Params, t_in: f64, h_in: f64, area: f64, dt: f64, pen: Option<(f64, f64, f64)>) -> f64 {
        let dx = prm.lower_head_thickness_m / (WALL_NODES - 1) as f64;
        let rho = steel_rho();
        let q_in = h_in * (t_in - self.t_k[0]);
        let mut flux = [0.0; WALL_NODES];
        flux[0] += q_in;
        for i in 0..WALL_NODES - 1 {
            let k = 0.5 * (steel_k(self.t_k[i]) + steel_k(self.t_k[i + 1]));
            let f = k * (self.t_k[i] - self.t_k[i + 1]) / dx;
            flux[i] -= f;
            flux[i + 1] += f;
        }
        for i in 0..WALL_NODES {
            let m = rho * dx * if i == 0 || i == WALL_NODES - 1 { 0.5 } else { 1.0 };
            self.t_k[i] = steel_t(steel_h(self.t_k[i]) + flux[i] * dt / m);
        }
        // The penetration: per unit area, its wall's steel, heated from the
        // medium alone (implicit at the start-of-step heat capacity, then
        // set from its enthalpy). Its heat is not taken from the medium:
        // the penetrations' area is small beside the head's (stated).
        let m_pen = rho * prm.penetration_wall_m;
        let c_pen = m_pen * (steel_h(self.pen_k + 0.5) - steel_h(self.pen_k - 0.5));
        match pen {
            None => {
                let t_new = (c_pen * self.pen_k + dt * h_in * t_in) / (c_pen + dt * h_in);
                self.pen_k = steel_t(steel_h(self.pen_k) + h_in * (t_in - t_new) * dt / m_pen);
            }
            Some((cover, t_w, h_w)) => {
                let (a, b) = (cover.clamp(0.0, 1.0) * h_in, (1.0 - cover.clamp(0.0, 1.0)) * h_w);
                let t_new = (c_pen * self.pen_k + dt * (a * t_in + b * t_w)) / (c_pen + dt * (a + b));
                self.pen_k = steel_t(steel_h(self.pen_k) + (a * (t_in - t_new) + b * (t_w - t_new)) * dt / m_pen);
            }
        }
        q_in * area
    }

    /// Accumulate Larson-Miller creep strain over `dt` under a pressure
    /// difference `dp` plus the debris's own head, Pa, and say whether the
    /// head has failed (inner node or strain). Stress from the thick-sphere
    /// relation MELCOR's 0-D option uses (Vol. 2, Eq 6-26); temperature the
    /// wall's mean; skipped below 1000 Pa effective (its C1600(3)). With
    /// `penetrations`, a penetration at the same temperature limit fails it
    /// too, as in MELCOR (Vol. 2, §6.3: a penetration or the innermost head
    /// node at TPFAIL).
    fn failed(&mut self, prm: &Params, dp: f64, dt: f64, penetrations: bool) -> bool {
        let (ri, ro) = (prm.rpv_radius_m, prm.rpv_radius_m + prm.lower_head_thickness_m);
        if dp >= 1000.0 {
            let sigma = dp * ri * ri / (ro * ro - ri * ri);
            let t_mean = self.t_k.iter().sum::<f64>() / WALL_NODES as f64;
            let p_lm = prm.lm_p0 - prm.lm_slope * sigma.r_log10();
            let t_rupture = ((p_lm / t_mean - prm.lm_c) * std::f64::consts::LN_10).r_exp();
            self.strain += prm.creep_fail_strain * dt / t_rupture;
        }
        self.t_k[0] >= prm.head_failure_wall_k || self.strain >= prm.creep_fail_strain || (penetrations && self.pen_k >= prm.head_failure_wall_k)
    }
}

/// Zircaloy emissivity against oxide thickness, after MELCOR's C1104 (Vol.
/// 2, COR §2.1, Eqs 2-7 to 2-10): 0.325 bare, 0.808 at 3.88 um, 0.759 at
/// 1 mm and beyond, linear between; above 1500 K multiplied by
/// `exp((1500 - T)/300)`, no lower than 0.325. MELCOR degrades on the
/// highest temperature the surface has reached; this uses the present one
/// (stated simplification).
fn zr_emissivity(oxide_m: f64, t_k: f64) -> f64 {
    let e = if oxide_m <= 3.88e-6 {
        0.325 + (0.808 - 0.325) * oxide_m / 3.88e-6
    } else if oxide_m <= 1.0e-3 {
        0.808 + (0.759 - 0.808) * (oxide_m - 3.88e-6) / (1.0e-3 - 3.88e-6)
    } else {
        0.759
    };
    let e = if t_k > 1500.0 { (e * ((1500.0 - t_k) / 300.0).r_exp()).max(0.325) } else { e };
    e.clamp(1.0e-4, 0.9999)
}

/// Steel emissivity, after MELCOR's C1102 (Vol. 2, COR §2.1, Eq. 2-11).
fn steel_emissivity(t_k: f64) -> f64 {
    (0.25617 + 3.474e-4 * (t_k - 616.48)).clamp(1.0e-4, 0.9999)
}

/// Gray-body exchange between two surfaces, W: `F A sigma (T1^4 - T2^4)
/// / (1/e1 + 1/e2 - 1)`.
fn gray_exchange(f_a: f64, e1: f64, e2: f64, t1: f64, t2: f64) -> f64 {
    const SIGMA: f64 = 5.670_374_419e-8;
    f_a * SIGMA * (t1.r_powi(4) - t2.r_powi(4)) / (1.0 / e1 + 1.0 / e2 - 1.0)
}

/// The core's equivalent radius, m: the assemblies' lattice area as a
/// circle. The rings hold equal numbers of rods, so equal areas.
fn core_radius(prm: &Params) -> f64 {
    (prm.assemblies * prm.assembly_pitch_m * prm.assembly_pitch_m / std::f64::consts::PI).sqrt()
}

/// Steel in one axial level of the shroud, kg (304L at 300 K, the wall's).
fn shroud_mass_per_level(prm: &Params, dz: f64) -> f64 {
    2.0 * std::f64::consts::PI * core_radius(prm) * dz * prm.shroud_thickness_m * steel_rho()
}

impl Core {
    /// Radiative sources, W: on each node's cladding, and on each shroud
    /// level (gh:#1037). Radial between neighbouring rings across their
    /// boundary cylinder, axial between neighbouring levels across the
    /// ring's cross-section, and from the outer ring to the shroud, each
    /// with its exchange factor. A relocated node takes no part: there is
    /// no view across an emptied cell (stated simplification; MELCOR looks
    /// through it to the next surface). The top and bottom of the core
    /// exchange nothing (no upper-plenum structure or pool surface).
    fn radiation(&self, prm: &Params, dz: f64) -> ([f64; NODES], [f64; AXIAL_NODES]) {
        let mut q = [0.0; NODES];
        let mut q_sh = [0.0; AXIAL_NODES];
        let g = geom(prm);
        let rc = core_radius(prm);
        let oxide = |n: &Node| n.w_ox.min(n.zr_kg / g.a_zr) * (ZRO2_PER_ZR / prm.zro2_density);
        let eps = |n: &Node| zr_emissivity(oxide(n), n.t_clad);
        let pi = std::f64::consts::PI;
        for j in 0..AXIAL_NODES {
            for k in 0..RINGS {
                let a = &self.nodes[k * AXIAL_NODES + j];
                if a.relocated {
                    continue;
                }
                // Radially outward: the next ring, or the shroud.
                let r_out = rc * ((k + 1) as f64 / RINGS as f64).sqrt();
                let area = 2.0 * pi * r_out * dz * prm.radial_exchange;
                if k + 1 < RINGS {
                    let b = &self.nodes[(k + 1) * AXIAL_NODES + j];
                    if !b.relocated {
                        let x = gray_exchange(area, eps(a), eps(b), a.t_clad, b.t_clad);
                        q[k * AXIAL_NODES + j] -= x;
                        q[(k + 1) * AXIAL_NODES + j] += x;
                    }
                } else {
                    let t_sh = self.shroud_k[j];
                    let x = gray_exchange(area, eps(a), steel_emissivity(t_sh), a.t_clad, t_sh);
                    q[k * AXIAL_NODES + j] -= x;
                    q_sh[j] += x;
                }
                // Axially upward.
                if j + 1 < AXIAL_NODES {
                    let b = &self.nodes[k * AXIAL_NODES + j + 1];
                    if !b.relocated {
                        let area = pi * rc * rc / RINGS as f64 * prm.axial_exchange;
                        let x = gray_exchange(area, eps(a), eps(b), a.t_clad, b.t_clad);
                        q[k * AXIAL_NODES + j] -= x;
                        q[k * AXIAL_NODES + j + 1] += x;
                    }
                }
            }
        }
        (q, q_sh)
    }
}

/// kg of ZrO2 per kg of Zr oxidised (123.22 / 91.224 g/mol).
const ZRO2_PER_ZR: f64 = 123.222 / 91.224;

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
    /// The table for `model`, with its heat of fusion `(melting point K,
    /// J/kg)` added across the one-kelvin step that starts at the melting
    /// point, so h(T) still rises strictly and inverts.
    fn new(model: HeatCapacityModel, fusion: (f64, f64)) -> Self {
        let n = ((H_TOP_K - H_REF_K) / H_STEP_K) as usize + 1;
        let mut h = Vec::with_capacity(n);
        h.push(0.0);
        for i in 1..n {
            let (t0, t1) = (H_REF_K + (i - 1) as f64 * H_STEP_K, H_REF_K + i as f64 * H_STEP_K);
            let melt = if t0 <= fusion.0 && fusion.0 < t1 { fusion.1 } else { 0.0 };
            h.push(h[i - 1] + 0.5 * (cp_matpro(model, t0) + cp_matpro(model, t1)) * H_STEP_K + melt);
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

/// Melting points, K, and heats of fusion, J/kg (gh:#1035): MELCOR 2.2
/// Vol. 2, MP package Tables 7 and 8 (MP-RM-49/50), which cite MATPRO for
/// both. Material properties, not plant inputs, so constants rather than
/// [`Params`]. Before 2026-10-29 the tables had no heat of fusion: a node
/// crossed 2098 K without paying it.
const ZR_FUSION: (f64, f64) = (2098.0, 2.25e5);
/// ZrO2's melting point, K: the same MP Table 7 (MP-RM-49).
const ZRO2_MELT_K: f64 = 2990.0;
const UO2_FUSION: (f64, f64) = (3113.0, 2.74e5);

fn uo2_enthalpy() -> &'static Enthalpy {
    static T: OnceLock<Enthalpy> = OnceLock::new();
    T.get_or_init(|| Enthalpy::new(HeatCapacityModel::MatproUo2, UO2_FUSION))
}

fn zr_enthalpy() -> &'static Enthalpy {
    static T: OnceLock<Enthalpy> = OnceLock::new();
    T.get_or_init(|| Enthalpy::new(HeatCapacityModel::MatproZircaloy, ZR_FUSION))
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

/// The reference temperature of the heat of reaction
/// ([`Params::oxidation_heat_j_per_kg_zr`], a 298.15 K standard value), K.
pub(crate) const T_298_K: f64 = 298.15;

/// Hydrogen's heat capacity, J/(kg K): a constant molar `c_v` 20.5 J/(mol K)
/// plus R. Since 2026-10-29 only the `without_hydrogen_cp_t` ablation uses
/// it; the default is MELCOR's `c_v(T) + R` ([`super::gas`]). ~~the containment's constant molar `c_v`, so the two parts agree
/// (`containment.rs`)~~ **CORRECTED 2026-10-29**: the containment now uses
/// MELCOR's `c_v(T)` ([`super::gas`]), so the two parts no longer agree.
/// MELCOR's NCG library fit gives `c_v` 20.5-21.9 J/(mol K) over 300-1000 K
/// (Vol. 2, NCG/H2O-RM-33 to 37), so a constant is within about 5 % there,
/// ~~.~~ but the core's gas reaches 2000 K and more, where the fit is
/// higher still (about 25.9 J/(mol K) at 2000 K, +26 %): ~~not checked
/// here, and a gap~~ closed the same day by `hydrogen_cp_t`.
pub(crate) const CP_H2_J_PER_KG_K: f64 = (20.5 + R_GAS) / M_H2;

/// Steam's enthalpy at `t_k` and `p_pa` above water vapour at 298.15 K
/// (IAPWS-IF97 at 1 kPa, below its 3.17 kPa saturation pressure), J/kg: what
/// steam consumed by oxidation brings into the reaction beyond the standard
/// state (gh:#1046). Call it with `t_k` above saturation at `p_pa`.
///
/// Above IF97's ceiling, 2273.15 K (region 5), it **extrapolates** linearly
/// with c_p taken at the ceiling: outside IF97, stated. The stream reaches
/// that only when rods stand to MELCOR's collapse temperatures (gh:#1035).
fn steam_enthalpy_above_298(t_k: f64, p_pa: f64) -> f64 {
    static H_298: OnceLock<f64> = OnceLock::new();
    const IF97_TOP_K: f64 = 2273.15;
    let t_of = |t: f64| ThermodynamicTemperature::new::<kelvin>(t);
    let p = Pressure::new::<pascal>(p_pa);
    let h = |t: f64, p: Pressure| h_tp_eqm_single_phase(t_of(t), p).get::<joule_per_kilogram>();
    let above = if t_k > IF97_TOP_K {
        h(IF97_TOP_K, p) + cp_tp_eqm_single_phase(t_of(IF97_TOP_K), p).get::<joule_per_kilogram_kelvin>() * (t_k - IF97_TOP_K)
    } else {
        h(t_k, p)
    };
    above - *H_298.get_or_init(|| h(T_298_K, Pressure::new::<pascal>(1.0e3)))
}

/// Lipinski's zero-dimensional dryout heat flux of a particulate bed under
/// water, W/m^2 (MELCOR Vol. 2, COR §2.3.8, Eqs 2-114, 2-115; C1244: the
/// constant 0.756, the nominal capillary head 0.089 m for 0.5 mm particles,
/// scaled as `1/d`, and a minimum porosity of 0.15), at pressure `p_pa`
/// for a bed `height` m deep.
pub fn lipinski_dryout_flux(prm: &Params, p_pa: f64, height: f64) -> f64 {
    let sat = super::steam::sat_from_tables(p_pa.clamp(super::steam::P_MIN_PA, super::steam::P_MAX_PA));
    let (rho_l, rho_v, h_lv) = (1.0 / sat.v_f, 1.0 / sat.v_g, sat.h_g - sat.h_f);
    let d = prm.debris_particle_m;
    let eps = prm.debris_porosity.max(0.15);
    let lambda_c = 0.089 * 5.0e-4 / d;
    let num = rho_v * (rho_l - rho_v) * 9.81 * d * eps.r_powi(3) * (1.0 + lambda_c / height.max(1e-3));
    let den = (1.0 - eps) * (1.0 + (rho_v / rho_l).r_powf(0.25)).r_powi(4);
    0.756 * h_lv * (num / den).sqrt()
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

/// MELCOR's gaseous-diffusion limit on Zr oxidation, kg Zr/(m^2 s) (Vol. 2,
/// SAND2023-10994 O, COR §2.5.3, Eq 2-168; Vol. 1, SAND2017-0455 O,
/// C1003; no source used): steam diffusing through the hydrogen to the
/// surface,
///
/// ```text
/// dW/dt = M_Zr k_c p_ox / (n R T_f),   n = 2 mol H2O per mol Zr
/// ```
///
/// (`M_Zr/(n R)` is MELCOR's C1003(1), 0.00548 kg K/(Pa m^3), here derived
/// from the molar masses). `p_ox` is the steam's partial pressure in the
/// ring's gas (the surface's taken as zero, the limit), `T_f` the film
/// temperature, and `k_c = Sh D / D_h` by the heat-mass analogy on this
/// model's own convection correlation (`steam_htc`: Dittus-Boelter with the
/// Schmidt number for the Prandtl, laminar floor Sh = 4.36; MELCOR's
/// developing-flow factor is 1 at its 1000 m entrance length and is left
/// out). `D` is steam in hydrogen ([`Params::h2o_h2_diffusivity_a`]).
/// Simplification: the Reynolds number and the viscosity are the steam's,
/// as in `steam_htc`; the density in the Schmidt number is the mixture's.
/// MELCOR uses this rate when it is below the parabolic one, which is how
/// the caller applies it.
pub fn diffusion_limited_rate(prm: &Params, w_steam: f64, w_h2: f64, t_film: f64, p_pa: f64) -> f64 {
    let (n_s, n_h) = (w_steam / M_H2O, w_h2 / M_H2);
    if n_s <= 0.0 {
        return 0.0;
    }
    let x_steam = n_s / (n_s + n_h);
    let m_mix = (w_steam + w_h2) / (n_s + n_h);
    let t = ThermodynamicTemperature::new::<kelvin>(t_film.clamp(300.0, 2200.0));
    let mu = mu_tp_eqm_single_phase(t, Pressure::new::<pascal>(p_pa)).get::<pascal_second>();
    let d = prm.h2o_h2_diffusivity_a * t_film.r_powf(prm.h2o_h2_diffusivity_b) / p_pa;
    let rho = p_pa * m_mix / (R_GAS * t_film);
    let dh = prm.hydraulic_diameter_m();
    let re = (w_steam / (prm.core_flow_area_m2() / RINGS as f64)) * dh / mu;
    let sc = mu / (rho * d);
    let sh = (0.023 * re.r_powf(0.8) * sc.r_powf(0.4)).max(4.36);
    let k_c = sh * d / dh;
    M_ZR / (2.0 * R_GAS) * k_c * x_steam * p_pa / t_film
}

/// Fractional release rate of the volatile fission products (Xe, Cs, I)
/// from fuel at `t_k`, 1/s: MELCOR's CORSOR-M (Vol. 2, SAND2023-10994 O, RN
/// §2.3.1, Eq 2-11, C7102; no source used), `k0 exp(-Q / (R T))` per
/// minute. Xe, Cs, I2 and Te share one row of its table, so one fraction
/// serves the three tracked here; Te's retention by unoxidised cladding is
/// not modelled and Te is not tracked. Not modelled either: the gap
/// inventory's burst release at cladding failure (RN1_GAP00).
pub fn volatile_release_rate(prm: &Params, t_k: f64) -> f64 {
    const CAL: f64 = 4.184;
    prm.corsor_k0_per_min / 60.0 * (-prm.corsor_q_kcal_per_mol * 1000.0 * CAL / (R_GAS * t_k)).r_exp()
}

/// Fractional release rate of UO2 itself from fuel at `t_k`, 1/s: CORSOR-M's
/// row for La and UO2 (Vol. 2, RN §2.3.1, C7102), `k0 exp(-Q/(R T))` per
/// minute. The released UO2 is the aerosol the vapours condense on
/// (gh:#1047); its inventory is the fuel's UO2.
pub fn uo2_release_rate(prm: &Params, t_k: f64) -> f64 {
    const CAL: f64 = 4.184;
    prm.corsor_uo2_k0_per_min / 60.0 * (-prm.corsor_uo2_q_kcal_per_mol * 1000.0 * CAL / (R_GAS * t_k)).r_exp()
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
                nodes.push(Node { t_fuel: t_clad + q / g.g_fc, t_clad, w_ox: 0.0, relocated: false, damage: 0.0, fp_left: 1.0, uo2_left: 1.0, zr_kg: g.zr_kg });
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
            head: HeadWall::new(t_sat_k),
            shroud_k: [t_sat_k; AXIAL_NODES],
            debris_fp: 0.0,
            fp_released: 0.0,
            debris_uo2: 0.0,
            debris_zr_ox: 0.0,
            quench_f: 0.0,
            uo2_released: 0.0,
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
        let mut q_pool = 0.0;
        let mut steam_used = 0.0;
        let mut h2 = 0.0;
        let mut q_ox_total = 0.0;
        let mut peak: f64 = s.t_sat_k;
        let mut exit_sum = 0.0;
        let mut exit_w = 0.0;
        let mut newly_relocated = Vec::new();
        // Radiation (gh:#1037), explicit from the start-of-step temperatures:
        // a source on each node's cladding, and on each shroud level.
        let (q_rad, q_shroud) = if s.core_radiation { self.radiation(prm, dz) } else { ([0.0; NODES], [0.0; AXIAL_NODES]) };
        // Debris in the lower plenum (relocated in earlier steps): quenched
        // while the plenum holds water, giving its stored heat over a
        // quench time and its decay power at once; the steam it raises
        // rises through the core. Dry, it heats up until the lower head
        // fails; after that its power leaves the vessel (ex-vessel).
        let p_debris = s.decay_w * self.debris_power_share;
        let mut q_quench = 0.0;
        let (mut debris_steam_used, mut debris_h2) = (0.0, 0.0);
        // The quench decay factor decays in time (wet or dry).
        self.quench_f *= (-dt / prm.quench_tau_s).r_exp();
        let mut ex_vessel = 0.0;
        let tdp = s.t_dependent_props;
        if tdp && self.debris_mf + self.debris_mc > 0.0 {
            if self.lower_head_failed {
                ex_vessel = p_debris;
            } else if s.level_m > -prm.baf_m {
                let h_sat = self.debris_mf * uo2_enthalpy().at(s.t_sat_k) + self.debris_mc * zr_enthalpy().at(s.t_sat_k);
                if s.debris_quench {
                    // MELCOR's falling-debris quench (COR §2.3.8): the decay
                    // heat stays in the debris, which gives the water
                    // h f A_p (T_d - T_sat) over the particle area, f the
                    // decay factor; once f < 0.01 the bed is limited by
                    // Lipinski's dryout flux over the plenum's
                    // cross-section. It cannot cool below saturation.
                    let t_d = debris_temperature(self.debris_mf, self.debris_mc, self.debris_h);
                    let v_solid = self.debris_mf / prm.uo2_density + self.debris_mc / prm.zr_density;
                    let a_p = 6.0 * v_solid / prm.debris_particle_m;
                    let conv = prm.quench_h * a_p * (t_d - s.t_sat_k).max(0.0);
                    let mut q = if self.quench_f >= 0.01 {
                        self.quench_f * conv
                    } else {
                        let (_, height, _) = debris_cap(prm, self.debris_mf, self.debris_mc);
                        let a_cs = std::f64::consts::PI * prm.rpv_radius_m * prm.rpv_radius_m;
                        conv.min(lipinski_dryout_flux(prm, s.p_pa, height) * a_cs)
                    };
                    q = q.min(p_debris + (self.debris_h - h_sat).max(0.0) / dt).max(0.0);
                    self.debris_h += (p_debris - q) * dt;
                    q_quench = q;
                } else {
                    let excess = (self.debris_h - h_sat).max(0.0);
                    let release = excess * (1.0 - (-dt / prm.debris_quench_s).r_exp());
                    self.debris_h -= release;
                    q_quench = p_debris + release / dt;
                }
                // The debris oxidises while the water it boils gives it
                // steam (MELCOR COR §3.2.1: particulate debris oxidises by
                // the same parabolic law over `6 V / D` times its Zr volume
                // fraction). Simplifications, stated: the steam is the
                // steam this debris raises this step; dry, it has none (no
                // flow through a dried plenum); no gaseous-diffusion limit.
                if s.debris_oxidation {
                    let t_d = debris_temperature(self.debris_mf, self.debris_mc, self.debris_h);
                    let k = oxidation_rate(prm, t_d);
                    let a_zr = 6.0 * self.debris_mc / prm.zr_density / prm.debris_particle_m;
                    if k > 0.0 && a_zr > 0.0 {
                        let w = self.debris_zr_ox / a_zr;
                        let w_new = (w * w + k * dt).sqrt().min(self.debris_mc / a_zr);
                        let steam_per_zr = 2.0 * M_H2O / M_ZR;
                        let steam = (q_quench * dt / s.h_fg).max(0.0);
                        let dzr = ((w_new - w) * a_zr).min(steam / steam_per_zr).max(0.0);
                        let used = dzr * steam_per_zr / dt;
                        let made = dzr * 2.0 * M_H2 / M_ZR / dt;
                        // Hess's law with reactants and products at the
                        // water's temperature: the reaction heat goes to
                        // the debris and is given up by the quench.
                        let q_ox = dzr * prm.oxidation_heat_j_per_kg_zr / dt + used * steam_enthalpy_above_298(s.t_sat_k + 0.5, s.p_pa)
                            - made * h2_enthalpy_above_298(s.t_sat_k);
                        self.debris_h += q_ox * dt;
                        self.debris_zr_ox += dzr;
                        self.zr_oxidised_kg += dzr;
                        debris_steam_used += used;
                        debris_h2 += made;
                    }
                }
                if s.lower_head_wall && s.wet_head_contact {
                    // The debris sits on the wall under water too: the wall
                    // takes heat from the debris it touches (as dry), and can
                    // fail with water in the plenum (MELCOR's lower head
                    // conducts from the debris in contact with it).
                    let (area, height, rho_bed) = debris_cap(prm, self.debris_mf, self.debris_mc);
                    let t_d = debris_temperature(self.debris_mf, self.debris_mc, self.debris_h);
                    let pen = s.penetration_coverage.then(|| ((height / prm.lp_bottom_level_m).min(1.0), s.t_sat_k, prm.boiling_h));
                    self.debris_h -= self.head.step_with(prm, t_d, prm.debris_to_head_w_m2_k, area, dt, pen) * dt;
                    let dp = (s.p_pa - s.p_dw_pa).max(0.0) + rho_bed * 9.81 * height;
                    if self.head.failed(prm, dp, dt, s.penetrations) {
                        self.lower_head_failed = true;
                    }
                } else if s.lower_head_wall {
                    // Under water the wall boils against the plenum.
                    let (area, _, _) = debris_cap(prm, self.debris_mf, self.debris_mc);
                    q_quench -= self.head.step(prm, s.t_sat_k, prm.boiling_h, area, dt);
                }
            } else {
                self.debris_h += p_debris * dt;
                if s.lower_head_wall {
                    let (area, height, rho_bed) = debris_cap(prm, self.debris_mf, self.debris_mc);
                    let t_d = debris_temperature(self.debris_mf, self.debris_mc, self.debris_h);
                    let pen = s.penetration_coverage.then(|| ((height / prm.lp_bottom_level_m).min(1.0), t_d, 0.0));
                    self.debris_h -= self.head.step_with(prm, t_d, prm.debris_to_head_w_m2_k, area, dt, pen) * dt;
                    let dp = (s.p_pa - s.p_dw_pa).max(0.0) + rho_bed * 9.81 * height;
                    if self.head.failed(prm, dp, dt, s.penetrations) {
                        self.lower_head_failed = true;
                    }
                } else if debris_temperature(self.debris_mf, self.debris_mc, self.debris_h) > prm.lower_head_failure_k {
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
                if s.lower_head_wall {
                    let (area, _, _) = debris_cap(prm, self.debris_mf, self.debris_mc);
                    q_quench -= self.head.step(prm, s.t_sat_k, prm.boiling_h, area, dt);
                }
            } else {
                self.debris_e += p_debris * dt;
                if s.lower_head_wall {
                    let (area, height, rho_bed) = debris_cap(prm, self.debris_mf, self.debris_mc);
                    let t_d = self.debris_e / self.debris_c;
                    self.debris_e -= self.head.step(prm, t_d, prm.debris_to_head_w_m2_k, area, dt) * dt;
                    let dp = (s.p_pa - s.p_dw_pa).max(0.0) + rho_bed * 9.81 * height;
                    if self.head.failed(prm, dp, dt, s.penetrations) {
                        self.lower_head_failed = true;
                    }
                } else if self.debris_e / self.debris_c > prm.lower_head_failure_k {
                    self.lower_head_failed = true;
                }
            }
        }
        q_fluid += q_quench;
        q_pool += q_quench.max(0.0);
        // Steam from below the core, when the level is below it.
        let w_below = if s.level_m <= 0.0 { q_quench / s.h_fg / RINGS as f64 } else { 0.0 };
        for ring in 0..RINGS {
            // Steam rising in this ring: from below, plus the boil-off of
            // its covered nodes.
            let mut w_steam = w_below;
            let mut t_steam = s.t_sat_k;
            // Hydrogen made lower in this ring, kg/s: it rides in the gas
            // stream and is heated with it (gh:#1046).
            let mut w_h2 = 0.0;
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
                        let w_max = node.zr_kg / g.a_zr;
                        let w_new = (node.w_ox * node.w_ox + k * dt).sqrt().min(w_max);
                        let mut dzr = (w_new - node.w_ox) * g.a_zr;
                        if s.diffusion_limit {
                            let rate = diffusion_limited_rate(prm, w_steam, w_h2, 0.5 * (node.t_clad + t_steam), s.p_pa);
                            dzr = dzr.min(rate * g.a_zr * dt);
                        }
                        let steam_per_zr = 2.0 * M_H2O / M_ZR;
                        let dzr_steam = w_steam * dt / steam_per_zr;
                        dzr = dzr.min(dzr_steam);
                        let n = &mut self.nodes[idx];
                        n.w_ox += dzr / g.a_zr;
                        let used = dzr * steam_per_zr / dt;
                        let made = dzr * 2.0 * M_H2 / M_ZR / dt;
                        // Hess's law at the node (gh:#1046): the 298 K heat
                        // of reaction, plus the enthalpy the consumed steam
                        // brings above 298 K vapour, minus heating the new
                        // hydrogen from 298 K to the stream it joins.
                        let h2_heating = if s.hydrogen_cp_t {
                            made * h2_enthalpy_above_298(t_steam)
                        } else {
                            made * CP_H2_J_PER_KG_K * (t_steam - T_298_K)
                        };
                        q_ox = dzr * prm.oxidation_heat_j_per_kg_zr / dt
                            + used * steam_enthalpy_above_298(t_steam.max(s.t_sat_k + 0.5), s.p_pa)
                            - h2_heating;
                        w_steam -= used;
                        w_h2 += made;
                        steam_used += used;
                        h2 += made;
                        self.zr_oxidised_kg += dzr;
                    }
                }
                q_ox_total += q_ox;
                // Clad-to-coolant conductance and coolant temperature.
                let (ha, t_cool, c_rate) = if covered {
                    (prm.boiling_h * g.a_rod, s.t_sat_k, 0.0)
                } else if w_steam > 0.0 {
                    let (h, cp) = steam_htc(prm, w_steam, 0.5 * (node.t_clad + t_steam), s.p_pa);
                    // The stream's heat capacity rate, steam and hydrogen.
                    // Hydrogen's c_p at the stream's inlet temperature (a
                    // linearisation over the node; exact for the ablation).
                    let c_rate = w_steam * cp + w_h2 * cp_h2_per_kg(t_steam, s.hydrogen_cp_t);
                    let ntu = h * g.a_rod / c_rate;
                    (c_rate * (1.0 - (-ntu).r_exp()), t_steam, c_rate)
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
                        n.zr_kg * cp_matpro(HeatCapacityModel::MatproZircaloy, n.t_clad),
                        g.g_fc,
                    )
                } else {
                    (g.c_fuel, g.c_clad * (n.zr_kg / g.zr_kg), g.g_fc)
                };
                let a11 = cf / dt + gfc;
                let a12 = -gfc;
                let b1 = cf / dt * n.t_fuel + q;
                let a21 = -gfc;
                let a22 = cc / dt + gfc + ha;
                let b2 = cc / dt * n.t_clad + ha * t_cool + q_ox + q_rad[idx];
                let det = a11 * a22 - a12 * a21;
                let tf = (b1 * a22 - a12 * b2) / det;
                let tc = (a11 * b2 - a21 * b1) / det;
                let q_out = ha * (tc - t_cool);
                q_fluid += q_out;
                if covered {
                    w_steam += q_out.max(0.0) / s.h_fg;
                    q_pool += q_out.max(0.0);
                } else if w_steam > 0.0 && c_rate > 0.0 {
                    t_steam += q_out / c_rate;
                }
                // With temperature-dependent properties, advance each lump's
                // enthalpy by the heat the solve moved and invert for its
                // temperature (exact energy conservation); otherwise the
                // solve's temperatures are the answer.
                let (tf, tc) = if tdp {
                    let ef = g.m_fuel * uo2_enthalpy().at(n.t_fuel) + dt * (q - gfc * (tf - tc));
                    let ec = n.zr_kg * zr_enthalpy().at(n.t_clad) + dt * (gfc * (tf - tc) - q_out + q_ox + q_rad[idx]);
                    // A node whose cladding has all candled away (no oxide
                    // grew on it) has no clad lump to invert: the solve's
                    // temperature, at zero heat capacity, is its value
                    // (found 2026-10-29 as a 0/0 after the head failed dry).
                    let tc = if n.zr_kg > 0.0 { zr_enthalpy().temperature(ec / n.zr_kg) } else { tc };
                    (uo2_enthalpy().temperature(ef / g.m_fuel), tc)
                } else {
                    (tf, tc)
                };
                let m = &mut self.nodes[idx];
                m.t_fuel = tf;
                m.t_clad = tc;
                peak = peak.max(tc);
                let falls = if s.rod_collapse {
                    // MELCOR's rod collapse (gh:#1035): a rod whose metal is
                    // gone (oxidised below MIN_METAL_M, or candled away once
                    // the shell breaches) accumulates time-at-temperature
                    // damage and collapses at ROD_COLLAPSE_K or when the
                    // damage reaches 1; any rod at ROD_COLLAPSE_MAX_K.
                    // The metal left as a thickness over the cladding's
                    // original one (candling moves metal away).
                    // (Written so the no-candling ablation is bit for bit.)
                    let w_max = m.zr_kg / g.a_zr;
                    let metal_m = (1.0 - m.w_ox / w_max).max(0.0) * prm.clad_wall_m * (m.zr_kg / g.zr_kg);
                    let bare = metal_m < prm.min_metal_m || tc >= prm.clad_breach_k;
                    if bare {
                        m.damage += dt * prm.damage_a * (prm.damage_b * tf).r_exp();
                    }
                    // And the shell itself: once the cladding reaches ZrO2's
                    // melting point the oxide that holds the rod melts.
                    // MELCOR candles it down; this model cannot move it, so
                    // the rod falls (stated simplification). Without this the
                    // cladding, heated by its own oxidation, ran past the
                    // enthalpy tables' 3200 K top (found 2026-10-29).
                    tf >= prm.rod_collapse_max_k
                        || tc >= ZRO2_MELT_K
                        || (bare && (tf >= prm.rod_collapse_k || m.damage >= 1.0))
                } else {
                    tc >= prm.zr_melt_k
                };
                if falls {
                    newly_relocated.push((idx, q / s.decay_w.max(1.0)));
                }
            }
            exit_sum += w_steam * t_steam;
            exit_w += w_steam;
        }
        // Candling (MELCOR COR §3.1, simplified; 2026-10-29): once a
        // node's oxide shell has breached (CLAD_BREACH_K) and its metal is
        // molten, the metal flows down the rod and refreezes in the first
        // node below that is under Zr's melting point, taking its enthalpy
        // with it; through the bottom it joins the lower-plenum debris.
        // The oxide stays. Simplifications, stated: it moves within the
        // step (MELCOR follows a candling velocity), it refreezes whole in
        // that node (no partial refreezing or blockage), and molten UO2 is
        // not candled.
        if s.candling {
            for k in 0..RINGS {
                for j in (0..AXIAL_NODES).rev() {
                    let src = k * AXIAL_NODES + j;
                    let n = self.nodes[src];
                    if n.relocated || n.t_clad < prm.clad_breach_k || n.t_clad < prm.zr_melt_k {
                        continue;
                    }
                    let metal = (n.zr_kg - (n.w_ox * g.a_zr).min(n.zr_kg)).max(0.0);
                    if metal <= 0.0 {
                        continue;
                    }
                    self.nodes[src].zr_kg -= metal;
                    let h_src = zr_enthalpy().at(n.t_clad);
                    let target = (0..j).rev().map(|jj| k * AXIAL_NODES + jj).find(|&t| !self.nodes[t].relocated && self.nodes[t].t_clad < prm.zr_melt_k);
                    match target {
                        Some(t) => {
                            let tn = &mut self.nodes[t];
                            if tdp {
                                let e = tn.zr_kg * zr_enthalpy().at(tn.t_clad) + metal * h_src;
                                tn.zr_kg += metal;
                                tn.t_clad = zr_enthalpy().temperature(e / tn.zr_kg);
                            } else {
                                tn.t_clad = (tn.zr_kg * tn.t_clad + metal * n.t_clad) / (tn.zr_kg + metal);
                                tn.zr_kg += metal;
                            }
                        }
                        None => {
                            self.debris_mc += metal;
                            if tdp {
                                self.debris_h += metal * h_src;
                            } else {
                                self.debris_c += metal * prm.zr_cp;
                                self.debris_e += metal * prm.zr_cp * n.t_clad;
                            }
                        }
                    }
                }
            }
        }
        // Relocation to the lower plenum. The quench decay factor rises by
        // the share of the plenum's debris that arrives (MELCOR's
        // `V_cor / V_LP`, held at most 1).
        let v_before = self.debris_mf / prm.uo2_density + self.debris_mc / prm.zr_density;
        let arriving = !newly_relocated.is_empty();
        for (idx, share) in newly_relocated {
            let n = &mut self.nodes[idx];
            n.relocated = true;
            // Masses in both property paths: the lower-head wall's bed
            // geometry needs them (gh:#1034).
            self.debris_mf += g.m_fuel;
            let zr = n.zr_kg;
            let c_clad = g.c_clad * (zr / g.zr_kg);
            self.debris_mc += zr;
            self.debris_zr_ox += (n.w_ox * g.a_zr).min(zr);
            if tdp {
                self.debris_h += g.m_fuel * uo2_enthalpy().at(n.t_fuel) + zr * zr_enthalpy().at(n.t_clad);
            } else {
                self.debris_c += g.c_fuel + c_clad;
                self.debris_e += g.c_fuel * n.t_fuel + c_clad * n.t_clad;
            }
            self.debris_power_share += share;
            // Its fission products go with it (inventory taken as
            // proportional to power, [A]).
            self.debris_fp += share * n.fp_left;
            n.fp_left = 0.0;
            // The fuel's UO2 by mass (each node's share of it).
            self.debris_uo2 += n.uo2_left / NODES as f64;
            n.uo2_left = 0.0;
        }
        if arriving {
            let v_after = self.debris_mf / prm.uo2_density + self.debris_mc / prm.zr_density;
            self.quench_f = (self.quench_f + (v_after - v_before) / v_after.max(1e-12)).min(1.0);
        }
        let debris_k = if tdp && self.debris_mf + self.debris_mc > 0.0 {
            debris_temperature(self.debris_mf, self.debris_mc, self.debris_h)
        } else if !tdp && self.debris_c > 0.0 {
            self.debris_e / self.debris_c
        } else {
            s.t_sat_k
        };
        // Volatile fission-product release, CORSOR-M at the fuel's (or the
        // in-vessel debris's) temperature (gh:#1040). An output only:
        // nothing in the model reads it yet.
        let k_rel = |t: f64| volatile_release_rate(prm, t);
        let axial_r = axial_shape(prm);
        for ring in 0..RINGS {
            for j in 0..AXIAL_NODES {
                let n = &mut self.nodes[ring * AXIAL_NODES + j];
                if n.relocated {
                    continue;
                }
                let keep = (-k_rel(n.t_fuel) * dt).r_exp();
                let share = prm.ring_factors[ring] / RINGS as f64 * axial_r[j];
                self.fp_released += share * n.fp_left * (1.0 - keep);
                n.fp_left *= keep;
                let keep_u = (-uo2_release_rate(prm, n.t_fuel) * dt).r_exp();
                self.uo2_released += n.uo2_left / NODES as f64 * (1.0 - keep_u);
                n.uo2_left *= keep_u;
            }
        }
        // In-vessel only: once the lower head fails the debris is on the
        // drywell floor, where MELCOR releases by its cavity model (VANESA),
        // not CORSOR; that is not built, so ex-vessel debris releases
        // nothing here (stated). Its temperature is not followed there
        // either.
        if self.debris_fp > 0.0 && !self.lower_head_failed {
            let keep = (-k_rel(debris_k) * dt).r_exp();
            self.fp_released += self.debris_fp * (1.0 - keep);
            self.debris_fp *= keep;
        }
        if self.debris_uo2 > 0.0 && !self.lower_head_failed {
            let keep = (-uo2_release_rate(prm, debris_k) * dt).r_exp();
            self.uo2_released += self.debris_uo2 * (1.0 - keep);
            self.debris_uo2 *= keep;
        }
        // The shroud: held at saturation where the collapsed level covers it
        // (water on both faces), otherwise heated by the outer ring's
        // radiation, its outer face adiabatic (stated).
        if s.core_radiation {
            let m_level = shroud_mass_per_level(prm, dz);
            for j in 0..AXIAL_NODES {
                let z_mid = (j as f64 + 0.5) * dz;
                self.shroud_k[j] = if s.level_m >= z_mid {
                    s.t_sat_k
                } else {
                    steel_t(steel_h(self.shroud_k[j]) + q_shroud[j] * dt / m_level)
                };
            }
        }
        self.h2_total_kg += h2 * dt;
        let relocated = self.nodes.iter().filter(|n| n.relocated).count() as f64 / NODES as f64;
        CoreResult {
            q_fluid_w: q_fluid,
            steam_used_kg_s: steam_used + debris_steam_used,
            h2_kg_s: h2 + debris_h2,
            q_ox_w: q_ox_total,
            peak_clad_k: peak,
            exit_gas_k: if exit_w > 0.0 { exit_sum / exit_w } else { s.t_sat_k },
            exit_steam_kg_s: exit_w,
            fp_volatile_released: self.fp_released,
            uo2_released_kg: self.uo2_released * prm.core_uo2_kg(),
            zr_oxidised_kg: self.zr_oxidised_kg,
            h2_total_kg: self.h2_total_kg,
            relocated,
            debris_k,
            lower_head_failed: self.lower_head_failed,
            ex_vessel_w: ex_vessel,
            head_inner_k: self.head.t_k[0],
            head_strain: self.head.strain,
            q_pool_w: q_pool,
            clad_k: self.nodes.iter().map(|n| if n.relocated { f64::NAN } else { n.t_clad }).collect(),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // used by the browser's solver workers (and the tests)
impl Core {
    /// The whole state, for a stateless worker to step.
    pub fn write_state(&self, w: &mut Writer) {
        for n in &self.nodes {
            w.f(&[n.t_fuel, n.t_clad, n.w_ox, if n.relocated { 1.0 } else { 0.0 }, n.damage, n.fp_left, n.uo2_left, n.zr_kg]);
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
        w.f(&self.head.t_k);
        w.f(&[self.head.strain, self.head.pen_k]);
        w.f(&self.shroud_k);
        w.f(&[self.debris_fp, self.fp_released, self.debris_uo2, self.uo2_released, self.debris_zr_ox, self.quench_f]);
    }
    pub fn read_state(r: &mut Reader) -> Self {
        let nodes = (0..NODES).map(|_| Node { t_fuel: r.f(), t_clad: r.f(), w_ox: r.f(), relocated: r.f() != 0.0, damage: r.f(), fp_left: r.f(), uo2_left: r.f(), zr_kg: r.f() }).collect();
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
            head: HeadWall { t_k: std::array::from_fn(|_| r.f()), strain: r.f(), pen_k: r.f() },
            shroud_k: std::array::from_fn(|_| r.f()),
            debris_fp: r.f(),
            fp_released: r.f(),
            debris_uo2: r.f(),
            uo2_released: r.f(),
            debris_zr_ox: r.f(),
            quench_f: r.f(),
        }
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))] // used by the browser's solver workers (and the tests)
impl CoreSnapshot {
    pub fn write(&self, w: &mut Writer) {
        w.f(&[
            self.dt_s,
            self.p_pa,
            self.t_sat_k,
            self.h_fg,
            self.level_m,
            self.decay_w,
            if self.t_dependent_props { 1.0 } else { 0.0 },
            self.p_dw_pa,
            if self.lower_head_wall { 1.0 } else { 0.0 },
            if self.rod_collapse { 1.0 } else { 0.0 },
            if self.core_radiation { 1.0 } else { 0.0 },
            if self.diffusion_limit { 1.0 } else { 0.0 },
            if self.hydrogen_cp_t { 1.0 } else { 0.0 },
            if self.penetrations { 1.0 } else { 0.0 },
            if self.debris_oxidation { 1.0 } else { 0.0 },
            if self.candling { 1.0 } else { 0.0 },
            if self.debris_quench { 1.0 } else { 0.0 },
            if self.wet_head_contact { 1.0 } else { 0.0 },
            if self.penetration_coverage { 1.0 } else { 0.0 },
        ]);
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
            p_dw_pa: r.f(),
            lower_head_wall: r.f() != 0.0,
            rod_collapse: r.f() != 0.0,
            core_radiation: r.f() != 0.0,
            diffusion_limit: r.f() != 0.0,
            hydrogen_cp_t: r.f() != 0.0,
            penetrations: r.f() != 0.0,
            debris_oxidation: r.f() != 0.0,
            candling: r.f() != 0.0,
            debris_quench: r.f() != 0.0,
            wet_head_contact: r.f() != 0.0,
            penetration_coverage: r.f() != 0.0,
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
            self.exit_steam_kg_s,
            self.fp_volatile_released,
            self.uo2_released_kg,
            self.zr_oxidised_kg,
            self.h2_total_kg,
            self.relocated,
            self.debris_k,
            if self.lower_head_failed { 1.0 } else { 0.0 },
            self.ex_vessel_w,
            self.head_inner_k,
            self.head_strain,
            self.q_pool_w,
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
            exit_steam_kg_s: r.f(),
            fp_volatile_released: r.f(),
            uo2_released_kg: r.f(),
            zr_oxidised_kg: r.f(),
            h2_total_kg: r.f(),
            relocated: r.f(),
            debris_k: r.f(),
            lower_head_failed: r.f() != 0.0,
            ex_vessel_w: r.f(),
            head_inner_k: r.f(),
            head_strain: r.f(),
            q_pool_w: r.f(),
            clad_k: r.vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CORSOR-M's volatile release rate against hand values: Q = 63.8 x
    /// 4184 = 266.9 kJ/mol, so at 2400 K `exp(-13.378) = 1.548e-6` times
    /// 2e5/60 gives 5.16e-3 /s (half released in about 2 min), and at
    /// 1000 K 3.8e-11 /s (nothing on the accident's time scale).
    #[test]
    fn volatile_release_follows_corsor_m() {
        let prm = Params::default();
        let k = volatile_release_rate(&prm, 2400.0);
        assert!((k / 5.16e-3 - 1.0).abs() < 2e-3, "{k}");
        let cold = volatile_release_rate(&prm, 1000.0);
        assert!((cold / 3.8e-11 - 1.0).abs() < 0.02, "{cold}");
    }
    /// CORSOR-M's UO2 row against a hand value: `Q = 143.1 x 4184 = 598.7
    /// kJ/mol`, so at 2500 K `exp(-28.80) = 3.1e-13` times 1.46e7/60 gives
    /// 7.6e-8 /s.
    #[test]
    fn uo2_release_follows_corsor_m() {
        let k = uo2_release_rate(&Params::default(), 2500.0);
        assert!((k / 7.6e-8 - 1.0).abs() < 0.03, "{k}");
    }


    /// The gaseous-diffusion limit (MELCOR COR Eq 2-168) against hand
    /// values, in the laminar floor (a small steam flow, Sh = 4.36):
    /// - `M_Zr/(2R)` is MELCOR's C1003(1), 0.00548 kg K/(Pa m^3);
    /// - pure steam at 1500 K film and 7 MPa: `D = 6.60639e-4 x 1500^1.68 /
    ///   7e6 = 2.044e-5 m^2/s`, so the rate is `0.005486 x 4.36 D / D_h x
    ///   7e6 / 1500`;
    /// - it does not depend on pressure there (`D ~ 1/p`, `p_ox ~ p`), and
    ///   it halves at equal moles of steam and hydrogen; no steam, no rate.
    #[test]
    fn the_diffusion_limit_follows_melcors_equation() {
        let prm = Params::default();
        assert!((M_ZR / (2.0 * R_GAS) - 0.00548).abs() < 0.00001);
        let w = 1e-6;
        let r = diffusion_limited_rate(&prm, w, 0.0, 1500.0, 7.0e6);
        let d = 2.044e-5;
        let hand = 0.005486 * 4.36 * d / prm.hydraulic_diameter_m() * 7.0e6 / 1500.0;
        assert!((r / hand - 1.0).abs() < 1e-3, "{r} vs {hand}");
        assert!((diffusion_limited_rate(&prm, w, 0.0, 1500.0, 0.5e6) / r - 1.0).abs() < 1e-12);
        let half = diffusion_limited_rate(&prm, w, w * M_H2 / M_H2O, 1500.0, 7.0e6);
        assert!((half / r - 0.5).abs() < 1e-12, "{half} vs {r}");
        assert_eq!(diffusion_limited_rate(&prm, 0.0, 1.0, 1500.0, 7.0e6), 0.0);
    }

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
        let s = CoreSnapshot { dt_s: 1.0, p_pa: 7.0e6, t_sat_k: 560.0, h_fg: 1.5e6, level_m: 8.0, decay_w: 20.0e6, t_dependent_props: false, p_dw_pa: 1.0e5, lower_head_wall: true, rod_collapse: true, core_radiation: true, diffusion_limit: true, hydrogen_cp_t: true, penetrations: true, debris_oxidation: true, candling: true, debris_quench: true, wet_head_contact: true, penetration_coverage: true, prm: Params::default() };
        let mut r = c.step(&s);
        for _ in 0..3000 {
            r = c.step(&s);
        }
        assert!((r.q_fluid_w - 20.0e6).abs() < 20.0e6 * 1e-6, "{}", r.q_fluid_w);
        assert!(r.peak_clad_k < 562.0 && r.h2_kg_s == 0.0, "{}", r.peak_clad_k);
    }

    /// An uncovered core with no steam heats adiabatically: the energy it
    /// gains is the decay power times the time (no oxidation without steam).
    /// Since gh:#1037 the shroud is in the balance: radiation moves heat to
    /// it and between nodes, and creates none.
    #[test]
    fn a_dry_core_without_steam_heats_adiabatically() {
        let mut c = Core::new(560.0, &Params::default());
        // Start from equilibrium at saturation so the stored heat is gone.
        for n in c.nodes.iter_mut() {
            n.t_fuel = 560.0;
            n.t_clad = 560.0;
        }
        let g = geom(&Params::default());
        let dz = Params::default().active_length_m / AXIAL_NODES as f64;
        let m_sh = shroud_mass_per_level(&Params::default(), dz);
        let shroud = |c: &Core| -> f64 { c.shroud_k.iter().map(|t| m_sh * steel_h(*t)).sum() };
        let e0: f64 = c.nodes.iter().map(|n| g.c_fuel * n.t_fuel + g.c_clad * (n.zr_kg / g.zr_kg) * n.t_clad).sum::<f64>() + shroud(&c);
        let s = CoreSnapshot { dt_s: 1.0, p_pa: 7.0e6, t_sat_k: 560.0, h_fg: 1.5e6, level_m: -1.0, decay_w: 10.0e6, t_dependent_props: false, p_dw_pa: 1.0e5, lower_head_wall: true, rod_collapse: true, core_radiation: true, diffusion_limit: true, hydrogen_cp_t: true, penetrations: true, debris_oxidation: true, candling: true, debris_quench: true, wet_head_contact: true, penetration_coverage: true, prm: Params::default() };
        for _ in 0..100 {
            let r = c.step(&s);
            assert_eq!(r.h2_kg_s, 0.0);
        }
        let e1: f64 = c.nodes.iter().map(|n| g.c_fuel * n.t_fuel + g.c_clad * (n.zr_kg / g.zr_kg) * n.t_clad).sum::<f64>() + shroud(&c);
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
        let dz = Params::default().active_length_m / AXIAL_NODES as f64;
        let m_sh = shroud_mass_per_level(&Params::default(), dz);
        let h = |c: &Core| -> f64 {
            c.nodes.iter().map(|n| g.m_fuel * uo2_enthalpy().at(n.t_fuel) + n.zr_kg * zr_enthalpy().at(n.t_clad)).sum::<f64>()
                + c.shroud_k.iter().map(|t| m_sh * steel_h(*t)).sum::<f64>()
        };
        let e0 = h(&c);
        let s = CoreSnapshot { dt_s: 1.0, p_pa: 7.0e6, t_sat_k: 560.0, h_fg: 1.5e6, level_m: -1.0, decay_w: 10.0e6, t_dependent_props: true, p_dw_pa: 1.0e5, lower_head_wall: true, rod_collapse: true, core_radiation: true, diffusion_limit: true, hydrogen_cp_t: true, penetrations: true, debris_oxidation: true, candling: true, debris_quench: true, wet_head_contact: true, penetration_coverage: true, prm: Params::default() };
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
        // The heat of fusion (gh:#1035) is the one-kelvin step at 2098 K:
        // 2.25e5 J/kg plus about c_p.
        let step = zr_enthalpy().at(2099.0) - zr_enthalpy().at(2098.0);
        let sensible = zr_enthalpy().at(2098.0) - zr_enthalpy().at(2097.0);
        assert!((step - sensible - ZR_FUSION.1).abs() < 1.0, "{step} {sensible}");
        assert!((zr_enthalpy().temperature(zr_enthalpy().at(2098.5)) - 2098.5).abs() < 1e-6);
    }

    /// The lower-head wall (gh:#1034). Methodology:
    /// - **Energy:** heat a wall from 560 K against 2000 K debris for 600
    ///   one-second steps; the heat it reports taking equals its enthalpy
    ///   rise (Kim 304L), to round-off.
    /// - **Creep:** at 10 MPa effective stress and 1273 K, the time to
    ///   rupture from the Larson-Miller constants is about 6.0e4 s, as
    ///   MELCOR's Eqs 6-23 and 6-24 give by hand. So one 600 s step adds
    ///   about 0.18 x 600 / 6.0e4 of strain.
    /// - **Bed:** a whole core's debris (about 19.8 m^3 at 0.4 porosity)
    ///   sits as a spherical cap of exactly that volume inside the 29 m^3
    ///   hemisphere, wetting `2 pi R h`; twice that overflows it and wets
    ///   all of it, `2 pi R^2`.
    ///
    /// Results (2026-10-29):
    /// - energy matches to 1e-9 relative;
    /// - t_R is within 10 % of 6.0e4 s;
    /// - the cap volume matches to 1e-9 relative, and the overflowing bed's
    ///   area is `2 pi R^2`.
    #[test]
    fn the_lower_head_wall_conserves_energy_and_creeps_by_larson_miller() {
        let prm = Params::default();
        let mut w = HeadWall::new(560.0);
        let rho = steel_rho();
        let dx = prm.lower_head_thickness_m / (WALL_NODES - 1) as f64;
        let mass = |i: usize| rho * dx * if i == 0 || i == WALL_NODES - 1 { 0.5 } else { 1.0 };
        let e0: f64 = (0..WALL_NODES).map(|i| mass(i) * steel_h(w.t_k[i])).sum();
        let mut taken = 0.0;
        for _ in 0..600 {
            taken += w.step(&prm, 2000.0, prm.debris_to_head_w_m2_k, 1.0, 1.0);
        }
        let e1: f64 = (0..WALL_NODES).map(|i| mass(i) * steel_h(w.t_k[i])).sum();
        assert!(((e1 - e0) - taken).abs() <= 1e-9 * taken, "{} vs {taken}", e1 - e0);
        assert!(w.t_k[0] > w.t_k[WALL_NODES - 1], "{:?}", w.t_k);
        // The penetration (10 mm of steel, about 42 kJ/(m^2 K) at 560 K)
        // rises about 1440 x 1000 / (42e3 + 1000) = 33 K in the first 1 s
        // at 2000 K, so its time constant is about 40 s: after 600 s it is
        // at the medium, far ahead of the wall's inner node. At the
        // temperature limit it fails the head only with `penetrations`.
        let mut pw = HeadWall::new(560.0);
        pw.step(&prm, 2000.0, prm.debris_to_head_w_m2_k, 1.0, 1.0);
        assert!((pw.pen_k - 560.0 - 33.0).abs() < 4.0, "{}", pw.pen_k);
        assert!(w.pen_k > 1990.0 && w.pen_k > w.t_k[0] + 300.0, "{} {:?}", w.pen_k, w.t_k);
        let mut q = HeadWall::new(560.0);
        q.pen_k = prm.head_failure_wall_k;
        assert!(q.failed(&prm, 0.0, 1.0, true) && !q.failed(&prm, 0.0, 1.0, false));
        // Larson-Miller: sigma 10 MPa at 1273 K.
        let (ri, ro) = (prm.rpv_radius_m, prm.rpv_radius_m + prm.lower_head_thickness_m);
        let dp = 10.0e6 * (ro * ro - ri * ri) / (ri * ri);
        let mut c = HeadWall::new(1273.0);
        assert!(!c.failed(&prm, dp, 600.0, false));
        let t_rupture = prm.creep_fail_strain * 600.0 / c.strain;
        assert!((t_rupture / 6.0e4 - 1.0).abs() < 0.1, "{t_rupture}");
        // A whole core is a cap of its own volume; twice that overflows.
        let (mf, mc) = (prm.core_uo2_kg(), prm.core_zr_kg());
        let (area, h, rho_bed) = debris_cap(&prm, mf, mc);
        let r = prm.rpv_radius_m;
        let v = (mf + mc) / rho_bed;
        let cap = std::f64::consts::PI * h * h * (3.0 * r - h) / 3.0;
        assert!((cap - v).abs() <= 1e-9 * v && h < r, "{cap} vs {v}, h {h}");
        assert!((area - 2.0 * std::f64::consts::PI * r * h).abs() < 1e-9);
        let (full, h2, _) = debris_cap(&prm, 2.0 * mf, 2.0 * mc);
        assert!((full - 2.0 * std::f64::consts::PI * r * r).abs() < 1e-9 && h2 == r);
    }

    /// Radiation (gh:#1037): MELCOR's Zircaloy emissivity points (0.325
    /// bare, 0.808 at 3.88 um, 0.759 at 1 mm), its degradation above 1500 K
    /// floored at 0.325, steel's linear law at 616.48 K, and the gray-body
    /// exchange's sign and symmetry.
    #[test]
    fn radiation_follows_melcors_emissivities_and_conserves() {
        assert!((zr_emissivity(0.0, 1000.0) - 0.325).abs() < 1e-12);
        assert!((zr_emissivity(3.88e-6, 1000.0) - 0.808).abs() < 1e-12);
        assert!((zr_emissivity(2.0e-3, 1000.0) - 0.759).abs() < 1e-12);
        assert!((zr_emissivity(2.0e-3, 1600.0) - 0.759 * (-1.0f64 / 3.0).exp()).abs() < 1e-9);
        assert_eq!(zr_emissivity(2.0e-3, 3000.0), 0.325);
        assert!((steel_emissivity(616.48) - 0.25617).abs() < 1e-12);
        let x = gray_exchange(1.0, 0.8, 0.5, 1500.0, 1000.0);
        assert!(x > 0.0 && (x + gray_exchange(1.0, 0.5, 0.8, 1000.0, 1500.0)).abs() < 1e-9 * x);
    }

    /// The steam term of Hess's law (gh:#1046): saturated steam at 7 MPa
    /// carries about 0.23 MJ/kg above 298 K vapour (IF97: h_g at 7 MPa is
    /// 2772 kJ/kg, vapour at 298 K 2547 kJ/kg), and superheating adds about
    /// c_p per kelvin.
    #[test]
    fn steam_brings_its_enthalpy_above_298_k_into_the_reaction() {
        let sat = steam_enthalpy_above_298(560.0, 7.0e6);
        assert!((0.15e6..0.30e6).contains(&sat), "{sat}");
        let hot = steam_enthalpy_above_298(1500.0, 7.0e6);
        assert!(hot > sat + 1.5e6 && hot < sat + 3.0e6, "{hot}");
        assert!(CP_H2_J_PER_KG_K > 14.0e3 && CP_H2_J_PER_KG_K < 14.6e3);
        // Past IF97's 2273.15 K ceiling it extrapolates, continuously.
        let below = steam_enthalpy_above_298(2273.0, 1.0e6);
        let past = steam_enthalpy_above_298(2400.0, 1.0e6);
        assert!(past > below && past - below < 127.0 * 4.0e3, "{below} {past}");
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

    /// Zircaloy is conserved through candling and relocation (2026-10-29):
    /// over 8 h of the historical run (through the core's melt and
    /// relocation), the Zr in the nodes plus the debris's is the core's
    /// to 1e-9 at every step, and some metal has candled by then (a node
    /// holds more than its geometric share).
    #[test]
    fn zircaloy_is_conserved_through_candling() {
        use super::super::{Mode, Plant};
        let prm = Params::default();
        let g = geom(&prm);
        let mut p = Plant::new(Mode::Historical);
        let (mut c, mut k) = p.parts();
        let total = prm.core_zr_kg();
        let mut candled = false;
        for _ in 0..(8 * 3600) {
            p.step_inline(&mut c, &mut k);
            let in_nodes: f64 = c.nodes.iter().filter(|n| !n.relocated).map(|n| n.zr_kg).sum();
            let sum = in_nodes + c.debris_mc;
            assert!((sum - total).abs() < 1e-9 * total, "{sum} vs {total}");
            candled |= c.nodes.iter().any(|n| !n.relocated && n.zr_kg > g.zr_kg * (1.0 + 1e-9));
        }
        assert!(candled);
    }

    /// Lipinski's dryout flux against a hand value (2026-10-29): at 0.5 MPa
    /// (rho_l 915, rho_v 2.67 kg/m^3, h_lv 2.108 MJ/kg), d = 1 cm, eps =
    /// 0.4 and a 1 m bed, 0.756 h_lv sqrt(15.36 / 1.384) = 5.31 MW/m^2. It
    /// rises with the particle diameter (as sqrt(d)), and the porosity is
    /// held at least 0.15.
    #[test]
    fn lipinski_dryout_matches_a_hand_value() {
        let prm = Params::default();
        let q = lipinski_dryout_flux(&prm, 0.5e6, 1.0);
        assert!((q / 5.31e6 - 1.0).abs() < 0.02, "{q}");
        let mut big = prm;
        big.debris_particle_m = 0.04;
        assert!(lipinski_dryout_flux(&big, 0.5e6, 1.0) / q > 1.9, "{}", lipinski_dryout_flux(&big, 0.5e6, 1.0) / q);
        let mut tight = prm;
        tight.debris_porosity = 0.05;
        let mut floor = prm;
        floor.debris_porosity = 0.15;
        assert_eq!(lipinski_dryout_flux(&tight, 0.5e6, 1.0), lipinski_dryout_flux(&floor, 0.5e6, 1.0));
    }
}
