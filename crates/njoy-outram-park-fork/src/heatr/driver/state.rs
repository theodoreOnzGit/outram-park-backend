// Ported from NJOY2016 `src/heatr.f90` (module `heatm` globals, `horder`,
// `indx`, `df`).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! HEATR's module-level state, and the small routines that only touch it.
//!
//! `heatm`'s globals (`heatr.f90:1-46`) are the fields of [`Heatr`]. Each
//! routine's Fortran `save` variables are a struct of their own, held in
//! [`Heatr`], because upstream relies on them persisting between calls: a
//! routine is initialised with `e = 0` and then walked over ascending
//! energies. `df`'s saved constants are the sharpest case: every `df` call
//! uses the screening constants of the **last** `e = 0` call, whatever
//! arguments it is given itself, and this port keeps that.
//!
//! Index conventions: arrays upstream indexes as scalars (`mtp`, `c`,
//! `elist`, `c458`, `mt6no`, ...) are 1-based here with element 0 unused, so
//! the translation reads like the Fortran; flat ENDF records are 0-based
//! slices (see [`super::flat`]).

use crate::endf::tape::Tape;

/// `nbuf`, `ilmax`, `nqamax`, `maxmf6` (`heatr.f90:13-17`).
pub(super) const ILMAX: usize = 200;
pub(super) const NQAMAX: usize = 30;
pub(super) const MAXMF6: usize = 320;

/// `disbar`'s saved variables (`heatr.f90:1916-1917`).
#[derive(Debug, Clone, Default)]
pub(super) struct DisbarState {
    pub imiss: i32,
    pub thresh: f64,
    pub awp: f64,
    pub afact: f64,
    pub arat: f64,
    pub en: f64,
    pub cn: f64,
    pub el: f64,
    pub cl: f64,
    pub z: f64,
    pub daml: f64,
    pub damn: f64,
    pub enx: f64,
    pub enext: f64,
}

/// `capdam`'s saved variables (`:1763-1764`).
#[derive(Debug, Clone, Default)]
pub(super) struct CapdamState {
    pub en: f64,
    pub damn: f64,
    pub el: f64,
    pub daml: f64,
    pub zx: f64,
    pub ax: f64,
    pub denom: f64,
    pub z: f64,
    pub aw1fac: f64,
}

/// `conbar`'s saved variables (`:2077-2078`) and its work array `c`, which
/// holds the MF=5 section from initialisation on.
#[derive(Debug, Clone, Default)]
pub(super) struct ConbarState {
    pub nktot: usize,
    pub awr: f64,
    pub awfac: f64,
    pub aw1fac: f64,
    pub z: f64,
    pub za: f64,
    pub mtt: i32,
    pub loc: Vec<usize>,
    pub d1: Vec<f64>,
    pub d2: Vec<f64>,
    pub e1: Vec<f64>,
    pub e2: Vec<f64>,
    pub mf1: i32,
    pub mt1: i32,
    /// `conbar`'s `c`, 1-based (element 0 unused).
    pub c: Vec<f64>,
    /// `t1`, `t2` from the last LF=3 subsection at initialisation. Upstream
    /// does not save them, so a later `anabar` call sees whatever the last
    /// assignment left in those stack slots; holding the value here is the
    /// defined reading of that.
    pub t1: f64,
    pub t2: f64,
}

/// `hgtyld`'s saved variables (`:2318`) and its data array.
#[derive(Debug, Clone, Default)]
pub(super) struct HgtyldState {
    pub lnu: i32,
    pub ip: usize,
    pub ir: usize,
    /// The TAB1 (or polynomial LIST) read at initialisation, flat.
    pub a: Vec<f64>,
}

/// One MF=6 subsection, read once and held, with its data records in the
/// order upstream reads them off `nend6`.
#[derive(Debug, Clone, Default)]
pub(super) struct Mf6Sub {
    /// The yield TAB1, flat (upstream's `c(1)` after `tab1io`).
    pub yld: Vec<f64>,
    /// `LAW`.
    pub law: i32,
    /// LAW=1, 2, 5: the TAB2 that precedes the LISTs. LAW=7: its outer TAB2.
    pub tab2: Vec<f64>,
    /// LAW=1, 2, 5: one LIST per incident energy, flat. LAW=7: per incident
    /// energy, the inner TAB2 followed by its `NMU` TAB1s, concatenated, as
    /// upstream reads them into `c(iraw)`. LAW=6: the one CONT.
    pub recs: Vec<Vec<f64>>,
}

