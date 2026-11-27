// Reads reference files or the deck work directory: needs `std` (gh:#1592).
#![cfg(feature = "std")]

// SPDX-License-Identifier: AGPL-3.0-only
//! **RECONR at `tempr > 0` (SLBW, MLBW, Adler-Adler), the SLBW/MLBW competitive
//! width (`LRX != 0`), and `csnorp` on the per-range path, against NJOY2016.80**
//! (GitHub #1323, #1731).
//!
//! # Methodology
//!
//! NJOY2016.80 (`build/njoy`) and the port's deck driver ([`run_deck`]) run
//! `reconr 20 22` (tolerance 0.001, `tempr` on card 4) on the same tape text.
//! **Pass criterion:** tape 22 identical line for line, MF=1 to MF=3.
//!
//! Tapes. Only Na-23 (ENDF/B-VIII.0, MAT 1125: one MLBW range, 600 eV to
//! 500 keV, l = 0, 1, 2) and U-235 (ENDF/B-V tape 511, MAT 1395: SLBW 1 to
//! 82 eV, then an unresolved range) are used as held. No held evaluation has
//! `LRX != 0`, an Adler-Adler range, or an `LRU = 0` range in an `LRP = 1`
//! material (scanned 2026-11-24: `reference-data/endf/` and NJOY2016's
//! `tests/resources/`), so the other cases are Na-23 with its MF=2/MT=151
//! edited here. They are code-to-code devices with no physical meaning:
//!
//! - `slbw`: Na-23's range with `LRF = 1`.
//! - `lrx`: every l-state `LRX = 1`, `QX = -440 keV` (MT=51's QI), and
//!   `GT = GN + GG + GF + 0.1 GN` for the resonances above the threshold
//!   `-QX/arat`, so `gc != 0` there (538.6 keV and up).
//! - `lrx_not_last`: as `lrx`, but the last l-state (l = 2) keeps `LRX = 0`:
//!   the widths still enter `gtt`, but `emerge`'s `lrx` is 0 and MT=51 gets
//!   nothing (`reconr.f90:4769`).
//! - `csnorp_below`: an `LRU = 0` range `[1e-5, 600]` eV before the MLBW range.
//! - `csnorp_only`: one `LRU = 0` range `[1e-5, 1e5]` eV.
//! - `csnorp_below_300`, `mlbw_split_0`, `mlbw_split_300` (GitHub #340, added
//!   2026-11-24): an MLBW range on a seam. `csnorp_below` at 300 K, and Na-23's
//!   MLBW range cut at 3 keV into two MLBW ranges with the same parameters
//!   (`[600, 3000)`, `[3000, 5e5)`), the MF=1 dictionary's MF=2 count raised to
//!   match (NJOY2016 sizes `res` from it). At `tempr > 0` such a material takes
//!   `sigma_walk`'s one walk.
//! - `aa`: one Adler-Adler range `[1e-5, 5000]` eV (`LI = 7`): l = 0 with two J
//!   groups (two resonances, then one), l = 1 with one.
//!
//! # Prediction (posted on gh:#1323 and gh:#1731 before the first run)
//!
//! Every case identical to NJOY2016 line for line.
//!
//! # Results
//!
//! ~~12~~ **15** cases (2026-11-24, GitHub #340) identical line for line; the two Adler-Adler cases identical except
//! for NJOY2016's own undefined `sigp(5)`. Table and details:
//! [`reconr_hot_and_competitive_match_njoy2016`].

use std::path::{Path, PathBuf};

use njoy_outram_park_fork::deck::{run_deck, DeckOptions};
use njoy_outram_park_fork::endf::tape::{Section, Tape};

mod common;

