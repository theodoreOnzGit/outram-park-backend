// Ported from NJOY2016 `src/acepn.f90` (subroutine `phnout`, lines 2460-2828).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The photo-nuclear ACE table's **layout walk** — which word is where, and
//! which are integers.
//!
//! A photo-nuclear table (`class u`) is the most structured of the ACE
//! classes. Its top level looks like a continuous-energy table, but each
//! **emitted particle** — neutron, proton, alpha, photon, … — gets a complete
//! production sub-table of its own, addressed through a `NEIXS × NTYPE`
//! locator array:
//!
//! | NXS | meaning | JXS | meaning |
//! |---|---|---|---|
//! | 1 | `LXS` length | 1 | `ESZ` energies, then `TOT` |
//! | 2 | `ZA` | 2 | `TOT` |
//! | 3 | `NES` | 3 | `NON` (aliases `TOT` when there is no elastic) |
//! | 4 | `NTR` | 4 | `ELS` (0 when absent) |
//! | 5 | `NTYPE` emitted particles | 5 | `THN` heating |
//! | 6 | `NPIXS` | 6 | `MTR` |
//! | 7 | `NEIXS` (12) | 7 | `LQR` |
//! | 9 | `IS` | 8 | `LSIG` |
//! | 10 | `IZ` | 9 | `SIG` |
//! | 11 | `IA` | 10 | `IXSA` the per-particle locator array |
//! | 16 | `TVN` | 11 | `IXS` |
//!
//! and each particle's twelve entries are
//! `IPT, NTRP, PXS, PHN, MTRP, TYRP, LSIGP, SIGP, LANDP, ANDP, LDLWP, DLWP`.
//!
//! ## Why the walk is worth having on its own
//!
//! `phnout` does not write blocks in a fixed pattern — it *navigates*, by
//! locator, with `advance_to_locator` between blocks, and every count it needs
//! is stored in `xss` beside the data. So the whole layout is recoverable from
//! a table alone, and one walk serves three purposes:
//!
//! 1. it tells the Type-1 writer which words are `i20` and which are
//!    `1pE20.11` ([`int_mask`]), which is what makes a read-then-write round
//!    trip byte-exact for this class;
//! 2. it is a **structural validator** — a table whose counts do not add up
//!    walks off its own end, and [`walk`] says so rather than producing a
//!    plausible answer;
//! 3. it is the skeleton the builder fills.
//!
//! Upstream's `advance_to_locator` pads a gap with **integers** and prints a
//! `mess`; a well-formed table has no gaps, and this walk reports one as an
//! error rather than papering over it.

use crate::NjoyError;

/// Named photo-nuclear **NXS** indices (0-based).
pub mod nxs {
    /// NXS(1): `LXS` — the XSS length.
    pub const LXS: usize = 0;
    /// NXS(2): `ZA`.
    pub const ZA: usize = 1;
    /// NXS(3): `NES` — incident-energy grid length.
    pub const NES: usize = 2;
    /// NXS(4): `NTR` — reaction count.
    pub const NTR: usize = 3;
    /// NXS(5): `NTYPE` — number of emitted particle types.
    pub const NTYPE: usize = 4;
    /// NXS(6): `NPIXS`.
    pub const NPIXS: usize = 5;
    /// NXS(7): `NEIXS` — locators per particle (12).
    pub const NEIXS: usize = 6;
    /// NXS(9): `IS` — isomeric state.
    pub const IS: usize = 8;
    /// NXS(10): `IZ`.
    pub const IZ: usize = 9;
    /// NXS(11): `IA`.
    pub const IA: usize = 10;
    /// NXS(16): `TVN`.
    pub const TVN: usize = 15;
}

/// Named photo-nuclear **JXS** indices (0-based).
pub mod jxs {
    /// JXS(1): `ESZ` — the energy grid, immediately followed by `TOT`.
    pub const ESZ: usize = 0;
    /// JXS(2): `TOT`.
    pub const TOT: usize = 1;
    /// JXS(3): `NON` — non-elastic; aliases `TOT` when there is no elastic.
    pub const NON: usize = 2;
    /// JXS(4): `ELS` — elastic, `0` when absent.
    pub const ELS: usize = 3;
    /// JXS(5): `THN` — heating numbers.
    pub const THN: usize = 4;
    /// JXS(6): `MTR`.
    pub const MTR: usize = 5;
    /// JXS(7): `LQR`.
    pub const LQR: usize = 6;
    /// JXS(8): `LSIG`.
    pub const LSIG: usize = 7;
    /// JXS(9): `SIG`.
    pub const SIG: usize = 8;
    /// JXS(10): `IXSA` — the `NEIXS × NTYPE` per-particle locator array.
    pub const IXSA: usize = 9;
    /// JXS(11): `IXS`.
    pub const IXS: usize = 10;
}

