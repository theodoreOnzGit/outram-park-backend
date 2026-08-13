// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
// Upstream sources, all under
//   applications/modules/multiphaseEuler/phaseSystem/interfacialModels/
//   heatTransferModels/:
//     RanzMarshall/RanzMarshall.C                   -- RanzMarshall::K
//     sphericalHeatTransfer/sphericalHeatTransfer.C -- sphericalHeatTransfer::K
//     Gunn/Gunn.C                                   -- Gunn::K
//     heatTransferModel/heatTransferModel.H         -- the K(residualAlpha) interface
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

//! Interfacial **heat-transfer** closures for the two-fluid model.
//!
//! # Why this module exists
//!
//! [`crate::two_fluid`] carries interfacial *momentum* transfer (the drag
//! models) but had no interfacial *heat* transfer, so a six-equation model
//! built on it had nothing to close its two energy equations with. This module
//! ports that missing half from OpenFOAM's `multiphaseEuler`.
//!
//! It matters because interfacial heat transfer is what makes a six-equation
//! model worth having at all. Drift flux and HEM both assume the two phases sit
//! on the saturation line; only a two-fluid model can carry subcooled liquid
//! beside superheated vapour, and the rate at which that difference decays
//! *is* `K`.
//!
//! # The quantity
//!
//! Every variant returns the **volumetric heat-transfer coefficient** `K`
//! \[W/(m³·K)\], defined so the interfacial heat flux to a phase is
//!
//! `Q_i = K (T_interface − T_phase)`  \[W/m³\].
//!
//! Upstream's `heatTransferModel::K(residualAlpha)` returns exactly this, and
//! the port keeps that `residual_alpha` argument rather than hard-coding a
//! floor — the caller's own residual-fraction convention has to win, or a phase
//! that has legitimately vanished keeps exchanging heat.
//!
//! # One-resistance and two-resistance, and why the `κ` differs
//!
//! Read side by side, [`RanzMarshall`](InterfacialHeatTransfer::RanzMarshall)
//! uses the **continuous**-phase conductivity while
//! [`Spherical`](InterfacialHeatTransfer::Spherical) uses the **dispersed**-phase
//! conductivity. That is not an inconsistency — it is the two-resistance
//! framework (upstream `twoResistanceHeatTransfer`). Heat crossing the
//! interface passes through two resistances in series: convection through the
//! boundary layer *outside* the inclusion, and conduction *inside* it.
//! Ranz-Marshall closes the outside, spherical conduction the inside. A
//! one-resistance model keeps only the continuous-side term.
//!
//! Getting this backwards is a **silent** error — the code runs and the answer
//! is wrong by whatever the conductivity ratio happens to be, roughly a factor
//! of 10 for steam/water — so each variant's doc says which side it closes, and
//! [`InterfacialHeatTransfer::resistance_side`] lets a caller assert it.
//!
//! # Units
//!
//! Public methods are `uom`-typed where a dimension exists.
//! [`InterfacialHeatTransfer::volumetric_coefficient_si`] takes and returns raw
//! `f64` in strict SI — `W/(m·K)` for conductivity, metre for diameter,
//! dimensionless for the fractions and the Reynolds/Prandtl numbers,
//! `W/(m³·K)` for the result — because a 1-D system-code march calls it once
//! per cell per step.

use uom::si::f64::{Length, ThermalConductivity};
use uom::si::length::meter;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;

use crate::MultiphaseError;

