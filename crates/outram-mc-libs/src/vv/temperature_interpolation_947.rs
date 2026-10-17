//! Measuring instruments for GitHub #947: how far stochastic interpolation
//! between tabulated temperatures
//! ([`crate::material::temperature_interpolation`]) lands from data processed
//! exactly at the intermediate temperature.
//!
//! Everything here works on plain slices on one shared energy grid, so it is
//! tested on closed forms (`#[cfg(test)]` below) and the study example
//! (`examples/temperature_interpolation_947.rs`) only supplies nuclear data.
//! Methodology and results: `verification_and_validation/temperature_interpolation_947/`.
//!
//! # The three things measured, and why each
//!
//! 1. [`pointwise`]: the expected cross section `E[σ](E)` against the exact
//!    `σ(E, T)`, energy by energy — the largest relative error in a band,
//!    both signs, and where.
//! 2. [`lethargy_integral`] of `σ`: an infinitely-dilute band integral
//!    `∫ σ dE/E`. Doppler broadening nearly conserves resonance area, so
//!    this is **expected to hide** the error; it is reported to show that.
//! 3. [`ShieldedIntegrals`]: the narrow-resonance self-shielded integrals
//!    `N = ∫ σ_x/(σ_t+σ_0) dE/E`, `D = ∫ 1/(σ_t+σ_0) dE/E`. These are what
//!    a collision sees: per lookup a transport code draws one table, so in an
//!    infinite medium the rate is the **mixture of the two tables' `N` and
//!    `D`** ([`ShieldedIntegrals::mix`]), not `N` of the mixed σ. Both are
//!    reported.
//!
//! The transport quantity proper — a resonance escape probability — uses
//! [`crate::physics::slowing_down`]; [`mix_slowing_down_grids`] forms the
//! stochastic arm's grid from the per-table grids.

use crate::physics::slowing_down::SlowingDownGrid;

/// The largest relative errors of `approx` against `exact` over grid points
/// with `e_lo <= E <= e_hi`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointwiseError {
    /// Largest `approx/exact - 1` (most positive; 0 if none positive).
    pub max_over: f64,
    /// Energy \[eV\] of [`Self::max_over`].
    pub e_over: f64,
    /// Most negative `approx/exact - 1` (0 if none negative).
    pub max_under: f64,
    /// Energy \[eV\] of [`Self::max_under`].
    pub e_under: f64,
    /// Points compared.
    pub points: usize,
}

impl PointwiseError {
    /// The larger magnitude of the two signs.
    pub fn max_abs(&self) -> f64 {
        self.max_over.max(-self.max_under)
    }
}

/// Pointwise relative error over `[e_lo, e_hi]` \[eV\]; points where `exact`
/// is not positive are skipped (a relative error is undefined there).
pub fn pointwise(
    grid: &[f64],
    approx: &[f64],
    exact: &[f64],
    e_lo: f64,
    e_hi: f64,
) -> PointwiseError {
    let mut out = PointwiseError {
        max_over: 0.0,
        e_over: f64::NAN,
        max_under: 0.0,
        e_under: f64::NAN,
        points: 0,
    };
    for ((&e, &a), &x) in grid.iter().zip(approx).zip(exact) {
        if e < e_lo || e > e_hi || x <= 0.0 {
            continue;
        }
        out.points += 1;
        let r = a / x - 1.0;
        if r > out.max_over {
            out.max_over = r;
            out.e_over = e;
        }
        if r < out.max_under {
            out.max_under = r;
            out.e_under = e;
        }
    }
    out
}

/// `∫ y dE/E` over `[e_lo, e_hi]` by the trapezoid rule in lethargy on the
/// grid points inside the band (the same quadrature for every arm, so arm
/// differences are not quadrature differences).
pub fn lethargy_integral(grid: &[f64], y: &[f64], e_lo: f64, e_hi: f64) -> f64 {
    let mut sum = 0.0;
    let mut prev: Option<(f64, f64)> = None;
    for (&e, &v) in grid.iter().zip(y) {
        if e < e_lo || e > e_hi {
            continue;
        }
        if let Some((e0, v0)) = prev {
            sum += 0.5 * (v0 + v) * (e / e0).ln();
        }
        prev = Some((e, v));
    }
    sum
}

/// Narrow-resonance self-shielded integrals at background `σ_0` \[b\]:
/// `N = ∫ σ_x/(σ_t+σ_0) dE/E` and `D = ∫ 1/(σ_t+σ_0) dE/E`, whose ratio is
/// the flux-weighted (Bondarenko) `σ_x,eff`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShieldedIntegrals {
    /// `∫ σ_x/(σ_t+σ_0) dE/E` \[b\].
    pub n: f64,
    /// `∫ 1/(σ_t+σ_0) dE/E` \[1/b\].
    pub d: f64,
}

