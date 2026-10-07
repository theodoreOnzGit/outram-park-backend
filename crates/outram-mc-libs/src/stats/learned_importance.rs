//! **Variance reduction from learned importance** — weight windows built from
//! early-batch tallies, with a RAFFLES surrogate filling the cells the tally
//! could not resolve. GitHub #497.
//!
//! # OFF by default, and why that is not a breach of "correct physics is the
//! default"
//!
//! The root `CLAUDE.md` rule binds **physics** terms. A weight window is an
//! **estimator** — it splits and rouletted particles in fair games that leave
//! every expected score unchanged — so it is an acceleration, not physics, and
//! the correct default is analog (the same position as
//! [`crate::physics::variance_reduction`]). Nothing in this module runs unless
//! a caller builds the windows here **and** attaches them to a
//! [`VarianceReduction`](crate::physics::variance_reduction::VarianceReduction)
//! (`with_weight_windows`); `VarianceReduction::default()` stays analog, so a
//! default run is untouched.
//!
//! # What "learned" means here
//!
//! MAGIC ([`WeightWindows::update_magic`]) sets each window's lower bound
//! proportional to the forward flux in that cell, and marks every cell whose
//! tally is empty or too noisy as **"no window" (`-1`)**. In a deep-penetration
//! problem those are exactly the cells that most need importance. This module
//! keeps MAGIC's rule where the tally resolves the flux and, in each energy
//! group, fits a [`raffles::surrogate::PolynomialSurrogate`] to
//! `ln(flux density)` against the cell-centre coordinates of the **resolved**
//! cells, then fills the unresolved cells from the surrogate. `ln` because flux
//! attenuates roughly exponentially through a shield, so a low-order
//! polynomial in `ln φ` is the physically natural reduced model.
//!
//! - **Extrapolation is bounded**, at a limit fixed in advance: a filled value
//!   may not exceed the group's largest measured density, nor fall more than
//!   [`LearnedImportanceSettings::max_extrapolation_decades`] decades below its
//!   smallest. A polynomial in `ln φ` extrapolated far enough produces
//!   astronomically large or small windows; the clamp is reported, never
//!   silent.
//! - **The surrogate's held-out error is reported** (leave-one-out RMSE in
//!   `ln φ`, [`raffles::surrogate::validation`]), so a reader can see how good
//!   the fill is, rather than trusting it.
//! - Too few resolved cells to identify the polynomial: the group is left as
//!   MAGIC left it (unresolved = no window) and the report says so.
//!
//! **Source biasing** from the same importance map is NOT implemented here;
//! the existing `physics/ufs.rs` uniform-fission-site weights remain the only
//! source-side scheme.
//!
//! # Why the mean cannot be biased in principle — and why that is still tested
//!
//! Any set of positive window bounds plays fair split/roulette games, so the
//! expected tally is the analog one whatever the surrogate predicts; a bad
//! surrogate costs variance, not bias. **That is an argument, not a
//! measurement.** A defect in the game (the `max_split` weight-creation bug in
//! `weight_windows.rs`' history) is invisible except by comparing against the
//! analog arm, which is why [`FomComparison`] carries the unbiasedness check
//! and treats it as the gate.
//!
//! # V&V gate (from #497, fixed in advance)
//!
//! An unbiased mean (agreement with the analog result within combined `σ`)
//! and a measured FOM ratio `FOM = 1/(σ²T)` with uncertainty, against analog
//! **and** against plain MAGIC (`weight_windows.rs`). A biased answer fails
//! regardless of the FOM. "Within combined `σ`" is implemented as
//! `|z| ≤ UNBIASED_Z_BAND = 2` (95 %); the issue's wording could be read as
//! 1σ, which a correct estimator fails 32 % of the time — **flagged for the
//! maintainer to confirm, not decided after seeing a result.**
//!
//! **Result: NOT YET MEASURED (testing deferred by maintainer, 2026-10-03).**
//! Protocol: `verification_and_validation/stats_epic_493/497_learned_importance.md`.