/// The twelve per-particle locators of one `IXSA` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticleLocators {
    /// `IPT` — the MCNP particle type.
    pub ipt: i32,
    /// `NTRP` — this particle's production reaction count.
    pub ntrp: usize,
    /// `PXS` — production cross section.
    pub pxs: usize,
    /// `PHN` — heating numbers.
    pub phn: usize,
    /// `MTRP` — reaction numbers.
    pub mtrp: usize,
    /// `TYRP` — frame/multiplicity flags.
    pub tyrp: usize,
    /// `LSIGP` — locators into `SIGP`.
    pub lsigp: usize,
    /// `SIGP` — per-reaction production data.
    pub sigp: usize,
    /// `LANDP` — locators into `ANDP`.
    pub landp: usize,
    /// `ANDP` — angular distributions.
    pub andp: usize,
    /// `LDLWP` — locators into `DLWP`.
    pub ldlwp: usize,
    /// `DLWP` — energy distributions.
    pub dlwp: usize,
}

/// What [`walk`] found: the integer mask, and the per-particle locators it
/// read on the way.
#[derive(Debug, Clone)]
pub struct Walk {
    /// `true` where the word is written `i20` rather than `1pE20.11`.
    pub is_int: Vec<bool>,
    /// One entry per emitted particle, in `IXSA` order.
    pub particles: Vec<ParticleLocators>,
    /// The highest word index the walk reached, 1-based. A well-formed table
    /// ends exactly at `NXS(1)`.
    pub reached: usize,
}

/// A cursor that marks words as it goes and refuses to run off the end.
struct Cur<'a> {
    xss: &'a [f64],
    mask: Vec<bool>,
    /// 1-based, upstream's `l`.
    l: usize,
    max: usize,
}

impl Cur<'_> {
    fn peek(&self) -> Result<f64, NjoyError> {
        self.xss.get(self.l - 1).copied().ok_or_else(|| {
            NjoyError::EndfParse(format!(
                "photonuclear layout: word {} is past the end of a {}-word table",
                self.l,
                self.xss.len()
            ))
        })
    }
    fn count(&self) -> Result<usize, NjoyError> {
        let v = self.peek()?;
        if !(v.is_finite() && (0.0..=1.0e9).contains(&v)) {
            return Err(NjoyError::EndfParse(format!(
                "photonuclear layout: word {} is {v}, which is not a count",
                self.l
            )));
        }
        Ok(v.round() as usize)
    }
    /// `write_integer` / `write_integer_list`.
    fn ints(&mut self, n: usize) -> Result<(), NjoyError> {
        for _ in 0..n {
            self.peek()?;
            self.mask[self.l - 1] = true;
            self.l += 1;
            self.max = self.max.max(self.l - 1);
        }
        Ok(())
    }
    /// `write_real` / `write_real_list` — the default, so this only advances.
    fn reals(&mut self, n: usize) -> Result<(), NjoyError> {
        for _ in 0..n {
            self.peek()?;
            self.l += 1;
            self.max = self.max.max(self.l - 1);
        }
        Ok(())
    }
    /// `advance_to_locator`: a gap is padded **with integers** and is a
    /// malformed table; going backwards is upstream's hard error.
    fn seek(&mut self, locator: usize) -> Result<(), NjoyError> {
        if locator == 0 {
            return Err(NjoyError::EndfParse(
                "photonuclear layout: a zero locator cannot be sought to".into(),
            ));
        }
        if self.l > locator {
            return Err(NjoyError::EndfParse(format!(
                "photonuclear layout: locator {locator} is behind the cursor at {} \
                 (acecm.f90's advance_to_locator calls this a serious problem)",
                self.l
            )));
        }
        while self.l < locator {
            self.mask[self.l - 1] = true;
            self.l += 1;
        }
        Ok(())
    }
}

