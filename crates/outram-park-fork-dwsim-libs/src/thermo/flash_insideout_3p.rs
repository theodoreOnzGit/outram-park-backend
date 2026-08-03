//! Boston-Fournier **Inside-Out** three-phase (VLLE) isothermal-isobaric
//! (**PT**) flash.
//!
//! Ported from DWSIM
//! `DWSIM.Thermodynamics/FlashAlgorithms/BostonFournierInsideOut3P.vb`
//! (`Flash_PT` orchestration lines 74-225, the Inside-Out three-phase core
//! `Flash_PT_3P` lines 1285-1504, and the inner simple-model residuals
//! `TPErrorFunc` lines 1506-1557 / `SErrorFunc` lines 1571-1581), GPL-3.0,
//! commit `1abf72d`. The second-liquid estimate mirrors the same source's
//! `Flash_PT` lines 176-209. Specific ported lines are cited at each function
//! below.
//!
//! Ref: J. F. Boston, V. B. Fournier, *A quasi-Newton algorithm for solving
//! multiphase equilibrium flash problems* (the Inside-Out family; the
//! two-phase parent is Boston & Britt, Computers & Chemical Engineering **2**
//! (1978) 109-122, <https://doi.org/10.1016/0098-1354(78)80015-5>).
//!
//! # Provenance
//!
//! ```text
//! Upstream project : DWSIM (Daniel Wagner O. de Medeiros)
//! Source file      : DWSIM.Thermodynamics/FlashAlgorithms/BostonFournierInsideOut3P.vb
//! Commit           : 1abf72d
//! Licence          : GPL-3.0
//! ```
//!
//! # What this module computes
//!
//! Given a feed of overall mole fractions `z_i` \[-\] at fixed temperature `T`
//! \[K\] and pressure `P` \[Pa\], split it into up to three coexisting phases —
//! a vapour (`y_i`, molar fraction `V`) and two liquids (`x^{I}_i`, fraction
//! `L^{I}`; `x^{II}_i`, fraction `L^{II}`) with `V + L^{I} + L^{II} = 1`, in
//! mutual equilibrium (`φ_i^V y_i = φ_i^{L I} x^{I}_i = φ_i^{L II} x^{II}_i`).
//! The K-values are `K^{j}_i = φ_i^{L j} / φ_i^{V} = y_i / x^{j}_i`
//! \[-\] for liquid `j ∈ {I, II}`.
//!
//! # The Inside-Out idea, extended to three phases
//!
//! Exactly as the two-phase parent
//! ([`crate::thermo::flash_insideout`]), the rigorous property model (fugacity
//! coefficients from a cubic EOS) is called as *rarely* as possible by wrapping
//! the phase split in **two nested loops**:
//!
//! - **Inner ("inside") loop — cheap simple model.** With the two liquid
//!   K-vectors frozen and the base component pinned to unity (DWSIM `Flash_PT_3P`
//!   line 1363, `Kb = Kb0 = 1`), the compositions and the two liquid fractions
//!   `(L^{I}, L^{II})` are found with **no property-model call**. DWSIM
//!   parametrises this inner solve with a vapour-stripping variable `R` and a
//!   liquid-split variable `S` and minimises `(Kb − 1)^2` over `R` (outer Brent,
//!   `TPErrorFunc`) with `SErrorFunc = 0` over `S` (inner Brent). With the base
//!   pinned to unity that `(R, S)` model is **algebraically the two-equation
//!   three-phase Rachford-Rice system** (see [`inside_out_3p_core`] docs for the
//!   identity), so this port solves that system directly with the already-ported,
//!   more robust damped 2×2 Newton core
//!   [`crate::thermo::flash_vlle::solve_3p_fixed_k`] — the same substitution
//!   [`crate::thermo::flash_insideout`] makes in the two-phase case (Rachford-Rice
//!   root instead of Brent minimisation).
//! - **Outer ("outside") loop — rigorous update.** With the inner
//!   `(x^{I}, x^{II}, y, L^{I}, L^{II})` converged, the rigorous property model
//!   is called *twice* (once per liquid) to recompute
//!   `K^{I} ← k_values(x^{I}, y)`, `K^{II} ← k_values(x^{II}, y)`, the log-K
//!   variables `u^{j}_i = ln K^{j}_i` are updated by plain successive
//!   substitution (DWSIM `Flash_PT_3P` lines 1455-1458, the `fastmode = 0`
//!   branch), and the inner loop is re-entered. Convergence is on
//!   `Σ_i |u^{I}_i − u^{I,new}_i| + Σ_i |u^{II}_i − u^{II,new}_i|` \[-\]
//!   (DWSIM `AbsSum(fx) < etol`, line 1475).
//!
//! # Orchestration (when three phases appear)
//!
//! [`inside_out_flash_3p`] mirrors DWSIM `Flash_PT` (lines 74-225): first a
//! rigorous **two-phase VLE** Inside-Out flash
//! ([`crate::thermo::flash_insideout::inside_out_flash`]); then, if a liquid
//! exists, a **phase-stability test** on that liquid
//! ([`crate::thermo::stability::stability_test`], the analogue of DWSIM's
//! `StabTest2`) to detect a distinct second liquid. Only if the liquid is
//! unstable does the three-phase Inside-Out core [`inside_out_3p_core`] run;
//! otherwise the two-phase result is returned unchanged (VLLE with `L^{II} = 0`).
//!
//! This is the identical orchestration as the nested-loops three-phase port
//! [`crate::thermo::flash_vlle::flash_pt_vlle`]; the sole difference is that the
//! three-phase *inner/outer* split here is organised as Inside-Out (fully solve
//! the frozen-K inner system, then successive-substitute the log-K), whereas
//! `flash_pt_vlle` interleaves one Newton step per rigorous-K refresh.
//!
//! # Honest scope (verification, not benchmark validation, and a *partial* port)
//!
//! Three-phase flash robustness is genuinely hard, and this is a **first port**:
//!
//! - **Base component pinned to unity** (`Kb = Kb0 = 1`), following the DWSIM
//!   source exactly (its `CalcKbjw` base-component selector, line 1363, is
//!   commented out). The variable-base Boston-Fournier simple K-model is therefore
//!   **not** reproduced; the inner model is the unity-referenced three-phase
//!   Rachford-Rice split.
//! - **PT specification with plain successive substitution** (`fastmode = 0`).
//!   DWSIM's Broyden `fastmode` acceleration (lines 1433-1451) and the PH / PS /
//!   TV / PV energy flashes are out of scope.
//! - **Second-liquid detection is only as good as the two Wilson-seeded stability
//!   trials** ([`crate::thermo::stability`]); a liquid-liquid split neither Wilson
//!   seed reaches is missed and the flash silently returns two phases. No global
//!   TPD minimisation.
//! - **Liquid labelling is not physically canonical.** DWSIM condenses trivial
//!   (identical) liquids via `AUX_CheckTrivial` and orders the two liquids by
//!   density `AUX_LIQDENS` (lines 1485-1500). This port applies a
//!   composition-distance trivial-liquid check (condense to two phases when the
//!   two liquid compositions coincide) but does **not** density-order the two
//!   liquids — that needs an absolute-density closure this K-only interface does
//!   not expose. Which liquid is labelled `L^{I}` vs `L^{II}` is therefore not
//!   canonical; mass balance and the sum-to-one identities (the V&V checks) are
//!   independent of the labelling.
//! - **`k_ij = 0`** throughout (geometric-mean mixing), which makes a genuine
//!   liquid-liquid split under a cubic EOS with the bundled reference compounds
//!   unlikely; the three-phase numerics are therefore verified on the **fixed-K**
//!   core [`inside_out_3p_core`] (constant K-closure) against the algebraic
//!   mass-balance identity and against the already-ported
//!   [`crate::thermo::flash_vlle::solve_3p_fixed_k`], and the composed driver is
//!   verified to **reduce to the two-phase result** when no second liquid is
//!   found. A full EOS-driven LLE benchmark is deferred.
//!
//! > **⚠️ Unverified until validated.** AI-assisted **partial** port — untrusted
//! > draft material until human-reviewed per the crate `CLAUDE.md`. Verification,
//! > not validation. Not for nuclear facility operation, reactor control,
//! > safety-critical, or licensing decisions. Independent OUTRAM PARK fork, not
//! > the official DWSIM.
//!
//! # Design (workspace + crate `CLAUDE.md`)
//!
//! Enum dispatch (the fugacity model is the [`CubicEos`] **enum**), no trait
//! objects / `dyn` / `Box` / lifetimes / channels. The rigorous K-update is a
//! **generic `Fn` closure**, so this module carries no dependency on the EOS /
//! activity code and no `dyn` dispatch. Compositions owned by value; documented
//! raw `f64` (SI: K, Pa, mole fractions \[-\]) in the inner loops.

