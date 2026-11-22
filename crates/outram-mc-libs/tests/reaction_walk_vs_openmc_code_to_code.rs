// SPDX-License-Identifier: AGPL-3.0-only
//! `classify_reaction` (default `ReactionOrder::OpenMc`) against OpenMC `8f79d9d0`
//! `scatter()`'s reaction walk (`physics.cpp:732-775`, extracted verbatim, 1600
//! fixed-xi cases from the real OpenMC LCG; harness
//! `verification_and_validation/upstream_openmc_reaction_walk/`).
//!
//! Pass criterion: the same group in every case, and for inelastic the same MT
//! from walking the levels with the returned cutoff by upstream's `>=` rule.
//! Result (2026-11-11, rustc 1.99.1): see the assertion counts printed by the
//! test; recorded in the harness README.

use outram_mc_libs::material::nuclide::MicroXS;
use outram_mc_libs::physics::scatter::{classify_reaction, ReactionGroup, ReactionOrder};

const REF: &str = include_str!(
    "../verification_and_validation/upstream_openmc_reaction_walk/reference.txt"
);

struct Case {
    abs: f64,
    xs: MicroXS,
    levels: Vec<(i32, f64)>,
}

fn case(name: &str) -> Case {
    // mirrors driver.cpp: (total, absorption, elastic, inelastic-class list)
    let (total, abs, el, list): (f64, f64, f64, Vec<(i32, f64)>) = match name {
        "heavy" => (
            9.39,
            1.5,
            4.0,
            vec![(5, 0.05), (16, 0.3), (17, 0.1), (22, 0.04), (51, 1.0), (52, 0.8), (53, 0.6), (91, 1.0)],
        ),
        "light" => (4.0, 0.2, 3.7, vec![(51, 0.05), (52, 0.03), (91, 0.02)]),
        "sab" => (6.0, 0.5, 5.0, vec![(51, 0.2), (91, 0.3)]),
        _ => (3.0, 0.0, 3.0, vec![]),
    };
    let g = |mt: i32| list.iter().filter(|r| r.0 == mt).map(|r| r.1).sum::<f64>();
    let levels: Vec<(i32, f64)> = list.iter().cloned().filter(|r| r.0 >= 51).collect();
    let xs = MicroXS {
        total,
        elastic: el,
        absorption: abs,
        mt5: g(5),
        n2n: g(16),
        n3n: g(17),
        other: g(22),
        inelastic: levels.iter().map(|r| r.1).sum(),
        ..Default::default()
    };
    Case { abs, xs, levels }
}

/// Methodology: see module docs. Measured: all cases agree (count printed).
#[test]
fn reaction_walk_matches_openmc_scatter() {
    let mut n = 0;
    for line in REF.lines() {
        let f: Vec<&str> = line.split_whitespace().collect();
        let c = case(f[0]);
        let cutoff = f64::from_bits(u64::from_str_radix(f[2], 16).unwrap());
        let mt: i32 = f[4].parse().unwrap();
        let g = classify_reaction(ReactionOrder::OpenMc, &c.xs, c.abs + cutoff);
        let want = match mt {
            2 => ReactionGroup::Elastic,
            5 => ReactionGroup::Mt5,
            16 => ReactionGroup::N2n,
            17 => ReactionGroup::N3n,
            22 => ReactionGroup::Other,
            _ => ReactionGroup::Inelastic(None),
        };
        match (g, want) {
            (ReactionGroup::Inelastic(Some(lc)), ReactionGroup::Inelastic(None)) => {
                let mut cum = 0.0;
                let mut got = -1;
                for (m, x) in &c.levels {
                    cum += x;
                    if cum >= lc {
                        got = *m;
                        break;
                    }
                }
                assert_eq!(got, mt, "{line}");
            }
            _ => assert_eq!(g, want, "{line}"),
        }
        n += 1;
    }
    assert_eq!(n, 1600);
    println!("reaction walk: {n}/{n} agree with OpenMC");
}

/// Legacy order keeps the old partition: elastic is the remainder, inelastic first.
#[test]
fn legacy_order_is_the_old_ladder() {
    let c = case("heavy");
    let g = classify_reaction(ReactionOrder::Legacy, &c.xs, c.abs + 0.1);
    assert_eq!(g, ReactionGroup::Inelastic(None));
    let g = classify_reaction(ReactionOrder::Legacy, &c.xs, c.xs.total - 1e-9);
    assert_eq!(g, ReactionGroup::Elastic);
}
