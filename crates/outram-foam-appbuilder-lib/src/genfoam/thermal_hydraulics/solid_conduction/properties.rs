// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NEW WORK (gh:#1149), not a port: GeN-Foam's structure models carry constant
// properties only. This file is part of OUTRAM PARK, distributed under
// GPL-3.0 like the rest of this crate's GeN-Foam module.

//! Temperature-dependent material properties for the solid conduction field
//! (and, optionally, the porous energy equation of
//! [`super::super::solver::OnePhaseSolver`]).
//!
//! **New work, not port (gh:#1149).** Upstream GeN-Foam's structure and
//! porous energy equations carry constant `kappa`, `rho Cp`; nothing here is
//! translated from it.
//!
//! ## The model set (closed enum, no `dyn`)
//!
//! | [`PropertyModel`] variant | value `v(T)` | `int v dT` |
//! |---|---|---|
//! | `Constant(c)` | `c` | exact |
//! | `Linear { v0, beta, t_ref }` | `v0 (1 + beta (T - t_ref))` | exact |
//! | `Polynomial(c)` | `sum_i c_i T^i` (Horner) | exact |
//! | `Tabulated(table)` | piecewise-linear, **clamped** to the end values outside the table | exact (trapezoids of the interpolant) |
//! | `Function(f)` | a plain `fn(f64) -> f64` in SI | 8-point Gauss-Legendre on 16 sub-intervals |
//!
//! ## Hook for the gh:#1148 closures
//!
//! The Breitbach/Barthels bed conductivity, KTA and graphite/steel fits that
//! gh:#1148 adds under `closures/` depend on parameters (porosity, pebble
//! diameter, emissivity, fluence), so a plain `fn` pointer cannot carry them.
//! They plug in by **adding a variant** to [`PropertyModel`] that wraps the
//! closure struct and calls it from [`PropertyModel::value`]; the `match`
//! arms below then fail to compile until each is handled, which is the point
//! of the enum. Until then a parameter-free fit can be passed through
//! [`PropertyModel::Function`].
//!
//! Status: untrusted AI-generated draft (2026-11-03) per `RESPONSIBLE_USE.md`;
//! research, education and V&V only.

use uom::si::f64::{TemperatureCoefficient, ThermalConductivity, ThermodynamicTemperature};
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::volumetric_heat_capacity::joule_per_cubic_meter_kelvin;

use super::SolidConductionError;
use crate::genfoam::thermal_hydraulics::structure::units::VolumetricHeatCapacity;

/// A piecewise-linear table `v(T)`, SI units, temperatures strictly
/// increasing. Outside `[T_first, T_last]` the end value is held (clamped).
#[derive(Debug, Clone)]
pub struct PropertyTable {
    temperatures: Vec<f64>,
    values: Vec<f64>,
}

impl PropertyTable {
    /// Build a table from `(T [K], v [SI])` points.
    ///
    /// # Errors
    /// [`SolidConductionError::InvalidInput`] for fewer than two points, a
    /// non-finite entry, or temperatures not strictly increasing.
    pub fn new(points: &[(f64, f64)]) -> Result<Self, SolidConductionError> {
        if points.len() < 2 {
            return Err(SolidConductionError::InvalidInput(
                "a property table needs at least two points".into(),
            ));
        }
        for w in points.windows(2) {
            if !(w[1].0 > w[0].0) {
                return Err(SolidConductionError::InvalidInput(format!(
                    "table temperatures must be strictly increasing ({} then {})",
                    w[0].0, w[1].0
                )));
            }
        }
        if points.iter().any(|(t, v)| !t.is_finite() || !v.is_finite()) {
            return Err(SolidConductionError::InvalidInput(
                "table entries must be finite".into(),
            ));
        }
        Ok(Self {
            temperatures: points.iter().map(|p| p.0).collect(),
            values: points.iter().map(|p| p.1).collect(),
        })
    }

    fn value(&self, t: f64) -> f64 {
        let ts = &self.temperatures;
        let n = ts.len();
        if t <= ts[0] {
            return self.values[0];
        }
        if t >= ts[n - 1] {
            return self.values[n - 1];
        }
        // First index with ts[i] > t; t lies in [ts[i-1], ts[i]).
        let i = ts.partition_point(|x| *x <= t);
        let (t0, t1) = (ts[i - 1], ts[i]);
        let (v0, v1) = (self.values[i - 1], self.values[i]);
        v0 + (v1 - v0) * (t - t0) / (t1 - t0)
    }

