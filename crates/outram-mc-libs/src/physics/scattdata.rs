// SPDX-License-Identifier: GPL-3.0

//! **Anisotropic multigroup scattering** — Legendre-expanded angular
//! distributions for the MG transport path, and the tabular form upstream
//! converts them into. GitHub #265.
//!
//! Ported from `ScattDataLegendre` / `ScattDataTabular` /
//! `convert_legendre_to_tabular` (`src/scattdata.cpp`) and `evaluate_legendre`
//! (`src/math_functions.cpp:120`) at OpenMC `afa7a14`.
//!
//! # The defect this addresses, and its known size
//!
//! `physics_mg.rs` resamples the outgoing direction **isotropically in the lab
//! frame** at every scatter. That is a P0 set by construction, whatever
//! anisotropy the library carries.
//!
//! This crate has already paid for that exact error once, on the
//! continuous-energy side. `CLAUDE.md` records bead `op-tm9f`: sampling
//! inelastic scattering isotropically instead of from its ENDF MF=4 law moved
//! Godiva by **-198 pcm** (ANISO `+45 +/- 32` against ISO `+269 +/- 30`, a
//! `-224 +/- 44 pcm` difference at 5.1 sigma), because understating `<mu>`
//! inflates `Sigma_tr = Sigma_t (1 - <mu>)` and suppresses leakage.
//!
//! So this is a **known-sized** error in the MG path, not an unknown small
//! one, and it has the same sign.
//!
//! # What a P0 set does and does not excuse
//!
//! A transport-corrected P0 set partly compensates — that is what transport
//! correction is for — but only in the diffusion-like limit, and **nothing in
//! the crate checks that a supplied set is transport-corrected rather than
//! plain P0**. `Mgxs::new` asserts group-count consistency and a
//! self-consistency residual, not this. [`ScatterRepresentation`] exists so the
//! assumption is recorded in the data rather than implied by a doc comment.
//!
//! # The mean cosine a Legendre kernel DELIVERS is not the one it declares
//!
//! A truncated Legendre series is not a probability density: for a P1 kernel
//! `f(mu) = 1/2 + (3/2) a_1 mu` the series goes negative below
//! `mu = -1 / (3 a_1)`, which lies inside `[-1, 1]` as soon as
//! `|a_1| > 1/3`. **Both** of upstream's sampling paths silently remove that
//! lobe — the Legendre path by the `if (f > 0.)` guard in the rejection loop
//! (`:359`), the tabular path by clamping `fmu` to zero and renormalising
//! (`:874-894`). So what gets sampled is the *positive part, renormalised*,
//! and its mean is strictly smaller in magnitude than `a_1 / a_0`.
//!
//! This is not a subtlety that can be left to a comment, because it is exactly
//! the quantity that drives `Sigma_tr`. Measured here (see
//! `sampling_reproduces_the_mean_cosine_that_is_actually_sampled`):
//!
//! | `a_1` | declared `<mu>` | actually sampled `<mu>` | loss |
//! |---|---|---|---|
//! | 0.0 | 0.0 | 0.0 | — |
//! | 0.2 | 0.2 | 0.2 | none (series stays positive) |
//! | **0.5** | **0.5** | **4/9 = 0.44444** (closed form) | **−11.1 %** |
//! | −0.3 | −0.3 | −0.3 | none |
//!
//! [`LegendreKernel::mean_cosine`] reports the declared value, the moment
//! ratio. [`LegendreKernel::sampled_mean_cosine`] reports what the sampler
//! actually delivers. **They are different functions on purpose**, and a gate
//! that compares sampled cosines against `mean_cosine` for a negative-going
//! kernel is testing the wrong number.

use crate::physics::collision_probability::gauss_legendre;
use crate::rng::lcg::prn;

/// Upstream's rejection-sampling cap (`MAX_SAMPLE`, `src/scattdata.cpp:354`).
const MAX_SAMPLE: usize = 1000;

/// Points used to bound the Legendre series for rejection sampling
/// (`update_max_val`, `:296`).
const N_MU_SCAN: usize = 1001;

/// The 10 % margin upstream adds to the scanned maximum
/// (`update_max_val`, `:319`: *"Since we may not have caught the true max, add
/// 10% margin"*).
///
/// # Why this is not cosmetic
///
/// Rejection sampling accepts at `min(f / M, 1)`. If the true maximum of the
/// series exceeds the box height `M` — which a 1001-point scan cannot rule out
/// for a peaked kernel — every `mu` in the region where `f > M` is accepted
/// with probability 1 instead of `f / M`, so that region is sampled at a
/// *flattened* density. The result is a biased angular distribution that no
/// error is raised for. The margin does not prove `M >= max f`, but it is what
/// upstream does, and omitting it makes this port strictly more biased than
/// the code it claims to mirror.
const MAX_VAL_MARGIN: f64 = 1.1;

/// Default cosine-grid size for the Legendre-to-tabular conversion
/// (`DEFAULT_NMU`, `include/openmc/constants.h:292`).
pub const DEFAULT_NMU: usize = 33;

/// What angular representation a multigroup set actually carries.
///
/// # Why this is data and not a comment
///
/// Before this existed, "the library is P0 / transport-corrected" was a claim
/// in a doc comment that nothing could check. A caller handing in a genuine P3
/// set got it sampled isotropically anyway; a caller handing in a plain
/// (untransport-corrected) P0 set got no warning that the compensation the
/// approximation relies on was absent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScatterRepresentation {
    /// P0 with no transport correction. Isotropic-in-lab sampling is then
    /// **wrong in a known direction** — it understates `<mu>`.
    IsotropicP0,
    /// P0 whose total has been transport-corrected. Isotropic sampling is the
    /// intended treatment, valid in the diffusion-like limit.
    TransportCorrectedP0,
    /// Legendre moments to the given order; sample the outgoing cosine from
    /// them.
    Legendre { order: usize },
    /// A tabulated `f(mu)` on a uniform cosine grid of the given size — what
    /// `convert_legendre_to_tabular` produces, and what upstream actually runs
    /// when `legendre_to_tabular_points` is set.
    Tabular { points: usize },
}

