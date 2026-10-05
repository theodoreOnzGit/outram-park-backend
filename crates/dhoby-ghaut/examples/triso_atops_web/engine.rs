//! **The engine**: every calculation the TRISO-ATOPS demo shows, run off the
//! UI thread (the no-lagging HARD RULE) through `dhoby_ghaut::web_demo::link`.
//! Each request is one short calculation (a slice of at most ~4·10⁴ region
//! lookups, a frame of a few hundred Walk-on-Spheres walkers, a few hundred
//! accident steps), so a worker answers it in milliseconds to tens of
//! milliseconds and the UI never waits.
//!
//! **No physics is written here.** Every number is a call into `boon-lay`,
//! the crate the lessons teach (`tutorials/triso-atops/`):
//!
//! | rung | call |
//! |---|---|
//! | 1 `triso` | `TrisoCell::get_triso_region` at every pixel of a slice (the geometry the solver sees, never the constants) |
//! | 2 `decay` | `DecayLibrary` (ENDF/B-VIII.0 decay data compiled into the crate), `StochasticDecayChain::new_single_stochastic_chain_from_nuclide`, `SingleNuclideSimulatorMC::get_time_to_decay_stochastic` |
//! | 3 `walk` | CRP-6 Case 1 animated: `WoSWalker::hop` in an assembled `TrisoCell`, each walker absorbed when it reaches the kernel surface, against Crank's `calculate_analytical_fraction_released`; the batch check is `mc_kernel_release_fraction` itself |
//! | 4 `layers` | `try_get_diffusion_coeff_jiang` per layer, `first_passage::interface::transmission_probability` |
//! | 5 `failure` | `fuel_failure::htr10::particle` / `end_of_irradiation_failure`, `AccidentHistory::step`, `pressure_at`, `induced_stress_with_thinning_factor` |
//! | 6 `chemistry` | `chemistry::{graphite_air, graphite_steam, kernel_hydrolysis}` |
//! | 7 `release` | every nuclide of the inventory that TRISO-ATOPS's table carries (19 of 22), each through `normal_operation_node`'s chain: `diffusion_coefficient`, `rb_fail`, `release_rate`, `base_activities`, upstream's removal routing, then `live_pools::step` through rung 5's history; inventory from `changi::activity::inventory` (Liu & Cao 2002 Table 1); the pool diagram's arrows are the terms of `live_pools`' balances evaluated on the pools it returned |
//!
//! The demo's own logic is bookkeeping only: which atom is which species at a
//! time (rung 2), slicing each walker's walk into frames (rung 3: hop while
//! the walker's own clock is behind the frame time, the loop
//! `walk_to_absorbing_sphere` runs to the end), which of a drawn population of particles counts as failed
//! at a failure fraction (rung 5, an illustration of the fraction, stated as
//! such), and stepping the pools at the accident steps (rung 7).
//!
//! **Calls this engine must not make, because they panic** (a panic in the
//! Web Worker aborts it): the Kr-in-buffer diffusion coefficient is a
//! `todo!()` (gh:#541), so the layers rung omits that curve and says so; and
//! `SingleNuclideSimulatorMC::advance_timestep` reads `SystemTime`, which
//! panics on `wasm32-unknown-unknown`, so rung 2 samples the lifetimes with
//! the same sampler directly instead of stepping a simulator.
//!
//! **Why rung 3 animates the bare kernel, not the five layers.** The live
//! multilayer ensemble (`first_passage::live::LiveEnsemble`) cannot advance
//! in real time on a CPU: a walker that reaches an interface it rarely
//! crosses is reinserted `2 * capture_eps` = 20 nm from it, so each of its
//! hops adds nanoseconds. Measured with `multilayer_bounce_cost_probe`
//! (2026-10-05, native, one core): 2·10⁷ steps (3.1·10⁶ of them interface
//! events) advanced one Cs-137 walker born in the buffer by only 221 s at
//! 1200 °C and 949 s at 1600 °C, in about 3 s of wall time; release through
//! the SiC takes days. A frame of 400 walkers did not finish in 300 s
//! (gh:#551).

use boon_lay::chemistry::{graphite_air, graphite_steam, kernel_hydrolysis};
use boon_lay::fuel_failure::history::{AccidentHistory, AccidentStep};
use boon_lay::fuel_failure::htr10;
use boon_lay::fuel_failure::induced_stress_with_thinning_factor;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::central_limit_theorem::oorandom_rng::OoRng64;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::interface::transmission_probability;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::walk_on_spheres::{
    sample_uniform_in_ball, HopOutcome, WoSWalker,
};
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::constructive_solid_geometry::{
    TrisoCell, TrisoRegion,
};
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::release_fraction_analytical_solution::calculate_analytical_fraction_released;
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::release_fraction_crp_6_case_1a_1b::simulation_code::{
    kernel_diffusion_coefficient, mc_kernel_release_fraction,
};
use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::temperature_dependent_collisions::{
    try_get_diffusion_coeff_jiang, TrisoPebbleLayerMaterial,
};
use boon_lay::lagrangian_decay_simulator::StochasticDecayChain;
use boon_lay::prelude::decay_library::DecayLibrary;
use boon_lay::prelude::{HalfLifeAndDecayEnergyInfo, SingleNuclideSimulatorMC};
use boon_lay::triso_atops_fork::activities::live_pools::{self, PoolRates, PrimaryPools};
use boon_lay::triso_atops_fork::activities::source_terms::{
    base_activities, release_rate, FailureFractions,
};
use boon_lay::triso_atops_fork::diffusion::diffusion_coefficient;
use boon_lay::triso_atops_fork::nuclide_model::nuclide_database::find_nuclide;
use boon_lay::triso_atops_fork::release_models::rb_fail;
use boon_lay::triso_atops_fork::TrisoAtopsNuclide;
use boon_lay::triso_atops_fork::ElementGroup;
use boon_lay::Nuclide;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::{
    ArealNumberDensity, DiffusionCoefficient, Frequency, Length, Pressure,
    ThermodynamicTemperature, Time,
};
use uom::si::frequency::hertz;
use uom::si::length::{meter, micrometer};
use uom::si::pressure::{kilopascal, megapascal};
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::degree_celsius;
use uom::si::time::{day, hour, second};
use uom::ConstZero;

/// Atoms in the decay rung (one stochastic chain each).
pub const DECAY_ATOMS: usize = 500;
/// Walkers in the walk rung's animated CRP-6 Case 1.
pub const WALKERS: usize = 400;
/// Hops one walker may take in one frame (a guard: a kernel walker needs a
/// few dozen to be absorbed).
const HOPS_PER_FRAME: usize = 10_000;
/// Histories per time point in the CRP-6 Case 1 check.
pub const CHECK_HISTORIES: usize = 400;
/// Particles in the failure rung's drawn population (a 20 × 20 grid).
pub const POPULATION: usize = 400;
/// Accident steps per transient (rungs 5 and 7).
pub const ACCIDENT_STEPS: usize = 200;

/// The decay rung's nuclides: label, `Nuclide`.
pub const DECAY_NUCLIDES: [(&str, Nuclide); 6] = [
    ("Kr-88", Nuclide::Kr88),
    ("I-135", Nuclide::I135),
    ("Xe-133", Nuclide::Xe133),
    ("I-131", Nuclide::I131),
    ("Cs-137", Nuclide::Cs137),
    ("Sr-90", Nuclide::Sr90),
];

/// The layers rung's nuclides (the walker's own coefficient table).
pub const LAYER_NUCLIDES: [(&str, Nuclide); 4] = [
    ("Cs-137", Nuclide::Cs137),
    ("Sr-90", Nuclide::Sr90),
    ("Ag-110m", Nuclide::Ag110m),
    ("Kr-85", Nuclide::Kr85),
];


/// The release rung's frame, for every nuclide at once (the UI sums or
/// filters them). `scalars` = `[n_nuclides, n_points]`, then per nuclide
/// [`REL_HEADER`] values (inventory Bq, lambda 1/s, and the k_plate, k_clean,
/// k_leak it was routed, 1/s) followed by [`REL_PER_POINT`] values per point
/// (indices in [`rel`]). `names` are the nuclides in that order, then the
/// inventory nuclides TRISO-ATOPS does not model, prefixed
/// [`RELEASE_SKIPPED`]; `tags` the transport group of each modelled one
/// ([`GROUPS`]); `series[0].xs` the hours of the hold.
pub const REL_HEADER: usize = 5;
pub const REL_PER_POINT: usize = 12;
pub const RELEASE_SKIPPED: &str = "skip:";

/// Indices within one point of the release frame. Pools in atoms, flows in
/// atoms/s: they are the terms of `live_pools`' balances (its module doc)
/// evaluated on the pools it returned, plus `base_activities`' graphite
/// hold-up `G`.
pub mod rel {
    /// Source into the helium `S`.
    pub const S: usize = 0;
    /// Circulating `C`, plated `P`, purification `H`, cumulative leaked `L`.
    pub const C: usize = 1;
    pub const P: usize = 2;
    pub const H: usize = 3;
    pub const LEAKED: usize = 4;
    /// `k_plate C`, `k_clean C`, `k_leak C`.
    pub const Q_PLATE: usize = 5;
    pub const Q_CLEAN: usize = 6;
    pub const Q_LEAK: usize = 7;
    /// Decay `lambda C`, `lambda P`, `lambda H`.
    pub const D_C: usize = 8;
    pub const D_P: usize = 9;
    pub const D_H: usize = 10;
    /// Held in the fuel element's matrix graphite (`base_activities`' `G`).
    pub const G: usize = 11;
}

