use crate::lagrangian_decay_simulator::StochasticDecayChain;
use crate::prelude::decay_library::DecayLibrary;
use crate::prelude::DecayType;
use fission_yields_data::prelude::Nuclide;
use uom::si::f64::*;
use uom::si::ratio::ratio;
use uom::si::time::minute;
use uom::si::time::{day, second, year};
#[test]
fn test_rng() {
    let some_seed = 4u128;
    let mut rng = outram_mc_libs::rng::lcg::Lcg64::new(some_seed);
    println!("Your random number is: {}", rng.rand_float());
    println!("Your random number is: {}", rng.rand_float());
    println!("Your random number is: {}", rng.rand_float());
    println!("Your random number is: {}", rng.rand_float());
    // let's consider the decay of Au172
    // which will branch into 3 types of decay
    //
    // Here is the data:
    // <decay type="alpha" target="Ir168" branching_ratio="0.49"/>
    // <decay type="alpha" target="Ir168_m1" branching_ratio="0.49"/>
    // <decay type="p" target="Pt171" branching_ratio="0.02"/>

    //
    let gold172 = Nuclide::Au172;
    // basically, i expect that from 1000 trials, I should get around
    // 20 times the target of Pt171,
    // 490 times the target of Ir168
    // and 490 times the target of Ir168m

    let plat171 = Nuclide::Pt171;
    let ir168 = Nuclide::Ir168;
    let ir168m = Nuclide::Ir168m;

    let mut plat_counter = 0;
    let mut ir_counter = 0;
    let mut ir168_m_counter = 0;
    // construct the library
    let decay_library = DecayLibrary::new();
    let gold_decay_data = decay_library
        .try_match_nuclides_to_decay_data(gold172)
        .unwrap();

    // let me perform about 1000 decays

    for _i in 0..10000 {
        let (new_target, _decay_type): (Nuclide, DecayType) = gold_decay_data
            .get_next_target_nuclide_with_rng(&mut rng)
            .unwrap();

        if new_target == plat171 {
            plat_counter += 1;
        } else if new_target == ir168 {
            ir_counter += 1;
        } else if new_target == ir168m {
            ir168_m_counter += 1;
        };
    }

    dbg!(&(plat_counter, ir_counter, ir168_m_counter));
    // Au172 branching ratios: 0.49 Ir168, 0.49 Ir168m, 0.02 Pt171
    // With N=10000, expect ~4900 / ~4900 / ~200; allow ±10% (~490 counts)
    let tol = 600i32;
    assert!(
        (plat_counter as i32 - 200).abs() < tol,
        "Pt171 count {plat_counter} out of range"
    );
    assert!(
        (ir_counter as i32 - 4900).abs() < tol,
        "Ir168  count {ir_counter}  out of range"
    );
    assert!(
        (ir168_m_counter as i32 - 4900).abs() < tol,
        "Ir168m count {ir168_m_counter} out of range"
    );
}

#[test]
fn test_decay_chain() {
    // construct the library
    let mut decay_library = DecayLibrary::new();

    // assume we have a decay library
    // and let's obtain a decay chain of U238

    let u238 = Nuclide::U238;

    let decay_chain =
        StochasticDecayChain::new_single_stochastic_chain_from_nuclide(u238, &mut decay_library);

    dbg!(&decay_chain);

    // now, the correct decay chain for this test is:
    //
    let reference_decay_chain: Vec<Nuclide> = vec![
        Nuclide::Th234,
        Nuclide::Pa234m,
        Nuclide::U234,
        Nuclide::Th230,
        Nuclide::Ra226,
        Nuclide::Rn222,
        Nuclide::Po218,
        Nuclide::Pb214,
        Nuclide::Bi214,
        Nuclide::Po214,
        Nuclide::Pb210,
        Nuclide::Bi210,
        Nuclide::Po210,
        // Pb206 is stable
        Nuclide::Pb206,
    ];

    let mut test_decay_chain: Vec<Nuclide> = vec![];
    let mut test_decay_half_lives: Vec<Time> = vec![];

    for (subsequent_nuclide, half_life_information) in decay_chain.iter() {
        test_decay_chain.push(*subsequent_nuclide);

        match half_life_information {
            crate::prelude::HalfLifeAndDecayEnergyInfo::Stable => {
                // if it's stable, push INFINITY
                test_decay_half_lives.push(Time::new::<second>(f64::INFINITY));
            }
            crate::prelude::HalfLifeAndDecayEnergyInfo::Unstable(half_life, _decay_energy) => {
                test_decay_half_lives.push(*half_life);
            }
        }
    }
    // assert decay chains
    assert_eq!(reference_decay_chain, test_decay_chain);
    // Half-lives aligned with the order above.
    // Values are typical literature values; units chosen for clarity per nuclide.
    // This reference list is vibe coded
    let reference_half_lives: Vec<Time> = vec![
        Time::new::<day>(24.10),            // Th-234
        Time::new::<minute>(1.17),          // Pa-234m (~73.9 s)
        Time::new::<year>(245_500.0),       // U-234
        Time::new::<year>(75_380.0),        // Th-230
        Time::new::<year>(1_600.0),         // Ra-226
        Time::new::<day>(3.8235),           // Rn-222
        Time::new::<minute>(3.10),          // Po-218
        Time::new::<minute>(26.8),          // Pb-214
        Time::new::<minute>(19.9),          // Bi-214
        Time::new::<second>(0.000164),      // Po-214 (~164 µs)
        Time::new::<year>(22.3),            // Pb-210
        Time::new::<day>(5.012),            // Bi-210
        Time::new::<day>(138.376),          // Po-210
        Time::new::<second>(f64::INFINITY), // Pb-206 (stable)
    ];

    // for half lives, we need to do some rounding.
    for (i, half_life_ptr) in reference_half_lives.iter().enumerate() {
        let reference_half_life: Time = *half_life_ptr;
        let test_half_life: Time = test_decay_half_lives[i];

        // for stable nuclides, skip
        if reference_half_life.value.is_infinite() {
            return;
        }

        // let's get an error ratio
        let error_ratio: Ratio = (test_half_life - reference_half_life).abs() / reference_half_life;

        // at most allow a 1% error (for Pa234m is errors are big)
        let tolerance: Ratio = Ratio::new::<ratio>(1e-2);

        if error_ratio > tolerance {
            dbg!(&test_decay_chain[i]);
            dbg!(&error_ratio);
            panic!("debugging decay chain: error too big");
        }
    }
}

