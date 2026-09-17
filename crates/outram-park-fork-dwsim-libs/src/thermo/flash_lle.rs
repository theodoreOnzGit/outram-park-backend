//! **Simple liquid-liquid equilibrium** (LLE) isothermal split at fixed
//! temperature, driven by an activity-coefficient model.
//!
//! Ported from DWSIM `DWSIM.Thermodynamics/FlashAlgorithms/SimpleLLE.vb`
//! (`Flash_PT`, lines 82-330), GPL-3.0, commit `1abf72d`. Specific ported lines
//! are cited at each function below. The vapour-free, energy-flash paths
//! (`Flash_PH`/`Flash_PS`/`Flash_TV`/`Flash_PV`) of the DWSIM class are **not**
//! ported here — see *Honest scope*.
//!
//! # Provenance
//!
//! ```text
//! Upstream project : DWSIM (Daniel Wagner O. de Medeiros; Gregor Reichert)
//! Source file      : DWSIM.Thermodynamics/FlashAlgorithms/SimpleLLE.vb
//! Commit           : 1abf72d
//! Licence          : GPL-3.0
//! ```
//!
//! # What this module computes
//!
//! Given a single-liquid feed of overall mole fractions `z_i` \[-\] at a fixed
//! temperature `T` \[K\], split it (if it is unstable) into two coexisting liquid
//! phases — phase I (`x^{I}_i`, molar fraction `L^{I}`) and phase II (`x^{II}_i`,
//! molar fraction `L^{II}`) with `L^{I} + L^{II} = 1` — in mutual equilibrium.
//! For an activity-coefficient description the equilibrium (isoactivity)
//! condition is
//!
//! ```text
//! gamma_i^{I}(x^{I}, T) x^{I}_i = gamma_i^{II}(x^{II}, T) x^{II}_i   for every i,
//! ```
//!
//! i.e. the **activity** `a_i = gamma_i x_i` of each species is equal in the two
//! liquids. When the feed is a stable single liquid, no split exists and the
//! flash reports one phase.
//!
//! ## Why there is no pressure argument
//!
//! DWSIM's `SimpleLLE.Flash_PT` takes `P` and forms the activity coefficient as
//! `gamma_i = P / Vp_i · phi_i` from its liquid **fugacity-coefficient** call
//! (`SimpleLLE.vb` lines 214-224): the `P` and the vapour pressure `Vp_i` cancel
//! exactly, leaving the liquid activity coefficient. This port takes the activity
//! coefficient **directly** from [`crate::thermo::activity::ActivityModel`], so
//! `P` never enters. Physically, at the low-to-moderate pressures where an
//! incompressible-liquid activity model applies, `gamma_i` is pressure-
//! independent, so the LLE split at fixed `T` depends only on `T` and `z`. The
//! API therefore takes `t` only; a caller wanting the DWSIM "PT" framing supplies
//! the same `T` and any `P` — the split is unchanged.
//!
//! # Method (successive substitution, DWSIM `SimpleLLE.vb` lines 194-285)
//!
//! With the liquid-liquid distribution ratio written through the activity
//! coefficients as `K_i = x^{I}_i / x^{II}_i = gamma_i^{II} / gamma_i^{I}`, the
//! per-component material balance `n^{I}_i + n^{II}_i = z_i` (mole numbers per
//! mole of feed) with `x^{I}_i = n^{I}_i / L^{I}`, `x^{II}_i = n^{II}_i / L^{II}`
//! gives the closed update
//!
//! ```text
//! n^{I}_i = z_i / ( 1 + gamma_i^{I} L^{II} / (gamma_i^{II} L^{I}) ),
//! n^{II}_i = z_i - n^{I}_i,   L^{I} = sum_i n^{I}_i,   L^{II} = 1 - L^{I}
//! ```
//!
//! (DWSIM `SimpleLLE.vb` lines 260-267). Each outer pass renormalises the two
//! liquid compositions (`x^{j} = n^{j} / L^{j}`), refreshes both activity-
//! coefficient vectors, then applies the update — a fixed-point iteration on the
//! phase split. An oscillation guard (DWSIM lines 269-278) averages the current
//! and previous phase-I mole numbers when the two phase fractions swap identities.
//!
//! Convergence is declared (DWSIM lines 251-258) when the summed isoactivity
//! residual `sum_i |gamma_i^{I} x^{I}_i - gamma_i^{II} x^{II}_i|` falls below
//! `activity_tol`, or a phase fraction collapses (`< min_phase_fraction`), or the
//! two compositions coincide (`sum_i |x^{I}_i - x^{II}_i| < composition_merge_tol`),
//! or the phase fractions stop moving (`< fraction_change_tol`). The last three
//! all mean the split has *merged* back to a single liquid.
//!
//! # Phase labelling
//!
//! On a genuine split the two liquids are ordered by a **reduced molar Gibbs
//! energy of mixing** `g/RT = sum_i x_i (ln x_i + ln gamma_i)` (DWSIM re-orders by
//! `DW_CalcGibbsEnergy`, lines 305-312). This mixing Gibbs energy is self-
//! contained in the activity model; it omits the pure-component reference
//! `sum_i x_i g_i^{pure}` that DWSIM's absolute Gibbs energy includes (that needs
//! pure-component chemical potentials this activity-only interface does not
//! expose), so the phase-I / phase-II **labelling is not guaranteed identical to
//! DWSIM's** — mass balance and the sum-to-one / isoactivity identities (the V&V
//! checks) are label-independent.
//!
//! # Honest scope (verification, not benchmark validation; a *partial* port)
//!
//! - **`Flash_PT` only.** DWSIM `SimpleLLE` also exposes `Flash_PH`, `Flash_PS`,
//!   `Flash_TV`, `Flash_PV` (energy / spec flashes that wrap `Flash_PT` in a
//!   temperature/pressure root-find). None of those are ported here.
//! - **Activity-coefficient driver only.** The split is driven by
//!   [`crate::thermo::activity`] (NRTL / UNIQUAC / Ideal), not by a cubic-EOS
//!   liquid fugacity. No phi-phi LLE, no vapour phase, no solid.
//! - **Seeding is DWSIM's heuristic** ([`flash_pt_lle`]) or a caller-supplied
//!   estimate ([`flash_pt_lle_with_estimates`]); there is no built-in stability
//!   pre-test. A feed already inside a miscibility gap that the heuristic seed
//!   cannot leave may be reported as a single liquid — supply an estimate (e.g.
//!   from [`crate::thermo::stability`]) for a hard case.
//! - The tests below are **verification** against the algebraic identities (mass
//!   balance, sum-to-one, the isoactivity condition, the single-liquid limit),
//!   **not** validation against measured LLE tie-line data.
//!
//! > **⚠️ Unverified until validated.** Untrusted AI-assisted **draft** material,
//! > pending human V&V per the crate `CLAUDE.md` (verification, not validation).
//! > Not for nuclear facility operation, reactor control, safety-critical, or
//! > licensing decisions. Independent OUTRAM PARK fork, not the official DWSIM.
//!
//! # Design (workspace + crate `CLAUDE.md`)
//!
//! The activity model is the [`crate::thermo::activity::ActivityModel`] **enum**
//! (no trait object, no `dyn` / `Box` / lifetimes / channels). Compositions are
//! owned by value (`Vec<f64>`); inner arithmetic is documented raw `f64` (SI:
//! K, mole fractions \[-\]). `#![forbid(unsafe_code)]` at the crate root.

