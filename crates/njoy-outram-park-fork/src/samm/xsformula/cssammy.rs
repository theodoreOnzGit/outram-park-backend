//! The RECONR-facing per-energy entry point — ported from `cssammy`'s
//! non-derivative, non-angular core (`samm.f90:79-166`). Maps
//! [`super::crosss::cross_sections`]'s per-particle-pair output into the
//! MT-like reaction slots RECONR's own driver loop expects (total,
//! elastic, fission, capture, other).
//!
//! `cssammy` is the function `reconr.f90`/`errorr.f90` actually call once
//! per pointwise energy grid point (`reconr.f90:2624`); everything in this
//! port up through Phase 5 exists to make this one function possible.

use crate::samm::betset::ResonanceAmplitudes;
use crate::samm::context::{ChannelKinematics, GroupQuantumInfo};
use crate::samm::derivs::DerivSetup;
use crate::samm::mf2::RmlSection;

use super::crosss::{cross_sections, cross_sections_with_derivs};

/// Cross sections (barns) at one incident energy, in the MT-like slots
/// `cssammy` returns to its caller — ported from `samm.f90:102-121`.
#[derive(Debug, Clone, Default)]
pub struct CssammyResult {
    /// `sigp(1)`: total = elastic + (capture-bucket before subtracting
    /// fission/other, `samm.f90:108` — computed from the *raw*
    /// non-elastic bucket, matching upstream's own evaluation order).
    pub total: f64,
    /// `sigp(2)`.
    pub elastic: f64,
    /// `sigp(3)`, `0.0` if this section has no `MT=18` particle pair.
    pub fission: f64,
    /// `sigp(4)`: pure radiative capture, i.e. the non-elastic bucket with
    /// every explicit reaction (fission and `other`) subtracted out.
    pub capture: f64,
    /// `sigp(5..)`: `(MT, cross_section)` for every particle pair beyond
    /// elastic/fission/capture, same order as
    /// [`RmlSection::particle_pairs`].
    pub other: Vec<(i32, f64)>,
}

/// Evaluate all cross sections for one section at incident (CM) energy
/// `energy` (eV) — ported from `cssammy` (`samm.f90:79-166`, its
/// `Want_Angular_Dist`/`Want_Partial_Derivs` branches excluded, see this
/// module's doc comment and `README.md`'s scope-history note).
///
/// `kinematics`/`amplitudes`/`quantum_info` must be
/// [`crate::samm::setup::setup`]'s output for this same `section`.
pub fn cssammy(
    section: &RmlSection,
    kinematics: &[Vec<ChannelKinematics>],
    amplitudes: &[Vec<ResonanceAmplitudes>],
    quantum_info: &[GroupQuantumInfo],
    energy: f64,
) -> CssammyResult {
    let sigmas = cross_sections(section, kinematics, amplitudes, quantum_info, energy);
    slots_from_sigmas(section, &sigmas)
}

/// `cssammy` with `Want_Partial_Derivs` (`samm.f90:79-166` incl.
/// l.152-164): the cross sections as [`cssammy`] plus `sigd`, the
/// sensitivity of reaction slot `l` to resonance parameter `ipar`, flat
/// at `sigd[ipar * npp + l]` — slot 0 elastic, slot 1 capture (the
/// non-elastic bucket **minus** every explicit reaction pair), slots
/// `2..` the explicit pairs 3.. in particle-pair order (`mmtres(3..)`),
/// `nmtres = npp` slots in all.
#[allow(clippy::needless_range_loop)]
pub fn cssammy_with_derivs(
    section: &RmlSection,
    kinematics: &[Vec<ChannelKinematics>],
    amplitudes: &[Vec<ResonanceAmplitudes>],
    quantum_info: &[GroupQuantumInfo],
    ds: &DerivSetup,
    energy: f64,
) -> (CssammyResult, Vec<f64>) {
    let (sigmas, dsigma) =
        cross_sections_with_derivs(section, kinematics, amplitudes, quantum_info, ds, energy);
    let npp = section.particle_pairs.len();
    let mut sigd = vec![0.0_f64; ds.npar * npp];
    for i in 0..ds.npar {
        let row = &mut sigd[i * npp..(i + 1) * npp];
        row[0] = dsigma[0][i];
        row[1] = dsigma[1][i];
        for l in 3..=npp {
            row[l - 1] = dsigma[l - 1][i];
            row[1] -= dsigma[l - 1][i];
        }
    }
    (slots_from_sigmas(section, &sigmas), sigd)
}

/// `samm.f90:102-121` -- map per-pair `sigmas` into the MT-like slots.
fn slots_from_sigmas(section: &RmlSection, sigmas: &[f64]) -> CssammyResult {
    let elastic = sigmas.first().copied().unwrap_or(0.0);
    let raw_nonelastic = sigmas.get(1).copied().unwrap_or(0.0);
    let total = elastic + raw_nonelastic;

    let mut capture = raw_nonelastic;
    let mut fission = 0.0_f64;
    let mut other = Vec::new();

    let npp = section.particle_pairs.len();
    if npp > 2 {
        for i in 3..=npp {
            let s_i = sigmas[i - 1];
            capture -= s_i;
            let mt = section.particle_pairs[i - 1].mt;
            if mt == 18 {
                fission = s_i;
            } else {
                other.push((mt, s_i));
            }
        }
    }

    CssammyResult {
        total,
        elastic,
        fission,
        capture,
        other,
    }
}
