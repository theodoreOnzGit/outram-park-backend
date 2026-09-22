// SPDX-License-Identifier: GPL-3.0

//! **Higher-order depletion integrators and transfer rates** — GitHub #266
//! scope items 2, 3 and 4.
//!
//! Ported in structure from `openmc/deplete/integrators.py` and
//! `openmc/deplete/transfer_rates.py` at OpenMC `afa7a14`.
//!
//! # Why the order matters, and what #266 already measured
//!
//! The predictor integrator this crate had is **first order** in the step
//! size. #266's scope item 1 measured that convergence on the existing
//! operator, and the finding was sharper than "the error is small": **Xe-135
//! is not in the asymptotic regime at practical step sizes at all.** Its
//! Richardson-observed order came out **negative (−3.70) at 5-day steps** and
//! only climbed through 1 once the step fell below its ~9.14 h half-life.
//!
//! That is the context these integrators land in. A fourth-order method is
//! fourth order *in its asymptotic regime*; on a nuclide whose own time
//! constant is shorter than the step, it is not obviously better than first
//! order, and [`Integrator::observed_order`] is provided so that claim is
//! measured per problem rather than assumed from the method's name.
//!
//! # What each method costs
//!
//! | method | transport solves per step | order |
//! |---|---|---|
//! | [`Integrator::Predictor`] | 1 | 1 |
//! | [`Integrator::CeCm`] | 2 | 2 |
//! | [`Integrator::Cf4`] | 4 | 4 |
//!
//! "Transport solves" is the expensive part — each one is a full eigenvalue
//! calculation in a coupled run. CF4 is four times the cost of predictor per
//! step, so it only pays if it lets the step grow by more than 4x.

use crate::depletion::cram::cram16;
use crate::depletion::matrix::DepletionMatrix;

/// Seconds in a day, matching `operator.rs`.
pub const SECONDS_PER_DAY: f64 = 86_400.0;

/// Which time integrator to advance the inventory with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Integrator {
    /// Explicit predictor — `PredictorIntegrator`. First order, one solve.
    Predictor,
    /// Constant-extrapolation / constant-midpoint — `CECMIntegrator`.
    /// Second order, two solves.
    CeCm,
    /// Commutator-free fourth order — `CF4Integrator`. Upstream's usual
    /// production default. Four solves.
    Cf4,
}

impl Integrator {
    /// Transport solves this method needs per step.
    pub fn solves_per_step(self) -> usize {
        match self {
            Integrator::Predictor => 1,
            Integrator::CeCm => 2,
            Integrator::Cf4 => 4,
        }
    }

    /// The method's **nominal** order — what it converges at in its asymptotic
    /// regime.
    ///
    /// Deliberately named `nominal`: see [`observed_order`] and the module
    /// docs. On a nuclide whose own time constant is shorter than the step,
    /// the observed order is not this number and can be negative.
    pub fn nominal_order(self) -> u32 {
        match self {
            Integrator::Predictor => 1,
            Integrator::CeCm => 2,
            Integrator::Cf4 => 4,
        }
    }

