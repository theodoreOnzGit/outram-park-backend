// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine acelcp`, l.9545-9636: the angular representation of the
//     elastic recoil particle (`mt.eq.2`) and its heating;
//   - `subroutine acelcp`, l.10601-10742: the LAW = 6 (phase space) energy
//     distribution and its heating.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Two `acelcp` branches kept apart from [`super::acelcp`] (that file is at
//! its size cap): the elastic recoil's reversed angular distribution, and the
//! LAW = 6 phase-space block (gh:#1485).
//!
//! Both are called from `acelcp`'s loops, with upstream's 1-based XSS indices.

use petir::mathf::RealMath;

use crate::endf::interp::terpa;
use crate::mixr::mix::sigfig;

use super::acelcp::Xss;

const SMALL: f64 = 1.0e-30;
const ELOW: f64 = 1.0e-5;

/// The elastic recoil's angular distribution and heating (`mt.eq.2`,
/// `acefc.f90:9555-9636`): the scattered particle's distribution at `land`
/// with the cosines reversed, the pdf renormalised and the cdf rebuilt, one
/// block per incident energy. The block's locators are negative (tabulated).
///
/// `landh_slot` is `landh+jp-1`, `andh` the first ANDH word, `land`/`and` the
/// table's own LAND and AND pointers, `hpd` the production's HPD block with
/// `it` its first ESZ index; the recoil heating `h(E) * sigma(E)` is added to
/// the production heating at `hpd+2+naa`, `h` log-log (`INT = 5`) in the table
/// built here.
#[allow(clippy::too_many_arguments)]
pub(super) fn elastic_recoil_angular(
    x: &mut Xss,
    next: &mut usize,
    landh_slot: usize,
    andh: usize,
    land: usize,
    and: usize,
    hpd: usize,
    it: usize,
    nes: usize,
    esz: usize,
    amass: f64,
) {
    x.n(landh_slot, (*next - andh + 1) as f64);
    let mut loce = x.get(land).round() as usize + and - 1;
    let ne = x.get(loce).round() as usize;
    x.n(*next, ne as f64);
    let leee = *next;
    for ie in 1..=ne {
        let v = x.get(loce + ie);
        x.r(*next + ie, v);
    }
    let nb = *next + ne;
    *next += 1 + 2 * ne;
    loce += 1 + 2 * ne;
    let mut heat: Vec<(f64, f64)> = Vec::with_capacity(ne);
    for ie in 1..=ne {
        let int = x.get(loce).round() as i32;
        let n = x.get(loce + 1).round() as usize;
        let nx = *next;
        x.n(nb + ie, -((nx - andh + 1) as f64));
        x.n(nx, f64::from(int));
        x.n(nx + 1, n as f64);
        let mut ubar = 0.0;
        for i in 1..=n {
            let mu = -x.get(loce + 2 + n - i);
            x.r(nx + 1 + i, mu);
            let mut pv = x.get(loce + 2 + 2 * n - i);
            if pv < SMALL {
                pv = 0.0;
            }
            x.r(nx + 1 + n + i, pv);
            if i == 1 {
                x.r(nx + 1 + 2 * n + 1, 0.0);
                ubar = 0.0;
            }
            if i > 1 {
                let (m1, m0) = (x.get(nx + 1 + i), x.get(nx + i));
                let (p1, p0) = (x.get(nx + 1 + n + i), x.get(nx + n + i));
                let c0 = x.get(nx + 2 * n + i);
                if int == 1 {
                    let sum = c0 + p0 * (m1 - m0);
                    x.r(nx + 1 + 2 * n + i, sigfig(sum, 7, 0));
                    ubar = ubar + p0 * (m1 - m0) + (m1 + m0) / 2.0;
                }
                if int == 2 {
                    let sum = c0 + (p1 + p0) * (m1 - m0) / 2.0;
                    x.r(nx + 1 + 2 * n + i, sigfig(sum, 7, 0));
                    ubar = ubar + (p1 + p0) * (m1 - m0) * (m1 + m0) / 4.0;
                }
            }
        }
        let renorm = 1.0 / x.get(nx + 1 + 3 * n);
        for i in 1..=n {
            let pv = x.get(nx + 1 + n + i);
            x.r(nx + 1 + n + i, sigfig(renorm * pv, 7, 0));
            let cv = x.get(nx + 1 + 2 * n + i);
            x.r(nx + 1 + 2 * n + i, sigfig(renorm * cv, 9, 0));
        }
        *next += 2 + 3 * n;
        loce += 2 + 3 * n;
        let e = x.get(leee + ie);
        heat.push((e, 2.0 * amass * e * (1.0 + ubar) / ((1.0 + amass) * (1.0 + amass))));
    }
    let naa = x.get(hpd + 1).round() as usize;
    let hint = vec![(ne as u32, 5)];
    for ie in it..=nes {
        let e = x.get(esz + ie - 1);
        let (h, _, _) = terpa(&hint, &heat, e);
        let h0 = x.get(hpd + 2 + naa + ie - it);
        let s = x.get(hpd + 2 + ie - it);
        x.r(hpd + 2 + naa + ie - it, h0 + h * s);
    }
}

