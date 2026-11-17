// SPDX-License-Identifier: GPL-3.0

//! **k-eigenvalue search coupled to depletion** — an opt-in hook that, at the
//! beginning of every burnup step, adjusts one scalar model parameter (soluble
//! boron, an enrichment, a rod position) until the system is critical, then
//! depletes at that parameter. GitHub #1616, split from #1391; epic #257.
//!
//! This is the OUTRAM PARK port of OpenMC's `_KeffSearchControl`
//! (`openmc/deplete/keff_search_control.py`, upstream PR #4083) and of the
//! integrator hook that calls it. Read at `openmc-dev/openmc` commit
//! `8f79d9d02483c90e4134f03a9df4b75ce6b062bf`, from
//! `crates/outram-mc-libs/upstream_source/OpenMC/`.
//!
//! # What upstream does (read first, not guessed)
//!
//! * `Integrator.add_keff_search_control(function, x0, x1, bracket,
//!   **search_kwargs)` (`openmc/deplete/abc.py:1106-1199`) stores a
//!   `_KeffSearchControl` (`keff_search_control.py:7-134`). `bracket` must hold
//!   exactly two values with `bracket[0] < bracket[1]` (`:32-35`); they become
//!   `x_min` / `x_max` of the search (`:41-42`). `x0`, `x1` are the two
//!   **initial guesses** and are the same at every step (`:36-37`).
//! * **Where it runs.** In `Integrator.integrate` (`abc.py:924-985`), at the
//!   **beginning of each step**, through `_get_bos_data`
//!   (`abc.py:883-900`): `self._keff_search_control.run(bos_conc)` is called
//!   (`abc.py:887`) *before* the operator is evaluated for that step, only when
//!   `source_rate != 0.0`. It runs once more before the final operator
//!   evaluation after the last step (`abc.py:969-972`). The root is saved with
//!   the step (`keff_search_root=`, `abc.py:955, 983`) and is what a restart
//!   restores (`abc.py:871-881`).
//! * **What `run(x)` does** (`keff_search_control.py:44-65`): push the current
//!   atom densities `x` into the operator's materials (`:61`), run the search
//!   (`:63`), then write the search's final material compositions back into
//!   `x` (`_update_vec`, `:106-134`): for every burnable nuclide present in the
//!   searched material, `x[mat][nuc] = density * volume`; others keep their
//!   value. So a parameter that changes a **burnable** nuclide (enrichment)
//!   changes what is depleted; one that changes only a non-burnable one (boron)
//!   changes the transport solves but not the inventory.
//! * **The search** (`_search_for_keff`, `:67-104`) is
//!   `Model.keff_search(function, x0, x1, **kwargs)`
//!   (`openmc/model/model.py:3248-3405`), the GRsecant method of Price and
//!   Roskoff (2023): two seed evaluations at `x0`, `x1`; then repeated
//!   weighted-linear-fit secant steps over the last `memory = 4` points,
//!   **clamped to `[x_min, x_max]`** (`:3344-3348`), with the batch count per
//!   evaluation adapted to a target uncertainty. Stopping rule
//!   (`:3318-3321, 3388-3389`): `|k - target| <= k_tol` **and**
//!   `sigma <= sigma_final`; defaults `target = 1.0`, `k_tol = 1e-4`,
//!   `sigma_final = 3e-4`, `maxiter = 50`. If it has not converged the hook
//!   raises `ValueError` (`:85-89`) — it never carries on with a wrong
//!   parameter. A root outside `[x_min, x_max]` only warns (`:93-99`).
//!
//! # What this port does, and where it differs (stated, not hidden)
//!
//! | upstream | here |
//! |---|---|
//! | GRsecant (`Model.keff_search`), unbracketed, clamped to `x_min`/`x_max` | **the default since #1618:** [`grsecant_search_for_keff`](crate::physics::search_grsecant::grsecant_search_for_keff), a port of `model.py:3248-3405`; `x_min`/`x_max` are the `bracket` (`keff_search_control.py:41-42`). The workspace's bracketed [`search_for_keff`](crate::physics::search::search_for_keff) stays selectable by name ([`KeffSearchMethod::Bracketed`], [`KeffSearchControl::with_search_settings`]) |
//! | `x0`, `x1` seed a secant that may extrapolate | GRsecant: as upstream, no straddle needed, the seeds are evaluated as given. Bracketed: `(x0, x1)` is the bracket; if it does not straddle the target the search is retried **once** on `[x_min, x_max]` ([`KeffSearchStep::used_absolute_bracket`]), then errors |
//! | adaptive batches per evaluation to meet `sigma_final` | GRsecant: yes. The `model` closure is handed the active-batch count and must set `KeffSettings::n_active` from it; upstream's seeds use `b0`, later evaluations use the fitted count (`b_min`, `b_max`). The bracketed search has no `sigma_final` and always passes `b0` |
//! | search runs on the transport model | the caller's `model` closure: `(parameter, inventory, active_batches) -> KeffResult` |
//! | `_update_vec` writes the searched materials back into `x` | the caller's `adjust` closure: `(parameter, &mut inventory)`; a no-op for a parameter that changes no burnable nuclide |
//! | warns when the root is outside the bracket (`keff_search_control.py:92-99`) | [`KeffSearchStep::root_outside_bracket`] (a seed outside `[x_min, x_max]` can be the root) |
//! | runs for every `Integrator` | refused for the stochastic-implicit integrators ([`Integrator::is_stochastic_implicit`]): upstream's `SIIntegrator.integrate` (`abc.py:1400-1429`) never calls the hook |
//! | restart restores the stored root (`abc.py:871-881`) | **not ported**: this crate has no depletion restart files, so there is nothing to restore from; the roots a run found are in [`KeffSearchDepletion::searches`] and a caller re-applying one calls its own `function(root)` |
//!
//! # What it composes with
//!
//! * [`deplete_independent_with_keff_search`] drives the **independent**
//!   (frozen cross-section) one-group operator, [`OneGroupOperator`], through
//!   PETIR's integrators exactly as
//!   [`deplete_independent`](crate::depletion::integrators::deplete_independent)
//!   does. Because the cross sections are frozen, the parameter cannot reach
//!   the depletion through the reaction rates; it reaches it **only** through
//!   `adjust`. For a non-burnable parameter such as boron the burnup history is
//!   therefore the same as without the hook (bit for bit, pinned by
//!   `tests::a_parameter_that_changes_no_burnable_nuclide_leaves_the_burnup_unchanged`).
//! * A transport-coupled spectrum ([`deplete_coupled`](crate::depletion::operator::deplete_coupled))
//!   is not composed with the hook here.
//!
//! The existing drivers are **not edited**. This is a new function in a new
//! file; with no hook there is nothing to call, and the pins in
//! `tests/depletion_move_fingerprints.rs` are the evidence the old drivers did
//! not move.
//!
//! # No trait objects
//!
//! `model` and `adjust` are generic closure parameters, never boxed and never
//! stored in a struct.