/// Walk a photo-nuclear table's layout, exactly as `phnout` writes it.
///
/// # Errors
/// [`NjoyError::EndfParse`] when a count or locator sends the walk outside the
/// table — which means the table is malformed, not that the walk is wrong.
pub fn walk(nxs: &[i32; 16], jxs: &[i32; 32], xss: &[f64]) -> Result<Walk, NjoyError> {
    let nes = nxs[nxs::NES].max(0) as usize;
    let ntr = nxs[nxs::NTR].max(0) as usize;
    let ntype = nxs[nxs::NTYPE].max(0) as usize;
    let neixs = nxs[nxs::NEIXS].max(0) as usize;
    let loc = |k: usize| -> usize { jxs[k].max(0) as usize };

    let mut c = Cur {
        xss,
        mask: vec![false; xss.len()],
        l: 1,
        max: 0,
    };

    // ESZ: the grid and the total, 2*NES reals (`acepn.f90:2506-2508`).
    c.seek(loc(jxs::ESZ))?;
    c.reals(2 * nes)?;
    // ELS and THN, NES reals each when present.
    if jxs[jxs::ELS] != 0 {
        c.seek(loc(jxs::ELS))?;
        c.reals(nes)?;
    }
    if jxs[jxs::THN] != 0 {
        c.seek(loc(jxs::THN))?;
        c.reals(nes)?;
    }
    // MTR (ints), LQR (reals), LSIG (ints).
    c.seek(loc(jxs::MTR))?;
    c.ints(ntr)?;
    c.seek(loc(jxs::LQR))?;
    c.reals(ntr)?;
    c.seek(loc(jxs::LSIG))?;
    let lsig_at = c.l;
    c.ints(ntr)?;
    // SIG: each reaction at `SIG + LSIG(i) - 1`, as IE, NE, then NE reals.
    let sig = loc(jxs::SIG);
    for i in 0..ntr {
        let rel = xss[lsig_at - 1 + i].round() as usize;
        c.seek(sig + rel - 1)?;
        c.ints(1)?; // IE
        let ne = c.count()?;
        c.ints(1)?; // NE
        c.reals(ne)?;
    }

    let mut particles = Vec::with_capacity(ntype);
    if ntype > 0 {
        c.seek(loc(jxs::IXSA))?;
        let ixsa_at = c.l;
        c.ints(neixs * ntype)?;
        for p in 0..ntype {
            let b = ixsa_at - 1 + neixs * p;
            let g = |k: usize| -> usize { xss[b + k].round().max(0.0) as usize };
            let pl = ParticleLocators {
                ipt: xss[b].round() as i32,
                ntrp: g(1),
                pxs: g(2),
                phn: g(3),
                mtrp: g(4),
                tyrp: g(5),
                lsigp: g(6),
                sigp: g(7),
                landp: g(8),
                andp: g(9),
                ldlwp: g(10),
                dlwp: g(11),
            };
            let ntrp = pl.ntrp;

            // PXS and PHN: IE, NE, then NE reals each.
            for start in [pl.pxs, pl.phn] {
                c.seek(start)?;
                c.ints(1)?;
                let ne = c.count()?;
                c.ints(1)?;
                c.reals(ne)?;
            }
            // MTRP and TYRP: NTRP ints each.
            c.seek(pl.mtrp)?;
            c.ints(ntrp)?;
            c.seek(pl.tyrp)?;
            c.ints(ntrp)?;

            // LSIGP / SIGP.
            c.seek(pl.lsigp)?;
            let lsigp_at = c.l;
            c.ints(ntrp)?;
            for i in 0..ntrp {
                let rel = xss[lsigp_at - 1 + i].round() as usize;
                c.seek(pl.sigp + rel - 1)?;
                let mftype = c.count()?;
                c.ints(1)?;
                if mftype == 13 {
                    c.ints(1)?; // IE
                    let ne = c.count()?;
                    c.ints(1)?; // NE
                    c.reals(ne)?;
                } else {
                    c.ints(1)?; // MTMULT
                    let nr = c.count()?;
                    c.ints(1)?; // NR
                    c.ints(2 * nr)?;
                    let ne = c.count()?;
                    c.ints(1)?; // NE
                    c.reals(2 * ne)?;
                }
            }

            // LANDP / ANDP.
            c.seek(pl.landp)?;
            let landp_at = c.l;
            c.ints(ntrp)?;
            for i in 0..ntrp {
                let nn = xss[landp_at - 1 + i].round() as i64;
                if nn <= 0 {
                    continue;
                }
                c.seek(pl.andp + nn as usize - 1)?;
                let ne = c.count()?;
                c.ints(1)?;
                c.reals(ne)?;
                let ie_at = c.l;
                c.ints(ne)?;
                for j in 0..ne {
                    let nn2 = xss[ie_at - 1 + j].round() as i64;
                    if nn2 == 0 {
                        continue;
                    }
                    c.seek(pl.andp + nn2.unsigned_abs() as usize - 1)?;
                    if nn2 > 0 {
                        c.reals(33)?; // 32 equiprobable bins
                    } else {
                        c.ints(1)?; // interpolation flag
                        let np = c.count()?;
                        c.ints(1)?; // NP
                        c.reals(3 * np)?;
                    }
                }
            }

            // LDLWP / DLWP.
            c.seek(pl.ldlwp)?;
            let ldlwp_at = c.l;
            c.ints(ntrp)?;
            for i in 0..ntrp {
                let nn = xss[ldlwp_at - 1 + i].round() as i64;
                if nn <= 0 {
                    continue;
                }
                c.seek(pl.dlwp + nn as usize - 1)?;
                loop {
                    let lnw = c.count()?;
                    c.ints(1)?; // LNW
                    let law = c.count()?;
                    c.ints(1)?; // LAW
                    c.ints(1)?; // IDAT
                    let nr = c.count()?;
                    c.ints(1)?; // NR
                    c.ints(2 * nr)?;
                    let ne = c.count()?;
                    c.ints(1)?; // NE
                    c.reals(2 * ne)?; // E and P
                    law_body(&mut c, law, pl.dlwp)?;
                    if lnw == 0 {
                        break;
                    }
                }
            }
            particles.push(pl);
        }
    }

    let reached = c.max;
    Ok(Walk {
        is_int: c.mask,
        particles,
        reached,
    })
}

