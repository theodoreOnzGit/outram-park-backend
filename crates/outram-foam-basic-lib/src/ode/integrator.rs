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

//! Enum-dispatched ODE integration — solver choice *and* system ownership.
//!
//! # What this adds over the bare steppers
//!
//! [`Euler`], [`Rkf45`] and [`Rosenbrock23`] each integrate a system you hand
//! them by reference on every call. That is fine inside one function, but a
//! constitutive law or a solver loop usually wants to *store* "the integrator
//! for this material point" — solver plus system together — and step it later.
//! Storing a borrow would require a lifetime parameter on the storing struct,
//! which the workspace design rules forbid outright.
//!
//! This module removes the need for one. Two enums, no lifetimes anywhere:
//!
//! - [`OdeSolver`] — *which* stepper. A closed set (`Euler`, `Rkf45`,
//!   `Rosenbrock23`), so the scheme can be chosen at run time without a trait
//!   object and without heap allocation. Adding a stepper forces every `match`
//!   site to be updated.
//! - [`OdeIntegrator`] — *how the system is supplied*. Two variants, and they
//!   are the whole point of this module:
//!   - [`OdeIntegrator::TypedState`] owns a concrete, statically-known system
//!     **by value**. Derivative calls are statically dispatched and inlinable.
//!     **This is the preferred variant.**
//!   - [`OdeIntegrator::DynSystem`] holds the system behind
//!     [`SharedOdeSystem`] (`Arc<dyn OdeSystem + Send + Sync>`). It exists **by
//!     maintainer decision, for flexibility**, so a caller that genuinely does
//!     not know the system type at the call site — a registry, a case-file
//!     reader, a test harness sweeping several systems — has a path that does
//!     not require inventing an enum of its own.
//!
//! # Why there is no lifetime parameter
//!
//! Both variants **own** their system: `S` by value, or shared ownership
//! through `Arc`. Nothing here borrows the system, so nothing here needs to
//! name the region the borrow is valid for. An [`OdeIntegrator`] can therefore
//! be stored in a struct, moved between threads, or held in a `Vec` for one
//! integrator per material point, with no lifetime plumbing at any call site.
//!
//! # On the `dyn` in [`OdeIntegrator::DynSystem`]
//!
//! The workspace rule is *"no trait objects for dispatch — use enums"*, and the
//! dispatch here **is** the enum: [`OdeIntegrator`] chooses between two owning
//! strategies by `match`, exhaustively. The `Arc<dyn OdeSystem + Send + Sync>`
//! inside one of its variants is a boundary coercion for callers who ask for
//! it, kept deliberately, not an accident to be cleaned up later. Prefer
//! [`OdeIntegrator::TypedState`]; reach for [`OdeIntegrator::DynSystem`] when
//! the type genuinely is not known statically.
//!
//! # Example — the preferred typed path
//!
//! ```rust
//! use outram_foam_basic_lib::ode::{OdeIntegrator, OdeSolver, OdeSystem};
//!
//! struct Decay;
//! impl OdeSystem for Decay {
//!     fn n_eqns(&self) -> usize { 1 }
//!     fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
//!         dydx[0] = -y[0];
//!     }
//! }
//!
//! // Owned by value — no borrow, no lifetime, storable in any struct.
//! let mut integrator = OdeIntegrator::typed(Decay, OdeSolver::rkf45(1, 1e-10, 1e-8));
//! let mut y = vec![1.0_f64];
//! let mut dx = 0.1;
//! integrator.integrate(0.0, 1.0, &mut y, &mut dx).unwrap();
//! assert!((y[0] - (-1.0_f64).exp()).abs() < 1e-8);
//! ```
//!
//! # Example — the shared `dyn` path
//!
//! ```rust
//! use std::sync::Arc;
//! use outram_foam_basic_lib::ode::{OdeIntegrator, OdeSolver, OdeSystem, SharedOdeSystem};
//!
//! struct Decay;
//! impl OdeSystem for Decay {
//!     fn n_eqns(&self) -> usize { 1 }
//!     fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
//!         dydx[0] = -y[0];
//!     }
//! }
//!
//! let shared: SharedOdeSystem = Arc::new(Decay);
//! let mut integrator = OdeIntegrator::shared(shared, OdeSolver::rkf45(1, 1e-10, 1e-8));
//! let mut y = vec![1.0_f64];
//! let mut dx = 0.1;
//! integrator.integrate(0.0, 1.0, &mut y, &mut dx).unwrap();
//! assert!((y[0] - (-1.0_f64).exp()).abs() < 1e-8);
//! ```

