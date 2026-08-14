// Ported from NJOY2016 `src/leapr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `LEAPR` — generate the thermal scattering law S(alpha, beta) (ENDF MF=7).
//!
//! LEAPR is the *generator* upstream of THERMR: it builds S(alpha, beta) for a
//! bound moderator from a phonon/vibrational model (incoherent-Gaussian
//! approximation), where THERMR only *reads* an existing MF=7 evaluation. The
//! model combines a solid-type **phonon expansion** from a frequency spectrum
//! rho(E), an optional **translational** (free-gas or diffusion) term, and
//! optional **discrete oscillators**.
//!
//! ## Module map (what belongs where)
//!
//! - [`input`] — the typed card deck ([`LeaprInput`] and its enums).
//! - [`deck`] — the **text** card-deck reader: `.leapr` job text -> [`LeaprDeck`]
//!   -> one [`LeaprInput`] per temperature. This is what lets a 12 KB ENDF/B
//!   `tsl-*.leapr` deck stand in for the multi-MB MF=7 tape it generated.
//! - [`decks`] — the **registry** of decks this crate knows about, and where
//!   their text is found (embedded / local directory / cache).
//! - [`generate`] — the **consumer surface**: ask for a material at a
//!   temperature, get an MF=7 law. Regeneration is the default source; a tape is
//!   used only when a caller points at one. Composes the kernels below into
//!   [`endout`] and caches the result.
//! - [`vintage`] — the **physical-constant set**, selected by the evaluation's
//!   own `EVAL-<MON><YY>` field. Reproducing a pre-2018 evaluation needs that
//!   era's Boltzmann constant, not CODATA2018 (a ~100x difference).
//! - [`frequency`] — `start`/`fsum`: phonon-spectrum integrals, Debye-Waller
//!   lambda, effective temperature, and the first phonon term `T_1(beta)`.
//! - [`continuous`] — `contin`/`terpt`/`convol`: the phonon-expansion sum
//!   `S = e^{-alpha lambda} sum_n (alpha lambda)^n/n! T_n(beta)`, plus the
//!   short-collision-time tail fill and moment checks.
//! - [`translation`] — `trans`/`stable`/`terps`/`sbfill`/`besk1`: the
//!   translational (free-gas / diffusion) term.
//! - [`discrete`] — `discre`/`bfact`/`bfill`/`exts`/`sint` and the modified
//!   Bessel `I0`/`I1`: discrete-oscillator convolution.
//! - [`coldh`] — the Young-Koppel cold-hydrogen/deuterium orchestrator
//!   ([`coldh::add_cold_hydrogen`]) plus its helpers (`bt`, `sumh`, `cn`,
//!   `sjbes`, `terpk`). Self-consistency tested, not reference-validated.
//! - [`coher`] — the coherent-elastic (Bragg) reciprocal-lattice sum
//!   ([`coher::coher`], `formf`/`tausq`/`taufcc`/`taubcc`).
//! - [`endout`] — the ENDF-6 MF=7 tape writer ([`endout::endout`]) that a THERMR
//!   MF=7 read-back round-trips.
//! - [`sct`] — the free-gas / short-collision-time Gaussian used by several
//!   kernels.
//!
//! ## Ported vs. not ported
//!
//! The physics kernels above are ported and unit-tested. Now also ported:
//! - `endout` (leapr.f90:2972-3623): the MF=7 tape writer — see [`endout`]. Its
//!   output round-trips back through [`crate::thermr::mf7::parse_mf7`]. It writes
//!   the secondary-scatterer `B(7)..B(12)` constants ([`SecondaryScatterer`]);
//!   MF=1/451 descriptive records and the `b7 = 0` mixed-moderator merge are
//!   intentionally omitted (see [`endout`], and the entry below).
//! - `coher`/`formf`/`tausq`/`taufcc`/`taubcc` (2489-2814, 2924-2970): the
//!   coherent-elastic (Bragg) calculation — see [`coher`].
//! - the `coldh` convolution orchestrator (1936-2183) — see
//!   [`coldh::add_cold_hydrogen`].
//!
//! **Still not ported** (return [`crate::NjoyError::NotPorted`] or are absent):
//! - `copys` (2468-2487): the scratch-tape plumbing for the mixed-moderator
//!   merge (not needed for the single-scatterer in-memory path).
//! - `skold` (2816-2922): the Sköld pair-correlation correction.
//! - the NJOY `run` driver — [`run`] still returns `NotPorted`, but only its
//!   Fortran unit/file plumbing is now missing. Card reading is ported
//!   ([`deck::LeaprDeck::parse`]) and so is the **orchestration**:
//!   [`generate::generate_tape`] composes the kernels, performs the `dwpix` and
//!   `tempf` conversions [`endout`] expects (`leapr.f90:717, 3035`), and emits
//!   the MF=7 tape for one temperature. Use that rather than [`run`].
//!   Since 2026-08-14 it runs all three law-building stages in the Fortran's
//!   order — `contin`, then `trans` when `twt > 0`, then `discre` when the deck
//!   declares oscillators (`leapr.f90:376-384`) — so molecular moderators are
//!   built correctly, not just solid-type ones. Incoherent-elastic (`iel < 0`)
//!   output is refused rather than approximated.
//! - the **mixed-moderator `S(alpha, beta)` merge** for a short-collision-time
//!   secondary scatterer (`b7 = 0`; 3018-3030). A secondary scatterer of the
//!   *analytic* kinds — `b7 = 1` free gas, `b7 = 2` diffusion — **is** supported:
//!   it is carried entirely by the `B(7)..B(12)` constants [`endout`] writes,
//!   which is what light water needs. No registered deck uses `b7 = 0`, and
//!   [`generate::generate_tape`] refuses it rather than emit a law whose
//!   secondary is silently missing.
//!
//! ## Validation status
//!
//! The **incoherent-inelastic phonon-expansion path is validated against
//! ENDF/B-VIII.0** as of 2026-08-13: regenerating S(alpha, beta) for
//! crystalline graphite from its distributed 12 KB LEAPR deck reproduces the
//! official MF=7/MT=4 tape to a maximum relative deviation of 4.917e-6 and an
//! RMS of 6.390e-7 over 48,941 grid points at 296 K (4.838e-6 / 5.817e-7 at
//! 1000 K), which is the tape's own 6-to-7-significant-figure storage
//! precision. Methodology, the full result table and the Boltzmann-constant
//! caveat are in `tests/leapr_graphite_deck_parity.rs`.
//!
//! End-to-end through [`generate`] the agreement is **exact for both channels**
//! at 296 K: MF=7/MT=4 matches on **60,000 of 60,000** stored values and
//! MF=7/MT=2 on **221 of 221** Bragg grid points (max relative deviation
//! 0.000e0 on edge energies and `S(E)`), because [`endout`] applies the same
//! `sigfig` rounding NJOY does. Across all ten tabulated temperatures the raw
//! MT=2 kernel output agrees to **1.001e-13**
//! (`tests/leapr_graphite_coherent_elastic_parity.rs`).
//!
//! Both rest on taking the physical constants from the deck's declared vintage,
//! and the two channels need *different* constants from it — see [`vintage`].
//! `docs/leapr-deck-provenance.md` holds the full V&V record and the licence
//! finding; [`generate`] states what is **not** validated (the 10P/30P porous
//! reactor grades, which have never been measured).
//!
//! **The translational and discrete-oscillator stages now have a reference
//! check too, at a weaker standard.** The graphite case exercises only `contin`
//! with `twt = c = 0` and `nd = 0`; light water exercises all three stages, and
//! regenerating H-in-H2O at 293.6 K agrees with the published ENDF/B-VIII.0
//! evaluation to **~0.6 % on sigma_inel across 0.0253-8 eV** and **+0.09 % on
//! `T_eff`** (1195.35 K against 1194.3 K) — see
//! `tests/leapr_h2o_secondary_scatterer.rs`. That is an *agreement band*, not
//! the bit-parity graphite reaches, and the reference values are this
//! repository's own recorded measurements rather than a live tape diff, so
//! light water is still reported unvalidated by
//! [`generate::SabRequest::validation`].
//!
//! That check is what exposed the stages being skipped entirely by
//! [`generate::generate_tape`] before 2026-08-14 (`T_eff` 482 K against 1194 K,
//! sigma_inel +48 % at 8 eV) — a defect graphite could not see.
//!
//! **Everything else remains an untrusted AI draft** — the diffusive,
//! cold-hydrogen and elastic-output paths still have only closed-form and
//! self-consistency checks. See `README.md`.
//!
//! **Upstream:** `leapr.f90` (~3.6k lines). **Manual:** LA-UR-17-20093 §LEAPR.

