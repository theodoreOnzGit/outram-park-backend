// Ported from NJOY2016 `src/acefc.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da):
//   - `subroutine acefix`, l.13966-14216: ACER `iopt = 7` (print or edit an
//     ACE file), Type 1 (ASCII) input;
//   - `subroutine aceout`, l.12988-13089: the Type 1 header and the
//     directory (`xsdir`) line.
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under AGPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! ACER `iopt = 7` for a fast (`c`) or charged-particle (`h o r s a`) Type 1
//! ACE file: read it, adjust the ZAID, comment and `(iz, aw)` list, and write
//! it again together with the directory line (gh:#1492, parent #1369).
//!
//! What this port does and does not do of `acefix`:
//!
//! - **Done**: the header fields `acefix` reads (`hz`, `aw0`, `tz`, `hd`,
//!   `hk`, `hm`, the 16 `(iz, aw)` pairs, `NXS`/`JXS`), the ZAID rule
//!   (`suff >= 0`: `zaid = int(zaid + 0.001) + suff`, `f9.2`), the comment
//!   rule (a blank `hk` keeps the file's), the `nxtra` rule (the file's list
//!   only when `nxtra != 0`), and `aceout`'s Type 1 header and `xsdir` line.
//! - **XSS is re-printed by `change`** ([`super::acefix_change`]): each word
//!   as `i20` or `1pe20.11` by its position in the table's block layout, which
//!   the file does not record. NJOY's own iopt = 1 output prints a few words
//!   of the charged-particle blocks as reals that `change` prints as integers
//!   (deck 24: `-6.33700000000E+03` becomes `-6337`), so the check file is
//!   not a copy of its input.
//! - **Not ported**: `consis` (the consistency checks, `acefc.f90:14218-15180`,
//!   which only raise errors and print), `aceprt` (the listing), the Type 2
//!   (binary) container and MCNPX formats. A deck asking for them gets
//!   `NotPorted`. The plot file `aplots` is ported ([`super::aplots`]).

use super::fortran_fmt::{fixed, fortran_e, fortran_f, fortran_f0};
use crate::NjoyError;

/// What `acefix` is asked for: ACER cards 2 and 3 (and the `nxtra` pairs).
#[derive(Debug, Clone, PartialEq)]
pub struct AcefixRequest {
    /// `suff`: a new ZAID suffix, or negative to keep the file's ZAID.
    pub suff: f64,
    /// `hk`: the new comment; blank keeps the file's.
    pub hk: String,
    /// `nxtra`: the number of leading `(iz, aw)` pairs taken from the file; 0
    /// writes a zero list, as `acefix` does.
    pub nxtra: i32,
    /// `ngend != 0`: also write the plot file (`aplots`, [`super::aplots`]).
    pub plot: bool,
}

/// The files `aceout` writes.
#[derive(Debug, Clone, PartialEq)]
pub struct AcefixOutput {
    /// The ACE file, Type 1.
    pub ace: String,
    /// The directory (`ndir`) line.
    pub xsdir: String,
    /// The viewr input text of `aplots`, when asked for.
    pub plot: Option<String>,
}

fn fmt_err(what: &str) -> NjoyError {
    NjoyError::InvalidInput(format!("acefix: {what}"))
}

/// Fortran `f12.6` read of a field: decimal point in the field wins.
fn read_real(field: &str) -> Result<f64, NjoyError> {
    let t = field.trim();
    if t.is_empty() {
        return Ok(0.0);
    }
    t.replace(['d', 'D'], "e").parse::<f64>().map_err(|_| fmt_err("a real field of the header does not parse"))
}

/// A `4e20.0` word: Fortran also reads `1.5-117` as `1.5E-117`.
fn read_xss_word(field: &str) -> Result<f64, NjoyError> {
    let t = field.trim();
    if t.is_empty() {
        return Ok(0.0);
    }
    let t = t.replace(['d', 'D'], "e");
    if let Ok(x) = t.parse::<f64>() {
        return Ok(x);
    }
    let b = t.as_bytes();
    for i in 1..b.len() {
        if (b[i] == b'+' || b[i] == b'-') && b[i - 1] != b'e' && b[i - 1] != b'E' {
            return format!("{}e{}", &t[..i], &t[i..])
                .parse::<f64>()
                .map_err(|_| fmt_err("an XSS word does not parse"));
        }
    }
    Err(fmt_err("an XSS word does not parse"))
}

fn read_int(field: &str) -> Result<i64, NjoyError> {
    let t = field.trim();
    if t.is_empty() {
        return Ok(0);
    }
    t.parse::<i64>().map_err(|_| fmt_err("an integer field of the header does not parse"))
}

