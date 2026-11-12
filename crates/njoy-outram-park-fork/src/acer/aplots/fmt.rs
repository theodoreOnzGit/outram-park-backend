// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da; the 2016.80 line numbers
// after 7547 are 3 higher) and `src/acecm.f90`: ACER's plot-file writer `aplots` and its callees.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible; this derivative file is
// distributed under AGPL-3.0-only. This is a modified, non-LANL version, not endorsed by LANL/DOE.
// See crate root LICENSE.njoy + NOTICE.

//! The text the plot file is made of: Fortran `E`, `I` and `F` edit
//! descriptors, the `viewr` page header, and the 1-based `xss` accessor.
//!
//! Every line `aplots` writes is one of a handful of shapes, each with a trap
//! Rust's `{}` does not have:
//!
//! | descriptor | used for | trap |
//! |---|---|---|
//! | `e12.4` (scale 0) | the `xtag ytag` label position | mantissa `0.dddd`, not `d.ddd` |
//! | `1p,e12.3`, `1p,e14.6`, `1p,e10.3` | axis limits, curve points | one digit before the point |
//! | exponent of 3 digits | tiny/huge values | Fortran drops the `E` |
//! | `a` of `hk(1:it)` | the page title | `it` is the last non-blank of 70 |

/// Fortran `Ew.d` (`scale1 = false`) or `1pEw.d` (`scale1 = true`),
/// right-justified in `w` columns. A value that does not fit gives asterisks,
/// as Fortran does.
pub fn fe(x: f64, w: usize, d: usize, scale1: bool) -> String {
    if x.is_nan() {
        return format!("{:>w$}", "NaN");
    }
    if x.is_infinite() {
        return format!("{:>w$}", if x < 0.0 { "-Infinity" } else { "Infinity" });
    }
    let neg = x.is_sign_negative();
    let (mant, exp) = if x == 0.0 {
        // `0.000E+00` (scale 1) or `0.0000E+00` (scale 0).
        let digits = if scale1 { "0".repeat(d + 1) } else { "0".repeat(d) };
        (digits, 0i32)
    } else {
        let sig = if scale1 { d + 1 } else { d };
        let s = format!("{:.*E}", sig - 1, x.abs());
        let (m, e) = s.split_once('E').unwrap_or((s.as_str(), "0"));
        let e: i32 = e.parse().unwrap_or(0);
        (m.replace('.', ""), if scale1 { e } else { e + 1 })
    };
    let mant = if scale1 { format!("{}.{}", &mant[..1], &mant[1..]) } else { format!("0.{mant}") };
    let sign = if neg { "-" } else { "" };
    let body = if exp.abs() >= 100 {
        format!("{sign}{mant}{}{}", if exp < 0 { "-" } else { "+" }, exp.abs())
    } else {
        format!("{sign}{mant}E{}{:02}", if exp < 0 { "-" } else { "+" }, exp.abs())
    };
    if body.len() > w {
        // Fortran drops the leading zero of `0.ddd` before giving up.
        if !scale1 && body.len() - 1 <= w {
            let t = if neg { format!("-{}", &body[2..]) } else { body[1..].to_string() };
            return format!("{t:>w$}");
        }
        return "*".repeat(w);
    }
    format!("{body:>w$}")
}

/// `i<w>`.
pub fn fi(i: i64, w: usize) -> String {
    let s = format!("{i}");
    if s.len() > w {
        "*".repeat(w)
    } else {
        format!("{s:>w$}")
    }
}

/// Fortran `nint`: round half away from zero.
pub fn nint(x: f64) -> i64 {
    x.round() as i64
}

/// `character(70)` padded or cut to 70 columns.
pub fn pad70(hk: &str) -> String {
    let mut t: String = hk.chars().take(70).collect();
    while t.chars().count() < 70 {
        t.push(' ');
    }
    t
}

/// `it` of `it=1; do i=1,70; if (hk(i:i).ne.' ') it=i; enddo`.
pub fn hk_it(hk: &str) -> usize {
    let mut it = 1;
    for (i, c) in hk.chars().take(70).enumerate() {
        if c != ' ' {
            it = i + 1;
        }
    }
    it
}

/// The viewr input text, built line by line.
#[derive(Debug, Default, Clone)]
pub struct Plot {
    /// The text written so far.
    pub text: String,
}

impl Plot {
    /// One record, a `\n` added.
    pub fn line(&mut self, s: &str) {
        self.text.push_str(s);
        self.text.push('\n');
    }

    /// `(a,'<',a,'>',a,'/')` of `qu,hk(1:it),qu`: the page title record.
    pub fn title(&mut self, hk: &str, it: usize) {
        let t: String = hk.chars().take(it).collect();
        self.line(&format!("'<{t}>'/"));
    }

    /// `('1',i3,'/')` followed by the title record: the start of a page.
    pub fn page(&mut self, iwcol: i64, hk: &str, it: usize) {
        self.line(&format!("1{}/", fi(iwcol, 3)));
        self.title(hk, it);
    }

    /// `(1p,3e12.3,'/')`: an axis record.
    pub fn axis(&mut self, lo: f64, hi: f64, step: f64) {
        self.line(&format!("{}{}{}/", fe(lo, 12, 3, true), fe(hi, 12, 3, true), fe(step, 12, 3, true)));
    }

