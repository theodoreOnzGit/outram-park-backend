// Ported from NJOY2016 `src/covr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! The BOXER *text* layer: the record layout `press` writes
//! (`covr.f90:2199-2207`) and a reader for it.
//!
//! Every compressed page is three blocks (`covr.f90:2199-2207`):
//!
//! ```text
//! (i1,1x,a12,1x,a21,2(i5,i4),2(i4,i3),3i4)   first page
//! (i1,1x,34('-'),2(i5,i4),2(i4,i3),3i4)      continuation page
//! ivft  — nval values in the setfor format (1p8e10.3, 11f7.4, ...)
//! icft  — ncon control codes in the setfor format (26i3, 20i4, ...)
//! ```
//!
//! The header fields are `itype, hlibid, hdescr, mat, mt, mat1, mt1, nval,
//! nvf, ncon, ncf, nrowm, nrow, ncol`; `nrowm` is the number of rows still to
//! come on later pages and `ncol = 0` marks a symmetric (upper-triangle)
//! matrix. Numbers are laid out with the Fortran edit descriptors of
//! [`setfor`] — `1pEw.d` via [`fortran_e`], `Fw.d`, `Iw` — and a record
//! holds exactly the remaining items, no trailing padding.
//!
//! [`press_text`] is the writer; [`parse_boxer_text`] is the matching reader
//! (no upstream counterpart — NJOY's COVR only writes BOXER — provided so the
//! writer can be verified against NJOY output value-by-value as well as
//! byte-by-byte).

use crate::covr::boxer::{setfor, BoxerData, BoxerDataType, BoxerHeader, BoxerPage, BoxerShape};
use crate::dtfr::format::fortran_e;
use crate::NjoyError;

/// Fortran `aN` output of a variable of length `N`: truncate or blank-pad.
fn fixed(s: &str, width: usize) -> String {
    let mut t: String = s.chars().take(width).collect();
    while t.chars().count() < width {
        t.push(' ');
    }
    t
}

/// One value in the `ift(nvf)` edit descriptor (`covr.f90:2230-2234`):
/// `Fw.(w-3)` for `nvf = 7, 8`, `1pEw.(w-7)` for `nvf = 9..=14`.
fn value_field(x: f64, nvf: i32) -> String {
    let w = nvf as usize;
    if nvf < 9 {
        let d = w - 3;
        format!("{x:>w$.d$}")
    } else {
        fortran_e(x, w - 7, w)
    }
}

/// Write the BOXER text of one compressed matrix (`covr.f90:2199-2207`).
///
/// Per page: the header record (the first page carries `hlibid`/`hdescr`,
/// later ones 34 dashes), then `nval` values and `ncon` control codes, each
/// wrapped to the per-line count of [`setfor`]. Byte-exact with NJOY2016's
/// `press` (`tests/covr_boxer_golden.rs`).
///
/// # Errors
/// Propagates [`setfor`]'s range check.
pub fn press_text(
    header: &BoxerHeader,
    data: &BoxerData,
    nvf: i32,
    ncf: i32,
) -> Result<String, NjoyError> {
    let (vfmt, cfmt) = setfor(nvf, ncf)?;
    let ncol_hdr = match data.shape {
        BoxerShape::SymmetricUpperTriangle => 0,
        BoxerShape::Rectangular => data.ncol,
    };
    let mut out = String::new();
    for (p, page) in data.pages.iter().enumerate() {
        let nval = page.xval.len();
        let ncon = page.icon.len();
        let nrowm = data.nrow - page.last_row;
        let label = if p == 0 {
            format!(
                "{} {}",
                fixed(&header.hlibid, 12),
                fixed(&header.hdescr, 21)
            )
        } else {
            "-".repeat(34)
        };
        out.push_str(&format!(
            "{} {}{:5}{:4}{:5}{:4}{:4}{:3}{:4}{:3}{:4}{:4}{:4}\n",
            header.itype.code(),
            label,
            header.mat,
            header.mt,
            header.mat1,
            header.mt1,
            nval,
            nvf,
            ncon,
            ncf,
            nrowm,
            data.nrow,
            ncol_hdr,
        ));
        for chunk in page.xval.chunks(vfmt.per_line) {
            for &x in chunk {
                out.push_str(&value_field(x, nvf));
            }
            out.push('\n');
        }
        let w = ncf as usize;
        for chunk in page.icon.chunks(cfmt.per_line) {
            for &c in chunk {
                out.push_str(&format!("{c:>w$}"));
            }
            out.push('\n');
        }
    }
    Ok(out)
}

