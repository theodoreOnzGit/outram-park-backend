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

**Rule: always use `--release` for builds and tests.** Never run in debug mode.

```bash
cargo test -p chem-eng-real-time-process-control-simulator --release
cargo run  -p chem-eng-real-time-process-control-simulator --bin library_demo --release
```

## Migration notes (read on demand)

The 2026-06 consolidation log for this crate lives in **`docs/notes.md`**.
