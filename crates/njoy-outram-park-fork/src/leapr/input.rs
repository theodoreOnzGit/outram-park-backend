// Ported from NJOY2016 `src/leapr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! LEAPR input model — a typed form of the free-format card deck.
//!
//! NJOY reads the LEAPR job as a sequence of free-format cards (leapr.f90:122-372).
//! This module replaces the `read(nsysi,*)` sequence with plain Rust structs and
//! enums so a caller builds a job by value instead of writing a text deck. Only
//! the fields the ported physics kernels actually consume are modelled; card-1
//! output-unit / print-control plumbing and the ENDF-writer options live with the
//! (not-yet-ported) `endout` writer.
//!
//! ## What lives here
//! - [`LeaprInput`] — one scatterer at one temperature: the alpha/beta grids, the
//!   continuous frequency distribution, and any discrete oscillators.
//! - [`ContinuousDist`] — card 11-13: the phonon frequency spectrum rho(E) plus
//!   the translational weight, diffusion constant, and continuum normalization.
//! - [`DiscreteOscillator`] — cards 15-16: one molecular vibrational mode.
//! - [`TranslationKind`], [`ColdOption`], [`ElasticOption`] — the small closed
//!   option sets, as enums (per the workspace no-`dyn` rule).
//!
//! ## Units
//! - `alpha`, `beta` — dimensionless ENDF thermal variables.
//! - `energy_ev`, `delta_ev` — eV.
//! - `temperature_k` — K.
//! - `twt`, `tbeta`, `weight` — dimensionless mode weights (sum to 1).
//! - `c` — dimensionless diffusion constant (0 => free gas).

use crate::leapr::vintage::PhysicalConstants;

/// Coherent-elastic (Bragg) lattice option (card 5 `iel`), selecting which
/// lattice the reciprocal-lattice sum in [`crate::leapr::coher`] (the ported
/// `coher`/`formf`/`tausq` path of leapr.f90) runs over. Downstream, the
/// resulting Bragg-edge `S(E)` is *evaluated* by [`crate::thermr::coherent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElasticOption {
    /// Incoherent elastic instead of a Bragg lattice (`iel < 0`).
    ///
    /// Undocumented in NJOY's card comments, which list only 0..=6, but real
    /// decks use it — `tsl-YinYH2.leapr` and `tsl-HinYH2.leapr` of
    /// ENDF/B-VIII.0 both pass `iel = -1` — and `endout` acts on it
    /// (`leapr.f90:3053`, `if (iel.lt.0)`), writing an MF=7/MT=2 `LTHR=2`
    /// Debye-Waller TAB1 rather than Bragg edges. NJOY also *derives* it, with
    /// `iel = -1` set internally when `iel == 0` and `twt == 0`
    /// (`leapr.f90:3052`).
    Incoherent,
    /// No coherent elastic (default).
    None,
    /// Graphite.
    Graphite,
    /// Beryllium.
    Beryllium,
    /// Beryllium oxide.
    BerylliumOxide,
    /// Aluminium.
    Aluminium,
    /// Lead.
    Lead,
    /// Iron.
    Iron,
}

impl ElasticOption {
    /// Map the card-5 `iel` integer code onto the option, or `None` if the code
    /// is outside the set NJOY accepts (any negative value, or 0..=6).
    pub fn from_code(iel: i32) -> Option<Self> {
        match iel {
            i if i < 0 => Some(Self::Incoherent),
            0 => Some(Self::None),
            1 => Some(Self::Graphite),
            2 => Some(Self::Beryllium),
            3 => Some(Self::BerylliumOxide),
            4 => Some(Self::Aluminium),
            5 => Some(Self::Lead),
            6 => Some(Self::Iron),
            _ => None,
        }
    }

