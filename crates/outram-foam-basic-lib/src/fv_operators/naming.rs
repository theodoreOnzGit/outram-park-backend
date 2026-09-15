// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Bounded names for operator-derived fields.
//!
//! # What belongs here
//!
//! The two helpers every `fvc::` operator uses to name its result, and nothing
//! else. They exist to make a specific memory bug **structurally impossible**
//! rather than merely avoided by convention.
//!
//! # The bug this prevents
//!
//! This crate's `CLAUDE.md` records that building a field's `name` from its
//! operands' names is dangerous: a solver that repeatedly reassigns a
//! persistent field from an expression containing that same field makes the
//! `name` `String` grow every timestep. In the original incident the growth was
//! *exponential* — doubling per step, reaching tens of gigabytes within about
//! 25 steps and killing the `compressible_lid_cavity` test with a 24 GB
//! SIGTERM. The data was always correct; only the label ran away.
//!
//! The arithmetic operators on [`crate::fields::VolField`] and
//! [`crate::fields::SurfaceField`] were fixed by keeping the left operand's
//! name, and that fix is still in place. But the `fvc::` operators legitimately
//! *do* derive compound names — `div(phi,rho)`, `grad(p)`, `interpolate(T)` —
//! because those names are genuinely useful diagnostics, and OpenFOAM names
//! them the same way.
//!
//! That leaves a narrower hole, which is real rather than theoretical. Some
//! `fvc::` operators return the **same type** they consume:
//!
//! - [`crate::fv_operators::fvc::div`] — `VolScalarField` in, `VolScalarField` out
//! - [`crate::fv_operators::fvc::div_vec`] — `VolVectorField` in, `VolVectorField` out
//!
//! so `psi = fvc::div(&phi, &psi)` compiles. Each call would then nest the
//! name one level deeper: `rho`, `div(phi,rho)`, `div(phi,div(phi,rho))`, and
//! so on without bound. That is linear rather than exponential growth, so it
//! is slower to bite than the original bug — which is precisely what makes it
//! easy to miss.
//!
//! # The rule
//!
//! An operand that is *already* an operator-derived name is elided to `..`
//! instead of being nested. So the sequence above reaches a **fixed point**
//! after one application:
//!
//! ```text
//! rho  ->  div(phi,rho)  ->  div(phi,..)  ->  div(phi,..)  ->  ...
//! ```
//!
//! The first application keeps full diagnostic value, which is the one that
//! matters when reading a solver's field list. Every later application is
//! idempotent, so the name cannot grow no matter how many timesteps run.
//!
//! # Units
//!
//! None — these are diagnostic labels, not quantities.

/// The marker substituted for an operand that is itself operator-derived.
///
/// Chosen as plain ASCII so field names stay safe to write into an OpenFOAM
/// case file and to compare in tests.
pub const ELIDED_OPERAND: &str = "..";

/// Whether `name` is already an operator-derived name rather than a plain
/// field label.
///
/// # How it decides
///
/// A derived name always contains `(`, because every operator in
/// [`crate::fv_operators::fvc`] wraps its operands in parentheses. A
/// user-declared field name (`rho`, `U`, `p`, `T`, `alpha.water`) does not.
///
/// # Arguments
///
/// - `name` — a field name. Dimensionless text.
///
/// # Example
///
/// ```
/// use outram_foam_basic_lib::fv_operators::naming::is_derived;
///
/// assert!(!is_derived("rho"));
/// assert!(!is_derived("alpha.water"));
/// assert!(is_derived("div(phi,rho)"));
/// ```
#[must_use]
pub fn is_derived(name: &str) -> bool {
    name.contains('(')
}

/// Name the result of a one-operand operator, without unbounded nesting.
///
/// # Arguments
///
/// - `op` — the operator's name, e.g. `"grad"`, `"interpolate"`, `"snGrad"`.
/// - `operand` — the input field's name.
///
/// # Returns
///
/// `op(operand)` when `operand` is a plain field name, or `op(..)` when it is
/// already derived. The length is therefore bounded by `op.len() + 4` in the
/// worst case, regardless of how many times the operator is applied.
///
/// # Example
///
/// ```
/// use outram_foam_basic_lib::fv_operators::naming::derived_name;
///
/// assert_eq!(derived_name("grad", "p"), "grad(p)");
///
/// // Repeated application reaches a fixed point instead of growing.
/// let once = derived_name("grad", "p");
/// let twice = derived_name("grad", &once);
/// let thrice = derived_name("grad", &twice);
/// assert_eq!(twice, "grad(..)");
/// assert_eq!(thrice, "grad(..)");
/// ```
#[must_use]
pub fn derived_name(op: &str, operand: &str) -> String {
    if is_derived(operand) {
        format!("{op}({ELIDED_OPERAND})")
    } else {
        format!("{op}({operand})")
    }
}

/// Name the result of a two-operand operator, without unbounded nesting.
///
/// Each operand is elided independently, so `div(phi,rho)` keeps both useful
/// names while `div(phi, div(phi,rho))` collapses to `div(phi,..)`.
///
/// # Arguments
///
/// - `op` — the operator's name, e.g. `"div"`.
/// - `first`, `second` — the input fields' names.
///
/// # Returns
///
/// `op(first,second)` with either operand replaced by `..` if it is already
/// derived. This is the helper that closes the
/// [`crate::fv_operators::fvc::div`] self-feedback hole described in the
/// module docs.
///
/// # Example
///
/// ```
/// use outram_foam_basic_lib::fv_operators::naming::derived_name2;
///
/// assert_eq!(derived_name2("div", "phi", "rho"), "div(phi,rho)");
///
/// // The self-feedback pattern `rho = div(phi, rho)` reaches a fixed point.
/// let step1 = derived_name2("div", "phi", "rho");
/// let step2 = derived_name2("div", "phi", &step1);
/// let step3 = derived_name2("div", "phi", &step2);
/// assert_eq!(step1, "div(phi,rho)");
/// assert_eq!(step2, "div(phi,..)");
/// assert_eq!(step3, "div(phi,..)");
/// ```
#[must_use]
pub fn derived_name2(op: &str, first: &str, second: &str) -> String {
    let a = if is_derived(first) {
        ELIDED_OPERAND
    } else {
        first
    };
    let b = if is_derived(second) {
        ELIDED_OPERAND
    } else {
        second
    };
    format!("{op}({a},{b})")
}

#[cfg(test)]
mod tests;