use std::sync::Arc;

use super::{Euler, OdeError, OdeSolverConfig, OdeSystem, Rkf45, Rosenbrock23};

/// A shared, runtime-typed ODE system.
///
/// `Send + Sync` is required because the only reason to reach for `Arc` over
/// owning the system by value is to share it — including across threads, which
/// is how this workspace shares simulation state (`Arc<T>` for read-only data).
/// A system that cannot cross a thread boundary should use
/// [`OdeIntegrator::TypedState`] instead.
pub type SharedOdeSystem = Arc<dyn OdeSystem + Send + Sync>;

// ── Solver choice ────────────────────────────────────────────────────────────

/// Which stepper integrates the system — enum dispatch over the closed set of
/// solvers this crate ports.
///
/// The steppers carry per-equation scratch buffers sized at construction, so a
/// solver built for `n` equations must only be used with an `n`-equation
/// system. All variants integrate a bare `f64` state vector in the caller's own
/// units; only [`OdeSolverConfig`] tolerances are interpreted here.
///
/// Cloning a solver clones its scratch buffers; the clone integrates
/// independently of the original.
#[derive(Debug, Clone)]
pub enum OdeSolver {
    /// Explicit first-order Euler with adaptive step size. Cheapest per step,
    /// but the global error falls only linearly in the step size — use it for
    /// smooth, non-stiff systems where accuracy is not critical.
    Euler(Euler),
    /// Explicit Runge-Kutta-Fehlberg 4(5), the general-purpose default for
    /// non-stiff systems. Six derivative evaluations per step, fifth-order
    /// propagation with an embedded fourth-order error estimate.
    Rkf45(Rkf45),
    /// Semi-implicit W-method Rosenbrock23 for **stiff** systems. Requires the
    /// system to implement [`OdeSystem::jacobian`]; the default `jacobian`
    /// panics, so check [`OdeSolver::requires_jacobian`] before selecting it
    /// for a system you did not write.
    Rosenbrock23(Rosenbrock23),
}

impl OdeSolver {
    /// Explicit Euler for an `n`-equation system.
    ///
    /// `abs_tol` is the absolute and `rel_tol` the relative per-equation error
    /// tolerance, both dimensionless ratios against the state values; the
    /// remaining controller parameters take their [`OdeSolverConfig::default`]
    /// values.
    #[must_use]
    pub fn euler(n: usize, abs_tol: f64, rel_tol: f64) -> Self {
        Self::Euler(Euler::new(n, abs_tol, rel_tol))
    }

    /// Runge-Kutta-Fehlberg 4(5) for an `n`-equation system. See
    /// [`OdeSolver::euler`] for the tolerance arguments.
    #[must_use]
    pub fn rkf45(n: usize, abs_tol: f64, rel_tol: f64) -> Self {
        Self::Rkf45(Rkf45::new(n, abs_tol, rel_tol))
    }

    /// Stiff Rosenbrock23 for an `n`-equation system. See [`OdeSolver::euler`]
    /// for the tolerance arguments. The system **must** implement
    /// [`OdeSystem::jacobian`].
    #[must_use]
    pub fn rosenbrock23(n: usize, abs_tol: f64, rel_tol: f64) -> Self {
        Self::Rosenbrock23(Rosenbrock23::new(n, abs_tol, rel_tol))
    }

