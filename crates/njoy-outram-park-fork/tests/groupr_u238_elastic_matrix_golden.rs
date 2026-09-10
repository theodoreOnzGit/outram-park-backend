//! GROUPR **elastic transfer matrix** (MF=6/MT=2) vs an NJOY2016 GENDF —
//! the `getdis`/`panel`/`displa` matrix path end to end.
//!
//! # Oracle
//! `reference-data/dtfr/u238-ENDF8.0-293.6K-29g-iwt3-6sigz-mf6.gendf`
//! (NJOY2016 `ac5adf5`, gfortran 13.3.0, 2026-09-10; see that directory's
//! README): U-238 (MAT 9237) at 293.6 K, 29 groups, `iwt = 3` (1/E),
//! six sigma-zero values, `lord = 0`, so the section carries `NL = 1`,
//! `NZ = 6` and, per initial group, the group flux (slot 1) and the P0
//! transfer elements to `ng2 - 1` secondary groups from `ig2lo`.
//!
//! # Methodology (tier 1 — engine isolation)
//! Pointwise input = NJOY's own 293.6 K PENDF (`OUTRAM_PARK_NJOY_U238_PENDF`,
//! the same tape the oracle's GROUPR read), so any difference is the group
//! averaging itself, not RECONR/BROADR. The Bondarenko flux is tabulated on
//! NJOY's greedy `genflx` grid exactly as in `groupr_u238_gendf_golden.rs`
//! (that test pins the group fluxes to 1e-5); the File-4 MT=2 distribution
//! (`LTT = 3`, CM) comes from the committed ENDF tape; the feed is
//! `getdis` and the quadrature `panel` (`groupr::two_body`,
//! `groupr::matrix_panel`).
//!
//! **Prediction**, stated before running: identical record structure (same
//! initial groups written, same `ig2lo`/`ng2` per group), and every word —
//! flux and transfer element — within **1e-5** relative of the GENDF. Both
//! sides round to seven significant figures, so the residual should be
//! O(1e-6); a structural mismatch or a >1e-4 element would mean a real
//! port defect, not rounding.
//!
//! **Result (2026-09-10):** all 29 initial groups written on both sides
//! with identical `(ig2lo, ng2)`; 516 words compared; worst transfer
//! element 5.52e-6 (group 14, first element at σ0 = ∞: 0.14482830 vs
//! NJOY 0.14482910), worst group flux 1.44e-7. PASS at 1e-5. The elastic
//! PENDF grid has no duplicate energies, so `gety1`'s discontinuity path
//! is not exercised by this oracle.

use std::sync::Arc;

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::groupr::file4::{File4Angular, NLD};
use njoy_outram_park_fork::groupr::gendf::GendfSection;
use njoy_outram_park_fork::groupr::matrix_panel::{two_body_matrix, MatrixHeader};
use njoy_outram_park_fork::groupr::panel::{GroupFlux, PointwiseXs};
use njoy_outram_park_fork::groupr::pendf_feed::read_pendf_cross_section;
use njoy_outram_park_fork::groupr::two_body::TwoBodyFeed;
use njoy_outram_park_fork::groupr::unresolved::genflx_bondarenko_urr;
use njoy_outram_park_fork::groupr::weights::AnalyticWeight;
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};

const LABEL: &str = "groupr-u238-elastic-matrix";
const MAT: i32 = 9237;
const TEMP_K: f64 = 293.6;
const GOLDEN: &str = "u238-ENDF8.0-293.6K-29g-iwt3-6sigz-mf6.gendf";
const NJOY_PENDF_ENV: &str = "OUTRAM_PARK_NJOY_U238_PENDF";
const ENDF: &str = "n-092_U_238.endf";

/// The 30 group breaks of the deck (card 6b), eV, ascending.
const BOUNDS: [f64; 30] = [
    1e-5, 0.1, 0.625, 1.0, 4.0, 6.5, 8.0, 10.0, 15.0, 20.0, 25.0, 30.0, 40.0, 50.0, 100.0, 200.0,
    500.0, 1e3, 2e3, 5e3, 1e4, 2e4, 5e4, 1e5, 5e5, 1e6, 2e6, 5e6, 1e7, 2e7,
];
const N_GROUPS: usize = BOUNDS.len() - 1;
/// Card 5 `sigz`, barn, in NJOY's order (infinity first).
const SIGZ: [f64; 6] = [1e10, 1e4, 1e3, 1e2, 10.0, 1.0];
const N_SIGZ: usize = SIGZ.len();

/// Pass criterion (prediction): seven-figure rounding on both sides.
const TOL: f64 = 1e-5;

/// NJOY's flux tabulation grid for `iwt = 3`, `nsigz > 1` (`genflx`,
/// `groupr.f90:5623-5636`, `getwtf` `enext = 1.01*e`) — as in
/// `groupr_u238_gendf_golden.rs`.
fn njoy_flux_grid(sigt: &[(f64, f64)]) -> Vec<f64> {
    let etop = sigt.last().unwrap().0;
    let mut grid = Vec::with_capacity(sigt.len() * 2);
    let mut e = sigt[0].0;
    grid.push(e);
    let mut ip = 1;
    while e < etop {
        while ip < sigt.len() && sigt[ip].0 <= e {
            ip += 1;
        }
        let en = if ip < sigt.len() { sigt[ip].0 } else { etop };
        let enext = (1.01 * e).min(en);
        grid.push(enext);
        e = enext;
    }
    grid
}

