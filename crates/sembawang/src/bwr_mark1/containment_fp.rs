//! The fission products a Mark I containment holds, and how they move
//! (gh:#1040, gh:#1047; shared since gh:#1100).
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! **Moved out of [`super::containment::Containment`] on 2026-11-01
//! (gh:#1100) with its arithmetic unchanged**, so that
//! [`super::containment_network::ContainmentNetwork`] carries the same
//! physics rather than a second copy: the noble gas as a trace
//! non-condensable, cesium, iodine and tellurium as vapour and on the
//! aerosol in the drywell and the wetwell gas ([`RnVolume`]), what the pool
//! has scrubbed (SPARC-90's condensation, impaction and swarm rise,
//! [`super::sparc`]; MELCOR RN RM §2.7.1 and Appendix F, cited), what has
//! left to the stack and the building, and the decay heat each location
//! takes (`fp_decay`). The byte-for-byte `headless_30h.csv` fixture is the
//! evidence the move changed nothing in the plant.
//!
//! The caller owns the gas: each method takes the state of the volumes it
//! needs as plain numbers (the old model passes its own inventories, the
//! network its control volumes'), and moves the fission products in the
//! fraction of a donor's gas a path took.

use super::aerosol::{diffusiophoretic_velocity, Facing, Medium, Surface};
use super::containment::{air_conductivity, air_viscosity, condensation_pass, nc_share, rn_sections, ContainmentSnapshot, RN_STEP_S};
use super::iodine;
use super::params::*;
use super::rn_volume::{RnSurface, RnVolume, CS, IODINE, N_CLASSES, TE};
use super::sparc;
use super::steam::p_sat_pa;
use super::vapour::{BulkGas, FpClass};

/// Index of the particle mass in `pool_scrubbed`, after the classes.
pub(crate) const AEROSOL: usize = N_CLASSES;

/// The decay heat of the fission products a containment holds, W, by where
/// it is deposited: airborne in the drywell (its gas), on the drywell's
/// structures (their walls), and in the wetwell gas, on the pool's surface
/// and scrubbed into the pool (the pool).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FpDecayHeat {
    pub dw_gas_w: f64,
    pub dw_wall_w: f64,
    pub pool_w: f64,
}

/// Where the fission products are, as the old model's
/// [`super::containment::ContainmentResult`] reports them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FpReport {
    pub xe_dw: f64,
    pub xe_ww: f64,
    pub xe_vented: f64,
    pub xe_building: f64,
    pub cs_dw_structures_kg: f64,
    pub cs_ww_pool_kg: f64,
    pub cs_airborne_kg: f64,
    pub cs_vented_kg: f64,
    pub cs_building_kg: f64,
    pub i_dw_structures_kg: f64,
    pub i_ww_pool_kg: f64,
    pub i_airborne_kg: f64,
    pub i_vented_kg: f64,
    pub i_building_kg: f64,
    pub te_kg: f64,
}

/// Gas a path moved from the drywell into the pool this step (the
/// downcomers), and the drywell's state it left: the fraction `f` of the
/// drywell's gas, `n` mol in all, of which `dn2` mol of nitrogen-like
/// non-condensables, `dh2` mol of hydrogen and `dsteam_kg` of steam.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DowncomerFlow {
    pub f: f64,
    pub n: f64,
    pub dn2: f64,
    pub dh2: f64,
    pub dsteam_kg: f64,
    pub t_dw_k: f64,
    pub p_dw_pa: f64,
}

/// The pool the scrubbing sees: its (top layer's) temperature, the wetwell
/// pressure above it and all its water, kg.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PoolSide {
    pub t_k: f64,
    pub p_ww_pa: f64,
    pub water_kg: f64,
}

