// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/physicsModels/heatTransferModels/
//             FFHeatTransferCoefficientModels/Nusselt/
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `ff_htc` — fluid-fluid (interfacial) forced-convection HTC correlation
//!
//! [`FfForcedConvectionHtc`] ports upstream's `FFHeatTransferCoefficientModels::Nusselt`
//! — the interfacial-transfer analogue of [`super::fs_htc::FsForcedConvectionHtc::Nusselt`],
//! same `Nu = A + B*Re^C*Pr^D` form (no wall-temperature-ratio term; there is
//! no "wall" at a fluid-fluid interface), `h = Nu*k/D_h`, evaluated with the
//! **dispersed**-phase Reynolds number and hydraulic diameter and the
//! **continuous**-phase Prandtl number and thermal conductivity (matching
//! upstream's `pair.PrContinuous()`/`pair.DhDispersed()` field selection —
//! the two phases play asymmetric roles at an interface, unlike a
//! fluid-structure pair).
//!
//! ## Deferred: `NoKazimi`
//!
//! Upstream's `FFHeatTransferCoefficientModels::NoKazimi` (interfacial
//! condensation/evaporation htc, taking the lesser of a continuum
//! correlation and a kinetic-theory/molecular-effusion limit) is **not**
//! ported. Its continuum-limit term multiplies by `bulkFluid_[celli]` — a
//! `fluid::operator[]` call whose return semantics are not documented in
//! `FFHeatTransferCoefficientModel.H`/`NoKazimiFFHeatTransferCoefficient.H`
//! and are not derivable from either header (the void-fraction-weighted
//! `normalized()` accessor is a *separate*, already-used member,
//! `alpha_(bulkFluid_.normalized())`, so `operator[]` is not simply that).
//! Confirming its meaning needs the upstream `fluid` class implementation,
//! which is out of this bead's file scope (it belongs to
//! [`super::super::super::phase`], and per the task brief this port does not
//! reach into sibling phase modules). Porting a physics correlation on a
//! guessed interpretation would risk exactly the "fabricated result" this
//! workspace's compliance rules prohibit, so it is omitted rather than
//! half-stubbed. Tracked for follow-up once the `phase`/`fluid` port exposes
//! (and documents) that accessor.

use super::PrandtlNumber;
use crate::genfoam::thermal_hydraulics::units::{HeatTransferCoefficient, ReynoldsNumber};
use uom::si::f64::{Length, ThermalConductivity};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;

/// Fluid-fluid (interfacial) forced-convection heat-transfer coefficient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FfForcedConvectionHtc {
    /// `Nu = A + B * Re^C * Pr^D`.
    Nusselt {
        /// Additive constant `A`.
        a: f64,
        /// Leading coefficient `B`. `0.0` skips the `Re`/`Pr` term (`Nu = A`).
        b: f64,
        /// Reynolds-number exponent `C`.
        c: f64,
        /// Prandtl-number exponent `D`.
        d: f64,
    },
}

impl FfForcedConvectionHtc {
    /// Evaluate `h = Nu * k / D_h`. `re`, `pr` are the dispersed-phase
    /// Reynolds number and continuous-phase Prandtl number; `k` the
    /// continuous-phase thermal conductivity; `d_h` the dispersed-phase
    /// hydraulic diameter (see the module doc for why the two phases play
    /// asymmetric roles here). Faithful translation of upstream's `value()`,
    /// including its `usePeclet_` fast path (`C == D` collapses to
    /// `(Re*Pr)^C`) and the `Dhi = max(Dh, 1e-4)` floor that guards the
    /// division against a vanishing hydraulic diameter as the dispersed
    /// phase disappears.
    #[must_use]
    pub fn heat_transfer_coefficient(
        &self,
        re: ReynoldsNumber,
        pr: PrandtlNumber,
        k: ThermalConductivity,
        d_h: Length,
    ) -> HeatTransferCoefficient {
        let Self::Nusselt { a, b, c, d } = *self;
        let dh_v = d_h.get::<meter>().max(1e-4);
        let re_v = re.get::<ratio>();
        let pr_v = pr.get::<ratio>();
        let nu = if b != 0.0 {
            let re_c_pr_d = if c == d {
                (re_v * pr_v).powf(c)
            } else {
                re_v.powf(c) * pr_v.powf(d)
            };
            a + b * re_c_pr_d
        } else {
            a
        };
        HeatTransferCoefficient::new::<watt_per_square_meter_kelvin>(
            nu * k.get::<watt_per_meter_kelvin>() / dh_v,
        )
    }
}