use crate::physics::weight_windows::WeightWindows;
use crate::tally::mesh::{unflatten, RegularMesh};
use raffles::surrogate::validation::LeaveOneOut;

/// `|z|` band for the unbiasedness gate (two-sided 95 %), fixed in advance.
/// See the module docs for why 2 and not 1.
pub const UNBIASED_Z_BAND: f64 = 2.0;

/// Settings for [`learned_weight_windows`]. Defaults are fixed in advance and
/// documented per field; none is tuned to a result.
#[derive(Debug, Clone, PartialEq)]
pub struct LearnedImportanceSettings {
    /// MAGIC's relative-error threshold: a cell with a larger relative error
    /// is "unresolved". Default `0.5`, the usual MAGIC choice.
    pub threshold: f64,
    /// `upper = ratio · lower`. Default `5.0` (OpenMC's default
    /// `upper_bound_ratio`).
    pub ratio: f64,
    /// Total degree of the `ln φ(x, y, z)` polynomial. Default `2`: an
    /// exponential attenuation is degree 1 in `ln φ`, and one more degree
    /// admits curvature (geometric spreading) without the wild extrapolation
    /// of higher orders.
    pub degree: usize,
    /// Ridge on the normal equations. Default `1e-8` (the polynomial module's
    /// own suggested start), for near-degenerate cell layouts.
    pub ridge: f64,
    /// Decades below the smallest measured density a filled cell may go.
    /// Default `3`.
    pub max_extrapolation_decades: f64,
    /// Replace resolved cells' measured values by the surrogate as well
    /// (smoothing). Default `false`: measured values are kept.
    pub smooth_resolved: bool,
}

impl Default for LearnedImportanceSettings {
    fn default() -> Self {
        Self {
            threshold: 0.5,
            ratio: 5.0,
            degree: 2,
            ridge: 1e-8,
            max_extrapolation_decades: 3.0,
            smooth_resolved: false,
        }
    }
}

/// Per-energy-group account of what was measured, filled and left empty.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupFill {
    /// Energy group index.
    pub group: usize,
    /// Cells whose tally resolved the flux (MAGIC's windows).
    pub resolved: usize,
    /// Unresolved cells given a window from the surrogate.
    pub filled: usize,
    /// Of those, how many hit the extrapolation clamp.
    pub clamped: usize,
    /// Cells still without a window.
    pub empty: usize,
    /// Leave-one-out RMSE of the surrogate in `ln φ` over the resolved cells;
    /// `None` when no surrogate was fitted.
    pub loo_rmse_ln: Option<f64>,
    /// Why no surrogate was fitted, when none was.
    pub note: Option<String>,
}

