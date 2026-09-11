// Ported from NJOY2016 `src/gaminr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `gtff` — the GAMINR **feed functions** (`gaminr.f90:1162-1514`): the
//! photon-interaction cross-section/heating responses (`mfd = 23`) and the
//! three scattering-matrix feeds (`mfd = 26`):
//!
//! * **coherent** (`mtd = 502`, `:1232-1298`): at incident energy `e` the
//!   Rayleigh angular distribution `(1 + mu^2) F(x)^2` is projected onto
//!   `P_l(mu)` by integrating over the momentum-transfer variable
//!   `x = c1 e sqrt((1 - mu)/2)`... in NJOY's form `mu = 1 - x^2/(c1 e)^2`,
//!   panel by panel over the MF=27/MT=502 form-factor grid with a 6-point
//!   (10 when `nl > 4`) Lobatto rule, stopping at `mu = -1`, at
//!   `x = sqrt(2) c1 e`, or when `F` has fallen below `toler F(0)`. The feed
//!   is normalised by its own P0 (`sigcoh`); the photon stays in its group
//!   (`iglo = igp`, `ng = 1`) and the next break is the group top.
//! * **incoherent** (`mtd = 504`, `:1300-1467`): the Klein–Nishina kernel
//!   times the MF=27/MT=504 scattering function `S(x)`, integrated in the
//!   outgoing energy `p` (in electron-mass units) from `enow` downward in
//!   panels bounded by the sink-group boundaries and the `S(x)` grid mapped
//!   to `mu`, with a 6-point (10 when `nl > 6`) Lobatto rule per panel; the
//!   accumulated `siginc` and mean energy `ebar` give two extra slots
//!   `ff(1, ng+1) = siginc`, `ff(1, ng+2) = (e - ebar) siginc` (the
//!   cross-section and heating responses), everything then divided by
//!   `siginc`. The next critical point is the energy where a group boundary
//!   becomes the Compton edge (`egg(i)/(1 - 2 c3 egg(i))`).
//! * **pair production** (`mtd = 516`, `:1470-1509`): two annihilation
//!   photons into the group holding `epair` (`ig2pp`) and the heating slot
//!   `e - 2 epair`.
//!
//! The MF=27 tables are read by `terpa` (`endf.f90`), NJOY's sequential
//! TAB1 interpolator; [`PhotonTab1::terpa`] reproduces its value / next-break
//! / discontinuity conventions including the `shade` extrapolation just past
//! the last point.

use crate::endf::interp::{terp1, IntLaw};
use crate::groupr::kinematics::legndr;
use crate::NjoyError;

/// `c1` (`:1200`): `x [1/Angstrom] = c1 * E [eV]` at `mu = -1`... the
/// conversion `1/(hc sqrt2)` in NJOY's rounding.
const C1: f64 = 57.03156e-6;
/// `c2` (`:1201`): `pi r_e^2 / 2` in barns (`0.249467`).
const C2: f64 = 0.249467;
/// `c3` (`:1202`): `1/(m_e c^2)` per eV.
const C3: f64 = 1.95693e-6;
/// `c4` (`:1203`).
const C4: f64 = 0.0485262;
/// `c5` (`:1204`).
const C5: f64 = 20.60744;
/// `toler` (`:1205`).
const TOLER: f64 = 1.0e-6;
/// `rndoff` (`:1206`).
const RNDOFF: f64 = 1.0000001;
/// `emax` (`:1207`) — GAMINR's "no further break" sentinel.
pub const EMAX: f64 = 1.0e12;
/// `close` (`:1210`).
const CLOSE: f64 = 0.99999;
/// `shade` in `terpa` (`endf.f90`).
const SHADE: f64 = 1.00001;
/// `xbig` in `terpa`.
const XBIG: f64 = 1.0e12;

