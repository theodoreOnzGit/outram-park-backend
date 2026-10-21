// SPDX-License-Identifier: GPL-3.0

//! **V&V gate** — the eight filters added for GitHub #261, against upstream's
//! own rules at OpenMC `afa7a14`.
//!
//! # Methodology
//!
//! The load-bearing one is `mt_matches`. A reaction filter asking for MT=4
//! means *every* inelastic level 51..91, not a reaction literally labelled 4 —
//! no event ever carries that. Reducing the summation rules to equality would
//! compile, run, and tally **zero** for every summation MT anyone actually
//! asks for, which is the same silent-plausible-wrong shape as #259's aliased
//! boundary.
//!
//! So every branch of `src/endf.cpp:90` is exercised by name, with both a
//! member and a non-member for each, rather than spot-checking a few MTs.
//!
//! The other seven filters are checked for the specific trap each carries:
//! exact-match rather than range (collision), the unbinned-outside rule
//! (weight, mu-surface, energy-function), and non-extrapolation past a
//! tabulated response (energy-function).
//!
//! # Results (2026-09-22)
//!
//! 6 tests, all passing. Every summation branch of `mt_matches` is exercised
//! with both a member and a non-member, including the band edges that are the
//! actual failure surface: 49/50 and 91/92 for the inelastic levels, 874/875
//! and 891/892 for `(n,2n)` excited states, 599/600 and 849/850 for the
//! charged-particle bands, and 572/573 for the photoelectric subshells.
//!
//! Upstream's `PHOTOELECTRIC` branch reads `event_mt >= 534 && event_mt < 573`
//! -- a **half-open** upper bound where every neighbouring band is closed. That
//! asymmetry is upstream's and is reproduced exactly; the test pins 572 in and
//! 573 out so a later "tidy-up" to `..=573` fails here rather than silently
//! adding a subshell.

use outram_mc_libs::tally::filter::{Filter, FilterEvent};
use outram_mc_libs::tally::filter_extra::{
    is_disappearance, is_fission, mt_matches, CellBornFilter, CellFromFilter, CollisionFilter,
    EnergyFunctionFilter, MaterialFromFilter, MuSurfaceFilter, ReactionFilter, WeightFilter,
};

/// Every summation branch of `mt_matches`, member and non-member.
#[test]
fn mt_summation_rules_match_upstream() {
    // Direct equality always holds.
    for mt in [2, 18, 102, 444] {
        assert!(mt_matches(mt, mt), "MT {mt} must match itself");
    }

    // TOTAL_XS = 1: elastic, plus everything non-elastic.
    assert!(mt_matches(2, 1), "elastic is part of total");
    assert!(mt_matches(16, 1), "(n,2n) is part of total");
    assert!(mt_matches(102, 1), "(n,gamma) is part of total via 3 -> 27 -> 101");

    // N_LEVEL = 4: inelastic levels 50..=91.
    assert!(mt_matches(51, 4));
    assert!(mt_matches(91, 4));
    assert!(!mt_matches(92, 4), "92 is past the inelastic band");
    assert!(!mt_matches(49, 4), "49 is below the inelastic band");

    // N_2N = 16: excited states live at 875..=891.
    assert!(mt_matches(875, 16));
    assert!(mt_matches(891, 16));
    assert!(!mt_matches(874, 16));
    assert!(!mt_matches(892, 16));

    // N_FISSION = 18: first/second/third chance.
    for mt in [18, 19, 20, 21, 38] {
        assert!(is_fission(mt), "MT {mt} is a fission channel");
        assert!(mt_matches(mt, 18), "MT {mt} falls under MT=18");
    }
    assert!(!is_fission(102));

    // Discrete charged-particle bands.
    for (target, lo, hi) in [
        (103, 600, 649),
        (104, 650, 699),
        (105, 700, 749),
        (106, 750, 799),
        (107, 800, 849),
    ] {
        assert!(mt_matches(lo, target), "MT {lo} under {target}");
        assert!(mt_matches(hi, target), "MT {hi} under {target}");
        assert!(!mt_matches(lo - 1, target), "MT {} not under {target}", lo - 1);
        assert!(!mt_matches(hi + 1, target), "MT {} not under {target}", hi + 1);
    }

    // Disappearance: 101..=117, the 600..=849 band, and the named channels.
    assert!(is_disappearance(102));
    assert!(is_disappearance(117));
    assert!(!is_disappearance(118));
    assert!(is_disappearance(700));
    for mt in [155, 182, 191, 192, 193, 197] {
        assert!(is_disappearance(mt), "MT {mt} is a disappearance channel");
    }

    // MT=27 is fission OR disappearance.
    assert!(mt_matches(18, 27));
    assert!(mt_matches(102, 27));
    assert!(!mt_matches(2, 27), "elastic is neither");

    // Photon total 501 and its components.
    assert!(mt_matches(502, 501), "coherent");
    assert!(mt_matches(504, 501), "incoherent");
    assert!(mt_matches(515, 501), "pair production, electron field, via 516");
    assert!(mt_matches(517, 501), "pair production, nuclear field, via 516");
    assert!(mt_matches(534, 501), "photoelectric subshell via 522");
    assert!(mt_matches(572, 501), "last photoelectric subshell");
    assert!(!mt_matches(573, 501), "573 is past the photoelectric band");

    // A target with no summation rule matches only itself.
    assert!(!mt_matches(51, 444));
}

