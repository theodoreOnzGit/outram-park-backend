// SPDX-License-Identifier: AGPL-3.0-only
//
// Attribution (RESEARCH_INTEGRITY_AND_PROVENANCE.md): port of EGSnrc.
//   Upstream project: EGSnrc, https://github.com/nrc-cnrc/EGSnrc
//   Upstream file:    HEN_HOUSE/src/egsnrc.mortran
//   Upstream commit:  bd49a47aae3938f49f4100b8e018091861408b19
//   Copyright:        National Research Council of Canada (and the EGS4 SLAC
//                     authors for the parts of the file that carry that notice)
//   Upstream licence: AGPL-3.0-or-later; this port is AGPL-3.0-only.
//! `HEN_HOUSE/src/egsnrc.mortran` (EGSnrc, 9016 lines): HATCH (data set-up),
//! the ELECTR and PHOTON transport loops and the physics routines they call.
//!
//! **Status: first port of ten routines, not differentially tested (gh:#1680).**
//! Everything else in the file is still a placeholder.
//!
//! | upstream | here |
//! |---|---|
//! | `SUBROUTINE ANNIH` (`egsnrc.mortran:70-183`) | [`annih`] |
//! | `SUBROUTINE ANNIH_AT_REST` (`egsnrc.mortran:186-224`) | [`annih_at_rest`] |
//! | `SUBROUTINE UPHI` (`egsnrc.mortran:6722-6838`), `IENTRY` 2 and 3 | [`uphi`] |
//! | `SUBROUTINE RELAX` (`egsnrc.mortran:5556-5790`), the built-in K/L1/L2/L3/M/N scheme | [`relax`] |
//! | `SUBROUTINE PAIR` (`egsnrc.mortran:4517-4670`) with `$SET-PAIR-ANGLE` (`egsnrc.macros:2070-2165`) | [`pair`] |
//! | `UPHI` `IENTRY = 1` (angle from `theta`, `egsnrc.mortran:6759-6769`) | [`uphi_from_theta`] |
//! | `SUBROUTINE PHOTO` (`egsnrc.mortran:5049-5170`) with `$SELECT-PHOTOELECTRON-DIRECTION` (`egsnrc.macros:1916-1953`) | [`photo`], [`select_photoelectron_direction`] |
//! | `SUBROUTINE BHABHA` (`egsnrc.mortran:229-309`) | [`bhabha`] |
//! | `SUBROUTINE MOLLER` (`egsnrc.mortran:2598-2745`), without the EII branch | [`moller`] |
//! | `SUBROUTINE BREMS` (`egsnrc.mortran:312-525`), Bethe-Heitler path | [`brems`] |
//! | `SUBROUTINE COMPT` (`egsnrc.mortran:528-806`) | [`compt`] |
//! | `$PLAYRUSSIANROULETTEWITHELECTRONSFROM#` (`egsnrc.macros:3479-3516`) | [`play_russian_roulette`] |
//!
//! Not yet ported from this file: `BHABHA`, `BREMS`, `ELECTR`,
//! `HATCH`, `MOLLER`, `EDGSET`, `PHOTON`, `SHOWER`,
//! `PHOTONUC`. `UPHI` entry 1 (which derives `sinthe`/`costhe` from `theta`
//! through the sine look-up table) is also not ported: it needs the table that
//! `HATCH` fills. The user-scoring `$AUSCALL` hooks are omitted (no `AUSGAB`).

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use super::egsnrc_macros::{EgsError, Stack, UniformSource, select_azimuthal_angle};
use petir::mathf::RealMath;
use petir::real::Real;

/// `ANNIH_AT_REST` (`egsnrc.mortran:186-224`): the particle on top of the stack
/// (a positron that has stopped) is replaced by `2 * nbr_split` annihilation
/// photons of energy `prm` (the electron rest energy, MeV), emitted in
/// back-to-back pairs in random directions. With `nbr_split > 1` (radiative
/// splitting) the weight of the parent is divided by `nbr_split` first.
///
/// `np_old` and `np` are updated as upstream does.
///
/// # Errors
///
/// [`EgsError::StackExceeded`] when `np + 2*nbr_split - 1 > $MXSTACK`.
pub fn annih_at_rest<R: UniformSource>(
    stack: &mut Stack,
    nbr_split: usize,
    prm: f64,
    rng: &mut R,
) -> Result<(), EgsError> {
    stack.np_old = stack.np;
    stack.check_stack(stack.np + 2 * nbr_split - 1, "ANNIH_AT_REST")?;
    if nbr_split > 1 {
        let np = stack.np;
        stack.wt[np] /= nbr_split as f64;
    }
    for ibr in 1..=nbr_split {
        // Pick random direction for first gamma
        let mut costhe = rng.uniform();
        costhe = 2.0 * costhe - 1.0;
        let sinthe = Real::sqrt(((1.0 - costhe) * (1.0 + costhe)).max(0.0));
        let (cphi, sphi) = select_azimuthal_angle(rng);
        let np = stack.np;
        stack.e[np] = prm;
        stack.iq[np] = 0;
        let ip = if ibr == 1 { stack.np_old } else { np - 1 };
        stack.transfer_properties(np, ip);
        stack.u[np] = sinthe * cphi;
        stack.v[np] = sinthe * sphi;
        stack.w[np] = costhe;
        stack.np += 1;
        let np = stack.np;
        stack.e[np] = prm;
        stack.iq[np] = 0;
        stack.transfer_properties(np, np - 1);
        stack.u[np] = -stack.u[np - 1];
        stack.v[np] = -stack.v[np - 1];
        stack.w[np] = -stack.w[np - 1];
        stack.np += 1;
    }
    stack.np -= 1;
    Ok(())
}

/// `ANNIH` (`egsnrc.mortran:70-183`): two-photon in-flight positron
/// annihilation (Heitler's formulae, Kawrakow's rejection normalised to its
/// maximum). The positron on top of the stack (kinetic energy `e[np]`, total
/// energy `e[np] + rm`) is replaced by `2 * nbr_split` photons; `rm` is the
/// electron rest energy (`RM` and `PRM`, MeV; both are `f64` here, so the
/// "precise" variables are the plain ones).
///
/// `e[np]` is the positron's **total** energy; the sampling needs
/// `(e + rm) / rm >= 2`, i.e. `e >= rm`, which any positron satisfies. Below
/// that `p` is NaN and upstream spins in the rejection loop, and so does this
/// port: the caller owns that precondition (found when a test passed a
/// kinetic energy and hung).
///
/// `nbr_split <= 0` returns with only `np_old` set, as upstream.
///
/// # Errors
///
/// [`EgsError::StackExceeded`] when `np + 1 > $MXSTACK` inside the loop.
pub fn annih<R: UniformSource>(stack: &mut Stack, nbr_split: usize, rm: f64, rng: &mut R) -> Result<(), EgsError> {
    stack.np_old = stack.np;
    if nbr_split == 0 {
        return Ok(());
    }
    let np0 = stack.np;
    let pavip = stack.e[np0] + rm;
    let avip = pavip;
    let a = avip / rm;
    let g = a - 1.0;
    let t = g - 1.0;
    let p = Real::sqrt(a * t);
    let pot = p / t;
    let ep0 = 1.0 / (a + p);
    let wsamp = RealMath::r_ln((1.0 - ep0) / ep0);

    let aa = stack.u[np0];
    let bb = stack.v[np0];
    let cc = stack.w[np0];
    let mut sinpsi = aa * aa + bb * bb;
    let (mut sindel, mut cosdel) = (0.0, 0.0);
    if sinpsi > 1e-20 {
        sinpsi = Real::sqrt(sinpsi);
        sindel = bb / sinpsi;
        cosdel = aa / sinpsi;
    }
    if nbr_split > 1 {
        stack.wt[np0] /= nbr_split as f64;
    }
    for ibr in 1..=nbr_split {
        if stack.np + 1 > stack.mxstack() {
            return Err(EgsError::StackExceeded {
                routine: "ANNIH",
                needed: stack.np + 1,
                mxstack: stack.mxstack(),
            });
        }
        let mut ep;
        loop {
            let rnno01 = rng.uniform();
            ep = ep0 * RealMath::r_exp(rnno01 * wsamp);
            let rnno02 = rng.uniform();
            let t2 = ep * a - 1.0;
            let rejf = 1.0 - t2 * t2 / (ep * (a * a - 2.0));
            if rnno02 <= rejf {
                break;
            }
        }
        // set up energies
        let esg1 = avip * ep;
        let pesg1 = esg1;
        let np = stack.np;
        stack.e[np] = pesg1;
        stack.iq[np] = 0;
        let ip = if ibr == 1 { stack.np_old } else { np - 1 };
        stack.transfer_properties(np, ip);
        let mut costhe = (-1.0_f64).max(1.0_f64.min((esg1 - rm) * pot / esg1));
        let mut sinthe = Real::sqrt(1.0 - costhe * costhe);
        let (cphi, sphi) = select_azimuthal_angle(rng);
        annih_rotate(stack, np, (aa, bb, cc), (sinpsi, sindel, cosdel), (sinthe, costhe, cphi, sphi));
        stack.np += 1;
        let pesg2 = pavip - pesg1;
        let esg2 = pesg2;
        let np = stack.np;
        stack.e[np] = pesg2;
        stack.iq[np] = 0;
        stack.transfer_properties(np, np - 1);
        costhe = (-1.0_f64).max(1.0_f64.min((esg2 - rm) * pot / esg2));
        sinthe = -Real::sqrt(1.0 - costhe * costhe);
        annih_rotate(stack, np, (aa, bb, cc), (sinpsi, sindel, cosdel), (sinthe, costhe, cphi, sphi));
        stack.np += 1;
    }
    stack.np -= 1;
    Ok(())
}

/// The inline rotation `ANNIH` writes twice (`egsnrc.mortran:151-157`,
/// `169-175`): into slot `np`, the scattering direction `(sinthe, costhe,
/// cphi, sphi)` relative to the incident `(aa, bb, cc)`.
fn annih_rotate(
    stack: &mut Stack,
    np: usize,
    (aa, bb, cc): (f64, f64, f64),
    (sinpsi, sindel, cosdel): (f64, f64, f64),
    (sinthe, costhe, cphi, sphi): (f64, f64, f64, f64),
) {
    if sinpsi >= 1e-10 {
        let us = sinthe * cphi;
        let vs = sinthe * sphi;
        stack.u[np] = cc * cosdel * us - sindel * vs + aa * costhe;
        stack.v[np] = cc * sindel * us + cosdel * vs + bb * costhe;
        stack.w[np] = cc * costhe - sinpsi * us;
    } else {
        stack.u[np] = sinthe * cphi;
        stack.v[np] = sinthe * sphi;
        stack.w[np] = cc * costhe;
    }
}

/// The scratch `UPHI` keeps between calls (`save A,B,C,...`): the direction of
/// the particle before the rotation, so a second particle can reuse it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UphiSaved {
    /// `A`.
    pub a: f64,
    /// `B`.
    pub b: f64,
    /// `C`.
    pub c: f64,
    /// `COSPHI` (kept so `IENTRY = 3` can reuse the azimuth of the last call).
    pub cosphi: f64,
    /// `SINPHI`.
    pub sinphi: f64,
}

/// `UPHI` (`egsnrc.mortran:6722-6838`), `IENTRY` 2 and 3: rotate the direction
/// of a stack particle by the polar angle `(sinthe, costhe)`.
///
/// `ientry = 2` draws a fresh uniform azimuth (box method) and stores it in
/// `saved`; `ientry = 3` (`:NRK:`) **reuses the azimuth of the previous call**,
/// as `COMPT` does for the recoil electron (opposite side of the same plane).
/// `ientry = 1` (derive the angle from `theta` through the sine table) is not
/// ported and is refused.
///
/// `lvl` selects the particle: `1` old particle (its direction is saved and
/// adjusted), `2` new particle (properties copied from `np - 1`, saved
/// direction reused), `3` bremsstrahlung photon (the electron's direction at
/// `np - 1` is saved, then the photon's is set).
///
/// # Errors
///
/// [`EgsError::BadEntry`] for an `ientry` other than 2 or 3, or an `lvl`
/// outside 1..=3.
pub fn uphi<R: UniformSource>(
    stack: &mut Stack,
    saved: &mut UphiSaved,
    ientry: i32,
    lvl: i32,
    sinthe: f64,
    costhe: f64,
    rng: &mut R,
) -> Result<(), EgsError> {
    match ientry {
        2 => {
            let (c, sn) = select_azimuthal_angle(rng);
            saved.cosphi = c;
            saved.sinphi = sn;
        }
        3 => {}
        _ => return Err(EgsError::BadEntry { ientry, lvl }),
    }
    let (cosphi, sinphi) = (saved.cosphi, saved.sinphi);
    let np = stack.np;
    match lvl {
        1 => {
            saved.a = stack.u[np];
            saved.b = stack.v[np];
            saved.c = stack.w[np];
        }
        3 => {
            saved.a = stack.u[np - 1];
            saved.b = stack.v[np - 1];
            saved.c = stack.w[np - 1];
            stack.transfer_properties(np, np - 1);
        }
        2 => {
            stack.transfer_properties(np, np - 1);
        }
        _ => return Err(EgsError::BadEntry { ientry, lvl }),
    }
    let (a, b, c) = (saved.a, saved.b, saved.c);
    let sinps2 = a * a + b * b;
    if sinps2 < 1.0e-20 {
        // small polar angle case
        stack.u[np] = sinthe * cosphi;
        stack.v[np] = sinthe * sinphi;
        stack.w[np] = c * costhe;
    } else {
        let sinpsi = Real::sqrt(sinps2);
        let us = sinthe * cosphi;
        let vs = sinthe * sinphi;
        let sindel = b / sinpsi;
        let cosdel = a / sinpsi;
        stack.u[np] = c * cosdel * us - sindel * vs + a * costhe;
        stack.v[np] = c * sindel * us + cosdel * vs + b * costhe;
        stack.w[np] = -sinpsi * us + c * costhe;
    }
    Ok(())
}

