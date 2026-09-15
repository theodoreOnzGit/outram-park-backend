// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/thermalHydraulics/src/phaseModels/
//                    structureModels/powerModels/nuclearSteadyStatePebble/
//                    nuclearSteadyStatePebble.{C,H}  (the `correct` chain)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal author: Carlo Fiorina (EPFL)
//   Upstream license: GPL-3.0
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
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # `nuclearSteadyStatePebble` — steady sub-scale temperatures in a fuel pebble
//!
//! GeN-Foam's steady-state pebble power model, as used by its `3D_gFHR`
//! tutorial. Given the volumetric power a porous cell produces and the pebble's
//! outer surface temperature, it returns the temperature at every interface from
//! that surface down to the hottest UO2 kernel centre.
//!
//! **It is a closed-form chain, not an iteration.** Upstream's `correct()` walks
//! it once per cell per timestep; there is no inner solve to converge, so the
//! port is an exact transcription rather than a re-derivation.
//!
//! ## The pebble
//!
//! A pebble-bed FHR pebble is **not** shaped like an HTR-10 one: the fuel sits in
//! an *annulus* around an unfuelled graphite core, inside an unfuelled shell.
//!
//! ```text
//!   r = 0 ....... r_core ....... r_matrix ....... r_shell
//!   | graphite core | fuelled matrix | graphite shell |
//!     (unheated)      (N TRISO)        (unheated)
//! ```
//!
//! ## The chain (upstream's own order and formulas)
//!
//! With `Q` the total power of one pebble, `q_m` the volumetric power in the
//! fuelled annulus and `Q_t = Q/N` the power of one TRISO particle:
//!
//! ```text
//!   T_m,out = T_surf  + Q/(4 pi k_g) (1/r_m - 1/r_shell)          [unheated shell]
//!   T_m,av  = T_m,out + (q_m/(6 k_eff)) [ r_m^2 - 3(r_m^5 - r_c^5)/(5(r_m^3 - r_c^3)) ]
//!                     + (r_c^3 q_m)/(3 k_eff) [ 1/r_m - 3(r_m^2 - r_c^2)/(2(r_m^3 - r_c^3)) ]
//!   T_f,S   = T_m,av  + Q_t/(4 pi) * R_coat
//!   T_f,av  = T_f,S   + q_f r_f^2 / (15 k_f)
//!   T_f,max = T_f,S   + q_f r_f^2 / (6 k_f)
//! ```
//!
//! where `R_coat` is the series resistance of the four TRISO coatings and
//! `q_f = Q_t / ((4/3) pi r_f^3)`.
//!
//! The `T_m,av` expression is the **surface-to-volume-average** temperature of a
//! uniformly heated spherical annulus whose inner face is adiabatic — adiabatic
//! because the central core produces no power and symmetry puts zero flux at
//! `r = 0`. Note it is an *average*, not the peak: the peak matrix temperature
//! is not what the TRISO particles see on average, and upstream deliberately
//! feeds the average into the particle-scale step.
//!
//! ## Effective matrix conductivity — Maxwell
//!
//! The fuelled matrix is graphite with TRISO particles dispersed in it, and
//! upstream homogenises the pair with the **Maxwell** (Maxwell-Garnett) mixture
//! rule rather than using the bare graphite value:
//!
//! ```text
//!   k_triso = (1/r_f - 1/r_PyCout) / R_coat          [an equivalent solid sphere]
//!   PF      = N (4/3) pi r_PyCout^3 / ((4/3) pi (r_m^3 - r_c^3))
//!   kappa   = k_triso / k_graphite
//!   beta    = (kappa - 1)/(kappa + 2)
//!   k_eff   = k_graphite (1 + 2 beta PF)/(1 - beta PF)
//! ```
//!
//! This matters: for the gFHR tutorial it is a meaningfully different number from
//! the bare graphite conductivity, and using graphite in its place silently
//! changes the matrix temperature rise.
//!
//! ## Power bookkeeping
//!
//! Upstream converts the *cell* volumetric power into a *per-pebble* power with
//! the solid volume fraction `alpha`:
//!
//! ```text
//!   Q = q (4/3) pi r_shell^3 / alpha
//! ```
//!
//! — i.e. the cell's power is shared among the pebbles that occupy `alpha` of
//! it. A cell with `alpha = 0` carries no pebbles and produces no power.
//!
//! ## What is not ported here
//!
//! - **The convective coupling.** Upstream computes the pebble surface
//!   temperature as `T_surf = (q_surf + sum h T)/sum h` from the fluid solve;
//!   here it is an input, because this crate has no coupled porous
//!   thermal-hydraulic driver to produce it.
//! - **Temperature-dependent conductivities.** Upstream evaluates each `k` from a
//!   polynomial in the local temperature. This port takes the conductivities as
//!   values, which the caller may evaluate however it likes; the gFHR tutorial's
//!   own coefficients are constant (only the zeroth term is non-zero), so for
//!   that case the two are identical.
//! - Density correlations and the min/max/average field reductions, which are
//!   reporting rather than physics.

