//! Phase-stability analysis via the **tangent-plane distance** (TPD) criterion —
//! Michelsen's stability test for robust flash initialisation and single-/two-
//! phase identification.
//!
//! Composed on the DWSIM (GPL-3.0) thermo kernel: the fugacity coefficients come
//! from [`crate::thermo::cubic_eos::CubicEos::ln_phi`] and the trial-phase seeds
//! from [`crate::thermo::flash::wilson_k_values`]. DWSIM applies exactly this
//! test inside its `NestedLoops` flash (the `StabTest` phase-stability routine on
//! `PropertyPackage`) to decide whether a candidate single phase should be split
//! and to reject the *trivial* flash solution (both phases converging back to the
//! feed). This module is the Rust analogue of that check.
//!
//! # The criterion (Michelsen, 1982)
//!
//! At a feed of overall mole fractions `z_i` \[-\] and a fixed temperature `T`
//! \[K\] and pressure `P` \[Pa\], the single feed phase is **stable** iff the
//! tangent-plane distance
//!
//! ```text
//! tm(w) = Σ_i w_i ( ln w_i + ln φ_i(w) − ln z_i − ln φ_i(z) ) ≥ 0
//! ```
//!
//! for **every** trial composition `w` (`Σ w_i = 1`). `tm(w)` is the vertical gap
//! between the molar Gibbs-energy-of-mixing surface at `w` and the tangent
//! hyperplane drawn at the feed `z`; a `w` with `tm(w) < 0` sits **below** that
//! tangent plane, i.e. a distinct phase of composition `w` has lower Gibbs energy
//! than the feed, so the feed is **unstable** (it will split).
//!
//! # How stationary points are found (Michelsen's modified formulation)
//!
//! Rather than minimise `tm` over the composition simplex globally, Michelsen
//! locates its **stationary points** by successive substitution on unnormalised
//! trial mole numbers `Y_i`. Writing `d_i = ln z_i + ln φ_i(z)`, a stationary
//! point of the modified function
//!
//! ```text
//! tm*(Y) = 1 + Σ_i Y_i ( ln Y_i + ln φ_i(Y) − d_i − 1 )
//! ```
//!
//! satisfies `ln Y_i = d_i − ln φ_i(w)`, with `w_i = Y_i / Σ_k Y_k`. That fixed
//! point is reached by the iteration
//!
//! ```text
//! Y_i^(k+1) = exp( d_i − ln φ_i(w^(k)) ),   w^(k) = Y^(k) / Σ_j Y_j^(k).
//! ```
//!
//! At convergence, with `S = Σ_i Y_i`, the tangent-plane distance at the
//! stationary composition reduces to `tm(w) = −ln S`: therefore `S > 1` (i.e.
//! `tm < 0`) flags instability. This module reports the **normalised** `tm(w)`
//! (via [`tangent_plane_distance`]) at each converged stationary point, which
//! carries the same sign as `tm*` and `−ln S`.
//!
//! Two Wilson-based seeds are launched — a **vapour-like** trial `Y_i = z_i K_i`
//! and a **liquid-like** trial `Y_i = z_i / K_i` — so that a split is detected
//! whether the incipient phase is lighter or heavier than the feed.
//!
//! # Which cubic-EOS root feeds each `ln φ`
//!
//! [`CubicEos::ln_phi`] needs an explicit [`Phase`] to pick the compressibility
//! root. Stability analysis must use the **thermodynamically correct** root at
//! each composition, i.e. the one of lower Gibbs energy, not a fixed phase label.
//! At fixed `(w, T, P)` only the `Σ_i w_i ln φ_i(w)` part of `G/RT` depends on the
//! root, so the helper selects, for both the feed and every trial, whichever of
//! the vapour/liquid root minimises `Σ_i w_i ln φ_i(w)` (falling back to the sole
//! real root when only one exists). This is documented so the omission of a
//! separate Gibbs-root pre-selection API is explicit.
//!
//! # Honest scope (verification, not benchmark validation)
//!
//! - **Two Wilson-seeded successive-substitution trials only.** This is *not* a
//!   full global minimisation of the TPD surface: successive substitution finds a
//!   *stationary point* near each seed, not a certified global minimum. A phase
//!   reported stable here is stable *with respect to the two Wilson trials* — the
//!   standard, practical Michelsen check DWSIM itself uses, but it can in
//!   principle miss a stationary point that neither Wilson seed reaches (e.g. some
//!   near-critical or strongly non-ideal multicomponent cases). Acceleration
//!   (GDEM/DEM), a third "ideal-mix" seed, and second-order/global TPD
//!   minimisation are out of scope.
//! - **VLE fugacity from the cubic EOS only** ([`CubicEos`], `k_ij = 0`,
//!   geometric-mean mixing). No activity-coefficient / γ-φ stability, no
//!   three-phase (VLLE) or solid stability.
//! - The tests below are **verification** against closed-form identities
//!   (`tm(z) = 0`), single-phase sanity, and internal sign-consistency — **not**
//!   validation against an experimental or NIST/DECHEMA phase-envelope benchmark.
//!
//! > **⚠️ Unverified until validated.** AI-assisted port — untrusted draft
//! > material until human-reviewed per the crate `CLAUDE.md`. Not for nuclear
//! > facility operation, reactor control, safety-critical, or licensing
//! > decisions. Independent OUTRAM PARK fork, not the official DWSIM.
//!
//! # Design (crate `CLAUDE.md`)
//!
//! The fugacity model is taken as the [`CubicEos`] **enum** (no trait object, no
//! `dyn`, no `Box`, no lifetimes, no channels). Compositions are owned by value
//! (`Vec<f64>` / `&[f64]`). Inner arithmetic is documented raw `f64` (SI:
//! K, Pa, mole fractions \[-\]), matching [`crate::thermo::cubic_eos`].

