// SPDX-License-Identifier: AGPL-3.0-only
//
// PORT of STRATO's analytical Sturm-Liouville solver.
//   Upstream project: STRATO, https://github.com/weewzr/STRATO
//   Upstream file:    src/analytical/sturm_liouville.rs
//   Upstream commit:  36f1a98848bec9d66fe515994e075cff09d6cefb
//   Author:           Wee Zhern Ray (GitHub: weewzr)
//   Licence:          AGPL-3.0-only (upstream and this port)
//   Ported:           2026-11-26, into boon-lay (`boon_lay::strato`)
//
// Every upstream function keeps its name, arguments, f64 SI arithmetic and
// order of operations. The local changes are numbered in `strato/mod.rs`
// (L1 to L7); each one is marked `LOCAL CHANGE Ln` where it occurs below.

//! Code C: analytical approach. Sturm-Liouville eigenfunction series for the layered
//! sphere. Nothing in this module calls the finite-volume solver.
//!
//! *Ported from STRATO by Wee Zhern Ray (weewzr), commit `36f1a98`; see the
//! [parent module](super) for provenance and the list of local changes.*
//!
//! Two independent pieces live here:
//!
//! 1. A homogeneous Robin-sphere eigenvalue finder (`robin_residual`, `robin_roots`).
//! 2. The exact eigenfunction expansion of the five-layer initial-inventory release
//!    problem (initial kernel concentration, no source, absorbing outer surface).
//!    This is the same physical problem solved by the FV code, so the two can be
//!    compared directly.
//!
//! Modal formulation (u = r*phi, k_i^2 = Lambda/D_i):
//!   u_i'' + k_i^2 u_i = 0,
//!   u continuous and D (u' - u/r) continuous at every interface,
//!   u(0) = 0 (regular centre), u(R) = 0 (absorbing outer surface).
//! Orthogonality weight is r^2 for phi, i.e. weight 1 for u, so the release fraction is
//!   F(t) = 1 - sum_n w_n exp(-Lambda_n t),
//!   w_n  = 3 I1_n In_n / (R1^3 N_n),
//!   I1_n = int_0^R1 r u_n dr, In_n = int_0^R r u_n dr, N_n = int_0^R u_n^2 dr.
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
// LOCAL CHANGE L8: transcendentals through PETIR (see `strato/mod.rs`).
use petir::mathf::RealMath;
use core::f64::consts::PI;
use uom::si::diffusion_coefficient::square_meter_per_second;
use uom::si::f64::{DiffusionCoefficient, Length, Ratio, Time};
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::time::second;

/// LOCAL CHANGE L1: replaces upstream's `anyhow::bail!` errors. One variant per
/// upstream `bail!` site; `Display` prints upstream's message text exactly.
#[derive(Clone, Debug, PartialEq)]
pub enum StratoError {
    /// Upstream: "at least two layers are required".
    TooFewLayers,
    /// Upstream: "layers need increasing finite radii and positive finite diffusivities".
    InvalidLayers,
    /// Upstream: "lambda_max and max_modes must be positive".
    InvalidTruncation,
    /// Upstream: "could not bracket the lowest eigenvalue".
    CouldNotBracketLowestEigenvalue,
    /// Upstream: "unresolved eigenvalue cluster near Lambda = {b:.6e}".
    UnresolvedEigenvalueCluster {
        /// Upper end of the unresolved bracket (1/s).
        lambda: f64,
    },
    /// Upstream: "non-finite modal weight at Lambda = {lam:.6e}".
    NonFiniteModalWeight {
        /// Eigenvalue whose weight was not finite (1/s).
        lambda: f64,
    },
    /// Upstream: "need at least one positive time".
    NoPositiveTime,
}