    /// `int_a^b v dT` of the clamped interpolant, `a <= b`.
    fn integral_ordered(&self, a: f64, b: f64) -> f64 {
        // Breakpoints: the table temperatures strictly inside (a, b).
        let mut sum = 0.0;
        let mut lo = a;
        for &tk in self.temperatures.iter().filter(|tk| **tk > a && **tk < b) {
            sum += 0.5 * (self.value(lo) + self.value(tk)) * (tk - lo);
            lo = tk;
        }
        sum + 0.5 * (self.value(lo) + self.value(b)) * (b - lo)
    }
}

/// A scalar material property as a function of temperature, SI units
/// (see the module docs for the variant table).
#[derive(Debug, Clone)]
pub enum PropertyModel {
    /// Temperature-independent.
    Constant(f64),
    /// `v0 (1 + beta (T - t_ref))`.
    Linear {
        /// Value at `t_ref`.
        value_at_reference: f64,
        /// Relative slope, 1/K.
        beta: f64,
        /// Reference temperature, K.
        reference_temperature: f64,
    },
    /// `sum_i c_i T^i`, `T` in K, lowest order first.
    Polynomial(Vec<f64>),
    /// Piecewise-linear, clamped.
    Tabulated(PropertyTable),
    /// A parameter-free function of `T` [K] returning the SI value.
    Function(fn(f64) -> f64),
}

/// 8-point Gauss-Legendre nodes and weights on `[-1, 1]` (Abramowitz &
/// Stegun Table 25.4).
const GL8: [(f64, f64); 8] = [
    (-0.960_289_856_497_536_2, 0.101_228_536_290_376_3),
    (-0.796_666_477_413_626_7, 0.222_381_034_453_374_5),
    (-0.525_532_409_916_329_0, 0.313_706_645_877_887_3),
    (-0.183_434_642_495_649_8, 0.362_683_783_378_362_0),
    (0.183_434_642_495_649_8, 0.362_683_783_378_362_0),
    (0.525_532_409_916_329_0, 0.313_706_645_877_887_3),
    (0.796_666_477_413_626_7, 0.222_381_034_453_374_5),
    (0.960_289_856_497_536_2, 0.101_228_536_290_376_3),
];

impl PropertyModel {
    /// The value at temperature `t` [K].
    #[must_use]
    pub fn value(&self, t: f64) -> f64 {
        match self {
            Self::Constant(c) => *c,
            Self::Linear {
                value_at_reference,
                beta,
                reference_temperature,
            } => value_at_reference * (1.0 + beta * (t - reference_temperature)),
            Self::Polynomial(c) => c.iter().rev().fold(0.0, |acc, ci| acc * t + ci),
            Self::Tabulated(table) => table.value(t),
            Self::Function(f) => f(t),
        }
    }

    /// `int_{t0}^{t1} v(T) dT` (signed: negative when `t1 < t0`).
    ///
    /// Exact for every variant but [`Self::Function`], which uses composite
    /// 8-point Gauss-Legendre (exact for polynomials up to degree 15 on each
    /// of 16 sub-intervals).
    #[must_use]
    pub fn integral(&self, t0: f64, t1: f64) -> f64 {
        match self {
            Self::Constant(c) => c * (t1 - t0),
            Self::Linear {
                value_at_reference,
                beta,
                reference_temperature,
            } => {
                let u0 = t0 - reference_temperature;
                let u1 = t1 - reference_temperature;
                value_at_reference * ((t1 - t0) + 0.5 * beta * (u1 * u1 - u0 * u0))
            }
            Self::Polynomial(c) => {
                // Antiderivative by Horner: sum c_i T^(i+1)/(i+1).
                let anti = |t: f64| {
                    c.iter()
                        .enumerate()
                        .rev()
                        .fold(0.0, |acc, (i, ci)| acc * t + ci / (i as f64 + 1.0))
                        * t
                };
                anti(t1) - anti(t0)
            }
            Self::Tabulated(table) => {
                if t1 >= t0 {
                    table.integral_ordered(t0, t1)
                } else {
                    -table.integral_ordered(t1, t0)
                }
            }
            Self::Function(f) => {
                const N: usize = 16;
                let h = (t1 - t0) / N as f64;
                let mut sum = 0.0;
                for k in 0..N {
                    let mid = t0 + (k as f64 + 0.5) * h;
                    for (x, w) in GL8 {
                        sum += w * f(mid + 0.5 * h * x);
                    }
                }
                0.5 * h * sum
            }
        }
    }

