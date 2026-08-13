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

//! MODULE-DOC-PLACEHOLDER

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::compute::ComputeBackend;
use crate::matrix::SquareMatrix;
use crate::ode::OdeSystem;

#[cfg(test)]
mod tests;

// ── Constants ────────────────────────────────────────────────────────────────

/// `f64::EPSILON.cbrt()` = `6.0554544523933395e-6`.
pub const CBRT_EPSILON: f64 = 6.055_454_452_393_339_5e-6;

/// `f64::EPSILON.powf(0.2)` = `7.40095979741405e-4`.
pub const FIFTH_ROOT_EPSILON: f64 = 7.400_959_797_414_05e-4;

/// Crossover placeholder — replaced by measurement.
pub const DERIVATIVE_BATCH_MIN_POINTS: usize = 256;

/// Crossover placeholder — replaced by measurement.
pub const JACOBIAN_BATCH_MIN_PROBLEMS: usize = 64;

/// Crossover placeholder — replaced by measurement.
pub const JACOBIAN_COLUMN_MIN_DIMENSION: usize = 64;

// ── Backend dispatch ─────────────────────────────────────────────────────────

/// Resolve a requested backend to the one this module will actually run.
///
/// Three reductions, in order: [`ComputeBackend::resolve`] degrades anything
/// whose feature is off; `Gpu` degrades again because this module has no GPU
/// kernel yet; and `CpuMulti` degrades to `Serial` below `min_work_items`. The
/// result is only ever `Serial` or `CpuMulti`, and none of the degradations can
/// change a returned value.
///
/// Identical in shape to `minimise::effective_backend` and
/// `parallel::effective_backend`; kept private and duplicated rather than
/// hoisted because all three are four lines and hoisting would put a
/// dispatch-policy helper in a module whose docs say it holds no kernels.
fn effective_backend(
    requested: ComputeBackend,
    work_items: usize,
    min_work_items: usize,
) -> ComputeBackend {
    let cpu = match requested.resolve() {
        ComputeBackend::Gpu => ComputeBackend::CpuMulti.resolve(),
        other => other,
    };
    match cpu {
        ComputeBackend::CpuMulti if work_items >= min_work_items => ComputeBackend::CpuMulti,
        _ => ComputeBackend::Serial,
    }
}

/// The [`ComputeBackend`] [`derivative_batch`] would actually use for `n`
/// points if asked for `requested` — without running anything.
///
/// Applies exactly the same reduction the kernel does (feature availability, no
/// GPU kernel here, and the [`DERIVATIVE_BATCH_MIN_POINTS`] size floor), so what
/// it reports is what would run.
///
/// # Arguments
///
/// - `requested` — the backend a caller would pass to [`derivative_batch`].
/// - `n` — number of independent points in the batch, dimensionless.
///
/// # Returns
///
/// Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`]; never
/// [`ComputeBackend::Gpu`], because no GPU kernel exists here yet.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::differentiate::{
///     derivative_backend_for, DERIVATIVE_BATCH_MIN_POINTS,
/// };
///
/// assert_eq!(
///     derivative_backend_for(ComputeBackend::CpuMulti, 8),
///     ComputeBackend::Serial
/// );
/// assert!(derivative_backend_for(ComputeBackend::CpuMulti, DERIVATIVE_BATCH_MIN_POINTS)
///     .is_available());
/// ```
#[must_use]
pub fn derivative_backend_for(requested: ComputeBackend, n: usize) -> ComputeBackend {
    effective_backend(requested, n, DERIVATIVE_BATCH_MIN_POINTS)
}

/// The [`ComputeBackend`] [`jacobian_batch`] would actually use for `n`
/// independent Jacobian problems — without running anything.
///
/// # Arguments
///
/// - `requested` — the backend a caller would pass to [`jacobian_batch`].
/// - `n` — number of independent Jacobian problems (lanes), dimensionless.
///
/// # Returns
///
/// Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`].
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::differentiate::jacobian_batch_backend_for;
///
/// assert_eq!(
///     jacobian_batch_backend_for(ComputeBackend::CpuMulti, 2),
///     ComputeBackend::Serial
/// );
/// ```
#[must_use]
pub fn jacobian_batch_backend_for(requested: ComputeBackend, n: usize) -> ComputeBackend {
    effective_backend(requested, n, JACOBIAN_BATCH_MIN_PROBLEMS)
}

/// The [`ComputeBackend`] [`jacobian`] would actually use to spread the columns
/// of **one** `dimension`-dimensional Jacobian — without running anything.
///
/// This is the *other* axis of parallelism in this module: [`jacobian_batch`]
/// spreads independent problems across threads, while [`jacobian`] spreads the
/// `n` independent column evaluations of a single problem.
///
/// # Arguments
///
/// - `requested` — the backend a caller would pass to [`jacobian`].
/// - `dimension` — the length of the point `x`, i.e. the number of Jacobian
///   columns. Dimensionless.
///
/// # Returns
///
/// Either [`ComputeBackend::Serial`] or [`ComputeBackend::CpuMulti`].
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::differentiate::jacobian_column_backend_for;
///
/// // A 3-equation ODE Jacobian is never worth threading.
/// assert_eq!(
///     jacobian_column_backend_for(ComputeBackend::CpuMulti, 3),
///     ComputeBackend::Serial
/// );
/// ```
#[must_use]
pub fn jacobian_column_backend_for(requested: ComputeBackend, dimension: usize) -> ComputeBackend {
    effective_backend(requested, dimension, JACOBIAN_COLUMN_MIN_DIMENSION)
}

// ── Scheme, settings ─────────────────────────────────────────────────────────

/// Which finite-difference stencil to use.
///
/// The choice is a **cost against accuracy** trade, and both halves are
/// measured — see the module-level "Achievable accuracy" table for the observed
/// error floors and "Cost" for the evaluation counts.
///
/// # Units
///
/// Dimensionless — a mode selector, not a quantity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DiffScheme {
    /// `(f(x + h) - f(x)) / h`. Truncation error `O(h)`.
    ///
    /// The cheapest scheme for a Jacobian: the base evaluation `f(x)` is shared
    /// by every column, so an `n`-dimensional Jacobian costs `n + 1`
    /// evaluations rather than `2n`.
    Forward,
    /// `(f(x) - f(x - h)) / h`. Truncation error `O(h)`.
    ///
    /// Same cost and accuracy as [`Forward`](Self::Forward); it exists for
    /// callers whose function is undefined or unphysical just *above* `x` — a
    /// saturation pressure at the phase boundary, a volume fraction at 1.
    Backward,
    /// `(f(x + h) - f(x - h)) / (2h)`. Truncation error `O(h^2)`. **The
    /// default.**
    ///
    /// This is the scheme both existing workspace implementations use — see the
    /// module-level "Provenance" section.
    Central,
    /// Richardson extrapolation of two central differences, `(4*D(h/2) -
    /// D(h)) / 3`. Truncation error `O(h^4)`.
    ///
    /// The most accurate scheme here and the most expensive: 4 evaluations per
    /// derivative and `4n` per Jacobian, because the `h` and `h/2` stencils
    /// share no points.
    #[default]
    Central4th,
}

impl DiffScheme {
    /// The relative step size that balances truncation against round-off for
    /// this scheme in `f64`.
    ///
    /// Truncation error goes as `h^p` and round-off as `eps/h`, so the balance
    /// is at `h ~ eps^(1/(p+1))`:
    ///
    /// | Scheme | Order `p` | Optimal relative step |
    /// |---|---|---|
    /// | [`Forward`](Self::Forward), [`Backward`](Self::Backward) | 1 | [`crate::math::minimise::SQRT_EPSILON`] = `1.4901161193847656e-8` |
    /// | [`Central`](Self::Central) | 2 | [`CBRT_EPSILON`] = `6.0554544523933395e-6` |
    /// | [`Central4th`](Self::Central4th) | 4 | [`FIFTH_ROOT_EPSILON`] = `7.40095979741405e-4` |
    ///
    /// # Units
    ///
    /// Dimensionless — it multiplies a length scale in `x` to give a step in
    /// `x`.
    #[must_use]
    pub fn default_relative_step(self) -> f64 {
        match self {
            Self::Forward | Self::Backward => crate::math::minimise::SQRT_EPSILON,
            Self::Central => CBRT_EPSILON,
            Self::Central4th => FIFTH_ROOT_EPSILON,
        }
    }

    /// How many evaluations of the function one **scalar** derivative costs.
    ///
    /// # Units
    ///
    /// A count, dimensionless.
    #[must_use]
    pub fn evaluations_per_derivative(self) -> usize {
        match self {
            Self::Forward | Self::Backward | Self::Central => 2,
            Self::Central4th => 4,
        }
    }