/// The volumes' state for the aerosol step: the drywell's gas (steam, kg,
/// nitrogen-like and hydrogen moles, temperature, pressure, the gas volume
/// for its density), its structures' heat-transfer coefficient and
/// temperature; the pool; the wetwell gas's steam mole fraction,
/// temperature, nitrogen-like and hydrogen moles, and the pool's area.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RnContext {
    pub dw_steam_kg: f64,
    pub dw_n2: f64,
    pub dw_h2: f64,
    pub dw_t_k: f64,
    pub p_dw_pa: f64,
    pub dw_gas_m3: f64,
    pub h_wall: f64,
    pub dw_wall_t_k: f64,
    pub pool: PoolSide,
    pub ww_x_steam: f64,
    pub ww_t_k: f64,
    pub ww_n2: f64,
    pub ww_h2: f64,
    pub pool_area_m2: f64,
}

/// Specific heats for the heat-mass analogy, J/(kg K): steam, N2, H2. \[A\]
/// constant values near 400-500 K.
const CP_STEAM_GAS: f64 = 2000.0;
const CP_N2_GAS: f64 = 1040.0;
const CP_H2_GAS: f64 = 14_300.0;

/// The fission products in a Mark I containment. See the module docs.
#[derive(Clone, Debug, PartialEq)]
pub struct FpHold {
    /// Noble gas (Xe), fractions of the core inventory: in the drywell, the
    /// wetwell gas, vented to the stack, leaked to the building (gh:#1040).
    pub(crate) xe_dw: f64,
    pub(crate) xe_ww: f64,
    pub(crate) xe_vented: f64,
    pub(crate) xe_building: f64,
    /// Fission products and aerosol in the drywell and the wetwell gas
    /// (gh:#1047), the time since their last aerosol step, s, and cesium
    /// vented and leaked to the building, kg.
    pub(crate) dw_rn: RnVolume,
    pub(crate) ww_rn: RnVolume,
    pub(crate) rn_clock_s: f64,
    pub(crate) cs_vented_kg: f64,
    pub(crate) cs_building_kg: f64,
    pub(crate) i_vented_kg: f64,
    pub(crate) i_building_kg: f64,
    /// Tellurium vented or leaked to the building, kg.
    pub(crate) te_out_kg: f64,
    /// The swarm-rise pass fractions per section at the downcomers, and
    /// their age, s: recomputed at most once per aerosol step (`sparc`
    /// costs about 0.1 ms a call), from the conditions of the step that
    /// recomputes them.
    pub(crate) swarm_pass: Vec<f64>,
    pub(crate) swarm_age_s: f64,
    /// The pool's partition coefficient for I2 (`iodine::partition`),
    /// updated at each aerosol step.
    pub(crate) iodine_h: f64,
    /// The swarm rise's pass fraction for I2 vapour at the downcomers, and
    /// its age, s: recomputed at most once per aerosol step, as the
    /// particles' are.
    pub(crate) iodine_swarm_pass: f64,
    pub(crate) iodine_swarm_age_s: f64,
    /// Steam condensed in the drywell since the last aerosol step, kg: on
    /// its walls, for their diffusiophoresis.
    pub(crate) dw_condensed_kg: f64,
    /// Scrubbed into the pool water at the vent exits (SPARC-90's DF_EC),
    /// kg: cesium, iodine, tellurium, aerosol.
    pub(crate) pool_scrubbed: [f64; N_CLASSES + 1],
}

impl FpHold {
    /// Empty, the drywell's and the wetwell's gas volumes `dw_m3`, `ww_m3`.
    pub fn new(dw_m3: f64, ww_m3: f64) -> Self {
        Self {
            xe_dw: 0.0,
            xe_ww: 0.0,
            xe_vented: 0.0,
            xe_building: 0.0,
            dw_rn: RnVolume::new(dw_m3, rn_sections(), 2),
            ww_rn: RnVolume::new(ww_m3, rn_sections(), 1),
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
        }
    }

