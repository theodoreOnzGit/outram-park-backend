// SPDX-License-Identifier: GPL-3.0

//! **Delayed-group-resolved `delayed-nu-fission` and `decay-rate` tallies on
//! real ENDF/B-VIII.0 U-235 data** - gh:#262.
//!
//! # Methodology
//!
//! Upstream resolves these two scores by delayed group analytically
//! (`src/tallies/tally_scoring.cpp:705`, `:773`): each group bin receives
//! `sigma_f * nu_d,k(E) * N * flux` (times `lambda_k` for `decay-rate`). The
//! reference here is that formula evaluated from the evaluation's own tables
//! (`DelayedData::nu_delayed_at`, `group_fraction`, `lambda`) and the
//! nuclide's microscopic fission cross section, at 6 energies from thermal to
//! 14 MeV. Checks:
//! 1. each `MacroXs::nu_fission_delayed_group[k]` equals that product to 1e-12
//!    relative;
//! 2. the groups sum to `nu_fission_delayed * sum_k p_k`, and the
//!    lambda-weighted groups to `decay_rate`, to 1e-12 relative (one loop
//!    fills both, so a mismatch means a group was dropped). **A first version
//!    of this check compared against `nu_fission_delayed` alone and failed at
//!    1.5e-7**: that identity needs `sum_k p_k = 1`, and the evaluation's
//!    tabulated shares sum to 1 only to that level. The criterion was
//!    ill-posed, not the code (per-group values matched to 2e-16); it now
//!    carries the data's `sum p_k` and separately asserts `|sum p_k - 1| < 1e-6`;
//! 3. a track-length tally with a `DelayedGroupFilter` over all six groups
//!    deposits `w d` times those values, and summed over the bins equals the
//!    same tally without the filter times `sum p_k` (see 2).
//!
//! # Results (2026-11-14, rustc 1.99.1 release, ENDF/B-VIII.0 U-235)
//!
//! Printed by the test (`--nocapture`): the worst relative deviations and the
//! sum of the evaluation's `p_k(E)` at each energy. Skips cleanly when the
//! tape is absent.

#![cfg(feature = "std")] // needs files, threads or reports (gh:#1605)

use outram_mc_libs::geometry::position::{Direction, Position};
use outram_mc_libs::material::material::{Material, NuclideComponent};
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::tally::filter::{CellFilter, DelayedGroupFilter, FilterKind};
use outram_mc_libs::tally::scoring::{flush_batch, score_track_length};
use outram_mc_libs::tally::tally::{ScoreType, Tally, TallyBin};

const TEMP: f64 = 293.6;
const ENERGIES: [f64; 6] = [0.0253, 1.0e3, 1.0e5, 1.0e6, 5.0e6, 1.4e7];

fn rel(a: f64, b: f64) -> f64 {
    if a == b {
        0.0
    } else {
        (a - b).abs() / a.abs().max(b.abs())
    }
}