    /// How many evaluations of the vector function an `n`-dimensional Jacobian
    /// costs.
    ///
    /// [`Forward`](Self::Forward) and [`Backward`](Self::Backward) get `n + 1`
    /// because the unperturbed evaluation is shared across all `n` columns;
    /// [`Central`](Self::Central) gets `2n` and [`Central4th`](Self::Central4th)
    /// `4n` because their stencils are symmetric about `x` and so share nothing.
    ///
    /// # Arguments
    ///
    /// - `n` — the dimension of the point, dimensionless.
    ///
    /// # Units
    ///
    /// A count, dimensionless.
    ///
    /// # Example
    ///
    /// ```rust
    /// use outram_foam_basic_lib::math::differentiate::DiffScheme;
    ///
    /// assert_eq!(DiffScheme::Forward.evaluations_per_jacobian(10), 11);
    /// assert_eq!(DiffScheme::Central.evaluations_per_jacobian(10), 20);
    /// assert_eq!(DiffScheme::Central4th.evaluations_per_jacobian(10), 40);
    /// ```
    #[must_use]
    pub fn evaluations_per_jacobian(self, n: usize) -> usize {
        match self {
            Self::Forward | Self::Backward => n + 1,
            Self::Central => 2 * n,
            Self::Central4th => 4 * n,
        }
    }

    /// A short human-readable label, for benchmark tables and log lines.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Forward => "forward",
            Self::Backward => "backward",
            Self::Central => "central",
            Self::Central4th => "central-4th",
        }
    }
}

/// Step-size policy for every entry point in this module.
///
/// # The step-size rule
///
/// ```text
/// h = relative_step * max(|x|, min_scale)
/// ```
///
/// The step is **relative to the magnitude of the variable being perturbed**,
/// because a step that is right for `x ~ 1` is far too small for a pressure in
/// pascals and far too large for a mole fraction. `min_scale` is the floor that
/// keeps the rule usable at `x = 0` — see [`Self::step_for`].
///
/// # Units
///
/// `relative_step` is dimensionless. `min_scale` carries the **same units as
/// the variable being differentiated with respect to**, because it is a
/// fallback magnitude for `x`, and its default of `1.0` therefore means "one of
/// whatever unit `x` is in". A caller differentiating with respect to a
/// pressure in pascals near zero wants `min_scale` set to a pascal-scale
/// number, not `1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiffSettings {
    /// Which stencil to use. See [`DiffScheme`].
    pub scheme: DiffScheme,
    /// The relative step, dimensionless. Defaults to
    /// [`DiffScheme::default_relative_step`] for the chosen scheme.
    pub relative_step: f64,
    /// Floor on `|x|` in the step rule, so `x = 0` still gets a usable step.
    /// Same units as `x`. Default `1.0`.
    pub min_scale: f64,
}

impl Default for DiffSettings {
    /// [`DiffScheme::Central4th`] with its optimal relative step and
    /// `min_scale = 1.0`.
    ///
    /// The default is the *most accurate* scheme rather than the cheapest,
    /// because a caller who has not thought about step size is far more likely
    /// to be surprised by a wrong derivative than by four function evaluations.
    fn default() -> Self {
        Self::with_scheme(DiffScheme::Central4th)
    }
}

impl DiffSettings {
    /// Settings for `scheme`, with that scheme's optimal relative step and
    /// `min_scale = 1.0`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use outram_foam_basic_lib::math::differentiate::{DiffScheme, DiffSettings};
    ///
    /// let s = DiffSettings::with_scheme(DiffScheme::Central);
    /// assert_eq!(s.relative_step, DiffScheme::Central.default_relative_step());
    /// assert_eq!(s.min_scale, 1.0);
    /// ```
    #[must_use]
    pub fn with_scheme(scheme: DiffScheme) -> Self {
        Self {
            scheme,
            relative_step: scheme.default_relative_step(),
            min_scale: 1.0,
        }
    }

    /// [`DiffScheme::Forward`] with its optimal relative step — the `n + 1`
    /// evaluation Jacobian.
    #[must_use]
    pub fn forward() -> Self {
        Self::with_scheme(DiffScheme::Forward)
    }

    /// [`DiffScheme::Backward`] with its optimal relative step.
    #[must_use]
    pub fn backward() -> Self {
        Self::with_scheme(DiffScheme::Backward)
    }

    /// [`DiffScheme::Central`] with its optimal relative step — the `2n`
    /// evaluation Jacobian, and the scheme both existing workspace
    /// implementations use.
    #[must_use]
    pub fn central() -> Self {
        Self::with_scheme(DiffScheme::Central)
    }

    /// [`DiffScheme::Central4th`] with its optimal relative step — the `4n`
    /// evaluation Jacobian. Same as [`Self::default`].
    #[must_use]
    pub fn central_4th() -> Self {
        Self::with_scheme(DiffScheme::Central4th)
    }

    /// The step this policy uses to perturb a variable currently at `x`.
    ///
    /// ```text
    /// h = relative_step * max(|x|, min_scale)
    /// ```
    ///
    /// # What happens at `x = 0`
    ///
    /// A purely relative step `relative_step * |x|` is **exactly zero** at
    /// `x = 0`, which would divide by zero and hand back `NaN` or `inf`. The
    /// `max(|x|, min_scale)` floor is what prevents that: at `x = 0` the step
    /// becomes `relative_step * min_scale`, i.e. an *absolute* step of
    /// `relative_step` in the default `min_scale = 1.0` case.
    ///
    /// This is the convention already settled elsewhere in this workspace —
    /// `outram-park-fork-offbeat`'s `newton_perturbed` uses
    /// `perturbation * x.abs().max(1.0)` for exactly this reason. It is
    /// **not** the convention `outram-park-fork-dwsim-libs`'
    /// `finite_difference_jacobian` uses; see the module-level "Provenance"
    /// section for that divergence and why.
    ///
    /// The same floor also rescues the near-zero case, which is the one that
    /// actually bites: at `x = 1e-300` a relative step is `1e-308`-ish, so
    /// `x + h` rounds straight back to `x` and the difference is identically
    /// zero. That is reported as [`DiffStatus::DegenerateStep`], not as a
    /// derivative of zero.
    ///
    /// # Returns
    ///
    /// The step, in the same units as `x`. Non-finite or non-positive results
    /// are possible if the caller supplies a nonsensical `relative_step`, and
    /// are caught by the kernels rather than by this function.
    ///
    /// # Example
    ///
    /// ```rust
    /// use outram_foam_basic_lib::math::differentiate::DiffSettings;
    ///
    /// let s = DiffSettings::central();
    /// // Relative where |x| is large ...
    /// assert_eq!(s.step_for(1000.0), s.relative_step * 1000.0);
    /// // ... absolute where it is not, so x = 0 still works.
    /// assert_eq!(s.step_for(0.0), s.relative_step);
    /// assert_eq!(s.step_for(-1000.0), s.relative_step * 1000.0);
    /// ```
    #[must_use]
    pub fn step_for(&self, x: f64) -> f64 {
        self.relative_step * x.abs().max(self.min_scale)
    }
}

// ── Status ───────────────────────────────────────────────────────────────────

/// Why a derivative or Jacobian entry is, or is not, trustworthy.
///
/// # Units
///
/// Dimensionless — a status code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiffStatus {
    /// The difference quotient was formed from finite evaluations with a
    /// non-degenerate step. The value is usable.
    Ok,
    /// The point `x` itself was not finite, so no step could be taken.
    InvalidPoint,
    /// At least one function evaluation returned a non-finite value, or the
    /// difference quotient itself came out non-finite (overflow in the
    /// subtraction, for instance).
    NotFinite,
    /// The step collapsed: `relative_step` was zero, negative or non-finite, or
    /// `x + h` rounded back to `x` so the realised step was exactly zero. The
    /// quotient would have been a division by zero.
    DegenerateStep,
    /// The vector function returned a different number of components than the
    /// point has, so the Jacobian is not square and cannot be assembled.
    ///
    /// Only reachable from [`jacobian`] and its batched form; the square
    /// restriction is documented on [`jacobian`].
    DimensionMismatch,
}

impl DiffStatus {
    /// Whether the value this status accompanies may be used.
    #[must_use]
    pub fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }

    /// A short human-readable label, for log lines and benchmark tables.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::InvalidPoint => "invalid-point",
            Self::NotFinite => "not-finite",
            Self::DegenerateStep => "degenerate-step",
            Self::DimensionMismatch => "dimension-mismatch",
        }
    }
}

// ── Scalar derivative ────────────────────────────────────────────────────────

