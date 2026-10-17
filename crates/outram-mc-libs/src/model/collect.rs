//! **How [`Diagnostics`] become tallies, and tallies become results**
//! (gh #940). Internal to the beginner API: the public types are the result
//! structs below and [`MCDiagnosticResult`].
//!
//! # The plan
//!
//! Each diagnostic asks for the tallies (`tally::Tally`, filters × scores)
//! and leakage grids it reads; a request equal to one already made shares
//! it. All are scored on the same histories by
//! [`CsgPowerIteration::step_scored`](crate::physics::transport_csg::CsgPowerIteration::step_scored).
//!
//! | diagnostic | tally filters | scores | leakage grid |
//! |---|---|---|---|
//! | NeutronBalance | cell | flux, absorption, fission, ν-fission | `[0, ∞)` |
//! | Ealf | energy (preset 1000-bin grid) | fission | — |
//! | Leakage | — | — | the grid, with under/overflow |
//! | SixFactors | energy (3 groups, `[0, ∞)`), material | absorption, ν-fission | the 3 groups |
//! | Spectrum | energy (grid, with under/overflow), cell (region) | flux | — |
//! | Kinetics | — (IFP lineage on the power iteration) | — | — |
//! | FissionMap | mesh | fission | — |
//! | FluxMap | mesh, energy | flux | — |
//!
//! # Statistics
//!
//! Every quantity is **per source neutron** (the generation's population).
//! A quantity built from several bins (a group sum, a region's spectrum, a
//! ratio) is formed **per active generation** and its mean and standard
//! error taken over the generations, so correlations between bins are
//! carried rather than assumed away. Ratios and products (the six factors,
//! EALF, β_eff, Λ, the balance closure) use the delta method with the full
//! covariance of their inputs across generations ([`delta_method`]). One
//! bin on its own (a spectrum bin, a map cell) takes its standard error from
//! its tally bin. Standard errors ignore cycle-to-cycle correlation, as
//! `k`'s does (see `RunProvenance::sigma_batch_means`).

use crate::geometry::cell::CellFill;
use crate::physics::ifp::IfpSettings;
use crate::physics::reactor_physics::{assemble_six_factors, Estimate};
use crate::physics::transport_csg::{ExtraScoring, GenerationTotals};
use crate::tally::filter::{CellFilter, EnergyFilter, FilterKind, MaterialFilter, MeshFilter};
use crate::tally::mesh::{MeshKind, RegularMesh};
use crate::tally::tally::{ScoreType, Tally, TallyBin};

use super::diagnostics::{BalanceBy, DiagnosticError, MCDiagnostic, Region};
use super::model::AssembledModel;
use super::shape::Component;

// ─────────────────────────────────────────────────────────────────────────────
// Results
// ─────────────────────────────────────────────────────────────────────────────

/// **Run health**: is the run itself trustworthy?
#[derive(Debug, Clone, PartialEq)]
pub struct RunHealth {
    /// k per cycle, inactive first.
    pub k_by_cycle: Vec<f64>,
    /// Source entropy \[bits\] per cycle.
    pub entropy_by_cycle: Vec<Option<f64>>,
    /// Neutrons per cycle.
    pub particles_by_cycle: Vec<usize>,
    /// Inactive cycles.
    pub inactive_cycles: usize,
    /// The convergence verdict (see `RunProvenance::convergence`).
    pub converged: bool,
    /// Source histories over all cycles.
    pub histories: u64,
    /// Real collisions per source history.
    pub collisions_per_history: f64,
    /// Histories lost (no cell found), scored as leaks. Should be 0.
    pub lost_locate: u64,
    /// Histories ended by the event cap, scored as leaks. Should be 0.
    pub stuck_events: u64,
    /// Escapes with no surface ahead (a model not closed). Should be 0.
    pub leak_infinity: u64,
    /// Negative distances to a boundary. Should be 0.
    pub negative_distances: u64,
    /// Transport wall time \[s\] (all cycles; not measured on wasm32).
    pub transport_seconds: Option<f64>,
    /// The machine (`perf_report::HardwareInfo::headline`).
    pub hardware: String,
    /// Threads the transport used.
    pub threads: usize,
}

impl RunHealth {
    /// Whether every "should be 0" counter is 0.
    pub fn clean(&self) -> bool {
        self.lost_locate == 0
            && self.stuck_events == 0
            && self.leak_infinity == 0
            && self.negative_distances == 0
    }
}

/// One row of a [`NeutronBalance`]: rates per source neutron.
#[derive(Debug, Clone, PartialEq)]
pub struct BalanceRow {
    /// What the row covers (a material, a component, or "total").
    pub label: String,
    /// Absorption (capture + fission, MT=27 as OpenMC).
    pub absorption: Estimate,
    /// Fission.
    pub fission: Estimate,
    /// Capture = absorption − fission.
    pub capture: Estimate,
    /// ν-fission production.
    pub production: Estimate,
    /// Track-length flux, volume-integrated \[cm per source neutron\].
    pub flux: Estimate,
}

/// **Where neutrons end**, per source neutron.
#[derive(Debug, Clone, PartialEq)]
pub struct NeutronBalance {
    /// The split asked for.
    pub by: BalanceBy,
    /// One row per material or component (empty for `BalanceBy::Total`).
    pub rows: Vec<BalanceRow>,
    /// The whole model.
    pub total: BalanceRow,
    /// Leakage (every escape).
    pub leakage: Estimate,
    /// Analog counts of how neutrons ended, per source neutron: fission,
    /// capture, leakage, and their sum (`1 +` the net (n,xn) gain).
    pub analog_fission: Estimate,
    /// See [`Self::analog_fission`].
    pub analog_capture: Estimate,
    /// See [`Self::analog_fission`].
    pub analog_leakage: Estimate,
    /// See [`Self::analog_fission`].
    pub ends_per_source: Estimate,
    /// **The balance check**: `(absorption + leakage) / ends − 1`, the
    /// track-length losses against the analog count of ends. Zero within
    /// statistics when the balance closes.
    pub closure: Estimate,
}