/// An MF=6 section: its HEAD and subsections.
#[derive(Debug, Clone, Default)]
pub(super) struct Mf6Section {
    pub head: Vec<f64>,
    pub subs: Vec<Mf6Sub>,
}

/// `sixbar`'s saved variables (`:2780-2782`) and the position it reads
/// `nend6` from.
#[derive(Debug, Clone, Default)]
pub(super) struct SixbarState {
    pub nne: usize,
    pub ne: usize,
    pub law: i32,
    pub lang: i32,
    pub lep: i32,
    pub intl: i64,
    pub elo: f64,
    pub ehi: f64,
    pub flo: f64,
    pub fhi: f64,
    pub dlo: f64,
    pub dhi: f64,
    pub disc102: f64,
    pub zp: f64,
    pub ap: f64,
    pub zt: f64,
    pub at: f64,
    /// The section being read.
    pub sec: Mf6Section,
    /// The 0-based index of the next subsection `tab1io` would read.
    pub pos: usize,
    /// The subsection currently loaded.
    pub cur: usize,
    /// Upstream's `c`: the current subsection's yield TAB1 at `c(1)`.
    pub c_yld: Vec<f64>,
    /// Upstream's `c(iraw)`: the record of the current high-energy point.
    pub c_raw: Vec<f64>,
    /// Records of the current subsection read so far.
    pub nread: usize,
}

/// `h6cm`'s saved variables (`:3471`).
#[derive(Debug, Clone, Default)]
pub(super) struct H6cmState {
    pub na: usize,
    pub eps: f64,
    pub xc: f64,
    pub ndnow: usize,
    pub npnow: usize,
    pub ncnow: usize,
    pub elmax: f64,
    pub e: f64,
    pub epmax: f64,
}

/// `h6ddx`'s saved variables (`:3714`).
#[derive(Debug, Clone, Default)]
pub(super) struct H6ddxState {
    pub efirst: f64,
    pub enow: f64,
    pub nl: usize,
    pub inow: usize,
    pub mnow: usize,
    pub lnow: usize,
    pub ncnow: usize,
    pub na: usize,
    pub illdef: i32,
}

/// `h6dis`'s saved variables (`:3893`).
#[derive(Debug, Clone, Default)]
pub(super) struct H6disState {
    pub nl: usize,
    pub inow: usize,
    pub lnow: usize,
    pub mnow: usize,
    pub ncnow: usize,
    pub na: usize,
    pub enow: f64,
}

/// `h6psp`'s saved variables (`:4097`).
#[derive(Debug, Clone, Default)]
pub(super) struct H6pspState {
    pub cn: f64,
    pub ex: f64,
    pub eimax: f64,
}

/// `hgtfle`'s saved variables (`:4234-4236`) and its raw-data position.
#[derive(Debug, Clone)]
pub(super) struct HgtfleState {
    pub iso: i32,
    pub nne: usize,
    pub ne: usize,
    pub inn: i64,
    pub ir: usize,
    pub elo: f64,
    pub ehi: f64,
    pub nlo: usize,
    pub nhi: usize,
    pub flo: [f64; 66],
    pub fhi: [f64; 66],
    pub ltt: i32,
    pub ltt3: i32,
    pub lttn: i32,
    pub lct: i32,
    pub awr: f64,
    /// The TAB2 (`c(1)`), flat.
    pub tab2: Vec<f64>,
    /// The MF=4 section's records after the current TAB2, in reading order.
    pub recs: Vec<Vec<f64>>,
    /// Index into `recs` of the next record to read.
    pub next: usize,
    /// For LTT=3: the second TAB2 and its records.
    pub tab2b: Vec<f64>,
    pub recsb: Vec<Vec<f64>>,
}

impl Default for HgtfleState {
    fn default() -> Self {
        HgtfleState {
            iso: 0,
            nne: 0,
            ne: 0,
            inn: 0,
            ir: 0,
            elo: 0.0,
            ehi: 0.0,
            nlo: 0,
            nhi: 0,
            flo: [0.0; 66],
            fhi: [0.0; 66],
            ltt: 0,
            ltt3: 0,
            lttn: 0,
            lct: 0,
            awr: 0.0,
            tab2: Vec::new(),
            recs: Vec::new(),
            next: 0,
            tab2b: Vec::new(),
            recsb: Vec::new(),
        }
    }
}

