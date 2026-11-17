// SPDX-License-Identifier: AGPL-3.0-only
//! **Source biasing driven by the learned importance map** (GitHub #1615,
//! maintainer direction on #497, 2026-11-17: "connect source biasing with
//! importance, let it be an option").
//!
//! # OFF by default
//!
//! Like [`super::learned_importance`], this is an **estimator**, not physics:
//! a biased source samples births from `q` and gives each the weight `p/q`, so
//! every expected score is the analog one. Nothing here runs unless a caller
//! passes a source and a set of weight windows to [`with_learned_source_bias`]
//! (or the lower-level builders) **and** hands the returned source to
//! `run_fixed_source`. No driver calls this module; `FixedSource` and
//! `VarianceReduction::default()` stay analog.
//!
//! # The importance used, and why (CADIS-style)
//!
//! A weight window is centred where `weight x importance` is constant, so the
//! importance a window set encodes is `I = 1 / lower` (MAGIC sets `lower`
//! proportional to the forward flux; the learned fill extends the same map).
//! CADIS biases the source with `q ∝ p · I`, so a birth in cell `c` carries
//! `w = p/q ∝ lower_c`: **every birth lands at the same position relative to
//! its own window** (`w / lower_c = 1 / Σ_k p_k/lower_k`, the same constant
//! for all cells), which is what "births consistent with the windows" means.
//! The windows themselves are used, not a second map, so the source and the
//! game cannot disagree, and a learned fill reaches the source too.
//!
//! - **Position.** The source's own spatial distribution `p` (a uniform box;
//!   a point is a single cell, so biasing it is the identity) is split over
//!   the window mesh cells it overlaps, `p_c` = overlap fraction. A cell is
//!   chosen with `q_c ∝ p_c I_c`, the position is uniform inside the overlap
//!   (`SpatialDistribution::Cells`, which reuses the ported OpenMC
//!   `DiscreteIndex` / `compute_importance_weights`), weight `p_c/q_c`.
//! - **Energy.** A mono-energetic source has one group and nothing to bias.
//!   For a **discrete spectrum** ([`learned_biased_energy`]) the line `E_i`
//!   is chosen with `q_i ∝ p_i · Ī_g(E_i)`, `Ī_g` the `p`-weighted mean
//!   importance of the group over the source cells, using the existing
//!   `Distribution::discrete_biased`. Energy and position are drawn
//!   independently, so this is the product-form bias; it cannot represent
//!   the joint (cell, group) importance, which is stated, not hidden.
//! - **A cell with no window** (`lower <= 0`) has no importance. It gets the
//!   `p`-weighted mean importance of the windowed source cells (neutral), and
//!   the report counts them. Any positive `q` is unbiased; this only affects
//!   variance. If no source cell has a window the call fails rather than
//!   guess.
//!
//! # Unbiasedness
//!
//! `E_q[w f] = Σ q_c (p_c/q_c) f_c = Σ p_c f_c` whenever `q_c > 0` where
//! `p_c > 0`; that holds because `I_c > 0`. **That is an argument, not a
//! measurement**: `tests/learned_source_bias_1615.rs` and the shielded-room
//! record in `verification_and_validation/stats_epic_493/497_learned_importance.md`
//! measure it.

use std::sync::Arc;

use crate::physics::fixed_source::FixedSource;
use crate::physics::weight_windows::WeightWindows;
use crate::source::angle::{AngleKind, IsotropicAngle};
use crate::source::distribution::{Distribution, SpatialDistribution};
use crate::source::energy::{EnergyKind, Monoenergetic};
use crate::source::source::IndependentSource;
use crate::source::spatial::SpatialKind;
use crate::tally::mesh::unflatten;

type V3 = [f64; 3];

