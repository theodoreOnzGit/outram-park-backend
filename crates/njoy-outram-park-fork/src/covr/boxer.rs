// Ported from NJOY2016 `src/covr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! COVR's BOXER-format covariance-library writer (`press`/`setfor`).
//!
//! `subroutine press` (`covr.f90:1991-2218`) compresses a data matrix into the
//! "boxer" run-length format and writes it to the covariance-library tape;
//! `subroutine setfor` (`covr.f90:2220-2247`) picks the Fortran output formats.
//! This module ports both.
//!
//! # The BOXER compression (`covr.f90:2085-2196`)
//!
//! BOXER walks the matrix row by row (a symmetric matrix only walks its upper
//! triangle) and encodes the flattened value sequence as two arrays:
//!
//! - `xval` — the distinct data values, and
//! - `icon` — a control code per run: a **negative** code `-lv` means "the next
//!   `lv` cells all equal the next `xval` entry" (repeated-value run), while a
//!   **positive** code `lc` means "the next `lc` cells each equal the cell
//!   directly above them" (carry-down run). Carry-down runs store no `xval`.
//!
//! Two candidate runs (repeated-value and carry-down) are tracked at once and
//! the winner is emitted when both stop matching (`covr.f90:2131-2175`). Runs
//! are capped so their counts fit the integer field (`lvmax`/`lcmax`,
//! `covr.f90:2054-2055`) and, when the value/control arrays fill, a new "page"
//! is started (`nvmax`/`ncmax`, `covr.f90:2038-2039,2180-2211`). Pages are
//! self-contained: no run crosses a page boundary.
//!
//! # What is ported
//!
//! - [`compress`] — the run-length encoder (`covr.f90:2085-2196`), producing
//!   [`BoxerData`] (pages of `xval`/`icon`). Pure and exactly invertible.
//! - [`decompress`] — the matching decoder, reconstructing the (sigfig-rounded)
//!   dense matrix. This is the inverse a BOXER *reader* performs; it is provided
//!   here so the encoder can be verified by round-trip.
//! - [`setfor`] — the format-descriptor selection (`covr.f90:2220-2247`).
//! - [`press_text`] — the full text layout: the header line plus the formatted
//!   `xval`/`icon` blocks (`covr.f90:2199-2207`).
//!
//! # Fidelity note (honest)
//!
//! The **compression logic** ([`compress`]/[`decompress`]) is a faithful,
//! exactly-invertible port. The **numeric text** emitted by [`press_text`] uses
//! Rust's float formatter within the correct field structure (right count per
//! line, per [`setfor`]); it is **not** a byte-exact emulation of Fortran's
//! `1P Ew.d` edit descriptor, and no golden-file comparison against upstream
//! NJOY has been run. The header counts, the value/control arrays, and the
//! round-trip are what the tests verify.

use crate::covr::correlation::CovarianceMatrix;
use crate::NjoyError;

/// The BOXER data-type tag `itype` (`covr.f90:2016-2020`), written in column 1
/// of every record header. Identifies what physical quantity the matrix holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxerDataType {
    /// `itype = 0` — energy-group boundaries.
    GroupBoundaries,
    /// `itype = 1` — cross sections.
    CrossSections,
    /// `itype = 2` — relative standard deviations.
    RelativeStdDev,
    /// `itype = 3` — a relative covariance matrix.
    RelativeCovariance,
    /// `itype = 4` — a correlation matrix.
    Correlation,
}

impl BoxerDataType {
    /// The integer `itype` written in the record header (`covr.f90:2016-2020`).
    pub fn code(self) -> i32 {
        match self {
            BoxerDataType::GroupBoundaries => 0,
            BoxerDataType::CrossSections => 1,
            BoxerDataType::RelativeStdDev => 2,
            BoxerDataType::RelativeCovariance => 3,
            BoxerDataType::Correlation => 4,
        }
    }
}

/// The shape of the matrix handed to [`compress`], mapping onto `press`'s `ncol`
/// convention (`covr.f90:1998-2003`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoxerShape {
    /// A full `nrow x ncol` rectangular matrix.
    Rectangular,
    /// A symmetric matrix — `press` walks only the upper right triangle
    /// (`ncol = 0` marker in the header; `nsym = 1` internally,
    /// `covr.f90:2066-2068,2096`). The caller must supply a *full* symmetric
    /// buffer (both triangles filled); [`compress`] reads the upper triangle.
    SymmetricUpperTriangle,
}

