//! Three-phase **vapour-liquid-liquid equilibrium** (VLLE) isothermal-isobaric
//! (**PT**) flash via the two-equation nested-loops method.
//!
//! Ported from DWSIM `DWSIM.Thermodynamics/FlashAlgorithms/NestedLoops3PV3.vb`
//! (`Flash_PT` orchestration lines 98-185, `Flash_PT_3P` core lines 349-739),
//! GPL-3.0, commit `1abf72d`, with the second-liquid detection from
//! `BaseFlashAlgorithm.vb` `GetPhaseSplitEstimates` (lines 1233-1282). Specific
//! ported lines are cited at each function below.
//!
//! # Provenance
//!
//! ```text
//! Upstream project : DWSIM (Daniel Wagner O. de Medeiros; Gregor Reichert)
//! Source file      : DWSIM.Thermodynamics/FlashAlgorithms/NestedLoops3PV3.vb
//!                    DWSIM.Thermodynamics/FlashAlgorithms/BaseFlashAlgorithm.vb
//! Commit           : 1abf72d
//! Licence          : GPL-3.0
//! ```
//!
//! # What this module computes
//!
//! Given a feed `z_i` \[-\] at fixed `T` \[K\], `P` \[Pa\], split it into up to
//! three coexisting phases — a vapour (`y_i`, fraction `V`) and two liquids
//! (`x^{I}_i`, fraction `L^{I}`; `x^{II}_i`, fraction `L^{II}`) with
//! `V + L^{I} + L^{II} = 1`, in mutual equilibrium
//! (`φ_i^V y_i = φ_i^{L I} x^{I}_i = φ_i^{L II} x^{II}_i`).
//!
//! # Method (the two Rachford-Rice equations)
//!
//! Writing `K^{j}_i = φ_i^{L j} / φ_i^{V}` for liquid `j ∈ {I, II}` and
//! `β^{j}_i = 1 − 1/K^{j}_i` (DWSIM `NestedLoops3PV3.vb` lines 544-545), the
//! material balance is
//!
//! ```text
//! z_i = y_i (1 − β^{I}_i L^{I} − β^{II}_i L^{II}),   y_i = z_i / D_i,
//! ```
//!
//! with `D_i = 1 − β^{I}_i L^{I} − β^{II}_i L^{II}` (DWSIM line 546) and
//! `x^{I}_i = y_i / K^{I}_i`, `x^{II}_i = y_i / K^{II}_i` (lines 547-548). The two
//! phase fractions solve the coupled Rachford-Rice pair
//!
//! ```text
//! F_1(L^{I}, L^{II}) = Σ_i β^{I}_i z_i / D_i = 0,
//! F_2(L^{I}, L^{II}) = Σ_i β^{II}_i z_i / D_i = 0
//! ```
//!
//! (DWSIM lines 606-607), which force `Σ x^{I} = Σ y` and `Σ x^{II} = Σ y`. They
//! are solved by a damped 2×2 Newton iteration (DWSIM lines 620-654); the
//! K-values are refreshed from the rigorous EOS each outer pass (lines 523-534).
//!
//! Note the identity: for **any** `(L^{I}, L^{II})`, the un-normalised
//! `z_i = V y_i + L^{I} x^{I}_i + L^{II} x^{II}_i` holds exactly with
//! `V = 1 − L^{I} − L^{II}` (substitute `x^{j}_i = y_i/K^{j}_i` and
//! `β^{j}_i = 1 − 1/K^{j}_i`). `F_1 = F_2 = 0` additionally makes the three
//! normalised compositions each sum to 1, so overall mass balance closes on the
//! **normalised** phases too — this is V&V check (2) below.
//!
//! # Orchestration (when three phases appear)
//!
//! [`flash_pt_vlle`] mirrors DWSIM `Flash_PT` (lines 145-181): first a rigorous
//! **two-phase VLE** flash ([`crate::thermo::flash::nested_loops_flash`]); then,
//! if a liquid exists, a **phase-stability test** on that liquid
//! ([`crate::thermo::stability::stability_test`], the analogue of DWSIM's
//! `StabTest2` inside `GetPhaseSplitEstimates`) to detect a distinct second
//! liquid. Only if the liquid is unstable does the three-phase Newton solve
//! [`solve_3p_fixed_k`] run; otherwise the two-phase result is returned unchanged
//! (VLLE with `L^{II} = 0`).
//!
//! # Honest scope (verification, not benchmark validation, and a *partial* port)
//!
//! Three-phase flash robustness is genuinely hard, and this is a **first port**:
//!
//! - **Second-liquid detection is only as good as the two Wilson-seeded stability
//!   trials** ([`crate::thermo::stability`]); a liquid-liquid split that neither
//!   Wilson seed reaches will be missed and the flash silently returns two
//!   phases. No global TPD minimisation, no third "ideal-mix" seed.
//! - **No `SimpleLLE` / Gibbs-minimisation fallback.** DWSIM switches to a
//!   `SimpleLLE` solver when the vapour vanishes (line 678) and does a final
//!   Gibbs-energy re-ordering of the two liquids (lines 726-737). This port does
//!   **neither**: if the vapour collapses it returns the LL-only estimate as-is,
//!   and it does **not** order the two liquids by Gibbs energy (that needs an
//!   absolute-fugacity closure this K-only interface does not expose) — so which
//!   liquid is labelled `L^{I}` vs `L^{II}` is **not** physically canonical.
//!   Mass balance and the sum-to-one identities (the V&V checks) are independent
//!   of that labelling.
//! - **`k_ij = 0`** throughout (geometric-mean mixing), which makes a genuine
//!   liquid-liquid split under a cubic EOS with the bundled reference compounds
//!   unlikely; the three-phase numerics are therefore verified on the
//!   **fixed-K** core [`solve_3p_fixed_k`] against the algebraic mass-balance
//!   identity, and the composed driver is verified to **reduce to the two-phase
//!   result** when no second liquid is found. A full EOS-driven LLE benchmark is
//!   deferred.
//!
//! > **⚠️ Unverified until validated.** AI-assisted **partial** port — untrusted
//! > draft material until human-reviewed per the crate `CLAUDE.md`. Not for
//! > nuclear facility operation, reactor control, safety-critical, or licensing
//! > decisions. Independent OUTRAM PARK fork, not the official DWSIM.
//!
//! # Design (workspace + crate `CLAUDE.md`)
//!
//! Enum dispatch (the fugacity model is the [`CubicEos`] **enum**), no trait
//! objects / `dyn` / `Box` / lifetimes / channels. Compositions owned by value;
//! documented raw `f64` (SI: K, Pa, mole fractions \[-\]) in the inner loops.

