// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! Exact steady temperature for uniform heat generation in nested slab,
//! cylinder and sphere shells: the verification reference for the
//! finite-volume solve (gh:#949).
//!
//! ## Derivation
//!
//! Write the geometry with an exponent `m`: slab `m = 0` (`r` is the distance
//! from the mid-plane), cylinder `m = 1`, sphere `m = 2`. In steady state the
//! heat crossing radius `r` outwards, per unit of the geometry's measure
//! (`r^m` times `1`, `2 pi H` or `4 pi`), is everything generated inside it:
//!
//! ```text
//!   Q(r) = int_0^r q'''(s) s^m ds,         -k dT/dr = Q(r) / r^m
//! ```
//!
//! Layer `j` spans `r_j <= r <= r_{j+1}` (with `r_0 = 0`) and carries a
//! constant `k_j` and `q_j`. Inside it `Q(r) = q_j r^(m+1)/(m+1) + C_j` with
//! `C_j = Q(r_j) - q_j r_j^(m+1)/(m+1)`, and integrating `dT/dr` gives
//!
//! ```text
//!   T(r) = T(r_{j+1}) + [ q_j (r_{j+1}^2 - r^2) / (2(m+1)) + C_j G_m(r, r_{j+1}) ] / k_j
//!   G_0 = R - r,   G_1 = ln(R/r),   G_2 = 1/r - 1/R
//! ```
//!
//! `C_0 = 0`, so the centre is regular. The surface `R = r_N` is either held at
//! a fixed temperature or cooled convectively, `T(R) = T_a + Q(R) / (R^m h)`.
//!
//! For one layer this is the textbook result, e.g. a sphere's
//! `T(r) = T_s + q (R^2 - r^2) / (6 k)`. **Citation status:** derived here
//! from the energy balance above; no textbook page from the literature corpus
//! is cited yet (UNCITED: maintainer to supply a heat-conduction text from the
//! corpus if a citation is wanted).

use petir::mathf::RealMath;
use uom::si::f64::{Length, ThermalConductivity, ThermodynamicTemperature, VolumetricPowerDensity};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::volumetric_power_density::watt_per_cubic_meter;

use super::mesh::RadialGeometry;
use super::solve::ThermalBoundary;
use super::SteadyConductionError;

/// One layer of a nested radial body, from the previous layer's outer radius
/// (or the centre) out to [`Self::outer_radius`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadialLayer {
    /// Outer radius of the layer (half-thickness for a slab).
    pub outer_radius: Length,
    /// Constant thermal conductivity of the layer.
    pub conductivity: ThermalConductivity,
    /// Uniform volumetric heat generation in the layer (zero for a cladding).
    pub heat_source: VolumetricPowerDensity,
}

/// The exact steady temperature of a nested radial body (see module docs).
#[derive(Debug, Clone)]
pub struct RadialClosedForm {
    m: i32,
    /// Layer outer radii, m.
    radii: Vec<f64>,
    /// Layer conductivities, W/(m K).
    k: Vec<f64>,
    /// Layer sources, W/m^3.
    q: Vec<f64>,
    /// `C_j` of each layer.
    c: Vec<f64>,
    /// Temperature at each layer's outer radius, K.
    t_outer: Vec<f64>,
}

