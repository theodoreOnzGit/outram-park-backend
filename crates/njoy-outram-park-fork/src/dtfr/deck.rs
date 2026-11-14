// Translated from NJOY2016.80 (commit ac5adf5) `src/dtfr.f90`: `dtfr` (`:51-588`),
// `ruin` (`:590-767`) and `dtfout` (`:769-946`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! **DTFR from a deck** (gh:#1369, deck 03): the cards, the accumulation loop
//! over the GENDF records of each material, the DTF tables on `nout` and the
//! plot commands on `nplot` ([`super::plots`]), line for line with upstream.
//!
//! This is a translation of the module's own control flow, beside the
//! structured kernels of [`super::assemble`] (which were validated against
//! NJOY's printed tables but take an in-memory GENDF and write no file).
//!
//! Upstream behaviour kept:
//! - `iphph` (photon-photon tables) is a module variable set by any MF=23
//!   section and never cleared (`:298`), so it carries to later materials.
//! - an MT=501 section turns the total edit's MT into 501 (`:297`), which is
//!   the MT the plot of the total then reads from the PENDF.
//! - the scratch tape `nscr` has `nin`'s mode, so a coded GENDF is read back as
//!   written (the values are unchanged by the round trip).
//! - a GAMINR tape's head carries `ngg` where GROUPR's carries `ngn`, so a photon
//!   table is laid out as a "neutron" table of the photon groups (`:203-214`).
//!
//! Refused by name: a material not found on the GENDF tape is upstream's
//! `***material not found` and the next card is read, as upstream does.

use crate::deck::cards::{CardReader, Slot};
use crate::errorr::gendf::{GendfKind, GendfRecord, GendfTape};
use crate::NjoyError;

/// DTFR's cards (`ruin` and card 8).
#[derive(Debug, Clone, PartialEq)]
pub struct DtfrCards {
    /// `nin nout npend nplot`.
    pub units: [i32; 4],
    /// `iprint ifilm iedit`.
    pub options: [i32; 3],
    /// Card 3 (`iedit = 0`): `nlmax ng iptotl ipingp itabl ned nthrm`, or card 6
    /// (`iedit = 1`) in the first two.
    pub tables: [i32; 7],
    /// Card 3a: `mti mtc nlc`.
    pub thermal: [i32; 3],
    /// Card 4: the `iptotl - 3` edit names.
    pub names: Vec<String>,
    /// Card 5: `(jpos, mt, mult)` triplets.
    pub edits: Vec<[i32; 3]>,
    /// Card 7: `nptabl ngp`.
    pub photon: [i32; 2],
    /// Card 8, one per table set: `hisnam matd jsigz dtemp`.
    pub materials: Vec<(String, i32, i32, f64)>,
}

/// Read DTFR's cards (`ruin`, `:590-767`, and card 8 until `matd = 0`).
///
/// # Errors
/// A malformed card; `iping.le.iptotl.` as upstream.
pub fn read_dtfr(r: &mut CardReader) -> Result<DtfrCards, NjoyError> {
    let u = r.read_ints(&[0, 0, 0, 0])?;
    let o = r.read_ints(&[0, 0, 0])?;
    let mut tables = [0i32; 7];
    let mut thermal = [0i32; 3];
    let mut names = Vec::new();
    let mut edits = Vec::new();
    if o[2] == 0 {
        let t = r.read_ints(&[0, 0, 0, 0, 0, 0, 0])?;
        tables.copy_from_slice(&t);
        if tables[3] <= tables[2] {
            return Err(NjoyError::InvalidInput("dtfr: iping.le.iptotl.".into()));
        }
        if tables[6] != 0 {
            let t = r.read_ints(&[0, 0, 0])?;
            thermal.copy_from_slice(&t);
        }
        let (nlmax, iptotl, ned) = (tables[0], tables[2], tables[5]);
        if nlmax > 0 && ned != 0 {
            let nedpos = iptotl - 3;
            if nedpos != 0 {
                let mut s: Vec<Slot> = (0..nedpos.max(0)).map(|_| Slot::string(6, " ")).collect();
                r.read(&mut s)?;
                names = s.iter().map(|x| format!("{:<6.6}", x.text())).collect();
            }
            let nedu = usize::try_from(ned).map_err(|_| NjoyError::InvalidInput("dtfr: negative ned".into()))?;
            let v = r.read_ints(&vec![0; 3 * nedu])?;
            edits = v.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
        }
    } else {
        let t = r.read_ints(&[5, 30])?;
        tables[0] = t[0];
        tables[1] = t[1];
    }
    let p = r.read_ints(&[0, 0])?;
    let mut materials = Vec::new();
    loop {
        let mut s = [Slot::string(6, " "), Slot::Int(0), Slot::Int(1), Slot::Real(300.0)];
        r.read(&mut s)?;
        let matd = s[1].int();
        if matd == 0 {
            break;
        }
        materials.push((format!("{:<6.6}", s[0].text()), matd, s[2].int(), s[3].real()));
    }
    Ok(DtfrCards { units: [u[0], u[1], u[2], u[3]], options: [o[0], o[1], o[2]], tables, thermal, names, edits, photon: [p[0], p[1]], materials })
}

