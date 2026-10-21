//! **V&V: the unresolved-range kernel at infinite dilution, against NJOY2016 —
//! and the explanation of the residual.**
//!
//! # Why this test exists
//!
//! `reconr::urr` reconstructs the unresolved range by calling
//! [`unresolved_cross_sections`] — the ETOX kernel UNRESR already uses — at a
//! single dilution of `1e10`. That choice rests on a claim: **RECONR's own
//! `csunr1`/`csunr2` (`reconr.f90:3826-4317`, 491 lines) need not be ported,
//! because the ETOX kernel reduces to the same numbers at infinite dilution.**
//!
//! That claim was originally measured in a throwaway probe and written only
//! into a doc comment. This file exists so it is *executed* instead: a
//! load-bearing code-to-code result that nothing re-checks is one refactor away
//! from being quietly false.
//!
//! # Methodology
//!
//! **Quantity.** MT=18 (fission) from
//! `unresolved_cross_sections(sig0 = [1e10], sigbkg = 0)`, evaluated directly —
//! no reconstruction, no background — at every point of NJOY2016's own PENDF
//! grid inside U-234's unresolved range, `1.5e3 .. 1.0e5 eV`.
//!
//! MT=18 is the probe because the evaluation's MF=3 MT=18 carries **no
//! background below the top of the window** (verified in
//! [`mf3_background_is_absent_below_the_top_of_the_unresolved_window`]), so
//! NJOY's PENDF value there is the pure unresolved cross section with nothing
//! added — the cleanest possible comparison.
//!
//! The one boundary detail, found by that test rather than assumed: MF=3 MT=18
//! **does** have a point at exactly `1.0e5 eV` (0.022 b), the URR's upper
//! bound, where the fast-range fission background begins. Every energy sampled
//! here lies strictly below it — the topmost is NJOY's shaded `9.99999e4` —
//! so the probe is unaffected, but a future window that reached `1e5` itself
//! would no longer be comparing like with like.
//!
//! **Reference.** NJOY2016 `ac5adf5f` (2016.79), gfortran 13.3.0, from the deck
//! at `reference-data/reconr/u234-ENDF8.0-0K-err0.001.njoy-input`.
//!
//! # The finding: NJOY does not evaluate every point it writes
//!
//! Of NJOY's 34 grid points in the URR, **26 agree with this kernel to ~1e-7**
//! — the 7-figure floor of NJOY's printed output. The other 8 do not, by up to
//! 6.2e-3, and they are not scattered randomly: 7.0e3 disagrees while 7.2e3 is
//! exact, 7.0e4 disagrees while 7.2e4 is exact.
//!
//! **Every one of those 8 is reproduced by lin-lin interpolation of NJOY's own
//! neighbouring exact points**, to between 2.9e-7 and 1.4e-16:
//!
//! | E (eV) | NJOY | lin-lin of NJOY's neighbours | rel |
//! |---|---|---|---|
//! | 7.00000e3 | 1.228344e-2 | from 6.0e3, 7.2e3 | 1.4e-7 |
//! | 4.368748e4 | 1.485438e-2 | from 4.0e4, 5.0e4 | 2.9e-7 |
//! | 4.368749e4 | 1.485437e-2 | from 4.0e4, 5.0e4 | 2.1e-7 |
//! | 4.51800e4 | 1.447637e-2 | from 4.0e4, 5.0e4 | 2.4e-7 |
//! | 5.25000e4 | 1.277159e-2 | from 5.0e4, 6.0e4 | 1.4e-16 |
//! | 7.00000e4 | 1.429696e-2 | from 6.0e4, 7.2e4 | 2.3e-7 |
//! | 8.00000e4 | 1.669535e-2 | from 7.2e4, 8.5e4 | 2.8e-7 |
//! | 9.00000e4 | 1.866262e-2 | from 8.5e4, 1.0e5 | 1.2e-7 |
//!
//! That is `genunr`/`sigunr` working exactly as documented: `genunr`
//! (`reconr.f90:1628-1735`) tabulates the dilute cross sections on the `eunr`
//! grid as MF=2/MT=152, and `sigunr` (`:1737-1769`) **interpolates that table**
//! when a PENDF energy falls between its points. The 8 outliers are PENDF grid
//! energies contributed by other MF=3 sections, filled from the table rather
//! than evaluated.
//!
//! **So the residual is NJOY's interpolation error, not this port's.** Where
//! the two differ, this port evaluates the physics at the point and NJOY
//! interpolates to it, which makes ours the more accurate value. Three
//! competing explanations were tested and refuted before this one was accepted:
//!
//! - *NJOY interpolates its table with a different ENDF law.* Refuted — no
//!   single law fits: at 4.368748e4 log-lin was closest for MT=18 (3.7e-4) but
//!   lin-log for MT=102 (1.3e-3), and neither reached NJOY's value.
//! - *The unresolved parameters are interpolated differently.* Refuted by
//!   reading upstream: `intr` (`unresr.f90:1261-1288`) interpolates D/GX/GN0/
//!   GG/GF with `terp1(…, int)` where `int` has just been **forced to 2**
//!   (`:1057-1058`, it reads the file's flag and overwrites it), which is
//!   exactly what [`crate::unresr::interp_case_c`] does.
//! - *Parameter energies are special.* Refuted by the data: 1.7e3, 1.0e4,
//!   3.0e4 and 5.0e4 are **not** parameter energies and are exact, while
//!   7.0e3 is not a parameter energy and is interpolated.
//!
//! # What this does NOT establish
//!
//! Only MT=18 on one material. MT=102/2/1 show the same pattern at the same
//! energies but are not asserted here — MT=1 and MT=2 carry MF=3 background in
//! this window, so they are not a clean probe of the kernel alone.
//!
//! This is verification against NJOY2016, not validation against measurement.