impl RadialClosedForm {
    /// Build the solution for `layers` (innermost first) with the surface
    /// condition `surface`.
    ///
    /// # Errors
    /// [`SteadyConductionError::InvalidInput`] for no layers, radii not
    /// strictly increasing from zero, a non-positive conductivity, a negative
    /// source, or an [`ThermalBoundary::Adiabatic`] surface (no steady state
    /// unless nothing is generated) or a convective one with `h <= 0`.
    pub fn new(
        geometry: RadialGeometry,
        layers: &[RadialLayer],
        surface: ThermalBoundary,
    ) -> Result<Self, SteadyConductionError> {
        if layers.is_empty() {
            return Err(SteadyConductionError::InvalidInput(
                "closed form needs at least one layer".into(),
            ));
        }
        let m = geometry.exponent();
        let mf = f64::from(m);
        let radii: Vec<f64> = layers
            .iter()
            .map(|l| l.outer_radius.get::<meter>())
            .collect();
        let k: Vec<f64> = layers
            .iter()
            .map(|l| l.conductivity.get::<watt_per_meter_kelvin>())
            .collect();
        let q: Vec<f64> = layers
            .iter()
            .map(|l| l.heat_source.get::<watt_per_cubic_meter>())
            .collect();
        let mut previous = 0.0;
        for (j, r) in radii.iter().enumerate() {
            if !(*r > previous) || !r.is_finite() {
                return Err(SteadyConductionError::InvalidInput(format!(
                    "layer {j} outer radius {r} m must exceed the previous one ({previous} m)"
                )));
            }
            if !(k[j] > 0.0) || !(q[j] >= 0.0) {
                return Err(SteadyConductionError::InvalidInput(format!(
                    "layer {j} needs k > 0 and q''' >= 0 (got k = {}, q''' = {})",
                    k[j], q[j]
                )));
            }
            previous = *r;
        }

        // Cumulative Q at each layer's inner radius, and C_j.
        let mut c = Vec::with_capacity(layers.len());
        let mut q_cum = 0.0;
        let mut r_in = 0.0_f64;
        for j in 0..layers.len() {
            c.push(q_cum - q[j] * r_in.r_powi(m + 1) / (mf + 1.0));
            q_cum += q[j] * (radii[j].r_powi(m + 1) - r_in.r_powi(m + 1)) / (mf + 1.0);
            r_in = radii[j];
        }
        let r_surface = radii[radii.len() - 1];
        let surface_flux = q_cum / r_surface.r_powi(m); // W/m^2 leaving the surface
        let t_surface = match surface {
            ThermalBoundary::FixedTemperature(t) => t.get::<kelvin>(),
            ThermalBoundary::Convective {
                coefficient,
                ambient,
            } => {
                let h = coefficient.get::<watt_per_square_meter_kelvin>();
                if !(h > 0.0) {
                    return Err(SteadyConductionError::InvalidInput(format!(
                        "convective surface needs h > 0, got {h} W/(m^2 K)"
                    )));
                }
                ambient.get::<kelvin>() + surface_flux / h
            }
            ThermalBoundary::Adiabatic => {
                return Err(SteadyConductionError::InvalidInput(
                    "an adiabatic surface has no steady state with heat generated inside".into(),
                ))
            }
        };

        let mut solution = Self {
            m,
            radii,
            k,
            q,
            c,
            t_outer: vec![0.0; layers.len()],
        };
        let n = layers.len();
        solution.t_outer[n - 1] = t_surface;
        for j in (0..n - 1).rev() {
            // T at the outer radius of layer j = T at the inner radius of layer j+1.
            solution.t_outer[j] =
                solution.t_outer[j + 1] + solution.rise_in_layer(j + 1, solution.radii[j]);
        }
        Ok(solution)
    }

    /// `T(r) - T(r_{j+1})` inside layer `j`, K.
    fn rise_in_layer(&self, j: usize, r: f64) -> f64 {
        let mf = f64::from(self.m);
        let r_out = self.radii[j];
        let mut rise = self.q[j] * (r_out * r_out - r * r) / (2.0 * (mf + 1.0));
        if self.c[j] != 0.0 {
            let g = match self.m {
                0 => r_out - r,
                1 => (r_out / r).r_ln(),
                _ => 1.0 / r - 1.0 / r_out,
            };
            rise += self.c[j] * g;
        }
        rise / self.k[j]
    }

    /// The exact temperature at radius `r` (distance from the mid-plane for a
    /// slab).
    ///
    /// # Errors
    /// [`SteadyConductionError::InvalidInput`] for `r` outside `[0, R]`.
    pub fn temperature_at(
        &self,
        r: Length,
    ) -> Result<ThermodynamicTemperature, SteadyConductionError> {
        let r = r.get::<meter>();
        let r_surface = self.radii[self.radii.len() - 1];
        if !(0.0..=r_surface).contains(&r) {
            return Err(SteadyConductionError::InvalidInput(format!(
                "radius {r} m is outside the body [0, {r_surface}] m"
            )));
        }
        let j = self
            .radii
            .iter()
            .position(|ro| r <= *ro)
            .unwrap_or(self.radii.len() - 1);
        Ok(ThermodynamicTemperature::new::<kelvin>(
            self.t_outer[j] + self.rise_in_layer(j, r),
        ))
    }

    /// The exact surface temperature.
    #[must_use]
    pub fn surface_temperature(&self) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.t_outer[self.t_outer.len() - 1])
    }

    /// The exact temperature at the centre (`r = 0`), the peak when the heat
    /// is generated in the innermost layer.
    #[must_use]
    pub fn centre_temperature(&self) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(self.t_outer[0] + self.rise_in_layer(0, 0.0))
    }
}
