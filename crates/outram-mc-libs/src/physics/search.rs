//! Criticality search — a bracketed root-find of a scalar model parameter `p`
//! such that `k_eff(p) = target` (default `target = 1.0`).
//!
//! This is the OUTRAM PARK analogue of OpenMC's `openmc.search_for_keff`
//! (`openmc/search.py`): given a model that turns one scalar knob `p` into a
//! k-eigenvalue calculation, find the value of `p` that makes the system exactly
//! critical (or hits any other target eigenvalue). Typical knobs a reactor
//! physicist sweeps this way:
//!
//! - **Soluble-boron concentration** \[ppm\] in a PWR pin cell — the canonical
//!   OpenMC `search.ipynb` case (critical ≈ 1926 ppm; more boron ⇒ lower `k`).
//! - **Critical dimension** — the radius \[cm\] of a bare fissile sphere, the
//!   pitch of a lattice, a control-rod insertion depth (bigger ⇒ higher `k`).
//! - **Enrichment**, **moderator density**, **temperature** — any single scalar
//!   the k-eigenvalue depends on monotonically over the search bracket.
//!
//! # What "criticality search" means physically
//!
//! `k_eff` is the neutron multiplication factor: the ratio of neutrons produced
//! in one fission generation to those in the previous one. `k_eff = 1` is exact
//! criticality (a self-sustaining chain reaction, steady in time); `k > 1` is
//! supercritical, `k < 1` subcritical. A criticality search answers the inverse
//! design question — *what value of some physical parameter makes `k_eff` equal a
//! chosen target* — by root-finding the residual `g(p) = k_eff(p) − target`.
//!
//! # Method (mirrors OpenMC's bisection semantics)
//!
//! The driver is a **bracketed** root-find: the caller supplies an interval
//! `[lo, hi]` on which `g` changes sign, guaranteeing a root inside (intermediate
//! value theorem, `g` assumed continuous and monotone over the bracket). Two
//! bracket-preserving methods are offered — both keep the root enclosed at every
//! step, which matters because each `k_eff(p)` is a *noisy* Monte Carlo estimate:
//!
//! - [`SearchMethod::Bisect`] — halve the bracket each step. The method OpenMC's
//!   `search.ipynb` uses (`bracketed_method='bisect'`). Linear convergence, one
//!   `k_eff` solve per iteration, and maximally robust to Monte Carlo noise
//!   because it only ever uses the *sign* of `g`, never its magnitude.
//! - [`SearchMethod::Secant`] — bracket-preserving false position (regula falsi):
//!   the next guess is the `g`-weighted secant intercept, but the sub-interval
//!   that still straddles the root is always retained. Usually fewer iterations
//!   than bisection, still guaranteed to converge. (This is the *bracketed*
//!   secant; unbracketed secant can diverge on a noisy `g`, so it is deliberately
//!   not offered.)
//!
//! Both stop when the bracket width falls below [`SearchSettings::tol`] (a
//! tolerance in the *units of `p`*), or when `|g(mid)|` falls below
//! [`SearchSettings::k_tol`], or when [`SearchSettings::max_iterations`] is hit.
//!
//! # No trait objects
//!
//! The model is passed as a generic closure `F: FnMut(f64) -> KeffResult`, a
//! compile-time-monomorphised function parameter — **not** a `Box<dyn Fn>` — in
//! keeping with the workspace's no-trait-object rule. Nothing here stores the
//! closure in a struct, so there are no lifetime parameters either.
//!
//! # Example
//!
//! ```no_run
//! use outram_mc_libs::material::material::{Material, NuclideComponent};
//! use outram_mc_libs::material::nuclide::Nuclide;
//! use outram_mc_libs::physics::keff::{run_keff, KeffSettings};
//! use outram_mc_libs::physics::search::{search_for_keff, SearchSettings};
//!
//! // Find the critical radius of a bare HEU (Godiva) sphere: k_eff(r) = 1.
//! let nuclides = vec![
//!     Nuclide::from_core("U234").unwrap(),
//!     Nuclide::from_core("U235").unwrap(),
//!     Nuclide::from_core("U238").unwrap(),
//! ];
//! let mat = Material {
//!     id: 1,
//!     name: "HEU".into(),
//!     temperature: 293.6,
//!     components: vec![
//!         NuclideComponent { nuclide_idx: 0, atom_density: 4.9184e-4 },
//!         NuclideComponent { nuclide_idx: 1, atom_density: 4.4994e-2 },
//!         NuclideComponent { nuclide_idx: 2, atom_density: 2.4984e-3 },
//!     ],
//! };
//! let keff_settings = KeffSettings::default();
//! // k_eff increases with radius, so the bracket [7, 10] cm straddles k = 1.
//! let result = search_for_keff(
//!     |r| run_keff(r, &mat, &nuclides, &keff_settings),
//!     (7.0, 10.0),
//!     &SearchSettings::default(),
//! )
//! .unwrap();
//! println!("critical radius = {:.3} cm, k = {:.5}", result.parameter, result.keff);
//! ```

