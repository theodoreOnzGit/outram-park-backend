// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Upstream: GeN-Foam 652b3da
//   GeN-Foam/GeN-Foam.C:129,156,165               `mappingMode`, `solvers.mapper().initializeMappedFields(runTime, mappingMode)`, no time loop
//   src/classes/multiRegion/meshHandler/meshHandler.C:332-386, meshHandlerTemplates.C:95-148   `initializeMappedFields`, `mapAndWrite`
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full notice.

//! # `GeN-Foam -initializeMappedFields` (gh:#1961, README round 81)
//!
//! [`RegionSolvers::initialize_mapped_fields`]: with the regions constructed from the start directory, one `mapAllFields` pass (the same
//! target-region / source-region order, `meshToMesh::mapTgtToSrc` with `plusEqOp` as [`RegionSolvers::step`] uses) and the `write()` of
//! every mapped TARGET field that exists, into `<start time>/<target region>/<field>`. Upstream then leaves the time loop (`mappingMode`).
//! A field is written `zeroGradient` on its non-constraint patches (`cyclic`, `empty` on those), which is what every mapped field of the
//! 1-D MSR tutorials carries; the numbers are what the transient reads back.

use std::path::Path;

use outram_foam_basic_lib::mesh::PatchKind;

use super::{case_err, RegionSolvers};
use crate::error::AppBuilderError;
use crate::genfoam::multi_region::written_state::vector_data;
use crate::io::foam_ascii::{write_field_file, Data, FieldFile, PatchOut, WriteOptions};

/// `(class, dimensionSet)` of a mapped target field of the 1-D MSR coupling dictionaries (the real `-initializeMappedFields` files).
fn mapped_meta(region: &str, field: &str) -> Option<(&'static str, [i32; 7])> {
    let t = [0, 0, 0, 1, 0, 0, 0];
    let q = [1, -1, -3, 0, 0, 0, 0];
    match (region, field) {
        ("neutroRegion", "U") => Some(("volVectorField", [0, 1, -1, 0, 0, 0, 0])),
        ("neutroRegion", "alpha") => Some(("volScalarField", [0, 0, 0, 0, 0, 0, 0])),
        ("neutroRegion", "mu" | "alphat") => Some(("volScalarField", [1, -1, -1, 0, 0, 0, 0])),
        ("neutroRegion", "rhoCool") => Some(("volScalarField", [1, -3, 0, 0, 0, 0, 0])),
        ("neutroRegion", "TFuel" | "TClad" | "TCool" | "TStruct" | "TStructMech") => Some(("volScalarField", t)),
        ("fluidRegion", "powerDensityNeutronics" | "powerDensityNeutronicsToLiquid") => Some(("volScalarField", q)),
        _ => None,
    }
}

impl RegionSolvers {
    /// `GeN-Foam -initializeMappedFields`: map every coupling-dictionary field once and write each mapped target field that exists into the
    /// start time's directory under `root`. Returns the written paths (relative to `root`, in mapping order).
    ///
    /// # Errors
    /// [`AppBuilderError`] for a mapping without its source field, a mapped field this writer has no class for (refused by name), or a file that
    /// cannot be written.
    pub fn initialize_mapped_fields(&mut self, root: &Path, opts: &WriteOptions) -> Result<Vec<String>, AppBuilderError> {
        self.map_all_fields()?;
        let tn = self.time_directory_name();
        let mut written = Vec::new();
        for entry in &self.mappings {
            let Some(ti) = self.regions.iter().position(|r| r.name == entry.target_region) else { continue };
            if !self.regions.iter().any(|r| r.name == entry.source_region) {
                continue;
            }
            let r = &self.regions[ti];
            for tf in &entry.target_fields {
                if self.switches.disabled_targets.iter().any(|(rg, f)| rg == &entry.target_region && f == tf) {
                    continue;
                }
                let data = if let Some(v) = r.fields.scalar(tf) {
                    Data::scalar(v.to_vec())
                } else if let Some(v) = r.fields.vector(tf) {
                    vector_data(v)
                } else {
                    // `mapAndWrite`: a target field the region does not register is skipped
                    continue;
                };
                let Some((class, dimensions)) = mapped_meta(&r.name, tf) else {
                    return Err(case_err(root, format!("-initializeMappedFields: {}/{tf} has no class here (refused by name)", r.name)));
                };
                let patches = r
                    .fv
                    .patches
                    .iter()
                    .map(|p| match p.kind {
                        PatchKind::Empty => PatchOut::plain(&p.name, "empty"),
                        PatchKind::Cyclic => PatchOut::plain(&p.name, "cyclic"),
                        _ => PatchOut::plain(&p.name, "zeroGradient"),
                    })
                    .collect();
                let f = FieldFile {
                    class: class.to_string(),
                    location: format!("{tn}/{}", r.name),
                    object: tf.clone(),
                    dimensions,
                    oriented: false,
                    internal: data,
                    patches,
                };
                let text = write_field_file(&f, opts).map_err(|e| case_err(root, e.to_string()))?;
                let rel = format!("{tn}/{}/{tf}", r.name);
                let path = root.join(&rel);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| AppBuilderError::Io { path: parent.to_path_buf(), source: e })?;
                }
                std::fs::write(&path, text).map_err(|e| AppBuilderError::Io { path: path.clone(), source: e })?;
                written.push(rel);
            }
        }
        Ok(written)
    }
}
