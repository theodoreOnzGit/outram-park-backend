//! The plant: Fukushima Daiichi Unit 1 after the earthquake's scram.
//!
//! A BWR/3 in a Mark I containment. This is the model the `dhoby-ghaut`
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
//! | [`containment::Containment`] | drywell, wetwell, pool, vent, reactor building | the containment worker |
//!
//! Each step, [`Plant::begin`] reads the vessel at the start of the step and
//! hands each worker a **snapshot**; the core and the containment advance
//! from it; [`Plant::finish`] takes their results and closes the vessel's
//! mass and energy balance. The coupling is explicit across the step (each
//! part sees the others' start-of-step state), the same splitting CIET v2
//! uses for its flows. Because the snapshots and results cross as exact
//! `f64`s ([`wire`]), the three-worker step is bit-for-bit the one-worker
//! step ([`Plant::step_inline`]); `tests::three_workers_are_one_worker`
//! holds that.
//!
//! **AI draft, not validated.** See [`params`] for where every number comes
//! from (many are placeholders) and each module's "Not in the model" list.
//! Research, education and V&V only: never for operation, licensing, safety
//! decisions or emergency response.

pub mod compare;
pub mod containment;
pub mod core;
pub mod params;
pub mod steam;
pub mod wire;

use petir::mathf::RealMath;
use containment::{Containment, ContainmentResult, ContainmentSnapshot};
use self::core::{Core, CoreResult, CoreSnapshot};
use tampines_steam_tables::prelude::get_choked_flow_massrate_and_state_from_stagnation_properties_and_area;
use teh_o_prke::decay_heat::{DecayHeat, FissioningNuclide};
use params::*;
use steam::{Sat, SatTable};
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::{Area, AvailableEnergy, Power, Pressure, Time};
use uom::si::mass_rate::kilogram_per_second;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::time::second;
use wire::{Reader, Writer};

/// The plant's time step, s.
pub const DT_S: f64 = 1.0;
/// Seconds of the accident the historical sequence runs to (30 h).
pub const SCENARIO_END_S: f64 = 30.0 * 3600.0;

/// Who drives the operator actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The recorded Unit 1 sequence (INPO 11-005 timeline, \[U\] times).
    Historical,
    /// The reader's own actions.
    FreePlay,
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
/// The other five times (IC stop, IC cycling, station blackout, the three
/// injection changes) have no BSAF anchor and stay `[U]` until checked
/// against INPO 11-005, IAEA TV1 or the Japanese government report (#1031).
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
}

