# CLAUDE.md — `dover` (DOVER)

**DOVER** — ***D**eck-based **O**pen-source **V**isualisation **E**ngine for
**R**eactors*. Backronym set by the maintainer on 2026-09-25 (recorded in
`docs/ecosystem-naming.md`).

Crate-specific guidance. The workspace `CLAUDE.md` at the repository root
governs everything not stated here; this file never relaxes it.

## What this crate is: an EMPTY SKELETON, scope to be decided

Created 2026-09-25 at the maintainer's direction ("empty skeleton"). The name
points towards visualisation driven by input decks, but **the scope is
deliberately undecided**. The crate has no dependencies, no public items and
no behaviour; `src/lib.rs` holds only the crate doc and one build-and-link
test.

## Hard rules for this crate, while it is a skeleton

- **Do not infer a scope from the name.** No deck format, visualisation
  engine, physics or API is to be invented here until the maintainer decides
  what DOVER is for. When that happens, record the decision and its date here
  and in the README, replacing the "scope to be decided" statements.
- **No placeholder modules, no stubbed API, no TODO physics.** An empty crate
  is honest; a stub that looks like a capability is not.
- **Search the workspace before building anything** (root `CLAUDE.md` hard
  rule). Whatever scope DOVER is given, related code may already exist
  elsewhere in the workspace (for example the egui simulators in
  `outram-park-digital-twin-engine`, or `dhoby-ghaut`, the intended GUI home).
- **Dependencies come from the root `[workspace.dependencies]`** and are added
  only when code here calls into them.
- **Portability.** The library must keep compiling for
  `aarch64-linux-android` (`--all-targets`) and for `wasm32-unknown-unknown`
  (`scripts/check-wasm.sh` picks it up automatically). A future windowing GUI
  dependency must be target-gated off Android in the same change.

## Maturity

**Not declared mature**, and has nothing to be mature about. No V&V exists
because nothing is implemented. Proposing maturity is allowed once there is
something to assess; declaring it is the maintainer's call alone.

## API mirror

There is deliberately no `docs/dover-api.md`: the crate has no public API to
mirror. Generate one with `kovan-cli api-docs dover` once public items exist.
