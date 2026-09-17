// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
// Derived from OpenFOAM (www.openfoam.com)
// Copyright (C) 2004-2023 OpenFOAM Foundation
// Copyright (C) 2016-2023 OpenCFD Ltd.
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

//! Whole-**case** reader (`system/`, `constant/polyMesh`, a time directory).
//!
//! [`FoamCase::read`] loads an OpenFOAM case directory into memory:
//!
//! - `system/controlDict`, `system/fvSchemes`, `system/fvSolution` (and any
//!   other `system/` dictionaries) as [`FoamFile`] dictionaries;
//! - `constant/polyMesh` as a [`PolyMesh`] (plus a derived [`FvMesh`]);
//! - the fields in a time directory (default `0/`) as [`CaseField`]s,
//!   dispatched by their `FoamFile` `class` (`volScalarField` /
//!   `volVectorField`).
//!
//! Field classes other than scalar/vector volume fields are **skipped** and
//! their file names recorded in [`FoamCase::skipped_fields`] (honest partial
//! coverage — nothing is silently lost). [`FoamCase::write`] is a best-effort
//! counterpart that re-emits the system dictionaries, the mesh, and the
//! scalar/vector fields.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::dict::FoamFile;
use super::field::{
    read_vol_scalar_field, read_vol_vector_field, write_vol_scalar_field, write_vol_vector_field,
    Dimensions,
};
use super::poly_mesh::PolyMesh;
use super::IoError;
use crate::fields::{VolScalarField, VolVectorField};
use crate::mesh::FvMesh;

/// A field loaded from a time directory, tagged by its value type.
///
/// An enum (not a trait object) so callers get exhaustive `match` handling and
/// the field lives inline.
#[derive(Debug, Clone)]
pub enum CaseField {
    /// A `volScalarField` and its dimensions.
    Scalar(VolScalarField, Dimensions),
    /// A `volVectorField` and its dimensions.
    Vector(VolVectorField, Dimensions),
}

impl CaseField {
    /// The field name (its `object`).
    pub fn name(&self) -> &str {
        match self {
            CaseField::Scalar(f, _) => &f.name,
            CaseField::Vector(f, _) => &f.name,
        }
    }
}

/// An in-memory OpenFOAM case.
#[derive(Debug, Clone)]
pub struct FoamCase {
    /// The case root directory.
    pub root: PathBuf,
    /// `system/` dictionaries, keyed by file name (`controlDict`, …).
    pub system: Vec<(String, FoamFile)>,
    /// The connectivity-carrying mesh from `constant/polyMesh`, if present.
    pub poly_mesh: Option<PolyMesh>,
    /// The geometry-carrying FV mesh derived from `poly_mesh`, if present.
    pub mesh: Option<Arc<FvMesh>>,
    /// The time directory that was read (e.g. `"0"`).
    pub time_dir: String,
    /// Fields loaded from the time directory.
    pub fields: Vec<CaseField>,
    /// Field files skipped because their `class` is not a scalar/vector volume
    /// field (`(file_name, class)`).
    pub skipped_fields: Vec<(String, String)>,
}

impl FoamCase {
    /// Read a case from `root`, using time directory `"0"`.
    pub fn read(root: impl AsRef<Path>) -> Result<Self, IoError> {
        Self::read_time(root, "0")
    }

