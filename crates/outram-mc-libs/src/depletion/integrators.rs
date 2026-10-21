// SPDX-License-Identifier: GPL-3.0

//! **Depletion integrators (from PETIR), this crate's one-group operator,
//! and transfer rates** — GitHub #266 scope items 2, 3 and 4; gh:#950.
//!
//! # The integrators live in PETIR (2026-10-17, gh:#950)
//!
//! The maintainer's direction on #950 is that **all depletion math** goes to
//! `petir`, while this crate keeps the operator: the chain, the reaction
//! rates and the burnup-matrix entries. So:
//!
//! * [`Integrator`] is `petir::depletion::Integrator`, re-exported at its old
//!   path. It now has all **eight** of OpenMC's integrators
//!   (`openmc/deplete/integrators.py`): Predictor, CE/CM, CF4, CE/LI,
//!   EPC-RK4, LE/QI, SI-CE/LI and SI-LE/QI. Before the move this crate had
//!   three, written here in structure from upstream.
//! * [`OneGroupOperator`] is this crate's side: it implements
//!   `petir::depletion::TransportOperator` from the one-group cross
//!   sections, holding the requested power by renormalising the flux at every
//!   evaluation, and forms the matrix with
//!   [`DepletionChain::build_matrix`].
//! * [`deplete_with`](super::operator::deplete_with) and
//!   [`deplete_independent`] drive PETIR's integrators through it.
//!
//! The old `Integrator::step(n, dt, rebuild)` closure API is gone; nothing
//! outside this module called it.
//!
//! # What changed in the numbers, and why
//!
//! Predictor and CE/CM are **bit-identical** to before the move
//! (`tests/depletion_move_fingerprints.rs`). **CF4 is not, deliberately**:
//! the pre-move CF4 applied its last two exponentials in the order upstream's
//! *docstring* implies, while upstream's *code* applies them the other way
//! round (`integrators.py:180-183`), and it skipped upstream's clip of the
//! first stage (`abc.py:765-766`). PETIR follows the code. On PETIR's exact
//! test problem the docstring order converges at **order 2.000** and the
//! code order at **3.98**
//! (`crates/petir/verification_and_validation/depletion/petir_integrators_2026_10_17.md`),
//! which is why `tests/depletion_step_convergence.rs` measured CF4 at 1.94
//! on this crate's chain before the move.
//!
//! # Solver: CRAM16, as before
//!
//! The drivers here use `CramSolver::Cram16`, this crate's choice since the
//! burnup loop was first written, so the recorded results do not move.
//! Upstream's default is CRAM48 (`abc.py:643`).

use crate::mathf::RealMath;
use std::collections::HashMap;
use std::convert::Infallible;

use crate::depletion::chain::DepletionChain;
use crate::depletion::matrix::DepletionMatrix;
use crate::depletion::operator::{
    flux_for_power, k_inf, one_group_cross_sections, reaction_rates, BurnupResult, BurnupSettings,
    BurnupStep, OneGroupXs,
};
use crate::depletion::{MicroRate, ReactionRates};

pub use petir::depletion::{
    AverageRates, CramSolver, DepletionError, DepletionIntegrator, Integrator, Keff,
    OperatorResult, TransportOperator,
};

/// Seconds in a day, matching `operator.rs`.
pub const SECONDS_PER_DAY: f64 = 86_400.0;