fn njoy_root() -> PathBuf {
    std::env::var("NJOY2016_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("upstream_source/NJOY2016"))
}

const NA: i32 = 1125;

/// Na-23 with [`mlbw_split`]'s MF=2 at [`SPLIT`], its dictionary count updated.
fn split_text(na: &Tape, base: &[[f64; 6]]) -> String {
    let rows = mlbw_split(base, SPLIT);
    let nc = rows.len();
    with_mf2_count(with_mf2(na, NA, rows).to_text().unwrap(), nc)
}

/// Where [`mlbw_split`] cuts Na-23's MLBW range: on the high flank of its
/// 2.81 keV s-wave resonance.
const SPLIT: f64 = 3000.0;

fn na23() -> Option<Tape> {
    let p = njoy_outram_park_fork::reference_data::reference_endf("n-011_Na_023-ENDF8.0.endf")?;
    Some(Tape::read_file(&p).expect("Na-23 tape"))
}

fn with_mf2(tape: &Tape, mat: i32, rows: Vec<[f64; 6]>) -> Tape {
    let mut secs: Vec<Section> = tape.sections().to_vec();
    let sec = secs.iter_mut().find(|s| s.key.mat == mat && s.key.mf == 2 && s.key.mt == 151).expect("MF=2/MT=151");
    sec.rows = rows;
    Tape::from_sections(String::from("synthetic RECONR tempr / LRX / csnorp tape"), secs)
}

fn mf2_rows(tape: &Tape) -> Vec<[f64; 6]> {
    tape.section(NA, 2, 151).expect("MF=2").rows.clone()
}

/// Indices of the l-state LIST heads of Na-23's MLBW range (rows 4, 9, 22).
fn lstate_heads(rows: &[[f64; 6]]) -> Vec<usize> {
    let mut heads = Vec::new();
    let mut k = 4;
    while k < rows.len() {
        heads.push(k);
        k += 1 + rows[k][5] as usize;
    }
    heads
}

fn slbw(base: &[[f64; 6]]) -> Vec<[f64; 6]> {
    let mut rows = base.to_vec();
    assert_eq!(rows[2][3], 2.0, "LRF of the MLBW range");
    rows[2][3] = 1.0;
    rows
}

fn lrx(base: &[[f64; 6]], last_too: bool) -> Vec<[f64; 6]> {
    const QX: f64 = -440000.0;
    let mut rows = base.to_vec();
    let heads = lstate_heads(&rows);
    for (n, &h) in heads.iter().enumerate() {
        if n + 1 == heads.len() && !last_too {
            continue;
        }
        rows[h][1] = QX;
        rows[h][3] = 1.0;
        let arat = rows[h][0] / (rows[h][0] + 1.0);
        let nrs = rows[h][5] as usize;
        for r in rows.iter_mut().skip(h + 1).take(nrs) {
            if r[0] >= -QX / arat {
                r[2] = r[3] + r[4] + r[5] + 0.1 * r[3];
            }
        }
    }
    rows
}

fn csnorp_below(base: &[[f64; 6]]) -> Vec<[f64; 6]> {
    let mut rows = vec![base[0], base[1]];
    rows[1][4] = 2.0; // NER
    rows.push([1.0e-5, 600.0, 0.0, 0.0, 0.0, 0.0]);
    rows.push([1.5, 0.541, 0.0, 0.0, 0.0, 0.0]);
    rows.extend_from_slice(&base[2..]);
    rows
}

/// Na-23's MLBW range split at `x` into two back-to-back MLBW ranges with the same
/// parameters (`[600, x)` and `[x, 5e5)`): an MLBW range on a seam (GitHub #340).
/// Each range keeps its own `res` table, so the second range's g/h stay at
/// `rdf2bw`'s zero until `csmlbw2`'s `enow` next moves past it.
fn mlbw_split(base: &[[f64; 6]], x: f64) -> Vec<[f64; 6]> {
    let mut rows = vec![base[0], base[1]];
    rows[1][4] = 2.0; // NER
    let (mut lo, mut hi) = (base[2], base[2]);
    assert!(x > lo[0] && x < lo[1], "split inside the range");
    lo[1] = x;
    hi[0] = x;
    rows.push(lo);
    rows.extend_from_slice(&base[3..]);
    rows.push(hi);
    rows.extend_from_slice(&base[3..]);
    rows
}

/// `text` with the MF=1/MT=451 dictionary's `NC` for MF=2/MT=151 set to `nc`.
/// NJOY2016 sizes its resonance store from that count (`maxres = 12*NC`,
/// `reconr.f90:551`), so a case that adds MF=2 records must say so there, or
/// `rdf2bw` stops with "res storage exceeded".
fn with_mf2_count(text: String, nc: usize) -> String {
    text.lines()
        .map(|l| {
            let dict = l.len() >= 75 && &l[70..75] == " 1451" && l[22..33].trim() == "2" && l[33..44].trim() == "151";
            if dict {
                format!("{}{:>11}{}", &l[..44], nc, &l[55..])
            } else {
                l.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn csnorp_only(base: &[[f64; 6]]) -> Vec<[f64; 6]> {
    vec![base[0], base[1], [1.0e-5, 1.0e5, 0.0, 0.0, 0.0, 0.0], [1.5, 0.541, 0.0, 0.0, 0.0, 0.0]]
}

/// One Adler-Adler resonance's 12 words: total, fission, capture `(DE, DW, GR, GI)`.
fn aa_res(de: f64, dw: f64, scale: f64) -> [f64; 12] {
    [de, dw, 2.4e-3 * scale, 1.0e-4 * scale, de, dw, 1.0e-4 * scale, -1.0e-5 * scale, de, dw, 2.0e-4 * scale, 2.0e-5 * scale]
}

fn aa(base: &[[f64; 6]]) -> Vec<[f64; 6]> {
    let awri = 22.792;
    let mut rows = vec![base[0], base[1]];
    rows.push([1.0e-5, 5000.0, 1.0, 4.0, 0.0, 0.0]);
    rows.push([1.5, 0.541, 0.0, 0.0, 2.0, 0.0]);
    // Background (LI = 7: total, fission, capture).
    rows.push([awri, 0.0, 7.0, 0.0, 18.0, 3.0]);
    rows.push([1.0e-4, 0.0, 0.0, 0.0, 0.0, 0.0]);
    rows.push([1.0e-5, 0.0, 0.0, 0.0, 0.0, 0.0]);
    rows.push([2.0e-5, 0.0, 0.0, 0.0, 0.0, 0.0]);
    let list = |rows: &mut Vec<[f64; 6]>, aj: f64, res: &[[f64; 12]]| {
        rows.push([aj, 0.0, 0.0, 0.0, 12.0 * res.len() as f64, res.len() as f64]);
        for r in res {
            rows.push([r[0], r[1], r[2], r[3], r[4], r[5]]);
            rows.push([r[6], r[7], r[8], r[9], r[10], r[11]]);
        }
    };
    rows.push([0.0, 0.0, 0.0, 0.0, 2.0, 0.0]); // l = 0, NJS = 2
    list(&mut rows, 1.0, &[aa_res(35.0, 0.4, 0.3), aa_res(1200.0, 3.0, 2.0)]);
    list(&mut rows, 2.0, &[aa_res(300.0, 1.0, 1.0)]);
    rows.push([0.0, 0.0, 1.0, 0.0, 1.0, 0.0]); // l = 1, NJS = 1
    list(&mut rows, 1.0, &[aa_res(2800.0, 5.0, 3.0)]);
    rows
}

fn deck(mat: i32, tempr: f64) -> String {
    format!("reconr\n20 22/\n'synthetic'/\n{mat} 0/\n0.001 {tempr:.1}/\n0/\nstop\n")
}

fn lines(p: &Path) -> Vec<String> {
    std::fs::read_to_string(p).unwrap_or_default().lines().map(str::to_string).collect()
}

/// `reconr 20 22` at `tempr` on `text` with NJOY2016 and the port; `(ours, NJOY's)` tape 22.
fn run_both(root: &Path, tmp: &Path, name: &str, text: &str, mat: i32, tempr: f64) -> (Vec<String>, Vec<String>) {
    let (ours, theirs) = (tmp.join(format!("{name}_ours")), tmp.join(format!("{name}_njoy")));
    let deck = deck(mat, tempr);
    for d in [&ours, &theirs] {
        std::fs::create_dir_all(d).unwrap();
        std::fs::write(d.join("tape20"), text).unwrap();
    }
    std::fs::write(theirs.join("deck"), &deck).unwrap();
    let (ok, date) = common::run_njoy_dated(&root.join("build/njoy"), &theirs);
    assert!(ok, "NJOY2016 failed on {name}");
    let run = run_deck(&deck, &DeckOptions { workdir: Some(ours.clone()), parse_only: false, run_date: Some(date) }).unwrap();
    assert_eq!(run.blocked, None, "{name} must run to its end");
    (lines(&ours.join("tape22")), lines(&theirs.join("tape22")))
}

/// MF=3 `(MT, E, sigma)` data pairs of a PENDF, as printed (11-column fields),
/// without the section heads.
fn mf3_pairs(v: &[String]) -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    let mut line_in_section = 0;
    let mut mt_now = String::new();
    for l in v.iter().filter(|l| l.len() >= 75 && &l[70..72] == " 3") {
        let mt = l[72..75].to_string();
        if mt != mt_now {
            mt_now = mt.clone();
            line_in_section = 0;
        }
        line_in_section += 1;
        if line_in_section <= 3 || mt.trim() == "0" {
            continue;
        }
        for k in 0..3 {
            let (e, y) = (&l[22 * k..22 * k + 11], &l[22 * k + 11..22 * k + 22]);
            if !e.trim().is_empty() {
                out.push((mt.clone(), e.to_string(), y.to_string()));
            }
        }
    }
    out
}

/// A printed ENDF real (`1.000000-4`, `600.000004`) as `f64`.
fn endf_real(s: &str) -> f64 {
    let t = s.trim();
    if let Some(k) = t.rfind(['+', '-']).filter(|&k| k > 0) {
        let (m, e) = t.split_at(k);
        format!("{m}e{e}").parse().unwrap()
    } else {
        t.parse().unwrap()
    }
}

/// Where NJOY2016's stale `sigp(5)` from `csaa` changes the grid on the
/// Adler-Adler cases: from 1e-4 eV, where `resxs` prints its first
/// resonance-integral line and the stale value appears, through the panels its
/// step-increase rule then re-splits. See the test's doc.
const AA_PERTURBED: (f64, f64) = (1.0e-4, 2.0e-4);

fn count(v: &[String], mf: &str) -> usize {
    v.iter().filter(|l| l.len() >= 75 && &l[70..72] == mf).count()
}

/// **Every case against NJOY2016.80, tape 22 line for line.**
///
/// # Results (2026-11-24, NJOY2016.80 `build2/njoy`, gfortran, release)
///
/// | case | `tempr` | tape 22 lines | differing |
/// |---|---|---|---|
/// | `mlbw_300` (Na-23 as held) | 300 K | 10 342 | **0** |
/// | `mlbw_5` (`csmlbw2` below `tmin`) | 5 K | 10 470 | **0** |
/// | `mlbw_2400` | 2400 K | 10 198 | **0** |
/// | `slbw_300` | 300 K | 10 346 | **0** |
/// | `u235_slbw_300` (tape 511, MAT 1395, held) | 300 K | 26 414 | **0** |
/// | `lrx_mlbw_0` / `lrx_mlbw_300` | 0 / 300 K | 10 598 / 10 413 | **0** / **0** |
/// | `lrx_slbw_0` / `lrx_slbw_300` | 0 / 300 K | 10 616 / 10 480 | **0** / **0** |
/// | `lrx_not_last_0` | 0 K | 10 636 | **0** |
/// | `csnorp_below` | 0 K | 10 726 | **0** |
/// | `csnorp_only` | 0 K | 7 396 | **0** |
/// | `csnorp_below_300` (GitHub #340) | 300 K | 10 610 | **0** |
/// | `mlbw_split_0` / `mlbw_split_300` (GitHub #340) | 0 / 300 K | 10 474 / 10 370 | **0** / **0** |
/// | `aa_0` / `aa_300` | 0 / 300 K | 7 572 ours, 7 600 NJOY's | see below |
///
/// `mlbw_*` pass only because the walk is sequential: `csmlbw2`'s g/h are kept
/// from earlier calls, so the order of `sigma` calls is part of the answer.
///
/// **The seam cases (GitHub #340, 2026-11-24).** Until this change [`run_deck`]
/// refused `tempr > 0` with an MLBW range adjoining another resolved range: the
/// per-range path rebuilds each range, then tests the panel across the seam
/// afterwards (`reconr::seam`), which calls `csmlbw2` out of `resxs`'s order. Such
/// a material now takes `sigma_walk`'s one walk. Prediction (posted on gh:#340
/// before the run): identical line for line. Result: 0 differing lines on all
/// three. Control, run once with the one walk switched off for these cases (the
/// per-range path, refusal removed): `mlbw_split_300` differed in 10 096 of
/// 10 370 lines (NJOY has 4 478 MF=3 points per section, the per-range path
/// 4 475; range 2's g/h at the seam were its values from 5e5 eV, not `rdf2bw`'s
/// zeros) and `csnorp_below_300` in 18 lines (MT=1 at 600.000004 eV: 3.023151
/// against NJOY's 3.299144). So the refusal was warranted, and the one walk is
/// what removes the need for it.
///
/// What the first run found (fixed, then re-run): `csnorp_below` differed in
/// 10 436 lines. `resxs` tests the panel across 600 eV, where the `LRU = 0` range
/// ends and the MLBW range starts, and NJOY bisected it 30 times; the port's seam
/// pass took SLBW/MLBW/RM ranges only. It now takes an `LRU = 0` range of an
/// `LRP = 1` material too.
///
/// **The Adler-Adler cases are not a line-for-line pass, and the cause is in
/// NJOY2016.** `csaa` declares `sigp(4)` and zeroes four words (`reconr.f90:4327,
/// 4340`), but `sigma` passes `sigp(nsig)` with `nsig = 5` and adds and tests
/// `sigp(5)` (`:2641-2645`, `resxs` `:2384-2400`). So `sigp(5)` is whatever the
/// stack held. Instrumented (`if (sigp(5).ne.0) write(77,*)` after the `csaa`
/// call), it is 0 until 1e-4 eV, where `resxs` prints its first
/// resonance-integral line, and 3 490 000 from then on. The panel where it
/// changes fails the convergence test and is bisected down to the
/// significant-figure limit; then the step-increase rule re-splits the next
/// panels. That is 20 extra points in each of MT=1, 2 and 102, all in
/// `[1e-4, 1.012782e-4]` eV. With `sigp(5)=0` added after the `csaa` call in a
/// scratch NJOY2016 build, both cases are identical to the port's tape **line
/// for line** (7 572 lines each). The port keeps `sigp(5) = 0`, the defined
/// reading. What this test asserts against the stock binary: MF=2 identical
/// line for line, every MF=3 pair of ours present in NJOY's tape in order and
/// as printed (17 724 of 17 784), and NJOY's 60 extra pairs all inside
/// `AA_PERTURBED`.
#[test]
fn reconr_hot_and_competitive_match_njoy2016() {
    let root = njoy_root();
    if !root.join("build/njoy").exists() {
        eprintln!("skip: {} not present (set NJOY2016_ROOT)", root.join("build/njoy").display());
        return;
    }
    let Some(na) = na23() else { return };
    let base = mf2_rows(&na);
    let mut cases: Vec<(&str, String, i32, f64)> = vec![
        ("mlbw_300", na.to_text().unwrap(), NA, 300.0),
        ("mlbw_5", na.to_text().unwrap(), NA, 5.0),
        ("mlbw_2400", na.to_text().unwrap(), NA, 2400.0),
        ("slbw_300", with_mf2(&na, NA, slbw(&base)).to_text().unwrap(), NA, 300.0),
        ("lrx_mlbw_0", with_mf2(&na, NA, lrx(&base, true)).to_text().unwrap(), NA, 0.0),
        ("lrx_mlbw_300", with_mf2(&na, NA, lrx(&base, true)).to_text().unwrap(), NA, 300.0),
        ("lrx_slbw_0", with_mf2(&na, NA, slbw(&lrx(&base, true))).to_text().unwrap(), NA, 0.0),
        ("lrx_slbw_300", with_mf2(&na, NA, slbw(&lrx(&base, true))).to_text().unwrap(), NA, 300.0),
        ("lrx_not_last_0", with_mf2(&na, NA, lrx(&base, false)).to_text().unwrap(), NA, 0.0),
        ("csnorp_below", with_mf2(&na, NA, csnorp_below(&base)).to_text().unwrap(), NA, 0.0),
        ("csnorp_only", with_mf2(&na, NA, csnorp_only(&base)).to_text().unwrap(), NA, 0.0),
        // An MLBW range on a seam at `tempr > 0` (GitHub #340): the one walk.
        ("csnorp_below_300", with_mf2(&na, NA, csnorp_below(&base)).to_text().unwrap(), NA, 300.0),
        ("mlbw_split_0", split_text(&na, &base), NA, 0.0),
        ("mlbw_split_300", split_text(&na, &base), NA, 300.0),
        ("aa_0", with_mf2(&na, NA, aa(&base)).to_text().unwrap(), NA, 0.0),
        ("aa_300", with_mf2(&na, NA, aa(&base)).to_text().unwrap(), NA, 300.0),
    ];
    // ENDF/B-V tape 511 (NJOY2016's `tests/resources`), U-235's real SLBW range.
    if let Ok(text) = std::fs::read_to_string(root.join("tests/resources/t511")) {
        cases.push(("u235_slbw_300", text, 1395, 300.0));
    }
    let only = std::env::var("RECONR_HOT_CASE").ok();
    let tmp = std::env::temp_dir().join(format!("njoy_reconr_hot_{}", std::process::id()));
    let mut bad = Vec::new();
    for (name, text, mat, tempr) in &cases {
        if only.as_deref().is_some_and(|o| o != *name) {
            continue;
        }
        let (a, b) = run_both(&root, &tmp, name, text, *mat, *tempr);
        if name.starts_with("aa_") {
            // NJOY2016's undefined `sigp(5)` (see the doc). Checked instead:
            // MF=2 line for line; every MF=3 pair of ours is in NJOY's tape,
            // in order and as printed; NJOY's extra pairs all lie in the
            // perturbed window.
            let (lo, hi) = AA_PERTURBED;
            let (pa, pb) = (mf3_pairs(&a), mf3_pairs(&b));
            let mut extra = Vec::new();
            let mut k = 0;
            for q in &pb {
                if k < pa.len() && pa[k] == *q {
                    k += 1;
                } else {
                    extra.push(q.clone());
                }
            }
            let subsequence = k == pa.len();
            let window = extra.iter().all(|q| (lo..hi).contains(&endf_real(&q.1)));
            let span = extra.iter().map(|q| endf_real(&q.1)).fold((f64::MAX, 0.0f64), |(a, b), e| (a.min(e), b.max(e)));
            let mf2 = |v: &[String]| v.iter().filter(|l| l.len() >= 75 && &l[70..72] == " 2").cloned().collect::<Vec<_>>();
            println!(
                "{name}: {} MF=3 pairs ours, {} NJOY's; ours a subsequence of NJOY's: {subsequence}; {} NJOY extras in [{:e}, {:e}]",
                pa.len(),
                pb.len(),
                extra.len(),
                span.0,
                span.1
            );
            if !subsequence || !window || mf2(&a) != mf2(&b) {
                bad.push((*name, extra.len()));
            }
            continue;
        }
        let d: Vec<usize> = (0..a.len().max(b.len())).filter(|&i| a.get(i) != b.get(i)).collect();
        println!("{name}: {} differing lines of {} (NJOY MF=3 lines {}, ours {} lines)", d.len(), b.len(), count(&b, " 3"), a.len());
        for &i in d.iter().take(6) {
            println!("  line {}\n  ours: {:?}\n  njoy: {:?}", i + 1, a.get(i), b.get(i));
        }
        assert!(count(&b, " 3") > 100, "{name}: NJOY's tape 22 must carry MF=3");
        if !d.is_empty() {
            bad.push((*name, d.len()));
        }
    }
    if std::env::var_os("KEEP_DECK_DIRS").is_none() {
        let _ = std::fs::remove_dir_all(&tmp);
    }
    assert!(bad.is_empty(), "tape 22 differs from NJOY2016: {bad:?}");
}
