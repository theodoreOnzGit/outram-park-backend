// SPDX-License-Identifier: GPL-3.0

//! **GRsecant criticality search** — a noise-aware, adaptive-statistics secant
//! search for the parameter `x` at which `k_eff(x) = target`. GitHub #1618
//! item 1 (gaps left by #1616), epic #257.
//!
//! This is a port of OpenMC's `Model.keff_search`
//! (`openmc/model/model.py:3248-3405`, commit
//! `8f79d9d02483c90e4134f03a9df4b75ce6b062bf`, read from
//! `crates/outram-mc-libs/upstream_source/OpenMC/`), which implements the
//! GRsecant method of Price and Roskoff (doi:10.1016/j.pnucene.2023.104731).
//! It sits beside [`search_for_keff`](crate::physics::search::search_for_keff)
//! (the port of the older `openmc.search_for_keff`, bracketed bisection or false
//! position), which is **not edited**; the two are alternatives, and
//! [`crate::depletion::keff_search`] selects between them by name.
//!
//! # What upstream does (read first)
//!
//! * **Seeds** (`model.py:3399-3405`): evaluate `x0` then `x1`, each with `b0`
//!   active batches; return at once if either already has `|f| <= k_tol` and
//!   `sigma <= sigma_final`. `f = k - target`.
//! * **Step 1, the next point** (`:3408-3427`): a weighted linear fit
//!   `f(x) = a + b x` of the last `memory` evaluations with
//!   `sigma=ss, absolute_sigma=True` (scipy `curve_fit`), `x_new = -a / b`,
//!   clamped to `[x_min, x_max]` where given. Here the fit is the closed-form
//!   weighted least squares with weights `1 / sigma^2`, which is the same
//!   minimiser.
//! * **Step 2, the target sigma** (`:3429-3434`, Eq. 8 of the paper):
//!   `sig = q * sigma_final * (min|f| / k_tol)^p`, floored at
//!   `q * sigma_final`; `min|f|` is over **all** evaluations so far.
//! * **Step 3, the batches** (`:3436-3459`, Appendix C): `sigma ~ K / sqrt(b)`
//!   with the exponent fixed at `r = 0.5`. When at least two evaluations exist
//!   and `var(ln gs) > 0` over **all** of them, `ln K` is the least-squares
//!   fit of `ln s = ln K - 0.5 ln b` over the **last 4**, which for one
//!   free parameter is the mean of `ln s + 0.5 ln b`; otherwise `K = s * sqrt(b)`
//!   of the last evaluation. Then `b_new = ceil((K / sig)^2)`, at least `b_min`,
//!   at most `b_max` if given.
//! * **Stop** (`:3461-3466`): both `|f| <= k_tol` and `sigma <= sigma_final`
//!   (the seeds use the same test); after at most `maxiter` evaluations
//!   (`range(maxiter - 2)` more after the two seeds) it returns the **last
//!   evaluated** `x` with `converged = false` and flag `"maxiter"`.
//! * `batches` handed to the model is `b + inactive` (`:3335`); upstream stores
//!   that **total** in `gs`, and later feeds the fitted `b_new`, which is a
//!   total-batch estimate, back through `eval_at`, which adds `inactive`
//!   again. This port keeps that quirk: [`GrSecantResult::batches`] holds
//!   `active + inactive`, as upstream's `SearchResult.batches` does.
//!
//! # How this port differs (stated, not hidden)
//!
//! | upstream | here |
//! |---|---|
//! | `func(x)` edits the `openmc.lib` model, then `lib.run` | the caller's closure `model(x, active_batches) -> KeffResult` builds and runs its own model |
//! | `b0=None` reads the model's `batches - inactive` | [`GrSecantSettings::b0`] is required (default 50) and [`GrSecantSettings::inactive`] is the model's inactive count |
//! | `SearchResult.means` holds `f = k - target` (`:3343`) | [`GrSecantResult::keffs`] holds `k` itself, so it reads like [`SearchIteration`](crate::physics::search::SearchIteration); `k - target` is a subtraction away |
//! | `curve_fit` on a singular or zero-sigma fit warns and returns whatever scipy gives | [`GrSecantError::DegenerateFit`] / [`GrSecantError::NonPositiveSigma`]: a search is not continued on a number with no meaning |
//! | `maxiter` below 2 silently runs the two seeds | refused ([`GrSecantError::InvalidSettings`]) |
//!
//! The workspace's k-eigenvalue drivers take the active-generation count in
//! `KeffSettings::n_active`, so the adaptive batch count is supported: the
//! caller's closure sets `n_active` from the `usize` it is handed.
//!
//! # No trait objects
//!
//! The model is a generic `FnMut(f64, usize) -> KeffResult`, never boxed.

