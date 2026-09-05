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

//! # `case_runner` — driving a solver from an OpenFOAM case directory
//!
//! [`solvers`](crate::solvers) own their time loops but know nothing about
//! case *files*: each is constructed from a mesh and three dictionaries, and
//! the caller is left to read `constant/polyMesh`, lift `deltaT`/`endTime`
//! out of `system/controlDict`, find the `0/` fields, march the loop and
//! write the results back. That glue used to live, four times over, inside
//! `outram-foam-cli`'s solver binaries — which could not share it, because a
//! `src/bin/*.rs` has no module to share it *through*.
//!
//! This module is that glue, once. [`CaseRun`] is the whole of it:
//!
//! ```no_run
//! use outram_foam_appbuilder_lib::case_runner::{CaseRun, SolverKind};
//!
//! let mut run = CaseRun::from_case("cavity", SolverKind::PimpleFoam)?;
//! while !run.is_done() {
//!     let t = run.step()?;            // one time step; returns the new time
//!     println!("Time = {t}");         // ... or inspect the fields here
//! }
//! let summary = run.write_fields()?;
//! # Ok::<(), outram_foam_appbuilder_lib::error::AppBuilderError>(())
//! ```
//!
//! [`CaseRun::run_to_end`] does the same loop in one call when nothing needs
//! to happen between steps.
//!
//! ## Dispatch is an enum, not a trait object
//!
//! [`SolverState`] is an enum over the concrete solver types rather than a
//! `Box<dyn Solver>`, per the workspace rule against trait objects: adding a
//! solver is then a compile error at every match site instead of a silent
//! gap. It also keeps the solver's own fields reachable — `run.solver` hands
//! back the real [`PimpleFoam`](crate::solvers::pimple_foam::PimpleFoam), so
//! a caller can read `u`/`p` between steps.
//!
//! ## Which solvers are wired
//!
//! Only the two whose state can be built from what the case reader actually
//! parses. `rhoPimpleFoam` and `sonicFoam` need the compressibility
//! `psi = rho/p` from `constant/thermophysicalProperties`, which no reader
//! covers yet; they report [`AppBuilderError::SolverNotCaseWired`] rather
//! than running with a default equation of state. See
//! [`SolverKind::is_case_wired`].

use std::path::{Path, PathBuf};

use outram_foam_basic_lib::io::case::{CaseField, FoamCase};
use outram_foam_basic_lib::io::field::{write_vol_scalar_field, write_vol_vector_field};
use outram_foam_basic_lib::prelude::{VolScalarField, VolVectorField};

use crate::error::AppBuilderError;
use crate::io::control_dict::{ControlDict, StartControl, StopControl};
use crate::io::fv_schemes::FvSchemes;
use crate::io::fv_solution::FvSolution;
use crate::solvers::pimple_foam::PimpleFoam;
use crate::solvers::rho_central_foam::RhoCentralFoam;

/// Dimension exponents `[kg, m, s, K, kmol, A, cd]`, the order OpenFOAM
/// writes in a field file's `dimensions` entry.
const U_DIMS: [f64; 7] = [0.0, 1.0, -1.0, 0.0, 0.0, 0.0, 0.0]; // m/s
const P_KIN_DIMS: [f64; 7] = [0.0, 2.0, -2.0, 0.0, 0.0, 0.0, 0.0]; // m²/s² (kinematic p)
const P_DIMS: [f64; 7] = [1.0, -1.0, -2.0, 0.0, 0.0, 0.0, 0.0]; // Pa
const RHO_DIMS: [f64; 7] = [1.0, -3.0, 0.0, 0.0, 0.0, 0.0, 0.0]; // kg/m³
const E_DIMS: [f64; 7] = [0.0, 2.0, -2.0, 0.0, 0.0, 0.0, 0.0]; // J/kg = m²/s²

/// Which ported solver application to run a case with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SolverKind {
    /// `pimpleFoam` — incompressible transient PIMPLE.
    PimpleFoam,
    /// `rhoCentralFoam` — density-based central-upwind (Kurganov-Tadmor).
    RhoCentralFoam,
    /// `rhoPimpleFoam` — compressible transient PIMPLE. Not case-wired.
    RhoPimpleFoam,
    /// `sonicFoam` — transonic/supersonic compressible. Not case-wired.
    SonicFoam,
}

impl SolverKind {
    /// Every solver this module knows about, wired or not.
    pub const ALL: [SolverKind; 4] = [
        SolverKind::PimpleFoam,
        SolverKind::RhoCentralFoam,
        SolverKind::RhoPimpleFoam,
        SolverKind::SonicFoam,
    ];

