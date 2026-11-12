// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors.
//
// Ported from OpenMC (MIT licence), commit 8f79d9d02483c90e4134f03a9df4b75ce6b062bf:
//   src/distribution.cpp, include/openmc/distribution.h (Distribution, DiscreteIndex,
//   Discrete, Uniform, PowerLaw, Maxwell, Watt, Normal, Tabular, Mixture),
//   src/math_functions.cpp (exprel, log1prel, standard_normal_cdf),
//   src/distribution_spatial.cpp (CartesianIndependent, CylindricalIndependent,
//   SphericalIndependent, PointCloud, SpatialBox, SpatialPoint).

//! **OpenMC's univariate and spatial source distributions** (gh:#1388, epic #257).
//!
//! # What this adds
//!
//! The crate's energy sources were four fixed shapes ([`super::energy`]); OpenMC's
//! `openmc.stats` has a closed family of univariate distributions that can be
//! composed into any source coordinate. This module is that family, as one enum:
//!
//! | OpenMC class | here | draws per sample |
//! |---|---|---|
//! | `Discrete` (many lines, **Vose alias**) | [`Distribution::Discrete`] | 2 (0 if one line) |
//! | `Uniform` | [`Distribution::Uniform`] | 1 |
//! | `PowerLaw` | [`Distribution::PowerLaw`] | 1 |
//! | `Maxwell`, `Watt` | [`Distribution::Maxwell`], [`Distribution::Watt`] | as `random_dist.cpp` |
//! | `Normal` (optionally truncated) | [`Distribution::Normal`] | Marsaglia polar, rejection if truncated |
//! | `Tabular` (histogram, lin-lin, log-lin, log-log) | [`Distribution::Tabular`] | 1 |
//! | `Mixture` | [`Distribution::Mixture`] | 2 + component |
//!
//! and the spatial family [`SpatialDistribution`]: `CartesianIndependent`,
//! `CylindricalIndependent`, `SphericalIndependent`, `PointCloud`, `SpatialBox`,
//! `SpatialPoint`. Every `sample` returns `(value, weight)` as upstream does, so
//! biased sampling (`bias`) carries its importance weight.
//!
//! # Not ported, and why
//!
//! - `Legendre` is in `openmc.stats` (Python) only: its `sample` raises
//!   `NotImplementedError` upstream. [`Legendre`] therefore evaluates the density
//!   and nothing more.
//! - `DecaySpectrum` needs the depletion chain (pairs with gh:#975).
//! - `MeshSpatial` is [`super::extra::MeshSource`] (gh:#264).
//! - `Equiprobable`: upstream's `sample_unbiased` reads `x_[i + i]` where `x_[i + 1]`
//!   is meant, so it is wrong for `i > 0`; not ported rather than copying a defect.
//! - `Muir` is a Python helper that builds a [`Distribution::Normal`].
//!
//! # Verification
//!
//! `tests/source_distributions_vs_openmc_code_to_code.rs` against OpenMC's real
//! sources: see `verification_and_validation/upstream_openmc_samplers/README.md`.

use std::sync::Arc;

use crate::mathf::RealMath;
use crate::rng::distributions::{maxwell, normal_variate, watt};
use crate::rng::lcg::prn;

const INFTY: f64 = f64::MAX;
const PI: f64 = std::f64::consts::PI;

/// `compute_importance_weights` (`distribution.cpp:34`): `p_norm / b_norm`, or
/// `INFTY` where the bias probability is zero.
pub fn compute_importance_weights(p: &[f64], b: &[f64]) -> Vec<f64> {
    let sum_p: f64 = p.iter().fold(0.0, |a, &x| a + x);
    let sum_b: f64 = b.iter().fold(0.0, |a, &x| a + x);
    p.iter()
        .zip(b.iter())
        .map(|(&pi, &bi)| {
            let bn = bi / sum_b;
            if bn == 0.0 { INFTY } else { (pi / sum_p) / bn }
        })
        .collect()
}

/// `exprel(x) = expm1(x)/x` (`math_functions.cpp:931`).
pub fn exprel(x: f64) -> f64 {
    if x.abs() < 1e-16 { 1.0 } else { x.r_exp_m1() / x }
}

