//! Measure what a **thinned temperature grid** costs in accuracy.
//!
//! A `tsl-*` thermal-scattering evaluation tabulates `S(E)` (coherent elastic)
//! and `S(α,β)` (incoherent inelastic) at several temperatures — ten of them
//! (296, 400, 500, 600, 700, 800, 1000, 1200, 1600, 2000 K) for the ENDF/B-VIII.0
//! graphites — and the inelastic tables are ~99.6 % of the tape. A low-fidelity
//! data option is therefore to **keep only a few tabulated temperatures and
//! interpolate between them**, trading accuracy for bytes.
//!
//! This module is the instrument that measures that trade rather than guessing
//! it. The measurement is exact because the evaluation is its own oracle: drop a
//! tabulated temperature from the grid, interpolate to it from the temperatures
//! that were kept, and compare against the values the evaluation actually
//! tabulates there.
//!
//! ```text
//!   full grid:      296  400  500  600  700  800  1000  1200  1600  2000
//!   kept (example): 296            600                  1200        2000
//!   withheld:            400  500       700  800  1000        1600
//!                         ^ interpolate here, compare against the tabulated row
//! ```
//!
//! ## What this module is not
//!
//! It is a **study tool, not part of the production data path.** Nothing here
//! changes how [`super::mf7`] resolves a requested temperature; the production
//! path always interpolates between *adjacent tabulated* temperatures with the
//! evaluation's own `LI` law. The `li_override` arguments below exist only so a
//! study can ask "would a different interpolation law have done better?" — the
//! answer is a reportable finding, not a licence to change the evaluation's
//! stated law.
//!
//! ## Two other uses of the same machinery
//!
//! - **Leave-one-out on the *full* grid** (drop one interior temperature, keep
//!   its immediate neighbours) characterises the accuracy of the *existing*
//!   production interpolation, not just of a thinned library.
//! - **Interpolation-law comparison** — pass `li_override = Some(4)` (log-lin,
//!   `ln S` linear in `T`) against the elastic channel's stated `LI = 2`
//!   (lin-lin) to test the physical prior that Debye-Waller suppression is
//!   roughly exponential in temperature.
//!
//! Units: temperatures in K, energies in eV, `α`/`β` dimensionless, `S` per the
//! ENDF convention. Relative errors are dimensionless fractions (not per cent).

use super::mf7::{interp_s_temperature, CoherentElastic, IncoherentInelastic};
use crate::endf::tape::Tape;
use crate::units::NeutronEnergy;
use crate::NjoyError;
use uom::si::energy::electronvolt;

/// Where the largest relative error of a comparison sits.
///
/// The two channels live on different grids, so the locator is an enum rather
/// than an untyped coordinate pair (workspace rule: enums, not trait objects).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WorstPoint {
    /// Coherent-elastic channel: the Bragg-edge energy at which `S(E)` (and
    /// hence `σ = S/E`, since the energy is the same on both sides) is worst.
    BraggEdge {
        /// Incident energy of the Bragg edge.
        energy: NeutronEnergy,
    },
    /// Incoherent-inelastic channel: the `(α, β)` grid point at which `S(α,β)`
    /// is worst. Both are dimensionless (and, for `LAT = 1`, scaled to
    /// 0.0253 eV).
    AlphaBeta {
        /// Momentum-transfer variable `α`.
        alpha: f64,
        /// Energy-transfer variable `β`.
        beta: f64,
    },
    /// Cross-section comparison: the incident energy at which `σ(E)` is worst.
    IncidentEnergy {
        /// Incident neutron energy.
        energy: NeutronEnergy,
    },
    /// Nothing was compared (every reference value was below the floor).
    Nothing,
}