/// A weighted sum of burnup matrices, `sum_i c_i A_i`, accumulated from zero
/// and skipping zero entries.
///
/// Written for the pre-move CF4 and kept for callers; PETIR's integrators use
/// `petir::depletion::lincomb`, which evaluates upstream's expressions left
/// to right instead (`_matrix_funcs.py`).
///
/// # Panics
///
/// An empty list, or matrices of different order. That is a programmer error
/// (the chain does not change between stages of one step), not a runtime
/// condition.
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
        for (o, &v) in out.as_mut_slice().iter_mut().zip(m.as_slice()) {
            if v != 0.0 {
                *o += c * v;
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
    Ok((e_coarse / e_fine).r_log2())
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
        matrix
            .add(t.nuclide_idx, t.nuclide_idx, -t.rate)
            .map_err(|e| format!("transfer rate on nuclide {}: {e}", t.nuclide_idx))?;
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

// ═══════════════════════════════════════════════════════════════════════════
// The one-group operator (this crate's side of PETIR's TransportOperator)
// ═══════════════════════════════════════════════════════════════════════════

/// The stochastic-implicit integrators average reaction rates between
/// iterations (`integrators.py:463`). OpenMC averages its `ReactionRates`
/// array element by element; this does the same per nuclide and channel, and
/// to the flux the rates were normalised to.
impl AverageRates for ReactionRates {
    fn weighted_sum(a: f64, x: &Self, b: f64, y: &Self) -> Self {
        let mut micro_rates: HashMap<String, MicroRate> = HashMap::new();
        for name in x.micro_rates.keys().chain(y.micro_rates.keys()) {
            let (rx, ry) = (x.rate_for(name), y.rate_for(name));
            micro_rates.insert(
                name.clone(),
                MicroRate {
                    gamma: a * rx.gamma + b * ry.gamma,
                    fission: a * rx.fission + b * ry.fission,
                    n2n: a * rx.n2n + b * ry.n2n,
                },
            );
        }
        ReactionRates {
            flux: a * x.flux + b * y.flux,
            micro_rates,
        }
    }
}

/// This crate's one-group, single-material depletion operator, for PETIR's
/// integrators — the role of OpenMC's `IndependentOperator`
/// (`openmc/deplete/independent_operator.py`) with the chain's
/// `form_matrix`.
///
/// Each evaluation renormalises the flux so the fission power in
/// `settings.fuel_volume_cm3` equals `settings.power_watts`
/// ([`flux_for_power`]) and returns the flux-multiplied one-group rates;
/// [`TransportOperator::form_matrix`] assembles them with
/// [`DepletionChain::build_matrix`]. The cross sections `xs` are held fixed.
/// The reported `k` is the one-group [`k_inf`] with no uncertainty.
///
/// It owns a copy of the chain (the workspace rules forbid borrowed fields).
#[derive(Debug, Clone)]
pub struct OneGroupOperator {
    chain: DepletionChain,
    xs: Vec<OneGroupXs>,
    settings: BurnupSettings,
    /// The flux of the most recent evaluation, `neutrons/(cm²·s)`.
    pub last_flux: f64,
}

impl OneGroupOperator {
    /// An operator over `chain` with fixed one-group cross sections `xs`
    /// (one per chain nuclide, in chain order).
    ///
    /// # Errors
    ///
    /// `xs` not matching the chain.
    pub fn new(
        chain: &DepletionChain,
        xs: Vec<OneGroupXs>,
        settings: &BurnupSettings,
    ) -> Result<Self, String> {
        let n = chain.nuclide_names().len();
        if xs.len() != n {
            return Err(format!(
                "the chain has {n} nuclides but {} cross sections were given",
                xs.len()
            ));
        }
        Ok(Self {
            chain: chain.clone(),
            xs,
            settings: settings.clone(),
            last_flux: 0.0,
        })
    }
}

impl TransportOperator for OneGroupOperator {
    type Rates = ReactionRates;
    type Error = Infallible;

    fn evaluate(
        &mut self,
        n: &[Vec<f64>],
        _source_rate: f64,
    ) -> Result<OperatorResult<ReactionRates>, Infallible> {
        let names = self.chain.nuclide_names();
        let d = n.first().map(Vec::as_slice).unwrap_or(&[]);
        let flux = flux_for_power(&names, d, &self.xs, &self.settings);
        self.last_flux = flux;
        Ok(OperatorResult {
            k: Keff {
                value: k_inf(d, &self.xs),
                std_dev: 0.0,
            },
            rates: reaction_rates(&names, flux, &self.xs),
        })
    }

    fn form_matrix(
        &self,
        rates: &ReactionRates,
        _material: usize,
    ) -> Result<DepletionMatrix, Infallible> {
        Ok(self.chain.build_matrix(rates))
    }
}

/// Which flux a burnup history records against each step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecordedFlux {
    /// The beginning-of-step flux (`deplete_with`, `deplete_predictor`).
    BeginningOfStep,
    /// The flux of the step's last operator evaluation
    /// (`deplete_independent`, as it recorded before the move).
    LastStage,
}

/// Run a one-group burnup history with PETIR's integrator `method` and
/// CRAM16 — the step loop of OpenMC's `Integrator.integrate`
/// (`abc.py:944-970`) and `SIIntegrator.integrate` (`abc.py:1400-1429`):
/// evaluate the operator at the beginning of each step (for the
/// stochastic-implicit methods only at step 0, then carry the averaged
/// result), call the integrator, advance.
///
/// It drives `DepletionIntegrator::step` itself, rather than
/// `DepletionIntegrator::integrate`, only so it can read the operator's flux
/// between steps for [`RecordedFlux::LastStage`].
///
/// # Panics
///
/// If PETIR's CRAM reports a singular pivot, which before the move produced
/// non-finite densities silently; this crate's burnup drivers return a plain
/// `BurnupResult`, so it panics with PETIR's error rather than inventing one.
pub(crate) fn run_one_group(
    method: Integrator,
    chain: &DepletionChain,
    initial: &[(String, f64)],
    settings: &BurnupSettings,
    xs: Vec<OneGroupXs>,
    recorded: RecordedFlux,
) -> Result<BurnupResult, String> {
    let names: Vec<&str> = chain.nuclide_names();
    let mut op = OneGroupOperator::new(chain, xs, settings)?;

    let mut densities = vec![0.0_f64; names.len()];
    for (name, dens) in initial {
        if let Some(idx) = chain.index_of(name) {
            densities[idx] = *dens;
        }
    }

    let record =
        |step: usize, time_days: f64, flux: f64, d: &[f64], op: &OneGroupOperator| BurnupStep {
            step,
            time_days,
            flux,
            k_inf: k_inf(d, &op.xs),
            densities: names
                .iter()
                .zip(d)
                .map(|(n, v)| (n.to_string(), *v))
                .collect(),
        };

    let mut steps = Vec::with_capacity(settings.n_steps + 1);
    steps.push(record(0, 0.0, 0.0, &densities, &op));

    let dt = settings.step_days * SECONDS_PER_DAY;
    let power = settings.power_watts;
    let mut integ = DepletionIntegrator::new(
        method,
        CramSolver::Cram16,
        vec![dt; settings.n_steps],
        vec![power; settings.n_steps],
    )
    .map_err(|e| e.to_string())?;
    let fail = |e: DepletionError<Infallible>| -> String {
        match e {
            DepletionError::Numeric(e) => panic!("depletion step failed in PETIR: {e}"),
            DepletionError::Operator(never) => match never {},
        }
    };

    let mut n = vec![densities];
    let mut carried: Option<OperatorResult<ReactionRates>> = None;
    for step in 1..=settings.n_steps {
        let i = step - 1;
        let bos = match carried.take() {
            Some(res) if method.is_stochastic_implicit() && i > 0 => res,
            _ => op
                .evaluate(&n, power)
                .map_err(|e: Infallible| -> String { match e {} })?,
        };
        let flux0 = bos.rates.flux;
        let out = integ
            .step(&mut op, &n, &bos.rates, dt, power, i)
            .map_err(fail)?;
        n = out.n_end;
        carried = out.res_end;
        let flux = match recorded {
            RecordedFlux::BeginningOfStep => flux0,
            RecordedFlux::LastStage => op.last_flux,
        };
        let d = n.first().cloned().unwrap_or_default();
        steps.push(record(
            step,
            step as f64 * settings.step_days,
            flux,
            &d,
            &op,
        ));
    }
    Ok(BurnupResult { steps })
}

// ═══════════════════════════════════════════════════════════════════════════
// Independent operator (GitHub #266 scope item 4)
// ═══════════════════════════════════════════════════════════════════════════

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
/// the microscopic cross sections stay as given. Any of PETIR's eight
/// integrators may be used. The flux recorded against each step is that of
/// the step's **last** operator evaluation, as before the move.
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
    run_one_group(
        method,
        chain,
        initial,
        settings,
        micro.xs.clone(),
        RecordedFlux::LastStage,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::depletion::cram::cram16;

    /// A one-nuclide pure-decay problem: `dN/dt = -lambda N`.
    fn decay_matrix(lambda: f64) -> DepletionMatrix {
        let mut m = DepletionMatrix::zeros(1);
        m.set(0, 0, -lambda).unwrap();
        m
    }

    // The pre-move tests of `Integrator::step` (exactness on a constant
    // matrix, CF4 against the predictor, the solve counts) moved with the
    // integrators: `petir::depletion::integrators::tests::{
    // exact_on_a_constant_matrix, operator_call_counts}` and
    // `crates/petir/tests/depletion_integrator_orders.rs`.

    /// `combine` accumulates from zero and checks orders.
    #[test]
    fn combine_is_a_weighted_sum() {
        let a = decay_matrix(2.0);
        let b = decay_matrix(4.0);
        assert_eq!(combine(&[(&a, 0.5), (&b, 0.25)]).get(0, 0), -2.0);
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
            &[TransferRate {
                nuclide_idx: 9,
                rate: 1.0,
                feed: 0.0
            }]
        )
        .is_err());
        assert!(apply_transfer_rates(
            &mut m,
            &[TransferRate {
                nuclide_idx: 0,
                rate: -1.0,
                feed: 0.0
            }]
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
            &[TransferRate {
                nuclide_idx: 0,
                rate: 0.0,
                feed: 1.0e-9,
            }],
        )
        .unwrap_err();
        assert!(err.contains("INHOMOGENEOUS"), "{err}");

        // `apply_feed` is where it belongs.
        let mut n = vec![1.0_f64];
        apply_feed(
            &mut n,
            &[TransferRate {
                nuclide_idx: 0,
                rate: 0.0,
                feed: 2.0,
            }],
            3.0,
        );
        assert!((n[0] - 7.0).abs() < 1e-12, "{}", n[0]);
    }

    /// A removal at a known rate leaves `1/e` after one cycle time.
    #[test]
    fn a_pebble_recirculation_rate_removes_the_expected_fraction() {
        let cycle_days = 30.0;
        let t = TransferRate::from_cycle_days(0, cycle_days).unwrap();
        let mut m = DepletionMatrix::zeros(1);
        apply_transfer_rates(&mut m, &[t]).unwrap();
        let out = cram16(&m, &[1.0], cycle_days * SECONDS_PER_DAY);
        let want = std::f64::consts::E.recip();
        assert!(
            (out[0] - want).abs() < 1e-10,
            "after one cycle time {} remains, expected 1/e = {want}",
            out[0]
        );
    }

    /// The SI average of `ReactionRates` is per nuclide, per channel, and
    /// covers nuclides present on either side.
    #[test]
    fn reaction_rates_average_like_openmc_arrays() {
        let mut x = ReactionRates {
            flux: 2.0,
            ..ReactionRates::zero()
        };
        x.set(
            "U235",
            MicroRate {
                gamma: 1.0,
                fission: 2.0,
                n2n: 0.0,
            },
        );
        let mut y = ReactionRates {
            flux: 6.0,
            ..ReactionRates::zero()
        };
        y.set(
            "U238",
            MicroRate {
                gamma: 4.0,
                fission: 0.0,
                n2n: 8.0,
            },
        );
        let avg = ReactionRates::weighted_sum(0.25, &x, 0.75, &y);
        assert_eq!(avg.flux, 0.25 * 2.0 + 0.75 * 6.0);
        assert_eq!(
            avg.rate_for("U235"),
            MicroRate {
                gamma: 0.25,
                fission: 0.5,
                n2n: 0.0
            }
        );
        assert_eq!(
            avg.rate_for("U238"),
            MicroRate {
                gamma: 3.0,
                fission: 0.0,
                n2n: 6.0
            }
        );
    }

    /// The operator refuses cross sections that do not match the chain, and
    /// every one of the eight integrators runs a burnup history through it.
    #[test]
    fn the_one_group_operator_drives_all_eight() {
        let chain = DepletionChain::simple();
        let settings = BurnupSettings {
            n_steps: 3,
            step_days: 2.0,
            ..BurnupSettings::default()
        };
        assert!(OneGroupOperator::new(&chain, vec![], &settings).is_err());
        let micro = MicroXs::from_chain(&chain, &settings);
        let initial = vec![("U235".to_string(), 1.0e-3), ("U238".to_string(), 2.0e-2)];
        for method in Integrator::ALL {
            let r = deplete_independent(&chain, &initial, &settings, &micro, method).unwrap();
            assert_eq!(r.steps.len(), 4, "{method:?}");
            let u235 = r.history("U235");
            assert!(
                u235.windows(2).all(|w| w[1].1 < w[0].1),
                "{method:?}: U-235 must burn"
            );
            assert!(r.steps.iter().all(|s| s.flux >= 0.0 && s.k_inf.is_finite()));
        }
    }
}
