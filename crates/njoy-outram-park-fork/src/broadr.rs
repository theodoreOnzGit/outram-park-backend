//! BROADR — Doppler broadening of pointwise cross sections (SIGMA1 method).
//!
//! Ported from `bsigma`, `hunky`, and `funky` in NJOY2016 `broadr.f90`.
//!
//! ## SIGMA1 algorithm
//!
//! The free-gas Doppler-broadened cross section at energy E is:
//!
//! ```text
//! σ_D(y) = Σ_panels [σ(u_right) · s1 + slope · s2]  (below y)
//!        + Σ_panels [σ(u_left)  · s1 + slope · s2]  (above y)
//! ```
//!
//! where `y = √(α·E)`, `α = AWR / (k_B·T)`, and the cross section is
//! interpolated linearly in u² within each panel. The integrals `s1` and `s2`
//! are expressed via the analytic f-functions:
//!
//! ```text
//! f_n(a) = ∫_a^∞  z^n · exp(-z²) dz / √π
//! h_n    = f_n(a_old) - f_n(a_new)   (integral over one velocity panel)
//! s1 = h₂·oy² + 2·h₁·oy + h₀
//! s2 = ((h₄+(6y²-x²)·h₂)·oy + (4·h₃+(4y²-2x²)·h₁))·oy + (y²-x²)·h₀
//! ```
//!
//! where `oy = ±1/y` (negative for panels below y, positive for above).
//!
//! ## Implementation notes
//!
//! - `erfc` is implemented in pure Rust (A&S 7.1.26, max error < 1.2 × 10⁻⁷).
//! - The `hnabb` Taylor-series refinement from the Fortran is not implemented;
//!   the direct difference `h = f_old - f_new` is used throughout. The error
//!   from cancellation is < `TOLER × |f|` ≈ 10⁻⁵ of an already-small tail,
//!   negligible for practical energy grids.
//! - Both kernel terms are implemented: the dominant σ₊ (exp(-(x-y)²)) pass and
//!   the σ₋ (exp(-(x+y)²)) correction. The latter is only evaluated for y ≤ 4
//!   (it is < 10⁻⁷ above that) and matters mainly near thermal energies, where
//!   it pulls the broadened value back down toward the physical result.

use crate::{common::phys::BK_EV_PER_K, reconr::ReconrSection};

// ── erfc ──────────────────────────────────────────────────────────────────────

/// erfc(x) for x ≥ 0, max absolute error < 1.2 × 10⁻⁷ (A&S 7.1.26).
fn erfc_nonneg(x: f64) -> f64 {
    if x >= 10.0 { return 0.0; }
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let poly = t * (0.254_829_592
        + t * (-0.284_496_736
        + t * (1.421_413_741
        + t * (-1.453_152_027
        + t * 1.061_405_429))));
    poly * (-x * x).exp()
}

// ── f-functions ───────────────────────────────────────────────────────────────

// f_n(0) values (exact, used to initialise each panel loop)
const SQRT_PI: f64 = 1.772_453_850_905_516_0;
const INV2SQRTPI: f64 = 1.0 / (2.0 * SQRT_PI);
const F_ZERO: [f64; 5] = [0.5, INV2SQRTPI, 0.25, INV2SQRTPI, 0.375];

/// `f_n(a)` for n = 0..4 (0-indexed). Ported from `funky` in `broadr.f90`.
///
/// `f_n(a) = ∫_a^∞ z^n · exp(-z²) dz / √π`
fn f_funcs(a: f64) -> [f64; 5] {
    if a >= 10.0 { return [0.0; 5]; }
    let asq   = a * a;
    let expo  = (-asq).exp() / SQRT_PI;   // exp(-a²)/√π
    let f0    = 0.5 * erfc_nonneg(a);
    let f1    = 0.5 * expo;
    let expoa = expo * a;                  // a·exp(-a²)/√π
    let f2    = 0.5 * (f0 + expoa);
    let expoa2 = expoa * a;
    let f3    = 0.5 * (2.0 * f1 + expoa2);
    let expoa3 = expoa2 * a;
    let f4    = 0.5 * (3.0 * f2 + expoa3);
    [f0, f1, f2, f3, f4]
}

