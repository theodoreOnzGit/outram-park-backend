// Translated from NJOY2016.80 (commit ac5adf5) `src/matxsr.f90`: `mtxdat`
// (`:908-1270`), `findg` (`:1272-1326`) and `vector` (`:1538-1873`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `mtxdat` and `vector`: the GENDF tapes read, each material's sections
//! copied to the scratch tape, the vectors and matrices built, and the
//! scratch files `nscrt2`, `nscrt3` and `nscrt6` that `cmatxs` writes out.
//!
//! **Precision:** the container array `a` is `real(k4)` (`:11`). A value
//! stored in it is rounded to `f32`; `a + b` with `b` a `real(kr)` is a double
//! sum rounded to `f32`; `a + a` is a single-precision sum (`:1800`).

use crate::errorr::gendf::{GendfKind, GendfRecord, GendfTape};
use crate::matxsr::cards::MatxsrCards;
use crate::matxsr::hname::hname;
use crate::NjoyError;

/// `mult`: 8-byte words per Hollerith entry, in 4-byte units.
pub(crate) const MULT: i32 = 2;
/// `isiza`.
pub(crate) const ISIZA: i32 = 200_000;

/// One record of `nscrt2`.
#[derive(Debug, Clone)]
pub(crate) enum Ctl2 {
    /// A vector control record: names, then `nfg`, `nlg`.
    Vector { names: Vec<String>, ints: Vec<i32> },
    /// A matrix control record: the name, then `lord jconst jband ijj`.
    Matrix { name: String, ints: Vec<i32> },
}

/// One material control record of `nscrt6`.
#[derive(Debug, Clone)]
pub(crate) struct MatCtl {
    pub name: String,
    pub awr: f32,
    /// `(temp, sigz, itype, n1d, n2d, locs)`.
    pub subs: Vec<(f32, f32, i32, i32, i32, i32)>,
}

/// A GENDF tape being read record by record (`contio`/`listio`/`tosend`),
/// or a scratch tape written the same way.
#[derive(Debug, Clone, Default)]
pub(crate) struct Cursor {
    pub recs: Vec<GendfRecord>,
    pub pos: usize,
}

/// `b(1:6)` and the LIST words, 1-based (`b[0]` unused).
pub(crate) fn words(r: &GendfRecord) -> Vec<f64> {
    let mut b = vec![0.0, r.c[0], r.c[1], f64::from(r.n[0]), f64::from(r.n[1]), f64::from(r.n[2]), f64::from(r.n[3])];
    b.extend_from_slice(&r.data);
    b
}

impl Cursor {
    pub(crate) fn next(&mut self) -> Result<GendfRecord, NjoyError> {
        let r = self.recs.get(self.pos).cloned().ok_or_else(|| NjoyError::InvalidInput("matxsr: read past end of tape".into()))?;
        self.pos += 1;
        Ok(r)
    }
    /// `tosend`: through the SEND record, copying to `out` if given.
    pub(crate) fn tosend(&mut self, mut out: Option<&mut Vec<GendfRecord>>) -> Result<(), NjoyError> {
        loop {
            let r = self.next()?;
            let send = r.kind == GendfKind::Send;
            if let Some(o) = out.as_deref_mut() {
                o.push(r);
            }
            if send {
                return Ok(());
            }
        }
    }
    /// `findg(mat,mf,mt)` (`:1272-1326`): forward search, then once from the
    /// start; positioned on the section's first record.
    pub(crate) fn findg(&mut self, mat: i32, mf: i32, mt: i32) -> Result<(), NjoyError> {
        let hit = |r: &GendfRecord| r.mat == mat && r.mf == mf && r.mt == mt;
        if let Some(i) = self.recs[self.pos.min(self.recs.len())..].iter().position(hit) {
            self.pos += i;
            return Ok(());
        }
        if let Some(i) = self.recs.iter().position(hit) {
            self.pos = i;
            return Ok(());
        }
        Err(NjoyError::InvalidInput(format!("findg: mat={mat:4} mf={mf:2} mt={mt:3} not on tape")))
    }
}

