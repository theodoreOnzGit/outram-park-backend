// SPDX-License-Identifier: AGPL-3.0-only

//! **Port-reconstructed cross sections against the OpenMC `.h5` library,
//! nuclide by nuclide and band by band** (GitHub #550, 2026-11-19).
//!
//! # Methodology
//!
//! For each nuclide of the 11-nuclide LCT-008 tier, build two `Nuclide`s: the
//! default route (`Nuclide::from_endf_file_with_speed`, RECONR + BROADR at
//! 293.6 K, ENDF/B-VIII.0 tape) and the OpenMC route (`Nuclide::from_hdf5`
//! on the NJOY2016-made `.h5`, `294K` cross sections). Evaluate both on one
//! fine log grid (`--per-decade` points per decade, default 4000) and, per
//! reaction (total, elastic, capture = absorption - fission, fission) and per
//! energy band, report the **lethargy-weighted band mean** of each route and
//! its relative difference `(port - h5) / h5`, plus the largest pointwise
//! relative difference in the band. Equal weight per unit lethargy is the
//! slowing-down (1/E) weighting of an epithermal flux, and a band mean is
//! grid-independent where a pointwise difference inside a resonance is not
//! (two independently thinned grids never agree point for point there).
//!
//! No physics is changed; no tolerance is gated. This is an instrument.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example xs_port_vs_h5_550 -- --hdf5-dir DIR [--nuclides H1,U238] [--per-decade 4000]
//! ```
//!
//! Results: `verification_and_validation/tutorial_rung4/gap_240_550.md`.

use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::nuclide::Nuclide;
use petir::mathf::RealMath;

#[path = "common/lct008_model.rs"]
mod lct008_model;

const TEMP_K: f64 = 293.6;

/// Energy bands [eV]: sub-thermal, thermal, resolved, keV, MeV, fast.
const BANDS: [(f64, f64, &str); 8] = [
    (1.0e-5, 1.0e-2, "1e-5..0.01"),
    (1.0e-2, 0.1, "0.01..0.1"),
    (0.1, 0.625, "0.1..0.625"),
    (0.625, 10.0, "0.625..10"),
    (10.0, 1.0e3, "10..1e3"),
    (1.0e3, 1.0e5, "1e3..1e5"),
    (1.0e5, 1.0e6, "1e5..1e6"),
    (1.0e6, 2.0e7, "1e6..2e7"),
];

/// Relative difference `(a - b) / b`, `0.0` when both vanish.
fn rel(a: f64, b: f64) -> f64 {
    if b == 0.0 {
        if a == 0.0 {
            0.0
        } else {
            f64::INFINITY
        }
    } else {
        (a - b) / b
    }
}

