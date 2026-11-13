# AGENTS.md — kovan-discovery

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
