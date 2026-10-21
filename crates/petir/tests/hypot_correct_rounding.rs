//! Correct rounding of PETIR's `hypot` / `hypotf` (CORE-MATH port, gh:#997),
//! replayed against GNU MPFR.
//!
//! # Methodology
//!
//! - **Reference:** `reference-data/core-math/hypot-mpfr-reference.txt`,
//!   written by `reference-data/core-math/hypot_mpfr_driver.c`
//!   (`build_and_run.sh` beside it) from MPFR 4.2.2's `mpfr_hypot` at 53 / 24
//!   bits, round-to-nearest, with the binary64 / binary32 exponent range and
//!   `mpfr_subnormalize`: the correctly rounded IEEE result.
//! - **Inputs:** (a) every ordered pair of 24 (f64) / 23 (f32) special values
//!   in all four sign combinations: `+-0`, the smallest and largest
//!   subnormals, the smallest normals, `1`, `1 + ulp`, `sqrt 3`, `2^-53`,
//!   `2^-27`, huge powers of two, `MAX`, `inf`, `NaN`; (b) four splitmix64
//!   sweeps of 10^6 pairs per type: uniform finite bit patterns, exponents
//!   within +-60 (f32 +-30) of each other, subnormal and tiny arguments, and
//!   Pythagorean triples scaled by `2^s` with the second leg nudged by -1, 0
//!   or +1 ulp (exact results and near-midpoints); (c) CORE-MATH's own
//!   worst-case files `hypot.wc` (33 174 inputs) and `hypotf.wc` (7 244).
//! - **What the C driver checks:** every input, one by one, for PETIR's port
//!   (linked through `petir_hypot_shim.rs`), glibc's `hypot` and the `libm`
//!   crate's (musl) `hypot`. **What this test checks:** the explicit `E`
//!   lines (all special pairs, every 16th worst case) bit for bit, and each
//!   10^6-input sweep, regenerated here with the same generator, through an
//!   FNV-1a hash of the result bits against MPFR's hash. A sweep hash match
//!   means every one of its 10^6 results is bit-identical to MPFR's.
//! - **Pass criterion:** zero mismatches. Equal bits, or both NaN.
//!
//! # Results (2026-10-20, MPFR 4.2.2, glibc 2.44, x86-64)
//!
//! Mismatches against MPFR, per input set (from the driver's `# SUMMARY`
//! lines in the reference file):
//!
//! ```text
//!                        n        PETIR   glibc 2.44   libm crate (musl)
//! f64 special pairs      2 304    0       0               16
//! f64 uniform bits       10^6     0       23           3 489
//! f64 close exponents    10^6     0       430         59 745
//! f64 subnormal/tiny     10^6     0       702         87 290
//! f64 Pythagorean +-ulp  10^6     0       403             68
//! f64 CORE-MATH worst    33 174   0       6 884        6 566
//! f32 special pairs      2 116    0       0                4
//! f32 uniform bits       10^6     0       0           12 936
//! f32 close exponents    10^6     0       0           58 686
//! f32 subnormal/tiny     10^6     0       0           83 800
//! f32 Pythagorean +-ulp  10^6     0       0           48 877
//! f32 CORE-MATH worst    7 244    0       1 912        1 952
//! ```
//!
//! **PETIR's port is correctly rounded on all 8 044 838 inputs.** Two
//! findings beside it: musl's `hypot`, which PETIR used until this change,
//! misrounds up to 8.7 % of the time on these sweeps; and **glibc 2.44's
//! binary64 `hypot` is not correctly rounded either** (23 per million on
//! uniform inputs, more near subnormals and exact cases), which corrects the
//! gh:#993 premise that it was. So "bit-identical to stock GSL" is NOT
//! implied by a correctly rounded `hypot`: GSL on glibc inherits glibc's
//! misroundings.
//!
//! The speed test below prints ns/call; the measured figures are recorded
//! in `src/hypot.rs` and `docs/verification-summary.md`.

use std::fs;
use std::hint::black_box;
use std::time::Instant;

const REF: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../reference-data/core-math/hypot-mpfr-reference.txt"
);