impl core::fmt::Display for StratoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            StratoError::TooFewLayers => write!(f, "at least two layers are required"),
            StratoError::InvalidLayers => write!(
                f,
                "layers need increasing finite radii and positive finite diffusivities"
            ),
            StratoError::InvalidTruncation => {
                write!(f, "lambda_max and max_modes must be positive")
            }
            StratoError::CouldNotBracketLowestEigenvalue => {
                write!(f, "could not bracket the lowest eigenvalue")
            }
            StratoError::UnresolvedEigenvalueCluster { lambda } => {
                write!(f, "unresolved eigenvalue cluster near Lambda = {lambda:.6e}")
            }
            StratoError::NonFiniteModalWeight { lambda } => {
                write!(f, "non-finite modal weight at Lambda = {lambda:.6e}")
            }
            StratoError::NoPositiveTime => write!(f, "need at least one positive time"),
        }
    }
}

/// LOCAL CHANGE L1: the module's result type (upstream: `anyhow::Result`).
pub type Result<T> = core::result::Result<T, StratoError>;

/// One homogeneous spherical layer: outer radius (m) and diffusivity (m^2/s).
#[derive(Clone, Copy, Debug)]
pub struct Layer {
    pub outer_radius: f64,
    pub diffusivity: f64,
}

impl Layer {
    /// LOCAL CHANGE L4 (boon-lay addition, not upstream): build a layer from
    /// `uom` quantities. Stores the SI values (m, m^2/s) in upstream's fields.
    pub fn new(outer_radius: Length, diffusivity: DiffusionCoefficient) -> Self {
        Layer {
            outer_radius: outer_radius.get::<meter>(),
            diffusivity: diffusivity.get::<square_meter_per_second>(),
        }
    }
}

/// Dimensionless Robin-sphere eigenvalue residual for phi=sin(z r/R)/r.
/// Boundary: -D phi'(R)=h phi(R), Bi=hR/D.
/// Root equation: z cot(z) = 1 - Bi.
pub fn robin_residual(z: f64, bi: f64) -> f64 {
    z / z.r_tan() - (1.0 - bi)
}

/// Locate positive Robin eigenvalues by bracket scanning and bisection.
/// Poles of cot(z) are skipped by scanning finite sign changes.
pub fn robin_roots(bi: f64, count: usize) -> Vec<f64> {
    let mut roots = Vec::new();
    let eps = 1e-8;
    for n in 0..(count + 4) {
        let lo = n as f64 * PI + eps;
        let hi = (n as f64 + 1.0) * PI - eps;
        let segments = 128;
        let mut x0 = lo;
        let mut f0 = robin_residual(x0, bi);
        for k in 1..=segments {
            let x1 = lo + (hi - lo) * k as f64 / segments as f64;
            let f1 = robin_residual(x1, bi);
            if f0.is_finite() && f1.is_finite() && f0 * f1 < 0.0 {
                let mut a = x0;
                let mut b = x1;
                let mut fa = f0;
                for _ in 0..80 {
                    let m = 0.5 * (a + b);
                    let fm = robin_residual(m, bi);
                    if fa * fm <= 0.0 {
                        b = m;
                    } else {
                        a = m;
                        fa = fm;
                    }
                }
                let root = 0.5 * (a + b);
                if root > 1e-6 && roots.last().is_none_or(|x: &f64| (root - *x).abs() > 1e-6) {
                    roots.push(root);
                    if roots.len() == count {
                        return roots;
                    }
                }
            }
            x0 = x1;
            f0 = f1;
        }
    }
    roots
}

/// Propagate the two-component state (u, q = D u') through ONE homogeneous layer
/// of thickness `length` for u'' + k^2 u = 0.
///
/// This is only the within-layer step. The state (u, D u') is NOT continuous across a
/// material interface when the diffusivities differ; use `interface_state` there.
pub fn transfer_layer(u: f64, du_flux: f64, d: f64, k: f64, length: f64) -> (f64, f64) {
    let c = (k * length).r_cos();
    let s = (k * length).r_sin();
    let u2 = c * u + s * du_flux / (d * k);
    let q2 = -d * k * s * u + c * du_flux;
    (u2, q2)
}

