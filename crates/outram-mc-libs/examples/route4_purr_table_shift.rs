// SPDX-License-Identifier: GPL-3.0

//! **How far gh:#931 moves route 4's URR probability tables** (data, before
//! any transport): PURR at `process`'s 20 bands / 64 ladders / 10 000
//! samples after HEATR (route 4 since gh:#931) against the pre-#931
//! 20 / 16 / 2000 without HEATR (`Nuclide::process_evaluation_pre_931`).
//!
//! # Methodology
//!
//! For each nuclide (ENDF/B-VIII.0, `reference-data/endf/`, 293.6 K,
//! tolerance 1e-3) both products are made, and at every tabulated URR energy
//! two numbers are formed from each table's bands (probability `p_i`, total
//! `t_i`, capture `c_i`, fission `f_i`; barns, or factors on the smooth
//! cross section when `LSSF = 1`, the same either way for a ratio between
//! the two tables):
//!
//! - the **infinite-dilution** capture `Σ p_i c_i` (what an unshielded
//!   medium sees; PURR preserves it up to sampling noise);
//! - the **fully shielded** capture `Σ p_i c_i / t_i / Σ p_i / t_i` (the
//!   `σ0 → 0` Bondarenko limit; the largest self-shielding a medium can
//!   apply, so an upper bound on what a table change can do to transport).
//!
//! The same for fission. Reported per nuclide: the relative change new/old −
//! 1, its mean over the energies (a systematic shift) and its RMS (noise).
//! Also the load time of each product, with the hardware.
//!
//! Run: `cargo run --release -p outram-mc-libs --example
//! route4_purr_table_shift [NAME:TAPE ...]` (default U-235 and U-238).
//!
//! # Results
//!
//! Recorded in `verification_and_validation/route4_process_931/README.md`.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::purr::UrrProbabilityTables;
use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::perf_report::HardwareInfo;
use std::time::Instant;

const TEMP_K: f64 = 293.6;
const TOL: f64 = 1.0e-3;

/// Per energy: `(E, infinite-dilution capture, shielded capture,
/// infinite-dilution fission, shielded fission)`, from
/// [`UrrProbabilityTables::to_f64s`]'s layout.
fn band_moments(t: &UrrProbabilityTables) -> Vec<[f64; 5]> {
    let w = t.to_f64s();
    let n_e = w[7] as usize;
    let energy = &w[8..8 + n_e];
    let mut at = 8 + n_e;
    let mut out = Vec::with_capacity(n_e);
    for &e in energy {
        let nb = w[at] as usize;
        let cum = &w[at + 1..at + 1 + nb];
        let vals = &w[at + 1 + nb..at + 1 + 5 * nb];
        let (mut cid, mut fid, mut inv, mut cs, mut fs) = (0.0, 0.0, 0.0, 0.0, 0.0);
        let mut prev = 0.0;
        for i in 0..nb {
            let p = cum[i] - prev;
            prev = cum[i];
            let (tot, fis, cap) = (vals[4 * i], vals[4 * i + 2], vals[4 * i + 3]);
            cid += p * cap;
            fid += p * fis;
            if tot > 0.0 {
                inv += p / tot;
                cs += p * cap / tot;
                fs += p * fis / tot;
            }
        }
        out.push([
            e,
            cid,
            if inv > 0.0 { cs / inv } else { 0.0 },
            fid,
            if inv > 0.0 { fs / inv } else { 0.0 },
        ]);
        at += 1 + 6 * nb;
    }
    out
}

/// Mean and RMS of `new/old − 1` over the entries where `old` is non-zero.
fn shift(old: &[f64], new: &[f64]) -> (f64, f64, usize) {
    let r: Vec<f64> = old
        .iter()
        .zip(new)
        .filter(|(o, _)| **o != 0.0)
        .map(|(o, n)| n / o - 1.0)
        .collect();
    if r.is_empty() {
        return (0.0, 0.0, 0);
    }
    let n = r.len() as f64;
    let mean = r.iter().sum::<f64>() / n;
    let rms = (r.iter().map(|x| x * x).sum::<f64>() / n).sqrt();
    (mean, rms, r.len())
}

