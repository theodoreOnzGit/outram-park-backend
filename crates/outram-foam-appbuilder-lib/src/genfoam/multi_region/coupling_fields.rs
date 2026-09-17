// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from GeN-Foam (Generalized Nuclear Foam)
//   Upstream: https://gitlab.com/foam-for-nuclear/GeN-Foam
//   Upstream source: src/classes/multiRegion/meshHandler/
//                    meshHandler.{C,H} (the region-mesh registry, the `mappings`
//                    dictionary, and mapAllFields / mapTheseFields —
//                    "interpolateCouplingFields")
//   Upstream commit: 652b3da
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

//! # Coupled-feedback field assembly (`interpolateCouplingFields`)
//!
//! Rust port of GeN-Foam's `meshHandler`: the registry of region meshes and the
//! `mappings` specification that pulls the coupled-feedback fields onto the
//! meshes that consume them. In a reactor multiphysics run the fields exchanged
//! each coupling step are:
//!
//! | Field | Produced on | Consumed on |
//! |---|---|---|
//! | `TFuel` (fuel temperature) | thermal-hydraulics | neutronics (Doppler XS feedback) |
//! | `TStruct` (structure temperature) | thermal-hydraulics / thermo-mechanics | neutronics, thermo-mechanics |
//! | coolant density `rho` | thermal-hydraulics | neutronics (moderator/coolant feedback) |
//! | `powerDensityNeutronics` | neutronics | thermal-hydraulics (volumetric heat source) |
//! | `meshDisp` (mesh displacement) | thermo-mechanics | neutronics, thermal-hydraulics (geometry feedback) |
//!
//! Each region lives on its own mesh; the [`MeshHandler`] holds those meshes,
//! the named fields on them, and the pairwise [`MeshToMesh`] mappings that move a
//! source field onto a target field. [`MeshHandler::interpolate_coupling_fields`]
//! executes the whole `mappings` spec in one call — the direct analogue of
//! upstream `mapAllFields`; [`MeshHandler::interpolate_these`] restricts it to a
//! named subset (upstream `mapTheseFields`, called inside the outer loop so only
//! the actively-iterated regions re-exchange).
//!
//! Field values are unit-agnostic raw `f64` / [`Vector3`] cell arrays (matching
//! [`outram_foam_basic_lib`] fields); the physical `uom` interpretation is
//! attached where the values are *used* — see [`super::outer_iteration`].

use std::collections::BTreeMap;
use std::sync::Arc;

use outram_foam_basic_lib::fields::vol_field::{VolScalarField, VolVectorField};
use outram_foam_basic_lib::mesh::fv_mesh::FvMesh;

use super::mesh_to_mesh::{MapCombine, MappingMethod, MeshToMesh};

/// Errors from building or running the coupling-field registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CouplingError {
    /// A region name was referenced that is not registered.
    UnknownRegion(String),
    /// A region with this name was already registered.
    DuplicateRegion(String),
    /// A mapping referenced a field absent from a region.
    MissingField {
        /// The region the field was expected on.
        region: String,
        /// The missing field's name.
        field: String,
    },
}

impl std::fmt::Display for CouplingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CouplingError::UnknownRegion(r) => write!(f, "unknown region '{r}'"),
            CouplingError::DuplicateRegion(r) => write!(f, "region '{r}' already registered"),
            CouplingError::MissingField { region, field } => {
                write!(f, "field '{field}' not found on region '{region}'")
            }
        }
    }
}

impl std::error::Error for CouplingError {}

/// Whether a coupling field is scalar or vector valued.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldKind {
    /// A [`VolScalarField`] (temperature, density, power density).
    Scalar,
    /// A [`VolVectorField`] (mesh displacement, velocity).
    Vector,
}

/// One entry of a region-to-region mapping: which source field feeds which
/// target field, its kind, and how it combines with the target's contents.
///
/// Upstream this is one aligned `(sourceFields, targetFields)` pair in the
/// `mappings` sub-dictionary; the combine mode reflects upstream's `plusEqOp`
/// accumulation (use [`MapCombine::Accumulate`] when several source regions sum
/// into one target field, [`MapCombine::Replace`] for a single clean transfer).
#[derive(Debug, Clone)]
pub struct CouplingField {
    /// Field name on the source region.
    pub source: String,
    /// Field name on the target region.
    pub target: String,
    /// Scalar or vector.
    pub kind: FieldKind,
    /// How the mapped value combines with the target's current value.
    pub combine: MapCombine,
}

