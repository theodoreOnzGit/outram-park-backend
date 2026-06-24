use fission_yields_data::prelude::Nuclide;
use openmc_libs::rng::lcg::Lcg64 as Rand64;
use uom::si::time::{second, year};
use uom::si::f64::*;

use crate::lagrangian_decay_simulator::monte_carlo_single_radionuclide_decay_simulator::SingleNuclideSimulatorMC;
use crate::prelude::decay_library::DecayLibrary;

/// basically we want to sample a time to live for 100000 particles
/// based on half life of 30s
///
/// See if the surviving_fraction of nuclides over time is 
/// equal to the analytical solution to within 1%
#[test]
fn stochastic_half_life_calculator(){

    let half_life = Time::new::<second>(30.0);
    let mut rng = Rand64::new(77);

    let mut time_to_live_vec: Vec<Time> = vec![];


    // now lets do time to live for 100000 particles 
    let number_of_particles = 100000;
    // we will also have a survivor counter 
    // that is if the particles live beyond a certain time, then it 
    // survives 

    for _i in 1..number_of_particles {
        let time_to_live = 
            SingleNuclideSimulatorMC::get_time_to_decay_stochastic(
                &mut rng, half_life
            );

        time_to_live_vec.push(time_to_live);
    }

    // now let's determine a function to see how many particles remain 
    fn determine_surviving_fraction(
        simulated_time: Time, 
        time_to_live_vec: &Vec<Time>) -> f64 {

        let mut survivor_counter = 0;
        let number_of_particles = time_to_live_vec.len();

        for time_to_live in time_to_live_vec {

            if *time_to_live > simulated_time {
                survivor_counter += 1;
            }

        }


        let surviving_fraction: f64 = 
            survivor_counter as f64/number_of_particles as f64;

        return surviving_fraction;

    }

    // let's first test at 10 seconds 

    
    // now do this for all times from 0 to 100 s 
    //
    // this is a vector of time vs surviving_fraction

    let decay_vec_30s_halflife_reference: Vec<(f64,f64)> = 
        vec![
        (0_f64,1_f64),
        (1_f64,0.977159968434246_f64),
        (2_f64,0.954841603910417_f64),
        (3_f64,0.933032991536807_f64),
        (4_f64,0.911722488558217_f64),
        (5_f64,0.890898718140339_f64),
        (6_f64,0.870550563296124_f64),
        (7_f64,0.850667160950856_f64),
        (8_f64,0.831237896142788_f64),
        (9_f64,0.812252396356236_f64),
        (10_f64,0.7937005259841_f64),
        (11_f64,0.775572380916867_f64),
        (12_f64,0.757858283255199_f64),
        (13_f64,0.740548776143282_f64),
        (14_f64,0.723634618720189_f64),
        (15_f64,0.707106781186548_f64),
        (16_f64,0.690956439983888_f64),
        (17_f64,0.675174973084095_f64),
        (18_f64,0.659753955386447_f64),
        (19_f64,0.64468515421979_f64),
        (20_f64,0.629960524947437_f64),
        (21_f64,0.615572206672458_f64),
        (22_f64,0.601512518041058_f64),
        (23_f64,0.587773953141804_f64),
        (24_f64,0.574349177498518_f64),
        (25_f64,0.561231024154687_f64),
        (26_f64,0.548412489847313_f64),
        (27_f64,0.535886731268147_f64),
        (28_f64,0.523647061410313_f64),
        (29_f64,0.511686945998388_f64),
        (30_f64,0.5_f64),
        (31_f64,0.488579984217123_f64),
        (32_f64,0.477420801955208_f64),
        (33_f64,0.466516495768404_f64),
        (34_f64,0.455861244279108_f64),
        (35_f64,0.44544935907017_f64),
        (36_f64,0.435275281648062_f64),
        (37_f64,0.425333580475428_f64),
        (38_f64,0.415618948071394_f64),
        (39_f64,0.406126198178118_f64),
        (40_f64,0.39685026299205_f64),
        (41_f64,0.387786190458434_f64),
        (42_f64,0.3789291416276_f64),
        (43_f64,0.370274388071641_f64),
        (44_f64,0.361817309360095_f64),
        (45_f64,0.353553390593274_f64),
        (46_f64,0.345478219991944_f64),
        (47_f64,0.337587486542048_f64),
        (48_f64,0.329876977693224_f64),
        (49_f64,0.322342577109895_f64),
        (50_f64,0.314980262473718_f64),
        (51_f64,0.307786103336229_f64),
        (52_f64,0.300756259020529_f64),
        (53_f64,0.293886976570902_f64),
        (54_f64,0.287174588749259_f64),
        (55_f64,0.280615512077343_f64),
        (56_f64,0.274206244923657_f64),
        (57_f64,0.267943365634073_f64),
        (58_f64,0.261823530705157_f64),
        (59_f64,0.255843472999194_f64),
        (60_f64,0.25_f64),
        (61_f64,0.244289992108562_f64),
        (62_f64,0.238710400977604_f64),
        (63_f64,0.233258247884202_f64),
        (64_f64,0.227930622139554_f64),
        (65_f64,0.222724679535085_f64),
        (66_f64,0.217637640824031_f64),
        (67_f64,0.212666790237714_f64),
        (68_f64,0.207809474035697_f64),
        (69_f64,0.203063099089059_f64),
        (70_f64,0.198425131496025_f64),
        (71_f64,0.193893095229217_f64),
        (72_f64,0.1894645708138_f64),
        (73_f64,0.185137194035821_f64),
        (74_f64,0.180908654680047_f64),
        (75_f64,0.176776695296637_f64),
        (76_f64,0.172739109995972_f64),
        (77_f64,0.168793743271024_f64),
        (78_f64,0.164938488846612_f64),
        (79_f64,0.161171288554947_f64),
        (80_f64,0.157490131236859_f64),
        (81_f64,0.153893051668115_f64),
        (82_f64,0.150378129510265_f64),
        (83_f64,0.146943488285451_f64),
        (84_f64,0.143587294374629_f64),
        (85_f64,0.140307756038672_f64),
        (86_f64,0.137103122461828_f64),
        (87_f64,0.133971682817037_f64),
        (88_f64,0.130911765352578_f64),
        (89_f64,0.127921736499597_f64),
        (90_f64,0.125_f64),
        (91_f64,0.122144996054281_f64),
        (92_f64,0.119355200488802_f64),
        (93_f64,0.116629123942101_f64),
        (94_f64,0.113965311069777_f64),
        (95_f64,0.111362339767542_f64),
        (96_f64,0.108818820412016_f64),
        (97_f64,0.106333395118857_f64),
        (98_f64,0.103904737017849_f64),
        (99_f64,0.101531549544529_f64),
        (100_f64,0.0992125657480125_f64),
        ];

    for (simulated_time, analytical_surviving_fraction_libreoffice) in 
        decay_vec_30s_halflife_reference {

            let test_surviving_fraction = 
                determine_surviving_fraction(
                    Time::new::<second>(simulated_time), &time_to_live_vec
                );

            approx::assert_relative_eq!(
                test_surviving_fraction,
                analytical_surviving_fraction_libreoffice,
                max_relative=9e-3
            );


        }




}