use crate::thermo::cubic_eos::{CubicEos, Phase};
use crate::thermo::flash::{
    nested_loops_flash, wilson_k_values, FlashError, FlashResult, NestedLoopsOptions,
};
use crate::thermo::stability::stability_test;
use crate::thermo::Component;

/// Tuning parameters for [`flash_pt_vlle`] and [`solve_3p_fixed_k`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VlleOptions {
    /// Maximum outer (rigorous-K + Newton) iterations before returning
    /// [`FlashError::NotConverged`]. DWSIM `maxit_e` default is 100.
    pub max_outer_iter: usize,
    /// Convergence tolerance on `|F_1| + |F_2|` \[-\], the summed Rachford-Rice
    /// residuals of the two liquid-fraction equations (DWSIM `etol`).
    pub f_tol: f64,
    /// Convergence tolerance on the total per-pass composition + phase-fraction
    /// change \[-\] (DWSIM line 586 uses `1e-10`).
    pub change_tol: f64,
    /// A liquid phase whose fraction falls below this \[-\] is treated as absent
    /// (the split has collapsed back to two phases).
    pub min_phase_fraction: f64,
    /// Newton per-step damping cap \[-\]: the fractional change in each liquid
    /// fraction is limited to this (DWSIM line 646 uses `0.1`).
    pub max_damping: f64,
}

impl Default for VlleOptions {
    fn default() -> Self {
        Self {
            max_outer_iter: 100,
            f_tol: 1.0e-10,
            change_tol: 1.0e-10,
            min_phase_fraction: 1.0e-8,
            max_damping: 0.1,
        }
    }
}

