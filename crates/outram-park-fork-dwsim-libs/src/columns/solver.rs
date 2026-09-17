//! The closed set of rigorous-column solvers, dispatched by enum.
//!
//! Ports DWSIM's `MustInherit Class ColumnSolver` abstraction
//! (`RigorousColumn.vb` lines 818-832) — but as an **enum**, not a base class
//! or a trait object, per the workspace design rules ("No trait objects — use
//! enums for dispatch"). Upstream selects a solver by assigning a
//! `ColumnSolver` subclass instance to `Column.Solver`
//! (`RigorousColumn.vb:1959`, `SetColumnSolver` at `:2748`); here the selection
//! is a [`ColumnSolverMethod`] value and dispatch is a `match`.
//!
//! Ported from DWSIM (GPL-3.0), upstream commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream copyright: 2008-2022
//! Daniel Wagner O. de Medeiros et al.
//!
//! # Choosing a method
//!
//! | Method | Best for | Cost per iteration |
//! |---|---|---|
//! | [`ColumnSolverMethod::WangHenke`] | Narrow-boiling distillation (the common case) | `N` bubble points + `2N` enthalpy calls + `C` tridiagonal solves |
//! | [`ColumnSolverMethod::ModifiedWangHenke`] | The same, with a much tighter convergence gate | as above |
//! | [`ColumnSolverMethod::SumRates`] | Wide-boiling absorbers and strippers | `N` K-value calls + `6N` enthalpy calls + `C + 1` tridiagonal solves |
//! | [`ColumnSolverMethod::NaphtaliSandholm`] | Hard / strongly non-ideal columns, and any column where the specifications are awkward | one `N(2C+1)`-square Jacobian, i.e. `2N(2C+1)` residual evaluations |
//!
//! `N` = stages, `C` = components.
//!
//! # Excluded DWSIM behavior
//!
//! - `ColumnSolver.SolveColumn(col, input)`, the overload that takes the
//!   flowsheet `Column` object (`RigorousColumn.vb:822-830`) — it throws
//!   `NotImplementedException` upstream anyway.
//! - The `IExternalColumnSolver` .NET plug-in hook
//!   (`RigorousColumnSolvers/ExternalColumnSolver.vb`, the entire 14-line
//!   file). It exists so a third-party assembly can register a solver at
//!   runtime, which is exactly the `dyn`-dispatch pattern this workspace
//!   forbids. **Deliberately not ported**: the solver set here is closed, and
//!   adding a method means adding an enum variant, which the compiler then
//!   forces every `match` site to handle.

use crate::columns::bubble_point::WangHenkeSolver;
use crate::columns::bubble_point2::ModifiedWangHenkeSolver;
use crate::columns::model::{ColumnError, ColumnSolverInput, ColumnSolverOutput};
use crate::columns::newton_raphson::NaphtaliSandholmSolver;
use crate::columns::sum_rates::SumRatesSolver;

/// The rigorous-column solver set — enum dispatch, no `dyn`.
///
/// Each variant carries its solver's own configuration struct, so
/// method-specific options (sub-cooling, warm start, relaxation) travel with
/// the choice of method instead of being smeared across a shared options bag.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColumnSolverMethod {
    /// Wang-Henke bubble-point (BP) — [`crate::columns::bubble_point`].
    WangHenke(WangHenkeSolver),
    /// Modified Wang-Henke bubble-point (MBP) — [`crate::columns::bubble_point2`].
    ModifiedWangHenke(ModifiedWangHenkeSolver),
    /// Burningham-Otto sum-rates (SR) — [`crate::columns::sum_rates`].
    SumRates(SumRatesSolver),
    /// Naphtali-Sandholm simultaneous correction (SC) —
    /// [`crate::columns::newton_raphson`].
    NaphtaliSandholm(NaphtaliSandholmSolver),
}

impl Default for ColumnSolverMethod {
    /// Wang-Henke, upstream's default
    /// (`SolvingMethodName = "Wang-Henke (Bubble Point)"`,
    /// `RigorousColumn.vb:1913`).
    fn default() -> Self {
        Self::WangHenke(WangHenkeSolver::default())
    }
}

impl ColumnSolverMethod {
    /// The method's display name — upstream's `ColumnSolver.Name` property.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::WangHenke(_) => WangHenkeSolver::name(),
            Self::ModifiedWangHenke(_) => ModifiedWangHenkeSolver::name(),
            Self::SumRates(_) => SumRatesSolver::name(),
            Self::NaphtaliSandholm(_) => NaphtaliSandholmSolver::name(),
        }
    }

    /// The method's description — upstream's `ColumnSolver.Description`
    /// property.
    #[must_use]
    pub fn description(&self) -> &'static str {
        match self {
            Self::WangHenke(_) => WangHenkeSolver::description(),
            Self::ModifiedWangHenke(_) => ModifiedWangHenkeSolver::description(),
            Self::SumRates(_) => SumRatesSolver::description(),
            Self::NaphtaliSandholm(_) => NaphtaliSandholmSolver::description(),
        }
    }

    /// Solve the column — upstream's
    /// `ColumnSolver.SolveColumn(input As ColumnSolverInputData)`
    /// (`RigorousColumn.vb:820`).
    ///
    /// # Errors
    ///
    /// Any [`ColumnError`] the chosen method raises: input-shape validation,
    /// non-convergence, an unphysical profile, a failed stage bubble point, a
    /// singular tridiagonal system, or a trivial-solution collapse.
    pub fn solve(&self, input: &ColumnSolverInput) -> Result<ColumnSolverOutput, ColumnError> {
        match self {
            Self::WangHenke(s) => s.solve_column(input),
            Self::ModifiedWangHenke(s) => s.solve_column(input),
            Self::SumRates(s) => s.solve_column(input),
            Self::NaphtaliSandholm(s) => s.solve_column(input),
        }
    }
}