use crate::thermo::activity::ActivityModel;
use crate::thermo::flash::FlashError;

/// Tuning parameters for [`flash_pt_lle`] / [`flash_pt_lle_with_estimates`].
///
/// The defaults mirror the DWSIM `SimpleLLE` hard-coded tolerances
/// (`SimpleLLE.vb` lines 251-282).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LleOptions {
    /// Maximum outer successive-substitution passes before returning
    /// [`FlashError::NotConverged`]. Matches DWSIM's abort at `ecount > 10000`
    /// (`SimpleLLE.vb` line 282); interior splits converge in far fewer (the
    /// reference water/n-butanol 70/30 split takes ~222), but feeds sitting right
    /// on a miscibility-gap boundary converge only slowly (successive
    /// substitution is near-singular there — an honest limitation, see the module
    /// header).
    pub max_iter: usize,
    /// Convergence tolerance on the summed **isoactivity residual**
    /// `sum_i |gamma_i^{I} x^{I}_i - gamma_i^{II} x^{II}_i|` \[-\] (DWSIM `etol`,
    /// line 251 uses `1e-6`).
    pub activity_tol: f64,
    /// A liquid phase whose molar fraction falls below this \[-\] is treated as
    /// absent — the split has merged to one liquid (DWSIM line 251 uses `1e-4`).
    pub min_phase_fraction: f64,
    /// Total composition difference `sum_i |x^{I}_i - x^{II}_i|` \[-\] below which
    /// the two liquids are deemed identical (merge to one liquid; DWSIM line 251
    /// uses `1e-3`).
    pub composition_merge_tol: f64,
    /// Convergence tolerance on the per-pass change of the two phase fractions
    /// `|L^{I}_{prev} - L^{I}| + |L^{II}_{prev} - L^{II}|` \[-\] (DWSIM line 255
    /// uses `1e-7`).
    pub fraction_change_tol: f64,
    /// Optional successive-substitution **under-relaxation factor** `lambda`
    /// ∈ `(0, 1]` applied to the phase-I mole-number update
    /// `n^{I} <- (1 - lambda) n^{I}_{prev} + lambda n^{I}_{raw}` \[-\].
    ///
    /// The **default `lambda = 1.0`** is DWSIM's literal undamped substitution
    /// (`SimpleLLE.vb` lines 260-267) and is the fastest — for the reference
    /// water/n-butanol split the iteration is monotone (not oscillatory), so
    /// damping only slows it. A `lambda < 1` is offered as a stabilizer for a
    /// caller that hits a genuinely oscillatory system; it converges to the
    /// identical fixed point and subsumes DWSIM's conditional 50/50 swap-average
    /// (`SimpleLLE.vb` lines 269-278), which this port replaces with the general
    /// relaxation knob rather than the swap-specific guard (the guard mis-fires
    /// at symmetric fixed points).
    pub relaxation: f64,
}