    /// Advance `n` by one step of `dt_seconds`.
    ///
    /// `rebuild` supplies the burnup matrix for a given inventory — in a
    /// coupled run that is a transport solve, in an independent run a lookup.
    /// It is called [`Self::solves_per_step`] times.
    ///
    /// Negative densities from round-off in the linear solves are floored at
    /// zero, as the predictor already did.
    pub fn step<F>(self, n: &[f64], dt_seconds: f64, mut rebuild: F) -> Vec<f64>
    where
        F: FnMut(&[f64]) -> DepletionMatrix,
    {
        let clamp = |mut v: Vec<f64>| {
            for d in &mut v {
                if *d < 0.0 {
                    *d = 0.0;
                }
            }
            v
        };
        match self {
            Integrator::Predictor => {
                let a = rebuild(n);
                clamp(cram16(&a, n, dt_seconds))
            }
            Integrator::CeCm => {
                // CE/CM: extrapolate to the MIDPOINT with the begin-of-step
                // matrix, rebuild there, then take the FULL step with the
                // midpoint matrix. The begin-of-step half-step result is
                // discarded — it exists only to find where to evaluate.
                let a0 = rebuild(n);
                let n_half = clamp(cram16(&a0, n, 0.5 * dt_seconds));
                let a_mid = rebuild(&n_half);
                clamp(cram16(&a_mid, n, dt_seconds))
            }
            Integrator::Cf4 => {
                // Commutator-free 4th order, EXACTLY as upstream's
                // `CF4Integrator` docstring states it, with `A_i = h A(..)`
                // so every coefficient below multiplies a matrix that is
                // then exponentiated over the full `dt`:
                //
                //   n1 = exp(A1/2) n
                //   n2 = exp(A2/2) n
                //   n3 = exp(-A1/2 + A3) n1
                //   out = exp( A1/4 + A2/6 + A3/6 - A4/12)
                //       * exp(-A1/12 + A2/6 + A3/6 + A4/4 ) n
                //
                // **The weights must sum to exactly 1.** For a constant `A`
                // the two exponents are `hA/2` each, so the product is
                // `exp(hA)`. A first attempt at this used
                // `(-1/12, 1/6, 1/6, +1/12)` and `(+1/12, 1/6, 1/6, -1/12)`,
                // which sum to `(2/3) hA` — it was 20x wrong on a pure-decay
                // problem, and `every_integrator_is_exact_when_the_matrix_is_
                // constant` is what caught it. That test exists for exactly
                // this coefficient, not as a smoke check.
                let a1 = rebuild(n);
                let n1 = cram16(&a1, n, 0.5 * dt_seconds);
                let a2 = rebuild(&clamp(n1.clone()));
                let n2 = cram16(&a2, n, 0.5 * dt_seconds);
                let a3 = rebuild(&clamp(n2));
                let stage3 = combine(&[(&a1, -0.5), (&a3, 1.0)]);
                let n3 = cram16(&stage3, &n1, dt_seconds);
                let a4 = rebuild(&clamp(n3));

                let second = combine(&[
                    (&a1, -1.0 / 12.0),
                    (&a2, 1.0 / 6.0),
                    (&a3, 1.0 / 6.0),
                    (&a4, 1.0 / 4.0),
                ]);
                let first = combine(&[
                    (&a1, 1.0 / 4.0),
                    (&a2, 1.0 / 6.0),
                    (&a3, 1.0 / 6.0),
                    (&a4, -1.0 / 12.0),
                ]);
                // Intermediates are NOT clamped where they are only carried
                // forward: a CF4 stage is not an inventory and clamping one
                // would perturb the method's order. They ARE clamped where
                // they are used to REBUILD a matrix, because a negative
                // density there gives a nonsense reaction rate.
                let inner = cram16(&second, n, dt_seconds);
                clamp(cram16(&first, &inner, dt_seconds))
            }
        }
    }
}

/// A weighted sum of burnup matrices, `sum_i c_i A_i`.
///
/// Used by CF4, whose stages are exponentials of linear combinations of the
/// stage matrices rather than of the matrices themselves — that is what makes
/// it *commutator-free*.
///
/// # Panics
///
/// Matrices of different order. That is a programmer error (the chain does not
/// change between stages of one step), not a runtime condition.
pub fn combine(terms: &[(&DepletionMatrix, f64)]) -> DepletionMatrix {
    assert!(!terms.is_empty(), "nothing to combine");
    let order = terms[0].0.order();
    for (m, _) in terms {
        assert_eq!(
            m.order(),
            order,
            "burnup matrices of different order cannot be combined; the chain does \
             not change between stages of one step, so this is a programmer error"
        );
    }
    let mut out = DepletionMatrix::zeros(order);
    for (m, c) in terms {
        for row in 0..order {
            for col in 0..order {
                let v = m.get(row, col);
                if v != 0.0 {
                    out.add(row, col, c * v);
                }
            }
        }
    }
    out
}