/// Build weight windows from early-batch flux moments.
///
/// `sum`, `sum_sq` (per-batch first and second moments summed over `n`
/// batches, laid out `[energy][mesh]` exactly as for
/// [`WeightWindows::update_magic`]) and `volumes` (per mesh cell) describe the
/// early tally; `energy_bounds` are the windows' energy groups.
///
/// # Errors
///
/// Anything [`WeightWindows::new`] or [`WeightWindows::update_magic`] rejects.
pub fn learned_weight_windows(
    mesh: RegularMesh,
    energy_bounds: Vec<f64>,
    sum: &[f64],
    sum_sq: &[f64],
    volumes: &[f64],
    n: usize,
    settings: &LearnedImportanceSettings,
) -> Result<(WeightWindows, Vec<GroupFill>), String> {
    let n_m = mesh.n_bins();
    let n_e = energy_bounds.len().saturating_sub(1);
    let placeholder = vec![-1.0; n_e * n_m];
    let mut ww = WeightWindows::new(mesh.clone(), energy_bounds, placeholder.clone(), placeholder)?;
    // MAGIC first: it validates the inputs and marks resolved cells with a
    // positive lower bound proportional to the flux density.
    ww.update_magic(sum, sum_sq, volumes, n, settings.threshold, settings.ratio)?;

    let centres = cell_centres(&mesh);
    let axes: Vec<usize> = (0..3).filter(|&a| mesh.dimension[a] > 1).collect();
    let mut fills = Vec::with_capacity(n_e);
    for e in 0..n_e {
        let lower = &mut ww.lower[e * n_m..(e + 1) * n_m];
        let resolved: Vec<usize> = (0..n_m).filter(|&m| lower[m] > 0.0).collect();
        let unresolved = n_m - resolved.len();
        let mut fill = GroupFill {
            group: e,
            resolved: resolved.len(),
            filled: 0,
            clamped: 0,
            empty: unresolved,
            loo_rmse_ln: None,
            note: None,
        };
        let terms = raffles::surrogate::PolynomialSurrogate::basis_size(axes.len(), settings.degree);
        if axes.is_empty() || resolved.len() < terms + 2 || (unresolved == 0 && !settings.smooth_resolved) {
            fill.note = Some(if unresolved == 0 && !settings.smooth_resolved {
                "every cell resolved; nothing to fill".into()
            } else {
                format!(
                    "{} resolved cells cannot identify a {}-term polynomial (need >= {}); \
                     unresolved cells left without windows, as MAGIC leaves them",
                    resolved.len(),
                    terms,
                    terms + 2
                )
            });
            fills.push(fill);
            continue;
        }
        let xs: Vec<Vec<f64>> = resolved
            .iter()
            .map(|&m| axes.iter().map(|&a| centres[m][a]).collect())
            .collect();
        let ys: Vec<f64> = resolved.iter().map(|&m| lower[m].ln()).collect();
        let loo = match LeaveOneOut::fit(&xs, &ys, settings.degree, settings.ridge) {
            Ok(l) => l,
            Err(err) => {
                fill.note = Some(format!("surrogate fit failed: {err}"));
                fills.push(fill);
                continue;
            }
        };
        fill.loo_rmse_ln = Some(loo.rmse());
        let ln_max = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let ln_min = ys.iter().cloned().fold(f64::INFINITY, f64::min)
            - settings.max_extrapolation_decades * std::f64::consts::LN_10;
        let is_resolved: Vec<bool> = (0..n_m).map(|m| lower[m] > 0.0).collect();
        for m in 0..n_m {
            if is_resolved[m] && !settings.smooth_resolved {
                continue;
            }
            let x: Vec<f64> = axes.iter().map(|&a| centres[m][a]).collect();
            let Ok(p) = loo.full.predict(&x) else { continue };
            let c = p.clamp(ln_min, ln_max);
            if !is_resolved[m] {
                if c != p {
                    fill.clamped += 1;
                }
                fill.filled += 1;
                fill.empty -= 1;
            }
            lower[m] = c.exp();
        }
        fills.push(fill);
    }
    // Upper bounds follow the (possibly filled) lower bounds, as MAGIC sets
    // them; a filled cell is NOT renormalised, so measured cells keep exactly
    // MAGIC's values and the group maximum stays at MAGIC's 1/2 (the clamp
    // keeps fills at or below it).
    for i in 0..ww.lower.len() {
        ww.upper[i] = if ww.lower[i] > 0.0 {
            settings.ratio * ww.lower[i]
        } else {
            settings.ratio * -1.0
        };
    }
    Ok((ww, fills))
}

/// Cell centres `[x, y, z]` in cm, in the mesh's flat bin order.
fn cell_centres(mesh: &RegularMesh) -> Vec<[f64; 3]> {
    let w = mesh.width();
    (0..mesh.n_bins())
        .map(|b| {
            let ijk = unflatten(b, mesh.dimension);
            [
                mesh.lower_left[0] + (ijk[0] as f64 + 0.5) * w[0],
                mesh.lower_left[1] + (ijk[1] as f64 + 0.5) * w[1],
                mesh.lower_left[2] + (ijk[2] as f64 + 0.5) * w[2],
            ]
        })
        .collect()
}

