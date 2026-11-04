// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NOT ported from RAVEN. Added 2026-11-04 for GitHub issue #1194 (epic #1177)
// at the WORKSPACE MAINTAINER's direction; the crate owner's (Adolphus Lye's)
// review is outstanding — see this crate's CLAUDE.md, "Scope boundaries".

//! [`ParameterSpace`]: a bound or a prior per channel of a [`Quantities`]
//! record, as a RAFFLES [`Distribution`] on the channel's SI base value.

use core::marker::PhantomData;

use super::quantities::{Channel, Quantities, SiQuantity};
use crate::distributions::{ContinuousDistribution1D, Distribution, Normal, Uniform};
use crate::{RafflesError, Result};

/// One [`Distribution`] per channel of `Q`, on the channel's SI base value.
///
/// A bound is a [`Uniform`] prior. Construct with [`bounded`](Self::bounded)
/// (both bounds as a typed `Q`), then replace channels with
/// [`with_normal`](Self::with_normal) (typed) or
/// [`with_prior`](Self::with_prior) (any [`Distribution`], whose parameters
/// are then in the channel's SI base unit, [`Channel::unit`]).
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterSpace<Q: Quantities> {
    channels: Vec<Channel>,
    priors: Vec<Distribution>,
    _record: PhantomData<fn() -> Q>,
}