/// MATXSR's state: `matxsm`'s globals that the routines share.
#[derive(Debug, Clone)]
pub(crate) struct Matxs {
    pub c: MatxsrCards,
    pub npart: usize,
    pub ntype: usize,
    pub nmat: usize,
    pub maxw: i32,
    pub nsubmx: usize,
    pub maxord: i32,
    pub next: i32,
    pub length: i32,
    /// `ia(ifild ...)` per material: `isubm`, `loca`.
    pub nsubm: Vec<i32>,
    pub loca: Vec<i32>,
    /// The group-structure records, all particles in order (`a(igrup ...)`).
    pub groups: Vec<f32>,
    pub ning: usize,
    pub noutg: usize,
    pub iz: usize,
    pub nritev: i32,
    pub nritem: i32,
    pub scr2: Vec<Ctl2>,
    pub scr3: Vec<Vec<f32>>,
    pub scr6: Vec<MatCtl>,
    /// `hvps` (1-based).
    pub hvps: Vec<String>,
    /// `gendf` tapes by unit.
    pub tapes: Vec<(i32, GendfTape)>,
}

fn tape_of(tapes: &[(i32, GendfTape)], lun: i32) -> Result<Cursor, NjoyError> {
    let t = tapes.iter().find(|(u, _)| *u == lun).ok_or_else(|| NjoyError::InvalidInput(format!("matxsr: unit {lun} holds no gendf tape")))?;
    Ok(Cursor { recs: t.1.records.clone(), pos: 0 })
}

/// `x as real(k4)`.
fn k4(x: f64) -> f32 {
    x as f32
}