impl Default for LleOptions {
    fn default() -> Self {
        Self {
            max_iter: 10000,
            activity_tol: 1.0e-6,
            min_phase_fraction: 1.0e-4,
            composition_merge_tol: 1.0e-3,
            fraction_change_tol: 1.0e-7,
            relaxation: 1.0,
        }
    }
}

/// A converged (or best-effort) simple-LLE flash result.
///
/// When [`split`](LleResult::split) is `true` the feed separates into two liquids
/// with `l1 + l2 = 1`, each composition summing to 1, satisfying the isoactivity
/// condition `gamma_i^{I} x^{I}_i = gamma_i^{II} x^{II}_i` to `activity_tol`. When
/// `split` is `false` the feed is a single stable liquid: `l1 = 1`, `l2 = 0`, and
/// `x1 == x2 ==` the (normalised) feed, with `gamma1 == gamma2` its activity
/// coefficients.
///
/// The `l1`/`x1` vs `l2`/`x2` labelling follows a reduced-molar-Gibbs-of-mixing
/// ordering that is **not** guaranteed identical to DWSIM's absolute-Gibbs
/// ordering (see the module *Phase labelling* note); the mass-balance and
/// sum-to-one identities are label-independent.
#[derive(Debug, Clone, PartialEq)]
pub struct LleResult {
    /// `true` iff a genuine two-liquid split was found; `false` for a single
    /// stable liquid.
    pub split: bool,
    /// Phase-I molar fraction `L^{I}` \[-\] ∈ `[0, 1]` (`1.0` when `split` is
    /// `false`).
    pub l1: f64,
    /// Phase-II molar fraction `L^{II}` \[-\] ∈ `[0, 1]` (`0.0` when `split` is
    /// `false`).
    pub l2: f64,
    /// Phase-I mole fractions `x^{I}_i` \[-\] (sum to 1).
    pub x1: Vec<f64>,
    /// Phase-II mole fractions `x^{II}_i` \[-\] (sum to 1); equals `x1` when
    /// `split` is `false`.
    pub x2: Vec<f64>,
    /// Phase-I activity coefficients `gamma_i^{I}` \[-\] at `(x1, T)`.
    pub gamma1: Vec<f64>,
    /// Phase-II activity coefficients `gamma_i^{II}` \[-\] at `(x2, T)`; equals
    /// `gamma1` when `split` is `false`.
    pub gamma2: Vec<f64>,
    /// Number of completed outer successive-substitution passes.
    pub iterations: usize,
    /// Final summed isoactivity residual
    /// `sum_i |gamma_i^{I} x^{I}_i - gamma_i^{II} x^{II}_i|` \[-\]. Near `0` on a
    /// converged split; carries the last loop value on a merge.
    pub activity_residual: f64,
}

/// Reduced molar Gibbs energy of mixing `g/RT = sum_i x_i (ln x_i + ln gamma_i)`
/// \[-\] of a liquid of composition `x` with activity coefficients `gamma`.
///
/// Self-contained in the activity model (the pure-component reference
/// `sum_i x_i g_i^{pure}` is omitted — see the module *Phase labelling* note).
/// Zero-composition components contribute a zero term (`x_i ln x_i -> 0`).
fn reduced_molar_gibbs(x: &[f64], gamma: &[f64]) -> f64 {
    let mut g = 0.0;
    for i in 0..x.len() {
        if x[i] > 0.0 {
            g += x[i] * (x[i].ln() + gamma[i].ln());
        }
    }
    g
}

/// Normalise `v` in place so its entries sum to 1 (no-op on a zero sum).
fn normalize(v: &mut [f64]) {
    let s: f64 = v.iter().sum();
    if s != 0.0 {
        for vi in v.iter_mut() {
            *vi /= s;
        }
    }
}

/// DWSIM's default two-liquid seed (`SimpleLLE.vb` lines 141-160): the smallest
/// positive feed component is placed `5 %` into phase I / `95 %` into phase II,
/// every other component the reverse, giving an asymmetric starting split.
///
/// Returns the phase-I and phase-II **mole-number** seeds `n^{I}_i`, `n^{II}_i`
/// (per mole of feed).
fn default_seed(z: &[f64]) -> (Vec<f64>, Vec<f64>) {
    let n = z.len();
    // minn = smallest positive z_i, initialised to z[0] as DWSIM does (line 141).
    let mut minn = z[0];
    for &zi in z.iter() {
        if zi > 0.0 && zi < minn {
            minn = zi;
        }
    }
    let mut vn1 = vec![0.0; n];
    let mut vn2 = vec![0.0; n];
    let mut found_first = false;
    for i in 0..n {
        if z[i] == minn && !found_first {
            vn1[i] = z[i] * 0.05;
            vn2[i] = z[i] * 0.95;
            found_first = true;
        } else {
            vn1[i] = z[i] * 0.95;
            vn2[i] = z[i] * 0.05;
        }
    }
    (vn1, vn2)
}

