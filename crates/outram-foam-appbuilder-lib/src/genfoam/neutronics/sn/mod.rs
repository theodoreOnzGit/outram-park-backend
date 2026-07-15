// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/SN/ (SNNeutronics.{C,H} —
//     discrete-ordinates transport). Not yet ported; this is a documented
//     scaffold.
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

//! # Discrete-ordinates (SN) transport — scaffold
//!
//! GeN-Foam's `SN` neutronics solves the transport equation directly over a
//! quadrature set of discrete directions (angular fluxes), the highest-fidelity
//! deterministic option — needed where the diffusion / SP3 angular
//! approximations break down (strong absorbers, voids, streaming paths).
//!
//! **Status: scaffold, not yet implemented.** As with [`super::sp3`], this type
//! exists so [`crate::genfoam::neutronics::NeutronicsModel`] can name the SN
//! variant, the `match`-exhaustiveness guarantee holds, and the shared-state
//! surface works. The SN sweep + quadrature port is tracked in beads (child of
//! the `neutronics` epic).

use std::sync::Arc;

use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;

use crate::genfoam::neutronics::state::NeutronicsState;
use crate::genfoam::neutronics::{NeutronicsError, NeutronicsModelKind};

/// Discrete-ordinates transport model (scaffold — see the module docs).
///
/// Carries an allocated [`NeutronicsState`] so the shared
/// [`crate::genfoam::neutronics::NeutronicsModel`] surface (`power`, `k_eff`)
/// is available, but every solve currently returns
/// [`NeutronicsError::ModelNotImplemented`].
#[derive(Debug, Clone)]
pub struct SnNeutronics {
    state: NeutronicsState,
}

impl SnNeutronics {
    /// Allocate an SN scaffold with `energy_groups` (scalar-flux) fields and
    /// `prec_groups` precursor fields on `mesh`. The angular-flux storage is
    /// added when the model is ported.
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

    /// Solve the SN k-eigenvalue problem — **not yet implemented**.
    ///
    /// # Errors
    ///
    /// Always [`NeutronicsError::ModelNotImplemented`] with
    /// [`NeutronicsModelKind::Sn`].
    pub fn solve_eigenvalue(&mut self) -> Result<(), NeutronicsError> {
        Err(NeutronicsError::ModelNotImplemented(
            NeutronicsModelKind::Sn,
        ))
    }
}
