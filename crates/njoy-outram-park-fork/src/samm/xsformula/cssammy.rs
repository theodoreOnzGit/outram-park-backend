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
use crate::samm::mf2::RmlSection;

use super::crosss::cross_sections;

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

    CssammyResult { total, elastic, fission, capture, other }
}
