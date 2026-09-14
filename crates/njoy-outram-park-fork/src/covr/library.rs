// Ported from NJOY2016 `src/covr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! COVR's **library option** (`nout > 0`): ERRORR tape in, BOXER library out.
//!
//! This is the `nout > 0` path of `subroutine covr` (`covr.f90:338-474`) with
//! `subroutine corr` (`covr.f90:578-718`) underneath it:
//!
//! 1. classify the tape (`covr.f90:319-333`, [`ErrorrTapeKind`]);
//! 2. per case, expand `mt <= 0` into every `(mt, mt1 >= mt)` pair of the
//!    MTs present (`expndo`, `covr.f90:508-576`), else take the card as is;
//! 3. per pair, `corr`: read the cross block (skip a null one), the column
//!    reaction's own auto block for `rsd_y`, the row reaction's for `rsd_x`,
//!    the cross block again as the matrix, and — for `matype = 4` only —
//!    divide by `rsd_x(i) rsd_y(j)` (`covr.f90:600-711`);
//! 4. `press` the group bounds (first written pair only), then for an auto
//!    pair the cross section and `rsd` vectors, then the matrix
//!    (`covr.f90:425-459`).
//!
//! In the library option `irelco` is forced to 1 — "do not translate abs.
//! covariances to rel. in library option, thus allowing stand. devs. and
//! covariances to track the errorr output" (`covr.f90:240-243`) — so an
//! absolute ERRORR tape yields absolute covariances and *absolute* standard
//! deviations under `itype = 2`. Zero-cross-section zeroing still applies.
//!
//! Upstream's scratch-file dance (`nscr1`/`nscr2`, the `icall` restart when a
//! second material has more groups) is replaced by in-memory matrices; the
//! group-count agreement check (`covr.f90:715-716`) is kept.

use crate::covr::boxer::{compress, BoxerDataType, BoxerHeader, BoxerShape};
use crate::covr::boxer_text::press_text;
use crate::covr::covard::{expand_mt_pairs, ErrorrCovarianceSection};
use crate::covr::input::{CovarianceForm, LibraryOptions, MatrixOutputType, ReactionSelector};
use crate::covr::tape::{present_mts, read_covariance_section, ErrorrTapeKind};
use crate::endf::tape::Tape;
use crate::NjoyError;

/// What happened to one `(mat,mt,mat1,mt1)` pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairReport {
    /// Row material.
    pub mat: i32,
    /// Row reaction.
    pub mt: i32,
    /// Column material.
    pub mat1: i32,
    /// Column reaction.
    pub mt1: i32,
    /// `true` when the matrix was written, `false` for "null covariance
    /// matrix.  output suppressed." (`covr.f90:433-434`).
    pub written: bool,
}

/// The library option's products.
#[derive(Debug, Clone, PartialEq)]
pub struct CovrLibraryOutput {
    /// The BOXER text upstream writes to `nout`.
    pub text: String,
    /// One entry per pair processed, in order.
    pub pairs: Vec<PairReport>,
    /// The `mess` lines upstream prints (`expndo` stripping, `covard`
    /// zero-cross-section warnings).
    pub messages: Vec<String>,
}

/// `corr`'s product for one pair in the library option.
struct PairMatrices {
    /// `x` — group boundaries \[eV\] of the column material (`covr.f90:624-627`).
    x: Vec<f64>,
    /// `xx` — the last `covard` call's row cross section (the pair's own for
    /// an auto pair).
    xx: Vec<f64>,
    /// `rsdx` — row standard deviations.
    rsdx: Vec<f64>,
    /// `cf` — the dense matrix (covariance, or correlation for `matype=4`).
    cf: Vec<f64>,
    ixmax: usize,
}

/// `sqrt` of a diagonal element as the library path does it
/// (`covr.f90:645-648, 663-666`: no sign guard). A negative diagonal would be
/// `NaN` upstream; it is mapped to 0 here, as the plot path does
/// (`covr.f90:636-641`), and never occurs on an ERRORR tape.
fn rsd_of(diag: f64) -> f64 {
    if diag > 0.0 {
        diag.sqrt()
    } else {
        0.0
    }
}