/// **The energy of the average lethargy causing fission.**
#[derive(Debug, Clone, PartialEq)]
pub struct Ealf {
    /// EALF \[eV\].
    pub ealf_ev: Estimate,
    /// The binning bound: each fission is placed at its bin's lethargy
    /// midpoint, so `|ln EALF − ln EALF_exact| ≤` this (half the widest bin
    /// in lethargy).
    pub discretisation_bound_ln: f64,
    /// Fraction of the fission rate outside the grid (not counted).
    pub fission_outside_grid: f64,
}

impl Ealf {
    /// EALF as an energy.
    pub fn energy(&self) -> uom::si::f64::Energy {
        uom::si::f64::Energy::new::<uom::si::energy::electronvolt>(self.ealf_ev.mean)
    }
}

/// **Leakage**, per source neutron.
#[derive(Debug, Clone, PartialEq)]
pub struct Leakage {
    /// Neutrons leaking per source neutron (all energies).
    pub per_source: Estimate,
    /// Leaked / ended (the leakage probability of a neutron).
    pub fraction_of_ends: Estimate,
    /// The grid's edges \[eV\].
    pub edges_ev: Vec<f64>,
    /// Leakage per source neutron in each bin.
    pub per_bin: Vec<Estimate>,
    /// Leakage below / above the grid per source neutron.
    pub outside_grid: (f64, f64),
}

/// **The six-factor formula** on a three-group split, from one run.
/// `η·f·p·ε·P_FNL·P_TNL = P / (A + L)` exactly
/// (`physics::reactor_physics::assemble_six_factors`); see
/// [`Self::k_tracklength`] for how that compares with k.
///
/// **Undefined in a fast system.** The factors divide by thermal rates
/// (η by the fuel's thermal absorption, ε by the thermal production, ...).
/// Where no neutron is absorbed below the thermal edge (Godiva) every
/// factor is 0/0: they are then `NaN` and [`Self::undefined_because`] says
/// why, rather than a zero that reads as a result. The group rates and
/// [`Self::production_over_losses`] stay defined.
#[derive(Debug, Clone, PartialEq)]
pub struct SixFactors {
    /// Reproduction factor.
    pub eta: Estimate,
    /// Thermal utilisation.
    pub f: Estimate,
    /// Resonance escape probability (three-group, leakage-corrected).
    pub p: Estimate,
    /// Fast fission factor.
    pub epsilon: Estimate,
    /// Fast (+ resonance) non-leakage probability.
    pub p_fnl: Estimate,
    /// Thermal non-leakage probability.
    pub p_tnl: Estimate,
    /// The product of the six factors (`NaN` when undefined).
    pub k_from_factors: Estimate,
    /// `P / (A + L)`, computed directly: what the product telescopes to,
    /// defined whether or not the factors are.
    pub production_over_losses: Estimate,
    /// Why the factors are undefined (`None` when they are defined).
    pub undefined_because: Option<String>,
    /// The track-length ν-fission production per source neutron, `P`: the
    /// track-length estimate of k. `k_from_factors × (A + L) = P` exactly;
    /// `A + L = 1 +` the net (n,xn) gain, so `k_from_factors` sits below k
    /// by that gain, and `P` is the number to compare with k.
    pub k_tracklength: Estimate,
    /// Absorption per source neutron, `[thermal, resonance, fast]`.
    pub absorption: [Estimate; 3],
    /// ν-fission production per source neutron, same groups.
    pub production: [Estimate; 3],
    /// Leakage per source neutron, same groups.
    pub leakage: [Estimate; 3],
    /// Thermal absorption in the fuel (materials holding heavy metal).
    pub thermal_absorption_fuel: Estimate,
    /// The edges \[eV\]: `(thermal, fast)`.
    pub edges_ev: (f64, f64),
}

/// **A flux spectrum** in a region.
#[derive(Debug, Clone, PartialEq)]
pub struct Spectrum {
    /// Where.
    pub region: Region,
    /// The region's label ("whole model" or the component's).
    pub label: String,
    /// The grid's edges \[eV\].
    pub edges_ev: Vec<f64>,
    /// Track-length flux per bin, volume-integrated \[cm per source
    /// neutron\].
    pub flux: Vec<Estimate>,
    /// Flux per unit lethargy, normalised to unit area over the grid
    /// (`Σ ψ_i Δu_i = 1`). The σ is the bin's own; the normalisation is
    /// taken as exact.
    pub per_lethargy: Vec<Estimate>,
    /// `Σ φ_i` over the grid \[cm per source neutron\].
    pub total_in_grid: Estimate,
    /// Flux below / above the grid \[cm per source neutron\].
    pub outside_grid: (f64, f64),
}

/// **Kinetics by IFP.**
#[derive(Debug, Clone, PartialEq)]
pub struct Kinetics {
    /// Adjoint-weighted delayed-neutron fraction.
    pub beta_eff: Estimate,
    /// Adjoint-weighted generation time Λ \[s\] =
    /// time numerator / (denominator × k), as OpenMC
    /// (`docs/source/usersguide/kinetics.rst`, eq. `lambda_eff`).
    pub generation_time_s: Estimate,
    /// Generations of lineage.
    pub ifp_generations: usize,
    /// Active cycles whose fissions scored (lineage deep enough).
    pub scoring_cycles: usize,
}

impl Kinetics {
    /// Λ as a time.
    pub fn generation_time(&self) -> uom::si::f64::Time {
        uom::si::f64::Time::new::<uom::si::time::second>(self.generation_time_s.mean)
    }
}