use crate::thermo::cubic_eos::CubicEos;
use crate::thermo::flash::{FlashError, FlashResult, NestedLoopsOptions};
use crate::thermo::flash_insideout::{inside_out_flash, InsideOutOptions};
use crate::thermo::flash_vlle::{eos_k_values, solve_3p_fixed_k, VlleOptions, VlleResult};
use crate::thermo::stability::stability_test;
use crate::thermo::Component;

/// Tuning parameters for [`inside_out_3p_core`] and [`inside_out_flash_3p`].
///
/// Combines the Inside-Out **outer** (rigorous-K successive-substitution)
/// controls with the [`VlleOptions`] that bound the frozen-K **inner** three-phase
/// Newton solve ([`crate::thermo::flash_vlle::solve_3p_fixed_k`]).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InsideOut3POptions {
    /// Maximum outer (rigorous-K successive-substitution) iterations before
    /// returning [`FlashError::NotConverged`]. DWSIM `maxit_e` default is 100.
    pub max_outer_iter: usize,
    /// Outer convergence tolerance on
    /// `Σ_i |u^{I}_i − u^{I,new}_i| + Σ_i |u^{II}_i − u^{II,new}_i|` \[-\], the
    /// summed absolute change in the two liquids' log-K variables (DWSIM
    /// `AbsSum(fx) < etol`, line 1475).
    pub outer_tol: f64,
    /// A liquid phase whose fraction falls below this \[-\] is treated as absent
    /// (the split has collapsed back to two phases).
    pub min_phase_fraction: f64,
    /// Composition distance `Σ_i |x^{I}_i − x^{II}_i|` \[-\] below which the two
    /// liquids are deemed identical (the trivial-liquid solution) and condensed
    /// to a single liquid — the K-only analogue of DWSIM `AUX_CheckTrivial`
    /// (line 1485).
    pub trivial_tol: f64,
    /// Controls for the frozen-K inner three-phase Newton solve
    /// ([`crate::thermo::flash_vlle::solve_3p_fixed_k`]).
    pub inner: VlleOptions,
}

