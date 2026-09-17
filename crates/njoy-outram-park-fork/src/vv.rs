//! Verification & validation gates — the assertion side of an oracle comparison.
//!
//! # Why this module lives in the *data* crate
//!
//! Both this crate and `outram-mc-libs` run oracle comparisons, and
//! `outram-mc-libs` depends on this one. Putting the gate helpers in the lower
//! crate and re-exporting them upward (`outram_mc_libs::vv`) means there is one
//! implementation rather than two — which is the same argument the module makes
//! below about oracle *values*, applied to the code that checks them.
//!
//! # Why this module exists
//!
//! Every example under `examples/` that compares a crate against an oracle
//! (NJOY's PENDF/THERMR output, a closed-form analytic result, an ICSBEP
//! benchmark, or OpenMC) used to *print* the comparison and exit 0 whatever the
//! numbers were. An oracle comparison that cannot fail is not a V&V case: it is
//! a report that nobody reads until the number has already drifted.
//!
//! These helpers turn each such comparison into a gate. They all take a
//! `label`, print a one-line PASS/FAIL verdict with the actual margin, and
//! panic on failure with a message that names the oracle, the measurement and
//! the tolerance — so a CI log says *what* drifted and *by how much*, not just
//! "assertion failed".
//!
//! # Two different things get asserted
//!
//! A V&V case pins **two** numbers, and they are not the same number:
//!
//! 1. **Agreement with the oracle** — "our σ is within 0.2 % of NJOY's". This
//!    is the physics claim. Its tolerance is a judgement about what counts as
//!    agreement, and it is set once, from evidence.
//! 2. **Reproduction of the recorded result** — "the last time a human looked
//!    at this, the answer was +57 pcm, and it still is". This is the
//!    regression claim. Its tolerance is the run-to-run noise, nothing more.
//!
//! Claim 1 without claim 2 lets a result wander anywhere inside a loose
//! envelope without anyone noticing. Claim 2 without claim 1 pins a number
//! that was never right. The doc comment of each V&V example therefore records
//! the measured value **and** the date, and the code asserts both claims.
//!
//! # Never widen a gate to make it pass
//!
//! If a gate here starts failing, the measurement changed. Find out why. A
//! tolerance is only ever loosened together with a written justification in the
//! example's doc comment saying what new evidence supports the wider envelope.

use std::fmt::Write as _;

/// Outcome of a tabulated oracle comparison: the worst relative deviation seen
/// and where it occurred.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorstDeviation {
    /// Abscissa (usually energy in eV) of the worst point.
    pub at: f64,
    /// Value this crate produced there.
    pub measured: f64,
    /// Value the oracle produced there.
    pub oracle: f64,
    /// Signed relative deviation `(measured - oracle) / oracle`.
    pub rel: f64,
}

impl WorstDeviation {
    /// Magnitude of the worst signed deviation, as a fraction.
    #[must_use]
    pub fn abs_rel(&self) -> f64 {
        self.rel.abs()
    }
}

/// Assert a single scalar agrees with an oracle to a relative tolerance.
///
/// `tol_frac` is a fraction, not a percentage: `0.002` means 0.2 %.
///
/// # Panics
///
/// If `|measured - oracle| / |oracle|` exceeds `tol_frac`, or if `oracle` is
/// zero (a relative tolerance is meaningless there — use
/// [`assert_absolute`] instead).
///
/// ```
/// # use njoy_outram_park_fork::vv::assert_relative;
/// // NJOY PENDF gives 11.847 b for U-238 MT=2 at 1 eV; we give 11.849 b.
/// assert_relative("U-238 elastic @ 1 eV", 11.849, 11.847, 0.002);
/// ```
pub fn assert_relative(label: &str, measured: f64, oracle: f64, tol_frac: f64) -> f64 {
    assert!(
        oracle != 0.0,
        "{label}: oracle value is exactly zero, so a relative tolerance is \
         meaningless — use `assert_absolute` for this comparison"
    );
    let rel = (measured - oracle) / oracle;
    println!(
        "  [{}] {label}: {measured:.6e} vs oracle {oracle:.6e}  ({:+.3} %, envelope ±{:.3} %)",
        if rel.abs() <= tol_frac {
            "PASS"
        } else {
            "FAIL"
        },
        rel * 100.0,
        tol_frac * 100.0,
    );
    assert!(
        rel.abs() <= tol_frac,
        "{label}: measured {measured:.8e} deviates {:+.4} % from the oracle \
         {oracle:.8e}, outside the ±{:.4} % envelope.\n\
         This gate is pinned to a recorded oracle comparison. Do not widen it \
         to make the build green — find out what changed in the physics.",
        rel * 100.0,
        tol_frac * 100.0,
    );
    rel
}

