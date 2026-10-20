// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **V&V — `petir::linalg::{dgebal, dgebak}` against Reference-LAPACK
//! 3.12.1 compiled and run** (gh:#998).
//!
//! # Methodology
//!
//! `reference-data/lapack/gebal_reference_driver.c` calls `DGEBAL` and
//! `DGEBAK` from the system `liblapack.so.3` (Arch `lapack 3.12.1-2`, the
//! Netlib reference build, with reference BLAS `blas 3.12.1-2`; GNU Octave
//! 11.3.0 links exactly these two here). `build_and_run.sh` also compiles
//! `dgebal.f`, `dgebak.f`, `dnrm2.f90`, `idamax.f` and the rest from the
//! v3.12.1 source tag with gfortran and checks the output is byte-identical
//! to the system library's, so the reference is the source that was ported.
//!
//! Cases (27): the fourteen input matrices of the Octave `expm` reference
//! (so `expm.m`'s balancing is checked on exactly its inputs), an
//! isolatable row and column (`isolate5`), a zero matrix, a `1 x 1`, a zero
//! row and column inside the active block (`holes4`, the `c == 0 || r == 0`
//! skip), entries from `2^-1000` to `2^1000` (`extreme4`, all three `dnrm2`
//! accumulators), entries at `2^+-960` (`guard3`, the `SFMIN`/`SFMAX`
//! guards), subnormals (`subnormal3`), and dense 6, 10, 25 matrices with
//! rows and columns scaled over 16 decades, each also with an isolated first
//! column and last row. Every case is run with every `JOB` (`N`, `P`, `S`,
//! `B`); `DGEBAK` back-transforms a fixed `n x 3` matrix for `SIDE = R` and
//! `L`.
//!
//! Pass criterion: every value bit-identical — `INFO`, `ILO`, `IHI`, every
//! `SCALE` entry, every entry of the balanced matrix and of both
//! back-transforms.
//!
//! # Prediction, written before the first run (2026-10-20)
//!
//! Bit-identical throughout: every operation is a power-of-two scaling, a
//! swap, or a comparison of `dnrm2` values, and `dnrm2` is ported line by
//! line, so any difference would be a porting defect (most likely in the
//! Fortran DO-loop bound semantics of the permutation search).
//!
//! # Results (2026-10-20)
//!
//! **26 024/26 024 values bit-identical** over 68 cases x 4 jobs (`SCALE`,
//! balanced matrix, both back-transforms), and all 544 `ILO`/`IHI` equal;
//! with `JOB = B`, 45 cases permute and 43 scale. The prediction held.
//! **Negative control:** giving the column search's inner loop the outer
//! loop's `K` (the plausible misreading of the Fortran) still passes on the
//! first 67 cases, so `loopbound6` was added — a 0/1 pattern found by
//! search on which the two readings give different `SCALE` — and with it
//! that variant fails (38 values differ) while the port passes.

mod linalg_gsl_common;

use linalg_gsl_common::{load_in, Score};
use petir::linalg::{dgebak, dgebal, BackSide, BalanceJob, Matrix};

const FILE: &str = "gebal-lapack-3.12.1-reference.txt";

/// Every output of `DGEBAL` and `DGEBAK`, every job, every case.
///
/// # Results (2026-10-20)
///
/// 26 024/26 024 bit-identical; see the module doc.
#[test]
fn gebal_and_gebak_match_reference_lapack_bit_for_bit() {
    let Some(cases) = load_in("lapack", FILE) else {
        eprintln!("skipping: {FILE} not present");
        return;
    };
    assert_eq!(cases.len(), 68);
    let mut ints = 0usize;
    let mut s = Score::default();
    let mut permuted = 0usize;
    let mut scaled = 0usize;
    for (name, case) in &cases {
        let a = case.matrix("A").unwrap();
        let n = a.rows();
        let vin = case.matrix("VIN").unwrap();
        for (job, c) in [
            (BalanceJob::None, 'N'),
            (BalanceJob::Permute, 'P'),
            (BalanceJob::Scale, 'S'),
            (BalanceJob::Both, 'B'),
        ] {
            let mut bal = a.clone();
            let b = dgebal(job, &mut bal).unwrap();
            assert_eq!(case.scalar(&format!("INFO_{c}")), Some(0.0), "{name} {c}");
            assert_eq!(
                (b.ilo as f64, b.ihi as f64),
                (
                    case.scalar(&format!("ILO_{c}")).unwrap(),
                    case.scalar(&format!("IHI_{c}")).unwrap()
                ),
                "{name} JOB={c} ilo/ihi"
            );
            ints += 2;
            if c == 'B' && (b.ilo != 1 || b.ihi != n) {
                permuted += 1;
            }
            if c == 'B' && b.scale[b.ilo - 1..b.ihi].iter().any(|&x| x != 1.0) {
                scaled += 1;
            }
            for (idx, v) in case.tag(&format!("SCALE_{c}")) {
                s.add(
                    || format!("{name} JOB={c} SCALE[{}]", idx[0]),
                    *v,
                    b.scale[idx[0]],
                );
            }
            for (idx, v) in case.tag(&format!("BAL_{c}")) {
                let (i, j) = (idx[0], idx[1]);
                s.add(
                    || format!("{name} JOB={c} BAL[{i}][{j}]"),
                    *v,
                    bal.get(i, j),
                );
            }
            for (side, sc) in [(BackSide::Right, 'R'), (BackSide::Left, 'L')] {
                let mut v: Matrix = vin.clone();
                dgebak(&b, side, &mut v).unwrap();
                assert_eq!(case.scalar(&format!("BAKINFO{sc}_{c}")), Some(0.0));
                for (idx, val) in case.tag(&format!("BAK{sc}_{c}")) {
                    let (i, j) = (idx[0], idx[1]);
                    s.add(
                        || format!("{name} JOB={c} BAK{sc}[{i}][{j}]"),
                        *val,
                        v.get(i, j),
                    );
                }
            }
        }
    }
    eprintln!("{}", s.summary("dgebal/dgebak vs Reference-LAPACK 3.12.1"));
    eprintln!(
        "{ints} ilo/ihi values equal; JOB=B permuted {permuted} cases, scaled {scaled} cases"
    );
    assert_eq!(s.identical, s.compared, "{}", s.summary("dgebal/dgebak"));
    // The cases must actually exercise both halves of the routine.
    assert!(permuted >= 5 && scaled >= 5, "{permuted} {scaled}");
}