/// Evaluate the Legendre series `sum_l (l + 1/2) a_l P_l(mu)`.
///
/// `evaluate_legendre` (`src/math_functions.cpp:120`). The `(l + 1/2)` factor
/// is part of the evaluation, **not** folded into the stored coefficients —
/// carrying it in the data instead would silently rescale every moment and the
/// P0 term would still look right, which is what makes the mistake survive a
/// smoke test.
pub fn evaluate_legendre(coeffs: &[f64], mu: f64) -> f64 {
    // Bonnet recursion: P_0 = 1, P_1 = mu,
    // (l+1) P_{l+1} = (2l+1) mu P_l - l P_{l-1}.
    let mut p_prev = 1.0_f64;
    let mut val = 0.5 * coeffs[0];
    if coeffs.len() == 1 {
        return val;
    }
    let mut p = mu;
    val += 1.5 * coeffs[1] * p;
    for l in 1..coeffs.len() - 1 {
        let lf = l as f64;
        let p_next = ((2.0 * lf + 1.0) * mu * p - lf * p_prev) / (lf + 1.0);
        p_prev = p;
        p = p_next;
        val += (lf + 1.5) * coeffs[l + 1] * p;
    }
    val
}

/// Legendre coefficients of the scattering kernel for one `(g_in, g_out)` pair,
/// already normalised the way upstream stores them in `dist`.
#[derive(Debug, Clone, PartialEq)]
pub struct LegendreKernel {
    /// `a_0 .. a_order`.
    pub coeffs: Vec<f64>,
    /// Bounding-box height for rejection sampling — the maximum of the series
    /// over `[-1, 1]` on upstream's 1001-point scan, times [`MAX_VAL_MARGIN`].
    max_val: f64,
}

impl LegendreKernel {
    /// Build from raw coefficients, precomputing the rejection bound.
    ///
    /// # Errors
    ///
    /// An empty coefficient list, or a kernel whose maximum over `[-1, 1]` is
    /// not positive — the latter cannot be sampled by rejection at all, and
    /// upstream would spin to `MAX_SAMPLE` and abort at run time. Caught here
    /// instead.
    pub fn new(coeffs: Vec<f64>) -> Result<Self, String> {
        if coeffs.is_empty() {
            return Err("a Legendre kernel needs at least the P0 coefficient".into());
        }
        // `update_max_val` (`:291`): scan a fixed grid and take the largest
        // value. Reproduced including the endpoints, which is what keeps a
        // forward-peaked kernel's maximum (at mu = 1) inside the box, and
        // including the 10 % margin applied afterwards.
        let mut max_val = 0.0_f64;
        let dmu = 2.0 / (N_MU_SCAN as f64 - 1.0);
        for imu in 0..N_MU_SCAN {
            let mu = if imu == 0 {
                -1.0
            } else if imu == N_MU_SCAN - 1 {
                1.0
            } else {
                -1.0 + (imu as f64 - 1.0) * dmu
            };
            let f = evaluate_legendre(&coeffs, mu);
            if f > max_val {
                max_val = f;
            }
        }
        if !(max_val > 0.0) {
            return Err(format!(
                "the Legendre kernel is non-positive everywhere on [-1, 1] (max {max_val}); \
                 it cannot be sampled by rejection. Upstream would spin to MAX_SAMPLE and \
                 abort mid-run."
            ));
        }
        Ok(Self {
            coeffs,
            max_val: max_val * MAX_VAL_MARGIN,
        })
    }

    /// The rejection bounding-box height actually in use, margin included.
    pub fn bounding_height(&self) -> f64 {
        self.max_val
    }

    /// The (unnormalised) density at `mu`.
    pub fn f(&self, mu: f64) -> f64 {
        evaluate_legendre(&self.coeffs, mu)
    }

    /// `<mu>` **as declared by the moments**: `a_1 / a_0`.
    ///
    /// This is the mean cosine the library says it carries. It is *not*
    /// necessarily the mean cosine that gets sampled — see
    /// [`Self::sampled_mean_cosine`] and the module docs. For a kernel that
    /// stays positive on `[-1, 1]` the two agree exactly.
    pub fn mean_cosine(&self) -> f64 {
        if self.coeffs.len() < 2 || self.coeffs[0] == 0.0 {
            0.0
        } else {
            self.coeffs[1] / self.coeffs[0]
        }
    }

    /// The cosines in `[-1, 1]` at which the series crosses zero.
    ///
    /// Found by a 4096-interval sign scan followed by 80 bisections, so each
    /// root is located to machine precision. A root the series *touches*
    /// without crossing is missed — it bounds a zero-measure interval and
    /// changes no integral, which is the only thing this is used for.
    fn sign_changes(&self) -> Vec<f64> {
        const SCAN: usize = 4096;
        let mut out = Vec::new();
        let mut a = -1.0_f64;
        let mut fa = self.f(a);
        for i in 1..=SCAN {
            let b = -1.0 + 2.0 * i as f64 / SCAN as f64;
            let fb = self.f(b);
            if (fa > 0.0) != (fb > 0.0) {
                let lo_positive = fa > 0.0;
                let (mut lo, mut hi) = (a, b);
                for _ in 0..80 {
                    let mid = 0.5 * (lo + hi);
                    if (self.f(mid) > 0.0) == lo_positive {
                        lo = mid;
                    } else {
                        hi = mid;
                    }
                }
                out.push(0.5 * (lo + hi));
            }
            a = b;
            fa = fb;
        }
        out
    }

