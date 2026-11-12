// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine change`, l.13091-13965: walk the XSS array by its block
//     structure and print each word as an integer (`i20`) or a real
//     (`1pe20.11`);
// and from `src/acecm.f90`:
//   - `advance_to_locator`, `write_integer`, `write_real`, the two list
//     writers and `typen`, l.686-795.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! `change`: how ACER prints the XSS array of a fast ACE file in Type 1
//! format (gh:#1492). Which words are integers is **not stored in the file**;
//! `change` re-derives it from the block layout (`NXS`/`JXS` and the counts
//! inside each block), and prints every word it walks past without
//! visiting as an integer (`advance_to_locator`'s padding is `typen(l,nout,1)`).
//!
//! The walk is reproduced here as [`xss_lines`], returning the lines that
//! `change` writes. Word `l` is 1-based, as upstream's. A locator that points
//! backwards is upstream's `error('advance', ...)` and is an `Err` here; a
//! forward gap is padded with integers (upstream also prints a message).

use super::fortran_fmt::fortran_e20;
use crate::NjoyError;

/// `NXS(1..16)` and `JXS(1..32)` as `acefix` reads them (0-based arrays).
#[derive(Debug, Clone)]
pub struct AceIndex {
    /// `NXS`.
    pub nxs: [i64; 16],
    /// `JXS`.
    pub jxs: [i64; 32],
}

struct Walk<'a> {
    x: &'a [f64],
    kind: Vec<Option<bool>>,
    l: usize,
}

fn err(msg: String) -> NjoyError {
    NjoyError::InvalidInput(format!("change: {msg}"))
}

impl Walk<'_> {
    fn nint(&self) -> Result<i64, NjoyError> {
        self.at(self.l)
    }

    fn at(&self, i: usize) -> Result<i64, NjoyError> {
        if i == 0 || i > self.x.len() {
            return Err(err(format!("xss index {i} outside 1..{}", self.x.len())));
        }
        Ok(self.x[i - 1].round() as i64)
    }

    fn put(&mut self, is_int: bool) -> Result<(), NjoyError> {
        if self.l == 0 || self.l > self.x.len() {
            return Err(err(format!("xss index {} outside 1..{}", self.l, self.x.len())));
        }
        self.kind[self.l - 1] = Some(is_int);
        self.l += 1;
        Ok(())
    }

    /// `advance_to_locator`: pad forward with integers, refuse to go back.
    fn advance(&mut self, locator: i64) -> Result<(), NjoyError> {
        if locator < 1 {
            return Err(err(format!("locator {locator} is not positive")));
        }
        let loc = locator as usize;
        if self.l > loc {
            return Err(err(format!("expected xss index ({loc}) less than current index ({})", self.l)));
        }
        while self.l < loc {
            self.put(true)?;
        }
        Ok(())
    }

    fn int(&mut self) -> Result<(), NjoyError> {
        self.put(true)
    }

    fn real(&mut self) -> Result<(), NjoyError> {
        self.put(false)
    }

    fn int_list(&mut self, n: i64) -> Result<(), NjoyError> {
        for _ in 0..n.max(0) {
            self.put(true)?;
        }
        Ok(())
    }

    fn real_list(&mut self, n: i64) -> Result<(), NjoyError> {
        for _ in 0..n.max(0) {
            self.put(false)?;
        }
        Ok(())
    }

    /// `NR`, the `NBT, INT` pairs when `NR > 0`, then `NE`: the header of an
    /// interpolated table. Returns `NE`.
    fn nr_table_ne(&mut self) -> Result<i64, NjoyError> {
        let nrr = self.nint()?;
        self.int()?;
        if nrr > 0 {
            self.int_list(2 * nrr)?;
        }
        let ne = self.nint()?;
        self.int()?;
        Ok(ne)
    }

    /// `INTT, NP, 3 NP reals` style record at `locator`: `n_per` reals per
    /// point.
    fn int_np_block(&mut self, locator: i64, n_per: i64) -> Result<i64, NjoyError> {
        self.advance(locator)?;
        self.int()?;
        let np = self.nint()?;
        self.int()?;
        self.real_list(n_per * np)?;
        Ok(np)
    }

    /// `NE` reals of energies, then `NE` integer locators `L`; returns the
    /// index of the first locator.
    fn ne_energies_then_locators(&mut self, ne: i64) -> Result<usize, NjoyError> {
        self.real_list(ne)?;
        let at = self.l;
        self.int_list(ne)?;
        Ok(at)
    }
}

