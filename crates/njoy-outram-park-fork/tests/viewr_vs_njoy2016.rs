// SPDX-License-Identifier: AGPL-3.0-only
//! **VIEWR features no upstream deck reaches, against the NJOY2016.80 binary** (gh:#1369).
//!
//! The ACER plot files of upstream's regression decks use a narrow part of VIEWR's input
//! (see `verification_and_validation/viewr/README.md`): no error bars, no tags, no alternate
//! axis, no hatching, no titles-only plot, no portrait page, no negative character size. This
//! test writes plot-command files that do, runs the NJOY2016.80 binary on each as a one-card
//! deck (`viewr / 20 21 / stop`, the file on unit 20) and runs [`viewr_postscript`] on the
//! same text, and compares the PostScript **line by line**.
//!
//! The binary is located by `NJOY2016_ROOT` (else `upstream_source/NJOY2016`); the test skips
//! with a message when it is absent.
//!
//! Prediction (before the run): identical. The new code paths are `legndb`, `tagit`, `erbar`,
//! `set3d` and the alternate-axis branch of `set2d`, plus `hatch`; all arithmetic is translated
//! in Fortran operation order.
//!
//! Results: see each test's doc comment.

use std::path::PathBuf;
use std::process::Command;

use njoy_outram_park_fork::viewr::viewr_postscript;

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

