//! The interfaces the coupled drivers call into — **provisional**.
//!
//! # Provenance
//!
//! Translated alongside Than Yan Ren's (SNRSI) BEDOK MATLAB snapshot
//! (`BEDOKfiles.zip`, sha256 `e45cd6f57be2087c…`). Original author: **Than Yan
//! Ren**, Singapore Nuclear Research and Safety Institute. Translated with
//! permission; see `docs/bedok-port-scoping.md` §6.
//!
//! # Why this module exists, and what must happen to it
//!
//! The coupled drivers in this directory are the *callers* of the semi-analytic
//! nodal solver and the thermal-hydraulics solver. Those two live in
//! [`crate::reference::nodal`] and [`crate::reference::th`] and were being
//! written at the same time as this module, so the call sites here are declared
//! against the shapes the MATLAB actually passes, and every function body is
//! [`todo!`].
//!
//! **Nothing in this module is a design proposal.** Each item names the MATLAB
//! function it stands for and the module that will own it. When `nodal/` and
//! `th/` land, the declarations here are deleted and the drivers import theirs;
//! the only work is reconciling names and field layouts. The state types
//! (`CaseParams`, `CoreGeometry`, `ThermalState`, `SigmaValues`) are shared by
//! all three directories and will end up in one place — most naturally next to
//! [`crate::reference::grid`], which already owns [`Grid`] and
//! [`crate::reference::grid::Geometry`].
//!
//! # Conventions used throughout
//!
//! - **State vectors** (`philen = nodes * ngroups` entries) are indexed through
//!   [`Grid::index`]. Never index them by hand.
//! - **Per-node fields** (`es = nodes` entries) use the same rule with `g = 0`.
//! - **Per-`(ix,iy)` maps** (rod banks, `zhis`) are indexed `ix*ny + iy`.
//! - **Lengths are centimetres, temperatures kelvin, densities g/cm³, power
//!   watts, time seconds, boron ppm** — the units the MATLAB and the benchmark
//!   specifications both use. `uom` types are deliberately absent from the
//!   reference path so the arithmetic stays line-for-line comparable
//!   (`docs/bedok-port-scoping.md` §7).

use super::error::Result;
use super::sparse::SparseMatrix;
use crate::reference::grid::{Geometry, Grid};

// =====================================================================
// Case inputs — MATLAB `params`
// =====================================================================

/// Run controls and case data — the MATLAB `params` struct.
///
/// MATLAB reads this struct with `isfield`, so every optional control is an
/// [`Option`] here and the defaults are applied at the point of use, exactly
/// where the MATLAB applies them.
///
/// # Ownership
///
/// Will be owned by the case layer ([`crate::reference::cases`]) once it
/// lands; the fields below are only those the coupling layer reads.
#[derive(Debug, Clone)]
pub struct CaseParams {
    /// Node grid and energy-group count. MATLAB `params.maxix/maxiy/maxiz/G`
    /// via `handle3dcoords`.
    pub grid: Grid,

    /// Extra state components beyond the `G` flux groups. MATLAB `params.Nc`
    /// (absent → 0). All four benchmark cases set it to zero; it lengthens the
    /// operators to `philenf = philen + Nc*es`.
    pub n_components: usize,

    /// Radial solution rings in the fuel rod. MATLAB `params.fuel.maxir`.
    pub fuel_max_ir: usize,

    /// Radial rings inside the fuel pellet proper. MATLAB `params.fuel.fueln`.
    pub fuel_n: usize,

    /// Boron concentration \[ppm\] the cross-section feedback is evaluated at.
    /// MATLAB `params.boron`.
    pub boron: f64,

    /// Initial flat fuel temperature \[K\]. MATLAB `params.fueltempavg`.
    pub fuel_temp_avg_init: f64,

    /// Initial flat coolant temperature \[K\]. MATLAB `params.cooltempavg`.
    pub cool_temp_avg_init: f64,

    /// Initial flat coolant density \[g/cm³\]. MATLAB `params.cooldenavg`.
    pub cool_den_avg_init: f64,

    /// Fuel-temperature convergence tolerance \[K\] for the coupled outer loop.
    /// MATLAB `params.fueltemptol`; default 0.5 K.
    pub fuel_temp_tol: Option<f64>,

    /// Outer fission-source / `k_eff` tolerance \[-\]. MATLAB `params.fluxtol`;
    /// default 1e-4.
    pub flux_tol: Option<f64>,

    /// Cap on coupled outer iterations. MATLAB `params.thmaxiter`; default 50.
    pub th_max_iter: Option<usize>,

    /// Picard under-relaxation factor for the T-H feedback fields, `0 < w <= 1`.
    /// MATLAB `params.threlax`; default 0.5.
    pub th_relax: Option<f64>,

    /// Set to `Some(0.0)` to disable the Eisenstat-Walker-style inexact inner
    /// tolerance. MATLAB `params.inexactinner`.
    pub inexact_inner: Option<f64>,

    /// Forcing factor of the inexact inner schedule. MATLAB `params.inexacteta`;
    /// default 1e-3.
    pub inexact_eta: Option<f64>,

    /// Inner eigenvalue-solve tolerance handed to the nodal solver. MATLAB
    /// `params.innertol` — written by the coupling layer, read by
    /// `sanodaldiffusion_solverxyz`.
    pub inner_tol: Option<f64>,

    /// `k_eff` tolerance of the critical-boron search. MATLAB `params.crittol`;
    /// default 1e-5.
    pub crit_tol: Option<f64>,

    /// End of the transient \[s\]. MATLAB `params.tend`.
    pub t_end: Option<f64>,

    /// Time points of the transient \[s\]. MATLAB `params.tgrid`.
    pub t_grid: Option<Vec<f64>>,

