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
//! ~~Not ported: the slowing-down branch (`iwt < 0`, `nflmax > 0`) and `iwt = 0`
//! (`getfwt`, a flux read from `ninwt`). Both are refused by name.~~
//! **CORRECTED 2026-11-14 (gh:#1370):** `iwt < 0` is [`genflx_card8a`]: the
//! Bondarenko branch with card 8a's `sigpot` (`nflmax = 0`), and the
//! slowing-down branch (`nflmax > 0`, solved by
//! [`crate::groupr::slowing_down::solve_flux`]) with its `ninwt` file. `iwt = 0`
//! is [`genflx_fwt`]: `getfwt` ([`FwtReader`]) reads that file back.

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
/// [`NjoyError::NotPorted`] for `iwt <= 0` (use [`genflx_card8a`] for `iwt < 0`);
/// [`NjoyError::InvalidInput`] when MT=1 is missing or starts above `egn1`.
pub fn genflx(block: &Tape, mat: i32, iwt: i32, lord: i32, sigz: &[f64], egn1: f64, unr: Option<&UnresolvedTable>) -> Result<FluxTable, NjoyError> {
    if iwt <= 0 {
        return Err(NjoyError::NotPorted("groupr genflx with iwt <= 0 here: use genflx_card8a or genflx_fwt"));
    }
    let nl = table_nl(lord, sigz.len());
    let (mut g, _awr) = mt1_reader(block, mat)?;
    let enext = g.get(0.0).xnext;
    if enext > 1.001 * egn1 {
        return Err(NjoyError::InvalidInput("genflx: total not defined over energy range.".into()));
    }
    crate::groupr::njoy_panel::getwtf(iwt, 0.0)?;
    let mut rows = Vec::new();
    complete_nr(&mut rows, &mut g, enext, iwt, nl, sigz, 0.0, unr, None, 0.0, None)?;
    Ok(FluxTable { nl, rows })
}

/// `genflx`'s Bondarenko branch for `iwt = 0` (`:5380-5395`, `:5622-5677`): the
/// weight and `xc` come from `getfwt` on `fwt` (the `ninwt` file a flux
/// calculation wrote), and each sigma-zero's factor is `sigz*xc/(sigz*xc+tt)`.
/// `temp` is `temp(jtemp)`, `jtemp` 1-based.
///
/// # Errors
/// As [`genflx`], and those of [`FwtReader`].
#[allow(clippy::too_many_arguments)]
pub fn genflx_fwt(
    block: &Tape,
    mat: i32,
    lord: i32,
    sigz: &[f64],
    egn1: f64,
    unr: Option<&UnresolvedTable>,
    fwt: &mut FwtReader,
    temp: f64,
    jtemp: usize,
) -> Result<FluxTable, NjoyError> {
    let nl = table_nl(lord, sigz.len());
    let (mut g, _awr) = mt1_reader(block, mat)?;
    let enext = g.get(0.0).xnext;
    if enext > 1.001 * egn1 {
        return Err(NjoyError::InvalidInput("genflx: total not defined over energy range.".into()));
    }
    fwt.init(temp, jtemp)?;
    let mut rows = Vec::new();
    complete_nr(&mut rows, &mut g, enext, 0, nl, sigz, 0.0, unr, None, 0.0, Some(fwt))?;
    Ok(FluxTable { nl, rows })
}

/// `getfwt` (`groupr.f90:5686-5775`): the weight function and `xc` read back
/// from the `ninwt` file, with its `save`d `el, en, wl, wn, xl, xn` and the
/// file position, which persists from one temperature to the next.
#[derive(Debug, Clone)]
pub struct FwtReader {
    recs: Vec<Vec<f64>>,
    pos: usize,
    el: f64,
    en: f64,
    wl: f64,
    wn: f64,
    xl: f64,
    xn: f64,
    templ: f64,
}

