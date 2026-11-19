// SPDX-License-Identifier: GPL-3.0

//! **H in H2O emission tables sit on THERMR's own incident-energy grid** (GitHub
//! #550).
//!
//! # Methodology
//!
//! `ThermalScattering::from_tape_with_grids` labels each emission table with a
//! log-spaced energy but fills it from `IncoherentInelasticScattering::emission`,
//! which returns the `calcem` record **nearest** that energy (4 to 10 % away).
//! The sampled outgoing energy is therefore a staircase in incident energy.
//! `from_tape_on_calcem_grid` (the default `from_tape` since 2026-11-19) puts one table at each `calcem` energy, as NJOY's
//! ACE does, so `sample_inelastic` interpolates between true records.
//!
//! Reference: the direct integral of the evaluation's own S(alpha,beta)
//! (`verification_and_validation/tutorial_rung4/sab_reference_550/sab_reference.py`,
//! ENDF INT = 4 law), `<E'>(0.1 eV) = 0.08228607 eV` and `<E'>(1.0 eV) =
//! 0.56876230 eV` (64 x 2 refinement, converged to 3e-6).
//!
//! Checks, 4e6 samples each, same seed:
//! 1. node-grid `<E'>` within 5e-4 of the direct integral at 0.1 and 1.0 eV
//!    (neither is a `calcem` node);
//! 2. node-grid `<E'>` at 0.1 eV equals the f-weighted mean of the two bracketing
//!    node tables' own means (the interpolation rule, computed independently
//!    from `emission`) to 5e-4;
//! 3. the log-spaced table is off the reference by more than 5e-3 at 1.0 eV
//!    (recorded: +1.46 %), pinning the staircase it is the contrast to;
//! 4. `sigma_inel` is bit-identical between the two constructions.
//!
//! # Results (2026-11-18, Intel Xeon 2.10 GHz, 4 logical cores)
//!
//! Node-grid `<E'>` relative to the direct integral (1e6 samples, sampling
//! sem about 4e-4): 0.1 eV -2.4e-5, 1.0 eV -4.0e-5; log-spaced at 1.0 eV
//! +1.46e-2 (4e6 samples in the record: 0.1 eV -1.12e-2). Independent
//! node-interpolation prediction at 0.1 eV 0.082290 against sampled 0.082284.
//! Run time 100 s. Record: `verification_and_validation/tutorial_rung4/sab_reference_550.md`.

#![cfg(feature = "std")] // needs files (gh:#1605)

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::thermr::scattering::IncoherentInelasticScattering;
use njoy_outram_park_fork::units::{NeutronEnergy, Temperature};
use outram_mc_libs::material::thermal::ThermalScattering;
use uom::si::energy::electronvolt;
use uom::si::thermodynamic_temperature::kelvin;

const N: usize = 1_000_000;

/// Sampled `<E'>` of `sample_inelastic` at `e` over `N` draws.
fn mean_ep(th: &ThermalScattering, e: f64) -> f64 {
    let mut seed = 20_261_118_u64;
    let mut s = 0.0;
    for _ in 0..N {
        s += th.sample_inelastic(e, &mut seed).expect("inside the treatment").0;
    }
    s / N as f64
}

/// `<E'>` of the `calcem` node table at an exact node energy.
fn node_mean(sab: &IncoherentInelasticScattering, e: f64) -> f64 {
    let bins = sab.emission(NeutronEnergy::new::<electronvolt>(e), 64, 8);
    bins.iter().map(|b| b.outgoing_energy.get::<electronvolt>()).sum::<f64>() / bins.len() as f64
}

#[test]
fn node_grid_emission_follows_the_evaluation_between_nodes() {
    let Some(p) = njoy_outram_park_fork::reference_data::reference_endf("tsl-HinH2O.endf") else {
        println!("tsl-HinH2O.endf absent: skipping");
        return;
    };
    let tape = Tape::read_file(&p).expect("tape");
    let node = ThermalScattering::from_tape_on_calcem_grid(&tape, 1, 293.6, "c_H_in_H2O").expect("node");
    // The log-spaced build is the explicit ablation since the 2026-11-19 flip.
    let legacy = ThermalScattering::from_tape_log_spaced_grid(&tape, 1, 293.6, "c_H_in_H2O")
        .expect("legacy");

    // 1. against the direct integral of the tape's S(alpha,beta).
    for (e, reference) in [(0.1, 0.082_286_07), (1.0, 0.568_762_30)] {
        let m = mean_ep(&node, e);
        let rel = m / reference - 1.0;
        println!("node grid  E {e}: <E'> {m:.6} vs integral {reference:.6}  rel {rel:+.2e}");
        assert!(rel.abs() < 5e-4, "node-grid <E'> at {e} eV is {rel:+.2e} from the integral");
    }
    let rel_legacy = mean_ep(&legacy, 1.0) / 0.568_762_30 - 1.0;
    println!("log-spaced E 1.0: rel {rel_legacy:+.2e}");
    assert!(
        rel_legacy.abs() > 5e-3,
        "the log-spaced table's staircase is no longer visible at 1.0 eV ({rel_legacy:+.2e}); \
         if it was fixed, update this contrast and the #550 record"
    );

    // 2. the interpolation rule, computed independently from `emission`.
    let sab = IncoherentInelasticScattering::from_tape(&tape, 1, Temperature::new::<kelvin>(293.6))
        .expect("sab");
    let (ea, eb) = (0.096, 0.1035); // neighbouring calcem nodes of 0.1 eV
    let f = (0.1 - ea) / (eb - ea);
    let expected = (1.0 - f) * node_mean(&sab, ea) + f * node_mean(&sab, eb);
    let got = mean_ep(&node, 0.1);
    println!("0.1 eV: sampled {got:.6}, interpolated nodes {expected:.6}");
    assert!((got / expected - 1.0).abs() < 5e-4);

    // 4. same sigma_inel (the xs table is THERMR's own in both).
    for e in [0.0253, 0.1, 1.0, 3.5] {
        assert_eq!(node.inelastic_xs(e).to_bits(), legacy.inelastic_xs(e).to_bits());
    }
}