use crate::physics::keff::KeffResult;
use petir::mathf::RealMath;

/// Settings of a [`grsecant_search_for_keff`] run — the keyword arguments of
/// upstream's `Model.keff_search` (`model.py:3248-3270`) with upstream's
/// defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GrSecantSettings {
    /// The eigenvalue searched for (upstream `target`, 1.0).
    pub target: f64,
    /// Stopping criterion on `|k - target|` (upstream `k_tol`, 1e-4).
    pub k_tol: f64,
    /// Largest accepted uncertainty of the final k (upstream `sigma_final`,
    /// 3e-4).
    pub sigma_final: f64,
    /// Exponent of Eq. (8) (upstream `p`, 0.5).
    pub p: f64,
    /// Multiplicative factor of Eq. (8), below 1 (upstream `q`, 0.95).
    pub q: f64,
    /// Number of most-recent points in the weighted linear fit (upstream
    /// `memory`, 4). At least 2.
    pub memory: usize,
    /// Lower clamp on `x` (upstream `x_min`); `None` for none.
    pub x_min: Option<f64>,
    /// Upper clamp on `x` (upstream `x_max`); `None` for none.
    pub x_max: Option<f64>,
    /// Active batches of the two seed evaluations (upstream `b0`, read from
    /// the model when `None`; here it must be given).
    pub b0: usize,
    /// Fewest active batches of any later evaluation (upstream `b_min`, 20).
    pub b_min: usize,
    /// Most active batches of any later evaluation (upstream `b_max`);
    /// `None` for no cap.
    pub b_max: Option<usize>,
    /// The model's inactive batches, added to every count recorded in
    /// [`GrSecantResult::batches`] (upstream `self.settings.inactive`,
    /// `:3335`).
    pub inactive: usize,
    /// Most evaluations, seeds included (upstream `maxiter`, 50). At least 2.
    pub maxiter: usize,
}

impl Default for GrSecantSettings {
    /// Upstream's defaults, with `b0 = 50` and `inactive = 10` standing in for
    /// the model settings upstream reads. Set both to the model's own.
    fn default() -> Self {
        Self {
            target: 1.0,
            k_tol: 1e-4,
            sigma_final: 3e-4,
            p: 0.5,
            q: 0.95,
            memory: 4,
            x_min: None,
            x_max: None,
            b0: 50,
            b_min: 20,
            b_max: None,
            inactive: 10,
            maxiter: 50,
        }
    }
}

/// Why a GRsecant run ended — upstream's `SearchResult.flag`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrSecantFlag {
    /// Both `|f| <= k_tol` and `sigma <= sigma_final` were met (`"converged"`).
    Converged,
    /// `maxiter` evaluations were spent (`"maxiter"`).
    MaxIter,
}

/// The outcome of a GRsecant search — upstream's `SearchResult`
/// (`model.py:3467-3500`).
#[derive(Debug, Clone)]
pub struct GrSecantResult {
    /// The last evaluated parameter (upstream `root`): the converged one, or
    /// on `maxiter` the last tried.
    pub root: f64,
    /// Every parameter evaluated, in order.
    pub parameters: Vec<f64>,
    /// The k at each (upstream stores `k - target` here, see the module table).
    pub keffs: Vec<f64>,
    /// The 1-sigma of each k.
    pub stdevs: Vec<f64>,
    /// Batches of each evaluation, **active plus inactive**.
    pub batches: Vec<usize>,
    /// Whether both stopping criteria were met.
    pub converged: bool,
    /// Why the search ended.
    pub flag: GrSecantFlag,
}

impl GrSecantResult {
    /// Number of function evaluations (upstream `function_calls`).
    pub fn function_calls(&self) -> usize {
        self.parameters.len()
    }

    /// The k at [`root`](Self::root).
    pub fn keff(&self) -> f64 {
        self.keffs.last().copied().unwrap_or(f64::NAN)
    }