/// `covard` in the library option: read, densify with `irelco = 1`, collect
/// any zero-cross-section warning (`covr.f90:918-925`). `None` for a
/// placeholder section (treated as null).
fn covard(
    tape: &Tape,
    kind: ErrorrTapeKind,
    mat: i32,
    mt: i32,
    mat1: i32,
    mt1: i32,
    messages: &mut Vec<String>,
) -> Result<Option<(ErrorrCovarianceSection, Vec<f64>, bool)>, NjoyError> {
    let Some(sec) = read_covariance_section(tape, kind, mat, mt, mat1, mt1)? else {
        return Ok(None);
    };
    let dense = sec.to_dense(CovarianceForm::Relative)?;
    if dense.zeroed_for_zero_xsec > 0 {
        messages.push(format!(
            "covard: due to zero xsec, covariance zeroed for mt={mt:3} mt1={mt1:3} ({} elements)",
            dense.zeroed_for_zero_xsec
        ));
    }
    let n = sec.ixmax;
    let mut cf = vec![0.0; n * n];
    for i in 0..n {
        for j in 0..n {
            cf[i * n + j] = dense.matrix.get(i, j);
        }
    }
    Ok(Some((sec, cf, !dense.is_null)))
}

/// One `(mat,mt) x (mat1,mt1)` pair after the card-4 defaults are applied.
#[derive(Debug, Clone, Copy)]
struct Pair {
    mat: i32,
    mt: i32,
    mat1: i32,
    mt1: i32,
}

/// `subroutine corr` for the library option (`covr.f90:578-718`). `None`
/// means `izero = 0`: the pair is suppressed.
fn corr(
    tape: &Tape,
    kind: ErrorrTapeKind,
    matype: MatrixOutputType,
    pair: Pair,
    messages: &mut Vec<String>,
) -> Result<Option<PairMatrices>, NjoyError> {
    let Pair { mat, mt, mat1, mt1 } = pair;
    let auto = mat == mat1 && mt == mt1;
    // covr.f90:600-605 — a null cross matrix is dropped before any work.
    if !auto {
        match covard(tape, kind, mat, mt, mat1, mt1, messages)? {
            Some((_, _, true)) => {}
            _ => return Ok(None),
        }
    }
    // covr.f90:607, 624-627, 645-648 — column auto block: x, rsdy (= rsdx).
    let Some((col_sec, col_cf, col_nonnull)) = covard(tape, kind, mat1, mt1, mat1, mt1, messages)?
    else {
        return Ok(None);
    };
    let ixmax = col_sec.ixmax;
    let x = col_sec.group_boundaries.clone();
    let rsdy: Vec<f64> = (0..ixmax).map(|i| rsd_of(col_cf[i * ixmax + i])).collect();
    let (xx, rsdx, cf, nonnull) = if auto {
        (col_sec.xx.clone(), rsdy.clone(), col_cf, col_nonnull)
    } else {
        // covr.f90:653, 663-666 — row auto block for rsdx.
        let Some((row_sec, row_cf, _)) = covard(tape, kind, mat, mt, mat, mt, messages)? else {
            return Ok(None);
        };
        if row_sec.ixmax != ixmax {
            return Err(NjoyError::EndfParse(
                "corr: group structures do not agree.".into(),
            ));
        }
        let rsdx: Vec<f64> = (0..ixmax).map(|i| rsd_of(row_cf[i * ixmax + i])).collect();
        // covr.f90:669 — the cross block again, as the matrix.
        let Some((cross_sec, cross_cf, nonnull)) =
            covard(tape, kind, mat, mt, mat1, mt1, messages)?
        else {
            return Ok(None);
        };
        if cross_sec.ixmax != ixmax {
            return Err(NjoyError::EndfParse(
                "corr: group structures do not agree.".into(),
            ));
        }
        (cross_sec.xx.clone(), rsdx, cross_cf, nonnull)
    };
    if !nonnull {
        return Ok(None);
    }
    let mut cf = cf;
    // covr.f90:692-706 — correlations only for matype != 3.
    if matype == MatrixOutputType::Correlations {
        for (row, &sx) in cf.chunks_mut(ixmax).zip(&rsdx) {
            for (c, &sy) in row.iter_mut().zip(&rsdy) {
                let d = sx * sy;
                if *c != 0.0 && d != 0.0 {
                    *c /= d;
                } else {
                    *c = 0.0;
                }
            }
        }
    }
    Ok(Some(PairMatrices {
        x,
        xx,
        rsdx,
        cf,
        ixmax,
    }))
}

