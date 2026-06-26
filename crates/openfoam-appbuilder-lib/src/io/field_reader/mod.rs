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

//! Readers for OpenFOAM field files (volScalarField, volVectorField).
//!
//! Supports both `uniform` and `nonuniform List<...>` internal fields.

use crate::error::AppBuilderError;
use crate::io::poly_mesh::strip_foam_comments;
use openfoam_basic_lib::prelude::{
    BoundaryCondition, Field, FvMesh, PatchField, Vector3, VolScalarField, VolVectorField,
};
use std::path::Path;
use std::sync::Arc;

/// Read the `internalField` of a `volVectorField` file.
///
/// Handles:
/// - `internalField uniform (x y z);`
/// - `internalField nonuniform List<vector> N\n(\n(x y z)\n...\n);`
pub fn read_vol_vector_field(path: &Path, n_cells: usize) -> Result<Vec<Vector3>, AppBuilderError> {
    let file_name = path.display().to_string();
    let text = std::fs::read_to_string(path).map_err(|e| AppBuilderError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let stripped = strip_foam_comments(&text);

    if let Some(v) = parse_uniform_vector(&stripped, &file_name)? {
        return Ok(vec![v; n_cells]);
    }

    parse_nonuniform_vector_list(&stripped, n_cells, &file_name)
}

/// Read the `internalField` of a `volScalarField` file.
///
/// Handles:
/// - `internalField uniform <value>;`
/// - `internalField nonuniform List<scalar> N\n(\n<value>\n...\n);`
pub fn read_vol_scalar_field(path: &Path, n_cells: usize) -> Result<Vec<f64>, AppBuilderError> {
    let file_name = path.display().to_string();
    let text = std::fs::read_to_string(path).map_err(|e| AppBuilderError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let stripped = strip_foam_comments(&text);

    if let Some(v) = parse_uniform_scalar(&stripped, &file_name)? {
        return Ok(vec![v; n_cells]);
    }

    parse_nonuniform_scalar_list(&stripped, n_cells, &file_name)
}

// ── uniform parsers ───────────────────────────────────────────────────────────

fn parse_uniform_vector(text: &str, file: &str) -> Result<Option<Vector3>, AppBuilderError> {
    let marker = "internalField";
    let pos = match text.find(marker) {
        Some(p) => p,
        None => return Ok(None),
    };
    let after = text[pos + marker.len()..].trim_start();
    if !after.starts_with("uniform") {
        return Ok(None);
    }
    let rest = after["uniform".len()..].trim_start();

    let open = rest.find('(').ok_or_else(|| AppBuilderError::Parse {
        file: file.to_string(),
        line: 0,
        msg: "uniform vector: missing '('".into(),
    })?;
    let close = rest[open..]
        .find(')')
        .ok_or_else(|| AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: "uniform vector: missing ')'".into(),
        })?
        + open;
    parse_vector_triple(&rest[open + 1..close], file).map(Some)
}

fn parse_uniform_scalar(text: &str, file: &str) -> Result<Option<f64>, AppBuilderError> {
    let marker = "internalField";
    let pos = match text.find(marker) {
        Some(p) => p,
        None => return Ok(None),
    };
    let after = text[pos + marker.len()..].trim_start();
    if !after.starts_with("uniform") {
        return Ok(None);
    }
    let rest = after["uniform".len()..].trim_start();

    let end = rest
        .find(|c: char| c.is_whitespace() || c == ';')
        .unwrap_or(rest.len());
    rest[..end]
        .parse::<f64>()
        .map(Some)
        .map_err(|_| AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: format!("uniform scalar: bad value {:?}", &rest[..end.min(20)]),
        })
}

// ── nonuniform list parsers ───────────────────────────────────────────────────

fn parse_nonuniform_vector_list(
    text: &str,
    n_cells: usize,
    file: &str,
) -> Result<Vec<Vector3>, AppBuilderError> {
    let (count, body) = nonuniform_list_body(text, file)?;
    if count != n_cells {
        return Err(AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: format!("field has {count} entries, mesh has {n_cells} cells"),
        });
    }
    let mut vecs = Vec::with_capacity(count);
    let mut s = body;
    while let Some(open) = s.find('(') {
        let close = s[open..].find(')').ok_or_else(|| AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: "nonuniform vector list: unclosed '('".into(),
        })? + open;
        vecs.push(parse_vector_triple(&s[open + 1..close], file)?);
        s = &s[close + 1..];
    }
    if vecs.len() != count {
        return Err(AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: format!("expected {count} vectors, parsed {}", vecs.len()),
        });
    }
    Ok(vecs)
}