/// A converged (or best-effort) three-phase VLLE flash result.
///
/// Phase fractions are molar \[-\] and satisfy `v + l1 + l2 = 1`; compositions are
/// mole fractions \[-\] each summing to 1. When no second liquid is present
/// `l2 = 0`, `x2` mirrors `x1`, and the result is the two-phase VLE split.
///
/// The `l1`/`l2` (and `x1`/`x2`, `k1`/`k2`) labelling is **not** Gibbs-ordered —
/// see the module scope note. Only mass balance and the sum-to-one identities are
/// label-independent.
#[derive(Debug, Clone, PartialEq)]
pub struct VlleResult {
    /// Vapour molar fraction `V` \[-\] ∈ `[0, 1]`.
    pub v: f64,
    /// First-liquid molar fraction `L^{I}` \[-\] ∈ `[0, 1]`.
    pub l1: f64,
    /// Second-liquid molar fraction `L^{II}` \[-\] ∈ `[0, 1]`; `0.0` when no
    /// second liquid was detected (two-phase reduction).
    pub l2: f64,
    /// Vapour mole fractions `y_i` \[-\] (sum to 1).
    pub y: Vec<f64>,
    /// First-liquid mole fractions `x^{I}_i` \[-\] (sum to 1).
    pub x1: Vec<f64>,
    /// Second-liquid mole fractions `x^{II}_i` \[-\] (sum to 1); equals `x1` when
    /// `l2 = 0`.
    pub x2: Vec<f64>,
    /// First-liquid K-values `K^{I}_i = y_i / x^{I}_i` \[-\].
    pub k1: Vec<f64>,
    /// Second-liquid K-values `K^{II}_i = y_i / x^{II}_i` \[-\]; equals `k1` when
    /// `l2 = 0`.
    pub k2: Vec<f64>,
    /// `true` iff a distinct second liquid was detected and retained.
    pub three_phase: bool,
    /// Number of completed outer iterations of the three-phase Newton solve
    /// (`0` when the flash reduced to two phases without entering it).
    pub iterations: usize,
}

/// The converged fixed-K three-phase split (compositions + phase fractions).
#[derive(Debug, Clone, PartialEq)]
pub struct ThreePhaseSplit {
    /// Vapour molar fraction `V` \[-\].
    pub v: f64,
    /// First-liquid molar fraction `L^{I}` \[-\].
    pub l1: f64,
    /// Second-liquid molar fraction `L^{II}` \[-\].
    pub l2: f64,
    /// Vapour mole fractions `y_i` \[-\] (sum to 1).
    pub y: Vec<f64>,
    /// First-liquid mole fractions `x^{I}_i` \[-\] (sum to 1).
    pub x1: Vec<f64>,
    /// Second-liquid mole fractions `x^{II}_i` \[-\] (sum to 1).
    pub x2: Vec<f64>,
    /// Completed Newton iterations.
    pub iterations: usize,
}

/// K-values `K_i = φ_i^L(x) / φ_i^V(y) = exp(ln φ_i^L − ln φ_i^V)` \[-\] from the
/// cubic EOS, liquid root for `x`, vapour root for `y` (`k_ij = 0`).
///
/// Mirrors [`crate::thermo::property_package::PropertyPackageModel`]'s cubic
/// K-update. Falls back to the Wilson estimate if either phase yields no usable
/// `Z`-root, so the driver stays well-posed. `t` \[K\] > 0, `p` \[Pa\] > 0.
#[must_use]
pub fn eos_k_values(
    eos: CubicEos,
    components: &[Component],
    x: &[f64],
    y: &[f64],
    t: f64,
    p: f64,
) -> Vec<f64> {
    let ln_phi_l = eos.ln_phi(components, x, t, p, Phase::Liquid, None);
    let ln_phi_v = eos.ln_phi(components, y, t, p, Phase::Vapor, None);
    match (ln_phi_l, ln_phi_v) {
        (Some(l), Some(v)) => l
            .iter()
            .zip(v.iter())
            .map(|(&li, &vi)| (li - vi).exp())
            .collect(),
        _ => wilson_k_values(components, t, p),
    }
}

fn normalize(v: &mut [f64]) {
    let s: f64 = v.iter().sum();
    if s != 0.0 {
        for vi in v.iter_mut() {
            *vi /= s;
        }
    }
}

/// Recover the un-normalised then normalised three-phase compositions from the
/// current `(L^{I}, L^{II})` and fixed K-vectors.
///
/// `y_i = z_i / D_i`, `x^{I}_i = y_i / K^{I}_i`, `x^{II}_i = y_i / K^{II}_i` with
/// `D_i = 1 − β^{I}_i L^{I} − β^{II}_i L^{II}`, then each phase normalised
/// (DWSIM `NestedLoops3PV3.vb` lines 540-569).
fn recover_3p_compositions(
    z: &[f64],
    k1: &[f64],
    k2: &[f64],
    l1: f64,
    l2: f64,
) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let n = z.len();
    let mut y = vec![0.0; n];
    let mut x1 = vec![0.0; n];
    let mut x2 = vec![0.0; n];
    for i in 0..n {
        let b1 = 1.0 - 1.0 / k1[i];
        let b2 = 1.0 - 1.0 / k2[i];
        let d = 1.0 - b1 * l1 - b2 * l2;
        y[i] = z[i] / d;
        x1[i] = y[i] / k1[i];
        x2[i] = y[i] / k2[i];
    }
    normalize(&mut y);
    normalize(&mut x1);
    normalize(&mut x2);
    (y, x1, x2)
}

