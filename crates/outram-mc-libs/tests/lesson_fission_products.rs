//! Reaches every function of the Fission products lessons' data readers
//! (`examples/lesson_fission_products/{data,spectrum}.rs`), and checks each
//! against something it can fail on: an exact identity, an analytic case, or
//! the evaluation's own numbers. The generated page data are checked
//! separately by `docs/site/science/fission-products/check-fission.js`.
//!
//! Tests that need an ENDF/B-VIII.0 tape from `reference-data/endf/` or the
//! fission-yields-data source directory skip, with a note, when it is absent.
//!
//! Results 2026-10-20 (all pass): U-235 MF=1/MT=458 EFR at thermal
//! 169.13 MeV (the tape's value, ENDF lines ~1162-1164); MF=5/MT=18 is one
//! LF=1 law; the sampler's histogram of 200 000 draws at 2 MeV against the
//! exact law: chi-square per degree of freedom 1.006 at 2 MeV (a tabulated
//! energy) and 1.053 at 2.5 MeV (between two tables), 41 bins each; means
//! 2.0544 MeV exact against 2.0530 sampled, and 2.0688 against 2.0673.
//! U-235's peak-to-valley ratios 635.8, 218.4 and 5.34 at 0.0253 eV,
//! 500 keV and 14 MeV.

use petir::mathf::RealMath;
#[path = "../examples/lesson_fission_products/data.rs"]
mod data;
#[path = "../examples/lesson_fission_products/spectrum.rs"]
mod spectrum;

use data::*;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::nuclear_data::secondary::{ChiEout, ChiTabular, FissionSpectrum};
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;

fn u235() -> Option<Tape> {
    let p = reference_endf_or_skip("n-092_U_235-ENDF8.0.endf", "lesson_fission_products")?;
    Some(Tape::read(std::fs::File::open(p).ok()?).ok()?)
}

/// A two-table χ built by hand: a flat density on [0, 2] at E = 0 and a
/// triangle on [0, 4] (peak at 0) at E = 10.
fn toy_chi() -> ChiTabular {
    let flat = ChiEout { e_out: vec![0.0, 2.0], pdf: vec![0.5, 0.5], cdf: vec![0.0, 1.0], linlin: true, n_discrete: 0 };
    let tri = ChiEout { e_out: vec![0.0, 4.0], pdf: vec![0.5, 0.0], cdf: vec![0.0, 1.0], linlin: true, n_discrete: 0 };
    ChiTabular { incident: vec![0.0, 10.0], tables: vec![flat, tri], incident_interp: vec![] }
}

#[test]
fn small_helpers_are_exact_on_hand_cases() {
    assert_eq!(linlin(&[(0.0, 0.0), (2.0, 4.0)], 1.0), 2.0);
    assert_eq!(linlin(&[(0.0, 1.0), (2.0, 4.0)], -1.0), 1.0, "clamped below");
    assert_eq!(linlin(&[(0.0, 1.0), (2.0, 4.0)], 9.0), 4.0, "clamped above");
    // Uniform on [0, 2]: mean 1. Triangle 1 - x/2 on [0, 2]: mean 2/3.
    assert!((mean_energy(&[0.0, 2.0], &[1.0, 1.0], true) - 1.0).abs() < 1e-15);
    assert!((mean_energy(&[0.0, 2.0], &[1.0, 0.0], true) - 2.0 / 3.0).abs() < 1e-15);
    assert!((mean_energy(&[0.0, 1.0, 2.0], &[1.0, 3.0, 0.0], false) - (0.5 + 3.0 * 1.5) / 4.0).abs() < 1e-15);
    let (x, y) = thin(&[0.0, 1.0, 2.0, 3.0, 4.0], &[5.0, 6.0, 7.0, 8.0, 9.0], 3);
    assert_eq!(x, vec![0.0, 3.0, 4.0], "every third point and the last");
    assert_eq!(y, vec![5.0, 8.0, 9.0]);
    // ENDF-102 channel radius for AWRI = 1000: 0.123 * 10 + 0.08 = 1.31 [1e-12 cm] = 13.1 fm.
    assert!((channel_radius_fm(1000.0) - 13.1).abs() < 1e-12);
    assert_eq!(unit_seconds("ms"), Some(1e-3));
    assert_eq!(unit_seconds("y"), Some(365.25 * 86400.0));
    assert_eq!(unit_seconds("MeV"), None, "a width is not a half-life");
}

