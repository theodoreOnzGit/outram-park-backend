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
//! **Status: first port of fourteen routines, not differentially tested (gh:#1680).**
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
//! | `SUBROUTINE PHOTON` (`egsnrc.mortran:6438-6640`), without Rayleigh or photonuclear | [`photon`] |
//! | `SUBROUTINE SHOWER` (`egsnrc.mortran:6645-6718`) | [`shower`] |
//! | `SUBROUTINE mscat`, `spin_rejection`, `sscat` (`egsnrc.mortran:2983-3263`) and the table reader of `init_ms_SR` (`:3266-3306`) | [`mscat`], [`spin_rejection`], [`sscat`], [`MsData::from_text`] |
//! | `SUBROUTINE EDGSET` (`egsnrc.mortran:6311-6430`), the data readers | [`edgset_from_text`] |
//! | `SUBROUTINE msdist_pI` (`egsnrc.mortran:4309-4455`), `msdist_pII` (`:4049-4290`) | [`msdist_p1`], [`msdist_p2`] |
//! | `SUBROUTINE ELECTR` (`egsnrc.mortran:985-1825`) with the step, range and energy-loss macros of `egsnrc.macros:1308-1360, 2358-2395, 3105-3330` | [`electr`] |
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

/// What `HOWFAR` reads and writes in `PHOTON` (`USTEP`, `IRNEW`, `IDISC`):
/// the geometry shortens `ustep` to the boundary, sets `irnew` to the region
/// beyond it, and may request a discard with `idisc`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HowfarState {
    /// Requested and returned step, cm.
    pub ustep: f64,
    /// Region after the step (the geometry sets it when a boundary is hit).
    pub irnew: usize,
    /// `> 0` discard now, `< 0` discard after the step, `0` keep.
    pub idisc: i32,
}

/// The geometry seam (`SUBROUTINE HOWFAR`, a user routine in EGSnrc). A
/// compile-time contract: `photon` is generic over it, no trait objects.
pub trait Geometry {
    /// Called when `ustep > dnear` or the weight is not positive
    /// (`$CALL-HOWFAR-IN-PHOTON`, `egsnrc.macros:2422`). Reads the particle
    /// from `stack[np]`; sets `state`.
    fn howfar(&mut self, stack: &Stack, state: &mut HowfarState);
}

/// Per-region data `PHOTON` reads.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PhotonRegionData {
    /// `med(region)`; `0` is vacuum.
    pub med: usize,
    /// `rhor(region)`: density of the region, g/cm^3.
    pub rhor: f64,
    /// `pcut(region)`, MeV.
    pub pcut: f64,
    /// `ecut(region)`, total energy MeV.
    pub ecut: f64,
    /// `ibcmp(region)`.
    pub ibcmp: i32,
    /// `iedgfl(region)`.
    pub iedgfl: i32,
    /// `iphter(region)`.
    pub iphter: i32,
    /// `iraylr(region)`; `1` (Rayleigh) is not ported and is refused.
    pub irayl: i32,
    /// `iphotonucr(region)`; `1` (photonuclear) is not ported and is refused.
    pub iphotonucr: i32,
}

/// Per-medium tables `PHOTON` reads: the piecewise-linear `GMFP`, `GBR1`,
/// `GBR2` fits (`$SET INTERVAL GLE,GE` then `f = f1(L) * GLE + f0(L)` with
/// `L = ge1 * GLE + ge0`), the density, and the data its interactions need.
/// Interval-indexed vectors are 1-based.
#[derive(Debug, Clone, Default)]
pub struct PhotonMediumData {
    /// `rho(medium)`.
    pub rho: f64,
    /// `ge0`, `ge1`: interval index `L = ge1 * GLE + ge0`.
    pub ge0: f64,
    /// See `ge0`.
    pub ge1: f64,
    /// `gmfp0(L)`, `gmfp1(L)`: photon mean free path, cm.
    pub gmfp0: Vec<f64>,
    /// See `gmfp0`.
    pub gmfp1: Vec<f64>,
    /// `gbr10(L)`, `gbr11(L)`: pair probability.
    pub gbr10: Vec<f64>,
    /// See `gbr10`.
    pub gbr11: Vec<f64>,
    /// `gbr20(L)`, `gbr21(L)`: (pair + Compton) probability.
    pub gbr20: Vec<f64>,
    /// See `gbr20`.
    pub gbr21: Vec<f64>,
    /// The element list `PHOTO` samples from.
    pub photo: PhotoMedium,
    /// The data `PAIR` reads.
    pub pair: PairMedium,
}

/// All tables `photon` needs. `regions` and `media` are 1-based (index 0 unused).
#[derive(Debug, Clone, Default)]
pub struct PhotonTables {
    /// Regions.
    pub regions: Vec<PhotonRegionData>,
    /// Media.
    pub media: Vec<PhotonMediumData>,
    /// `COMMON/COMPTON-DATA/`.
    pub compton: ComptonData,
    /// `COMMON/EDGE/` relaxation part.
    pub relax: RelaxData,
    /// `COMMON/EDGE/` photoelectric part.
    pub photo: PhotoData,
}

/// Switches `photon` reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhotonConfig {
    /// `PAIR` switches.
    pub pair: PairConfig,
    /// `Some(prob_RR)` when `i_play_RR = 1`.
    pub rr: Option<f64>,
    /// `PRM`, MeV.
    pub prm: f64,
    /// `VACDST`: step through vacuum, cm.
    pub vacdst: f64,
}

/// What `photon` returns.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PhotonResult {
    /// `IRCODE`: `1` normal return (an electron is on top of the stack, or the
    /// stack changed so the caller must re-enter), `2` the photon was discarded
    /// (`np` has been decremented).
    pub ircode: i32,
    /// `EDEP` of a discard: the energy of the discarded photon, MeV.
    pub edep: f64,
    /// Energy deposited locally by `RELAX`, and by sub-1 keV Compton binding
    /// energies, that upstream hands to `AUSGAB`.
    pub edep_local: f64,
}

/// `$EPSGMFP` (`egsnrc.macros:1402`): smallest gamma MFP value.
pub const EPSGMFP: f64 = 1e-8;

/// A photon removed from the stack: its energy is `edep`, `ircode = 2`.
fn photon_discard(stack: &mut Stack, res: &mut PhotonResult, peig: f64) -> PhotonResult {
    res.edep = peig;
    res.ircode = 2;
    stack.np -= 1;
    *res
}

/// `PHOTON` (`egsnrc.mortran:6438-6640`): transport the photon on top of the
/// stack until it interacts and an electron tops the stack (`ircode = 1`) or
/// it is discarded (`ircode = 2`). Interactions call [`pair`], [`compt`]
/// (with [`relax`] for its vacancy) and [`photo`].
///
/// Not ported: Rayleigh scattering (`iraylr = 1`, `egs_rayleigh_sampling`)
/// and photonuclear reactions (`iphotonucr = 1`, `PHOTONUC`), refused with
/// [`EgsError::Unsupported`]; the `AUSGAB` hooks; `$PARTICLE-SELECTION-*`
/// (empty by default). `lgle` is recomputed whenever the medium changes, as
/// upstream.
///
/// # Errors
///
/// [`EgsError`] from the interaction routines, or `Unsupported` as above.
pub fn photon<R: UniformSource, G: Geometry>(
    stack: &mut Stack,
    tables: &PhotonTables,
    cfg: &PhotonConfig,
    geometry: &mut G,
    rng: &mut R,
) -> Result<PhotonResult, EgsError> {
    let rmt2 = 2.0 * RM;
    let mut res = PhotonResult { ircode: 1, ..Default::default() };
    let mut peig = stack.e[stack.np];
    let mut eig = peig;
    let mut irl = stack.ir[stack.np] as usize;
    let mut medium = tables.regions[irl].med;
    let mut lgle = 0usize;
    let mut gmfpr0 = 0.0;
    let mut gmfp = 0.0;

    if eig <= tables.regions[irl].pcut {
        return Ok(photon_discard(stack, &mut res, peig));
    }

    // :PNEWENERGY:
    loop {
        if stack.wt[stack.np] == 0.0 {
            return Ok(photon_discard(stack, &mut res, peig));
        }
        let gle = RealMath::r_ln(eig);
        // $SELECT-PHOTON-MFP
        let mut rnno35 = rng.uniform();
        if rnno35 == 0.0 {
            rnno35 = 1.0e-30;
        }
        let mut dpmfp = -RealMath::r_ln(rnno35);

        // :PNEWMEDIUM:
        'pnewmedium: loop {
            if medium != 0 {
                let m = &tables.media[medium];
                lgle = (m.ge1 * gle + m.ge0) as usize;
                gmfpr0 = m.gmfp1[lgle] * gle + m.gmfp0[lgle];
            }
            // :PTRANS:
            loop {
                let tstep;
                if medium == 0 {
                    tstep = cfg.vacdst;
                } else {
                    let region = &tables.regions[irl];
                    if region.irayl == 1 {
                        return Err(EgsError::Unsupported("PHOTON: Rayleigh scattering (iraylr = 1)"));
                    }
                    if region.iphotonucr == 1 {
                        return Err(EgsError::Unsupported("PHOTON: photonuclear reactions (iphotonucr = 1)"));
                    }
                    let rhof = region.rhor / tables.media[medium].rho;
                    gmfp = gmfpr0 / rhof;
                    tstep = gmfp * dpmfp;
                }
                let np = stack.np;
                let mut st = HowfarState { ustep: tstep, irnew: stack.ir[np] as usize, idisc: 0 };
                if st.ustep > stack.dnear[np] || stack.wt[np] <= 0.0 {
                    geometry.howfar(stack, &mut st);
                }
                if st.idisc > 0 {
                    return Ok(photon_discard(stack, &mut res, peig));
                }
                let vstep = st.ustep;
                // transport the photon
                stack.x[np] += stack.u[np] * vstep;
                stack.y[np] += stack.v[np] * vstep;
                stack.z[np] += stack.w[np] * vstep;
                stack.dnear[np] -= st.ustep;
                if medium != 0 {
                    dpmfp = (0.0_f64).max(dpmfp - st.ustep / gmfp);
                }
                let medold = medium;
                if st.irnew != stack.ir[np] as usize {
                    // $photon_region_change
                    stack.ir[np] = st.irnew as i32;
                    irl = st.irnew;
                    medium = tables.regions[irl].med;
                }
                if eig <= tables.regions[irl].pcut {
                    return Ok(photon_discard(stack, &mut res, peig));
                }
                if st.idisc < 0 {
                    return Ok(photon_discard(stack, &mut res, peig));
                }
                if medium != medold {
                    continue 'pnewmedium;
                }
                if medium != 0 && dpmfp <= EPSGMFP {
                    break 'pnewmedium;
                }
            }
        }

        // it is finally time to interact
        {
            let region = &tables.regions[irl];
            if region.irayl == 1 {
                return Err(EgsError::Unsupported("PHOTON: Rayleigh scattering (iraylr = 1)"));
            }
            if region.iphotonucr == 1 {
                return Err(EgsError::Unsupported("PHOTON: photonuclear reactions (iphotonucr = 1)"));
            }
        }
        let rnno36 = rng.uniform();
        let m = &tables.media[medium];
        let gbr1 = m.gbr11[lgle] * gle + m.gbr10[lgle];
        let mut pair_done = false;
        if rnno36 <= gbr1 && stack.e[stack.np] > rmt2 {
            pair(stack, &m.pair, &cfg.pair, RM, rng)?;
            if stack.iq[stack.np] != 0 {
                return Ok(res);
            }
            // pair electrons killed via Russian roulette
            pair_done = true;
        }
        if !pair_done {
            let gbr2 = m.gbr21[lgle] * gle + m.gbr20[lgle];
            let region = tables.regions[irl];
            if rnno36 < gbr2 {
                let out = compt(stack, &tables.compton, medium, irl, region.ibcmp, cfg.rr, cfg.prm, rng)?;
                if let Some(v) = out.vacancy {
                    let r = relax(
                        stack,
                        &tables.relax,
                        v.uj,
                        v.shn,
                        v.iz as usize,
                        region.ecut,
                        region.pcut,
                        cfg.prm,
                        rng,
                    )?;
                    res.edep_local += r.edep;
                    if let Some(p) = cfg.rr {
                        play_russian_roulette(stack, stack.np_old + 1, p, rng);
                    }
                }
                res.edep_local += out.edep_local;
                if stack.iq[stack.np] != 0 {
                    return Ok(res);
                }
            } else {
                let preg = PhotoRegion { iedgfl: region.iedgfl, iphter: region.iphter, ecut: region.ecut, pcut: region.pcut };
                let o = photo(stack, &tables.photo, &tables.relax, &m.photo, &preg, gle, cfg.rr, cfg.prm, rng)?;
                res.edep_local += o.edep;
                // Russian roulette may have emptied the stack or removed the
                // electrons: force a return so the caller re-enters properly
                if stack.np == 0 || stack.np < stack.np_old {
                    return Ok(res);
                }
                if stack.iq[stack.np] != 0 {
                    return Ok(res);
                }
            }
        }
        // :PAIR-ELECTRONS-KILLED: the gamma is the lowest-energy particle
        peig = stack.e[stack.np];
        eig = peig;
        if eig < tables.regions[irl].pcut {
            return Ok(photon_discard(stack, &mut res, peig));
        }
    }
}

/// The electron-transport seam `SHOWER` calls (`SUBROUTINE ELECTR(IRCODE)`,
/// `egsnrc.mortran:985-1825`, not yet ported). A compile-time contract:
/// [`shower`] is generic over it, no trait objects.
pub trait ElectronTransport {
    /// Transport the charged particle on top of the stack until it is
    /// discarded or a photon tops the stack; returns `IRCODE`.
    ///
    /// # Errors
    ///
    /// Whatever the implementation raises.
    fn electr<R: UniformSource>(&mut self, stack: &mut Stack, rng: &mut R) -> Result<i32, EgsError>;
}

/// Energy `SHOWER` saw leave the stack through the photon routine.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ShowerResult {
    /// Sum of the `edep` of discarded photons, MeV.
    pub edep_photon_discards: f64,
    /// Sum of the local energy deposited by `RELAX` and Compton binding, MeV.
    pub edep_local: f64,
}

/// The starting particle of a shower (`SHOWER`'s arguments).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShowerStart {
    /// `IQI`: -1, 0, +1, or 2 for the pi-zero option.
    pub iqi: i32,
    /// `EI`: total energy, MeV.
    pub ei: f64,
    /// `XI`.
    pub xi: f64,
    /// `YI`.
    pub yi: f64,
    /// `ZI`.
    pub zi: f64,
    /// `UI`.
    pub ui: f64,
    /// `VI`.
    pub vi: f64,
    /// `WI`.
    pub wi: f64,
    /// `IRI`: region.
    pub iri: i32,
    /// `WTI`: weight.
    pub wti: f64,
}

