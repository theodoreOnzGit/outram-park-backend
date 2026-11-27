// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da0cc55
//   Upstream source: src/classes/neutronics/XS/XS/XS.C (`XS::adjustDiscFactors`,
//                    members `dfAdjustList_`, `discFactorList_`,
//                    `integralFluxList_`, `adjustDiscFactors_`,
//                    `useGivenDiscFactors_`, `groupsWoDF_`),
//                    include/adjustDiscFactors.H, include/readNuclearData.H,
//                    include/setNeutronicsVariables.H (the `discFactor_` fill)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal authors: Carlo Fiorina, Thomas Guilbaud (EPFL)
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

//! Discontinuity-factor adjustment of GeN-Foam's `XS` class (gh:#1785, child of #1216).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! ## What upstream does (read before any code, 2026-11-27)
//!
//! * Every reference zone carries a per-group `discFactor` list (`readNuclearData.H:100`) and an
//!   `integralFlux` list (`:101`), and a `dfAdjust` flag (`:96`, default `true`).
//! * `XS::correct()` (`setNeutronicsVariables.H`, end of the cell loop) fills the per-cell field
//!   `discFactor_[g][cell] = discFactorList_[zone][g]` **when `useGivenDiscFactors` or
//!   `timeIndex - startTimeIndex > 0`**; otherwise it does the reverse and overwrites the list by the
//!   field (initial value 1). A solver always calls `correct()` after `runTime++`, so inside a run the
//!   field is the **list as read from the dictionary, adjusted or not**: a shipped `discFactor` that is
//!   not 1 (2D_FFTF has 24-group lists) enters every equation even with `adjustDiscFactors false`.
//!   The port had no discontinuity factor at all before this module (every solver divided by 1).
//! * `adjustDiscFactors true` additionally calls `xs_.adjustDiscFactors(fluxStar_)` at the end of each
//!   `eigenvalueNeutronics` step (`diffusionNeutronics`: `solveNeutronics.H:128`, after `normFluxes.H`;
//!   `SP3Neutronics`: `solveNeutronicsSP3.H:119`). For every group not in `groupsWoDF` it sets, for each
//!   zone with `dfAdjust` whose cell zone exists in the mesh,
//!   `df[zone][g] = (int_zone fluxStar_g / int_adjusted fluxStar_g) / (integralFlux[zone][g] / sum integralFlux[g])`,
//!   the integrals being `fvc::domainIntegrate(indicator * fluxStar)` = a sequential
//!   `sum_c (V_c * (indicator_c * fluxStar_c))`, and the sum of `integralFlux` running over the same
//!   zones in dictionary order. The new list takes effect at the NEXT step's `correct()`.
//!
//! ## Port
//!
//! [`DiscFactorState`] owns the zone lists and the zone cells (the mesh's `cellZones`, by name, not the
//! driveline-remapped zones: upstream's `adjustDiscFactors.H` walks `mesh_.cellZones()[zoneId]`), and
//! [`DiscFactorState::adjust`] is `adjustDiscFactors.H` line by line. The `useGivenDiscFactors == false`
//! and `timeIndex == startTimeIndex` branch (the field overwrites the list with 1) is NOT ported: it
//! only runs when `correct()` is called before the first `runTime++`, which no solver does; the
//! `dump_xs` probe of round 6 is the one caller that does, which is why it printed 1 for every
//! `discFactor`.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::{CrossSectionData, XsError};

/// The three top-level `nuclearData` keys of the discontinuity-factor adjustment
/// (`XS.C:163-165`): `adjustDiscFactors` (default false), `useGivenDiscFactors` (default false) and
/// `groupsWoDF` (default empty).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DiscFactorControls {
    /// `adjustDiscFactors`.
    pub adjust_disc_factors: bool,
    /// `useGivenDiscFactors` (only matters at `timeIndex == startTimeIndex`, see the module doc).
    pub use_given_disc_factors: bool,
    /// `groupsWoDF`: groups whose factor is never adjusted.
    pub groups_wo_df: Vec<usize>,
}

/// `discFactorList_` and what `XS::adjustDiscFactors` needs to update it.
#[derive(Debug, Clone)]
pub struct DiscFactorState {
    controls: DiscFactorControls,
    energy_groups: usize,
    df_adjust: Vec<bool>,
    integral_flux: Vec<Vec<f64>>,
    values: Vec<Vec<f64>>,
    zone_cells: Vec<Option<Vec<usize>>>,
}