/// Interface rule for the state q = D u' (u = r*phi).
/// Concentration continuity gives u continuous. Flux continuity D phi' continuous with
/// phi' = (u' - u/r)/r gives D (u' - u/r) continuous, hence
///   q_outer = q_inner + (D_outer - D_inner) u / r.
pub fn interface_state(q_inner: f64, u: f64, d_inner: f64, d_outer: f64, radius: f64) -> f64 {
    q_inner + (d_outer - d_inner) * u / radius
}

#[derive(Clone, Copy, Debug)]
struct LayerStart {
    u: f64,
    q: f64,
    k: f64,
    d: f64,
    left: f64,
    right: f64,
}

/// Upstream's layer check: at least two layers, increasing finite radii and
/// positive finite diffusivities.
pub fn validate_layers(layers: &[Layer]) -> Result<()> {
    if layers.len() < 2 {
        return Err(StratoError::TooFewLayers);
    }
    let mut last = 0.0;
    for l in layers {
        if !(l.outer_radius.is_finite() && l.diffusivity.is_finite())
            || l.outer_radius <= last
            || l.diffusivity <= 0.0
        {
            return Err(StratoError::InvalidLayers);
        }
        last = l.outer_radius;
    }
    Ok(())
}

/// LOCAL CHANGE L5 (boon-lay addition, not upstream): the one-layer check
/// used by [`ModalRelease::single_sphere`]. Upstream's per-layer predicate
/// (with `last = 0`), without the two-layer requirement.
fn validate_single_layer(layer: &Layer) -> Result<()> {
    let last = 0.0;
    if !(layer.outer_radius.is_finite() && layer.diffusivity.is_finite())
        || layer.outer_radius <= last
        || layer.diffusivity <= 0.0
    {
        return Err(StratoError::InvalidLayers);
    }
    Ok(())
}

/// Shoot the regular solution from the centre at eigenvalue `lambda`.
/// Returns u(R) and the state at the start of each layer. With `rescale` the running
/// state is renormalised after each layer (fine for counting, not for integrals).
fn shoot(lambda: f64, layers: &[Layer], rescale: bool) -> (f64, Vec<LayerStart>) {
    let mut starts = Vec::with_capacity(layers.len());
    let (mut u, mut q) = (0.0, 1.0);
    let mut left = 0.0;
    let mut prev_d: Option<f64> = None;
    for l in layers {
        if let Some(pd) = prev_d {
            q = interface_state(q, u, pd, l.diffusivity, left);
        }
        let k = (lambda / l.diffusivity).sqrt();
        starts.push(LayerStart {
            u,
            q,
            k,
            d: l.diffusivity,
            left,
            right: l.outer_radius,
        });
        (u, q) = transfer_layer(u, q, l.diffusivity, k, l.outer_radius - left);
        if rescale {
            let m = u.abs().max((q / (l.diffusivity * k)).abs());
            if m > 0.0 && m.is_finite() {
                u /= m;
                q /= m;
            }
        }
        left = l.outer_radius;
        prev_d = Some(l.diffusivity);
    }
    (u, starts)
}

/// Sturm oscillation count: number of eigenvalues Lambda_n <= `lambda`, equal to the
/// number of zeros of the regular solution in (0, R].
pub fn eigenvalue_count(lambda: f64, layers: &[Layer]) -> usize {
    let mut n: i64 = 0;
    let (mut u, mut q) = (0.0, 1.0);
    let mut left = 0.0;
    let mut prev_d: Option<f64> = None;
    for l in layers {
        if let Some(pd) = prev_d {
            q = interface_state(q, u, pd, l.diffusivity, left);
        }
        let k = (lambda / l.diffusivity).sqrt();
        let a = u;
        let b = q / (l.diffusivity * k);
        // u(s) = A sin(k s + theta0); zeros at k s + theta0 = m pi for s in (0, L].
        let theta0 = a.r_atan2(b);
        let length = l.outer_radius - left;
        n += ((theta0 + k * length) / PI).floor() as i64 - (theta0 / PI).floor() as i64;
        (u, q) = transfer_layer(u, q, l.diffusivity, k, length);
        let m = u.abs().max((q / (l.diffusivity * k)).abs());
        if m > 0.0 && m.is_finite() {
            u /= m;
            q /= m;
        }
        left = l.outer_radius;
        prev_d = Some(l.diffusivity);
    }
    n.max(0) as usize
}