#[test]
fn nudat_tables_parse_ground_states() {
    let masses = mass_excess_table("name,z,n,massExcess(keV)\nU235,92,143,40900\nX,92,143,41000\nBad,line\n");
    assert_eq!(masses.get(&(92, 235)), Some(&40900.0), "the lower (ground-state) row is kept");
    let decays = decay_table(
        "z,n,name,levelEnergy(MeV),decayMode,branchingRatio,u,m,mu,halflife,halflifeUnit,hu\n\
         35,52,87Br,0,B-,100,,,,55.65,s,0.13\n35,52,87Br,0,B-N,2.53,,,,55.65,s,0.13\n35,52,87Br,0.5,IT,100,,,,1,ms,0\n",
    );
    let br = decays.get(&(35, 87)).expect("Br-87");
    assert_eq!(br.half_life_s, Some(55.65));
    assert_eq!((br.beta_minus, br.beta_n), (100.0, 2.53));
    assert_eq!(br.modes.len(), 2, "the isomer row is not the ground state");
}

#[test]
fn yields_are_complete_and_peaks_fall_with_energy() {
    let products = product_nuclides();
    assert!(products.len() > 1000, "{} product names", products.len());
    let (_, _, a, nuc, _) = parents()[0];
    let mut ratios = Vec::new();
    for e in tabulated_energies(nuc) {
        let y = independent_yields(nuc, e, &products);
        assert!((yield_sum(&y) - 2.0).abs() < 1e-4, "two products per fission at {e} eV");
        let m = mass_yields(&y);
        assert!((m.values().sum::<f64>() - 2.0).abs() < 1e-4);
        let pv = peak_valley(&m, (a + 1) as f64 / 2.0).expect("two peaks");
        assert!(pv.light_peak.0 < pv.valley.0 && pv.valley.0 < pv.heavy_peak.0);
        ratios.push(pv.ratio);
    }
    println!("U-235 peak-to-valley at the tabulated energies: {ratios:?}");
    assert!(ratios.windows(2).all(|w| w[1] < w[0]), "the humps flatten as the energy rises");
}

#[test]
fn the_crate_tables_ship_beside_its_source() {
    match fission_yields_data_src() {
        Ok(dir) => assert!(dir.join("half_life_branching_ratio_nndc_nudat_data_export.csv").is_file()),
        Err(e) => println!("SKIP: {e}"),
    }
}

#[test]
fn u235_tape_readers_give_the_evaluations_numbers() {
    let Some(tape) = u235() else { return };
    let mat = tape.materials()[0];
    let er = read_mt458(&tape, mat).expect("MT=458");
    assert_eq!(er.lfc, 1);
    assert!((er.thermal[0].0 - 169.13e6).abs() < 1.0, "EFR {}", er.thermal[0].0);
    assert!((er.at(1, 0.0253) - 169.13e6).abs() < 10.0, "EFR(E) at thermal");
    assert!(er.at(1, 1.4e7) < er.at(1, 0.0253), "EFR falls with incident energy");
    assert_eq!(er.at(5, 1.4e7), er.thermal[4].0, "EB is not tabulated: the thermal point");
    let sn = capture_q(&tape, mat).expect("MT=102");
    assert!((sn - 6.5444e6).abs() < 100.0, "S_n {sn}");
    let res = resonances(&tape, mat).expect("MF=2");
    assert!(res.list.len() > 1000 && res.el < 1.0 && res.eh > 1000.0);
    assert!(res.list.windows(2).all(|w| w[0].0 <= w[1].0), "sorted by energy");
}