// in this case, thorium 232 goes via a series of decays, well known 
// to Pb208
#[test] 
fn decay_chain_th232(){

    let th232 = Nuclide::Th232;
    let mut decay_library = DecayLibrary::new();

    let mut th232_decay_simulation = SingleNuclideSimulatorMC
        ::new_decay_chain_simulation(th232,&mut decay_library);

    // as the simulation starts, 
    // I want to get timesteps, and slowly get the time to next decay 

    let mut timestep = Time::new::<year>(500.0);
    th232_decay_simulation.advance_timestep(timestep);

    // current nuclide is still Th232 
    assert_eq!(th232_decay_simulation.get_current_nuclide(), 
        th232);

    let time_to_next_decay = th232_decay_simulation.get_time_to_next_decay();

    // let's force a decay 
    //
    // we should get Radium 228
    timestep = time_to_next_decay;
    let (ra228, _half_life_info) = 
        th232_decay_simulation.advance_timestep(timestep);
    assert_eq!(ra228, 
        Nuclide::Ra228);

    // the next nuclide in the decay chain is Ra228 
    // this is the other way to get the current nuclide
    assert_eq!(th232_decay_simulation.current_nuclide, 
        Nuclide::Ra228);

    dbg!(&th232_decay_simulation);
    
    
    
    
    // force Ra228 → Ac228 → Th228 (stochastic time advances are RNG-dependent)
    let (current_nuclide, _) = th232_decay_simulation.force_decay_to_next_nuclide();
    assert_eq!(current_nuclide, Nuclide::Ac228);

    let (current_nuclide, _) = th232_decay_simulation.force_decay_to_next_nuclide();
    assert_eq!(current_nuclide, Nuclide::Th228);

    // force Th228 → Ra224
    let (current_nuclide, _) = th232_decay_simulation.force_decay_to_next_nuclide();
    assert_eq!(current_nuclide, Nuclide::Ra224);

    // force Ra224 → Rn220 → Po216 → Pb212
    let (current_nuclide, _) = th232_decay_simulation.force_decay_to_next_nuclide();
    assert_eq!(current_nuclide, Nuclide::Rn220);

    let (current_nuclide, _) = th232_decay_simulation.force_decay_to_next_nuclide();
    assert_eq!(current_nuclide, Nuclide::Po216);

    let (current_nuclide, _) = th232_decay_simulation.force_decay_to_next_nuclide();
    assert_eq!(current_nuclide, Nuclide::Pb212);

    // Pb212 → Bi212
    let (current_nuclide, _) = th232_decay_simulation.force_decay_to_next_nuclide();
    assert_eq!(current_nuclide, Nuclide::Bi212);

    // Bi212 branches: ~64% beta to Po212, ~36% alpha to Tl208
    let (bi212_daughter, _) = th232_decay_simulation.force_decay_to_next_nuclide();
    assert!(
        bi212_daughter == Nuclide::Po212 || bi212_daughter == Nuclide::Tl208,
        "expected Po212 or Tl208 after Bi212, got {:?}", bi212_daughter,
    );

    // Po212 or Tl208 → Pb208 (stable)
    let (current_nuclide, _) = th232_decay_simulation.force_decay_to_next_nuclide();
    assert_eq!(current_nuclide, Nuclide::Pb208);
    



}