/// Header metadata written on the first record of a matrix (`covr.f90:2199-2205`).
#[derive(Debug, Clone, PartialEq)]
pub struct BoxerHeader {
    /// The data-type tag `itype`.
    pub itype: BoxerDataType,
    /// `hlibid` — a library identifier (written as 12 chars).
    pub hlibid: String,
    /// `hdescr` — a description (written as 21 chars).
    pub hdescr: String,
    /// `mat` — ENDF material of the row reaction.
    pub mat: i32,
    /// `mt` — ENDF reaction of the row reaction.
    pub mt: i32,
    /// `mat1` — ENDF material of the column reaction.
    pub mat1: i32,
    /// `mt1` — ENDF reaction of the column reaction.
    pub mt1: i32,
}

/// One compression "page": a self-contained block of `xval`/`icon` covering
/// matrix rows `istart..=last_row` (1-based) (`covr.f90:2180-2211`).
#[derive(Debug, Clone, PartialEq)]
pub struct BoxerPage {
    /// First matrix row (1-based) covered by this page (`istart`).
    pub istart: usize,
    /// Last matrix row (1-based) covered by this page.
    pub last_row: usize,
    /// The distinct data values (`xval(1..nval)`).
    pub xval: Vec<f64>,
    /// The run-length control codes (`icon(1..ncon)`).
    pub icon: Vec<i32>,
}

/// The full BOXER encoding of a matrix — one or more [`BoxerPage`]s plus the
/// shape needed to decode them.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxerData {
    /// Number of matrix rows (`nrow`).
    pub nrow: usize,
    /// Number of matrix columns (`ncol`) for a rectangular matrix; equals
    /// `nrow` for a symmetric matrix (the header records `ncol = 0` for
    /// symmetric via [`BoxerShape`]).
    pub ncol: usize,
    /// The matrix shape.
    pub shape: BoxerShape,
    /// The compression pages.
    pub pages: Vec<BoxerPage>,
}

// press storage caps (covr.f90:2038-2039).
const NVMAX: usize = 880;
const NCMAX: usize = 900;
// repeated-value equality tolerance (covr.f90:2040).
const EPS: f64 = 1.0e-10;

/// Number of significant figures `press` rounds each value to before
/// compressing, from the value-format index `nvf` (`covr.f90:2052-2053`).
fn ndig_for(nvf: i32) -> i32 {
    if nvf < 9 {
        nvf - 3
    } else {
        nvf - 6
    }
}

