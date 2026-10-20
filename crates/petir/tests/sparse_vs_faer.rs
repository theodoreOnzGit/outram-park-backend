// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **V&V: `petir::sparse` against the `faer` 0.24.4 it was ported from**
//! (gh #994), with faer kept as a dev-dependency so the comparison runs
//! against upstream's own compiled code on every test run.
//!
//! # Methodology
//!
//! For each matrix below, the same CSC input (built by
//! `CscMatrix::try_from_triplets` and by faer's
//! `SparseColMat::try_new_from_triplets` from one triplet list, no duplicates)
//! is factorised and solved by both codes against one right-hand side. Checked:
//!
//! 1. **Orderings, exactly:** petir's COLAMD column permutation against faer's
//!    `SymbolicLu::col_perm`, and petir's AMD permutation against faer's
//!    `SymbolicCholesky::perm`. Integer, so equality is the only pass.
//! 2. **LU, bit for bit, three ways:** PETIR's simplicial path against
//!    faer's forced simplicial (`SupernodalThreshold::FORCE_SIMPLICIAL`),
//!    PETIR's supernodal path against faer's `FORCE_SUPERNODAL`, and PETIR's
//!    default (`SymbolicKind::Auto`) against faer's default `sp_lu`, after
//!    checking both pick the same path.
//! 3. **Cholesky, bit for bit, the same three ways** against
//!    `sp_cholesky(Side::Lower)` and `factorize_symbolic_cholesky` with each
//!    threshold forced (the path decision is also checked against faer's).
//!
//! ~~3. **LU against faer's default `sp_lu`** (AUTO path choice): bit for
//! bit when faer picks simplicial; when it picks supernodal, the largest
//! relative difference is measured and must be at rounding level~~
//! **CORRECTED 2026-10-20:** since the supernodal paths were ported (gh
//! #994, "Port lah") every comparison is bit for bit; the rounding-level
//! allowance is gone.
//!
//! Matrices: seeded random unsymmetric (diagonally weighted) at n = 1, 5, 40,
//! 300, 1500; a two-group 3-D seven-point convection-diffusion operator
//! (n = 2 x 12^3 = 3456, and 2 x 24^3 = 27 648) shaped like `bedok`'s nodal
//! diffusion `LHS`; SPD lower triangles of a 2-D five-point Laplacian plus
//! identity (n = 30^2 and 120^2) like `outram-blender`'s `I + lambda L`, and a
//! seeded random sparse SPD matrix.
//!
//! # Results
//!
//! **2026-10-20, faer 0.24.4, x86_64 Linux (i9-13900K: AVX2+FMA, no
//! AVX-512), release, after the supernodal port** — from the `RESULT` lines:
//!
//! | case | n | perm | simplicial vs faer | supernodal vs faer | faer AUTO picks | AUTO vs faer |
//! |---|---|---|---|---|---|---|
//! | LU random | 1, 5, 40 | identical | all bits | all bits | simplicial | all bits |
//! | LU random | 300, 1500 | identical | all bits | all bits | supernodal | all bits |
//! | LU 2-group 3-D diffusion | 3456, 27 648 | identical | all bits | all bits | supernodal | all bits |
//! | LLT 2-D Laplacian | 900, 14 400 | identical | all bits | all bits | simplicial | all bits |
//! | LLT random SPD | 1, 50 | identical | all bits | all bits | simplicial | all bits |
//! | LLT random SPD | 2000 | identical | all bits | all bits | supernodal | all bits |
//!
//! Every solution entry of every case is bit-identical to faer's; the path
//! decision agreed with faer's on every case. Before the supernodal port
//! (same day) the AUTO column read "133/300 bits, worst 7.1e-16" (random300),
//! "366/1500, 2.8e-15", "204/3456, 5.0e-15", "32/27 648, 4.9e-14" and, for
//! the LLT, "372/2000, 1.4e-15"; those were the simplicial port against
//! faer's supernodal path.
//!
//! **Timing** (`factorisation_time_vs_faer`, best of 3, one thread, same day,
//! before the GEMM loop interchange that took the scalar GEMM from 1.55 to
//! 1.2-1.4 ns per FMA): LU n = 3456: PETIR simplicial 0.143 s, PETIR
//! supernodal 0.738 s, faer AUTO (supernodal) 0.032 s; LU n = 27 648: 20.5 s,
//! 69.5 s, 1.42 s; LLT n = 2000 (supernodal): PETIR 0.038 s, faer 0.003 s.
//! **The supernodal path is slower than the simplicial one in PETIR**: faer's
//! kernels do ~8 FMAs per cycle in AVX2 registers, PETIR's scalar replay one
//! `libm::fma` call per FMA (see `petir::sparse`'s module docs).
//!
//! Superseded record (2026-10-20, simplicial port only): `bedok`'s n = 10 404
//! `LHS` 3383/10 404 bit-identical to faer, worst 2.2e-15; factorisation 1.21
//! s (faer simplicial 1.17-1.22 s, faer supernodal 0.17 s). `outram-blender`'s
//! 44 SPD systems (n = 8 to 266): all simplicial, all bit-identical.