use crate::physics::keff::KeffResult;

/// Which bracketed root-finding method [`search_for_keff`] uses.
///
/// Both variants keep the root enclosed at every step (they never leave the
/// straddling interval), so both are robust to the Monte Carlo noise on each
/// `k_eff(p)` estimate. See the module docs for the trade-off.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMethod {
    /// Bisection — halve the bracket each step. Mirrors OpenMC
    /// `bracketed_method='bisect'` (the `search.ipynb` default). Uses only the
    /// *sign* of the residual, so it is the most noise-tolerant option.
    Bisect,
    /// Bracket-preserving false position (regula falsi) — the *bracketed*
    /// secant. The next guess is the `g`-weighted linear intercept, but the
    /// straddling sub-interval is always retained. Usually converges in fewer
    /// `k_eff` solves than bisection.
    Secant,
}

/// Settings controlling a [`search_for_keff`] criticality search.
///
/// The physical parameter `p` being searched has whatever units the caller's
/// model uses (ppm boron, cm radius, …); [`tol`](Self::tol) is therefore
/// expressed in *those same units*.
#[derive(Debug, Clone, Copy)]
pub struct SearchSettings {
    /// Target eigenvalue to solve for. `1.0` = exact criticality (the default).
    pub target: f64,
    /// Convergence tolerance on the **parameter** `p`, in the caller's units of
    /// `p`. The search stops once the bracket width `|hi − lo|` falls below this.
    /// Choose it comfortably larger than the parameter change that moves `k_eff`
    /// by one standard error, so the search stops before Monte Carlo noise
    /// dominates the residual sign.
    pub tol: f64,
    /// Convergence tolerance on the **residual** `|k_eff(mid) − target|`. The
    /// search also stops early if a midpoint lands this close to the target `k`.
    /// Set it near the per-solve statistical error `k_std`; a value of `0.0`
    /// disables this criterion (parameter tolerance / iteration cap only).
    pub k_tol: f64,
    /// Hard cap on the number of iterations (each iteration is one `k_eff`
    /// solve). A guard against a non-converging search; a converged search
    /// usually needs `ceil(log2(width / tol))` iterations for bisection.
    pub max_iterations: usize,
    /// Which bracketed method to use ([`SearchMethod::Bisect`] by default,
    /// mirroring the OpenMC `search.ipynb`).
    pub method: SearchMethod,
}

impl Default for SearchSettings {
    /// Exact-criticality search (`target = 1.0`) by bisection, tolerances tuned
    /// for a typical noisy Monte Carlo residual: parameter tolerance `0.25` (in
    /// the caller's units), residual tolerance `0.005` (≈ one k-eigenvalue
    /// standard error on a modest run), capped at 20 iterations.
    fn default() -> Self {
        Self {
            target: 1.0,
            tol: 0.25,
            k_tol: 0.005,
            max_iterations: 20,
            method: SearchMethod::Bisect,
        }
    }
}