/// Electron rest energy, MeV (`RM` and `PRM`, `egsnrc.macros:1140`).
pub const RM: f64 = 0.5109989461;

/// The Compton data `COMPT` reads (`COMMON/COMPTON-DATA/`,
/// `egsnrc.macros:450-472`), filled upstream by `HATCH` from the media's
/// shell lists. All arrays are **1-based** (index 0 unused), per-medium ones
/// indexed `[medium][shell-slot]`.
#[derive(Debug, Clone, Default)]
pub struct ComptonData {
    /// `iz_array`: atomic number of each shell.
    pub iz_array: Vec<i32>,
    /// `be_array`: shell binding energies in units of `rm`.
    pub be_array: Vec<f64>,
    /// `Jo_array`: Compton profile parameter.
    pub jo_array: Vec<f64>,
    /// `erfJo_array`: erf-related constant of the shell's profile.
    pub erf_jo_array: Vec<f64>,
    /// `shn_array`: shell type (K, L1, ...).
    pub shn_array: Vec<i32>,
    /// `shell_array(slot, medium)`: shell number of a slot.
    pub shell_array: Vec<Vec<i32>>,
    /// `eno_array(slot, medium)`: alias probability of a slot.
    pub eno_array: Vec<Vec<f64>>,
    /// `eno_atbin_array(slot, medium)`: alias partner slot.
    pub eno_atbin_array: Vec<Vec<i32>>,
    /// `n_shell(medium)`: number of slots.
    pub n_shell: Vec<i32>,
}

/// A shell vacancy `COMPT` leaves for `RELAX` (not ported): upstream calls
/// `relax(Uj, shn_array(j), iz_array(j))` when `Uj > 1e-3` MeV.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellVacancy {
    /// Binding energy `Uj`, MeV.
    pub uj: f64,
    /// Shell type `shn_array(j)`.
    pub shn: i32,
    /// Atomic number `iz_array(j)`.
    pub iz: i32,
}

/// What `COMPT` produced besides the stack change.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ComptonOutcome {
    /// `:INTERACTION-REJECTED:` (only with `ibcmp = 1`): the photon is
    /// unscattered, nothing was pushed.
    pub rejected: bool,
    /// A vacancy for `RELAX` (`ibcmp` 1 or 3 and `Uj > 1e-3` MeV).
    pub vacancy: Option<ShellVacancy>,
    /// `edep` deposited locally when the vacancy energy is too small to relax
    /// (`Uj <= 1e-3`), otherwise `0`.
    pub edep_local: f64,
}

/// `$PLAY RUSSIAN ROULETTE WITH ELECTRONS FROM #;`
/// (`egsnrc.macros:3479-3516`): each charged particle from stack slot `from`
/// to `np` survives with probability `prob_rr` (weight divided by it) or is
/// replaced by the top of the stack. Returns the number killed
/// (`i_survived_RR` upstream, which counts the kills). `prob_rr <= 0` plays
/// nothing (upstream prints a warning, not reproduced). An emptied stack gets
/// one zero-weight photon so the transport loop can exit.
pub fn play_russian_roulette<R: UniformSource>(stack: &mut Stack, from: usize, prob_rr: f64, rng: &mut R) -> usize {
    let mut killed = 0;
    if prob_rr <= 0.0 {
        return killed;
    }
    let mut ip = from;
    loop {
        if stack.iq[ip] != 0 {
            let rnno_rr = rng.uniform();
            if rnno_rr < prob_rr {
                stack.wt[ip] /= prob_rr;
                ip += 1;
            } else {
                killed += 1;
                let np = stack.np;
                if ip < np {
                    stack.e[ip] = stack.e[np];
                    stack.iq[ip] = stack.iq[np];
                    stack.wt[ip] = stack.wt[np];
                    stack.u[ip] = stack.u[np];
                    stack.v[ip] = stack.v[np];
                    stack.w[ip] = stack.w[np];
                }
                stack.np -= 1;
            }
        } else {
            ip += 1;
        }
        if ip > stack.np {
            break;
        }
    }
    if stack.np == 0 {
        stack.np = 1;
        stack.e[1] = 0.0;
        stack.iq[1] = 0;
        stack.wt[1] = 0.0;
    }
    killed
}

/// `COMPT` (`egsnrc.mortran:528-806`): incoherent (Compton) scattering of the
/// photon on top of the stack. `ibcmp` (the region's flag): 0 Klein-Nishina,
/// 1 impulse approximation with rejection of the whole interaction, 2 impulse
/// approximation without binding or Doppler energy and 3 the full bound
/// cross section with resampling.
///
/// The scattered photon stays in slot `np_old`; the recoil electron is pushed
/// (`iq = -1`, total energy `pese`). `medium` and `region` are 1-based. `rr`
/// is `Some(prob_RR)` when `i_play_RR = 1`.
///
/// **Seams, deliberate:**
/// - A shell vacancy is *returned* ([`ComptonOutcome::vacancy`]) because `RELAX`
///   is not ported. Upstream plays Russian roulette after `RELAX`; here it is
///   played inside only when no vacancy is pending, so with a vacancy the
///   caller applies [`play_russian_roulette`] from `np_old + 1` after relaxing.
/// - Radiative Compton corrections (`$RADC_CHECK`, `$RADC_REJECTION`, empty by
///   default, `egsnrc.macros:3531`) and the `AUSGAB` calls are omitted.
/// - `$egs_warning` for a `br` outside its range is not reproduced.
///
/// # Errors
///
/// [`EgsError::StackExceeded`] when `np + 1 > $MXSTACK`.
#[allow(unused_assignments, clippy::too_many_arguments)]
pub fn compt<R: UniformSource>(
    stack: &mut Stack,
    data: &ComptonData,
    medium: usize,
    region: usize,
    ibcmp: i32,
    rr: Option<f64>,
    prm: f64,
    rng: &mut R,
) -> Result<ComptonOutcome, EgsError> {
    let rm = RM;
    stack.np_old = stack.np;
    let np_old = stack.np_old;
    let peig = stack.e[stack.np];
    let ko = peig / rm;
    let broi = 1.0 + 2.0 * ko;
    let broi2 = broi * broi;
    let _ = region;
    let ibcmpl = ibcmp;
    let mut first_time = true;

    // sampling state that persists across the RESAMPLE loops
    let (mut alph1, mut alpha, mut bro, mut bro1, mut rejmax) = (0.0, 0.0, 0.0, 0.0, 0.0);
    let (mut uj, mut jo) = (0.0, 0.0);
    let mut j = 0usize;
    let mut br;
    let mut temp;
    let mut sinthe;
    let mut costhe;

    'resample_compton: loop {
        if ibcmpl > 0 {
            // first sample the shell and see whether an interaction is possible
            let mut rnno17 = rng.uniform();
            rnno17 = 1.0 + rnno17 * data.n_shell[medium] as f64;
            let mut i = rnno17 as usize;
            if rnno17 > data.eno_array[medium][i] {
                i = data.eno_atbin_array[medium][i] as usize;
            }
            j = data.shell_array[medium][i] as usize;
            uj = data.be_array[j];
            if ko <= uj {
                if ibcmpl == 1 {
                    return Ok(ComptonOutcome { rejected: true, ..Default::default() });
                }
                continue 'resample_compton;
            }
            jo = data.jo_array[j];
        }

        // We always sample the scattering angle from Klein-Nishina
        'resample: loop {
            if ko > 2.0 {
                if first_time {
                    alph1 = RealMath::r_ln(broi);
                    bro = 1.0 / broi;
                    let alph2 = ko * (broi + 1.0) * bro * bro;
                    alpha = alph1 + alph2;
                }
                loop {
                    let rnno15 = rng.uniform();
                    let rnno16 = rng.uniform();
                    if rnno15 * alpha < alph1 {
                        br = RealMath::r_exp(alph1 * rnno16) * bro;
                    } else {
                        br = Real::sqrt(rnno16 * broi2 + (1.0 - rnno16)) * bro;
                    }
                    temp = (1.0 - br) / (ko * br);
                    sinthe = (0.0_f64).max(temp * (2.0 - temp));
                    let aux = 1.0 + br * br;
                    let rejf3 = aux - br * sinthe;
                    let rnno19 = rng.uniform();
                    if rnno19 * aux <= rejf3 {
                        break;
                    }
                }
            } else {
                if first_time {
                    bro = 1.0 / broi;
                    bro1 = 1.0 - bro;
                    rejmax = broi + bro;
                }
                loop {
                    let rnno15 = rng.uniform();
                    let rnno16 = rng.uniform();
                    br = bro + bro1 * rnno15;
                    temp = (1.0 - br) / (ko * br);
                    sinthe = (0.0_f64).max(temp * (2.0 - temp));
                    let rejf3 = 1.0 + br * br - br * sinthe;
                    if rnno16 * br * rejmax <= rejf3 {
                        break;
                    }
                }
            }
            first_time = false;
            if br < bro || br > 1.0 {
                continue 'resample;
            }
            break;
        }

        costhe = 1.0 - temp;
        if ibcmp == 0 {
            // Klein-Nishina: done
            uj = 0.0;
            break 'resample_compton;
        }

        // Check for rejection due to the limited range of pzmax
        let br2 = br * br;
        let mut aux = ko * (ko - uj) * temp;
        let aux1 = 2.0 * aux + uj * uj;
        let mut pzmax = aux - uj;
        if pzmax < 0.0 && pzmax * pzmax >= aux1 {
            if ibcmpl == 1 {
                return Ok(ComptonOutcome { rejected: true, ..Default::default() });
            }
            continue 'resample_compton;
        }
        pzmax /= Real::sqrt(aux1);
        let qc2 = 1.0 + br * br - 2.0 * br * costhe;
        let qc = Real::sqrt(qc2);

        let mut af = 0.0;
        let mut fmax = 1.0;
        let mut fpz = 1.0;
        'retry_setup: {
            if pzmax > 1.0 {
                pzmax = 1.0;
                af = 0.0;
                fmax = 1.0;
                fpz = 1.0;
                break 'retry_setup;
            }
            let mut aux3 = 1.0 + 2.0 * jo * pzmax.abs();
            let mut aux4 = 0.5 * (1.0 - aux3 * aux3);
            fpz = 0.5 * RealMath::r_exp(aux4);
            af = qc * (1.0 + br * (br - costhe) / qc2);
            if af < 0.0 {
                if pzmax > 0.0 {
                    fpz = 1.0 - fpz;
                }
                let eta_incoh = rng.uniform();
                if eta_incoh > fpz {
                    if ibcmpl == 1 {
                        return Ok(ComptonOutcome { rejected: true, ..Default::default() });
                    }
                    continue 'resample_compton;
                }
                af = 0.0;
                fmax = 1.0;
                break 'retry_setup;
            }
            let fpz1;
            if pzmax < -0.15 {
                fmax = 1.0 - af * 0.15;
                fpz1 = fpz * fmax * jo;
            } else if pzmax < 0.15 {
                fmax = 1.0 + af * pzmax;
                aux3 = 1.0 / (1.0 + 0.33267252734 * aux3);
                aux4 = fpz * aux3 * (0.3480242 + aux3 * (-0.0958798 + aux3 * 0.7478556)) + data.erf_jo_array[j];
                if pzmax > 0.0 {
                    fpz1 = (1.0 - fmax * fpz) * jo - 0.62665706866 * af * aux4;
                    fpz = 1.0 - fpz;
                } else {
                    fpz1 = fmax * fpz * jo - 0.62665706866 * af * aux4;
                }
            } else {
                fmax = 1.0 + af * 0.15;
                fpz1 = (1.0 - fmax * fpz) * jo;
                fpz = 1.0 - fpz;
            }
            let eta_incoh = rng.uniform();
            if eta_incoh * jo > fpz1 {
                if ibcmpl == 1 {
                    return Ok(ComptonOutcome { rejected: true, ..Default::default() });
                }
                continue 'resample_compton;
            }
        }

        // sample pz between -1 and pzmax from the Compton profile of the shell
        let mut pz;
        if ibcmpl != 2 {
            loop {
                let mut rnno18 = rng.uniform();
                rnno18 *= fpz;
                if rnno18 < 0.5 {
                    rnno18 = (1e-30_f64).max(2.0 * rnno18);
                    pz = 0.5 * (1.0 - Real::sqrt(1.0 - 2.0 * RealMath::r_ln(rnno18))) / jo;
                } else {
                    rnno18 = 2.0 * (1.0 - rnno18);
                    pz = 0.5 * (Real::sqrt(1.0 - 2.0 * RealMath::r_ln(rnno18)) - 1.0) / jo;
                }
                if pz.abs() > 1.0 {
                    continue;
                }
                if pz < 0.15 {
                    let frej = if pz < -0.15 { 1.0 - af * 0.15 } else { 1.0 + af * pz };
                    let eta = rng.uniform();
                    if eta * fmax > frej {
                        continue;
                    }
                }
                break;
            }
        } else {
            pz = 0.0;
            uj = 0.0;
        }

        // energy of the scattered photon
        let pz2 = pz * pz;
        if pz.abs() < 0.01 {
            br *= 1.0 + pz * (qc + (br2 - costhe) * pz);
        } else {
            aux = 1.0 - pz2 * br * costhe;
            let aux1 = 1.0 - pz2 * br2;
            let aux2 = qc2 - br2 * pz2 * sinthe;
            if aux2 > 1e-10 {
                br = br / aux1 * (aux + pz * Real::sqrt(aux2));
            }
        }
        uj *= prm;
        break 'resample_compton;
    }

    // :FINISHED-COMPTON-SAMPLING:
    let pesg = br * peig;
    let pese = peig - pesg - uj + prm;
    sinthe = Real::sqrt(sinthe);
    let mut saved = UphiSaved::default();
    uphi(stack, &mut saved, 2, 1, sinthe, costhe, rng)?;
    let np = stack.np;
    stack.e[np] = pesg;
    let aux = 1.0 + br * br - 2.0 * br * costhe;
    if aux > 1e-8 {
        costhe = (1.0 - br * costhe) / Real::sqrt(aux);
        sinthe = (1.0 - costhe) * (1.0 + costhe);
        if sinthe > 0.0 {
            sinthe = -Real::sqrt(sinthe);
        } else {
            sinthe = 0.0;
        }
    } else {
        costhe = 0.0;
        sinthe = -1.0;
    }
    stack.np += 1;
    stack.check_stack(stack.np, "COMPT")?;
    uphi(stack, &mut saved, 3, 2, sinthe, costhe, rng)?;
    let np = stack.np;
    stack.e[np] = pese;
    stack.iq[np] = -1;

    let mut out = ComptonOutcome::default();
    if ibcmpl == 1 || ibcmpl == 3 {
        if uj > 1e-3 {
            out.vacancy = Some(ShellVacancy { uj, shn: data.shn_array[j], iz: data.iz_array[j] });
        } else {
            out.edep_local = uj;
        }
    }
    if out.vacancy.is_none() {
        if let Some(prob) = rr {
            play_russian_roulette(stack, np_old + 1, prob, rng);
        }
    }
    Ok(out)
}

