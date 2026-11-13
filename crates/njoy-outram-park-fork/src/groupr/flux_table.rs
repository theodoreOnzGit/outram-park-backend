// Translated from NJOY2016.80 `src/groupr.f90`: `genflx`'s Bondarenko branch
// (`:5380-5395`, `:5629-5671`), `getflx`'s `nsigz > 1` branch (`:6457-6510`),
// `getunr`'s `xtot`/`iovl` handling (`:6896-6994`) and `stounr`'s
// temperature/`lrp` gate (`:6802-6894`), with `util.f90`'s `scana` (`:291-359`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **The self-shielded weighting flux of GROUPR's tape driver** (`nsigz > 1`,
//! gh:#1369 round 4).
//!
//! [`super::unresolved::genflx_bondarenko`] evaluates the Bondarenko formula on a
//! grid the caller supplies. NJOY's `genflx` builds its own grid, and the group
//! constants depend on it, because `getflx` interpolates the flux linearly
//! between the grid's nodes. That grid is [`FluxTable`]:
//!
//! - it starts at MT=1's first energy;
//! - the next node is the smallest of three energies: `getwtf`'s `enext`, MT=1's
//!   next `gety1` break, and `getunr`'s next URR energy;
//! - each node stores `wtf * sigz / (tot_iz + sigz)` for each sigma-zero
//!   (`sigpot = 0` for `iwt > 0`), and for `il > 1` that again times the MT=261
//!   factor.
//!
//! [`FluxCursor`] is `getflx` on that table. [`getunr`] wraps
//! [`super::unresolved::UnresolvedTable::shield`] (the `iovl = 0` path) with
//! `getunr`'s `iovl` test and its `xtot` state.
//!
//! Not ported: the slowing-down branch (`iwt < 0`, `nflmax > 0`) and `iwt = 0`
//! (`getfwt`, a flux read from `ninwt`). Both are refused by name.

use crate::endf::gety1::Gety1;
use crate::endf::interp::{terp1, IntLaw};
use crate::endf::tape::Tape;
use crate::groupr::unresolved::{UnresolvedTable, UrrReaction};
use crate::NjoyError;

/// `getunr` (`groupr.f90:6896-6994`) on the table `stounr` stored: shield
/// `sig(1..nsigz)` in place at energy `e` for reaction `mt`, and return
/// `enext`, or `-1` when `e` is outside the URR range, `mt` has no column, or
/// there is no table.
///
/// `xtot` is the module variable: for MT=1 at an overlap point (`iovl = 1`) it
/// becomes `sig(1) - sinf` (`:6963`, `:6980`); at an overlap point every
/// reaction's background is `sigz + xtot` (`:6966`, `:6984`). `iovl` follows
/// upstream's word tests: at the last point it is the sign of `unr(locl)`
/// (`:6961`), which for `ix > 1` is the last sigma-zero word of the previous
/// reaction column, not the energy.
///
/// # Errors
/// From [`UnresolvedTable::shield`].
pub fn getunr(unr: Option<&UnresolvedTable>, sigz: &[f64], mt: i32, e: f64, sig: &mut [f64], xtot: &mut f64) -> Result<f64, NjoyError> {
    let Some(t) = unr else { return Ok(-1.0) };
    let Some(r) = UrrReaction::from_mt(mt) else { return Ok(-1.0) };
    let Some(ctx) = t.overlap_context(r, e) else { return Ok(-1.0) };
    let pts = t.points();
    let last = &pts[pts.len() - 1];
    // the interval search of `:6944-6956` found no bracket: the last-point exit.
    let at_last = !pts.windows(2).any(|w| {
        let elo = w[0].energy_ev - 1.0e-10 * w[0].energy_ev;
        e > elo && e < w[1].energy_ev
    });
    let iovl = if at_last {
        let ix = r.column();
        if ix == 0 {
            last.overlap
        } else {
            let prev = &last.xs[ix - 1];
            prev[prev.len() - 1] < 0.0
        }
    } else {
        ctx.in_overlap
    };
    let n = sigz.len();
    if r == UrrReaction::Total && iovl {
        *xtot = sig[0] - ctx.sinf;
    }
    let dil: Vec<f64> = sigz.iter().map(|&s| if iovl { s + *xtot } else { s }).collect();
    let out = t.shield(r, e, &sig[..n], &dil)?;
    sig[..n].copy_from_slice(&out.sig);
    Ok(out.next_energy.unwrap_or(-1.0))
}