    /// The OpenFOAM application name, as it appears on a command line.
    pub fn name(self) -> &'static str {
        match self {
            SolverKind::PimpleFoam => "pimpleFoam",
            SolverKind::RhoCentralFoam => "rhoCentralFoam",
            SolverKind::RhoPimpleFoam => "rhoPimpleFoam",
            SolverKind::SonicFoam => "sonicFoam",
        }
    }

    /// Parse an application name. Accepts the OpenFOAM spelling
    /// (`"pimpleFoam"`) and the snake_case binary name (`"pimple_foam"`).
    pub fn from_name(name: &str) -> Option<SolverKind> {
        let flat = name.replace('_', "").to_ascii_lowercase();
        SolverKind::ALL
            .into_iter()
            .find(|k| k.name().to_ascii_lowercase() == flat)
    }

    /// Whether [`CaseRun::from_case`] can build this solver's initial state
    /// from a case directory.
    ///
    /// `false` means the case reader does not yet parse something the solver
    /// needs — not that the solver itself is unfinished. See the module docs.
    pub fn is_case_wired(self) -> bool {
        matches!(self, SolverKind::PimpleFoam | SolverKind::RhoCentralFoam)
    }

    fn not_wired_reason(self) -> &'static str {
        match self {
            SolverKind::RhoPimpleFoam | SolverKind::SonicFoam => {
                "the solver needs the compressibility psi = rho/p (and mu) from \
                 constant/thermophysicalProperties, which the case reader does not \
                 parse yet (it reads only vol fields and system/ dicts). Running with \
                 a default psi would impose the wrong equation of state, so no run is \
                 performed."
            }
            _ => "",
        }
    }
}

/// A constructed solver, dispatched by value rather than through a trait
/// object so that each variant's own fields stay reachable.
#[derive(Debug)]
pub enum SolverState {
    /// See [`PimpleFoam`].
    PimpleFoam(PimpleFoam),
    /// See [`RhoCentralFoam`].
    RhoCentralFoam(RhoCentralFoam),
}

impl SolverState {
    /// Which [`SolverKind`] this state belongs to.
    pub fn kind(&self) -> SolverKind {
        match self {
            SolverState::PimpleFoam(_) => SolverKind::PimpleFoam,
            SolverState::RhoCentralFoam(_) => SolverKind::RhoCentralFoam,
        }
    }

    /// Number of mesh cells the solver is running on.
    pub fn n_cells(&self) -> usize {
        match self {
            SolverState::PimpleFoam(s) => s.mesh.n_cells,
            SolverState::RhoCentralFoam(s) => s.mesh.n_cells,
        }
    }

    /// Advance the underlying solver by one time step.
    pub fn step(&mut self) -> Result<(), AppBuilderError> {
        match self {
            SolverState::PimpleFoam(s) => s.step(),
            SolverState::RhoCentralFoam(s) => s.step(),
        }
    }
}

/// What a completed (or partially completed) run did.
#[derive(Debug, Clone, PartialEq)]
pub struct CaseSummary {
    /// The solver that was run.
    pub solver: SolverKind,
    /// Cell count of the case mesh.
    pub n_cells: usize,
    /// Time the run started from, per `system/controlDict`.
    pub start_time: f64,
    /// Time actually reached.
    pub end_time: f64,
    /// Time-step size used.
    pub delta_t: f64,
    /// Number of steps marched.
    pub n_steps: usize,
    /// Directory the result fields were written to, once
    /// [`CaseRun::write_fields`] has run.
    pub output_dir: PathBuf,
}

/// A case, loaded and marched one step at a time.
///
/// Construct with [`CaseRun::from_case`], then either drive the loop yourself
/// (see the module example) or call [`CaseRun::run_to_end`].
#[derive(Debug)]
pub struct CaseRun {
    /// The configured solver. Public so a caller can read or perturb its
    /// fields between steps — `run.solver` is the real solver, not a handle.
    pub solver: SolverState,
    /// The case directory this was read from.
    pub case_dir: PathBuf,
    /// Current simulation time.
    pub time: f64,
    /// Time the run started from.
    pub start_time: f64,
    /// Time the run stops at.
    pub end_time: f64,
    /// Time-step size.
    pub delta_t: f64,
    /// Steps marched so far.
    pub n_steps: usize,
}

