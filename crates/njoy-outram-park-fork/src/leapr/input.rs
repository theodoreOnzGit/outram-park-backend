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

use crate::common::phys::BK_EV_PER_K;

/// Coherent-elastic (Bragg) lattice option (card 5 `iel`), selecting which
/// lattice the reciprocal-lattice sum in [`crate::leapr::coher`] (the ported
/// `coher`/`formf`/`tausq` path of leapr.f90) runs over. Downstream, the
/// resulting Bragg-edge `S(E)` is *evaluated* by [`crate::thermr::coherent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElasticOption {
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
}

impl LeaprInput {
    /// Thermal energy `tev = k_B * T` \[eV\] for this job.
    pub fn tev(&self) -> f64 {
        BK_EV_PER_K * self.temperature_k.abs()
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
        };
        assert_eq!(inp.scale(), 1.0);
        // tev at 296 K ~ 0.0255 eV
        assert!((inp.tev() - 0.0255).abs() < 1e-3, "tev = {}", inp.tev());
    }
}
