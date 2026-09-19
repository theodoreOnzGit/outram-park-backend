//! Verification & validation gates and the committed oracle tables this crate is
//! measured against.
//!
//! The gate helpers themselves live one crate down, in
//! [`njoy_outram_park_fork::vv`], because both crates run oracle comparisons and
//! this one depends on that one — so there is a single implementation rather
//! than two that drift. They are re-exported here so callers need only one path.
//!
//! [`njoy_golden`] holds the NJOY2016 oracle **values** for the comparisons that
//! are about *this* crate's fidelity: the S(alpha,beta) laws and the U-238 point
//! cross sections. Those belong here rather than in the data crate, because what
//! they measure is `outram-mc-libs` reproducing NJOY, not NJOY reproducing
//! itself.

pub mod njoy_golden;

pub use njoy_outram_park_fork::vv::{
    assert_absolute, assert_monotone, assert_relative, assert_reproduces_keff,
    assert_reproduces_recorded, assert_table_relative, RecordedKeff, WorstDeviation,
};

/// Pooled statistics of a seed ensemble: `(mean, sample sd, standard error)`.
///
/// # Why a shared helper and not a local closure
///
/// Every criticality benchmark in this crate is a Monte Carlo estimate whose
/// **single-run scatter is far larger than the effect sizes being argued
/// about** — Godiva carries `sd ≈ 175 pcm` per run at 5000 histories ×
/// [40 + 120]. A single run therefore cannot resolve a 100–200 pcm change, and
/// this crate's record contains three headline numbers that were single draws
/// and moved by more than their own quoted uncertainty when pooled
/// (`+57 → +228`, `+512 → +247`, URR `+79 → +43`).
///
/// The fix is to make pooling the default way a benchmark reports, which means
/// it has to be one line at every call site.
///
/// # The distinction that matters
///
/// - **`sd`** is what *one* run scatters by. Quote it when telling a reader
///   what a single reproduction of the example will give them.
/// - **`sem = sd/√n`** is the uncertainty *on the pooled mean*. Quote it when
///   comparing against a benchmark or another code.
///
/// Confusing the two is how a result gets over- or under-claimed. Both are
/// returned so a caller cannot silently pick the flattering one.
///
/// Returns `(mean, 0.0, 0.0)` for a single sample: one draw has no measurable
/// spread, and reporting `0` uncertainty is more honest than inventing one.
///
/// # Example
///
/// ```
/// use outram_mc_libs::vv::pooled;
/// let (mean, sd, sem) = pooled(&[100.0, 200.0, 300.0]);
/// assert!((mean - 200.0).abs() < 1e-9);
/// assert!((sd - 100.0).abs() < 1e-9);
/// assert!((sem - 100.0 / 3.0_f64.sqrt()).abs() < 1e-9);
/// ```
pub fn pooled(x: &[f64]) -> (f64, f64, f64) {
    let n = x.len();
    if n == 0 {
        return (f64::NAN, 0.0, 0.0);
    }
    let mean = x.iter().sum::<f64>() / n as f64;
    if n == 1 {
        return (mean, 0.0, 0.0);
    }
    let var = x.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
    let sd = var.sqrt();
    (mean, sd, sd / (n as f64).sqrt())
}