/// One evaluated point of a criticality search: the parameter value tried and
/// the k-eigenvalue (with its statistical error) the model returned there.
///
/// The sequence of these (in [`SearchResult::iterations`]) is the OUTRAM PARK
/// analogue of the `(guesses, keffs)` trajectory OpenMC's `search_for_keff`
/// returns — useful for plotting the search path or diagnosing non-convergence.
#[derive(Debug, Clone, Copy)]
pub struct SearchIteration {
    /// Parameter value `p` evaluated at this step (caller's units).
    pub parameter: f64,
    /// Mean k-eigenvalue `k_eff(p)` at this parameter.
    pub keff: f64,
    /// Standard error (1σ) of that k-eigenvalue estimate.
    pub keff_std: f64,
}

/// Outcome of a [`search_for_keff`] criticality search.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Converged parameter value `p*` with `k_eff(p*) ≈ target` (caller's units).
    pub parameter: f64,
    /// Mean k-eigenvalue at the converged parameter.
    pub keff: f64,
    /// Standard error (1σ) of the k-eigenvalue at the converged parameter.
    pub keff_std: f64,
    /// Whether a convergence criterion (parameter tolerance or residual
    /// tolerance) was met before the iteration cap. `false` means the search
    /// ran out of iterations; [`parameter`](Self::parameter) then holds the best
    /// (final-bracket-midpoint) estimate so far.
    pub converged: bool,
    /// Full search trajectory: the two bracket endpoints first, then every
    /// midpoint evaluated, in order. Mirrors OpenMC's `(guesses, keffs)`.
    pub iterations: Vec<SearchIteration>,
}

/// Why a [`search_for_keff`] call could not be started.
#[derive(Debug, Clone, PartialEq)]
pub enum SearchError {
    /// The supplied bracket does not straddle the target: the residual
    /// `g(p) = k_eff(p) − target` has the **same sign** at both endpoints, so no
    /// root is guaranteed inside. Widen or move the bracket. Fields are the two
    /// endpoint eigenvalues and the target, for diagnostics.
    BracketDoesNotStraddle {
        /// `k_eff` at the low endpoint `lo`.
        k_lo: f64,
        /// `k_eff` at the high endpoint `hi`.
        k_hi: f64,
        /// The target eigenvalue searched for.
        target: f64,
    },
    /// The bracket was degenerate (`lo` and `hi` equal, or non-finite), so no
    /// interval exists to search.
    InvalidBracket {
        /// Low endpoint as supplied.
        lo: f64,
        /// High endpoint as supplied.
        hi: f64,
    },
}

impl std::fmt::Display for SearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SearchError::BracketDoesNotStraddle { k_lo, k_hi, target } => write!(
                f,
                "criticality-search bracket does not straddle the target: \
                 k_eff(lo) = {k_lo:.5}, k_eff(hi) = {k_hi:.5}, both on the same \
                 side of target {target:.5}"
            ),
            SearchError::InvalidBracket { lo, hi } => {
                write!(f, "invalid criticality-search bracket [{lo}, {hi}]")
            }
        }
    }
}

impl std::error::Error for SearchError {}