    /// The stepper's name, for diagnostics and log lines.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Euler(_) => "Euler",
            Self::Rkf45(_) => "RKF45",
            Self::Rosenbrock23(_) => "Rosenbrock23",
        }
    }

    /// Whether this stepper calls [`OdeSystem::jacobian`], whose default
    /// implementation panics. `true` only for
    /// [`Rosenbrock23`](OdeSolver::Rosenbrock23).
    #[must_use]
    pub const fn requires_jacobian(&self) -> bool {
        matches!(self, Self::Rosenbrock23(_))
    }

    /// The adaptive step-size controller settings in force.
    #[must_use]
    pub fn config(&self) -> &OdeSolverConfig {
        match self {
            Self::Euler(s) => &s.config,
            Self::Rkf45(s) => &s.config,
            Self::Rosenbrock23(s) => &s.config,
        }
    }

    /// Mutable access to the controller settings, e.g. to lower `max_steps`.
    pub fn config_mut(&mut self) -> &mut OdeSolverConfig {
        match self {
            Self::Euler(s) => &mut s.config,
            Self::Rkf45(s) => &mut s.config,
            Self::Rosenbrock23(s) => &mut s.config,
        }
    }

    /// Take one adaptive step of `ode`. On return `x` and `y` are advanced and
    /// `dx_try` holds the step size suggested for the next call.
    ///
    /// `Sys` is `?Sized`, so this accepts both a concrete `&MySystem` (static
    /// dispatch) and a `&dyn OdeSystem` (dynamic) without a second method.
    ///
    /// # Errors
    ///
    /// [`OdeError::StepSizeUnderflow`] if the tolerance cannot be met.
    pub fn solve_step<Sys: OdeSystem + ?Sized>(
        &mut self,
        ode: &Sys,
        x: &mut f64,
        y: &mut Vec<f64>,
        dx_try: &mut f64,
    ) -> Result<(), OdeError> {
        match self {
            Self::Euler(s) => s.solve_step(ode, x, y, dx_try),
            Self::Rkf45(s) => s.solve_step(ode, x, y, dx_try),
            Self::Rosenbrock23(s) => s.solve_step(ode, x, y, dx_try),
        }
    }

    /// Integrate `ode` from `x_start` to `x_end`, updating `y` in place and
    /// leaving the last accepted step size in `dx_est`.
    ///
    /// # Errors
    ///
    /// [`OdeError::StepSizeUnderflow`] or [`OdeError::MaxStepsExceeded`].
    pub fn integrate<Sys: OdeSystem + ?Sized>(
        &mut self,
        ode: &Sys,
        x_start: f64,
        x_end: f64,
        y: &mut Vec<f64>,
        dx_est: &mut f64,
    ) -> Result<(), OdeError> {
        match self {
            Self::Euler(s) => s.integrate(ode, x_start, x_end, y, dx_est),
            Self::Rkf45(s) => s.integrate(ode, x_start, x_end, y, dx_est),
            Self::Rosenbrock23(s) => s.integrate(ode, x_start, x_end, y, dx_est),
        }
    }
}

// ── The placeholder system type ──────────────────────────────────────────────

/// The zero-equation system, and the default type argument of
/// [`OdeIntegrator`].
///
/// [`OdeIntegrator::DynSystem`] does not use the `S` type parameter, but Rust
/// still requires one to be named. `NoTypedSystem` is that name: writing
/// `OdeIntegrator` with no type argument means "the `dyn` variant is the only
/// one I intend to use".
///
/// It is a genuine, well-defined system rather than a panicking stub — a system
/// of zero equations, whose derivative vector is empty — so nothing goes wrong
/// if one is integrated by accident. Integrating it is simply a no-op on an
/// empty state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoTypedSystem;

impl OdeSystem for NoTypedSystem {
    fn n_eqns(&self) -> usize {
        0
    }

