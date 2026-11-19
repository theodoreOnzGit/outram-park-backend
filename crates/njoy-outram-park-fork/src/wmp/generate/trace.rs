// SPDX-License-Identifier: AGPL-3.0-only
//
// Test-only diagnostic for gh:#1390 (2026-11-19). The loop below is a COPY of
// `fit.rs::vectfit_xs` (itself a translation of OpenMC `openmc/data/multipole.py`
// `_vectfit_xs` at commit 8f79d9d02483c90e4134f03a9df4b75ce6b062bf, MIT licence,
// see `fit.rs` and `vectfit.rs` for the notice) with logging added. It calls the
// library's own `vectfit` and `evaluate`; no library function is changed.
//
// Why a copy: `vectfit_xs` has no observer hook, and adding one would change a
// library function for a diagnosis. The copy is validated against `vectfit_xs`
// (bit-identical poles and residues) by `trace_reproduces_vectfit_xs`.

//! Order-by-order trace of the port's `_vectfit_xs` on one piece (gh:#1390).
//!
//! Methodology and results: `verification_and_validation/wmp_generate_u238/README.md`
//! ("Diagnosis of the dense end"). Inputs are the files written by
//! `openmc_inputs/make_trial_inputs.py` (little-endian f64: `ne`, `nmt`, energy,
//! then `nmt` rows), the same files `openmc_inputs/trace_openmc.py` reads.
//!
//! Environment: `OUTRAM_WMP_TRACE_IN` (input file), `OUTRAM_WMP_TRACE_REF`
//! (the unperturbed input the final fit is counted against; default = input),
//! `OUTRAM_WMP_TRACE_OUT` (output prefix). Writes `<out>.log.tsv`,
//! `<out>.orders.tsv`, `<out>.poles.f64`, `<out>.final.f64`, `<out>.summary.txt`
//! in the same layout as the OpenMC trace (the OpenMC-only lstsq columns are
//! absent here).

use std::fmt::Write as _;
use std::time::Instant;

use crate::wmp::Cf64;

use super::npy::{ccmp, cdiv, count_peaks, interp, linspace};
use super::vectfit::{evaluate, vectfit};
use super::{vectfit_xs, VectfitOptions};

/// One test-grid evaluation of the order loop.
struct Row {
    order: usize,
    it: usize,
    n_real: usize,
    ratio: f64,
    ratio2: f64,
    maxre: f64,
    quality: f64,
    n_out: usize,
    accepted: bool,
}

/// The result of a traced fit.
struct Traced {
    rows: Vec<Row>,
    /// `(order, found_better, n_discarded, best_ratio_after, best_quality_after)`.
    order_end: Vec<(usize, bool, usize, f64, f64)>,
    poles_dump: Vec<f64>,
    best_poles: Vec<Cf64>,
    best_residues: Vec<Vec<Cf64>>,
    n_peaks: usize,
    orders: Vec<usize>,
    stop: &'static str,
}

fn read_input(path: &str) -> (Vec<f64>, Vec<Vec<f64>>) {
    let bytes = std::fs::read(path).expect("input file");
    let v: Vec<f64> = bytes.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect();
    let (ne, nmt) = (v[0] as usize, v[1] as usize);
    let energy = v[2..2 + ne].to_vec();
    let rows = (0..nmt).map(|k| v[2 + ne + k * ne..2 + ne + (k + 1) * ne].to_vec()).collect();
    (energy, rows)
}

