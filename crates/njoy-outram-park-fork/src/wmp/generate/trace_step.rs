// SPDX-License-Identifier: AGPL-3.0-only
//
// Test-only diagnostic for gh:#1390 (2026-11-19). Calls the library's `vectfit` unchanged.
// `vectfit` is a translation of OpenMC `openmc/data/vectfit.py` at commit
// 8f79d9d02483c90e4134f03a9df4b75ce6b062bf (MIT licence, notice in `vectfit.rs`).

//! Single-step differential: one relocation step of the port (`vectfit`), started from the exact
//! poles OpenMC's trace logged at iteration `k`, against the poles OpenMC logged at iteration `k + 1`.
//!
//! Why: the traces of the two codes diverge through chaos or through a defect. A single step has no
//! accumulated chaos: given identical input poles and identical cross sections, how far does the
//! port's output lie from OpenMC's, compared with how far OpenMC's own output moves when its input
//! poles change by one ulp (`openmc_inputs/step_openmc.py`)?
//!
//! Environment: `OUTRAM_WMP_STEP_IN` (piece input file), `OUTRAM_WMP_STEP_LOG` / `OUTRAM_WMP_STEP_POLES`
//! (an OpenMC trace's `.log.tsv` / `.poles.f64`), `OUTRAM_WMP_STEP_OUT` (result tsv),
//! `OUTRAM_WMP_STEP_ROWS` (comma-separated trace row indices `k`; step `k` -> `k + 1`).

use std::fmt::Write as _;
use std::time::Instant;

use crate::wmp::Cf64;

use super::npy::count_peaks;
use super::vectfit::vectfit;

fn read_input(path: &str) -> (Vec<f64>, Vec<Vec<f64>>) {
    let bytes = std::fs::read(path).expect("input file");
    let v: Vec<f64> = bytes.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect();
    let (ne, nmt) = (v[0] as usize, v[1] as usize);
    let energy = v[2..2 + ne].to_vec();
    let rows = (0..nmt).map(|k| v[2 + ne + k * ne..2 + ne + (k + 1) * ne].to_vec()).collect();
    (energy, rows)
}

/// Largest and median over the poles of `a` of `|a - nearest b| / |a|`.
fn nearest_rel_diff(a: &[Cf64], b: &[Cf64]) -> (f64, f64) {
    let mut d: Vec<f64> = a
        .iter()
        .map(|x| {
            let best = b.iter().map(|y| ((x.re - y.re) * (x.re - y.re) + (x.im - y.im) * (x.im - y.im)).sqrt()).fold(f64::INFINITY, f64::min);
            let r = best / (x.re * x.re + x.im * x.im).sqrt();
            if r.is_nan() { f64::INFINITY } else { r }
        })
        .collect();
    d.sort_by(|p, q| p.partial_cmp(q).unwrap());
    (d[d.len() - 1], d[d.len() / 2])
}

