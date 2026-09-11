//! GROUPR **MF=10 residual-production** cross sections (`op-urh`: the
//! `getsig` MF=10 yield search, `groupr.f90:6719-6746`) vs an NJOY2016 GENDF
//! tape — U-235 `MT=4` into the ground state and into the `235m` isomer.
//!
//! # Oracle
//! `reference-data/gendf/u235-ENDF8.0-293.6K-29g-iwt3-1sigz-mf10.njoy-input`
//! (NJOY2016 `ac5adf5`, 2026-09-10): RECONR (`err = 0.002`) → BROADR
//! (293.6 K) → GROUPR `9228 1 0 3 0 1 1 1` (29 groups, `iwt = 3`,
//! `nsigz = 1`), reactions `3 1`, `3 4`, **`40922350 4`** (MF=10, residual
//! ZA 92235, level 0) and **`40922351 4`** (level 1, the isomer). GROUPR
//! writes both as `MF=3/MT=4` sections — alongside the ordinary `3 4` one —
//! whose HEAD `C2` carries `izam = 922350` / `922351` (`:696`, `:875`).
//!
//! # What upstream does, read before measuring
//! - `getsig` reads MF=10 from the **PENDF** (`nsig = npend`), and RECONR
//!   does not copy the file through untouched: it rewrites the lin-lin
//!   88/87-point ENDF tables onto its union grid (675 points on this run).
//!   The subsection is chosen by its TAB1 `L1 = izar`, `L2 = lfs`;
//!   `q = c2h` of that TAB1; `lrflag = 0` (`:6749-6750`).
//! - With `nsigz = 1` the flux is `getwtf`'s bare 1/E weight on its 1 %
//!   ladder (`getflx`, `:6512-6516`), so the total cross section is not
//!   needed; `panel` integrates on the union of that ladder and the MF=10
//!   grid, and groups whose top lies below `gety1`'s "first" energy — the
//!   last leading zero point times 0.999999 (`endf.f90` label 100) — are
//!   not written: the ENDF table starts (77.33 eV, 0), (17.7 keV, 0), so
//!   the first record is group 21 on both tables.
//!
//! # Two tiers
//! 1. **Strict** — pointwise input = the MF=10 of NJOY's own 293.6 K U-235
//!    PENDF (`OUTRAM_PARK_NJOY_U235_PENDF`, not committed; the deck
//!    regenerates it). Prediction: identical record sets and every flux /
//!    cross-section word within 1e-5 (seven-figure storage).
//! 2. **Committed** — the same with the ENDF tape's own 88/87-point MF=10.
//!    The values are the same piecewise-linear function, so the only
//!    difference is the panel rule (`sigma*phi` linear per sub-panel) on a
//!    coarser grid; expected O(1e-4), gated at 1e-3.
//!
//! **Result (2026-09-10, first run):** tier 1 — both subsections, 9 records
//! each (groups 21-29), every word within **2.2e-6**; tier 2 — within
//! **4.9e-5** (group 21, where the 17.7-22 keV rise sits in one ENDF panel).
//! The only defect surfaced was in the test itself: `first` must follow
//! `gety1`'s leading-zero scan ([`gety1_first_energy`]), not `x(1)`.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::groupr::gendf::GendfSection;
use njoy_outram_park_fork::groupr::panel::{group_integral, GroupFlux};
use njoy_outram_park_fork::groupr::pendf_feed::{
    decode_extended_mfd, gety1_first_energy, read_pendf_mf10_cross_section,
};
use njoy_outram_park_fork::groupr::weights::AnalyticWeight;
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};

const MAT: i32 = 9228;
const TEMP_K: f64 = 293.6;
const LABEL: &str = "groupr-u235-mf10";
const ENDF: &str = "n-092_U_235-ENDF8.0.endf";
const GOLDEN: &str = "u235-ENDF8.0-293.6K-29g-iwt3-1sigz-mf10.gendf";
const BOUNDS: [f64; 30] = [
    1e-5, 0.1, 0.625, 1.0, 4.0, 6.5, 8.0, 10.0, 15.0, 20.0, 25.0, 30.0, 40.0, 50.0, 100.0, 200.0,
    500.0, 1e3, 2e3, 5e3, 1e4, 2e4, 5e4, 1e5, 5e5, 1e6, 2e6, 5e6, 1e7, 2e7,
];
const NJOY_PENDF_ENV: &str = "OUTRAM_PARK_NJOY_U235_PENDF";
const TOL_STRICT: f64 = 1e-5;
const TOL_ENDF: f64 = 1e-3;

