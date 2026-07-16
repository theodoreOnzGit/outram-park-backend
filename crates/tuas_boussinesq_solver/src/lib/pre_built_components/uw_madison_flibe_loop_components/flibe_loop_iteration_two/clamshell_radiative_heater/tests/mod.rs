//! Unit tests for the clamshell radiative heater.
//!
//! Intended to verify the heater at steady state via energy balance: the
//! 1.7 kW (W) input to the heating element must equal the sum of its radiative
//! and convective losses, and the net radiant heat received by the tube fluid
//! and inner tube must equal their convective and radiative losses. These
//! tests are not yet written (see the checklist below).

// TODO: for unit test
//
// There are some first orders of business,
//
// (1) finish the ClamshellRadiativeHeater struct first 
// (2) create a radiative heating element material because steel temperature 
// may get too hot and therefore out of thermophysical property range
//
// (3) start constructing steady state simulations and tweaking them
//
//
// misc cleanup work includes the STHE constructor unit test
