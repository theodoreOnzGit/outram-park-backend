// SPDX-License-Identifier: AGPL-3.0-only
//
// Test-only diagnostic for gh:#1390 (2026-11-19). Calls the library's `lstsq` and `eigvals`
// (generate/npy.rs) on matrices dumped from OpenMC 8f79d9d's `vectfit.py`
// (`openmc_inputs/dump_kernel_inputs.py`; OpenMC is MIT-licensed, notice in `vectfit.rs`).

//! Kernel differential: the port's least-squares solve and eigenvalue solver on the exact matrices of one
//! OpenMC pole-relocation call, against what LAPACK (DGELSD, DGEEV) returned for them.
//!
//! Environment: `OUTRAM_WMP_KERNEL_FILES` (comma-separated dump files).

use petir::linalg::{Matrix, Svd};

use crate::wmp::Cf64;

use super::npy::{eigvals, lstsq, HouseholderQr};

struct Dump {
    m: usize,
    n: usize,
    a: Vec<f64>,
    b: Vec<f64>,
    x: Vec<f64>,
    s: Vec<f64>,
    rank: usize,
    nn: usize,
    h: Vec<f64>,
    ev: Vec<Cf64>,
}

fn read_dump(path: &str) -> Dump {
    let bytes = std::fs::read(path).expect("dump file");
    let v: Vec<f64> = bytes.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect();
    let (m, n) = (v[0] as usize, v[1] as usize);
    let mut p = 2;
    let mut take = |k: usize| {
        let r = v[p..p + k].to_vec();
        p += k;
        r
    };
    let a = take(m * n);
    let b = take(m);
    let x = take(n);
    let ns = take(1)[0] as usize;
    let s = take(ns);
    let rank = take(1)[0] as usize;
    let nn = take(1)[0] as usize;
    let h = take(nn * nn);
    let ev = take(2 * nn).chunks_exact(2).map(|z| Cf64::new(z[0], z[1])).collect();
    Dump { m, n, a, b, x, s, rank, nn, h, ev }
}

/// Largest `|a - nearest b| / |a|` over `a`.
fn nearest_max(a: &[Cf64], b: &[Cf64]) -> f64 {
    a.iter()
        .map(|x| {
            let best = b.iter().map(|y| ((x.re - y.re) * (x.re - y.re) + (x.im - y.im) * (x.im - y.im)).sqrt()).fold(f64::INFINITY, f64::min);
            best / (x.re * x.re + x.im * x.im).sqrt()
        })
        .fold(0.0, f64::max)
}

/// Every `lstsq` record of the files in `OUTRAM_WMP_RANK_FILES` (`dump_kernel_rows.py` output): the numerical rank
/// LAPACK (DGELSD, `cond = eps`) returned against the rank of the port's solve, and the solution difference.
/// One line per record: `file record m n rank_lapack rank_port near_lapack near_port x_rel_diff`.
#[test]
#[ignore = "diagnostic for gh:#1390: needs OUTRAM_WMP_RANK_FILES (dump_kernel_rows.py output)"]
fn rank_differential() {
    let Ok(files) = std::env::var("OUTRAM_WMP_RANK_FILES") else { return };
    for path in files.split(',') {
        let bytes = std::fs::read(path).expect("dump file");
        let v: Vec<f64> = bytes.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect();
        let nrec = v[0] as usize;
        let mut p = 1;
        for rec in 0..nrec {
            let (m, n) = (v[p] as usize, v[p + 1] as usize);
            p += 2;
            let a = &v[p..p + m * n];
            p += m * n;
            let b = v[p..p + m].to_vec();
            p += m;
            let xl = &v[p..p + n];
            p += n;
            let ns = v[p] as usize;
            p += 1;
            let sl = &v[p..p + ns];
            p += ns;
            let rank_l = v[p] as usize;
            p += 1;
            let mut col = vec![0.0_f64; m * n];
            for i in 0..m {
                for j in 0..n {
                    col[j * m + i] = a[i * n + j];
                }
            }
            let x = lstsq(m, n, &col, &[b], f64::EPSILON).expect("lstsq").remove(0);
            let xmax = xl.iter().fold(0.0_f64, |p, q| p.max(q.abs()));
            let dx = x.iter().zip(xl).fold(0.0_f64, |p, (a, b)| p.max((a - b).abs())) / xmax;
            let qr = HouseholderQr::new(m.max(n), n, {
                let mut c = vec![0.0_f64; m.max(n) * n];
                for j in 0..n {
                    c[j * m.max(n)..j * m.max(n) + m].copy_from_slice(&col[j * m..(j + 1) * m]);
                }
                c
            });
            let mut rdata = vec![0.0_f64; n * n];
            for i in 0..n {
                for j in i..n {
                    rdata[i * n + j] = qr.r(i, j);
                }
            }
            let svd = Svd::new(&Matrix::from_row_major(n, n, rdata).expect("matrix")).expect("svd");
            let mut sp: Vec<f64> = svd.s().to_vec();
            sp.sort_by(|p, q| q.partial_cmp(p).unwrap());
            let thr_l = f64::EPSILON * sl.iter().cloned().fold(0.0_f64, f64::max);
            let thr_p = f64::EPSILON * sp[0];
            let rank_p = sp.iter().filter(|&&q| q > thr_p).count();
            let near_l = sl.iter().filter(|&&q| q > thr_l / 100.0 && q < thr_l * 100.0).count();
            let near_p = sp.iter().filter(|&&q| q > thr_p / 100.0 && q < thr_p * 100.0).count();
            println!("RANK {path} {rec} {m} {n} {rank_l} {rank_p} {near_l} {near_p} {dx:.3e}");
        }
    }
}

