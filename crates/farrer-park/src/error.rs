// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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

//! The crate error type.
//!
//! # What belongs in this module
//!
//! One `thiserror`-derived enum, [`FemError`], covering every way a Farrer Park
//! call can fail, and the [`Result`] alias every fallible function returns.
//! Each variant carries enough context to name the offending element, node or
//! quantity without the caller having to guess.
//!
//! # What does NOT belong here
//!
//! Warnings, diagnostics, or convergence *reports*. A Newton solve that runs
//! out of iterations returns [`FemError::NewtonNotConverged`]; a Newton solve
//! that converges returns a report struct from [`crate::solver`], not an error.

use thiserror::Error;

/// Result alias for every fallible Farrer Park operation.
pub type Result<T> = std::result::Result<T, FemError>;

/// Everything that can go wrong in a Farrer Park finite-element run.
///
/// Variants are ordered roughly by the stage they arise in: mesh, then
/// assembly, then constitutive integration, then the linear and nonlinear
/// solves.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum FemError {
    /// A mesh generator or reader was given a parameter that cannot describe a
    /// mesh (zero divisions, non-positive size, inner radius not less than
    /// outer).
    #[error("invalid mesh parameter `{parameter}` = {value}: {reason}")]
    InvalidMeshParameter {
        /// Name of the offending parameter, as it appears in the signature.
        parameter: &'static str,
        /// The value supplied.
        value: f64,
        /// Why it is not usable.
        reason: &'static str,
    },

    /// An element's connectivity references a node that does not exist.
    #[error("element {element} references node {node}, but the mesh has {n_nodes} nodes")]
    NodeOutOfRange {
        /// Zero-based element index.
        element: usize,
        /// The offending zero-based node index.
        node: usize,
        /// How many nodes the mesh actually has.
        n_nodes: usize,
    },

    /// The isoparametric mapping is singular or inverted at a quadrature point:
    /// `det J <= 0`. Almost always a badly shaped or inside-out element.
    ///
    /// `det_j` has units of length^dim (m^2 in 2-D, m^3 in 3-D).
    #[error("element {element} has non-positive Jacobian determinant {det_j} m^dim at quadrature point {point}")]
    DegenerateElement {
        /// Zero-based element index.
        element: usize,
        /// Zero-based quadrature-point index within the element.
        point: usize,
        /// The computed determinant, length^dim.
        det_j: f64,
    },

    /// A quadrature rule of the requested strength does not exist for the
    /// requested reference element.
    #[error("no quadrature rule of exactness order {order} implemented for {element}")]
    NoSuchQuadratureRule {
        /// Name of the reference element.
        element: &'static str,
        /// Requested polynomial exactness order.
        order: usize,
    },

    /// Two slices that must have matching lengths did not.
    #[error("length mismatch in `{context}`: expected {expected}, got {actual}")]
    LengthMismatch {
        /// Where the mismatch was detected.
        context: &'static str,
        /// The length required.
        expected: usize,
        /// The length supplied.
        actual: usize,
    },

    /// A material parameter is outside the range where the model is defined.
    ///
    /// Young's modulus must be strictly positive \[Pa\]; Poisson's ratio must
    /// satisfy `-1 < nu < 0.5` for a positive-definite isotropic tensor; yield
    /// stress must be positive \[Pa\]; the hardening modulus must exceed
    /// `-3 mu` \[Pa\] or the radial return is not unique.
    #[error("material parameter `{parameter}` = {value} {unit} is out of range: {reason}")]
    MaterialOutOfRange {
        /// Name of the parameter.
        parameter: &'static str,
        /// The value supplied.
        value: f64,
        /// SI unit of the value, spelled out.
        unit: &'static str,
        /// The admissible range and why.
        reason: &'static str,
    },

    /// The local constitutive integration (the radial return) failed to reach
    /// its tolerance.
    ///
    /// Returning an unconverged stress would leave a quadrature point off the
    /// yield surface, which propagates silently into the momentum balance and
    /// looks like a stiffer material.
    #[error(
        "radial return at element {element}, quadrature point {point} did not converge \
         in {iterations} iterations (residual {residual} Pa)"
    )]
    ConstitutiveNotConverged {
        /// Zero-based element index.
        element: usize,
        /// Zero-based quadrature-point index within the element.
        point: usize,
        /// Iterations spent.
        iterations: usize,
        /// Final consistency residual \[Pa\].
        residual: f64,
    },

    /// The Krylov solve did not reach its relative-residual tolerance.
    #[error(
        "linear solve did not converge: {n_iterations} iterations, \
         relative residual {final_residual} > tolerance {tolerance}"
    )]
    LinearSolveNotConverged {
        /// Iterations performed.
        n_iterations: usize,
        /// True relative residual `||b - K u||_2 / ||b||_2` (dimensionless).
        final_residual: f64,
        /// The tolerance that was requested (dimensionless).
        tolerance: f64,
    },

    /// The Newton iteration exhausted its iteration budget, and all permitted
    /// load-step cutbacks, without converging.
    #[error(
        "Newton did not converge at load factor {load_factor}: \
         {iterations} iterations, relative residual {residual}, {cutbacks} cutbacks used"
    )]
    NewtonNotConverged {
        /// Fraction of the total load applied when the failure occurred
        /// (dimensionless, 0 to 1).
        load_factor: f64,
        /// Newton iterations spent in the failing step.
        iterations: usize,
        /// Relative residual reached (dimensionless).
        residual: f64,
        /// How many step cutbacks had already been taken.
        cutbacks: usize,
    },

    /// A boundary condition refers to a degree of freedom that does not exist,
    /// or the constrained set leaves the system singular (rigid-body motion not
    /// removed).
    #[error("boundary condition problem: {0}")]
    BoundaryCondition(String),
}
