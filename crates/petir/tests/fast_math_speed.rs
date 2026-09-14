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
//!                platform (glibc)   libm crate (old)   petir::real (ARM, now)
//!   exp                12.0 ms            16.2 ms              8.9 ms
//!   ln                 10.0 ms            12.9 ms             13.7 ms
//!   powf               28.4 ms            93.8 ms             43.4 ms
//! ```
//!
//! Ratios over five runs, so the spread is visible rather than hidden behind a
//! single figure:
//!
//! ```text
//!            old libm route / ARM route      ARM route / platform
//!   exp            1.75 - 1.85x                   0.71 - 0.75x
//!   ln             1.09 - 1.18x                   1.20 - 1.25x
//!   powf           2.10 - 2.18x                   1.51 - 1.57x
//! ```
//!
//! Two things worth reading off that honestly. `exp` is **faster than the
//! platform** (0.7x), because it inlines instead of calling through a dynamic
//! symbol. And `ln` gains only ~1.1x, not the 1.7-2.2x the other two do — the
//! `libm` crate's `log` was already good, and the portable non-FMA branch this
//! port must take costs `log` a second table and a longer near-1 branch. A
//! single run early in this work showed `ln` at 0.94x; five further runs put
//! it at 1.09-1.18x, so that figure was noise, but it is the right order of
//! magnitude to expect: `ln` is close to a wash.
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
    let petir_exp = timed("libm crate (the old default)", || {
        xs.iter().map(|&x| libm::exp(x)).sum()
    });
    let fast_exp = timed("petir::real::exp (ARM port, default)", || {
        xs.iter().map(|&x| petir::real::exp(x)).sum()
    });

    println!("\n  ln, {N} calls over (0, 1e6]:");
    let xs: Vec<f64> = (1..=N).map(|i| i as f64 * 0.5).collect();
    let std_ln = timed("std (platform libm)", || xs.iter().map(|&x| x.ln()).sum());
    let petir_ln = timed("libm crate (the old default)", || {
        xs.iter().map(|&x| libm::log(x)).sum()
    });
    let fast_ln = timed("petir::real::ln (ARM port, default)", || {
        xs.iter().map(|&x| petir::real::ln(x)).sum()
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
    let petir_pow = timed("libm crate (the old default)", || {
        xy.iter().map(|&(x, y)| libm::pow(x, y)).sum()
    });
    let fast_pow = timed("petir::real::powf (ARM port, default)", || {
        xy.iter().map(|&(x, y)| petir::real::powf(x, y)).sum()
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
