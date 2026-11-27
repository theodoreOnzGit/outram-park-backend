// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Upstream: GeN-Foam 652b3da on OpenFOAM-ESI v2506 (`runTime.write()` of every registered field object)
//   OpenFOAM-v2506/src/OpenFOAM/fields/GeometricFields/GeometricField/GeometricFieldIO.C (`writeData`)
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full notice.

//! # Helpers to describe a region's fields as `runTime.write()` writes them (gh:#1931, README round 68)
//!
//! `no_std` + `alloc`: pure descriptions ([`FieldFile`]) built from arrays; [`super::region_solvers::RegionSolvers`] assembles the whole
//! directory (`std`). The class and `dimensionSet` of every field object are [`FIELD_META`], read off the real written directory.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
use outram_foam_basic_lib::mesh::{BoundaryPatch, FvMesh, PatchKind};
use outram_foam_basic_lib::primitives::Vector3;

use crate::io::foam_ascii::{Data, FieldFile, PatchOut};

pub(super) use super::written_state_meta::FIELD_META;

/// `(class, dimensionSet)` of the field object at `path` (`<region>/<name>`), if GeN-Foam writes one in this case.
#[must_use]
pub fn meta(path: &str) -> Option<(&'static str, [i32; 7])> {
    FIELD_META.iter().find(|(p, _, _)| *p == path).map(|(_, c, d)| (*c, *d))
}

/// A field file for `path` (`<region>/<name>`) at time directory `time_name`; `None` if `path` has no registered class.
#[must_use]
pub fn field_file(path: &str, time_name: &str, internal: Data, patches: Vec<PatchOut>) -> Option<FieldFile> {
    let (class, dimensions) = meta(path)?;
    let (region, object) = path.split_once('/')?;
    Some(FieldFile {
        class: class.to_string(),
        location: format!("{time_name}/{region}"),
        object: object.to_string(),
        dimensions,
        // `meshPhi` is created by `fvMesh::movePoints` as a plain `surfaceScalarField` (not `setOriented()`): the real file has no `oriented` entry (README round 69)
        oriented: class.starts_with("surface") && object != "meshPhi",
        internal,
        patches,
    })
}

/// `empty` on patches of kind `Empty`, `ptype` (no entries) on every other patch.
#[must_use]
pub fn plain_patches(mesh: &FvMesh, ptype: &str) -> Vec<PatchOut> {
    mesh.patches.iter().map(|p| PatchOut::plain(&p.name, if p.kind == PatchKind::Empty { "empty" } else { ptype })).collect()
}

/// `empty` on patches of kind `Empty`; on every other patch `ptype` with `value` = `fixed` (uniform).
#[must_use]
pub fn value_patches(mesh: &FvMesh, ptype: &str, fixed: Data) -> Vec<PatchOut> {
    mesh.patches
        .iter()
        .map(|p| if p.kind == PatchKind::Empty { PatchOut::plain(&p.name, "empty") } else { PatchOut::with_value(&p.name, ptype, fixed.clone()) })
        .collect()
}

/// The faces of `patch` of an array over every face.
#[must_use]
pub fn face_slice(values: &[f64], patch: &BoundaryPatch) -> Vec<f64> {
    values[patch.start..patch.start + patch.size].to_vec()
}

/// The values of a face array over the boundary faces only (indexed `face - n_internal`) on `patch`.
#[must_use]
pub fn boundary_slice(values: &[f64], mesh: &FvMesh, patch: &BoundaryPatch) -> Vec<f64> {
    let s = patch.start - mesh.n_internal_faces;
    values[s..s + patch.size].to_vec()
}

/// A vector list as writer data.
#[must_use]
pub fn vector_data(v: &[Vector3]) -> Data {
    Data::flat(3, v.iter().flat_map(|x| [x.x, x.y, x.z]).collect())
}

/// The surface field over every face: `internal` = the first `n_internal_faces`, `calculated` patch values per patch (`empty` patches none).
#[must_use]
pub fn surface_field(path: &str, time_name: &str, mesh: &FvMesh, faces: &[f64]) -> Option<FieldFile> {
    let patches = mesh
        .patches
        .iter()
        .map(|p| if p.kind == PatchKind::Empty { PatchOut::plain(&p.name, "empty") } else { PatchOut::with_value(&p.name, "calculated", Data::scalar(face_slice(faces, p))) })
        .collect();
    field_file(path, time_name, Data::scalar(faces[..mesh.n_internal_faces].to_vec()), patches)
}