/// `epair` (`phys.f90:49`): the electron rest energy `m_e c^2` \[eV\],
/// `amasse * amu * c^2 / ev` with `amasse = 5.48579909065e-4` amu and
/// `amu = 931.49410242e6` eV.
pub const EPAIR: f64 = 5.485_799_090_65e-4 * 931.494_102_42e6;

/// One MF=27 TAB1 (form factor `F(x, Z)` for MT=502, scattering function
/// `S(x, Z)` for MT=504): `x` in inverse Angstroms ascending, the value, and
/// the ENDF interpolation regions `(NBT, INT)`.
#[derive(Debug, Clone, PartialEq)]
pub struct PhotonTab1 {
    /// `Z` — the TAB1's `C2` (`zz = pff(l+1)`, `:1225`).
    pub z: f64,
    /// Interpolation regions `(nbt, int)`, `nbt` the 1-based last point.
    pub interp: Vec<(u32, u32)>,
    /// The `(x, y)` pairs.
    pub pairs: Vec<(f64, f64)>,
}

impl PhotonTab1 {
    /// `terpa(y, x, xnext, idis, a, ip, ir)`: the value at `x`, the next
    /// tabulated `x` above it, and the discontinuity flag. Below the first
    /// point `y = 0`, `xnext = x_1`, `idis = 1`; from the last point up to
    /// `shade` times it `y = y_np` with `xnext = shade^2 x_np` (`xbig` if
    /// `y_np = 0`); beyond that `y = 0`, `xnext = xbig`. `idis = 1` for a
    /// histogram region or a duplicated `xnext`.
    pub fn terpa(&self, x: f64) -> (f64, f64, bool) {
        let p = &self.pairs;
        let np = p.len();
        if np == 0 {
            return (0.0, XBIG, false);
        }
        if x < p[0].0 {
            return (0.0, p[0].0, true);
        }
        let last = p[np - 1];
        if x >= last.0 {
            if x < SHADE * last.0 {
                let xnext = if last.1 > 0.0 {
                    SHADE * SHADE * last.0
                } else {
                    XBIG
                };
                return (last.1, xnext, false);
            }
            return (0.0, XBIG, false);
        }
        // Bracket: 1-based ip with x(ip-1) <= x < x(ip).
        let ip = p.partition_point(|q| q.0 <= x) + 1; // 1-based upper point
        let ip = ip.max(2).min(np);
        // Region: the first with nbt >= ip.
        let mut law = 2u32;
        for &(nbt, int) in &self.interp {
            if ip as u32 <= nbt {
                law = int;
                break;
            }
        }
        let (x1, y1) = p[ip - 2];
        let (x2, y2) = p[ip - 1];
        let y = if x == x1 {
            y1
        } else {
            terp1(x1, y1, x2, y2, x, IntLaw::from_code(law)).unwrap_or(y1)
        };
        let mut idis = law == 1;
        if ip < np && p[ip].0 == x2 {
            idis = true;
        }
        (y, x2, idis)
    }
}

/// `gtff`'s answer at one incident energy: `ff(il, ig)` as `ff[il][ig]`.
#[derive(Debug, Clone, PartialEq)]
pub struct PhotonFeed {
    /// `ff(il, 1..ng)`, `ig` counting from `iglo` upward; the incoherent
    /// feed carries two extra trailing slots (cross section, heating).
    pub ff: Vec<Vec<f64>>,
    /// `ng` — slots filled.
    pub ng: usize,
    /// `iglo` — 1-based sink group of slot 1.
    pub iglo: usize,
    /// `nq` — quadrature-order hint for `gpanel`.
    pub nq: usize,
    /// `enext` — next energy at which the feed changes or breaks.
    pub enext: f64,
    /// `idisc` — `enext` is a discontinuity.
    pub idisc: bool,
}

/// `igp`: the group holding `e` (`do while (e.ge.egg(igp+1)) igp=igp+1`,
/// `:1234-1237`), 1-based, capped at `ngg`.
fn group_of(e: f64, egg: &[f64]) -> usize {
    let ngg = egg.len() - 1;
    let mut igp = 1usize;
    while igp < ngg && e >= egg[igp] {
        igp += 1;
    }
    igp
}

