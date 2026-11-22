// SPDX-License-Identifier: AGPL-3.0-only
//
// Attribution (RESEARCH_INTEGRITY_AND_PROVENANCE.md): this module is a port of
// OpenMC's decay-photon spectrum code.
//   Upstream project: OpenMC, https://github.com/openmc-dev/openmc
//   Upstream files:   openmc/data/decay.py, openmc/material.py,
//                     openmc/stats/univariate.py, src/distribution.cpp, src/chain.cpp
//   Upstream commit:  8f79d9d0 (the line numbers cited below were checked
//                     against it on 2026-11-21, gh:#1685; the header was missing)
//   Upstream licence: MIT, Copyright (c) 2011-2025 Massachusetts Institute of
//                     Technology, UChicago Argonne LLC, and OpenMC contributors
//   This port:        AGPL-3.0-only, with the MIT notice preserved here.
//! **Decay-photon energy spectra from the depletion chain** (gh:#1391, non-photon part).
//!
//! OpenMC builds a photon-energy distribution for a decaying material from the
//! `<source particle="photon">` elements of a depletion chain. This module
//! ports that, with no photon transport (that is `punggol`, gh:#975):
//!
//! | OpenMC | here |
//! |---|---|
//! | `openmc.data.decay_photon_energy` (`openmc/data/decay.py:558`) | [`decay_photon_energy`] |
//! | `Material.get_decay_photon_energy` (`openmc/material.py:402`) | [`get_decay_photon_energy`] with [`DecayPhotonOptions`] |
//! | `combine_distributions`, `Discrete.merge/clip`, `Mixture.clip`, `_intensity_clip` (`openmc/stats/univariate.py`) | [`combine_distributions`], [`DiscreteSpectrum`], [`MixtureSpectrum`], [`PhotonSpectrum::clip`] |
//! | `openmc.stats.DecaySpectrum` (`univariate.py:2204`) | [`DecaySpectrum`] (the Python class: nuclides, densities, volume) |
//! | C++ `DecaySpectrum` (`src/distribution.cpp:778`) and `ChainNuclide::photon_energy_` (`src/chain.cpp:49`) | [`ResolvedDecaySpectrum`] |
//!
//! # Why two layers
//!
//! [`outram_mc_libs::source::distribution::Distribution`] is the C++ side:
//! normalised, sampled. It is `outram-mc-libs`' univariate family, reused here
//! rather than copied; the chain codec is `outram_mc_libs::depletion::DepletionChainXml`.
//! This module lives in PUNGGOL because decay photons are not neutrons
//! (maintainer, 2026-11-20).
//! Its `Discrete` and `Tabular` forget the unnormalised intensities, and
//! `Material.get_decay_photon_energy` needs them (it clips on `p * x`, and
//! its `Bq` integral is the sum of the intensities). So the Python-side
//! algebra lives here on raw [`PhotonSpectrum`] values, and
//! [`PhotonSpectrum::to_distribution`] hands a result to the sampler.
//!
//! # numpy and Python semantics that matter for bits
//!
//! The reference sums with `np.sum` (pairwise, blocks of 8, split above 128)
//! and `np.trapezoid`; this port reproduces that summation order
//! ([`np_sum`], checked against numpy 2.4.6 for n = 1..299, 1000, 5000) so
//! the integrals agree bit for bit. Python's built-in `sum` is plain
//! sequential on 3.11, the version used for the code-to-code check; Python
//! 3.12 and later compensate it, which this port does not copy.
//! `np.argsort` is not stable for long arrays; [`intensity_clip`] uses a
//! stable sort, which agrees with numpy where intensities are distinct and
//! for n <= 16.
//!
//! # Not ported
//!
//! - Interpolation `linear-log` (the sampler has no such law either) and, for
//!   [`TabularSpectrum::mean`]/[`TabularSpectrum::integral`], `log-linear` and
//!   `log-log`: refused by name ([`DecaySpectrumError::Unsupported`]), never
//!   approximated. No chain in this workspace uses them for decay photons.
//! - Mass density from atomic masses (`Material.get_mass_density` reads the
//!   AME2020 table): the caller supplies `g/cm^3`.
//! - Wiring a [`ResolvedDecaySpectrum`] into an external-source strength
//!   (`src/source.cpp:371`): the energy sampler and the emission rate are
//!   here; nothing transports the photons.
//! - Biased distributions (`bias`), which chains do not carry.
//!
//! # Differences between the two upstream readers (both kept)
//!
//! - A nuclide with several `<source particle="photon">` elements: Python's
//!   `decay_photon_energy` keeps the **last** (dict overwrite,
//!   `deplete/nuclide.py:255`), the C++ `ChainNuclide` the **first** (`break`,
//!   `chain.cpp:53`). [`decay_photon_energy`] follows Python,
//!   [`ResolvedDecaySpectrum::resolve`] follows C++.
//! - A `<source type="tabular">` with no `interpolation`: C++ takes
//!   `histogram` (`distribution.cpp:481`); Python's reader has no default.
//!   Both of these paths here take `histogram`.
//! - Python's `get_decay_photon_energy` returns `combined.distribution[0]`
//!   when clipping leaves one mixture component, **dropping that component's
//!   mixture weight**. Ported as is ([`get_decay_photon_energy`]); it matters
//!   only when a continuous component survives alone.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use outram_mc_libs::depletion::{ChainDecaySource, DepletionChainXml};
use outram_mc_libs::source::distribution::{DiscreteIndex, Distribution, TabularInterp};
use petir::mathf::RealMath;

/// Why a decay spectrum could not be built or evaluated.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum DecaySpectrumError {
    /// A chain `<source>` of a kind other than `discrete` / `tabular`.
    #[error("decay source distribution '{0}' is not discrete or tabular")]
    UnknownDistribution(String),
    /// An interpolation name this port does not carry.
    #[error("interpolation '{0}' is not supported")]
    Interpolation(String),
    /// A quantity that upstream computes but this port refuses by name.
    #[error("{what} is not ported for {interp} interpolation")]
    Unsupported {
        /// What was asked for (`integral`, `mean`).
        what: &'static str,
        /// The interpolation.
        interp: &'static str,
    },
    /// Tabulated values and probabilities of impossible lengths.
    #[error("tabulated values ({x}) and probabilities ({p}) do not fit")]
    TabularLengths {
        /// Number of abscissae.
        x: usize,
        /// Number of probabilities.
        p: usize,
    },
    /// `x` and `p` of a discrete distribution differ in length.
    #[error("discrete values ({x}) and probabilities ({p}) differ in length")]
    DiscreteLengths {
        /// Number of values.
        x: usize,
        /// Number of probabilities.
        p: usize,
    },
    /// A negative or non-finite probability (upstream: `check_greater_than(.., 0.0, True)`).
    #[error("probabilities must be non-negative and finite")]
    BadProbability,
    /// A combine weight that is not strictly positive (`combine_distributions`).
    #[error("combine_distributions needs strictly positive weights")]
    NonPositiveWeight,
    /// `clip` tolerance outside `[0, 1]`.
    #[error("clip tolerance {0} is outside [0, 1]")]
    BadTolerance(f64),
    /// `units = 'Bq'` with no volume.
    #[error("volume must be specified if units='Bq'")]
    VolumeRequired,
    /// A per-mass unit with no mass density.
    #[error("mass density must be specified for per-mass units")]
    MassDensityRequired,
    /// Both `exclude_nuclides` and `include_nuclides`.
    #[error("cannot specify both exclude_nuclides and include_nuclides")]
    BothFilters,
    /// A non-positive volume.
    #[error("volume must be positive")]
    BadVolume,
    /// C++ `DecaySpectrum::init`: nothing with a spectrum and a positive density.
    #[error("DecaySpectrum resolved no nuclide with a decay photon spectrum and a positive atom density")]
    NothingResolved,
    /// Odd-length `<parameters>` on a discrete source.
    #[error("discrete source parameters have odd length {0}")]
    OddDiscreteParameters(usize),
}