/// The LAW = 6 phase-space grid `(x, pdf, cdf)` of `acelcp`
/// (`acefc.f90:10620-10668`), `n = nn + 1` points, the first the origin. It
/// is not `acelf6`'s grid: the end test is `1 - 1e-5` and the step switch
/// `0.1 - 1e-6` here.
pub(super) fn law6_grid(npsx: i32) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let step1 = 10f64.r_powf(1.0 / 5.0);
    let step2 = 1.0 / 50.0;
    let test1 = 1.0 + 1.0 / 100_000.0;
    let test2 = 1.0 / 10.0 - 1.0 / 1_000_000.0;
    let test3 = 1.0 - 1.0 / 100_000.0;
    let mut xx = ELOW;
    let mut n = 1usize;
    while xx < test1 {
        n += 1;
        if xx < test2 {
            xx *= step1;
        } else {
            xx += step2;
        }
    }
    let nn = n;
    let (mut xl, mut pl, mut yn) = (0.0f64, 0.0f64, 0.0f64);
    let mut xs = vec![0.0; nn];
    let mut ps = vec![0.0; nn];
    let mut cs = vec![0.0; nn];
    xs[0] = xl;
    ps[0] = pl;
    cs[0] = yn;
    xx = ELOW;
    let mut n = 1usize;
    while xx < test1 {
        n += 1;
        let pn;
        if xx > test3 {
            xx = 1.0;
            pn = 0.0;
        } else {
            let rn = 3.0;
            pn = xx.sqrt() * (1.0 - xx).r_powf(rn * f64::from(npsx) / 2.0 - 4.0);
        }
        yn += (xx - xl) * (pn + pl) / 2.0;
        xs[n - 1] = sigfig(xx, 7, 0);
        ps[n - 1] = pn;
        cs[n - 1] = yn;
        xl = xx;
        pl = pn;
        if xx < test2 {
            xx *= step1;
        } else {
            xx += step2;
        }
    }
    let sum = yn;
    for k in 0..nn {
        ps[k] = sigfig(ps[k] / sum, 7, 0);
        cs[k] = sigfig(cs[k] / sum, 9, 0);
    }
    (xs, ps, cs)
}