use crate::NjoyError;

pub mod coher;
pub mod coldh;
pub mod continuous;
pub mod deck;
pub mod decks;
pub mod discrete;
pub mod endout;
pub mod frequency;
pub mod generate;
pub mod input;
pub mod sct;
pub mod translation;
pub mod vintage;

pub use coher::{coher, BraggEdges, CoherentLattice};
pub use deck::{CardCursor, LeaprDeck, LeaprTemperature, PairCorrelation};
pub use coldh::add_cold_hydrogen;
pub use endout::{endout, ElasticOutput, LeaprOutput, SecondaryScatterer};
pub use frequency::FrequencyModel;
pub use vintage::{EvaluationDate, PhysicalConstants};
pub use input::{
    ColdOption, ContinuousDist, DiscreteOscillator, ElasticOption, LeaprInput,
    SecondaryScattererKind, TranslationKind,
};

/// The thermal energy 0.0253 eV used as the `lat` scaling reference (eV).
pub const THERM_EV: f64 = 0.0253;

/// Single-temperature asymmetric scattering-law table `S_s(alpha, -beta)`.
///
/// Stores the LEAPR working array with **beta as the fastest index** (matching
/// NJOY's `ssm(nbeta, nalpha)` layout). Entries are the dimensionless asymmetric
/// scattering law evaluated on the negative-beta side; the physical S at positive
/// beta is recovered with the usual `exp(-beta)` / `exp(-beta/2)` detailed-balance
/// factors.
#[derive(Debug, Clone, PartialEq)]
pub struct SabMatrix {
    /// Number of alpha grid points.
    pub nalpha: usize,
    /// Number of beta grid points.
    pub nbeta: usize,
    data: Vec<f64>,
}