/// Max / RMS relative error of an approximation against a reference, plus the
/// location of the worst point.
///
/// "Relative error" is `|approx − reference| / |reference|`, accumulated only
/// over points whose reference magnitude clears the accumulator's floor; the
/// number skipped is reported so a small `n_compared` cannot hide behind a
/// flattering `max_rel`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RelativeErrorStats {
    /// Largest relative error over the compared points (dimensionless
    /// fraction; multiply by 100 for per cent).
    pub max_rel: f64,
    /// Root-mean-square relative error over the compared points.
    pub rms_rel: f64,
    /// Largest *absolute* difference `|approx − reference|`, over **all**
    /// points including those below the floor. Units are the compared
    /// quantity's own (ENDF `S`, or barn for a cross-section comparison).
    pub max_abs: f64,
    /// Number of points that entered the relative statistics.
    pub n_compared: usize,
    /// Number of points skipped because `|reference|` was below the floor
    /// (including exact zeros), where a relative error is meaningless.
    pub n_skipped: usize,
    /// Where [`max_rel`](Self::max_rel) occurs.
    pub worst: WorstPoint,
    /// Reference value at the worst point.
    pub worst_reference: f64,
    /// Approximate (interpolated) value at the worst point.
    pub worst_approx: f64,
}

/// Accumulates relative-error statistics point by point.
///
/// Construct with [`new`](Self::new) (compare every nonzero reference) or
/// [`with_floor`](Self::with_floor) (ignore reference magnitudes below a
/// threshold, for grids like `S(α,β)` whose far corners hold physically
/// irrelevant values many decades below the peak).
#[derive(Debug, Clone)]
pub struct ErrorAccumulator {
    floor: f64,
    sum_sq: f64,
    max_rel: f64,
    max_abs: f64,
    n_compared: usize,
    n_skipped: usize,
    worst: WorstPoint,
    worst_reference: f64,
    worst_approx: f64,
}

impl ErrorAccumulator {
    /// An accumulator that compares every point with a strictly positive
    /// reference magnitude.
    pub fn new() -> Self {
        Self::with_floor(0.0)
    }

    /// An accumulator that ignores points whose reference magnitude is `<=
    /// floor` (they are counted in [`RelativeErrorStats::n_skipped`] and still
    /// contribute to [`RelativeErrorStats::max_abs`]).
    pub fn with_floor(floor: f64) -> Self {
        Self {
            floor,
            sum_sq: 0.0,
            max_rel: 0.0,
            max_abs: 0.0,
            n_compared: 0,
            n_skipped: 0,
            worst: WorstPoint::Nothing,
            worst_reference: 0.0,
            worst_approx: 0.0,
        }
    }

    /// Add one comparison: `reference` is the withheld tabulated value,
    /// `approx` the value interpolated from the kept temperatures, and `at`
    /// locates the point for reporting.
    pub fn push(&mut self, reference: f64, approx: f64, at: WorstPoint) {
        let abs = (approx - reference).abs();
        if abs > self.max_abs {
            self.max_abs = abs;
        }
        if !(reference.abs() > self.floor) {
            self.n_skipped += 1;
            return;
        }
        let rel = abs / reference.abs();
        self.sum_sq += rel * rel;
        self.n_compared += 1;
        if rel > self.max_rel {
            self.max_rel = rel;
            self.worst = at;
            self.worst_reference = reference;
            self.worst_approx = approx;
        }
    }

    /// Finish and return the statistics. `rms_rel` is 0 when nothing was
    /// compared.
    pub fn finish(self) -> RelativeErrorStats {
        let rms_rel = if self.n_compared == 0 {
            0.0
        } else {
            (self.sum_sq / self.n_compared as f64).sqrt()
        };
        RelativeErrorStats {
            max_rel: self.max_rel,
            rms_rel,
            max_abs: self.max_abs,
            n_compared: self.n_compared,
            n_skipped: self.n_skipped,
            worst: self.worst,
            worst_reference: self.worst_reference,
            worst_approx: self.worst_approx,
        }
    }
}

impl Default for ErrorAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

/// A subset of an evaluation's tabulated temperatures that a thinned library
/// would keep, held as indices into the full ascending temperature grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThinnedTemperatureGrid {
    kept: Vec<usize>,
}