/// Photon-interaction cross section / heating response for `mfd = 23`
/// (`:1213-1229`): `ff(1,1) = 1`, plus `ff(1,2) = e` (the heating response)
/// for MT=522 and MT=602. `ng = 0` is upstream's "illegal reaction" error.
pub fn gtff_vector(e: f64, mtd: i32) -> Result<PhotonFeed, NjoyError> {
    let ng = match mtd {
        501 | 502 | 504 | 516 => 1,
        602 | 522 => 2,
        _ => {
            return Err(NjoyError::EndfParse(format!(
                "gtff: illegal reaction MT={mtd} for cross section"
            )))
        }
    };
    let mut ff = vec![vec![0.0f64; ng]];
    if e > 0.0 {
        ff[0][0] = 1.0;
        if ng == 2 {
            ff[0][1] = e;
        }
    }
    Ok(PhotonFeed {
        ff,
        ng,
        iglo: 1,
        nq: 0,
        enext: EMAX,
        idisc: false,
    })
}

/// Coherent-scattering feed (`mtd = 502`, `:1232-1298`) at `e > 0` for `nl`
/// Legendre orders over the photon groups `egg` (`ngg + 1` bounds) with the
/// MF=27/MT=502 form factor.
pub fn gtff_coherent(e: f64, egg: &[f64], nl: usize, ff_table: &PhotonTab1) -> PhotonFeed {
    let igp = group_of(e, egg);
    let mut ff = vec![vec![0.0f64; 1]; nl];
    let mut xnow = 0.0f64;
    let mut unow = 1.0f64;
    let (mut snow, mut xnext, _) = ff_table.terpa(xnow);
    let fact = 2.0 * C2 / (C1 * C1 * e * e);
    let xlim = 2.0f64.sqrt() * C1 * e;
    let c1e = 1.0 / (C1 * e).powi(2);
    let mut arg = vec![2.0 * snow * snow; nl];
    let stest = TOLER * snow;
    let nq = if nl > 4 { 10 } else { 6 };
    let (qp, qw): (&[f64], &[f64]) = if nq == 10 {
        (&QP10, &QW10)
    } else {
        (&QP6, &QW6)
    };
    loop {
        let aq = (xnext + xnow) / 2.0;
        let bq = (xnext - xnow) / 2.0;
        for iq in 0..nq {
            let xq = aq + bq * qp[iq];
            let wq = bq * qw[iq];
            xnow = xq * RNDOFF;
            if iq > 0 {
                unow = 1.0 - c1e * xnow * xnow;
                if unow < -1.0 {
                    unow = -1.0;
                }
                let (s, xn, _) = ff_table.terpa(xnow);
                snow = s;
                xnext = xn;
                let pl = legndr(unow, nl);
                for il in 0..nl {
                    arg[il] = (1.0 + unow * unow) * snow * snow * pl[il];
                }
            }
            for il in 0..nl {
                ff[il][0] += wq * fact * xnow * arg[il];
            }
        }
        if unow <= -1.0 || xnow >= xlim || snow < stest {
            break;
        }
        if xnext > xlim {
            xnext = xlim;
        }
    }
    let sigcoh = ff[0][0];
    for f in ff.iter_mut() {
        f[0] /= sigcoh;
    }
    PhotonFeed {
        ff,
        ng: 1,
        iglo: igp,
        nq: 0,
        enext: egg[igp],
        idisc: true,
    }
}

