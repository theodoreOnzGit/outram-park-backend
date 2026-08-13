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

//! OpenFOAM ASCII **case I/O** — read and write OpenFOAM cases the way the
//! upstream utilities do.
//!
//! This module is the foundation the OUTRAM PARK CLI reads/writes OpenFOAM
//! cases with. The format and algorithms are OpenFOAM-derived (the `FoamFile`
//! dictionary grammar, the `polyMesh` list layout, the time-directory field
//! layout); this is an independent Rust re-implementation of the ASCII reader
//! and writer, not the official OpenFOAM software.
//!
//! ## What lives here
//!
//! - [`dict`](crate::io::dict) — the `FoamFile` **dictionary** format: a tokeniser (strips
//!   `//` and `/* */` comments; treats `( ) { } ; [ ]` as delimiters), an
//!   in-memory AST ([`FoamDict`](crate::io::dict::FoamDict),
//!   [`FoamEntry`](crate::io::dict::FoamEntry),
//!   [`FoamValue`](crate::io::dict::FoamValue),
//!   [`Dimensioned`](crate::io::dict::Dimensioned)), the
//!   [`FoamHeader`](crate::io::dict::FoamHeader) block, and an exact-round-trip
//!   writer. Handles `system/controlDict`, `fvSchemes`, `fvSolution`-style
//!   dictionaries.
//! - [`poly_mesh`](crate::io::poly_mesh) — [`PolyMesh`](crate::io::poly_mesh::PolyMesh): read/write `constant/polyMesh/{points,
//!   faces, owner, neighbour, boundary}` and convert to the crate's
//!   geometry-carrying [`crate::mesh::FvMesh`] via
//!   [`PolyMesh::to_fv_mesh`](crate::io::poly_mesh::PolyMesh::to_fv_mesh).
//! - [`field`](crate::io::field) — read/write a time-directory field file
//!   (`0/p` volScalarField, `0/U` volVectorField): the `dimensions`,
//!   `internalField`, and `boundaryField` blocks ↔ the crate's
//!   [`crate::fields::VolScalarField`] / [`crate::fields::VolVectorField`].
//! - [`case`](crate::io::case) — [`FoamCase`](crate::io::case::FoamCase): read a whole case directory (`system/…`,
//!   `constant/polyMesh`, `0/…`) into memory, with a best-effort writer.
//!
//! ## Round-trip guarantee
//!
//! The writer and parser are inverse at the **AST level**: constructing a
//! [`FoamDict`](crate::io::dict::FoamDict) /
//! [`PolyMesh`](crate::io::poly_mesh::PolyMesh) / field, serialising it, and parsing it back
//! reproduces an equal in-memory value. (It is not a byte-for-byte re-emitter
//! of an arbitrary pre-existing file — comment banners and incidental
//! whitespace are normalised — but every value round-trips.)

pub mod case;
pub mod dict;
pub mod field;
pub mod poly_mesh;

pub use case::FoamCase;
pub use dict::{Dimensioned, FoamDict, FoamEntry, FoamHeader, FoamValue};
pub use field::{
    read_vol_scalar_field, read_vol_vector_field, write_vol_scalar_field, write_vol_vector_field,
};
pub use poly_mesh::{MeshFace, PolyMesh};

/// Errors raised while reading or writing OpenFOAM ASCII case files.
#[derive(Debug, thiserror::Error)]
pub enum IoError {
    /// An underlying filesystem error (file missing, permission denied, …).
    #[error("I/O error on {path}: {source}")]
    Io {
        /// The path being read or written when the error occurred.
        path: String,
        /// The underlying `std::io` error.
        source: std::io::Error,
    },

    /// The token stream did not match the expected grammar.
    #[error("parse error in {context}: {message}")]
    Parse {
        /// What was being parsed (file name / entry / block).
        context: String,
        /// Human-readable description of the mismatch.
        message: String,
    },

    /// A boundary-condition or field type that this reader does not yet
    /// support was encountered.
    #[error("unsupported {kind} `{name}` in {context}")]
    Unsupported {
        /// Category of the unsupported item (e.g. `"boundaryField type"`).
        kind: String,
        /// The offending type/keyword.
        name: String,
        /// Where it was found.
        context: String,
    },

    /// The parsed topology could not be assembled into a valid mesh.
    #[error("mesh construction error: {0}")]
    Mesh(String),
}

impl IoError {
    /// Build a [`IoError::Parse`] from a context label and message.
    pub(crate) fn parse(context: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Parse {
            context: context.into(),
            message: message.into(),
        }
    }

    /// Build a [`IoError::Io`] wrapping a filesystem error at `path`.
    pub(crate) fn io(path: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
