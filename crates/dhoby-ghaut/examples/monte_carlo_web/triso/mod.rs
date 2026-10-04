//! The `triso` rung: a 2D analogue of an HTR-10 fuel pebble in a reflective
//! cell, one neutron at a time. (The hook of the Monte Carlo tutorial; rung 6
//! of the ladder, gh:#520.)

pub mod model;
#[cfg(not(target_arch = "wasm32"))]
pub mod render;
pub mod sim;