/// Assert a single scalar agrees with an oracle to an absolute tolerance, in
/// whatever units the quantity is carried in.
///
/// Use this where the oracle is zero or near zero, or where the meaningful
/// comparison is an absolute one (a cosine, a probability, a pcm offset).
///
/// # Panics
///
/// If `|measured - oracle|` exceeds `tol`.
///
/// ```
/// # use njoy_outram_park_fork::vv::assert_absolute;
/// // Isotropic scattering in the CM frame has mean cosine exactly zero.
/// assert_absolute("mubar_cm, isotropic", 0.0004, 0.0, 0.01);
/// ```
pub fn assert_absolute(label: &str, measured: f64, oracle: f64, tol: f64) -> f64 {
    let diff = measured - oracle;
    println!(
        "  [{}] {label}: {measured:.6e} vs oracle {oracle:.6e}  ({diff:+.3e}, envelope ±{tol:.3e})",
        if diff.abs() <= tol { "PASS" } else { "FAIL" },
    );
    assert!(
        diff.abs() <= tol,
        "{label}: measured {measured:.8e} differs from the oracle {oracle:.8e} \
         by {diff:+.4e}, outside the ±{tol:.4e} envelope.\n\
         This gate is pinned to a recorded oracle comparison. Do not widen it \
         to make the build green — find out what changed in the physics.",
    );
    diff
}

/// Assert a whole tabulated comparison lies inside one relative envelope, and
/// return the worst point so the caller can pin it separately.
///
/// `rows` are `(abscissa, measured, oracle)`. Points whose oracle magnitude is
/// below `significance` are skipped — a channel that is closed, or a cross
/// section of 1e-7 b, carries no information and its relative deviation is
/// pure noise. The number of skipped points is returned to the caller through
/// the printed line and should itself be asserted, so that a data change which
/// silently zeroes a whole column cannot slip through as "everything passed".
///
/// # Panics
///
/// If any significant row falls outside `tol_frac`, or if no row is
/// significant at all.
///
/// ```
/// # use njoy_outram_park_fork::vv::assert_table_relative;
/// let rows = [(1.0, 11.849, 11.847), (10.0, 11.702, 11.700)];
/// let worst = assert_table_relative("U-238 MT=2", &rows, 0.002, 1e-6);
/// assert!(worst.abs_rel() < 0.002);
/// ```
pub fn assert_table_relative(
    label: &str,
    rows: &[(f64, f64, f64)],
    tol_frac: f64,
    significance: f64,
) -> WorstDeviation {
    let mut worst = WorstDeviation {
        at: f64::NAN,
        measured: 0.0,
        oracle: 0.0,
        rel: 0.0,
    };
    let mut n_used = 0usize;
    let mut failures = String::new();

    for &(at, measured, oracle) in rows {
        if oracle.abs() < significance {
            continue;
        }
        n_used += 1;
        let rel = (measured - oracle) / oracle;
        if rel.abs() > worst.abs_rel() || worst.at.is_nan() {
            worst = WorstDeviation {
                at,
                measured,
                oracle,
                rel,
            };
        }
        if rel.abs() > tol_frac {
            let _ = writeln!(
                failures,
                "    at {at:.6e}: measured {measured:.6e} vs oracle {oracle:.6e} ({:+.4} %)",
                rel * 100.0
            );
        }
    }

    assert!(
        n_used > 0,
        "{label}: every one of the {} rows was below the significance floor \
         {significance:.3e}, so nothing was actually compared. Either the \
         oracle table is empty or the measurement collapsed to zero.",
        rows.len(),
    );

    println!(
        "  [{}] {label}: {n_used}/{} points compared, worst {:+.3} % at {:.4e} \
         (envelope ±{:.3} %)",
        if failures.is_empty() { "PASS" } else { "FAIL" },
        rows.len(),
        worst.rel * 100.0,
        worst.at,
        tol_frac * 100.0,
    );

    assert!(
        failures.is_empty(),
        "{label}: {} of {n_used} compared points fall outside the ±{:.4} % \
         envelope:\n{failures}\
         This gate is pinned to a recorded oracle comparison. Do not widen it \
         to make the build green — find out what changed in the physics.",
        failures.lines().count(),
        tol_frac * 100.0,
    );

    worst
}