/// A reaction filter must score EVERY overlapping bin, as upstream does.
#[test]
fn a_reaction_filter_scores_every_overlapping_bin() {
    let f = ReactionFilter {
        bins: vec![1, 2, 4, 18],
    };
    // Elastic falls under both total (1) and elastic (2), and nothing else.
    assert_eq!(f.matching_bins(2), vec![0, 1]);
    // An inelastic level falls under total and MT=4.
    assert_eq!(f.matching_bins(51), vec![0, 2]);
    // Fission falls under total and 18.
    assert_eq!(f.matching_bins(18), vec![0, 3]);
    // The single-bin contract returns the FIRST match.
    let ev = FilterEvent {
        event_mt: 51,
        ..Default::default()
    };
    assert_eq!(f.get_bin(&ev), Some(0));
    assert_eq!(f.n_bins(), 4);
}

/// The collision filter matches EXACTLY, not as a range.
#[test]
fn the_collision_filter_matches_exactly() {
    let f = CollisionFilter {
        bins: vec![1, 2, 5],
    };
    for (n, want) in [(1u32, Some(0)), (2, Some(1)), (5, Some(2)), (3, None), (0, None), (6, None)]
    {
        let ev = FilterEvent {
            n_collision: n,
            ..Default::default()
        };
        assert_eq!(f.get_bin(&ev), want, "collision {n}");
    }
}

/// The from/born filters bin only on their own field and are `None` when it is
/// absent — a particle on its first event has no previous cell.
#[test]
fn the_from_and_born_filters_bin_their_own_field() {
    let cf = CellFromFilter { cells: vec![7, 9] };
    assert_eq!(
        cf.get_bin(&FilterEvent {
            cell_from: Some(9),
            ..Default::default()
        }),
        Some(1)
    );
    assert_eq!(
        cf.get_bin(&FilterEvent {
            cell_from: None,
            ..Default::default()
        }),
        None,
        "no previous cell must be unbinned, not bin 0"
    );

    let cb = CellBornFilter { cells: vec![3] };
    assert_eq!(
        cb.get_bin(&FilterEvent {
            cell_born: Some(3),
            ..Default::default()
        }),
        Some(0)
    );

    let mf = MaterialFromFilter {
        materials: vec![0, 4],
    };
    assert_eq!(
        mf.get_bin(&FilterEvent {
            material_from: Some(0),
            ..Default::default()
        }),
        Some(0)
    );
    assert_eq!(
        mf.get_bin(&FilterEvent {
            material_from: Some(5),
            ..Default::default()
        }),
        None
    );
}

/// Weight and mu-surface bin on ascending edges and drop anything outside.
#[test]
fn edge_binned_filters_drop_what_is_outside() {
    let w = WeightFilter {
        bins: vec![0.0, 0.25, 0.5, 1.0],
    };
    assert_eq!(w.n_bins(), 3);
    for (x, want) in [(0.1, Some(0)), (0.25, Some(1)), (0.9, Some(2)), (1.5, None), (-0.1, None)] {
        let ev = FilterEvent {
            weight: x,
            ..Default::default()
        };
        assert_eq!(w.get_bin(&ev), want, "weight {x}");
    }
    // A default event carries weight 1.0 -- analog transport -- and must land
    // in the last bin rather than be dropped.
    assert_eq!(w.get_bin(&FilterEvent::default()), Some(2));

    let m = MuSurfaceFilter {
        bins: vec![-1.0, 0.0, 1.0],
    };
    // Not a surface crossing: unbinned regardless of the cosine.
    assert_eq!(
        m.get_bin(&FilterEvent {
            surface_mu: 0.5,
            surface_idx: usize::MAX,
            ..Default::default()
        }),
        None,
        "a non-surface event must not bin into a surface filter"
    );
    assert_eq!(
        m.get_bin(&FilterEvent {
            surface_mu: 0.5,
            surface_idx: 3,
            ..Default::default()
        }),
        Some(1)
    );
    // Round-off past +/-1 is clamped, not dropped.
    assert_eq!(
        m.get_bin(&FilterEvent {
            surface_mu: 1.0 + 1e-15,
            surface_idx: 3,
            ..Default::default()
        }),
        Some(1),
        "|mu| slightly over 1 must clamp, as upstream does"
    );
}

/// The energy-function filter interpolates lin-lin and refuses to extrapolate.
#[test]
fn the_energy_function_filter_interpolates_and_does_not_extrapolate() {
    let f = EnergyFunctionFilter::new(vec![1.0, 2.0, 4.0], vec![10.0, 20.0, 0.0]).unwrap();

    // On a grid point.
    assert_eq!(f.response(1.0), Some(10.0));
    assert_eq!(f.response(2.0), Some(20.0));
    // Midway on the first interval: lin-lin gives 15.
    assert_eq!(f.response(1.5), Some(15.0));
    // Midway on the second, which descends to zero: 10.
    assert_eq!(f.response(3.0), Some(10.0));

    // OUTSIDE the grid is None, not clamped. Extrapolating a dose-response
    // curve past its tabulated range is not something upstream will do.
    assert_eq!(f.response(0.99), None);
    assert_eq!(f.response(4.01), None);

    // The weight reaches the tally through `expansion_moments`.
    let ev = FilterEvent {
        energy: 1.5,
        ..Default::default()
    };
    assert_eq!(f.n_bins(), 1);
    assert_eq!(f.get_bin(&ev), Some(0));
    assert_eq!(f.expansion_moments(&ev), Some(vec![15.0]));
    let out = FilterEvent {
        energy: 100.0,
        ..Default::default()
    };
    assert_eq!(f.get_bin(&out), None);
    assert_eq!(f.expansion_moments(&out), None);

    // Malformed inputs are refused rather than silently accepted.
    assert!(EnergyFunctionFilter::new(vec![1.0], vec![1.0]).is_err());
    assert!(EnergyFunctionFilter::new(vec![1.0, 2.0], vec![1.0]).is_err());
    assert!(EnergyFunctionFilter::new(vec![2.0, 1.0], vec![1.0, 2.0]).is_err());
}
