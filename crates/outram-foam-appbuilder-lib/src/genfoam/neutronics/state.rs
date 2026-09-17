// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream commit: 652b3da
//   Upstream source: src/classes/neutronics/neutronics.{C,H}
//                    (the abstract base: keff_, pTarget_, pTotOld_,
//                     powerDensity_, oneGroupFlux_, residual_) and
//                    diffusion/diffusionNeutronics.H (flux_, prec_ PtrLists).
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

//! # Shared spatial neutronics state
//!
//! The mesh-based state every *spatial* deterministic neutronics model
//! (diffusion / SP3 / SN) reads and writes: the per-group scalar flux, the
//! per-group delayed-neutron precursor concentration, the fuel power-density
//! field, the collapsed one-group flux, and the two scalars the eigenvalue /
//! transient loops carry — the effective multiplication factor `k_eff` and the
//! integrated total fission power.
//!
//! This is the Rust analogue of GeN-Foam's `Foam::neutronics` abstract base
//! (`keff_`, `powerDensity_`, `oneGroupFlux_`) plus the `flux_` / `prec_`
//! `PtrList`s that `diffusionNeutronics` adds. The 0-D
//! [`super::point_kinetics`] model does **not** use this state — it has no
//! mesh — so the shared state lives here rather than being forced onto every
//! [`super::NeutronicsModel`] variant.
//!
//! ## Field units (basic-lib fields are bare `f64`, unitful by convention)
//!
//! `outram_foam_basic_lib`'s [`VolScalarField`] stores plain `f64` per cell, so
//! the physical unit of each field is a documented convention rather than a
//! `uom` type. The scalar *summary* quantities that cross the public API
//! ([`Self::k_eff`], [`Self::total_power`]) are returned as named `uom`
//! aliases.
//!
//! | Field | Symbol | Unit |
//! |---|---|---|
//! | [`Self::flux`] (per group) | `phi_g` | `1/(m^2 s)` (scalar group flux) |
//! | [`Self::precursors`] (per group) | `C_k` | `1/m^3` (precursor number density) |
//! | [`Self::power_density`] | `q'''` | `W/m^3` |
//! | [`Self::one_group_flux`] | `sum_g phi_g` | `1/(m^2 s)` |

use std::sync::Arc;

use outram_foam_basic_lib::fields::vol_field::VolScalarField;
use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;
use uom::si::f64::{Power, Ratio};
use uom::si::power::watt;
use uom::si::ratio::ratio;

/// Effective neutron multiplication factor `k_eff` — dimensionless. Named alias
/// over `uom`'s [`Ratio`]; `k_eff = 1` is exact criticality, `> 1` supercritical.
pub type NeutronMultiplicationFactor = Ratio;

/// The shared spatial neutronics state: per-group flux and precursors, the
/// power-density field, the one-group collapsed flux, `k_eff`, and the total
/// fission power.
///
/// Construct with [`Self::new`] (uniform unit flux, zero precursors,
/// `k_eff = 1`); a spatial model then overwrites the fields as it iterates.
/// The flux / precursor vectors are indexed by energy / precursor group in the
/// same order as the [`super::xs::CrossSectionData`] group constants.
#[derive(Debug, Clone)]
pub struct NeutronicsState {
    /// The mesh every field is defined on (shared, read-only after build).
    mesh: Arc<FvMesh>,
    /// Scalar neutron flux `phi_g` per energy group, `1/(m^2 s)`.
    flux: Vec<VolScalarField>,
    /// Delayed-neutron precursor concentration `C_k` per precursor group,
    /// `1/m^3`.
    precursors: Vec<VolScalarField>,
    /// Fuel volumetric power density `q'''`, `W/m^3`.
    power_density: VolScalarField,
    /// Energy-collapsed one-group flux `sum_g phi_g`, `1/(m^2 s)`.
    one_group_flux: VolScalarField,
    /// Effective multiplication factor `k_eff` (dimensionless).
    k_eff: f64,
    /// Domain-integrated total fission power `P`, watts.
    total_power: f64,
}

impl NeutronicsState {
    /// Allocate the state for `energy_groups` flux fields and `prec_groups`
    /// precursor fields on `mesh`.
    ///
    /// The flux fields start at a uniform value of `1.0` (the usual power-
    /// iteration seed), precursors and power density at zero, and `k_eff` at
    /// `1.0`. All flux fields carry [`outram_foam_basic_lib`]'s default
    /// zero-gradient boundaries; a solver overwrites the boundary conditions
    /// (e.g. a vacuum / zero-flux edge) before its first solve.
    ///
    /// # Parameters
    ///
    /// - `mesh` — the finite-volume mesh (shared `Arc`).
    /// - `energy_groups` — number of energy groups `G` (`>= 1`).
    /// - `prec_groups` — number of delayed-neutron precursor groups `N`
    ///   (may be `0` for a prompt-only model).
    #[must_use]
    pub fn new(mesh: Arc<FvMesh>, energy_groups: usize, prec_groups: usize) -> Self {
        let flux = (0..energy_groups)
            .map(|g| VolScalarField::uniform(format!("flux{g}"), mesh.clone(), 1.0))
            .collect();
        let precursors = (0..prec_groups)
            .map(|k| VolScalarField::zeros(format!("prec{k}"), mesh.clone()))
            .collect();
        Self {
            power_density: VolScalarField::zeros("powerDensity", mesh.clone()),
            one_group_flux: VolScalarField::zeros("oneGroupFlux", mesh.clone()),
            flux,
            precursors,
            k_eff: 1.0,
            total_power: 0.0,
            mesh,
        }
    }