/// **A map on a regular mesh**: one value per cell, x fastest, then y, z.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshMap {
    /// The mesh.
    pub mesh: RegularMesh,
    /// The cell size \[cm\].
    pub cell_cm: [f64; 3],
    /// What the values are.
    pub quantity: String,
    /// The energy-group edges \[eV\] (one group for a fission map).
    pub group_edges_ev: Vec<f64>,
    /// `values[g][cell]` per source neutron per cm³ of cell, for each group;
    /// for a flux map the last entry is the total.
    pub values: Vec<Vec<Estimate>>,
}

impl MeshMap {
    /// The cell size as lengths.
    pub fn cell_size(&self) -> [uom::si::f64::Length; 3] {
        self.cell_cm
            .map(uom::si::f64::Length::new::<uom::si::length::centimeter>)
    }
}

/// **One diagnostic's result**, the variant matching its [`MCDiagnostic`].
#[derive(Debug, Clone, PartialEq)]
pub enum MCDiagnosticResult {
    /// [`MCDiagnostic::RunHealth`].
    RunHealth(RunHealth),
    /// [`MCDiagnostic::NeutronBalance`].
    NeutronBalance(NeutronBalance),
    /// [`MCDiagnostic::Ealf`].
    Ealf(Ealf),
    /// [`MCDiagnostic::Leakage`].
    Leakage(Leakage),
    /// [`MCDiagnostic::SixFactors`].
    SixFactors(SixFactors),
    /// [`MCDiagnostic::Spectrum`].
    Spectrum(Spectrum),
    /// [`MCDiagnostic::Kinetics`].
    Kinetics(Kinetics),
    /// [`MCDiagnostic::FissionMap`].
    FissionMap(MeshMap),
    /// [`MCDiagnostic::FluxMap`].
    FluxMap(MeshMap),
}

// ─────────────────────────────────────────────────────────────────────────────
// Statistics helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Mean and standard error of a series (0 σ for fewer than two values).
pub(crate) fn series_estimate(x: &[f64]) -> Estimate {
    let n = x.len();
    if n == 0 {
        return Estimate::exact(0.0);
    }
    let mean = x.iter().sum::<f64>() / n as f64;
    if n < 2 {
        return Estimate::exact(mean);
    }
    let var = x.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / (n - 1) as f64;
    Estimate {
        mean,
        std: (var / n as f64).sqrt(),
    }
}

/// **The delta method with covariance**: `f` of the means of the series
/// `x[g][i]` (generation `g`, input `i`), with
/// `σ² = ∇fᵀ Σ ∇f / n`, `Σ` the sample covariance of the inputs across
/// generations and `∇f` by central differences at the means. Exact to first
/// order for any smooth `f`; for a ratio of means it is the standard
/// ratio-estimator variance.
pub(crate) fn delta_method<F: Fn(&[f64]) -> f64>(x: &[Vec<f64>], f: F) -> Estimate {
    let n = x.len();
    if n == 0 {
        return Estimate::exact(f64::NAN);
    }
    let m = x[0].len();
    let mean: Vec<f64> = (0..m)
        .map(|i| x.iter().map(|g| g[i]).sum::<f64>() / n as f64)
        .collect();
    let value = f(&mean);
    if n < 2 || !value.is_finite() {
        return Estimate::exact(value);
    }
    let grad: Vec<f64> = (0..m)
        .map(|i| {
            let h = 1.0e-6 * mean[i].abs().max(1.0e-300);
            let (mut a, mut b) = (mean.clone(), mean.clone());
            a[i] += h;
            b[i] -= h;
            (f(&a) - f(&b)) / (2.0 * h)
        })
        .collect();
    let mut var = 0.0;
    for g in x {
        let d: f64 = (0..m).map(|i| grad[i] * (g[i] - mean[i])).sum();
        var += d * d;
    }
    var /= (n - 1) as f64;
    Estimate {
        mean: value,
        std: (var / n as f64).sqrt(),
    }
}

