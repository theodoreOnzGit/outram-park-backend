//! Ready-made cases: the boundary conditions, solver settings and
//! diagnostics of a named flow, shared by V&V tests and demos.
#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;
pub mod double_pipe;
pub mod helical_coil;
pub mod laminar_pipe;