impl SabMatrix {
    /// Allocate a zeroed `nbeta x nalpha` table.
    pub fn zeros(nbeta: usize, nalpha: usize) -> Self {
        Self {
            nalpha,
            nbeta,
            data: vec![0.0; nbeta * nalpha],
        }
    }

    /// Read `S(alpha_ialpha, -beta_ibeta)` (dimensionless).
    #[inline]
    pub fn get(&self, ibeta: usize, ialpha: usize) -> f64 {
        self.data[ialpha * self.nbeta + ibeta]
    }

    /// Write `S(alpha_ialpha, -beta_ibeta)` (dimensionless).
    #[inline]
    pub fn set(&mut self, ibeta: usize, ialpha: usize, v: f64) {
        self.data[ialpha * self.nbeta + ibeta] = v;
    }
}

/// Run the LEAPR driver (NJOY module entry point).
///
/// **Status: still `NotPorted`, but for a narrower reason than before.** The
/// physics kernels are ported (see the module map above), the MF=7 tape writer
/// is ported ([`endout::endout`]), and the free-format card deck can now be read
/// with [`deck::LeaprDeck::parse`]. What this function would still need is
/// NJOY's Fortran unit plumbing (`nsysi`/`nout`, `openz`/`closz`) plus the
/// orchestration that composes [`frequency::FrequencyModel::start`],
/// [`continuous::phonon_expansion`], [`coher::coher`] and
/// [`endout::endout`] into a [`endout::LeaprOutput`] — including the `dwpix`
/// and `tempf` conversions `endout` expects (`leapr.f90:717, 3035`), which no
/// code path performs yet.
///
/// Until then, drive it explicitly: [`deck::LeaprDeck::parse`] the deck,
/// [`deck::LeaprDeck::input_at`] a temperature, then call the kernels.
pub fn run() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted(
        "leapr unit plumbing + kernel orchestration (cards, kernels and endout are ported — \
         read a deck with LeaprDeck::parse and drive the module API)",
    ))
}