use crate::thermo::cubic_eos::{CubicEos, Phase};
use crate::thermo::flash::wilson_k_values;
use crate::thermo::Component;

/// Maximum successive-substitution iterations per trial before it is abandoned
/// as non-converged (that trial contributes no stationary point).
const MAX_SS_ITER: usize = 2000;

/// Convergence tolerance on the successive-substitution step, measured as the
/// sum of squared changes in `ln Y_i` between iterations \[-\].
const SS_TOL: f64 = 1.0e-12;

/// Trivial-solution guard: a converged trial whose composition `w` differs from
/// the feed `z` by less than this (in `Σ_i (w_i − z_i)²` \[-\]) is treated as the
/// **trivial** solution `w = z` and discarded — it is the feed re-found, not a
/// distinct phase, and its `tm` is ~0 by construction.
const TRIVIAL_TOL: f64 = 1.0e-6;

/// Instability threshold: a converged, non-trivial trial is judged to lower the
/// Gibbs energy (⇒ the feed is unstable) only when its tangent-plane distance
/// falls below `−STABILITY_TOL` \[-\]. The small negative band absorbs
/// successive-substitution round-off so a stationary point that is numerically
/// on the tangent plane (`tm ≈ 0`) is not mis-flagged as unstable.
const STABILITY_TOL: f64 = 1.0e-8;

/// Outcome of a [`stability_test`].
///
/// `tm_min` is the smallest tangent-plane distance \[-\] found over the
/// non-trivial converged trials (or `0.0` if every trial converged to the trivial
/// feed solution). `stable` is `true` iff no non-trivial trial dipped below the
/// tangent plane (`tm_min ≥ −STABILITY_TOL`). When `stable` is `false`,
/// `trial_composition` carries the destabilising trial composition `w`
/// (mole fractions \[-\], summing to 1) — a good warm-start for a two-phase
/// flash; it is `None` when the feed is stable.
#[derive(Debug, Clone, PartialEq)]
pub struct StabilityResult {
    /// `true` iff the single feed phase is stable with respect to both Wilson
    /// trials (no non-trivial trial found `tm < −STABILITY_TOL`).
    pub stable: bool,
    /// The destabilising trial composition `w` (mole fractions \[-\], sum 1) at
    /// the most-negative `tm`, or `None` when the feed is stable.
    pub trial_composition: Option<Vec<f64>>,
    /// Minimum tangent-plane distance `tm` \[-\] over the non-trivial converged
    /// trials; `0.0` if both trials converged to the trivial (feed) solution.
    pub tm_min: f64,
}