pub const EVENTS: [Event; 9] = [
    Event { t_s: 17.0 * 60.0, label: "15:03 Operators stop both IC trains (cooldown > 55 °C/h)", kind: EventKind::IcStopped },
    Event { t_s: 31.0 * 60.0, label: "15:17 Train A cycled to hold pressure", kind: EventKind::IcCycleA },
    Event { t_s: 51.0 * 60.0, label: "15:37 Tsunami: station blackout, IC valves fail closed", kind: EventKind::StationBlackout },
    Event { t_s: (14.0 * 60.0 + 59.0) * 60.0, label: "05:46 (12 Mar) Fresh-water injection by fire engine", kind: EventKind::InjectionOn },
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
    pub core: CoreResult,
    pub containment: ContainmentResult,
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
        let n = 80;
        let (a, b) = ((2.0e5f64).r_ln(), steam::P_MAX_PA.r_ln());
        let ln_p: Vec<f64> = (0..n).map(|i| a + (b - a) * i as f64 / (n - 1) as f64).collect();
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
    /// The inputs this plant runs with ([`Params`]); fixed at construction.
    prm: Params,
    t_s: f64,
    /// Vessel water and steam: mass, kg, internal energy, J; hydrogen, kg.
    m: f64,
    u: f64,
    h2: f64,
    decay: DecayHeat,
    ic_shell_k: [f64; IC_TRAINS],
    ic_shell_kg: [f64; IC_TRAINS],
    ic_autostarted: bool,
    ic_a_cycling: bool,
    relief_open: bool,
    hot_leak_open: bool,
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
        let sat = SatTable::new();
        let choked = ChokedTable::new(&sat);
        let s = sat.at(prm.initial_pressure_pa);
        let v_total = volume_below(&prm, prm.rpv_height_m);
        let v_liq = volume_below(&prm, prm.baf_m + prm.active_length_m + prm.normal_level_above_taf_m);
        let (m_l, m_g) = (v_liq / s.v_f, (v_total - v_liq) / s.v_g);
        let decay = DecayHeat::new_at_equilibrium(FissioningNuclide::U235Thermal, Power::new::<watt>(prm.thermal_power_w));
        let mut p = Self {
            sat,
            choked,
            mode,
            mechanisms,
            controls: Controls::default(),
            prm,
            t_s: 0.0,
            m: m_l + m_g,
            u: m_l * s.u_f + m_g * s.u_g,
            h2: 0.0,
            decay,
            ic_shell_k: [prm.initial_pool_k; IC_TRAINS],
            ic_shell_kg: [prm.ic_shell_water_kg; IC_TRAINS],
            ic_autostarted: false,
            ic_a_cycling: false,
            relief_open: false,
            hot_leak_open: false,
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

    /// A fresh core and containment matching a fresh plant.
    pub fn parts(&self) -> (Core, Containment) {
        (Core::new(self.vessel.sat.t_k, &self.prm), Containment::new(&self.prm))
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

    /// The operators' actions from the historical sequence, at time `t`.
    fn apply_history(&mut self) {
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
        // Isolation condensers: automatic start on high pressure (needs DC).
        if self.controls.dc_power && !self.ic_autostarted && p >= prm.ic_autostart_pa {
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
        let flux = self.choked.at(p);
        let w_relief = if self.relief_open { flux * prm.relief_throat_m2 * prm.relief_valves } else { 0.0 };
        if self.mechanisms.hot_leak && self.last_core.exit_gas_k >= prm.hot_leak_gas_k {
            self.hot_leak_open = true;
        }
        let leak_area = if self.hot_leak_open { prm.hot_leak_m2 } else { 0.0 } + if self.last_core.lower_head_failed { prm.breach_m2 } else { 0.0 };
        let w_leak = if p > p_dw { flux * leak_area } else { 0.0 };
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
        let scale = if out > 0.0 { ((steam_gas + self.h2) / out).min(1.0) } else { 1.0 };
        let (w_relief, w_leak) = (w_relief * scale, w_leak * scale);
        let decay_w = self.decay.total_decay_heat_power().get::<watt>();
        // Before the MSIVs close, the pressure regulator passes steam to the
        // turbine bypass.
        let w_bypass = if self.t_s < prm.msiv_closure_s && !v.dry {
            (prm.bypass_gain_kg_s_pa * (p - prm.initial_pressure_pa)).clamp(0.0, steam_gas / DT_S)
        } else {
            0.0
        };
        self.pending = Pending {
            w_bypass,
            q_ic_w: q_ic_train.iter().sum(),
            q_ic_train,
            w_relief,
            w_leak,
            w_inj,
            h2_out: (w_relief + w_leak) * h2_share,
        };
        let h_gas = v.sat.h_g;
        Snapshots {
            core: CoreSnapshot { dt_s: DT_S, p_pa: v.sat.p_pa, t_sat_k: t_sat, h_fg: v.sat.h_g - v.sat.h_f, level_m: v.level_m, decay_w, t_dependent_props: self.mechanisms.temperature_dependent_properties, p_dw_pa: self.last_containment.p_dw_pa, lower_head_wall: self.mechanisms.lower_head_wall, rod_collapse: self.mechanisms.rod_collapse, core_radiation: self.mechanisms.core_radiation, prm },
            containment: ContainmentSnapshot {
                dt_s: DT_S,
                srv_steam_kg_s: w_relief * (1.0 - h2_share),
                srv_h: h_gas,
                srv_h2_kg_s: w_relief * h2_share,
                leak_steam_kg_s: w_leak * (1.0 - h2_share),
                leak_h: h_gas,
                leak_h2_kg_s: w_leak * h2_share,
                leak_gas_k: self.last_core.exit_gas_k,
                vent_ordered: self.controls.vent_ordered,
                ex_vessel_w: self.last_core.ex_vessel_w,
                pedestal_inflow_kg_s: if self.last_core.lower_head_failed { w_inj } else { 0.0 },
                prm,
            },
        }
    }

    /// Finish the step with the core's and the containment's results.
    pub fn finish(&mut self, core: CoreResult, containment: ContainmentResult) {
        let prm = self.prm;
        let d = self.pending;
        let v = self.vessel;
        let h_g = v.sat.h_g;
        let h_inj = 4186.0 * (prm.injection_temperature_k - 273.15);
        let gas_out = d.w_relief + d.w_leak;
        let steam_out = gas_out - d.h2_out + d.w_bypass;
        // After the lower head fails the injected water pours through the
        // breach onto the drywell floor (the containment's pedestal).
        let w_inj_rpv = if self.last_core.lower_head_failed { 0.0 } else { d.w_inj };
        self.m += (w_inj_rpv - steam_out - core.steam_used_kg_s) * DT_S;
        // Hydrogen leaving takes the enthalpy above 298 K that the core's
        // gas stream gave it, which reached the water through `q_fluid_w`
        // (gh:#1046): at saturation through the relief valves, at the core
        // exit gas temperature through the leak (as the containment
        // receives it). Wetwell gas has no energy equation, so the relief
        // path's share is not delivered there (stated limitation).
        let to_relief = if gas_out > 0.0 { d.w_relief / gas_out } else { 0.0 };
        let h2_enthalpy_out = d.h2_out
            * self::core::CP_H2_J_PER_KG_K
            * (to_relief * (v.sat.t_k - self::core::T_298_K) + (1.0 - to_relief) * (self.last_core.exit_gas_k - self::core::T_298_K));
        self.u += (core.q_fluid_w - d.q_ic_w + w_inj_rpv * h_inj - steam_out * h_g - core.steam_used_kg_s * h_g - h2_enthalpy_out) * DT_S;
        self.h2 = (self.h2 + (core.h2_kg_s - d.h2_out) * DT_S).max(0.0);
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
        self.decay.advance_timestep(Power::new::<watt>(0.0), Time::new::<second>(DT_S));
        self.t_s += DT_S;
        self.last_core = core;
        self.last_containment = containment;
        self.vessel = self.solve_vessel();
        // Once the lower head has failed the vessel is open to the drywell:
        // its pressure is the drywell's (no back-flow model; what is left
        // inside is a few tonnes of steam and gas).
        if self.last_core.lower_head_failed {
            self.vessel.p_pa = self.last_containment.p_dw_pa;
        }
    }

    /// One whole step with the core and containment advanced here, in
    /// order: what the one-worker mode (and natively, the engine thread)
    /// runs.
    pub fn step_inline(&mut self, core: &mut Core, containment: &mut Containment) {
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
            decay_w: self.decay.total_decay_heat_power().get::<watt>(),
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
            core: self.last_core.clone(),
            containment: self.last_containment,
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
        ]);
        self.core.write(w);
        self.containment.write(w);
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
            core: CoreResult::read(r),
            containment: ContainmentResult::read(r),
        }
    }
}

