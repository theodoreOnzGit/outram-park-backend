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
//! [`cross_sections`] is `crosss` with both flags off (RECONR's case);
//! [`cross_sections_with_derivs`] is `crosss` with `Want_Partial_Derivs`
//! (ERRORR's case, `samm.f90:3041-3222`): the same per-group evaluation
//! plus `setqri`/`settri`/`derres` and the u-parameter normalisation.
//! **Not ported:** the `Want_Angular_Dist` bookkeeping (`setleg`, `cscs`)
//! and `derext` (background R-matrix parameters).

use crate::samm::betset::ResonanceAmplitudes;
use crate::samm::context::{ChannelKinematics, GroupQuantumInfo};
use crate::samm::mf2::RmlSection;
use crate::samm::rmatrix_invert::{invert, zero_triangular};
use crate::common::phys::PI;

use crate::samm::derivs::energy::{abpart_derivs, derres, setqri, settri};
use crate::samm::derivs::DerivSetup;

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
        let out = setr(
            group,
            &kinematics[n],
            &section.particle_pairs,
            &amplitudes[n],
            &alpha,
            energy,
        );

        let xqx = if out.lrmat_trivial {
            // samm.f90:3121-3123 -- trivial R-matrix: Xq/Xxxx are all zero.
            let z = zero_triangular(out.nchan);
            super::assembly::XqxOutput {
                xxxxr: z.re,
                xxxxi: z.im,
            }
        } else {
            let yinv = invert(&out.ymat, out.nchan as i64);
            setxqx(
                out.nchan,
                &out.rmat,
                &yinv,
                &out.rootp,
                &out.elinvr,
                &out.elinvi,
            )
        };

        let zke: Vec<f64> = kinematics[n][..out.nchan].iter().map(|k| k.zke).collect();
        let particle_pair: Vec<usize> = group.channels[..out.nchan]
            .iter()
            .map(|c| c.particle_pair)
            .collect();

        let crss = sectio(
            npp,
            &quantum_info[n],
            &zke,
            &particle_pair,
            &out.sinsqr,
            &out.sin2ph,
            &xqx.xxxxr,
            &xqx.xxxxi,
        );
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

/// `crosss` with `Want_Partial_Derivs` (`samm.f90:3011-3229`): every
/// particle pair's cross section (as [`cross_sections`]) **and**
/// `dsigma[ip][ipar]`, the partial derivative of pair `ip`'s cross section
/// with respect to resonance parameter `ipar` of `ds` (barns per eV for
/// `E_λ`/`Γ` parameters), normalised by `4π/E` and converted from
/// u-parameters (`samm.f90:3193-3222`).
///
/// A spin group whose R-matrix is trivially zero at this energy (`lrmat`)
/// contributes no derivative, as upstream (`needxq`, `samm.f90:3125-3127`).
#[allow(clippy::needless_range_loop)]
pub fn cross_sections_with_derivs(
    section: &RmlSection,
    kinematics: &[Vec<ChannelKinematics>],
    amplitudes: &[Vec<ResonanceAmplitudes>],
    quantum_info: &[GroupQuantumInfo],
    ds: &DerivSetup,
    energy: f64,
) -> (Vec<f64>, Vec<Vec<f64>>) {
    let npp = section.particle_pairs.len();
    let npar = ds.npar;
    let mut sigmas = vec![0.0_f64; npp];
    let mut dsigma = vec![vec![0.0_f64; npar]; npp];

    // abpart for every group first (upstream fills the module arrays for
    // all resonances before crosss runs), then its derivative half.
    let alpha_all: Vec<Vec<super::abpart::AlphaTerms>> = section
        .spin_groups
        .iter()
        .enumerate()
        .map(|(n, g)| abpart(&g.resonances, &amplitudes[n], energy))
        .collect();
    let nchan_full: Vec<usize> = section.spin_groups.iter().map(|g| g.channels.len()).collect();
    let terms = abpart_derivs(ds, &nchan_full, &alpha_all);

    for (n, group) in section.spin_groups.iter().enumerate() {
        let out = setr(
            group,
            &kinematics[n],
            &section.particle_pairs,
            &amplitudes[n],
            &alpha_all[n],
            energy,
        );
        let npr = ds.npr[n];
        let kstart = ds.kstart[n];

        let (xqx, yinv) = if out.lrmat_trivial {
            let z = zero_triangular(out.nchan);
            (
                super::assembly::XqxOutput {
                    xxxxr: z.re,
                    xxxxi: z.im,
                },
                None,
            )
        } else {
            let yinv = invert(&out.ymat, out.nchan as i64);
            let xqx = setxqx(
                out.nchan,
                &out.rmat,
                &yinv,
                &out.rootp,
                &out.elinvr,
                &out.elinvi,
            );
            (xqx, Some(yinv))
        };
        // samm.f90:3125-3127
        let needxq = npr == 0 || out.lrmat_trivial;

        let zke: Vec<f64> = kinematics[n][..out.nchan].iter().map(|k| k.zke).collect();
        let particle_pair: Vec<usize> = group.channels[..out.nchan]
            .iter()
            .map(|c| c.particle_pair)
            .collect();

        let crss = sectio(
            npp,
            &quantum_info[n],
            &zke,
            &particle_pair,
            &out.sinsqr,
            &out.sin2ph,
            &xqx.xxxxr,
            &xqx.xxxxi,
        );
        for (ip, c) in crss.into_iter().enumerate() {
            sigmas[ip] += c;
        }

        // samm.f90:3149-3176 -- derivatives for this group, accumulated
        // straight into dsigma (see `derres`'s doc comment)
        if !needxq {
            let yinv = yinv.as_ref().expect("non-trivial R has an inverse");
            let q = setqri(
                out.nchan,
                &out.rootp,
                &out.elinvr,
                &out.elinvi,
                &out.psmall,
                yinv,
            );
            let t = settri(
                npp,
                quantum_info[n].n_entrance,
                out.nchan,
                &zke,
                &particle_pair,
                &out.sinsqr,
                &out.sin2ph,
                &xqx.xxxxr,
                &xqx.xxxxi,
                &q,
            );
            derres(
                npp,
                out.nchan,
                quantum_info[n].goj,
                kstart,
                npr,
                &terms,
                &t,
                &mut dsigma,
            );
        }
    }

    // samm.f90:3179-3182
    let fourpi = 4.0 * PI / 100.0;
    for s in sigmas.iter_mut() {
        *s *= fourpi / energy;
    }

    // samm.f90:3193-3222 -- normalise by 4pi/E, divide by uuuu (u -> Gamma,
    // sqrt(E) -> E), and fold the penetrability's energy dependence (duuu)
    // of the same resonance's channel widths into the E_lambda derivative.
    for m in 0..npar {
        let u = ds.uuuu[m];
        if u != 0.0 {
            let u = fourpi / energy / u;
            if ds.iduu[m] == 1 {
                for mx in 1..=ds.mchan {
                    let mplus = m + mx + 1;
                    if mplus >= npar {
                        break;
                    }
                    if ds.iduu[mplus] > 0 {
                        break;
                    }
                    if ds.duuu[mplus] != 0.0 {
                        for ip in 0..npp {
                            let sub = ds.duuu[mplus] * dsigma[ip][mplus];
                            dsigma[ip][m] -= sub;
                        }
                    }
                }
            }
            for ip in 0..npp {
                dsigma[ip][m] *= u;
            }
        } else {
            for ip in 0..npp {
                dsigma[ip][m] = 0.0;
            }
        }
    }

    (sigmas, dsigma)
}