/// `log1prel(x) = log1p(x)/x` (`math_functions.cpp:940`).
pub fn log1prel(x: f64) -> f64 {
    if x.abs() < 1e-16 { 1.0 } else { x.r_ln_1p() / x }
}

/// `standard_normal_cdf(z) = 0.5 erfc(-z/sqrt 2)` (`math_functions.cpp:100`).
pub fn standard_normal_cdf(z: f64) -> f64 {
    0.5 * petir::real::erfc(-z / 2.0_f64.sqrt())
}

/// `DiscreteIndex` (`distribution.cpp:101`): Vose alias sampling over indices.
#[derive(Debug, Clone, PartialEq)]
pub struct DiscreteIndex {
    prob: Vec<f64>,
    alias: Vec<usize>,
    integral: f64,
}

impl DiscreteIndex {
    /// `DiscreteIndex::assign` + `init_alias` (normalise, then Vose's tables).
    pub fn new(p: &[f64]) -> Self {
        let integral: f64 = p.iter().fold(0.0, |a, &x| a + x);
        let mut prob: Vec<f64> = p.iter().map(|&x| x / integral).collect();
        let n = prob.len();
        let mut alias = vec![0usize; n];
        let mut large: Vec<usize> = Vec::new();
        let mut small: Vec<usize> = Vec::new();
        for i in 0..n {
            prob[i] *= n as f64;
            if prob[i] > 1.0 {
                large.push(i)
            } else {
                small.push(i)
            }
        }
        while !large.is_empty() && !small.is_empty() {
            let j = *small.last().unwrap();
            let k = *large.last().unwrap();
            small.pop();
            prob[k] += prob[j] - 1.0;
            alias[j] = k;
            if prob[k] < 1.0 {
                small.push(k);
                large.pop();
            }
        }
        Self { prob, alias, integral }
    }

    /// Sum of the unnormalised input (its "relative strength").
    pub fn integral(&self) -> f64 {
        self.integral
    }

    /// `DiscreteIndex::sample`: two `prn` draws, none for a single entry.
    pub fn sample(&self, seed: &mut u64) -> usize {
        let n = self.prob.len();
        if n > 1 {
            let u = (prn(seed) * n as f64) as usize;
            if prn(seed) < self.prob[u] { u } else { self.alias[u] }
        } else {
            0
        }
    }
}

/// Interpolation law of a [`Tabular`] distribution (`Interpolation` in OpenMC).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabularInterp {
    /// Piecewise constant.
    Histogram,
    /// Linear-linear.
    LinLin,
    /// Log in `p`, linear in `x`.
    LogLin,
    /// Log-log.
    LogLog,
}

/// `Tabular` (`distribution.cpp:498`).
#[derive(Debug, Clone, PartialEq)]
pub struct Tabular {
    x: Vec<f64>,
    p: Vec<f64>,
    c: Vec<f64>,
    interp: TabularInterp,
    integral: f64,
}

impl Tabular {
    /// `Tabular::init`: build the CDF by the interpolation law, then normalise.
    pub fn new(x: &[f64], p: &[f64], interp: TabularInterp) -> Self {
        let n = x.len();
        let x = x.to_vec();
        let mut p = p.to_vec();
        let mut c = vec![0.0; n];
        for i in 1..n {
            let dx = x[i] - x[i - 1];
            c[i] = match interp {
                TabularInterp::Histogram => c[i - 1] + p[i - 1] * dx,
                TabularInterp::LinLin => c[i - 1] + 0.5 * (p[i - 1] + p[i]) * dx,
                TabularInterp::LogLin => {
                    let m = (p[i] / p[i - 1]).r_ln() / dx;
                    c[i - 1] + p[i - 1] * dx * exprel(m * dx)
                }
                TabularInterp::LogLog => {
                    let m = ((x[i] * p[i]) / (x[i - 1] * p[i - 1])).r_ln() / (x[i] / x[i - 1]).r_ln();
                    c[i - 1]
                        + x[i - 1] * p[i - 1] * (x[i] / x[i - 1]).r_ln()
                            * exprel(m * (x[i] / x[i - 1]).r_ln())
                }
            };
        }
        let integral = c[n - 1];
        for i in 0..n {
            p[i] /= integral;
            c[i] /= integral;
        }
        Self { x, p, c, interp, integral }
    }

