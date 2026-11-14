// Translated from NJOY2016.80 (commit ac5adf5) `src/matxsr.f90`: `matrix`
// (`:1875-2026`), `band` (`:2028-2121`) and `shufl` (`:2123-2325`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The MATXS matrix blocks: `band` finds each secondary group's band of
//! initial groups, `shufl` packs the Legendre components into those bands
//! (several passes when the blocks would overflow the container), and `matrix`
//! writes the control record, the blocks and the constant sub-block.
//!
//! Upstream kept: `maxord` is a module variable that `band` sets to `nl - 1`
//! of the last head read, and the container is not cleared between `shufl`'s
//! passes.

use crate::errorr::gendf::{GendfKind, GendfRecord};
use crate::matxsr::data::{words, Ctl2, Cursor, Matxs, ISIZA, MULT};
use crate::matxsr::hname::hname;
use crate::NjoyError;

fn k4(x: f64) -> f32 {
    x as f32
}

/// The band of initial groups for each secondary group (`band`'s `jg1lo`,
/// `jg1hi`, 1-based).
struct Bands {
    lo: Vec<i32>,
    hi: Vec<i32>,
}

impl Bands {
    fn np(&self, i: usize) -> i32 {
        (self.hi[i] - self.lo[i] + 1).max(0)
    }
}

/// `matrix` (`:1875-2026`).
///
/// # Errors
/// A scratch tape that ends early.
pub(crate) fn matrix(m: &mut Matxs, mfd: i32, hp: char, n2d: usize, scr: &[GendfRecord], iref: Option<&[GendfRecord]>) -> Result<(), NjoyError> {
    let (ning, noutg) = (m.ning, m.noutg);
    let nwmcr = MULT + 2 + 2 * noutg as i32;
    let imcon = m.next;
    m.next = imcon + nwmcr;
    let nwdc = noutg + ning;
    m.next += nwdc as i32;
    m.next += 2 * noutg as i32;
    let imdat = m.next;
    let mut sc = Cursor { recs: scr.to_vec(), pos: 0 };
    let mut rf = iref.map(|r| Cursor { recs: r.to_vec(), pos: 0 });
    sc.next()?;
    if let Some(r) = rf.as_mut() {
        r.next()?;
    }
    m.nritem = 0;
    let mut region = vec![0.0f32; (ISIZA - imdat + 1).max(1) as usize + 1];
    for _ in 0..n2d {
        let mut cdat = vec![0.0f32; nwdc + 1];
        let mut lord1 = 0i32;
        let mut irite = 0i32;
        let mut jconst = 0i32;
        let head = loop {
            if let Some(r) = rf.as_mut() {
                r.next()?;
            }
            let h = sc.next()?;
            let nz = h.n[1] as usize;
            if nz >= m.iz && (h.mf == mfd || (h.mf == 17 && mfd == 16)) {
                break h;
            }
            if let Some(r) = rf.as_mut() {
                r.tosend(None)?;
            }
            sc.tosend(None)?;
        };
        let lr = head.n[2];
        let mut rhead = None;
        if let Some(r) = rf.as_mut() {
            r.findg(head.mat, head.mf, head.mt)?;
            rhead = Some(r.next()?);
        }
        region.iter_mut().for_each(|v| *v = 0.0);
        let name = hname(hp, head.mt, lr, 0);
        // `band`: `nl = l1h` of the last head read.
        let nl_band = rhead.as_ref().map_or(head.n[0], |h| h.n[0]);
        m.maxord = nl_band - 1;
        let (bands, sec, rsec) = band(m, &mut sc, rf.as_mut())?;
        let mut sec_all = vec![head.clone()];
        sec_all.extend(sec);
        let rsec_all = rhead.map(|h| {
            let mut v = vec![h];
            v.extend(rsec);
            v
        });
        shufl(m, &sec_all, rsec_all.as_deref(), &bands, &mut region, &mut cdat, imdat, &mut lord1, &mut jconst, &mut irite)?;
        let mut ints = vec![0i32; 2 + 2 * noutg];
        for i in 1..=noutg {
            ints[1 + i] = bands.np(i);
            ints[1 + noutg + i] = bands.hi[i];
        }
        sc.tosend(None)?;
        if irite != 0 {
            ints[0] = lord1;
            ints[1] = jconst;
            m.scr2.push(Ctl2::Matrix { name, ints });
            irite += 1;
        } else {
            if lord1 == 0 {
                lord1 = m.maxord + 1;
            }
            ints[0] = lord1;
            ints[1] = jconst;
            ints[2] = 1;
            ints[2 + noutg] = 1;
            m.scr2.push(Ctl2::Matrix { name, ints });
            irite += 1;
            let lordn = (m.maxord + 1).max(0) as usize;
            m.scr3.push(vec![0.0f32; lordn]);
            irite += 1;
        }
        if jconst != 0 {
            let ioff = ning as i32 - jconst;
            for ik in 1..=jconst {
                let loca = (noutg as i32 + ik) as usize;
                cdat[loca] = cdat[(loca as i32 + ioff) as usize];
            }
            let nwds = noutg + jconst as usize;
            m.scr3.push(cdat[1..=nwds].to_vec());
            irite += 1;
        }
        m.nritem += irite;
    }
    m.next = imcon;
    Ok(())
}

