//! Hardcoded per-fluid Helmholtz EOS data (`const FluidEos`), generated from
//! CoolProp's fluid JSON by `dev/gen_fluid.py`. One file per fluid.
//!
//! These are the only "data" the shipped crate carries — a few KB each, no
//! runtime JSON. Add a fluid by running the generator and declaring it here
//! (and adding a [`super::fluid::Fluid`] variant).

pub mod ammonia;
pub mod fluorine;
pub mod helium;
pub mod methanol;
pub mod n_heptane;
pub mod nitrogen;
pub mod r125;
pub mod r22;
pub mod water;