#[test]
fn njoy_pendf_elastic_matrix_matches_gendf_mf6() {
    let Some(golden_path) = reference_file_or_skip("dtfr", GOLDEN, LABEL) else {
        return;
    };
    let Some(endf_path) = reference_endf_or_skip(ENDF, LABEL) else {
        return;
    };
    let Ok(pendf_path) = std::env::var(NJOY_PENDF_ENV) else {
        println!("[{LABEL}] SKIP: set {NJOY_PENDF_ENV} to the NJOY 293.6 K PENDF");
        return;
    };

    let golden_tape = Tape::read_file(&golden_path).expect("golden GENDF parses");
    let sec = golden_tape
        .section(MAT, 6, 2)
        .expect("GENDF MF=6/MT=2 present");
    let golden = GendfSection::from_rows(6, 2, &sec.rows).expect("MF=6 section decodes");
    assert_eq!(golden.nl, 1, "oracle NL");
    assert_eq!(golden.nz, N_SIGZ as i32, "oracle NZ");
    assert_eq!(golden.num_groups, N_GROUPS as i32, "oracle NGN");

    let pendf = Tape::read_file(std::path::Path::new(&pendf_path)).expect("NJOY PENDF parses");
    let total = read_pendf_cross_section(&pendf, MAT, 1).expect("PENDF MF=3/MT=1");
    let elastic = read_pendf_cross_section(&pendf, MAT, 2).expect("PENDF MF=3/MT=2");
    let PointwiseXs::LinLin(sigt_pairs) = &total.xs else {
        panic!("non-tabulated total")
    };
    let PointwiseXs::LinLin(el_pairs) = &elastic.xs else {
        panic!("non-tabulated elastic")
    };
    let dup = el_pairs.windows(2).filter(|w| w[0].0 == w[1].0).count();
    println!(
        "[{LABEL}] elastic grid {} points ({dup} duplicate energies), awr {}",
        el_pairs.len(),
        elastic.awr
    );

    let grid = njoy_flux_grid(sigt_pairs);
    let sig_t = PointwiseXs::LinLin(Arc::new((**sigt_pairs).clone()));
    let weight = GroupFlux::analytic(AnalyticWeight::OneOverE, TEMP_K);
    let flux_set = genflx_bondarenko_urr(&sig_t, None, &weight, 0.0, &SIGZ, &grid).unwrap();
    let fluxes: Vec<GroupFlux> = (0..N_SIGZ)
        .map(|iz| flux_set.flux(iz).unwrap().clone())
        .collect();

    let endf = Tape::read_file(&endf_path).expect("ENDF tape parses");
    let angular = File4Angular::from_tape(&endf, MAT, 2, NLD).expect("MF=4/MT=2");
    println!(
        "[{LABEL}] File 4: LCT {}, {} incident energies",
        angular.lct,
        angular.len()
    );
    let mut feed = TwoBodyFeed::new(angular, &BOUNDS, elastic.awr, 0.0, 0).unwrap();
    let header = MatrixHeader {
        mf: 6,
        mt: 2,
        za: 92238.0,
        zam: 0.0,
        lrflag: 0,
        temperature_k: TEMP_K,
        emaxx: 2.0e7,
    };
    let ours = two_body_matrix(&elastic.xs, &fluxes, &mut feed, 1, &header).unwrap();

    assert_eq!(
        ours.records.len(),
        golden.records.len(),
        "record count: ours {:?} vs NJOY {:?}",
        ours.records.iter().map(|r| r.ig).collect::<Vec<_>>(),
        golden.records.iter().map(|r| r.ig).collect::<Vec<_>>()
    );
    let mut worst = (0.0f64, 0i32, 0usize, 0.0f64, 0.0f64);
    let mut worst_flux = (0.0f64, 0i32, 0usize);
    let mut n_words = 0usize;
    for (a, b) in ours.records.iter().zip(&golden.records) {
        assert_eq!(a.ig, b.ig, "initial group");
        assert_eq!(
            (a.ig2lo, a.ng2),
            (b.ig2lo, b.ng2),
            "ig {}: (ig2lo, ng2) ours vs NJOY; ours {:?}",
            a.ig,
            a.data
        );
        assert_eq!(a.data.len(), b.data.len(), "ig {}: word count", a.ig);
        for (k, (&x, &y)) in a.data.iter().zip(&b.data).enumerate() {
            n_words += 1;
            assert_eq!(
                x == 0.0,
                y == 0.0,
                "ig {} word {k}: zero pattern {x} vs {y}",
                a.ig
            );
            if y != 0.0 {
                let r = ((x - y) / y).abs();
                if k < N_SIGZ {
                    if r > worst_flux.0 {
                        worst_flux = (r, a.ig, k);
                    }
                } else if r > worst.0 {
                    worst = (r, a.ig, k, x, y);
                }
            }
        }
    }
    println!(
        "[{LABEL}] RESULT: {} records, {n_words} words; worst transfer dev {:.3e} (ig {}, word {}: \
         ours {:.7e} vs NJOY {:.7e}); worst flux dev {:.3e} (ig {}, iz {})",
        ours.records.len(),
        worst.0,
        worst.1,
        worst.2,
        worst.3,
        worst.4,
        worst_flux.0,
        worst_flux.1,
        worst_flux.2
    );
    assert!(worst.0 < TOL, "transfer element deviates: {worst:?}");
    assert!(worst_flux.0 < TOL, "group flux deviates: {worst_flux:?}");
}
