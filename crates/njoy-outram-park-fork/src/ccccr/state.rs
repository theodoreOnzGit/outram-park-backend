// SPDX-License-Identifier: AGPL-3.0-only
// Translated from NJOY2016.80 `src/ccccr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - module `ccccm`'s variables, l.17-71; the input of `subroutine ccccr`,
//     l.199-226; `ruinis`, l.461-506; `ruinbr`, l.1974-1987.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! CCCCR's cards and its module variables (`ccccm`), as one owned struct.

use crate::ccccr::core::{Core, Cursor, Hdr, Scratch, MAXE};

/// One isotope's card 5 of `ccccr` (`ccccr.f90:221-224`).
#[derive(Debug, Clone, PartialEq)]
pub struct IsotopeCard {
    /// `hisonm`, `habsid`, `hident`, `hmat` (`character(6)`, blank-padded).
    pub names: [String; 4],
    /// `imat`: the ENDF MAT number.
    pub imat: i32,
    /// `xspot`: the potential scattering cross section for BRKOXS (barn).
    pub xspo: f64,
}

/// One isotope's card 4 of `cisotx` (`ruinis`, `ccccr.f90:500-503`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IsotxsIsotope {
    /// `kbr`, the isotope classification.
    pub kbr: i32,
    /// `amass` (gram atomic weight), `efiss`, `ecapt` (J per event),
    /// `temp` (K), `sigpot` (barn), `adens` (atom density).
    pub values: [f64; 6],
}

/// `cisotx`'s cards (`ruinis`).
#[derive(Debug, Clone, PartialEq)]
pub struct IsotxsCards {
    /// `nsblok`, `maxup`, `maxdn`, `ichix`.
    pub control: [i32; 4],
    /// `spec(1:ngroup)` when `ichix > 0`.
    pub spec: Vec<f64>,
    /// Card 4, one per isotope.
    pub isotopes: Vec<IsotxsIsotope>,
}

/// `cbrkxs`'s cards (`ruinbr`).
#[derive(Debug, Clone, PartialEq)]
pub struct BrkoxsCards {
    /// `nti`, `nzi` (negative: accept the first `|n|`).
    pub nti: i32,
    /// See `nti`.
    pub nzi: i32,
    /// `atem(1:nti)` (K), when `nti > 0`.
    pub atem: Vec<f64>,
    /// `asig(1:nzi)` (barn), when `nzi > 0`.
    pub asig: Vec<f64>,
}

/// Every card of one CCCCR run, in upstream's read order.
#[derive(Debug, Clone, PartialEq)]
pub struct CcccrCards {
    /// Card 1: `nin`, `nisot`, `nbrks`, `ndlay`.
    pub units: [i32; 4],
    /// Card 2: `lprint`, `ivers`.
    pub lprint: i32,
    /// See `lprint`.
    pub ivers: i32,
    /// Card 2's `huse` (12 characters).
    pub huse: String,
    /// Card 3: `hsetid` (72 characters).
    pub hsetid: String,
    /// Card 4: `ngroup`, `nggrup`, `niso`, `maxord`, `ifopt`.
    pub control: [i32; 5],
    /// Card 5, one per isotope.
    pub isotopes: Vec<IsotopeCard>,
    /// `cisotx`'s cards, when `nisot /= 0`.
    pub isotxs: Option<IsotxsCards>,
    /// `cbrkxs`'s cards, when `nbrks /= 0`.
    pub brkoxs: Option<BrkoxsCards>,
}

