/// Tally definition — filter composition and accumulator.
///
/// C++ source: `src/tallies/tally.cpp`, `include/openmc/tallies/tally.h`.
///
/// A `Tally` accumulates scores over a user-defined subset of the phase space,
/// filtered by a conjunction of `Filter`s (cell, energy bin, material, mesh, …).
///
/// Score types: flux, total reaction rate, fission, absorption, current, etc.
/// Multiple scores can be accumulated per tally.
use super::filter::FilterKind;

/// Score type.  Maps to `openmc::TallyScore`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScoreType {
    Flux,
    Total,
    Fission,
    Absorption,
    NuFission,
    /// Fission energy-deposition rate (a.k.a. heating from fission).
    ///
    /// Maps to `openmc::SCORE_KAPPA_FISSION` (`src/tallies/tally_scoring.cpp:1480`).
    /// Scores the fission reaction rate multiplied by the recoverable energy per
    /// fission `Q` \[J\], so the accumulated bin is a fission **power** in J per
    /// source-particle-generation. See [`super::scoring::Q_FISSION_J`] for the
    /// constant and its provenance; it is used to normalize a k-eigenvalue tally
    /// to a target reactor thermal power (the `tally-power-normalization`
    /// notebook).
    KappaFission,
    ScatterN, // (n,xn) scatter
    Current,
    Events,

    // ── GitHub #262 ────────────────────────────────────────────────────────
    /// `SCORE_SCATTER` — the plain scattering rate `Σ_t − Σ_a`
    /// (`src/tallies/tally_scoring.cpp:615`).
    ///
    /// **Not** the same quantity as [`Self::ScatterN`], which is the elastic
    /// channel only. Both exist because a P0 transfer matrix is built on the
    /// first and an (n,xn) production rate on the second.
    Scatter,
    /// `SCORE_NU_SCATTER` — the scattering rate weighted by the number of
    /// neutrons each scatter emits, so (n,2n) counts twice.
    NuScatter,
    /// `SCORE_DELAYED_NU_FISSION` — delayed fission production ν̄_d·Σ_f.
    /// **Kinetics.**
    DelayedNuFission,
    /// `SCORE_PROMPT_NU_FISSION` — prompt fission production ν̄_p·Σ_f.
    /// **Kinetics.**
    PromptNuFission,
    /// `SCORE_INVERSE_VELOCITY` — the flux-weighted `1/v` \[s/cm\]
    /// (`src/tallies/tally_scoring.cpp:607`), whose ratio to the flux gives
    /// the neutron generation time. **Kinetics.**
    InverseVelocity,
    /// `SCORE_DECAY_RATE` — `Σ_k λ_k ν̄_d,k Σ_f` \[cm⁻¹ s⁻¹\]
    /// (`src/tallies/tally_scoring.cpp:773`), summed over precursor groups.
    /// **Kinetics.**
    DecayRate,
    /// `SCORE_FISS_Q_PROMPT` — fission energy deposition counting the prompt
    /// components only.
    FissionQPrompt,
    /// `SCORE_FISS_Q_RECOV` — fission energy deposition counting everything
    /// recoverable, including delayed betas and gammas.
    ///
    /// Distinct from [`Self::KappaFission`], which carries this crate's single
    /// `Q_FISSION_J` constant; see [`super::scoring::Q_FISSION_PROMPT_J`] and
    /// [`super::scoring::Q_FISSION_RECOVERABLE_J`] for the split and its
    /// provenance.
    FissionQRecoverable,
}

/// A single tally accumulator bin: running sum + sum-of-squares for statistics.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct TallyBin {
    pub sum: f64,
    pub sum_sq: f64,
    pub count: u64,
    /// Running mean, for the **numerically stable** variance below.
    ///
    /// Kept beside `sum`/`sum_sq` rather than replacing them: OpenMC's
    /// statepoint format stores `sum` and `sum_sq`, so they have to be written
    /// out verbatim (`njoy-outram-park-fork::hdf5::statepoint_write`).
    mean_w: f64,
    /// Welford's `M2`, the running sum of squared deviations from the mean.
    m2: f64,
}

impl TallyBin {
    pub fn score(&mut self, value: f64) {
        self.sum += value;
        self.sum_sq += value * value;
        self.count += 1;
        // Welford, alongside the raw moments. One divide and two multiplies
        // per score; the cost is irrelevant next to a transport step and it
        // buys a variance that does not cancel.
        let delta = value - self.mean_w;
        self.mean_w += delta / self.count as f64;
        self.m2 += delta * (value - self.mean_w);
    }