/// `gambar`'s saved variables (`:5468-5470`).
#[derive(Debug, Clone, Default)]
pub(super) struct GambarState {
    pub lf: i64,
    pub z: f64,
    pub awr: f64,
    pub ne: usize,
    pub nne: usize,
    pub nbt: usize,
    pub inn: i64,
    pub nnt: usize,
    pub ilo: i32,
    pub ihi: i32,
    pub elo: f64,
    pub ehi: f64,
    pub flo: f64,
    pub fhi: f64,
    pub glo: f64,
    pub ghi: f64,
    pub hlo: f64,
    pub hhi: f64,
    pub tab2: Vec<f64>,
    pub recs: Vec<Vec<f64>>,
    pub next: usize,
    pub lo_rec: Vec<f64>,
    pub hi_rec: Vec<f64>,
}

/// HEATR's state for one material and temperature (`heatm`'s globals).
#[derive(Debug)]
pub(super) struct Heatr {
    // input
    pub matd: i32,
    pub npk: usize,
    /// `mtp(1..=npk)`: 0, 301, then the partial MTs in ascending order.
    pub mtp: Vec<i32>,
    pub nqa: usize,
    pub mta: Vec<i32>,
    pub qa: Vec<f64>,
    /// `lqs(i)`: 1-based start in `qbar` of reaction `i`'s TAB1, 0 for none.
    pub lqs: Vec<usize>,
    /// The user's energy-dependent Q TAB1s, concatenated, 1-based.
    pub qbar: Vec<f64>,
    pub kchk: i32,
    pub iprint: i32,
    pub local: i32,
    /// `break`: the displacement energy (0 until defaulted).
    pub brk: f64,

    // dictionary and material
    pub iverf: i32,
    pub mgam: i32,
    pub mt303: usize,
    pub mt19: i32,
    pub ne: usize,
    pub efirst: f64,
    pub elast: f64,
    pub za: f64,
    pub awr: f64,
    pub elist: Vec<f64>,
    pub qdel: f64,
    pub etabmax: f64,
    pub mt103: i32,
    pub mt104: i32,
    pub mt105: i32,
    pub mt106: i32,
    pub mt107: i32,
    pub mt16: i32,
    pub miss4: Vec<i32>,
    pub mt6: Vec<i32>,
    pub i6g: usize,
    pub mt6no: Vec<usize>,
    pub mt6yp: Vec<i32>,
    pub jp: i32,
    pub jpn: i32,
    pub jpp: i32,
    pub q: f64,
    pub zat: f64,
    pub awrt: f64,
    pub zap: f64,
    pub awp: f64,
    pub lct: i32,
    pub idame: i32,
    pub ebot: f64,
    pub etop: f64,
    pub mt458: i32,
    pub nply: usize,
    pub lfc: i32,
    pub ifc1: i32,
    pub ifc2: i32,
    pub ifc3: i32,
    pub ifc4: i32,
    pub ifc5: i32,
    pub ifc6: i32,
    pub c458: Vec<f64>,
    pub cpoly: Vec<f64>,
    pub hpoly: Vec<f64>,
    pub afr: Vec<f64>,
    pub anp: Vec<f64>,
    pub agp: Vec<f64>,
    pub emc2: f64,
    pub tm: f64,
    pub rtm: f64,
    pub izat: i64,
    pub izap: i64,

    // df's saved constants
    pub df_rel: f64,
    pub df_fl: f64,

    // routine states
    pub dis: DisbarState,
    pub cap: CapdamState,
    pub con: ConbarState,
    pub hyl: HgtyldState,
    pub six: SixbarState,
    pub h6c: H6cmState,
    pub h6d: H6ddxState,
    pub h6s: H6disState,
    pub psp: H6pspState,
    pub fle: HgtfleState,
    pub gam: GambarState,

    /// `nscr` / `nend4`: the material after `hconvr`.
    pub conv: Tape,
    /// `nend6`: the MF=6 sections `hinit` copied.
    pub mf6: Vec<(i32, Mf6Section)>,

    /// `iold`: the kerma table, `rows[nn][1..=npkk]`, `c(1)` the energy.
    pub rows: Vec<Vec<f64>>,

    /// Upstream's `nsyso` listing.
    pub listing: String,
}