// ---------------------------------------------------------------------------
// numpy / Python summation and sorting helpers
// ---------------------------------------------------------------------------

/// `np.sum` of a contiguous `float64` array: numpy's pairwise summation
/// (`DOUBLE_pairwise_sum`): a plain loop below 8 elements, eight interleaved
/// accumulators up to 128, and a recursive split (rounded down to a multiple
/// of 8) above. Verified against numpy 2.4.6.
#[must_use]
pub fn np_sum(a: &[f64]) -> f64 {
    let n = a.len();
    if n < 8 {
        let mut res = 0.0;
        for &x in a {
            res += x;
        }
        res
    } else if n <= 128 {
        let mut r = [a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]];
        let mut i = 8;
        while i < n - (n % 8) {
            for (j, rj) in r.iter_mut().enumerate() {
                *rj += a[i + j];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while i < n {
            res += a[i];
            i += 1;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        np_sum(&a[..n2]) + np_sum(&a[n2..])
    }
}

/// Python's built-in `sum` of floats on 3.11: sequential from `0`.
fn py_sum(a: &[f64]) -> f64 {
    let mut s = 0.0;
    for &x in a {
        s += x;
    }
    s
}

fn np_diff(a: &[f64]) -> Vec<f64> {
    a.windows(2).map(|w| w[1] - w[0]).collect()
}

/// `_intensity_clip` (`openmc/stats/univariate.py:219`): the indices, in
/// original order, of the intensities that make up `1 - tolerance` of the
/// total, taking the largest first.
#[must_use]
pub fn intensity_clip(intensity: &[f64], tolerance: f64) -> Vec<usize> {
    let n = intensity.len();
    if n == 0 {
        return Vec::new();
    }
    let mut idx: Vec<usize> = (0..n).collect();
    idx.sort_by(|&a, &b| {
        intensity[a]
            .partial_cmp(&intensity[b])
            .unwrap_or(core::cmp::Ordering::Equal)
    });
    idx.reverse();
    let mut cumsum = Vec::with_capacity(n);
    let mut run = 0.0;
    for &i in &idx {
        run += intensity[i];
        cumsum.push(run);
    }
    let last = cumsum[n - 1];
    for c in &mut cumsum {
        *c /= last;
    }
    let v = 1.0 - tolerance;
    let cutoff = cumsum.partition_point(|&c| c < v);
    let mut keep: Vec<usize> = idx[..(cutoff + 1).min(n)].to_vec();
    keep.sort_unstable();
    keep
}

fn check_probabilities(p: &[f64]) -> Result<(), DecaySpectrumError> {
    if p.iter().any(|&v| !(v >= 0.0) || !v.is_finite()) {
        return Err(DecaySpectrumError::BadProbability);
    }
    Ok(())
}

fn check_tolerance(tolerance: f64) -> Result<(), DecaySpectrumError> {
    if !(0.0..=1.0).contains(&tolerance) {
        return Err(DecaySpectrumError::BadTolerance(tolerance));
    }
    Ok(())
}

fn interp_name(i: TabularInterp) -> &'static str {
    match i {
        TabularInterp::Histogram => "histogram",
        TabularInterp::LinLin => "linear-linear",
        TabularInterp::LogLin => "log-linear",
        TabularInterp::LogLog => "log-log",
    }
}

/// An interpolation attribute as `distribution.cpp:466-482` reads it
/// (absent = `histogram`).
fn parse_interp(name: Option<&str>) -> Result<TabularInterp, DecaySpectrumError> {
    match name {
        None | Some("histogram") => Ok(TabularInterp::Histogram),
        Some("linear-linear") => Ok(TabularInterp::LinLin),
        Some("log-linear") => Ok(TabularInterp::LogLin),
        Some("log-log") => Ok(TabularInterp::LogLog),
        Some(other) => Err(DecaySpectrumError::Interpolation(other.to_string())),
    }
}

// ---------------------------------------------------------------------------
// Raw univariates
// ---------------------------------------------------------------------------

/// `openmc.stats.Discrete` with its unnormalised probabilities (intensities).
#[derive(Debug, Clone, PartialEq)]
pub struct DiscreteSpectrum {
    x: Vec<f64>,
    p: Vec<f64>,
}

impl DiscreteSpectrum {
    /// `Discrete(x, p)`.
    ///
    /// # Errors
    /// Unequal lengths or a negative probability.
    pub fn new(x: Vec<f64>, p: Vec<f64>) -> Result<Self, DecaySpectrumError> {
        if x.len() != p.len() {
            return Err(DecaySpectrumError::DiscreteLengths { x: x.len(), p: p.len() });
        }
        check_probabilities(&p)?;
        Ok(Self { x, p })
    }

    /// The values (eV for a photon spectrum).
    #[must_use]
    pub fn x(&self) -> &[f64] {
        &self.x
    }

    /// The intensities.
    #[must_use]
    pub fn p(&self) -> &[f64] {
        &self.p
    }

    /// `Discrete.integral` (`univariate.py:485`): `np.sum(p)`.
    #[must_use]
    pub fn integral(&self) -> f64 {
        np_sum(&self.p)
    }

    /// `Discrete.mean` (`univariate.py:497`): `np.sum(x * p) / np.sum(p)`.
    #[must_use]
    pub fn mean(&self) -> f64 {
        let xp: Vec<f64> = self.x.iter().zip(&self.p).map(|(x, p)| x * p).collect();
        np_sum(&xp) / np_sum(&self.p)
    }

    /// `Discrete.clip` (`univariate.py:511`): drop the lines that carry less
    /// than `tolerance` of `sum(x p)`.
    ///
    /// # Errors
    /// A tolerance outside `[0, 1]`.
    pub fn clip(&self, tolerance: f64) -> Result<Self, DecaySpectrumError> {
        check_tolerance(tolerance)?;
        let intensity: Vec<f64> = self.p.iter().zip(&self.x).map(|(p, x)| p * x).collect();
        let keep = intensity_clip(&intensity, tolerance);
        Ok(Self {
            x: keep.iter().map(|&i| self.x[i]).collect(),
            p: keep.iter().map(|&i| self.p[i]).collect(),
        })
    }

    /// `Discrete.merge` (`univariate.py:413`): sum lines with equal `x`,
    /// each distribution scaled by its probability; `x` sorted ascending.
    #[must_use]
    pub fn merge(dists: &[&DiscreteSpectrum], probs: &[f64]) -> Self {
        let mut pairs: Vec<(f64, f64)> = Vec::new();
        for (d, &pd) in dists.iter().zip(probs) {
            for (&x, &p) in d.x.iter().zip(&d.p) {
                pairs.push((x, p * pd));
            }
        }
        // stable: equal x keep insertion order, as a defaultdict accumulates
        pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(core::cmp::Ordering::Equal));
        let mut x: Vec<f64> = Vec::new();
        let mut p: Vec<f64> = Vec::new();
        for (xi, pi) in pairs {
            match x.last() {
                Some(&last) if last == xi => {
                    let k = p.len() - 1;
                    p[k] += pi;
                }
                _ => {
                    x.push(xi);
                    p.push(0.0 + pi);
                }
            }
        }
        Self { x, p }
    }
}

/// `openmc.stats.Tabular` (no bias, negative probabilities refused).
#[derive(Debug, Clone, PartialEq)]
pub struct TabularSpectrum {
    x: Vec<f64>,
    p: Vec<f64>,
    interp: TabularInterp,
}

impl TabularSpectrum {
    /// `Tabular(x, p, interpolation)` (`univariate.py:1466`).
    ///
    /// # Errors
    /// More probabilities than values; unequal lengths unless `histogram`;
    /// a negative probability.
    pub fn new(x: Vec<f64>, p: Vec<f64>, interp: TabularInterp) -> Result<Self, DecaySpectrumError> {
        if p.len() > x.len() || (interp != TabularInterp::Histogram && x.len() != p.len()) {
            return Err(DecaySpectrumError::TabularLengths { x: x.len(), p: p.len() });
        }
        check_probabilities(&p)?;
        Ok(Self { x, p, interp })
    }