/// One lane's scalar derivative, with the diagnostics needed to judge it.
///
/// # Units
///
/// [`value`](Self::value) carries the units of `f` divided by the units of `x`
/// — a derivative changes dimension, which is exactly why this module does not
/// try to `uom`-type the generic form. See the module-level "Units" section.
/// [`realised_step`](Self::realised_step) carries the units of `x`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DerivativeSolution {
    value: f64,
    realised_step: f64,
    status: DiffStatus,
}

impl DerivativeSolution {
    /// The derivative, **only if** this lane succeeded.
    ///
    /// Returns `None` for every non-[`DiffStatus::Ok`] status, so a caller
    /// cannot accidentally consume a `NaN` as a derivative. The diagnostic
    /// number is behind the deliberately-named [`Self::raw_value`].
    #[must_use]
    pub fn derivative(&self) -> Option<f64> {
        if self.status.is_ok() {
            Some(self.value)
        } else {
            None
        }
    }

    /// The difference quotient as computed, whatever the status — a diagnostic,
    /// **not** an answer. Frequently `NaN`.
    #[must_use]
    pub fn raw_value(&self) -> f64 {
        self.value
    }

    /// The step actually taken, after the `x + h` rounding correction described
    /// on [`derivative`]. Units of `x`.
    ///
    /// This is the denominator that was really divided by, not the `h` that
    /// [`DiffSettings::step_for`] asked for, and comparing the two is the
    /// cheapest way to see step-size trouble.
    #[must_use]
    pub fn realised_step(&self) -> f64 {
        self.realised_step
    }

    /// Why this lane succeeded or failed.
    #[must_use]
    pub fn status(&self) -> DiffStatus {
        self.status
    }

    /// Whether this lane produced a usable derivative.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.status.is_ok()
    }

    fn ok(value: f64, realised_step: f64) -> Self {
        Self {
            value,
            realised_step,
            status: DiffStatus::Ok,
        }
    }

    fn failed(status: DiffStatus, realised_step: f64) -> Self {
        Self {
            value: f64::NAN,
            realised_step,
            status,
        }
    }
}

/// One or more lanes of a [`DerivativeBatch`] or [`JacobianBatch`] failed.
///
/// Returned by the all-or-nothing accessors [`DerivativeBatch::values`] and
/// [`JacobianBatch::matrices`]. It names both the scale of the problem (how
/// many of how many) and a specific lane to look at, because "3 of 10 000 lanes
/// failed" is only actionable once you know *which* lane.
///
/// # Units
///
/// All counts and indices are dimensionless.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "{failure_count} of {total} differentiation lanes failed; \
     first failure at lane {first_index} with status {first_status:?}"
)]
pub struct DiffBatchFailure {
    /// Number of lanes in the batch.
    pub total: usize,
    /// Number of lanes that failed.
    pub failure_count: usize,
    /// Index of the first failing lane.
    pub first_index: usize,
    /// Why that lane failed.
    pub first_status: DiffStatus,
}

/// The result of [`derivative_batch`] — one [`DerivativeSolution`] per point,
/// in point order.
#[derive(Debug, Clone, PartialEq)]
pub struct DerivativeBatch {
    solutions: Vec<DerivativeSolution>,
}

impl DerivativeBatch {
    /// Every lane's solution, in the order the points were supplied.
    #[must_use]
    pub fn solutions(&self) -> &[DerivativeSolution] {
        &self.solutions
    }

    /// Consume the batch, yielding the per-lane solutions.
    #[must_use]
    pub fn into_solutions(self) -> Vec<DerivativeSolution> {
        self.solutions
    }

    /// Number of lanes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.solutions.len()
    }

    /// Whether the batch has no lanes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.solutions.is_empty()
    }

    /// Lane `i`'s solution, or `None` if `i` is out of range.
    #[must_use]
    pub fn get(&self, i: usize) -> Option<DerivativeSolution> {
        self.solutions.get(i).copied()
    }

    /// Whether every lane produced a usable derivative.
    #[must_use]
    pub fn all_ok(&self) -> bool {
        self.solutions.iter().all(DerivativeSolution::is_ok)
    }

    /// How many lanes failed.
    #[must_use]
    pub fn failure_count(&self) -> usize {
        self.solutions.iter().filter(|s| !s.is_ok()).count()
    }

    /// The first failing lane and its solution, if any.
    #[must_use]
    pub fn first_failure(&self) -> Option<(usize, DerivativeSolution)> {
        self.solutions
            .iter()
            .enumerate()
            .find(|(_, s)| !s.is_ok())
            .map(|(i, s)| (i, *s))
    }

    /// Every failing lane and its solution.
    #[must_use]
    pub fn failures(&self) -> Vec<(usize, DerivativeSolution)> {
        self.solutions
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.is_ok())
            .map(|(i, s)| (i, *s))
            .collect()
    }

    /// Every lane's derivative, **all or nothing**.
    ///
    /// # Errors
    ///
    /// [`DiffBatchFailure`] naming the failure count and the first failing lane
    /// if any lane failed. A partially-`NaN` `Vec<f64>` is never returned.
    pub fn values(&self) -> Result<Vec<f64>, DiffBatchFailure> {
        self.check_all_ok()?;
        Ok(self.solutions.iter().map(|s| s.value).collect())
    }

    /// `Err` describing the first failure, if any lane failed.
    ///
    /// # Errors
    ///
    /// [`DiffBatchFailure`] as for [`Self::values`].
    pub fn check_all_ok(&self) -> Result<(), DiffBatchFailure> {
        if let Some((i, s)) = self.first_failure() {
            return Err(DiffBatchFailure {
                total: self.solutions.len(),
                failure_count: self.failure_count(),
                first_index: i,
                first_status: s.status(),
            });
        }
        Ok(())
    }
}

/// Differentiate one scalar function at one point.
///
/// The single-lane form of [`derivative_batch`], for callers with one
/// derivative to take. It runs on the calling thread — there is nothing to
/// spread — and calls the *same* per-lane kernel, so it agrees with a
/// one-element batch bit for bit.
///
/// # The realised-step correction
///
/// `x + h` is generally not representable, so the value the machine actually
/// evaluates at differs from `x + h` in the last bits and the true step is not
/// `h`. This kernel therefore evaluates at `xp = x + h` and divides by
/// `xp - x`, which **is** exact, rather than by `h`. The device is from
/// *Numerical Recipes* (Press et al., 3rd ed., section 5.7) and it removes an
/// error source that would otherwise be comparable to the round-off term the
/// step rule is trying to balance. [`DerivativeSolution::realised_step`]
/// reports the corrected denominator.
///
/// # Arguments
///
/// - `x` — the point, in the caller's own units.
/// - `settings` — scheme and step-size policy; see [`DiffSettings`].
/// - `f` — the function. Units of the return value are the caller's.
///
/// # Returns
///
/// A [`DerivativeSolution`] whose [`derivative`](DerivativeSolution::derivative)
/// is `Some` only if every evaluation was finite and the step was
/// non-degenerate.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::math::differentiate::{derivative, DiffSettings};
///
/// // d/dx sin(x) at x = 1 is cos(1).
/// let s = derivative(1.0, DiffSettings::central(), |x: f64| x.sin());
/// let d = s.derivative().expect("finite everywhere");
/// assert!((d - 1.0_f64.cos()).abs() < 1e-10, "got {d}");
///
/// // A function that blows up is reported, not silently returned as NaN.
/// let bad = derivative(0.0, DiffSettings::central(), |x: f64| 1.0 / x);
/// assert!(bad.derivative().is_none());
/// ```
#[must_use]
pub fn derivative<F>(x: f64, settings: DiffSettings, f: F) -> DerivativeSolution
where
    F: Fn(f64) -> f64,
{
    derivative_one(0, x, settings, &|_, t| f(t))
}

