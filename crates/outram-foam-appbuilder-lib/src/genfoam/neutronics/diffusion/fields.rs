// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/diffusion/diffusionNeutronics.C
//                    (XS field members) and
//                    include/setNeutronicsVariables.H / setXSFields.H
//                    (materialisation of the group-wise volScalarField XS
//                     onto the mesh cells).
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

//! # Mesh-materialised multigroup cross-section fields
//!
//! GeN-Foam's `diffusionNeutronics` stores its cross sections as
//! `PtrList<volScalarField>` — one field per energy group, filled cell-by-cell
//! from each cell's material zone (`setNeutronicsVariables.H`). This module is
//! the Rust analogue: it "bakes" the mesh-free per-zone
//! [`CrossSectionData`](crate::genfoam::neutronics::xs::CrossSectionData) onto
//! the mesh via a `cell -> zone` map, producing the
//! [`outram_foam_basic_lib::fields::VolScalarField`] bundle the FV assembly
//! consumes.
//!
//! ## Scope: a single fixed feedback point (feedback fields deferred)
//!
//! Materialisation happens at **one** feedback state supplied as
//! `raw_parameters` (fuel/coolant temperature, density, ...). Every cross
//! section is evaluated there, so the fields are *spatially* heterogeneous
//! (different zones give different constants) but the feedback state itself is
//! uniform and static. Per-cell feedback driven by live thermal-hydraulics
//! fields — GeN-Foam's `xs_.correct(TFuel_, ...)` each time step — needs the
//! TH / multi-region coupling layer and is deferred (see the `neutronics` epic
//! and the `xs` module docs). For a benchmark whose `nuclearData` declares no
//! feedback variables, pass `raw_parameters = &[]` and every value equals the
//! `reference` state.

use std::sync::Arc;

use outram_foam_basic_lib::prelude::{
    fvc, BoundaryCondition, Field, FvMesh, PatchField, SurfaceScalarField, VolScalarField,
};

use crate::genfoam::neutronics::xs::{CrossSectionData, XsError};

/// The full group-wise cross-section field set for a diffusion solve, baked
/// onto a mesh.
///
/// Every `Vec<VolScalarField>` is indexed by energy group `g` in the same order
/// as [`CrossSectionData`]. All fields carry the base-SI units documented on
/// [`CrossSectionData`]'s typed accessors (`D` in metres, cross sections in
/// `m^-1`, `sigma_pow` in `J/m`, inverse velocity in `s/m`).
#[derive(Debug, Clone)]
pub struct DiffusionXsFields {
    /// Number of energy groups `G`.
    pub energy_groups: usize,
    /// Number of delayed-neutron precursor groups `N`.
    pub prec_groups: usize,
    /// Diffusion coefficient `D_g` per group (m).
    pub d: Vec<VolScalarField>,
    /// `D_g` interpolated to faces, ready for
    /// [`fvm::laplacian`](outram_foam_basic_lib::fv_operators::fvm::laplacian).
    pub d_face: Vec<SurfaceScalarField>,
    /// Fission-neutron production `nu Sigma_{f,g}` per group (m^-1).
    pub nu_sigma_f: Vec<VolScalarField>,
    /// Fission energy-release cross section `kappa Sigma_{f,g}` per group (J/m).
    pub sigma_pow: Vec<VolScalarField>,
    /// Removal cross section `Sigma_{r,g}` per group (m^-1).
    pub sigma_removal: Vec<VolScalarField>,
    /// Prompt fission spectrum `chi_{p,g}` per group (dimensionless).
    pub chi_prompt: Vec<VolScalarField>,
    /// Delayed fission spectrum `chi_{d,g}` per group (dimensionless).
    pub chi_delayed: Vec<VolScalarField>,
    /// Inverse neutron speed `1/v_g` per group (s/m).
    pub inv_velocity: Vec<VolScalarField>,
    /// Scattering `Sigma_{s, j->i}` (moment 0), indexed `[from j][into i]`
    /// (m^-1). The diagonal `j == i` (self-scatter) is present but ignored by
    /// the in-scatter source, matching GeN-Foam's `energyJ != energyI` guard.
    pub scattering: Vec<Vec<VolScalarField>>,
    /// Per-group delayed fraction `beta_k` per precursor group (dimensionless).
    pub beta: Vec<VolScalarField>,
    /// Total delayed fraction `beta_tot` (dimensionless).
    pub beta_tot: VolScalarField,
    /// Precursor decay constant `lambda_k` per precursor group (s^-1).
    pub lambda: Vec<VolScalarField>,
}

/// Build a per-cell [`VolScalarField`] from a value vector, with zero-gradient
/// boundaries (an XS field never carries a boundary condition — only the flux
/// does).
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