/// Fugacity coefficients `ln φ_i` \[-\] at composition `w`, `T` \[K\], `P` \[Pa\]
/// using the **Gibbs-selected** cubic-EOS root.
///
/// Evaluates [`CubicEos::ln_phi`] for both the vapour and liquid roots and
/// returns whichever minimises `Σ_i w_i ln φ_i(w)` (the only root-dependent part
/// of the molar Gibbs energy at fixed composition). Falls back to the sole real
/// root when the cubic yields only one, and to an ideal-gas `0` vector in the
/// (non-physical) event that neither root is available.
fn ln_phi_min_gibbs(eos: CubicEos, comps: &[Component], w: &[f64], t: f64, p: f64) -> Vec<f64> {
    let vap = eos.ln_phi(comps, w, t, p, Phase::Vapor, None);
    let liq = eos.ln_phi(comps, w, t, p, Phase::Liquid, None);
    let reduced_g =
        |ln_phi: &[f64]| -> f64 { w.iter().zip(ln_phi).map(|(&wi, &lp)| wi * lp).sum() };
    match (vap, liq) {
        (Some(v), Some(l)) => {
            if reduced_g(&l) < reduced_g(&v) {
                l
            } else {
                v
            }
        }
        (Some(v), None) => v,
        (None, Some(l)) => l,
        (None, None) => vec![0.0; comps.len()],
    }
}

/// Tangent-plane distance `tm(w) = Σ_i w_i ( ln w_i + ln φ_i(w) − ln z_i − ln φ_i(z) )`
/// \[-\] of a trial composition `w` relative to the feed `z`.
///
/// This is the vertical gap between the reduced molar Gibbs-energy-of-mixing
/// surface at `w` and the tangent hyperplane at `z` (both at the same `T`, `P`).
/// `tm(w) ≥ 0` for all `w` ⟺ the feed phase is stable; any `tm(w) < 0` means a
/// distinct phase of composition `w` is more stable than the feed.
///
/// # Units / ranges
///
/// - `components`, `z`, `w`: equal length; `z`, `w` are mole fractions \[-\] that
///   should each sum to 1, with `z_i > 0` (the feed must contain every component,
///   since `ln z_i` appears). Components of `w` equal to 0 contribute a zero term
///   (`w_i ln w_i → 0`) and are skipped.
/// - `t` \[K\] > 0, `p` \[Pa\] > 0.
/// - `eos`: the [`CubicEos`] fugacity model (`k_ij = 0`); both `ln φ(z)` and
///   `ln φ(w)` use the Gibbs-selected root (see [`ln_phi_min_gibbs`]).
///
/// # Special values
///
/// `tangent_plane_distance(.., z, z, ..)` is `0.0` **exactly** — the trial equals
/// the feed, every bracket cancels term-by-term. This is the trivial solution.
///
/// # Returns
///
/// The scalar `tm` \[-\]. Dimensionless (it is a Gibbs energy divided by `RT`).
#[must_use]
pub fn tangent_plane_distance(
    components: &[Component],
    z: &[f64],
    w: &[f64],
    t: f64,
    p: f64,
    eos: CubicEos,
) -> f64 {
    let ln_phi_z = ln_phi_min_gibbs(eos, components, z, t, p);
    let ln_phi_w = ln_phi_min_gibbs(eos, components, w, t, p);
    let mut tm = 0.0;
    for i in 0..components.len() {
        if w[i] > 0.0 {
            tm += w[i] * (w[i].ln() + ln_phi_w[i] - z[i].ln() - ln_phi_z[i]);
        }
    }
    tm
}