/// Bracketed criticality search: find the scalar parameter `p` in `bracket`
/// such that `k_eff(p) = settings.target`.
///
/// # Physical meaning
///
/// `k_of_p` is the caller's model: it takes one scalar knob `p` (boron ppm, a
/// critical radius \[cm\], enrichment, …) and returns a full [`KeffResult`] from
/// a k-eigenvalue transport solve at that `p`. This function root-finds the
/// residual `g(p) = k_eff(p) − target` to locate the parameter that hits the
/// target eigenvalue — exact criticality when `target = 1.0`.
///
/// # Arguments
///
/// - `k_of_p` — the model closure. Called once per endpoint and once per
///   iteration. It is `FnMut` so it may carry mutable scratch state (e.g. a
///   reusable geometry buffer); it is a monomorphised generic parameter, **not**
///   a trait object.
/// - `bracket` — `(lo, hi)` in the units of `p`. `g` must change sign across it
///   (i.e. one endpoint is subcritical relative to the target and the other
///   supercritical); otherwise [`SearchError::BracketDoesNotStraddle`] is
///   returned before any iteration. `lo` and `hi` may be given in either order.
/// - `settings` — target, tolerances, iteration cap, and method (see
///   [`SearchSettings`]).
///
/// # Returns
///
/// A [`SearchResult`] with the converged parameter, the eigenvalue (± 1σ) there,
/// a `converged` flag, and the full `(parameter, k)` trajectory. Returns
/// [`SearchError`] only for a bad bracket — a search that merely exhausts its
/// iteration budget still returns `Ok` with `converged = false` and the best
/// estimate so far.
///
/// # Monotonicity assumption
///
/// Like OpenMC's `search_for_keff`, this assumes `k_eff` varies monotonically
/// with `p` across the bracket (true for the usual knobs over a sensible range).
/// The bracketed methods stay convergent even when each `k_eff` is a noisy Monte
/// Carlo estimate, because they retain the straddling sub-interval every step.
pub fn search_for_keff<F>(
    mut k_of_p: F,
    bracket: (f64, f64),
    settings: &SearchSettings,
) -> Result<SearchResult, SearchError>
where
    F: FnMut(f64) -> KeffResult,
{
    // Normalise the bracket so lo < hi; reject a degenerate/non-finite one.
    let (mut lo, mut hi) = (bracket.0.min(bracket.1), bracket.0.max(bracket.1));
    if !lo.is_finite() || !hi.is_finite() || (hi - lo).abs() <= f64::EPSILON {
        return Err(SearchError::InvalidBracket {
            lo: bracket.0,
            hi: bracket.1,
        });
    }

    let target = settings.target;
    let mut iterations: Vec<SearchIteration> = Vec::with_capacity(settings.max_iterations + 2);

    // Evaluate both endpoints; the residual g = k - target must straddle zero.
    let r_lo = k_of_p(lo);
    let r_hi = k_of_p(hi);
    let mut g_lo = r_lo.k_mean - target;
    let mut g_hi = r_hi.k_mean - target;
    iterations.push(SearchIteration {
        parameter: lo,
        keff: r_lo.k_mean,
        keff_std: r_lo.k_std,
    });
    iterations.push(SearchIteration {
        parameter: hi,
        keff: r_hi.k_mean,
        keff_std: r_hi.k_std,
    });

    if g_lo * g_hi > 0.0 {
        return Err(SearchError::BracketDoesNotStraddle {
            k_lo: r_lo.k_mean,
            k_hi: r_hi.k_mean,
            target,
        });
    }

    // Track the most recent midpoint evaluation as the running best estimate.
    let mut best = SearchIteration {
        parameter: 0.5 * (lo + hi),
        keff: 0.5 * (r_lo.k_mean + r_hi.k_mean),
        keff_std: 0.5 * (r_lo.k_std + r_hi.k_std),
    };
    let mut converged = false;

    for _ in 0..settings.max_iterations {
        // Stop once the bracket is tight enough in parameter space.
        if (hi - lo).abs() <= settings.tol {
            converged = true;
            break;
        }

        // Next trial point: bracket bisection, or the g-weighted false-position
        // (bracketed secant) intercept, clamped strictly inside (lo, hi).
        let mid = match settings.method {
            SearchMethod::Bisect => 0.5 * (lo + hi),
            SearchMethod::Secant => {
                let denom = g_hi - g_lo;
                let raw = if denom.abs() <= f64::EPSILON {
                    0.5 * (lo + hi)
                } else {
                    // False-position: intercept of the secant through
                    // (lo, g_lo)–(hi, g_hi). Guaranteed inside [lo, hi] when the
                    // endpoints straddle zero.
                    hi - g_hi * (hi - lo) / denom
                };
                // Guard against a degenerate secant landing on an endpoint
                // (which would stall progress): nudge back toward the midpoint.
                let span = hi - lo;
                raw.clamp(lo + 1e-6 * span, hi - 1e-6 * span)
            }
        };

        let r_mid = k_of_p(mid);
        let g_mid = r_mid.k_mean - target;
        best = SearchIteration {
            parameter: mid,
            keff: r_mid.k_mean,
            keff_std: r_mid.k_std,
        };
        iterations.push(best);

        // Residual-tolerance early exit (disabled when k_tol == 0.0).
        if settings.k_tol > 0.0 && g_mid.abs() <= settings.k_tol {
            converged = true;
            break;
        }

        // Retain the sub-interval that still straddles the root.
        if g_lo * g_mid <= 0.0 {
            hi = mid;
            g_hi = g_mid;
        } else {
            lo = mid;
            g_lo = g_mid;
        }

        if (hi - lo).abs() <= settings.tol {
            converged = true;
            break;
        }
    }

    Ok(SearchResult {
        parameter: best.parameter,
        keff: best.keff,
        keff_std: best.keff_std,
        converged,
        iterations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a synthetic [`KeffResult`] with a given mean (zero noise) so the
    /// root-finding logic can be unit-tested without a transport solve.
    fn synthetic(k: f64) -> KeffResult {
        KeffResult {
            k_mean: k,
            k_std: 0.0,
            k_by_generation: vec![k],
        }
    }

    /// Bisection converges to the analytic root of a clean monotone function.
    /// Model `k(p) = 1 + 0.1 (p − 5)` ⇒ critical at `p = 5`.
    #[test]
    fn bisect_finds_analytic_root() {
        let s = SearchSettings {
            tol: 1e-4,
            k_tol: 0.0,
            max_iterations: 60,
            ..Default::default()
        };
        let res = search_for_keff(|p| synthetic(1.0 + 0.1 * (p - 5.0)), (0.0, 10.0), &s).unwrap();
        assert!(res.converged);
        assert!(
            (res.parameter - 5.0).abs() < 1e-3,
            "root p = {}",
            res.parameter
        );
        assert!((res.keff - 1.0).abs() < 1e-3, "k = {}", res.keff);
    }

    /// The bracketed secant (false position) also nails a clean root, and does it
    /// in fewer iterations than bisection on this linear model (exact in one step).
    #[test]
    fn secant_finds_analytic_root() {
        let s = SearchSettings {
            tol: 1e-4,
            k_tol: 0.0,
            max_iterations: 60,
            method: SearchMethod::Secant,
            ..Default::default()
        };
        let res = search_for_keff(|p| synthetic(1.0 + 0.1 * (p - 5.0)), (0.0, 10.0), &s).unwrap();
        assert!(res.converged);
        assert!(
            (res.parameter - 5.0).abs() < 1e-2,
            "root p = {}",
            res.parameter
        );
    }

    /// A decreasing model (more boron ⇒ lower k) works too: `k(p) = 1.2 − 0.001 p`
    /// ⇒ critical near `p = 200`. Verifies the sign handling is orientation-free.
    #[test]
    fn handles_decreasing_model() {
        let s = SearchSettings {
            tol: 1e-3,
            k_tol: 0.0,
            max_iterations: 60,
            ..Default::default()
        };
        let res = search_for_keff(|p| synthetic(1.2 - 0.001 * p), (0.0, 1000.0), &s).unwrap();
        assert!(
            (res.parameter - 200.0).abs() < 1.0,
            "critical p = {}",
            res.parameter
        );
    }

    /// A bracket whose endpoints are both supercritical is rejected up front.
    #[test]
    fn rejects_non_straddling_bracket() {
        let s = SearchSettings::default();
        let err = search_for_keff(|_p| synthetic(1.5), (0.0, 10.0), &s).unwrap_err();
        assert!(matches!(err, SearchError::BracketDoesNotStraddle { .. }));
    }

    /// A degenerate bracket is rejected as invalid.
    #[test]
    fn rejects_degenerate_bracket() {
        let s = SearchSettings::default();
        let err = search_for_keff(|_p| synthetic(1.0), (3.0, 3.0), &s).unwrap_err();
        assert!(matches!(err, SearchError::InvalidBracket { .. }));
    }

    /// Exhausting the iteration budget returns Ok with converged = false and the
    /// best-so-far estimate, never an error.
    #[test]
    fn iteration_cap_returns_best_effort() {
        let s = SearchSettings {
            tol: 1e-12,
            k_tol: 0.0,
            max_iterations: 2,
            ..Default::default()
        };
        let res = search_for_keff(|p| synthetic(1.0 + 0.1 * (p - 5.0)), (0.0, 10.0), &s).unwrap();
        assert!(!res.converged);
        // Two bisection steps from [0,10] bracket the root in [2.5, 7.5]; midpoint ~5.
        assert!(res.parameter > 2.0 && res.parameter < 8.0);
    }
}
