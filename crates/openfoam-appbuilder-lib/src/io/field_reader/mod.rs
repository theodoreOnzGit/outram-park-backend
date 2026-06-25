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

use std::path::Path;
use openfoam_basic_lib::prelude::Vector3;
use crate::error::AppBuilderError;
use crate::io::poly_mesh::strip_foam_comments;

/// Read the `internalField` of a `volVectorField` file.
///
/// Handles:
/// - `internalField uniform (x y z);`
/// - `internalField nonuniform List<vector> N\n(\n(x y z)\n...\n);`
pub fn read_vol_vector_field(
    path:    &Path,
    n_cells: usize,
) -> Result<Vec<Vector3>, AppBuilderError> {
    let file_name = path.display().to_string();
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppBuilderError::Io { path: path.to_path_buf(), source: e })?;
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
pub fn read_vol_scalar_field(
    path:    &Path,
    n_cells: usize,
) -> Result<Vec<f64>, AppBuilderError> {
    let file_name = path.display().to_string();
    let text = std::fs::read_to_string(path)
        .map_err(|e| AppBuilderError::Io { path: path.to_path_buf(), source: e })?;
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
        None    => return Ok(None),
    };
    let after = text[pos + marker.len()..].trim_start();
    if !after.starts_with("uniform") { return Ok(None); }
    let rest = after["uniform".len()..].trim_start();

    let open = rest.find('(').ok_or_else(|| AppBuilderError::Parse {
        file: file.to_string(), line: 0,
        msg: "uniform vector: missing '('".into(),
    })?;
    let close = rest[open..].find(')').ok_or_else(|| AppBuilderError::Parse {
        file: file.to_string(), line: 0,
        msg: "uniform vector: missing ')'".into(),
    })? + open;
    parse_vector_triple(&rest[open + 1..close], file).map(Some)
}

fn parse_uniform_scalar(text: &str, file: &str) -> Result<Option<f64>, AppBuilderError> {
    let marker = "internalField";
    let pos = match text.find(marker) {
        Some(p) => p,
        None    => return Ok(None),
    };
    let after = text[pos + marker.len()..].trim_start();
    if !after.starts_with("uniform") { return Ok(None); }
    let rest = after["uniform".len()..].trim_start();

    let end = rest.find(|c: char| c.is_whitespace() || c == ';').unwrap_or(rest.len());
    rest[..end].parse::<f64>().map(Some).map_err(|_| AppBuilderError::Parse {
        file: file.to_string(), line: 0,
        msg: format!("uniform scalar: bad value {:?}", &rest[..end.min(20)]),
    })
}

// ── nonuniform list parsers ───────────────────────────────────────────────────

fn parse_nonuniform_vector_list(
    text:    &str,
    n_cells: usize,
    file:    &str,
) -> Result<Vec<Vector3>, AppBuilderError> {
    let (count, body) = nonuniform_list_body(text, file)?;
    if count != n_cells {
        return Err(AppBuilderError::Parse {
            file: file.to_string(), line: 0,
            msg: format!("field has {count} entries, mesh has {n_cells} cells"),
        });
    }
    let mut vecs = Vec::with_capacity(count);
    let mut s = body;
    while let Some(open) = s.find('(') {
        let close = s[open..].find(')').ok_or_else(|| AppBuilderError::Parse {
            file: file.to_string(), line: 0,
            msg: "nonuniform vector list: unclosed '('".into(),
        })? + open;
        vecs.push(parse_vector_triple(&s[open + 1..close], file)?);
        s = &s[close + 1..];
    }
    if vecs.len() != count {
        return Err(AppBuilderError::Parse {
            file: file.to_string(), line: 0,
            msg: format!("expected {count} vectors, parsed {}", vecs.len()),
        });
    }
    Ok(vecs)
}

fn parse_nonuniform_scalar_list(
    text:    &str,
    n_cells: usize,
    file:    &str,
) -> Result<Vec<f64>, AppBuilderError> {
    let (count, body) = nonuniform_list_body(text, file)?;
    if count != n_cells {
        return Err(AppBuilderError::Parse {
            file: file.to_string(), line: 0,
            msg: format!("field has {count} entries, mesh has {n_cells} cells"),
        });
    }
    let scalars: Vec<f64> = body.split_whitespace()
        .map(|t| t.parse::<f64>().map_err(|_| AppBuilderError::Parse {
            file: file.to_string(), line: 0,
            msg: format!("bad scalar in nonuniform list: {t:?}"),
        }))
        .collect::<Result<_, _>>()?;
    if scalars.len() != count {
        return Err(AppBuilderError::Parse {
            file: file.to_string(), line: 0,
            msg: format!("expected {count} scalars, parsed {}", scalars.len()),
        });
    }
    Ok(scalars)
}

/// Find the `internalField nonuniform List<...> N ( ... )` body.
/// Returns `(count, body_inside_parens)`.
fn nonuniform_list_body<'a>(text: &'a str, file: &str)
    -> Result<(usize, &'a str), AppBuilderError>
{
    let marker  = "internalField";
    let pos     = text.find(marker).ok_or_else(|| AppBuilderError::Parse {
        file: file.to_string(), line: 0,
        msg: "internalField not found".into(),
    })?;
    let after = &text[pos + marker.len()..];

    let non_pos = after.find("nonuniform").ok_or_else(|| AppBuilderError::Parse {
        file: file.to_string(), line: 0,
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
    let count_end = rest.find(|c: char| c.is_whitespace() || c == '(').unwrap_or(rest.len());
    let count: usize = rest[..count_end].trim().parse().map_err(|_| AppBuilderError::Parse {
        file: file.to_string(), line: 0,
        msg: format!("expected list count before '(', got {:?}", &rest[..count_end.min(20)]),
    })?;

    // Find the body between `(` and matching `)`
    let open = rest.find('(').ok_or_else(|| AppBuilderError::Parse {
        file: file.to_string(), line: 0,
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
                if depth == 0 { close = body_start + off; break; }
            }
            _ => {}
        }
    }

    Ok((count, &rest[body_start..close]))
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn parse_vector_triple(s: &str, file: &str) -> Result<Vector3, AppBuilderError> {
    let nums: Vec<f64> = s.split_whitespace()
        .map(|t| t.parse::<f64>().map_err(|_| AppBuilderError::Parse {
            file: file.to_string(), line: 0,
            msg: format!("bad float in vector triple: {t:?}"),
        }))
        .collect::<Result<_, _>>()?;
    if nums.len() != 3 {
        return Err(AppBuilderError::Parse {
            file: file.to_string(), line: 0,
            msg: format!("vector has {} components, expected 3", nums.len()),
        });
    }
    Ok(Vector3::new(nums[0], nums[1], nums[2]))
}