fn parse_nonuniform_scalar_list(
    text: &str,
    n_cells: usize,
    file: &str,
) -> Result<Vec<f64>, AppBuilderError> {
    let (count, body) = nonuniform_list_body(text, file)?;
    if count != n_cells {
        return Err(AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: format!("field has {count} entries, mesh has {n_cells} cells"),
        });
    }
    let scalars: Vec<f64> = body
        .split_whitespace()
        .map(|t| {
            t.parse::<f64>().map_err(|_| AppBuilderError::Parse {
                file: file.to_string(),
                line: 0,
                msg: format!("bad scalar in nonuniform list: {t:?}"),
            })
        })
        .collect::<Result<_, _>>()?;
    if scalars.len() != count {
        return Err(AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: format!("expected {count} scalars, parsed {}", scalars.len()),
        });
    }
    Ok(scalars)
}

/// Find the `internalField nonuniform List<...> N ( ... )` body.
/// Returns `(count, body_inside_parens)`.
fn nonuniform_list_body<'a>(
    text: &'a str,
    file: &str,
) -> Result<(usize, &'a str), AppBuilderError> {
    let marker = "internalField";
    let pos = text.find(marker).ok_or_else(|| AppBuilderError::Parse {
        file: file.to_string(),
        line: 0,
        msg: "internalField not found".into(),
    })?;
    let after = &text[pos + marker.len()..];

    let non_pos = after
        .find("nonuniform")
        .ok_or_else(|| AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: "expected 'nonuniform' in internalField".into(),
        })?;
    let rest = after[non_pos + "nonuniform".len()..].trim_start();

    // Skip `List<scalar|vector|...>` token
    let rest = if rest.starts_with("List") {
        let gt = rest.find('>').unwrap_or(rest.len());
        rest[gt + 1..].trim_start()
    } else {
        rest
    };

    // Read the count
    let count_end = rest
        .find(|c: char| c.is_whitespace() || c == '(')
        .unwrap_or(rest.len());
    let count: usize = rest[..count_end]
        .trim()
        .parse()
        .map_err(|_| AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: format!(
                "expected list count before '(', got {:?}",
                &rest[..count_end.min(20)]
            ),
        })?;

    // Find the body between `(` and matching `)`
    let open = rest.find('(').ok_or_else(|| AppBuilderError::Parse {
        file: file.to_string(),
        line: 0,
        msg: "nonuniform list: no opening '('".into(),
    })?;
    let body_start = open + 1;
    let mut depth = 1usize;
    let mut close = body_start;
    for (off, c) in rest[body_start..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close = body_start + off;
                    break;
                }
            }
            _ => {}
        }
    }

    Ok((count, &rest[body_start..close]))
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn parse_vector_triple(s: &str, file: &str) -> Result<Vector3, AppBuilderError> {
    let nums: Vec<f64> = s
        .split_whitespace()
        .map(|t| {
            t.parse::<f64>().map_err(|_| AppBuilderError::Parse {
                file: file.to_string(),
                line: 0,
                msg: format!("bad float in vector triple: {t:?}"),
            })
        })
        .collect::<Result<_, _>>()?;
    if nums.len() != 3 {
        return Err(AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: format!("vector has {} components, expected 3", nums.len()),
        });
    }
    Ok(Vector3::new(nums[0], nums[1], nums[2]))
}

// ── BC-aware whole-field readers ──────────────────────────────────────────────
//
// These read both `internalField` and `boundaryField`, mapping each named patch
// in the file onto the matching mesh patch (by name) and translating the
// OpenFOAM BC type string into a `BoundaryCondition`.

