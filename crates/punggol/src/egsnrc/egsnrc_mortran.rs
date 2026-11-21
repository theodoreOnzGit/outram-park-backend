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
//! | `SUBROUTINE MOLLER` (`egsnrc.mortran:2598-2745`); its EII selection (`2633-2661`) is [`moller_eii_select`] | [`moller`], [`moller_eii_select`] |
//! | `SUBROUTINE BREMS` (`egsnrc.mortran:312-525`), Bethe-Heitler path | [`brems`] |
//! | `SUBROUTINE PHOTON` (`egsnrc.mortran:6438-6640`), without Rayleigh or photonuclear | [`photon`] |
//! | `SUBROUTINE SHOWER` (`egsnrc.mortran:6645-6718`) | [`shower`] |
//! | `SUBROUTINE mscat`, `spin_rejection`, `sscat` (`egsnrc.mortran:2983-3263`) and the table reader of `init_ms_SR` (`:3266-3306`) | [`mscat`], [`spin_rejection`], [`sscat`], [`MsData::from_text`] |
//! | `SUBROUTINE EDGSET` (`egsnrc.mortran:6311-6430`), the data readers | [`edgset_from_text`] |
//! | `SUBROUTINE msdist_pI` (`egsnrc.mortran:4309-4455`), `msdist_pII` (`:4049-4290`) | [`msdist_p1`], [`msdist_p2`] |
//! | `SUBROUTINE ELECTR` (`egsnrc.mortran:985-1825`) with the step, range and energy-loss macros of `egsnrc.macros:1308-1360, 2358-2395, 3105-3330` | [`electr`] |
//! | `SUBROUTINE HATCH`, the regular PEGS4 intake block and the `DUNIT` scaling (`egsnrc.mortran:2036-2250`) | [`Pegs4Medium`], [`parse_pegs4_media`] |
//! | `SUBROUTINE fix_brems`, `FCOULC`, `XSIF` (`egsnrc.mortran:2325-2420`) | [`fix_brems`] |
//! | `SUBROUTINE init_compton` (`egsnrc.mortran:2422-2575`) | [`init_compton`] |
//! | `SUBROUTINE mscati` (`egsnrc.mortran:2749-2975`), without `init_spin` | [`mscati`], [`MscatiSettings::resolve`] |
//! | the table assembly of `HATCH` (`egsnrc.mortran:2036-2325`, `egsnrc.macros:1981-2036, 3681-3692`) | [`hatch_media`], [`adjust_rhor_ecut_pcut`] |
//! | `SUBROUTINE init_spin` (`egsnrc.mortran:3309-3706`), from decoded records | [`init_spin`] |
//! | `SUBROUTINE PHOTONUC` (`egsnrc.mortran:9001-9015`), a placeholder upstream | [`photonuc`] |
//! | `$PHOTONUC-CORRECTION`, `$PHOTONUCLEAR` (`egsnrc.macros:1615-1629`) | inside [`photon`] |
//! | `egs_init_rayleigh_sampling`, `prepare_rayleigh_data`, `egs_rayleigh_sampling`, `egs_rayleigh_sigma`, `ZERO` (`egsnrc.mortran:8283-8700`, `egs_utilities.mortran:2152-2186`) | [`init_rayleigh_sampling`], [`prepare_rayleigh_data`], [`rayleigh_sampling`], [`rayleigh_sigma`], [`zero_value`] |
//! | `$RAYLEIGH-CORRECTION`, `$RAYLEIGH-SCATTERING` (`egsnrc.macros:1579, 1602`) | inside [`photon`] |
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
use super::egs_utilities_mortran::{alias_sample1, gauss_legendre, prepare_alias_table, set_spline, spline};
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
    /// `ne_array`: number of electrons in each shell.
    pub ne_array: Vec<i32>,
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
    /// The EADL relaxation tables of `egs_init_relax` (`eadl_relax = true`,
    /// the stock EGSnrc default). `None` keeps the built-in scheme above,
    /// bit for bit. With `Some`, [`relax`] dispatches to [`eadl_relax`] first
    /// thing, as `RELAX` does (`egsnrc.mortran:5644`), and [`photo`] does not
    /// let a vacancy start below L3 (`egsnrc.mortran:5156`).
    pub eadl: Option<EadlData>,
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

/// `RELAX` (`egsnrc.mortran:5556-5790`), the built-in scheme, or the EADL
/// scheme ([`eadl_relax`]) when `data.eadl` is `Some` (upstream's
/// `eadl_relax` switch; `energy` is then not used, the binding energies are
/// EADL's): fill a vacancy
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
    if let Some(eadl) = &data.eadl {
        // `IF (eadl_relax) [ call egs_eadl_relax(iZ,n); return; ]`, egsnrc.mortran:5644
        return eadl_relax(stack, eadl, iz, n, ecut, pcut, rng);
    }
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

/// `$MXESHLL` (`egsnrc.macros:677`): maximum shells of one element.
pub const MXESHLL: usize = 30;
/// `$MAXSHELL` (`egsnrc.macros:678`): maximum shells over all media elements.
pub const MAXSHELL_EADL: usize = 3000;
/// `$MAXRELAX` (`egsnrc.macros:679`): maximum relaxation channels.
pub const MAXRELAX_EADL: usize = 10000;
/// `$MAXTRANS` (`egsnrc.macros:681`): maximum transitions per shell.
pub const MAXTRANS_EADL: usize = 300;

/// One shell of one element as stored in `relax.data`.
#[derive(Debug, Clone, PartialEq)]
pub struct EadlShellRecord {
    /// EADL shell type (`shell_type`).
    pub shell_type: i32,
    /// Binding energy, MeV (`real*4` in the file).
    pub binding_energy: f32,
    /// `(final state, probability)` per transition, exactly as stored (C-style
    /// 0-based states; `egs_init_relax` adds 1 or 65).
    pub transitions: Vec<(i32, f32)>,
}

/// `$HEN_HOUSE/data/relax.data` (`egs_init_relax`, `egsnrc.mortran:5793-6008`):
/// a Fortran direct-access file of **4-byte records**
/// (`recl = $RECL-FACTOR`, with the 4-byte-integer re-creation the upstream
/// header describes; the 1-byte `relax_onebyte.data` is the C++ code's file
/// and is not read by the Fortran). Record 1 holds the number of elements
/// (100 in the shipped file), record `iZ + 1` the record number where the
/// data of element `iZ` start, and there: the number of shells, then per
/// shell `shell_type`, `ntran`, binding energy (`real*4`), then `ntran` pairs
/// of final state and probability (`real*4`). Upstream does no byte swapping,
/// so the byte order is the machine's; the decoder takes little-endian and
/// falls back to big-endian when record 1 is not a plausible element count.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EadlFile {
    words: Vec<u32>,
}

impl EadlFile {
    /// Decode the bytes of `relax.data`.
    ///
    /// # Errors
    ///
    /// A message when the length is not a multiple of 4 or record 1 is not an
    /// element count in either byte order.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 4 || bytes.len() % 4 != 0 {
            return Err(alloc::format!("relax.data: {} bytes is not a multiple of 4", bytes.len()));
        }
        let le = |c: &[u8]| u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        let be = |c: &[u8]| u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
        let plausible = |n: u32| (1..=1000).contains(&n);
        let swap = if plausible(le(&bytes[0..4])) {
            false
        } else if plausible(be(&bytes[0..4])) {
            true
        } else {
            return Err(String::from("relax.data: record 1 is not an element count in either byte order"));
        };
        let words = bytes.chunks_exact(4).map(|c| if swap { be(c) } else { le(c) }).collect();
        Ok(Self { words })
    }

    fn int(&self, rec: usize) -> Result<i32, String> {
        self.words.get(rec.wrapping_sub(1)).map(|w| *w as i32).ok_or_else(|| alloc::format!("relax.data: record {rec} is outside the file"))
    }

    fn real(&self, rec: usize) -> Result<f32, String> {
        self.words.get(rec.wrapping_sub(1)).map(|w| f32::from_bits(*w)).ok_or_else(|| alloc::format!("relax.data: record {rec} is outside the file"))
    }

    /// The number of elements in the file (record 1).
    ///
    /// # Errors
    ///
    /// A message when the file is empty.
    pub fn nz(&self) -> Result<usize, String> {
        Ok(self.int(1)? as usize)
    }

    /// The shells of element `iz` (`egsnrc.mortran:5918-5950`).
    ///
    /// # Errors
    ///
    /// A message when `iz` is not in the file or a record is out of range.
    pub fn element(&self, iz: usize) -> Result<Vec<EadlShellRecord>, String> {
        if iz < 1 || iz > self.nz()? {
            return Err(alloc::format!("relax.data has no element {iz}"));
        }
        let mut rec = self.int(iz + 1)? as usize;
        let nshell = self.int(rec)?;
        if !(0..=1000).contains(&nshell) {
            return Err(alloc::format!("relax.data: element {iz} claims {nshell} shells"));
        }
        let mut out = Vec::new();
        for _ in 0..nshell {
            let shell_type = self.int(rec + 1)?;
            let ntran = self.int(rec + 2)?;
            let binding_energy = self.real(rec + 3)?;
            if !(0..=100_000).contains(&ntran) {
                return Err(alloc::format!("relax.data: element {iz} claims {ntran} transitions"));
            }
            rec += 3;
            let mut transitions = Vec::with_capacity(ntran as usize);
            for _ in 0..ntran {
                transitions.push((self.int(rec + 1)?, self.real(rec + 2)?));
                rec += 2;
            }
            out.push(EadlShellRecord { shell_type, binding_energy, transitions });
        }
        Ok(out)
    }
}

/// The EADL relaxation tables `egs_init_relax` fills (`COMMON/RELAX-INIT/`,
/// `egsnrc.macros:703-725`). Shells of all media elements are one list,
/// **1-based** (index 0 unused), in upstream's load order (media in order,
/// elements ascending in `Z`, a `Z` loaded once). The per-shell transition
/// tables are stored as one vector per shell (1-based) in place of upstream's
/// flat `relax_first`/`relax_prob`/`relax_atbin`/`relax_state`, so that
/// [`sample_alias_histogram`](super::egs_utilities_mortran::sample_alias_histogram)
/// is used as it is.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EadlData {
    /// `shell_be(ish)`, MeV.
    pub shell_be: Vec<f64>,
    /// `shell_type(ish)`.
    pub shell_type: Vec<i32>,
    /// `shell_num(ish)`: position within the element.
    pub shell_num: Vec<i32>,
    /// `shell_Z(ish)`.
    pub shell_z: Vec<i32>,
    /// `shell_eadl(iZ, k)`, `[iz][k]`: global index of shell `k` of element
    /// `iZ`, `-1` where unset.
    pub shell_eadl: Vec<Vec<i32>>,
    /// `relax_ntran(ish)`: `-1` below the relaxation cut-off.
    pub relax_ntran: Vec<i32>,
    /// `relax_state(.)` of each shell, 1-based; `-1` is the "no transition"
    /// remainder bin when the EADL probabilities sum to less than 1.
    pub relax_state: Vec<Vec<i32>>,
    /// `relax_prob(.)` of each shell after `prepare_alias_histogram`
    /// (alias weights), 1-based.
    pub relax_prob: Vec<Vec<f64>>,
    /// `relax_atbin(.)` of each shell (alias indices), 1-based.
    pub relax_atbin: Vec<Vec<i32>>,
}

/// `egs_init_relax` (`egsnrc.mortran:5793-6008`): load the EADL data of every
/// element of the media from `file` (`relax.data`).
///
/// `media_z[m]` lists the `ZELEM` of medium `m` (only the elements are used;
/// upstream loads each distinct `Z` once, medium by medium, elements in
/// ascending `Z`). `binding_energies[iz][k]` is `COMMON/EDGE/`'s array, in
/// MeV: a positive value replaces the file's binding energy of that shell,
/// and, for the `epdl` series (`photon_xsections = 'epdl'`), a zero one is
/// filled in from the file (`egsnrc.mortran:5934-5941`). (For `xcom`,
/// [`init_compton_eadl`] does that filling, and must run first.)
/// `RELAX-CUTOFF` is 1 keV.
///
/// Reading only the real `relax.data` of the pinned commit has been done
/// (see the V&V notes in `docs/egsnrc_phase2_differential.md`).
///
/// # Errors
///
/// A message for a malformed file, more than [`MAXSHELL_EADL`] shells,
/// [`MAXRELAX_EADL`] channels, [`MAXTRANS_EADL`] transitions in a shell,
/// a shell number above [`MXESHLL`], or a `binding_energies` too small.
pub fn init_relax_eadl(file: &EadlFile, media_z: &[Vec<f64>], binding_energies: &mut [Vec<f64>], epdl: bool) -> Result<EadlData, String> {
    let min_be = RELAX_CUTOFF;
    let mut d = EadlData {
        shell_be: alloc::vec![0.0],
        shell_type: alloc::vec![0],
        shell_num: alloc::vec![0],
        shell_z: alloc::vec![0],
        shell_eadl: alloc::vec![alloc::vec![-1; MXESHLL + 1]; 101],
        relax_ntran: alloc::vec![0],
        relax_state: alloc::vec![Vec::new()],
        relax_prob: alloc::vec![Vec::new()],
        relax_atbin: alloc::vec![Vec::new()],
    };
    let mut relax_ntot = 0usize;
    for zs in media_z {
        let mut sorted: Vec<i64> = zs.iter().map(|z| *z as i64).collect();
        sorted.sort();
        for iz in sorted {
            if d.shell_z.iter().skip(1).any(|z| *z as i64 == iz) {
                continue; // already loaded for this atomic number
            }
            if iz < 1 || iz > 100 {
                return Err(alloc::format!("init_relax_eadl: atomic number {iz} is not between 1 and 100"));
            }
            let izu = iz as usize;
            let shells = file.element(izu)?;
            let shell_ntot = d.shell_be.len() - 1;
            if shell_ntot + shells.len() > MAXSHELL_EADL {
                return Err(alloc::format!("Too many shells to fit in the list: {} (at least)", shell_ntot + shells.len()));
            }
            for (i, sh) in shells.iter().enumerate() {
                let num = i + 1;
                let ish = shell_ntot + num;
                if num > MXESHLL {
                    return Err(alloc::format!("Z = {iz} has more than {MXESHLL} shells"));
                }
                let mut shell_be = sh.binding_energy as f64;
                let slot = binding_energies.get_mut(izu).and_then(|r| r.get_mut(num)).ok_or_else(|| String::from("binding_energies is too small for the EADL shells"))?;
                if *slot > 0.0 {
                    shell_be = *slot;
                } else if epdl {
                    *slot = shell_be;
                }
                d.shell_be.push(shell_be);
                d.shell_type.push(sh.shell_type);
                d.shell_num.push(num as i32);
                d.shell_z.push(iz as i32);
                d.shell_eadl[izu][num] = ish as i32;
                let mut itmp: Vec<i32> = alloc::vec![0];
                let mut wtmp: Vec<f64> = alloc::vec![0.0];
                for &(state, prob) in &sh.transitions {
                    // C-style indexing: +1 for radiative, +65 for non-radiative states
                    itmp.push(if state < 64 { state + 1 } else { state + 65 });
                    wtmp.push(prob as f64);
                }
                let mut ntran = sh.transitions.len();
                if ntran > MAXTRANS_EADL {
                    return Err(alloc::format!("Z = {iz} shell {num} has {ntran} transitions, more than {MAXTRANS_EADL}"));
                }
                if shell_be < min_be {
                    d.relax_ntran.push(-1);
                    d.relax_state.push(Vec::new());
                    d.relax_prob.push(Vec::new());
                    d.relax_atbin.push(Vec::new());
                } else {
                    let mut sumw = 0.0;
                    for k in 1..=ntran {
                        sumw += wtmp[k];
                    }
                    if sumw > 1.0 {
                        for k in 1..=ntran {
                            wtmp[k] /= sumw;
                        }
                    } else if sumw < 1.0 {
                        ntran += 1;
                        itmp.push(-1);
                        wtmp.push(1.0 - sumw);
                    }
                    if relax_ntot + ntran > MAXRELAX_EADL {
                        return Err(alloc::format!("Too many relaxation transitions: {} (at least)", relax_ntot + ntran));
                    }
                    let atbin = super::egs_utilities_mortran::prepare_alias_histogram(&mut wtmp);
                    d.relax_ntran.push(ntran as i32);
                    d.relax_state.push(itmp);
                    d.relax_prob.push(wtmp);
                    d.relax_atbin.push(atbin);
                    relax_ntot += ntran;
                }
            }
        }
    }
    Ok(d)
}

/// `egs_eadl_relax(iZ, shell_egs)` (`egsnrc.mortran:6019-6208`): fill a
/// vacancy in shell `shell_egs` (position within the element) of element `iz`
/// with EADL fluorescence, Coster-Kronig and Auger transitions. Products above
/// the cut-offs go on the stack (properties copied from the slot that was on
/// top at entry, `np_save`; direction uniform in `cos` with the box-method
/// azimuth), the rest accumulates in the returned `edep`. `ecut` is the
/// region's `ECUT` (total energy), `pcut` its `PCUT`. The `RELAX-USER`/`rfu_*`
/// outputs and the `AUSGAB` calls are omitted.
///
/// Differences from [`relax`]'s built-in scheme that matter: the vacancy
/// energy is the table's binding energy (not an argument); a vacancy cascades
/// down all EADL shells, but a vacancy in a shell above number 4 (below L3) is
/// deposited locally without cascade (`:6107`, the MCDF option is off);
/// the below-cut-off test is `Evac < 1 keV`, strict.
///
/// **Upstream oddities, reproduced:**
/// - `new_state = sample_alias_histogram(..)` is followed by
///   `IF( new_state < 0 )` (`:6145`), but the sampler returns a bin number
///   `>= 1`, so that branch (meant for the remainder bin) never runs. The
///   remainder bin has `relax_state = -1`, which then passes
///   `new_state <= 64` as a *radiative* transition to the shell at global
///   index `vac - shell_num(vac) - 1` (the last shell of the previously loaded
///   element, or index 0 for the first one, where Fortran reads whatever
///   precedes the array; here `0.0` and a vacancy that ends the cascade).
/// - `edep = Evac` (`:6108`; the cascade's own deposits at `:6125` are `+=`): an assignment, which equals
///   the addition only when the caller zeroed `edep` first (`COMPT` does).
///
/// # Errors
///
/// [`EgsError::StackExceeded`] when the stack fills.
pub fn eadl_relax<R: UniformSource>(
    stack: &mut Stack,
    d: &EadlData,
    iz: usize,
    shell_egs: i32,
    ecut: f64,
    pcut: f64,
    rng: &mut R,
) -> Result<RelaxOutcome, EgsError> {
    let mut out = RelaxOutcome::default();
    let shell = if shell_egs < 0 { -1 } else { d.shell_eadl.get(iz).and_then(|r| r.get(shell_egs as usize)).copied().unwrap_or(-1) };
    if shell < 1 || shell as usize > MAXSHELL_EADL || shell as usize >= d.shell_be.len() {
        return Ok(out); // unknown vacancy
    }
    let ec = ecut - RM;
    let pc = pcut;
    let min_e = RELAX_CUTOFF;
    let be_of = |idx: i64| -> f64 { if idx >= 1 { d.shell_be.get(idx as usize).copied().unwrap_or(0.0) } else { 0.0 } };
    let shell = shell as usize;
    let mut evac = d.shell_be[shell];
    // vacancies below L3 deposit locally (mcdf_pe_xsections is off)
    if shell_egs > 4 {
        out.edep = evac;
        return Ok(out);
    }
    let mut vac = shell as i64;
    let mut vacs: Vec<i64> = Vec::new();
    let np_save = stack.np;
    loop {
        'vacancy: {
            let ntran = if vac >= 1 { d.relax_ntran.get(vac as usize).copied().unwrap_or(-1) } else { -1 };
            if evac < min_e || ntran < 1 {
                out.edep += evac;
                break 'vacancy;
            }
            let v = vac as usize;
            let bin = super::egs_utilities_mortran::sample_alias_histogram(&d.relax_prob[v], &d.relax_atbin[v], rng) as i32;
            let (ef, iqf, ecc);
            if bin < 0 {
                // never taken upstream either (see the doc comment)
                ef = 0.0;
                iqf = -1;
                ecc = ec;
            } else {
                let mut new_state = d.relax_state[v][bin as usize] as i64;
                let base = vac - d.shell_num[v] as i64;
                if new_state <= 64 {
                    iqf = 0;
                    new_state += base;
                    ef = be_of(new_state);
                    vacs.push(new_state);
                    ecc = pc;
                } else {
                    iqf = -1;
                    let mut new1 = new_state / 64;
                    let mut new2 = new_state - 64 * new1;
                    new1 += base;
                    new2 += base;
                    ef = be_of(new1) + be_of(new2);
                    vacs.push(new1);
                    vacs.push(new2);
                    ecc = ec;
                }
            }
            let ex = evac - ef;
            if ex > ecc {
                stack.np += 1;
                stack.check_stack(stack.np, "new_relax")?;
                let np = stack.np;
                stack.iq[np] = iqf;
                stack.transfer_properties(np, np_save);
                let rnno = rng.uniform();
                let cost = 2.0 * rnno - 1.0;
                let mut sint = 1.0 - cost * cost;
                if sint > 0.0 {
                    sint = Real::sqrt(sint);
                    let (cphi, sphi) = select_azimuthal_angle(rng);
                    stack.u[np] = sint * cphi;
                    stack.v[np] = sint * sphi;
                    stack.w[np] = cost;
                } else {
                    stack.u[np] = 0.0;
                    stack.v[np] = 0.0;
                    stack.w[np] = cost;
                }
                stack.e[np] = if iqf == 0 { ex } else { ex + RM };
            } else {
                out.edep += ex;
            }
        }
        match vacs.pop() {
            None => break,
            Some(next) => {
                vac = next;
                evac = be_of(next);
            }
        }
    }
    Ok(out)
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
    /// `pair_nrc` (default 0): NRC pair tables (`1` needs [`pair_with_nrc`]).
    pub pair_nrc: bool,
    /// `itriplet` (default 0): triplet production (`> 0` needs [`pair_with_triplet`]).
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
/// [`EgsError::Unsupported`]: the NRC pair tables (`pair_nrc`) need [`pair_with_nrc`] and triplet
/// production (`itriplet > 0`) needs [`pair_with_triplet`].
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
    pair_core(stack, med, cfg, rm, None, None, rng)
}

/// [`pair`] with triplet production (`itriplet > 0`): `triplet` is the
/// medium's [`TripletData`], the sampler's persistent [`TripletState`], the
/// medium index, and the largest photon energy any medium's tables reach
/// (`UP`, MeV; the `kmax` of `sample_triplet`'s binning). `gle` is taken as
/// `ln` of the photon energy, which is the value `PHOTON` leaves in `GLE`.
///
/// # Errors
///
/// As [`pair`].
pub fn pair_with_triplet<R: UniformSource>(
    stack: &mut Stack,
    med: &PairMedium,
    cfg: &PairConfig,
    rm: f64,
    triplet: (&TripletData, &mut TripletState, usize, f64),
    rng: &mut R,
) -> Result<bool, EgsError> {
    pair_core(stack, med, cfg, rm, Some(triplet), None, rng)
}

/// [`pair`] sampling the positron energy from the NRC pair tables
/// (`pair_nrc = 1`): `nrc` is the tables ([`NrcPairData`]) and the medium
/// index. Photons above the table range fall back to the usual sampling.
///
/// # Errors
///
/// As [`pair`].
pub fn pair_with_nrc<R: UniformSource>(
    stack: &mut Stack,
    med: &PairMedium,
    cfg: &PairConfig,
    rm: f64,
    nrc: (&NrcPairData, usize),
    rng: &mut R,
) -> Result<bool, EgsError> {
    pair_core(stack, med, cfg, rm, None, Some(nrc), rng)
}

fn pair_core<R: UniformSource>(
    stack: &mut Stack,
    med: &PairMedium,
    cfg: &PairConfig,
    rm: f64,
    triplet: Option<(&TripletData, &mut TripletState, usize, f64)>,
    nrc: Option<(&NrcPairData, usize)>,
    rng: &mut R,
) -> Result<bool, EgsError> {
    if cfg.pair_nrc && nrc.is_none() {
        return Err(EgsError::Unsupported("PAIR: NRC pair cross sections (pair_nrc = 1) without NRC tables"));
    }
    if cfg.itriplet > 0 && triplet.is_none() {
        return Err(EgsError::Unsupported("PAIR: triplet production (itriplet > 0) without triplet data"));
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
    if cfg.itriplet > 0 && eig > 4.0 * rm {
        if let Some((td, state, imed, up_max)) = triplet {
            let gle = RealMath::r_ln(eig);
            let itrip = (td.dli_triplet * gle + td.bli_triplet) as usize;
            let ftrip = if itrip < td.a_triplet[imed].len() {
                td.a_triplet[imed][itrip] * gle + td.b_triplet[imed][itrip]
            } else {
                0.0
            };
            let rnno34 = rng.uniform();
            if rnno34 < ftrip {
                sample_triplet(stack, state, up_max, rm, gle, rng)?;
                return Ok(false);
            }
        }
    }
    let rmt2 = 2.0 * rm;
    let (pese1, pese2, iq1, iq2);

    // the NRC pair cross section data base, within its energy range
    let mut nrc_pair: Option<(f64, f64, i32, i32)> = None;
    if cfg.pair_nrc {
        if let Some((nd, imed)) = nrc {
            let k = eig / rm;
            if k < nd.emax {
                let ibin = if k <= nd.emin {
                    1usize
                } else {
                    let mut abin = 1.0 + RealMath::r_ln((k - 2.0) / (nd.emin - 2.0)) * nd.dlei;
                    let mut ib = abin as usize;
                    abin -= ib as f64;
                    let rbin = rng.uniform();
                    if rbin < abin {
                        ib += 1;
                    }
                    ib
                };
                let t = &nd.tables[imed][ibin];
                let xx = alias_sample1(&nd.xdata, &t.fdata, &t.wdata, &t.idata, rng);
                // the energy fraction of the positron
                nrc_pair = Some(if xx > 0.5 {
                    let e1 = rm * (1.0 + xx * (k - 2.0));
                    (e1, peig - e1, 1, -1)
                } else {
                    let e2 = rm * (1.0 + xx * (k - 2.0));
                    (peig - e2, e2, -1, 1)
                });
            }
        }
    }

    if let Some((e1, e2, q1, q2)) = nrc_pair {
        pese1 = e1;
        pese2 = e2;
        iq1 = q1;
        iq2 = q2;
    } else if eig <= 2.1 {
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
/// With `relax_data.eadl` set (`eadl_relax`) a shell above L3 (`k > 4`) emits
/// the photoelectron without a vacancy, and K..L3 vacancies are relaxed by
/// [`eadl_relax`] through [`relax`]. Not ported: the shellwise MCDF branch
/// (`mcdf_pe_xsections`, `egs_shellwise_photo`), off by default. The below-1 keV warning is not printed. If the element or edge
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
            if relax_data.eadl.is_some() && k > 4 {
                // EADL: no initial vacancy below L3, just emit the electron
                // (`egsnrc.mortran:5156-5160`)
                stack.iq[np] = -1;
                stack.e[np] = peig + prm;
            } else {
                e_vac = relax_data.binding_energies[iz][k];
                k_shell = k;
                stack.e[np] = peig - e_vac + prm;
                do_relax = true;
                stack.iq[np] = -1;
            }
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
/// Refused with [`EgsError::Unsupported`] when `eii_active` (`eii_flag > 0`
/// and the medium has EII shells): call [`moller_eii_select`] first, which
/// samples the EII event or says to carry on with this routine.
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
    /// `LZBRANG(MEDIUM) = -log(zbrang)` (`egsnrc.macros:1991`; the sign is
    /// upstream's: an earlier doc line here said `+log`, which was wrong).
    pub lzbrang: f64,
    /// `AP(MEDIUM)`: photon production threshold, MeV.
    pub ap: f64,
    /// `log_ap(MEDIUM) = log(ap)`.
    pub log_ap: f64,
    /// The NIST / NRC cross-section alias tables (`COMMON/nist_brems/`), built
    /// by [`init_nist_brems`]; needed with `ibr_nist >= 1`.
    pub nist: Option<NistBremsMedium>,
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
    /// tables) need the medium's `nist` tables.
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
/// With `ibr_nist >= 1` the photon energy comes from the medium's alias
/// tables ([`init_nist_brems`]); without them it is refused with
/// [`EgsError::Unsupported`].
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
    if cfg.ibr_nist >= 1 && med.nist.is_none() {
        return Err(EgsError::Unsupported("BREMS: NIST/NRC bremsstrahlung tables (ibr_nist >= 1) without tables"));
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

    // the energy bin of the NIST / NRC tables
    let (mut jj, mut ajj) = (0usize, 0.0_f64);
    if let Some(nb) = med.nist.as_ref().filter(|_| cfg.ibr_nist >= 1) {
        let a = 1.0 + (waux + med.log_ap - nb.lemin) * nb.dlei;
        jj = a as usize;
        ajj = a - jj as f64;
        if jj > MXBRES {
            jj = MXBRES;
            ajj = -1.0;
        }
    }

    let mut pese = peie;
    for _ in 1..=cfg.nbr_split {
        let (esg, ese);
        if let Some(nb) = med.nist.as_ref().filter(|_| cfg.ibr_nist >= 1) {
            // the NIST or NRC bremsstrahlung cross section data base
            let br;
            if ekin > nb.emin {
                let r1 = rng.uniform();
                let j = if r1 < ajj { jj + 1 } else { jj };
                let bin = &nb.bins[j];
                br = alias_sample1(&bin.xdata, &bin.fdata, &bin.wdata, &bin.idata, rng);
            } else {
                br = rng.uniform();
            }
            esg = med.ap * RealMath::r_exp(br * waux);
            ese = peie - esg;
        } else {
        // Bethe-Heitler
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
    /// `iraylr(region)`: `1` turns on Rayleigh scattering (needs `cohe` and
    /// `rayleigh` data in the medium).
    pub irayl: i32,
    /// `iphotonucr(region)`: `1` turns on the photonuclear placeholder, which
    /// discards the photon.
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
    /// `cohe0(L)`, `cohe1(L)`: the coherent-scattering correction `COHFAC` that
    /// scales the photon mean free path (`cohe1(L) * GLE + cohe0(L)`); needed
    /// with `iraylr = 1`.
    pub cohe: Pwlf,
    /// The Rayleigh sampling data ([`init_rayleigh_sampling`]); needed with
    /// `iraylr = 1`.
    pub rayleigh: Option<RayleighData>,
    /// `photonuc0(L)`, `photonuc1(L)`: the factor `PHOTONUCFAC` that scales the
    /// photon mean free path (`photonuc1(L) * GLE + photonuc0(L)`), used when
    /// a region has `iphotonucr = 1`.
    pub photonuc: Pwlf,
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

/// `PHOTONUC` (`egsnrc.mortran:9001-9015`): a placeholder for photonuclear
/// modelling. The photon's energy and weight are set to zero: it is thrown
/// away. `photon` then finds the zero weight on re-entering its loop and
/// discards it with `edep = peig`, the photon's energy before it was zeroed
/// (upstream's `USER-PHOTON-DISCARD` takes `EDEP=PEIG`, so a scoring routine
/// is told the energy was deposited even though the comment says it is not).
pub fn photonuc(stack: &mut Stack) {
    stack.np_old = stack.np;
    let np = stack.np;
    stack.e[np] = 0.0;
    stack.wt[np] = 0.0;
}

/// `$MXRAYFF` and `$RAYCDFSIZE` (`egsnrc.macros:205-206`).
pub const MXRAYFF: usize = 100;
/// See [`MXRAYFF`].
pub const RAYCDFSIZE: usize = 100;

/// `ZERO()` (`egs_utilities.mortran:2152-2186`): the smallest positive value
/// the arithmetic can hold, found as upstream finds it (stepping down by
/// factors of 1e5, then 10, then 2..10 until the next step underflows).
pub fn zero_value() -> f64 {
    let mut x = 1.0e-20_f64;
    let mut xtemp = x;
    for _ in 0..100 {
        if x == 0.0 {
            break;
        }
        xtemp = x;
        x /= 1.0e5;
    }
    x = xtemp;
    for _ in 0..5 {
        if x != 0.0 {
            xtemp = x;
        } else {
            break;
        }
        x /= 10.0;
    }
    x = xtemp;
    for i in 2..=10 {
        if x != 0.0 {
            xtemp = x;
        } else {
            break;
        }
        x /= i as f64;
    }
    xtemp
}

/// The atomic form factors of `pgs4form.dat`: the momentum-transfer grid
/// and `aff(i, iZ)` for 100 elements, read as `read(ff_unit,*) xval, aff`.
#[derive(Debug, Clone, PartialEq)]
pub struct FormFactorTable {
    /// `xval(1..MXRAYFF)`, 1-based.
    pub xval: Vec<f64>,
    /// `aff(i, iZ)`: `aff[iz][i]`, both 1-based.
    pub aff: Vec<Vec<f64>>,
}

impl FormFactorTable {
    /// Parse the data set from an in-memory copy of `pgs4form.dat`
    /// (list-directed, `MXRAYFF + MXRAYFF * 100` values; `aff` is column-major
    /// in `i`).
    ///
    /// # Errors
    ///
    /// A message when the text is short or a value does not parse.
    pub fn from_text(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let n = MXRAYFF + MXRAYFF * 100;
        let v = read_record(&mut lines, n)?;
        let xval: Vec<f64> = core::iter::once(0.0).chain(v[..MXRAYFF].iter().copied()).collect();
        let mut aff = alloc::vec![alloc::vec![0.0; MXRAYFF + 1]];
        for iz in 0..100 {
            let base = MXRAYFF + iz * MXRAYFF;
            aff.push(core::iter::once(0.0).chain(v[base..base + MXRAYFF].iter().copied()).collect());
        }
        Ok(Self { xval, aff })
    }
}

/// The Rayleigh sampling data of one medium (`COMMON/rayleigh_sampling/`),
/// 1-based.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RayleighData {
    /// `xgrid(i, medium)`.
    pub xgrid: Vec<f64>,
    /// `fcum`.
    pub fcum: Vec<f64>,
    /// `b_array`.
    pub b_array: Vec<f64>,
    /// `c_array`.
    pub c_array: Vec<f64>,
    /// `i_array(1..RAYCDFSIZE)`.
    pub i_array: Vec<i32>,
    /// `pmax0`, `pmax1`: the maximum cumulative probability fit over energy.
    pub pmax: Pwlf,
}

/// `prepare_rayleigh_data` (`egsnrc.mortran:8607-8707`): the cumulative
/// distribution of `x f(x)^2` for a form factor given at `x[1..=ndat]`
/// (power-law between points), the maximum cumulative probability at each of
/// `ne` photon energies between `emin` and `emax` (MeV), and the uniform
/// cumulative bins the sampler uses. `f` is rescaled in place as upstream
/// does. Returns `pe_array` (1-based over `ne`) with the data in `out`.
pub fn prepare_rayleigh_data(
    x: &[f64],
    f: &mut [f64],
    ne: usize,
    emin: f64,
    emax: f64,
    prm: f64,
) -> (Vec<f64>, RayleighData) {
    let ndat = x.len() - 1;
    let ncbin = RAYCDFSIZE;
    let zero = zero_value();
    let mut fcum = alloc::vec![0.0_f64; ndat + 1];
    let mut b_array = alloc::vec![0.0_f64; ndat + 1];
    let mut c_array = alloc::vec![0.0_f64; ndat + 1];
    let mut i_array = alloc::vec![0_i32; ncbin + 2];
    let mut pe_array = alloc::vec![0.0_f64; ne + 1];
    for i in 1..=ndat {
        if f[i] == 0.0 {
            f[i] = zero;
        }
    }
    // the cumulative distribution
    let mut sum0 = 0.0_f64;
    fcum[1] = 0.0;
    for i in 1..ndat {
        let b = RealMath::r_ln(f[i + 1] / f[i]) / RealMath::r_ln(x[i + 1] / x[i]);
        b_array[i] = b;
        let (x1, x2) = (x[i], x[i + 1]);
        let pow_x1 = RealMath::r_powf(x1, 2.0 * b);
        let pow_x2 = RealMath::r_powf(x2, 2.0 * b);
        sum0 += f[i] * f[i] * (x2 * x2 * pow_x2 - x1 * x1 * pow_x1) / ((1.0 + b) * pow_x1);
        fcum[i + 1] = sum0;
    }
    // the maximum cumulative probability as a function of photon energy
    let dle = RealMath::r_ln(emax / emin) / (ne - 1) as f64;
    let mut i = 1usize;
    for j in 1..=ne {
        let e = emin * RealMath::r_exp(dle * (j - 1) as f64);
        let xmax = 20.607544 * 2.0 * e / prm;
        let mut k = i;
        while k <= ndat - 1 {
            if xmax >= x[k] && xmax < x[k + 1] {
                break;
            }
            k += 1;
        }
        i = k;
        let b = b_array[i];
        let (x1, x2) = (x[i], xmax);
        let pow_x1 = RealMath::r_powf(x1, 2.0 * b);
        let pow_x2 = RealMath::r_powf(x2, 2.0 * b);
        pe_array[j] = fcum[i] + f[i] * f[i] * (x2 * x2 * pow_x2 - x1 * x1 * pow_x1) / ((1.0 + b) * pow_x1);
    }
    i_array[ncbin] = i as i32;
    // renormalise so that pe_array(emax) = 1 (slightly larger so fcum is never underestimated)
    let anorm = 1.0 / Real::sqrt(pe_array[ne]);
    let anorm1 = 1.005 / pe_array[ne];
    let anorm2 = 1.0 / pe_array[ne];
    for j in 1..=ne {
        pe_array[j] *= anorm1;
        if pe_array[j] > 1.0 {
            pe_array[j] = 1.0;
        }
    }
    for j in 1..=ndat {
        f[j] *= anorm;
        fcum[j] *= anorm2;
        c_array[j] = (1.0 + b_array[j]) / ((x[j] * f[j]) * (x[j] * f[j]));
    }
    // uniform cumulative bins
    let dw = 1.0 / (ncbin - 1) as f64;
    let mut xold = x[1];
    let mut ibin = 1usize;
    let mut b = b_array[1];
    let mut pow_x1 = RealMath::r_powf(x[1], 2.0 * b);
    i_array[1] = 1;
    for i in 2..ncbin {
        let mut w = dw;
        loop {
            let x1 = xold;
            let x2 = x[ibin + 1];
            let t = x1 * x1 * RealMath::r_powf(x1, 2.0 * b);
            let pow_x2 = RealMath::r_powf(x2, 2.0 * b);
            let aux = f[ibin] * f[ibin] * (x2 * x2 * pow_x2 - t) / ((1.0 + b) * pow_x1);
            if aux > w {
                xold = RealMath::r_exp(RealMath::r_ln(t + w * (1.0 + b) * pow_x1 / f[ibin] / f[ibin]) / (2.0 + 2.0 * b));
                i_array[i] = ibin as i32;
                break;
            }
            w -= aux;
            xold = x2;
            ibin += 1;
            b = b_array[ibin];
            pow_x1 = RealMath::r_powf(xold, 2.0 * b);
        }
    }
    // b_array as needed at run time
    for j in 1..=ndat {
        b_array[j] = 0.5 / (1.0 + b_array[j]);
    }
    let data = RayleighData {
        xgrid: x.to_vec(),
        fcum,
        b_array,
        c_array,
        i_array,
        pmax: Pwlf::default(),
    };
    (pe_array, data)
}

/// `egs_init_rayleigh_sampling` (`egsnrc.mortran:8446-8530`) for one medium,
/// using the PEGS4 photon data (so the independent-atom form factor is built
/// from `pgs4form.dat`): `comp` is `(ZELEM, PZ)` per element, `ge0`, `ge1` and
/// `mge` the photon energy grid of the medium. Returns the sampling data with
/// the `pmax` fit filled in.
pub fn init_rayleigh_sampling(comp: &[(f64, f64)], ff: &FormFactorTable, ge0: f64, ge1: f64, mge: usize, prm: f64) -> RayleighData {
    let mut xgrid = alloc::vec![0.0_f64; MXRAYFF + 1];
    let mut f = alloc::vec![0.0_f64; MXRAYFF + 1];
    for i in 1..=MXRAYFF {
        xgrid[i] = ff.xval[i];
        let mut v = 0.0;
        for &(z, pz) in comp {
            let a = ff.aff[(z + 0.5) as usize][i];
            v += pz * a * a;
        }
        f[i] = Real::sqrt(v);
    }
    // x = 1e-4 avoids log(0)
    if xgrid[1] < 1e-6 {
        xgrid[1] = 1e-4;
    }
    let emin = RealMath::r_exp((1.0 - ge0) / ge1);
    let emax = RealMath::r_exp((mge as f64 - ge0) / ge1);
    let (pe_array, mut data) = prepare_rayleigh_data(&xgrid, &mut f, mge, emin, emax, prm);
    let ne = mge;
    let mut pmax = Pwlf { a0: alloc::vec![0.0; ne + 1], a1: alloc::vec![0.0; ne + 1] };
    for i in 1..ne {
        let gle = (i as f64 - ge0) / ge1;
        pmax.a1[i] = (pe_array[i + 1] - pe_array[i]) * ge1;
        pmax.a0[i] = pe_array[i] - pmax.a1[i] * gle;
    }
    pmax.a0[ne] = pmax.a0[ne - 1];
    pmax.a1[ne] = pmax.a1[ne - 1];
    data.pmax = pmax;
    data
}

/// `egs_rayleigh_sigma` (`egsnrc.mortran:8531-8555`): the analytic Rayleigh
/// cross section in barns at energy `e` (MeV) from a form factor `f` on the
/// grid `x` (both 1-based, `ndat` points). Zero entries are replaced by
/// [`zero_value`] in place, as upstream does.
pub fn rayleigh_sigma(e: f64, x: &mut [f64], f: &mut [f64]) -> f64 {
    const HC: f64 = 0.0123984768438;
    const HC2: f64 = 0.0001537222280;
    let ndat = x.len() - 1;
    let zero = zero_value();
    let c = 2.0 * HC2 / (e * e);
    let c2 = c * c;
    let xmax = e / HC;
    let mut sigma = 0.0_f64;
    for i in 1..ndat {
        if x[i] == 0.0 {
            x[i] = zero;
        }
        if x[i + 1] == 0.0 {
            x[i + 1] = zero;
        }
        if f[i] == 0.0 {
            f[i] = zero;
        }
        if f[i + 1] == 0.0 {
            f[i + 1] = zero;
        }
        let b = RealMath::r_ln(f[i + 1] / f[i]) / RealMath::r_ln(x[i + 1] / x[i]);
        let x1 = x[i];
        let mut x2 = x[i + 1];
        if x2 > xmax {
            x2 = xmax;
        }
        let pow_x1 = RealMath::r_powf(x1, 2.0 * b);
        let pow_x2 = RealMath::r_powf(x2, 2.0 * b);
        let mut raysig = pow_x2 * (x2 * x2 / (b + 1.0) - (c * x2.powi(4)) / (b + 2.0) + (c2 * x2.powi(6)) / (2.0 * b + 6.0));
        raysig -= pow_x1 * (x1 * x1 / (b + 1.0) - (c * x1.powi(4)) / (b + 2.0) + (c2 * x1.powi(6)) / (2.0 * b + 6.0));
        raysig = raysig * f[i] * f[i] / pow_x1;
        sigma += raysig;
        if x[i + 1] > xmax {
            break;
        }
    }
    0.49893439187842413747 * c * sigma
}

/// `egs_rayleigh_sampling` (`egsnrc.mortran:8558-8590`): the cosine and sine of
/// the Rayleigh scattering angle of a photon of energy `e` (MeV,
/// `gle = ln e`, `lgle` its table interval). The form factor is sampled by the
/// cumulative bins, then the angular factor `1 + cos^2` is rejected on.
pub fn rayleigh_sampling<R: UniformSource>(d: &RayleighData, e: f64, gle: f64, lgle: usize, rng: &mut R) -> (f64, f64) {
    const HC_I: f64 = 80.65506856998;
    const TWICE_HC2: f64 = 0.000307444456;
    let dwi = (RAYCDFSIZE - 1) as f64;
    let pmax = d.pmax.eval(lgle, gle);
    let xmax = HC_I * e;
    let csqthe;
    let mut costhe;
    loop {
        let rnnray1 = rng.uniform();
        let mut xv;
        loop {
            let mut rnnray0 = rng.uniform();
            rnnray0 *= pmax;
            let ibin = (1.0 + rnnray0 * dwi) as usize;
            let mut ib = d.i_array[ibin] as usize;
            if d.i_array[ibin + 1] as usize > ib {
                loop {
                    if rnnray0 < d.fcum[ib + 1] {
                        break;
                    }
                    ib += 1;
                }
            }
            rnnray0 = (rnnray0 - d.fcum[ib]) * d.c_array[ib];
            xv = d.xgrid[ib] * RealMath::r_exp(RealMath::r_ln(1.0 + rnnray0) * d.b_array[ib]);
            if xv < xmax {
                break;
            }
        }
        xv /= e;
        costhe = 1.0 - TWICE_HC2 * xv * xv;
        let c2 = costhe * costhe;
        if 2.0 * rnnray1 < 1.0 + c2 {
            csqthe = c2;
            break;
        }
    }
    (costhe, Real::sqrt(1.0 - csqthe))
}

/// `$N_TRIPLET_DATA` (`egsnrc.mortran:6215`) and `$MAX_TRIPLET`
/// (`egsnrc.macros:432`).
pub const N_TRIPLET_DATA: usize = 55;
/// See [`N_TRIPLET_DATA`].
pub const MAX_TRIPLET: usize = 250;

/// `COMMON/triplet_data/`, filled by [`init_triplet`]: the triplet fraction of
/// pair production as `a * ln(E) + b` in `MAX_TRIPLET` log-energy bins per
/// medium (1-based medium and bin).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TripletData {
    /// `a_triplet(bin, medium)`: `a_triplet[medium][bin]`.
    pub a_triplet: Vec<Vec<f64>>,
    /// `b_triplet(bin, medium)`.
    pub b_triplet: Vec<Vec<f64>>,
    /// `dl_triplet`.
    pub dl_triplet: f64,
    /// `dli_triplet`.
    pub dli_triplet: f64,
    /// `bli_triplet`.
    pub bli_triplet: f64,
    /// `log_4rm`.
    pub log_4rm: f64,
}

/// `init_triplet` (`egsnrc.mortran:6218-6285`) for `itriplet > 0`: reads
/// `triplet.data` from `text` (the energy count and grid, then per element a
/// header line, the pair and the triplet cross sections) and builds the
/// per-medium triplet fraction by cubic spline. `comp[medium]` is that
/// medium's `(ZELEM, PZ)` per element (`comp[0]` unused); `rm` is `RM`.
///
/// # Errors
///
/// A message when the data are short, malformed or exceed `N_TRIPLET_DATA`.
pub fn init_triplet(text: &str, comp: &[Vec<(f64, f64)>], rm: f64) -> Result<TripletData, String> {
    let mut lines = text.lines();
    let ntrip = read_record(&mut lines, 1)?[0] as usize;
    if ntrip > N_TRIPLET_DATA || ntrip < 3 {
        return Err(alloc::format!("Max. number of data points per element is {N_TRIPLET_DATA}"));
    }
    let mut energies = core::iter::once(0.0).chain(read_record(&mut lines, ntrip)?).collect::<Vec<f64>>();
    let mut sig_pair = alloc::vec![alloc::vec![0.0; ntrip + 1]];
    let mut sig_triplet = alloc::vec![alloc::vec![0.0; ntrip + 1]];
    for _ in 1..=100 {
        lines.next().ok_or_else(|| String::from("data ends early"))?;
        sig_pair.push(core::iter::once(0.0).chain(read_record(&mut lines, ntrip)?).collect());
        sig_triplet.push(core::iter::once(0.0).chain(read_record(&mut lines, ntrip)?).collect());
    }
    let mut ifirst = 0usize;
    for i in 1..=ntrip {
        if ifirst == 0 && energies[i] > 4.01 * rm {
            ifirst = i;
        }
        energies[i] = RealMath::r_ln(energies[i]);
    }
    if ifirst < 2 {
        return Err(String::from("triplet energies must start below 4.01 m and rise above it"));
    }
    let log_4rm = RealMath::r_ln(4.0 * rm);
    energies[ifirst - 1] = log_4rm;
    let dl_triplet = (energies[ntrip] - log_4rm) / MAX_TRIPLET as f64;
    let dli_triplet = 1.0 / dl_triplet;
    let bli_triplet = 1.0 - log_4rm / dl_triplet;
    let nmed = comp.len() - 1;
    let mut a_triplet = alloc::vec![alloc::vec![0.0; MAX_TRIPLET + 1]; nmed + 1];
    let mut b_triplet = a_triplet.clone();
    let npts = ntrip - ifirst + 2;
    let xs: Vec<f64> = core::iter::once(0.0).chain(energies[ifirst - 1..=ntrip].iter().copied()).collect();
    for imed in 1..=nmed {
        let mut sigp = alloc::vec![0.0; ntrip + 1];
        let mut sigt = alloc::vec![0.0; ntrip + 1];
        for i in 1..=ntrip {
            for &(z, pz) in &comp[imed] {
                let iz = (z + 0.1) as usize;
                sigp[i] += pz * sig_pair[iz][i];
                sigt[i] += pz * sig_triplet[iz][i];
            }
        }
        let mut f_triplet = alloc::vec![0.0; npts + 1];
        for i in ifirst..=ntrip {
            f_triplet[i - ifirst + 2] = sigt[i] / (sigp[i] + sigt[i]);
        }
        f_triplet[1] = 0.0;
        let co = set_spline(&xs, &f_triplet);
        let mut log_e = log_4rm;
        let mut f_old = 0.0;
        for i in 1..MAX_TRIPLET {
            log_e += dl_triplet;
            let f_new = spline(log_e, &xs, &co);
            a_triplet[imed][i] = (f_new - f_old) * dli_triplet;
            b_triplet[imed][i] = f_new - a_triplet[imed][i] * log_e;
            f_old = f_new;
        }
    }
    Ok(TripletData { a_triplet, b_triplet, dl_triplet, dli_triplet, bli_triplet, log_4rm })
}

/// The `save`d state of `sample_triplet` (`egsnrc.mortran:4744-4757`): the
/// per-energy-bin Markov chain (best cross section so far and the random
/// numbers that gave it) and the binning, set up on the first call.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TripletState {
    is_initialized: bool,
    fmax: Vec<f64>,
    eta_p: Vec<f64>,
    eta_ep: Vec<f64>,
    eta_costp: Vec<f64>,
    eta_costm: Vec<f64>,
    ebin: Vec<f64>,
    wp: Vec<f64>,
    qmin: Vec<f64>,
    kmin: f64,
    kmax: f64,
    dlogki: f64,
    alogkm: f64,
    prmi: f64,
}

/// Kinematic limits of the positron energy for pair momentum `pr`.
fn triplet_pp_limits(k: f64, pr: f64, er: f64) -> (f64, f64) {
    let aux = er - pr - 1.0;
    let a1 = (k - pr) * (1.0 - er - k * aux);
    let a2 = 1.0 + k - er;
    let a3 = 1.0 / (aux * (pr + er - 2.0 * k - 1.0));
    let d = a2 * Real::sqrt(aux * (2.0 * k * er + k * k * aux - pr * (er + pr + 1.0) / 2.0));
    let px1 = (a1 + d) * a3;
    let px2 = (a1 - d) * a3;
    if px1 < px2 { (px1, px2) } else { (px2, px1) }
}

/// `sample_triplet` (`egsnrc.mortran:4732-5043`): triplet production by the
/// photon on top of the stack, after Borsellino's first Born approximation,
/// by a Markov chain over the kinematic variables in `MAX_TRIPLET`
/// log-energy bins. The recoil electron goes on top (slot `np + 2`), the pair
/// below it. `up_max` is the largest photon energy in any medium's tables
/// (`UP(i)`, MeV), `prm` is `PRM`, `gle` is `ln` of the photon energy. A
/// photon at or below `4 prm`, or a state whose `up_max` is not above
/// `4.1 prm`, returns with the stack untouched, as upstream does.
///
/// # Errors
///
/// [`EgsError::StackExceeded`].
pub fn sample_triplet<R: UniformSource>(
    stack: &mut Stack,
    st: &mut TripletState,
    up_max: f64,
    prm: f64,
    gle: f64,
    rng: &mut R,
) -> Result<(), EgsError> {
    let tiny_eta = 1e-6;
    if !st.is_initialized {
        st.is_initialized = true;
        st.fmax = alloc::vec![-1.0; MAX_TRIPLET + 1];
        st.eta_p = alloc::vec![0.0; MAX_TRIPLET + 1];
        st.eta_ep = st.eta_p.clone();
        st.eta_costp = st.eta_p.clone();
        st.eta_costm = st.eta_p.clone();
        st.ebin = st.eta_p.clone();
        st.wp = st.eta_p.clone();
        st.qmin = st.eta_p.clone();
        st.kmax = up_max;
        st.kmin = 4.1 * prm;
        if st.kmax <= st.kmin {
            return Ok(());
        }
        st.dlogki = (MAX_TRIPLET - 1) as f64;
        st.dlogki /= RealMath::r_ln(st.kmax / st.kmin);
        st.alogkm = 1.0 - st.dlogki * RealMath::r_ln(st.kmin);
        st.prmi = 1.0 / prm;
        for i in 1..=MAX_TRIPLET {
            let k = 4.1 * RealMath::r_exp((i as f64 - 1.0) / st.dlogki);
            st.ebin[i] = k;
            let qmin = 4.0 * k / (k * (k - 1.0) + (k + 1.0) * Real::sqrt(k * (k - 4.0)));
            let qmax = (k * (k - 1.0) + (k + 1.0) * Real::sqrt(k * (k - 4.0))) / (2.0 * k + 1.0);
            st.qmin[i] = qmin;
            st.wp[i] = RealMath::r_ln(qmax / qmin);
        }
    }
    if st.kmax <= st.kmin {
        return Ok(());
    }
    let peig = stack.e[stack.np];
    if peig <= 4.0 * prm {
        return Ok(());
    }
    stack.check_stack(stack.np + 2, "sample_triplet")?;

    // energy bin
    let i;
    if peig <= st.kmin {
        i = 1usize;
    } else if peig >= st.kmax {
        i = MAX_TRIPLET;
    } else {
        let ai = st.alogkm + st.dlogki * gle;
        let mut ii = ai as usize;
        let ai = ai - ii as f64;
        let rnno = rng.uniform();
        if rnno < ai {
            ii += 1;
        }
        i = ii;
    }

    let mut k = st.ebin[i];
    let (mut eta_pr, mut eta_ep, mut eta_costp, mut eta_costm);
    // the retry loop of the bin-energy trial
    loop {
        eta_pr = rng.uniform();
        if eta_pr < tiny_eta {
            eta_pr = tiny_eta;
        }
        let pr = st.qmin[i] * RealMath::r_exp(eta_pr * st.wp[i]);
        let pr2 = pr * pr;
        let er = Real::sqrt(1.0 + pr2);
        let (pp_min, pp_max) = triplet_pp_limits(k, pr, er);
        let ep_min = Real::sqrt(1.0 + pp_min * pp_min);
        let ep_max = Real::sqrt(1.0 + pp_max * pp_max);
        eta_ep = rng.uniform();
        if eta_ep < tiny_eta {
            eta_ep = tiny_eta;
        }
        let w_ep = ep_max - ep_min;
        let ep = ep_min + eta_ep * w_ep;
        let pp2 = ep * ep - 1.0;
        let pp = Real::sqrt(pp2);
        let k2p2 = k * k + pp2;
        let em = k + 1.0 - er - ep;
        let pm2 = em * em - 1.0;
        let pm = Real::sqrt(pm2);
        let mup_min = (k2p2 - (pr + pm) * (pr + pm)) / (2.0 * k * pp);
        eta_costp = rng.uniform();
        if eta_costp < tiny_eta {
            eta_costp = tiny_eta;
        }
        let epp = ep / pp;
        let mut wmup = RealMath::r_ln((epp - 1.0) / (epp - mup_min));
        let cost_p = epp - (epp - mup_min) * RealMath::r_exp(wmup * eta_costp);
        wmup *= cost_p - epp;
        let mut sint_p = 1.0 - cost_p * cost_p;
        if sint_p > 1e-20 {
            sint_p = Real::sqrt(sint_p);
        } else {
            sint_p = 1e-10;
        }
        let k2p2x = k2p2 - 2.0 * k * pp * cost_p;
        let b = pr2 - k2p2x - pm2;
        let aux1 = k - pp * cost_p;
        let aux12 = aux1 * aux1;
        let pp_sintp = pp * sint_p;
        let pp_sntp2 = pp_sintp * pp_sintp;
        let d1 = pm2 * (aux12 + pp_sntp2) - b * b / 4.0;
        if d1 <= 0.0 {
            continue;
        }
        let d = 2.0 * pp_sintp * Real::sqrt(d1);
        let aux3 = 0.5 / (aux12 + pp_sntp2);
        let xmin = (-b * aux1 - d) * aux3;
        let xmax = (-b * aux1 + d) * aux3;
        eta_costm = rng.uniform();
        if eta_costm < tiny_eta {
            eta_costm = tiny_eta;
        }
        let aux6 = Real::sqrt((em - xmin) / (em - xmax));
        let aux7 = aux6 * RealMath::r_tan(1.570796326794897 * eta_costm);
        let uu = (aux7 - 1.0) / (aux7 + 1.0);
        let mut cost_m = 0.5 * (xmax + xmin + 2.0 * uu * (xmax - xmin) / (1.0 + uu * uu));
        let mut wmum = Real::sqrt((xmax - cost_m) * (cost_m - xmin));
        wmum = wmum * aux6 * (em - cost_m) / (em - xmin);
        cost_m /= pm;
        let sint_m = Real::sqrt(1.0 - cost_m * cost_m);
        let pm_sintm = pm * sint_m;
        let cphi = (b + 2.0 * pm * cost_m * aux1) / (2.0 * pp_sintp * pm_sintm);
        if cphi.abs() >= 1.0 {
            continue;
        }
        let sphi = Real::sqrt(1.0 - cphi * cphi);
        // the Borsellino cross section
        let k3 = k * (pp * cost_p - ep);
        let k2 = k * (pm * cost_m - em);
        let k22 = k2 * k2;
        let k32 = k3 * k3;
        let k2k3i = 1.0 / (k2 * k3);
        let s2 = pp * pm * (cost_p * cost_m + sint_p * sint_m * cphi) - ep * em;
        let s3 = k2 - em + 1.0 - s2;
        let q2 = 2.0 * (er - 1.0);
        let s_1 = k32 + k22 + (q2 - 2.0) * s2 - (1.0 - q2 / 2.0) * (k32 + k22) * k2k3i;
        let aux4 = k3 * ep - k2 * em;
        let s_2 = -q2 * (ep * ep + em * em) + 2.0 * s2 - (2.0 * aux4 * aux4 - k22 - k32) * k2k3i;
        let mut sigma = (pp * pm2 * pm * k2k3i / (q2 * q2 * (em * s3 + er)) * (s_1 * (1.0 - q2 / 4.0) + s_2 * (1.0 + q2 / 4.0))).abs();
        let cphi_factor = (2.0 * er * pm2 - em * (k2p2x - pr2 - pm2)).abs() / (2.0 * pp_sintp * pm_sintm * pm2 * sphi);
        sigma = sigma * cphi_factor * w_ep * wmup * wmum * st.wp[i] * pr2 / er;
        let mut use_it = true;
        if sigma < st.fmax[i] {
            let rnno = rng.uniform();
            if sigma < st.fmax[i] * rnno {
                use_it = false;
            }
        }
        if use_it {
            st.fmax[i] = sigma;
            st.eta_p[i] = eta_pr;
            st.eta_ep[i] = eta_ep;
            st.eta_costp[i] = eta_costp;
            st.eta_costm[i] = eta_costm;
        } else {
            eta_pr = st.eta_p[i];
            eta_ep = st.eta_ep[i];
            eta_costp = st.eta_costp[i];
            eta_costm = st.eta_costm[i];
        }
        // recompute with the actual photon energy; a failure retries from the top
        k = peig * st.prmi;
        let aux5 = k * (k - 1.0) + (k + 1.0) * Real::sqrt(k * (k - 4.0));
        let qmin = 4.0 * k / aux5;
        let qmax = aux5 / (2.0 * k + 1.0);
        let pr = qmin * RealMath::r_exp(eta_pr * RealMath::r_ln(qmax / qmin));
        let pr2 = pr * pr;
        let er = Real::sqrt(1.0 + pr2);
        let (pp_min, pp_max) = triplet_pp_limits(k, pr, er);
        let ep_min = Real::sqrt(1.0 + pp_min * pp_min);
        let ep_max = Real::sqrt(1.0 + pp_max * pp_max);
        let w_ep = ep_max - ep_min;
        let ep = ep_min + eta_ep * w_ep;
        let pp2 = ep * ep - 1.0;
        let pp = Real::sqrt(pp2);
        let k2p2 = k * k + pp2;
        let em = k + 1.0 - er - ep;
        let pm2 = em * em - 1.0;
        let pm = Real::sqrt(pm2);
        let mup_min = (k2p2 - (pr + pm) * (pr + pm)) / (2.0 * k * pp);
        let epp = ep / pp;
        let wmup = RealMath::r_ln((epp - 1.0) / (epp - mup_min));
        let cost_p = epp - (epp - mup_min) * RealMath::r_exp(wmup * eta_costp);
        let sint_p = Real::sqrt(1.0 - cost_p * cost_p);
        let k2p2x = k2p2 - 2.0 * k * pp * cost_p;
        let b = pr2 - k2p2x - pm2;
        let aux1 = k - pp * cost_p;
        let aux12 = aux1 * aux1;
        let pp_sintp = pp * sint_p;
        let pp_sntp2 = pp_sintp * pp_sintp;
        let d1 = pm2 * (aux12 + pp_sntp2) - b * b / 4.0;
        if d1 <= 0.0 {
            // upstream does not reset `k` before `goto :retry_triplet:`
            continue;
        }
        let d = 2.0 * pp_sintp * Real::sqrt(d1);
        let aux3 = 0.5 / (aux12 + pp_sntp2);
        let xmin = (-b * aux1 - d) * aux3;
        let xmax = (-b * aux1 + d) * aux3;
        let aux6 = Real::sqrt((em - xmin) / (em - xmax));
        let aux7 = aux6 * RealMath::r_tan(1.570796326794897 * eta_costm);
        let uu = (aux7 - 1.0) / (aux7 + 1.0);
        let cost_m = 0.5 * (xmax + xmin + 2.0 * uu * (xmax - xmin) / (1.0 + uu * uu)) / pm;
        let sint_m = Real::sqrt(1.0 - cost_m * cost_m);
        let pm_sintm = pm * sint_m;
        let cphi = (b + 2.0 * pm * cost_m * aux1) / (2.0 * pp_sintp * pm_sintm);
        if cphi.abs() >= 1.0 {
            // upstream does not reset `k` before `goto :retry_triplet:`
            continue;
        }
        let sphi = Real::sqrt(1.0 - cphi * cphi);

        // the final momenta, in the frame with the photon along z
        let phi = rng.uniform() * 6.283185307179586;
        let mut ppx = pp * sint_p;
        let mut ppy = 0.0;
        let mut pmx = pm * sint_m * cphi;
        let mut pmy = pm * sint_m * sphi;
        let cphi = RealMath::r_cos(phi);
        let sphi = RealMath::r_sin(phi);
        let mut tmp = ppx * sphi;
        ppx = ppx * cphi - ppy * sphi;
        ppy = tmp + ppy * cphi;
        tmp = pmx * sphi;
        pmx = pmx * cphi - pmy * sphi;
        pmy = tmp + pmy * cphi;
        let ppz = pp * cost_p;
        let pmz = pm * cost_m;
        let prx = -ppx - pmx;
        let pry = -ppy - pmy;
        let prz = k - ppz - pmz;

        let np = stack.np;
        stack.np_old = np;
        stack.transfer_properties(np, np);
        stack.transfer_properties(np + 1, np);
        stack.transfer_properties(np + 2, np + 1);
        let (ppi, pmi, pri) = (1.0 / pp, 1.0 / pm, 1.0 / pr);
        let (a, bb, c) = (stack.u[np], stack.v[np], stack.w[np]);
        let sinpsi2 = a * a + bb * bb;
        // (slot, charge, energy, 1/p, px, py, pz) for the positron, electron, recoil
        let ep_gt_em = ep > em;
        let (slot_p, slot_m) = if ep_gt_em { (np, np + 1) } else { (np + 1, np) };
        let parts = [
            (slot_p, 1_i32, ep * prm, ppi, ppx, ppy, ppz),
            (slot_m, -1_i32, em * prm, pmi, pmx, pmy, pmz),
            (np + 2, -1_i32, er * prm, pri, prx, pry, prz),
        ];
        if sinpsi2 > 1e-20 {
            let sinpsi = Real::sqrt(sinpsi2);
            let sindel = bb / sinpsi;
            let cosdel = a / sinpsi;
            for (slot, iq, en, pi, px, py, pz) in parts {
                stack.u[slot] = pi * (c * cosdel * px - sindel * py + a * pz);
                stack.v[slot] = pi * (c * sindel * px + cosdel * py + bb * pz);
                stack.w[slot] = pi * (c * pz - sinpsi * px);
                stack.iq[slot] = iq;
                stack.e[slot] = en;
            }
        } else {
            for (slot, iq, en, pi, px, py, pz) in parts {
                stack.u[slot] = pi * px;
                stack.v[slot] = pi * py;
                stack.w[slot] = c * pi * pz;
                stack.iq[slot] = iq;
                stack.e[slot] = en;
            }
        }
        stack.np = np + 2;
        return Ok(());
    }
}

/// `$NRC-PAIR-NXX` and `$NRC-PAIR-NEE` (`egsnrc.macros:413-414`).
pub const NRC_PAIR_NXX: usize = 65;
/// See [`NRC_PAIR_NXX`].
pub const NRC_PAIR_NEE: usize = 84;

/// One energy bin of the NRC pair data of a medium: the density on the `x`
/// grid and its alias table (1-based).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NrcPairBin {
    /// `nrcp_fdata(1..NXX, ie, medium)` (index 0 unused, so `fdata[1..=65]`;
    /// the alias grid runs over `fdata[1..=NXX]` as `fs(0:nsbin)` shifted).
    pub fdata: Vec<f64>,
    /// `nrcp_wdata`.
    pub wdata: Vec<f64>,
    /// `nrcp_idata`.
    pub idata: Vec<i32>,
}

/// `COMMON/nrc_pair/` for all media, filled by [`init_nrc_pair`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NrcPairData {
    /// `nrcp_emin`, `nrcp_emax` (units of `m c^2`).
    pub emin: f64,
    /// See `emin`.
    pub emax: f64,
    /// `nrcp_dle`.
    pub dle: f64,
    /// `nrcp_dlei`.
    pub dlei: f64,
    /// `nrcp_xdata(1..NXX)` as the 0-based abscissas `0..=NX-1` the alias
    /// sampler wants (`xdata[ix] = nrcp_xdata(ix + 1)`).
    pub xdata: Vec<f64>,
    /// `tables[medium][ie]`, both 1-based.
    pub tables: Vec<Vec<NrcPairBin>>,
}

/// `init_nrc_pair` (`egsnrc.mortran:7255-7360`): builds the NRC pair tables
/// from the data base `pair_nrc1.data`, which is a Fortran direct-access
/// file whose record length depends on build-time macros (`$RECL-FACTOR`,
/// `$BYTE_ORDER`) and is not in the clone. The file decoding is therefore
/// left to the caller: `emin`, `emax`, `ne`, `nb` are the header record, and
/// `record(irec)` returns the `NRC_PAIR_NXX` single-precision values of
/// direct-access record `irec` (1-based; record 1 is the header and element
/// `Z` energy `ie` is record `(Z-1) * ne + 1 + ie`). `comp[medium]` is
/// `(ZELEM, PZ)` per element (`comp[0]` unused).
///
/// # Errors
///
/// A message when `nb` or `ne` do not match the compiled grid sizes, or when
/// `record` returns `None` or a short record.
pub fn init_nrc_pair<F: FnMut(usize) -> Option<Vec<f32>>>(
    emin: f32,
    emax: f32,
    ne: i32,
    nb: i32,
    comp: &[Vec<(f64, f64)>],
    mut record: F,
) -> Result<NrcPairData, String> {
    if nb as usize != NRC_PAIR_NXX {
        return Err(String::from("Inconsistent x-grid size"));
    }
    if ne as usize != NRC_PAIR_NEE {
        return Err(String::from("Inconsistent energy grid size"));
    }
    let (emin, emax) = (f64::from(emin), f64::from(emax));
    let dle = RealMath::r_ln((emax - 2.0) / (emin - 2.0)) / (ne - 1) as f64;
    let nb = nb as usize;
    let nbb = nb / 2;
    let ddx = Real::sqrt(0.5) / nbb as f64;
    // nrcp_xdata(1..nb)
    let mut x1 = alloc::vec![0.0_f64; nb + 1];
    for ix in 0..=nbb {
        let xx = ddx * ix as f64;
        x1[ix + 1] = xx * xx;
    }
    for ix in (0..nbb).rev() {
        let xx = ddx * ix as f64;
        x1[nb - ix] = 1.0 - xx * xx;
    }
    let xdata: Vec<f64> = x1[1..].to_vec();
    let nmed = comp.len() - 1;
    let mut tables: Vec<Vec<NrcPairBin>> = alloc::vec![Vec::new()];
    for imed in 1..=nmed {
        let mut f = alloc::vec![alloc::vec![0.0_f64; nb + 1]; NRC_PAIR_NEE + 1];
        for &(z, pz) in &comp[imed] {
            let iz = (z + 0.5) as usize;
            let tmp = pz * z * z;
            let mut irec = (iz - 1) * ne as usize + 2;
            for fe in f.iter_mut().skip(1) {
                let tarray = record(irec).ok_or_else(|| String::from("I/O error while reading NRC pair data file"))?;
                if tarray.len() < nb {
                    return Err(String::from("short NRC pair data record"));
                }
                for ix in 1..=nb {
                    fe[ix] += tmp * f64::from(tarray[ix - 1]);
                }
                irec += 1;
            }
        }
        let mut bins = alloc::vec![NrcPairBin::default()];
        for fe in f.iter().skip(1) {
            // prepare_alias_table(nb-1, nrcp_xdata, nrcp_fdata(1..)): the
            // 0-based f(0:nb-1) is the data stored at 1..nb
            let (ws, ib) = prepare_alias_table(&xdata, &fe[1..]);
            bins.push(NrcPairBin { fdata: fe[1..].to_vec(), wdata: ws, idata: ib });
        }
        tables.push(bins);
    }
    Ok(NrcPairData { emin, emax, dle, dlei: 1.0 / dle, xdata, tables })
}

/// `$MXBREN`, `$MXBRXX`, `$MXBREL`, `$MXGAUSS`, `$MXBRES`, `$MXBRXS`
/// (`egsnrc.macros:350-355`).
pub const MXBREN: usize = 57;
/// See [`MXBREN`].
pub const MXBRXX: usize = 54;
/// See [`MXBREN`].
pub const MXBREL: usize = 100;
/// See [`MXBREN`].
pub const MXGAUSS: usize = 64;
/// See [`MXBREN`].
pub const MXBRES: usize = 100;
/// See [`MXBREN`].
pub const MXBRXS: usize = 50;

/// One photon-energy bin of the NIST / NRC bremsstrahlung tables: the density
/// on `x = k / T` at `0..=MXBRXS` and its alias table (1-based).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NistBremsBin {
    /// `nb_xdata(0:MXBRXS, i, medium)`.
    pub xdata: Vec<f64>,
    /// `nb_fdata`.
    pub fdata: Vec<f64>,
    /// `nb_wdata(1:MXBRXS, ...)` (index 0 unused).
    pub wdata: Vec<f64>,
    /// `nb_idata`.
    pub idata: Vec<i32>,
}

/// `COMMON/nist_brems/` for one medium.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NistBremsMedium {
    /// `nb_emin`: lowest kinetic energy covered, MeV.
    pub emin: f64,
    /// `nb_emax`.
    pub emax: f64,
    /// `nb_lemin`.
    pub lemin: f64,
    /// `nb_lemax`.
    pub lemax: f64,
    /// `nb_dle`.
    pub dle: f64,
    /// `nb_dlei`.
    pub dlei: f64,
    /// `bins[i]`, `i = 1..=MXBRES` (index 0 unused).
    pub bins: Vec<NistBremsBin>,
}

/// What `init_nist_brems` takes from the media of the run.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NistBremsInput {
    /// `(ZELEM, PZ, WA)` per element.
    pub comp: Vec<(f64, f64, f64)>,
    /// `AP(medium)`, MeV.
    pub ap: f64,
    /// `AE(medium)`, total energy, MeV.
    pub ae: f64,
    /// `UE(medium)`, total energy, MeV.
    pub ue: f64,
}

/// `init_nist_brems` (`egsnrc.mortran:6892-7228`): replaces the PEGS4
/// bremsstrahlung part of the electron and positron cross sections with the
/// NIST (`ibr_nist = 1`, `nist_brems.data`) or NRC (`2`, `nrc_brems.data`)
/// data in `text`, in place in `media[m].esig`, `ebr1`, `psig`, `pbr1`,
/// `pbr2` (and the `esig_e`, `psig_e` maxima), and builds the alias tables
/// `BREMS` samples from. `media` and `inputs` are 1-based; the data text is
/// the file's content (first line a header, then `nmix kmix`, the energy
/// grid, the `x` grid, a skipped line and `MXBREL` blocks of `nmix * kmix`
/// values). Returns `(esige_max, psige_max)`, the maxima over the media.
///
/// # Errors
///
/// A message when the data are short or too large, `ibr_nist` is not 1 or 2,
/// a medium has no table interval to overwrite, or the energy range is empty.
pub fn init_nist_brems(
    text: &str,
    ibr_nist: i32,
    inputs: &[NistBremsInput],
    media: &mut [ElectronMediumData],
    rm: f64,
) -> Result<(f64, f64), String> {
    if ibr_nist != 1 && ibr_nist != 2 {
        return Err(alloc::format!("init_nist_brems: unknown value of ibr_nist! ibr_nist = {ibr_nist}"));
    }
    const AMU: f64 = 1660.5655; // converts the cross sections from mB/atom to cm^2/g
    let mut lines = text.lines();
    lines.next().ok_or_else(|| String::from("data ends early"))?;
    let nk = read_record(&mut lines, 2)?;
    let (nmix, kmix) = (nk[0] as usize, nk[1] as usize);
    if kmix > MXBRXX {
        return Err(String::from("init_nist_brems: to many k values in data file!"));
    }
    if nmix > MXBREN {
        return Err(String::from("init_nist_brems: to many T values in data file!"));
    }
    let energy_array: Vec<f64> = core::iter::once(0.0).chain(read_record(&mut lines, nmix)?).collect();
    let x_array: Vec<f64> = core::iter::once(0.0).chain(read_record(&mut lines, kmix)?).collect();
    lines.next().ok_or_else(|| String::from("data ends early"))?;
    // cs_array(n, k, i): cs_array[i][k][n]
    let mut cs_array = alloc::vec![alloc::vec![alloc::vec![0.0_f64; nmix + 1]; kmix + 1]; MXBREL + 1];
    for plane in cs_array.iter_mut().skip(1) {
        let v = read_record(&mut lines, nmix * kmix)?;
        for k in 1..=kmix {
            for n in 1..=nmix {
                plane[k][n] = v[(k - 1) * nmix + (n - 1)];
            }
        }
    }
    let mut xi_array = alloc::vec![0.0_f64; kmix + 1];
    for k in 1..=kmix {
        xi_array[k] = RealMath::r_ln(1.0 - x_array[k] + 1e-6);
    }
    let (x_gauss, w_gauss) = gauss_legendre(0.0, 1.0, MXGAUSS);

    let nmed = inputs.len() - 1;
    let (mut esige_max, mut psige_max) = (0.0_f64, 0.0_f64);
    for medium in 1..=nmed {
        let inp = &inputs[medium];
        let ap = inp.ap;
        let log_ap = RealMath::r_ln(ap);
        let emin = (inp.ae - rm).max(ap);
        let mut ifirst = nmix + 1;
        for i in 1..=nmix {
            if energy_array[i] >= emin {
                ifirst = i;
                break;
            }
        }
        let mut ilast = 1usize;
        for i in (1..=nmix).rev() {
            if energy_array[i] < inp.ue - rm {
                ilast = i + 1;
                break;
            }
            ilast = 1;
        }
        if ilast > nmix {
            ilast = nmix;
        }
        if ifirst > ilast {
            return Err(String::from("init_nist_brems: no data in the energy range of the medium"));
        }
        let nener = ilast - ifirst + 1;
        let mut ee = alloc::vec![0.0_f64; nener + 1];
        let mut ele = alloc::vec![0.0_f64; nener + 1];
        let mut cs = alloc::vec![alloc::vec![0.0_f64; kmix + 1]; nener + 1];
        let mut cse = alloc::vec![0.0_f64; nener + 1];
        let mut sum_a = 0.0;
        for &(_, pz, wa) in &inp.comp {
            sum_a += pz * wa;
        }
        sum_a *= AMU;
        let mut csx = alloc::vec![0.0_f64; kmix + 1];
        for i in ifirst..=ilast {
            let ii = i + 1 - ifirst;
            ee[ii] = energy_array[i];
            ele[ii] = RealMath::r_ln(ee[ii]);
            for k in 1..=kmix {
                cs[ii][k] = 0.0;
                for &(z0, pz, _) in &inp.comp {
                    let iz = (z0 + 0.1) as usize;
                    let z = z0 * z0 / sum_a;
                    cs[ii][k] += pz * z * cs_array[iz][k][i];
                }
                csx[k] = RealMath::r_ln(cs[ii][k]);
            }
            let co = set_spline(&xi_array, &csx);
            // integrate
            cse[ii] = 0.0;
            let aux = RealMath::r_ln(ee[ii] / ap);
            for ig in 0..MXGAUSS {
                let xi = RealMath::r_ln(1.0 - ap / ee[ii] * RealMath::r_exp(x_gauss[ig] * aux) + 1e-6);
                let res = spline(xi, &xi_array, &co);
                cse[ii] += w_gauss[ig] * RealMath::r_exp(res);
            }
        }
        let co_e = set_spline(&ele[..=nener], &cse[..=nener]);
        let m = &mut media[medium];
        let neke = m.esig.a0.len() - 1;
        if neke < 2 {
            return Err(String::from("init_nist_brems: the electron tables have no energy intervals"));
        }
        let (eke0, eke1, rho) = (m.eke0, m.eke1, m.rho);
        // now replace the PEGS cross sections
        let (mut sigee, mut sigep) = (1e-15_f64, 1e-15_f64);
        let (mut si1_esig, mut si1_ebr1) = (0.0, 0.0);
        let (mut si1_psig, mut si1_pbr1, mut si1_pbr2) = (0.0, 0.0, 0.0);
        for i in 1..=neke {
            let eil = (i as f64 - eke0) / eke1;
            let ei = RealMath::r_exp(eil);
            let beta2 = ei * (ei + 2.0 * rm) / ((ei + rm) * (ei + rm));
            let sigb = if ei <= ap {
                1e-30
            } else {
                spline(eil, &ele[..=nener], &co_e) * RealMath::r_ln(ei / ap) / beta2 * rho
            };
            let mut sigt = m.esig.eval(i, eil);
            let mut ebr1 = m.ebr1.eval(i, eil);
            if sigt < 0.0 {
                sigt = 0.0;
            }
            ebr1 = ebr1.clamp(0.0, 1.0);
            if i > 1 {
                let (si_esig, si_ebr1) = (si1_esig, si1_ebr1);
                si1_esig = sigt * (1.0 - ebr1) + sigb;
                si1_ebr1 = sigb / si1_esig;
                m.esig.a1[i - 1] = (si1_esig - si_esig) * eke1;
                m.esig.a0[i - 1] = si1_esig - m.esig.a1[i - 1] * eil;
                m.ebr1.a1[i - 1] = (si1_ebr1 - si_ebr1) * eke1;
                m.ebr1.a0[i - 1] = si1_ebr1 - m.ebr1.a1[i - 1] * eil;
            } else {
                si1_esig = sigt * (1.0 - ebr1) + sigb;
                si1_ebr1 = sigb / si1_esig;
            }
            // positrons
            let mut sigt = m.psig.eval(i, eil);
            let mut pbr1 = m.pbr1.eval(i, eil);
            let mut pbr2 = m.pbr2.eval(i, eil);
            if sigt < 0.0 {
                sigt = 0.0;
            }
            pbr1 = pbr1.clamp(0.0, 1.0);
            pbr2 = pbr2.clamp(0.0, 1.0);
            let mut sig_bhabha = sigt * (pbr2 - pbr1);
            if sig_bhabha < 0.0 {
                sig_bhabha = 0.0;
            }
            if i > 1 {
                let (si_psig, si_pbr1, si_pbr2) = (si1_psig, si1_pbr1, si1_pbr2);
                si1_psig = sigt * (1.0 - pbr1) + sigb;
                si1_pbr1 = sigb / si1_psig;
                si1_pbr2 = (sigb + sig_bhabha) / si1_psig;
                m.psig.a1[i - 1] = (si1_psig - si_psig) * eke1;
                m.psig.a0[i - 1] = si1_psig - m.psig.a1[i - 1] * eil;
                m.pbr1.a1[i - 1] = (si1_pbr1 - si_pbr1) * eke1;
                m.pbr1.a0[i - 1] = si1_pbr1 - m.pbr1.a1[i - 1] * eil;
                m.pbr2.a1[i - 1] = (si1_pbr2 - si_pbr2) * eke1;
                m.pbr2.a0[i - 1] = si1_pbr2 - m.pbr2.a1[i - 1] * eil;
            } else {
                si1_psig = sigt * (1.0 - pbr1) + sigb;
                si1_pbr1 = sigb / si1_psig;
                si1_pbr2 = (sigb + sig_bhabha) / si1_psig;
            }
            let sige = si1_esig / m.ededx.eval(i, eil);
            if sige > sigee {
                sigee = sige;
            }
            let sige = si1_psig / m.pdedx.eval(i, eil);
            if sige > sigep {
                sigep = sige;
            }
        }
        for pw in [&mut m.esig, &mut m.ebr1, &mut m.psig, &mut m.pbr1, &mut m.pbr2] {
            pw.a1[neke] = pw.a1[neke - 1];
            pw.a0[neke] = pw.a0[neke - 1];
        }
        m.esig_e = sigee;
        m.psig_e = sigep;
        if sigee > esige_max {
            esige_max = sigee;
        }
        if sigep > psige_max {
            psige_max = sigep;
        }

        // the arrays for brems sampling
        let mut nb_emin = energy_array[ifirst];
        if nb_emin <= ap {
            nb_emin = energy_array[ifirst + 1];
        }
        let nb_emax = energy_array[ilast];
        let nb_lemin = RealMath::r_ln(nb_emin);
        let nb_lemax = RealMath::r_ln(nb_emax);
        let nb_dle = (nb_lemax - nb_lemin) / (MXBRES - 1) as f64;
        let nb_dlei = 1.0 / nb_dle;
        let mut bins = alloc::vec![NistBremsBin::default()];
        let mut eil = nb_lemin - nb_dle;
        for _ in 1..=MXBRES {
            eil += nb_dle;
            let ei = RealMath::r_exp(eil);
            let mut ii = nener + 1;
            for iii in 1..=nener {
                if ei < ee[iii] {
                    ii = iii;
                    break;
                }
            }
            // the Mortran loop leaves ii at nener + 1 when no energy exceeds ei
            ii = ii.saturating_sub(1);
            if ii < 1 {
                ii = 1;
            }
            if ii > nener - 1 {
                ii = nener - 1;
            }
            let ple = (eil - ele[ii]) / (ele[ii + 1] - ele[ii]);
            let qle = 1.0 - ple;
            for k in 1..=kmix {
                csx[k] = RealMath::r_ln(qle * cs[ii][k] + ple * cs[ii + 1][k]);
            }
            let co = set_spline(&xi_array, &csx);
            let mut xd = alloc::vec![0.0_f64; MXBRXS + 1];
            let mut fd = alloc::vec![0.0_f64; MXBRXS + 1];
            let x = ap / ei;
            let aux = -RealMath::r_ln(x);
            let xi = RealMath::r_ln(1.0 - x + 1e-6);
            let res = spline(xi, &xi_array, &co);
            xd[0] = 0.0;
            fd[0] = RealMath::r_exp(res);
            let mut k = kmix + 1;
            for kk in 1..=kmix {
                if x_array[kk] > x {
                    k = kk;
                    break;
                }
            }
            if k > kmix {
                k = kmix;
            }
            let mut ndat = 0usize;
            for j in (k + 1)..kmix {
                ndat += 1;
                if ndat > MXBRXS {
                    return Err(String::from("init_nist_brems: too many x points"));
                }
                xd[ndat] = RealMath::r_ln(x_array[j] / x) / aux;
                fd[ndat] = RealMath::r_exp(csx[j]);
            }
            ndat += 1;
            if ndat > MXBRXS {
                return Err(String::from("init_nist_brems: too many x points"));
            }
            xd[ndat] = 1.0;
            fd[ndat] = RealMath::r_exp(csx[kmix]);
            // fill intermediate points where linear interpolation in x is worst
            if ndat < MXBRXS {
                loop {
                    let (mut x_max_error, mut f_max_error, mut k_max_error) = (0.0, 0.0, 0usize);
                    let mut max_error = 0.0;
                    for kk in 0..ndat {
                        let xm = 0.5 * (xd[kk] + xd[kk + 1]);
                        let fm = 0.5 * (fd[kk] + fd[kk + 1]);
                        let xi = RealMath::r_ln(1.0 - ap / ei * RealMath::r_exp(xm * aux) + 1e-6);
                        let res = RealMath::r_exp(spline(xi, &xi_array, &co));
                        let error = (1.0 - fm / res).abs();
                        if error > max_error {
                            x_max_error = xm;
                            f_max_error = res;
                            max_error = error;
                            k_max_error = kk;
                        }
                    }
                    ndat += 1;
                    let mut kk = ndat;
                    while kk >= k_max_error + 2 {
                        xd[kk] = xd[kk - 1];
                        fd[kk] = fd[kk - 1];
                        kk -= 1;
                    }
                    xd[k_max_error + 1] = x_max_error;
                    fd[k_max_error + 1] = f_max_error;
                    if ndat == MXBRXS {
                        break;
                    }
                }
            }
            let (wd, id) = prepare_alias_table(&xd, &fd);
            bins.push(NistBremsBin { xdata: xd, fdata: fd, wdata: wd, idata: id });
        }
        m.brems.log_ap = log_ap;
        m.brems.nist = Some(NistBremsMedium {
            emin: nb_emin,
            emax: nb_emax,
            lemin: nb_lemin,
            lemax: nb_lemax,
            dle: nb_dle,
            dlei: nb_dlei,
            bins,
        });
    }
    Ok((esige_max, psige_max))
}

/// `$MXINPUT`, `$MXBCINP` (`egsnrc.mortran:7967-7968`) and `$MXSHXSEC`
/// (`egsnrc.macros:491`).
pub const MXINPUT: usize = 2000;
/// See [`MXINPUT`].
pub const MXBCINP: usize = 183;
/// See [`MXINPUT`].
pub const MXSHXSEC: usize = 30;

/// One `<prefix>_photo.data`-style file (`_photo`, `_pair`, `_triplet`,
/// `_rayleigh`, `_photonuc`): per element `Z = 1..` a count `ndat` and then
/// `ndat` pairs of `ln E` and `ln sigma` (1-based).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UserXsecTable {
    /// `elements[Z]` is `(etmp, ftmp)`, both 1-based; index 0 unused.
    pub elements: Vec<(Vec<f64>, Vec<f64>)>,
}

impl UserXsecTable {
    /// Parse every element block in the file's text.
    ///
    /// # Errors
    ///
    /// A message when a block is malformed or `ndat` exceeds [`MXINPUT`].
    pub fn from_text(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let mut elements = alloc::vec![(Vec::new(), Vec::new())];
        while let Ok(h) = read_record(&mut lines, 1) {
            let ndat = h[0] as usize;
            if ndat > MXINPUT {
                return Err(alloc::format!("Too many input data points. Max. is {MXINPUT}"));
            }
            let v = read_record(&mut lines, 2 * ndat)?;
            let mut e = alloc::vec![0.0];
            let mut f = alloc::vec![0.0];
            for k in 0..ndat {
                e.push(v[2 * k]);
                f.push(v[2 * k + 1]);
            }
            elements.push((e, f));
        }
        Ok(Self { elements })
    }
}

/// `egsi_get_data` (`egsnrc.mortran:8765-8850`): the cross section of a
/// medium on the photon energy grid `gle = (k - ge0) / ge1`, `k = 1..=n`, as
/// the sum over its elements (`zsorted`, ascending, with `pz_sorted`) of
/// `pz * sigma_Z(E)`. `flag` is 0 for photoelectric, Rayleigh and Compton
/// (log-log), 1 for pair and 2 for triplet (threshold-subtracted, zero below
/// threshold) and 3 for photonuclear (linear, zero outside the data). `rm` is
/// the electron rest energy. Returns the 1-based data (index 0 unused).
///
/// # Errors
///
/// A message when an element has no data, or (flag 0) an energy lies outside
/// the available range.
pub fn user_xsec_data(
    table: &UserXsecTable,
    flag: i32,
    n: usize,
    zsorted: &[f64],
    pz_sorted: &[f64],
    ge1: f64,
    ge0: f64,
    rm: f64,
) -> Result<Vec<f64>, String> {
    let mut data = alloc::vec![0.0_f64; n + 1];
    for (i, &zs) in zsorted.iter().enumerate() {
        let iz = (zs + 0.5) as usize;
        let (e0, f0) = table.elements.get(iz).ok_or_else(|| alloc::format!("no data for Z = {iz}"))?;
        let mut ndat = e0.len() - 1;
        let (etmp, ftmp);
        let mut eth = 0.0;
        if flag == 0 || flag == 3 {
            etmp = e0.clone();
            ftmp = f0.clone();
        } else {
            eth = if flag == 1 { 2.0 * rm } else { 4.0 * rm };
            let mut e = alloc::vec![0.0; ndat + 2];
            let mut f = alloc::vec![0.0; ndat + 2];
            for k in 1..=ndat {
                e[k + 1] = e0[k];
                f[k + 1] = f0[k];
            }
            ndat += 1;
            for k in 2..=ndat {
                f[k] -= 3.0 * RealMath::r_ln(1.0 - eth / RealMath::r_exp(e[k]));
            }
            f[1] = f[2];
            e[1] = RealMath::r_ln(eth);
            etmp = e;
            ftmp = f;
        }
        for k in 1..=n {
            let gle = (k as f64 - ge0) / ge1;
            let e = RealMath::r_exp(gle);
            let sig;
            if gle < etmp[1] || gle >= etmp[ndat] {
                if flag == 0 {
                    return Err(alloc::format!(
                        "Energy {} is outside the available data range of {} {}",
                        RealMath::r_exp(gle),
                        RealMath::r_exp(etmp[1]),
                        RealMath::r_exp(etmp[ndat])
                    ));
                } else if flag == 1 || flag == 2 {
                    sig = if gle < etmp[1] { 0.0 } else { RealMath::r_exp(ftmp[ndat]) };
                } else {
                    sig = 0.0;
                }
            } else {
                let mut kk = ndat;
                for k2 in 1..ndat {
                    if gle >= etmp[k2] && gle < etmp[k2 + 1] {
                        kk = k2;
                        break;
                    }
                }
                if flag != 3 {
                    let p = (gle - etmp[kk]) / (etmp[kk + 1] - etmp[kk]);
                    sig = RealMath::r_exp(p * ftmp[kk + 1] + (1.0 - p) * ftmp[kk]);
                } else {
                    let p = (e - RealMath::r_exp(etmp[kk])) / (RealMath::r_exp(etmp[kk + 1]) - RealMath::r_exp(etmp[kk]));
                    sig = p * RealMath::r_exp(ftmp[kk + 1]) + (1.0 - p) * RealMath::r_exp(ftmp[kk]);
                }
            }
            let mut sig = sig;
            if (flag == 1 || flag == 2) && e > eth {
                let t = 1.0 - eth / e;
                sig *= t * t * t;
            }
            data[k] += pz_sorted[i] * sig;
        }
    }
    Ok(data)
}

/// `egs_KN_sigma0` (`egsnrc.mortran:8728-8745`): the Klein-Nishina cross
/// section per electron at photon energy `e` (MeV), in barns scaled by upstream's
/// constant, with the low-energy series below `e = 0.01 prm`.
pub fn egs_kn_sigma0(e: f64, prm: f64) -> f64 {
    let con = 0.1274783851;
    let ko = e / prm;
    if ko < 0.01 {
        return 8.0 * con / 3.0 * (1.0 - ko * (2.0 - ko * (5.2 - 13.3 * ko))) / prm;
    }
    let c1 = 1.0 / (ko * ko);
    let c2 = 1.0 - 2.0 * (1.0 + ko) * c1;
    let c3 = (1.0 + 2.0 * ko) * c1;
    let eps2 = 1.0;
    let eps1 = 1.0 / (1.0 + 2.0 * ko);
    (c1 * (1.0 / eps1 - 1.0 / eps2) + c2 * RealMath::r_ln(eps2 / eps1) + eps2 * (c3 + 0.5 * eps2) - eps1 * (c3 + 0.5 * eps1)) / e * con
}

/// The bound-Compton input of `egs_init_user_photon` (used when `ibcmp(1) > 1`).
#[derive(Debug, Clone, PartialEq, Default)]
pub enum BoundComptonData {
    /// `ibcmp(1) <= 1`: plain Klein-Nishina.
    #[default]
    None,
    /// The default `compton_sigma.data`: a header `emin emax ne` and per
    /// element a row of `ne` values of the bound-to-KN cross-section ratio.
    Ratio(String),
    /// A user `<prefix>_compton.data` in the layout of the other cross-section files.
    Custom(String),
}

/// Everything `egs_init_user_photon` reads from the data files (as text) and
/// from the run's switches.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UserPhotonInput {
    /// `<prefix>_photo.data`.
    pub photo: String,
    /// `<prefix>_pair.data`.
    pub pair: String,
    /// `<prefix>_triplet.data`.
    pub triplet: String,
    /// `<prefix>_rayleigh.data`.
    pub rayleigh: String,
    /// `<photonuc>_photonuc.data` when `iphotonuc = 1`.
    pub photonuc: Option<String>,
    /// Bound Compton data when `ibcmp(1) > 1`.
    pub compton: BoundComptonData,
    /// `$MXGE`, the number of grid points (EGSnrc builds use 2000; the macro
    /// definition is not in the clone this was ported from).
    pub mxge: usize,
    /// `eadl_relax`.
    pub eadl_relax: bool,
    /// `rm` = `prm`.
    pub rm: f64,
    /// The atomic form factors for Rayleigh sampling (`pgs4form.dat`), needed
    /// when a medium has `iraylm = 1`.
    pub form_factors: Option<FormFactorTable>,
}

/// One medium for `egs_init_user_photon`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UserPhotonMedium {
    /// `(ZELEM, PZ, WA)` per element.
    pub comp: Vec<(f64, f64, f64)>,
    /// `rho`, g/cm^3.
    pub rho: f64,
    /// `AP`, MeV.
    pub ap: f64,
    /// `UP`, MeV.
    pub up: f64,
    /// `iraylm(medium) = 1`.
    pub rayleigh: bool,
}

/// The tables `egs_init_user_photon` builds for one medium (1-based).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UserPhotonMediumTables {
    /// `mge(medium)`.
    pub mge: usize,
    /// `ge0`, `ge1`.
    pub ge0: f64,
    /// See `ge0`.
    pub ge1: f64,
    /// `gmfp0/1`.
    pub gmfp: Pwlf,
    /// `gbr10/11`.
    pub gbr1: Pwlf,
    /// `gbr20/21`.
    pub gbr2: Pwlf,
    /// `cohe0/1`.
    pub cohe: Pwlf,
    /// `photonuc0/1` (flat 1 when photonuclear is off... upstream leaves it
    /// untouched, so this is all zeros then).
    pub photonuc: Pwlf,
    /// The Rayleigh sampling data when `rayleigh` and form factors were given.
    pub rayleigh: Option<RayleighData>,
}

/// The result of [`init_user_photon`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UserPhotonTables {
    /// Per medium, 1-based.
    pub media: Vec<UserPhotonMediumTables>,
    /// `binding_energies(k, iz)` replaced by the photo-absorption edges:
    /// `binding_energies[iz][k]`, `k = 1..=MXSHXSEC`, MeV, 1-based.
    pub binding_energies: Vec<Vec<f64>>,
}

/// `egs_init_user_photon` (`egsnrc.mortran:7972-8268`), without the
/// `.xsections` listing (`out`), the shellwise (MCDF) photoelectric option
/// and custom Rayleigh form-factor files: it reads the photo-absorption,
/// pair, triplet, Rayleigh, photonuclear and Compton data bases and builds,
/// per medium, the mean free path, pair and (pair + Compton) probabilities,
/// the coherent-scattering factor and the photonuclear factor as piecewise
/// linear fits in `ln E`, plus the photo-absorption edges.
///
/// Elements are taken in ascending `Z`; upstream's heap sort is replaced by a
/// stable sort, which only differs for repeated `Z`.
///
/// # Errors
///
/// A message when any data text is malformed or lacks an element.
pub fn init_user_photon(inp: &UserPhotonInput, media: &[UserPhotonMedium]) -> Result<UserPhotonTables, String> {
    let rm = inp.rm;
    let photo = UserXsecTable::from_text(&inp.photo)?;
    let pair = UserXsecTable::from_text(&inp.pair)?;
    let triplet = UserXsecTable::from_text(&inp.triplet)?;
    let rayleigh = UserXsecTable::from_text(&inp.rayleigh)?;
    let photonuc = match &inp.photonuc {
        Some(t) => Some(UserXsecTable::from_text(t)?),
        None => None,
    };
    let custom_compton = match &inp.compton {
        BoundComptonData::Custom(t) => Some(UserXsecTable::from_text(t)?),
        _ => None,
    };

    // replace binding energies with the edges in the photo-absorption file
    let mut binding_energies = alloc::vec![alloc::vec![0.0_f64; MXSHXSEC + 1]];
    for iz in 1..=100 {
        let (etmp, _) = photo.elements.get(iz).ok_or_else(|| alloc::format!("photo data has no element {iz}"))?;
        let ndat = etmp.len() - 1;
        let mut be = alloc::vec![0.0_f64; MXSHXSEC + 1];
        let mut k = 0usize;
        for j in (2..=ndat).rev() {
            if etmp[j] - etmp[j - 1] < 1e-5 {
                k += 1;
                if k <= MXSHXSEC {
                    be[k] = RealMath::r_exp(etmp[j]);
                } else {
                    return Err(alloc::format!("{k} binding energies read exceeding array size of {MXSHXSEC}"));
                }
                if !inp.eadl_relax && k >= 4 {
                    break;
                }
            }
        }
        binding_energies.push(be);
    }

    let nmed = media.len() - 1;
    let mut out = alloc::vec![UserPhotonMediumTables::default()];
    let nge = inp.mxge;
    for medium in 1..=nmed {
        let m = &media[medium];
        let mut t = UserPhotonMediumTables { mge: nge, ..Default::default() };
        t.ge1 = (nge - 1) as f64 / RealMath::r_ln(m.up / m.ap);
        t.ge0 = 1.0 - t.ge1 * RealMath::r_ln(m.ap);
        let (ge0, ge1) = (t.ge0, t.ge1);
        let (mut sum_z, mut sum_a) = (0.0, 0.0);
        for &(z, pz, wa) in &m.comp {
            sum_z += pz * z;
            sum_a += pz * wa;
        }
        let con2 = m.rho / (sum_a * 1.6605655);
        let mut order: Vec<usize> = (0..m.comp.len()).collect();
        order.sort_by(|&a, &b| m.comp[a].0.partial_cmp(&m.comp[b].0).unwrap_or(core::cmp::Ordering::Equal));
        let z_sorted: Vec<f64> = order.iter().map(|&i| m.comp[i].0).collect();
        let pz_sorted: Vec<f64> = order.iter().map(|&i| m.comp[i].1).collect();

        let sig_photo = user_xsec_data(&photo, 0, nge, &z_sorted, &pz_sorted, ge1, ge0, rm)?;
        let sig_rayleigh = user_xsec_data(&rayleigh, 0, nge, &z_sorted, &pz_sorted, ge1, ge0, rm)?;
        let sig_pair = user_xsec_data(&pair, 1, nge, &z_sorted, &pz_sorted, ge1, ge0, rm)?;
        let sig_triplet = user_xsec_data(&triplet, 2, nge, &z_sorted, &pz_sorted, ge1, ge0, rm)?;
        let sig_photonuc = match &photonuc {
            Some(tab) => Some(user_xsec_data(tab, 3, nge, &z_sorted, &pz_sorted, ge1, ge0, rm)?),
            None => None,
        };
        // the bound Compton cross section data
        let mut sig_compton: Option<Vec<f64>> = None;
        let (mut bc_emin, mut bc_emax, mut bc_dle) = (0.0, 0.0, 0.0);
        let mut bc_data: Vec<f64> = Vec::new();
        match &inp.compton {
            BoundComptonData::None => {}
            BoundComptonData::Custom(_) => {
                let tab = custom_compton.as_ref().ok_or_else(|| String::from("no Compton table"))?;
                sig_compton = Some(user_xsec_data(tab, 0, nge, &z_sorted, &pz_sorted, ge1, ge0, rm)?);
            }
            BoundComptonData::Ratio(text) => {
                let mut lines = text.lines();
                let h = read_record(&mut lines, 3)?;
                bc_emin = h[0];
                bc_emax = h[1];
                let bc_ne = h[2] as usize;
                if bc_ne > MXBCINP {
                    return Err(String::from("Number of input Compton data exceeds array size"));
                }
                bc_dle = RealMath::r_ln(bc_emax / bc_emin) / (bc_ne - 1) as f64;
                bc_data = alloc::vec![0.0; bc_ne + 2];
                let mut bc_tmp = alloc::vec![0.0; bc_ne];
                let mut iz_old = 1usize;
                for i in 0..z_sorted.len() {
                    let iz = (z_sorted[i] + 0.5) as usize;
                    for _ in iz_old..=iz {
                        bc_tmp = read_record(&mut lines, bc_ne)?;
                    }
                    for j in 1..=bc_ne {
                        bc_data[j] += pz_sorted[i] * z_sorted[i] * bc_tmp[j - 1];
                    }
                    iz_old = iz + 1;
                }
                for j in 1..=bc_ne {
                    bc_data[j] = RealMath::r_ln(bc_data[j] / sum_z);
                }
            }
        }
        if m.rayleigh {
            if let Some(ff) = &inp.form_factors {
                t.rayleigh = Some(init_rayleigh_sampling(&m.comp.iter().map(|c| (c.0, c.1)).collect::<Vec<_>>(), ff, ge0, ge1, nge, rm));
            }
        }

        let mut a0 = |v: &mut Vec<f64>| *v = alloc::vec![0.0; nge + 1];
        a0(&mut t.gmfp.a0);
        a0(&mut t.gmfp.a1);
        a0(&mut t.gbr1.a0);
        a0(&mut t.gbr1.a1);
        a0(&mut t.gbr2.a0);
        a0(&mut t.gbr2.a1);
        a0(&mut t.cohe.a0);
        a0(&mut t.cohe.a1);
        a0(&mut t.photonuc.a0);
        a0(&mut t.photonuc.a1);
        let (mut gmfp_old, mut gbr1_old, mut gbr2_old, mut cohe_old, mut photonuc_old) = (0.0, 0.0, 0.0, 0.0, 0.0);
        let (mut gmfp, mut gbr1, mut gbr2, mut cohe, mut photonuc, mut gle) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        for i in 1..=nge {
            gle = (i as f64 - ge0) / ge1;
            let e = RealMath::r_exp(gle);
            let mut sig_kn = sum_z * egs_kn_sigma0(e, rm);
            match &inp.compton {
                BoundComptonData::None => {}
                BoundComptonData::Custom(_) => {
                    sig_kn = sig_compton.as_ref().map_or(sig_kn, |v| v[i]);
                }
                BoundComptonData::Ratio(_) => {
                    let bcf;
                    if e <= bc_emin {
                        bcf = RealMath::r_exp(bc_data[1]);
                    } else if e < bc_emax {
                        let mut aj = 1.0 + RealMath::r_ln(e / bc_emin) / bc_dle;
                        let j = aj as usize;
                        aj -= j as f64;
                        bcf = RealMath::r_exp(bc_data[j] * (1.0 - aj) + bc_data[j + 1] * aj);
                    } else {
                        bcf = 1.0;
                    }
                    sig_kn *= bcf;
                }
            }
            let sig_p = sig_pair[i] + sig_triplet[i];
            let sigma = sig_kn + sig_p + sig_photo[i];
            gmfp = 1.0 / (sigma * con2);
            gbr1 = sig_p / sigma;
            gbr2 = gbr1 + sig_kn / sigma;
            cohe = sigma / (sig_rayleigh[i] + sigma);
            if let Some(sp) = &sig_photonuc {
                photonuc = sigma / (sp[i] + sigma);
            }
            if i > 1 {
                t.gmfp.a1[i - 1] = (gmfp - gmfp_old) * ge1;
                t.gmfp.a0[i - 1] = gmfp - t.gmfp.a1[i - 1] * gle;
                t.gbr1.a1[i - 1] = (gbr1 - gbr1_old) * ge1;
                t.gbr1.a0[i - 1] = gbr1 - t.gbr1.a1[i - 1] * gle;
                t.gbr2.a1[i - 1] = (gbr2 - gbr2_old) * ge1;
                t.gbr2.a0[i - 1] = gbr2 - t.gbr2.a1[i - 1] * gle;
                t.cohe.a1[i - 1] = (cohe - cohe_old) * ge1;
                t.cohe.a0[i - 1] = cohe - t.cohe.a1[i - 1] * gle;
                t.photonuc.a1[i - 1] = (photonuc - photonuc_old) * ge1;
                t.photonuc.a0[i - 1] = photonuc - t.photonuc.a1[i - 1] * gle;
            }
            gmfp_old = gmfp;
            gbr1_old = gbr1;
            gbr2_old = gbr2;
            cohe_old = cohe;
            photonuc_old = photonuc;
        }
        // the last interval repeats the slope; the intercept uses the last point
        t.gmfp.a1[nge] = t.gmfp.a1[nge - 1];
        t.gmfp.a0[nge] = gmfp - t.gmfp.a1[nge] * gle;
        t.gbr1.a1[nge] = t.gbr1.a1[nge - 1];
        t.gbr1.a0[nge] = gbr1 - t.gbr1.a1[nge] * gle;
        t.gbr2.a1[nge] = t.gbr2.a1[nge - 1];
        t.gbr2.a0[nge] = gbr2 - t.gbr2.a1[nge] * gle;
        t.cohe.a1[nge] = t.cohe.a1[nge - 1];
        t.cohe.a0[nge] = cohe - t.cohe.a1[nge] * gle;
        t.photonuc.a1[nge] = t.photonuc.a1[nge - 1];
        t.photonuc.a0[nge] = photonuc - t.photonuc.a1[nge] * gle;
        out.push(t);
    }
    Ok(UserPhotonTables { media: out, binding_energies })
}

/// `$MAX_EII_SHELLS` and `$N_EII_BINS` (`egsnrc.macros:596-598`).
pub const MAX_EII_SHELLS: usize = 40;
/// See [`MAX_EII_SHELLS`].
pub const N_EII_BINS: usize = 250;

/// `COMMON/eii_data/` (`egsnrc.macros:603-625`), filled by [`eii_init`]. All
/// shell and bin arrays are 1-based.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EiiData {
    /// `eii_xsection_a(1..MAX_EII_BINS)`.
    pub xsection_a: Vec<f64>,
    /// `eii_xsection_b`.
    pub xsection_b: Vec<f64>,
    /// `eii_cons(medium)`: `rho / (1.6605655 sum_a)`.
    pub cons: Vec<f64>,
    /// `eii_a(shell)`, `eii_b(shell)`: the energy grid of each shell.
    pub a: Vec<f64>,
    /// See `a`.
    pub b: Vec<f64>,
    /// `eii_z(shell)`.
    pub z: Vec<usize>,
    /// `eii_sh(shell)`: 1 K, 2 LI, 3 LII, 4 LIII.
    pub sh: Vec<usize>,
    /// `eii_nshells(Z)`.
    pub nshells: Vec<usize>,
    /// `eii_nsh(medium)`.
    pub nsh: Vec<usize>,
    /// `eii_first(medium, element)`.
    pub first: Vec<Vec<usize>>,
    /// `eii_no(medium, element)`.
    pub no: Vec<Vec<usize>>,
    /// `eii_flag`: `false` after `eii_init` found no EII shells (upstream turns
    /// itself off).
    pub flag: bool,
}

/// One medium for [`eii_init`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EiiMediumInput {
    /// `(ZELEM, PZ, WA)` per element.
    pub comp: Vec<(f64, f64, f64)>,
    /// `rho`.
    pub rho: f64,
    /// `AE`, total energy, MeV.
    pub ae: f64,
    /// `AP`, MeV.
    pub ap: f64,
}

/// How to read three places in `eii.mortran` where the source looks wrong
/// and only a built EGSnrc can say what is real (`docs/egsnrc_port_remaining.md`).
/// The default is the *intended* reading in all three; the literal one is
/// there for phase 2 to compare.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EiiReadings {
    /// `eii.mortran:176` writes `binding_energies(iZ, ish)` for an array
    /// declared `(shell, iZ)`. `true` reproduces that: the flat Fortran
    /// (column-major) address `(ish - 1) * MXSHELL + iZ - 1` is read.
    pub binding_energy_swapped: bool,
    /// `eii.mortran:227` uses `eii_a(jjj)`, `eii_b(jjj)` (shell type) where the
    /// shell index `jj` is meant. `true` reproduces the shell-type index.
    pub bin_index_by_shell_type: bool,
    /// `eii.mortran:215` reads `ap(medium)` (a COMMON variable, not the loop
    /// index `imed`) in `Ecc = min(Ec, ap(medium))`. `Some(ap)` uses that
    /// value; `None` uses the loop medium's own `AP` (the intended reading).
    pub ecc_ap: Option<f64>,
}

/// `eii_init` (`eii.mortran:51-305`): reads `eii_ik.data` (text), sets the EII
/// shells per element and medium, and rewrites the restricted stopping power
/// and discrete-interaction cross section of each medium with EII shells
/// (`media[m].ededx`, `esig`) so the Moller part of those shells is replaced
/// by the EII cross section. `relax` supplies the binding energies (upstream
/// calls `EDGSET` first for that). `rm` is `RM`.
///
/// Three places in the source look wrong; [`EiiReadings`] chooses the reading
/// (intended by default, literal on request).
///
/// # Errors
///
/// A message for short or malformed data, too many shells, a data file that
/// lacks shells the binding energies require, or a bin index outside the
/// table (which upstream would read out of bounds).
pub fn eii_init(
    text: &str,
    media_in: &[EiiMediumInput],
    relax: &RelaxData,
    media: &mut [ElectronMediumData],
    rm: f64,
    readings: EiiReadings,
) -> Result<EiiData, String> {
    const MXELEMENT: usize = 100;
    const CONS: f64 = 0.153536; // 2 pi Re^2 rm / u
    let nmed = media_in.len() - 1;
    let mut d = EiiData {
        nshells: alloc::vec![0; MXELEMENT + 1],
        nsh: alloc::vec![0; nmed + 1],
        cons: alloc::vec![0.0; nmed + 1],
        first: alloc::vec![Vec::new(); nmed + 1],
        no: alloc::vec![Vec::new(); nmed + 1],
        flag: true,
        ..Default::default()
    };
    // the minimum of all threshold energies
    let mut e_eii_min = 1e30_f64;
    for m in media_in.iter().skip(1) {
        e_eii_min = e_eii_min.min(m.ae - rm).min(m.ap);
    }
    // the elements that need EII data
    for m in media_in.iter().skip(1) {
        for &(z, _, _) in &m.comp {
            let iz = (z + 0.5) as usize;
            if d.nshells[iz] == 0 {
                let mut nsh = 0;
                for ish in 1..=4 {
                    if relax.binding_energies[iz][ish] > e_eii_min {
                        nsh += 1;
                    }
                }
                d.nshells[iz] = nsh;
            }
        }
    }
    let nsh_tot: usize = d.nshells.iter().sum();
    if nsh_tot == 0 {
        d.flag = false; // "EII requested but no shells ... => turning off EII"
        return Ok(d);
    }
    if nsh_tot > MAX_EII_SHELLS {
        return Err(alloc::format!("Number of shells with binding energies above threshold is {nsh_tot}; more than the {MAX_EII_SHELLS} allocated"));
    }
    let mut tmp_array = alloc::vec![0usize; MXELEMENT + 1];
    for j in 2..=MXELEMENT {
        tmp_array[j] = tmp_array[j - 1] + d.nshells[j - 1];
    }
    // the EII active shells per medium and element
    for imed in 1..=nmed {
        let mut nsh = 0;
        for &(z, _, _) in &media_in[imed].comp {
            let iz = (z + 0.5) as usize;
            d.no[imed].push(d.nshells[iz]);
            nsh += d.nshells[iz];
            d.first[imed].push(if d.nshells[iz] > 0 { tmp_array[iz] + 1 } else { 0 });
        }
        d.nsh[imed] = nsh;
    }
    // read the EII data
    let mut lines = text.lines();
    let nskip = read_record(&mut lines, 1)?[0] as usize;
    for _ in 0..nskip {
        lines.next().ok_or_else(|| String::from("I/O error while reading EII data"))?;
    }
    let h = read_record(&mut lines, 2)?;
    let (emax, nbin) = (h[0], h[1] as usize);
    if nbin != N_EII_BINS {
        return Err(String::from("Inconsistent EII data file"));
    }
    d.xsection_a = alloc::vec![0.0; N_EII_BINS * MAX_EII_SHELLS + 1];
    d.xsection_b = d.xsection_a.clone();
    d.a = alloc::vec![0.0; MAX_EII_SHELLS + 1];
    d.b = d.a.clone();
    d.z = alloc::vec![0; MAX_EII_SHELLS + 1];
    d.sh = alloc::vec![0; MAX_EII_SHELLS + 1];
    let mut ii = 0usize;
    for _ in 1..=MXELEMENT {
        let h = read_record(&mut lines, 2)?;
        let (iz, nsh) = (h[0] as usize, h[1] as usize);
        if nsh < d.nshells[iz] {
            return Err(alloc::format!("EII data file has data for {nsh} shells for element {iz}, but {} are required", d.nshells[iz]));
        }
        for ish in 1..=nsh {
            let fmax = read_record(&mut lines, 1)?[0];
            let aux_array = read_record(&mut lines, nbin)?;
            if ish <= d.nshells[iz] {
                ii += 1;
                d.z[ii] = iz;
                d.sh[ii] = ish;
                let be = if readings.binding_energy_swapped {
                    // the flat column-major address of binding_energies(iZ, ish)
                    let flat = (ish - 1) * MXSHELL + iz - 1;
                    let (zz, shell) = (flat / MXSHELL + 1, flat % MXSHELL + 1);
                    *relax.binding_energies.get(zz).and_then(|r| r.get(shell)).ok_or_else(|| String::from("swapped binding_energies address outside the table"))?
                } else {
                    relax.binding_energies[iz][ish]
                };
                d.a[ii] = nbin as f64 / RealMath::r_ln(emax / be);
                d.b[ii] = 1.0 - d.a[ii] * RealMath::r_ln(be);
                for k in 1..=nbin {
                    let sigo = if k > 1 { fmax * aux_array[k - 2] } else { 0.0 };
                    let loge = (k as f64 - d.b[ii]) / d.a[ii];
                    let iii = nbin * (ii - 1) + k;
                    d.xsection_a[iii] = (fmax * aux_array[k - 1] - sigo) * d.a[ii];
                    d.xsection_b[iii] = sigo - d.xsection_a[iii] * loge;
                }
            }
        }
        if ii == nsh_tot {
            break;
        }
    }
    // adjust the restricted stopping power and the discrete cross sections
    for imed in 1..=nmed {
        let mi = &media_in[imed];
        let ec = mi.ae - rm;
        let ecc = ec.min(readings.ecc_ap.unwrap_or(mi.ap));
        let mut sum_a = 0.0;
        for &(_, pz, wa) in &mi.comp {
            sum_a += pz * wa;
        }
        let con_med = mi.rho / 1.6605655 / sum_a;
        d.cons[imed] = con_med;
        if d.nsh[imed] > 0 {
            let m = &mut media[imed];
            let meke = m.esig.a0.len() - 1;
            let (eke0, eke1) = (m.eke0, m.eke1);
            let (mut dedx_old, mut sigm_old) = (0.0, 0.0);
            for j in 1..=meke {
                let loge = (j as f64 - eke0) / eke1;
                let e = RealMath::r_exp(loge);
                let tau = e / rm;
                let beta2 = tau * (tau + 2.0) / ((tau + 1.0) * (tau + 1.0));
                let p2 = 2.0 * rm * tau * (tau + 2.0);
                let mut dedx = m.ededx.eval(j, loge);
                let mut sigm = if e > 2.0 * ec {
                    let sig = m.esig.eval(j, loge);
                    let wbrem = m.ebr1.eval(j, loge);
                    sig * (1.0 - wbrem)
                } else {
                    0.0
                };
                let (mut sum_sigma, mut sum_dedx) = (0.0, 0.0);
                for (iele, &(z, pz, _)) in mi.comp.iter().enumerate() {
                    let iz = (z + 0.5) as usize;
                    for ish in 1..=d.no[imed][iele] {
                        let jj = d.first[imed][iele] + ish - 1;
                        let jjj = d.sh[jj];
                        let u = relax.binding_energies[iz][jjj];
                        let wmax = (e + u) / 2.0;
                        let uwm = u / wmax;
                        if u < e && u > ecc {
                            let uwm3 = uwm * uwm * uwm;
                            let ss_0 = 2.0 * (RealMath::r_ln(p2 / u) - uwm3 * RealMath::r_ln(p2 / wmax) - (beta2 + 0.833333) * (1.0 - uwm3)) / 3.0 / u;
                            let t1 = tau + 1.0;
                            let sh_0 = (1.0 - uwm
                                + uwm / (t1 * t1) * (tau * tau * (wmax - u) / (2.0 * wmax - u) - (tau + 0.5) * RealMath::r_ln((2.0 * wmax - u) / u)))
                                / u;
                            let ss_1 = RealMath::r_ln(p2 / u) - uwm * uwm * RealMath::r_ln(p2 / wmax) - (beta2 + 1.0) * (1.0 - uwm * uwm);
                            let sh_1 = RealMath::r_ln((2.0 * wmax - u) / u) + (tau / t1) * (tau / t1) * (wmax - u) / (wmax - u / 2.0);
                            let av_e = (ss_1 + sh_1) / (ss_0 + sh_0);
                            // eii_a/eii_b: shell index jj (intended) or shell type jjj (literal), per `readings`
                            let kb = if readings.bin_index_by_shell_type { jjj } else { jj };
                            let ib = d.a[kb] * loge + d.b[kb];
                            let i = (jj as i64 - 1) * N_EII_BINS as i64 + ib as i64;
                            if i < 1 || i as usize >= d.xsection_a.len() {
                                return Err(alloc::format!("EII cross-section bin {i} outside the table (shell type {jjj}, shell {jj})"));
                            }
                            let i = i as usize;
                            let sig = (d.xsection_a[i] * loge + d.xsection_b[i]) * pz * con_med;
                            sum_sigma += sig;
                            sum_dedx += sig * av_e;
                        }
                    }
                }
                sigm += sum_sigma;
                dedx -= sum_dedx;
                let aux = ec / e;
                if e > 2.0 * ec {
                    let t1 = tau + 1.0;
                    let r = tau / t1;
                    sigm -= CONS * mi.rho / (beta2 * ec)
                        * (1.0 - 2.0 * aux + r * r * aux * (1.0 - 2.0 * aux) / 2.0 + (2.0 * tau + 1.0) / (t1 * t1) * aux * RealMath::r_ln(2.0 * aux))
                        / sum_a;
                    dedx += CONS * mi.rho / beta2
                        * (-RealMath::r_ln(2.0 * aux) + r * r * (1.0 - 2.0 * aux) * (1.0 + 2.0 * aux) / 8.0 - (2.0 * tau + 1.0) / (t1 * t1) * (1.0 - 2.0 * aux) / 2.0)
                        / sum_a;
                }
                if j > 1 {
                    m.ededx.a1[j - 1] = (dedx - dedx_old) * eke1;
                    m.ededx.a0[j - 1] = dedx - m.ededx.a1[j - 1] * loge;
                    m.esig.a1[j - 1] = (sigm - sigm_old) * eke1;
                    m.esig.a0[j - 1] = sigm - m.esig.a1[j - 1] * loge;
                }
                dedx_old = dedx;
                sigm_old = sigm;
            }
            m.ededx.a1[meke] = m.ededx.a1[meke - 1];
            m.ededx.a0[meke] = m.ededx.a0[meke - 1];
            m.esig.a1[meke] = m.esig.a1[meke - 1];
            m.esig.a0[meke] = m.esig.a0[meke - 1];
        }
    }
    Ok(d)
}

/// `eii_sample` (`eii.mortran:308-369`): electron-impact ionisation of shell
/// `ish` of element `iz` (binding energy `uj`, MeV) by the electron on top of
/// the stack: the hard or soft collision is sampled, the primary keeps slot
/// `np`, the delta electron goes to `np + 1` when above `ae`, and the vacancy
/// relaxes. Returns the energy deposited locally (`edep`: the unemitted delta
/// energy plus what `RELAX` leaves). `ae` is the medium's `AE`; `ecut` and
/// `pcut` are the region's, for `RELAX`; `prm` is `PRM`.
///
/// # Errors
///
/// [`EgsError::StackExceeded`].
pub fn eii_sample<R: UniformSource>(
    stack: &mut Stack,
    relax_data: &RelaxData,
    ish: usize,
    iz: usize,
    uj: f64,
    ae: f64,
    ecut: f64,
    pcut: f64,
    prm: f64,
    rng: &mut R,
) -> Result<f64, EgsError> {
    let rmt2 = 2.0 * prm;
    let np = stack.np;
    let peie = stack.e[np];
    let t = peie - prm;
    let tau = t / prm;
    let tau1 = tau + 1.0;
    let tau12 = tau1 * tau1;
    let tau2 = tau * tau;
    let p2 = tau2 + 2.0 * tau;
    let beta2 = p2 / tau12;
    let c1 = tau2 / tau12;
    let c2 = (2.0 * tau + 1.0) / tau12;
    let wmax = 0.5 * (t + uj);
    let xmax = uj / wmax;
    let fm_s = RealMath::r_ln(rmt2 * p2 / uj) - beta2 - 0.5;
    let prob_s = 2.0 * fm_s * (1.0 + xmax + xmax * xmax);
    let mut fm_h = 1.0 + c1 - c2;
    if fm_h < 1.0 {
        fm_h = 1.0;
    }
    let prob = fm_h + prob_s;
    let mut wx;
    loop {
        let r1 = rng.uniform();
        let r2 = rng.uniform();
        let _r3 = rng.uniform();
        let frej;
        if r1 * prob < fm_h {
            // the hard collision cross section
            wx = 1.0 / (r2 * xmax + 1.0 - r2);
            let wxx = wx * xmax;
            let aux = wxx / (2.0 - wxx);
            frej = (1.0 + c1 * aux * aux - c2 * aux) / fm_h;
        } else {
            // the soft collision cross section
            wx = 1.0 / Real::sqrt((r2 * xmax) * (r2 * xmax) + (1.0 - r2) * (1.0 - r2));
            frej = 1.0 - RealMath::r_ln(wx) / fm_s;
        }
        if r2 < frej {
            break;
        }
    }
    wx *= uj;

    // set up the new particles
    let h1 = (peie + prm) / t;
    let pese1 = peie - prm;
    stack.e[np] = pese1;
    let mut dcosth = h1 * (pese1 - prm) / (pese1 + prm);
    if dcosth < 0.0 {
        // upstream takes `sqrt` of this and gets NaN directions; refuse instead
        return Err(EgsError::Fatal("eii_sample: upstream's primary energy peie - prm is below prm, cos^2 < 0"));
    }
    let mut sinthe = Real::sqrt(1.0 - dcosth);
    let mut costhe = Real::sqrt(dcosth);
    let mut saved = UphiSaved::default();
    uphi(stack, &mut saved, 2, 1, sinthe, costhe, rng)?;

    let pese2 = wx - uj + prm;
    let mut edep;
    if pese2 > ae {
        stack.check_stack(stack.np + 1, "eii_sample")?;
        stack.np += 1;
        let np2 = stack.np;
        stack.e[np2] = pese2;
        dcosth = h1 * (pese2 - prm) / (pese2 + prm);
        sinthe = -Real::sqrt(1.0 - dcosth);
        costhe = Real::sqrt(dcosth);
        stack.iq[np2] = -1;
        uphi(stack, &mut saved, 3, 2, sinthe, costhe, rng)?;
        edep = 0.0;
    } else {
        edep = wx - uj;
    }
    let r = relax(stack, relax_data, uj, ish as i32, iz, ecut, pcut, prm, rng)?;
    edep += r.edep;
    Ok(edep)
}

/// The EII selection that opens `MOLLER` (`egsnrc.mortran:2633-2661`): with
/// EII on and EII shells in the medium, the interaction is first tried against
/// each EII shell. Returns `Some(edep)` when an EII event was sampled by
/// [`eii_sample`] (the caller then does not run the Moller sampling), `None`
/// to carry on with Moller. `comp` is `(ZELEM, PZ)` per element of the
/// medium, `elke` and `lelke` the interval of `ln(E_kin)`, and `ekin`, `te`,
/// `ap` the energies of the interaction and the medium (MeV).
///
/// # Errors
///
/// [`EgsError::StackExceeded`], or [`EgsError::Fatal`] when a bin index falls
/// outside the table.
pub fn moller_eii_select<R: UniformSource>(
    stack: &mut Stack,
    eii: &EiiData,
    medium: usize,
    comp: &[(f64, f64)],
    emed: &ElectronMediumData,
    relax_data: &RelaxData,
    elke: f64,
    lelke: usize,
    region: (f64, f64),
    prm: f64,
    ap: f64,
    rng: &mut R,
) -> Result<Option<f64>, EgsError> {
    if !eii.flag || eii.nsh[medium] == 0 {
        return Ok(None);
    }
    let np = stack.np;
    stack.np_old = np;
    let ekin = stack.e[np] - prm;
    let mut sigm = emed.esig.eval(lelke, elke);
    let pbrem = emed.ebr1.eval(lelke, elke);
    sigm *= 1.0 - pbrem;
    let mut rsh = rng.uniform();
    rsh *= sigm;
    for (iele, &(z, pz)) in comp.iter().enumerate() {
        let iz = (z + 0.5) as usize;
        let nsh = eii.no[medium][iele];
        if nsh > 0 {
            let ifirst = eii.first[medium][iele];
            for ish in 1..=nsh {
                let uj = relax_data.binding_energies[iz][ish];
                if ekin > uj && (uj > emed.te || uj > ap) {
                    let jj = ifirst + ish - 1;
                    let i = (eii.a[jj] * elke + eii.b[jj] + ((jj - 1) * N_EII_BINS) as f64) as i64;
                    if i < 1 || i as usize >= eii.xsection_a.len() {
                        return Err(EgsError::Fatal("MOLLER/EII: cross-section bin outside the table"));
                    }
                    let i = i as usize;
                    let sig_j = (eii.xsection_a[i] * elke + eii.xsection_b[i]) * pz * eii.cons[medium];
                    rsh -= sig_j;
                    if rsh < 0.0 {
                        let ecut_pcut = region;
                        let edep = eii_sample(stack, relax_data, ish, iz, uj, emed.ae, ecut_pcut.0, ecut_pcut.1, prm, rng)?;
                        return Ok(Some(edep));
                    }
                }
            }
        }
    }
    Ok(None)
}

/// The direct-access file `pair_nrc1.data` read by [`init_nrc_pair`] (record
/// length `NRC_PAIR_NXX * $RECL-FACTOR` = 65 four-byte words = 260 bytes; the
/// factor is a build macro not in the clone, but the physical record size is
/// the data's own). Record 1 is `emin`, `emax` (`real*4`), `ne`, `nb`
/// (`integer*4`), one byte `endian` (0 means `1234`, little-endian, anything
/// else big-endian) and 243 bytes of padding; record `r` is 65 `real*4`.
/// **Not re-checked against a real `pair_nrc1.data`** (not in the clone).
#[derive(Debug, Clone, PartialEq)]
pub struct NrcPairFile {
    /// `emin`, `emax` (units of `m c^2`).
    pub emin: f32,
    /// See `emin`.
    pub emax: f32,
    /// `ne`: energy grid size.
    pub ne: i32,
    /// `nb`: `x` grid size.
    pub nb: i32,
    little: bool,
    bytes: Vec<u8>,
}

/// Bytes per record of `pair_nrc1.data`.
pub const NRC_PAIR_RECORD_BYTES: usize = NRC_PAIR_NXX * 4;

impl NrcPairFile {
    /// Decode the header of the file content.
    ///
    /// # Errors
    ///
    /// A message when the file is shorter than its header record.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < NRC_PAIR_RECORD_BYTES {
            return Err(String::from("pair_nrc1.data is shorter than its header record"));
        }
        let little = bytes[16] == 0;
        let f = |o: usize| {
            let b = [bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]];
            if little { f32::from_le_bytes(b) } else { f32::from_be_bytes(b) }
        };
        let i = |o: usize| {
            let b = [bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]];
            if little { i32::from_le_bytes(b) } else { i32::from_be_bytes(b) }
        };
        Ok(Self { emin: f(0), emax: f(4), ne: i(8), nb: i(12), little, bytes: bytes.to_vec() })
    }

    /// Direct-access record `irec` (1-based): its `NRC_PAIR_NXX` values, or
    /// `None` past the end of the file.
    pub fn record(&self, irec: usize) -> Option<Vec<f32>> {
        let o = irec.checked_sub(1)? * NRC_PAIR_RECORD_BYTES;
        let rec = self.bytes.get(o..o + NRC_PAIR_RECORD_BYTES)?;
        Some(
            rec.chunks_exact(4)
                .map(|c| {
                    let b = [c[0], c[1], c[2], c[3]];
                    if self.little { f32::from_le_bytes(b) } else { f32::from_be_bytes(b) }
                })
                .collect(),
        )
    }

    /// [`init_nrc_pair`] on this file.
    ///
    /// # Errors
    ///
    /// As [`init_nrc_pair`].
    pub fn init(&self, comp: &[Vec<(f64, f64)>]) -> Result<NrcPairData, String> {
        init_nrc_pair(self.emin, self.emax, self.ne, self.nb, comp, |r| self.record(r))
    }
}

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
/// With `iraylr = 1` a region needs the medium's `cohe` and `rayleigh` data
/// (else [`EgsError::Unsupported`]); the `AUSGAB` hooks are not ported; `$PARTICLE-SELECTION-*`
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

    let mut photonucfac = 1.0_f64;
    let mut cohfac = 1.0_f64;

    // :PNEWENERGY:
    'pnewenergy: loop {
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
                    if region.irayl == 1 && tables.media[medium].rayleigh.is_none() {
                        return Err(EgsError::Unsupported("PHOTON: Rayleigh scattering (iraylr = 1) without Rayleigh data"));
                    }
                    let rhof = region.rhor / tables.media[medium].rho;
                    gmfp = gmfpr0 / rhof;
                    // $RAYLEIGH-CORRECTION
                    if region.irayl == 1 {
                        cohfac = tables.media[medium].cohe.eval(lgle, gle);
                        gmfp *= cohfac;
                    }
                    // $PHOTONUC-CORRECTION
                    if region.iphotonucr == 1 {
                        photonucfac = tables.media[medium].photonuc.eval(lgle, gle);
                        gmfp *= photonucfac;
                    }
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
            // $RAYLEIGH-SCATTERING
            if region.irayl == 1 {
                let rnno37 = rng.uniform();
                if rnno37 <= 1.0 - cohfac {
                    stack.np_old = stack.np;
                    let mrec = &tables.media[medium];
                    let rd = mrec.rayleigh.as_ref().ok_or(EgsError::Unsupported("PHOTON: no Rayleigh data"))?;
                    let (costhe, sinthe) = rayleigh_sampling(rd, stack.e[stack.np], gle, lgle, rng);
                    let mut saved = UphiSaved::default();
                    uphi(stack, &mut saved, 2, 1, sinthe, costhe, rng)?;
                    continue 'pnewenergy;
                }
            }
            // $PHOTONUCLEAR
            if region.iphotonucr == 1 {
                let rnno39 = rng.uniform();
                if rnno39 <= 1.0 - photonucfac {
                    photonuc(stack);
                    continue 'pnewenergy;
                }
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
/// The `eadl_relax` variant is [`edgset_eadl_from_text`]. Not ported: the
/// "relaxations not requested" early return (the caller decides whether to
/// read at all).
///
/// # Errors
///
/// A message when the text is short or a value does not parse.
pub fn edgset_from_text(photo: &str, phocs: &str) -> Result<(RelaxData, PhotoData), String> {
    edgset_read(photo, phocs, false)
}

/// `EDGSET` with `eadl_relax = true` (`egsnrc.mortran:6355-6430`): as
/// [`edgset_from_text`] but the binding-energy block of `*_photo.data` is
/// skipped (they come from the photo-absorption edges, `binding_energies` as
/// [`init_user_photon`] built them, then [`init_compton_eadl`] for `xcom`),
/// and `egs_init_relax` ([`init_relax_eadl`]) loads `relax.data` into
/// [`RelaxData::eadl`]. The returned `binding_energies` are the final
/// (post-`egs_init_relax`) ones, and the mutated input is left in the same
/// state. For the `epdl` series call this *before* [`init_compton_eadl`]; for
/// `xcom` after (the order in `HATCH`, `egsnrc.mortran:2264-2274`).
///
/// # Errors
///
/// As [`edgset_from_text`] and [`init_relax_eadl`].
pub fn edgset_eadl_from_text(
    photo: &str,
    phocs: &str,
    relax_file: &EadlFile,
    media_z: &[Vec<f64>],
    binding_energies: &mut [Vec<f64>],
    epdl: bool,
) -> Result<(RelaxData, PhotoData), String> {
    let (mut rd, pd) = edgset_read(photo, phocs, true)?;
    let eadl = init_relax_eadl(relax_file, media_z, binding_energies, epdl)?;
    rd.binding_energies = binding_energies.to_vec();
    rd.eadl = Some(eadl);
    Ok((rd, pd))
}

fn edgset_read(photo: &str, phocs: &str, eadl: bool) -> Result<(RelaxData, PhotoData), String> {
    const NEL: usize = 100; // $MXELEMENT
    let mut lines = photo.lines();
    let mut rd = RelaxData {
        binding_energies: alloc::vec![alloc::vec![0.0; MXSHELL + 1]; NEL + 1],
        relaxation_prob: alloc::vec![alloc::vec![0.0; MXTRANS + 1]; NEL + 1],
        eadl: None,
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
        if eadl {
            // "Skip, using binding_energies from *_photo.data file" (:6376)
            lines.next().ok_or_else(|| String::from("data ends early"))?;
            continue;
        }
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
#[derive(Debug, Clone, Default, PartialEq)]
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
    /// `pcut(region)`, photon cut-off, MeV (used by `RELAX` after EII).
    pub pcut: f64,
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
    /// Electron-impact ionisation (`eii_flag > 0`), or `None` for off.
    pub eii: Option<ElectrEii>,
}

/// What `ELECTR` needs for electron-impact ionisation: the tables of
/// [`eii_init`], the `(ZELEM, PZ)` of each medium's elements (1-based by
/// medium) and the relaxation data `RELAX` uses after an ionisation.
#[derive(Debug, Clone, Default)]
pub struct ElectrEii {
    /// `COMMON/eii_data/`.
    pub data: EiiData,
    /// `comp[medium]` = `(ZELEM, PZ)` per element.
    pub comp: Vec<Vec<(f64, f64)>>,
    /// The relaxation data (binding energies and transition probabilities).
    pub relax: RelaxData,
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
    /// Energy deposited locally by electron-impact ionisation events
    /// (`edep` of [`eii_sample`], not part of `edep_steps`), MeV.
    pub edep_eii: f64,
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
                    // with EII on, interactions go down to the ionisation threshold,
                    // which may be below thmoll, so the Moller threshold is not applied
                    let eii_on = tables.eii.as_ref().is_some_and(|e| e.data.flag);
                    if stack.e[stack.np] <= m.thmoll && !eii_on {
                        // not enough energy for Moller: force bremsstrahlung if allowed
                        if ebr1 <= 0.0 {
                            continue 'newelectron;
                        }
                        do_brems = true;
                    }
                    if !do_brems {
                        let mut eii_done = false;
                        if let Some(ei) = &tables.eii {
                            let reg = &tables.regions[irl];
                            if let Some(edep) = moller_eii_select(
                                stack,
                                &ei.data,
                                medium,
                                &ei.comp[medium],
                                m,
                                &ei.relax,
                                elke,
                                lelke,
                                (reg.ecut, reg.pcut),
                                prm,
                                m.brems.ap,
                                rng,
                            )? {
                                res.edep_eii += edep;
                                eii_done = true;
                            }
                        }
                        if !eii_done {
                            moller(stack, m.te, m.thmoll, false, prm, rng)?;
                        }
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

/// One element of a PEGS4 medium (`ASYM`, `ZELEM`, `WA`, `PZ`, `RHOZ`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pegs4Element {
    /// Two-character symbol.
    pub asym: String,
    /// Atomic number.
    pub zelem: f64,
    /// Atomic weight.
    pub wa: f64,
    /// Number of atoms (relative).
    pub pz: f64,
    /// Partial density, g/cm^3.
    pub rhoz: f64,
}

/// The optional Rayleigh block of a PEGS4 medium (`IRAYL = 1`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pegs4Rayleigh {
    /// `NGR(IM)`.
    pub ngr: i32,
    /// `RCO0`, `RCO1`.
    pub rco: [f64; 2],
    /// `RSCT0(i)`, `RSCT1(i)` for `i = 1..NGR`.
    pub rsct: Vec<[f64; 2]>,
    /// `COHE0(i)`, `COHE1(i)` for `i = 1..NGE`.
    pub cohe: Vec<[f64; 2]>,
}

/// The data of one medium as `HATCH` reads it from a PEGS4 data set
/// (`egsnrc.mortran:2036-2165`), before `DUNIT` scaling. Arrays are
/// 0-based here; where a Mortran loop runs `i = 1..N` the entry for `i` is at
/// index `i - 1`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Pegs4Medium {
    /// The 24-character medium name as in the file.
    pub name: String,
    /// `RHO(IM)`, g/cm^3.
    pub rho: f64,
    /// `NNE(IM)`: number of elements.
    pub nne: usize,
    /// `IUNRST`, `EPSTFL`, `IAPRIM`.
    pub iunrst: i32,
    /// See `iunrst`.
    pub epstfl: i32,
    /// See `iunrst`.
    pub iaprim: i32,
    /// The elements.
    pub elements: Vec<Pegs4Element>,
    /// `RLC`: radiation length, cm.
    pub rlc: f64,
    /// `AE`, `AP`, `UE`, `UP`: thresholds and upper limits, MeV.
    pub ae: f64,
    /// See `ae`.
    pub ap: f64,
    /// See `ae`.
    pub ue: f64,
    /// See `ae`.
    pub up: f64,
    /// `MSGE, MGE, MSEKE, MEKE, MLEKE, MCMFP, MRANGE` (array sizes).
    pub sizes: [i32; 7],
    /// `IRAYL`: Rayleigh data present.
    pub irayl: i32,
    /// `DL1..DL6(I)`: `dl[k][i]` for `k = 0..5`, `i = 0..5`.
    pub dl: [[f64; 6]; 6],
    /// `DELCM`.
    pub delcm: f64,
    /// `ALPHI(I)`, `I = 1..2`.
    pub alphi: [f64; 2],
    /// `BPAR(I)`.
    pub bpar: [f64; 2],
    /// `DELPOS(I)`.
    pub delpos: [f64; 2],
    /// `XR0`, `TEFF0`, `BLCC`, `XCC`.
    pub xr0: f64,
    /// See `xr0`.
    pub teff0: f64,
    /// See `xr0`.
    pub blcc: f64,
    /// See `xr0`.
    pub xcc: f64,
    /// `EKE0`, `EKE1`.
    pub eke: [f64; 2],
    /// Per energy point `i = 1..NEKE`:
    /// `[ESIG0, ESIG1, PSIG0, PSIG1, EDEDX0, EDEDX1, PDEDX0, PDEDX1, EBR10, EBR11,
    /// PBR10, PBR11, PBR20, PBR21, TMXS0, TMXS1]`.
    pub elecin: Vec<[f64; 16]>,
    /// `EBINDA`.
    pub ebinda: f64,
    /// `GE0`, `GE1`.
    pub ge: [f64; 2],
    /// Per gamma energy point `i = 1..NGE`:
    /// `[GMFP0, GMFP1, GBR10, GBR11, GBR20, GBR21]`.
    pub photin: Vec<[f64; 6]>,
    /// The Rayleigh block, when `irayl == 1`.
    pub rayleigh: Option<Pegs4Rayleigh>,
}

/// A line padded with blanks to `n` characters (a Fortran `A1` read of a short
/// record gives blanks).
fn padded(line: &str, n: usize) -> Vec<char> {
    let mut v: Vec<char> = line.chars().collect();
    while v.len() < n {
        v.push(' ');
    }
    v
}

/// A fixed-width Fortran `Fw.d` / `Iw` field from columns `[start, start + w)`
/// of `chars`; a blank field is zero.
fn fixed_field(chars: &[char], start: usize, w: usize) -> Result<f64, String> {
    let f: String = chars.iter().skip(start).take(w).collect();
    let t = f.trim().replace(['D', 'd'], "e");
    if t.is_empty() {
        return Ok(0.0);
    }
    t.parse::<f64>().map_err(|_| alloc::format!("bad numeric field '{}'", f.trim()))
}

/// A cursor over the lines of a PEGS4 data set that reads Fortran-formatted
/// lists as `HATCH` does.
struct Pegs4Lines<'a> {
    lines: core::str::Lines<'a>,
}

impl<'a> Pegs4Lines<'a> {
    fn next_line(&mut self) -> Result<&'a str, String> {
        self.lines.next().ok_or_else(|| String::from("PEGS4 data ends early"))
    }

    /// `READ(KMPI, :FLT:)` of `n` values: format `(1X, 1PE14.5, 4E14.5)`, five
    /// 14-column values per record, a fresh record per five.
    fn flt(&mut self, n: usize) -> Result<Vec<f64>, String> {
        let mut out = Vec::with_capacity(n);
        while out.len() < n {
            let chars = padded(self.next_line()?, 71);
            for k in 0..5 {
                if out.len() == n {
                    break;
                }
                out.push(fixed_field(&chars, 1 + 14 * k, 14)?);
            }
        }
        Ok(out)
    }

    /// `READ(KMPI, :INT:)` of `n` values: format `(1X, 14I5)`.
    fn int(&mut self, n: usize) -> Result<Vec<i32>, String> {
        let mut out = Vec::with_capacity(n);
        while out.len() < n {
            let chars = padded(self.next_line()?, 71);
            for k in 0..14 {
                if out.len() == n {
                    break;
                }
                out.push(fixed_field(&chars, 1 + 5 * k, 5)? as i32);
            }
        }
        Ok(out)
    }
}

/// The composition header line: format 1 `(5A1,5X,F11.0,4X,I2,9X,I1,9X,I1,9X,I1)`
/// or, when a gas pressure is present, format 2 (`...,I2,26X,I1,9X,I1,9X,I1`),
/// tried when format 1 does not parse (`egsnrc.mortran:2092-2108`).
fn parse_medium_header(line: &str) -> Result<(f64, usize, i32, i32, i32), String> {
    let c = padded(line, 80);
    let rho = fixed_field(&c, 10, 11)?;
    let nne = fixed_field(&c, 25, 2)? as usize;
    let f1 = (|| -> Result<(i32, i32, i32), String> {
        Ok((fixed_field(&c, 36, 1)? as i32, fixed_field(&c, 46, 1)? as i32, fixed_field(&c, 56, 1)? as i32))
    })();
    if let Ok((a, b, d)) = f1 {
        // format 1 only holds if the text between the fields is the expected
        // label text (a gas-pressure line puts different text there)
        let label: String = c[27..36].iter().collect();
        if label.trim_start().starts_with(',') && label.contains("IUNRST") {
            return Ok((rho, nne, a, b, d));
        }
    }
    // format 2: 26 columns of gas pressure before IUNRST. 5 + 5 + 11 + 4 + 2 + 26
    // = 53 columns precede IUNRST, so its digit is column 54 (0-based index 53);
    // ~~indices 52, 62, 72~~ **CORRECTED 2026-11-21**: they were one column early
    // and only matched a test fixture that left out the comma after GASP's value
    // (found against the shipped `521icru.pegs4dat` by the pegs4 port).
    let a = fixed_field(&c, 53, 1)? as i32;
    let b = fixed_field(&c, 63, 1)? as i32;
    let d = fixed_field(&c, 73, 1)? as i32;
    Ok((rho, nne, a, b, d))
}

/// The regular PEGS4 intake of `HATCH` (`egsnrc.mortran:2036-2165`) on an
/// in-memory copy of a PEGS4 data set: for each name in `names` (padded to 24
/// characters as `MEDIA(IB,IM)` is), find its ` MEDIUM=` header and read its
/// data, returned in the order of `names`. Reading the file is the caller's
/// (the data sets are not vendored here).
///
/// # Errors
///
/// A message when a requested medium is not in the file (upstream stops with
/// the list of unrecognised names), or the data is short or malformed.
pub fn parse_pegs4_media(text: &str, names: &[&str]) -> Result<Vec<Pegs4Medium>, String> {
    const LMDL: usize = 8;
    const LMDN: usize = 24;
    let wanted: Vec<Vec<char>> = names.iter().map(|n| padded(n, LMDN)).collect();
    let mut found: Vec<Option<Pegs4Medium>> = alloc::vec![None; names.len()];
    let mut cur = Pegs4Lines { lines: text.lines() };
    let mut nfound = 0;
    while nfound < names.len() {
        // medium header search
        let mut im = None;
        loop {
            let line = match cur.lines.next() {
                Some(l) => l,
                None => {
                    let missing: Vec<&str> =
                        names.iter().zip(&found).filter(|(_, f)| f.is_none()).map(|(n, _)| *n).collect();
                    return Err(alloc::format!("PEGS4 media not recognised: {missing:?}"));
                }
            };
            let mbuf = padded(line, 72);
            let label: String = mbuf[..LMDL].iter().collect();
            if label != " MEDIUM=" {
                continue;
            }
            if let Some(i) = wanted.iter().position(|w| mbuf[LMDL..LMDL + LMDN] == w[..]) {
                im = Some(i);
                break;
            }
        }
        let im = im.unwrap_or(0);
        if found[im].is_some() {
            continue; // we already have this one
        }
        let mut med = Pegs4Medium { name: wanted[im].iter().collect(), ..Default::default() };
        let (rho, nne, iunrst, epstfl, iaprim) = parse_medium_header(cur.next_line()?)?;
        med.rho = rho;
        med.nne = nne;
        med.iunrst = iunrst;
        med.epstfl = epstfl;
        med.iaprim = iaprim;
        for _ in 0..nne {
            let c = padded(cur.next_line()?, 60);
            med.elements.push(Pegs4Element {
                asym: c[6..8].iter().collect(),
                zelem: fixed_field(&c, 11, 3)?,
                wa: fixed_field(&c, 17, 9)?,
                pz: fixed_field(&c, 30, 12)?,
                rhoz: fixed_field(&c, 48, 12)?,
            });
        }
        // MEDIA and THRESH
        let v = cur.flt(5)?;
        (med.rlc, med.ae, med.ap, med.ue, med.up) = (v[0], v[1], v[2], v[3], v[4]);
        // actual array sizes from PEGS
        let v = cur.int(8)?;
        med.sizes = [v[0], v[1], v[2], v[3], v[4], v[5], v[6]];
        med.irayl = v[7];
        let (nge, neke) = (med.sizes[1] as usize, med.sizes[3] as usize);
        // BREMPR
        let v = cur.flt(36)?;
        for i in 0..6 {
            for k in 0..6 {
                med.dl[k][i] = v[i * 6 + k];
            }
        }
        let v = cur.flt(7)?;
        med.delcm = v[0];
        for i in 0..2 {
            med.alphi[i] = v[1 + i * 3];
            med.bpar[i] = v[2 + i * 3];
            med.delpos[i] = v[3 + i * 3];
        }
        // ELECIN
        let v = cur.flt(4)?;
        (med.xr0, med.teff0, med.blcc, med.xcc) = (v[0], v[1], v[2], v[3]);
        let v = cur.flt(2)?;
        med.eke = [v[0], v[1]];
        let v = cur.flt(16 * neke)?;
        for i in 0..neke {
            let mut row = [0.0; 16];
            row.copy_from_slice(&v[16 * i..16 * i + 16]);
            med.elecin.push(row);
        }
        // PHOTIN
        let v = cur.flt(3)?;
        med.ebinda = v[0];
        med.ge = [v[1], v[2]];
        let v = cur.flt(6 * nge)?;
        for i in 0..nge {
            let mut row = [0.0; 6];
            row.copy_from_slice(&v[6 * i..6 * i + 6]);
            med.photin.push(row);
        }
        // optional Rayleigh input
        if med.irayl == 1 {
            let ngr = cur.int(1)?[0];
            let v = cur.flt(2)?;
            let rco = [v[0], v[1]];
            let v = cur.flt(2 * ngr as usize)?;
            let rsct: Vec<[f64; 2]> = (0..ngr as usize).map(|i| [v[2 * i], v[2 * i + 1]]).collect();
            let v = cur.flt(2 * nge)?;
            let cohe: Vec<[f64; 2]> = (0..nge).map(|i| [v[2 * i], v[2 * i + 1]]).collect();
            med.rayleigh = Some(Pegs4Rayleigh { ngr, rco, rsct, cohe });
        }
        found[im] = Some(med);
        nfound += 1;
    }
    Ok(found.into_iter().map(|m| m.expect("every requested medium was found")).collect())
}

impl Pegs4Medium {
    /// The distance-unit change of `HATCH` (`egsnrc.mortran:2204-2223`): PEGS4
    /// data is per radiation length; `dunit` (cm) is the unit EGSnrc will run
    /// in (the caller resolves a negative `DUNIT` to the radiation length of
    /// the medium it names). Cross sections and stopping powers scale by
    /// `DFACTI`, step limits and mean free paths by `DFACT`. Returns
    /// `RLDU = RLC / DUNIT`.
    pub fn apply_dunit(&mut self, dunit: f64) -> f64 {
        let dfact = self.rlc / dunit;
        let dfacti = 1.0 / dfact;
        for row in &mut self.elecin {
            for v in &mut row[0..8] {
                *v *= dfacti; // ESIG, PSIG, EDEDX, PDEDX (both coefficients)
            }
            row[14] *= dfact; // TMXS0
            row[15] *= dfact; // TMXS1
        }
        self.teff0 *= dfact;
        self.blcc *= dfacti;
        self.xcc *= Real::sqrt(dfacti);
        for row in &mut self.photin {
            row[0] *= dfact; // GMFP0
            row[1] *= dfact; // GMFP1
        }
        self.rlc / dunit
    }
}

/// `FCOULC` (`egsnrc.mortran:2413-2421`): the Coulomb correction function.
pub fn fcoulc(z: f64) -> f64 {
    const FINE: f64 = 137.03604;
    let mut asq = z / FINE;
    asq *= asq;
    asq * (1.0 / (1.0 + asq) + 0.20206 + asq * (-0.0369 + asq * (0.0083 + asq * (-0.002))))
}

/// `XSIF` (`egsnrc.mortran:2423-2437`): the ratio of the inelastic to the
/// elastic radiation logarithms.
pub fn xsif(z: f64) -> f64 {
    const ALRAD: [f64; 4] = [5.31, 4.79, 4.74, 4.71];
    const ALRADP: [f64; 4] = [6.144, 5.621, 5.805, 5.924];
    const A1440: f64 = 1194.0;
    const A183: f64 = 184.15;
    if z <= 4.0 {
        let iz = z as usize; // 1-based in upstream
        ALRADP[iz - 1] / (ALRAD[iz - 1] - fcoulc(z))
    } else {
        RealMath::r_ln(A1440 * RealMath::r_powf(z, -0.666667))
            / (RealMath::r_ln(A183 * RealMath::r_powf(z, -0.33333)) - fcoulc(z))
    }
}

/// The screening-function fits `fix_brems` computes for one medium.
#[derive(Debug, Clone, PartialEq)]
pub struct BremsFits {
    /// `log_ap(medium) = log(ap)`.
    pub log_ap: f64,
    /// `dl1..dl6(l)`, `l = 1..8` (index 0 unused): `dl[k - 1][l]`.
    pub dl: [Vec<f64>; 6],
    /// `bpar(1..2)` (index 0 unused).
    pub bpar: [f64; 3],
}

/// `fix_brems` (`egsnrc.mortran:2325-2410`): re-calculates the `dl1..dl6`
/// parameters for the bremsstrahlung and pair sampling used here (the old
/// EGS4 sampling had a bug that shows up when `AP` is not much smaller than the
/// electron kinetic energy). `elements` is `(ZELEM, PZ)` per element of the
/// medium, `ap` its photon threshold in MeV.
pub fn fix_brems(elements: &[(f64, f64)], ap: f64) -> BremsFits {
    let log_ap = RealMath::r_ln(ap);
    let (mut zt, mut zb, mut zf) = (0.0, 0.0, 0.0);
    for &(zi, pi) in elements {
        let fc = fcoulc(zi);
        let xi = xsif(zi);
        let aux = pi * zi * (zi + xi);
        zt += aux;
        zb -= aux * RealMath::r_ln(zi) / 3.0;
        zf += aux * fc;
    }
    let zv = (zb - zf) / zt;
    let zg = zb / zt;
    let fmax1 = 2.0 * (20.863 + 4.0 * zg) - 2.0 * (20.029 + 4.0 * zg) / 3.0;
    let fmax2 = 2.0 * (20.863 + 4.0 * zv) - 2.0 * (20.029 + 4.0 * zv) / 3.0;
    let mut dl: [Vec<f64>; 6] = core::array::from_fn(|_| alloc::vec![0.0; 9]);
    let mut set = |l: usize, v: [f64; 6]| {
        for k in 0..6 {
            dl[k][l] = v[k];
        }
    };
    // used in BREMS
    set(1, [(20.863 + 4.0 * zg) / fmax1, -3.242 / fmax1, 0.625 / fmax1, (21.12 + 4.0 * zg) / fmax1, -4.184 / fmax1, 0.952]);
    set(2, [(20.029 + 4.0 * zg) / fmax1, -1.93 / fmax1, -0.086 / fmax1, (21.12 + 4.0 * zg) / fmax1, -4.184 / fmax1, 0.952]);
    set(3, [(20.863 + 4.0 * zv) / fmax2, -3.242 / fmax2, 0.625 / fmax2, (21.12 + 4.0 * zv) / fmax2, -4.184 / fmax2, 0.952]);
    set(4, [(20.029 + 4.0 * zv) / fmax2, -1.93 / fmax2, -0.086 / fmax2, (21.12 + 4.0 * zv) / fmax2, -4.184 / fmax2, 0.952]);
    // and these in PAIR
    set(5, [3.0 * (20.863 + 4.0 * zg) - (20.029 + 4.0 * zg), 3.0 * (-3.242) - (-1.930), 3.0 * 0.625 - (-0.086), 2.0 * 21.12 + 8.0 * zg, 2.0 * (-4.184), 0.952]);
    set(6, [3.0 * (20.863 + 4.0 * zg) + (20.029 + 4.0 * zg), 3.0 * (-3.242) + (-1.930), 3.0 * 0.625 + (-0.086), 4.0 * 21.12 + 16.0 * zg, 4.0 * (-4.184), 0.952]);
    set(7, [3.0 * (20.863 + 4.0 * zv) - (20.029 + 4.0 * zv), 3.0 * (-3.242) - (-1.930), 3.0 * 0.625 - (-0.086), 2.0 * 21.12 + 8.0 * zv, 2.0 * (-4.184), 0.952]);
    set(8, [3.0 * (20.863 + 4.0 * zv) + (20.029 + 4.0 * zv), 3.0 * (-3.242) + (-1.930), 3.0 * 0.625 + (-0.086), 4.0 * 21.12 + 16.0 * zv, 4.0 * (-4.184), 0.952]);
    let mut bpar = [0.0; 3];
    bpar[2] = dl[0][7] / (3.0 * dl[0][8] + dl[0][7]);
    bpar[1] = 12.0 * dl[0][8] / (3.0 * dl[0][8] + dl[0][7]);
    BremsFits { log_ap, dl, bpar }
}

/// `$MXTOTSH` (`egsnrc.macros:444`): shells for Z = 1..100.
pub const MXTOTSH: usize = 1538;
/// `$MXMDSH` (`egsnrc.macros:445`): maximum shells per medium.
pub const MXMDSH: usize = 200;

/// `init_compton` (`egsnrc.mortran:2422-2575`), the non-EADL path: the bound
/// Compton data from an in-memory copy of `incoh.data` (skip 18 comment lines,
/// then `MXTOTSH` records `iz shn ne Jo be` with `Jo` in units of 1/137 and
/// `be` in eV), and the per-medium alias tables `COMPT` samples shells from.
/// `media[m]` is `(ZELEM, PZ)` for each element of medium `m + 1`; `prm` is
/// `PRM` (MeV). The `ComptonData` arrays are 1-based, the per-medium ones
/// indexed `[medium]` (index 0 unused).
///
/// The `eadl_relax` variant (shell numbers and binding energies replaced by
/// the photoelectric data's) is [`init_compton_eadl`]. Not ported: the
/// radiative-Compton data
/// (`$RADC_HATCH`, empty by default), and the trailing check that relaxations
/// are on (the caller's).
///
/// # Errors
///
/// A message when the data is short, a medium has no shells, or has more than
/// `MXMDSH` (upstream stops).
pub fn init_compton(incoh_text: &str, media: &[Vec<(f64, f64)>], prm: f64) -> Result<ComptonData, String> {
    let mut lines = incoh_text.lines();
    for _ in 0..18 {
        lines.next().ok_or_else(|| String::from("incoh data ends early"))?;
    }
    let mut d = ComptonData {
        iz_array: alloc::vec![0; MXTOTSH + 1],
        be_array: alloc::vec![0.0; MXTOTSH + 1],
        jo_array: alloc::vec![0.0; MXTOTSH + 1],
        erf_jo_array: alloc::vec![0.0; MXTOTSH + 1],
        shn_array: alloc::vec![0; MXTOTSH + 1],
        ne_array: alloc::vec![0; MXTOTSH + 1],
        shell_array: alloc::vec![Vec::new(); media.len() + 1],
        eno_array: alloc::vec![Vec::new(); media.len() + 1],
        eno_atbin_array: alloc::vec![Vec::new(); media.len() + 1],
        n_shell: alloc::vec![0; media.len() + 1],
    };
    for j in 1..=MXTOTSH {
        let r = read_record(&mut lines, 5)?;
        d.iz_array[j] = r[0] as i32;
        d.shn_array[j] = r[1] as i32;
        d.ne_array[j] = r[2] as i32;
        d.jo_array[j] = r[3] * 137.0;
        d.be_array[j] = r[4] * 1e-6 / prm;
        let aux_erf = 0.70710678119 * (1.0 + 0.3 * d.jo_array[j]);
        d.erf_jo_array[j] = 0.82436063535 * (super::egs_utilities_mortran::erf1(aux_erf) - 1.0);
    }
    for (m0, comp) in media.iter().enumerate() {
        let medium = m0 + 1;
        let mut pztot = 0.0;
        let mut shell = alloc::vec![0i32; MXMDSH + 2];
        let mut eno = alloc::vec![0.0_f64; MXMDSH + 2];
        let mut atbin = alloc::vec![0i32; MXMDSH + 2];
        let mut nsh = 0usize;
        for &(zelem, pz) in comp {
            let iz = zelem as i32;
            for j in 1..=MXTOTSH {
                if iz == d.iz_array[j] {
                    nsh += 1;
                    if nsh > MXMDSH {
                        return Err(alloc::format!("medium {medium}: more than {MXMDSH} shells"));
                    }
                    shell[nsh] = j as i32;
                    let aux = pz * d.ne_array[j] as f64;
                    eno[nsh] = aux;
                    pztot += aux;
                }
            }
        }
        if nsh == 0 {
            return Err(alloc::format!("medium {medium} has zero shells"));
        }
        d.n_shell[medium] = nsh as i32;
        for i in 1..=nsh {
            eno[i] /= pztot;
            eno[i] = -eno[i];
            atbin[i] = i as i32;
        }
        let atav = 1.0 / nsh as f64;
        for i in 1..=nsh.saturating_sub(1) {
            // Mortran DO loops leave the index at `nsh` when they run out
            let mut j_h = nsh;
            for jh in 1..nsh {
                if eno[jh] < 0.0 && eno[jh].abs() > atav {
                    j_h = jh;
                    break;
                }
            }
            let mut j_l = nsh;
            for jl in 1..nsh {
                if eno[jl] < 0.0 && eno[jl].abs() < atav {
                    j_l = jl;
                    break;
                }
            }
            let aux = atav - eno[j_l].abs();
            eno[j_h] += aux;
            eno[j_l] = -eno[j_l] / atav + j_l as f64;
            atbin[j_l] = j_h as i32;
            if i == nsh - 1 {
                eno[j_h] = 1.0 + j_h as f64;
            }
        }
        for i in 1..=nsh {
            if eno[i] < 0.0 {
                eno[i] = 1.0 + i as f64;
            }
        }
        // store 1-based, trimmed to nsh
        d.shell_array[medium] = shell[..=nsh].to_vec();
        d.eno_array[medium] = eno[..=nsh].to_vec();
        d.eno_atbin_array[medium] = atbin[..=nsh].to_vec();
    }
    Ok(d)
}

/// `init_compton` with `eadl_relax = true` (`egsnrc.mortran:2490-2506`): the
/// data of [`init_compton`], then, shell by shell in file order, the shell
/// type `shn_array(j)` is replaced by the actual shell number within the
/// element (1 for the first shell of each `Z`, then counting up), and the
/// binding energy by the photo-absorption edge `binding_energies[iz][shn]`
/// (MeV, [`init_user_photon`]'s output) when it is positive; otherwise, for
/// the `xcom` series, the file's binding energy is written *into*
/// `binding_energies` (`egsnrc.mortran:2500-2503`; the shells below 1 keV
/// that have no edge). The alias tables do not depend on either, so applying
/// the replacement after [`init_compton`] is the same as upstream's
/// in-loop one.
///
/// Upstream's fatal when bound Compton is off and `eadl_relax` is on with
/// `xcom` (`:2467-2472`) is the caller's, as is the call order (see
/// [`edgset_eadl_from_text`]).
///
/// # Errors
///
/// As [`init_compton`], or a message when a shell number exceeds
/// `binding_energies`.
pub fn init_compton_eadl(
    incoh_text: &str,
    media: &[Vec<(f64, f64)>],
    prm: f64,
    binding_energies: &mut [Vec<f64>],
    xcom: bool,
) -> Result<ComptonData, String> {
    let mut d = init_compton(incoh_text, media, prm)?;
    let mut iz = 0;
    for j in 1..=MXTOTSH {
        if d.iz_array[j] != iz {
            d.shn_array[j] = 1;
            iz = d.iz_array[j];
        } else {
            d.shn_array[j] = d.shn_array[j - 1] + 1;
        }
        let slot = binding_energies
            .get_mut(iz as usize)
            .and_then(|r| r.get_mut(d.shn_array[j] as usize))
            .ok_or_else(|| alloc::format!("binding_energies has no shell {} of Z = {iz}", d.shn_array[j]))?;
        if *slot > 0.0 {
            d.be_array[j] = *slot / prm;
        } else if xcom {
            *slot = d.be_array[j] * prm;
        }
    }
    Ok(d)
}

/// `$MAX-ELOSS` (`egsnrc.macros:2265`).
pub const MAX_ELOSS: f64 = 0.25;
/// `$EXACT-BCA-XIMAX`, `$INEXACT-BCA-XIMAX` (`egsnrc.macros:2260-2262`).
pub const BCA_XIMAX: f64 = 0.5;
/// `$SKIN-DEPTH-FOR-BCA` (`egsnrc.macros:2256`).
pub const SKIN_DEPTH_FOR_BCA: f64 = 3.0;

/// The step-size settings `mscati` resolves from the user's inputs
/// (`egsnrc.mortran:2775-2810`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MscatiSettings {
    /// `exact_bca`.
    pub exact_bca: bool,
    /// `estepe`: maximum fractional energy loss per step.
    pub estepe: f64,
    /// `ximax`: maximum first multiple-scattering moment per step.
    pub ximax: f64,
    /// `skindepth_for_bca`, in elastic MFPs.
    pub skindepth_for_bca: f64,
}

impl MscatiSettings {
    /// The defaults block of `mscati`: `exact_bca = (bca_algorithm == 0)`,
    /// `estepe` and `ximax` outside `(0, 1)` become their defaults, and a
    /// `skindepth_for_bca <= 1e-4` becomes the PRESTA-I default (from the
    /// smallest `ECUT` and each medium's `blcc` and `xcc`, as read from PEGS4,
    /// before `mscati` rescales them) when the BCA is not exact, else
    /// [`SKIN_DEPTH_FOR_BCA`]. `media` is `(blcc, xcc)` per medium.
    pub fn resolve(bca_algorithm: i32, estepe: f64, ximax: f64, skindepth_for_bca: f64, ecutmn: f64, prm: f64, media: &[(f64, f64)]) -> Self {
        let exact_bca = bca_algorithm == 0;
        let estepe = if estepe <= 0.0 || estepe >= 1.0 { MAX_ELOSS } else { estepe };
        let ximax = if ximax <= 0.0 || ximax >= 1.0 { BCA_XIMAX } else { ximax };
        let mut skin = skindepth_for_bca;
        if skin <= 1e-4 {
            if !exact_bca {
                let mut tstbmn = 1e30_f64;
                for &(blcc, xcc) in media {
                    let mut tstbm = (ecutmn - prm) * (ecutmn + prm) / (ecutmn * ecutmn);
                    let r = ecutmn / xcc;
                    tstbm = blcc * tstbm * r * r;
                    let aux = RealMath::r_ln(tstbm);
                    tstbm = RealMath::r_ln(tstbm / aux);
                    tstbmn = tstbmn.min(tstbm);
                }
                skin = RealMath::r_exp(tstbmn);
            } else {
                skin = SKIN_DEPTH_FOR_BCA;
            }
        }
        Self { exact_bca, estepe, ximax, skindepth_for_bca: skin }
    }
}

/// The stopping-power and cross-section fits `mscati` reads for one medium
/// (`COMMON/ELECIN/`), straight from PEGS4 after the `DUNIT` scaling.
#[derive(Debug, Clone, Default)]
pub struct MscatiMedium {
    /// `eke0`, `eke1`.
    pub eke0: f64,
    /// See `eke0`.
    pub eke1: f64,
    /// `meke(medium)`: number of table energies.
    pub neke: usize,
    /// `ededx`, `pdedx`, `esig`, `psig` fits (1-based over the `neke` bins).
    pub ededx: Pwlf,
    /// See `ededx`.
    pub pdedx: Pwlf,
    /// See `ededx`.
    pub esig: Pwlf,
    /// See `ededx`.
    pub psig: Pwlf,
    /// `blcc(medium)` as read.
    pub blcc: f64,
    /// `xcc(medium)` as read.
    pub xcc: f64,
}

/// What `mscati` computes for one medium.
#[derive(Debug, Clone, PartialEq)]
pub struct MscatiTables {
    /// `blcc(medium)` with the Euler constant absorbed: `1.16699413758864573 * blcc`.
    pub blcc: f64,
    /// `xcc(medium)` squared, as `ELECTR` uses it.
    pub xcc: f64,
    /// `E_array(i, medium)`, 1-based.
    pub e_array: Vec<f64>,
    /// `expeke1(medium) = exp(1/eke1) - 1`.
    pub expeke1: f64,
    /// `range_ep(qel, i, medium)`: `[qel][i]`, 1-based; `qel` 0 is e-, 1 is e+.
    pub range_ep: [Vec<f64>; 2],
    /// `tmxs0`, `tmxs1`: the maximum multiple-scattering step.
    pub tmxs: Pwlf,
    /// `esig_e(medium)`, `psig_e(medium)`: maximum cross section per energy loss.
    pub esig_e: f64,
    /// See `esig_e`.
    pub psig_e: f64,
    /// `sig_ismonotone(0..1, medium)`.
    pub sig_ismonotone: [bool; 2],
}

/// `mscati` (`egsnrc.mortran:2749-2975`) without `init_spin` and the MS-table
/// read (`init_ms_SR`, see [`MsData::from_text`]): for each medium, absorb
/// the Euler constant into `blcc`, square `xcc`, find the maximum cross section
/// per energy loss (and whether it is monotone), build the range-to-threshold
/// arrays `range_ep` and the PRESTA step-size limit `tmxs`. Returns the
/// tables per medium and `(esige_max, psige_max)`.
///
/// `rm` is `RM`.
pub fn mscati(media: &[MscatiMedium], settings: &MscatiSettings, rm: f64) -> (Vec<MscatiTables>, f64, f64) {
    let rmt2 = 2.0 * rm;
    let rmsq = rm * rm;
    let (mut esige_max, mut psige_max) = (0.0_f64, 0.0_f64);
    let mut out = Vec::with_capacity(media.len());
    for m in media {
        let neke = m.neke;
        let blcc = 1.16699413758864573 * m.blcc;
        let xcc = m.xcc * m.xcc;
        let (eke0, eke1) = (m.eke0, m.eke1);

        // maximum cross section per energy loss
        let (mut sigee, mut sigep) = (1e-15_f64, 1e-15_f64);
        let (mut ise_monoton, mut isp_monoton) = (true, true);
        let (mut sige_old, mut sigp_old) = (-1.0_f64, -1.0_f64);
        for i in 1..=neke {
            let ei = RealMath::r_exp((i as f64 - eke0) / eke1);
            let eil = RealMath::r_ln(ei);
            let sig = m.esig.eval(i, eil) / m.ededx.eval(i, eil);
            if sig > sigee {
                sigee = sig;
            }
            if sig < sige_old {
                ise_monoton = false;
            }
            sige_old = sig;
            let sig = m.psig.eval(i, eil) / m.pdedx.eval(i, eil);
            if sig > sigep {
                sigep = sig;
            }
            if sig < sigp_old {
                isp_monoton = false;
            }
            sigp_old = sig;
        }
        if sigee > esige_max {
            esige_max = sigee;
        }
        if sigep > psige_max {
            psige_max = sigep;
        }

        // range array
        let mut e_array = alloc::vec![0.0; neke + 2];
        let mut range_ep = [alloc::vec![0.0; neke + 2], alloc::vec![0.0; neke + 2]];
        let mut ei = RealMath::r_exp((1.0 - eke0) / eke1);
        e_array[1] = ei;
        let expeke1 = RealMath::r_exp(1.0 / eke1) - 1.0;
        for i in 1..neke {
            let eip1 = RealMath::r_exp(((i + 1) as f64 - eke0) / eke1);
            e_array[i + 1] = eip1;
            let eke = 0.5 * (eip1 + ei);
            let elke = RealMath::r_ln(eke);
            let lelke = (eke1 * elke + eke0) as usize;
            let d = eip1 - ei;
            let ededx = m.pdedx.eval(lelke, elke);
            let aux = m.pdedx.a1[i] / ededx;
            range_ep[1][i + 1] = range_ep[1][i] + d / ededx * (1.0 + aux * (1.0 + 2.0 * aux) * (d / eke) * (d / eke) / 24.0);
            let ededx = m.ededx.eval(lelke, elke);
            let aux = m.ededx.a1[i] / ededx;
            range_ep[0][i + 1] = range_ep[0][i] + d / ededx * (1.0 + aux * (1.0 + 2.0 * aux) * (d / eke) * (d / eke) / 24.0);
            ei = eip1;
        }

        // tmxs
        let mut tmxs0 = alloc::vec![0.0; neke + 2];
        let mut tmxs1 = alloc::vec![0.0; neke + 2];
        let eil = (1.0 - eke0) / eke1;
        let ei = RealMath::r_exp(eil);
        let p2 = ei * (ei + rmt2);
        let beta2 = p2 / (p2 + rmsq);
        let chia2 = xcc / (4.0 * p2 * blcc);
        let dedx0 = m.ededx.eval(1, eil);
        let mut estepx = 2.0 * p2 * beta2 * dedx0 / ei / xcc / (RealMath::r_ln(1.0 + 1.0 / chia2) * (1.0 + chia2) - 1.0);
        estepx *= settings.ximax;
        if estepx > settings.estepe {
            estepx = settings.estepe;
        }
        let mut si = estepx * ei / dedx0;
        for i in 1..neke {
            let elke = ((i + 1) as f64 - eke0) / eke1;
            let eke = RealMath::r_exp(elke);
            let lelke = i + 1;
            let p2 = eke * (eke + rmt2);
            let beta2 = p2 / (p2 + rmsq);
            let chia2 = xcc / (4.0 * p2 * blcc);
            let ededx = m.ededx.eval(lelke, elke);
            let mut estepx = 2.0 * p2 * beta2 * ededx / eke / xcc / (RealMath::r_ln(1.0 + 1.0 / chia2) * (1.0 + chia2) - 1.0);
            estepx *= settings.ximax;
            if estepx > settings.estepe {
                estepx = settings.estepe;
            }
            let mut ekef = (1.0 - estepx) * eke;
            let mut sip1;
            let lelkef;
            if ekef <= e_array[1] {
                sip1 = (e_array[1] - ekef) / dedx0;
                ekef = e_array[1];
                let _ = ekef;
                lelkef = 1usize;
            } else {
                let elkef = RealMath::r_ln(ekef);
                lelkef = (eke1 * elkef + eke0) as usize;
                let leip1l = lelkef + 1;
                let eip1l = (leip1l as f64 - eke0) / eke1;
                let eip1 = e_array[leip1l];
                let mut aux = (eip1 - ekef) / eip1;
                let elktmp = 0.5 * (elkef + eip1l + 0.25 * aux * aux * (1.0 + aux * (1.0 + 0.875 * aux)));
                let ektmp = 0.5 * (ekef + eip1);
                let lelktmp = lelkef;
                let ededx = m.ededx.eval(lelktmp, elktmp);
                aux = m.ededx.a1[lelktmp] / ededx;
                let dd = eip1 - ekef;
                sip1 = dd / ededx * (1.0 + aux * (1.0 + 2.0 * aux) * (dd / ektmp) * (dd / ektmp) / 24.0);
            }
            sip1 += range_ep[0][lelke] - range_ep[0][lelkef + 1];
            tmxs1[i] = (sip1 - si) * eke1;
            tmxs0[i] = sip1 - tmxs1[i] * elke;
            si = sip1;
        }
        if neke >= 2 {
            tmxs0[neke] = tmxs0[neke - 1];
            tmxs1[neke] = tmxs1[neke - 1];
        }
        out.push(MscatiTables {
            blcc,
            xcc,
            e_array,
            expeke1,
            range_ep,
            tmxs: Pwlf { a0: tmxs0, a1: tmxs1 },
            esig_e: sigee,
            psig_e: sigep,
            sig_ismonotone: [ise_monoton, isp_monoton],
        });
    }
    (out, esige_max, psige_max)
}

/// What `hatch_media` needs besides the PEGS4 data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HatchConfig {
    /// `ibrdst` (bremsstrahlung angular distribution), for `ZBRANG`.
    pub ibrdst: i32,
    /// `iprdst` (pair angular distribution), for `ZBRANG`.
    pub iprdst: i32,
    /// `bca_algorithm` (0 exact), with `estepe`, `ximax`, `skindepth_for_bca`
    /// as for [`MscatiSettings::resolve`].
    pub bca_algorithm: i32,
    /// See [`MscatiSettings`].
    pub estepe: f64,
    /// See [`MscatiSettings`].
    pub ximax: f64,
    /// See [`MscatiSettings`].
    pub skindepth_for_bca: f64,
    /// `$set_ecutmn`: the smallest `ECUT` over the regions, MeV.
    pub ecutmn: f64,
    /// `PRM`, MeV.
    pub prm: f64,
}

impl Default for HatchConfig {
    fn default() -> Self {
        Self {
            ibrdst: 1,
            iprdst: 1,
            bca_algorithm: 0,
            estepe: 0.0,
            ximax: 0.0,
            skindepth_for_bca: 0.0,
            ecutmn: RM + 0.01,
            prm: RM,
        }
    }
}

/// One medium after `HATCH`: the tables the transport routines take.
#[derive(Debug, Clone, Default)]
pub struct HatchedMedium {
    /// For `ELECTR`, `MOLLER`, `BHABHA`, `BREMS`.
    pub electron: ElectronMediumData,
    /// For `PHOTON`, `PAIR`, `PHOTO`.
    pub photon: PhotonMediumData,
}

/// The result of [`hatch_media`].
#[derive(Debug, Clone)]
pub struct HatchResult {
    /// One entry per medium, in the order given (medium `m + 1` is index `m`).
    pub media: Vec<HatchedMedium>,
    /// The resolved step-size settings.
    pub settings: MscatiSettings,
    /// `esige_max`, `psige_max`.
    pub sige_max: (f64, f64),
}

/// A 1-based [`Pwlf`] from PEGS4 rows (`a0` at column `c0`, `a1` at `c1`).
fn pwlf_from_rows<const N: usize>(rows: &[[f64; N]], c0: usize, c1: usize) -> Pwlf {
    let mut a0 = alloc::vec![0.0];
    let mut a1 = alloc::vec![0.0];
    for r in rows {
        a0.push(r[c0]);
        a1.push(r[c1]);
    }
    Pwlf { a0, a1 }
}

/// The table assembly of `HATCH` for media already read ([`parse_pegs4_media`]
/// and scaled ([`Pegs4Medium::apply_dunit`])): `ZBRANG` and `LZBRANG`
/// (`$INITIALIZE-BREMS-ANGLE`, `$INITIALIZE-PAIR-ANGLE`), `TE` and `THMOLL`,
/// [`fix_brems`], [`mscati`], and the move of the PEGS4 columns into
/// [`ElectronMediumData`] and [`PhotonMediumData`].
///
/// Not assembled here (their data come from other routines, not ported):
/// the spin-effect fits of `MsMedium` (`init_spin`; left empty, so
/// `spin_effects` must be off), the Rayleigh data, the per-medium
/// [`ComptonData`] ([`init_compton`]) and the photoelectric edge data
/// ([`edgset_from_text`]), which are global and are passed to the transport
/// routines beside these tables.
pub fn hatch_media(pegs4: &[Pegs4Medium], cfg: &HatchConfig) -> HatchResult {
    let raw: Vec<(f64, f64)> = pegs4.iter().map(|m| (m.blcc, m.xcc)).collect();
    let settings = MscatiSettings::resolve(
        cfg.bca_algorithm,
        cfg.estepe,
        cfg.ximax,
        cfg.skindepth_for_bca,
        cfg.ecutmn,
        cfg.prm,
        &raw,
    );
    let mmedia: Vec<MscatiMedium> = pegs4
        .iter()
        .map(|m| MscatiMedium {
            eke0: m.eke[0],
            eke1: m.eke[1],
            neke: m.sizes[3] as usize,
            ededx: pwlf_from_rows(&m.elecin, 4, 5),
            pdedx: pwlf_from_rows(&m.elecin, 6, 7),
            esig: pwlf_from_rows(&m.elecin, 0, 1),
            psig: pwlf_from_rows(&m.elecin, 2, 3),
            blcc: m.blcc,
            xcc: m.xcc,
        })
        .collect();
    let (tabs, esige_max, psige_max) = mscati(&mmedia, &settings, RM);
    let mut media = Vec::with_capacity(pegs4.len());
    for (m, t) in pegs4.iter().zip(tabs) {
        // ZBRANG, LZBRANG (set when either angular distribution asks for it)
        let mut zbrang = 0.0;
        let mut lzbrang = 0.0;
        if cfg.ibrdst == 1 || cfg.iprdst > 0 {
            let mut pznorm = 0.0;
            for e in &m.elements {
                zbrang += e.pz * e.zelem * (e.zelem + 1.0);
                pznorm += e.pz;
            }
            zbrang = 8.116224e-05 * RealMath::r_powf(zbrang / pznorm, 1.0 / 3.0);
            lzbrang = -RealMath::r_ln(zbrang);
        }
        let comp: Vec<(f64, f64)> = m.elements.iter().map(|e| (e.zelem, e.pz)).collect();
        let fits = fix_brems(&comp, m.ap);
        let te = m.ae - RM;
        let pair = PairMedium { delcm: m.delcm, dl: fits.dl.clone(), bpar: fits.bpar, zbrang };
        let electron = ElectronMediumData {
            rho: m.rho,
            eke0: m.eke[0],
            eke1: m.eke[1],
            e_array: t.e_array,
            range_ep: t.range_ep,
            ededx: pwlf_from_rows(&m.elecin, 4, 5),
            pdedx: pwlf_from_rows(&m.elecin, 6, 7),
            esig: pwlf_from_rows(&m.elecin, 0, 1),
            psig: pwlf_from_rows(&m.elecin, 2, 3),
            ebr1: pwlf_from_rows(&m.elecin, 8, 9),
            pbr1: pwlf_from_rows(&m.elecin, 10, 11),
            pbr2: pwlf_from_rows(&m.elecin, 12, 13),
            tmxs: t.tmxs,
            esig_e: t.esig_e,
            psig_e: t.psig_e,
            sig_ismonotone: t.sig_ismonotone,
            te,
            thmoll: te * 2.0 + RM,
            ae: m.ae,
            ms: MsMedium { blcc: t.blcc, xcc: t.xcc, eke0: m.eke[0], eke1: m.eke[1], ..Default::default() },
            brems: BremsMedium {
                delcm: m.delcm,
                dl: fits.dl,
                zbrang,
                lzbrang,
                ap: m.ap,
                log_ap: fits.log_ap,
                nist: None,
            },
        };
        let mut photo = PhotoMedium { zelem: alloc::vec![0.0], pz: alloc::vec![0.0] };
        for e in &m.elements {
            photo.zelem.push(e.zelem);
            photo.pz.push(e.pz);
        }
        let photon = PhotonMediumData {
            rho: m.rho,
            ge0: m.ge[0],
            ge1: m.ge[1],
            gmfp0: pwlf_from_rows(&m.photin, 0, 1).a0,
            gmfp1: pwlf_from_rows(&m.photin, 0, 1).a1,
            gbr10: pwlf_from_rows(&m.photin, 2, 3).a0,
            gbr11: pwlf_from_rows(&m.photin, 2, 3).a1,
            gbr20: pwlf_from_rows(&m.photin, 4, 5).a0,
            gbr21: pwlf_from_rows(&m.photin, 4, 5).a1,
            cohe: m
                .rayleigh
                .as_ref()
                .map(|r| Pwlf {
                    a0: core::iter::once(0.0).chain(r.cohe.iter().map(|c| c[0])).collect(),
                    a1: core::iter::once(0.0).chain(r.cohe.iter().map(|c| c[1])).collect(),
                })
                .unwrap_or_default(),
            rayleigh: None,
            photonuc: Pwlf::default(),
            photo,
            pair,
        };
        media.push(HatchedMedium { electron, photon });
    }
    HatchResult { media, settings, sige_max: (esige_max, psige_max) }
}

/// `$adjust_rhor_ecut_pcut` (`egsnrc.macros:3681-3692`): raise each region's
/// `ECUT` and `PCUT` to at least its medium's `AE` and `AP`, and give regions
/// without a density the medium's. `med`, `ecut`, `pcut`, `rhor` are per
/// region (the same index); `ae`, `ap`, `rho` per medium, 1-based
/// (index 0 unused). A region with `med == 0` or beyond `ae.len()` is vacuum
/// and untouched.
pub fn adjust_rhor_ecut_pcut(
    med: &[usize],
    ecut: &mut [f64],
    pcut: &mut [f64],
    rhor: &mut [f64],
    ae: &[f64],
    ap: &[f64],
    rho: &[f64],
) {
    for (jr, &md) in med.iter().enumerate() {
        if md >= 1 && md < ae.len() {
            ecut[jr] = ecut[jr].max(ae[md]);
            pcut[jr] = pcut[jr].max(ap[md]);
            if rhor[jr] == 0.0 {
                rhor[jr] = rho[md];
            }
        }
    }
}

/// One direct-access record of `spinms.data` as `init_spin` reads it
/// (`egsnrc.mortran:3403-3419`): four 32-bit floats, `fmax_array(0:15)` and
/// 512 16-bit integers.
#[derive(Debug, Clone, PartialEq)]
pub struct SpinRecord {
    /// `dum1`, `dum2`, `dum3`, `aux_o` (`real*4`).
    pub dum: [f32; 3],
    /// `aux_o`.
    pub aux_o: f32,
    /// `fmax_array(0:MAXQ_SPIN)`.
    pub fmax: [f32; MAXQ_SPIN + 1],
    /// `i2_array(1:512)`.
    pub i2: Vec<i16>,
}

/// The decoded contents of `spinms.data`: the header ranges and every record,
/// indexed `[(iz - 1) * 4 * (n_ener + 1) + 2 * iq * (n_ener + 1) + i]` for
/// `iz = 1..=100`, `iq = 0..=1`, `i = 0..=MAXE_SPI1`.
///
/// [`SpinFile::from_bytes`] decodes the binary file. `init_spin` reads it as a
/// Fortran direct-access file with `recl = 276 * $RECL-FACTOR`; `$RECL-FACTOR`
/// and `$BYTE_ORDER` are build-time macros not in the source this port read,
/// so the decoder is written from the record contents instead: every record
/// is 276 four-byte words = 1104 bytes (header: 32-character version, 4-character
/// byte-order token `1234` or `4321`, four `real*4`; data record: four `real*4`,
/// `fmax_array(0:15)` as `real*4`, 512 `integer*2`), and the byte order is the
/// file's own token, which `init_spin` compares with the CPU's to decide on
/// swapping. **Not re-checked against a real `spinms.data`** (not in the clone).
#[derive(Debug, Clone, PartialEq)]
pub struct SpinFile {
    /// `espin_min`, `espin_max`: energy range, keV (`real*4`).
    pub espin_min: f32,
    /// See `espin_min`.
    pub espin_max: f32,
    /// `b2spin_min`, `b2spin_max` (`real*4`).
    pub b2spin_min: f32,
    /// See `b2spin_min`.
    pub b2spin_max: f32,
    /// The records.
    pub records: Vec<SpinRecord>,
}

/// Bytes per direct-access record of `spinms.data`: `276 * 4`
/// (`egsnrc.mortran:3372`).
pub const SPIN_RECORD_BYTES: usize = 276 * 4;

impl SpinFile {
    /// Decode the content of `spinms.data` (see the type's documentation).
    ///
    /// # Errors
    ///
    /// A message when the file is shorter than its header, the byte-order token
    /// is neither `1234` nor `4321`, or a record is cut short.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < SPIN_RECORD_BYTES {
            return Err(String::from("spinms.data is shorter than its header record"));
        }
        let le = match &bytes[32..36] {
            b"1234" => true,
            b"4321" => false,
            _ => return Err(String::from("spinms.data: unknown byte-order token")),
        };
        let f32_at = |o: usize| -> f32 {
            let b = [bytes[o], bytes[o + 1], bytes[o + 2], bytes[o + 3]];
            if le { f32::from_le_bytes(b) } else { f32::from_be_bytes(b) }
        };
        let i16_at = |o: usize| -> i16 {
            let b = [bytes[o], bytes[o + 1]];
            if le { i16::from_le_bytes(b) } else { i16::from_be_bytes(b) }
        };
        let (espin_min, espin_max, b2spin_min, b2spin_max) = (f32_at(36), f32_at(40), f32_at(44), f32_at(48));
        let nrec = bytes.len() / SPIN_RECORD_BYTES - 1;
        if bytes.len() % SPIN_RECORD_BYTES != 0 {
            return Err(String::from("spinms.data: the last record is cut short"));
        }
        let mut records = Vec::with_capacity(nrec);
        for r in 1..=nrec {
            let o = r * SPIN_RECORD_BYTES;
            let mut fmax = [0.0_f32; MAXQ_SPIN + 1];
            for (j, f) in fmax.iter_mut().enumerate() {
                *f = f32_at(o + 16 + 4 * j);
            }
            let base = o + 16 + 4 * (MAXQ_SPIN + 1);
            records.push(SpinRecord {
                dum: [f32_at(o), f32_at(o + 4), f32_at(o + 8)],
                aux_o: f32_at(o + 12),
                fmax,
                i2: (0..512).map(|k| i16_at(base + 2 * k)).collect(),
            });
        }
        Ok(Self { espin_min, espin_max, b2spin_min, b2spin_max, records })
    }
}

/// What `init_spin` needs of one medium.
#[derive(Debug, Clone, Default)]
pub struct SpinMediumInput {
    /// `(ZELEM, PZ, WA)` per element.
    pub elements: Vec<(f64, f64, f64)>,
    /// `eke0`, `eke1`, `meke`.
    pub eke0: f64,
    /// See `eke0`.
    pub eke1: f64,
    /// See `eke0`.
    pub neke: usize,
    /// `blcc`, `xcc` as `mscati` leaves them (Euler constant absorbed, `xcc` squared).
    pub blcc: f64,
    /// See `blcc`.
    pub xcc: f64,
    /// `TE(medium)` and `UE(medium)`, MeV.
    pub te: f64,
    /// See `te`.
    pub ue: f64,
    /// The `esig` and `ededx` fits (1-based).
    pub esig: Pwlf,
    /// See `esig`.
    pub ededx: Pwlf,
}

/// The spin-effect fits of one medium (`COMMON/ELECIN/`, 1-based).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SpinFits {
    /// `etae_ms`, `etap_ms`: screening-angle correction.
    pub etae_ms: Pwlf,
    /// See `etae_ms`.
    pub etap_ms: Pwlf,
    /// `q1ce_ms`, `q1cp_ms`: first-moment correction.
    pub q1ce_ms: Pwlf,
    /// See `q1ce_ms`.
    pub q1cp_ms: Pwlf,
    /// `q2ce_ms`, `q2cp_ms`: second-moment correction.
    pub q2ce_ms: Pwlf,
    /// See `q2ce_ms`.
    pub q2cp_ms: Pwlf,
    /// `blcce`: correction to `blcc` for the scattering power already in
    /// discrete Moller/Bhabha events.
    pub blcce: Pwlf,
}

/// `init_spin` (`egsnrc.mortran:3309-3706`), from the decoded records: the
/// spin rejection function for multiple elastic scattering and the
/// interpolation arrays for the screening parameter, elastic cross section and
/// first and second moments. Returns the [`SpinData`] (rejection tables for
/// all media and the interpolation constants) and the [`SpinFits`] of each
/// medium. `prm` is `PRM`. Single-precision storage follows upstream:
/// the file values and `spin_rej`, `espin_*`, `dlener(i)`, `dbeta2(i)`, `dqq1(i)`
/// are `real*4`, and the arithmetic upstream does on them in single precision
/// is done in `f32` here.
///
/// # Errors
///
/// A message when a medium element's record is missing.
pub fn init_spin(file: &SpinFile, media: &[SpinMediumInput], prm: f64) -> Result<(SpinData, Vec<SpinFits>), String> {
    const FINE: f64 = 137.03604;
    const TF_CONSTANT: f64 = 0.88534138;
    let n_ener = MAXE_SPIN;
    let ne1 = MAXE_SPI1 + 1; // 0..=MAXE_SPI1
    let fool = |v: f64| v; // (the $FOOL-INTEL-OPTIMIZER hooks print only)
    let _ = fool;

    // header ranges and the energy grid (keV), `real*4` arithmetic where upstream's is
    let (e_min_kev, e_max_kev) = (file.espin_min, file.espin_max);
    let dloge = (RealMath::r_ln(e_max_kev / e_min_kev) / n_ener as f32) as f64;
    let mut eloge = RealMath::r_ln(e_min_kev) as f64;
    let mut earray = alloc::vec![0.0_f64; ne1 + 1];
    earray[0] = e_min_kev as f64;
    for i in 1..=n_ener {
        eloge += dloge;
        earray[i] = RealMath::r_exp(eloge);
    }
    let dbeta2_first = ((file.b2spin_max - file.b2spin_min) / n_ener as f32) as f64;
    let mut beta2 = file.b2spin_min as f64;
    earray[n_ener + 1] = e_max_kev as f64;
    for i in (n_ener + 2)..=(2 * n_ener + 1) {
        beta2 += dbeta2_first;
        earray[i] = if beta2 < 0.999 { prm * 1000.0 * (1.0 / Real::sqrt(1.0 - beta2) - 1.0) } else { 50585.1 };
    }

    // convert to MeV and set the interpolation intervals (real*4 storage)
    let espin_min = e_min_kev / 1000.0_f32;
    let espin_max = e_max_kev / 1000.0_f32;
    let dlener = RealMath::r_ln(espin_max / espin_min) / MAXE_SPIN as f32;
    let dleneri = 1.0_f32 / dlener;
    let espml = RealMath::r_ln(espin_min);
    let dbeta2 = (file.b2spin_max - file.b2spin_min) / MAXE_SPIN as f32;
    let dbeta2i = 1.0_f32 / dbeta2;
    let dqq1 = 0.5_f32 / MAXQ_SPIN as f32;
    let dqq1i = 1.0_f32 / dqq1;

    let nmed = media.len();
    let mut spin = SpinData {
        spin_rej: alloc::vec![0.0; nmed * 2 * (MAXE_SPI1 + 1) * (MAXQ_SPIN + 1) * (MAXU_SPIN + 1)],
        espml,
        b2spin_min: file.b2spin_min,
        dbeta2i,
        dleneri,
        dqq1i,
    };
    let mut fits_out = Vec::with_capacity(nmed);

    for (m0, med) in media.iter().enumerate() {
        let medium = m0 + 1;
        let mut eta_array = [[0.0_f64; MAXE_SPI1 + 1]; 2];
        let mut c_array = [[0.0_f64; MAXE_SPI1 + 1]; 2];
        let mut g_array = [[0.0_f64; MAXE_SPI1 + 1]; 2];
        let (mut sum_z2, mut sum_z) = (0.0_f64, 0.0_f64);
        for &(z, pz, _wa) in &med.elements {
            let iz = (z + 0.5) as usize;
            let tmp = pz * z * (z + 1.0);
            sum_z2 += tmp;
            sum_z += pz * z;
            let z23 = RealMath::r_powf(z, 0.6666667);
            for iq in 0..2usize {
                for i in 0..=MAXE_SPI1 {
                    let idx = (iz - 1) * 4 * (n_ener + 1) + 2 * iq * (n_ener + 1) + i;
                    let rec = file
                        .records
                        .get(idx)
                        .ok_or_else(|| alloc::format!("spin data record missing for Z = {iz}"))?;
                    let (dum1, dum2, dum3, aux_o) = (rec.dum[0] as f64, rec.dum[1] as f64, rec.dum[2] as f64, rec.aux_o as f64);
                    eta_array[iq][i] += tmp * RealMath::r_ln(z23 * aux_o);
                    let tau = earray[i] / prm * 0.001;
                    let eta = z23 / ((FINE * TF_CONSTANT) * (FINE * TF_CONSTANT)) * aux_o / 4.0 / tau / (tau + 2.0);
                    c_array[iq][i] += tmp * (RealMath::r_ln(1.0 + 1.0 / eta) - 1.0 / (1.0 + eta)) * dum1 * dum3;
                    g_array[iq][i] += tmp * dum2;
                    for j in 0..=MAXQ_SPIN {
                        let tmp_4 = rec.fmax[j];
                        for k in 0..=MAXU_SPIN {
                            let mut ii4 = rec.i2[(MAXU_SPIN + 1) * j + k] as i32;
                            if ii4 < 0 {
                                ii4 += 65536;
                            }
                            // dum1 = ii4; dum1 = dum1*tmp_4/65535 in single precision
                            let d1: f32 = ii4 as f32 * tmp_4 / 65535.0_f32;
                            let at = spin_idx(medium, iq, i, j, k);
                            spin.spin_rej[at] = (spin.spin_rej[at] as f64 + tmp * d1 as f64) as f32;
                        }
                    }
                }
            }
        }
        // scale the maximum of the rejection function to unity
        for iq in 0..2usize {
            for i in 0..=MAXE_SPI1 {
                for j in 0..=MAXQ_SPIN {
                    let mut fmax = 0.0_f64;
                    for k in 0..=MAXU_SPIN {
                        let v = spin.spin_rej[spin_idx(medium, iq, i, j, k)] as f64;
                        if v > fmax {
                            fmax = v;
                        }
                    }
                    for k in 0..=MAXU_SPIN {
                        let at = spin_idx(medium, iq, i, j, k);
                        spin.spin_rej[at] = (spin.spin_rej[at] as f64 / fmax) as f32;
                    }
                }
            }
        }
        // eta, c and g arrays to their final form
        for i in 0..=MAXE_SPI1 {
            let tau = earray[i] / prm * 0.001;
            let _beta2 = tau * (tau + 2.0) / ((tau + 1.0) * (tau + 1.0));
            for iq in 0..2usize {
                let aux_o = (RealMath::r_exp(eta_array[iq][i] / sum_z2) / ((FINE * TF_CONSTANT) * (FINE * TF_CONSTANT))) as f32 as f64;
                eta_array[iq][i] = 0.26112447 * aux_o * med.blcc / med.xcc;
                let eta = aux_o / 4.0 / tau / (tau + 2.0);
                let l = RealMath::r_ln(1.0 + 1.0 / eta);
                let gamma = 3.0 * (1.0 + eta) * (l * (1.0 + 2.0 * eta) - 2.0) / (l * (1.0 + eta) - 1.0);
                g_array[iq][i] = g_array[iq][i] / sum_z2 / gamma;
                c_array[iq][i] = c_array[iq][i] / sum_z2 / (l - 1.0 / (1.0 + eta));
            }
        }

        let neke = med.neke;
        let (eke0, eke1) = (med.eke0, med.eke1);
        let new_pwlf = || Pwlf { a0: alloc::vec![0.0; neke + 2], a1: alloc::vec![0.0; neke + 2] };
        let mut fits = SpinFits {
            etae_ms: new_pwlf(),
            etap_ms: new_pwlf(),
            q1ce_ms: new_pwlf(),
            q1cp_ms: new_pwlf(),
            q2ce_ms: new_pwlf(),
            q2cp_ms: new_pwlf(),
            blcce: new_pwlf(),
        };

        // the screening-parameter interpolation tables
        let eta_at = |e: f64, eil: f64| -> (f64, f64) {
            if e <= espin_min as f64 {
                (eta_array[0][0], eta_array[1][0])
            } else {
                let (aae, je);
                if e <= espin_max as f64 {
                    let a = (eil - espml as f64) * dleneri as f64;
                    let j = a as usize;
                    aae = a - j as f64;
                    je = j;
                } else {
                    let tau = e / prm;
                    let beta2 = tau * (tau + 2.0) / ((tau + 1.0) * (tau + 1.0));
                    let a = (beta2 - file.b2spin_min as f64) * dbeta2i as f64;
                    let j = a as usize;
                    aae = a - j as f64;
                    je = j + MAXE_SPIN + 1;
                }
                (
                    (1.0 - aae) * eta_array[0][je] + aae * eta_array[0][je + 1],
                    (1.0 - aae) * eta_array[1][je] + aae * eta_array[1][je + 1],
                )
            }
        };
        let eil0 = (1.0 - eke0) / eke1;
        let (mut si1e, mut si1p) = eta_at(RealMath::r_exp(eil0), eil0);
        for i in 1..neke {
            let eil = ((i + 1) as f64 - eke0) / eke1;
            let e = RealMath::r_exp(eil);
            let (si2e, si2p) = eta_at(e, eil);
            fits.etae_ms.a1[i] = (si2e - si1e) * eke1;
            fits.etae_ms.a0[i] = si2e - fits.etae_ms.a1[i] * eil;
            fits.etap_ms.a1[i] = (si2p - si1p) * eke1;
            fits.etap_ms.a0[i] = si2p - fits.etap_ms.a1[i] * eil;
            si1e = si2e;
            si1p = si2p;
        }
        fits.etae_ms.a1[neke] = fits.etae_ms.a1[neke - 1];
        fits.etae_ms.a0[neke] = fits.etae_ms.a0[neke - 1];
        fits.etap_ms.a1[neke] = fits.etap_ms.a1[neke - 1];
        fits.etap_ms.a0[neke] = fits.etap_ms.a0[neke - 1];

        // splines over the energy grid (ln of the energy in MeV)
        let mut elarray = alloc::vec![0.0_f64; ne1 + 1]; // 1-based over ndata = MAXE_SPI1 + 1
        let ndata = MAXE_SPI1 + 1;
        // upstream indexes elarray(0:MAXE_SPI1); the spline routines take it as
        // x(1:n), so the 0-based Mortran array maps to 1-based by one
        for i in 0..=MAXE_SPIN {
            elarray[i + 1] = RealMath::r_ln(earray[i] / 1000.0);
        }
        for i in (MAXE_SPIN + 1)..MAXE_SPI1 {
            elarray[i + 1] = RealMath::r_ln(earray[i + 1] / 1000.0);
        }
        elarray[ndata] = if med.ue > 1e5 { RealMath::r_ln(med.ue) } else { RealMath::r_ln(1e5) };
        let mut moment_fit = |src: &[f64; MAXE_SPI1 + 1], dst: &mut Pwlf| {
            let mut farray = alloc::vec![0.0_f64; ne1 + 1];
            for i in 0..=MAXE_SPIN {
                farray[i + 1] = src[i];
            }
            for i in (MAXE_SPIN + 1)..MAXE_SPI1 {
                farray[i + 1] = src[i + 1];
            }
            farray[ndata] = 1.0;
            let co = set_spline(&elarray, &farray);
            let eil = (1.0 - eke0) / eke1;
            let mut si1 = spline(eil, &elarray, &co);
            for i in 1..neke {
                let eil = ((i + 1) as f64 - eke0) / eke1;
                let si2 = spline(eil, &elarray, &co);
                dst.a1[i] = (si2 - si1) * eke1;
                dst.a0[i] = si2 - dst.a1[i] * eil;
                si1 = si2;
            }
            dst.a1[neke] = dst.a1[neke - 1];
            dst.a0[neke] = dst.a0[neke - 1];
        };
        moment_fit(&c_array[0], &mut fits.q1ce_ms);
        moment_fit(&c_array[1], &mut fits.q1cp_ms);
        moment_fit(&g_array[0], &mut fits.q2ce_ms);
        moment_fit(&g_array[1], &mut fits.q2cp_ms);

        // subtract the scattering power already in discrete Moller/Bhabha events
        let tauc = med.te / prm;
        let mut si1e = 1.0_f64;
        for i in 1..neke {
            let eil = ((i + 1) as f64 - eke0) / eke1;
            let e = RealMath::r_exp(eil);
            let tau = e / prm;
            let si2e;
            if tau > 2.0 * tauc {
                let sig = med.esig.eval(i + 1, eil) / med.ededx.eval(i + 1, eil);
                if sig > 1e-6 {
                    let etap = fits.etae_ms.eval(i + 1, eil);
                    let eta = 0.25 * etap * med.xcc / med.blcc / tau / (tau + 2.0);
                    let g_r = (1.0 + 2.0 * eta) * RealMath::r_ln(1.0 + 1.0 / eta) - 2.0;
                    let t1 = (tau + 2.0) / (tau + 1.0);
                    let mut g_m = RealMath::r_ln(0.5 * tau / tauc)
                        + (1.0 + t1 * t1) * RealMath::r_ln(2.0 * (tau - tauc + 2.0) / (tau + 4.0))
                        - 0.25
                            * (tau + 2.0)
                            * (tau + 2.0 + 2.0 * (2.0 * tau + 1.0) / ((tau + 1.0) * (tau + 1.0)))
                            * RealMath::r_ln((tau + 4.0) * (tau - tauc) / tau / (tau - tauc + 2.0))
                        + 0.5 * (tau - 2.0 * tauc) * (tau + 2.0) * (1.0 / (tau - tauc) - 1.0 / ((tau + 1.0) * (tau + 1.0)));
                    if g_m < g_r {
                        g_m /= g_r;
                    } else {
                        g_m = 1.0;
                    }
                    si2e = 1.0 - g_m * sum_z / sum_z2;
                } else {
                    si2e = 1.0;
                }
            } else {
                si2e = 1.0;
            }
            fits.blcce.a1[i] = (si2e - si1e) * eke1;
            fits.blcce.a0[i] = si2e - fits.blcce.a1[i] * eil;
            si1e = si2e;
        }
        fits.blcce.a1[neke] = fits.blcce.a1[neke - 1];
        fits.blcce.a0[neke] = fits.blcce.a0[neke - 1];
        fits_out.push(fits);
    }
    Ok((spin, fits_out))
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
            ne_array: alloc::vec![0, 2],
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
            eadl: None,
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
            lzbrang: -(0.02f64).ln(),
            ap: 0.01,
            log_ap: (0.01f64).ln(),
            nist: None,
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
            cohe: const_pwlf(1.0),
            rayleigh: None,
            photonuc: const_pwlf(1.0),
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
    fn photonuclear_placeholder_discards_the_photon_and_keeps_energy_balance() {
        let mut st = Stack::new(5);
        st.np = 2;
        st.e[2] = 3.0;
        st.wt[2] = 1.5;
        photonuc(&mut st);
        assert_eq!((st.np_old, st.np, st.e[2], st.wt[2]), (2, 2, 0.0, 0.0));
        // with a photonuclear factor of one half the mean free path halves and
        // half of the interactions are photonuclear; every history still balances
        // because the discard reports the photon's energy as `edep`
        let mut tables = photon_tables(0.0, 0.5, 0);
        tables.regions[1].iphotonucr = 1;
        tables.media[1].photonuc = const_pwlf(0.5);
        let cfg = PhotonConfig { pair: PairConfig::default(), rr: None, prm: RM, vacdst: 100.0 };
        let mut rng = Ranmar::new(60, 61);
        let mut absorbed = 0;
        for _ in 0..200 {
            let r = run_history(&tables, &cfg, 0.5, &mut rng);
            assert!(r.abs() < 1e-9, "residual {r}");
            absorbed += 1;
        }
        assert_eq!(absorbed, 200);
        // with the placeholder on fewer histories reach an ordinary interaction (a charged
        // particle on top of the stack), and the photonuclear discards report the photon's
        // energy as `edep` (upstream's USER-PHOTON-DISCARD), so they look like full-energy
        // escapes: both are pinned here
        let mut off = tables.clone();
        off.regions[1].iphotonucr = 0;
        let mut geo = Slab { length: 3.0 };
        let mut tally = |t: &PhotonTables, rng: &mut Ranmar| {
            let (mut interacted, mut full_energy_discards) = (0, 0);
            for _ in 0..300 {
                let mut st = Stack::new(100);
                st.np = 1;
                st.e[1] = 1.0;
                st.w[1] = 1.0;
                st.z[1] = 1e-4;
                st.ir[1] = 1;
                st.wt[1] = 1.0;
                let r = photon(&mut st, t, &cfg, &mut geo, rng).unwrap();
                if r.ircode == 1 {
                    interacted += 1;
                }
                if r.ircode == 2 && r.edep == 1.0 {
                    full_energy_discards += 1;
                }
            }
            (interacted, full_energy_discards)
        };
        let (int_on, disc_on) = tally(&tables, &mut rng);
        let (int_off, disc_off) = tally(&off, &mut rng);
        assert!(int_on < int_off, "interacting histories {int_on} vs {int_off}");
        assert!(disc_on > disc_off, "full-energy discards {disc_on} vs {disc_off}");
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
        // Rayleigh is refused
        let mut bad = tables.clone();
        bad.regions[1].irayl = 1;
        let mut st = Stack::new(10);
        st.np = 1;
        st.e[1] = 1.0;
        st.ir[1] = 1;
        st.wt[1] = 1.0;
        st.w[1] = 1.0;
        assert!(matches!(photon(&mut st, &bad, &cfg, &mut geo, &mut rng), Err(EgsError::Unsupported(_))));
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
        let slab = ElectrRegion { med: 1, rhor: 1.0, ecut: te + RM, smaxir: 1.0e10, pcut: 0.01 };
        let outside = ElectrRegion { med: 0, ..slab };
        ElectrTables {
            regions: alloc::vec![ElectrRegion::default(), slab, outside],
            media: alloc::vec![ElectronMediumData::default(), med],
            ms: Some(MsData::from_text(&synthetic_ms_text()).unwrap()),
            spin: flat_spin(1.0),
            eii: None,
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
            self.tally += r.edep_steps + r.edep + r.edep_eii;
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


    /// A synthetic PEGS4 data set in the real file's fixed-width format.
    fn pegs4_text(name: &str, rayleigh: bool, gas: bool) -> String {
        fn flt(vals: &[f64]) -> String {
            let mut s = String::new();
            for chunk in vals.chunks(5) {
                s.push(' ');
                for v in chunk {
                    s.push_str(&alloc::format!("{:>14}", alloc::format!("{:.5E}", v).replace("E", "E+").replace("E+-", "E-")));
                }
                s.push('\n');
            }
            s
        }
        fn int(vals: &[i32]) -> String {
            let mut s = String::new();
            for chunk in vals.chunks(14) {
                s.push(' ');
                for v in chunk {
                    s.push_str(&alloc::format!("{:>5}", v));
                }
                s.push('\n');
            }
            s
        }
        let (nge, neke) = (3usize, 4usize);
        let mut t = String::new();
        t.push_str(&alloc::format!(" MEDIUM={:<24},STERNCID=XX\n", name));
        if gas {
            t.push_str(" GAS ,RHO= 1.2000E-03,NE= 1,GASP= 1.0000E+00, IUNRST=0, EPSTFL=1, IAPRIM=1\n");
        } else {
            t.push_str(" ELEM,RHO= 1.1340E+01,NE= 2, IUNRST=0, EPSTFL=1, IAPRIM=1\n");
        }
        let nne = if gas { 1 } else { 2 };
        for i in 0..nne {
            t.push_str(&alloc::format!(
                " ASYM=PB,Z={:>2}.,A={:>9.3},PZ={:>12.5E},RHOZ={:>12.5E}\n",
                82 - i,
                207.19,
                1.0,
                207.19
            ));
        }
        t.push_str(&flt(&[0.5617, 0.512, 0.001, 2.511, 2.0]));
        t.push_str(&int(&[0, nge as i32, 0, neke as i32, 0, 0, 0, rayleigh as i32]));
        let dl: Vec<f64> = (0..36).map(|k| 1.0 + k as f64 * 0.1).collect();
        t.push_str(&flt(&dl));
        t.push_str(&flt(&[16.0, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4]));
        t.push_str(&flt(&[0.3, 0.0006, 3.4e4, 5.7]));
        t.push_str(&flt(&[136.5, 19.4]));
        let el: Vec<f64> = (0..16 * neke).map(|k| 100.0 + k as f64).collect();
        t.push_str(&flt(&el));
        t.push_str(&flt(&[0.057, 2.5, 7.0]));
        let ph: Vec<f64> = (0..6 * nge).map(|k| 10.0 + k as f64).collect();
        t.push_str(&flt(&ph));
        if rayleigh {
            t.push_str(&int(&[2]));
            t.push_str(&flt(&[1.0, 2.0]));
            t.push_str(&flt(&[3.0, 4.0, 5.0, 6.0]));
            let co: Vec<f64> = (0..2 * nge).map(|k| 7.0 + k as f64).collect();
            t.push_str(&flt(&co));
        }
        t
    }

    #[test]
    fn pegs4_reader_parses_the_fixed_width_layout() {
        let text = pegs4_text("PB", true, false);
        let meds = parse_pegs4_media(&text, &["PB"]).unwrap();
        let m = &meds[0];
        assert_eq!(m.name.trim_end(), "PB");
        assert_eq!((m.nne, m.iunrst, m.epstfl, m.iaprim), (2, 0, 1, 1));
        assert!((m.rho - 11.34).abs() < 1e-12);
        assert_eq!(m.elements.len(), 2);
        assert_eq!(m.elements[0].asym, "PB");
        assert_eq!((m.elements[0].zelem, m.elements[1].zelem), (82.0, 81.0));
        assert!((m.elements[0].wa - 207.19).abs() < 1e-9 && m.elements[0].pz == 1.0);
        assert_eq!((m.rlc, m.ae, m.ap, m.ue, m.up), (0.5617, 0.512, 0.001, 2.511, 2.0));
        assert_eq!(m.sizes, [0, 3, 0, 4, 0, 0, 0]);
        assert_eq!(m.irayl, 1);
        // DL(k, i): i-major order in the file
        assert!((m.dl[0][0] - 1.0).abs() < 1e-9 && (m.dl[1][0] - 1.1).abs() < 1e-9 && (m.dl[0][1] - 1.6).abs() < 1e-9);
        assert_eq!((m.delcm, m.alphi, m.bpar, m.delpos), (16.0, [0.9, 0.6], [0.8, 0.5], [0.7, 0.4]));
        assert_eq!((m.xr0, m.teff0, m.blcc, m.xcc), (0.3, 0.0006, 3.4e4, 5.7));
        assert_eq!(m.eke, [136.5, 19.4]);
        assert_eq!(m.elecin.len(), 4);
        assert_eq!(m.elecin[1][0], 116.0);
        assert_eq!((m.ebinda, m.ge), (0.057, [2.5, 7.0]));
        assert_eq!(m.photin.len(), 3);
        assert_eq!(m.photin[2][5], 10.0 + 17.0);
        let r = m.rayleigh.as_ref().unwrap();
        assert_eq!((r.ngr, r.rco), (2, [1.0, 2.0]));
        assert_eq!(r.rsct, alloc::vec![[3.0, 4.0], [5.0, 6.0]]);
        assert_eq!(r.cohe.len(), 3);
        // without a Rayleigh block
        let plain = parse_pegs4_media(&pegs4_text("AL", false, false), &["AL"]).unwrap();
        assert!(plain[0].rayleigh.is_none());
        // a gas-pressure header line (format 2)
        let gas = parse_pegs4_media(&pegs4_text("AIR", false, true), &["AIR"]).unwrap();
        assert_eq!((gas[0].nne, gas[0].iunrst, gas[0].epstfl, gas[0].iaprim), (1, 0, 1, 1));
    }

    #[test]
    fn pegs4_reader_finds_media_in_request_order_and_reports_missing() {
        let text = alloc::format!("{}{}{}", pegs4_text("A1", false, false), pegs4_text("B2", false, false), pegs4_text("C3", true, false));
        let meds = parse_pegs4_media(&text, &["C3", "A1"]).unwrap();
        assert_eq!((meds[0].name.trim_end(), meds[1].name.trim_end()), ("C3", "A1"));
        assert!(meds[0].rayleigh.is_some() && meds[1].rayleigh.is_none());
        // a duplicated name in the request: the second copy of its data is skipped
        let dup = alloc::format!("{}{}", pegs4_text("A1", false, false), pegs4_text("A1", false, false));
        assert!(parse_pegs4_media(&dup, &["A1", "A1"]).is_err(), "only one copy of A1 exists to match the second request");
        let e = parse_pegs4_media(&text, &["A1", "ZZ"]).unwrap_err();
        assert!(e.contains("ZZ"), "{e}");
        assert!(parse_pegs4_media(" MEDIUM=PB\n", &["PB"]).is_err(), "truncated data");
    }

    #[test]
    fn dunit_scaling_follows_radiation_length() {
        let mut m = parse_pegs4_media(&pegs4_text("PB", true, false), &["PB"]).unwrap().remove(0);
        let before = m.clone();
        let rldu = m.apply_dunit(m.rlc); // unit = one radiation length: no change
        assert!((rldu - 1.0).abs() < 1e-15);
        assert!((m.blcc - before.blcc).abs() < 1e-9 * before.blcc);
        let mut m2 = before.clone();
        let rldu2 = m2.apply_dunit(1.0); // centimetres
        assert!((rldu2 - before.rlc).abs() < 1e-15);
        let dfact = before.rlc;
        assert!((m2.elecin[0][0] - before.elecin[0][0] / dfact).abs() < 1e-9, "stopping power per cm");
        assert!((m2.elecin[0][14] - before.elecin[0][14] * dfact).abs() < 1e-9, "tmxs in cm");
        assert!((m2.teff0 - before.teff0 * dfact).abs() < 1e-12);
        assert!((m2.blcc - before.blcc / dfact).abs() < 1e-6);
        assert!((m2.xcc - before.xcc / dfact.sqrt()).abs() < 1e-9);
        assert!((m2.photin[0][1] - before.photin[0][1] * dfact).abs() < 1e-9, "mean free path in cm");
        assert_eq!(m2.photin[0][2], before.photin[0][2], "branching ratios do not scale");
    }


    /// Reads a real PEGS4 data set when `EGSNRC_PEGS4_DATA` names one (the file
    /// is EGSnrc's, not vendored here, so the test is skipped without it) and
    /// checks the structural invariants the layout implies: the array sizes
    /// match what was read and the tables are finite and ordered.
    #[cfg(feature = "std")]
    #[test]
    fn pegs4_reader_on_a_real_data_set_when_available() {
        let Ok(path) = std::env::var("EGSNRC_PEGS4_DATA") else {
            return;
        };
        let text = std::fs::read_to_string(&path).expect("EGSNRC_PEGS4_DATA is readable");
        let first = text.lines().next().unwrap();
        let name = first[8..32].trim().to_string();
        let meds = parse_pegs4_media(&text, &[name.as_str()]).unwrap();
        let m = &meds[0];
        assert_eq!(m.elecin.len(), m.sizes[3] as usize);
        assert_eq!(m.photin.len(), m.sizes[1] as usize);
        assert!(m.rho > 0.0 && m.rlc > 0.0 && m.ae > 0.5 && m.ap > 0.0);
        assert_eq!(m.elements.len(), m.nne);
        assert!(m.elecin.iter().flatten().all(|v| v.is_finite()));
        assert!(m.photin.iter().flatten().all(|v| v.is_finite()));
        // the fits are over increasing energy: the cut-off energy sits in the table
        assert!(m.eke[1] > 0.0 && m.ge[1] > 0.0);
        std::eprintln!("PEGS4 {name}: rho {} rlc {} ae {} neke {} nge {}", m.rho, m.rlc, m.ae, m.sizes[3], m.sizes[1]);

        // run the whole assembly on it, in centimetres
        let mut scaled = meds[0].clone();
        scaled.apply_dunit(1.0);
        let cfg = HatchConfig { ecutmn: scaled.ae, ..Default::default() };
        let h = hatch_media(&[scaled.clone()], &cfg);
        let e = &h.media[0].electron;
        let neke = scaled.sizes[3] as usize;
        assert!(e.e_array[1] > 0.0 && e.e_array[1] < scaled.ae, "the table starts below AE");
        assert!(e.e_array[1..=neke].windows(2).all(|w| w[1] > w[0]), "table energies increase");
        assert!(e.range_ep[0][1..=neke].windows(2).all(|w| w[1] > w[0]), "e- range increases with energy");
        assert!(e.range_ep[1][1..=neke].windows(2).all(|w| w[1] > w[0]), "e+ range increases with energy");
        assert!(e.range_ep[1][neke] <= e.range_ep[0][neke] * 1.5 && e.range_ep[1][neke] > 0.0);
        assert!(e.esig_e > 0.0 && e.psig_e > 0.0 && e.esig_e.is_finite());
        for i in 1..neke {
            let elke = (i as f64 + 1.0 - e.eke0) / e.eke1;
            let t = e.tmxs.a1[i] * elke + e.tmxs.a0[i];
            assert!(t > 0.0 && t.is_finite(), "tmxs at node {i}: {t}");
        }
        assert!(e.brems.zbrang > 0.0 && e.brems.lzbrang.is_finite());
        std::eprintln!(
            "hatch_media {name}: range(top) {:.4} cm, esig_e {:.4}, tmxs(top) {:.4} cm, blcc {:.4e}",
            e.range_ep[0][neke],
            e.esig_e,
            e.tmxs.a1[neke - 1] * ((neke as f64 - e.eke0) / e.eke1) + e.tmxs.a0[neke - 1],
            e.ms.blcc
        );
    }


    #[test]
    fn fix_brems_fits_are_consistent() {
        // hydrogen-like (Z <= 4 branch of XSIF) and a heavier element
        for z in [1.0, 4.0, 6.0, 82.0] {
            assert!(xsif(z).is_finite() && fcoulc(z) >= 0.0);
        }
        assert!((fcoulc(1.0) - (1.0 / 137.03604f64).powi(2) * (1.0 / (1.0 + (1.0 / 137.03604f64).powi(2)) + 0.20206)).abs() < 1e-5);
        let f = fix_brems(&[(6.0, 1.0), (8.0, 2.0)], 0.001);
        assert!((f.log_ap - 0.001f64.ln()).abs() < 1e-15);
        // dl6 is the constant 0.952 for every l, dl1(1..4) are normalised by the maxima
        assert!((1..=8).all(|l| f.dl[5][l] == 0.952));
        // the normalised brems rejection functions peak at one
        let phi1_at_zero = f.dl[0][1];
        let phi2_at_zero = f.dl[0][2];
        assert!(2.0 * phi1_at_zero - 2.0 * phi2_at_zero / 3.0 > 0.999 && 2.0 * phi1_at_zero - 2.0 * phi2_at_zero / 3.0 < 1.001);
        // bpar derives from dl1(7), dl1(8)
        assert!((f.bpar[2] - f.dl[0][7] / (3.0 * f.dl[0][8] + f.dl[0][7])).abs() < 1e-15);
        assert!((f.bpar[1] - 12.0 * f.dl[0][8] / (3.0 * f.dl[0][8] + f.dl[0][7])).abs() < 1e-15);
        // a single element: Zg == Zv - Zf/Zt relation holds and nothing is NaN
        let g = fix_brems(&[(82.0, 1.0)], 0.01);
        assert!(g.dl.iter().all(|v| v.iter().all(|x| x.is_finite())));
    }

    /// An `incoh.data`-shaped text: 18 comment lines, then `MXTOTSH` shells.
    /// Z = 6 has shells with 2,2,1,1 electrons, Z = 8 has 2,2,2,1,1; the rest
    /// is filler.
    fn synthetic_incoh_text() -> String {
        let mut t = String::new();
        for k in 0..18 {
            t.push_str(&alloc::format!("# comment {k}\n"));
        }
        let mut n = 0;
        for (z, nes) in [(6, alloc::vec![2, 2, 1, 1]), (8, alloc::vec![2, 2, 2, 1, 1])] {
            for (k, ne) in nes.iter().enumerate() {
                t.push_str(&alloc::format!("{z} {} {ne} {:.5} {:.1}\n", k + 1, 0.05 + 0.01 * k as f64, 300.0 / (k + 1) as f64));
                n += 1;
            }
        }
        while n < MXTOTSH {
            t.push_str("99 1 2 0.05 1000.0\n");
            n += 1;
        }
        t
    }

    #[test]
    fn init_compton_builds_exact_alias_tables() {
        let text = synthetic_incoh_text();
        let media = alloc::vec![alloc::vec![(6.0, 1.0), (8.0, 2.0)], alloc::vec![(6.0, 1.0)]];
        let d = init_compton(&text, &media, RM).unwrap();
        // unit conversions
        assert!((d.jo_array[1] - 0.05 * 137.0).abs() < 1e-12);
        assert!((d.be_array[1] - 300.0 * 1e-6 / RM).abs() < 1e-15);
        assert!((d.erf_jo_array[1] - 0.82436063535 * (petir::specfunc::erf(0.70710678119 * (1.0 + 0.3 * d.jo_array[1])) - 1.0)).abs() < 1e-11);
        assert_eq!((d.n_shell[1], d.n_shell[2]), (9, 4));
        // the alias table reproduces the shell probabilities pz*ne/sum exactly
        for medium in 1..=2 {
            let nsh = d.n_shell[medium] as usize;
            let comp = &media[medium - 1];
            let total: f64 = comp
                .iter()
                .map(|&(z, pz)| pz * (1..=MXTOTSH).filter(|&j| d.iz_array[j] == z as i32).map(|j| d.ne_array[j] as f64).sum::<f64>())
                .sum();
            for s in 1..=nsh {
                let j = d.shell_array[medium][s] as usize;
                let pz = comp.iter().find(|&&(z, _)| z as i32 == d.iz_array[j]).unwrap().1;
                let want = pz * d.ne_array[j] as f64 / total;
                let stay = d.eno_array[medium][s] - s as f64;
                let mut got = stay.clamp(0.0, 1.0);
                for i in 1..=nsh {
                    if d.eno_atbin_array[medium][i] as usize == s && i != s {
                        got += 1.0 - (d.eno_array[medium][i] - i as f64).clamp(0.0, 1.0);
                    }
                }
                got /= nsh as f64;
                assert!((got - want).abs() < 1e-12, "medium {medium} shell {s}: {got} vs {want}");
            }
        }
        // errors: short data, a medium with no shells
        assert!(init_compton("x\n", &media, RM).is_err());
        assert!(init_compton(&text, &[alloc::vec![(50.0, 1.0)]], RM).is_err());
        assert!(init_compton(&text, &[alloc::vec![(99.0, 1.0)]], RM).is_err(), "more than MXMDSH shells");
    }


    fn constant_mscati_medium(dedx: f64, esig: f64) -> MscatiMedium {
        let neke = 14;
        MscatiMedium {
            eke0: 11.0,
            eke1: 2.0,
            neke,
            ededx: const_pwlf(dedx),
            pdedx: const_pwlf(dedx),
            esig: const_pwlf(esig),
            psig: const_pwlf(esig),
            blcc: 1.0e4,
            xcc: 1.0e-2,
        }
    }

    #[test]
    fn mscati_settings_resolve_defaults() {
        let media = [(1.0e4, 1.0e-2)];
        let s = MscatiSettings::resolve(0, -1.0, 2.0, 0.0, 0.7, RM, &media);
        assert_eq!((s.exact_bca, s.estepe, s.ximax, s.skindepth_for_bca), (true, MAX_ELOSS, BCA_XIMAX, SKIN_DEPTH_FOR_BCA));
        // user values inside (0, 1) are kept, a skin depth above 1e-4 too
        let s = MscatiSettings::resolve(0, 0.1, 0.3, 5.0, 0.7, RM, &media);
        assert_eq!((s.estepe, s.ximax, s.skindepth_for_bca), (0.1, 0.3, 5.0));
        // an inexact BCA derives the PRESTA-I skin depth from ecut, blcc, xcc
        let s = MscatiSettings::resolve(1, 0.0, 0.0, 0.0, 0.7, RM, &media);
        assert!(!s.exact_bca && s.skindepth_for_bca.is_finite() && s.skindepth_for_bca > 0.0);
        let tstbm = (0.7 - RM) * (0.7 + RM) / 0.49;
        let tstbm = 1.0e4 * tstbm * (0.7f64 / 1.0e-2).powi(2);
        let want = (tstbm / tstbm.ln()).ln().exp();
        assert!((s.skindepth_for_bca - want).abs() < 1e-9 * want);
    }

    #[test]
    fn mscati_builds_range_tmxs_and_maxima() {
        let m = constant_mscati_medium(2.0, 5.0);
        let settings = MscatiSettings { exact_bca: true, estepe: 0.25, ximax: 0.5, skindepth_for_bca: 3.0 };
        let (tabs, esige, psige) = mscati(&[m.clone()], &settings, RM);
        let t = &tabs[0];
        // Euler constant absorbed, xcc squared
        assert!((t.blcc - 1.16699413758864573e4).abs() < 1e-9);
        assert!((t.xcc - 1.0e-4).abs() < 1e-15);
        // constant cross section per energy loss: max 5/2, monotone (not decreasing)
        assert!((t.esig_e - 2.5).abs() < 1e-12 && (t.psig_e - 2.5).abs() < 1e-12);
        assert_eq!((esige, psige), (2.5, 2.5));
        assert_eq!(t.sig_ismonotone, [true, true]);
        // E_array follows the table definition and range_ep = (E_i - E_1)/dedx for constant stopping power
        assert!((t.e_array[1] - ((1.0 - 11.0) / 2.0f64).exp()).abs() < 1e-15);
        assert!((t.expeke1 - ((0.5f64).exp() - 1.0)).abs() < 1e-15);
        for i in 1..=14 {
            let want = (t.e_array[i] - t.e_array[1]) / 2.0;
            assert!((t.range_ep[0][i] - want).abs() < 1e-12 * want.max(1e-9), "e- range at {i}");
            assert!((t.range_ep[1][i] - want).abs() < 1e-12 * want.max(1e-9), "e+ range at {i}");
        }
        assert!(t.range_ep[0].windows(2).take(14).all(|w| w[1] >= w[0]));
        // tmxs at a table node is the path for a fractional energy loss estepx(E) * E / dedx
        for i in 2..=13usize {
            let elke = (i as f64 - 11.0) / 2.0;
            let e = elke.exp();
            let p2 = e * (e + 2.0 * RM);
            let beta2 = p2 / (p2 + RM * RM);
            let xcc = t.xcc;
            let blcc = t.blcc;
            let chia2 = xcc / (4.0 * p2 * blcc);
            let mut estepx = 2.0 * p2 * beta2 * 2.0 / e / xcc / ((1.0 + 1.0 / chia2).ln() * (1.0 + chia2) - 1.0) * 0.5;
            estepx = estepx.min(0.25);
            let want = estepx * e / 2.0;
            let got = t.tmxs.a1[i - 1] * elke + t.tmxs.a0[i - 1];
            assert!((got - want).abs() < 1e-8 * want.max(1e-9), "tmxs at node {i}: {got} vs {want}");
        }
        // the last table entry repeats the one before it
        assert_eq!(t.tmxs.a0[14], t.tmxs.a0[13]);
        assert_eq!(t.tmxs.a1[14], t.tmxs.a1[13]);
        // a cross section per energy loss that falls with energy is not monotone-increasing
        let mut falling = constant_mscati_medium(2.0, 5.0);
        falling.esig = Pwlf { a0: (0..16).map(|i| 5.0 - 0.1 * i as f64).collect(), a1: alloc::vec![0.0; 16] };
        let (tf, _, _) = mscati(&[falling], &settings, RM);
        assert!(!tf[0].sig_ismonotone[0] && tf[0].sig_ismonotone[1]);
    }


    /// A PEGS4 medium consistent with the toy transport tables: 14 energy bins
    /// (`lelke = 2 ln E + 11`), constant stopping power 2 MeV/cm, constant
    /// cross section, and a 1-point-per-bin photon table.
    fn consistent_pegs4_medium() -> Pegs4Medium {
        let neke = 14;
        let mut row = [0.0; 16];
        row[0] = 5.0; // ESIG0
        row[2] = 5.0; // PSIG0
        row[4] = 2.0; // EDEDX0
        row[6] = 2.0; // PDEDX0
        row[8] = 0.3; // EBR10
        row[10] = 0.3; // PBR10
        row[12] = 0.6; // PBR20
        row[14] = 0.1; // TMXS0 (overwritten by mscati)
        let mut dl = [[0.0; 6]; 6];
        for k in 0..6 {
            for i in 0..6 {
                dl[k][i] = 1.0;
            }
        }
        Pegs4Medium {
            name: String::from("TOY"),
            rho: 1.0,
            nne: 2,
            elements: alloc::vec![
                Pegs4Element { asym: "C ".into(), zelem: 6.0, wa: 12.0, pz: 1.0, rhoz: 0.4 },
                Pegs4Element { asym: "O ".into(), zelem: 8.0, wa: 16.0, pz: 2.0, rhoz: 0.6 },
            ],
            rlc: 40.0,
            ae: 0.521,
            ap: 0.01,
            ue: 2.0,
            up: 2.0,
            sizes: [0, 3, 0, neke as i32, 0, 0, 0],
            dl,
            delcm: 0.5,
            blcc: 1.0e4,
            xcc: 1.0e-2,
            eke: [11.0, 2.0],
            elecin: alloc::vec![row; neke],
            ge: [1.0, 0.0],
            photin: alloc::vec![[1.0, 0.0, 0.2, 0.0, 0.6, 0.0]; 3],
            ..Default::default()
        }
    }

    #[test]
    fn hatch_media_assembles_the_tables() {
        let m = consistent_pegs4_medium();
        let h = hatch_media(&[m.clone()], &HatchConfig::default());
        let e = &h.media[0].electron;
        // PEGS4 columns land in the right fits (1-based over the bins)
        assert_eq!((e.ededx.a0[3], e.pdedx.a0[3], e.esig.a0[3], e.psig.a0[3]), (2.0, 2.0, 5.0, 5.0));
        assert_eq!((e.ebr1.a0[1], e.pbr1.a0[1], e.pbr2.a0[1]), (0.3, 0.3, 0.6));
        assert_eq!(e.ededx.a0.len(), 15);
        // thresholds
        assert!((e.te - (0.521 - RM)).abs() < 1e-15);
        assert!((e.thmoll - (2.0 * (0.521 - RM) + RM)).abs() < 1e-15);
        // mscati's products: range from the table, Euler constant in blcc, xcc squared
        assert!((e.range_ep[0][5] - (e.e_array[5] - e.e_array[1]) / 2.0).abs() < 1e-12);
        assert!((e.ms.blcc - 1.16699413758864573e4).abs() < 1e-9 && (e.ms.xcc - 1.0e-4).abs() < 1e-15);
        assert!(e.esig_e > 0.0 && e.sig_ismonotone == [true, true]);
        // ZBRANG = 8.116224e-5 * (sum pz Z (Z+1) / sum pz)^(1/3), LZBRANG = -ln ZBRANG
        let want = 8.116224e-05 * ((1.0 * 6.0 * 7.0 + 2.0 * 8.0 * 9.0) / 3.0f64).powf(1.0 / 3.0);
        assert!((e.brems.zbrang - want).abs() < 1e-15 * want.max(1.0) + 1e-18);
        assert!((e.brems.lzbrang + want.ln()).abs() < 1e-12);
        assert_eq!(h.media[0].photon.pair.zbrang, e.brems.zbrang);
        // fix_brems results reach both users
        let fits = fix_brems(&[(6.0, 1.0), (8.0, 2.0)], 0.01);
        assert_eq!(e.brems.dl[0][1], fits.dl[0][1]);
        assert_eq!(h.media[0].photon.pair.bpar, fits.bpar);
        assert_eq!(e.brems.log_ap, fits.log_ap);
        // photon side
        let p = &h.media[0].photon;
        assert_eq!((p.ge0, p.ge1, p.rho), (1.0, 0.0, 1.0));
        assert_eq!((p.gmfp0[2], p.gbr10[2], p.gbr20[2]), (1.0, 0.2, 0.6));
        assert_eq!(p.photo.zelem, alloc::vec![0.0, 6.0, 8.0]);
        assert_eq!(p.photo.pz, alloc::vec![0.0, 1.0, 2.0]);
        // ZBRANG is not computed when neither angular distribution needs it
        let off = hatch_media(&[m], &HatchConfig { ibrdst: 0, iprdst: 0, ..Default::default() });
        assert_eq!((off.media[0].electron.brems.zbrang, off.media[0].electron.brems.lzbrang), (0.0, 0.0));
    }

    #[test]
    fn adjust_rhor_ecut_pcut_raises_cuts_and_sets_densities() {
        let med = [1usize, 0, 2];
        let mut ecut = [0.4, 0.4, 0.9];
        let mut pcut = [0.001, 0.001, 0.5];
        let mut rhor = [0.0, 0.0, 2.5];
        adjust_rhor_ecut_pcut(&med, &mut ecut, &mut pcut, &mut rhor, &[0.0, 0.521, 0.6], &[0.0, 0.01, 0.02], &[0.0, 1.2, 3.0]);
        assert_eq!(ecut, [0.521, 0.4, 0.9]);
        assert_eq!(pcut, [0.01, 0.001, 0.5]);
        assert_eq!(rhor, [1.2, 0.0, 2.5], "vacuum untouched, a set density kept");
    }

    #[test]
    fn hatched_tables_drive_an_electron_shower() {
        // the same energy-balance check as the hand-built tables, now with the
        // electron tables assembled by hatch_media from PEGS4-shaped data
        let h = hatch_media(&[consistent_pegs4_medium()], &HatchConfig::default());
        let med = h.media[0].electron.clone();
        let mut ms_med = toy_ms_medium();
        ms_med.blcc = med.ms.blcc;
        ms_med.xcc = med.ms.xcc;
        ms_med.eke0 = med.eke0;
        ms_med.eke1 = med.eke1;
        let mut electron = med;
        let toy = toy_electron_tables();
        electron.ms = MsMedium { etae_ms: ms_med.etae_ms.clone(), ..toy.media[1].ms.clone() };
        electron.ms.blcc = ms_med.blcc;
        electron.ms.xcc = ms_med.xcc;
        electron.brems = toy.media[1].brems.clone();
        let mut tables = toy.clone();
        tables.media[1] = electron;
        let mut rng = Ranmar::new(14, 15);
        let ptables = photon_tables(0.0, 0.5, 0);
        let pcfg = PhotonConfig { pair: PairConfig::default(), rr: None, prm: RM, vacdst: 100.0 };
        let mut pgeo = Slab { length: 3.0 };
        for _ in 0..10 {
            let mut drv = ElectrDriver {
                tables: tables.clone(),
                cfg: electr_cfg(true, true, false),
                geo: Slab { length: 3.0 },
                counters: ElectrCounters::default(),
                tally: 0.0,
            };
            let ei = 0.5 + RM + 0.3;
            let mut st = Stack::new(500);
            let start = ShowerStart { iqi: -1, ei, xi: 0.0, yi: 0.0, zi: 1.0, ui: 0.0, vi: 0.0, wi: 1.0, iri: 1, wti: 1.0 };
            let r = shower(&mut st, &ptables, &pcfg, &mut pgeo, &mut drv, &start, &mut rng).unwrap();
            let total = r.edep_photon_discards + r.edep_local + drv.tally;
            assert!((total - (ei - RM)).abs() < 1e-6, "total {total}");
        }
    }


    /// A synthetic `spinms.data` content: every record the same smooth,
    /// non-trivial shape.
    fn synthetic_spin_file() -> SpinFile {
        let n = 100 * 4 * (MAXE_SPIN + 1);
        let rec = SpinRecord {
            dum: [1.0, 0.9, 1.1],
            aux_o: 2.0,
            fmax: [3.0; MAXQ_SPIN + 1],
            i2: (0..512).map(|v| ((v * 97) % 30000) as i16 - 100).collect(),
        };
        SpinFile {
            espin_min: 1.0,
            espin_max: 1.0e3,
            b2spin_min: 0.4,
            b2spin_max: 0.99,
            records: alloc::vec![rec; n],
        }
    }

    fn spin_medium() -> SpinMediumInput {
        let t = toy_ms_medium();
        SpinMediumInput {
            elements: alloc::vec![(6.0, 1.0, 12.0), (8.0, 2.0, 16.0)],
            eke0: 11.0,
            eke1: 2.0,
            neke: 14,
            blcc: t.blcc,
            xcc: t.xcc,
            te: 0.01,
            ue: 2.0,
            esig: const_pwlf(5.0),
            ededx: const_pwlf(2.0),
        }
    }

    #[test]
    fn init_spin_builds_normalised_rejection_functions_and_continuous_fits() {
        let file = synthetic_spin_file();
        let (spin, fits) = init_spin(&file, &[spin_medium(), spin_medium()], RM).unwrap();
        assert_eq!(fits.len(), 2);
        // interpolation constants (real*4, as upstream)
        let dlener = (1.0f32 / 1000.0).ln();
        let _ = dlener;
        assert!((spin.espml as f64 - (1.0f64 / 1000.0).ln()).abs() < 1e-6, "espml {}", spin.espml);
        assert!((spin.dqq1i - 30.0).abs() < 1e-5);
        assert!(spin.b2spin_min == 0.4);
        // the rejection function has unit maximum for every (qel, i, j) and is non-negative
        for medium in 1..=2 {
            for iq in 0..2 {
                for i in 0..=MAXE_SPI1 {
                    for j in 0..=MAXQ_SPIN {
                        let row: Vec<f32> = (0..=MAXU_SPIN).map(|k| spin.spin_rej[spin_idx(medium, iq, i, j, k)]).collect();
                        let mx = row.iter().cloned().fold(0.0_f32, f32::max);
                        assert!((mx - 1.0).abs() < 1e-6, "max {mx}");
                        assert!(row.iter().all(|&v| v >= 0.0 && v.is_finite()));
                    }
                }
            }
        }
        // the piecewise-linear fits are continuous across bins and the last repeats the one before
        let f = &fits[0];
        for p in [&f.etae_ms, &f.etap_ms, &f.q1ce_ms, &f.q1cp_ms, &f.q2ce_ms, &f.q2cp_ms, &f.blcce] {
            for i in 1..13usize {
                let eil = ((i + 1) as f64 - 11.0) / 2.0;
                let left = p.a1[i] * eil + p.a0[i];
                let right = p.a1[i + 1] * eil + p.a0[i + 1];
                assert!((left - right).abs() < 1e-9 * left.abs().max(1.0), "continuity at {i}: {left} vs {right}");
                assert!(left.is_finite());
            }
            assert_eq!((p.a0[14], p.a1[14]), (p.a0[13], p.a1[13]));
        }
        // blcce is 1 below twice the Moller threshold (tau <= 2 tauc) and otherwise in (0, 1]
        for i in 1..=13usize {
            let eil = ((i + 1) as f64 - 11.0) / 2.0;
            let tau = eil.exp() / RM;
            let v = f.blcce.a1[i] * eil + f.blcce.a0[i];
            if tau <= 2.0 * 0.01 / RM {
                assert!((v - 1.0).abs() < 1e-9, "blcce below threshold at {i}: {v}");
            } else {
                assert!(v > 0.0 && v <= 1.0 + 1e-9, "blcce {v} at {i}");
            }
        }
        // a missing record is an error
        let mut short = file.clone();
        short.records.truncate(10);
        assert!(init_spin(&short, &[spin_medium()], RM).is_err());
    }

    #[test]
    fn init_spin_output_drives_a_scattering_step() {
        // the spin tables feed mscat/msdist: a spin-on step conserves norms
        let file = synthetic_spin_file();
        let (spin, fits) = init_spin(&file, &[spin_medium()], RM).unwrap();
        let ms = MsData::from_text(&synthetic_ms_text()).unwrap();
        let mut med = toy_ms_medium();
        med.eke0 = 11.0;
        med.eke1 = 2.0;
        let f = &fits[0];
        med.etae_ms = f.etae_ms.clone();
        med.etap_ms = f.etap_ms.clone();
        med.q1ce_ms = f.q1ce_ms.clone();
        med.q1cp_ms = f.q1cp_ms.clone();
        med.q2ce_ms = f.q2ce_ms.clone();
        med.q2cp_ms = f.q2cp_ms.clone();
        med.blcce = f.blcce.clone();
        let mut rng = Ranmar::new(2, 7);
        for _ in 0..20 {
            let inp = MsDistInput {
                e0: 1.0,
                eloss: 0.002,
                tustep: 0.01,
                rhof: 1.0,
                qel: 0,
                spin_effects: true,
                u0: 0.0,
                v0: 0.0,
                w0: 1.0,
                x0: 0.0,
                y0: 0.0,
                z0: 0.0,
            };
            let o = msdist_p2(&med, &ms, &spin, &inp, 1, &mut 0.0, &mut rng).unwrap();
            assert!((o.us * o.us + o.vs * o.vs + o.ws * o.ws - 1.0).abs() < 1e-6);
        }
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
    /// Smooth toy form factor `f(x) = 1/(1+x^2)` on a log grid.
    fn toy_form_factor() -> (Vec<f64>, Vec<f64>) {
        let mut x = vec![0.0];
        let mut f = vec![0.0];
        for i in 0..MXRAYFF {
            let xv = 1e-4 * (1e6_f64).powf(i as f64 / (MXRAYFF - 1) as f64);
            x.push(xv);
            f.push(10.0 / (1.0 + xv * xv));
        }
        (x, f)
    }

    #[test]
    fn zero_value_is_tiny_positive() {
        let z = zero_value();
        assert!(z > 0.0 && z < 1e-300, "{z}");
    }

    #[test]
    fn rayleigh_prepare_and_sampling_are_consistent() {
        let (x, mut f) = toy_form_factor();
        let (ge0, ge1, mge) = (1.0 - 1.0 * 20.0, 20.0 / 7.0 * 2.0, 200usize);
        let _ = (ge0, ge1);
        let emin = 0.001;
        let emax = 0.5; // xmax = 20.6*2*emax/prm stays inside the toy grid (<= 100)
        let (pe, mut d) = prepare_rayleigh_data(&x, &mut f, 50, emin, emax, 0.511);
        assert!(pe[1] > 0.0);
        for j in 2..=50 {
            assert!(pe[j] >= pe[j - 1] && pe[j] <= 1.0);
        }
        assert!((pe[50] - 1.0).abs() < 1e-12);
        // a flat pmax fit of 1 so the whole distribution is sampled
        d.pmax = Pwlf { a0: vec![1.0; mge + 1], a1: vec![0.0; mge + 1] };
        let mut rng = Ranmar::new(1802, 9373);
        let mut mean = 0.0;
        let n = 4000;
        for _ in 0..n {
            let (c, sn) = rayleigh_sampling(&d, 0.1, 0.1_f64.ln(), 5, &mut rng);
            assert!((-1.0..=1.0).contains(&c));
            assert!((c * c + sn * sn - 1.0).abs() < 1e-12);
            mean += c;
        }
        mean /= n as f64;
        // forward peaked: mean cosine clearly positive
        assert!(mean > 0.5, "{mean}");
    }

    #[test]
    fn rayleigh_sigma_is_positive_and_falls_with_energy() {
        let (mut x, mut f) = toy_form_factor();
        let s1 = rayleigh_sigma(0.05, &mut x, &mut f);
        let s2 = rayleigh_sigma(1.0, &mut x, &mut f);
        assert!(s1 > 0.0 && s2 > 0.0 && s1 > s2, "{s1} {s2}");
    }

    #[test]
    fn init_rayleigh_sampling_builds_pmax_fit() {
        let (x, f) = toy_form_factor();
        let mut ff = FormFactorTable { xval: x.clone(), aff: vec![vec![0.0; MXRAYFF + 1]] };
        for _ in 1..=100 {
            ff.aff.push(f.clone());
        }
        let (ge0, ge1, mge) = (1.0 - 0.01_f64.ln() * 20.0, 20.0, 90usize);
        let d = init_rayleigh_sampling(&[(8.0, 2.0), (1.0, 1.0)], &ff, ge0, ge1, mge, 0.511);
        assert_eq!(d.pmax.a0.len(), mge + 1);
        let e = 0.1_f64;
        let l = (e.ln() * ge1 + ge0) as usize;
        assert!(l >= 1 && l < mge);
        let p = d.pmax.eval(l, e.ln());
        assert!(p > 0.0 && p <= 1.0 + 1e-9, "{p}");
        let bad = FormFactorTable::from_text("1.0 2.0");
        assert!(bad.is_err());
    }

    #[test]
    fn photon_with_rayleigh_scattering_keeps_energy_balance() {
        let mut tables = photon_tables(0.0, 0.5, 3);
        let (x, mut f) = toy_form_factor();
        let (_, mut d) = prepare_rayleigh_data(&x, &mut f, 50, 0.001, 0.5, 0.511);
        d.pmax = Pwlf { a0: vec![1.0; 3], a1: vec![0.0; 3] };
        tables.media[1].rayleigh = Some(d);
        tables.media[1].cohe = const_pwlf(0.5);
        tables.regions[1].irayl = 1;
        let cfg = PhotonConfig { pair: PairConfig::default(), rr: None, prm: RM, vacdst: 100.0 };
        let mut rng = Ranmar::new(5, 6);
        for _ in 0..200 {
            let r = run_history(&tables, &cfg, 0.1, &mut rng);
            assert!(r.abs() < 1e-9, "residual {r}");
        }
    }

    fn toy_triplet_text() -> String {
        // 6 energies (MeV): two below 4.01 m = 2.044, four above; same
        // cross sections for every element
        let en = [1.5, 2.0, 3.0, 5.0, 10.0, 100.0];
        let mut t = String::from("6\n");
        t += &en.iter().map(|e| alloc::format!("{e}")).collect::<Vec<_>>().join(" ");
        t += "\n";
        for z in 1..=100 {
            t += &alloc::format!("Z={z}\n");
            t += &en.iter().map(|e| alloc::format!("{}", 1.0e-3 * (e - 1.0))).collect::<Vec<_>>().join(" ");
            t += "\n";
            t += &en.iter().map(|e| alloc::format!("{}", 1.0e-5 * e)).collect::<Vec<_>>().join(" ");
            t += "\n";
        }
        t
    }

    #[test]
    fn init_triplet_builds_a_sensible_fraction() {
        let comp = vec![vec![], vec![(8.0, 1.0)]];
        let td = init_triplet(&toy_triplet_text(), &comp, RM).unwrap();
        // fraction rises from 0 at threshold; check a mid bin
        let gle = RealMath::r_ln(5.0);
        let i = (td.dli_triplet * gle + td.bli_triplet) as usize;
        let f = td.a_triplet[1][i] * gle + td.b_triplet[1][i];
        // sigma_t/(sigma_p+sigma_t) at 5 MeV = 5e-5/(4e-3+5e-5)
        assert!((f - 5e-5 / (4e-3 + 5e-5)).abs() < 5e-4, "{f}");
        assert!(init_triplet("1000\n", &comp, RM).is_err());
        assert!(init_triplet("6\n1 2 3 4 5 6\n", &comp, RM).is_err());
    }

    #[test]
    fn sample_triplet_conserves_energy_and_momentum() {
        let mut rng = Ranmar::new(31, 41);
        let mut state = TripletState::default();
        for &e0 in &[3.0_f64, 5.0, 30.0] {
            for _ in 0..50 {
                let mut st = Stack::new(10);
                st.np = 1;
                st.e[1] = e0;
                st.wt[1] = 1.0;
                let (u, v, w) = (0.3_f64, 0.4_f64, (1.0_f64 - 0.25).sqrt());
                st.u[1] = u;
                st.v[1] = v;
                st.w[1] = w;
                sample_triplet(&mut st, &mut state, 50.0, RM, e0.ln(), &mut rng).unwrap();
                assert_eq!((st.np_old, st.np), (1, 3));
                let (mut et, mut px, mut py, mut pz) = (0.0, 0.0, 0.0, 0.0);
                for i in 1..=3 {
                    et += st.e[i];
                    let p = (st.e[i] * st.e[i] - RM * RM).sqrt();
                    px += p * st.u[i];
                    py += p * st.v[i];
                    pz += p * st.w[i];
                    assert!((norm(&st, i) - 1.0).abs() < 1e-9);
                }
                assert_eq!(st.iq[3], -1);
                assert!((et - e0 - RM).abs() < 1e-9 * e0, "{et} {e0}");
                assert!((px - e0 * u).abs() < 1e-7 && (py - e0 * v).abs() < 1e-7 && (pz - e0 * w).abs() < 1e-7);
            }
        }
        // at or below threshold nothing happens
        let mut st = Stack::new(10);
        st.np = 1;
        st.e[1] = 1.9;
        sample_triplet(&mut st, &mut state, 50.0, RM, 1.9_f64.ln(), &mut rng).unwrap();
        assert_eq!(st.np, 1);
    }

    #[test]
    fn pair_with_triplet_takes_the_triplet_branch() {
        let comp = vec![vec![], vec![(8.0, 1.0)]];
        let mut td = init_triplet(&toy_triplet_text(), &comp, RM).unwrap();
        // force the triplet fraction to 1 everywhere
        for i in 0..td.a_triplet[1].len() {
            td.a_triplet[1][i] = 0.0;
            td.b_triplet[1][i] = 1.0;
        }
        let mut state = TripletState::default();
        let cfg = PairConfig { itriplet: 1, ..Default::default() };
        let mut rng = Ranmar::new(8, 9);
        let mut st = Stack::new(10);
        st.np = 1;
        st.e[1] = 5.0;
        st.w[1] = 1.0;
        st.wt[1] = 1.0;
        let killed = pair_with_triplet(&mut st, &toy_pair_medium(), &cfg, RM, (&td, &mut state, 1, 50.0), &mut rng).unwrap();
        assert!(!killed);
        assert_eq!(st.np, 3);
        // without the data the plain entry point refuses
        let mut st = Stack::new(10);
        st.np = 1;
        st.e[1] = 5.0;
        assert!(matches!(pair(&mut st, &toy_pair_medium(), &cfg, RM, &mut rng), Err(EgsError::Unsupported(_))));
    }

    fn toy_nrc_pair() -> NrcPairData {
        // a density peaked at both ends of x, same for every record
        let comp = vec![vec![], vec![(8.0, 1.0)]];
        let rec: Vec<f32> = (0..NRC_PAIR_NXX).map(|i| 1.0 + ((i as f32) - 32.0).powi(2)).collect();
        init_nrc_pair(2.5, 100.0, NRC_PAIR_NEE as i32, NRC_PAIR_NXX as i32, &comp, |_| Some(rec.clone())).unwrap()
    }

    #[test]
    fn nrc_pair_tables_are_built_and_validated() {
        let nd = toy_nrc_pair();
        assert_eq!(nd.xdata.len(), NRC_PAIR_NXX);
        assert!((nd.xdata[0]).abs() < 1e-15 && (nd.xdata[NRC_PAIR_NXX - 1] - 1.0).abs() < 1e-12);
        assert_eq!(nd.tables[1].len(), NRC_PAIR_NEE + 1);
        assert!((nd.dle * nd.dlei - 1.0).abs() < 1e-12);
        let comp = vec![vec![], vec![(8.0, 1.0)]];
        assert!(init_nrc_pair(2.5, 100.0, 3, 65, &comp, |_| None).is_err());
        assert!(init_nrc_pair(2.5, 100.0, 84, 64, &comp, |_| None).is_err());
        assert!(init_nrc_pair(2.5, 100.0, 84, 65, &comp, |_| None).is_err());
        assert!(init_nrc_pair(2.5, 100.0, 84, 65, &comp, |_| Some(vec![1.0])).is_err());
    }

    #[test]
    fn alias_table_of_a_linear_density_samples_its_mean() {
        let xs: Vec<f64> = (0..=8).map(|i| i as f64 / 8.0).collect();
        // (a uniform density has every bin exactly at the average, which upstream's
        // loop leaves unfinalised, as `prepare_alias_sampling`'s port records)
        let fs: Vec<f64> = xs.iter().map(|x| 1.0 + 0.8 * x).collect();
        let (ws, ib) = super::super::egs_utilities_mortran::prepare_alias_table(&xs, &fs);
        let mut rng = Ranmar::new(3, 4);
        let (mut m, n) = (0.0, 20000);
        for _ in 0..n {
            let x = alias_sample1(&xs, &fs, &ws, &ib, &mut rng);
            assert!((0.0..=1.0).contains(&x));
            m += x;
        }
        // mean of f = 1 + 0.8 x on [0,1]: (1/2 + 0.8/3) / 1.4
        assert!((m / n as f64 - (0.5 + 0.8 / 3.0) / 1.4).abs() < 0.01);
    }

    #[test]
    fn pair_with_nrc_splits_energy_and_falls_back_above_range() {
        let nd = toy_nrc_pair();
        let cfg = PairConfig { pair_nrc: true, ..Default::default() };
        let mut rng = Ranmar::new(12, 13);
        for &e0 in &[1.5_f64, 5.0, 100.0] {
            for _ in 0..100 {
                let mut st = Stack::new(10);
                st.np = 1;
                st.e[1] = e0;
                st.w[1] = 1.0;
                st.wt[1] = 1.0;
                let killed = pair_with_nrc(&mut st, &toy_pair_medium(), &cfg, RM, (&nd, 1), &mut rng).unwrap();
                assert!(!killed && st.np == 2);
                assert!((st.e[1] + st.e[2] - e0).abs() < 1e-9 && st.iq[1] * st.iq[2] == -1);
            }
        }
        let mut st = Stack::new(10);
        st.np = 1;
        st.e[1] = 5.0;
        assert!(matches!(pair(&mut st, &toy_pair_medium(), &cfg, RM, &mut rng), Err(EgsError::Unsupported(_))));
    }

    fn toy_nist_brems_text() -> String {
        let en = [0.02, 0.1, 0.5, 1.0, 5.0, 20.0];
        let xs = [0.0, 0.2, 0.5, 0.8, 0.95, 1.0];
        let mut t = String::from("toy table\n6 6\n");
        t += &en.iter().map(|e| alloc::format!("{e}")).collect::<Vec<_>>().join(" ");
        t += "\n";
        t += &xs.iter().map(|x| alloc::format!("{x}")).collect::<Vec<_>>().join(" ");
        t += "\nblock header\n";
        for z in 1..=MXBREL {
            for x in &xs {
                for e in &en {
                    t += &alloc::format!("{} ", (z as f64) * 10.0 * (1.0 - 0.7 * x) * (1.0 + 0.1 * (e * 100.0_f64).ln()));
                }
                t += "\n";
            }
        }
        t
    }

    #[test]
    fn gauss_legendre_integrates_polynomials_on_zero_one() {
        let (x, w) = gauss_legendre(0.0, 1.0, MXGAUSS);
        let i4: f64 = x.iter().zip(&w).map(|(x, w)| w * x.powi(4)).sum();
        assert!((i4 - 0.2).abs() < 1e-13, "{i4}");
        let one: f64 = w.iter().sum();
        assert!((one - 1.0).abs() < 1e-13);
    }

    #[test]
    fn nist_brems_replaces_the_cross_sections_and_samples_photons() {
        let mut tabs = toy_electron_tables();
        let inputs = vec![NistBremsInput::default(), NistBremsInput { comp: vec![(8.0, 1.0, 16.0)], ap: 0.01, ae: 0.01 + RM, ue: 10.5 }];
        let before = tabs.media[1].esig.clone();
        let (se, sp) = init_nist_brems(&toy_nist_brems_text(), 1, &inputs, &mut tabs.media, RM).unwrap();
        let m = &tabs.media[1];
        assert!(se > 1e-15 && sp > 1e-15 && m.esig_e == se && m.psig_e == sp);
        assert_ne!(m.esig, before);
        let neke = m.esig.a1.len() - 1; // the toy tables carry one entry more than `N_EKE`
        assert!(m.esig.a1.len() == before.a1.len() && m.esig.a1[neke] == m.esig.a1[neke - 1]);
        let nb = m.brems.nist.as_ref().unwrap();
        assert_eq!(nb.bins.len(), MXBRES + 1);
        assert!(nb.emin >= 0.02 && nb.emax <= 20.0 && (nb.dle * nb.dlei - 1.0).abs() < 1e-12);
        assert_eq!(nb.bins[5].xdata.len(), MXBRXS + 1);
        assert!((nb.bins[5].xdata[MXBRXS] - 1.0).abs() < 1e-12);
        // BREMS using the tables
        let mut rng = Ranmar::new(2, 3);
        for &ekin in &[0.05_f64, 0.5, 4.0] {
            for _ in 0..50 {
                let mut st = Stack::new(10);
                st.np = 1;
                st.e[1] = ekin + RM;
                st.iq[1] = -1;
                st.wt[1] = 1.0;
                st.w[1] = 1.0;
                let cfg = BremsConfig { ibr_nist: 1, ..Default::default() };
                brems(&mut st, &m.brems, &cfg, ekin.ln(), RM, &mut rng).unwrap();
                assert_eq!(st.np, 2);
                assert!(st.e[2] >= 0.01 * (1.0 - 1e-9) && st.e[2] <= ekin * (1.0 + 1e-9), "{} {ekin}", st.e[2]);
                assert!((st.e[1] + st.e[2] - ekin - RM).abs() < 1e-12);
            }
        }
        // errors
        assert!(init_nist_brems("x", 3, &inputs, &mut tabs.media, RM).is_err());
        assert!(init_nist_brems("x\n99 6\n", 1, &inputs, &mut tabs.media, RM).is_err());
        assert!(init_nist_brems("x\n6 99\n", 1, &inputs, &mut tabs.media, RM).is_err());
        let mut bad = inputs.clone();
        bad[1].ae = 100.0;
        assert!(init_nist_brems(&toy_nist_brems_text(), 1, &bad, &mut tabs.media, RM).is_err());
    }

    fn toy_xsec_file(energies: &[f64], scale: f64, slope: f64) -> String {
        let mut t = String::new();
        for z in 1..=100 {
            t += &alloc::format!("{}\n", energies.len());
            for e in energies {
                t += &alloc::format!("{:.12} {:.12}\n", e.ln(), (scale * z as f64).ln() + slope * e.ln());
            }
        }
        t
    }

    fn toy_user_photon_input(compton: BoundComptonData, photonuc: bool) -> UserPhotonInput {
        UserPhotonInput {
            photo: toy_xsec_file(&[1e-4, 0.01, 0.01, 1.0, 20.0, 1000.0], 1.0, -2.5),
            pair: toy_xsec_file(&[1.1, 2.0, 5.0, 20.0, 100.0], 1e-3, 0.3),
            triplet: toy_xsec_file(&[2.2, 5.0, 20.0, 100.0], 1e-5, 0.3),
            rayleigh: toy_xsec_file(&[1e-4, 1.0, 1000.0], 0.5, -1.5),
            photonuc: photonuc.then(|| toy_xsec_file(&[0.001, 5.0, 1000.0], 1e-4, 0.0)),
            compton,
            mxge: 60,
            eadl_relax: false,
            rm: RM,
            form_factors: None,
        }
    }

    fn toy_user_photon_media() -> Vec<UserPhotonMedium> {
        vec![UserPhotonMedium::default(), UserPhotonMedium { comp: vec![(8.0, 2.0, 16.0), (1.0, 1.0, 1.0)], rho: 1.0, ap: 0.01, up: 20.0, rayleigh: false }]
    }

    #[test]
    fn user_photon_tables_are_consistent_with_the_data_bases() {
        let inp = toy_user_photon_input(BoundComptonData::None, true);
        let media = toy_user_photon_media();
        let t = init_user_photon(&inp, &media).unwrap();
        let m = &t.media[1];
        assert_eq!(m.mge, 60);
        // the fits reproduce the grid values: at every grid energy the two
        // adjacent intervals agree (continuous piecewise-linear fit)
        for i in 1..m.mge - 1 {
            let gle = (i as f64 + 1.0 - m.ge0) / m.ge1;
            let (a, b) = (m.gmfp.eval(i, gle), m.gmfp.eval(i + 1, gle));
            assert!((a - b).abs() <= 1e-9 * a.abs(), "{a} {b}");
            assert!(m.gbr1.eval(i, gle) >= 0.0 && m.gbr2.eval(i, gle) >= m.gbr1.eval(i, gle) - 1e-12);
            let c = m.cohe.eval(i, gle);
            assert!(c > 0.0 && c <= 1.0);
            let pn = m.photonuc.eval(i, gle);
            assert!(pn > 0.0 && pn <= 1.0);
        }
        // below the pair threshold there is no pair production
        let gle0 = (1.0 - m.ge0) / m.ge1;
        assert_eq!(m.gbr1.eval(1, gle0), 0.0);
        // photo-absorption edges: the duplicated energy 0.01 is found
        assert!((t.binding_energies[8][1] - 0.01).abs() < 1e-9);
        // a Rayleigh switch without form factors leaves the sampler empty
        assert!(m.rayleigh.is_none());
    }

    #[test]
    fn user_photon_bound_compton_and_failure_paths() {
        let media = toy_user_photon_media();
        // bound-Compton ratio file: emin, emax, ne, then one row per element Z
        let mut ratio = String::from("0.001 100.0 5\n");
        for _ in 1..=8 {
            ratio += "0.5 0.6 0.8 0.95 1.0\n";
        }
        let kn = init_user_photon(&toy_user_photon_input(BoundComptonData::None, false), &media).unwrap();
        let bc = init_user_photon(&toy_user_photon_input(BoundComptonData::Ratio(ratio), false), &media).unwrap();
        // bound Compton reduces the Compton cross section, so the mean free path grows
        let gle = (30.0 - kn.media[1].ge0) / kn.media[1].ge1;
        assert!(bc.media[1].gmfp.eval(30, gle) > kn.media[1].gmfp.eval(30, gle));
        let custom = init_user_photon(
            &toy_user_photon_input(BoundComptonData::Custom(toy_xsec_file(&[1e-4, 1.0, 1000.0], 0.05, -1.0)), false),
            &media,
        );
        assert!(custom.is_ok());
        // failures: photo data outside the grid range, a truncated file
        let mut bad = toy_user_photon_input(BoundComptonData::None, false);
        bad.photo = toy_xsec_file(&[0.05, 0.06, 0.07], 1.0, -2.5);
        assert!(init_user_photon(&bad, &media).is_err());
        let mut bad = toy_user_photon_input(BoundComptonData::None, false);
        bad.pair = String::from("3\n1.0 2.0\n");
        assert!(init_user_photon(&bad, &media).is_err());
        assert!(UserXsecTable::from_text("5000\n").is_err());
        // the low-energy series joins the closed form at k = 0.01 m c^2
        let (lo, hi) = (egs_kn_sigma0(0.00999 * RM, RM), egs_kn_sigma0(0.01001 * RM, RM));
        assert!((lo - hi).abs() / hi < 5e-3, "{lo} {hi}");
    }

    fn toy_eii_relax() -> RelaxData {
        let mut d = toy_relax();
        // element 1 gets K = 50 keV and L1 = 20 keV (above the 10 keV threshold),
        // L2, L3 below it
        d.binding_energies[1] = alloc::vec![0.0, 0.05, 0.02, 0.0008, 0.0008, 0.0005, 0.0001];
        d
    }

    fn toy_eii_text() -> String {
        toy_eii_text_with([8.0, 3.0])
    }

    fn toy_eii_text_with(fmaxes: [f64; 2]) -> String {
        let mut t = String::from("1\nskipped line\n100.0 250\n");
        // element 1 with two shells, each: fmax, then the cumulative probability on 250 bins
        t += "1 2\n";
        for fmax in fmaxes {
            t += &alloc::format!("{fmax}\n");
            let cum: Vec<String> = (1..=N_EII_BINS).map(|k| alloc::format!("{}", ((k as f64) / 250.0).sqrt())).collect();
            t += &cum.join(" ");
            t += "\n";
        }
        t
    }

    fn toy_eii_inputs() -> Vec<EiiMediumInput> {
        vec![EiiMediumInput::default(), EiiMediumInput { comp: vec![(1.0, 1.0, 1.0)], rho: 1.0, ae: 0.01 + RM, ap: 0.01 }]
    }

    #[test]
    fn eii_init_sets_shells_and_rewrites_the_cross_sections() {
        let mut tabs = toy_electron_tables();
        let relax_d = toy_eii_relax();
        let (esig0, ededx0) = (tabs.media[1].esig.clone(), tabs.media[1].ededx.clone());
        let d = eii_init(&toy_eii_text(), &toy_eii_inputs(), &relax_d, &mut tabs.media, RM, EiiReadings::default()).unwrap();
        assert!(d.flag);
        assert_eq!(d.nshells[1], 2);
        assert_eq!((d.nsh[1], d.first[1][0], d.no[1][0]), (2, 1, 2));
        assert!(d.cons[1] > 0.0 && (d.sh[1], d.sh[2]) == (1, 2) && d.z[1] == 1);
        assert_ne!(tabs.media[1].esig, esig0);
        assert_ne!(tabs.media[1].ededx, ededx0);
        let neke = tabs.media[1].esig.a1.len() - 1;
        assert_eq!(tabs.media[1].esig.a1[neke], tabs.media[1].esig.a1[neke - 1]);
        // no shell above threshold: EII turns itself off
        let mut none = toy_eii_relax();
        none.binding_energies[1] = alloc::vec![0.0, 0.001, 0.0008, 0.0008, 0.0008, 0.0005, 0.0001];
        let off = eii_init(&toy_eii_text(), &toy_eii_inputs(), &none, &mut tabs.media, RM, EiiReadings::default()).unwrap();
        assert!(!off.flag);
        // failures: wrong bin count, truncated data, a file lacking shells
        assert!(eii_init("0\n100.0 7\n", &toy_eii_inputs(), &relax_d, &mut tabs.media, RM, EiiReadings::default()).is_err());
        assert!(eii_init("0\n100.0 250\n1 2\n", &toy_eii_inputs(), &relax_d, &mut tabs.media, RM, EiiReadings::default()).is_err());
        assert!(eii_init("0\n100.0 250\n1 1\n", &toy_eii_inputs(), &relax_d, &mut tabs.media, RM, EiiReadings::default()).is_err());
        assert!(eii_init("x", &toy_eii_inputs(), &relax_d, &mut tabs.media, RM, EiiReadings::default()).is_err());
    }

    #[test]
    fn eii_sample_follows_upstream_including_its_primary_energy() {
        // UPSTREAM QUIRK, reproduced: `eii.mortran:345` sets the primary's
        // total energy to `peie - prm` (its kinetic energy), not `peie - wx`,
        // which looks like a typo and loses `prm` of energy per event. This
        // test pins the literal behaviour so phase 2 can settle it against a
        // built EGSnrc; it is NOT a statement that the physics is right.
        let relax_d = toy_eii_relax();
        let mut rng = Ranmar::new(77, 78);
        let (mut with_delta, mut without) = (0, 0);
        for &(t, ish, uj) in &[(1.0_f64, 1usize, 0.05_f64), (0.9, 2, 0.02)] {
            for _ in 0..100 {
                let mut st = Stack::new(40);
                st.np = 1;
                st.e[1] = t + RM;
                st.iq[1] = -1;
                st.wt[1] = 1.0;
                st.ir[1] = 3;
                (st.u[1], st.v[1], st.w[1]) = (0.0, 0.0, 1.0);
                let edep = eii_sample(&mut st, &relax_d, ish, 1, uj, 0.01 + RM, RM + 1e-4, 1e-4, RM, &mut rng).unwrap();
                assert!((st.e[1] - t).abs() < 1e-12, "the literal upstream primary energy");
                assert!(edep >= -1e-12);
                assert!((norm(&st, 1) - 1.0).abs() < 1e-9);
                if st.np > 1 && st.iq[2] == -1 && st.e[2] > 0.01 + RM {
                    with_delta += 1;
                } else {
                    without += 1;
                }
            }
        }
        assert!(with_delta > 0 && without > 0, "{with_delta} {without}");
        // below T = prm the literal formula has cos^2 < 0: refused, not NaN
        let mut st = Stack::new(40);
        st.np = 1;
        st.e[1] = 0.1 + RM;
        st.w[1] = 1.0;
        assert!(matches!(
            eii_sample(&mut st, &relax_d, 1, 1, 0.05, 0.01 + RM, RM + 1e-4, 1e-4, RM, &mut rng),
            Err(EgsError::Fatal(_))
        ));
    }

    #[test]
    fn moller_eii_select_picks_eii_events() {
        let mut tabs = toy_electron_tables();
        let relax_d = toy_eii_relax();
        let d = eii_init(&toy_eii_text_with([1.0, 0.5]), &toy_eii_inputs(), &relax_d, &mut tabs.media, RM, EiiReadings::default()).unwrap();
        // fixed Moller-type cross section so EII competes with it
        tabs.media[1].esig = const_pwlf(5.0);
        tabs.media[1].ebr1 = const_pwlf(0.3);
        let emed = &tabs.media[1];
        let mut rng = Ranmar::new(5, 6);
        let (mut hits, mut misses) = (0, 0);
        let t = 1.0_f64;
        let elke = t.ln();
        let l = (emed.eke1 * elke + emed.eke0) as usize;
        for _ in 0..400 {
            let mut st = Stack::new(40);
            st.np = 1;
            st.e[1] = t + RM;
            st.iq[1] = -1;
            st.wt[1] = 1.0;
            st.ir[1] = 3;
            (st.u[1], st.v[1], st.w[1]) = (0.0, 0.0, 1.0);
            match moller_eii_select(&mut st, &d, 1, &[(1.0, 1.0)], emed, &relax_d, elke, l, (RM + 1e-4, 1e-4), RM, 0.01, &mut rng).unwrap() {
                Some(edep) => {
                    hits += 1;
                    assert!(st.e[1] < t + RM && edep >= -1e-12);
                }
                None => misses += 1,
            }
        }
        assert!(hits > 0 && misses > 0, "{hits} {misses}");
        // EII off, or no EII shells in the medium: always carry on with Moller
        let mut off = d.clone();
        off.flag = false;
        let mut st = Stack::new(5);
        st.np = 1;
        st.e[1] = 2.0;
        assert_eq!(moller_eii_select(&mut st, &off, 1, &[(1.0, 1.0)], emed, &relax_d, 0.0, 5, (RM, 0.0), RM, 0.01, &mut rng).unwrap(), None);
    }

    #[test]
    fn eii_literal_readings_differ_from_the_intended_ones() {
        let relax_d = toy_eii_relax();
        let mut tabs = toy_electron_tables();
        let intended = eii_init(&toy_eii_text(), &toy_eii_inputs(), &relax_d, &mut tabs.media, RM, EiiReadings::default()).unwrap();
        let mut tabs2 = toy_electron_tables();
        let swapped = EiiReadings { binding_energy_swapped: true, ..Default::default() };
        let lit = eii_init(&toy_eii_text(), &toy_eii_inputs(), &relax_d, &mut tabs2.media, RM, swapped).unwrap();
        // shell 1 of Z = 1 reads the same word either way; shell 2 reads Z = 2's K energy
        assert_eq!(lit.a[1], intended.a[1]);
        assert_ne!(lit.a[2], intended.a[2]);
        // a different Ecc changes which shells use the EII cross section
        // (Ec = 0.03 MeV here, so Ecc = 0.04 -> 0.03 drops the 0.02 MeV shell)
        let mut inputs = toy_eii_inputs();
        inputs[1].ae = 0.03 + RM;
        let mut tabs3 = toy_electron_tables();
        eii_init(&toy_eii_text(), &inputs, &relax_d, &mut tabs3.media, RM, EiiReadings::default()).unwrap();
        let mut tabs4 = toy_electron_tables();
        let r = EiiReadings { ecc_ap: Some(0.04), ..Default::default() };
        eii_init(&toy_eii_text(), &inputs, &relax_d, &mut tabs4.media, RM, r).unwrap();
        assert_ne!(tabs3.media[1].esig, tabs4.media[1].esig);
        // the shell-type bin index agrees with the shell index when type == index
        let mut tabs5 = toy_electron_tables();
        let r = EiiReadings { bin_index_by_shell_type: true, ..Default::default() };
        eii_init(&toy_eii_text(), &toy_eii_inputs(), &relax_d, &mut tabs5.media, RM, r).unwrap();
        assert_eq!(tabs5.media[1].esig, tabs.media[1].esig);
    }

    #[test]
    fn electr_uses_the_eii_selection_when_eii_is_on() {
        let mut rng = Ranmar::new(123, 124);
        let mut tables = toy_electron_tables();
        let relax_d = toy_eii_relax();
        let d = eii_init(&toy_eii_text_with([1.0, 0.5]), &toy_eii_inputs(), &relax_d, &mut tables.media, RM, EiiReadings::default()).unwrap();
        // the toy rewrite can drive esig negative (tiny Ec); restore a positive
        // cross section so the test exercises the wiring, not the toy numbers
        tables.media[1].esig = const_pwlf(5.0);
        tables.media[1].ededx = const_pwlf(2.0);
        tables.eii = Some(ElectrEii { data: d, comp: vec![Vec::new(), vec![(1.0, 1.0)]], relax: relax_d });
        let cfg = electr_cfg(true, true, false);
        let (mut deposited, mut fatal, mut done) = (0.0, 0, 0);
        for _ in 0..300 {
            let mut st = Stack::new(200);
            st.np = 1;
            st.e[1] = 2.0 + RM;
            st.iq[1] = -1;
            st.wt[1] = 1.0;
            st.ir[1] = 1;
            st.z[1] = 1.0;
            (st.u[1], st.v[1], st.w[1]) = (0.0, 0.0, 1.0);
            let mut geo = Slab { length: 3.0 };
            let mut counters = ElectrCounters::default();
            match electr(&mut st, &tables, &cfg, &mut geo, &mut counters, &mut rng) {
                Ok(r) => {
                    deposited += r.edep_eii;
                    done += 1;
                }
                // upstream's primary energy after EII makes cos^2 < 0 below T = prm
                Err(EgsError::Fatal(_)) => fatal += 1,
                Err(e) => panic!("{e:?}"),
            }
        }
        assert!(deposited > 0.0 || fatal > 0, "no EII event in {done} runs");
    }

    /// The bytes `spinms.data` would hold for `f`, in either byte order.
    fn spin_file_bytes(f: &SpinFile, little: bool) -> Vec<u8> {
        let f32b = |v: f32| if little { v.to_le_bytes() } else { v.to_be_bytes() };
        let i16b = |v: i16| if little { v.to_le_bytes() } else { v.to_be_bytes() };
        let mut out = Vec::new();
        let mut head = alloc::vec![b' '; SPIN_RECORD_BYTES];
        head[..9].copy_from_slice(b"toy spins");
        head[32..36].copy_from_slice(if little { b"1234" } else { b"4321" });
        for (k, v) in [f.espin_min, f.espin_max, f.b2spin_min, f.b2spin_max].into_iter().enumerate() {
            head[36 + 4 * k..40 + 4 * k].copy_from_slice(&f32b(v));
        }
        out.extend_from_slice(&head);
        for r in &f.records {
            let mut rec = Vec::new();
            for v in r.dum {
                rec.extend_from_slice(&f32b(v));
            }
            rec.extend_from_slice(&f32b(r.aux_o));
            for v in r.fmax {
                rec.extend_from_slice(&f32b(v));
            }
            for &v in &r.i2 {
                rec.extend_from_slice(&i16b(v));
            }
            assert_eq!(rec.len(), SPIN_RECORD_BYTES);
            out.extend_from_slice(&rec);
        }
        out
    }

    #[test]
    fn spin_file_decoder_round_trips_both_byte_orders() {
        let f = synthetic_spin_file();
        for little in [true, false] {
            let back = SpinFile::from_bytes(&spin_file_bytes(&f, little)).unwrap();
            assert_eq!(back, f);
        }
        // errors
        assert!(SpinFile::from_bytes(&[0u8; 10]).is_err());
        let mut bad = spin_file_bytes(&f, true);
        bad[32..36].copy_from_slice(b"????");
        assert!(SpinFile::from_bytes(&bad).is_err());
        let mut cut = spin_file_bytes(&f, true);
        cut.pop();
        assert!(SpinFile::from_bytes(&cut).is_err());
    }

    /// The bytes of a toy `pair_nrc1.data` for elements `1..=nz`.
    fn toy_nrc_pair_bytes(nz: usize, little: bool) -> Vec<u8> {
        let f32b = |v: f32| if little { v.to_le_bytes() } else { v.to_be_bytes() };
        let i32b = |v: i32| if little { v.to_le_bytes() } else { v.to_be_bytes() };
        let mut out = Vec::new();
        out.extend_from_slice(&f32b(2.5));
        out.extend_from_slice(&f32b(100.0));
        out.extend_from_slice(&i32b(NRC_PAIR_NEE as i32));
        out.extend_from_slice(&i32b(NRC_PAIR_NXX as i32));
        out.push(if little { 0 } else { 1 });
        out.resize(NRC_PAIR_RECORD_BYTES, 0);
        for _ in 0..nz * NRC_PAIR_NEE {
            for i in 0..NRC_PAIR_NXX {
                out.extend_from_slice(&f32b(1.0 + ((i as f32) - 32.0).powi(2)));
            }
        }
        out
    }

    #[test]
    fn nrc_pair_file_decoder_matches_the_closure_route() {
        let comp = vec![vec![], vec![(8.0, 1.0)]];
        let reference = toy_nrc_pair();
        for little in [true, false] {
            let file = NrcPairFile::from_bytes(&toy_nrc_pair_bytes(8, little)).unwrap();
            assert_eq!((file.ne, file.nb), (NRC_PAIR_NEE as i32, NRC_PAIR_NXX as i32));
            assert_eq!(file.init(&comp).unwrap(), reference);
        }
        let file = NrcPairFile::from_bytes(&toy_nrc_pair_bytes(8, true)).unwrap();
        assert!(file.record(0).is_none() && file.record(1 + 8 * NRC_PAIR_NEE + 5).is_none());
        // too few elements in the file for Z = 8
        let short = NrcPairFile::from_bytes(&toy_nrc_pair_bytes(2, true)).unwrap();
        assert!(short.init(&comp).is_err());
        assert!(NrcPairFile::from_bytes(&[0u8; 10]).is_err());
    }

    #[test]
    fn gas_header_of_a_shipped_pegs4_file_parses() {
        // the header line of `521icru.pegs4dat` (EGSnrc pegs4/data), verbatim
        let line = " MIXT,RHO= 1.2048E-03,NE= 4,GASP= 1.0000E+00, IUNRST=0, EPSTFL=1, IAPRIM=1";
        assert_eq!(parse_medium_header(line).unwrap(), (1.2048e-3, 4, 0, 1, 1));
    }

    /// Phase 2: the data readers on the REAL EGSnrc data files (not vendored
    /// here: `EGSNRC_HEN_HOUSE_DATA`, default
    /// `/tmp/claude-0/up/egsnrc/HEN_HOUSE/data`, plus `.../pegs4/pgs4form.dat`).
    /// Checks structure and sanity, not agreement with EGSnrc output.
    ///
    /// Results (2026-11-21): `spinms.data` is exactly `1 + 100*4*16` records of
    /// 1104 bytes (7066704 bytes) and decodes; `pair_nrc1.data` is exactly
    /// `1 + 100*84` records of 260 bytes (2184260 bytes), header `ne = 84`,
    /// `nb = 65`, and builds the alias tables for water; `eii_ik.data`,
    /// `nist_brems.data`, `nrc_brems.data`, `triplet.data`, `epdl_*.data`,
    /// `compton_sigma.data` parse. The user-photon tables for water give a
    /// no-Rayleigh mean free path at 1 MeV within 2 % of 1/(0.0707 cm^2/g)
    /// (a value recalled from NIST XCOM, not read from the repo).
    #[test]
    fn real_hen_house_data_files_when_available() {
        let dir = std::env::var("EGSNRC_HEN_HOUSE_DATA").unwrap_or_else(|_| String::from("/tmp/claude-0/up/egsnrc/HEN_HOUSE/data"));
        let Ok(spin_bytes) = std::fs::read(format!("{dir}/spinms.data")) else {
            eprintln!("EGSnrc data not found in {dir}: real-data test skipped");
            return;
        };
        let text = |f: &str| std::fs::read_to_string(format!("{dir}/{f}")).unwrap();
        // spinms.data
        let sf = SpinFile::from_bytes(&spin_bytes).unwrap();
        eprintln!("spinms.data: {} records, E {}..{} keV, b2 {}..{}", sf.records.len(), sf.espin_min, sf.espin_max, sf.b2spin_min, sf.b2spin_max);
        assert_eq!(sf.records.len(), 100 * 4 * (MAXE_SPIN + 1));
        assert!(sf.espin_min > 0.0 && sf.espin_max > sf.espin_min && sf.b2spin_max > sf.b2spin_min);
        // pair_nrc1.data
        let nf = NrcPairFile::from_bytes(&std::fs::read(format!("{dir}/pair_nrc1.data")).unwrap()).unwrap();
        eprintln!("pair_nrc1.data: emin {} emax {} ne {} nb {}", nf.emin, nf.emax, nf.ne, nf.nb);
        let water = vec![vec![], vec![(1.0, 2.0), (8.0, 1.0)]];
        let nd = nf.init(&water).unwrap();
        assert_eq!(nd.tables[1].len(), NRC_PAIR_NEE + 1);
        // triplet.data
        let td = init_triplet(&text("triplet.data"), &water, RM).unwrap();
        assert!(td.a_triplet[1].iter().any(|v| *v != 0.0));
        // user photon set for water, with Rayleigh form factors and bound Compton
        let ff = FormFactorTable::from_text(&std::fs::read_to_string(format!("{dir}/../pegs4/pgs4form.dat")).unwrap()).unwrap();
        let inp = UserPhotonInput {
            photo: text("epdl_photo.data"),
            pair: text("epdl_pair.data"),
            triplet: text("epdl_triplet.data"),
            rayleigh: text("epdl_rayleigh.data"),
            photonuc: Some(text("iaea_photonuc.data")),
            compton: BoundComptonData::Ratio(text("compton_sigma.data")),
            mxge: 2000,
            eadl_relax: false,
            rm: RM,
            form_factors: Some(ff),
        };
        let media = vec![
            UserPhotonMedium::default(),
            UserPhotonMedium { comp: vec![(1.0, 2.0, 1.00794), (8.0, 1.0, 15.9994)], rho: 1.0, ap: 0.01, up: 50.0, rayleigh: true },
        ];
        let t = init_user_photon(&inp, &media).unwrap();
        let m = &t.media[1];
        let gle = 1.0_f64.ln();
        let l = (m.ge1 * gle + m.ge0) as usize;
        let mfp = m.gmfp.eval(l, gle);
        eprintln!("water, 1 MeV: no-Rayleigh mean free path {mfp} cm (1/0.0707 = {})", 1.0 / 0.0707);
        assert!((mfp * 0.0707 - 1.0).abs() < 0.02, "{mfp}");
        assert!(m.rayleigh.is_some());
        // the EDGSET readers on photo_relax.data and photo_cs.data, then eii_ik.data
        let (relax_d, _photo_d) = edgset_from_text(&text("photo_relax.data"), &text("photo_cs.data")).unwrap();
        eprintln!("Pb binding energies (MeV): {:?}", &relax_d.binding_energies[82][1..=4]);
        let mut etabs = toy_electron_tables();
        let ein = vec![EiiMediumInput::default(), EiiMediumInput { comp: vec![(82.0, 1.0, 207.2)], rho: 11.34, ae: 0.521, ap: 0.01 }];
        let ed = eii_init(&text("eii_ik.data"), &ein, &relax_d, &mut etabs.media, RM, EiiReadings::default());
        eprintln!("eii_ik.data: {:?}", ed.as_ref().map(|d| (d.flag, d.nshells[82], d.nsh[1])).map_err(|e| e.clone()));
        assert!(ed.is_ok());
        // eii_ik.data and the NIST/NRC bremsstrahlung tables parse (toy electron tables, lead)
        let mut tabs = toy_electron_tables();
        let nin = vec![NistBremsInput::default(), NistBremsInput { comp: vec![(82.0, 1.0, 207.2)], ap: 0.01, ae: 0.01 + RM, ue: 10.5 }];
        for (file, which) in [("nist_brems.data", 1), ("nrc_brems.data", 2)] {
            let r = init_nist_brems(&text(file), which, &nin, &mut tabs.media, RM);
            eprintln!("{file}: {:?}", r.as_ref().map(|v| *v).map_err(|e| e.clone()));
            assert!(r.is_ok(), "{file}");
        }
    }

    /// Phase 2: the user-photon tables (`egs_init_user_photon`) against the
    /// tables a REAL EGSnrc `HATCH` builds. `EGSNRC_REFERENCE_PHOTON_TABLES` is
    /// the `tables_photon.txt` written by a copy of `tutor1.mortran` (Ta, 20 MeV
    /// electrons, `tutor_data.pegs4dat`, default options: photon series `xcom`,
    /// bound Compton, EADL relaxation) that dumps `GMFP0/1`, `GBR10/11`,
    /// `GBR20/21`, `COHE0/1` after `CALL HATCH`. The data files come from
    /// `EGSNRC_HEN_HOUSE_DATA`. Reports the worst relative difference per
    /// array; asserts only the grid constants (`mge`, `ge0`, `ge1`).
    #[test]
    fn differential_user_photon_tables_against_compiled_egsnrc() {
        let Ok(reference) = std::env::var("EGSNRC_REFERENCE_PHOTON_TABLES") else {
            eprintln!("EGSNRC_REFERENCE_PHOTON_TABLES not set: differential test skipped");
            return;
        };
        let dir = std::env::var("EGSNRC_HEN_HOUSE_DATA").unwrap_or_else(|_| String::from("/tmp/claude-0/up/egsnrc/HEN_HOUSE/data"));
        let text = |f: &str| std::fs::read_to_string(format!("{dir}/{f}")).unwrap();
        let ff = FormFactorTable::from_text(&std::fs::read_to_string(format!("{dir}/../pegs4/pgs4form.dat")).unwrap()).unwrap();
        let inp = UserPhotonInput {
            photo: text("xcom_photo.data"),
            pair: text("xcom_pair.data"),
            triplet: text("xcom_triplet.data"),
            rayleigh: text("xcom_rayleigh.data"),
            photonuc: None,
            compton: BoundComptonData::Ratio(text("compton_sigma.data")),
            mxge: 2000,
            eadl_relax: true,
            rm: RM,
            form_factors: Some(ff),
        };
        // tutor_data.pegs4dat: TA, Z = 73, A = 180.948, rho = 16.6, AP = 0.01, UP = 50
        let media = vec![
            UserPhotonMedium::default(),
            UserPhotonMedium { comp: vec![(73.0, 1.0, 180.948)], rho: 16.6, ap: 0.01, up: 50.0, rayleigh: true },
        ];
        let t = init_user_photon(&inp, &media).unwrap();
        let m = &t.media[1];
        let refs = std::fs::read_to_string(reference).unwrap();
        let mut lines = refs.lines();
        let mut scalar = |name: &str| -> f64 {
            let l = lines.next().unwrap();
            assert!(l.trim_start().starts_with(name), "{l}");
            l.split_whitespace().nth(1).unwrap().parse().unwrap()
        };
        assert_eq!(scalar("mge") as usize, m.mge);
        let (ge0, ge1) = (scalar("ge0"), scalar("ge1"));
        eprintln!("ge0 port {} ref {ge0}; ge1 port {} ref {ge1}", m.ge0, m.ge1);
        let (_ap, _up) = (scalar("ap"), scalar("up"));
        let names = ["gmfp0", "gmfp1", "gbr10", "gbr11", "gbr20", "gbr21", "cohe0", "cohe1"];
        let mine = [&m.gmfp.a0, &m.gmfp.a1, &m.gbr1.a0, &m.gbr1.a1, &m.gbr2.a0, &m.gbr2.a1, &m.cohe.a0, &m.cohe.a1];
        let mut worst = [0.0_f64; 8];
        let mut worst_at = [0usize; 8];
        // the fitted FUNCTIONS (a0 + a1 * gle at the interval's own grid energy), the
        // quantity the transport uses, as the slopes alone amplify cancellation
        let mut worst_f = [0.0_f64; 4];
        let mut n = 0;
        for l in lines {
            let v: Vec<f64> = l.split_whitespace().map(|t| t.parse().unwrap()).collect();
            let i = v[0] as usize;
            n += 1;
            let gle = (i as f64 - ge0) / ge1;
            for f in 0..4 {
                let (a, b) = (mine[2 * f][i] + mine[2 * f + 1][i] * gle, v[1 + 2 * f] + v[2 + 2 * f] * gle);
                // probabilities and a mean free path of order 1e-3 to 1e3: mixed absolute/relative
                worst_f[f] = worst_f[f].max((a - b).abs() / b.abs().max(1e-6));
            }
            for k in 0..8 {
                let (a, b) = (mine[k][i], v[k + 1]);
                let scale = b.abs().max(1e-300);
                let rel = (a - b).abs() / scale;
                if rel > worst[k] {
                    worst[k] = rel;
                    worst_at[k] = i;
                }
            }
        }
        eprintln!("{n} grid points compared");
        for k in 0..8 {
            eprintln!("{}: worst relative difference {:e} at interval {}", names[k], worst[k], worst_at[k]);
        }
        for (f, name) in ["gmfp", "gbr1", "gbr2", "cohe"].iter().enumerate() {
            eprintln!("{name} function value: worst relative difference {:e}", worst_f[f]);
        }
        assert_eq!(n, m.mge);
        assert!((m.ge0 - ge0).abs() <= 1e-9 * ge0.abs() && (m.ge1 - ge1).abs() <= 1e-9 * ge1.abs());
    }

    /// Phase 2: the electron tables of `hatch_media` (PEGS4 intake, `DUNIT`
    /// scaling, `fix_brems`, `mscati`) against the tables a REAL EGSnrc
    /// `HATCH` builds for the same data set: `EGSNRC_REFERENCE_ELECTRON_TABLES`
    /// is the `tables_electron.txt` of the `difftab` copy of `tutor1`
    /// (`tutor_data.pegs4dat`, medium TA, default options), `EGSNRC_PEGS4_FILE`
    /// the data set (default `.../HEN_HOUSE/pegs4/data/tutor_data.pegs4dat`).
    /// Reports the worst relative difference per table; asserts that the
    /// scalar table sizes agree.
    #[test]
    fn differential_electron_tables_against_compiled_egsnrc() {
        let Ok(reference) = std::env::var("EGSNRC_REFERENCE_ELECTRON_TABLES") else {
            eprintln!("EGSNRC_REFERENCE_ELECTRON_TABLES not set: differential test skipped");
            return;
        };
        let path = std::env::var("EGSNRC_PEGS4_FILE").unwrap_or_else(|_| String::from("/tmp/claude-0/up/egsnrc/HEN_HOUSE/pegs4/data/tutor_data.pegs4dat"));
        let mut media = parse_pegs4_media(&std::fs::read_to_string(path).unwrap(), &["TA"]).unwrap();
        media[0].apply_dunit(1.0);
        let r = hatch_media(&media, &HatchConfig { ecutmn: 1.5, ..Default::default() });
        let e = &r.media[0].electron;
        let refs = std::fs::read_to_string(reference).unwrap();
        let mut lines = refs.lines();
        let mut scalar = |name: &str| -> f64 {
            let l = lines.next().unwrap();
            assert!(l.trim_start().starts_with(name), "{l}");
            l.split_whitespace().nth(1).unwrap().parse().unwrap()
        };
        let meke = scalar("meke") as usize;
        assert_eq!(meke, e.esig.a0.len() - 1);
        let rel = |a: f64, b: f64| (a - b).abs() / b.abs().max(1e-300);
        eprintln!("eke0 {:e} eke1 {:e}", rel(e.eke0, scalar("eke0")), rel(e.eke1, scalar("eke1")));
        eprintln!("blcc {:e} xcc {:e}", rel(e.ms.blcc, scalar("blcc")), rel(e.ms.xcc, scalar("xcc")));
        eprintln!("esig_e {:e} psig_e {:e}", rel(e.esig_e, scalar("esig_e")), rel(e.psig_e, scalar("psig_e")));
        eprintln!("te {:e} thmoll {:e}", rel(e.te, scalar("te")), rel(e.thmoll, scalar("thmoll")));
        let names = [
            "ededx0", "ededx1", "pdedx0", "pdedx1", "esig0", "esig1", "psig0", "psig1", "ebr10", "ebr11", "pbr10", "pbr11", "pbr20", "pbr21", "tmxs0",
            "tmxs1", "range_e", "range_p", "e_array",
        ];
        let mut worst = [0.0_f64; 19];
        for l in lines {
            let v: Vec<f64> = l.split_whitespace().map(|t| t.parse().unwrap()).collect();
            let i = v[0] as usize;
            let mine_row: Vec<f64> = if v.len() == 13 {
                vec![e.ededx.a0[i], e.ededx.a1[i], e.pdedx.a0[i], e.pdedx.a1[i], e.esig.a0[i], e.esig.a1[i], e.psig.a0[i], e.psig.a1[i], e.ebr1.a0[i], e.ebr1.a1[i], e.pbr1.a0[i], e.pbr1.a1[i]]
            } else {
                vec![e.pbr2.a0[i], e.pbr2.a1[i], e.tmxs.a0[i], e.tmxs.a1[i], e.range_ep[0][i], e.range_ep[1][i], e.e_array[i]]
            };
            let off = if v.len() == 13 { 0 } else { 12 };
            for (k, (a, b)) in mine_row.iter().zip(&v[1..]).enumerate() {
                let scale = b.abs().max(1e-12);
                worst[off + k] = worst[off + k].max((a - b).abs() / scale);
            }
        }
        for (n, w) in names.iter().zip(worst) {
            eprintln!("{n}: worst relative difference {w:e}");
        }
    }

    /// Phase 2: the spin fits of `init_spin` (decoded from the REAL
    /// `spinms.data`) against the `ETAE_MS`, `ETAP_MS`, `Q1C*_MS`, `Q2C*_MS`
    /// and `BLCCE` arrays of a built EGSnrc `HATCH` for the same medium
    /// (`EGSNRC_REFERENCE_SPIN_TABLES` = `tables_spin.txt` of the `difftab` run;
    /// `EGSNRC_HEN_HOUSE_DATA`, `EGSNRC_PEGS4_FILE` as above). Reports worst
    /// relative differences; asserts nothing numerical.
    #[test]
    fn differential_spin_fits_against_compiled_egsnrc() {
        let Ok(reference) = std::env::var("EGSNRC_REFERENCE_SPIN_TABLES") else {
            eprintln!("EGSNRC_REFERENCE_SPIN_TABLES not set: differential test skipped");
            return;
        };
        let dir = std::env::var("EGSNRC_HEN_HOUSE_DATA").unwrap_or_else(|_| String::from("/tmp/claude-0/up/egsnrc/HEN_HOUSE/data"));
        let path = std::env::var("EGSNRC_PEGS4_FILE").unwrap_or_else(|_| String::from("/tmp/claude-0/up/egsnrc/HEN_HOUSE/pegs4/data/tutor_data.pegs4dat"));
        let mut media = parse_pegs4_media(&std::fs::read_to_string(path).unwrap(), &["TA"]).unwrap();
        media[0].apply_dunit(1.0);
        let r = hatch_media(&media, &HatchConfig { ecutmn: 1.5, ..Default::default() });
        let e = &r.media[0].electron;
        let file = SpinFile::from_bytes(&std::fs::read(format!("{dir}/spinms.data")).unwrap()).unwrap();
        let input = SpinMediumInput {
            elements: media[0].elements.iter().map(|x| (x.zelem, x.pz, x.wa)).collect(),
            eke0: e.eke0,
            eke1: e.eke1,
            neke: e.esig.a0.len() - 1,
            blcc: e.ms.blcc,
            xcc: e.ms.xcc,
            te: e.te,
            ue: media[0].ue,
            esig: e.esig.clone(),
            ededx: e.ededx.clone(),
        };
        let (_data, fits) = init_spin(&file, &[input], RM).unwrap();
        let f = &fits[0];
        let names = ["etae0", "etae1", "etap0", "etap1", "q1ce0", "q1ce1", "q1cp0", "q1cp1", "q2ce0", "q2ce1", "q2cp0", "q2cp1", "blcce0", "blcce1"];
        let mut worst = [0.0_f64; 14];
        let refs = std::fs::read_to_string(reference).unwrap();
        let mut pending: Vec<f64> = Vec::new();
        for l in refs.lines() {
            let v: Vec<f64> = l.split_whitespace().map(|t| t.parse().unwrap()).collect();
            let i = v[0] as usize;
            pending.extend_from_slice(&v[1..]);
            if pending.len() == 14 {
                let mine = [
                    f.etae_ms.a0[i], f.etae_ms.a1[i], f.etap_ms.a0[i], f.etap_ms.a1[i], f.q1ce_ms.a0[i], f.q1ce_ms.a1[i], f.q1cp_ms.a0[i], f.q1cp_ms.a1[i],
                    f.q2ce_ms.a0[i], f.q2ce_ms.a1[i], f.q2cp_ms.a0[i], f.q2cp_ms.a1[i], f.blcce.a0[i], f.blcce.a1[i],
                ];
                for k in 0..14 {
                    worst[k] = worst[k].max((mine[k] - pending[k]).abs() / pending[k].abs().max(1e-6));
                }
                pending.clear();
            }
        }
        for (n, w) in names.iter().zip(worst) {
            eprintln!("{n}: worst relative (floor 1e-6) difference {w:e}");
        }
    }

    /// Phase 2: the Rayleigh sampling tables (`init_rayleigh_sampling` ->
    /// `prepare_rayleigh_data`) against `XGRID`, `FCUM`, `B_ARRAY`, `C_ARRAY`,
    /// `I_ARRAY`, `PMAX0/1` of a built EGSnrc `HATCH` (Ta;
    /// `EGSNRC_REFERENCE_RAYLEIGH_TABLES` = `tables_rayleigh.txt` of the
    /// `difftab` run; `EGSNRC_HEN_HOUSE_DATA` for `pgs4form.dat`). Reports the
    /// worst relative differences; the integer `I_ARRAY` must be equal.
    #[test]
    fn differential_rayleigh_tables_against_compiled_egsnrc() {
        let Ok(reference) = std::env::var("EGSNRC_REFERENCE_RAYLEIGH_TABLES") else {
            eprintln!("EGSNRC_REFERENCE_RAYLEIGH_TABLES not set: differential test skipped");
            return;
        };
        let dir = std::env::var("EGSNRC_HEN_HOUSE_DATA").unwrap_or_else(|_| String::from("/tmp/claude-0/up/egsnrc/HEN_HOUSE/data"));
        let ff = FormFactorTable::from_text(&std::fs::read_to_string(format!("{dir}/../pegs4/pgs4form.dat")).unwrap()).unwrap();
        // Ta as in tutor_data: 2000 photon grid points over AP = 0.01 .. UP = 50 MeV
        let ge1 = 1999.0 / (50.0_f64 / 0.01).ln();
        let ge0 = 1.0 - ge1 * 0.01_f64.ln();
        let d = init_rayleigh_sampling(&[(73.0, 1.0)], &ff, ge0, ge1, 2000, RM);
        let refs = std::fs::read_to_string(reference).unwrap();
        let (mut wx, mut wf, mut wb, mut wc, mut wp0, mut wp1) = (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
        let mut bad_i = 0;
        let rel = |a: f64, b: f64| (a - b).abs() / b.abs().max(1e-12);
        for (n, l) in refs.lines().enumerate() {
            let v: Vec<f64> = l.split_whitespace().map(|t| t.parse().unwrap()).collect();
            let i = v[0] as usize;
            if n < 100 {
                wx = wx.max(rel(d.xgrid[i], v[1]));
                wf = wf.max(rel(d.fcum[i], v[2]));
                wb = wb.max(rel(d.b_array[i], v[3]));
                wc = wc.max(rel(d.c_array[i], v[4]));
                if d.i_array[i] != v[5] as i32 {
                    bad_i += 1;
                }
            } else {
                wp0 = wp0.max(rel(d.pmax.a0[i], v[1]));
                wp1 = wp1.max(rel(d.pmax.a1[i], v[2]));
            }
        }
        eprintln!("xgrid {wx:e} fcum {wf:e} b {wb:e} c {wc:e} pmax0 {wp0:e} pmax1 {wp1:e}; i_array mismatches {bad_i}");
        assert_eq!(bad_i, 0);
    }

    /// The `tutor1` geometry (`HOWFAR`, `HOWNEAR` of `tutor1.mortran`): vacuum,
    /// a 1 mm plate (region 2), vacuum; every discarded particle is logged as
    /// `(iq, kinetic energy, w)`, which is what its `AUSGAB(3)` prints.
    #[derive(Clone)]
    struct Tutor1Geo {
        zbound: f64,
        prm: f64,
        log: std::sync::Arc<std::sync::RwLock<Vec<(i32, f64, f64)>>>,
    }

    impl Geometry for Tutor1Geo {
        fn howfar(&mut self, stack: &Stack, st: &mut HowfarState) {
            let np = stack.np;
            let (z, w) = (stack.z[np], stack.w[np]);
            let mut discard = |this: &Tutor1Geo, st: &mut HowfarState| {
                st.idisc = 1;
                let ek = if stack.iq[np] == 0 { stack.e[np] } else { stack.e[np] - this.prm };
                this.log.write().unwrap().push((stack.iq[np], ek, w));
            };
            match stack.ir[np] {
                3 => discard(self, st),
                2 => {
                    if w > 0.0 {
                        let tval = (self.zbound - z) / w;
                        if tval <= st.ustep {
                            st.ustep = tval;
                            st.irnew = 3;
                        }
                    } else if w < 0.0 {
                        let tval = -z / w;
                        if tval <= st.ustep {
                            st.ustep = tval;
                            st.irnew = 1;
                        }
                    }
                }
                1 => {
                    if w > 0.0 {
                        st.ustep = 0.0;
                        st.irnew = 2;
                    } else {
                        discard(self, st);
                    }
                }
                _ => {}
            }
        }
    }

    impl ElectronGeometry for Tutor1Geo {
        fn hownear(&mut self, stack: &Stack) -> f64 {
            let np = stack.np;
            if stack.ir[np] == 2 { stack.z[np].min(self.zbound - stack.z[np]) } else { 0.0 }
        }
    }

    /// Bytes of a synthetic `relax.data`: `elements[z - 1]` is the shell list
    /// of element `z` as `(shell_type, binding energy, [(state, prob)])`.
    fn synthetic_relax_bytes(elements: &[Vec<(i32, f32, Vec<(i32, f32)>)>], big_endian: bool) -> Vec<u8> {
        let nz = elements.len();
        let mut words: Vec<u32> = alloc::vec![nz as u32];
        words.resize(1 + nz, 0);
        for (i, shells) in elements.iter().enumerate() {
            words[1 + i] = (words.len() + 1) as u32; // record number of the element's data
            words.push(shells.len() as u32);
            for (ty, be, trans) in shells {
                words.push(*ty as u32);
                words.push(trans.len() as u32);
                words.push(be.to_bits());
                for (state, prob) in trans {
                    words.push(*state as u32);
                    words.push(prob.to_bits());
                }
            }
        }
        words.iter().flat_map(|w| if big_endian { w.to_be_bytes() } else { w.to_le_bytes() }).collect()
    }

    /// Element 2 of a toy EADL file: K (10 keV) with a fluorescence line to L2
    /// (0.5), an Auger L1L1 (0.3) and 0.2 left over (so a remainder bin);
    /// L1..L3 below the 1 keV cut-off; M and N at 2 keV with no transitions.
    fn toy_eadl_file(big_endian: bool) -> EadlFile {
        let shells = alloc::vec![
            (1, 0.01_f32, alloc::vec![(2, 0.5_f32), (65, 0.3)]),
            (2, 0.0005, Vec::new()),
            (3, 0.0005, Vec::new()),
            (4, 0.0005, Vec::new()),
            (5, 0.002, Vec::new()),
            (6, 0.002, Vec::new()),
        ];
        let bytes = synthetic_relax_bytes(&[Vec::new(), shells], big_endian);
        EadlFile::from_bytes(&bytes).unwrap()
    }

    fn toy_binding_energies() -> Vec<Vec<f64>> {
        alloc::vec![alloc::vec![0.0; MXSHXSEC + 1]; 101]
    }

    #[test]
    fn eadl_file_decodes_both_byte_orders_and_refuses_garbage() {
        let le = toy_eadl_file(false);
        let be = toy_eadl_file(true);
        assert_eq!(le, be);
        assert_eq!(le.nz().unwrap(), 2);
        let sh = le.element(2).unwrap();
        assert_eq!(sh.len(), 6);
        assert_eq!(sh[0].transitions, alloc::vec![(2, 0.5_f32), (65, 0.3_f32)]);
        assert_eq!(sh[4].binding_energy, 0.002_f32);
        assert!(le.element(0).is_err() && le.element(3).is_err());
        assert!(EadlFile::from_bytes(&[1, 2, 3]).is_err());
        assert!(EadlFile::from_bytes(&[0xff; 8]).is_err());
        // a pointer past the end of the file is an error, not a panic
        let mut bytes = synthetic_relax_bytes(&[Vec::new(), Vec::new()], false);
        bytes[8..12].copy_from_slice(&1000u32.to_le_bytes());
        assert!(EadlFile::from_bytes(&bytes).unwrap().element(2).is_err());
    }

    #[test]
    fn init_relax_eadl_builds_alias_tables_and_shifts_states() {
        let file = toy_eadl_file(false);
        let mut be = toy_binding_energies();
        let d = init_relax_eadl(&file, &[alloc::vec![2.0], alloc::vec![2.0]], &mut be, false).unwrap();
        // each Z is loaded once, shells 1-based
        assert_eq!(d.shell_be.len(), 7);
        assert_eq!(d.shell_eadl[2][1..=6], [1, 2, 3, 4, 5, 6]);
        assert_eq!(d.shell_eadl[2][7], -1);
        assert_eq!((d.shell_num[3], d.shell_z[3], d.shell_type[1]), (3, 2, 1));
        // K: states +1 / +65, the remainder bin has state -1 and probability 1 - 0.8
        assert_eq!(d.relax_ntran[1], 3);
        assert_eq!(d.relax_state[1][1..=3], [3, 130, -1]);
        // shells under 1 keV have no relaxation; M and N (>= 1 keV, no transitions) get a certain remainder bin
        assert_eq!(d.relax_ntran[2], -1);
        assert_eq!((d.relax_ntran[5], d.relax_state[5][1]), (1, -1));
        // the alias table reproduces the probabilities: bin j is drawn with
        // (w_j + sum over i with atbin_i = j of (1 - w_i)) / n
        let (w, at) = (&d.relax_prob[1], &d.relax_atbin[1]);
        let want = [0.5_f32 as f64, 0.3_f32 as f64, 1.0 - (0.5_f32 as f64 + 0.3_f32 as f64)];
        for j in 1..=3usize {
            let mut p = w[j];
            for i in 1..=3usize {
                if i != j && at[i] as usize == j {
                    p += 1.0 - w[i];
                }
            }
            assert!((p / 3.0 - want[j - 1]).abs() < 1e-12, "bin {j}: {} vs {}", p / 3.0, want[j - 1]);
        }
        // binding energies are not touched for xcom; for epdl the zeros are filled from the file
        assert!(be[2].iter().all(|v| *v == 0.0));
        init_relax_eadl(&file, &[alloc::vec![2.0]], &mut be, true).unwrap();
        assert_eq!((be[2][1], be[2][5]), (0.01_f32 as f64, 0.002_f32 as f64));
        // a positive table value wins over the file's
        be[2][1] = 0.011;
        let d2 = init_relax_eadl(&file, &[alloc::vec![2.0]], &mut be, false).unwrap();
        assert_eq!(d2.shell_be[1], 0.011);
        // an element absent from the file is an error
        assert!(init_relax_eadl(&file, &[alloc::vec![3.0]], &mut be, false).is_err());
    }

    #[test]
    fn eadl_relax_conserves_energy_and_follows_the_toy_cascade() {
        let file = toy_eadl_file(false);
        let mut be = toy_binding_energies();
        let eadl = init_relax_eadl(&file, &[alloc::vec![2.0]], &mut be, true).unwrap();
        let rd = RelaxData { binding_energies: be, eadl: Some(eadl.clone()), ..Default::default() };
        let mut rng = Ranmar::new(17, 23);
        let (mut photons, mut augers, mut nothing) = (0, 0, 0);
        for _ in 0..400 {
            let mut st = Stack::new(40);
            st.np = 1;
            st.wt[1] = 1.0;
            st.ir[1] = 3;
            st.x[1] = 4.0;
            // `relax` dispatches to `eadl_relax` when EADL tables are present
            let out = relax(&mut st, &rd, 0.123, 1, 2, RM + 1e-4, 1e-4, RM, &mut rng).unwrap();
            let mut tot = out.edep;
            for i in 2..=st.np {
                tot += if st.iq[i] == 0 { st.e[i] } else { st.e[i] - RM };
                assert_eq!((st.ir[i], st.x[i]), (3, 4.0), "properties come from the slot on top at entry");
                assert!((norm(&st, i) - 1.0).abs() < 1e-9);
            }
            // the vacancy energy is the table's (10 keV), not the argument
            assert!((tot - 0.01_f32 as f64).abs() < 1e-12, "{tot}");
            match st.np {
                2 if st.iq[2] == 0 => photons += 1,
                2 => augers += 1,
                _ => nothing += 1,
            }
        }
        // fluorescence 0.5 / Auger 0.3 / remainder 0.2 (the remainder bin emits
        // the whole 10 keV as a photon: the upstream oddity, see `eadl_relax`)
        assert!(photons > 150 && augers > 60 && nothing == 0, "{photons} {augers} {nothing}");
        // a vacancy below L3 is deposited locally, no cascade, edep is the table's
        let mut st = Stack::new(10);
        st.np = 1;
        let out = eadl_relax(&mut st, &eadl, 2, 5, RM + 1e-4, 1e-4, &mut rng).unwrap();
        assert_eq!((st.np, out.edep), (1, 0.002_f32 as f64));
        // unknown vacancies: other element, shell number outside the table, negative
        for (z, s) in [(3usize, 1i32), (2, 7), (2, 0), (2, -1), (200, 1)] {
            let out = eadl_relax(&mut st, &eadl, z, s, 1.0, 1.0, &mut rng).unwrap();
            assert_eq!((st.np, out.edep), (1, 0.0));
        }
        // a vacancy under 1 keV is deposited
        let out = eadl_relax(&mut st, &eadl, 2, 2, 1.0, 1.0, &mut rng).unwrap();
        assert_eq!(out.edep, 0.0005_f32 as f64);
        // cut-offs: nothing is emitted above them, everything is deposited
        let out = eadl_relax(&mut st, &eadl, 2, 1, 10.0, 10.0, &mut rng).unwrap();
        assert!((out.edep - 0.01_f32 as f64).abs() < 1e-12 && st.np == 1);
        // a full stack is an error
        let mut full = Stack::new(1);
        full.np = 1;
        let mut err = false;
        for _ in 0..20 {
            full.np = 1;
            err |= matches!(eadl_relax(&mut full, &eadl, 2, 1, RM + 1e-4, 1e-4, &mut rng), Err(EgsError::StackExceeded { .. }));
        }
        assert!(err);
    }

    #[test]
    fn photo_with_eadl_tables_emits_no_vacancy_below_l3() {
        let (mut d, one, _) = toy_photo_data();
        for k in 1..=4 {
            d.interaction_prob[6][k] = 0.0; // every K..L3 candidate is declined: the shell ends above L3
        }
        let file = toy_eadl_file(false);
        let mut be = toy_binding_energies();
        // the same binding-energy ladder as the toy built-in tables, element 6 for the photo data
        be[6] = alloc::vec![0.0, 0.01, 0.0008, 0.0008, 0.0008, 0.0005, 0.0001];
        let eadl = init_relax_eadl(&file, &[alloc::vec![2.0]], &mut be, false).unwrap();
        let rd = RelaxData { binding_energies: be, eadl: Some(eadl), ..Default::default() };
        let reg = PhotoRegion { iedgfl: 1, iphter: 0, ecut: RM + 1e-4, pcut: 1e-4 };
        let mut rng = Ranmar::new(3, 4);
        for _ in 0..20 {
            let mut st = photon_stack(0.05);
            let o = photo(&mut st, &d, &rd, &one, &reg, (0.05f64).ln(), None, RM, &mut rng).unwrap();
            assert_eq!((st.np, st.iq[1], o.edep), (1, -1, 0.0));
            assert!((st.e[1] - (0.05 + RM)).abs() < 1e-15);
        }
        // with the built-in tables the same draws do start a vacancy (K..L3 or M)
        let builtin = photo_relax_data();
        let mut started = 0;
        for _ in 0..40 {
            let mut st = photon_stack(0.05);
            photo(&mut st, &d, &builtin, &one, &reg, (0.05f64).ln(), None, RM, &mut rng).unwrap();
            if st.e[1] < 0.05 + RM - 1e-9 {
                started += 1;
            }
        }
        assert!(started > 0);
    }

    /// An `incoh.data`-shaped text whose every `Z` has at most `MXSHXSEC` shells:
    /// Z = 2 has two (type 1 then 2, as the EADL renumbering wants), the rest
    /// is filler in blocks of 20.
    fn eadl_incoh_text() -> String {
        let mut t = String::new();
        for k in 0..18 {
            t.push_str(&alloc::format!("# comment {k}\n"));
        }
        t.push_str("2 1 2 0.05 500.0\n2 2 1 0.06 100.0\n");
        for n in 2..MXTOTSH {
            t.push_str(&alloc::format!("{} 7 2 0.05 1000.0\n", 10 + (n - 2) / 20));
        }
        t
    }

    #[test]
    fn init_compton_eadl_renumbers_shells_and_swaps_binding_energies() {
        let text = eadl_incoh_text();
        let media = alloc::vec![alloc::vec![(2.0, 1.0)]];
        let plain = init_compton(&text, &media, RM).unwrap();
        // incoh.data's own shell types are kept by the plain version
        assert_eq!(plain.shn_array[3], 7);
        let mut be = toy_binding_energies();
        be[2][1] = 0.004; // an edge for the first shell only
        let d = init_compton_eadl(&text, &media, RM, &mut be, true).unwrap();
        // shell numbers within each element: 1, 2 for Z = 2, then 1..20 per filler Z
        assert_eq!((d.shn_array[1], d.shn_array[2], d.shn_array[3], d.shn_array[4], d.shn_array[22]), (1, 2, 1, 2, 20));
        // an edge replaces the binding energy (in units of rm); without one, file value kept and written to the table (xcom)
        assert!((d.be_array[1] - 0.004 / RM).abs() < 1e-15);
        assert!((d.be_array[2] - 100.0 * 1e-6 / RM).abs() < 1e-15);
        assert!((be[2][2] - d.be_array[2] * RM).abs() < 1e-18);
        assert_eq!(be[2][1], 0.004);
        // for another series (epdl) the table is left alone
        let mut be2 = toy_binding_energies();
        let _ = init_compton_eadl(&text, &media, RM, &mut be2, false).unwrap();
        assert!(be2[2].iter().all(|v| *v == 0.0));
        // everything the alias tables use is the plain version's
        assert_eq!((d.n_shell[1], d.shell_array[1].clone(), d.eno_array[1].clone()), (plain.n_shell[1], plain.shell_array[1].clone(), plain.eno_array[1].clone()));
        // a table without room for the shell number is an error
        let mut tiny = alloc::vec![alloc::vec![0.0; 3]; 101];
        assert!(init_compton_eadl(&text, &media, RM, &mut tiny, true).is_err());
    }

    #[test]
    fn edgset_eadl_skips_binding_energies_and_loads_the_relaxation_tables() {
        // photo_relax.data and photo_cs.data shaped text: 100 elements each
        let mut photo = String::new();
        for i in 1..=100 {
            photo.push_str(&alloc::format!("{i} 1 2 3 4 5 6\n"));
        }
        photo.push_str("header\n");
        for i in 1..=100 {
            photo.push_str(&alloc::format!("{i} 0.5 0.5 0.5 0.5 0.5\n"));
        }
        for &(first, last) in &[(1usize, 19usize), (20, 26), (27, 32), (33, 37), (38, 38)] {
            photo.push_str("header\n");
            for i in 1..=100 {
                photo.push_str(&alloc::format!("{i}"));
                for _ in first..=last {
                    photo.push_str(" 0.1");
                }
                photo.push('\n');
            }
        }
        let mut phocs = String::new();
        for i in 1..=100 {
            phocs.push_str(&alloc::format!("{i} 1\n1 2 3 4 0.001\n"));
        }
        // the plain reader reads the binding energies (eV -> MeV); the EADL one skips them
        let (plain, _) = edgset_from_text(&photo, &phocs).unwrap();
        assert!((plain.binding_energies[1][1] - 1e-6).abs() < 1e-18);
        let file = toy_eadl_file(false);
        let mut be = toy_binding_energies();
        be[2][3] = 0.0123;
        let (rd, pd) = edgset_eadl_from_text(&photo, &phocs, &file, &[alloc::vec![2.0]], &mut be, false).unwrap();
        assert_eq!(rd.binding_energies[1][1], 0.0);
        assert_eq!(rd.binding_energies[2][3], 0.0123);
        assert_eq!(rd.eadl.as_ref().unwrap().shell_be[3], 0.0123);
        assert_eq!(pd.interaction_prob[5][MXSHELL], 1.01);
        assert!((rd.relaxation_prob[7][38] - 0.1).abs() < 1e-15);
        assert!(plain.eadl.is_none());
    }

    /// The EADL data of the real files, structure and sanity (no upstream
    /// output): `relax.data` of the pinned commit is 199947 four-byte
    /// records, record 1 is 100 elements, Ta has 22 shells and its K shell
    /// 178 transitions.
    #[test]
    fn real_relax_data_when_available() {
        let dir = std::env::var("EGSNRC_HEN_HOUSE_DATA").unwrap_or_else(|_| String::from("/tmp/claude-0/up/egsnrc/HEN_HOUSE/data"));
        let Ok(bytes) = std::fs::read(format!("{dir}/relax.data")) else {
            eprintln!("EGSnrc data not found in {dir}: real-data test skipped");
            return;
        };
        let f = EadlFile::from_bytes(&bytes).unwrap();
        assert_eq!(f.nz().unwrap(), 100);
        let ta = f.element(73).unwrap();
        eprintln!("Ta: {} shells, K shell {} transitions, K binding energy {} MeV", ta.len(), ta[0].transitions.len(), ta[0].binding_energy);
        assert_eq!((ta.len(), ta[0].transitions.len()), (22, 178));
        for z in 1..=100 {
            let sh = f.element(z).unwrap();
            assert!(!sh.is_empty() && sh.len() <= MXESHLL);
            assert!(sh.iter().all(|s| s.binding_energy > 0.0 && s.transitions.iter().all(|t| t.1 >= 0.0 && t.1 <= 1.0)));
        }
        // all 100 elements load: shell and channel counts under upstream's limits
        let mut be = toy_binding_energies();
        let all: Vec<f64> = (1..=100).map(|z| z as f64).collect();
        match init_relax_eadl(&f, &[all], &mut be, true) {
            Ok(d) => eprintln!("all 100 elements: {} shells, {} relax channels", d.shell_be.len() - 1, d.relax_state.iter().map(|v| v.len().saturating_sub(1)).sum::<usize>()),
            Err(e) => eprintln!("all 100 elements together: {e} (upstream limits $MAXSHELL / $MAXRELAX)"),
        }
    }

    /// Phase 2: EADL relaxation tables against the REAL EGSnrc `HATCH`
    /// (`eadl_relax = true`, the stock default, series `xcom`, tutor1's Ta).
    /// `EGSNRC_REFERENCE_EADL_TABLES` is the directory written by `difftabE`
    /// (a copy of `difftab` with the `COMPTON-DATA`, `SHELL-DATA` and
    /// `RELAX-DATA` commons dumped): `tables_compton.txt` (the medium's shell
    /// alias table and every `iz/shn/ne/be/Jo/erfJo` of the 1538 shells),
    /// `tables_shell.txt` (`shell_*`, `shell_eadl(73,.)`, `relax_*`),
    /// `tables_edges.txt` (`binding_energies(1..30, 73)` after `HATCH`).
    /// Integers are compared exactly, reals by the worst relative difference
    /// (reported, and asserted below 1e-9).
    #[test]
    fn differential_eadl_tables_against_compiled_egsnrc() {
        let Ok(refdir) = std::env::var("EGSNRC_REFERENCE_EADL_TABLES") else {
            eprintln!("EGSNRC_REFERENCE_EADL_TABLES not set: differential test skipped");
            return;
        };
        let dir = std::env::var("EGSNRC_HEN_HOUSE_DATA").unwrap_or_else(|_| String::from("/tmp/claude-0/up/egsnrc/HEN_HOUSE/data"));
        let text = |f: &str| std::fs::read_to_string(format!("{dir}/{f}")).unwrap();
        let rd_ref = |f: &str| -> Vec<Vec<f64>> {
            std::fs::read_to_string(format!("{refdir}/{f}"))
                .unwrap()
                .lines()
                .map(|l| l.split_whitespace().skip_while(|t| t.parse::<f64>().is_err() && false).map(|t| t.parse::<f64>().unwrap_or(f64::NAN)).collect())
                .collect()
        };
        // the same assembly as HATCH for xcom + eadl_relax (egsnrc.mortran:2264-2274)
        let um = vec![UserPhotonMedium::default(), UserPhotonMedium { comp: vec![(73.0, 1.0, 180.948)], rho: 16.6, ap: 0.01, up: 50.0, rayleigh: false }];
        let inp = UserPhotonInput {
            photo: text("xcom_photo.data"),
            pair: text("xcom_pair.data"),
            triplet: text("xcom_triplet.data"),
            rayleigh: text("xcom_rayleigh.data"),
            photonuc: None,
            compton: BoundComptonData::Ratio(text("compton_sigma.data")),
            mxge: 2000,
            eadl_relax: true,
            rm: RM,
            form_factors: None,
        };
        let ut = init_user_photon(&inp, &um).unwrap();
        let mut be = ut.binding_energies.clone();
        let comp = vec![(73.0, 1.0)];
        let compton = init_compton_eadl(&text("incoh.data"), &[comp], RM, &mut be, true).unwrap();
        let rfile = EadlFile::from_bytes(&std::fs::read(format!("{dir}/relax.data")).unwrap()).unwrap();
        let (rd, _) = edgset_eadl_from_text(&text("photo_relax.data"), &text("photo_cs.data"), &rfile, &[vec![73.0]], &mut be, false).unwrap();
        let eadl = rd.eadl.clone().unwrap();
        let rel = |a: f64, b: f64| if a == b { 0.0 } else { (a - b).abs() / b.abs().max(1e-300) };
        // binding energies of Ta after HATCH
        let edges = rd_ref("tables_edges.txt");
        let worst_edge = (1..=30).map(|k| rel(rd.binding_energies[73][k], edges[k - 1][1])).fold(0.0_f64, f64::max);
        // Compton tables
        let comp_t = rd_ref("tables_compton.txt");
        let n_shell = comp_t[0][1] as usize;
        assert_eq!(n_shell as i32, compton.n_shell[1]);
        let (mut ints_bad, mut w_eno) = (0usize, 0.0_f64);
        for i in 1..=n_shell {
            let r = &comp_t[i];
            ints_bad += usize::from(r[1] as i32 != compton.shell_array[1][i]);
            ints_bad += usize::from(r[2] as i32 != compton.eno_atbin_array[1][i]);
            w_eno = w_eno.max(rel(compton.eno_array[1][i], r[3]));
        }
        let (mut w_be, mut w_jo, mut w_erf) = (0.0_f64, 0.0_f64, 0.0_f64);
        for j in 1..=MXTOTSH {
            let r = &comp_t[n_shell + j];
            ints_bad += usize::from(r[1] as i32 != compton.iz_array[j]);
            ints_bad += usize::from(r[2] as i32 != compton.shn_array[j]);
            ints_bad += usize::from(r[3] as i32 != compton.ne_array[j]);
            w_be = w_be.max(rel(compton.be_array[j], r[4]));
            w_jo = w_jo.max(rel(compton.jo_array[j], r[5]));
            w_erf = w_erf.max(rel(compton.erf_jo_array[j], r[6]));
        }
        // shell and relaxation tables
        let sh_t = rd_ref("tables_shell.txt");
        let (ntot, rtot) = (sh_t[0][1] as usize, sh_t[0][2] as usize);
        assert_eq!(ntot, eadl.shell_be.len() - 1);
        let mut relax_first = 0usize;
        let (mut s_bad, mut w_sbe, mut w_prob, mut r_bad) = (0usize, 0.0_f64, 0.0_f64, 0usize);
        let mut flat_at = 0usize;
        let mut flat: Vec<(i32, f64, i32)> = Vec::new();
        for ish in 1..=ntot {
            let r = &sh_t[ish];
            s_bad += usize::from(r[1] as i32 != eadl.shell_type[ish]);
            s_bad += usize::from(r[2] as i32 != eadl.shell_num[ish]);
            s_bad += usize::from(r[3] as i32 != eadl.shell_z[ish]);
            s_bad += usize::from(r[5] as i32 != eadl.relax_ntran[ish]);
            let ntran = eadl.relax_ntran[ish];
            let first = if ntran < 0 { -1 } else { (relax_first + 1) as i32 };
            s_bad += usize::from(r[4] as i32 != first);
            s_bad += usize::from(r[7] as i32 != eadl.shell_eadl[73][eadl.shell_num[ish] as usize]);
            w_sbe = w_sbe.max(rel(eadl.shell_be[ish], r[6]));
            if ntran > 0 {
                for k in 1..=ntran as usize {
                    flat.push((eadl.relax_state[ish][k], eadl.relax_prob[ish][k], eadl.relax_atbin[ish][k]));
                }
                relax_first += ntran as usize;
            }
        }
        assert_eq!(rtot, relax_first);
        for (i, f) in flat.iter().enumerate() {
            let r = &sh_t[ntot + 1 + i];
            r_bad += usize::from(r[1] as i32 != f.0);
            r_bad += usize::from(r[3] as i32 != f.2);
            w_prob = w_prob.max(rel(f.1, r[2]));
            flat_at += 1;
        }
        eprintln!("binding_energies(1..30, Ta): worst relative difference {worst_edge:e}");
        eprintln!("compton: {n_shell} medium shells, {} shells total, integer mismatches {ints_bad}; worst relative: eno {w_eno:e}, be {w_be:e}, Jo {w_jo:e}, erfJo {w_erf:e}", MXTOTSH);
        eprintln!("eadl: {ntot} shells, {rtot} channels ({flat_at} compared), shell integer mismatches {s_bad}, channel integer mismatches {r_bad}; worst relative: shell_be {w_sbe:e}, relax_prob(alias) {w_prob:e}");
        assert_eq!((ints_bad, s_bad, r_bad), (0, 0, 0));
        for w in [worst_edge, w_eno, w_be, w_jo, w_erf, w_sbe, w_prob] {
            assert!(w < 1e-9, "{w:e}");
        }
    }

    struct Tutor1Driver {
        tables: ElectrTables,
        cfg: ElectrConfig,
        geo: Tutor1Geo,
        counters: ElectrCounters,
    }

    impl ElectronTransport for Tutor1Driver {
        fn electr<R: UniformSource>(&mut self, stack: &mut Stack, rng: &mut R) -> Result<i32, EgsError> {
            Ok(electr(stack, &self.tables, &self.cfg, &mut self.geo, &mut self.counters, rng)?.ircode)
        }
    }

    /// Phase 2, the transport kernel: 200 histories of `tutor1` (20 MeV
    /// electrons, 1 mm Ta plate, `tutor_data.pegs4dat`, EGSnrc defaults except
    /// `eadl_relax = false`, the one variant this port has) against the
    /// discards a REAL EGSnrc logs (`histories.txt` of the `difftut` user code:
    /// RANMAR seeded 1802/9373, full precision, `-fdefault-real-8` build).
    /// `EGSNRC_REFERENCE_HISTORIES` is that file; the data files come from
    /// `EGSNRC_HEN_HOUSE_DATA`, the data set from `EGSNRC_PEGS4_FILE`. Prints
    /// how many histories agree in structure and the worst relative energy
    /// and direction differences over those; asserts nothing numerical.
    #[test]
    fn differential_tutor1_histories_against_compiled_egsnrc() {
        let Ok(reference) = std::env::var("EGSNRC_REFERENCE_HISTORIES") else {
            eprintln!("EGSNRC_REFERENCE_HISTORIES not set: differential test skipped");
            return;
        };
        tutor1_histories_report(&reference, false);
    }

    /// Phase 2, EADL relaxation (the stock default, `eadl_relax = true`):
    /// the same 200 histories with the Rust assembly using
    /// [`init_compton_eadl`], [`edgset_eadl_from_text`] and the real
    /// `relax.data`, against `histories.txt` of the `difftutE` build (the
    /// `difftut` user code with its `eadl_relax=.false.;` line removed, run
    /// from `EGSNRC_REFERENCE_HISTORIES_EADL`). Prints the same figures as
    /// [`differential_tutor1_histories_against_compiled_egsnrc`].
    #[test]
    fn differential_tutor1_histories_against_compiled_egsnrc_eadl() {
        let Ok(reference) = std::env::var("EGSNRC_REFERENCE_HISTORIES_EADL") else {
            eprintln!("EGSNRC_REFERENCE_HISTORIES_EADL not set: differential test skipped");
            return;
        };
        tutor1_histories_report(&reference, true);
    }

    fn tutor1_histories_report(reference: &str, eadl: bool) {
        use petir::rng::ranmar::Ranmar;
        use std::sync::{Arc, RwLock};
        let dir = std::env::var("EGSNRC_HEN_HOUSE_DATA").unwrap_or_else(|_| String::from("/tmp/claude-0/up/egsnrc/HEN_HOUSE/data"));
        let path = std::env::var("EGSNRC_PEGS4_FILE").unwrap_or_else(|_| String::from("/tmp/claude-0/up/egsnrc/HEN_HOUSE/pegs4/data/tutor_data.pegs4dat"));
        let text = |f: &str| std::fs::read_to_string(format!("{dir}/{f}")).unwrap();
        // media and the HATCH assembly
        let mut pm = parse_pegs4_media(&std::fs::read_to_string(path).unwrap(), &["TA"]).unwrap();
        pm[0].apply_dunit(1.0);
        let r = hatch_media(&pm, &HatchConfig { ecutmn: 1.5, ..Default::default() });
        let comp: Vec<(f64, f64)> = pm[0].elements.iter().map(|x| (x.zelem, x.pz)).collect();
        // photon tables from the xcom series
        let ff = FormFactorTable::from_text(&std::fs::read_to_string(format!("{dir}/../pegs4/pgs4form.dat")).unwrap()).unwrap();
        let inp = UserPhotonInput {
            photo: text("xcom_photo.data"),
            pair: text("xcom_pair.data"),
            triplet: text("xcom_triplet.data"),
            rayleigh: text("xcom_rayleigh.data"),
            photonuc: None,
            compton: BoundComptonData::Ratio(text("compton_sigma.data")),
            mxge: 2000,
            eadl_relax: eadl,
            rm: RM,
            form_factors: Some(ff),
        };
        let um = vec![
            UserPhotonMedium::default(),
            UserPhotonMedium {
                comp: pm[0].elements.iter().map(|x| (x.zelem, x.pz, x.wa)).collect(),
                rho: pm[0].rho,
                ap: pm[0].ap,
                up: pm[0].up,
                rayleigh: true,
            },
        ];
        let ut = init_user_photon(&inp, &um).unwrap();
        let u = &ut.media[1];
        let mut photon_med = r.media[0].photon.clone();
        photon_med.ge0 = u.ge0;
        photon_med.ge1 = u.ge1;
        photon_med.gmfp0 = u.gmfp.a0.clone();
        photon_med.gmfp1 = u.gmfp.a1.clone();
        photon_med.gbr10 = u.gbr1.a0.clone();
        photon_med.gbr11 = u.gbr1.a1.clone();
        photon_med.gbr20 = u.gbr2.a0.clone();
        photon_med.gbr21 = u.gbr2.a1.clone();
        photon_med.cohe = u.cohe.clone();
        photon_med.rayleigh = u.rayleigh.clone();
        let (relax_d, photo_d, compton) = if eadl {
            // HATCH order for eadl_relax with the xcom series (egsnrc.mortran:2264-2274)
            let mut be = ut.binding_energies.clone();
            let compton = init_compton_eadl(&text("incoh.data"), &[comp.clone()], RM, &mut be, true).unwrap();
            let rfile = EadlFile::from_bytes(&std::fs::read(format!("{dir}/relax.data")).unwrap()).unwrap();
            let media_z = vec![comp.iter().map(|c| c.0).collect::<Vec<f64>>()];
            let (rd, pd) = edgset_eadl_from_text(&text("photo_relax.data"), &text("photo_cs.data"), &rfile, &media_z, &mut be, false).unwrap();
            (rd, pd, compton)
        } else {
            let (rd, pd) = edgset_from_text(&text("photo_relax.data"), &text("photo_cs.data")).unwrap();
            (rd, pd, init_compton(&text("incoh.data"), &[comp.clone()], RM).unwrap())
        };
        let rho = pm[0].rho;
        let photo_med = PhotoMedium { zelem: core::iter::once(0.0).chain(comp.iter().map(|c| c.0)).collect(), pz: core::iter::once(0.0).chain(comp.iter().map(|c| c.1)).collect() };
        photon_med.photo = photo_med;
        let reg = |med: usize, rhor: f64| PhotonRegionData { med, rhor, pcut: 0.1, ecut: 1.5, ibcmp: 3, iedgfl: 1, iphter: 1, irayl: 1, iphotonucr: 0 };
        let ptables = PhotonTables {
            regions: vec![PhotonRegionData::default(), reg(0, 0.0), reg(1, rho), reg(0, 0.0)],
            media: vec![PhotonMediumData::default(), photon_med],
            compton,
            relax: relax_d.clone(),
            photo: photo_d,
        };
        // electron tables with the spin fits
        let mut emed = r.media[0].electron.clone();
        let sfile = SpinFile::from_bytes(&std::fs::read(format!("{dir}/spinms.data")).unwrap()).unwrap();
        let sin = SpinMediumInput {
            elements: pm[0].elements.iter().map(|x| (x.zelem, x.pz, x.wa)).collect(),
            eke0: emed.eke0,
            eke1: emed.eke1,
            neke: emed.esig.a0.len() - 1,
            blcc: emed.ms.blcc,
            xcc: emed.ms.xcc,
            te: emed.te,
            ue: pm[0].ue,
            esig: emed.esig.clone(),
            ededx: emed.ededx.clone(),
        };
        let (sdata, fits) = init_spin(&sfile, &[sin], RM).unwrap();
        let f = fits[0].clone();
        emed.ms.etae_ms = f.etae_ms;
        emed.ms.etap_ms = f.etap_ms;
        emed.ms.q1ce_ms = f.q1ce_ms;
        emed.ms.q1cp_ms = f.q1cp_ms;
        emed.ms.q2ce_ms = f.q2ce_ms;
        emed.ms.q2cp_ms = f.q2cp_ms;
        emed.ms.blcce = f.blcce;
        let ereg = |med: usize, rhor: f64| ElectrRegion { med, rhor, ecut: 1.5, smaxir: 1.0e10, pcut: 0.1 };
        let etables = ElectrTables {
            regions: vec![ElectrRegion::default(), ereg(0, 0.0), ereg(1, rho), ereg(0, 0.0)],
            media: vec![ElectronMediumData::default(), emed],
            ms: Some(MsData::from_text(&text("msnew.data")).unwrap()),
            spin: sdata,
            eii: None,
        };
        let log = Arc::new(RwLock::new(Vec::new()));
        let geo = Tutor1Geo { zbound: 0.1, prm: RM, log: log.clone() };
        let ecfg = ElectrConfig { spin_effects: true, exact_bca: true, skindepth_for_bca: 3.0, presta_ii: true, prm: RM, vacdst: 1.0e8, brems: BremsConfig::default() };
        let pcfg = PhotonConfig { pair: PairConfig::default(), rr: None, prm: RM, vacdst: 1.0e8 };
        let mut drv = Tutor1Driver { tables: etables, cfg: ecfg, geo: geo.clone(), counters: ElectrCounters::default() };
        let mut pgeo = geo;
        let mut rng = Ranmar::new(1802, 9373);
        let mut mine: Vec<Vec<(i32, f64, f64)>> = Vec::new();
        for _ in 0..200 {
            log.write().unwrap().clear();
            let mut st = Stack::new(15);
            let start = ShowerStart { iqi: -1, ei: 20.5109989461, xi: 0.0, yi: 0.0, zi: 0.0, ui: 0.0, vi: 0.0, wi: 1.0, iri: 2, wti: 1.0 };
            match shower(&mut st, &ptables, &pcfg, &mut pgeo, &mut drv, &start, &mut rng) {
                Ok(_) => mine.push(log.read().unwrap().clone()),
                Err(e) => {
                    eprintln!("history {} stopped with {e:?}", mine.len() + 1);
                    mine.push(Vec::new());
                    break;
                }
            }
        }
        // the reference
        let mut theirs: Vec<Vec<(i32, f64, f64)>> = Vec::new();
        for l in std::fs::read_to_string(reference).unwrap().lines() {
            if l.starts_with('H') {
                theirs.push(Vec::new());
            } else {
                let v: Vec<&str> = l.split_whitespace().collect();
                theirs.last_mut().unwrap().push((v[0].parse().unwrap(), v[1].parse().unwrap(), v[2].parse().unwrap()));
            }
        }
        let (mut same_shape, mut first_diff, mut worst_e, mut worst_w) = (0usize, None, 0.0_f64, 0.0_f64);
        for (i, (a, b)) in mine.iter().zip(&theirs).enumerate() {
            let shape = a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.0 == y.0);
            if shape {
                same_shape += 1;
                for (x, y) in a.iter().zip(b) {
                    worst_e = worst_e.max((x.1 - y.1).abs() / y.1.abs().max(1e-12));
                    worst_w = worst_w.max((x.2 - y.2).abs());
                }
            } else if first_diff.is_none() {
                first_diff = Some(i + 1);
            }
        }
        eprintln!(
            "tutor1: {} port histories, {} reference; same charge sequence {same_shape}; first history with a different sequence {first_diff:?}; over those: worst relative kinetic energy difference {worst_e:e}, worst |dw| {worst_w:e}",
            mine.len(),
            theirs.len()
        );
        if let (Some(a), Some(b)) = (mine.first(), theirs.first()) {
            eprintln!("history 1 port {a:?}\nhistory 1 ref  {b:?}");
        }
    }

}