    fn sample_unbiased(&self, seed: &mut u64) -> f64 {
        let n = self.x.len();
        let c = prn(seed);
        // lower_bound over c_[1..] gives the first entry >= c; the bin is one below
        let mut i = self.c[1..].partition_point(|&v| v < c);
        if self.interp != TabularInterp::Histogram {
            i = i.min(n - 2);
        }
        let c_i = self.c[i];
        let x_i = self.x[i];
        let p_i = self.p[i];
        match self.interp {
            TabularInterp::Histogram => {
                if p_i > 0.0 { x_i + (c - c_i) / p_i } else { x_i }
            }
            TabularInterp::LinLin => {
                let (x_i1, p_i1) = (self.x[i + 1], self.p[i + 1]);
                let m = (p_i1 - p_i) / (x_i1 - x_i);
                if m == 0.0 {
                    x_i + (c - c_i) / p_i
                } else {
                    x_i + ((p_i * p_i + 2.0 * m * (c - c_i)).max(0.0).sqrt() - p_i) / m
                }
            }
            TabularInterp::LogLin => {
                let (x_i1, p_i1) = (self.x[i + 1], self.p[i + 1]);
                let m = (p_i1 / p_i).r_ln() / (x_i1 - x_i);
                let f = (c - c_i) / p_i;
                x_i + f * log1prel(m * f)
            }
            TabularInterp::LogLog => {
                let (x_i1, p_i1) = (self.x[i + 1], self.p[i + 1]);
                let m = ((x_i1 * p_i1) / (x_i * p_i)).r_ln() / (x_i1 / x_i).r_ln();
                let f = (c - c_i) / (p_i * x_i);
                x_i * (f * log1prel(m * f)).r_exp()
            }
        }
    }

    fn evaluate(&self, x: f64) -> f64 {
        if self.interp == TabularInterp::Histogram {
            let i = self.x.partition_point(|&v| v <= x) as isize - 1;
            if i < 0 || i >= self.p.len() as isize { 0.0 } else { self.p[i as usize] }
        } else {
            let i = self.x.partition_point(|&v| v < x) as isize - 1;
            if i < 0 || i >= self.p.len() as isize - 1 {
                0.0
            } else {
                let i = i as usize;
                let t = (x - self.x[i]) / (self.x[i + 1] - self.x[i]);
                (1.0 - t) * self.p[i] + t * self.p[i + 1]
            }
        }
    }
}

/// `Discrete` (`distribution.cpp:166`): lines `x` with alias sampling.
#[derive(Debug, Clone, PartialEq)]
pub struct Discrete {
    x: Vec<f64>,
    di: DiscreteIndex,
    /// Importance weights; empty when unbiased.
    weight: Vec<f64>,
}

/// `PowerLaw` (`distribution.cpp:253`): `f(x) ∝ x^n` on `[a, b]`.
#[derive(Debug, Clone, PartialEq)]
pub struct PowerLaw {
    offset: f64,
    span: f64,
    ninv: f64,
}

/// `Normal` (`distribution.cpp:343`), optionally truncated to `[lower, upper]`.
#[derive(Debug, Clone, PartialEq)]
pub struct Normal {
    mean: f64,
    std_dev: f64,
    lower: f64,
    upper: f64,
    is_truncated: bool,
    norm_factor: f64,
}

/// `Mixture` (`distribution.cpp:640`).
#[derive(Debug, Clone, PartialEq)]
pub struct Mixture {
    components: Vec<Distribution>,
    di: DiscreteIndex,
    weight: Vec<f64>,
    integral: f64,
}