struct Sm(u64);
impl Sm {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

fn fin64(b: u64) -> u64 {
    if ((b >> 52) & 0x7ff) == 0x7ff {
        b ^ (1 << 62)
    } else {
        b
    }
}

fn fin32(b: u32) -> u32 {
    if ((b >> 23) & 0xff) == 0xff {
        b ^ (1 << 30)
    } else {
        b
    }
}

/// Mirror of `gen64` in the C driver. Keep in step.
fn gen64(set: u32, r: &mut Sm) -> (f64, f64) {
    let (a, b) = match set {
        1 => {
            let a = fin64(r.next());
            (a, fin64(r.next()))
        }
        2 => {
            let a = fin64(r.next());
            let e = (((a >> 52) & 0x7ff) as i64 + (r.next() % 121) as i64 - 60).clamp(0, 0x7fe);
            (a, (r.next() & 0x800f_ffff_ffff_ffff) | ((e as u64) << 52))
        }
        3 => {
            let ma = r.next();
            let ea = r.next() % 64;
            let mb = r.next();
            let eb = r.next() % 64;
            (
                (ma & 0x800f_ffff_ffff_ffff) | (ea << 52),
                (mb & 0x800f_ffff_ffff_ffff) | (eb << 52),
            )
        }
        _ => {
            let mut m = 1 + r.next() % 8191;
            let n = 1 + r.next() % 8191;
            if m == n {
                m += 1;
            }
            let p = (if m > n { m * m - n * n } else { n * n - m * m }) as f64;
            let q = (2 * m * n) as f64;
            let s = (r.next() % 2001) as i32 - 1000;
            let (p, q) = (libm::ldexp(p, s), libm::ldexp(q, s));
            let mut b = q.to_bits();
            match r.next() % 3 {
                1 => b += 1,
                2 => b -= 1,
                _ => {}
            }
            (p.to_bits(), b)
        }
    };
    (f64::from_bits(a), f64::from_bits(b))
}

/// Mirror of `gen32` in the C driver. Keep in step.
fn gen32(set: u32, r: &mut Sm) -> (f32, f32) {
    let (a, b) = match set {
        1 => {
            let a = fin32((r.next() >> 32) as u32);
            (a, fin32((r.next() >> 32) as u32))
        }
        2 => {
            let a = fin32((r.next() >> 32) as u32);
            let e = (((a >> 23) & 0xff) as i64 + (r.next() % 61) as i64 - 30).clamp(0, 0xfe);
            (
                a,
                (((r.next() >> 32) as u32) & 0x807f_ffff) | ((e as u32) << 23),
            )
        }
        3 => {
            let ma = r.next();
            let ea = r.next() % 32;
            let mb = r.next();
            let eb = r.next() % 32;
            (
                (((ma >> 32) as u32) & 0x807f_ffff) | ((ea as u32) << 23),
                (((mb >> 32) as u32) & 0x807f_ffff) | ((eb as u32) << 23),
            )
        }
        _ => {
            let mut m = 1 + r.next() % 2047;
            let n = 1 + r.next() % 2047;
            if m == n {
                m += 1;
            }
            let p = (if m > n { m * m - n * n } else { n * n - m * m }) as f32;
            let q = (2 * m * n) as f32;
            let s = (r.next() % 241) as i32 - 120;
            let (p, q) = (libm::ldexpf(p, s), libm::ldexpf(q, s));
            let mut b = q.to_bits();
            match r.next() % 3 {
                1 => b += 1,
                2 => b -= 1,
                _ => {}
            }
            (p.to_bits(), b)
        }
    };
    (f32::from_bits(a), f32::from_bits(b))
}

fn fnv(mut h: u64, v: u64, bytes: u32) -> u64 {
    for i in 0..bytes {
        h ^= (v >> (8 * i)) & 0xff;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn hex(s: &str) -> u64 {
    u64::from_str_radix(s, 16).unwrap()
}

#[test]
fn explicit_cases_match_mpfr_bit_for_bit() {
    let text = fs::read_to_string(REF).expect("reference file");
    let (mut n64, mut n32) = (0, 0);
    for line in text.lines().filter(|l| l.starts_with("E ")) {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f[1] == "f64" {
            let (x, y, z) = (
                f64::from_bits(hex(f[2])),
                f64::from_bits(hex(f[3])),
                f64::from_bits(hex(f[4])),
            );
            let got = petir::real::hypot(x, y);
            assert!(
                got.to_bits() == z.to_bits() || (got.is_nan() && z.is_nan()),
                "hypot({x:e}, {y:e}) [{line}] gave {:016x}",
                got.to_bits()
            );
            n64 += 1;
        } else {
            let (x, y, z) = (
                f32::from_bits(hex(f[2]) as u32),
                f32::from_bits(hex(f[3]) as u32),
                f32::from_bits(hex(f[4]) as u32),
            );
            let got = petir::real::hypotf(x, y);
            assert!(
                got.to_bits() == z.to_bits() || (got.is_nan() && z.is_nan()),
                "hypotf({x:e}, {y:e}) [{line}] gave {:08x}",
                got.to_bits()
            );
            n32 += 1;
        }
    }
    assert_eq!((n64, n32), (4378, 2569), "explicit case count changed");
}

#[test]
fn every_sweep_hashes_to_mpfr() {
    let text = fs::read_to_string(REF).expect("reference file");
    let mut sweeps = 0;
    for line in text.lines().filter(|l| l.starts_with("H ")) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let seed: u64 = f[3].parse().unwrap();
        let n: u64 = f[4].parse().unwrap();
        let want = hex(f[5]);
        let mut r = Sm(seed);
        let mut h = 0xcbf2_9ce4_8422_2325_u64;
        if f[1] == "f64" {
            let set = seed as u32;
            for _ in 0..n {
                let (x, y) = gen64(set, &mut r);
                let z = petir::real::hypot(x, y);
                h = fnv(
                    h,
                    if z.is_nan() {
                        0x7ff8_0000_0000_0000
                    } else {
                        z.to_bits()
                    },
                    8,
                );
            }
        } else {
            let set = (seed - 100) as u32;
            for _ in 0..n {
                let (x, y) = gen32(set, &mut r);
                let z = petir::real::hypotf(x, y);
                h = fnv(
                    h,
                    u64::from(if z.is_nan() { 0x7fc0_0000 } else { z.to_bits() }),
                    4,
                );
            }
        }
        assert_eq!(h, want, "sweep {line}: PETIR's results differ from MPFR's");
        sweeps += 1;
    }
    assert_eq!(sweeps, 8);
}

/// Every PETIR route reaches the port: `real::hypot`, `Real::hypot`,
/// `RealMath::r_hypot`, on both types.
#[test]
fn every_route_is_the_core_math_port() {
    use petir::mathf::RealMath;
    use petir::real::Real;
    let mut r = Sm(7);
    for _ in 0..10_000 {
        let (x, y) = gen64(2, &mut r);
        let z = petir::hypot::hypot(x, y).to_bits();
        assert_eq!(petir::real::hypot(x, y).to_bits(), z);
        assert_eq!(<f64 as Real>::hypot(x, y).to_bits(), z);
        assert_eq!(RealMath::r_hypot(x, y).to_bits(), z);
        let (a, b) = gen32(2, &mut r);
        let w = petir::hypot::hypotf(a, b).to_bits();
        assert_eq!(petir::real::hypotf(a, b).to_bits(), w);
        assert_eq!(<f32 as Real>::hypot(a, b).to_bits(), w);
        assert_eq!(RealMath::r_hypot(a, b).to_bits(), w);
    }
}

/// Cost per call, PETIR's port against the `libm` crate and the platform
/// (`f64::hypot`, glibc here), over the close-exponents sweep (the common
/// case: neither huge ratio nor special values). Prints only; timing is not
/// asserted because it depends on the host.
#[test]
fn speed_per_call() {
    const N: usize = 2_000_000;
    let mut r = Sm(2);
    let xs: Vec<(f64, f64)> = (0..N).map(|_| gen64(2, &mut r)).collect();
    let mut r = Sm(102);
    let fs_: Vec<(f32, f32)> = (0..N).map(|_| gen32(2, &mut r)).collect();
    let p = time64(&xs, |x, y| petir::real::hypot(x, y));
    let m = time64(&xs, |x, y| libm::hypot(x, y));
    let g = time64(&xs, |x, y| x.hypot(y));
    let pf = time32(&fs_, |x, y| petir::real::hypotf(x, y));
    let mf = time32(&fs_, |x, y| libm::hypotf(x, y));
    let gf = time32(&fs_, |x, y| x.hypot(y));
    println!("hypot  ns/call: petir {p:.2}  libm {m:.2}  platform {g:.2}");
    println!("hypotf ns/call: petir {pf:.2}  libm {mf:.2}  platform {gf:.2}");
}

fn time64<F: Fn(f64, f64) -> f64>(xs: &[(f64, f64)], f: F) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..5 {
        let t = Instant::now();
        let mut acc = 0.0;
        for &(x, y) in xs {
            acc += f(black_box(x), black_box(y));
        }
        black_box(acc);
        best = best.min(t.elapsed().as_secs_f64() * 1e9 / xs.len() as f64);
    }
    best
}

fn time32<F: Fn(f32, f32) -> f32>(xs: &[(f32, f32)], f: F) -> f64 {
    let mut best = f64::INFINITY;
    for _ in 0..5 {
        let t = Instant::now();
        let mut acc = 0.0f32;
        for &(x, y) in xs {
            acc += f(black_box(x), black_box(y));
        }
        black_box(acc);
        best = best.min(t.elapsed().as_secs_f64() * 1e9 / xs.len() as f64);
    }
    best
}