impl Default for InsideOut3POptions {
    fn default() -> Self {
        Self {
            max_outer_iter: 100,
            outer_tol: 1.0e-10,
            min_phase_fraction: 1.0e-8,
            trivial_tol: 1.0e-6,
            inner: VlleOptions::default(),
        }
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

/// Build the DWSIM second-liquid estimate from a stability-test trial
/// composition (DWSIM `Flash_PT` lines 176-209; identical construction to
/// [`crate::thermo::flash_vlle`]'s private `phase_split_estimate`).
///
/// Given the equilibrium liquid `x_liq` (fraction `l_total` of the feed) and the
/// destabilising trial composition `x2_trial` (the second liquid), returns
/// `(L^{I}, x^{I}, L^{II}, x^{II})` scaled so `L^{I} + L^{II} = l_total`:
/// `L^{II} = l_total · x_liq[m] · max(x2_trial)` for the component `m` where
/// `x2_trial` peaks, and `x^{I} = (x_liq − x2_trial · L^{II}/l_total)`
/// renormalised. All arguments/returns are mole fractions \[-\] except the two
/// molar phase fractions \[-\].
fn phase_split_estimate(
    x_liq: &[f64],
    x2_trial: &[f64],
    l_total: f64,
) -> (f64, Vec<f64>, f64, Vec<f64>) {
    let n = x_liq.len();
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

/// Package a converged frozen-K three-phase split plus its two rigorous K-vectors
/// into a [`VlleResult`].
fn split_to_result(
    split: &crate::thermo::flash_vlle::ThreePhaseSplit,
    k1: &[f64],
    k2: &[f64],
    iterations: usize,
) -> VlleResult {
    VlleResult {
        v: split.v,
        l1: split.l1,
        l2: split.l2,
        y: split.y.clone(),
        x1: split.x1.clone(),
        x2: split.x2.clone(),
        k1: k1.to_vec(),
        k2: k2.to_vec(),
        three_phase: true,
        iterations,
    }
}

/// Condense a three-phase result to a two-phase (`L^{II} = 0`) result: the two
/// liquids are combined into liquid I and the second liquid is zeroed.
fn condense_to_two_phase(mut r: VlleResult) -> VlleResult {
    r.l1 += r.l2;
    r.l2 = 0.0;
    r.x2 = r.x1.clone();
    r.k2 = r.k1.clone();
    r.three_phase = false;
    r
}

/// **Inside-Out three-phase core**: the frozen-K inner three-phase split driven
/// to rigorous-K self-consistency by successive substitution on the two liquids'
/// log-K variables.
///
/// Ported from DWSIM `BostonFournierInsideOut3P.vb` `Flash_PT_3P` (lines
/// 1285-1504), `fastmode = 0` (plain successive substitution) branch, base
/// component pinned to unity (`Kb = Kb0 = 1`, line 1363).
///
/// # The inner model *is* three-phase Rachford-Rice (the base-unity identity)
///
/// DWSIM's inner simple model (`TPErrorFunc`, line 1523) writes the un-normalised
/// vapour amounts, with `Kb0 = 1`, as
///
/// ```text
/// p_i = z_i / [ R + (1 − R + S) / (2 K^{I}_i) + (1 − R − S) / (2 K^{II}_i) ],
/// ```
///
/// with the liquid fractions recovered (lines 1546-1548) as `L^{I} = ½(1 + S −
/// V)`, `L^{II} = ½(1 − S − V)` at the converged base `Kb = 1`. Substituting
/// `R = V`, the bracket equals
/// `V + L^{I}/K^{I}_i + L^{II}/K^{II}_i = 1 − β^{I}_i L^{I} − β^{II}_i L^{II}
/// = D_i`, with `β^{j}_i = 1 − 1/K^{j}_i` — exactly the denominator of the
/// two-equation three-phase Rachford-Rice system solved in
/// [`crate::thermo::flash_vlle`]. Moreover DWSIM's inner `S`-residual
/// (`SErrorFunc`, line 1577) is `Σ_i z_i (1/K^{I}_i − 1/K^{II}_i) / D_i`, which is
/// precisely `F_2 − F_1` of that system, and its `Kb = 1` condition closes the
/// remaining equation. This port therefore solves the frozen-K inner split with
/// the already-ported, monotone-well-posed damped 2×2 Newton
/// [`crate::thermo::flash_vlle::solve_3p_fixed_k`], targeting the identical root
/// as DWSIM's Brent-in-Brent `(R, S)` minimisation — the direct three-phase
/// analogue of the substitution [`crate::thermo::flash_insideout`] makes.
///
/// # Algorithm
///
/// 1. **Seed.** Frozen liquid K-vectors `K^{I}`, `K^{II}` and liquid-fraction
///    seeds `L^{I}_est`, `L^{II}_est`. Inner solve → converged
///    `(x^{I}, x^{II}, y, L^{I}, L^{II})`.
/// 2. **Outer loop** (rigorous K update, DWSIM lines 1385-1475), up to
///    `opts.max_outer_iter` passes:
///    a. **Rigorous K** — `K^{I,new} ← k_values(x^{I}, y)`,
///       `K^{II,new} ← k_values(x^{II}, y)` (DWSIM lines 1410-1411).
///    b. **Outer residual** — `Σ|u^{I} − u^{I,new}| + Σ|u^{II} − u^{II,new}|`
///       with `u = ln K` (DWSIM `AbsSum(fx)`, line 1475).
///    c. **Successive substitution** — `u^{j} ← ln K^{j,new}` (lines 1455-1458).
///    d. **Inner solve** — [`crate::thermo::flash_vlle::solve_3p_fixed_k`] on the
///       new K, reseeded from the previous `(L^{I}, L^{II})`.
///    e. **Convergence** — stop when the residual `< opts.outer_tol`.
/// 3. **Trivial-liquid guard** — if the two converged liquids coincide
///    (`Σ|x^{I}_i − x^{II}_i| < opts.trivial_tol`) the split is condensed to two
///    phases (DWSIM `AUX_CheckTrivial`, line 1485).
///
/// # The K-closure (decoupling boundary)
///
/// `k_values(x, y, T, P) -> Vec<f64>`: given a trial liquid `x` and the vapour
/// `y` (mole fractions \[-\]) at `T` \[K\], `P` \[Pa\], returns rigorous K-values
/// \[-\] for that liquid from a fugacity property model. It is called **twice** per
/// outer pass (once per liquid). A **generic `Fn`**, not a trait object — so this
/// module stays free of the EOS / activity code and of `dyn` dispatch.
///
/// # Units / ranges
///
/// `z`, `k1_init`, `k2_init`: equal length `n ≥ 1`; `z` feed mole fractions \[-\];
/// `k1_init`, `k2_init` liquid/vapour K-values \[-\] (`> 0`). `l1_est`, `l2_est`
/// ∈ `(0, 1)` with `l1_est + l2_est < 1` seed the inner Newton iteration.
/// `t` \[K\] > 0, `p` \[Pa\] > 0. `opts` bounds the iterations and tolerances.
///
/// # Returns
///
/// A [`VlleResult`] with `v = 1 − L^{I} − L^{II}` and the normalised
/// compositions. At convergence the inner `F_1 = F_2 = 0`, so `Σ y = Σ x^{I} =
/// Σ x^{II} = 1` and the overall mass balance
/// `z_i = v y_i + L^{I} x^{I}_i + L^{II} x^{II}_i` closes. `three_phase` is
/// `false` (and `l2 = 0`) when the trivial-liquid guard condensed the split.
///
/// # Errors
///
/// [`FlashError::Empty`] on empty `z`; [`FlashError::LengthMismatch`] on a size
/// mismatch (including a closure-output size mismatch);
/// [`FlashError::NonFinite`] on a non-finite / non-positive K;
/// [`FlashError::NotConverged`] if the outer successive substitution does not
/// reach `opts.outer_tol` within `opts.max_outer_iter`. Propagates
/// [`crate::thermo::flash_vlle::solve_3p_fixed_k`] errors from the inner solve.
pub fn inside_out_3p_core<F>(
    z: &[f64],
    k1_init: &[f64],
    k2_init: &[f64],
    l1_est: f64,
    l2_est: f64,
    t: f64,
    p: f64,
    k_values: F,
    opts: InsideOut3POptions,
) -> Result<VlleResult, FlashError>
where
    F: Fn(&[f64], &[f64], f64, f64) -> Vec<f64>,
{
    let n = z.len();
    if n == 0 {
        return Err(FlashError::Empty);
    }
    if k1_init.len() != n {
        return Err(FlashError::LengthMismatch { a: n, b: k1_init.len() });
    }
    if k2_init.len() != n {
        return Err(FlashError::LengthMismatch { a: n, b: k2_init.len() });
    }
    if k1_init.iter().chain(k2_init.iter()).any(|&v| !v.is_finite() || v <= 0.0) {
        return Err(FlashError::NonFinite);
    }

    let mut k1 = k1_init.to_vec();
    let mut k2 = k2_init.to_vec();
    let mut u1: Vec<f64> = k1.iter().map(|&ki| ki.ln()).collect();
    let mut u2: Vec<f64> = k2.iter().map(|&ki| ki.ln()).collect();

    // Inner ("inside") frozen-K three-phase split — no property-model call.
    let mut split = solve_3p_fixed_k(z, &k1, &k2, l1_est, l2_est, opts.inner)?;

    for iter in 1..=opts.max_outer_iter {
        // (a) Rigorous K from the inner-converged trial split (once per liquid).
        let k1_new = k_values(&split.x1, &split.y, t, p);
        let k2_new = k_values(&split.x2, &split.y, t, p);
        if k1_new.len() != n || k2_new.len() != n {
            return Err(FlashError::LengthMismatch {
                a: n,
                b: k1_new.len().max(k2_new.len()),
            });
        }
        if k1_new.iter().chain(k2_new.iter()).any(|&v| !v.is_finite() || v <= 0.0) {
            return Err(FlashError::NonFinite);
        }

        // (b) Outer residual: summed absolute change in the log-K variables.
        let u1_new: Vec<f64> = k1_new.iter().map(|&ki| ki.ln()).collect();
        let u2_new: Vec<f64> = k2_new.iter().map(|&ki| ki.ln()).collect();
        let residual: f64 = u1
            .iter()
            .zip(u1_new.iter())
            .map(|(&uo, &un)| (uo - un).abs())
            .sum::<f64>()
            + u2
                .iter()
                .zip(u2_new.iter())
                .map(|(&uo, &un)| (uo - un).abs())
                .sum::<f64>();

        // (c) Successive substitution on the log-K variables (fastmode = 0).
        u1 = u1_new;
        u2 = u2_new;
        k1 = k1_new;
        k2 = k2_new;

        // (d) Re-solve the inner split on the refreshed K, reseeded from the
        //     previous liquid fractions.
        split = solve_3p_fixed_k(z, &k1, &k2, split.l1, split.l2, opts.inner)?;

        // (e) Convergence on the successive-substitution residual.
        if residual < opts.outer_tol {
            let result = split_to_result(&split, &k1, &k2, iter);
            // Trivial-liquid guard (DWSIM AUX_CheckTrivial): condense identical
            // liquids to a single liquid phase.
            let comp_dist: f64 = (0..n).map(|i| (split.x1[i] - split.x2[i]).abs()).sum();
            if comp_dist < opts.trivial_tol || split.l2 <= opts.min_phase_fraction {
                return Ok(condense_to_two_phase(result));
            }
            return Ok(result);
        }
    }

    Err(FlashError::NotConverged {
        iterations: opts.max_outer_iter,
        residual: f64::NAN,
    })
}

/// Full **Boston-Fournier Inside-Out three-phase VLLE** isothermal-isobaric flash
/// of feed `z` at `T` \[K\], `P` \[Pa\] using the cubic EOS `eos` (`k_ij = 0`).
///
/// Ported from DWSIM `BostonFournierInsideOut3P.vb` `Flash_PT` (lines 74-225).
///
/// # Orchestration
///
/// 1. **Two-phase VLE** via the Inside-Out parent
///    ([`crate::thermo::flash_insideout::inside_out_flash`]) with the EOS
///    K-closure ([`crate::thermo::flash_vlle::eos_k_values`]).
/// 2. If a liquid exists, **stability-test** it
///    ([`crate::thermo::stability::stability_test`]). Stable ⇒ return the
///    two-phase result (`l2 = 0`).
/// 3. Unstable ⇒ build a second-liquid estimate ([`phase_split_estimate`]) and
///    run the **three-phase Inside-Out core** ([`inside_out_3p_core`]).
/// 4. If the second liquid collapses below `opts.min_phase_fraction`, or the two
///    liquids turn out trivially identical, fall back to the two-phase result.
///
/// # Units / ranges
///
/// `components.len() == z.len()`; `z` feed mole fractions \[-\] (sum to 1);
/// `t` \[K\] > 0, `p` \[Pa\] > 0. See the module scope note for the honest limits
/// (label ordering, missed splits, base pinned to unity, `k_ij = 0`).
///
/// # Errors
///
/// [`FlashError::LengthMismatch`] on a `components`/`z` size mismatch; propagates
/// [`FlashError`] from the two-phase Inside-Out flash and the three-phase core.
pub fn inside_out_flash_3p(
    components: &[Component],
    z: &[f64],
    t: f64,
    p: f64,
    eos: CubicEos,
    opts: InsideOut3POptions,
) -> Result<VlleResult, FlashError> {
    if components.len() != z.len() {
        return Err(FlashError::LengthMismatch {
            a: z.len(),
            b: components.len(),
        });
    }

    // --- Step 1: rigorous two-phase VLE via the Inside-Out parent. ---
    let k_closure = |x: &[f64], y: &[f64], t: f64, p: f64| eos_k_values(eos, components, x, y, t, p);
    let io_opts = InsideOutOptions {
        max_outer_iter: opts.max_outer_iter,
        ..InsideOutOptions::default()
    };
    let vle: FlashResult = inside_out_flash(z, components, t, p, &k_closure, io_opts)?;

    let l_total = 1.0 - vle.beta;

    // Package the two-phase result as a (degenerate) VLLE result.
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

    // --- Step 3: build the 3-phase estimate and run the Inside-Out core. ---
    let (l1, x1_est, l2, x2_est) = phase_split_estimate(&vle.x, &x2_trial, l_total);
    if l2 <= opts.min_phase_fraction {
        return Ok(two_phase(&vle, 0));
    }
    let y_est = vle.y.clone();

    // Seed the two liquid K-vectors from the EOS on the estimate compositions.
    let k1_init = eos_k_values(eos, components, &x1_est, &y_est, t, p);
    let k2_init = eos_k_values(eos, components, &x2_est, &y_est, t, p);

    let result = inside_out_3p_core(
        z, &k1_init, &k2_init, l1, l2, t, p, &k_closure, opts,
    )?;

    // Step 4: a collapsed / trivial second liquid means genuine two-phase.
    if !result.three_phase || result.l2 <= opts.min_phase_fraction {
        return Ok(two_phase(&vle, result.iterations));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    //! # V&V — verification of the Inside-Out three-phase (VLLE) flash
    //!
    //! **Scope (honesty).** Verification of the algebraic identities, the
    //! constant-K reduction, and the two-phase reduction — NOT validation against
    //! measured VLLE data. Base component pinned to unity, plain successive
    //! substitution (no Broyden), `k_ij = 0`, no density-ordering of the two
    //! liquids. Numbers below were **measured** on 2026-08-03 by compiling this
    //! module into the crate and running
    //! `cargo test -p outram-park-fork-dwsim-libs --lib --release`.

    use super::*;
    use crate::thermo::component::reference;
    use crate::thermo::flash::nested_loops_flash;
    use crate::thermo::flash_insideout::inside_out_flash;
    use crate::thermo::property_package::PropertyPackageModel;
    use approx::assert_abs_diff_eq;

    /// **Methodology (V&V check 1 — symmetric fixed-K three-phase split).** With a
    /// **constant** K-closure the Inside-Out core must, on convergence, close the
    /// overall mass balance `z_i = V y_i + L^{I} x^{I}_i + L^{II} x^{II}_i`, make
    /// each phase composition sum to 1, and keep `V + L^{I} + L^{II} = 1`. It must
    /// also reproduce the already-ported frozen-K core
    /// [`crate::thermo::flash_vlle::solve_3p_fixed_k`] on the same K. Synthetic
    /// immiscible ternary: `z = [0.4, 0.3, 0.3]`, component 0 volatile in both
    /// liquids (`K^{I}_0 = K^{II}_0 = 4`), component 1 concentrates in liquid I
    /// (`K^{I}_1 = 0.3`, `K^{II}_1 = 2`), component 2 in liquid II
    /// (`K^{I}_2 = 2`, `K^{II}_2 = 0.3`); seeds `L^{I} = L^{II} = 0.3`.
    /// **Result (measured 2026-08-03):** converges in `iterations = 1` (constant
    /// closure ⇒ zero log-K change on the first pass) to a genuine three-phase
    /// split `V = 0.6363636`, `L^{I} = 0.1818182`, `L^{II} = 0.1818182` (the
    /// symmetric split forced by the symmetric K); each phase sums to 1 to
    /// < 1e-12, `V + L^{I} + L^{II} = 1` to < 1e-12, the overall mass balance
    /// closes to < 1e-9, and the split is identical to
    /// [`crate::thermo::flash_vlle::solve_3p_fixed_k`] to < 1e-12.
    #[test]
    fn symmetric_fixed_k_three_phase_mass_balance() {
        let z = [0.4, 0.3, 0.3];
        let k1 = [4.0, 0.3, 2.0];
        let k2 = [4.0, 2.0, 0.3];
        // Frozen-K reference from the already-ported nested-loops 3-phase core.
        let split =
            solve_3p_fixed_k(&z, &k1, &k2, 0.3, 0.3, VlleOptions::default()).unwrap();

        // Drive the Inside-Out core with a per-liquid closure that echoes the
        // seed-consistent K: at the frozen split `x^{j}_i = y_i / K^{j}_i`, so
        // `y_i / x_i` returns `K^{I}` when called on `(x^{I}, y)` and `K^{II}` on
        // `(x^{II}, y)` — a fixed point of the successive substitution.
        let core = inside_out_3p_core(
            &z,
            &k1,
            &k2,
            0.3,
            0.3,
            300.0,
            1.0e5,
            |x: &[f64], y: &[f64], _t: f64, _p: f64| {
                // Recover the constant K consistent with the frozen split: for the
                // converged split, K_i = y_i / x_i. This reproduces k1 when called
                // on (x1, y) and k2 when called on (x2, y), so the successive
                // substitution is a fixed point from the first pass.
                (0..x.len()).map(|i| y[i] / x[i]).collect()
            },
            InsideOut3POptions::default(),
        )
        .unwrap();

        assert!(core.three_phase, "expected a genuine three-phase split");
        assert!(core.v > 0.0 && core.v < 1.0);
        assert!(core.l1 > 0.0 && core.l2 > 0.0);
        assert_abs_diff_eq!(core.v + core.l1 + core.l2, 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(core.y.iter().sum::<f64>(), 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(core.x1.iter().sum::<f64>(), 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(core.x2.iter().sum::<f64>(), 1.0, epsilon = 1e-12);
        for i in 0..z.len() {
            let recon = core.v * core.y[i] + core.l1 * core.x1[i] + core.l2 * core.x2[i];
            assert_abs_diff_eq!(recon, z[i], epsilon = 1e-9);
        }
        // Reproduces the frozen-K core exactly (labels may map I<->II symmetrically;
        // here the symmetric split makes them coincide).
        assert_abs_diff_eq!(core.v, split.v, epsilon = 1e-12);
        assert_abs_diff_eq!(core.l1, split.l1, epsilon = 1e-12);
        assert_abs_diff_eq!(core.l2, split.l2, epsilon = 1e-12);
        assert_eq!(core.iterations, 1, "constant closure converges on the first pass");
    }

    /// **Methodology (V&V check 1 — asymmetric fixed-K three-phase split).** The
    /// same identities away from the symmetric split, with all three phases
    /// genuinely present. `z = [0.4, 0.3, 0.3]`, `K^{I} = [3.0, 0.25, 2.5]`
    /// (comp 1 favours liquid I), `K^{II} = [3.5, 2.5, 0.28]` (comp 2 favours
    /// liquid II); seeds `L^{I} = L^{II} = 0.3`; the per-liquid closure echoes the
    /// seed-consistent K (a fixed point from the first pass).
    /// **Result (measured 2026-08-03):** converges in `iterations = 1` to
    /// `V = 0.4985306`, `L^{I} = 0.2740227`, `L^{II} = 0.2274467` (an asymmetric,
    /// strictly-positive three-phase split); each phase sums to 1 (< 1e-12),
    /// fractions sum to 1 (< 1e-12), overall mass balance closes to < 1e-9, and the
    /// split matches [`crate::thermo::flash_vlle::solve_3p_fixed_k`] to < 1e-12.
    #[test]
    fn asymmetric_fixed_k_three_phase_mass_balance() {
        let z = [0.4, 0.3, 0.3];
        let k1 = [3.0, 0.25, 2.5];
        let k2 = [3.5, 2.5, 0.28];
        let split =
            solve_3p_fixed_k(&z, &k1, &k2, 0.3, 0.3, VlleOptions::default()).unwrap();

        let core = inside_out_3p_core(
            &z,
            &k1,
            &k2,
            0.3,
            0.3,
            300.0,
            1.0e5,
            |x: &[f64], y: &[f64], _t: f64, _p: f64| {
                (0..x.len()).map(|i| y[i] / x[i]).collect()
            },
            InsideOut3POptions::default(),
        )
        .unwrap();

        assert!(core.three_phase);
        assert!(core.v > 0.0 && core.v < 1.0);
        assert!(core.l1 > 0.0 && core.l2 > 0.0);
        assert!((core.l1 - core.l2).abs() > 1e-3, "expected an asymmetric split");
        assert_abs_diff_eq!(core.v + core.l1 + core.l2, 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(core.y.iter().sum::<f64>(), 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(core.x1.iter().sum::<f64>(), 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(core.x2.iter().sum::<f64>(), 1.0, epsilon = 1e-12);
        for i in 0..z.len() {
            let recon = core.v * core.y[i] + core.l1 * core.x1[i] + core.l2 * core.x2[i];
            assert_abs_diff_eq!(recon, z[i], epsilon = 1e-9);
        }
        assert_abs_diff_eq!(core.v, split.v, epsilon = 1e-12);
        assert_abs_diff_eq!(core.l1, split.l1, epsilon = 1e-12);
        assert_abs_diff_eq!(core.l2, split.l2, epsilon = 1e-12);
    }

    /// **Methodology (V&V check 3 — agreement with the nested-loops 3-phase core
    /// on a common fixed-K case).** On the same feed and same fixed K-vectors, the
    /// Inside-Out core [`inside_out_3p_core`] (constant closure) and the
    /// nested-loops fixed-K core
    /// [`crate::thermo::flash_vlle::solve_3p_fixed_k`] must converge to the
    /// **same** three-phase root — both target `F_1 = F_2 = 0`. Asymmetric ternary
    /// as above. **Result (measured 2026-08-03):** the two agree on `V`, `L^{I}`,
    /// `L^{II}` and every phase composition to < 1e-12 (they share the identical
    /// inner solver at a fixed point, so agreement is exact to round-off).
    #[test]
    fn agrees_with_nested_loops_fixed_k_core() {
        let z = [0.4, 0.3, 0.3];
        let k1 = [3.0, 0.25, 2.5];
        let k2 = [3.5, 2.5, 0.28];

        let nl = solve_3p_fixed_k(&z, &k1, &k2, 0.3, 0.3, VlleOptions::default()).unwrap();
        let io = inside_out_3p_core(
            &z,
            &k1,
            &k2,
            0.3,
            0.3,
            300.0,
            1.0e5,
            |x: &[f64], y: &[f64], _t: f64, _p: f64| {
                (0..x.len()).map(|i| y[i] / x[i]).collect()
            },
            InsideOut3POptions::default(),
        )
        .unwrap();

        assert_abs_diff_eq!(io.v, nl.v, epsilon = 1e-12);
        assert_abs_diff_eq!(io.l1, nl.l1, epsilon = 1e-12);
        assert_abs_diff_eq!(io.l2, nl.l2, epsilon = 1e-12);
        for i in 0..z.len() {
            assert_abs_diff_eq!(io.y[i], nl.y[i], epsilon = 1e-12);
            assert_abs_diff_eq!(io.x1[i], nl.x1[i], epsilon = 1e-12);
            assert_abs_diff_eq!(io.x2[i], nl.x2[i], epsilon = 1e-12);
        }
    }

    /// **Methodology (V&V check 2 — two-phase reduction).** For a feed that is a
    /// genuine VLE two-phase mixture with **no** liquid-liquid split, the composed
    /// Inside-Out three-phase driver must detect a stable liquid and return exactly
    /// the two-phase result (`L^{II} = 0`), matching both the direct two-phase
    /// Inside-Out flash [`crate::thermo::flash_insideout::inside_out_flash`] and
    /// the nested-loops flash [`crate::thermo::flash::nested_loops_flash`]. Feed
    /// methane/ethane `z = [0.5, 0.5]`, `T = 200 K`, `P = 2·10⁶ Pa`,
    /// Peng-Robinson — a hydrocarbon pair that forms one liquid, not two.
    /// **Result (measured 2026-08-03):** `three_phase = false`, `l2 = 0`,
    /// `V = 0.2502841`, `x^{I} = [0.3707417, 0.6292583]`,
    /// `y = [0.8871881, 0.1128119]`; identical to the two-phase Inside-Out and
    /// nested-loops flashes to < 1e-6, and the overall mass balance closes to
    /// < 1e-9.
    #[test]
    fn reduces_to_two_phase_when_no_second_liquid() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.5, 0.5];
        let (t, p) = (200.0, 2.0e6);
        let eos = CubicEos::PengRobinson;
        let pkg = PropertyPackageModel::PengRobinson;

        let vlle = inside_out_flash_3p(&comps, &z, t, p, eos, InsideOut3POptions::default())
            .unwrap();

        let k_closure = |x: &[f64], y: &[f64], t: f64, p: f64| pkg.k_values(&comps, x, y, t, p);
        let io2 = inside_out_flash(&z, &comps, t, p, &k_closure, InsideOutOptions::default())
            .unwrap();
        let nl = nested_loops_flash(&z, &comps, t, p, &k_closure, NestedLoopsOptions::default())
            .unwrap();

        assert!(!vlle.three_phase, "hydrocarbon pair must not form a 2nd liquid");
        assert_abs_diff_eq!(vlle.l2, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(vlle.v, io2.beta, epsilon = 1e-6);
        assert_abs_diff_eq!(vlle.v, nl.beta, epsilon = 1e-6);
        for i in 0..z.len() {
            assert_abs_diff_eq!(vlle.x1[i], io2.x[i], epsilon = 1e-6);
            assert_abs_diff_eq!(vlle.y[i], io2.y[i], epsilon = 1e-6);
            let recon = vlle.v * vlle.y[i] + vlle.l1 * vlle.x1[i] + vlle.l2 * vlle.x2[i];
            assert_abs_diff_eq!(recon, z[i], epsilon = 1e-9);
        }
    }

    /// **Methodology.** Input-validation guards for the Inside-Out core.
    /// **Result (measured 2026-08-03):** empty `z` → `Empty`; a `k1` length
    /// mismatch → `LengthMismatch`; a non-positive K → `NonFinite`.
    #[test]
    fn input_validation_errors() {
        let closure = |_x: &[f64], _y: &[f64], _t: f64, _p: f64| vec![1.0, 1.0];
        assert_eq!(
            inside_out_3p_core(&[], &[], &[], 0.3, 0.3, 300.0, 1e5, &closure, InsideOut3POptions::default())
                .unwrap_err(),
            FlashError::Empty
        );
        assert!(matches!(
            inside_out_3p_core(&[0.5, 0.5], &[2.0], &[1.0, 1.0], 0.3, 0.3, 300.0, 1e5, &closure, InsideOut3POptions::default())
                .unwrap_err(),
            FlashError::LengthMismatch { .. }
        ));
        assert_eq!(
            inside_out_3p_core(&[0.5, 0.5], &[2.0, -1.0], &[1.0, 1.0], 0.3, 0.3, 300.0, 1e5, &closure, InsideOut3POptions::default())
                .unwrap_err(),
            FlashError::NonFinite
        );
    }
}
