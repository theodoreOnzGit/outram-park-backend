//! Reactor unit operations — conversion, equilibrium, CSTR, and PFR.
//!
//! Pure-Rust port of DWSIM's `DWSIM.UnitOperations/Reactors/` unit operations,
//! built on the reaction model in [`crate::reactions`]. Each reactor takes a
//! [`ReactorFeed`] (inlet molar flows, `T`, `P`, volumetric flow) and returns a
//! [`ReactorOutcome`] (outlet molar flows, per-reaction extents, heat of
//! reaction).
//!
//! ## Provenance (GPL-3.0)
//!
//! Ported from **DWSIM** (commit `1abf72d`, GPL-3.0; upstream copyright Daniel
//! Wagner O. de Medeiros):
//!
//! - [`conversion_reactor`] ← `Reactors/Conversion.vb`
//! - [`equilibrium_reactor`] ← `Reactors/Equilibrium.vb`
//! - [`gibbs_reactor`] ← `Reactors/Gibbs.vb`
//! - [`cstr`] ← `Reactors/CSTR.vb`
//! - [`pfr`] ← `Reactors/PFR.vb`
//!
//! ## Design
//!
//! Enum dispatch, no trait objects (workspace "no `dyn`" rule): the reactor
//! choice is the closed set [`ReactorModel`], which `match`es to the wrapped
//! reactor's own `solve`. Compounds are addressed by index into the feed's
//! molar-flow vector; every reaction's `component_index` shares that index
//! space.
//!
//! ## Honest scope (⚠️ untrusted draft, pending human V&V)
//!
//! Early-stage translation, **no human V&V** — untrusted draft material
//! (workspace `RESPONSIBLE_USE.md`). Not for nuclear facility operation, reactor
//! control, safety-critical, or licensing decisions. Independent OUTRAM PARK
//! fork, not the official DWSIM.
//!
//! Simplifications versus upstream, each an honest limitation:
//!
//! - **Constant volumetric flow.** DWSIM re-flashes the mixture each iteration
//!   to update the volumetric flow `Q` as composition changes. This port holds
//!   `Q` fixed at the feed value (exact for equimolar / liquid-phase reactions,
//!   approximate when the mole count changes). The reactor solvers are decoupled
//!   from [`crate::thermo`]'s flash, matching how [`crate::pump`] is decoupled.
//! - **Isothermal solve.** The reactors solve the material balance at the feed
//!   temperature. The heat of reaction is *reported* ([`ReactorOutcome::heat_of_reaction`])
//!   but not fed back into an energy balance to update `T` (DWSIM's adiabatic /
//!   outlet-T modes are not ported).
//! - **Ideal equilibrium basis.** The equilibrium reactor uses mole-fraction or
//!   partial-pressure activity with unit activity/fugacity coefficients.

pub mod conversion_reactor;
pub mod cstr;
pub mod equilibrium_reactor;
pub mod gibbs_reactor;
pub mod pfr;

pub use conversion_reactor::ConversionReactor;
pub use cstr::Cstr;
pub use equilibrium_reactor::EquilibriumReactor;
pub use gibbs_reactor::{GibbsFormation, GibbsReactor};
pub use pfr::Pfr;

/// Inlet state of a reactor: the per-compound molar flows plus the intensive
/// conditions the reactors need.
///
/// `molar_flows` is indexed by compound; every reaction's
/// [`ReactionComponent::component_index`](crate::reactions::ReactionComponent::component_index)
/// addresses this same vector.
#[derive(Debug, Clone, PartialEq)]
pub struct ReactorFeed {
    /// Inlet molar flow of each compound `[mol/s]`, by component index.
    pub molar_flows: Vec<f64>,
    /// Temperature `T` [K].
    pub temperature: f64,
    /// Pressure `P` [Pa].
    pub pressure: f64,
    /// Total volumetric flow `Q` [m³/s] (held constant through the reactor —
    /// see the module "Honest scope" note). Used to turn molar flows into
    /// concentrations `Cᵢ = Fᵢ / Q` for the kinetic reactors.
    pub volumetric_flow: f64,
}

impl ReactorFeed {
    /// Construct a feed. `volumetric_flow` may be `0.0` for reactors that do not
    /// need concentrations (the conversion and mole-fraction-basis equilibrium
    /// reactors); the kinetic reactors require `Q > 0`.
    #[must_use]
    pub fn new(molar_flows: Vec<f64>, temperature: f64, pressure: f64, volumetric_flow: f64) -> Self {
        Self {
            molar_flows,
            temperature,
            pressure,
            volumetric_flow,
        }
    }

    /// Total molar flow `Σᵢ Fᵢ` [mol/s].
    #[must_use]
    pub fn total_molar_flow(&self) -> f64 {
        self.molar_flows.iter().sum()
    }
}

/// Result of a reactor solve.
#[derive(Debug, Clone, PartialEq)]
pub struct ReactorOutcome {
    /// Outlet molar flow of each compound `[mol/s]`, by component index.
    pub molar_flows: Vec<f64>,
    /// Per-reaction extent `[mol/s]` (the reaction-as-written progress, one
    /// entry per reaction in the reactor's list). For flow reactors this is the
    /// reaction rate integrated over the reactor volume.
    pub extents: Vec<f64>,
    /// Net heat of reaction `[W]` = `Σ_r ΔH°_r · ζ_r`. Positive = net
    /// endothermic (heat must be supplied to hold the feed temperature).
    pub heat_of_reaction: f64,
}