impl FwtReader {
    /// The records of a Fortran unformatted file of 8-byte reals.
    ///
    /// # Errors
    /// A record whose length words disagree, or are not a multiple of 8.
    pub fn from_bytes(b: &[u8]) -> Result<Self, NjoyError> {
        let bad = || NjoyError::InvalidInput("getfwt: ninwt is not a file of real records".into());
        let mut recs = Vec::new();
        let mut i = 0;
        while i < b.len() {
            let n = u32::from_le_bytes(b.get(i..i + 4).ok_or_else(bad)?.try_into().map_err(|_| bad())?) as usize;
            let body = b.get(i + 4..i + 4 + n).ok_or_else(bad)?;
            let tail = b.get(i + 4 + n..i + 8 + n).ok_or_else(bad)?;
            if n % 8 != 0 || u32::from_le_bytes(tail.try_into().map_err(|_| bad())?) as usize != n {
                return Err(bad());
            }
            recs.push(body.chunks(8).map(|c| f64::from_le_bytes(c.try_into().unwrap_or([0; 8]))).collect());
            i += n + 8;
        }
        Ok(Self { recs, pos: 0, el: 0.0, en: 0.0, wl: 0.0, wn: 0.0, xl: 0.0, xn: 0.0, templ: 1.0e10 })
    }

    /// `read(ninwt)` of `n` words; `None` at the end of the file. A record
    /// shorter than `n` is upstream's runtime stop, refused by name.
    fn read(&mut self, n: usize) -> Result<Option<Vec<f64>>, NjoyError> {
        let Some(r) = self.recs.get(self.pos) else { return Ok(None) };
        if r.len() < n {
            return Err(NjoyError::InvalidInput("getfwt: ninwt record shorter than the read (Fortran runtime stop)".into()));
        }
        self.pos += 1;
        Ok(Some(r[..n].to_vec()))
    }

    /// `getfwt(0, ...)` (`:5706-5734`): position on the block of `temp`.
    ///
    /// # Errors
    /// `getfwt`'s "temperature" stop when the file has no block for `temp`.
    pub fn init(&mut self, temp: f64, jtemp: usize) -> Result<(), NjoyError> {
        const EPS: f64 = 1.0e-6;
        if jtemp == 1 || temp < self.templ {
            self.pos = 0;
        }
        self.el = 0.0;
        self.wl = 0.0;
        self.xl = 0.0;
        self.en = 0.0;
        self.wn = 0.0;
        self.xn = 0.0;
        let missing = || NjoyError::InvalidInput(format!("getfwt: temperature {temp:.4e}"));
        let mut skip_first = jtemp != 1;
        loop {
            if skip_first {
                // `:5713-5716`: the record at the position is a flag or a temperature.
                skip_first = false;
                let flag = self.read(1)?.ok_or_else(missing)?[0];
                if flag >= 0.0 {
                    if (temp - flag).abs() <= EPS {
                        self.templ = flag;
                        return Ok(());
                    }
                    self.skip_block()?;
                }
            }
            // `110`: read a temperature.
            let tempn = self.read(1)?.ok_or_else(missing)?[0];
            if (tempn - temp).abs() <= EPS {
                self.templ = tempn;
                return Ok(());
            }
            self.skip_block()?;
        }
    }

    /// `130`: read records until a negative flag.
    fn skip_block(&mut self) -> Result<(), NjoyError> {
        loop {
            let flag = self.read(1)?.ok_or_else(|| NjoyError::InvalidInput("getfwt: temperature not on ninwt".into()))?[0];
            if flag < 0.0 {
                return Ok(());
            }
        }
    }

    /// `getfwt(e > 0)` (`:5736-5773`): `(w, xc, enext)`.
    ///
    /// # Errors
    /// `getfwt`'s stops: "requested e is out of order", "e outside range of
    /// data".
    pub fn get(&mut self, e: f64) -> Result<(f64, f64, f64), NjoyError> {
        const SMALL: f64 = 1.0e-10;
        const EMAX: f64 = 1.0e10;
        if (e - self.el).abs() < self.el * SMALL && self.en < 0.0 {
            return Ok((self.wl, self.xl, self.en));
        }
        let mut exact = (e - self.en).abs() < self.en * SMALL;
        let mut inside = !exact && e > self.el * (1.0 - SMALL) && e < self.en * (1.0 + SMALL);
        if !exact && !inside {
            if e < self.el * (1.0 - SMALL) {
                return Err(NjoyError::InvalidInput("getfwt: requested e is out of order".into()));
            }
            loop {
                self.el = self.en;
                self.wl = self.wn;
                self.xl = self.xn;
                let r = self.read(3)?.ok_or_else(|| NjoyError::InvalidInput("getfwt: e outside range of data".into()))?;
                (self.en, self.wn, self.xn) = (r[0], r[1], r[2]);
                exact = (e - self.en).abs() < self.en * SMALL;
                if exact {
                    break;
                }
                inside = e > self.el * (1.0 - SMALL) && e < self.en * (1.0 + SMALL);
                if inside {
                    break;
                }
            }
        }
        if inside && !exact {
            let w = terp1(self.el, self.wl, self.en, self.wn, e, IntLaw::LinLin)?;
            let xc = terp1(self.el, self.xl, self.en, self.xn, e, IntLaw::LinLin)?;
            return Ok((w, xc, self.en));
        }
        // `240`: an exact match; the next record is read, and `enext = emax`.
        let (w, xc) = (self.wn, self.xn);
        self.el = self.en;
        self.wl = self.wn;
        self.xl = self.xn;
        let r = self.read(3)?.ok_or_else(|| NjoyError::InvalidInput("getfwt: end of ninwt after an exact match (Fortran runtime stop)".into()))?;
        (self.en, self.wn, self.xn) = (r[0], r[1], r[2]);
        Ok((w, xc, EMAX))
    }
}