/// `$MXSHELL`: shells the built-in relaxation treats (K, L1, L2, L3, M, N).
pub const MXSHELL: usize = 6;
/// `$MXTRANS`: possible transitions per element.
pub const MXTRANS: usize = 39;
/// `$MXVAC`: maximum vacancies during one cascade.
pub const MXVAC: usize = 50;
/// `$RELAX-CUTOFF` (`egsnrc.macros:777`): binding energies below this, MeV,
/// are absorbed locally.
pub const RELAX_CUTOFF: f64 = 0.001;

/// `COMMON/EDGE/` as `RELAX` reads it (`egsnrc.macros:501-515`), 1-based:
/// `binding_energies[iz][shell]` (MeV) and `relaxation_prob[iz][transition]`
/// (the cumulative-free probabilities `RELAX` subtracts one by one).
#[derive(Debug, Clone, Default)]
pub struct RelaxData {
    /// `binding_energies(shell, iZ)`: K, L1, L2, L3, M, N.
    pub binding_energies: Vec<Vec<f64>>,
    /// `relaxation_prob(transition, iZ)`.
    pub relaxation_prob: Vec<Vec<f64>>,
}

/// What `RELAX` leaves for the caller.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct RelaxOutcome {
    /// Energy deposited locally (added to the caller's `edep`).
    pub edep: f64,
}

/// `final_state(1..39)` (`egsnrc.mortran:5598-5614`): K fluorescence 4,3,5,6;
/// Auger codes `n1 + 100*n2`; Coster-Kronig the new vacancy plus 10.
const FINAL_STATE: [i32; MXTRANS] = [
    4, 3, 5, 6, 202, 302, 402, 404, 403, 303, 502, 503, 504, 602, 603, 604, 505, 605, 606, 13, 14, 5, 6, 505, 605, 606,
    14, 5, 6, 505, 605, 606, 5, 6, 505, 605, 606, 6, 606,
];
/// `first_transition(1..5)` (`egsnrc.mortran:5596`).
const FIRST_TRANSITION: [usize; 5] = [1, 20, 27, 33, 38];
/// `last_transition(1..5)` (`egsnrc.mortran:5597`).
const LAST_TRANSITION: [usize; 5] = [19, 26, 32, 37, 39];

/// `RELAX` (`egsnrc.mortran:5556-5790`), the built-in scheme (not the EADL
/// tables, `eadl_relax`, whose `egs_eadl_relax` is not ported): fill a vacancy
/// of energy `energy` (MeV) in shell `n` of element `iz` by fluorescence,
/// Coster-Kronig and Auger transitions. Charged and photon products above the
/// cut-offs go on the stack (copying properties of slot `np - 1`, as upstream),
/// the rest accumulates in the returned `edep`.
///
/// `ecut` is the region's `ECUT` (total energy, MeV) and `pcut` its `PCUT`;
/// `prm` is `PRM`. The `COMIN/RELAX-USER/` outputs and the `AUSGAB` calls
/// are omitted.
///
/// # Errors
///
/// [`EgsError::StackExceeded`] when the stack fills.
pub fn relax<R: UniformSource>(
    stack: &mut Stack,
    data: &RelaxData,
    energy: f64,
    n: i32,
    iz: usize,
    ecut: f64,
    pcut: f64,
    prm: f64,
    rng: &mut R,
) -> Result<RelaxOutcome, EgsError> {
    let mut out = RelaxOutcome::default();
    if n < 1 || n as usize > MXSHELL {
        return Ok(out); // unknown vacancy
    }
    let ekcut = ecut - RM;
    let pkcut = pcut;
    let min_e = RELAX_CUTOFF;
    if energy <= min_e {
        out.edep += energy;
        return Ok(out);
    }
    let mut vac_array = [0usize; MXVAC + 1];
    let mut e_array = [0.0_f64; MXVAC + 1];
    let mut n_vac = 1usize;
    vac_array[n_vac] = n as usize;
    e_array[n_vac] = energy;

    'start: loop {
        let shell = vac_array[n_vac];
        let ei = e_array[n_vac];
        n_vac -= 1;

        if ei <= min_e {
            // below cut-off: local absorption
            out.edep += ei;
            if n_vac > 0 {
                continue 'start;
            }
            break;
        }

        if shell == MXSHELL {
            // N-shell vacancy: just produce an Auger electron
            if ei > ekcut {
                relax_push_isotropic(stack, ei + prm, -1, rng)?;
            } else {
                out.edep += ei;
            }
            if n_vac > 0 {
                continue 'start;
            }
            break;
        }

        // sample the transition for this vacancy
        let mut eta = rng.uniform();
        let first = FIRST_TRANSITION[shell - 1];
        let last = LAST_TRANSITION[shell - 1];
        let mut k = first;
        while k <= last - 1 {
            eta -= data.relaxation_prob[iz][k];
            if eta <= 0.0 {
                break;
            }
            k += 1;
        }
        // (if the loop ran out, k == last, as the Mortran DO leaves it)
        let mut final_state = FINAL_STATE[k - 1];
        let finala = final_state;
        let iql;
        let elcut;
        let ex;
        if final_state < 100 {
            if final_state < 10 {
                // fluorescence
                iql = 0;
                elcut = pkcut;
            } else {
                // Coster-Kronig
                final_state -= 10;
                iql = -1;
                elcut = ekcut;
            }
            let ef = data.binding_energies[iz][final_state as usize];
            ex = ei - ef;
            n_vac += 1;
            vac_array[n_vac] = final_state as usize;
            e_array[n_vac] = ef;
        } else {
            // Auger
            let final1 = final_state / 100;
            let final2 = final_state - final1 * 100;
            n_vac += 1;
            vac_array[n_vac] = final1 as usize;
            e_array[n_vac] = data.binding_energies[iz][final1 as usize];
            n_vac += 1;
            vac_array[n_vac] = final2 as usize;
            e_array[n_vac] = data.binding_energies[iz][final2 as usize];
            iql = -1;
            ex = ei - e_array[n_vac] - e_array[n_vac - 1];
            elcut = ekcut;
        }
        if ex <= elcut {
            // below cut-off
            out.edep += ex;
        } else {
            let e_new = if iql == 0 { ex } else { ex + RM };
            relax_push_isotropic(stack, e_new, iql, rng)?;
        }
        let _ = finala;
    }
    Ok(out)
}

/// The particle-creation block `RELAX` repeats (`egsnrc.mortran:5667-5679`,
/// `5766-5777`): push slot `np + 1` with properties of `np`, energy `e` and
/// charge `iq`, direction isotropic (`w` uniform, azimuth by the box method).
fn relax_push_isotropic<R: UniformSource>(
    stack: &mut Stack,
    e: f64,
    iq: i32,
    rng: &mut R,
) -> Result<(), EgsError> {
    stack.np += 1;
    stack.check_stack(stack.np, "RELAX")?;
    let np = stack.np;
    stack.e[np] = e;
    stack.iq[np] = iq;
    stack.transfer_properties(np, np - 1);
    let mut eta = rng.uniform();
    eta = 2.0 * eta - 1.0;
    stack.w[np] = eta;
    eta = (1.0 - eta) * (1.0 + eta);
    if eta > 1e-20 {
        eta = Real::sqrt(eta);
        let (cphi, sphi) = select_azimuthal_angle(rng);
        stack.u[np] = eta * cphi;
        stack.v[np] = eta * sphi;
    } else {
        stack.u[np] = 0.0;
        stack.v[np] = 0.0;
        stack.w[np] = 1.0;
    }
    Ok(())
}

/// `UPHI` with `IENTRY = 1` (`egsnrc.mortran:6759-6769`): `sinthe` and `costhe`
/// from the polar angle `theta`, then the rest of `UPHI` as for `IENTRY = 2`.
/// The sine look-up table of EGS4 is `sin()` in this EGSnrc
/// (`$EVALUATE # USING SIN(#)` is `# = sin(#)`, `egsnrc.macros:1265`), and the
/// cosine is `sin(5 pi/2 - theta)` as upstream writes it.
///
/// # Errors
///
/// As [`uphi`].
pub fn uphi_from_theta<R: UniformSource>(
    stack: &mut Stack,
    saved: &mut UphiSaved,
    lvl: i32,
    theta: f64,
    rng: &mut R,
) -> Result<(), EgsError> {
    let sinthe = RealMath::r_sin(theta);
    let cthet = 2.5 * core::f64::consts::PI - theta;
    let costhe = RealMath::r_sin(cthet);
    uphi(stack, saved, 2, lvl, sinthe, costhe, rng)
}

/// The per-medium pair-production data `PAIR` reads (`COMMON/BREMPR/`, filled
/// by `HATCH`), with 1-based inner arrays (index 0 unused).
#[derive(Debug, Clone, Default)]
pub struct PairMedium {
    /// `delcm`: `136 m exp(Zg)`, eq. (2.7.51).
    pub delcm: f64,
    /// `dl1(l)`..`dl6(l)`, `l = 1..8`: screening-function fits.
    pub dl: [Vec<f64>; 6],
    /// `bpar(1..2)`.
    pub bpar: [f64; 3],
    /// `ZBRANG(MEDIUM) = ((1/111) Zeff^(1/3))^2`.
    pub zbrang: f64,
}

/// Switches `PAIR` reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairConfig {
    /// `iprdst` (default 1): 0 EGS4, 1 lowest-order angular distribution,
    /// 2 Motz-Olsen-Koch, 3 uniform, 4 mixture.
    pub iprdst: i32,
    /// `Some(prob_RR)` when `i_play_RR = 1`: Russian roulette on the pair.
    pub rr: Option<f64>,
    /// `pair_nrc` (default 0): NRC pair tables (not ported; `1` is refused).
    pub pair_nrc: bool,
    /// `itriplet` (default 0): triplet production (not ported; `> 0` is refused).
    pub itriplet: i32,
}

impl Default for PairConfig {
    fn default() -> Self {
        Self { iprdst: 1, rr: None, pair_nrc: false, itriplet: 0 }
    }
}

