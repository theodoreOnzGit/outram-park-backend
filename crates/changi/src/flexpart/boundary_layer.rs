// SPDX-License-Identifier: GPL-3.0
//
// FLEXPART port — provenance
// --------------------------
// Upstream project : FLEXPART (NILU) — https://github.com/flexpart/flexpart
// Upstream version : 10.4 (2019-11-12), commit 3d7eebf
// Upstream source  : src/pbl_profile.f90, src/richardson.f90, src/qvsat.f90
// Original licence : GPL-3.0-or-later — SPDX-FileCopyrightText: FLEXPART 1998-2019
// Ported into this GPL-3.0 work; see LICENSE.flexpart and NOTICE.flexpart.

//! Boundary-layer diagnostics from a meteorological column: surface fluxes
//! from the profile method (`pbl_profile`), the bulk-Richardson mixing height
//! with its convective velocity scale (`richardson`), and saturation specific
//! humidity (`f_qvsat`).
//!
//! # Units
//!
//! FLEXPART's, as bare `f64`: pressure Pa, temperature K, heights m, wind
//! m/s, heat flux W/m², stress N/m², specific humidity kg/kg.

use super::constants::{CONVKE, CPA, GA, R_AIR, VONKARMAN};
use super::surface_layer::{psih, psim, MetDataFormat};
use super::thermo::ew_kelvin;

/// What [`pbl_profile`] returns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PblProfile {
    /// Surface stress, N/m².
    pub stress: f64,
    /// Sensible heat flux, W/m² (FLEXPART's sign convention: positive is
    /// downward, so negative means unstable).
    pub hf: f64,
    /// Friction velocity, m/s.
    pub ustar: f64,
    /// Obukhov length, m, clamped to `±9999`.
    pub ol: f64,
}

/// `pbl_profile.f90`: surface stress and sensible heat flux from the 10 m and
/// lowest-model-level wind and the 2 m and lowest-level temperature (Berkowicz
/// and Prahm 1982 profile method).
///
/// # Arguments
/// - `ps` — surface pressure, Pa.
/// - `td2m` — 2 m dew point, K.
/// - `zml1` — height of the lowest model level, m.
/// - `t2m`, `tml1` — temperature at 2 m and at the lowest level, K.
/// - `u10m`, `uml1` — wind speed at 10 m and at the lowest level, m/s.
///
/// # Branches, as upstream
/// - wind shear `<= 0.001 m/s`: similarity not applicable, `u* = 0.01`, `hf = 0`;
/// - `|Δθ| <= 0.03 K`: neutral, `hf = 0`;
/// - stable with critical ratio `<= 1`: `L = 50 m`, no iteration;
/// - otherwise up to 10 successive approximations, stopping at 1 % in `L`.
#[must_use]
pub fn pbl_profile(
    ps: f64,
    td2m: f64,
    zml1: f64,
    t2m: f64,
    tml1: f64,
    u10m: f64,
    uml1: f64,
) -> PblProfile {
    const MAXITER: usize = 10;
    const R1: f64 = 0.74;

    let e = ew_kelvin(td2m);
    let tv = t2m * (1.0 + 0.378 * e / ps);
    let rhoa = ps / (R_AIR * tv);
    let deltau = uml1 - u10m;
    if deltau <= 0.001 {
        let ustar = 0.01;
        return PblProfile {
            stress: ustar * ustar * rhoa,
            hf: 0.0,
            ustar,
            ol: 9999.0,
        };
    }

    let ustar_at =
        |al: f64| (VONKARMAN * deltau) / ((zml1 / 10.0).ln() - psim(zml1, al) + psim(10.0, al));
    let deltat = tml1 - t2m + 0.0098 * (zml1 - 2.0);
    let thetastar_at =
        |al: f64| (VONKARMAN * deltat / R1) / ((zml1 / 2.0).ln() - psih(zml1, al) + psih(2.0, al));

    if deltat.abs() <= 0.03 {
        let al = 9999.0;
        let ustar = ustar_at(al);
        return PblProfile {
            stress: ustar * ustar * rhoa,
            hf: 0.0,
            ustar,
            ol: al,
        };
    }

    let tmean = 0.5 * (t2m + tml1);
    let dz10 = zml1 - 10.0;
    let crit = (0.0219 * tmean * (zml1 - 2.0) * (deltau * deltau)) / (deltat * (dz10 * dz10));
    if deltat > 0.0 && crit <= 1.0 {
        let al = 50.0;
        let ustar = ustar_at(al);
        let thetastar = thetastar_at(al);
        return PblProfile {
            stress: ustar * ustar * rhoa,
            hf: rhoa * CPA * ustar * thetastar,
            ustar,
            ol: al,
        };
    }

    let mut al: f64 = 9999.0;
    let mut ustar = 0.0;
    let mut thetastar = 0.0;
    for _ in 0..MAXITER {
        let alold = al;
        ustar = ustar_at(al);
        thetastar = thetastar_at(al);
        al = (tmean * (ustar * ustar)) / (GA * VONKARMAN * thetastar);
        let aldiff = ((al - alold) / alold).abs();
        if aldiff < 0.01 {
            break;
        }
    }
    let hf = rhoa * CPA * ustar * thetastar;
    let al = al.clamp(-9999.0, 9999.0);
    PblProfile {
        stress: ustar * ustar * rhoa,
        hf,
        ustar,
        ol: al,
    }
}