/// Module `ccccm`'s state for one run, plus the GENDF and scratch units.
#[derive(Debug, Clone)]
pub struct State {
    /// The equivalenced container.
    pub core: Core,
    /// `e(maxe)`.
    pub e: Vec<f64>,
    /// ENDF header variables.
    pub h: Hdr,
    /// `nin`.
    pub nin: Cursor,
    /// `nscrt1`.
    pub scr1: Cursor,
    /// `nscrt2`.
    pub scr2: Scratch,
    /// `nscrt3`.
    pub scr3: Scratch,
    /// `huse(1:2)` (`tuse` read `2a6`).
    pub huse: [[u8; 8]; 2],
    /// `ivers`.
    pub ivers: i64,
    /// `hsetid(1:12)` (`character(6)`).
    pub hsetid: Vec<String>,
    /// `ngps`, `nggrup`, `niso`, `maxord`, `ifopt`, `nsblk`, `nscmax`, `ichix`.
    pub ngps: i64,
    /// See `ngps`.
    pub niso: i64,
    /// See `ngps`.
    pub maxord: i64,
    /// See `ngps`.
    pub ifopt: i64,
    /// See `ngps`.
    pub nsblk: i64,
    /// See `ngps`.
    pub nscmax: i64,
    /// See `ngps`.
    pub ichix: i64,
    /// `imat(200)`, `kbr(200)` (1-based; index 0 unused).
    pub imat: Vec<i64>,
    /// See `imat`.
    pub kbr: Vec<i64>,
    /// `hisonm`, `habsid`, `hident`, `hmat` (1-based).
    pub hisonm: Vec<String>,
    /// See `hisonm`.
    pub habsid: Vec<String>,
    /// See `hisonm`.
    pub hident: Vec<String>,
    /// See `hisonm`.
    pub hmat: Vec<String>,
    /// `amass`, `efiss`, `ecapt`, `temp`, `sigpot`, `adens` (1-based).
    pub isodata: Vec<[f64; 6]>,
    /// `iflags(15)` (1-based).
    pub iflags: [i64; 16],
    /// `ltot`, `ltrn`.
    pub ltot: i64,
    /// See `ltot`.
    pub ltrn: i64,
    /// `irsize`, `ng2z`, `jlz`, `lord1`, `nsiza`.
    pub irsize: i64,
    /// See `irsize`.
    pub ng2z: i64,
    /// See `irsize`.
    pub jlz: i64,
    /// See `irsize`.
    pub lord1: i64,
    /// See `irsize`.
    pub nsiza: i64,
    /// `nti`, `nzi`, `nreact`, `nzt`, `ntj`, `nzj`.
    pub nti: i64,
    /// See `nti`.
    pub nzi: i64,
    /// See `nti`.
    pub nreact: i64,
    /// See `nti`.
    pub ntj: i64,
    /// See `nti`.
    pub nzj: i64,
    /// `atem`, `ctem`, `ntat`, `item`, `asig`, `csig`, `ntap`, `isig` (all
    /// `real(kr)` upstream, 1-based, 20 entries).
    pub atem: [f64; 21],
    /// See `atem`.
    pub ctem: [f64; 21],
    /// See `atem`.
    pub ntat: [f64; 21],
    /// See `atem`.
    pub item: [f64; 21],
    /// See `atem`.
    pub asig: [f64; 21],
    /// See `atem`.
    pub csig: [f64; 21],
    /// See `atem`.
    pub ntap: [f64; 21],
    /// See `atem`.
    pub isig: [f64; 21],
    /// `xspo`, `xspot(200)`, `tzro`.
    pub xspo: f64,
    /// See `xspo`.
    pub xspot: Vec<f64>,
    /// See `xspo`.
    pub tzro: f64,
    /// `next`, `isopec`, `l1`, `l2`, `l13`.
    pub next: i64,
    /// See `next`.
    pub isopec: i64,
    /// See `next`.
    pub l1: i64,
    /// See `next`.
    pub l2: i64,
    /// See `next`.
    pub l13: i64,
    /// `jbl`, `jbh`, `mt2tem(10)`, `ntfl`, `matd`, `nsb`, `lrsize`.
    pub jbl: i64,
    /// See `jbl`.
    pub jbh: i64,
    /// See `jbl`.
    pub mt2tem: [i64; 11],
    /// See `jbl`.
    pub ntfl: i64,
    /// See `jbl`.
    pub nsb: i64,
    /// See `jbl`.
    pub lrsize: i64,
    /// `nisod`, `nfam`.
    pub nisod: i64,
    /// See `nisod`.
    pub nfam: i64,
    /// Messages upstream writes with `mess`.
    pub notes: Vec<String>,
}

