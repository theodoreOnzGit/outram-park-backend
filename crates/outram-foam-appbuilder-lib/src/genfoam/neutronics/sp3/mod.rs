// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/SP3/ and albedoSP3/
//     (SP3Neutronics.{C,H} — simplified-P3 transport). Not yet ported; this is
//     a documented scaffold.
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

//! # Simplified-P3 (SP3) transport — scaffold
//!
//! GeN-Foam's `SP3` / `albedoSP3` neutronics adds the second SP3 moment on top
//! of the diffusion field/assembly machinery, improving accuracy in optically
//! thin or strongly-heterogeneous regions where diffusion theory is weakest.
//!
//! **Status: scaffold, not yet implemented.** This type exists so that
//! [`crate::genfoam::neutronics::NeutronicsModel`] can name the SP3 variant and
//! the `match`-exhaustiveness guarantee holds today, and so the shared-state
//! surface (`power`, `k_eff`) already works. The SP3 assembly is tracked in
//! beads (child of the `neutronics` epic); the port will reuse
//! [`crate::genfoam::neutronics::diffusion::fields::DiffusionXsFields`] and the
//! FV operators, adding the coupled second-moment equation.

use std::sync::Arc;

use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;

use crate::genfoam::neutronics::state::NeutronicsState;
use crate::genfoam::neutronics::{NeutronicsError, NeutronicsModelKind};

/// Simplified-P3 transport model (scaffold — see the module docs).
///
/// Carries an allocated [`NeutronicsState`] so the shared
/// [`crate::genfoam::neutronics::NeutronicsModel`] surface (`power`, `k_eff`)
/// is available, but every solve currently returns
/// [`NeutronicsError::ModelNotImplemented`].
#[derive(Debug, Clone)]
pub struct Sp3Neutronics {
    state: NeutronicsState,
}

impl Sp3Neutronics {
    /// Allocate an SP3 scaffold with `energy_groups` flux fields and
    /// `prec_groups` precursor fields on `mesh`.
    #[must_use]
    pub fn new(mesh: Arc<FvMesh>, energy_groups: usize, prec_groups: usize) -> Self {
        Self {
            state: NeutronicsState::new(mesh, energy_groups, prec_groups),
        }
    }

    /// The shared neutronics state.
    #[must_use]
    pub fn state(&self) -> &NeutronicsState {
        &self.state
    }

    /// Solve the SP3 k-eigenvalue problem — **not yet implemented**.
    ///
    /// # Errors
    ///
    /// Always [`NeutronicsError::ModelNotImplemented`] with
    /// [`NeutronicsModelKind::Sp3`].
    pub fn solve_eigenvalue(&mut self) -> Result<(), NeutronicsError> {
        Err(NeutronicsError::ModelNotImplemented(
            NeutronicsModelKind::Sp3,
        ))
    }
}