const SATFWA: f64 = 1.0007;
const SATFWB: f64 = 3.46e-8;
const SATEWA: f64 = 611.21;
const SATEWB: f64 = 17.502;
const SATEWC: f64 = 32.18;
const SATFIA: f64 = 1.0003;
const SATFIB: f64 = 4.18e-8;
const SATEIA: f64 = 611.15;
const SATEIB: f64 = 22.452;
const SATEIC: f64 = 0.6;

/// `f_esl` (`qvsat.f90`): saturation vapour pressure over liquid water, Pa,
/// with the pressure enhancement factor (Buck 1981).
#[must_use]
pub fn f_esl(p: f64, t: f64) -> f64 {
    let f = SATFWA + SATFWB * p;
    f * SATEWA * (SATEWB * (t - 273.15) / (t - SATEWC)).exp()
}

/// `f_esi` (`qvsat.f90`): saturation vapour pressure over ice, Pa.
#[must_use]
pub fn f_esi(p: f64, t: f64) -> f64 {
    let f = SATFIA + SATFIB * p;
    f * SATEIA * (SATEIB * (t - 273.15) / (t - SATEIC)).exp()
}

/// `f_qvsat` (`qvsat.f90`): saturation specific humidity, kg/kg, over liquid
/// water at or above 253.15 K and over ice below. Upstream returns `1` where
/// the denominator vanishes; so does the port.
#[must_use]
pub fn f_qvsat(p: f64, t: f64) -> f64 {
    const RD: f64 = 287.0;
    const RV: f64 = 461.0;
    const RDDRV: f64 = RD / RV;
    let fespt = if t >= 253.15 {
        f_esl(p, t)
    } else {
        f_esi(p, t)
    };
    if p - (1.0 - RDDRV) * fespt == 0.0 {
        1.0
    } else {
        RDDRV * fespt / (p - (1.0 - RDDRV) * fespt)
    }
}

/// What [`richardson`] returns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MixingHeight {
    /// Mixing height `h`, m above ground.
    pub h: f64,
    /// Convective velocity scale `w*`, m/s (0 unless the heat flux is upward).
    pub wst: f64,
    /// Turbulent-entrainment increment `hmixplus`, m; `9999` when the layer
    /// above `h` is not stably stratified.
    pub hmixplus: f64,
}