    /// The 1-sigma of the k at [`root`](Self::root).
    pub fn keff_std(&self) -> f64 {
        self.stdevs.last().copied().unwrap_or(f64::NAN)
    }
}

/// Why a GRsecant search could not continue.
#[derive(Debug, Clone, PartialEq)]
pub enum GrSecantError {
    /// A setting upstream would refuse or that has no meaning: `memory < 2`
    /// (`model.py:3382`, `ValueError("memory must be ≥ 2")`), `maxiter < 2`,
    /// a non-finite target or tolerance, `b0 == 0`.
    InvalidSettings(String),
    /// The weighted fit has no slope: every point in the fit window has the
    /// same `x`, or the fitted slope is zero or not finite, so
    /// `x = -a / b` does not exist.
    DegenerateFit {
        /// The evaluation count when it happened.
        evaluations: usize,
    },
    /// An evaluation reported a non-positive or non-finite uncertainty where
    /// the fit needs `1 / sigma^2` and `ln sigma`.
    NonPositiveSigma {
        /// The parameter whose evaluation had it.
        parameter: f64,
        /// The uncertainty it reported.
        sigma: f64,
    },
}

impl std::fmt::Display for GrSecantError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSettings(s) => write!(f, "invalid GRsecant settings: {s}"),
            Self::DegenerateFit { evaluations } => write!(
                f,
                "GRsecant weighted fit has no usable slope after {evaluations} evaluations"
            ),
            Self::NonPositiveSigma { parameter, sigma } => write!(
                f,
                "GRsecant needs a positive uncertainty, got {sigma} at parameter {parameter}"
            ),
        }
    }
}

impl std::error::Error for GrSecantError {}

fn validate(s: &GrSecantSettings) -> Result<(), GrSecantError> {
    let bad = |m: &str| Err(GrSecantError::InvalidSettings(m.to_string()));
    if s.memory < 2 {
        return bad("memory must be >= 2");
    }
    if s.maxiter < 2 {
        return bad("maxiter must be >= 2 (the two seed evaluations)");
    }
    if s.b0 == 0 {
        return bad("b0 must be >= 1");
    }
    if !(s.target.is_finite() && s.k_tol.is_finite() && s.sigma_final.is_finite()) {
        return bad("target, k_tol and sigma_final must be finite");
    }
    if !(s.p.is_finite() && s.q.is_finite() && s.q > 0.0) {
        return bad("p must be finite and q positive");
    }
    Ok(())
}

/// The weighted least-squares line `f(x) = a + b x` through `(xs, fs)` with
/// weights `1 / sigma^2` (scipy `curve_fit(..., sigma, absolute_sigma=True)`),
/// returned as the root `-a / b`. `None` when the fit is singular.
fn weighted_root(xs: &[f64], fs: &[f64], ss: &[f64]) -> Option<f64> {
    let w: Vec<f64> = ss.iter().map(|s| 1.0 / (s * s)).collect();
    let sw: f64 = w.iter().sum();
    let xbar = xs.iter().zip(&w).map(|(x, w)| x * w).sum::<f64>() / sw;
    let ybar = fs.iter().zip(&w).map(|(y, w)| y * w).sum::<f64>() / sw;
    let sxx: f64 = xs.iter().zip(&w).map(|(x, w)| w * (x - xbar) * (x - xbar)).sum();
    let sxy: f64 = xs
        .iter()
        .zip(fs)
        .zip(&w)
        .map(|((x, y), w)| w * (x - xbar) * (y - ybar))
        .sum();
    if !(sxx > 0.0) || !sxx.is_finite() {
        return None;
    }
    let b = sxy / sxx;
    if b == 0.0 || !b.is_finite() {
        return None;
    }
    let root = xbar - ybar / b;
    root.is_finite().then_some(root)
}

/// Population variance of `ln(g)`, as `np.var(np.log(gs))`.
fn var_ln(gs: &[usize]) -> f64 {
    let l: Vec<f64> = gs.iter().map(|g| (*g as f64).r_ln()).collect();
    let m = l.iter().sum::<f64>() / l.len() as f64;
    l.iter().map(|v| (v - m) * (v - m)).sum::<f64>() / l.len() as f64
}

