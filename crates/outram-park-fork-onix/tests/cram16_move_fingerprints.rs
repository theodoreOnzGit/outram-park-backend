//! **Bit-identity gate for moving ONIX's CRAM16 into PETIR** (GitHub issue
//! #999, row 1; maintainer direction 2026-10-20: "The cram from Onix can go
//! to petir and attribute onix").
//!
//! # Methodology
//!
//! The move relocates pure arithmetic: the order-16 partial-fraction CRAM
//! (`onix/salameche/cram.py:6-59`) and its dense complex Gaussian
//! elimination. The burnup-matrix assembly, the driver and
//! `clamp_nonnegative` stay in this crate. The move must not change a single
//! bit of any result.
//!
//! Each case runs a fixed calculation through this crate's ordinary public
//! API (`cram16`, `DepletionSystem::deplete`, `deplete_multi`) and folds
//! every output `f64` (`to_bits`) into a 64-bit FNV-1a fingerprint, which is
//! pinned. The pins were printed by THIS FILE on the commit before the move
//! (`ONIX_FP_PRINT=1 cargo test --release -p outram-park-fork-onix --test
//! cram16_move_fingerprints -- --nocapture`), then the move was made and the
//! file re-run unchanged. A fingerprint is a hash, so a mismatch says only
//! that something moved; rerun with `ONIX_FP_PRINT=1` to see which case.
//!
//! The CRAM path is rational arithmetic plus one `hypot` per pivot candidate
//! (the pivot search compares `|m[r][c]|`). Before the move that `hypot` was
//! `num_complex::Complex64::norm`, i.e. the platform libm's `hypot`; after it,
//! PETIR's correctly rounded CORE-MATH port. The two can differ by an ulp,
//! which could change a pivot choice only on a near-tie. These pins are what
//! measures whether that happened on these cases.
//!
//! The cases are every matrix the crate's own tests use (`vv_bateman.rs`,
//! `cram.rs` unit tests) plus one stiff, non-triangular 10-nuclide burnup
//! matrix (decay constants from 1e-13 to 2e1 /s, a capture loop and an
//! (n,2n) back-edge, so the elimination actually pivots and fills in).
//!
//! # Results
//!
//! Pre-move pins printed 2026-10-20 on `9ba00c1756` (x86_64 Linux, glibc).
//!
//! | case | pre-move | post-move |
//! |---|---|---|
//! | `single_nuclide` | `0x11bbacd8f70609dd` | identical |
//! | `stiff_diagonal` | `0x8c7da07f988c5126` | identical |
//! | `abc_chain` | `0xa7a83da186009ae6` | identical |
//! | `secular_equilibrium` | `0xd8bca4605bfea942` | identical |
//! | `transmutation` | `0x855fae983da14705` | identical |
//! | `fission_yield` | `0x47a5467c82448c01` | identical |
//! | `stiff_burnup_10` | `0x792d981a9650a5e7` | identical |
//!
//! **Result (2026-10-20): all seven cases bit-identical after the move**, as
//! predicted before the run (same operations in the same order; the `hypot`
//! swap could only matter on a pivot near-tie, and none occurred here).
//! Suites: `cargo test --release -p outram-park-fork-onix -p petir --lib
//! --tests`: 806 passed before; 813 after, i.e. this file, four PETIR
//! `onix_cram` tests and one onix error-mapping test, plus
//! `gsl_eigen_nonsymm_code_to_code`, which failed in the before-run and
//! passed in the after-run and three reruns without this change touching it.
//! `verbatim_provenance` failed in both runs and predates this change.

use outram_park_fork_onix::{
    cram16, BurnupMatrix, DecayData, DecayMode, DepletionSystem, FissionYields, Nuclide,
    ReactionChannel, ReactionRates,
};

