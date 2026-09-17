//! GROUPR **elastic transfer matrix** (MF=6/MT=2) vs NJOY2016 GENDF tapes —
//! the `getdis`/`panel`/`displa` matrix path end to end, at `lord = 0` and
//! `lord = 3`.
//!
//! # Oracles
//! - `reference-data/dtfr/u238-ENDF8.0-293.6K-29g-iwt3-6sigz-mf6.gendf`
//!   (NJOY2016 `ac5adf5`, gfortran 13.3.0, 2026-09-10; see that directory's
//!   README): U-238 (MAT 9237) at 293.6 K, 29 groups, `iwt = 3` (1/E), six
//!   sigma-zero values, `lord = 0`, so the section carries `NL = 1`, `NZ = 6`
//!   and, per initial group, the group flux (slot 1) and the P0 transfer
//!   elements to `ng2 - 1` secondary groups from `ig2lo`.
//! - `reference-data/gendf/u238-ENDF8.0-293.6K-29g-iwt3-6sigz-lord3-mf6.gendf`
//!   (same build, same PENDF, GROUPR only, `lord = 3`): `NL = 4`, `NZ = 6`,
//!   so each record carries the four Legendre flux components
//!   `wtf*fac^(il+1)` and the P0–P3 transfer elements.
//!
//! # Methodology (tier 1 — engine isolation)
//! Pointwise input = NJOY's own 293.6 K PENDF (`OUTRAM_PARK_NJOY_U238_PENDF`,
//! the same tape the oracles' GROUPR read), so any difference is the group
//! averaging itself, not RECONR/BROADR. The Bondarenko flux is tabulated on
//! NJOY's greedy `genflx` grid exactly as in `groupr_u238_gendf_golden.rs`
//! (that test pins the group fluxes to 1e-5), with the `fac^(il+1)` Legendre
//! components for the `lord = 3` case; the File-4 MT=2 distribution
//! (`LTT = 3`, CM) comes from the committed ENDF tape; the feed is `getdis`
//! and the quadrature `panel` (`groupr::two_body`, `groupr::matrix_panel`).
//!
//! **Prediction**, stated before running: identical record structure (same
//! initial groups written, same `ig2lo`/`ng2` per group), and every word —
//! flux and transfer element — within **1e-5** relative of the GENDF. Both
//! sides round to seven significant figures, so the residual should be
//! O(1e-6); a structural mismatch or a >1e-4 element would mean a real
//! port defect, not rounding. The feed function itself is rounded to
//! seven decimals at every energy (`getdis:9573-9580`), so an element of
//! order `1e-6 sigma_g` or smaller can legitimately differ by a rounding
//! flip: such elements must lie within two units of `1e-7 sigma_g`.
//!
//! **Result (2026-09-10), `lord = 0`:** all 29 initial groups written on
//! both sides with identical `(ig2lo, ng2)`; 516 words; every transfer
//! element within 3.42e-7 (group 29: 2.9244590 vs 2.9244600), every group
//! flux within 1.44e-7 — the seven-figure floor. The elastic PENDF grid
//! has no duplicate energies, so `gety1`'s discontinuity path is not
//! exercised by this oracle.
//!
//! **Result (2026-09-10), `lord = 3`:** 29 records, 2064 words, identical
//! structure; P0/P1 and the large P2/P3 elements agree to the seven-figure
//! floor; the elements outside 1e-5 relative are all P2/P3 elements below
//! `1e-5 sigma_g` (105 of 2064 words) and lie within 0.02 units of
//! `1e-7 sigma_g` (worst: group 9, a P3 element, -7.866e-7 vs -7.760e-7);
//! the rest agree to 9.99e-6 or better, fluxes to 6.89e-7.
//!
//! A port defect was found by this oracle before either result above was
//! reached: `getfle` writes the coefficient count back into `getdis`'s
//! `nld`, which sets the Gauss order (`npo`); with `nld` left at 21 the
//! group-1 P2 feed used the seven-digit 20-point table and came out one
//! 1e-7 unit (2.5 %) below NJOY's 8-point value.