/// Validate `z`, returning the component count `n` and a normalised copy of `z`.
fn prepare_feed(z: &[f64]) -> Result<(usize, Vec<f64>), FlashError> {
    let n = z.len();
    if n == 0 {
        return Err(FlashError::Empty);
    }
    if z.iter().any(|v| !v.is_finite()) {
        return Err(FlashError::NonFinite);
    }
    // `z` is already all-finite here, so a non-positive sum is the only failure.
    let s: f64 = z.iter().sum();
    if s <= 0.0 {
        return Err(FlashError::NonFinite);
    }
    let z_norm: Vec<f64> = z.iter().map(|&zi| zi / s).collect();
    Ok((n, z_norm))
}

/// Simple-LLE isothermal flash of feed `z` at temperature `t` \[K\] using the
/// **DWSIM default seed** ([`default_seed`]).
///
/// Entry point for DWSIM `SimpleLLE.Flash_PT` (`SimpleLLE.vb` lines 82-330) with
/// the no-initial-estimate seeding branch (lines 141-160). Splits the feed into
/// two liquids in equilibrium under the activity model
/// `model` (NRTL / UNIQUAC / Ideal), or reports a single stable liquid. See the
/// module header for the method and the pressure-independence rationale.
///
/// # Units / ranges
///
/// - `model`: the [`ActivityModel`]; for the non-ideal variants its parameter
///   dimension must equal `z.len()`.
/// - `z`: feed mole fractions \[-\] (need not be pre-normalised; normalised
///   internally). Every `z_i` should be `> 0` for a meaningful split.
/// - `t` \[K\] (`> 0`).
///
/// # Returns
///
/// An [`LleResult`]. On a genuine split `l1 + l2 = 1`, each phase sums to 1, and
/// the isoactivity residual is below `opts.activity_tol`. On a stable feed
/// `split = false`, `l1 = 1`, `l2 = 0`.
///
/// # Errors
///
/// [`FlashError::Empty`] on empty `z`; [`FlashError::NonFinite`] on a non-finite
/// or non-positive-sum feed; [`FlashError::NotConverged`] if the successive
/// substitution does not converge within `opts.max_iter` passes.
///
/// # Panics
///
/// Panics (via the activity model) if `model`'s parameter matrices are not sized
/// to `z.len()` — a programming error, not a runtime input error.
pub fn flash_pt_lle(
    model: &ActivityModel,
    z: &[f64],
    t: f64,
    opts: LleOptions,
) -> Result<LleResult, FlashError> {
    let (_n, z_norm) = prepare_feed(z)?;
    let (vn1, vn2) = default_seed(&z_norm);
    run_lle(model, &z_norm, t, vn1, vn2, opts)
}

/// Simple-LLE isothermal flash of feed `z` at `t` \[K\] from **caller-supplied
/// initial phase-composition estimates** (DWSIM `UseInitialEstimatesForPhase1/2`,
/// `SimpleLLE.vb` lines 117-172).
///
/// `x1_est`, `x2_est` are initial guesses for the phase-I and phase-II mole
/// fractions \[-\] (each length `z.len()`, normalised internally), and `l1_est`
/// ∈ `(0, 1)` seeds the phase-I molar fraction; the phase-I mole-number seed is
/// `n^{I}_i = l1_est · x1_est_i` and `n^{II}_i = (1 - l1_est) · x2_est_i`. Use
/// this for a feed inside a miscibility gap that the default heuristic seed
/// cannot reach — e.g. seeding phase II from a
/// [`crate::thermo::stability`] destabilising trial.
///
/// Units / ranges / errors / panics are otherwise as [`flash_pt_lle`], plus
/// [`FlashError::LengthMismatch`] if `x1_est` or `x2_est` is not length `z.len()`.
pub fn flash_pt_lle_with_estimates(
    model: &ActivityModel,
    z: &[f64],
    t: f64,
    x1_est: &[f64],
    x2_est: &[f64],
    l1_est: f64,
    opts: LleOptions,
) -> Result<LleResult, FlashError> {
    let (n, z_norm) = prepare_feed(z)?;
    if x1_est.len() != n {
        return Err(FlashError::LengthMismatch {
            a: n,
            b: x1_est.len(),
        });
    }
    if x2_est.len() != n {
        return Err(FlashError::LengthMismatch {
            a: n,
            b: x2_est.len(),
        });
    }
    if !l1_est.is_finite() || l1_est <= 0.0 || l1_est >= 1.0 {
        return Err(FlashError::NonFinite);
    }
    let l2_est = 1.0 - l1_est;
    let vn1: Vec<f64> = (0..n).map(|i| l1_est * x1_est[i]).collect();
    let vn2: Vec<f64> = (0..n).map(|i| l2_est * x2_est[i]).collect();
    run_lle(model, &z_norm, t, vn1, vn2, opts)
}