impl DiffusionXsFields {
    /// Materialise the cross sections onto `mesh` at the feedback point
    /// `raw_parameters`.
    ///
    /// `zone_of_cell[c]` is the material-zone index of cell `c` (in
    /// [`CrossSectionData`] zone order); its length must equal `mesh.n_cells`.
    /// `raw_parameters` is the feedback vector in
    /// [`CrossSectionData::variable_names`] order (pass `&[]` for feedback-free
    /// data).
    ///
    /// # Errors
    ///
    /// [`XsError`] if a `zone_of_cell` entry, an energy group, or a Legendre
    /// moment is out of range, or the RBF query fails; also
    /// [`XsError::IndexOutOfRange`] (re-purposed) if `zone_of_cell.len()` does
    /// not match `mesh.n_cells`.
    pub fn materialize(
        mesh: &Arc<FvMesh>,
        xs: &CrossSectionData,
        zone_of_cell: &[usize],
        raw_parameters: &[f64],
    ) -> Result<Self, XsError> {
        let n_cells = mesh.n_cells;
        if zone_of_cell.len() != n_cells {
            return Err(XsError::IndexOutOfRange {
                what: "zone_of_cell length vs mesh.n_cells",
                index: zone_of_cell.len(),
                count: n_cells,
            });
        }
        let g = xs.energy_groups();
        let np = xs.prec_groups();

        // Pre-evaluate group constants per (cell, group) once.
        let mut d = Vec::with_capacity(g);
        let mut nu_sigma_f = Vec::with_capacity(g);
        let mut sigma_pow = Vec::with_capacity(g);
        let mut sigma_removal = Vec::with_capacity(g);
        let mut chi_prompt = Vec::with_capacity(g);
        let mut chi_delayed = Vec::with_capacity(g);
        let mut inv_velocity = Vec::with_capacity(g);

        for group in 0..g {
            let mut vd = vec![0.0; n_cells];
            let mut vnf = vec![0.0; n_cells];
            let mut vsp = vec![0.0; n_cells];
            let mut vsr = vec![0.0; n_cells];
            let mut vcp = vec![0.0; n_cells];
            let mut vcd = vec![0.0; n_cells];
            let mut viv = vec![0.0; n_cells];
            for c in 0..n_cells {
                let gc = xs.evaluate_group_constants(zone_of_cell[c], group, raw_parameters)?;
                vd[c] = gc.diffusion_coefficient.value;
                vnf[c] = gc.nu_sigma_f.value;
                vsp[c] = gc.sigma_pow.value;
                vsr[c] = gc.sigma_removal.value;
                vcp[c] = gc.chi_prompt.value;
                vcd[c] = gc.chi_delayed.value;
                viv[c] = gc.inverse_velocity.value;
            }
            d.push(cell_field(format!("D{group}"), mesh, vd));
            nu_sigma_f.push(cell_field(format!("nuSigmaF{group}"), mesh, vnf));
            sigma_pow.push(cell_field(format!("sigmaPow{group}"), mesh, vsp));
            sigma_removal.push(cell_field(format!("sigmaRemoval{group}"), mesh, vsr));
            chi_prompt.push(cell_field(format!("chiPrompt{group}"), mesh, vcp));
            chi_delayed.push(cell_field(format!("chiDelayed{group}"), mesh, vcd));
            inv_velocity.push(cell_field(format!("IV{group}"), mesh, viv));
        }

        let d_face = d.iter().map(fvc::interpolate).collect();

        // Scattering matrices per cell, re-laid-out as scattering[j][i].
        let mut scattering: Vec<Vec<Vec<f64>>> = vec![vec![vec![0.0; n_cells]; g]; g];
        for c in 0..n_cells {
            let m = xs.scattering_matrix(zone_of_cell[c], 0, raw_parameters)?;
            for (j, row) in scattering.iter_mut().enumerate() {
                for (i, cell_vec) in row.iter_mut().enumerate() {
                    cell_vec[c] = m.get(j, i);
                }
            }
        }
        let scattering = scattering
            .into_iter()
            .enumerate()
            .map(|(j, row)| {
                row.into_iter()
                    .enumerate()
                    .map(|(i, vals)| cell_field(format!("sigma_{j}_to_{i}"), mesh, vals))
                    .collect()
            })
            .collect();

        // Precursor data (feedback-independent, one value per zone).
        let mut beta = vec![vec![0.0; n_cells]; np];
        let mut lambda = vec![vec![0.0; n_cells]; np];
        let mut vbeta_tot = vec![0.0; n_cells];
        for c in 0..n_cells {
            let pc = xs.precursor_constants(zone_of_cell[c])?;
            vbeta_tot[c] = pc.beta_tot.value;
            for k in 0..np {
                beta[k][c] = pc.beta[k].value;
                lambda[k][c] = pc.lambda[k].value;
            }
        }
        let beta = beta
            .into_iter()
            .enumerate()
            .map(|(k, v)| cell_field(format!("Beta{k}"), mesh, v))
            .collect();
        let lambda = lambda
            .into_iter()
            .enumerate()
            .map(|(k, v)| cell_field(format!("lambda{k}"), mesh, v))
            .collect();
        let beta_tot = cell_field("BetaTot", mesh, vbeta_tot);

        Ok(Self {
            energy_groups: g,
            prec_groups: np,
            d,
            d_face,
            nu_sigma_f,
            sigma_pow,
            sigma_removal,
            chi_prompt,
            chi_delayed,
            inv_velocity,
            scattering,
            beta,
            beta_tot,
            lambda,
        })
    }
}