/// Search for the parameter `x` with `k_eff(x) = settings.target` by GRsecant —
/// upstream `Model.keff_search` (`openmc/model/model.py:3248-3405`).
///
/// `model(x, active_batches)` runs the k-eigenvalue problem at parameter `x`
/// with `active_batches` active generations (set `KeffSettings::n_active` from
/// it) and returns the result; its `k_mean` and `k_std` are the evaluation.
/// The search is unbracketed, so `x0` and `x1` need not straddle the target;
/// `settings.x_min` and `settings.x_max` clamp every **proposed** point (the
/// seeds are evaluated as given, as upstream does).
///
/// # Errors
///
/// [`GrSecantError`]. A search that merely runs out of `maxiter` is **not** an
/// error: it returns `Ok` with `converged = false` and flag
/// [`GrSecantFlag::MaxIter`], and the caller decides (upstream's depletion hook
/// raises, `keff_search_control.py:85-89`).
pub fn grsecant_search_for_keff<F>(
    mut model: F,
    x0: f64,
    x1: f64,
    settings: &GrSecantSettings,
) -> Result<GrSecantResult, GrSecantError>
where
    F: FnMut(f64, usize) -> KeffResult,
{
    validate(settings)?;
    let mut xs: Vec<f64> = Vec::new();
    let mut fs: Vec<f64> = Vec::new();
    let mut ss: Vec<f64> = Vec::new();
    let mut gs: Vec<usize> = Vec::new();
    let mut keffs: Vec<f64> = Vec::new();

    // Evaluate and record, as upstream's `eval_at` (`:3328-3354`).
    let mut eval_at = |xs: &mut Vec<f64>,
                       fs: &mut Vec<f64>,
                       ss: &mut Vec<f64>,
                       gs: &mut Vec<usize>,
                       keffs: &mut Vec<f64>,
                       x: f64,
                       active: usize|
     -> (f64, f64) {
        let r = model(x, active);
        xs.push(x);
        keffs.push(r.k_mean);
        fs.push(r.k_mean - settings.target);
        ss.push(r.k_std);
        gs.push(active + settings.inactive);
        (r.k_mean - settings.target, r.k_std)
    };
    let finish = |xs: Vec<f64>,
                  keffs: Vec<f64>,
                  ss: Vec<f64>,
                  gs: Vec<usize>,
                  converged: bool| GrSecantResult {
        root: xs.last().copied().unwrap_or(f64::NAN),
        parameters: xs,
        keffs,
        stdevs: ss,
        batches: gs,
        converged,
        flag: if converged {
            GrSecantFlag::Converged
        } else {
            GrSecantFlag::MaxIter
        },
    };

    // Seeds (`:3399-3405`).
    let (f0, s0) = eval_at(&mut xs, &mut fs, &mut ss, &mut gs, &mut keffs, x0, settings.b0);
    if f0.abs() <= settings.k_tol && s0 <= settings.sigma_final {
        return Ok(finish(xs, keffs, ss, gs, true));
    }
    let (f1, s1) = eval_at(&mut xs, &mut fs, &mut ss, &mut gs, &mut keffs, x1, settings.b0);
    if f1.abs() <= settings.k_tol && s1 <= settings.sigma_final {
        return Ok(finish(xs, keffs, ss, gs, true));
    }

    for _ in 0..settings.maxiter - 2 {
        // Step 1 (`:3408-3427`): the weighted fit over the last `memory` points.
        let m = settings.memory.min(xs.len());
        let lo = xs.len() - m;
        for (x, s) in xs[lo..].iter().zip(&ss[lo..]) {
            if !(*s > 0.0) || !s.is_finite() {
                return Err(GrSecantError::NonPositiveSigma {
                    parameter: *x,
                    sigma: *s,
                });
            }
        }
        let mut x_new = weighted_root(&xs[lo..], &fs[lo..], &ss[lo..]).ok_or(
            GrSecantError::DegenerateFit {
                evaluations: xs.len(),
            },
        )?;
        if let Some(lo_x) = settings.x_min {
            x_new = x_new.max(lo_x);
        }
        if let Some(hi_x) = settings.x_max {
            x_new = x_new.min(hi_x);
        }

        // Step 2 (`:3429-3434`): Eq. (8), floored at `q * sigma_final`.
        let min_abs_f = fs.iter().fold(f64::INFINITY, |a, f| a.min(f.abs()));
        let base = settings.q * settings.sigma_final;
        let ratio = if settings.k_tol > 0.0 {
            min_abs_f / settings.k_tol
        } else {
            1.0
        };
        let sig_target = (base * ratio.r_powf(settings.p)).max(base);

        // Step 3 (`:3436-3459`): batches for that sigma, r = 0.5.
        let k = if gs.len() >= 2 && var_ln(&gs) > 0.0 {
            let tail = gs.len().min(4);
            let t0 = gs.len() - tail;
            let ln_k = gs[t0..]
                .iter()
                .zip(&ss[t0..])
                .map(|(g, s)| s.r_ln() + 0.5 * (*g as f64).r_ln())
                .sum::<f64>()
                / tail as f64;
            ln_k.r_exp()
        } else {
            let last = gs.len() - 1;
            ss[last] * (gs[last] as f64).sqrt()
        };
        let b_float = (k / sig_target).r_powi(2).ceil();
        let mut b_new = if b_float.is_finite() && b_float < usize::MAX as f64 {
            (b_float as usize).max(settings.b_min)
        } else {
            usize::MAX
        };
        if let Some(b_max) = settings.b_max {
            b_new = b_new.min(b_max);
        }

        let (f_new, s_new) =
            eval_at(&mut xs, &mut fs, &mut ss, &mut gs, &mut keffs, x_new, b_new);
        if f_new.abs() <= settings.k_tol && s_new <= settings.sigma_final {
            return Ok(finish(xs, keffs, ss, gs, true));
        }
    }
    Ok(finish(xs, keffs, ss, gs, false))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A [`KeffResult`] with mean `k` and 1-sigma `std`.
    fn result(k: f64, std: f64) -> KeffResult {
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

    /// `k(x) = 1 + 0.1 (x - 5)` with `sigma = 0.05 / sqrt(batches)`: noise
    /// shrinks as `1 / sqrt(b)`, the model GRsecant's step 3 assumes.
    fn linear(x: f64, b: usize) -> KeffResult {
        result(1.0 + 0.1 * (x - 5.0), 0.05 / (b as f64).sqrt())
    }

    fn settings() -> GrSecantSettings {
        GrSecantSettings {
            b0: 100,
            inactive: 10,
            ..GrSecantSettings::default()
        }
    }

    /// On an exactly linear model the first fit is the exact root, but its
    /// evaluation is given only `b_min` batches (Eq. 8 is loose while `|f|`
    /// is large: `sig = q sigma_final sqrt(2000)`, about 0.013), so `sigma`
    /// still exceeds `sigma_final`. The second fit lands on the same root,
    /// `min|f|` is now zero so the target sigma drops to `q sigma_final`, and
    /// the fourth evaluation gets thousands of batches and stops the search.
    /// (Predicted before running: three evaluations. Wrong; the floor
    /// `b_min` and the loose early target are why, and the run is correct.)
    #[test]
    fn a_linear_model_needs_one_more_evaluation_to_meet_sigma_final() {
        let r = grsecant_search_for_keff(linear, 3.0, 8.0, &settings()).unwrap();
        assert!(r.converged);
        assert_eq!(r.flag, GrSecantFlag::Converged);
        assert_eq!(r.function_calls(), 4);
        assert!((r.root - 5.0).abs() < 1e-12, "{}", r.root);
        assert!(r.keff_std() <= 3e-4, "sigma {}", r.keff_std());
        assert_eq!(r.batches[0], 110);
        assert_eq!(r.batches[1], 110);
        assert_eq!(r.batches[2], 30);
        assert!(r.batches[3] > 10 * r.batches[0], "{:?}", r.batches);
        assert_eq!(r.keff(), r.keffs[3]);
        assert!((r.parameters[2] - r.parameters[3]).abs() < 1e-12);
    }

    /// Steps 2 and 3 reproduced by hand. First proposal: the seeds have
    /// `|f| = 0.2, 0.3`, `sig = q sigma_final sqrt(2000)`, `K = 0.005
    /// sqrt(110)` (the two seeds share `gs`, so upstream's branch is
    /// `K = s sqrt(gs)`), giving 17, floored to `b_min = 20`. Second: `min|f| =
    /// 0`, `sig = q sigma_final`, and `ln K` is the mean of `ln s + 0.5 ln g`
    /// over the last four evaluations.
    #[test]
    fn the_batch_count_follows_equation_eight() {
        let r = grsecant_search_for_keff(linear, 3.0, 8.0, &settings()).unwrap();
        let k0 = 0.05 / 100f64.sqrt() * 110f64.sqrt();
        let sig0 = 0.95 * 3e-4 * (0.2f64 / 1e-4).powf(0.5);
        assert_eq!(((k0 / sig0).powi(2).ceil() as usize).max(20) + 10, r.batches[2]);
        let ln_k = r
            .batches
            .iter()
            .take(3)
            .zip(&r.stdevs)
            .map(|(g, s)| s.ln() + 0.5 * (*g as f64).ln())
            .sum::<f64>()
            / 3.0;
        let sig1 = 0.95 * 3e-4;
        let expect = ((ln_k.exp() / sig1).powi(2).ceil() as usize).max(20);
        assert_eq!(r.batches[3], expect + 10);
    }

    /// A seed that already meets both criteria returns after one (or two)
    /// evaluations, as upstream does.
    #[test]
    fn a_good_seed_returns_immediately() {
        let r = grsecant_search_for_keff(
            |x, _| result(1.0 + 0.1 * (x - 5.0), 1e-4),
            5.0,
            9.0,
            &settings(),
        )
        .unwrap();
        assert!(r.converged);
        assert_eq!(r.function_calls(), 1);
        assert_eq!(r.root, 5.0);
        let r = grsecant_search_for_keff(
            |x, _| result(1.0 + 0.1 * (x - 5.0), 1e-4),
            9.0,
            5.0,
            &settings(),
        )
        .unwrap();
        assert_eq!(r.function_calls(), 2);
        assert_eq!(r.root, 5.0);
    }

    /// The proposed point is clamped to `[x_min, x_max]`. The root lies below
    /// `x_min`, so the first proposal is the bound; a window that then holds
    /// only repeats of it is a degenerate fit, reported rather than looped.
    #[test]
    fn a_root_outside_the_clamp_is_pinned_to_it_then_reported() {
        let s = GrSecantSettings {
            x_min: Some(6.0),
            maxiter: 3,
            ..settings()
        };
        let r = grsecant_search_for_keff(linear, 7.0, 8.0, &s).unwrap();
        assert!(!r.converged);
        assert_eq!(r.flag, GrSecantFlag::MaxIter);
        assert_eq!(r.root, 6.0);
        assert_eq!(r.function_calls(), 3);
        // x_max clamps the other way.
        let s = GrSecantSettings {
            x_max: Some(4.0),
            maxiter: 3,
            ..settings()
        };
        let r = grsecant_search_for_keff(linear, 1.0, 2.0, &s).unwrap();
        assert_eq!(r.root, 4.0);
        // Memory 2 and an unchanged window: the third proposal has two equal x.
        let s = GrSecantSettings {
            x_min: Some(6.0),
            memory: 2,
            maxiter: 6,
            ..settings()
        };
        let e = grsecant_search_for_keff(linear, 7.0, 8.0, &s).unwrap_err();
        assert!(matches!(e, GrSecantError::DegenerateFit { .. }), "{e}");
    }

    /// `maxiter` is the total evaluation budget, seeds included, and running
    /// out of it is a result with `converged = false`, not an error.
    #[test]
    fn maxiter_is_a_flagged_result() {
        let r = grsecant_search_for_keff(
            |x, b| {
                result(
                    1.0 + 0.1 * (x - 5.0) + 0.01 * x.sin(),
                    0.05 / (b as f64).sqrt(),
                )
            },
            3.0,
            8.0,
            &GrSecantSettings {
                k_tol: 1e-12,
                maxiter: 5,
                ..settings()
            },
        )
        .unwrap();
        assert!(!r.converged);
        assert_eq!(r.flag, GrSecantFlag::MaxIter);
        assert_eq!(r.function_calls(), 5);
        assert_eq!(r.root, *r.parameters.last().unwrap());
    }

    /// `b_min` and `b_max` bound the adaptive count.
    #[test]
    fn batches_are_clamped_to_b_min_and_b_max() {
        let r = grsecant_search_for_keff(
            linear,
            3.0,
            8.0,
            &GrSecantSettings {
                b_max: Some(15),
                b_min: 1,
                maxiter: 4,
                ..settings()
            },
        )
        .unwrap();
        // The target batches of the 4th evaluation are in the thousands; the
        // cap holds them at 15 (+10 inactive).
        assert_eq!(r.batches[3], 25);
        // A loose sigma_final: the target batches fall below b_min.
        let r = grsecant_search_for_keff(
            linear,
            3.0,
            8.0,
            &GrSecantSettings {
                sigma_final: 10.0,
                k_tol: 1e-9,
                b0: 1000,
                ..settings()
            },
        )
        .unwrap();
        assert_eq!(r.batches[2], 20 + 10);
    }

    /// With three or more points the fit is weighted: a very noisy outlier
    /// barely moves it, and equal sigmas give the ordinary line.
    #[test]
    fn the_fit_is_weighted_by_inverse_variance() {
        let xs = [3.0, 4.0, 6.0, 10.0];
        let fs = [-0.2, -0.1, 0.1, 5.0];
        let ss = [1e-3, 1e-3, 1e-3, 1e3];
        let root = weighted_root(&xs, &fs, &ss).unwrap();
        assert!((root - 5.0).abs() < 1e-5, "{root}");
        let root_eq = weighted_root(&xs, &fs, &[1.0; 4]).unwrap();
        assert!((root_eq - 5.0).abs() > 0.5, "{root_eq}");
        // Two points: the exact secant.
        let r2 = weighted_root(&[2.0, 4.0], &[-0.3, 0.1], &[0.5, 0.01]).unwrap();
        assert!((r2 - 3.5).abs() < 1e-12);
        // Singular: one x, or a flat line.
        assert!(weighted_root(&[2.0, 2.0], &[1.0, 2.0], &[1.0, 1.0]).is_none());
        assert!(weighted_root(&[1.0, 2.0], &[1.0, 1.0], &[1.0, 1.0]).is_none());
    }

    /// The population variance of `ln g`, and the settings refusals.
    #[test]
    fn variance_and_settings_refusals() {
        assert_eq!(var_ln(&[10, 10, 10]), 0.0);
        assert!(var_ln(&[10, 100]) > 0.0);
        for bad in [
            GrSecantSettings { memory: 1, ..settings() },
            GrSecantSettings { maxiter: 1, ..settings() },
            GrSecantSettings { b0: 0, ..settings() },
            GrSecantSettings { k_tol: f64::NAN, ..settings() },
            GrSecantSettings { q: 0.0, ..settings() },
        ] {
            let e = grsecant_search_for_keff(linear, 3.0, 8.0, &bad).unwrap_err();
            assert!(matches!(e, GrSecantError::InvalidSettings(_)), "{e}");
            assert!(e.to_string().contains("invalid GRsecant settings"));
        }
    }

    /// A zero uncertainty cannot be weighted: refused with the parameter.
    #[test]
    fn a_zero_sigma_is_refused() {
        let e = grsecant_search_for_keff(
            |x, _| result(1.0 + 0.1 * (x - 5.0), 0.0),
            3.0,
            8.0,
            &settings(),
        )
        .unwrap_err();
        assert!(matches!(e, GrSecantError::NonPositiveSigma { .. }), "{e}");
        assert!(e.to_string().contains("positive uncertainty"));
        assert!(GrSecantError::DegenerateFit { evaluations: 2 }
            .to_string()
            .contains("no usable slope"));
    }

    /// On a mildly nonlinear model whose mean wobbles by an amount that
    /// shrinks with the batches, the search still converges, in more than the
    /// single fit a linear model needs.
    #[test]
    fn a_nonlinear_noisy_model_converges() {
        let model = |x: f64, b: usize| {
            let d = x - 5.0;
            let wobble = 0.01 * (x * 7.0).sin() / (b as f64).sqrt();
            result(
                1.0 + 0.1 * d + 0.004 * d * d + wobble,
                0.05 / (b as f64).sqrt(),
            )
        };
        let r = grsecant_search_for_keff(model, 3.0, 8.0, &settings()).unwrap();
        assert!(r.converged, "{:?}", r.parameters);
        assert!(r.keff_std() <= 3e-4);
        assert!((r.keff() - 1.0).abs() <= 1e-4);
        assert!(r.function_calls() > 3);
    }
}
