

/// this code here is meant to simulate decay chains 
/// Basically, it takes information from the nuclide, converts it into decay 
/// data and then terminates it as it reaches stability
///
///  
pub mod stochastic_decay_chain;
pub use stochastic_decay_chain::*;

/// this code here is meant to simulate decay chains 
/// basically, a single particle is simulated
///
/// The nuclide will be supplied into the simulator, 
/// the simulator will then determine the decay chain 
/// and how much time there is to decay.
///
/// The simulator, can of course, determine the radiation as well 
/// released, but that is another time.
///
/// this is not really vibe coded (still used chatgpt 5 advise on 
/// some algorithms)
pub mod monte_carlo_single_radionuclide_decay_simulator;

/// these assert that the half life for each nuclide is 
/// correct in the SingleNuclideSimulatorMC
#[cfg(test)]
pub mod tests;


/// Diffusion problems normally run on a continuum basis,
///
/// I chose Lagrangian-style diffusion here as it is easy to visualise
///
/// moreover, it is compatible with the monte carlo style of the simulator 
/// it is quite visual.
pub mod lagrangian_diffusion;