use std::f64::consts::PI;

/// The radii of a pebble and of the TRISO particles dispersed in it, metres.
///
/// Mirrors the `pebble*Radius` / `triso*Radius` entries of a
/// `nuclearSteadyStatePebble` dictionary. All radii are outer radii and must
/// increase strictly outward within each scale.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PebbleGeometry {
    /// Outer radius of the unfuelled central graphite core (m). gFHR: 1.38e-2.
    pub core_radius: f64,
    /// Outer radius of the fuelled matrix annulus (m). gFHR: 1.8e-2.
    pub matrix_radius: f64,
    /// Outer radius of the pebble, i.e. of the unfuelled shell (m). gFHR: 2e-2.
    pub shell_radius: f64,
    /// UO2 kernel radius (m). gFHR: 212.5e-6.
    pub fuel_radius: f64,
    /// Buffer outer radius (m). gFHR: 312.5e-6.
    pub buffer_radius: f64,
    /// Inner-PyC outer radius (m). gFHR: 352.5e-6.
    pub inner_pyc_radius: f64,
    /// SiC outer radius (m). gFHR: 387.5e-6.
    pub silicon_carbide_radius: f64,
    /// Outer-PyC outer radius — the particle's own surface (m). gFHR: 427.5e-6.
    pub outer_pyc_radius: f64,
    /// Number of TRISO particles per pebble. gFHR: 9022.
    pub triso_count: f64,
}

/// Thermal conductivities, W/(m K), at the temperatures the caller chose to
/// evaluate them.
///
/// Upstream evaluates each from a polynomial in the local temperature (the
/// `*KCoeffs` entries); this port takes the values so the caller keeps that
/// choice. The gFHR tutorial's coefficients are constant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PebbleConductivities {
    /// UO2 kernel. gFHR: 3.3073.
    pub fuel: f64,
    /// Porous carbon buffer. gFHR: 0.50.
    pub buffer: f64,
    /// Both pyrolytic-carbon layers — upstream uses one value for IPyC and
    /// OPyC. gFHR: 4.00.
    pub pyrolytic_carbon: f64,
    /// Silicon carbide. gFHR: 90.3.
    pub silicon_carbide: f64,
    /// Matrix and shell graphite. gFHR: 41.68.
    pub graphite: f64,
}