/// What a biased source did to the births. All numbers describe the bias that
/// was built, not a transport result.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceBiasReport {
    /// Mesh cells the source overlaps (each a bias category).
    pub n_cells: usize,
    /// Of those, cells with no window (given the neutral importance).
    pub n_no_window: usize,
    /// Smallest and largest birth weight `p_c/q_c`.
    pub weight_min: f64,
    /// Largest birth weight `p_c/q_c`.
    pub weight_max: f64,
    /// Relative variance of the birth weight, `E_q[w²] − 1 = Σ p_c w_c − 1`
    /// (`E_q[w] = 1` exactly). Zero means the bias is the identity.
    pub weight_rel_variance: f64,
}

/// One mesh cell the source overlaps: its sub-box and the probability `p_c`
/// that the unbiased source is born there.
struct SourceCell {
    cell: usize,
    lower: V3,
    upper: V3,
    p: f64,
}

/// Fraction of `[lo, hi]` inside `[clo, chi)`; a zero-width source extent is
/// a point, inside or not.
fn overlap_fraction(lo: f64, hi: f64, clo: f64, chi: f64) -> f64 {
    if hi <= lo {
        return if lo >= clo && lo < chi { 1.0 } else { 0.0 };
    }
    (hi.min(chi) - lo.max(clo)).max(0.0) / (hi - lo)
}

/// The window-mesh cells a uniform box source overlaps, with `p_c`.
fn source_cells(ww: &WeightWindows, lower: V3, upper: V3) -> Result<Vec<SourceCell>, String> {
    let w = ww.mesh.width();
    let mut out = Vec::new();
    for c in 0..ww.mesh.n_bins() {
        let ijk = unflatten(c, ww.mesh.dimension);
        let mut lo = [0.0; 3];
        let mut hi = [0.0; 3];
        let mut p = 1.0;
        for a in 0..3 {
            let clo = ww.mesh.lower_left[a] + ijk[a] as f64 * w[a];
            let chi = clo + w[a];
            p *= overlap_fraction(lower[a], upper[a], clo, chi);
            lo[a] = lower[a].max(clo);
            hi[a] = upper[a].min(chi).max(lo[a]);
        }
        if p > 0.0 {
            out.push(SourceCell { cell: c, lower: lo, upper: hi, p });
        }
    }
    if out.is_empty() {
        return Err("the source box does not overlap the weight-window mesh".into());
    }
    Ok(out)
}

/// Energy group of `e`, or `None` outside the window's energy bounds.
fn group_of(ww: &WeightWindows, e: f64) -> Option<usize> {
    let b = &ww.energy_bounds;
    (0..b.len().saturating_sub(1)).find(|&g| e >= b[g] && e < b[g + 1])
}

/// Importance `1/lower` per source cell in group `g`; cells with no window get
/// the `p`-weighted mean importance of the windowed ones. Returns the
/// importances and the number of no-window cells.
fn cell_importance(ww: &WeightWindows, cells: &[SourceCell], g: usize) -> Result<(Vec<f64>, usize), String> {
    let n_m = ww.mesh.n_bins();
    let raw: Vec<Option<f64>> = cells
        .iter()
        .map(|c| {
            let l = ww.lower[g * n_m + c.cell];
            if l > 0.0 { Some(1.0 / l) } else { None }
        })
        .collect();
    fill_neutral(cells.iter().map(|c| c.p).collect::<Vec<_>>().as_slice(), &raw)
}

/// Replace `None` entries by the `p`-weighted mean of the `Some` ones.
fn fill_neutral(p: &[f64], raw: &[Option<f64>]) -> Result<(Vec<f64>, usize), String> {
    let (mut sp, mut spi) = (0.0, 0.0);
    for (pi, r) in p.iter().zip(raw) {
        if let Some(i) = r {
            sp += pi;
            spi += pi * i;
        }
    }
    if sp <= 0.0 {
        return Err("no source cell (or line) has a weight window: no importance to bias with".into());
    }
    let neutral = spi / sp;
    let n_none = raw.iter().filter(|r| r.is_none()).count();
    Ok((raw.iter().map(|r| r.unwrap_or(neutral)).collect(), n_none))
}

