//! The `godiva` rung: ICSBEP HEU-MET-FAST-001, a bare sphere of highly
//! enriched uranium (rung 1 of the Monte Carlo ladder, gh:#521).

pub mod model;
#[cfg(not(target_arch = "wasm32"))]
pub mod render;
pub mod sim;