/// All eigenvalues up to `lambda_max`, capped at `max_modes`.
/// Returns the sorted eigenvalues and the effective upper bound actually covered.
/// Roots are isolated by bisection on the Sturm count, so no root can be skipped.
pub fn eigenvalues(layers: &[Layer], lambda_max: f64, max_modes: usize) -> Result<(Vec<f64>, f64)> {
    validate_layers(layers)?;
    eigenvalues_of_validated_layers(layers, lambda_max, max_modes)
}

/// LOCAL CHANGE L5: upstream's `eigenvalues` body after `validate_layers`,
/// split out unchanged so that [`ModalRelease::single_sphere`] can run it on
/// one layer. [`eigenvalues`] calls it right after `validate_layers`, so the
/// upstream path is the same sequence of operations.
fn eigenvalues_of_validated_layers(
    layers: &[Layer],
    lambda_max: f64,
    max_modes: usize,
) -> Result<(Vec<f64>, f64)> {
    if !(lambda_max > 0.0 && lambda_max.is_finite()) || max_modes == 0 {
        return Err(StratoError::InvalidTruncation);
    }
    let mut hi = lambda_max;
    if eigenvalue_count(hi, layers) > max_modes {
        let mut lo = hi * 1e-12;
        for _ in 0..200 {
            let mid = (lo * hi).sqrt();
            if eigenvalue_count(mid, layers) > max_modes {
                hi = mid;
            } else {
                lo = mid;
            }
            if hi - lo <= 1e-12 * hi {
                break;
            }
        }
        hi = lo;
    }
    let n_hi = eigenvalue_count(hi, layers);

    // Lower bound with zero eigenvalues below it.
    let phase_rate: f64 = {
        let mut left = 0.0;
        layers
            .iter()
            .map(|l| {
                let v = (l.outer_radius - left) / l.diffusivity.sqrt();
                left = l.outer_radius;
                v
            })
            .sum()
    };
    let mut lo = 1e-6 * (1.0 / phase_rate).r_powi(2);
    while eigenvalue_count(lo, layers) != 0 {
        lo *= 0.01;
        if lo < 1e-300 {
            return Err(StratoError::CouldNotBracketLowestEigenvalue);
        }
    }

    let mut roots = Vec::with_capacity(n_hi);
    let mut stack = vec![(lo, hi, 0usize, n_hi)];
    while let Some((a, b, na, nb)) = stack.pop() {
        if nb <= na {
            continue;
        }
        if b - a <= 4e-15 * b {
            if nb - na > 1 {
                return Err(StratoError::UnresolvedEigenvalueCluster { lambda: b });
            }
            roots.push(0.5 * (a + b));
            continue;
        }
        let m = if b > 4.0 * a {
            (a * b).sqrt()
        } else {
            0.5 * (a + b)
        };
        let nm = eigenvalue_count(m, layers).clamp(na, nb);
        stack.push((a, m, na, nm));
        stack.push((m, b, nm, nb));
    }
    // Panic justification (upstream's): every root is a bisection midpoint of
    // finite brackets, so `partial_cmp` cannot meet a NaN.
    roots.sort_by(|x, y| x.partial_cmp(y).expect("finite roots"));
    Ok((roots, hi))
}

fn gauss_legendre_16() -> ([f64; 16], [f64; 16]) {
    let n = 16usize;
    let mut x = [0.0; 16];
    let mut w = [0.0; 16];
    for i in 0..n {
        let mut z = ((i as f64 + 0.75) / (n as f64 + 0.5) * PI).r_cos();
        let mut dp = 1.0;
        for _ in 0..100 {
            let (mut p_prev, mut p) = (1.0, z);
            for k in 2..=n {
                let kf = k as f64;
                let pk = ((2.0 * kf - 1.0) * z * p - (kf - 1.0) * p_prev) / kf;
                p_prev = p;
                p = pk;
            }
            dp = n as f64 * (z * p - p_prev) / (z * z - 1.0);
            let dz = p / dp;
            z -= dz;
            if dz.abs() < 1e-15 {
                break;
            }
        }
        x[i] = z;
        w[i] = 2.0 / ((1.0 - z * z) * dp * dp);
    }
    (x, w)
}

