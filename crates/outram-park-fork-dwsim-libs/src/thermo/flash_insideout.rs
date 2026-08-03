//! Boston-Britt **Inside-Out** two-phase (VLE) isothermal-isobaric (**PT**) flash.
//!
//! Ported from DWSIM `DWSIM.Thermodynamics/FlashAlgorithms/BostonBrittInsideOut.vb`
//! (`Flash_PT`, lines 76-368), GPL-3.0, commit `1abf72d`. The Wilson K-value
//! first guess is DWSIM `BostonBrittInsideOut.vb` line 114 / 437. Specific ported
//! lines are cited at each function below.
//!
//! Ref: J. F. Boston, H. I. Britt, *A radically different formulation and
//! solution of the single-stage flash problem*, Computers & Chemical
//! Engineering **2** (1978) 109-122
//! (<https://doi.org/10.1016/0098-1354(78)80015-5>).
//!
//! # Provenance
//!
//! ```text
//! Upstream project : DWSIM (Daniel Wagner O. de Medeiros)
//! Source file      : DWSIM.Thermodynamics/FlashAlgorithms/BostonBrittInsideOut.vb
//! Commit           : 1abf72d
//! Licence          : GPL-3.0
//! ```
//!
//! # What this module computes
//!
//! Given a feed of overall mole fractions `z_i` \[-\] at fixed temperature `T`
//! \[K\] and pressure `P` \[Pa\], split it into an equilibrium liquid (`x_i`) and
//! vapour (`y_i`), returning the vapour molar fraction `V ≡ β` \[-\] and the
//! K-values `K_i = y_i / x_i` \[-\].
//!
//! # The Inside-Out idea (why it exists)
//!
//! A rigorous property model (fugacity coefficients from a cubic EOS) is
//! expensive. The Inside-Out method wraps the phase-split solve in **two nested
//! loops** that call the rigorous model as *rarely* as possible:
//!
//! - **Inner ("inside") loop — cheap simple model.** The K-values are frozen and
//!   re-expressed through the log-relative-volatility variables `u_i = ln K_i`
//!   (DWSIM `BostonBrittInsideOut.vb` line 240; the base-component reference is
//!   pinned to `K_b = K_b0 = 1` in the DWSIM source, lines 236-237, so `u_i` is
//!   taken relative to unity). Holding `u` fixed, the vapour fraction is found
//!   from the **mole-balance / energy-balance** stationarity condition
//!   [`inner_stripping_solve`] — no property-model call happens here.
//! - **Outer ("outside") loop — rigorous update.** With the inner `(x, y)`
//!   converged, the rigorous property model is called *once* to recompute the
//!   K-values (`K_i ← k_values(x, y, T, P)`), the `u_i` are updated by successive
//!   substitution (DWSIM line 344, non-`fastmode`), and the inner loop is
//!   re-entered. Convergence is on `Σ_i (u_i^{old} − u_i^{new})²` \[-\]
//!   (DWSIM `AbsSqrSumY(fx) < etol`, line 358).
//!
//! With the base component pinned to unity (as in the DWSIM source), the inner
//! simple model reduces algebraically to a Rachford-Rice solve — see
//! [`inner_stripping_solve`] — so at a fixed K-vector the Inside-Out split is
//! *identical* to the classic nested-loops split
//! ([`crate::thermo::flash::solve_rachford_rice`]). The two methods therefore
//! converge to the **same** fixed point `K_i = k_values(x, y, T, P)`; only the
//! bookkeeping of *how* the outer K-update is organised differs. This identity is
//! exactly V&V check (1) below and is the strongest correctness test available.
//!
//! # The K-closure (decoupling boundary)
//!
//! Mirroring [`crate::thermo::flash::nested_loops_flash`], the sole
//! model-dependent step — turning a trial `(x, y, T, P)` into rigorous K-values —
//! is handed in as a **generic `Fn` closure**, *not* a trait object. This module
//! therefore has no dependency on the EOS / activity code and no `dyn` dispatch,
//! per the workspace `CLAUDE.md` (no trait objects / `Box` / lifetimes).
//!
//! # Honest scope (verification, not benchmark validation)
//!
//! - **PT (isothermal-isobaric) two-phase VLE only.** DWSIM's
//!   `BostonBrittInsideOut.vb` also carries PH / PS / TV / PV energy flashes and a
//!   Broyden `fastmode` acceleration (lines 324-347); this port implements the
//!   **PT specification with plain successive substitution** (the `fastmode = 0`
//!   branch). The energy flashes and the Broyden acceleration are out of scope.
//! - **Base component pinned to unity** (`K_b = 1`), following the DWSIM source
//!   exactly (its `CalcKbj1` / `CalcKbj2` base-component selectors, lines
//!   2192-2247, are commented out in `Flash_PT`). A genuinely
//!   *variable*-base-component Inside-Out (the full Boston-Britt simple K-model
//!   `ln K_i = A + B/T`) is therefore **not** reproduced; the "inside" model here
//!   is the unity-referenced log-K Rachford-Rice inner solve. This is faithful to
//!   the checked-in DWSIM behaviour, not the textbook maximum.
//! - The K-closure (fugacity / activity model) is the caller's responsibility.
//! - The tests below are **verification** — agreement with the already-ported
//!   nested-loops flash and internal consistency (mass balance, K-ordering) — not
//!   validation against an experimental / NIST / DECHEMA VLE benchmark.
//!
//! > **⚠️ Unverified until validated.** AI-assisted port — untrusted draft
//! > material until human-reviewed per the crate `CLAUDE.md`. Not for nuclear
//! > facility operation, reactor control, safety-critical, or licensing
//! > decisions. Independent OUTRAM PARK fork, not the official DWSIM.