    /// The abscissae.
    #[must_use]
    pub fn x(&self) -> &[f64] {
        &self.x
    }

    /// The probabilities per unit `x`.
    #[must_use]
    pub fn p(&self) -> &[f64] {
        &self.p
    }

    /// The interpolation law.
    #[must_use]
    pub fn interpolation(&self) -> TabularInterp {
        self.interp
    }

    fn unsupported(&self, what: &'static str) -> DecaySpectrumError {
        DecaySpectrumError::Unsupported { what, interp: interp_name(self.interp) }
    }

    /// `max(Tabular.cdf())` (`univariate.py:1520`): `np.cumsum` is sequential.
    fn cdf_max(&self) -> Result<f64, DecaySpectrumError> {
        let dx = np_diff(&self.x);
        let mut run = 0.0;
        let mut max = 0.0_f64;
        for (i, d) in dx.iter().enumerate() {
            let c = match self.interp {
                TabularInterp::Histogram => self.p[i] * d,
                TabularInterp::LinLin => 0.5 * (self.p[i] + self.p[i + 1]) * d,
                _ => return Err(self.unsupported("cdf")),
            };
            run += c;
            if run > max {
                max = run;
            }
        }
        Ok(max)
    }

    /// `Tabular.integral` (`univariate.py:1778`).
    ///
    /// # Errors
    /// Interpolation other than histogram and linear-linear.
    pub fn integral(&self) -> Result<f64, DecaySpectrumError> {
        let dx = np_diff(&self.x);
        match self.interp {
            TabularInterp::Histogram => {
                let t: Vec<f64> = dx.iter().zip(&self.p).map(|(d, p)| d * p).collect();
                Ok(np_sum(&t))
            }
            // np.trapezoid: (d * (y[1:] + y[:-1]) / 2.0).sum()
            TabularInterp::LinLin => {
                let t: Vec<f64> = dx
                    .iter()
                    .enumerate()
                    .map(|(i, d)| d * (self.p[i + 1] + self.p[i]) / 2.0)
                    .collect();
                Ok(np_sum(&t))
            }
            _ => Err(self.unsupported("integral")),
        }
    }

    /// `Tabular.mean` (`univariate.py:1548`).
    ///
    /// # Errors
    /// Interpolation other than histogram and linear-linear.
    pub fn mean(&self) -> Result<f64, DecaySpectrumError> {
        let cmax = self.cdf_max()?;
        let n = self.x.len();
        let p: Vec<f64> = self.p.iter().map(|v| v / cmax).collect();
        let dx = np_diff(&self.x);
        match self.interp {
            TabularInterp::Histogram => {
                let t: Vec<f64> = (0..n - 1)
                    .map(|i| 0.5 * (self.x[i] + self.x[i + 1]) * dx[i] * p[i])
                    .collect();
                Ok(np_sum(&t))
            }
            TabularInterp::LinLin => {
                let t: Vec<f64> = (0..n - 1)
                    .map(|i| {
                        let m = (p[i + 1] - p[i]) / dx[i];
                        let dx3 = self.x[i + 1].r_powf(3.0) - self.x[i].r_powf(3.0);
                        let dx2 = self.x[i + 1] * self.x[i + 1] - self.x[i] * self.x[i];
                        (1.0 / 3.0) * m * dx3 + 0.5 * (p[i] - m * self.x[i]) * dx2
                    })
                    .collect();
                Ok(np_sum(&t))
            }
            _ => Err(self.unsupported("mean")),
        }
    }

    /// The probabilities padded with `0` to the length of `x`, as
    /// `Tabular::Tabular(node)` pads an odd parameter list
    /// (`distribution.cpp:487`).
    fn padded_p(&self) -> Vec<f64> {
        let mut p = self.p.clone();
        p.resize(self.x.len(), 0.0);
        p
    }
}

/// A component of a [`MixtureSpectrum`].
#[derive(Debug, Clone, PartialEq)]
pub enum SpectrumComponent {
    /// Discrete lines.
    Discrete(DiscreteSpectrum),
    /// A tabulated continuum.
    Tabular(TabularSpectrum),
}

impl SpectrumComponent {
    fn integral(&self) -> Result<f64, DecaySpectrumError> {
        match self {
            SpectrumComponent::Discrete(d) => Ok(d.integral()),
            SpectrumComponent::Tabular(t) => t.integral(),
        }
    }

    fn mean(&self) -> Result<f64, DecaySpectrumError> {
        match self {
            SpectrumComponent::Discrete(d) => Ok(d.mean()),
            SpectrumComponent::Tabular(t) => t.mean(),
        }
    }

    fn to_distribution(&self) -> Distribution {
        match self {
            SpectrumComponent::Discrete(d) => Distribution::discrete(&d.x, &d.p),
            SpectrumComponent::Tabular(t) => Distribution::tabular(&t.x, &t.padded_p(), t.interp),
        }
    }
}

/// `openmc.stats.Mixture` of [`SpectrumComponent`]s (no bias).
#[derive(Debug, Clone, PartialEq)]
pub struct MixtureSpectrum {
    probability: Vec<f64>,
    components: Vec<SpectrumComponent>,
}

/// What [`MixtureSpectrum::clip`] upstream only warns about: the fraction by
/// which `mean * integral` fell, when it exceeded the tolerance.
pub type ClipWarning = Option<f64>;

impl MixtureSpectrum {
    /// The mixture probabilities.
    #[must_use]
    pub fn probability(&self) -> &[f64] {
        &self.probability
    }

    /// The components.
    #[must_use]
    pub fn components(&self) -> &[SpectrumComponent] {
        &self.components
    }

    /// `Mixture.integral` (`univariate.py:2103`).
    ///
    /// # Errors
    /// A component whose integral is not ported.
    pub fn integral(&self) -> Result<f64, DecaySpectrumError> {
        let mut terms = Vec::with_capacity(self.components.len());
        for (p, c) in self.probability.iter().zip(&self.components) {
            terms.push(p * c.integral()?);
        }
        Ok(py_sum(&terms))
    }

    /// `Mixture.mean` (`univariate.py:2118`).
    ///
    /// # Errors
    /// A component whose mean or integral is not ported.
    pub fn mean(&self) -> Result<f64, DecaySpectrumError> {
        let mut weights = Vec::with_capacity(self.components.len());
        for (p, c) in self.probability.iter().zip(&self.components) {
            weights.push(p * c.integral()?);
        }
        let total = py_sum(&weights);
        if total == 0.0 {
            return Ok(0.0);
        }
        let mut terms = Vec::with_capacity(weights.len());
        for (w, c) in weights.iter().zip(&self.components) {
            terms.push(w * c.mean()?);
        }
        Ok(py_sum(&terms) / total)
    }

    /// `Mixture.clip` (`univariate.py:2142`), not in place. The second value
    /// is the warning upstream issues when `mean * integral` fell by more than
    /// `tolerance`.
    ///
    /// # Errors
    /// A tolerance outside `[0, 1]`, or a component whose mean is not ported.
    pub fn clip(&self, tolerance: f64) -> Result<(Self, ClipWarning), DecaySpectrumError> {
        let original = self.mean()? * self.integral()?;
        let mut mean_integrals = Vec::with_capacity(self.components.len());
        for (p, c) in self.probability.iter().zip(&self.components) {
            mean_integrals.push(p * c.mean()? * c.integral()?);
        }
        let keep = intensity_clip(&mean_integrals, tolerance);
        let mut components = Vec::with_capacity(keep.len());
        for &i in &keep {
            components.push(match &self.components[i] {
                SpectrumComponent::Discrete(d) => SpectrumComponent::Discrete(d.clip(tolerance)?),
                other => other.clone(),
            });
        }
        let new = Self { probability: keep.iter().map(|&i| self.probability[i]).collect(), components };
        let new_mean_integral = new.mean()? * new.integral()?;
        let diff = (original - new_mean_integral) / original;
        Ok((new, if diff > tolerance { Some(diff) } else { None }))
    }
}