/// `SHOWER` (`egsnrc.mortran:6645-6718`): follow one incident particle and all
/// it creates until the stack is empty. The pi-zero option (`iqi = 2`) turns
/// the incident into two decay photons first. Photons go to [`photon`],
/// charged particles to the [`ElectronTransport`] seam.
///
/// # Errors
///
/// [`EgsError::Fatal`] for a pi-zero start below its rest energy, otherwise
/// whatever the transport routines raise.
pub fn shower<R: UniformSource, G: Geometry, E: ElectronTransport>(
    stack: &mut Stack,
    tables: &PhotonTables,
    cfg: &PhotonConfig,
    geometry: &mut G,
    electrons: &mut E,
    start: &ShowerStart,
    rng: &mut R,
) -> Result<ShowerResult, EgsError> {
    const PI0MSQ: f64 = 1.8215416e4; // pi-zero mass (MeV) squared
    let mut out = ShowerResult::default();
    stack.np = 1;
    stack.np_old = stack.np;
    let dneari = 0.0;
    stack.iq[1] = start.iqi;
    stack.e[1] = start.ei;
    stack.u[1] = start.ui;
    stack.v[1] = start.vi;
    stack.w[1] = start.wi;
    // $TRANSFER PROPERTIES TO (1) FROM I
    stack.x[1] = start.xi;
    stack.y[1] = start.yi;
    stack.z[1] = start.zi;
    stack.ir[1] = start.iri;
    stack.wt[1] = start.wti;
    stack.dnear[1] = dneari;
    stack.latch[1] = stack.latchi;

    if start.iqi == 2 {
        // pi-zero option
        if start.ei * start.ei <= PI0MSQ {
            return Err(EgsError::Fatal("SHOWER: pi-zero option invoked but the total energy was too small"));
        }
        let csth = rng.uniform();
        let dcsth = csth;
        let dei = start.ei;
        let dpi = Real::sqrt(dei * dei - PI0MSQ);
        let deg = dei + dpi * dcsth;
        let dpgl = dpi + dei * dcsth;
        let dcosth = dpgl / deg;
        let mut costhe = dcosth;
        let mut sinthe = Real::sqrt(1.0 - dcosth * dcosth);
        stack.iq[1] = 0;
        stack.e[1] = deg / 2.0;
        let mut saved = UphiSaved::default();
        uphi(stack, &mut saved, 2, 1, sinthe, costhe, rng)?;
        stack.np = 2;
        let deg = dei - dpi * dcsth;
        let dpgl = dpi - dei * dcsth;
        let dcosth = dpgl / deg;
        costhe = dcosth;
        sinthe = -Real::sqrt(1.0 - dcosth * dcosth);
        stack.iq[2] = 0;
        stack.e[2] = deg / 2.0;
        uphi(stack, &mut saved, 3, 2, sinthe, costhe, rng)?;
    }

    loop {
        if stack.np == 0 {
            break;
        }
        if stack.iq[stack.np] == 0 {
            let r = photon(stack, tables, cfg, geometry, rng)?;
            if r.ircode == 2 {
                out.edep_photon_discards += r.edep;
            }
            out.edep_local += r.edep_local;
        } else {
            electrons.electr(stack, rng)?;
        }
    }
    Ok(out)
}

/// `$MAXL_MS` (`egsnrc.macros:851`).
pub const MAXL_MS: usize = 63;
/// `$MAXQ_MS`.
pub const MAXQ_MS: usize = 7;
/// `$MAXU_MS`.
pub const MAXU_MS: usize = 31;
/// `$LAMBMIN_MS`.
pub const LAMBMIN_MS: f64 = 1.0;
/// `$LAMBMAX_MS`.
pub const LAMBMAX_MS: f64 = 1e5;
/// `$QMIN_MS`.
pub const QMIN_MS: f64 = 1e-3;
/// `$QMAX_MS`.
pub const QMAX_MS: f64 = 0.5;

/// One Fortran list-directed `read(unit,*) a, b, c ...` of `n` values: values
/// are separated by blanks or commas, `D` exponents are accepted, and the
/// read continues over lines until `n` values are in; a record's surplus
/// values are dropped, as the Fortran `read` drops them.
fn read_record(lines: &mut core::str::Lines<'_>, n: usize) -> Result<Vec<f64>, String> {
    let mut out: Vec<f64> = Vec::with_capacity(n);
    while out.len() < n {
        let line = lines.next().ok_or_else(|| String::from("data ends early"))?;
        for tok in line.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty()) {
            if out.len() == n {
                break;
            }
            let t = tok.replace(['D', 'd'], "e");
            out.push(t.parse::<f64>().map_err(|_| alloc::format!("bad value '{tok}'"))?);
        }
    }
    Ok(out)
}

/// `EDGSET` (`egsnrc.mortran:6311-6430`): the readers of the simplified
/// photo-absorption and relaxation data and the parametrised XCOM photoelectric
/// fits, from in-memory copies of EGSnrc's `*_photo.data` (`photo`) and
/// `*_phocs.data`-style (`phocs`) files. Opening the files is the caller's
/// (EGSnrc's files are not vendored). Returns the [`RelaxData`] part
/// (`binding_energies` in MeV, `relaxation_prob`) and the [`PhotoData`] part
/// (`interaction_prob`, `edge_*`), 1-based over 100 elements.
///
/// Not ported: the `eadl_relax` variant (binding energies skipped, then
/// `egs_init_relax`), and the "relaxations not requested" early return (the
/// caller decides whether to read at all).
///
/// # Errors
///
/// A message when the text is short or a value does not parse.
pub fn edgset_from_text(photo: &str, phocs: &str) -> Result<(RelaxData, PhotoData), String> {
    const NEL: usize = 100; // $MXELEMENT
    let mut lines = photo.lines();
    let mut rd = RelaxData {
        binding_energies: alloc::vec![alloc::vec![0.0; MXSHELL + 1]; NEL + 1],
        relaxation_prob: alloc::vec![alloc::vec![0.0; MXTRANS + 1]; NEL + 1],
    };
    let mut pd = PhotoData {
        edge_energies: alloc::vec![alloc::vec![0.0; MXEDGE + 2]; NEL + 1],
        edge_number: alloc::vec![0; NEL + 1],
        edge_a: alloc::vec![alloc::vec![0.0; MXEDGE + 2]; NEL + 1],
        edge_b: alloc::vec![alloc::vec![0.0; MXEDGE + 2]; NEL + 1],
        edge_c: alloc::vec![alloc::vec![0.0; MXEDGE + 2]; NEL + 1],
        edge_d: alloc::vec![alloc::vec![0.0; MXEDGE + 2]; NEL + 1],
        interaction_prob: alloc::vec![alloc::vec![0.0; MXSHELL + 1]; NEL + 1],
    };
    for i in 1..=NEL {
        let r = read_record(&mut lines, 1 + MXSHELL)?;
        for k in 1..=MXSHELL {
            rd.binding_energies[i][k] = r[k] * 1e-6; // to MeV
        }
    }
    lines.next();
    for i in 1..=NEL {
        let r = read_record(&mut lines, 1 + MXINTER)?;
        for k in 1..=MXINTER {
            pd.interaction_prob[i][k] = r[k];
        }
        pd.interaction_prob[i][MXSHELL] = 1.01;
    }
    // K, L1, L2, L3 and M relaxation probabilities, each after a header line
    for &(first, last) in &[(1usize, 19usize), (20, 26), (27, 32), (33, 37), (38, 38)] {
        lines.next();
        for i in 1..=NEL {
            let r = read_record(&mut lines, 1 + last - first + 1)?;
            for k in first..=last {
                rd.relaxation_prob[i][k] = r[k - first + 1];
            }
        }
    }
    let mut lines = phocs.lines();
    for i in 1..=NEL {
        let r = read_record(&mut lines, 2)?;
        let n = r[1] as usize;
        if n > MXEDGE {
            return Err(alloc::format!("element {i} has {n} edges, more than {MXEDGE}"));
        }
        pd.edge_number[i] = n as i32;
        for j in 1..=n {
            let e = read_record(&mut lines, 5)?;
            pd.edge_a[i][j] = e[0];
            pd.edge_b[i][j] = e[1];
            pd.edge_c[i][j] = e[2];
            pd.edge_d[i][j] = e[3];
            pd.edge_energies[i][j] = e[4];
        }
    }
    Ok((rd, pd))
}

/// `COMMON/MS-DATA/` (`egsnrc.macros:861-871`): the pre-calculated screened
/// Rutherford multiple-scattering `q^(2+)` surface. **Stored in single
/// precision, as upstream** (`real*4`, `ims_array` a short integer): reading,
/// the `fms`/`ims` fix-ups and the scalar constants round to `f32` exactly where
/// `init_ms_SR` stores them, and `mscat` widens them on use.
#[derive(Debug, Clone)]
pub struct MsData {
    ums: Vec<f32>,
    fms: Vec<f32>,
    wms: Vec<f32>,
    ims: Vec<i16>,
    /// `dllambi` (`real*4`).
    pub dllambi: f32,
    /// `dqmsi` (`real*4`).
    pub dqmsi: f32,
}

const MS_N: usize = (MAXL_MS + 1) * (MAXQ_MS + 1) * (MAXU_MS + 1);

fn ms_idx(i: usize, j: usize, k: usize) -> usize {
    (i * (MAXQ_MS + 1) + j) * (MAXU_MS + 1) + k
}

impl MsData {
    /// The reader of `init_ms_SR` (`egsnrc.mortran:3266-3306`) on an
    /// in-memory copy of the data file (list-directed text: for every
    /// `(i, j)`, records of `MAXU_MS + 1` `u` values, `MAXU_MS + 1` `f`
    /// values, `MAXU_MS` `w` values and `MAXU_MS` `i` values; a record's
    /// surplus values are ignored as a Fortran `read` ignores them). Reading
    /// the file from disk is the caller's. EGSnrc's file is not vendored here.
    ///
    /// # Errors
    ///
    /// A message when the text is short or a value does not parse.
    pub fn from_text(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let mut record = |n: usize| read_record(&mut lines, n);
        let mut ums = alloc::vec![0.0_f32; MS_N];
        let mut fms = alloc::vec![0.0_f32; MS_N];
        let mut wms = alloc::vec![0.0_f32; MS_N];
        let mut ims = alloc::vec![0_i16; MS_N];
        for i in 0..=MAXL_MS {
            for j in 0..=MAXQ_MS {
                let u = record(MAXU_MS + 1)?;
                let f = record(MAXU_MS + 1)?;
                let w = record(MAXU_MS)?;
                let ii = record(MAXU_MS)?;
                for k in 0..=MAXU_MS {
                    ums[ms_idx(i, j, k)] = u[k] as f32;
                    fms[ms_idx(i, j, k)] = f[k] as f32;
                }
                for k in 0..MAXU_MS {
                    wms[ms_idx(i, j, k)] = w[k] as f32;
                    ims[ms_idx(i, j, k)] = ii[k] as i16;
                }
                for k in 0..MAXU_MS {
                    // fms(k) = fms(k+1)/fms(k) - 1 in double, stored real*4
                    let a = fms[ms_idx(i, j, k)] as f64;
                    let b = fms[ms_idx(i, j, k + 1)] as f64;
                    fms[ms_idx(i, j, k)] = (b / a - 1.0) as f32;
                    ims[ms_idx(i, j, k)] -= 1;
                }
                fms[ms_idx(i, j, MAXU_MS)] = fms[ms_idx(i, j, MAXU_MS - 1)];
            }
        }
        let llammin = RealMath::r_ln(LAMBMIN_MS) as f32 as f64;
        let llammax = RealMath::r_ln(LAMBMAX_MS) as f32 as f64;
        let dllamb = ((llammax - llammin) / MAXL_MS as f64) as f32 as f64;
        let dllambi = (1.0 / dllamb) as f32;
        let dqms = (QMAX_MS / MAXQ_MS as f64) as f32 as f64;
        let dqmsi = (1.0 / dqms) as f32;
        Ok(Self { ums, fms, wms, ims, dllambi, dqmsi })
    }
}

/// `$MAXE_SPIN`, `$MAXQ_SPIN`, `$MAXU_SPIN` (`egsnrc.macros:877-880`).
pub const MAXE_SPIN: usize = 15;
/// See [`MAXE_SPIN`].
pub const MAXQ_SPIN: usize = 15;
/// See [`MAXE_SPIN`].
pub const MAXU_SPIN: usize = 31;
/// `$MAXE_SPI1 = 2 * $MAXE_SPIN + 1`.
pub const MAXE_SPI1: usize = 2 * MAXE_SPIN + 1;

/// `COMMON/SPIN-DATA/` (`egsnrc.macros:884-896`): the spin (Mott)
/// correction tables, `real*4` upstream and stored as `f32` here.
/// `spin_rej` is indexed `[medium][qel][i][j][k]`, flattened; the data is
/// built by `init_spin` (not ported), so the fields are public.
#[derive(Debug, Clone, Default)]
pub struct SpinData {
    /// `spin_rej`, `(nmed, 2, MAXE_SPI1 + 1, MAXQ_SPIN + 1, MAXU_SPIN + 1)`
    /// with `medium` 1-based mapped to `medium - 1`.
    pub spin_rej: Vec<f32>,
    /// `espml`.
    pub espml: f32,
    /// `b2spin_min`.
    pub b2spin_min: f32,
    /// `dbeta2i`.
    pub dbeta2i: f32,
    /// `dleneri`.
    pub dleneri: f32,
    /// `dqq1i`.
    pub dqq1i: f32,
}

fn spin_idx(medium: usize, qel: usize, i: usize, j: usize, k: usize) -> usize {
    ((((medium - 1) * 2 + qel) * (MAXE_SPI1 + 1) + i) * (MAXQ_SPIN + 1) + j) * (MAXU_SPIN + 1) + k
}

/// The variables `spin_rejection` keeps between calls (`save i, j`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SpinState {
    /// Energy index `i`.
    pub i: usize,
    /// `q1` index `j`.
    pub j: usize,
}

/// `spin_rejection` (`egsnrc.mortran:3163-3221`): the rejection function from
/// spin effects for charge `qel` (0 for e-, 1 for e+), `elke = ln(E)`, speed
/// `beta2`, first MS moment `q1` and `cost`. With `spin_index` set the energy
/// and `q1` bins are re-chosen (stochastically) and stored in `state`.
pub fn spin_rejection<R: UniformSource>(
    spin: &SpinData,
    qel: usize,
    medium: usize,
    elke: f64,
    beta2: f64,
    q1: f64,
    cost: f64,
    spin_index: &mut bool,
    is_single: bool,
    state: &mut SpinState,
    rng: &mut R,
) -> f64 {
    if *spin_index {
        *spin_index = false;
        let mut ai;
        let mut i;
        if beta2 >= spin.b2spin_min as f64 {
            ai = (beta2 - spin.b2spin_min as f64) * spin.dbeta2i as f64;
            i = ai as usize;
            ai -= i as f64;
            i += MAXE_SPIN + 1;
        } else if elke > spin.espml as f64 {
            ai = (elke - spin.espml as f64) * spin.dleneri as f64;
            i = ai as usize;
            ai -= i as f64;
        } else {
            i = 0;
            ai = -1.0;
        }
        let rnno = rng.uniform();
        if rnno < ai {
            i += 1;
        }
        let j;
        if is_single {
            j = 0;
        } else {
            let mut qq1 = 2.0 * q1;
            qq1 /= 1.0 + qq1;
            let aj = qq1 * spin.dqq1i as f64;
            let mut jj = aj as usize;
            if jj >= MAXQ_SPIN {
                jj = MAXQ_SPIN;
            } else {
                let aj = aj - jj as f64;
                let rnno = rng.uniform();
                if rnno < aj {
                    jj += 1;
                }
            }
            j = jj;
        }
        state.i = i;
        state.j = j;
    }
    let xi = Real::sqrt(0.5 * (1.0 - cost));
    let mut ak = xi * MAXU_SPIN as f64;
    let k = ak as usize;
    ak -= k as f64;
    let (i, j) = (state.i, state.j);
    (1.0 - ak) * spin.spin_rej[spin_idx(medium, qel, i, j, k)] as f64
        + ak * spin.spin_rej[spin_idx(medium, qel, i, j, k + 1)] as f64
}