/// Run one successive-substitution trial from an initial mole-number seed and
/// return the **normalised** converged stationary composition `w` (mole fractions
/// \[-\]), or `None` if it did not converge within [`MAX_SS_ITER`] or the trial
/// mole numbers collapsed to a non-positive/non-finite sum.
///
/// Iteration: `ln Y_i ← d_i − ln φ_i(w)` with `w = Y / Σ Y`, stopping when the
/// sum of squared `ln Y` changes falls below [`SS_TOL`]. `d_i = ln z_i + ln φ_i(z)`
/// is supplied by the caller (constant across trials).
fn run_trial(
    eos: CubicEos,
    comps: &[Component],
    d: &[f64],
    seed: &[f64],
    t: f64,
    p: f64,
) -> Option<Vec<f64>> {
    let n = comps.len();
    let mut y = seed.to_vec();
    let mut ln_y: Vec<f64> = y.iter().map(|&yi| yi.ln()).collect();

    for _ in 0..MAX_SS_ITER {
        let s: f64 = y.iter().sum();
        if !s.is_finite() || s <= 0.0 {
            return None;
        }
        let w: Vec<f64> = y.iter().map(|&yi| yi / s).collect();
        let ln_phi_w = ln_phi_min_gibbs(eos, comps, &w, t, p);

        let mut err = 0.0;
        for i in 0..n {
            let new_ln_yi = d[i] - ln_phi_w[i];
            let dl = new_ln_yi - ln_y[i];
            err += dl * dl;
            ln_y[i] = new_ln_yi;
            y[i] = new_ln_yi.exp();
        }

        if err < SS_TOL {
            let s2: f64 = y.iter().sum();
            if !s2.is_finite() || s2 <= 0.0 {
                return None;
            }
            return Some(y.iter().map(|&yi| yi / s2).collect());
        }
    }
    None
}