/// `nl` of `genflx`'s table: `lord + 1`, and 2 when that is 1 with `nsigz > 1`
/// (`:5374-5375`).
fn table_nl(lord: i32, nsigz: usize) -> usize {
    ((lord + 1).max(1) as usize).max(if nsigz > 1 { 2 } else { 1 })
}

/// MT=1 of `block` as `gety1` reads it on `nscr`, and the AWR of its head.
fn mt1_reader(block: &Tape, mat: i32) -> Result<(Gety1, f64), NjoyError> {
    let sec = block.section(mat, 3, 1).ok_or(NjoyError::SectionNotFound { mat, mf: 3, mt: 1 })?;
    let mut cur = crate::endf::records::SectionCursor::new(&sec.rows);
    let head = cur.read_cont()?;
    let tab = cur.read_tab1()?;
    Ok((Gety1::new(&tab), head.c2))
}

/// The `ninwt` file `genflx` writes (`:5586-5615`, `:5662-5664`, `:5680-5682`):
/// Fortran unformatted sequential records of 8-byte reals, each between two
/// 4-byte length words.
#[derive(Debug, Clone, Default)]
pub struct NinwtFile {
    /// The file's bytes so far.
    pub bytes: Vec<u8>,
}

impl NinwtFile {
    /// `write(ninwt) v(1), ..., v(n)`.
    pub fn record(&mut self, v: &[f64]) {
        let len = (8 * v.len()) as u32;
        self.bytes.extend_from_slice(&len.to_le_bytes());
        for x in v {
            self.bytes.extend_from_slice(&x.to_le_bytes());
        }
        self.bytes.extend_from_slice(&len.to_le_bytes());
    }
}

/// `genflx`'s "complete flux using nr approximation" loop (`:5622-5677`) from
/// `enext`, appended to `rows`. `sigpot` is card 8a's (0 for `iwt > 0`).
/// With `ninwt`, each row is written as `(e, wtf, xc)`: `xc` is `genflx`'s local,
/// set only by the computed-flux rows, so these rows carry the last computed
/// row's value (`:5662-5664`); then the `-1` flag record (`:5680-5682`).
#[allow(clippy::too_many_arguments)]
fn complete_nr(
    rows: &mut Vec<Vec<f64>>,
    g: &mut Gety1,
    mut enext: f64,
    iwt: i32,
    nl: usize,
    sigz: &[f64],
    sigpot: f64,
    unr: Option<&UnresolvedTable>,
    mut ninwt: Option<&mut NinwtFile>,
    xc: f64,
    mut fwt: Option<&mut FwtReader>,
) -> Result<(), NjoyError> {
    const DN: f64 = 0.999;
    const ETOP: f64 = 1.0e10;
    let nsigz = sigz.len();
    let nfv = 1 + nl * nsigz + 1;
    loop {
        let e = enext;
        // `:5626-5627`: `getwtf` for `iwt /= 0`, `getfwt` (with its `xc`) for 0.
        let (wtf, en_w, xc_e) = match fwt.as_mut() {
            Some(f) => {
                let (w, xc_f, en) = f.get(e)?;
                (w, en, xc_f)
            }
            None => {
                let (w, en, _) = crate::groupr::njoy_panel::getwtf(iwt, e)?;
                (w, en, xc)
            }
        };
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
                let fac = if fwt.is_some() {
                    sigz[iz] * xc_e / (sigz[iz] * xc_e + tot[iz])
                } else {
                    (sigpot + sigz[iz]) / (tot[iz] + sigz[iz])
                };
                let l = il + nl * iz; // `1+il+nl*(iz-1)`, 0-based
                fout[l] = if il == 1 { wtf * fac } else { fout[l - 1] * fac };
            }
        }
        fout[nfv - 1] = xtot;
        rows.push(fout);
        if let Some(f) = ninwt.as_mut() {
            f.record(&[e, wtf, xc]);
        }
        if enext > DN * ETOP {
            break;
        }
    }
    if let Some(f) = ninwt {
        f.record(&[-1.0, -1.0, -1.0]);
    }
    Ok(())
}

