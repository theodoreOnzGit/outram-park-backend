//! ERRORR MF=32 (resonance-parameter covariance, `resprx` chain) golden
//! validation against NJOY2016 on TENDL-2023 Ar-37.
//!
//! # Why Ar-37
//!
//! It is the only committed evaluation with MF=32 (`reference-data/endf/
//! README.md`). Its MF=32 has a resolved MLBW range (`LRU=1/LRF=2`,
//! `LCOMP=2` compact format, 7 resonances × `MPAR=3` parameters, `NM=0`,
//! `ISR=1` with `DAP=2.032674e-2`) and an unresolved range (`LRU=2`,
//! `MPAR=2`, 11 `(L,J)` states, 22 parameters with a relative covariance
//! LIST); the MF=2 URR is `LRF=2`, which is where NJOY takes the degrees
//! of freedom from. So one material exercises `rpxlc2`, `rpxlc12` (with the
//! scattering-radius pass), `rpendf`/`ggmlbw`, `rpxgrp`, `rpxunr`/`ggunr1`
//! and `rescon` for the `(1,1)`, `(2,2)`, `(2,102)`, `(102,102)` pairs.
//!
//! # The oracle and the upstream defect it exposed
//!
//! NJOY2016 `ac5adf5` **aborts** on the unmodified tape
//! (`***error in rpxlc12***problem` at resonance 2, `E = -2100.016 eV`):
//! its resonance search (`errorr.f90:4363-4380`) advances the MF=2
//! pointer only inside `if (ipara.ne.0)`, and Ar-37's `L=1` block has zero
//! resonances, so the `L=2` block (which holds that resonance) is never
//! scanned. The oracle therefore runs on
//! `n-018_Ar_37-tendl2023-mf2-L1-last.endf`, the same evaluation with the
//! empty `L=1` block moved after the `L=2` block in MF=2 (sequence numbers
//! renumbered, nothing else touched). That is physics-identical — MLBW
//! sums over L — and NJOY's RECONR+BROADR PENDF from the two tapes is
//! **byte-identical** (checked 2026-09-10). The crate's port advances the
//! pointer for an empty block too, so [`original_tape_reproduces_the_variant_oracle`]
//! runs the unmodified tape through the port and compares it with the
//! variant-tape oracle.
//!
//! Deck (`reference-data/errorr/ar37-tendl2023-L1last-293.6K-ign3-iwt6-rel.njoy-input`):
//!
//! ```text
//! reconr (err 0.001) -> broadr (293.6 K, 0.001) -> errorr
//!   20 22 0 23 0 0 /   1828 3 6 1 1 /   1 293.6 /   0 33 1 1 -1 2e6 0 /
//! ```
//!
//! # Tiers
//!
//! **Tier 1** (NJOY's own PENDF in): the resonance sensitivities are
//! computed from MF=2/MF=32 alone, so the whole MF=32 chain plus the MF=33
//! path is what differs. Prediction: the tape's printing precision
//! (7 figures, 6 for two-digit exponents) — every group cross section and
//! every covariance element within 6e-6.
//!
//! **Listing diagonals**: NJOY prints, per output block that has a
//! resonance contribution, the `c**`/`u**` diagonal divided by `cflx²`
//! ("resolved" / "unresolve", 4 significant figures,
//! `errorr.f90:7660-7700`). Those tables (three blocks: total, elastic,
//! capture) are pinned here to 4 figures — the only direct oracle on the
//! split between the resolved and the unresolved contributions.
//!
//! **Tier 2** (crate RECONR + BROADR in): reported; the MF=32 sensitivity
//! path does not see the PENDF, so only the `σ_g` and `cflx` normalisation
//! moves.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::errorr::{run_mf33, ErrorrResult, ErrorrWeight, Mf33Config};
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};

const MAT: i32 = 1828;
const TEMP_K: f64 = 293.6;
const IGN: i32 = 3;
const IWT: i32 = 6;
const IRELCO: i32 = 1;
const LABEL: &str = "errorr-mf32-ar37";
const ENDF_ORIGINAL: &str = "n-018_Ar_37-tendl2023.endf";
const ENDF_VARIANT: &str = "n-018_Ar_37-tendl2023-mf2-L1-last.endf";
const GOLDEN: &str = "ar37-tendl2023-L1last-293.6K-ign3-iwt6-rel.errorr";
const PENDF: &str = "ar37-tendl2023-L1last-293.6K.pendf";
/// 7 printed figures -> half-ulp 5e-7; 6 for two-digit exponents -> 5e-6.
const TIER1_TOL: f64 = 6e-6;
const TIER1_ABS: f64 = 1e-12;
/// The listing prints 4 significant figures.
const LISTING_TOL: f64 = 6e-4;

