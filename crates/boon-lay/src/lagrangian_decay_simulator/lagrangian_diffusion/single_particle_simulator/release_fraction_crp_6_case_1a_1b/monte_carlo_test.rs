
use std::time::SystemTime;
use std::time::Duration;
use std::thread;
use std::sync::{Arc, Barrier, Mutex};

use crate::Nuclide;
use crate::prelude::SingleNuclideSimulatorMC;
use crate::prelude::decay_library::DecayLibrary;
use crate::lagrangian_decay_simulator::lagrangian_diffusion::single_particle_simulator::SingleParticleDiffusionSimulatorMC;
use uom::si::time::second;
use uom::si::f64::Time;
use uom::si::time::millisecond;
/// at each simulation, the simulator will run in the background 
///
/// now, challenge is, each simulator may run too fast, or slow 
/// depending on thread speed, relative to other simulations
///
///
/// the way to do it, according to ChatGPT5, is to use Arc barrier
/// 
pub fn run_decay_chain_simulation(
    thread_ptr: Arc<Mutex<(Vec<SingleNuclideSimulatorMC>,DecayLibrary)>>,
    thread_number: u8,
    barrier: Arc<Barrier>,
){

}