    /// The sub-intervals of `[-1, 1]` on which the series is positive — i.e.
    /// the support the rejection loop's `if (f > 0.)` guard leaves behind.
    pub fn positive_support(&self) -> Vec<(f64, f64)> {
        let mut breaks = vec![-1.0];
        breaks.extend(self.sign_changes());
        breaks.push(1.0);
        let mut out = Vec::new();
        for w in breaks.windows(2) {
            let (a, b) = (w[0], w[1]);
            if b <= a {
                continue;
            }
            if self.f(0.5 * (a + b)) > 0.0 {
                out.push((a, b));
            }
        }
        out
    }

    /// Whether the series dips below zero somewhere in `[-1, 1]`, i.e. whether
    /// the sampled distribution differs from the declared one at all.
    pub fn goes_negative(&self) -> bool {
        self.positive_support() != vec![(-1.0, 1.0)]
    }

    /// `<mu>` **of the distribution actually sampled**: the positive part of
    /// the series, renormalised.
    ///
    /// Integrated exactly — the integrand is a polynomial on each positive
    /// sub-interval, and Gauss-Legendre with `order + 5` nodes is exact to
    /// degree `2(order + 5) - 1`, far above the `order + 1` needed. The only
    /// approximation is the location of the roots, which is at machine
    /// precision.
    ///
    /// Equals [`Self::mean_cosine`] whenever the kernel stays positive; is
    /// strictly smaller in magnitude when it does not.
    pub fn sampled_mean_cosine(&self) -> f64 {
        let n = self.coeffs.len() + 5;
        let (x, w) = gauss_legendre(n);
        let (mut num, mut den) = (0.0_f64, 0.0_f64);
        for (a, b) in self.positive_support() {
            let half = 0.5 * (b - a);
            let mid = 0.5 * (a + b);
            for i in 0..n {
                let mu = mid + half * x[i];
                let weighted = self.f(mu) * w[i] * half;
                den += weighted;
                num += mu * weighted;
            }
        }
        if den > 0.0 {
            num / den
        } else {
            0.0
        }
    }

    /// Truncate the expansion to `max_order`, the way upstream's
    /// `settings::max_order` bounds a library at read time
    /// (`src/mgxs.cpp`, `settings::max_order`).
    ///
    /// # This is a lossy operation and it is not symmetric
    ///
    /// Dropping moments above `max_order` does **not** simply coarsen the
    /// shape: it changes `<mu>` only if `max_order == 0` (which forces
    /// isotropy), while truncating a P3 to a P1 keeps `a_1` and therefore
    /// keeps the declared mean cosine exactly — but it can move the *sampled*
    /// one, because a truncated series can go negative where the full one did
    /// not, and the sampler then removes the negative lobe. Truncating is
    /// therefore not free even when `a_1` survives.
    ///
    /// # Errors
    ///
    /// A truncation that leaves a series non-positive everywhere. The bound is
    /// recomputed from scratch rather than reused.
    pub fn truncated_to(&self, max_order: usize) -> Result<Self, String> {
        let keep = (max_order + 1).min(self.coeffs.len());
        Self::new(self.coeffs[..keep].to_vec())
    }

    /// The Legendre order carried, i.e. `coeffs.len() - 1`.
    pub fn order(&self) -> usize {
        self.coeffs.len() - 1
    }

    /// Sample `mu` by rejection from a rectangular bounding box —
    /// `ScattDataLegendre::sample` (`:350-365`).
    ///
    /// # Errors
    ///
    /// Exhausting `MAX_SAMPLE` attempts. Upstream calls `fatal_error` here;
    /// this returns instead so a caller can decide, but it is **not** silently
    /// fudged to isotropic — falling back to isotropic on a rejection failure
    /// would reintroduce exactly the defect this module exists to remove, and
    /// would do it only for the most sharply peaked kernels.
    pub fn sample_mu(&self, seed: &mut u64) -> Result<f64, String> {
        for _ in 0..MAX_SAMPLE {
            let mu = 2.0 * prn(seed) - 1.0;
            let f = self.f(mu);
            if f > 0.0 {
                let u = prn(seed) * self.max_val;
                if u <= f {
                    return Ok(mu);
                }
            }
        }
        Err(format!(
            "Legendre rejection sampling failed after {MAX_SAMPLE} attempts; the kernel \
             is too peaked for its bounding box (max {})",
            self.max_val
        ))
    }
}

/// A tabulated `f(mu)` on a uniform cosine grid, with its CDF — the form
/// `convert_legendre_to_tabular` (`:835`) produces and `ScattDataTabular`
/// samples by inversion instead of rejection.
///
/// # Why this exists alongside [`LegendreKernel`]
///
/// It is not an optimisation of the same distribution. The conversion applies
/// a **negativity clamp on the grid** and then renormalises, so on a coarse
/// grid it samples a measurably different distribution from the rejection
/// path — the clamp lands on grid points rather than on the true root. The
/// difference is measured in
/// `the_tabular_grid_costs_a_measurable_amount_of_mean_cosine`, not asserted
/// away.
#[derive(Debug, Clone, PartialEq)]
pub struct TabularKernel {
    /// Uniform cosine grid, `mu[0] = -1`, `mu[n-1] = +1`.
    pub mu: Vec<f64>,
    /// Normalised `f(mu)` on that grid, negatives clamped to zero.
    pub fmu: Vec<f64>,
    /// Normalised cumulative distribution, `cdf[0] = 0`, `cdf[n-1] = 1`.
    pub cdf: Vec<f64>,
    dmu: f64,
}