/// Compare both MF=10 subsections read from `source` against the golden.
fn compare(source: &Tape, golden: &Tape, label: &str) -> f64 {
    let weight = GroupFlux::analytic(AnalyticWeight::OneOverE, TEMP_K);
    let mut worst_all = 0.0f64;
    for mfd in [40_922_350i32, 40_922_351] {
        let ext = decode_extended_mfd(mfd).expect("extended mfd");
        assert_eq!((ext.file_class, ext.izar, ext.lfs), (4, 92235, mfd % 10));
        assert_eq!(ext.izam, 922_350 + mfd % 10);
        let xs = read_pendf_mf10_cross_section(source, MAT, 4, ext.izar, ext.lfs).unwrap();
        // The GENDF section with this izam in its HEAD C2 (`groupr.f90:875`).
        let sec = golden
            .sections()
            .iter()
            .find(|s| {
                s.key.mat == MAT
                    && s.key.mf == 3
                    && s.key.mt == 4
                    && (s.rows[0][1] - f64::from(ext.izam)).abs() < 0.5
            })
            .unwrap_or_else(|| panic!("golden MF=3/MT=4 section with zam {}", ext.izam));
        let theirs = GendfSection::from_rows(3, 4, &sec.rows).expect("GENDF decodes");
        assert_eq!((theirs.nl, theirs.nz), (1, 1));
        // `first` per gety1's leading-zero scan: the ENDF table starts
        // (77.33 eV, 0), (17.7 keV, 0), ... so it is 0.999999 x 17.7 keV.
        let first = match &xs.xs {
            njoy_outram_park_fork::groupr::panel::PointwiseXs::LinLin(p) => gety1_first_energy(p),
            _ => unreachable!(),
        };
        // Records: every group whose top lies above `first` (`groupr.f90:520`).
        let expect_groups: Vec<i32> = (1..=29).filter(|&g| BOUNDS[g as usize] > first).collect();
        assert_eq!(
            theirs.records.iter().map(|r| r.ig).collect::<Vec<_>>(),
            expect_groups,
            "mfd {mfd}: groups written"
        );
        let mut worst = (0.0f64, 0i32, "", 0.0f64, 0.0f64);
        for rec in &theirs.records {
            let g = rec.ig as usize;
            let integ = group_integral(&xs.xs, &weight, BOUNDS[g - 1], BOUNDS[g]);
            for (ours, njoy, what) in [
                (integ.flux, rec.data[0], "flux"),
                (integ.average(), rec.data[1], "sigma"),
            ] {
                let r = ((ours - njoy) / njoy).abs();
                if r > worst.0 {
                    worst = (r, rec.ig, what, ours, njoy);
                }
            }
        }
        println!(
            "[{LABEL}/{label}] mfd {mfd} (izam {}, QI = {} eV, {} records): worst rel dev \
             {:.3e} ({} group {}: ours {:.7e} vs NJOY {:.7e})",
            ext.izam,
            xs.qi,
            theirs.records.len(),
            worst.0,
            worst.2,
            worst.1,
            worst.3,
            worst.4
        );
        worst_all = worst_all.max(worst.0);
    }
    worst_all
}

/// Tier 1 — NJOY's PENDF MF=10 (strict, skips without the env var).
#[test]
fn njoy_pendf_mf10_isomer_production_matches() {
    let Some(golden_path) = reference_file_or_skip("gendf", GOLDEN, LABEL) else {
        return;
    };
    let Ok(pendf_path) = std::env::var(NJOY_PENDF_ENV) else {
        println!("[{LABEL}] SKIP tier 1: set {NJOY_PENDF_ENV} to the NJOY 293.6 K U-235 PENDF");
        return;
    };
    let pendf = Tape::read_file(std::path::Path::new(&pendf_path)).expect("NJOY PENDF parses");
    let golden = Tape::read_file(&golden_path).expect("golden GENDF parses");
    let worst = compare(&pendf, &golden, "pendf");
    assert!(
        worst < TOL_STRICT,
        "MF=10 production deviates from NJOY: {worst:e}"
    );
}

/// Tier 2 — the committed ENDF tape's MF=10 (coarser grid, looser gate).
#[test]
fn endf_mf10_isomer_production_matches_loosely() {
    let Some(endf_path) = reference_endf_or_skip(ENDF, LABEL) else {
        return;
    };
    let Some(golden_path) = reference_file_or_skip("gendf", GOLDEN, LABEL) else {
        return;
    };
    let endf = Tape::read_file(&endf_path).expect("U-235 tape parses");
    let golden = Tape::read_file(&golden_path).expect("golden GENDF parses");
    let worst = compare(&endf, &golden, "endf");
    assert!(
        worst < TOL_ENDF,
        "MF=10 production (ENDF grid) deviates from NJOY: {worst:e}"
    );
}