    /// The Bragg lattice this option selects for
    /// [`crate::leapr::coher::coher`], or `None` when the option asks for no
    /// coherent-elastic calculation.
    ///
    /// [`Incoherent`](Self::Incoherent) and [`None`](Self::None) both return
    /// `None` here, for different reasons: the first wants an MF=7/MT=2
    /// `LTHR=2` Debye-Waller section instead of Bragg edges, the second wants no
    /// MT=2 at all. A caller that needs to tell them apart should match on the
    /// option itself.
    pub const fn coherent_lattice(self) -> Option<crate::leapr::coher::CoherentLattice> {
        use crate::leapr::coher::CoherentLattice as L;
        match self {
            Self::Incoherent | Self::None => Option::None,
            Self::Graphite => Some(L::Graphite),
            Self::Beryllium => Some(L::Beryllium),
            Self::BerylliumOxide => Some(L::BerylliumOxide),
            Self::Aluminium => Some(L::Aluminium),
            Self::Lead => Some(L::Lead),
            Self::Iron => Some(L::Iron),
        }
    }

    /// The card-5 `iel` integer code for this option (`-1` for
    /// [`Incoherent`](Self::Incoherent), which is how NJOY spells it
    /// internally).
    pub fn code(self) -> i32 {
        match self {
            Self::Incoherent => -1,
            Self::None => 0,
            Self::Graphite => 1,
            Self::Beryllium => 2,
            Self::BerylliumOxide => 3,
            Self::Aluminium => 4,
            Self::Lead => 5,
            Self::Iron => 6,
        }
    }
}

/// Cold-moderator option (card 5 `ncold`) selecting a Young-Koppel rotational
/// treatment. The full `coldh` convolution is **not** ported (its helpers are —
/// see [`crate::leapr::coldh`]); this enum records the requested law.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColdOption {
    /// No cold treatment (default).
    None,
    /// Ortho hydrogen (`ncold = 1`, `law = 2`).
    OrthoHydrogen,
    /// Para hydrogen (`ncold = 2`, `law = 3`).
    ParaHydrogen,
    /// Ortho deuterium (`ncold = 3`, `law = 4`).
    OrthoDeuterium,
    /// Para deuterium (`ncold = 4`, `law = 5`).
    ParaDeuterium,
}

impl ColdOption {
    /// Map the card-5 `ncold` integer code onto the option, or `None` if the
    /// code is outside the 0..=4 set NJOY accepts.
    pub fn from_code(ncold: i32) -> Option<Self> {
        match ncold {
            0 => Some(Self::None),
            1 => Some(Self::OrthoHydrogen),
            2 => Some(Self::ParaHydrogen),
            3 => Some(Self::OrthoDeuterium),
            4 => Some(Self::ParaDeuterium),
            _ => None,
        }
    }

    /// The card-5 `ncold` integer code for this option.
    pub fn code(self) -> i32 {
        match self {
            Self::None => 0,
            Self::OrthoHydrogen => 1,
            Self::ParaHydrogen => 2,
            Self::OrthoDeuterium => 3,
            Self::ParaDeuterium => 4,
        }
    }
}

/// How a **secondary** scatterer is treated (card 6 `b7`).
///
/// A `tsl` evaluation for a compound moderator may describe a second atomic
/// species alongside the principal one — the oxygen in `tsl-HinH2O`, where H is
/// principal and O is secondary. `b7` says how that second species is handled,
/// and the choice matters because it decides whether the secondary contributes
/// to the tabulated `S(alpha, beta)` at all:
///
/// - [`ShortCollisionTime`](Self::ShortCollisionTime) (`b7 = 0`) is the only
///   kind whose scattering is **merged into the tabulated law**. NJOY runs its
///   whole temperature loop a second time for the secondary scatterer and adds
///   the result in (`leapr.f90:398`, `3018-3030`).
/// - [`FreeGas`](Self::FreeGas) (`b7 = 1`) and
///   [`Diffusion`](Self::Diffusion) (`b7 = 2`) are described **analytically**
///   by the `B(7)..B(12)` constants of MF=7/MT=4 instead. The tabulated
///   `S(alpha, beta)` stays purely the principal scatterer's, and a downstream
///   code reconstructs the secondary's contribution from those constants. This
///   is why light water's `S(alpha, beta)` is an H-only law.
///
/// The `b7` field is a *real* in the Fortran deck, so the codes are compared as
/// floats; see [`from_code`](Self::from_code).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondaryScattererKind {
    /// `b7 = 0` — short-collision-time only. The secondary scatterer's law is
    /// computed on its own and **merged** into the principal `S(alpha, beta)`.
    ShortCollisionTime,
    /// `b7 = 1` — free gas, carried analytically in `B(7)..B(12)`.
    FreeGas,
    /// `b7 = 2` — diffusion, carried analytically in `B(7)..B(12)`.
    Diffusion,
}

