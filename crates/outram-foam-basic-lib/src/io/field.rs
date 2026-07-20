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

//! Time-directory **field** file read/write (`0/p`, `0/U`, …).
//!
//! A field file carries a `dimensions [7]` block, an `internalField`
//! (`uniform <v>` or `nonuniform List<...>`), and a `boundaryField` sub-dict
//! of per-patch `{ type …; value …; }` entries. This module maps those to and
//! from the crate's [`VolScalarField`] / [`VolVectorField`].
//!
//! Because [`VolField`] itself carries no `dimensions`, the read functions
//! **return** the parsed dimension exponents alongside the field and the write
//! functions **take** them as an argument.
//!
//! ## Supported coverage
//!
//! - `internalField`: both `uniform` and `nonuniform List<scalar|vector>`.
//! - `boundaryField` types: `fixedValue` (uniform or nonuniform `value`),
//!   `zeroGradient`, `empty`, `symmetry` / `symmetryPlane`, and `calculated`.
//!
//! Any other boundary type (e.g. `inletOutlet`, `fixedFluxPressure`) raises
//! [`IoError::Unsupported`] — these are **deferred**, not silently dropped.
//!
//! [`VolField`]: crate::fields::VolField

use std::path::Path;
use std::sync::Arc;

use super::dict::{fmt_scalar, tokenize, write_header, FoamHeader, Token, BANNER, FOOTER, SEPARATOR};
use super::IoError;
use crate::fields::boundary::bc::{BoundaryCondition, PatchField};
use crate::fields::field::Field;
use crate::fields::{VolScalarField, VolVectorField};
use crate::mesh::FvMesh;
use crate::primitives::Vector3;

/// Seven SI dimension exponents in OpenFOAM order `[kg, m, s, K, mol, A, cd]`.
pub type Dimensions = [f64; 7];

// ───────────────────────────────────────────────────────────────────────────
//  Public API
// ───────────────────────────────────────────────────────────────────────────

/// Read a `volScalarField` file, returning the field and its dimensions.
///
/// The `mesh` supplies the cell count and the boundary-patch order/sizes the
/// field is defined on; the file's `boundaryField` is matched to it by patch
/// name.
pub fn read_vol_scalar_field(
    path: impl AsRef<Path>,
    mesh: Arc<FvMesh>,
) -> Result<(VolScalarField, Dimensions), IoError> {
    let path = path.as_ref();
    let text = read_to_string(path)?;
    let mut c = FieldCursor::new(&text, path.display().to_string());
    let (dims, internal_raw, boundary_raw, name) = c.parse_field_body()?;

    let internal = match internal_raw {
        InternalRaw::UniformScalar(v) => Field::new(vec![v; mesh.n_cells]),
        InternalRaw::NonuniformScalar(v) => Field::new(v),
        _ => return Err(c.err("expected a scalar internalField in a volScalarField")),
    };
    let boundary = build_scalar_boundary(&mesh, &boundary_raw, &c)?;
    let field = VolScalarField::new(name, mesh, internal, boundary);
    Ok((field, dims))
}

/// Read a `volVectorField` file, returning the field and its dimensions.
pub fn read_vol_vector_field(
    path: impl AsRef<Path>,
    mesh: Arc<FvMesh>,
) -> Result<(VolVectorField, Dimensions), IoError> {
    let path = path.as_ref();
    let text = read_to_string(path)?;
    let mut c = FieldCursor::new(&text, path.display().to_string());
    let (dims, internal_raw, boundary_raw, name) = c.parse_field_body()?;

    let internal = match internal_raw {
        InternalRaw::UniformVector(v) => Field::new(vec![v; mesh.n_cells]),
        InternalRaw::NonuniformVector(v) => Field::new(v),
        _ => return Err(c.err("expected a vector internalField in a volVectorField")),
    };
    let boundary = build_vector_boundary(&mesh, &boundary_raw, &c)?;
    let field = VolVectorField::new(name, mesh, internal, boundary);
    Ok((field, dims))
}