impl ThinnedTemperatureGrid {
    /// Build a thinned grid by naming the temperatures \[K\] to keep, matched
    /// against the evaluation's ascending tabulated grid `all_k` \[K\] within
    /// 0.5 K.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] if a requested temperature is not tabulated, or
    /// if fewer than two are kept (one temperature cannot bracket anything).
    pub fn from_kept_temperatures(all_k: &[f64], keep_k: &[f64]) -> Result<Self, NjoyError> {
        let mut kept = Vec::with_capacity(keep_k.len());
        for &t in keep_k {
            let j = all_k
                .iter()
                .position(|&a| (a - t).abs() < 0.5)
                .ok_or_else(|| {
                    NjoyError::EndfParse(format!(
                        "thinned grid asks for {t} K, which is not tabulated in {all_k:?}"
                    ))
                })?;
            kept.push(j);
        }
        kept.sort_unstable();
        kept.dedup();
        if kept.len() < 2 {
            return Err(NjoyError::EndfParse(
                "a thinned temperature grid needs at least two kept temperatures".into(),
            ));
        }
        Ok(Self { kept })
    }

    /// Build a leave-one-out grid: every tabulated index except `dropped`.
    ///
    /// Used to characterise the accuracy of the *existing* production
    /// interpolation, which always works from immediately adjacent tabulated
    /// temperatures.
    ///
    /// # Errors
    /// [`NjoyError::EndfParse`] if `dropped` is an end point (nothing brackets
    /// it) or the grid has fewer than three temperatures.
    pub fn leave_one_out(n_temperatures: usize, dropped: usize) -> Result<Self, NjoyError> {
        if n_temperatures < 3 || dropped == 0 || dropped + 1 >= n_temperatures {
            return Err(NjoyError::EndfParse(format!(
                "leave-one-out needs an interior index: dropped {dropped} of {n_temperatures}"
            )));
        }
        Ok(Self {
            kept: (0..n_temperatures).filter(|&j| j != dropped).collect(),
        })
    }

    /// The kept indices into the full tabulated grid, ascending.
    pub fn kept_indices(&self) -> &[usize] {
        &self.kept
    }

    /// How many temperatures this grid keeps.
    pub fn len(&self) -> usize {
        self.kept.len()
    }

    /// Whether the grid keeps no temperatures (never true for a grid built by
    /// the constructors, which require at least two).
    pub fn is_empty(&self) -> bool {
        self.kept.is_empty()
    }

    /// Whether tabulated index `j` survived the thinning.
    pub fn is_kept(&self, j: usize) -> bool {
        self.kept.binary_search(&j).is_ok()
    }

    /// The withheld tabulated indices, ascending — the points at which an
    /// error can be measured.
    pub fn withheld_indices(&self, n_temperatures: usize) -> Vec<usize> {
        (0..n_temperatures).filter(|&j| !self.is_kept(j)).collect()
    }

    /// The two kept indices that bracket withheld index `j`, or `None` if `j`
    /// is itself kept or lies outside the kept range (which would require
    /// extrapolation — refused, matching the production temperature policy).
    pub fn bracket(&self, j: usize) -> Option<(usize, usize)> {
        if self.is_kept(j) {
            return None;
        }
        let hi_pos = self.kept.partition_point(|&k| k < j);
        if hi_pos == 0 || hi_pos >= self.kept.len() {
            return None;
        }
        Some((self.kept[hi_pos - 1], self.kept[hi_pos]))
    }
}

/// The `S(E)` row a thinned grid would reconstruct at withheld tabulated index
/// `withheld`, as a single-temperature [`CoherentElastic`] so the production
/// [`CoherentElastic::cross_section`] evaluates it unchanged.
///
/// `li_override` replaces the evaluation's own `LI` law for the study
/// (`Some(2)` lin-lin, `Some(4)` log-lin); `None` uses the law the evaluation
/// states for the bracketing interval. Returns `None` when `withheld` is kept
/// or cannot be bracketed.
pub fn coherent_elastic_thinned_row(
    ce: &CoherentElastic,
    grid: &ThinnedTemperatureGrid,
    withheld: usize,
    li_override: Option<u32>,
) -> Option<CoherentElastic> {
    let (lo, hi) = grid.bracket(withheld)?;
    let target_k = *ce.temperatures_k.get(withheld)?;
    let (t_lo, t_hi) = (ce.temperatures_k[lo], ce.temperatures_k[hi]);
    // The evaluation's law for the interval that actually spans the gap.
    let li = li_override.unwrap_or_else(|| ce.temp_interp.get(lo).copied().unwrap_or(2));
    let row: Vec<f64> = ce.s_tables[lo]
        .iter()
        .zip(ce.s_tables[hi].iter())
        .map(|(&s_lo, &s_hi)| interp_s_temperature(t_lo, s_lo, t_hi, s_hi, target_k, li))
        .collect();
    Some(CoherentElastic {
        bragg_energies_ev: ce.bragg_energies_ev.clone(),
        temperatures_k: vec![target_k],
        s_tables: vec![row],
        temp_interp: Vec::new(),
    })
}