/// Run COVR's library option on a parsed ERRORR tape (`covr.f90:338-474`,
/// `nout > 0`).
///
/// `cases` are the card-4 selectors; `mt <= 0` expands to every pair of the
/// MTs present for `mat` minus the strip list `(-mt, -mat1, -mt1)`
/// (`expndo`), `mat1 = 0` means `mat`, `mt1 = 0` means `mt`. The result's
/// `text` is byte-identical to NJOY2016's `nout` for the same deck
/// (`tests/covr_boxer_golden.rs`).
///
/// # Errors
/// Tape-shape errors from [`crate::covr::tape`], "group structures do not
/// agree" when `mat1` has a different group count, and the `setfor`/`press`
/// range checks.
pub fn run_library(
    tape: &Tape,
    opts: &LibraryOptions,
    cases: &[ReactionSelector],
) -> Result<CovrLibraryOutput, NjoyError> {
    let kind = ErrorrTapeKind::from_tape(tape)?;
    let matype = opts.matrix_type;
    // covr.f90:246-253 — hinpid is a character(6), hdescr a character(21).
    let hinpid: String = {
        let mut s: String = opts.lib_id.chars().take(6).collect();
        while s.chars().count() < 6 {
            s.push(' ');
        }
        s
    };
    let hdescr: String = opts.description.chars().take(21).collect();

    let mut text = String::new();
    let mut pairs = Vec::new();
    let mut messages = Vec::new();
    let mut hlibid = String::new();

    for (n, sel) in cases.iter().enumerate() {
        let mat = sel.mat;
        // covr.f90:376-377 — expand the mt-mt1 list.
        let pair_list: Vec<(i32, i32, i32)> = if sel.mt <= 0 {
            let present = present_mts(tape, mat, kind.mf35);
            let mstrip = [-sel.mt, -sel.mat1, -sel.mt1];
            for &mtn in &present {
                if crate::covr::is_mt_stripped(mtn, mstrip) {
                    messages.push(format!("expndo: mt={mtn:3} stripped from list."));
                }
            }
            expand_mt_pairs(&present, mstrip)
                .into_iter()
                .map(|(a, b)| (a, 0, b))
                .collect()
        } else {
            vec![(sel.mt, sel.mat1, sel.mt1)]
        };

        for (ne, &(mt, mat1_raw, mt1_raw)) in pair_list.iter().enumerate() {
            // covr.f90:380-383
            let mat1 = if mat1_raw <= 0 { mat } else { mat1_raw };
            let mt1 = if mt1_raw == 0 { mt } else { mt1_raw };
            let auto = mat1 == mat && mt1 == mt;

            let pair = Pair { mat, mt, mat1, mt1 };
            let Some(pm) = corr(tape, kind, matype, pair, &mut messages)? else {
                pairs.push(PairReport {
                    mat,
                    mt,
                    mat1,
                    mt1,
                    written: false,
                });
                continue;
            };
            let ixmax = pm.ixmax;

            // covr.f90:436-439 — library id on the first pair of each case.
            if ne == 0 {
                let tag = match matype {
                    MatrixOutputType::Covariances => 'a',
                    MatrixOutputType::Correlations => 'b',
                };
                hlibid = format!("{hinpid}-{tag}-{ixmax:3}");
            }
            let header = |itype: BoxerDataType| BoxerHeader {
                itype,
                hlibid: hlibid.clone(),
                hdescr: hdescr.clone(),
                mat,
                mt,
                mat1,
                mt1,
            };

            // covr.f90:440-446 — group structure for the first written pair
            // of the first case only.
            if n == 0 && ne == 0 {
                let d = compress(&pm.x, ixmax + 1, 1, BoxerShape::Rectangular, 10, 3)?;
                text.push_str(&press_text(
                    &header(BoxerDataType::GroupBoundaries),
                    &d,
                    10,
                    3,
                )?);
            }
            // covr.f90:447-453 — cross section and rsd vectors for an auto pair.
            if auto {
                let d = compress(&pm.xx, ixmax, 1, BoxerShape::Rectangular, 10, 3)?;
                text.push_str(&press_text(
                    &header(BoxerDataType::CrossSections),
                    &d,
                    10,
                    3,
                )?);
                let d = compress(&pm.rsdx, ixmax, 1, BoxerShape::Rectangular, 10, 3)?;
                text.push_str(&press_text(
                    &header(BoxerDataType::RelativeStdDev),
                    &d,
                    10,
                    3,
                )?);
            }
            // covr.f90:454-461 — the matrix.
            let nvf = match matype {
                MatrixOutputType::Covariances => 10,
                MatrixOutputType::Correlations => 7,
            };
            let ncf = if ixmax <= 30 {
                4
            } else if ixmax <= 100 {
                5
            } else {
                6
            };
            let (shape, itype) = (
                if auto {
                    BoxerShape::SymmetricUpperTriangle
                } else {
                    BoxerShape::Rectangular
                },
                match matype {
                    MatrixOutputType::Covariances => BoxerDataType::RelativeCovariance,
                    MatrixOutputType::Correlations => BoxerDataType::Correlation,
                },
            );
            let d = compress(&pm.cf, ixmax, ixmax, shape, nvf, ncf)?;
            text.push_str(&press_text(&header(itype), &d, nvf, ncf)?);
            pairs.push(PairReport {
                mat,
                mt,
                mat1,
                mt1,
                written: true,
            });
        }
    }
    Ok(CovrLibraryOutput {
        text,
        pairs,
        messages,
    })
}