/// `PAIR` (`egsnrc.mortran:4517-4663`): pair production by the photon on top
/// of the stack. Below 2.1 MeV the energies are uniform; between 2.1 and 50
/// the Bethe-Heitler cross section is sampled, above it the
/// Coulomb-corrected one. The pair lands in slots `np_old` and `np_old + 1`
/// (charges `iq1`, `iq2`). With Russian roulette the pair may be killed before
/// sampling (`killed` in the result).
///
/// `rm` is `RM` (= `PRM`). Not ported, refused with
/// [`EgsError::Unsupported`]: the NRC pair tables (`pair_nrc`) and triplet
/// production (`itriplet > 0`), both off by default.
///
/// # Errors
///
/// [`EgsError::StackExceeded`], or [`EgsError::Unsupported`] as above.
pub fn pair<R: UniformSource>(
    stack: &mut Stack,
    med: &PairMedium,
    cfg: &PairConfig,
    rm: f64,
    rng: &mut R,
) -> Result<bool, EgsError> {
    if cfg.pair_nrc {
        return Err(EgsError::Unsupported("PAIR: NRC pair cross sections (pair_nrc = 1)"));
    }
    if cfg.itriplet > 0 {
        return Err(EgsError::Unsupported("PAIR: triplet production (itriplet > 0)"));
    }
    stack.np_old = stack.np;
    if let Some(prob_rr) = cfg.rr {
        if prob_rr > 0.0 {
            let rnno_rr = rng.uniform();
            if rnno_rr > prob_rr {
                let np = stack.np;
                if np > 1 {
                    stack.np -= 1;
                } else {
                    stack.wt[np] = 0.0;
                    stack.e[np] = 0.0;
                }
                return Ok(true);
            }
            let np = stack.np;
            stack.wt[np] /= prob_rr;
        }
    }
    stack.check_stack(stack.np + 1, "PAIR")?;
    let peig = stack.e[stack.np];
    let eig = peig;
    let rmt2 = 2.0 * rm;
    let (pese1, pese2, iq1, iq2);

    if eig <= 2.1 {
        // below 2.1: uniform energies
        let rnno30 = rng.uniform();
        let rnno34 = rng.uniform();
        let p2 = rm + 0.5 * rnno30 * (peig - 2.0 * rm);
        pese2 = p2;
        pese1 = peig - p2;
        if rnno34 < 0.5 {
            iq1 = -1;
            iq2 = 1;
        } else {
            iq1 = 1;
            iq2 = -1;
        }
    } else {
        let l;
        let (amax, bmax, aux1);
        if eig < 50.0 {
            // Bethe-Heitler without Coulomb correction
            l = 5usize;
            let l1 = l + 1;
            let delta = 4.0 * med.delcm / eig;
            if delta < 1.0 {
                amax = med.dl[0][l] + delta * (med.dl[1][l] + delta * med.dl[2][l]);
                bmax = med.dl[0][l1] + delta * (med.dl[1][l1] + delta * med.dl[2][l1]);
            } else {
                let aux2 = RealMath::r_ln(delta + med.dl[5][l]);
                amax = med.dl[3][l] + med.dl[4][l] * aux2;
                bmax = med.dl[3][l1] + med.dl[4][l1] * aux2;
            }
            let mut a1 = 1.0 - rmt2 / eig;
            a1 *= a1;
            a1 = a1 * amax / 3.0;
            aux1 = a1 / (bmax + a1);
        } else {
            // Coulomb-corrected
            l = 7;
            amax = med.dl[0][l];
            bmax = med.dl[0][l + 1];
            aux1 = med.bpar[2] * (1.0 - med.bpar[1] * rm / eig);
        }
        let del0 = eig * med.delcm;
        let eavail = eig - rmt2;
        let mut eminus;
        loop {
            let rnno30 = rng.uniform();
            let rnno31 = rng.uniform();
            let rnno34 = rng.uniform();
            let br;
            let rejmax;
            let l1;
            if rnno30 > aux1 {
                br = 0.5 * rnno31;
                rejmax = bmax;
                l1 = l + 1;
            } else {
                let rnno32 = rng.uniform();
                let rnno33 = rng.uniform();
                br = 0.5 * (1.0 - rnno31.max(rnno32).max(rnno33));
                rejmax = amax;
                l1 = l;
            }
            eminus = br * eavail + rm;
            let eplus = eig - eminus;
            let delta = del0 / (eminus * eplus);
            let rejf = if delta < 1.0 {
                med.dl[0][l1] + delta * (med.dl[1][l1] + delta * med.dl[2][l1])
            } else {
                med.dl[3][l1] + med.dl[4][l1] * RealMath::r_ln(delta + med.dl[5][l1])
            };
            if rnno34 * rejmax <= rejf {
                break;
            }
        }
        pese2 = eminus;
        pese1 = peig - pese2;
        let rnno34 = rng.uniform();
        if rnno34 < 0.5 {
            iq1 = -1;
            iq2 = 1;
        } else {
            iq1 = 1;
            iq2 = -1;
        }
    }

    // energy going to the lower secondary has now been determined
    let ese2 = pese2;
    let np = stack.np;
    stack.e[np] = pese1;
    stack.e[np + 1] = pese2;
    let mut saved = UphiSaved::default();

    // $SET-PAIR-ANGLE
    if cfg.iprdst > 0 {
        let mut iprdst_use;
        if cfg.iprdst == 4 {
            let rtest = rng.uniform();
            let gbeta = pese1 / (pese1 + 10.0);
            iprdst_use = if rtest < gbeta { 1 } else { 4 };
        } else if cfg.iprdst == 2 && eig < 4.14 {
            iprdst_use = 1;
        } else {
            iprdst_use = cfg.iprdst;
        }
        for ichrg in 1..=2 {
            let ese;
            if ichrg == 1 {
                ese = pese1;
            } else {
                ese = ese2;
                if cfg.iprdst == 4 {
                    let gbeta = ese / (ese + 10.0);
                    let rtest = rng.uniform();
                    iprdst_use = if rtest < gbeta { 1 } else { 4 };
                }
            }
            let (sinthe, costhe);
            if iprdst_use == 1 {
                let pse = Real::sqrt(((ese - rm) * (ese + rm)).max(0.0));
                let mut c = rng.uniform();
                c = 1.0 - 2.0 * c;
                sinthe = rm * Real::sqrt((1.0 - c) * (1.0 + c)) / (pse * c + ese);
                costhe = (ese * c + pse) / (pse * c + ese);
            } else if iprdst_use == 2 {
                let ztarg = med.zbrang;
                let tteig = eig / rm;
                let ttese = ese / rm;
                let esedei = ttese / (tteig - ttese);
                let eseder = 1.0 / esedei;
                let rej = |xi: f64| -> f64 {
                    2.0 + 3.0 * (esedei + eseder)
                        - 4.00
                            * (esedei + eseder + 1.0 - 4.0 * (xi - 0.5) * (xi - 0.5))
                            * (1.0
                                + 0.25
                                    * RealMath::r_ln(
                                        ((1.0 + eseder) * (1.0 + esedei) / (2.0 * tteig))
                                            * ((1.0 + eseder) * (1.0 + esedei) / (2.0 * tteig))
                                            + ztarg * xi * xi,
                                    ))
                };
                let pi_ttese = 3.141593 * ttese;
                let ximin = 1.0 / (1.0 + pi_ttese * pi_ttese);
                let rejmin = rej(ximin);
                let ya = (2.0 / tteig) * (2.0 / tteig);
                let xitry = (0.01_f64).max(ximin.max((0.5_f64).min(Real::sqrt(ya / ztarg))));
                let mut galpha = 1.0 + 0.25 * RealMath::r_ln(ya + ztarg * xitry * xitry);
                let gbeta = 0.5 * ztarg * xitry / (ya + ztarg * xitry * xitry);
                galpha -= gbeta * (xitry - 0.5);
                let mut ximid = galpha / (3.0 * gbeta);
                if galpha >= 0.0 {
                    ximid = 0.5 - ximid + Real::sqrt(ximid * ximid + 0.25);
                } else {
                    ximid = 0.5 - ximid - Real::sqrt(ximid * ximid + 0.25);
                }
                ximid = (0.01_f64).max(ximin.max((0.5_f64).min(ximid)));
                let rejmid = rej(ximid);
                let rejtop = 1.02 * rejmin.max(rejmid);
                let mut theta;
                loop {
                    let xitst = rng.uniform();
                    let rejtst = rej(xitst);
                    let rtest = rng.uniform();
                    theta = Real::sqrt(1.0 / xitst - 1.0) / ttese;
                    let ratio = rejtst / rejtop;
                    if rtest <= ratio && theta < core::f64::consts::PI {
                        break;
                    }
                }
                sinthe = RealMath::r_sin(theta);
                costhe = RealMath::r_cos(theta);
            } else if iprdst_use == 3 {
                let c = 1.0 - 2.0 * rng.uniform();
                let s = (1.0 - c) * (1.0 + c);
                sinthe = if s > 0.0 { Real::sqrt(s) } else { 0.0 };
                costhe = c;
            } else {
                let c = 1.0 - 2.0 * Real::sqrt(rng.uniform());
                let s = (1.0 - c) * (1.0 + c);
                sinthe = if s > 0.0 { Real::sqrt(s) } else { 0.0 };
                costhe = c;
            }
            if ichrg == 1 {
                uphi(stack, &mut saved, 2, 1, sinthe, costhe, rng)?;
            } else {
                stack.np += 1;
                uphi(stack, &mut saved, 3, 2, -sinthe, costhe, rng)?;
            }
        }
        let np = stack.np;
        stack.iq[np] = iq2;
        stack.iq[np - 1] = iq1;
        return Ok(false);
    }
    // iprdst = 0: theta = 0
    uphi_from_theta(stack, &mut saved, 1, 0.0, rng)?;
    stack.np += 1;
    let sinthe = -RealMath::r_sin(0.0);
    let costhe = RealMath::r_sin(2.5 * core::f64::consts::PI);
    uphi(stack, &mut saved, 3, 2, sinthe, costhe, rng)?;
    let np = stack.np;
    stack.iq[np] = iq2;
    stack.iq[np - 1] = iq1;
    Ok(false)
}

/// `$MXEDGE` (`egsnrc.macros:495`): maximum photo-absorption edges above 1 keV.
pub const MXEDGE: usize = 16;
/// `$MXINTER` (`egsnrc.macros:493`): `$MXSHELL - 1`.
pub const MXINTER: usize = MXSHELL - 1;

/// The photoelectric data `PHOTO` reads from `COMMON/EDGE/`
/// (`egsnrc.macros:501-522`), 1-based `[iz][edge]` with every inner array
/// sized `$MXEDGE + 1` (upstream reads one past `edge_number` when no edge
/// matches, and so may this port).
#[derive(Debug, Clone, Default)]
pub struct PhotoData {
    /// `edge_energies(edge, iZ)`, MeV.
    pub edge_energies: Vec<Vec<f64>>,
    /// `edge_number(iZ)`.
    pub edge_number: Vec<i32>,
    /// `edge_a(edge, iZ)`.
    pub edge_a: Vec<Vec<f64>>,
    /// `edge_b`.
    pub edge_b: Vec<Vec<f64>>,
    /// `edge_c`.
    pub edge_c: Vec<Vec<f64>>,
    /// `edge_d`.
    pub edge_d: Vec<Vec<f64>>,
    /// `interaction_prob(shell, iZ)`, shells K..M (1..5).
    pub interaction_prob: Vec<Vec<f64>>,
}

/// The medium's elements (`nne`, `zelem`, `pz`), 1-based.
#[derive(Debug, Clone, Default)]
pub struct PhotoMedium {
    /// `zelem(medium, k)`: atomic number of element `k`.
    pub zelem: Vec<f64>,
    /// `pz(medium, k)`: number of atoms of element `k`.
    pub pz: Vec<f64>,
}

/// Per-region switches `PHOTO` reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhotoRegion {
    /// `iedgfl`: 0 no atomic relaxation, otherwise relaxation on.
    pub iedgfl: i32,
    /// `iphter`: 1 samples the photoelectron angular distribution.
    pub iphter: i32,
    /// `ecut(region)`, total energy MeV.
    pub ecut: f64,
    /// `pcut(region)`, MeV.
    pub pcut: f64,
}

/// What `PHOTO` leaves for the caller.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PhotoOutcome {
    /// Energy deposited locally by `RELAX`.
    pub edep: f64,
}

/// `$SELECT-PHOTOELECTRON-DIRECTION` (`egsnrc.macros:1916-1953`): Sauter-type
/// rejection for the photoelectron angle; the electron on top of the stack
/// (total energy `e[np]`) gets its direction rotated about the photon's.
/// Does nothing unless `iphter == 1` and the electron is above `ecut`.
///
/// # Errors
///
/// As [`uphi`].
pub fn select_photoelectron_direction<R: UniformSource>(
    stack: &mut Stack,
    iphter: i32,
    ecut: f64,
    rng: &mut R,
) -> Result<(), EgsError> {
    if iphter != 1 {
        return Ok(());
    }
    let np = stack.np;
    let eelec = stack.e[np];
    if !(eelec > ecut) {
        return Ok(());
    }
    let rm = RM;
    let beta = Real::sqrt((eelec - rm) * (eelec + rm)) / eelec;
    let gamma = eelec / rm;
    let alpha = 0.5 * gamma - 0.5 + 1.0 / gamma;
    let ratio = beta / alpha;
    let (mut costhe, mut sinth2);
    loop {
        let mut rnpht = rng.uniform();
        rnpht = 2.0 * rnpht - 1.0;
        let xi;
        if ratio <= 0.2 {
            let fkappa = rnpht + 0.5 * ratio * (1.0 - rnpht) * (1.0 + rnpht);
            if gamma < 100.0 {
                costhe = (beta + fkappa) / (1.0 + beta * fkappa);
            } else if fkappa > 0.0 {
                let g1 = gamma - 1.0;
                costhe = 1.0 - (1.0 - fkappa) * (gamma - 3.0) / (2.0 * (1.0 + fkappa) * g1 * g1 * g1);
            } else {
                costhe = (beta + fkappa) / (1.0 + beta * fkappa);
            }
            xi = (1.0 + beta * fkappa) * gamma * gamma;
        } else {
            xi = gamma * gamma * (1.0 + alpha * (Real::sqrt(1.0 + ratio * (2.0 * rnpht + ratio)) - 1.0));
            costhe = (1.0 - 1.0 / xi) / beta;
        }
        sinth2 = ((1.0 - costhe) * (1.0 + costhe)).max(0.0);
        let rnpht2 = rng.uniform();
        if !(rnpht2 > 0.5 * (1.0 + gamma) * sinth2 * xi / gamma) {
            break;
        }
    }
    let sinthe = Real::sqrt(sinth2);
    let mut saved = UphiSaved::default();
    uphi(stack, &mut saved, 2, 1, sinthe, costhe, rng)
}