/// `vectfit_xs` with the order loop logged. The expressions are those of `fit.rs`.
fn traced_fit(energy: &[f64], ce_xs: &[Vec<f64>], opts: &VectfitOptions) -> Traced {
    let ne = energy.len();
    let nmt = ce_xs.len();
    let (rtol, atol) = (opts.rtol, opts.atol);

    let n_finer = 10usize;
    let ne_test = (ne - 1) * n_finer + 1;
    let xt: Vec<f64> = (0..ne_test).map(|i| i as f64).collect();
    let xp: Vec<f64> = (0..ne_test).step_by(n_finer).map(|i| i as f64).collect();
    let mut test_energy = interp(&xt, &xp, energy);
    test_energy[0] = energy[0];
    test_energy[ne_test - 1] = energy[ne - 1];
    let test_xs_ref: Vec<Vec<f64>> = ce_xs.iter().map(|r| interp(&test_energy, energy, r)).collect();

    let f: Vec<Vec<f64>> = ce_xs.iter().map(|r| r.iter().zip(energy).map(|(x, e)| x * e).collect()).collect();
    let s: Vec<f64> = energy.iter().map(|e| e.sqrt()).collect();
    let test_s: Vec<f64> = test_energy.iter().map(|e| e.sqrt()).collect();

    let min_cross_section = 1e-7;
    let mut weight: Vec<Vec<f64>> = f.iter().map(|r| r.iter().map(|x| 1.0 / x).collect()).collect();
    for i in 0..nmt {
        if ce_xs[i].iter().all(|&x| x <= min_cross_section) {
            weight[i].iter_mut().for_each(|w| *w = 1.0);
        } else if ce_xs[i].iter().any(|&x| x <= min_cross_section) {
            let mut mx = f64::NEG_INFINITY;
            let mut first = true;
            for (k, &x) in ce_xs[i].iter().enumerate() {
                if x > min_cross_section {
                    let w = weight[i][k];
                    if first || w > mx {
                        mx = w;
                        first = false;
                    }
                }
            }
            for (k, &x) in ce_xs[i].iter().enumerate() {
                if x <= min_cross_section {
                    weight[i][k] = mx;
                }
            }
        }
    }

    let sum01: Vec<f64> = ce_xs[0].iter().zip(&ce_xs[1]).map(|(a, b)| a + b).collect();
    let n_peaks = count_peaks(&sum01);
    let lowest = 2.max(2 * n_peaks);
    let highest = 200.max(4 * n_peaks);
    let orders: Vec<usize> = (lowest..=highest).step_by(2).collect();

    let mut rows = Vec::new();
    let mut order_end = Vec::new();
    let mut poles_dump: Vec<f64> = Vec::new();
    let mut found_ideal = false;
    let mut n_discarded = 0usize;
    let mut best_quality = f64::NEG_INFINITY;
    let mut best_ratio = f64::NEG_INFINITY;
    let mut best: Option<(Vec<Cf64>, Vec<Vec<Cf64>>)> = None;
    let s0 = s[0];
    let s_last = s[ne - 1];
    let mut stop = "orders exhausted";

    for &order in &orders {
        let poles_r = linspace(s0, s_last, order / 2);
        let mut poles: Vec<Cf64> = Vec::with_capacity(order);
        for &pr in &poles_r {
            let p = Cf64::new(pr, pr * 0.01);
            poles.push(p);
            poles.push(p.conj());
        }
        poles.sort_by(|a, b| ccmp(*a, *b));

        let mut found_better = false;
        for it in 0..opts.n_vf_iter {
            let r = vectfit(&f, &s, &poles, &weight, 0, false, false).expect("vectfit");
            poles = r.poles;
            let mut residues = r.residues;

            let mut n_real = 0usize;
            let mut new_poles = Vec::with_capacity(poles.len());
            for p in &poles {
                if s0 <= p.re && p.re <= s_last && p.im == 0.0 {
                    new_poles.push(Cf64::new(p.re, p.re * 0.01));
                    new_poles.push(Cf64::new(p.re, -(p.re * 0.01)));
                    n_real += 1;
                } else {
                    new_poles.push(*p);
                }
            }
            if n_real > 0 {
                let r2 = vectfit(&f, &s, &new_poles, &weight, 0, true, false).expect("vectfit");
                new_poles = r2.poles;
                residues = r2.residues;
            }

            let ev = evaluate(&test_s, &new_poles, &residues, &[]);
            let test_xs: Vec<Vec<f64>> =
                ev.iter().map(|r| r.iter().zip(&test_energy).map(|(x, e)| x / e).collect()).collect();
            let mut any_nan = false;
            let mut all_le_atol = true;
            let mut maxre = f64::NEG_INFINITY;
            let mut c1 = 0usize;
            let mut c2 = 0usize;
            let total = nmt * ne_test;
            for i in 0..nmt {
                for k in 0..ne_test {
                    let ab = (test_xs[i][k] - test_xs_ref[i][k]).abs();
                    let re = ab / test_xs_ref[i][k];
                    if ab.is_nan() {
                        any_nan = true;
                    }
                    if !(ab <= atol) {
                        all_le_atol = false;
                    }
                    if ab > atol {
                        maxre = if re.is_nan() || maxre.is_nan() { f64::NAN } else { maxre.max(re) };
                    }
                    if re < rtol || ab < atol {
                        c1 += 1;
                    }
                    if re < 10.0 * rtol || ab < atol {
                        c2 += 1;
                    }
                }
            }
            let (maxre, ratio, ratio2) = if any_nan {
                (f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY)
            } else if all_le_atol {
                (0.0, 1.0, 1.0)
            } else {
                (maxre, c1 as f64 / total as f64, c2 as f64 / total as f64)
            };
            let pen = 0.1 * maxre;
            let pen = if 1.0 < pen { 1.0 } else { pen };
            let mut quality = ratio + ratio2 - pen - 0.001 * new_poles.len() as f64;
            if test_xs.iter().any(|r| r.iter().any(|&x| x < -atol)) {
                quality = f64::NEG_INFINITY;
            }
            let accepted = quality > best_quality;
            rows.push(Row {
                order,
                it: it + 1,
                n_real: new_poles.iter().filter(|p| p.im == 0.0).count(),
                ratio,
                ratio2,
                maxre,
                quality,
                n_out: total - c1,
                accepted,
            });
            poles_dump.extend(new_poles.iter().flat_map(|p| [p.re, p.im]));

            if accepted {
                found_better = true;
                best_quality = quality;
                best_ratio = ratio;
                best = Some((new_poles.clone(), residues.clone()));
                if best_ratio >= 1.0 {
                    found_ideal = true;
                    break;
                }
            }
        }
        if found_ideal {
            order_end.push((order, found_better, n_discarded, best_ratio, best_quality));
            stop = "found_ideal";
            break;
        }
        if found_better {
            n_discarded = 0;
        } else if order > (2 * n_peaks).max(50) && best_ratio > 0.7 {
            n_discarded += 1;
        }
        order_end.push((order, found_better, n_discarded, best_ratio, best_quality));
        if !found_better && order > (2 * n_peaks).max(50) && best_ratio > 0.7 && (n_discarded >= 10 || (n_discarded >= 5 && best_ratio > 0.9)) {
            stop = "acceleration";
            break;
        }
    }
    let (best_poles, best_residues) = best.expect("a fit was accepted");
    Traced { rows, order_end, poles_dump, best_poles, best_residues, n_peaks, orders, stop }
}