// ── hunky / s-terms ───────────────────────────────────────────────────────────

/// Compute h = f(a_old) − f(a_new) and the new f values at `a_new`.
///
/// Corresponds to `hunky` in `broadr.f90`. The h functions represent:
/// `h_n = ∫_{a_old}^{a_new} z^n · exp(-z²) dz / √π` (integral over one panel).
fn h_funcs(f_old: &[f64; 5], a_new: f64) -> ([f64; 5], [f64; 5]) {
    let f_new = f_funcs(a_new);
    let mut h = *f_old;
    for k in 0..5 {
        h[k] -= f_new[k];
        const SMALL: f64 = 1e-12;
        if h[k].abs() <= SMALL * f_new[k].abs() { h[k] = 0.0; }
    }
    (h, f_new)
}

/// Compute `s1` and `s2` from h-functions, oy, yy, xx (from `hunky` in Fortran).
///
/// - `oy = ±1/y` (negative for below-y panels, positive for above-y panels)
/// - `yy = y²`
/// - `xx = (reference endpoint)²`
fn s_terms(h: &[f64; 5], oy: f64, yy: f64, xx: f64) -> (f64, f64) {
    let s1 = (h[2] * oy + 2.0 * h[1]) * oy + h[0];
    let s2 = ((h[4] + (6.0 * yy - xx) * h[2]) * oy
            + (4.0 * h[3] + (4.0 * yy - 2.0 * xx) * h[1])) * oy
            + (yy - xx) * h[0];
    (s1, s2)
}

// ── bsigma ────────────────────────────────────────────────────────────────────

