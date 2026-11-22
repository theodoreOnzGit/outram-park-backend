//! The processed nuclear data every Monte Carlo demo shares: moved to the
//! library as `dhoby_ghaut::mc_web::processed_cache` (gh:#1572);
//! re-exported here so each demo's `crate::processed_cache::…` path stays.
pub use dhoby_ghaut::mc_web::processed_cache::*;