use crate::depletion::chain::DepletionChain;
use crate::depletion::integrators::{
    CramSolver, DepletionError, DepletionIntegrator, Integrator, MicroXs, OneGroupOperator,
    SECONDS_PER_DAY,
};
use crate::depletion::operator::{k_inf, BurnupResult, BurnupSettings, BurnupStep};
use crate::physics::keff::KeffResult;
use crate::physics::search::{
    search_for_keff, SearchError, SearchIteration, SearchResult, SearchSettings,
};
use crate::physics::search_grsecant::{grsecant_search_for_keff, GrSecantSettings};
use petir::depletion::TransportOperator;
use std::convert::Infallible;

/// Which search the hook runs at each step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeffSearchMethod {
    /// GRsecant, upstream's `Model.keff_search` — the **default**, as upstream
    /// (`keff_search_control.py:79`). Settings: [`GrSecantSettings`].
    GrSecant,
    /// The workspace's bracketed bisection / false position,
    /// [`search_for_keff`]. Settings: [`SearchSettings`].
    Bracketed,
}

/// The configuration of the hook — upstream's `_KeffSearchControl.__init__`
/// arguments (`keff_search_control.py:31-42`) minus the callbacks, which are
/// passed to [`deplete_independent_with_keff_search`] as closures.
///
/// Plain data: `Clone`, `Debug`, no closures, no borrowed fields.
#[derive(Debug, Clone, Copy)]
pub struct KeffSearchControl {
    x0: f64,
    x1: f64,
    bracket: (f64, f64),
    method: KeffSearchMethod,
    search: SearchSettings,
    grsecant: GrSecantSettings,
    search_at_end: bool,
}

impl KeffSearchControl {
    /// A hook with initial guesses `x0`, `x1` and the absolute bracket
    /// `[bracket.0, bracket.1]` of allowed parameter values.
    ///
    /// The search is **GRsecant** with [`GrSecantSettings::default`] (upstream's
    /// defaults; `x_min`/`x_max` are the bracket, as `keff_search_control.py:41-42`).
    /// Replace the settings with
    /// [`with_grsecant_settings`](Self::with_grsecant_settings), or select the
    /// bracketed search with
    /// [`with_search_settings`](Self::with_search_settings). The final-state
    /// search is on, as upstream (`abc.py:969-972`).
    ///
    /// # Errors
    ///
    /// The same refusals as upstream's constructor
    /// (`keff_search_control.py:32-35`): `bracket.0 >= bracket.1`; and a
    /// non-finite or equal `x0`, `x1` (the bracketed search needs two distinct
    /// starting points, and so does a secant).
    pub fn new(x0: f64, x1: f64, bracket: (f64, f64)) -> Result<Self, String> {
        if !(bracket.0.is_finite() && bracket.1.is_finite()) || bracket.0 >= bracket.1 {
            return Err(format!(
                "bracket.0 must be < bracket.1, got ({}, {})",
                bracket.0, bracket.1
            ));
        }
        if !(x0.is_finite() && x1.is_finite()) || x0 == x1 {
            return Err(format!(
                "x0 and x1 must be finite and distinct, got ({x0}, {x1})"
            ));
        }
        Ok(Self {
            x0,
            x1,
            bracket,
            method: KeffSearchMethod::GrSecant,
            search: SearchSettings::default(),
            grsecant: GrSecantSettings::default(),
            search_at_end: true,
        })
    }

    /// Select the **bracketed** search ([`search_for_keff`]) and set its
    /// settings (target, tolerances, iteration cap, method). The named
    /// alternative to the GRsecant default.
    pub fn with_search_settings(mut self, search: SearchSettings) -> Self {
        self.search = search;
        self.method = KeffSearchMethod::Bracketed;
        self
    }

    /// Select **GRsecant** and set its settings — upstream's `**search_kwargs`
    /// (`keff_search_control.py:27-28`). `x_min` and `x_max` in `grsecant` are
    /// overridden by the bracket, as upstream does (`:41-42`).
    pub fn with_grsecant_settings(mut self, grsecant: GrSecantSettings) -> Self {
        self.grsecant = grsecant;
        self.method = KeffSearchMethod::GrSecant;
        self
    }

    /// Whether to also search on the final inventory, after the last step —
    /// upstream does (`abc.py:969-972`), so it defaults to `true`. Turning it
    /// off saves one search of transport solves and records no final root.
    pub fn with_search_at_end(mut self, search_at_end: bool) -> Self {
        self.search_at_end = search_at_end;
        self
    }