    fn derivatives(&self, _x: f64, _y: &[f64], _dydx: &mut Vec<f64>) {}
}

// ── The two owning integrators ───────────────────────────────────────────────

/// Solver plus a **statically-typed, owned** system — the preferred variant of
/// [`OdeIntegrator`].
///
/// The system is stored by value, so derivative evaluation is a direct,
/// inlinable call with no vtable and no borrow to outlive. `S` may be any
/// concrete type implementing [`OdeSystem`], including the caller's own enum
/// over several systems.
#[derive(Debug)]
pub struct TypedStateIntegrator<S: OdeSystem> {
    /// Which stepper advances the state.
    pub solver: OdeSolver,
    /// The system being integrated, owned outright.
    pub system: S,
}

impl<S: OdeSystem> TypedStateIntegrator<S> {
    /// Pair an owned system with a solver.
    pub fn new(system: S, solver: OdeSolver) -> Self {
        Self { solver, system }
    }

    /// Take one adaptive step. See [`OdeSolver::solve_step`].
    ///
    /// # Errors
    ///
    /// [`OdeError::StepSizeUnderflow`].
    pub fn solve_step(
        &mut self,
        x: &mut f64,
        y: &mut Vec<f64>,
        dx_try: &mut f64,
    ) -> Result<(), OdeError> {
        let Self { solver, system } = self;
        solver.solve_step(&*system, x, y, dx_try)
    }

    /// Integrate from `x_start` to `x_end`. See [`OdeSolver::integrate`].
    ///
    /// # Errors
    ///
    /// [`OdeError::StepSizeUnderflow`] or [`OdeError::MaxStepsExceeded`].
    pub fn integrate(
        &mut self,
        x_start: f64,
        x_end: f64,
        y: &mut Vec<f64>,
        dx_est: &mut f64,
    ) -> Result<(), OdeError> {
        let Self { solver, system } = self;
        solver.integrate(&*system, x_start, x_end, y, dx_est)
    }

    /// Consume the integrator and return the system, e.g. to read state the
    /// system accumulated during integration.
    pub fn into_system(self) -> S {
        self.system
    }
}

/// Solver plus a **shared, runtime-typed** system.
///
/// The flexibility variant, kept by maintainer decision: the system type need
/// not be known where the integrator is built, and the same system can be
/// shared by several integrators. Prefer [`TypedStateIntegrator`] when the type
/// *is* known — it dispatches statically.
#[derive(Clone)]
pub struct DynSystemIntegrator {
    /// Which stepper advances the state.
    pub solver: OdeSolver,
    /// The system being integrated, shared by `Arc`. Cloning the integrator
    /// shares the system rather than duplicating it.
    pub system: SharedOdeSystem,
}

// `dyn OdeSystem` is not `Debug`, so derive cannot produce this.
impl std::fmt::Debug for DynSystemIntegrator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DynSystemIntegrator")
            .field("solver", &self.solver)
            .field(
                "system",
                &format_args!("<dyn OdeSystem, {} eqns>", self.system.n_eqns()),
            )
            .finish()
    }
}

impl DynSystemIntegrator {
    /// Pair a shared system with a solver.
    pub fn new(system: SharedOdeSystem, solver: OdeSolver) -> Self {
        Self { solver, system }
    }

    /// Take one adaptive step. See [`OdeSolver::solve_step`].
    ///
    /// # Errors
    ///
    /// [`OdeError::StepSizeUnderflow`].
    pub fn solve_step(
        &mut self,
        x: &mut f64,
        y: &mut Vec<f64>,
        dx_try: &mut f64,
    ) -> Result<(), OdeError> {
        let Self { solver, system } = self;
        solver.solve_step(system.as_ref(), x, y, dx_try)
    }