/// Incoherent-scattering feed (`mtd = 504`, `:1300-1467`) at `e > 0`: `nl`
/// orders, sink groups `iglo..=ig` followed by the cross-section and heating
/// slots, all divided by `siginc`.
pub fn gtff_incoherent(e: f64, egg: &[f64], nl: usize, sf_table: &PhotonTab1) -> PhotonFeed {
    let ngg = egg.len() - 1;
    let zz = sf_table.z;
    let mut igp = group_of(e, egg);
    let enow = C3 * e;
    let enowi = 1.0 / enow;
    let enow2 = enow * enow;
    let mut pnow = enow;
    let mut xnow = 0.0f64;
    let xzz = C5 * (enow / 500.0).sqrt();
    let q2m = (2.0 * enow * (1.0 + enow) / (1.0 + 2.0 * enow)).powi(2);
    let use_table = xzz <= zz;
    let (mut snow, mut xnext) = if use_table {
        let (s, xn, _) = sf_table.terpa(xnow);
        (s, xn)
    } else {
        (zz, EMAX)
    };
    // ff(il, 1..ngg+3), indexed by absolute sink group while accumulating.
    let mut ff = vec![vec![0.0f64; ngg + 3]; nl];
    let mut arg = vec![0.0f64; nl];
    let ig = igp;
    let mut siginc = 0.0f64;
    let mut ebar = 0.0f64;
    if e == egg[igp - 1] {
        igp -= 1;
    }
    let nq = if nl > 6 { 10 } else { 6 };
    let (qp, qw): (&[f64], &[f64]) = if nq == 10 {
        (&QP10, &QW10)
    } else {
        (&QP6, &QW6)
    };
    let mut unow = 1.0f64;
    let mut unext;
    loop {
        // Panel upper bound in mu from the S(x) grid (`:1330-1349`).
        loop {
            let q2 = (C4 * xnext).powi(2);
            unext = -1.0;
            if q2 > q2m {
                break;
            }
            unext = 1.0 - ((1.0 - q2 * enowi) - (1.0 + q2).sqrt()) / (q2 - enow2 - 2.0 * enow);
            if unext < -1.0 {
                unext = -1.0;
            }
            if unext < CLOSE {
                break;
            }
            xnow = xnext * RNDOFF;
            let (s, xn, _) = sf_table.terpa(xnow);
            snow = s;
            xnext = xn;
        }
        let mut pnext = enow / (1.0 + 2.0 * enow);
        if igp > 0 {
            pnext = C3 * egg[igp - 1];
        }
        let px = enow / (1.0 + enow * (1.0 - unext));
        if px > pnext {
            pnext = px;
        }
        if pnext > pnow / RNDOFF {
            pnext = pnow / RNDOFF;
        }
        let aq = (pnext + pnow) / 2.0;
        let bq = (pnext - pnow) / 2.0;
        for iq in 0..nq {
            let uq = aq + bq * qp[iq];
            let wq = -C2 * bq * qw[iq];
            pnow = uq;
            if iq != 0 {
                let pnowi = 1.0 / pnow;
                unow = 1.0 + enowi - pnowi;
                if unow > 1.0 {
                    unow = 1.0;
                }
                if use_table {
                    let rm2 = (1.0 - unow) / 2.0;
                    let rm = rm2.sqrt();
                    let rt = 1.0 + 2.0 * enow * rm2;
                    xnow = C5 * 2.0 * enow * rm * (rt + enow2 * rm2).sqrt() / rt;
                    xnow *= RNDOFF;
                    let (s, xn, _) = sf_table.terpa(xnow);
                    snow = s;
                    xnext = xn;
                }
                let pl = legndr(unow, nl);
                let dk = unow - 1.0;
                let fact = snow * (enow * pnowi + pnow * enowi + dk * (2.0 + dk)) / enow2;
                for il in 0..nl {
                    arg[il] = fact * pl[il];
                }
            }
            if igp != 0 {
                for il in 0..nl {
                    ff[il][igp - 1] += wq * arg[il];
                }
            }
            siginc += wq * arg[0];
            ebar += wq * arg[0] * pnow / C3;
        }
        if unow < -CLOSE {
            break;
        }
        if igp > 0 && pnext <= C3 * egg[igp - 1] {
            igp -= 1;
        }
    }
    // Strip sink groups with zero cross section (`:1425-1440`).
    if igp == 0 {
        igp = 1;
    }
    let iglo = igp;
    let mut ng = ig - igp + 1;
    let mut out = vec![vec![0.0f64; ng + 2]; nl];
    for il in 0..nl {
        for i in 0..ng {
            out[il][i] = ff[il][iglo + i - 1];
        }
    }
    ebar /= siginc;
    out[0][ng] = siginc;
    out[0][ng + 1] = (e - ebar) * siginc;
    ng += 2;
    for f in out.iter_mut() {
        for v in f.iter_mut().take(ng) {
            *v /= siginc;
        }
    }
    // Next critical point (`:1443-1463`).
    let mut enext;
    if C3 * e >= 0.5 {
        enext = EMAX;
    } else {
        enext = EMAX;
        let mut idone = 0;
        let mut i = 0;
        while i < ngg && idone == 0 {
            i += 1;
            if C3 * egg[i - 1] >= 1.0 {
                idone = 1;
            } else {
                enext = egg[i - 1] / (1.0 - 2.0 * C3 * egg[i - 1]);
                if enext > e {
                    idone = 2;
                }
            }
        }
        if idone < 2 {
            enext = EMAX;
        }
    }
    if egg[ig] < enext {
        enext = egg[ig];
    }
    PhotonFeed {
        ff: out,
        ng,
        iglo,
        nq: nq + 2,
        enext,
        idisc: true,
    }
}