/// Write a `volScalarField` file to `path` with the given `dimensions`.
pub fn write_vol_scalar_field(
    path: impl AsRef<Path>,
    field: &VolScalarField,
    dimensions: Dimensions,
) -> Result<(), IoError> {
    let path = path.as_ref();
    let mut s = header_str("volScalarField", &field.name);
    write_dimensions(&mut s, &dimensions);
    write_internal_scalar(&mut s, field.internal.as_slice());
    write_boundary_scalar(&mut s, field);
    s.push('\n');
    s.push_str(FOOTER);
    std::fs::write(path, s).map_err(|e| IoError::io(path.display().to_string(), e))
}

/// Write a `volVectorField` file to `path` with the given `dimensions`.
pub fn write_vol_vector_field(
    path: impl AsRef<Path>,
    field: &VolVectorField,
    dimensions: Dimensions,
) -> Result<(), IoError> {
    let path = path.as_ref();
    let mut s = header_str("volVectorField", &field.name);
    write_dimensions(&mut s, &dimensions);
    write_internal_vector(&mut s, field.internal.as_slice());
    write_boundary_vector(&mut s, field);
    s.push('\n');
    s.push_str(FOOTER);
    std::fs::write(path, s).map_err(|e| IoError::io(path.display().to_string(), e))
}

// ───────────────────────────────────────────────────────────────────────────
//  Parsing
// ───────────────────────────────────────────────────────────────────────────

enum InternalRaw {
    UniformScalar(f64),
    NonuniformScalar(Vec<f64>),
    UniformVector(Vector3),
    NonuniformVector(Vec<Vector3>),
}

/// One patch's parsed `boundaryField` entry.
struct RawBcPatch {
    name: String,
    bc_type: String,
    /// `value` sub-entry, if present.
    value: Option<RawValue>,
}

enum RawValue {
    UniformScalar(f64),
    NonuniformScalar(Vec<f64>),
    UniformVector(Vector3),
    NonuniformVector(Vec<Vector3>),
}

/// Owns its tokens (no lifetime parameters, per crate design rules).
struct FieldCursor {
    tokens: Vec<Token>,
    pos: usize,
    context: String,
    object: String,
}

impl FieldCursor {
    fn new(text: &str, context: impl Into<String>) -> Self {
        let mut c = Self {
            tokens: tokenize(text),
            pos: 0,
            context: context.into(),
            object: String::new(),
        };
        c.read_header_object();
        c
    }

