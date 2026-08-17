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

### Present in the repository

Moved here from `crates/njoy-outram-park-fork/tests/resources/` on 2026-08-17 so
that no tape sits inside a crate directory (see "Why not inside a crate" below).
Sizes are as committed; ~89 MB total.

| File | Nuclide / material | Library | MAT | Size | Source | Date accessed |
|---|---|---|---|---|---|---|
| `a-002_He_004-ENDF8.0.endf` | He-4 | ENDF/B-VIII.0 (neutron) | 228 | 8 KB | NNDC/IAEA | 2026-07-20 |
| `n-001_H_002-ENDF8.0.endf` | H-2 (deuterium) | ENDF/B-VIII.0 (neutron) | 128 | 127 KB | NNDC/IAEA | 2026-07-20 |
| `n-018_Ar_37-tendl2023.endf` | Ar-37 | TENDL-2023 (neutron) | 1828 | 3.7 MB | TENDL | 2026-07-20 |
| `n-092_U_235-ENDF8.0.endf` | U-235 | ENDF/B-VIII.0 (neutron) | 9228 | 35 MB | NNDC/IAEA | 2026-07-20 |
| `n-092_U_238.endf` | U-238 | ENDF/B-VIII.0 (neutron) | 9237 | 14 MB | NNDC/IAEA | 2026-07-20 |
| `tsl-013_Al_027-ENDF8.0.endf` | Al-27 (S(α,β)) | ENDF/B-VIII.0 (thermal) | 53 | 2.0 MB | NNDC/IAEA | 2026-07-20 |
| `tsl-HinZrH-ENDF8.0.endf` | H in ZrH | ENDF/B-VIII.0 (thermal) | 7 | 1.2 MB | NNDC/IAEA | 2026-07-20 |
| `tsl-CinSiC.endf` | C in 3C-SiC | ENDF/B-VIII.0 (thermal) | 44 | 6.9 MB | [NNDC ENDF/B-VIII.0](https://www.nndc.bnl.gov/endf-b8.0/download.html) | 2026-08-14 |
| `tsl-SiinSiC.endf` | Si in 3C-SiC | ENDF/B-VIII.0 (thermal) | 43 | 6.9 MB | [NNDC ENDF/B-VIII.0](https://www.nndc.bnl.gov/endf-b8.0/download.html) | 2026-08-17 |
| `tsl-SiinSiC.readme` | — (evaluator's generation notes for MAT 43) | ENDF/B-VIII.0 (thermal) | 43 | 2 KB | shipped with the tape | 2026-08-17 |
| `tsl-crystalline-graphite.endf` | C in graphite | ENDF/B-VIII.0 (thermal) | 30 | 8.3 MB | [NNDC ENDF/B-VIII.0](https://www.nndc.bnl.gov/endf-b8.0/download.html) | 2026-08-14 |
| `tsl-reactor-graphite-10P.endf` | C in graphite, 10 % porosity | ENDF/B-VIII.0 (thermal) | 31 | 8.3 MB | [NNDC ENDF/B-VIII.0](https://www.nndc.bnl.gov/endf-b8.0/download.html) | 2026-08-14 |
| `tsl-reactor-graphite-30P.endf` | C in graphite, 30 % porosity | ENDF/B-VIII.0 (thermal) | 32 | 8.3 MB | [NNDC ENDF/B-VIII.0](https://www.nndc.bnl.gov/endf-b8.0/download.html) | 2026-08-14 |

### Still wanted

| File | Nuclide | Library | MAT | Source | Date accessed |
|---|---|---|---|---|---|
| `n-009_F_019-ENDF8.0.endf` | F-19 | ENDF/B-VIII.0 (neutron) | 925 | _(fill in: IAEA NDS / NNDC)_ | _(fill in)_ |
| `n-008_O_016-ENDF8.0.endf` | O-16 | ENDF/B-VIII.0 (neutron) | 825 | _(fill in)_ | _(fill in)_ |
| `tsl-HinH2O.endf` | H in H₂O | ENDF/B-VIII.0 (thermal) | 1 | _(fill in)_ | _(fill in)_ |

## The two SiC tapes share one coherent-elastic section — do not double-count

`tsl-CinSiC.endf` (MAT 44) and `tsl-SiinSiC.endf` (MAT 43) carry **byte-identical
MF=7/MT=2 sections**: 1044 records apiece, differing only in the header line's
`ZA`/`AWR`. Verified 2026-08-17 by extracting MT=2 from both tapes and diffing
columns 1–66 (the data columns, excluding the MAT number) — 1043 of 1044 lines
match exactly.

That is correct evaluation practice, not an error. Coherent elastic (Bragg)
scattering is a property of the **3C-SiC lattice as a whole**, not of one
sublattice, so the evaluators computed one structure factor and delivered it in
both materials. Measured at 0.0253 eV / 296 K, barn per principal atom:

| Material | elastic | inelastic | total | free-gas | change |
|---|---|---|---|---|---|
| C in SiC (MAT 44) | 2.94078 | 0.13880 | 3.07957 | 4.9382 (C) | −37.6 % |
| Si in SiC (MAT 43) | 2.94078 | 0.06615 | 3.00693 | 1.9914 (Si-28) | +51.0 % |

**The trap:** a transport code that builds a SiC region from both S(α,β)
materials and sums their elastic channels counts the same Bragg scattering
twice. Attribute MT=2 to the compound once — assign it to one sublattice, or
split it — rather than letting both nuclides carry it. This is live for the
TRISO SiC layer (bead `op-t33q`).

The evaluator's own `tsl-SiinSiC.readme` records that MT=2 "was generated using
an 'in-house' routine", citing Zhu & Hawari's generalized coherent-elastic
formulation (ICNC 2015) — i.e. **not** stock LEAPR, which is why regenerating
either deck through the NJOY port yields no elastic channel at all.

## Why not inside a crate

`cargo package` builds a tarball by walking the crate root, so **any** file under
a crate directory is a candidate for publication, and crates.io caps a package at
10 MB. Keeping tapes here — outside `crates/` — means the layout enforces the
limit rather than an `include`/`exclude` allowlist that has to stay correct.
`crates/njoy-outram-park-fork/tests/no_endf_inside_crates.rs` asserts the
invariant, failing if any `.endf` reappears under `crates/`.

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

Crate V&V tests resolve tapes through
[`njoy_outram_park_fork::reference_data`] — `reference_endf("<file>")` returns
`Option<PathBuf>`, and `reference_endf_or_skip("<file>", "<label>")` prints a
skip note when absent. It honours the `OUTRAM_PARK_ENDF_DIR` environment
override and otherwise reads this folder via
`env!("CARGO_MANIFEST_DIR")/../../reference-data/endf/`. Tests **skip
gracefully** when a tape is absent — so the crate still builds and tests without
this folder populated, including for a crates.io consumer with no repository
around it. The matching NJOY golden output
is generated on demand from the locally-built `vendor/njoy2016` oracle (which is
gitignored), never committed as a large tape; only extracted reference values
(CSV) are committed, per the existing `u238_doppler` pattern.