/// `(n_nuclides, n_points)` of a release frame (0, 0 for another rung's).
pub fn release_dims(f: &Frame) -> (usize, usize) {
    let n = f.scalars.first().copied().unwrap_or(0.0) as usize;
    let p = f.scalars.get(1).copied().unwrap_or(0.0) as usize;
    if f.scalars.len() == 2 + n * (REL_HEADER + p * REL_PER_POINT) {
        (n, p)
    } else {
        (0, 0)
    }
}

/// Header value `k` of nuclide `nuc` (0 inventory Bq, 1 lambda, 2-4 rates).
pub fn release_header(f: &Frame, nuc: usize, k: usize) -> f64 {
    let (_, p) = release_dims(f);
    f.scalars[2 + nuc * (REL_HEADER + p * REL_PER_POINT) + k]
}

/// Value `k` ([`rel`]) of nuclide `nuc` at point `pt`.
pub fn release_value(f: &Frame, nuc: usize, pt: usize, k: usize) -> f64 {
    let (_, p) = release_dims(f);
    f.scalars[2 + nuc * (REL_HEADER + p * REL_PER_POINT) + REL_HEADER + pt * REL_PER_POINT + k]
}

/// TRISO-ATOPS's transport groups, in the order of the frame's `tags`.
pub const GROUPS: [&str; 5] = ["noble gases", "halogens", "Cs, Sr (special metals)", "silver", "other"];

fn group_index(g: ElementGroup) -> usize {
    match g {
        ElementGroup::NobleGas => 0,
        ElementGroup::Halogen => 1,
        ElementGroup::SpecialMetal => 2,
        ElementGroup::Silver => 3,
        ElementGroup::Other => 4,
    }
}

/// NP-MHTGR reference geometry and grain size, the case TRISO-ATOPS ships
/// (pinned against upstream `de374c8` in
/// `boon-lay/tests/triso_atops_fork_verification.rs`; `a_grain` from Stoyer
/// et al. 2026 Case A Table 3, as `sembawang`'s `NormalOperation` carries
/// it). The metals' `<R/B>_fail` needs them; the volatiles' does not.
const NP_MHTGR_KERNEL_RADIUS_M: f64 = 213.0e-6;
const NP_MHTGR_SIC_THICKNESS_M: f64 = 35.0e-6;
const NP_MHTGR_GRAIN_SIZE_M: f64 = 1.0e-5;
/// The plant's run before the accident, for the pools' starting state:
/// NP-MHTGR reference, 40 upstream years (Stoyer Case A, `sembawang`'s
/// `NormalOperation::np_mhtgr_reference`). Long-lived nuclides are NOT at
/// equilibrium after it (Cs-137 plate-out still growing), as in upstream's
/// `normal_operation`.
const NP_MHTGR_RUN_TIME_S: f64 = 40.0 * 365.0 * 86_400.0;

/// One nuclide through the hold: the same chain as
/// `normal_operation_node` (diffusion coefficients, `rb_fail`,
/// `release_rate`, `base_activities`, upstream's group routing of the
/// removal constants), evaluated at the irradiation temperature for normal
/// operation and at the hold temperature (with that step's in-service
/// failure) for each step, then carried through the pools by
/// `live_pools::step`. Returns the nuclide's block of the frame and its
/// group.
#[allow(clippy::too_many_arguments)]
fn release_one(
    nuc: &TrisoAtopsNuclide,
    inventory_bq: f64,
    t_irr: Time,
    pts: &[AccidentPoint],
    irr_c: f64,
    hold_c: f64,
    f_hm: f64,
    [k_plate, k_clean, k_leak]: [f64; 3],
) -> (Vec<f64>, ElementGroup) {
    let group = nuc.element_group();
    let lambda = nuc.decay_constant();
    let lam = lambda.get::<hertz>();
    let short_lived = nuc.half_life.get::<second>() / t_irr.get::<second>() < 0.2;
    // Upstream's routing (normal_operation_node): noble gases do not plate
    // out; the HPS scrubs noble gases and halogens only.
    let (kp, kc) = match group {
        ElementGroup::NobleGas => (0.0, k_clean),
        ElementGroup::Halogen => (k_plate, k_clean),
        _ => (k_plate, 0.0),
    };
    let rates = PoolRates {
        decay: lam,
        plate_out: kp,
        clean_up: kc,
        leak: k_leak,
    };
    let source = |temp_c: f64, f_inc: f64| -> (f64, f64) {
        let t = celsius(temp_c);
        let diff = diffusion_coefficient(nuc.z, t, t);
        let rb = rb_fail(
            nuc.z,
            short_lived,
            lambda,
            t,
            t_irr,
            Length::new::<meter>(NP_MHTGR_GRAIN_SIZE_M),
            Length::new::<meter>(NP_MHTGR_SIC_THICKNESS_M),
            Length::new::<meter>(NP_MHTGR_KERNEL_RADIUS_M),
            diff.kernel,
        );
        let fractions = FailureFractions {
            heavy_metal: f_hm,
            sic: NP_MHTGR_F_SIC,
            incremental: f_inc,
            incremental_sic: NP_MHTGR_F_INC_SIC,
        };
        let r = release_rate(
            rb,
            group,
            fractions,
            Frequency::new::<hertz>(inventory_bq),
            short_lived,
            t_irr,
            lambda,
        );
        let sg = base_activities(
            group,
            lambda,
            t_irr,
            Length::new::<meter>(NP_MHTGR_A_GRAPH_M),
            diff.graphite,
            r,
        );
        (sg.source_rate, sg.graphite_activity)
    };
    // Normal operation: the plant's run from empty pools, at the
    // irradiation temperature (upstream's normal_operation does the same with
    // its closed forms; live_pools reproduces them from empty).
    let (s0, g0) = source(irr_c, pts[0].in_service);
    let (mut pools, _) = live_pools::step(PrimaryPools::default(), s0, rates, NP_MHTGR_RUN_TIME_S);
    pools.leaked = 0.0;
    let mut out = vec![inventory_bq, lam, kp, kc, k_leak];
    let mut push = |s: f64, g: f64, pools: &PrimaryPools| {
        let c = pools.circulating;
        out.extend_from_slice(&[
            s,
            c,
            pools.plate_out,
            pools.clean_up,
            pools.leaked,
            kp * c,
            kc * c,
            k_leak * c,
            lam * c,
            lam * pools.plate_out,
            lam * pools.clean_up,
            g,
        ]);
    };
    push(s0, g0, &pools);
    for w in pts.windows(2) {
        // The step's source at the hold temperature and the step's end
        // failure fraction, held over the step.
        let (s, g) = source(hold_c, w[1].in_service);
        let dt = (w[1].hours - w[0].hours) * 3600.0;
        pools = live_pools::step(pools, s, rates, dt).0;
        push(s, g, &pools);
    }
    (out, group)
}

/// NP-MHTGR reference fractions upstream TRISO-ATOPS ships, pinned against
/// upstream `de374c8` in `boon-lay/tests/triso_atops_fork_verification.rs`
/// and cited in `sembawang::accident::release`: as-fabricated SiC defects
/// and incremental SiC failure. They do not enter a volatile's release
/// (`release_rate` uses `f_hm + f_inc` for noble gases and halogens); they
/// are carried so the fractions are the upstream case's.
const NP_MHTGR_F_SIC: f64 = 1.0e-4;
const NP_MHTGR_F_INC_SIC: f64 = 3.6e-5;
/// NP-MHTGR graphite thickness `a_graph`, m (same case). Unused by
/// `base_activities` for volatiles, which pass straight to the coolant.
const NP_MHTGR_A_GRAPH_M: f64 = 0.0045;

/// The two particles of rung 1: label and five radii, µm.
pub const GEOMETRIES: [(&str, [f64; 5]); 2] = [
    // TrisoCell::new_crp6_geometry (Hales et al. 2013).
    ("CRP-6 (Hales et al. 2013)", [212.5, 312.5, 352.5, 387.5, 427.5]),
    // IAEA-TECDOC-1382 pt 2 Table 4-17, as boon-lay's triso_cell_slice
    // example builds it.
    ("HTR-10 (TECDOC-1382 Table 4-17)", [250.0, 340.0, 380.0, 415.0, 455.0]),
];

/// Everything the UI can ask for.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// A z = 0 slice of an assembled `TrisoCell`, `cells × cells`.
    Slice { id: u32, geometry: u8, cells: u32 },
    /// Every atom's species at `t_over_half` parent half-lives.
    Decay {
        id: u32,
        nuclide: u8,
        t_over_half: f64,
    },
    /// Advance the CRP-6 Case 1 walkers by `dt_s` simulated seconds (rebuilt
    /// at `temp_c` when `reset`).
    Walk {
        id: u32,
        reset: bool,
        temp_c: f64,
        dt_s: f64,
    },
    /// CRP-6 Case 1 at `temp_c`: Walk-on-Spheres against Crank over 200 h.
    WalkCheck { id: u32, temp_c: f64 },
    /// `D(T)` in every layer for one nuclide, and the PyC → SiC transmission
    /// probability at `temp_c`.
    Layers { id: u32, nuclide: u8, temp_c: f64 },
    /// An isothermal accident hold after irradiation at `irr_c`.
    Failure {
        id: u32,
        irr_c: f64,
        hold_c: f64,
        hours: f64,
        cursor_h: f64,
    },
    /// Oxidation rates and hydrolysis against temperature.
    Chemistry {
        id: u32,
        o2_kpa: f64,
        steam_kpa: f64,
        h2_kpa: f64,
    },
    /// Rung 5's transient carried into the coolant pools, every nuclide.
    Release {
        id: u32,
        irr_c: f64,
        hold_c: f64,
        hours: f64,
        f_hm: f64,
        k_plate: f64,
        k_clean: f64,
        k_leak: f64,
    },
}