impl Matxs {
    /// `mtxdat` (`:908-1270`).
    pub(crate) fn mtxdat(&mut self) -> Result<(), NjoyError> {
        const EPS: f64 = 1.0e-6;
        let u = self.c.units;
        let (ngen1, ngen2) = (u[0], u[1]);
        let ngen = [u[3], u[4], u[5], u[6], u[7], u[8]]; // ngen3..ngen8
        let mut ngpb = 0;
        for i in 0..self.npart {
            ngpb += self.c.ngrp[i] + 1;
        }
        let igrup = self.next;
        self.next = igrup + ((ngpb - 1) / 2 + 1) * 2;
        // The group structures (`:978-1015`).
        let mut nin = ngen1;
        for &g in &ngen {
            if nin == 0 {
                nin = g;
            }
        }
        if nin == 0 {
            nin = ngen2;
        }
        if nin == 0 {
            return Err(NjoyError::InvalidInput("mtxdat: input error (nin=0)".into()));
        }
        let mut t = tape_of(&self.tapes, nin)?;
        let (head, list) = loop {
            let r = t.next()?;
            if r.kind == GendfKind::Cont && r.mf != 0 && r.mt != 0 {
                break (r.clone(), t.next()?);
            }
        };
        let nz = head.n[1] as usize;
        let ntw = head.n[3] as usize;
        let b = words(&list);
        let (ng1, ng2) = (list.n[0] as usize, list.n[1] as usize);
        let iqd = 1 + 6 + ntw + nz;
        let iqg = iqd + ng1 + 1;
        let (nqd, nqg) = (ng1 + 1, ng2 + 1);
        for ip in 0..self.npart {
            if ip == 0 || self.c.hprt[ip] != "g       " {
                for i in (1..=nqd).rev() {
                    self.groups.push(k4(b.get(iqd - 1 + i).copied().unwrap_or(0.0)));
                }
            } else {
                for i in (1..=nqg).rev() {
                    self.groups.push(k4(b.get(iqg - 1 + i).copied().unwrap_or(0.0)));
                }
            }
        }
        // Material loop (`:1018-1266`).
        let nwmc = 2 * MULT + 6 * self.nsubmx as i32;
        let nwmc = ((nwmc - 1) / 2 + 1) * 2;
        let imatc = self.next;
        self.next = imatc + nwmc;
        let mut loca = 0;
        self.length = 0;
        let mut awr = 0.0f64;
        for im in 0..self.nmat {
            let (hmatn, matno, matgg) = self.c.mats[im].clone();
            let mut subs = Vec::new();
            let mut locs = 0;
            for i in 0..self.ntype {
                let htyp = self.c.htype[i].clone();
                let mut imat = matno;
                if htyp == "gscat   " {
                    imat = matgg;
                    if hmatn.starts_with("fm") {
                        imat = 9920;
                    }
                }
                let (ip1, ip2) = (self.c.jinp[i] as usize, self.c.joutp[i] as usize);
                self.ning = self.c.ngrp[ip1 - 1] as usize;
                self.noutg = self.c.ngrp[ip2 - 1] as usize;
                let hp1 = self.c.hprt[ip1 - 1].clone();
                let hp2 = self.c.hprt[ip2 - 1].clone();
                let mut nin = 0;
                if hp1 == "n       " {
                    nin = ngen1;
                }
                if hp1 == "g       " {
                    nin = u[8];
                }
                if hp1 == "g       " && htyp == "gscat   " {
                    nin = ngen2;
                }
                if hp1 == "p       " {
                    nin = u[3];
                }
                if hp1 == "d       " {
                    nin = u[4];
                }
                if hp1 == "t       " {
                    nin = u[5];
                }
                if hp1 == "h       " {
                    nin = u[6];
                }
                if hp1 == "a       " {
                    nin = u[7];
                }
                let mut mfv = 3;
                if ip1 != ip2 {
                    mfv = 0;
                }
                if hp2 == "g       " && htyp != "gg      " {
                    mfv = 13;
                }
                if hp1 == "g       " && htyp == "gscat   " {
                    mfv = 23;
                }
                let mut mfm = 0;
                if mfv != 0 {
                    mfm = mfv + 3;
                }
                match hp2.as_str() {
                    "n       " => mfm = 6,
                    "p       " => mfm = 21,
                    "d       " => mfm = 22,
                    "t       " => mfm = 23,
                    "h       " => mfm = 24,
                    "a       " => mfm = 25,
                    _ => {}
                }
                if hp2 == "g       " && htyp == "gg      " {
                    mfm = 16;
                }
                let hp = hp1.chars().next().unwrap_or(' ');
                let mut tin = tape_of(&self.tapes, nin)?;
                tin.findg(imat, 1, 451)?;
                // Head record of the material (`:1067-1083`).
                let h = tin.next()?;
                if i == 0 {
                    awr = h.c[1];
                }
                let nz = h.n[1] as usize;
                let l = tin.next()?;
                let mut temp = l.c[0];
                let ntw_m = h.n[3] as usize;
                let bl = words(&l);
                let nsigz = nz;
                let sigz: Vec<f64> = (1..=nsigz).map(|k| bl.get(k + 6 + ntw_m).copied().unwrap_or(0.0)).collect();
                let mut isigz = 1usize;
                let mut itemp = 1;
                let mut reference: Vec<GendfRecord> = Vec::new();
                loop {
                    // label 320: copy this material (temperature) to scratch.
                    let mut scr: Vec<GendfRecord> = vec![GendfRecord::marker(GendfKind::Mend, 0, 0)];
                    let keep_ref = itemp == 1 && isigz == 1;
                    let mut noned = vec![0i32; nsigz + 1];
                    let mut ntwod = vec![0i32; nsigz + 1];
                    // `tofend` past MF=1 is implicit: the loop skips `mfh = 0`.
                    loop {
                        let r = tin.next()?;
                        if r.mat <= 0 {
                            break;
                        }
                        if r.kind != GendfKind::Cont || r.mf == 0 || r.mt == 0 {
                            continue;
                        }
                        let (mfh, mth) = (r.mf, r.mt);
                        let (nl, nz, ng) = (r.n[0], r.n[1], r.n[3]);
                        let first = tin.next()?;
                        let (ng2, ig) = (first.n[0], first.n[3]);
                        let bb = words(&first);
                        let mut skip = false;
                        if ng2 == 2 && ig == ng {
                            let loc = 6 + (nl * nz) as usize + 1;
                            if bb.get(loc).copied().unwrap_or(0.0).abs() < EPS {
                                skip = true;
                            }
                        }
                        let ntherm = htyp == "ntherm  ";
                        if !skip {
                            if ntherm && !(221..=250).contains(&mth) {
                                skip = true;
                            } else if !ntherm && (221..=250).contains(&mth) {
                                skip = true;
                            } else if ntherm && mfh >= 90 {
                                skip = true;
                            } else if !(mfv == 3 && mfh > 90) && mfh != mfv && mfh != mfv + 2 && mfh != mfm && mfh != mfm + 1 {
                                skip = true;
                            }
                        }
                        if skip {
                            tin.tosend(None)?;
                            continue;
                        }
                        for mz in 1..=nsigz {
                            if (nz as usize) < mz {
                                continue;
                            }
                            if mfh > 90 {
                                noned[mz] += 1;
                            } else if mfh != mfv {
                                if mfh == mfv + 2 {
                                    if mth == 455 {
                                        noned[mz] += 1;
                                    }
                                } else {
                                    if mfh == mfm {
                                        ntwod[mz] += 1;
                                    }
                                    if mfh == 17 && mfm == 16 {
                                        ntwod[mz] += 1;
                                    }
                                }
                            } else if mth == 1 {
                                noned[mz] += 2 * nl;
                            } else if mth == 501 {
                                noned[mz] += 2;
                            } else if mfh == 2 && mfm >= 21 {
                                noned[mz] += 3;
                            } else {
                                noned[mz] += 1;
                            }
                        }
                        let mut sec = vec![r.clone(), first.clone()];
                        tin.tosend(Some(&mut sec))?;
                        if keep_ref {
                            reference.extend(sec.iter().cloned());
                        }
                        scr.extend(sec);
                    }
                    scr.push(GendfRecord::marker(GendfKind::Mend, 0, 0));
                    scr.push(GendfRecord::marker(GendfKind::Tend, -1, 0));
                    let mut refr = vec![GendfRecord::marker(GendfKind::Mend, 0, 0)];
                    if keep_ref {
                        refr.extend(reference.iter().cloned());
                        refr.push(GendfRecord::marker(GendfKind::Mend, 0, 0));
                        refr.push(GendfRecord::marker(GendfKind::Tend, -1, 0));
                        reference = refr;
                    }
                    // The sigma-zero loop (`:1189-1228`).
                    while isigz <= nsigz {
                        let use_ref = !(isigz == 1 && itemp == 1);
                        self.iz = isigz;
                        let n1d = noned[isigz];
                        self.nritev = 0;
                        let n2d = ntwod[isigz];
                        self.nritem = 0;
                        if n1d != 0 || n2d != 0 {
                            let iref = if use_ref { Some(reference.clone()) } else { None };
                            if n1d != 0 {
                                self.vector(mfv, hp, n1d as usize, &scr, iref.as_deref())?;
                            }
                            if n2d != 0 {
                                crate::matxsr::matrix::matrix(self, mfm, hp, n2d as usize, &scr, iref.as_deref())?;
                            }
                            if subs.len() >= self.nsubmx {
                                return Err(NjoyError::InvalidInput("mtxdat: too many submaterials".into()));
                            }
                            subs.push((k4(temp), k4(sigz[isigz - 1]), (i + 1) as i32, n1d, n2d, locs));
                            locs += self.nritev + self.nritem;
                        }
                        isigz += 1;
                    }
                    // label 300: the next temperature of this material, if any.
                    let r = match tin.next() {
                        Ok(r) => r,
                        Err(_) => break,
                    };
                    if r.mat != imat {
                        break;
                    }
                    itemp += 1;
                    isigz = 1;
                    let l = tin.next()?;
                    temp = l.c[0];
                }
            }
            self.scr6.push(MatCtl { name: hmatn.clone(), awr: k4(awr), subs: subs.clone() });
            self.nsubm.push(subs.len() as i32);
            self.loca.push(loca);
            loca += 1 + locs;
            self.length += 1 + locs;
        }
        self.next = imatc;
        Ok(())
    }

