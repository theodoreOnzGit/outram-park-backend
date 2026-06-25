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

use std::path::Path;
use crate::error::AppBuilderError;

/// Parsed contents of an OpenFOAM `system/controlDict` file.
#[derive(Debug, Clone)]
pub struct ControlDict {
    pub application:    String,
    pub start:          StartControl,
    pub stop:           StopControl,
    pub delta_t:        f64,
    pub write_control:  WriteControl,
    pub write_interval: f64,
    pub purge_write:    usize,
    pub write_format:   WriteFormat,
    pub write_precision: usize,
    pub run_time_modifiable: bool,
    pub adjust_time_step: bool,
    pub max_co:         f64,
    pub max_delta_t:    f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StartControl {
    StartTime(f64),
    LatestTime,
    FirstTime,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StopControl {
    EndTime(f64),
    WriteNow,
    NoWriteNow,
    NextWrite,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WriteControl {
    TimeStep(usize),
    RunTime(f64),
    AdjustableRunTime(f64),
    CpuTime(f64),
    ClockTime(f64),
}

#[derive(Debug, Clone, PartialEq)]
pub enum WriteFormat {
    Ascii,
    Binary,
}

impl ControlDict {
    /// Parse a `controlDict` file from disk.
    pub fn read(path: &Path) -> Result<Self, AppBuilderError> {
        let _ = path;
        todo!("ControlDict::read — tokenise OpenFOAM dictionary format, populate struct")
    }
}

impl Default for ControlDict {
    fn default() -> Self {
        Self {
            application: String::from("foamSolver"),
            start: StartControl::StartTime(0.0),
            stop: StopControl::EndTime(1.0),
            delta_t: 1e-3,
            write_control: WriteControl::TimeStep(1),
            write_interval: 1.0,
            purge_write: 0,
            write_format: WriteFormat::Ascii,
            write_precision: 6,
            run_time_modifiable: true,
            adjust_time_step: false,
            max_co: 0.5,
            max_delta_t: 1.0,
        }
    }
}
