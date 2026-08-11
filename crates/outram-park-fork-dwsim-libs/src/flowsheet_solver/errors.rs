//! Solver-wide error type, abort flag, and execution-mode selector.
//!
//! # What this module is
//!
//! The three cross-cutting concerns every other file in
//! [`crate::flowsheet_solver`] needs: how a failure is reported
//! ([`SolverError`]), how a user stops a running solve ([`AbortFlag`]), and
//! which of DWSIM's execution modes is being requested ([`SolverMode`]).
//!
//! # Attribution
//!
//! Pure-Rust port of parts of **DWSIM** (<https://dwsim.org>), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (branch `windows`), GPL-3.0.
//! Upstream copyright: 2008-2025 Daniel Wagner O. de Medeiros and the DWSIM
//! contributors. This port is GPL-3.0-only. Independent OUTRAM PARK fork, **not**
//! the official DWSIM software (see `TRADEMARKS.md`).
//!
//! Primary sources:
//!
//! - `DWSIM.FlowsheetSolver/FlowsheetSolver.vb:859-873` — `CheckCalculatorStatus`,
//!   the abort check ported as [`AbortFlag::check`].
//! - `DWSIM.FlowsheetSolver/FlowsheetSolver.vb:1117` and `:1203-1208` — the
//!   `mode` parameter documented as `0 = Main Thread, 1 = Background Thread,
//!   2 = Background Parallel Threads, 3 = Azure Service Bus, 4 = Network
//!   Computer`, ported as [`SolverMode`].
//! - `DWSIM.FlowsheetSolver/FlowsheetSolver.vb:496-503` —
//!   `CheckExceptionForAdditionalInfo`, whose two advisory strings are ported as
//!   [`SolverError::detailed_description`] and [`SolverError::user_action`].
//!
//! # Excluded DWSIM behavior
//!
//! - **`CancellationToken` / `CancellationTokenSource` plumbing**
//!   (FlowsheetSolver.vb:263-268, :484-494, :860-872, :1194-1199, :1679-1683).
//!   .NET's cooperative-cancellation machinery, the ambient
//!   `GlobalSettings.Settings.TaskCancellationTokenSource`, and
//!   `ThrowIfCancellationRequested` are replaced by one explicit
//!   [`AbortFlag`] (an `Arc<AtomicBool>`) threaded through the call chain. No
//!   ambient global state.
//! - **`GlobalSettings.Settings.CAPEOPENMode`** (FlowsheetSolver.vb:860). The
//!   CAPE-OPEN COM hosting mode, which suppresses abort checks entirely, has no
//!   analogue here.
//! - **`Exception.Data` dictionaries** (FlowsheetSolver.vb:497-502). .NET
//!   attaches the advisory strings to a mutable per-exception dictionary; this
//!   port exposes them as two constant accessors instead.
//! - **`AggregateException` nesting** (FlowsheetSolver.vb:562-598 and the three
//!   other copies of the same four-deep unwrapping). .NET's task-aggregation
//!   wrapper does not exist here; errors are collected in a flat `Vec`.
//! - **Azure Service Bus (`mode = 3`) and TCP network solving (`mode = 4`)**
//!   (FlowsheetSolver.vb:1628-1665, commented out even upstream). Remote solver
//!   clients are out of scope for this port — see [`SolverMode`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::flowsheet::ObjectType;

