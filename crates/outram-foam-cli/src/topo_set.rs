// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Algorithm reference (translated to Rust, operation for operation):
//   OpenFOAM ESI v2506 (OpenCFD Ltd.), applications/utilities/mesh/
//   manipulation/topoSet/topoSet.C and src/meshTools/topoSet/topoSets/
//   topoSet.C. Copyright (C) 2011-2017 OpenFOAM Foundation,
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

//! **`topoSet`** (gh:#1384): run the actions of `system/topoSetDict` on
//! `cellSet`s, writing `constant/polyMesh/sets/<name>`.
//!
//! ## Ported
//!
//! - set type `cellSet`; actions `new`, `add`, `subtract`, `subset`,
//!   `invert`, `clear`, `remove`;
//! - sources `boxToCell`, `sphereToCell`, `cylinderToCell`
//!   ([`outram_foam_mesh::topo_set::CellSource`]); the source entries are read
//!   from `sourceInfo` when present, else from the action dictionary itself
//!   (`optionalSubDict`);
//! - the **write order** of a set is upstream's `labelHashSet` iteration order,
//!   reproduced by [`outram_foam_mesh::topo_set::LabelHashSet`]: a set that is
//!   dense and small-labelled prints ascending, a sparse one does not.
//!
//! ## Deferred (gh:#1384)
//!
//! `faceSet`, `pointSet` and the zone set types (`cellZoneSet`, ...), every
//! source other than the three above, `-dict`, `-region`, `-time`,
//! `-noSync` (no coupled patches exist in this reader), and re-reading a
//! changed mesh between times.

use std::path::Path;

use outram_foam_basic_lib::io::dict::{tokenize, FoamDict, FoamEntry, FoamFile, FoamValue};
use outram_foam_basic_lib::io::esi::{write_label_set_esi, EsiWriteOptions};
use outram_foam_basic_lib::io::PolyMesh;
use outram_foam_mesh::topo_set::{CellSource, LabelHashSet, SetAction};

use crate::{CliError, WriteFlavour};

/// What a run did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TopoSetReport {
    /// `Info<<` lines (without the banner), in order.
    pub log: Vec<String>,
}

/// The labels of a `topoSet` file, in file order.
///
/// # Errors
/// [`CliError::Io`] if the file is unreadable or is not a `N(...)` list.
pub fn read_label_set(path: &Path) -> Result<Vec<usize>, CliError> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| CliError::Io(format!("{}: {e}", path.display())))?;
    let toks = tokenize(&text);
    // Skip the FoamFile header: everything up to and including its `}`.
    let start = toks
        .iter()
        .position(|t| t.text == "}")
        .map(|i| i + 1)
        .ok_or_else(|| CliError::Io(format!("{}: no FoamFile header", path.display())))?;
    let mut it = toks[start..].iter().map(|t| t.text.as_str());
    let bad = || CliError::Io(format!("{}: not a label list", path.display()));
    let n: usize = it.next().ok_or_else(bad)?.parse().map_err(|_| bad())?;
    if it.next() != Some("(") {
        return Err(bad());
    }
    let mut out = Vec::with_capacity(n);
    for t in it {
        if t == ")" {
            break;
        }
        out.push(t.parse::<usize>().map_err(|_| bad())?);
    }
    if out.len() != n {
        return Err(bad());
    }
    Ok(out)
}

fn action_dicts(dict: &FoamDict) -> Result<Vec<FoamDict>, CliError> {
    match dict.get("actions") {
        Some(FoamEntry::List(items)) => items
            .iter()
            .map(|v| match v {
                FoamValue::Dict(d) => Ok(d.clone()),
                _ => Err(CliError::Tool("topoSetDict: `actions` must hold dictionaries".into())),
            })
            .collect(),
        _ => Err(CliError::Tool("topoSetDict: no `actions` list".into())),
    }
}

fn word(d: &FoamDict, key: &str) -> Result<String, CliError> {
    d.get_word(key)
        .map(str::to_string)
        .ok_or_else(|| CliError::Tool(format!("topoSetDict: action has no `{key}` word")))
}

