//! **Measurement — how fast the three routes actually are, from Rust.**
//!
//! The case for `petir::fast_*` is a speed claim, and a speed claim measured
//! in C is not a speed claim about this crate: LLVM inlines, unrolls and
//! schedules differently from GCC, and the tables are laid out differently.
//! So this measures the Rust code as callers will actually call it.
//!
//! `#[ignore]`d, because a timing test is not a correctness test and must
//! never fail a CI run on a loaded machine. Run it deliberately:
//!
//! ```sh
//! cargo test --release -p petir --test fast_math_speed -- --ignored --nocapture
//! ```
//!
//! # Results (2026-09-14, this host, `--release`)
//!
//! ```text
//!                platform (glibc)   petir::real (libm)   petir::fast_*
//!   exp                11.9 ms            16.6 ms            9.7 ms
//!   ln                  9.7 ms            13.7 ms           12.2 ms
//!   powf               35.0 ms            94.9 ms           44.3 ms
//! ```
//!
//! Ratios over four runs, so the spread is visible rather than hidden behind a
//! single figure: `libm/fast` = 1.67-1.80x (`exp`), 1.04-1.06x (`ln`),
//! 2.12-2.14x (`powf`); `fast/platform` = 0.71-0.82x (`exp`, i.e. faster than
//! the platform, because it inlines instead of calling through a dynamic
//! symbol), 1.24-1.32x (`ln`), 1.27-1.50x (`powf`).
//!
//! Re-run rather than trusting these on different hardware.
//!
//! Each loop accumulates into a `black_box`ed sum so nothing is optimised
//! away, and each route sees the identical input sequence.

use std::hint::black_box;
use std::time::Instant;

const N: usize = 2_000_000;

fn timed(label: &str, f: impl Fn() -> f64) -> f64 {
    // One warm-up pass so neither route pays for a cold cache or a first-touch
    // page fault that the other has already taken.
    black_box(f());
    let t = Instant::now();
    let acc = f();
    let ms = t.elapsed().as_secs_f64() * 1e3;
    println!("    {label:<34} {ms:8.1} ms   (checksum {acc:.6e})");
    ms
}

#[test]
#[ignore = "a timing measurement, not a correctness check -- run with --ignored"]
fn exp_log_pow_are_faster_than_the_libm_route() {
    println!("\n  exp, {N} calls over [-700, 700]:");
    let xs: Vec<f64> = (0..N)
        .map(|i| (i as f64 / N as f64) * 1400.0 - 700.0)
        .collect();
    let std_exp = timed("std (platform libm)", || xs.iter().map(|&x| x.exp()).sum());
    let petir_exp = timed("petir::real::exp (libm crate)", || {
        xs.iter().map(|&x| petir::real::exp(x)).sum()
    });
    let fast_exp = timed("petir::fast_exp::exp", || {
        xs.iter()
            .map(|&x| petir::fast_exp::exp(x).unwrap_or(0.0))
            .sum()
    });

    println!("\n  ln, {N} calls over (0, 1e6]:");
    let xs: Vec<f64> = (1..=N).map(|i| i as f64 * 0.5).collect();
    let std_ln = timed("std (platform libm)", || xs.iter().map(|&x| x.ln()).sum());
    let petir_ln = timed("petir::real::ln (libm crate)", || {
        xs.iter().map(|&x| petir::real::ln(x)).sum()
    });
    let fast_ln = timed("petir::fast_log::ln", || {
        xs.iter()
            .map(|&x| petir::fast_log::ln(x).unwrap_or(0.0))
            .sum()
    });

    println!("\n  powf, {N} calls, base in (0, 4], exponent in [-3, 3]:");
    let xy: Vec<(f64, f64)> = (1..=N)
        .map(|i| {
            let t = i as f64 / N as f64;
            (t * 4.0, t * 6.0 - 3.0)
        })
        .collect();
    let std_pow = timed("std (platform libm)", || {
        xy.iter().map(|&(x, y)| x.powf(y)).sum()
    });
    let petir_pow = timed("petir::real::powf (libm crate)", || {
        xy.iter().map(|&(x, y)| petir::real::powf(x, y)).sum()
    });
    let fast_pow = timed("petir::fast_pow::powf", || {
        xy.iter()
            .map(|&(x, y)| petir::fast_pow::powf(x, y).unwrap_or(0.0))
            .sum()
    });

    println!("\n  Speed-up of the fast route over the libm route:");
    for (name, slow, fast, platform) in [
        ("exp", petir_exp, fast_exp, std_exp),
        ("ln", petir_ln, fast_ln, std_ln),
        ("powf", petir_pow, fast_pow, std_pow),
    ] {
        println!(
            "    {name:<6} libm/fast = {:.2}x    fast/platform = {:.2}x",
            slow / fast,
            fast / platform
        );
    }
    println!();
}