impl CouplingField {
    /// A scalar `source → target` transfer that replaces the target value.
    #[must_use]
    pub fn scalar(source: &str, target: &str) -> Self {
        Self {
            source: source.to_string(),
            target: target.to_string(),
            kind: FieldKind::Scalar,
            combine: MapCombine::Replace,
        }
    }

    /// A vector `source → target` transfer that replaces the target value.
    #[must_use]
    pub fn vector(source: &str, target: &str) -> Self {
        Self {
            source: source.to_string(),
            target: target.to_string(),
            kind: FieldKind::Vector,
            combine: MapCombine::Replace,
        }
    }

    /// Set the combine mode to [`MapCombine::Accumulate`] (upstream `plusEqOp`).
    #[must_use]
    pub fn accumulating(mut self) -> Self {
        self.combine = MapCombine::Accumulate;
        self
    }
}

/// A physics region: a named mesh and the coupling fields that live on it.
///
/// The neutronics, thermal-hydraulics and thermo-mechanics regions are each one
/// of these. It owns its mesh (`Arc<FvMesh>`, shared with the mappers) and its
/// coupling fields keyed by name.
#[derive(Debug, Clone)]
pub struct CouplingRegion {
    name: String,
    mesh: Arc<FvMesh>,
    scalars: BTreeMap<String, VolScalarField>,
    vectors: BTreeMap<String, VolVectorField>,
}

impl CouplingRegion {
    /// Create an empty region named `name` on `mesh`.
    #[must_use]
    pub fn new(name: &str, mesh: Arc<FvMesh>) -> Self {
        Self {
            name: name.to_string(),
            mesh,
            scalars: BTreeMap::new(),
            vectors: BTreeMap::new(),
        }
    }

    /// The region name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The region mesh.
    #[must_use]
    pub fn mesh(&self) -> &Arc<FvMesh> {
        &self.mesh
    }

    /// Register a scalar coupling field (keyed by the field's own name).
    pub fn insert_scalar(&mut self, field: VolScalarField) {
        self.scalars.insert(field.name.clone(), field);
    }

    /// Register a vector coupling field (keyed by the field's own name).
    pub fn insert_vector(&mut self, field: VolVectorField) {
        self.vectors.insert(field.name.clone(), field);
    }

    /// Borrow a scalar field by name.
    #[must_use]
    pub fn scalar(&self, name: &str) -> Option<&VolScalarField> {
        self.scalars.get(name)
    }

    /// Mutably borrow a scalar field by name.
    pub fn scalar_mut(&mut self, name: &str) -> Option<&mut VolScalarField> {
        self.scalars.get_mut(name)
    }

    /// Borrow a vector field by name.
    #[must_use]
    pub fn vector(&self, name: &str) -> Option<&VolVectorField> {
        self.vectors.get(name)
    }

    /// Mutably borrow a vector field by name.
    pub fn vector_mut(&mut self, name: &str) -> Option<&mut VolVectorField> {
        self.vectors.get_mut(name)
    }
}

/// A registered region-to-region mapping: a cached [`MeshToMesh`] plus the list
/// of field pairs it transfers.
#[derive(Debug, Clone)]
struct RegionMapping {
    /// Region the fields are written onto.
    target: String,
    /// Region the fields are read from.
    source: String,
    /// Cached cell-to-cell addressing (source mesh → target mesh).
    mapper: MeshToMesh,
    /// The field pairs moved by this mapping.
    fields: Vec<CouplingField>,
}

/// The multi-region field-mapping hub: region meshes, their coupling fields, and
/// the pairwise mappings between them.
///
/// Register regions with [`MeshHandler::add_region`], wire the `mappings` with
/// [`MeshHandler::add_mapping`], then call
/// [`MeshHandler::interpolate_coupling_fields`] each coupling step. This is the
/// Rust `meshHandler`.
#[derive(Debug, Clone, Default)]
pub struct MeshHandler {
    regions: Vec<CouplingRegion>,
    index: BTreeMap<String, usize>,
    mappings: Vec<RegionMapping>,
}

impl MeshHandler {
    /// An empty handler.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a region. Its name must be unique.
    ///
    /// # Errors
    ///
    /// [`CouplingError::DuplicateRegion`] if a region of that name already exists.
    pub fn add_region(&mut self, region: CouplingRegion) -> Result<(), CouplingError> {
        if self.index.contains_key(region.name()) {
            return Err(CouplingError::DuplicateRegion(region.name().to_string()));
        }
        self.index
            .insert(region.name().to_string(), self.regions.len());
        self.regions.push(region);
        Ok(())
    }

