# CLAUDE.md — njoy-outram-park-fork

Pure-Rust port (in progress) of **NJOY2016** nuclear-data processing. Produces
the ACE continuous-energy libraries that `outram-mc-libs` consumes — the data-prep
step upstream of an OpenMC run.

> Workspace member of the **OUTRAM PARK** backend. See the root `CLAUDE.md` for
> the shared dependency policy and design rules. Dep versions come from
> `[workspace.dependencies]` — do not pin locally.

## License compliance (MANDATORY — do not break)

This crate is a **derivative work** of NJOY2016, which is under a *modified BSD
3-Clause* license (LANL/DOE variant). That license is GPL-compatible, so the
crate as a whole is `GPL-3.0-only`. To stay compliant, you MUST:

- **Keep `LICENSE.njoy` and `NOTICE`** at the crate root, verbatim. Never delete
  or alter the upstream copyright notice/disclaimer.
- **Mark this as a modified, non-LANL version.** Do not remove the "not the LANL
  version / not endorsed" language from `NOTICE`, `README.md`, or the crate-level
  `//!` doc in `src/lib.rs`.
- **No endorsement (BSD-3 cond. 3).** Never use "Los Alamos", "LANL", "U.S.
  Government", or NJOY contributor names to endorse or promote this crate.
- If you publish to crates.io, flip `publish = false` off only after adding
  `include` so `LICENSE.njoy` + `NOTICE` ship in the tarball.

## Design rules (see also root CLAUDE.md)

- **Enum dispatch, not trait objects.** Module selection uses the `NjoyModule`
  enum (`src/modules/mod.rs`), not `Box<dyn _>`. The module set is closed.
- **No `Box<T>`, no lifetime parameters, no `dyn`.** Own data by value or share
  read-only tables with `Arc<T>`. Fortran `common` blocks become owned structs,
  not globals.
- **`uom` at physics boundaries.** Energies, temperatures, and cross sections in
  public signatures carry dimensioned types; spell out units in doc comments.
- **Errors via `Result<_, NjoyError>`**, never a process-aborting `error()` call
  the way upstream Fortran does.
- **File size cap: 1000 lines, 1500 only if truly necessary (mandatory,
  2026-07-07 onward).** Split a ported module by function/responsibility into
  a `module_name/` directory (`mod.rs` = module doc + `pub use` re-exports;
  siblings named for their functional group) rather than growing one flat
  file. See `src/samm/coulomb/` for the pattern. Applies to every module
  ported from this date forward. Existing over-length files are tracked in
  `docs/porting-plan.md` §5 — split opportunistically, don't grow them
  further without splitting first.

## Build and test

**Rule: always use `--release` for builds and tests.** Never run in debug mode.

```bash
cargo check -p njoy-outram-park-fork --lib
cargo test  -p njoy-outram-park-fork --lib --release
```

### HARD RULE — cap unit-test memory at ~12 GB

Unit tests for this crate **must** run under a hard ~12 GB address-space cap.
The ACER / thermal S(α,β) import paths build large per-incident-energy emission
tables; a malformed ENDF record (a cursor that fails to advance, an unbounded
energy grid) becomes runaway allocation that can freeze the whole machine
instead of failing a test.

Run tests through the wrapper, which sets `ulimit -v` before invoking cargo:

```bash
crates/njoy-outram-park-fork/scripts/test.sh              # full suite, capped
crates/njoy-outram-park-fork/scripts/test.sh thermal      # subset by substring
```

Do **not** invoke a bare `cargo test -p njoy-outram-park-fork` for interactive
runs — that has no cap. If a test legitimately needs more than 12 GB, that is a
design smell (stream/chunk the data); raise it with a human before lifting the
cap in `scripts/test.sh`.

## Porting plan & C-source map (read on demand)

The full module list, the Fortran-source → Rust-module map with line counts, the
phased porting order (OpenMC ACE path first), the Fortran→Rust translation
conventions, and the golden-file verification strategy against upstream NJOY all
live in **`docs/porting-plan.md`**. The reference Fortran source is at
`upstream_source/NJOY2016`.

## Model division of labour (MANDATORY for this port)

The NJOY Fortran→Rust port runs a two-model workflow to control cost:

- **Sonnet ports, module by module, WITHOUT tests.** Sonnet does the faithful,
  line-for-line translation of a module's Fortran into Rust only. It does **not**
  write or run verification tests, and it does **not** "improve" the algorithm
  during translation (see `docs/porting-plan.md` §5). Where a piece is not yet
  done, leave an explicit `NjoyError::NotPorted` / `TODO` marker — **never** paper
  over a gap with a plausible-looking value.
- **Opus debugs, verifies, and tests.** A separate Opus pass validates each
  translated module against the NJOY golden oracle (`upstream_source/NJOY2016`), writes
  the V&V tests (methodology **and** results, per the root `CLAUDE.md` V&V rule),
  and localises/fixes discrepancies. Opus does not redo the translation.

Keep every port **line-traceable to the Fortran** so the Opus verification pass
can localise a discrepancy to a specific subroutine. Per-module theory,
implementation notes, testing status, and caveats live in each module's
`README.md` (co-located with its Rust source under `src/`).