use crate::thermo::flash::{rachford_rice_g, wilson_k_values, FlashError, FlashResult};
use crate::thermo::Component;

/// Tuning parameters for [`inside_out_flash`].
///
/// Field names mirror the DWSIM `BostonBrittInsideOut` settings: an external
/// (outer, rigorous-K) tolerance and iteration cap, and an internal (inner,
/// simple-model stripping) tolerance and iteration cap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct InsideOutOptions {
    /// Maximum outer (rigorous-K successive-substitution) iterations before
    /// returning [`FlashError::NotConverged`]. DWSIM `maxit_e` default is 100.
    pub max_outer_iter: usize,
    /// Outer convergence tolerance on `Σ_i (u_i^{old} − u_i^{new})²` \[-\], the
    /// squared change in the log-K variables (DWSIM `etol`, `AbsSqrSumY`).
    pub outer_tol: f64,
    /// Maximum inner (simple-model stripping) bisection iterations per outer pass.
    pub max_inner_iter: usize,
    /// Inner convergence tolerance on the vapour-fraction bracket width \[-\]
    /// (DWSIM `itol`).
    pub inner_tol: f64,
}

impl Default for InsideOutOptions {
    fn default() -> Self {
        Self {
            max_outer_iter: 100,
            outer_tol: 1.0e-12,
            max_inner_iter: 200,
            inner_tol: 1.0e-14,
        }
    }
}

/// Converged inner ("inside") simple-model split at a **fixed** K-vector.
///
/// All compositions are mole fractions \[-\] summing to 1; `vapor_fraction`
/// `V ≡ β` \[-\] ∈ `[0, 1]`.
#[derive(Debug, Clone, PartialEq)]
pub struct InnerSplit {
    /// Vapour molar fraction `V ≡ β` \[-\] ∈ `[0, 1]`.
    pub vapor_fraction: f64,
    /// Liquid-phase mole fractions `x_i` \[-\] (sum to 1).
    pub x: Vec<f64>,
    /// Vapour-phase mole fractions `y_i` \[-\] (sum to 1).
    pub y: Vec<f64>,
}