    /// Integrate from `x_start` to `x_end`. See [`OdeSolver::integrate`].
    ///
    /// # Errors
    ///
    /// [`OdeError::StepSizeUnderflow`] or [`OdeError::MaxStepsExceeded`].
    pub fn integrate(
        &mut self,
        x_start: f64,
        x_end: f64,
        y: &mut Vec<f64>,
        dx_est: &mut f64,
    ) -> Result<(), OdeError> {
        let Self { solver, system } = self;
        solver.integrate(system.as_ref(), x_start, x_end, y, dx_est)
    }
}

// ── The wrapper ──────────────────────────────────────────────────────────────

/// Enum-dispatch wrapper over the two ways of owning an ODE system.
///
/// This is the type to store when a struct needs "an integrator" as a field.
/// Neither variant borrows, so no lifetime parameter propagates outward — the
/// reason this wrapper exists.
///
/// The type argument `S` names the concrete system used by
/// [`TypedState`](Self::TypedState). It defaults to [`NoTypedSystem`], so an
/// integrator that only ever uses [`DynSystem`](Self::DynSystem) can be written
/// as a plain `OdeIntegrator`.
///
/// # Choosing a variant
///
/// | | [`TypedState`](Self::TypedState) | [`DynSystem`](Self::DynSystem) |
/// |---|---|---|
/// | System known at compile time | yes | no |
/// | Dispatch | static, inlinable | vtable |
/// | Ownership | by value | shared, `Arc` |
/// | Use when | normal case — **prefer this** | the type is chosen at run time |
#[derive(Debug)]
pub enum OdeIntegrator<S: OdeSystem = NoTypedSystem> {
    /// The typed-state integrator: a concrete system owned by value, with
    /// static dispatch. **Preferred.**
    TypedState(TypedStateIntegrator<S>),
    /// The `dyn`-system integrator: `Arc<dyn OdeSystem + Send + Sync>`. Kept by
    /// maintainer decision, for flexibility where the system type is not
    /// statically known.
    DynSystem(DynSystemIntegrator),
}

impl<S: OdeSystem> OdeIntegrator<S> {
    /// Build the preferred, statically-typed integrator from an owned system.
    ///
    /// ```rust
    /// # use outram_foam_basic_lib::ode::{OdeIntegrator, OdeSolver, OdeSystem};
    /// # struct Decay;
    /// # impl OdeSystem for Decay {
    /// #     fn n_eqns(&self) -> usize { 1 }
    /// #     fn derivatives(&self, _x: f64, y: &[f64], d: &mut Vec<f64>) { d[0] = -y[0]; }
    /// # }
    /// let integrator = OdeIntegrator::typed(Decay, OdeSolver::rkf45(1, 1e-8, 1e-6));
    /// assert!(integrator.is_typed_state());
    /// ```
    pub fn typed(system: S, solver: OdeSolver) -> Self {
        Self::TypedState(TypedStateIntegrator::new(system, solver))
    }

    /// Number of coupled equations the stored system reports.
    #[must_use]
    pub fn n_eqns(&self) -> usize {
        match self {
            Self::TypedState(i) => i.system.n_eqns(),
            Self::DynSystem(i) => i.system.n_eqns(),
        }
    }

    /// The stepper in use.
    #[must_use]
    pub fn solver(&self) -> &OdeSolver {
        match self {
            Self::TypedState(i) => &i.solver,
            Self::DynSystem(i) => &i.solver,
        }
    }

    /// Mutable access to the stepper, e.g. to adjust tolerances between steps.
    pub fn solver_mut(&mut self) -> &mut OdeSolver {
        match self {
            Self::TypedState(i) => &mut i.solver,
            Self::DynSystem(i) => &mut i.solver,
        }
    }

    /// `true` for the preferred, statically-dispatched variant.
    #[must_use]
    pub const fn is_typed_state(&self) -> bool {
        matches!(self, Self::TypedState(_))
    }