/// FNV-1a over the bit patterns of `xs`.
fn fnv(state: &mut u64, xs: &[f64]) {
    for x in xs {
        for b in x.to_bits().to_le_bytes() {
            *state ^= u64::from(b);
            *state = state.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
}

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;

fn fp(xs: &[f64]) -> u64 {
    let mut h = FNV_OFFSET;
    fnv(&mut h, xs);
    h
}

fn abc_system(la: f64, lb: f64) -> (DepletionSystem, Nuclide) {
    let a = Nuclide::new(50, 100, 0);
    let b = Nuclide::new(51, 100, 0);
    let c = Nuclide::new(52, 100, 0);
    let mut sys = DepletionSystem::new();
    sys.add_nuclide(
        a,
        DecayData::single_mode(la, DecayMode::BetaMinus),
        ReactionRates::none(),
        FissionYields::empty(),
    )
    .unwrap();
    sys.add_nuclide(
        b,
        DecayData::single_mode(lb, DecayMode::BetaMinus),
        ReactionRates::none(),
        FissionYields::empty(),
    )
    .unwrap();
    sys.add_nuclide(
        c,
        DecayData::stable(),
        ReactionRates::none(),
        FissionYields::empty(),
    )
    .unwrap();
    (sys, a)
}

/// A stiff, non-triangular 10x10 burnup matrix (`1/s`). Not physical data:
/// magnitudes chosen to span the stiffness range of a real chain.
fn stiff_matrix() -> BurnupMatrix {
    let n = 10;
    let mut m = BurnupMatrix::zeros(n);
    // Loss rates on the diagonal, 1e-13 .. 2e1 /s.
    let loss = [
        -3.1e-13, -2.9e-5, -4.9e-4, -3.4e-6, -1.0e-9, -2.7e-9, -1.5e-9, -2.1e-5, -2.0e1, -7.3e-3,
    ];
    for (i, l) in loss.iter().enumerate() {
        m.set(i, i, *l);
    }
    // Chain edges parent j -> daughter i.
    let edges = [
        (1, 0, 2.5e-13),
        (2, 1, 2.9e-5),
        (3, 2, 4.9e-4),
        (4, 3, 3.4e-6),
        (5, 4, 6.0e-10),
        (6, 5, 2.0e-9),
        (4, 5, 7.0e-10), // (n,2n) back-edge 5 -> 4
        (7, 6, 1.4e-9),
        (8, 7, 2.1e-5),
        (9, 8, 2.0e1),
        (2, 9, 7.0e-3), // capture loop 9 -> 2
        (0, 6, 1.0e-10),
    ];
    for (i, j, v) in edges {
        m.add(i, j, v);
    }
    m
}

fn cases() -> Vec<(&'static str, u64)> {
    let mut out = Vec::new();

    // cram.rs unit test.
    let mut a = BurnupMatrix::zeros(1);
    a.set(0, 0, -1e-3);
    out.push(("single_nuclide", fp(&cram16(&a, 500.0, &[1.0]).unwrap())));

    // vv_bateman::cram16_stiff_diagonal.
    let mut m = BurnupMatrix::zeros(2);
    m.set(0, 0, -1.0);
    m.set(1, 1, -1e-6);
    out.push(("stiff_diagonal", fp(&cram16(&m, 5.0, &[1.0, 1.0]).unwrap())));

    // vv_bateman decay chain cases.
    let (sys, a) = abc_system(1e-2, 3e-3);
    let n0 = sys.inventory_vector(&[(a, 1.0)]).unwrap();
    let mut h = FNV_OFFSET;
    for dt in [200.0, 300.0, 500.0] {
        fnv(&mut h, &sys.deplete(&n0, dt).unwrap());
    }
    fnv(
        &mut h,
        &sys.deplete_multi(&n0, &[100.0, 100.0, 100.0]).unwrap(),
    );
    out.push(("abc_chain", h));

    let (sys, a) = abc_system(1e-8, 1e-2);
    let n0 = sys.inventory_vector(&[(a, 1.0)]).unwrap();
    out.push(("secular_equilibrium", fp(&sys.deplete(&n0, 1e4).unwrap())));

    // vv_bateman::burnup_step_transmutation.
    let pa = Nuclide::new(50, 100, 0);
    let pb = Nuclide::new(50, 101, 0);
    let mut sys = DepletionSystem::new();
    sys.add_nuclide(
        pa,
        DecayData::stable(),
        ReactionRates {
            channels: vec![(ReactionChannel::NGamma, 1e-4)],
        },
        FissionYields::empty(),
    )
    .unwrap();
    sys.add_nuclide(
        pb,
        DecayData::stable(),
        ReactionRates::none(),
        FissionYields::empty(),
    )
    .unwrap();
    let n0 = sys.inventory_vector(&[(pa, 1.0)]).unwrap();
    out.push(("transmutation", fp(&sys.deplete(&n0, 3000.0).unwrap())));

    // vv_bateman::fission_yield_assembly.
    let f = Nuclide::new(92, 235, 0);
    let p1 = Nuclide::new(54, 135, 0);
    let p2 = Nuclide::new(55, 137, 0);
    let mut sys = DepletionSystem::new();
    sys.add_nuclide(
        f,
        DecayData::stable(),
        ReactionRates {
            channels: vec![(ReactionChannel::Fission, 1e-5)],
        },
        FissionYields {
            products: vec![(p1, 0.06), (p2, 0.02)],
        },
    )
    .unwrap();
    for p in [p1, p2] {
        sys.add_nuclide(
            p,
            DecayData::stable(),
            ReactionRates::none(),
            FissionYields::empty(),
        )
        .unwrap();
    }
    let n0 = sys.inventory_vector(&[(f, 1.0)]).unwrap();
    out.push(("fission_yield", fp(&sys.deplete(&n0, 4000.0).unwrap())));

    // Stiff non-triangular matrix at four steps, 1 s .. 1 year.
    let s = stiff_matrix();
    let n0 = [1.0, 0.5, 1e-3, 0.0, 2.0, 1e-6, 0.3, 0.0, 1e-9, 0.1];
    let mut h = FNV_OFFSET;
    for dt in [1.0, 3600.0, 86400.0 * 30.0, 86400.0 * 365.25] {
        fnv(&mut h, &cram16(&s, dt, &n0).unwrap());
    }
    out.push(("stiff_burnup_10", h));

    out
}

/// Pre-move fingerprints (see the module docs).
const PINS: [(&str, u64); 7] = [
    ("single_nuclide", 0x11bbacd8f70609dd),
    ("stiff_diagonal", 0x8c7da07f988c5126),
    ("abc_chain", 0xa7a83da186009ae6),
    ("secular_equilibrium", 0xd8bca4605bfea942),
    ("transmutation", 0x855fae983da14705),
    ("fission_yield", 0x47a5467c82448c01),
    ("stiff_burnup_10", 0x792d981a9650a5e7),
];

/// Every case reproduces its pre-move fingerprint exactly.
#[test]
fn cram16_move_is_bit_identical() {
    let got = cases();
    if std::env::var_os("ONIX_FP_PRINT").is_some() {
        for (name, h) in &got {
            println!("(\"{name}\", 0x{h:016x}),");
        }
    }
    assert_eq!(got.len(), PINS.len());
    for ((name, h), (pname, pin)) in got.iter().zip(PINS.iter()) {
        assert_eq!(name, pname);
        assert_eq!(
            *h, *pin,
            "case {name}: fingerprint 0x{h:016x} != pin 0x{pin:016x}"
        );
    }
}