    /// Feedback Picard passes per time step. MATLAB `params.timepicard`;
    /// default 1.
    pub time_picard: Option<usize>,

    /// Update the SA-nodal correction every N time steps; 0 freezes it at the
    /// steady state. MATLAB `params.nodalupdtime`; default 1.
    pub nodal_upd_time: Option<usize>,

    /// Kinetics scheme. MATLAB `params.timescheme`; default
    /// [`KineticsScheme::ExponentialTransform`].
    pub time_scheme: Option<KineticsScheme>,

    /// Flux solves per time step for the exponential-transform scheme: one
    /// predictor plus `freq_iter - 1` frequency correctors. MATLAB
    /// `params.freqiter`; default 2, floored at 1.
    pub freq_iter: Option<usize>,

    /// Whether the exponential-transform frequencies are per-group-global or
    /// per-node. MATLAB `params.freqmode`; default
    /// [`FrequencyMode::Global`].
    pub freq_mode: Option<FrequencyMode>,

    /// Prefix of the transient output CSV files. MATLAB `params.outprefix`;
    /// default `"neacrpa2t"`.
    pub out_prefix: Option<String>,

    /// Prompt neutron group velocities \[cm/s\]. MATLAB `params.velocities`.
    pub velocities: Vec<f64>,

    /// Delayed-neutron fractions, one per precursor family \[-\]. MATLAB
    /// `params.beta_dnp`.
    pub beta_dnp: Vec<f64>,

    /// Delayed-neutron decay constants \[1/s\]. MATLAB `params.lambda_dnp`.
    pub lambda_dnp: Vec<f64>,

    /// Control-assembly ejection duration \[s\]. MATLAB
    /// `params.ejectduration`; required only when the case ejects a bank.
    pub eject_duration: Option<f64>,

    /// Path of the `.mat`-equivalent steady-state cache. MATLAB
    /// `params.steadyfile`.
    ///
    /// # Not implemented in the port
    ///
    /// The MATLAB `load`/`save` of a `.mat` file has no translation here, so
    /// the field is carried for round-tripping the case data and **not acted
    /// on**: [`super::transient::solve_coupled_transient`] and
    /// [`super::critical_boron::search_critical_boron`] always run the steady
    /// solve. Reinstating the cache means choosing a serialisation format,
    /// which is a decision for the crate, not for the translation.
    pub steady_file: Option<String>,

    /// Write the MATLAB debug CSV dumps. MATLAB `params.debugdump`.
    ///
    /// Carried but **not acted on** by the coupling layer, which writes no
    /// files at all (see [`super::steady::solve_coupled_steady`]). The nodal
    /// and T-H layers have their own `debugdump` blocks.
    pub debug_dump: bool,

    /// Directory a caller may write the returned histories into. Has no MATLAB
    /// counterpart — MATLAB writes into the working directory — and nothing in
    /// the coupling layer reads it. Present so that a future CSV writer has a
    /// place to be told where to put its output rather than defaulting to the
    /// caller's working directory.
    pub output_dir: Option<String>,

    /// JFNK preconditioner switch. MATLAB `params.jfnkprecon`.
    ///
    /// # Dead control in the snapshot
    ///
    /// `main_exec_diff3d.m:19-21` and `run_neacrpd1t.m:11` set
    /// `params.jfnkprecon`, `params.jfnkrel` and `params.jfnkverb`, and
    /// `main_exec_diff3d.m:54-61` documents `params.ptc` and
    /// `params.jfnk_max_iter` as controls of `driftflux_solverstatic1d.m`.
    /// **No file in the snapshot reads any of them, and
    /// `driftflux_solverstatic1d.m` is not in the snapshot at all.** The
    /// Jacobian-free Newton-Krylov solver those controls belong to is
    /// therefore missing upstream, not omitted here. Carried as a field so the
    /// case data round-trips and so the gap is recorded where a reader meets
    /// it; nothing in this crate reads it either.
    pub jfnk_precon: Option<f64>,

    /// JFNK relaxation factor. MATLAB `params.jfnkrel`. Dead control — see
    /// [`jfnk_precon`](Self::jfnk_precon).
    pub jfnk_rel: Option<f64>,

    /// JFNK verbosity. MATLAB `params.jfnkverb`. Dead control — see
    /// [`jfnk_precon`](Self::jfnk_precon).
    pub jfnk_verb: Option<f64>,
}

/// Time-integration scheme for the flux and the delayed-neutron precursors.
///
/// MATLAB `params.timescheme`. Enum dispatch rather than the MATLAB's integer
/// switch, so a new scheme forces every match site to be revisited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KineticsScheme {
    /// `timescheme = 1` (default). Exponential-transform implicit Euler for the
    /// flux with per-node or per-group frequencies, and analytic precursor
    /// integration assuming a linearly varying transformed fission source over
    /// the step — the scheme of the nodal program Ants (A. Rintala,
    /// U. Lauranto, *Ann. Nucl. Energy* **190** (2023) 109868, Eqs. (3)–(13)).
    #[default]
    ExponentialTransform,
    /// `timescheme = 0`. Plain first-order implicit Euler for both flux and
    /// precursors; the legacy scheme.
    ImplicitEuler,
}

/// How the exponential-transform frequencies are computed.
///
/// MATLAB `params.freqmode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FrequencyMode {
    /// `'global'` (default): one amplitude frequency per energy group, uniform
    /// in space, taken from the volume-integrated group flux. Robust for
    /// super-prompt rod ejections.
    #[default]
    Global,
    /// `'node'`: per-node, per-group frequencies as written in the Ants paper.
    /// Yan Ren records this as **unstable in super-prompt HZP rod ejections** —
    /// node-wise frequency noise near the ejected channel feeds back through
    /// the nearly singular prompt operator.
    Node,
}

