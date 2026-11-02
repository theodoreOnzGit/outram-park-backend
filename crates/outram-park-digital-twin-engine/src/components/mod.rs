//! Visual process object wrappers.
//!
//! **MOVED 2026-11-02 (gh:#1112):** the widget library now lives in
//! `bugis::components` (feature `widgets`), the workspace's central GUI
//! crate; everything there is re-exported here so no call site changes.
//!
//! One module stays in this crate: [`excursion`], the fuel-temperature
//! excursion overlay. `ExcursionTrigger::htr10_fuel_temperature` reads the
//! cited Gao & Shi (2002) limit and the generic 1600 degC landmark from
//! [`crate::htr10::design`], which is physics data and this crate's sanctioned
//! exception; moving it into a GUI crate, or changing the constructor to take
//! the limits as arguments, would each do more than a move. The kovan review
//! records for the old paths stay in this folder.

pub use bugis::components::*;

pub mod excursion;
pub use excursion::{ExcursionOverlay, ExcursionStage, ExcursionTrigger};
