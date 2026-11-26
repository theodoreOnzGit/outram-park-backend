// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! gh:#1790 (stage 1 of the TetGen port, epic gh:#1789): the Rust port of
//! TetGen 1.6.0's `predicates.cxx` (`outram_park_fork_cfmesh::tetgen::predicates`)
//! against upstream, **bit for bit**.
//!
//! # Methodology
//!
//! Reference: upstream `predicates.cxx` (libigl/tetgen `e05aca7df74e`, TetGen
//! 1.6.0), compiled unmodified by our C harness
//! `verification_and_validation/tetgen_predicates_gh1790/gen_vectors.cxx`
//! (g++ 13.3.0, x86_64, `-O2 -ffp-contract=off -fno-fast-math`; the same file
//! at `-O0`, upstream's own flag for the predicates, and the stage-counting
//! build write a byte-identical file). The harness writes the inputs and the
//! results' bit patterns to
//! `test_data/tetgen_predicates/predicates_vectors.bin.gz`; this test reads
//! only that file (no compiler needed, so it runs on Android).
//!
//! - `exactinit` on 12 extent triples and option pairs: all 19 constants and
//!   flags compared bit for bit.
//! - 6000 four-point cases: `orient3d` in three modes (static filter on, the
//!   default; filter off, `-X1`; inexact, `-X`; extents 2.7, 0.9, 1.3),
//!   `orient3dfast`, `orient3dexact`, `orient2dexact`.
//! - 6000 five-point cases with heights: `insphere` and `orient4d` in the
//!   three modes, `insphereexact`, `orient4dexact`.
//! - Ten input categories of 600 each per family: uniform random, full
//!   random mantissas, exactly coplanar / cospherical integer lattices, exact
//!   degeneracies with inexact differences (points on the plane x = y), nearly
//!   degenerate (floating-point construction, non-dyadic translation, unit
//!   sphere by normalisation), 1-ulp perturbations of the degenerate sets,
//!   large (up to 2^200) and tiny (down to 2^-340) magnitudes, repeated and
//!   collinear points, heights on a hyperplane.
//!
//! Pass criterion: every result `to_bits()`-equal to upstream's (no
//! tolerance).
//!
//! # Results (2026-11-26)
//!
//! **84 000 of 84 000 predicate results bit-identical (14 columns of 6000), and 12 of 12
//! `exactinit` configurations (228 constants).** Per predicate and mode,
//! identical / cases:
//!
//! | predicate | filter on | filter off | inexact |
//! |---|---|---|---|
//! | `orient3d` | 6000/6000 | 6000/6000 | 6000/6000 |
//! | `insphere` | 6000/6000 | 6000/6000 | 6000/6000 |
//! | `orient4d` | 6000/6000 | 6000/6000 | 6000/6000 |
//! | `orient3dfast`, `orient3dexact`, `orient2dexact` | 6000/6000 each | (mode-independent) | |
//! | `insphereexact`, `orient4dexact` | 6000/6000 each | (mode-independent) | |
//!
//! Every return site of `orient3d`, `orient3dadapt`, `insphere`,
//! `insphereadapt`, `orient4d` and `orient4dadapt` (static filter, stage A,
//! B, all-tails-zero, C and the exact tail) is reached in the modes where it
//! can be; the counts are in the V&V README. No difference was found, so
//! there is none to explain.
use outram_park_fork_cfmesh::tetgen::predicates::Predicates;
use std::path::PathBuf;

/// Inflate a `gzip -n` file (10-byte header, 8-byte trailer).
fn gunzip(bytes: &[u8]) -> Vec<u8> {
    assert!(
        bytes.len() > 18 && bytes[0] == 0x1f && bytes[1] == 0x8b,
        "not gzip"
    );
    assert_eq!(bytes[3], 0, "gzip -n expected (no name/extra flags)");
    miniz_oxide::inflate::decompress_to_vec(&bytes[10..bytes.len() - 8]).expect("inflate")
}

/// A little-endian cursor over the vector file.
struct Reader {
    b: Vec<u8>,
    i: usize,
}

impl Reader {
    fn take<const N: usize>(&mut self) -> [u8; N] {
        let out: [u8; N] = self.b[self.i..self.i + N].try_into().unwrap();
        self.i += N;
        out
    }
    fn u8(&mut self) -> u8 {
        self.take::<1>()[0]
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take())
    }
    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(self.take())
    }
    fn f64(&mut self) -> f64 {
        f64::from_le_bytes(self.take())
    }
    fn point(&mut self) -> [f64; 3] {
        [self.f64(), self.f64(), self.f64()]
    }
}

/// Count of identical results and of cases, per (predicate, mode) column.
#[derive(Default, Clone, Copy)]
struct Tally {
    same: usize,
    total: usize,
}

fn check(t: &mut Tally, got: f64, want: u64, what: &str, case: usize, misses: &mut Vec<String>) {
    t.total += 1;
    if got.to_bits() == want {
        t.same += 1;
    } else if misses.len() < 20 {
        misses.push(format!(
            "{what} case {case}: port {got:e} ({:#018x}), upstream {:e} ({want:#018x})",
            got.to_bits(),
            f64::from_bits(want)
        ));
    }
}