/// One damped 2×2 Newton step on `(L^{I}, L^{II})` for the coupled Rachford-Rice
/// pair `F_1 = F_2 = 0`.
///
/// Returns `(l1_new, l2_new, |F_1| + |F_2|)`. The Jacobian is
/// `∂F_a/∂L_b = Σ_i β^{a}_i β^{b}_i z_i / D_i²` (symmetric); the Newton direction
/// `ΔL = J^{-1} F` (DWSIM `NestedLoops3PV3.vb` lines 606-654) is capped so no
/// fraction moves by more than `max_damping` in one step, then clamped to `≥ 0`.
///
/// **Documented deviation from DWSIM.** DWSIM's literal per-step factor
/// `df = min(|ΔL^{I}/L^{I}|, |ΔL^{II}/L^{II}|, 0.1)` (lines 643-646) shrinks the
/// step *proportionally* to the Newton correction, which under-relaxes to a
/// premature stall near the fixed point when the K-values are held constant (as
/// in [`solve_3p_fixed_k`], with no per-pass composition/K update to keep driving
/// convergence). This port instead applies a **trust-region cap**: scale the full
/// Newton step by `s = min(1, max_damping/|ΔL^{I}|, max_damping/|ΔL^{II}|)`, so
/// far from the root each fraction moves at most `max_damping`, and **once the
/// Newton step is within the cap the full step is taken** — giving proper
/// quadratic convergence to `F_1 = F_2 = 0`. Both target the identical root.
fn newton_step_l1l2(
    z: &[f64],
    k1: &[f64],
    k2: &[f64],
    l1: f64,
    l2: f64,
    max_damping: f64,
) -> (f64, f64, f64) {
    let n = z.len();
    let (mut f1, mut f2) = (0.0, 0.0);
    let (mut j11, mut j12, mut j22) = (0.0, 0.0, 0.0);
    for i in 0..n {
        let b1 = 1.0 - 1.0 / k1[i];
        let b2 = 1.0 - 1.0 / k2[i];
        let d = 1.0 - b1 * l1 - b2 * l2;
        let d2 = d * d;
        f1 += b1 * z[i] / d;
        f2 += b2 * z[i] / d;
        j11 += b1 * b1 * z[i] / d2;
        j12 += b1 * b2 * z[i] / d2;
        j22 += b2 * b2 * z[i] / d2;
    }
    let f_norm = f1.abs() + f2.abs();
    let det = j11 * j22 - j12 * j12;
    if det.abs() < 1.0e-30 || !det.is_finite() {
        return (l1, l2, f_norm);
    }
    // ΔL = J^{-1} F.
    let dl1 = (j22 * f1 - j12 * f2) / det;
    let dl2 = (j11 * f2 - j12 * f1) / det;

    // Trust-region cap: full Newton step when small, capped at `max_damping`.
    let mut s = 1.0_f64;
    if dl1.abs() > max_damping {
        s = s.min(max_damping / dl1.abs());
    }
    if dl2.abs() > max_damping {
        s = s.min(max_damping / dl2.abs());
    }

    let mut l1n = l1 - dl1 * s;
    let mut l2n = l2 - dl2 * s;
    if l1n < 0.0 {
        l1n = 0.0;
    }
    if l2n < 0.0 {
        l2n = 0.0;
    }
    (l1n, l2n, f_norm)
}