/// Anything that can go wrong while ordering, queueing, or solving a flowsheet.
///
/// Dimensionless — an error report, not a physical quantity. Every variant
/// carries enough context to name the offending object by its user-visible tag.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SolverError {
    /// The user requested a stop and [`AbortFlag::check`] observed it.
    ///
    /// Upstream throws `OperationCanceledException` / `"Calculation Aborted"`
    /// (FlowsheetSolver.vb:868, :1592, :1609).
    #[error("calculation aborted by user request")]
    Aborted,

    /// A single object failed to calculate. `tag` is the user-visible label,
    /// `name` the [`crate::flowsheet::ObjectId`] string.
    ///
    /// Upstream formats this identically — `myinfo.Tag & ": " & ex.Message`
    /// (FlowsheetSolver.vb:606, :722).
    #[error("{tag}: {message}")]
    ObjectCalculation {
        /// User-visible tag of the failing object.
        tag: String,
        /// Immutable identity of the failing object.
        name: String,
        /// The underlying failure message.
        message: String,
    },

    /// The dependency walk in [`crate::flowsheet_solver::ordering`] exceeded its
    /// 10 000-step guard, which upstream reads as an unbroken cycle.
    ///
    /// Upstream message verbatim (FlowsheetSolver.vb:985, :1057).
    #[error(
        "Infinite loop detected while obtaining flowsheet object calculation order. \
         Please insert recycle blocks where needed."
    )]
    InfiniteOrderingLoop,

    /// A queue entry, connection, or block referenced an object the flowsheet
    /// does not contain.
    ///
    /// Upstream guards this with `SimulationObjects.ContainsKey` and silently
    /// skips (FlowsheetSolver.vb:531, :1045); this port reports it where
    /// skipping would hide a real inconsistency.
    #[error("object '{0}' is not present in the flowsheet")]
    UnknownObject(String),

    /// The evaluator was asked to calculate an object type it has no model for.
    ///
    /// Has no upstream analogue: DWSIM dispatches to a `Calculate` override that
    /// always exists. Here the equipment models live outside this module, so the
    /// built-in evaluator reports the gap rather than silently doing nothing.
    #[error("no calculation model is available for object type {0:?}")]
    NoModel(ObjectType),

    /// A recycle block hit [`crate::flowsheet_solver::recycle::RecycleBlock::max_iterations`].
    ///
    /// Upstream throws `TimeoutException("RecycleMaxItsReached")`
    /// (Recycle.vb:439-442).
    #[error("recycle '{0}': maximum number of iterations reached")]
    RecycleMaxIterations(String),

    /// A recycle block could not run because its inlet stream had never been
    /// calculated.
    ///
    /// Upstream throws `"RecycleStreamNotCalculated"` (Recycle.vb:396-398,
    /// :419-421).
    #[error("recycle '{0}': the connected stream has not been calculated")]
    RecycleStreamNotCalculated(String),

    /// A recycle block is missing its inlet or outlet connection.
    ///
    /// Upstream throws `"Nohcorrentedematriac7"` / `"Verifiqueasconexesdo"`
    /// (Recycle.vb:297-301) and `"NohcorrentedeEnergyFlow2"`
    /// (EnergyRecycle.vb:202-206).
    #[error("recycle '{0}': check the inlet and outlet connections")]
    RecycleNotConnected(String),

    /// The simultaneous adjust solver ran 25 Newton iterations without
    /// converging.
    ///
    /// Upstream throws `"SADJMaxIterationsReached"` (FlowsheetSolver.vb:2035,
    /// :2144). The cap is hard-coded upstream and is hard-coded here.
    #[error("simultaneous adjust solver: maximum number of iterations (25) reached")]
    AdjustMaxIterations,

    /// The simultaneous adjust solver produced a non-finite sum-of-squares error.
    ///
    /// Upstream throws `"SADJGeneralError"` on `Double.IsNaN(il_err)`
    /// (FlowsheetSolver.vb:2036, :2145).
    #[error("simultaneous adjust solver: the error norm became non-finite")]
    AdjustNonFinite,

    /// An adjust or spec block named a variable that does not exist on its
    /// target object.
    ///
    /// Stands in for upstream's reflection-based `GetPropertyValue` returning
    /// `Nothing` (FlowsheetSolver.vb:2343-2403).
    #[error("object '{object}' has no variable '{variable}'")]
    UnknownVariable {
        /// The object that was addressed.
        object: String,
        /// The variable name that could not be resolved.
        variable: String,
    },

    /// The whole solve exceeded its wall-clock budget \[s\].
    ///
    /// Upstream throws `TimeoutException("SolverTimeout")`
    /// (FlowsheetSolver.vb:1597-1602).
    #[error("solver timed out after {0} s")]
    Timeout(f64),

    /// Anything else, carried verbatim.
    #[error("{0}")]
    Other(String),
}