/// A previously recorded `k_eff` offset, **with the uncertainty it was measured
/// at** — the second half a drift gate needs and used not to have.
///
/// # Why the recorded σ has to be carried
///
/// A drift gate compares this run's offset against a number someone wrote into
/// a doc comment. That number is itself a measurement with its own error, so
/// the difference of the two has variance `σ_run² + σ_recorded²`. Treating the
/// recorded value as exact and gating at `4 σ_run` is too tight by up to `√2`,
/// which turns a ~6e-5 false-failure rate into ~5e-3 — a hundredfold increase
/// in flakiness, on a gate whose whole job is to be believed when it fires.
/// That was GitHub #196 / bead `op-awwi`, and
/// `examples/godiva_keff_ensemble.rs` had already worked around it locally
/// before this type existed.
///
/// The distinction is not academic in this crate: some recorded values are
/// single draws (`σ` of a few hundred pcm) and some are pooled seed ensembles
/// (`sem` of ~11 pcm). One constant cannot serve both.
#[derive(Debug, Clone, Copy)]
pub struct RecordedKeff {
    /// The recorded offset from the benchmark \[pcm\].
    pub pcm: f64,
    /// The **1 σ uncertainty on that recorded number** \[pcm\] — the `sem` of
    /// the ensemble it was pooled over, or the statistical `σ` of the single run
    /// it came from.
    pub sigma_pcm: f64,
}

impl RecordedKeff {
    /// A value pooled over a seed ensemble, carrying that ensemble's standard
    /// error of the mean \[pcm\].
    ///
    /// This is the form to prefer. A pooled number is reproducible; a single
    /// draw is not, and a drift gate built on one mostly measures
    /// re-randomisation.
    pub fn pooled(pcm: f64, sem_pcm: f64) -> Self {
        RecordedKeff {
            pcm,
            sigma_pcm: sem_pcm,
        }
    }

    /// A value recorded from a **single run whose statistics resemble this
    /// one's**, so its uncertainty is taken to equal the current run's `σ`.
    ///
    /// The drift gate then widens by exactly `√2`, which is the correct factor
    /// for the difference of two independent runs of the same program at the
    /// same settings. Use this for a legacy recorded number whose own `σ` was
    /// not written down — it is the honest reading of "measured once, like
    /// this".
    pub fn from_comparable_run(pcm: f64) -> Self {
        RecordedKeff {
            pcm,
            sigma_pcm: f64::NAN, // resolved against the run's own sigma
        }
    }

    /// The uncertainty to combine with this run's `σ_run` \[pcm\].
    fn sigma_against(&self, sigma_run_pcm: f64) -> f64 {
        if self.sigma_pcm.is_finite() {
            self.sigma_pcm
        } else {
            sigma_run_pcm
        }
    }
}