/// Solve the three-phase split for **fixed** liquid K-vectors `K^{I}`, `K^{II}`.
///
/// This is the numerical core of DWSIM `Flash_PT_3P` (lines 496-705) with the
/// K-values held constant (no property-model call): a damped 2×2 Newton iteration
/// on the coupled Rachford-Rice pair `F_1 = F_2 = 0` for `(L^{I}, L^{II})`,
/// recovering the phase compositions each pass.
///
/// # Units / ranges
///
/// `z`, `k1`, `k2`: equal length `n ≥ 1`; `z` feed mole fractions \[-\]; `k1`,
/// `k2` liquid/vapour K-values \[-\] (`> 0`). `l1_est`, `l2_est` ∈ `(0, 1)` with
/// `l1_est + l2_est < 1` seed the Newton iteration. `opts` bounds the iteration
/// and tolerances.
///
/// # Returns
///
/// A [`ThreePhaseSplit`] with `v = 1 − L^{I} − L^{II}` and the normalised
/// compositions. On convergence `F_1 = F_2 = 0` to `opts.f_tol`, so `Σ y = Σ x^I
/// = Σ x^II = 1` and the overall mass balance `z_i = v y_i + L^{I} x^{I}_i +
/// L^{II} x^{II}_i` closes.
///
/// # Errors
///
/// [`FlashError::Empty`] on empty `z`; [`FlashError::LengthMismatch`] on a size
/// mismatch; [`FlashError::NonFinite`] on a non-finite / non-positive K;
/// [`FlashError::NotConverged`] if the Newton iteration does not reach `f_tol`
/// within `opts.max_outer_iter`.
pub fn solve_3p_fixed_k(
    z: &[f64],
    k1: &[f64],
    k2: &[f64],
    l1_est: f64,
    l2_est: f64,
    opts: VlleOptions,
) -> Result<ThreePhaseSplit, FlashError> {
    let n = z.len();
    if n == 0 {
        return Err(FlashError::Empty);
    }
    if k1.len() != n {
        return Err(FlashError::LengthMismatch { a: n, b: k1.len() });
    }
    if k2.len() != n {
        return Err(FlashError::LengthMismatch { a: n, b: k2.len() });
    }
    if z
        .iter()
        .chain(k1.iter())
        .chain(k2.iter())
        .any(|v| !v.is_finite())
        || k1.iter().chain(k2.iter()).any(|&v| v <= 0.0)
    {
        return Err(FlashError::NonFinite);
    }

    let mut l1 = l1_est;
    let mut l2 = l2_est;

    for iter in 1..=opts.max_outer_iter {
        let (l1n, l2n, f_norm) = newton_step_l1l2(z, k1, k2, l1, l2, opts.max_damping);
        let change = (l1n - l1).abs() + (l2n - l2).abs();
        l1 = l1n;
        l2 = l2n;
        if f_norm < opts.f_tol || change < opts.change_tol {
            let (y, x1, x2) = recover_3p_compositions(z, k1, k2, l1, l2);
            return Ok(ThreePhaseSplit {
                v: (1.0 - l1 - l2).clamp(0.0, 1.0),
                l1,
                l2,
                y,
                x1,
                x2,
                iterations: iter,
            });
        }
    }
    Err(FlashError::NotConverged {
        iterations: opts.max_outer_iter,
        residual: f64::NAN,
    })
}

/// Build the DWSIM `GetPhaseSplitEstimates` second-liquid estimate from a
/// stability-test trial composition (`BaseFlashAlgorithm.vb` lines 1251-1278).
///
/// Given the equilibrium liquid `x_liq` (fraction `l_total` of the feed) and the
/// destabilising trial composition `x2_trial` (the second liquid), returns
/// `(L^{I}, x^{I}, L^{II}, x^{II})` scaled so `L^{I} + L^{II} = l_total`:
/// `L^{II} = l_total · x_liq[m] · max(x2_trial)` for the component `m` where
/// `x2_trial` peaks, `x^{I} = (x_liq − x2_trial L^{II}/l_total)` renormalised.
fn phase_split_estimate(
    x_liq: &[f64],
    x2_trial: &[f64],
    l_total: f64,
) -> (f64, Vec<f64>, f64, Vec<f64>) {
    let n = x_liq.len();
    // Argmax of the trial composition.
    let mut m = 0;
    let mut maxv = x2_trial[0];
    for i in 1..n {
        if x2_trial[i] > maxv {
            maxv = x2_trial[i];
            m = i;
        }
    }
    let l2_frac = x_liq[m] * maxv; // fraction of the *liquid* that is phase II
    let l1_frac = 1.0 - l2_frac;
    let mut x1: Vec<f64> = (0..n)
        .map(|i| (x_liq[i] - x2_trial[i] * l2_frac) / l1_frac)
        .collect();
    normalize(&mut x1);
    let x2 = x2_trial.to_vec();
    (l1_frac * l_total, x1, l2_frac * l_total, x2)
}