/// `PHOTO` (`egsnrc.mortran:5049-5171`): photoelectric absorption of the
/// photon on top of the stack, which becomes a photoelectron (`iq = -1`, total
/// energy `e - binding + prm`). With relaxation on (`iedgfl`) the element is
/// sampled from the fits in `data`, then the shell; a K..M vacancy goes to
/// [`relax`]. `gle` is `ln(E)` of the photon (the caller's `GLE`).
///
/// Not ported: the shellwise MCDF branch (`mcdf_pe_xsections`,
/// `egs_shellwise_photo`) and the EADL branch (`eadl_relax`), both off by
/// default. The below-1 keV warning is not printed. If the element or edge
/// loops run off their end (round-off), the index is clamped to the last
/// element instead of reading past the arrays.
///
/// # Errors
///
/// [`EgsError::StackExceeded`] from `RELAX`, or an atomic number outside
/// `1..=100` as [`EgsError::Unsupported`] (upstream `$egs_fatal`).
#[allow(clippy::too_many_arguments)]
pub fn photo<R: UniformSource>(
    stack: &mut Stack,
    data: &PhotoData,
    relax_data: &RelaxData,
    medium: &PhotoMedium,
    region: &PhotoRegion,
    gle: f64,
    rr: Option<f64>,
    prm: f64,
    rng: &mut R,
) -> Result<PhotoOutcome, EgsError> {
    let mut out = PhotoOutcome::default();
    stack.np_old = stack.np;
    let np = stack.np;
    let peig = stack.e[np];
    if peig < data.edge_energies[1][2] {
        // below the minimum energy of 1 keV: convert to an electron
        stack.iq[np] = -1;
        stack.e[np] = peig + prm;
        return Ok(out);
    }
    let mut do_relax = false;
    let (mut e_vac, mut k_shell, mut iz) = (0.0, 0usize, 0usize);
    if region.iedgfl != 0 {
        let nne = medium.zelem.len() - 1;
        if nne == 1 {
            iz = (medium.zelem[1] + 0.5) as usize;
        } else {
            let mut aux = peig * peig;
            let aux1 = aux * peig;
            aux *= Real::sqrt(peig);
            let mut sigtot = 0.0;
            let mut probs = alloc::vec![0.0; nne + 1];
            for k in 1..=nne {
                let izk = (medium.zelem[k] + 0.5) as i64;
                if izk < 1 || izk > 100 {
                    return Err(EgsError::Unsupported("PHOTO: atomic number not between 1 and 100"));
                }
                let izk = izk as usize;
                let mut sigma;
                if peig > data.edge_energies[izk][1] {
                    sigma = (data.edge_a[izk][1]
                        + data.edge_b[izk][1] / peig
                        + data.edge_c[izk][1] / aux
                        + data.edge_d[izk][1] / aux1)
                        / peig;
                } else {
                    let mut j = 2usize;
                    while j <= data.edge_number[izk] as usize {
                        if peig >= data.edge_energies[izk][j] {
                            break;
                        }
                        j += 1;
                    }
                    sigma = data.edge_a[izk][j]
                        + gle * (data.edge_b[izk][j] + gle * (data.edge_c[izk][j] + gle * data.edge_d[izk][j]));
                    sigma = RealMath::r_exp(sigma);
                }
                sigma *= medium.pz[k];
                sigtot += sigma;
                probs[k] = sigma;
            }
            let mut br = rng.uniform();
            br *= sigtot;
            let mut k = 1usize;
            while k <= nne {
                br -= probs[k];
                if br <= 0.0 {
                    break;
                }
                k += 1;
            }
            let k = k.min(nne);
            iz = (medium.zelem[k] + 0.5) as usize;
        }
        // sample the shell
        if peig <= relax_data.binding_energies[iz][MXSHELL] {
            // outer shells, no atomic relaxation
            stack.iq[np] = -1;
            stack.e[np] = peig + prm;
        } else {
            let mut br = rng.uniform();
            let mut k = 1usize;
            while k <= MXINTER {
                if peig > relax_data.binding_energies[iz][k] {
                    if br < data.interaction_prob[iz][k] {
                        break;
                    }
                    br = (br - data.interaction_prob[iz][k]) / (1.0 - data.interaction_prob[iz][k]);
                }
                k += 1;
            }
            // (defaults to the MXSHELL shell if the loop completes)
            e_vac = relax_data.binding_energies[iz][k];
            k_shell = k;
            stack.e[np] = peig - e_vac + prm;
            do_relax = true;
            stack.iq[np] = -1;
        }
    } else {
        stack.e[np] = peig + prm;
        stack.iq[np] = -1;
    }

    if stack.iq[np] == -1 {
        select_photoelectron_direction(stack, region.iphter, region.ecut, rng)?;
    }
    if do_relax {
        let r = relax(stack, relax_data, e_vac, k_shell as i32, iz, region.ecut, region.pcut, prm, rng)?;
        out.edep += r.edep;
    }
    if let Some(prob) = rr {
        play_russian_roulette(stack, stack.np_old, prob, rng);
    }
    Ok(out)
}

/// `BHABHA` (`egsnrc.mortran:229-309`): discrete Bhabha scattering of the
/// positron on top of the stack, with a secondary "electron" above the
/// threshold `te` (kinetic energy, MeV, `TE(MEDIUM)`). The exact Bhabha
/// cross section is sampled; the angles follow from the kinematics. The
/// incident positron keeps slot `np_old`, the other particle goes to
/// `np_old + 1`, the lower-energy one on top and positively charged if it is
/// the positron.
///
/// # Errors
///
/// [`EgsError::StackExceeded`] when `np + 1 > $MXSTACK`.
pub fn bhabha<R: UniformSource>(stack: &mut Stack, te: f64, prm: f64, rng: &mut R) -> Result<(), EgsError> {
    let rm = RM;
    stack.np_old = stack.np;
    let np = stack.np;
    let peip = stack.e[np];
    let pekin = peip - prm;
    let ekin = pekin;
    let t0 = ekin / rm;
    let e0 = t0 + 1.0;
    let yy = 1.0 / (t0 + 2.0);
    let e02 = e0 * e0;
    let beta2 = (e02 - 1.0) / e02;
    let ep0 = te / ekin;
    let ep0c = 1.0 - ep0;
    let y2 = yy * yy;
    let yp = 1.0 - 2.0 * yy;
    let yp2 = yp * yp;
    let b4 = yp2 * yp;
    let b3 = b4 + yp2;
    let b2 = yp * (3.0 + y2);
    let b1 = 2.0 - y2;
    let mut br;
    loop {
        let rnno03 = rng.uniform();
        br = ep0 / (1.0 - ep0c * rnno03);
        let rnno04 = rng.uniform();
        let rejf2 = 1.0 - beta2 * br * (b1 - br * (b2 - br * (b3 - br * b4)));
        if rnno04 <= rejf2 {
            break;
        }
    }
    stack.check_stack(stack.np + 1, "BHABHA")?;
    if br < 0.5 {
        stack.iq[np + 1] = -1;
    } else {
        stack.iq[np] = -1;
        stack.iq[np + 1] = 1;
        br = 1.0 - br;
    }
    br = br.max(0.0);
    let pekse2 = br * ekin;
    let pese1 = peip - pekse2;
    let pese2 = pekse2 + prm;
    stack.e[np] = pese1;
    stack.e[np + 1] = pese2;
    // Bhabha angles are uniquely determined by kinematics
    let h1 = (peip + prm) / pekin;
    let mut dcosth = (1.0_f64).min(h1 * (pese1 - prm) / (pese1 + prm));
    let mut sinthe = Real::sqrt(1.0 - dcosth);
    let mut costhe = Real::sqrt(dcosth);
    let mut saved = UphiSaved::default();
    uphi(stack, &mut saved, 2, 1, sinthe, costhe, rng)?;
    stack.np += 1;
    dcosth = h1 * (pese2 - prm) / (pese2 + prm);
    sinthe = -Real::sqrt(1.0 - dcosth);
    costhe = Real::sqrt(dcosth);
    uphi(stack, &mut saved, 3, 2, sinthe, costhe, rng)
}

/// `MOLLER` (`egsnrc.mortran:2598-2745`): discrete Moller scattering of the
/// electron on top of the stack, with a secondary above the threshold `te`
/// (kinetic energy, `TE(MEDIUM)`); `thmoll = 2 te + rm` is `THMOLL(MEDIUM)`.
/// Returns `false` without scattering when `ekin <= 2 te`. The secondary
/// goes to `np_old + 1` with `iq = -1`.
///
/// Not ported, refused with [`EgsError::Unsupported`] when `eii_active`
/// (`eii_flag > 0` and the medium has EII shells): electron-impact ionisation
/// sampling (`eii_sample`, `eii.mortran`). It is off by default.
///
/// # Errors
///
/// [`EgsError::StackExceeded`], or [`EgsError::Unsupported`] as above.
pub fn moller<R: UniformSource>(
    stack: &mut Stack,
    te: f64,
    thmoll: f64,
    eii_active: bool,
    prm: f64,
    rng: &mut R,
) -> Result<bool, EgsError> {
    if eii_active {
        return Err(EgsError::Unsupported("MOLLER: electron-impact ionisation (eii_flag > 0)"));
    }
    let rm = RM;
    stack.np_old = stack.np;
    let np = stack.np;
    let peie = stack.e[np];
    let pekin = peie - prm;
    let ekin = pekin;
    if ekin <= 2.0 * te {
        return Ok(false);
    }
    let t0 = ekin / rm;
    let e0 = t0 + 1.0;
    let extrae = peie - thmoll;
    let e02 = e0 * e0;
    let g2 = t0 * t0 / e02;
    let g3 = (2.0 * t0 + 1.0) / e02;
    let gmax = 1.0 + 1.25 * g2;
    let mut br;
    loop {
        let rnno27 = rng.uniform();
        br = te / (ekin - extrae * rnno27);
        let r = br / (1.0 - br);
        let mut rnno28 = rng.uniform();
        let rejf4 = 1.0 + g2 * br * br + r * (r - g3);
        rnno28 *= gmax;
        if rnno28 <= rejf4 {
            break;
        }
    }
    let pekse2 = br * ekin;
    let pese1 = peie - pekse2;
    let pese2 = pekse2 + prm;
    stack.e[np] = pese1;
    stack.check_stack(stack.np + 1, "MOLLER")?;
    stack.e[np + 1] = pese2;
    let h1 = (peie + prm) / pekin;
    let mut dcosth = h1 * (pese1 - prm) / (pese1 + prm);
    let mut sinthe = Real::sqrt(1.0 - dcosth);
    let mut costhe = Real::sqrt(dcosth);
    let mut saved = UphiSaved::default();
    uphi(stack, &mut saved, 2, 1, sinthe, costhe, rng)?;
    stack.np += 1;
    let np = stack.np;
    stack.iq[np] = -1;
    dcosth = h1 * (pese2 - prm) / (pese2 + prm);
    sinthe = -Real::sqrt(1.0 - dcosth);
    costhe = Real::sqrt(dcosth);
    uphi(stack, &mut saved, 3, 2, sinthe, costhe, rng)?;
    Ok(true)
}

/// The per-medium bremsstrahlung data `BREMS` reads (`COMMON/BREMPR/`, filled
/// by `HATCH` and `fix_brems`), `dl` 1-based as in [`PairMedium`].
#[derive(Debug, Clone, Default)]
pub struct BremsMedium {
    /// `delcm`.
    pub delcm: f64,
    /// `dl1(l)`..`dl6(l)`.
    pub dl: [Vec<f64>; 6],
    /// `ZBRANG(MEDIUM)`.
    pub zbrang: f64,
    /// `LZBRANG(MEDIUM) = log(zbrang)`.
    pub lzbrang: f64,
    /// `AP(MEDIUM)`: photon production threshold, MeV.
    pub ap: f64,
    /// `log_ap(MEDIUM) = log(ap)`.
    pub log_ap: f64,
}

/// Switches `BREMS` reads (`COMMON/BREMPR/`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BremsConfig {
    /// `nbr_split` (default 1): photons per event; `0` turns bremsstrahlung off.
    pub nbr_split: usize,
    /// `ibrdst` (default 1): 1 Koch-Motz 2BS, 0 leading term, `< 0` photon
    /// inherits the electron direction.
    pub ibrdst: i32,
    /// `ibr_nist` (default 0): 0 Bethe-Heitler; 1 and 2 (NIST / NRC alias
    /// tables) are not ported and are refused.
    pub ibr_nist: i32,
}

impl Default for BremsConfig {
    fn default() -> Self {
        Self { nbr_split: 1, ibrdst: 1, ibr_nist: 0 }
    }
}