    fn err(&self, m: impl Into<String>) -> IoError {
        IoError::parse(self.context.clone(), m.into())
    }

    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.pos).map(|t| t.text.as_str())
    }

    fn next(&mut self) -> Option<String> {
        let t = self.tokens.get(self.pos).map(|t| t.text.clone());
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn expect(&mut self, tok: &str) -> Result<(), IoError> {
        match self.next() {
            Some(t) if t == tok => Ok(()),
            other => Err(self.err(format!("expected `{tok}`, found {other:?}"))),
        }
    }

    fn parse_usize(&mut self) -> Result<usize, IoError> {
        let t = self
            .next()
            .ok_or_else(|| self.err("expected an integer, found end of input"))?;
        t.parse::<usize>()
            .map_err(|_| self.err(format!("expected an integer, found `{t}`")))
    }

    fn parse_f64(&mut self) -> Result<f64, IoError> {
        let t = self
            .next()
            .ok_or_else(|| self.err("expected a number, found end of input"))?;
        t.parse::<f64>()
            .map_err(|_| self.err(format!("expected a number, found `{t}`")))
    }

    fn parse_vector(&mut self) -> Result<Vector3, IoError> {
        self.expect("(")?;
        let x = self.parse_f64()?;
        let y = self.parse_f64()?;
        let z = self.parse_f64()?;
        self.expect(")")?;
        Ok(Vector3::new(x, y, z))
    }

    /// Read the object name out of the `FoamFile` header, leaving the cursor
    /// positioned just after the header's closing `}`.
    fn read_header_object(&mut self) {
        if self.peek() != Some("FoamFile") {
            return;
        }
        self.next(); // FoamFile
        // opening brace
        while self.peek().is_some() && self.peek() != Some("{") {
            self.next();
        }
        self.next(); // {
        let mut depth = 1usize;
        while depth > 0 {
            let key = match self.next() {
                Some(k) => k.to_string(),
                None => return,
            };
            match key.as_str() {
                "{" => depth += 1,
                "}" => depth -= 1,
                "object" => {
                    if let Some(v) = self.next() {
                        self.object = v.to_string();
                    }
                    // consume ';'
                    if self.peek() == Some(";") {
                        self.next();
                    }
                }
                _ => {
                    // skip to ';' (header entries are simple)
                    while self.peek().is_some()
                        && self.peek() != Some(";")
                        && self.peek() != Some("}")
                    {
                        self.next();
                    }
                    if self.peek() == Some(";") {
                        self.next();
                    }
                }
            }
        }
    }

    /// Parse the field body: `dimensions`, `internalField`, `boundaryField`.
    fn parse_field_body(
        &mut self,
    ) -> Result<(Dimensions, InternalRaw, Vec<RawBcPatch>, String), IoError> {
        let mut dims: Dimensions = [0.0; 7];
        let mut internal: Option<InternalRaw> = None;
        let mut boundary: Vec<RawBcPatch> = Vec::new();
        let name = if self.object.is_empty() {
            "field".to_string()
        } else {
            self.object.clone()
        };

        while let Some(kw) = self.peek() {
            match kw {
                "dimensions" => {
                    self.next();
                    dims = self.parse_dimensions()?;
                    self.expect(";")?;
                }
                "internalField" => {
                    self.next();
                    internal = Some(self.parse_internal()?);
                    self.expect(";")?;
                }
                "boundaryField" => {
                    self.next();
                    boundary = self.parse_boundary_field()?;
                }
                other => {
                    return Err(self.err(format!("unexpected keyword `{other}` in field file")));
                }
            }
        }

        let internal = internal.ok_or_else(|| self.err("field file has no internalField"))?;
        Ok((dims, internal, boundary, name))
    }

    fn parse_dimensions(&mut self) -> Result<Dimensions, IoError> {
        self.expect("[")?;
        let mut d = [0.0f64; 7];
        for slot in d.iter_mut() {
            *slot = self.parse_f64()?;
        }
        self.expect("]")?;
        Ok(d)
    }

    /// Parse `uniform <v>` or `nonuniform List<scalar|vector> N ( … )`.
    fn parse_internal(&mut self) -> Result<InternalRaw, IoError> {
        match self.next().as_deref() {
            Some("uniform") => {
                if self.peek() == Some("(") {
                    Ok(InternalRaw::UniformVector(self.parse_vector()?))
                } else {
                    Ok(InternalRaw::UniformScalar(self.parse_f64()?))
                }
            }
            Some("nonuniform") => {
                let tag = self
                    .next()
                    .ok_or_else(|| self.err("expected List<…> after nonuniform"))?
                    .to_string();
                let is_vector = tag.contains("vector");
                let count = self.parse_usize()?;
                self.expect("(")?;
                if is_vector {
                    let mut v = Vec::with_capacity(count);
                    for _ in 0..count {
                        v.push(self.parse_vector()?);
                    }
                    self.expect(")")?;
                    Ok(InternalRaw::NonuniformVector(v))
                } else {
                    let mut v = Vec::with_capacity(count);
                    for _ in 0..count {
                        v.push(self.parse_f64()?);
                    }
                    self.expect(")")?;
                    Ok(InternalRaw::NonuniformScalar(v))
                }
            }
            other => Err(self.err(format!(
                "expected `uniform` or `nonuniform`, found {other:?}"
            ))),
        }
    }

    fn parse_boundary_field(&mut self) -> Result<Vec<RawBcPatch>, IoError> {
        self.expect("{")?;
        let mut patches = Vec::new();
        while self.peek() != Some("}") {
            let name = self
                .next()
                .ok_or_else(|| self.err("expected a patch name in boundaryField"))?
                .to_string();
            self.expect("{")?;
            let mut bc_type = String::new();
            let mut value: Option<RawValue> = None;
            while self.peek() != Some("}") {
                let key = self
                    .next()
                    .ok_or_else(|| self.err("unexpected end of input in patch entry"))?
                    .to_string();
                match key.as_str() {
                    "type" => {
                        bc_type = self
                            .next()
                            .ok_or_else(|| self.err("expected a boundary type"))?
                            .to_string();
                        self.expect(";")?;
                    }
                    "value" => {
                        value = Some(self.parse_value()?);
                        self.expect(";")?;
                    }
                    _ => {
                        // Skip an arbitrary sub-entry up to ';', balancing parens.
                        let mut depth = 0usize;
                        loop {
                            match self.next().as_deref() {
                                Some("(") => depth += 1,
                                Some(")") => depth = depth.saturating_sub(1),
                                Some(";") if depth == 0 => break,
                                Some(_) => {}
                                None => return Err(self.err("unterminated patch entry")),
                            }
                        }
                    }
                }
            }
            self.expect("}")?;
            patches.push(RawBcPatch {
                name,
                bc_type,
                value,
            });
        }
        self.expect("}")?;
        Ok(patches)
    }

    /// Parse a `value` sub-entry: `uniform <v>` or `nonuniform List<…> N ( … )`.
    fn parse_value(&mut self) -> Result<RawValue, IoError> {
        match self.parse_internal()? {
            InternalRaw::UniformScalar(v) => Ok(RawValue::UniformScalar(v)),
            InternalRaw::NonuniformScalar(v) => Ok(RawValue::NonuniformScalar(v)),
            InternalRaw::UniformVector(v) => Ok(RawValue::UniformVector(v)),
            InternalRaw::NonuniformVector(v) => Ok(RawValue::NonuniformVector(v)),
        }
    }
}