impl SolverError {
    /// The advisory "detailed description" DWSIM attaches to every
    /// calculation exception (FlowsheetSolver.vb:497-499), verbatim.
    ///
    /// Returns `None` for errors that are not raised during the calculation of a
    /// unit operation or material stream, matching where upstream calls
    /// `CheckExceptionForAdditionalInfo`.
    #[must_use]
    pub fn detailed_description(&self) -> Option<&'static str> {
        match self {
            SolverError::ObjectCalculation { .. } | SolverError::NoModel(_) => Some(
                "This error was raised during the calculation of a Unit Operation \
                 or Material Stream.",
            ),
            _ => None,
        }
    }

    /// The advisory "user action" DWSIM attaches to every calculation exception
    /// (FlowsheetSolver.vb:500-502), verbatim.
    #[must_use]
    pub fn user_action(&self) -> Option<&'static str> {
        match self {
            SolverError::ObjectCalculation { .. } | SolverError::NoModel(_) => Some(
                "Check input parameters. If this error keeps occurring, try another \
                 Property Package and/or Flash Algorithm.",
            ),
            _ => None,
        }
    }

    /// Build an [`SolverError::ObjectCalculation`] the way
    /// `ProcessQueueInternal` does (FlowsheetSolver.vb:603-606).
    #[must_use]
    pub fn for_object(tag: impl Into<String>, name: impl Into<String>, message: impl Into<String>) -> Self {
        SolverError::ObjectCalculation {
            tag: tag.into(),
            name: name.into(),
            message: message.into(),
        }
    }
}

/// A cooperative stop request shared between the caller and a running solve.
///
/// # What it represents
///
/// The single boolean DWSIM keeps in
/// `GlobalSettings.Settings.CalculatorStopRequested` plus the
/// `CancellationTokenSource` it forwards to background tasks
/// (FlowsheetSolver.vb:859-873). Dimensionless.
///
/// # How to use it
///
/// Clone it — the clone shares the same flag — hand one copy to
/// [`crate::flowsheet_solver::solver::FlowsheetSolver`] and keep the other. Call
/// [`AbortFlag::request_abort`] from any thread to stop the solve at the next
/// check point (between queue items, and once per recycle iteration).
///
/// Per the workspace shared-state rule this is an `Arc<AtomicBool>` — not a
/// channel, and not a trait object.
#[derive(Debug, Clone, Default)]
pub struct AbortFlag {
    flag: Arc<AtomicBool>,
}

impl AbortFlag {
    /// A cleared flag.
    #[must_use]
    pub fn new() -> Self {
        AbortFlag::default()
    }

    /// Request that the running solve stop at its next check point.
    ///
    /// Upstream equivalent: setting `Settings.CalculatorStopRequested = True`.
    pub fn request_abort(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    /// Clear a previous request.
    ///
    /// The master solve routine does this on entry
    /// (FlowsheetSolver.vb:1152-1154) and on exit (:1671-1673).
    pub fn clear(&self) {
        self.flag.store(false, Ordering::SeqCst);
    }

    /// Whether a stop has been requested.
    #[must_use]
    pub fn is_aborted(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// `Err(`[`SolverError::Aborted`]`)` if a stop has been requested, `Ok(())`
    /// otherwise.
    ///
    /// This is the port of `CheckCalculatorStatus` (FlowsheetSolver.vb:859-873)
    /// and of every `ct.ThrowIfCancellationRequested()` call site.
    ///
    /// # Errors
    ///
    /// [`SolverError::Aborted`] when the flag is set.
    pub fn check(&self) -> Result<(), SolverError> {
        if self.is_aborted() {
            Err(SolverError::Aborted)
        } else {
            Ok(())
        }
    }
}

/// Which of DWSIM's execution modes a solve is running in.
///
/// # What it represents
///
/// The `mode` integer of `SolveFlowsheet` (FlowsheetSolver.vb:1117, :1203-1208).
/// Dimensionless.
///
/// # This port runs every mode sequentially
///
/// All three ported variants execute the **same sequential algorithm** on the
/// calling thread. The distinction is preserved because it changes observable
/// behaviour upstream and is recorded in a solve report, not because it changes
/// scheduling here:
///
/// | Variant | Upstream `mode` | Upstream behaviour | Here |
/// |---|---|---|---|
/// | [`SolverMode::Synchronous`] | 0 | promoted to 1 immediately (`If mode = 0 Then mode = 1`, :1355) | sequential |
/// | [`SolverMode::Background`] | 1 | one background task, queue drained in order | sequential (identical order) |
/// | [`SolverMode::BackgroundParallel`] | 2 | `Parallel.ForEach` over each *level* of the ordering (:745-853), with a per-object property-package clone (:753-761) | sequential, **level order preserved** |
///
/// Modes 3 (Azure Service Bus) and 4 (TCP network computer) are **not ported**
/// — see the module's "Excluded DWSIM behavior".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SolverMode {
    /// Upstream `mode = 0`. Upstream rewrites it to `1` on entry
    /// (FlowsheetSolver.vb:1355), so it is indistinguishable from
    /// [`SolverMode::Background`] there and here.
    Synchronous,
    /// Upstream `mode = 1` — the default.
    #[default]
    Background,
    /// Upstream `mode = 2` — level-parallel. See the table above for what this
    /// port does and does not reproduce.
    BackgroundParallel,
}

impl SolverMode {
    /// Map DWSIM's integer `mode` onto this enum.
    ///
    /// Returns `None` for `3` (Azure Service Bus) and `4` (network computer),
    /// which are excluded from the port, and for any other value.
    #[must_use]
    pub fn from_upstream_mode(mode: i32) -> Option<SolverMode> {
        match mode {
            0 => Some(SolverMode::Synchronous),
            1 => Some(SolverMode::Background),
            2 => Some(SolverMode::BackgroundParallel),
            _ => None,
        }
    }