    /// The owned system, when this is the typed variant; `None` otherwise.
    #[must_use]
    pub fn typed_system(&self) -> Option<&S> {
        match self {
            Self::TypedState(i) => Some(&i.system),
            Self::DynSystem(_) => None,
        }
    }

    /// The shared system, when this is the `dyn` variant; `None` otherwise.
    #[must_use]
    pub fn shared_system(&self) -> Option<&SharedOdeSystem> {
        match self {
            Self::TypedState(_) => None,
            Self::DynSystem(i) => Some(&i.system),
        }
    }

    /// Take one adaptive step. `x` is the independent variable, `y` the state
    /// vector (both in the caller's units), `dx_try` the step size to attempt;
    /// on return `dx_try` is the size suggested for the next call.
    ///
    /// # Errors
    ///
    /// [`OdeError::StepSizeUnderflow`] if the requested tolerance cannot be met
    /// at any representable step size.
    pub fn solve_step(
        &mut self,
        x: &mut f64,
        y: &mut Vec<f64>,
        dx_try: &mut f64,
    ) -> Result<(), OdeError> {
        match self {
            Self::TypedState(i) => i.solve_step(x, y, dx_try),
            Self::DynSystem(i) => i.solve_step(x, y, dx_try),
        }
    }

    /// Integrate from `x_start` to `x_end`, updating `y` in place and leaving
    /// the last accepted step size in `dx_est`.
    ///
    /// # Errors
    ///
    /// [`OdeError::StepSizeUnderflow`], or [`OdeError::MaxStepsExceeded`] if
    /// the interval could not be spanned within `config().max_steps` sub-steps.
    pub fn integrate(
        &mut self,
        x_start: f64,
        x_end: f64,
        y: &mut Vec<f64>,
        dx_est: &mut f64,
    ) -> Result<(), OdeError> {
        match self {
            Self::TypedState(i) => i.integrate(x_start, x_end, y, dx_est),
            Self::DynSystem(i) => i.integrate(x_start, x_end, y, dx_est),
        }
    }
}

impl OdeIntegrator<NoTypedSystem> {
    /// Build the shared, runtime-typed integrator.
    ///
    /// Only available on `OdeIntegrator<NoTypedSystem>` — i.e. on the
    /// default-parameter spelling `OdeIntegrator` — so that the unused type
    /// argument never has to be written out.
    ///
    /// ```rust
    /// # use std::sync::Arc;
    /// # use outram_foam_basic_lib::ode::{OdeIntegrator, OdeSolver, OdeSystem, SharedOdeSystem};
    /// # struct Decay;
    /// # impl OdeSystem for Decay {
    /// #     fn n_eqns(&self) -> usize { 1 }
    /// #     fn derivatives(&self, _x: f64, y: &[f64], d: &mut Vec<f64>) { d[0] = -y[0]; }
    /// # }
    /// let shared: SharedOdeSystem = Arc::new(Decay);
    /// let integrator = OdeIntegrator::shared(shared, OdeSolver::rkf45(1, 1e-8, 1e-6));
    /// assert!(!integrator.is_typed_state());
    /// ```
    pub fn shared(system: SharedOdeSystem, solver: OdeSolver) -> Self {
        Self::DynSystem(DynSystemIntegrator::new(system, solver))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matrix::SquareMatrix;

    struct Decay;
    impl OdeSystem for Decay {
        fn n_eqns(&self) -> usize {
            1
        }
        fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
            dydx[0] = -y[0];
        }
        fn jacobian(&self, _x: f64, _y: &[f64], dfdx: &mut Vec<f64>, dfdy: &mut SquareMatrix) {
            dfdx[0] = 0.0;
            dfdy.set(0, 0, -1.0);
        }
    }

