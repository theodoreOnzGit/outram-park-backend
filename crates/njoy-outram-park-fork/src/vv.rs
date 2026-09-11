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

/// Assert a Monte Carlo `k_eff` reproduces a benchmark value.
///
/// The gate is `band + 4 σ`, where `band` is the benchmark's own stated
/// uncertainty (ICSBEP quotes one for every evaluation) and `σ` is the
/// reported statistical error of *this* run. Four sigma, not one: a V&V gate
/// that fires on ordinary statistical fluctuation trains people to ignore it.
///
/// `recorded_pcm` is the offset measured when this case was last looked at by
/// a human and written into the example's doc comment. It is checked as a
/// *separate* claim from agreement with the benchmark — see the module docs.
/// Pass `None` for a case that has never been recorded.
///
/// # Panics
///
/// If the run misses the benchmark by more than `band + 4 σ`, or if it has
/// drifted from `recorded_pcm` by more than `4 σ`.
pub fn assert_reproduces_keff(
    label: &str,
    k_mean: f64,
    k_std: f64,
    benchmark_k: f64,
    band: f64,
    recorded_pcm: Option<f64>,
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

    if let Some(recorded) = recorded_pcm {
        let drift = pcm - recorded;
        let drift_gate = 4.0 * sigma_pcm;
        println!(
            "  [{}] {label}: reproduces the recorded {recorded:+.0} pcm \
             (drift {drift:+.0} pcm, gate ±{drift_gate:.0} pcm)",
            if drift.abs() <= drift_gate {
                "PASS"
            } else {
                "FAIL"
            },
        );
        assert!(
            drift.abs() <= drift_gate,
            "{label}: this run gives {pcm:+.0} pcm but the doc comment records \
             {recorded:+.0} pcm — a drift of {drift:+.0} pcm, more than 4 σ \
             ({drift_gate:.0} pcm).\n\
             Something changed since that number was recorded. Either the \
             physics moved (find out what) or the recorded value is stale (say \
             so in the doc comment, with the date and the reason).",
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