/// Run NJOY2016.80's VIEWR on `plot`; its PostScript file, or `None` (skip) without the binary.
fn njoy_viewr(tag: &str, plot: &str) -> Option<String> {
    let bin = njoy_root().join("build/njoy");
    if !bin.exists() {
        eprintln!("skip: {} not present (set NJOY2016_ROOT)", bin.display());
        return None;
    }
    let tmp = std::env::temp_dir().join(format!("njoy_deck_vv_{tag}_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("tape20"), plot).unwrap();
    std::fs::write(tmp.join("deck"), "viewr\n 20 21\nstop\n").unwrap();
    let st = Command::new("sh").arg("-c").arg(format!("{} < deck > log 2>&1", bin.display())).current_dir(&tmp).status().unwrap();
    assert!(st.success(), "NJOY2016 binary failed on {tag}");
    let ps = std::fs::read_to_string(tmp.join("tape21")).unwrap();
    let _ = std::fs::remove_dir_all(&tmp);
    Some(ps)
}

fn check(tag: &str, plot: &str) -> Option<usize> {
    let theirs = njoy_viewr(tag, plot)?;
    let ours = viewr_postscript(plot).unwrap();
    let (a, b): (Vec<&str>, Vec<&str>) = (ours.lines().collect(), theirs.lines().collect());
    let diff: Vec<usize> = (0..a.len().max(b.len())).filter(|&i| a.get(i) != b.get(i)).collect();
    println!("{tag}: {} differing lines of {}; first {:?}", diff.len(), b.len(), diff.first().map(|&i| (i + 1, a.get(i).copied(), b.get(i).copied())));
    assert!(diff.is_empty(), "{tag}: PostScript differs from NJOY2016.80");
    Some(b.len())
}

/// Landscape page, light-yellow window: a log-log plot with a legend block (`ileg = 1`), three
/// curves (symbols every point and every second, hatching 11, coloured shading 45 with an
/// invisible border), y and x error bars; then a second set of axes on the same page (`iplot =
/// -1`, window 6 x 4 in at (0.5, 0.5), lin-lin, grid lines, tags with arrows, `ileg = 2`); then
/// a titles-only plot.
const PLOT_2D: &str = "1 2 .30 2/
 1 3/
'<f>irst title'/
'second <t>itle'/
 4 0 3 1/
 1.e-2 1.e3/
'<e>nergy (eV)'/
 1.e-3 1.e5/
'<c>ross section (barns)'/
 0/
 1 2 0 1 1 0/
'curve <o>ne'/
 0/
 1.e-2 5.e0 2.e-1 3.e-1 0 0/
 1.e-1 3.e1 0 0 0.02 0.03/
 1. 4.e2 1.e1 1.e1 .3 .3/
 1.e1 7.e3/
 5.e1 2.e4 0 0 0 0/
 2.e2 1.e3/
/
 2 0 1 1/
 0/
 2 5 1 2 2 11/
'curve <t>wo'/
 0/
 1.e-2 5.e-1/
 1.e0 3.e0/
 1.e2 5.e3/
/
 3 0 1 1/
 0/
 -3 9 2 3 0 45/
'curve three, [a]lpha'/
 0/
 2.e-2 1.e0/
 3.e0 1.e2/
 4.e2 2.e1/
 6.e2 1.e1/
/
 -1 2 1 1 .5 .5 6 4 0/
'<s>ub plot'/
'<t>ags and a grid'/
 1 0 1 2/
 0 0 0/
'<x> axis'/
 0 0 0/
'<y> axis'/
 0/
 0 0 0 1 2 21/
'first'/
 3 8 4/
 0
 1 2/
 2 5/
 4 3/
 5 7/
/
 2 0 1 1/
 0/
 4 12 3 4 1 31/
'second'/
 2 1 6/
 0/
 1 1/
 3 6/
 5 2/
/
 -1 0 1 1 7 4.5 3 2 30/
'<o>nly a title'/
''/
 0 0 0 0/
 99/
";

/// **2-d features**: legend block, tags, error bars, hatching, shading, symbols, rotated and
/// positioned windows, titles only.
#[test]
fn two_d_features_match_njoy() {
    let _ = check("two_d", PLOT_2D);
}

/// Portrait page, character size as a fraction of the window (`size < 0`), lin-lin with a user
/// scale (`xstep`, `ystep` given), a right-hand (alternate) axis with its own curves (`iplot =
/// -2`), then `itype = 2` and `itype = 3` plots.
const PLOT_PORTRAIT: &str = "0 1 -.05 5/
 1 0/
'<p>ortrait'/
'<u>ser scales'/
 1 1 2 0/
 0 10 2/
'<t>ime'/
 0 100 20/
'<v>alue'/
 0 0 0/
'<r>ight'/
 0/
 0 0 0 0 1 0/
 0/
 0 5/
 2 30/
 5 60/
 9 20/
/
 -2 0 1 1/
 0/
 5 3 3 2 1 0/
 0/
 0 1/
 4 3/
 10 4/
/
 1 0/
'<l>og y'/
''/
 2 0 2 0/
 0 0 0/
'<x>'/
 1.e-3 1.e2/
'<y>'/
 0/
 0 0 0 5 1 0/
 0/
 1 1.e-2/
 5 1.e0/
 9 1.e1/
/
 1 0/
'<l>og x'/
''/
 3 0 2 0/
 1. 1.e4/
'<x>'/
 0 0 0/
'<y>'/
 0/
 -1 7 4 6 1 0/
 0/
 10 5/
 100 15/
 1000 8/
/
 99/
";

/// **Page set-ups**: portrait, negative size, user scales, alternate axis, `itype` 2 and 3.
#[test]
fn portrait_alternate_axis_and_axis_types_match_njoy() {
    let _ = check("portrait", PLOT_PORTRAIT);
}

/// 3-d plots (`itype < 0`, `nform = 1`): a family of curves `z(x)` at stepped `y` with the default
/// viewpoint and work box, linear z (`jtype = 1`) and a grid; then log x and y with `jtype = 2`,
/// a viewpoint of the opposite side (`x3 < 0`) and a user work box.
const PLOT_3D: &str = "1 2 .25 1/
 1 2/
'<t>hree-d'/
'<s>urface'/
 -1 1 1 0/
 0 0 0/
'<x>'/
 0 0 0/
'<y>'/
 0 0 0/
'<z>'/
 0/
 15. -15. 15. 2.5 6.5 2.5/
 1/
 0.5/
 0 1/
 1 2/
 2 5/
 3 3/
/
 1.5/
 0 2/
 1 4/
 2 6/
 3 5/
/
 2.5/
 0 1/
 1 5/
 2 7/
 3 2/
/
/
 1 2/
'<l>og'/
'<s>urface'/
 -4 2 2 0/
 0 0 0/
'<x>'/
 0 0 0/
'<y>'/
 0 0 0/
'<z>'/
 0/
 12. -10. 8. -2.5 5.5 2.0/
 1/
 1.0/
 1 1.e1/
 10 1.e3/
 100 1.e2/
/
 10.0/
 1 5.e1/
 10 2.e3/
 100 4.e1/
/
 100.0/
 1 2.e1/
 10 8.e2/
 100 9.e1/
/
/
 99/
";

/// **3-d**: `set3d`, `init3`, `axis3` in perspective, `grid3`, `curv3`.
#[test]
fn three_d_surface_plots_match_njoy() {
    let _ = check("three_d", PLOT_3D);
}