    /// `vector` (`:1538-1873`).
    fn vector(&mut self, mfd: i32, hp: char, n1d_in: usize, scr: &[GendfRecord], iref: Option<&[GendfRecord]>) -> Result<(), NjoyError> {
        const STOL: f64 = 1.0e-3;
        const SMALL: f64 = 1.0e-30;
        let mut n1d = n1d_in;
        let (mut k004, mut k016, mut k103, mut k104, mut k105, mut k106, mut k107) = (0, 0, 0, 0, 0, 0, 0);
        let lz = 6usize;
        let mut n1i = 0usize;
        let ning = self.ning;
        let noutg = self.noutg;
        let iz = self.iz;
        let ivcon = self.next;
        self.next = ivcon + ((((3 + MULT) as usize * n1d) as i32 - 1) / 2 + 1) * 2;
        let ivdat_len = ning * n1d;
        self.next += ivdat_len as i32;
        // `a(ivdat ...)`, 0-based here: `a(ivdat+k)` is `v[k]`.
        let mut v = vec![0.0f32; ivdat_len + ning + 2];
        if self.hvps.len() < 3002 {
            self.hvps = vec![String::new(); 3002];
        }
        let mut sc = Cursor { recs: scr.to_vec(), pos: 0 };
        let mut rf = iref.map(|r| Cursor { recs: r.to_vec(), pos: 0 });
        sc.next()?;
        if let Some(r) = rf.as_mut() {
            r.next()?;
        }
        let mut ig = ning as i32;
        let (mut mfh, mut mth) = (0, 0);
        let (mut nl, mut nz) = (0usize, 0usize);
        let mut igr = 0i32;
        let mut igw = 0;
        let mut b: Vec<f64> = Vec::new();
        let mut bref: Vec<f64> = Vec::new();
        let mut hl = [0i32; 2];
        loop {
            // label 115.
            if ig >= ning as i32 {
                let r = sc.next()?;
                if r.mat <= 0 {
                    break;
                }
                mfh = r.mf;
                mth = r.mt;
                if r.kind != GendfKind::Cont || mfh == 0 || mth == 0 {
                    continue;
                }
                if mfh == 12 {
                    mfh = 13;
                }
                let mut skip = mfh <= 1;
                let mut izam = 0;
                if !skip {
                    if mfh > 90 {
                        izam = r.c[1].round() as i32;
                    }
                    nl = r.n[0] as usize;
                    nz = r.n[1] as usize;
                    if nz < iz {
                        skip = true;
                    }
                }
                if !skip && n1i != 0 && (mth == 1 || mth == 501 || (mth == 2 && hp != 'n')) && mfh == mfd {
                    // `:1623-1640` write `hvps(1:2)` whatever `n1i` is: after an
                    // earlier vector reaction upstream overwrites its name and data
                    // and later prints a name it never set (zero bytes, deck X2).
                    return Err(NjoyError::NotPorted(
                        "MATXSR vector: MT=1/501 (or charged-particle MT=2) after another vector reaction (upstream reads an unset hvps)",
                    ));
                }
                if !skip {
                    if mfh == mfd + 2 {
                        n1i += 1;
                        self.hvps[n1i] = "chid    ".into();
                    } else if mfh != mfd && mfh <= 90 {
                        skip = true;
                    } else if mth == 1 {
                        let ltot = r.n[0] as usize;
                        for i in 1..=ltot {
                            self.hvps[i] = format!("{:<8.8}", format!("{hp}wt{}", crate::viewr::fmt::i((i - 1) as i64, 1)));
                            self.hvps[i + ltot] = format!("{:<8.8}", format!("{hp}tot{}", crate::viewr::fmt::i((i - 1) as i64, 1)));
                        }
                        n1i += 2 * ltot;
                    } else if mth == 501 {
                        self.hvps[1] = "gwt0    ".into();
                        self.hvps[2] = "gtot0   ".into();
                        n1i += 2;
                    } else {
                        if mth == 2 && hp != 'n' {
                            self.hvps[1] = format!("{:<8.8}", format!("{hp}wt0"));
                            self.hvps[2] = format!("{:<8.8}", format!("{hp}tot0"));
                            n1i += 2;
                        }
                        n1i += 1;
                        self.hvps[n1i] = hname(hp, mth, r.n[2], izam);
                    }
                }
                if skip {
                    sc.tosend(None)?;
                    continue;
                }
                hl = [r.n[0], r.n[1]];
                if let Some(rc) = rf.as_mut() {
                    rc.findg(r.mat, r.mf, r.mt)?;
                    rc.next()?;
                }
                ig = 0;
                igr = 0;
                igw = 0;
            }
            // label 155.
            if rf.is_none() || ig <= igr {
                let l = sc.next()?;
                b = words(&l);
                hl = [l.n[0], l.n[1]];
                ig = l.n[3];
            }
            if let Some(rc) = rf.as_mut() {
                if igr <= ig {
                    let l = rc.next()?;
                    bref = words(&l);
                    hl = [l.n[0], l.n[1]];
                    igr = l.n[3];
                }
            }
            let at = |x: &[f64], k: usize| x.get(k).copied().unwrap_or(0.0);
            if mfh == mfd + 2 {
                // label 200: delayed fission spectrum.
                let (ng2, ig2lo) = (hl[0] as i64, hl[1] as i64);
                for i in 2..=ng2 {
                    let jg2 = noutg as i64 + 1 - ig2lo - i + 2;
                    let lout = (n1i as i64 - 1) * ning as i64 + jg2 - 1;
                    let lin = lz + nl * (i as usize - 1);
                    for j in 1..=nl {
                        let k = lout as usize;
                        v[k] = k4(at(&b, j + lin) + f64::from(v[k]));
                    }
                }
                continue;
            }
            // label 300.
            if mth == 1 || mth == 501 {
                // label 320.
                for il in 1..=nl {
                    let lout = il * ning - ig as usize;
                    let lin = lz + nl * (iz - 1) + il;
                    v[lout] = k4(at(&b, lin));
                    if rf.is_none() || ig <= igr {
                        let lout = (nl + il) * ning - ig as usize;
                        let lin = lz + nz * nl + nl * (iz - 1) + il;
                        v[lout] = k4(at(&b, lin));
                    }
                    if rf.is_some() && igr <= ig {
                        let lout = (nl + il) * ning - igr as usize;
                        let lin = lz + nz * nl + il;
                        v[lout] = k4(f64::from(v[lout]) - at(&bref, lin));
                        if f64::from(v[lout]).abs() >= STOL * at(&bref, lin) {
                            igw = 1;
                        }
                        if f64::from(v[lout]).abs() < STOL * at(&bref, lin) && igw > 0 {
                            v[lout] = 0.0;
                        }
                    }
                }
                continue;
            }
            if mth == 2 && hp != 'n' {
                // label 350: charged-particle elastic and flux.
                let mut lout = ning - ig as usize;
                let mut lin = lz + nl * (iz - 1) + 1;
                v[lout] = k4(at(&b, lin));
                lout += ning;
                lin += nl * nz;
                v[lout] = k4(at(&b, lin));
                lout += ning;
                v[lout] = k4(at(&b, lin));
                continue;
            }
            let mut to310 = rf.is_some() && ig > igr;
            if !to310 {
                let lout = n1i * ning - ig as usize;
                let lin = lz + nl * (nz + iz - 1) + 1;
                v[lout] = k4(at(&b, lin));
                to310 = hp == 'n' || hp == 'g' || mth == 500 || (201..=207).contains(&mth);
                if !to310 {
                    if mth == 4 {
                        k004 = 1;
                    }
                    to310 = k004 == 1 && (50..=91).contains(&mth);
                }
                if !to310 {
                    if mth == 103 {
                        k103 = 1;
                    }
                    to310 = k103 == 1 && (600..=649).contains(&mth);
                }
                if !to310 {
                    if mth == 104 {
                        k104 = 1;
                    }
                    to310 = k104 == 1 && (650..=699).contains(&mth);
                }
                if !to310 {
                    if mth == 105 {
                        k105 = 1;
                    }
                    to310 = k105 == 1 && (700..=749).contains(&mth);
                }
                if !to310 {
                    if mth == 106 {
                        k106 = 1;
                    }
                    to310 = k106 == 1 && (750..=799).contains(&mth);
                }
                if !to310 {
                    if mth == 107 {
                        k107 = 1;
                    }
                    to310 = k107 == 1 && (800..=849).contains(&mth);
                }
                if !to310 {
                    if mth == 16 {
                        k016 = 1;
                    }
                    to310 = k016 == 1 && (875..=891).contains(&mth);
                }
                if !to310 {
                    let lout = 2 * ning - ig as usize;
                    v[lout] = k4(f64::from(v[lout]) + at(&b, lin));
                }
            }
            // label 310.
            if rf.is_none() || igr > ig {
                continue;
            }
            let lout = n1i * ning - igr as usize;
            let lin = lz + nl * nz + 1;
            v[lout] = k4(f64::from(v[lout]) - at(&bref, lin));
            if f64::from(v[lout]).abs() < STOL * at(&bref, lin) {
                v[lout] = 0.0;
            }
        }
        // label 410: add up separate contributions to production reactions.
        'merge: loop {
            for i in 1..=n1d {
                let n1i = n1d - i + 1;
                for n1j in 1..n1i {
                    if self.hvps[n1j] == self.hvps[n1i] {
                        for k in 1..=ning {
                            let lout = n1j * ning - k;
                            let lin = n1i * ning - k;
                            v[lout] += v[lin];
                        }
                        if n1i != n1d {
                            let nmove = (n1d - n1i) * ning;
                            for k in 1..=nmove {
                                v[(n1i - 1) * ning + k - 1] = v[n1i * ning + k - 1];
                            }
                            for k in 1..=(n1d - n1i) {
                                self.hvps[n1i + k - 1] = self.hvps[n1i + k].clone();
                            }
                        }
                        n1d -= 1;
                        continue 'merge;
                    }
                }
            }
            break;
        }
        // label 480: crunch and write.
        let mut ndex = 0usize;
        let mut mdex = 0usize;
        let mut nfgs = Vec::with_capacity(n1d);
        let mut nlgs = Vec::with_capacity(n1d);
        let mut names = Vec::with_capacity(n1d);
        for i in 1..=n1d {
            let mut nfg = ning + 1;
            let mut nlg = 0usize;
            for k in 1..=ning {
                if f64::from(v[ndex + k - 1]).abs() >= SMALL {
                    nfg = nfg.min(k);
                    nlg = nlg.max(k);
                }
            }
            if nlg == 0 {
                nfg = 1;
                nlg = 1;
            }
            for k in 1..=ning {
                if nfg <= k && nlg >= k {
                    v[mdex] = v[ndex];
                    mdex += 1;
                }
                ndex += 1;
            }
            names.push(self.hvps[i].clone());
            nfgs.push(nfg as i32);
            nlgs.push(nlg as i32);
        }
        let mut nwds = 0i32;
        let mut ivnow = 0usize;
        for i in 0..n1d {
            let nw = nlgs[i] - nfgs[i] + 1;
            if nwds + nw >= self.maxw {
                self.scr3.push(v[ivnow..ivnow + nwds as usize].to_vec());
                ivnow += nwds as usize;
                self.nritev += 1;
                nwds = 0;
            }
            nwds += nw;
        }
        if nwds > 0 {
            self.scr3.push(v[ivnow..ivnow + nwds as usize].to_vec());
            self.nritev += 1;
        }
        let mut ints = nfgs;
        ints.extend(nlgs);
        self.scr2.push(Ctl2::Vector { names, ints });
        self.nritev += 1;
        self.next = ivcon;
        Ok(())
    }
}