#[test]
fn per_group_macro_xs_match_the_evaluations_own_tables() {
    let Some(p) = njoy_outram_park_fork::reference_data::reference_endf_or_skip(
        "n-092_U_235.endf",
        "U235 (delayed-group scoring)",
    ) else {
        return;
    };
    let nuc = Nuclide::from_endf_file(
        &p,
        "U235",
        uom::si::f64::ThermodynamicTemperature::new::<uom::si::thermodynamic_temperature::kelvin>(
            TEMP,
        ),
        5.0e-3,
    )
    .expect("reconstruct");
    let Some(d) = nuc.delayed() else {
        println!("[skip] no MF=1/455");
        return;
    };
    let n_dens = 0.045;
    let nuclides = vec![nuc.clone()];
    let mat = Material {
        id: 1,
        name: "U235".into(),
        temperature: TEMP,
        components: vec![NuclideComponent { nuclide_idx: 0, atom_density: n_dens }],
    };
    let n_groups = d.n_groups();
    assert_eq!(n_groups, 6);
    let (mut worst_group, mut worst_sum, mut worst_decay, mut worst_p) =
        (0.0_f64, 0.0_f64, 0.0_f64, 0.0_f64);
    for &e in &ENERGIES {
        let x = mat.macro_xs(e, &nuclides);
        let micro = nuc.xs_at_energy(e, TEMP);
        let nu_d = d.nu_delayed_at(e);
        let mut sum_p = 0.0;
        let (mut s_nu, mut s_dec) = (0.0, 0.0);
        for k in 0..n_groups {
            let pk = d.group_fraction(k, e);
            sum_p += pk;
            let want = n_dens * micro.fission * nu_d * pk;
            worst_group = worst_group.max(rel(x.nu_fission_delayed_group[k], want));
            let want_dec = want * d.lambda[k];
            worst_decay = worst_decay.max(rel(x.decay_rate_group[k], want_dec));
            s_nu += x.nu_fission_delayed_group[k];
            s_dec += x.decay_rate_group[k];
        }
        // The groups sum to `nu_fission_delayed * sum_k p_k`; `sum_k p_k` is
        // the evaluation's own tabulation and is 1 only to ~1e-7 (printed).
        worst_p = worst_p.max((sum_p - 1.0).abs());
        worst_sum = worst_sum
            .max(rel(s_nu, x.nu_fission_delayed * sum_p))
            .max(rel(s_dec, x.decay_rate));
        println!("E = {e:9.3e} eV: sum p_k = {sum_p:.6}, delayed nu-sigma_f = {:.5e}", x.nu_fission_delayed);
    }
    println!("worst: per-group {worst_group:.2e}, decay {worst_decay:.2e}, sums {worst_sum:.2e}");
    println!("worst |sum p_k - 1| = {worst_p:.2e}");
    assert!(worst_group < 1e-12 && worst_decay < 1e-12 && worst_sum < 1e-12);
    assert!(worst_p < 1e-6, "the evaluation's p_k should sum to 1");

    // Tally level: all six groups against the unfiltered score.
    let groups = DelayedGroupFilter::new((0..n_groups).collect()).unwrap();
    let mk = |filters: Vec<FilterKind>, nb: usize| Tally {
        id: 1,
        name: "t".into(),
        filters,
        scores: vec![ScoreType::DelayedNuFission],
        bins: vec![TallyBin::default(); nb],
    };
    let mut by_group = mk(
        vec![
            FilterKind::Cell(CellFilter { cell_indices: vec![0] }),
            FilterKind::DelayedGroup(groups),
        ],
        n_groups,
    );
    by_group.validate().unwrap();
    let mut total = mk(vec![FilterKind::Cell(CellFilter { cell_indices: vec![0] })], 1);
    let e = 1.0e6;
    let x = mat.macro_xs(e, &nuclides);
    let (mut bg, mut bt) = (vec![0.0; n_groups], vec![0.0; 1]);
    for t_and_b in [(&by_group, &mut bg), (&total, &mut bt)] {
        score_track_length(
            t_and_b.1, t_and_b.0, 0, 0, 0, e, 3.0, Position::ZERO, Some(&x), 0.7, None, 0.0,
            Direction::new(0.0, 0.0, 1.0),
        );
    }
    let sum_bins: f64 = bg.iter().sum();
    println!("tally: sum over groups {sum_bins:.6e}, unfiltered {:.6e}", bt[0]);
    let sp: f64 = (0..n_groups).map(|k| d.group_fraction(k, e)).sum();
    assert!(rel(sum_bins, bt[0] * sp) < 1e-12);
    for k in 0..n_groups {
        assert!(rel(bg[k], 0.7 * 3.0 * x.nu_fission_delayed_group[k]) < 1e-15);
    }
    flush_batch(&mut by_group, &mut bg);
    flush_batch(&mut total, &mut bt);
    assert_eq!(by_group.bins[0].count, 1);
}
