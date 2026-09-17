// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/fluidDiameterModels/
//             {isomolarBubble/isomolarBubbleFluidDiameter.{H,C},
//             isothermalBubble/isothermalBubbleFluidDiameter.{H,C},
//             pipeFilm/pipeFilmFluidDiameter.{H,C}, WallisFilm/WallisFilmFluidDiameter.{H,C}}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `interfacial::diameter` — bubble/droplet and film characteristic diameter
//!
//! Port of GeN-Foam's `fluidDiameterModels` family, split into **two** closed
//! enums (rather than one) because the four upstream models fall into two
//! physically distinct groups that do not share an input signature:
//!
//! | Enum | Upstream models | Driven by |
//! |---|---|---|
//! | [`BubbleDiameter`] | `isomolarBubble`, `isothermalBubble` | Local pressure (and, for `isomolarBubble`, temperature) via the ideal-gas law |
//! | [`FilmDiameter`] | `pipeFilm`, `WallisFilm` | The phase's own (pair-)normalized void fraction and the structure's hydraulic diameter |
//!
//! Both return the same physical quantity, the dispersed phase's characteristic
//! diameter ([`FluidDiameter`]) — the `Dh` field GeN-Foam feeds into
//! [`super::area::InterfacialArea::area_concentration`]'s `dispersed_diameter`
//! argument.
//!
//! ## `BubbleDiameter` — ideal-gas bubble/droplet resizing
//!
//! Both variants assume a fixed reference diameter `d0` at a reference state
//! `(p0[, T0])` and rescale the diameter as the bubble/droplet expands or
//! contracts isomolarly (`pV/T = const`, i.e. constant moles, no mass
//! exchange) or isothermally (`pV = const` at fixed `T`, i.e. constant mass at
//! constant temperature). Both are cube-root ideal-gas volume scalings —
//! `isothermalBubble` is exactly `isomolarBubble` with the `T/T0` factor
//! dropped.
//!
//! ## `FilmDiameter` — annular-flow film thickness
//!
//! Both variants estimate a film thickness from the phase's own
//! (pair-)normalized void fraction `alphaN` and the structure's hydraulic
//! diameter `Dhs` — `pipeFilm` assumes a film conformal to a circular pipe wall
//! (`d = (1 - sqrt(1-alphaN)) * Dhs`, from the pipe cross-section area balance);
//! `WallisFilm` assumes a linear film-thickness/void-fraction relationship
//! calibrated against the Wallis interfacial-drag coefficient (`d = 0.25 *
//! alphaN * Dhs`; see TRACE theory manual eqns. 4-37/4-38,
//! <https://www.nrc.gov/docs/ML1200/ML120060218.pdf>, pp. 135-136).

use super::units::{fluid_diameter, FluidDiameter};
use uom::si::f64::{Pressure, Ratio, ThermodynamicTemperature};
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

/// Ideal-gas bubble/droplet diameter closure — rescales a reference diameter
/// `d0` (at reference state `p0`[, `T0`]) with local pressure (and, for
/// [`BubbleDiameter::IsomolarBubble`], temperature).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BubbleDiameter {
    /// Constant-moles (isomolar) ideal-gas rescaling:
    /// `d = d0 * cbrt((p0 * T) / (p * T0))`.
    IsomolarBubble {
        /// Reference diameter, m.
        d0_m: f64,
        /// Reference pressure, Pa.
        p0_pa: f64,
        /// Reference temperature, K.
        t0_k: f64,
    },
    /// Constant-mass, constant-temperature (isothermal) ideal-gas rescaling:
    /// `d = d0 * cbrt(p0 / p)`.
    IsothermalBubble {
        /// Reference diameter, m.
        d0_m: f64,
        /// Reference pressure, Pa.
        p0_pa: f64,
    },
}

impl BubbleDiameter {
    /// Evaluate the diameter at the given local `(pressure, temperature)`.
    /// `temperature` is ignored by [`BubbleDiameter::IsothermalBubble`] (it is
    /// constant by construction — see the module docs).
    #[must_use]
    pub fn diameter(
        &self,
        pressure: Pressure,
        temperature: ThermodynamicTemperature,
    ) -> FluidDiameter {
        fluid_diameter(self.value(pressure.get::<pascal>(), temperature.get::<kelvin>()))
    }

    fn value(&self, p: f64, t: f64) -> f64 {
        match *self {
            Self::IsomolarBubble { d0_m, p0_pa, t0_k } => d0_m * ((p0_pa * t) / (p * t0_k)).cbrt(),
            Self::IsothermalBubble { d0_m, p0_pa } => d0_m * (p0_pa / p).cbrt(),
        }
    }
}

/// Annular-flow film-thickness closure — estimates a film thickness from the
/// phase's own (pair-)normalized void fraction and the structure's hydraulic
/// diameter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilmDiameter {
    /// Film conformal to a circular pipe wall:
    /// `d = (1 - sqrt(1 - max(alphaN, residual_alpha))) * Dhs`.
    PipeFilm {
        /// Minimum allowable normalized void fraction, avoiding a null film
        /// thickness at `alphaN = 0` (upstream default `1e-2`).
        residual_alpha: f64,
    },
    /// Wallis interfacial-drag-calibrated linear film thickness:
    /// `d = 0.25 * max(alphaN, residual_alpha) * Dhs`.
    WallisFilm {
        /// Minimum allowable normalized void fraction (upstream default `1e-2`).
        residual_alpha: f64,
    },
}

impl FilmDiameter {
    /// Evaluate the film thickness given the phase's (pair-)normalized void
    /// fraction and the structure hydraulic diameter `Dhs`.
    #[must_use]
    pub fn diameter(
        &self,
        alpha_normalized: Ratio,
        structure_hydraulic_diameter: FluidDiameter,
    ) -> FluidDiameter {
        fluid_diameter(self.value(
            alpha_normalized.get::<ratio>(),
            structure_hydraulic_diameter.get::<uom::si::length::meter>(),
        ))
    }

    fn value(&self, alpha_n: f64, dhs: f64) -> f64 {
        match *self {
            Self::PipeFilm { residual_alpha } => {
                (1.0 - (1.0 - alpha_n.max(residual_alpha)).sqrt()) * dhs
            }
            Self::WallisFilm { residual_alpha } => 0.25 * alpha_n.max(residual_alpha) * dhs,
        }
    }
}
