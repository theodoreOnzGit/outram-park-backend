//! Steam-turbine equations: converging-diverging nozzle / choked-flow
//! relations ([`converging_diverging_nozzles`]) and three-phase electric
//! generator equations ([`generator`]).

/// equations for isentropic nozzles, including choked flow
/// at sonic speeds
pub mod converging_diverging_nozzles;
pub use converging_diverging_nozzles::*;

/// these contain equations for generator
/// where flux and stuff are used
#[allow(non_snake_case)]
pub mod generator;
pub use generator::*;