/// One matrix read back from BOXER text: its header and compressed pages.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxerRecord {
    /// The header of the first page.
    pub header: BoxerHeader,
    /// `nvf` and `ncf` as recorded on the tape.
    pub nvf: i32,
    /// See `nvf`.
    pub ncf: i32,
    /// The pages, ready for [`crate::covr::boxer::decompress`].
    pub data: BoxerData,
}

fn field<T: std::str::FromStr>(
    line: &str,
    range: std::ops::Range<usize>,
    what: &str,
) -> Result<T, NjoyError> {
    let s: String = line
        .chars()
        .skip(range.start)
        .take(range.end - range.start)
        .collect();
    s.trim()
        .parse()
        .map_err(|_| NjoyError::EndfParse(format!("boxer: bad {what} field {s:?} in {line:?}")))
}

/// Parse a run of fixed-width numeric fields spanning as many lines as
/// needed; each line holds at most `per_line` fields of `width` columns.
fn parse_fields<T: std::str::FromStr>(
    lines: &mut std::iter::Peekable<std::str::Lines<'_>>,
    count: usize,
    width: usize,
    per_line: usize,
    what: &str,
) -> Result<Vec<T>, NjoyError> {
    let mut out = Vec::with_capacity(count);
    while out.len() < count {
        let line = lines
            .next()
            .ok_or_else(|| NjoyError::EndfParse(format!("boxer: truncated {what} block")))?;
        let take = (count - out.len()).min(per_line);
        for k in 0..take {
            out.push(field::<T>(line, k * width..(k + 1) * width, what)?);
        }
    }
    Ok(out)
}

