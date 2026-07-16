//! Adiabatic mixing-joint tests for the [`HeatTransferEntity`] enum layer.
//!
//! These check that `FluidArray`s can form an adiabatic mixing joint with a
//! single control volume: two 0.05 kg/s therminol VP-1 streams (one at 50 deg C,
//! one at 100 deg C) merge in a single-CV joint and 0.10 kg/s leaves at the
//! adiabatically mixed temperature (~75 deg C). Split by flow sign into
//! [`fwd_flow`] (positive/forward) and [`reverse_flow`] (negative/reverse).

/// this test checks if FluidArrays can form adiabatic mixing joints
/// with single cvs
///
/// so let's say, two pipes with 0.05 kg/s of therminol vp1
/// flowing into a mixing joint (singleCV)
///
/// one is 50C, one is 100C
///
/// and 0.10 kg/s flows out. it should be 75 C is adiabatically mixed
///
/// flows are positive (forward)
pub mod fwd_flow;


/// this test checks if FluidArrays can form adiabatic mixing joints 
/// with single cvs 
///
/// so let's say, two pipes with 0.05 kg/s of therminol vp1 
/// flowing into a mixing joint (singleCV)
///
/// one is 50C, one is 100C
///
/// and 0.10 kg/s flows out. it should be 75 C is adiabatically mixed
///
/// flows are negative value (reverse)
pub mod reverse_flow;
