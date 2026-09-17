//! ERRORR MF=32 through the SAMM derivative path (`rpxsamm`, `LRF=7`
//! `LCOMP=2`) — golden validation against NJOY2016 on ENDF/B-VII.1 Cl-35.
//!
//! # Why Cl-35
//!
//! It is the `LRF=7` case NJOY2016's own test suite uses (`tests/20`,
//! resource `cl35rml`, now `reference-data/endf/n-017_Cl_035-ENDF7.1.endf`):
//! one R-matrix-limited range 1e-5 eV – 1.2 MeV, 8 spin groups, particle
//! pairs `(γ, n, p)`, and an MF=32 `LCOMP=2` section with 1088 parameter
//! uncertainties (`DER`, `DΓ_γ`, `DΓ_n`, `DΓ_p` per resonance) and 3093
//! `NDIGIT=2` INTG correlation lines. The tape has no MF=33, so — as the
//! NJOY deck does — the four dummy sections for `MT=1/2/102/600` are
//! inserted first (`errorr::covadd`, the `999` option).
//!
//! # Deck (`reference-data/errorr/cl35-ENDF7.1-293.6K-ign4-iwt2-rel.njoy-input`)
//!
//! ```text
//! reconr (err 0.001) -> broadr (293.6 K, 0.001) -> errorr
//!   20 22 0 23 0 0 /   1725 4 2 1 1 /   1 293.6 /   0 33 1 1 -1 2e6 0 /
//! ```
//!
//! `ign=4` (27 groups), `iwt=2` (constant), `irelco=1`; `tape20` is the
//! test-20 `covadd` output (`reference-data/endf/...` plus the dummies).
//! NJOY's `tests/20` deck itself takes the group cross sections from a
//! GROUPR GENDF (`ngout`); this deck takes them from the PENDF
//! (`npend`), which is the path the crate implements — the MF=32
//! arithmetic is the same, only `csig`/`cflx` differ at the 1e-3 level.
//!
//! # Tiers and predictions
//!
//! **Tier 1** (NJOY's own PENDF in): the sensitivities depend only on
//! MF=2/MF=32 and the weight, so the whole `rpxsamm` chain — the SAMM
//! derivatives, the panel integration, the INTG covariance and the `crr`
//! fold — is what differs. Prediction: every element of every block
//! within the tape's printing precision (6e-6). The `(1,1)` block is all
//! zero on both sides: the dummy `MT=1` is directly evaluated, so `akxy`
//! gives the total no resonance sensitivity (upstream, `errorr.f90:3632-3641`).
//!
//! **Tier 2** (crate RECONR + BROADR in): reported; only `csig`/`cflx`
//! move.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::errorr::covadd::covadd;
use njoy_outram_park_fork::errorr::{run_mf33, ErrorrResult, ErrorrWeight, Mf33Config};
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};

const MAT: i32 = 1725;
const TEMP_K: f64 = 293.6;
const IGN: i32 = 4;
const IWT: i32 = 2;
const IRELCO: i32 = 1;
const LABEL: &str = "errorr-mf32-cl35-rml";
const ENDF: &str = "n-017_Cl_035-ENDF7.1.endf";
const GOLDEN: &str = "cl35-ENDF7.1-293.6K-ign4-iwt2-rel.errorr";
const PENDF: &str = "cl35-ENDF7.1-293.6K.pendf";
const DUMMY_MTS: [i32; 4] = [1, 2, 102, 600];
/// 7 printed figures -> half-ulp 5e-7; 6 for two-digit exponents -> 5e-6.
const TIER1_TOL: f64 = 6e-6;
const TIER1_ABS: f64 = 1e-12;

// ---------------------------------------------------------------------------
// Golden tape reader (the Ar-37 golden test's, unchanged)
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
                assert_ne!(h.n2, 0, "no lumped reactions expected on Cl-35");
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
    cov: Worst,
    blocks: usize,
    nonzero_elements: usize,
    /// Per block `(mt, mt1)`: non-zero count on the NJOY side, worst.
    per_block: Vec<(i32, i32, usize, f64)>,
}