/// One arm of a variance-reduction comparison: a tally's estimate, its 1σ,
/// the wall-clock it cost and the number of batches its `σ` came from.
///
/// Timings are only comparable as ratios on one machine (crate `CLAUDE.md`,
/// "Every timing carries its hardware") — record the hardware with them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VrArm {
    /// The estimate.
    pub mean: f64,
    /// Its 1σ standard error.
    pub sigma: f64,
    /// Wall-clock seconds.
    pub seconds: f64,
    /// Independent batches (or seeds) the `σ` is estimated from.
    pub n_batches: usize,
}

impl VrArm {
    /// `FOM = 1/(R² T)` with `R = σ/|mean|` the relative error — the issue's
    /// `1/(σ²T)` on the relative scale, so arms with different means compare.
    pub fn fom(&self) -> f64 {
        let r = self.sigma / self.mean.abs();
        1.0 / (r * r * self.seconds)
    }

    /// Relative 1σ of [`Self::fom`] from the statistical error of `σ` alone:
    /// `FOM ∝ σ⁻²`, and a sample `σ` from `n` batches carries relative error
    /// `1/sqrt(2(n − 1))`, so the FOM carries twice that.
    pub fn fom_relative_sigma(&self) -> f64 {
        2.0 / (2.0 * (self.n_batches as f64 - 1.0)).sqrt()
    }
}

/// Analog against variance-reduced: the unbiasedness gate and the FOM ratio.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FomComparison {
    /// `(vr.mean − analog.mean) / sqrt(σ_a² + σ_v²)`.
    pub z: f64,
    /// `|z| ≤ UNBIASED_Z_BAND`. **The gate**: a `false` here fails the scheme
    /// whatever its FOM.
    pub unbiased: bool,
    /// `FOM_vr / FOM_analog`.
    pub fom_ratio: f64,
    /// 1σ on `fom_ratio`, combining both arms' relative FOM errors.
    pub fom_ratio_sigma: f64,
}