/// Assert a Monte Carlo `k_eff` reproduces a benchmark value.
///
/// The gate is `band + 4 σ`, where `band` is the benchmark's own stated
/// uncertainty (ICSBEP quotes one for every evaluation) and `σ` is the
/// reported statistical error of *this* run. Four sigma, not one: a V&V gate
/// that fires on ordinary statistical fluctuation trains people to ignore it.
///
/// `recorded` is the offset measured when this case was last looked at by
/// a human and written into the example's doc comment, together with the
/// uncertainty it was measured at ([`RecordedKeff`]). It is checked as a
/// *separate* claim from agreement with the benchmark — see the module docs.
/// Pass `None` for a case that has never been recorded.
///
/// The drift gate is `4 · √(σ_run² + σ_recorded²)`, the spread of the
/// *difference of two independent measurements*. Using `4 · σ_run` instead
/// treats the recorded number as exact and makes the gate too tight by up to
/// `√2` — GitHub #196 / bead `op-awwi`.
///
/// # Panics
///
/// If the run misses the benchmark by more than `band + 4 σ`, or if it has
/// drifted from `recorded.pcm` by more than `4 · √(σ_run² + σ_recorded²)`.
pub fn assert_reproduces_keff(
    label: &str,
    k_mean: f64,
    k_std: f64,
    benchmark_k: f64,
    band: f64,
    recorded: Option<RecordedKeff>,
) {
    let pcm = (k_mean - benchmark_k) * 1.0e5;
    let sigma_pcm = k_std * 1.0e5;
    let gate_pcm = (band + 4.0 * k_std) * 1.0e5;

    println!(
        "  [{}] {label}: k = {k_mean:.5} ± {k_std:.5} vs benchmark \
         {benchmark_k:.5} ± {band:.5}  ({pcm:+.0} pcm, gate ±{gate_pcm:.0} pcm)",
        if pcm.abs() <= gate_pcm {
            "PASS"
        } else {
            "FAIL"
        },
    );

    if let Some(recorded) = recorded {
        let drift = pcm - recorded.pcm;
        // The difference of two independent measurements, not of one
        // measurement and a constant: sigma_diff = sqrt(sigma_run^2 +
        // sigma_recorded^2). Gating at 4*sigma_run instead would treat the
        // recorded number as exact and be too tight by up to sqrt(2) —
        // GitHub #196 / bead op-awwi.
        let sigma_recorded = recorded.sigma_against(sigma_pcm);
        let sigma_diff = (sigma_pcm * sigma_pcm + sigma_recorded * sigma_recorded).sqrt();
        let drift_gate = 4.0 * sigma_diff;
        println!(
            "  [{}] {label}: reproduces the recorded {:+.0} ± {sigma_recorded:.0} pcm \
             (drift {drift:+.0} pcm, gate ±{drift_gate:.0} pcm = 4·√(σ_run² + σ_rec²))",
            if drift.abs() <= drift_gate {
                "PASS"
            } else {
                "FAIL"
            },
            recorded.pcm,
        );
        assert!(
            drift.abs() <= drift_gate,
            "{label}: this run gives {pcm:+.0} ± {sigma_pcm:.0} pcm but the doc \
             comment records {:+.0} ± {sigma_recorded:.0} pcm — a drift of \
             {drift:+.0} pcm, more than 4 σ of the difference \
             ({drift_gate:.0} pcm).\n\
             Something changed since that number was recorded. Either the \
             physics moved (find out what) or the recorded value is stale (say \
             so in the doc comment, with the date and the reason).",
            recorded.pcm,
        );
    }

    assert!(
        pcm.abs() <= gate_pcm,
        "{label}: k_eff = {k_mean:.5} ± {k_std:.5} misses the benchmark \
         {benchmark_k:.5} ± {band:.5} by {pcm:+.0} pcm, outside the \
         ±{gate_pcm:.0} pcm gate (benchmark band + 4 σ).\n\
         This gate is pinned to a measured criticality experiment. Do not \
         widen it to make the build green.",
    );
}