// ── Boundary reconstruction ─────────────────────────────────────────────────

fn find_patch<'a>(raw: &'a [RawBcPatch], name: &str) -> Option<&'a RawBcPatch> {
    raw.iter().find(|p| p.name == name)
}

fn build_scalar_boundary(
    mesh: &FvMesh,
    raw: &[RawBcPatch],
    c: &FieldCursor,
) -> Result<Vec<PatchField<f64>>, IoError> {
    let mut out = Vec::with_capacity(mesh.patches.len());
    for patch in &mesh.patches {
        let size = patch.size;
        let rp = find_patch(raw, &patch.name).ok_or_else(|| {
            c.err(format!("boundaryField is missing patch `{}`", patch.name))
        })?;
        let (bc, values) = match rp.bc_type.as_str() {
            "fixedValue" | "calculated" => {
                let (bc_ctor, field) = match &rp.value {
                    Some(RawValue::UniformScalar(v)) => {
                        (BoundaryCondition::FixedValue(*v), Field::new(vec![*v; size]))
                    }
                    Some(RawValue::NonuniformScalar(v)) => {
                        (BoundaryCondition::FixedField(Field::new(v.clone())), Field::new(v.clone()))
                    }
                    _ => {
                        return Err(c.err(format!(
                            "patch `{}` type `{}` requires a scalar `value`",
                            patch.name, rp.bc_type
                        )))
                    }
                };
                if rp.bc_type == "calculated" {
                    (BoundaryCondition::Calculated(field.clone()), field)
                } else {
                    (bc_ctor, field)
                }
            }
            "zeroGradient" => (BoundaryCondition::ZeroGradient, Field::zeros(size)),
            "empty" => (BoundaryCondition::Empty, Field::new(vec![])),
            "symmetry" | "symmetryPlane" => {
                (BoundaryCondition::Symmetry, Field::zeros(size))
            }
            other => {
                return Err(IoError::Unsupported {
                    kind: "boundaryField type".into(),
                    name: other.into(),
                    context: format!("patch `{}`", patch.name),
                })
            }
        };
        out.push(PatchField { bc, values });
    }
    Ok(out)
}