impl Heatr {
    pub(super) fn new(conv: Tape) -> Self {
        Heatr {
            matd: 0,
            npk: 0,
            mtp: vec![0; 31],
            nqa: 0,
            mta: vec![0; NQAMAX + 1],
            qa: vec![0.0; NQAMAX + 1],
            lqs: vec![0; NQAMAX + 1],
            qbar: vec![0.0],
            kchk: 0,
            iprint: 0,
            local: 0,
            brk: 0.0,
            iverf: 6,
            mgam: 0,
            mt303: 0,
            mt19: 0,
            ne: 0,
            efirst: 0.0,
            elast: 0.0,
            za: 0.0,
            awr: 0.0,
            elist: vec![0.0; ILMAX + 2],
            qdel: 0.0,
            etabmax: -1.0,
            mt103: 0,
            mt104: 0,
            mt105: 0,
            mt106: 0,
            mt107: 0,
            mt16: 0,
            miss4: vec![0],
            mt6: vec![0],
            i6g: 0,
            mt6no: vec![0],
            mt6yp: vec![0],
            jp: 0,
            jpn: 0,
            jpp: 0,
            q: 0.0,
            zat: 0.0,
            awrt: 0.0,
            zap: 0.0,
            awp: 0.0,
            lct: 0,
            idame: 0,
            ebot: 1.0e-5,
            etop: 2.0e7,
            mt458: 0,
            nply: 0,
            lfc: 0,
            ifc1: 0,
            ifc2: 0,
            ifc3: 0,
            ifc4: 0,
            ifc5: 0,
            ifc6: 0,
            c458: Vec::new(),
            cpoly: Vec::new(),
            hpoly: Vec::new(),
            afr: Vec::new(),
            anp: Vec::new(),
            agp: Vec::new(),
            emc2: 0.0,
            tm: 0.0,
            rtm: 0.0,
            izat: 0,
            izap: 0,
            df_rel: 0.0,
            df_fl: 0.0,
            dis: DisbarState::default(),
            cap: CapdamState::default(),
            con: ConbarState::default(),
            hyl: HgtyldState::default(),
            six: SixbarState::default(),
            h6c: H6cmState::default(),
            h6d: H6ddxState::default(),
            h6s: H6disState::default(),
            psp: H6pspState::default(),
            fle: HgtfleState::default(),
            gam: GambarState::default(),
            conv,
            mf6: Vec::new(),
            rows: Vec::new(),
            listing: String::new(),
        }
    }

    /// `i6`: the number of MF=6 reactions.
    pub(super) fn i6(&self) -> usize {
        self.mt6.len() - 1
    }

    /// `npkk`: the number of columns in a kerma row, as `nheat`, `gheat`
    /// and `hout` each compute it (`heatr.f90:1047-1050`).
    pub(super) fn npkk(&self) -> usize {
        let mut npkk = self.npk;
        if self.kchk == 1 {
            npkk = 3 * self.npk - 2;
        }
        if self.mgam > 0 {
            npkk += 3;
        }
        if self.mgam == 0 && self.i6() > 0 {
            npkk += 1;
        }
        npkk
    }

    /// `mess` (`util.f90`): a message on the listing.
    pub(super) fn mess(&mut self, from: &str, l1: &str, l2: &str) {
        self.listing
            .push_str(&format!("\n ---message from {from}---{l1}\n"));
        if !l2.trim().is_empty() {
            self.listing.push_str(&format!("{:26}{l2}\n", ""));
        }
    }

    /// `df` (`heatr.f90:2015-2053`): the Lindhard damage partition. A call
    /// with `e <= 0` (and `zr != 0`) sets the saved screening constants from
    /// its arguments; every other call uses the saved constants.
    pub(super) fn df(&mut self, e: f64, zr: f64, ar: f64, zl: f64, al: f64) -> f64 {
        const TWOTHD: f64 = 0.666666667;
        const THREEQ: f64 = 0.75;
        const SIXTH: f64 = 0.166666667;
        const ONEP5: f64 = 1.5;
        const C1: f64 = 30.724;
        const C2: f64 = 0.0793;
        const C3: f64 = 3.4008;
        const C4: f64 = 0.40244;
        if zr == 0.0 {
            0.0
        } else if e <= 0.0 {
            let el = C1 * zr * zl * (zr.powf(TWOTHD) + zl.powf(TWOTHD)).sqrt() * (ar + al) / al;
            self.df_rel = 1.0 / el;
            let denom =
                (zr.powf(TWOTHD) + zl.powf(TWOTHD)).powf(THREEQ) * ar.powf(ONEP5) * al.sqrt();
            self.df_fl = C2 * zr.powf(TWOTHD) * zl.sqrt() * (ar + al).powf(ONEP5) / denom;
            0.0
        } else if e < self.brk {
            0.0
        } else {
            let ep = e * self.df_rel;
            e / (1.0 + self.df_fl * (C3 * ep.powf(SIXTH) + C4 * ep.powf(THREEQ) + ep))
        }
    }
}