// =====================================================================
// Geometry — MATLAB `geometry`
// =====================================================================

/// Core geometry as the coupled drivers need it — the MATLAB `geometry` struct.
///
/// Wraps [`Geometry`] (node sizes and volumes, already owned by
/// [`crate::reference::grid`]) and adds the fuel-rod, control-rod and
/// axial-block data the coupling layer reads.
///
/// # Ownership
///
/// Provisional. [`crate::reference::cases`] builds these; `nodal/` and `th/`
/// read them. Expect the extra fields below to be folded into
/// [`Geometry`] or into a case-layer type.
#[derive(Debug, Clone)]
pub struct CoreGeometry {
    /// Node sizes, volumes and the grid. MATLAB `geometry.Lx/Ly/Lz/Vi`.
    ///
    /// # Length of `lz` — a discrepancy to reconcile
    ///
    /// The coupling layer indexes `base.lz` as **one value per spatial node**
    /// (`Grid::index(0, ix, iy, iz)`), because that is what the MATLAB does:
    /// `sigmavalupd3d_handler.m:57` sums `Lz(idx+1 : idx+iz)` off a flat node
    /// offset, and `neacrpa2t.m:56` fills `geometry.Lz` with
    /// `maxix*maxiy*maxiz` entries. [`Geometry::lz`] currently documents itself
    /// as "one per z index"; the two must be made to agree before either is
    /// trusted, and the MATLAB is the authority.
    pub base: Geometry,

    /// Fuel-rod radial geometry. MATLAB `geometry.fuel`.
    pub fuel: FuelGeometry,

    /// Control-bank positions \[steps withdrawn\], one per bank. MATLAB
    /// `geometry.crod`. Mutated during the transient as the bank moves.
    pub crod: Vec<f64>,

    /// Which control bank covers each `(ix,iy)` column, 0 for none; indexed
    /// `ix*ny + iy`. MATLAB `geometry.crodbanks`.
    pub crod_banks: Vec<usize>,

    /// Axial position of a fully inserted bank tip \[cm\]. MATLAB
    /// `geometry.crodbtm`.
    pub crod_btm: f64,

    /// Length of one control-rod step \[cm\]. MATLAB `geometry.crodstep`.
    pub crod_step: f64,

    /// Index of the ejected bank into [`crod`](Self::crod), **1-based as in the
    /// MATLAB**; 0 or [`None`] means the case has no rod motion (NEACRP D1).
    /// MATLAB `geometry.crodeject`.
    pub crod_eject: Option<usize>,

    /// Final position of the ejected bank \[steps\]. MATLAB
    /// `geometry.crodejectto`.
    pub crod_eject_to: Option<f64>,

    /// Lowest fuel-bearing axial node of each column, **1-based**; indexed
    /// `ix*ny + iy`. MATLAB `geometry.zlows`.
    pub zlows: Vec<usize>,

    /// Highest fuel-bearing axial node of each column, **1-based**; indexed
    /// `ix*ny + iy`. MATLAB `geometry.zhis`. The transient uses it to find each
    /// channel's outlet node.
    pub zhis: Vec<usize>,

    /// Mesh layers per axial benchmark block. MATLAB `geometry.zscale`.
    pub zscale: usize,

    /// Semi-analytic nodal coefficients `A,B,E,F,G,H`, rebuilt whenever the
    /// operators are. MATLAB `geometry.nodalcoeffs` (from `calc_ABEFGHxyz`).
    pub nodal_coeffs: NodalCoefficients,
}

/// Radial fuel-rod geometry — the MATLAB `geometry.fuel` struct.
#[derive(Debug, Clone)]
pub struct FuelGeometry {
    /// Conductivity-material tag of each radial ring: 0 = gap, 1 = fuel,
    /// 2 = cladding. MATLAB `geometry.fuel.whichk`, length
    /// [`CaseParams::fuel_max_ir`].
    pub which_k: Vec<usize>,

    /// Radius of each ring centre \[cm\]. MATLAB `geometry.fuel.Ctr`.
    pub ctr: Vec<f64>,

    /// Fuel pellet radius \[cm\]. MATLAB `geometry.fuel.fuelrad`.
    pub fuel_rad: f64,
}

// =====================================================================
// Thermal-hydraulic state — MATLAB `th`
// =====================================================================

/// Thermal-hydraulic state of the core — the MATLAB `th` struct.
///
/// # Ownership
///
/// Provisional; [`crate::reference::th`] will own this. Only the fields the
/// coupling layer reads or writes are declared. The T-H solver carries several
/// more (`th.coolant.enth`, `press`, `alphag`, `vm`, `ldens`, `gdens`,
/// `quality`, `th.linpwrdens`) that the CHF call needs.
#[derive(Debug, Clone)]
pub struct ThermalState {
    /// Volume-average fuel temperature per node \[K\]. MATLAB `th.fueltempavg`.
    /// Length `nodes`.
    pub fuel_temp_avg: Vec<f64>,

    /// Effective Doppler fuel temperature per node \[K\], the quantity the
    /// cross-section feedback uses (with a square-root law). MATLAB
    /// `th.fueltempdoppler`. Length `nodes`.
    pub fuel_temp_doppler: Vec<f64>,

    /// Radial fuel-rod temperature profile \[K\], `nodes * n_solution_ids`,
    /// indexed `node * n_solution_ids + id`. MATLAB `th.fueltemp`.
    pub fuel_temp: Vec<f64>,

    /// Number of radial solution ids per node — `maxir` plus one node per
    /// material interface. MATLAB `maxid`, computed in `thdiffusion_solverxyz`.
    pub n_solution_ids: usize,