fn build_vector_boundary(
    mesh: &FvMesh,
    raw: &[RawBcPatch],
    c: &FieldCursor,
) -> Result<Vec<PatchField<Vector3>>, IoError> {
    let mut out = Vec::with_capacity(mesh.patches.len());
    for patch in &mesh.patches {
        let size = patch.size;
        let rp = find_patch(raw, &patch.name).ok_or_else(|| {
            c.err(format!("boundaryField is missing patch `{}`", patch.name))
        })?;
        let (bc, values) = match rp.bc_type.as_str() {
            "fixedValue" | "calculated" => {
                let field = match &rp.value {
                    Some(RawValue::UniformVector(v)) => Field::new(vec![*v; size]),
                    Some(RawValue::NonuniformVector(v)) => Field::new(v.clone()),
                    _ => {
                        return Err(c.err(format!(
                            "patch `{}` type `{}` requires a vector `value`",
                            patch.name, rp.bc_type
                        )))
                    }
                };
                let bc = if rp.bc_type == "calculated" {
                    BoundaryCondition::Calculated(field.clone())
                } else {
                    match &rp.value {
                        Some(RawValue::UniformVector(v)) => BoundaryCondition::FixedValue(*v),
                        _ => BoundaryCondition::FixedField(field.clone()),
                    }
                };
                (bc, field)
            }
            "zeroGradient" => (BoundaryCondition::ZeroGradient, Field::zero_vec(size)),
            "empty" => (BoundaryCondition::Empty, Field::new(vec![])),
            "symmetry" | "symmetryPlane" => {
                (BoundaryCondition::Symmetry, Field::zero_vec(size))
            }
            other => {
                return Err(IoError::Unsupported {
                    kind: "boundaryField type".into(),
                    name: other.into(),
                    context: format!("patch `{}`", patch.name),
                })
            }
        };
        out.push(PatchField { bc, values });
    }
    Ok(out)
}

// ───────────────────────────────────────────────────────────────────────────
//  Writing
// ───────────────────────────────────────────────────────────────────────────

fn write_dimensions(s: &mut String, d: &Dimensions) {
    s.push_str("dimensions      [");
    for (i, e) in d.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(&fmt_scalar(*e));
    }
    s.push_str("];\n\n");
}

fn all_equal_scalar(v: &[f64]) -> Option<f64> {
    let first = *v.first()?;
    if v.iter().all(|x| *x == first) {
        Some(first)
    } else {
        None
    }
}

fn all_equal_vector(v: &[Vector3]) -> Option<Vector3> {
    let first = *v.first()?;
    if v.iter().all(|x| *x == first) {
        Some(first)
    } else {
        None
    }
}

fn fmt_vector(v: Vector3) -> String {
    format!("({} {} {})", fmt_scalar(v.x), fmt_scalar(v.y), fmt_scalar(v.z))
}

fn write_internal_scalar(s: &mut String, v: &[f64]) {
    match all_equal_scalar(v) {
        Some(u) => {
            s.push_str(&format!("internalField   uniform {};\n\n", fmt_scalar(u)));
        }
        None => {
            s.push_str(&format!(
                "internalField   nonuniform List<scalar> \n{}\n(\n",
                v.len()
            ));
            for x in v {
                s.push_str(&fmt_scalar(*x));
                s.push('\n');
            }
            s.push_str(")\n;\n\n");
        }
    }
}

fn write_internal_vector(s: &mut String, v: &[Vector3]) {
    match all_equal_vector(v) {
        Some(u) => {
            s.push_str(&format!("internalField   uniform {};\n\n", fmt_vector(u)));
        }
        None => {
            s.push_str(&format!(
                "internalField   nonuniform List<vector> \n{}\n(\n",
                v.len()
            ));
            for x in v {
                s.push_str(&fmt_vector(*x));
                s.push('\n');
            }
            s.push_str(")\n;\n\n");
        }
    }
}