/// An interfacial heat-transfer closure.
///
/// Enum dispatch rather than trait objects, per the workspace rule: the set of
/// closures is closed and known at compile time, so a `match` is exhaustive and
/// every variant is rust-analyzer-navigable.
///
/// # Not ported
///
/// Upstream also carries `Prandtl`, `constantNu`, `nonSphericalHeatTransfer`
/// and `timeScaleFilteredHeatTransfer`. Left out deliberately rather than
/// approximated: `timeScaleFiltered` wraps another model and needs the
/// run-time model-selection machinery this port does not have, and
/// `nonSpherical` needs a shape factor only the population-balance layer
/// supplies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfacialHeatTransfer {
    /// **Ranz-Marshall** (Ranz & Marshall, 1952) — closes the
    /// **continuous-side** convective resistance.
    ///
    /// Upstream `RanzMarshall::K`:
    ///
    /// ```text
    /// Nu = 2 + 0.6 sqrt(Re) cbrt(Pr)
    /// K  = 6 max(alpha_d, alpha_res) kappa_c Nu / d^2
    /// ```
    ///
    /// The `Nu → 2` limit as `Re → 0` is the exact conduction solution for a
    /// sphere in a stagnant infinite medium, which is why this correlation
    /// degrades gracefully at low slip instead of going to zero.
    RanzMarshall,

    /// **Spherical conduction** — closes the **dispersed-side** internal
    /// resistance.
    ///
    /// Upstream `sphericalHeatTransfer::K`:
    ///
    /// ```text
    /// K = 60 max(alpha_d, alpha_res) kappa_d / d^2
    /// ```
    ///
    /// Note `kappa_d`, the **dispersed** phase's conductivity — this is heat
    /// conducting *within* the inclusion, so the continuous phase does not
    /// appear at all. The factor 60 is `6 × 10`, the 10 being the Nusselt
    /// number of the classic transient-conduction solution for a sphere with a
    /// uniform internal sink.
    Spherical,

    /// **Gunn** (Gunn, 1978) — continuous-side, with a voidage correction for
    /// packed and dense fluidised beds.
    ///
    /// Upstream `Gunn::K`:
    ///
    /// ```text
    /// alpha2 = max(alpha_c, alpha_c_res)
    /// Nu = (7 - 10 alpha2 + 5 alpha2^2)(1 + 0.7 Re^0.2 cbrt(Pr))
    ///    + (1.33 - 2.4 alpha2 + 1.2 alpha2^2) Re^0.7 cbrt(Pr)
    /// K  = 6 max(alpha_d, alpha_res) kappa_c Nu / d^2
    /// ```
    ///
    /// At `alpha2 = 1` the leading bracket is `7 − 10 + 5 = 2`, recovering the
    /// same `Nu → 2` stagnant-sphere limit as Ranz-Marshall — but the two are
    /// not identical there, because Gunn's `Re` exponents differ.
    Gunn,
}

