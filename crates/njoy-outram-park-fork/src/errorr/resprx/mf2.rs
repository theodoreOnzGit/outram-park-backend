// Ported from NJOY2016 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine rdumrd2`, l.5091-5225 — dummy read of MF=2 recording the L-state
//     count of every energy range (`nlspepi`) and the URR degrees of freedom (`amur`).
//   - `subroutine rskiprp`, l.5369-5409 + `rpxlc12` l.4171-4222 — position on one
//     range's MF=2 records and lay them out in the `b` work array with the
//     shift/penetration factors at each resonance energy appended.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The MF=2 records the MF=32 chain needs: one [`Mf2Range`] per
//! `(isotope, energy range)` in file order, and the URR
//! `(AMUN, AMUF, AMUX)` triples `rdumrd2` collects from an `LRU=2/LRF=2`
//! range (`amur`; upstream never fills them from an `LRU=2/LRF=1` range,
//! so `ggunr1` on such a material would read uninitialised memory — this
//! port refuses that case instead, see [`super::unresolved`]).

use crate::endf::records::{Cont, List, SectionCursor};
use crate::endf::tape::Tape;
use crate::reconr::slbw::WAVE_K;
use crate::samm::mf2::{parse_rml_section, RmlSection};
use crate::NjoyError;

use super::super::math::efacts;

/// One `(isotope, energy range)` of MF=2 as `rdumrd2` walks it.
#[derive(Debug, Clone)]
pub struct Mf2Range {
    /// The range CONT `(EL, EH, LRU, LRF, NRO, NAPS)`.
    pub range: Cont,
    /// `LFW` of the isotope CONT.
    pub lfw: i32,
    /// `nlspepi(indx)` — `NLS` (or `NJS` for `LRF=7`) recorded for the
    /// MF=2/MF=32 consistency check.
    pub nls: i32,
    /// For resolved BW/RM ranges: the `(SPI, AP, 0, 0, NLS, 0)` CONT.
    pub spi_cont: Option<Cont>,
    /// For resolved BW/RM ranges: the `NLS` L-state LISTs in file order.
    pub lists: Vec<List>,
    /// For an `LRF=7` range: the parsed R-matrix-limited section (what
    /// `rdsammy` reads from the `nscr6` copy in `rpxsamm`).
    pub rml: Option<RmlSection>,
}

/// `rdumrd2`'s output: every range plus the URR `amur` triples.
#[derive(Debug, Clone, Default)]
pub struct Mf2Resonances {
    /// `AWR` of the MF=2/MT=151 HEAD (the mass `ppsammy`'s kinematics use).
    pub awr: f64,
    /// `NIS`.
    pub nis: i32,
    /// Ranges in file order (`indx` = position + 1).
    pub ranges: Vec<Mf2Range>,
    /// `amur(1:3, nlru2)` = `(AMUN, AMUF, AMUX)` per `(L, J)` of every
    /// `LRU=2/LRF=2` range, in file order.
    pub amur: Vec<[f64; 3]>,
}

impl Mf2Resonances {
    /// Walk MF=2/MT=151 of `matd` (`rdumrd2`, `errorr.f90:5091-5225`).
    ///
    /// # Errors
    /// [`NjoyError::SectionNotFound`] without MF=2, [`NjoyError::EndfParse`]
    /// for an `(LRU, LRF)` upstream has "no coding" for.
    pub fn read(endf: &Tape, matd: i32) -> Result<Self, NjoyError> {
        let sec = endf
            .section(matd, 2, 151)
            .ok_or(NjoyError::SectionNotFound {
                mat: matd,
                mf: 2,
                mt: 151,
            })?;
        let mut cur = SectionCursor::new(&sec.rows);
        let head = cur.read_cont()?;
        let nis = head.n1;
        let mut out = Mf2Resonances {
            awr: head.c2,
            nis,
            ..Default::default()
        };
        for _ni in 0..nis {
            let iso = cur.read_cont()?;
            let lfw = iso.l2;
            let ner = iso.n1;
            for _ne in 0..ner {
                let range = cur.read_cont()?;
                let (lru, lrf, nro) = (range.l1, range.l2, range.n1);
                if nro > 0 {
                    let _ = cur.read_tab1()?;
                }
                let mut r = Mf2Range {
                    range,
                    lfw,
                    nls: 0,
                    spi_cont: None,
                    lists: Vec::new(),
                    rml: None,
                };
                match (lru, lrf) {
                    // breit-wigner / reich-moore (errorr.f90:5119-5140)
                    (1, 1) | (1, 2) | (1, 3) => {
                        let c = cur.read_cont()?;
                        r.nls = c.n1;
                        for _ in 0..c.n1 {
                            r.lists.push(cur.read_list()?);
                        }
                        r.spi_cont = Some(c);
                    }
                    // reich-moore limited (errorr.f90:5143-5155): upstream skips it
                    // here (nlspepi unset) and rpxsamm re-reads it with rdsammy;
                    // the port parses it once. `parse_rml_section` reads the
                    // (SPI, AP, IFG, KRM, NJS) CONT itself.
                    (1, 7) => {
                        let pos = cur.position();
                        let c = cur.read_cont()?;
                        r.nls = c.n1;
                        let mut sub = SectionCursor::new(&sec.rows[pos..]);
                        let rml = parse_rml_section(&mut sub)?;
                        cur.skip_rows(sub.position() - 1)?;
                        r.spi_cont = Some(c);
                        r.rml = Some(rml);
                    }
                    // unresolved lrf=1, lfw=0 (errorr.f90:5158-5166)
                    (2, 1) if lfw == 0 => {
                        let c = cur.read_cont()?;
                        r.nls = c.n1;
                        for _ in 0..c.n1 {
                            r.lists.push(cur.read_list()?);
                        }
                        r.spi_cont = Some(c);
                    }
                    // unresolved lrf=1, lfw=1 (errorr.f90:5169-5181)
                    (2, 1) => {
                        let l = cur.read_list()?;
                        r.nls = l.head.n2;
                        for _ in 0..l.head.n2 {
                            let c = cur.read_cont()?;
                            for _ in 0..c.n1 {
                                let _ = cur.read_list()?;
                            }
                        }
                    }
                    // unresolved lrf=2 (errorr.f90:5184-5199): amur from each (L, J) LIST
                    (2, 2) => {
                        let c = cur.read_cont()?;
                        r.nls = c.n1;
                        for _ in 0..c.n1 {
                            let lc = cur.read_cont()?;
                            for _ in 0..lc.n1 {
                                let l = cur.read_list()?;
                                // a(10)=AMUN, a(12)=AMUF, a(9)=AMUX with a(1..6) the head
                                out.amur.push([l.data[3], l.data[5], l.data[2]]);
                            }
                        }
                        r.spi_cont = Some(c);
                    }
                    _ => {
                        return Err(NjoyError::EndfParse(format!(
                            "errorr::rdumrd2: lru={lru} lrf={lrf} no coding"
                        )));
                    }
                }
                out.ranges.push(r);
            }
        }
        Ok(out)
    }
}