/// Differentiate `N` independent scalar functions at `N` points, on the chosen
/// backend.
///
/// This is the batched, GPU-shaped form: lane `i` differentiates `f(i, .)` at
/// `points[i]`, and no arithmetic crosses lanes.
///
/// # Arguments
///
/// - `points` — one abscissa per lane, in the caller's own units.
/// - `settings` — scheme and step-size policy, shared by every lane.
/// - `backend` — requested execution backend. What actually runs is
///   [`derivative_backend_for`] applied to it. **None of the degradations
///   changes the answer.**
/// - `f` — `f(i, x)` is lane `i`'s function evaluated at `x`. It **must be a
///   pure deterministic function of its arguments** — see the module-level
///   "Determinism" section. The `Sync` bound is present in both feature builds
///   so enabling `parallel` never changes a public signature.
///
/// # Returns
///
/// A [`DerivativeBatch`] with one solution per point, in point order. An empty
/// `points` slice returns an empty batch and calls `f` zero times.
///
/// # Cost
///
/// [`DiffScheme::evaluations_per_derivative`] calls to `f` per lane — 2 for the
/// three second-order-or-lower schemes, 4 for [`DiffScheme::Central4th`].
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::differentiate::{
///     derivative_batch, DiffSettings,
/// };
///
/// // Lane i differentiates x^(i+1) at x = 2; the answer is (i+1) * 2^i.
/// let points = vec![2.0_f64; 4];
/// let batch = derivative_batch(
///     &points,
///     DiffSettings::central(),
///     ComputeBackend::CpuMulti,
///     |i, x: f64| x.powi(i as i32 + 1),
/// );
///
/// let d = batch.values().expect("all lanes finite");
/// for (i, got) in d.iter().enumerate() {
///     let want = (i as f64 + 1.0) * 2.0_f64.powi(i as i32);
///     assert!((got - want).abs() < 1e-6 * want.abs().max(1.0), "lane {i}: {got} vs {want}");
/// }
/// ```
#[must_use]
pub fn derivative_batch<F>(
    points: &[f64],
    settings: DiffSettings,
    backend: ComputeBackend,
    f: F,
) -> DerivativeBatch
where
    F: Fn(usize, f64) -> f64 + Sync,
{
    derivative_batch_min(points, settings, backend, DERIVATIVE_BATCH_MIN_POINTS, f)
}

/// [`derivative_batch`] with the size floor supplied by the caller.
///
/// Exists so the crossover benchmark can measure the multi-CPU path *below*
/// [`DERIVATIVE_BATCH_MIN_POINTS`] — the only way to find where the crossover
/// actually is — and so the cross-backend bitwise tests are not vacuous on
/// small batches. Not public: production callers get the measured floor.
pub(crate) fn derivative_batch_min<F>(
    points: &[f64],
    settings: DiffSettings,
    backend: ComputeBackend,
    min_points: usize,
    f: F,
) -> DerivativeBatch
where
    F: Fn(usize, f64) -> f64 + Sync,
{
    let n = points.len();
    let solutions: Vec<DerivativeSolution> = match effective_backend(backend, n, min_points) {
        #[cfg(feature = "parallel")]
        ComputeBackend::CpuMulti => points
            .par_iter()
            .enumerate()
            .map(|(i, &x)| derivative_one(i, x, settings, &f))
            .collect(),
        _ => points
            .iter()
            .enumerate()
            .map(|(i, &x)| derivative_one(i, x, settings, &f))
            .collect(),
    };
    DerivativeBatch { solutions }
}

// ── Per-lane scalar kernel — one implementation, both backends ───────────────

/// The single-lane finite difference that **both** backends run.
///
/// `#[inline]` so the serial loop and the `rayon` map compile to the same inner
/// code — part of why the two backends agree bit for bit.
#[inline]
fn derivative_one<F>(i: usize, x: f64, settings: DiffSettings, f: &F) -> DerivativeSolution
where
    F: Fn(usize, f64) -> f64,
{
    if !x.is_finite() {
        return DerivativeSolution::failed(DiffStatus::InvalidPoint, f64::NAN);
    }
    let h = settings.step_for(x);
    if !h.is_finite() || h <= 0.0 {
        return DerivativeSolution::failed(DiffStatus::DegenerateStep, h);
    }

    match settings.scheme {
        DiffScheme::Forward => one_sided(i, x, h, f),
        DiffScheme::Backward => one_sided(i, x, -h, f),
        DiffScheme::Central => central(i, x, h, f),
        DiffScheme::Central4th => {
            // Richardson extrapolation of two central differences.
            // D(h) has error c2*h^2 + c4*h^4 + ...; D(h/2) has c2*h^2/4 + ...
            // so (4*D(h/2) - D(h)) / 3 cancels the h^2 term exactly.
            let coarse = central(i, x, h, f);
            if !coarse.is_ok() {
                return coarse;
            }
            let fine = central(i, x, 0.5 * h, f);
            if !fine.is_ok() {
                return fine;
            }
            let value = (4.0 * fine.value - coarse.value) / 3.0;
            if value.is_finite() {
                DerivativeSolution::ok(value, fine.realised_step)
            } else {
                DerivativeSolution::failed(DiffStatus::NotFinite, fine.realised_step)
            }
        }
    }
}

/// Forward (`h > 0`) or backward (`h < 0`) difference at `x`.
#[inline]
fn one_sided<F>(i: usize, x: f64, h: f64, f: &F) -> DerivativeSolution
where
    F: Fn(usize, f64) -> f64,
{
    let xp = x + h;
    // `xp - x` is exact and is the step the machine really took; `h` is not.
    let dh = xp - x;
    if dh == 0.0 || !dh.is_finite() {
        return DerivativeSolution::failed(DiffStatus::DegenerateStep, dh);
    }
    let f0 = f(i, x);
    let f1 = f(i, xp);
    if !f0.is_finite() || !f1.is_finite() {
        return DerivativeSolution::failed(DiffStatus::NotFinite, dh);
    }
    let value = (f1 - f0) / dh;
    if value.is_finite() {
        DerivativeSolution::ok(value, dh)
    } else {
        DerivativeSolution::failed(DiffStatus::NotFinite, dh)
    }
}

/// Central difference at `x` with half-width `h`.
#[inline]
fn central<F>(i: usize, x: f64, h: f64, f: &F) -> DerivativeSolution
where
    F: Fn(usize, f64) -> f64,
{
    let xp = x + h;
    let xm = x - h;
    let dh = xp - xm;
    if dh == 0.0 || !dh.is_finite() {
        return DerivativeSolution::failed(DiffStatus::DegenerateStep, dh);
    }
    let fp = f(i, xp);
    let fm = f(i, xm);
    if !fp.is_finite() || !fm.is_finite() {
        return DerivativeSolution::failed(DiffStatus::NotFinite, dh);
    }
    let value = (fp - fm) / dh;
    if value.is_finite() {
        DerivativeSolution::ok(value, dh)
    } else {
        DerivativeSolution::failed(DiffStatus::NotFinite, dh)
    }
}

// ── Jacobians ────────────────────────────────────────────────────────────────

/// One lane's Jacobian, with the status needed to judge it.
///
/// # Units
///
/// Entry `(i, j)` of the matrix carries the units of `f_i` divided by the units
/// of `x_j`. See the module-level "Units" section for why the generic form is
/// not `uom`-typed.
#[derive(Debug, Clone)]
pub struct JacobianSolution {
    matrix: SquareMatrix,
    status: DiffStatus,
    first_bad_column: usize,
}

impl JacobianSolution {
    /// The Jacobian, **only if** every column of this lane succeeded.
    ///
    /// Returns `None` for every non-[`DiffStatus::Ok`] status, so a caller
    /// cannot accidentally factorise a partially-`NaN` matrix believing it to
    /// be a Jacobian. The diagnostic matrix is behind the deliberately-named
    /// [`Self::raw_matrix`].
    #[must_use]
    pub fn matrix(&self) -> Option<&SquareMatrix> {
        if self.status.is_ok() {
            Some(&self.matrix)
        } else {
            None
        }
    }

    /// Consume the solution, yielding the Jacobian only if it succeeded.
    #[must_use]
    pub fn into_matrix(self) -> Option<SquareMatrix> {
        if self.status.is_ok() {
            Some(self.matrix)
        } else {
            None
        }
    }

    /// The matrix as assembled, whatever the status — a diagnostic, **not** an
    /// answer. Failed columns are filled with `NaN`.
    #[must_use]
    pub fn raw_matrix(&self) -> &SquareMatrix {
        &self.matrix
    }

    /// Why this lane succeeded or failed.
    #[must_use]
    pub fn status(&self) -> DiffStatus {
        self.status
    }

    /// Whether this lane produced a usable Jacobian.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.status.is_ok()
    }

    /// The index of the first column that failed, or `usize::MAX` if none did.
    ///
    /// "Column `j`" means the derivative with respect to `x[j]`, so this points
    /// straight at the offending *variable* rather than at the offending
    /// equation — which is the useful direction, because a failed column
    /// usually means the step took `x[j]` somewhere the function is not
    /// defined.
    #[must_use]
    pub fn first_bad_column(&self) -> usize {
        self.first_bad_column
    }
}

/// The result of [`jacobian_batch`] — one [`JacobianSolution`] per lane.
#[derive(Debug, Clone)]
pub struct JacobianBatch {
    solutions: Vec<JacobianSolution>,
}

impl JacobianBatch {
    /// Every lane's solution, in the order the points were supplied.
    #[must_use]
    pub fn solutions(&self) -> &[JacobianSolution] {
        &self.solutions
    }

