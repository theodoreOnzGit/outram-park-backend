// SPDX-License-Identifier: AGPL-3.0-only
//
// Test-only diagnostic for gh:#1667 (2026-11-19). Serves the port's numerical kernels
// (`npy.rs::{lstsq, eigvals, HouseholderQr}`, `vectfit.rs::evaluate`) over a pair of named pipes so
// that OpenMC's own, unmodified Python loop (`openmc_inputs/swap_openmc.py`) can call ONE of them in
// place of LAPACK/BLAS on identical inputs. No library function is changed or copied; this file only
// moves bytes. OpenMC is MIT-licensed (notice in `vectfit.rs`).

//! Kernel server for the single-kernel swap experiments of gh:#1667.
//!
//! Environment: `OUTRAM_WMP_SRV_REQ` and `OUTRAM_WMP_SRV_RESP` (paths of two FIFOs made by the caller).
//! The server opens the request pipe for reading first and the response pipe for writing second, so the
//! client must open them in that order (request for writing, then response for reading).
//!
//! Protocol, little-endian: a request is `op: u64` then the operands; every response is a run of `f64`.
//!
//! | op | operands | response |
//! |---|---|---|
//! | 1 `lstsq` | `m n: u64`, `rcond: f64`, `a` (`m*n`, column-major), `b` (`m`) | `x` (`n`) |
//! | 2 `eigvals` | `n: u64`, `a` (`n*n`, row-major) | `re, im` pairs (`2n`) |
//! | 3 `qr` (factor, keep it) | `m n: u64`, `a` (`m*n`, column-major) | `R` (`min(m,n) * n`, row-major) |
//! | 4 `apply_qt` on the kept factor | `lo hi m: u64`, `v` (`m`) | `(Q^T v)[lo..hi]` |
//! | 5 `evaluate` | `ns np nv: u64`, `s` (`ns`), poles (`2 np`), residues (`2 nv np`, `[vector][pole]`) | `nv * ns` |
//! | 6 `eigvals` variant | `n mode: u64`, `a` (`n*n`, row-major) | `re, im` pairs (`2n`), the pair order of `npy::eigvals` |
//! | 0 quit | | nothing |
//!
//! Op 6 modes: 0 = petir's Francis QR with GSL's balancing (what `eigvals` does), 1 = the same without balancing,
//! 2 = LAPACK `DGEBAL('B')` ([`petir::linalg::dgebal`]) first, then the Francis QR without GSL's balancing.

use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};

use crate::wmp::Cf64;

use petir::linalg::{dgebal, eigen_nonsymm, BalanceJob, Matrix};

use super::npy::{eigvals, lstsq, HouseholderQr};
use super::vectfit::evaluate;

fn read_u64(r: &mut impl Read) -> Option<u64> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b).ok()?;
    Some(u64::from_le_bytes(b))
}

fn read_f64(r: &mut impl Read) -> f64 {
    let mut b = [0u8; 8];
    r.read_exact(&mut b).expect("pipe closed mid-request");
    f64::from_le_bytes(b)
}

fn read_f64s(r: &mut impl Read, n: usize) -> Vec<f64> {
    let mut bytes = vec![0u8; 8 * n];
    r.read_exact(&mut bytes).expect("pipe closed mid-request");
    bytes.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect()
}

fn write_f64s(w: &mut impl Write, v: &[f64]) {
    for x in v {
        w.write_all(&x.to_le_bytes()).expect("response pipe");
    }
    w.flush().expect("response pipe");
}

fn pair_up(v: &[f64]) -> Vec<Cf64> {
    v.chunks_exact(2).map(|c| Cf64::new(c[0], c[1])).collect()
}