/// The CLAW tables of `ruin` (`:603-621`).
const KMTED: [i32; 50] = [
    2, 4, 16, 6, 7, 8, 9, 17, 102, 107, 103, 19, 20, 21, 18, 104, 105, 106, 111, 112, 113, 114, 108, 109, 32, 35, 34, 22, 24, 23,
    25, 28, 29, 30, 33, 36, 37, 38, 470, 471, 455, 300, 301, 443, 443, 444, 452, 1, 0, 0,
];
const KJPED: [i32; 50] = [
    1, 2, 3, 3, 3, 3, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30,
    31, 32, 33, 34, 35, 36, 37, 38, 39, 39, 40, 41, 42, 43, 44, 45,
];
const HMTID: [&str; 50] = [
    "  els", "  ins", "  n2n", "  n3n", "  ngm", "  nal", "   np", " fdir", "  nnf", " n2nf", " ftot", "   nd", "   nt", " nhe3",
    "  n2p", "  npa", " nt2a", " nd2a", "  n2a", "  n3a", "  nnd", "nnd2a", "nnhe3", "  nna", " n2na", " nn3a", " n3na", "  nnp",
    " nn2a", "n2n2a", "  nnt", "nnt2a", "  n4n", " n3nf", "  chi", " chid", "  nud", "  phi", "theat", "kerma", "tdame", " nusf",
    " totl", "     ", "     ", "     ", "     ", "     ", "     ", "     ",
];

/// DTFR's module state (`dtfm`'s globals).
#[derive(Debug, Clone)]
pub(crate) struct Dtfr {
    pub nout: i32,
    pub npend: i32,
    pub ifilm: i32,
    pub iedit: i32,
    pub nlmax: i32,
    pub ng: i32,
    pub iptotl: i32,
    pub ipingp: i32,
    pub itabl: i32,
    pub ned: i32,
    pub nptabl: i32,
    pub jgthrm: i32,
    /// 1-based `hednam(1..3+nedmax)`.
    pub hednam: Vec<String>,
    pub jped: Vec<i32>,
    pub mted: Vec<i32>,
    pub multed: Vec<i32>,
    pub ids: Vec<i32>,
    pub hisnam: String,
    pub matd: i32,
    pub jz: i32,
    pub dtemp: f64,
    pub kpos: i32,
    pub ked: i32,
    pub lpos: i32,
    pub led: i32,
    pub iphph: i32,
    pub mti: i32,
    pub mtc: i32,
    pub bot: f64,
    pub axl: f64,
    pub axd: f64,
    pub top: f64,
    pub ixo: i32,
    pub iyo: i32,
    pub xmin: f64,
    pub xmax: f64,
    pub ymin: f64,
    pub ymax: f64,
    /// 1-based `egn`, `egg`.
    pub egn: Vec<f64>,
    pub ngn: i32,
    pub egg: Vec<f64>,
    pub ngp: i32,
    /// `iverf` of the endf module, as last set (the PENDF's, `:251-258`).
    pub iverf: i32,
    /// The PENDF for the point plots.
    pub pendf: Option<crate::endf::tape::Tape>,
    pub out: String,
    pub plot: String,
}

const NEDMAX: usize = 50;
const NWSMAX: usize = 500_000;