/// A photon-energy distribution as the Python layer holds it: discrete lines,
/// a tabulated continuum, or a mixture of those.
#[derive(Debug, Clone, PartialEq)]
pub enum PhotonSpectrum {
    /// `Discrete`.
    Discrete(DiscreteSpectrum),
    /// `Tabular`.
    Tabular(TabularSpectrum),
    /// `Mixture`.
    Mixture(MixtureSpectrum),
}

impl PhotonSpectrum {
    /// `Univariate.integral`: the intensity in photons per second per atom for
    /// a per-atom spectrum.
    ///
    /// # Errors
    /// An integral that is not ported for the interpolation.
    pub fn integral(&self) -> Result<f64, DecaySpectrumError> {
        match self {
            PhotonSpectrum::Discrete(d) => Ok(d.integral()),
            PhotonSpectrum::Tabular(t) => t.integral(),
            PhotonSpectrum::Mixture(m) => m.integral(),
        }
    }

    /// `Univariate.mean`: the mean photon energy in eV.
    ///
    /// # Errors
    /// A mean that is not ported for the interpolation.
    pub fn mean(&self) -> Result<f64, DecaySpectrumError> {
        match self {
            PhotonSpectrum::Discrete(d) => Ok(d.mean()),
            PhotonSpectrum::Tabular(t) => t.mean(),
            PhotonSpectrum::Mixture(m) => m.mean(),
        }
    }

    /// The clip `Material.get_decay_photon_energy` applies
    /// (`material.py:474`): `Discrete` and `Mixture` only, a `Tabular` is
    /// returned as it is.
    ///
    /// # Errors
    /// As [`DiscreteSpectrum::clip`] / [`MixtureSpectrum::clip`].
    pub fn clip(&self, tolerance: f64) -> Result<(Self, ClipWarning), DecaySpectrumError> {
        match self {
            PhotonSpectrum::Discrete(d) => Ok((PhotonSpectrum::Discrete(d.clip(tolerance)?), None)),
            PhotonSpectrum::Mixture(m) => {
                let (m, w) = m.clip(tolerance)?;
                Ok((PhotonSpectrum::Mixture(m), w))
            }
            PhotonSpectrum::Tabular(_) => Ok((self.clone(), None)),
        }
    }

    /// The sampler form. A mixture's probabilities are weighted by their
    /// component's integral, as `Mixture` does in C++ and in
    /// [`Distribution::mixture`].
    #[must_use]
    pub fn to_distribution(&self) -> Distribution {
        match self {
            PhotonSpectrum::Discrete(d) => Distribution::discrete(&d.x, &d.p),
            PhotonSpectrum::Tabular(t) => Distribution::tabular(&t.x, &t.padded_p(), t.interp),
            PhotonSpectrum::Mixture(m) => Distribution::mixture(
                m.probability
                    .iter()
                    .zip(&m.components)
                    .map(|(&p, c)| (p, c.to_distribution()))
                    .collect(),
            ),
        }
    }
}

/// `combine_distributions` (`univariate.py:2508`): merge every discrete
/// distribution into one, flatten mixtures, and put the continuous remainder
/// in a mixture.
///
/// # Errors
/// A weight that is not strictly positive.
pub fn combine_distributions(
    dists: &[PhotonSpectrum],
    probs: &[f64],
) -> Result<PhotonSpectrum, DecaySpectrumError> {
    let mut new_probs: Vec<f64> = Vec::new();
    let mut new_dists: Vec<SpectrumComponent> = Vec::new();
    for (dist, &prob) in dists.iter().zip(probs) {
        if !(prob > 0.0) {
            return Err(DecaySpectrumError::NonPositiveWeight);
        }
        match dist {
            PhotonSpectrum::Mixture(m) => {
                for (pj, d) in m.probability.iter().zip(&m.components) {
                    new_probs.push(prob * pj);
                    new_dists.push(d.clone());
                }
            }
            PhotonSpectrum::Discrete(d) => {
                new_probs.push(prob);
                new_dists.push(SpectrumComponent::Discrete(d.clone()));
            }
            PhotonSpectrum::Tabular(t) => {
                new_probs.push(prob);
                new_dists.push(SpectrumComponent::Tabular(t.clone()));
            }
        }
    }
    let mut disc: Vec<&DiscreteSpectrum> = Vec::new();
    let mut disc_probs: Vec<f64> = Vec::new();
    let mut cont: Vec<TabularSpectrum> = Vec::new();
    let mut cont_probs: Vec<f64> = Vec::new();
    for (d, &p) in new_dists.iter().zip(&new_probs) {
        match d {
            SpectrumComponent::Discrete(x) => {
                disc.push(x);
                disc_probs.push(p);
            }
            SpectrumComponent::Tabular(t) => {
                cont.push(t.clone());
                cont_probs.push(p);
            }
        }
    }
    if !disc.is_empty() {
        let combined = DiscreteSpectrum::merge(&disc, &disc_probs);
        if cont.is_empty() {
            Ok(PhotonSpectrum::Discrete(combined))
        } else {
            let mut probability = cont_probs;
            probability.push(1.0);
            let mut components: Vec<SpectrumComponent> =
                cont.into_iter().map(SpectrumComponent::Tabular).collect();
            components.push(SpectrumComponent::Discrete(combined));
            Ok(PhotonSpectrum::Mixture(MixtureSpectrum { probability, components }))
        }
    } else if cont.len() == 1 {
        let t = &cont[0];
        let scaled: Vec<f64> = t.p.iter().map(|v| v * cont_probs[0]).collect();
        Ok(PhotonSpectrum::Tabular(TabularSpectrum { x: t.x.clone(), p: scaled, interp: t.interp }))
    } else {
        Ok(PhotonSpectrum::Mixture(MixtureSpectrum {
            probability: cont_probs,
            components: cont.into_iter().map(SpectrumComponent::Tabular).collect(),
        }))
    }
}

// ---------------------------------------------------------------------------
// Chain `<source>` -> spectrum
// ---------------------------------------------------------------------------

/// A chain `<source>` as the Python reader makes it (`Univariate.from_xml_element`
/// for `discrete` and `tabular`).
///
/// # Errors
/// Any other distribution type, an unknown interpolation, negative
/// probabilities, or an odd discrete parameter list.
pub fn spectrum_from_chain_source(src: &ChainDecaySource) -> Result<PhotonSpectrum, DecaySpectrumError> {
    let params = &src.parameters;
    match src.distribution.as_str() {
        "discrete" => {
            if params.len() % 2 != 0 {
                return Err(DecaySpectrumError::OddDiscreteParameters(params.len()));
            }
            let n = params.len() / 2;
            Ok(PhotonSpectrum::Discrete(DiscreteSpectrum::new(params[..n].to_vec(), params[n..].to_vec())?))
        }
        "tabular" => {
            let interp = parse_interp(src.interpolation.as_deref())?;
            let m = (params.len() + 1) / 2;
            Ok(PhotonSpectrum::Tabular(TabularSpectrum::new(params[..m].to_vec(), params[m..].to_vec(), interp)?))
        }
        other => Err(DecaySpectrumError::UnknownDistribution(other.to_string())),
    }
}

