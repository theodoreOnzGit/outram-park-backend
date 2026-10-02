//! Random numbers: OpenMC's 64-bit LCG and the samplers built on it.
//!
//! - [`lcg`] — **re-exported from [`petir::rng::lcg`]** since 2026-10-02. The
//!   generator was ported here first and moved to PETIR at the maintainer's
//!   direction, so that `raffles` could use it without depending on this
//!   crate (that edge closed a dependency cycle once this crate took on
//!   `outram-park-fork-liggghts` for DEM pebble beds). The path
//!   `outram_mc_libs::rng::lcg` is kept on purpose so no call site had to
//!   change, and the move changed no random number
//!   (`petir::rng::lcg`'s `moved_stream_is_pinned` test).
//! - [`distributions`] — OpenMC's `random_dist.cpp` samplers. These stay
//!   here: they call `cos`/`sin` through this crate's [`crate::mathf`]
//!   route (platform libm by default, `petir::real` under
//!   `deterministic-math`), which a `no_std` copy in PETIR could not
//!   reproduce bit for bit.

pub use petir::rng::lcg;
pub mod distributions;