    /// The **sample variance of the scores**, computed stably.
    ///
    /// # Why this exists rather than `sum_sq/n - mean^2`
    ///
    /// The textbook form subtracts two nearly-equal numbers whenever the
    /// relative spread is small, which is exactly the regime a well-converged
    /// tally bin lives in. It loses most of its significant digits there and
    /// **can go negative**, and both of this workspace's consumers then did
    /// something silently wrong with the result:
    ///
    /// - `WeightWindows::update_magic` guarded it with `var.max(0.0)`, turning
    ///   a cancelled variance into `rel_err = 0` -- so a cell whose variance
    ///   had lost all precision was treated as **perfectly resolved** and was
    ///   granted a weight window.
    /// - `rel_std_dev` took `variance.sqrt()` unguarded, so the same cell
    ///   produced **NaN** and was silently dropped from any figure of merit.
    ///
    /// This is the `bedok` lesson in a different guise (commit `98e7c0586`):
    /// there, a one-ulp `sinh` difference amplified by a catastrophically
    /// cancelling closed form made the solver take a different number of
    /// iterations and land somewhere else. A cancelling form feeding a
    /// **branch** does not round the answer, it changes which branch is taken.
    ///
    /// Welford's recurrence never forms the difference of two large numbers,
    /// so it stays accurate to full precision and cannot return a negative
    /// variance.
    pub fn variance(&self) -> f64 {
        if self.count < 2 {
            return 0.0;
        }
        self.m2 / (self.count - 1) as f64
    }

    /// Whether the textbook `sum_sq/n - mean^2` form has **catastrophically
    /// cancelled** for this bin -- it returns a non-positive variance where the
    /// stable form finds a positive one.
    ///
    /// Diagnostic only. It exists so the effect can be counted on a real
    /// problem instead of argued about.
    pub fn naive_variance_cancelled(&self) -> bool {
        if self.count < 2 || self.sum <= 0.0 {
            return false;
        }
        let n = self.count as f64;
        let mean = self.sum / n;
        let naive = (self.sum_sq / n - mean * mean) / (n - 1.0);
        naive <= 0.0 && self.variance() > 0.0
    }

    /// Mean over `n_realizations` active batches.
    pub fn mean(&self, n_realizations: u64) -> f64 {
        if n_realizations == 0 {
            return 0.0;
        }
        self.sum / n_realizations as f64
    }

    /// Relative standard deviation (as fraction of mean).
    pub fn rel_std_dev(&self, n_realizations: u64) -> f64 {
        if n_realizations < 2 {
            return f64::INFINITY;
        }
        let n = n_realizations as f64;
        let mean = self.sum / n;
        if mean == 0.0 {
            return f64::INFINITY;
        }
        // **This returns the relative standard error OF THE MEAN**, i.e.
        // `sqrt(s^2 / n) / mean`, which is what every caller wants and what the
        // `sum_sq` path below computes: `(sum_sq/n - mean^2)/(n-1)` is
        // algebraically `s^2/n` exactly, not `s^2`.
        //
        // **FIXED 2026-09-24 — this branch was returning `sqrt(s^2)/mean`, i.e.
        // exactly `sqrt(n)` TOO LARGE.** `Self::variance` is the sample variance
        // of the realization values; dividing by `n` once more is what turns it
        // into the variance of their mean, and that step was missing when the
        // Welford path was added. The two branches therefore computed different
        // quantities, and the branch taken in the ORDINARY case (one score per
        // bin per batch, so `count == n_realizations`) was the wrong one.
        //
        // The error scales with the realization count, which is how it was
        // found: a shielding tally at 270 realizations resolved 2 mesh cells
        // where the same run at 10 realizations resolved 384, because `R` was
        // inflated by `sqrt(270) = 16.4` against `sqrt(10) = 3.2`. More
        // statistics made the reported error worse, which no correct estimator
        // does.
        let variance = if self.count == n_realizations {
            self.variance() / n
        } else {
            (self.sum_sq / n - mean * mean) / (n - 1.0)
        };
        variance.max(0.0).sqrt() / mean.abs()
    }
}

#[cfg(test)]
mod bin_variance_tests {
    use super::*;