/// Read every matrix of a BOXER text (the inverse of [`press_text`]).
///
/// # Errors
/// [`NjoyError::EndfParse`] on a malformed header, a truncated block, an
/// unparsable field, or an `itype` outside `0..=4`.
pub fn parse_boxer_text(text: &str) -> Result<Vec<BoxerRecord>, NjoyError> {
    let mut lines = text.lines().peekable();
    let mut records = Vec::new();
    while let Some(line) = lines.next() {
        if line.trim().is_empty() {
            continue;
        }
        let itype = match field::<i32>(line, 0..1, "itype")? {
            0 => BoxerDataType::GroupBoundaries,
            1 => BoxerDataType::CrossSections,
            2 => BoxerDataType::RelativeStdDev,
            3 => BoxerDataType::RelativeCovariance,
            4 => BoxerDataType::Correlation,
            other => {
                return Err(NjoyError::EndfParse(format!(
                    "boxer: itype {other} out of range"
                )))
            }
        };
        let hlibid: String = line.chars().skip(2).take(12).collect();
        let hdescr: String = line.chars().skip(15).take(21).collect();
        let header = BoxerHeader {
            itype,
            hlibid,
            hdescr,
            mat: field(line, 36..41, "mat")?,
            mt: field(line, 41..45, "mt")?,
            mat1: field(line, 45..50, "mat1")?,
            mt1: field(line, 50..54, "mt1")?,
        };
        let nvf: i32 = field(line, 58..61, "nvf")?;
        let ncf: i32 = field(line, 65..68, "ncf")?;
        let nrow: usize = field(line, 72..76, "nrow")?;
        let ncol_hdr: usize = field(line, 76..80, "ncol")?;
        let (vfmt, cfmt) = setfor(nvf, ncf)?;
        let mut pages = Vec::new();
        let mut istart = 1usize;
        let mut cur = line;
        loop {
            let nval: usize = field(cur, 54..58, "nval")?;
            let ncon: usize = field(cur, 61..65, "ncon")?;
            let nrowm: usize = field(cur, 68..72, "nrowm")?;
            let xval = parse_fields::<f64>(&mut lines, nval, nvf as usize, vfmt.per_line, "xval")?;
            let icon = parse_fields::<i32>(&mut lines, ncon, ncf as usize, cfmt.per_line, "icon")?;
            let last_row = nrow - nrowm;
            pages.push(BoxerPage {
                istart,
                last_row,
                xval,
                icon,
            });
            if nrowm == 0 {
                break;
            }
            istart = last_row + 1;
            cur = lines
                .next()
                .ok_or_else(|| NjoyError::EndfParse("boxer: missing continuation page".into()))?;
        }
        let (shape, ncol) = if ncol_hdr == 0 {
            (BoxerShape::SymmetricUpperTriangle, nrow)
        } else {
            (BoxerShape::Rectangular, ncol_hdr)
        };
        records.push(BoxerRecord {
            header,
            nvf,
            ncf,
            data: BoxerData {
                nrow,
                ncol,
                shape,
                pages,
            },
        });
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::covr::boxer::{compress, decompress};

    /// Methodology: the header edit list `(i1,1x,a12,1x,a21,2(i5,i4),
    /// 2(i4,i3),3i4)` (`covr.f90:2199-2201`) fixes every column. Encode a
    /// 2x1 vector and compare the header against the layout NJOY2016 wrote
    /// for H-2 (`0 w4cov -a- 30 oracle covr w4         128   1  128   1  31
    /// 10  31  3   0  31   1`, `reference-data/covr/`), substituting this
    /// vector's counts. Result (2026-09-10): byte-identical columns.
    #[test]
    fn header_columns_match_fortran_edit_list() {
        let data = compress(&[1.39e-4, 0.152], 2, 1, BoxerShape::Rectangular, 10, 3).unwrap();
        let header = BoxerHeader {
            itype: BoxerDataType::GroupBoundaries,
            hlibid: "w4cov -a- 30".into(),
            hdescr: "oracle covr w4".into(),
            mat: 128,
            mt: 1,
            mat1: 128,
            mt1: 1,
        };
        let text = press_text(&header, &data, 10, 3).unwrap();
        let mut l = text.lines();
        assert_eq!(
            l.next().unwrap(),
            "0 w4cov -a- 30 oracle covr w4         128   1  128   1   2 10   2  3   0   2   1"
        );
        assert_eq!(l.next().unwrap(), " 1.390E-04 1.520E-01");
        assert_eq!(l.next().unwrap(), " -1 -1");
        assert!(l.next().is_none());
    }

    /// Methodology: `11f7.4` (nvf=7) and `20i4` (ncf=4) are the correlation
    /// formats; negative values must pack without separators the way
    /// Fortran does (`-1.007E-01-1.270E-02` in the H-2 oracle). Check both a
    /// negative F field and adjacent negative E fields. Result
    /// (2026-09-10): as expected.
    #[test]
    fn value_fields_pack_like_fortran() {
        assert_eq!(value_field(-0.1234, 7), "-0.1234");
        assert_eq!(value_field(1.0, 7), " 1.0000");
        assert_eq!(
            format!("{}{}", value_field(-0.1007, 10), value_field(-1.27e-2, 10)),
            "-1.007E-01-1.270E-02"
        );
        assert_eq!(value_field(0.0, 10), " 0.000E+00");
    }

    /// Methodology: writer -> reader -> decoder must reproduce the encoded
    /// matrix, including a symmetric matrix (ncol=0 header) and a
    /// multi-page one (a 40x40 of distinct values pages at 880 xval).
    /// Result (2026-09-10): structure exact, values to 1e-7, 2 pages.
    #[test]
    fn text_round_trips_including_pages() {
        let n = 40;
        let m: Vec<f64> = (0..n * n).map(|k| (k + 1) as f64 * 1.0e3).collect();
        let data = compress(&m, n, n, BoxerShape::Rectangular, 14, 4).unwrap();
        assert!(data.pages.len() > 1);
        let header = BoxerHeader {
            itype: BoxerDataType::RelativeCovariance,
            hlibid: "id".into(),
            hdescr: "descr".into(),
            mat: 9237,
            mt: 18,
            mat1: 9237,
            mt1: 102,
        };
        let text = press_text(&header, &data, 14, 4).unwrap();
        let recs = parse_boxer_text(&text).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].header.mat1, 9237);
        // Structure round-trips exactly; values only to the 8 printed figures
        // (`sigfig` leaves a 1e-13 bias the text cannot carry).
        assert_eq!(recs[0].data.pages.len(), data.pages.len());
        for (p, q) in recs[0].data.pages.iter().zip(&data.pages) {
            assert_eq!(
                (p.istart, p.last_row, &p.icon),
                (q.istart, q.last_row, &q.icon)
            );
            for (a, b) in p.xval.iter().zip(&q.xval) {
                assert!((a - b).abs() <= 1e-7 * b.abs(), "{a} vs {b}");
            }
        }
        let back = decompress(&recs[0].data).unwrap();
        for (a, b) in decompress(&data).unwrap().iter().zip(&back) {
            assert!((a - b).abs() <= 1e-7 * a.abs(), "{a} vs {b}");
        }

        let sym = vec![4.0, 2.0, 2.0, 9.0];
        let d2 = compress(&sym, 2, 2, BoxerShape::SymmetricUpperTriangle, 10, 4).unwrap();
        let t2 = press_text(&header, &d2, 10, 4).unwrap();
        let r2 = parse_boxer_text(&t2).unwrap();
        assert_eq!(r2[0].data.shape, BoxerShape::SymmetricUpperTriangle);
        assert_eq!(r2[0].data.pages[0].icon, d2.pages[0].icon);
    }
}