/// The mean outgoing-energy factor `h` of LAW = 6 at incident energy `e`
/// [MeV] (`acefc.f90:10677-10736`): eleven lab cosines, the energy-angle
/// density of each phase-space point folded into `sum / chk`, then trapezoids
/// over the cosines.
pub(super) fn law6_h(e: f64, awi: f64, awr: f64, awp: f64, apsx: f64, q: f64, xs: &[f64], ps: &[f64]) -> f64 {
    let nn = xs.len();
    let mut ecm = e * awi / (awi + awr);
    ecm = ecm * awp / (awi + awr);
    let ea = e * awr / (awi + awr) + q;
    let eim = ea * (apsx - awp) / apsx;
    let mut h = 0.0;
    let mut chek = 0.0;
    let dmu = 1.0 / 5.0;
    let (mut suml, mut chkl) = (0.0f64, 0.0f64);
    let (mut pp1l, mut pp2l) = (0.0f64, 0.0f64);
    for imu in 1..=11 {
        let amuu = -1.0 + dmu * f64::from(imu - 1);
        let (mut e1l, mut e2l) = (0.0f64, 0.0f64);
        let mut sum = 0.0;
        let mut chk = 0.0;
        for iep in 0..nn {
            let ep = xs[iep] * eim;
            let pp = ps[iep] / eim;
            let disc = ep - ecm * (1.0 - amuu * amuu);
            if disc >= 0.0 {
                let v1 = amuu * ecm.sqrt() + disc.sqrt();
                let v2 = amuu * ecm.sqrt() - disc.sqrt();
                let e1 = v1 * v1;
                let mut e2 = v2 * v2;
                if v2 < 0.0 {
                    e2 = -e2;
                }
                let mut pp1 = 0.0;
                if ep > 0.0 {
                    pp1 = pp * e1.sqrt() / ep.sqrt();
                }
                let mut pp2 = 0.0;
                if v2 > 0.0 && ep > 0.0 {
                    pp2 = pp * e2.sqrt() / ep.sqrt();
                }
                if e1l > 0.0 {
                    let mut de = e1 - e1l;
                    if de < 0.0 {
                        de = -de;
                    }
                    sum += de * (e1 * pp1 + e1l * pp1l) / 2.0;
                    chk += de * (pp1 + pp1l) / 2.0;
                    if e2 > 0.0 && e2l > 0.0 {
                        let mut de = e2l - e2;
                        if de < 0.0 {
                            de = -de;
                        }
                        sum += de * (e2 * pp2 + e2l * pp2l) / 2.0;
                        chk += de * (pp2 + pp2l) / 2.0;
                    }
                }
                e1l = e1;
                e2l = e2;
                pp1l = pp1;
                pp2l = pp2;
            }
        }
        if imu > 1 {
            h += (sum + suml) / 10.0;
            chek += (chk + chkl) / 10.0;
        }
        suml = sum;
        chkl = chk;
    }
    if chek > 0.0 {
        h /= chek;
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `law6_h` against NJOY2016's own value of `h` at each of the 119
    /// incident energies of test 51's MT=28 neutron (LAW = 6, `npsx = 3`).
    ///
    /// **Methodology.** `verification_and_validation/acer_charged/
    /// law6_h_instrument.diff` adds one `write` after `h=h/chek`
    /// (`acefc.f90:10736`); NJOY2016.80 built from that source (gfortran 13.3,
    /// `-O0`) was run on deck 51 and printed `ie, e, q, awr, awi, awp, apsx,
    /// npsx, h` at 17 figures (`law6_h_reference.txt`). The grid the heating
    /// reads is [`law6_grid`], whose words the deck-51 ACE comparison already
    /// holds identical.
    ///
    /// **Pass criterion.** Bit-identical `h`; a difference is reported with its
    /// relative size. The heating itself never reaches the ACE file for this
    /// reaction (`ip = 1` zeroes the neutron production heating,
    /// `acefc.f90:10918`), which is why it needs this gate and not the deck's.
    #[test]
    fn law6_heating_matches_njoy2016() {
        const REF: &str = include_str!("../../verification_and_validation/acer_charged/law6_h_reference.txt");
        let (mut n, mut same, mut worst) = (0, 0, 0.0f64);
        for l in REF.lines() {
            let f: Vec<f64> = l.split_whitespace().map(|v| v.parse().unwrap()).collect();
            let (e, q, awr, awi, awp, apsx, npsx, h) = (f[1], f[2], f[3], f[4], f[5], f[6], f[7], f[8]);
            let (xs, ps, _) = law6_grid(npsx as i32);
            let ours = law6_h(e, awi, awr, awp, apsx, q, &xs, &ps);
            n += 1;
            if ours.to_bits() == h.to_bits() {
                same += 1;
            } else {
                worst = worst.max(((ours - h) / h).abs());
            }
        }
        println!("law6_h: {same}/{n} bit-identical, worst relative difference {worst:e}");
        assert_eq!(n, 119);
        assert_eq!(same, n, "worst relative difference {worst:e}");
    }

    /// [`elastic_recoil_angular`] against NJOY2016's own `mt.eq.2` block of
    /// `acelcp` (`acefc.f90:9555-9636`), on the same input words (gh:#1490).
    ///
    /// **Methodology.** `verification_and_validation/acer_charged/
    /// elastic_recoil_instrument.diff` adds `write`s to that block; NJOY2016.80
    /// built from the patched source (gfortran 13.3, `-O3`, every file) was run
    /// on test deck 54 (p + H-3, the only held deck whose ACE has an elastic
    /// recoil: deck 62's empty dictionary gives it no production at all), and
    /// its unit 31 is byte-identical to the unpatched build's. It printed, as
    /// IEEE-754 bits (`elastic_recoil_reference.txt`): the block's pointers
    /// (`HEAD`), `awr` and `awi`, the scattered particle's AND words it reads
    /// (`IN`, 4 628 words), the ESZ energies, the production's HPD block
    /// before the recoil heating (`HPD`), and after it the recoil's AND words
    /// (`OUT`, 4 628) and the heating accumulators (`HOUT`, 140). The test puts
    /// the input words at the same 1-based XSS indices and calls this function.
    ///
    /// **Prediction (posted on #1490 before the run):** every output word bit-
    /// identical. **Pass criterion:** the same `next`, the same LANDH word and
    /// every `OUT` and `HOUT` word bit-identical. **Result (2026-11-12, rustc
    /// 1.99.1, first run):** 4 768 / 4 768 words bit-identical. The `UBAR`
    /// lines (each recoil energy's `e`, `ubar` and heating point) are kept
    /// for diagnosis only; the heating points reach `HOUT` through `h * sigma`.
    #[test]
    fn elastic_recoil_matches_njoy2016() {
        use super::super::acelcp::Xss;
        use super::super::AceTable;
        const REF: &str = include_str!("../../verification_and_validation/acer_charged/elastic_recoil_reference.txt");
        let hex = |s: &str| f64::from_bits(u64::from_str_radix(s, 16).unwrap());
        let mut head: Vec<usize> = Vec::new();
        let (mut awr, mut awi) = (0.0, 0.0);
        let mut regions: Vec<(String, usize, Vec<f64>)> = Vec::new();
        let mut lines = REF.lines();
        while let Some(l) = lines.next() {
            let f: Vec<&str> = l.split_whitespace().collect();
            match f[0] {
                "HEAD" => head = f[1..].iter().map(|v| v.parse().unwrap()).collect(),
                "AWR" => (awr, awi) = (hex(f[1]), hex(f[2])),
                "UBAR" => {}
                name => {
                    let (start, n): (usize, usize) = (f[1].parse().unwrap(), f[2].parse().unwrap());
                    let mut w = Vec::with_capacity(n);
                    while w.len() < n {
                        w.extend(lines.next().unwrap().split_whitespace().map(hex));
                    }
                    regions.push((name.to_string(), start, w));
                }
            }
        }
        let region = |name: &str| regions.iter().find(|r| r.0 == name).unwrap();
        let (land, and, slot, andh, hpd, it, nes, esz, next0, land_word) =
            (head[0], head[1], head[2], head[3], head[4], head[5], head[6], head[7], head[8], head[9]);
        let len = regions.iter().map(|r| r.1 + r.2.len()).max().unwrap() + 8;
        let mut table = AceTable {
            zaid: String::new(),
            awr,
            kt_mev: 0.0,
            date: String::new(),
            comment: String::new(),
            mat_id: String::new(),
            iz: [0; 16],
            aw: [0.0; 16],
            nxs: [0; 16],
            jxs: [0; 32],
            xss: vec![0.0; len],
            xss_is_int: vec![false; len],
        };
        table.xss[land - 1] = land_word as f64;
        for name in ["IN", "ESZ", "HPD"] {
            let (_, start, w) = region(name);
            table.xss[start - 1..start - 1 + w.len()].copy_from_slice(w);
        }
        let mut x = Xss::over(&mut table);
        let mut next = next0;
        elastic_recoil_angular(&mut x, &mut next, slot, andh, land, and, hpd, it, nes, esz, awr / awi);
        let out = region("OUT");
        assert_eq!(next, out.1 + out.2.len(), "next after the block");
        assert_eq!(x.get(slot), (next0 - andh + 1) as f64, "LANDH word");
        let (mut n, mut same, mut worst) = (0, 0, 0.0f64);
        for name in ["OUT", "HOUT"] {
            let (_, start, w) = region(name);
            for (i, &theirs) in w.iter().enumerate() {
                let ours = x.get(start + i);
                n += 1;
                if ours.to_bits() == theirs.to_bits() {
                    same += 1;
                } else {
                    let rel = if theirs == 0.0 { ours.abs() } else { ((ours - theirs) / theirs).abs() };
                    worst = worst.max(rel);
                    println!("{name} word {}: ours {ours:e} njoy {theirs:e}", start + i);
                }
            }
        }
        println!("elastic recoil: {same}/{n} words bit-identical, worst relative difference {worst:e}");
        assert_eq!(n, 4628 + 140);
        assert_eq!(same, n, "worst relative difference {worst:e}");
    }

    /// `law6_grid`: 3 * nn points, strictly increasing abscissae from 0 to 1,
    /// the cdf ending at 1 (to the 9 figures it is stored in) and the pdf non-negative (the shape
    /// `sqrt(x)(1-x)^(3 npsx/2-4)`); its words are gated by test 51's ACE.
    #[test]
    fn law6_grid_is_a_normalised_phase_space_distribution() {
        let (xs, ps, cs) = law6_grid(3);
        assert_eq!(xs.len(), ps.len());
        assert_eq!(xs[0], 0.0);
        // `sigfig(1, 7, 0)` is 1 + 1e-13 (its bias), as in NJOY's own words
        assert!((xs.last().unwrap() - 1.0).abs() < 1.0e-12);
        assert!(xs.windows(2).all(|w| w[1] > w[0]));
        assert!(ps.iter().all(|&p| p >= 0.0));
        assert!((cs.last().unwrap() - 1.0).abs() < 1.0e-8, "cdf ends at {}", cs.last().unwrap());
    }
}
