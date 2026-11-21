//! Phase 2 (gh:#1688): `petir`'s `f32` `RealMath` (`r_exp`, `r_ln`, `r_powf`,
//! `r_powi`) against gfortran's `real*4` `exp`, `log`, `x ** y` and `x ** n`
//! (glibc `expf`, `logf`, `powf`, libgcc `__powisf2`), bit for bit, on 4000
//! (exp/ln/powf) and 48000 (powi) arguments. Sources of the reference programs:
//! `verification_and_validation/f32_math/{exp_log_pow.f,powi.f}`
//! (`gfortran -O1 x.f && ./a.out > ref.txt`); the test reads them from
//! `F32_REF` and `F32_REFP`. Skipped unless set; prints the mismatch counts.
//!
//! **Result, 2026-11-21:** `r_exp` 319, `r_ln` 282, `r_powf` 379 of 4000
//! arguments differ from glibc by one ulp (about 7-9 %); `r_powi` 0 of 48000.
//! This was the main cause of the PEGS4 residual: computing those three in
//! `f64` and rounding (what `punggol::egsnrc::pegs4_*` now does) moved the
//! differing lines of `pb700` from 606 to 9 and the worst `|d|/max|line|` from
//! 1.7e-4 to 8.9e-6. `petir` itself is not changed here.

use petir::mathf::RealMath;
#[test]
fn f32_math_vs_gfortran() {
    let Ok(p) = std::env::var("F32_REF") else { return };
    let mut bad = [0usize; 3];
    let mut via64 = [0usize; 3];
    let mut n = 0;
    for (k, l) in std::fs::read_to_string(p).unwrap().lines().enumerate() {
        let i = (k + 1) as f32;
        let t = 1.0f32 + 0.00731f32 * i;
        let x = 1.0e-3f32 * (t * t);
        let v: Vec<u32> = l.split_whitespace().map(|h| u32::from_str_radix(h, 16).unwrap()).collect();
        assert_eq!(v[0], x.to_bits(), "x {k}");
        if RealMath::r_exp(x - 5.0f32).to_bits() != v[1] { bad[0] += 1; }
        if RealMath::r_ln(x).to_bits() != v[2] { bad[1] += 1; }
        if RealMath::r_powf(x, -0.3333333f32).to_bits() != v[3] { bad[2] += 1; }
        if (RealMath::r_exp(f64::from(x - 5.0f32)) as f32).to_bits() != v[1] { via64[0] += 1; }
        if (RealMath::r_ln(f64::from(x)) as f32).to_bits() != v[2] { via64[1] += 1; }
        if (RealMath::r_powf(f64::from(x), f64::from(-0.3333333f32)) as f32).to_bits() != v[3] { via64[2] += 1; }
        n += 1;
    }
    eprintln!("{n} args: exp mismatches {}, ln {}, powf {}; via f64: exp {}, ln {}, powf {}", bad[0], bad[1], bad[2], via64[0], via64[1], via64[2]);
}

#[test]
fn f32_powi_vs_gfortran() {
    let Ok(p) = std::env::var("F32_REFP") else { return };
    let txt = std::fs::read_to_string(p).unwrap();
    let mut it = txt.lines();
    let mut bad = 0usize;
    let mut n_all = 0usize;
    for i in 1..=3000 {
        let x = 0.5f32 + 0.00131f32 * (i as f32);
        for n in -6..=9i32 {
            let want = u32::from_str_radix(it.next().unwrap().trim(), 16).unwrap();
            if RealMath::r_powi(x, n).to_bits() != want { bad += 1; }
            n_all += 1;
        }
    }
    eprintln!("{n_all} powi: mismatches {bad}");
}

/// The same for `f64`: exp, ln, powf, sin, cos, tan, atan, log10 against glibc
/// (gfortran `real*8`), 20000 arguments (`F64_REF`, source
/// `verification_and_validation/f32_math/f64_funcs.f`).
#[test]
fn f64_math_vs_gfortran() {
    let Ok(p) = std::env::var("F64_REF") else { return };
    let mut bad = [0usize; 8];
    let mut n = 0usize;
    for (k, l) in std::fs::read_to_string(p).unwrap().lines().enumerate() {
        let i = (k + 1) as f64;
        let t = 1.0f64 + 0.00731f64 * i;
        let x = 1.0e-3f64 * (t * t);
        let h: Vec<u64> = l.split_whitespace().map(|h| u64::from_str_radix(h, 16).unwrap()).collect();
        let mine = [
            RealMath::r_exp(x - 5.0f64),
            RealMath::r_ln(x),
            RealMath::r_powf(x, -0.3333333f64),
            RealMath::r_sin(x),
            RealMath::r_cos(x),
            RealMath::r_tan(x * 0.01f64),
            RealMath::r_atan(x),
            RealMath::r_log10(x),
        ];
        for j in 0..8 {
            if mine[j].to_bits() != h[j] {
                bad[j] += 1;
            }
        }
        n += 1;
    }
    eprintln!("{n} args: mismatches exp {} ln {} pow {} sin {} cos {} tan {} atan {} log10 {}", bad[0], bad[1], bad[2], bad[3], bad[4], bad[5], bad[6], bad[7]);
}
