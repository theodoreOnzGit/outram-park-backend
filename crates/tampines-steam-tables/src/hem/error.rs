//! The one error type of the HEM pipe solver, [`HemError`].
//!
//! Every failure is reported, never clamped: a negative density, a negative
//! pressure, or a state the equation of state cannot flash stops the step and
//! leaves the solver at the last good state. `tampines` maps this type into
//! its own `TampinesError` (variant `Hem`) so `?` works across the crates.

/// Why a HEM pipe operation failed.
///
/// Values are raw SI (`f64`) so a log line shows exactly what broke without
/// unit conversion: pressures in Pa, densities in kg/m³, energies in J/kg.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum HemError {
    /// A caller-supplied input is unusable before any physics runs: a zero
    /// cell count, a non-positive length, a per-cell initial condition of the
    /// wrong length, a non-positive time step.
    #[error("HEM pipe invalid input: {0}")]
    InvalidInput(String),

    /// A fixed time step would exceed the stability limit of the explicit
    /// scheme. Returned *before* the step runs, so the solver state is
    /// unchanged.
    ///
    /// `courant` and `limit` are upstream `rhoCentralFoam`'s central Courant
    /// number (`centralCourantNo.H`, half the classical `c·Δt/Δx` in 1-D for a
    /// fluid at rest); see [`super::TimeStepControl::Fixed`].
    #[error("HEM pipe Courant number {courant:.4} exceeds the fixed-step limit {limit:.4}")]
    CourantViolation {
        /// Courant number the requested step would have run at.
        courant: f64,
        /// The limit it exceeded.
        limit: f64,
    },

    /// A conserved or derived quantity left the range physics allows during a
    /// solve (negative or non-finite density, pressure or internal energy).
    /// The solver state is rolled back to the start of the failed step.
    #[error("HEM pipe unphysical {quantity} = {value} in cell {cell}")]
    Unphysical {
        /// Index of the offending cell (0 at the inlet, `x = 0`).
        cell: usize,
        /// Which quantity (`"density"`, `"pressure"`, ...).
        quantity: &'static str,
        /// The offending raw SI value.
        value: f64,
    },

    /// The equation of state could not close a state: a `(ρ,e)` inversion did
    /// not bracket or converge, or the state lies outside the backend's
    /// validity range (for IAPWS-IF97, 273.15–1073.15 K and up to 100 MPa).
    #[error("HEM equation of state failed{}: {detail}", match .cell { Some(c) => format!(" in cell {c}"), None => String::new() })]
    EquationOfState {
        /// The cell, when the failure came from the solver; `None` when the
        /// EOS was called directly.
        cell: Option<usize>,
        /// What failed, in prose, with the raw SI inputs.
        detail: String,
    },

    /// The chosen equation of state does not provide something the caller
    /// asked for, e.g. a [`super::HemBoundary::ChokedBreak`] on a backend
    /// with no critical-flow model.
    #[error("HEM equation of state `{eos}` does not support {what}")]
    Unsupported {
        /// The backend's name (see [`super::HemEquationOfState::name`]).
        eos: &'static str,
        /// What was asked for.
        what: &'static str,
    },
}

impl HemError {
    /// Attach a cell index to an [`HemError::EquationOfState`] that was raised
    /// without one. Other variants pass through unchanged.
    pub(crate) fn in_cell(self, cell: usize) -> Self {
        match self {
            HemError::EquationOfState { cell: None, detail } => HemError::EquationOfState {
                cell: Some(cell),
                detail,
            },
            other => other,
        }
    }
}