/// Core successive-substitution loop shared by the two public entry points.
///
/// `z` is the already-normalised feed; `vn1` / `vn2` are the phase-I / phase-II
/// mole-number seeds. Direct port of DWSIM `SimpleLLE.Flash_PT` lines 174-313.
// Explicit `usize` indexing keeps the per-component update faithful to the
// ported DWSIM loops.
#[allow(clippy::needless_range_loop)]
fn run_lle(
    model: &ActivityModel,
    z: &[f64],
    t: f64,
    mut vn1: Vec<f64>,
    mut vn2: Vec<f64>,
    opts: LleOptions,
) -> Result<LleResult, FlashError> {
    let n = z.len();

    // Renormalise the seeds by their combined sum (DWSIM lines 175-179). With a
    // normalised feed this sum is 1, so the phase fractions read off directly.
    let s0: f64 = vn1.iter().sum::<f64>() + vn2.iter().sum::<f64>();
    if s0 > 0.0 {
        for i in 0..n {
            vn1[i] /= s0;
            vn2[i] /= s0;
        }
    }
    let mut l1: f64 = vn1.iter().sum();
    let mut l2: f64 = vn2.iter().sum();

    let mut l1_ant = 0.0_f64;
    let mut l2_ant = 0.0_f64;

    let mut vx1 = vec![0.0_f64; n];
    let mut vx2 = vec![0.0_f64; n];
    // Previous-pass compositions for the "nothing is moving" stall exit; seeded
    // to +inf so the first pass never counts as converged.
    let mut vx1_prev = vec![f64::INFINITY; n];
    let mut vx2_prev = vec![f64::INFINITY; n];
    let mut gamma1 = vec![1.0_f64; n];
    let mut gamma2 = vec![1.0_f64; n];
    // Assigned on every pass before any read and before every loop exit; the
    // `loop` never falls through, so no initialiser is needed (and a dummy one
    // would be a dead write).
    let mut err: f64;
    let mut comp_diff: f64; // sum_i |x1_i - x2_i| (DWSIM `S`)
    let mut ecount = 0_usize;

    loop {
        let vn1_ant = vn1.clone();

        // x^{j} = n^{j} / L^{j}, renormalised (DWSIM lines 206-207).
        for i in 0..n {
            vx1[i] = vn1[i] / l1;
            vx2[i] = vn2[i] / l2;
        }
        normalize(&mut vx1);
        normalize(&mut vx2);

        // Total composition movement since the previous pass (DWSIM `e1 + e2`,
        // lines 228-229 — computed there but, unlike here, not used for exit).
        let mut comp_change = 0.0;
        for i in 0..n {
            comp_change += (vx1[i] - vx1_prev[i]).abs() + (vx2[i] - vx2_prev[i]).abs();
        }

        // Activity coefficients of the two liquids (DWSIM lines 214-224, taken
        // directly from the activity model rather than P/Vp·phi).
        gamma1 = model.activity_coefficients(&vx1, t);
        gamma2 = model.activity_coefficients(&vx2, t);

        // Isoactivity residual and total composition difference (DWSIM 227-230).
        err = 0.0;
        comp_diff = 0.0;
        for i in 0..n {
            err += (vx1[i] * gamma1[i] - vx2[i] * gamma2[i]).abs();
            comp_diff += (vx1[i] - vx2[i]).abs();
        }
        if !err.is_finite() {
            return Err(FlashError::NonFinite);
        }

        // Convergence / merge exits (DWSIM lines 251-258).
        //
        // The isoactivity residual `err` is the primary signal: it drives to 0
        // for a genuine split (activities equalise) **and** for a merge (the two
        // liquids coincide, so `gamma_i^I x_i^I - gamma_i^II x_i^II -> 0`), so
        // `err < activity_tol` catches both — the merge-vs-split decision is made
        // after the loop from `comp_diff`. The `l < min_phase_fraction` /
        // `comp_diff < composition_merge_tol` guards are early merge exits.
        //
        // **Deviation from DWSIM.** DWSIM's standalone
        // `|L1_ant - L1| + |L2_ant - L2| < 1e-7` exit (line 255) fires whenever
        // the phase *fractions* momentarily stall even while the *compositions*
        // are still moving — which mis-reports a not-yet-converged near-miscible
        // mixture (and every symmetric feed, whose `L` sits at `0.5` throughout)
        // as a spurious micro-split. This port therefore gates that stall exit on
        // **both** the fractions *and* the compositions having stopped moving.
        if ecount > 0
            && (err < opts.activity_tol
                || l1 < opts.min_phase_fraction
                || l2 < opts.min_phase_fraction
                || comp_diff < opts.composition_merge_tol)
        {
            break;
        }
        if ecount > 0
            && (l1_ant - l1).abs() + (l2_ant - l2).abs() < opts.fraction_change_tol
            && comp_change < opts.fraction_change_tol
        {
            break;
        }

        vx1_prev.copy_from_slice(&vx1);
        vx2_prev.copy_from_slice(&vx2);

        // Successive-substitution update of the phase split (DWSIM lines 260-267):
        // n^{I}_i = z_i / (1 + gamma1_i L2 / (gamma2_i L1)); n^{II}_i = z_i - n^{I}_i,
        // under-relaxed with `lambda` against the previous pass to damp the
        // period-2 oscillation (see [`LleOptions::relaxation`] for the DWSIM
        // deviation; `lambda = 1` recovers DWSIM's undamped update, and DWSIM's
        // conditional 50/50 swap-average is the special case `lambda = 0.5`).
        let lambda = opts.relaxation;
        for i in 0..n {
            let ratio = gamma1[i] * l2 / (gamma2[i] * l1);
            let raw = z[i] / (1.0 + ratio);
            vn1[i] = (1.0 - lambda) * vn1_ant[i] + lambda * raw;
            vn2[i] = z[i] - vn1[i];
        }
        l1_ant = l1;
        l2_ant = l2;
        l1 = vn1.iter().sum();
        l2 = 1.0 - l1;

        ecount += 1;
        if ecount > opts.max_iter {
            return Err(FlashError::NotConverged {
                iterations: opts.max_iter,
                residual: err,
            });
        }
    }

    // Merge decision (DWSIM lines 291-313): one phase vanished or the two
    // compositions coincide -> report a single stable liquid.
    let merged = l1 < opts.min_phase_fraction
        || l2 < opts.min_phase_fraction
        || comp_diff < opts.composition_merge_tol;
    if merged {
        let gamma = model.activity_coefficients(z, t);
        return Ok(LleResult {
            split: false,
            l1: 1.0,
            l2: 0.0,
            x1: z.to_vec(),
            x2: z.to_vec(),
            gamma1: gamma.clone(),
            gamma2: gamma,
            iterations: ecount,
            activity_residual: err,
        });
    }

    // Genuine split: order the two liquids by reduced molar Gibbs of mixing
    // (DWSIM lines 305-312; see the module *Phase labelling* note for the caveat).
    let g1 = reduced_molar_gibbs(&vx1, &gamma1);
    let g2 = reduced_molar_gibbs(&vx2, &gamma2);
    let (l1o, x1o, gamma1o, l2o, x2o, gamma2o) = if g1 < g2 {
        (l2, vx2, gamma2, l1, vx1, gamma1)
    } else {
        (l1, vx1, gamma1, l2, vx2, gamma2)
    };

    Ok(LleResult {
        split: true,
        l1: l1o,
        l2: l2o,
        x1: x1o,
        x2: x2o,
        gamma1: gamma1o,
        gamma2: gamma2o,
        iterations: ecount,
        activity_residual: err,
    })
}