    /// Per-step settings from the snapshot, and the swarm caches' age.
    pub(crate) fn begin_step(&mut self, s: &ContainmentSnapshot) {
        self.swarm_age_s += s.dt_s;
        self.iodine_swarm_age_s += s.dt_s;
        self.dw_rn.lumped = !s.sectional_classes;
        self.ww_rn.lumped = !s.sectional_classes;
        self.dw_rn.condensate_mass = s.condensate_mass;
        self.ww_rn.condensate_mass = s.condensate_mass;
    }

    /// The decay heat of the fission products held (`fp_decay`), from the
    /// snapshot's whole-core class powers: each location takes its share of
    /// the class's inventory. `None` when the snapshot carries none (the
    /// old model then leaves its energies untouched, bit for bit).
    pub fn decay_heat(&self, prm: &Params, s: &ContainmentSnapshot) -> Option<FpDecayHeat> {
        if !(s.q_noble_w + s.q_alkali_w + s.q_halogen_w + s.q_chalcogen_w > 0.0) {
            return None;
        }
        let cs = |kg: f64| kg / prm.cs_inventory_kg * s.q_alkali_w;
        let io = |kg: f64| kg / prm.i_inventory_kg * s.q_halogen_w;
        let te = |kg: f64| kg / prm.te_inventory_kg * s.q_chalcogen_w;
        let air = |v: &RnVolume, c: usize| v.vapour_kg[c] + v.on_aerosol(c);
        let held = |v: &RnVolume, c: usize| v.on_surface_kg.iter().map(|x| x[c]).sum::<f64>();
        let dw_gas_w = self.xe_dw * s.q_noble_w + cs(air(&self.dw_rn, CS)) + io(air(&self.dw_rn, IODINE)) + te(air(&self.dw_rn, TE));
        let dw_wall_w = cs(held(&self.dw_rn, CS)) + io(held(&self.dw_rn, IODINE)) + te(held(&self.dw_rn, TE));
        let pool_w = self.xe_ww * s.q_noble_w
            + cs(air(&self.ww_rn, CS) + held(&self.ww_rn, CS) + self.pool_scrubbed[CS])
            + io(air(&self.ww_rn, IODINE) + held(&self.ww_rn, IODINE) + self.pool_scrubbed[IODINE])
            + te(air(&self.ww_rn, TE) + held(&self.ww_rn, TE) + self.pool_scrubbed[TE]);
        Some(FpDecayHeat { dw_gas_w, dw_wall_w, pool_w })
    }

