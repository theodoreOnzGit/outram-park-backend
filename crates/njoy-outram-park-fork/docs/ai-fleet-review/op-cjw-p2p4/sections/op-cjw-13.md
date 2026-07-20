# op-cjw.13 — Split `errorr/groups.rs` (pure refactor, no behavior change)

**Status:** DONE
**Type:** P2 pure refactor — file-size-cap split, zero logic/behavior change.
**Scope:** `crates/njoy-outram-park-fork/src/errorr/` only.

## What was done

The single flat file `src/errorr/groups.rs` (1898 lines — over the crate's
1000/1500-line file-size cap) was converted into a directory module. The
~1117 lines of literal `EG*`/`GL*` (plus `DELTL`/`NDELTA`/`U80`/`IG14`)
constant boundary tables were moved into a sibling `tables.rs`; the dispatch
function, the `sigfig` helper, the `NeutronGroupStructure` enum, and the unit
tests stay in `mod.rs`.

## Files changed

| File | Action | Final line count |
|---|---|---|
| `src/errorr/groups.rs` | **deleted** (converted to directory module) | — (was 1898) |
| `src/errorr/groups/mod.rs` | **new** — dispatch fn + `sigfig` + `NeutronGroupStructure` enum/impl + `#[cfg(test)] mod tests` + `mod tables; use tables::*;` | **773** |
| `src/errorr/groups/tables.rs` | **new** — the 39 `EG*`/`GL*`/`DELTL`/`NDELTA`/`U80`/`IG14` `const` arrays only, each `pub(super)` | **1138** |

Both files are under the 1000/1500 cap (mod.rs < 1000; tables.rs < 1500), so
no further sub-split of `tables.rs` was needed.

## Split rationale

- `groups.rs` was flagged in its own module doc's "File-size note" as
  data-heavy: ~9800 `f64` values across 39 arrays. That note already named the
  natural split — move the `EG*`/`GL*` `const` tables to `groups/tables.rs` and
  keep the dispatch logic. This change simply executes that named split.
- The constant tables carry no logic, so they move cleanly into a leaf module.
  Made them `pub(super)` so the dispatch fn in `mod.rs` (their parent module)
  still resolves the bare names via `use tables::*;`.
- The `#[cfg(test)] mod tests` was checked and references **no** `EG*`/`GL*`
  constant directly (it reads all values through `neutron_group_structure` /
  `NeutronGroupStructure`), so it stayed in `mod.rs` unchanged with its existing
  `use super::*;`.

## Verbatim-move verification

- **Const item count:** 39 `const` items in the original file → 39
  `pub(super) const` items in `tables.rs`, 0 left in `mod.rs`. No const lost or
  duplicated.
- **Byte-level diff:** extracted the original const region (`git show
  HEAD:.../groups.rs` lines 588–1704) and diffed it against `tables.rs`'s const
  region with the `pub(super) ` visibility prefix stripped back to `const `.
  Result: **identical** (`diff` reported no differences). Every digit, `e0`/`E+00`
  suffix, sign, and element order is preserved exactly.
- The only textual change to any const line is the added `pub(super) ` visibility
  keyword; the array types, names, lengths, and values are untouched.

## Other (non-const) edits

- The module-doc "File-size note" in `mod.rs` was updated from "if the
  maintainer prefers to honour the cap, the natural split is…" (future tense) to
  describe the completed split and point at the new `tables` module. Doc-only;
  no code/logic change.
- Both new files carry the GPLv3 provenance header block (upstream NJOY2016
  commit, source file, licence, "modified non-LANL / not endorsed" language).
  `tables.rs` gained its own `//!` header stating it holds only the literal
  ENDF/NJOY group-boundary constant tables.

## Verification result

- `cargo build -p njoy-outram-park-fork --release` — clean, **0 warnings, 0 errors**.
- Lib tests run under the crate's mandatory `ulimit -v` 12 GB cap:
  `cargo test -p njoy-outram-park-fork --lib --release`

  ```
  test result: ok. 274 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.40s
  ```

  Baseline preserved exactly: **274 passed, 0 failed, 0 warnings.**

## Human re-verify

None specifically required — the byte-level `diff` of the const region proves
the tables were moved verbatim, so no hand-transcribed digit changed. (For due
diligence a reviewer may re-run the same `git show … | diff` check; it should
report no differences.)
