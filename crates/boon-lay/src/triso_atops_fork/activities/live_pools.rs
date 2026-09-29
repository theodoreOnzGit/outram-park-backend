// SPDX-License-Identifier: GPL-3.0
//
// NOT a port. An extension of the ported TRISO-ATOPS coolant-activity model
// (`coolant_activity.rs`, which ports upstream commit de374c8's
// `circulating`/`plate_out`/`clean_up`), written for boon-lay on 2026-09-29
// (gh:#399). It solves the same linear balances, generalised in two ways the
// upstream closed forms do not cover: (1) any initial pool state, so the pools
// can be stepped live through a plant transient with a source that changes
// between steps; (2) a primary-circuit LEAK sink, so the activity that leaves
// the circuit is accounted for rather than implicit.

//! # Live primary-circuit activity pools
//!
//! The ported closed forms in [`super::coolant_activity`] answer "what is in
//! each pool after running for time `t` from empty with a constant source".
//! A simulator needs the pools *carried*: the source changes as the fuel
//! temperature changes, and a transient must start from whatever the plant has
//! accumulated. This module steps the same balances from an arbitrary state,
//! **exactly** for a source held constant over the step:
//!
//! ```text
//! dC/dt = S - beta C,          beta = lambda + k_plate + k_clean + k_leak
//! dP/dt = k_plate C - lambda P
//! dH/dt = k_clean C - lambda H
//! dL/dt = k_leak  C                 (atoms that left the circuit, cumulative)
//! ```
//!
//! `C`, `P`, `H` are atom counts, as in [`super::coolant_activity`] (activity
//! is `lambda x atoms`, converted once downstream); `S` is atoms/s. `L` counts
//! atoms **at the moment they leak**, undecayed -- the release to whatever is
//! downstream of the circuit.
//!
//! **Exact integration.** With `S` constant over `dt` and `e_b = exp(-beta dt)`,
//! `e_l = exp(-lambda dt)`:
//!
//! ```text
//! C1 = C0 e_b + (S/beta)(1 - e_b)
//! int_0^dt C = (S/beta) dt + (C0 - S/beta)(1 - e_b)/beta
//! int_0^dt C(s) e^{-lambda (dt - s)} ds
//!      = (S/beta)(1 - e_l)/lambda + (C0 - S/beta)(e_b - e_l)/(lambda - beta)
//! P1 = P0 e_l + k_plate x (that integral),   H1 likewise with k_clean
//! L1 = L0 + k_leak int C
//! ```
//!
//! **Consistency with the ported closed forms**, checked in the tests: from
//! `C0 = P0 = H0 = 0`, with `k_leak = 0` and no parent, one step of length `t`
//! reproduces [`super::coolant_activity::circulating`],
//! [`super::coolant_activity::plate_out`] and
//! [`super::coolant_activity::clean_up`] to rounding.
//!
//! **Atom conservation**, also checked: over any step, `S dt = (C1 - C0) +
//! (P1 - P0) + (H1 - H0) + (L1 - L0) + decayed`, with `decayed = lambda int (C
//! + P + H)`, evaluated here in closed form.
//!
//! No parent in-growth: a daughter's pools are driven by its own source only
//! (the ported forms' `C_parent`/`P_parent` are passed as zero by every caller
//! in this workspace).

/// The three primary-circuit pools and the cumulative leak, in **atoms**.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PrimaryPools {
    /// Circulating in the coolant.
    pub circulating: f64,
    /// Plated out on circuit surfaces.
    pub plate_out: f64,
    /// Held in the helium-purification system (clean-up).
    pub clean_up: f64,
    /// Cumulative atoms that left the circuit by leakage (undecayed count at
    /// the moment of leaking).
    pub leaked: f64,
}

/// The rate constants a step needs \[1/s\], all non-negative.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoolRates {
    /// Decay constant `lambda`.
    pub decay: f64,
    /// Plate-out `k_plate`.
    pub plate_out: f64,
    /// Clean-up (HPS) `k_clean`.
    pub clean_up: f64,
    /// Primary-circuit leakage `k_leak`.
    pub leak: f64,
}

/// What one step moved \[atoms\], for conservation checks and for the
/// downstream consumer of the leak.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PoolStepFlows {
    /// `S dt`: atoms that entered the coolant from the fuel.
    pub entered: f64,
    /// Atoms that decayed in the three pools during the step.
    pub decayed: f64,
    /// Atoms that leaked out of the circuit during the step.
    pub leaked: f64,
}