    /// **The textbook variance cancels, and the stable one does not.**
    ///
    /// Scores a bin with values whose spread is tiny next to their magnitude --
    /// the regime a converged tally bin sits in, and the regime that breaks
    /// `sum_sq/n - mean^2`.
    ///
    /// # Result
    ///
    /// At a mean of 1e8 with a spread of 1e-3, the naive form returns a
    /// NEGATIVE variance while Welford returns the right one to ~1e-12
    /// relative. Negative is the visible failure; the dangerous one is the
    /// near-miss just above zero, which produces a plausible-looking but wrong
    /// relative error and no warning at all.
    #[test]
    fn the_textbook_variance_cancels_where_welford_does_not() {
        let mean = 1.0e8_f64;
        let spread = 1.0e-3_f64;
        let values: Vec<f64> = (0..20)
            .map(|k| mean + spread * ((k as f64) - 9.5) / 9.5)
            .collect();

        let mut bin = TallyBin::default();
        for v in &values {
            bin.score(*v);
        }

        // Reference: two-pass, which is accurate because it never forms the
        // difference of two large numbers.
        let m = values.iter().sum::<f64>() / values.len() as f64;
        let reference = values.iter().map(|v| (v - m) * (v - m)).sum::<f64>()
            / (values.len() - 1) as f64;

        let n = bin.count as f64;
        let naive_mean = bin.sum / n;
        let naive = (bin.sum_sq / n - naive_mean * naive_mean) / (n - 1.0);

        println!(
            "reference {reference:.6e}  welford {:.6e}  naive {naive:.6e}",
            bin.variance()
        );
        // **1e-4, not 1e-9, and the reason matters.** This case has a
        // condition number of order (mean/spread)^2 ~ 1e22 for the variance,
        // so NO finite-precision method keeps nine digits here -- Welford is
        // stable, not exact. Measured: it lands 2.7e-5 relative from the
        // two-pass reference while the naive form returns a NEGATIVE variance
        // six orders of magnitude out. A 1e-9 gate would fail a correct
        // implementation, which is a gate sized off hope rather than off what
        // the arithmetic can deliver.
        assert!(
            (bin.variance() - reference).abs() <= 1.0e-4 * reference,
            "Welford variance {} is not the reference {reference} (rel {:.2e})",
            bin.variance(),
            ((bin.variance() - reference) / reference).abs()
        );
        assert!(
            naive <= 0.0 || (naive - reference).abs() > 0.5 * reference,
            "this test is pointless unless the naive form actually fails here; \
             it returned {naive} against a true {reference}"
        );
        assert!(
            bin.naive_variance_cancelled(),
            "the cancellation detector should fire on exactly this case"
        );
    }

    /// A well-spread bin must be unaffected -- the stable variance must not move
    /// ordinary results.
    ///
    /// **This test asserted the WRONG VALUE from the day it was written, and its
    /// own name is what makes that worth recording.** It claimed
    /// `rel_std_dev == sqrt(2.5)/3`, which is the sample standard deviation over
    /// the mean; the relative standard error OF THE MEAN is
    /// `sqrt(2.5/5)/3` -- smaller by `sqrt(5) = 2.236`. So the one test written
    /// to confirm that adding Welford "did not move ordinary results" instead
    /// **pinned a 2.236x regression in place**, and the two branches of
    /// `rel_std_dev` silently computed different quantities for four days.
    ///
    /// Both branches are now asserted to agree, which is the check that would
    /// have caught it: they are two formulas for one quantity, so the test that
    /// matters is that they return the same number, not that either returns a
    /// number someone wrote down.
    #[test]
    fn a_well_conditioned_bin_is_unchanged() {
        let xs = [1.0_f64, 2.0, 3.0, 4.0, 5.0];
        let mut bin = TallyBin::default();
        for v in xs {
            bin.score(v);
        }
        // Sample variance of the VALUES: sum (x - 3)^2 / 4 = 10/4 = 2.5.
        assert!((bin.variance() - 2.5).abs() < 1.0e-12, "variance {}", bin.variance());
        assert!(!bin.naive_variance_cancelled());

        // Relative standard error of the MEAN: sqrt(s^2 / n) / mean.
        let n = xs.len() as f64;
        let want = (2.5_f64 / n).sqrt() / 3.0;
        let rel = bin.rel_std_dev(5);
        assert!((rel - want).abs() < 1.0e-12, "rel {rel}, want {want}");

        // **The two branches must agree.** `rel_std_dev` takes the Welford path
        // when `count == n_realizations` and the `sum_sq` path otherwise; they
        // are two formulas for one quantity, and asserting they match is what
        // catches a missing `/ n` in either.
        let mean = bin.sum / n;
        let via_sum_sq = ((bin.sum_sq / n - mean * mean) / (n - 1.0)).sqrt() / mean;
        assert!(
            (rel - via_sum_sq).abs() < 1.0e-12,
            "the Welford path gives {rel} and the sum_sq path {via_sum_sq}; they \
             must be the same quantity"
        );
    }
}

/// A tally.  Maps to `openmc::Tally`.
#[derive(Debug, Clone, PartialEq)]
pub struct Tally {
    pub id: i32,
    pub name: String,
    /// The filters this tally is conditioned on, as a closed enum rather than
    /// trait objects — see [`super::filter::FilterKind`] for why (workspace
    /// design rules: traits for the contract, enums for dispatch).
    pub filters: Vec<FilterKind>,
    pub scores: Vec<ScoreType>,
    /// Accumulated bins, indexed `[filter_bin * n_scores + score_idx]`.
    pub bins: Vec<TallyBin>,
}

impl Tally {
    /// Total number of bins = product of each filter's bin count × number of scores.
    pub fn n_bins(&self) -> usize {
        self.filters.iter().map(|f| f.n_bins()).product::<usize>() * self.scores.len()
    }
}