fn compare(tag: &str, got: &ErrorrResult, golden: &Golden, abs_floor: f64) -> Deviation {
    let ngn = got.ngn();
    assert_eq!(ngn + 1, golden.egn.len(), "{tag}: group count");
    let mut d = Deviation {
        egn: Worst::default(),
        xs: Worst::default(),
        cov: Worst::default(),
        blocks: 0,
        nonzero_elements: 0,
        per_block: Vec::new(),
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
        let mut wb = Worst::default();
        let mut nz = 0usize;
        for ig in 0..ngn {
            for igp in 0..ngn {
                let a = b.values[ig * ngn + igp];
                let want = gb.values[ig * ngn + igp];
                if want != 0.0 {
                    nz += 1;
                }
                if (a - want).abs() <= abs_floor {
                    continue;
                }
                wb.note(a, want, || {
                    format!("MT={} MT1={} ig={} igp={}", b.mt, b.mt1, ig + 1, igp + 1)
                });
            }
        }
        d.nonzero_elements += nz;
        if wb.rel > d.cov.rel {
            d.cov = wb.clone();
        }
        d.per_block.push((b.mt, b.mt1, nz, wb.rel));
    }
    assert_eq!(
        d.blocks,
        golden.cov.values().map(|v| v.len()).sum::<usize>(),
        "{tag}: block count"
    );
    println!(
        "[{tag}] {} blocks, {} non-zero elements | egn {} | sigma_g {} | cov {}",
        d.blocks, d.nonzero_elements, d.egn, d.xs, d.cov
    );
    for (mt, mt1, nz, w) in &d.per_block {
        println!("[{tag}]   block ({mt},{mt1}): {nz} non-zero, worst {w:.3e}");
    }
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

/// The pristine evaluation plus the four dummy MF=33 sections, as NJOY's
/// `999` step wrote `tape21` for the oracle deck.
fn endf_with_dummies() -> Option<Tape> {
    let endf_path = reference_endf_or_skip(ENDF, LABEL)?;
    let endf = Tape::read_file(&endf_path).expect("ENDF parses");
    Some(covadd(&endf, MAT, &DUMMY_MTS).expect("covadd"))
}

fn run_with_njoy_pendf(tag: &str) -> Option<(ErrorrResult, Golden)> {
    let golden_path = reference_file_or_skip("errorr", GOLDEN, LABEL)?;
    let pendf_path = reference_file_or_skip("errorr", PENDF, LABEL)?;
    let endf = endf_with_dummies()?;
    let golden = load_golden(&golden_path);
    let pendf = Tape::read_file(&pendf_path).expect("NJOY PENDF parses");
    let t0 = std::time::Instant::now();
    let result = run_mf33(&endf, &pendf, &config()).unwrap_or_else(|e| panic!("{tag}: {e}"));
    println!(
        "[{tag}] run_mf33 with MF=32 (rpxsamm) took {:.2} s",
        t0.elapsed().as_secs_f64()
    );
    for m in &result.resonance.as_ref().unwrap().messages {
        println!("[{tag}] message: {m}");
    }
    Some((result, golden))
}

// ---------------------------------------------------------------------------
// Tier 1 — NJOY PENDF in
// ---------------------------------------------------------------------------

/// **Prediction:** every group cross section and every covariance
/// element of the ten blocks within the tape's printing precision.
#[test]
fn tier1_matches_njoy_tape23() {
    let tag = "tier1 cl35 rml";
    let Some((got, golden)) = run_with_njoy_pendf(tag) else {
        return;
    };
    let rc = got.resonance.as_ref().expect("MF=32 was processed");
    assert!(rc.sammy, "{tag}: the SAMMY branch ran");
    assert_eq!(rc.nmt, 4);
    assert_eq!(got.reactions.mts, vec![1, 2, 102, 600]);
    let d = compare(tag, &got, &golden, TIER1_ABS);
    assert!(d.egn.rel < 1e-6, "{tag}: group bounds: {}", d.egn);
    assert!(d.xs.rel < TIER1_TOL, "{tag}: sigma_g: {}", d.xs);
    assert!(d.cov.rel < TIER1_TOL, "{tag}: covariance: {}", d.cov);
    // the resonance covariance is non-trivial: elastic, capture and (n,p)
    // blocks all carry it
    for (mt, mt1) in [(2, 2), (102, 102), (600, 600), (2, 102), (2, 600), (102, 600)] {
        let b = d
            .per_block
            .iter()
            .find(|b| b.0 == mt && b.1 == mt1)
            .unwrap();
        assert!(b.2 > 0, "{tag}: block ({mt},{mt1}) has non-zero elements");
    }
    let tt = d.per_block.iter().find(|b| b.0 == 1 && b.1 == 1).unwrap();
    assert_eq!(tt.2, 0, "{tag}: the directly-evaluated dummy MT=1 gets no sensitivity");
}

// ---------------------------------------------------------------------------
// Tier 2 — crate RECONR + BROADR in (reported)
// ---------------------------------------------------------------------------

/// **Prediction:** the covariances move only through `csig`/`cflx`; the
/// crate's LRF=7 RECONR agrees with NJOY's to a few 1e-4 on elastic and
/// capture (`tests/reconr_cl35_rml_njoy_golden.rs`), so the elements
/// should sit within ~1e-3. Asserted loosely at 1e-2, reported exactly.
#[test]
fn tier2_crate_pendf_reported() {
    let tag = "tier2 cl35 rml (crate PENDF)";
    let Some(golden_path) = reference_file_or_skip("errorr", GOLDEN, LABEL) else {
        return;
    };
    let Some(endf) = endf_with_dummies() else {
        return;
    };
    let golden = load_golden(&golden_path);
    let r0 = reconr(
        &endf,
        &ReconrConfig {
            mat: MAT,
            tolerance: 0.001,
            temperature: 0.0,
        },
    )
    .expect("RECONR");
    let broadened = broaden_result(&r0, TEMP_K);
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
    let got = run_mf33(&endf, &pendf, &config()).unwrap_or_else(|e| panic!("{tag}: {e}"));
    let d = compare(tag, &got, &golden, TIER1_ABS);
    assert!(d.xs.rel < 1e-2, "{tag}: sigma_g: {}", d.xs);
    assert!(d.cov.rel < 1e-2, "{tag}: covariance: {}", d.cov);
}
