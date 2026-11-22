//! Function-level differential tests of the plot-file helpers `ascle`, `ascll`
//! and `mtname` against NJOY2016.80's own Fortran (gh:#1369).
//!
//! # Methodology
//!
//! `verification_and_validation/aplots/extract.sh` copies the three upstream
//! routines unchanged (after checking the SHA-256 of `acefc.f90` and
//! `acecm.f90`) into a module, builds `scale_driver.f90` with `gfortran -O3`
//! (13.3, the flags of the NJOY build) and records every input and output as
//! IEEE-754 hex bit patterns in `reference.txt`:
//!
//! - 40 hand-picked `ascle` cases, one or more per branch (unreasonable
//!   arguments, zero ends, `|zbar| >= 1000`, `|zbar| <= 1e-3`, the narrow-span
//!   branch, `p < 1`, `p >= 1e4`, and the `p` thresholds 1.99, 2.01 and 4.99),
//!   plus 3 000 pseudo-random ones;
//! - 3 000 `ascll` cases over 24 decades, with equal ends, ends 1e-7 apart and
//!   exact decade/doubled-decade ends;
//! - `mtname` for every MT 1..1000 whose table entry is defined upstream (for
//!   `iverf >= 6` MT 501..599 index `hndf` out of range: undefined, skipped),
//!   six incident particles and the four `iverf` values (-1, 4, 5, 6).
//!
//! The Rust side reads the *inputs* back from the reference and compares the
//! outputs bit for bit. The expectation, written before the first run:
//! `ascle` is bit-identical everywhere (only `+ - * /`, `abs`, truncation);
//! `ascll` is bit-identical except possibly where `10**x` / `log10` differ in
//! the last bit between PETIR's `libm` and glibc (the maintainer's decision of
//! 2026-11-11, gh:#1455, keeps PETIR's maths); `mtname` is identical.
//!
//! # Result
//!
//! 2026-11-12 (gfortran 13.3, glibc 2.39, Rust toolchain of `rust-toolchain.toml`):
//! `ascle` 3 040 of 3 040 bit-identical; `ascll` 3 000 of 3 000 bit-identical
//! (no PETIR-against-glibc difference showed); `mtname` 21 600 of 21 600 names
//! identical. The prediction held in full. See
//! `verification_and_validation/aplots/README.md`.

use njoy_outram_park_fork::acer::aplots::scale::{ascle, ascll, mtname};

const REFERENCE: &str = include_str!("../verification_and_validation/aplots/reference.txt");

fn hex(s: &str) -> f64 {
    f64::from_bits(u64::from_str_radix(s, 16).unwrap())
}

/// `ascle`: 3 040 cases, every output word bit-identical.
#[test]
fn ascle_is_bit_identical_to_upstream() {
    let (mut n, mut same) = (0, 0);
    for l in REFERENCE.lines().filter(|l| l.starts_with("ascle ")) {
        let t: Vec<&str> = l.split_whitespace().collect();
        let m: i32 = t[1].parse().unwrap();
        let (z1, z2) = (hex(t[2]), hex(t[3]));
        let (e1, e2) = (hex(t[4]), hex(t[5]));
        let (emaj, emin): (i64, i64) = (t[6].parse().unwrap(), t[7].parse().unwrap());
        let (g1, g2, gmaj, gmin) = ascle(m, z1, z2);
        n += 1;
        if g1.to_bits() == e1.to_bits() && g2.to_bits() == e2.to_bits() && gmaj == emaj && gmin == emin {
            same += 1;
        } else {
            panic!("ascle({m}, {z1:e}, {z2:e}): got ({g1:e}, {g2:e}, {gmaj}, {gmin}), upstream ({e1:e}, {e2:e}, {emaj}, {emin})");
        }
    }
    println!("ascle: {same} of {n} bit-identical");
    assert_eq!(n, 3040);
    assert_eq!(same, n);
}

/// `ascll`: 3 000 cases, every output word bit-identical (the `10**x` and
/// `log10` of PETIR agree with glibc on all of them).
#[test]
fn ascll_is_bit_identical_to_upstream() {
    let (mut n, mut same, mut worst) = (0, 0, 0.0f64);
    for l in REFERENCE.lines().filter(|l| l.starts_with("ascll ")) {
        let t: Vec<&str> = l.split_whitespace().collect();
        let (z1, z2) = (hex(t[1]), hex(t[2]));
        let (e1, e2) = (hex(t[3]), hex(t[4]));
        let (g1, g2) = ascll(z1, z2);
        n += 1;
        if g1.to_bits() == e1.to_bits() && g2.to_bits() == e2.to_bits() {
            same += 1;
        } else {
            worst = worst.max(((g1 - e1) / e1).abs()).max(((g2 - e2) / e2).abs());
        }
    }
    println!("ascll: {same} of {n} bit-identical, worst relative difference {worst:e}");
    assert_eq!(n, 3000);
    assert_eq!(same, n, "worst relative difference {worst:e}");
}

