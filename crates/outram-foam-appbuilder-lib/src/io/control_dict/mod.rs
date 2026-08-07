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

//! Typed equivalent of OpenFOAM's `system/controlDict` — the time-loop and
//! output control dictionary. [`ControlDict`] replaces the free-form text
//! dictionary with a struct whose start/stop/write controls are enums, so an
//! invalid selection cannot silently fall back to a default.
//!
//! **Status: the struct exists, the on-disk parser does not.**
//! [`ControlDict::read`] is `todo!()`. Build a case with
//! `ControlDict::default()` and field assignment.
//!
//! **Only three fields currently drive anything.** The solver loops in
//! [`crate::solvers`] read [`ControlDict::start`], [`ControlDict::stop`] and
//! [`ControlDict::delta_t`]. The write-control, `adjustTimeStep` and
//! `runTimeModifiable` fields are carried faithfully for a future output layer
//! but are **not consulted by any solver today** — each is flagged below.

use crate::error::AppBuilderError;
use std::path::Path;

/// The contents of an OpenFOAM `system/controlDict`, as a typed struct.
///
/// Construct with [`ControlDict::default`] and assign the fields you need;
/// [`ControlDict::read`] (parsing the file from disk) is not implemented.
///
/// See the module documentation for which fields the solvers actually honour.
#[derive(Debug, Clone)]
pub struct ControlDict {
    /// Name of the OpenFOAM application the case was written for (e.g.
    /// `"pimpleFoam"`). Informational only — this crate never dispatches on it.
    pub application: String,
    /// Where the run starts. See [`StartControl`].
    pub start: StartControl,
    /// Where the run stops. See [`StopControl`].
    pub stop: StopControl,
    /// Fixed time step Δt in seconds. Must be > 0. Because `adjust_time_step` is not
    /// implemented (below), this is the step size for the whole run.
    pub delta_t: f64,
    /// How often results should be written. See [`WriteControl`].
    ///
    /// **Not honoured** — [`crate::io::output`] has no working writers, so no
    /// solver writes anything to disk regardless of this value.
    pub write_control: WriteControl,
    /// Interval used by `write_control` — steps for
    /// [`WriteControl::TimeStep`], seconds otherwise. **Not honoured** (see
    /// `write_control`).
    pub write_interval: f64,
    /// Number of old time directories to retain (`0` = keep all).
    /// **Not honoured** (see `write_control`).
    pub purge_write: usize,
    /// ASCII or binary field output. **Not honoured** (see `write_control`).
    pub write_format: WriteFormat,
    /// Significant figures for ASCII output. **Not honoured** (see
    /// `write_control`).
    pub write_precision: usize,
    /// Whether OpenFOAM would re-read the dictionary each step.
    /// **Not honoured** — this crate holds the struct in memory and never
    /// re-reads it.
    pub run_time_modifiable: bool,
    /// Whether the time step should adapt to `max_co` / `max_delta_t`.
    ///
    /// **Not implemented.** There is no adaptive-Δt path in this crate: the
    /// solver loops step at the fixed [`ControlDict::delta_t`] whatever this is
    /// set to. Setting it `true` changes nothing — pick a Δt that satisfies
    /// your Courant limit yourself.
    pub adjust_time_step: bool,
    /// Target maximum Courant number for adaptive stepping [-].
    /// **Not honoured** (see `adjust_time_step`).
    pub max_co: f64,
    /// Ceiling on the adaptive time step, in seconds.
    /// **Not honoured** (see `adjust_time_step`).
    pub max_delta_t: f64,
}

/// Where a run begins — OpenFOAM's `startFrom`.
///
/// Only [`StartControl::StartTime`] is acted on; the loops in
/// [`crate::solvers`] treat the other two as `t = 0`, because selecting a time
/// directory on disk needs field *reading per time step*, which this crate does
/// not do.
#[derive(Debug, Clone, PartialEq)]
pub enum StartControl {
    /// Begin at this time, in seconds (`startFrom startTime`).
    StartTime(f64),
    /// Begin at the newest time directory present (`startFrom latestTime`).
    /// **Treated as `t = 0`.**
    LatestTime,
    /// Begin at the earliest time directory present (`startFrom firstTime`).
    /// **Treated as `t = 0`.**
    FirstTime,
}

/// Where a run ends — OpenFOAM's `stopAt`.
///
/// Only [`StopControl::EndTime`] is acted on. The solver `run()` loops return
/// immediately (`Ok(())`, zero steps taken) for every other variant, because
/// each of them is defined in terms of a write that this crate cannot perform.
#[derive(Debug, Clone, PartialEq)]
pub enum StopControl {
    /// Stop once the simulated time reaches this value, in seconds (`stopAt endTime`).
    EndTime(f64),
    /// Stop and write immediately. **Runs zero steps** (see the enum docs).
    WriteNow,
    /// Stop immediately without writing. **Runs zero steps.**
    NoWriteNow,
    /// Stop at the next scheduled write. **Runs zero steps.**
    NextWrite,
}

/// How often results are written — OpenFOAM's `writeControl`.
///
/// **No variant is honoured**: [`crate::io::output`]'s writers are `todo!()`,
/// so a solver run produces no files. The enum exists so a case description is
/// complete and so the output layer, once written, has its selection already
/// typed.
#[derive(Debug, Clone, PartialEq)]
pub enum WriteControl {
    /// Write every N time steps.
    TimeStep(usize),
    /// Write every N seconds of *simulated* time.
    RunTime(f64),
    /// Write every N seconds of simulated time, adjusting Δt to land exactly on
    /// the write instants.
    AdjustableRunTime(f64),
    /// Write every N seconds of CPU time.
    CpuTime(f64),
    /// Write every N seconds of wall-clock time.
    ClockTime(f64),
}

/// Field-file encoding — OpenFOAM's `writeFormat`. **Not honoured**; see
/// [`WriteControl`].
#[derive(Debug, Clone, PartialEq)]
pub enum WriteFormat {
    /// Human-readable ASCII.
    Ascii,
    /// Binary (smaller and faster, not diffable).
    Binary,
}

impl ControlDict {
    /// Parse a `controlDict` file from disk.
    ///
    /// **Not yet implemented — calling this panics (`todo!`).** OpenFOAM
    /// dictionary tokenising is not wired up for any of the `system/` files;
    /// see [`crate::io::fv_schemes::FvSchemes::read`] and
    /// [`crate::io::fv_solution::FvSolution::read`], which are in the same
    /// state.
    ///
    /// Until it exists, build the struct in Rust:
    ///
    /// ```
    /// use outram_foam_appbuilder_lib::io::control_dict::{ControlDict, StopControl};
    ///
    /// let mut control = ControlDict::default();
    /// control.delta_t = 1.0e-3;                     // Δt [s]
    /// control.stop = StopControl::EndTime(0.5);     // run to t = 0.5 s
    /// ```
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
