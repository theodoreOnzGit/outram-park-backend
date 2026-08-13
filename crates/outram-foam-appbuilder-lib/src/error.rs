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

//! # `error` — the crate's single error type
//!
//! Every fallible public function in `outram-foam-appbuilder-lib` returns
//! [`AppBuilderError`], so a caller matches one enum across case-file parsing
//! and the solver time loops rather than juggling a per-module error type.
//!
//! Variants that carry a file path or line number always refer to the OpenFOAM
//! case file being read; variants carrying a residual or iteration count come
//! from a solver loop.
//!
//! Note that the *unimplemented* parts of this crate (the `todo!()` dictionary
//! readers and field writers — see the crate-root docs) **panic** rather than
//! returning an error variant. `AppBuilderError` reports genuine runtime
//! failures, not missing features.

use std::path::PathBuf;
use thiserror::Error;

/// Errors returned by this crate's case I/O and solver-loop entry points.
///
/// Every fallible public function in `outram-foam-appbuilder-lib` reports
/// through this single enum, so a caller matches one error type across mesh/
/// dictionary parsing and the time-advancement loops.
#[derive(Debug, Error)]
pub enum AppBuilderError {
    /// An OS-level I/O failure while reading a case file; `path` is the file
    /// that could not be read and `source` is the underlying [`std::io::Error`].
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// A syntactic error in an OpenFOAM dictionary or field file. `file` and
    /// `line` locate the offending token (1-based line number) and `msg`
    /// describes what was expected.
    #[error("parse error in {file} at line {line}: {msg}")]
    Parse {
        file: String,
        line: usize,
        msg: String,
    },
    /// A required dictionary entry was absent: `key` is the missing keyword and
    /// `dict` names the dictionary (e.g. `controlDict`) it was expected in.
    #[error("missing required key '{key}' in {dict}")]
    MissingKey {
        key: &'static str,
        dict: &'static str,
    },
    /// The linear/nonlinear solve failed to converge: `iter` iterations were
    /// taken and `residual` is the (dimensionless) residual reached at bail-out.
    #[error("solver diverged after {iter} iterations (residual {residual:.3e})")]
    Diverged { iter: usize, residual: f64 },
    /// The time loop reached its configured end time `t` (seconds). Returned as
    /// a normal stop signal, not a physics failure.
    #[error("time limit reached: t = {t:.6} s")]
    TimeLimitReached { t: f64 },
    /// An `fvSchemes` selection was parsed and understood, but the solver layer
    /// has no discretisation for it yet. `family` is the dictionary sub-entry
    /// (e.g. `"ddtSchemes"`), `scheme` the requested keyword, and `reason` says
    /// what is missing.
    ///
    /// This is deliberately an error rather than a silent fallback to a default
    /// scheme: a scheme selection that is quietly discarded reads as a promise
    /// the solver does not keep.
    #[error("unsupported {family} selection '{scheme}': {reason}")]
    UnsupportedScheme {
        family: &'static str,
        scheme: String,
        reason: &'static str,
    },
}