    /// What arrives this step: the relief valves' into the pool at the
    /// quenchers, scrubbed by the condensation (SPARC-90's DF_EC), the rest
    /// into the wetwell gas; the leaks' into the drywell.
    pub(crate) fn arrive(&mut self, prm: &Params, s: &ContainmentSnapshot, pool_side: PoolSide) {
        let dt = s.dt_s;
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
        let (pool_t, p_ww) = (pool_side.t_k, pool_side.p_ww_pa);
        let pool = sparc::Pool::submerged(pool_t, p_ww, prm.vent_submergence_pa);
        let x_o = if s.scrubbing_saturation { pool.x_o() } else { 1.0 };
        let pass = if s.pool_scrubbing { condensation_pass(nc_share(s.srv_h2_kg_s / M_H2, 0.0, s.srv_steam_kg_s / M_H2O), x_o) } else { 1.0 };
        let relief_m3 = (s.srv_h2_kg_s / M_H2 + s.srv_steam_kg_s / M_H2O) * dt * R_GAS * pool_t / (p_ww + prm.vent_submergence_pa);
        if s.rn_relief.moved.is_empty() {
            let pass_i = self.iodine_vapour_pass(&pool, pool_side, s.pool_scrubbing && s.iodine_partition, pass, s.i_relief_kg, relief_m3);
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
            let pass_i = self.iodine_vapour_pass(&pool, pool_side, s.pool_scrubbing && s.iodine_partition, pass, rf.vapour_kg[IODINE], relief_m3);
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
    }

    /// The downcomers: the fraction `g.f` of the drywell's gas into the
    /// pool, scrubbed by the condensation, then each section by impaction
    /// and the swarm rise (`sparc`), with the cesium and iodine on that
    /// section, the I2 vapour by its partition limit and its own swarm
    /// rise; the rest into the wetwell gas.
    pub(crate) fn downcomer(&mut self, prm: &Params, s: &ContainmentSnapshot, g: DowncomerFlow, pool_side: PoolSide) {
        let dt = s.dt_s;
        let (f, n, dn2, dh2, dsteam) = (g.f, g.n, g.dn2, g.dh2, g.dsteam_kg);
        let (pool_t, p_ww, p_dw, t_dw) = (pool_side.t_k, pool_side.p_ww_pa, g.p_dw_pa, g.t_dw_k);
        let dxe = self.xe_dw * f;
        self.xe_dw -= dxe;
        self.xe_ww += dxe;
        let (moved, cv, ca) = self.dw_rn.take(f);
        // Through the downcomers into the pool: scrubbed likewise by the
        // condensation, then each section by impaction and the swarm
        // rise (`sparc`), with the cesium and iodine on that section.
        let pool = sparc::Pool::submerged(pool_t, p_ww, prm.vent_submergence_pa);
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
                    q_exit_m3_s: n * R_GAS * t_dw / p_dw / dt / n_v,
                };
                let d: Vec<f64> = (0..moved.len()).map(|k| self.dw_rn.sections.diameter(k)).collect();
                self.swarm_pass = sparc::pass_fractions(&pool, &inj, &d, rho_p);
                self.swarm_age_s = 0.0;
            }
            self.swarm_pass.clone()
        } else {
            vec![1.0; moved.len()]
        };
        let mut pass_i = self.iodine_vapour_pass(&pool, pool_side, s.pool_scrubbing && s.iodine_partition, pass, cv[IODINE], n * R_GAS * t_dw / p_dw);
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
                q_exit_m3_s: n * R_GAS * t_dw / p_dw / dt / n_v,
            };
            let p0 = p_ww + prm.vent_submergence_pa;
            let p_s = p_sat_pa(pool_t);
            let x_s = (p_s / p0).min(1.0);
            let gas_l = n_nc * R_GAS * pool_t / (p0 - p_s).max(1.0) * 1e3;
            let gases = [(BulkGas::Steam, x_s), (BulkGas::N2, (1.0 - x_s) * dn2 / n_nc), (BulkGas::H2, (1.0 - x_s) * dh2 / n_nc)];
            let (_, sum_aq, _) = self.pool_chemistry(&pool, pool_side.water_kg);
            let iv = sparc::IodineVapour {
                d_gas: FpClass::I2.diffusivity(&gases, pool_t, p0),
                d_liquid: sparc::i2_liquid_diffusivity(pool_t, p0),
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
        self.pool_scrubbed[AEROSOL] += moved.iter().sum::<f64>() * rho_p * (1.0 - pass) + moved.iter().zip(&swarm).map(|(m, p)| m * pass * (1.0 - p)).sum::<f64>() * rho_p;
        let moved: Vec<f64> = moved.iter().zip(&swarm).map(|(m, p)| m * pass * p).collect();
        let ca: [Vec<f64>; N_CLASSES] = std::array::from_fn(|k| ca[k].iter().zip(&swarm).map(|(m, p)| m * pass * p).collect());
        self.ww_rn.receive(&moved, std::array::from_fn(|k| cv[k] * pass_v[k]), &ca);
    }

    /// The fraction `f` of the wetwell gas into the drywell (the vacuum
    /// breakers), unscrubbed.
    pub(crate) fn ww_to_dw(&mut self, f: f64) {
        let dxe = self.xe_ww * f;
        self.xe_ww -= dxe;
        self.xe_dw += dxe;
        let (moved, cv, ca) = self.ww_rn.take(f);
        self.dw_rn.receive(&moved, cv, &ca);
    }

    /// The wetwell vent to the stack: the fraction `f_xe` of the wetwell's
    /// noble gas, and `f_rn` of its gas volume with what rides in it.
    pub(crate) fn vent(&mut self, f_xe: f64, f_rn: f64) {
        let dxe = self.xe_ww * f_xe;
        self.xe_ww -= dxe;
        self.xe_vented += dxe;
        let (_, cv, ca) = self.ww_rn.take(f_rn);
        self.cs_vented_kg += cv[CS] + ca[CS].iter().sum::<f64>();
        self.i_vented_kg += cv[IODINE] + ca[IODINE].iter().sum::<f64>();
        self.te_out_kg += cv[TE] + ca[TE].iter().sum::<f64>();
    }

    /// The drywell head flange to the building: the fraction `f` of the
    /// drywell's gas.
    pub(crate) fn flange(&mut self, f: f64) {
        let dxe = self.xe_dw * f;
        self.xe_dw -= dxe;
        self.xe_building += dxe;
        let (_, cv, ca) = self.dw_rn.take(f);
        self.cs_building_kg += cv[CS] + ca[CS].iter().sum::<f64>();
        self.i_building_kg += cv[IODINE] + ca[IODINE].iter().sum::<f64>();
        self.te_out_kg += cv[TE] + ca[TE].iter().sum::<f64>();
    }

    /// The aerosol clock: true, and its accumulated time in `h`, once
    /// [`RN_STEP_S`] has passed (the transport moves the fission products
    /// every step, the aerosol step every 10 s).
    pub(crate) fn rn_due(&mut self, dt: f64) -> Option<f64> {
        self.rn_clock_s += dt;
        if self.rn_clock_s >= RN_STEP_S {
            let h = self.rn_clock_s;
            self.rn_clock_s = 0.0;
            Some(h)
        } else {
            None
        }
    }

    /// Where the fission products are (as `ContainmentResult` reports them).
    pub fn report(&self) -> FpReport {
        FpReport {
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
        }
    }

    /// The pool water, L, and its dissolved iodine (as I2) and cesium (as
    /// CsOH), mol/L: what was scrubbed into it and what deposited on its
    /// surface from the wetwell gas.
    fn pool_chemistry(&self, pool: &sparc::Pool, water_kg: f64) -> (f64, f64, f64) {
        let litres = water_kg / pool.rho_l() * 1e3;
        let i_kg = self.pool_scrubbed[IODINE] + self.ww_rn.on_surface_kg[0][IODINE];
        let cs_kg = self.pool_scrubbed[CS] + self.ww_rn.on_surface_kg[0][CS];
        (litres, i_kg / iodine::M_I2 / litres, cs_kg / iodine::M_CS / litres)
    }

    /// The share of iodine vapour (`i_kg` in `gas_m3` of entering gas) that
    /// passes the condensation, given the gas's share `pass`: MELCOR's
    /// partition limit (Eq D-7, with the pool's `iodine_h`), and no
    /// scrubbing at all when the pool's own iodine over the partition is
    /// at least the gas's (RN §2.7.1). `on` false is the gas's share.
    fn iodine_vapour_pass(&self, pool: &sparc::Pool, side: PoolSide, on: bool, pass: f64, i_kg: f64, gas_m3: f64) -> f64 {
        if !on || pass >= 1.0 || gas_m3 <= 0.0 {
            return pass;
        }
        let (_, s_pool, _) = self.pool_chemistry(pool, side.water_kg);
        let c_in = i_kg / iodine::M_I2 / (gas_m3 * 1e3);
        if s_pool / self.iodine_h >= c_in {
            return 1.0;
        }
        let rho_v = p_sat_pa(side.t_k) * M_H2O / (R_GAS * side.t_k);
        let m = (self.iodine_h * rho_v / pool.rho_l()).min(1.0);
        // 1 / DF_EC,vap with DF_EC = 1 / pass, written without dividing
        // by `pass` (which is 0 for pure steam).
        if m >= 1.0 { pass } else { pass / (pass + m * (1.0 - pass)) }
    }

    /// The fission-product and aerosol step of both volumes (gh:#1047).
    /// Drywell: its floor and its structures (the wall heat sink's area and
    /// temperature), the vapour's mass-transfer coefficient from the
    /// sink's heat-transfer coefficient by the heat-mass analogy,
    /// `k = h/(rho c_p)` (Lewis number about 1, stated). Wetwell: the pool
    /// surface (a floor, at the pool temperature); its shell is not a
    /// deposition surface yet (stated).
    pub(crate) fn step_rn(&mut self, prm: &Params, c: &RnContext, dt: f64, phoresis: bool) {
        let n = self.dw_rn.sections.v.len();
        // Drywell gas.
        let n_s = c.dw_steam_kg / M_H2O;
        let n_tot = (c.dw_n2 + c.dw_h2 + n_s).max(1e-30);
        let gases_dw = [(BulkGas::Steam, n_s / n_tot), (BulkGas::N2, c.dw_n2 / n_tot), (BulkGas::H2, c.dw_h2 / n_tot)];
        let p_dw = c.p_dw_pa;
        let m_dw = Medium::air(c.dw_t_k, p_dw, air_viscosity(c.dw_t_k));
        let mass = c.dw_steam_kg + c.dw_n2 * M_N2 + c.dw_h2 * M_H2;
        let cp = (c.dw_steam_kg * CP_STEAM_GAS + c.dw_n2 * M_N2 * CP_N2_GAS + c.dw_h2 * M_H2 * CP_H2_GAS) / mass.max(1e-30);
        let rho = mass / c.dw_gas_m3;
        let h_wall = c.h_wall;
        let k_w = h_wall / (rho * cp);
        // Phoresis onto the drywell's structures (MELCOR RN §2.4.2.2):
        // thermophoresis from the gradient of their heat flux, `grad T =
        // q'' / k_air` at the surface temperature (Eq 2-43 and the text
        // after it), and diffusiophoresis from the steam condensed on them
        // since the last aerosol step (Eqs 2-45, 2-46), spread over their
        // area. Both surfaces are at the wall's temperature.
        let phoretic = if phoresis {
            let q = h_wall * (c.dw_t_k - c.dw_wall_t_k);
            let grad_t = q / air_conductivity(c.dw_wall_t_k);
            let w_cond = self.dw_condensed_kg / (dt * (prm.dw_floor_m2 + prm.dw_wall_area_m2));
            let n_nc = (c.dw_n2 + c.dw_h2).max(1e-30);
            let m_nc = (c.dw_n2 * M_N2 + c.dw_h2 * M_H2) / n_nc;
            let rho_sat = p_sat_pa(c.dw_t_k.min(640.0)) * M_H2O / (R_GAS * c.dw_t_k);
            let v_dp = diffusiophoretic_velocity(w_cond, n_s / n_tot, M_H2O, m_nc, rho, rho_sat);
            (0..n).map(|k| m_dw.thermophoretic_velocity(self.dw_rn.sections.diameter(k), grad_t, c.dw_wall_t_k) + v_dp).collect()
        } else {
            vec![0.0; n]
        };
        self.dw_condensed_kg = 0.0;
        let dw_surfaces = [
            RnSurface { surface: Surface { area: prm.dw_floor_m2, facing: Facing::Floor, phoretic: phoretic.clone() }, t_k: c.dw_wall_t_k, k_vapour: k_w },
            RnSurface { surface: Surface { area: prm.dw_wall_area_m2, facing: Facing::Wall, phoretic }, t_k: c.dw_wall_t_k, k_vapour: k_w },
        ];
        self.dw_rn.step(&m_dw, &gases_dw, &dw_surfaces, dt);
        // The pool's iodine partition (`iodine`), for the scrubbing until
        // the next aerosol step.
        let pool_t = c.pool.t_k;
        let pool = sparc::Pool::submerged(pool_t, c.pool.p_ww_pa, prm.vent_submergence_pa);
        let (_, s_i, oh) = self.pool_chemistry(&pool, c.pool.water_kg);
        self.iodine_h = iodine::partition(pool_t, s_i, oh, 0.0);
        // Wetwell gas, saturated with steam at the pool temperature (or the
        // atmosphere's own, with `ww_atmosphere`): the caller's `ww_x_steam`
        // and `ww_t_k`.
        let p_ww = c.pool.p_ww_pa;
        let (x_s, t_gas) = (c.ww_x_steam, c.ww_t_k);
        let n_nc = (c.ww_n2 + c.ww_h2).max(1e-30);
        let gases_ww = [(BulkGas::Steam, x_s), (BulkGas::N2, (1.0 - x_s) * c.ww_n2 / n_nc), (BulkGas::H2, (1.0 - x_s) * c.ww_h2 / n_nc)];
        let m_ww = Medium::air(t_gas, p_ww, air_viscosity(t_gas));
        let ww_surfaces = [RnSurface {
            surface: Surface { area: c.pool_area_m2, facing: Facing::Floor, phoretic: vec![0.0; n] },
            t_k: pool_t,
            k_vapour: k_w,
        }];
        self.ww_rn.step(&m_ww, &gases_ww, &ww_surfaces, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each location takes its share of the class inventory (prediction 15
    /// of `tests/containment_network_vs_containment.rs`, exact), nothing
    /// without decay power, and transport conserves what it moves.
    #[test]
    fn the_decay_heat_is_each_locations_share_and_transport_conserves() {
        let prm = Params::default();
        let mut fp = FpHold::new(prm.drywell_volume_m3, prm.wetwell_gas_m3);
        let s = ContainmentSnapshot { dt_s: 1.0, xe_leak: 1e-3, xe_relief: 2e-3, cs_leak_kg: 0.3, cs_relief_kg: 0.1, i_leak_kg: 0.05, uo2_leak_kg: 1.0, srv_steam_kg_s: 1.0, prm, ..Default::default() };
        assert_eq!(fp.decay_heat(&prm, &s), None);
        let side = PoolSide { t_k: 320.0, p_ww_pa: 2.0e5, water_kg: 1.7e6 };
        fp.begin_step(&s);
        fp.arrive(&prm, &s, side);
        fp.ww_to_dw(0.25);
        fp.flange(0.1);
        fp.vent(0.2, 0.2);
        let r = fp.report();
        assert!((r.xe_dw + r.xe_ww + r.xe_vented + r.xe_building - 3e-3).abs() < 1e-15, "{r:?}");
        let cs = r.cs_airborne_kg + r.cs_dw_structures_kg + r.cs_ww_pool_kg + r.cs_vented_kg + r.cs_building_kg;
        assert!((cs - 0.4).abs() < 1e-15, "{r:?}");
        let s = ContainmentSnapshot { q_noble_w: 3.0e6, q_alkali_w: 2.0e6, q_halogen_w: 1.0e6, ..s };
        let q = fp.decay_heat(&prm, &s).unwrap();
        let expect = (r.xe_dw + r.xe_ww) * 3.0e6 + (r.cs_airborne_kg + r.cs_dw_structures_kg + r.cs_ww_pool_kg) / prm.cs_inventory_kg * 2.0e6 + (r.i_airborne_kg + r.i_dw_structures_kg + r.i_ww_pool_kg) / prm.i_inventory_kg * 1.0e6;
        let got = q.dw_gas_w + q.dw_wall_w + q.pool_w;
        assert!(((got - expect) / expect).abs() < 1e-12, "{q:?} {expect}");
        assert!(q.dw_gas_w > 0.0 && q.pool_w > 0.0 && q.dw_wall_w == 0.0, "{q:?}");
        assert_eq!(fp.rn_due(4.0), None);
        assert_eq!(fp.rn_due(6.0), Some(10.0));
    }
}