    /// Read a case from `root`, using the given `time_dir` (e.g. `"0"`).
    pub fn read_time(root: impl AsRef<Path>, time_dir: &str) -> Result<Self, IoError> {
        let root = root.as_ref().to_path_buf();

        // system/ dictionaries.
        let mut system = Vec::new();
        let sys_dir = root.join("system");
        if sys_dir.is_dir() {
            let mut names = list_files(&sys_dir)?;
            names.sort();
            for name in names {
                let ff = FoamFile::read(sys_dir.join(&name))?;
                system.push((name, ff));
            }
        }

        // constant/polyMesh.
        let poly_dir = root.join("constant").join("polyMesh");
        let (poly_mesh, mesh) = if poly_dir.join("points").is_file() {
            let pm = PolyMesh::read(&poly_dir)?;
            let fv = Arc::new(pm.to_fv_mesh()?);
            (Some(pm), Some(fv))
        } else {
            (None, None)
        };

        // time-directory fields (need a mesh to bind boundary patches).
        let mut fields = Vec::new();
        let mut skipped_fields = Vec::new();
        let t_dir = root.join(time_dir);
        if let Some(mesh) = &mesh {
            if t_dir.is_dir() {
                let mut names = list_files(&t_dir)?;
                names.sort();
                for name in names {
                    let path = t_dir.join(&name);
                    match peek_class(&path)?.as_deref() {
                        Some("volScalarField") => {
                            let (f, d) = read_vol_scalar_field(&path, Arc::clone(mesh))?;
                            fields.push(CaseField::Scalar(f, d));
                        }
                        Some("volVectorField") => {
                            let (f, d) = read_vol_vector_field(&path, Arc::clone(mesh))?;
                            fields.push(CaseField::Vector(f, d));
                        }
                        Some(other) => skipped_fields.push((name, other.to_string())),
                        None => skipped_fields.push((name, "<no class>".to_string())),
                    }
                }
            }
        }

        Ok(Self {
            root,
            system,
            poly_mesh,
            mesh,
            time_dir: time_dir.to_string(),
            fields,
            skipped_fields,
        })
    }

    /// Look up a `system/` dictionary by file name.
    pub fn system_dict(&self, name: &str) -> Option<&FoamFile> {
        self.system.iter().find(|(n, _)| n == name).map(|(_, f)| f)
    }

    /// Best-effort write of the case back to `root`: `system/` dictionaries,
    /// `constant/polyMesh`, and the scalar/vector fields into the time
    /// directory. Skipped (unsupported-class) fields are not re-emitted.
    pub fn write(&self, root: impl AsRef<Path>) -> Result<(), IoError> {
        let root = root.as_ref();

        let sys_dir = root.join("system");
        create_dir(&sys_dir)?;
        for (name, ff) in &self.system {
            ff.write(sys_dir.join(name))?;
        }

        if let Some(pm) = &self.poly_mesh {
            pm.write(root.join("constant").join("polyMesh"))?;
        }

        let t_dir = root.join(&self.time_dir);
        if !self.fields.is_empty() {
            create_dir(&t_dir)?;
        }
        for field in &self.fields {
            match field {
                CaseField::Scalar(f, d) => write_vol_scalar_field(t_dir.join(&f.name), f, *d)?,
                CaseField::Vector(f, d) => write_vol_vector_field(t_dir.join(&f.name), f, *d)?,
            }
        }
        Ok(())
    }
}

// ── helpers ─────────────────────────────────────────────────────────────────

fn create_dir(dir: &Path) -> Result<(), IoError> {
    std::fs::create_dir_all(dir).map_err(|e| IoError::io(dir.display().to_string(), e))
}

/// List regular-file names (not sub-directories) in `dir`.
fn list_files(dir: &Path) -> Result<Vec<String>, IoError> {
    let mut out = Vec::new();
    let rd = std::fs::read_dir(dir).map_err(|e| IoError::io(dir.display().to_string(), e))?;
    for entry in rd {
        let entry = entry.map_err(|e| IoError::io(dir.display().to_string(), e))?;
        let path = entry.path();
        if path.is_file() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                out.push(name.to_string());
            }
        }
    }
    Ok(out)
}

/// Read just the `FoamFile` `class` field from a file, without parsing the
/// whole body.
fn peek_class(path: &Path) -> Result<Option<String>, IoError> {
    let text =
        std::fs::read_to_string(path).map_err(|e| IoError::io(path.display().to_string(), e))?;
    let tokens = super::dict::tokenize(&text);
    // Find the FoamFile block and its `class` entry.
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i].text == "class" {
            if let Some(v) = tokens.get(i + 1) {
                return Ok(Some(v.text.clone()));
            }
        }
        i += 1;
    }
    Ok(None)
}