/// `sscat` (`egsnrc.mortran:3225-3260`): single elastic scattering,
/// returning `(cost, sint)`.
pub fn sscat<R: UniformSource>(
    spin: &SpinData,
    chia2: f64,
    elke: f64,
    beta2: f64,
    qel: usize,
    medium: usize,
    spin_effects: bool,
    state: &mut SpinState,
    rng: &mut R,
) -> (f64, f64) {
    let mut spin_index = true;
    loop {
        let mut xi = rng.uniform();
        xi = 2.0 * chia2 * xi / (1.0 - xi + chia2);
        let cost = 1.0 - xi;
        if spin_effects {
            let qzero = 0.0;
            let rejf = spin_rejection(spin, qel, medium, elke, beta2, qzero, cost, &mut spin_index, true, state, rng);
            let rnno = rng.uniform();
            if rnno > rejf {
                continue;
            }
        }
        return (cost, Real::sqrt(xi * (2.0 - xi)));
    }
}

/// What `mscat` keeps between calls (`save i, j, omega2`), so a re-iteration
/// with `find_index = false` reuses them.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MscatState {
    /// Lambda bin.
    pub i: usize,
    /// `q1` bin.
    pub j: usize,
    /// `omega2`.
    pub omega2: f64,
}

/// `mscat` (`egsnrc.mortran:2983-3153`): multiple elastic scattering angle
/// `(cost, sint)` from the exact screened-Rutherford distribution (with the
/// Mott correction if `spin_effects`). `lambda` is the number of elastic MFPs
/// in the step, `chia2` the screening angle squared.
///
/// # Errors
///
/// [`EgsError::Fatal`] when `lambda > LAMBMAX_MS` (upstream stops: "Maximum
/// step size in mscat exceeded").
#[allow(clippy::too_many_arguments)]
pub fn mscat<R: UniformSource>(
    ms: &MsData,
    spin: &SpinData,
    lambda: f64,
    chia2: f64,
    q1: f64,
    elke: f64,
    beta2: f64,
    qel: usize,
    medium: usize,
    spin_effects: bool,
    find_index: &mut bool,
    spin_index: &mut bool,
    state: &mut MscatState,
    spin_state: &mut SpinState,
    rng: &mut R,
) -> Result<(f64, f64), EgsError> {
    if lambda <= 13.8 {
        let sprob = rng.uniform();
        let explambda = RealMath::r_exp(-lambda);
        if sprob < explambda {
            // no scattering
            return Ok((1.0, 0.0));
        }
        let wsum = (1.0 + lambda) * explambda;
        if sprob < wsum {
            loop {
                let mut xi = rng.uniform();
                xi = 2.0 * chia2 * xi / (1.0 - xi + chia2);
                let cost = 1.0 - xi;
                if spin_effects {
                    let rejf = spin_rejection(
                        spin, qel, medium, elke, beta2, q1, cost, spin_index, false, spin_state, rng,
                    );
                    let rnno = rng.uniform();
                    if rnno > rejf {
                        continue;
                    }
                }
                return Ok((cost, Real::sqrt(xi * (2.0 - xi))));
            }
        }
        if lambda <= 1.0 {
            let mut wprob = explambda;
            let mut wsum = explambda;
            let mut cost = 1.0;
            let mut sint = 0.0;
            let mut icount = 0;
            loop {
                icount += 1;
                if icount > 20 {
                    break;
                }
                wprob = wprob * lambda / icount as f64;
                wsum += wprob;
                let cosz;
                let mut xi;
                loop {
                    xi = rng.uniform();
                    xi = 2.0 * chia2 * xi / (1.0 - xi + chia2);
                    let c = 1.0 - xi;
                    if spin_effects {
                        let rejf = spin_rejection(
                            spin, qel, medium, elke, beta2, q1, c, spin_index, false, spin_state, rng,
                        );
                        let rnno = rng.uniform();
                        if rnno > rejf {
                            continue;
                        }
                    }
                    cosz = c;
                    break;
                }
                let mut sinz = xi * (2.0 - xi);
                if sinz > 1.0e-20 {
                    sinz = Real::sqrt(sinz);
                    xi = rng.uniform();
                    let phi = xi * 6.2831853;
                    cost = cost * cosz - sint * sinz * RealMath::r_cos(phi);
                    sint = Real::sqrt((0.0_f64).max((1.0 - cost) * (1.0 + cost)));
                }
                if wsum > sprob {
                    break;
                }
            }
            return Ok((cost, sint));
        }
    }

    // a multiple scattering event: sample from the q^(2+) surface
    if lambda <= LAMBMAX_MS {
        if *find_index {
            let llmbda = RealMath::r_ln(lambda);
            let mut ai = llmbda * ms.dllambi as f64;
            let mut i = ai as usize;
            ai -= i as f64;
            let xi = rng.uniform();
            if xi < ai {
                i += 1;
            }
            let j;
            if q1 < QMIN_MS {
                j = 0;
            } else if q1 < QMAX_MS {
                let mut aj = q1 * ms.dqmsi as f64;
                let mut jj = aj as usize;
                aj -= jj as f64;
                let xi = rng.uniform();
                if xi < aj {
                    jj += 1;
                }
                j = jj;
            } else {
                j = MAXQ_MS;
            }
            state.omega2 = if llmbda < 2.2299 {
                chia2
                    * (lambda + 4.0)
                    * (1.347006
                        + llmbda * (0.209364 - llmbda * (0.45525 - llmbda * (0.50142 - 0.081234 * llmbda))))
            } else {
                chia2
                    * (lambda + 4.0)
                    * (-2.77164 + llmbda * (2.94874 - llmbda * (0.1535754 - llmbda * 0.00552888)))
            };
            state.i = i;
            state.j = j;
            *find_index = false;
        }
        loop {
            let xi = rng.uniform();
            let mut ak = xi * MAXU_MS as f64;
            let mut k = ak as usize;
            ak -= k as f64;
            let (i, j) = (state.i, state.j);
            if ak > ms.wms[ms_idx(i, j, k)] as f64 {
                k = ms.ims[ms_idx(i, j, k)] as usize;
            }
            let a = ms.fms[ms_idx(i, j, k)] as f64;
            let mut u = ms.ums[ms_idx(i, j, k)] as f64;
            let du = ms.ums[ms_idx(i, j, k + 1)] as f64 - u;
            let xi = rng.uniform();
            if a.abs() < 0.2 {
                let x1 = 0.5 * (1.0 - xi) * a;
                u += xi * du * (1.0 + x1 * (1.0 - xi * a));
            } else {
                u -= du / a * (1.0 - Real::sqrt(1.0 + xi * a * (2.0 + a)));
            }
            let mut xi = state.omega2 * u / (1.0 + 0.5 * state.omega2 - u);
            if xi > 1.99999 {
                xi = 1.99999;
            }
            let cost = 1.0 - xi;
            if spin_effects {
                let rejf =
                    spin_rejection(spin, qel, medium, elke, beta2, q1, cost, spin_index, false, spin_state, rng);
                let rnno = rng.uniform();
                if rnno > rejf {
                    continue;
                }
            }
            return Ok((cost, Real::sqrt(xi * (2.0 - xi))));
        }
    }
    Err(EgsError::Fatal("mscat: maximum step size exceeded (lambda > LAMBMAX_MS)"))
}

/// A piecewise-linear fit `f(lelke) = a1(lelke) * elke + a0(lelke)`
/// (`$EVALUATE f USING name(elke)` with the interval `lelke` already set,
/// `egsnrc.macros:1237-1247`). Both vectors are 1-based.
#[derive(Debug, Clone, Default)]
pub struct Pwlf {
    /// `name0(lelke, medium)`.
    pub a0: Vec<f64>,
    /// `name1(lelke, medium)`.
    pub a1: Vec<f64>,
}

impl Pwlf {
    /// `a1(l) * x + a0(l)`.
    pub fn eval(&self, l: usize, x: f64) -> f64 {
        self.a1[l] * x + self.a0[l]
    }
}

/// The per-medium data `msdist_pI` and `msdist_pII` read (`COMMON/ELECIN/`),
/// for one medium.
#[derive(Debug, Clone, Default)]
pub struct MsMedium {
    /// `blcc(medium)`: multiple-scattering parameter (with the Euler constant
    /// absorbed, as `mscati` leaves it).
    pub blcc: f64,
    /// `xcc(medium)` (squared, as `mscati` leaves it).
    pub xcc: f64,
    /// `eke0`, `eke1`: `lelke = eke1 * elke + eke0`.
    pub eke0: f64,
    /// See `eke0`.
    pub eke1: f64,
    /// `etae_ms`, `etap_ms`: screening-angle correction for e-, e+.
    pub etae_ms: Pwlf,
    /// See `etae_ms`.
    pub etap_ms: Pwlf,
    /// `q1ce_ms`, `q1cp_ms`: first-moment correction.
    pub q1ce_ms: Pwlf,
    /// See `q1ce_ms`.
    pub q1cp_ms: Pwlf,
    /// `q2ce_ms`, `q2cp_ms`: second-moment correction (`msdist_pII` only).
    pub q2ce_ms: Pwlf,
    /// See `q2ce_ms`.
    pub q2cp_ms: Pwlf,
    /// `blcce`: correction to `blcc`.
    pub blcce: Pwlf,
}

/// The inputs of `msdist_pI` / `msdist_pII`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MsDistInput {
    /// `e0`: kinetic energy at the start of the step, MeV.
    pub e0: f64,
    /// `eloss`: energy loss for the step.
    pub eloss: f64,
    /// `tustep`: total path length, cm.
    pub tustep: f64,
    /// `rhof`: density scaling.
    pub rhof: f64,
    /// `qel`: 0 for e-, 1 for e+.
    pub qel: usize,
    /// `spin_effects`.
    pub spin_effects: bool,
    /// Initial direction cosines.
    pub u0: f64,
    /// See `u0`.
    pub v0: f64,
    /// See `u0`.
    pub w0: f64,
    /// Initial position, cm.
    pub x0: f64,
    /// See `x0`.
    pub y0: f64,
    /// See `x0`.
    pub z0: f64,
}

/// The outputs of `msdist_pI` / `msdist_pII`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MsDistOutput {
    /// Direction cosines after the step.
    pub us: f64,
    /// See `us`.
    pub vs: f64,
    /// See `us`.
    pub ws: f64,
    /// Final position, cm.
    pub xf: f64,
    /// See `xf`.
    pub yf: f64,
    /// See `xf`.
    pub zf: f64,
    /// Straight-line distance between the initial and final position.
    pub ustep: f64,
}

/// The `SET INTERVAL elke,eke` of the spin-effect branches: `lelke`, with
/// the "should normally not happen" clamp (`egsnrc.mortran:4119-4122`).
fn spin_interval(med: &MsMedium, elke: f64) -> (usize, f64) {
    let l = (med.eke1 * elke + med.eke0) as i64;
    if l < 1 {
        (1, (1.0 - med.eke0) / med.eke1)
    } else {
        (l as usize, elke)
    }
}

/// Rotate the scattering and transport directions from the frame where the
/// initial direction is along z into the lab frame, then transport
/// (`egsnrc.mortran:4262-4290`, shared by both routines).
#[allow(clippy::too_many_arguments)]
fn ms_rotate_and_transport(
    inp: &MsDistInput,
    (mut us, mut vs, mut ws): (f64, f64, f64),
    (mut ut, mut vt, mut wt): (f64, f64, f64),
) -> MsDistOutput {
    let sint02 = inp.u0 * inp.u0 + inp.v0 * inp.v0;
    if sint02 > 1e-20 {
        let sint0 = Real::sqrt(sint02);
        let sint0i = 1.0 / sint0;
        let cphi0 = sint0i * inp.u0;
        let sphi0 = sint0i * inp.v0;
        // scattering angles
        let mut u2p = inp.w0 * us + sint0 * ws;
        ws = inp.w0 * ws - sint0 * us;
        us = u2p * cphi0 - vs * sphi0;
        vs = u2p * sphi0 + vs * cphi0;
        // transport angles
        u2p = inp.w0 * ut + sint0 * wt;
        wt = inp.w0 * wt - sint0 * ut;
        ut = u2p * cphi0 - vt * sphi0;
        vt = u2p * sphi0 + vt * cphi0;
    } else {
        wt *= inp.w0;
        ws *= inp.w0;
    }
    MsDistOutput {
        us,
        vs,
        ws,
        xf: inp.x0 + inp.tustep * ut,
        yf: inp.y0 + inp.tustep * vt,
        zf: inp.z0 + inp.tustep * wt,
        ustep: 0.0,
    }
}