    /// Consume the batch, yielding the per-lane solutions.
    #[must_use]
    pub fn into_solutions(self) -> Vec<JacobianSolution> {
        self.solutions
    }

    /// Number of lanes.
    #[must_use]
    pub fn len(&self) -> usize {
        self.solutions.len()
    }

    /// Whether the batch has no lanes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.solutions.is_empty()
    }

    /// Lane `i`'s solution, or `None` if `i` is out of range.
    #[must_use]
    pub fn get(&self, i: usize) -> Option<&JacobianSolution> {
        self.solutions.get(i)
    }

    /// Whether every lane produced a usable Jacobian.
    #[must_use]
    pub fn all_ok(&self) -> bool {
        self.solutions.iter().all(JacobianSolution::is_ok)
    }

    /// How many lanes failed.
    #[must_use]
    pub fn failure_count(&self) -> usize {
        self.solutions.iter().filter(|s| !s.is_ok()).count()
    }

    /// The first failing lane index and its status, if any.
    #[must_use]
    pub fn first_failure(&self) -> Option<(usize, DiffStatus)> {
        self.solutions
            .iter()
            .enumerate()
            .find(|(_, s)| !s.is_ok())
            .map(|(i, s)| (i, s.status()))
    }

    /// Every lane's Jacobian, **all or nothing**.
    ///
    /// # Errors
    ///
    /// [`DiffBatchFailure`] naming the failure count and the first failing lane
    /// if any lane failed. A `Vec` containing a partially-`NaN` matrix is never
    /// returned.
    pub fn matrices(self) -> Result<Vec<SquareMatrix>, DiffBatchFailure> {
        self.check_all_ok()?;
        Ok(self.solutions.into_iter().map(|s| s.matrix).collect())
    }

    /// `Err` describing the first failure, if any lane failed.
    ///
    /// # Errors
    ///
    /// [`DiffBatchFailure`] as for [`Self::matrices`].
    pub fn check_all_ok(&self) -> Result<(), DiffBatchFailure> {
        if let Some((i, status)) = self.first_failure() {
            return Err(DiffBatchFailure {
                total: self.solutions.len(),
                failure_count: self.failure_count(),
                first_index: i,
                first_status: status,
            });
        }
        Ok(())
    }
}

/// Assemble the Jacobian `J[i][j] = d f_i / d x_j` of one `n`-dimensional
/// vector function at one point, by finite differences.
///
/// The direct feeder for multi-dimensional Newton and — through
/// [`NumericalJacobian`] — for [`crate::ode::Rosenbrock23`].
///
/// # Square only
///
/// `f` must return exactly `x.len()` components. A rectangular Jacobian is
/// rejected with [`DiffStatus::DimensionMismatch`] rather than silently padded.
/// This restriction is deliberate and matches the prior art: the consumer
/// (`n` ODE equations in `n` states) is square, the crate's [`SquareMatrix`] is
/// square, and `outram-park-fork-dwsim-libs`' `finite_difference_jacobian`
/// rejects the non-square case too.
///
/// # Arguments
///
/// - `x` — the point, one component per variable, in the caller's own units.
/// - `settings` — scheme and step-size policy; see [`DiffSettings`]. The step
///   is computed per-column from that column's own `x[j]`, so variables of
///   wildly different magnitude each get an appropriate step.
/// - `backend` — requested backend for spreading the **columns** of this one
///   Jacobian. What actually runs is [`jacobian_column_backend_for`] applied to
///   it; a small `n` runs serially. **None of the degradations changes the
///   answer.**
/// - `f` — `f(0, x, out)` must fill `out` with the `n` function components at
///   `x`. The lane index is always `0` here; it is in the signature so the same
///   closure works with [`jacobian_batch`]. It **must be a pure deterministic
///   function of its arguments**.
///
/// # Returns
///
/// A [`JacobianSolution`] whose [`matrix`](JacobianSolution::matrix) is `Some`
/// only if every column succeeded.
///
/// # Cost
///
/// [`DiffScheme::evaluations_per_jacobian`] calls to `f`: `n + 1` for
/// [`DiffScheme::Forward`]/[`DiffScheme::Backward`], `2n` for
/// [`DiffScheme::Central`], `4n` for [`DiffScheme::Central4th`].
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::differentiate::{jacobian, DiffSettings};
///
/// // f(x, y) = [x^2 * y, sin(x) + y^3]
/// // J = [[2xy, x^2], [cos(x), 3y^2]]
/// let point = [1.5_f64, 2.0];
/// let s = jacobian(
///     &point,
///     DiffSettings::central(),
///     ComputeBackend::Serial,
///     |_, v: &[f64], out: &mut Vec<f64>| {
///         out.push(v[0] * v[0] * v[1]);
///         out.push(v[0].sin() + v[1] * v[1] * v[1]);
///     },
/// );
///
/// let j = s.matrix().expect("smooth everywhere");
/// let (x, y) = (point[0], point[1]);
/// for (got, want) in [
///     (j.get(0, 0), 2.0 * x * y),
///     (j.get(0, 1), x * x),
///     (j.get(1, 0), x.cos()),
///     (j.get(1, 1), 3.0 * y * y),
/// ] {
///     assert!((got - want).abs() < 1e-8 * want.abs().max(1.0), "{got} vs {want}");
/// }
/// ```
#[must_use]
pub fn jacobian<F>(
    x: &[f64],
    settings: DiffSettings,
    backend: ComputeBackend,
    f: F,
) -> JacobianSolution
where
    F: Fn(usize, &[f64], &mut Vec<f64>) + Sync,
{
    jacobian_columns_min(0, x, settings, backend, JACOBIAN_COLUMN_MIN_DIMENSION, &f)
}

/// Assemble `N` independent Jacobians, one per lane, on the chosen backend.
///
/// This is the batched form: the parallel axis is the **lane**, not the column,
/// so it is the right entry point when there are many small Jacobians (a
/// per-cell chemistry Jacobian over a mesh, an ensemble of ODE systems). Use
/// [`jacobian`] when there is one large Jacobian instead.
///
/// # The flat point layout
///
/// `points` is a **flat, row-major** buffer of `lanes * n` values: lane `i`'s
/// point is `points[i * n .. (i + 1) * n]`. A `&[Vec<f64>]` would be the
/// obvious alternative and is rejected on purpose — it costs one allocation and
/// one pointer chase per lane, and it is not the layout a GPU buffer would ever
/// take. `points.len()` must be an exact multiple of `n`.
///
/// # Arguments
///
/// - `points` — flat `lanes * n` buffer as above, in the caller's own units.
/// - `n` — the dimension of each point, dimensionless. Must be non-zero.
/// - `settings` — scheme and step-size policy, shared by every lane.
/// - `backend` — requested backend; see [`jacobian_batch_backend_for`]. Each
///   lane's columns are computed serially, since the lane axis is already the
///   parallel one.
/// - `f` — `f(i, x, out)` must fill `out` with lane `i`'s `n` function
///   components at `x`. It **must be a pure deterministic function of its
///   arguments**.
///
/// # Returns
///
/// A [`JacobianBatch`] with one solution per lane, in lane order. An empty
/// `points` slice, or `n == 0`, returns an empty batch and calls `f` zero
/// times. A `points.len()` that is not a multiple of `n` returns an empty batch
/// as well — it is a caller bug, not a numerical failure, and there is no
/// sensible lane count to report per-lane statuses against.
///
/// # Cost
///
/// `lanes * `[`DiffScheme::evaluations_per_jacobian`]`(n)` calls to `f`.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::compute::ComputeBackend;
/// use outram_foam_basic_lib::math::differentiate::{jacobian_batch, DiffSettings};
///
/// // 3 lanes of the 2-D rotation-like system f = [-k_i * y, k_i * x],
/// // whose Jacobian is [[0, -k_i], [k_i, 0]].
/// let k = [1.0_f64, 2.5, 7.0];
/// let points: Vec<f64> = vec![0.3, -0.7, 0.3, -0.7, 0.3, -0.7];
///
/// let batch = jacobian_batch(
///     &points,
///     2,
///     DiffSettings::central(),
///     ComputeBackend::CpuMulti,
///     |i, v: &[f64], out: &mut Vec<f64>| {
///         out.push(-k[i] * v[1]);
///         out.push(k[i] * v[0]);
///     },
/// );
///
/// let mats = batch.matrices().expect("linear system, exact everywhere");
/// for (i, m) in mats.iter().enumerate() {
///     assert!((m.get(0, 1) + k[i]).abs() < 1e-9);
///     assert!((m.get(1, 0) - k[i]).abs() < 1e-9);
/// }
/// ```
#[must_use]
pub fn jacobian_batch<F>(
    points: &[f64],
    n: usize,
    settings: DiffSettings,
    backend: ComputeBackend,
    f: F,
) -> JacobianBatch
where
    F: Fn(usize, &[f64], &mut Vec<f64>) + Sync,
{
    jacobian_batch_min(
        points,
        n,
        settings,
        backend,
        JACOBIAN_BATCH_MIN_PROBLEMS,
        f,
    )
}