impl Dtfr {
    fn from_cards(c: &DtfrCards) -> Self {
        let n = 3 + NEDMAX + 1;
        let mut d = Dtfr {
            nout: c.units[1],
            npend: c.units[2],
            ifilm: if c.units[3] == 0 { 0 } else { c.options[1] },
            iedit: c.options[2],
            nlmax: 0,
            ng: 0,
            iptotl: 0,
            ipingp: 0,
            itabl: 0,
            ned: 0,
            nptabl: 0,
            jgthrm: 0,
            hednam: vec!["      ".to_string(); n],
            jped: vec![0; n],
            mted: vec![0; n],
            multed: vec![0; n],
            ids: vec![0; n],
            hisnam: String::new(),
            matd: 0,
            jz: 1,
            dtemp: 300.0,
            kpos: 0,
            ked: 0,
            lpos: 0,
            led: 0,
            iphph: 0,
            mti: 0,
            mtc: 0,
            bot: 0.0,
            axl: 0.0,
            axd: 0.0,
            top: 0.0,
            ixo: 0,
            iyo: 0,
            xmin: 0.0,
            xmax: 0.0,
            ymin: 0.0,
            ymax: 0.0,
            egn: vec![0.0; 2502],
            ngn: 0,
            egg: vec![0.0; 401],
            ngp: 0,
            iverf: 0,
            pendf: None,
            out: String::new(),
            plot: String::new(),
        };
        let t = c.tables;
        let nedpos;
        if d.iedit == 0 {
            d.nlmax = t[0];
            d.ng = t[1];
            d.iptotl = t[2];
            d.ipingp = t[3];
            d.itabl = t[4];
            d.ned = t[5];
            d.jgthrm = d.ng + 1 - t[6];
            if t[6] != 0 {
                d.mti = c.thermal[0];
                d.mtc = c.thermal[1];
            }
            nedpos = d.iptotl - 3;
            for (i, s) in c.names.iter().enumerate() {
                d.hednam[i + 1] = s.clone();
            }
            for (j, e) in c.edits.iter().enumerate() {
                d.jped[j + 1] = e[0];
                d.mted[j + 1] = e[1];
                d.multed[j + 1] = e[2];
            }
        } else {
            d.nlmax = t[0];
            d.ng = t[1];
            d.iptotl = 43 + 3;
            d.ned = 48;
            d.jgthrm = d.ng + 1;
            d.ipingp = d.iptotl + 1;
            d.itabl = d.iptotl + d.ng;
            nedpos = d.iptotl - 3;
            for i in 1..=nedpos as usize {
                d.hednam[i] = format!("{:<6.6}", HMTID[i - 1]);
            }
            for i in 1..=d.ned as usize {
                d.jped[i] = KJPED[i - 1];
                d.mted[i] = KMTED[i - 1];
                d.multed[i] = if i == 44 { 0 } else { 1 };
            }
        }
        // `kpos`, `lpos` (`:719-734`), only with edits and neutron tables.
        if d.nlmax > 0 && d.ned > 0 {
            for i in 1..=d.ned as usize {
                if d.mted[i] == 470 {
                    d.kpos = d.jped[i];
                    d.ked = i as i32;
                }
                if d.mted[i] == 471 {
                    d.lpos = d.jped[i];
                    d.led = i as i32;
                }
            }
        }
        let _ = nedpos;
        // The three standard edits (`:739-747`).
        let (ned, iptotl) = (d.ned as usize, d.iptotl);
        d.hednam[(iptotl - 1) as usize] = "nusigf".into();
        d.jped[ned + 2] = iptotl - 1;
        d.mted[ned + 2] = 1000;
        d.hednam[(iptotl - 2) as usize] = "absorp".into();
        d.jped[ned + 1] = iptotl - 2;
        d.mted[ned + 1] = 1000;
        d.hednam[iptotl as usize] = "total ".into();
        d.jped[ned + 3] = iptotl;
        d.mted[ned + 3] = 1;
        d.nptabl = c.photon[0];
        d.ngp = c.photon[1];
        d
    }
}

/// A reader over one material's records on the scratch tape `nscr`.
struct Scratch {
    recs: Vec<GendfRecord>,
    pos: usize,
}

impl Scratch {
    fn next(&mut self) -> Result<&GendfRecord, NjoyError> {
        let r = self.recs.get(self.pos).ok_or_else(|| NjoyError::InvalidInput("dtfr: end of scratch tape".into()))?;
        self.pos += 1;
        Ok(r)
    }
    fn tosend(&mut self) -> Result<(), NjoyError> {
        loop {
            if self.next()?.kind == GendfKind::Send {
                return Ok(());
            }
        }
    }
}

/// `a(1:6)` then the LIST words, as `listio` leaves them (1-based: `a[0]` unused).
fn a_of(r: &GendfRecord) -> Vec<f64> {
    let mut a = vec![0.0, r.c[0], r.c[1], f64::from(r.n[0]), f64::from(r.n[1]), f64::from(r.n[2]), f64::from(r.n[3])];
    a.extend_from_slice(&r.data);
    a
}

fn nint(x: f64) -> i32 {
    x.round() as i32
}