/// Pair-production feed (`mtd = 516`, `:1470-1509`) at `e > 0`: two
/// annihilation photons into `ig2pp` and the heating slot `e - 2 epair`.
pub fn gtff_pair(e: f64, ig2pp: usize) -> PhotonFeed {
    PhotonFeed {
        ff: vec![vec![2.0, e - 2.0 * EPAIR]],
        ng: 2,
        iglo: ig2pp,
        nq: 0,
        enext: EMAX,
        idisc: false,
    }
}

/// `ig2pp` (`:238-241`): the last group index whose lower bound is below
/// `epair`.
pub fn ig2pp(egg: &[f64]) -> usize {
    let mut ig2pp = 0usize;
    for (i, &eg) in egg.iter().enumerate() {
        if eg < EPAIR {
            ig2pp = i + 1;
        }
    }
    ig2pp.max(1)
}

const QP6: [f64; 6] = [
    -1.0,
    -0.765_055_32,
    -0.285_231_52,
    0.285_231_52,
    0.765_055_32,
    1.0,
];
const QW6: [f64; 6] = [
    0.066_666_67,
    0.378_474_96,
    0.554_858_38,
    0.554_858_38,
    0.378_474_96,
    0.066_666_67,
];
const QP10: [f64; 10] = [
    -1.0,
    -0.919_533_908_2,
    -0.738_773_865_1,
    -0.477_924_949_8,
    -0.165_278_957_7,
    0.165_278_957_7,
    0.477_924_949_8,
    0.738_773_865_1,
    0.919_533_908_2,
    1.0,
];
const QW10: [f64; 10] = [
    0.022_222_222_2,
    0.133_305_990_8,
    0.224_889_342_0,
    0.292_042_683_6,
    0.327_539_761_2,
    0.327_539_761_2,
    0.292_042_683_6,
    0.224_889_342_0,
    0.133_305_990_8,
    0.022_222_222_2,
];

#[cfg(test)]
mod tests {
    use super::*;

    fn form_factor(z: f64) -> PhotonTab1 {
        let xs = [0.0, 0.01, 0.1, 0.5, 1.0, 5.0, 100.0, 1.0e9];
        PhotonTab1 {
            z,
            interp: vec![(xs.len() as u32, 2)],
            pairs: xs
                .iter()
                .map(|&x| (x, z / (1.0 + (x / 0.6f64).powi(2)).powi(2)))
                .collect(),
        }
    }