/// Full **three-phase VLLE** isothermal-isobaric flash of feed `z` at `T` \[K\],
/// `P` \[Pa\] using the cubic EOS `eos` (`k_ij = 0`).
///
/// Orchestration (DWSIM `NestedLoops3PV3.vb` `Flash_PT`, lines 145-181):
///
/// 1. **Two-phase VLE** ([`crate::thermo::flash::nested_loops_flash`]) with the
///    EOS K-closure ([`eos_k_values`]).
/// 2. If a liquid exists, **stability test** it
///    ([`crate::thermo::stability::stability_test`]). Stable ⇒ return the
///    two-phase result (`l2 = 0`).
/// 3. Unstable ⇒ build a second-liquid estimate ([`phase_split_estimate`]) and
///    run the **three-phase Newton solve** (an [`solve_3p_fixed_k`] Newton step
///    with the K-vectors refreshed from the EOS each outer pass, DWSIM
///    lines 496-705).
/// 4. If the second liquid collapses below `opts.min_phase_fraction`, fall back
///    to the two-phase result.
///
/// # Units / ranges
///
/// `components.len() == z.len()`; `z` feed mole fractions \[-\] (sum to 1);
/// `t` \[K\] > 0, `p` \[Pa\] > 0. See the module scope note for the honest limits
/// (label ordering, missed splits, no `SimpleLLE` fallback).
///
/// # Errors
///
/// Propagates [`FlashError`] from the two-phase flash and the three-phase Newton
/// solve; [`FlashError::LengthMismatch`] on a `components`/`z` size mismatch.
pub fn flash_pt_vlle(
    components: &[Component],
    z: &[f64],
    t: f64,
    p: f64,
    eos: CubicEos,
    opts: VlleOptions,
) -> Result<VlleResult, FlashError> {
    if components.len() != z.len() {
        return Err(FlashError::LengthMismatch {
            a: z.len(),
            b: components.len(),
        });
    }
    let n = z.len();

    // --- Step 1: rigorous two-phase VLE. ---
    let k_closure = |x: &[f64], y: &[f64], t: f64, p: f64| eos_k_values(eos, components, x, y, t, p);
    let vle: FlashResult =
        nested_loops_flash(z, components, t, p, &k_closure, NestedLoopsOptions::default())?;

    let l_total = 1.0 - vle.beta;

    // Helper to package a two-phase result as a (degenerate) VLLE result.
    let two_phase = |vle: &FlashResult, iters: usize| VlleResult {
        v: vle.beta,
        l1: 1.0 - vle.beta,
        l2: 0.0,
        y: vle.y.clone(),
        x1: vle.x.clone(),
        x2: vle.x.clone(),
        k1: vle.k.clone(),
        k2: vle.k.clone(),
        three_phase: false,
        iterations: iters,
    };

    // No liquid to split (all vapour, or an infinitesimal liquid) → two-phase.
    if l_total <= opts.min_phase_fraction {
        return Ok(two_phase(&vle, 0));
    }

    // --- Step 2: is the equilibrium liquid stable, or does a 2nd liquid exist? ---
    let stab = stability_test(components, &vle.x, t, p, eos);
    let Some(x2_trial) = stab.trial_composition.filter(|_| !stab.stable) else {
        // Stable liquid: genuine two-phase VLE, no second liquid.
        return Ok(two_phase(&vle, 0));
    };

    // --- Step 3: build the 3-phase estimate and run the Newton solve. ---
    let (mut l1, mut x1, mut l2, mut x2) = phase_split_estimate(&vle.x, &x2_trial, l_total);
    let mut y = vle.y.clone();

    if l2 <= opts.min_phase_fraction {
        return Ok(two_phase(&vle, 0));
    }

    let mut k1 = eos_k_values(eos, components, &x1, &y, t, p);
    let mut k2 = eos_k_values(eos, components, &x2, &y, t, p);
    let mut iters = 0;

    for iter in 1..=opts.max_outer_iter {
        iters = iter;
        // Refresh K from the current compositions (rigorous EOS update).
        k1 = eos_k_values(eos, components, &x1, &y, t, p);
        k2 = eos_k_values(eos, components, &x2, &y, t, p);
        if k1.iter().chain(k2.iter()).any(|v| !v.is_finite() || *v <= 0.0) {
            return Err(FlashError::NonFinite);
        }

        let (y_new, x1_new, x2_new) = recover_3p_compositions(z, &k1, &k2, l1, l2);
        let comp_change: f64 = (0..n)
            .map(|i| {
                (x1_new[i] - x1[i]).abs() + (x2_new[i] - x2[i]).abs() + (y_new[i] - y[i]).abs()
            })
            .sum();
        y = y_new;
        x1 = x1_new;
        x2 = x2_new;

        // Newton update of the two liquid fractions.
        let (l1n, l2n, f_norm) = newton_step_l1l2(z, &k1, &k2, l1, l2, opts.max_damping);
        let frac_change = (l1n - l1).abs() + (l2n - l2).abs();
        l1 = l1n;
        l2 = l2n;

        // Second liquid collapsed → fall back to two-phase VLE.
        if l2 <= opts.min_phase_fraction {
            return Ok(two_phase(&vle, iter));
        }

        if (comp_change + frac_change + f_norm) < opts.change_tol || f_norm < opts.f_tol {
            break;
        }
        if iter == opts.max_outer_iter {
            return Err(FlashError::NotConverged {
                iterations: opts.max_outer_iter,
                residual: f_norm,
            });
        }
    }

    let v = (1.0 - l1 - l2).clamp(0.0, 1.0);
    Ok(VlleResult {
        v,
        l1,
        l2,
        y,
        x1,
        x2,
        k1,
        k2,
        three_phase: true,
        iterations: iters,
    })
}

