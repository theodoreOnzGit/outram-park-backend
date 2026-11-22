// SPDX-License-Identifier: AGPL-3.0-only

//! GitHub #463 item 4: the resonance-scattering lower bound is a setting,
//! OpenMC's `<resonance_scattering><energy_min>` (`res_scat_energy_min`,
//! `src/settings.cpp:126`, default 0.01 eV), not a constant. The five-route
//! OpenMC deck sets 1e-5 eV.
//!
//! Methodology. A synthetic 0 K table (log grid 1e-6 eV to 1 keV, with a
//! resonance-like bump) on a heavy target (awr 236). Prediction recorded
//! before running: (a) the default is exactly 0.01 and an explicit 0.01 is
//! bit-identical to the default at every energy; (b) at 0.5 eV, above both
//! bounds, a 1e-5 eV bound is bit-identical to the default; (c) at 0.005 eV,
//! between the bounds, the two bounds take different sampler paths (default:
//! constant cross section; 1e-5: the table's method), so the outgoing states
//! differ somewhere in 64 histories. Pass criterion: those three.

use outram_mc_libs::geometry::position::Direction;
use outram_mc_libs::physics::scatter::{
    elastic_scatter_openmc, DbrcTable, DBRC_DEFAULT_E_MIN_EV,
};

fn table() -> DbrcTable {
    let pairs: Vec<(f64, f64)> = (0..120)
        .map(|i| {
            let e = 1.0e-6 * 10f64.powf(f64::from(i) * 9.0 / 119.0);
            let bump = 4000.0 * (-((e.ln() - 0.5f64.ln()) / 0.3).powi(2)).exp();
            (e, 10.0 + bump)
        })
        .collect();
    DbrcTable::from_pairs(&pairs, 1.0e3).expect("table")
}

fn run(t: &DbrcTable, e: f64) -> Vec<(u64, u64, u64, u64)> {
    let kt = 8.617333262e-5 * 293.6;
    (0..64u64)
        .map(|k| {
            let mut seed = 1000 + k;
            let (eo, uo, _) = elastic_scatter_openmc(
                e,
                Direction::new(0.0, 0.0, 1.0),
                236.0,
                kt,
                Some(t),
                false,
                |s| 2.0 * outram_mc_libs::rng::lcg::prn(s) - 1.0,
                &mut seed,
            );
            (eo.to_bits(), uo.u.to_bits(), uo.v.to_bits(), uo.w.to_bits())
        })
        .collect()
}

#[test]
fn the_lower_bound_is_a_setting_with_upstreams_default() {
    let t = table();
    assert_eq!(DBRC_DEFAULT_E_MIN_EV, 0.01);
    assert_eq!(t.energy_min(), 0.01);
    assert_eq!(t.clone().with_energy_min(1.0e-5).energy_min(), 1.0e-5);
    for e in [0.005, 0.5] {
        assert_eq!(run(&t, e), run(&t.clone().with_energy_min(0.01), e));
    }
}

#[test]
fn above_both_bounds_nothing_changes_and_between_them_the_path_does() {
    let t = table();
    let low = t.clone().with_energy_min(1.0e-5);
    assert_eq!(run(&t, 0.5), run(&low, 0.5), "above both bounds");
    assert_ne!(run(&t, 0.005), run(&low, 0.005), "between the bounds the sampler path differs");
}
