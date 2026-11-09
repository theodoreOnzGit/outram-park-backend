//! The nusselt correlations class has calculates nusselt numbers
//! given a certain geometry

/// These are nusselt correlations for pipes
pub mod pipe_correlations;

/// contains data types used for nusselt number correlation
/// enums
pub mod input_structs;

/// contains nusselt number enums used for input calculation
pub mod enums;

/// free (natural) convection correlations (Churchill–Chu vertical plate)
/// with the Grashof and Rayleigh numbers
pub mod free_convection_correlations;

/// tests to ensure correlations are working correctly
pub mod tests;

/// MSRE TH-C1 (gh:#1135): the correlations against the analytical Graetz
/// solution and the textbook turbulent correlations (tests only).
pub mod graetz_th_c1;

pub use free_convection_correlations::{
    churchill_chu_vertical_plate_nusselt, grashof_number, rayleigh_number, ChurchillChuData,
};
pub use input_structs::{DittusBoelterData, FluidHeatingDirection};