use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::{reference_file, reference_file_or_skip};
use njoy_outram_park_fork::unresr::mf2::parse_lru2_ranges;
use njoy_outram_park_fork::unresr::unresolved_cross_sections;
use njoy_outram_park_fork::unresr::wfun::WTable;

const MAT: i32 = 9225;
const URR_LO: f64 = 1.5e3;
const URR_HI: f64 = 1.0e5;
/// Infinite dilution — `genunr`'s `big = 1.e10_kr` (`reconr.f90:1644`).
const SIG0: f64 = 1.0e10;
/// Agreement counted as "NJOY evaluated this point": its output carries 7
/// significant figures, so this is a little looser than that floor.
const EXACT: f64 = 1.0e-5;

fn njoy_mt(tape: &Tape, mt: i32) -> Option<Vec<(f64, f64)>> {
    let sec = tape.section(MAT, 3, mt)?;
    let mut c = SectionCursor::new(&sec.rows);
    c.read_cont().ok()?;
    Some(c.read_tab1().ok()?.pairs)
}

/// `(energy, NJOY MT=18, our kernel MT=18)` at every NJOY grid point in the URR.
fn samples() -> Option<Vec<(f64, f64, f64)>> {
    let ep = reference_file("endf", "n-092_U_234-ENDF8.0.endf")?;
    let pp = reference_file_or_skip(
        "reconr",
        "u234-ENDF8.0-0K-err0.001.pendf",
        "U-234 unresolved RECONR golden",
    )?;
    let tape = Tape::read_file(&ep).ok()?;
    let sec = tape.section(MAT, 2, 151)?;
    let ranges = parse_lru2_ranges(&sec.rows[1..]).ok()?;
    let range = ranges.first()?;
    let njt = Tape::read_file(&pp).ok()?;
    let pairs = njoy_mt(&njt, 18)?;
    let table = WTable::new();

    let mut out = Vec::new();
    let mut prev = f64::NAN;
    for &(e, v) in &pairs {
        if e == prev {
            // NJOY writes duplicate energies at discontinuities; both are kept
            // here deliberately -- 4.368748e4/4.368749e4 are one such pair and
            // both are interpolated, which is part of the finding.
        }
        prev = e;
        if !(URR_LO..=URR_HI).contains(&e) {
            continue;
        }
        let ours = unresolved_cross_sections(
            std::slice::from_ref(range),
            e,
            0.0,
            &[SIG0],
            [0.0; 4],
            &table,
        )
        .ok()?;
        out.push((e, v, ours.first().map_or(0.0, |r| r[2])));
    }
    Some(out)
}

