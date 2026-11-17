//! The `MicroXS` collapse (gh:#1625) on real data: RECONR's resonance-
//! reconstructed Zn-68 (n,γ) from ENDF/B-VIII.0, against an independent
//! integrator, `njoy_outram_park_fork::isomeric_branching::collapse`
//! (Simpson on the union grid, gh:#1628), asked for the same quantity:
//! a "state" whose MF=10 cross section is σ itself over σ_MT = 1 b gives
//! the flux-weighted mean σ.
//!
//! Prediction: the two agree to round-off (relative 1e-12) in a thermal,
//! a resonance and a fast group structure; both integrate a piecewise
//! linear function exactly, in a different order.
//!
//! Measured 2026-11-17: thermal 0.8601368764972 b (2.2e-15 relative), resonance
//! 0.5880053174707 b (1.5e-14), fast 6.051245356441e-3 b (2.9e-16).

use njoy_outram_park_fork::endf::records::{Cont, Tab1};
use njoy_outram_park_fork::isomeric_branching::{collapse, ProductionState, StateData};
use outram_mc_libs::depletion::microxs::{nuclide_collapse_rate, NuclideReaction, ReactionTable};

#[test]
fn collapse_rate_matches_an_independent_integrator_on_reconr_zn68_capture() {
    let Some(path) = njoy_outram_park_fork::reference_data::reference_endf_or_skip("n-030_Zn_068-ENDF8.0.endf", "microxs_on_reconr_data") else { return };
    let tape = njoy_outram_park_fork::endf::tape::Tape::from_bytes(&std::fs::read(path).unwrap()).unwrap();
    let rec = njoy_outram_park_fork::process::reconstruct(&tape, &njoy_outram_park_fork::process::CrossSectionSettings::default()).unwrap();
    let mt102 = njoy_outram_park_fork::endf::mt::MtReaction::from_any(102);
    let sigma = rec.reconr.sections.iter().find(|s| s.mt == mt102).unwrap().pairs.clone();

    let rx = NuclideReaction { low: ReactionTable::from_pointwise(&sigma), high: None };
    let head = Cont { c1: 0.0, c2: 0.0, l1: 0, l2: 0, n1: 1, n2: sigma.len() as i32 };
    let as_state = ProductionState {
        mt: 102,
        zap: 30069,
        lfs: 0,
        isomer: 0,
        level_energy_ev: 0.0,
        data: StateData::CrossSection(Tab1 { head, interp: vec![(sigma.len() as u32, 2)], pairs: sigma.clone() }),
    };
    let lo = sigma[0].0;
    let hi = sigma[sigma.len() - 1].0;
    let unit = vec![(lo, 1.0), (hi, 1.0)];
    for (label, bounds, flux) in [
        ("thermal", vec![1e-5, 0.1, 1.0], vec![3.0, 1.0]),
        ("resonance", vec![1.0, 1e2, 1e3, 1e4, 1e5], vec![1.0, 2.0, 1.5, 0.5]),
        ("fast", vec![1e5, 1e6, 2e7], vec![1.0, 1.0]),
    ] {
        let total: f64 = flux.iter().sum();
        let norm: Vec<f64> = flux.iter().map(|f| f / total).collect();
        let ours = nuclide_collapse_rate(&rx, &bounds, &norm);
        let other = collapse(std::slice::from_ref(&as_state), &unit, &bounds, &flux).unwrap()[0].branching_ratio;
        let rel = ((ours - other) / other).abs();
        println!("{label}: collapse_rate {ours:.12e} b, independent {other:.12e} b, relative {rel:.2e}");
        assert!(rel < 1e-12, "{label}: {rel}");
    }
}