    /// The two initial guesses `(x0, x1)`.
    pub fn initial_guesses(&self) -> (f64, f64) {
        (self.x0, self.x1)
    }

    /// The absolute bracket `(x_min, x_max)`.
    pub fn bracket(&self) -> (f64, f64) {
        self.bracket
    }

    /// Which search runs.
    pub fn method(&self) -> KeffSearchMethod {
        self.method
    }

    /// The bracketed-search settings (used when [`method`](Self::method) is
    /// [`KeffSearchMethod::Bracketed`]).
    pub fn search_settings(&self) -> SearchSettings {
        self.search
    }

    /// The GRsecant settings as given (used when [`method`](Self::method) is
    /// [`KeffSearchMethod::GrSecant`]; the run sets `x_min`/`x_max` from the
    /// bracket).
    pub fn grsecant_settings(&self) -> GrSecantSettings {
        self.grsecant
    }
}

/// What one search found: the parameter that made the step's beginning-of-step
/// inventory critical — upstream's `keff_search_root` stored per step.
#[derive(Debug, Clone)]
pub struct KeffSearchStep {
    /// The burnup step the search ran in front of: `0..n_steps`, and
    /// `n_steps` for the final-state search.
    pub step: usize,
    /// Cumulative burnup time \[days\] at the inventory searched.
    pub time_days: f64,
    /// The parameter found (the caller's units).
    pub parameter: f64,
    /// The k-eigenvalue the model returned at that parameter.
    pub keff: f64,
    /// Its 1-sigma standard error.
    pub keff_std: f64,
    /// Transport solves the search spent, including bracket endpoints (and
    /// the failed first attempt when the absolute bracket was used).
    pub n_evaluations: usize,
    /// Bracketed search only: `true` if `(x0, x1)` did not straddle the target
    /// and the search was redone on the absolute bracket. Always `false` for
    /// GRsecant, which needs no straddle.
    pub used_absolute_bracket: bool,
    /// `true` if the root lies outside `[x_min, x_max]` — upstream warns
    /// (`keff_search_control.py:92-99`). Only a GRsecant seed outside the
    /// bracket can be the root, since proposals are clamped.
    pub root_outside_bracket: bool,
    /// Every evaluation of the successful search, in order. Bracketed: the two
    /// bracket endpoints, then each trial. GRsecant: the two seeds, then each
    /// proposal. Lets a caller read the model's sensitivity `dk/dp` from the
    /// search itself.
    pub iterations: Vec<SearchIteration>,
    /// GRsecant only: the batches of each evaluation, **active plus inactive**
    /// as upstream records them; empty for the bracketed search.
    pub batches: Vec<usize>,
}

/// A burnup history together with the search at every step.
#[derive(Debug, Clone)]
pub struct KeffSearchDepletion {
    /// The burnup record, as [`deplete_independent`](crate::depletion::integrators::deplete_independent)
    /// returns it.
    pub burnup: BurnupResult,
    /// One entry per search, in step order (`n_steps` or `n_steps + 1` of
    /// them; empty when the power is zero, as upstream skips the search when
    /// `source_rate == 0`, `abc.py:886`).
    pub searches: Vec<KeffSearchStep>,
}

/// Why a [`deplete_independent_with_keff_search`] run stopped.
#[derive(Debug, Clone, PartialEq)]
pub enum KeffSearchDepletionError {
    /// The integrator is stochastic-implicit; upstream's hook does not
    /// support those (`abc.py:1400-1429`).
    UnsupportedIntegrator(Integrator),
    /// The cross-section set, or the chain, was refused by the operator.
    Operator(String),
    /// PETIR's integrator or CRAM failed.
    Depletion(String),
    /// Neither `(x0, x1)` nor the absolute bracket straddled the target.
    /// Upstream cannot converge in this case either (`keff_search_control.py:85-89`).
    Bracket {
        /// The step the search ran in front of.
        step: usize,
        /// The refusal from the second (absolute-bracket) attempt.
        error: SearchError,
    },
    /// GRsecant could not continue (degenerate fit, unusable uncertainty,
    /// invalid settings): the message is `GrSecantError`'s.
    Search {
        /// The step the search ran in front of.
        step: usize,
        /// The refusal.
        message: String,
    },
    /// The search ran out of iterations — upstream's `ValueError("Search for
    /// keff failed to converge")` (`keff_search_control.py:85-89`).
    NotConverged {
        /// The step the search ran in front of.
        step: usize,
        /// The best parameter reached, for diagnosis. It is **not** used.
        parameter: f64,
        /// The k there.
        keff: f64,
    },
}

impl std::fmt::Display for KeffSearchDepletionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedIntegrator(i) => write!(
                f,
                "the k-search hook does not support the stochastic-implicit integrator {i:?}"
            ),
            Self::Operator(e) => write!(f, "operator: {e}"),
            Self::Depletion(e) => write!(f, "depletion step failed: {e}"),
            Self::Bracket { step, error } => {
                write!(f, "k search before step {step}: {error}")
            }
            Self::Search { step, message } => write!(f, "k search before step {step}: {message}"),
            Self::NotConverged {
                step,
                parameter,
                keff,
            } => write!(
                f,
                "k search before step {step} failed to converge (best parameter {parameter}, k = {keff})"
            ),
        }
    }
}

impl std::error::Error for KeffSearchDepletionError {}

/// The burnup inventory `(name, density)` for a density vector in chain order.
fn inventory_of(names: &[&str], d: &[f64]) -> Vec<(String, f64)> {
    names
        .iter()
        .zip(d)
        .map(|(n, x)| (n.to_string(), *x))
        .collect()
}