/// The claim `reconr::urr` rests on: at infinite dilution the ETOX kernel *is*
/// RECONR's dilute answer, so `csunr1`/`csunr2` need not be ported.
///
/// Asserted as: **every** NJOY grid point is explained, either by our kernel
/// matching it, or by it being a lin-lin interpolation of NJOY's own matched
/// neighbours. That formulation is deliberately non-circular — it does not
/// hard-code which points are which, so if NJOY's grid or this kernel changes,
/// the test re-derives the split instead of asserting a stale list.
#[test]
fn every_njoy_unresolved_point_is_either_reproduced_or_is_njoys_own_interpolation() {
    let Some(pts) = samples() else {
        eprintln!("skipping: U-234 evaluation or RECONR golden absent");
        return;
    };
    assert!(pts.len() > 25, "only {} NJOY points in the URR", pts.len());

    let matched: Vec<bool> = pts
        .iter()
        .map(|&(_, njoy, ours)| (ours - njoy).abs() / njoy.abs() < EXACT)
        .collect();
    let n_matched = matched.iter().filter(|&&m| m).count();
    println!(
        "  {n_matched} of {} NJOY grid points reproduced directly by the kernel",
        pts.len()
    );
    assert!(
        n_matched * 4 >= pts.len() * 3,
        "only {n_matched} of {} points match; the kernel no longer reproduces \
         NJOY at infinite dilution, which is the premise reconr::urr rests on",
        pts.len()
    );

    let mut unexplained = Vec::new();
    for i in 0..pts.len() {
        if matched[i] {
            continue;
        }
        let lo = (0..i).rev().find(|&k| matched[k]);
        let hi = (i + 1..pts.len()).find(|&k| matched[k]);
        let (Some(a), Some(b)) = (lo, hi) else {
            unexplained.push((pts[i].0, f64::NAN));
            continue;
        };
        let (ea, va) = (pts[a].0, pts[a].1);
        let (eb, vb) = (pts[b].0, pts[b].1);
        let lin = va + (pts[i].0 - ea) / (eb - ea) * (vb - va);
        let dev = (lin - pts[i].1).abs() / pts[i].1.abs();
        println!(
            "  E={:.6e} differs by {:.2e}; NJOY's own lin-lin from [{ea:.4e},{eb:.4e}] \
             reproduces it to {dev:.2e}",
            pts[i].0,
            (pts[i].2 - pts[i].1).abs() / pts[i].1.abs()
        );
        if dev >= EXACT {
            unexplained.push((pts[i].0, dev));
        }
    }

    assert!(
        unexplained.is_empty(),
        "these NJOY points match neither this kernel nor an interpolation of \
         NJOY's own neighbours: {unexplained:?}. That would be a real physics \
         disagreement rather than NJOY's table interpolation -- see this file's \
         module doc, which rules out three other explanations."
    );
}

/// The premise of the probe above: MF=3 MT=18 carries no background below the
/// top of the unresolved window, so NJOY's PENDF value there is the pure
/// unresolved cross section.
///
/// The interval is deliberately **half-open**. U-234's MF=3 MT=18 begins at
/// exactly `1.0e5 eV` with 0.022 b — the URR's upper bound, where the
/// fast-range fission background starts. Every energy the kernel probe samples
/// is strictly below that (the topmost is NJOY's shaded `9.99999e4`), so the
/// comparison is clean; asserting the closed interval would fail on a boundary
/// point that does not affect it. If background ever appears *inside* the
/// window, the comparison silently stops being like-for-like, and that is what
/// this guards.
#[test]
fn mf3_background_is_absent_below_the_top_of_the_unresolved_window() {
    let Some(ep) = reference_file("endf", "n-092_U_234-ENDF8.0.endf") else {
        eprintln!("skipping: U-234 evaluation absent");
        return;
    };
    let tape = Tape::read_file(&ep).expect("evaluation parses");
    let pairs = njoy_mt(&tape, 18).expect("MF=3 MT=18 present");
    let inside: Vec<(f64, f64)> = pairs
        .iter()
        .copied()
        .filter(|&(e, _)| (URR_LO..URR_HI).contains(&e))
        .filter(|&(_, v)| v != 0.0)
        .collect();
    println!(
        "  MF=3 MT=18 non-zero points inside [{URR_LO:.1e}, {URR_HI:.1e}): {}",
        inside.len()
    );
    // Record the boundary point rather than merely excluding it.
    let at_top: Vec<(f64, f64)> = pairs
        .iter()
        .copied()
        .filter(|&(e, v)| (e - URR_HI).abs() <= 1e-9 * URR_HI && v != 0.0)
        .collect();
    println!("  (at the {URR_HI:.1e} boundary itself: {at_top:?})");
    assert!(
        inside.is_empty(),
        "MF=3 MT=18 carries background strictly inside the unresolved window \
         ({inside:?}), so NJOY's PENDF value there is no longer the pure \
         unresolved cross section and the kernel comparison above is not \
         like-for-like. The known point at exactly 1.0e5 eV is excluded by \
         construction -- see this test's doc comment."
    );
}