/// The temperature chain from the pebble surface inward, kelvin.
///
/// Produced by [`PebbleGeometry::steady_temperatures`]. Every field is an
/// absolute temperature.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PebbleTemperatures {
    /// Pebble outer surface — the input boundary condition, returned unchanged.
    pub surface: f64,
    /// Outer face of the fuelled matrix annulus (`r = r_matrix`), i.e. the inner
    /// face of the unfuelled shell. Upstream's `Tmout`.
    pub matrix_outer: f64,
    /// **Volume-average** temperature of the fuelled matrix annulus. Upstream's
    /// `Tmav`, and the boundary condition the particle-scale step uses.
    pub matrix_average: f64,
    /// Outer surface of a TRISO particle sitting at the matrix average
    /// temperature. Upstream's `TfS`.
    pub particle_surface: f64,
    /// Volume-average UO2 kernel temperature. Upstream's `Tfav`.
    pub fuel_average: f64,
    /// UO2 kernel **centre** — the peak fuel temperature, and the quantity a
    /// fuel-temperature limit applies to. Upstream's `Tfmax`.
    pub fuel_max: f64,
    /// Effective conductivity of the TRISO-bearing matrix from the Maxwell
    /// mixture rule, W/(m K). Upstream's `keffMatrix`.
    pub matrix_effective_conductivity: f64,
    /// Total power of one pebble, W. Upstream's `Q_pebble_tot`.
    pub pebble_power: f64,
}

impl PebbleGeometry {
    /// Series thermal resistance of the four TRISO coatings for unit power,
    /// `K/W` before the `1/(4 pi)`: the bracket upstream writes as
    /// `1/k_b (1/r_f - 1/r_b) + 1/k_p (1/r_b - 1/r_i) + 1/k_s (1/r_i - 1/r_s) + 1/k_p (1/r_s - 1/r_o)`.
    #[must_use]
    pub fn coating_resistance(&self, k: &PebbleConductivities) -> f64 {
        (1.0 / k.buffer) * (1.0 / self.fuel_radius - 1.0 / self.buffer_radius)
            + (1.0 / k.pyrolytic_carbon) * (1.0 / self.buffer_radius - 1.0 / self.inner_pyc_radius)
            + (1.0 / k.silicon_carbide)
                * (1.0 / self.inner_pyc_radius - 1.0 / self.silicon_carbide_radius)
            + (1.0 / k.pyrolytic_carbon)
                * (1.0 / self.silicon_carbide_radius - 1.0 / self.outer_pyc_radius)
    }

    /// TRISO particle packing fraction within the fuelled annulus,
    /// dimensionless — upstream's `PF`.
    #[must_use]
    pub fn packing_fraction(&self) -> f64 {
        self.triso_count * self.outer_pyc_radius.powi(3)
            / (self.matrix_radius.powi(3) - self.core_radius.powi(3))
    }

    /// Effective conductivity of the TRISO-bearing matrix, W/(m K), by the
    /// Maxwell mixture rule — upstream's `keffmatrix`.
    ///
    /// The dispersed phase is represented by an equivalent solid sphere of
    /// conductivity `k_triso = (1/r_f - 1/r_PyCout)/R_coat`, i.e. the
    /// conductivity a solid particle of the same outer radius would need to
    /// present the same coating resistance.
    #[must_use]
    pub fn effective_matrix_conductivity(&self, k: &PebbleConductivities) -> f64 {
        let k_triso =
            (1.0 / self.fuel_radius - 1.0 / self.outer_pyc_radius) / self.coating_resistance(k);
        let pf = self.packing_fraction();
        let kappa = k_triso / k.graphite;
        let beta = (kappa - 1.0) / (kappa + 2.0);
        k.graphite * (1.0 + 2.0 * beta * pf) / (1.0 - beta * pf)
    }