/// Run DTFR (`dtfr.f90:51-588`) on the GENDF `gendf`: the DTF tables text and
/// the plot-command text.
///
/// # Errors
/// The upstream `error` stops (group counts, storage), a missing PENDF
/// section, or a malformed GENDF.
pub fn run_dtfr_tape(c: &DtfrCards, gendf: &GendfTape, pendf: Option<&crate::endf::tape::Tape>) -> Result<(String, String), NjoyError> {
    let mut d = Dtfr::from_cards(c);
    d.pendf = pendf.cloned();
    // `:166-168`.
    let mut csz = 0.30f64;
    if d.ifilm > 1 {
        csz /= 2.0;
    }
    d.plot.push_str(&format!("1 2{} /\n", crate::viewr::fmt::f(csz, 6, 3)));
    let mut spect = vec![0.0f64; 2502];
    let mut fcap = vec![0.0f64; 2502];
    let mut ffis = vec![0.0f64; 2502];
    let mut sig = vec![0.0f64; NWSMAX + 1];
    for (hisnam, matd, jzd, dtemp) in &c.materials {
        d.hisnam = hisnam.clone();
        d.matd = *matd;
        d.dtemp = *dtemp;
        let jzd = *jzd;
        let mut cnorm = 0.0f64;
        let mut cnm = 0.0f64;
        let mut dnorm = 0.0f64;
        let mut ilmax = 0i32;
        let mut ipmax = 0i32;
        d.ixo = 0;
        d.iyo = 0;
        // `:189-238`: the material's head at the requested temperature, and the
        // rest of the material to the scratch tape.
        let recs = &gendf.records;
        let mut i = 0usize;
        let mut found: Option<(usize, Vec<f64>)> = None;
        while i < recs.len() {
            let r = &recs[i];
            if r.kind == GendfKind::Tend {
                break;
            }
            if r.mat == d.matd && r.kind == GendfKind::Cont && r.mf == 1 {
                let list = recs.get(i + 1).ok_or_else(|| NjoyError::InvalidInput("dtfr: truncated gendf".into()))?;
                let temp = list.c[0];
                if (temp - d.dtemp).abs() <= 1.0 / 1000.0 {
                    // `a(1:6)` the head, `a(7:)` the LIST as `listio(nin,0,0,a(7))` reads it.
                    let mut a = a_of(r);
                    a.extend_from_slice(&a_of(list)[1..]);
                    found = Some((i + 2, a));
                    break;
                }
            }
            i += 1;
        }
        let Some((after, a)) = found else {
            // `***material not found` (`:200-202`).
            continue;
        };
        let nsigz = nint(a[4]);
        let ntw = nint(a[6]);
        d.ngn = nint(a[9]);
        if d.ngn != d.ng {
            return Err(NjoyError::InvalidInput("dtfr: number of neutron groups disagrees with number requested.".into()));
        }
        let ngg = nint(a[10]);
        if d.ngp > 0 && ngg != d.ngp {
            return Err(NjoyError::InvalidInput("dtfr: number of gamma groups disagrees with number requested.".into()));
        }
        let mut jbase = 12 + ntw as usize;
        jbase += nsigz.max(0) as usize;
        for j in 1..=(d.ngn + 1) as usize {
            d.egn[j] = a[j + jbase];
        }
        jbase += (d.ngn + 1) as usize;
        for j in 1..=(ngg + 1) as usize {
            d.egg[j] = a[j + jbase];
        }
        // `tofend`, then `tomend` to the scratch tape.
        let mut k = after;
        while k < recs.len() && recs[k].kind != GendfKind::Fend {
            k += 1;
        }
        k += 1;
        let mut scr = Vec::new();
        while k < recs.len() {
            let r = recs[k].clone();
            let end = r.kind == GendfKind::Mend;
            scr.push(r);
            k += 1;
            if end {
                break;
            }
        }
        // `:241-270`: the PENDF at this temperature.
        if d.npend != 0 {
            pendf_temperature(&mut d)?;
        }
        let mut il: i32 = if d.nlmax == 0 { 0 } else { 1 };
        let mut ip: i32 = if d.nlmax == 0 { 1 } else { 0 };
        let mut i618 = 0;
        loop {
            // label 110.
            let mut sc = Scratch { recs: scr.clone(), pos: 0 };
            let mut ig = d.ngn;
            let nws = if il > 0 { d.itabl * d.ng } else { d.ngp * d.ng };
            if nws as usize > NWSMAX {
                return Err(NjoyError::InvalidInput("dtfr: not enough storage for table.".into()));
            }
            let n3 = (d.ned + 3) as usize;
            for v in d.ids.iter_mut().take(n3 + 1) {
                *v = 0;
            }
            sig.iter_mut().for_each(|v| *v = 0.0);
            let (mut nl, mut nz) = (1i32, 1i32);
            let (mut mfh, mut mth);
            mfh = 0;
            mth = 0;
            loop {
                // label 150.
                if ig >= d.ngn {
                    let r = sc.next()?.clone();
                    if r.kind == GendfKind::Mend || r.mat == 0 {
                        break;
                    }
                    mfh = r.mf;
                    mth = r.mt;
                    if r.kind != GendfKind::Cont || mfh == 0 || mth == 0 {
                        continue;
                    }
                    if mth == 501 {
                        d.mted[d.ned as usize + 3] = 501;
                    }
                    if mfh == 23 {
                        d.iphph = 1;
                    }
                    nl = r.n[0];
                    if il > nl || ip > nl {
                        sc.tosend()?;
                        continue;
                    }
                    nz = r.n[1];
                    d.jz = jzd;
                    if d.jz > nz {
                        d.jz = 1;
                    }
                }
                // label 160.
                let r = sc.next()?.clone();
                if r.kind != GendfKind::List {
                    return Err(NjoyError::InvalidInput("dtfr: expected a LIST record".into()));
                }
                let a = a_of(&r);
                if a.len() > 40_000 {
                    return Err(NjoyError::InvalidInput("dtfr: not enough storage for record.".into()));
                }
                let ng2 = nint(a[3]);
                let ig2lo = nint(a[4]);
                ig = nint(a[6]);
                let lz = 6i32;
                let at = |idx: i32| a.get(idx as usize).copied().unwrap_or(0.0);
                let ng = d.ng;
                let itabl = d.itabl;
                let iptotl = d.iptotl;
                let jz = d.jz;
                let s = |loc: i32| loc as usize;
                // The dispatch of `:333-340`.
                let mut target = 0;
                if il == 1 && mth == 455 {
                    target = 400;
                } else if il > 0 && (mfh == 3 || mfh == 23) {
                    target = 200;
                } else if il > 0 && (mfh == 6 || mfh == 26) {
                    target = 300;
                } else if ip > 0 && (mfh == 16 || mfh == 17) {
                    target = 600;
                }
                if target == 400 {
                    if mfh == 5 {
                        // label 440.
                        for kg in 2..=ng2 {
                            let jg = ng - ig2lo - kg + 3;
                            for id in 1..=nl {
                                let loca = lz + id + nl * (kg - 1);
                                if d.lpos > 0 {
                                    let locs = d.lpos + itabl * (jg - 1);
                                    sig[s(locs)] += at(loca);
                                    d.ids[d.led as usize] = d.led;
                                }
                                if d.kpos > 0 {
                                    if dnorm == 0.0 {
                                        return Err(NjoyError::InvalidInput("dtfr: delayed nubar required to add delayed chi to total.".into()));
                                    }
                                    let locs = d.kpos + itabl * (jg - 1);
                                    sig[s(locs)] += dnorm * at(loca);
                                    cnorm += dnorm * at(loca);
                                    d.ids[d.ked as usize] = d.ked;
                                }
                            }
                        }
                        continue;
                    }
                    let jg = ng - ig + 1;
                    let locs = iptotl - 1 + itabl * (jg - 1);
                    let loca = lz + 1;
                    sig[s(locs)] += at(loca + 1) * at(loca + 2) * ffis[s(jg)];
                    d.ids[d.ned as usize + 2] = 1;
                    dnorm += at(loca) * at(loca + 1) * at(loca + 2) * ffis[s(jg)];
                    target = 200;
                }
                if target == 200 {
                    let lbl;
                    let mut jg = ng - ig + 1;
                    if mth == 1 || mth == 501 {
                        // label 210.
                        if il > nl {
                            continue;
                        }
                        jg = ng - ig + 1;
                        let loca = lz + il + nl * ((jz - 1) + nz);
                        let mut locs = iptotl + itabl * (jg - 1);
                        d.ids[d.ned as usize + 3] = 1;
                        sig[s(locs)] = at(loca);
                        if il > 1 {
                            continue;
                        }
                        locs -= 2;
                        sig[s(locs)] += at(loca);
                        d.ids[d.ned as usize + 1] = 1;
                        if d.ned == 0 {
                            continue;
                        }
                        lbl = 240;
                    } else if mth == 102 || mth == 18 {
                        // label 220.
                        jg = ng - ig + 1;
                        let loca = lz + il + nl * nz;
                        if mth == 18 {
                            ffis[s(jg)] = 1.0;
                            if at(loca) != 0.0 {
                                ffis[s(jg)] = at(loca + nl * (jz - 1)) / at(loca);
                            }
                        }
                        if mth == 102 {
                            fcap[s(jg)] = 1.0;
                            if at(loca) != 0.0 {
                                fcap[s(jg)] = at(loca + nl * (jz - 1)) / at(loca);
                            }
                        }
                        lbl = if d.ned > 0 { 240 } else { 260 };
                    } else {
                        lbl = if d.ned > 0 { 240 } else { 260 };
                    }
                    if lbl == 240 {
                        if il > 1 {
                            continue;
                        }
                        for ied in 1..=d.ned as usize {
                            if mth == 1 && d.mted[ied] == 300 {
                                // `jg` is whatever it was: upstream does not set it here.
                                let loca = lz + il + nl * (jz - 1);
                                let jpos = d.jped[ied];
                                let locs = jpos + itabl * (jg - 1);
                                sig[s(locs)] = at(loca) * f64::from(d.multed[ied]) + sig[s(locs)];
                                d.ids[ied] = ied as i32;
                            } else if mth == d.mted[ied] {
                                jg = ng - ig + 1;
                                let loca = lz + il + nl * ((jz - 1) + nz);
                                let locs = d.jped[ied] + itabl * (jg - 1);
                                sig[s(locs)] += f64::from(d.multed[ied]) * at(loca);
                                d.ids[ied] = ied as i32;
                            }
                        }
                    }
                    // label 260.
                    if il > 1 {
                        continue;
                    }
                    if mth != 2 && mth != d.mti && mth != d.mtc {
                        continue;
                    }
                    let jg = ng - ig + 1;
                    if jg < d.jgthrm {
                        continue;
                    }
                    let loca = lz + il + nl * ((jz - 1) + nz);
                    let locs = iptotl + itabl * (jg - 1);
                    if mth == 2 {
                        sig[s(locs)] -= at(loca);
                    }
                    if mth == d.mti || mth == d.mtc {
                        sig[s(locs)] += at(loca);
                    }
                    if mth == 2 {
                        sig[s(locs - 2)] -= at(loca);
                    }
                    if mth == d.mti || mth == d.mtc {
                        sig[s(locs - 2)] += at(loca);
                    }
                    continue;
                }
                if target == 300 {
                    if matches!(mth, 18 | 19 | 20 | 21 | 38) {
                        // label 340.
                        if il > 1 {
                            continue;
                        }
                        if mth == 18 {
                            i618 = 1;
                        }
                        if mth == 19 && i618 > 0 {
                            return Err(NjoyError::InvalidInput("dtfr: mt18 already processed, mt19 not allowed.".into()));
                        }
                        d.ids[d.ned as usize + 2] = 1;
                        if ig == 0 {
                            // label 360.
                            for k in 1..=ng {
                                let jg = ng - k + 1;
                                spect[s(jg)] = 0.0;
                                if k >= ig2lo && k <= ig2lo + ng2 - 1 {
                                    let loca = lz + k - ig2lo + 1;
                                    spect[s(jg)] = at(loca);
                                }
                            }
                            continue;
                        }
                        if ig2lo == 0 {
                            // label 380.
                            if d.kpos == 0 {
                                continue;
                            }
                            let jg = ng - ig + 1;
                            let loca = lz + il + nl * (jz - 1) + jz;
                            let locf = lz + il;
                            let locs = iptotl - 1 + itabl * (jg - 1);
                            let mut sss = at(loca);
                            if mth == 18 || mth == 19 {
                                sss *= ffis[s(jg)];
                            }
                            sig[s(locs)] += sss;
                            cnm += sss * at(locf);
                        } else {
                            let jg = ng - ig + 1;
                            for k in 2..=ng2 {
                                let loca = lz + il + nl * ((jz - 1) + nz * (k - 1));
                                let locs = iptotl - 1 + itabl * (jg - 1);
                                let mut sss = at(loca);
                                if mth == 18 || mth == 19 {
                                    sss *= ffis[s(jg)];
                                }
                                sig[s(locs)] += sss;
                                if d.kpos > 0 {
                                    let locf = lz + il + nl * (jz - 1);
                                    let jg2 = ng - ig2lo - k + 3;
                                    let locs = d.kpos + itabl * (jg2 - 1);
                                    sig[s(locs)] += at(locf) * sss;
                                    cnorm += at(locf) * sss;
                                    d.ids[d.ked as usize] = d.ked;
                                }
                            }
                        }
                        // label 390.
                        if ig >= ng && cnm != 0.0 {
                            for k in 1..=ng {
                                let locs = d.kpos + itabl * (k - 1);
                                sig[s(locs)] += cnm * spect[s(k)];
                                cnorm += cnm * spect[s(k)];
                                d.ids[d.ked as usize] = d.ked;
                            }
                        }
                        continue;
                    }
                    ilmax = il;
                    let jg = ng - ig + 1;
                    if mth == 2 && jg >= d.jgthrm {
                        continue;
                    }
                    if (200..=250).contains(&mth) {
                        if jg < d.jgthrm {
                            continue;
                        }
                        let ok = mth == d.mti || (mth == d.mtc && mth - d.mtc + 1 == il);
                        if !ok {
                            continue;
                        }
                    }
                    // label 305.
                    for k in 2..=ng2 {
                        let mut jg2 = ng - ig2lo - k + 3;
                        let mut jpos = d.ipingp + (jg2 - jg);
                        if jpos > itabl {
                            jg2 = jg2 - jpos + itabl;
                            jpos = itabl;
                        }
                        if jpos <= iptotl {
                            jg2 = jg2 + iptotl + 1 - jpos;
                            jpos = iptotl + 1;
                        }
                        if jpos <= itabl {
                            let loca = lz + il + nl * ((jz - 1) + nz * (k - 1));
                            let locs = jpos + itabl * (jg2 - 1);
                            sig[s(locs)] += at(loca);
                            if il <= 1 {
                                let locb = iptotl - 2 + itabl * (jg - 1);
                                sig[s(locb)] -= at(loca);
                            }
                        }
                    }
                    continue;
                }
                if target == 600 {
                    let jg = ng - ig + 1;
                    let mut ff = 1.0;
                    if mth == 102 {
                        ff = fcap[s(jg)];
                    }
                    if mth == 18 {
                        ff = ffis[s(jg)];
                    }
                    let ngp = d.ngp;
                    if ig == 0 {
                        for k in 1..=ngp {
                            let jgp = ngp - k + 1;
                            spect[s(jgp)] = 0.0;
                            if k >= ig2lo && k <= ig2lo + ng2 - 1 {
                                let loca = lz + k - ig2lo + 1;
                                spect[s(jgp)] = at(loca);
                            }
                        }
                    } else if ig2lo == 0 {
                        let loca = lz + ip + nl * ((jz - 1) + nz);
                        for k in 1..=ngp {
                            let locs = k + ngp * (jg - 1);
                            sig[s(locs)] += at(loca) * ff * spect[s(k)];
                        }
                    } else {
                        ipmax = ip;
                        for k in 2..=ng2 {
                            let jpos = ngp - ig2lo - k + 3;
                            if jpos >= 1 && jpos <= ngp {
                                let loca = lz + ip + nl * ((jz - 1) + nz * (k - 1));
                                let locs = jpos + ngp * (jg - 1);
                                sig[s(locs)] += at(loca) * ff;
                            }
                        }
                    }
                }
            }
            // label 800.
            if il == 1 && d.kpos != 0 && cnorm != 0.0 {
                for k in 1..=d.ng {
                    let locs = (d.kpos + d.itabl * (k - 1)) as usize;
                    sig[locs] /= cnorm;
                }
            }
            dtfout(&mut d, &mut sig, il, ip, ilmax, ipmax)?;
            if ip == 0 {
                il += 1;
            }
            if il > d.nlmax {
                il = 0;
            }
            if il == 0 {
                ip += 1;
            }
            if ip > d.nptabl {
                ip = 0;
            }
            if il == 0 && ip == 0 {
                break;
            }
        }
    }
    // label 900.
    d.plot.push_str(" 99/\n");
    Ok((d.out, d.plot))
}