/// `genflx` (`groupr.f90:5309-5684`) for `iwt < 0`, with card 8a's values `p`
/// (`genwtf`, `:4980-4997`).
///
/// - `nflmax = 0`: the Bondarenko branch with card 8a's `sigpot`.
/// - `nflmax > 0`: the slowing-down flux from `felo = max(egn(1), 0.1)` up to
///   `fehi` or `nflmax` points (grid by `gety1` on MT=1, `gety2` on MT=2, weights
///   by `getwtf`), solved by [`crate::groupr::slowing_down::solve_flux`]; the
///   weight function's shape below `felo` (`:5555-5583`); the computed rows
///   (`:5585-5619`); then the narrow-resonance completion. `fehi` is the module
///   variable: `genflx` leaves it at the grid's last energy (`:5452`), which the
///   next temperature's call starts from.
/// - `ninwt != 0`: the computed and narrow-resonance rows are written to
///   `ninwt` as upstream writes them, `temp` first.
///
/// # Errors
/// Refused by name, as upstream reads values it never set:
/// - a computed (`nflmax > 0`) table with an unresolved range below its first
///   narrow-resonance row: the computed rows' `xtot` word (`fout(nfv)`) is never
///   set (`:5568-5575`, `:5596-5605`), and `getflx` would interpolate it there;
/// - `ninwt != 0` with `nflmax = 0`: `xc` is written but never set;
/// - MT=2 starting at zero: `gety2` has no zero-extension scan, which this
///   crate's reader does (not translated).
#[allow(clippy::too_many_arguments)]
pub fn genflx_card8a(
    block: &Tape,
    mat: i32,
    iwt: i32,
    lord: i32,
    sigz: &[f64],
    egn1: f64,
    unr: Option<&UnresolvedTable>,
    p: &crate::groupr::FluxCalcParams,
    fehi: &mut f64,
    temp: f64,
    ninwt: &mut NinwtFile,
) -> Result<FluxTable, NjoyError> {
    if iwt >= 0 {
        return Err(NjoyError::InvalidInput("genflx_card8a: iwt must be negative".into()));
    }
    let nsigz = sigz.len();
    let nl = table_nl(lord, nsigz);
    let nfv = 1 + nl * nsigz + 1;
    let (mut g, awr) = mt1_reader(block, mat)?;
    if p.nflmax == 0 {
        if p.ninwt != 0 {
            return Err(NjoyError::NotPorted("groupr genflx: ninwt with nflmax = 0 writes xc, which is never set (undefined)"));
        }
        let enext = g.get(0.0).xnext;
        if enext > 1.001 * egn1 {
            return Err(NjoyError::InvalidInput("genflx: total not defined over energy range.".into()));
        }
        crate::groupr::njoy_panel::getwtf(iwt, 0.0)?;
        let mut rows = Vec::new();
        complete_nr(&mut rows, &mut g, enext, iwt, nl, sigz, p.sigpot_barns, unr, None, 0.0, None)?;
        return Ok(FluxTable { nl, rows });
    }
    // `:5396-5416`.
    let small = 1.0e-10;
    let alpha2 = if p.alpha3 != 0.0 && p.alpha2 == 0.0 { small } else { p.alpha2 };
    let a = (awr - 1.0) / (awr + 1.0);
    let mut alphas = vec![a * a];
    if alpha2 != 0.0 {
        alphas.push(alpha2);
    }
    if p.alpha3 != 0.0 {
        alphas.push(p.alpha3);
    }
    let nemax = p.nflmax.max(0) as usize;
    let mut felo = egn1;
    let ebot = felo;
    if felo < 0.1 {
        felo = 0.1;
    }
    crate::groupr::njoy_panel::getwtf(iwt, 0.0)?;
    // `:5420-5432`: MT=1 through the scratch tape (`gety1`), MT=2 (`gety2`).
    let s2 = block.section(mat, 3, 2).ok_or(NjoyError::SectionNotFound { mat, mf: 3, mt: 2 })?;
    let mut c2 = crate::endf::records::SectionCursor::new(&s2.rows);
    let _ = c2.read_cont()?;
    let t2 = c2.read_tab1()?;
    if t2.pairs.first().is_some_and(|x| x.1 == 0.0) {
        return Err(NjoyError::NotPorted("groupr genflx: gety2 on an MT=2 that starts at zero"));
    }
    let mut g2 = Gety1::new(&t2);
    let _ = g.get(0.0);
    let _ = g2.get(0.0);
    // `:5434-5452`.
    let (mut energies, mut tt, mut ss, mut wt) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut enext = felo;
    let mut e = 0.0;
    while e < *fehi && energies.len() < nemax {
        e = enext;
        if e > *fehi {
            e = *fehi;
        }
        let v = g.get(e);
        enext = v.xnext;
        let y2 = g2.get(e).y;
        let (wtf, _, _) = crate::groupr::njoy_panel::getwtf(iwt, e)?;
        energies.push(e);
        tt.push(v.y);
        ss.push(y2);
        wt.push(wtf);
    }
    if energies.is_empty() {
        return Err(NjoyError::InvalidInput("genflx: no computed flux points (nflmax or fehi)".into()));
    }
    *fehi = e;
    let b = crate::groupr::slowing_down::solve_flux(&energies, &tt, &ss, &wt, sigz, p.sigpot_barns, &alphas, p.sam_barns, p.beta, p.gamma);
    // `:5553-5583`: the weight function's shape below `b(1)`.
    let mut rows: Vec<Vec<f64>> = Vec::new();
    crate::groupr::njoy_panel::getwtf(iwt, 0.0)?;
    let (w1, _, _) = crate::groupr::njoy_panel::getwtf(iwt, energies[0])?;
    let factor: Vec<f64> = (0..nsigz).map(|iz| b[0][iz] / w1).collect();
    crate::groupr::njoy_panel::getwtf(iwt, 0.0)?;
    let mut enxt = ebot;
    while enxt < energies[0] {
        let e = enxt;
        let (wtf, en, _) = crate::groupr::njoy_panel::getwtf(iwt, e)?;
        enxt = en;
        let mut fout = vec![0.0f64; nfv];
        fout[0] = e;
        for iz in 0..nsigz {
            for il in 1..=nl {
                fout[il + nl * iz] = factor[iz] * wtf;
            }
        }
        rows.push(fout);
    }
    // `:5585-5619`: the computed rows, and `ninwt`.
    let jsigz = p.jsigz.max(1) as usize;
    let write = p.ninwt != 0;
    let (sigzj, indx) = if write {
        let jz = sigz.get(jsigz - 1).copied().ok_or_else(|| NjoyError::InvalidInput(format!("genflx: jsigz {jsigz} > nsigz")))?;
        ninwt.record(&[temp]);
        (jz, 1 + nl * (jsigz - 1))
    } else {
        (1.0e10, 0)
    };
    let mut gw = if write {
        let (mut r, _) = mt1_reader(block, mat)?;
        let _ = r.get(0.0);
        Some(r)
    } else {
        None
    };
    let mut xc = 0.0;
    for (ie, &e) in energies.iter().enumerate() {
        let mut fout = vec![0.0f64; nfv];
        fout[0] = e;
        for iz in 0..nsigz {
            let fac = (p.sigpot_barns + sigz[iz]) / (tt[ie] + sigz[iz]);
            for il in 1..=nl {
                let l = il + nl * iz;
                fout[l] = if il == 1 { b[ie][iz] } else { fout[l - 1] * fac };
            }
        }
        if let Some(r) = gw.as_mut() {
            let t = r.get(e).y;
            xc = 1.0 + t / sigzj;
            ninwt.record(&[e, fout[indx], xc]);
        }
        rows.push(fout);
    }
    let first_nr = rows.len();
    let out = if write { Some(ninwt) } else { None };
    complete_nr(&mut rows, &mut g, enext, iwt, nl, sigz, p.sigpot_barns, unr, out, xc, None)?;
    if let Some(t) = unr {
        let lowest = t.points().first().map_or(f64::INFINITY, |x| x.energy_ev);
        if lowest < rows[first_nr][0] {
            return Err(NjoyError::NotPorted("groupr genflx: unresolved range below the computed flux's end (fout(nfv) unset there, undefined)"));
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