/// Relative error of a thinned grid's reconstructed `S(E)` against the withheld
/// tabulated row, over every Bragg edge.
///
/// Because `σ_coh(E,T) = S(E,T)/E` and the Bragg-edge energies are
/// temperature-independent, the relative error in `S` at an edge **is** the
/// relative error in the coherent-elastic cross section at that energy.
///
/// Returns `None` when `withheld` is kept or cannot be bracketed.
pub fn coherent_elastic_thinning_error(
    ce: &CoherentElastic,
    grid: &ThinnedTemperatureGrid,
    withheld: usize,
    li_override: Option<u32>,
) -> Option<RelativeErrorStats> {
    coherent_elastic_thinning_error_below(ce, grid, withheld, li_override, f64::INFINITY)
}

/// As [`coherent_elastic_thinning_error`], but restricted to Bragg edges at or
/// below `e_max_ev` \[eV\].
///
/// The graphite coherent-elastic table runs to 5 eV, where `σ_coh = S/E` is
/// two orders of magnitude below its thermal value and the channel is
/// irrelevant to a thermal-reactor calculation. Restricting to (say)
/// 0.0253 eV or 1 eV answers "how wrong is it *where it matters*", which is a
/// different — and for HTR-10 the decisive — question from the whole-table
/// figure.
///
/// Returns `None` when `withheld` is kept or cannot be bracketed.
pub fn coherent_elastic_thinning_error_below(
    ce: &CoherentElastic,
    grid: &ThinnedTemperatureGrid,
    withheld: usize,
    li_override: Option<u32>,
    e_max_ev: f64,
) -> Option<RelativeErrorStats> {
    let approx = coherent_elastic_thinned_row(ce, grid, withheld, li_override)?;
    let mut acc = ErrorAccumulator::new();
    for (i, &e_ev) in ce.bragg_energies_ev.iter().enumerate() {
        if e_ev > e_max_ev {
            break;
        }
        acc.push(
            ce.s_tables[withheld][i],
            approx.s_tables[0][i],
            WorstPoint::BraggEdge {
                energy: NeutronEnergy::new::<electronvolt>(e_ev),
            },
        );
    }
    Some(acc.finish())
}

/// How a temperature interpolation treats an `S(α,β)` cell that is **exactly
/// zero** at one of the two bracketing temperatures.
///
/// This is a diagnostic knob, not a production option. It exists because the
/// choice turns out to dominate the inelastic error at high incident energy:
/// a floored (`S = 0`) cell makes [`IncoherentInelastic::double_differential`]
/// fall through to the short-collision-time kernel, which is what carries
/// `σ_inel(E)` to the free-atom limit; a small *nonzero* value in the same cell
/// keeps it on the tabulated branch and contributes almost nothing instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZeroPolicy {
    /// What the production reader does: [`interp_s_temperature`]'s log-law
    /// domain error at a zero endpoint degrades to lin-lin, so the
    /// interpolated cell is small but nonzero.
    AsProduction,
    /// Diagnostic: a cell zero at *either* bracketing temperature stays zero,
    /// preserving the floor that triggers the SCT fall-through.
    PreserveZeros,
}

/// Every tabulated temperature's incoherent-inelastic kernel for one material —
/// the input a thinning study needs, since [`super::mf7::parse_mf7_at_temperature`]
/// retains only one temperature at a time.
///
/// Built by parsing the MF=7/MT=4 section once per tabulated temperature, so
/// each kernel comes from the verified production reader rather than a
/// second implementation. For the ENDF/B-VIII.0 graphites this is ten kernels
/// of 400 `β` × 150 `α`, roughly 10 MB resident.
#[derive(Debug, Clone)]
pub struct SabTemperatureStack {
    /// One kernel per tabulated temperature, ascending. `kernels[j]` holds the
    /// evaluation's own `S(α,β)` at `temperatures_k[j]`.
    pub kernels: Vec<IncoherentInelastic>,
    /// The tabulated temperatures \[K\], ascending.
    pub temperatures_k: Vec<f64>,
    /// ENDF `LI` codes: `temp_interp[j]` governs the interval
    /// `[temperatures_k[j], temperatures_k[j+1]]` (`4` = log-lin for the
    /// ENDF/B-VIII.0 graphite inelastic).
    pub temp_interp: Vec<u32>,
}