    /// Borrow a region by name.
    #[must_use]
    pub fn region(&self, name: &str) -> Option<&CouplingRegion> {
        self.index.get(name).map(|&i| &self.regions[i])
    }

    /// Mutably borrow a region by name.
    pub fn region_mut(&mut self, name: &str) -> Option<&mut CouplingRegion> {
        let i = *self.index.get(name)?;
        Some(&mut self.regions[i])
    }

    /// Register a mapping that transfers `fields` from region `source` onto
    /// region `target`, building the [`MeshToMesh`] addressing between their
    /// meshes with `method`.
    ///
    /// # Errors
    ///
    /// [`CouplingError::UnknownRegion`] if either name is not registered.
    pub fn add_mapping(
        &mut self,
        target: &str,
        source: &str,
        method: MappingMethod,
        fields: Vec<CouplingField>,
    ) -> Result<(), CouplingError> {
        let src_mesh = self
            .region(source)
            .ok_or_else(|| CouplingError::UnknownRegion(source.to_string()))?
            .mesh()
            .clone();
        let tgt_mesh = self
            .region(target)
            .ok_or_else(|| CouplingError::UnknownRegion(target.to_string()))?
            .mesh()
            .clone();
        let mapper = MeshToMesh::new(src_mesh, tgt_mesh, method);
        self.mappings.push(RegionMapping {
            target: target.to_string(),
            source: source.to_string(),
            mapper,
            fields,
        });
        Ok(())
    }

    /// Execute *all* registered mappings — the coupled-feedback assembly
    /// (`mapAllFields`).
    ///
    /// # Errors
    ///
    /// [`CouplingError::MissingField`] if any mapping names a field absent from
    /// its source or target region.
    pub fn interpolate_coupling_fields(&mut self) -> Result<(), CouplingError> {
        for m in 0..self.mappings.len() {
            self.run_mapping(m)?;
        }
        Ok(())
    }

    /// Execute only the mappings whose *both* endpoints are in `region_names`
    /// (upstream `mapTheseFields`, used by the outer loop to re-exchange only the
    /// regions currently being iterated).
    ///
    /// # Errors
    ///
    /// [`CouplingError::MissingField`] as [`Self::interpolate_coupling_fields`].
    pub fn interpolate_these(&mut self, region_names: &[&str]) -> Result<(), CouplingError> {
        for m in 0..self.mappings.len() {
            let (src, tgt) = (
                self.mappings[m].source.clone(),
                self.mappings[m].target.clone(),
            );
            if region_names.contains(&src.as_str()) && region_names.contains(&tgt.as_str()) {
                self.run_mapping(m)?;
            }
        }
        Ok(())
    }

    /// Run a single registered mapping by index.
    fn run_mapping(&mut self, m: usize) -> Result<(), CouplingError> {
        let si = self.index[&self.mappings[m].source];
        let ti = self.index[&self.mappings[m].target];
        // Field specs are cloned out so the borrow of `self.mappings` is released
        // before the region fields are mutated.
        let fields = self.mappings[m].fields.clone();
        for field in &fields {
            match field.kind {
                FieldKind::Scalar => {
                    let src = self.regions[si]
                        .scalar(&field.source)
                        .ok_or_else(|| CouplingError::MissingField {
                            region: self.mappings[m].source.clone(),
                            field: field.source.clone(),
                        })?
                        .clone();
                    let missing = CouplingError::MissingField {
                        region: self.mappings[m].target.clone(),
                        field: field.target.clone(),
                    };
                    let tgt = self.regions[ti].scalar_mut(&field.target).ok_or(missing)?;
                    self.mappings[m].mapper.map_scalar(&src, field.combine, tgt);
                }
                FieldKind::Vector => {
                    let src = self.regions[si]
                        .vector(&field.source)
                        .ok_or_else(|| CouplingError::MissingField {
                            region: self.mappings[m].source.clone(),
                            field: field.source.clone(),
                        })?
                        .clone();
                    let missing = CouplingError::MissingField {
                        region: self.mappings[m].target.clone(),
                        field: field.target.clone(),
                    };
                    let tgt = self.regions[ti].vector_mut(&field.target).ok_or(missing)?;
                    self.mappings[m].mapper.map_vector(&src, field.combine, tgt);
                }
            }
        }
        Ok(())
    }