/// A line or a set of markers on a plot.
#[derive(Clone, Debug, PartialEq)]
pub struct Series {
    pub label: String,
    /// 0 solid (published), 1 dotted (ours), 2 dashed (reference or
    /// extrapolated), 3 markers only.
    pub style: u8,
    /// Index into the app's palette.
    pub colour: u8,
    /// Which plot panel it belongs to (0 top, 1 bottom).
    pub panel: u8,
    pub xs: Vec<f64>,
    pub ys: Vec<f64>,
}

/// One answer: the parts a rung uses are filled, the rest empty.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub id: u32,
    pub ms: f64,
    /// Region index per cell (rung 1), row 0 at +y.
    pub regions: Vec<f64>,
    pub cells: u32,
    /// The five radii, µm.
    pub radii: Vec<f64>,
    /// `(x, y)` pairs, µm (atoms, walkers).
    pub dots: Vec<f64>,
    /// One tag per dot (species index, failure cause).
    pub tags: Vec<f64>,
    pub series: Vec<Series>,
    /// Rung-specific numbers, documented where each rung fills them.
    pub scalars: Vec<f64>,
    /// Rung-specific names (species, notes).
    pub names: Vec<String>,
}

/// Everything the engine sends back.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Frame(Frame),
    /// A failure (also constructed by the browser worker's error path).
    Error(String),
}

/// One atom of rung 2: the species it passes through and when it leaves each.
struct Atom {
    xy: [f64; 2],
    /// Species index (into `DecayCache::species`) of each stage.
    stages: Vec<usize>,
    /// Cumulative decay times, s; `stages[k]` lasts until `times[k]`.
    times: Vec<f64>,
}

struct DecayCache {
    nuclide: u8,
    half_life_s: f64,
    species: Vec<String>,
    atoms: Vec<Atom>,
}

/// Rung 3's walkers: CRP-6 Case 1, Cs-137 born uniformly in the kernel, the
/// kernel surface absorbing.
struct KernelWalk {
    cell: TrisoCell,
    temp_c: f64,
    walkers: Vec<WoSWalker>,
    /// When each walker reached the kernel surface, s.
    absorbed: Vec<Option<f64>>,
    time_s: f64,
    /// (hours, released fraction) per frame.
    history: Vec<(f64, f64)>,
}

impl KernelWalk {
    /// The births and child generators of `mc_kernel_release_fraction`, with
    /// its seed, so the animation and the batch check walk the same atoms.
    fn new(temp_c: f64) -> Self {
        let mut cell = TrisoCell::new_crp6_geometry();
        cell.set_uniform_temperature(celsius(temp_c));
        let mut master = OoRng64::from_u64(0x0C1A_5EED);
        let walkers = (0..WALKERS)
            .map(|_| {
                let start = sample_uniform_in_ball(&mut master.0, cell.get_fuel_radius());
                let child = OoRng64::from_u64(master.next_u64());
                WoSWalker::new(start, Nuclide::Cs137, child)
            })
            .collect();
        KernelWalk {
            cell,
            temp_c,
            walkers,
            absorbed: vec![None; WALKERS],
            time_s: 0.0,
            history: vec![(0.0, 0.0)],
        }
    }

    /// Hop every walker still in the kernel until its own clock passes
    /// `until`. `hop` sizes each hop to the nearest interface and reports
    /// `ReachedInterface` within `capture_eps` of it; in the kernel the only
    /// interface is its surface, the Case 1 sink, with `capture_eps` = 10⁻³ R
    /// as in `mc_kernel_release_fraction`.
    fn advance(&mut self, until: f64) {
        let eps = self.cell.get_fuel_radius() * 1e-3;
        for (w, a) in self.walkers.iter_mut().zip(self.absorbed.iter_mut()) {
            let mut hops = 0;
            while a.is_none() && w.time.get::<second>() < until && hops < HOPS_PER_FRAME {
                match w.hop(&self.cell, eps) {
                    HopOutcome::Stepped => hops += 1,
                    HopOutcome::ReachedInterface | HopOutcome::Released => {
                        *a = Some(w.time.get::<second>());
                    }
                }
            }
        }
        self.time_s = until;
        let released = self.absorbed.iter().filter(|a| a.is_some()).count();
        self.history.push((until / 3600.0, released as f64 / WALKERS as f64));
    }
}

/// The engine's state between requests.
#[derive(Default)]
pub struct Engine {
    library: Option<DecayLibrary>,
    decay: Option<DecayCache>,
    walk: Option<KernelWalk>,
}

fn um(x: f64) -> Length {
    Length::new::<micrometer>(x)
}

fn celsius(c: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<degree_celsius>(c)
}

fn cell_of(geometry: u8) -> TrisoCell {
    let r = GEOMETRIES[(geometry as usize).min(1)].1;
    if geometry == 0 {
        TrisoCell::new_crp6_geometry()
    } else {
        TrisoCell::new(um(r[0]), um(r[1]), um(r[2]), um(r[3]), um(r[4]))
    }
}

fn radii_um(cell: &TrisoCell) -> Vec<f64> {
    [
        cell.get_fuel_radius(),
        cell.get_buffer_radius(),
        cell.get_ipyc_radius(),
        cell.get_sic_radius(),
        cell.get_opyc_radius(),
    ]
    .iter()
    .map(|r| r.get::<micrometer>())
    .collect()
}

fn region_index(r: TrisoRegion) -> f64 {
    match r {
        TrisoRegion::Fuel => 0.0,
        TrisoRegion::Buffer => 1.0,
        TrisoRegion::IPyC => 2.0,
        TrisoRegion::SiC => 3.0,
        TrisoRegion::OPyC => 4.0,
        TrisoRegion::Outside => 5.0,
    }
}

/// The species name `boon-lay`'s decay data use, e.g. `Rb88`.
fn species_name(n: Nuclide) -> String {
    format!("{n:?}")
}

fn half_life_s(info: &HalfLifeAndDecayEnergyInfo) -> f64 {
    match info {
        HalfLifeAndDecayEnergyInfo::Stable => f64::INFINITY,
        HalfLifeAndDecayEnergyInfo::Unstable(t, _) => t.get::<second>(),
    }
}

/// `n` points from `a` to `b`, evenly.
fn linspace(a: f64, b: f64, n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| a + (b - a) * i as f64 / (n - 1).max(1) as f64)
        .collect()
}

/// One accident step's reported state (rungs 5 and 7).
#[derive(Clone, Copy, Debug)]
pub struct AccidentPoint {
    pub hours: f64,
    pub pressure_vessel: f64,
    pub decomposition: f64,
    pub in_service: f64,
    pub pressure_mpa: f64,
    pub stress_mpa: f64,
}

/// HTR-10's particle (boon-lay fuel failure's `htr10::particle`, with its
/// stand-in strength, Weibull modulus and fluence) irradiated at `irr_c`,
/// then held at `hold_c` for `hours`, in [`ACCIDENT_STEPS`] steps. Point 0 is
/// the end of irradiation.
pub fn accident_points(irr_c: f64, hold_c: f64, hours: f64) -> Vec<AccidentPoint> {
    let t_irr = celsius(irr_c);
    let t_hold = celsius(hold_c);
    let particle = htr10::particle(t_irr);
    let phi_1 = htr10::end_of_irradiation_failure(t_irr);
    let mut history = AccidentHistory::new(particle, phi_1);
    let layer = particle.layer;
    let point = |h: &AccidentHistory| {
        let p = h.progress();
        let pressure = h.pressure_at(p.elapsed, t_hold);
        let stress = induced_stress_with_thinning_factor(&layer, pressure, p.thinning_factor);
        AccidentPoint {
            hours: p.elapsed.get::<hour>(),
            pressure_vessel: p.pressure_vessel.get::<ratio>(),
            decomposition: p.thermal_decomposition.get::<ratio>(),
            in_service: p.in_service_failure_fraction().get::<ratio>(),
            pressure_mpa: pressure.get::<megapascal>(),
            stress_mpa: stress.get::<megapascal>(),
        }
    };
    let mut out = vec![point(&history)];
    let dt = Time::new::<hour>(hours.max(1e-3) / ACCIDENT_STEPS as f64);
    for _ in 0..ACCIDENT_STEPS {
        history.step(AccidentStep {
            duration: dt,
            mean_temperature: t_hold,
        });
        out.push(point(&history));
    }
    out
}

impl Engine {
    fn library(&mut self) -> &mut DecayLibrary {
        self.library.get_or_insert_with(DecayLibrary::new)
    }

    fn half_life_of(&mut self, n: Nuclide) -> Result<f64, String> {
        let data = self
            .library()
            .try_match_nuclides_to_decay_data(n)
            .ok_or_else(|| format!("no decay data for {n:?}"))?;
        Ok(half_life_s(&data.half_life_information))
    }