    /// True for [`Self::Constant`]: the solver then takes the exact
    /// constant-property path (no Picard re-evaluation is needed).
    #[must_use]
    pub fn is_constant(&self) -> bool {
        matches!(self, Self::Constant(_))
    }
}

/// Thermal conductivity `lambda(T)` of a region.
#[derive(Debug, Clone)]
pub struct ConductivityModel(pub PropertyModel);

impl ConductivityModel {
    /// A constant conductivity.
    #[must_use]
    pub fn constant(k: ThermalConductivity) -> Self {
        Self(PropertyModel::Constant(k.get::<watt_per_meter_kelvin>()))
    }

    /// `lambda = lambda_ref (1 + beta (T - T_ref))`.
    #[must_use]
    pub fn linear(
        at_reference: ThermalConductivity,
        beta: TemperatureCoefficient,
        reference: ThermodynamicTemperature,
    ) -> Self {
        Self(PropertyModel::Linear {
            value_at_reference: at_reference.get::<watt_per_meter_kelvin>(),
            beta: beta.get::<per_kelvin>(),
            reference_temperature: reference.get::<kelvin>(),
        })
    }

    /// A clamped piecewise-linear table.
    ///
    /// # Errors
    /// See [`PropertyTable::new`].
    pub fn tabulated(
        points: &[(ThermodynamicTemperature, ThermalConductivity)],
    ) -> Result<Self, SolidConductionError> {
        let p: Vec<(f64, f64)> = points
            .iter()
            .map(|(t, k)| (t.get::<kelvin>(), k.get::<watt_per_meter_kelvin>()))
            .collect();
        Ok(Self(PropertyModel::Tabulated(PropertyTable::new(&p)?)))
    }

    /// `lambda` at temperature `t`.
    #[must_use]
    pub fn at(&self, t: ThermodynamicTemperature) -> ThermalConductivity {
        ThermalConductivity::new::<watt_per_meter_kelvin>(self.0.value(t.get::<kelvin>()))
    }
}

/// Volumetric heat capacity `rho c_p (T)` of a region.
#[derive(Debug, Clone)]
pub struct HeatCapacityModel(pub PropertyModel);

impl HeatCapacityModel {
    /// A constant `rho c_p`.
    #[must_use]
    pub fn constant(c: VolumetricHeatCapacity) -> Self {
        Self(PropertyModel::Constant(
            c.get::<joule_per_cubic_meter_kelvin>(),
        ))
    }

    /// `rho c_p = C_ref (1 + beta (T - T_ref))`.
    #[must_use]
    pub fn linear(
        at_reference: VolumetricHeatCapacity,
        beta: TemperatureCoefficient,
        reference: ThermodynamicTemperature,
    ) -> Self {
        Self(PropertyModel::Linear {
            value_at_reference: at_reference.get::<joule_per_cubic_meter_kelvin>(),
            beta: beta.get::<per_kelvin>(),
            reference_temperature: reference.get::<kelvin>(),
        })
    }

    /// A clamped piecewise-linear table.
    ///
    /// # Errors
    /// See [`PropertyTable::new`].
    pub fn tabulated(
        points: &[(ThermodynamicTemperature, VolumetricHeatCapacity)],
    ) -> Result<Self, SolidConductionError> {
        let p: Vec<(f64, f64)> = points
            .iter()
            .map(|(t, c)| (t.get::<kelvin>(), c.get::<joule_per_cubic_meter_kelvin>()))
            .collect();
        Ok(Self(PropertyModel::Tabulated(PropertyTable::new(&p)?)))
    }

    /// `rho c_p` at temperature `t`.
    #[must_use]
    pub fn at(&self, t: ThermodynamicTemperature) -> VolumetricHeatCapacity {
        VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(self.0.value(t.get::<kelvin>()))
    }
}