// ---------------------------------------------------------------------------
// Golden tape reader (the MF=33 golden test's, unchanged)
// ---------------------------------------------------------------------------

struct GoldenBlock {
    mat1: i32,
    mt1: i32,
    values: Vec<f64>,
}

struct Golden {
    egn: Vec<f64>,
    xs: BTreeMap<i32, Vec<f64>>,
    cov: BTreeMap<i32, Vec<GoldenBlock>>,
}

fn load_golden(path: &Path) -> Golden {
    let tape = Tape::read_file(path).expect("golden ERRORR tape parses");
    let head = tape.section(MAT, 1, 451).expect("MF=1/451");
    let mut cur = SectionCursor::new(&head.rows);
    let _h = cur.read_cont().unwrap();
    let list = cur.read_list().unwrap();
    let ngn = list.head.l1 as usize;
    let egn: Vec<f64> = list.data[..ngn + 1].to_vec();
    let mut xs = BTreeMap::new();
    let mut cov = BTreeMap::new();
    for sec in tape.sections().iter().filter(|s| s.key.mat == MAT) {
        match sec.key.mf {
            3 => {
                let mut cur = SectionCursor::new(&sec.rows);
                let l = cur.read_list().unwrap();
                xs.insert(sec.key.mt, l.data[..ngn].to_vec());
            }
            33 => {
                let mut cur = SectionCursor::new(&sec.rows);
                let h = cur.read_cont().unwrap();
                assert_ne!(h.n2, 0, "no lumped reactions expected on Ar-37");
                let mut blocks = Vec::new();
                while cur.remaining() > 0 {
                    let c = cur.read_cont().unwrap();
                    let (mat1, mt1) = (c.l1, c.l2);
                    let mut values = vec![0.0; ngn * ngn];
                    loop {
                        let l = cur.read_list().unwrap();
                        let ig2lo = l.head.l2 as usize;
                        let ig = l.head.n2 as usize;
                        for (j, &v) in l.data.iter().enumerate() {
                            values[(ig - 1) * ngn + ig2lo - 1 + j] = v;
                        }
                        if ig == ngn {
                            break;
                        }
                    }
                    blocks.push(GoldenBlock { mat1, mt1, values });
                }
                cov.insert(sec.key.mt, blocks);
            }
            _ => {}
        }
    }
    Golden { egn, xs, cov }
}

#[derive(Default, Clone)]
struct Worst {
    rel: f64,
    at: String,
    got: f64,
    want: f64,
}

impl Worst {
    fn note(&mut self, got: f64, want: f64, at: impl FnOnce() -> String) {
        let scale = got.abs().max(want.abs());
        let rel = if scale > 0.0 {
            (got - want).abs() / scale
        } else {
            0.0
        };
        if rel > self.rel {
            self.rel = rel;
            self.at = at();
            self.got = got;
            self.want = want;
        }
    }
}

impl fmt::Display for Worst {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:.3e} at {} (got {:.9e}, njoy {:.9e})",
            self.rel, self.at, self.got, self.want
        )
    }
}

struct Deviation {
    egn: Worst,
    xs: Worst,
    /// The four blocks `rescon` touches: (1,1), (2,2), (2,102), (102,102).
    cov_resonance: Worst,
    /// Every other block (pure MF=33).
    cov_other: Worst,
    blocks: usize,
    nonzero_elements: usize,
}

fn is_resonance_pair(mt: i32, mt1: i32) -> bool {
    matches!((mt, mt1), (1, 1) | (2, 2) | (2, 102) | (102, 102))
}