/// The merge of `fit.rs` (conjugate pairs to one pole, residues x2 / i).
fn merge(best_poles: &[Cf64], best_residues: &[Vec<Cf64>]) -> (Vec<Cf64>, Vec<Vec<Cf64>>) {
    let mut real_idx = Vec::new();
    let mut conj_idx = Vec::new();
    let mut found_conj = false;
    for (i, p) in best_poles.iter().enumerate() {
        if found_conj {
            found_conj = false;
            continue;
        }
        if p.im == 0.0 {
            real_idx.push(i);
        } else if i + 1 < best_poles.len() && p.conj() == best_poles[i + 1] {
            found_conj = true;
            conj_idx.push(i);
        } else {
            panic!("complex poles are not conjugate");
        }
    }
    let jj = Cf64::new(0.0, 1.0);
    let poles: Vec<Cf64> = real_idx.iter().chain(conj_idx.iter()).map(|&i| best_poles[i]).collect();
    let residues = best_residues
        .iter()
        .map(|row| {
            let mut out = Vec::new();
            for &i in &real_idx {
                out.push(cdiv(row[i], jj));
            }
            for &i in &conj_idx {
                out.push(cdiv(Cf64::new(row[i].re * 2.0, row[i].im * 2.0), jj));
            }
            out
        })
        .collect();
    (poles, residues)
}

/// Grid points outside `rtol`/`atol` of `data` for a merged pole set (the OpenMC trace's `grid_points_outside`).
fn grid_outside(energy: &[f64], data: &[Vec<f64>], poles: &[Cf64], residues: &[Vec<Cf64>]) -> usize {
    let s: Vec<f64> = energy.iter().map(|e| e.sqrt()).collect();
    let jj = Cf64::new(0.0, 1.0);
    let scaled: Vec<Vec<Cf64>> = residues.iter().map(|row| row.iter().map(|r| r.mul(jj)).collect()).collect();
    let ev = evaluate(&s, poles, &scaled, &[]);
    let mut n = 0;
    for (v, row) in ev.iter().enumerate() {
        for (i, x) in row.iter().enumerate() {
            let sg = x / energy[i];
            let dab = (sg - data[v][i]).abs();
            if !((dab / data[v][i].abs() < 1e-3) || (dab < 1e-5)) {
                n += 1;
            }
        }
    }
    n
}

fn fmt_f(x: f64) -> String {
    format!("{x:?}")
}