impl CaseRun {
    /// Read a case directory and build the solver's initial state from it.
    ///
    /// Reads `constant/polyMesh`, takes `deltaT` / `startTime` / `endTime`
    /// from `system/controlDict`, and loads the `0/` fields the chosen solver
    /// requires. Fields the solver treats as optional are left at their
    /// constructed defaults when absent.
    pub fn from_case(
        case_dir: impl AsRef<Path>,
        kind: SolverKind,
    ) -> Result<CaseRun, AppBuilderError> {
        let case_dir = case_dir.as_ref().to_path_buf();
        if !kind.is_case_wired() {
            return Err(AppBuilderError::SolverNotCaseWired {
                solver: kind.name(),
                reason: kind.not_wired_reason(),
            });
        }
        if !case_dir.is_dir() {
            // `FoamCase::read` tolerates a missing directory and returns an
            // empty case, which would surface below as "no constant/polyMesh"
            // -- true, but it buries the actual mistake (usually a typo in the
            // path) under a mesh error.
            return Err(AppBuilderError::Io {
                path: case_dir.clone(),
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "case directory does not exist",
                ),
            });
        }
        let case = FoamCase::read(&case_dir).map_err(|e| AppBuilderError::Io {
            path: case_dir.clone(),
            source: std::io::Error::other(e.to_string()),
        })?;
        let mesh = case.mesh.clone().ok_or_else(|| AppBuilderError::Case {
            case: case_dir.clone(),
            msg: "case has no constant/polyMesh".to_string(),
        })?;

        let control = control_from_case(&case);
        let (start_time, end_time, delta_t) = time_span(&control);
        let missing = |what: &str| AppBuilderError::Case {
            case: case_dir.clone(),
            msg: format!("missing 0/{what}"),
        };

        let solver = match kind {
            SolverKind::PimpleFoam => {
                let mut s =
                    PimpleFoam::new(mesh, control, FvSchemes::default(), FvSolution::default());
                s.u = find_vector(&case, "U").ok_or_else(|| missing("U (velocity)"))?;
                s.p = find_scalar(&case, "p").ok_or_else(|| missing("p (kinematic pressure)"))?;
                // `nu` is optional: constant/transportProperties is not parsed,
                // so the solver's constructed default stands when 0/nu is absent.
                if let Some(nu) = find_scalar(&case, "nu") {
                    s.nu = nu;
                }
                SolverState::PimpleFoam(s)
            }
            SolverKind::RhoCentralFoam => {
                let mut s =
                    RhoCentralFoam::new(mesh, control, FvSchemes::default(), FvSolution::default());
                s.u = find_vector(&case, "U").ok_or_else(|| missing("U (velocity)"))?;
                s.rho = find_scalar(&case, "rho").ok_or_else(|| missing("rho (density)"))?;
                // This port evolves (rho, e); there is no thermophysical reader
                // to derive e from a temperature T, so 0/e is required.
                s.e = find_scalar(&case, "e")
                    .ok_or_else(|| missing("e (specific internal energy)"))?;
                if let Some(p) = find_scalar(&case, "p") {
                    s.p = p;
                }
                SolverState::RhoCentralFoam(s)
            }
            // `is_case_wired` gates the unwired kinds above.
            SolverKind::RhoPimpleFoam | SolverKind::SonicFoam => unreachable!(),
        };

        Ok(CaseRun {
            solver,
            case_dir,
            time: start_time,
            start_time,
            end_time,
            delta_t,
            n_steps: 0,
        })
    }

    /// Whether the run has reached its end time.
    ///
    /// The comparison carries a relative tolerance of `1e-9 * deltaT`, so a
    /// final step whose accumulated time lands a rounding error short of
    /// `endTime` does not trigger one extra step.
    pub fn is_done(&self) -> bool {
        self.time >= self.end_time - self.delta_t * 1e-9
    }

    /// Advance one time step, returning the new simulation time.
    pub fn step(&mut self) -> Result<f64, AppBuilderError> {
        self.solver.step()?;
        self.time += self.delta_t;
        self.n_steps += 1;
        Ok(self.time)
    }

    /// March to the end time and write the result fields.
    ///
    /// Equivalent to stepping until [`is_done`](CaseRun::is_done) and then
    /// calling [`write_fields`](CaseRun::write_fields).
    pub fn run_to_end(&mut self) -> Result<CaseSummary, AppBuilderError> {
        while !self.is_done() {
            self.step()?;
        }
        self.write_fields()
    }

    /// Write the solver's fields into `<case>/<time>/` and describe the run.
    ///
    /// Which fields are written depends on the solver: `p` and `U` for
    /// `pimpleFoam`; `p`, `rho`, `e` and `U` for `rhoCentralFoam`.
    pub fn write_fields(&self) -> Result<CaseSummary, AppBuilderError> {
        let out_dir = self.case_dir.join(format_time(self.time));
        std::fs::create_dir_all(&out_dir).map_err(|e| AppBuilderError::Io {
            path: out_dir.clone(),
            source: e,
        })?;
        match &self.solver {
            SolverState::PimpleFoam(s) => {
                write_scalar(&out_dir, "p", &s.p, P_KIN_DIMS)?;
                write_vector(&out_dir, "U", &s.u, U_DIMS)?;
            }
            SolverState::RhoCentralFoam(s) => {
                write_scalar(&out_dir, "p", &s.p, P_DIMS)?;
                write_scalar(&out_dir, "rho", &s.rho, RHO_DIMS)?;
                write_scalar(&out_dir, "e", &s.e, E_DIMS)?;
                write_vector(&out_dir, "U", &s.u, U_DIMS)?;
            }
        }
        Ok(self.summary(out_dir))
    }

    /// Describe the run so far without writing anything.
    pub fn summary(&self, output_dir: PathBuf) -> CaseSummary {
        CaseSummary {
            solver: self.solver.kind(),
            n_cells: self.solver.n_cells(),
            start_time: self.start_time,
            end_time: self.time,
            delta_t: self.delta_t,
            n_steps: self.n_steps,
            output_dir,
        }
    }
}