/// No test reaches `main`: CLI glue around `band_moments`/`shift`
/// (tested below) and two library calls tested in `tests/route4_is_process.rs`.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let list: Vec<(String, String)> = if args.is_empty() {
        vec![
            ("U235".into(), "n-092_U_235-ENDF8.0.endf".into()),
            ("U238".into(), "n-092_U_238.endf".into()),
        ]
    } else {
        args.iter()
            .map(|a| {
                let (n, f) = a.split_once(':').expect("NAME:TAPE");
                (n.to_string(), f.to_string())
            })
            .collect()
    };
    println!("{}", HardwareInfo::detect().headline());
    println!("| nuclide | energies | load pre-931 [s] | load process [s] | inf-dil capture mean / RMS | shielded capture mean / RMS | inf-dil fission mean / RMS | shielded fission mean / RMS |");
    println!("|---|---|---|---|---|---|---|---|");
    for (name, file) in list {
        let p = reference_endf(&file).unwrap_or_else(|| panic!("missing {file}"));
        let tape = Tape::read_file(&p).expect("tape");
        let mat = tape.materials()[0];
        let t0 = Instant::now();
        let old =
            Nuclide::process_evaluation_pre_931(&tape, mat, TEMP_K, TOL, TOL).expect("pre-931");
        let t_old = t0.elapsed().as_secs_f64();
        let t0 = Instant::now();
        let new = Nuclide::process_evaluation(&tape, mat, TEMP_K, TOL, TOL).expect("route 4");
        let t_new = t0.elapsed().as_secs_f64();
        let (Some(a), Some(b)) = (old.urr.as_ref(), new.urr.as_ref()) else {
            println!("| {name} | no URR | {t_old:.1} | {t_new:.1} | | | | |");
            continue;
        };
        let (ma, mb) = (band_moments(a), band_moments(b));
        assert_eq!(ma.len(), mb.len(), "same energy grid");
        let col = |k: usize| {
            shift(
                &ma.iter().map(|r| r[k]).collect::<Vec<_>>(),
                &mb.iter().map(|r| r[k]).collect::<Vec<_>>(),
            )
        };
        let fmt = |(m, r, _): (f64, f64, usize)| format!("{:+.2e} / {:.2e}", m, r);
        println!(
            "| {name} | {} | {t_old:.1} | {t_new:.1} | {} | {} | {} | {} |",
            ma.len(),
            fmt(col(1)),
            fmt(col(2)),
            fmt(col(3)),
            fmt(col(4))
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `shift` on a known pair, and `band_moments` on a two-band table:
    /// p = (0.25, 0.75), t = (10, 2), c = (4, 1), f = (0, 0) gives
    /// infinite-dilution capture 0.25*4 + 0.75*1 = 1.75 and shielded
    /// (0.25*0.4 + 0.75*0.5) / (0.25/10 + 0.75/2) = 0.475 / 0.4 = 1.1875.
    #[test]
    fn moments_and_shift_by_hand() {
        let (m, r, n) = shift(&[1.0, 2.0, 0.0], &[1.1, 1.8, 5.0]);
        assert_eq!(n, 2);
        assert!((m - 0.0).abs() < 1e-12 && (r - 0.1).abs() < 1e-12);
        let words = vec![
            0.0, 1.0e4, 2.0e4, 293.6, 2.0, -1.0, -1.0, 1.0, 1.5e4, // header, one energy
            2.0, 0.25, 1.0, // two bands, cumulative
            10.0, 5.0, 0.0, 4.0, 2.0, 1.0, 0.0, 1.0, // (t, e, f, c) per band
            0.0, 0.0, // heating
        ];
        let t = UrrProbabilityTables::from_f64s(&words).expect("table");
        let m = band_moments(&t);
        assert_eq!(m.len(), 1);
        assert!((m[0][1] - 1.75).abs() < 1e-12, "{:?}", m[0]);
        assert!((m[0][2] - 1.1875).abs() < 1e-12, "{:?}", m[0]);
    }
}