/// `richardson.f90`: mixing height from the bulk Richardson number (Vogelezang
/// and Holtslag 1996), with the convective excess temperature iterated up to
/// three times for an unstable surface layer.
///
/// # Arguments
/// - `psurf` — surface pressure, Pa.
/// - `ust` — friction velocity, m/s.
/// - `akz`, `bkz` — hybrid coefficients of each level (Pa, dimensionless); the
///   level pressure is `akz + bkz * psurf`.
/// - `ttlev`, `qvlev`, `ulev`, `vlev` — temperature (K), specific humidity
///   (kg/kg) and wind components (m/s) on each level.
///
/// The level slices are 0-based (upstream's level `k` is index `k-1`) and all
/// the same length. For ECMWF data, index 0 is the surface level.
/// - `hf` — sensible heat flux, W/m² (negative = upward, unstable).
/// - `tt2`, `td2` — 2 m temperature and dew point, K.
/// - `format` — which product; ECMWF columns start at level 2, NCEP columns at
///   the first pressure level above the ground. Only the variant is used; the
///   coefficients it carries are `obukhov`'s, not this routine's.
///
/// # Upstream behaviour preserved
/// - If no level exceeds the critical Richardson number, upstream steps `k`
///   back to the top level and interpolates within a zero-thickness layer.
///   The Brunt–Väisälä frequency then comes out `0/0`, so `hmixplus` is
///   **NaN**, and the port returns NaN there too.
/// - The relative-humidity profile upstream computes along the way is never
///   used for any output and is not computed here.
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn richardson(
    psurf: f64,
    ust: f64,
    akz: &[f64],
    bkz: &[f64],
    ttlev: &[f64],
    qvlev: &[f64],
    ulev: &[f64],
    vlev: &[f64],
    hf: f64,
    tt2: f64,
    td2: f64,
    format: &MetDataFormat,
) -> MixingHeight {
    const RIC: f64 = 0.25;
    const B: f64 = 100.0;
    const BS: f64 = 8.5;
    const ITMAX: usize = 3;
    let konst = R_AIR / GA;
    let nuvz = ttlev.len();

    // 1-based level index of the first NCEP level above ground.
    let mut llev = 0;
    if matches!(format, MetDataFormat::Ncep) {
        for i in 1..=nuvz {
            if psurf < akz[i - 1] {
                llev = i;
            }
        }
        llev += 1;
        if llev == 1 {
            llev = 2;
        }
        if llev > nuvz {
            llev = nuvz - 1;
        }
    }

    let mut excess = 0.0;
    let mut iter = 0;
    loop {
        iter += 1;
        let mut pold = psurf;
        let mut tvold = tt2 * (1.0 + 0.378 * ew_kelvin(td2) / psurf);
        let mut zold = 2.0;
        let zref = zold;
        let thetaref = tvold * (100_000.0 / pold).powf(R_AIR / CPA) + excess;
        let mut thetaold = thetaref;
        let loop_start = match format {
            MetDataFormat::Ecmwf { .. } => 2,
            MetDataFormat::Ncep => llev,
        };

        let ust2 = ust * ust;
        let shear = |uk: f64, vk: f64| {
            let du = uk - ulev[1];
            let dv = vk - vlev[1];
            (du * du + dv * dv + B * ust2).max(0.1)
        };

        // 1-based k, as upstream.
        let mut k = loop_start;
        let mut z = 0.0;
        let mut theta = 0.0;
        let mut crossed = false;
        while k <= nuvz {
            let pint = akz[k - 1] + bkz[k - 1] * psurf;
            let tv = ttlev[k - 1] * (1.0 + 0.608 * qvlev[k - 1]);
            z = if (tv - tvold).abs() > 0.2 {
                zold + konst * (pold / pint).ln() * (tv - tvold) / (tv / tvold).ln()
            } else {
                zold + konst * (pold / pint).ln() * tv
            };
            theta = tv * (100_000.0 / pint).powf(R_AIR / CPA);
            let ri =
                GA / thetaref * (theta - thetaref) * (z - zref) / shear(ulev[k - 1], vlev[k - 1]);
            if ri > RIC && thetaold < theta {
                crossed = true;
                break;
            }
            tvold = tv;
            pold = pint;
            thetaold = theta;
            zold = z;
            k += 1;
        }
        if !crossed {
            k -= 1; // upstream: "make sure k <= nuvz (ticket #139)"
        }

        let mut zl1 = zold;
        let mut theta1 = thetaold;
        let (mut zl, mut ul, mut vl) = (0.0, 0.0, 0.0);
        let mut zl2 = 0.0;
        let mut theta2 = 0.0;
        for i in 1..=20 {
            let f = f64::from(i) / 20.0;
            zl = zold + f * (z - zold);
            ul = ulev[k - 2] + f * (ulev[k - 1] - ulev[k - 2]);
            vl = vlev[k - 2] + f * (vlev[k - 1] - vlev[k - 2]);
            let thetal = thetaold + f * (theta - thetaold);
            let ril = GA / thetaref * (thetal - thetaref) * (zl - zref) / shear(ul, vl);
            zl2 = zl;
            theta2 = thetal;
            if ril > RIC {
                break;
            }
            zl1 = zl;
            theta1 = thetal;
        }

        let h = zl;
        let thetam = 0.5 * (theta1 + theta2);
        let wspeed = (ul * ul + vl * vl).sqrt();
        let bvfsq = (GA / thetam) * (theta2 - theta1) / (zl2 - zl1);
        let hmixplus = if bvfsq <= 0.0 {
            9999.0
        } else {
            let bvf = bvfsq.sqrt();
            wspeed / bvf * CONVKE
        };

        if hf < 0.0 {
            let wst = (-(h * GA / thetaref * hf / CPA)).powf(0.333);
            excess = -(BS * hf / CPA / wst);
            if iter < ITMAX {
                continue;
            }
            return MixingHeight { h, wst, hmixplus };
        }
        return MixingHeight {
            h,
            wst: 0.0,
            hmixplus,
        };
    }
}