/// OpenMC's univariate `Distribution` hierarchy as a closed enum (no `dyn`).
#[derive(Debug, Clone, PartialEq)]
pub enum Distribution {
    /// Multi-line discrete distribution.
    Discrete(Discrete),
    /// Uniform on `[a, b]`; optional bias.
    Uniform { a: f64, b: f64, bias: Option<Arc<Distribution>> },
    /// Power law; optional bias.
    PowerLaw { law: PowerLaw, bias: Option<Arc<Distribution>> },
    /// Maxwellian with temperature `theta`.
    Maxwell { theta: f64, bias: Option<Arc<Distribution>> },
    /// Watt spectrum.
    Watt { a: f64, b: f64, bias: Option<Arc<Distribution>> },
    /// Normal, optionally truncated.
    Normal { law: Normal, bias: Option<Arc<Distribution>> },
    /// Tabulated density.
    Tabular { law: Tabular, bias: Option<Arc<Distribution>> },
    /// Mixture of distributions.
    Mixture(Mixture),
}

impl Distribution {
    /// `Discrete(x, p, n)`: a single line is `Discrete::new(&[v], &[1.0])`.
    pub fn discrete(x: &[f64], p: &[f64]) -> Self {
        Distribution::Discrete(Discrete { x: x.to_vec(), di: DiscreteIndex::new(p), weight: Vec::new() })
    }

    /// Biased `Discrete`: sampled by `bias`, weighted `p_norm / bias_norm`.
    pub fn discrete_biased(x: &[f64], p: &[f64], bias: &[f64]) -> Self {
        Distribution::Discrete(Discrete {
            x: x.to_vec(),
            di: DiscreteIndex::new(bias),
            weight: compute_importance_weights(p, bias),
        })
    }

    /// `Uniform(a, b)`.
    pub fn uniform(a: f64, b: f64) -> Self {
        Distribution::Uniform { a, b, bias: None }
    }

    /// `PowerLaw(a, b, n)`.
    pub fn power_law(a: f64, b: f64, n: f64) -> Self {
        let offset = a.r_powf(n + 1.0);
        let span = b.r_powf(n + 1.0) - offset;
        Distribution::PowerLaw { law: PowerLaw { offset, span, ninv: 1.0 / (n + 1.0) }, bias: None }
    }

    /// `Maxwell(theta)`.
    pub fn maxwell(theta: f64) -> Self {
        Distribution::Maxwell { theta, bias: None }
    }

    /// `Watt(a, b)`.
    pub fn watt(a: f64, b: f64) -> Self {
        Distribution::Watt { a, b, bias: None }
    }

    /// `Normal(mean, std_dev, lower, upper)`; use `±f64::MAX` for no truncation
    /// (OpenMC's `INFTY`).
    ///
    /// # Errors
    ///
    /// As upstream's `fatal_error`s: `lower >= upper`, or bounds that exclude
    /// the whole distribution.
    pub fn normal(mean: f64, std_dev: f64, lower: f64, upper: f64) -> Result<Self, String> {
        if lower >= upper {
            return Err("Normal distribution lower bound must be less than upper bound.".into());
        }
        let is_truncated = lower > -INFTY || upper < INFTY;
        let norm_factor = if is_truncated {
            let alpha = (lower - mean) / std_dev;
            let beta = (upper - mean) / std_dev;
            let diff = standard_normal_cdf(beta) - standard_normal_cdf(alpha);
            if diff <= 0.0 {
                return Err("Normal distribution truncation bounds exclude entire distribution.".into());
            }
            1.0 / diff
        } else {
            1.0
        };
        Ok(Distribution::Normal {
            law: Normal { mean, std_dev, lower, upper, is_truncated, norm_factor },
            bias: None,
        })
    }

    /// `Tabular(x, p, n, interp)`.
    pub fn tabular(x: &[f64], p: &[f64], interp: TabularInterp) -> Self {
        Distribution::Tabular { law: Tabular::new(x, p, interp), bias: None }
    }

    /// `Mixture` of `(probability, distribution)` pairs; each probability is
    /// weighted by its component's [`Distribution::integral`] as upstream does.
    pub fn mixture(pairs: Vec<(f64, Distribution)>) -> Self {
        Self::mixture_inner(pairs, None)
    }