    /// The full steady temperature chain for one porous cell.
    ///
    /// # Parameters
    ///
    /// - `surface_temperature` — pebble outer surface temperature (K), which
    ///   upstream obtains from the convective coupling and this port takes as an
    ///   input.
    /// - `power_density` — the cell's volumetric power `q` (W/m^3), upstream's
    ///   `powerDensityNeutronics`.
    /// - `solid_fraction` — `alpha`, the volume fraction of the cell occupied by
    ///   pebbles, dimensionless in `(0, 1]`. gFHR: 0.61.
    /// - `k` — the conductivities, already evaluated.
    ///
    /// # Panics
    ///
    /// Panics if the geometry is not strictly nested, if `solid_fraction` is
    /// outside `(0, 1]`, if `triso_count` is not positive, or if any
    /// conductivity is not positive — each of which is a case-setup error that
    /// would otherwise produce a plausible-looking temperature.
    #[must_use]
    pub fn steady_temperatures(
        &self,
        surface_temperature: f64,
        power_density: f64,
        solid_fraction: f64,
        k: &PebbleConductivities,
    ) -> PebbleTemperatures {
        assert!(
            0.0 < self.core_radius
                && self.core_radius < self.matrix_radius
                && self.matrix_radius < self.shell_radius,
            "pebble radii must increase strictly outward"
        );
        assert!(
            0.0 < self.fuel_radius
                && self.fuel_radius < self.buffer_radius
                && self.buffer_radius < self.inner_pyc_radius
                && self.inner_pyc_radius < self.silicon_carbide_radius
                && self.silicon_carbide_radius < self.outer_pyc_radius,
            "TRISO radii must increase strictly outward"
        );
        assert!(
            self.triso_count > 0.0,
            "a pebble needs at least one particle"
        );
        assert!(
            solid_fraction > 0.0 && solid_fraction <= 1.0,
            "the solid volume fraction must lie in (0, 1], got {solid_fraction}"
        );
        for (name, value) in [
            ("fuel", k.fuel),
            ("buffer", k.buffer),
            ("pyrolytic carbon", k.pyrolytic_carbon),
            ("silicon carbide", k.silicon_carbide),
            ("graphite", k.graphite),
        ] {
            assert!(value > 0.0, "the {name} conductivity must be positive");
        }

        let r_c = self.core_radius;
        let r_m = self.matrix_radius;
        let r_s = self.shell_radius;
        let r_f = self.fuel_radius;

        // Cell power -> per-pebble power, upstream's alpha bookkeeping.
        let pebble_power = power_density * (4.0 / 3.0) * PI * r_s.powi(3) / solid_fraction;
        let annulus_volume = (4.0 / 3.0) * PI * (r_m.powi(3) - r_c.powi(3));
        let q_matrix = pebble_power / annulus_volume;
        let particle_power = pebble_power / self.triso_count;
        let q_fuel = particle_power / ((4.0 / 3.0) * PI * r_f.powi(3));

        let k_eff = self.effective_matrix_conductivity(k);

        // Unheated outer shell: the whole pebble power crosses it.
        let matrix_outer =
            surface_temperature + pebble_power / (4.0 * PI * k.graphite) * (1.0 / r_m - 1.0 / r_s);

        // Uniformly heated annulus with an adiabatic inner face, outer face to
        // volume average. Transcribed term for term from upstream.
        let matrix_average = matrix_outer
            + (q_matrix / (6.0 * k_eff))
                * (r_m.powi(2)
                    - 3.0 * (r_m.powi(5) - r_c.powi(5)) / (5.0 * (r_m.powi(3) - r_c.powi(3))))
            + (r_c.powi(3) * q_matrix) / (3.0 * k_eff)
                * (1.0 / r_m
                    - 3.0 * (r_m.powi(2) - r_c.powi(2)) / (2.0 * (r_m.powi(3) - r_c.powi(3))));

        let particle_surface =
            matrix_average + particle_power / (4.0 * PI) * self.coating_resistance(k);
        let fuel_average = particle_surface + q_fuel * r_f.powi(2) / (15.0 * k.fuel);
        let fuel_max = particle_surface + q_fuel * r_f.powi(2) / (6.0 * k.fuel);

        PebbleTemperatures {
            surface: surface_temperature,
            matrix_outer,
            matrix_average,
            particle_surface,
            fuel_average,
            fuel_max,
            matrix_effective_conductivity: k_eff,
            pebble_power,
        }
    }
}

impl PebbleTemperatures {
    /// Total rise from the pebble surface to the peak kernel centre, K.
    ///
    /// For a cell whose power density and `alpha` match another's, this is the
    /// same number — the chain is linear in power and carries no other spatial
    /// dependence. That is what makes a uniformly-powered bed's `Tfmax` spread
    /// equal to its *surface-temperature* spread exactly.
    #[must_use]
    pub fn total_rise(&self) -> f64 {
        self.fuel_max - self.surface
    }
}

#[cfg(test)]
mod tests;