    /// The mesh all fields are defined on.
    #[must_use]
    pub fn mesh(&self) -> &Arc<FvMesh> {
        &self.mesh
    }

    /// Number of energy groups `G`.
    #[must_use]
    pub fn energy_groups(&self) -> usize {
        self.flux.len()
    }

    /// Number of delayed-neutron precursor groups `N`.
    #[must_use]
    pub fn prec_groups(&self) -> usize {
        self.precursors.len()
    }

    /// The per-group scalar flux fields `phi_g` (`1/(m^2 s)`), read-only.
    #[must_use]
    pub fn flux(&self) -> &[VolScalarField] {
        &self.flux
    }

    /// The per-group flux fields, mutable (for a solver to overwrite in place).
    #[must_use]
    pub fn flux_mut(&mut self) -> &mut Vec<VolScalarField> {
        &mut self.flux
    }

    /// The per-group precursor concentration fields `C_k` (`1/m^3`), read-only.
    #[must_use]
    pub fn precursors(&self) -> &[VolScalarField] {
        &self.precursors
    }

    /// The per-group precursor fields, mutable.
    #[must_use]
    pub fn precursors_mut(&mut self) -> &mut Vec<VolScalarField> {
        &mut self.precursors
    }

    /// The fuel power-density field `q'''` (`W/m^3`), read-only.
    #[must_use]
    pub fn power_density(&self) -> &VolScalarField {
        &self.power_density
    }

    /// The fuel power-density field, mutable.
    #[must_use]
    pub fn power_density_mut(&mut self) -> &mut VolScalarField {
        &mut self.power_density
    }

    /// The energy-collapsed one-group flux `sum_g phi_g` (`1/(m^2 s)`).
    #[must_use]
    pub fn one_group_flux(&self) -> &VolScalarField {
        &self.one_group_flux
    }

    /// The one-group flux field, mutable.
    #[must_use]
    pub fn one_group_flux_mut(&mut self) -> &mut VolScalarField {
        &mut self.one_group_flux
    }

    /// The effective multiplication factor `k_eff` (dimensionless).
    #[must_use]
    pub fn k_eff(&self) -> NeutronMultiplicationFactor {
        Ratio::new::<ratio>(self.k_eff)
    }

    /// Overwrite `k_eff`.
    pub fn set_k_eff(&mut self, k_eff: NeutronMultiplicationFactor) {
        self.k_eff = k_eff.get::<ratio>();
    }

    /// The raw `k_eff` value (dimensionless `f64`) — the numerical working copy
    /// the solve loops read and update without `uom` wrapping overhead.
    #[must_use]
    pub fn k_eff_raw(&self) -> f64 {
        self.k_eff
    }

    /// Overwrite the raw `k_eff` value.
    pub fn set_k_eff_raw(&mut self, k_eff: f64) {
        self.k_eff = k_eff;
    }

    /// The domain-integrated total fission power `P` (watts).
    #[must_use]
    pub fn total_power(&self) -> Power {
        Power::new::<watt>(self.total_power)
    }

    /// Overwrite the total power (raw watts).
    pub fn set_total_power_raw(&mut self, watts: f64) {
        self.total_power = watts;
    }

    /// The raw total-power value (watts, `f64`).
    #[must_use]
    pub fn total_power_raw(&self) -> f64 {
        self.total_power
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::interface::one_dimensional_meshing::create_one_d_mesh;
    use uom::si::area::square_meter;
    use uom::si::f64::{Area, Length};
    use uom::si::length::meter;

    fn slab(n: i64) -> Arc<FvMesh> {
        Arc::new(
            create_one_d_mesh(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0), n)
                .unwrap(),
        )
    }

    #[test]
    fn new_allocates_group_fields_with_unit_flux() {
        let s = NeutronicsState::new(slab(4), 2, 6);
        assert_eq!(s.energy_groups(), 2);
        assert_eq!(s.prec_groups(), 6);
        assert_eq!(s.flux().len(), 2);
        assert_eq!(s.precursors().len(), 6);
        // seed flux is uniform 1.0
        assert!((s.flux()[0].internal[0] - 1.0).abs() < 1e-15);
        // precursors seed to zero
        assert!(s.precursors()[3].internal[0].abs() < 1e-15);
        assert!((s.k_eff().get::<ratio>() - 1.0).abs() < 1e-15);
    }

    #[test]
    fn keff_round_trips_through_uom() {
        let mut s = NeutronicsState::new(slab(2), 1, 0);
        s.set_k_eff(Ratio::new::<ratio>(1.2345));
        assert!((s.k_eff_raw() - 1.2345).abs() < 1e-12);
        assert!((s.k_eff().get::<ratio>() - 1.2345).abs() < 1e-12);
    }
}