fn write_boundary_scalar(s: &mut String, field: &VolScalarField) {
    s.push_str("boundaryField\n{\n");
    for (patch, pf) in field.mesh.patches.iter().zip(&field.boundary) {
        s.push_str(&format!("    {}\n    {{\n", patch.name));
        match &pf.bc {
            BoundaryCondition::FixedValue(v) => {
                s.push_str("        type            fixedValue;\n");
                s.push_str(&format!("        value           uniform {};\n", fmt_scalar(*v)));
            }
            BoundaryCondition::FixedField(f) => {
                s.push_str("        type            fixedValue;\n");
                write_value_scalar(s, f.as_slice());
            }
            BoundaryCondition::Calculated(f) => {
                s.push_str("        type            calculated;\n");
                write_value_scalar(s, f.as_slice());
            }
            BoundaryCondition::ZeroGradient => {
                s.push_str("        type            zeroGradient;\n");
            }
            BoundaryCondition::Symmetry => {
                s.push_str("        type            symmetry;\n");
            }
            BoundaryCondition::Empty => {
                s.push_str("        type            empty;\n");
            }
        }
        s.push_str("    }\n");
    }
    s.push_str("}\n");
}

fn write_boundary_vector(s: &mut String, field: &VolVectorField) {
    s.push_str("boundaryField\n{\n");
    for (patch, pf) in field.mesh.patches.iter().zip(&field.boundary) {
        s.push_str(&format!("    {}\n    {{\n", patch.name));
        match &pf.bc {
            BoundaryCondition::FixedValue(v) => {
                s.push_str("        type            fixedValue;\n");
                s.push_str(&format!("        value           uniform {};\n", fmt_vector(*v)));
            }
            BoundaryCondition::FixedField(f) => {
                s.push_str("        type            fixedValue;\n");
                write_value_vector(s, f.as_slice());
            }
            BoundaryCondition::Calculated(f) => {
                s.push_str("        type            calculated;\n");
                write_value_vector(s, f.as_slice());
            }
            BoundaryCondition::ZeroGradient => {
                s.push_str("        type            zeroGradient;\n");
            }
            BoundaryCondition::Symmetry => {
                s.push_str("        type            symmetry;\n");
            }
            BoundaryCondition::Empty => {
                s.push_str("        type            empty;\n");
            }
        }
        s.push_str("    }\n");
    }
    s.push_str("}\n");
}

fn write_value_scalar(s: &mut String, v: &[f64]) {
    if let Some(u) = all_equal_scalar(v) {
        s.push_str(&format!("        value           uniform {};\n", fmt_scalar(u)));
        return;
    }
    s.push_str(&format!(
        "        value           nonuniform List<scalar> {} (",
        v.len()
    ));
    for (i, x) in v.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(&fmt_scalar(*x));
    }
    s.push_str(");\n");
}

fn write_value_vector(s: &mut String, v: &[Vector3]) {
    if let Some(u) = all_equal_vector(v) {
        s.push_str(&format!("        value           uniform {};\n", fmt_vector(u)));
        return;
    }
    s.push_str(&format!(
        "        value           nonuniform List<vector> {} (",
        v.len()
    ));
    for (i, x) in v.iter().enumerate() {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(&fmt_vector(*x));
    }
    s.push_str(");\n");
}

// ── file helpers ────────────────────────────────────────────────────────────

fn read_to_string(path: &Path) -> Result<String, IoError> {
    std::fs::read_to_string(path).map_err(|e| IoError::io(path.display().to_string(), e))
}

fn header_str(class: &str, object: &str) -> String {
    let mut s = String::new();
    s.push_str(BANNER);
    let header: FoamHeader = FoamHeader::standard(class, object);
    write_header(&mut s, &header);
    s.push_str(SEPARATOR);
    s.push_str("\n\n");
    s
}