/// Assert a measurement still reproduces the value recorded in a doc comment.
///
/// This is the regression half of a V&V case, for deterministic quantities
/// where there is no statistical σ to scale by. `tol_frac` should be tight —
/// it is bounding arithmetic reproducibility, not physics agreement.
///
/// # Panics
///
/// If `measured` has drifted from `recorded` by more than `tol_frac`.
///
/// ```
/// # use njoy_outram_park_fork::vv::assert_reproduces_recorded;
/// // Recorded 2026-09-11: xi/xi_0 = 1.0000 for graphite.
/// assert_reproduces_recorded("graphite xi/xi_0", 1.00003, 1.0000, 1e-3);
/// ```
pub fn assert_reproduces_recorded(label: &str, measured: f64, recorded: f64, tol_frac: f64) {
    assert!(
        recorded != 0.0,
        "{label}: recorded value is exactly zero — use `assert_absolute`"
    );
    let drift = (measured - recorded) / recorded;
    println!(
        "  [{}] {label}: {measured:.6e} reproduces the recorded {recorded:.6e} \
         ({:+.4} %, envelope ±{:.4} %)",
        if drift.abs() <= tol_frac {
            "PASS"
        } else {
            "FAIL"
        },
        drift * 100.0,
        tol_frac * 100.0,
    );
    assert!(
        drift.abs() <= tol_frac,
        "{label}: measured {measured:.8e} has drifted {:+.4} % from the \
         {recorded:.8e} recorded in this example's doc comment, outside \
         ±{:.4} %.\n\
         Either the physics moved (find out what) or the recorded value is \
         stale (update it, with the date and the reason).",
        drift * 100.0,
        tol_frac * 100.0,
    );
}