    /// Moderator temperature per node \[K\], used only when the case supplies a
    /// `modtemp` feedback table. MATLAB `th.modtemp`.
    pub mod_temp: Option<Vec<f64>>,

    /// Coolant state.
    pub coolant: CoolantState,

    /// Wall heat flux per node \[W/cm²\]. MATLAB `th.heatflux`. Length `nodes`.
    pub heat_flux: Vec<f64>,

    /// Core power relative to [`max_power`](Self::max_power) \[-\]. MATLAB
    /// `th.powratio`; the transient rescales it every step.
    pub power_ratio: f64,

    /// Rated core thermal power \[W\]. MATLAB `th.maxpow`.
    pub max_power: f64,

    /// Fuel pins per node. MATLAB `th.nfuelpin`.
    pub n_fuel_pins: f64,

    /// Fraction of fission energy deposited directly in the coolant \[-\].
    /// MATLAB `th.coolheatfrac`.
    pub coolant_heat_fraction: f64,

    /// Area-averaged coolant mass flux \[g/s/cm²\]. MATLAB `th.flowrate`.
    pub flow_rate: f64,

    /// Flow direction: `+1` upwards, `-1` downwards. MATLAB `th.flowdir`.
    pub flow_dir: f64,

    /// Time-dependent inlet-temperature forcing. MATLAB `th.inlettemp_t`, a
    /// function handle the case supplies; absent for cases with a fixed inlet.
    pub inlet_temp_schedule: Option<InletTemperatureSchedule>,
}

/// Coolant channel state — the MATLAB `th.coolant` struct.
#[derive(Debug, Clone)]
pub struct CoolantState {
    /// Coolant temperature per node \[K\]. MATLAB `th.coolant.temps`.
    pub temps: Vec<f64>,

    /// Coolant density per node \[g/cm³\]. MATLAB `th.coolant.dens`.
    pub dens: Vec<f64>,

    /// Channel inlet temperature \[K\]. MATLAB `th.coolant.inlettemp`. The
    /// transient overwrites it each step from
    /// [`ThermalState::inlet_temp_schedule`].
    pub inlet_temp: f64,

    /// Channel inlet pressure \[MPa\]. MATLAB `th.coolant.inletpress`.
    pub inlet_press: f64,
}

/// Prescribed inlet-temperature history — the MATLAB `th.inlettemp_t` handle.
///
/// Enum dispatch, per the workspace rule against trait objects: the set of
/// forcings is closed and known from the benchmark cases.
///
/// # Ownership
///
/// Provisional; the case layer defines the forcings, so the variants will grow
/// there. Only the D1 forcing exists in the snapshot.
#[derive(Debug, Clone, Copy)]
pub enum InletTemperatureSchedule {
    /// No time dependence — the inlet stays at
    /// [`CoolantState::inlet_temp`].
    Constant,

    /// NEACRP D1 cold-water injection, `neacrpd1t.m` (spec Fig. 6.1): the
    /// inlet subcooling doubles with a 2.5 s time constant,
    /// `dH(t) = 46.52*(2 - exp(-0.4 t))` kJ/kg below the saturated-liquid
    /// enthalpy at the constant core pressure, converted to a temperature with
    /// an IAPWS-IF97 `(p,h)` flash.
    NeacrpD1ColdWater {
        /// Constant core pressure \[MPa\]. MATLAB `th.coolant.inletpress`.
        inlet_pressure: f64,
        /// Saturated-liquid enthalpy at that pressure \[kJ/kg\]. MATLAB
        /// `hsat0 = IAPWS_IF97('h1_pT', Pin, IAPWS_IF97('Tsat_p', Pin))`.
        saturated_liquid_enthalpy: f64,
    },
}

impl InletTemperatureSchedule {
    /// Inlet temperature \[K\] at time `t` \[s\].
    ///
    /// # Panics
    ///
    /// [`Self::NeacrpD1ColdWater`] is unimplemented: it needs an IAPWS-IF97
    /// `(p,h)` flash, which by `docs/bedok-port-scoping.md` §3 comes from
    /// `tampines-steam-tables` rather than being ported. Wiring that crate in
    /// belongs to [`crate::reference::th`], which owns every other steam-table
    /// call in the reference path.
    #[must_use]
    pub fn evaluate(&self, t: f64, constant_inlet_temp: f64) -> f64 {
        match self {
            Self::Constant => constant_inlet_temp,
            Self::NeacrpD1ColdWater { .. } => {
                let _ = t;
                todo!(
                    "IAPWS-IF97 T_ph flash for the NEACRP D1 inlet forcing; \
                     owned by reference::th via tampines-steam-tables"
                )
            }
        }
    }
}

// =====================================================================
// Cross sections — MATLAB `sigmavalues`, `whichsigma`, `sigma`
// =====================================================================

/// Which composition (or, after a feedback pass, which compacted table row)
/// each spatial node uses — the MATLAB `whichsigma` array.
///
/// Zero means "no material": a void node outside the core, skipped by every
/// loop that walks this map.
///
/// # The two meanings, and why they share a type
///
/// The MATLAB overloads this array, and the translation keeps the overload
/// because the feedback chain depends on it (see
/// [`super::cross_section_feedback`]):
///
/// - As handed in by the case (`whichsigmaref`), entries are **composition
///   ids**, 1-based, indexing the benchmark's material tables.
/// - As returned by a feedback update, entries are **row indices into the
///   compacted per-node table**, 1-based, counting non-void nodes in
///   `ix, iy, iz` order.
#[derive(Debug, Clone)]
pub struct MaterialMap {
    /// The grid it is defined on.
    pub grid: Grid,
    /// One entry per spatial node, indexed `Grid::index(0, ix, iy, iz)`.
    pub ids: Vec<usize>,
}

