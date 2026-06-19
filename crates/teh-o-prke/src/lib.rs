
#[warn(missing_docs)]
/// contains structs for the zero power point reactor kinetics equations
///
/// the SixGroupPRKE struct contains the code which performs solution of 
/// the PRKE matrix with six precursor groups
///
/// but you must supply reactivity (or keff equivalently) as an input.
///
/// for real-time calculations, only thermal reactors are okay 
/// because the neutron generation time is on the order of 10E-4 s
/// for fast reactors, neutron generation time is on the order of 10E-8
/// but home computers calculate on the order of 1E-5s per calculation or 
/// 1E-6s at best
///
/// Will probably need some other kind of method to calculate feedback
/// 
pub mod zero_power_prke;


/// contains functions and structs for fuel temperature feedback 
///
/// this is the simplest feedback mechanism
/// where rudimentary thermal hydraulics model is added.
pub mod fuel_temperature_feedback;

/// contains functions and structs for control rod feedback 
pub mod control_rod_feedback;

/// error type for the crate 
pub mod teh_o_prke_error;



/// contains code for various feedback mechanisms
/// this uses the six factor formula rather than simply adjusting reactivity
pub mod feedback_mechanisms;


/// contains code for decay heat simulation
/// the user can have up to seven groups
///
pub mod decay_heat;

/// contains code for time stepping for prke 
/// some algorithms copied from OpenFOAM
///
pub mod time_stepping;