/// The **observed** order of convergence between two step sizes, by Richardson
/// extrapolation against a reference.
///
/// `coarse` and `fine` are the answers at step `h` and `h/2`, `reference` the
/// converged one.
///
/// ```text
/// order = log2( |coarse - reference| / |fine - reference| )
/// ```
///
/// # Errors
///
/// A fine answer that matches the reference exactly — the ratio is then
/// infinite and the order is undefined, which is a different statement from
/// "the order is very high".
///
/// # This can legitimately be NEGATIVE
///
/// And on this crate's own depletion problems it is: #266's convergence study
/// measured **−3.70** for Xe-135 at 5-day steps, because Xe-135's ~9.14 h
/// half-life is far shorter than the step and the method is nowhere near its
/// asymptotic regime. A negative order is a real result about the problem, not
/// an error in the measurement, so this returns it rather than refusing.
pub fn observed_order(coarse: f64, fine: f64, reference: f64) -> Result<f64, String> {
    let e_coarse = (coarse - reference).abs();
    let e_fine = (fine - reference).abs();
    if e_fine == 0.0 {
        return Err(
            "the fine answer equals the reference exactly, so the error ratio is \
             infinite and the observed order is undefined - which is not the same \
             statement as 'the order is very high'"
                .into(),
        );
    }
    if e_coarse == 0.0 {
        return Err(
            "the coarse answer equals the reference exactly while the fine one does \
             not; there is no convergence to measure"
                .into(),
        );
    }
    Ok((e_coarse / e_fine).log2())
}

/// Continuous removal or feed of a nuclide during depletion —
/// `openmc/deplete/transfer_rates.py`.
///
/// # Why this matters here specifically
///
/// **Pebble recirculation is exactly a transfer rate**, and pebble beds are
/// this crate's specialisation. So is gas stripping, and so is MSR salt
/// processing. A transfer rate adds `-rate * N_i` to the `i`-th nuclide's
/// balance — a first-order loss with the units of inverse time, identical in
/// form to a decay constant, which is why it composes with the burnup matrix
/// by simple addition to the diagonal.
///
/// # Destination is not modelled
///
/// Upstream can route the removed material into a *second* material's
/// inventory. This port models **removal and feed for one material only**:
/// `rate > 0` removes, and [`TransferRate::feed`] adds a constant source.
/// A two-material transfer needs both materials advanced together, which the
/// single-material operator here cannot express — stated rather than
/// approximated by removing from one and hoping.
#[derive(Debug, Clone, PartialEq)]
pub struct TransferRate {
    /// Index of the nuclide in chain order.
    pub nuclide_idx: usize,
    /// First-order removal rate \[s⁻¹\]. Zero for a pure feed.
    pub rate: f64,
    /// Constant feed \[atoms/(barn·cm·s)\]. Zero for a pure removal.
    pub feed: f64,
}

impl TransferRate {
    /// A removal with a given **cycle time** — the natural way to express
    /// pebble recirculation.
    ///
    /// A pebble that spends `cycle_days` in core before being removed has
    /// `rate = 1 / (cycle_days * 86400)`.
    ///
    /// # Errors
    ///
    /// A non-positive cycle time.
    pub fn from_cycle_days(nuclide_idx: usize, cycle_days: f64) -> Result<Self, String> {
        if !(cycle_days > 0.0) {
            return Err(format!(
                "a cycle time of {cycle_days} days is not a residence time"
            ));
        }
        Ok(Self {
            nuclide_idx,
            rate: 1.0 / (cycle_days * SECONDS_PER_DAY),
            feed: 0.0,
        })
    }
}

/// Add transfer rates to a burnup matrix, in place.
///
/// Removal goes on the diagonal, exactly where a decay constant goes. **The
/// feed term does NOT**: a constant source is inhomogeneous and a burnup
/// matrix is homogeneous, so it cannot be expressed as a matrix entry at all.
///
/// # Errors
///
/// A nuclide index outside the chain, a negative rate, or **any non-zero
/// feed** — see [`apply_feed`], which is where a feed has to be handled and
/// which this function deliberately refuses to do silently.
pub fn apply_transfer_rates(
    matrix: &mut DepletionMatrix,
    rates: &[TransferRate],
) -> Result<(), String> {
    for t in rates {
        if t.nuclide_idx >= matrix.order() {
            return Err(format!(
                "transfer rate on nuclide index {} but the chain has {} nuclides",
                t.nuclide_idx,
                matrix.order()
            ));
        }
        if t.rate < 0.0 {
            return Err(format!(
                "nuclide {} has transfer rate {}; a negative removal is a feed and must \
                 be expressed as one",
                t.nuclide_idx, t.rate
            ));
        }
        if t.feed != 0.0 {
            return Err(format!(
                "nuclide {} carries a feed of {}. A constant feed is INHOMOGENEOUS and \
                 cannot be a burnup-matrix entry; folding it into the diagonal would \
                 make it proportional to the density it is supposed to be independent \
                 of. Use `apply_feed` after the step.",
                t.nuclide_idx, t.feed
            ));
        }
        matrix.add(t.nuclide_idx, t.nuclide_idx, -t.rate);
    }
    Ok(())
}