impl MaterialMap {
    /// A map of `grid.nodes()` entries, all void.
    #[must_use]
    pub fn zeros(grid: Grid) -> Self {
        Self {
            ids: vec![0; grid.nodes()],
            grid,
        }
    }

    /// The id at `(ix, iy, iz)`, all 0-based; 0 means void.
    #[must_use]
    pub fn at(&self, ix: usize, iy: usize, iz: usize) -> usize {
        self.ids[self.grid.index(0, ix, iy, iz)]
    }

    /// Set the id at `(ix, iy, iz)`, all 0-based.
    pub fn set(&mut self, ix: usize, iy: usize, iz: usize, id: usize) {
        let idx = self.grid.index(0, ix, iy, iz);
        self.ids[idx] = id;
    }
}

/// Multigroup cross sections plus the feedback derivative tables — the MATLAB
/// `sigmavalues` struct.
///
/// Rows are compositions on the way in (`sigmavaluesref`) and per-node table
/// entries on the way out of a feedback pass. Row count is
/// [`n_rows`](Self::n_rows).
///
/// # Units
///
/// `tot`, `f` and `s` are macroscopic cross sections \[1/cm\]; `f` is
/// `nu*Sigma_f`. `fp` is `kappa*Sigma_f` \[J/cm\], the power-producing
/// operator. `nu` \[-\] and `chi` \[-\] are the neutron yield and fission
/// spectrum.
#[derive(Debug, Clone)]
pub struct SigmaValues {
    /// Energy groups.
    pub ngroups: usize,
    /// Total cross section, `row*ngroups + g` \[1/cm\].
    pub tot: Vec<f64>,
    /// Fission production `nu*Sigma_f`, `row*ngroups + g` \[1/cm\].
    pub f: Vec<f64>,
    /// Power production `kappa*Sigma_f`, `row*ngroups + g` \[J/cm\].
    pub fp: Vec<f64>,
    /// Scattering matrix, `row*ngroups*ngroups + to*ngroups + from` \[1/cm\].
    ///
    /// The index order follows the MATLAB `sigmavalues.s(w, to, from)`, which
    /// `sigmavalupd3d_handler.m:93` fixes by computing the absorption as
    /// `tot(w,g) - sum(s(w,:,g))` — a sum over destinations at fixed source
    /// group `g`.
    pub s: Vec<f64>,
    /// Neutron yield per fission, `row*ngroups + g` \[-\].
    pub nu: Vec<f64>,
    /// Fission spectrum, `row*ngroups + g` \[-\].
    pub chi: Vec<f64>,
    /// The feedback derivative tables the case supplies.
    pub feedback: FeedbackTables,
}

impl SigmaValues {
    /// Number of table rows — compositions, or nodes after a feedback pass.
    #[must_use]
    pub fn n_rows(&self) -> usize {
        self.tot.len().checked_div(self.ngroups).unwrap_or(0)
    }

    /// Scattering entry `(row, to, from)`, all 0-based \[1/cm\].
    #[must_use]
    pub fn scattering(&self, row: usize, to: usize, from: usize) -> f64 {
        self.s[row * self.ngroups * self.ngroups + to * self.ngroups + from]
    }

    /// Mutable scattering entry `(row, to, from)`, all 0-based \[1/cm\].
    pub fn scattering_mut(&mut self, row: usize, to: usize, from: usize) -> &mut f64 {
        let g = self.ngroups;
        &mut self.s[row * g * g + to * g + from]
    }
}

/// The set of feedback channels a case defines — the optional sub-structs of
/// the MATLAB `sigmavalues` (`sigmavalues.boron`, `.fueltemp`, …).
///
/// `None` is the MATLAB `isfield(...) == false`: the channel is simply not
/// applied.
#[derive(Debug, Clone, Default)]
pub struct FeedbackTables {
    /// Boron concentration \[ppm\], linear.
    pub boron: Option<FeedbackTable>,
    /// Doppler fuel temperature \[K\], square-root law (`m = 0.5`).
    pub fuel_temp: Option<FeedbackTable>,
    /// Moderator temperature \[K\], linear.
    pub mod_temp: Option<FeedbackTable>,
    /// Coolant temperature \[K\], linear.
    pub cool_temp: Option<FeedbackTable>,
    /// Coolant density \[g/cm³\], linear.
    pub cool_den: Option<FeedbackTable>,
    /// Control-rod insertion fraction \[-\], linear about zero.
    pub crod: Option<FeedbackTable>,
}

/// Cross-section derivatives with respect to one feedback variable — the MATLAB
/// `deltasigmavalues` struct.
///
/// Rows are **composition ids**, always: the derivative tables are never
/// compacted, so `sigmavalupd3d.m` indexes them with `whichsigmaref` while it
/// indexes the base values with `whichsigmaold`.
#[derive(Debug, Clone)]
pub struct FeedbackTable {
    /// Energy groups.
    pub ngroups: usize,
    /// Value of the feedback variable the base cross sections were tabulated
    /// at. MATLAB `deltasigmavalues.ref` (`ref` is a Rust keyword).
    pub reference_value: f64,
    /// d(total)/d(variable), `row*ngroups + g`.
    pub tot: Vec<f64>,
    /// d(nu*Sigma_f)/d(variable), `row*ngroups + g`.
    pub f: Vec<f64>,
    /// d(kappa*Sigma_f)/d(variable), `row*ngroups + g`.
    pub fp: Vec<f64>,
    /// d(scattering)/d(variable), `row*ngroups*ngroups + to*ngroups + from`.
    pub s: Vec<f64>,
}