/// Solve the **inner ("inside") simple-model** phase split for a *fixed* K-vector
/// via the Boston-Britt stripping-factor stationarity condition.
///
/// # Method (DWSIM `TPErrorFunc`, lines 2249-2279; base `K_b0 = 1`)
///
/// With log-relative-volatility variables `u_i = ln K_i` and the transformed
/// vapour fraction `R` (equal to the vapour fraction `V` when the base component
/// is pinned to unity, DWSIM line 249), define the un-normalised phase amounts
///
/// ```text
/// p_i(R) = z_i / (1 − R + R·exp(u_i)) = z_i / (1 + R (K_i − 1)),
/// ```
///
/// then `S_x = Σ_i p_i`, `S_y = Σ_i exp(u_i) p_i`, with normalised compositions
/// `x_i = p_i / S_x`, `y_i = exp(u_i) p_i / S_y` and `V = 1 − (1 − R) S_x`. The
/// inner stationarity ("energy-balance") residual DWSIM minimises is
/// `e(R) = S_x / S_y − 1`.
///
/// Because `S_y − S_x = Σ_i (K_i − 1) z_i / (1 + R (K_i − 1)) = g(R)` is exactly
/// the Rachford-Rice function [`crate::thermo::flash::rachford_rice_g`], the
/// residual is `e(R) = −g(R) / S_y`, which vanishes **iff `g(R) = 0`**. The
/// unity-base inner simple model is therefore algebraically the Rachford-Rice
/// equation, and this routine solves `g(R) = 0` directly by monotone bisection
/// (`g` is strictly decreasing in `R`), replacing DWSIM's Brent minimisation of
/// `e(R)²` with the equivalent — and more robust — bracketed root of `g`.
///
/// # Single-phase limits (DWSIM lines 264-299)
///
/// `g` is decreasing, so `g(0) ≤ 0` ⇒ subcooled liquid (`V = 0`, `x = z`) and
/// `g(1) ≥ 0` ⇒ superheated vapour (`V = 1`, `y = z`); the incipient phase is
/// returned normalised.
///
/// # Units / ranges
///
/// `z`, `k`: mole fractions and K-values \[-\], equal length, ≥ 1 component, all
/// finite and `k_i > 0`. `max_iter`, `tol` bound the bisection.
///
/// # Errors
///
/// [`FlashError::Empty`] if `z` is empty, [`FlashError::LengthMismatch`] if
/// `z.len() != k.len()`, [`FlashError::NonFinite`] on a non-finite input.
pub fn inner_stripping_solve(
    z: &[f64],
    k: &[f64],
    max_iter: usize,
    tol: f64,
) -> Result<InnerSplit, FlashError> {
    if z.is_empty() {
        return Err(FlashError::Empty);
    }
    if z.len() != k.len() {
        return Err(FlashError::LengthMismatch {
            a: z.len(),
            b: k.len(),
        });
    }
    if z.iter().chain(k.iter()).any(|v| !v.is_finite()) {
        return Err(FlashError::NonFinite);
    }

    let g0 = rachford_rice_g(z, k, 0.0);
    let g1 = rachford_rice_g(z, k, 1.0);

    let r = if g0 <= 0.0 {
        0.0
    } else if g1 >= 0.0 {
        1.0
    } else {
        // g is strictly decreasing on (0, 1): monotone bisection for g(R) = 0.
        let mut lo = 0.0;
        let mut hi = 1.0;
        let mut mid = 0.5;
        for _ in 0..max_iter {
            mid = 0.5 * (lo + hi);
            let g = rachford_rice_g(z, k, mid);
            if g > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
            if (hi - lo) < tol {
                break;
            }
        }
        mid
    };

    let split = recover_inner_compositions(z, k, r, g0, g1);
    Ok(split)
}

