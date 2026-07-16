//! Radiation view factors for enclosure heat-transfer geometries.
//!
//! A view factor `F_(i-j)` is the dimensionless fraction (0 to 1) of
//! radiation leaving surface `i` that is intercepted by surface `j`. The
//! functions here compute the analytical view factors for a pair of coaxial
//! (concentric) cylinders and the annular end rings between them, for use in
//! the clamshell radiative heater model. Currently only the concentric
//! cylinder geometry is implemented; new enclosure geometries belong in their
//! own submodule here.

/// this module contains functions for calculating view factors for
/// cocentric cylinders. This is for the clamshell radiative heater 
/// still under construction
///
/// However, the view factors themselves have been tested to check 
/// if they add up to one
pub mod cocentric_cylinders;