    /// Biased `Mixture`: components are chosen with `bias` probabilities.
    pub fn mixture_biased(pairs: Vec<(f64, Distribution)>, bias: &[f64]) -> Self {
        Self::mixture_inner(pairs, Some(bias))
    }

    fn mixture_inner(pairs: Vec<(f64, Distribution)>, bias: Option<&[f64]>) -> Self {
        let probs: Vec<f64> = pairs.iter().map(|(p, d)| p * d.integral()).collect();
        let integral = probs.iter().fold(0.0, |a, &x| a + x);
        let components: Vec<Distribution> = pairs.into_iter().map(|(_, d)| d).collect();
        let (di, weight) = match bias {
            Some(b) => (DiscreteIndex::new(b), compute_importance_weights(&probs, b)),
            None => (DiscreteIndex::new(&probs), Vec::new()),
        };
        Distribution::Mixture(Mixture { components, di, weight, integral })
    }

    /// Attach a bias distribution (`Distribution::set_bias`).
    ///
    /// # Errors
    ///
    /// For `Discrete` and `Mixture`, which are biased by their own constructors.
    pub fn with_bias(mut self, b: Distribution) -> Result<Self, String> {
        let slot = match &mut self {
            Distribution::Uniform { bias, .. }
            | Distribution::PowerLaw { bias, .. }
            | Distribution::Maxwell { bias, .. }
            | Distribution::Watt { bias, .. }
            | Distribution::Normal { bias, .. }
            | Distribution::Tabular { bias, .. } => bias,
            _ => return Err("Discrete and Mixture are biased by their own constructors".into()),
        };
        *slot = Some(Arc::new(b));
        Ok(self)
    }

    /// Upstream `integral()`: 1 except `Tabular`, `Discrete`, `Mixture`.
    pub fn integral(&self) -> f64 {
        match self {
            Distribution::Discrete(d) => d.di.integral(),
            Distribution::Tabular { law, .. } => law.integral,
            Distribution::Mixture(m) => m.integral,
            _ => 1.0,
        }
    }

    /// Probability density (`Distribution::evaluate`).
    ///
    /// # Errors
    ///
    /// Where upstream throws: `Discrete` and `Mixture`.
    pub fn evaluate(&self, x: f64) -> Result<f64, String> {
        Ok(match self {
            Distribution::Uniform { a, b, .. } => {
                if x <= *a || x >= *b { 0.0 } else { 1.0 / (b - a) }
            }
            Distribution::PowerLaw { law, .. } => {
                let a = law.offset.r_powf(law.ninv);
                let b = (law.offset + law.span).r_powf(law.ninv);
                let n = 1.0 / law.ninv - 1.0;
                if x <= a || x >= b {
                    0.0
                } else {
                    // upstream: `int pwr = n() + 1;` truncates toward zero
                    let pwr = (n + 1.0) as i32 as f64;
                    let norm = pwr / law.span;
                    norm * x.abs().r_powf(n)
                }
            }
            Distribution::Maxwell { theta, .. } => {
                let c = (2.0 / PI.sqrt()) * theta.r_powf(-1.5);
                c * x.sqrt() * (-x / theta).r_exp()
            }
            Distribution::Watt { a, b, .. } => {
                let c = 2.0 / ((PI * b).sqrt() * a.r_powf(1.5) * (a * b / 4.0).r_exp());
                c * (-x / a).r_exp() * (b * x).sqrt().r_sinh()
            }
            Distribution::Normal { law, .. } => {
                if x < law.lower || x > law.upper {
                    0.0
                } else {
                    let pdf = (1.0 / ((2.0 * PI).sqrt() * law.std_dev))
                        * (-(x - law.mean).r_powf(2.0) / (2.0 * law.std_dev.r_powf(2.0))).r_exp();
                    pdf * law.norm_factor
                }
            }
            Distribution::Tabular { law, .. } => law.evaluate(x),
            Distribution::Discrete(_) | Distribution::Mixture(_) => {
                return Err("PDF evaluation not implemented for this distribution type.".into());
            }
        })
    }

