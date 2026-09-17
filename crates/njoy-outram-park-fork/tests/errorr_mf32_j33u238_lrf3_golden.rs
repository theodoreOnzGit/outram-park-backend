//! ERRORR MF=32 through the ERRORJ branch for **Reich-Moore** resonances
//! (`LRF=3`, `ggrmat`) with the **`LCOMP=1`** general covariance format —
//! golden validation against NJOY2016 on JENDL-3.3 U-238 (MAT 9237).
//!
//! # Why JENDL-3.3 U-238
//!
//! It is the `LCOMP=1` case NJOY2016's own test suite uses (`tests/15`,
//! `16`, `17`; resource `J33U238`, now
//! `reference-data/endf/n-092_U_238-JENDL3.3.endf`): MF=2 has ten
//! resolved `LRF=3` ranges of 1 keV each from 1e-5 eV to 10 keV plus an
//! `LRU=2/LRF=2` URR to 150 keV, and MF=32 carries one `LCOMP=1`
//! short-range block per resolved range (`NSRS=1`, `MPAR=3`: `ER`, `Γn`,
//! `Γγ`; 26–37 resonances each, full covariance LIST) and an `LRU=2`
//! block. So one material exercises `ggrmat` (the 1-channel R-function
//! path — the only fission width on file is `1e-10`-scale, see below —
//! and the `(L, J)`-selected evaluation `rpendf` asks for), the `LCOMP=1`
//! reader, ten consecutive `rpxlc12` ranges with their own `iest/ieed`
//! windows, and `rpxunr`.
//!
//! # Deck (`reference-data/errorr/u238-JENDL3.3-300K-ign3-iwt6-rel.njoy-input`)
//!
//! ```text
//! reconr (err 0.001) -> broadr (300 K, 0.001) -> errorr
//!   20 22 0 23 0 0 /   9237 3 6 1 1 /   1 300. /   0 33 1 1 -1 2e6 0 /
//! ```
//!
//! # Tiers and predictions
//!
//! **Tier 1** (NJOY's 300 K PENDF in — 25 MB, not committed: set
//! `OUTRAM_PARK_NJOY_J33U238_PENDF` to `tape22` of the deck above; the
//! test skips otherwise): every group cross section and every element of
//! every block within the tape's printing precision (6e-6). Upstream's
//! own listing warns `mf2 nls=2, but mf32 nls=0` for each range — the
//! `LCOMP=1` CONT carries no `NLS` — and continues; the port does the same.
//!
//! **Listing** (always run, on the crate's own PENDF): the
//! `resolved`/`unresolve` diagonals NJOY printed for the seven resonance
//! pairs, 4 significant figures. These are ratios of MF=32 quantities to
//! `cflx² csig²`, so they move with the PENDF only through `csig`; pinned
//! at 2e-2 here (tier 2), the tier-1 run pins them at 6e-4.
//!
//! **Tier 2** (crate RECONR + BROADR in): the crate's RECONR does not yet
//! reconstruct the infinitely-dilute averages of an `LSSF=0` unresolved
//! range (JENDL-3.3's MF=3 elastic is zero up to 150 keV; NJOY's `sigunr`
//! supplies the URR average — `src/reconr/README.md` lists LRU=2 as
//! "next to port"; bead `op-t0wt`), so its PENDF has no elastic in the
//! URR groups 13–18 (9120 eV – 183 keV) and everything that touches them
//! is reported only. A pointwise probe (2026-09-11) showed the crate's
//! RECONR within 1e-4 of NJOY's `tape21` at 3.5, 5, 7 and 9 keV — the ten
//! resolved ranges are right — and zero from 10 to 150 keV. Groups 1–12
//! (below 9120 eV) are asserted at 2e-2 on `σ_g` and 5e-2 on the
//! covariances, and the resolved listing rows at 2e-2.
//!
//! One more exclusion in tier 2, measured 2026-09-11: U-238's fission in
//! the resolved range is sub-threshold, `σ_g(MT=18) ≈ 1e-7 b` (group 11:
//! crate 9.225e-8 b, NJOY 8.623e-8 b, 6.5 % apart). Both RECONR/BROADR
//! pipelines accept a panel once its resonance-integral error is under
//! `errint = err/20000 = 5e-8 b` (`reconr.f90:2401`, `broadr.f90:209`),
//! so a cross section of that size is not resolved by either and its
//! value depends on the grid each pipeline kept. The *relative*
//! covariance inherits `1/σ_g²`: the (18,18) g11 element came out at
//! 0.8736 of NJOY's, and `(8.623/9.225)² = 0.8736` — the absolute
//! covariance agrees, only the normalisation moved. Tier 2 therefore
//! compares no group cross section under 1e-6 b and no covariance
//! element (or listing row) whose row or column partial is under 1e-6 b
//! in that group. Tier 1 has no floor and pins every element.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use njoy_outram_park_fork::broadr::broaden_result;
use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::errorr::{run_mf33, ErrorrResult, ErrorrWeight, Mf33Config};
use njoy_outram_park_fork::reconr::{reconr, ReconrConfig};
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};