fn compare(tag: &str, got: &ErrorrResult, golden: &Golden, abs_floor: f64) -> Deviation {
    let ngn = got.ngn();
    assert_eq!(ngn + 1, golden.egn.len(), "{tag}: group count");
    let mut d = Deviation {
        egn: Worst::default(),
        xs: Worst::default(),
        cov_resonance: Worst::default(),
        cov_other: Worst::default(),
        blocks: 0,
        nonzero_elements: 0,
    };
    for (i, (&a, &b)) in got.egn.iter().zip(&golden.egn).enumerate() {
        d.egn.note(a, b, || format!("egn[{}]", i + 1));
    }
    for (ix, &mt) in got.reactions.mts.iter().enumerate() {
        let want = golden
            .xs
            .get(&mt)
            .unwrap_or_else(|| panic!("{tag}: NJOY tape has no MF=3/MT={mt}"));
        for ig in 0..ngn {
            d.xs.note(got.coarse.csig[ix][ig], want[ig], || {
                format!("MT={mt} ig={}", ig + 1)
            });
        }
    }
    let golden_mts: Vec<i32> = golden.xs.keys().copied().collect();
    assert_eq!(
        got.reactions.mts, golden_mts,
        "{tag}: reaction list (MF=3 sections)"
    );
    for b in &got.blocks {
        let gb = golden
            .cov
            .get(&b.mt)
            .and_then(|v| v.iter().find(|g| g.mat1 == b.mat1 && g.mt1 == b.mt1))
            .unwrap_or_else(|| panic!("{tag}: NJOY has no block MT={} MT1={}", b.mt, b.mt1));
        d.blocks += 1;
        let w = if is_resonance_pair(b.mt, b.mt1) {
            &mut d.cov_resonance
        } else {
            &mut d.cov_other
        };
        for ig in 0..ngn {
            for igp in 0..ngn {
                let a = b.values[ig * ngn + igp];
                let want = gb.values[ig * ngn + igp];
                if want != 0.0 {
                    d.nonzero_elements += 1;
                }
                if (a - want).abs() <= abs_floor {
                    continue;
                }
                w.note(a, want, || {
                    format!("MT={} MT1={} ig={} igp={}", b.mt, b.mt1, ig + 1, igp + 1)
                });
            }
        }
    }
    assert_eq!(
        d.blocks,
        golden.cov.values().map(|v| v.len()).sum::<usize>(),
        "{tag}: block count"
    );
    println!(
        "[{tag}] {} blocks, {} non-zero elements | egn {} | sigma_g {} | cov(resonance pairs) {} | cov(other) {}",
        d.blocks, d.nonzero_elements, d.egn, d.xs, d.cov_resonance, d.cov_other
    );
    d
}

fn config() -> Mf33Config {
    Mf33Config {
        matd: MAT,
        ign: IGN,
        user_egn: None,
        weight: ErrorrWeight::from_iwt(IWT, None, None).unwrap(),
        tempin: TEMP_K,
        irelco: IRELCO,
        dap: 0.0,
    }
}

fn run_with_njoy_pendf(endf_file: &str, tag: &str) -> Option<(ErrorrResult, Golden)> {
    let golden_path = reference_file_or_skip("errorr", GOLDEN, LABEL)?;
    let endf_path = reference_endf_or_skip(endf_file, LABEL)?;
    let pendf_path = reference_file_or_skip("errorr", PENDF, LABEL)?;
    let golden = load_golden(&golden_path);
    let endf = Tape::read_file(&endf_path).expect("ENDF parses");
    let pendf = Tape::read_file(&pendf_path).expect("NJOY PENDF parses");
    let t0 = std::time::Instant::now();
    let result = run_mf33(&endf, &pendf, &config()).unwrap_or_else(|e| panic!("{tag}: {e}"));
    println!(
        "[{tag}] run_mf33 with MF=32 took {:.2} s",
        t0.elapsed().as_secs_f64()
    );
    Some((result, golden))
}

// ---------------------------------------------------------------------------
// Tier 1 — NJOY PENDF in, the variant tape NJOY itself can process
// ---------------------------------------------------------------------------

/// **Prediction:** every group cross section and every covariance
/// element within the tape's printing precision (6e-6), the resonance
/// pairs included.
#[test]
fn tier1_variant_tape_matches_njoy_tape23() {
    let tag = "tier1 ar37 (L1-last tape)";
    let Some((got, golden)) = run_with_njoy_pendf(ENDF_VARIANT, tag) else {
        return;
    };
    let rc = got.resonance.as_ref().expect("MF=32 was processed");
    assert!(rc.ifresr && rc.ifunrs, "{tag}: both ranges processed");
    assert_eq!(
        rc.nresg, 30,
        "{tag}: nresg reaches ngn through the URR loop"
    );
    let d = compare(tag, &got, &golden, TIER1_ABS);
    assert!(d.egn.rel < 1e-6, "{tag}: group bounds: {}", d.egn);
    assert!(d.xs.rel < TIER1_TOL, "{tag}: sigma_g: {}", d.xs);
    assert!(
        d.cov_other.rel < TIER1_TOL,
        "{tag}: cov(pure MF=33 blocks): {}",
        d.cov_other
    );
    assert!(
        d.cov_resonance.rel < TIER1_TOL,
        "{tag}: cov(resonance pairs): {}",
        d.cov_resonance
    );
}

