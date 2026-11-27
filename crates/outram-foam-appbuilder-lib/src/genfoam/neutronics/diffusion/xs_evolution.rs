// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da0cc55
//   Upstream source: src/classes/neutronics/XS/XS/XS.C (`XS::correct`,
//                    `XS::adjustDiscFactors`), include/setNeutronicsVariables.H,
//                    include/adjustDiscFactors.H
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK (GPL-3.0-only, see the other files of this module).

//! The time-dependent part of `XS::correct()` for the diffusion pseudo-time driver (gh:#1785,
//! gh:#1474): control-rod driveline motion and discontinuity factors.
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! [`XsEvolution`] holds what `XS::correct()` needs besides the flux: the cross-section data, the mesh
//! zone of every cell, the rods, the discontinuity-factor lists, and the (static) per-cell feedback
//! parameters. [`XsEvolution::materialize`] is `correct()` at time `t`;
//! [`super::DiffusionNeutronics`] calls it at the start of every eigenvalue time step, after `pTotOld_`
//! was taken, as upstream does. Feedback fields that change in time (`TFuel`, `rhoCool`: they come from
//! the thermal-hydraulics region) are refused here by construction: `cell_parameters` is fixed; use
//! [`super::DiffusionNeutronics::refresh_cross_sections`] for coupled feedback.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::sync::Arc;

use outram_foam_basic_lib::prelude::FvMesh;

use super::DiffusionXsFields;
use crate::genfoam::neutronics::xs::disc_factors::{DiscFactorControls, DiscFactorState};
use crate::genfoam::neutronics::xs::driveline::{ControlRod, ControlRodDriveline};
use crate::genfoam::neutronics::xs::{CrossSectionData, XsError};

/// See the module documentation.
#[derive(Debug, Clone)]
pub struct XsEvolution {
    xs: CrossSectionData,
    zone_names: Vec<String>,
    mesh_zone_of_cell: Vec<usize>,
    cell_z: Vec<f64>,
    cell_parameters: Vec<f64>,
    driveline: Option<ControlRodDriveline>,
    disc: DiscFactorState,
}

impl XsEvolution {
    /// Build the evolution of `xs` on `mesh`. `mesh_zone_of_cell[c]` is the reference zone of cell `c`'s
    /// own cellZone, `cell_zones` lists every mesh cellZone as `(name, cells)`, `cell_parameters` is the
    /// static `n_cells x n_variables` feedback state (empty for feedback-free data).
    ///
    /// # Errors
    ///
    /// [`XsError`] for a rod whose follower cellZone is not in the mesh (upstream indexes
    /// `cellZones()[-1]`) or a size mismatch.
    pub fn new(
        xs: &CrossSectionData,
        mesh: &Arc<FvMesh>,
        mesh_zone_of_cell: Vec<usize>,
        cell_zones: &[(String, Vec<usize>)],
        cell_parameters: Vec<f64>,
        rods: Vec<ControlRod>,
        controls: DiscFactorControls,
    ) -> Result<Self, XsError> {
        let zone_names: Vec<String> = (0..xs.zone_count())
            .map(|i| xs.zone(i).map(|z| z.name().to_string()))
            .collect::<Option<Vec<_>>>()
            .ok_or(XsError::IndexOutOfRange { what: "zone", index: 0, count: 0 })?;
        let cell_z: Vec<f64> = mesh.cell_centres.iter().map(|c| c.z).collect();
        let zone_cells: Vec<Option<Vec<usize>>> = zone_names
            .iter()
            .map(|name| cell_zones.iter().find(|(n, _)| n == name).map(|(_, cells)| cells.clone()))
            .collect();
        let driveline = if rods.is_empty() {
            None
        } else {
            let mut followers = Vec::with_capacity(rods.len());
            for rod in &rods {
                let cells = cell_zones
                    .iter()
                    .find(|(n, _)| *n == rod.follower_name)
                    .map(|(_, cells)| cells.clone())
                    .ok_or(XsError::IndexOutOfRange { what: "follower cellZone", index: 0, count: 0 })?;
                followers.push(cells);
            }
            Some(ControlRodDriveline::new(rods, &followers, &cell_z)?)
        };
        let disc = DiscFactorState::new(xs, controls, zone_cells)?;
        Ok(Self { xs: xs.clone(), zone_names, mesh_zone_of_cell, cell_z, cell_parameters, driveline, disc })
    }

    /// The discontinuity-factor state.
    #[must_use]
    pub fn disc(&self) -> &DiscFactorState {
        &self.disc
    }

    /// The driveline, if any rod is defined.
    #[must_use]
    pub fn driveline(&self) -> Option<&ControlRodDriveline> {
        self.driveline.as_ref()
    }

    /// The zone every cell's variable cross sections come from at time `t`.
    #[must_use]
    pub fn xs_zone_of_cell(&self, t: f64) -> Vec<usize> {
        match &self.driveline {
            Some(d) => d.effective_zone_of_cell(&self.zone_names, &self.mesh_zone_of_cell, &self.cell_z, t),
            None => self.mesh_zone_of_cell.clone(),
        }
    }

    /// `XS::correct()` at time `t` (with `timeIndex > startTimeIndex`): the cross-section fields with the
    /// driveline-remapped zones and the current discontinuity-factor lists.
    ///
    /// # Errors
    ///
    /// [`XsError`] from the evaluation.
    pub fn materialize(&self, mesh: &Arc<FvMesh>, t: f64) -> Result<DiffusionXsFields, XsError> {
        let xs_zone = self.xs_zone_of_cell(t);
        DiffusionXsFields::materialize_evolved(
            mesh,
            &self.xs,
            &xs_zone,
            &self.mesh_zone_of_cell,
            &self.cell_parameters,
            Some(self.disc.values()),
        )
    }

    /// `xs_.adjustDiscFactors(fluxStar_)`.
    pub fn adjust_disc_factors(&mut self, flux_star: &[Vec<f64>], cell_volumes: &[f64]) {
        self.disc.adjust(flux_star, cell_volumes);
    }
}