/// Run `topoSet` on `case`.
///
/// # Errors
/// [`CliError::Io`] for unreadable/unwritable files; [`CliError::Tool`] for
/// an action, set type or source that is not ported.
pub fn run_topo_set(case: &Path, flavour: WriteFlavour) -> Result<TopoSetReport, CliError> {
    let io = |e: outram_foam_basic_lib::io::IoError| CliError::Io(e.to_string());
    // ESI is the layout written for both flavours: the set file has no
    // project-specific form (integers only).
    let _ = flavour;
    let poly = PolyMesh::read(case.join("constant/polyMesh")).map_err(io)?;
    let mesh = poly.to_fv_mesh().map_err(io)?;
    let dict = FoamFile::read(case.join("system/topoSetDict")).map_err(io)?.dict;
    let sets_dir = case.join("constant/polyMesh/sets");
    let opts = EsiWriteOptions::default();
    let mut rep = TopoSetReport::default();
    rep.log.push("Reading topoSetDict".into());

    for d in action_dicts(&dict)? {
        let name = word(&d, "name")?;
        let set_type = word(&d, "type")?;
        if set_type != "cellSet" {
            return Err(CliError::Tool(format!(
                "topoSet: set type `{set_type}` is not ported (gh:#1384 deferred; cellSet only)"
            )));
        }
        let action = word(&d, "action")?;
        let path = sets_dir.join(&name);
        let write = |set: &LabelHashSet| {
            write_label_set_esi(
                &sets_dir,
                "cellSet",
                "constant/polyMesh/sets",
                &name,
                &set.iter_in_write_order(),
                &opts,
            )
            .map_err(io)
        };
        let source = |d: &FoamDict| -> Result<CellSource, CliError> {
            let kind = word(d, "source")?;
            let info = d.get_dict("sourceInfo").unwrap_or(d);
            CellSource::from_dict(&kind, info)
                .map_err(|e| CliError::Tool(e.to_string()))?
                .ok_or_else(|| {
                    CliError::Tool(format!("topoSet: source `{kind}` is not ported (gh:#1384 deferred)"))
                })
        };
        let mut current: Option<LabelHashSet> = None;
        match action.as_str() {
            "new" | "clear" => {
                current = Some(LabelHashSet::with_capacity(16384));
                rep.log.push(format!("Created cellSet {name}"));
            }
            "remove" => {}
            "add" | "subtract" | "subset" | "invert" => {
                let s = LabelHashSet::read_back(&read_label_set(&path)?);
                rep.log
                    .push(format!("Read set cellSet {name} size:{}", s.len()));
                current = Some(s);
            }
            other => {
                return Err(CliError::Tool(format!(
                    "topoSet: action `{other}` is not ported (gh:#1384 deferred)"
                )))
            }
        }
        match action.as_str() {
            "new" | "add" | "subtract" => {
                let src = source(&d)?;
                rep.log.push(format!("    Applying source {}", word(&d, "source")?));
                let act = match action.as_str() {
                    "new" => SetAction::New,
                    "add" => SetAction::Add,
                    _ => SetAction::Subtract,
                };
                let set = current.as_mut().expect("set loaded");
                src.apply_to_hash_set(act, &mesh.cell_centres, set);
                write(set)?;
            }
            "subset" => {
                let src = source(&d)?;
                rep.log.push(format!("    Applying source {}", word(&d, "source")?));
                let set = current.as_mut().expect("set loaded");
                let old = set.clone();
                set.clear();
                src.apply_to_hash_set(SetAction::New, &mesh.cell_centres, set);
                set.retain_in(&old);
                write(set)?;
            }
            "clear" => {
                rep.log.push("    Clearing cellSet".into());
                write(current.as_ref().expect("set created"))?;
            }
            "invert" => {
                rep.log.push("    Inverting cellSet".into());
                let set = current.as_mut().expect("set loaded");
                set.invert(mesh.n_cells);
                write(set)?;
            }
            _ => {
                rep.log.push("    Removing set".into());
                if path.is_file() {
                    std::fs::remove_file(&path)
                        .map_err(|e| CliError::Io(format!("{}: {e}", path.display())))?;
                }
            }
        }
        if let Some(s) = &current {
            rep.log
                .push(format!("    cellSet {name} now size {}", s.len()));
        }
    }
    Ok(rep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_list_parses_and_label_file_round_trips() {
        let d = FoamFile::parse(
            "actions ( { name a; type cellSet; action new; source boxToCell; box (0 0 0) (1 1 1); } );",
        )
        .unwrap()
        .dict;
        let a = action_dicts(&d).unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(word(&a[0], "name").unwrap(), "a");
        assert!(word(&a[0], "nothing").is_err());

        let dir = std::env::temp_dir().join("outram_topo_set_rt");
        let _ = std::fs::remove_dir_all(&dir);
        write_label_set_esi(&dir, "cellSet", "constant/polyMesh/sets", "s", &[9, 3, 4], &EsiWriteOptions::default()).unwrap();
        assert_eq!(read_label_set(&dir.join("s")).unwrap(), vec![9, 3, 4]);
        assert!(read_label_set(&dir.join("missing")).is_err());
    }
}