/// Trace one piece. Skips (passes) when `OUTRAM_WMP_TRACE_IN` is unset.
#[test]
#[ignore = "diagnostic for gh:#1390: about 20 min on a dense U-238 piece; needs OUTRAM_WMP_TRACE_IN and OUTRAM_WMP_TRACE_OUT"]
fn trace_one_piece() {
    let Ok(inp) = std::env::var("OUTRAM_WMP_TRACE_IN") else { return };
    let out = std::env::var("OUTRAM_WMP_TRACE_OUT").expect("OUTRAM_WMP_TRACE_OUT");
    let reference = std::env::var("OUTRAM_WMP_TRACE_REF").unwrap_or_else(|_| inp.clone());
    let (energy, ce) = read_input(&inp);
    let (_, ce_ref) = read_input(&reference);
    let t0 = Instant::now();
    let tr = traced_fit(&energy, &ce, &VectfitOptions::default());
    let secs = t0.elapsed().as_secs_f64();
    let (mp, mr) = merge(&tr.best_poles, &tr.best_residues);
    let outside = grid_outside(&energy, &ce_ref, &mp, &mr);

    let mut log = String::from("order\tit\tn_real\tratio\tratio2\tmaxre\tquality\tn_out\taccepted\n");
    for r in &tr.rows {
        let _ = writeln!(
            log,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            r.order, r.it, r.n_real, fmt_f(r.ratio), fmt_f(r.ratio2), fmt_f(r.maxre), fmt_f(r.quality), r.n_out, r.accepted as u8
        );
    }
    std::fs::write(format!("{out}.log.tsv"), log).unwrap();
    let mut ord = String::from("order\tfound_better\tn_discarded\tbest_ratio_after\tbest_quality_after\n");
    for o in &tr.order_end {
        let _ = writeln!(ord, "{}\t{}\t{}\t{}\t{}", o.0, o.1 as u8, o.2, fmt_f(o.3), fmt_f(o.4));
    }
    std::fs::write(format!("{out}.orders.tsv"), ord).unwrap();
    let bytes: Vec<u8> = tr.poles_dump.iter().flat_map(|x| x.to_le_bytes()).collect();
    std::fs::write(format!("{out}.poles.f64"), bytes).unwrap();
    let mut fin: Vec<f64> = vec![mp.len() as f64, mr.len() as f64];
    fin.extend(mp.iter().flat_map(|p| [p.re, p.im]));
    fin.extend(mr.iter().flat_map(|row| row.iter().flat_map(|r| [r.re, r.im])));
    std::fs::write(format!("{out}.final.f64"), fin.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<u8>>()).unwrap();
    let last = tr.order_end.last().unwrap().0;
    let summary = format!(
        "{{\"code\": \"port\", \"input\": \"{inp}\", \"n_peaks\": {}, \"first_order\": {}, \"last_order_in_list\": {}, \"last_order_visited\": {last}, \"orders_visited\": {}, \"stop\": \"{}\", \"final_poles\": {}, \"grid_points_outside\": {outside}, \"seconds\": {secs:.1}}}\n",
        tr.n_peaks, tr.orders[0], tr.orders[tr.orders.len() - 1], tr.order_end.len(), tr.stop, mp.len()
    );
    std::fs::write(format!("{out}.summary.txt"), &summary).unwrap();
    println!("{summary}");
}

/// The traced loop is the library's loop: same poles and residues, bit for bit,
/// on a cheap piece. Needs `OUTRAM_WMP_TRACE_VALIDATE_IN` (a piece input file).
#[test]
#[ignore = "validation of the diagnostic: needs OUTRAM_WMP_TRACE_VALIDATE_IN (about 2 min on U-238 piece 50)"]
fn trace_reproduces_vectfit_xs() {
    let Ok(inp) = std::env::var("OUTRAM_WMP_TRACE_VALIDATE_IN") else { return };
    let (energy, ce) = read_input(&inp);
    let opts = VectfitOptions::default();
    let tr = traced_fit(&energy, &ce, &opts);
    let (mp, mr) = merge(&tr.best_poles, &tr.best_residues);
    let lib = vectfit_xs(&energy, &ce, &opts).expect("vectfit_xs");
    assert_eq!(lib.poles.len(), mp.len(), "pole count");
    for (a, b) in lib.poles.iter().zip(&mp) {
        assert_eq!((a.re.to_bits(), a.im.to_bits()), (b.re.to_bits(), b.im.to_bits()), "pole bits");
    }
    for (ra, rb) in lib.residues.iter().zip(&mr) {
        for (a, b) in ra.iter().zip(rb) {
            assert_eq!((a.re.to_bits(), a.im.to_bits()), (b.re.to_bits(), b.im.to_bits()), "residue bits");
        }
    }
    println!("trace reproduces vectfit_xs bit for bit: {} poles", mp.len());
}
