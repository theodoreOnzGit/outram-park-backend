// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Algorithm reference (translated to Rust, operation for operation):
//   OpenFOAM ESI v2506 (OpenCFD Ltd.), applications/utilities/preProcessing/
//   setFields/setFields.C. Copyright (C) 2011-2017 OpenFOAM Foundation,
//   (C) 2018-2024 OpenCFD Ltd. Licence: GPL-3.0-or-later.
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

//! **`setFields`** (gh:#1384): set values on a selected set of cells through
//! `system/setFieldsDict`.
//!
//! ## What is ported
//!
//! - `defaultFieldValues ( volScalarFieldValue T 300 volVectorFieldValue U (0 0 0) )`
//!   sets whole fields.
//! - `regions ( boxToCell { box (..) (..); fieldValues (..); } ... )` with the
//!   cell sources `boxToCell`, `sphereToCell`, `cylinderToCell`
//!   ([`outram_foam_mesh::topo_set`]), for `volScalarField` and
//!   `volVectorField`. The selected cells are taken in ascending order
//!   (`sortedToc`), the field is read from the start-time directory and
//!   rewritten there.
//! - Boundary treatment as upstream: after the internal values are set every
//!   patch field does `pfld = pfld.patchInternalField()`. `fixedValue` (and
//!   the `mixed` family) ignore the assignment; `calculated` takes the owner
//!   cell's value (and writes it); `zeroGradient` takes it (and does not write
//!   it).
//!
//! ## Deferred (gh:#1384)
//!
//! Face sources (`patchToFace`, ...) with `volScalarFieldValue` on boundary
//! faces, the other cell sources, tensor field types, finite-area fields,
//! `-dict`/`-region`/`-time` options, and field BC types the field reader
//! does not read (see `outram_foam_basic_lib::io::field`). Each of these
//! returns [`CliError::Tool`] naming what is missing rather than being
//! ignored. Upstream's "Field X not found" is a warning and is kept as one.

use std::path::Path;
use std::sync::Arc;

use outram_foam_basic_lib::fields::boundary::bc::BoundaryCondition;
use outram_foam_basic_lib::fields::field::Field;
use outram_foam_basic_lib::io::dict::{FoamDict, FoamEntry, FoamFile, FoamValue};
use outram_foam_basic_lib::io::esi::{format_g, EsiWriteOptions};
use outram_foam_basic_lib::io::field::{
    read_vol_scalar_field, read_vol_vector_field, write_vol_scalar_field,
    write_vol_scalar_field_esi, write_vol_vector_field, write_vol_vector_field_esi,
};
use outram_foam_basic_lib::io::PolyMesh;
use outram_foam_basic_lib::mesh::FvMesh;
use outram_foam_basic_lib::primitives::Vector3;
use outram_foam_mesh::topo_set::{CellSource, SetAction};

use crate::{CliError, WriteFlavour};

/// One `volXFieldValue name value` triple.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    /// `volScalarFieldValue name value`.
    Scalar(String, f64),
    /// `volVectorFieldValue name (x y z)`.
    Vector(String, Vector3),
}

/// What a run did, for the CLI printout and the tests.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SetFieldsReport {
    /// `Info<<` lines upstream prints (without the banner), in order.
    pub log: Vec<String>,
    /// Number of cells each region selected, in order.
    pub selected: Vec<usize>,
}

/// Parse the flat `fieldValues` / `defaultFieldValues` list.
///
/// # Errors
/// [`CliError::Tool`] for an unsupported type (tensor types, `area` fields,
/// face-set `volXFieldValue` on boundaries is a different caller) or a
/// malformed triple.
pub fn parse_field_values(items: &[FoamValue]) -> Result<Vec<FieldValue>, CliError> {
    let bad = |m: String| CliError::Tool(format!("setFieldsDict: {m}"));
    let mut out = Vec::new();
    let mut i = 0;
    while i < items.len() {
        let ty = items[i]
            .as_word()
            .ok_or_else(|| bad(format!("expected a field type, found {:?}", items[i])))?;
        let name = items
            .get(i + 1)
            .and_then(FoamValue::as_word)
            .ok_or_else(|| bad(format!("`{ty}` has no field name")))?;
        let val = items
            .get(i + 2)
            .ok_or_else(|| bad(format!("`{ty} {name}` has no value")))?;
        match ty {
            "volScalarFieldValue" => {
                let v = val
                    .as_scalar()
                    .ok_or_else(|| bad(format!("`{name}`: expected a scalar")))?;
                out.push(FieldValue::Scalar(name.to_string(), v));
            }
            "volVectorFieldValue" => {
                let v = val
                    .as_vector3()
                    .ok_or_else(|| bad(format!("`{name}`: expected a vector")))?;
                out.push(FieldValue::Vector(name.to_string(), v));
            }
            other => {
                return Err(CliError::Tool(format!(
                    "setFields: field type `{other}` is not ported (gh:#1384 deferred)"
                )))
            }
        }
        i += 3;
    }
    Ok(out)
}

