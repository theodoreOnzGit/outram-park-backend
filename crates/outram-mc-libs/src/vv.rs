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
    assert_reproduces_recorded, assert_table_relative, WorstDeviation,
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
