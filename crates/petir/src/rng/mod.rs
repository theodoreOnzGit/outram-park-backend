// SPDX-License-Identifier: GPL-3.0-only

//! **Random numbers** — OpenMC's 64-bit linear congruential generator with
//! its PCG-RXS-M-XS output permutation and O(log n) jump-ahead.
//!
//! # Lineage: ported (from OpenMC, not GSL)
//!
//! [`lcg`] ports OpenMC's `src/random_lcg.cpp` (MIT). It is the workspace's
//! one generator: `outram-mc-libs` (which re-exports it as
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
//! `cos` and `sin`, which `outram-mc-libs` routes to the **platform** libm by
//! default and to `petir::real` only under its `deterministic-math` feature.
//! A `no_std` copy here could only reach `libm`, so moving them would have
//! changed the default build's directions and energies in the last ulp — and
//! with them every seed-pinned k_eff. Nothing outside `outram-mc-libs`'
//! dependents needs them, so the move would have bought nothing.

pub mod lcg;