/// The assembled multigroup operators — the MATLAB `sigma` struct from
/// `makesigmadfxyz.m`.
///
/// All four are square, of side
/// `philenf = grid.state_len() + n_components*grid.nodes()`.
///
/// # Ownership
///
/// [`crate::reference::nodal`] builds these.
#[derive(Debug, Clone)]
pub struct SigmaOperators {
    /// Total-removal operator (diagonal) \[1/cm × cm³\]. MATLAB `sigma.tot`.
    pub tot: SparseMatrix,
    /// Scattering operator. MATLAB `sigma.s`.
    pub s: SparseMatrix,
    /// Fission production operator `chi * nu*Sigma_f`. MATLAB `sigma.f`.
    pub f: SparseMatrix,
    /// Power production operator `kappa*Sigma_f`. MATLAB `sigma.fp`.
    pub fp: SparseMatrix,
}

// =====================================================================
// Opaque products of the nodal layer
// =====================================================================

/// Diffusion coefficients per node and group \[cm\] — the MATLAB `DiffD` array
/// from `calcdiffvalues3d.m`.
///
/// # Ownership
///
/// [`crate::reference::nodal`]. Opaque to the coupling layer, which only passes
/// it between nodal calls.
#[derive(Debug, Clone)]
pub struct DiffusionCoefficients {
    /// `nx*ny*nz*ngroups` values, indexed `Grid::index(g, ix, iy, iz)`.
    pub values: Vec<f64>,
}

/// Interface-current terms produced alongside the diffusion operator — the
/// MATLAB `gradterms` from `makegradDxyz.m`.
///
/// # Ownership
///
/// [`crate::reference::nodal`]. Opaque here; passed straight into
/// [`calc_semi_analytic_nodal`].
#[derive(Debug, Clone, Default)]
pub struct GradientTerms {
    /// Placeholder for the nodal layer's own representation.
    pub placeholder: (),
}

/// Semi-analytic nodal coefficients `A, B, E, F, G, H` — the MATLAB
/// `calc_ABEFGHxyz.m` output, stored as `geometry.nodalcoeffs`.
///
/// # Ownership
///
/// [`crate::reference::nodal`]. Opaque here.
#[derive(Debug, Clone, Default)]
pub struct NodalCoefficients {
    /// Placeholder for the nodal layer's own representation.
    pub placeholder: (),
}

/// The six per-node transverse-leakage / expansion terms carried across nodal
/// updates — the MATLAB `nodalterms`, a `philen x 6` array.
///
/// Warm-starting the nodal correction from the previous update is why this is
/// threaded through the drivers rather than rebuilt.
#[derive(Debug, Clone)]
pub struct NodalTerms {
    /// `state_len * 6` values, indexed `state_index*6 + term`.
    pub values: Vec<f64>,
}

impl NodalTerms {
    /// All-zero terms, the MATLAB `zeros(philen,6)` cold start.
    #[must_use]
    pub fn zeros(state_len: usize) -> Self {
        Self {
            values: vec![0.0; state_len * 6],
        }
    }
}

/// What the SA-nodal eigenvalue solver returns — the MATLAB
/// `sanodaldiffusion_solverxyz` output struct.
#[derive(Debug, Clone)]
pub struct DiffusionSolution {
    /// Multiplication factor \[-\]. MATLAB `output.k_eff`.
    pub k_eff: f64,
    /// Converged scalar flux \[n/cm²/s, to an arbitrary normalisation\],
    /// `state_len` entries.
    ///
    /// # Representation note
    ///
    /// MATLAB returns `output.scalar_flux` as a **matrix** whose extra columns
    /// hold the fission-source extrapolation history; every consumer in the
    /// snapshot reads only `scalar_flux(:,1)` (the coupling layer explicitly,
    /// `main_exec_diff3d.m` implicitly through linear indexing). Only that
    /// first column is carried here.
    pub scalar_flux: Vec<f64>,
    /// Fission source `sigma.f * phi`, `state_len` entries.
    pub fission_source: Vec<f64>,
    /// Node power density `fission_source .* Vi`, `state_len` entries.
    pub pwrdens: Vec<f64>,
    /// Final fission-source residual \[-\]. MATLAB `output.residual`.
    pub residual: f64,
    /// Final `k_eff` residual \[-\]. MATLAB `output.k_eff_residual`.
    pub k_eff_residual: f64,
}

/// Critical-heat-flux result — the MATLAB `w3chfhottest.m` output.
///
/// # Ownership
///
/// [`crate::reference::th`]. Opaque here: `thdiffusion_solverxyz.m:191`
/// computes it and **never uses it** — it is not placed in the output struct,
/// so the call is currently dead. Translated as-is.
#[derive(Debug, Clone, Default)]
pub struct ChfResult {
    /// Placeholder for the thermal-hydraulics layer's own representation.
    pub placeholder: (),
}

// =====================================================================
// Calls into the nodal layer
// =====================================================================

/// Assemble the multigroup operators from tabulated cross sections.
///
/// MATLAB `makesigmadfxyz.m`, called as `makesigmadfxyz(params, sigmavalues,
/// whichsigma, 1)` — mode 1, full indices only.
///
/// # Ownership
///
/// **[`crate::reference::nodal`] owns this.** Declared here only so the
/// coupled drivers compile ahead of it.
///
/// # Panics
///
/// Always — the body is [`todo!`].
#[must_use]
pub fn make_sigma_operators(
    params: &CaseParams,
    sigma_values: &SigmaValues,
    which_sigma: &MaterialMap,
) -> SigmaOperators {
    let _ = (params, sigma_values, which_sigma);
    todo!("makesigmadfxyz.m — owned by reference::nodal")
}