/// Read a complete `volScalarField` (internal + boundary) bound to `mesh`.
pub fn read_vol_scalar_field_full(
    path: &Path,
    mesh: &Arc<FvMesh>,
) -> Result<VolScalarField, AppBuilderError> {
    let file = path.display().to_string();
    let internal = read_vol_scalar_field(path, mesh.n_cells)?;
    let text = std::fs::read_to_string(path).map_err(|e| AppBuilderError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let stripped = strip_foam_comments(&text);
    let specs = parse_boundary_field(&stripped, &file)?;

    let boundary: Vec<PatchField<f64>> = mesh
        .patches
        .iter()
        .map(|p| {
            let spec = specs.iter().find(|(n, _)| *n == p.name);
            let bc = match spec {
                Some((_, s)) => scalar_bc(s, &file)?,
                None => BoundaryCondition::ZeroGradient,
            };
            Ok(scalar_patch_field(bc, p.size))
        })
        .collect::<Result<_, AppBuilderError>>()?;

    Ok(VolScalarField::new(
        field_name(path),
        mesh.clone(),
        Field::new(internal),
        boundary,
    ))
}

/// Read a complete `volVectorField` (internal + boundary) bound to `mesh`.
pub fn read_vol_vector_field_full(
    path: &Path,
    mesh: &Arc<FvMesh>,
) -> Result<VolVectorField, AppBuilderError> {
    let file = path.display().to_string();
    let internal = read_vol_vector_field(path, mesh.n_cells)?;
    let text = std::fs::read_to_string(path).map_err(|e| AppBuilderError::Io {
        path: path.to_path_buf(),
        source: e,
    })?;
    let stripped = strip_foam_comments(&text);
    let specs = parse_boundary_field(&stripped, &file)?;

    let boundary: Vec<PatchField<Vector3>> = mesh
        .patches
        .iter()
        .map(|p| {
            let spec = specs.iter().find(|(n, _)| *n == p.name);
            let bc = match spec {
                Some((_, s)) => vector_bc(s, &file)?,
                None => BoundaryCondition::ZeroGradient,
            };
            Ok(vector_patch_field(bc, p.size))
        })
        .collect::<Result<_, AppBuilderError>>()?;

    Ok(VolVectorField::new(
        field_name(path),
        mesh.clone(),
        Field::new(internal),
        boundary,
    ))
}

/// Parsed BC entry: the `type` string and the optional `value` payload text.
struct PatchSpec {
    bc_type: String,
    value: Option<String>,
}

fn field_name(path: &Path) -> String {
    path.file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("field")
        .to_string()
}

/// Build the `values` payload for a scalar patch from its BC.
fn scalar_patch_field(bc: BoundaryCondition<f64>, size: usize) -> PatchField<f64> {
    let values = match &bc {
        BoundaryCondition::FixedValue(v) => Field::uniform(size, *v),
        BoundaryCondition::FixedField(f) => f.clone(),
        BoundaryCondition::Calculated(f) => f.clone(),
        _ => Field::uniform(size, 0.0),
    };
    PatchField { bc, values }
}

fn vector_patch_field(bc: BoundaryCondition<Vector3>, size: usize) -> PatchField<Vector3> {
    let values = match &bc {
        BoundaryCondition::FixedValue(v) => Field::uniform(size, *v),
        BoundaryCondition::FixedField(f) => f.clone(),
        BoundaryCondition::Calculated(f) => f.clone(),
        _ => Field::uniform(size, Vector3::ZERO),
    };
    PatchField { bc, values }
}

/// Translate an OpenFOAM scalar BC type into a `BoundaryCondition<f64>`.
///
/// Types we cannot model exactly (inletOutlet, fixedFluxPressure, calculated,
/// waveTransmissive, …) fall back to `ZeroGradient`, the closest stable
/// approximation for these tutorial cases.
fn scalar_bc(spec: &PatchSpec, file: &str) -> Result<BoundaryCondition<f64>, AppBuilderError> {
    Ok(match spec.bc_type.as_str() {
        "fixedValue" => {
            let v = spec.value.as_deref().unwrap_or("0");
            BoundaryCondition::FixedValue(parse_uniform_scalar_payload(v, file)?)
        }
        "empty" => BoundaryCondition::Empty,
        "symmetry" | "symmetryPlane" => BoundaryCondition::Symmetry,
        _ => BoundaryCondition::ZeroGradient,
    })
}

/// Translate an OpenFOAM vector BC type into a `BoundaryCondition<Vector3>`.
fn vector_bc(spec: &PatchSpec, file: &str) -> Result<BoundaryCondition<Vector3>, AppBuilderError> {
    Ok(match spec.bc_type.as_str() {
        "fixedValue" => {
            let v = spec.value.as_deref().unwrap_or("(0 0 0)");
            BoundaryCondition::FixedValue(parse_uniform_vector_payload(v, file)?)
        }
        "noSlip" => BoundaryCondition::FixedValue(Vector3::ZERO),
        "slip" => BoundaryCondition::Symmetry,
        "empty" => BoundaryCondition::Empty,
        "symmetry" | "symmetryPlane" => BoundaryCondition::Symmetry,
        _ => BoundaryCondition::ZeroGradient,
    })
}

/// Parse `uniform 0` / `uniform 1e5` payload into a scalar.
fn parse_uniform_scalar_payload(s: &str, file: &str) -> Result<f64, AppBuilderError> {
    let s = s.trim().strip_prefix("uniform").unwrap_or(s).trim();
    let end = s
        .find(|c: char| c.is_whitespace() || c == ';')
        .unwrap_or(s.len());
    s[..end].parse::<f64>().map_err(|_| AppBuilderError::Parse {
        file: file.to_string(),
        line: 0,
        msg: format!("bad scalar BC value {:?}", &s[..end.min(20)]),
    })
}

/// Parse `uniform (x y z)` payload into a vector.
fn parse_uniform_vector_payload(s: &str, file: &str) -> Result<Vector3, AppBuilderError> {
    let open = s.find('(').ok_or_else(|| AppBuilderError::Parse {
        file: file.to_string(),
        line: 0,
        msg: "vector BC value missing '('".into(),
    })?;
    let close = s[open..].find(')').ok_or_else(|| AppBuilderError::Parse {
        file: file.to_string(),
        line: 0,
        msg: "vector BC value missing ')'".into(),
    })? + open;
    parse_vector_triple(&s[open + 1..close], file)
}

/// Parse the `boundaryField { patch { type …; value …; } … }` block into a
/// list of `(patch_name, PatchSpec)`.
fn parse_boundary_field(
    stripped: &str,
    file: &str,
) -> Result<Vec<(String, PatchSpec)>, AppBuilderError> {
    let marker = "boundaryField";
    let pos = match stripped.find(marker) {
        Some(p) => p,
        None => return Ok(Vec::new()),
    };
    let after = &stripped[pos + marker.len()..];
    let open = after.find('{').ok_or_else(|| AppBuilderError::Parse {
        file: file.to_string(),
        line: 0,
        msg: "boundaryField: missing '{'".into(),
    })?;
    let end = block_end(&after[open..]).ok_or_else(|| AppBuilderError::Parse {
        file: file.to_string(),
        line: 0,
        msg: "boundaryField: unclosed '{'".into(),
    })? + open;
    let body = &after[open + 1..end];

    let mut specs = Vec::new();
    let mut s = body.trim_start();
    while !s.is_empty() {
        // Patch name = word before the next '{'
        let name_end = match s.find('{') {
            Some(i) => i,
            None => break,
        };
        let name = s[..name_end].trim().to_string();
        s = &s[name_end..];
        let blk_end = block_end(s).ok_or_else(|| AppBuilderError::Parse {
            file: file.to_string(),
            line: 0,
            msg: format!("patch {name:?}: unclosed '{{'"),
        })?;
        let block = &s[1..blk_end];
        s = s[blk_end + 1..].trim_start();
        if name.is_empty() {
            continue;
        }

        specs.push((
            name,
            PatchSpec {
                bc_type: dict_word(block, "type").unwrap_or_else(|| "zeroGradient".into()),
                value: dict_value(block, "value"),
            },
        ));
    }
    Ok(specs)
}

/// Index of the `}` matching the leading `{` in `s`.
fn block_end(s: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Extract the single word value for `key` (e.g. `type fixedValue;`).
fn dict_word(block: &str, key: &str) -> Option<String> {
    let pos = find_keyword(block, key)?;
    let after = block[pos + key.len()..].trim_start();
    let end = after
        .find(|c: char| c.is_whitespace() || c == ';')
        .unwrap_or(after.len());
    Some(after[..end].trim().to_string())
}

/// Extract everything between `key` and the terminating `;` (e.g. the
/// `uniform (1 0 0)` payload of a `value` entry).
fn dict_value(block: &str, key: &str) -> Option<String> {
    let pos = find_keyword(block, key)?;
    let after = &block[pos + key.len()..];
    let end = after.find(';')?;
    Some(after[..end].trim().to_string())
}

/// Find `key` as a whole word (preceded by start/whitespace), so that `value`
/// does not match inside `inletValue`, etc.
fn find_keyword(block: &str, key: &str) -> Option<usize> {
    let mut start = 0;
    while let Some(rel) = block[start..].find(key) {
        let abs = start + rel;
        let prev_ok = abs == 0 || block.as_bytes()[abs - 1].is_ascii_whitespace();
        let after = abs + key.len();
        let next_ok = after >= block.len() || block.as_bytes()[after].is_ascii_whitespace();
        if prev_ok && next_ok {
            return Some(abs);
        }
        start = abs + key.len();
    }
    None
}
