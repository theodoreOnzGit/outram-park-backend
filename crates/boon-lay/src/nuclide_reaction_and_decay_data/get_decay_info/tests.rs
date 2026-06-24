
use crate::prelude::DecayType;
use crate::prelude::decay_library::DecayLibrary;
use crate::lagrangian_decay_simulator::StochasticDecayChain;
use fission_yields_data::prelude::Nuclide;
use uom::si::ratio::ratio;
use uom::si::time::minute;
use uom::si::f64::*;
use uom::si::time::{day, second, year};
#[test]
fn test_rng(){

    let some_seed = 4u128;
    let mut rng = openmc_libs::rng::lcg::Lcg64::new(some_seed);
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
    let gold_decay_data = decay_library.try_match_nuclides_to_decay_data(gold172).unwrap();

    // let me perform about 1000 decays 

    for _i in 0..10000 {

        let (new_target,_decay_type):
            (Nuclide, DecayType) = gold_decay_data
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

    
    dbg!(&(plat_counter,ir_counter,ir168_m_counter));
    // Au172 branching ratios: 0.49 Ir168, 0.49 Ir168m, 0.02 Pt171
    // With N=10000, expect ~4900 / ~4900 / ~200; allow ±10% (~490 counts)
    let tol = 600i32;
    assert!((plat_counter  as i32 - 200 ).abs() < tol, "Pt171 count {plat_counter} out of range");
    assert!((ir_counter    as i32 - 4900).abs() < tol, "Ir168  count {ir_counter}  out of range");
    assert!((ir168_m_counter as i32 - 4900).abs() < tol, "Ir168m count {ir168_m_counter} out of range");
}


#[test]
fn test_decay_chain(){
    // construct the library 
    let mut decay_library = DecayLibrary::new();

    // assume we have a decay library
    // and let's obtain a decay chain of U238 

    let u238 = Nuclide::U238;

    let decay_chain = StochasticDecayChain::new_single_stochastic_chain_from_nuclide(
        u238, &mut decay_library);

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
            },
            crate::prelude::HalfLifeAndDecayEnergyInfo::Unstable(
                half_life, _decay_energy
            ) => {
                test_decay_half_lives.push(*half_life);
            },
        }

    };
    // assert decay chains 
    assert_eq!(reference_decay_chain,test_decay_chain);
    // Half-lives aligned with the order above.
    // Values are typical literature values; units chosen for clarity per nuclide.
    // This reference list is vibe coded
    let reference_half_lives: Vec<Time> = vec![
        Time::new::<day>(24.10),        // Th-234
        Time::new::<minute>(1.17),      // Pa-234m (~73.9 s)
        Time::new::<year>(245_500.0),   // U-234
        Time::new::<year>(75_380.0),    // Th-230
        Time::new::<year>(1_600.0),     // Ra-226
        Time::new::<day>(3.8235),       // Rn-222
        Time::new::<minute>(3.10),      // Po-218
        Time::new::<minute>(26.8),      // Pb-214
        Time::new::<minute>(19.9),      // Bi-214
        Time::new::<second>(0.000164),  // Po-214 (~164 µs)
        Time::new::<year>(22.3),        // Pb-210
        Time::new::<day>(5.012),        // Bi-210
        Time::new::<day>(138.376),      // Po-210
        Time::new::<second>(f64::INFINITY),  // Pb-206 (stable)
    ];

    // for half lives, we need to do some rounding.
    for (i,half_life_ptr) in reference_half_lives.iter().enumerate() {
        let reference_half_life: Time = *half_life_ptr;
        let test_half_life: Time = test_decay_half_lives[i];

        // for stable nuclides, skip
        if reference_half_life.value.is_infinite() {
            return;
        }

        // let's get an error ratio 
        let error_ratio: Ratio = (test_half_life - reference_half_life).abs()/
            reference_half_life;

        // at most allow a 1% error (for Pa234m is errors are big)
        let tolerance: Ratio = Ratio::new::<ratio>(1e-2);

        if error_ratio > tolerance {

            dbg!(&test_decay_chain[i]);
            dbg!(&error_ratio);
            panic!("debugging decay chain: error too big");
        }
    }


}