/// Apply one value to the case's field file in `time_dir`. `selected` is
/// `None` for "all cells" (`labelList::null()`); a list as long as the field
/// is also all cells (upstream's `selectedCells.size() == field.size()`).
fn set_one(
    value: &FieldValue,
    mesh: &Arc<FvMesh>,
    selected: Option<&[usize]>,
    time_dir: &Path,
    time_name: &str,
    flavour: WriteFlavour,
    opts: &EsiWriteOptions,
    report: &mut SetFieldsReport,
) -> Result<(), CliError> {
    let name = match value {
        FieldValue::Scalar(n, _) | FieldValue::Vector(n, _) => n,
    };
    let path = time_dir.join(name);
    if !path.is_file() {
        report.log.push(format!("Warning: Field {name} not found"));
        return Ok(());
    }
    let all = selected.map_or(true, |s| s.len() == mesh.n_cells);
    let io = |e: outram_foam_basic_lib::io::IoError| CliError::Io(e.to_string());
    match value {
        FieldValue::Scalar(_, v) => {
            let (mut f, dims) = read_vol_scalar_field(&path, mesh.clone()).map_err(io)?;
            report.log.push(format!(
                "    - set internal values of volScalarField: {name} = {}",
                format_g(*v, 6)
            ));
            if all {
                f.internal = Field::new(vec![*v; mesh.n_cells]);
            } else if let Some(cells) = selected {
                for &c in cells {
                    f.internal[c] = *v;
                }
            }
            for (pi, pf) in f.boundary.iter_mut().enumerate() {
                let p = &mesh.patches[pi];
                match &mut pf.bc {
                    BoundaryCondition::Calculated(vals) => {
                        for k in 0..p.size {
                            vals[k] = f.internal[mesh.owner[p.start + k]];
                        }
                    }
                    BoundaryCondition::ZeroGradient => {
                        for k in 0..p.size {
                            pf.values[k] = f.internal[mesh.owner[p.start + k]];
                        }
                    }
                    _ => {}
                }
            }
            match flavour {
                WriteFlavour::Default => write_vol_scalar_field(&path, &f, dims),
                WriteFlavour::Esi => write_vol_scalar_field_esi(&path, &f, dims, time_name, opts),
            }
            .map_err(io)
        }
        FieldValue::Vector(_, v) => {
            let (mut f, dims) = read_vol_vector_field(&path, mesh.clone()).map_err(io)?;
            report.log.push(format!(
                "    - set internal values of volVectorField: {name} = ({} {} {})",
                format_g(v.x, 6),
                format_g(v.y, 6),
                format_g(v.z, 6)
            ));
            if all {
                f.internal = Field::new(vec![*v; mesh.n_cells]);
            } else if let Some(cells) = selected {
                for &c in cells {
                    f.internal[c] = *v;
                }
            }
            for (pi, pf) in f.boundary.iter_mut().enumerate() {
                let p = &mesh.patches[pi];
                match &mut pf.bc {
                    BoundaryCondition::Calculated(vals) => {
                        for k in 0..p.size {
                            vals[k] = f.internal[mesh.owner[p.start + k]];
                        }
                    }
                    BoundaryCondition::ZeroGradient => {
                        for k in 0..p.size {
                            pf.values[k] = f.internal[mesh.owner[p.start + k]];
                        }
                    }
                    _ => {}
                }
            }
            match flavour {
                WriteFlavour::Default => write_vol_vector_field(&path, &f, dims),
                WriteFlavour::Esi => write_vol_vector_field_esi(&path, &f, dims, time_name, opts),
            }
            .map_err(io)
        }
    }
}

/// The `startTime` of `system/controlDict` as the time-directory name
/// (`Time::timeName`, `%g` at `timePrecision` 6), `"0"` when absent.
pub fn start_time_name(case: &Path) -> String {
    let t = FoamFile::read(case.join("system/controlDict"))
        .ok()
        .and_then(|f| f.dict.get_scalar("startTime"))
        .unwrap_or(0.0);
    format_g(t, 6)
}

