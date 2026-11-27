// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da0cc55
//   Upstream source: src/classes/neutronics/XS/XS/XS.C (members `CRmove_`,
//                    `CRentries_`, `CRstart_`, `CRfinish_`, `CRspeed_`,
//                    `CRFollowerName_`, `initialDistanceFromMeshCR_`),
//                    include/readNuclearData.H (control-rod block),
//                    include/setNeutronicsVariables.H (position and zone remap)
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

//! Control-rod driveline motion of GeN-Foam's `XS` class (gh:#1474, child of #1216).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! (This is the cross-section rod MOTION of `constant/<region>/CRmove`; the point-kinetics
//! driveline REACTIVITY of round 36 is `point_kinetics::PointKineticNeutronics::calc_driveline_reactivity`.)
//!
//! ## What upstream does
//!
//! `constant/neutroRegion/CRmove` (read if present) lists `zones ( rod { startTime; endTime; speed;
//! initialDistanceFromMeshCR; followerName; } )`. Each rod is a cellZone and its follower another one,
//! both also reference zones of `nuclearData`. At every `XS::correct()` (`setNeutronicsVariables.H`):
//!
//! 1. `maxFollowerHeight` = the largest cell-centre `z` of the follower cellZone (a running maximum that
//!    starts at -100), `CRinitialPosition = maxFollowerHeight - initialDistanceFromMeshCR`;
//! 2. `CRposition = CRinitialPosition - speed (max(t, start) - start)` while `t < endTime`, else
//!    `CRinitialPosition - speed (end - start)` (`t` = `mesh.time().value()`);
//! 3. for the cells of a reference zone that is a follower: a cell with `z > CRposition` takes its
//!    cross sections (`D`, `nuSigmaEff`, `sigmaPow`, `sigmaRemoval`, `chi`, `sigmaFromTo`, `discFactor`)
//!    from the rod's reference zone; for the cells of a rod zone, a cell with `z < CRposition` takes them
//!    from the follower's zone. `IV`, `Beta`, `lambda`, `BetaTot` (`setNeutronicsConstants.H`,
//!    `setPrecConst.H`) stay with the cell's own mesh zone, never remapped.
//!
//! [`ControlRodDriveline::effective_zone_of_cell`] returns step 3's per-cell zone. A follower cellZone
//! that does not exist makes upstream index `cellZones()[-1]`; here it is an error.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::XsError;

/// One `CRmove` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct ControlRod {
    /// The rod's cellZone name (the entry keyword).
    pub name: String,
    /// `startTime` (default 0).
    pub start_time: f64,
    /// `endTime` (default 0).
    pub end_time: f64,
    /// `speed` (default 0), positive entering the core.
    pub speed: f64,
    /// `followerName`.
    pub follower_name: String,
    /// `initialDistanceFromMeshCR` (default 0).
    pub initial_distance_from_mesh_cr: f64,
}

/// The rods of `CRmove` with the geometry they need.
#[derive(Debug, Clone)]
pub struct ControlRodDriveline {
    rods: Vec<ControlRod>,
    max_follower_height: Vec<f64>,
}

impl ControlRodDriveline {
    /// `follower_cells[r]` are the cells of rod `r`'s follower cellZone, `cell_z[c]` the `z` of cell
    /// `c`'s centre (`mesh.C() & (0,0,1)`).
    ///
    /// # Errors
    ///
    /// [`XsError::IndexOutOfRange`] if `follower_cells` is not one entry per rod, or a follower cell is
    /// not a cell.
    pub fn new(
        rods: Vec<ControlRod>,
        follower_cells: &[Vec<usize>],
        cell_z: &[f64],
    ) -> Result<Self, XsError> {
        if follower_cells.len() != rods.len() {
            return Err(XsError::IndexOutOfRange {
                what: "follower_cells length vs rods",
                index: follower_cells.len(),
                count: rods.len(),
            });
        }
        let mut max_follower_height = Vec::with_capacity(rods.len());
        for cells in follower_cells {
            let mut cell_height = -100.0_f64;
            let mut max_height = cell_height;
            for &c in cells {
                let z = *cell_z.get(c).ok_or(XsError::IndexOutOfRange {
                    what: "follower cell",
                    index: c,
                    count: cell_z.len(),
                })?;
                max_height = cell_height.max(z);
                cell_height = max_height;
            }
            max_follower_height.push(max_height);
        }
        Ok(Self { rods, max_follower_height })
    }

    /// The rods.
    #[must_use]
    pub fn rods(&self) -> &[ControlRod] {
        &self.rods
    }

    /// `CRposition_` of every rod at time `t`.
    #[must_use]
    pub fn positions(&self, t: f64) -> Vec<f64> {
        self.rods
            .iter()
            .zip(&self.max_follower_height)
            .map(|(rod, &max_h)| {
                let initial = max_h - rod.initial_distance_from_mesh_cr;
                if t < rod.end_time {
                    initial - rod.speed * (t.max(rod.start_time) - rod.start_time)
                } else {
                    initial - rod.speed * (rod.end_time - rod.start_time)
                }
            })
            .collect()
    }

    /// The reference zone each cell's variable cross sections come from at time `t`
    /// (`setNeutronicsVariables.H`). `zone_names[z]` names reference zone `z`; `mesh_zone_of_cell[c]`
    /// is the zone of cell `c`'s own cellZone.
    #[must_use]
    pub fn effective_zone_of_cell(
        &self,
        zone_names: &[String],
        mesh_zone_of_cell: &[usize],
        cell_z: &[f64],
        t: f64,
    ) -> Vec<usize> {
        let positions = self.positions(t);
        let find = |name: &str| zone_names.iter().position(|n| n == name);
        let mut out = mesh_zone_of_cell.to_vec();
        for (c, slot) in out.iter_mut().enumerate() {
            let zone_i = mesh_zone_of_cell[c];
            let name = &zone_names[zone_i];
            let mut is_follower = false;
            let mut is_cr = false;
            let mut cr_zone = 0usize;
            for (r, rod) in self.rods.iter().enumerate() {
                if *name == rod.name {
                    is_cr = true;
                    cr_zone = r;
                }
                if *name == rod.follower_name {
                    is_follower = true;
                    cr_zone = r;
                }
            }
            let mut zone = zone_i;
            if !self.rods.is_empty() {
                if is_follower && cell_z[c] > positions[cr_zone] {
                    if let Some(z) = find(&self.rods[cr_zone].name) {
                        zone = z;
                    }
                }
                if is_cr && cell_z[c] < positions[cr_zone] {
                    if let Some(z) = find(&self.rods[cr_zone].follower_name) {
                        zone = z;
                    }
                }
            }
            *slot = zone;
        }
        out
    }
}