#[cfg(test)]
mod tests {
    //! # V&V — verification of the simple-LLE flash
    //!
    //! **Scope (honesty).** Verification of the algebraic LLE identities and the
    //! single-liquid limit, NOT validation against measured LLE tie-line data.
    //! The two non-ideal cases use published **DECHEMA NRTL parameters** shipped
    //! in DWSIM's GPL `Assets/nrtl.dat` (ChemSep 2 interaction-parameter database,
    //! Copyright 1992 Harry Kooijman & Ross Taylor; DECHEMA NRTL data at 1 atm,
    //! units cal/mol). Numbers below were **measured** on 2026-08-03 by compiling
    //! this module into the crate and running
    //! `cargo test -p outram-park-fork-dwsim-libs --lib --release`.
    //!
    //! > **⚠️ Verification, not validation.** Untrusted AI-assisted draft pending
    //! > human V&V. The parameter sets are public literature values used to
    //! > exercise the algorithm; no claim is made here that the computed tie-lines
    //! > match experiment.

    use super::*;
    use crate::thermo::activity::NrtlParams;
    use approx::assert_abs_diff_eq;

    fn m2(a: f64, b: f64, c: f64, d: f64) -> Vec<Vec<f64>> {
        vec![vec![a, b], vec![c, d]]
    }

    /// NRTL model for Water(0) / n-Butanol(1).
    ///
    /// Parameters (DECHEMA, DWSIM `Assets/nrtl.dat` record
    /// `1921;1105;2633.6951;504.0381;0.4447;Water/n-Butanol p336 1/1a`):
    /// `A_{01} = 2633.6951`, `A_{10} = 504.0381` cal/mol, `alpha = 0.4447`.
    fn water_nbutanol_nrtl() -> ActivityModel {
        ActivityModel::Nrtl(NrtlParams::from_a_alpha(
            m2(0.0, 2633.6951, 504.0381, 0.0),
            m2(0.0, 0.4447, 0.4447, 0.0),
        ))
    }