    /// The integer DWSIM would use for this mode.
    #[must_use]
    pub fn upstream_mode(self) -> i32 {
        match self {
            SolverMode::Synchronous => 0,
            SolverMode::Background => 1,
            SolverMode::BackgroundParallel => 2,
        }
    }
}

#[cfg(test)]
mod tests {
    //! # Verification — abort flag and mode mapping
    //!
    //! **Methodology.** Check the two behaviours that other modules rely on:
    //! that an [`AbortFlag`] clone shares state with its original (so a stop
    //! request from a second thread is visible to the solve), and that
    //! [`SolverMode`] round-trips DWSIM's integer `mode` for the three ported
    //! values while rejecting the two excluded ones. Pass criterion: exact
    //! equality. Verification only, no physics.
    //! **Results (2026-08-11, release build):** both tests pass; see each test's
    //! doc comment for the observed values.

    use super::*;

    /// **Methodology.** Clone an [`AbortFlag`], set the clone, and read the
    /// original; then clear and re-read.
    /// **Result (2026-08-11):** original reads `true` after the clone is set and
    /// `false` after `clear`; `check()` returns `Err(Aborted)` then `Ok(())`.
    #[test]
    fn abort_flag_is_shared_between_clones() {
        let a = AbortFlag::new();
        let b = a.clone();
        assert!(!a.is_aborted());
        assert_eq!(a.check(), Ok(()));

        b.request_abort();
        assert!(a.is_aborted());
        assert_eq!(a.check(), Err(SolverError::Aborted));

        a.clear();
        assert!(!b.is_aborted());
        assert_eq!(b.check(), Ok(()));
    }

    /// **Methodology.** Map `0..=4` through
    /// [`SolverMode::from_upstream_mode`] and map the three ported values back.
    /// **Result (2026-08-11):** `0 -> Synchronous`, `1 -> Background`,
    /// `2 -> BackgroundParallel`, `3 -> None`, `4 -> None`; round-trip exact.
    #[test]
    fn solver_mode_maps_the_three_ported_upstream_modes() {
        assert_eq!(
            SolverMode::from_upstream_mode(0),
            Some(SolverMode::Synchronous)
        );
        assert_eq!(
            SolverMode::from_upstream_mode(1),
            Some(SolverMode::Background)
        );
        assert_eq!(
            SolverMode::from_upstream_mode(2),
            Some(SolverMode::BackgroundParallel)
        );
        assert_eq!(SolverMode::from_upstream_mode(3), None);
        assert_eq!(SolverMode::from_upstream_mode(4), None);
        for m in [
            SolverMode::Synchronous,
            SolverMode::Background,
            SolverMode::BackgroundParallel,
        ] {
            assert_eq!(SolverMode::from_upstream_mode(m.upstream_mode()), Some(m));
        }
        assert_eq!(SolverMode::default(), SolverMode::Background);
    }

    /// **Methodology.** Check the two advisory strings appear only on the
    /// calculation-failure variants, matching where upstream calls
    /// `CheckExceptionForAdditionalInfo`.
    /// **Result (2026-08-11):** present on `ObjectCalculation` and `NoModel`,
    /// absent on `Aborted`.
    #[test]
    fn advisory_strings_follow_upstream_call_sites() {
        let e = SolverError::for_object("MIX-1", "id-3", "boom");
        assert!(e.detailed_description().is_some());
        assert!(e.user_action().is_some());
        assert_eq!(e.to_string(), "MIX-1: boom");

        assert!(SolverError::Aborted.detailed_description().is_none());
        assert!(SolverError::Aborted.user_action().is_none());
    }
}
