// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **V&V: the dense kernels against faer 0.24.4's own compiled ones, bit for
//! bit** (gh #994). `faer` is a dev-dependency, so these unit tests call
//! `faer::linalg::{matmul, triangular_solve, lu, cholesky}` on the same
//! strided buffers and compare every `f64` of the output buffer — padding
//! included, so a kernel that writes outside its view is caught too.
//!
//! # Methodology
//!
//! Seeded xorshift entries; each case lays the operands out column-major or
//! row-major (leading dimension padded by 3), and for the product also
//! reversed, so faer's stride-driven dispatch (matrix-vector, rank-1,
//! nano-gemm, private-gemm, the stride flips) is exercised path by path.
//! Shapes straddle every threshold the dispatch uses (`n == 1`, `m == 1`,
//! `k == 1`, `m*n*k = 4096`, depth 16, 64 and 512), the triangular solve's
//! base case (n <= 4), its column split (more than 64 right-hand sides) and
//! block sizes, the LU's recursion threshold (16) and the `LLᵀ`'s 16-row band
//! and 64/128 recursion.
//!
//! Pass criterion: every output `f64` bit-identical to faer's. faer runs on
//! the host CPU, so the pin holds on an x86-64 with AVX2 and FMA and without
//! AVX-512 (the CPU the port reproduces); on another CPU faer itself takes
//! other kernels.
//!
//! # Results
//!
//! See the `RESULT` lines and the crate's `tests/sparse_vs_faer.rs`
//! documentation for the date and machine they were recorded on.

// The whole file is test code (included as `#[cfg(test)] mod vs_faer;`);
// the attribute below also tells `tests/no_panic_gate.rs` so.
#[cfg(test)]
use super::factor::{llt_in_place, lu_in_place_recursion};
use super::matmul::matmul;
use super::trsm;
use super::{Accum, View};
use faer::dyn_stack::{MemBuffer, MemStack};
use faer::{MatMut, MatRef, Par};
use std::vec::Vec;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        ((x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64) - 0.5
    }
    fn fill(&mut self, n: usize) -> Vec<f64> {
        (0..n).map(|_| self.next()).collect()
    }
}

#[derive(Clone, Copy, Debug)]
enum Lay {
    Col,
    Row,
    RevCol,
}

/// A view of `nrows x ncols` in a buffer of the returned length.
fn lay(l: Lay, nrows: usize, ncols: usize) -> (View, usize) {
    match l {
        Lay::Col => {
            let ld = nrows + 3;
            (View::col_major(0, nrows, ncols, ld), ld * ncols.max(1) + 3)
        }
        Lay::Row => {
            let ld = ncols + 3;
            (
                View::col_major(0, ncols, nrows, ld).t(),
                ld * nrows.max(1) + 3,
            )
        }
        Lay::RevCol => {
            let ld = nrows + 3;
            (
                View::col_major(0, nrows, ncols, ld).rev_rows(),
                ld * ncols.max(1) + 3,
            )
        }
    }
}

fn fref(buf: &[f64], l: Lay, nrows: usize, ncols: usize) -> MatRef<'_, f64> {
    match l {
        Lay::Col => MatRef::from_column_major_slice_with_stride(buf, nrows, ncols, nrows + 3),
        Lay::Row => MatRef::from_row_major_slice_with_stride(buf, nrows, ncols, ncols + 3),
        Lay::RevCol => {
            MatRef::from_column_major_slice_with_stride(buf, nrows, ncols, nrows + 3).reverse_rows()
        }
    }
}

fn fmut(buf: &mut [f64], l: Lay, nrows: usize, ncols: usize) -> MatMut<'_, f64> {
    match l {
        Lay::Col => MatMut::from_column_major_slice_with_stride_mut(buf, nrows, ncols, nrows + 3),
        // Not `from_row_major_slice_with_stride_mut`: in faer 0.24.4 that
        // constructor returns a COLUMN-major view (strides `(1, row_stride)`,
        // matmut.rs:1742), unlike its `MatRef` counterpart.
        Lay::Row => MatMut::from_column_major_slice_with_stride_mut(buf, ncols, nrows, ncols + 3)
            .transpose_mut(),
        Lay::RevCol => {
            MatMut::from_column_major_slice_with_stride_mut(buf, nrows, ncols, nrows + 3)
                .reverse_rows_mut()
        }
    }
}

