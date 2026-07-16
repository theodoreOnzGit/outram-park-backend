//! GFHR (generic Fluoride-salt-cooled High-temperature Reactor) pipe-network
//! test suite.
//!
//! This module builds and exercises a simplified two-loop thermal-hydraulic
//! model of a gFHR-style reactor and verifies the pipe-network fluid-mechanics
//! solvers against expected mass flowrates (kg/s) and pressure drops (Pa).
//!
//! Module map:
//! - [`components`] — builders for the individual pipes, pumps, mixing nodes,
//!   and the intermediate heat exchanger (IHX) that make up the primary
//!   (FLiBe) and intermediate (HITEC) loops.
//! - [`single_pipe_tests`] — single-component checks: pressure change from a
//!   fixed mass flowrate and its inverse, in forward and reverse flow.
//! - [`single_branch`] — checks across an individual branch (a series
//!   collection of components), e.g. the reactor branch and IHX branch.
//! - [`multi_branch`] — the multi-branch parallel network solvers and the
//!   full four-branch primary + intermediate loop tests (isothermal and
//!   coupled thermal-hydraulic).
//!
//! Note: Radiation Heat Transfer (RHT) is NOT accounted for anywhere in this
//! module.

/// contains the code for single pipes and other components in
/// primary and intermediate loop of what could be used to cool the gFHR
///
/// generic (Fluoride Salt Cooled High Temperature Reactor)
///
/// Note: Radiation Heat Transfer (RHT) NOT accounted for
pub mod components;
/// first, single pipe tests 
///
/// this is where single pipes of the typical fhrs are concerned 
/// both pressure change from mass flowrate and vice verse should be tested
/// in fwd and backwd flow
pub mod single_pipe_tests;


/// second, need to have 
/// tests across individual branches
pub mod single_branch;

/// third, need to have tests across multiple branches 
///
/// for building the components,
/// we should design it around the intermediate heat exchanger 
/// heat exchanger is salt to salt, STHE 
/// maybe 1200 kg/s on tube side of FLiBe 
/// and enough flowrate of HITEC on shell side to ensure that 
/// temperature settings are right
///
/// the flowrate through both sides is about 700-800 kg/s 
/// this is about the right order of magnitude for gFHR. 
/// I think the pressure drops are on the right order of magnitude 
/// roughly
pub mod multi_branch;


