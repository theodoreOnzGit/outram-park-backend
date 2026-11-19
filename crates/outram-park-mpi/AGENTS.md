# AGENTS.md — outram-park-mpi

Guidance for AI assistants (Claude Code and others) working in this crate.
The workspace rules in the root [`CLAUDE.md`](../../CLAUDE.md) and
[`AGENTS.md`](../../AGENTS.md) bind here in full. This crate has no
crate-specific rules yet; add them to this file.

## Avoid writing macros (workspace rule, 2026-11-05)

Maintainer direction: macros make code harder to read and review. Do not
write new `macro_rules!` or proc macros in this crate; use functions,
generics, enums and plain `match` arms. Using std and dependency macros
(`vec!`, `format!`, derives) is fine. Full rule: root `CLAUDE.md`, "Avoid
writing macros".

## `std` only: never converted to `no_std` (maintainer, 2026-11-17)

**Maintainer direction, 2026-11-17: "mpi shld be std only".** Its ranks are OS
threads, so this crate stays `std` and is out of scope for the `no_std` epic
(gh:#1585). A `no_std`-capable crate that uses it puts the dependency, and the
code that calls it, behind its own `std` feature, as
`outram-park-fork-pflotran` does with `decomposition` (gh:#1603).