/// [`jacobian_batch`] with the size floor supplied by the caller.
///
/// Exists so the crossover benchmark can measure the multi-CPU path *below*
/// [`JACOBIAN_BATCH_MIN_PROBLEMS`], and so the cross-backend bitwise tests are
/// not vacuous on small batches. Not public.
pub(crate) fn jacobian_batch_min<F>(
    points: &[f64],
    n: usize,
    settings: DiffSettings,
    backend: ComputeBackend,
    min_problems: usize,
    f: F,
) -> JacobianBatch
where
    F: Fn(usize, &[f64], &mut Vec<f64>) + Sync,
{
    if n == 0 || points.is_empty() || !points.len().is_multiple_of(n) {
        return JacobianBatch {
            solutions: Vec::new(),
        };
    }
    let lanes = points.len() / n;
    // Per-lane columns always run serially: the lane axis is the parallel one,
    // and nesting a rayon map inside a rayon map only fights the same pool.
    let solutions: Vec<JacobianSolution> = match effective_backend(backend, lanes, min_problems) {
        #[cfg(feature = "parallel")]
        ComputeBackend::CpuMulti => points
            .par_chunks(n)
            .enumerate()
            .map(|(i, x)| {
                jacobian_columns_min(i, x, settings, ComputeBackend::Serial, usize::MAX, &f)
            })
            .collect(),
        _ => points
            .chunks(n)
            .enumerate()
            .map(|(i, x)| {
                jacobian_columns_min(i, x, settings, ComputeBackend::Serial, usize::MAX, &f)
            })
            .collect(),
    };
    JacobianBatch { solutions }
}

/// Assemble one lane's Jacobian, spreading the columns across `backend` when
/// the dimension justifies it.
///
/// Delegates to [`jacobian_columns_serial`] for everything except the `rayon`
/// arm, and both arms call the same [`jacobian_column`] kernel — which is why
/// every public Jacobian entry point agrees bit for bit.
fn jacobian_columns_min<F>(
    lane: usize,
    x: &[f64],
    settings: DiffSettings,
    backend: ComputeBackend,
    min_dimension: usize,
    f: &F,
) -> JacobianSolution
where
    F: Fn(usize, &[f64], &mut Vec<f64>) + Sync,
{
    match effective_backend(backend, x.len(), min_dimension) {
        #[cfg(feature = "parallel")]
        ComputeBackend::CpuMulti => {
            let n = x.len();
            let base = match jacobian_base(lane, x, settings, f) {
                Ok(b) => b,
                Err(status) => return failed_jacobian(n, status, 0),
            };
            if !x.iter().all(|v| v.is_finite()) {
                return failed_jacobian(n, DiffStatus::InvalidPoint, 0);
            }
            let base_slice = base.as_deref();
            let columns: Vec<Result<Vec<f64>, DiffStatus>> = (0..n)
                .into_par_iter()
                .map(|j| jacobian_column(lane, x, j, settings, base_slice, f))
                .collect();
            assemble_columns(n, columns)
        }
        _ => jacobian_columns_serial(lane, x, settings, f),
    }
}

/// One lane's Jacobian, columns computed on the calling thread.
///
/// Carries **no** `Sync` bound, which is what lets [`ode_system_jacobian`]
/// difference an `OdeSystem` that is not itself `Sync`.
fn jacobian_columns_serial<F>(
    lane: usize,
    x: &[f64],
    settings: DiffSettings,
    f: &F,
) -> JacobianSolution
where
    F: Fn(usize, &[f64], &mut Vec<f64>),
{
    let n = x.len();
    if n == 0 {
        return JacobianSolution {
            matrix: SquareMatrix::new(0),
            status: DiffStatus::Ok,
            first_bad_column: usize::MAX,
        };
    }
    if !x.iter().all(|v| v.is_finite()) {
        return failed_jacobian(n, DiffStatus::InvalidPoint, 0);
    }
    let base = match jacobian_base(lane, x, settings, f) {
        Ok(b) => b,
        Err(status) => return failed_jacobian(n, status, 0),
    };
    let base_slice = base.as_deref();
    let columns: Vec<Result<Vec<f64>, DiffStatus>> = (0..n)
        .map(|j| jacobian_column(lane, x, j, settings, base_slice, f))
        .collect();
    assemble_columns(n, columns)
}

/// The unperturbed evaluation `f(x)`, shared by every column of a one-sided
/// scheme — which is what makes [`DiffScheme::Forward`] cost `n + 1`
/// evaluations rather than `2n`. `None` for the symmetric schemes, whose
/// stencils never touch `x` itself.
fn jacobian_base<F>(
    lane: usize,
    x: &[f64],
    settings: DiffSettings,
    f: &F,
) -> Result<Option<Vec<f64>>, DiffStatus>
where
    F: Fn(usize, &[f64], &mut Vec<f64>),
{
    match settings.scheme {
        DiffScheme::Forward | DiffScheme::Backward => {
            let n = x.len();
            let mut out = Vec::with_capacity(n);
            f(lane, x, &mut out);
            if out.len() != n {
                return Err(DiffStatus::DimensionMismatch);
            }
            if !out.iter().all(|v| v.is_finite()) {
                return Err(DiffStatus::NotFinite);
            }
            Ok(Some(out))
        }
        DiffScheme::Central | DiffScheme::Central4th => Ok(None),
    }
}

/// Pack per-column results into a [`SquareMatrix`], filling failed columns with
/// `NaN` and recording the first failure.
fn assemble_columns(n: usize, columns: Vec<Result<Vec<f64>, DiffStatus>>) -> JacobianSolution {
    let mut matrix = SquareMatrix::new(n);
    let mut status = DiffStatus::Ok;
    let mut first_bad_column = usize::MAX;
    for (j, column) in columns.into_iter().enumerate() {
        match column {
            Ok(values) => {
                for (i, v) in values.into_iter().enumerate() {
                    matrix.set(i, j, v);
                }
            }
            Err(bad) => {
                for i in 0..n {
                    matrix.set(i, j, f64::NAN);
                }
                if status.is_ok() {
                    status = bad;
                    first_bad_column = j;
                }
            }
        }
    }
    JacobianSolution {
        matrix,
        status,
        first_bad_column,
    }
}

/// An all-`NaN` Jacobian carrying the reason it could not be formed.
fn failed_jacobian(n: usize, status: DiffStatus, first_bad_column: usize) -> JacobianSolution {
    let mut matrix = SquareMatrix::new(n);
    for i in 0..n {
        for j in 0..n {
            matrix.set(i, j, f64::NAN);
        }
    }
    JacobianSolution {
        matrix,
        status,
        first_bad_column,
    }
}

/// Column `j` of the Jacobian: `d f / d x_j`, all `n` components at once.
///
/// `base` carries the shared unperturbed evaluation for the one-sided schemes
/// and is `None` for the symmetric ones.
#[inline]
fn jacobian_column<F>(
    lane: usize,
    x: &[f64],
    j: usize,
    settings: DiffSettings,
    base: Option<&[f64]>,
    f: &F,
) -> Result<Vec<f64>, DiffStatus>
where
    F: Fn(usize, &[f64], &mut Vec<f64>),
{
    let n = x.len();
    let h = settings.step_for(x[j]);
    if !h.is_finite() || h <= 0.0 {
        return Err(DiffStatus::DegenerateStep);
    }

    match settings.scheme {
        DiffScheme::Forward => one_sided_column(lane, x, j, h, base, f),
        DiffScheme::Backward => one_sided_column(lane, x, j, -h, base, f),
        DiffScheme::Central => central_column(lane, x, j, h, f),
        DiffScheme::Central4th => {
            let coarse = central_column(lane, x, j, h, f)?;
            let fine = central_column(lane, x, j, 0.5 * h, f)?;
            let mut out = Vec::with_capacity(n);
            for (c, fi) in coarse.into_iter().zip(fine) {
                let v = (4.0 * fi - c) / 3.0;
                if !v.is_finite() {
                    return Err(DiffStatus::NotFinite);
                }
                out.push(v);
            }
            Ok(out)
        }
    }
}