/// `openmc.data.decay_photon_energy(nuclide)` (`openmc/data/decay.py:558`):
/// the photon spectrum of one decay, as intensities in Bq per atom (decay
/// constants), or `None` when the chain has none. Python keeps the **last**
/// photon `<source>` of a nuclide.
///
/// # Errors
/// A photon source this port cannot read ([`spectrum_from_chain_source`]).
pub fn decay_photon_energy(
    chain: &DepletionChainXml,
    nuclide: &str,
) -> Result<Option<PhotonSpectrum>, DecaySpectrumError> {
    match chain.get(nuclide).and_then(|n| n.sources.iter().rev().find(|s| s.particle == "photon")) {
        Some(src) => spectrum_from_chain_source(src).map(Some),
        None => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// Material.get_decay_photon_energy
// ---------------------------------------------------------------------------

/// The `units` of `Material.get_decay_photon_energy`: the unit of the integral
/// of the returned distribution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecayPhotonUnits {
    /// Total, needs a volume.
    Bq,
    /// Per gram, needs the mass density.
    BqPerG,
    /// Per kilogram, needs the mass density.
    BqPerKg,
    /// Per cm^3.
    BqPerCm3,
    /// Per m^3.
    BqPerM3,
}

/// The arguments of `Material.get_decay_photon_energy` (`material.py:402`).
#[derive(Debug, Clone, PartialEq)]
pub struct DecayPhotonOptions {
    /// `clip_tolerance`: default `1e-6`.
    pub clip_tolerance: f64,
    /// `units`: default [`DecayPhotonUnits::Bq`].
    pub units: DecayPhotonUnits,
    /// Material volume, cm^3 (`volume`).
    pub volume_cm3: Option<f64>,
    /// Material mass density, g/cm^3 (upstream computes it from AME2020).
    pub mass_density_g_cm3: Option<f64>,
    /// `exclude_nuclides`.
    pub exclude_nuclides: Option<Vec<String>>,
    /// `include_nuclides`.
    pub include_nuclides: Option<Vec<String>>,
}

impl Default for DecayPhotonOptions {
    fn default() -> Self {
        Self {
            clip_tolerance: 1e-6,
            units: DecayPhotonUnits::Bq,
            volume_cm3: None,
            mass_density_g_cm3: None,
            exclude_nuclides: None,
            include_nuclides: None,
        }
    }
}

/// The result of [`get_decay_photon_energy`].
#[derive(Debug, Clone, PartialEq)]
pub struct MaterialDecayPhotons {
    /// The distribution; its integral is the photon emission rate in the
    /// requested units.
    pub spectrum: PhotonSpectrum,
    /// The fall in `mean * integral` that `Mixture.clip` only warns about,
    /// when it exceeded the clip tolerance.
    pub clip_warning: ClipWarning,
}

/// `Material.get_decay_photon_energy` (`openmc/material.py:402`).
///
/// `atom_densities` is `Material.get_nuclide_atom_densities()`: nuclide name
/// and density in atoms/(barn cm), in the material's order. Each nuclide with
/// a photon source and a positive density contributes its per-atom spectrum
/// weighted by `1e24 * density * multiplier`; the combined distribution is
/// clipped (Discrete and Mixture), and a mixture left with one component is
/// replaced by that component.
///
/// Returns `None` when no nuclide has a photon source ("no photon sources,
/// exit early").
///
/// # Errors
/// Missing volume or mass density for the units, both nuclide filters, a bad
/// clip tolerance, or a source that cannot be read.
pub fn get_decay_photon_energy(
    chain: &DepletionChainXml,
    atom_densities: &[(String, f64)],
    options: &DecayPhotonOptions,
) -> Result<Option<MaterialDecayPhotons>, DecaySpectrumError> {
    if options.exclude_nuclides.is_some() && options.include_nuclides.is_some() {
        return Err(DecaySpectrumError::BothFilters);
    }
    let density = |o: Option<f64>| o.ok_or(DecaySpectrumError::MassDensityRequired);
    let multiplier = match options.units {
        DecayPhotonUnits::Bq => options.volume_cm3.ok_or(DecaySpectrumError::VolumeRequired)?,
        DecayPhotonUnits::BqPerCm3 => 1.0,
        DecayPhotonUnits::BqPerM3 => 1e6,
        DecayPhotonUnits::BqPerG => 1.0 / density(options.mass_density_g_cm3)?,
        DecayPhotonUnits::BqPerKg => 1000.0 / density(options.mass_density_g_cm3)?,
    };
    let mut dists: Vec<PhotonSpectrum> = Vec::new();
    let mut probs: Vec<f64> = Vec::new();
    for (nuc, atoms_per_bcm) in atom_densities {
        if options.exclude_nuclides.as_ref().is_some_and(|e| e.contains(nuc)) {
            continue;
        }
        if options.include_nuclides.as_ref().is_some_and(|i| !i.contains(nuc)) {
            continue;
        }
        if let Some(per_atom) = decay_photon_energy(chain, nuc)? {
            if *atoms_per_bcm > 0.0 {
                dists.push(per_atom);
                probs.push(1e24 * atoms_per_bcm * multiplier);
            }
        }
    }
    if dists.is_empty() {
        return Ok(None);
    }
    let combined = combine_distributions(&dists, &probs)?;
    let (mut spectrum, clip_warning) = combined.clip(options.clip_tolerance)?;
    if let PhotonSpectrum::Mixture(m) = &spectrum {
        if m.components.len() == 1 {
            // upstream returns `combined.distribution[0]`, mixture weight and all
            spectrum = match m.components[0].clone() {
                SpectrumComponent::Discrete(d) => PhotonSpectrum::Discrete(d),
                SpectrumComponent::Tabular(t) => PhotonSpectrum::Tabular(t),
            };
        }
    }
    Ok(Some(MaterialDecayPhotons { spectrum, clip_warning }))
}

// ---------------------------------------------------------------------------
// openmc.stats.DecaySpectrum (Python class)
// ---------------------------------------------------------------------------

/// `openmc.stats.DecaySpectrum` (`univariate.py:2204`): nuclide names with
/// atom densities (atoms/(barn cm)) and the volume of the source region (cm^3).
/// The chain is passed to each method where upstream reads
/// `openmc.config['chain_file']`.
#[derive(Debug, Clone, PartialEq)]
pub struct DecaySpectrum {
    nuclides: Vec<(String, f64)>,
    volume_cm3: f64,
}

impl DecaySpectrum {
    /// `DecaySpectrum(nuclides, volume)`: densities `>= 0`, volume `> 0`.
    ///
    /// # Errors
    /// [`DecaySpectrumError::BadProbability`] for a negative density,
    /// [`DecaySpectrumError::BadVolume`] for a non-positive volume.
    pub fn new(nuclides: Vec<(String, f64)>, volume_cm3: f64) -> Result<Self, DecaySpectrumError> {
        if nuclides.iter().any(|(_, d)| !(*d >= 0.0) || !d.is_finite()) {
            return Err(DecaySpectrumError::BadProbability);
        }
        if !(volume_cm3 > 0.0) || !volume_cm3.is_finite() {
            return Err(DecaySpectrumError::BadVolume);
        }
        Ok(Self { nuclides, volume_cm3 })
    }

    /// The nuclides and densities.
    #[must_use]
    pub fn nuclides(&self) -> &[(String, f64)] {
        &self.nuclides
    }

    /// The region volume, cm^3.
    #[must_use]
    pub fn volume_cm3(&self) -> f64 {
        self.volume_cm3
    }

    /// `DecaySpectrum.to_distribution` (`univariate.py:2286`): the per-atom
    /// spectra weighted by `density * 1e24 * volume`; `None` when no listed
    /// nuclide has a photon source.
    ///
    /// # Errors
    /// A zero weight on a nuclide with a source (upstream's
    /// `combine_distributions` raises there too), or an unreadable source.
    pub fn to_spectrum(&self, chain: &DepletionChainXml) -> Result<Option<PhotonSpectrum>, DecaySpectrumError> {
        let mut dists = Vec::new();
        let mut weights = Vec::new();
        for (name, density) in &self.nuclides {
            if let Some(d) = decay_photon_energy(chain, name)? {
                dists.push(d);
                weights.push(density * 1e24 * self.volume_cm3);
            }
        }
        if dists.is_empty() {
            return Ok(None);
        }
        combine_distributions(&dists, &weights).map(Some)
    }

    /// `DecaySpectrum.integral` (`univariate.py:2377`): the emission rate in
    /// photons/s, `0.0` when there is no spectrum or it cannot be evaluated.
    #[must_use]
    pub fn integral(&self, chain: &DepletionChainXml) -> f64 {
        match self.to_spectrum(chain) {
            Ok(Some(s)) => s.integral().unwrap_or(0.0),
            _ => 0.0,
        }
    }

    /// `DecaySpectrum.mean` (`univariate.py:2489`): mean photon energy, eV.
    ///
    /// # Errors
    /// No spectrum ([`DecaySpectrumError::NothingResolved`]) or a mean that is
    /// not ported.
    pub fn mean(&self, chain: &DepletionChainXml) -> Result<f64, DecaySpectrumError> {
        self.to_spectrum(chain)?.ok_or(DecaySpectrumError::NothingResolved)?.mean()
    }

    /// `DecaySpectrum.clip` (`univariate.py:2405`), not in place: drop the
    /// non-emitters, then the nuclides below `tolerance` of the emission rate.
    ///
    /// # Errors
    /// An integral that is not ported.
    pub fn clip(&self, chain: &DepletionChainXml, tolerance: f64) -> Result<Self, DecaySpectrumError> {
        let mut names: Vec<(String, f64)> = Vec::new();
        let mut rates: Vec<f64> = Vec::new();
        for (name, density) in &self.nuclides {
            if let Some(d) = decay_photon_energy(chain, name)? {
                rates.push(density * self.volume_cm3 * d.integral()?);
                names.push((name.clone(), *density));
            }
        }
        let nuclides = if names.is_empty() {
            Vec::new()
        } else {
            intensity_clip(&rates, tolerance).into_iter().map(|i| names[i].clone()).collect()
        };
        Ok(Self { nuclides, volume_cm3: self.volume_cm3 })
    }
}

// ---------------------------------------------------------------------------
// C++ DecaySpectrum
// ---------------------------------------------------------------------------

/// One nuclide the C++ constructor skipped, and why (it warns; here it is
/// recorded).
#[derive(Debug, Clone, PartialEq)]
pub struct IgnoredNuclide {
    /// The nuclide.
    pub name: String,
    /// Why.
    pub reason: IgnoredReason,
}

/// Why `DecaySpectrum::DecaySpectrum` skipped a nuclide (`distribution.cpp:800-824`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgnoredReason {
    /// Not in the chain (a warning upstream).
    NotInChain,
    /// In the chain, with no photon source (silent upstream).
    NoPhotonSource,
    /// Zero density (silent upstream).
    ZeroDensity,
    /// Negative density (a warning upstream).
    NegativeDensity,
}