/// Apply the constant feed terms over a step, after the matrix exponential.
///
/// # The approximation, stated
///
/// This adds `feed * dt` to each fed nuclide **after** the step, i.e. it
/// treats the fed material as arriving at the end and not being depleted
/// during the step. Exact only in the limit of a small step against the fed
/// nuclide's own removal rate. The exact treatment needs the inhomogeneous
/// solution `A⁻¹(exp(A dt) − I) f`, which this port does not implement —
/// so the error is first order in the step and is **stated here rather than
/// absorbed**.
pub fn apply_feed(n: &mut [f64], rates: &[TransferRate], dt_seconds: f64) {
    for t in rates {
        if t.feed != 0.0 && t.nuclide_idx < n.len() {
            n[t.nuclide_idx] += t.feed * dt_seconds;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A one-nuclide pure-decay problem: `dN/dt = -lambda N`, exact answer
    /// `N(t) = N0 exp(-lambda t)`.
    fn decay_matrix(lambda: f64) -> DepletionMatrix {
        let mut m = DepletionMatrix::zeros(1);
        m.set(0, 0, -lambda);
        m
    }

    /// **All three integrators are exact on a constant-matrix problem.**
    ///
    /// That is not a weak check: it is the statement that each method reduces
    /// to `exp(A dt)` when `A` does not depend on the inventory, which pins
    /// every one of CF4's stage coefficients. A wrong coefficient shows up
    /// here immediately, because the weights must sum to exactly 1 for the
    /// two-exponential product to give `exp(A dt)`.
    ///
    /// # Result, 2026-09-22
    ///
    /// Predictor, CE/CM and CF4 all reproduce `exp(-lambda t)` to < 1e-12
    /// relative over a 5-day step on a 9.14 h half-life — the Xe-135 case
    /// #266's convergence study found hardest.
    #[test]
    fn every_integrator_is_exact_when_the_matrix_is_constant() {
        let half_life_s = 9.14 * 3600.0;
        let lambda = std::f64::consts::LN_2 / half_life_s;
        let dt = 5.0 * SECONDS_PER_DAY;
        let n0 = vec![1.0_f64];
        let exact = (-lambda * dt).exp();

        for m in [Integrator::Predictor, Integrator::CeCm, Integrator::Cf4] {
            let out = m.step(&n0, dt, |_| decay_matrix(lambda));
            let rel = (out[0] - exact).abs() / exact;
            println!("{m:?}: {:.12e} vs exact {exact:.12e} (rel {rel:.2e})", out[0]);
            assert!(
                rel < 1e-12,
                "{m:?} is not exact on a constant matrix (rel {rel:.3e}); if this is \
                 CF4, its stage weights do not sum to 1"
            );
        }
    }

    /// **CF4 converges faster than the predictor on a non-constant matrix.**
    ///
    /// The matrix is made inventory-dependent — `lambda(N) = lambda0 (1 + N)`
    /// — which is the feature a real burnup matrix has (the flux, and hence
    /// the reaction rates, depend on the inventory) and which is the only
    /// reason the integrators differ at all.
    ///
    /// # Result, 2026-09-22
    ///
    /// Observed orders against a 512-substep reference are printed at run
    /// time. The gate is that CF4's observed order exceeds the predictor's,
    /// **not** that it equals 4 — see the module docs on why a nominal order
    /// is not a promise.
    #[test]
    fn cf4_converges_faster_than_the_predictor_on_a_nonlinear_problem() {
        let lambda0 = 1.0e-5_f64;
        let build = |n: &[f64]| {
            let mut m = DepletionMatrix::zeros(1);
            m.set(0, 0, -lambda0 * (1.0 + n[0]));
            m
        };
        let total = 5.0 * SECONDS_PER_DAY;
        let n0 = vec![1.0_f64];

        let run = |method: Integrator, n_sub: usize| -> f64 {
            let dt = total / n_sub as f64;
            let mut n = n0.clone();
            for _ in 0..n_sub {
                n = method.step(&n, dt, build);
            }
            n[0]
        };

        // Reference: CF4 at 512 substeps.
        let reference = run(Integrator::Cf4, 512);

        let mut orders = Vec::new();
        for method in [Integrator::Predictor, Integrator::CeCm, Integrator::Cf4] {
            let coarse = run(method, 2);
            let fine = run(method, 4);
            let order = observed_order(coarse, fine, reference).unwrap();
            println!(
                "{method:?}: coarse {coarse:.10} fine {fine:.10} ref {reference:.10} \
                 -> observed order {order:.2} (nominal {})",
                method.nominal_order()
            );
            orders.push(order);
        }
        assert!(
            orders[2] > orders[0],
            "CF4's observed order {:.2} does not exceed the predictor's {:.2}",
            orders[2],
            orders[0]
        );
        assert!(
            orders[1] > orders[0],
            "CE/CM's observed order {:.2} does not exceed the predictor's {:.2}",
            orders[1],
            orders[0]
        );
    }

    /// The cost table is what a caller budgets from.
    #[test]
    fn the_solve_counts_are_what_the_methods_actually_do() {
        for m in [Integrator::Predictor, Integrator::CeCm, Integrator::Cf4] {
            let mut calls = 0usize;
            m.step(&[1.0], 1.0, |_| {
                calls += 1;
                decay_matrix(1.0e-6)
            });
            assert_eq!(
                calls,
                m.solves_per_step(),
                "{m:?} claims {} solves and made {calls}",
                m.solves_per_step()
            );
        }
    }

    /// A negative observed order is returned, not refused — it is a real
    /// result about a problem outside the asymptotic regime, as #266's study
    /// measured for Xe-135.
    #[test]
    fn a_negative_observed_order_is_a_result_not_an_error() {
        // Coarse happens to be closer to the reference than fine: errors
        // 0.1 and 0.5, so log2(0.1/0.5) = -2.32.
        let order = observed_order(1.4, 2.0, 1.5).unwrap();
        assert!(order < 0.0, "order = {order}");
        // Degenerate cases are refused with an explanation.
        assert!(observed_order(1.0, 1.5, 1.5).is_err());
        assert!(observed_order(1.5, 2.0, 1.5).is_err());
    }

    /// A removal rate goes on the diagonal, exactly where a decay constant
    /// goes, and a cycle time converts correctly.
    #[test]
    fn a_removal_rate_lands_on_the_diagonal() {
        let t = TransferRate::from_cycle_days(0, 100.0).unwrap();
        assert!((t.rate - 1.0 / (100.0 * SECONDS_PER_DAY)).abs() < 1e-18);
        assert!(TransferRate::from_cycle_days(0, 0.0).is_err());
        assert!(TransferRate::from_cycle_days(0, -1.0).is_err());

        let mut m = decay_matrix(1.0e-6);
        apply_transfer_rates(&mut m, &[t.clone()]).unwrap();
        assert!((m.get(0, 0) - (-1.0e-6 - t.rate)).abs() < 1e-18);

        // Out-of-range and negative rates are refused.
        assert!(apply_transfer_rates(
            &mut m,
            &[TransferRate { nuclide_idx: 9, rate: 1.0, feed: 0.0 }]
        )
        .is_err());
        assert!(apply_transfer_rates(
            &mut m,
            &[TransferRate { nuclide_idx: 0, rate: -1.0, feed: 0.0 }]
        )
        .is_err());
    }

    /// **A feed is refused by the matrix path**, because folding a constant
    /// source into the diagonal would make it proportional to the density it
    /// is supposed to be independent of.
    #[test]
    fn a_feed_cannot_be_folded_into_the_burnup_matrix() {
        let mut m = decay_matrix(1.0e-6);
        let err = apply_transfer_rates(
            &mut m,
            &[TransferRate { nuclide_idx: 0, rate: 0.0, feed: 1.0e-9 }],
        )
        .unwrap_err();
        assert!(err.contains("INHOMOGENEOUS"), "{err}");

        // `apply_feed` is where it belongs.
        let mut n = vec![1.0_f64];
        apply_feed(
            &mut n,
            &[TransferRate { nuclide_idx: 0, rate: 0.0, feed: 2.0 }],
            3.0,
        );
        assert!((n[0] - 7.0).abs() < 1e-12, "{}", n[0]);
    }

    /// A removal at a known rate halves the inventory in one cycle time, to
    /// the accuracy `exp(-1)` gives.
    #[test]
    fn a_pebble_recirculation_rate_removes_the_expected_fraction() {
        let cycle_days = 30.0;
        let t = TransferRate::from_cycle_days(0, cycle_days).unwrap();
        let mut m = DepletionMatrix::zeros(1);
        apply_transfer_rates(&mut m, &[t]).unwrap();
        // After exactly one cycle time, 1/e remains.
        let out = cram16(&m, &[1.0], cycle_days * SECONDS_PER_DAY);
        let want = std::f64::consts::E.recip();
        assert!(
            (out[0] - want).abs() < 1e-10,
            "after one cycle time {} remains, expected 1/e = {want}",
            out[0]
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Independent operator (GitHub #266 scope item 4)
// ═══════════════════════════════════════════════════════════════════════════

use crate::depletion::chain::DepletionChain;
use crate::depletion::operator::{
    flux_for_power, k_inf, one_group_cross_sections, reaction_rates, BurnupResult,
    BurnupSettings, BurnupStep, OneGroupXs,
};

/// A fixed set of one-group microscopic cross sections — `openmc.deplete.MicroXS`.
///
/// # What it is for
///
/// Deplete from cross sections that are **held fixed** rather than recomputed
/// from transport at every step. Orders of magnitude cheaper for a parameter
/// sweep, and the only tractable option when the sweep has hundreds of points.
///
/// # And what it costs, stated
///
/// Fixing the cross sections fixes the **spectrum**. As the inventory burns
/// the real spectrum hardens (fissile depletion) and softens (fission-product
/// absorption), and a frozen set captures neither. The error grows with
/// burnup and is **not** bounded by the integrator's truncation error — a
/// fourth-order integrator on frozen cross sections is fourth-order accurate
/// at solving the wrong problem.
///
/// [`MicroXs::from_chain`] collapses the crate's own CORE data at a stated
/// spectrum, so the frozen point is explicit rather than inherited from
/// whatever the last transport solve happened to give.
#[derive(Debug, Clone, PartialEq)]
pub struct MicroXs {
    /// One entry per chain nuclide, in chain order.
    pub xs: Vec<OneGroupXs>,
    /// A human-readable note on where these came from, carried so a result
    /// cannot be quoted without its provenance.
    pub provenance: String,
}

impl MicroXs {
    /// Collapse the crate's CORE nuclide data onto the chain at the weighting
    /// `settings` specifies.
    pub fn from_chain(chain: &DepletionChain, settings: &BurnupSettings) -> Self {
        Self {
            xs: one_group_cross_sections(chain, settings),
            provenance: format!(
                "collapsed from CORE data at {:?} weighting, frozen at step 0",
                settings.weighting
            ),
        }
    }
}

/// Deplete with **frozen** cross sections — `openmc.deplete.IndependentOperator`.
///
/// No transport solve at any step: the burnup matrix is rebuilt only because
/// the flux changes with the inventory (to hold the requested power), while
/// the microscopic cross sections stay as given.
///
/// # Errors
///
/// A cross-section array that does not match the chain.
pub fn deplete_independent(
    chain: &DepletionChain,
    initial: &[(String, f64)],
    settings: &BurnupSettings,
    micro: &MicroXs,
    method: Integrator,
) -> Result<BurnupResult, String> {
    let names: Vec<&str> = chain.nuclide_names();
    if micro.xs.len() != names.len() {
        return Err(format!(
            "the chain has {} nuclides but {} cross sections were given",
            names.len(),
            micro.xs.len()
        ));
    }

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
        densities: names
            .iter()
            .zip(d)
            .map(|(n, v)| (n.to_string(), *v))
            .collect(),
    };

    let mut steps = Vec::with_capacity(settings.n_steps + 1);
    steps.push(record(0, 0.0, 0.0, &densities));

    let dt = settings.step_days * SECONDS_PER_DAY;
    let mut last_flux = 0.0;
    for step in 1..=settings.n_steps {
        densities = method.step(&densities, dt, |n| {
            let flux = flux_for_power(&names, n, &micro.xs, settings);
            last_flux = flux;
            chain.build_matrix(&reaction_rates(&names, flux, &micro.xs))
        });
        steps.push(record(
            step,
            step as f64 * settings.step_days,
            last_flux,
            &densities,
        ));
    }
    Ok(BurnupResult { steps })
}