/// `msdist_pI` (`egsnrc.mortran:4309-4455`): multiple elastic scattering and
/// the spatial deflection for a path length `tustep`, as PRESTA-I does it.
/// `rmt2 = 2 RM`, `rmsq = RM^2`.
///
/// # Errors
///
/// From [`mscat`].
#[allow(clippy::too_many_arguments)]
pub fn msdist_p1<R: UniformSource>(
    med: &MsMedium,
    ms: &MsData,
    spin: &SpinData,
    inp: &MsDistInput,
    medium: usize,
    rng: &mut R,
) -> Result<MsDistOutput, EgsError> {
    let rmt2 = 2.0 * RM;
    let rmsq = RM * RM;
    let blccc = med.blcc;
    let xcccc = med.xcc;
    let e = inp.e0 - 0.5 * inp.eloss;
    let p2 = e * (e + rmt2);
    let p2i = 1.0 / p2;
    let _chia2 = xcccc * p2i / (4.0 * blccc);
    let beta2 = p2 / (p2 + rmsq);
    let mut lambda = inp.tustep * inp.rhof * blccc / beta2;

    // account for energy loss in the MS distribution
    let factor = 1.0 / (1.0 + 0.9784671 * e);
    let mut epsilon = inp.eloss / inp.e0;
    epsilon /= 1.0 - 0.5 * epsilon;
    let temp = 0.25 * (1.0 - factor * (1.0 - 0.333333 * factor)) * epsilon * epsilon;
    lambda *= 1.0 + temp;

    let (etap, xi_corr, ms_corr);
    let mut elke = 0.0;
    if inp.spin_effects {
        elke = RealMath::r_ln(e);
        let (l, el) = spin_interval(med, elke);
        elke = el;
        if inp.qel == 0 {
            etap = med.etae_ms.eval(l, elke);
            xi_corr = med.q1ce_ms.eval(l, elke);
        } else {
            etap = med.etap_ms.eval(l, elke);
            xi_corr = med.q1cp_ms.eval(l, elke);
        }
        ms_corr = med.blcce.eval(l, elke);
    } else {
        etap = 1.0;
        xi_corr = 1.0;
        ms_corr = 1.0;
    }
    let chia2 = xcccc * p2i / (4.0 * blccc) * etap;
    lambda = lambda / etap / (1.0 + chia2) * ms_corr;
    let chilog = RealMath::r_ln(1.0 + 1.0 / chia2);
    let q1 = 2.0 * chia2 * (chilog * (1.0 + chia2) - 1.0);
    let mut xi = q1 * lambda;

    // sample the multiple scattering angle
    let (mut find_index, mut spin_index) = (true, true);
    let mut st = MscatState::default();
    let mut ss = SpinState::default();
    let (ws, sint) = mscat(
        ms, spin, lambda, chia2, xi, elke, beta2, inp.qel, medium, inp.spin_effects, &mut find_index,
        &mut spin_index, &mut st, &mut ss, rng,
    )?;
    let (cphi, sphi) = select_azimuthal_angle(rng);
    let us = sint * cphi;
    let vs = sint * sphi;

    // correct xi used for the PLC calculation for spin effects
    xi *= xi_corr;
    let z = if xi < 0.1 {
        1.0 - xi * (0.5 - xi * (0.166666667 - 0.041666667 * xi))
    } else {
        (1.0 - RealMath::r_exp(-xi)) / xi
    };
    let mut r = 0.5 * sint;
    let mut r2 = r * r;
    let z2 = z * z;
    let r2max = 1.0 - z2;
    if r2max < r2 {
        r2 = r2max;
        r = Real::sqrt(r2);
    }
    let (ut, vt, wt) = (r * cphi, r * sphi, z);
    let ustep = Real::sqrt(z2 + r2) * inp.tustep;
    let mut out = ms_rotate_and_transport(inp, (us, vs, ws), (ut, vt, wt));
    out.ustep = ustep;
    Ok(out)
}

/// `msdist_pII` (`egsnrc.mortran:4049-4290`): multiple elastic scattering and
/// the spatial deflection for a path length `tustep`, as PRESTA-II does it
/// (two substeps with randomised transport distances). `count_p2_steps` is
/// `count_pII_steps` (`COMMON/CH-Steps/`).
///
/// # Errors
///
/// From [`mscat`].
#[allow(clippy::too_many_arguments)]
pub fn msdist_p2<R: UniformSource>(
    med: &MsMedium,
    ms: &MsData,
    spin: &SpinData,
    inp: &MsDistInput,
    medium: usize,
    count_p2_steps: &mut f64,
    rng: &mut R,
) -> Result<MsDistOutput, EgsError> {
    let rmt2 = 2.0 * RM;
    let rmsq = RM * RM;
    let prm = RM;
    *count_p2_steps += 1.0;
    let blccc = med.blcc;
    let xcccc = med.xcc;

    // commonly used factors
    let mut e = inp.e0 - 0.5 * inp.eloss;
    let tau = e / prm;
    let tau2 = tau * tau;
    let epsilon = inp.eloss / inp.e0;
    let epsilonp = inp.eloss / e;
    e *= 1.0 - epsilonp * epsilonp * (6.0 + 10.0 * tau + 5.0 * tau2) / (24.0 * tau2 + 72.0 * tau + 48.0);
    let p2 = e * (e + rmt2);
    let beta2 = p2 / (p2 + rmsq);
    let mut chia2 = xcccc / (4.0 * p2 * blccc);
    let mut lambda = 0.5 * inp.tustep * inp.rhof * blccc / beta2; // a half step
    let q = epsilonp / ((tau + 1.0) * (tau + 2.0));
    let temp2 = 0.166666 * (4.0 + tau * (6.0 + tau * (7.0 + tau * (4.0 + tau)))) * q * q;
    lambda *= 1.0 - temp2;

    let (etap, xi_corr, mut gamma, ms_corr);
    let mut elke = 0.0;
    if inp.spin_effects {
        elke = RealMath::r_ln(e);
        let (l, el) = spin_interval(med, elke);
        elke = el;
        if inp.qel == 0 {
            etap = med.etae_ms.eval(l, elke);
            xi_corr = med.q1ce_ms.eval(l, elke);
            gamma = med.q2ce_ms.eval(l, elke);
        } else {
            etap = med.etap_ms.eval(l, elke);
            xi_corr = med.q1cp_ms.eval(l, elke);
            gamma = med.q2cp_ms.eval(l, elke);
        }
        ms_corr = med.blcce.eval(l, elke);
    } else {
        etap = 1.0;
        xi_corr = 1.0;
        gamma = 1.0;
        ms_corr = 1.0;
    }
    chia2 *= etap;
    lambda = lambda / (etap * (1.0 + chia2)) * ms_corr;
    let chilog = RealMath::r_ln(1.0 + 1.0 / chia2);
    let q1 = 2.0 * chia2 * (chilog * (1.0 + chia2) - 1.0);
    gamma = 6.0 * chia2 * (1.0 + chia2) * (chilog * (1.0 + 2.0 * chia2) - 2.0) / q1 * gamma;
    let mut xi = q1 * lambda;

    // first substep scattering angle
    let (mut find_index, mut spin_index) = (true, true);
    let mut st = MscatState::default();
    let mut ss = SpinState::default();
    let (w1, sint1) = mscat(
        ms, spin, lambda, chia2, xi, elke, beta2, inp.qel, medium, inp.spin_effects, &mut find_index,
        &mut spin_index, &mut st, &mut ss, rng,
    )?;
    let (cphi1, sphi1) = select_azimuthal_angle(rng);
    // second substep (re-using the indices the first call found)
    let (w2, sint2) = mscat(
        ms, spin, lambda, chia2, xi, elke, beta2, inp.qel, medium, inp.spin_effects, &mut find_index,
        &mut spin_index, &mut st, &mut ss, rng,
    )?;
    let (cphi2, sphi2) = select_azimuthal_angle(rng);

    // final direction of motion, relative to the z axis
    let u2 = sint2 * cphi2;
    let v2 = sint2 * sphi2;
    let u2p = w1 * u2 + sint1 * w2;
    let us = u2p * cphi1 - v2 * sphi1;
    let vs = u2p * sphi1 + v2 * cphi1;
    let ws = w1 * w2 - sint1 * u2;

    // delta, b, c
    xi = 2.0 * xi * xi_corr; // xi was for a half step
    let mut eta = rng.uniform();
    eta = Real::sqrt(eta);
    let eta1 = 0.5 * (1.0 - eta);
    let mut delta = 0.9082483 - (0.1020621 - 0.0263747 * gamma) * xi;

    // correct the coefficients for energy loss
    let mut temp1 = 2.0 + tau;
    let mut temp = (2.0 + tau * temp1) / ((tau + 1.0) * temp1);
    temp -= (tau + 1.0) / ((tau + 2.0) * (chilog * (1.0 + chia2) - 1.0));
    temp *= epsilonp;
    temp1 = 1.0 - temp;
    delta += 0.40824829
        * (epsilon * (tau + 1.0)
            / ((tau + 2.0) * (chilog * (1.0 + chia2) - 1.0) * (chilog * (1.0 + 2.0 * chia2) - 2.0))
            - 0.25 * temp * temp);
    let b = eta * delta;
    let c = eta * (1.0 - delta);

    // transport direction cosines
    let w1v2 = w1 * v2;
    let ut = b * sint1 * cphi1 + c * (cphi1 * u2 - sphi1 * w1v2) + eta1 * us * temp1;
    let vt = b * sint1 * sphi1 + c * (sphi1 * u2 + cphi1 * w1v2) + eta1 * vs * temp1;
    let wt = eta1 * (1.0 + temp) + b * w1 + c * w2 + eta1 * ws * temp1;
    let ustep = inp.tustep * Real::sqrt(ut * ut + vt * vt + wt * wt);

    let mut out = ms_rotate_and_transport(inp, (us, vs, ws), (ut, vt, wt));
    out.ustep = ustep;
    Ok(out)
}

/// `$EPSEMFP` (`egsnrc.macros:1401`): smallest electron MFP value.
pub const EPSEMFP: f64 = 1.0e-8;

/// A geometry that can also say how far the nearest boundary is
/// (`$CALL-HOWNEAR(tperp)`, a user macro in EGSnrc; PRESTA-II aborts without it).
pub trait ElectronGeometry: Geometry {
    /// The perpendicular distance to the nearest boundary, cm.
    fn hownear(&mut self, stack: &Stack) -> f64;
}

/// Per-medium data `ELECTR` reads (`COMMON/ELECIN/`, `BREMPR`, `THRESH`,
/// filled by `HATCH` and `mscati`). Interval-indexed vectors are 1-based.
#[derive(Debug, Clone, Default)]
pub struct ElectronMediumData {
    /// `rho(medium)`.
    pub rho: f64,
    /// `eke0`, `eke1`: `lelke = eke1 * elke + eke0`.
    pub eke0: f64,
    /// See `eke0`.
    pub eke1: f64,
    /// `E_array(i, medium)`: the energies of the table, MeV, 1-based.
    pub e_array: Vec<f64>,
    /// `range_ep(qel, i, medium)`: range from table energy `i` to the lowest, `[qel][i]`.
    pub range_ep: [Vec<f64>; 2],
    /// `ededx`, `pdedx`: restricted stopping power for e-, e+ (its `a1` is the
    /// slope the range formulas use as `ededx1`).
    pub ededx: Pwlf,
    /// See `ededx`.
    pub pdedx: Pwlf,
    /// `esig`, `psig`: discrete-interaction cross sections.
    pub esig: Pwlf,
    /// See `esig`.
    pub psig: Pwlf,
    /// `ebr1`, `pbr1`, `pbr2`: branching ratios.
    pub ebr1: Pwlf,
    /// See `ebr1`.
    pub pbr1: Pwlf,
    /// See `ebr1`.
    pub pbr2: Pwlf,
    /// `tmxs`: maximum step for multiple scattering.
    pub tmxs: Pwlf,
    /// `esig_e`, `psig_e`: global maxima of cross section per energy loss.
    pub esig_e: f64,
    /// See `esig_e`.
    pub psig_e: f64,
    /// `sig_ismonotone(0..1)`.
    pub sig_ismonotone: [bool; 2],
    /// `TE(medium)`, `THMOLL(medium)`, `AE(medium)`, MeV.
    pub te: f64,
    /// See `te`.
    pub thmoll: f64,
    /// See `te`.
    pub ae: f64,
    /// Multiple-scattering data (`blcc`, `xcc`, and the spin fits).
    pub ms: MsMedium,
    /// Bremsstrahlung data.
    pub brems: BremsMedium,
}

/// Per-region data `ELECTR` reads.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ElectrRegion {
    /// `med(region)`; `0` is vacuum.
    pub med: usize,
    /// `rhor(region)`.
    pub rhor: f64,
    /// `ecut(region)`, total energy, MeV.
    pub ecut: f64,
    /// `smaxir(region)`: maximum step in the region, cm.
    pub smaxir: f64,
}

/// All tables `electr` needs; `regions` and `media` are 1-based.
#[derive(Debug, Clone, Default)]
pub struct ElectrTables {
    /// Regions.
    pub regions: Vec<ElectrRegion>,
    /// Media.
    pub media: Vec<ElectronMediumData>,
    /// The multiple-scattering `q^(2+)` surface.
    pub ms: Option<MsData>,
    /// The spin-effect tables.
    pub spin: SpinData,
}

/// Switches `ELECTR` reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ElectrConfig {
    /// `spin_effects`.
    pub spin_effects: bool,
    /// `exact_bca` (default true): exact boundary-crossing algorithm.
    pub exact_bca: bool,
    /// `skindepth_for_bca` (default 3): skin depth in elastic MFPs.
    pub skindepth_for_bca: f64,
    /// `transport_algorithm`: true PRESTA-II (default), false PRESTA-I.
    pub presta_ii: bool,
    /// `PRM`, MeV.
    pub prm: f64,
    /// `VACDST`: step through vacuum, cm.
    pub vacdst: f64,
    /// `nbr_split`, `ibrdst`, `ibr_nist`.
    pub brems: BremsConfig,
}

/// What `ELECTR` keeps between calls and the CH-step counters
/// (`COMMON/CH-Steps/`, `save ierust`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ElectrCounters {
    /// `count_pII_steps`.
    pub count_p2_steps: f64,
    /// `count_all_steps`.
    pub count_all_steps: f64,
    /// `ierust`: negative-`ustep` errors seen.
    pub ierust: i32,
}

/// What `electr` returns.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ElectrResult {
    /// `IRCODE`: `1` normal (a photon is on top, or the stack changed),
    /// `2` the particle was discarded (`np` decremented).
    pub ircode: i32,
    /// `EDEP` of a discard (valid when a particle was discarded), MeV.
    pub edep: f64,
    /// Sum of the energy loss `de` of the condensed-history steps, MeV
    /// (what upstream hands to `AUSGAB` as `EDEP` at each step).
    pub edep_steps: f64,
}

/// `$SET INTERVAL elke,eke` (`egsnrc.macros:1225`): the table interval.
fn eke_interval(m: &ElectronMediumData, elke: f64) -> usize {
    (m.eke1 * elke + m.eke0) as usize
}

/// The ELECTR-local variables the macros share (`$DEFINE-LOCAL-VARIABLES-ELECTR`).
#[derive(Debug, Clone, Copy, Default)]
struct Loc {
    blccl: f64,
    xccl: f64,
    p2: f64,
    beta2: f64,
    chia2: f64,
    xi: f64,
    xi_corr: f64,
    etap: f64,
    ms_corr: f64,
    ekems: f64,
    elkems: f64,
    rhof: f64,
}

/// `$EVALUATE ededx|pdedx USING ...(x)` with the index of that variable.
fn eval_dedx(m: &ElectronMediumData, lelec: i32, l: usize, x: f64) -> f64 {
    if lelec < 0 {
        m.ededx.eval(l, x)
    } else {
        m.pdedx.eval(l, x)
    }
}

/// `$COMPUTE-DRANGE(p1, p2, p3, p4, p5, result)` (`egsnrc.macros:3207-3236`):
/// the path length between energies `e1` and `e2` in interpolation bin `l`
/// (`ln e1 = el1`, `ln e2 = el2`).
fn compute_drange(m: &ElectronMediumData, lelec: i32, e1: f64, e2: f64, l: usize, el1: f64, el2: f64) -> f64 {
    let fedep = 1.0 - e2 / e1;
    let elktmp = 0.5 * (el1 + el2 + 0.25 * fedep * fedep * (1.0 + fedep * (1.0 + 0.875 * fedep)));
    let dedxmid = 1.0 / eval_dedx(m, lelec, l, elktmp);
    let slope = if lelec < 0 { m.ededx.a1[l] } else { m.pdedx.a1[l] };
    let mut aux = slope * dedxmid;
    let r = fedep / (2.0 - fedep);
    aux = aux * (1.0 + 2.0 * aux) * r * r / 6.0;
    fedep * e1 * dedxmid * (1.0 + aux)
}