impl SecondaryScattererKind {
    /// Map the card-6 `b7` value onto the kind, or `None` if it is outside the
    /// `0..=2` set NJOY accepts.
    ///
    /// `b7` is read as a real, so this rounds to the nearest integer and rejects
    /// anything more than `1e-6` away from it rather than silently truncating a
    /// malformed `1.5`.
    pub fn from_code(b7: f64) -> Option<Self> {
        if !b7.is_finite() || (b7 - b7.round()).abs() > 1e-6 {
            return None;
        }
        match b7.round() as i32 {
            0 => Some(Self::ShortCollisionTime),
            1 => Some(Self::FreeGas),
            2 => Some(Self::Diffusion),
            _ => None,
        }
    }

    /// The card-6 `b7` value for this kind, as written into `B(7)` of MF=7/MT=4.
    pub fn code(self) -> f64 {
        match self {
            Self::ShortCollisionTime => 0.0,
            Self::FreeGas => 1.0,
            Self::Diffusion => 2.0,
        }
    }

    /// Whether this kind's scattering is merged into the tabulated
    /// `S(alpha, beta)` rather than carried analytically in `B(7)..B(12)`.
    ///
    /// True only for [`ShortCollisionTime`](Self::ShortCollisionTime) — the
    /// `b7 <= 0` branch of `leapr.f90:3018`.
    pub fn merges_into_sab(self) -> bool {
        matches!(self, Self::ShortCollisionTime)
    }
}

/// Kind of translational term convolved into the continuous law (card 6 `b7`
/// combined with card 13 `c`). Chosen from a closed set, so an enum.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TranslationKind {
    /// No translational term.
    None,
    /// Free-gas translation (`c == 0`, `twt > 0`).
    FreeGas,
    /// Diffusion translation (`c > 0`, `twt > 0`); `c` is the diffusion constant.
    Diffusion,
}

/// One discrete oscillator (a molecular vibrational mode): cards 15 (energy) and
/// 16 (weight) of leapr.f90.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscreteOscillator {
    /// Oscillator energy `bdel` \[eV\].
    pub energy_ev: f64,
    /// Oscillator weight `adel` (dimensionless); the discrete weights plus `twt`
    /// plus `tbeta` sum to 1.
    pub weight: f64,
}

/// Continuous frequency distribution and translational parameters
/// (cards 11-13 of leapr.f90).
#[derive(Debug, Clone, PartialEq)]
pub struct ContinuousDist {
    /// Energy-grid spacing `delta` of the tabulated spectrum \[eV\].
    pub delta_ev: f64,
    /// Phonon frequency spectrum `rho(E)` sampled on `0, delta, 2 delta, ...`
    /// (dimensionless; `rho[0]` at E = 0). This is the raw `p1` card-12 array.
    pub rho: Vec<f64>,
    /// Translational weight `twt` (dimensionless); `> 0` enables a translational
    /// term.
    pub twt: f64,
    /// Diffusion constant `c` (dimensionless). Zero selects free-gas translation.
    pub c: f64,
    /// Continuum normalization `tbeta` (dimensionless).
    pub tbeta: f64,
}

impl ContinuousDist {
    /// Which translational kind these parameters select.
    pub fn translation_kind(&self) -> TranslationKind {
        if self.twt <= 0.0 {
            TranslationKind::None
        } else if self.c == 0.0 {
            TranslationKind::FreeGas
        } else {
            TranslationKind::Diffusion
        }
    }
}