/// One `DLWP` law body (`acepn.f90:2680-2776`).
fn law_body(c: &mut Cur<'_>, law: usize, dlwp: usize) -> Result<(), NjoyError> {
    match law {
        // Law 4 (tabulated E'), 44 (Kalbach-Mann) and 61 (tabulated angle)
        // share an outer shape and differ in the per-point payload.
        4 | 44 | 61 => {
            let nr = c.count()?;
            c.ints(1)?;
            c.ints(2 * nr)?;
            let ne = c.count()?;
            c.ints(1)?;
            c.reals(ne)?;
            let ie_at = c.l;
            c.ints(ne)?;
            for j in 0..ne {
                let rel = c.xss[ie_at - 1 + j].round() as usize;
                c.seek(dlwp + rel - 1)?;
                c.ints(1)?; // INTT
                let np = c.count()?;
                c.ints(1)?; // NP
                match law {
                    4 => c.reals(3 * np)?,       // E', pdf, cdf
                    44 => c.reals(5 * np)?,      // E', pdf, cdf, R, A
                    _ => {
                        c.reals(3 * np)?; // E', pdf, cdf
                        let oe_at = c.l;
                        c.ints(np)?;
                        for k in 0..np {
                            let rel = c.xss[oe_at - 1 + k].round() as usize;
                            if rel == 0 {
                                continue;
                            }
                            c.seek(dlwp + rel - 1)?;
                            c.ints(1)?; // JJ
                            let nmu = c.count()?;
                            c.ints(1)?; // NMU
                            c.reals(3 * nmu)?; // mu, pdf, cdf
                        }
                    }
                }
            }
            Ok(())
        }
        // Law 7 / 9: an evaporation-style (E, theta) table plus U.
        7 | 9 => {
            let nr = c.count()?;
            c.ints(1)?;
            c.ints(2 * nr)?;
            let ne = c.count()?;
            c.ints(1)?;
            c.reals(2 * ne)?;
            c.reals(1)?; // U
            Ok(())
        }
        // Law 33: two reals.
        33 => c.reals(2),
        other => Err(NjoyError::EndfParse(format!(
            "photonuclear DLWP: law {other} is not one phnout can write \
             (acepn.f90:2774 calls this a fatal error)"
        ))),
    }
}

/// The integer mask alone, for the Type-1 writer.
///
/// Returns `None` rather than an error when the walk fails, because the
/// writer's fallback is the value heuristic and a malformed table should
/// still produce *something* a human can look at. Callers that want the
/// diagnosis use [`walk`].
pub fn int_mask(nxs: &[i32; 16], jxs: &[i32; 32], xss: &[f64]) -> Option<Vec<bool>> {
    walk(nxs, jxs, xss).ok().map(|w| w.is_int)
}