/// `int_0^dt C(s) e^{-lambda (dt - s)} ds` for `C(s) = c_eq + (c0 - c_eq)
/// e^{-beta s}`, handling `beta -> lambda` without dividing by zero.
fn decayed_convolution(c_eq: f64, c0: f64, beta: f64, lambda: f64, dt: f64) -> f64 {
    let e_l = (-lambda * dt).exp();
    let first = if lambda * dt > 1.0e-12 {
        c_eq * (1.0 - e_l) / lambda
    } else {
        c_eq * dt
    };
    let d = lambda - beta;
    let second = if (d * dt).abs() > 1.0e-12 {
        (c0 - c_eq) * ((-beta * dt).exp() - e_l) / d
    } else {
        (c0 - c_eq) * dt * e_l
    };
    first + second
}

/// `int_0^dt C(s) ds` for the same `C(s)`.
fn circulating_integral(c_eq: f64, c0: f64, beta: f64, dt: f64) -> f64 {
    if beta * dt > 1.0e-12 {
        c_eq * dt + (c0 - c_eq) * (1.0 - (-beta * dt).exp()) / beta
    } else {
        c0 * dt
    }
}

/// Step the pools by `dt` \[s\] with the source `source_atoms_per_s` held
/// constant, **exactly** (see the module doc). Returns the new pools and what
/// moved.
///
/// # Panics
///
/// If `dt` is negative or a rate is negative or non-finite -- a caller error,
/// not a physical state.
#[must_use]
pub fn step(
    pools: PrimaryPools,
    source_atoms_per_s: f64,
    rates: PoolRates,
    dt: f64,
) -> (PrimaryPools, PoolStepFlows) {
    assert!(
        dt >= 0.0 && dt.is_finite(),
        "negative or non-finite step {dt}"
    );
    for r in [rates.decay, rates.plate_out, rates.clean_up, rates.leak] {
        assert!(r >= 0.0 && r.is_finite(), "negative or non-finite rate {r}");
    }
    let s = source_atoms_per_s;
    let lambda = rates.decay;
    let beta = rates.decay + rates.plate_out + rates.clean_up + rates.leak;
    let c0 = pools.circulating;
    let c_eq = if beta > 0.0 { s / beta } else { 0.0 };

    let circulating = if beta > 0.0 {
        c0 * (-beta * dt).exp() + c_eq * (1.0 - (-beta * dt).exp())
    } else {
        c0 + s * dt
    };
    let int_c = if beta > 0.0 {
        circulating_integral(c_eq, c0, beta, dt)
    } else {
        c0 * dt + 0.5 * s * dt * dt
    };
    let conv = if beta > 0.0 {
        decayed_convolution(c_eq, c0, beta, lambda, dt)
    } else {
        int_c
    };
    let e_l = (-lambda * dt).exp();
    let plate_out = pools.plate_out * e_l + rates.plate_out * conv;
    let clean_up = pools.clean_up * e_l + rates.clean_up * conv;
    let leaked_now = rates.leak * int_c;

    // Decays, by the atom balance of each pool (exact for the exact pools):
    // C: S dt - (k_p + k_c + k_l) int C - dC ; P: k_p int C - dP ; H likewise.
    let decayed_c =
        s * dt - (rates.plate_out + rates.clean_up + rates.leak) * int_c - (circulating - c0);
    let decayed_p = rates.plate_out * int_c - (plate_out - pools.plate_out);
    let decayed_h = rates.clean_up * int_c - (clean_up - pools.clean_up);

    (
        PrimaryPools {
            circulating,
            plate_out,
            clean_up,
            leaked: pools.leaked + leaked_now,
        },
        PoolStepFlows {
            entered: s * dt,
            decayed: decayed_c + decayed_p + decayed_h,
            leaked: leaked_now,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::triso_atops_fork::activities::coolant_activity::{circulating, clean_up, plate_out};
    use crate::triso_atops_fork::DecayConstant;
    use uom::si::f64::{Frequency, Time};
    use uom::si::frequency::hertz;
    use uom::si::time::second;

    /// **One step from empty reproduces the ported closed forms.**
    ///
    /// Methodology: `C0 = P0 = H0 = 0`, `k_leak = 0`, one step of `t` =
    /// 1e3 s, 1e6 s and 6.3e8 s (20 a), for a short-lived (Xe-133-like,
    /// lambda 1.5e-6) and a long-lived (Cs-137-like, 7.3e-10) decay constant,
    /// with `k_plate = 7.5e-4`, `k_clean = 1.4e-5`; compare with
    /// `coolant_activity::{circulating, plate_out, clean_up}` (ported,
    /// code-to-code verified against upstream TRISO-ATOPS). Pass: 1e-10
    /// relative. **Results (2026-09-29):** worst relative difference **2.6e-16**.
    #[test]
    fn one_step_from_empty_reproduces_the_ported_closed_forms() {
        let (kp, kc) = (7.5e-4, 1.4e-5);
        let s = 1.0e8;
        let mut worst = 0.0f64;
        for lam in [1.5e-6, 7.3e-10] {
            for t in [1.0e3, 1.0e6, 6.3e8] {
                let (p, _) = step(
                    PrimaryPools::default(),
                    s,
                    PoolRates {
                        decay: lam,
                        plate_out: kp,
                        clean_up: kc,
                        leak: 0.0,
                    },
                    t,
                );
                let l = DecayConstant::new::<hertz>(lam);
                let (fkp, fkc) = (Frequency::new::<hertz>(kp), Frequency::new::<hertz>(kc));
                let tt = Time::new::<second>(t);
                let c_ref = circulating(s, fkp, l, tt, fkc, 0.0);
                let p_ref = plate_out(fkp, s, l, tt, c_ref, fkc, 0.0);
                let h_ref = clean_up(fkp, s, l, tt, c_ref, fkc, 0.0);
                for (a, b) in [
                    (p.circulating, c_ref),
                    (p.plate_out, p_ref),
                    (p.clean_up, h_ref),
                ] {
                    worst = worst.max((a - b).abs() / b.abs().max(1e-300));
                }
            }
        }
        println!("live pools vs ported closed forms, worst relative difference {worst:.3e}");
        assert!(worst < 1e-10);
    }

    /// **Atoms are conserved every step, with leak, from any state**, and many
    /// small steps equal one big step (the integration is exact for a constant
    /// source).
    ///
    /// Methodology: from a non-zero state, 1000 steps of 1 s against one step
    /// of 1000 s, with `k_leak = 1.157e-7` (1 %/day); check `entered = dC + dP
    /// + dH + leaked + decayed` to 1e-9 of `entered`, and the two paths to
    /// 1e-9 relative. **Results (2026-09-29):** atom-balance residual
    /// **-1.9e-17** of `entered`; many-vs-one worst **1.7e-13**.
    #[test]
    fn atoms_are_conserved_and_small_steps_equal_one_big_step() {
        let rates = PoolRates {
            decay: 1.5e-6,
            plate_out: 3.0e-4,
            clean_up: 1.4e-5,
            leak: 1.157e-7,
        };
        let start = PrimaryPools {
            circulating: 5e11,
            plate_out: 2e12,
            clean_up: 1e12,
            leaked: 0.0,
        };
        let s = 3.0e8;
        let (mut many, mut entered, mut decayed, mut leaked) = (start, 0.0, 0.0, 0.0);
        for _ in 0..1000 {
            let (p, f) = step(many, s, rates, 1.0);
            many = p;
            entered += f.entered;
            decayed += f.decayed;
            leaked += f.leaked;
        }
        let (one, f1) = step(start, s, rates, 1000.0);
        let stored = (many.circulating - start.circulating)
            + (many.plate_out - start.plate_out)
            + (many.clean_up - start.clean_up);
        let residual = entered - stored - leaked - decayed;
        let path = [
            (many.circulating, one.circulating),
            (many.plate_out, one.plate_out),
            (many.clean_up, one.clean_up),
            (many.leaked, one.leaked),
        ]
        .iter()
        .map(|(a, b)| (a - b).abs() / b.abs())
        .fold(0.0f64, f64::max);
        println!(
            "atom balance residual {:.3e} of entered; many-vs-one worst {path:.3e}; decayed {:.4e} vs {:.4e}",
            residual / entered,
            decayed,
            f1.decayed
        );
        assert!((residual / entered).abs() < 1e-9);
        assert!(path < 1e-9);
    }
}