fn integrate(a: f64, b: f64, panels: usize, f: impl Fn(f64) -> f64) -> f64 {
    let (x, w) = gauss_legendre_16();
    let h = (b - a) / panels as f64;
    let mut total = 0.0;
    for p in 0..panels {
        let lo = a + p as f64 * h;
        let (mid, half) = (lo + 0.5 * h, 0.5 * h);
        total += half * (0..16).map(|i| w[i] * f(mid + half * x[i])).sum::<f64>();
    }
    total
}

/// Exact five-layer (or any N >= 2 layer) release series for the
/// initial-kernel-inventory, absorbing-outer-surface problem.
///
/// boon-lay: [`ModalRelease::single_sphere`] (LOCAL CHANGE L5) builds the
/// same series for N = 1, which is Booth's uniform sphere.
#[derive(Clone, Debug)]
pub struct ModalRelease {
    pub eigenvalues: Vec<f64>,
    pub weights: Vec<f64>,
    /// Largest eigenvalue covered by the series (1/s).
    pub lambda_covered: f64,
    /// Smallest value, over all modes and interfaces, of
    /// |D (u' - u/r)| / (|D u'| + |D u / r|) in the shooting solution. The physical flux is
    /// a difference of two terms of that size, so about eps / this ratio of relative
    /// accuracy is lost. See `accuracy_floor`.
    pub min_flux_ratio: f64,
}

impl ModalRelease {
    /// Build the series from all eigenvalues up to `lambda_max` (capped at `max_modes`).
    pub fn new(layers: &[Layer], lambda_max: f64, max_modes: usize) -> Result<Self> {
        validate_layers(layers)?;
        Self::from_validated_layers(layers, lambda_max, max_modes)
    }

    /// LOCAL CHANGE L5 (boon-lay addition, NOT upstream): the series for ONE
    /// homogeneous sphere of radius `outer_radius_m` (m) and diffusivity
    /// `diffusivity_m2_s` (m^2/s), with a uniform initial concentration
    /// (R1 = R, so the "kernel" is the whole sphere): Booth's sphere,
    /// `F = 1 - (6/pi^2) sum 1/n^2 exp(-n^2 pi^2 D t / R^2)`.
    ///
    /// Runs exactly the code path of [`ModalRelease::new`] (shooting, Sturm
    /// counting, eigenvalue bisection, Gauss-Legendre weights) on a one-layer
    /// list, skipping only upstream's two-layer check. With one layer there
    /// is no interface, so `min_flux_ratio` stays `+inf` and
    /// [`accuracy_floor`](Self::accuracy_floor) is 2.2e-16.
    pub fn single_sphere(
        outer_radius_m: f64,
        diffusivity_m2_s: f64,
        lambda_max: f64,
        max_modes: usize,
    ) -> Result<Self> {
        let layers = [Layer {
            outer_radius: outer_radius_m,
            diffusivity: diffusivity_m2_s,
        }];
        validate_single_layer(&layers[0])?;
        Self::from_validated_layers(&layers, lambda_max, max_modes)
    }