const MAT: i32 = 9237;
const TEMP_K: f64 = 300.0;
const IGN: i32 = 3;
const IWT: i32 = 6;
const IRELCO: i32 = 1;
const LABEL: &str = "errorr-mf32-j33u238-lrf3";
const ENDF: &str = "n-092_U_238-JENDL3.3.endf";
const GOLDEN: &str = "u238-JENDL3.3-300K-ign3-iwt6-rel.errorr";
const PENDF_ENV: &str = "OUTRAM_PARK_NJOY_J33U238_PENDF";
/// 7 printed figures -> half-ulp 5e-7; 6 for two-digit exponents -> 5e-6.
const TIER1_TOL: f64 = 6e-6;
const TIER1_ABS: f64 = 1e-12;
/// The listing prints 4 significant figures.
const LISTING_TOL: f64 = 6e-4;

// ---------------------------------------------------------------------------
// Golden tape reader (the Cl-35 golden test's)
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
                assert_ne!(h.n2, 0, "no lumped reactions expected on JENDL-3.3 U-238");
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

fn is_resonance_pair(mt: i32, mt1: i32) -> bool {
    matches!(
        (mt, mt1),
        (1, 1) | (2, 2) | (2, 18) | (2, 102) | (18, 18) | (18, 102) | (102, 102)
    )
}

struct Deviation {
    xs: Worst,
    cov_resonance: Worst,
    cov_other: Worst,
    blocks: usize,
    nonzero: usize,
}

