//! UNRESR + PURR against an NJOY2016 oracle on ENDF/B-VIII.0 U-238 (MAT 9237,
//! unresolved range 20–149.0087 keV, `LRF=2`, **`NRO=1`**, `NAPS=0`,
//! `LSSF=1`) — the energy-dependent-scattering-radius path (`op-as32`).
//!
//! Fixture: `reference-data/endf/n-092_U_238.endf` (skips when absent).
//!
//! ## Why this evaluation matters
//!
//! U-238's URR range carries an 83-point `INT=5` (log-log) `TAB1` of
//! `AP(E)` (0.9454 → 0.9136 ×10⁻¹² cm) *and* writes `AP=0.0` in the
//! `SPI/AP/LSSF` header. Without the table the phase shift `ρ_c = k·AP` is
//! zero, potential scattering vanishes, and the whole 20–149 keV band is
//! wrong; upstream reads the table right after the range CONT (`rdunf2`,
//! `unresr.f90:515-528`) and interpolates it at every working energy with
//! `terpa` (`unresl:966-971`). `NAPS=0`, so the channel radius stays
//! mass-derived and `AP(E)` enters through the phase shift alone.
//!
//! ## Oracle
//!
//! NJOY2016 (upstream 2016.79, gfortran 13.3.0, built in-session) run as
//! `moder / reconr(0.001) / broadr(300 K) / unresr / purr` with `temp = 300 K`,
//! `sigz = 1e10 1e4 1e3 100 10 1`, `nbin = 20`, `nladr = 32`, `iprint = 1`
//! (deck transcribed in the `op-as32` bead). Numbers below are from its
//! listing (4 printed figures) and from the `MF=2/MT=152` section PURR wrote
//! to the PENDF (7 figures; the σ₀ = 10¹⁰ column is `unresx`'s `infd` row).
//!
//! `LSSF=1`, so both modules zero the File-3 background. Competition
//! (`MT=51`) opens at 45.107 keV: PURR's `infd` *total* above that includes
//! `sigx` read from the PENDF, which `infinite_dilution_reference` does not
//! model, so the 7-figure `unresx` comparison stays below the threshold
//! (20 and 40 keV) while the UNRESR comparison — which never adds `sigx` —
//! spans the whole range.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::purr::wfun::DopplerTable;
use njoy_outram_park_fork::purr::{infinite_dilution_reference, probability_table, Rng};
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use njoy_outram_park_fork::unresr::mf2::{parse_lru2_ranges, UnresolvedRange};
use njoy_outram_park_fork::unresr::unresolved_cross_sections;
use njoy_outram_park_fork::unresr::wfun::WTable;

const MAT: i32 = 9237;
const EL: f64 = 2.0e4;
const EH: f64 = 1.490087e5;

fn ranges() -> Option<Vec<UnresolvedRange>> {
    let path = reference_endf_or_skip("n-092_U_238.endf", "purr-u238")?;
    let tape = Tape::read_file(&path).unwrap();
    let sec = tape.section(MAT, 2, 151).expect("MF=2/MT=151");
    let ranges = parse_lru2_ranges(&sec.rows[1..]).unwrap();
    assert_eq!(ranges.len(), 1);
    let r = &ranges[0];
    assert_eq!(r.el, EL);
    assert_eq!(r.eh, EH);
    assert_eq!(r.nro, 1);
    assert_eq!(r.naps, 0);
    assert_eq!(r.lssf, 1);
    Some(ranges)
}

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

/// Relative agreement for non-zero oracle values, absolute for a zero one
/// (U-238 has no fission width in its URR, so every fission column is 0).
fn close(ours: f64, oracle: f64, tol: f64) -> bool {
    if oracle == 0.0 {
        ours.abs() < 1e-12
    } else {
        rel(ours, oracle) < tol
    }
}