    /// Methodology: integrate `y' = -y`, `y(0) = 1` from `x = 0` to `x = 1`
    /// through the typed variant of the wrapper with each of the three
    /// steppers, and compare against the closed form `y(1) = e^{-1} =
    /// 0.3678794411714423`. Pass criterion: `|y - e^{-1}| < 1e-6` for the two
    /// second-order-or-better steppers and `< 1e-2` for Euler, whose global
    /// error is first order.
    ///
    /// Results, transcribed from the test's own `println!` output on
    /// 2026-08-05 (release mode, `cargo test -p outram-foam-basic-lib --lib
    /// --release ode::integrator -- --nocapture`):
    ///
    /// ```text
    /// Euler: y = 0.3665836536448949, error = 1.2958e-3
    /// RKF45: y = 0.3678794396441354, error = 1.5273e-9
    /// Rosenbrock23: y = 0.3678794397825046, error = 1.3889e-9
    /// ```
    ///
    /// All three are within their tolerance, and the two high-order steppers
    /// agree with each other to 1.4e-9 while Euler is six orders coarser —
    /// the expected signature of three genuinely different steppers, which is
    /// what confirms the enum dispatch reaches each of them rather than
    /// silently routing to one.
    #[test]
    fn typed_variant_reaches_every_stepper() {
        let exact = (-1.0_f64).exp();
        for (solver, tol) in [
            (OdeSolver::euler(1, 1e-3, 1e-2), 1e-2),
            (OdeSolver::rkf45(1, 1e-10, 1e-8), 1e-6),
            (OdeSolver::rosenbrock23(1, 1e-10, 1e-8), 1e-6),
        ] {
            let name = solver.name();
            let mut integrator = OdeIntegrator::typed(Decay, solver);
            let mut y = vec![1.0_f64];
            let mut dx = 0.1;
            integrator.integrate(0.0, 1.0, &mut y, &mut dx).unwrap();
            let err = (y[0] - exact).abs();
            println!("{name}: y = {:.16}, error = {err:.4e}", y[0]);
            assert!(err < tol, "{name}: y = {:.16}, error = {err:.4e}", y[0]);
        }
    }

    /// Methodology: run the identical problem of
    /// [`typed_variant_reaches_every_stepper`] through the `dyn` variant with
    /// RKF45 and require the result to be **bit-identical** to the typed
    /// variant's. The two paths differ only in how the system is reached
    /// (static call versus vtable), so any difference in the final state would
    /// mean one path is not calling the same derivative function.
    ///
    /// Result, transcribed from the test's `println!` on 2026-08-05 (release
    /// mode): `typed = 0.36787943964413539, shared = 0.36787943964413539` —
    /// equal to the last bit, as the `assert_eq!` on the raw `f64` requires.
    #[test]
    fn dyn_variant_matches_typed_bit_for_bit() {
        let mut typed = OdeIntegrator::typed(Decay, OdeSolver::rkf45(1, 1e-10, 1e-8));
        let mut y_typed = vec![1.0_f64];
        let mut dx = 0.1;
        typed.integrate(0.0, 1.0, &mut y_typed, &mut dx).unwrap();

        let shared: SharedOdeSystem = Arc::new(Decay);
        let mut dynamic = OdeIntegrator::shared(shared, OdeSolver::rkf45(1, 1e-10, 1e-8));
        let mut y_dyn = vec![1.0_f64];
        let mut dx2 = 0.1;
        dynamic.integrate(0.0, 1.0, &mut y_dyn, &mut dx2).unwrap();

        println!("typed = {:.17}, shared = {:.17}", y_typed[0], y_dyn[0]);
        assert_eq!(
            y_typed[0], y_dyn[0],
            "typed = {:.17}, shared = {:.17}",
            y_typed[0], y_dyn[0]
        );
    }

