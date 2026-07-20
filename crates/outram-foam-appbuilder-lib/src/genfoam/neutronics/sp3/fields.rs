// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/SP3/SP3Neutronics.C (XS field
//                    members) and include/{createFluxMatricesSP3,fluxEqSP3}.H
//                    (the second-moment diffusion coefficient
//                    D2 = (3/7)/Sigma_t and the second-moment removal
//                    coefficient), plus src/classes/neutronics/include/
//                    setXSFields.H (materialisation of the group-wise XS).
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//     Principal author of the SP3 files: Carlo Fiorina (EPFL).
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

//! # Mesh-materialised SP3 cross-section fields
//!
//! The SP3 second-moment equation needs two group constants that the plain
//! diffusion field set does not carry:
//!
//! - the **total** cross section `Sigma_t = Sigma_r + Sigma_{s,g->g}` (removal
//!   plus self-scatter — GeN-Foam writes it as
//!   `sigmaRemoval[g] + sigmaFromTo[g][g]`), and
//! - the **second-moment diffusion coefficient** `D_2 = (3/7)/Sigma_t`
//!   (GeN-Foam `Dalbedo_ = 1/(sigmaRemoval+sigmaFromTo)*3/7` in `fluxEqSP3.H`),
//! - the **second-moment removal coefficient**
//!   `A_2 = (5/3) Sigma_t + (4/3) Sigma_r` (the `fvm::Sp` diagonal of the second
//!   SP3 equation).
//!
//! Everything else (the 0th-moment diffusion coefficient `D_0 = D`, the removal
//! `Sigma_r`, the fission / scattering / precursor data) is exactly the diffusion
//! field set, so this module **wraps**
//! [`DiffusionXsFields`](crate::genfoam::neutronics::diffusion::DiffusionXsFields)
//! and adds only the SP3-specific derived fields. It re-uses the diffusion
//! materialisation verbatim (single fixed feedback point; live feedback is the
//! same documented deferral as diffusion).

use std::sync::Arc;

use outram_foam_basic_lib::prelude::{
    fvc, BoundaryCondition, Field, FvMesh, PatchField, SurfaceScalarField, VolScalarField,
};

use crate::genfoam::neutronics::diffusion::DiffusionXsFields;
use crate::genfoam::neutronics::xs::{CrossSectionData, XsError};

/// Build a per-cell [`VolScalarField`] with zero-gradient boundaries (an XS
/// coefficient field never carries a boundary condition of its own).
fn cell_field(name: impl Into<String>, mesh: &Arc<FvMesh>, values: Vec<f64>) -> VolScalarField {
    let boundary = mesh
        .patches
        .iter()
        .map(|p| PatchField {
            bc: BoundaryCondition::ZeroGradient,
            values: Field::uniform(p.size, 0.0),
        })
        .collect();
    VolScalarField::new(name, mesh.clone(), Field::new(values), boundary)
}

/// The full SP3 cross-section field set on a mesh: the diffusion field set plus
/// the three second-moment derived fields.
///
/// All base-SI units follow [`DiffusionXsFields`]; the added fields are
/// `sigma_total` in `m^-1`, `d2` (and its face interpolant `d2_face`) in metres,
/// and `second_moment_removal` in `m^-1`.
#[derive(Debug, Clone)]
pub struct Sp3XsFields {
    /// The shared diffusion field set (`D_0`, `Sigma_r`, fission, scattering,
    /// precursors, ...). SP3's 0th-moment equation uses these directly.
    pub base: DiffusionXsFields,
    /// Total cross section `Sigma_t = Sigma_r + Sigma_{s,g->g}` per group
    /// (`m^-1`). GeN-Foam `sigmaRemoval[g] + sigmaFromTo[g][g]`.
    pub sigma_total: Vec<VolScalarField>,
    /// Second-moment diffusion coefficient `D_2 = (3/7)/Sigma_t` per group (m).
    pub d2: Vec<VolScalarField>,
    /// `D_2` interpolated to faces, ready for
    /// [`fvm::laplacian`](outram_foam_basic_lib::fv_operators::fvm::laplacian).
    pub d2_face: Vec<SurfaceScalarField>,
    /// Second-moment removal coefficient
    /// `A_2 = (5/3) Sigma_t + (4/3) Sigma_r` per group (`m^-1`) — the diagonal
    /// of the second SP3 equation.
    pub second_moment_removal: Vec<VolScalarField>,
}

impl Sp3XsFields {
    /// Materialise the SP3 cross sections onto `mesh` at the feedback point
    /// `raw_parameters`.
    ///
    /// `zone_of_cell[c]` is the material-zone index of cell `c`; its length must
    /// equal `mesh.n_cells`. `raw_parameters` is the feedback vector in
    /// [`CrossSectionData::variable_names`] order (`&[]` for feedback-free data).
    ///
    /// # Errors
    ///
    /// [`XsError`] if the underlying diffusion materialisation fails (bad
    /// zone/group index, singular RBF, or `zone_of_cell` length mismatch).
    pub fn materialize(
        mesh: &Arc<FvMesh>,
        xs: &CrossSectionData,
        zone_of_cell: &[usize],
        raw_parameters: &[f64],
    ) -> Result<Self, XsError> {
        let base = DiffusionXsFields::materialize(mesh, xs, zone_of_cell, raw_parameters)?;
        let g = base.energy_groups;
        let n = mesh.n_cells;

        let mut sigma_total = Vec::with_capacity(g);
        let mut d2 = Vec::with_capacity(g);
        let mut second_moment_removal = Vec::with_capacity(g);

        for group in 0..g {
            let sr = base.sigma_removal[group].internal.as_slice();
            // Self-scatter Sigma_{s, g->g} is scattering[from g][into g].
            let ss = base.scattering[group][group].internal.as_slice();
            let mut vst = vec![0.0; n];
            let mut vd2 = vec![0.0; n];
            let mut va2 = vec![0.0; n];
            for c in 0..n {
                let sigma_t = sr[c] + ss[c];
                vst[c] = sigma_t;
                // D_2 = (3/7)/Sigma_t; guard a void (Sigma_t -> 0) cell.
                vd2[c] = if sigma_t > 0.0 {
                    (3.0 / 7.0) / sigma_t
                } else {
                    0.0
                };
                va2[c] = (5.0 / 3.0) * sigma_t + (4.0 / 3.0) * sr[c];
            }
            sigma_total.push(cell_field(format!("sigmaTotal{group}"), mesh, vst));
            d2.push(cell_field(format!("D2_{group}"), mesh, vd2));
            second_moment_removal.push(cell_field(format!("A2_{group}"), mesh, va2));
        }

        let d2_face = d2.iter().map(fvc::interpolate).collect();

        Ok(Self {
            base,
            sigma_total,
            d2,
            d2_face,
            second_moment_removal,
        })
    }

    /// Number of energy groups `G`.
    #[must_use]
    pub fn energy_groups(&self) -> usize {
        self.base.energy_groups
    }

    /// Number of delayed-neutron precursor groups `N`.
    #[must_use]
    pub fn prec_groups(&self) -> usize {
        self.base.prec_groups
    }
}