#[test]
fn nro1_scattering_radius_table_is_read_and_interpolated_like_terpa() {
    let Some(ranges) = ranges() else { return };
    let r = &ranges[0];
    let t = r.ap_table.as_ref().expect("NRO=1 table");
    assert_eq!(t.xy.len(), 83);
    assert_eq!(t.interp, vec![(83, 5)]);
    // (The tape reader's `1.490087+5` lands at 149008.69999999998, so the
    // node comparisons are to 1e-12 relative rather than bit-exact.)
    let near = |a: f64, b: f64| rel(a, b) < 1e-12;
    assert_eq!(t.xy[0], (2.0e4, 9.454080e-1));
    assert!(near(t.xy[82].0, 1.490087e5) && near(t.xy[82].1, 9.136041e-1), "{:?}", t.xy[82]);
    // The header scalar is zero on this evaluation — the table is the only
    // source of the radius.
    assert_eq!(r.case_.ap(), 0.0);

    // Table nodes exactly.
    assert_eq!(r.scattering_radius(EL).unwrap(), 9.454080e-1);
    assert!(near(r.scattering_radius(EH).unwrap(), 9.136041e-1));
    // INT=5 between nodes: log-log between (2.0e4, 0.9454080) and
    // (2.05e4, 0.9453198).
    let e: f64 = 2.02e4;
    let expect = (0.9454080f64.ln()
        + (0.9453198f64 / 0.9454080).ln() * (e / 2.0e4).ln() / (2.05e4f64 / 2.0e4).ln())
    .exp();
    assert!((r.scattering_radius(e).unwrap() - expect).abs() < 1e-12);
    // terpa's edge conventions: the last value inside a 1.00001 shade above
    // the last node, zero beyond it, zero below the first node.
    assert!(near(r.scattering_radius(EH * 1.000_005).unwrap(), 9.136041e-1));
    assert_eq!(r.scattering_radius(EH * 1.01).unwrap(), 0.0);
    assert_eq!(r.scattering_radius(EL * 0.99).unwrap(), 0.0);
}

/// NJOY UNRESR at 300 K: `[total, elastic, fission, capture]` for σ₀ = 1e10
/// and σ₀ = 1 (first and last printed columns). The last row is the
/// range-top node `sigfig(EH,7,-1)` = 149008.6 eV (printed as 1.4901E+05).
const UNRESR_ORACLE: [(f64, [f64; 4], [f64; 4]); 4] = [
    (2.0e4, [14.36, 13.85, 0.0, 0.5124], [12.91, 12.47, 0.0, 0.4394]),
    (6.0e4, [12.54, 12.27, 0.0, 0.2637], [12.00, 11.75, 0.0, 0.2480]),
    (1.0e5, [11.47, 11.29, 0.0, 0.1758], [11.13, 10.96, 0.0, 0.1685]),
    (1.490086e5, [10.62, 10.48, 0.0, 0.1435], [10.39, 10.25, 0.0, 0.1383]),
];

/// NJOY PURR's `unresx` reference: `(E, spot, dbar, [total, elastic(+spot),
/// fission, capture])`; `spot`/`dbar` are the listing's 5 figures, the cross
/// sections the 7-figure `MT=152` σ₀ = 10¹⁰ column. NJOY's first grid energy
/// is `sigfig(EL,7,+1)` = 20000.01 eV (5e-7 relative, invisible here).
const UNRESX_ORACLE: [(f64, f64, f64, [f64; 4]); 2] = [
    (2.0e4, 10.940, 2.4276, [14.36093, 13.84856, 0.0, 0.5123672]),
    (4.0e4, 10.609, 2.3178, [13.29079, 12.91497, 0.0, 0.3758178]),
];