/// The bias bookkeeping for weights `w_c = p_c/q_c`.
fn report(p: &[f64], q: &[f64], n_no_window: usize) -> SourceBiasReport {
    let sp: f64 = p.iter().sum();
    let sq: f64 = q.iter().sum();
    let w: Vec<f64> = p.iter().zip(q).map(|(pi, qi)| (pi / sp) / (qi / sq)).collect();
    let second: f64 = p.iter().zip(&w).map(|(pi, wi)| (pi / sp) * wi).sum();
    SourceBiasReport {
        n_cells: p.len(),
        n_no_window,
        weight_min: w.iter().cloned().fold(f64::INFINITY, f64::min),
        weight_max: w.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        weight_rel_variance: second - 1.0,
    }
}

/// A uniform box source (probability `p`) re-sampled over the window mesh
/// cells it overlaps with `q_c ∝ p_c / lower_c` in group `group`, weight
/// `p_c/q_c`. See the module docs for why `1/lower`.
///
/// # Errors
///
/// `group` out of range, a box off the mesh, or no windowed source cell.
pub fn learned_biased_spatial(
    ww: &WeightWindows,
    lower: V3,
    upper: V3,
    group: usize,
) -> Result<(SpatialDistribution, SourceBiasReport), String> {
    if group >= ww.n_energy() {
        return Err(format!("group {group} out of range ({} groups)", ww.n_energy()));
    }
    let cells = source_cells(ww, lower, upper)?;
    let (imp, n_none) = cell_importance(ww, &cells, group)?;
    let p: Vec<f64> = cells.iter().map(|c| c.p).collect();
    let q: Vec<f64> = p.iter().zip(&imp).map(|(pi, ii)| pi * ii).collect();
    let boxes: Vec<(V3, V3)> = cells.iter().map(|c| (c.lower, c.upper)).collect();
    let rep = report(&p, &q, n_none);
    Ok((SpatialDistribution::cells_biased(boxes, &p, &q)?, rep))
}

/// A **discrete energy spectrum** (`energies[i]` with probability `p[i]`)
/// re-sampled with `q_i ∝ p_i · Ī_g(E_i)`, where `Ī_g` is the `p`-weighted
/// mean importance `1/lower` of `E_i`'s group over the cells the uniform box
/// source overlaps; weight `p_i/q_i`. A line outside the windows' energy range
/// gets the neutral importance.
///
/// # Errors
///
/// Mismatched lengths, a box off the mesh, or no windowed group.
pub fn learned_biased_energy(
    ww: &WeightWindows,
    lower: V3,
    upper: V3,
    energies: &[f64],
    p: &[f64],
) -> Result<(Distribution, SourceBiasReport), String> {
    if energies.len() != p.len() || energies.is_empty() {
        return Err("energies and probabilities must be non-empty and the same length".into());
    }
    let cells = source_cells(ww, lower, upper)?;
    let n_m = ww.mesh.n_bins();
    let group_importance = |g: usize| -> Option<f64> {
        let (mut sp, mut spi) = (0.0, 0.0);
        for c in &cells {
            let l = ww.lower[g * n_m + c.cell];
            if l > 0.0 {
                sp += c.p;
                spi += c.p / l;
            }
        }
        if sp > 0.0 { Some(spi / sp) } else { None }
    };
    let raw: Vec<Option<f64>> = energies
        .iter()
        .map(|&e| group_of(ww, e).and_then(group_importance))
        .collect();
    let (imp, n_none) = fill_neutral(p, &raw)?;
    let q: Vec<f64> = p.iter().zip(&imp).map(|(pi, ii)| pi * ii).collect();
    let rep = report(p, &q, n_none);
    Ok((Distribution::discrete_biased(energies, p, &q), rep))
}