/// The unmodified TENDL-2023 tape (which NJOY2016 cannot process, see the
/// module docs) through the port, against the variant-tape oracle.
///
/// **Prediction:** identical up to the float summation order over L in
/// `ggmlbw` (`L = 0,1,2` vs `0,2,1`), i.e. far inside 6e-6.
#[test]
fn original_tape_reproduces_the_variant_oracle() {
    let tag = "tier1 ar37 (original tape)";
    let Some((got, golden)) = run_with_njoy_pendf(ENDF_ORIGINAL, tag) else {
        return;
    };
    let d = compare(tag, &got, &golden, TIER1_ABS);
    assert!(d.xs.rel < TIER1_TOL, "{tag}: sigma_g: {}", d.xs);
    assert!(
        d.cov_resonance.rel < TIER1_TOL,
        "{tag}: cov(resonance pairs): {}",
        d.cov_resonance
    );
    assert!(
        d.cov_other.rel < TIER1_TOL,
        "{tag}: cov(other): {}",
        d.cov_other
    );
}

// ---------------------------------------------------------------------------
// The listing's "resolved / unresolve" diagonals
// ---------------------------------------------------------------------------

/// `ctt/cflx²` and `utt/cflx²` for groups 1..12 as NJOY printed them for
/// the `(MT=1, MT=1)` block (the listing stops at the last group with a
/// resonance contribution; groups 13..30 are zero).
const LISTING_TT: [(f64, f64); 12] = [
    (1.313e-07, 0.0),
    (4.275e-07, 0.0),
    (1.355e-06, 0.0),
    (4.213e-06, 0.0),
    (1.244e-05, 0.0),
    (3.532e-05, 0.0),
    (9.730e-05, 0.0),
    (2.624e-04, 0.0),
    (7.053e-04, 0.0),
    (1.064e-03, 0.0),
    (1.711e-01, 0.0),
    (5.791e-04, 4.479e-06),
];
/// Same for `(MT=2, MT=2)`: `cee`, `uee`.
const LISTING_EE: [(f64, f64); 12] = [
    (7.228e-02, 0.0),
    (7.320e-02, 0.0),
    (7.337e-02, 0.0),
    (7.350e-02, 0.0),
    (7.375e-02, 0.0),
    (7.439e-02, 0.0),
    (7.615e-02, 0.0),
    (8.132e-02, 0.0),
    (9.859e-02, 0.0),
    (1.108e-01, 0.0),
    (2.244e-01, 0.0),
    (1.009e-03, 7.788e-06),
];
/// Same for `(MT=102, MT=102)`: `cgg`, `ugg`.
const LISTING_GG: [(f64, f64); 12] = [
    (3.081e-01, 0.0),
    (3.080e-01, 0.0),
    (3.080e-01, 0.0),
    (3.081e-01, 0.0),
    (3.087e-01, 0.0),
    (3.103e-01, 0.0),
    (3.146e-01, 0.0),
    (3.262e-01, 0.0),
    (3.590e-01, 0.0),
    (4.950e-01, 0.0),
    (1.405e-01, 0.0),
    (8.351e-02, 1.563e-04),
];

/// **Prediction:** the resolved column (groups 1..12) and the unresolved
/// column (group 12 only) agree to the listing's 4 figures for all three
/// blocks; groups 13..30 are exactly zero in both.
#[test]
fn listing_resolved_and_unresolved_diagonals() {
    let tag = "listing ar37";
    let Some((got, _golden)) = run_with_njoy_pendf(ENDF_VARIANT, tag) else {
        return;
    };
    let rc = got.resonance.as_ref().expect("MF=32 was processed");
    let cflx = &got.coarse.cflx;
    let mut worst = Worst::default();
    for (which, mt, table) in [
        ("tt", 1, &LISTING_TT),
        ("ee", 2, &LISTING_EE),
        ("gg", 102, &LISTING_GG),
    ] {
        // irelco = 1: the listing divides the absolute diagonal by
        // csig(ig,ix)*csig(ig,ixp) (errorr.f90:7712-7721)
        let ix = got.reactions.mts.iter().position(|&m| m == mt).unwrap();
        let csig = &got.coarse.csig[ix];
        let rel = |v: Vec<f64>| -> Vec<f64> {
            v.iter()
                .zip(csig)
                .map(|(&a, &s)| if a == 0.0 { 0.0 } else { a / (s * s) })
                .collect()
        };
        let res = rel(rc.diagonal(which, false, cflx));
        let unr = rel(rc.diagonal(which, true, cflx));
        for (ig, &(want_r, want_u)) in table.iter().enumerate() {
            worst.note(res[ig], want_r, || {
                format!("{which} resolved ig={}", ig + 1)
            });
            if want_u == 0.0 {
                assert_eq!(
                    unr[ig],
                    0.0,
                    "{tag}: {which} unresolved ig={} must be zero",
                    ig + 1
                );
            } else {
                worst.note(unr[ig], want_u, || {
                    format!("{which} unresolved ig={}", ig + 1)
                });
            }
        }
        for ig in 12..30 {
            assert_eq!(
                res[ig],
                0.0,
                "{tag}: {which} resolved ig={} beyond the range",
                ig + 1
            );
            assert_eq!(
                unr[ig],
                0.0,
                "{tag}: {which} unresolved ig={} beyond the range",
                ig + 1
            );
        }
        println!(
            "[{tag}] {which}: resolved g12 {:.4e} (njoy {:.4e}) | unresolved g12 {:.4e} (njoy {:.4e})",
            res[11], table[11].0, unr[11], table[11].1
        );
    }
    println!("[{tag}] worst listing deviation {worst}");
    assert!(worst.rel < LISTING_TOL, "{tag}: {worst}");
}