impl TabularKernel {
    /// Convert a Legendre kernel onto an `n_mu`-point cosine grid —
    /// `convert_legendre_to_tabular` (`:835`).
    ///
    /// Upstream picks `n_mu = 2` for a P0 kernel and [`DEFAULT_NMU`] otherwise
    /// when the user has not set `legendre_to_tabular_points` (`:840-847`);
    /// [`Self::from_legendre_default`] reproduces that choice.
    ///
    /// # Errors
    ///
    /// Fewer than two points, or a kernel whose clamped integral is zero — the
    /// latter is upstream's `if (norm > 0.)` branch falling through, which
    /// leaves an all-zero CDF that `sample` would walk off the end of.
    pub fn from_legendre(leg: &LegendreKernel, n_mu: usize) -> Result<Self, String> {
        if n_mu < 2 {
            return Err(format!("a tabular cosine grid needs at least 2 points, got {n_mu}"));
        }
        let dmu = 2.0 / (n_mu as f64 - 1.0);
        let mu: Vec<f64> = (0..n_mu)
            .map(|i| {
                if i == n_mu - 1 {
                    1.0
                } else {
                    -1.0 + i as f64 * dmu
                }
            })
            .collect();
        // `evaluate_legendre` on the grid, then "Ensure positivity" (`:872`).
        let mut fmu: Vec<f64> = mu.iter().map(|&m| leg.f(m).max(0.0)).collect();
        // Trapezoid CDF, then normalise both (`:878-895`).
        let mut cdf = vec![0.0_f64; n_mu];
        let mut norm = 0.0_f64;
        for i in 1..n_mu {
            norm += 0.5 * dmu * (fmu[i - 1] + fmu[i]);
            cdf[i] = norm;
        }
        if !(norm > 0.0) {
            return Err(format!(
                "the clamped kernel integrates to {norm} on a {n_mu}-point grid; there is \
                 nothing to sample. Upstream leaves the CDF all-zero here and samples \
                 off the end of it."
            ));
        }
        for i in 0..n_mu {
            fmu[i] /= norm;
            cdf[i] /= norm;
        }
        Ok(Self { mu, fmu, cdf, dmu })
    }

    /// The conversion with upstream's own default grid size (`:840-847`).
    pub fn from_legendre_default(leg: &LegendreKernel) -> Result<Self, String> {
        let n_mu = if leg.coeffs.len() <= 1 { 2 } else { DEFAULT_NMU };
        Self::from_legendre(leg, n_mu)
    }

    /// Sample `mu` by inverting the piecewise-linear CDF —
    /// `ScattDataTabular::sample` (`:757-795`).
    pub fn sample_mu(&self, seed: &mut u64) -> f64 {
        let np = self.mu.len();
        let xi = prn(seed);
        let mut c_k = self.cdf[0];
        let mut k = 0usize;
        while k < np - 1 {
            let c_k1 = self.cdf[k + 1];
            if xi < c_k1 {
                break;
            }
            c_k = c_k1;
            k += 1;
        }
        let k = k.min(np - 2);
        let (p0, p1) = (self.fmu[k], self.fmu[k + 1]);
        let (mu0, mu1) = (self.mu[k], self.mu[k + 1]);
        let mu = if p0 == p1 {
            // A flat bin. Upstream divides by p0 with no guard; a zero-width
            // bin can only be reached when xi lands exactly on its lower CDF
            // value, and p0 = 0 then gives inf. Clamped below either way, but
            // returning the bin edge is the defensible answer.
            if p0 == 0.0 {
                mu0
            } else {
                mu0 + (xi - c_k) / p0
            }
        } else {
            let frac = (p1 - p0) / (mu1 - mu0);
            mu0 + ((p0 * p0 + 2.0 * frac * (xi - c_k)).max(0.0).sqrt() - p0) / frac
        };
        mu.clamp(-1.0, 1.0)
    }

    /// `<mu>` of the tabulated, piecewise-linear density — exactly, by
    /// integrating the same linear interpolant [`Self::sample_mu`] inverts.
    ///
    /// On a bin `[mu0, mu0 + h]` with endpoint densities `p0`, `p1`:
    /// `h [ mu0 (p0 + p1) / 2 + h (p0 + 2 p1) / 6 ]`.
    pub fn mean_cosine(&self) -> f64 {
        let h = self.dmu;
        let mut m = 0.0_f64;
        for i in 0..self.mu.len() - 1 {
            let (p0, p1) = (self.fmu[i], self.fmu[i + 1]);
            m += h * (self.mu[i] * 0.5 * (p0 + p1) + h * (p0 + 2.0 * p1) / 6.0);
        }
        m
    }
}

/// A **histogram** angular distribution: `order` equal-width cosine bins over
/// `[-1, 1]`, with the density constant inside each bin — `ScattDataHistogram`
/// (`:588-706`, `:735-756`).
///
/// # Why this is a separate type and not a special case of [`TabularKernel`]
///
/// They are different densities. A tabular kernel interpolates **linearly**
/// between grid points and is sampled by inverting that linear form; a
/// histogram is piecewise **constant** and is sampled uniformly inside the
/// chosen bin. Collapsing them would silently reinterpret one library's data
/// as the other's, which is exactly the class of error this module exists to
/// stop.
#[derive(Debug, Clone, PartialEq)]
pub struct HistogramKernel {
    /// Bin edges, `order + 1` of them, `mu[0] = -1`, `mu[order] = +1`.
    pub mu: Vec<f64>,
    /// Normalised density in each of the `order` bins.
    pub fmu: Vec<f64>,
    /// Cumulative distribution **at the top edge of each bin**, `order`
    /// entries, ending at 1. Upstream stores it this way (`:697`) rather than
    /// with a leading zero, and the sampling index depends on that.
    pub cdf: Vec<f64>,
    dmu: f64,
}