/// `$COMPUTE-ELOSS(path, e, elog, l, de)` (`egsnrc.macros:3282-3300`): the
/// energy loss over `path` when initial and final energy share bin `l`.
fn compute_eloss(m: &ElectronMediumData, lelec: i32, rhof: f64, path: f64, e: f64, elog: f64, l: usize) -> f64 {
    let dedxmid = eval_dedx(m, lelec, l, elog);
    let slope = if lelec < 0 { m.ededx.a1[l] } else { m.pdedx.a1[l] };
    let aux = slope / dedxmid;
    let mut de = dedxmid * path * rhof;
    let fedep = de / e;
    de *= 1.0 - 0.5 * fedep * aux * (1.0 - 0.333333 * fedep * (aux - 1.0 - 0.25 * fedep * (2.0 - aux * (4.0 - aux))));
    de
}

/// `$COMPUTE-ELOSS-G(path, e, elog, l, de)` (`egsnrc.macros:3304-3350`).
#[allow(clippy::too_many_arguments)]
fn compute_eloss_g(
    m: &ElectronMediumData,
    lelec: i32,
    qel: usize,
    rhof: f64,
    range: f64,
    path: f64,
    e: f64,
    elog: f64,
    l: usize,
) -> f64 {
    let mut tuss = range - m.range_ep[qel][l] / rhof;
    if tuss >= path {
        compute_eloss(m, lelec, rhof, path, e, elog, l)
    } else {
        let mut lelktmp = l;
        tuss = (range - path) * rhof;
        if tuss <= 0.0 {
            e - m.te * 0.99
        } else {
            while tuss < m.range_ep[qel][lelktmp] {
                lelktmp -= 1;
            }
            let elktmp = ((lelktmp + 1) as f64 - m.eke0) / m.eke1;
            let eketmp = m.e_array[lelktmp + 1];
            tuss = (m.range_ep[qel][lelktmp + 1] - tuss) / rhof;
            compute_eloss(m, lelec, rhof, tuss, eketmp, elktmp, lelktmp) + e - eketmp
        }
    }
}

/// `$COMPUTE-RANGE` (`egsnrc.macros:3245-3256`).
fn compute_range(m: &ElectronMediumData, lelec: i32, qel: usize, rhof: f64, eke: f64, elke: f64, l: usize) -> f64 {
    let ekei = m.e_array[l];
    let elkei = (l as f64 - m.eke0) / m.eke1;
    let range = compute_drange(m, lelec, eke, ekei, l, elke, elkei);
    (range + m.range_ep[qel][l]) / rhof
}

/// `$EVALUATE-SIGF` (`egsnrc.macros:3125-3141`): cross section per energy
/// loss at `elke` (and `dedx0` as a side effect, returned too).
fn evaluate_sigf(m: &ElectronMediumData, lelec: i32, l: usize, elke: f64) -> (f64, f64) {
    let (sig, dedx0) = if lelec < 0 {
        (m.esig.eval(l, elke), m.ededx.eval(l, elke))
    } else {
        (m.psig.eval(l, elke), m.pdedx.eval(l, elke))
    };
    (sig / dedx0, dedx0)
}

/// `$CALCULATE-ELASTIC-SCATTERING-MFP(ssmfp, e, elog)` (`egsnrc.macros:2372-2392`).
fn elastic_scattering_mfp(m: &ElectronMediumData, loc: &mut Loc, cfg: &ElectrConfig, lelec: i32, eke: f64, elke: f64, l: usize) -> f64 {
    loc.blccl = loc.rhof * m.ms.blcc;
    loc.xccl = loc.rhof * m.ms.xcc;
    let rmt2 = 2.0 * RM;
    let rmsq = RM * RM;
    loc.p2 = eke * (eke + rmt2);
    loc.beta2 = loc.p2 / (loc.p2 + rmsq);
    if cfg.spin_effects {
        loc.etap = if lelec < 0 { m.ms.etae_ms.eval(l, elke) } else { m.ms.etap_ms.eval(l, elke) };
        loc.ms_corr = m.ms.blcce.eval(l, elke);
        loc.blccl = loc.blccl / loc.etap / (1.0 + 0.25 * loc.etap * loc.xccl / loc.blccl / loc.p2) * loc.ms_corr;
    }
    loc.beta2 / loc.blccl
}

/// `$CALCULATE-XI(path)` (`egsnrc.macros:1316-1338`), using `loc.ekems`.
fn calculate_xi(m: &ElectronMediumData, loc: &mut Loc, cfg: &ElectrConfig, lelec: i32, path: f64) {
    let rmt2 = 2.0 * RM;
    let rmsq = RM * RM;
    loc.p2 = loc.ekems * (loc.ekems + rmt2);
    loc.beta2 = loc.p2 / (loc.p2 + rmsq);
    loc.chia2 = loc.xccl / (4.0 * loc.blccl * loc.p2);
    loc.xi = 0.5 * loc.xccl / loc.p2 / loc.beta2 * path;
    if cfg.spin_effects {
        loc.elkems = RealMath::r_ln(loc.ekems);
        let l = eke_interval(m, loc.elkems);
        if lelec < 0 {
            loc.etap = m.ms.etae_ms.eval(l, loc.elkems);
            loc.xi_corr = m.ms.q1ce_ms.eval(l, loc.elkems);
        } else {
            loc.etap = m.ms.etap_ms.eval(l, loc.elkems);
            loc.xi_corr = m.ms.q1cp_ms.eval(l, loc.elkems);
        }
        loc.chia2 *= loc.etap;
        loc.xi *= loc.xi_corr;
        loc.ms_corr = m.ms.blcce.eval(l, loc.elkems);
        loc.blccl *= loc.ms_corr;
    } else {
        loc.xi_corr = 1.0;
        loc.etap = 1.0;
    }
    loc.xi *= RealMath::r_ln(1.0 + 1.0 / loc.chia2) - 1.0 / (1.0 + loc.chia2);
}

/// How the discard section is entered.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Discard {
    /// `:ECUT-DISCARD:`.
    Ecut,
    /// `:USER-ELECTRON-DISCARD:`.
    User,
}