/// Mean of samples taken at equal lethargy steps: the 1/E-weighted band
/// mean of the sampled function.
fn lethargy_mean(vals: &[f64]) -> f64 {
    vals.iter().sum::<f64>() / vals.len() as f64
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |f: &str| -> Option<String> {
        let i = args.iter().position(|a| a == f)?;
        args.get(i + 1).cloned()
    };
    let dir = std::path::PathBuf::from(value("--hdf5-dir").expect("--hdf5-dir"));
    let per_decade: usize = value("--per-decade").map_or(4000, |v| v.parse().expect("per-decade"));
    let only: Option<Vec<String>> =
        value("--nuclides").map(|v| v.split(',').map(|s| s.trim().to_string()).collect());
    println!("nuclide,band,reaction,port_mean,h5_mean,rel_diff,max_pointwise_rel");
    for (name, file) in lct008_model::TAPES_CHEAP {
        if only.as_ref().is_some_and(|l| !l.iter().any(|n| n == name)) {
            continue;
        }
        let p = reference_endf(file).unwrap_or_else(|| panic!("missing tape {file}"));
        let port =
            Nuclide::from_endf_file_with_speed(&p, name, TEMP_K, outram_mc_libs::vv::bench_speed())
                .unwrap_or_else(|e| panic!("{name} tape: {e:?}"));
        let read = njoy_outram_park_fork::hdf5::nuclide_read::read_nuclide_selected(
            dir.join(format!("{name}.h5")),
            Some("294K"),
            &["0K"],
        )
        .unwrap_or_else(|e| panic!("{name}.h5: {e}"));
        let h5 =
            Nuclide::from_hdf5(&read, name).unwrap_or_else(|e| panic!("from_hdf5 {name}: {e}"));
        for &(lo, hi, label) in BANDS.iter() {
            let decades = (hi / lo).r_ln() / 10.0_f64.r_ln();
            let n = ((decades * per_decade as f64) as usize).max(50);
            // reaction order: total, elastic, capture, fission
            let mut a: [Vec<f64>; 4] = Default::default();
            let mut b: [Vec<f64>; 4] = Default::default();
            let mut worst = [0.0_f64; 4];
            for i in 0..n {
                let e = lo * (hi / lo).r_powf((i as f64 + 0.5) / n as f64);
                let xp = port.xs_at_energy(e, TEMP_K);
                let xh = h5.xs_at_energy(e, TEMP_K);
                let vp = [xp.total, xp.elastic, xp.absorption - xp.fission, xp.fission];
                let vh = [xh.total, xh.elastic, xh.absorption - xh.fission, xh.fission];
                for k in 0..4 {
                    a[k].push(vp[k]);
                    b[k].push(vh[k]);
                    let r = rel(vp[k], vh[k]).abs();
                    if r.is_finite() && r > worst[k] {
                        worst[k] = r;
                    }
                }
            }
            for (k, rname) in ["total", "elastic", "capture", "fission"].iter().enumerate() {
                let (ia, ib) = (lethargy_mean(&a[k]), lethargy_mean(&b[k]));
                println!(
                    "{name},{label},{rname},{ia:.6e},{ib:.6e},{:.3e},{:.3e}",
                    rel(ia, ib),
                    worst[k]
                );
            }
        }
        // Non-cross-section data the two routes may carry differently: the
        // data kT (free-gas scattering), the URR range, and the mean elastic
        // CM cosine at a few energies (MF=4 angular law).
        println!(
            "#kT,{name},port={:?},h5={:?},urr_port={:?},urr_h5={:?},dbrc_port={},dbrc_h5={}",
            port.data_kt_ev(),
            h5.data_kt_ev(),
            port.urr_range_ev(),
            h5.urr_range_ev(),
            port.has_dbrc(),
            h5.has_dbrc()
        );
        for e in [1.0e-2, 0.0253, 1.0, 100.0, 1.0e4, 1.0e5, 1.0e6, 5.0e6] {
            println!(
                "#mubar,{name},{e:e},port={:.6},h5={:.6}",
                port.elastic_mubar_cm(e),
                h5.elastic_mubar_cm(e)
            );
        }
        // Resonance-scattering 0 K elastic table, nu-bar and the fission
        // spectrum, port against `.h5`.
        if let (Some(dp), Some(dh)) = (port.dbrc_table(), h5.dbrc_table()) {
            for &(lo, hi, label) in BANDS.iter().filter(|b| b.1 <= 1.0e3 && b.0 >= 1.0e-2) {
                let n = 4000;
                let (mut sp, mut sh, mut worst) = (0.0, 0.0, 0.0_f64);
                for i in 0..n {
                    let e = lo * (hi / lo).r_powf((i as f64 + 0.5) / n as f64);
                    let (a, b) = (dp.xs_at(e), dh.xs_at(e));
                    sp += a;
                    sh += b;
                    worst = worst.max(rel(a, b).abs());
                }
                println!("#dbrc0k,{name},{label},mean_rel={:.3e},max_pointwise_rel={worst:.3e}", rel(sp, sh));
            }
        }
        if port.nu_bar(0.0253) > 0.0 {
            for e in [0.0253, 1.0, 1.0e3, 1.0e5, 1.0e6, 2.0e6, 5.0e6, 1.0e7] {
                println!("#nubar,{name},{e:e},port={:.6},h5={:.6}", port.nu_bar(e), h5.nu_bar(e));
            }
            for e_in in [0.0253, 1.0e6] {
                let (mut mp, mut mh) = (0.0, 0.0);
                let (mut sp, mut sh) = (1234567u64, 1234567u64);
                let n = 400_000;
                for _ in 0..n {
                    mp += port.sample_fission_energy(e_in, &mut sp);
                    mh += h5.sample_fission_energy(e_in, &mut sh);
                }
                println!(
                    "#chi,{name},{e_in:e},mean_E_port={:.5e},mean_E_h5={:.5e},rel={:.3e}",
                    mp / n as f64,
                    mh / n as f64,
                    rel(mp, mh)
                );
            }
        }
        // URR probability tables: the mean (over xi) of the table-sampled total
        // and capture, in 8 log bands across the unresolved range, port against
        // `.h5`; the tabulated infinite-dilute value is the xs above.
        if let (Some((lo, hi)), Some(_)) = (port.urr_range_ev(), h5.urr_range_ev()) {
            let nb = 8;
            for b in 0..nb {
                let (l, u) = (
                    lo * (hi / lo).r_powf(b as f64 / nb as f64),
                    lo * (hi / lo).r_powf((b + 1) as f64 / nb as f64),
                );
                let (mut tp, mut th, mut cp, mut ch, mut dp, mut dh) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
                let ne = 100;
                let nx = 200;
                for i in 0..ne {
                    let e = l * (u / l).r_powf((i as f64 + 0.5) / ne as f64);
                    let (xa, xb) = (port.xs_at_energy(e, TEMP_K), h5.xs_at_energy(e, TEMP_K));
                    dp += xa.total;
                    dh += xb.total;
                    for j in 0..nx {
                        let xi = (j as f64 + 0.5) / nx as f64;
                        let (a, h) = (port.xs_at_energy_urr(e, TEMP_K, xi), h5.xs_at_energy_urr(e, TEMP_K, xi));
                        tp += a.total;
                        th += h.total;
                        cp += a.absorption - a.fission;
                        ch += h.absorption - h.fission;
                    }
                }
                println!(
                    "#urr,{name},{l:.4e}..{u:.4e},total_table_mean_rel={:.3e},capture_table_mean_rel={:.3e},port_table_vs_infdilute={:.3e},h5_table_vs_infdilute={:.3e}",
                    rel(tp, th),
                    rel(cp, ch),
                    rel(tp / nx as f64, dp),
                    rel(th / nx as f64, dh)
                );
            }
        }
        eprintln!("{name} done");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two helpers the report is built from: `rel` and `lethargy_mean`.
    #[test]
    fn relative_difference_and_lethargy_mean() {
        assert!((rel(1.001, 1.0) - 1.0e-3).abs() < 1.0e-12);
        assert_eq!(rel(0.0, 0.0), 0.0);
        assert!(rel(1.0, 0.0).is_infinite());
        assert_eq!(lethargy_mean(&[1.0, 2.0, 3.0]), 2.0);
    }
}