    fn sample_unbiased(&self, seed: &mut u64) -> f64 {
        match self {
            Distribution::Discrete(d) => d.x[d.di.sample(seed)],
            Distribution::Uniform { a, b, .. } => a + prn(seed) * (b - a),
            Distribution::PowerLaw { law, .. } => (law.offset + prn(seed) * law.span).r_powf(law.ninv),
            Distribution::Maxwell { theta, .. } => maxwell(seed, *theta),
            Distribution::Watt { a, b, .. } => watt(seed, *a, *b),
            Distribution::Normal { law, .. } => {
                if !law.is_truncated {
                    return normal_variate(law.mean, law.std_dev, seed);
                }
                loop {
                    let x = normal_variate(law.mean, law.std_dev, seed);
                    if !(x < law.lower || x > law.upper) {
                        return x;
                    }
                }
            }
            Distribution::Tabular { law, .. } => law.sample_unbiased(seed),
            Distribution::Mixture(m) => {
                let idx = m.di.sample(seed);
                m.components[idx].sample(seed).0
            }
        }
    }

    /// `Distribution::sample`: the value and its importance weight (1 unbiased).
    ///
    /// # Panics
    ///
    /// If a bias is attached and either distribution has no `evaluate`
    /// (upstream throws there too).
    pub fn sample(&self, seed: &mut u64) -> (f64, f64) {
        match self {
            Distribution::Discrete(d) => {
                let idx = d.di.sample(seed);
                (d.x[idx], if d.weight.is_empty() { 1.0 } else { d.weight[idx] })
            }
            Distribution::Mixture(m) => {
                let idx = m.di.sample(seed);
                let (val, sub) = m.components[idx].sample(seed);
                (val, if m.weight.is_empty() { 1.0 } else { m.weight[idx] } * sub)
            }
            Distribution::Uniform { bias, .. }
            | Distribution::PowerLaw { bias, .. }
            | Distribution::Maxwell { bias, .. }
            | Distribution::Watt { bias, .. }
            | Distribution::Normal { bias, .. }
            | Distribution::Tabular { bias, .. } => match bias {
                Some(b) => {
                    let val = b.sample_unbiased(seed);
                    let wgt = self.evaluate(val).expect("no PDF") / b.evaluate(val).expect("no PDF");
                    (val, wgt)
                }
                None => (self.sample_unbiased(seed), 1.0),
            },
        }
    }
}

/// `openmc.stats.Legendre`: density `sum_l (2l+1)/2 a_l P_l(x)`. **Evaluation
/// only**: upstream's `sample` raises `NotImplementedError`, so none is invented.
#[derive(Debug, Clone, PartialEq)]
pub struct Legendre {
    /// Expansion coefficients `a_l` (without the `(2l+1)/2` factor).
    pub coefficients: Vec<f64>,
}

impl Legendre {
    /// Evaluate the density at `x` by the Bonnet recurrence
    /// `(l+1) P_{l+1} = (2l+1) x P_l - l P_{l-1}` (upstream uses numpy's Clenshaw
    /// form, so agreement is to round-off, not bits).
    pub fn evaluate(&self, x: f64) -> f64 {
        let (mut p_prev, mut p) = (1.0, x);
        let mut sum = 0.0;
        for (l, a) in self.coefficients.iter().enumerate() {
            let pl = match l {
                0 => 1.0,
                1 => x,
                _ => {
                    let lf = (l - 1) as f64;
                    let next = ((2.0 * lf + 1.0) * x * p - lf * p_prev) / (lf + 1.0);
                    p_prev = p;
                    p = next;
                    next
                }
            };
            sum += (2.0 * l as f64 + 1.0) / 2.0 * a * pl;
        }
        sum
    }
}

type V3 = [f64; 3];