/// Compute the Doppler-broadened cross section at one output energy point.
///
/// `y = √(α·E_out)` is the query point in velocity space.
/// `eu` is the energy grid transformed to velocity space: `eu[i] = √(α·E[i])`.
/// `sigma` is the cross section grid \[b\].
///
/// Returns the broadened cross section \[b\] at the query point.
fn bsigma_scalar(y: f64, eu: &[f64], sigma: &[f64]) -> f64 {
    let n = eu.len();
    if n < 2 { return sigma[0]; }

    const ATOP: f64 = 4.0;     // Gaussian tail cutoff: exp(-16) ≈ 1e-7

    let yy  = y * y;
    let oy  = -1.0 / y;        // negative 1/y for below-y panels

    // Locate panel containing y
    let k = {
        let pos = eu.partition_point(|&u| u <= y);
        if pos == 0 { 0 } else { (pos - 1).min(n - 2) }
    };

    let mut sbt = 0.0f64;

    // ── Below-y panels (l = k down to 0) ────────────────────────────────────
    let mut f = F_ZERO;

    let mut l = k + 1;
    while l > 0 {
        l -= 1;
        let x  = eu[l];
        let xp = eu[l + 1];
        let xx = xp * xp;
        if xx <= x * x { continue; }

        let aa = y - x;
        let (h, f_new) = h_funcs(&f, aa);
        f = f_new;

        let denom = 1.0 / (xx - x * x);
        let slope = (sigma[l + 1] - sigma[l]) * denom;
        let (s1, s2) = s_terms(&h, oy, yy, xx);
        sbt += sigma[l + 1] * s1 + slope * s2;

        if aa > ATOP { break; }
    }

    // 1/v extension below klow=0: σ(u) = σ_0 · u_0 / u
    {
        let aa = y; // distance from y to u=0
        let (h, _) = h_funcs(&f, aa);
        // contribution: -σ_0 · u_0 · (oy² · h[1] + oy · h[0])
        sbt -= sigma[0] * eu[0] * (oy * oy * h[1] + oy * h[0]);
    }

    // ── Above-y panels (l = k up to n-2) ───────────────────────────────────
    let oy = -oy;               // flip: now positive 1/y for above-y panels
    let mut f = F_ZERO;

    for l in k..(n - 1) {
        let x  = eu[l + 1];    // right endpoint
        let xm = eu[l];        // left endpoint
        let xx = xm * xm;
        if xx >= x * x { continue; }

        let aa = x - y;
        let (h, f_new) = h_funcs(&f, aa);
        f = f_new;

        let denom = 1.0 / (x * x - xx);
        let slope = (sigma[l + 1] - sigma[l]) * denom;
        let (s1, s2) = s_terms(&h, oy, yy, xx);
        sbt += sigma[l] * s1 + slope * s2;

        if aa > ATOP { break; }
    }

    // Constant extension above khigh (last value)
    let factor = (f[2] * oy + 2.0 * f[1]) * oy + f[0];
    sbt += sigma[n - 1] * factor;

    // ── Second pass: σ₋ correction (the exp(-(x+y)²) term) ───────────────────
    //
    // The full SIGMA1 kernel is σ₊ − σ₋, where σ₋ integrates against
    // exp(-(x+y)²). It is negligible once y > ATOP (exp(-16) ≈ 1e-7) but is
    // significant near thermal (small y), where it pulls the result back down.
    // Ported from `bsigma` lines 1626–1660 in `broadr.f90` (label 210 onward).
    //
    // The (x+y) mapping means a = x + y, so at x = 0 we start at a = y (hence
    // `f_funcs(y)`, not `F_ZERO`), and a increases as x increases.
    if y <= ATOP {
        let oy = -1.0 / y;          // Fortran: y=-y; oy=-oy  ⇒  oy = -1/y
        let mut f = f_funcs(y);     // funky initialised at a = y

        // 1/v extension below klow=0: panel x ∈ [0, eu[0]], a ∈ [y, eu[0]+y]
        let aa0 = eu[0] + y;
        let (h, f_new) = h_funcs(&f, aa0);
        f = f_new;
        sbt -= sigma[0] * eu[0] * (oy * oy * h[1] + oy * h[0]);

        if aa0 <= ATOP {
            for l in 0..(n - 1) {
                let x  = eu[l + 1];     // right endpoint
                let xm = eu[l];         // left endpoint
                let xx = xm * xm;
                if x * x <= xx { continue; }

                let aa = x + y;
                let (h, f_new) = h_funcs(&f, aa);
                f = f_new;

                let denom = 1.0 / (x * x - xx);
                let slope = (sigma[l + 1] - sigma[l]) * denom;
                let (s1, s2) = s_terms(&h, oy, yy, xx);
                sbt -= sigma[l] * s1 + slope * s2;

                if aa > ATOP { break; }
            }

            // Constant extension above khigh
            let factor = (f[2] * oy + 2.0 * f[1]) * oy + f[0];
            sbt -= sigma[n - 1] * factor;
        }
    }

    sbt.max(0.0)
}

// ── Public API ────────────────────────────────────────────────────────────────