fn col(line: &str, a: usize, b: usize) -> &str {
    let n = line.len();
    &line[a.min(n)..b.min(n)]
}

/// `acefix` + `aceout` for a Type 1, non-MCNPX file of class `c h o r s a`.
///
/// `text` is the ACE file; `req` the cards. Returns the new file and the
/// directory line.
///
/// # Errors
/// `InvalidInput` for a header that does not read as `acefix` reads it, an
/// XSS block shorter than `len2` words, or a ZAID class `acefix` rejects
/// (`illegal file type`); `NotPorted` for thermal (`t`), dosimetry (`y`),
/// photo-atomic (`p`) and photo-nuclear (`u`) files, which use `thrfix`,
/// `dosfix`, `phofix` and `phnfix`.
pub fn acefix_type1(text: &str, req: &AcefixRequest) -> Result<AcefixOutput, NjoyError> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() < 12 {
        return Err(fmt_err("fewer than 12 lines"));
    }
    // `(a10,f12.6,1x,1p,e11.4,1x,a10 / a70,a10)`
    let (l1, l2) = (lines[0], lines[1]);
    let hz_in = fixed(col(l1, 0, 10), 10);
    let aw0 = read_real(col(l1, 10, 22))?;
    let tz_text = col(l1, 23, 34);
    let tz = read_real(tz_text)?;
    let hd = fixed(col(l1, 35, 45), 10);
    let hko = fixed(col(l2, 0, 70), 70);
    let hm = fixed(col(l2, 70, 80), 10);
    // `(4(i7,f11.0))` x 4
    let mut izo = Vec::with_capacity(16);
    for l in &lines[2..6] {
        for k in 0..4 {
            let a = k * 18;
            izo.push((read_int(col(l, a, a + 7))? as i32, read_real(col(l, a + 7, a + 18))?));
        }
    }
    // `(8i9)` x 6: NXS(16) then JXS(32), `acefix`'s variable list.
    let mut v: Vec<i64> = Vec::with_capacity(48);
    for l in &lines[6..12] {
        for k in 0..8 {
            v.push(read_int(col(l, k * 9, k * 9 + 9))?);
        }
    }
    let len2 = v[0];
    let iurpt = v[16 + 22];
    let n = ((len2 + 3) / 4) as usize;
    if lines.len() < 12 + n {
        return Err(fmt_err("the XSS block is shorter than NXS(1)"));
    }
    let mut xss: Vec<f64> = Vec::with_capacity(len2 as usize);
    for (i, l) in lines[12..12 + n].iter().enumerate() {
        let words = if i + 1 == n && len2 % 4 != 0 { (len2 % 4) as usize } else { 4 };
        for k in 0..words {
            xss.push(read_xss_word(col(l, k * 20, k * 20 + 20))?);
        }
    }
    let mut nxs = [0i64; 16];
    nxs.copy_from_slice(&v[..16]);
    let mut jxs = [0i64; 32];
    jxs.copy_from_slice(&v[16..48]);

    // Class letter and ZAID (`acefix`: `read(hz,'(a9,a1)') str,ht`).
    let ht = hz_in.chars().nth(9).unwrap_or(' ');
    match ht {
        'c' | 'h' | 'o' | 'r' | 's' | 'a' => {}
        't' => return Err(NjoyError::NotPorted("ACER iopt = 7 on a thermal file (thrfix)")),
        'y' => return Err(NjoyError::NotPorted("ACER iopt = 7 on a dosimetry file (dosfix)")),
        'p' => return Err(NjoyError::NotPorted("ACER iopt = 7 on a photo-atomic file (phofix)")),
        'u' => return Err(NjoyError::NotPorted("ACER iopt = 7 on a photo-nuclear file (phnfix)")),
        _ => return Err(fmt_err("problem with particle id in zaid")),
    }
    // `izai`/`awi` from the class letter (`acefix`, `awit`).
    let (izai, awi) = match ht {
        'c' => (1, 1.0),
        'h' => (1001, 0.99862),
        'o' => (1002, 1.99626),
        'r' => (1003, 2.98960),
        's' => (2003, 2.98903),
        _ => (2004, 3.96713),
    };
    // `consis` edits the table in memory before it is written and plotted.
    let cidx = super::consis::ConsisIndex {
        nr: nxs[4],
        mtr: jxs[2],
        lqr: jxs[3],
        tyr: jxs[4],
        ldlw: jxs[9],
        dlw: jxs[10],
        aw0,
        awi,
    };
    super::consis::consis_shift_distributions(&mut xss, &cidx);
    let xss_lines = super::acefix_change::xss_lines(&xss, &super::acefix_change::AceIndex { nxs, jxs })?;
    let mut hz = hz_in.clone();
    if req.suff >= 0.0 {
        let zaid = read_real(&hz_in[..9])?;
        let iza = (zaid + 0.001).trunc();
        let new = iza + req.suff;
        hz = format!("{}{ht}", fortran_f(new, 9, 2));
    }
    let hk = if req.hk.trim().is_empty() { hko } else { fixed(&req.hk, 70) };
    // `izn/awn` start at zero (`acer.f90:289-292`); card 4's pairs are
    // overwritten by the file's own for `i = 1..nxtra` (`acefc.f90:14131-14136`).
    let mut izn = [(0i32, 0.0f64); 16];
    for i in 0..(req.nxtra.max(0) as usize).min(16) {
        izn[i] = izo[i];
    }

    // `aceout`, Type 1.
    let mut out = String::with_capacity(text.len() + 16);
    out.push_str(&format!("{}{} {} {}\n", hz, fortran_f(aw0, 12, 6), fortran_e(tz, 4, 11), hd));
    out.push_str(&format!("{hk}{hm}\n"));
    for row in izn.chunks(4) {
        for (iz, aw) in row {
            out.push_str(&format!("{iz:7}{}", fortran_f0(*aw)));
        }
        out.push('\n');
    }
    for l in &lines[6..12] {
        out.push_str(l);
        out.push('\n');
    }
    for l in &xss_lines {
        out.push_str(l);
        out.push('\n');
    }

    // The directory line: `itype = 1`, `irec1 = 1`, `lrec = nern = 0`.
    let mut xsdir = format!(
        "{}{} filename route{:2}{:4} {:8}{:6}{:6}{}",
        hz,
        fortran_f(aw0, 12, 6),
        1,
        1,
        len2,
        0,
        0,
        fortran_e(tz, 3, 10)
    );
    if iurpt > 0 {
        xsdir.push_str(" ptable");
    }
    xsdir.push('\n');
    let plot = if req.plot {
        // `iverf` is the `endf` module's, 6 after an ENDF-6 `iopt = 1` run in the
        // same job and -1 otherwise: `mtname` treats both alike.
        let table = super::aplots::AceTable::new(&nxs, &jxs, xss.clone(), aw0, izai, awi, -1);
        Some(super::aplots::aplots(&table, hk.trim_end())?)
    } else {
        None
    };
    Ok(AcefixOutput { ace: out, xsdir, plot })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiny() -> String {
        // A 2-word table: header lines as NJOY writes them.
        let mut s = String::new();
        s.push_str(&format!("{}{} {} {}\n", "  1001.10c", fortran_f(0.999167, 12, 6), fortran_e(2.53e-8, 4, 11), "  11/11/26"));
        s.push_str(&format!("{}{}\n", fixed("old comment", 70), "   mat 125"));
        for _ in 0..4 {
            for _ in 0..4 {
                s.push_str(&format!("{:7}{}", 0, fortran_f0(0.0)));
            }
            s.push('\n');
        }
        let mut nums = vec![0i64; 48];
        nums[0] = 6;
        nums[2] = 1; // NES
        nums[16] = 1; // ESZ
        nums[23] = 6; // LAND
        nums[24] = 7; // AND
        for l in nums.chunks(8) {
            s.push_str(&l.iter().map(|x| format!("{x:9}")).collect::<String>());
            s.push('\n');
        }
        s.push_str(&format!("{}{}{}{}\n", fortran_e20(1.0), fortran_e20(2.0), fortran_e20(3.0), fortran_e20(4.0)));
        s.push_str(&format!("{}{:20}\n", fortran_e20(5.0), 0));
        s
    }

    use super::super::fortran_fmt::fortran_e20;

    #[test]
    fn comment_replaced_everything_else_kept() {
        let req = AcefixRequest { suff: -1.0, hk: "new comment".into(), nxtra: 0, plot: false };
        let out = acefix_type1(&tiny(), &req).unwrap();
        let t = tiny();
        let (a, b): (Vec<&str>, Vec<&str>) = (out.ace.lines().collect(), t.lines().collect());
        assert_eq!(a.len(), b.len());
        assert!(a[1].starts_with("new comment"));
        assert_eq!(a[0], b[0]);
        assert_eq!(&a[2..], &b[2..]);
        assert!(out.xsdir.starts_with("  1001.10c    0.999167 filename route 1   1        6     0     0 2.530E-08"));
    }

    #[test]
    fn blank_comment_keeps_and_suffix_applies() {
        let req = AcefixRequest { suff: 0.2, hk: "  ".into(), nxtra: 0, plot: false };
        let out = acefix_type1(&tiny(), &req).unwrap();
        assert!(out.ace.starts_with("  1001.20c"));
        assert!(out.ace.lines().nth(1).unwrap().starts_with("old comment"));
    }
}