/// Assert a sequence is monotone in the stated direction.
///
/// Several oracle comparisons here are really claims about *shape*, not
/// magnitude: a 1/v absorber must fall monotonically with energy, a bound
/// scattering kernel must converge onto the free-gas limit from above. A
/// magnitude envelope alone does not catch a kernel that has the right area
/// and the wrong shape — which is exactly how the H-in-H₂O defect (GitHub
/// #188) survived a full analytic test file.
///
/// # Panics
///
/// If any adjacent pair violates the direction by more than `slack` (a
/// fraction of the earlier value, to tolerate grid noise).
pub fn assert_monotone(label: &str, values: &[f64], increasing: bool, slack: f64) {
    assert!(
        values.len() >= 2,
        "{label}: monotonicity needs at least two points, got {}",
        values.len()
    );
    for (i, pair) in values.windows(2).enumerate() {
        let (a, b) = (pair[0], pair[1]);
        let ok = if increasing {
            b >= a - slack * a.abs()
        } else {
            b <= a + slack * a.abs()
        };
        assert!(
            ok,
            "{label}: sequence is not {} at index {i}→{}: {a:.6e} → {b:.6e} \
             (slack {:.3} %).\n\
             This is a shape assertion. A kernel with the right area and the \
             wrong shape passes every magnitude envelope — that is how the \
             H-in-H₂O defect survived an entire analytic test file.",
            if increasing {
                "non-decreasing"
            } else {
                "non-increasing"
            },
            i + 1,
            slack * 100.0,
        );
    }
    println!(
        "  [PASS] {label}: {} points are {}",
        values.len(),
        if increasing {
            "non-decreasing"
        } else {
            "non-increasing"
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The k \[pcm\] a benchmark gate is judged at, and a σ typical of a single
    /// 5000-history Godiva run.
    const SIGMA_RUN_PCM: f64 = 173.0;
    /// A pooled 256-seed `sem`, as `examples/godiva_keff_ensemble.rs` reports.
    const SIGMA_POOLED_PCM: f64 = 11.0;

    /// Drive `assert_reproduces_keff` with a drift and report whether it passed.
    ///
    /// The benchmark arm is made unreachable (a huge band) so only the **drift**
    /// gate can fire — otherwise a failure here would be ambiguous between the
    /// two claims the function checks.
    fn drift_passes(drift_pcm: f64, sigma_run_pcm: f64, recorded: RecordedKeff) -> bool {
        let k_mean = 1.0 + (recorded.pcm + drift_pcm) * 1.0e-5;
        std::panic::catch_unwind(|| {
            assert_reproduces_keff(
                "gate-sizing unit test",
                k_mean,
                sigma_run_pcm * 1.0e-5,
                1.0,
                1.0, // benchmark band of 1.0 in k: the benchmark gate cannot fire
                Some(recorded),
            );
        })
        .is_ok()
    }

    /// The drift gate must be `4·√(σ_run² + σ_recorded²)` — the spread of the
    /// **difference of two independent measurements** — not `4·σ_run`.
    ///
    /// This is the GitHub #196 / `op-awwi` defect, tested rather than asserted.
    /// Treating the recorded number as exact makes the gate too tight by up to
    /// `√2`, which raises the false-failure rate of a 4 σ gate from ~6e-5 to
    /// ~5e-3 — a hundredfold increase in flakiness on a gate whose whole value
    /// is being believed when it fires.
    #[test]
    fn the_drift_gate_combines_both_uncertainties() {
        let recorded = RecordedKeff::pooled(16.0, SIGMA_POOLED_PCM);
        let expected =
            4.0 * (SIGMA_RUN_PCM * SIGMA_RUN_PCM + SIGMA_POOLED_PCM * SIGMA_POOLED_PCM).sqrt();

        assert!(
            drift_passes(expected * 0.99, SIGMA_RUN_PCM, recorded),
            "a drift just inside 4·√(σ_run² + σ_rec²) = {expected:.1} pcm must pass"
        );
        assert!(
            !drift_passes(expected * 1.01, SIGMA_RUN_PCM, recorded),
            "a drift just outside 4·√(σ_run² + σ_rec²) = {expected:.1} pcm must fail"
        );

        // And it is genuinely wider than the old `4·σ_run` rule wherever the
        // recorded value carries any uncertainty at all. With a pooled sem of 11
        // against a run sigma of 173 the widening is small (0.2 %) — which is
        // the point: the correction matters where the recorded value is itself
        // noisy, not where it is well pooled.
        let old_gate = 4.0 * SIGMA_RUN_PCM;
        assert!(
            expected > old_gate,
            "the corrected gate {expected:.1} must be at least as wide as the old {old_gate:.1}"
        );
    }

    /// A value recorded from a run with comparable statistics widens the gate by
    /// exactly `√2`, which is the correct factor for the difference of two
    /// independent runs of one program at one setting.
    #[test]
    fn a_comparable_run_widens_the_gate_by_root_two() {
        let recorded = RecordedKeff::from_comparable_run(1042.0);
        let root_two_gate = 4.0 * SIGMA_RUN_PCM * std::f64::consts::SQRT_2;

        assert!(
            drift_passes(root_two_gate * 0.99, SIGMA_RUN_PCM, recorded),
            "a drift just inside 4·√2·σ_run = {root_two_gate:.1} pcm must pass"
        );
        assert!(
            !drift_passes(root_two_gate * 1.01, SIGMA_RUN_PCM, recorded),
            "a drift just outside 4·√2·σ_run = {root_two_gate:.1} pcm must fail"
        );

        // The old behaviour — treating the recorded value as exact — would have
        // FAILED a drift of 1.2·σ_run·4, which is a perfectly ordinary outcome
        // for two independent runs. Pinning that here is what stops the fix
        // being quietly reverted.
        let ordinary = 4.0 * SIGMA_RUN_PCM * 1.2;
        assert!(
            ordinary > 4.0 * SIGMA_RUN_PCM,
            "test setup: the probe drift must exceed the old gate"
        );
        assert!(
            drift_passes(ordinary, SIGMA_RUN_PCM, recorded),
            "a drift of {ordinary:.1} pcm is 1.2 × the OLD 4·σ_run gate but only 0.85 × the \
             correct one; it must pass. If it fails, the gate has reverted to treating the \
             recorded value as exact (gh:#196)."
        );
    }

    /// A `None` recorded value checks the benchmark claim only, and must not
    /// panic on the drift path at all.
    #[test]
    fn no_recorded_value_means_no_drift_gate() {
        let ok = std::panic::catch_unwind(|| {
            assert_reproduces_keff("no recorded value", 1.0, 1.0e-3, 1.0, 1.0e-3, None);
        })
        .is_ok();
        assert!(ok, "a run sitting on the benchmark with no recorded value must pass");
    }
}
