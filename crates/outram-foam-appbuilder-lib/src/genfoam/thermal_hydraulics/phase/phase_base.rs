// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: src/classes/thermalHydraulics/src/phaseModels/phaseBase.{C,H}
//   Upstream commit: 652b3da
//   Upstream copyright: (C) 2015-2022 EPFL; built on OpenFOAM v2506
//   Upstream author: Stefan Radman (EPFL)
//   Upstream license: GPL-3.0
//
// This file is part of OUTRAM PARK, distributed under GPL-3.0. See the crate
// root or <https://www.gnu.org/licenses/> for the full license text.

//! # `phase_base` — shared per-phase field foundation
//!
//! Rust analogue of GeN-Foam's `Foam::phaseBase`, the abstract base every
//! phase (the [`super::fluid::Fluid`] and the solid `structure`) inherits from.
//! Upstream `phaseBase` **is** a `volScalarField` — the phase **volume
//! fraction** `alpha` — plus three scalars: the phase name, a reference to the
//! mesh, and the *residual* volume fraction used to stabilise the phase
//! momentum equation as `alpha -> 0`.
//!
//! Here that is a plain composition ([`PhaseBase`] owns the `alpha`
//! [`VolScalarField`]) rather than inheritance, since Rust has no field-type
//! inheritance and the workspace forbids trait-object dispatch. A [`Fluid`]
//! (and, later, a solid structure) embeds a `PhaseBase` and forwards to it.
//!
//! [`Fluid`]: super::fluid::Fluid
//!
//! ## Units
//!
//! [`outram_foam_basic_lib`]'s [`VolScalarField`] stores bare `f64` per cell, so
//! the `alpha` field is dimensionless-by-convention (a volume fraction, `0..=1`).
//! The scalar [`PhaseBase::residual_alpha`] that crosses the public API is a
//! [`uom`]-typed [`VolumeFraction`] so a reader hovering in their editor sees a
//! dimensionless ratio, not a bare number.

use std::sync::Arc;

use outram_foam_basic_lib::fields::vol_field::VolScalarField;
use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;
use uom::si::f64::Ratio;
use uom::si::ratio::ratio;

/// Phase **volume fraction** `alpha` — **dimensionless** (`0..=1`).
///
/// The fraction of a cell's volume occupied by this phase. Named alias over
/// [`uom`]'s [`Ratio`]; used for the scalar volume-fraction parameters that
/// cross the public API ([`PhaseBase::residual_alpha`],
/// [`super::fluid::Fluid::thermo_residual_alpha`]). The per-cell `alpha`
/// **field** itself is a bare-`f64` [`VolScalarField`] (basic-lib convention),
/// documented as a volume fraction.
pub type VolumeFraction = Ratio;

/// Build a [`VolumeFraction`] from a plain (dimensionless) magnitude.
#[inline]
#[must_use]
pub fn volume_fraction(value: f64) -> VolumeFraction {
    VolumeFraction::new::<ratio>(value)
}

/// The shared per-phase state common to the fluid and structure phases.
///
/// Port of `Foam::phaseBase`. Owns the phase **volume-fraction field**
/// `alpha` (one dimensionless value per cell), the phase **name**, and the
/// **residual volume fraction** used to keep the phase momentum equation
/// well-posed as `alpha -> 0`.
///
/// The mesh is shared read-only (`Arc<FvMesh>`); the `alpha` field carries its
/// own clone of that `Arc`, matching the rest of the field state in
/// [`super::fluid::Fluid`].
#[derive(Debug, Clone)]
pub struct PhaseBase {
    /// Phase name (e.g. `"liquid"`, `"vapour"`), used to group per-phase fields.
    name: String,
    /// The mesh every field of this phase is defined on (shared, read-only).
    mesh: Arc<FvMesh>,
    /// Volume-fraction field `alpha` (dimensionless, `0..=1`), one per cell.
    alpha: VolScalarField,
    /// Residual volume fraction (dimensionless). Upstream default `1e-6`;
    /// stabilises the phase momentum as `alpha -> 0`.
    residual_alpha: VolumeFraction,
}

impl PhaseBase {
    /// Allocate a phase base on `mesh` with a uniform `alpha` of zero and the
    /// upstream default residual volume fraction (`1e-6`).
    ///
    /// The `alpha` field is named `alpha.<name>`, mirroring GeN-Foam's
    /// `IOobject::groupName("alpha", name)`. Set the initial fraction with
    /// [`Self::alpha_mut`] (or [`Self::set_residual_alpha`] for the residual).
    ///
    /// # Parameters
    /// - `mesh` — the finite-volume mesh (shared `Arc`).
    /// - `name` — the phase name.
    #[must_use]
    pub fn new(mesh: Arc<FvMesh>, name: impl Into<String>) -> Self {
        let name = name.into();
        let alpha = VolScalarField::zeros(format!("alpha.{name}"), mesh.clone());
        Self {
            name,
            mesh,
            alpha,
            residual_alpha: volume_fraction(1.0e-6),
        }
    }

    /// The phase name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The mesh all fields of this phase are defined on.
    #[must_use]
    pub fn mesh(&self) -> &Arc<FvMesh> {
        &self.mesh
    }

    /// The phase volume-fraction field `alpha` (dimensionless, `0..=1`).
    #[must_use]
    pub fn alpha(&self) -> &VolScalarField {
        &self.alpha
    }

    /// Mutable access to the phase volume-fraction field `alpha`.
    pub fn alpha_mut(&mut self) -> &mut VolScalarField {
        &mut self.alpha
    }

    /// The residual volume fraction (dimensionless), the floor that keeps the
    /// phase momentum well-posed as `alpha -> 0`.
    #[must_use]
    pub fn residual_alpha(&self) -> VolumeFraction {
        self.residual_alpha
    }

    /// Overwrite the residual volume fraction.
    pub fn set_residual_alpha(&mut self, value: VolumeFraction) {
        self.residual_alpha = value;
    }
}
