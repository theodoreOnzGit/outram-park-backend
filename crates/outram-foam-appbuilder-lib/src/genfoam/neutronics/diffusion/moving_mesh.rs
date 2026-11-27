// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: GeN-Foam, repository foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/neutronics.C (deformMesh, :363-408); OpenFOAM-ESI v2506
//                    src/finiteVolume/fvMesh/fvMesh.C (movePoints), EulerDdtScheme.C (fvmDdt)
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
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

//! The diffusion neutronics region on a mesh that `deformMesh()` moves (gh:#1856 item 1, README round 56).
//!
//! AI-written, not human-reviewed. Research, education and V&V only.
//!
//! In upstream the fields stay where they are and `fvMesh::movePoints` changes the geometry under them, so every operator
//! assembled afterwards (`fvm::laplacian`, `fvm::Sp`, `fvc::domainIntegrate` in `power()`, the interpolation weights of
//! `D/discFactor`) sees the moved `V`, `Sf`, `C` and delta coefficients, and `fvm::ddt` takes the moving-mesh branch of
//! `EulerDdtScheme` (`diag = rDeltaT rho Vsc`, `source = rDeltaT rho0 vf0 Vsc0`). [`DiffusionNeutronics::rebind_mesh`] is the
//! Rust form: put the state and the cross-section fields on the moved [`FvMesh`] and, for the `ddt`, hand over `V0`.
//!
//! Refused by name: a circulating-fuel region (`PrecursorTransport` holds a face flux `phi` and `alpha` built on the old geometry:
//! `phi = fvc::flux(U alpha)` would have to be rebuilt by the caller), and an `XsEvolution` (no moving-mesh comparison exists).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use alloc::sync::Arc;

use outram_foam_basic_lib::prelude::{fvc, FvMesh};

use super::DiffusionNeutronics;
use crate::genfoam::neutronics::NeutronicsError;

impl DiffusionNeutronics {
    /// `Vsc0()` for the `ddt` of this step: `V0` when [`Self::rebind_mesh`] was given one, else `volumes` (a static mesh).
    pub(super) fn old_volumes_or<'a>(&'a self, volumes: &'a [f64]) -> &'a [f64] {
        self.old_volumes.as_deref().unwrap_or(volumes)
    }

    /// Select the `laplacianSchemes` of the eigenvalue pseudo-time step: `Corrected` is `Gauss linear corrected` (the explicit
    /// non-orthogonal correction, needed on a mesh `deformMesh()` has sheared). `Uncorrected` (the default) is the earlier form.
    #[must_use]
    pub fn with_eigenvalue_laplacian(mut self, scheme: super::external_source::NeutronicsLaplacianScheme) -> Self {
        self.eigenvalue_laplacian = scheme;
        self
    }

    /// Whether the region is bound to a moving mesh (it was given `V0`).
    #[must_use]
    pub fn on_moving_mesh(&self) -> bool {
        self.old_volumes.is_some()
    }

    /// `deformMesh()` has moved the region's mesh to `mesh` (the same topology, new points): bind the state and the cross sections to
    /// it and remember `v0` (`fvMesh::V0()`, `None` when the mesh has never moved: a static mesh keeps the earlier operators bit for
    /// bit). The face diffusion coefficients `D/discFactor` are re-interpolated with the moved weights, as `fvm::laplacian` does at
    /// each assembly, and the albedo patches are rebuilt.
    ///
    /// # Errors
    ///
    /// [`NeutronicsError::UpstreamTransient`] when the topology differs, `v0` is not sized to the mesh, circulating fuel or an
    /// `XsEvolution` is set (refused by name, see the module documentation).
    pub fn rebind_mesh(&mut self, mesh: Arc<FvMesh>, v0: Option<Vec<f64>>) -> Result<(), NeutronicsError> {
        if self.drift.is_some() {
            return Err(NeutronicsError::UpstreamTransient(
                "a circulating-fuel region on a moving mesh is not ported (the precursor flux phi and alpha belong to the old geometry)",
            ));
        }
        if self.evolution.is_some() {
            return Err(NeutronicsError::UpstreamTransient("an XsEvolution on a moving mesh is not ported (gh:#1856)"));
        }
        if v0.as_ref().is_some_and(|v| v.len() != mesh.n_cells) {
            return Err(NeutronicsError::UpstreamTransient("V0 is not sized to the mesh"));
        }
        self.state.rebind_mesh(mesh.clone()).map_err(NeutronicsError::UpstreamTransient)?;
        let x = &mut self.xs;
        for f in x
            .d
            .iter_mut()
            .chain(x.nu_sigma_f.iter_mut())
            .chain(x.sigma_pow.iter_mut())
            .chain(x.sigma_removal.iter_mut())
            .chain(x.chi_prompt.iter_mut())
            .chain(x.chi_delayed.iter_mut())
            .chain(x.inv_velocity.iter_mut())
            .chain(x.beta.iter_mut())
            .chain(x.lambda.iter_mut())
            .chain(x.disc_factor.iter_mut())
        {
            f.mesh = mesh.clone();
        }
        x.beta_tot.mesh = mesh.clone();
        for row in x.scattering.iter_mut() {
            for f in row.iter_mut() {
                f.mesh = mesh.clone();
            }
        }
        x.d_face = x.d.iter().map(fvc::interpolate).collect();
        for f in self.prec_star.iter_mut() {
            f.mesh = mesh.clone();
        }
        for (patch, gamma, lin) in self.albedo_specs.clone() {
            self.set_albedo_boundary(patch, gamma, lin);
        }
        self.old_volumes = v0;
        Ok(())
    }
}