// ---------------------------------------------------------------------------
// gh:#538: the decay sampler's reachable panics and the spontaneous-fission
// self-target. Methodology: each case is built from the bundled OpenMC chain
// data (or, for the rounding gap, a hand-made two-branch record), and the
// result is compared with how OpenMC's own depletion chain reads the same
// data (`openmc/deplete/chain.py`, `from_endf` and `form_matrix`). Results
// (2026-11-10, release): all pass. Before the fix (develop `a836e2f07`),
// `he5_alpha_without_target_does_not_panic` and
// `rounding_gap_selects_largest_branch_not_todo` panicked, and
// `u238_spontaneous_fission_has_no_tracked_daughter` failed because the SF
// branch returned U-238 as its own daughter.
// ---------------------------------------------------------------------------

use super::DecaySample;
use crate::nuclide_reaction_and_decay_data::DecayData;
use crate::prelude::{HalfLifeAndDecayEnergyInfo, NuclideReactionAndDecayData};

/// A deviate in the middle of the branch at `index`, from the cumulative
/// ratios in evaluation order.
fn u_inside_branch(data: &NuclideReactionAndDecayData, index: usize) -> f64 {
    let branches = data.get_decay_branch_info();
    let before: f64 = branches[..index]
        .iter()
        .map(|(br, _, _)| br.get::<ratio>())
        .sum();
    before + 0.5 * branches[index].0.get::<ratio>()
}

#[test]
fn u238_spontaneous_fission_has_no_tracked_daughter() {
    let lib = DecayLibrary::new();
    let u238 = lib.try_match_nuclides_to_decay_data(Nuclide::U238).unwrap();
    let branches = u238.get_decay_branch_info();
    let sf_index = branches
        .iter()
        .position(|(_, _, t)| *t == DecayType::SpontaneousFission)
        .expect("the chain carries a U-238 sf branch");
    // The chain file names the parent itself as the sf "target".
    assert_eq!(branches[sf_index].1, Some(Nuclide::U238));

    let u = u_inside_branch(&u238, sf_index);
    assert_eq!(
        u238.sample_decay_with_float(u),
        DecaySample::NoTrackedDaughter {
            decay_type: DecayType::SpontaneousFission
        }
    );
    // The old accessor no longer hands back the parent as a daughter.
    assert_eq!(u238.get_next_target_nuclide_with_float(u), None);

    // The alpha branch is unchanged.
    let alpha_index = branches
        .iter()
        .position(|(_, _, t)| *t == DecayType::Alpha)
        .unwrap();
    let ua = u_inside_branch(&u238, alpha_index);
    assert_eq!(
        u238.get_next_target_nuclide_with_float(ua),
        Some((Nuclide::Th234, DecayType::Alpha))
    );
}

#[test]
fn he5_alpha_without_target_does_not_panic() {
    let lib = DecayLibrary::new();
    let he5 = lib.try_match_nuclides_to_decay_data(Nuclide::He5).unwrap();
    assert_eq!(he5.get_decay_branch_info().len(), 1);
    assert_eq!(he5.get_decay_branch_info()[0].1, None);
    assert_eq!(
        he5.sample_decay_with_float(0.5),
        DecaySample::NoTrackedDaughter {
            decay_type: DecayType::Alpha
        }
    );
    assert_eq!(he5.get_next_target_nuclide_with_float(0.5), None);
}

