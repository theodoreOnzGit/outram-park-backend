// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/phaseChangeModels/
//             latentHeatModels/{FinkLeibowitz,water,fromThermophysicalProperties}/
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! Specific latent heat of vaporization `h_fg(T)`.
//!
//! Port of GeN-Foam's `latentHeatModel` run-time-selectable family. Every
//! [`LatentHeatModel`] correlation returns a **magnitude** (always physically
//! positive): GeN-Foam's `LSign_` (+1 if fluid1 is the liquid, -1 otherwise)
//! is a property of the fluid-fluid pair being modelled, not of the
//! correlation itself, so applying it is left to the caller — the same
//! separation [`super::rate`] uses for which phase is "driving".
//!
//! **Deferred:** upstream's `latentHeatModel::adjust()` (an optional
//! enthalpy-consistency correction, enabled via the `adjust` dictionary key,
//! that nudges `L` by the difference between a phase's *current* enthalpy and
//! its *saturation* enthalpy so evaporation/condensation stays energy
//! conservative under the solver's explicit mass-transfer discretisation) is
//! **not** ported here. It needs live per-cell enthalpy fields from both
//! phases' thermodynamic packages and the sign of the local `dmdt` field —
//! solver/thermo-package state this module's mandate excludes (see the
//! `phase_change` module docs). Apply it, if needed, at the call site once
//! that state is available.
//!
//! ## Reference
//!
//! - `FinkLeibowitz`: N.P. Fink, L. Leibowitz, "Thermodynamic and Transport
//!   Properties of Sodium Liquid and Vapor", ANL/RE-95/2, 1995,
//!   <https://www.ne.anl.gov/eda/ANL-RE-95-2.pdf>.
//! - `water`: fit (`L = A + B*log(C-T)`) to NIST water saturation data,
//!   <https://www.nist.gov/system/files/documents/srd/NISTIR5078-Tab1.pdf>.

use super::LatentHeat;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

/// Specific latent heat of vaporization correlation.
///
/// Closed enum port of GeN-Foam's `latentHeatModel` family. Evaluate with
/// [`LatentHeatModel::latent_heat`], which takes both the local temperature
/// and the two phases' specific enthalpies even though a given variant may
/// only use one input set — mirrors upstream, where every model has access
/// to the interfacial temperature and (via the phase-change model) both
/// fluids' thermodynamic state regardless of which it reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LatentHeatModel {
    /// Fink & Leibowitz (1995) correlation for liquid sodium, valid
    /// (extrapolated with clamping, matching upstream) for `T` in `[371, 2500] K`.
    FinkLeibowitz,
    /// Fit to NIST water saturation data, `L = A + B*log(C-T)`, valid
    /// (clamped, matching upstream) for `T` in `[273.16, 680] K`. Performs
    /// reasonably to about 638 K even though water's true critical
    /// temperature is 647.096 K (`C = 681.718 K` is the fit's own asymptote,
    /// not the physical critical point — see upstream source comment).
    Water,
    /// `L = h_vapour - h_liquid`: the difference of the two phases' specific
    /// enthalpies (upstream: their enthalpies of formation, `Hf`, from each
    /// phase's `thermophysicalProperties`). The caller supplies both — this
    /// module has no dependency on a thermo package.
    FromThermophysicalProperties,
}

impl LatentHeatModel {
    /// Evaluate the specific latent heat of vaporization.
    ///
    /// `t` is used by [`LatentHeatModel::FinkLeibowitz`] and
    /// [`LatentHeatModel::Water`] (ignored by
    /// [`LatentHeatModel::FromThermophysicalProperties`]); `h_vapour` and
    /// `h_liquid` are used only by
    /// [`LatentHeatModel::FromThermophysicalProperties`] (ignored otherwise).
    /// Faithful translation of each upstream model's `value(celli)` /
    /// `correctField(L)`.
    #[must_use]
    pub fn latent_heat(
        &self,
        t: ThermodynamicTemperature,
        h_vapour: LatentHeat,
        h_liquid: LatentHeat,
    ) -> LatentHeat {
        match *self {
            Self::FinkLeibowitz => {
                LatentHeat::new::<joule_per_kilogram>(fink_leibowitz(t.get::<kelvin>()))
            }
            Self::Water => {
                LatentHeat::new::<joule_per_kilogram>(water_latent_heat(t.get::<kelvin>()))
            }
            Self::FromThermophysicalProperties => h_vapour - h_liquid,
        }
    }
}

/// Fink & Leibowitz sodium latent heat, J/kg.
///
/// `T` clamped to `[371, 2500] K` matching upstream's `min(max(iT_, 371), 2500)`.
fn fink_leibowitz(t_k: f64) -> f64 {
    let t = t_k.clamp(371.0, 2500.0);
    let one_minus_t_over_tc = 1.0 - t / 2503.7;
    393370.0 * one_minus_t_over_tc + 4398600.0 * one_minus_t_over_tc.powf(0.29302)
}

/// Water latent heat fit, J/kg. `T` clamped to `[273.16, 680] K` matching
/// upstream's `min(max(iT_, 273.16), 680)`.
fn water_latent_heat(t_k: f64) -> f64 {
    let t = t_k.clamp(273.16, 680.0);
    1.0e6 * (-2.44 + 0.82 * (681.718 - t).ln())
}