// ---------------------------------------------------------------------------
// Tier 2 — crate RECONR + BROADR in
// ---------------------------------------------------------------------------

/// Reported, not asserted beyond structure: the resonance sensitivities do
/// not depend on the PENDF, so what moves is the `cflx`/`σ_g`
/// normalisation of the relative covariance.
#[test]
fn tier2_crate_pendf_reported() {
    let tag = "tier2 ar37 (crate RECONR+BROADR)";
    let Some(golden_path) = reference_file_or_skip("errorr", GOLDEN, LABEL) else {
        return;
    };
    let Some(endf_path) = reference_endf_or_skip(ENDF_VARIANT, LABEL) else {
        return;
    };
    let golden = load_golden(&golden_path);
    let endf = Tape::read_file(&endf_path).expect("ENDF parses");
    let recon = reconr(
        &endf,
        &ReconrConfig {
            mat: MAT,
            tolerance: 0.001,
            temperature: 0.0,
        },
    )
    .expect("RECONR");
    let broadened = broaden_result(&recon, TEMP_K);
    let sections: Vec<(i32, &[(f64, f64)])> = broadened
        .sections
        .iter()
        .map(|s| (i32::from(s.mt), s.pairs.as_slice()))
        .collect();
    let pendf = Tape::pendf_from_pointwise(
        MAT,
        6,
        broadened.material.za,
        broadened.material.awr,
        TEMP_K,
        sections,
    );
    // Pointwise diagnostic against NJOY's PENDF: separates a RECONR/BROADR
    // difference from anything ERRORR does.
    if let Some(p) = reference_file_or_skip("errorr", PENDF, LABEL) {
        let njoy = Tape::read_file(&p).expect("NJOY PENDF parses");
        for mt in [1, 2, 102] {
            let ours = broadened
                .sections
                .iter()
                .find(|s| i32::from(s.mt) == mt)
                .expect("crate PENDF section");
            let sec = njoy.section(MAT, 3, mt).expect("NJOY PENDF section");
            let mut cur = SectionCursor::new(&sec.rows);
            let _ = cur.read_cont().unwrap();
            let tab = cur.read_tab1().unwrap();
            let line: Vec<String> = [1e-4, 1e-3, 1e-2, 0.0253, 0.1, 1.0, 100.0, 1540.0, 1e5]
                .iter()
                .map(|&e| {
                    let a =
                        njoy_outram_park_fork::endf::interp::eval_tab1(e, &tab.interp, &tab.pairs)
                            .unwrap();
                    let b = njoy_outram_park_fork::endf::interp::eval_tab1(
                        e,
                        &[(ours.pairs.len() as u32, 2)],
                        &ours.pairs,
                    )
                    .unwrap();
                    format!("{e:.3e}: {:+.2e}", (b - a) / a)
                })
                .collect();
            println!(
                "[{tag}] pointwise (crate-njoy)/njoy MT={mt}: {}",
                line.join("  ")
            );
        }
    }
    let got = run_mf33(&endf, &pendf, &config()).unwrap_or_else(|e| panic!("{tag}: {e}"));
    let d = compare(tag, &got, &golden, TIER1_ABS);
    // structure is asserted by `compare`; the numbers are reported above
    assert!(d.egn.rel < 1e-6, "{tag}: group bounds: {}", d.egn);
}