#[test]
fn rounding_gap_selects_largest_branch_not_todo() {
    // Two branches summing to 0.8: a draw at 0.9 is past the running sum.
    // OpenMC's `from_endf` gives the residual to the largest branch.
    let data = NuclideReactionAndDecayData {
        nuclide: Nuclide::Au172,
        half_life_information: HalfLifeAndDecayEnergyInfo::Unstable(
            Time::new::<second>(1.0),
            Energy::new::<uom::si::energy::electronvolt>(1.0),
        ),
        decay_information: vec![
            DecayData {
                decay_type: DecayType::Proton,
                target: Some(Nuclide::Pt171),
                branching_ratio: Ratio::new::<ratio>(0.3),
            },
            DecayData {
                decay_type: DecayType::Alpha,
                target: Some(Nuclide::Ir168),
                branching_ratio: Ratio::new::<ratio>(0.5),
            },
        ],
    };
    assert_eq!(
        data.get_next_target_nuclide_with_float(0.9),
        Some((Nuclide::Ir168, DecayType::Alpha))
    );
    // Inside the sum, the sequential subtraction is unchanged.
    assert_eq!(
        data.get_next_target_nuclide_with_float(0.1),
        Some((Nuclide::Pt171, DecayType::Proton))
    );
    assert_eq!(
        data.get_next_target_nuclide_with_float(0.79),
        Some((Nuclide::Ir168, DecayType::Alpha))
    );
}

/// Pins the assumption the rounding-gap rule rests on: in the bundled
/// libraries every multi-branch nuclide's ratios sum to 1 within 1e-9
/// (OpenMC's `math.isclose` default), so the gap is rounding, never a missing
/// branch. Counts every nuclide reachable by ZAID and the tally of target-less
/// non-sf branches (He-5's is expected to be the only one).
#[test]
fn branching_ratios_sum_to_one_within_1e_9() {
    let lib = DecayLibrary::new();
    let mut multi_branch = 0usize;
    let mut seen: Vec<Nuclide> = vec![];
    let mut worst = 0.0_f64;
    let mut targetless = vec![];
    // Z >= 112 has no bundled data and its lookup is still `todo!()`.
    for z in 1u32..=111 {
        for a in 1u32..=300 {
            for offset in [0u32, 400, 800, 1200] {
                let Some(n) = Nuclide::try_get_nuclide_from_zaid(1000 * z + a + offset) else {
                    continue;
                };
                let (zz, _) = n.get_z_a();
                if zz == 0 || zz > 111 || seen.contains(&n) {
                    continue;
                }
                seen.push(n);
                let Some(d) = lib.try_match_nuclides_to_decay_data(n) else {
                    continue;
                };
                let b = d.get_decay_branch_info();
                for (_, t, dt) in b.iter() {
                    if t.is_none() && !dt.has_no_tracked_daughter() {
                        targetless.push(n);
                    }
                }
                if b.len() > 1 {
                    multi_branch += 1;
                    let sum: f64 = b.iter().map(|(br, _, _)| br.get::<ratio>()).sum();
                    worst = worst.max((sum - 1.0).abs());
                }
            }
        }
    }
    println!("multi-branch nuclides {multi_branch}, worst |sum - 1| {worst:e}, target-less {targetless:?}");
    assert!(multi_branch > 100);
    assert!(worst <= 1e-9, "worst |sum - 1| = {worst:e}");
    assert_eq!(targetless, vec![Nuclide::He5]);
}

#[test]
fn stochastic_chain_from_u238_does_not_panic() {
    let mut lib = DecayLibrary::new();
    for _ in 0..200 {
        let chain = StochasticDecayChain::new_single_stochastic_chain_from_nuclide(
            Nuclide::U238,
            &mut lib,
        );
        // Either the alpha path to stable Pb-206, or an sf ending (empty
        // chain): never U-238 as its own daughter.
        if let Some((first, _)) = chain.nuclides_and_decay_data_vec.first() {
            assert_ne!(*first, Nuclide::U238);
        }
    }
}

/// gh:#538 (found while sweeping): the lookup for Z = 0 and Z >= 112 was
/// `todo!()`; it returns `None`, as `decay-data.md` says. Every nuclide the
/// ZAID table knows, at any Z, must look up without panicking.
#[test]
fn lookup_never_panics_for_any_known_nuclide() {
    let lib = DecayLibrary::new();
    let mut heavy = 0usize;
    for zaid in 0u32..=120_000 + 1_300 {
        let Some(n) = Nuclide::try_get_nuclide_from_zaid(zaid) else {
            continue;
        };
        let r = lib.try_match_nuclides_to_decay_data(n);
        let (z, _) = n.get_z_a();
        if z == 0 || z >= 112 {
            heavy += 1;
            assert!(r.is_none(), "{n:?} has no bundled data");
        }
    }
    println!("nuclides with Z = 0 or Z >= 112 in the ZAID table: {heavy}");
}