/// `stounr` (`groupr.f90:6802-6894`) on one temperature's block of the PENDF:
/// `None` when `lrp = -1` or the block has no MF=2/MT=152 (`nunr = 0`), else
/// the table at `temp` (`abs(temz - temp) <= 1`).
///
/// # Errors
/// From [`crate::groupr::urr_pendf::read_urr_table`] ("cannot find temp").
pub fn stounr(block: &Tape, mat: i32, temp: f64) -> Result<Option<UnresolvedTable>, NjoyError> {
    let Some(mf1) = block.section(mat, 1, 451) else { return Ok(None) };
    let lrp = mf1.rows[0][2].round() as i32;
    crate::groupr::urr_pendf::read_urr_from_tape(Some(block), mat, lrp, temp)
}

/// `genflx`'s table (`loada` on `nflx`): one row of `nfv = 1 + nl*nsigz + 1`
/// words per node, `(e, flux(il=1..nl, iz=1), ..., flux(il, iz=nsigz), xtot)`.
#[derive(Debug, Clone)]
pub struct FluxTable {
    /// `nl` of the table: `lord + 1`, at least 2 (`:5374-5375`).
    pub nl: usize,
    /// The rows, in energy order.
    pub rows: Vec<Vec<f64>>,
}

/// `genflx` (`groupr.f90:5309-5684`), Bondarenko branch (`nflmax = 0`, `iwt > 0`):
/// the narrow-resonance flux from MT=1's first energy to the end of MT=1, on
/// `block` (one temperature of the PENDF).
///
/// `egn1` is the lowest group bound (`egn(1)`, the "total not defined" check).
///
/// # Errors
/// [`NjoyError::NotPorted`] for `iwt <= 0` (slowing-down flux, `getfwt`);
/// [`NjoyError::InvalidInput`] when MT=1 is missing or starts above `egn1`.
pub fn genflx(block: &Tape, mat: i32, iwt: i32, lord: i32, sigz: &[f64], egn1: f64, unr: Option<&UnresolvedTable>) -> Result<FluxTable, NjoyError> {
    const UP: f64 = 1.001;
    const DN: f64 = 0.999;
    const ETOP: f64 = 1.0e10;
    if iwt <= 0 {
        return Err(NjoyError::NotPorted("groupr genflx with iwt <= 0 (slowing-down flux or getfwt)"));
    }
    let nsigz = sigz.len();
    let nl = ((lord + 1).max(1) as usize).max(if nsigz > 1 { 2 } else { 1 });
    let nfv = 1 + nl * nsigz + 1;
    let sec = block
        .section(mat, 3, 1)
        .ok_or(NjoyError::SectionNotFound { mat, mf: 3, mt: 1 })?;
    let mut cur = crate::endf::records::SectionCursor::new(&sec.rows);
    let _head = cur.read_cont()?;
    let tab = cur.read_tab1()?;
    let mut g = Gety1::new(&tab);
    let mut enext = g.get(0.0).xnext;
    if enext > UP * egn1 {
        return Err(NjoyError::InvalidInput("genflx: total not defined over energy range.".into()));
    }
    crate::groupr::njoy_panel::getwtf(iwt, 0.0)?;
    let mut rows = Vec::new();
    // label "complete flux using nr approximation" (`:5629-5671`).
    loop {
        let e = enext;
        let (wtf, en_w, _) = crate::groupr::njoy_panel::getwtf(iwt, e)?;
        enext = en_w;
        let v = g.get(e);
        let en = v.xnext;
        if en < enext {
            enext = en;
        }
        if en > DN * ETOP {
            enext = ETOP;
        }
        let mut fout = vec![0.0f64; nfv];
        fout[0] = e;
        let mut xtot = 0.0;
        for il in 1..=nl {
            let mt = if il == 1 { 1 } else { 261 };
            let mut tot = vec![v.y; nsigz];
            xtot = 0.0;
            let en = getunr(unr, sigz, mt, e, &mut tot, &mut xtot)?;
            if en >= 0.0 && en < enext {
                enext = en;
            }
            for iz in 0..nsigz {
                // `sigpot = 0` for `iwt > 0` (`ruinb` card 8 is not read).
                let fac = (0.0 + sigz[iz]) / (tot[iz] + sigz[iz]);
                let l = il + nl * iz; // `1+il+nl*(iz-1)`, 0-based
                fout[l] = if il == 1 { wtf * fac } else { fout[l - 1] * fac };
            }
        }
        fout[nfv - 1] = xtot;
        rows.push(fout);
        if enext > DN * ETOP {
            break;
        }
    }
    Ok(FluxTable { nl, rows })
}

/// `getflx` (`groupr.f90:6439-6519`) on a [`FluxTable`]: the `save`d `ip` and the
/// `falo`/`fahi` rows, as row numbers (1-based, `finda`'s).
#[derive(Debug, Clone)]
pub struct FluxCursor {
    table: std::sync::Arc<FluxTable>,
    ip: usize,
    falo: usize,
    fahi: usize,
}