impl SabTemperatureStack {
    /// Parse `mat`'s MF=7/MT=4 section at every tabulated temperature.
    ///
    /// # Errors
    /// Whatever [`super::mf7::parse_mf7_at_temperature`] returns, plus
    /// [`NjoyError::SectionNotFound`] if the material has no MT=4.
    pub fn from_tape(tape: &Tape, mat: i32) -> Result<Self, NjoyError> {
        let base = super::mf7::parse_mf7(tape, mat)?
            .incoherent_inelastic
            .ok_or(NjoyError::SectionNotFound { mat, mf: 7, mt: 4 })?;
        let temperatures_k = base.tabulated_temperatures_k.clone();
        let temp_interp = base.temp_interp.clone();
        let mut kernels = Vec::with_capacity(temperatures_k.len());
        for (j, &t) in temperatures_k.iter().enumerate() {
            if j == 0 {
                kernels.push(base.clone());
            } else {
                kernels.push(
                    super::mf7::parse_mf7_at_temperature(tape, mat, Some(t))?
                        .incoherent_inelastic
                        .ok_or(NjoyError::SectionNotFound { mat, mf: 7, mt: 4 })?,
                );
            }
        }
        Ok(Self {
            kernels,
            temperatures_k,
            temp_interp,
        })
    }

    /// The kernel a thinned grid would reconstruct at withheld tabulated index
    /// `withheld`: the withheld kernel's own `α`/`β` grid and constants, with
    /// `S(α,β)` interpolated point-by-point between the bracketing **kept**
    /// temperatures.
    ///
    /// `li_override` replaces the evaluation's law for the study; `None` uses
    /// the stated law. Returns `None` when `withheld` is kept or unbracketed.
    pub fn thinned_kernel(
        &self,
        grid: &ThinnedTemperatureGrid,
        withheld: usize,
        li_override: Option<u32>,
    ) -> Option<IncoherentInelastic> {
        self.thinned_kernel_with_zero_policy(grid, withheld, li_override, ZeroPolicy::AsProduction)
    }

    /// As [`thinned_kernel`](Self::thinned_kernel), with explicit control over
    /// zero-endpoint cells — see [`ZeroPolicy`].
    pub fn thinned_kernel_with_zero_policy(
        &self,
        grid: &ThinnedTemperatureGrid,
        withheld: usize,
        li_override: Option<u32>,
        zeros: ZeroPolicy,
    ) -> Option<IncoherentInelastic> {
        let (lo, hi) = grid.bracket(withheld)?;
        let target_k = *self.temperatures_k.get(withheld)?;
        let (t_lo, t_hi) = (self.temperatures_k[lo], self.temperatures_k[hi]);
        let li = li_override.unwrap_or_else(|| self.temp_interp.get(lo).copied().unwrap_or(2));
        let mut out = self.kernels[withheld].clone();
        for (ib, table) in out.s_tables.iter_mut().enumerate() {
            let s_lo = &self.kernels[lo].s_tables[ib].s;
            let s_hi = &self.kernels[hi].s_tables[ib].s;
            for (ia, s) in table.s.iter_mut().enumerate() {
                *s = if zeros == ZeroPolicy::PreserveZeros && (s_lo[ia] == 0.0 || s_hi[ia] == 0.0) {
                    0.0
                } else {
                    interp_s_temperature(t_lo, s_lo[ia], t_hi, s_hi[ia], target_k, li)
                };
            }
        }
        out.temperature_k = target_k;
        Some(out)
    }

    /// How many `S(α,β)` cells are exactly zero (floored by LEAPR) at tabulated
    /// index `j`, out of how many total — the population that decides whether
    /// [`ZeroPolicy`] matters for this evaluation.
    pub fn zero_cell_count(&self, j: usize) -> (usize, usize) {
        let k = &self.kernels[j];
        let total: usize = k.s_tables.iter().map(|t| t.s.len()).sum();
        let zeros = k
            .s_tables
            .iter()
            .flat_map(|t| t.s.iter())
            .filter(|&&s| s == 0.0)
            .count();
        (zeros, total)
    }