fn same(a: &[f64], b: &[f64]) -> bool {
    let ok = a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.to_bits() == y.to_bits());
    if !ok {
        for (i, (x, y)) in a.iter().zip(b).enumerate() {
            if x.to_bits() != y.to_bits() {
                std::eprintln!("  [{i}] ours {x:e} faer {y:e}");
            }
        }
    }
    ok
}

#[test]
fn matmul_is_bit_identical_to_faer() {
    let mut r = Rng(0x9e37_79b9_7f4a_7c15);
    let shapes = [
        (1, 1, 1),
        (1, 7, 5),
        (7, 1, 5),
        (4, 4, 1),
        (1, 1, 9),
        (3, 5, 2),
        (16, 16, 16),
        (17, 16, 16),
        (5, 9, 17),
        (5, 9, 70),
        (40, 30, 20),
        (13, 1, 300),
        (1, 13, 300),
        (2, 3, 700),
        (6, 7, 1100),
        (33, 2, 64),
        (70, 50, 65),
    ];
    let lays = [Lay::Col, Lay::Row, Lay::RevCol];
    let mut cases = 0usize;
    for &(m, n, k) in &shapes {
        for &dl in &lays {
            for &ll in &lays {
                for &rl in &lays {
                    for (beta, alpha) in
                        [(Accum::Replace, 1.0), (Accum::Add, -1.0), (Accum::Add, 0.7)]
                    {
                        let (dv, dn) = lay(dl, m, n);
                        let (lv, ln) = lay(ll, m, k);
                        let (rv, rn) = lay(rl, k, n);
                        let a = r.fill(ln);
                        let b = r.fill(rn);
                        let c0 = r.fill(dn);
                        let mut ours = c0.clone();
                        matmul(&mut ours, dv, beta, Some(&a), lv, Some(&b), rv, alpha).unwrap();
                        let mut theirs = c0.clone();
                        faer::linalg::matmul::matmul(
                            fmut(&mut theirs, dl, m, n),
                            match beta {
                                Accum::Replace => faer::Accum::Replace,
                                Accum::Add => faer::Accum::Add,
                            },
                            fref(&a, ll, m, k),
                            fref(&b, rl, k, n),
                            alpha,
                            Par::Seq,
                        );
                        assert!(
                            same(&ours, &theirs),
                            "matmul {m}x{n}x{k} dst {dl:?} lhs {ll:?} rhs {rl:?} {beta:?} {alpha}"
                        );
                        cases += 1;
                    }
                }
            }
        }
    }
    std::println!("RESULT dense matmul: {cases}/{cases} cases bit-identical to faer");
}

#[test]
fn triangular_solves_are_bit_identical_to_faer() {
    let mut r = Rng(0x1234_5678_9abc_def1);
    let mut cases = 0usize;
    for &n in &[1usize, 2, 3, 4, 5, 6, 9, 17, 33, 70, 130] {
        for &k in &[1usize, 3, 70] {
            for &tl in &[Lay::Col, Lay::Row] {
                for &bl in &[Lay::Col, Lay::Row] {
                    for which in 0..4 {
                        let (tv, tn) = lay(tl, n, n);
                        let mut t = r.fill(tn);
                        for i in 0..n {
                            let p = tv.at(i, i) as usize;
                            t[p] = 2.0 + t[p];
                        }
                        let (bv, bn) = lay(bl, n, k);
                        let b0 = r.fill(bn);
                        let mut ours = b0.clone();
                        let mut theirs = b0.clone();
                        let tr = fref(&t, tl, n, n);
                        let fb = fmut(&mut theirs, bl, n, k);
                        use faer::linalg::triangular_solve as ts;
                        match which {
                            0 => {
                                trsm::lower(&mut ours, Some(&t), tv, bv).unwrap();
                                ts::solve_lower_triangular_in_place(tr, fb, Par::Seq);
                            }
                            1 => {
                                trsm::unit_lower(&mut ours, Some(&t), tv, bv).unwrap();
                                ts::solve_unit_lower_triangular_in_place(tr, fb, Par::Seq);
                            }
                            2 => {
                                trsm::upper(&mut ours, Some(&t), tv, bv).unwrap();
                                ts::solve_upper_triangular_in_place(tr, fb, Par::Seq);
                            }
                            _ => {
                                trsm::unit_upper(&mut ours, Some(&t), tv, bv).unwrap();
                                ts::solve_unit_upper_triangular_in_place(tr, fb, Par::Seq);
                            }
                        }
                        assert!(
                            same(&ours, &theirs),
                            "trsm {which} n={n} k={k} tri {tl:?} rhs {bl:?}"
                        );
                        cases += 1;
                    }
                }
            }
        }
    }
    std::println!("RESULT dense trsm: {cases}/{cases} cases bit-identical to faer");
}

