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
#[derive(Debug, Default, Clone)]
pub struct TallyBin {
    pub sum: f64,
    pub sum_sq: f64,
    pub count: u64,
}

impl TallyBin {
    pub fn score(&mut self, value: f64) {
        self.sum += value;
        self.sum_sq += value * value;
        self.count += 1;
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
        let variance = (self.sum_sq / n - mean * mean) / (n - 1.0);
        variance.sqrt() / mean.abs()
    }
}

/// A tally.  Maps to `openmc::Tally`.
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
