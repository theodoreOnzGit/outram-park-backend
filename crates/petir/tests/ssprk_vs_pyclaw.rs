// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors

//! **Code-to-code: `petir::ssprk` against PyClaw's own `SharpClawSolver.step`.**
//!
//! # Methodology
//!
//! `reference-data/pyclaw/ssp_rk_reference_driver.py` ran upstream's
//! unmodified `src/pyclaw/sharpclaw/solver.py` (commit `f522337e`, BSD-3) under
//! CPython 3.11.17 / NumPy 2.4.6 on x86_64 Linux, 2026-10-16. The system is
//! 16 periodic cells with `L(t,q)_i = -4 (q_i - q_{i-1}) - 0.5 q_i^2 + 0.1 t`.
//! That is pure arithmetic, so both sides do the same IEEE operations. It was
//! run for 40 steps of `SSP22`, `SSP33` and `SSP104`, at `dt = 2^-4` and at
//! `dt = 0.03`. Every final value is compared by its bit pattern.
//!
//! **Pass criterion.** At `dt = 2^-4`, all 48 values are bit-identical. At
//! `dt = 0.03`, every value agrees to 1e-14 relative
//! (measured: bit-identical). The `dt = 0.03` case
//! is not held to bit equality because of the one documented deviation: PyClaw
//! forms the first stage as `q + dt * ((dt*L)/dt)` (it caches `dq/dt` for
//! step rejection), while this port forms `q + dt*L`. The round trip is exact
//! when `dt` is a power of two, and only then. See `petir::ssprk`'s module
//! docs.
//!
//! # Prediction (written before the first run)
//!
//! 48/48 bit-identical at `dt = 2^-4`. At `dt = 0.03`, a minority of values
//! differ, by a few ulp.
//!
//! # Results (2026-10-16, x86_64-linux, release)
//!
//! - **`dt = 2^-4`: 48/48 bit-identical** (16 per method). The prediction held.
//! - **`dt = 0.03`: also 48/48 bit-identical.** The prediction **missed**:
//!   `(dt*L)/dt*dt` came back exactly `dt*L` at every first stage of these
//!   120 steps. That is a property of this case, not a guarantee. The
//!   deviation is real, and the `dt = 0.03` assertion stays at a 1e-14
//!   relative bound rather than bit equality, so that a different system is
//!   not reported as a port defect.
//!
//! **Interpretation.** Over 40 chained steps, each of PyClaw's three SSP
//! integrators and this port perform the same floating-point operations in
//! the same order. This verifies the port against upstream's running code,
//! not against the formulas in a paper.

use petir::ssprk::{ssp_step, SspMethod, SspWorkspace};

const N: usize = 16;
const C: f64 = 4.0;

fn rhs(t: f64, q: &[f64], d: &mut [f64]) {
    // np.roll(q, 1)[i] == q[i-1], periodic.
    let mut qm = q.last().copied().unwrap_or(0.0);
    for (di, &qi) in d.iter_mut().zip(q.iter()) {
        *di = -C * (qi - qm) - 0.5 * qi * qi + 0.1 * t;
        qm = qi;
    }
}

fn initial() -> Vec<f64> {
    (0..N).map(|i| if (4..10).contains(&i) { 1.0 } else { 0.25 + 0.01 * i as f64 }).collect()
}

fn run(method: SspMethod, dt: f64, nsteps: usize) -> Vec<f64> {
    let mut q = initial();
    let mut w = SspWorkspace::new(N);
    let mut t = 0.0;
    for _ in 0..nsteps {
        ssp_step(method, rhs, t, &mut q, dt, &mut w).unwrap();
        t += dt;
    }
    q
}

struct Row {
    method: SspMethod,
    dt: f64,
    nsteps: usize,
    i: usize,
    bits: u64,
}

fn parse_hex_float(s: &str) -> f64 {
    // CPython's float.hex(): "0x1.0000000000000p-4" / "0x1.eb851eb851eb8p-6".
    let (mant, exp) = s.trim_start_matches("0x").split_once('p').unwrap();
    let (int, frac) = mant.split_once('.').unwrap();
    let int: u64 = int.parse().unwrap();
    assert_eq!(int, 1, "normalised hex float expected");
    let frac = u64::from_str_radix(frac, 16).unwrap();
    let e: i64 = exp.parse().unwrap();
    f64::from_bits((((e + 1023) as u64) << 52) | frac)
}

fn reference() -> Vec<Row> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../reference-data/pyclaw/ssp-rk-pyclaw-reference.txt");
    let text = std::fs::read_to_string(&path).expect("reference file present");
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            let method = match f[0] {
                "SSP22" => SspMethod::Ssp22,
                "SSP33" => SspMethod::Ssp33,
                "SSP104" => SspMethod::Ssp104,
                other => panic!("unknown method {other}"),
            };
            Row {
                method,
                dt: parse_hex_float(f[1]),
                nsteps: f[2].parse().unwrap(),
                i: f[3].parse().unwrap(),
                bits: u64::from_str_radix(f[4], 16).unwrap(),
            }
        })
        .collect()
}

/// (bit-identical count, total, worst relative difference) per method and dt.
fn compare(method: SspMethod, dt: f64) -> (usize, usize, f64) {
    let rows: Vec<Row> = reference().into_iter().filter(|r| r.method == method && r.dt == dt).collect();
    assert_eq!(rows.len(), N, "{method:?} dt={dt}: reference rows");
    let q = run(method, dt, rows[0].nsteps);
    let mut same = 0;
    let mut worst = 0.0_f64;
    for r in &rows {
        let ours = q[r.i];
        let theirs = f64::from_bits(r.bits);
        if ours.to_bits() == r.bits {
            same += 1;
        }
        worst = worst.max(((ours - theirs) / theirs).abs());
    }
    (same, rows.len(), worst)
}

#[test]
fn power_of_two_dt_is_bit_identical_to_pyclaw() {
    for m in [SspMethod::Ssp22, SspMethod::Ssp33, SspMethod::Ssp104] {
        let (same, total, worst) = compare(m, 0.0625);
        println!("{m:?} dt=2^-4: {same}/{total} bit-identical, worst rel {worst:e}");
        assert_eq!(same, total, "{m:?}: every value must match upstream bit for bit");
    }
}

#[test]
fn non_power_of_two_dt_agrees_with_pyclaw() {
    for m in [SspMethod::Ssp22, SspMethod::Ssp33, SspMethod::Ssp104] {
        let (same, total, worst) = compare(m, 0.03);
        println!("{m:?} dt=0.03: {same}/{total} bit-identical, worst rel {worst:e}");
        assert!(worst < 1e-14, "{m:?} dt=0.03: worst rel {worst:e}");
    }
}
