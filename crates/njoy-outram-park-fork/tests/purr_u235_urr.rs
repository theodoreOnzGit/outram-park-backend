//! UNRESR + PURR against an NJOY2016 oracle on ENDF/B-VIII.0 U-235 (MAT 9228,
//! unresolved range 2.25–25 keV, `LRF=2`, `NRO=0`, `LSSF=1`).
//!
//! Fixture: `reference-data/endf/n-092_U_235-ENDF8.0.endf` (skips when absent).
//!
//! ## Oracle
//!
//! NJOY2016 (upstream commit `ac5adf5`, gfortran 13.3.0, built 2026-09-10)
//! run as `moder / reconr(0.001) / broadr(300 K) / unresr / purr` with
//! `temp = 300 K`, `sigz = 1e10 1e4 1e3 100 10 1`, `nbin = 20`, `nladr = 32`,
//! `iprint = 1`. Numbers below are transcribed from its listing:
//!
//! - UNRESR prints, per energy, the 5 rows `total / elastic / fission /
//!   capture / transport` × 6 σ₀ columns (4 significant figures).
//! - PURR prints, per energy, `spot`, `dbar`, the `infd` row (the analytic
//!   infinite-dilution reference from `unresx`: total, elastic *including*
//!   potential scattering, fission, capture), the `aver`/`pcsd` rows (mean
//!   and % standard deviation over the 32 ladders), and the Bondarenko
//!   cross sections by direct sampling.
//!
//! `LSSF=1`, so both modules zero the File-3 background (`rdunf3`,
//! `rdf3un`'s "sanity check for lssf=1") and no competition is open below
//! 7.5 keV: every number here is resonance + potential scattering only.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::purr::wfun::DopplerTable;
use njoy_outram_park_fork::purr::{infinite_dilution_reference, probability_table, Rng};
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use njoy_outram_park_fork::unresr::mf2::{parse_lru2_ranges, UnresolvedRange};
use njoy_outram_park_fork::unresr::wfun::WTable;
use njoy_outram_park_fork::unresr::unresolved_cross_sections;

const MAT: i32 = 9228;

fn ranges() -> Option<Vec<UnresolvedRange>> {
    let path = reference_endf_or_skip("n-092_U_235-ENDF8.0.endf", "purr-u235")?;
    let tape = Tape::read_file(&path).unwrap();
    let sec = tape.section(MAT, 2, 151).expect("MF=2/MT=151");
    // `parse_lru2_ranges` starts at the per-isotope CONT (rdunf2:439), i.e.
    // after the section HEAD record.
    let ranges = parse_lru2_ranges(&sec.rows[1..]).unwrap();
    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0].el, 2250.0);
    assert_eq!(ranges[0].eh, 25000.0);
    assert_eq!(ranges[0].lssf, 1);
    Some(ranges)
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// NJOY UNRESR at 300 K: `[total, elastic, fission, capture]` for σ₀ = 1e10
/// and σ₀ = 1 (first and last columns of the printed block).
const UNRESR_ORACLE: [(f64, [f64; 4], [f64; 4]); 3] = [
    (2.25e3, [19.78, 12.11, 5.636, 2.036], [19.16, 12.01, 5.270, 1.884]),
    (5.5e3, [16.97, 11.92, 3.725, 1.327], [16.92, 11.89, 3.710, 1.320]),
    (1.0e4, [15.69, 11.73, 2.921, 1.032], [15.70, 11.73, 2.935, 1.037]),
];

/// NJOY PURR's `unresx` reference at the same energies:
/// `(E, spot, dbar, [total, elastic(+spot), fission, capture])`. `spot` and
/// `dbar` are the listing's 5 figures; the four cross sections are the
/// 7-figure values PURR wrote to the PENDF `MF=2/MT=152` σ₀ = 10¹⁰ column
/// (which is its `infd` row renormalised to itself, i.e. exactly `unresx`).
/// NJOY's first grid energy is `sigfig(EL,7,+1)` = 2250.001 eV, a 5e-7
/// relative shift that is invisible at this precision.
const UNRESX_ORACLE: [(f64, f64, f64, [f64; 4]); 3] = [
    (2.25e3, 11.700, 0.16137, [19.77818, 12.10549, 5.636382, 2.036314]),
    (5.5e3, 11.646, 0.16032, [16.96934, 11.91663, 3.725428, 1.327288]),
    (1.0e4, 0.0, 0.0, [15.68594, 11.73279, 2.920641, 1.032492]),
];