use std::sync::Arc;

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::groupr::file4::{File4Angular, NLD};
use njoy_outram_park_fork::groupr::gendf::GendfSection;
use njoy_outram_park_fork::groupr::matrix_panel::{two_body_matrix, FluxComponents, MatrixHeader};
use njoy_outram_park_fork::groupr::panel::{GroupFlux, PointwiseXs};
use njoy_outram_park_fork::groupr::pendf_feed::read_pendf_cross_section;
use njoy_outram_park_fork::groupr::two_body::TwoBodyFeed;
use njoy_outram_park_fork::groupr::unresolved::{genflx_bondarenko_components, genflx_bondarenko_urr};
use njoy_outram_park_fork::groupr::weights::AnalyticWeight;
use njoy_outram_park_fork::reference_data::{reference_endf_or_skip, reference_file_or_skip};

const MAT: i32 = 9237;
const TEMP_K: f64 = 293.6;
const GOLDEN: &str = "u238-ENDF8.0-293.6K-29g-iwt3-6sigz-mf6.gendf";
const GOLDEN_LORD3: &str = "u238-ENDF8.0-293.6K-29g-iwt3-6sigz-lord3-mf6.gendf";
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
/// Elements outside `TOL` must sit within this many units of `1e-7*sigma_g`
/// — the feed function's own seven-decimal rounding (`getdis:9573-9580`).
const FLOOR_UNITS: f64 = 2.0;

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