impl FluxCursor {
    /// A cursor on `table`, initialised (`getflx(e = 0)`).
    pub fn new(table: std::sync::Arc<FluxTable>) -> Self {
        let mut c = Self { table, ip: 0, falo: 1, fahi: 2 };
        c.init();
        c
    }

    fn row(&self, i: usize) -> &[f64] {
        let n = self.table.rows.len();
        &self.table.rows[i.clamp(1, n) - 1]
    }

    /// `getflx(e = 0)` (`:6457-6467`): `falo` and `fahi` are rows 1 and 2,
    /// `ip = 0`; returns `enext = falo(1)`.
    pub fn init(&mut self) -> f64 {
        self.falo = 1;
        self.fahi = 2;
        self.ip = 0;
        self.row(1)[0]
    }

    /// `scana` (`util.f90:291-359`): the last row whose energy is `<= e`, or 1
    /// when `e` is below the first.
    fn scana(&self, e: f64) -> usize {
        let rows = &self.table.rows;
        if e < rows[0][0] {
            return 1;
        }
        let mut ip = 0;
        while ip < rows.len() && rows[ip][0] <= e {
            if rows[ip][0] == e {
                return ip + 1;
            }
            ip += 1;
        }
        ip
    }

    /// `getflx(e)` for `nsigz > 1` (`:6476-6510`): `flux[iz * nl + il]` for
    /// `iz < nz`, `il < nl`, and `xtot`; returns `(enext, idis = 0)`.
    ///
    /// # Errors
    /// From `terp1`.
    pub fn get(&mut self, e: f64, nl: usize, nz: usize, flux: &mut [f64], xtot: &mut f64) -> Result<(f64, i32), NjoyError> {
        const EMAX: f64 = 1.0e10;
        let nfp = self.table.rows.len();
        if self.ip == 0 {
            self.ip = self.scana(e);
            self.falo = self.ip;
            self.fahi = self.ip + 1;
        }
        // `do while (e.ge.fahi(1).and.ip.lt.nfp)`: `falo = fahi`, `ip = ip + 1`,
        // `fahi = finda(ip)`. Right after `scana`, `ip` is `falo`'s row, so the
        // first step reads the same row again and a second step follows.
        while e >= self.row(self.fahi)[0] && self.ip < nfp {
            self.falo = self.fahi;
            self.ip += 1;
            self.fahi = self.ip;
        }
        let tnl = self.table.nl;
        let (lo, hi) = (self.row(self.falo).to_vec(), self.row(self.fahi).to_vec());
        for iz in 0..nz {
            for il in 0..nl {
                let l = 1 + il + tnl * iz;
                flux[iz * nl + il] = terp1(lo[0], lo[l], hi[0], hi[l], e, IntLaw::LinLin)?;
            }
        }
        let nfv = lo.len();
        *xtot = terp1(lo[0], lo[nfv - 1], hi[0], hi[nfv - 1], e, IntLaw::LinLin)?;
        let enext = if self.ip == nfp { EMAX } else { hi[0] };
        Ok((enext, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> std::sync::Arc<FluxTable> {
        // nl = 2, nsigz = 2: (e, f11, f21, f12, f22, xtot)
        let rows = vec![
            vec![1.0, 1.0, 1.0, 0.5, 0.25, 0.0],
            vec![2.0, 2.0, 2.0, 1.0, 0.5, 0.0],
            vec![4.0, 4.0, 4.0, 2.0, 1.0, 0.0],
        ];
        std::sync::Arc::new(FluxTable { nl: 2, rows })
    }

    #[test]
    fn getflx_interpolates_each_sigma_zero_and_order() {
        let mut c = FluxCursor::new(table());
        let mut f = vec![0.0; 4];
        let mut xtot = 1.0;
        let (en, idis) = c.get(1.5, 2, 2, &mut f, &mut xtot).unwrap();
        assert_eq!((en, idis), (2.0, 0));
        assert_eq!(f, vec![1.5, 1.5, 0.75, 0.375]);
        assert_eq!(xtot, 0.0);
        // stepping across a node advances `fahi` to the last row: `enext = emax`.
        let (en, _) = c.get(3.0, 1, 1, &mut f, &mut xtot).unwrap();
        assert_eq!(f[0], 3.0);
        assert_eq!(en, 1.0e10);
    }

    #[test]
    fn getunr_without_table_returns_minus_one() {
        let mut s = vec![5.0, 5.0];
        let mut xtot = 0.0;
        assert_eq!(getunr(None, &[1e10, 1.0], 1, 1.0e3, &mut s, &mut xtot).unwrap(), -1.0);
        assert_eq!(s, vec![5.0, 5.0]);
    }
}