impl ShieldedIntegrals {
    /// The integrals of one table (or of a plugged-in σ) over `[e_lo, e_hi]`.
    pub fn of(
        grid: &[f64],
        sigma_x: &[f64],
        sigma_t: &[f64],
        sigma0: f64,
        e_lo: f64,
        e_hi: f64,
    ) -> Self {
        let num: Vec<f64> = sigma_x
            .iter()
            .zip(sigma_t)
            .map(|(x, t)| x / (t + sigma0))
            .collect();
        let den: Vec<f64> = sigma_t.iter().map(|t| 1.0 / (t + sigma0)).collect();
        Self {
            n: lethargy_integral(grid, &num, e_lo, e_hi),
            d: lethargy_integral(grid, &den, e_lo, e_hi),
        }
    }

    /// What a per-lookup draw between tables gives: each integral is linear
    /// in the per-energy collision quantities, so it is the weighted sum of
    /// the tables' integrals.
    pub fn mix(parts: &[(ShieldedIntegrals, f64)]) -> Self {
        let (mut n, mut d) = (0.0, 0.0);
        for (p, w) in parts {
            n += w * p.n;
            d += w * p.d;
        }
        Self { n, d }
    }

    /// The flux-weighted effective cross section `N / D` \[b\].
    pub fn effective(&self) -> f64 {
        self.n / self.d
    }

    /// `σ_0 · N`: the NR **resonance integral** of channel `x` \[b\], the
    /// form `p = exp(-N_x I/(ξ Σ_p))` uses.
    pub fn resonance_integral(&self, sigma0: f64) -> f64 {
        sigma0 * self.n
    }
}

/// The slowing-down grid a per-collision draw between tables sees: every
/// collision fraction is the weighted sum of the tables'. All grids must
/// share their lethargy points and α (build them with the same band, energy
/// grid and mixture).
///
/// # Panics
///
/// If the grids do not share lethargy points, mixture size or α: mixing them
/// would mean nothing, and this is only called by the study's own code.
pub fn mix_slowing_down_grids(parts: &[(&SlowingDownGrid, f64)]) -> SlowingDownGrid {
    let first = parts.first().expect("at least one grid").0;
    let n_mix = first.alpha.len();
    let n = first.lethargy.len();
    let mut scatter_frac = vec![vec![0.0; n]; n_mix];
    let mut absorb_frac = vec![vec![0.0; n]; n_mix];
    for (g, w) in parts {
        assert_eq!(
            g.lethargy, first.lethargy,
            "grids must share lethargy points"
        );
        assert_eq!(g.alpha, first.alpha, "grids must share the mixture");
        for i in 0..n_mix {
            for j in 0..n {
                scatter_frac[i][j] += w * g.scatter_frac[i][j];
                absorb_frac[i][j] += w * g.absorb_frac[i][j];
            }
        }
    }
    SlowingDownGrid {
        lethargy: first.lethargy.clone(),
        scatter_frac,
        absorb_frac,
        alpha: first.alpha.clone(),
    }
}

/// The slowing-down grid on `lethargy` (from a grid already built, with
/// source energy `e_top` \[eV\]) when each component's macroscopic
/// `(Σ_scatter, Σ_absorb)` \[1/cm\] at grid point j, energy E, is `sigma(j, E)[i]`. For arms
/// whose cross sections are not one table's (a plugged-in mixture, a fit).
pub fn slowing_down_grid_from<F>(
    lethargy: &[f64],
    e_top: f64,
    alpha: &[f64],
    mut sigma: F,
) -> SlowingDownGrid
where
    F: FnMut(usize, f64) -> Vec<(f64, f64)>,
{
    let n_mix = alpha.len();
    let n = lethargy.len();
    let mut scatter_frac = vec![vec![0.0; n]; n_mix];
    let mut absorb_frac = vec![vec![0.0; n]; n_mix];
    for (j, &u) in lethargy.iter().enumerate() {
        let s = sigma(j, e_top * (-u).exp());
        let tot: f64 = s.iter().map(|(a, b)| a + b).sum();
        for (i, (sc, ab)) in s.iter().enumerate().take(n_mix) {
            scatter_frac[i][j] = sc / tot;
            absorb_frac[i][j] = ab / tot;
        }
    }
    SlowingDownGrid {
        lethargy: lethargy.to_vec(),
        scatter_frac,
        absorb_frac,
        alpha: alpha.to_vec(),
    }
}