#[test]
#[ignore = "diagnostic for gh:#1390: needs the OUTRAM_WMP_STEP_* variables and an OpenMC trace"]
fn single_step_differential() {
    let Ok(inp) = std::env::var("OUTRAM_WMP_STEP_IN") else { return };
    let log = std::fs::read_to_string(std::env::var("OUTRAM_WMP_STEP_LOG").unwrap()).unwrap();
    let pbytes = std::fs::read(std::env::var("OUTRAM_WMP_STEP_POLES").unwrap()).unwrap();
    let out = std::env::var("OUTRAM_WMP_STEP_OUT").unwrap();
    let wanted: Vec<usize> = std::env::var("OUTRAM_WMP_STEP_ROWS").unwrap().split(',').map(|x| x.parse().unwrap()).collect();
    let (energy, ce) = read_input(&inp);
    let nmt = ce.len();
    // the same weights as `vectfit_xs` (fit.rs, copied as in trace.rs)
    let f: Vec<Vec<f64>> = ce.iter().map(|r| r.iter().zip(&energy).map(|(x, e)| x * e).collect()).collect();
    let s: Vec<f64> = energy.iter().map(|e| e.sqrt()).collect();
    let min_cross_section = 1e-7;
    let mut weight: Vec<Vec<f64>> = f.iter().map(|r| r.iter().map(|x| 1.0 / x).collect()).collect();
    for i in 0..nmt {
        if ce[i].iter().all(|&x| x <= min_cross_section) {
            weight[i].iter_mut().for_each(|w| *w = 1.0);
        } else if ce[i].iter().any(|&x| x <= min_cross_section) {
            let mut mx = f64::NEG_INFINITY;
            let mut first = true;
            for (k, &x) in ce[i].iter().enumerate() {
                if x > min_cross_section {
                    let w = weight[i][k];
                    if first || w > mx {
                        mx = w;
                        first = false;
                    }
                }
            }
            for (k, &x) in ce[i].iter().enumerate() {
                if x <= min_cross_section {
                    weight[i][k] = mx;
                }
            }
        }
    }
    let sum01: Vec<f64> = ce[0].iter().zip(&ce[1]).map(|(a, b)| a + b).collect();
    let first_order = 2.max(2 * count_peaks(&sum01));
    // The OpenMC log's column `order` is the size of new_poles; the true order of row r is
    // first_order + 2 (r / 30) (30 iterations per order).
    let mut lines = log.lines();
    let header: Vec<&str> = lines.next().unwrap().split('\t').collect();
    let col = header.iter().position(|h| *h == "order").unwrap();
    let sizes: Vec<usize> = lines.map(|l| l.split('\t').nth(col).unwrap().parse().unwrap()).collect();
    let vals: Vec<f64> = pbytes.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect();
    let mut offs = Vec::with_capacity(sizes.len() + 1);
    let mut o = 0usize;
    for &n in &sizes {
        offs.push(o);
        o += 2 * n;
    }
    assert_eq!(o, vals.len(), "poles file does not match the log");
    // new_poles has each in-range real pole split into p(1 +- 0.01j); the raw relocation output (the next
    // iteration's input) is recovered by collapsing every adjacent pair with im = +-0.01 re exactly back to
    // the real pole (a genuine complex pole has that relation with probability zero).
    let poles_of = |r: usize| -> Vec<Cf64> {
        let z: Vec<Cf64> = vals[offs[r]..offs[r] + 2 * sizes[r]].chunks_exact(2).map(|z| Cf64::new(z[0], z[1])).collect();
        let mut raw = Vec::with_capacity(z.len());
        let mut i = 0;
        while i < z.len() {
            if i + 1 < z.len() && z[i].im != 0.0 && z[i].im == z[i].re * 0.01 && z[i + 1].re == z[i].re && z[i + 1].im == -(z[i].re * 0.01) {
                raw.push(Cf64::new(z[i].re, 0.0));
                i += 2;
            } else {
                raw.push(z[i]);
                i += 1;
            }
        }
        raw
    };
    let mut text = String::from("row\torder\titer_from\tn_poles\tmax_rel\tmedian_rel\tt_s\n");
    for &k in &wanted {
        let order = first_order + 2 * (k / 30);
        if (k + 1) / 30 != k / 30 {
            continue; // the next row belongs to another order
        }
        let input = poles_of(k);
        let expect = poles_of(k + 1);
        if input.len() != order || expect.len() != order {
            continue; // not a relocation output (cannot happen unless a pair was not a split)
        }
        let t0 = Instant::now();
        let r = vectfit(&f, &s, &input, &weight, 0, false, false).expect("vectfit");
        let (mx, med) = nearest_rel_diff(&expect, &r.poles);
        let _ = writeln!(text, "{k}\t{order}\t{}\t{order}\t{mx:?}\t{med:?}\t{:.2}", k % 30, t0.elapsed().as_secs_f64());
    }
    std::fs::write(out, text).unwrap();
}