    /// Relative error of a thinned grid's reconstructed `S(α,β)` against the
    /// withheld tabulated tables, over the whole `(α,β)` grid.
    ///
    /// `floor` skips reference `S` values at or below it — the far corners of
    /// the grid hold values tens of decades below the peak, where a large
    /// relative error carries no physical weight. Pass `0.0` for the
    /// unfiltered figure; pass `1e-6 × S_max` for the physically significant
    /// subset. Report both: they answer different questions.
    ///
    /// Returns `None` when `withheld` is kept or unbracketed.
    pub fn thinning_error(
        &self,
        grid: &ThinnedTemperatureGrid,
        withheld: usize,
        li_override: Option<u32>,
        floor: f64,
    ) -> Option<RelativeErrorStats> {
        let approx = self.thinned_kernel(grid, withheld, li_override)?;
        let reference = &self.kernels[withheld];
        let mut acc = ErrorAccumulator::with_floor(floor);
        for (ib, table) in reference.s_tables.iter().enumerate() {
            for (ia, &s_ref) in table.s.iter().enumerate() {
                acc.push(
                    s_ref,
                    approx.s_tables[ib].s[ia],
                    WorstPoint::AlphaBeta {
                        alpha: table.alpha[ia],
                        beta: table.beta,
                    },
                );
            }
        }
        Some(acc.finish())
    }

    /// The largest tabulated `S(α,β)` over every temperature — the natural
    /// scale for choosing a [`thinning_error`](Self::thinning_error) floor.
    pub fn max_s(&self) -> f64 {
        self.kernels
            .iter()
            .flat_map(|k| k.s_tables.iter())
            .flat_map(|t| t.s.iter())
            .fold(0.0f64, |m, &s| m.max(s))
    }
}

/// Relative error in the **integrated** inelastic cross section `σ_inel(E)`
/// that a thinned grid would produce at a withheld tabulated temperature.
///
/// This is the physics-weighted answer: an error in `S(α,β)` only matters in
/// proportion to that point's contribution to the cross section, and this
/// comparison does the weighting by running both the reference and the
/// reconstructed tables through the same production kernel
/// ([`IncoherentInelastic::cross_section`]) at the same physical temperature.
///
/// `energies` are the incident energies \[eV\] to test; `natom` is the number
/// of principal atoms (1 for graphite); `zeros` selects the zero-endpoint
/// handling ([`ZeroPolicy::AsProduction`] for the real cost of thinning).
/// Returns `None` when `withheld` is kept or unbracketed.
pub fn inelastic_cross_section_thinning_error(
    stack: &SabTemperatureStack,
    grid: &ThinnedTemperatureGrid,
    withheld: usize,
    li_override: Option<u32>,
    energies: &[f64],
    natom: f64,
    zeros: ZeroPolicy,
) -> Option<RelativeErrorStats> {
    let approx = stack.thinned_kernel_with_zero_policy(grid, withheld, li_override, zeros)?;
    let reference = &stack.kernels[withheld];
    let temp_k = stack.temperatures_k[withheld];
    let mut acc = ErrorAccumulator::new();
    for &e in energies {
        acc.push(
            reference.cross_section(e, temp_k, natom),
            approx.cross_section(e, temp_k, natom),
            WorstPoint::IncidentEnergy {
                energy: NeutronEnergy::new::<electronvolt>(e),
            },
        );
    }
    Some(acc.finish())
}

/// Bytes an ENDF-6 text record occupies on a `tsl-*` tape: 75 columns of
/// content plus one newline.
///
/// Verified against the three ENDF/B-VIII.0 graphite tapes on 2026-08-13
/// (8 730 804 B / 114 879 lines = 76.000 B/line exactly).
pub const ENDF_BYTES_PER_RECORD: usize = 76;

