// ---------------------------------------------------------------------------
// Ported from OpenDA (open data assimilation toolbox).
//
//   Upstream project: OpenDA, OpenDA Association
//   Upstream repo:    https://github.com/OpenDA-Association/OpenDA
//   Upstream files:   core/java/src/org/openda/utils/StochVector.java
//                       (constructors, createRealization, evaluatePdf,
//                        getSqrtCovariance)
//                     core/java/src/org/openda/utils/SqrtCovariance.java
//                       (rightSolve / leftSolve / asMatrix)
//   Upstream commit:  e51565c2c8def7d35f8b378a9ac118edbd98a94d
//   Accessed:         2026-11-04
//
//   Copyright (c) 2012 OpenDA Association. All rights reserved.
//   OpenDA is free software: you can redistribute it and/or modify it under
//   the terms of the GNU Lesser General Public License as published by the
//   Free Software Foundation, either version 3 of the License, or (at your
//   option) any later version. OpenDA is distributed in the hope that it will
//   be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of
//   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Lesser
//   General Public License for more details (LICENSE-LGPL-3.0-OPENDA).
//
// This Rust translation is part of RAFFLES / Outram Park, conveyed under
// AGPL-3.0-only as a combined work (GPLv3 section 13 / AGPLv3 section 13).
//
// Translation notes: OpenDA draws from one static java.util.Random; here the
// source of standard normals is the `DrawSource` enum, which is NOT a port:
// `Injected` replays draws recorded from OpenDA (gh:#1192's differential
// tests), `Lcg` draws from the workspace LCG (`petir::rng::lcg`, Box-Muller,
// `distributions::seeded`). No java.util.Random is reimplemented.
// `evaluatePdf`'s independent branch is ported as written: it returns the
// product of the standard-normal densities MULTIPLIED by the product of the
// standard deviations (see `StochVector::evaluate_pdf`).
// ---------------------------------------------------------------------------

//! **OpenDA's Gaussian uncertainty** (`StochVector`) and the source of its
//! random draws (gh:#1192).

use petir::mathf::RealMath;
use super::jama::Rows;
use super::linalg;
use crate::distributions::seeded::sample_normal;
use crate::{RafflesError, Result};

/// Where OpenDA's random numbers come from. **Written here, not ported.**
#[derive(Debug, Clone, PartialEq)]
pub enum DrawSource {
    /// Replay recorded upstream draws, each stream in order. A stream that
    /// runs out is an error (the port consumed more than OpenDA did).
    Injected {
        /// `nextGaussian()` of `StochVector`'s generator.
        gaussians: Vec<f64>,
        /// `nextDouble()` of the particle filter's generator.
        uniforms: Vec<f64>,
        /// `nextInt(n)` of the Zhang localisation's generator.
        ints: Vec<usize>,
        /// Next unread index of each stream.
        cursor: [usize; 3],
    },
    /// The workspace LCG at this state: Box–Muller normals, `prn` uniforms,
    /// `⌊n·u⌋` integers.
    Lcg {
        /// LCG state.
        seed: u64,
    },
}

fn exhausted(stream: &str, used: usize) -> RafflesError {
    RafflesError::InvalidParameter { parameter: format!("injected {stream} draws"), value: used as f64, reason: "the recorded stream ran out".into() }
}

impl DrawSource {
    /// Replay recorded draws.
    #[must_use]
    pub fn injected(gaussians: Vec<f64>, uniforms: Vec<f64>, ints: Vec<usize>) -> Self {
        Self::Injected { gaussians, uniforms, ints, cursor: [0; 3] }
    }

    /// The workspace LCG.
    #[must_use]
    pub fn lcg(seed: u64) -> Self {
        Self::Lcg { seed }
    }

    /// A standard normal.
    ///
    /// # Errors
    /// An exhausted injected stream.
    pub fn gaussian(&mut self) -> Result<f64> {
        match self {
            Self::Injected { gaussians, cursor, .. } => {
                let v = *gaussians.get(cursor[0]).ok_or_else(|| exhausted("gaussian", cursor[0]))?;
                cursor[0] += 1;
                Ok(v)
            }
            Self::Lcg { seed } => Ok(sample_normal(seed)),
        }
    }

    /// A uniform on `[0, 1)`.
    ///
    /// # Errors
    /// An exhausted injected stream.
    pub fn uniform(&mut self) -> Result<f64> {
        match self {
            Self::Injected { uniforms, cursor, .. } => {
                let v = *uniforms.get(cursor[1]).ok_or_else(|| exhausted("uniform", cursor[1]))?;
                cursor[1] += 1;
                Ok(v)
            }
            Self::Lcg { seed } => Ok(petir::rng::lcg::prn(seed)),
        }
    }

