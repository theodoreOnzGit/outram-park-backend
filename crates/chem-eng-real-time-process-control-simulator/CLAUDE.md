# CLAUDE.md

Guidance for Claude Code (and other AI assistants) working in this crate.

> This crate is a member of the **OUTRAM PARK** workspace
> (`crates/chem-eng-real-time-process-control-simulator`). See the workspace root
> `CLAUDE.md` for the shared dependency policy and full migration history.
> Dependencies are inherited from the root `[workspace.dependencies]` — do not
> pin versions in this crate's `Cargo.toml`.

## What this is

**chem-eng-real-time-process-control-simulator** — a real-time process-control
library for chemical (and general) engineering: transfer functions and
controllers (PID and friends) intended to run inside time-stepping simulators.
Within the suite it supplies the **PID controllers** used by the TUAS natural-
circulation loops and the FHR educational simulators.

**License: Apache-2.0** — unlike the GPL-3.0 default of the rest of the
workspace. The `Cargo.toml` therefore sets `license` explicitly instead of
inheriting `license.workspace = true`. Keep it Apache-2.0.

## Layout (`src/lib/`)

API stability tiers (import from the tier you want):

- `stable/` — stable API.
- `beta_testing/` — recommended for new code; mostly stable.
- `alpha_nightly/` — unstable; `controllers/`, `stable_transfer_functions/`,
  `transfer_fn_wrapper_and_enums/`, `errors/`.

Targets: `[lib]` is `chem_eng_real_time_process_control_simulator`
(`src/lib/lib.rs`); there is also a `library_demo` `[[bin]]` (`src/main.rs`).

## Build, test, run

```bash
cargo test -p chem-eng-real-time-process-control-simulator
cargo run  -p chem-eng-real-time-process-control-simulator --bin library_demo
```

## Migration notes (OUTRAM PARK consolidation, 2026-06)

Done while moving this crate into the workspace and bumping to latest deps:

- Now built/tested/published from the workspace; standalone git history dropped.
- Dependencies (`approx`, `csv`, `thiserror`, `uom`) switched to
  `*.workspace = true`. Notably `uom` 0.36 → **0.38** and `thiserror` 1 → 2.
  Unifying `uom` to a single version across the workspace is what made the TUAS /
  TAMPINES controller-based tests compile again (they were hitting two
  incompatible `uom::Quantity` types).
- The crate's `[profile.release]` (opt-level 2) and `[profile.dev.package."*"]`
  (opt-level 2) sections were **removed** — Cargo only honors `[profile.*]` on
  the workspace root. The `dev.package."*"` optimization was re-added at the
  root; the `release` opt-level override was intentionally dropped so the
  numerical solvers build at the default `-O3`.
- License kept as **Apache-2.0** (explicit, not inherited).

The library and its tests compile cleanly on the bumped dependencies; no source
changes were required here.
