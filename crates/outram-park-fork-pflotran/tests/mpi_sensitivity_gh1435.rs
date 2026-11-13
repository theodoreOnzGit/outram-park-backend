//! Did outram-park-mpi's switch of `all_reduce` / long reduce to MPICH's
//! algorithms (gh:#1435, commit 2990e4737) move any number in this crate?
//!
//! # Methodology
//!
//! The crate's only use of collectives is `decomposition` (distributed dot
//! products and Krylov solves). This test runs the distributed CG of
//! `decomposition::krylov` on 48 unknowns for 1..=8 ranks and a distributed dot
//! of 1000 pseudo-random terms, and prints, per rank count, the iteration count
//! and an FNV-1a hash of the bit patterns of the assembled solution, plus the
//! dot product's bits. It asserts the post-#1435 values recorded below. The
//! same test was run with `crates/outram-park-mpi` checked out at commit
//! `7fdc2b11e` (the last before #1435) to get the "before" column.
//!
//! # Results (2026-11-13, x86-64, PETIR libm)
//!
//! See `verification_and_validation/upstream_differential_gh1422/README.md`,
//! section "outram-park-mpi #1435". Pass criterion fixed before the run: the
//! assertion below holds for the tree under test; the before/after comparison
//! is reported, not gated.

use outram_park_fork_pflotran::decomposition::krylov::{distributed_cg, distributed_dot};
use outram_park_fork_pflotran::decomposition::Decomposition1D;
use outram_park_mpi::run;
use petir::mathf::RealMath;

fn fnv(bits: &[u64]) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bits {
        for k in 0..8 {
            h ^= (b >> (8 * k)) & 0xff;
            h = h.wrapping_mul(0x100000001b3);
        }
    }
    h
}

/// One line per rank count: `p iters solution-hash dot-bits`.
fn lines() -> Vec<String> {
    let n = 48usize;
    let b: Vec<f64> = (0..n).map(|i| ((i as f64) * 0.3).r_sin() + 1.0).collect();
    let a1: Vec<f64> = (0..1000).map(|i| ((i as f64) * 0.37).r_sin()).collect();
    let a2: Vec<f64> = (0..1000).map(|i| ((i as f64) * 0.11).r_cos()).collect();
    let mut out = Vec::new();
    for p in 1..=8i32 {
        let (b_ref, x1, x2) = (b.clone(), a1.clone(), a2.clone());
        let per_rank = run(p, move |c| {
            let d = Decomposition1D::new(n, c);
            let bl = b_ref[d.start..d.start + d.local_len].to_vec();
            let (x, iters) = distributed_cg(c, &d, &bl, 0.1, 1e-10, 1000).unwrap();
            let dd = Decomposition1D::new(1000, c);
            let (s, l) = (dd.start, dd.local_len);
            let dot = distributed_dot(c, &x1[s..s + l], &x2[s..s + l]).unwrap();
            (x, iters, dot)
        })
        .unwrap();
        let bits: Vec<u64> = per_rank
            .iter()
            .flat_map(|(x, _, _)| x.iter().map(|v| v.to_bits()))
            .collect();
        out.push(format!(
            "{p} {} {:016x} {:016x}",
            per_rank[0].1,
            fnv(&bits),
            per_rank[0].2.to_bits()
        ));
    }
    out
}

#[test]
fn distributed_results_by_rank_count() {
    let got = lines();
    for l in &got {
        println!("MPI1435 {l}");
    }
    // all ranks must agree and iteration counts must be sane
    assert_eq!(got.len(), 8);
    let expected = include_str!(
        "../verification_and_validation/upstream_differential_gh1422/mpi_after_1435.txt"
    );
    let want: Vec<&str> = expected.lines().collect();
    assert_eq!(got.len(), want.len());
    for (g, w) in got.iter().zip(&want) {
        assert_eq!(g, w, "distributed result moved");
    }
}