    /// Methodology: the wrapper's reason for existing is that it can be a
    /// *field* of a struct with no lifetime parameter. This test is a
    /// compile-time assertion of exactly that: `Holder` stores an
    /// `OdeIntegrator<Decay>` by value and steps it. If the wrapper ever
    /// acquired a borrow, `Holder` would stop compiling without a `'a`.
    ///
    /// Result, transcribed from the test's `println!` on 2026-08-05 (release
    /// mode): `stored integrator: n_eqns = 1, y = 0.3678794396441354` — the
    /// struct compiles with no lifetime parameter and its stored integrator
    /// reproduces the same `e^{-1}` as the free-standing RKF45 run above.
    #[test]
    fn integrator_is_storable_without_a_lifetime() {
        struct Holder {
            integrator: OdeIntegrator<Decay>,
            state: Vec<f64>,
        }

        let mut holder = Holder {
            integrator: OdeIntegrator::typed(Decay, OdeSolver::rkf45(1, 1e-10, 1e-8)),
            state: vec![1.0],
        };
        let mut dx = 0.1;
        holder
            .integrator
            .integrate(0.0, 1.0, &mut holder.state, &mut dx)
            .unwrap();
        println!(
            "stored integrator: n_eqns = {}, y = {:.16}",
            holder.integrator.n_eqns(),
            holder.state[0]
        );
        assert_eq!(holder.integrator.n_eqns(), 1);
        assert!(
            (holder.state[0] - (-1.0_f64).exp()).abs() < 1e-6,
            "y = {:.16}",
            holder.state[0]
        );
    }

    /// Methodology: check the metadata accessors report the truth for each
    /// variant — `requires_jacobian` must be true only for Rosenbrock23,
    /// `name` must be distinct, and `typed_system`/`shared_system` must each
    /// return `Some` for exactly one variant.
    ///
    /// Results (2026-08-05): `Euler`/`RKF45` report `requires_jacobian =
    /// false`, `Rosenbrock23` reports `true`; the typed integrator returns
    /// `Some` from `typed_system` and `None` from `shared_system`, and the
    /// shared integrator the reverse. All assertions below passed.
    #[test]
    fn accessors_report_the_variant_truthfully() {
        assert!(!OdeSolver::euler(1, 1e-6, 1e-4).requires_jacobian());
        assert!(!OdeSolver::rkf45(1, 1e-6, 1e-4).requires_jacobian());
        assert!(OdeSolver::rosenbrock23(1, 1e-6, 1e-4).requires_jacobian());
        assert_eq!(OdeSolver::euler(1, 1e-6, 1e-4).name(), "Euler");
        assert_eq!(OdeSolver::rkf45(1, 1e-6, 1e-4).name(), "RKF45");
        assert_eq!(
            OdeSolver::rosenbrock23(1, 1e-6, 1e-4).name(),
            "Rosenbrock23"
        );

        let typed = OdeIntegrator::typed(Decay, OdeSolver::rkf45(1, 1e-6, 1e-4));
        assert!(typed.typed_system().is_some());
        assert!(typed.shared_system().is_none());

        let shared: SharedOdeSystem = Arc::new(Decay);
        let dynamic = OdeIntegrator::shared(shared, OdeSolver::rkf45(1, 1e-6, 1e-4));
        assert!(dynamic.typed_system().is_none());
        assert!(dynamic.shared_system().is_some());
    }

    /// Methodology: `NoTypedSystem` is the default type argument and must be a
    /// harmless zero-equation system rather than a panicking stub. Integrate it
    /// directly and require no panic and an untouched (empty) state.
    ///
    /// Result (2026-08-05): integration from `x = 0` to `x = 1` completed with
    /// `n_eqns() = 0` and the state vector still empty (`len = 0`).
    #[test]
    fn placeholder_system_integrates_as_a_no_op() {
        let mut integrator = OdeIntegrator::typed(NoTypedSystem, OdeSolver::euler(0, 1e-6, 1e-4));
        let mut y: Vec<f64> = Vec::new();
        let mut dx = 0.5;
        integrator.integrate(0.0, 1.0, &mut y, &mut dx).unwrap();
        assert_eq!(integrator.n_eqns(), 0);
        assert_eq!(y.len(), 0);
    }
}