/// `mtname`: every defined name, all identical.
#[test]
fn mtname_is_identical_to_upstream() {
    let mut n = 0;
    for l in REFERENCE.lines().filter(|l| l.starts_with("mtname ")) {
        let open = l.find('[').unwrap();
        let close = l.rfind(']').unwrap();
        let expect = &l[open + 1..close];
        let t: Vec<&str> = l[..open].split_whitespace().collect();
        let (iverf, izai, mt): (i32, i64, i64) = (t[1].parse().unwrap(), t[2].parse().unwrap(), t[3].parse().unwrap());
        let got = mtname(mt, izai, iverf);
        assert_eq!(got, expect, "mtname({mt}, izai {izai}, iverf {iverf})");
        n += 1;
    }
    println!("mtname: {n} names identical");
    assert!(n > 20000);
}

/// **Law 67 (`getl7` and `aplodd`'s law-67 page): reached, not compared.** No
/// NJOY ACE file here carries a law 67, so there is no upstream plot to diff.
/// This builds a 45-word table by hand (two incident energies, two cosines,
/// two outgoing-energy points each) and checks that the page is produced with
/// the structure upstream writes: `(n,2n)` neutron emission page, `-1 2/`, a
/// `1/` curve-set record, one energy record and one `ep, f0` pair list per
/// incident energy. It is a smoke test of a translation that is **unverified**
/// against upstream (see `verification_and_validation/aplots/README.md`).
#[test]
fn law_67_page_is_written() {
    use njoy_outram_park_fork::acer::aplots::{aplots, AceTable};
    let mut x = vec![0.0f64; 45];
    let set = |x: &mut Vec<f64>, i: usize, v: f64| x[i - 1] = v;
    // ESZ: energy, total, absorption, elastic, heating (nes = 2).
    for (i, v) in [1.0e-5, 20.0, 1.0, 1.0, 0.1, 0.1, 0.5, 0.5, 0.01, 0.01].into_iter().enumerate() {
        set(&mut x, i + 1, v);
    }
    set(&mut x, 11, 16.0); // MTR
    set(&mut x, 13, 1.0); // TYR
    set(&mut x, 14, 1.0); // LDLW
    set(&mut x, 16, 67.0); // law
    set(&mut x, 21, 3.0); // NE
    for (i, v) in [1.0, 10.0, 20.0, 14.0, 14.0, 14.0].into_iter().enumerate() {
        set(&mut x, 22 + i, v); // energies, then locators (a7 index 14)
    }
    for (i, v) in [2.0, 2.0, -1.0, 1.0, 20.0, 26.0].into_iter().enumerate() {
        set(&mut x, 28 + i, v); // intmu, nmu, u1, u2, loc1, loc2
    }
    for (i, v) in [2.0, 2.0, 0.1, 5.0, 0.5, 0.2, 2.0, 2.0, 0.1, 5.0, 0.4, 0.1].into_iter().enumerate() {
        set(&mut x, 34 + i, v); // per cosine: intep, npep, ep1, ep2, f1, f2
    }
    let mut nxs = [0i64; 16];
    nxs[0] = 45;
    nxs[2] = 2; // nes
    nxs[4] = 1; // nr
    let mut jxs = [0i64; 32];
    jxs[0] = 1; // esz
    jxs[2] = 11; // mtr
    jxs[4] = 13; // tyr
    jxs[9] = 14; // ldlw
    jxs[10] = 15; // dlw
    let table = AceTable::new(&nxs, &jxs, x, 8.934, 1, 1.0, -1);
    let text = aplots(&table, "synthetic law 67").unwrap();
    assert!(text.contains("'<n>eutron emission for (n,2n)    '/\n-1 2/\n"), "no law-67 page:\n{text}");
    let tail = text.split("'<n>eutron emission for").nth(1).unwrap();
    assert!(tail.contains("\n1/\n"));
    // Two incident energies (10 and 20 MeV), each with its energy record.
    assert!(tail.contains("  1.000000E+01/\n") && tail.contains("  2.000000E+01/\n"));
    assert!(text.ends_with("99/\nstop\n"));
}