    /// Build rung 2's population for one nuclide: every atom gets its own
    /// stochastic chain (branching sampled per atom) and a sampled lifetime
    /// per stage, from the library's own seeded generator.
    fn build_decay(&mut self, nuclide: u8) -> Result<(), String> {
        let (_, n) = DECAY_NUCLIDES[(nuclide as usize).min(DECAY_NUCLIDES.len() - 1)];
        let parent_half_life = self.half_life_of(n)?;
        let mut species = vec![species_name(n)];
        let mut atoms = Vec::with_capacity(DECAY_ATOMS);
        let kernel = um(GEOMETRIES[0].1[0]);
        let mut seed = OoRng64::from_u64(0x0B00_1A47_DECA);
        for _ in 0..DECAY_ATOMS {
            let p = sample_uniform_in_ball(&mut seed.0, kernel);
            let lib = self.library();
            let chain = StochasticDecayChain::new_single_stochastic_chain_from_nuclide(n, lib);
            let mut stages = vec![0usize];
            let mut times = Vec::new();
            let mut clock = 0.0;
            let mut life = parent_half_life;
            for (daughter, info) in chain.iter() {
                let t = SingleNuclideSimulatorMC::get_time_to_decay_stochastic(
                    &mut lib.random_number_generator,
                    Time::new::<second>(life),
                )
                .get::<second>();
                clock += t;
                times.push(clock);
                let name = species_name(*daughter);
                let k = match species.iter().position(|s| *s == name) {
                    Some(k) => k,
                    None => {
                        species.push(name);
                        species.len() - 1
                    }
                };
                stages.push(k);
                life = half_life_s(info);
            }
            times.push(f64::INFINITY);
            atoms.push(Atom {
                xy: [p[0].get::<micrometer>(), p[1].get::<micrometer>()],
                stages,
                times,
            });
        }
        self.decay = Some(DecayCache {
            nuclide,
            half_life_s: parent_half_life,
            species,
            atoms,
        });
        Ok(())
    }

    /// Serve one request, posting its answer.
    pub fn serve(&mut self, req: Request, post: &mut impl FnMut(Event)) {
        let t0 = dhoby_ghaut::web_demo::platform::now_s();
        match self.compute(req) {
            Ok(mut f) => {
                f.ms = (dhoby_ghaut::web_demo::platform::now_s() - t0) * 1e3;
                post(Event::Frame(f));
            }
            Err(e) => post(Event::Error(e)),
        }
    }