/// Forward (`h > 0`) or backward (`h < 0`) column, reusing the shared base.
#[inline]
fn one_sided_column<F>(
    lane: usize,
    x: &[f64],
    j: usize,
    h: f64,
    base: Option<&[f64]>,
    f: &F,
) -> Result<Vec<f64>, DiffStatus>
where
    F: Fn(usize, &[f64], &mut Vec<f64>),
{
    let n = x.len();
    let base = base.ok_or(DiffStatus::DegenerateStep)?;
    let mut xp = x.to_vec();
    xp[j] = x[j] + h;
    let dh = xp[j] - x[j];
    if dh == 0.0 || !dh.is_finite() {
        return Err(DiffStatus::DegenerateStep);
    }
    let mut fp = Vec::with_capacity(n);
    f(lane, &xp, &mut fp);
    if fp.len() != n {
        return Err(DiffStatus::DimensionMismatch);
    }
    let mut out = Vec::with_capacity(n);
    for (a, b) in fp.into_iter().zip(base) {
        let v = (a - b) / dh;
        if !v.is_finite() {
            return Err(DiffStatus::NotFinite);
        }
        out.push(v);
    }
    Ok(out)
}

/// Central column with half-width `h`.
#[inline]
fn central_column<F>(
    lane: usize,
    x: &[f64],
    j: usize,
    h: f64,
    f: &F,
) -> Result<Vec<f64>, DiffStatus>
where
    F: Fn(usize, &[f64], &mut Vec<f64>),
{
    let n = x.len();
    // One probe buffer, reused for both stencil points -- the two evaluations
    // differ in a single component, so a second copy of `x` buys nothing.
    let mut probe = x.to_vec();
    probe[j] = x[j] + h;
    let plus = probe[j];
    probe[j] = x[j] - h;
    let minus = probe[j];
    let dh = plus - minus;
    if dh == 0.0 || !dh.is_finite() {
        return Err(DiffStatus::DegenerateStep);
    }
    let mut fm = Vec::with_capacity(n);
    f(lane, &probe, &mut fm);
    probe[j] = plus;
    let mut fp = Vec::with_capacity(n);
    f(lane, &probe, &mut fp);
    if fp.len() != n || fm.len() != n {
        return Err(DiffStatus::DimensionMismatch);
    }
    let mut out = Vec::with_capacity(n);
    for (a, b) in fp.into_iter().zip(fm) {
        let v = (a - b) / dh;
        if !v.is_finite() {
            return Err(DiffStatus::NotFinite);
        }
        out.push(v);
    }
    Ok(out)
}

// ── The ODE consumer ─────────────────────────────────────────────────────────

/// Fill an [`OdeSystem`]'s Jacobian slots by finite differences.
///
/// This is the free-function form of what [`NumericalJacobian`] does, for
/// callers who already have an `OdeSystem` and want the numbers rather than a
/// wrapper. It fills exactly the two buffers
/// [`OdeSystem::jacobian`] is contracted to fill:
///
/// - `dfdy[i][j] = d f_i / d y_j`, an `n x n` [`SquareMatrix`];
/// - `dfdx[i] = d f_i / d x`, the derivative with respect to the **independent
///   variable** (time, for a transient), length `n`.
///
/// Both are resized to `n = system.n_eqns()` if the caller's buffers are the
/// wrong size, so it is safe to pass freshly-defaulted ones.
///
/// # Failure is written into the buffers, not swallowed
///
/// On failure the offending entries are filled with `NaN` **and** the reason is
/// returned. Nothing is quietly zeroed. This matters: filling a failed entry
/// with `0.0` — which
/// `outram-park-fork-dwsim-libs`' `finite_difference_jacobian` does — turns an
/// undetectable-at-the-call-site failure into a plausible-looking Jacobian, and
/// a Rosenbrock step built on it produces a wrong trajectory rather than an
/// error. With `NaN` the failure propagates into the step, the normalised error
/// estimate becomes `NaN`, the step controller shrinks `dx` and
/// [`crate::ode::OdeError::StepSizeUnderflow`] is reported. Loud is better.
///
/// # Arguments
///
/// - `system` — the ODE system whose [`OdeSystem::derivatives`] is sampled.
/// - `x` — the independent variable, caller's units.
/// - `y` — the state, length `system.n_eqns()`.
/// - `settings` — scheme and step-size policy; see [`DiffSettings`].
/// - `dfdx`, `dfdy` — output buffers, filled in place.
///
/// # Returns
///
/// [`DiffStatus::Ok`] if every entry of both outputs was formed from finite
/// evaluations; the first failing status otherwise.
///
/// # Cost
///
/// `1 + `[`DiffScheme::evaluations_per_jacobian`]`(n)` calls to
/// [`OdeSystem::derivatives`] for the one-sided schemes — the base evaluation
/// is shared between `dfdy`'s columns and `dfdx` — and
/// [`DiffScheme::evaluations_per_jacobian`]`(n) + 2` or `+ 4` for the symmetric
/// ones.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::math::differentiate::{
///     ode_system_jacobian, DiffSettings, DiffStatus,
/// };
/// use outram_foam_basic_lib::matrix::SquareMatrix;
/// use outram_foam_basic_lib::ode::OdeSystem;
///
/// // dy/dx = [-2*y0 + y1, x * y0] -- Jacobian [[-2, 1], [x, 0]], dfdx = [0, y0].
/// struct Linear;
/// impl OdeSystem for Linear {
///     fn n_eqns(&self) -> usize { 2 }
///     fn derivatives(&self, x: f64, y: &[f64], dydx: &mut Vec<f64>) {
///         dydx.clear();
///         dydx.push(-2.0 * y[0] + y[1]);
///         dydx.push(x * y[0]);
///     }
/// }
///
/// let mut dfdx = Vec::new();
/// let mut dfdy = SquareMatrix::new(2);
/// let status = ode_system_jacobian(
///     &Linear, 0.5, &[1.0, 2.0], DiffSettings::central(), &mut dfdx, &mut dfdy,
/// );
///
/// assert_eq!(status, DiffStatus::Ok);
/// assert!((dfdy.get(0, 0) + 2.0).abs() < 1e-8);
/// assert!((dfdy.get(0, 1) - 1.0).abs() < 1e-8);
/// assert!((dfdy.get(1, 0) - 0.5).abs() < 1e-8);
/// assert!(dfdy.get(1, 1).abs() < 1e-8);
/// assert!(dfdx[0].abs() < 1e-8);
/// assert!((dfdx[1] - 1.0).abs() < 1e-8); // d/dx (x * y0) = y0 = 1
/// ```
pub fn ode_system_jacobian<S>(
    system: &S,
    x: f64,
    y: &[f64],
    settings: DiffSettings,
    dfdx: &mut Vec<f64>,
    dfdy: &mut SquareMatrix,
) -> DiffStatus
where
    S: OdeSystem + ?Sized,
{
    let n = system.n_eqns();
    if dfdx.len() != n {
        dfdx.resize(n, 0.0);
    }
    if dfdy.n() != n {
        *dfdy = SquareMatrix::new(n);
    }

    // d f_i / d y_j -- the state Jacobian. Columns run serially: an ODE
    // Jacobian is small (n is equation count, typically single or double
    // digits) and `ode::parallel::integrate_ensemble` already provides the
    // parallel axis that matters for ODE work, which is the ensemble lane.
    let solution = jacobian_columns_serial(
        0,
        y,
        settings,
        &|_: usize, state: &[f64], out: &mut Vec<f64>| {
            out.clear();
            system.derivatives(x, state, out);
        },
    );
    for i in 0..n {
        for j in 0..n {
            dfdy.set(i, j, solution.raw_matrix().get(i, j));
        }
    }
    let mut status = solution.status();

    // d f_i / d x -- the explicit dependence on the independent variable.
    // Same stencil, same step rule, one scalar direction.
    let x_status = ode_dfdx(system, x, y, settings, n, dfdx);
    if status.is_ok() {
        status = x_status;
    }
    status
}

/// Fill `dfdx` — the derivative of each equation with respect to the
/// independent variable, holding the state fixed.
fn ode_dfdx<S>(
    system: &S,
    x: f64,
    y: &[f64],
    settings: DiffSettings,
    n: usize,
    dfdx: &mut [f64],
) -> DiffStatus
where
    S: OdeSystem + ?Sized,
{
    // Treat `x` as a one-component point and reuse the column kernel, so the
    // stencil, the step rule and the realised-step correction are literally the
    // same code as for `dfdy`.
    let point = [x];
    let solution = jacobian_column_scalar_direction(&point, settings, n, &|t: f64,
                                                                           out: &mut Vec<f64>| {
        out.clear();
        system.derivatives(t, y, out);
    });
    match solution {
        Ok(values) => {
            dfdx[..n].copy_from_slice(&values[..n]);
            DiffStatus::Ok
        }
        Err(status) => {
            for v in dfdx.iter_mut().take(n) {
                *v = f64::NAN;
            }
            status
        }
    }
}