#[test]
fn partial_pivoting_lu_is_bit_identical_to_faer() {
    let mut r = Rng(0x0bad_cafe_dead_beef);
    let mut cases = 0usize;
    for &(m, n) in &[
        (1usize, 1usize),
        (5, 3),
        (16, 16),
        (17, 17),
        (40, 33),
        (100, 64),
        (200, 150),
        (300, 7),
    ] {
        let (v, len) = lay(Lay::Col, m, n);
        let a = r.fill(len);
        let mut ours = a.clone();
        let mut trans = std::vec![0usize; n];
        lu_in_place_recursion(&mut ours, v, 0, n, &mut trans).unwrap();
        let mut ours_perm: Vec<usize> = (0..m).collect();
        for (i, &t) in trans.iter().enumerate() {
            ours_perm.swap(i, i + t);
        }
        let mut theirs = a.clone();
        let mut perm = std::vec![0usize; m];
        let mut perm_inv = std::vec![0usize; m];
        let mut mem = MemBuffer::new(
            faer::linalg::lu::partial_pivoting::factor::lu_in_place_scratch::<usize, f64>(
                m,
                n,
                Par::Seq,
                Default::default(),
            ),
        );
        faer::linalg::lu::partial_pivoting::factor::lu_in_place(
            fmut(&mut theirs, Lay::Col, m, n),
            &mut perm,
            &mut perm_inv,
            Par::Seq,
            MemStack::new(&mut mem),
            Default::default(),
        );
        assert!(same(&ours, &theirs), "lu {m}x{n} values");
        assert_eq!(ours_perm, perm, "lu {m}x{n} permutation");
        cases += 1;
    }
    std::println!("RESULT dense lu: {cases}/{cases} cases bit-identical to faer");
}

#[test]
fn llt_is_bit_identical_to_faer() {
    let mut r = Rng(0x5eed_5eed_5eed_5eed);
    let mut cases = 0usize;
    for &n in &[1usize, 2, 5, 16, 17, 40, 64, 65, 100, 130, 300] {
        let (v, len) = lay(Lay::Col, n, n);
        let b = r.fill(n * n);
        // A = B B^T + n I, stored in the padded buffer.
        let mut a = r.fill(len);
        for j in 0..n {
            for i in 0..n {
                let mut s = if i == j { n as f64 } else { 0.0 };
                for k in 0..n {
                    s += b[i + k * n] * b[j + k * n];
                }
                a[v.at(i, j) as usize] = s;
            }
        }
        let mut ours = a.clone();
        llt_in_place(&mut ours, v).unwrap();
        let mut theirs = a.clone();
        let mut mem = MemBuffer::new(
            faer::linalg::cholesky::llt::factor::cholesky_in_place_scratch::<f64>(
                n,
                Par::Seq,
                Default::default(),
            ),
        );
        faer::linalg::cholesky::llt::factor::cholesky_in_place(
            fmut(&mut theirs, Lay::Col, n, n),
            Default::default(),
            Par::Seq,
            MemStack::new(&mut mem),
            Default::default(),
        )
        .unwrap();
        assert!(same(&ours, &theirs), "llt n={n}");
        cases += 1;
    }
    std::println!("RESULT dense llt: {cases}/{cases} cases bit-identical to faer");
}