fn norm3(v: V3) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// OpenMC's `SpatialDistribution` family; `sample` returns `(position, weight)`.
#[derive(Debug, Clone, PartialEq)]
pub enum SpatialDistribution {
    /// `SpatialPoint`.
    Point(V3),
    /// `SpatialBox` (uniform in the box; the `fission` flag is a constraint, not a draw).
    Box { lower_left: V3, upper_right: V3 },
    /// `CartesianIndependent`.
    Cartesian { x: Distribution, y: Distribution, z: Distribution },
    /// `CylindricalIndependent`.
    Cylindrical {
        r: Distribution,
        phi: Distribution,
        z: Distribution,
        origin: V3,
        z_dir: V3,
        r_dir: V3,
        phi_dir: V3,
    },
    /// `SphericalIndependent`.
    Spherical { r: Distribution, cos_theta: Distribution, phi: Distribution, origin: V3 },
    /// `PointCloud`: weighted discrete points.
    PointCloud { points: Vec<V3>, di: DiscreteIndex, weight: Vec<f64> },
}

impl SpatialDistribution {
    /// `CylindricalIndependent` with upstream's frame construction: `z_dir`,
    /// `r_dir` normalised by division, `phi_dir = normalise(z_dir x r_dir)`.
    ///
    /// # Errors
    ///
    /// If `r_dir . z_dir > 1e-12` (upstream: "r_dir must be perpendicular to z_dir").
    pub fn cylindrical(
        r: Distribution,
        phi: Distribution,
        z: Distribution,
        origin: V3,
        z_dir: V3,
        r_dir: V3,
    ) -> Result<Self, String> {
        let nz = norm3(z_dir);
        let z_dir = [z_dir[0] / nz, z_dir[1] / nz, z_dir[2] / nz];
        let nr = norm3(r_dir);
        let r_dir = [r_dir[0] / nr, r_dir[1] / nr, r_dir[2] / nr];
        let dot = r_dir[0] * z_dir[0] + r_dir[1] * z_dir[1] + r_dir[2] * z_dir[2];
        if dot > 1e-12 {
            return Err("r_dir must be perpendicular to z_dir".into());
        }
        let c = [
            z_dir[1] * r_dir[2] - z_dir[2] * r_dir[1],
            z_dir[2] * r_dir[0] - z_dir[0] * r_dir[2],
            z_dir[0] * r_dir[1] - z_dir[1] * r_dir[0],
        ];
        let nc = norm3(c);
        let phi_dir = [c[0] / nc, c[1] / nc, c[2] / nc];
        Ok(SpatialDistribution::Cylindrical { r, phi, z, origin, z_dir, r_dir, phi_dir })
    }

    /// `PointCloud(points, strengths)`.
    ///
    /// # Errors
    ///
    /// If the lengths differ (upstream's `fatal_error`).
    pub fn point_cloud(points: Vec<V3>, strengths: &[f64]) -> Result<Self, String> {
        if points.len() != strengths.len() {
            return Err(format!(
                "Number of entries for the strengths array {} does not match the number of spatial points provided {}.",
                strengths.len(),
                points.len()
            ));
        }
        Ok(SpatialDistribution::PointCloud { points, di: DiscreteIndex::new(strengths), weight: Vec::new() })
    }

    /// Biased `PointCloud`: points drawn with `bias` strengths, weighted
    /// `p_norm / bias_norm`. (Upstream's XML reader passes the *unbiased*
    /// strengths as the bias, `distribution_spatial.cpp:258`, so its XML route
    /// never biases; this follows the documented intent.)
    ///
    /// # Errors
    ///
    /// If the three lengths differ.
    pub fn point_cloud_biased(points: Vec<V3>, strengths: &[f64], bias: &[f64]) -> Result<Self, String> {
        if points.len() != strengths.len() || points.len() != bias.len() {
            return Err("points, strengths and bias strengths must have the same length".into());
        }
        Ok(SpatialDistribution::PointCloud {
            points,
            di: DiscreteIndex::new(bias),
            weight: compute_importance_weights(strengths, bias),
        })
    }

