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
//! **Status: first port of three routines, not differentially tested (gh:#1680).**
//! Everything else in the file is still a placeholder.
//!
//! | upstream | here |
//! |---|---|
//! | `SUBROUTINE ANNIH` (`egsnrc.mortran:70-183`) | [`annih`] |
//! | `SUBROUTINE ANNIH_AT_REST` (`egsnrc.mortran:186-224`) | [`annih_at_rest`] |
//! | `SUBROUTINE UPHI` (`egsnrc.mortran:6722-6838`), entries 2 and 3 | [`uphi`] |
//!
//! Not yet ported from this file: `BHABHA`, `BREMS`, `COMPT`, `ELECTR`,
//! `HATCH`, `MOLLER`, `PAIR`, `PHOTO`, `RELAX`, `EDGSET`, `PHOTON`, `SHOWER`,
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
}

/// `UPHI` (`egsnrc.mortran:6722-6838`), entries 2 and 3: rotate the direction
/// of a stack particle by the polar angle `(sinthe, costhe)` and a fresh uniform
/// azimuth (box method).
///
/// `lvl` selects the particle: `1` old particle (its direction is saved and
/// adjusted), `2` new particle (properties copied from `np - 1`, saved
/// direction reused), `3` bremsstrahlung photon (the electron's direction at
/// `np - 1` is saved, then the photon's is set).
///
/// # Errors
///
/// [`EgsError::BadEntry`] for an `lvl` outside 1..=3.
pub fn uphi<R: UniformSource>(
    stack: &mut Stack,
    saved: &mut UphiSaved,
    lvl: i32,
    sinthe: f64,
    costhe: f64,
    rng: &mut R,
) -> Result<(), EgsError> {
    // UPHI2: azimuth
    let (cosphi, sinphi) = select_azimuthal_angle(rng);
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
        _ => return Err(EgsError::BadEntry { ientry: 2, lvl }),
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
        uphi(&mut st, &mut saved, 1, sinthe, costhe, &mut rng).unwrap();
        assert!((norm(&st, 1) - 1.0).abs() < 1e-12);
        let dot = st.u[1] * u + st.v[1] * v + st.w[1] * w;
        assert!((dot - costhe).abs() < 1e-12, "angle to the old direction is theta");
        // lvl 2: a new particle above, using the saved direction
        st.np = 2;
        uphi(&mut st, &mut saved, 2, sinthe, costhe, &mut rng).unwrap();
        let dot2 = st.u[2] * u + st.v[2] * v + st.w[2] * w;
        assert!((dot2 - costhe).abs() < 1e-12);
        // lvl 3: brems photon, relative to the electron at np-1
        st.np = 3;
        uphi(&mut st, &mut saved, 3, sinthe, costhe, &mut rng).unwrap();
        let dot3 = st.u[3] * st.u[2] + st.v[3] * st.v[2] + st.w[3] * st.w[2];
        assert!((dot3 - costhe).abs() < 1e-12);
        assert_eq!(uphi(&mut st, &mut saved, 4, sinthe, costhe, &mut rng), Err(EgsError::BadEntry { ientry: 2, lvl: 4 }));
    }

    #[test]
    fn uphi_small_polar_angle_case() {
        let mut rng = Ranmar::new(5, 6);
        let mut st = Stack::new(10);
        st.np = 1;
        st.w[1] = -1.0; // along -z: a = b = 0
        let mut saved = UphiSaved::default();
        uphi(&mut st, &mut saved, 1, 0.6, 0.8, &mut rng).unwrap();
        assert!((norm(&st, 1) - 1.0).abs() < 1e-12);
        assert!((st.w[1] + 0.8).abs() < 1e-15, "w = c * costhe");
    }
}