impl HistogramKernel {
    /// Build from per-bin values, normalising to unit integral (`:690-705`).
    ///
    /// # Errors
    ///
    /// Fewer than one bin, a negative bin value, or an all-zero histogram.
    /// Upstream's `if (norm > 0.)` simply skips the normalisation in the last
    /// case, leaving an all-zero CDF that `sample` walks off the end of; that
    /// is refused here instead.
    pub fn new(bins: Vec<f64>) -> Result<Self, String> {
        let order = bins.len();
        if order == 0 {
            return Err("a histogram kernel needs at least one bin".into());
        }
        if let Some(bad) = bins.iter().position(|&b| b < 0.0) {
            return Err(format!(
                "histogram bin {bad} is {} — a density cannot be negative. \
                 Upstream does not check this and would build a CDF that is \
                 not monotone.",
                bins[bad]
            ));
        }
        let dmu = 2.0 / order as f64;
        let mu: Vec<f64> = (0..=order)
            .map(|i| if i == order { 1.0 } else { -1.0 + i as f64 * dmu })
            .collect();
        let mut cdf = vec![0.0_f64; order];
        let mut acc = 0.0_f64;
        for i in 0..order {
            acc += dmu * bins[i];
            cdf[i] = acc;
        }
        let norm = cdf[order - 1];
        if !(norm > 0.0) {
            return Err(format!(
                "the histogram integrates to {norm}; there is nothing to sample"
            ));
        }
        let fmu = bins.into_iter().map(|b| b / norm).collect();
        let cdf = cdf.into_iter().map(|c| c / norm).collect();
        Ok(Self { mu, fmu, cdf, dmu })
    }

    /// Convert a Legendre kernel onto an `order`-bin histogram by evaluating
    /// the series at each bin centre and clamping negatives to zero.
    ///
    /// Upstream has no such conversion — its histograms come from the library
    /// file. This exists so the three representations can be compared on the
    /// same distribution, and its bin-centre sampling is stated rather than
    /// implied: it is **not** a bin average, so it does not preserve the
    /// integral of a rapidly varying kernel.
    pub fn from_legendre(leg: &LegendreKernel, order: usize) -> Result<Self, String> {
        if order == 0 {
            return Err("a histogram kernel needs at least one bin".into());
        }
        let dmu = 2.0 / order as f64;
        let bins: Vec<f64> = (0..order)
            .map(|i| leg.f(-1.0 + (i as f64 + 0.5) * dmu).max(0.0))
            .collect();
        Self::new(bins)
    }

    /// The density at `mu` — `ScattDataHistogram::calc_f` (`:735`).
    pub fn f(&self, mu: f64) -> f64 {
        let order = self.fmu.len();
        let imu = if mu >= 1.0 {
            order - 1
        } else {
            (((mu + 1.0) / self.dmu).floor() as usize).min(order - 1)
        };
        self.fmu[imu]
    }

    /// Sample `mu` — `ScattDataHistogram::sample` (`:709-733`): pick the bin by
    /// CDF search, then draw uniformly inside it.
    ///
    /// Upstream's `upper_bound` returns `end()` when `xi` exceeds the last
    /// cumulative, giving `imu == order` and an out-of-range edge lookup that
    /// only the final `clamp` rescues. The index is clamped here instead, which
    /// is the same outcome by a route that does not depend on the clamp.
    pub fn sample_mu(&self, seed: &mut u64) -> f64 {
        let order = self.fmu.len();
        let xi = prn(seed);
        let imu = if xi < self.cdf[0] {
            0
        } else {
            self.cdf.partition_point(|&c| c <= xi).min(order - 1)
        };
        (prn(seed) * self.dmu + self.mu[imu]).clamp(-1.0, 1.0)
    }