/// The CSV the headless mode prints: a stable header, fixed precision.
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
        | (f.vessel.dry as u32) << 10;
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
pub fn headless_csv(steps: usize, every: usize) -> String {
    let mut plant = Plant::new(Mode::Historical);
    let (mut core, mut cont) = plant.parts();
    let mut out = String::from(CSV_HEADER);
    out.push('\n');
    for i in 0..steps {
        plant.step_inline(&mut core, &mut cont);
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

    /// The core and containment work from snapshots that cross a worker
    /// boundary as flat f64s; that gives the in-place step bit for bit.
    /// Methodology: run 2000 steps of the historical sequence in place, and
    /// again with every snapshot and result written to the wire format and
    /// read back on the other side (what the three-worker mode does); compare
    /// every field of the frame after every step with `==` on the f64s.
    #[test]
    fn three_workers_are_one_worker() {
        one_worker_against_three(Params::default(), 2000);
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
    /// for frame bit-identical, and returns the last frame.
    fn one_worker_against_three(prm: Params, steps: usize) -> Frame {
        let mut a = Plant::with_params(Mode::Historical, Mechanisms::default(), prm);
        let (mut ca, mut ka) = a.parts();
        let mut b = Plant::with_params(Mode::Historical, Mechanisms::default(), prm);
        let (mut cb, mut kb) = b.parts();
        for _ in 0..steps {
            a.step_inline(&mut ca, &mut ka);
            let s = b.begin();
            let mut w = Writer::default();
            s.core.write(&mut w);
            let core_snap = CoreSnapshot::read(&mut Reader::new(&w.0));
            let mut w = Writer::default();
            s.containment.write(&mut w);
            let cont_snap = ContainmentSnapshot::read(&mut Reader::new(&w.0));
            // The worker gets the state and the snapshot, steps a copy of
            // its own, and sends back the new state and the result.
            let (core_res, cont_res);
            (cb, core_res) = remote_core(&cb, &core_snap);
            (kb, cont_res) = remote_containment(&kb, &cont_snap);
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
    #[test]
    fn the_headless_run_matches_the_committed_fixture() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/verification_and_validation/bwr_mark1/headless_30h.csv");
        let now = headless_csv(108_000, 600);
        if std::env::var("FUKUSHIMA_BLESS").is_ok() {
            std::fs::write(path, &now).expect("write the fixture");
        }
        let fixture = std::fs::read_to_string(path).expect("the committed fixture");
        assert!(now == fixture, "the headless trace changed; re-bless only if the change is deliberate");
    }

    /// Physical bounds over the whole recorded sequence, every step: positive
    /// pressures below the vessel's design range, cladding between the
    /// coolant and 2600 K, a level inside the vessel, hydrogen never
    /// decreasing. A harness check that catches divergence, not physics V&V.
    #[test]
    fn the_whole_sequence_stays_in_physical_bounds() {
        let mut p = Plant::new(Mode::Historical);
        let (mut c, mut k) = p.parts();
        let mut h2 = 0.0;
        for _ in 0..108_000 {
            p.step_inline(&mut c, &mut k);
            let f = p.frame();
            assert!(f.vessel.p_pa > 5.0e4 && f.vessel.p_pa < 9.5e6, "p {} at {}", f.vessel.p_pa, f.t_s);
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
    #[test]
    fn the_vessel_conserves_mass_and_energy() {
        let mut p = Plant::new(Mode::Historical);
        let (mut c, mut k) = p.parts();
        let (m0, u0) = (p.m, p.u);
        let (mut dm, mut du) = (0.0, 0.0);
        for _ in 0..3600 {
            let h_g = p.vessel.sat.h_g;
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
            let h2_out = d.h2_out * self::core::CP_H2_J_PER_KG_K * (share * (t_sat - 298.15) + (1.0 - share) * (p.last_core.exit_gas_k - 298.15));
            du += (cr.q_fluid_w - d.q_ic_w + w_inj * 4186.0 * (INJECTION_TEMPERATURE_K - 273.15) - steam_out * h_g - h2_out) * DT_S;
            p.finish(cr, kr);
        }
        assert!(((p.m - m0) - dm).abs() <= 1e-9 * m0, "mass {} vs {}", p.m - m0, dm);
        assert!(((p.u - u0) - du).abs() <= 1e-9 * u0.abs(), "energy {} vs {}", p.u - u0, du);
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