/// **The opt-in entry point**: `source` with its births biased by the learned
/// importance in `ww`. Returns the biased source and what the bias did.
///
/// Supported inputs, each re-expressed as an
/// [`IndependentSource`](crate::source::source::IndependentSource) (so the
/// birth weight is carried into transport):
///
/// - [`FixedSource::Box`]: the box is split over the window cells.
/// - [`FixedSource::Point`]: a single cell, so the bias is the identity; the
///   source is returned **unchanged** with a one-cell report (`weight_rel_variance = 0`).
/// - [`FixedSource::Independent`] with a `Box`/`Point` spatial part and a
///   mono-energetic energy part: as `Box`/`Point`, keeping its angle and
///   strength.
///
/// The group is that of the source energy. Anything else is an `Err`, not a
/// silent analog source.
///
/// # Errors
///
/// An unsupported source, an energy outside the windows' range, or whatever
/// [`learned_biased_spatial`] rejects.
pub fn with_learned_source_bias(
    source: &FixedSource,
    ww: &WeightWindows,
) -> Result<(FixedSource, SourceBiasReport), String> {
    let (lower, upper, energy, angle, strength, point) = match source {
        FixedSource::Point { r, energy_ev } => {
            ([r.x, r.y, r.z], [r.x, r.y, r.z], *energy_ev, AngleKind::Isotropic(IsotropicAngle), 1.0, true)
        }
        FixedSource::Box { lower, upper, energy_ev } => (
            [lower.x, lower.y, lower.z],
            [upper.x, upper.y, upper.z],
            *energy_ev,
            AngleKind::Isotropic(IsotropicAngle),
            1.0,
            false,
        ),
        FixedSource::Independent(s) => {
            let EnergyKind::Mono(Monoenergetic { e }) = &s.energy else {
                return Err("only a mono-energetic Independent source is supported; \
                            use learned_biased_energy for a discrete spectrum"
                    .into());
            };
            match &s.spatial {
                SpatialKind::Box(b) => (
                    [b.lower_left.x, b.lower_left.y, b.lower_left.z],
                    [b.upper_right.x, b.upper_right.y, b.upper_right.z],
                    *e,
                    s.angle.clone(),
                    s.strength,
                    false,
                ),
                SpatialKind::Point(pt) => (
                    [pt.r.x, pt.r.y, pt.r.z],
                    [pt.r.x, pt.r.y, pt.r.z],
                    *e,
                    s.angle.clone(),
                    s.strength,
                    true,
                ),
                _ => return Err("only Box and Point spatial parts are supported".into()),
            }
        }
        FixedSource::Surface(_) => return Err("a surface-source replay is not biased".into()),
    };
    let group = group_of(ww, energy)
        .ok_or_else(|| format!("source energy {energy} eV is outside the weight windows' energy bounds"))?;
    if point {
        // One cell: q = p, the identity. Nothing to build, nothing to change.
        let cells = source_cells(ww, lower, upper)?;
        let (_, n_none) = cell_importance(ww, &cells, group)?;
        let rep = SourceBiasReport {
            n_cells: 1,
            n_no_window: n_none,
            weight_min: 1.0,
            weight_max: 1.0,
            weight_rel_variance: 0.0,
        };
        return Ok((source.clone(), rep));
    }
    let (spatial, rep) = learned_biased_spatial(ww, lower, upper, group)?;
    let biased = IndependentSource {
        spatial: SpatialKind::Distribution(spatial),
        energy: EnergyKind::Mono(Monoenergetic { e: energy }),
        angle,
        strength,
    };
    Ok((FixedSource::Independent(Arc::new(biased)), rep))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::position::Position;
    use crate::tally::mesh::RegularMesh;

    /// A 4-cell 1-D mesh over `[0, 4]` with windows `lower = [1, 2, 4, -1]`
    /// (the last cell has no window) in one group.
    fn windows() -> WeightWindows {
        let mesh = RegularMesh {
            lower_left: [0.0; 3],
            upper_right: [4.0, 1.0, 1.0],
            dimension: [4, 1, 1],
        };
        WeightWindows::new(
            mesh,
            vec![0.0, 2.0e7],
            vec![1.0, 2.0, 4.0, -1.0],
            vec![5.0, 10.0, 20.0, -1.0],
        )
        .unwrap()
    }

    fn box_source() -> FixedSource {
        FixedSource::Box {
            lower: Position::new(0.0, 0.0, 0.0),
            upper: Position::new(4.0, 1.0, 1.0),
            energy_ev: 1.0e6,
        }
    }

    /// **Methodology.** Hand numbers. Box `[0,4]` over four unit cells:
    /// `p = 1/4` each; importance `1/lower = [1, 1/2, 1/4, neutral]` with
    /// neutral the p-weighted mean of the windowed three, `(1 + 1/2 + 1/4)/3`.
    /// Weights `w_c = p_c/q_c` must satisfy `Σ q_c w_c = 1` (so `E[w] = 1`)
    /// and, for the windowed cells, `w_c / lower_c` identical across cells —
    /// the CADIS "every birth is at the same place in its window" property.
    #[test]
    fn spatial_weights_are_cadis_consistent_and_normalised() {
        let ww = windows();
        let (_, rep) = learned_biased_spatial(&ww, [0.0; 3], [4.0, 1.0, 1.0], 0).unwrap();
        assert_eq!(rep.n_cells, 4);
        assert_eq!(rep.n_no_window, 1);
        let neutral = (1.0 + 0.5 + 0.25) / 3.0;
        let imp = [1.0, 0.5, 0.25, neutral];
        let sq: f64 = imp.iter().sum();
        let w: Vec<f64> = imp.iter().map(|i| sq / (4.0 * i)).collect();
        // E_q[w] = Σ q w = Σ p = 1.
        let eq: f64 = imp.iter().zip(&w).map(|(i, wi)| i / sq * wi).sum();
        assert!((eq - 1.0).abs() < 1e-14);
        let lower = [1.0, 2.0, 4.0];
        let r: Vec<f64> = (0..3).map(|c| w[c] / lower[c]).collect();
        assert!((r[0] - r[1]).abs() < 1e-14 && (r[1] - r[2]).abs() < 1e-14);
        assert!((rep.weight_min - w[0]).abs() < 1e-14 && (rep.weight_max - w[2]).abs() < 1e-14);
        let second: f64 = (0..4).map(|c| 0.25 * w[c]).sum();
        assert!((rep.weight_rel_variance - (second - 1.0)).abs() < 1e-14);
    }

    /// **Methodology.** Statistical unbiasedness through the full
    /// `FixedSource` sampling path: 5 seeds x 200 000 births of the biased box
    /// source; `E[w] = 1`, `E[w x] = 2` (uniform on `[0,4]`) and
    /// `P(x < 1) = 1/4`, each within 4 sigma of the seed-to-seed error. The
    /// biased births are far from uniform (cell 0 is chosen most), so the
    /// weights are doing real work.
    #[test]
    fn biased_box_source_is_unbiased_in_the_mean() {
        let ww = windows();
        let (biased, _) = with_learned_source_bias(&box_source(), &ww).unwrap();
        let FixedSource::Independent(src) = &biased else { panic!("expected an Independent source") };
        let mut counts = [0usize; 4];
        let mut est = Vec::new();
        for s in 0..5u64 {
            let mut seed = 99 + 7_919 * s;
            let (mut sw, mut swx, mut sp) = (0.0, 0.0, 0.0);
            let n = 200_000;
            for _ in 0..n {
                let site = src.sample(&mut seed);
                sw += site.wgt;
                swx += site.wgt * site.r.x;
                if site.r.x < 1.0 {
                    sp += site.wgt;
                }
                counts[(site.r.x.floor() as usize).min(3)] += 1;
            }
            est.push([sw / n as f64, swx / n as f64, sp / n as f64]);
        }
        let truth = [1.0, 2.0, 0.25];
        for k in 0..3 {
            let m = est.iter().map(|e| e[k]).sum::<f64>() / 5.0;
            let v = est.iter().map(|e| (e[k] - m).powi(2)).sum::<f64>() / 4.0;
            let sem = (v / 5.0).sqrt();
            assert!((m - truth[k]).abs() <= 4.0 * sem, "estimator {k}: {m} +- {sem}, truth {}", truth[k]);
        }
        // The births really are biased towards cell 0 (importance highest).
        assert!(counts[0] > 2 * counts[2], "{counts:?}");
    }

    /// **Methodology.** A point source is a single cell: the identity, the
    /// source is returned unchanged, the report has unit weight and zero
    /// variance. Unsupported sources are errors, not silent analog.
    #[test]
    fn point_source_is_the_identity_and_unsupported_sources_error() {
        let ww = windows();
        let pt = FixedSource::Point { r: Position::new(0.5, 0.5, 0.5), energy_ev: 1.0e6 };
        let (out, rep) = with_learned_source_bias(&pt, &ww).unwrap();
        assert!(matches!(out, FixedSource::Point { .. }));
        assert_eq!(rep.weight_rel_variance, 0.0);
        assert_eq!((rep.weight_min, rep.weight_max), (1.0, 1.0));
        let off = FixedSource::Box {
            lower: Position::new(10.0, 0.0, 0.0),
            upper: Position::new(11.0, 1.0, 1.0),
            energy_ev: 1.0e6,
        };
        assert!(with_learned_source_bias(&off, &ww).is_err());
        let hi_e = FixedSource::Point { r: Position::new(0.5, 0.5, 0.5), energy_ev: 3.0e7 };
        assert!(with_learned_source_bias(&hi_e, &ww).is_err());
    }

    /// **Methodology.** No windowed source cell: an error rather than a guess.
    #[test]
    fn no_windowed_cell_is_an_error() {
        let mesh = RegularMesh { lower_left: [0.0; 3], upper_right: [2.0, 1.0, 1.0], dimension: [2, 1, 1] };
        let ww = WeightWindows::new(mesh, vec![0.0, 2.0e7], vec![-1.0; 2], vec![-1.0; 2]).unwrap();
        assert!(learned_biased_spatial(&ww, [0.0; 3], [2.0, 1.0, 1.0], 0).is_err());
        assert!(learned_biased_spatial(&ww, [0.0; 3], [2.0, 1.0, 1.0], 3).is_err());
    }

    /// **Methodology.** Energy bias on a two-line spectrum over two groups:
    /// group 0 window `lower = 1` everywhere, group 1 `lower = 4`, so
    /// `Ī = 1` and `1/4`; with `p = (1/2, 1/2)` the weights are
    /// `w_i = Σq/(2 q_i)`, `E_q[w] = 1`, and sampling reproduces
    /// `E[w E] = (E_1 + E_2)/2` within 4 sigma.
    #[test]
    fn discrete_energy_bias_is_unbiased() {
        let mesh = RegularMesh { lower_left: [0.0; 3], upper_right: [2.0, 1.0, 1.0], dimension: [2, 1, 1] };
        let ww = WeightWindows::new(
            mesh,
            vec![0.0, 1.0e6, 2.0e7],
            vec![1.0, 1.0, 4.0, 4.0],
            vec![5.0, 5.0, 20.0, 20.0],
        )
        .unwrap();
        let (d, rep) =
            learned_biased_energy(&ww, [0.0; 3], [2.0, 1.0, 1.0], &[5.0e5, 5.0e6], &[0.5, 0.5]).unwrap();
        assert_eq!(rep.n_cells, 2);
        let (mut a, mut b) = (0.0, 0.0);
        let n = 200_000;
        let mut seed = 5u64;
        let mut ws = Vec::new();
        for _ in 0..n {
            let (e, w) = d.sample(&mut seed);
            a += w;
            b += w * e;
            ws.push(w);
        }
        assert!((a / n as f64 - 1.0).abs() < 0.01, "E[w] = {}", a / n as f64);
        assert!((b / n as f64 - 2.75e6).abs() < 0.03 * 2.75e6, "E[wE] = {}", b / n as f64);
        // The low-energy (more important, Ī = 1) line is the more often chosen: q = (0.8, 0.2).
        assert!((rep.weight_min - 0.625).abs() < 1e-12 && (rep.weight_max - 2.5).abs() < 1e-12);
    }
}