    /// `terpa` conventions: below the first point `(0, x_1, idis)`, on a
    /// point its own value, between points the region law, at/just past
    /// the last point the last value with `xnext = shade^2 x_np`, beyond
    /// `shade x_np` zero with `xbig`.
    #[test]
    fn terpa_conventions() {
        let t = form_factor(6.0);
        assert_eq!(t.terpa(-1.0), (0.0, 0.0, true));
        let (y, xn, _) = t.terpa(0.0);
        assert_eq!((y, xn), (6.0, 0.01));
        let (y, xn, _) = t.terpa(0.3);
        let expect = t.pairs[2].1 + (t.pairs[3].1 - t.pairs[2].1) * (0.3 - 0.1) / 0.4;
        assert!((y - expect).abs() < 1e-14 && xn == 0.5);
        let (y, xn, _) = t.terpa(1.0e9);
        assert!((y - t.pairs[7].1).abs() < 1e-30 && (xn - SHADE * SHADE * 1.0e9).abs() < 1.0);
        assert_eq!(t.terpa(1.1e9), (0.0, XBIG, false));
    }

    /// At low energy the momentum transfer stays tiny (`x_max = sqrt2 c1 e
    /// = 2.4e-3 /Angstrom` at 30 eV, where `F^2` has fallen by 6.5e-5), so
    /// the coherent feed is the pure Rayleigh `(1 + mu^2)` distribution:
    /// `P1` component `0`, `P2` component `int (1+mu^2) P2 / int (1+mu^2)
    /// = (4/15)/(8/3) = 0.1`. The table's first interval must lie inside
    /// `x_max`: upstream
    /// integrates the first panel `[0, x_1]` *before* capping `xnext` at
    /// `xlim` (`:1290-1296`), so a coarse first interval at very low energy
    /// carries the clamped `mu = -1` integrand past the physical range —
    /// a faithful quirk this port keeps (with the 0.01-spaced table the
    /// P2 component comes out 0.083 at 100 eV). Result (2026-09-10): P1
    /// within 5e-5 of 0 (1.7e-4 at 100 eV — the F^2 fall), P2 within 2e-4
    /// of 0.1.
    #[test]
    fn coherent_low_energy_is_rayleigh() {
        let xs = [0.0, 0.001, 0.003, 0.01, 0.1, 0.5, 1.0, 5.0, 100.0, 1.0e9];
        let t = PhotonTab1 {
            z: 6.0,
            interp: vec![(xs.len() as u32, 2)],
            pairs: xs
                .iter()
                .map(|&x| (x, 6.0 / (1.0 + (x / 0.6f64).powi(2)).powi(2)))
                .collect(),
        };
        let egg = [10.0, 1.0e3, 1.0e4];
        let f = gtff_coherent(30.0, &egg, 4, &t);
        assert_eq!((f.ng, f.iglo), (1, 1));
        assert!((f.ff[0][0] - 1.0).abs() < 1e-12);
        assert!(f.ff[1][0].abs() < 5e-5, "P1 {}", f.ff[1][0]);
        assert!((f.ff[2][0] - 0.1).abs() < 2e-4, "P2 {}", f.ff[2][0]);
        assert_eq!(f.enext, 1.0e3);
    }

    /// The pair feed and `ig2pp`: `epair` is 510 998.95 eV, so a structure
    /// `1e4, 1e5, 5e5, 1e6` puts the annihilation photons in group 3.
    #[test]
    fn pair_feed_and_ig2pp() {
        assert!((EPAIR - 510_998.95).abs() < 0.01);
        let egg = [1.0e4, 1.0e5, 5.0e5, 1.0e6, 2.0e6];
        assert_eq!(ig2pp(&egg), 3);
        let f = gtff_pair(3.0e6, 3);
        assert_eq!(f.ff[0][0], 2.0);
        assert!((f.ff[0][1] - (3.0e6 - 2.0 * EPAIR)).abs() < 1e-6);
    }
}