/// `:241-270`: position the PENDF on the material at `dtemp`, which sets the
/// endf module's `iverf` from the PENDF.
fn pendf_temperature(d: &mut Dtfr) -> Result<(), NjoyError> {
    let p = d.pendf.as_ref().ok_or_else(|| NjoyError::InvalidInput("dtfr: no pendf tape".into()))?;
    let test = d.dtemp / 1000.0 + 1.0;
    for s in p.sections().iter().filter(|s| s.key.mat == d.matd && s.key.mf == 1 && s.key.mt == 451) {
        let rows = &s.rows;
        let r1 = rows.get(1).copied().unwrap_or([0.0; 6]);
        let iverf = if nint(r1[4]) != 0 {
            4
        } else if nint(r1[5]) == 0 {
            5
        } else {
            6
        };
        let nc = 1 + usize::from(iverf >= 5) + usize::from(iverf >= 6);
        let t = rows.get(nc).map_or(0.0, |r| r[0]);
        d.iverf = iverf;
        if (t - d.dtemp).abs() < test {
            return Ok(());
        }
    }
    Err(NjoyError::InvalidInput("dtfr: desired temperature not on pendf".into()))
}

/// `1p,eW.D` as gfortran writes it.
pub(crate) fn e1p(x: f64, d: usize, w: usize) -> String {
    crate::acer::fortran_fmt::fortran_e(x, d, w)
}