/// `ELECTR` (`egsnrc.mortran:985-1825`): transport the charged particle on top
/// of the stack by condensed-history steps (PRESTA-II or PRESTA-I
/// boundary crossing, multiple scattering, continuous energy loss, discrete
/// Moller, Bhabha, bremsstrahlung and annihilation) until it is discarded or a
/// photon tops the stack.
///
/// Not ported: the EM-field macros (all empty by default), range rejection
/// (`i_do_rr = 0`), `$USER-RANGE-DISCARD`, the energy-loss fluctuation hook
/// (`$DE-FLUCTUATION`, empty), electron-impact ionisation (`eii_flag = 0`),
/// the `AUSGAB` calls, and all diagnostic output. The `ustep <= 0` message is
/// not printed but its 1000-error limit is kept.
///
/// **Assumption to check in phase 2:** upstream's `NEXT :TSTEP:` is read as a
/// restart of the `:TSTEP:` loop without re-testing its trailing
/// `UNTIL (rfict <= sigratio)` (re-testing stale random numbers would make
/// every vacuum step an interaction). The Mortran language definition is not
/// in the EGSnrc source; a differential run settles it.
///
/// # Errors
///
/// [`EgsError`] from the interaction routines; [`EgsError::Fatal`] where
/// upstream stops the run (too many negative `ustep`s, `xi > 1` in
/// `$SET-TVSTEP`, `lambda` beyond the table).
#[allow(clippy::too_many_lines, clippy::too_many_arguments)]
pub fn electr<R: UniformSource, G: ElectronGeometry>(
    stack: &mut Stack,
    tables: &ElectrTables,
    cfg: &ElectrConfig,
    geometry: &mut G,
    counters: &mut ElectrCounters,
    rng: &mut R,
) -> Result<ElectrResult, EgsError> {
    let rm = RM;
    let prm = cfg.prm;
    let rmt2 = 2.0 * RM;
    let rmsq = RM * RM;
    let ms = tables.ms.as_ref().ok_or(EgsError::Unsupported("ELECTR: no multiple-scattering data"))?;
    let mut res = ElectrResult { ircode: 1, ..Default::default() };

    let mut irl = stack.ir[stack.np] as usize;
    let mut medium = tables.regions[irl].med;
    let mut loc = Loc { etap: 1.0, xi_corr: 1.0, ms_corr: 1.0, rhof: 1.0, ..Default::default() };
    let (mut lelec, mut qel);
    let (mut peie, mut eie);
    let mut eke = 0.0;
    let mut elke = 0.0;
    let mut lelke = 0usize;
    let mut total_tstep = 0.0;
    let mut sig0 = 0.0;

    // :NEWELECTRON:
    'newelectron: loop {
        lelec = stack.iq[stack.np];
        qel = ((1 + lelec) / 2) as usize;
        peie = stack.e[stack.np];
        eie = peie;

        let mut discard: Option<Discard> = None;
        let mut idisc;

        if eie <= tables.regions[irl].ecut {
            discard = Some(Discard::Ecut);
        } else if stack.wt[stack.np] == 0.0 {
            discard = Some(Discard::User);
        }
        idisc = 0;

        if discard.is_none() {
            // :TSTEP:
            'tstep: loop {
                let mut compute_tstep = true;
                let mut demfp = 0.0;
                let mut tstep = 0.0;
                eke = eie - rm;
                if medium != 0 {
                    // $SELECT-ELECTRON-MFP
                    let mut rnne1 = rng.uniform();
                    if rnne1 == 0.0 {
                        rnne1 = 1.0e-30;
                    }
                    demfp = (-RealMath::r_ln(rnne1)).max(EPSEMFP);
                    elke = RealMath::r_ln(eke);
                    let m = &tables.media[medium];
                    lelke = eke_interval(m, elke);
                    // $EVALUATE-SIG0
                    if m.sig_ismonotone[qel] {
                        sig0 = evaluate_sigf(m, lelec, lelke, elke).0;
                    } else if lelec < 0 {
                        sig0 = m.esig_e;
                    } else {
                        sig0 = m.psig_e;
                    }
                }

                // :USTEP:
                loop {
                    let mut callhowfar;
                    let mut domultiple = false;
                    let mut dosingle = false;
                    let mut callmsdist = false;
                    let (mut uscat, mut vscat, mut wscat) = (0.0, 0.0, 0.0);
                    let (mut xtrans, mut ytrans, mut ztrans) = (0.0, 0.0, 0.0);
                    let mut ustep;
                    let mut tustep;
                    let mut dedx = 0.0;
                    let mut range = 0.0;
                    let mut de = 0.0;
                    let mut tvstep;
                    let mut sig = 0.0;
                    let m = &tables.media[medium.max(1).min(tables.media.len() - 1)];
                    let np = stack.np;

                    if medium == 0 {
                        // vacuum
                        tstep = cfg.vacdst;
                        ustep = tstep;
                        tustep = ustep;
                        callhowfar = true;
                    } else {
                        // non-vacuum
                        loc.rhof = tables.regions[irl].rhor / m.rho; // $SET-RHOF
                        sig = sig0; // $SCALE-SIG0
                        if sig <= 0.0 {
                            tstep = cfg.vacdst;
                            sig0 = 1.0e-15;
                        } else {
                            // $CALCULATE-TSTEP-FROM-DEMFP
                            if compute_tstep {
                                let total_de = demfp / sig;
                                let ekef = eke - total_de;
                                if ekef <= m.e_array[1] {
                                    tstep = cfg.vacdst;
                                } else {
                                    let elkef = RealMath::r_ln(ekef);
                                    let lelkef = eke_interval(m, elkef);
                                    if lelkef == lelke {
                                        tstep = compute_drange(m, lelec, eke, ekef, lelke, elke, elkef);
                                    } else {
                                        let ekei = m.e_array[lelke];
                                        let elkei = (lelke as f64 - m.eke0) / m.eke1;
                                        let tuss = compute_drange(m, lelec, eke, ekei, lelke, elke, elkei);
                                        let ekei = m.e_array[lelkef + 1];
                                        let elkei = ((lelkef + 1) as f64 - m.eke0) / m.eke1;
                                        tstep = compute_drange(m, lelec, ekei, ekef, lelkef, elkei, elkef);
                                        tstep = tstep + tuss + m.range_ep[qel][lelke] - m.range_ep[qel][lelkef + 1];
                                    }
                                }
                                total_tstep = tstep;
                                compute_tstep = false;
                            }
                            tstep = total_tstep / loc.rhof;
                        }

                        let dedx0 = eval_dedx(m, lelec, lelke, elke);
                        dedx = loc.rhof * dedx0;
                        let mut tmxs = m.tmxs.eval(lelke, elke) / loc.rhof;
                        range = compute_range(m, lelec, qel, loc.rhof, eke, elke, lelke);
                        // $RANDOMIZE-TUSTEP is .false.
                        tmxs = tmxs.min(tables.regions[irl].smaxir);
                        tustep = tstep.min(tmxs).min(range);
                        let tperp = geometry.hownear(stack);
                        stack.dnear[np] = tperp;
                        // $SET-SKINDEPTH(eke, elke)
                        let ssmfp = elastic_scattering_mfp(m, &mut loc, cfg, lelec, eke, elke, lelke);
                        let skindepth = cfg.skindepth_for_bca * ssmfp;
                        tustep = tustep.min(tperp.max(skindepth));

                        counters.count_all_steps += 1.0;
                        ustep = tustep;
                        tvstep = tustep;
                        de = 0.0;
                        if tustep <= tperp && (!cfg.exact_bca || tustep > skindepth) {
                            // a normal condensed-history step
                            callhowfar = false;
                            domultiple = false;
                            dosingle = false;
                            callmsdist = true;
                            de = compute_eloss_g(m, lelec, qel, loc.rhof, range, tustep, eke, elke, lelke);
                            tvstep = tustep;
                            let inp = MsDistInput {
                                e0: eke,
                                eloss: de,
                                tustep,
                                rhof: loc.rhof,
                                qel,
                                spin_effects: cfg.spin_effects,
                                u0: stack.u[np],
                                v0: stack.v[np],
                                w0: stack.w[np],
                                x0: stack.x[np],
                                y0: stack.y[np],
                                z0: stack.z[np],
                            };
                            let o = if cfg.presta_ii {
                                msdist_p2(&m.ms, ms, &tables.spin, &inp, medium, &mut counters.count_p2_steps, rng)?
                            } else {
                                msdist_p1(&m.ms, ms, &tables.spin, &inp, medium, rng)?
                            };
                            uscat = o.us;
                            vscat = o.vs;
                            wscat = o.ws;
                            xtrans = o.xf;
                            ytrans = o.yf;
                            ztrans = o.zf;
                            ustep = o.ustep;
                        } else {
                            // within a skin depth of a boundary
                            callmsdist = false;
                            if cfg.exact_bca {
                                domultiple = false;
                                let mut rnnoss = rng.uniform();
                                if rnnoss < 1.0e-30 {
                                    rnnoss = 1.0e-30;
                                }
                                let lambda = -RealMath::r_ln(1.0 - rnnoss);
                                let lambda_max = 0.5 * loc.blccl * rm / dedx * {
                                    let t = eke / rm + 1.0;
                                    t * t * t
                                };
                                if lambda >= 0.0 && lambda_max > 0.0 {
                                    let tuss = if lambda < lambda_max {
                                        lambda * ssmfp * (1.0 - 0.5 * lambda / lambda_max)
                                    } else {
                                        0.5 * lambda * ssmfp
                                    };
                                    if tuss < tustep {
                                        tustep = tuss;
                                        dosingle = true;
                                    } else {
                                        dosingle = false;
                                    }
                                } else {
                                    // upstream warns, drops the particle and returns
                                    stack.np -= 1;
                                    return Ok(res);
                                }
                                ustep = tustep;
                            } else {
                                // EGS4/PRESTA-I boundary crossing with exact PLC: $SET-USTEP
                                dosingle = false;
                                domultiple = true;
                                loc.ekems = eke - 0.5 * tustep * dedx;
                                calculate_xi(m, &mut loc, cfg, lelec, tustep);
                                ustep = if loc.xi < 0.1 {
                                    tustep * (1.0 - loc.xi * (0.5 - loc.xi * 0.166667))
                                } else {
                                    tustep * (1.0 - RealMath::r_exp(-loc.xi)) / loc.xi
                                };
                            }
                            callhowfar = ustep >= tperp;
                        }
                    }

                    let irold = stack.ir[np] as usize;
                    let mut st = HowfarState { ustep, irnew: irold, idisc: 0 };
                    let ustep0 = ustep;
                    // $CALL-HOWFAR-IN-ELECTR
                    if callhowfar || stack.wt[np] <= 0.0 {
                        geometry.howfar(stack, &mut st);
                    }
                    ustep = st.ustep;
                    let irnew = st.irnew;
                    idisc = st.idisc;
                    if idisc > 0 {
                        discard = Some(Discard::User);
                        break 'tstep;
                    }
                    // $CHECK-NEGATIVE-USTEP
                    if ustep <= 0.0 {
                        if ustep < -1e-4 {
                            counters.ierust += 1;
                            if counters.ierust > 1000 {
                                return Err(EgsError::Fatal("ELECTR: too many negative ustep errors"));
                            }
                        }
                        ustep = 0.0;
                    }

                    if ustep == 0.0 || medium == 0 {
                        // fast step in vacuum
                        if ustep != 0.0 {
                            let vstep = ustep;
                            stack.x[np] += stack.u[np] * vstep;
                            stack.y[np] += stack.v[np] * vstep;
                            stack.z[np] += stack.w[np] * vstep;
                            stack.dnear[np] -= vstep;
                        }
                        if irnew != irold {
                            stack.ir[np] = irnew as i32;
                            irl = irnew;
                            medium = tables.regions[irl].med;
                        }
                        if eie <= tables.regions[irl].ecut {
                            discard = Some(Discard::Ecut);
                            break 'tstep;
                        }
                        if ustep != 0.0 && idisc < 0 {
                            discard = Some(Discard::User);
                            break 'tstep;
                        }
                        continue 'tstep; // NEXT :TSTEP:
                    }

                    let vstep = ustep;
                    if callhowfar {
                        if cfg.exact_bca {
                            tvstep = vstep;
                            if tvstep != tustep {
                                dosingle = false;
                            }
                        } else {
                            // $SET-TVSTEP
                            if vstep < ustep0 {
                                loc.ekems = eke - 0.5 * tustep * vstep / ustep0 * dedx;
                                calculate_xi(m, &mut loc, cfg, lelec, vstep);
                                if loc.xi < 0.1 {
                                    tvstep = vstep * (1.0 + loc.xi * (0.5 + loc.xi * 0.333333));
                                } else if loc.xi < 0.999999 {
                                    tvstep = -vstep * RealMath::r_ln(1.0 - loc.xi) / loc.xi;
                                } else {
                                    return Err(EgsError::Fatal("ELECTR: xi > 1 in $SET-TVSTEP"));
                                }
                            } else {
                                tvstep = tustep;
                            }
                        }
                        de = compute_eloss_g(m, lelec, qel, loc.rhof, range, tvstep, eke, elke, lelke);
                    } else {
                        tvstep = tustep;
                        if !callmsdist {
                            de = compute_eloss_g(m, lelec, qel, loc.rhof, range, tvstep, eke, elke, lelke);
                        }
                    }
                    let save_de = de;
                    let edep = de; // $DE-FLUCTUATION is empty
                    let ekef = eke - de;
                    let _eold = eie;
                    let _enew = _eold - de;
                    let (mut costhe, mut sinthe) = (1.0, 0.0);

                    // multiple scattering
                    if !callmsdist {
                        if domultiple {
                            let lambda = loc.blccl * tvstep / loc.beta2 / loc.etap / (1.0 + loc.chia2);
                            let xi = loc.xi / loc.xi_corr;
                            let (mut findindex, mut spin_index) = (true, true);
                            let mut mst = MscatState::default();
                            let mut sst = SpinState::default();
                            let r = mscat(
                                ms, &tables.spin, lambda, loc.chia2, xi, loc.elkems, loc.beta2, qel, medium,
                                cfg.spin_effects, &mut findindex, &mut spin_index, &mut mst, &mut sst, rng,
                            )?;
                            costhe = r.0;
                            sinthe = r.1;
                        } else if dosingle {
                            // single scattering
                            loc.ekems = ekef.max(tables.regions[irl].ecut - rm);
                            loc.p2 = loc.ekems * (loc.ekems + rmt2);
                            loc.beta2 = loc.p2 / (loc.p2 + rmsq);
                            loc.chia2 = m.ms.xcc / (4.0 * m.ms.blcc * loc.p2);
                            if cfg.spin_effects {
                                loc.elkems = RealMath::r_ln(loc.ekems);
                                let l = eke_interval(m, loc.elkems);
                                loc.etap = if lelec < 0 {
                                    m.ms.etae_ms.eval(l, loc.elkems)
                                } else {
                                    m.ms.etap_ms.eval(l, loc.elkems)
                                };
                                loc.chia2 *= loc.etap;
                            }
                            let mut sst = SpinState::default();
                            let r = sscat(
                                &tables.spin, loc.chia2, loc.elkems, loc.beta2, qel, medium, cfg.spin_effects,
                                &mut sst, rng,
                            );
                            costhe = r.0;
                            sinthe = r.1;
                        } else {
                            sinthe = 0.0;
                            costhe = 1.0;
                        }
                    }

                    // final position and direction
                    let (x_final, y_final, z_final, u_final, v_final, w_final);
                    if callmsdist {
                        u_final = uscat;
                        v_final = vscat;
                        w_final = wscat;
                        x_final = xtrans;
                        y_final = ytrans;
                        z_final = ztrans;
                    } else {
                        x_final = stack.x[np] + stack.u[np] * vstep;
                        y_final = stack.y[np] + stack.v[np] * vstep;
                        z_final = stack.z[np] + stack.w[np] * vstep;
                        if domultiple || dosingle {
                            let (u_tmp, v_tmp, w_tmp) = (stack.u[np], stack.v[np], stack.w[np]);
                            let mut saved = UphiSaved::default();
                            uphi(stack, &mut saved, 2, 1, sinthe, costhe, rng)?;
                            u_final = stack.u[np];
                            v_final = stack.v[np];
                            w_final = stack.w[np];
                            stack.u[np] = u_tmp;
                            stack.v[np] = v_tmp;
                            stack.w[np] = w_tmp;
                        } else {
                            u_final = stack.u[np];
                            v_final = stack.v[np];
                            w_final = stack.w[np];
                        }
                    }

                    // transport the particle
                    stack.x[np] = x_final;
                    stack.y[np] = y_final;
                    stack.z[np] = z_final;
                    stack.u[np] = u_final;
                    stack.v[np] = v_final;
                    stack.w[np] = w_final;
                    stack.dnear[np] -= vstep;
                    res.edep_steps += edep;

                    peie -= de;
                    eie = peie;
                    stack.e[np] = peie;
                    if irnew == irl && eie <= tables.regions[irl].ecut {
                        discard = Some(Discard::Ecut);
                        break 'tstep;
                    }
                    let medold = medium;
                    if medium != 0 {
                        eke = eie - rm;
                        elke = RealMath::r_ln(eke);
                        lelke = eke_interval(&tables.media[medium], elke);
                    }
                    if irnew != irold {
                        stack.ir[np] = irnew as i32;
                        irl = irnew;
                        medium = tables.regions[irl].med;
                    }
                    if eie <= tables.regions[irl].ecut {
                        discard = Some(Discard::Ecut);
                        break 'tstep;
                    }
                    if idisc < 0 {
                        discard = Some(Discard::User);
                        break 'tstep;
                    }
                    if medium != medold {
                        continue 'tstep;
                    }
                    // $UPDATE-DEMFP
                    demfp -= save_de * sig;
                    total_tstep -= tvstep * loc.rhof;
                    if total_tstep < 1e-9 {
                        demfp = 0.0;
                    }
                    if demfp < EPSEMFP {
                        break;
                    }
                }

                // compute the final sigma to see if a resample is needed
                let m = &tables.media[medium];
                let sigf = evaluate_sigf(m, lelec, lelke, elke).0;
                let sigratio = sigf / sig0;
                let rfict = rng.uniform();
                if rfict <= sigratio {
                    break 'tstep;
                }
            }
        }

        if discard.is_none() {
            // now sample the electron interaction
            let m = &tables.media[medium];
            let mut do_brems = false;
            if lelec < 0 {
                let ebr1 = m.ebr1.eval(lelke, elke);
                let rnno24 = rng.uniform();
                if rnno24 <= ebr1 {
                    do_brems = true;
                } else {
                    if stack.e[stack.np] <= m.thmoll {
                        // not enough energy for Moller: force bremsstrahlung if allowed
                        if ebr1 <= 0.0 {
                            continue 'newelectron;
                        }
                        do_brems = true;
                    }
                    if !do_brems {
                        moller(stack, m.te, m.thmoll, false, prm, rng)?;
                        if stack.iq[stack.np] == 0 {
                            return Ok(res);
                        }
                        continue 'newelectron;
                    }
                }
            } else {
                // e+: pbr1 = brems/(brems + bhabha + annih)
                let pbr1 = m.pbr1.eval(lelke, elke);
                let rnno25 = rng.uniform();
                if rnno25 < pbr1 {
                    do_brems = true;
                } else {
                    let pbr2 = m.pbr2.eval(lelke, elke);
                    if rnno25 < pbr2 {
                        bhabha(stack, m.te, prm, rng)?;
                        if stack.iq[stack.np] == 0 {
                            return Ok(res);
                        }
                    } else {
                        // in-flight annihilation
                        annih(stack, cfg.brems.nbr_split, rm, rng)?;
                        return Ok(res);
                    }
                    continue 'newelectron;
                }
            }
            if do_brems {
                brems(stack, &m.brems, &cfg.brems, elke, prm, rng)?;
                if stack.iq[stack.np] == 0 {
                    return Ok(res);
                }
                continue 'newelectron;
            }
        }

        // discard sections
        let kind = discard.unwrap_or(Discard::Ecut);
        let np = stack.np;
        let mut annihilate_at_rest = false;
        match kind {
            Discard::Ecut => {
                if medium > 0 {
                    if eie > tables.media[medium].ae {
                        res.edep = if lelec < 0 { stack.e[np] - prm } else { peie - prm };
                    } else {
                        res.edep = stack.e[np] - prm;
                    }
                } else {
                    res.edep = 0.0;
                }
                annihilate_at_rest = lelec > 0;
            }
            Discard::User => {
                let d = idisc.abs();
                res.edep = if lelec < 0 || d == 99 { stack.e[np] - prm } else { stack.e[np] + prm };
                annihilate_at_rest = d == 99 && lelec > 0;
            }
        }
        if annihilate_at_rest && res.edep < peie {
            annih_at_rest(stack, cfg.brems.nbr_split, prm, rng)?;
            res.ircode = 1;
            return Ok(res);
        }
        stack.np -= 1;
        res.ircode = 2;
        return Ok(res);
    }
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


    /// A slab 0 < z < L (region 1, medium 1) in vacuum (region 2): the photon
    /// is discarded once outside.
    struct Slab {
        length: f64,
    }

    impl Geometry for Slab {
        fn howfar(&mut self, stack: &Stack, st: &mut HowfarState) {
            let np = stack.np;
            if stack.ir[np] == 2 {
                st.idisc = 1;
                return;
            }
            let w = stack.w[np];
            let z = stack.z[np];
            let d = if w > 0.0 {
                (self.length - z) / w
            } else if w < 0.0 {
                -z / w
            } else {
                f64::INFINITY
            };
            if st.ustep > d {
                st.ustep = d;
                st.irnew = 2;
            }
        }
    }

    fn photon_tables(gbr_pair: f64, gbr_compt: f64, ibcmp: i32) -> PhotonTables {
        let (photo, photo_med, _) = toy_photo_data();
        let med = PhotonMediumData {
            rho: 1.0,
            ge0: 1.0,
            ge1: 0.0,
            gmfp0: alloc::vec![0.0, 1.0],
            gmfp1: alloc::vec![0.0, 0.0],
            gbr10: alloc::vec![0.0, gbr_pair],
            gbr11: alloc::vec![0.0, 0.0],
            gbr20: alloc::vec![0.0, gbr_pair + gbr_compt],
            gbr21: alloc::vec![0.0, 0.0],
            photo: photo_med,
            pair: toy_pair_medium(),
        };
        let slab = PhotonRegionData { med: 1, rhor: 1.0, pcut: 0.01, ecut: RM + 0.01, ibcmp, iedgfl: 1, iphter: 1, irayl: 0, iphotonucr: 0 };
        let outside = PhotonRegionData { med: 0, rhor: 0.0, pcut: 0.01, ecut: RM + 0.01, ..slab };
        PhotonTables {
            regions: alloc::vec![PhotonRegionData::default(), slab, outside],
            media: alloc::vec![PhotonMediumData::default(), med],
            compton: toy_compton_data(0.01),
            relax: photo_relax_data(),
            photo,
        }
    }

    /// Follow one photon and everything it makes until the stack is empty:
    /// charged particles deposit their kinetic energy on the spot. Returns the
    /// energy balance residual.
    fn run_history(tables: &PhotonTables, cfg: &PhotonConfig, e0: f64, rng: &mut Ranmar) -> f64 {
        let mut st = Stack::new(200);
        st.np = 1;
        st.e[1] = e0;
        st.w[1] = 1.0;
        st.z[1] = 1e-4;
        st.ir[1] = 1;
        st.wt[1] = 1.0;
        let mut geo = Slab { length: 3.0 };
        let mut tally = 0.0;
        let mut pairs = 0;
        let mut guard = 0;
        while st.np > 0 {
            guard += 1;
            assert!(guard < 100_000, "history did not terminate");
            if st.iq[st.np] == 0 {
                let r = photon(&mut st, tables, cfg, &mut geo, rng).unwrap();
                tally += r.edep + r.edep_local;
                if r.ircode == 1 && st.np > 0 && st.iq[st.np] == 1 {
                    // (counted below when the positron is popped)
                }
            } else {
                let np = st.np;
                if st.iq[np] == 1 {
                    pairs += 1;
                }
                tally += st.e[np] - RM;
                st.np -= 1;
            }
        }
        tally + 2.0 * RM * pairs as f64 - e0
    }

    #[test]
    fn photon_histories_conserve_energy() {
        let mut rng = Ranmar::new(77, 78);
        // Compton + photoelectric only
        let t1 = photon_tables(0.0, 0.5, 3);
        let cfg = PhotonConfig { pair: PairConfig::default(), rr: None, prm: RM, vacdst: 100.0 };
        for _ in 0..200 {
            let r = run_history(&t1, &cfg, 0.1, &mut rng);
            assert!(r.abs() < 1e-9, "residual {r}");
        }
        // with pair production at 5 MeV, Klein-Nishina Compton
        let t2 = photon_tables(0.3, 0.4, 0);
        for _ in 0..200 {
            let r = run_history(&t2, &cfg, 5.0, &mut rng);
            assert!(r.abs() < 1e-9, "residual {r}");
        }
    }

    #[test]
    fn photon_discards_refusals_and_roulette() {
        let mut rng = Ranmar::new(3, 5);
        let tables = photon_tables(0.0, 0.5, 0);
        let cfg = PhotonConfig { pair: PairConfig::default(), rr: None, prm: RM, vacdst: 100.0 };
        let mut geo = Slab { length: 3.0 };
        // below pcut: discarded at once with its energy
        let mut st = Stack::new(10);
        st.np = 1;
        st.e[1] = 0.005;
        st.ir[1] = 1;
        st.wt[1] = 1.0;
        let r = photon(&mut st, &tables, &cfg, &mut geo, &mut rng).unwrap();
        assert_eq!((r.ircode, r.edep, st.np), (2, 0.005, 0));
        // zero weight: user discard
        let mut st = Stack::new(10);
        st.np = 1;
        st.e[1] = 1.0;
        st.ir[1] = 1;
        let r = photon(&mut st, &tables, &cfg, &mut geo, &mut rng).unwrap();
        assert_eq!((r.ircode, r.edep, st.np), (2, 1.0, 0));
        // a photon in the vacuum region is discarded by the geometry
        let mut st = Stack::new(10);
        st.np = 1;
        st.e[1] = 1.0;
        st.ir[1] = 2;
        st.wt[1] = 1.0;
        st.w[1] = 1.0;
        let r = photon(&mut st, &tables, &cfg, &mut geo, &mut rng).unwrap();
        assert_eq!(r.ircode, 2);
        // Rayleigh and photonuclear are refused
        let mut bad = tables.clone();
        bad.regions[1].irayl = 1;
        let mut st = Stack::new(10);
        st.np = 1;
        st.e[1] = 1.0;
        st.ir[1] = 1;
        st.wt[1] = 1.0;
        st.w[1] = 1.0;
        assert!(matches!(photon(&mut st, &bad, &cfg, &mut geo, &mut rng), Err(EgsError::Unsupported(_))));
        let mut bad2 = tables.clone();
        bad2.regions[1].iphotonucr = 1;
        let mut st2 = Stack::new(10);
        st2.np = 1;
        st2.e[1] = 1.0;
        st2.ir[1] = 1;
        st2.wt[1] = 1.0;
        st2.w[1] = 1.0;
        assert!(matches!(photon(&mut st2, &bad2, &cfg, &mut geo, &mut rng), Err(EgsError::Unsupported(_))));
        // Russian roulette: histories still terminate
        let cfg_rr = PhotonConfig { rr: Some(0.5), ..cfg };
        for _ in 0..50 {
            let _ = run_history(&tables, &cfg_rr, 0.1, &mut rng);
        }
        // a passing photon in vacuum (medium 0) uses VACDST and crosses regions
        let mut vac = tables.clone();
        vac.regions[1].med = 0;
        let mut st = Stack::new(10);
        st.np = 1;
        st.e[1] = 1.0;
        st.ir[1] = 1;
        st.wt[1] = 1.0;
        st.w[1] = 1.0;
        let r = photon(&mut st, &vac, &cfg, &mut geo, &mut rng).unwrap();
        assert_eq!((r.ircode, st.np), (2, 0));
    }


    /// Stands in for `ELECTR`: deposits the kinetic energy on the spot.
    struct DepositElectrons {
        kinetic: f64,
        positrons: usize,
    }

    impl ElectronTransport for DepositElectrons {
        fn electr<R: UniformSource>(&mut self, stack: &mut Stack, _rng: &mut R) -> Result<i32, EgsError> {
            let np = stack.np;
            self.kinetic += stack.e[np] - RM;
            if stack.iq[np] == 1 {
                self.positrons += 1;
            }
            stack.np -= 1;
            Ok(2)
        }
    }

    #[test]
    fn shower_balances_energy_for_each_starting_particle() {
        let mut rng = Ranmar::new(31, 32);
        let tables = photon_tables(0.2, 0.4, 0);
        let cfg = PhotonConfig { pair: PairConfig::default(), rr: None, prm: RM, vacdst: 100.0 };
        let mut geo = Slab { length: 3.0 };
        for &(iqi, ei) in &[(0, 5.0), (0, 0.1), (-1, 1.0 + RM), (1, 1.0 + RM)] {
            for _ in 0..50 {
                let mut st = Stack::new(200);
                let mut el = DepositElectrons { kinetic: 0.0, positrons: 0 };
                let start = ShowerStart { iqi, ei, xi: 0.0, yi: 0.0, zi: 1e-4, ui: 0.0, vi: 0.0, wi: 1.0, iri: 1, wti: 1.0 };
                let r = shower(&mut st, &tables, &cfg, &mut geo, &mut el, &start, &mut rng).unwrap();
                assert_eq!(st.np, 0);
                if iqi == 0 {
                    // photon start: everything ends up discarded, deposited
                    // locally or as kinetic energy (plus the pair rest masses)
                    let total = r.edep_photon_discards + r.edep_local + el.kinetic + 2.0 * RM * el.positrons as f64;
                    assert!((total - ei).abs() < 1e-9, "total {total} vs {ei}");
                } else {
                    // charged start: the stand-in ELECTR deposits it and makes no photons
                    assert!((el.kinetic - (ei - RM)).abs() < 1e-12);
                    assert_eq!((r.edep_photon_discards, r.edep_local), (0.0, 0.0));
                }
            }
        }
    }

    #[test]
    fn shower_pi_zero_makes_two_gammas_summing_to_the_pion_energy() {
        let mut rng = Ranmar::new(41, 42);
        let mut tables = photon_tables(0.0, 0.5, 0);
        tables.regions[1].pcut = 1.0e9; // both gammas are discarded at once
        let cfg = PhotonConfig { pair: PairConfig::default(), rr: None, prm: RM, vacdst: 100.0 };
        let mut geo = Slab { length: 3.0 };
        for _ in 0..20 {
            let mut st = Stack::new(20);
            let mut el = DepositElectrons { kinetic: 0.0, positrons: 0 };
            let start = ShowerStart { iqi: 2, ei: 200.0, xi: 0.0, yi: 0.0, zi: 1e-4, ui: 0.0, vi: 0.0, wi: 1.0, iri: 1, wti: 1.0 };
            let r = shower(&mut st, &tables, &cfg, &mut geo, &mut el, &start, &mut rng).unwrap();
            assert!((r.edep_photon_discards - 200.0).abs() < 1e-9, "{}", r.edep_photon_discards);
        }
        // below the pi-zero rest energy
        let mut st = Stack::new(20);
        let mut el = DepositElectrons { kinetic: 0.0, positrons: 0 };
        let start = ShowerStart { iqi: 2, ei: 100.0, xi: 0.0, yi: 0.0, zi: 0.0, ui: 0.0, vi: 0.0, wi: 1.0, iri: 1, wti: 1.0 };
        assert!(matches!(
            shower(&mut st, &tables, &cfg, &mut geo, &mut el, &start, &mut rng),
            Err(EgsError::Fatal(_))
        ));
    }


    /// A synthetic MS data file: for every (i, j), `u` rising 0..1, `f` = 1 + k/50,
    /// `w` = 1 (no alias redirection), `i` = k + 1 (the identity after the fix-up).
    fn synthetic_ms_text() -> String {
        let mut t = String::new();
        for _ in 0..=MAXL_MS {
            for _ in 0..=MAXQ_MS {
                let u: Vec<String> = (0..=MAXU_MS).map(|k| alloc::format!("{:.6}", k as f64 / MAXU_MS as f64)).collect();
                let f: Vec<String> = (0..=MAXU_MS).map(|k| alloc::format!("{:.6}", 1.0 + k as f64 / 50.0)).collect();
                let w: Vec<String> = (0..MAXU_MS).map(|_| String::from("1.0")).collect();
                let ii: Vec<String> = (0..MAXU_MS).map(|k| alloc::format!("{}", k + 1)).collect();
                t.push_str(&u.join(" "));
                t.push('\n');
                t.push_str(&f.join(" "));
                t.push('\n');
                t.push_str(&w.join(" "));
                t.push('\n');
                t.push_str(&ii.join(" "));
                t.push('\n');
            }
        }
        t
    }

    fn flat_spin(rej: f32) -> SpinData {
        SpinData {
            spin_rej: alloc::vec![rej; 2 * (MAXE_SPI1 + 1) * (MAXQ_SPIN + 1) * (MAXU_SPIN + 1)],
            espml: -5.0,
            b2spin_min: 0.5,
            dbeta2i: 10.0,
            dleneri: 2.0,
            dqq1i: 20.0,
        }
    }

    #[test]
    fn ms_reader_parses_fixes_up_and_rejects_bad_input() {
        let ms = MsData::from_text(&synthetic_ms_text()).unwrap();
        // fms(k) = fms(k+1)/fms(k) - 1, ims(k) = ims(k) - 1
        let f0 = 1.0f64;
        let f1 = 1.0 + 1.0 / 50.0;
        assert!((ms.fms[ms_idx(0, 0, 0)] as f64 - (f1 / f0 - 1.0)).abs() < 1e-6);
        assert_eq!(ms.ims[ms_idx(3, 2, 5)], 5);
        assert_eq!(ms.fms[ms_idx(0, 0, MAXU_MS)], ms.fms[ms_idx(0, 0, MAXU_MS - 1)]);
        assert!((ms.dllambi as f64 - 63.0 / 1e5f64.ln()).abs() < 1e-5);
        assert!((ms.dqmsi as f64 - 14.0).abs() < 1e-6);
        assert!(MsData::from_text("1 2 3\n").is_err());
        assert!(MsData::from_text(&synthetic_ms_text().replacen("0.000000", "x", 1)).is_err());
        // list-directed leniency: D exponents and commas
        let t = synthetic_ms_text().replacen("1.0 1.0", "1.0D0, 1.0", 1);
        assert!(MsData::from_text(&t).is_ok());
    }

    #[test]
    fn scattering_samplers_return_unit_cosines() {
        let ms = MsData::from_text(&synthetic_ms_text()).unwrap();
        let spin = flat_spin(1.0);
        let mut rng = Ranmar::new(61, 62);
        // single scattering, with and without spin rejection (always accepted here)
        for &se in &[false, true] {
            let mut ss = SpinState::default();
            for _ in 0..50 {
                let (c, s) = sscat(&spin, 1e-6, 0.0, 0.9, 0, 1, se, &mut ss, &mut rng);
                assert!((c * c + s * s - 1.0).abs() < 1e-9);
            }
        }
        // lambda regimes: <1 (Poisson series), 1..13.8 (single/multi), large (table)
        for &lambda in &[0.3, 2.0, 12.0, 50.0, 1000.0] {
            for &se in &[false, true] {
                let mut st = MscatState::default();
                let mut ss = SpinState::default();
                for _ in 0..50 {
                    let (mut find, mut sidx) = (true, true);
                    let (c, s) =
                        mscat(&ms, &spin, lambda, 1e-5, 0.1, 0.0, 0.9, 0, 1, se, &mut find, &mut sidx, &mut st, &mut ss, &mut rng)
                            .unwrap();
                    assert!((c * c + s * s - 1.0).abs() < 1e-6, "lambda {lambda}: {c} {s}");
                    assert!((-1.0..=1.0).contains(&c));
                }
            }
        }
        // a re-iteration with find_index = false reuses the stored bins
        let mut st = MscatState::default();
        let mut ss = SpinState::default();
        let (mut find, mut sidx) = (true, true);
        mscat(&ms, &spin, 100.0, 1e-5, 0.01, 0.0, 0.9, 0, 1, false, &mut find, &mut sidx, &mut st, &mut ss, &mut rng).unwrap();
        assert!(!find);
        let keep = st;
        mscat(&ms, &spin, 100.0, 1e-5, 0.01, 0.0, 0.9, 0, 1, false, &mut find, &mut sidx, &mut st, &mut ss, &mut rng).unwrap();
        assert_eq!(st, keep);
        // q1 bins: below QMIN, above QMAX
        for &q1 in &[1e-4, 0.7] {
            let (mut f, mut sx) = (true, true);
            let mut st = MscatState::default();
            mscat(&ms, &spin, 100.0, 1e-5, q1, 0.0, 0.9, 0, 1, false, &mut f, &mut sx, &mut st, &mut ss, &mut rng).unwrap();
        }
        // beyond LAMBMAX_MS is fatal
        let (mut f, mut sx) = (true, true);
        let mut st = MscatState::default();
        assert!(matches!(
            mscat(&ms, &spin, 2e5, 1e-5, 0.1, 0.0, 0.9, 0, 1, false, &mut f, &mut sx, &mut st, &mut ss, &mut rng),
            Err(EgsError::Fatal(_))
        ));
    }

    #[test]
    fn spin_rejection_bins_and_interpolates() {
        let mut spin = flat_spin(0.5);
        let mut rng = Ranmar::new(1, 2);
        let mut st = SpinState::default();
        // all-0.5 table: the rejection function is 0.5 everywhere, in every branch
        for &(beta2, elke, is_single) in &[(0.9, 0.0, false), (0.3, 1.0, false), (0.3, -10.0, true), (0.99, 0.0, true)] {
            let mut idx = true;
            let r = spin_rejection(&spin, 0, 1, elke, beta2, 0.2, 0.3, &mut idx, is_single, &mut st, &mut rng);
            assert!((r - 0.5).abs() < 1e-6 && !idx);
            // without re-indexing, the stored bins are reused
            let keep = st;
            let r2 = spin_rejection(&spin, 1, 1, elke, beta2, 0.2, -0.5, &mut idx, is_single, &mut st, &mut rng);
            assert!((r2 - 0.5).abs() < 1e-6 && st == keep);
        }
        // large q1 saturates the q bin
        let mut idx = true;
        spin_rejection(&spin, 0, 1, 0.0, 0.9, 50.0, 0.0, &mut idx, false, &mut st, &mut rng);
        assert_eq!(st.j, MAXQ_SPIN);
        // linear interpolation in k
        let n = MAXU_SPIN + 1;
        for k in 0..n {
            spin.spin_rej[spin_idx(1, 0, st.i, st.j, k)] = k as f32;
        }
        let mut idx = false;
        let cost = 1.0 - 2.0 * (0.5 / MAXU_SPIN as f64).powi(2); // xi*MAXU = 0.5
        let r = spin_rejection(&spin, 0, 1, 0.0, 0.9, 50.0, cost, &mut idx, false, &mut st, &mut rng);
        assert!((r - 0.5).abs() < 1e-6, "{r}");
    }


    fn synthetic_photo_text() -> String {
        let mut t = String::new();
        for i in 1..=100 {
            // j then K, L1, L2, L3, M, N binding energies in eV
            t.push_str(&alloc::format!("{i} {}.0 {}.0 {}.0 {}.0 {}.0 {}.0\n", i * 100, i * 10, i * 9, i * 8, i * 3, i));
        }
        t.push_str("header\n");
        for i in 1..=100 {
            t.push_str(&alloc::format!("{i} 0.8 0.5 0.5 0.5 0.5\n"));
        }
        for (first, last) in [(1usize, 19usize), (20, 26), (27, 32), (33, 37), (38, 38)] {
            t.push_str("header\n");
            for i in 1..=100 {
                let vals: Vec<String> = (first..=last).map(|k| alloc::format!("{}.{}", k, i)).collect();
                t.push_str(&alloc::format!("{i} {}\n", vals.join(" ")));
            }
        }
        t
    }

    fn synthetic_phocs_text() -> String {
        let mut t = String::new();
        for i in 1..=100 {
            let n = 2 + i % 3;
            t.push_str(&alloc::format!("{i} {n}\n"));
            for j in 1..=n {
                t.push_str(&alloc::format!("{}.0 {}.5 {}.25 {}.75 0.0{}\n", j, j, j, j, i % 10 + j));
            }
        }
        t
    }

    #[test]
    fn edgset_reads_photo_and_relaxation_data() {
        let (rd, pd) = edgset_from_text(&synthetic_photo_text(), &synthetic_phocs_text()).unwrap();
        // binding energies converted from eV to MeV
        assert!((rd.binding_energies[6][1] - 600.0e-6).abs() < 1e-15);
        assert!((rd.binding_energies[6][6] - 6.0e-6).abs() < 1e-15);
        assert_eq!(pd.interaction_prob[6][1], 0.8);
        assert_eq!(pd.interaction_prob[6][MXSHELL], 1.01, "the last shell always interacts");
        // the five relaxation blocks land at their k ranges
        assert_eq!(rd.relaxation_prob[7][1], 1.7);
        assert_eq!(rd.relaxation_prob[7][19], 19.7);
        assert_eq!(rd.relaxation_prob[7][20], 20.7);
        assert_eq!(rd.relaxation_prob[7][26], 26.7);
        assert_eq!(rd.relaxation_prob[7][27], 27.7);
        assert_eq!(rd.relaxation_prob[7][33], 33.7);
        assert_eq!(rd.relaxation_prob[7][37], 37.7);
        assert_eq!(rd.relaxation_prob[7][38], 38.7);
        assert_eq!(rd.relaxation_prob[7][39], 0.0, "transition 39 is never read");
        // edge fits
        assert_eq!(pd.edge_number[3], 2);
        assert_eq!(pd.edge_a[3][2], 2.0);
        assert_eq!(pd.edge_b[3][2], 2.5);
        assert_eq!(pd.edge_c[3][1], 1.25);
        assert_eq!(pd.edge_d[3][1], 1.75);
        // short or malformed input is an error
        assert!(edgset_from_text("1 2 3", &synthetic_phocs_text()).is_err());
        assert!(edgset_from_text(&synthetic_photo_text(), "1 3\n").is_err());
        assert!(edgset_from_text(&synthetic_photo_text(), "1 99\n").is_err(), "more edges than MXEDGE");
        assert!(edgset_from_text(&synthetic_photo_text().replacen("0.8", "q", 1), &synthetic_phocs_text()).is_err());
    }


    fn toy_ms_medium() -> MsMedium {
        let one = Pwlf { a0: alloc::vec![0.0, 1.0, 1.0, 1.0], a1: alloc::vec![0.0, 0.0, 0.0, 0.0] };
        MsMedium {
            blcc: 1.0e4,
            xcc: 1.0e-4,
            eke0: 1.0,
            eke1: 0.0,
            etae_ms: one.clone(),
            etap_ms: one.clone(),
            q1ce_ms: one.clone(),
            q1cp_ms: one.clone(),
            q2ce_ms: one.clone(),
            q2cp_ms: one.clone(),
            blcce: one,
        }
    }

    #[test]
    fn msdist_preserves_norms_and_distances() {
        let ms = MsData::from_text(&synthetic_ms_text()).unwrap();
        let spin = flat_spin(1.0);
        let med = toy_ms_medium();
        let mut rng = Ranmar::new(83, 84);
        let mut count = 0.0;
        let dirs = [(0.0, 0.0, 1.0), (0.3, -0.4, (1.0f64 - 0.25).sqrt()), (0.0, 0.0, -1.0)];
        for &(u0, v0, w0) in &dirs {
            for &spin_effects in &[false, true] {
                for &qel in &[0usize, 1] {
                    for _ in 0..30 {
                        let inp = MsDistInput {
                            e0: 1.0,
                            eloss: 0.002,
                            tustep: 0.01,
                            rhof: 1.0,
                            qel,
                            spin_effects,
                            u0,
                            v0,
                            w0,
                            x0: 1.0,
                            y0: 2.0,
                            z0: 3.0,
                        };
                        for which in 0..2 {
                            let o = if which == 0 {
                                msdist_p1(&med, &ms, &spin, &inp, 1, &mut rng).unwrap()
                            } else {
                                msdist_p2(&med, &ms, &spin, &inp, 1, &mut count, &mut rng).unwrap()
                            };
                            let n = o.us * o.us + o.vs * o.vs + o.ws * o.ws;
                            assert!((n - 1.0).abs() < 1e-6, "direction norm {n} (pII = {which})");
                            // the straight-line distance is the displacement
                            let d = ((o.xf - 1.0).powi(2) + (o.yf - 2.0).powi(2) + (o.zf - 3.0).powi(2)).sqrt();
                            assert!((d - o.ustep).abs() < 1e-9 * inp.tustep.max(d), "d {d} vs ustep {}", o.ustep);
                            assert!(o.ustep <= inp.tustep * (1.0 + 1e-9) && o.ustep > 0.0);
                        }
                    }
                }
            }
        }
        assert_eq!(count, 3.0 * 2.0 * 2.0 * 30.0);
    }

    #[test]
    fn msdist_spin_interval_clamp_and_pwlf() {
        let p = Pwlf { a0: alloc::vec![0.0, 2.0, 5.0], a1: alloc::vec![0.0, 3.0, 4.0] };
        assert_eq!(p.eval(2, 0.5), 7.0);
        let mut med = toy_ms_medium();
        med.eke1 = 1.0;
        med.eke0 = 0.0;
        // elke = -5 gives an interval below 1: clamped to 1 and elke reset
        assert_eq!(spin_interval(&med, -5.0), (1, 1.0));
        assert_eq!(spin_interval(&med, 2.5), (2, 2.5));
    }


    impl ElectronGeometry for Slab {
        fn hownear(&mut self, stack: &Stack) -> f64 {
            let z = stack.z[stack.np];
            z.min(self.length - z).max(0.0)
        }
    }

    const N_EKE: usize = 14;

    fn const_pwlf(v: f64) -> Pwlf {
        Pwlf { a0: alloc::vec![v; N_EKE + 2], a1: alloc::vec![0.0; N_EKE + 2] }
    }

    /// One medium, 14 energy bins: `lelke = 2 ln(E) + 11`, constant stopping
    /// power 2 MeV/cm so that range = (E - E_1)/2, constant cross sections.
    fn toy_electron_tables() -> ElectrTables {
        let (eke0, eke1) = (11.0, 2.0);
        let e_array: Vec<f64> =
            core::iter::once(0.0).chain((1..=N_EKE).map(|i| ((i as f64 - eke0) / eke1).exp())).collect();
        let range_ep: Vec<f64> = e_array.iter().map(|&e| (e - e_array[1]) / 2.0).collect();
        let mut ms_med = toy_ms_medium();
        ms_med.eke0 = eke0;
        ms_med.eke1 = eke1;
        let ones = const_pwlf(1.0);
        ms_med.etae_ms = ones.clone();
        ms_med.etap_ms = ones.clone();
        ms_med.q1ce_ms = ones.clone();
        ms_med.q1cp_ms = ones.clone();
        ms_med.q2ce_ms = ones.clone();
        ms_med.q2cp_ms = ones.clone();
        ms_med.blcce = ones;
        let te = 0.01;
        let med = ElectronMediumData {
            rho: 1.0,
            eke0,
            eke1,
            e_array,
            range_ep: [range_ep.clone(), range_ep],
            ededx: const_pwlf(2.0),
            pdedx: const_pwlf(2.0),
            esig: const_pwlf(5.0),
            psig: const_pwlf(5.0),
            ebr1: const_pwlf(0.3),
            pbr1: const_pwlf(0.3),
            pbr2: const_pwlf(0.6),
            tmxs: const_pwlf(0.1),
            esig_e: 2.5,
            psig_e: 2.5,
            sig_ismonotone: [true, true],
            te,
            thmoll: 2.0 * te + RM,
            ae: te + RM,
            ms: ms_med,
            brems: toy_brems_medium(),
        };
        let slab = ElectrRegion { med: 1, rhor: 1.0, ecut: te + RM, smaxir: 1.0e10 };
        let outside = ElectrRegion { med: 0, ..slab };
        ElectrTables {
            regions: alloc::vec![ElectrRegion::default(), slab, outside],
            media: alloc::vec![ElectronMediumData::default(), med],
            ms: Some(MsData::from_text(&synthetic_ms_text()).unwrap()),
            spin: flat_spin(1.0),
        }
    }

    /// `ELECTR` behind the `ElectronTransport` seam of `SHOWER`.
    struct ElectrDriver {
        tables: ElectrTables,
        cfg: ElectrConfig,
        geo: Slab,
        counters: ElectrCounters,
        tally: f64,
    }

    impl ElectronTransport for ElectrDriver {
        fn electr<R: UniformSource>(&mut self, stack: &mut Stack, rng: &mut R) -> Result<i32, EgsError> {
            let r = electr(stack, &self.tables, &self.cfg, &mut self.geo, &mut self.counters, rng)?;
            self.tally += r.edep_steps + r.edep;
            Ok(r.ircode)
        }
    }

    fn electr_cfg(presta_ii: bool, exact_bca: bool, spin_effects: bool) -> ElectrConfig {
        ElectrConfig {
            spin_effects,
            exact_bca,
            skindepth_for_bca: 3.0,
            presta_ii,
            prm: RM,
            vacdst: 1.0e8,
            brems: BremsConfig::default(),
        }
    }

    #[test]
    fn electron_showers_conserve_energy() {
        let mut rng = Ranmar::new(91, 92);
        let ptables = photon_tables(0.0, 0.5, 0);
        let pcfg = PhotonConfig { pair: PairConfig::default(), rr: None, prm: RM, vacdst: 100.0 };
        let mut pgeo = Slab { length: 3.0 };
        let configs = [
            electr_cfg(true, true, false),
            electr_cfg(false, false, false),
            electr_cfg(true, true, true),
            electr_cfg(true, false, false),
        ];
        for cfg in configs {
            for &(iqi, zi, wi) in &[(-1, 1.0, 1.0), (-1, 2.9999, 1.0), (1, 1.5, 1.0), (-1, 1.5, -1.0)] {
                for _ in 0..8 {
                    let mut drv = ElectrDriver {
                        tables: toy_electron_tables(),
                        cfg,
                        geo: Slab { length: 3.0 },
                        counters: ElectrCounters::default(),
                        tally: 0.0,
                    };
                    let ei = 0.5 + RM + 0.3;
                    let mut st = Stack::new(500);
                    let start = ShowerStart { iqi, ei, xi: 0.0, yi: 0.0, zi, ui: 0.0, vi: 0.0, wi, iri: 1, wti: 1.0 };
                    let r = shower(&mut st, &ptables, &pcfg, &mut pgeo, &mut drv, &start, &mut rng).unwrap();
                    assert_eq!(st.np, 0);
                    let total = r.edep_photon_discards + r.edep_local + drv.tally;
                    let expect = (ei - RM) + if iqi == 1 { 2.0 * RM } else { 0.0 };
                    if iqi == 1 && zi > 2.0 {
                        continue;
                    }
                    assert!((total - expect).abs() < 1e-6, "cfg {cfg:?} iqi {iqi} z {zi}: {total} vs {expect}");
                }
            }
        }
    }

    #[test]
    fn electr_vacuum_step_and_error_paths() {
        let mut rng = Ranmar::new(5, 6);
        let tables = toy_electron_tables();
        let cfg = electr_cfg(true, true, false);
        let mut geo = Slab { length: 3.0 };
        let mut counters = ElectrCounters::default();
        // an electron below the cut-off is discarded at once with its kinetic energy
        let mut st = Stack::new(10);
        st.np = 1;
        st.iq[1] = -1;
        st.e[1] = RM + 0.005;
        st.ir[1] = 1;
        st.wt[1] = 1.0;
        let r = electr(&mut st, &tables, &cfg, &mut geo, &mut counters, &mut rng).unwrap();
        assert_eq!((r.ircode, st.np), (2, 0));
        assert!((r.edep - 0.005).abs() < 1e-12);
        // zero weight: user discard
        let mut st = Stack::new(10);
        st.np = 1;
        st.iq[1] = -1;
        st.e[1] = RM + 1.0;
        st.ir[1] = 1;
        let r = electr(&mut st, &tables, &cfg, &mut geo, &mut counters, &mut rng).unwrap();
        assert_eq!((r.ircode, st.np), (2, 0));
        assert!((r.edep - 1.0).abs() < 1e-12);
        // an electron in the vacuum region is discarded by the geometry
        let mut st = Stack::new(10);
        st.np = 1;
        st.iq[1] = -1;
        st.e[1] = RM + 1.0;
        st.ir[1] = 2;
        st.wt[1] = 1.0;
        st.w[1] = 1.0;
        let r = electr(&mut st, &tables, &cfg, &mut geo, &mut counters, &mut rng).unwrap();
        assert_eq!(r.ircode, 2);
        // a missing MS table is refused
        let mut none = tables.clone();
        none.ms = None;
        let mut st = Stack::new(10);
        st.np = 1;
        st.iq[1] = -1;
        st.e[1] = RM + 1.0;
        st.ir[1] = 1;
        st.wt[1] = 1.0;
        assert!(matches!(
            electr(&mut st, &none, &cfg, &mut geo, &mut counters, &mut rng),
            Err(EgsError::Unsupported(_))
        ));
        assert!(counters.count_all_steps >= 0.0);
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