/// `regions ( kind { ... } kind { ... } )`: the (keyword, dictionary) pairs.
fn region_list(dict: &FoamDict) -> Result<Vec<(String, FoamDict)>, CliError> {
    let items = match dict.get("regions") {
        Some(FoamEntry::List(v)) => v,
        _ => return Err(CliError::Tool("setFieldsDict: no `regions` list".into())),
    };
    let mut out = Vec::new();
    let mut it = items.iter();
    while let Some(k) = it.next() {
        let kw = k
            .as_word()
            .ok_or_else(|| CliError::Tool("setFieldsDict: region keyword expected".into()))?;
        match it.next() {
            Some(FoamValue::Dict(d)) => out.push((kw.to_string(), d.clone())),
            _ => {
                return Err(CliError::Tool(format!(
                    "setFieldsDict: region `{kw}` has no dictionary"
                )))
            }
        }
    }
    Ok(out)
}

fn list_entry<'a>(d: &'a FoamDict, key: &str) -> Result<&'a [FoamValue], CliError> {
    match d.get(key) {
        Some(FoamEntry::List(v)) => Ok(v.as_slice()),
        _ => Err(CliError::Tool(format!("setFieldsDict: no `{key}` list entry"))),
    }
}

/// Run `setFields` on `case` (`<case>/system/setFieldsDict`, fields in the
/// start-time directory).
///
/// # Errors
/// [`CliError::Io`] for unreadable mesh/dictionary/field files;
/// [`CliError::Tool`] for anything in the dictionary that is not ported.
pub fn run_set_fields(case: &Path, flavour: WriteFlavour) -> Result<SetFieldsReport, CliError> {
    let io = |e: outram_foam_basic_lib::io::IoError| CliError::Io(e.to_string());
    let poly = PolyMesh::read(case.join("constant/polyMesh")).map_err(io)?;
    let mesh = Arc::new(poly.to_fv_mesh().map_err(io)?);
    let dict = FoamFile::read(case.join("system/setFieldsDict"))
        .map_err(io)?
        .dict;
    let time_name = start_time_name(case);
    let time_dir = case.join(&time_name);
    let opts = EsiWriteOptions::default().with_precision(crate::write_precision(case));
    let mut report = SetFieldsReport::default();

    if dict.get("defaultFieldValues").is_some() {
        report.log.push("Setting volume field default values".into());
        let vals = parse_field_values(list_entry(&dict, "defaultFieldValues")?)?;
        for v in &vals {
            set_one(v, &mesh, None, &time_dir, &time_name, flavour, &opts, &mut report)?;
        }
    }

    report.log.push("Setting field region values".into());
    for (kind, rd) in region_list(&dict)? {
        let src = CellSource::from_dict(&kind, &rd)
            .map_err(|e| CliError::Tool(e.to_string()))?
            .ok_or_else(|| {
                CliError::Tool(format!(
                    "setFields: source `{kind}` is not ported (gh:#1384 deferred)"
                ))
            })?;
        let mut set = std::collections::BTreeSet::new();
        src.apply(SetAction::New, &mesh.cell_centres, &mut set);
        let cells: Vec<usize> = set.into_iter().collect();
        report.log.push(format!(
            "    Selected {}/{} cells",
            cells.len(),
            mesh.n_cells
        ));
        report.selected.push(cells.len());
        let vals = parse_field_values(list_entry(&rd, "fieldValues")?)?;
        for v in &vals {
            set_one(
                v,
                &mesh,
                Some(&cells),
                &time_dir,
                &time_name,
                flavour,
                &opts,
                &mut report,
            )?;
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_value_triples_parse_and_unsupported_types_are_reported() {
        let d = FoamFile::parse("fieldValues ( volScalarFieldValue a 1.5 volVectorFieldValue U (1 2 3) );")
            .unwrap()
            .dict;
        let v = parse_field_values(list_entry(&d, "fieldValues").unwrap()).unwrap();
        assert_eq!(
            v,
            vec![
                FieldValue::Scalar("a".into(), 1.5),
                FieldValue::Vector("U".into(), Vector3::new(1.0, 2.0, 3.0))
            ]
        );
        let t = FoamFile::parse("fieldValues ( volTensorFieldValue T (1 0 0 0 1 0 0 0 1) );")
            .unwrap()
            .dict;
        assert!(parse_field_values(list_entry(&t, "fieldValues").unwrap()).is_err());
        assert!(list_entry(&t, "nothing").is_err());
    }

    #[test]
    fn regions_pair_keywords_with_dictionaries() {
        let d = FoamFile::parse(
            "regions ( boxToCell { box (0 0 0) (1 1 1); fieldValues ( volScalarFieldValue a 1 ); } \
             sphereToCell { origin (0 0 0); radius 1; fieldValues (); } );",
        )
        .unwrap()
        .dict;
        let r = region_list(&d).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!(r[0].0, "boxToCell");
        assert_eq!(r[1].0, "sphereToCell");
        assert_eq!(start_time_name(Path::new("/nonexistent")), "0");
    }
}