    /// An integer in `0..n`.
    ///
    /// # Errors
    /// An exhausted injected stream, or an injected value `≥ n`.
    pub fn int_below(&mut self, n: usize) -> Result<usize> {
        match self {
            Self::Injected { ints, cursor, .. } => {
                let v = *ints.get(cursor[2]).ok_or_else(|| exhausted("integer", cursor[2]))?;
                cursor[2] += 1;
                if v >= n {
                    return Err(RafflesError::InvalidParameter { parameter: "injected integer".into(), value: v as f64, reason: format!("must be below {n}") });
                }
                Ok(v)
            }
            Self::Lcg { seed } => Ok(((petir::rng::lcg::prn(seed) * n as f64) as usize).min(n - 1)),
        }
    }

    /// How many draws of each stream were consumed (injected only).
    #[must_use]
    pub fn consumed(&self) -> Option<[usize; 3]> {
        match self {
            Self::Injected { cursor, .. } => Some(*cursor),
            Self::Lcg { .. } => None,
        }
    }
}

/// OpenDA's `StochVector`: a Gaussian vector, independent (mean and
/// standard deviations) or correlated (mean and a square root `L` of the
/// covariance, `P = L·Lᵀ`).
#[derive(Debug, Clone, PartialEq)]
pub struct StochVector {
    /// Mean.
    pub mean: Vec<f64>,
    /// Standard deviations (√ of the covariance's diagonal when correlated).
    pub std: Vec<f64>,
    /// `Some((P, L))` when correlated.
    pub correlated: Option<(Rows, Rows)>,
}

impl StochVector {
    /// Independent elements (`StochVector(mean, std)`).
    ///
    /// # Errors
    /// Lengths that differ.
    pub fn independent(mean: Vec<f64>, std: Vec<f64>) -> Result<Self> {
        if mean.len() != std.len() {
            return Err(RafflesError::DimensionMismatch { expected: mean.len(), found: std.len() });
        }
        Ok(Self { mean, std, correlated: None })
    }

    /// Correlated (`StochVector(mean, covariance, isSquareRoot)`): from `P`
    /// its square root is `Matrix.sqrt()` (symmetric, SVD); from `L`,
    /// `P = L·Lᵀ`.
    ///
    /// # Errors
    /// Shapes that disagree, or the SVD's.
    pub fn correlated(mean: Vec<f64>, covariance: Rows, is_square_root: bool) -> Result<Self> {
        if covariance.len() != mean.len() {
            return Err(RafflesError::DimensionMismatch { expected: mean.len(), found: covariance.len() });
        }
        let (p, l) = if is_square_root {
            (linalg::mult(&covariance, &covariance, false, true)?, covariance)
        } else {
            let l = linalg::sqrt(&covariance)?;
            (covariance, l)
        };
        let mut std = linalg::diag(&p);
        for s in &mut std {
            *s = s.sqrt();
        }
        Ok(Self { mean, std, correlated: Some((p, l)) })
    }