/// Size, in ENDF text bytes, of an MF=7/MT=4 section holding `n_temperatures`
/// of an `n_beta` × `n_alpha` `S(α,β)` table.
///
/// Record model (ENDF-102 §7.4, confirmed against the graphite tapes):
/// a HEAD, the `B` LIST (head + one data record), a TAB2 (2 records), then per
/// `β` a TAB1 (2 head records + `ceil(2·n_alpha/6)` interleaved `(α,S)` data
/// records, which carries the shared `α` grid *and* the base temperature's
/// `S`) followed by `n_temperatures − 1` LISTs (1 head record +
/// `ceil(n_alpha/6)` `S`-only data records), then the effective-temperature
/// TAB1 (2 head records + `ceil(2·n_temperatures/6)` data records).
///
/// The base temperature is structural — it carries the `α` grid — so a thinned
/// grid is assumed to keep it.
pub fn mf7_mt4_endf_bytes(n_temperatures: usize, n_beta: usize, n_alpha: usize) -> usize {
    let per_extra = 1 + n_alpha.div_ceil(6);
    let per_beta = 2 + (2 * n_alpha).div_ceil(6) + n_temperatures.saturating_sub(1) * per_extra;
    let teff = 2 + (2 * n_temperatures).div_ceil(6);
    let records = 1 + 2 + 2 + n_beta * per_beta + teff;
    records * ENDF_BYTES_PER_RECORD
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bracket_finds_the_kept_neighbours() {
        let all = [
            296.0, 400.0, 500.0, 600.0, 700.0, 800.0, 1000.0, 1200.0, 1600.0, 2000.0,
        ];
        let grid =
            ThinnedTemperatureGrid::from_kept_temperatures(&all, &[296.0, 600.0, 1200.0, 2000.0])
                .unwrap();
        assert_eq!(grid.kept_indices(), &[0, 3, 7, 9]);
        assert_eq!(grid.withheld_indices(10), vec![1, 2, 4, 5, 6, 8]);
        assert_eq!(grid.bracket(1), Some((0, 3))); // 400 K from 296/600
        assert_eq!(grid.bracket(2), Some((0, 3))); // 500 K from 296/600
        assert_eq!(grid.bracket(6), Some((3, 7))); // 1000 K from 600/1200
        assert_eq!(grid.bracket(8), Some((7, 9))); // 1600 K from 1200/2000
        assert_eq!(grid.bracket(3), None, "600 K is kept");
    }

    #[test]
    fn leave_one_out_keeps_the_neighbours() {
        let grid = ThinnedTemperatureGrid::leave_one_out(10, 2).unwrap();
        assert_eq!(grid.bracket(2), Some((1, 3)));
        assert!(ThinnedTemperatureGrid::leave_one_out(10, 0).is_err());
        assert!(ThinnedTemperatureGrid::leave_one_out(10, 9).is_err());
    }

    #[test]
    fn accumulator_reports_max_rms_and_worst_point() {
        let mut acc = ErrorAccumulator::new();
        acc.push(
            1.0,
            1.1,
            WorstPoint::BraggEdge {
                energy: NeutronEnergy::new::<electronvolt>(0.01),
            },
        );
        acc.push(
            2.0,
            2.0,
            WorstPoint::BraggEdge {
                energy: NeutronEnergy::new::<electronvolt>(0.02),
            },
        );
        acc.push(0.0, 1e-30, WorstPoint::Nothing); // zero reference → skipped
        let s = acc.finish();
        assert_eq!(s.n_compared, 2);
        assert_eq!(s.n_skipped, 1);
        assert!((s.max_rel - 0.1).abs() < 1e-12);
        assert!((s.rms_rel - (0.01f64 / 2.0).sqrt()).abs() < 1e-12);
        assert!((s.max_abs - 0.1).abs() < 1e-12);
        match s.worst {
            WorstPoint::BraggEdge { energy } => {
                assert!((energy.get::<electronvolt>() - 0.01).abs() < 1e-15)
            }
            other => panic!("unexpected worst point {other:?}"),
        }
    }

    /// The byte model must reproduce the real ENDF/B-VIII.0 graphite MT=4
    /// section exactly: 400 β × 150 α × 10 temperatures = 114 411 records
    /// (measured from the tape on 2026-08-13) = 8 695 236 B.
    #[test]
    fn byte_model_matches_the_graphite_tape() {
        assert_eq!(mf7_mt4_endf_bytes(10, 400, 150), 114_411 * 76);
    }
}
