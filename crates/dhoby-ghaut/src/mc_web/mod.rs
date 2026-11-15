//! # The browser Monte Carlo pipeline (gh:#1572)
//!
//! What every in-browser Monte Carlo run shares, moved here from the
//! `monte_carlo_web` example (where no other demo could reuse it) so that
//! its HTR-10 whole core and the museum MSRE simulator
//! (~~`outram-park-digital-twin-engine`, `examples/msre_web`~~ since
//! gh:#1578 that crate's library, `msre::web`) call the same code:
//!
//! | Module | What |
//! |---|---|
//! | [`tapes`] | ENDF tapes for the browser: covariances stripped, zlib, wire names, the LEAPR "tapes" |
//! | [`processed_cache`] | the processed nuclear data every demo shares, read back from the browser's cache when it is there (gh:#818) |
//! | [`keff`] | a run's settings, caps, console and summary |
//! | [`pool`] | the page side of a worker pool: process and relay the data ([`pool::DataRelay`]), then run k_eff on the workers ([`pool::KeffPool`]); the worker side of a CSG model |
//!
//! Moved, not rewritten: each file is the example's, with its paths
//! changed. The example keeps thin re-exports under the old module names,
//! so its rungs did not change. Research, education and V&V only.

pub mod keff;
pub mod pool;
pub mod processed_cache;
pub mod tapes;