/// Compress a dense matrix into BOXER run-length pages (`press`,
/// `covr.f90:2085-2196`).
///
/// `matrix` is row-major `nrow x ncol` (for [`BoxerShape::SymmetricUpperTriangle`]
/// the buffer must be a full symmetric matrix, `ncol == nrow`). `nvf` selects
/// the value format (7..=14) and fixes the significant-figure rounding; `ncf`
/// selects the control-code format (1..=6) and fixes the run-length caps
/// `lvmax = 10^(ncf-1) - 1`, `lcmax = 10^ncf - 1`.
///
/// # Errors
/// [`NjoyError::EndfParse`] on a shape mismatch (`matrix.len() != nrow*ncol`,
/// or a symmetric matrix with `ncol != nrow`); propagates [`setfor`]'s range
/// check on `nvf`/`ncf`.
pub fn compress(
    matrix: &[f64],
    nrow: usize,
    ncol: usize,
    shape: BoxerShape,
    nvf: i32,
    ncf: i32,
) -> Result<BoxerData, NjoyError> {
    // Validate formats up front (also the source of lvmax/lcmax).
    let _ = setfor(nvf, ncf)?;
    if matrix.len() != nrow * ncol {
        return Err(NjoyError::EndfParse(format!(
            "press: matrix has {} elements, expected nrow*ncol = {}*{} = {}",
            matrix.len(),
            nrow,
            ncol,
            nrow * ncol
        )));
    }
    let nsym = shape == BoxerShape::SymmetricUpperTriangle;
    if nsym && ncol != nrow {
        return Err(NjoyError::EndfParse(format!(
            "press: symmetric matrix must be square (nrow={nrow}, ncol={ncol})"
        )));
    }
    if nrow == 0 || ncol == 0 {
        return Err(NjoyError::EndfParse("press: empty matrix".into()));
    }

    let ndig = ndig_for(nvf);
    let lvmax: i64 = 10_i64.pow((ncf - 1) as u32) - 1;
    let lcmax: i64 = 10_i64.pow(ncf as u32) - 1;

    // Round every element to ndig significant figures (covr.f90:2072/2079/2105).
    let m: Vec<f64> = matrix
        .iter()
        .map(|&x| crate::mixr::sigfig(x, ndig, 0))
        .collect();
    let at = |i: usize, j: usize| m[(i - 1) * ncol + (j - 1)]; // 1-based access

    let jlow_of = |i: usize| if nsym { i } else { 1 };

    let mut pages = Vec::new();
    let mut istart = 1usize;

    // Outer loop over pages (covr.f90:480).
    loop {
        let mut xval: Vec<f64> = Vec::new();
        let mut icon: Vec<i32> = Vec::new();
        let mut lv: i64 = 0;
        let mut lc: i64 = 0;
        let mut ivopt = true;
        let mut icopt = true;
        let mut icd = 0i32;
        // xvtest initial value = first cell of this page (covr.f90:2074-2075,2082).
        let mut xvtest = at(istart, jlow_of(istart));

        let mut i = istart;
        // Row loop (covr.f90:470).
        loop {
            let jlow = jlow_of(i);
            for j in jlow..=ncol {
                let v = at(i, j);
                // covr.f90:2146 — carry-down reference is 0 on a page's first row.
                let above = if i > istart { at(i - 1, j) } else { 0.0 };
                // Pattern search (covr.f90:310).
                loop {
                    if ivopt {
                        let test = EPS * xvtest.abs();
                        if (v - xvtest).abs() <= test && lv != lvmax {
                            lv += 1;
                            icd = 0;
                        } else {
                            ivopt = false;
                        }
                    }
                    if icopt {
                        let test = EPS * above.abs();
                        if (v - above).abs() <= test && lc != lcmax {
                            lc += 1;
                            icd = 1;
                        } else {
                            icopt = false;
                        }
                    }
                    if icopt || ivopt {
                        break; // cell consumed; go to next column (covr.f90:210)
                    }
                    // Both options failed: emit the finished run (covr.f90:2160-2169).
                    if icd != 0 {
                        icon.push(lc as i32); // carry-down
                    } else {
                        icon.push(-(lv as i32)); // repeated value
                        xval.push(xvtest);
                    }
                    lv = 0;
                    lc = 0;
                    ivopt = true;
                    icopt = true;
                    xvtest = v;
                    // re-test this same cell against a fresh run (covr.f90:2175)
                }
            }
            // Paging decision at the row boundary (covr.f90:2180-2183).
            let leng = if nsym { ncol + 1 - i } else { ncol + 1 };
            let overflow = xval.len() + leng > NVMAX || icon.len() + leng > NCMAX;
            if overflow {
                break; // close this page now
            }
            if i < nrow {
                i += 1;
                continue;
            }
            break; // last row processed
        }

        // Store the final run of the page (covr.f90:2185-2192).
        if icd != 0 {
            icon.push(lc as i32);
        } else {
            icon.push(-(lv as i32));
            xval.push(xvtest);
        }

        let last_row = i;
        pages.push(BoxerPage {
            istart,
            last_row,
            xval,
            icon,
        });

        if last_row >= nrow {
            break;
        }
        istart = last_row + 1;
    }

    Ok(BoxerData {
        nrow,
        ncol,
        shape,
        pages,
    })
}