use faer::dyn_stack::{MemBuffer, MemStack};
use faer::linalg::solvers::Solve;
use faer::sparse::linalg::lu::{factorize_symbolic_lu, LuSymbolicParams, NumericLu};
use faer::sparse::linalg::SupernodalThreshold;
use faer::sparse::{SparseColMat, Triplet};
use faer::{Conj, Mat, Par, Side};
use petir::sparse::{CscMatrix, SparseLlt, SparseLu, SymbolicKind};

/// xorshift64*: a test-only, seeded source of matrix entries.
struct Rng(u64);
impl Rng {
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

fn dedup(n: usize, mut t: Vec<(usize, usize, f64)>) -> Vec<(usize, usize, f64)> {
    let mut seen = std::collections::HashSet::new();
    t.retain(|&(i, j, _)| seen.insert((i, j)));
    assert!(t.iter().all(|&(i, j, _)| i < n && j < n));
    t
}

fn random_unsym(n: usize, per_col: usize, seed: u64) -> Vec<(usize, usize, f64)> {
    let mut r = Rng(seed);
    let mut t = Vec::new();
    for j in 0..n {
        t.push((j, j, 4.0 + r.unit()));
        for _ in 0..per_col {
            t.push((r.below(n), j, r.unit() - 0.5));
        }
    }
    dedup(n, t)
}

/// Two-group 3-D seven-point convection-diffusion on an `m^3` grid, with a
/// down-scatter coupling from group 0 into group 1: unsymmetric, like bedok's
/// nodal-diffusion LHS.
fn diffusion_3d(m: usize) -> Vec<(usize, usize, f64)> {
    let idx = |g: usize, x: usize, y: usize, z: usize| g * m * m * m + (z * m + y) * m + x;
    let mut t = Vec::new();
    for g in 0..2 {
        let d = [1.3, 0.4][g];
        let rem = [0.03, 0.09][g];
        for z in 0..m {
            for y in 0..m {
                for x in 0..m {
                    let c = idx(g, x, y, z);
                    let mut diag = rem;
                    let nb = [
                        (x > 0, (x.wrapping_sub(1), y, z), 1.1),
                        (x + 1 < m, (x + 1, y, z), 0.9),
                        (y > 0, (x, y.wrapping_sub(1), z), 1.05),
                        (y + 1 < m, (x, y + 1, z), 0.95),
                        (z > 0, (x, y, z.wrapping_sub(1)), 1.0),
                        (z + 1 < m, (x, y, z + 1), 1.0),
                    ];
                    for (ok, (a, b, cc), skew) in nb {
                        diag += d;
                        if ok {
                            t.push((c, idx(g, a, b, cc), -d * skew));
                        }
                    }
                    t.push((c, c, diag));
                    if g == 1 {
                        t.push((c, idx(0, x, y, z), -0.02));
                    }
                }
            }
        }
    }
    t
}

/// Lower triangle of `I + lambda * L` for a 2-D five-point grid Laplacian.
fn laplacian_2d_lower(m: usize, lambda: f64) -> Vec<(usize, usize, f64)> {
    let mut t = Vec::new();
    for y in 0..m {
        for x in 0..m {
            let c = y * m + x;
            let mut deg = 0.0;
            for (ok, nb) in [
                (x > 0, c.wrapping_sub(1)),
                (x + 1 < m, c + 1),
                (y > 0, c.wrapping_sub(m)),
                (y + 1 < m, c + m),
            ] {
                if ok {
                    deg += 1.0;
                    if nb < c {
                        t.push((c, nb, -lambda));
                    }
                }
            }
            t.push((c, c, 1.0 + lambda * deg));
        }
    }
    t
}

/// Lower triangle of a seeded random sparse SPD matrix (diagonally dominant).
fn random_spd_lower(n: usize, per_col: usize, seed: u64) -> Vec<(usize, usize, f64)> {
    let mut r = Rng(seed);
    let mut off = Vec::new();
    for j in 0..n {
        for _ in 0..per_col {
            let i = r.below(n);
            if i > j {
                off.push((i, j, r.unit() - 0.5));
            }
        }
    }
    let off = dedup(n, off);
    let mut rowsum = vec![0.0; n];
    for &(i, j, v) in &off {
        rowsum[i] += f64::abs(v);
        rowsum[j] += f64::abs(v);
    }
    let mut t = off;
    for (j, s) in rowsum.iter().enumerate() {
        t.push((j, j, 1.0 + s));
    }
    t
}

fn rhs(n: usize) -> Vec<f64> {
    (0..n)
        .map(|i| 1.0 + (i % 7) as f64 * 0.25 - (i % 3) as f64 * 0.5)
        .collect()
}

fn faer_mat(n: usize, t: &[(usize, usize, f64)]) -> SparseColMat<usize, f64> {
    let tr: Vec<Triplet<usize, usize, f64>> =
        t.iter().map(|&(i, j, v)| Triplet::new(i, j, v)).collect();
    SparseColMat::try_new_from_triplets(n, n, &tr).unwrap()
}

fn col(b: &[f64]) -> Mat<f64> {
    Mat::from_fn(b.len(), 1, |i, _| b[i])
}

fn max_rel_diff(a: &[f64], b: &[f64]) -> (usize, f64) {
    let scale = b.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    let bits = a
        .iter()
        .zip(b)
        .filter(|(x, y)| x.to_bits() == y.to_bits())
        .count();
    let worst = a
        .iter()
        .zip(b)
        .map(|(x, y)| (x - y).abs() / scale)
        .fold(0.0, f64::max);
    (bits, worst)
}

fn check_lu(name: &str, n: usize, t: &[(usize, usize, f64)]) {
    let ours = CscMatrix::try_from_triplets(n, n, t).unwrap();
    let theirs = faer_mat(n, t);
    assert_eq!(
        ours.col_ptr(),
        theirs.symbolic().col_ptr(),
        "{name}: CSC col_ptr"
    );
    assert_eq!(
        ours.row_idx(),
        theirs.symbolic().row_idx(),
        "{name}: CSC row_idx"
    );
    let b = rhs(n);

    let lu = SparseLu::factorize_with(&ours, SymbolicKind::Simplicial).expect("petir LU");
    let x = lu.solve(&b).unwrap();

    // faer, simplicial forced.
    let xs = faer_lu_forced(&theirs, &b, SupernodalThreshold::FORCE_SIMPLICIAL, Some(lu.col_perm()), name);
    let (bits, worst) = max_rel_diff(&x, &xs);
    println!("RESULT lu {name} n={n}: simplicial vs faer FORCE_SIMPLICIAL {bits}/{n} bit-identical, worst rel {worst:.3e}");
    assert_eq!(bits, n, "{name}: not bit-identical to faer's simplicial LU");

    // supernodal forced, both sides.
    let lus = SparseLu::factorize_with(&ours, SymbolicKind::Supernodal).expect("petir supernodal LU");
    assert!(lus.is_supernodal());
    let xsn = lus.solve(&b).unwrap();
    let xfs = faer_lu_forced(&theirs, &b, SupernodalThreshold::FORCE_SUPERNODAL, None, name);
    let (bits, worst) = max_rel_diff(&xsn, &xfs);
    println!("RESULT lu {name} n={n}: supernodal vs faer FORCE_SUPERNODAL {bits}/{n} bit-identical, worst rel {worst:.3e}");
    assert_eq!(bits, n, "{name}: not bit-identical to faer's supernodal LU");

    // default (AUTO) on both sides: same path, bit for bit.
    let lua = SparseLu::factorize(&ours).unwrap();
    let auto_sym = factorize_symbolic_lu(theirs.symbolic(), Default::default()).unwrap();
    let auto_simplicial = format!("{auto_sym:?}").contains("Simplicial");
    assert_eq!(lua.is_supernodal(), !auto_simplicial, "{name}: path decision differs from faer's");
    let xo = lua.solve(&b).unwrap();
    let xa = theirs.sp_lu().unwrap().solve(col(&b));
    let xa: Vec<f64> = (0..n).map(|i| xa[(i, 0)]).collect();
    let (bits, worst) = max_rel_diff(&xo, &xa);
    println!(
        "RESULT lu {name} n={n}: AUTO picks {} on both; {bits}/{n} bit-identical, worst rel {worst:.3e}",
        if auto_simplicial { "simplicial" } else { "SUPERNODAL" }
    );
    assert_eq!(bits, n, "{name}: AUTO not bit-identical to faer's sp_lu");
}

fn faer_lu_forced(
    theirs: &SparseColMat<usize, f64>,
    b: &[f64],
    thr: SupernodalThreshold,
    expect_perm: Option<&[usize]>,
    name: &str,
) -> Vec<f64> {
    let n = b.len();
    let sym = factorize_symbolic_lu(
        theirs.symbolic(),
        LuSymbolicParams {
            supernodal_flop_ratio_threshold: thr,
            ..Default::default()
        },
    )
    .unwrap();
    if let Some(p) = expect_perm {
        assert_eq!(p, sym.col_perm().arrays().0, "{name}: COLAMD permutation");
    }
    let mut num = NumericLu::new();
    let mut mem =
        MemBuffer::new(sym.factorize_numeric_lu_scratch::<f64>(Par::Seq, Default::default()));
    let flu = sym
        .factorize_numeric_lu(
            &mut num,
            theirs.as_ref(),
            Par::Seq,
            MemStack::new(&mut mem),
            Default::default(),
        )
        .unwrap();
    let mut xb = col(b);
    let mut mem2 = MemBuffer::new(sym.solve_in_place_scratch::<f64>(1, Par::Seq));
    flu.solve_in_place_with_conj(Conj::No, xb.as_mut(), Par::Seq, MemStack::new(&mut mem2));
    (0..n).map(|i| xb[(i, 0)]).collect()
}

fn check_llt(name: &str, n: usize, t: &[(usize, usize, f64)]) {
    let ours = CscMatrix::try_from_triplets(n, n, t).unwrap();
    let theirs = faer_mat(n, t);
    let b = rhs(n);
    let llt = SparseLlt::factorize_lower(&ours).expect("petir LLT");
    let x = llt.solve(&b).unwrap();

    let fsym = faer::sparse::linalg::solvers::SymbolicLlt::try_new(theirs.symbolic(), Side::Lower)
        .unwrap();
    let faer_simplicial = format!("{fsym:?}").contains("Simplicial");
    assert_eq!(
        llt.faer_would_use_supernodal(),
        !faer_simplicial,
        "{name}: the ported supernodal decision disagrees with faer's"
    );
    assert_eq!(llt.is_supernodal(), !faer_simplicial, "{name}: path taken");
    let flu = theirs.sp_cholesky(Side::Lower).unwrap();
    let xf = flu.solve(col(&b));
    let xf: Vec<f64> = (0..n).map(|i| xf[(i, 0)]).collect();
    let (bits, worst) = max_rel_diff(&x, &xf);
    println!(
        "RESULT llt {name} n={n}: AUTO picks {} on both; {bits}/{n} bit-identical, worst rel {worst:.3e}",
        if faer_simplicial {
            "simplicial"
        } else {
            "SUPERNODAL"
        }
    );
    assert_eq!(bits, n, "{name}: AUTO not bit-identical to faer's sp_cholesky");

    // Both paths forced, both sides.
    for (kind, thr) in [
        (SymbolicKind::Simplicial, SupernodalThreshold::FORCE_SIMPLICIAL),
        (SymbolicKind::Supernodal, SupernodalThreshold::FORCE_SUPERNODAL),
    ] {
        let ours_f = SparseLlt::factorize_lower_with(&ours, kind).unwrap();
        assert_eq!(ours_f.is_supernodal(), kind == SymbolicKind::Supernodal);
        let xo = ours_f.solve(&b).unwrap();
        let sym = faer::sparse::linalg::cholesky::factorize_symbolic_cholesky(
            theirs.symbolic(),
            Side::Lower,
            Default::default(),
            faer::sparse::linalg::cholesky::CholeskySymbolicParams {
                supernodal_flop_ratio_threshold: thr,
                ..Default::default()
            },
        )
        .unwrap();
        let mut lval = vec![0.0f64; sym.len_val()];
        let mut mem = MemBuffer::new(sym.factorize_numeric_llt_scratch::<f64>(Par::Seq, Default::default()));
        let lref = sym
            .factorize_numeric_llt(
                &mut lval,
                theirs.as_ref(),
                Side::Lower,
                Default::default(),
                Par::Seq,
                MemStack::new(&mut mem),
                Default::default(),
            )
            .unwrap();
        let mut xb = col(&b);
        let mut mem2 = MemBuffer::new(sym.solve_in_place_scratch::<f64>(1, Par::Seq));
        lref.solve_in_place_with_conj(Conj::No, xb.as_mut(), Par::Seq, MemStack::new(&mut mem2));
        let xs: Vec<f64> = (0..n).map(|i| xb[(i, 0)]).collect();
        let (bits, worst) = max_rel_diff(&xo, &xs);
        println!("RESULT llt {name} n={n}: {kind:?} forced on both; {bits}/{n} bit-identical, worst rel {worst:.3e}");
        assert_eq!(bits, n, "{name}: {kind:?} not bit-identical to faer");
    }
}

/// Check the AMD permutation against faer's, through the public symbolic
/// factor.
fn check_amd(name: &str, n: usize, t: &[(usize, usize, f64)]) {
    let ours = CscMatrix::try_from_triplets(n, n, t).unwrap();
    let theirs = faer_mat(n, t);
    let llt = SparseLlt::factorize_lower(&ours).unwrap();
    let sym = faer::sparse::linalg::cholesky::factorize_symbolic_cholesky(
        theirs.symbolic(),
        Side::Lower,
        Default::default(),
        Default::default(),
    )
    .unwrap();
    let fperm = sym.perm().expect("AMD gives a permutation");
    assert_eq!(llt.perm(), fperm.arrays().0, "{name}: AMD permutation");
}

#[test]
fn lu_random_unsymmetric() {
    for (n, per_col, seed) in [(1, 0, 1), (5, 2, 2), (40, 3, 3), (300, 4, 4), (1500, 5, 5)] {
        check_lu(&format!("random{n}"), n, &random_unsym(n, per_col, seed));
    }
}

#[test]
fn lu_two_group_3d_diffusion() {
    for m in [12, 24] {
        let t = diffusion_3d(m);
        check_lu(&format!("diffusion{m}^3x2"), 2 * m * m * m, &t);
    }
}

#[test]
fn llt_grid_laplacians_and_random_spd() {
    for m in [30, 120] {
        let t = laplacian_2d_lower(m, 0.7);
        check_amd(&format!("laplace{m}^2"), m * m, &t);
        check_llt(&format!("laplace{m}^2"), m * m, &t);
    }
    for (n, per_col, seed) in [(1, 0, 7), (50, 3, 8), (2000, 4, 9)] {
        let t = random_spd_lower(n, per_col, seed);
        check_amd(&format!("spd{n}"), n, &t);
        check_llt(&format!("spd{n}"), n, &t);
    }
}

/// Factorisation time against faer's, one thread (`Par::Seq` in faer, PETIR
/// has no threads). Printed, not asserted: timing is machine-dependent. The
/// recorded numbers are in this file's module docs.
#[test]
fn factorisation_time_vs_faer() {
    use std::time::Instant;
    let best = |f: &mut dyn FnMut()| {
        let mut t = f64::INFINITY;
        for _ in 0..3 {
            let t0 = Instant::now();
            f();
            t = t.min(t0.elapsed().as_secs_f64());
        }
        t
    };
    // m = 24 (n = 27 648) was measured once, by hand, with `[12, 24]`: about
    // 5 minutes, too long for every run; its numbers are in the module docs.
    for m in [12usize] {
        let n = 2 * m * m * m;
        let t = diffusion_3d(m);
        let ours = CscMatrix::try_from_triplets(n, n, &t).unwrap();
        let theirs = faer_mat(n, &t);
        let tp_s = best(&mut || {
            SparseLu::factorize_with(&ours, SymbolicKind::Simplicial).unwrap();
        });
        let tp_n = best(&mut || {
            SparseLu::factorize_with(&ours, SymbolicKind::Supernodal).unwrap();
        });
        let tf = best(&mut || {
            theirs.sp_lu().unwrap();
        });
        println!("RESULT time lu diffusion{m}^3x2 n={n}: petir simplicial {tp_s:.3} s, petir supernodal {tp_n:.3} s, faer sp_lu (AUTO) {tf:.3} s");
    }
    let t = random_spd_lower(2000, 4, 9);
    let ours = CscMatrix::try_from_triplets(2000, 2000, &t).unwrap();
    let theirs = faer_mat(2000, &t);
    let tp = best(&mut || {
        SparseLlt::factorize_lower(&ours).unwrap();
    });
    let tf = best(&mut || {
        theirs.sp_cholesky(Side::Lower).unwrap();
    });
    println!("RESULT time llt spd2000: petir (AUTO, supernodal) {tp:.3} s, faer {tf:.3} s");
}