    /// `(1p,2e14.6,'/')`: a curve point.
    pub fn pair(&mut self, x: f64, y: f64) {
        self.line(&format!("{}{}/", fe(x, 14, 6, true), fe(y, 14, 6, true)));
    }

    /// `(1p,e14.6,'/')`: the energy record of a 3-D page.
    pub fn single(&mut self, x: f64) {
        self.line(&format!("{}/", fe(x, 14, 6, true)));
    }

    /// `('<lead>',2e12.4,'/')`: the label position record (scale factor 0).
    pub fn tag(&mut self, lead: &str, xtag: f64, ytag: f64) {
        self.line(&format!("{lead}{}{}/", fe(xtag, 12, 4, false), fe(ytag, 12, 4, false)));
    }

    /// `('0 0',i2,'/')` or `('0 0 0',i2,'/')`: the curve style record, by `iwcol`.
    pub fn style(&mut self, iwcol: i64, icurv: i64) {
        if iwcol == 0 {
            self.line(&format!("0 0{}/", fi(icurv, 2)));
        } else {
            self.line(&format!("0 0 0{}/", fi(icurv, 2)));
        }
    }

    /// `(i2,'/')` and `('/')`: the start of curve number `n` (`n > 1`).
    pub fn next_curve(&mut self, n: i64) {
        self.line(&format!("{}/", fi(n, 2)));
        self.line("/");
    }
}

/// An immutable ACE table as `aplots` sees it: the `xss` array with the
/// `NXS`/`JXS` pointers `acefix` reads into module variables
/// (`acefc.f90:14037`).
#[derive(Debug, Clone)]
pub struct AceTable {
    /// The XSS block, `xss(1..len2)`.
    pub xss: Vec<f64>,
    /// `aw0` of the header.
    pub aw0: f64,
    /// `izai`: 1 neutron, 1001 p, 1002 d, 1003 t, 2003 he-3, 2004 alpha.
    pub izai: i64,
    /// `awi`: the incident-particle mass in neutron masses (`awit`).
    pub awi: f64,
    /// `iverf` of the `endf` module (`mtname`'s table choice; -1 when unset).
    pub iverf: i32,
    /// `nes`.
    pub nes: i64,
    /// `ntr`.
    pub ntr: i64,
    /// `nr`.
    pub nr: i64,
    /// `ntrp`.
    pub ntrp: i64,
    /// `ntype`.
    pub ntype: i64,
    /// `ndnf`.
    pub ndnf: i64,
    /// `esz`.
    pub esz: i64,
    /// `nu`.
    pub nu: i64,
    /// `mtr`.
    pub mtr: i64,
    /// `tyr`.
    pub tyr: i64,
    /// `lsig`.
    pub lsig: i64,
    /// `sig`.
    pub sig: i64,
    /// `land`.
    pub land: i64,
    /// `and`.
    pub and: i64,
    /// `ldlw`.
    pub ldlw: i64,
    /// `dlw`.
    pub dlw: i64,
    /// `gpd`.
    pub gpd: i64,
    /// `mtrp`.
    pub mtrp: i64,
    /// `lsigp`.
    pub lsigp: i64,
    /// `sigp`.
    pub sigp: i64,
    /// `ldlwp`.
    pub ldlwp: i64,
    /// `dlwp`.
    pub dlwp: i64,
    /// `iurpt`.
    pub iurpt: i64,
    /// `nud`.
    pub nud: i64,
    /// `dndat`.
    pub dndat: i64,
    /// `ldnd`.
    pub ldnd: i64,
    /// `dnd`.
    pub dnd: i64,
    /// `ptype`.
    pub ptype: i64,
    /// `ntro`.
    pub ntro: i64,
    /// `ploct`.
    pub ploct: i64,
}

impl AceTable {
    /// The table from the `NXS`/`JXS` arrays `acefix` reads (0-based here).
    pub fn new(nxs: &[i64; 16], jxs: &[i64; 32], xss: Vec<f64>, aw0: f64, izai: i64, awi: f64, iverf: i32) -> Self {
        AceTable {
            xss,
            aw0,
            izai,
            awi,
            iverf,
            nes: nxs[2],
            ntr: nxs[3],
            nr: nxs[4],
            ntrp: nxs[5],
            ntype: nxs[6],
            ndnf: nxs[7],
            esz: jxs[0],
            nu: jxs[1],
            mtr: jxs[2],
            tyr: jxs[4],
            lsig: jxs[5],
            sig: jxs[6],
            land: jxs[7],
            and: jxs[8],
            ldlw: jxs[9],
            dlw: jxs[10],
            gpd: jxs[11],
            mtrp: jxs[12],
            lsigp: jxs[13],
            sigp: jxs[14],
            ldlwp: jxs[17],
            dlwp: jxs[18],
            iurpt: jxs[22],
            nud: jxs[23],
            dndat: jxs[24],
            ldnd: jxs[25],
            dnd: jxs[26],
            ptype: jxs[29],
            ntro: jxs[30],
            ploct: jxs[31],
        }
    }

    /// `xss(i)`, 1-based. Outside the block Fortran reads the zero-filled tail
    /// of its 50 000 000-word array; so does this.
    pub fn x(&self, i: i64) -> f64 {
        if i >= 1 && (i as usize) <= self.xss.len() {
            self.xss[(i - 1) as usize]
        } else {
            0.0
        }
    }

    /// `nint(xss(i))`.
    pub fn xi(&self, i: i64) -> i64 {
        nint(self.x(i))
    }
}