    /// **Methodology (V&V checks 1-3 — mass balance, sum-to-one, isoactivity).**
    /// Water/n-butanol is a classic partially-miscible pair. A 70/30 mol
    /// water/butanol feed at `T = 298.15 K` sits inside the miscibility gap and
    /// must split into a water-rich and a butanol-rich liquid. Checks: `split`,
    /// `l1 + l2 = 1`, each phase sums to 1, the isoactivity condition
    /// `gamma_i^{I} x^{I}_i = gamma_i^{II} x^{II}_i` holds for every component to
    /// `< 1e-6`, and the overall mass balance `z_i = L^{I} x^{I}_i + L^{II} x^{II}_i`
    /// closes to `< 1e-9`.
    ///
    /// **Result (measured 2026-08-03, `cargo test --release`):** the feed splits
    /// in 222 passes into a **water-rich** liquid (`L = 0.2518925`,
    /// `x_water = 0.9944722`) and a **butanol-rich** liquid (`L = 0.7481075`,
    /// `x_water = 0.6008494`). Isoactivity residual `= 9.812e-7 < 1e-6`; each
    /// phase sums to 1 and `L^{I} + L^{II} = 1` to `< 1e-12`; every component's
    /// activity matches across phases to `< 1e-6`; overall mass balance
    /// `z_i = L^{I} x^{I}_i + L^{II} x^{II}_i` closes to `< 1e-9`
    /// (`0.2518925·0.9944722 + 0.7481075·0.6008494 = 0.70`). The tie-line is
    /// physically sensible for n-butanol/water at 25 °C (a nearly pure-water
    /// phase and a butanol-rich phase holding ~60 mol% water).
    #[test]
    fn water_butanol_splits() {
        let model = water_nbutanol_nrtl();
        let z = [0.70, 0.30];
        let t = 298.15;
        let r = flash_pt_lle(&model, &z, t, LleOptions::default()).unwrap();

        assert!(r.split, "water/n-butanol at 298.15 K must split, got {r:?}");

        // Phase fractions in range and summing to 1.
        assert!(r.l1 > 0.0 && r.l1 < 1.0, "L1 out of range: {}", r.l1);
        assert!(r.l2 > 0.0 && r.l2 < 1.0, "L2 out of range: {}", r.l2);
        assert_abs_diff_eq!(r.l1 + r.l2, 1.0, epsilon = 1e-12);

        // Each phase composition sums to 1.
        assert_abs_diff_eq!(r.x1.iter().sum::<f64>(), 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(r.x2.iter().sum::<f64>(), 1.0, epsilon = 1e-12);

        // The two liquids are genuinely distinct.
        let diff: f64 = (0..2).map(|i| (r.x1[i] - r.x2[i]).abs()).sum();
        assert!(
            diff > 1e-2,
            "phases must be distinct, got x1={:?} x2={:?}",
            r.x1,
            r.x2
        );

        // Isoactivity: gamma_i^I x_i^I == gamma_i^II x_i^II for every component.
        for i in 0..2 {
            let a1 = r.gamma1[i] * r.x1[i];
            let a2 = r.gamma2[i] * r.x2[i];
            assert_abs_diff_eq!(a1, a2, epsilon = 1e-6);
        }
        assert!(
            r.activity_residual < 1e-6,
            "isoactivity residual too large: {}",
            r.activity_residual
        );

        // Overall mass balance: z_i = L1 x1_i + L2 x2_i.
        for (i, &zi) in z.iter().enumerate() {
            let recon = r.l1 * r.x1[i] + r.l2 * r.x2[i];
            assert_abs_diff_eq!(recon, zi, epsilon = 1e-9);
        }

        println!(
            "water/n-butanol 70/30 @298.15K: split L1={:.7} L2={:.7} \
             x1(water)={:.7} x2(water)={:.7} residual={:.3e} iters={}",
            r.l1, r.l2, r.x1[0], r.x2[0], r.activity_residual, r.iterations
        );
    }

    /// **Methodology.** Independence of the tie-line endpoints from the feed
    /// point inside the gap: two different feeds **both inside** the miscibility
    /// gap (70/30 and 85/15 mol water/butanol, i.e. overall `x_water = 0.70` and
    /// `0.85`, both within the two-phase window `0.601 < x_water < 0.994`) must
    /// yield the **same two tie-line compositions** — an LLE tie-line depends only
    /// on `T`, the lever rule only moves the phase *amounts* `L^{I}`/`L^{II}`.
    /// Compares the water-rich and butanol-rich phase compositions between feeds.
    /// **Result (measured 2026-08-03):** both feeds split to the identical
    /// tie-line endpoints (water-rich `x_water = 0.994472`, butanol-rich
    /// `x_water = 0.600849`) to `< 1e-6`; only the phase amounts differ.
    #[test]
    fn tie_line_independent_of_feed() {
        let model = water_nbutanol_nrtl();
        let t = 298.15;
        let ra = flash_pt_lle(&model, &[0.85, 0.15], t, LleOptions::default()).unwrap();
        let rb = flash_pt_lle(&model, &[0.70, 0.30], t, LleOptions::default()).unwrap();
        assert!(ra.split && rb.split);

        // Identify the water-rich phase in each (larger x[water]) and compare.
        let water_rich = |r: &LleResult| {
            if r.x1[0] > r.x2[0] {
                r.x1.clone()
            } else {
                r.x2.clone()
            }
        };
        let but_rich = |r: &LleResult| {
            if r.x1[0] > r.x2[0] {
                r.x2.clone()
            } else {
                r.x1.clone()
            }
        };
        let (wa, wb) = (water_rich(&ra), water_rich(&rb));
        let (ba, bb) = (but_rich(&ra), but_rich(&rb));
        for i in 0..2 {
            assert_abs_diff_eq!(wa[i], wb[i], epsilon = 1e-6);
            assert_abs_diff_eq!(ba[i], bb[i], epsilon = 1e-6);
        }
    }

    /// **Methodology (V&V check 4 — single-liquid / no split).** A fully-miscible
    /// pair must return one liquid. Methanol/water (DECHEMA NRTL,
    /// `Assets/nrtl.dat` record `1101;1921;-189.0469;792.802;0.2999;Methanol/Water`;
    /// Methanol(0) / Water(1), `A_{01} = -189.0469`, `A_{10} = 792.802` cal/mol,
    /// `alpha = 0.2999`) is completely miscible, so a 50/50 feed at `T = 298.15 K`
    /// must report `split = false`, `l1 = 1`, `l2 = 0`, single liquid = feed.
    /// **Result (measured 2026-08-03):** `split = false`, `l1 = 1.0`, `l2 = 0.0`,
    /// `x1 == x2 == [0.5, 0.5]`.
    #[test]
    fn methanol_water_is_miscible() {
        let model = ActivityModel::Nrtl(NrtlParams::from_a_alpha(
            m2(0.0, -189.0469, 792.802, 0.0),
            m2(0.0, 0.2999, 0.2999, 0.0),
        ));
        let z = [0.50, 0.50];
        let r = flash_pt_lle(&model, &z, 298.15, LleOptions::default()).unwrap();
        assert!(
            !r.split,
            "methanol/water must be miscible (one liquid), got {r:?}"
        );
        assert_abs_diff_eq!(r.l1, 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(r.l2, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(r.x1[0], 0.5, epsilon = 1e-12);
        assert_abs_diff_eq!(r.x2[0], 0.5, epsilon = 1e-12);
    }

    /// **Methodology.** An [`ActivityModel::Ideal`] mixture (`gamma_i = 1`) can
    /// never split — the isoactivity condition reduces to `x^{I}_i = x^{II}_i`.
    /// A 50/50 ideal feed must report a single liquid.
    /// **Result (measured 2026-08-03):** `split = false`, one liquid = feed.
    #[test]
    fn ideal_never_splits() {
        let r = flash_pt_lle(
            &ActivityModel::Ideal,
            &[0.5, 0.5],
            300.0,
            LleOptions::default(),
        )
        .unwrap();
        assert!(!r.split, "ideal mixture cannot split, got {r:?}");
        assert_abs_diff_eq!(r.l1, 1.0, epsilon = 1e-12);
    }

    /// **Methodology.** The caller-seeded entry point must reach the same split as
    /// the default-seeded one for the water/n-butanol gap when given a sensible
    /// estimate (phase I water-rich, phase II butanol-rich, `L^{I} = 0.5`).
    /// **Result (measured 2026-08-03):** same tie-line compositions as
    /// [`flash_pt_lle`] to `< 1e-6`.
    #[test]
    fn seeded_entry_matches_default() {
        let model = water_nbutanol_nrtl();
        let t = 298.15;
        let z = [0.75, 0.25];
        let def = flash_pt_lle(&model, &z, t, LleOptions::default()).unwrap();
        let seeded = flash_pt_lle_with_estimates(
            &model,
            &z,
            t,
            &[0.98, 0.02], // phase I: water-rich estimate
            &[0.50, 0.50], // phase II: butanol-rich estimate
            0.5,
            LleOptions::default(),
        )
        .unwrap();
        assert!(def.split && seeded.split);
        // Compare water-rich endpoints (labelling may differ between the two).
        let water_rich = |r: &LleResult| {
            if r.x1[0] > r.x2[0] {
                r.x1.clone()
            } else {
                r.x2.clone()
            }
        };
        let (a, b) = (water_rich(&def), water_rich(&seeded));
        for i in 0..2 {
            assert_abs_diff_eq!(a[i], b[i], epsilon = 1e-6);
        }
    }

    /// **Methodology.** Input-validation guards.
    /// **Result (measured 2026-08-03):** empty feed -> `Empty`; a non-finite feed
    /// -> `NonFinite`; an estimate-length mismatch -> `LengthMismatch`.
    #[test]
    fn input_validation_errors() {
        assert_eq!(
            flash_pt_lle(&ActivityModel::Ideal, &[], 300.0, LleOptions::default()).unwrap_err(),
            FlashError::Empty
        );
        assert_eq!(
            flash_pt_lle(
                &ActivityModel::Ideal,
                &[f64::NAN, 0.5],
                300.0,
                LleOptions::default()
            )
            .unwrap_err(),
            FlashError::NonFinite
        );
        assert!(matches!(
            flash_pt_lle_with_estimates(
                &ActivityModel::Ideal,
                &[0.5, 0.5],
                300.0,
                &[1.0],
                &[0.5, 0.5],
                0.5,
                LleOptions::default(),
            )
            .unwrap_err(),
            FlashError::LengthMismatch { .. }
        ));
    }
}