#[test]
#[ignore = "diagnostic for gh:#1390: needs OUTRAM_WMP_KERNEL_FILES (dump_kernel_inputs.py output)"]
fn kernel_differential() {
    let Ok(files) = std::env::var("OUTRAM_WMP_KERNEL_FILES") else { return };
    for path in files.split(',') {
        let d = read_dump(path);
        let (m, n) = (d.m, d.n);
        // lstsq as `wlstsq` calls it (the matrix is already column-scaled by the dump)
        let mut col = vec![0.0_f64; m * n];
        for i in 0..m {
            for j in 0..n {
                col[j * m + i] = d.a[i * n + j];
            }
        }
        let x = lstsq(m, n, &col, &[d.b.clone()], f64::EPSILON).expect("lstsq").remove(0);
        let xmax = d.x.iter().fold(0.0_f64, |p, q| p.max(q.abs()));
        let dx = x.iter().zip(&d.x).fold(0.0_f64, |p, (a, b)| p.max((a - b).abs())) / xmax;
        // singular values of R, as lstsq computes them
        let qr = HouseholderQr::new(m, n, col.clone());
        let mut rdata = vec![0.0_f64; n * n];
        for i in 0..n {
            for j in i..n {
                rdata[i * n + j] = qr.r(i, j);
            }
        }
        let svd = Svd::new(&Matrix::from_row_major(n, n, rdata).expect("matrix")).expect("svd");
        let mut sp: Vec<f64> = svd.s().to_vec();
        sp.sort_by(|p, q| q.partial_cmp(p).unwrap());
        let smax_l = d.s.iter().cloned().fold(0.0_f64, f64::max);
        let smax_p = sp[0];
        let thr_l = f64::EPSILON * smax_l;
        let thr_p = f64::EPSILON * smax_p;
        let rank_p = sp.iter().filter(|&&q| q > thr_p).count();
        // singular values relative to s_max, worst absolute difference, and those around the cutoff
        let ds = d.s.iter().zip(&sp).fold(0.0_f64, |p, (a, b)| p.max((a / smax_l - b / smax_p).abs()));
        let near_l = d.s.iter().filter(|&&q| q > thr_l / 100.0 && q < thr_l * 100.0).count();
        let near_p = sp.iter().filter(|&&q| q > thr_p / 100.0 && q < thr_p * 100.0).count();
        // eigenvalues of the same matrix
        let ep = eigvals(d.nn, &d.h).expect("eigvals");
        let de = nearest_max(&d.ev, &ep);
        println!(
            "{path}: lstsq {m}x{n}: rank LAPACK {} port {rank_p}; |x_port - x_lapack|/|x|_max = {dx:.3e}; worst |s_i/s_max| difference {ds:.3e}; \
             singular values within 1e2 of the cutoff: LAPACK {near_l}, port {near_p}; eig N = {}: worst relative eigenvalue difference {de:.3e}",
            d.rank, d.nn
        );
    }
}
