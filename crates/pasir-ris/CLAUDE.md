# CLAUDE.md — pasir-ris

**PASIR RIS — Probabilistic Assessment of Safety In Reactors: Risk &
Reliability Integrated Studio.** The reserved home for an integrated risk and reliability GUI over `raffles`, `bishan`, `sembawang`,
`changi` and `buangkok`.

The workspace root `CLAUDE.md` binds here in full. This file adds only what is
specific to this crate.

## Status: placeholder (2026-10-06)

Nothing is implemented. Do not add code without the maintainer asking for it.

## Rules for when work starts

- **Search before building.** The physics and the probabilistic machinery
  already live in the crates it would drive (`raffles::scram` fault trees and
  the UQ core, `bishan::building`, `sembawang`'s chain). This crate is a GUI
  over them, not a second implementation.
- **Reuse the GUI building blocks** in `dhoby-ghaut` (`web_demo::view`,
  `web_demo::panel`) and follow the mobile-first and no-lag rules.
- **Dependencies are declared when code calls into them**, not before.
- **No "PSA" wording** beyond the backronym until the maintainer decides it
  (`docs/ecosystem-naming.md`).
- **If it comes to depend on `kovan`**, it becomes AGPL-3.0-only, as
  `dhoby-ghaut` did. Ask the maintainer first.