#[test]
fn spectrum_law_is_exact_on_a_hand_built_table() {
    let chi = toy_chi();
    assert_eq!(spectrum::bracket(&chi, -1.0), (0, 0.0));
    assert_eq!(spectrum::bracket(&chi, 5.0), (0, 0.5));
    assert_eq!(spectrum::bracket(&chi, 20.0), (0, 1.0));
    let flat = &chi.tables[0];
    assert!((spectrum::table_cdf(flat, 1.0) - 0.5).abs() < 1e-15);
    assert!((spectrum::table_pdf(&chi.tables[1], 2.0) - 0.25).abs() < 1e-15);
    assert!((spectrum::table_mean(&chi.tables[1]) - 4.0 / 3.0).abs() < 1e-12);
    // At E = 5: range [0, 3]; half flat (mean 1 -> 1.5 after mapping), half
    // triangle (mean 4/3 -> 1). Mean 1.25.
    let comps = spectrum::components(&chi, 5.0);
    assert_eq!(comps.len(), 2);
    assert!((spectrum::law_mean(&chi, 5.0) - 1.25).abs() < 1e-12);
    assert!((spectrum::law_cdf(&chi, 5.0, 3.0) - 1.0).abs() < 1e-12);
    // The density integrates to the CDF.
    let n = 3000;
    let integral: f64 = (0..n).map(|k| spectrum::law_pdf(&chi, 5.0, (k as f64 + 0.5) * 1.5 / n as f64) * 1.5 / n as f64).sum();
    assert!((integral - spectrum::law_cdf(&chi, 5.0, 1.5)).abs() < 1e-6);
    assert!(spectrum::law_mode(&chi, 5.0) < 1e-6, "the triangle's peak at 0 wins");
    let (p, over) = spectrum::bin_probabilities(&chi, 5.0, 0.5, 4);
    assert!((p.iter().sum::<f64>() + over - 1.0).abs() < 1e-12);
    assert!((over - (1.0 - spectrum::law_cdf(&chi, 5.0, 2.0))).abs() < 1e-12, "the bins end at 2, the range at 3");
}

#[test]
fn sampler_reproduces_the_exact_law_on_u235() {
    let Some(tape) = u235() else { return };
    let mat = tape.materials()[0];
    let spec = FissionSpectrum::from_endf_mf5(&tape, mat).unwrap().unwrap();
    let FissionSpectrum::ContinuousTabular(chi) = &spec else { panic!("U-235 MF=5/MT=18 is LF=1") };
    // 2 MeV is a tabulated energy; 2.5 MeV lies between two tables.
    for e in [2.0e6, 2.5e6] {
        let (width, nbins, n) = (2.5e5, 40, 200_000);
        let (counts, over, sum) = spectrum::sample_histogram(&spec, e, n, 7_000_000, width, nbins);
        let (p, pover) = spectrum::bin_probabilities(chi, e, width, nbins);
        let (mut chi2, mut dof) = (0.0, 0);
        for (c, q) in counts.iter().zip(&p).chain(std::iter::once((&over, &pover))) {
            let m = q * n as f64;
            if m > 20.0 {
                chi2 += (*c as f64 - m).r_powi(2) / m;
                dof += 1;
            }
        }
        let mean = spectrum::law_mean(chi, e);
        let sampled = sum / n as f64;
        println!("U-235 at {e:e} eV: chi2/dof {:.3} over {dof} bins; mean exact {mean:.1} eV sampled {sampled:.1} eV; most probable {:.0} eV", chi2 / dof as f64, spectrum::law_mode(chi, e));
        // 5 sigma on chi2 with ~dof degrees of freedom: dof + 5 sqrt(2 dof).
        assert!(chi2 < dof as f64 + 5.0 * (2.0 * dof as f64).sqrt(), "histogram disagrees with the exact law");
        assert!((sampled - mean).abs() < 5.0 * 1.5e6 / (n as f64).sqrt(), "mean");
    }
}