/// Michelsen phase-stability test: is the single feed phase `z` at `(T, P)`
/// stable, or will it split?
///
/// Launches two Wilson-seeded successive-substitution trials — vapour-like
/// (`Y_i = z_i K_i`) and liquid-like (`Y_i = z_i / K_i`) — to locate stationary
/// points of the tangent-plane-distance surface. The feed is declared
/// **unstable** if either trial converges to a **non-trivial** composition whose
/// tangent-plane distance is negative (`tm < −STABILITY_TOL`): such a `w` is a
/// composition of lower Gibbs energy than the feed.
///
/// # Trivial-solution guard
///
/// A trial that drifts back to the feed (`Σ_i (w_i − z_i)² < TRIVIAL_TOL`) is the
/// *trivial* stationary point `w = z` (`tm = 0` by construction) and is
/// **discarded** — it carries no information about a phase split. This is exactly
/// why DWSIM runs the test with two opposed seeds: at least one must leave the
/// feed for an instability to be found.
///
/// # Convergence criteria
///
/// Each trial iterates `ln Y_i ← d_i − ln φ_i(w)` (with `w = Y/ΣY`) until the sum
/// of squared `ln Y` changes drops below [`SS_TOL`] (`1e-12`), for at most
/// [`MAX_SS_ITER`] (`2000`) iterations; a trial that fails to converge in that
/// budget is dropped (contributes no stationary point). Successive substitution
/// can be slow very near a phase boundary or critical point — see the honest
/// scope in the module header.
///
/// # Units / ranges
///
/// - `components`, `z`: equal length; `z` mole fractions \[-\] summing to 1 with
///   every `z_i > 0`. `t` \[K\] > 0, `p` \[Pa\] > 0. `eos`: the [`CubicEos`]
///   fugacity model (`k_ij = 0`).
///
/// # Returns
///
/// A [`StabilityResult`]: `stable`, the minimum non-trivial `tm` found
/// (`tm_min`), and — when unstable — the destabilising trial composition to
/// warm-start a flash.
#[must_use]
pub fn stability_test(
    components: &[Component],
    z: &[f64],
    t: f64,
    p: f64,
    eos: CubicEos,
) -> StabilityResult {
    let n = components.len();
    let ln_phi_z = ln_phi_min_gibbs(eos, components, z, t, p);
    let d: Vec<f64> = (0..n).map(|i| z[i].ln() + ln_phi_z[i]).collect();

    let k = wilson_k_values(components, t, p);
    let vapor_seed: Vec<f64> = (0..n).map(|i| z[i] * k[i]).collect();
    let liquid_seed: Vec<f64> = (0..n).map(|i| z[i] / k[i]).collect();

    let mut tm_min = f64::INFINITY;
    let mut best_trial: Option<Vec<f64>> = None;

    for seed in [vapor_seed, liquid_seed] {
        let Some(w) = run_trial(eos, components, &d, &seed, t, p) else {
            continue;
        };
        // Trivial-solution guard: converged back to the feed.
        let dist: f64 = (0..n).map(|i| (w[i] - z[i]).powi(2)).sum();
        if dist < TRIVIAL_TOL {
            continue;
        }
        let tpd = tangent_plane_distance(components, z, &w, t, p, eos);
        if tpd < tm_min {
            tm_min = tpd;
            best_trial = Some(w);
        }
    }

    if !tm_min.is_finite() {
        // Every trial was trivial (or non-converging): no distinct phase found.
        tm_min = 0.0;
    }

    let stable = tm_min >= -STABILITY_TOL;
    StabilityResult {
        stable,
        trial_composition: if stable { None } else { best_trial },
        tm_min,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermo::component::reference;
    use approx::assert_abs_diff_eq;

    /// **Methodology.** The trivial solution `w = z` must give `tm = 0` exactly
    /// (every bracket cancels term-by-term, independent of the fugacity model).
    /// Evaluated for a methane/ethane feed `z = [0.5, 0.5]` at `T = 250 K`,
    /// `P = 20 bar`, Peng-Robinson.
    /// **Results (2026-07-24):** `tangent_plane_distance(z, z) = 0.0` to bit
    /// exactness (`|tm| = 0` ≤ 1e-300).
    #[test]
    fn trivial_solution_is_zero_exactly() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.5, 0.5];
        let tm = tangent_plane_distance(&comps, &z, &z, 250.0, 20.0e5, CubicEos::PengRobinson);
        assert_eq!(tm, 0.0, "tm(z,z) must be exactly zero, got {tm}");
    }

    /// **Methodology.** A single component can only ever re-find itself: every
    /// trial composition normalises to `w = [1.0] = z`, so the test must report
    /// **stable** with `tm_min = 0`. Pure CO₂ at `T = 350 K` (supercritical,
    /// Tc = 304.12 K), `P = 10 bar`, Peng-Robinson.
    /// **Results (2026-07-24):** `stable = true`, `tm_min = 0.0`,
    /// `trial_composition = None` (both Wilson trials collapse to the trivial
    /// feed solution and are discarded).
    #[test]
    fn single_component_is_stable() {
        let comps = [reference::carbon_dioxide()];
        let z = [1.0];
        let r = stability_test(&comps, &z, 350.0, 10.0e5, CubicEos::PengRobinson);
        assert!(r.stable, "pure CO₂ must be stable, got {r:?}");
        assert_abs_diff_eq!(r.tm_min, 0.0, epsilon = 1e-9);
        assert!(r.trial_composition.is_none());
    }

    /// **Methodology.** A binary well above both critical temperatures is a
    /// single supercritical gas — clearly single-phase, so the stability test
    /// must return **stable** with `tm_min ≥ 0`. Methane/ethane `z = [0.5, 0.5]`
    /// at `T = 350 K` (> ethane Tc = 305.32 K and methane Tc = 190.56 K),
    /// `P = 5 bar`, Peng-Robinson. Both Wilson trials should either converge back
    /// to the feed (trivial, discarded) or find only `tm ≥ 0`.
    /// **Results (2026-07-24):** `stable = true`, `tm_min = 0.0`
    /// (both trials trivial), `trial_composition = None`.
    #[test]
    fn supercritical_binary_gas_is_stable() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.5, 0.5];
        let r = stability_test(&comps, &z, 350.0, 5.0e5, CubicEos::PengRobinson);
        assert!(
            r.stable,
            "supercritical CH₄/C₂H₆ gas must be stable, got {r:?}"
        );
        assert!(
            r.tm_min >= -1e-8,
            "tm_min must be ≥ 0 for a stable phase, got {}",
            r.tm_min
        );
    }

    /// **Methodology.** A methane/ethane feed held deep inside its two-phase
    /// envelope must be detected as **unstable**: at least one Wilson trial
    /// converges to a non-trivial composition with `tm < 0`. Feed
    /// `z = [0.5, 0.5]` at `T = 200 K`, `P = 10 bar` — well below ethane's
    /// Tc = 305.32 K, so ethane is a strong liquid-former while methane
    /// (Tc = 190.56 K) is the light, volatile partner; the mixture splits into a
    /// methane-rich vapour and an ethane-rich liquid. Peng-Robinson.
    /// **Results (2026-07-24):** `stable = false`, `tm_min = -0.66325` (< 0), and
    /// the destabilising trial is ethane-enriched relative to the feed
    /// (`w = [0.05286, 0.94714]`, so `w₁ = 0.947 > 0.5`), a physically sensible
    /// incipient ethane-rich liquid. The trial composition is a valid
    /// two-phase-flash warm start.
    #[test]
    fn two_phase_binary_is_unstable() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.5, 0.5];
        let r = stability_test(&comps, &z, 200.0, 10.0e5, CubicEos::PengRobinson);
        assert!(
            !r.stable,
            "CH₄/C₂H₆ at 200 K / 10 bar must be unstable, got {r:?}"
        );
        assert!(
            r.tm_min < 0.0,
            "unstable feed must have tm_min < 0, got {}",
            r.tm_min
        );
        assert_abs_diff_eq!(r.tm_min, -0.66325, epsilon = 1e-4);
        let w = r.trial_composition.expect("unstable ⇒ a trial composition");
        assert_abs_diff_eq!(w.iter().sum::<f64>(), 1.0, epsilon = 1e-9);
        // The destabilising trial is a distinct (non-trivial) phase.
        let dist: f64 = (0..2).map(|i| (w[i] - z[i]).powi(2)).sum();
        assert!(
            dist > 1e-3,
            "trial must be non-trivial (≠ feed), got w = {w:?}"
        );
        // Incipient phase is ethane-enriched (heavier than the feed).
        assert!(
            w[1] > z[1],
            "expected an ethane-rich incipient phase, got w = {w:?}"
        );
    }

    /// **Methodology.** Sign/consistency check tying [`tangent_plane_distance`] to
    /// [`stability_test`]: for the unstable 200 K / 10 bar CH₄/C₂H₆ feed above,
    /// re-evaluating `tangent_plane_distance` at the returned destabilising trial
    /// must reproduce `tm_min` (the reported minimum is that trial's TPD), and it
    /// must be negative.
    /// **Results (2026-07-24):** recomputed `tm(w*) = -0.66325` matches
    /// `tm_min = -0.66325` to < 1e-9 and is negative.
    #[test]
    fn reported_tm_min_matches_recomputed_tpd_at_trial() {
        let comps = [reference::methane(), reference::ethane()];
        let z = [0.5, 0.5];
        let eos = CubicEos::PengRobinson;
        let r = stability_test(&comps, &z, 200.0, 10.0e5, eos);
        let w = r
            .trial_composition
            .clone()
            .expect("unstable ⇒ a trial composition");
        let tm = tangent_plane_distance(&comps, &z, &w, 200.0, 10.0e5, eos);
        assert_abs_diff_eq!(tm, r.tm_min, epsilon = 1e-9);
        assert!(tm < 0.0);
    }
}