/// The text lines `change` writes for `xss` (`len2` words) under `idx`.
///
/// # Errors
/// An `Err` where upstream calls `error()`: a locator that points backwards,
/// an undefined law in the DLW, DLWP or particle-production blocks, or a
/// count that runs outside the array.
pub fn xss_lines(xss: &[f64], idx: &AceIndex) -> Result<Vec<String>, NjoyError> {
    let n = &idx.nxs;
    let j = &idx.jxs;
    let (nes, ntr, nr, ntrp, ntype, ndnf) = (n[2], n[3], n[4], n[5], n[6], n[7]);
    let (esz, nu, mtr, lqr, tyr, lsig, sig, land, and, ldlw, dlw) = (j[0], j[1], j[2], j[3], j[4], j[5], j[6], j[7], j[8], j[9], j[10]);
    let (gpd, mtrp, lsigp, sigp, landp, andp, ldlwp, dlwp, yp) = (j[11], j[12], j[13], j[14], j[15], j[16], j[17], j[18], j[19]);
    let (iurpt, nud, dndat, ldnd, dnd) = (j[22], j[23], j[24], j[25], j[26]);
    let (ptype, ntro, ploct) = (j[29], j[30], j[31]);
    let mut w = Walk { x: xss, kind: vec![None; xss.len()], l: 1 };

    // ESZ
    w.advance(esz)?;
    w.real_list(5 * nes)?;

    // NU
    if nu != 0 {
        w.advance(nu)?;
        let lnu = w.nint()?;
        let m = if lnu > 0 {
            1
        } else {
            w.int()?; // locator for the total nu array
            2
        };
        for _ in 0..m {
            let lnu = w.nint()?;
            w.int()?;
            if lnu != 2 {
                let nc = w.nint()?;
                w.int()?;
                w.real_list(nc)?;
            } else {
                let nrr = w.nint()?;
                w.int()?;
                if nrr != 0 {
                    w.int_list(2 * nrr)?;
                }
                let ne = w.nint()?;
                w.int()?;
                w.real_list(2 * ne)?;
            }
        }
    }

    // Cross sections.
    if ntr != 0 {
        w.advance(mtr)?;
        w.int_list(ntr)?;
        w.advance(lqr)?;
        w.real_list(ntr)?;
        w.advance(tyr)?;
        w.int_list(ntr)?;
        w.advance(lsig)?;
        let mut rloc = w.l;
        w.int_list(ntr)?;
        w.advance(sig)?;
        for _ in 0..ntr {
            let off = w.at(rloc)?;
            w.advance(sig + off - 1)?;
            w.int()?;
            let ne = w.nint()?;
            w.int()?;
            w.real_list(ne)?;
            rloc += 1;
        }
    }

    // LAND and AND.
    w.advance(land)?;
    let nland = nr + 1;
    let mut rloc = w.l;
    w.int_list(nland)?;
    w.advance(and)?;
    for _ in 0..nland {
        let nn = w.at(rloc)?;
        if nn > 0 {
            w.advance(and + nn - 1)?;
            let ne = w.nint()?;
            w.int()?;
            let mut iloc = w.ne_energies_then_locators(ne)?;
            for _ in 0..ne {
                let nn = w.at(iloc)?;
                if nn != 0 {
                    w.advance(and + nn.abs() - 1)?;
                    if nn >= 0 {
                        w.real_list(33)?;
                    } else {
                        w.int()?;
                        let np = w.nint()?;
                        w.int()?;
                        w.real_list(3 * np)?;
                    }
                }
                iloc += 1;
            }
        }
        rloc += 1;
    }

    // Distributions.
    if nr != 0 {
        w.advance(ldlw)?;
        let mut rloc = w.l;
        w.int_list(nr)?;
        w.advance(dlw)?;
        for i in 0..nr {
            let ly = w.at((tyr + i) as usize)?.abs();
            if ly > 100 {
                w.advance(dlw + ly - 101)?;
                let nrr = w.nint()?;
                w.int()?;
                if nrr > 0 {
                    w.int_list(2 * nrr)?;
                }
                let ne = w.nint()?;
                w.int()?;
                w.real_list(2 * ne)?;
            }
            let off = w.at(rloc)?;
            w.advance(dlw + off - 1)?;
            let mut lnw = 1;
            while lnw > 0 {
                lnw = w.nint()?;
                w.int()?;
                let law = w.nint()?;
                w.int()?;
                w.int()?; // IDAT
                let ne = w.nr_table_ne()?;
                w.real_list(2 * ne)?;
                dlw_law(&mut w, law, dlw)?;
            }
            rloc += 1;
        }
    }

    // Unresolved-range probability tables.
    if iurpt > 0 {
        w.advance(iurpt)?;
        let nure = w.nint()?;
        w.int()?;
        let nurb = w.nint()?;
        w.int_list(5)?; // M, INT, ILF, IOA, IFF
        w.real_list(nure * (1 + 6 * nurb))?;
    }

    // Delayed neutrons.
    if ndnf != 0 {
        w.advance(nud)?;
        w.int()?; // LNU
        let ne = w.nr_table_ne()?;
        w.real_list(2 * ne)?;
        w.advance(dndat)?;
        for _ in 0..ndnf {
            w.real()?; // DEC
            let ne = w.nr_table_ne()?;
            w.real_list(2 * ne)?;
        }
        w.advance(ldnd)?;
        let mut rloc = w.l;
        w.int_list(ndnf)?;
        w.advance(dnd)?;
        for _ in 0..ndnf {
            let off = w.at(rloc)?;
            w.advance(dnd + off - 1)?;
            w.int_list(3)?; // LNW, LAW, IDAT
            let ne = w.nr_table_ne()?;
            w.real_list(2 * ne)?;
            // law 4
            let ne = w.nr_table_ne()?;
            let mut iloc = w.ne_energies_then_locators(ne)?;
            for _ in 0..ne {
                let o = w.at(iloc)?;
                w.int_np_block(dnd + o - 1, 3)?;
                iloc += 1;
            }
            rloc += 1;
        }
    }

    // GPD.
    if gpd != 0 {
        w.advance(gpd)?;
        w.real_list(nes)?;
        if mtrp > gpd + nes {
            w.real_list(20 * 30)?;
        }
    }

    // Detailed photon production.
    if ntrp != 0 {
        photon_blocks(&mut w, (ntrp, mtrp, lsigp, sigp, landp, andp, ldlwp, dlwp, yp))?;
    }

    // Particle production.
    if ntype > 0 {
        particle_blocks(&mut w, ntype, ptype, ntro, ploct)?;
    }

    // Print: `typen` writes four words a line, a partial last line at the end.
    let mut lines = Vec::new();
    let mut cur = String::new();
    let end = w.l - 1;
    for p in 0..end {
        let s = match w.kind[p] {
            Some(true) => format!("{:20}", w.x[p].round() as i64),
            _ => fortran_e20(w.x[p]),
        };
        cur.push_str(&s);
        if p % 4 == 3 {
            lines.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    Ok(lines)
}

/// One DLW law body after its common header (`acefc.f90:13265-13450`).
fn dlw_law(w: &mut Walk, law: i64, dlw: i64) -> Result<(), NjoyError> {
    match law {
        1 => {
            let ne = w.nr_table_ne()?;
            w.real_list(ne)?;
            let net = w.nint()?;
            w.int()?;
            w.real_list(ne * net)?;
        }
        3 => w.real_list(2)?,
        4 => {
            let ne = w.nr_table_ne()?;
            let mut iloc = w.ne_energies_then_locators(ne)?;
            for _ in 0..ne {
                let o = w.at(iloc)?;
                w.int_np_block(dlw + o - 1, 3)?;
                iloc += 1;
            }
        }
        5 => {
            let ne = w.nr_table_ne()?;
            w.real_list(2 * ne)?;
            let net = w.nint()?;
            w.int()?;
            w.real_list(net)?;
        }
        7 | 9 => {
            let ne = w.nr_table_ne()?;
            w.real_list(2 * ne)?;
            w.real()?;
        }
        11 => {
            let ne = w.nr_table_ne()?;
            w.real_list(2 * ne)?;
            let ne = w.nr_table_ne()?;
            w.real_list(2 * ne)?;
            w.real()?;
        }
        44 => {
            let ne = w.nr_table_ne()?;
            let mut iloc = w.ne_energies_then_locators(ne)?;
            for _ in 0..ne {
                let o = w.at(iloc)?;
                w.int_np_block(dlw + o - 1, 5)?;
                iloc += 1;
            }
        }
        61 => law61(w, dlw)?,
        66 => law66(w)?,
        67 => law67(w, dlw)?,
        _ => return Err(err(format!("Undefined law for dlw block: {law}"))),
    }
    Ok(())
}

fn law61(w: &mut Walk, base: i64) -> Result<(), NjoyError> {
    let ne = w.nr_table_ne()?;
    let mut iloc = w.ne_energies_then_locators(ne)?;
    for _ in 0..ne {
        let o = w.at(iloc)?;
        w.advance(base + o - 1)?;
        w.int()?;
        let np = w.nint()?;
        w.int()?;
        w.real_list(3 * np)?;
        let mut oloc = w.l;
        w.int_list(np)?;
        for _ in 0..np {
            let o2 = w.at(oloc)?;
            w.int_np_block(base + o2 - 1, 3)?;
            oloc += 1;
        }
        iloc += 1;
    }
    Ok(())
}

fn law66(w: &mut Walk) -> Result<(), NjoyError> {
    w.int()?; // NPSX
    w.real()?; // AP
    w.int()?; // INTT
    let np = w.nint()?;
    w.int()?;
    w.real_list(3 * np)
}

fn law67(w: &mut Walk, base: i64) -> Result<(), NjoyError> {
    let ne = w.nr_table_ne()?;
    let mut iloc = w.ne_energies_then_locators(ne)?;
    for _ in 0..ne {
        let o = w.at(iloc)?;
        w.advance(base + o - 1)?;
        w.int()?; // INTMU
        let nmu = w.nint()?;
        w.int()?;
        w.real_list(nmu)?;
        let mut oloc = w.l;
        w.int_list(nmu)?;
        for _ in 0..nmu {
            let o2 = w.at(oloc)?;
            w.int_np_block(base + o2 - 1, 3)?;
            oloc += 1;
        }
        iloc += 1;
    }
    Ok(())
}

type PhotonIdx = (i64, i64, i64, i64, i64, i64, i64, i64, i64);

/// The detailed photon-production blocks (`acefc.f90:13550-13700`).
fn photon_blocks(w: &mut Walk, p: PhotonIdx) -> Result<(), NjoyError> {
    let (ntrp, mtrp, lsigp, sigp, landp, andp, ldlwp, dlwp, yp) = p;
    w.advance(mtrp)?;
    w.int_list(ntrp)?;
    w.advance(lsigp)?;
    let mut rloc = w.l;
    w.int_list(ntrp)?;
    w.advance(sigp)?;
    for _ in 0..ntrp {
        let off = w.at(rloc)?;
        w.advance(sigp + off - 1)?;
        let mftype = w.nint()?;
        w.int()?;
        if mftype != 12 && mftype != 16 {
            w.int()?; // IE
            let ne = w.nint()?;
            w.int()?;
            w.real_list(ne)?;
        } else {
            w.int()?; // MTMULT
            let ne = w.nr_table_ne()?;
            w.real_list(2 * ne)?;
        }
        rloc += 1;
    }
    w.advance(landp)?;
    let mut rloc = w.l;
    w.int_list(ntrp)?;
    w.advance(andp)?;
    for _ in 0..ntrp {
        let nn = w.at(rloc)?;
        if nn > 0 {
            w.advance(andp + nn - 1)?;
            let ne = w.nint()?;
            w.int()?;
            let mut iloc = w.ne_energies_then_locators(ne)?;
            for _ in 0..ne {
                let nn = w.at(iloc)?;
                if nn > 0 {
                    w.advance(andp + nn - 1)?;
                    w.real_list(33)?;
                }
                iloc += 1;
            }
        }
        rloc += 1;
    }
    w.advance(ldlwp)?;
    let mut rloc = w.l;
    w.int_list(ntrp)?;
    w.advance(dlwp)?;
    for _ in 0..ntrp {
        let off = w.at(rloc)?;
        w.advance(dlwp + off - 1)?;
        let mut lnw = 1;
        while lnw != 0 {
            lnw = w.nint()?;
            w.int()?;
            let law = w.nint()?;
            w.int()?;
            w.int()?; // IDAT
            let ne = w.nr_table_ne()?;
            w.real_list(2 * ne)?;
            match law {
                1 => {
                    let ne = w.nr_table_ne()?;
                    w.real_list(ne)?;
                    let net = w.nint()?;
                    w.int()?;
                    w.real_list(ne * net)?;
                }
                2 => {
                    w.int()?; // LP
                    w.real()?; // EG
                }
                4 | 44 => {
                    let ne = w.nr_table_ne()?;
                    let mut iloc = w.ne_energies_then_locators(ne)?;
                    for _ in 0..ne {
                        let o = w.at(iloc)?;
                        w.int_np_block(dlwp + o - 1, 3)?;
                        iloc += 1;
                    }
                }
                _ => return Err(err(format!("Undefined law for dlwp block: {law}"))),
            }
        }
        rloc += 1;
    }
    w.advance(yp)?;
    let nyp = w.nint()?;
    w.int()?;
    w.int_list(nyp)
}

/// The particle-production blocks (`acefc.f90:13693-13962`).
fn particle_blocks(w: &mut Walk, ntype: i64, ptype: i64, ntro: i64, ploct: i64) -> Result<(), NjoyError> {
    w.advance(ptype)?;
    w.int_list(ntype)?;
    w.advance(ntro)?;
    w.int_list(ntype)?;
    w.advance(ploct)?;
    let mut ploc = w.l;
    w.int_list(10 * ntype)?;
    for i in 0..ntype {
        let g = |k: usize| w.at(ploc + k);
        let (hpd, mtrh, tyrh, lsigh, sigh) = (g(0)?, g(1)?, g(2)?, g(3)?, g(4)?);
        let (landh, andh, ldlwh, dlwh, yh) = (g(5)?, g(6)?, g(7)?, g(8)?, g(9)?);
        w.advance(hpd)?;
        w.int()?; // IE
        let ne = w.nint()?;
        w.int()?;
        if ne != 0 {
            w.real_list(2 * ne)?;
            let ntrh = w.at((ntro + i) as usize)?;
            w.advance(mtrh)?;
            w.int_list(ntrh)?;
            w.advance(tyrh)?;
            w.int_list(ntrh)?;
            w.advance(lsigh)?;
            let mut rloc = w.l;
            w.int_list(ntrh)?;
            w.advance(sigh)?;
            for _ in 0..ntrh {
                let off = w.at(rloc)?;
                w.advance(sigh + off - 1)?;
                w.int_list(2)?; // MFTYPE, MTMULT
                let ne = w.nr_table_ne()?;
                w.real_list(2 * ne)?;
                rloc += 1;
            }
            w.advance(landh)?;
            let mut rloc = w.l;
            w.int_list(ntrh)?;
            w.advance(andh)?;
            for _ in 0..ntrh {
                let nn = w.at(rloc)?;
                if nn > 0 {
                    w.advance(andh + nn - 1)?;
                    let ne = w.nint()?;
                    w.int()?;
                    let mut iloc = w.ne_energies_then_locators(ne)?;
                    for _ in 0..ne {
                        let nn = w.at(iloc)?;
                        w.advance(andh + nn.abs() - 1)?;
                        if nn > 0 {
                            w.real_list(33)?;
                        } else if nn < 0 {
                            w.int()?;
                            let np = w.nint()?;
                            w.int()?;
                            w.real_list(3 * np)?;
                        }
                        iloc += 1;
                    }
                }
                rloc += 1;
            }
            w.advance(ldlwh)?;
            let mut rloc = w.l;
            w.int_list(ntrh)?;
            w.advance(dlwh)?;
            for _ in 0..ntrh {
                let nn = w.at(rloc)?;
                if nn > 0 {
                    w.advance(dlwh + nn - 1)?;
                    w.int()?; // LNW
                    let law = w.nint()?;
                    w.int()?;
                    w.int()?; // IDAT
                    let ne = w.nr_table_ne()?;
                    w.real_list(2 * ne)?;
                    match law {
                        4 => {
                            let ne = w.nr_table_ne()?;
                            let mut iloc = w.ne_energies_then_locators(ne)?;
                            for _ in 0..ne {
                                let o = w.at(iloc)?;
                                w.int_np_block(dlwh + o - 1, 3)?;
                                iloc += 1;
                            }
                        }
                        44 => {
                            let ne = w.nr_table_ne()?;
                            let mut iloc = w.ne_energies_then_locators(ne)?;
                            for _ in 0..ne {
                                let o = w.at(iloc)?;
                                w.int_np_block(dlwh + o - 1, 5)?;
                                iloc += 1;
                            }
                        }
                        33 => w.real_list(2)?,
                        61 => law61(w, dlwh)?,
                        66 => law66(w)?,
                        67 => law67(w, dlwh)?,
                        _ => return Err(err(format!("Undefined law for dlwh block: {law}"))),
                    }
                }
                rloc += 1;
            }
            w.advance(yh)?;
            let nyh = w.nint()?;
            w.int()?;
            w.int_list(nyh)?;
        }
        ploc += 10;
    }
    Ok(())
}
