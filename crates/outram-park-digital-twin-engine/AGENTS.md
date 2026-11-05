# Agent instructions: outram-park-digital-twin-engine

This crate's rules are in [`CLAUDE.md`](CLAUDE.md), and the workspace rules
in the root [`CLAUDE.md`](../../CLAUDE.md) and [`AGENTS.md`](../../AGENTS.md);
they bind every agent, not only Claude. The rule below is repeated here
because it was broken in this crate.

## Transcendentals go through PETIR, never the platform libm (HARD RULE, 2026-11-05)

Maintainer direction (gh:#1290): in this crate, call `exp`, `ln`, `log10`,
`powi`, `powf`, `sin`, `cos`, `atan2` and the rest through
`petir::mathf::RealMath` (`x.r_exp()`, `x.r_ln()`, `x.r_powi(n)`, ...),
never the inherent `f64` methods (`x.exp()`). The inherent methods take
their last bit from the platform's C library (glibc, MSVC, Apple, wasm), so
the same run gives different bits on different machines. PETIR's are one
implementation everywhere. A raw call still compiles and runs, so the
workspace gate `crates/petir/tests/no_platform_libm.rs` is what catches it;
it was red on 2026-11-05 with 40 raw calls in this crate and its sibling
(#1290). Keep it green.
