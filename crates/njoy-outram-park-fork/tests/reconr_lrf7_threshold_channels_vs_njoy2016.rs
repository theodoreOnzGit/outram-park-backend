//! `LRF=7` evaluations with an inelastic or reaction channel beyond (γ, n),
//! through RECONR against NJOY2016's own PENDFs, with a memory and time bound
//! (GitHub #339).
//!
//! # Why this test exists
//!
//! Until 2026-10-01, reconstructing ENDF/B-VIII.0 **Fe-57** exhausted memory:
//! 13.4 GB anon RSS (OOM-killed) in `htr10_rmc_keff`, `memory allocation of
//! 1476395008 bytes failed` under a 14 GB `ulimit -v` in `lct008_keff`. Fe-57's
//! MF=2 is `LRU=1, LRF=7, KRM=3`, EH = 190 keV, with particle pairs
//! (γ, n, n'₁): the third is MT=51, inelastic to the 14.413 keV level, with
//! threshold 14 668.34 eV lab. Of its six spin groups, J=1⁻ and J=2⁺ have
//! **four** explicit channels. **Mo-95** (ENDF/B-VIII.0 beta, four particle
//! pairs, ten spin groups) failed the same way, which nobody had noticed.
//!
//! Two port defects combined, both found by reading `samm.f90` and
//! `reconr.f90` against the port:
//!
//! 1. **`xspsl`'s `xdot` read the wrong vector** (`samm.f90:6189, 6201-6204`:
//!    `xdot(xdoti,k-1,ap(1,ik+1),1,b(1,1),1)`). The port's `xdot` took its
//!    second operand from the packed factor instead of from `b`. So `yfour`
//!    (`n >= 4` channels) returned a wrong `Y^-1` whenever the level matrix
//!    had a non-zero off-diagonal. On Fe-57's J=1⁻ group, `|Y·Y^-1 - I|` was
//!    3.3 at 14.67 keV and 40 at 110 keV, and `Σ|U|²` reached 9 120 against
//!    the unitarity bound of 1. Total and elastic were still right to 7
//!    figures. The MT=51 channel was 10x to 1e4x too large, so capture
//!    (`nonelastic - MT51`, `samm.f90:108-118`) went as low as -4.2e4 b.
//! 2. **`sigma`'s non-negativity guard was missing** (`reconr.f90:2641-2645`,
//!    `if (sigp(j).lt.zero) sigp(j)=0`). `resxs`'s midpoint test
//!    `dm(j) > errn*sig(j+1)` fails at every panel when `sig` is negative.
//!    So the negative capture made RECONR bisect every panel above the MT=51
//!    threshold down to the significant-figure floor. That is what consumed
//!    the memory, not a slow kernel.
//!
//! A third, smaller one appeared once those two were fixed: `emerge` writes
//! each section from the point before its first positive value (`ith`,
//! `reconr.f90:4808, 4919`), and the port did not. Fe-57's MT=51 and MT=4
//! carried 3 206 extra sub-threshold zeros from 1e-5 eV.
//!
//! The prediction stated before the first run was that fixing `xdot` would
//! bring `|Y·Y^-1 - I|` to round-off, put MT=51 and capture on NJOY's values,
//! and cut the reconstruction to seconds on NJOY's 17 213-point grid. All
//! three held.
//!
//! # Methodology
//!
//! - **Inputs:** `reference-data/endf/n-026_Fe_057-ENDF8.0.endf` (MAT 2634)
//!   and `n-042_Mo_095-ENDF8.0-beta.endf` (MAT 4234).
//! - **References:** `reference-data/reconr/fe57-ENDF8.0-0K-err0.001.pendf`
//!   and `mo95-ENDF8.0beta-0K-err0.001.pendf`, NJOY2016 `ac5adf5` RECONR at
//!   0 K, `err = 0.001`. Each deck is committed beside its PENDF.
//! - **Ours:** `reconr::reconr` at `tolerance = 0.001`, 0 K, then
//!   `through_pendf_text` (11-column ENDF floats), exactly as
//!   `tests/pendf_stages_vs_njoy2016.rs` does for the other nuclides.
//! - **Bounds:** wall time of the `reconr` call, and the process's peak
//!   resident set (`VmHWM` in `/proc/self/status`; Linux only, skipped
//!   elsewhere). The RSS is process-wide, so it covers both tests when they
//!   run in parallel. Gates: **60 s** and **2 GB**. Each is more than 10x the
//!   measured value below, so a slower machine passes. A regression to the
//!   old behaviour (gigabytes, minutes, then an abort) cannot.
//! - **Agreement, asserted:** every MF=3 section NJOY writes is present in
//!   ours, with the same QI and number of points, and ours has no section
//!   NJOY lacks. Then every `(E, σ)` word is compared exactly. The count of
//!   differing words must equal the value measured below (zero).
//!
//! # Results, 2026-10-01
//!
//! Before the fix, under `ulimit -v 8000000`: Fe-57 aborted after about
//! 2.5 min (`memory allocation of 184549376 bytes failed`); Mo-95 aborted
//! (`memory allocation of 738197504 bytes failed`).
//!
//! After. Release build, Ryzen 5 5600, `-j 3`, one test thread:
//!
//! | case | `reconr` time | peak RSS | MF=3 sections | MT=1 points, ours / NJOY | differing words |
//! |---|---|---|---|---|---|
//! | Fe-57 ENDF/B-VIII.0 | 0.10 s | 24 MB | 82 | 17 213 / 17 213 | **0** |
//! | Mo-95 ENDF/B-VIII.0 beta | 0.40 s | 30 MB (process, after both cases) | 84 | 34 939 / 34 939 | **0** |
//!
//! NJOY2016 itself on the Fe-57 deck, same machine: 1.1 s.
//!
//! Ablation, Fe-57. With the `xdot` fix alone, without the clamp: identical,
//! 0 differing words, because no partial goes negative. With the clamp alone,
//! without the `xdot` fix: bounded (0.15 s, 26 MB) but wrong. That gives
//! 24 545 points against NJOY's 17 213 and 285 212 differing words, since
//! MT=51 is still too large and capture is clamped to zero.
//!
//! Also compared word for word on the same day, against PENDFs generated by
//! the same NJOY build with the same deck but not committed (Fe-54's is
//! 16.6 MB). All are **identical**, and were identical before this change
//! too:
//! - Fe-54 ENDF/B-VIII.0 (MAT 2625): 79 sections, 48 565 points;
//! - Cu-63 ENDF/B-VIII.0 (MAT 2925): 44 sections, 50 199 points;
//! - Cu-65 ENDF/B-VIII.0 (MAT 2931): 53 sections, 40 224 points.
//!
//! None of them reaches `yfour` with a coupled matrix.
//!
//! **Interpretation.** The OOM was a wrong matrix inverse, made unbounded
//! by a missing guard. It was not a performance problem; the per-point
//! allocation in the issue's profile was a symptom. With both fixed, the
//! port's RECONR is word-for-word NJOY2016's on every `LRF=7` evaluation
//! held here. That includes the two with a threshold channel and four-channel
//! spin groups, the only ones that exercise `yfour` with off-diagonal
//! coupling.