/// Write an adjusted inventory back into a density vector in chain order —
/// upstream's `_update_vec` (`keff_search_control.py:106-134`). Names the chain
/// does not carry are ignored: only burnable nuclides live in `x`.
fn write_back(chain: &DepletionChain, inventory: &[(String, f64)], d: &mut [f64]) {
    for (name, dens) in inventory {
        if let Some(idx) = chain.index_of(name) {
            d[idx] = *dens;
        }
    }
}

/// Run the search on one beginning-of-step inventory — upstream `run`
/// (`keff_search_control.py:44-65`) and `_search_for_keff` (`:67-104`).
///
/// Returns the step record and leaves `d` holding the adjusted inventory
/// (`_update_vec`). Free of the depletion loop so that it can be tested alone.
fn search_step<M, A>(
    chain: &DepletionChain,
    names: &[&str],
    control: &KeffSearchControl,
    step: usize,
    time_days: f64,
    d: &mut [f64],
    model: &mut M,
    adjust: &mut A,
) -> Result<KeffSearchStep, KeffSearchDepletionError>
where
    M: FnMut(f64, &[(String, f64)], usize) -> KeffResult,
    A: FnMut(f64, &mut [(String, f64)]),
{
    let bos = inventory_of(names, d);
    let (x_min, x_max) = control.bracket;
    let found = match control.method {
        KeffSearchMethod::Bracketed => {
            let b0 = control.grsecant.b0;
            let mut n_evaluations = 0usize;
            let mut k_of_p = |p: f64| -> KeffResult {
                n_evaluations += 1;
                let mut trial = bos.clone();
                adjust(p, &mut trial);
                model(p, &trial, b0)
            };
            let mut used_absolute_bracket = false;
            let first = search_for_keff(&mut k_of_p, (control.x0, control.x1), &control.search);
            let result: SearchResult = match first {
                Ok(r) => r,
                Err(SearchError::BracketDoesNotStraddle { .. })
                | Err(SearchError::InvalidBracket { .. }) => {
                    used_absolute_bracket = true;
                    search_for_keff(&mut k_of_p, control.bracket, &control.search)
                        .map_err(|error| KeffSearchDepletionError::Bracket { step, error })?
                }
            };
            if !result.converged {
                return Err(KeffSearchDepletionError::NotConverged {
                    step,
                    parameter: result.parameter,
                    keff: result.keff,
                });
            }
            Found {
                parameter: result.parameter,
                keff: result.keff,
                keff_std: result.keff_std,
                n_evaluations,
                used_absolute_bracket,
                iterations: result.iterations,
                batches: Vec::new(),
            }
        }
        KeffSearchMethod::GrSecant => {
            // `x_min`, `x_max` are the bracket (`keff_search_control.py:41-42`).
            let settings = GrSecantSettings {
                x_min: Some(x_min),
                x_max: Some(x_max),
                ..control.grsecant
            };
            let mut k_of = |p: f64, batches: usize| -> KeffResult {
                let mut trial = bos.clone();
                adjust(p, &mut trial);
                model(p, &trial, batches)
            };
            let r = grsecant_search_for_keff(&mut k_of, control.x0, control.x1, &settings)
                .map_err(|e| KeffSearchDepletionError::Search {
                    step,
                    message: e.to_string(),
                })?;
            if !r.converged {
                return Err(KeffSearchDepletionError::NotConverged {
                    step,
                    parameter: r.root,
                    keff: r.keff(),
                });
            }
            Found {
                parameter: r.root,
                keff: r.keff(),
                keff_std: r.keff_std(),
                n_evaluations: r.function_calls(),
                used_absolute_bracket: false,
                iterations: r
                    .parameters
                    .iter()
                    .zip(&r.keffs)
                    .zip(&r.stdevs)
                    .map(|((p, k), s)| SearchIteration {
                        parameter: *p,
                        keff: *k,
                        keff_std: *s,
                    })
                    .collect(),
                batches: r.batches,
            }
        }
    };

    // `_update_vec`: the inventory the found parameter implies goes back into x.
    let mut adjusted = bos;
    adjust(found.parameter, &mut adjusted);
    write_back(chain, &adjusted, d);

    Ok(KeffSearchStep {
        step,
        time_days,
        parameter: found.parameter,
        keff: found.keff,
        keff_std: found.keff_std,
        n_evaluations: found.n_evaluations,
        used_absolute_bracket: found.used_absolute_bracket,
        root_outside_bracket: found.parameter < x_min || found.parameter > x_max,
        iterations: found.iterations,
        batches: found.batches,
    })
}

/// What either search hands back to [`search_step`].
struct Found {
    parameter: f64,
    keff: f64,
    keff_std: f64,
    n_evaluations: usize,
    used_absolute_bracket: bool,
    iterations: Vec<SearchIteration>,
    batches: Vec<usize>,
}

