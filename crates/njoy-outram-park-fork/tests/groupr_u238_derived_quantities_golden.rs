//! GROUPR **derived quantities** `MT=257/258/259` (average energy, lethargy,
//! reciprocal velocity) vs an NJOY2016 GENDF — the `getsig` analytic branch
//! (`groupr.f90:6758-6772`) through the ported `panel` vector reduction.
//!
//! # Oracle
//! `reference-data/gendf/u238-ENDF8.0-293.6K-29g-iwt3-6sigz-mt257-259.gendf`
//! (NJOY2016 `ac5adf5`, gfortran 13.3.0, 2026-09-10; README there): U-238 at
//! 293.6 K, 29 groups, `iwt = 3`, six sigma-zero values; GROUPR writes the
//! three quantities with `NL = NZ = 1`, weighting them with the infinite-
//! dilution (`iz = 1`) P0 component of the `genflx` flux table
//! (`getflx`, `groupr.f90:6498-6503`).
//!
//! # Methodology (tier 1 — engine isolation)
//! Pointwise input = NJOY's own 293.6 K PENDF (`OUTRAM_PARK_NJOY_U238_PENDF`)
//! for the total cross section that shapes the Bondarenko flux, tabulated on
//! NJOY's greedy grid as in `groupr_u238_gendf_golden.rs`. The quantity
//! itself is [`PointwiseXs::Derived`] (no tape data), whose `next_break` is
//! `getsig`'s `1.01 E`, so the panel march sub-divides every group on the
//! union of the 1 % ladder and the flux grid exactly as upstream, with the
//! integrand `sig*phi` linear across each sub-panel.
//!
//! **Prediction**, stated before running: every group average within
//! **1e-5** of the GENDF (seven-figure storage; the `rndoff`/`delta` end
//! shading upstream applies to each sub-panel is 1e-6 of a 1 % panel).
//!
//! **Result (2026-09-10):** recorded in the assertion message once measured.

use std::sync::Arc;

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::groupr::gendf::GendfSection;
use njoy_outram_park_fork::groupr::panel::{group_integral, GroupFlux, PointwiseXs};
use njoy_outram_park_fork::groupr::pendf_feed::read_pendf_cross_section;
use njoy_outram_park_fork::groupr::unresolved::genflx_bondarenko_urr;
use njoy_outram_park_fork::groupr::weights::AnalyticWeight;
use njoy_outram_park_fork::reference_data::reference_file_or_skip;

const LABEL: &str = "groupr-u238-derived";
const MAT: i32 = 9237;
const TEMP_K: f64 = 293.6;
const GOLDEN: &str = "u238-ENDF8.0-293.6K-29g-iwt3-6sigz-mt257-259.gendf";
const NJOY_PENDF_ENV: &str = "OUTRAM_PARK_NJOY_U238_PENDF";

const BOUNDS: [f64; 30] = [
    1e-5, 0.1, 0.625, 1.0, 4.0, 6.5, 8.0, 10.0, 15.0, 20.0, 25.0, 30.0, 40.0, 50.0, 100.0, 200.0,
    500.0, 1e3, 2e3, 5e3, 1e4, 2e4, 5e4, 1e5, 5e5, 1e6, 2e6, 5e6, 1e7, 2e7,
];
const N_GROUPS: usize = BOUNDS.len() - 1;
const SIGZ: [f64; 6] = [1e10, 1e4, 1e3, 1e2, 10.0, 1.0];
const TOL: f64 = 1e-5;

/// NJOY's `genflx` flux grid for `iwt = 3`, `nsigz > 1` (`groupr.f90:5623-5636`).
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
fn njoy_pendf_derived_quantities_match_gendf() {
    let Some(golden_path) = reference_file_or_skip("gendf", GOLDEN, LABEL) else {
        return;
    };
    let Ok(pendf_path) = std::env::var(NJOY_PENDF_ENV) else {
        println!("[{LABEL}] SKIP: set {NJOY_PENDF_ENV} to the NJOY 293.6 K PENDF");
        return;
    };
    let golden_tape = Tape::read_file(&golden_path).expect("golden GENDF parses");
    let pendf = Tape::read_file(std::path::Path::new(&pendf_path)).expect("NJOY PENDF parses");
    let total = read_pendf_cross_section(&pendf, MAT, 1).expect("PENDF MF=3/MT=1");
    let PointwiseXs::LinLin(sigt_pairs) = &total.xs else {
        panic!("non-tabulated total")
    };
    let grid = njoy_flux_grid(sigt_pairs);
    let sig_t = PointwiseXs::LinLin(Arc::new((**sigt_pairs).clone()));
    let weight = GroupFlux::analytic(AnalyticWeight::OneOverE, TEMP_K);
    let flux_set = genflx_bondarenko_urr(&sig_t, None, &weight, 0.0, &SIGZ, &grid).unwrap();
    let phi = flux_set.flux(0).unwrap();

    let mut worst = (0.0f64, 0i32, 0usize, 0.0f64, 0.0f64);
    let mut worst_flux = 0.0f64;
    for mt in [257, 258, 259] {
        let sec = golden_tape
            .section(MAT, 3, mt)
            .unwrap_or_else(|| panic!("GENDF MF=3/MT={mt} present"));
        let gold = GendfSection::from_rows(3, mt, &sec.rows).expect("section decodes");
        assert_eq!((gold.nl, gold.nz), (1, 1), "MT={mt}: NL, NZ");
        assert_eq!(gold.records.len(), N_GROUPS, "MT={mt}: every group written");
        let q = read_pendf_cross_section(&pendf, MAT, mt).expect("derived quantity");
        assert!(
            matches!(q.xs, PointwiseXs::Derived(_)),
            "MT={mt} is analytic"
        );
        for rec in &gold.records {
            let g = (rec.ig - 1) as usize;
            let integ = group_integral(&q.xs, phi, BOUNDS[g], BOUNDS[g + 1]);
            let ours = integ.average();
            let njoy = rec.data[1];
            let r = ((ours - njoy) / njoy).abs();
            if r > worst.0 {
                worst = (r, mt, g + 1, ours, njoy);
            }
            worst_flux = worst_flux.max(((integ.flux - rec.data[0]) / rec.data[0]).abs());
        }
    }
    println!(
        "[{LABEL}] RESULT: worst dev {:.3e} (MT={} group {}: ours {:.7e} vs NJOY {:.7e}); \
         worst flux dev {worst_flux:.3e}",
        worst.0, worst.1, worst.2, worst.3, worst.4
    );
    assert!(worst.0 < TOL, "derived quantity deviates: {worst:?}");
    assert!(worst_flux < TOL, "group flux deviates: {worst_flux:e}");
}
