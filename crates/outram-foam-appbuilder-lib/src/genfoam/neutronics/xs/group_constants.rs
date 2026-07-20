// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/XS/ (XS.{C,H}; the group-wise
//                    volScalarField accessors D/nuSigmaEff/sigmaPow/...
//                    and include/setNeutronicsConstants.H, setPrecConst.H)
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

//! # Typed group-constant and precursor views
//!
//! These are the `uom`-typed values a deterministic multigroup solver
//! (diffusion / SP3 / SN) reads out of the cross-section store after
//! interpolation. They sit at the boundary where the unit-free interpolation
//! core hands data to physics code, so every field is a named, dimension-checked
//! quantity from [`super::units`].
//!
//! The scattering matrix is returned separately (as an
//! `outram_foam_basic_lib::matrix::SquareMatrix`) by
//! [`super::CrossSectionData::scattering_matrix`], because its size
//! (`energy_groups x energy_groups`) and per-Legendre-moment indexing make a
//! dense matrix the natural container.

use crate::genfoam::neutronics::xs::units::{
    DiffusionCoefficient, Dimensionless, EnergyReleaseCrossSection, InverseVelocity,
    MacroscopicCrossSection, PrecursorDecayConstant,
};

/// The multigroup constants for **one energy group** in one material zone at one
/// feedback state.
///
/// Every field carries its physical dimension; see [`super::units`] for the base
/// SI units. Assembled by [`super::CrossSectionData::evaluate_group_constants`]
/// (feedback-interpolated) or
/// [`super::CrossSectionData::reference_group_constants`] (reference values).
#[derive(Debug, Clone, Copy)]
pub struct GroupConstants {
    /// Diffusion coefficient `D_g` (m).
    pub diffusion_coefficient: DiffusionCoefficient,
    /// Fission-neutron production cross section `nu Sigma_{f,g}` (m^-1).
    pub nu_sigma_f: MacroscopicCrossSection,
    /// Fission energy-release cross section `kappa Sigma_{f,g}` (J/m).
    pub sigma_pow: EnergyReleaseCrossSection,
    /// Removal cross section `Sigma_{r,g}` (m^-1):
    /// absorption + out-scatter from group `g`.
    pub sigma_removal: MacroscopicCrossSection,
    /// Prompt fission emission spectrum `chi_{p,g}` (dimensionless).
    pub chi_prompt: Dimensionless,
    /// Delayed fission emission spectrum `chi_{d,g}` (dimensionless).
    pub chi_delayed: Dimensionless,
    /// Inverse neutron speed `1/v_g` (s/m).
    pub inverse_velocity: InverseVelocity,
    /// Discontinuity factor `gamma_g` (dimensionless).
    pub disc_factor: Dimensionless,
}

/// The delayed-neutron precursor constants for one material zone.
///
/// Precursor data are feedback-independent in GeN-Foam (read once from the
/// reference state), so there is a single value per zone rather than one per
/// feedback point. `beta` and `lambda` have one entry per precursor group.
#[derive(Debug, Clone)]
pub struct PrecursorConstants {
    /// Per-group delayed-neutron fractions `beta_k` (dimensionless).
    pub beta: Vec<Dimensionless>,
    /// Total delayed-neutron fraction `beta_tot = sum_k beta_k` (dimensionless).
    pub beta_tot: Dimensionless,
    /// Per-group precursor decay constants `lambda_k` (s^-1).
    pub lambda: Vec<PrecursorDecayConstant>,
}
