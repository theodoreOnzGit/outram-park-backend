// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/thermalHydraulics/src/physicsModels/heatTransferModels/
//                    FSHeatTransferCoefficientModels/subModels/postCHFModels/
//                    {CachardLiquid,CachardVapour}/*FSHeatTransferCoefficient.{H,C}
//   Upstream commit: 652b3da (GeN-Foam), built on OpenFOAM v2506
//   Upstream copyright: (C) 2015-2022 EPFL
//   Upstream author: Gauthier Lazare
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK. OUTRAM PARK is free software: you can
// redistribute it and/or modify it under the terms of the GNU General Public
// License as published by the Free Software Foundation, either version 3 of
// the License, or (at your option) any later version. It is distributed in
// the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the
// implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
// See the GNU General Public License for more details.
//
// This offering is not approved or endorsed by EPFL, the OpenFOAM Foundation,
// nor OpenCFD Limited, producer and distributor of the OpenFOAM(R) software.

//! # `CachardLiquid` / `CachardVapour` — inverted-annular film boiling (gh:#1224)
//!
//! The annular-flow models `multiRegimeBoilingTRACECHF` and `multiRegimeBoilingVapourTRACE` call
//! for their film-boiling branches.
//!
//! ## The upstream defect, and the choice made (default = upstream)
//!
//! Both models compute the vapour-film thickness
//! `delta = Dh/2 (sqrt(1 + alpha (4/pi (pi_/DR)^2 - 1)) - 1)` with `DR = 0.01` m (a rod diameter)
//! and `pi_` meant to be a rod pitch. Upstream binds `pi_` to the cell PRESSURE
//! (`const scalar& pi(p_[celli])`, `CachardLiquid...C:124`, `CachardVapour...C:101`), so
//! `(p/DR)^2` is about `2e18` at 15 MPa and `delta` is enormous: the film thickness is wrong by
//! many orders of magnitude and `h = 2 k_g / delta` is tiny.
//!
//! Per the maintainer's direction "upstream is the specification" the DEFAULT is
//! [`CachardPitch::UpstreamPressure`], bit for bit, and the defect is flagged here and in the
//! V&V record. The physically intended form is the named option
//! [`CachardPitch::Pitch`] (`pi_` = a pitch in metres, supplied by the caller;
//! upstream has no keyword for it). Whether to make the corrected form the default, and to report
//! it upstream, is the maintainer's decision (gh:#1224).
//!
//! `pow(x, 2)` is evaluated as `x*x` (GCC folds `pow(x, 2.0)` to a multiplication), the other
//! powers as `pow`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use petir::mathf::RealMath;

/// What the model's `pi` is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CachardPitch {
    /// Upstream: `pi` is the cell pressure [Pa] (the default; a defect, see the module doc).
    UpstreamPressure,
    /// The intended reading: a rod pitch [m].
    Pitch(f64),
}

/// Bundle rod diameter hard-wired upstream (`DRi = 0.01`).
const ROD_DIAMETER: f64 = 0.01;
const GRAVITY: f64 = 9.81;
const SIGMA_SB: f64 = 5.670 * 1e-8;

/// The film thickness `delta` shared by both models.
#[must_use]
pub fn film_thickness(dh: f64, alpha_vapour_normalized: f64, pi_: f64) -> f64 {
    let r = pi_ / ROD_DIAMETER.max(1e-6);
    dh / 2.0
        * ((1.0 + alpha_vapour_normalized * ((4.0 / core::f64::consts::PI) * (r * r) - 1.0)).r_powf(0.5) - 1.0)
}

/// Per-cell inputs of [`CachardLiquid::value`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CachardLiquidInputs {
    /// `Twall` (the argument the TRACE model swaps in for the transition branch).
    pub t_wall: f64,
    /// `T.interface`.
    pub t_sat: f64,
    /// Liquid temperature.
    pub t_liquid: f64,
    /// `normalized.alpha.vapour`.
    pub alpha_vapour: f64,
    /// Pressure.
    pub p: f64,
    /// Liquid density.
    pub rho_liquid: f64,
    /// `thermo:rho.vapour`.
    pub rho_vapour: f64,
    /// `kappa.vapour`.
    pub kappa_vapour: f64,
    /// `mu.vapour`.
    pub mu_vapour: f64,
    /// `Dh.vapour`.
    pub dh_vapour: f64,
}