/// How many seeds a benchmark example should run, from `OUTRAM_BENCH_SEEDS`.
///
/// Defaults to `1`, so an example keeps its original single-seed behaviour and
/// its original runtime unless a caller asks for an ensemble. Set it to the
/// count that reaches the uncertainty you need: at seed-to-seed `sd`, the
/// pooled `sem` is `sd/√n`, so 7 seeds reach ~70 pcm on a case with
/// `sd ≈ 175 pcm` and 64 reach ~22.
pub fn bench_seeds() -> usize {
    std::env::var("OUTRAM_BENCH_SEEDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&n: &usize| n >= 1)
        .unwrap_or(1)
}

/// Report the transport-loss channels of a [`KeffResult`] to stderr.
///
/// **What this is for.** A Monte Carlo history that is lost, or that exhausts
/// its event budget, is normally **scored as a leak**. The neutron balance
/// then closes and `k_eff` looks entirely healthy, so a transport defect of
/// this class is invisible in the reported eigenvalue and its uncertainty.
/// This crate has already been bitten by exactly that: an HTR-10 core model
/// in which **68.5 % of histories** ended on the event budget and were
/// counted as leakage, with no outward sign in `k`.
///
/// Call this on any case whose residual is being interpreted as physics.
/// A non-zero `lost_locate` or `stuck_events` means some fraction of the
/// answer is a geometry defect wearing a leakage costume, and the residual
/// cannot be attributed to nuclear data until it is zero.
///
/// **Interpreting the fields**
///
/// - `lost_locate` — [`Geometry::locate`] failed to place a particle. Always
///   a defect; there is no physical configuration in which a point inside the
///   model has no cell.
/// - `stuck_events` — the history hit the per-history event cap. Either the
///   geometry traps it (coincident or mis-sensed surfaces) or the cap is too
///   low for a legitimately long walk. `stuck_path_cm` and `stuck_last_e`
///   distinguish the two: a trapped particle accumulates almost no path.
/// - `neg_dist` — a negative distance-to-boundary was returned.
///   `neg_from_lattice` splits out the lattice path, which is where the
///   coordinate-frame bugs of this crate's history have lived.
///
/// All rates are expressed per history, because an absolute count is
/// meaningless without the denominator.
///
/// This writes to **stderr** so it composes with examples whose stdout is a
/// machine-read data stream.
pub fn report_transport_losses(label: &str, r: &crate::physics::keff::KeffResult) {
    let n = r.histories.max(1) as f64;
    let pct = |x: u64| 100.0 * x as f64 / n;

    eprintln!("  [transport losses] {label}");
    eprintln!(
        "    histories {}   collisions {}   leaks: vacuum {} / infinity {}",
        r.histories, r.collisions, r.leak_vacuum, r.leak_infinity
    );
    eprintln!(
        "    lost_locate  {:>10}  ({:.4} % of histories)",
        r.lost_locate,
        pct(r.lost_locate)
    );
    eprintln!(
        "    stuck_events {:>10}  ({:.4} %)   mean path {:.3e} cm   last E {:.3e} eV",
        r.stuck_events,
        pct(r.stuck_events),
        r.stuck_path_cm,
        r.stuck_last_e
    );
    eprintln!(
        "    neg_dist     {:>10}  ({:.4} %)   worst {:.3e}   from lattice {}",
        r.neg_dist,
        pct(r.neg_dist),
        r.neg_worst,
        r.neg_from_lattice
    );

    let bad = r.lost_locate + r.stuck_events + r.neg_dist;
    if bad == 0 {
        eprintln!("    => CLEAN: no history was lost, trapped, or given a negative distance.");
    } else {
        eprintln!(
            "    => {bad} histories ({:.4} %) ended in a TRANSPORT DEFECT, not physics. \
             Any residual interpreted as nuclear data is contaminated by this.",
            pct(bad)
        );
    }
}

/// Report source-convergence evidence for a k-eigenvalue run.
///
/// **Why this is not optional.** A Monte Carlo eigenvalue run scores `k` while
/// the fission source is still relaxing toward its fundamental mode if too few
/// inactive generations were run. The result is a **bias, not a variance** —
/// it does not shrink when you pool more seeds, and the reported `sem` gives
/// no hint of it, because every seed is biased the same way. Pooling 32 seeds
/// of a badly converged case produces a tight, confident, wrong number.
///
/// Loosely coupled systems — large lattices, reflected assemblies, anything
/// with a dominance ratio near 1 — need far more inactive generations than a
/// compact bare sphere. Copying a settings block from one case to another is
/// therefore not safe, which is exactly how this goes unnoticed.
///
/// **The diagnostic.** Split the ACTIVE generations in half and compare the
/// two means. Under a converged source the halves differ only by statistics;
/// a systematic drift between them means the source was still moving while
/// scoring, and `n_inactive` is too low. Shannon entropy over the fission
/// source (`entropy`) is the companion check and is reported when present.
///
/// `drift_pcm` is returned so a caller can gate on it rather than eyeball it.
/// A drift comparable to, or larger than, the residual being interpreted means
/// that residual cannot be attributed to nuclear data at all.
pub fn report_source_convergence(
    label: &str,
    r: &crate::physics::keff::KeffResult,
    n_inactive: usize,
) -> f64 {
    let k = &r.k_by_generation;
    let n_in = n_inactive.min(k.len());
    let active = &k[n_in..];
    eprintln!("  [source convergence] {label}");
    if active.len() < 4 {
        eprintln!("    too few active generations ({}) to split", active.len());
        return f64::NAN;
    }
    let mean = |s: &[f64]| s.iter().sum::<f64>() / s.len() as f64;
    let h = active.len() / 2;
    let (a, b) = (mean(&active[..h]), mean(&active[h..]));
    let drift = (b - a) * 1.0e5;

    let q = n_in / 4;
    if q > 0 {
        let qs: Vec<String> = (0..4)
            .map(|i| format!("{:.4}", mean(&k[i * q..(i + 1) * q])))
            .collect();
        eprintln!("    inactive quarters: {}", qs.join("  "));
    }
    // The drift needs its own uncertainty or it is not a measurement. Estimate
    // it from the scatter of k ACROSS active generations. NOTE this is a LOWER
    // BOUND on the true uncertainty: successive generations share a fission
    // source and are positively correlated, so the independent-sample formula
    // understates the error bar. A drift that is not resolved against even
    // this optimistic bound is certainly not resolved.
    let n = active.len() as f64;
    let var = active.iter().map(|x| (x - mean(active)).powi(2)).sum::<f64>() / (n - 1.0);
    let drift_sem = (var * 2.0 / (n / 2.0)).sqrt() * 1.0e5;
    eprintln!(
        "    active halves:     {:.5} then {:.5}   drift {:+.0} +/- {:.0} pcm (>= this err)",
        a, b, drift, drift_sem
    );
    if !r.entropy.is_empty() {
        let e = &r.entropy;
        eprintln!(
            "    Shannon entropy:   first {:.4}  last {:.4}  (flat => converged)",
            e[0],
            e[e.len() - 1]
        );
    } else {
        eprintln!("    Shannon entropy:   NOT COMPUTED for this run");
    }
    if drift.abs() > 2.0 * drift_sem && drift.abs() > 100.0 {
        eprintln!(
            "    => DRIFT {:+.0} pcm across the active phase. The source was still \
             moving while scoring: raise n_inactive. Any residual smaller than this \
             is not interpretable.",
            drift
        );
    } else {
        eprintln!("    => no resolved drift; source looks converged.");
    }
    drift
}

/// Active-phase drift in pcm, computed quietly (no printing).
///
/// Splits the active generations in half and returns `(second - first)` in
/// pcm. See [`report_source_convergence`] for what this measures and why it
/// matters; this variant exists so the statistic can be **pooled across
/// seeds**, which is the only way to tell a real convergence bias from noise.
///
/// **Why pooling is the decisive test.** At one seed the drift carries an
/// uncertainty comparable to its own size, so a single value proves nothing.
/// A genuine convergence bias is *systematic* — the source relaxes the same
/// way every time — so it survives averaging over independent seeds. Statistical
/// scatter does not. Pooling `n` seeds shrinks the error by `sqrt(n)` while
/// leaving a true bias untouched.
///
/// Returns `NaN` when there are too few active generations to split.
pub fn source_convergence_drift_pcm(
    r: &crate::physics::keff::KeffResult,
    n_inactive: usize,
) -> f64 {
    let k = &r.k_by_generation;
    let n_in = n_inactive.min(k.len());
    let active = &k[n_in..];
    if active.len() < 4 {
        return f64::NAN;
    }
    let mean = |s: &[f64]| s.iter().sum::<f64>() / s.len() as f64;
    let h = active.len() / 2;
    (mean(&active[h..]) - mean(&active[..h])) * 1.0e5
}