/// `eigvals` of `npy.rs` with a choice of balancing (a test-only copy of its pair-ordering step, which is
/// private there): `mode` as in the op 6 table.
fn eigvals_variant(n: usize, a: &[f64], mode: u64) -> Vec<Cf64> {
    let mut mat = Matrix::from_row_major(n, n, a.to_vec()).expect("matrix");
    let ev = match mode {
        0 => eigen_nonsymm(&mat, true),
        1 => eigen_nonsymm(&mat, false),
        _ => {
            dgebal(BalanceJob::Both, &mut mat).expect("dgebal");
            eigen_nonsymm(&mat, false)
        }
    }
    .expect("eigen_nonsymm");
    let mut vals: Vec<Cf64> = ev.iter().map(|z| Cf64::new(z.re, z.im)).collect();
    let mut i = 0;
    while i < vals.len() {
        if vals[i].im != 0.0 && i + 1 < vals.len() && vals[i + 1].re == vals[i].re && vals[i + 1].im == -vals[i].im {
            if vals[i].im < 0.0 {
                vals.swap(i, i + 1);
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    vals
}

/// Serve requests until the quit opcode or end of input. Returns the number of requests served.
fn serve(r: &mut impl Read, w: &mut impl Write) -> usize {
    let mut kept: Option<HouseholderQr> = None;
    let mut served = 0;
    while let Some(op) = read_u64(r) {
        match op {
            0 => break,
            1 => {
                let m = read_u64(r).unwrap() as usize;
                let n = read_u64(r).unwrap() as usize;
                let rcond = read_f64(r);
                let a = read_f64s(r, m * n);
                let b = read_f64s(r, m);
                let x = lstsq(m, n, &a, &[b], rcond).expect("lstsq").remove(0);
                write_f64s(w, &x);
            }
            2 => {
                let n = read_u64(r).unwrap() as usize;
                let a = read_f64s(r, n * n);
                let ev = eigvals(n, &a).expect("eigvals");
                let flat: Vec<f64> = ev.iter().flat_map(|z| [z.re, z.im]).collect();
                write_f64s(w, &flat);
            }
            3 => {
                let m = read_u64(r).unwrap() as usize;
                let n = read_u64(r).unwrap() as usize;
                let a = read_f64s(r, m * n);
                let qr = HouseholderQr::new(m, n, a);
                let k = m.min(n);
                let mut rm = vec![0.0_f64; k * n];
                for i in 0..k {
                    for j in 0..n {
                        rm[i * n + j] = qr.r(i, j);
                    }
                }
                kept = Some(qr);
                write_f64s(w, &rm);
            }
            4 => {
                let lo = read_u64(r).unwrap() as usize;
                let hi = read_u64(r).unwrap() as usize;
                let qr = kept.as_ref().expect("apply_qt before qr");
                // the vector length is the factored row count; the client sends it with the operands
                let m = read_u64(r).unwrap() as usize;
                let mut v = read_f64s(r, m);
                qr.apply_qt(&mut v);
                write_f64s(w, &v[lo..hi]);
            }
            5 => {
                let ns = read_u64(r).unwrap() as usize;
                let np = read_u64(r).unwrap() as usize;
                let nv = read_u64(r).unwrap() as usize;
                let s = read_f64s(r, ns);
                let poles = pair_up(&read_f64s(r, 2 * np));
                let flat = pair_up(&read_f64s(r, 2 * nv * np));
                let residues: Vec<Vec<Cf64>> = flat.chunks(np).map(|c| c.to_vec()).collect();
                let out = evaluate(&s, &poles, &residues, &[]);
                let flat: Vec<f64> = out.into_iter().flatten().collect();
                write_f64s(w, &flat);
            }
            6 => {
                let n = read_u64(r).unwrap() as usize;
                let mode = read_u64(r).unwrap();
                let a = read_f64s(r, n * n);
                let ev = eigvals_variant(n, &a, mode);
                let flat: Vec<f64> = ev.iter().flat_map(|z| [z.re, z.im]).collect();
                write_f64s(w, &flat);
            }
            other => panic!("unknown opcode {other}"),
        }
        served += 1;
    }
    served
}

#[test]
#[ignore = "diagnostic for gh:#1667: serves the port's kernels on FIFOs named by OUTRAM_WMP_SRV_REQ / _RESP"]
fn kernel_server() {
    let (Ok(req), Ok(resp)) = (std::env::var("OUTRAM_WMP_SRV_REQ"), std::env::var("OUTRAM_WMP_SRV_RESP")) else {
        return;
    };
    let mut r = BufReader::with_capacity(1 << 20, File::open(req).expect("request pipe"));
    let mut w = BufWriter::with_capacity(1 << 20, OpenOptions::new().write(true).open(resp).expect("response pipe"));
    let n = serve(&mut r, &mut w);
    eprintln!("kernel_server: {n} requests served");
}

/// The server protocol, driven in memory: `lstsq`, `eigvals`, `qr` + `apply_qt` and `evaluate` return
/// what the library functions return. (Reaches `serve`, `read_*`, `write_f64s`, `pair_up`.)
#[test]
fn kernel_server_protocol_matches_library_calls() {
    let le = |v: &[f64]| -> Vec<u8> { v.iter().flat_map(|x| x.to_le_bytes()).collect() };
    let mut req: Vec<u8> = Vec::new();
    // lstsq: 3 x 2, column-major
    let (m, n) = (3usize, 2usize);
    let a = [1.0, 2.0, 3.0, 1.0, 0.5, 0.25];
    let b = [1.0, 2.0, 4.0];
    req.extend(1u64.to_le_bytes());
    req.extend((m as u64).to_le_bytes());
    req.extend((n as u64).to_le_bytes());
    req.extend(f64::EPSILON.to_le_bytes());
    req.extend(le(&a));
    req.extend(le(&b));
    // eigvals: 2 x 2 row-major [[0, 1], [-1, 0]] has +-i
    req.extend(2u64.to_le_bytes());
    req.extend(2u64.to_le_bytes());
    req.extend(le(&[0.0, 1.0, -1.0, 0.0]));
    // qr then apply_qt of the first column: Q^T a_0 = (R_00, 0, 0)
    req.extend(3u64.to_le_bytes());
    req.extend((m as u64).to_le_bytes());
    req.extend((n as u64).to_le_bytes());
    req.extend(le(&a));
    req.extend(4u64.to_le_bytes());
    req.extend(0u64.to_le_bytes());
    req.extend(3u64.to_le_bytes());
    req.extend((m as u64).to_le_bytes());
    req.extend(le(&a[0..3]));
    // evaluate: one pole at 1 + i, residue 1, at s = 3: Re(1/(3 - 1 - i)) = 2/5
    req.extend(5u64.to_le_bytes());
    req.extend(1u64.to_le_bytes());
    req.extend(1u64.to_le_bytes());
    req.extend(1u64.to_le_bytes());
    req.extend(le(&[3.0]));
    req.extend(le(&[1.0, 1.0]));
    req.extend(le(&[1.0, 0.0]));
    // eigvals variants of a 3 x 3 with eigenvalues 1, 2, 3 (upper triangular): every mode returns them in some order
    for mode in 0u64..3 {
        req.extend(6u64.to_le_bytes());
        req.extend(3u64.to_le_bytes());
        req.extend(mode.to_le_bytes());
        req.extend(le(&[1.0, 5.0, 7.0, 0.0, 2.0, 9.0, 0.0, 0.0, 3.0]));
    }
    req.extend(0u64.to_le_bytes());
    let mut resp: Vec<u8> = Vec::new();
    let served = serve(&mut &req[..], &mut resp);
    assert_eq!(served, 8);
    let got: Vec<f64> = resp.chunks_exact(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect();
    let x = lstsq(m, n, &a, &[b.to_vec()], f64::EPSILON).unwrap().remove(0);
    assert_eq!(&got[0..2], &x[..]);
    assert_eq!(&got[2..6], &[0.0, 1.0, 0.0, -1.0]);
    // R is 2 x 2 row-major (4 values), then (Q^T a_0)[0..3] = (R_00, 0, 0)
    let r00 = got[6];
    assert!((got[10] - r00).abs() < 1e-15 && got[11].abs() < 1e-15 && got[12].abs() < 1e-15);
    assert!((got[13] - 0.4).abs() < 1e-15);
    for mode in 0..3 {
        let o = 14 + mode * 6;
        let mut re: Vec<f64> = (0..3).map(|k| got[o + 2 * k]).collect();
        re.sort_by(|p, q| p.partial_cmp(q).unwrap());
        for (k, want) in [1.0, 2.0, 3.0].iter().enumerate() {
            assert!((re[k] - want).abs() < 1e-14, "mode {mode}: {re:?}");
        }
        assert!((0..3).all(|k| got[o + 2 * k + 1] == 0.0));
    }
}