impl State {
    /// The state after `ccccr`'s own input (`ccccr.f90:187-224`); `next = 1`.
    pub fn new(c: &CcccrCards, nin: Cursor) -> Self {
        let word = format!("{:<12}", c.huse);
        let huse = [crate::ccccr::core::text8(&word[0..6]), crate::ccccr::core::text8(&word[6..12])];
        let text = format!("{:<72}", c.hsetid);
        let hsetid = (0..12).map(|i| text[6 * i..6 * i + 6].to_string()).collect();
        let mut s = State {
            core: Core::new(),
            e: vec![0.0; MAXE],
            h: Hdr::default(),
            nin,
            scr1: Cursor::default(),
            scr2: Scratch::default(),
            scr3: Scratch::default(),
            huse,
            ivers: i64::from(c.ivers),
            hsetid,
            ngps: i64::from(c.control[0]),
            niso: i64::from(c.control[2]),
            maxord: i64::from(c.control[3]),
            ifopt: i64::from(c.control[4]),
            nsblk: 0,
            nscmax: 0,
            ichix: 0,
            imat: vec![0; 201],
            kbr: vec![0; 201],
            hisonm: vec![String::new(); 201],
            habsid: vec![String::new(); 201],
            hident: vec![String::new(); 201],
            hmat: vec![String::new(); 201],
            isodata: vec![[0.0; 6]; 201],
            iflags: [0; 16],
            ltot: 0,
            ltrn: 0,
            irsize: 0,
            ng2z: 0,
            jlz: 0,
            lord1: 0,
            nsiza: 0,
            nti: 0,
            nzi: 0,
            nreact: 0,
            ntj: 0,
            nzj: 0,
            atem: [0.0; 21],
            ctem: [0.0; 21],
            ntat: [0.0; 21],
            item: [0.0; 21],
            asig: [0.0; 21],
            csig: [0.0; 21],
            ntap: [0.0; 21],
            isig: [0.0; 21],
            xspo: 0.0,
            xspot: vec![0.0; 201],
            tzro: 0.0,
            next: 1,
            isopec: 0,
            l1: 0,
            l2: 0,
            l13: 0,
            jbl: 0,
            jbh: 0,
            mt2tem: [0; 11],
            ntfl: 0,
            nsb: 0,
            lrsize: 0,
            nisod: 0,
            nfam: 0,
            notes: Vec::new(),
        };
        for (k, iso) in c.isotopes.iter().enumerate().take(200) {
            let i = k + 1;
            s.hisonm[i] = iso.names[0].clone();
            s.habsid[i] = iso.names[1].clone();
            s.hident[i] = iso.names[2].clone();
            s.hmat[i] = iso.names[3].clone();
            s.imat[i] = i64::from(iso.imat);
            s.xspot[i] = iso.xspo;
        }
        s
    }

    /// `e(i)`, 1-based (0 outside the array, which only a malformed tape reaches).
    pub fn e(&self, i: i64) -> f64 {
        if i >= 1 { self.e.get((i - 1) as usize).copied().unwrap_or(0.0) } else { 0.0 }
    }

    /// `contio(nin, ...)` on `nin`.
    pub fn contio_nin(&mut self) -> Result<(), crate::NjoyError> {
        self.nin.contio(&mut self.h, &mut self.e)
    }

    /// `listio(nin, ...)` with its `moreio` loop.
    pub fn listio_nin(&mut self) -> Result<(), crate::NjoyError> {
        self.nin.listio(&mut self.h, &mut self.e)
    }

    /// `contio(nscrt1, ...)`.
    pub fn contio_scr1(&mut self) -> Result<(), crate::NjoyError> {
        self.scr1.contio(&mut self.h, &mut self.e)
    }

    /// `listio(nscrt1, ...)` with its `moreio` loop.
    pub fn listio_scr1(&mut self) -> Result<(), crate::NjoyError> {
        self.scr1.listio(&mut self.h, &mut self.e)
    }
}