#[test]
fn tetgen_predicates_are_bit_identical_to_upstream() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test_data/tetgen_predicates/predicates_vectors.bin.gz");
    let mut r = Reader {
        b: gunzip(&std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))),
        i: 0,
    };
    assert_eq!(&r.take::<8>(), b"TGPRED01");

    // exactinit: every constant and flag.
    let nconfig = r.u32() as usize;
    let mut configs = Vec::new();
    let mut const_same = 0;
    for c in 0..nconfig {
        let noexact = r.u8() != 0;
        let nofilter = r.u8() != 0;
        let (mx, my, mz) = (r.f64(), r.f64(), r.f64());
        let p = Predicates::exactinit(noexact, nofilter, mx, my, mz);
        let mine = [
            p.epsilon,
            p.splitter,
            p.resulterrbound,
            p.ccwerrbound_a,
            p.ccwerrbound_b,
            p.ccwerrbound_c,
            p.o3derrbound_a,
            p.o3derrbound_b,
            p.o3derrbound_c,
            p.iccerrbound_a,
            p.iccerrbound_b,
            p.iccerrbound_c,
            p.isperrbound_a,
            p.isperrbound_b,
            p.isperrbound_c,
            p.o3dstaticfilter,
            p.ispstaticfilter,
            p.use_inexact_arith as i32 as f64,
            p.use_static_filter as i32 as f64,
        ];
        for (k, m) in mine.iter().enumerate() {
            let want = r.f64();
            assert_eq!(
                m.to_bits(),
                want.to_bits(),
                "exactinit config {c}, constant {k}: port {m:e}, upstream {want:e}"
            );
            const_same += 1;
        }
        configs.push(p);
    }
    let neval = r.u32() as usize;
    assert_eq!(neval, 3);
    let modes = &configs[..neval];

    let mut misses = Vec::new();
    // Columns: orient3d x3, orient3dfast, orient3dexact, orient2dexact.
    let mut t4 = [Tally::default(); 6];
    let n4 = r.u32() as usize;
    for i in 0..n4 {
        let _cat = r.u8();
        let (pa, pb, pc, pd) = (r.point(), r.point(), r.point(), r.point());
        let want: Vec<u64> = (0..6).map(|_| r.u64()).collect();
        for (m, p) in modes.iter().enumerate() {
            check(
                &mut t4[m],
                p.orient3d(&pa, &pb, &pc, &pd),
                want[m],
                "orient3d",
                i,
                &mut misses,
            );
        }
        let p = &modes[0];
        check(
            &mut t4[3],
            p.orient3dfast(&pa, &pb, &pc, &pd),
            want[3],
            "orient3dfast",
            i,
            &mut misses,
        );
        check(
            &mut t4[4],
            p.orient3dexact(&pa, &pb, &pc, &pd),
            want[4],
            "orient3dexact",
            i,
            &mut misses,
        );
        check(
            &mut t4[5],
            p.orient2dexact(&pa, &pb, &pc),
            want[5],
            "orient2dexact",
            i,
            &mut misses,
        );
    }
    // Columns: insphere x3, insphereexact, orient4d x3, orient4dexact.
    let mut t5 = [Tally::default(); 8];
    let n5 = r.u32() as usize;
    for i in 0..n5 {
        let _cat = r.u8();
        let (pa, pb, pc, pd, pe) = (r.point(), r.point(), r.point(), r.point(), r.point());
        let h: Vec<f64> = (0..5).map(|_| r.f64()).collect();
        let want: Vec<u64> = (0..8).map(|_| r.u64()).collect();
        for (m, p) in modes.iter().enumerate() {
            check(
                &mut t5[m],
                p.insphere(&pa, &pb, &pc, &pd, &pe),
                want[m],
                "insphere",
                i,
                &mut misses,
            );
            let o = p.orient4d(&pa, &pb, &pc, &pd, &pe, h[0], h[1], h[2], h[3], h[4]);
            check(&mut t5[4 + m], o, want[4 + m], "orient4d", i, &mut misses);
        }
        let p = &modes[0];
        check(
            &mut t5[3],
            p.insphereexact(&pa, &pb, &pc, &pd, &pe),
            want[3],
            "insphereexact",
            i,
            &mut misses,
        );
        let o = p.orient4dexact(&pa, &pb, &pc, &pd, &pe, h[0], h[1], h[2], h[3], h[4]);
        check(&mut t5[7], o, want[7], "orient4dexact", i, &mut misses);
    }
    assert_eq!(r.i, r.b.len(), "trailing bytes in the vector file");

    let names4 = [
        "orient3d filter-on",
        "orient3d filter-off",
        "orient3d inexact",
        "orient3dfast",
        "orient3dexact",
        "orient2dexact",
    ];
    let names5 = [
        "insphere filter-on",
        "insphere filter-off",
        "insphere inexact",
        "insphereexact",
        "orient4d filter-on",
        "orient4d filter-off",
        "orient4d inexact",
        "orient4dexact",
    ];
    println!("exactinit: {nconfig} configurations, {const_same} constants bit-identical");
    let (mut same, mut total) = (0, 0);
    for (n, t) in names4
        .iter()
        .zip(t4.iter())
        .chain(names5.iter().zip(t5.iter()))
    {
        println!("{n:22} {:6} / {:6} bit-identical", t.same, t.total);
        same += t.same;
        total += t.total;
    }
    println!("total {same} / {total}");
    assert_eq!(n4, 6000);
    assert_eq!(n5, 6000);
    assert!(
        misses.is_empty(),
        "{} of {total} differ; first ones:\n{}",
        total - same,
        misses.join("\n")
    );
}