impl<Q: Quantities> ParameterSpace<Q> {
    /// Uniform on `[lower, upper]` in every channel.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if a bound is not finite or a lower
    /// bound is not strictly below its upper bound (from [`Uniform::new`]).
    pub fn bounded(lower: &Q, upper: &Q) -> Result<Self> {
        let (lo, hi) = (lower.to_si(), upper.to_si());
        let priors = lo
            .iter()
            .zip(hi.iter())
            .map(|(a, b)| Uniform::new(*a, *b).map(Distribution::Uniform))
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            channels: Q::channels(),
            priors,
            _record: PhantomData,
        })
    }

    /// Replaces channel `name`'s prior with `prior`, whose parameters are in
    /// the channel's SI base unit.
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if `Q` has no channel `name`.
    pub fn with_prior(mut self, name: &str, prior: Distribution) -> Result<Self> {
        let i = self.index(name)?;
        self.priors[i] = prior;
        Ok(self)
    }

    /// Replaces channel `name`'s prior with a normal of mean `mean` and
    /// standard deviation `std_dev`, both stated as the channel's quantity
    /// type `T` (for a `ThermodynamicTemperature` channel, `std_dev`'s value
    /// in kelvin is the spread).
    ///
    /// # Errors
    ///
    /// [`RafflesError::InvalidParameter`] if `Q` has no channel `name`, if
    /// `T`'s SI unit is not the channel's, or from [`Normal::new`].
    pub fn with_normal<T: SiQuantity>(self, name: &str, mean: T, std_dev: T) -> Result<Self> {
        let i = self.index(name)?;
        if T::si_unit() != self.channels[i].unit {
            return Err(RafflesError::InvalidParameter {
                parameter: name.to_string(),
                value: mean.si_value(),
                reason: format!(
                    "unit {} does not match the channel's {}",
                    T::si_unit(),
                    self.channels[i].unit
                ),
            });
        }
        let prior = Distribution::Normal(Normal::new(mean.si_value(), std_dev.si_value())?);
        self.with_prior(name, prior)
    }

    /// The channels, in order.
    #[must_use]
    pub fn channels(&self) -> &[Channel] {
        &self.channels
    }

    /// The prior of each channel, in order, on SI base values.
    #[must_use]
    pub fn priors(&self) -> &[Distribution] {
        &self.priors
    }

    /// Lower and upper ends of each channel's support, as typed records
    /// (infinite where a prior is unbounded).
    ///
    /// # Errors
    ///
    /// None in practice: the widths agree by construction.
    pub fn support(&self) -> Result<(Q, Q)> {
        let (lo, hi): (Vec<f64>, Vec<f64>) = self
            .priors
            .iter()
            .map(ContinuousDistribution1D::support)
            .unzip();
        Ok((Q::from_si(&lo)?, Q::from_si(&hi)?))
    }

    /// Whether every channel of `x` lies in its prior's support.
    #[must_use]
    pub fn contains(&self, x: &Q) -> bool {
        x.to_si().iter().zip(self.priors.iter()).all(|(v, p)| {
            let (a, b) = p.support();
            *v >= a && *v <= b
        })
    }

    /// Maps one row of cumulative probabilities (a [`crate::samplers`]
    /// design row, each in `[0, 1)`) through each channel's inverse CDF.
    ///
    /// # Errors
    ///
    /// [`RafflesError::DimensionMismatch`] if the row's width is not the
    /// channel count; [`RafflesError::InvalidParameter`] from a `ppf`.
    pub fn map_unit(&self, row: &[f64]) -> Result<Q> {
        if row.len() != self.priors.len() {
            return Err(RafflesError::DimensionMismatch {
                expected: self.priors.len(),
                found: row.len(),
            });
        }
        let si = row
            .iter()
            .zip(self.priors.iter())
            .map(|(u, p)| p.ppf(*u))
            .collect::<Result<Vec<f64>>>()?;
        Q::from_si(&si)
    }

    fn index(&self, name: &str) -> Result<usize> {
        self.channels
            .iter()
            .position(|c| c.name == name)
            .ok_or_else(|| RafflesError::InvalidParameter {
                parameter: name.to_string(),
                value: f64::NAN,
                reason: "no channel of this name".to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::{Length, ThermodynamicTemperature};
    use uom::si::length::millimeter;
    use uom::si::thermodynamic_temperature::{degree_celsius, kelvin};

    crate::quantities! {
        #[derive(Debug, Clone, Copy, PartialEq)]
        struct Pt {
            d: Length,
            t: ThermodynamicTemperature,
        }
    }

    fn pt(d_mm: f64, t_c: f64) -> Pt {
        Pt {
            d: Length::new::<millimeter>(d_mm),
            t: ThermodynamicTemperature::new::<degree_celsius>(t_c),
        }
    }

    /// Bounds stated in mm and °C become SI uniforms; the unit design maps
    /// onto them; a typed normal replaces one channel and a unit mismatch is
    /// refused.
    #[test]
    fn bounds_and_priors_are_stated_in_uom() {
        let space = ParameterSpace::bounded(&pt(10.0, 20.0), &pt(30.0, 80.0)).unwrap();
        assert_eq!(space.channels().len(), 2);
        let (lo, hi) = space.support().unwrap();
        assert_eq!(lo, pt(10.0, 20.0));
        assert_eq!(hi, pt(30.0, 80.0));
        let mid = space.map_unit(&[0.5, 0.5]).unwrap();
        assert!((mid.d.get::<millimeter>() - 20.0).abs() < 1e-12);
        assert!((mid.t.get::<degree_celsius>() - 50.0).abs() < 1e-9);
        assert!(space.contains(&mid));
        assert!(!space.contains(&pt(31.0, 50.0)));
        assert!(space.map_unit(&[0.5]).is_err());
        assert!(space.map_unit(&[0.5, 1.5]).is_err());

        let t0 = ThermodynamicTemperature::new::<kelvin>(300.0);
        let s = ThermodynamicTemperature::new::<kelvin>(2.0);
        let space = space.with_normal("t", t0, s).unwrap();
        assert_eq!(
            space.priors()[1],
            Distribution::Normal(Normal::new(300.0, 2.0).unwrap())
        );
        assert!(space
            .clone()
            .with_normal(
                "t",
                Length::new::<millimeter>(1.0),
                Length::new::<millimeter>(1.0)
            )
            .is_err());
        assert!(space.clone().with_prior("nope", space.priors()[0]).is_err());
        assert!(ParameterSpace::<Pt>::bounded(&pt(30.0, 20.0), &pt(10.0, 80.0)).is_err());
    }
}
