// SPDX-License-Identifier: GPL-3.0-only

//! **Random numbers** — OpenMC's 64-bit linear congruential generator with
//! its PCG-RXS-M-XS output permutation and O(log n) jump-ahead.
//!
//! # Lineage: ported (from OpenMC, not GSL)
//!
//! [`lcg`] ports OpenMC's `src/random_lcg.cpp` (MIT). ~~It is the workspace's
//! one generator~~ **CORRECTED 2026-10-20 (gh:#995)** — since then PETIR also
//! holds [`ChaCha8Rng`] (ported from `rand` 0.10.3, below), which only
//! `boon-lay`'s RustBCA copy uses; the LCG is the generator everything else
//! draws from: `outram-mc-libs` (which re-exports it as
//! `outram_mc_libs::rng::lcg`), `boon-lay`, `nee_soon` and `raffles` all draw
//! from it, and its GPU twin is [`crate::wgsl::LCG`].
//!
//! It lived in `outram-mc-libs` until 2026-10-02 and moved here at the
//! maintainer's direction, because `raffles` needed it and was the only
//! thing tying `raffles` to the transport crate. That edge closed a
//! dependency cycle (`outram-mc-libs` -> `outram-park-fork-liggghts` ->
//! `raffles` -> `outram-mc-libs`) the moment `outram-mc-libs` wanted DEM
//! pebble beds. The move changed no random number: see
//! `lcg::tests::moved_stream_is_pinned`.
//!
//! # What did NOT move, and why
//!
//! OpenMC's samplers (`random_dist.cpp`: Maxwell, Watt, isotropic direction,
//! Box-Muller normal) stay in `outram_mc_libs::rng::distributions`. They call
//! `cos` and `sin`, which ~~`outram-mc-libs` routes to the **platform** libm by
//! default and to `petir::real` only under its `deterministic-math` feature.
//! A `no_std` copy here could only reach `libm`, so moving them would have
//! changed the default build's directions and energies in the last ulp — and
//! with them every seed-pinned k_eff.~~ **CORRECTED 2026-10-20 (gh:#985):**
//! `outram-mc-libs` now routes to PETIR's `RealMath` always, so that reason
//! is gone. Nothing outside `outram-mc-libs`'
//! dependents needs them, so the move would have bought nothing.

pub mod lcg;

// ---------------------------------------------------------------------------
// The `rand` pieces RustBCA uses (gh:#995, maintainer direction 2026-10-20).
// ---------------------------------------------------------------------------
//
// # A second lineage: ported from the Rand project and RustCrypto
//
// `boon-lay`'s copy of RustBCA drew from `rand` 0.10.3 (`ChaCha8Rng`,
// `random::<f64>()`, `Uniform<f64>`) and `rand_distr` 0.6.0 (`Normal<f64>`).
// Those algorithms are ported here so it draws from PETIR instead, from the
// exact registry sources of the locked versions (MIT OR Apache-2.0; each
// notice is in its file header and in NOTICE):
//
// | module | upstream |
// |---|---|
// | `traits` | `rand_core` 0.10.1 `seedable_rng.rs` (PCG32 `seed_from_u64`, `from_rng`); `rand` 0.10.3 `rng.rs`, `distr/float.rs`, `distr/integer.rs` |
// | `chacha` | `chacha20` 0.10.2 (`rand::rngs::ChaCha8Rng`) + `rand_core` 0.10.1 `block.rs` |
// | `uniform` | `rand` 0.10.3 `distr/uniform*.rs`, `distr/float.rs` (`Open01`) |
// | `normal`, `ziggurat_tables` | `rand_distr` 0.6.0 `normal.rs`, `utils.rs`, `ziggurat_tables.rs` |
//
// Not a single random number may differ from the crates they came from:
// `tests/rng_vs_rand.rs` compares long streams, draw for draw, against a
// reference those crates produced (`reference-data/rand/`). There is no
// entropy source here (PETIR is `no_std`); seeding from the OS stays with
// the caller.

mod chacha;
mod normal;
mod traits;
mod uniform;
mod ziggurat_tables;

pub use chacha::ChaCha8Rng;
pub use normal::{standard_normal, Normal, NormalError};
pub use traits::{Rng, RngExt, SeedableRng, StandardSample};
pub use uniform::{open01, Uniform, UniformError};