#[cfg(test)]
mod tests {
    //! # V&V — verification of the VLLE flash
    //!
    //! **Scope (honesty).** Verification of the algebraic identities and the
    //! two-phase reduction, NOT validation against measured VLLE data. `k_ij = 0`.
    //! Numbers below were **measured** on 2026-08-03 by compiling this module into
    //! the crate and running `cargo test -p outram-park-fork-dwsim-libs --lib
    //! --release`.

    use super::*;
    use crate::thermo::component::reference;
    use approx::assert_abs_diff_eq;

    /// **Methodology (V&V check 2 — mass balance & sum-to-one).** The fixed-K
    /// three-phase core must, at convergence, close the overall mass balance
    /// `z_i = V y_i + L^{I} x^{I}_i + L^{II} x^{II}_i`, make each phase composition
    /// sum to 1, and keep every phase fraction in `[0, 1]` with `V + L^{I} + L^{II}
    /// = 1`. Synthetic immiscible ternary: `z = [0.4, 0.3, 0.3]`, component 0
    /// volatile in both liquids (`K^{I}_0 = K^{II}_0 = 4`), component 1 concentrates
    /// in liquid I (`K^{I}_1 = 0.3`, `K^{II}_1 = 2`), component 2 concentrates in
    /// liquid II (`K^{I}_2 = 2`, `K^{II}_2 = 0.3`); seeds `L^{I} = L^{II} = 0.3`.
    /// **Result (measured 2026-08-03):** converged in 5 Newton iterations to a
    /// genuine three-phase split `V = 0.6363636`, `L^{I} = 0.1818182`,
    /// `L^{II} = 0.1818182` (all three fractions strictly positive); each phase
    /// sums to 1 to < 1e-12, `V + L^{I} + L^{II} = 1` to < 1e-12, and the overall
    /// mass balance closes to < 1e-9 for every component. (The symmetric
    /// `L^{I} = L^{II}` is the expected consequence of the symmetric K-choice.)
    #[test]
    fn fixed_k_three_phase_mass_balance_and_sums() {
        let z = [0.4, 0.3, 0.3];
        let k1 = [4.0, 0.3, 2.0];
        let k2 = [4.0, 2.0, 0.3];
        let split = solve_3p_fixed_k(&z, &k1, &k2, 0.3, 0.3, VlleOptions::default()).unwrap();

        // Genuine three-phase split: every fraction strictly positive and in [0,1].
        assert!(split.v > 0.0 && split.v < 1.0, "V out of range: {}", split.v);
        assert!(split.l1 > 0.0 && split.l1 < 1.0, "L1 out of range: {}", split.l1);
        assert!(split.l2 > 0.0 && split.l2 < 1.0, "L2 out of range: {}", split.l2);
        assert_abs_diff_eq!(split.v + split.l1 + split.l2, 1.0, epsilon = 1e-12);

        // Each phase composition sums to 1.
        assert_abs_diff_eq!(split.y.iter().sum::<f64>(), 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(split.x1.iter().sum::<f64>(), 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(split.x2.iter().sum::<f64>(), 1.0, epsilon = 1e-12);

        // Overall mass balance Σ(phase fraction · phase composition) = feed.
        for i in 0..z.len() {
            let recon = split.v * split.y[i] + split.l1 * split.x1[i] + split.l2 * split.x2[i];
            assert_abs_diff_eq!(recon, z[i], epsilon = 1e-9);
        }
    }

    /// **Methodology.** An asymmetric fixed-K ternary checks the mass balance
    /// identity away from the symmetric split, with all three phases genuinely
    /// present. `z = [0.4, 0.3, 0.3]`, `K^{I} = [3.0, 0.25, 2.5]` (comp 1 favours
    /// liquid I), `K^{II} = [3.5, 2.5, 0.28]` (comp 2 favours liquid II); seeds
    /// `L^{I} = L^{II} = 0.3`. **Result (measured 2026-08-03):** converged in 5
    /// Newton iterations to `V = 0.4985306`, `L^{I} = 0.2740227`,
    /// `L^{II} = 0.2274467` (an asymmetric, strictly-positive three-phase split);
    /// each phase sums to 1 (< 1e-12), fractions sum to 1 (< 1e-12), and the
    /// overall mass balance closes to < 1e-9.
    #[test]
    fn fixed_k_three_phase_asymmetric_mass_balance() {
        let z = [0.4, 0.3, 0.3];
        let k1 = [3.0, 0.25, 2.5];
        let k2 = [3.5, 2.5, 0.28];
        let split = solve_3p_fixed_k(&z, &k1, &k2, 0.3, 0.3, VlleOptions::default()).unwrap();

        // Genuine, asymmetric three-phase split (all fractions strictly positive).
        assert!(split.v > 0.0 && split.v < 1.0);
        assert!(split.l1 > 0.0 && split.l2 > 0.0);
        assert!((split.l1 - split.l2).abs() > 1e-3, "expected an asymmetric split");
        assert_abs_diff_eq!(split.v + split.l1 + split.l2, 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(split.x1.iter().sum::<f64>(), 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(split.x2.iter().sum::<f64>(), 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(split.y.iter().sum::<f64>(), 1.0, epsilon = 1e-12);
        for i in 0..z.len() {
            let recon = split.v * split.y[i] + split.l1 * split.x1[i] + split.l2 * split.x2[i];
            assert_abs_diff_eq!(recon, z[i], epsilon = 1e-9);
        }
    }

    /// **Methodology (V&V check 2 — two-phase reduction).** For a feed that is a
    /// genuine VLE two-phase mixture with **no** liquid-liquid split, the composed
    /// VLLE driver must detect a stable liquid and return exactly the two-phase
    /// result (`L^{II} = 0`), matching a direct
    /// [`crate::thermo::flash::nested_loops_flash`]. Feed methane/ethane
    /// `z = [0.5, 0.5]`, `T = 200 K`, `P = 2·10⁶ Pa`, Peng-Robinson — a
    /// hydrocarbon pair that forms one liquid, not two.
    /// **Result (measured 2026-08-03):** `three_phase = false`, `l2 = 0`,
    /// `V = 0.2502840`, `x^{I} = [0.3707417, 0.6292583]`,
    /// `y = [0.8871881, 0.1128119]`; identical to the two-phase flash to < 1e-9,
    /// and the overall mass balance closes to < 1e-10.
    #[test]
    fn reduces_to_two_phase_when_no_second_liquid() {
        use crate::thermo::flash::nested_loops_flash;

        let comps = [reference::methane(), reference::ethane()];
        let z = [0.5, 0.5];
        let (t, p) = (200.0, 2.0e6);
        let eos = CubicEos::PengRobinson;

        let vlle = flash_pt_vlle(&comps, &z, t, p, eos, VlleOptions::default()).unwrap();

        let k_closure = |x: &[f64], y: &[f64], t: f64, p: f64| eos_k_values(eos, &comps, x, y, t, p);
        let vle =
            nested_loops_flash(&z, &comps, t, p, &k_closure, NestedLoopsOptions::default()).unwrap();

        assert!(!vlle.three_phase, "hydrocarbon pair must not form a 2nd liquid");
        assert_abs_diff_eq!(vlle.l2, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(vlle.v, vle.beta, epsilon = 1e-9);
        for i in 0..z.len() {
            assert_abs_diff_eq!(vlle.x1[i], vle.x[i], epsilon = 1e-9);
            assert_abs_diff_eq!(vlle.y[i], vle.y[i], epsilon = 1e-9);
            // Overall mass balance with L2 = 0: z = V y + L1 x1.
            let recon = vlle.v * vlle.y[i] + vlle.l1 * vlle.x1[i] + vlle.l2 * vlle.x2[i];
            assert_abs_diff_eq!(recon, z[i], epsilon = 1e-10);
        }
    }

    /// **Methodology.** Input-validation guards for the fixed-K core.
    /// **Result (measured 2026-08-03):** empty `z` → `Empty`; a `k1` length
    /// mismatch → `LengthMismatch`; a non-positive K → `NonFinite`.
    #[test]
    fn input_validation_errors() {
        assert_eq!(
            solve_3p_fixed_k(&[], &[], &[], 0.3, 0.3, VlleOptions::default()).unwrap_err(),
            FlashError::Empty
        );
        assert!(matches!(
            solve_3p_fixed_k(&[0.5, 0.5], &[2.0], &[1.0, 1.0], 0.3, 0.3, VlleOptions::default())
                .unwrap_err(),
            FlashError::LengthMismatch { .. }
        ));
        assert_eq!(
            solve_3p_fixed_k(&[0.5, 0.5], &[2.0, -1.0], &[1.0, 1.0], 0.3, 0.3, VlleOptions::default())
                .unwrap_err(),
            FlashError::NonFinite
        );
    }
}