impl ReactorOutcome {
    /// Fractional conversion of the compound at `component_index`, relative to
    /// the given inlet feed: `X = (F_in − F_out) / F_in`. Returns `0.0` if the
    /// inlet flow is zero.
    #[must_use]
    pub fn conversion_of(&self, feed: &ReactorFeed, component_index: usize) -> f64 {
        let f_in = feed.molar_flows[component_index];
        if f_in.abs() < f64::EPSILON {
            return 0.0;
        }
        (f_in - self.molar_flows[component_index]) / f_in
    }
}

/// What can go wrong in a reactor solve.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ReactorError {
    /// The iterative solver did not converge within its iteration budget.
    #[error("reactor solver failed to converge after {iterations} iterations (residual {residual:e})")]
    NonConvergence {
        /// Iterations attempted.
        iterations: usize,
        /// Final residual norm.
        residual: f64,
    },
    /// The feed was malformed (e.g. a kinetic reactor with `Q ≤ 0`, or a
    /// component index out of range).
    #[error("invalid reactor feed: {0}")]
    InvalidFeed(String),
}

/// The closed set of reactor unit operations — enum dispatch, no `dyn`.
///
/// Each variant wraps a fully-configured reactor (reactions + geometry).
/// [`solve`](ReactorModel::solve) `match`es to the wrapped reactor's own solve,
/// so adding a variant forces every dispatch site to handle it.
#[derive(Debug, Clone, PartialEq)]
pub enum ReactorModel {
    /// Fixed-conversion reactor (`Reactors/Conversion.vb`).
    Conversion(ConversionReactor),
    /// Chemical-equilibrium reactor (`Reactors/Equilibrium.vb`).
    Equilibrium(EquilibriumReactor),
    /// Gibbs-energy-minimisation equilibrium reactor (`Reactors/Gibbs.vb`) —
    /// outlet speciation from a feed with **no reaction list**.
    Gibbs(GibbsReactor),
    /// Continuous stirred-tank reactor (`Reactors/CSTR.vb`).
    Cstr(Cstr),
    /// Plug-flow reactor (`Reactors/PFR.vb`).
    Pfr(Pfr),
}

impl ReactorModel {
    /// Solve this reactor for the given `feed`.
    pub fn solve(&self, feed: &ReactorFeed) -> Result<ReactorOutcome, ReactorError> {
        match self {
            ReactorModel::Conversion(r) => r.solve(feed),
            ReactorModel::Equilibrium(r) => r.solve(feed),
            ReactorModel::Gibbs(r) => r.solve(feed),
            ReactorModel::Cstr(r) => r.solve(feed),
            ReactorModel::Pfr(r) => r.solve(feed),
        }
    }
}

/// Solve the dense linear system `A x = b` by Gaussian elimination with partial
/// pivoting, in place. `a` is an `n × n` row-major matrix (`a[i][j]`), `b` the
/// right-hand side; on success returns the solution `x`. Returns `None` if the
/// matrix is singular to working precision.
///
/// A small self-contained solver (no `ndarray-linalg`/BLAS) used by the Newton
/// iterations in [`cstr`] and [`equilibrium_reactor`]; the systems are tiny (one
/// unknown per reaction), so an `O(n³)` dense solve is ample and keeps the crate
/// Android-buildable (no system LAPACK).
pub(crate) fn solve_linear(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for col in 0..n {
        // Partial pivot: largest magnitude in this column at or below the diagonal.
        let mut pivot = col;
        let mut best = a[col][col].abs();
        for row in (col + 1)..n {
            let v = a[row][col].abs();
            if v > best {
                best = v;
                pivot = row;
            }
        }
        if best < 1e-300 {
            return None; // singular
        }
        a.swap(col, pivot);
        b.swap(col, pivot);

        // Eliminate below.
        for row in (col + 1)..n {
            let factor = a[row][col] / a[col][col];
            if factor != 0.0 {
                for k in col..n {
                    a[row][k] -= factor * a[col][k];
                }
                b[row] -= factor * b[col];
            }
        }
    }

    // Back-substitution.
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let mut sum = b[row];
        for k in (row + 1)..n {
            sum -= a[row][k] * x[k];
        }
        x[row] = sum / a[row][row];
    }
    Some(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Methodology.** The 2×2 system `[[2,1],[1,3]] x = [3,5]` has the exact
    /// solution `x = [0.8, 1.4]`. Partial pivoting must also handle a zero
    /// leading pivot (`[[0,1],[1,0]] x = [2,3] → x = [3,2]`).
    ///
    /// **Measured result (2026-08-03).** Both solve to `< 1e−12`; a singular
    /// matrix `[[1,1],[1,1]]` returns `None`.
    #[test]
    fn gaussian_elimination() {
        let x = solve_linear(vec![vec![2.0, 1.0], vec![1.0, 3.0]], vec![3.0, 5.0]).unwrap();
        assert!((x[0] - 0.8).abs() < 1e-12 && (x[1] - 1.4).abs() < 1e-12);

        let x2 = solve_linear(vec![vec![0.0, 1.0], vec![1.0, 0.0]], vec![2.0, 3.0]).unwrap();
        assert!((x2[0] - 3.0).abs() < 1e-12 && (x2[1] - 2.0).abs() < 1e-12);

        assert!(solve_linear(vec![vec![1.0, 1.0], vec![1.0, 1.0]], vec![1.0, 2.0]).is_none());
    }
}