use std::time::Instant;

use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};

/// Wall-time gate on one `reconr` call (measured values in the module doc).
const MAX_SECONDS: f64 = 60.0;
/// Peak-RSS gate on the whole test process, kB (2 GB).
const MAX_RSS_KB: u64 = 2_000_000;

/// Peak resident set of this process in kB, from `/proc/self/status`.
fn peak_rss_kb() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = s.lines().find(|l| l.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

fn tab1(rows: &[[f64; 6]]) -> (f64, Vec<(f64, f64)>) {
    let mut c = SectionCursor::new(rows);
    let _ = c.read_cont().expect("HEAD");
    let t = c.read_tab1().expect("TAB1");
    (t.head.c2, t.pairs)
}

/// Reconstruct `endf` (MAT `mat`) and compare every MF=3 word with NJOY's
/// `golden`; asserts the time and memory gates and `expected_diff` words.
fn check(label: &str, endf: &str, mat: i32, golden: &str, expected_diff: usize) {
    let Some(endf_path) = reference_endf_or_skip(endf, label) else {
        return;
    };
    let Some(golden_path) = reference_file_or_skip("reconr", golden, label) else {
        return;
    };
    let tape = Tape::read_file(&endf_path).expect("ENDF tape");
    let theirs = Tape::read_file(&golden_path).expect("NJOY PENDF");

    let t0 = Instant::now();
    let r = reconr(
        &tape,
        &ReconrConfig {
            mat,
            tolerance: 1.0e-3,
            temperature: 0.0,
        },
    )
    .expect("RECONR");
    let secs = t0.elapsed().as_secs_f64();
    let rss = peak_rss_kb();
    let ours = r.through_pendf_text();

    let mut n_sec = 0usize;
    let mut n_diff = 0usize;
    let mut structural = Vec::new();
    let mut firsts = Vec::new();
    let mut their_mts = Vec::new();
    let mut mt1_points = (0usize, 0usize);
    for sec in theirs.sections().iter().filter(|s| s.key.mat == mat && s.key.mf == 3) {
        let mt = sec.key.mt;
        their_mts.push(mt);
        let (q_t, p_t) = tab1(&sec.rows);
        let Some(o) = ours.sections.iter().find(|s| i32::from(s.mt) == mt) else {
            structural.push(format!("MT={mt}: missing from ours"));
            continue;
        };
        n_sec += 1;
        if mt == 1 {
            mt1_points = (o.pairs.len(), p_t.len());
        }
        if o.qi != q_t {
            structural.push(format!("MT={mt}: QI {} vs NJOY {}", o.qi, q_t));
        }
        if o.pairs.len() != p_t.len() {
            structural.push(format!("MT={mt}: {} points vs NJOY {}", o.pairs.len(), p_t.len()));
        }
        let mut first = None;
        for (k, (a, b)) in o.pairs.iter().zip(&p_t).enumerate() {
            for (x, y) in [(a.0, b.0), (a.1, b.1)] {
                if x != y {
                    n_diff += 1;
                    first.get_or_insert((k, *a, *b));
                }
            }
        }
        if let Some((k, a, b)) = first {
            firsts.push(format!("MT={mt}: first differing pair #{k}: ours {a:?} NJOY {b:?}"));
        }
    }
    for s in &ours.sections {
        let mt = i32::from(s.mt);
        if !their_mts.contains(&mt) {
            structural.push(format!("MT={mt}: in ours, not in NJOY's PENDF"));
        }
    }

    println!("{label}: reconr {secs:.2} s, peak RSS {rss:?} kB");
    println!(
        "{label}: {n_sec} MF=3 sections, MT=1 points ours {} / NJOY {}, {n_diff} differing words",
        mt1_points.0, mt1_points.1
    );
    for line in structural.iter().chain(&firsts) {
        println!("  {line}");
    }

    assert!(secs < MAX_SECONDS, "{label}: RECONR took {secs:.1} s (gate {MAX_SECONDS} s)");
    if let Some(kb) = rss {
        assert!(kb < MAX_RSS_KB, "{label}: peak RSS {kb} kB (gate {MAX_RSS_KB} kB)");
    }
    assert!(structural.is_empty(), "{label}: structural differences from NJOY2016: {structural:#?}");
    assert_eq!(
        n_diff, expected_diff,
        "{label}: MF=3 words differing from NJOY2016 moved: {firsts:#?}"
    );
}

#[test]
fn fe57_reconr_is_bounded_and_matches_njoy2016() {
    check(
        "reconr-fe57-lrf7",
        "n-026_Fe_057-ENDF8.0.endf",
        2634,
        "fe57-ENDF8.0-0K-err0.001.pendf",
        0,
    );
}

#[test]
fn mo95_reconr_is_bounded_and_matches_njoy2016() {
    check(
        "reconr-mo95-lrf7",
        "n-042_Mo_095-ENDF8.0-beta.endf",
        4234,
        "mo95-ENDF8.0beta-0K-err0.001.pendf",
        0,
    );
}