/// Diffusion coefficients from the total cross sections.
///
/// MATLAB `calcdiffvalues3d.m`, default mode 1:
/// `D = mode / ((2*mode + 1) * Sigma_tot)`.
///
/// # Ownership
///
/// **[`crate::reference::nodal`] owns this.**
///
/// # Panics
///
/// Always — the body is [`todo!`].
#[must_use]
pub fn calc_diffusion_coefficients(
    params: &CaseParams,
    sigma_tot: &[f64],
    which_sigma: &MaterialMap,
) -> DiffusionCoefficients {
    let _ = (params, sigma_tot, which_sigma);
    todo!("calcdiffvalues3d.m — owned by reference::nodal")
}

/// Finite-difference diffusion operator and the interface-current terms.
///
/// MATLAB `makegradDxyz.m`, returning `[gradD, gradterms]`.
///
/// # Ownership
///
/// **[`crate::reference::nodal`] owns this.**
///
/// # Panics
///
/// Always — the body is [`todo!`].
#[must_use]
pub fn make_gradient_diffusion_operator(
    geometry: &CoreGeometry,
    params: &CaseParams,
    diffusion: &DiffusionCoefficients,
    which_sigma: &MaterialMap,
) -> (SparseMatrix, GradientTerms) {
    let _ = (geometry, params, diffusion, which_sigma);
    todo!("makegradDxyz.m — owned by reference::nodal")
}

/// Semi-analytic nodal coefficients `A, B, E, F, G, H`.
///
/// MATLAB `calc_ABEFGHxyz.m`, stored by the caller into
/// [`CoreGeometry::nodal_coeffs`].
///
/// # Ownership
///
/// **[`crate::reference::nodal`] owns this.**
///
/// # Panics
///
/// Always — the body is [`todo!`].
#[must_use]
pub fn calc_nodal_coefficients(
    params: &CaseParams,
    geometry: &CoreGeometry,
    sigma: &SigmaOperators,
    diffusion: &DiffusionCoefficients,
) -> NodalCoefficients {
    let _ = (params, geometry, sigma, diffusion);
    todo!("calc_ABEFGHxyz.m — owned by reference::nodal")
}

/// One refinement of the semi-analytic nodal correction at a fixed flux.
///
/// MATLAB `calc_sanodalxyz.m`, returning `[nodal, nodalterms]`. Reads
/// `geometry.nodalcoeffs`, so [`calc_nodal_coefficients`] must have been
/// stored into the geometry first.
///
/// # Ownership
///
/// **[`crate::reference::nodal`] owns this.**
///
/// # Panics
///
/// Always — the body is [`todo!`].
// Arity mirrors the MATLAB `calc_sanodalxyz` signature exactly; grouping the
// arguments into a struct here would pre-empt a decision that belongs to
// `reference::nodal`.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn calc_semi_analytic_nodal(
    params: &CaseParams,
    geometry: &CoreGeometry,
    flux: &[f64],
    sigma: &SigmaOperators,
    diffusion: &DiffusionCoefficients,
    gradient_terms: &GradientTerms,
    nodal_terms_old: &NodalTerms,
    k_eff: f64,
) -> (SparseMatrix, NodalTerms) {
    let _ = (
        params,
        geometry,
        flux,
        sigma,
        diffusion,
        gradient_terms,
        nodal_terms_old,
        k_eff,
    );
    todo!("calc_sanodalxyz.m — owned by reference::nodal")
}

/// The SA-nodal `k`-eigenvalue solve — the production eigensolver.
///
/// MATLAB `sanodaldiffusion_solverxyz(geometry, params, sigmavalues,
/// whichsigma, initial_k_eff, initflux)`. `warm_flux` is `varargin{2}`: a
/// previously converged flux used to seed the source iteration instead of a
/// flat guess. `params.innertol`, when set, replaces the default 1e-6 inner
/// tolerance.
///
/// # Ownership
///
/// **[`crate::reference::nodal`] owns this.**
///
/// # Panics
///
/// Always — the body is [`todo!`].
#[must_use]
pub fn solve_sanodal_eigenvalue(
    geometry: &CoreGeometry,
    params: &CaseParams,
    sigma_values: &SigmaValues,
    which_sigma: &MaterialMap,
    initial_k_eff: f64,
    warm_flux: Option<&[f64]>,
) -> DiffusionSolution {
    let _ = (
        geometry,
        params,
        sigma_values,
        which_sigma,
        initial_k_eff,
        warm_flux,
    );
    todo!("sanodaldiffusion_solverxyz.m — owned by reference::nodal")
}

// =====================================================================
// Calls into the thermal-hydraulics layer
// =====================================================================

/// One steady thermal-hydraulics update at a given power distribution.
///
/// MATLAB `th_solverxyz(params, geometry, th, whichsigma, pwrdens)`. Marches
/// the coolant enthalpy up each channel and solves the 1-D cylindrical fuel-rod
/// conduction, returning the updated state. `pwrdens` is the node power
/// \[W per node\], `state_len` entries.
///
/// # Ownership
///
/// **[`crate::reference::th`] owns this.**
///
/// # Panics
///
/// Always — the body is [`todo!`].
#[must_use]
pub fn solve_thermal_hydraulics_steady(
    params: &CaseParams,
    geometry: &CoreGeometry,
    th: &ThermalState,
    which_sigma: &MaterialMap,
    pwrdens: &[f64],
) -> ThermalState {
    let _ = (params, geometry, th, which_sigma, pwrdens);
    todo!("th_solverxyz.m — owned by reference::th")
}