/// One draw of [`ResolvedDecaySpectrum::sample_with_parent`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecaySample {
    /// Photon energy, eV.
    pub energy: f64,
    /// Importance weight (1 for an unbiased spectrum).
    pub weight: f64,
    /// Index of the parent nuclide among [`ResolvedDecaySpectrum::parents`].
    pub parent: usize,
}

/// The C++ `DecaySpectrum` (`src/distribution.cpp:778`): the nuclides of a
/// [`DecaySpectrum`] that resolve against the chain, each with its photon
/// distribution, chosen by absolute emission rate (`atoms * integral`) through
/// an alias table, then the energy drawn from that nuclide's distribution.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedDecaySpectrum {
    parents: Vec<String>,
    dists: Vec<Distribution>,
    di: DiscreteIndex,
    integral: f64,
    ignored: Vec<IgnoredNuclide>,
}

impl ResolvedDecaySpectrum {
    /// `DecaySpectrum::DecaySpectrum(node)` + `init` (`distribution.cpp:778-863`).
    /// The photon distribution of a nuclide is its **first** photon source
    /// (`chain.cpp:49-55`).
    ///
    /// # Errors
    /// [`DecaySpectrumError::NothingResolved`] where upstream calls
    /// `fatal_error`; an unreadable source.
    pub fn resolve(chain: &DepletionChainXml, spec: &DecaySpectrum) -> Result<Self, DecaySpectrumError> {
        let mut parents = Vec::new();
        let mut dists = Vec::new();
        let mut atoms = Vec::new();
        let mut ignored = Vec::new();
        for (name, density) in &spec.nuclides {
            let Some(nuc) = chain.get(name) else {
                ignored.push(IgnoredNuclide { name: name.clone(), reason: IgnoredReason::NotInChain });
                continue;
            };
            let Some(src) = nuc.sources.iter().find(|s| s.particle == "photon") else {
                ignored.push(IgnoredNuclide { name: name.clone(), reason: IgnoredReason::NoPhotonSource });
                continue;
            };
            let dist = cpp_distribution(src)?;
            if *density <= 0.0 {
                let reason = if *density < 0.0 { IgnoredReason::NegativeDensity } else { IgnoredReason::ZeroDensity };
                ignored.push(IgnoredNuclide { name: name.clone(), reason });
                continue;
            }
            parents.push(name.clone());
            dists.push(dist);
            atoms.push(density * 1.0e24 * spec.volume_cm3);
        }
        let probs: Vec<f64> = atoms.iter().zip(&dists).map(|(a, d)| a * d.integral()).collect();
        let mut integral = 0.0;
        for &p in &probs {
            integral += p;
        }
        if parents.is_empty() || integral <= 0.0 {
            return Err(DecaySpectrumError::NothingResolved);
        }
        Ok(Self { parents, dists, di: DiscreteIndex::new(&probs), integral, ignored })
    }

    /// The nuclides that resolved, in order.
    #[must_use]
    pub fn parents(&self) -> &[String] {
        &self.parents
    }

    /// The nuclides that were skipped, with the reason.
    #[must_use]
    pub fn ignored(&self) -> &[IgnoredNuclide] {
        &self.ignored
    }

    /// `DecaySpectrum::integral`: the total emission rate, photons/s.
    #[must_use]
    pub fn integral(&self) -> f64 {
        self.integral
    }

    /// `DecaySpectrum::sample_with_parent` (`distribution.cpp:865`): the alias
    /// draw of the parent, then the energy from its distribution.
    pub fn sample_with_parent(&self, seed: &mut u64) -> DecaySample {
        let parent = self.di.sample(seed);
        let (energy, weight) = self.dists[parent].sample(seed);
        DecaySample { energy, weight, parent }
    }

    /// `DecaySpectrum::sample`: energy and weight.
    pub fn sample(&self, seed: &mut u64) -> (f64, f64) {
        let s = self.sample_with_parent(seed);
        (s.energy, s.weight)
    }
}