/// The `dfdx` stencil: one scalar direction, `n` outputs.
///
/// Structurally identical to [`jacobian_column`] with `j = 0`; kept separate
/// only because its function takes a bare `f64` rather than a slice, which is
/// what [`OdeSystem::derivatives`] wants for its independent variable.
fn jacobian_column_scalar_direction<G>(
    point: &[f64; 1],
    settings: DiffSettings,
    n: usize,
    g: &G,
) -> Result<Vec<f64>, DiffStatus>
where
    G: Fn(f64, &mut Vec<f64>),
{
    let x = point[0];
    if !x.is_finite() {
        return Err(DiffStatus::InvalidPoint);
    }
    let h = settings.step_for(x);
    if !h.is_finite() || h <= 0.0 {
        return Err(DiffStatus::DegenerateStep);
    }

    let sample = |t: f64| -> Result<Vec<f64>, DiffStatus> {
        let mut out = Vec::with_capacity(n);
        g(t, &mut out);
        if out.len() != n {
            return Err(DiffStatus::DimensionMismatch);
        }
        if out.iter().any(|v| !v.is_finite()) {
            return Err(DiffStatus::NotFinite);
        }
        Ok(out)
    };
    let quotient = |a: Vec<f64>, b: Vec<f64>, dh: f64| -> Result<Vec<f64>, DiffStatus> {
        let mut out = Vec::with_capacity(n);
        for (p, q) in a.into_iter().zip(b) {
            let v = (p - q) / dh;
            if !v.is_finite() {
                return Err(DiffStatus::NotFinite);
            }
            out.push(v);
        }
        Ok(out)
    };
    let one_sided = |step: f64| -> Result<Vec<f64>, DiffStatus> {
        let xp = x + step;
        let dh = xp - x;
        if dh == 0.0 || !dh.is_finite() {
            return Err(DiffStatus::DegenerateStep);
        }
        quotient(sample(xp)?, sample(x)?, dh)
    };
    let central = |half: f64| -> Result<Vec<f64>, DiffStatus> {
        let (xp, xm) = (x + half, x - half);
        let dh = xp - xm;
        if dh == 0.0 || !dh.is_finite() {
            return Err(DiffStatus::DegenerateStep);
        }
        quotient(sample(xp)?, sample(xm)?, dh)
    };

    match settings.scheme {
        DiffScheme::Forward => one_sided(h),
        DiffScheme::Backward => one_sided(-h),
        DiffScheme::Central => central(h),
        DiffScheme::Central4th => {
            let coarse = central(h)?;
            let fine = central(0.5 * h)?;
            let mut out = Vec::with_capacity(n);
            for (c, fi) in coarse.into_iter().zip(fine) {
                let v = (4.0 * fi - c) / 3.0;
                if !v.is_finite() {
                    return Err(DiffStatus::NotFinite);
                }
                out.push(v);
            }
            Ok(out)
        }
    }
}

/// Wrap any [`OdeSystem`] so that [`crate::ode::Rosenbrock23`] can integrate it
/// **without a hand-coded Jacobian**.
///
/// # The problem this solves
///
/// [`OdeSystem::jacobian`] has a default body that is `unimplemented!()`, so a
/// system that does not override it panics the moment a stiff solver asks for a
/// Jacobian — inside `Rosenbrock23::inner_step`, and, if the integration is
/// running in an ensemble, out through the `rayon` scope. Every system that
/// only knows its own `derivatives` is locked out of the crate's only stiff
/// solver.
///
/// Wrapping it in `NumericalJacobian` supplies the missing method by finite
/// differences and changes nothing else: `n_eqns` and `derivatives` are
/// forwarded verbatim.
///
/// # Owning, not borrowing
///
/// The wrapper **owns** the system by value, so it needs no lifetime parameter
/// and no `Box` — both forbidden by the workspace design rules. Construct it
/// with [`Self::new`], get the system back with [`Self::into_inner`].
///
/// # An analytic Jacobian is still better
///
/// Finite differences cost `n + 1` to `4n` extra `derivatives` calls per
/// Rosenbrock stage and are accurate to roughly `sqrt(eps)` to `eps^(4/5)`
/// rather than to machine precision — see the module-level "Achievable
/// accuracy" table. If the analytic Jacobian is available, write it. This
/// wrapper is for the systems where it is not, and as a **verification oracle**
/// for the ones where it is: differencing a system that also implements
/// `jacobian` analytically and comparing is the cheapest real check that the
/// hand-derived version has no sign or transposition error.
///
/// # Units
///
/// Inherited from the wrapped system; nothing here is dimensioned.
///
/// # Example
///
/// ```rust
/// use outram_foam_basic_lib::math::differentiate::{DiffSettings, NumericalJacobian};
/// use outram_foam_basic_lib::ode::{OdeSystem, Rosenbrock23};
///
/// // A stiff scalar system with NO hand-coded Jacobian: dy/dx = -1000 y.
/// struct StiffDecay;
/// impl OdeSystem for StiffDecay {
///     fn n_eqns(&self) -> usize { 1 }
///     fn derivatives(&self, _x: f64, y: &[f64], dydx: &mut Vec<f64>) {
///         dydx.clear();
///         dydx.push(-1000.0 * y[0]);
///     }
///     // no `jacobian` override -- the default would panic
/// }
///
/// let system = NumericalJacobian::new(StiffDecay, DiffSettings::central());
/// let mut solver = Rosenbrock23::new(1, 1e-10, 1e-10);
/// let mut y = vec![1.0_f64];
/// let mut dx = 1e-5;
/// solver.integrate(&system, 0.0, 0.01, &mut y, &mut dx).expect("integrates");
///
/// // exp(-1000 * 0.01) = exp(-10) = 4.5399929762484854e-5
/// let exact = (-10.0_f64).exp();
/// assert!((y[0] - exact).abs() < 1e-8, "got {}, want {exact}", y[0]);
/// assert_eq!(system.non_finite_jacobians(), 0);
/// ```
#[derive(Debug)]
pub struct NumericalJacobian<S> {
    system: S,
    settings: DiffSettings,
    non_finite: AtomicUsize,
}

impl<S> NumericalJacobian<S> {
    /// Wrap `system`, differencing its `derivatives` with `settings`.
    ///
    /// # Arguments
    ///
    /// - `system` — owned by value.
    /// - `settings` — scheme and step-size policy. [`DiffSettings::central`] is
    ///   the usual choice for a Rosenbrock Jacobian: `2n` evaluations for
    ///   `O(h^2)` truncation, where [`DiffScheme::Central4th`] doubles the cost
    ///   again for accuracy the step controller cannot exploit.
    pub fn new(system: S, settings: DiffSettings) -> Self {
        Self {
            system,
            settings,
            non_finite: AtomicUsize::new(0),
        }
    }

    /// Borrow the wrapped system.
    pub fn inner(&self) -> &S {
        &self.system
    }

    /// Unwrap, returning the system.
    pub fn into_inner(self) -> S {
        self.system
    }

    /// The step-size policy in force.
    #[must_use]
    pub fn settings(&self) -> DiffSettings {
        self.settings
    }

    /// How many [`OdeSystem::jacobian`] calls have failed since construction.
    ///
    /// The trait method returns `()`, so it has nowhere to report a failure;
    /// this counter is that report. A non-zero value means at least one
    /// Jacobian was handed to the solver with `NaN` entries, which the solver
    /// will have turned into [`crate::ode::OdeError::StepSizeUnderflow`] rather
    /// than a wrong answer — but it is worth knowing *why* an integration
    /// failed, and "the Jacobian could not be differenced" is a different bug
    /// from "the system is too stiff".
    ///
    /// # Units
    ///
    /// A count, dimensionless.
    #[must_use]
    pub fn non_finite_jacobians(&self) -> usize {
        self.non_finite.load(Ordering::Relaxed)
    }
}

impl<S> OdeSystem for NumericalJacobian<S>
where
    S: OdeSystem,
{
    fn n_eqns(&self) -> usize {
        self.system.n_eqns()
    }

    fn derivatives(&self, x: f64, y: &[f64], dydx: &mut Vec<f64>) {
        self.system.derivatives(x, y, dydx);
    }

    fn jacobian(&self, x: f64, y: &[f64], dfdx: &mut Vec<f64>, dfdy: &mut SquareMatrix) {
        let status = ode_system_jacobian(&self.system, x, y, self.settings, dfdx, dfdy);
        if !status.is_ok() {
            self.non_finite.fetch_add(1, Ordering::Relaxed);
        }
    }
}