    /// LOCAL CHANGE L5: upstream's `ModalRelease::new` body, unchanged, on
    /// layers already validated by the caller (`new` or `single_sphere`).
    fn from_validated_layers(layers: &[Layer], lambda_max: f64, max_modes: usize) -> Result<Self> {
        let (lams, lambda_covered) = eigenvalues_of_validated_layers(layers, lambda_max, max_modes)?;
        let r1 = layers[0].outer_radius;
        let mut weights = Vec::with_capacity(lams.len());
        let mut min_flux_ratio = f64::INFINITY;
        for &lam in &lams {
            let (_, starts) = shoot(lam, layers, false);
            for st in starts.iter().skip(1) {
                let scale = st.q.abs() + (st.d * st.u / st.left).abs();
                if scale > 0.0 {
                    let flux = (st.q - st.d * st.u / st.left).abs();
                    min_flux_ratio = min_flux_ratio.min(flux / scale);
                }
            }
            let (mut norm, mut moment, mut i1) = (0.0, 0.0, 0.0);
            for (idx, st) in starts.iter().enumerate() {
                let (a, b) = (st.u, st.q / (st.d * st.k));
                let panels = ((st.k * (st.right - st.left)) / (0.5 * PI)).ceil() as usize + 1;
                let u_at = |r: f64| {
                    let s = r - st.left;
                    a * (st.k * s).r_cos() + b * (st.k * s).r_sin()
                };
                norm += integrate(st.left, st.right, panels, |r| u_at(r).r_powi(2));
                let m = integrate(st.left, st.right, panels, |r| r * u_at(r));
                moment += m;
                if idx == 0 {
                    i1 = m;
                }
            }
            let w = 3.0 * i1 * moment / (r1.r_powi(3) * norm);
            if !w.is_finite() {
                return Err(StratoError::NonFiniteModalWeight { lambda: lam });
            }
            weights.push(w);
        }
        Ok(Self {
            eigenvalues: lams,
            weights,
            lambda_covered,
            min_flux_ratio,
        })
    }

    /// Estimated absolute accuracy of `release_fraction`, about 2.2e-16 / `min_flux_ratio`
    /// (at least 2.2e-16). Cancellation in the shooting solution is the dominant error
    /// when adjacent layers differ strongly in diffusivity. Treat differences between FV and
    /// the series below this level as unresolved. Measured against FV on test inputs, the
    /// true error stayed about 10 to 20 times below this estimate.
    pub fn accuracy_floor(&self) -> f64 {
        2.2e-16 / self.min_flux_ratio.min(1.0)
    }

    /// Smallest time for which the omitted modes (Lambda > covered) contribute
    /// less than exp(-40) of their weight.
    pub fn min_valid_time(&self) -> f64 {
        40.0 / self.lambda_covered
    }

    /// Fraction of the initial kernel inventory still inside the particle at time t.
    pub fn remaining_fraction(&self, t: f64) -> f64 {
        self.eigenvalues
            .iter()
            .zip(&self.weights)
            .map(|(l, w)| w * (-l * t).r_exp())
            .sum()
    }

    /// Cumulative released fraction F(t) = 1 - sum_n w_n exp(-Lambda_n t).
    pub fn release_fraction(&self, t: f64) -> f64 {
        1.0 - self.remaining_fraction(t)
    }

    /// LOCAL CHANGE L4 (boon-lay addition, not upstream): `release_fraction`
    /// at a `uom` time. `None` when `t < min_valid_time()`, where the omitted
    /// modes are not negligible: the same rule upstream's [`exact_release`]
    /// applies (`(t >= valid).then(..)`). Otherwise exactly
    /// `release_fraction(t in s)`.
    pub fn release_fraction_at(&self, t: Time) -> Option<Ratio> {
        let t = t.get::<second>();
        let valid = self.min_valid_time();
        (t >= valid).then(|| Ratio::new::<ratio>(self.release_fraction(t)))
    }
}

/// Convenience: exact release fraction at the given times (all must be > 0).
/// The series is truncated so that exp(-45) of the omitted modes remains at the
/// earliest time; `max_modes` bounds the cost.
pub fn exact_release(
    layers: &[Layer],
    times: &[f64],
    max_modes: usize,
) -> Result<Vec<Option<f64>>> {
    let t_min = times
        .iter()
        .copied()
        .filter(|t| *t > 0.0)
        .fold(f64::INFINITY, f64::min);
    if !t_min.is_finite() {
        return Err(StratoError::NoPositiveTime);
    }
    let series = ModalRelease::new(layers, 45.0 / t_min, max_modes)?;
    let valid = series.min_valid_time();
    Ok(times
        .iter()
        .map(|&t| (t >= valid).then(|| series.release_fraction(t)))
        .collect())
}