/// `distribution_from_xml` for a chain source (`Discrete(node)`, `Tabular(node)`):
/// discrete takes `params.size() / 2` lines; tabular pads an odd list with a
/// zero probability and defaults to `histogram`.
fn cpp_distribution(src: &ChainDecaySource) -> Result<Distribution, DecaySpectrumError> {
    let params = &src.parameters;
    match src.distribution.as_str() {
        "discrete" => {
            let n = params.len() / 2;
            Ok(Distribution::discrete(&params[..n], &params[n..2 * n]))
        }
        "tabular" => {
            let interp = parse_interp(src.interpolation.as_deref())?;
            let mut padded = params.clone();
            if padded.len() % 2 != 0 {
                padded.push(0.0);
            }
            let n = padded.len() / 2;
            Ok(Distribution::tabular(&padded[..n], &padded[n..], interp))
        }
        other => Err(DecaySpectrumError::UnknownDistribution(other.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain() -> DepletionChainXml {
        DepletionChainXml::parse(
            r#"<depletion_chain>
  <nuclide name="A1" half_life="10.0" decay_modes="0" reactions="0">
    <source type="discrete" particle="photon"><parameters>100.0 200.0 300.0 2.0 1.0 0.001</parameters></source>
  </nuclide>
  <nuclide name="B1" half_life="20.0" decay_modes="0" reactions="0">
    <source type="discrete" particle="photon"><parameters>200.0 400.0 3.0 0.5</parameters></source>
  </nuclide>
  <nuclide name="C1" half_life="30.0" decay_modes="0" reactions="0">
    <source type="tabular" interpolation="histogram" particle="photon"><parameters>0.0 1000.0 2000.0 1e-3 2e-3 0.0</parameters></source>
  </nuclide>
  <nuclide name="D1" half_life="30.0" decay_modes="0" reactions="0"/>
</depletion_chain>"#,
        )
        .unwrap()
    }

    #[test]
    fn np_sum_matches_hand_pairwise_blocks() {
        // 9 elements: blocks of 8 then the tail; 1..=9 sums exactly either way
        let a: Vec<f64> = (1..=9).map(f64::from).collect();
        assert_eq!(np_sum(&a), 45.0);
        // a case where order matters: 1e16 + 1 + 1 ... pairwise keeps the ones
        let mut b = vec![1e16];
        b.extend(core::iter::repeat(1.0).take(15));
        let seq: f64 = b.iter().fold(0.0, |s, x| s + x);
        assert_ne!(np_sum(&b), seq);
        assert!((np_sum(&b) - (1e16 + 15.0)).abs() <= 2.0);
        assert_eq!(np_sum(&[]), 0.0);
        let big: Vec<f64> = (0..1000).map(|i| f64::from(i) * 0.1).collect();
        assert!((np_sum(&big) - 49950.0).abs() < 1e-8);
    }

    #[test]
    fn intensity_clip_keeps_the_largest_until_tolerance() {
        // total 10: dropping 1e-3 of it removes only the 0.001 entry
        let k = intensity_clip(&[5.0, 4.999, 0.001], 1e-3);
        assert_eq!(k, vec![0, 1]);
        assert_eq!(intensity_clip(&[1.0, 1.0], 0.0), vec![0, 1]);
        assert!(intensity_clip(&[], 0.1).is_empty());
    }

    #[test]
    fn merge_sums_equal_lines_and_sorts() {
        let a = DiscreteSpectrum::new(vec![300.0, 100.0], vec![1.0, 2.0]).unwrap();
        let b = DiscreteSpectrum::new(vec![100.0, 50.0], vec![4.0, 8.0]).unwrap();
        let m = DiscreteSpectrum::merge(&[&a, &b], &[1.0, 0.5]);
        assert_eq!(m.x(), &[50.0, 100.0, 300.0]);
        assert_eq!(m.p(), &[4.0, 4.0, 1.0]);
        assert!(DiscreteSpectrum::new(vec![1.0], vec![]).is_err());
        assert!(DiscreteSpectrum::new(vec![1.0], vec![-1.0]).is_err());
    }

    #[test]
    fn discrete_mean_integral_and_clip() {
        let d = DiscreteSpectrum::new(vec![100.0, 200.0, 300.0], vec![2.0, 1.0, 0.001]).unwrap();
        assert!((d.integral() - 3.001).abs() < 1e-15);
        assert!((d.mean() - (200.0 + 200.0 + 0.3) / 3.001).abs() < 1e-12);
        let c = d.clip(1e-3).unwrap();
        assert_eq!(c.x(), &[100.0, 200.0]);
        assert!(d.clip(2.0).is_err());
    }

    #[test]
    fn tabular_histogram_and_linlin_integral_and_mean() {
        let h = TabularSpectrum::new(vec![0.0, 1.0, 3.0], vec![2.0, 1.0, 0.0], TabularInterp::Histogram).unwrap();
        assert_eq!(h.integral().unwrap(), 4.0);
        // mean of histogram: (0.5*1*1*2 + 0.5*4*2*1)/4 = (1 + 4)/4
        assert!((h.mean().unwrap() - 1.25).abs() < 1e-15);
        let l = TabularSpectrum::new(vec![0.0, 2.0], vec![1.0, 1.0], TabularInterp::LinLin).unwrap();
        assert_eq!(l.integral().unwrap(), 2.0);
        assert!((l.mean().unwrap() - 1.0).abs() < 1e-12);
        let ll = TabularSpectrum::new(vec![1.0, 2.0], vec![1.0, 1.0], TabularInterp::LogLog).unwrap();
        assert!(matches!(ll.integral(), Err(DecaySpectrumError::Unsupported { .. })));
        assert!(TabularSpectrum::new(vec![0.0], vec![1.0, 1.0], TabularInterp::Histogram).is_err());
        assert!(TabularSpectrum::new(vec![0.0, 1.0], vec![1.0], TabularInterp::LinLin).is_err());
    }

    #[test]
    fn combine_covers_all_branches() {
        let d1 = PhotonSpectrum::Discrete(DiscreteSpectrum::new(vec![1.0], vec![1.0]).unwrap());
        let t = PhotonSpectrum::Tabular(
            TabularSpectrum::new(vec![0.0, 1.0], vec![1.0, 0.0], TabularInterp::Histogram).unwrap(),
        );
        assert!(matches!(combine_distributions(&[d1.clone()], &[2.0]).unwrap(), PhotonSpectrum::Discrete(_)));
        let one = combine_distributions(&[t.clone()], &[3.0]).unwrap();
        match &one {
            PhotonSpectrum::Tabular(x) => assert_eq!(x.p(), &[3.0, 0.0]),
            other => panic!("{other:?}"),
        }
        let two = combine_distributions(&[t.clone(), t.clone()], &[1.0, 2.0]).unwrap();
        assert!(matches!(&two, PhotonSpectrum::Mixture(m) if m.components().len() == 2));
        let mixed = combine_distributions(&[d1.clone(), t.clone()], &[1.0, 2.0]).unwrap();
        // a mixture flattens when combined again
        let again = combine_distributions(&[mixed, d1], &[1.0, 1.0]).unwrap();
        match &again {
            PhotonSpectrum::Mixture(m) => assert_eq!(m.probability(), &[2.0, 1.0]),
            other => panic!("{other:?}"),
        }
        assert!(combine_distributions(&[t], &[0.0]).is_err());
    }

    #[test]
    fn chain_sources_become_spectra() {
        let c = chain();
        let a = decay_photon_energy(&c, "A1").unwrap().unwrap();
        assert!((a.integral().unwrap() - 3.001).abs() < 1e-15);
        assert!(decay_photon_energy(&c, "D1").unwrap().is_none());
        assert!(decay_photon_energy(&c, "absent").unwrap().is_none());
        let t = decay_photon_energy(&c, "C1").unwrap().unwrap();
        assert!(matches!(t, PhotonSpectrum::Tabular(_)));
        // odd discrete list is refused
        let bad = ChainDecaySource {
            distribution: "discrete".into(),
            particle: "photon".into(),
            interpolation: None,
            parameters: vec![1.0, 2.0, 3.0],
        };
        assert!(spectrum_from_chain_source(&bad).is_err());
        let uniform = ChainDecaySource { distribution: "uniform".into(), parameters: vec![], ..bad };
        assert!(spectrum_from_chain_source(&uniform).is_err());
    }

    #[test]
    fn material_photons_units_filters_and_single_component_pick() {
        let c = chain();
        let dens = vec![("A1".to_string(), 1e-3), ("B1".to_string(), 2e-3), ("C1".to_string(), 1e-4), ("D1".to_string(), 1.0)];
        let opt = DecayPhotonOptions { volume_cm3: Some(10.0), clip_tolerance: 0.0, ..Default::default() };
        let r = get_decay_photon_energy(&c, &dens, &opt).unwrap().unwrap();
        let total = r.spectrum.integral().unwrap();
        // Bq = sum over nuclides of atoms * per-atom intensity
        let expect = 1e24 * 1e-3 * 10.0 * 3.001 + 1e24 * 2e-3 * 10.0 * 3.5 + 1e24 * 1e-4 * 10.0 * 3.0;
        assert!((total - expect).abs() / expect < 1e-12, "{total} vs {expect}");
        // per cm3 is the Bq total / volume
        let per = DecayPhotonOptions { units: DecayPhotonUnits::BqPerCm3, clip_tolerance: 0.0, ..Default::default() };
        let rp = get_decay_photon_energy(&c, &dens, &per).unwrap().unwrap();
        assert!((rp.spectrum.integral().unwrap() * 10.0 - total).abs() / total < 1e-12);
        // include / exclude
        let only = DecayPhotonOptions { include_nuclides: Some(vec!["B1".into()]), ..opt.clone() };
        let rb = get_decay_photon_energy(&c, &dens, &only).unwrap().unwrap();
        assert!(matches!(rb.spectrum, PhotonSpectrum::Discrete(_)));
        let none = DecayPhotonOptions { include_nuclides: Some(vec!["D1".into()]), ..opt.clone() };
        assert!(get_decay_photon_energy(&c, &dens, &none).unwrap().is_none());
        let both = DecayPhotonOptions { include_nuclides: Some(vec![]), exclude_nuclides: Some(vec![]), ..opt.clone() };
        assert_eq!(get_decay_photon_energy(&c, &dens, &both), Err(DecaySpectrumError::BothFilters));
        let novol = DecayPhotonOptions::default();
        assert_eq!(get_decay_photon_energy(&c, &dens, &novol), Err(DecaySpectrumError::VolumeRequired));
        let pg = DecayPhotonOptions { units: DecayPhotonUnits::BqPerG, ..Default::default() };
        assert_eq!(get_decay_photon_energy(&c, &dens, &pg), Err(DecaySpectrumError::MassDensityRequired));
        let pg = DecayPhotonOptions { units: DecayPhotonUnits::BqPerKg, mass_density_g_cm3: Some(2.0), clip_tolerance: 0.0, ..Default::default() };
        let rk = get_decay_photon_energy(&c, &dens, &pg).unwrap().unwrap();
        let per_cm3 = rp.spectrum.integral().unwrap();
        assert!((rk.spectrum.integral().unwrap() * 2.0 / 1000.0 - per_cm3).abs() < 1e-12 * per_cm3);
        let pm = DecayPhotonOptions { units: DecayPhotonUnits::BqPerM3, clip_tolerance: 0.0, ..Default::default() };
        let rm = get_decay_photon_energy(&c, &dens, &pm).unwrap().unwrap();
        assert!((rm.spectrum.integral().unwrap() / 1e6 - rp.spectrum.integral().unwrap()).abs() < 1e-9 * total);
    }

    #[test]
    fn mixture_clip_drops_negligible_continuum_and_returns_the_lone_component() {
        let c = chain();
        // tiny continuum next to strong lines: a loose tolerance removes it
        let dens = vec![("A1".to_string(), 1.0), ("C1".to_string(), 1e-12)];
        let opt = DecayPhotonOptions { volume_cm3: Some(1.0), clip_tolerance: 1e-3, ..Default::default() };
        let r = get_decay_photon_energy(&c, &dens, &opt).unwrap().unwrap();
        assert!(matches!(r.spectrum, PhotonSpectrum::Discrete(_)), "{:?}", r.spectrum);
    }

    #[test]
    fn decay_spectrum_python_class() {
        let c = chain();
        let ds = DecaySpectrum::new(vec![("A1".into(), 1e-3), ("C1".into(), 1e-4), ("D1".into(), 1.0)], 2.0).unwrap();
        let rate = 1e24 * 1e-3 * 2.0 * 3.001 + 1e24 * 1e-4 * 2.0 * 3.0;
        assert!((ds.integral(&c) - rate).abs() / rate < 1e-12);
        assert!(ds.mean(&c).unwrap() > 0.0);
        // clip drops D1 (no source) unconditionally and a negligible nuclide
        let tiny = DecaySpectrum::new(vec![("A1".into(), 1.0), ("C1".into(), 1e-15), ("D1".into(), 1.0)], 1.0).unwrap();
        let k = tiny.clip(&c, 1e-6).unwrap();
        assert_eq!(k.nuclides().len(), 1);
        assert_eq!(k.nuclides()[0].0, "A1");
        assert_eq!(k.volume_cm3(), 1.0);
        let none = DecaySpectrum::new(vec![("D1".into(), 1.0)], 1.0).unwrap();
        assert_eq!(none.integral(&c), 0.0);
        assert!(none.mean(&c).is_err());
        assert!(none.clip(&c, 0.0).unwrap().nuclides().is_empty());
        assert!(DecaySpectrum::new(vec![("A1".into(), -1.0)], 1.0).is_err());
        assert!(DecaySpectrum::new(vec![], 0.0).is_err());
    }

    #[test]
    fn resolved_spectrum_integral_ignores_and_sampling() {
        let c = chain();
        let ds = DecaySpectrum::new(
            vec![
                ("A1".into(), 1e-3),
                ("B1".into(), 3e-3),
                ("C1".into(), 1e-4),
                ("D1".into(), 1.0),
                ("zz".into(), 1.0),
                ("B1".into(), 0.0),
            ],
            2.0,
        )
        .unwrap();
        let r = ResolvedDecaySpectrum::resolve(&c, &ds).unwrap();
        assert_eq!(r.parents(), &["A1".to_string(), "B1".to_string(), "C1".to_string()]);
        let kinds: Vec<IgnoredReason> = r.ignored().iter().map(|i| i.reason).collect();
        assert_eq!(kinds, vec![IgnoredReason::NoPhotonSource, IgnoredReason::NotInChain, IgnoredReason::ZeroDensity]);
        // the C++ integral is atoms * Distribution::integral; histogram Tabular
        // integral is sum(p dx) = 1e-3*1000 + 2e-3*1000 = 3
        let expect = 1e24 * 1e-3 * 2.0 * 3.001 + 1e24 * 3e-3 * 2.0 * 3.5 + 1e24 * 1e-4 * 2.0 * 3.0;
        assert!((r.integral() - expect).abs() / expect < 1e-12);
        // sampling: parents drawn in proportion to their emission rate
        let mut seed = 12345_u64;
        let mut count = [0usize; 3];
        for _ in 0..30000 {
            let s = r.sample_with_parent(&mut seed);
            assert_eq!(s.weight, 1.0);
            count[s.parent] += 1;
            if s.parent == 1 {
                assert!(s.energy == 200.0 || s.energy == 400.0);
            }
        }
        let rates = [1e-3 * 3.001, 3e-3 * 3.5, 1e-4 * 3.0];
        let tot: f64 = rates.iter().sum();
        for k in 0..3 {
            let f = count[k] as f64 / 30000.0;
            assert!((f - rates[k] / tot).abs() < 0.01, "{k}: {f} vs {}", rates[k] / tot);
        }
        let (e, w) = r.sample(&mut seed);
        assert!(e > 0.0 && w == 1.0);
        // nothing resolves -> upstream fatal_error
        let none = DecaySpectrum::new(vec![("D1".into(), 1.0)], 1.0).unwrap();
        assert_eq!(ResolvedDecaySpectrum::resolve(&c, &none), Err(DecaySpectrumError::NothingResolved));
    }

    #[test]
    fn spectrum_to_distribution_has_the_same_integral() {
        let c = chain();
        let ds = DecaySpectrum::new(vec![("A1".into(), 1e-3), ("C1".into(), 1e-4)], 2.0).unwrap();
        let s = ds.to_spectrum(&c).unwrap().unwrap();
        let d = s.to_distribution();
        let expect = s.integral().unwrap();
        assert!((d.integral() - expect).abs() / expect < 1e-12);
    }
}