/// The URR band intervals behind
/// [`UrrProbabilityTables::band_representatives`](njoy_outram_park_fork::purr::UrrProbabilityTables::band_representatives):
/// `(xi, weight)` pairs, one per interval of `xi` on which the band choice is
/// constant, weight = the interval's length. `reps` must be that function's
/// output (cuts `0, c_1, ..` and the midpoints between them, in any order).
///
/// Returns `None` if `reps` is not of that form, so a change upstream is
/// noticed rather than integrated wrongly.
pub fn urr_band_weights(reps: &[f64]) -> Option<Vec<(f64, f64)>> {
    let mut r = reps.to_vec();
    r.sort_by(f64::total_cmp);
    if r.len() < 2 || r.len() % 2 != 0 || r[0] != 0.0 {
        return None;
    }
    let mut out = Vec::with_capacity(r.len() / 2);
    for k in 0..r.len() / 2 {
        let (cut, mid) = (r[2 * k], r[2 * k + 1]);
        let next = if 2 * k + 2 < r.len() {
            r[2 * k + 2]
        } else {
            1.0
        };
        if (mid - 0.5 * (cut + next)).abs() > 1e-12 {
            return None;
        }
        out.push((mid, next - cut));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointwise_finds_both_signs_in_band() {
        let g = [1.0, 2.0, 3.0, 4.0];
        let p = pointwise(&g, &[1.1, 0.9, 2.0, 5.0], &[1.0, 1.0, 2.0, 1.0], 1.0, 3.0);
        assert_eq!(p.points, 3);
        assert!((p.max_over - 0.1).abs() < 1e-12 && p.e_over == 1.0);
        assert!((p.max_under + 0.1).abs() < 1e-12 && p.e_under == 2.0);
        assert!((p.max_abs() - 0.1).abs() < 1e-12);
    }

    #[test]
    fn lethargy_integral_of_constant_is_log_ratio() {
        let g: Vec<f64> = (0..=100).map(|k| 10f64.powf(k as f64 / 50.0)).collect();
        let y = vec![3.0; g.len()];
        assert!((lethargy_integral(&g, &y, 1.0, 100.0) - 3.0 * 100f64.ln()).abs() < 1e-12);
        // 1/v-like y = 1/sqrt(E): exact is 2 (1 - 1/10) over [1, 100].
        let y: Vec<f64> = g.iter().map(|e| 1.0 / e.sqrt()).collect();
        assert!((lethargy_integral(&g, &y, 1.0, 100.0) - 1.8).abs() < 1e-3);
    }

    #[test]
    fn shielded_integrals_mix_linearly_and_differ_from_plug_in() {
        let g: Vec<f64> = (0..=200).map(|k| 1.0 + k as f64 * 0.01).collect();
        // Two "tables": a tall narrow line and a broad low one, same area.
        let a: Vec<f64> = g
            .iter()
            .map(|e| 10.0 + 1000.0 * (-((e - 2.0) / 0.02f64).powi(2)).exp())
            .collect();
        let b: Vec<f64> = g
            .iter()
            .map(|e| 10.0 + 500.0 * (-((e - 2.0) / 0.04f64).powi(2)).exp())
            .collect();
        let s0 = 50.0;
        let (ia, ib) = (
            ShieldedIntegrals::of(&g, &a, &a, s0, 1.0, 3.0),
            ShieldedIntegrals::of(&g, &b, &b, s0, 1.0, 3.0),
        );
        let m = ShieldedIntegrals::mix(&[(ia, 0.5), (ib, 0.5)]);
        assert!((m.n - 0.5 * (ia.n + ib.n)).abs() < 1e-14);
        // Plugging the mixed σ in is NOT the mixture: g(σ) = σ/(σ+σ0) is
        // concave, so the plug-in absorbs more.
        let mixed: Vec<f64> = a.iter().zip(&b).map(|(x, y)| 0.5 * (x + y)).collect();
        let plug = ShieldedIntegrals::of(&g, &mixed, &mixed, s0, 1.0, 3.0);
        assert!(plug.n > m.n);
        assert!(m.effective() > 0.0 && (m.resonance_integral(s0) - s0 * m.n).abs() < 1e-12);
    }

    #[test]
    fn slowing_down_grids_mix_and_rebuild() {
        let lethargy = vec![0.0, 0.01, 0.02];
        let alpha = vec![0.5];
        let g1 = slowing_down_grid_from(&lethargy, 10.0, &alpha, |_, _| vec![(3.0, 1.0)]);
        let g2 = slowing_down_grid_from(&lethargy, 10.0, &alpha, |_, _| vec![(1.0, 1.0)]);
        assert!((g1.absorb_frac[0][1] - 0.25).abs() < 1e-15);
        let m = mix_slowing_down_grids(&[(&g1, 0.5), (&g2, 0.5)]);
        assert!((m.absorb_frac[0][2] - 0.375).abs() < 1e-15);
        assert!((m.scatter_frac[0][0] - 0.625).abs() < 1e-15);
    }

    #[test]
    fn urr_band_weights_rebuilds_the_intervals() {
        // cuts 0, 0.2, 0.7 (and 1); midpoints 0.1, 0.45, 0.85.
        let reps = [0.1, 0.45, 0.85, 0.0, 0.2, 0.7];
        let w = urr_band_weights(&reps).unwrap();
        assert_eq!(w.len(), 3);
        assert!((w.iter().map(|p| p.1).sum::<f64>() - 1.0).abs() < 1e-15);
        assert!((w[1].0 - 0.45).abs() < 1e-15 && (w[1].1 - 0.5).abs() < 1e-15);
        assert!(urr_band_weights(&[0.1, 0.45]).is_none());
        assert!(urr_band_weights(&[]).is_none());
    }
}