impl InterfacialHeatTransfer {
    /// Volumetric heat-transfer coefficient `K` \[W/(m³·K)\] for one cell.
    ///
    /// # Arguments (raw `f64`, strict SI)
    ///
    /// - `alpha_dispersed` — dispersed volume fraction `α_d` \[-\].
    /// - `alpha_continuous` — continuous volume fraction `α_c` \[-\]. Read only
    ///   by [`Gunn`](Self::Gunn); the other two ignore it.
    /// - `kappa_continuous`, `kappa_dispersed` — thermal conductivity of each
    ///   phase \[W/(m·K)\]. Which one a variant reads is **not**
    ///   interchangeable — see the type-level docs on one- versus
    ///   two-resistance.
    /// - `diameter` — dispersed-inclusion diameter `d` \[m\], strictly
    ///   positive.
    /// - `reynolds`, `prandtl` — slip Reynolds and Prandtl numbers \[-\],
    ///   non-negative.
    /// - `residual_alpha` — the caller's residual-fraction floor \[-\], applied
    ///   as `max(α_d, α_res)` exactly as upstream does.
    ///
    /// # Errors
    ///
    /// [`MultiphaseError::InvalidInput`] for a non-positive diameter, a
    /// negative conductivity, or a negative Reynolds or Prandtl number.
    #[allow(clippy::too_many_arguments)]
    pub fn volumetric_coefficient_si(
        self,
        alpha_dispersed: f64,
        alpha_continuous: f64,
        kappa_continuous: f64,
        kappa_dispersed: f64,
        diameter: f64,
        reynolds: f64,
        prandtl: f64,
        residual_alpha: f64,
    ) -> Result<f64, MultiphaseError> {
        if !(diameter > 0.0) {
            return Err(MultiphaseError::InvalidInput(format!(
                "dispersed-phase diameter must be > 0 m (got {diameter})"
            )));
        }
        if kappa_continuous < 0.0 || kappa_dispersed < 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "thermal conductivities must be >= 0 W/(m K) \
                 (got kappa_c={kappa_continuous}, kappa_d={kappa_dispersed})"
            )));
        }
        if reynolds < 0.0 || prandtl < 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "Reynolds and Prandtl numbers must be >= 0 (got Re={reynolds}, Pr={prandtl})"
            )));
        }

        let ad = alpha_dispersed.max(residual_alpha);
        let d2 = diameter * diameter;

        Ok(match self {
            Self::RanzMarshall => {
                let nu = 2.0 + 0.6 * reynolds.sqrt() * prandtl.cbrt();
                6.0 * ad * kappa_continuous * nu / d2
            }
            Self::Spherical => 60.0 * ad * kappa_dispersed / d2,
            Self::Gunn => {
                let nu = self.nusselt(alpha_continuous, reynolds, prandtl, residual_alpha)?;
                6.0 * ad * kappa_continuous * nu / d2
            }
        })
    }

    /// The Nusselt number `Nu` \[-\] this correlation predicts.
    ///
    /// Exposed separately because `Nu` is the quantity a reader recognises and
    /// can check against a textbook, whereas `K` folds in the geometry.
    /// [`Spherical`](Self::Spherical) returns its implied constant `Nu = 10`.
    ///
    /// # Errors
    ///
    /// [`MultiphaseError::InvalidInput`] for a negative Reynolds or Prandtl
    /// number.
    pub fn nusselt(
        self,
        alpha_continuous: f64,
        reynolds: f64,
        prandtl: f64,
        residual_alpha: f64,
    ) -> Result<f64, MultiphaseError> {
        if reynolds < 0.0 || prandtl < 0.0 {
            return Err(MultiphaseError::InvalidInput(format!(
                "Reynolds and Prandtl numbers must be >= 0 (got Re={reynolds}, Pr={prandtl})"
            )));
        }
        Ok(match self {
            Self::RanzMarshall => 2.0 + 0.6 * reynolds.sqrt() * prandtl.cbrt(),
            Self::Spherical => 10.0,
            Self::Gunn => {
                let a2 = alpha_continuous.max(residual_alpha);
                let a2_sq = a2 * a2;
                (7.0 - 10.0 * a2 + 5.0 * a2_sq) * (1.0 + 0.7 * reynolds.powf(0.2) * prandtl.cbrt())
                    + (1.33 - 2.4 * a2 + 1.2 * a2_sq) * reynolds.powf(0.7) * prandtl.cbrt()
            }
        })
    }

    /// Which phase's conductivity this variant reads.
    ///
    /// Returned so a caller assembling a two-resistance pair can assert it has
    /// one of each side rather than two of the same — the silent error the
    /// type-level docs warn about.
    #[must_use]
    pub fn resistance_side(self) -> ResistanceSide {
        match self {
            Self::RanzMarshall | Self::Gunn => ResistanceSide::Continuous,
            Self::Spherical => ResistanceSide::Dispersed,
        }
    }

    /// `uom`-typed wrapper over
    /// [`volumetric_coefficient_si`](Self::volumetric_coefficient_si).
    ///
    /// Returns `K` in \[W/(m³·K)\] as a raw `f64`, because `uom`'s SI quantity
    /// set has no volumetric-heat-transfer-coefficient dimension; the inputs
    /// are still dimension-checked.
    ///
    /// # Errors
    ///
    /// As [`volumetric_coefficient_si`](Self::volumetric_coefficient_si).
    #[allow(clippy::too_many_arguments)]
    pub fn volumetric_coefficient(
        self,
        alpha_dispersed: f64,
        alpha_continuous: f64,
        kappa_continuous: ThermalConductivity,
        kappa_dispersed: ThermalConductivity,
        diameter: Length,
        reynolds: f64,
        prandtl: f64,
        residual_alpha: f64,
    ) -> Result<f64, MultiphaseError> {
        self.volumetric_coefficient_si(
            alpha_dispersed,
            alpha_continuous,
            kappa_continuous.get::<watt_per_meter_kelvin>(),
            kappa_dispersed.get::<watt_per_meter_kelvin>(),
            diameter.get::<meter>(),
            reynolds,
            prandtl,
            residual_alpha,
        )
    }
}

/// Which side of the interface a heat-transfer closure resolves.
///
/// See [`InterfacialHeatTransfer`]'s type-level documentation: a two-resistance
/// pair needs one of each, and using two of the same side is a silent error
/// worth a factor of the conductivity ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResistanceSide {
    /// The convective boundary layer outside the inclusion, closed with the
    /// continuous phase's conductivity.
    Continuous,
    /// Conduction within the inclusion, closed with the dispersed phase's
    /// conductivity.
    Dispersed,
}

#[cfg(test)]
mod tests;