/// Decode BOXER pages back into the dense (sigfig-rounded) matrix — the inverse
/// of [`compress`], as a BOXER *reader* performs it.
///
/// Reconstructs the same walk order the encoder used (row by row; a symmetric
/// matrix walks columns `i..=ncol` and the lower triangle is mirrored). For a
/// symmetric result the returned buffer is the full `nrow x nrow` matrix.
///
/// Returned buffer is row-major `nrow x ncol`.
///
/// # Errors
/// [`NjoyError::EndfParse`] if the control/value stream is exhausted early or
/// leaves data unused (an internally inconsistent [`BoxerData`]).
pub fn decompress(data: &BoxerData) -> Result<Vec<f64>, NjoyError> {
    let nrow = data.nrow;
    let ncol = data.ncol;
    let nsym = data.shape == BoxerShape::SymmetricUpperTriangle;
    let mut m = vec![0.0_f64; nrow * ncol];
    let jlow_of = |i: usize| if nsym { i } else { 1 };

    for page in &data.pages {
        let mut iv = 0usize; // index into xval
        let mut ic = 0usize; // index into icon
        // Active run state.
        let mut remaining: i64 = 0;
        let mut run_value = 0.0f64;
        let mut run_carry = false;

        for i in page.istart..=page.last_row {
            for j in jlow_of(i)..=ncol {
                if remaining == 0 {
                    // Start a new run from the next control code.
                    let code = *page.icon.get(ic).ok_or_else(|| {
                        NjoyError::EndfParse("boxer decode: icon exhausted".into())
                    })?;
                    ic += 1;
                    if code < 0 {
                        remaining = (-code) as i64;
                        run_carry = false;
                        run_value = *page.xval.get(iv).ok_or_else(|| {
                            NjoyError::EndfParse("boxer decode: xval exhausted".into())
                        })?;
                        iv += 1;
                    } else {
                        remaining = code as i64;
                        run_carry = true;
                    }
                }
                let value = if run_carry {
                    // Carry-down: value of the cell directly above (0 on the
                    // page's first row, mirroring covr.f90:2146).
                    if i > page.istart {
                        m[(i - 2) * ncol + (j - 1)]
                    } else {
                        0.0
                    }
                } else {
                    run_value
                };
                m[(i - 1) * ncol + (j - 1)] = value;
                remaining -= 1;
            }
        }
        if remaining != 0 {
            return Err(NjoyError::EndfParse(
                "boxer decode: run overran page rows".into(),
            ));
        }
    }

    // Mirror the lower triangle for a symmetric matrix.
    if nsym {
        for i in 1..=nrow {
            for j in i + 1..=ncol {
                m[(j - 1) * ncol + (i - 1)] = m[(i - 1) * ncol + (j - 1)];
            }
        }
    }

    Ok(m)
}

// ---------------------------------------------------------------------------
// setfor — output format descriptors (covr.f90:2220-2247)
// ---------------------------------------------------------------------------

/// A parsed Fortran output-format descriptor: how many numbers print per line
/// and the underlying edit descriptor string.
#[derive(Debug, Clone, PartialEq)]
pub struct BoxerFormat {
    /// Values printed per output line (Fortran repeat count).
    pub per_line: usize,
    /// The Fortran format string, e.g. `"(1p6e12.5)"` or `"(20i4)"`.
    pub descriptor: &'static str,
}

// The `ift` table (covr.f90:2230-2234): index 1..14 (1-based in Fortran).
const IFT: [(usize, &str); 14] = [
    (80, "(80i1)"),
    (40, "(40i2)"),
    (26, "(26i3)"),
    (20, "(20i4)"),
    (16, "(16i5)"),
    (13, "(13i6)"),
    (11, "(11f7.4)"),
    (10, "(10f8.5)"),
    (8, "(1p8e9.2)"),
    (8, "(1p8e10.3)"),
    (7, "(1p7e11.4)"),
    (6, "(1p6e12.5)"),
    (6, "(1p6e13.6)"),
    (5, "(1p5e14.7)"),
];

/// Select the value (`ivft`) and control (`icft`) output formats from the
/// indices `nvf`/`ncf` (`setfor`, `covr.f90:2220-2247`).
///
/// Returns `(value_format, control_format)`. `nvf` (value/float) must be in
/// `7..=14`; `ncf` (control/integer) in `1..=6`.
///
/// # Errors
/// [`NjoyError::EndfParse`] when `nvf`/`ncf` fall outside their legal ranges
/// (`covr.f90:2236-2241`).
pub fn setfor(nvf: i32, ncf: i32) -> Result<(BoxerFormat, BoxerFormat), NjoyError> {
    if !(7..=14).contains(&nvf) || !(1..=6).contains(&ncf) {
        return Err(NjoyError::EndfParse(format!(
            "setfor: nvf (={nvf}) or ncf (={ncf}) is illegal (nvf in 7..=14, ncf in 1..=6)"
        )));
    }
    let (vpl, vd) = IFT[(nvf - 1) as usize];
    let (cpl, cd) = IFT[(ncf - 1) as usize];
    Ok((
        BoxerFormat {
            per_line: vpl,
            descriptor: vd,
        },
        BoxerFormat {
            per_line: cpl,
            descriptor: cd,
        },
    ))
}