impl FomComparison {
    /// Compare `vr` against `analog`.
    pub fn new(analog: VrArm, vr: VrArm) -> Self {
        let z = (vr.mean - analog.mean) / (analog.sigma.powi(2) + vr.sigma.powi(2)).sqrt();
        let fom_ratio = vr.fom() / analog.fom();
        let rel = (analog.fom_relative_sigma().powi(2) + vr.fom_relative_sigma().powi(2)).sqrt();
        Self {
            z,
            unbiased: z.abs() <= UNBIASED_Z_BAND,
            fom_ratio,
            fom_ratio_sigma: fom_ratio * rel,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slab() -> RegularMesh {
        RegularMesh {
            lower_left: [0.0, 0.0, 0.0],
            upper_right: [20.0, 1.0, 1.0],
            dimension: [20, 1, 1],
        }
    }

    /// **Methodology.** A 1-D slab whose "tally" is an exact exponential
    /// attenuation `φ(x) = exp(−0.5 x)` resolved in the first 12 cells and
    /// empty in the last 8 (zero sum — what a deep shield looks like). The
    /// degree-1 fit in `ln φ` is then exact, so every filled cell must equal
    /// MAGIC's normalisation of the true flux to `1e-9` relative, no fill may
    /// be clamped, and resolved cells must keep MAGIC's values bit for bit.
    /// The fit is exact only **without** regularisation, so this test sets
    /// `ridge: 0.0`.
    ///
    /// ~~**Result:** NOT YET MEASURED (testing deferred by maintainer,
    /// 2026-10-03).~~ **CORRECTED 2026-10-07 (gh:#584).** The first run
    /// (2026-10-05) failed at cell 12. The test kept the default `ridge: 1e-8`,
    /// which `PolynomialSurrogate::fit` adds to the whole normal-equation
    /// diagonal. That shrinks the slope and gives a relative error of
    /// `9.95e-9` at cell 12 (`1.7e-8` at cell 19), against the `1e-9` gate.
    /// With no ridge the error is ~`1e-15` (numpy replica, gh:#584
    /// 2026-10-06 audit). The premise "the degree-1 fit is exact" requires no
    /// ridge, so the test now sets `ridge: 0.0`. The gate stays at `1e-9`.
    /// The default ridge is unchanged.
    #[test]
    fn exact_attenuation_is_filled_exactly() {
        let mesh = slab();
        let n = 10;
        let mut sum = vec![0.0; 20];
        let mut sum_sq = vec![0.0; 20];
        for m in 0..12 {
            let phi = (-0.5 * (m as f64 + 0.5)).exp();
            // Ten batches scoring phi*(1 ± 0.01): resolved, rel err ~0.3 %.
            for b in 0..n {
                let v = phi * (1.0 + if b % 2 == 0 { 0.01 } else { -0.01 });
                sum[m] += v;
                sum_sq[m] += v * v;
            }
        }
        let volumes = vec![1.0; 20];
        let settings = LearnedImportanceSettings {
            degree: 1,
            ridge: 0.0,
            ..Default::default()
        };
        let (ww, fills) =
            learned_weight_windows(mesh.clone(), vec![0.0, 2.0e7], &sum, &sum_sq, &volumes, n, &settings)
                .unwrap();
        let mut magic = WeightWindows::new(mesh, vec![0.0, 2.0e7], vec![-1.0; 20], vec![-1.0; 20]).unwrap();
        magic.update_magic(&sum, &sum_sq, &volumes, n, 0.5, 5.0).unwrap();
        assert_eq!(fills[0].resolved, 12);
        assert_eq!(fills[0].filled, 8);
        assert_eq!(fills[0].clamped, 0);
        for m in 0..12 {
            assert_eq!(ww.lower[m].to_bits(), magic.lower[m].to_bits());
        }
        let scale = magic.lower[0] / (-0.25_f64).exp();
        for m in 12..20 {
            let want = scale * (-0.5 * (m as f64 + 0.5)).exp();
            assert!(((ww.lower[m] - want) / want).abs() < 1e-9, "cell {m}");
            assert_eq!(ww.upper[m], 5.0 * ww.lower[m]);
        }
    }

    /// **Methodology.** Too few resolved cells for the polynomial: the group
    /// is left exactly as MAGIC left it and the report says why.
    #[test]
    fn underdetermined_groups_are_left_alone() {
        let mesh = slab();
        let mut sum = vec![0.0; 20];
        let mut sum_sq = vec![0.0; 20];
        for m in 0..2 {
            sum[m] = 10.0 + m as f64;
            sum_sq[m] = 10.2 + 2.0 * m as f64;
        }
        let (ww, fills) = learned_weight_windows(
            mesh,
            vec![0.0, 2.0e7],
            &sum,
            &sum_sq,
            &[1.0; 20],
            10,
            &LearnedImportanceSettings::default(),
        )
        .unwrap();
        assert_eq!(fills[0].filled, 0);
        assert!(fills[0].note.is_some());
        assert!(ww.lower[2..].iter().all(|&l| l < 0.0));
    }

    /// **Methodology.** FOM bookkeeping on hand numbers: halving `σ` at equal
    /// time quadruples the FOM; a 3σ shift fails the unbiasedness gate.
    #[test]
    fn fom_and_unbiasedness_bookkeeping() {
        let a = VrArm { mean: 1.0, sigma: 0.02, seconds: 10.0, n_batches: 50 };
        let v = VrArm { mean: 1.0, sigma: 0.01, seconds: 10.0, n_batches: 50 };
        let c = FomComparison::new(a, v);
        assert!((c.fom_ratio - 4.0).abs() < 1e-12);
        assert!(c.unbiased);
        let biased = VrArm { mean: 1.0 + 3.0 * (0.02_f64.powi(2) + 0.01_f64.powi(2)).sqrt(), ..v };
        assert!(!FomComparison::new(a, biased).unbiased);
    }
}