    /// Sample a position and its importance weight, with upstream's draw order
    /// and operation order.
    pub fn sample(&self, seed: &mut u64) -> (V3, f64) {
        match self {
            SpatialDistribution::Point(r) => (*r, 1.0),
            SpatialDistribution::Box { lower_left: ll, upper_right: ur } => {
                let xi = [prn(seed), prn(seed), prn(seed)];
                (
                    [
                        ll[0] + xi[0] * (ur[0] - ll[0]),
                        ll[1] + xi[1] * (ur[1] - ll[1]),
                        ll[2] + xi[2] * (ur[2] - ll[2]),
                    ],
                    1.0,
                )
            }
            SpatialDistribution::Cartesian { x, y, z } => {
                let (xv, xw) = x.sample(seed);
                let (yv, yw) = y.sample(seed);
                let (zv, zw) = z.sample(seed);
                ([xv, yv, zv], xw * yw * zw)
            }
            SpatialDistribution::Cylindrical { r, phi, z, origin, z_dir, r_dir, phi_dir } => {
                let (rv, rw) = r.sample(seed);
                let (pv, pw) = phi.sample(seed);
                let (zv, zw) = z.sample(seed);
                let (c, s) = (pv.r_cos(), pv.r_sin());
                let mut out = [0.0; 3];
                for k in 0..3 {
                    out[k] = rv * (c * r_dir[k] + s * phi_dir[k]) + zv * z_dir[k] + origin[k];
                }
                (out, rw * pw * zw)
            }
            SpatialDistribution::Spherical { r, cos_theta, phi, origin } => {
                let (rv, rw) = r.sample(seed);
                let (ct, cw) = cos_theta.sample(seed);
                let (pv, pw) = phi.sample(seed);
                let x = rv * (1.0 - ct * ct).sqrt() * pv.r_cos() + origin[0];
                let y = rv * (1.0 - ct * ct).sqrt() * pv.r_sin() + origin[1];
                let z = rv * ct + origin[2];
                ([x, y, z], rw * cw * pw)
            }
            SpatialDistribution::PointCloud { points, di, weight } => {
                let i = di.sample(seed);
                (points[i], if weight.is_empty() { 1.0 } else { weight[i] })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Moments with no upstream involved: Discrete frequencies, PowerLaw mean,
    /// Mixture mean.
    #[test]
    fn statistical_sanity() {
        let mut seed = 77u64;
        let d = Distribution::discrete(&[1.0, 2.0, 3.0], &[1.0, 2.0, 1.0]);
        let n = 100_000;
        let mut count = [0usize; 3];
        for _ in 0..n {
            count[d.sample(&mut seed).0 as usize - 1] += 1;
        }
        assert!((count[1] as f64 / n as f64 - 0.5).abs() < 0.01);
        // PowerLaw n=2 on [1,2]: mean = (3/4)(2^4-1)/(2^3-1)
        let p = Distribution::power_law(1.0, 2.0, 2.0);
        let m: f64 = (0..n).map(|_| p.sample(&mut seed).0).sum::<f64>() / n as f64;
        assert!((m - 0.75 * 15.0 / 7.0).abs() < 0.01, "{m}");
        let mix = Distribution::mixture(vec![
            (1.0, Distribution::uniform(0.0, 2.0)),
            (1.0, Distribution::discrete(&[10.0], &[1.0])),
        ]);
        let m: f64 = (0..n).map(|_| mix.sample(&mut seed).0).sum::<f64>() / n as f64;
        assert!((m - 5.5).abs() < 0.1, "{m}");
        assert!(Distribution::discrete(&[1.0], &[1.0]).evaluate(1.0).is_err());
    }

    /// Legendre with `a_0 = 1` is the uniform density 1/2 on [-1, 1]; `a_1`
    /// adds `(3/2) a_1 x`.
    #[test]
    fn legendre_density() {
        let l = Legendre { coefficients: vec![1.0, 0.4, 0.0, 0.2] };
        let x = 0.3;
        let p3 = 0.5 * (5.0 * x * x * x - 3.0 * x);
        let want = 0.5 + 1.5 * 0.4 * x + 3.5 * 0.2 * p3;
        assert!((l.evaluate(x) - want).abs() < 1e-14);
    }

    /// Frame construction rejects a non-perpendicular `r_dir`.
    #[test]
    fn cylindrical_rejects_parallel_frame() {
        let u = Distribution::discrete(&[0.0], &[1.0]);
        let r = SpatialDistribution::cylindrical(u.clone(), u.clone(), u, [0.0; 3], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]);
        assert!(r.is_err());
    }
}