#[test]
fn unresr_and_infinite_dilution_reference_match_njoy() {
    let Some(ranges) = ranges() else { return };
    let wtable = WTable::new();

    // Print everything first so a failure still shows the whole picture.
    let mut failures: Vec<String> = Vec::new();
    let mut check = |ok: bool, msg: String| {
        if !ok {
            failures.push(msg);
        }
    };

    // Step 4 of src/purr/README.md, against the oracle rather than only
    // against each other.
    for &(e, spot, dbar, infd) in &UNRESX_ORACLE {
        let inf = infinite_dilution_reference(&ranges, e).unwrap();
        let el = inf.sigma_elastic_inf + inf.potential_scattering;
        let tot = el + inf.sigma_fission_inf + inf.sigma_capture_inf;
        println!(
            "unresx E={e}: spot {:.4} (NJOY {spot}) dbar {:.5} (NJOY {dbar}) tot {tot:.4} el {el:.4} fis {:.4} cap {:.4} (NJOY {infd:?})",
            inf.potential_scattering,
            1.0 / inf.mean_inverse_spacing,
            inf.sigma_fission_inf,
            inf.sigma_capture_inf
        );
        for s in &inf.sequences {
            println!("   seq: D {:.5} gn {:.4e} gf {:.4e} gg {:.4e} gx {:.4e} ndf n/f/x {}/{}/{} csz {:.4e}",
                s.dbar, s.gn_mean, s.gf_mean, s.gg_mean, s.gx_mean, s.ndf_n, s.ndf_f, s.ndf_x, s.csz);
        }
        if spot > 0.0 {
            check(rel(inf.potential_scattering, spot) < 1e-4, format!("spot at {e}"));
            check(rel(1.0 / inf.mean_inverse_spacing, dbar) < 1e-4, format!("dbar at {e}"));
        }
        // 7-figure oracle; 2e-5 leaves room for the 2250.001 grid shift and
        // summation order.
        check(rel(tot, infd[0]) < 2e-5, format!("infd total at {e}: {tot} vs {}", infd[0]));
        check(rel(el, infd[1]) < 2e-5, format!("infd elastic at {e}: {el} vs {}", infd[1]));
        check(rel(inf.sigma_fission_inf, infd[2]) < 2e-5, format!("infd fission at {e}: {} vs {}", inf.sigma_fission_inf, infd[2]));
        check(rel(inf.sigma_capture_inf, infd[3]) < 2e-5, format!("infd capture at {e}: {} vs {}", inf.sigma_capture_inf, infd[3]));
    }

    for &(e, dilute, shielded) in &UNRESR_ORACLE {
        let rows = unresolved_cross_sections(&ranges, e, 300.0, &[1e10, 1.0], [0.0; 4], &wtable)
            .unwrap();
        println!("unresr E={e}: sig0=1e10 {:?} (NJOY {dilute:?})", rows[0]);
        println!("             sig0=1    {:?} (NJOY {shielded:?})", rows[1]);
        for k in 0..4 {
            // 4 printed significant figures → ±5e-4 relative rounding.
            check(rel(rows[0][k], dilute[k]) < 1.5e-3, format!("E={e} sig0=1e10 col {k}: {} vs NJOY {}", rows[0][k], dilute[k]));
            check(rel(rows[1][k], shielded[k]) < 1.5e-3, format!("E={e} sig0=1 col {k}: {} vs NJOY {}", rows[1][k], shielded[k]));
        }
        // UNRESR at σ₀ → ∞ and PURR's analytic reference are the same
        // width-fluctuation theory; they must agree with each other too.
        let inf = infinite_dilution_reference(&ranges, e).unwrap();
        check(rel(rows[0][3], inf.sigma_capture_inf) < 2e-3, format!("unresr vs unresx capture at {e}: {} vs {}", rows[0][3], inf.sigma_capture_inf));
        check(rel(rows[0][2], inf.sigma_fission_inf) < 2e-3, format!("unresr vs unresx fission at {e}: {} vs {}", rows[0][2], inf.sigma_fission_inf));
        check(rel(rows[0][1], inf.sigma_elastic_inf + inf.potential_scattering) < 2e-3, format!("unresr vs unresx elastic at {e}"));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn purr_probability_table_at_first_energy_matches_njoy() {
    let Some(ranges) = ranges() else { return };
    let dtable = DopplerTable::new();
    // NJOY's driver seeds `kk=-101` once and processes energies in order, so
    // its first energy (2.25 keV) is the one whose random stream a fresh
    // `Rng::new(-101)` reproduces.
    let mut rng = Rng::new(-101);
    let e = 2.25e3;
    let inf = infinite_dilution_reference(&ranges, e).unwrap();
    let sig0 = [1e10, 1e4, 1e3, 100.0, 10.0, 1.0];
    let r = probability_table(&inf.sequences, &inf, [0.0; 4], &sig0, &[300.0], 20, 32, 10_000, &mut rng, &dtable)
        .unwrap();
    let c = &r.convergence;
    println!(
        "aver {:.4} {:.4} {:.4} {:.4}   pcsd {:.2} {:.2} {:.2} {:.2}",
        c.mean_total, c.mean_elastic, c.mean_fission, c.mean_capture,
        c.pct_std_total, c.pct_std_elastic, c.pct_std_fission, c.pct_std_capture
    );
    let t = &r.tables[0];
    println!("bondarenko: {:?}", t.bondarenko);
    println!("direct sampling: {:?}", t.bondarenko_direct_sampling);
    println!("bin edges: {:?}", t.bin_upper_edge);
    println!("bin prob: {:?}", t.bin_probability);

    // NJOY (32 ladders, same seed): aver 19.842 / 12.101 / 5.6676 / 2.0738,
    // pcsd 1.65 / 0.37 / 3.84 / 4.91; direct-sampling Bondarenko at
    // σ₀ = 1: 18.963 / 11.985 / 5.1204 / 1.8573.
    let njoy_aver = [19.842, 12.101, 5.6676, 2.0738];
    let njoy_ds_inf = [19.842, 12.101, 5.6676, 2.0738];
    let njoy_ds_1 = [18.963, 11.985, 5.1204, 1.8573];

    // Structural invariants first.
    assert_eq!(t.bin_probability.len(), 20);
    let psum: f64 = t.bin_probability.iter().sum();
    assert!((psum - 1.0).abs() < 1e-9, "probabilities sum to {psum}");
    assert!(t.bin_upper_edge.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(*t.bin_upper_edge.last().unwrap(), 1.0e6);
    // Probability-weighted table means reproduce the (renormalised)
    // infinite-dilution values.
    for k in 0..4 {
        let mean: f64 = t.bin_probability.iter().zip(&t.bin_xs).map(|(p, x)| p * x[k]).sum();
        assert!(rel(mean, t.bondarenko[0][k]) < 1e-6, "col {k}: table mean {mean} vs {}", t.bondarenko[0][k]);
    }
    // Self-shielding must be monotone in σ₀ for total, fission, capture.
    for k in [0, 2, 3] {
        for w in t.bondarenko.windows(2) {
            assert!(w[1][k] <= w[0][k] * (1.0 + 1e-12), "col {k} not monotone: {:?}", t.bondarenko);
        }
    }

    // Statistical agreement with the oracle: the Monte Carlo mean over 32
    // ladders carries pcsd/√32 ≈ 0.3 % (total) … 0.9 % (capture) of its
    // own noise, and the port's random stream is only identical to NJOY's if
    // every `rann` call happens in the same order — so 3 % here, and the
    // printed values above tell the tighter story.
    let ours = [c.mean_total, c.mean_elastic, c.mean_fission, c.mean_capture];
    for k in 0..4 {
        assert!(rel(ours[k], njoy_aver[k]) < 3e-2, "aver col {k}: {} vs NJOY {}", ours[k], njoy_aver[k]);
        assert!(rel(t.bondarenko_direct_sampling[0][k], njoy_ds_inf[k]) < 3e-2, "ds inf col {k}");
        assert!(rel(t.bondarenko_direct_sampling[5][k], njoy_ds_1[k]) < 3e-2, "ds sig0=1 col {k}");
    }
    // The self-shielding *ratio* at σ₀ = 1 b is far less noisy than the
    // absolute values: NJOY 0.9557 (total), 0.9904 (elastic), 0.9034
    // (fission), 0.8956 (capture).
    let ratio = |k: usize| t.bondarenko[5][k] / t.bondarenko[0][k];
    let njoy_ratio = [18.963 / 19.842, 11.985 / 12.101, 5.1204 / 5.6676, 1.8573 / 2.0738];
    for k in 0..4 {
        println!("self-shielding ratio col {k}: {:.4} vs NJOY {:.4}", ratio(k), njoy_ratio[k]);
        assert!((ratio(k) - njoy_ratio[k]).abs() < 0.02, "ratio col {k}");
    }
}