/// `max_group`: only groups `1..=max_group` (both indices) are compared —
/// tier 2 stops below the URR (see the module docs). `xs_floor` \[b\]: a
/// group cross section below it is not compared, and neither is any
/// covariance element whose row or column partial is below it in that
/// group (tier 2's sub-threshold fission, see the module docs); tier 1
/// passes 0.
fn compare(
    tag: &str,
    got: &ErrorrResult,
    golden: &Golden,
    abs_floor: f64,
    max_group: usize,
    xs_floor: f64,
) -> Deviation {
    let ngn = got.ngn();
    let gmax = max_group.min(ngn);
    assert_eq!(ngn + 1, golden.egn.len(), "{tag}: group count");
    for (i, (&a, &b)) in got.egn.iter().zip(&golden.egn).enumerate() {
        assert!((a - b).abs() <= 1e-6 * b, "{tag}: egn[{}]", i + 1);
    }
    let mut d = Deviation {
        xs: Worst::default(),
        cov_resonance: Worst::default(),
        cov_other: Worst::default(),
        blocks: 0,
        nonzero: 0,
    };
    for (ix, &mt) in got.reactions.mts.iter().enumerate() {
        let want = golden
            .xs
            .get(&mt)
            .unwrap_or_else(|| panic!("{tag}: NJOY tape has no MF=3/MT={mt}"));
        for ig in 0..gmax {
            if got.coarse.csig[ix][ig].abs() < xs_floor && want[ig].abs() < xs_floor {
                continue;
            }
            d.xs.note(got.coarse.csig[ix][ig], want[ig], || {
                format!("MT={mt} ig={}", ig + 1)
            });
        }
    }
    let golden_mts: Vec<i32> = golden.xs.keys().copied().collect();
    assert_eq!(got.reactions.mts, golden_mts, "{tag}: reaction list");
    for b in &got.blocks {
        let gb = golden
            .cov
            .get(&b.mt)
            .and_then(|v| v.iter().find(|g| g.mat1 == b.mat1 && g.mt1 == b.mt1))
            .unwrap_or_else(|| panic!("{tag}: NJOY has no block MT={} MT1={}", b.mt, b.mt1));
        d.blocks += 1;
        let ix = got.reactions.mts.iter().position(|&m| m == b.mt).unwrap();
        let ixp = got.reactions.mts.iter().position(|&m| m == b.mt1).unwrap();
        let w = if is_resonance_pair(b.mt, b.mt1) {
            &mut d.cov_resonance
        } else {
            &mut d.cov_other
        };
        for ig in 0..gmax {
            for igp in 0..gmax {
                let a = b.values[ig * ngn + igp];
                let want = gb.values[ig * ngn + igp];
                if want != 0.0 {
                    d.nonzero += 1;
                }
                if (a - want).abs() <= abs_floor {
                    continue;
                }
                if got.coarse.csig[ix][ig].abs() < xs_floor
                    || got.coarse.csig[ixp][igp].abs() < xs_floor
                {
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
        "[{tag}] {} blocks, {} non-zero elements | sigma_g {} | cov(resonance pairs) {} | cov(other) {}",
        d.blocks, d.nonzero, d.xs, d.cov_resonance, d.cov_other
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

fn njoy_pendf_path() -> Option<PathBuf> {
    match std::env::var(PENDF_ENV) {
        Ok(p) if Path::new(&p).is_file() => Some(PathBuf::from(p)),
        _ => {
            println!("[{LABEL}] skipped: set {PENDF_ENV} to NJOY's tape22 for the tier-1 run");
            None
        }
    }
}

/// The listing's `resolved` / `unresolve` diagonals (`errorr.f90:7660-7721`,
/// relative, 4 figures) for the seven pairs, `(ig, resolved, unresolved)`;
/// `None` where the listing has no unresolved column for the pair.
type Row = (usize, f64, Option<f64>);
fn listing() -> Vec<(&'static str, i32, i32, Vec<Row>)> {
    let u = Some;
    vec![
        (
            "tt",
            1,
            1,
            vec![
                (1, 3.580e-05, u(0.0)),
                (2, 4.065e-05, u(0.0)),
                (3, 4.610e-05, u(0.0)),
                (4, 5.595e-05, u(0.0)),
                (5, 3.529e-04, u(0.0)),
                (6, 2.334e-02, u(0.0)),
                (7, 9.331e-04, u(0.0)),
                (8, 1.054e-04, u(0.0)),
                (9, 4.208e-04, u(0.0)),
                (10, 3.169e-06, u(0.0)),
                (11, 1.985e-05, u(0.0)),
                (12, 3.883e-06, u(0.0)),
                (13, 8.706e-08, u(8.385e-04)),
                (14, 0.0, u(3.719e-04)),
                (15, 0.0, u(9.920e-05)),
            ],
        ),
        (
            "ee",
            2,
            2,
            vec![
                (1, 5.133e-05, u(0.0)),
                (2, 5.276e-05, u(0.0)),
                (3, 5.626e-05, u(0.0)),
                (4, 6.825e-05, u(0.0)),
                (5, 5.834e-05, u(0.0)),
                (6, 1.024e-01, u(0.0)),
                (7, 2.687e-03, u(0.0)),
                (8, 2.121e-04, u(0.0)),
                (9, 7.002e-04, u(0.0)),
                (10, 4.686e-06, u(0.0)),
                (11, 2.457e-05, u(0.0)),
                (12, 4.624e-06, u(0.0)),
                (13, 9.558e-08, u(8.316e-04)),
                (14, 0.0, u(3.703e-04)),
                (15, 0.0, u(1.068e-04)),
            ],
        ),
        ("ef", 2, 18, vec![(8, 4.901e-06, None)]),
        ("eg", 2, 102, vec![(9, 6.864e-08, None)]),
        (
            "ff",
            18,
            18,
            vec![
                (1, 8.027e-05, u(0.0)),
                (2, 8.518e-05, u(0.0)),
                (3, 9.781e-05, u(0.0)),
                (4, 1.381e-04, u(0.0)),
                (5, 1.264e-05, u(0.0)),
                (6, 6.173e-02, u(0.0)),
                (7, 7.800e-04, u(0.0)),
                (8, 1.721e-05, u(0.0)),
                (9, 9.587e-05, u(0.0)),
                (10, 1.779e-07, u(0.0)),
                (11, 4.376e-06, u(0.0)),
                (12, 5.221e-04, u(0.0)),
                (13, 1.736e-09, u(0.0)),
            ],
        ),
        (
            "fg",
            18,
            102,
            vec![
                (1, 8.943e-05, u(0.0)),
                (2, 2.719e-04, u(0.0)),
                (9, 8.265e-08, u(0.0)),
            ],
        ),
        (
            "gg",
            102,
            102,
            vec![
                (1, 3.589e-04, u(0.0)),
                (2, 3.795e-04, u(0.0)),
                (3, 4.333e-04, u(0.0)),
                (4, 6.457e-04, u(0.0)),
                (5, 4.198e-04, u(0.0)),
                (6, 3.788e-03, u(0.0)),
                (7, 5.265e-04, u(0.0)),
                (8, 2.927e-06, u(0.0)),
                (9, 3.869e-04, u(0.0)),
                (10, 3.054e-06, u(0.0)),
                (11, 9.124e-05, u(0.0)),
                (12, 2.256e-05, u(0.0)),
                (13, 6.404e-07, u(2.870e-03)),
                (14, 0.0, u(1.182e-03)),
                (15, 0.0, u(1.750e-03)),
            ],
        ),
    ]
}

/// Compare the listing's diagonals (rows `ig <= max_group` only); returns
/// the worst relative deviation.
fn check_listing(
    tag: &str,
    got: &ErrorrResult,
    tol: f64,
    max_group: usize,
    xs_floor: f64,
) -> f64 {
    let rc = got.resonance.as_ref().expect("MF=32 was processed");
    let cflx = &got.coarse.cflx;
    let mut worst = Worst::default();
    for (which, mt, mt1, rows) in listing() {
        let ix = got.reactions.mts.iter().position(|&m| m == mt).unwrap();
        let ixp = got.reactions.mts.iter().position(|&m| m == mt1).unwrap();
        let rel = |v: Vec<f64>| -> Vec<f64> {
            v.iter()
                .enumerate()
                .map(|(ig, &a)| {
                    let s = got.coarse.csig[ix][ig] * got.coarse.csig[ixp][ig];
                    if a == 0.0 {
                        0.0
                    } else {
                        a / s
                    }
                })
                .collect()
        };
        let res = rel(rc.diagonal(which, false, cflx));
        let unr = rel(rc.diagonal(which, true, cflx));
        for (ig, want_r, want_u) in rows {
            if ig > max_group {
                continue;
            }
            if got.coarse.csig[ix][ig - 1].abs() < xs_floor
                || got.coarse.csig[ixp][ig - 1].abs() < xs_floor
            {
                continue;
            }
            worst.note(res[ig - 1], want_r, || format!("{which} resolved ig={ig}"));
            if let Some(wu) = want_u {
                worst.note(unr[ig - 1], wu, || format!("{which} unresolved ig={ig}"));
            }
        }
        println!(
            "[{tag}] {which} ({mt},{mt1}): resolved g6 {:.4e} | resolved g13 {:.4e} | unresolved g13 {:.4e}",
            res[5], res[12], unr[12]
        );
    }
    println!("[{tag}] listing diagonals worst {worst}");
    assert!(worst.rel < tol, "{tag}: listing diagonals: {worst}");
    worst.rel
}

// ---------------------------------------------------------------------------
// Tier 1 — NJOY PENDF in (env-gated)
// ---------------------------------------------------------------------------

/// **Prediction:** every group cross section and covariance element
/// within the printing precision; the listing diagonals to 4 figures.
#[test]
fn tier1_env_gated_matches_njoy_tape23() {
    let tag = "tier1 j33 u238 lrf3 lcomp1";
    let Some(golden_path) = reference_file_or_skip("errorr", GOLDEN, LABEL) else {
        return;
    };
    let Some(endf_path) = reference_endf_or_skip(ENDF, LABEL) else {
        return;
    };
    let Some(pendf_path) = njoy_pendf_path() else {
        return;
    };
    let golden = load_golden(&golden_path);
    let endf = Tape::read_file(&endf_path).expect("ENDF parses");
    let pendf = Tape::read_file(&pendf_path).expect("NJOY PENDF parses");
    let t0 = std::time::Instant::now();
    let got = run_mf33(&endf, &pendf, &config()).unwrap_or_else(|e| panic!("{tag}: {e}"));
    println!("[{tag}] run_mf33 took {:.2} s", t0.elapsed().as_secs_f64());
    let rc = got.resonance.as_ref().unwrap();
    for m in &rc.messages {
        println!("[{tag}] message: {m}");
    }
    assert!(rc.ifresr && rc.ifunrs, "{tag}: resolved and unresolved ranges processed");
    assert!(!rc.sammy);
    let d = compare(tag, &got, &golden, TIER1_ABS, usize::MAX, 0.0);
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
    check_listing(tag, &got, LISTING_TOL, usize::MAX, 0.0);
}

// ---------------------------------------------------------------------------
// Tier 2 — crate RECONR + BROADR in (always runs; reported)
// ---------------------------------------------------------------------------

#[test]
fn tier2_crate_pendf_and_listing() {
    let tag = "tier2 j33 u238 (crate PENDF)";
    let Some(golden_path) = reference_file_or_skip("errorr", GOLDEN, LABEL) else {
        return;
    };
    let Some(endf_path) = reference_endf_or_skip(ENDF, LABEL) else {
        return;
    };
    let golden = load_golden(&golden_path);
    let endf = Tape::read_file(&endf_path).expect("ENDF parses");
    let t0 = std::time::Instant::now();
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
    println!(
        "[{tag}] crate RECONR + BROADR took {:.1} s",
        t0.elapsed().as_secs_f64()
    );
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
    // the whole tape, reported: the URR groups (14-18) carry the RECONR gap
    let _all = compare(
        &format!("{tag} all groups, reported"),
        &got,
        &golden,
        TIER1_ABS,
        usize::MAX,
        0.0,
    );
    // groups 1..=12 lie below 9120 eV, inside the resolved ranges (group 13
    // is 9120-24800 eV, mostly the LSSF=0 URR the crate does not reconstruct)
    const RESOLVED_GROUPS: usize = 12;
    // sub-threshold fission (module docs): group cross sections under 1e-6 b
    // sit below the reconstruction pipelines' integral criterion
    const TIER2_XS_FLOOR: f64 = 1e-6;
    let d = compare(tag, &got, &golden, TIER1_ABS, RESOLVED_GROUPS, TIER2_XS_FLOOR);
    assert!(d.xs.rel < 2e-2, "{tag}: sigma_g: {}", d.xs);
    assert!(
        d.cov_resonance.rel < 5e-2,
        "{tag}: cov(resonance pairs): {}",
        d.cov_resonance
    );
    check_listing(tag, &got, 2e-2, RESOLVED_GROUPS, TIER2_XS_FLOOR);
}