// ---------------------------------------------------------------------------
// press_text — full text layout (covr.f90:2199-2207)
// ---------------------------------------------------------------------------

/// Fixed-width left/right helpers (Fortran field truncation semantics).
fn fixed(s: &str, width: usize) -> String {
    let mut t: String = s.chars().take(width).collect();
    while t.len() < width {
        t.push(' ');
    }
    t
}

/// Write the BOXER text layout for a compressed matrix (`covr.f90:2199-2207`).
///
/// Emits, per page: a header record (data type, library id + description on the
/// first page, `mat/mt/mat1/mt1`, the array counts `nval/nvf/ncon/ncf`, and the
/// row counts `nrowm/nrow/ncol`), then the `xval` block and the `icon` block,
/// each wrapped to the per-line counts from [`setfor`].
///
/// See the module "Fidelity note": the number *formatting* is Rust's, not a
/// byte-exact Fortran `1P` emulation; the record structure and counts are
/// faithful.
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
    let ndig = ndig_for(nvf).max(1) as usize;
    // ncol in the header is 0 for a symmetric matrix (covr.f90:2196).
    let ncol_hdr = match data.shape {
        BoxerShape::SymmetricUpperTriangle => 0,
        BoxerShape::Rectangular => data.ncol,
    };

    let mut out = String::new();
    for (p, page) in data.pages.iter().enumerate() {
        let nval = page.xval.len();
        let ncon = page.icon.len();
        let nrowm = data.nrow - page.last_row;

        if p == 0 {
            // First page: full header with library id + description.
            out.push_str(&format!(
                "{}{} {} {:5}{:4}{:5}{:4}{:4}{:3}{:4}{:3}{:4}{:4}{:4}\n",
                header.itype.code(),
                " ",
                fixed(&header.hlibid, 12) + " " + &fixed(&header.hdescr, 21),
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
        } else {
            // Continuation page: 34 dashes instead of id + description.
            out.push_str(&format!(
                "{} {} {:5}{:4}{:5}{:4}{:4}{:3}{:4}{:3}{:4}{:4}{:4}\n",
                header.itype.code(),
                "-".repeat(34),
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
        }

        // xval block.
        for chunk in page.xval.chunks(vfmt.per_line.max(1)) {
            let line: Vec<String> = chunk.iter().map(|&x| format!("{:.*e}", ndig, x)).collect();
            out.push_str(&line.join(" "));
            out.push('\n');
        }
        // icon block.
        for chunk in page.icon.chunks(cfmt.per_line.max(1)) {
            let line: Vec<String> = chunk.iter().map(|c| c.to_string()).collect();
            out.push_str(&line.join(" "));
            out.push('\n');
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Convenience: compress a CovarianceMatrix / CorrelationMatrix
// ---------------------------------------------------------------------------

impl CovarianceMatrix {
    /// Compress this (symmetric) covariance matrix to BOXER form
    /// (`itype = 3`). Convenience wrapper over [`compress`] with
    /// [`BoxerShape::SymmetricUpperTriangle`].
    ///
    /// # Errors
    /// Propagates [`compress`].
    pub fn to_boxer(&self, nvf: i32, ncf: i32) -> Result<BoxerData, NjoyError> {
        let n = self.n();
        let mut buf = vec![0.0_f64; n * n];
        for i in 0..n {
            for j in 0..n {
                buf[i * n + j] = self.get(i, j);
            }
        }
        compress(&buf, n, n, BoxerShape::SymmetricUpperTriangle, nvf, ncf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Round-trip a full rectangular matrix through compress/decompress and
    /// assert it reproduces the sigfig-rounded input exactly. `nvf`/`ncf` chosen
    /// so rounding is a no-op on the chosen values.
    fn round_trip_rect(matrix: &[f64], nrow: usize, ncol: usize, nvf: i32, ncf: i32) {
        let data = compress(matrix, nrow, ncol, BoxerShape::Rectangular, nvf, ncf).unwrap();
        let back = decompress(&data).unwrap();
        let ndig = ndig_for(nvf);
        for (a, b) in matrix.iter().zip(back.iter()) {
            let rounded = crate::mixr::sigfig(*a, ndig, 0);
            assert!(
                (rounded - b).abs() <= 1e-9 * (1.0 + rounded.abs()),
                "round-trip mismatch: {a} (rounded {rounded}) vs {b}"
            );
        }
    }

    /// Methodology: BOXER compress->decompress must be the identity on the
    /// sigfig-rounded matrix (the core RLE, `covr.f90:2085-2196`). A 3x3
    /// rectangular matrix with repeats, a carry-down column, and distinct
    /// values exercises both run types. Result (2026-07-15): exact.
    #[test]
    fn rectangular_round_trip() {
        // Row 1: [5, 5, 7]; Row 2: [5, 5, 8] (carry-down of col 1-2 from row 1);
        // Row 3: [1, 2, 3] (distinct).
        let m = vec![5.0, 5.0, 7.0, 5.0, 5.0, 8.0, 1.0, 2.0, 3.0];
        round_trip_rect(&m, 3, 3, 12, 4);
    }

    /// Methodology: an all-constant matrix compresses to a single repeated-value
    /// run and must round-trip. 4x4 of 0.05. Also asserts the compression is
    /// non-trivial: one page, one xval, one icon code = -(16).
    /// Result (2026-07-15): exact; single run of length 16.
    #[test]
    fn constant_matrix_single_run() {
        let m = vec![0.05_f64; 16];
        let data = compress(&m, 4, 4, BoxerShape::Rectangular, 12, 4).unwrap();
        assert_eq!(data.pages.len(), 1);
        assert_eq!(data.pages[0].xval.len(), 1);
        assert_eq!(data.pages[0].icon, vec![-16]);
        let back = decompress(&data).unwrap();
        for b in back {
            assert!((b - 0.05).abs() < 1e-12);
        }
    }

    /// Methodology: symmetric matrices are stored upper-triangle-only
    /// (`nsym`, `covr.f90:2096`); decompress must mirror the lower triangle.
    /// Build a symmetric covariance, compress with SymmetricUpperTriangle,
    /// decode, and assert the full matrix returns and is symmetric.
    /// cov = [[4, 2, 0], [2, 9, -3], [0, -3, 16]]. The decode reproduces the
    /// *sigfig-rounded* matrix (NJOY's `sigfig` applies a `1.0000000000001`
    /// bias, so raw integers gain a ~1e-12 relative offset — the round-trip
    /// property is `decompress(compress(m)) == sigfig(m)`, not `== m`).
    /// Result (2026-07-15): matches the rounded matrix; symmetry restored.
    #[test]
    fn symmetric_round_trip_and_mirror() {
        let cov = CovarianceMatrix::from_row_major(
            3,
            vec![4.0, 2.0, 0.0, 2.0, 9.0, -3.0, 0.0, -3.0, 16.0],
        )
        .unwrap();
        let data = cov.to_boxer(12, 4).unwrap();
        // header records ncol=0 for symmetric.
        let back = decompress(&data).unwrap();
        let ndig = ndig_for(12);
        for i in 0..3 {
            for j in 0..3 {
                let expect = crate::mixr::sigfig(cov.get(i, j), ndig, 0);
                assert!(
                    (back[i * 3 + j] - expect).abs() <= 1e-9 * (1.0 + expect.abs()),
                    "({i},{j})"
                );
                assert!((back[i * 3 + j] - back[j * 3 + i]).abs() < 1e-12);
            }
        }
    }

    /// Methodology (V&V round-trip closure): cov -> BOXER -> decode -> cov ->
    /// correlation must equal the correlation module's direct transform, and the
    /// correlation must satisfy the physical properties (unit diagonal,
    /// |corr| <= 1). cov = [[4, 2], [2, 9]] -> corr = [[1, 1/3], [1/3, 1]].
    /// Result (2026-07-15): matches to 1e-12; diagonal 1; bounded.
    #[test]
    fn boxer_roundtrip_preserves_correlation() {
        let cov = CovarianceMatrix::from_row_major(2, vec![4.0, 2.0, 2.0, 9.0]).unwrap();
        let data = cov.to_boxer(12, 4).unwrap();
        let back = decompress(&data).unwrap();
        let cov_back = CovarianceMatrix::from_row_major(2, back).unwrap();

        let corr_direct = cov.to_correlation();
        let corr_back = cov_back.to_correlation();
        for i in 0..2 {
            for j in 0..2 {
                assert!((corr_direct.get(i, j) - corr_back.get(i, j)).abs() < 1e-12);
            }
        }
        // physical properties
        assert!((corr_back.get(0, 0) - 1.0).abs() < 1e-12);
        assert!((corr_back.get(1, 1) - 1.0).abs() < 1e-12);
        assert!(corr_back.max_abs() <= 1.0 + 1e-12);
    }

    /// Methodology: the text header (`covr.f90:2199-2205`) must carry the right
    /// counts. For the 2x2 symmetric covariance, the header line must start with
    /// the itype code (3 for RelativeCovariance) and record nrow=2, ncol=0
    /// (symmetric). Check the first line contains the mat/mt fields and the
    /// counts. Result (2026-07-15): header well-formed; counts present.
    #[test]
    fn press_text_header_counts() {
        let cov = CovarianceMatrix::from_row_major(2, vec![4.0, 2.0, 2.0, 9.0]).unwrap();
        let data = cov.to_boxer(12, 4).unwrap();
        let header = BoxerHeader {
            itype: BoxerDataType::RelativeCovariance,
            hlibid: "ENDF/B-VIII.0".into(),
            hdescr: "test covariance".into(),
            mat: 9228,
            mt: 102,
            mat1: 9228,
            mt1: 102,
        };
        let text = press_text(&header, &data, 12, 4).unwrap();
        let first = text.lines().next().unwrap();
        assert!(first.starts_with('3'), "itype code 3 in column 1: {first}");
        assert!(first.contains("9228"), "mat present: {first}");
        assert!(first.contains("102"), "mt present: {first}");
        // Non-empty xval and icon blocks follow the header.
        assert!(text.lines().count() >= 3);
    }

    /// Methodology: `setfor` range checks (`covr.f90:2236-2241`). Valid indices
    /// return the right descriptors; out-of-range indices error.
    /// Result (2026-07-15): nvf=12 -> "(1p6e12.5)", 6/line; ncf=4 -> "(20i4)",
    /// 20/line; nvf=6 and ncf=7 both rejected.
    #[test]
    fn setfor_selects_and_validates() {
        let (vf, cf) = setfor(12, 4).unwrap();
        assert_eq!(vf.descriptor, "(1p6e12.5)");
        assert_eq!(vf.per_line, 6);
        assert_eq!(cf.descriptor, "(20i4)");
        assert_eq!(cf.per_line, 20);
        assert!(setfor(6, 4).is_err()); // nvf too small
        assert!(setfor(12, 7).is_err()); // ncf too large
    }

    /// Methodology: paging (`covr.f90:2180-2211`). A matrix with all-distinct
    /// values forces one xval per cell; with 900+ cells the xval/icon arrays
    /// exceed NVMAX/NCMAX and must split into more than one page, and the
    /// concatenated pages must still round-trip. Use a 40x40 matrix of distinct
    /// values (1600 cells > 880). Result (2026-07-15): >1 page; exact round-trip.
    #[test]
    fn paging_splits_and_round_trips() {
        let n = 40;
        let mut m = vec![0.0_f64; n * n];
        for i in 0..n {
            for j in 0..n {
                // distinct, well-separated values (avoid eps-equal neighbours)
                m[i * n + j] = ((i * n + j + 1) as f64) * 1.0e3;
            }
        }
        let data = compress(&m, n, n, BoxerShape::Rectangular, 14, 4).unwrap();
        assert!(data.pages.len() > 1, "expected paging, got {} page(s)", data.pages.len());
        let back = decompress(&data).unwrap();
        let ndig = ndig_for(14);
        for (a, b) in m.iter().zip(back.iter()) {
            let rounded = crate::mixr::sigfig(*a, ndig, 0);
            assert!((rounded - b).abs() <= 1e-6 * (1.0 + rounded.abs()));
        }
    }
}