/// `horder` (`heatr.f90:416-436`): ascending order, by upstream's exchange
/// sort, over the 1-based `iarray(1..=n)`.
pub(super) fn horder(iarray: &mut [i32], n: usize) {
    for i in 1..=n {
        for j in i..=n {
            if iarray[i] > iarray[j] {
                iarray.swap(i, j);
            }
        }
    }
}

/// `indx` (`heatr.f90:1666-1740`): the 1-based positions in `mtp(3..=npk)`
/// that reaction `mt` contributes to.
pub(super) fn indx(npk: usize, mtp: &[i32], mt19: i32, mt: i32, iverf: i32) -> Vec<usize> {
    let mut imt = Vec::new();
    if npk < 3 {
        return imt;
    }
    for (i, &mtpi) in mtp.iter().enumerate().take(npk + 1).skip(3) {
        let mut iflag = matches!(mtpi, 442 | 443 | 444)
            || (mtpi == 445 && mt == 2)
            || (mtpi == 446 && (51..=91).contains(&mt))
            || (mtpi == 447 && (102..=120).contains(&mt));
        if iverf < 6 {
            iflag |= mtpi == 447 && (700..=799).contains(&mt);
        } else {
            iflag |= mtpi == 447 && (600..=849).contains(&mt);
        }
        iflag |= mtpi == 303 && mt != 2 && mt != 3;
        iflag |= mtpi == 304 && (51..=91).contains(&mt);
        if mtpi == 318 {
            iflag |= mt19 == 0 && mt == 18;
            iflag |= mt19 == 1 && matches!(mt, 19 | 20 | 21 | 38);
            iflag |= mt19 == 2 && mt == 18;
        }
        if mtpi == 401 {
            iflag |= (102..=120).contains(&mt);
            iflag |= iverf < 6 && (700..800).contains(&mt);
            iflag |= iverf >= 6 && (600..850).contains(&mt);
        }
        if iverf < 6 {
            iflag |= mtpi == 403 && (700..=719).contains(&mt);
            iflag |= mtpi == 404 && (720..=739).contains(&mt);
            iflag |= mtpi == 405 && (740..=759).contains(&mt);
            iflag |= mtpi == 406 && (760..=779).contains(&mt);
            iflag |= mtpi == 407 && (780..=799).contains(&mt);
        } else {
            iflag |= mtpi == 403 && (600..=649).contains(&mt);
            iflag |= mtpi == 404 && (650..=699).contains(&mt);
            iflag |= mtpi == 405 && (700..=749).contains(&mt);
            iflag |= mtpi == 406 && (750..=799).contains(&mt);
            iflag |= mtpi == 407 && (800..=849).contains(&mt);
        }
        iflag |= mtpi == mt + 300;
        if iflag {
            imt.push(i);
        }
    }
    imt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn horder_sorts_the_one_based_slice() {
        let mut a = vec![0, 0, 301, 446, 442, 444];
        horder(&mut a, 5);
        assert_eq!(a, vec![0, 0, 301, 442, 444, 446]);
    }

    #[test]
    fn indx_routes_partials_as_upstream() {
        let mtp = vec![0, 0, 301, 302, 304, 442, 444, 445, 446];
        assert_eq!(indx(8, &mtp, 0, 2, 6), vec![3, 5, 6, 7]);
        assert_eq!(indx(8, &mtp, 0, 51, 6), vec![4, 5, 6, 8]);
        assert_eq!(indx(8, &mtp, 0, 102, 6), vec![5, 6]);
    }

    #[test]
    fn df_uses_the_constants_of_the_last_initialising_call() {
        let mut h = Heatr::new(Tape::from_sections(String::new(), Vec::new()));
        h.brk = 25.0;
        h.df(0.0, 26.0, 57.4, 26.0, 57.4);
        let a = h.df(1.0e4, 26.0, 57.4, 26.0, 57.4);
        // Different arguments, same saved constants: same answer.
        let b = h.df(1.0e4, 1.0, 1.0, 1.0, 1.0);
        assert_eq!(a, b);
        assert!(a > 0.0 && a < 1.0e4);
        assert_eq!(h.df(10.0, 26.0, 57.4, 26.0, 57.4), 0.0);
    }
}