/// `dtfout` (`:769-946`).
fn dtfout(d: &mut Dtfr, sig: &mut [f64], il: i32, ip: i32, ilmax: i32, ipmax: i32) -> Result<(), NjoyError> {
    let ng = d.ng;
    let itabl = d.itabl;
    let line6 = |dat: &[f64; 6], d5: usize, w: usize| -> String { dat.iter().map(|&v| e1p(v, d5, w)).collect() };
    if d.iedit == 0 {
        let (nw, head) = if il != 0 {
            (itabl * ng, format!(" il= {} table{:3} gp{:3} pos, mat={:5} iz={:2} temp={}", il, ng, itabl, d.matd, d.jz, e1p(d.dtemp, 5, 12)))
        } else {
            (d.ngp * ng, format!(" ip= {} table{:3} gp{:3} pos, mat={:5} iz={:2} temp={}", ip, ng, d.ngp, d.matd, d.jz, e1p(d.dtemp, 5, 12)))
        };
        if d.nout > 0 {
            d.out.push('\n');
            d.out.push_str(&head);
            d.out.push('\n');
            let vals: Vec<f64> = (1..=nw as usize).map(|j| sig[j]).collect();
            for ch in vals.chunks(6) {
                let s: String = ch.iter().map(|&v| e1p(v, 4, 12)).collect();
                d.out.push_str(&s);
                d.out.push('\n');
            }
        }
    } else if il != 0 {
        if il <= 1 {
            let hdat = crate::deck::run::dater();
            let hdat = hdat.trim_start();
            if d.nout > 0 {
                d.out.push_str(&format!("{:<6.6}     edit xsec ({:3}x{:3}) proc by njoy   on {:<8.8}\n", d.hisnam, ng, d.ned, hdat));
            }
            let ifis = d.ids[d.ned as usize + 2];
            let nj = d.iptotl - 3;
            for j in 1..=nj {
                let mut take = j <= 7 || (ifis > 0 && j <= 11) || j >= nj - 1;
                if !take {
                    take = (1..=d.ned as usize).any(|i| d.jped[i] == j && d.ids[i] > 0);
                }
                if !take {
                    continue;
                }
                let mut jpos = j;
                if j == nj - 1 && ifis == 0 {
                    continue;
                }
                if j == nj - 1 {
                    jpos = d.iptotl - 1;
                }
                if j == nj {
                    jpos = d.iptotl;
                }
                let hmti = d.hednam[j as usize].clone();
                let mut iseq = 0;
                let mut k = 0usize;
                let mut dat = [0.0f64; 6];
                for ig in 1..=ng {
                    let locs = (jpos + itabl * (ig - 1)) as usize;
                    k += 1;
                    dat[k - 1] = sig[locs];
                    if k >= 6 || ig >= ng {
                        iseq += 1;
                        for v in dat.iter_mut().skip(k) {
                            *v = 0.0;
                        }
                        if d.nout > 0 {
                            d.out.push_str(&format!("{}{:<2.2}{:<5.5}{}\n", line6(&dat, 5, 12), d.hisnam, hmti, iseq % 10));
                        }
                        k = 0;
                    }
                }
            }
        }
        // label 200.
        let l = il - 1;
        if il <= ilmax {
            let med = d.iptotl - 3;
            let nws = ng * itabl;
            let mut iseq = 0;
            let ltabn = itabl - d.iptotl + 3;
            if d.nout > 0 {
                d.out.push_str(&format!("{:<6.6} l={} n-n table ({:3}x{:3})\n", d.hisnam, l, ltabn, ng));
            }
            let mut k = 0usize;
            let mut dat = [0.0f64; 6];
            for ig in 1..=ng {
                for jpos in 1..=itabl {
                    if jpos > med {
                        k += 1;
                        let locs = jpos + itabl * (ig - 1);
                        dat[k - 1] = sig[locs as usize];
                        if k >= 6 || locs >= nws {
                            iseq += 1;
                            for v in dat.iter_mut().skip(k) {
                                *v = 0.0;
                            }
                            if d.nout > 0 {
                                d.out.push_str(&format!("{}{:<2.2}{}{}\n", line6(&dat, 5, 12), d.hisnam, crate::viewr::fmt::i(i64::from(l), 2), crate::viewr::fmt::i(i64::from(iseq), 4)));
                            }
                            k = 0;
                        }
                    }
                }
            }
        }
    } else {
        // label 250.
        let l = ip - 1;
        if ip <= ipmax {
            let mut iseq = 0;
            if d.nout > 0 {
                d.out.push_str(&format!(" {:<6.6} l={}h n-p table ({:3}x{:3})\n", d.hisnam, l, d.ngp, ng));
            }
            let mut k = 0usize;
            let nws = ng * d.ngp;
            let mut dat = [0.0f64; 6];
            for ig in 1..=ng {
                for igp in 1..=d.ngp {
                    k += 1;
                    let locs = igp + d.ngp * (ig - 1);
                    dat[k - 1] = sig[locs as usize];
                    if k >= 6 || locs >= nws {
                        iseq += 1;
                        for v in dat.iter_mut().skip(k) {
                            *v = 0.0;
                        }
                        if d.nout > 0 {
                            d.out.push_str(&format!("{}{:<2.2}{}{}\n", line6(&dat, 5, 12), d.hisnam, crate::viewr::fmt::i(i64::from(l), 2), crate::viewr::fmt::i(i64::from(iseq), 4)));
                        }
                        k = 0;
                    }
                }
            }
        }
    }
    // label 300.
    if d.ifilm > 0 && il == 1 {
        crate::dtfr::plots::ploted(d, sig)?;
    }
    if d.ifilm > 0 && il == 1 && il <= ilmax {
        crate::dtfr::plots::plotnn(d, sig, il);
    }
    if d.ifilm > 0 && ip == 1 && ip <= ipmax {
        crate::dtfr::plots::plotnp(d, sig, ip);
    }
    Ok(())
}

/// DTFR on the deck's units: the GENDF on `nin`, the PENDF on `npend` (for the
/// point plots), the DTF tables to `nout` and the plot commands to `nplot`.
///
/// # Errors
/// Those of [`run_dtfr_tape`] and of the units.
pub fn run_dtfr(c: &DtfrCards, units: &mut crate::deck::tapes::TapeUnits) -> Result<(), NjoyError> {
    let [nin, nout, npend, nplot] = c.units;
    let g = units.gendf(nin)?.ok_or_else(|| NjoyError::InvalidInput(format!("dtfr: unit {nin} holds no gendf tape")))?;
    let pendf = if npend != 0 {
        units.openz(npend, 0)?;
        Some(units.get(npend)?.tape.clone())
    } else {
        None
    };
    let (out, plot) = run_dtfr_tape(c, &g, pendf.as_ref())?;
    if nout != 0 {
        units.openz(nout, 1)?;
        units.put_text(nout, out);
    }
    if nplot != 0 {
        units.openz(nplot, 1)?;
        units.put_text(nplot, plot);
    }
    Ok(())
}
