//! One-time, per-section setup tying Phase 1 (parsing) into Phase 2
//! (spin/parity/penetrability) — ported from `ppsammy`'s non-derivative,
//! non-angular core (`samm.f90:1523-1561`).
//!
//! `ppsammy` itself also calls `angle` (`Want_Angular_Dist`-gated) and
//! `babb` (`Want_Partial_Derivs`-gated) — both deferred to build alongside
//! `ERRORR`, per this crate's `README.md`.

use crate::samm::betset::{self, ResonanceAmplitudes};
use crate::samm::context::{self, ChannelKinematics, GroupQuantumInfo};
use crate::samm::mf2::RmlSection;
use crate::NjoyError;

/// Everything [`crate::samm::xsformula::cross_sections`] needs, computed
/// once per resonance-range section — ported from `ppsammy`
/// (`samm.f90:1523-1561`): `checkqn` → [`quantum_info`](Self::quantum_info),
/// `fxradi` → [`kinematics`](Self::kinematics), `betset` →
/// [`amplitudes`](Self::amplitudes).
pub struct SammSetup {
    /// Per spin group, per channel (same order as
    /// [`crate::samm::mf2::SpinGroup::channels`]).
    pub kinematics: Vec<Vec<ChannelKinematics>>,
    /// Per spin group, per resonance (same order as
    /// [`crate::samm::mf2::SpinGroup::resonances`]).
    pub amplitudes: Vec<Vec<ResonanceAmplitudes>>,
    /// Per spin group.
    pub quantum_info: Vec<GroupQuantumInfo>,
}

/// Run Phase 2's one-time-per-section setup — ported from `ppsammy`
/// (`samm.f90:1523-1561`, minus `angle`/`babb`, see this module's doc
/// comment). `section.particle_pairs` is mutated in place to fill in
/// ENDF-implied defaults (`ppdefs`+`fxradi`'s own generic fallback); `awr`
/// is the target nuclide's mass in neutron-mass units (ENDF `AWR`, read
/// one level up from [`crate::samm::mf2::parse_rml_section`] — see
/// [`context::compute_channel_kinematics`]'s doc comment).
pub fn setup(section: &mut RmlSection, awr: f64) -> Result<SammSetup, NjoyError> {
    context::apply_particle_pair_defaults(&mut section.particle_pairs);
    let quantum_info =
        context::check_quantum_numbers(&section.particle_pairs, &section.spin_groups)?;
    let kinematics = context::compute_channel_kinematics(
        &mut section.particle_pairs,
        &section.spin_groups,
        awr,
    )?;

    let mut amplitudes = Vec::with_capacity(section.spin_groups.len());
    for (n, group) in section.spin_groups.iter().enumerate() {
        amplitudes.push(betset::compute_resonance_amplitudes(
            group,
            &kinematics[n],
            &section.particle_pairs,
        )?);
    }

    Ok(SammSetup {
        kinematics,
        amplitudes,
        quantum_info,
    })
}