/// A persistent bin as a per-source estimate over `n` realizations.
fn bin_estimate(b: &TallyBin, n: u64, per_source: f64) -> Estimate {
    let mean = b.mean(n) / per_source;
    if mean == 0.0 {
        return Estimate::exact(0.0);
    }
    let r = b.rel_std_dev(n);
    Estimate {
        mean,
        std: if r.is_finite() { mean.abs() * r } else { 0.0 },
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// The collector
// ─────────────────────────────────────────────────────────────────────────────

/// One diagnostic's plan and its per-generation accumulators.
#[derive(Debug, Clone)]
enum Plan {
    RunHealth,
    Balance {
        by: BalanceBy,
        tally: usize,
        leak: usize,
        /// Rows: label and the cells (tally bins) summed into it.
        rows: Vec<(String, Vec<usize>)>,
        n_cells: usize,
        /// Per generation: per row (incl. a final total row) the 4 scores,
        /// then leakage, then analog fission, capture, leak, ends.
        series: Vec<Vec<f64>>,
    },
    Ealf {
        tally: usize,
        mid_ln: Vec<f64>,
        bound_ln: f64,
        /// Per generation: Σ F·ū, Σ F (in grid), F outside.
        series: Vec<Vec<f64>>,
        outside: f64,
        total: f64,
    },
    Leakage {
        leak: usize,
        /// Index range of the grid's own bins inside the leak grid.
        inner: std::ops::Range<usize>,
        edges: Vec<f64>,
        /// Per generation: leak total, ends.
        series: Vec<Vec<f64>>,
    },
    SixFactors {
        tally: usize,
        leak: usize,
        n_mat: usize,
        fuel: Vec<bool>,
        edges: (f64, f64),
        /// Per generation: A[3], P[3], L[3], A_fuel_thermal.
        series: Vec<Vec<f64>>,
    },
    Spectrum {
        region: Region,
        label: String,
        tally: usize,
        n_cells: usize,
        inner: std::ops::Range<usize>,
        edges: Vec<f64>,
        /// Per bin (all bins incl. under/overflow), the region's sum.
        bins: Vec<TallyBin>,
        /// Per generation: in-grid total.
        series: Vec<f64>,
    },
    Kinetics {
        generations: usize,
        /// Per generation: β numerator, time numerator, denominator, k.
        series: Vec<Vec<f64>>,
    },
    Map {
        fission: bool,
        tally: usize,
        mesh: RegularMesh,
        groups: Vec<f64>,
        /// Flux maps: per cell, the sum over groups.
        total: Vec<TallyBin>,
    },
}

/// Turns a [`super::Diagnostics`] into tallies, steps them, and assembles
/// the results.
#[derive(Debug, Clone)]
pub(crate) struct Collector {
    pub(crate) extra: ExtraScoring,
    items: Vec<MCDiagnostic>,
    plans: Vec<Plan>,
    ifp: Option<IfpSettings>,
    active: u64,
    population: f64,
    health: GenerationTotals,
}

/// What the run hands the collector at the end.
pub(crate) struct RunContext {
    pub k_by_cycle: Vec<f64>,
    pub entropy_by_cycle: Vec<Option<f64>>,
    pub particles_by_cycle: Vec<usize>,
    pub inactive_cycles: usize,
    pub converged: bool,
    pub transport_seconds: Option<f64>,
}

fn add_tally(extra: &mut ExtraScoring, filters: Vec<FilterKind>, scores: Vec<ScoreType>) -> usize {
    let t = Tally {
        id: 0,
        name: "diagnostics".into(),
        filters,
        scores,
        bins: vec![],
    };
    let mut i = 0;
    while let Some(have) = extra.tallies().get(i) {
        if have.filters == t.filters && have.scores == t.scores {
            return i;
        }
        i += 1;
    }
    extra.add_tally(t)
}

fn add_leak(extra: &mut ExtraScoring, edges: Vec<f64>) -> usize {
    let mut i = 0;
    while let Some(have) = extra.leak_grids().get(i) {
        if *have == edges {
            return i;
        }
        i += 1;
    }
    extra.add_leak_grid(edges)
}

/// `edges` with an underflow bin below (from 0) and an overflow bin above
/// (to `f64::MAX`) when the grid does not already reach them; returns the
/// padded edges and the range of the grid's own bins.
fn padded(edges: &[f64]) -> (Vec<f64>, std::ops::Range<usize>) {
    let mut v = Vec::with_capacity(edges.len() + 2);
    let lo = edges[0] > 0.0;
    let hi = *edges.last().unwrap_or(&0.0) < f64::MAX;
    if lo {
        v.push(0.0);
    }
    v.extend_from_slice(edges);
    if hi {
        v.push(f64::MAX);
    }
    let start = usize::from(lo);
    (v, start..start + edges.len() - 1)
}

fn component_label(a: &AssembledModel, c: Component) -> String {
    a.component_labels
        .get(c.index)
        .cloned()
        .unwrap_or_else(|| format!("component {}", c.index))
}

impl Collector {
    /// Plan every diagnostic on the assembled model.
    pub(crate) fn new(items: &[MCDiagnostic], a: &AssembledModel) -> Result<Self, DiagnosticError> {
        let mut extra = ExtraScoring::new();
        let mut plans = Vec::with_capacity(items.len());
        let mut ifp: Option<IfpSettings> = None;
        let n_cells = a.geometry.cells.len();
        let all_cells: Vec<usize> = (0..n_cells).collect();
        let n_mat = a.materials.len();
        let cell_material: Vec<Option<usize>> = a
            .geometry
            .cells
            .iter()
            .map(|c| match c.fill {
                CellFill::Material(m) => Some(m),
                _ => None,
            })
            .collect();
        for d in items {
            let plan = match d {
                MCDiagnostic::RunHealth => Plan::RunHealth,
                MCDiagnostic::NeutronBalance { by } => {
                    let tally = add_tally(
                        &mut extra,
                        vec![FilterKind::Cell(CellFilter {
                            cell_indices: all_cells.clone(),
                        })],
                        vec![
                            ScoreType::Flux,
                            ScoreType::Absorption,
                            ScoreType::Fission,
                            ScoreType::NuFission,
                        ],
                    );
                    let leak = add_leak(&mut extra, vec![0.0, f64::MAX]);
                    let rows: Vec<(String, Vec<usize>)> = match by {
                        BalanceBy::Total => vec![],
                        BalanceBy::Material => (0..n_mat)
                            .map(|m| {
                                (
                                    a.resolved[m].name.clone(),
                                    (0..n_cells)
                                        .filter(|&c| cell_material[c] == Some(m))
                                        .collect(),
                                )
                            })
                            .collect(),
                        BalanceBy::Component => {
                            let mut rows: Vec<(Option<Component>, Vec<usize>)> = Vec::new();
                            for c in 0..n_cells {
                                if cell_material[c].is_none() {
                                    continue;
                                }
                                let k = a.cell_components[c];
                                match rows.iter_mut().find(|r| r.0 == k) {
                                    Some(r) => r.1.push(c),
                                    None => rows.push((k, vec![c])),
                                }
                            }
                            rows.into_iter()
                                .map(|(k, cells)| {
                                    let label = match k {
                                        Some(c) => component_label(a, c),
                                        None => "(no component)".to_string(),
                                    };
                                    (label, cells)
                                })
                                .collect()
                        }
                    };
                    Plan::Balance {
                        by: *by,
                        tally,
                        leak,
                        rows,
                        n_cells,
                        series: Vec::new(),
                    }
                }
                MCDiagnostic::Ealf => {
                    let edges = super::diagnostics::EnergyGrid::default_spectrum().edges_ev()?;
                    let (full, inner) = padded(&edges);
                    let mid_ln: Vec<f64> = full
                        .windows(2)
                        .enumerate()
                        .map(|(i, w)| {
                            if inner.contains(&i) {
                                0.5 * (w[0].ln() + w[1].ln())
                            } else {
                                f64::NAN
                            }
                        })
                        .collect();
                    let bound_ln = edges
                        .windows(2)
                        .map(|w| 0.5 * (w[1] / w[0]).ln())
                        .fold(0.0, f64::max);
                    let tally = add_tally(
                        &mut extra,
                        vec![FilterKind::Energy(EnergyFilter { bins: full })],
                        vec![ScoreType::Fission],
                    );
                    Plan::Ealf {
                        tally,
                        mid_ln,
                        bound_ln,
                        series: Vec::new(),
                        outside: 0.0,
                        total: 0.0,
                    }
                }
                MCDiagnostic::Leakage { grid } => {
                    let edges = grid.edges_ev()?;
                    let (full, inner) = padded(&edges);
                    let leak = add_leak(&mut extra, full);
                    Plan::Leakage {
                        leak,
                        inner,
                        edges,
                        series: Vec::new(),
                    }
                }
                MCDiagnostic::SixFactors {
                    thermal_edge,
                    fast_edge,
                } => {
                    let edges = super::diagnostics::EnergyGrid::TotalAndThreeGroup {
                        thermal_edge: *thermal_edge,
                        fast_edge: *fast_edge,
                    }
                    .edges_ev()?;
                    let tally = add_tally(
                        &mut extra,
                        vec![
                            FilterKind::Energy(EnergyFilter {
                                bins: edges.clone(),
                            }),
                            FilterKind::Material(MaterialFilter {
                                material_indices: (0..n_mat).collect(),
                            }),
                        ],
                        vec![ScoreType::Absorption, ScoreType::NuFission],
                    );
                    let leak = add_leak(&mut extra, edges.clone());
                    let fuel = a
                        .resolved
                        .iter()
                        .map(|r| r.nuclides.iter().any(|(i, _)| i.is_heavy_metal()))
                        .collect();
                    Plan::SixFactors {
                        tally,
                        leak,
                        n_mat,
                        fuel,
                        edges: (edges[1], edges[2]),
                        series: Vec::new(),
                    }
                }
                MCDiagnostic::Spectrum { region, grid } => {
                    let edges = grid.edges_ev()?;
                    let (full, inner) = padded(&edges);
                    let (cells, label) = match region {
                        Region::WholeModel => (all_cells.clone(), "whole model".to_string()),
                        Region::Component(c) => {
                            let cells: Vec<usize> = (0..n_cells)
                                .filter(|&i| a.cell_components[i] == Some(*c))
                                .collect();
                            if cells.is_empty() {
                                return Err(DiagnosticError::Component(format!(
                                    "a Spectrum asks for component {} but no shape of this model is in it \
                                     (made by another model, or never given to a shape with .in_component(..))",
                                    c.index
                                )));
                            }
                            (cells, component_label(a, *c))
                        }
                    };
                    let n = cells.len();
                    let nb = full.len() - 1;
                    let tally = add_tally(
                        &mut extra,
                        vec![
                            FilterKind::Energy(EnergyFilter { bins: full.clone() }),
                            FilterKind::Cell(CellFilter {
                                cell_indices: cells,
                            }),
                        ],
                        vec![ScoreType::Flux],
                    );
                    Plan::Spectrum {
                        region: *region,
                        label,
                        tally,
                        n_cells: n,
                        inner,
                        edges: full,
                        bins: vec![TallyBin::default(); nb],
                        series: Vec::new(),
                    }
                }
                MCDiagnostic::Kinetics { ifp_generations } => {
                    match ifp {
                        Some(s) if s.n_generation != *ifp_generations => {
                            return Err(DiagnosticError::Invalid(format!(
                                "two Kinetics with different IFP generations ({} and {}): one run carries one lineage",
                                s.n_generation, ifp_generations
                            )))
                        }
                        _ => {
                            ifp = Some(IfpSettings {
                                n_generation: *ifp_generations,
                                ..IfpSettings::default()
                            })
                        }
                    }
                    Plan::Kinetics {
                        generations: *ifp_generations,
                        series: Vec::new(),
                    }
                }
                MCDiagnostic::FissionMap { mesh } => {
                    let mesh = mesh.resolve(a.bounds)?;
                    let tally = add_tally(
                        &mut extra,
                        vec![FilterKind::Mesh(MeshFilter {
                            mesh: MeshKind::Regular(mesh.clone()),
                        })],
                        vec![ScoreType::Fission],
                    );
                    Plan::Map {
                        fission: true,
                        tally,
                        mesh,
                        groups: vec![0.0, f64::MAX],
                        total: Vec::new(),
                    }
                }
                MCDiagnostic::FluxMap { mesh, grid } => {
                    let mesh = mesh.resolve(a.bounds)?;
                    let groups = grid.edges_ev()?;
                    let tally = add_tally(
                        &mut extra,
                        vec![
                            FilterKind::Mesh(MeshFilter {
                                mesh: MeshKind::Regular(mesh.clone()),
                            }),
                            FilterKind::Energy(EnergyFilter {
                                bins: groups.clone(),
                            }),
                        ],
                        vec![ScoreType::Flux],
                    );
                    let n = mesh.n_bins();
                    Plan::Map {
                        fission: false,
                        tally,
                        mesh,
                        groups,
                        total: vec![TallyBin::default(); n],
                    }
                }
            };
            plans.push(plan);
        }
        Ok(Collector {
            extra,
            items: items.to_vec(),
            plans,
            ifp,
            active: 0,
            population: 0.0,
            health: GenerationTotals::default(),
        })
    }

    /// The IFP settings to enable on the power iteration, if any.
    pub(crate) fn ifp(&self) -> Option<IfpSettings> {
        self.ifp
    }

    fn absorb_health(&mut self, t: &GenerationTotals) {
        let h = &mut self.health;
        h.histories += t.histories;
        h.collisions += t.collisions;
        h.lost_locate += t.lost_locate;
        h.stuck_events += t.stuck_events;
        h.leak_vacuum += t.leak_vacuum;
        h.leak_infinity += t.leak_infinity;
        h.neg_dist += t.neg_dist;
    }

    /// An inactive generation: count its health, drop its scores.
    pub(crate) fn inactive(&mut self, t: &GenerationTotals) {
        self.absorb_health(t);
        self.extra.discard();
    }

    /// An active generation of `population` neutrons with eigenvalue `k`.
    pub(crate) fn active(&mut self, t: &GenerationTotals, population: usize, k: f64) {
        self.absorb_health(t);
        self.active += 1;
        self.population = population as f64;
        let n = population as f64;
        let x = &self.extra;
        let ends = t.ends;
        for plan in &mut self.plans {
            match plan {
                Plan::RunHealth => {}
                Plan::Balance {
                    tally,
                    leak,
                    rows,
                    n_cells,
                    series,
                    ..
                } => {
                    let b = x.batch(*tally);
                    let cell = |c: usize, s: usize| b[c * 4 + s] / n;
                    let mut v = Vec::with_capacity(4 * (rows.len() + 1) + 5);
                    for (_, cells) in rows.iter() {
                        for s in 0..4 {
                            v.push(cells.iter().map(|&c| cell(c, s)).sum());
                        }
                    }
                    for s in 0..4 {
                        v.push((0..*n_cells).map(|c| cell(c, s)).sum());
                    }
                    v.push(x.leak_batch(*leak).iter().sum::<f64>() / n);
                    v.push(ends.fissions() as f64 / n);
                    v.push(ends.captured as f64 / n);
                    v.push(ends.leaked as f64 / n);
                    v.push(ends.tracked as f64 / n);
                    series.push(v);
                }
                Plan::Ealf {
                    tally,
                    mid_ln,
                    series,
                    outside,
                    total,
                    ..
                } => {
                    let b = x.batch(*tally);
                    let (mut num, mut den, mut out) = (0.0, 0.0, 0.0);
                    for (i, &f) in b.iter().enumerate() {
                        if mid_ln[i].is_nan() {
                            out += f;
                        } else {
                            num += f * mid_ln[i];
                            den += f;
                        }
                    }
                    *outside += out;
                    *total += out + den;
                    series.push(vec![num / n, den / n]);
                }
                Plan::Leakage { leak, series, .. } => {
                    series.push(vec![
                        x.leak_batch(*leak).iter().sum::<f64>() / n,
                        ends.tracked as f64 / n,
                    ]);
                }
                Plan::SixFactors {
                    tally,
                    leak,
                    n_mat,
                    fuel,
                    series,
                    ..
                } => {
                    let b = x.batch(*tally);
                    let l = x.leak_batch(*leak);
                    let mut v = vec![0.0; 10];
                    for g in 0..3 {
                        for m in 0..*n_mat {
                            let base = (g * *n_mat + m) * 2;
                            v[g] += b[base] / n;
                            v[3 + g] += b[base + 1] / n;
                            if g == 0 && fuel[m] {
                                v[9] += b[base] / n;
                            }
                        }
                        v[6 + g] = l[g] / n;
                    }
                    series.push(v);
                }
                Plan::Spectrum {
                    tally,
                    n_cells,
                    inner,
                    bins,
                    series,
                    ..
                } => {
                    let b = x.batch(*tally);
                    let mut in_grid = 0.0;
                    for (e, bin) in bins.iter_mut().enumerate() {
                        let s: f64 = b[e * *n_cells..(e + 1) * *n_cells].iter().sum();
                        bin.score(s);
                        if inner.contains(&e) {
                            in_grid += s;
                        }
                    }
                    series.push(in_grid / n);
                }
                Plan::Kinetics { series, .. } => {
                    series.push(vec![
                        t.ifp.beta_numerator / n,
                        t.ifp.time_numerator / n,
                        t.ifp.denominator / n,
                        k,
                    ]);
                }
                Plan::Map {
                    fission,
                    tally,
                    groups,
                    total,
                    ..
                } => {
                    if !*fission {
                        let b = x.batch(*tally);
                        let g = groups.len() - 1;
                        for (c, bin) in total.iter_mut().enumerate() {
                            bin.score(b[c * g..(c + 1) * g].iter().sum());
                        }
                    }
                }
            }
        }
        self.extra.flush();
    }

    /// Assemble every result.
    pub(crate) fn finish(
        self,
        ctx: &RunContext,
    ) -> Result<Vec<(MCDiagnostic, MCDiagnosticResult)>, DiagnosticError> {
        let n_act = self.active;
        let per = self.population.max(1.0);
        let x = &self.extra;
        let mut out = Vec::with_capacity(self.plans.len());
        for (d, plan) in self.items.iter().zip(self.plans) {
            let r = match plan {
                Plan::RunHealth => {
                    let h = &self.health;
                    MCDiagnosticResult::RunHealth(RunHealth {
                        k_by_cycle: ctx.k_by_cycle.clone(),
                        entropy_by_cycle: ctx.entropy_by_cycle.clone(),
                        particles_by_cycle: ctx.particles_by_cycle.clone(),
                        inactive_cycles: ctx.inactive_cycles,
                        converged: ctx.converged,
                        histories: h.histories,
                        collisions_per_history: h.collisions as f64 / (h.histories.max(1)) as f64,
                        lost_locate: h.lost_locate,
                        stuck_events: h.stuck_events,
                        leak_infinity: h.leak_infinity,
                        negative_distances: h.neg_dist,
                        transport_seconds: ctx.transport_seconds,
                        hardware: hardware_headline(),
                        threads: 1,
                    })
                }
                Plan::Balance {
                    by, rows, series, ..
                } => {
                    let nr = rows.len();
                    let col = |i: usize| -> Vec<f64> { series.iter().map(|g| g[i]).collect() };
                    let row = |label: String, base: usize| -> BalanceRow {
                        let pair: Vec<Vec<f64>> = series
                            .iter()
                            .map(|g| vec![g[base + 1], g[base + 2]])
                            .collect();
                        BalanceRow {
                            label,
                            flux: series_estimate(&col(base)),
                            absorption: series_estimate(&col(base + 1)),
                            fission: series_estimate(&col(base + 2)),
                            capture: delta_method(&pair, |m| m[0] - m[1]),
                            production: series_estimate(&col(base + 3)),
                        }
                    };
                    let rows_out: Vec<BalanceRow> = rows
                        .iter()
                        .enumerate()
                        .map(|(i, (l, _))| row(l.clone(), 4 * i))
                        .collect();
                    let t = 4 * nr;
                    let total = row("total".to_string(), t);
                    let closure_in: Vec<Vec<f64>> = series
                        .iter()
                        .map(|g| vec![g[t + 1], g[t + 4], g[t + 8]])
                        .collect();
                    MCDiagnosticResult::NeutronBalance(NeutronBalance {
                        by,
                        rows: rows_out,
                        total,
                        leakage: series_estimate(&col(t + 4)),
                        analog_fission: series_estimate(&col(t + 5)),
                        analog_capture: series_estimate(&col(t + 6)),
                        analog_leakage: series_estimate(&col(t + 7)),
                        ends_per_source: series_estimate(&col(t + 8)),
                        closure: delta_method(&closure_in, |m| (m[0] + m[1]) / m[2] - 1.0),
                    })
                }
                Plan::Ealf {
                    bound_ln,
                    series,
                    outside,
                    total,
                    ..
                } => MCDiagnosticResult::Ealf(Ealf {
                    ealf_ev: delta_method(&series, |m| (m[0] / m[1]).exp()),
                    discretisation_bound_ln: bound_ln,
                    fission_outside_grid: if total > 0.0 { outside / total } else { 0.0 },
                }),
                Plan::Leakage {
                    leak,
                    inner,
                    edges,
                    series,
                } => {
                    let bins = x.leak_bins(leak);
                    let per_bin: Vec<Estimate> = bins[inner.clone()]
                        .iter()
                        .map(|b| bin_estimate(b, n_act, per))
                        .collect();
                    let below: f64 = bins[..inner.start]
                        .iter()
                        .map(|b| b.mean(n_act) / per)
                        .sum();
                    let above: f64 = bins[inner.end..].iter().map(|b| b.mean(n_act) / per).sum();
                    let col: Vec<f64> = series.iter().map(|g| g[0]).collect();
                    MCDiagnosticResult::Leakage(Leakage {
                        per_source: series_estimate(&col),
                        fraction_of_ends: delta_method(&series, |m| m[0] / m[1]),
                        edges_ev: edges,
                        per_bin,
                        outside_grid: (below, above),
                    })
                }
                Plan::SixFactors { edges, series, .. } => {
                    let factor = |which: usize| {
                        delta_method(&series, move |m| {
                            let e = Estimate::exact;
                            let sf = assemble_six_factors(
                                [e(m[0]), e(m[1]), e(m[2])],
                                e(m[9]),
                                [e(m[3]), e(m[4]), e(m[5])],
                                [e(m[6]), e(m[7]), e(m[8])],
                                edges,
                            );
                            match which {
                                0 => sf.eta.mean,
                                1 => sf.f.mean,
                                2 => sf.p.mean,
                                3 => sf.epsilon.mean,
                                4 => sf.p_fnl.mean,
                                5 => sf.p_tnl.mean,
                                _ => sf.k_from_factors.mean,
                            }
                        })
                    };
                    let col = |i: usize| -> Estimate {
                        series_estimate(&series.iter().map(|g| g[i]).collect::<Vec<_>>())
                    };
                    let ptot: Vec<f64> = series.iter().map(|g| g[3] + g[4] + g[5]).collect();
                    let undefined_because = [
                        (0, "no thermal absorption"),
                        (9, "no thermal absorption in the fuel"),
                        (3, "no thermal production"),
                    ]
                    .iter()
                    .find(|(i, _)| col(*i).mean <= 0.0)
                    .map(|(_, why)| {
                        format!(
                            "{why} below {} eV: every factor divides by a thermal rate, so the six-factor \
                             split does not apply (a fast system); the group rates and P/(A+L) stand",
                            edges.0
                        )
                    });
                    let nan = Estimate {
                        mean: f64::NAN,
                        std: f64::NAN,
                    };
                    let fac = |i| {
                        if undefined_because.is_some() {
                            nan
                        } else {
                            factor(i)
                        }
                    };
                    MCDiagnosticResult::SixFactors(SixFactors {
                        eta: fac(0),
                        f: fac(1),
                        p: fac(2),
                        epsilon: fac(3),
                        p_fnl: fac(4),
                        p_tnl: fac(5),
                        k_from_factors: fac(6),
                        production_over_losses: delta_method(&series, |m| {
                            (m[3] + m[4] + m[5]) / (m[0] + m[1] + m[2] + m[6] + m[7] + m[8])
                        }),
                        undefined_because,
                        k_tracklength: series_estimate(&ptot),
                        absorption: [col(0), col(1), col(2)],
                        production: [col(3), col(4), col(5)],
                        leakage: [col(6), col(7), col(8)],
                        thermal_absorption_fuel: col(9),
                        edges_ev: edges,
                    })
                }
                Plan::Spectrum {
                    region,
                    label,
                    inner,
                    edges,
                    bins,
                    series,
                    ..
                } => {
                    let flux_all: Vec<Estimate> =
                        bins.iter().map(|b| bin_estimate(b, n_act, per)).collect();
                    let grid_edges = edges[inner.start..=inner.end].to_vec();
                    let flux: Vec<Estimate> = flux_all[inner.clone()].to_vec();
                    let total: f64 = flux.iter().map(|e| e.mean).sum();
                    let per_lethargy = flux
                        .iter()
                        .zip(grid_edges.windows(2))
                        .map(|(f, w)| {
                            let du = (w[1] / w[0]).ln();
                            if total > 0.0 && du > 0.0 && du.is_finite() {
                                Estimate {
                                    mean: f.mean / (du * total),
                                    std: f.std / (du * total),
                                }
                            } else {
                                Estimate::exact(0.0)
                            }
                        })
                        .collect();
                    let below: f64 = flux_all[..inner.start].iter().map(|e| e.mean).sum();
                    let above: f64 = flux_all[inner.end..].iter().map(|e| e.mean).sum();
                    MCDiagnosticResult::Spectrum(Spectrum {
                        region,
                        label,
                        edges_ev: grid_edges,
                        flux,
                        per_lethargy,
                        total_in_grid: series_estimate(&series),
                        outside_grid: (below, above),
                    })
                }
                Plan::Kinetics {
                    generations,
                    series,
                } => {
                    let scoring: Vec<Vec<f64>> =
                        series.into_iter().filter(|g| g[2] > 0.0).collect();
                    if scoring.len() < 2 {
                        return Err(DiagnosticError::NoValue(format!(
                            "IFP with {generations} generations of lineage scored in {} active cycles; \
                             it needs at least two (run more active cycles than {generations})",
                            scoring.len()
                        )));
                    }
                    MCDiagnosticResult::Kinetics(Kinetics {
                        beta_eff: delta_method(&scoring, |m| m[0] / m[2]),
                        generation_time_s: delta_method(&scoring, |m| m[1] / (m[2] * m[3])),
                        ifp_generations: generations,
                        scoring_cycles: scoring.len(),
                    })
                }
                Plan::Map {
                    fission,
                    tally,
                    mesh,
                    groups,
                    total,
                } => {
                    let t = x.tally(tally);
                    let ng = groups.len() - 1;
                    let w = mesh.width();
                    let vol = w[0] * w[1] * w[2];
                    let nc = mesh.n_bins();
                    let mut values: Vec<Vec<Estimate>> = (0..ng)
                        .map(|g| {
                            (0..nc)
                                .map(|c| bin_estimate(&t.bins[c * ng + g], n_act, per * vol))
                                .collect()
                        })
                        .collect();
                    if !fission && ng > 1 {
                        values.push(
                            total
                                .iter()
                                .map(|b| bin_estimate(b, n_act, per * vol))
                                .collect(),
                        );
                    }
                    let m = MeshMap {
                        mesh,
                        cell_cm: w,
                        quantity: if fission {
                            "fission rate per source neutron per cm3".into()
                        } else {
                            "flux per source neutron [cm/cm3]".into()
                        },
                        group_edges_ev: groups,
                        values,
                    };
                    if fission {
                        MCDiagnosticResult::FissionMap(m)
                    } else {
                        MCDiagnosticResult::FluxMap(m)
                    }
                }
            };
            out.push((d.clone(), r));
        }
        Ok(out)
    }
}

/// The machine, for run health: `perf_report::HardwareInfo::headline` on
/// native targets. On wasm32 nothing is probed (the GPU probe goes through
/// JavaScript, which a WASI host does not have, and a page should not be
/// probed as a side effect of a run): it says so instead.
fn hardware_headline() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    {
        crate::perf_report::HardwareInfo::detect().headline()
    }
    #[cfg(target_arch = "wasm32")]
    {
        "wasm32: hardware not detected (write it in by hand with the timing)".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The delta method reproduces the textbook ratio-estimator variance,
    /// and a series' standard error is `s/√n`.
    #[test]
    fn delta_method_matches_the_ratio_variance() {
        let x: Vec<Vec<f64>> = (0..50)
            .map(|i| {
                let a = 2.0 + 0.1 * ((i * 7 % 11) as f64 - 5.0);
                let b = 4.0 + 0.05 * ((i * 3 % 7) as f64 - 3.0) + 0.3 * (a - 2.0);
                vec![a, b]
            })
            .collect();
        let r = delta_method(&x, |m| m[0] / m[1]);
        let n = x.len() as f64;
        let ma = x.iter().map(|g| g[0]).sum::<f64>() / n;
        let mb = x.iter().map(|g| g[1]).sum::<f64>() / n;
        let q = ma / mb;
        let var = x.iter().map(|g| (g[0] - q * g[1]).powi(2)).sum::<f64>() / (n - 1.0) / (mb * mb);
        assert!((r.mean - q).abs() < 1e-15);
        assert!(
            (r.std - (var / n).sqrt()).abs() < 1e-6 * r.std,
            "{} vs {}",
            r.std,
            (var / n).sqrt()
        );
        let s = series_estimate(&[1.0, 2.0, 3.0]);
        assert_eq!(s.mean, 2.0);
        assert!((s.std - (1.0f64 / 3.0).sqrt()).abs() < 1e-15);
        assert_eq!(series_estimate(&[]).mean, 0.0);
        assert!(delta_method(&[], |m: &[f64]| m.len() as f64).mean.is_nan());
        let (p, r) = padded(&[1.0, 2.0, 3.0]);
        assert_eq!(p, vec![0.0, 1.0, 2.0, 3.0, f64::MAX]);
        assert_eq!(r, 1..3);
        let (p, r) = padded(&[0.0, f64::MAX]);
        assert_eq!(p.len(), 2);
        assert_eq!(r, 0..1);
    }
}