/// Deplete with frozen one-group cross sections **and a k search before every
/// step** — OpenMC's `IndependentOperator` with
/// `Integrator.add_keff_search_control` (`openmc/deplete/abc.py:1106-1199`,
/// `keff_search_control.py`, `8f79d9d`).
///
/// The loop is that of
/// [`deplete_independent`](crate::depletion::integrators::deplete_independent)
/// (`run_one_group`, itself `Integrator.integrate`, `abc.py:924-985`) with the
/// hook placed where upstream places it: at the beginning of each step,
/// before the operator is evaluated for that step. For step `i`:
///
/// 1. the search (GRsecant by default, bracketed on request, see
///    [`KeffSearchMethod`]) on `|p| model(p, adjust(p, inventory), batches)`;
/// 2. the inventory is replaced by the one the found parameter implies;
/// 3. the operator is evaluated and PETIR's integrator advances the step.
///
/// After the last step the search runs once more on the final inventory
/// (unless [`KeffSearchControl::with_search_at_end`] is `false`); that
/// inventory is recorded as it is and **not** adjusted, since no step follows.
/// With `settings.power_watts == 0.0` no search runs, as upstream
/// (`abc.py:886`).
///
/// # Arguments
///
/// * `model(p, inventory, active_batches)` — the k-eigenvalue of the system
///   with parameter `p` and the (already adjusted) burnable inventory
///   `(name, density in atoms/(barn·cm))`, run with `active_batches` active
///   generations (set `KeffSettings::n_active` from it; GRsecant adapts it,
///   the bracketed search always passes [`GrSecantSettings::b0`]). Nuclides it
///   needs that are not in the chain (oxygen, moderator, boron) it holds itself.
/// * `adjust(p, inventory)` — how `p` changes the burnable inventory; a no-op
///   for a parameter such as boron. Called for every trial, so it must be a
///   pure function of `(p, inventory)`.
///
/// # Errors
///
/// [`KeffSearchDepletionError`]; notably a stochastic-implicit `method`, a
/// bracket that never straddles the target, and a search that does not
/// converge.
pub fn deplete_independent_with_keff_search<M, A>(
    chain: &DepletionChain,
    initial: &[(String, f64)],
    settings: &BurnupSettings,
    micro: &MicroXs,
    method: Integrator,
    control: &KeffSearchControl,
    mut model: M,
    mut adjust: A,
) -> Result<KeffSearchDepletion, KeffSearchDepletionError>
where
    M: FnMut(f64, &[(String, f64)], usize) -> KeffResult,
    A: FnMut(f64, &mut [(String, f64)]),
{
    if method.is_stochastic_implicit() {
        return Err(KeffSearchDepletionError::UnsupportedIntegrator(method));
    }
    let names: Vec<&str> = chain.nuclide_names();
    let mut op = OneGroupOperator::new(chain, micro.xs.clone(), settings)
        .map_err(KeffSearchDepletionError::Operator)?;

    let mut densities = vec![0.0_f64; names.len()];
    for (name, dens) in initial {
        if let Some(idx) = chain.index_of(name) {
            densities[idx] = *dens;
        }
    }

    let record = |step: usize, time_days: f64, flux: f64, d: &[f64]| BurnupStep {
        step,
        time_days,
        flux,
        k_inf: k_inf(d, &micro.xs),
        densities: inventory_of(&names, d),
    };

    let dt = settings.step_days * SECONDS_PER_DAY;
    let power = settings.power_watts;
    let searching = power != 0.0;
    let mut searches: Vec<KeffSearchStep> = Vec::new();
    let mut steps = Vec::with_capacity(settings.n_steps + 1);

    let mut integ = DepletionIntegrator::new(
        method,
        CramSolver::Cram16,
        vec![dt; settings.n_steps],
        vec![power; settings.n_steps],
    )
    .map_err(|e| KeffSearchDepletionError::Depletion(e.to_string()))?;

    let mut n = vec![densities];
    for i in 0..settings.n_steps {
        let time_days = i as f64 * settings.step_days;
        if searching {
            let found = search_step(
                chain,
                &names,
                control,
                i,
                time_days,
                &mut n[0],
                &mut model,
                &mut adjust,
            )?;
            searches.push(found);
        }
        if i == 0 {
            steps.push(record(0, 0.0, 0.0, &n[0]));
        }
        let bos = op
            .evaluate(&n, power)
            .map_err(|e: Infallible| -> KeffSearchDepletionError { match e {} })?;
        let out = integ
            .step(&mut op, &n, &bos.rates, dt, power, i)
            .map_err(|e: DepletionError<Infallible>| match e {
                DepletionError::Numeric(e) => KeffSearchDepletionError::Depletion(e.to_string()),
                DepletionError::Operator(never) => match never {},
            })?;
        n = out.n_end;
        let d = n.first().cloned().unwrap_or_default();
        steps.push(record(
            i + 1,
            (i + 1) as f64 * settings.step_days,
            op.last_flux,
            &d,
        ));
    }

    if searching && control.search_at_end {
        let mut final_inventory = n.first().cloned().unwrap_or_default();
        let found = search_step(
            chain,
            &names,
            control,
            settings.n_steps,
            settings.n_steps as f64 * settings.step_days,
            &mut final_inventory,
            &mut model,
            &mut adjust,
        )?;
        searches.push(found);
    }

    Ok(KeffSearchDepletion {
        burnup: BurnupResult { steps },
        searches,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::depletion::integrators::deplete_independent;

    /// A zero-noise [`KeffResult`] with mean `k` (the literal mirrors
    /// `physics::search::tests::synthetic`).
    fn synthetic(k: f64) -> KeffResult {
        synthetic_std(k, 0.0)
    }

    /// [`synthetic`] with 1-sigma `std`.
    fn synthetic_std(k: f64, std: f64) -> KeffResult {
        KeffResult {
            collisions: 0,
            lost_locate: 0,
            stuck_events: 0,
            stuck_path_cm: 0.0,
            stuck_last_e: 0.0,
            neg_dist: 0,
            neg_level: 0,
            neg_worst: 0.0,
            neg_from_lattice: 0,
            neg_from_surface: 0,
            leak_vacuum: 0,
            leak_infinity: 0,
            histories: 0,
            k_mean: k,
            k_std: std,
            k_by_generation: vec![k],
            entropy: Vec::new(),
            virtual_collisions: 0,
            majorant_violations: 0,
            delta_lost: 0,
        }
    }

    fn setup() -> (DepletionChain, Vec<(String, f64)>, BurnupSettings, MicroXs) {
        let chain = DepletionChain::simple();
        let initial = vec![("U235".to_string(), 1.0e-3), ("U238".to_string(), 2.2e-2)];
        let settings = BurnupSettings {
            n_steps: 4,
            step_days: 5.0,
            ..BurnupSettings::default()
        };
        let micro = MicroXs::from_chain(&chain, &settings);
        (chain, initial, settings, micro)
    }

    fn u235(inv: &[(String, f64)]) -> f64 {
        inv.iter().find(|(n, _)| n == "U235").unwrap().1
    }

    fn bits(r: &BurnupResult) -> Vec<u64> {
        r.steps
            .iter()
            .flat_map(|s| {
                [s.time_days, s.flux, s.k_inf]
                    .into_iter()
                    .chain(s.densities.iter().map(|(_, d)| *d))
                    .map(f64::to_bits)
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Upstream refuses a bracket that is not `lo < hi` (`:32-35`); the port
    /// also refuses unusable guesses.
    #[test]
    fn control_validates_like_upstream() {
        assert!(KeffSearchControl::new(1.0, 2.0, (0.0, 3.0)).is_ok());
        assert!(KeffSearchControl::new(1.0, 2.0, (3.0, 0.0)).is_err());
        assert!(KeffSearchControl::new(1.0, 2.0, (1.0, 1.0)).is_err());
        assert!(KeffSearchControl::new(1.0, 1.0, (0.0, 3.0)).is_err());
        assert!(KeffSearchControl::new(f64::NAN, 1.0, (0.0, 3.0)).is_err());
        let c = KeffSearchControl::new(1.0, 2.0, (0.0, 3.0))
            .unwrap()
            .with_search_at_end(false)
            .with_search_settings(SearchSettings {
                target: 1.05,
                ..SearchSettings::default()
            });
        assert_eq!(c.initial_guesses(), (1.0, 2.0));
        assert_eq!(c.bracket(), (0.0, 3.0));
        assert_eq!(c.search_settings().target, 1.05);
    }

    /// The pin the issue asks for: a hook whose parameter changes no burnable
    /// nuclide leaves the burnup **bit-identical** to the existing driver,
    /// for every non-stochastic integrator. The existing drivers are not
    /// edited; this shows the new loop adds nothing of its own to the
    /// physics.
    #[test]
    fn a_parameter_that_changes_no_burnable_nuclide_leaves_the_burnup_unchanged() {
        let (chain, initial, settings, micro) = setup();
        let control = KeffSearchControl::new(0.0, 10.0, (0.0, 10.0))
            .unwrap()
            .with_search_settings(SearchSettings {
                tol: 1e-6,
                k_tol: 0.0,
                max_iterations: 60,
                ..SearchSettings::default()
            });
        for method in [
            Integrator::Predictor,
            Integrator::CeCm,
            Integrator::Cf4,
            Integrator::CeLi,
            Integrator::EpcRk4,
            Integrator::LeQi,
        ] {
            let reference = deplete_independent(&chain, &initial, &settings, &micro, method).unwrap();
            let hooked = deplete_independent_with_keff_search(
                &chain,
                &initial,
                &settings,
                &micro,
                method,
                &control,
                |p, _, _| synthetic(1.0 + 0.1 * (p - 5.0)),
                |_, _| {},
            )
            .unwrap();
            assert_eq!(bits(&reference), bits(&hooked.burnup), "{method:?}");
            // A search before each of the 4 steps and one at the end.
            assert_eq!(hooked.searches.len(), 5, "{method:?}");
            for s in &hooked.searches {
                assert!((s.parameter - 5.0).abs() < 1e-3, "{method:?}: {s:?}");
            }
        }
    }

    /// The parameter reaches the depletion through `adjust` only. `adjust`
    /// sets U-235 to `p * 1e-3` and the model is critical at U-235 = 1.2e-3,
    /// so every search must find `p = 1.2` and the depletion must start from
    /// the adjusted inventory, not the initial one.
    #[test]
    fn the_found_parameter_is_written_back_into_the_inventory_before_the_step() {
        let (chain, initial, settings, micro) = setup();
        let control = KeffSearchControl::new(0.5, 2.0, (0.1, 5.0))
            .unwrap()
            .with_search_settings(SearchSettings {
                tol: 1e-9,
                k_tol: 0.0,
                max_iterations: 100,
                ..SearchSettings::default()
            });
        // The initial U-235 (1.0e-3) is off the critical value (1.2e-3).
        let result = deplete_independent_with_keff_search(
            &chain,
            &initial,
            &settings,
            &micro,
            Integrator::Predictor,
            &control,
            |_p, inv, _| synthetic(u235(inv) / 1.2e-3),
            |p, inv| {
                for e in inv.iter_mut() {
                    if e.0 == "U235" {
                        e.1 = p * 1.0e-3;
                    }
                }
            },
        )
        .unwrap();
        // The root is p = 1.2 at every step: adjust resets U-235 to 1.2e-3.
        for s in &result.searches {
            assert!((s.parameter - 1.2).abs() < 1e-6, "{s:?}");
        }
        // BOL record is the ADJUSTED inventory, as upstream saves x after run().
        assert!((result.burnup.steps[0].density("U235") - 1.2e-3).abs() < 1e-12);
        // Without the hook BOL is 1.0e-3: the hook changed the history.
        let plain = deplete_independent(&chain, &initial, &settings, &micro, Integrator::Predictor).unwrap();
        assert_ne!(
            plain.steps[1].density("U235").to_bits(),
            result.burnup.steps[1].density("U235").to_bits()
        );
    }

    /// Zero power: upstream skips the search (`abc.py:886`).
    #[test]
    fn zero_power_runs_no_search() {
        let (chain, initial, settings, micro) = setup();
        let settings = BurnupSettings {
            power_watts: 0.0,
            ..settings
        };
        let control = KeffSearchControl::new(0.0, 10.0, (0.0, 10.0))
            .unwrap()
            .with_search_settings(SearchSettings::default());
        let mut calls = 0usize;
        let r = deplete_independent_with_keff_search(
            &chain,
            &initial,
            &settings,
            &micro,
            Integrator::Predictor,
            &control,
            |_, _, _| {
                calls += 1;
                synthetic(1.0)
            },
            |_, _| {},
        )
        .unwrap();
        assert_eq!(calls, 0);
        assert!(r.searches.is_empty());
    }

    /// `with_search_at_end(false)` drops only the final search.
    #[test]
    fn the_final_search_can_be_turned_off() {
        let (chain, initial, settings, micro) = setup();
        let control = KeffSearchControl::new(0.0, 10.0, (0.0, 10.0))
            .unwrap()
            .with_search_at_end(false)
            .with_search_settings(SearchSettings::default());
        let r = deplete_independent_with_keff_search(
            &chain,
            &initial,
            &settings,
            &micro,
            Integrator::Predictor,
            &control,
            |p, _, _| synthetic(1.0 + 0.1 * (p - 5.0)),
            |_, _| {},
        )
        .unwrap();
        assert_eq!(r.searches.len(), settings.n_steps);
        assert_eq!(r.searches.last().unwrap().step, settings.n_steps - 1);
    }

    /// If `(x0, x1)` does not straddle the target the search is retried once on
    /// the absolute bracket; if that fails too the run errors.
    #[test]
    fn a_missed_first_bracket_falls_back_to_the_absolute_bracket_then_errors() {
        let (chain, initial, settings, micro) = setup();
        let model = |p: f64, _: &[(String, f64)], _: usize| synthetic(1.0 + 0.1 * (p - 5.0));
        // Guesses both above the root; absolute bracket straddles.
        let control = KeffSearchControl::new(7.0, 9.0, (0.0, 10.0))
            .unwrap()
            .with_search_settings(SearchSettings::default());
        let r = deplete_independent_with_keff_search(
            &chain,
            &initial,
            &settings,
            &micro,
            Integrator::Predictor,
            &control,
            model,
            |_, _| {},
        )
        .unwrap();
        assert!(r.searches.iter().all(|s| s.used_absolute_bracket));
        // Evaluations include the failed first attempt (2) and the retry.
        assert!(r.searches[0].n_evaluations > 2);
        // Absolute bracket also misses.
        let control = KeffSearchControl::new(7.0, 9.0, (6.0, 10.0))
            .unwrap()
            .with_search_settings(SearchSettings::default());
        let e = deplete_independent_with_keff_search(
            &chain,
            &initial,
            &settings,
            &micro,
            Integrator::Predictor,
            &control,
            model,
            |_, _| {},
        )
        .unwrap_err();
        assert!(matches!(e, KeffSearchDepletionError::Bracket { step: 0, .. }), "{e}");
    }

    /// A search that exhausts its iterations is an error, never a silent
    /// carry-on (`keff_search_control.py:85-89`); stochastic-implicit
    /// integrators are refused.
    #[test]
    fn non_convergence_and_si_integrators_are_errors() {
        let (chain, initial, settings, micro) = setup();
        let control = KeffSearchControl::new(0.0, 10.0, (0.0, 10.0))
            .unwrap()
            .with_search_settings(SearchSettings {
                tol: 1e-12,
                k_tol: 0.0,
                max_iterations: 2,
                ..SearchSettings::default()
            });
        let e = deplete_independent_with_keff_search(
            &chain,
            &initial,
            &settings,
            &micro,
            Integrator::Predictor,
            &control,
            |p, _, _| synthetic(1.0 + 0.1 * (p - 5.0)),
            |_, _| {},
        )
        .unwrap_err();
        assert!(matches!(e, KeffSearchDepletionError::NotConverged { step: 0, .. }), "{e}");
        let e = deplete_independent_with_keff_search(
            &chain,
            &initial,
            &settings,
            &micro,
            Integrator::SiCeLi,
            &control,
            |p, _, _| synthetic(p),
            |_, _| {},
        )
        .unwrap_err();
        assert_eq!(e, KeffSearchDepletionError::UnsupportedIntegrator(Integrator::SiCeLi));
        assert!(e.to_string().contains("stochastic-implicit"));
    }

    /// A cross-section set that does not match the chain is an operator error.
    #[test]
    fn a_mismatched_cross_section_set_is_an_operator_error() {
        let (chain, initial, settings, mut micro) = setup();
        micro.xs.pop();
        let control = KeffSearchControl::new(0.0, 10.0, (0.0, 10.0))
            .unwrap()
            .with_search_settings(SearchSettings::default());
        let e = deplete_independent_with_keff_search(
            &chain,
            &initial,
            &settings,
            &micro,
            Integrator::Predictor,
            &control,
            |p, _, _| synthetic(p),
            |_, _| {},
        )
        .unwrap_err();
        assert!(matches!(e, KeffSearchDepletionError::Operator(_)), "{e}");
    }

    /// `k(p) = 1 + 0.1 (p - 5)` with `sigma = 0.05 / sqrt(batches)`: the noise
    /// law GRsecant's batch fit assumes. Critical at `p = 5`.
    fn noisy(p: f64, batches: usize) -> KeffResult {
        synthetic_std(1.0 + 0.1 * (p - 5.0), 0.05 / (batches as f64).sqrt())
    }

    /// GRsecant is the default, as upstream; the bracketed search is selected
    /// by name and back again.
    #[test]
    fn grsecant_is_the_default_and_the_bracketed_search_is_named() {
        let c = KeffSearchControl::new(1.0, 2.0, (0.0, 3.0)).unwrap();
        assert_eq!(c.method(), KeffSearchMethod::GrSecant);
        assert_eq!(c.grsecant_settings(), GrSecantSettings::default());
        let c = c.with_search_settings(SearchSettings::default());
        assert_eq!(c.method(), KeffSearchMethod::Bracketed);
        let c = c.with_grsecant_settings(GrSecantSettings {
            k_tol: 1e-3,
            ..GrSecantSettings::default()
        });
        assert_eq!(c.method(), KeffSearchMethod::GrSecant);
        assert_eq!(c.grsecant_settings().k_tol, 1e-3);
    }

    /// The default hook finds the root at every step, meets `sigma_final`,
    /// raises the batch count above `b0` to do it, leaves the burnup
    /// bit-identical for a parameter that changes no burnable nuclide, and
    /// records the batches it used.
    #[test]
    fn the_default_grsecant_hook_adapts_batches_and_finds_the_root() {
        let (chain, initial, settings, micro) = setup();
        let control = KeffSearchControl::new(3.0, 8.0, (0.0, 10.0))
            .unwrap()
            .with_grsecant_settings(GrSecantSettings {
                b0: 100,
                inactive: 10,
                ..GrSecantSettings::default()
            });
        let mut max_batches = 0usize;
        let hooked = deplete_independent_with_keff_search(
            &chain,
            &initial,
            &settings,
            &micro,
            Integrator::Predictor,
            &control,
            |p, _, b| {
                max_batches = max_batches.max(b);
                noisy(p, b)
            },
            |_, _| {},
        )
        .unwrap();
        assert!(max_batches > 1000, "{max_batches}");
        assert_eq!(hooked.searches.len(), 5);
        for s in &hooked.searches {
            assert!((s.parameter - 5.0).abs() < 1e-6, "{s:?}");
            assert!(s.keff_std <= 3e-4, "{s:?}");
            assert!(!s.used_absolute_bracket && !s.root_outside_bracket);
            assert_eq!(s.batches.len(), s.n_evaluations);
            assert_eq!(s.iterations.len(), s.n_evaluations);
            assert_eq!(s.batches[0], 110);
        }
        let reference = deplete_independent(&chain, &initial, &settings, &micro, Integrator::Predictor).unwrap();
        assert_eq!(bits(&reference), bits(&hooked.burnup));
    }

    /// The bracketed search always hands the model `b0` and records no batches.
    #[test]
    fn the_bracketed_search_passes_b0_and_records_no_batches() {
        let (chain, initial, settings, micro) = setup();
        let control = KeffSearchControl::new(0.0, 10.0, (0.0, 10.0))
            .unwrap()
            .with_search_settings(SearchSettings::default());
        let b0 = control.grsecant_settings().b0;
        let mut seen = Vec::new();
        let r = deplete_independent_with_keff_search(
            &chain,
            &initial,
            &settings,
            &micro,
            Integrator::Predictor,
            &control,
            |p, _, b| {
                seen.push(b);
                synthetic(1.0 + 0.1 * (p - 5.0))
            },
            |_, _| {},
        )
        .unwrap();
        assert!(!seen.is_empty() && seen.iter().all(|b| *b == b0));
        assert!(r.searches.iter().all(|s| s.batches.is_empty()));
    }

    /// A GRsecant seed outside `[x_min, x_max]` can be the root; upstream warns
    /// (`keff_search_control.py:92-99`), the record flags it.
    #[test]
    fn a_root_outside_the_bracket_is_flagged() {
        let (chain, initial, settings, micro) = setup();
        let control = KeffSearchControl::new(5.0, 8.0, (6.0, 10.0)).unwrap();
        let r = deplete_independent_with_keff_search(
            &chain,
            &initial,
            &settings,
            &micro,
            Integrator::Predictor,
            &control,
            |p, _, _| synthetic_std(1.0 + 0.1 * (p - 5.0), 1e-5),
            |_, _| {},
        )
        .unwrap();
        assert!(r.searches.iter().all(|s| s.root_outside_bracket && s.parameter == 5.0));
    }

    /// Running out of `maxiter` raises (`keff_search_control.py:85-89`); a
    /// slope-free model is a refused fit, not a silent carry-on.
    #[test]
    fn grsecant_non_convergence_and_a_degenerate_fit_are_errors() {
        let (chain, initial, settings, micro) = setup();
        let control = KeffSearchControl::new(3.0, 8.0, (0.0, 10.0))
            .unwrap()
            .with_grsecant_settings(GrSecantSettings {
                k_tol: 1e-12,
                maxiter: 3,
                ..GrSecantSettings::default()
            });
        let e = deplete_independent_with_keff_search(
            &chain,
            &initial,
            &settings,
            &micro,
            Integrator::Predictor,
            &control,
            |p, _, b| noisy(p, b),
            |_, _| {},
        )
        .unwrap_err();
        assert!(matches!(e, KeffSearchDepletionError::NotConverged { step: 0, .. }), "{e}");
        let control = KeffSearchControl::new(3.0, 8.0, (0.0, 10.0)).unwrap();
        let e = deplete_independent_with_keff_search(
            &chain,
            &initial,
            &settings,
            &micro,
            Integrator::Predictor,
            &control,
            |_, _, b| synthetic_std(1.2, 0.05 / (b as f64).sqrt()),
            |_, _| {},
        )
        .unwrap_err();
        assert!(matches!(e, KeffSearchDepletionError::Search { step: 0, .. }), "{e}");
        assert!(e.to_string().contains("no usable slope"));
    }
}