/// Doppler-broaden a set of RECONR cross-section sections using the SIGMA1 method.
///
/// All sections must share the same energy grid (as produced by RECONR). The
/// output has the same energy grid and the same MT numbers as the input, with
/// cross sections broadened to the free-gas thermal spectrum at `temp_k` \[K\].
///
/// # Parameters
/// - `sections` — pointwise (energy \[eV\], σ \[b\]) sections from RECONR.
/// - `awr`      — atomic weight ratio: target mass / neutron mass.
/// - `temp_k`   — effective temperature \[K\] for broadening.
///
/// # Physics
///
/// The velocity-space scaling factor `α = AWR / (k_B · T)` maps energies to
/// the dimensionless velocity `u = √(α·E)`. The free-gas kernel becomes a
/// Gaussian in `u`, enabling analytic panel-by-panel integration.
pub fn doppler_broaden(
    sections: &[ReconrSection],
    awr: f64,
    temp_k: f64,
) -> Vec<ReconrSection> {
    if sections.is_empty() || temp_k <= 0.0 {
        return sections.to_vec();
    }

    // α = AWR / (kB · T)  [eV⁻¹]
    let alpha = awr / (BK_EV_PER_K * temp_k);

    sections
        .iter()
        .map(|sec| {
            if sec.pairs.len() < 2 {
                return sec.clone();
            }

            // Each reaction is broadened on its own energy grid: RECONR may give
            // different reactions slightly different grids, so `eu` and `sigma`
            // must be built from the *same* section to stay length-consistent.
            let eu: Vec<f64> = sec.pairs
                .iter()
                .map(|&(e, _)| if e > 0.0 { (alpha * e).sqrt() } else { 0.0 })
                .collect();
            let sigma: Vec<f64> = sec.pairs.iter().map(|&(_, s)| s.max(0.0)).collect();

            // Broaden at each energy point
            let pairs: Vec<(f64, f64)> = sec
                .pairs
                .iter()
                .enumerate()
                .map(|(j, &(e, _))| {
                    if e <= 0.0 {
                        return (e, 0.0);
                    }
                    (e, bsigma_scalar(eu[j], &eu, &sigma))
                })
                .collect();

            ReconrSection { mt: sec.mt, qi: sec.qi, pairs }
        })
        .collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MtReaction;

    #[test]
    fn erfc_at_zero_is_one() {
        assert!((erfc_nonneg(0.0) - 1.0).abs() < 1e-7);
    }

    #[test]
    fn erfc_at_one_approx() {
        // erfc(1.0) ≈ 0.15729921
        assert!((erfc_nonneg(1.0) - 0.157_299_21).abs() < 1e-6);
    }

    #[test]
    fn f_funcs_at_zero_match_fzero() {
        // f[0] = ½·erfc(0); our erfc rational approx gives 0.99999999… not 1,
        // so the tolerance reflects the A&S 7.1.26 error bound, not float eps.
        let f = f_funcs(0.0);
        for k in 0..5 {
            let err = (f[k] - F_ZERO[k]).abs();
            assert!(err < 1e-6, "f[{k}]={} vs F_ZERO[{k}]={}", f[k], F_ZERO[k]);
        }
    }

    /// Brute-force reference: the SIGMA1 kernel evaluated by fine trapezoid
    /// integration, using the same 1/v-below / constant-above extensions that
    /// `bsigma_scalar` assumes. This is the definition the analytic panel
    /// formulas must reproduce.
    fn bsigma_reference(y: f64, eu: &[f64], sigma: &[f64]) -> f64 {
        let u0 = eu[0];
        let un = eu[eu.len() - 1];
        let sig = |x: f64| -> f64 {
            if x < u0 {
                sigma[0] * u0 / x            // 1/v extension below the grid
            } else if x >= un {
                sigma[sigma.len() - 1]       // constant extension above the grid
            } else {
                // linear in u² (= linear in energy) within the panel
                let i = eu.partition_point(|&u| u <= x).max(1) - 1;
                let (xl, xr) = (eu[i], eu[i + 1]);
                let t = (x * x - xl * xl) / (xr * xr - xl * xl);
                sigma[i] + t * (sigma[i + 1] - sigma[i])
            }
        };
        // ∫₀^∞ σ(x) x² [exp(-(x-y)²) - exp(-(x+y)²)] dx / (y² √π)
        let n = 2_000_000;
        let xmax = y + 8.0;
        let dx = xmax / n as f64;
        let mut acc = 0.0;
        for k in 0..=n {
            let x = (k as f64 + 0.5) * dx;
            if x >= xmax { break; }
            let w = if k == 0 || k == n { 0.5 } else { 1.0 };
            let kern = (-(x - y).powi(2)).exp() - (-(x + y).powi(2)).exp();
            acc += w * sig(x) * x * x * kern;
        }
        acc * dx / (y * y * SQRT_PI)
    }

    #[test]
    fn bsigma_matches_numerical_reference() {
        // Flat σ broadened at a spread of y values (low → high). The analytic
        // panel result must track the brute-force kernel integral everywhere.
        let kb = BK_EV_PER_K;
        let alpha = 1.0 / (kb * 300.0);          // AWR=1, T=300 K
        let e_grid: Vec<f64> = (0..50).map(|i| (i + 1) as f64 * 0.01).collect();
        let eu: Vec<f64> = e_grid.iter().map(|&e| (alpha * e).sqrt()).collect();
        let sigma = vec![100.0; e_grid.len()];

        for &j in &[0usize, 4, 9, 24, 49] {
            let y = eu[j];
            let got = bsigma_scalar(y, &eu, &sigma);
            let want = bsigma_reference(y, &eu, &sigma);
            let rel = (got - want).abs() / want.abs();
            assert!(
                rel < 5e-3,
                "y={y:.3}: analytic={got:.4} vs numerical={want:.4} (rel {rel:.2e})"
            );
        }
    }

    #[test]
    fn broadening_preserves_one_over_v() {
        // SIGMA1 leaves a 1/v cross section invariant: σ(E) = K/√E ∝ 1/u.
        // The 1/v-below extension is exact, so interior points should recover
        // the input to high accuracy.
        let kb = BK_EV_PER_K;
        let alpha = 1.0 / (kb * 300.0);
        let e_grid: Vec<f64> = (0..80).map(|i| (i + 1) as f64 * 0.01).collect();
        let eu: Vec<f64> = e_grid.iter().map(|&e| (alpha * e).sqrt()).collect();
        let k_const = 50.0;
        let sigma: Vec<f64> = e_grid.iter().map(|&e| k_const / e.sqrt()).collect();

        // Check interior points (away from the constant-above extension at the top).
        for j in 5..40 {
            let y = eu[j];
            let got = bsigma_scalar(y, &eu, &sigma);
            let want = sigma[j];
            let rel = (got - want).abs() / want;
            assert!(
                rel < 1e-2,
                "1/v not preserved at E={:.2}: got {got:.3}, want {want:.3} (rel {rel:.2e})",
                e_grid[j]
            );
        }
    }

    #[test]
    fn broadening_flat_xs_stays_flat_at_high_y() {
        // A flat σ does NOT stay flat at low y (the 1+1/2y² enhancement is real),
        // but at high y (E ≫ kT/AWR) broadening leaves it essentially unchanged.
        let kb = BK_EV_PER_K;
        let alpha = 1.0 / (kb * 300.0);
        // Energies chosen so the smallest y is large: E from 10 to 60 eV.
        let e_grid: Vec<f64> = (0..50).map(|i| 10.0 + i as f64 * 1.0).collect();
        let eu: Vec<f64> = e_grid.iter().map(|&e| (alpha * e).sqrt()).collect();
        let sigma = vec![100.0; e_grid.len()];

        for j in 5..45 {
            let y = eu[j];
            let got = bsigma_scalar(y, &eu, &sigma);
            assert!(
                (got - 100.0).abs() < 0.5,
                "high-y flat XS should stay ≈100, got {got} at E={:.1}",
                e_grid[j]
            );
        }
    }

    #[test]
    fn broadening_at_zero_temp_returns_unchanged() {
        // At T→0, alpha→∞, so all contributions collapse — in practice we bail
        // early on temp_k ≤ 0.
        let pairs = vec![(0.01, 10.0), (1.0, 5.0), (10.0, 2.0)];
        let sec = ReconrSection { mt: MtReaction::Mt2Elastic, qi: 0.0, pairs };
        let out = doppler_broaden(&[sec.clone()], 35.0, 0.0);
        assert_eq!(out[0].pairs, sec.pairs);
    }

}