/// `band` (`:2028-2121`): read the reaction's LISTs (returned, as copied to
/// `nscrr`/`irefr`) and the bands.
fn band(m: &Matxs, sc: &mut Cursor, mut rf: Option<&mut Cursor>) -> Result<(Bands, Vec<GendfRecord>, Vec<GendfRecord>), NjoyError> {
    let (ning, noutg) = (m.ning as i32, m.noutg);
    let mut b = Bands { lo: vec![ning + 1; noutg + 1], hi: vec![0; noutg + 1] };
    let mut sec = Vec::new();
    let mut rsec = Vec::new();
    let mut igr = 0;
    let mut ig = 0;
    let (mut ng2r, mut ig2lor) = (0, 0);
    while ig < ning {
        let l = sc.next()?;
        if l.kind != GendfKind::List {
            return Err(NjoyError::InvalidInput("band: expected a LIST record".into()));
        }
        let (ng2, ig2lo) = (l.n[0], l.n[1]);
        ig = l.n[3];
        sec.push(l);
        loop {
            // label 120.
            if let Some(r) = rf.as_deref_mut() {
                if ig > igr {
                    let l = r.next()?;
                    ng2r = l.n[0];
                    ig2lor = l.n[1];
                    igr = l.n[3];
                    rsec.push(l);
                }
            }
            if ig != 0 && ig2lo != 0 {
                let has_ref = rf.is_some();
                if !has_ref || ig <= igr {
                    let jg1 = ning + 1 - ig;
                    for ik in 2..=ng2 {
                        let jg2 = (noutg as i32 + 1 - (ig2lo + ik - 2)) as usize;
                        if jg1 < b.lo[jg2] {
                            b.lo[jg2] = jg1;
                        }
                        if jg1 > b.hi[jg2] {
                            b.hi[jg2] = jg1;
                        }
                    }
                }
                if has_ref && igr <= ig {
                    let jg1 = ning + 1 - igr;
                    for ik in 2..=ng2r {
                        let jg2 = (noutg as i32 + 1 - (ig2lor + ik - 2)) as usize;
                        if jg1 < b.lo[jg2] {
                            b.lo[jg2] = jg1;
                        }
                        if jg1 > b.hi[jg2] {
                            b.hi[jg2] = jg1;
                        }
                    }
                }
            }
            if !(rf.is_some() && igr < ig) {
                break;
            }
        }
    }
    Ok((b, sec, rsec))
}