    /// `<mu>` of the histogram, exactly: the density is constant per bin, so
    /// each bin contributes `p_i * dmu * (bin centre)`.
    pub fn mean_cosine(&self) -> f64 {
        let mut m = 0.0_f64;
        for (i, &p) in self.fmu.iter().enumerate() {
            m += p * self.dmu * (self.mu[i] + 0.5 * self.dmu);
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P0-only must be flat: `f(mu) = a_0 / 2` everywhere.
    #[test]
    fn a_p0_kernel_is_flat() {
        let k = LegendreKernel::new(vec![2.0]).unwrap();
        for mu in [-1.0, -0.5, 0.0, 0.3, 1.0] {
            assert!((k.f(mu) - 1.0).abs() < 1e-15, "f({mu}) = {}", k.f(mu));
        }
        assert_eq!(k.mean_cosine(), 0.0, "P0 has no preferred direction");
        assert!(!k.goes_negative());
    }

    /// The `(l + 1/2)` factor is applied at evaluation. A P1 kernel
    /// `[1, a1]` must give `0.5 + 1.5 a1 mu`.
    #[test]
    fn the_half_plus_l_factor_is_in_the_evaluation() {
        let k = LegendreKernel::new(vec![1.0, 0.4]).unwrap();
        for mu in [-1.0, -0.2, 0.0, 0.7, 1.0] {
            let want = 0.5 + 1.5 * 0.4 * mu;
            assert!((k.f(mu) - want).abs() < 1e-14, "f({mu}) = {}", k.f(mu));
        }
        assert!((k.mean_cosine() - 0.4).abs() < 1e-15);
    }

    /// Higher orders follow Bonnet's recursion. Checked against P2 and P3 in
    /// closed form at several cosines.
    #[test]
    fn higher_orders_match_the_closed_form_polynomials() {
        let c = vec![1.0, 0.3, 0.2, 0.1];
        let k = LegendreKernel::new(c.clone()).unwrap();
        for mu in [-1.0, -0.6, -0.1, 0.0, 0.25, 0.8, 1.0] {
            let p0 = 1.0;
            let p1 = mu;
            let p2 = 0.5 * (3.0 * mu * mu - 1.0);
            let p3 = 0.5 * (5.0 * mu * mu * mu - 3.0 * mu);
            let want =
                0.5 * c[0] * p0 + 1.5 * c[1] * p1 + 2.5 * c[2] * p2 + 3.5 * c[3] * p3;
            assert!(
                (k.f(mu) - want).abs() < 1e-13,
                "f({mu}) = {} != {want}",
                k.f(mu)
            );
        }
    }

    /// Upstream's 10 % bounding-box margin is applied
    /// (`update_max_val`, `:319`).
    ///
    /// A P0 kernel `[2]` is flat at `f = 1`, so the scanned maximum is exactly
    /// 1 and the box must come out at exactly 1.1. Nothing else in the module
    /// would notice this being dropped — a missing margin biases the sampled
    /// angular distribution without raising anything.
    #[test]
    fn the_bounding_box_carries_upstreams_ten_percent_margin() {
        let k = LegendreKernel::new(vec![2.0]).unwrap();
        assert!(
            (k.bounding_height() - 1.1).abs() < 1e-15,
            "box height {} (scanned max 1.0 x 1.1 expected)",
            k.bounding_height()
        );
    }

    /// **The negativity fact, in closed form.** For `a_1 = 1/2` the series
    /// `1/2 + (3/4) mu` is negative below `mu = -2/3`, and the positive part
    /// renormalised has mean exactly `4/9`.
    ///
    /// Derivation (all integrals over `[-2/3, 1]`):
    /// `int f = 25/24`, `int mu f = 25/54`, ratio `= 24/54 = 4/9`.
    ///
    /// This is the reference the statistical gate below is judged against, and
    /// it is derived from the kernel rather than read off a run.
    #[test]
    fn the_truncated_mean_matches_its_closed_form() {
        let k = LegendreKernel::new(vec![1.0, 0.5]).unwrap();
        assert!(k.goes_negative(), "1/2 + (3/4) mu is negative below -2/3");
        let support = k.positive_support();
        assert_eq!(support.len(), 1, "{support:?}");
        assert!(
            (support[0].0 - (-2.0 / 3.0)).abs() < 1e-14,
            "root at {} not -2/3",
            support[0].0
        );
        assert!((support[0].1 - 1.0).abs() < 1e-15);
        let want = 4.0 / 9.0;
        assert!(
            (k.sampled_mean_cosine() - want).abs() < 1e-13,
            "sampled_mean_cosine {} != 4/9 = {want}",
            k.sampled_mean_cosine()
        );
        // The declared moment is still 0.5 — the two functions must not have
        // been quietly collapsed into one.
        assert!((k.mean_cosine() - 0.5).abs() < 1e-15);
    }

    /// A kernel that stays positive has no truncation, so the two mean-cosine
    /// functions must agree to quadrature precision.
    #[test]
    fn a_positive_kernel_samples_the_mean_it_declares() {
        for coeffs in [
            vec![1.0, 0.3],
            vec![1.0, -0.3],
            vec![1.0, 1.0 / 3.0],
            vec![1.0, 0.1, 0.05],
        ] {
            let k = LegendreKernel::new(coeffs.clone()).unwrap();
            assert!(!k.goes_negative(), "{coeffs:?} unexpectedly goes negative");
            let (declared, sampled) = (k.mean_cosine(), k.sampled_mean_cosine());
            assert!(
                (declared - sampled).abs() < 1e-12,
                "{coeffs:?}: declared {declared} vs sampled {sampled}"
            );
        }
    }

    /// **The load-bearing statistical gate**: rejection sampling reproduces
    /// the mean cosine of the distribution it actually samples.
    ///
    /// Isotropic sampling gives `<mu> = 0` for every one of these kernels,
    /// which is the defect #265 exists to remove — so this fails loudly
    /// against the status quo as well as against a broken sampler.
    ///
    /// # Results, 2026-09-22 (400 000 samples per kernel, `--release`)
    ///
    /// | `a_1` | declared | reference (truncated) | sampled | dev |
    /// |---|---|---|---|---|
    /// | 0.0 | 0.0 | 0.0 | see run output | < 4 sigma |
    /// | 0.2 | 0.2 | 0.2 | ” | ” |
    /// | 0.5 | 0.5 | **0.444444** | ” | ” |
    /// | −0.3 | −0.3 | −0.3 | ” | ” |
    ///
    /// The `a_1 = 0.5` row is why this gate is written against
    /// [`LegendreKernel::sampled_mean_cosine`]. Written against
    /// `mean_cosine` it missed by **35 sigma**, and the failure was the gate's,
    /// not the sampler's.
    #[test]
    fn sampling_reproduces_the_mean_cosine_that_is_actually_sampled() {
        const N: usize = 400_000;
        for a1 in [0.0, 0.2, 0.5, -0.3] {
            let k = LegendreKernel::new(vec![1.0, a1]).unwrap();
            let want = k.sampled_mean_cosine();
            let mut seed = 0x5EED_0265_u64;
            let mut sum = 0.0;
            for _ in 0..N {
                sum += k.sample_mu(&mut seed).unwrap();
            }
            let measured = sum / N as f64;
            // Var(mu) <= 1, so the mean's sigma is at most 1/sqrt(N).
            let sigma = 1.0 / (N as f64).sqrt();
            let dev = (measured - want).abs() / sigma;
            println!(
                "a1 = {a1:+.2}: declared <mu> = {:+.6}, truncated reference = {want:+.6}, \
                 sampled = {measured:+.6} ({dev:.2} sigma)",
                k.mean_cosine()
            );
            assert!(
                dev < 4.0,
                "sampled <mu> = {measured:+.6} against the truncated reference \
                 {want:+.6} ({dev:.2} sigma). Isotropic sampling would give 0 for \
                 every one of these."
            );
        }
    }

    /// A kernel that is negative everywhere is refused rather than spinning to
    /// MAX_SAMPLE at run time.
    #[test]
    fn an_unsamplable_kernel_is_refused_at_construction() {
        let err = LegendreKernel::new(vec![-1.0]).unwrap_err();
        assert!(err.contains("non-positive"), "{err}");
        assert!(LegendreKernel::new(vec![]).is_err());
    }

    /// Sampling must stay inside `[-1, 1]`.
    #[test]
    fn sampled_cosines_are_physical() {
        let k = LegendreKernel::new(vec![1.0, 0.6, 0.25]).unwrap();
        let mut seed = 99;
        for _ in 0..50_000 {
            let mu = k.sample_mu(&mut seed).unwrap();
            assert!((-1.0..=1.0).contains(&mu), "mu = {mu}");
        }
        let t = TabularKernel::from_legendre_default(&k).unwrap();
        let mut seed = 99;
        for _ in 0..50_000 {
            let mu = t.sample_mu(&mut seed);
            assert!((-1.0..=1.0).contains(&mu), "tabular mu = {mu}");
        }
    }

    /// The conversion's grid, normalisation and defaults follow upstream.
    #[test]
    fn the_tabular_conversion_reproduces_upstreams_grid_and_normalisation() {
        let p0 = LegendreKernel::new(vec![1.0]).unwrap();
        let t0 = TabularKernel::from_legendre_default(&p0).unwrap();
        assert_eq!(t0.mu.len(), 2, "P0 converts onto 2 points (`:843`)");

        let p3 = LegendreKernel::new(vec![1.0, 0.3, 0.2, 0.1]).unwrap();
        let t3 = TabularKernel::from_legendre_default(&p3).unwrap();
        assert_eq!(t3.mu.len(), DEFAULT_NMU);
        assert!((t3.mu[0] + 1.0).abs() < 1e-15);
        assert!((t3.mu[DEFAULT_NMU - 1] - 1.0).abs() < 1e-15);
        assert_eq!(t3.cdf[0], 0.0);
        assert!(
            (t3.cdf[DEFAULT_NMU - 1] - 1.0).abs() < 1e-14,
            "cdf ends at {}",
            t3.cdf[DEFAULT_NMU - 1]
        );
        assert!(t3.cdf.windows(2).all(|w| w[1] >= w[0]), "CDF must be monotone");
        assert!(t3.fmu.iter().all(|&p| p >= 0.0), "clamped density must be >= 0");
        assert!(TabularKernel::from_legendre(&p3, 1).is_err());
    }

    /// **What the tabular grid costs.** The clamp lands on grid points, not on
    /// the true root, so a coarse grid samples a different distribution from
    /// the rejection path — measured rather than assumed away.
    ///
    /// # Results, 2026-09-22 — kernel `[1, 0.5]`, exact truncated `<mu>` = 4/9
    ///
    /// Printed by the test; the gate is that the error falls monotonically
    /// towards the exact value as the grid refines, and that 33 points (the
    /// upstream default) is already within 1 % of it.
    #[test]
    fn the_tabular_grid_costs_a_measurable_amount_of_mean_cosine() {
        let k = LegendreKernel::new(vec![1.0, 0.5]).unwrap();
        let exact = k.sampled_mean_cosine();
        let mut previous = f64::INFINITY;
        for n_mu in [9, 17, 33, 129, 1025] {
            let t = TabularKernel::from_legendre(&k, n_mu).unwrap();
            let err = (t.mean_cosine() - exact).abs();
            println!(
                "n_mu = {n_mu:5}: tabular <mu> = {:+.6} vs exact {exact:+.6} \
                 (error {err:.2e})",
                t.mean_cosine()
            );
            assert!(
                err < previous,
                "refining {n_mu} did not reduce the error ({err:.3e} vs {previous:.3e})"
            );
            previous = err;
            if n_mu == DEFAULT_NMU {
                assert!(
                    err / exact.abs() < 0.01,
                    "the default {DEFAULT_NMU}-point grid is {:.2} % off",
                    100.0 * err / exact.abs()
                );
            }
        }
        assert!(previous < 1e-5, "the finest grid is still {previous:.2e} off");
    }

    /// `max_order` truncation follows upstream's semantics, including the part
    /// that is easy to get wrong: dropping moments above P1 leaves the
    /// DECLARED mean cosine untouched, while P0 forces isotropy.
    #[test]
    fn max_order_truncation_keeps_a1_but_p0_forces_isotropy() {
        let k = LegendreKernel::new(vec![1.0, 0.3, 0.1, 0.03]).unwrap();
        assert_eq!(k.order(), 3);

        let p1 = k.truncated_to(1).unwrap();
        assert_eq!(p1.order(), 1);
        assert!(
            (p1.mean_cosine() - 0.3).abs() < 1e-15,
            "truncating to P1 must keep a_1"
        );

        let p0 = k.truncated_to(0).unwrap();
        assert_eq!(p0.order(), 0);
        assert_eq!(p0.mean_cosine(), 0.0, "P0 has no preferred direction");
        assert!(!p0.goes_negative());

        // Truncating above the carried order is a no-op, not an error.
        assert_eq!(k.truncated_to(9).unwrap().order(), 3);
    }

    /// Truncation is not free even when `a_1` survives: a P1 cut of a kernel
    /// with `a_1 > 1/3` goes negative where the full expansion did not, and the
    /// SAMPLED mean then falls below the declared one.
    ///
    /// # Result, 2026-09-22
    ///
    /// `[1, 0.5, 0.25, 0.1]` is positive on `[-1, 1]` and samples `<mu> = 0.5`.
    /// Its P1 truncation `[1, 0.5]` declares 0.5 and samples **4/9 = 0.4444**.
    /// So a `max_order = 1` setting costs 11.1 % of the transport correction on
    /// this kernel while leaving every reported moment unchanged.
    #[test]
    fn truncation_can_cost_sampled_mean_cosine_while_a1_is_untouched() {
        let full = LegendreKernel::new(vec![1.0, 0.5, 0.25, 0.1]).unwrap();
        assert!(!full.goes_negative(), "the P3 kernel stays positive");
        assert!((full.sampled_mean_cosine() - 0.5).abs() < 1e-12);

        let p1 = full.truncated_to(1).unwrap();
        assert!((p1.mean_cosine() - 0.5).abs() < 1e-15, "declared moment survives");
        assert!(p1.goes_negative(), "the P1 cut dips below zero");
        assert!(
            (p1.sampled_mean_cosine() - 4.0 / 9.0).abs() < 1e-12,
            "P1 cut samples {} not 4/9",
            p1.sampled_mean_cosine()
        );
    }

    /// The histogram representation: normalisation, the per-bin density, and
    /// that its sampling reproduces its own mean cosine.
    #[test]
    fn the_histogram_representation_normalises_and_samples_itself() {
        // Flat histogram: isotropic, density 1/2 in every bin.
        let flat = HistogramKernel::new(vec![1.0; 8]).unwrap();
        for p in &flat.fmu {
            assert!((p - 0.5).abs() < 1e-15, "flat density {p}");
        }
        assert!((flat.cdf[7] - 1.0).abs() < 1e-15);
        assert!(flat.mean_cosine().abs() < 1e-15);

        // A forward-peaked histogram from the P3 kernel.
        let leg = LegendreKernel::new(vec![1.0, 0.3, 0.1, 0.03]).unwrap();
        let h = HistogramKernel::from_legendre(&leg, 64).unwrap();
        assert!(h.cdf.windows(2).all(|w| w[1] >= w[0]), "CDF must be monotone");
        assert!((h.cdf[63] - 1.0).abs() < 1e-14);

        const N: usize = 300_000;
        let mut seed = 0x4151_0265_u64;
        let mut sum = 0.0;
        for _ in 0..N {
            let mu = h.sample_mu(&mut seed);
            assert!((-1.0..=1.0).contains(&mu), "mu = {mu}");
            sum += mu;
        }
        let measured = sum / N as f64;
        let want = h.mean_cosine();
        let sigma = 1.0 / (N as f64).sqrt();
        let dev = (measured - want).abs() / sigma;
        println!(
            "histogram(64): own <mu> = {want:+.6}, sampled {measured:+.6} \
             ({dev:.2} sigma); underlying Legendre <mu> = {:+.6}",
            leg.sampled_mean_cosine()
        );
        assert!(dev < 4.0, "histogram sampling is off by {dev:.2} sigma");
        // 64 bin centres resolve a P3 kernel well, but this is a bound the
        // test states rather than a claim of equality.
        assert!(
            (want - leg.sampled_mean_cosine()).abs() < 2.0e-3,
            "64-bin histogram <mu> {want} vs Legendre {}",
            leg.sampled_mean_cosine()
        );
    }

    /// Degenerate histograms are refused rather than building a CDF that
    /// cannot be sampled.
    #[test]
    fn degenerate_histograms_are_refused() {
        assert!(HistogramKernel::new(vec![]).is_err());
        assert!(HistogramKernel::new(vec![0.0; 4]).is_err());
        let err = HistogramKernel::new(vec![1.0, -0.5, 1.0]).unwrap_err();
        assert!(err.contains("cannot be negative"), "{err}");
    }

    /// The two samplers are independent implementations of the same
    /// distribution — rejection from a box against inversion of a CDF. On a
    /// fine grid they must agree statistically.
    #[test]
    fn rejection_and_inversion_sampling_agree_on_a_fine_grid() {
        const N: usize = 200_000;
        let k = LegendreKernel::new(vec![1.0, 0.5, 0.2]).unwrap();
        let t = TabularKernel::from_legendre(&k, 2049).unwrap();

        let mut seed = 0x1234_0265_u64;
        let mut rej = 0.0;
        for _ in 0..N {
            rej += k.sample_mu(&mut seed).unwrap();
        }
        let mut seed = 0x9876_0265_u64;
        let mut inv = 0.0;
        for _ in 0..N {
            inv += t.sample_mu(&mut seed);
        }
        let (rej, inv) = (rej / N as f64, inv / N as f64);
        // Independent streams, so the difference carries sqrt(2) sigma.
        let sigma = (2.0_f64 / N as f64).sqrt();
        let dev = (rej - inv).abs() / sigma;
        println!(
            "rejection <mu> = {rej:+.6}, inversion <mu> = {inv:+.6} ({dev:.2} sigma), \
             exact {:+.6}",
            k.sampled_mean_cosine()
        );
        assert!(dev < 4.0, "the two samplers disagree at {dev:.2} sigma");
    }
}