#[test]
fn unresr_and_infinite_dilution_reference_match_njoy_on_u238() {
    let Some(ranges) = ranges() else { return };
    let wtable = WTable::new();

    let mut failures: Vec<String> = Vec::new();
    let mut check = |ok: bool, msg: String| {
        if !ok {
            failures.push(msg);
        }
    };

    for &(e, spot, dbar, infd) in &UNRESX_ORACLE {
        let inf = infinite_dilution_reference(&ranges, e).unwrap();
        let el = inf.sigma_elastic_inf + inf.potential_scattering;
        let tot = el + inf.sigma_fission_inf + inf.sigma_capture_inf;
        println!(
            "unresx E={e}: spot {:.4} (NJOY {spot}) dbar {:.5} (NJOY {dbar}) tot {tot:.5} el {el:.5} fis {:.3e} cap {:.6} (NJOY {infd:?})",
            inf.potential_scattering,
            1.0 / inf.mean_inverse_spacing,
            inf.sigma_fission_inf,
            inf.sigma_capture_inf
        );
        for s in &inf.sequences {
            println!(
                "   seq: D {:.5} gn {:.4e} gf {:.4e} gg {:.4e} gx {:.4e} ndf n/f/x {}/{}/{}",
                s.dbar, s.gn_mean, s.gf_mean, s.gg_mean, s.gx_mean, s.ndf_n, s.ndf_f, s.ndf_x
            );
        }
        assert_eq!(inf.sequences.len(), 5, "U-238 URR: l=0 (1 J) + l=1 (2 J) + l=2 (2 J)");
        check(rel(inf.potential_scattering, spot) < 1e-4, format!("spot at {e}: {} vs {spot}", inf.potential_scattering));
        check(rel(1.0 / inf.mean_inverse_spacing, dbar) < 1e-4, format!("dbar at {e}"));
        check(close(tot, infd[0], 2e-5), format!("infd total at {e}: {tot} vs {}", infd[0]));
        check(close(el, infd[1], 2e-5), format!("infd elastic at {e}: {el} vs {}", infd[1]));
        check(close(inf.sigma_fission_inf, infd[2], 2e-5), format!("infd fission at {e}"));
        check(close(inf.sigma_capture_inf, infd[3], 2e-5), format!("infd capture at {e}: {} vs {}", inf.sigma_capture_inf, infd[3]));
    }

    for &(e, dilute, shielded) in &UNRESR_ORACLE {
        let rows = unresolved_cross_sections(&ranges, e, 300.0, &[1e10, 1.0], [0.0; 4], &wtable)
            .unwrap();
        println!("unresr E={e}: sig0=1e10 {:?} (NJOY {dilute:?})", rows[0]);
        println!("             sig0=1    {:?} (NJOY {shielded:?})", rows[1]);
        for k in 0..4 {
            // 4 printed significant figures → ±5e-4 relative rounding.
            check(close(rows[0][k], dilute[k], 1.5e-3), format!("E={e} sig0=1e10 col {k}: {} vs NJOY {}", rows[0][k], dilute[k]));
            check(close(rows[1][k], shielded[k], 1.5e-3), format!("E={e} sig0=1 col {k}: {} vs NJOY {}", rows[1][k], shielded[k]));
        }
        let inf = infinite_dilution_reference(&ranges, e).unwrap();
        check(rel(rows[0][3], inf.sigma_capture_inf) < 2e-3, format!("unresr vs unresx capture at {e}: {} vs {}", rows[0][3], inf.sigma_capture_inf));
        check(rel(rows[0][1], inf.sigma_elastic_inf + inf.potential_scattering) < 2e-3, format!("unresr vs unresx elastic at {e}"));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn purr_probability_table_at_first_energy_matches_njoy_on_u238() {
    let Some(ranges) = ranges() else { return };
    let dtable = DopplerTable::new();
    let mut rng = Rng::new(-101);
    let e = EL;
    let inf = infinite_dilution_reference(&ranges, e).unwrap();
    let sig0 = [1e10, 1e4, 1e3, 100.0, 10.0, 1.0];
    let r = probability_table(&inf.sequences, &inf, [0.0; 4], &sig0, &[300.0], 20, 32, 10_000, &mut rng, &dtable)
        .unwrap();
    let c = &r.convergence;
    println!(
        "aver {:.4} {:.4} {:.4} {:.5}   pcsd {:.2} {:.2} {:.2} {:.2}",
        c.mean_total, c.mean_elastic, c.mean_fission, c.mean_capture,
        c.pct_std_total, c.pct_std_elastic, c.pct_std_fission, c.pct_std_capture
    );
    let t = &r.tables[0];
    println!("bondarenko: {:?}", t.bondarenko);
    println!("direct sampling: {:?}", t.bondarenko_direct_sampling);
    println!("bin edges: {:?}", t.bin_upper_edge);
    println!("bin prob: {:?}", t.bin_probability);

    // NJOY (32 ladders, seed -101, nres 2184): aver 14.352 / 13.841 / 0 /
    // 0.51136, pcsd 1.81 / 1.83 / 0 / 2.99; direct-sampling Bondarenko at
    // σ₀ = 1: 12.578 / 12.133 / 0 / 0.44523.
    let njoy_aver = [14.352, 13.841, 0.0, 0.51136];
    let njoy_ds_1 = [12.578, 12.133, 0.0, 0.44523];

    assert_eq!(t.bin_probability.len(), 20);
    let psum: f64 = t.bin_probability.iter().sum();
    assert!((psum - 1.0).abs() < 1e-9, "probabilities sum to {psum}");
    assert!(t.bin_upper_edge.windows(2).all(|w| w[0] < w[1]));
    assert_eq!(*t.bin_upper_edge.last().unwrap(), 1.0e6);
    for k in [0, 1, 3] {
        let mean: f64 = t.bin_probability.iter().zip(&t.bin_xs).map(|(p, x)| p * x[k]).sum();
        assert!(rel(mean, t.bondarenko[0][k]) < 1e-6, "col {k}: table mean {mean} vs {}", t.bondarenko[0][k]);
    }
    for k in [0, 3] {
        for w in t.bondarenko.windows(2) {
            assert!(w[1][k] <= w[0][k] * (1.0 + 1e-12), "col {k} not monotone: {:?}", t.bondarenko);
        }
    }
    assert!(c.mean_fission.abs() < 1e-12);

    // Monte Carlo means over 32 ladders carry pcsd/√32 ≈ 0.3 % (total,
    // elastic) … 0.5 % (capture) of their own noise and the random streams
    // are not bit-identical to NJOY's (see the U-235 test) — 3 % as there.
    let ours = [c.mean_total, c.mean_elastic, c.mean_fission, c.mean_capture];
    for k in [0, 1, 3] {
        assert!(rel(ours[k], njoy_aver[k]) < 3e-2, "aver col {k}: {} vs NJOY {}", ours[k], njoy_aver[k]);
        assert!(rel(t.bondarenko_direct_sampling[0][k], njoy_aver[k]) < 3e-2, "ds inf col {k}");
        assert!(rel(t.bondarenko_direct_sampling[5][k], njoy_ds_1[k]) < 3e-2, "ds sig0=1 col {k}");
    }
    // Self-shielding ratios at σ₀ = 1 b: NJOY 0.8764 (total), 0.8766
    // (elastic), 0.8707 (capture) — U-238 self-shields far more than U-235
    // at its 20 keV URR floor (D = 2.4 eV vs 0.16 eV).
    let ratio = |k: usize| t.bondarenko[5][k] / t.bondarenko[0][k];
    let njoy_ratio = [12.578 / 14.352, 12.133 / 13.841, 0.0, 0.44523 / 0.51136];
    for k in [0, 1, 3] {
        println!("self-shielding ratio col {k}: {:.4} vs NJOY {:.4}", ratio(k), njoy_ratio[k]);
        assert!((ratio(k) - njoy_ratio[k]).abs() < 0.02, "ratio col {k}");
    }
}