impl DiscFactorState {
    /// Read `discFactorList_`, `integralFluxList_` and `dfAdjustList_` from `xs`'s reference zones.
    /// `zone_cells[z]` is the mesh cell zone named like reference zone `z`, `None` when the mesh has no
    /// such zone (`findZoneID == -1`: upstream skips it, "a completely withdrawn control rod").
    ///
    /// # Errors
    ///
    /// [`XsError::IndexOutOfRange`] if `zone_cells` does not hold one entry per zone or a `groupsWoDF`
    /// entry is not a group.
    pub fn new(
        xs: &CrossSectionData,
        controls: DiscFactorControls,
        zone_cells: Vec<Option<Vec<usize>>>,
    ) -> Result<Self, XsError> {
        let zones = xs.zone_count();
        if zone_cells.len() != zones {
            return Err(XsError::IndexOutOfRange {
                what: "zone_cells length vs reference zones",
                index: zone_cells.len(),
                count: zones,
            });
        }
        let groups = xs.energy_groups();
        for &g in &controls.groups_wo_df {
            if g >= groups {
                return Err(XsError::IndexOutOfRange { what: "groupsWoDF entry", index: g, count: groups });
            }
        }
        let mut df_adjust = Vec::with_capacity(zones);
        let mut integral_flux = Vec::with_capacity(zones);
        let mut values = Vec::with_capacity(zones);
        for z in 0..zones {
            let zone = xs.zone(z).ok_or(XsError::IndexOutOfRange { what: "zone", index: z, count: zones })?;
            df_adjust.push(zone.df_adjust());
            integral_flux.push((0..groups).map(|g| zone.integral_flux(g)).collect());
            values.push((0..groups).map(|g| zone.disc_factor(g)).collect());
        }
        Ok(Self { controls, energy_groups: groups, df_adjust, integral_flux, values, zone_cells })
    }

    /// The controls.
    #[must_use]
    pub fn controls(&self) -> &DiscFactorControls {
        &self.controls
    }

    /// `discFactorList_[zone][group]`.
    #[must_use]
    pub fn value(&self, zone: usize, group: usize) -> f64 {
        self.values[zone][group]
    }

    /// Every zone's list, `[zone][group]`.
    #[must_use]
    pub fn values(&self) -> &[Vec<f64>] {
        &self.values
    }

    /// Whether every factor is exactly 1 (then every equation divides by 1 and nothing changes).
    #[must_use]
    pub fn is_unity(&self) -> bool {
        self.values.iter().all(|z| z.iter().all(|&v| v == 1.0))
    }

    /// Whether [`Self::adjust`] would change anything (`adjustDiscFactors true`).
    #[must_use]
    pub fn adjusts(&self) -> bool {
        self.controls.adjust_disc_factors
    }

    /// `adjustDiscFactors.H`: update the lists from the normalised `fluxStar` (one internal-value
    /// vector per group) and the cell volumes. Does nothing unless `adjustDiscFactors` is set
    /// (`XS::adjustDiscFactors`, `XS.C:328`).
    pub fn adjust(&mut self, flux_star: &[Vec<f64>], cell_volumes: &[f64]) {
        if !self.controls.adjust_disc_factors {
            return;
        }
        let n = cell_volumes.len();
        for energy_i in 0..self.energy_groups {
            if self.controls.groups_wo_df.contains(&energy_i) {
                continue;
            }
            let mut indicator = vec![0.0_f64; n];
            let mut tot_flux_serpent = 0.0_f64;
            for zone_i in 0..self.zone_cells.len() {
                if let Some(cells) = &self.zone_cells[zone_i] {
                    if self.df_adjust[zone_i] {
                        for &c in cells {
                            indicator[c] = 1.0;
                        }
                        tot_flux_serpent += self.integral_flux[zone_i][energy_i];
                    }
                }
            }
            let tot_flux_gen_foam = domain_integrate(&indicator, &flux_star[energy_i], cell_volumes);

            let mut indicator = vec![0.0_f64; n];
            for zone_i in 0..self.zone_cells.len() {
                let Some(cells) = &self.zone_cells[zone_i] else {
                    continue;
                };
                if self.df_adjust[zone_i] {
                    for &c in cells {
                        indicator[c] = 1.0;
                    }
                    self.values[zone_i][energy_i] =
                        (domain_integrate(&indicator, &flux_star[energy_i], cell_volumes) / tot_flux_gen_foam)
                            / (self.integral_flux[zone_i][energy_i] / tot_flux_serpent);
                }
                for v in indicator.iter_mut() {
                    *v *= 0.0;
                }
            }
        }
    }

    /// The per-cell field `discFactor_[group]` for the cell -> zone map `xs_zone_of_cell` (the zone each
    /// cell's cross sections come from, after any driveline remapping): `discFactorList_[zone][group]`.
    #[must_use]
    pub fn cell_values(&self, xs_zone_of_cell: &[usize], group: usize) -> Vec<f64> {
        xs_zone_of_cell.iter().map(|&z| self.values[z][group]).collect()
    }
}

/// `fvc::domainIntegrate(indicator * flux)`: `gSum(V * (indicator * flux))`, summed from the first cell
/// (`Foam::sum` is a sequential `+=`, `FieldFunctions.C:403`).
#[must_use]
pub fn domain_integrate(indicator: &[f64], flux: &[f64], cell_volumes: &[f64]) -> f64 {
    let mut sum = 0.0_f64;
    for c in 0..cell_volumes.len() {
        let product = indicator[c] * flux[c];
        sum += cell_volumes[c] * product;
    }
    sum
}