/// `BREMS` (`egsnrc.mortran:312-525`): bremsstrahlung by the electron on top
/// of the stack. `nbr_split` photons of weight `wt/nbr_split` are pushed (energy
/// from Bethe-Heitler, `L = 1` below 50 MeV and `3` above), the electron
/// keeps slot `np_old` with `e = peie - e_photon(last)`. `elke` is
/// `ln(E_kinetic)` as `ELECTR` computes it.
///
/// Not ported, refused with [`EgsError::Unsupported`]: the NIST/NRC
/// cross-section alias tables (`ibr_nist >= 1`).
///
/// # Errors
///
/// [`EgsError::StackExceeded`], or [`EgsError::Unsupported`] as above.
pub fn brems<R: UniformSource>(
    stack: &mut Stack,
    med: &BremsMedium,
    cfg: &BremsConfig,
    elke: f64,
    prm: f64,
    rng: &mut R,
) -> Result<(), EgsError> {
    if cfg.nbr_split < 1 {
        return Ok(());
    }
    if cfg.ibr_nist >= 1 {
        return Err(EgsError::Unsupported("BREMS: NIST/NRC bremsstrahlung tables (ibr_nist >= 1)"));
    }
    let rm = RM;
    stack.np_old = stack.np;
    let npold = stack.np_old;
    let peie = stack.e[stack.np];
    let eie = peie;
    let weight = stack.wt[stack.np] / cfg.nbr_split as f64;
    let l = if eie < 50.0 { 1usize } else { 3usize };
    let l1 = l + 1;
    let ekin = peie - prm;
    let brmin = med.ap / ekin;
    let waux = elke - med.log_ap;

    let (mut a, mut b, mut c) = (0.0, 0.0, 0.0);
    let mut sinpsi = 0.0;
    let (mut sindel, mut cosdel) = (0.0, 0.0);
    let mut ztarg = 0.0;
    let mut tteie = 0.0;
    let mut y2max = 0.0;
    let mut y2maxi = 0.0;
    let (mut z2max, mut z2maxi) = (0.0, 0.0);
    if cfg.ibrdst >= 0 {
        let np = stack.np;
        a = stack.u[np];
        b = stack.v[np];
        c = stack.w[np];
        sinpsi = a * a + b * b;
        if sinpsi > 1e-20 {
            sinpsi = Real::sqrt(sinpsi);
            sindel = b / sinpsi;
            cosdel = a / sinpsi;
        }
        ztarg = med.zbrang;
        tteie = eie / rm;
        let beta = Real::sqrt((tteie - 1.0) * (tteie + 1.0)) / tteie;
        y2max = 2.0 * beta * (1.0 + beta) * tteie * tteie;
        y2maxi = 1.0 / y2max;
        if cfg.ibrdst == 1 {
            z2max = y2max + 1.0;
            z2maxi = Real::sqrt(z2max);
        }
    }
    let _ = z2max;

    let mut pese = peie;
    for _ in 1..=cfg.nbr_split {
        // Bethe-Heitler
        let (esg, ese);
        loop {
            let rnno06 = rng.uniform();
            let rnno07 = rng.uniform();
            let br = brmin * RealMath::r_exp(rnno06 * waux);
            let e_g = ekin * br;
            let e_e = peie - e_g;
            let delta = e_g / eie / e_e * med.delcm;
            let aux = e_e / eie;
            let (phi1, phi2);
            if delta < 1.0 {
                phi1 = med.dl[0][l] + delta * (med.dl[1][l] + delta * med.dl[2][l]);
                phi2 = med.dl[0][l1] + delta * (med.dl[1][l1] + delta * med.dl[2][l1]);
            } else {
                phi1 = med.dl[3][l] + med.dl[4][l] * RealMath::r_ln(delta + med.dl[5][l]);
                phi2 = phi1;
            }
            let rejf = (1.0 + aux * aux) * phi1 - 2.0 * aux * phi2 / 3.0;
            if rnno07 < rejf {
                esg = e_g;
                ese = e_e;
                break;
            }
        }
        pese = peie - esg;

        // set up the new photon
        stack.np += 1;
        if stack.np > stack.mxstack() {
            return Err(EgsError::StackExceeded { routine: "BREMS", needed: stack.np, mxstack: stack.mxstack() });
        }
        let np = stack.np;
        stack.e[np] = esg;
        stack.iq[np] = 0;
        stack.transfer_properties(np, np - 1);
        stack.wt[np] = weight;
        if cfg.ibrdst < 0 {
            stack.u[np] = stack.u[npold];
            stack.v[np] = stack.v[npold];
            stack.w[np] = stack.w[npold];
        } else {
            let mut y2tst;
            if cfg.ibrdst == 1 {
                let ttese = ese / rm;
                let esedei = ttese / tteie;
                let rjarg1 = 1.0 + esedei * esedei;
                let rjarg2 = rjarg1 + 2.0 * esedei;
                let mut aux = 2.0 * ese * tteie / esg;
                aux *= aux;
                let aux1 = aux * ztarg;
                let rjarg3 = if aux1 > 10.0 {
                    med.lzbrang + (1.0 - aux1) / (aux1 * aux1)
                } else {
                    RealMath::r_ln(aux / (1.0 + aux1))
                };
                let rejmax = rjarg1 * rjarg3 - rjarg2;
                loop {
                    y2tst = rng.uniform();
                    let mut rtest = rng.uniform();
                    let aux3 = z2maxi / (y2tst + (1.0 - y2tst) * z2maxi);
                    rtest *= aux3 * rejmax;
                    y2tst = aux3 * aux3 - 1.0;
                    let a34 = aux3 * aux3 * aux3 * aux3;
                    let y2tst1 = esedei * y2tst / a34;
                    let aux4 = 16.0 * y2tst1 - rjarg2;
                    let aux5 = rjarg1 - 4.0 * y2tst1;
                    if rtest < aux4 + aux5 * rjarg3 {
                        break;
                    }
                    let aux2 = RealMath::r_ln(aux / (1.0 + aux1 / a34));
                    let rejtst = aux4 + aux5 * aux2;
                    if rtest < rejtst {
                        break;
                    }
                }
            } else {
                y2tst = rng.uniform();
                y2tst /= 1.0 - y2tst + y2maxi;
            }
            let costhe = 1.0 - 2.0 * y2tst * y2maxi;
            let sinthe = Real::sqrt(((1.0 - costhe) * (1.0 + costhe)).max(0.0));
            let (cphi, sphi) = select_azimuthal_angle(rng);
            if sinpsi >= 1e-10 {
                let us = sinthe * cphi;
                let vs = sinthe * sphi;
                stack.u[np] = c * cosdel * us - sindel * vs + a * costhe;
                stack.v[np] = c * sindel * us + cosdel * vs + b * costhe;
                stack.w[np] = c * costhe - sinpsi * us;
            } else {
                stack.u[np] = sinthe * cphi;
                stack.v[np] = sinthe * sphi;
                stack.w[np] = c * costhe;
            }
        }
    }
    stack.e[npold] = pese;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use petir::rng::ranmar::Ranmar;

    fn norm(s: &Stack, i: usize) -> f64 {
        s.u[i] * s.u[i] + s.v[i] * s.v[i] + s.w[i] * s.w[i]
    }

    #[test]
    fn annihilation_at_rest_makes_back_to_back_pairs() {
        let mut rng = Ranmar::new(1802, 9373);
        let mut st = Stack::new(40);
        st.np = 1;
        st.iq[1] = 1;
        st.e[1] = 0.511;
        st.wt[1] = 2.0;
        st.ir[1] = 4;
        st.x[1] = 1.5;
        annih_at_rest(&mut st, 1, 0.5110034, &mut rng).unwrap();
        assert_eq!((st.np_old, st.np), (1, 2));
        for i in 1..=2 {
            assert_eq!((st.iq[i], st.e[i], st.wt[i], st.ir[i], st.x[i]), (0, 0.5110034, 2.0, 4, 1.5));
            assert!((norm(&st, i) - 1.0).abs() < 1e-12);
        }
        assert_eq!((st.u[1], st.v[1], st.w[1]), (-st.u[2], -st.v[2], -st.w[2]));
    }

    #[test]
    fn annihilation_with_radiative_splitting() {
        let mut rng = Ranmar::new(1, 2);
        let mut st = Stack::new(40);
        st.np = 3;
        st.wt[3] = 6.0;
        annih_at_rest(&mut st, 3, 0.511, &mut rng).unwrap();
        assert_eq!(st.np, 3 + 2 * 3 - 1);
        // the parent's slot and the five above it are photons of weight 6/3
        for i in 3..=8 {
            assert_eq!((st.iq[i], st.wt[i]), (0, 2.0));
        }
        let mut full = Stack::new(5);
        full.np = 5;
        assert_eq!(
            annih_at_rest(&mut full, 2, 0.511, &mut rng),
            Err(EgsError::StackExceeded { routine: "ANNIH_AT_REST", needed: 8, mxstack: 5 })
        );
    }


    #[test]
    fn annihilation_in_flight_conserves_energy_and_momentum() {
        let mut rng = Ranmar::new(3, 4);
        let rm = 0.5110034;
        // `e[np]` is the positron's TOTAL energy in EGSnrc (>= rm).
        for &(etot, u, v) in &[(0.6, 0.0, 0.0), (2.0, 0.6, 0.0), (10.0, 0.3, -0.4)] {
            let mut st = Stack::new(40);
            st.np = 1;
            st.e[1] = etot;
            st.iq[1] = 1;
            st.u[1] = u;
            st.v[1] = v;
            st.w[1] = (1.0f64 - u * u - v * v).sqrt();
            st.wt[1] = 1.0;
            let w0 = st.w[1];
            annih(&mut st, 1, rm, &mut rng).unwrap();
            assert_eq!((st.np_old, st.np), (1, 2));
            assert!((st.e[1] + st.e[2] - (etot + rm)).abs() < 1e-12);
            // photons carry the incident momentum (electron at rest)
            let p = (etot * etot - rm * rm).sqrt();
            let px = st.e[1] * st.u[1] + st.e[2] * st.u[2];
            let pz = st.e[1] * st.w[1] + st.e[2] * st.w[2];
            assert!((px - p * u).abs() < 1e-9, "px {px} vs {}", p * u);
            assert!((pz - p * w0).abs() < 1e-9);
            for i in 1..=2 {
                assert!((norm(&st, i) - 1.0).abs() < 1e-12);
                assert_eq!(st.iq[i], 0);
            }
        }
        // splitting: 2*n photons, weight divided; zero split is a no-op
        let mut st = Stack::new(40);
        st.np = 1;
        st.e[1] = 1.0;
        st.w[1] = 1.0;
        st.wt[1] = 4.0;
        annih(&mut st, 2, rm, &mut rng).unwrap();
        assert_eq!(st.np, 4);
        assert!((1..=4).all(|i| st.wt[i] == 2.0));
        let mut z = Stack::new(40);
        z.np = 1;
        annih(&mut z, 0, rm, &mut rng).unwrap();
        assert_eq!((z.np, z.np_old), (1, 1));
        let mut full = Stack::new(1);
        full.np = 1;
        full.e[1] = 1.0;
        assert!(matches!(annih(&mut full, 1, rm, &mut rng), Err(EgsError::StackExceeded { routine: "ANNIH", .. })));
    }

    #[test]
    fn uphi_rotation_preserves_norm_and_polar_angle() {
        let mut rng = Ranmar::new(77, 88);
        let mut st = Stack::new(10);
        st.np = 1;
        let (u, v, w) = (0.3, -0.4, (1.0f64 - 0.25).sqrt());
        st.u[1] = u;
        st.v[1] = v;
        st.w[1] = w;
        let costhe = 0.6;
        let sinthe = 0.8;
        let mut saved = UphiSaved::default();
        uphi(&mut st, &mut saved, 2, 1, sinthe, costhe, &mut rng).unwrap();
        assert!((norm(&st, 1) - 1.0).abs() < 1e-12);
        let dot = st.u[1] * u + st.v[1] * v + st.w[1] * w;
        assert!((dot - costhe).abs() < 1e-12, "angle to the old direction is theta");
        // lvl 2: a new particle above, using the saved direction
        st.np = 2;
        uphi(&mut st, &mut saved, 2, 2, sinthe, costhe, &mut rng).unwrap();
        let dot2 = st.u[2] * u + st.v[2] * v + st.w[2] * w;
        assert!((dot2 - costhe).abs() < 1e-12);
        // lvl 3: brems photon, relative to the electron at np-1
        st.np = 3;
        uphi(&mut st, &mut saved, 2, 3, sinthe, costhe, &mut rng).unwrap();
        let dot3 = st.u[3] * st.u[2] + st.v[3] * st.v[2] + st.w[3] * st.w[2];
        assert!((dot3 - costhe).abs() < 1e-12);
        assert_eq!(uphi(&mut st, &mut saved, 2, 4, sinthe, costhe, &mut rng), Err(EgsError::BadEntry { ientry: 2, lvl: 4 }));
    }


    fn toy_compton_data(be: f64) -> ComptonData {
        // one medium (index 1), one shell (index 1)
        ComptonData {
            iz_array: alloc::vec![0, 6],
            be_array: alloc::vec![0.0, be],
            jo_array: alloc::vec![0.0, 10.0],
            erf_jo_array: alloc::vec![0.0, 0.9],
            shn_array: alloc::vec![0, 1],
            shell_array: alloc::vec![Vec::new(), alloc::vec![0, 1]],
            eno_array: alloc::vec![Vec::new(), alloc::vec![0.0, 1.0]],
            eno_atbin_array: alloc::vec![Vec::new(), alloc::vec![0, 1]],
            n_shell: alloc::vec![0, 1],
        }
    }

    fn photon_stack(e: f64) -> Stack {
        let mut st = Stack::new(40);
        st.np = 1;
        st.e[1] = e;
        st.w[1] = 1.0;
        st.wt[1] = 1.0;
        st.ir[1] = 1;
        st
    }

    #[test]
    fn compt_klein_nishina_conserves_energy_and_momentum() {
        let mut rng = Ranmar::new(10, 20);
        let data = toy_compton_data(0.0);
        // below and above the ko = 2 switch between the two br samplers
        for &e in &[0.3, 0.9, 1.2, 5.0] {
            for _ in 0..40 {
                let mut st = photon_stack(e);
                let o = compt(&mut st, &data, 1, 1, 0, None, RM, &mut rng).unwrap();
                assert_eq!(o, ComptonOutcome::default());
                assert_eq!((st.np_old, st.np), (1, 2));
                assert_eq!((st.iq[1], st.iq[2]), (0, -1));
                assert!((st.e[1] + st.e[2] - (e + RM)).abs() < 1e-12);
                let pe = (st.e[2] * st.e[2] - RM * RM).max(0.0).sqrt();
                let px = st.e[1] * st.u[1] + pe * st.u[2];
                let pz = st.e[1] * st.w[1] + pe * st.w[2];
                assert!(px.abs() < 1e-5 * e, "px {px}");
                assert!((pz - e).abs() < 1e-5 * e, "pz {pz} vs {e}");
                assert!((norm(&st, 1) - 1.0).abs() < 1e-9 && (norm(&st, 2) - 1.0).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn compt_bound_modes_reject_resample_and_leave_vacancies() {
        let mut rng = Ranmar::new(30, 40);
        // binding energy 0.002 rm... large enough that Uj*prm > 1e-3 MeV
        let data = toy_compton_data(0.01);
        // photon below the binding energy: ibcmp 1 rejects (unscattered)
        let mut st = photon_stack(0.001);
        let o = compt(&mut st, &data, 1, 1, 1, None, RM, &mut rng).unwrap();
        assert!(o.rejected && st.np == 1 && st.np_old == 1);
        let (mut ok, mut rej, mut vac) = (0, 0, 0);
        for _ in 0..200 {
            let mut st = photon_stack(0.3);
            let o = compt(&mut st, &data, 1, 1, 1, None, RM, &mut rng).unwrap();
            if o.rejected {
                rej += 1;
                assert_eq!(st.np, 1);
            } else {
                ok += 1;
                assert_eq!(st.np, 2);
                if let Some(v) = o.vacancy {
                    vac += 1;
                    assert_eq!((v.iz, v.shn), (6, 1));
                    assert!(v.uj > 1e-3);
                    // electron energy lost the binding energy
                    assert!(st.e[1] + st.e[2] + v.uj - (0.3 + RM) < 1e-9);
                }
            }
        }
        assert!(ok > 0 && rej > 0 && vac == ok, "ok {ok} rej {rej} vac {vac}");
        // ibcmp 3 resamples instead of rejecting, ibcmp 2 has no binding energy
        for &mode in &[2, 3] {
            for _ in 0..100 {
                let mut st = photon_stack(0.3);
                let o = compt(&mut st, &data, 1, 1, mode, None, RM, &mut rng).unwrap();
                assert!(!o.rejected && st.np == 2);
                assert_eq!(o.vacancy.is_some(), mode == 3);
            }
        }
        // a tiny binding energy is deposited locally, not relaxed
        let small = toy_compton_data(1e-5);
        let mut st = photon_stack(0.5);
        let o = compt(&mut st, &small, 1, 1, 3, None, RM, &mut rng).unwrap();
        assert!(o.vacancy.is_none() && o.edep_local > 0.0);
        // a full stack is an error
        let mut full = Stack::new(1);
        full.np = 1;
        full.e[1] = 0.5;
        assert!(matches!(
            compt(&mut full, &data, 1, 1, 0, None, RM, &mut rng),
            Err(EgsError::StackExceeded { routine: "COMPT", .. })
        ));
    }

    #[test]
    fn russian_roulette_kills_and_reweights_charged_only() {
        let mut rng = Ranmar::new(50, 60);
        let mut st = Stack::new(10);
        st.np = 4;
        for i in 1..=4 {
            st.wt[i] = 1.0;
            st.e[i] = i as f64;
            st.iq[i] = if i == 2 { 0 } else { -1 };
        }
        // prob <= 0 plays nothing
        assert_eq!(play_russian_roulette(&mut st, 1, 0.0, &mut rng), 0);
        assert_eq!(st.np, 4);
        let killed = play_russian_roulette(&mut st, 1, 0.5, &mut rng);
        assert_eq!(st.np, 4 - killed);
        assert!((1..=st.np).all(|i| st.iq[i] == 0 || st.wt[i] == 2.0));
        assert!((1..=st.np).any(|i| st.iq[i] == 0), "the photon is never killed");
        // killing everything charged leaves a placeholder particle
        let mut all = Stack::new(5);
        all.np = 1;
        all.iq[1] = -1;
        all.e[1] = 3.0;
        all.wt[1] = 1.0;
        let mut r2 = Ranmar::new(1, 1);
        let mut guard = 0;
        while all.np == 1 && all.iq[1] != 0 && guard < 1000 {
            all.iq[1] = -1;
            all.wt[1] = 1.0;
            play_russian_roulette(&mut all, 1, 1e-9, &mut r2);
            guard += 1;
        }
        assert_eq!((all.np, all.iq[1], all.wt[1], all.e[1]), (1, 0, 0.0, 0.0));
    }

    #[test]
    fn compt_with_roulette_applies_it_when_no_vacancy() {
        let mut rng = Ranmar::new(70, 80);
        let data = toy_compton_data(0.0);
        let mut st = photon_stack(1.0);
        compt(&mut st, &data, 1, 1, 0, Some(1e-12), RM, &mut rng).unwrap();
        // the recoil electron was killed; the scattered photon remains
        assert_eq!((st.np, st.iq[1]), (1, 0));
    }

    #[test]
    fn uphi_entry3_reuses_the_azimuth() {
        let mut rng = Ranmar::new(2, 3);
        let mut st = Stack::new(10);
        st.np = 1;
        st.w[1] = 1.0;
        let mut saved = UphiSaved::default();
        uphi(&mut st, &mut saved, 2, 1, 0.6, 0.8, &mut rng).unwrap();
        st.np = 2;
        // opposite side of the same plane: negative sin(theta), same azimuth
        uphi(&mut st, &mut saved, 3, 2, -0.6, 0.8, &mut rng).unwrap();
        assert!((st.u[1] + st.u[2]).abs() < 1e-12 && (st.v[1] + st.v[2]).abs() < 1e-12);
        assert_eq!(uphi(&mut st, &mut saved, 1, 1, 0.6, 0.8, &mut rng), Err(EgsError::BadEntry { ientry: 1, lvl: 1 }));
    }


    /// A toy element: K binding 0.01 MeV, L1..L3 0.0008 (under the 1 keV relax
    /// cut-off, so the cascade ends there), M 0.0005, N 0.0001; one certain
    /// K transition per element (fluorescence or Auger).
    fn toy_relax() -> RelaxData {
        let mut prob = alloc::vec![0.0; MXTRANS + 1];
        prob[1] = 1.0; // K -> K-L3 fluorescence (final state 4: vacancy in L3... index 4)
        let mut prob_auger = alloc::vec![0.0; MXTRANS + 1];
        prob_auger[5] = 1.0; // K-L1L1 Auger: final 202 = vacancies 2 and 2
        RelaxData {
            binding_energies: alloc::vec![
                Vec::new(),
                alloc::vec![0.0, 0.01, 0.0008, 0.0008, 0.0008, 0.0005, 0.0001],
                alloc::vec![0.0, 0.01, 0.0008, 0.0008, 0.0008, 0.0005, 0.0001],
            ],
            // iz 1 -> fluorescence, iz 2 -> Auger
            relaxation_prob: alloc::vec![Vec::new(), prob, prob_auger],
        }
    }

    #[test]
    fn relax_fluorescence_and_auger_conserve_energy() {
        let d = toy_relax();
        let mut rng = Ranmar::new(8, 9);
        for (iz, charged) in [(1usize, false), (2usize, true)] {
            let mut st = Stack::new(40);
            st.np = 1;
            st.wt[1] = 1.0;
            st.ir[1] = 3;
            // cut-offs low enough that nothing is absorbed at the first step
            let out = relax(&mut st, &d, 0.01, 1, iz, RM + 1e-4, 1e-4, RM, &mut rng).unwrap();
            let mut ekin = out.edep;
            for i in 2..=st.np {
                ekin += if st.iq[i] == 0 { st.e[i] } else { st.e[i] - RM };
                assert_eq!(st.ir[i], 3, "properties come from the parent slot");
                assert!((norm(&st, i) - 1.0).abs() < 1e-12);
            }
            assert!((ekin - 0.01).abs() < 1e-12, "iz {iz}: {ekin}");
            assert_eq!((2..=st.np).any(|i| st.iq[i] == -1), charged);
        }
    }

    #[test]
    fn relax_local_absorption_and_edge_cases() {
        let d = toy_relax();
        let mut rng = Ranmar::new(1, 4);
        let mut st = Stack::new(40);
        st.np = 1;
        // unknown shell: nothing
        assert_eq!(relax(&mut st, &d, 0.01, 0, 1, 1.0, 1.0, RM, &mut rng).unwrap(), RelaxOutcome::default());
        assert_eq!(relax(&mut st, &d, 0.01, 7, 1, 1.0, 1.0, RM, &mut rng).unwrap().edep, 0.0);
        // below the 1 keV relax cut-off: all local
        assert_eq!(relax(&mut st, &d, 5e-4, 1, 1, 1.0, 1.0, RM, &mut rng).unwrap().edep, 5e-4);
        assert_eq!(st.np, 1);
        // cut-offs above everything: all deposited, nothing pushed
        let out = relax(&mut st, &d, 0.01, 1, 2, 10.0, 10.0, RM, &mut rng).unwrap();
        assert_eq!(st.np, 1);
        assert!((out.edep - 0.01).abs() < 1e-12);
        // an N-shell vacancy produces one Auger electron
        let out = relax(&mut st, &d, 0.0005, 6, 1, RM + 1e-9, 1e-4, RM, &mut rng).unwrap();
        assert_eq!(out.edep, 0.0 + 0.0005);
        // (0.0005 < RELAX_CUTOFF: absorbed before the N-shell branch)
        let out = relax(&mut st, &d, 0.003, 6, 1, RM + 1e-9, 1e-4, RM, &mut rng).unwrap();
        assert_eq!((st.np, st.iq[2], out.edep), (2, -1, 0.0));
        assert!((st.e[2] - (0.003 + RM)).abs() < 1e-12);
        // N-shell vacancy below ekcut is absorbed
        let mut s2 = Stack::new(40);
        s2.np = 1;
        let out = relax(&mut s2, &d, 0.003, 6, 1, 10.0, 1e-4, RM, &mut rng).unwrap();
        assert_eq!((s2.np, out.edep), (1, 0.003));
        // a full stack is an error
        let mut full = Stack::new(1);
        full.np = 1;
        assert!(matches!(
            relax(&mut full, &d, 0.01, 1, 1, RM + 1e-4, 1e-4, RM, &mut rng),
            Err(EgsError::StackExceeded { routine: "RELAX", .. })
        ));
    }


    fn toy_pair_medium() -> PairMedium {
        // rejection functions identically 1, so every candidate is accepted
        let one = alloc::vec![1.0; 9];
        let zero = alloc::vec![0.0; 9];
        PairMedium {
            delcm: 0.5,
            dl: [one.clone(), zero.clone(), zero, one.clone(), alloc::vec![0.0; 9], one],
            bpar: [0.0, 0.5, 0.5],
            zbrang: 0.02,
        }
    }

    #[test]
    fn pair_conserves_energy_and_makes_opposite_charges() {
        let mut rng = Ranmar::new(12, 34);
        let med = toy_pair_medium();
        for iprdst in 0..=4 {
            let cfg = PairConfig { iprdst, ..Default::default() };
            for &eig in &[1.5, 3.0, 10.0, 100.0] {
                for _ in 0..30 {
                    let mut st = photon_stack(eig);
                    st.w[1] = 1.0;
                    let killed = pair(&mut st, &med, &cfg, RM, &mut rng).unwrap();
                    assert!(!killed);
                    assert_eq!((st.np_old, st.np), (1, 2), "iprdst {iprdst} eig {eig}");
                    assert!((st.e[1] + st.e[2] - eig).abs() < 1e-12);
                    assert_eq!(st.iq[1] + st.iq[2], 0);
                    assert!(st.iq[1] != 0);
                    assert!(st.e[1] >= RM - 1e-12 && st.e[2] >= RM - 1e-12);
                    for i in 1..=2 {
                        assert!((norm(&st, i) - 1.0).abs() < 1e-9, "iprdst {iprdst} eig {eig}: {}", norm(&st, i));
                    }
                }
            }
        }
    }

    #[test]
    fn pair_roulette_and_refused_branches() {
        let mut rng = Ranmar::new(5, 6);
        let med = toy_pair_medium();
        // probability ~0: the pair is killed (np > 1: the stack shrinks)
        let mut st = photon_stack(5.0);
        st.np = 2;
        st.e[2] = 5.0;
        st.w[2] = 1.0;
        let cfg = PairConfig { rr: Some(1e-12), ..Default::default() };
        assert!(pair(&mut st, &med, &cfg, RM, &mut rng).unwrap());
        assert_eq!(st.np, 1);
        // a lone photon stays, with zero weight and energy
        let mut lone = photon_stack(5.0);
        assert!(pair(&mut lone, &med, &cfg, RM, &mut rng).unwrap());
        assert_eq!((lone.np, lone.wt[1], lone.e[1]), (1, 0.0, 0.0));
        // survivors have their weight divided
        let mut surv = photon_stack(5.0);
        let cfg2 = PairConfig { rr: Some(1.0), ..Default::default() };
        assert!(!pair(&mut surv, &med, &cfg2, RM, &mut rng).unwrap());
        assert_eq!(surv.wt[1], 1.0);
        // non-positive probability plays nothing
        let mut none = photon_stack(5.0);
        let cfg3 = PairConfig { rr: Some(0.0), ..Default::default() };
        assert!(!pair(&mut none, &med, &cfg3, RM, &mut rng).unwrap());
        // refused branches
        let mut st = photon_stack(5.0);
        let nrc = PairConfig { pair_nrc: true, ..Default::default() };
        assert!(matches!(pair(&mut st, &med, &nrc, RM, &mut rng), Err(EgsError::Unsupported(_))));
        let trip = PairConfig { itriplet: 1, ..Default::default() };
        assert!(matches!(pair(&mut st, &med, &trip, RM, &mut rng), Err(EgsError::Unsupported(_))));
        // a full stack
        let mut full = Stack::new(1);
        full.np = 1;
        full.e[1] = 5.0;
        assert!(matches!(
            pair(&mut full, &med, &PairConfig::default(), RM, &mut rng),
            Err(EgsError::StackExceeded { routine: "PAIR", .. })
        ));
    }

    #[test]
    fn uphi_from_theta_matches_explicit_angle() {
        let mut r1 = Ranmar::new(3, 3);
        let mut r2 = Ranmar::new(3, 3);
        let mut a = Stack::new(5);
        a.np = 1;
        a.w[1] = 1.0;
        let mut b = a.clone();
        let theta = 0.7;
        uphi_from_theta(&mut a, &mut UphiSaved::default(), 1, theta, &mut r1).unwrap();
        uphi(&mut b, &mut UphiSaved::default(), 2, 1, theta.sin(), theta.cos(), &mut r2).unwrap();
        assert!((a.w[1] - b.w[1]).abs() < 1e-12 && (a.u[1] - b.u[1]).abs() < 1e-12);
        assert!((a.w[1] - theta.cos()).abs() < 1e-12);
    }


    fn toy_photo_data() -> (PhotoData, PhotoMedium, PhotoMedium) {
        let mut d = PhotoData::default();
        let mk = |v: f64| alloc::vec![0.0, v, v, v, v];
        // element 6 (index 6) and 8 (index 8); others empty
        for iz in 0..=8usize {
            let _ = iz;
            d.edge_energies.push(alloc::vec![0.0; MXEDGE + 2]);
            d.edge_number.push(3);
            d.edge_a.push(alloc::vec![0.0; MXEDGE + 2]);
            d.edge_b.push(alloc::vec![0.0; MXEDGE + 2]);
            d.edge_c.push(alloc::vec![0.0; MXEDGE + 2]);
            d.edge_d.push(alloc::vec![0.0; MXEDGE + 2]);
            d.interaction_prob.push(alloc::vec![0.0; MXSHELL + 1]);
        }
        let _ = mk;
        // the 1 keV minimum lives in edge_energies(2, 1)
        d.edge_energies[1][2] = 0.001;
        for &iz in &[6usize, 8] {
            d.edge_energies[iz][1] = 0.01;
            d.edge_energies[iz][2] = 0.002;
            d.edge_energies[iz][3] = 0.001;
            // above the first edge: sigma = (a + b/E + c/E^2.5 + d/E^3)/E
            d.edge_a[iz][1] = 1.0;
            for j in 2..=4 {
                d.edge_a[iz][j] = if iz == 6 { 1.0 } else { 2.0 };
            }
            d.interaction_prob[iz][1] = 0.8; // K
            for k in 2..=5 {
                d.interaction_prob[iz][k] = 0.5;
            }
        }
        let one = PhotoMedium { zelem: alloc::vec![0.0, 6.0], pz: alloc::vec![0.0, 1.0] };
        let two = PhotoMedium { zelem: alloc::vec![0.0, 6.0, 8.0], pz: alloc::vec![0.0, 1.0, 2.0] };
        (d, one, two)
    }

    fn photo_relax_data() -> RelaxData {
        let t = toy_relax();
        // reuse the toy cascade for iz 6 and 8 (fluorescence)
        let be = t.binding_energies[1].clone();
        let rp = t.relaxation_prob[1].clone();
        let mut d = RelaxData::default();
        for _ in 0..=8 {
            d.binding_energies.push(be.clone());
            d.relaxation_prob.push(rp.clone());
        }
        d
    }

    #[test]
    fn photoelectron_direction_branches() {
        let mut rng = Ranmar::new(99, 98);
        // 30 keV (ratio > 0.2), 5 MeV (ratio <= 0.2, gamma < 100), 100 MeV (gamma >= 100)
        for &e in &[0.54, 5.5, 100.0] {
            for _ in 0..50 {
                let mut st = Stack::new(5);
                st.np = 1;
                st.e[1] = e;
                st.w[1] = 1.0;
                select_photoelectron_direction(&mut st, 1, 0.512, &mut rng).unwrap();
                assert!((norm(&st, 1) - 1.0).abs() < 1e-9, "e {e}");
            }
        }
        // iphter != 1 or below ecut: the direction is untouched
        let mut st = Stack::new(5);
        st.np = 1;
        st.e[1] = 1.0;
        st.w[1] = 1.0;
        select_photoelectron_direction(&mut st, 0, 0.0, &mut rng).unwrap();
        select_photoelectron_direction(&mut st, 1, 5.0, &mut rng).unwrap();
        assert_eq!((st.u[1], st.v[1], st.w[1]), (0.0, 0.0, 1.0));
    }

    #[test]
    fn photo_converts_and_conserves_energy() {
        let (d, one, two) = toy_photo_data();
        let rd = photo_relax_data();
        let mut rng = Ranmar::new(5, 7);
        let reg = PhotoRegion { iedgfl: 1, iphter: 1, ecut: RM + 0.0001, pcut: 0.0001 };
        // below the 1 keV minimum: electron with the photon energy as kinetic energy
        let mut st = photon_stack(0.0005);
        photo(&mut st, &d, &rd, &one, &reg, (0.0005f64).ln(), None, RM, &mut rng).unwrap();
        assert_eq!((st.iq[1], st.np), (-1, 1));
        assert!((st.e[1] - (0.0005 + RM)).abs() < 1e-15);
        // no relaxation requested
        let off = PhotoRegion { iedgfl: 0, ..reg };
        let mut st = photon_stack(0.05);
        photo(&mut st, &d, &rd, &one, &off, (0.05f64).ln(), None, RM, &mut rng).unwrap();
        assert_eq!((st.iq[1], st.np), (-1, 1));
        assert!((st.e[1] - (0.05 + RM)).abs() < 1e-15);
        // relaxation on, one and two elements: kinetic energy + products + edep = photon
        for medium in [&one, &two] {
            for _ in 0..50 {
                let e = 0.05;
                let mut st = photon_stack(e);
                let o = photo(&mut st, &d, &rd, medium, &reg, e.ln(), None, RM, &mut rng).unwrap();
                assert_eq!(st.np_old, 1);
                let mut tot = o.edep;
                for i in 1..=st.np {
                    tot += match st.iq[i] {
                        0 => st.e[i],
                        _ => st.e[i] - RM,
                    };
                }
                assert!((tot + 0.0 - e).abs() < 1e-9, "tot {tot} vs {e}");
                assert!((1..=st.np).all(|i| (norm(&st, i) - 1.0).abs() < 1e-9));
            }
        }
        // photon energy under the outermost binding energy: electron, no relaxation
        let mut st = photon_stack(0.00005);
        let mut low_d = d.clone();
        low_d.edge_energies[1][2] = 1e-9;
        photo(&mut st, &low_d, &rd, &one, &reg, (0.00005f64).ln(), None, RM, &mut rng).unwrap();
        assert!((st.e[1] - (0.00005 + RM)).abs() < 1e-15 && st.np == 1);
        // roulette kills the electron
        let mut st = photon_stack(0.05);
        photo(&mut st, &d, &rd, &one, &off, (0.05f64).ln(), Some(1e-12), RM, &mut rng).unwrap();
        assert_eq!((st.np, st.wt[1]), (1, 0.0));
        // a bad atomic number is refused
        let bad = PhotoMedium { zelem: alloc::vec![0.0, 0.0, 6.0], pz: alloc::vec![0.0, 1.0, 1.0] };
        let mut st = photon_stack(0.05);
        assert!(matches!(
            photo(&mut st, &d, &rd, &bad, &reg, (0.05f64).ln(), None, RM, &mut rng),
            Err(EgsError::Unsupported(_))
        ));
    }


    fn momentum_balance(st: &Stack, etot: f64, dir: (f64, f64, f64)) -> f64 {
        let p0 = (etot * etot - RM * RM).sqrt();
        let mut px = -p0 * dir.0;
        let mut py = -p0 * dir.1;
        let mut pz = -p0 * dir.2;
        for i in st.np_old..=st.np {
            let p = (st.e[i] * st.e[i] - RM * RM).max(0.0).sqrt();
            px += p * st.u[i];
            py += p * st.v[i];
            pz += p * st.w[i];
        }
        (px * px + py * py + pz * pz).sqrt()
    }

    #[test]
    fn moller_and_bhabha_conserve_energy_and_momentum() {
        let mut rng = Ranmar::new(13, 17);
        let te = 0.01;
        let thmoll = 2.0 * te + RM;
        for &ekin in &[0.05, 0.5, 3.0, 20.0] {
            for _ in 0..40 {
                let (u, v) = (0.3, -0.2);
                let w = (1.0f64 - u * u - v * v).sqrt();
                // electron
                let mut st = Stack::new(10);
                st.np = 1;
                st.e[1] = ekin + RM;
                st.iq[1] = -1;
                (st.u[1], st.v[1], st.w[1]) = (u, v, w);
                assert!(moller(&mut st, te, thmoll, false, RM, &mut rng).unwrap());
                assert_eq!((st.np_old, st.np), (1, 2));
                assert!((st.e[1] + st.e[2] - (ekin + 2.0 * RM)).abs() < 1e-12);
                assert_eq!((st.iq[1], st.iq[2]), (-1, -1));
                assert!(momentum_balance(&st, ekin + RM, (u, v, w)) < 1e-6 * (ekin + RM), "moller {ekin}");
                // positron
                let mut sp = Stack::new(10);
                sp.np = 1;
                sp.e[1] = ekin + RM;
                sp.iq[1] = 1;
                (sp.u[1], sp.v[1], sp.w[1]) = (u, v, w);
                bhabha(&mut sp, te, RM, &mut rng).unwrap();
                assert_eq!((sp.np_old, sp.np), (1, 2));
                assert!((sp.e[1] + sp.e[2] - (ekin + 2.0 * RM)).abs() < 1e-12);
                assert_eq!(sp.iq[1] + sp.iq[2], 0, "one positron, one electron");
                assert!(momentum_balance(&sp, ekin + RM, (u, v, w)) < 1e-6 * (ekin + RM), "bhabha {ekin}");
            }
        }
    }

    #[test]
    fn moller_edge_cases() {
        let mut rng = Ranmar::new(1, 9);
        // below twice the threshold: nothing happens
        let mut st = Stack::new(10);
        st.np = 1;
        st.e[1] = 0.01 + RM;
        st.w[1] = 1.0;
        assert!(!moller(&mut st, 0.01, 0.02 + RM, false, RM, &mut rng).unwrap());
        assert_eq!(st.np, 1);
        // EII is refused, a full stack is an error
        assert!(matches!(moller(&mut st, 0.01, 0.02 + RM, true, RM, &mut rng), Err(EgsError::Unsupported(_))));
        let mut full = Stack::new(1);
        full.np = 1;
        full.e[1] = 1.0 + RM;
        full.w[1] = 1.0;
        assert!(matches!(
            moller(&mut full, 0.01, 0.02 + RM, false, RM, &mut rng),
            Err(EgsError::StackExceeded { routine: "MOLLER", .. })
        ));
        let mut fullb = Stack::new(1);
        fullb.np = 1;
        fullb.e[1] = 1.0 + RM;
        fullb.w[1] = 1.0;
        assert!(matches!(bhabha(&mut fullb, 0.01, RM, &mut rng), Err(EgsError::StackExceeded { routine: "BHABHA", .. })));
    }


    fn toy_brems_medium() -> BremsMedium {
        let one = alloc::vec![1.0; 9];
        let zero = alloc::vec![0.0; 9];
        BremsMedium {
            delcm: 0.5,
            dl: [one.clone(), zero.clone(), zero, one.clone(), alloc::vec![0.0; 9], one],
            zbrang: 0.02,
            lzbrang: (0.02f64).ln(),
            ap: 0.01,
            log_ap: (0.01f64).ln(),
        }
    }

    #[test]
    fn brems_photon_energy_weight_and_direction() {
        let mut rng = Ranmar::new(21, 22);
        let med = toy_brems_medium();
        for &ibrdst in &[1, 0, -1] {
            for &ekin in &[0.1, 2.0, 60.0] {
                for _ in 0..30 {
                    let peie = ekin + RM;
                    let mut st = Stack::new(10);
                    st.np = 1;
                    st.e[1] = peie;
                    st.iq[1] = -1;
                    st.wt[1] = 3.0;
                    st.ir[1] = 4;
                    (st.u[1], st.v[1], st.w[1]) = (0.0, 0.6, 0.8);
                    let cfg = BremsConfig { ibrdst, ..Default::default() };
                    brems(&mut st, &med, &cfg, ekin.ln(), RM, &mut rng).unwrap();
                    assert_eq!((st.np_old, st.np), (1, 2));
                    assert_eq!((st.iq[2], st.ir[2], st.wt[2]), (0, 4, 3.0));
                    assert!(st.e[2] >= med.ap * 0.999 && st.e[2] <= ekin * (1.0 + 1e-12), "k {}", st.e[2]);
                    assert!((st.e[1] + st.e[2] - peie).abs() < 1e-12);
                    assert!((norm(&st, 2) - 1.0).abs() < 1e-9, "ibrdst {ibrdst}");
                    if ibrdst < 0 {
                        assert_eq!((st.u[2], st.v[2], st.w[2]), (0.0, 0.6, 0.8));
                    }
                }
            }
        }
        // splitting: n photons of weight wt/n, the electron keeps the last photon's loss
        let mut st = Stack::new(10);
        st.np = 1;
        st.e[1] = 1.0 + RM;
        st.wt[1] = 6.0;
        st.w[1] = 1.0;
        let cfg = BremsConfig { nbr_split: 3, ..Default::default() };
        brems(&mut st, &med, &cfg, (1.0f64).ln(), RM, &mut rng).unwrap();
        assert_eq!(st.np, 4);
        assert!((2..=4).all(|i| st.wt[i] == 2.0 && st.iq[i] == 0));
        assert!((st.e[1] - (1.0 + RM - st.e[4])).abs() < 1e-12);
        // off, refused tables, and a full stack
        let mut off = Stack::new(10);
        off.np = 1;
        off.e[1] = 1.0 + RM;
        brems(&mut off, &med, &BremsConfig { nbr_split: 0, ..Default::default() }, 0.0, RM, &mut rng).unwrap();
        assert_eq!(off.np, 1);
        assert!(matches!(
            brems(&mut off, &med, &BremsConfig { ibr_nist: 1, ..Default::default() }, 0.0, RM, &mut rng),
            Err(EgsError::Unsupported(_))
        ));
        let mut full = Stack::new(1);
        full.np = 1;
        full.e[1] = 1.0 + RM;
        full.w[1] = 1.0;
        assert!(matches!(
            brems(&mut full, &med, &BremsConfig::default(), 0.0, RM, &mut rng),
            Err(EgsError::StackExceeded { routine: "BREMS", .. })
        ));
    }

    #[test]
    fn uphi_small_polar_angle_case() {
        let mut rng = Ranmar::new(5, 6);
        let mut st = Stack::new(10);
        st.np = 1;
        st.w[1] = -1.0; // along -z: a = b = 0
        let mut saved = UphiSaved::default();
        uphi(&mut st, &mut saved, 2, 1, 0.6, 0.8, &mut rng).unwrap();
        assert!((norm(&st, 1) - 1.0).abs() < 1e-12);
        assert!((st.w[1] + 0.8).abs() < 1e-15, "w = c * costhe");
    }
}