/// Everything the two tiers share: the inputs, the run, and the word-by-word
/// comparison. `nl` selects `lord + 1`; the flux components are the
/// `genflx` Legendre components (`nl = 1` reduces to the plain Bondarenko
/// flux). Returns `(worst transfer, worst flux)` relative deviations.
fn run_and_compare(subdir: &str, golden: &str, nl: usize, label: &str) -> Option<(f64, f64)> {
    let golden_path = reference_file_or_skip(subdir, golden, label)?;
    let endf_path = reference_endf_or_skip(ENDF, label)?;
    let Ok(pendf_path) = std::env::var(NJOY_PENDF_ENV) else {
        println!("[{label}] SKIP: set {NJOY_PENDF_ENV} to the NJOY 293.6 K PENDF");
        return None;
    };

    let golden_tape = Tape::read_file(&golden_path).expect("golden GENDF parses");
    let sec = golden_tape
        .section(MAT, 6, 2)
        .expect("GENDF MF=6/MT=2 present");
    let golden = GendfSection::from_rows(6, 2, &sec.rows).expect("MF=6 section decodes");
    assert_eq!(golden.nl, nl as i32, "oracle NL");
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
        "[{label}] elastic grid {} points ({dup} duplicate energies), awr {}",
        el_pairs.len(),
        elastic.awr
    );

    let grid = njoy_flux_grid(sigt_pairs);
    let sig_t = PointwiseXs::LinLin(Arc::new((**sigt_pairs).clone()));
    let weight = GroupFlux::analytic(AnalyticWeight::OneOverE, TEMP_K);
    let fluxes = if nl == 1 {
        let flux_set = genflx_bondarenko_urr(&sig_t, None, &weight, 0.0, &SIGZ, &grid).unwrap();
        FluxComponents::p0(
            (0..N_SIGZ)
                .map(|iz| flux_set.flux(iz).unwrap().clone())
                .collect(),
        )
    } else {
        FluxComponents {
            per_dilution: genflx_bondarenko_components(
                &sig_t, None, &weight, 0.0, &SIGZ, &grid, nl,
            )
            .unwrap(),
        }
    };

    let endf = Tape::read_file(&endf_path).expect("ENDF tape parses");
    let angular = File4Angular::from_tape(&endf, MAT, 2, NLD).expect("MF=4/MT=2");
    println!(
        "[{label}] File 4: LCT {}, {} incident energies",
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
    let ours = two_body_matrix(&elastic.xs, &fluxes, &mut feed, nl, &header).unwrap();

    assert_eq!(
        ours.records.len(),
        golden.records.len(),
        "record count: ours {:?} vs NJOY {:?}",
        ours.records.iter().map(|r| r.ig).collect::<Vec<_>>(),
        golden.records.iter().map(|r| r.ig).collect::<Vec<_>>()
    );
    let mut worst = (0.0f64, 0i32, 0usize, 0.0f64, 0.0f64);
    let mut worst_flux = (0.0f64, 0i32, 0usize);
    let mut worst_units = (0.0f64, 0i32, 0usize, 0.0f64, 0.0f64);
    let mut n_words = 0usize;
    let mut n_floor = 0usize;
    let nlz = nl * N_SIGZ;
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
        // sigma_g per dilution: the P0 row sum (all secondary slots).
        let sig_g: Vec<f64> = (0..N_SIGZ)
            .map(|iz| {
                (1..b.ng2 as usize)
                    .map(|it| b.data[it * nlz + iz * nl])
                    .sum::<f64>()
            })
            .collect();
        for (k, (&x, &y)) in a.data.iter().zip(&b.data).enumerate() {
            n_words += 1;
            if k < nlz {
                // Flux components: never zero.
                let r = ((x - y) / y).abs();
                if r > worst_flux.0 {
                    worst_flux = (r, a.ig, k);
                }
                continue;
            }
            assert_eq!(
                x == 0.0,
                y == 0.0,
                "ig {} word {k}: zero pattern {x} vs {y}",
                a.ig
            );
            if y == 0.0 {
                continue;
            }
            let r = ((x - y) / y).abs();
            if r < TOL {
                if r > worst.0 {
                    worst = (r, a.ig, k, x, y);
                }
                continue;
            }
            // Outside the seven-figure floor: the feed function is rounded
            // to 1e-7 at every energy (`getdis:9573-9580`), so an element
            // may still differ by a unit or two of 1e-7 * sigma_g from a
            // rounding flip; nothing else is tolerated.
            let iz = (k % nlz) / nl;
            let units = (x - y).abs() / (1.0e-7 * sig_g[iz]);
            n_floor += 1;
            if units > worst_units.0 {
                worst_units = (units, a.ig, k, x, y);
            }
            assert!(
                units <= FLOOR_UNITS,
                "ig {} word {k}: {x:e} vs NJOY {y:e} ({r:.2e} rel) is {units:.2} units of \
                 1e-7*sigma_g (sigma_g = {})",
                a.ig,
                sig_g[iz]
            );
        }
    }
    println!(
        "[{label}] RESULT: {} records, {n_words} words; {n_floor} outside {TOL:e} rel, worst \
         {:.3} units of 1e-7*sigma_g (ig {}, word {}: ours {:.7e} vs NJOY {:.7e}); worst rel \
         dev among the rest {:.3e} (ig {}, word {}: ours {:.7e} vs NJOY {:.7e}); worst flux \
         dev {:.3e} (ig {}, word {})",
        ours.records.len(),
        worst_units.0,
        worst_units.1,
        worst_units.2,
        worst_units.3,
        worst_units.4,
        worst.0,
        worst.1,
        worst.2,
        worst.3,
        worst.4,
        worst_flux.0,
        worst_flux.1,
        worst_flux.2
    );
    Some((worst.0, worst_flux.0))
}

/// `lord = 0`: see the module doc for the prediction and the result.
#[test]
fn njoy_pendf_elastic_matrix_matches_gendf_mf6() {
    let Some((worst, worst_flux)) =
        run_and_compare("dtfr", GOLDEN, 1, "groupr-u238-elastic-matrix")
    else {
        return;
    };
    assert!(worst < TOL, "transfer element deviates: {worst:e}");
    assert!(worst_flux < TOL, "group flux deviates: {worst_flux:e}");
}

/// `lord = 3` (`NL = 4`, `NZ = 6`): the Legendre projection of the lab
/// cosine in `getdis` (`:9550-9558`), the `fac^(il+1)` flux components and
/// `displa`'s `nz > 1` branch over four orders. Prediction as in the
/// module doc (1e-5).
#[test]
fn njoy_pendf_p3_elastic_matrix_matches_gendf_mf6_lord3() {
    let Some((worst, worst_flux)) =
        run_and_compare("gendf", GOLDEN_LORD3, 4, "groupr-u238-elastic-matrix-lord3")
    else {
        return;
    };
    assert!(worst < TOL, "transfer element deviates: {worst:e}");
    assert!(worst_flux < TOL, "group flux deviates: {worst_flux:e}");
}