/// `CachardLiquid`: wall-to-liquid coefficient of inverted annular flow, and the
/// `hGammaCachard` field it writes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CachardLiquid {
    /// `wallEmissivity`.
    pub wall_emissivity: f64,
    /// `liquidEmissivity`.
    pub liquid_emissivity: f64,
    /// What `pi` is.
    pub pitch: CachardPitch,
}

impl CachardLiquid {
    /// Upstream's model (`pi` = pressure).
    #[must_use]
    pub fn new(wall_emissivity: f64, liquid_emissivity: f64) -> Self {
        Self { wall_emissivity, liquid_emissivity, pitch: CachardPitch::UpstreamPressure }
    }

    /// The same model with the intended pitch instead of the pressure.
    #[must_use]
    pub fn with_pitch(mut self, pitch: f64) -> Self {
        self.pitch = CachardPitch::Pitch(pitch);
        self
    }

    /// `(value, hGammaCachard)` of `CachardLiquid::value(celli)`: the model's coefficient and the
    /// value it writes into `hGammaCachard[celli]`.
    #[must_use]
    pub fn value(&self, i: &CachardLiquidInputs) -> (f64, f64) {
        let pi_ = match self.pitch {
            CachardPitch::UpstreamPressure => i.p,
            CachardPitch::Pitch(x) => x,
        };
        let mut d_t = i.t_wall - i.t_liquid;
        d_t = if d_t >= 0.0 { d_t.max(1e-6) } else { d_t.min(-1e-6) };
        let delta = film_thickness(i.dh_vapour, i.alpha_vapour, pi_);
        let delta_star = delta
            * (i.rho_vapour * GRAVITY * (i.rho_liquid - i.rho_vapour) / (i.mu_vapour.max(1e-6) * i.mu_vapour.max(1e-6)))
                .r_powf(1.0 / 3.0);
        let h_init = i.kappa_vapour / delta.max(1e-6);
        let nu_wl = 0.0_f64.max(1.3 * (0.268 * delta_star.r_powf(0.77) - 0.34));
        let hwl_phase = h_init * nu_wl * (i.t_wall - i.t_sat) / d_t;
        let a = (self.liquid_emissivity * (1.0 - i.alpha_vapour).sqrt()).max(1e-6);
        let b = 1.0 / self.wall_emissivity - 1.0;
        let hwl_rad = SIGMA_SB * (i.t_wall * i.t_wall + i.t_sat * i.t_sat) * (i.t_wall + i.t_sat) / (1.0 / a + b).max(1e-6);
        (hwl_phase + hwl_rad, hwl_phase)
    }
}

/// `CachardVapour`: wall-to-vapour coefficient `2 k_g / delta`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CachardVapour {
    /// What `pi` is.
    pub pitch: CachardPitch,
}

impl CachardVapour {
    /// Upstream's model (`pi` = pressure).
    #[must_use]
    pub fn new() -> Self {
        Self { pitch: CachardPitch::UpstreamPressure }
    }

    /// The same model with the intended pitch instead of the pressure.
    #[must_use]
    pub fn with_pitch(mut self, pitch: f64) -> Self {
        self.pitch = CachardPitch::Pitch(pitch);
        self
    }

    /// `CachardVapour::value(celli)`: `alpha` is the vapour fluid's `normalized()`, `kappa` its
    /// conductivity, `dh` its hydraulic diameter.
    #[must_use]
    pub fn value(&self, alpha: f64, p: f64, kappa: f64, dh: f64) -> f64 {
        let pi_ = match self.pitch {
            CachardPitch::UpstreamPressure => p,
            CachardPitch::Pitch(x) => x,
        };
        let delta = film_thickness(dh, alpha, pi_);
        2.0 * kappa / delta.max(1e-6)
    }
}

impl Default for CachardVapour {
    fn default() -> Self {
        Self::new()
    }
}