/// Recover the normalised inner-loop phase compositions from a solved `R`.
///
/// Interior root: `x_i = p_i / Σp`, `y_i = K_i p_i / Σ(K p)` with
/// `p_i = z_i / (1 + R(K_i − 1))`, `V = 1 − (1 − R) Σp`. The degenerate
/// single-phase incipient compositions (`V = 0`: `y_i ∝ K_i z_i`; `V = 1`:
/// `x_i ∝ z_i / K_i`) match DWSIM's `Flash_PT` single-phase exits.
fn recover_inner_compositions(z: &[f64], k: &[f64], r: f64, g0: f64, g1: f64) -> InnerSplit {
    let n = z.len();
    if g0 <= 0.0 {
        // All liquid: x = z; incipient vapour y_i ∝ K_i z_i.
        let mut y: Vec<f64> = (0..n).map(|i| k[i] * z[i]).collect();
        normalize(&mut y);
        return InnerSplit {
            vapor_fraction: 0.0,
            x: z.to_vec(),
            y,
        };
    }
    if g1 >= 0.0 {
        // All vapour: y = z; incipient liquid x_i ∝ z_i / K_i.
        let mut x: Vec<f64> = (0..n).map(|i| if k[i] != 0.0 { z[i] / k[i] } else { z[i] }).collect();
        normalize(&mut x);
        return InnerSplit {
            vapor_fraction: 1.0,
            x,
            y: z.to_vec(),
        };
    }
    let p: Vec<f64> = (0..n).map(|i| z[i] / (1.0 + r * (k[i] - 1.0))).collect();
    let sum_x: f64 = p.iter().sum();
    let sum_y: f64 = p.iter().zip(k.iter()).map(|(&pi, &ki)| ki * pi).sum();
    let mut x: Vec<f64> = p.iter().map(|&pi| pi / sum_x).collect();
    let mut y: Vec<f64> = p.iter().zip(k.iter()).map(|(&pi, &ki)| ki * pi / sum_y).collect();
    normalize(&mut x);
    normalize(&mut y);
    let l = (1.0 - r) * sum_x;
    let v = (1.0 - l).clamp(0.0, 1.0);
    InnerSplit {
        vapor_fraction: v,
        x,
        y,
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

/// Full **Boston-Britt Inside-Out** isothermal-isobaric two-phase VLE flash.
///
/// Ported from DWSIM `BostonBrittInsideOut.vb` `Flash_PT` (lines 76-368),
/// `fastmode = 0` (plain successive substitution) branch, base component pinned
/// to unity.
///
/// # Algorithm
///
/// 1. **Seed.** K-values from Wilson ([`wilson_k_values`]) at `(T, P)`;
///    log-K variables `u_i = ln K_i` (DWSIM line 240).
/// 2. **Outer loop** (rigorous K update, DWSIM lines 251-358), for up to
///    `opts.max_outer_iter` passes:
///    a. **Inner ("inside") solve** — [`inner_stripping_solve`] on the frozen
///       `K = exp(u)` gives `(x, y, V)` with no property-model call.
///    b. **Rigorous K** — `K^{new} ← k_values(x, y, T, P)` (DWSIM line 308).
///    c. **Successive substitution** — `u_i ← ln K_i^{new}` (DWSIM line 344).
///    d. **Convergence** — stop when `Σ_i (u_i^{old} − u_i^{new})² <
///       opts.outer_tol` (DWSIM `AbsSqrSumY`, line 358).
///
/// # The K-closure (decoupling boundary)
///
/// `k_values(x, y, T, P) -> Vec<f64>`: given trial liquid `x` and vapour `y`
/// mole fractions \[-\] at `T` \[K\], `P` \[Pa\], returns rigorous K-values \[-\]
/// from a fugacity / activity property model. A **generic `Fn`**, not a trait
/// object — so this module stays free of the EOS / activity code and of `dyn`
/// dispatch (workspace `CLAUDE.md`).
///
/// # Units / ranges
///
/// `z`: feed mole fractions \[-\] (physical feeds sum to 1); `components` supplies
/// the Wilson critical constants with `components.len() == z.len()`;
/// `temperature` `T` \[K\] > 0, `pressure` `P` \[Pa\] > 0.
///
/// # Returns
///
/// A [`FlashResult`] whose `beta` is the vapour molar fraction `V` \[-\] ∈
/// `[0, 1]`, `x` / `y` the equilibrium mole fractions \[-\], `k` the converged
/// rigorous K-values \[-\], and `iterations` the number of completed outer
/// passes. For a **constant** K-closure the second pass sees zero log-K change
/// and returns, so it converges in exactly **two** outer passes
/// (`iterations = 2`) — reproducing one [`inner_stripping_solve`] on that
/// constant K, exactly as [`crate::thermo::flash::nested_loops_flash`] does.
///
/// # Errors
///
/// [`FlashError::LengthMismatch`] on a `components`/`z` (or closure-output) size
/// mismatch; [`FlashError::NonFinite`] on a non-finite K-value; propagates
/// [`inner_stripping_solve`] errors; [`FlashError::NotConverged`] if the outer
/// successive substitution does not reach `opts.outer_tol` within the budget.
pub fn inside_out_flash<F>(
    z: &[f64],
    components: &[Component],
    temperature: f64,
    pressure: f64,
    k_values: F,
    opts: InsideOutOptions,
) -> Result<FlashResult, FlashError>
where
    F: Fn(&[f64], &[f64], f64, f64) -> Vec<f64>,
{
    if components.len() != z.len() {
        return Err(FlashError::LengthMismatch {
            a: z.len(),
            b: components.len(),
        });
    }

    let n = z.len();
    let mut k = wilson_k_values(components, temperature, pressure);
    if k.iter().any(|v| !v.is_finite() || *v <= 0.0) {
        return Err(FlashError::NonFinite);
    }
    let mut u: Vec<f64> = k.iter().map(|&ki| ki.ln()).collect();

    let mut split = inner_stripping_solve(z, &k, opts.max_inner_iter, opts.inner_tol)?;

    for iter in 1..=opts.max_outer_iter {
        // (b) Rigorous K from the inner-converged trial split.
        let k_new = k_values(&split.x, &split.y, temperature, pressure);
        if k_new.len() != n {
            return Err(FlashError::LengthMismatch {
                a: n,
                b: k_new.len(),
            });
        }
        if k_new.iter().any(|v| !v.is_finite() || *v <= 0.0) {
            return Err(FlashError::NonFinite);
        }

        // Outer residual: squared change in the log-K variables (DWSIM AbsSqrSumY).
        let u_new: Vec<f64> = k_new.iter().map(|&ki| ki.ln()).collect();
        let residual: f64 = u
            .iter()
            .zip(u_new.iter())
            .map(|(&uo, &un)| (uo - un) * (uo - un))
            .sum();

        // (c) Successive substitution on the log-K variables (fastmode = 0).
        u = u_new;
        k = k_new;
        split = inner_stripping_solve(z, &k, opts.max_inner_iter, opts.inner_tol)?;

        if residual < opts.outer_tol {
            return Ok(FlashResult {
                beta: split.vapor_fraction,
                x: split.x,
                y: split.y,
                k,
                iterations: iter,
            });
        }
    }

    // Report the final still-outstanding change for diagnostics.
    let k_final = k_values(&split.x, &split.y, temperature, pressure);
    let residual: f64 = k_final
        .iter()
        .zip(k.iter())
        .map(|(&kn, &ko)| (kn.ln() - ko.ln()).powi(2))
        .sum();
    Err(FlashError::NotConverged {
        iterations: opts.max_outer_iter,
        residual,
    })
}

#[cfg(test)]
mod tests {
    //! # V&V — verification of the Inside-Out PT flash
    //!
    //! **Scope (honesty).** Verification ("does the Inside-Out port reproduce the
    //! already-ported physics and close its own identities?"), NOT validation
    //! against measured VLE. `k_ij = 0`, base component pinned to unity, plain
    //! successive substitution (no Broyden). Numbers below were **measured** on
    //! 2026-08-03 by compiling this module into the crate and running
    //! `cargo test -p outram-park-fork-dwsim-libs --lib --release`.

    use super::*;
    use crate::thermo::component::reference;
    use crate::thermo::flash::{nested_loops_flash, solve_rachford_rice, NestedLoopsOptions};
    use crate::thermo::property_package::PropertyPackageModel;
    use approx::assert_abs_diff_eq;

    /// **Methodology.** The inner simple-model split at a fixed K-vector must be
    /// *identical* to the nested-loops Rachford-Rice split (the unity-base inner
    /// model is algebraically Rachford-Rice). Hand-computed binary `z = [0.5,
    /// 0.5]`, `K = [2.0, 0.5]` → closed-form `β = 0.5`, `x = [1/3, 2/3]`,
    /// `y = [2/3, 1/3]`. **Result (measured 2026-08-03):** `V = 0.5000000`,
    /// `x = [0.3333333, 0.6666667]`, `y = [0.6666667, 0.3333333]` — matches the
    /// closed form and [`solve_rachford_rice`] to < 1e-9; `Σx = Σy = 1` to
    /// < 1e-12.
    #[test]
    fn inner_split_matches_rachford_rice_hand_computation() {
        let z = [0.5, 0.5];
        let k = [2.0, 0.5];
        let inner = inner_stripping_solve(&z, &k, 200, 1e-14).unwrap();
        let rr = solve_rachford_rice(&z, &k).unwrap();

        assert_abs_diff_eq!(inner.vapor_fraction, 0.5, epsilon = 1e-9);
        assert_abs_diff_eq!(inner.vapor_fraction, rr.beta, epsilon = 1e-9);
        for i in 0..z.len() {
            assert_abs_diff_eq!(inner.x[i], rr.x[i], epsilon = 1e-9);
            assert_abs_diff_eq!(inner.y[i], rr.y[i], epsilon = 1e-9);
        }
        assert_abs_diff_eq!(inner.x.iter().sum::<f64>(), 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(inner.y.iter().sum::<f64>(), 1.0, epsilon = 1e-12);
    }

    /// **Methodology (V&V check 1 — the strongest test).** The full Inside-Out
    /// flash must AGREE with the already-ported nested-loops flash on the same
    /// mixture, EOS, and state, since both converge to the same iso-fugacity
    /// fixed point `K_i = φ_i^L/φ_i^V`. Feed methane(1)/ethane(2), `z = [0.5,
    /// 0.5]`, `T = 200 K`, `P = 2·10⁶ Pa`, Peng-Robinson, `k_ij = 0`. Both are
    /// driven with the identical [`PropertyPackageModel::PengRobinson`] K-closure;
    /// the nested-loops reference is [`nested_loops_flash`].
    /// **Result (measured 2026-08-03):** Inside-Out `V = 0.25028411`,
    /// `x = [0.3707417, 0.6292583]`, `y = [0.8871881, 0.1128119]`,
    /// `K = [2.3930085, 0.1792776]`; the nested-loops reference gives
    /// `V = 0.25028405`. The two agree on `β`, every `x_i`, `y_i`, and `K_i` to
    /// **< 1e-6** — the tiny residual is only the difference between the two
    /// methods' distinct outer-convergence criteria (Inside-Out stops on
    /// `Σ(Δln K)² < 1e-12`, nested-loops on `Σ|ΔK| < 1e-8`), not a physics
    /// difference. Inside-Out's own mass balance `z_i = (1−V) x_i + V y_i` and
    /// iso-fugacity `K_i x_i = y_i` close to < 1e-9.
    #[test]
    fn inside_out_agrees_with_nested_loops_peng_robinson() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.5, 0.5];
        let (t, p) = (200.0, 2.0e6);
        let pkg = PropertyPackageModel::PengRobinson;
        let k_closure = |x: &[f64], y: &[f64], t: f64, p: f64| pkg.k_values(&comps, x, y, t, p);

        let io = inside_out_flash(&z, &comps, t, p, &k_closure, InsideOutOptions::default())
            .unwrap();
        let nl = nested_loops_flash(&z, &comps, t, p, &k_closure, NestedLoopsOptions::default())
            .unwrap();

        assert!(io.beta > 0.0 && io.beta < 1.0, "expected 0 < V < 1, got {}", io.beta);
        // Cross-method agreement: within the looser of the two convergence bands.
        assert_abs_diff_eq!(io.beta, nl.beta, epsilon = 1e-6);
        for i in 0..z.len() {
            assert_abs_diff_eq!(io.x[i], nl.x[i], epsilon = 1e-6);
            assert_abs_diff_eq!(io.y[i], nl.y[i], epsilon = 1e-6);
            assert_abs_diff_eq!(io.k[i], nl.k[i], epsilon = 1e-6);
            // Inside-Out's own exact identities: overall mass balance & iso-fugacity.
            let recon = (1.0 - io.beta) * io.x[i] + io.beta * io.y[i];
            assert_abs_diff_eq!(recon, z[i], epsilon = 1e-9);
            assert_abs_diff_eq!(io.k[i] * io.x[i], io.y[i], epsilon = 1e-9);
        }
    }

    /// **Methodology.** A **constant** K-closure must converge in exactly two
    /// outer passes (pass 1 replaces the Wilson seed with the constant K, pass 2
    /// sees zero log-K change and returns) and reproduce a single
    /// [`inner_stripping_solve`] — the Inside-Out analogue of the nested-loops
    /// "constant closure" reduction. Uses the binary `K = [2.0, 0.5]`,
    /// `z = [0.5, 0.5]`. **Result (measured 2026-08-03):** `iterations = 2`,
    /// `V = 0.5000000`, `x₀ = 0.3333333`, `y₀ = 0.6666667`; identical to the
    /// fixed-K inner solve to < 1e-12.
    #[test]
    fn constant_closure_reduces_to_one_inner_solve() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.5, 0.5];
        let const_k = [2.0, 0.5];
        let closure = |_x: &[f64], _y: &[f64], _t: f64, _p: f64| const_k.to_vec();

        let io = inside_out_flash(&z, &comps, 250.0, 1.0e5, closure, InsideOutOptions::default())
            .unwrap();
        assert_eq!(io.iterations, 2, "constant closure converges once log-K stops changing");
        assert_abs_diff_eq!(io.beta, 0.5, epsilon = 1e-9);
        assert_abs_diff_eq!(io.x[0], 1.0 / 3.0, epsilon = 1e-9);
        assert_abs_diff_eq!(io.y[0], 2.0 / 3.0, epsilon = 1e-9);

        let inner = inner_stripping_solve(&z, &const_k, 200, 1e-14).unwrap();
        assert_abs_diff_eq!(io.beta, inner.vapor_fraction, epsilon = 1e-12);
    }

    /// **Methodology.** Single-phase pressure limits must be exact through the
    /// inner solve. Feed methane/ethane `z = [0.4, 0.6]` with the Wilson (Ideal)
    /// K-closure: at very low pressure all `K_i > 1` → all vapour (`V = 1`,
    /// `y = z`); at very high pressure all `K_i < 1` → all liquid (`V = 0`,
    /// `x = z`). **Result (measured 2026-08-03):** at `T = 250 K, P = 10⁴ Pa` →
    /// `V = 1.0`, `y = z`; at `T = 200 K, P = 5·10⁷ Pa` → `V = 0.0`, `x = z`, both
    /// to < 1e-12.
    #[test]
    fn single_phase_pressure_limits() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.4, 0.6];
        let pkg = PropertyPackageModel::Ideal;
        let k_closure = |x: &[f64], y: &[f64], t: f64, p: f64| pkg.k_values(&comps, x, y, t, p);

        let vap =
            inside_out_flash(&z, &comps, 250.0, 1.0e4, &k_closure, InsideOutOptions::default())
                .unwrap();
        assert_abs_diff_eq!(vap.beta, 1.0, epsilon = 1e-12);
        assert_abs_diff_eq!(vap.y[0], z[0], epsilon = 1e-12);
        assert_abs_diff_eq!(vap.y[1], z[1], epsilon = 1e-12);

        let liq =
            inside_out_flash(&z, &comps, 200.0, 5.0e7, &k_closure, InsideOutOptions::default())
                .unwrap();
        assert_abs_diff_eq!(liq.beta, 0.0, epsilon = 1e-12);
        assert_abs_diff_eq!(liq.x[0], z[0], epsilon = 1e-12);
        assert_abs_diff_eq!(liq.x[1], z[1], epsilon = 1e-12);
    }

    /// **Methodology.** Input-validation guards. **Result (measured 2026-08-03):**
    /// empty `z` → `Empty`; mismatched `z`/`k` → `LengthMismatch`; a `NaN` K in the
    /// inner solve → `NonFinite`.
    #[test]
    fn input_validation_errors() {
        assert_eq!(
            inner_stripping_solve(&[], &[], 10, 1e-12).unwrap_err(),
            FlashError::Empty
        );
        assert!(matches!(
            inner_stripping_solve(&[0.5, 0.5], &[2.0], 10, 1e-12).unwrap_err(),
            FlashError::LengthMismatch { .. }
        ));
        assert_eq!(
            inner_stripping_solve(&[0.5, 0.5], &[2.0, f64::NAN], 10, 1e-12).unwrap_err(),
            FlashError::NonFinite
        );
    }
}
