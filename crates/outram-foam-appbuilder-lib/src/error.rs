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

use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppBuilderError {
    #[error("I/O error reading {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("parse error in {file} at line {line}: {msg}")]
    Parse {
        file: String,
        line: usize,
        msg: String,
    },
    #[error("missing required key '{key}' in {dict}")]
    MissingKey {
        key: &'static str,
        dict: &'static str,
    },
    #[error("solver diverged after {iter} iterations (residual {residual:.3e})")]
    Diverged { iter: usize, residual: f64 },
    #[error("time limit reached: t = {t:.6} s")]
    TimeLimitReached { t: f64 },
}