/// Read a case and run it to completion in one call.
pub fn run_case(
    case_dir: impl AsRef<Path>,
    kind: SolverKind,
) -> Result<CaseSummary, AppBuilderError> {
    CaseRun::from_case(case_dir, kind)?.run_to_end()
}

/// Build a [`ControlDict`] from the case's `system/controlDict`.
///
/// Lifts the three keys the time loop needs — `deltaT`, `startTime`,
/// `endTime` — and leaves every other control setting at its default. The
/// typed [`ControlDict::read`] is not implemented yet, so these come through
/// `outram_foam_basic_lib`'s dictionary parser.
pub fn control_from_case(case: &FoamCase) -> ControlDict {
    let mut ctrl = ControlDict::default();
    if let Some(cd) = case.system_dict("controlDict") {
        let d = &cd.dict;
        if let Some(dt) = d.get_scalar("deltaT") {
            ctrl.delta_t = dt;
        }
        if let Some(t0) = d.get_scalar("startTime") {
            ctrl.start = StartControl::StartTime(t0);
        }
        if let Some(te) = d.get_scalar("endTime") {
            ctrl.stop = StopControl::EndTime(te);
        }
    }
    ctrl
}

/// The `(start, end, deltaT)` triple a time loop needs.
///
/// Control modes other than a plain start/end time fall back to `start`, so
/// the loop runs zero steps rather than an unbounded number.
pub fn time_span(ctrl: &ControlDict) -> (f64, f64, f64) {
    let start = match ctrl.start {
        StartControl::StartTime(t) => t,
        _ => 0.0,
    };
    let end = match ctrl.stop {
        StopControl::EndTime(t) => t,
        _ => start,
    };
    (start, end, ctrl.delta_t)
}

/// Find a named scalar field among the case's time-directory fields.
pub fn find_scalar(case: &FoamCase, name: &str) -> Option<VolScalarField> {
    case.fields.iter().find_map(|f| match f {
        CaseField::Scalar(sf, _) if sf.name == name => Some(sf.clone()),
        _ => None,
    })
}

/// Find a named vector field among the case's time-directory fields.
pub fn find_vector(case: &FoamCase, name: &str) -> Option<VolVectorField> {
    case.fields.iter().find_map(|f| match f {
        CaseField::Vector(vf, _) if vf.name == name => Some(vf.clone()),
        _ => None,
    })
}

