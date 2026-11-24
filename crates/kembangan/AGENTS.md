# AGENTS.md — kembangan

**KEMBANGAN — Kinetics, Elasticity and Materials Behaviour for Aging in
Nuclear Environments through Generative AI and Numerical Modelling.**
Materials damage under irradiation.

The workspace root `CLAUDE.md` binds here in full. This file adds only what is
specific to this crate.

## Status (2026-11-22, gh:#1699)

The only code is `src/rustbca/`, the RustBCA copy moved from `boon-lay`
unchanged. dpa, gas production and property changes are **not
implemented**; do not describe the crate as computing damage beyond what
RustBCA's cascades give.

## Dependency direction (maintainer, 2026-11-22)

- **KEMBANGAN serves `boon-lay` and other crates.** `boon-lay -> kembangan`;
  `boon_lay::rustbca` is a re-export of `kembangan::rustbca`.
- **`punggol`, `outram-mc-libs`, `njoy-outram-park-fork` and `petir` serve
  KEMBANGAN.** None of them may ever depend on this crate. They are wired in
  for the planned dpa work and not called yet.
- Charged-particle **transport** of fission fragments, alphas and recoils is
  RustBCA, so it lives here now; boon-lay keeps fuel, fission products and
  TRISO release. Photons and electrons stay in `punggol`, neutrons in
  `outram-mc-libs`.

## `no_std` + `alloc`, with a default `std` feature

Without `std` this crate is `no_std` + `alloc`, checked on a bare-metal
target by `scripts/check-nostd.sh`; `scripts/check-rustbca-no-std.sh` still
compiles the RustBCA tree alone as `#![no_std]`. New code uses `core::` /
`alloc::` paths, prints only behind `#[cfg(feature = "std")]`, and adds a
dependency only if it builds without `std` (or behind `std`, saying the cost).

## RustBCA is a copy: keep it one

Make changes to RustBCA itself in `src/rustbca/` and list each in its module
docs, as before the move. Its floats go through `petir::real::Real` and its
random numbers through `petir::rng` (draw for draw with upstream's
ChaCha8Rng, gh:#995). The "boon-lay local change" notes in those files record
where each change was made; they are history, not instructions.

## Avoid writing macros (workspace rule, 2026-11-05)

Maintainer direction: macros make code harder to read and review. Do not
write new `macro_rules!` or proc macros in this crate; use functions,
generics, enums and plain `match` arms. Full rule: root `CLAUDE.md`, "Avoid
writing macros".