/// One implicit-Euler time step of the thermal hydraulics.
///
/// MATLAB `th_solvertimexyz(params, geometry, th, whichsigma, pwrdens, thold,
/// dt)`. `th` is the current iterate (its `heatflux` feeds the coolant energy
/// source and its `powratio` must already carry the current relative power);
/// `th_old` is the converged state of the previous **time step**, supplying the
/// capacity terms; `dt` is the step \[s\].
///
/// # Ownership
///
/// **[`crate::reference::th`] owns this.**
///
/// # Panics
///
/// Always — the body is [`todo!`].
#[must_use]
pub fn solve_thermal_hydraulics_transient(
    params: &CaseParams,
    geometry: &CoreGeometry,
    th: &ThermalState,
    which_sigma: &MaterialMap,
    pwrdens: &[f64],
    th_old: &ThermalState,
    dt: f64,
) -> ThermalState {
    let _ = (params, geometry, th, which_sigma, pwrdens, th_old, dt);
    todo!("th_solvertimexyz.m — owned by reference::th")
}

/// W-3 critical heat flux evaluated on the hottest channel.
///
/// MATLAB `w3chfhottest.m`.
///
/// # Ownership
///
/// **[`crate::reference::th`] owns this.**
///
/// # Known defect in the MATLAB
///
/// `w3chfhottest.m:22` sets `highy = ix` instead of `highy = iy` when it
/// records the hottest channel, so the search always returns a diagonal
/// column. Recorded, not fixed — see `docs/bedok-port-scoping.md` §1.0.
///
/// # Panics
///
/// Always — the body is [`todo!`].
#[must_use]
pub fn w3_chf_hottest_channel(
    params: &CaseParams,
    geometry: &CoreGeometry,
    th: &ThermalState,
) -> ChfResult {
    let _ = (params, geometry, th);
    todo!("w3chfhottest.m — owned by reference::th")
}

// =====================================================================
// Small shared utilities — MATLAB `fixnegativematrix.m`, `pauseonnan.m`
// =====================================================================

/// Replicate a per-node field across every energy group — MATLAB
/// `repmat(geometry.Vi, G, 1)`.
///
/// The result has `grid.state_len()` entries, with entry
/// `Grid::index(g, ix, iy, iz)` holding the node's value for every `g`. Used
/// to turn node volumes \[cm³\] into the `ViG` vector that converts a fission
/// source into a node power.
///
/// # Panics
///
/// If `per_node.len()` is not `grid.nodes()`.
#[must_use]
pub fn replicate_per_group(grid: &Grid, per_node: &[f64]) -> Vec<f64> {
    assert_eq!(
        per_node.len(),
        grid.nodes(),
        "expected {} per-node values, got {}",
        grid.nodes(),
        per_node.len()
    );
    let mut out = Vec::with_capacity(grid.state_len());
    for _ in 0..grid.ngroups {
        out.extend_from_slice(per_node);
    }
    out
}

/// Zero every negative entry — MATLAB `fixnegativematrix.m`.
///
/// # Faithful quirk
///
/// The MATLAB operates on the result of `find(mat)`, i.e. only on **stored
/// non-zero** entries. For a dense MATLAB array that is every non-zero value,
/// which is what this reproduces: exact zeros are left alone (they are already
/// non-negative) and every negative value becomes zero.
pub fn fix_negative(values: &mut [f64]) {
    for v in values.iter_mut() {
        if *v < 0.0 {
            *v = 0.0;
        }
    }
}

/// The MATLAB `pauseonnan.m` guard, with its column semantics preserved.
///
/// # Faithful quirk — this guard is weaker than it looks
///
/// `pauseonnan.m` is `if any(isnan(input)) ... error(...)`. On a **matrix**,
/// MATLAB's `any` reduces down columns and returns a row vector, and an `if`
/// on a vector is true only when **every** element is non-zero. So for a
/// 2-D input the guard fires only when *every column* contains at least one
/// NaN — a single NaN, or a whole NaN row, passes silently. That behaviour is
/// reproduced here rather than tightened: the cross-section arrays it guards
/// are all 2-D or 3-D.
///
/// `data` is row-major with `ncols` columns, matching the storage in
/// [`SigmaValues`].
///
/// # Errors
///
/// [`super::error::CouplingError::NotANumber`] under the condition above.
pub fn pause_on_nan(field: &'static str, data: &[f64], ncols: usize) -> Result<()> {
    if ncols == 0 || data.is_empty() {
        return Ok(());
    }
    let nrows = data.len() / ncols;
    let every_column_has_a_nan =
        (0..ncols).all(|c| (0..nrows).any(|r| data[r * ncols + c].is_nan()));
    if every_column_has_a_nan {
        return Err(super::error::CouplingError::NotANumber { field });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_entries_are_zeroed_and_others_left_alone() {
        let mut v = [1.0, -1.0, 0.0, -0.0, 2.5];
        fix_negative(&mut v);
        assert_eq!(v, [1.0, 0.0, 0.0, -0.0, 2.5]);
    }

    #[test]
    fn nan_guard_reproduces_the_matlab_column_semantics() {
        // 2 rows x 2 columns, row-major. One NaN in column 0 only: MATLAB's
        // `if any(isnan(M))` is false here, so no error.
        let partial = [f64::NAN, 1.0, 2.0, 3.0];
        assert!(pause_on_nan("partial", &partial, 2).is_ok());

        // A NaN in every column does trip it.
        let full = [f64::NAN, 1.0, 2.0, f64::NAN];
        assert!(pause_on_nan("full", &full, 2).is_err());
    }

    #[test]
    fn material_map_round_trips_through_the_grid_convention() {
        let grid = Grid::new(3, 4, 5, 2).expect("valid grid");
        let mut map = MaterialMap::zeros(grid);
        map.set(2, 3, 4, 7);
        assert_eq!(map.at(2, 3, 4), 7);
        assert_eq!(map.ids.len(), grid.nodes());
        assert_eq!(map.at(0, 0, 0), 0);
    }
}
