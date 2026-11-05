//! The calls behind the lesson "Breathing: how the release leaves the
//! vessel" (gh:#1207, maintainer direction 2026-11-04). TRISO-ATOPS's
//! accident moves what leaves the fuel out of the vessel only with the gas
//! the core pushes out as it heats ("breathing"; Stoyer et al. 2026, pp. 4 and
//! 13), and it does so as a PRODUCT: released(t) = frac(t) × R(t). This
//! module computes that product exactly as the port does, and puts the same
//! release and the same gas on a well-mixed gas volume, so the product's
//! assumption can be measured.
//!
//! - The port: `lesson_triso_atops/atops.rs`'s `accident` (MHTGR Case A
//!   node, `integrate_diffusion_over_time`, `release_fraction_transient`,
//!   `mean_temperature_rate`, `coolant_release`), reused, not copied.
//! - The well-mixed gas: boon-lay's `live_pools::step` (an extension of the
//!   port, exact for a source held constant over a step), with the
//!   circulating pool as the gas, the source `dR/dt`, decay `λ`, no plate-out
//!   or clean-up, and the "leak" rate the gas's own outflow
//!   `q = (dT/dt)/T` while the core heats (ideal gas at fixed P and V:
//!   `n ∝ 1/T`, so the fraction of the gas present that leaves per second is
//!   `−ṅ/n = Ṫ/T`), zero while it cools.
//!
//! The temperatures are the ILLUSTRATIVE heat-up of chapter 4 and of
//! lesson "Accidents: Booth on the integral, and venting" (`atops::heatup`):
//! not a plant calculation.
#![allow(dead_code)]

#[path = "../lesson_triso_atops/atops.rs"]
pub mod atops;

use boon_lay::triso_atops_fork::activities::live_pools::{step as pools_step, PoolRates, PrimaryPools};
use uom::si::frequency::hertz;

/// The compared nuclides (TRISO-ATOPS table names).
pub const NUCLIDES: [&str; 4] = ["Cs-137", "Sr-90", "I-131", "Kr-88"];

/// One nuclide through one heat-up, all as fractions of the kernel
/// inventory (undecayed counts, as the port's release fractions are).
pub struct Breath {
    pub hours: Vec<f64>,
    pub t_c: Vec<f64>,
    /// The port's kernel release fraction R(t) (available to vent).
    pub release: Vec<f64>,
    /// The port's vented-gas fraction, `None` where it does not vent.
    pub vent: Vec<Option<f64>>,
    /// The ideal-gas fraction 1 − T0/max(T so far), for comparison.
    pub vent_exact: Vec<f64>,
    /// The port's product frac(t) × R(t), `None` where it does not vent.
    pub atops: Vec<Option<f64>>,
    /// The well-mixed gas: cumulative atoms carried out, with decay.
    pub mixed: Vec<f64>,
    /// The same with decay switched off (isolates the timing assumption).
    pub mixed_no_decay: Vec<f64>,
}

/// The well-mixed gas volume: cumulative fraction carried out at each
/// sample, `substeps` exact `live_pools` steps per sample interval with the
/// source `ΔR/Δt` and the outflow rate at each substep's midpoint.
pub fn well_mixed(hours: &[f64], t_c: &[f64], release: &[f64], decay_per_s: f64, substeps: usize) -> Vec<f64> {
    // Whatever has left the fuel by the first sample is in the gas then (in
    // the lesson's runs R(0) = 0; the test's closed-form case needs it).
    let mut pools = PrimaryPools { circulating: release[0], ..PrimaryPools::default() };
    let mut out = vec![0.0];
    for i in 1..hours.len() {
        let dt = (hours[i] - hours[i - 1]) * 3600.0;
        let s = (release[i] - release[i - 1]) / dt;
        let (t0, t1) = (t_c[i - 1] + 273.15, t_c[i] + 273.15);
        let slope = (t1 - t0) / dt;
        let h = dt / substeps as f64;
        for k in 0..substeps {
            let t_mid = t0 + slope * h * (k as f64 + 0.5);
            let q = if slope > 0.0 { slope / t_mid } else { 0.0 };
            let rates = PoolRates { decay: decay_per_s, plate_out: 0.0, clean_up: 0.0, leak: q };
            pools = pools_step(pools, s, rates, h).0;
        }
        out.push(pools.leaked);
    }
    out
}

/// The comparison for nuclide `name` on `atops::heatup(900, peak)`.
pub fn breath(name: &str, peak: f64, substeps: usize) -> Breath {
    let n = atops::nuclide(name);
    let (hours, t_c) = atops::heatup(900.0, peak);
    let acc = atops::accident(&n, &hours, &t_c);
    let release = acc.rf_kernel.clone();
    let atops_v: Vec<Option<f64>> = acc.vent.iter().zip(&release).map(|(v, r)| v.map(|f| f * r)).collect();
    let t0 = t_c[0] + 273.15;
    let mut tmax = t0;
    let vent_exact = t_c
        .iter()
        .map(|t| {
            tmax = tmax.max(t + 273.15);
            1.0 - t0 / tmax
        })
        .collect();
    let lam = n.decay_constant().get::<hertz>();
    let mixed = well_mixed(&hours, &t_c, &release, lam, substeps);
    let mixed_no_decay = well_mixed(&hours, &t_c, &release, 0.0, substeps);
    Breath { hours, t_c, release, vent: acc.vent, vent_exact, atops: atops_v, mixed, mixed_no_decay }
}

/// The no-decay well-mixed answer written out for piecewise-linear R and
/// T (the derivation on the page), a cross-check of [`well_mixed`] that does
/// not use `live_pools`. With n ∝ 1/T at fixed P and V, gas leaves only
/// while T climbs to a new maximum; an atom released at s, when T(s) is the
/// highest temperature so far, has left with share 1 − T(s)/Tmax(t) by t. An
/// atom released during a hold or while cooling never leaves (share 0). So
/// `A(t) = ∫ dR(s) · [1 − T(s)/Tmax(t)]` over the rising stretches,
/// evaluated with `n` midpoints per sample interval.
pub fn mixed_formula(hours: &[f64], t_c: &[f64], release: &[f64], n: usize) -> Vec<f64> {
    let k: Vec<f64> = t_c.iter().map(|t| t + 273.15).collect();
    let mut out = Vec::with_capacity(hours.len());
    for j in 0..hours.len() {
        let tmax = k[..=j].iter().cloned().fold(f64::MIN, f64::max);
        let mut a = 0.0;
        let mut run_max = k[0];
        for i in 1..=j {
            let dr = release[i] - release[i - 1];
            if k[i] > k[i - 1] && k[i - 1] >= run_max {
                let mut share = 0.0;
                for m in 0..n {
                    let ts = k[i - 1] + (k[i] - k[i - 1]) * (m as f64 + 0.5) / n as f64;
                    share += (1.0 - ts / tmax) / n as f64;
                }
                a += dr * share;
            }
            run_max = run_max.max(k[i]);
        }
        out.push(a);
    }
    out
}
