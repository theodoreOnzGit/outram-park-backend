//! ENDF tape bytes for the browser: moved to the library as
//! `crate::mc_web::tapes` (gh:#1572); re-exported here so every rung's
//! `crate::mc_demo::tapes::…` path stays.
pub use crate::mc_web::tapes::*;