    /// Number of elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.mean.len()
    }

    /// True for a zero-length vector.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.mean.is_empty()
    }

    /// `createRealization()`: one standard normal per element, in order;
    /// then `L·v` (correlated) or `v·σ` (independent); then `+ 1.0·mean`.
    ///
    /// # Errors
    /// The draw source's.
    pub fn realization(&self, draws: &mut DrawSource) -> Result<Vec<f64>> {
        let mut sample = Vec::with_capacity(self.len());
        for _ in 0..self.len() {
            sample.push(draws.gaussian()?);
        }
        if let Some((_, l)) = &self.correlated {
            let mut temp = vec![0.0; sample.len()];
            linalg::right_multiply(l, 1.0, &sample, 1.0, &mut temp);
            sample = temp;
        } else {
            linalg::pointwise_multiply(&mut sample, &self.std);
        }
        linalg::axpy(&mut sample, 1.0, &self.mean);
        Ok(sample)
    }

    /// `evaluatePdf(tv)`, **as OpenDA computes it**. Correlated: the
    /// Gaussian density. Independent: `Π √(1/2π)·e^{−t²/2}` with
    /// `t = (x − μ)/σ`, divided by `1/Πσ`, i.e. **multiplied** by `Πσ`
    /// (OpenDA divides `sqrtDetCov` by each σ and then divides by it; a
    /// density would divide by `Πσ`). Only ratios matter to the particle
    /// filter, so with σ fixed over the members the weights are unchanged.
    /// `exp` is the target's libm; OpenDA uses the JVM's `Math.exp`
    /// intrinsic (gh:#1192 record, D3).
    ///
    /// # Errors
    /// A length mismatch, or the SVD's (correlated).
    pub fn evaluate_pdf(&self, tv: &[f64]) -> Result<f64> {
        if tv.len() != self.len() {
            return Err(RafflesError::DimensionMismatch { expected: self.len(), found: tv.len() });
        }
        let mut diff = tv.to_vec();
        linalg::axpy(&mut diff, -1.0, &self.mean);
        let mut sqrt_det_cov = 1.0;
        if let Some((p, l)) = &self.correlated {
            diff = linalg::right_solve(l, &diff)?;
            let d = super::jama::svd(p)?;
            let mut det = 1.0;
            for s in &d.s {
                det *= s;
            }
            sqrt_det_cov = det.sqrt();
        } else {
            linalg::pointwise_divide(&mut diff, &self.std);
            for s in &self.std {
                sqrt_det_cov /= s;
            }
        }
        let mut prob = 1.0;
        for t in &diff {
            prob *= (0.5 / std::f64::consts::PI).sqrt() * (-0.5 * t * t).r_exp();
        }
        Ok(prob / sqrt_det_cov)
    }

    /// `getSqrtCovariance().rightSolve(v, x)` (= `leftSolve` here): `v/σ`
    /// (independent) or OpenDA's SVD least-squares solve with `L`.
    ///
    /// # Errors
    /// A length mismatch, or the SVD's.
    pub fn sqrt_solve(&self, v: &[f64]) -> Result<Vec<f64>> {
        if v.len() != self.len() {
            return Err(RafflesError::DimensionMismatch { expected: self.len(), found: v.len() });
        }
        match &self.correlated {
            Some((_, l)) => linalg::right_solve(l, v),
            None => Ok(v.iter().zip(&self.std).map(|(a, s)| a / s).collect()),
        }
    }

    /// `getSqrtCovariance().asMatrix()`: `diag(σ)` or `L`.
    #[must_use]
    pub fn sqrt_matrix(&self) -> Rows {
        match &self.correlated {
            Some((_, l)) => l.clone(),
            None => linalg::diag_matrix(&self.std),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Realisations replay injected draws exactly; the LCG source gives
    /// N(0, 1) moments; the PDF and the solves; exhausted streams and bad
    /// shapes are errors.
    #[test]
    fn stoch_vector_and_draws() {
        let sv = StochVector::independent(vec![1.0, -2.0], vec![0.5, 2.0]).unwrap();
        let mut d = DrawSource::injected(vec![1.0, -1.0], vec![0.25], vec![3]);
        assert_eq!(sv.realization(&mut d).unwrap(), vec![1.0 * 0.5 + 1.0, -1.0 * 2.0 + -2.0]);
        assert!(d.gaussian().is_err());
        assert_eq!(d.uniform().unwrap(), 0.25);
        assert!(d.uniform().is_err());
        assert!(DrawSource::injected(vec![], vec![], vec![3]).int_below(3).is_err());
        assert_eq!(d.int_below(5).unwrap(), 3);
        assert!(d.int_below(5).is_err());
        assert_eq!(d.consumed(), Some([2, 1, 1]));
        let mut l = DrawSource::lcg(42);
        assert_eq!(l.consumed(), None);
        let xs: Vec<f64> = (0..20000).map(|_| l.gaussian().unwrap()).collect();
        let m = xs.iter().sum::<f64>() / xs.len() as f64;
        let v = xs.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / xs.len() as f64;
        assert!(m.abs() < 0.03 && (v - 1.0).abs() < 0.05);
        assert!((0..100).all(|_| l.int_below(7).unwrap() < 7) && l.uniform().unwrap() < 1.0);
        // PDF at the mean: Π √(1/2π), times Πσ = 1 here (independent branch).
        let dens = (0.5 / std::f64::consts::PI).sqrt().powi(2);
        assert!((sv.evaluate_pdf(&[1.0, -2.0]).unwrap() - dens * 1.0).abs() < 1e-15);
        let c = StochVector::correlated(vec![0.0, 0.0], vec![vec![4.0, 0.0], vec![0.0, 1.0]], false).unwrap();
        assert!((c.std[0] - 2.0).abs() < 1e-14);
        assert!((c.evaluate_pdf(&[0.0, 0.0]).unwrap() - dens / 2.0).abs() < 1e-14);
        let cl = StochVector::correlated(vec![0.0, 0.0], vec![vec![2.0, 0.0], vec![1.0, 1.0]], true).unwrap();
        assert_eq!(cl.correlated.as_ref().unwrap().0, vec![vec![4.0, 2.0], vec![2.0, 2.0]]);
        let mut d2 = DrawSource::injected(vec![1.0, 1.0], vec![], vec![]);
        assert_eq!(cl.realization(&mut d2).unwrap(), vec![2.0, 2.0]);
        let x = cl.sqrt_solve(&[2.0, 2.0]).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-14 && (x[1] - 1.0).abs() < 1e-14);
        assert_eq!(sv.sqrt_solve(&[1.0, 4.0]).unwrap(), vec![2.0, 2.0]);
        assert_eq!(sv.sqrt_matrix(), vec![vec![0.5, 0.0], vec![0.0, 2.0]]);
        assert_eq!(cl.sqrt_matrix()[1][0], 1.0);
        assert!(sv.evaluate_pdf(&[1.0]).is_err() && sv.sqrt_solve(&[1.0]).is_err());
        assert!(StochVector::independent(vec![1.0], vec![]).is_err());
        assert!(StochVector::correlated(vec![1.0], vec![], false).is_err());
        assert!(!sv.is_empty() && sv.len() == 2);
    }
}