/// `shufl` (`:2123-2325`).
#[allow(clippy::too_many_arguments)]
fn shufl(
    m: &mut Matxs,
    sec: &[GendfRecord],
    rsec: Option<&[GendfRecord]>,
    bands: &Bands,
    region: &mut [f32],
    cdat: &mut [f32],
    imdat: i32,
    lord1: &mut i32,
    jconst: &mut i32,
    irite: &mut i32,
) -> Result<(), NjoyError> {
    let (ning, noutg) = (m.ning as i32, m.noutg);
    let lz = 6usize;
    let iz = m.iz;
    let maxw = m.maxw;
    let mut ng2z = 0usize;
    let mut imax: Option<usize> = None;
    loop {
        // label 100.
        let i0 = ng2z + 1;
        let mut num = 0i32;
        let mut nwds = 0i32;
        let mut i = i0 - 1;
        let mut idone = false;
        while i < noutg && !idone {
            i += 1;
            let n = bands.np(i) * (m.maxord + 1);
            nwds += n;
            if imdat + nwds > ISIZA {
                idone = true;
            } else {
                if num + n >= maxw {
                    imax = Some(i - 1);
                    num = 0;
                }
                num += n;
            }
        }
        if !idone {
            imax = Some(noutg);
        }
        let imax_v = imax.ok_or_else(|| NjoyError::InvalidInput("shufl: container too small for one block (imax undefined upstream)".into()))?;
        let head = &sec[0];
        let nl = head.n[0] as usize;
        let nz = head.n[1] as usize;
        let mut k = 1usize;
        let mut kr = 1usize;
        let mut igr = 0i32;
        let (mut bref, mut ng2r, mut ig2lor): (Vec<f64>, i32, i32) = (Vec::new(), 0, 0);
        loop {
            // label 110.
            let l = sec.get(k).ok_or_else(|| NjoyError::InvalidInput("shufl: section ends early".into()))?;
            k += 1;
            let b = words(l);
            let (ng2, ig2lo, ig) = (l.n[0], l.n[1], l.n[3]);
            loop {
                // label 120.
                if let Some(rs) = rsec {
                    if ig > igr {
                        let r = rs.get(kr).ok_or_else(|| NjoyError::InvalidInput("shufl: reference ends early".into()))?;
                        kr += 1;
                        bref = words(r);
                        ng2r = r.n[0];
                        ig2lor = r.n[1];
                        igr = r.n[3];
                    }
                }
                let at = |x: &[f64], j: usize| x.get(j).copied().unwrap_or(0.0);
                if ig == 0 {
                    // label 135.
                    for ik in 1..=ng2 {
                        let jg2 = (noutg as i32 - ig2lo - ik + 2) as usize;
                        cdat[jg2] = k4(at(&b, lz + ik as usize));
                    }
                } else if ig2lo == 0 {
                    // label 140.
                    *jconst = ig;
                    let jg1 = (ning + 1 - ig) as usize;
                    cdat[noutg + jg1] = k4(at(&b, lz + 2));
                } else {
                    // label 145.
                    let store = |ng2: i32, ig2lo: i32, igg: i32, src: &[f64], with_iz: bool, sub: bool, region: &mut [f32]| {
                        let jg1 = ning + 1 - igg;
                        for ik in 2..=ng2 {
                            let niloc = lz + nl * nz * (ik as usize - 1) + if with_iz { nl * (iz - 1) } else { 0 };
                            let jg2 = (noutg as i32 + 1 - (ig2lo + ik - 2)) as usize;
                            if jg2 >= i0 && jg2 <= imax_v {
                                let kp = bands.hi[jg2];
                                let mut jp = 0i32;
                                let mut np = 0i32;
                                let mut i = i0 - 1;
                                while i < noutg {
                                    i += 1;
                                    np = bands.np(i);
                                    if i == jg2 {
                                        break;
                                    }
                                    jp += np;
                                }
                                let noloc = nl as i32 * jp + kp - jg1;
                                for il in 1..=nl {
                                    let idx = (noloc + np * (il as i32 - 1)) as usize;
                                    let v = at(src, niloc + il);
                                    if let Some(slot) = region.get_mut(idx) {
                                        *slot = if sub { k4(f64::from(*slot) - v) } else { k4(v) };
                                    }
                                }
                            }
                        }
                    };
                    if rsec.is_none() || ig <= igr {
                        store(ng2, ig2lo, ig, &b, true, false, region);
                    }
                    if rsec.is_some() && igr <= ig {
                        store(ng2r, ig2lor, igr, &bref, false, true, region);
                    }
                }
                // label 240.
                if !(rsec.is_some() && igr < ig) {
                    break;
                }
            }
            if ig >= ning {
                break;
            }
        }
        if nl as i32 + 1 > *lord1 {
            *lord1 = nl as i32 + 1;
        }
        if *lord1 > m.maxord + 1 {
            *lord1 = m.maxord + 1;
        }
        // Write out this range of sub-block data.
        let mut nwds = 0i32;
        let mut i1 = 0usize;
        let mut np = 0i32;
        for i in i0..=imax_v + 1 {
            if i <= imax_v {
                np = bands.np(i) * nl as i32;
                if nwds + np < maxw {
                    nwds += np;
                    continue;
                }
            }
            if nwds == 0 {
                nwds += np;
            } else {
                m.scr3.push(region[i1..i1 + nwds as usize].to_vec());
                *irite += 1;
                i1 += nwds as usize;
                nwds = np;
            }
        }
        ng2z = imax_v;
        if ng2z >= noutg {
            break;
        }
    }
    Ok(())
}
