# Reference ENDF tapes for V&V (repo-tracked, NOT crate-packaged)

This folder holds raw ENDF-6 evaluated nuclear-data tapes used **only** as
verification & validation (V&V) reference inputs by the workspace test suites. It
lives at the repo root, **outside `crates/`**, so it is git-tracked but is
**never** part of any crate's published tarball (Cargo packages only files under
the crate directory). Large reference data therefore stays reproducible for
contributors without bloating a published crate.

## Data policy / provenance (mandatory)

Only **open, published** nuclear data may go here (see `DATA_POLICY.md` and
`RESEARCH_INTEGRITY_AND_PROVENANCE.md`). ENDF/B-VIII.0 is public, open-source
evaluated data (NNDC/IAEA); that is allowed. For every tape added, record its
provenance in the table below: nuclide, library + version, MAT, source URL, date
accessed.

| File | Nuclide | Library | MAT | Source | Date accessed |
|---|---|---|---|---|---|
| `n-009_F_019-ENDF8.0.endf` | F-19 | ENDF/B-VIII.0 (neutron) | 925 | _(fill in: IAEA NDS / NNDC)_ | _(fill in)_ |
| `n-008_O_016-ENDF8.0.endf` | O-16 | ENDF/B-VIII.0 (neutron) | 825 | _(fill in)_ | _(fill in)_ |

## What is needed and why

These are needed because they are **not** available in this build environment
(the egress policy denies `www-nds.iaea.org`) and are not shipped in any crate's
`tests/resources/`:

- **`n-009_F_019-ENDF8.0.endf`** — F-19, **the priority**. ENDF/B-VIII.0 uses the
  R-matrix-limited (LRF=7 / KRM=3) resonance format for F-19, which is exactly
  the SAMM reconstruction path under verification (bead **op-cjw.2**). F-19 is
  preferred because it has spin groups where the eliminated capture channel is
  not first — it exercises the `reorder_eliminated_channel` fix.
- **`n-008_O_016-ENDF8.0.endf`** — O-16, also LRF=7 in VIII.0. A simpler second
  SAMM case, and doubles as a clean elastic-scatter-matrix golden for GROUPR
  (bead **op-3ut**): light nuclide, no unresolved range.

Provide the **raw ENDF-6 ASCII tape** (the unzipped single-material `.endf`
text file), named exactly as in the table so the tests find it.

## How the tests use it

Crate V&V tests read tapes from here via a path relative to the crate, e.g.
`env!("CARGO_MANIFEST_DIR")/../../reference-data/endf/<file>`, and **skip
gracefully** (with a printed note) when a tape is absent — so the crate still
builds and tests without this folder populated. The matching NJOY golden output
is generated on demand from the locally-built `vendor/njoy2016` oracle (which is
gitignored), never committed as a large tape; only extracted reference values
(CSV) are committed, per the existing `u238_doppler` pattern.
