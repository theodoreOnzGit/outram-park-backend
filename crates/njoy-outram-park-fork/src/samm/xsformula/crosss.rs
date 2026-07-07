//! Top-level per-energy cross-section evaluation, summed across every
//! spin group — ported from `crosss`'s non-derivative, non-angular core
//! (`samm.f90:3011-3229`).
//!
//! Ties together every phase of this port so far: [`crate::samm::mf2`]
//! (the parsed section), [`crate::samm::context`]/[`crate::samm::betset`]
//! (per-group kinematics/amplitudes, Phase 2), [`super::abpart`]/
//! [`super::setr`]/[`super::assembly`]/[`super::sectio`] (this phase), and
//! [`crate::samm::rmatrix_invert`] (Phase 4). This is the first point in
//! the port where a real number — a cross section in barns — comes out
//! the other end.
//!
//! **Not ported here (deferred, see this crate's `README.md`):** the
//! `Want_Angular_Dist`/`Want_Partial_Derivs`-gated bookkeeping
//! (`setleg`/`setqri`/`settri`/`derres`/`derext`) — no caller until
//! `ERRORR` is built (RECONR, the only current caller, disables both).

use crate::samm::betset::ResonanceAmplitudes;
use crate::samm::context::{ChannelKinematics, GroupQuantumInfo};
use crate::samm::mf2::RmlSection;
use crate::samm::rmatrix_invert::{invert, zero_triangular};
use crate::common::phys::PI;

use super::abpart::abpart;
use super::assembly::setxqx;
use super::sectio::sectio;
use super::setr::setr;

/// Compute every particle pair's cross section (barns) at incident (CM)
/// energy `energy` (eV) — ported from `crosss` (`samm.f90:3011-3229`),
/// summing [`setr`]/[`setxqx`]/[`sectio`]'s per-group contributions and
/// applying the overall `4*pi/su` normalization
/// (`samm.f90:3024`/`3179-3182`, `su` being the current energy).
///
/// `kinematics`/`amplitudes`/`quantum_info` must be
/// [`crate::samm::context::compute_channel_kinematics`]/
/// [`crate::samm::betset::compute_resonance_amplitudes`]/
/// [`crate::samm::context::check_quantum_numbers`]'s output, one entry per
/// [`RmlSection::spin_groups`] entry, same order.
///
/// Returns `sigma[pair_index]` (0-indexed, `pair_index = particle_pair - 1`)
/// — see [`super::sectio`]'s doc comment for the (upstream) quirk that
/// positions 0/1 are hardcoded to elastic/capture rather than following
/// pair numbering.
pub fn cross_sections(
    section: &RmlSection,
    kinematics: &[Vec<ChannelKinematics>],
    amplitudes: &[Vec<ResonanceAmplitudes>],
    quantum_info: &[GroupQuantumInfo],
    energy: f64,
) -> Vec<f64> {
    let npp = section.particle_pairs.len();
    let mut sigmas = vec![0.0_f64; npp];

    for (n, group) in section.spin_groups.iter().enumerate() {
        let alpha = abpart(&group.resonances, &amplitudes[n], energy);
        let out = setr(group, &kinematics[n], &section.particle_pairs, &amplitudes[n], &alpha, energy);

        let xqx = if out.lrmat_trivial {
            // samm.f90:3121-3123 -- trivial R-matrix: Xq/Xxxx are all zero.
            let z = zero_triangular(out.nchan);
            super::assembly::XqxOutput { xxxxr: z.re, xxxxi: z.im }
        } else {
            let yinv = invert(&out.ymat, out.nchan as i64);
            setxqx(out.nchan, &out.rmat, &yinv, &out.rootp, &out.elinvr, &out.elinvi)
        };

        let zke: Vec<f64> = kinematics[n][..out.nchan].iter().map(|k| k.zke).collect();
        let particle_pair: Vec<usize> = group.channels[..out.nchan].iter().map(|c| c.particle_pair).collect();

        let crss = sectio(npp, &quantum_info[n], &zke, &particle_pair, &out.sinsqr, &out.sin2ph, &xqx.xxxxr, &xqx.xxxxi);
        for (ip, c) in crss.into_iter().enumerate() {
            sigmas[ip] += c;
        }
    }

    // samm.f90:3024,3179-3182 -- normalize by 4*pi/su (su in barns-per-eV
    // convention: fourpi = 4*pi/100).
    let fourpi = 4.0 * PI / 100.0;
    for s in sigmas.iter_mut() {
        *s *= fourpi / energy;
    }

    sigmas
}