    fn compute(&mut self, req: Request) -> Result<Frame, String> {
        match req {
            Request::Slice {
                id,
                geometry,
                cells,
            } => {
                // Rung 1: the region of every pixel centre, asked of the
                // assembled cell (the crate's geometry rule).
                let cell = cell_of(geometry);
                let radii = radii_um(&cell);
                let half = radii[4] * 1.1;
                let n = cells.clamp(16, 240);
                let step = 2.0 * half / n as f64;
                let mut regions = Vec::with_capacity((n * n) as usize);
                for row in 0..n {
                    let y = half - (row as f64 + 0.5) * step;
                    for col in 0..n {
                        let x = -half + (col as f64 + 0.5) * step;
                        let r = cell.get_triso_region([um(x), um(y), Length::ZERO]);
                        regions.push(region_index(r));
                    }
                }
                // Scalars: the half-width of the slice, µm, and the free
                // volume V_f = half the buffer shell, m^3 (fuel failure's
                // definition, rung 5).
                let v_f = 0.5 * 4.0 / 3.0 * std::f64::consts::PI
                    * ((radii[1] * 1e-6).powi(3) - (radii[0] * 1e-6).powi(3));
                Ok(Frame {
                    id,
                    regions,
                    cells: n,
                    radii,
                    scalars: vec![half, v_f],
                    ..Default::default()
                })
            }
            Request::Decay {
                id,
                nuclide,
                t_over_half,
            } => {
                if self.decay.as_ref().map(|d| d.nuclide) != Some(nuclide) {
                    self.build_decay(nuclide)?;
                }
                let d = self.decay.as_ref().ok_or("decay population missing")?;
                let t = t_over_half.max(0.0) * d.half_life_s;
                let mut dots = Vec::with_capacity(2 * d.atoms.len());
                let mut tags = Vec::with_capacity(d.atoms.len());
                for a in &d.atoms {
                    dots.extend_from_slice(&a.xy);
                    let k = a.times.iter().position(|&end| t < end).unwrap_or(0);
                    tags.push(a.stages[k.min(a.stages.len() - 1)] as f64);
                }
                // The decay law the population should follow, and what it does.
                let xs = linspace(0.0, 5.0, 101);
                let n = d.atoms.len() as f64;
                let mc: Vec<f64> = xs
                    .iter()
                    .map(|x| {
                        let tt = x * d.half_life_s;
                        d.atoms.iter().filter(|a| a.times[0] > tt).count() as f64 / n
                    })
                    .collect();
                let exact: Vec<f64> = xs.iter().map(|x| 0.5f64.powf(*x)).collect();
                let surviving = d.atoms.iter().filter(|a| a.times[0] > t).count() as f64 / n;
                Ok(Frame {
                    id,
                    radii: GEOMETRIES[0].1.to_vec(),
                    dots,
                    tags,
                    series: vec![
                        Series {
                            label: "2^(-t/T½), the decay law (infinitely many atoms)".into(),
                            style: 2,
                            colour: 4,
                            panel: 0,
                            xs: xs.clone(),
                            ys: exact,
                        },
                        Series {
                            label: format!("surviving parents, {} sampled atoms", d.atoms.len()),
                            style: 1,
                            colour: 0,
                            panel: 0,
                            xs,
                            ys: mc,
                        },
                    ],
                    // Scalars: parent half-life (s), surviving fraction now,
                    // its binomial 1-sigma, the exact 2^(-t/T½).
                    scalars: vec![
                        d.half_life_s,
                        surviving,
                        (0.5f64.powf(t_over_half) * (1.0 - 0.5f64.powf(t_over_half)) / n).sqrt(),
                        0.5f64.powf(t_over_half),
                    ],
                    names: d.species.clone(),
                    ..Default::default()
                })
            }
            Request::Walk {
                id,
                reset,
                temp_c,
                dt_s,
            } => {
                if reset || self.walk.as_ref().map(|w| w.temp_c) != Some(temp_c) {
                    self.walk = Some(KernelWalk::new(temp_c));
                }
                let w = self.walk.as_mut().ok_or("no walkers")?;
                if dt_s > 0.0 {
                    w.advance(w.time_s + dt_s);
                }
                let radius = um(GEOMETRIES[0].1[0]);
                let d = kernel_diffusion_coefficient(Nuclide::Cs137, celsius(temp_c));
                let dots = w
                    .walkers
                    .iter()
                    .zip(&w.absorbed)
                    .filter(|(_, a)| a.is_none())
                    .flat_map(|(w, _)| [w.position[0].get::<micrometer>(), w.position[1].get::<micrometer>()])
                    .collect();
                let hours: Vec<f64> = w.history.iter().map(|p| p.0).collect();
                let crank: Vec<f64> = hours
                    .iter()
                    .map(|h| calculate_analytical_fraction_released(d, radius, Time::new::<hour>(*h), 200))
                    .collect();
                let released = w.history.last().map(|p| p.1).unwrap_or(0.0);
                Ok(Frame {
                    id,
                    radii: GEOMETRIES[0].1.to_vec(),
                    dots,
                    series: vec![
                        Series {
                            label: "Crank's series (exact for this case)".into(),
                            style: 2,
                            colour: 4,
                            panel: 0,
                            xs: hours.clone(),
                            ys: crank.clone(),
                        },
                        Series {
                            label: format!("reached the kernel surface, {WALKERS} walkers"),
                            style: 1,
                            colour: 0,
                            panel: 0,
                            xs: hours,
                            ys: w.history.iter().map(|p| p.1).collect(),
                        },
                    ],
                    // Scalars: simulated time (s), released fraction, Crank
                    // at that time, D (m^2/s), walkers.
                    scalars: vec![
                        w.time_s,
                        released,
                        crank.last().copied().unwrap_or(0.0),
                        d.get::<square_meter_per_second>(),
                        WALKERS as f64,
                    ],
                    ..Default::default()
                })
            }
            Request::WalkCheck { id, temp_c } => {
                // CRP-6 Case 1: Cs-137 born uniformly in the 212.5 µm kernel,
                // absorbed at its surface; the crate's own Monte Carlo against
                // Crank's series, at 0-200 h.
                let radius = um(GEOMETRIES[0].1[0]);
                let t = celsius(temp_c);
                let d = kernel_diffusion_coefficient(Nuclide::Cs137, t);
                let hours = linspace(0.0, 200.0, 11);
                let mc: Vec<f64> = hours
                    .iter()
                    .map(|h| {
                        mc_kernel_release_fraction(
                            Nuclide::Cs137,
                            radius,
                            t,
                            Time::new::<hour>(*h),
                            CHECK_HISTORIES,
                            0x0C1A_5EED,
                        )
                    })
                    .collect();
                let fine = linspace(0.0, 200.0, 101);
                let crank: Vec<f64> = fine
                    .iter()
                    .map(|h| {
                        calculate_analytical_fraction_released(d, radius, Time::new::<hour>(*h), 200)
                    })
                    .collect();
                let worst = hours
                    .iter()
                    .zip(&mc)
                    .map(|(h, m)| {
                        (m - calculate_analytical_fraction_released(
                            d,
                            radius,
                            Time::new::<hour>(*h),
                            200,
                        ))
                        .abs()
                    })
                    .fold(0.0, f64::max);
                Ok(Frame {
                    id,
                    series: vec![
                        Series {
                            label: "Crank's series (exact for this case)".into(),
                            style: 2,
                            colour: 4,
                            panel: 0,
                            xs: fine,
                            ys: crank,
                        },
                        Series {
                            label: format!(
                                "Walk-on-Spheres, {CHECK_HISTORIES} histories per point"
                            ),
                            style: 3,
                            colour: 0,
                            panel: 0,
                            xs: hours,
                            ys: mc,
                        },
                    ],
                    // Scalars: D (m^2/s), worst |MC - Crank|, its expected
                    // binomial 1-sigma at F = 0.5.
                    scalars: vec![
                        d.get::<square_meter_per_second>(),
                        worst,
                        (0.25 / CHECK_HISTORIES as f64).sqrt(),
                    ],
                    ..Default::default()
                })
            }
            Request::Layers {
                id,
                nuclide,
                temp_c,
            } => {
                let (label, n) = LAYER_NUCLIDES[(nuclide as usize).min(LAYER_NUCLIDES.len() - 1)];
                let layers = [
                    ("kernel (UO2)", TrisoPebbleLayerMaterial::KernelUO2, 0u8),
                    ("buffer", TrisoPebbleLayerMaterial::Buffer, 1),
                    ("PyC", TrisoPebbleLayerMaterial::PyC, 2),
                    ("SiC", TrisoPebbleLayerMaterial::SiC, 3),
                ];
                let ts = linspace(800.0, 1800.0, 81);
                let d_at = |m: TrisoPebbleLayerMaterial, c: f64| -> Option<f64> {
                    try_get_diffusion_coeff_jiang(m, n, celsius(c), Some(ArealNumberDensity::ZERO))
                        .map(|d| d.get::<square_meter_per_second>())
                };
                let mut series = Vec::new();
                let mut names = Vec::new();
                for (name, m, colour) in layers {
                    // Kr in the buffer is a todo!() in the coefficient table
                    // (gh:#541): a panic would abort the worker.
                    if n == Nuclide::Kr85 && m == TrisoPebbleLayerMaterial::Buffer {
                        names.push(format!("{label} in the {name}: no coefficient (a todo!() in the code, gh:#541)"));
                        continue;
                    }
                    let ys: Vec<f64> = ts.iter().map(|c| d_at(m, *c).unwrap_or(f64::NAN)).collect();
                    series.push(Series {
                        label: format!("{label} in the {name}"),
                        style: 1,
                        colour,
                        panel: 0,
                        xs: ts.clone(),
                        ys,
                    });
                }
                let d_pyc = d_at(TrisoPebbleLayerMaterial::PyC, temp_c).ok_or("no PyC D")?;
                let d_sic = d_at(TrisoPebbleLayerMaterial::SiC, temp_c).ok_or("no SiC D")?;
                let p = transmission_probability(
                    DiffusionCoefficient::new::<square_meter_per_second>(d_pyc),
                    DiffusionCoefficient::new::<square_meter_per_second>(d_sic),
                    1.0,
                );
                Ok(Frame {
                    id,
                    series,
                    // Scalars: D_PyC, D_SiC at temp_c (m^2/s), PyC -> SiC
                    // transmission probability (K = 1).
                    scalars: vec![d_pyc, d_sic, p],
                    names,
                    ..Default::default()
                })
            }
            Request::Failure {
                id,
                irr_c,
                hold_c,
                hours,
                cursor_h,
            } => {
                let pts = accident_points(irr_c, hold_c, hours);
                let hs: Vec<f64> = pts.iter().map(|p| p.hours).collect();
                let col = |f: fn(&AccidentPoint) -> f64| pts.iter().map(f).collect::<Vec<f64>>();
                let at = pts
                    .iter()
                    .rev()
                    .find(|p| p.hours <= cursor_h + 1e-9)
                    .copied()
                    .unwrap_or(pts[0]);
                // The drawn population: particle i fails once the fraction
                // passes its own uniform draw u_i; the cause is pressure
                // vessel when u_i < phi_1. An illustration of the fraction,
                // not a simulation of particles.
                let mut rng = OoRng64::from_u64(0x0F41_1A7E);
                let mut tags = Vec::with_capacity(POPULATION);
                let mut failed = 0usize;
                for _ in 0..POPULATION {
                    let u = rng.rand_float();
                    let tag = if u < at.pressure_vessel {
                        1.0
                    } else if u < at.in_service {
                        2.0
                    } else {
                        0.0
                    };
                    if tag > 0.0 {
                        failed += 1;
                    }
                    tags.push(tag);
                }
                Ok(Frame {
                    id,
                    tags,
                    // The total first, so phi_1 (nearly equal to it until
                    // the SiC decomposes) is drawn on top of it.
                    series: vec![
                        Series {
                            label: "in-service total 1-(1-phi_1)(1-phi_2)".into(),
                            style: 1,
                            colour: 0,
                            panel: 0,
                            xs: hs.clone(),
                            ys: col(|p| p.in_service),
                        },
                        Series {
                            label: "phi_1, pressure vessel (Weibull, Eqs 3-9)".into(),
                            style: 1,
                            colour: 1,
                            panel: 0,
                            xs: hs.clone(),
                            ys: col(|p| p.pressure_vessel),
                        },
                        Series {
                            label: "phi_2, SiC thermal decomposition (Eq 13)".into(),
                            style: 1,
                            colour: 3,
                            panel: 0,
                            xs: hs.clone(),
                            ys: col(|p| p.decomposition),
                        },
                        Series {
                            label: "gas pressure, MPa".into(),
                            style: 1,
                            colour: 2,
                            panel: 1,
                            xs: hs.clone(),
                            ys: col(|p| p.pressure_mpa),
                        },
                        Series {
                            label: "SiC tangential stress, MPa".into(),
                            style: 1,
                            colour: 5,
                            panel: 1,
                            xs: hs,
                            ys: col(|p| p.stress_mpa),
                        },
                    ],
                    // Scalars at the cursor: hours, phi_1, phi_2, in-service
                    // total, pressure MPa, stress MPa, failed drawn particles;
                    // then the end-of-irradiation phi_1.
                    scalars: vec![
                        at.hours,
                        at.pressure_vessel,
                        at.decomposition,
                        at.in_service,
                        at.pressure_mpa,
                        at.stress_mpa,
                        failed as f64,
                        pts[0].pressure_vessel,
                    ],
                    ..Default::default()
                })
            }
            Request::Chemistry {
                id,
                o2_kpa,
                steam_kpa,
                h2_kpa,
            } => {
                let ts = linspace(500.0, 1600.0, 111);
                let coefficients = graphite_steam::BlhCoefficients::wang_sun_2023_ig110();
                let o2 = Pressure::new::<kilopascal>(o2_kpa);
                let steam = Pressure::new::<kilopascal>(steam_kpa);
                let h2 = Pressure::new::<kilopascal>(h2_kpa);
                // Each curve split where its validity flag changes: dotted
                // inside the measured range, dashed where extrapolated.
                let mut series = Vec::new();
                let mut split = |label: &str, colour: u8, panel: u8, pts: Vec<(f64, f64, bool)>| {
                    let mut i = 0;
                    while i < pts.len() {
                        let inside = pts[i].2;
                        let mut j = i;
                        while j + 1 < pts.len() && pts[j + 1].2 == inside {
                            j += 1;
                        }
                        // Overlap one point so the segments join.
                        let end = (j + 1).min(pts.len() - 1);
                        let seg = &pts[i..=end];
                        series.push(Series {
                            label: format!(
                                "{label} ({})",
                                if inside { "inside its measured range" } else { "extrapolated" }
                            ),
                            style: if inside { 1 } else { 2 },
                            colour,
                            panel,
                            xs: seg.iter().map(|p| p.0).collect(),
                            ys: seg.iter().map(|p| p.1).collect(),
                        });
                        i = j + 1;
                    }
                };
                split(
                    "graphite in air, kinetic rate, 1/s (Contescu 2011)",
                    1,
                    0,
                    ts.iter()
                        .map(|c| {
                            let (r, v) = graphite_air::kinetic_specific_rate(celsius(*c), o2);
                            (*c, r.get::<hertz>(), v == graphite_air::Validity::InsideMeasuredRange)
                        })
                        .collect(),
                );
                split(
                    "graphite in steam, BLH rate, 1/s (Wang & Sun 2023)",
                    0,
                    0,
                    ts.iter()
                        .map(|c| {
                            let (r, v) =
                                graphite_steam::specific_rate(celsius(*c), steam, h2, &coefficients);
                            (*c, r.get::<hertz>(), v == graphite_steam::Validity::InsideMeasuredRange)
                        })
                        .collect(),
                );
                split(
                    "bare-kernel hydrolysis, stored gas released (TECDOC-978 Eq 5-2)",
                    2,
                    1,
                    ts.iter()
                        .map(|c| {
                            let (f, v) = kernel_hydrolysis::stored_gas_fraction(celsius(*c), steam);
                            (*c, f.get::<ratio>(), v == kernel_hydrolysis::Validity::InsideFittedRange)
                        })
                        .collect(),
                );
                Ok(Frame {
                    id,
                    series,
                    ..Default::default()
                })
            }
            Request::Release {
                id,
                irr_c,
                hold_c,
                hours,
                f_hm,
                k_plate,
                k_clean,
                k_leak,
            } => {
                let t_irr = Time::new::<day>(htr10::RESIDENCE_FULL_POWER_DAYS);
                let pts = accident_points(irr_c, hold_c, hours);
                let n_pts = pts.len();
                let mut names: Vec<String> = Vec::new();
                let mut tags: Vec<f64> = Vec::new();
                let mut skipped: Vec<String> = Vec::new();
                let mut blocks: Vec<f64> = Vec::new();
                for entry in changi::activity::inventory::htr10_equilibrium_core() {
                    // TRISO-ATOPS's own nuclide table supplies Z, the
                    // half-life and the transport group; a nuclide it does
                    // not carry is listed, not invented.
                    let Some(nuc) = find_nuclide(entry.nuclide) else {
                        skipped.push(entry.nuclide.to_string());
                        continue;
                    };
                    let inventory_bq = entry.activity.get::<uom::si::radioactivity::becquerel>();
                    let (block, group) = release_one(
                        &nuc,
                        inventory_bq,
                        t_irr,
                        &pts,
                        irr_c,
                        hold_c,
                        f_hm,
                        [k_plate, k_clean, k_leak],
                    );
                    names.push(nuc.name.to_string());
                    tags.push(group_index(group) as f64);
                    blocks.extend(block);
                }
                for sk in skipped {
                    names.push(format!("{RELEASE_SKIPPED}{sk}"));
                }
                let n_nuc = tags.len();
                Ok(Frame {
                    id,
                    // One series: the hold's hours, with the in-service
                    // failure fraction (rung 5) as its values.
                    series: vec![Series {
                        label: "in-service failure fraction".into(),
                        style: 1,
                        colour: 4,
                        panel: 9,
                        xs: pts.iter().map(|p| p.hours).collect(),
                        ys: pts.iter().map(|p| p.in_service).collect(),
                    }],
                    // Layout: see `release_value` and friends.
                    scalars: [vec![n_nuc as f64, n_pts as f64], blocks].concat(),
                    names,
                    tags,
                    ..Default::default()
                })
            }
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl dhoby_ghaut::web_demo::link::NativeEngine for Engine {
    type Req = Request;
    type Ev = Event;
    fn handle(&mut self, req: Request, post: &mut impl FnMut(Event)) {
        self.serve(req, post);
    }
}

#[cfg(target_arch = "wasm32")]
impl dhoby_ghaut::web_demo::link::WorkerEngine for Engine {
    type Req = Request;
    type Ev = Event;
    fn handle(
        state: &std::sync::Arc<std::sync::RwLock<Self>>,
        req: Request,
        post: dhoby_ghaut::web_demo::link::Poster<Event>,
    ) {
        if let Ok(mut e) = state.write() {
            e.serve(req, &mut |ev| post.post(ev));
        }
    }
    fn error(message: String) -> Event {
        Event::Error(message)
    }
}

// ─── Wire format: numbers in one Float64Array, text in one string ────────────
//
// Natively the enums cross the thread boundary as they are; in the browser
// they cross as a JS object `{kind: "msg", data: Float64Array, text}`.

impl Request {
    pub fn to_wire(&self) -> Vec<f64> {
        let b = |x: bool| if x { 1.0 } else { 0.0 };
        match *self {
            Request::Slice { id, geometry, cells } => {
                vec![0.0, id as f64, geometry as f64, cells as f64]
            }
            Request::Decay { id, nuclide, t_over_half } => {
                vec![1.0, id as f64, nuclide as f64, t_over_half]
            }
            Request::Walk { id, reset, temp_c, dt_s } => {
                vec![2.0, id as f64, b(reset), temp_c, dt_s]
            }
            Request::WalkCheck { id, temp_c } => vec![3.0, id as f64, temp_c],
            Request::Layers { id, nuclide, temp_c } => {
                vec![4.0, id as f64, nuclide as f64, temp_c]
            }
            Request::Failure { id, irr_c, hold_c, hours, cursor_h } => {
                vec![5.0, id as f64, irr_c, hold_c, hours, cursor_h]
            }
            Request::Chemistry { id, o2_kpa, steam_kpa, h2_kpa } => {
                vec![6.0, id as f64, o2_kpa, steam_kpa, h2_kpa]
            }
            Request::Release {
                id,
                irr_c,
                hold_c,
                hours,
                f_hm,
                k_plate,
                k_clean,
                k_leak,
            } => vec![
                7.0,
                id as f64,
                irr_c,
                hold_c,
                hours,
                f_hm,
                k_plate,
                k_clean,
                k_leak,
            ],
        }
    }

    pub fn from_wire(d: &[f64]) -> Result<Request, String> {
        let g = |i: usize| {
            d.get(i)
                .copied()
                .ok_or_else(|| format!("request too short ({} values)", d.len()))
        };
        let id = g(1)? as u32;
        Ok(match g(0)? as u8 {
            0 => Request::Slice { id, geometry: g(2)? as u8, cells: g(3)? as u32 },
            1 => Request::Decay { id, nuclide: g(2)? as u8, t_over_half: g(3)? },
            2 => Request::Walk { id, reset: g(2)? != 0.0, temp_c: g(3)?, dt_s: g(4)? },
            3 => Request::WalkCheck { id, temp_c: g(2)? },
            4 => Request::Layers { id, nuclide: g(2)? as u8, temp_c: g(3)? },
            5 => Request::Failure {
                id,
                irr_c: g(2)?,
                hold_c: g(3)?,
                hours: g(4)?,
                cursor_h: g(5)?,
            },
            6 => Request::Chemistry { id, o2_kpa: g(2)?, steam_kpa: g(3)?, h2_kpa: g(4)? },
            7 => Request::Release {
                id,
                irr_c: g(2)?,
                hold_c: g(3)?,
                hours: g(4)?,
                f_hm: g(5)?,
                k_plate: g(6)?,
                k_clean: g(7)?,
                k_leak: g(8)?,
            },
            k => return Err(format!("unknown request kind {k}")),
        })
    }
}

const UNIT: char = '\u{1f}';
const GROUP: char = '\u{1e}';

impl Event {
    pub fn to_wire(&self) -> (Vec<f64>, String) {
        match self {
            Event::Frame(f) => {
                let mut d = vec![
                    0.0,
                    f.id as f64,
                    f.ms,
                    f.cells as f64,
                    f.regions.len() as f64,
                    f.radii.len() as f64,
                    f.dots.len() as f64,
                    f.tags.len() as f64,
                    f.scalars.len() as f64,
                    f.series.len() as f64,
                ];
                for v in [&f.regions, &f.radii, &f.dots, &f.tags, &f.scalars] {
                    d.extend_from_slice(v);
                }
                for s in &f.series {
                    d.extend_from_slice(&[
                        s.style as f64,
                        s.colour as f64,
                        s.panel as f64,
                        s.xs.len() as f64,
                    ]);
                    d.extend_from_slice(&s.xs);
                    d.extend_from_slice(&s.ys);
                }
                let names = f.names.join(&UNIT.to_string());
                let labels = f
                    .series
                    .iter()
                    .map(|s| s.label.as_str())
                    .collect::<Vec<_>>()
                    .join(&UNIT.to_string());
                (d, format!("{names}{GROUP}{labels}"))
            }
            Event::Error(m) => (vec![1.0], m.clone()),
        }
    }

    pub fn from_wire(d: &[f64], text: &str) -> Result<Event, String> {
        let g = |i: usize| d.get(i).copied().ok_or_else(|| "event too short".to_string());
        if g(0)? as u8 == 1 {
            return Ok(Event::Error(text.to_string()));
        }
        let mut i = 10;
        let mut take = |n: usize| -> Result<Vec<f64>, String> {
            if d.len() < i + n {
                return Err("frame event truncated".into());
            }
            let v = d[i..i + n].to_vec();
            i += n;
            Ok(v)
        };
        let regions = take(g(4)? as usize)?;
        let radii = take(g(5)? as usize)?;
        let dots = take(g(6)? as usize)?;
        let tags = take(g(7)? as usize)?;
        let scalars = take(g(8)? as usize)?;
        let (names_text, labels_text) = text.split_once(GROUP).unwrap_or((text, ""));
        let names = if names_text.is_empty() {
            Vec::new()
        } else {
            names_text.split(UNIT).map(str::to_string).collect()
        };
        let mut labels = labels_text.split(UNIT);
        let mut series = Vec::new();
        for _ in 0..g(9)? as usize {
            let head = take(4)?;
            let len = head[3] as usize;
            let xs = take(len)?;
            let ys = take(len)?;
            series.push(Series {
                label: labels.next().unwrap_or("").to_string(),
                style: head[0] as u8,
                colour: head[1] as u8,
                panel: head[2] as u8,
                xs,
                ys,
            });
        }
        Ok(Event::Frame(Frame {
            id: g(1)? as u32,
            ms: g(2)?,
            cells: g(3)? as u32,
            regions,
            radii,
            dots,
            tags,
            series,
            scalars,
            names,
        }))
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::{Event, Request};
    use dhoby_ghaut::web_demo::link::{js, Message};
    use wasm_bindgen::JsValue;

    fn wrap(d: &[f64], text: &str) -> JsValue {
        let o = js::object();
        js::set(&o, "kind", "msg");
        js::set(&o, "data", js::f64s(d));
        js::set(&o, "text", text);
        o.into()
    }

    impl Message for Request {
        fn to_js(&self) -> JsValue {
            wrap(&self.to_wire(), "")
        }
        fn from_js(v: &JsValue) -> Result<Self, String> {
            Request::from_wire(&js::get_f64s(v, "data"))
        }
    }

    impl Message for Event {
        fn to_js(&self) -> JsValue {
            let (d, t) = self.to_wire();
            wrap(&d, &t)
        }
        fn from_js(v: &JsValue) -> Result<Self, String> {
            Event::from_wire(&js::get_f64s(v, "data"), &js::get_str(v, "text"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(e: &mut Engine, r: Request) -> Frame {
        let mut out = Vec::new();
        e.serve(r, &mut |ev| out.push(ev));
        match out.pop() {
            Some(Event::Frame(f)) => f,
            other => panic!("expected a frame, got {other:?}"),
        }
    }

    /// Every request kind and a full frame survive the worker's wire format.
    #[test]
    fn messages_round_trip_through_the_wire_format() {
        for r in [
            Request::Slice { id: 1, geometry: 1, cells: 64 },
            Request::Decay { id: 2, nuclide: 3, t_over_half: 1.5 },
            Request::Walk { id: 3, reset: true, temp_c: 1600.0, dt_s: 5e4 },
            Request::WalkCheck { id: 4, temp_c: 1400.0 },
            Request::Layers { id: 5, nuclide: 2, temp_c: 1200.0 },
            Request::Failure { id: 6, irr_c: 776.0, hold_c: 1800.0, hours: 300.0, cursor_h: 150.0 },
            Request::Chemistry { id: 7, o2_kpa: 21.0, steam_kpa: 5.0, h2_kpa: 0.0 },
            Request::Release {
                id: 8,
                irr_c: 776.0,
                hold_c: 1600.0,
                hours: 100.0,
                f_hm: 1e-4,
                k_plate: 7.5e-5,
                k_clean: 8.77e-5,
                k_leak: 1e-6,
            },
        ] {
            assert_eq!(Request::from_wire(&r.to_wire()).unwrap(), r);
        }
        let f = Event::Frame(Frame {
            id: 9,
            ms: 1.5,
            cells: 2,
            regions: vec![0.0, 1.0, 2.0, 5.0],
            radii: vec![1.0, 2.0, 3.0, 4.0, 5.0],
            dots: vec![1.0, 2.0],
            tags: vec![3.0],
            series: vec![Series {
                label: "a".into(),
                style: 1,
                colour: 2,
                panel: 1,
                xs: vec![1.0, 2.0],
                ys: vec![3.0, 4.0],
            }],
            scalars: vec![7.0],
            names: vec!["Kr88".into(), "Rb88".into()],
        });
        let (d, t) = f.to_wire();
        assert_eq!(Event::from_wire(&d, &t).unwrap(), f);
        let e = Event::Error("boom".into());
        let (d, t) = e.to_wire();
        assert_eq!(Event::from_wire(&d, &t).unwrap(), e);
    }

    /// Rung 1 draws what the solver sees: the slice's centre pixel is kernel,
    /// its corner outside, and walking out along +x crosses the five regions
    /// in order at the radii the cell reports, to within one pixel.
    #[test]
    fn the_slice_is_the_assembled_geometry() {
        let mut e = Engine::default();
        for g in 0..2u8 {
            let f = frame(&mut e, Request::Slice { id: 1, geometry: g, cells: 200 });
            let n = f.cells as usize;
            let half = f.scalars[0];
            let step = 2.0 * half / n as f64;
            assert_eq!(f.regions[0], 5.0, "corner is outside");
            let row = n / 2;
            let mut last = 0.0;
            for col in n / 2..n {
                let r = f.regions[row * n + col];
                assert!(r >= last, "regions are nested outwards");
                if r > last {
                    let x = -half + (col as f64 + 0.5) * step;
                    let edge = f.radii[last as usize];
                    assert!((x - edge).abs() <= step, "boundary at {x} vs radius {edge}");
                    last = r;
                }
            }
            assert_eq!(last, 5.0);
            for (a, b) in f.radii.iter().zip(GEOMETRIES[g as usize].1) {
                assert!((a - b).abs() < 1e-9, "reported radius {a} vs {b} µm");
            }
        }
    }

    /// Rung 2: the sampled parents follow the decay law within their binomial
    /// statistics (4 sigma at 500 atoms), and every atom starts as the parent.
    #[test]
    fn decay_population_follows_the_decay_law() {
        let mut e = Engine::default();
        let t0 = dhoby_ghaut::web_demo::platform::now_s();
        let f0 = frame(&mut e, Request::Decay { id: 1, nuclide: 0, t_over_half: 0.0 });
        println!("decay library + 500 Kr-88 chains: {:.0} ms", (dhoby_ghaut::web_demo::platform::now_s() - t0) * 1e3);
        assert!(f0.tags.iter().all(|t| *t == 0.0));
        println!("species: {:?}, T½ = {} s", f0.names, f0.scalars[0]);
        for x in [0.5, 1.0, 2.0, 3.0] {
            let f = frame(&mut e, Request::Decay { id: 2, nuclide: 0, t_over_half: x });
            let (mc, sigma, exact) = (f.scalars[1], f.scalars[2], f.scalars[3]);
            println!("t = {x} T½: surviving {mc:.3}, law {exact:.3} ± {sigma:.3}");
            assert!((mc - exact).abs() < 4.0 * sigma, "{mc} vs {exact}");
        }
    }

    /// Rung 3: the animated Case 1a (1200 C, 200 one-hour frames) stays in
    /// the kernel until absorbed and lands on Crank's 0.5337 at 200 h within
    /// 4 sigma of 400 walkers; the batch check (the crate's own
    /// `mc_kernel_release_fraction`) agrees with Crank too. Prints the frame
    /// cost.
    #[test]
    fn walk_and_its_check() {
        let mut e = Engine::default();
        let f = frame(&mut e, Request::Walk { id: 1, reset: true, temp_c: 1200.0, dt_s: 0.0 });
        assert_eq!(f.dots.len(), 2 * WALKERS);
        let mut worst_ms: f64 = 0.0;
        let mut last = f;
        for _ in 0..200 {
            last = frame(&mut e, Request::Walk { id: 2, reset: false, temp_c: 1200.0, dt_s: 3600.0 });
            worst_ms = worst_ms.max(last.ms);
            for p in last.dots.chunks(2) {
                assert!((p[0] * p[0] + p[1] * p[1]).sqrt() <= 212.5 + 1e-6);
            }
        }
        let (t, released, crank) = (last.scalars[0], last.scalars[1], last.scalars[2]);
        let sigma = (crank * (1.0 - crank) / WALKERS as f64).sqrt();
        println!(
            "walk: t = {:.0} h, released {released:.4}, Crank {crank:.4} (1 sigma {sigma:.4}), worst frame {worst_ms:.2} ms",
            t / 3600.0
        );
        assert!((crank - 0.5337).abs() < 1e-3);
        assert!((released - crank).abs() < 4.0 * sigma);
        let c = frame(&mut e, Request::WalkCheck { id: 4, temp_c: 1200.0 });
        println!(
            "Case 1 check at 1200 C: D = {:.4e}, worst |MC - Crank| = {:.4} (1 sigma {:.4}), {:.0} ms",
            c.scalars[0], c.scalars[1], c.scalars[2], c.ms
        );
        assert!(c.scalars[1] < 4.0 * c.scalars[2]);
    }

    /// Probe (run by hand): how many multilayer steps one walker born in the
    /// buffer takes to advance its clock, the cost that rules out a live
    /// five-layer walk.
    #[test]
    #[ignore]
    fn multilayer_bounce_cost_probe() {
        use boon_lay::lagrangian_decay_simulator::lagrangian_diffusion::first_passage::walk_on_spheres::WalkParams;
        for temp in [1200.0, 1600.0] {
            let mut cell = TrisoCell::new_crp6_geometry();
            cell.set_uniform_temperature(celsius(temp));
            let params = WalkParams::crp6_default();
            let mut w = WoSWalker::new([um(262.5), Length::ZERO, Length::ZERO], Nuclide::Cs137, OoRng64::from_u64(7));
            let t0 = dhoby_ghaut::web_demo::platform::now_s();
            let (mut steps, mut interface) = (0u64, 0u64);
            while steps < 20_000_000 {
                if w.step_multilayer(&cell, &params) == HopOutcome::ReachedInterface {
                    interface += 1;
                }
                steps += 1;
            }
            println!(
                "{temp} C: {steps} steps ({interface} at interfaces) to advance {:.2e} s, r = {:.2} um, {:.1} s wall",
                w.time.get::<second>(),
                w.radius().get::<micrometer>(),
                dhoby_ghaut::web_demo::platform::now_s() - t0
            );
        }
    }

    /// Rung 4 reproduces the lesson's recorded table row (Cs-137, 1600 C:
    /// D_PyC 4.062e-14, D_SiC 9.228e-17, p 2.266e-3) and never asks for the
    /// Kr-in-buffer coefficient, which panics (gh:#541).
    #[test]
    fn layers_match_the_recorded_table_and_skip_the_todo() {
        let mut e = Engine::default();
        let f = frame(&mut e, Request::Layers { id: 1, nuclide: 0, temp_c: 1600.0 });
        assert!((f.scalars[0] / 4.062e-14 - 1.0).abs() < 1e-3);
        assert!((f.scalars[1] / 9.228e-17 - 1.0).abs() < 1e-3);
        assert!((f.scalars[2] / 2.266e-3 - 1.0).abs() < 1e-3);
        assert_eq!(f.series.len(), 4);
        let kr = frame(&mut e, Request::Layers { id: 2, nuclide: 3, temp_c: 1600.0 });
        assert_eq!(kr.series.len(), 3);
        assert_eq!(kr.names.len(), 1);
    }

    /// Rung 5: the failure fractions never fall, stay in [0, 1], start at the
    /// end-of-irradiation phi_1, and the drawn population's failures track
    /// the in-service fraction.
    #[test]
    fn failure_history_is_monotone_and_starts_at_end_of_irradiation() {
        let pts = accident_points(776.0, 1800.0, 500.0);
        assert_eq!(pts.len(), ACCIDENT_STEPS + 1);
        let eoi = htr10::end_of_irradiation_failure(celsius(776.0)).get::<ratio>();
        assert_eq!(pts[0].pressure_vessel, eoi);
        for w in pts.windows(2) {
            assert!(w[1].in_service >= w[0].in_service - 1e-15);
            assert!((0.0..=1.0).contains(&w[1].in_service));
        }
        let mut e = Engine::default();
        let f = frame(
            &mut e,
            Request::Failure { id: 1, irr_c: 776.0, hold_c: 2100.0, hours: 200.0, cursor_h: 200.0 },
        );
        let (total, failed) = (f.scalars[3], f.scalars[6] / POPULATION as f64);
        println!("2100 C, 200 h: in-service {total:.3}, drawn {failed:.3}, {:.1} ms", f.ms);
        let sigma = (total * (1.0 - total) / POPULATION as f64).sqrt().max(1e-3);
        assert!((failed - total).abs() < 5.0 * sigma);
    }

    /// Rung 6 reproduces the crate's own transcription checks at their
    /// points (air 650 C 6.8877e-6 /s; hydrolysis 1000 C, 1 kPa, 0.296875).
    #[test]
    fn chemistry_curves_pass_through_the_transcription_checks() {
        let mut e = Engine::default();
        let f = frame(&mut e, Request::Chemistry { id: 1, o2_kpa: 21.27825, steam_kpa: 1.0, h2_kpa: 0.0 });
        let find = |prefix: &str, x: f64| {
            f.series
                .iter()
                .filter(|s| s.label.starts_with(prefix))
                .find_map(|s| s.xs.iter().position(|v| (*v - x).abs() < 1e-9).map(|i| s.ys[i]))
                .unwrap()
        };
        assert!((find("graphite in air", 650.0) / 6.8877e-6 - 1.0).abs() < 1e-4);
        assert!((find("bare-kernel", 1000.0) / 0.296875 - 1.0).abs() < 1e-4);
    }

    fn release(e: &mut Engine, hold_c: f64, k: [f64; 3]) -> Frame {
        frame(
            e,
            Request::Release {
                id: 1,
                irr_c: 776.0,
                hold_c,
                hours: 100.0,
                f_hm: 1e-4,
                k_plate: k[0],
                k_clean: k[1],
                k_leak: k[2],
            },
        )
    }

    fn index_of(f: &Frame, name: &str) -> usize {
        f.names.iter().position(|n| n == name).unwrap_or_else(|| panic!("{name} missing"))
    }

    /// **Every inventory nuclide TRISO-ATOPS models is in the frame, and the
    /// rest are listed as skipped.**
    ///
    /// Methodology: Liu & Cao's 22-nuclide inventory against TRISO-ATOPS's
    /// table. **Result (2026-10-05):** 19 modelled; H-3, Xe-135m and Rb-88
    /// skipped; the frame's layout checks (`release_dims`).
    #[test]
    fn release_carries_every_modelled_nuclide() {
        let mut e = Engine::default();
        let f = release(&mut e, 1600.0, [7.5e-5, 8.77e-5, 0.0]);
        let (n, p) = release_dims(&f);
        assert_eq!(n, 19);
        assert_eq!(p, f.series[0].xs.len());
        let skipped: Vec<&str> = f.names[n..].iter().map(|s| s.trim_start_matches(RELEASE_SKIPPED)).collect();
        assert_eq!(skipped, ["H-3", "Xe-135m", "Rb-88"]);
        for name in ["Cs-137", "Cs-134", "Sr-90", "Sr-89", "Ag-110m", "I-131", "Kr-88"] {
            index_of(&f, name);
        }
    }

    /// **The diagram's arrows balance where they must, and upstream's
    /// routing holds.**
    ///
    /// Methodology: every sink on (`k_plate` 7.5e-5, `k_clean` 8.77e-5,
    /// `k_leak` 1e-5 1/s). At the first point the pools have run 40 years
    /// at a constant source, so a short-lived nuclide is at equilibrium:
    /// for I-131 (halogen) the flow into the helium equals the flows out of
    /// it and each held pool's inflow equals its decay. Cs-137 (special
    /// metal) plates out but is never routed to the HPS; its plate-out after
    /// 40 years is below equilibrium (inflow exceeds decay), as upstream's
    /// normal operation gives. Kr-88 never plates out. Cs and Sr have a
    /// positive source and graphite hold-up at normal operation, a larger
    /// source and a smaller hold-up at the end of the 1600 C hold. Pass: 1e-9
    /// relative. Ag-110m has no source at normal operation (upstream's
    /// breakthrough time lag) and one during the hold.
    /// **Result (2026-10-05):** passes. Cs-137 S 5.8e11 -> 2.6e12 atoms/s,
    /// hold-up 8.1e19 -> 0 atoms; Sr-90 S 2.3 -> 1.9e12 atoms/s.
    #[test]
    fn release_flows_balance_and_follow_upstream_routing() {
        let mut e = Engine::default();
        let f = release(&mut e, 1600.0, [7.5e-5, 8.77e-5, 1e-5]);
        let rel_err = |a: f64, b: f64| (a - b).abs() / b.abs();
        let i = index_of(&f, "I-131");
        let v = |n: usize, k: usize| release_value(&f, n, 0, k);
        let out = v(i, rel::Q_PLATE) + v(i, rel::Q_CLEAN) + v(i, rel::Q_LEAK) + v(i, rel::D_C);
        println!("I-131 at t = 0: S {:.4e}, out of the helium {out:.4e} atoms/s", v(i, rel::S));
        assert!(rel_err(out, v(i, rel::S)) < 1e-9);
        assert!(rel_err(v(i, rel::D_P), v(i, rel::Q_PLATE)) < 1e-9);
        assert!(rel_err(v(i, rel::D_H), v(i, rel::Q_CLEAN)) < 1e-9);

        let cs = index_of(&f, "Cs-137");
        assert_eq!(release_header(&f, cs, 3), 0.0, "the HPS does not scrub metals");
        assert_eq!(v(cs, rel::H), 0.0);
        assert!(v(cs, rel::P) > 0.0);
        assert!(v(cs, rel::Q_PLATE) > 1.01 * v(cs, rel::D_P), "Cs-137 plate-out still growing after 40 y");
        let kr = index_of(&f, "Kr-88");
        assert_eq!(v(kr, rel::P), 0.0);
        // Metals are held up in the matrix graphite at normal operation
        // (776 C); at 1600 C graphite diffusion is fast enough that the
        // attenuation factor goes to 1 and the hold-up to ~0.
        // Silver: at 776 C, D(Ag in SiC) ~7e-20 m2/s gives D't ~5e-3 over the
        // residence, short of the membrane time lag, so upstream's
        // breakthrough_model clamps a negative release fraction to 0: no
        // silver through intact SiC at normal operation in this model.
        let ag = index_of(&f, "Ag-110m");
        let last = release_dims(&f).1 - 1;
        assert_eq!(release_value(&f, ag, 0, rel::S), 0.0);
        assert!(release_value(&f, ag, last, rel::S) > 0.0);
        for name in ["Cs-137", "Cs-134", "Sr-90", "Sr-89"] {
            let n = index_of(&f, name);
            let last = release_dims(&f).1 - 1;
            let (s0, g0) = (release_value(&f, n, 0, rel::S), release_value(&f, n, 0, rel::G));
            let (s1, g1) = (release_value(&f, n, last, rel::S), release_value(&f, n, last, rel::G));
            println!("{name}: S {s0:.3e} -> {s1:.3e} atoms/s; graphite hold-up {g0:.3e} -> {g1:.3e} atoms");
            assert!(s0 > 0.0 && s1 > s0 && g0 > 0.0 && g1 < g0, "{name}");
        }
    }

    /// Methodology: Kr-88 with no sinks: at normal operation the circulating
    /// activity equals the source (lambda C = S); a hotter hold raises the
    /// source. **Result (2026-10-05):** passes.
    #[test]
    fn release_pools_reach_equilibrium_and_rise_with_temperature() {
        let mut e = Engine::default();
        let f = release(&mut e, 776.0, [0.0; 3]);
        let kr = index_of(&f, "Kr-88");
        let lam = release_header(&f, kr, 1);
        let (s0, c0) = (release_value(&f, kr, 0, rel::S), lam * release_value(&f, kr, 0, rel::C));
        assert!((c0 / s0 - 1.0).abs() < 1e-9, "{c0} vs {s0}");
        let g = release(&mut e, 1600.0, [0.0; 3]);
        let last = release_dims(&g).1 - 1;
        let s_hot = release_value(&g, kr, last, rel::S);
        println!("Kr-88 source: {s0:.3e} -> {s_hot:.3e} atoms/s at 1600 C");
        assert!(s_hot > s0);
    }

}