/// A single LEAPR job: one scatterer, one temperature.
///
/// This is the minimal set of card-deck values consumed by the ported physics
/// kernels ([`crate::leapr::frequency`], [`crate::leapr::continuous`],
/// [`crate::leapr::translation`], [`crate::leapr::discrete`]).
#[derive(Debug, Clone, PartialEq)]
pub struct LeaprInput {
    /// Alpha grid (dimensionless), increasing order (card 8).
    pub alpha: Vec<f64>,
    /// Beta grid (dimensionless), increasing order (card 9).
    pub beta: Vec<f64>,
    /// If true, alpha/beta are scaled by `0.0253 / tev` (`lat == 1`, card 7).
    pub lat: bool,
    /// Secondary-scatterer alpha ratio `arat = aws/awr` (`1.0` for the principal
    /// scatterer). Input alpha values are divided by this.
    pub arat: f64,
    /// Phonon-expansion order `nphon` (card 3).
    pub nphon: usize,
    /// Temperature \[K\] (card 10).
    pub temperature_k: f64,
    /// Continuous frequency distribution + translational parameters.
    pub continuous: ContinuousDist,
    /// Discrete oscillators (may be empty).
    pub oscillators: Vec<DiscreteOscillator>,
    /// The physical-constant set this job is run with — i.e. the value of `k_B`
    /// that defines `tev = k_B T`, and through it the beta-grid spacing and the
    /// `LAT = 1` scale factor.
    ///
    /// **Defaults to [`PhysicalConstants::Codata2018`]**, the crate constant, so
    /// a job constructed by hand behaves exactly as it did before this field
    /// existed. Set it to the evaluation's vintage when *reproducing* published
    /// data: [`crate::leapr::deck::LeaprDeck::input_at`] does this for you from
    /// the deck's own `EVAL-<MON><YY>` comment card. Getting it wrong is a
    /// ~100x parity error, not a rounding difference — see
    /// [`crate::leapr::vintage`].
    pub constants: PhysicalConstants,
}

impl LeaprInput {
    /// Thermal energy `tev = k_B * T` \[eV\] for this job.
    ///
    /// `k_B` comes from [`LeaprInput::constants`], **not** from
    /// [`crate::common::phys::BK_EV_PER_K`] directly, so a job reproducing an
    /// older evaluation uses that evaluation's constant.
    pub fn tev(&self) -> f64 {
        self.constants.bk_ev_per_k() * self.temperature_k.abs()
    }

    /// Alpha/beta scale factor `sc` (`0.0253/tev` when `lat`, else `1`).
    pub fn scale(&self) -> f64 {
        if self.lat {
            0.0253 / self.tev()
        } else {
            1.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_kind_dispatch() {
        let mut c = ContinuousDist {
            delta_ev: 1e-3,
            rho: vec![0.0, 1.0, 2.0],
            twt: 0.0,
            c: 0.0,
            tbeta: 1.0,
        };
        assert_eq!(c.translation_kind(), TranslationKind::None);
        c.twt = 0.1;
        assert_eq!(c.translation_kind(), TranslationKind::FreeGas);
        c.c = 2.0;
        assert_eq!(c.translation_kind(), TranslationKind::Diffusion);
    }

    #[test]
    fn scale_is_unity_without_lat() {
        let inp = LeaprInput {
            alpha: vec![1.0],
            beta: vec![0.0],
            lat: false,
            arat: 1.0,
            nphon: 100,
            temperature_k: 296.0,
            continuous: ContinuousDist {
                delta_ev: 1e-3,
                rho: vec![0.0, 1.0],
                twt: 0.0,
                c: 0.0,
                tbeta: 1.0,
            },
            oscillators: vec![],
            constants: crate::leapr::vintage::PhysicalConstants::default(),
        };
        assert_eq!(inp.scale(), 1.0);
        // tev at 296 K ~ 0.0255 eV
        assert!((inp.tev() - 0.0255).abs() < 1e-3, "tev = {}", inp.tev());
    }
}