    /// The volume integral `∑ V φ` of a named scalar field on a named region — a
    /// convenience for conservation checks across a mapping.
    ///
    /// # Errors
    ///
    /// [`CouplingError::UnknownRegion`] / [`CouplingError::MissingField`].
    pub fn scalar_integral(&self, region: &str, field: &str) -> Result<f64, CouplingError> {
        let r = self
            .region(region)
            .ok_or_else(|| CouplingError::UnknownRegion(region.to_string()))?;
        let f = r.scalar(field).ok_or_else(|| CouplingError::MissingField {
            region: region.to_string(),
            field: field.to_string(),
        })?;
        Ok(MeshToMesh::integral(r.mesh(), f))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use outram_foam_basic_lib::mesh::fv_mesh::{BoundaryPatch, FvMeshBuilder, PatchKind};
    use outram_foam_basic_lib::primitives::Vector3;

    fn column(n: usize, length: f64, area: f64) -> Arc<FvMesh> {
        let h = length / n as f64;
        let n_internal = n - 1;
        let mut owner: Vec<usize> = (0..n_internal).collect();
        let neighbour: Vec<usize> = (1..n).collect();
        owner.push(0);
        owner.push(n - 1);
        let cell_centres: Vec<Vector3> = (0..n)
            .map(|i| Vector3::new((i as f64 + 0.5) * h, 0.0, 0.0))
            .collect();
        let mut fav = vec![Vector3::new(area, 0.0, 0.0); n_internal];
        fav.push(Vector3::new(-area, 0.0, 0.0));
        fav.push(Vector3::new(area, 0.0, 0.0));
        let mut fc: Vec<Vector3> = (1..n)
            .map(|i| Vector3::new(i as f64 * h, 0.0, 0.0))
            .collect();
        fc.push(Vector3::new(0.0, 0.0, 0.0));
        fc.push(Vector3::new(length, 0.0, 0.0));
        Arc::new(
            FvMeshBuilder::new()
                .n_cells(n)
                .n_internal_faces(n_internal)
                .owner(owner)
                .neighbour(neighbour)
                .patches(vec![
                    BoundaryPatch::new("bottom", n_internal, 1, PatchKind::Wall),
                    BoundaryPatch::new("top", n_internal + 1, 1, PatchKind::Wall),
                ])
                .cell_volumes(vec![h * area; n])
                .cell_centres(cell_centres)
                .face_area_vectors(fav)
                .face_centres(fc)
                .build()
                .unwrap(),
        )
    }

    /// Build a two-region handler: a coarse `neutro` region and a fine `fluid`
    /// region over the same `[0,1]` domain, with `powerDensityNeutronics`
    /// (neutro→fluid, conservative) and `TFuel` (fluid→neutro, interpolated).
    fn two_region_handler() -> MeshHandler {
        let neutro_mesh = column(4, 1.0, 3.0);
        let fluid_mesh = column(16, 1.0, 3.0);

        let mut neutro = CouplingRegion::new("neutro", neutro_mesh.clone());
        let mut pdens = VolScalarField::uniform("powerDensityNeutronics", neutro_mesh.clone(), 0.0);
        for (i, c) in neutro_mesh.cell_centres.iter().enumerate() {
            // A cosine-ish axial power shape.
            pdens.internal[i] = 1.0e6 * (1.0 + 0.5 * (std::f64::consts::PI * c.x).sin());
        }
        neutro.insert_scalar(pdens);
        neutro.insert_scalar(VolScalarField::uniform("TFuel", neutro_mesh.clone(), 0.0));

        let mut fluid = CouplingRegion::new("fluid", fluid_mesh.clone());
        fluid.insert_scalar(VolScalarField::uniform(
            "powerDensityNeutronics",
            fluid_mesh.clone(),
            0.0,
        ));
        let mut tfuel = VolScalarField::uniform("TFuel", fluid_mesh.clone(), 0.0);
        for (i, c) in fluid_mesh.cell_centres.iter().enumerate() {
            tfuel.internal[i] = 600.0 + 100.0 * c.x; // linear axial fuel-temp rise
        }
        fluid.insert_scalar(tfuel);

        let mut h = MeshHandler::new();
        h.add_region(neutro).unwrap();
        h.add_region(fluid).unwrap();
        h.add_mapping(
            "fluid",
            "neutro",
            MappingMethod::CellVolumeWeight,
            vec![CouplingField::scalar(
                "powerDensityNeutronics",
                "powerDensityNeutronics",
            )],
        )
        .unwrap();
        h.add_mapping(
            "neutro",
            "fluid",
            MappingMethod::InverseDistance { n_neighbours: 2 },
            vec![CouplingField::scalar("TFuel", "TFuel")],
        )
        .unwrap();
        h
    }

    /// V&V — the coupled-feedback assembly conserves total reactor power when
    /// mapping the volumetric heat source neutronics → thermal-hydraulics.
    ///
    /// Methodology: register the two-region handler above; the neutronics region
    /// carries a non-uniform `powerDensityNeutronics` [W/m³]. Its volume integral
    /// `∫ q''' dV` is the total fission power. After
    /// `interpolate_coupling_fields` maps it (conservatively) onto the finer
    /// fluid mesh, the fluid-side integral — the heat delivered to the coolant —
    /// must equal the neutronics-side integral, else the coupling leaks or
    /// manufactures energy.
    ///
    /// Result (2026-07-15, release): neutronics ∫q'''dV = fluid ∫q'''dV =
    /// 3.979922e6 W, relative difference 0.0 (assertion bound 1e-6). Pass.
    #[test]
    fn coupling_conserves_total_power() {
        let mut h = two_region_handler();
        let p_neutro = h
            .scalar_integral("neutro", "powerDensityNeutronics")
            .unwrap();
        h.interpolate_coupling_fields().unwrap();
        let p_fluid = h
            .scalar_integral("fluid", "powerDensityNeutronics")
            .unwrap();
        let rel = (p_neutro - p_fluid).abs() / p_neutro.abs();
        assert!(
            rel < 1e-6,
            "power not conserved: neutro={p_neutro} fluid={p_fluid}"
        );
    }

    /// V&V — the fuel-temperature feedback lands on the neutronics mesh tracking
    /// the linear TH profile.
    ///
    /// Methodology: `TFuel = 600 + 100x` on the fluid mesh is mapped onto the
    /// coarse neutronics cells by inverse-distance (2 neighbours). Each neutronics
    /// cell should read back the profile at its centre to within a fluid cell
    /// half-width (100 K/m · 0.03125 m ≈ 3.1 K).
    ///
    /// Result (2026-07-15, release): max deviation 0.0 K — here it is *exact*
    /// because each coarse neutronics cell centre lands symmetrically between two
    /// fluid cell centres, so the 2-neighbour inverse-distance blend recovers the
    /// linear profile value with equal weights. Assertion bound 4 K. Pass.
    #[test]
    fn fuel_temperature_feedback_maps_back() {
        let mut h = two_region_handler();
        h.interpolate_coupling_fields().unwrap();
        let neutro = h.region("neutro").unwrap();
        let tfuel = neutro.scalar("TFuel").unwrap();
        let max_err = neutro
            .mesh()
            .cell_centres
            .iter()
            .enumerate()
            .map(|(i, c)| (tfuel.internal[i] - (600.0 + 100.0 * c.x)).abs())
            .fold(0.0_f64, f64::max);
        assert!(max_err < 4.0, "TFuel feedback error {max_err} K");
    }

    #[test]
    fn interpolate_these_restricts_to_named_regions() {
        let mut h = two_region_handler();
        // Only the fluid→neutro (TFuel) leg has both endpoints named here... but
        // both regions are named, so both legs run. Restrict to just "neutro":
        // no mapping has both endpoints in {neutro}, so nothing changes.
        let before = h
            .region("fluid")
            .unwrap()
            .scalar("powerDensityNeutronics")
            .unwrap()
            .internal[0];
        h.interpolate_these(&["neutro"]).unwrap();
        let after = h
            .region("fluid")
            .unwrap()
            .scalar("powerDensityNeutronics")
            .unwrap()
            .internal[0];
        assert_eq!(before, after);
    }

    #[test]
    fn duplicate_region_rejected() {
        let mesh = column(2, 1.0, 1.0);
        let mut h = MeshHandler::new();
        h.add_region(CouplingRegion::new("r", mesh.clone()))
            .unwrap();
        let err = h.add_region(CouplingRegion::new("r", mesh)).unwrap_err();
        assert_eq!(err, CouplingError::DuplicateRegion("r".to_string()));
    }

    #[test]
    fn missing_field_reported() {
        let mesh = column(2, 1.0, 1.0);
        let mut h = MeshHandler::new();
        h.add_region(CouplingRegion::new("a", mesh.clone()))
            .unwrap();
        h.add_region(CouplingRegion::new("b", mesh)).unwrap();
        h.add_mapping(
            "b",
            "a",
            MappingMethod::NearestCell,
            vec![CouplingField::scalar("ghost", "ghost")],
        )
        .unwrap();
        let err = h.interpolate_coupling_fields().unwrap_err();
        assert!(matches!(err, CouplingError::MissingField { .. }));
    }
}
