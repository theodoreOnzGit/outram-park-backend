// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 Theodore Ong and the outram-park contributors.

//! **Code-to-code V&V: PETIR's eight depletion integrators against OpenMC's
//! own running code** (gh:#950).
//!
//! # Methodology
//!
//! `verification_and_validation/depletion/openmc_inputs/integrators_reference.py`
//! runs OpenMC's unmodified `integrate()` (commit `d7d3284a1`, OpenMC
//! 0.16.1-dev25, numpy 2.5.3, scipy 1.18.1) for all eight integrators on the
//! three-nuclide constant-power problem in `depletion_common`, four steps of
//! unequal length (0.1, 0.25, 0.2, 0.45 s, so LE/QI's `prev_dt != dt`), with
//! upstream's default `n_steps = 10` for the stochastic-implicit methods. It
//! records every saved step: time, source rate, the three densities, `k` and
//! its standard deviation, and both reaction rates. It does this twice:
//!
//! * with upstream's `CRAM48` (SuperLU), and
//! * with `dense_ipf48`, a pure-Python replica of PETIR's dense IPF CRAM48,
//!   passed through upstream's `solver=` argument.
//!
//! This test runs [`DepletionIntegrator::integrate`] with
//! [`CramSolver::Cram48`] on the same problem and compares every number.
//!
//! **Pass criteria, fixed before the first run:**
//!
//! 1. Against `dense_ipf48`: **bit-identical** for every time, density, `k`
//!    and rate, in every record of every integrator. The solver is the same
//!    arithmetic on both sides, so any difference is in the integrator
//!    logic. `k`'s standard deviation is compared separately (DEVIATION 7 in
//!    the module docs) and only reported.
//! 2. Against upstream's own `CRAM48`: relative difference below `1e-10` on
//!    every density (the solver DEVIATION; "the same answer to round-off").
//! 3. Operator-call totals equal on both sides.
//!
//! # Results (2026-10-17)
//!
//! Recorded in `verification_and_validation/depletion/petir_integrators_2026_10_17.md`
//! and printed at run time.

mod depletion_common;

use depletion_common::{FixedRate, N0, SOURCE};
use petir::depletion::{CramSolver, DepletionIntegrator, Integrator};

const REFERENCE: &str = include_str!(
    "../verification_and_validation/depletion/openmc_inputs/integrators_reference.txt"
);
const TIMESTEPS: [f64; 4] = [0.1, 0.25, 0.2, 0.45];

/// One reference line: `(solver, integrator, step, values, total calls)`.
fn rows() -> Vec<(String, String, usize, Vec<f64>, usize)> {
    REFERENCE
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            let vals = f[3..f.len() - 1]
                .iter()
                .map(|v| v.parse::<f64>().unwrap())
                .collect();
            (
                f[0].to_string(),
                f[1].to_string(),
                f[2].parse().unwrap(),
                vals,
                f[f.len() - 1].parse().unwrap(),
            )
        })
        .collect()
}

/// Our records in the reference's column order (minus k_std, which is last).
fn ours(method: Integrator) -> (Vec<(Vec<f64>, f64)>, usize) {
    let mut op = FixedRate::default();
    let mut integ = DepletionIntegrator::new(
        method,
        CramSolver::Cram48,
        TIMESTEPS.to_vec(),
        vec![SOURCE; 4],
    )
    .unwrap();
    let recs = integ.integrate(&mut op, vec![N0.to_vec()], true).unwrap();
    let out = recs
        .iter()
        .map(|r| {
            let mut v = vec![r.time[0], r.time[1], r.source_rate];
            v.extend(&r.n[0]);
            v.push(r.k.value);
            v.extend(&r.rates);
            (v, r.k.std_dev)
        })
        .collect();
    (out, op.calls)
}

#[test]
fn integrators_match_openmc_running_code() {
    let reference = rows();
    let mut bit_failures = Vec::new();
    let mut worst_upstream = 0.0_f64;
    let mut k_std_bits = (0usize, 0usize);
    for method in Integrator::ALL {
        let (mine, calls) = ours(method);
        for (solver, exact) in [("dense_ipf48", true), ("openmc_cram48", false)] {
            let refs: Vec<_> = reference
                .iter()
                .filter(|r| r.0 == solver && r.1 == method.name())
                .collect();
            assert_eq!(refs.len(), mine.len(), "{method:?} {solver}: record count");
            let mut worst = 0.0_f64;
            for (r, (m, k_std)) in refs.iter().zip(&mine) {
                assert_eq!(r.4, calls, "{method:?} {solver}: operator calls");
                // reference columns: t0 t1 src nA nB nC k k_std rf rg
                let mut want = r.3.clone();
                let ref_k_std = want.remove(7);
                if exact {
                    for (i, (a, b)) in m.iter().zip(&want).enumerate() {
                        if a.to_bits() != b.to_bits() {
                            bit_failures.push(format!(
                                "{method:?} step {} column {i}: {a:e} vs {b:e}",
                                r.2
                            ));
                        }
                    }
                    k_std_bits.1 += 1;
                    if k_std.to_bits() == ref_k_std.to_bits() {
                        k_std_bits.0 += 1;
                    }
                } else {
                    for (a, b) in m[3..6].iter().zip(&want[3..6]) {
                        let rel = if *b == 0.0 {
                            a.abs()
                        } else {
                            ((a - b) / b).abs()
                        };
                        worst = worst.max(rel);
                    }
                }
            }
            if !exact {
                println!(
                    "{method:?}: worst density difference vs OpenMC CRAM48 (SuperLU) {worst:.3e}"
                );
                worst_upstream = worst_upstream.max(worst);
            }
        }
    }
    println!(
        "k std-dev bit-identical in {} of {} records",
        k_std_bits.0, k_std_bits.1
    );
    assert!(
        bit_failures.is_empty(),
        "integrator logic differs from OpenMC's running code:\n{}",
        bit_failures.join("\n")
    );
    assert!(
        worst_upstream < 1.0e-10,
        "solver deviation {worst_upstream:e} exceeds round-off"
    );
}

/// The reference's own record of `pow(dt, 2) != dt * dt` (`matrix_funcs.rs`
/// DEVIATION) is empty for the timesteps it ran.
#[test]
fn pow_two_equals_square_on_the_reference_steps() {
    assert!(!REFERENCE.contains("POW_DIFFERS"));
}
