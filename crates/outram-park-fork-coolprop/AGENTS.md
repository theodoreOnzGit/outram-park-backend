# AGENTS.md — outram-park-fork-coolprop

Guidance for AI assistants (Claude Code and others) working in this crate.
The workspace rules in the root [`CLAUDE.md`](../../CLAUDE.md) and
[`AGENTS.md`](../../AGENTS.md) bind here in full. This crate has no
crate-specific rules yet; add them to this file.

## `no_std` + `alloc`, with a default `std` feature (gh:#1598, 2026-11-16)

Without its default `std` feature this crate is `no_std` + `alloc`, checked
on a bare-metal target by `scripts/check-nostd.sh` (CI job `no-std` in
`fast-tests.yml`). Keep it that way:

- **New code uses `core::` / `alloc::` paths** where they exist; every module
  glob-imports `crate::nostd_prelude::*` in `no_std` builds (copy the line
  from any module). Doc-comment examples keep `std::` paths (a doctest has no
  `extern crate alloc`).
- **What needs an operating system goes behind `#[cfg(feature = "std")]`**:
  printing (`dbg!`, `println!`, `eprintln!`: put the attribute on the
  statement), files, threads, clocks.
- **No `HashMap` in the library**: `BTreeMap` (`alloc`; fixed iteration
  order).
- **A `std` build computes what it did before**: `petir::real::Real`, which
  supplies `sqrt`, `floor`, ... without `std`, is imported only there.
- A library dependency must build without `std`; one only tests use belongs
  in `[dev-dependencies]`.

## Avoid writing macros (workspace rule, 2026-11-05)

Maintainer direction: macros make code harder to read and review. Do not
write new `macro_rules!` or proc macros in this crate; use functions,
generics, enums and plain `match` arms. Using std and dependency macros
(`vec!`, `format!`, derives) is fine. Full rule: root `CLAUDE.md`, "Avoid
writing macros".