/// The resolved-range work array `b` (`rskiprp` + `rpxlc12`,
/// `errorr.f90:4171-4222`): range CONT, SPI CONT, then per L-state the
/// LIST head, its `6*NRS` parameters and `3*NRS` appended words
/// `(S_l, P_l, 0)` evaluated at `rho = cwaven*arat*sqrt(|ER|)*ral`.
///
/// `ral`/`apl`/`ra` follow the upstream globals: entering with
/// `ral = ra_computed`, `apl = ap`; for `LRF=3` each L may carry its own
/// `APL` (zero means `AP`); with `NAPS = 1` the radius used for the
/// penetrabilities becomes `apl` (and `ra` becomes `ap`).
pub struct WorkArray {
    /// The flat array (0-based; Fortran `b(i)` is `b[i-1]`).
    pub b: Vec<f64>,
    /// `llmat(il)` — `L` of each L-state block, in file order.
    pub llmat: Vec<i32>,
    /// `ral` after the loop (what the sensitivity loop uses for `rho`).
    pub ral: f64,
    /// `apl` after the loop.
    pub apl: f64,
}

/// Build the work array for one resolved range (see [`WorkArray`]).
///
/// # Errors
/// [`NjoyError::EndfParse`] when the MF=32 `(LRU, LRF)` disagree with
/// MF=2 (upstream: "different type of resonance for lcomp=1").
#[allow(clippy::too_many_arguments)]
pub fn build_work_array(
    range: &Mf2Range,
    lru: i32,
    lrf: i32,
    naps: i32,
    ap: f64,
    arat: f64,
    ral0: f64,
    apl0: f64,
) -> Result<WorkArray, NjoyError> {
    let (lru1, lrf1) = (range.range.l1, range.range.l2);
    if lrf1 == 7 {
        return Err(NjoyError::NotPorted(
            "errorr::rpxlc12: cannot handle RML with isammy=0",
        ));
    }
    if lru != lru1 || lrf != lrf1 {
        return Err(NjoyError::EndfParse(format!(
            "errorr::rpxlc12: lru/lrf(mf=32)={lru}/{lrf} vs. lru/lrf(mf=2)={lru1}/{lrf1}"
        )));
    }
    let spi = range.spi_cont.as_ref().ok_or_else(|| {
        NjoyError::EndfParse("errorr::rpxlc12: MF=2 range carries no SPI record".into())
    })?;
    let mut b: Vec<f64> = Vec::new();
    let push_cont = |b: &mut Vec<f64>, c: &Cont| {
        b.extend_from_slice(&[
            c.c1,
            c.c2,
            c.l1 as f64,
            c.l2 as f64,
            c.n1 as f64,
            c.n2 as f64,
        ]);
    };
    push_cont(&mut b, &range.range);
    push_cont(&mut b, spi);
    let mut ral = ral0;
    let mut apl = apl0;
    let mut llmat = Vec::with_capacity(range.lists.len());
    for l in &range.lists {
        let ind = b.len();
        push_cont(&mut b, &l.head);
        b.extend_from_slice(&l.data);
        if lrf1 == 3 {
            apl = b[ind + 1];
            if apl == 0.0 {
                apl = ap;
            }
        }
        if naps == 1 {
            ral = apl;
            // ra = ap: the global `ra` is re-derived by ggmlbw from b itself.
        }
        let ll = b[ind + 2].round() as i32;
        llmat.push(ll);
        let nrs1 = b[ind + 5].round() as usize;
        for nr in 1..=nrs1 {
            let rho = WAVE_K * arat * b[ind + 6 * nr].abs().sqrt() * ral;
            let (ser, per) = efacts(ll, rho);
            b.extend_from_slice(&[ser, per, 0.0]);
        }
    }
    Ok(WorkArray { b, llmat, ral, apl })
}
