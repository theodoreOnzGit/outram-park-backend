# CLAUDE.md — `dover` (DOVER)

**DOVER** — ***D**eck-based **O**pen-source **V**isualisation **E**ngine for
**R**eactors*. Backronym set by the maintainer on 2026-09-25 (recorded in
`docs/ecosystem-naming.md`).

Crate-specific guidance. The workspace `CLAUDE.md` at the repository root
governs everything not stated here; this file never relaxes it.

## What this crate is for: the low-fidelity counterpart of DHOBY GHAUT

**Maintainer decision, 2026-09-25: "DOVER is meant to be the low fidelity
equivalent of dhoby ghaut."** `dhoby-ghaut` (**D**igital **H**igh-fidelity
**O**rchestration **by** **G**UI …) hosts the studios that drive the
high-fidelity solvers (`outram-mc`, cfMesh). DOVER is its low-fidelity twin:
the same role, visualising reactors from input decks, over low-fidelity models.

**Still undecided:** which low-fidelity models it drives, the deck schema
(TOML is the current direction, below), and whether it carries a windowing GUI as `dhoby-ghaut` does. Low-fidelity models
already exist in the workspace (e.g. the hierarchical surrogates in
`outram-park-digital-twin-engine`'s simulators), so search before building.
The root `CLAUDE.md` model-hierarchy rule applies to whatever DOVER drives:
hierarchical, physics-derived surrogates, run uncalibrated first.

## Direction: TOML input decks, steady-state and dynamic runs (2026-09-25)

**Maintainer direction, 2026-09-25, stated as tentative ("perhaps"):**

- **Input decks are perhaps TOML files**, serialised and deserialised by a
  reader against a **schema**, so a deck is a validated, typed document rather
  than free text.
- DOVER runs **steady-state** simulations, **like DWSIM** (a flowsheet solved
  to steady state), and **dynamic** simulations as well.

**Already in the workspace; reuse before writing anything (checked
2026-09-25):**

- `outram-park-fork-dwsim-libs`: the Rust translation of DWSIM's kernels,
  including `flowsheet`, `flowsheet_solver` and `dynamics` modules plus the
  unit operations (heater, cooler, pump, valve, heat exchanger, separator, …).
  The DWSIM-like steady-state path should compose these, not duplicate them.
- `chem-eng-real-time-process-control-simulator`: exact zero-order-hold
  transfer-function blocks for dynamic process-control runs.
- `serde` and `toml` are already in the root `[workspace.dependencies]`
  (`kovan` uses them for its own files). No crate currently reads a
  *simulation* input deck against a schema, so the deck reader is the new part.

Not decided: the schema's form and versioning, which models a deck may name,
and whether DOVER has a windowing GUI.

## What this crate is now: an EMPTY SKELETON

Created 2026-09-25 at the maintainer's direction ("empty skeleton").
~~The scope is deliberately undecided.~~ **CORRECTED 2026-09-25**: the role is
set (above), and the details are still open. The crate has no dependencies, no
public items and no behaviour; `src/lib.rs` holds only the crate doc and one
build-and-link test.

## Hard rules for this crate, while it is a skeleton

- **Do not infer the details from the name or the role.** No deck format,
  visualisation engine, physics or API is to be invented here until the
  maintainer decides them. Record each decision and its date here and in the
  README.
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
