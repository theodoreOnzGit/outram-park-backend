// SPDX-License-Identifier: AGPL-3.0-only
//! **VIEWR on a log axis whose limits are 0, against NJOY2016.80's own PostScript** (gh:#1543).
//!
//! # Methodology
//!
//! `verification_and_validation/viewr/gh1543_beta_axis_zero/deck.txt` (the issue's deck: PLOTR
//! on ENDF/B-VIII.0 Beta6 H in H2O `S(alpha,beta)`, MF=7 MT=4 at 293.6 K, an alpha plot and
//! then a `beta` plot with automatic axes, then `viewr 45 46`) was run on 2026-11-24 by the
//! NJOY2016.80 binary (`ab27c64`, gfortran, `build2/njoy`) with
//! `tests/resources/tsl-HinH2O-ENDF8.0-Beta6.endf` as `tape20`. Its PLOTR file `tape45`
//! (`plotr_tape45.txt`, 246 lines) and its VIEWR PostScript `tape46`
//! (`njoy2016_viewr_tape46.ps`, 1 098 lines) are committed beside the deck. This test runs
//! [`viewr_postscript`] on NJOY's `tape45` and compares the PostScript with NJOY's `tape46`
//! **line by line**. Pass criterion: zero differing lines.
//!
//! # What the case is
//!
//! The `beta` plot has one point, `(395.26, 0)`, lin-log (`itype = 2`). `ascalv(4, 0, 0)`
//! (`viewr.f90:1526`) returns `(0, 0, 1, 0)`, so the y axis reaches `axis3`
//! (`graph.f90:874-890`) as a log axis with `amin = amax = 0`. `log10(0) = -inf`;
//! `int(-inf)` is `-2147483648` under gfortran, and `i=i-1` wraps silently to `2147483647`
//! (likewise `j`). NJOY2016.80 then goes on: the axis spans zero cycles, `yscale` divides 0 by
//! 0, and the page carries `NaN` coordinates and a `10**` label (34 `NaN` words on 13 lines). This port
//! panicked at the first decrement (`attempt to subtract with overflow`, release overflow
//! checks, `viewr/axis.rs`). The two decrements now use `wrapping_sub`, gfortran's
//! two's-complement wrap, and the rest of the routine is unchanged.
//!
//! # Prediction (written 2026-11-24 before the run, recorded on gh:#1543)
//!
//! Bit-identical to NJOY's `tape46`, `NaN` words included, provided the port's `f9.2` writer
//! prints a NaN as gfortran does.
//!
//! # Result (2026-11-24)
//!
//! **0 differing lines of 1 098**; the file is byte-identical to NJOY2016.80's (`cmp`), with
//! its 34 `NaN` words. Before the change the same input panicked. NJOY's page 2 is not valid
//! PostScript (`NaN` is not a number token); matching it is the port-is-the-oracle choice, not a
//! claim that the plot is usable.

use njoy_outram_park_fork::viewr::viewr_postscript;

const PLOT: &str =
    include_str!("../verification_and_validation/viewr/gh1543_beta_axis_zero/plotr_tape45.txt");
const NJOY_PS: &str = include_str!(
    "../verification_and_validation/viewr/gh1543_beta_axis_zero/njoy2016_viewr_tape46.ps"
);

#[test]
fn log_axis_with_zero_limits_matches_njoy2016_postscript() {
    let ours = viewr_postscript(PLOT).expect("VIEWR runs through NJOY's own plot file");
    let (a, b): (Vec<&str>, Vec<&str>) = (ours.lines().collect(), NJOY_PS.lines().collect());
    let diff: Vec<usize> = (0..a.len().max(b.len())).filter(|&i| a.get(i) != b.get(i)).collect();
    println!(
        "gh:#1543: {} differing lines of {}; first {:?}",
        diff.len(),
        b.len(),
        diff.first().map(|&i| (i + 1, a.get(i).copied(), b.get(i).copied()))
    );
    assert_eq!(b.len(), 1098, "the committed NJOY PostScript changed");
    assert!(diff.is_empty(), "VIEWR PostScript differs from NJOY2016.80 on gh:#1543's deck");
    // Not vacuous: the zero-limit axis is on the page, as NJOY writes it.
    assert_eq!(ours.matches("NaN").count(), 34);
}
