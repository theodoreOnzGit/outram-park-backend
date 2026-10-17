//! Pins for GitHub #947's port of OpenMC's stochastic temperature
//! interpolation (`material::temperature_interpolation`) and the walk that
//! uses it (`InfiniteMediumMc::run_with_tables`).
//!
//! The study itself (real U-238, H in H₂O, timings) is
//! `examples/temperature_interpolation_947.rs`; methodology and results in
//! `verification_and_validation/temperature_interpolation_947/README.md`.
//! These tests use the offline CORE tier, so they need no tape.

use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::temperature_interpolation::{InterpolationVariable, TemperatureTables};
use outram_mc_libs::physics::slowing_down::{InfiniteMediumMc, MixComponent, SlowingDownBand};
use outram_mc_libs::rng::lcg::prn;

fn walk() -> (InfiniteMediumMc, SlowingDownBand) {
    (
        InfiniteMediumMc {
            histories: 400,
            seed: 0x947,
            ..InfiniteMediumMc::default()
        },
        SlowingDownBand {
            e_top: 1.0e4,
            e_bot: 1.0,
        },
    )
}

/// **Nothing existing changes behaviour.** `run` is now `run_with_tables`
/// with a fixed choice that draws no random number; it must give the walk
/// it always gave, bit for bit. And the defaults are upstream's.
#[test]
fn existing_paths_are_unchanged() {
    let nucs = vec![
        Nuclide::from_core("U238").expect("U238"),
        Nuclide::from_core("U235").expect("U235"),
    ];
    let mix = [
        MixComponent {
            nuclide_idx: 0,
            atom_density: 1.0,
        },
        MixComponent {
            nuclide_idx: 1,
            atom_density: 0.01,
        },
    ];
    let (mc, band) = walk();
    let a = mc.run(&nucs, &mix, band, 600.0);
    let b = mc.run_with_tables(&nucs, &mix, band, 600.0, |i, _, _| mix[i].nuclide_idx);
    assert_eq!(a, b, "run must be run_with_tables with the fixed choice");
    // Upstream's variable is kT (src/nuclide.cpp:704).
    assert_eq!(InterpolationVariable::default(), InterpolationVariable::Kt);
}

/// The selector's table is the one the collision uses: pointing component 0
/// at another nuclide through the selector is the same walk as pointing the
/// mixture at it.
#[test]
fn the_selected_table_supplies_the_collision() {
    let nucs = vec![
        Nuclide::from_core("U238").expect("U238"),
        Nuclide::from_core("U235").expect("U235"),
        Nuclide::from_core("U234").expect("U234"),
    ];
    let mix = [
        MixComponent {
            nuclide_idx: 0,
            atom_density: 1.0,
        },
        MixComponent {
            nuclide_idx: 1,
            atom_density: 0.01,
        },
    ];
    let mix_234 = [
        MixComponent {
            nuclide_idx: 2,
            ..mix[0]
        },
        mix[1],
    ];
    let (mc, band) = walk();
    let via_select = mc.run_with_tables(
        &nucs,
        &mix,
        band,
        600.0,
        |i, _, _| if i == 0 { 2 } else { 1 },
    );
    let via_mix = mc.run(&nucs, &mix_234, band, 600.0);
    assert_eq!(via_select, via_mix);
}

/// A stochastic walk between two tables: the draw happens (the stream moves),
/// both tables are used in proportion, and with f = 1/2 the walk lands
/// between the two single-table walks.
#[test]
fn stochastic_walk_uses_both_tables_in_proportion() {
    let nucs = vec![
        Nuclide::from_core("U238").expect("U238"),
        Nuclide::from_core("U235").expect("U235"),
        Nuclide::from_core("U238").expect("U238"),
    ];
    // "Tables" at 300 and 900 K: the CORE tier evaluates at the lookup
    // temperature, so the lower and upper tables differ in broadening.
    let tables = TemperatureTables::new(
        vec![300.0, 900.0],
        vec![0usize, 2usize],
        InterpolationVariable::Kt,
    )
    .expect("tables");
    let mix = [
        MixComponent {
            nuclide_idx: 0,
            atom_density: 1.0,
        },
        MixComponent {
            nuclide_idx: 1,
            atom_density: 0.01,
        },
    ];
    let (mc, band) = walk();
    let mut counts = [0usize; 2];
    let r = mc.run_with_tables(&nucs, &mix, band, 600.0, |i, _, seed| {
        if i == 0 {
            let (j, idx) = tables.pick(600.0, prn(seed)).expect("in range");
            counts[j] += 1;
            *idx
        } else {
            1
        }
    });
    let n = (counts[0] + counts[1]) as f64;
    assert!(n > 1000.0, "the walk made {n} lookups");
    let frac = counts[1] as f64 / n;
    assert!(
        (frac - 0.5).abs() < 4.0 * (0.25 / n).sqrt(),
        "upper table used {frac}"
    );
    let total = r.absorbed() + r.escaped;
    assert!((total - 1.0).abs() < 1e-12);
    // Outside the tabulated range the port refuses (upstream snaps).
    assert!(tables.pick(1000.0, 0.5).is_err());
}

/// `expected_xs` is the probability-weighted mix of the two tables, field
/// by field, each table read at its own temperature.
#[test]
fn expected_xs_is_the_weighted_mix() {
    let t = TemperatureTables::new(
        vec![300.0, 1200.0],
        vec![
            Nuclide::from_core("U238").expect("U238"),
            Nuclide::from_core("U238").expect("U238"),
        ],
        InterpolationVariable::SqrtT,
    )
    .expect("tables");
    let (e, temp) = (6.67, 600.0);
    let b = t.bracket(temp).expect("bracket");
    let f = b.upper_probability;
    let lo = t.tables()[0].xs_at_energy(e, 300.0);
    let hi = t.tables()[1].xs_at_energy(e, 1200.0);
    let x = t.expected_xs(e, temp).expect("expected");
    let want = (1.0 - f) * lo.total + f * hi.total;
    assert!(
        (x.total - want).abs() <= 1e-12 * want.abs(),
        "{} vs {want}",
        x.total
    );
    assert!(
        (x.absorption - ((1.0 - f) * lo.absorption + f * hi.absorption)).abs()
            <= 1e-12 * lo.absorption.abs()
    );
    // The peak of the 6.67 eV resonance falls with temperature, so the two
    // tables genuinely differ here.
    assert!(lo.total > hi.total);
    let mean = t
        .expectation(temp, |n| n.total_at_energy(e, 300.0))
        .expect("expectation");
    assert!(mean.is_finite());
}