/// Format a time value the way OpenFOAM names a time directory: `0.1`, not
/// `0.100000000`, and `1` rather than `1.0`.
pub fn format_time(t: f64) -> String {
    if t.is_finite() && t.fract().abs() < 1e-12 && t.abs() < 1e15 {
        return format!("{}", t.round() as i64);
    }
    let s = format!("{t:.9}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

fn write_scalar(
    dir: &Path,
    name: &str,
    field: &VolScalarField,
    dims: [f64; 7],
) -> Result<(), AppBuilderError> {
    let path = dir.join(name);
    write_vol_scalar_field(&path, field, dims).map_err(|e| AppBuilderError::Io {
        path,
        source: std::io::Error::other(e.to_string()),
    })
}

fn write_vector(
    dir: &Path,
    name: &str,
    field: &VolVectorField,
    dims: [f64; 7],
) -> Result<(), AppBuilderError> {
    let path = dir.join(name);
    write_vol_vector_field(&path, field, dims).map_err(|e| AppBuilderError::Io {
        path,
        source: std::io::Error::other(e.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn solver_names_round_trip() {
        for kind in SolverKind::ALL {
            assert_eq!(SolverKind::from_name(kind.name()), Some(kind));
        }
    }

    /// The binaries are named in snake_case (`pimple_foam`) while OpenFOAM
    /// spells the application `pimpleFoam`; both must resolve, so a caller can
    /// pass whichever it has.
    #[test]
    fn solver_names_accept_snake_case_and_are_case_insensitive() {
        assert_eq!(
            SolverKind::from_name("pimple_foam"),
            Some(SolverKind::PimpleFoam)
        );
        assert_eq!(
            SolverKind::from_name("RHOCENTRALFOAM"),
            Some(SolverKind::RhoCentralFoam)
        );
        assert_eq!(SolverKind::from_name("interFoam"), None);
    }

    /// Exactly the two solvers whose state the case reader can build. If a
    /// thermophysicalProperties reader lands, this test is the reminder to
    /// widen `is_case_wired` and wire the other two in `CaseRun::from_case`.
    #[test]
    fn only_the_incompressible_and_density_based_solvers_are_case_wired() {
        let wired: Vec<_> = SolverKind::ALL
            .into_iter()
            .filter(|k| k.is_case_wired())
            .collect();
        assert_eq!(
            wired,
            vec![SolverKind::PimpleFoam, SolverKind::RhoCentralFoam]
        );
    }

    /// An unwired solver must say so rather than run with invented physical
    /// properties -- and must say so before touching the filesystem, so the
    /// error does not depend on whether the case directory exists.
    #[test]
    fn unwired_solvers_refuse_before_reading_the_case() {
        for kind in [SolverKind::RhoPimpleFoam, SolverKind::SonicFoam] {
            let err = CaseRun::from_case("/nonexistent/case", kind)
                .expect_err("an unwired solver must not report success");
            match err {
                AppBuilderError::SolverNotCaseWired { solver, reason } => {
                    assert_eq!(solver, kind.name());
                    assert!(
                        reason.contains("thermophysicalProperties"),
                        "the reason should name what is missing, got: {reason}"
                    );
                }
                other => panic!("expected SolverNotCaseWired, got {other:?}"),
            }
        }
    }

    #[test]
    fn missing_case_directory_is_an_io_error() {
        let err = CaseRun::from_case("/nonexistent/case", SolverKind::PimpleFoam)
            .expect_err("a missing case directory must not report success");
        assert!(
            matches!(err, AppBuilderError::Io { .. }),
            "expected Io, got {err:?}"
        );
    }

    /// Time directories are named the way OpenFOAM names them: `0.1`, not
    /// `0.100000000`, and `1` rather than `1.0`. A mismatch here writes
    /// results into a directory the case will not find again.
    #[test]
    fn time_formatting_matches_openfoam_directory_names() {
        assert_eq!(format_time(0.0), "0");
        assert_eq!(format_time(1.0), "1");
        assert_eq!(format_time(0.1), "0.1");
        assert_eq!(format_time(0.005), "0.005");
        assert_eq!(format_time(1.5), "1.5");
    }

    /// `ControlDict::default()` describes a real, runnable span (0 -> 1 s at
    /// 1 ms), so a case whose `system/controlDict` omits the time keys still
    /// runs rather than silently doing nothing.
    #[test]
    fn time_span_of_a_default_control_is_runnable() {
        assert_eq!(time_span(&ControlDict::default()), (0.0, 1.0, 1e-3));
    }

    /// The stop controls that are not `endTime` mean "stop now", and must
    /// collapse to a zero-length span -- the loop then runs no steps instead
    /// of an unbounded number.
    #[test]
    fn non_end_time_stop_controls_run_no_steps() {
        for stop in [
            StopControl::WriteNow,
            StopControl::NoWriteNow,
            StopControl::NextWrite,
        ] {
            let label = format!("{stop:?}");
            let mut ctrl = ControlDict::default();
            ctrl.start = StartControl::StartTime(0.25);
            ctrl.stop = stop;
            let (start, end, _) = time_span(&ctrl);
            assert_eq!((start, end), (0.25, 0.25), "{label} must not run steps");
        }
    }

    #[test]
    fn time_span_lifts_start_and_end() {
        let mut ctrl = ControlDict::default();
        ctrl.delta_t = 0.005;
        ctrl.start = StartControl::StartTime(0.1);
        ctrl.stop = StopControl::EndTime(0.5);
        assert_eq!(time_span(&ctrl), (0.1, 0.5, 0.005));
    }
}
