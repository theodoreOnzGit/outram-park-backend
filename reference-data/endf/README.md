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

#### FHR TRISO-pebble neutron tapes (added 2026-09-10, `op-mzvp.2.6`)

The nuclide inventory for the explicit-TRISO and ring-RPT pebble V&V cases
(19.9 % HALEU UCO kernel, PyC/SiC layers, graphite matrix + shell, FLiBe
coolant). Copied from a local ENDF/B-VIII.0 neutron sublibrary mirror
(`ENDF-B-VIII.0_neutrons`, originally NNDC/IAEA NDS). All are LRU/LRF Reich-Moore
(LRF=3) or File-3-only and reconstruct through `Nuclide::from_endf_file`
(verified 2026-09-10 — U-235 σ_a(0.0253 eV) = 685.5 b, U-238 σ_a = 2.68 b, etc.).

| File | Nuclide | Library | MAT | Size | Source | Date accessed |
|---|---|---|---|---|---|---|
| `n-092_U_234-ENDF8.0.endf` | U-234 | ENDF/B-VIII.0 (neutron) | 9225 | 1.9 MB | NNDC/IAEA NDS | 2026-09-10 |
| `n-003_Li_006-ENDF8.0.endf` | Li-6 | ENDF/B-VIII.0 (neutron) | 325 | 752 KB | NNDC/IAEA NDS | 2026-09-10 |
| `n-003_Li_007-ENDF8.0.endf` | Li-7 | ENDF/B-VIII.0 (neutron) | 328 | 384 KB | NNDC/IAEA NDS | 2026-09-10 |
| `n-004_Be_009-ENDF8.0.endf` | Be-9 | ENDF/B-VIII.0 (neutron) | 425 | 1.3 MB | NNDC/IAEA NDS | 2026-09-10 |
| `n-006_C_012-ENDF8.0.endf` | C-12 | ENDF/B-VIII.0 (neutron) | 625 | 3.7 MB | NNDC/IAEA NDS | 2026-09-10 |
| `n-006_C_013-ENDF8.0.endf` | C-13 | ENDF/B-VIII.0 (neutron) | 628 | 3.4 MB | NNDC/IAEA NDS | 2026-09-10 |
| `n-008_O_016-ENDF8.0.endf` | O-16 | ENDF/B-VIII.0 (neutron) | 825 | 40 MB | NNDC/IAEA NDS | 2026-09-10 |
| `n-009_F_019-ENDF8.0.endf` | F-19 | ENDF/B-VIII.0 (neutron) | 925 | 840 KB | NNDC/IAEA NDS | 2026-09-10 |
| `n-014_Si_028-ENDF8.0.endf` | Si-28 | ENDF/B-VIII.0 (neutron) | 1425 | 1.8 MB | NNDC/IAEA NDS | 2026-09-10 |
| `n-014_Si_029-ENDF8.0.endf` | Si-29 | ENDF/B-VIII.0 (neutron) | 1428 | 1.7 MB | NNDC/IAEA NDS | 2026-09-10 |
| `n-014_Si_030-ENDF8.0.endf` | Si-30 | ENDF/B-VIII.0 (neutron) | 1431 | 1.5 MB | NNDC/IAEA NDS | 2026-09-10 |

Graphite and SiC S(α,β): `tsl-crystalline-graphite.endf` /
`tsl-reactor-graphite-10P.endf` / `-30P.endf` and `tsl-CinSiC.endf` /
`tsl-SiinSiC.endf` are already present (rows above).

#### Cl-35 ENDF/B-VII.1, the `LRF=7` (R-matrix limited) evaluation (added 2026-09-11, `op-cjw.2`/`op-cjw.4`)

| File | Nuclide | Library | MAT | Size | Source | Date accessed |
|---|---|---|---|---|---|---|
| `n-017_Cl_035-ENDF7.1.endf` | Cl-35 | ENDF/B-VII.1 (neutron), ORNL/LANL evaluation (Sayer, Guber, Leal, Larson, Young; `$Rev:: 532`, 2011-12-05, DIST-DEC06) | 1725 | 2.9 MB | NJOY2016 upstream test suite, `tests/resources/cl35rml` (https://github.com/njoy/NJOY2016, `master`, fetched through raw.githubusercontent.com — the NNDC/IAEA hosts answer 403 from this environment) | 2026-09-11 |

The only committed evaluation with an **R-matrix-limited resolved range**
(`LRU=1/LRF=7`, `KRM=3` Reich-Moore, `IFG=0`, 1e-5 eV – 1.2 MeV, 8 spin
groups, particle pairs `(γ, n, p)` so `MT=2/102/600` all come from the
R-matrix) **and an MF=32 for it** (`LCOMP=2`, 1088 parameter uncertainties,
3093 `NDIGIT=2` INTG correlation lines). It carries no MF=33; the ERRORR
oracle deck inserts the dummy `MT=1/2/102/600` sections with NJOY's own
`999` option (`errorr::covadd` in the crate). Every spin group lists the
eliminated capture channel first, so the `reorder_eliminated_channel`
correction (bug `op-cjw.3`) is *not* exercised by this tape. Oracles:
`../reconr/cl35-ENDF7.1-0K-err0.01.pendf` (SAMM kernel,
`tests/reconr_cl35_rml_njoy_golden.rs`) and
`../errorr/cl35-ENDF7.1-293.6K*` (`rpxsamm`,
`tests/errorr_mf32_cl35_rml_golden.rs`). Byte-identical to the upstream
resource (`sha256` recorded in the commit that added it). ENDF/B-VII.1 is
open, published evaluated data (NNDC/BNL), redistributed unchanged.

#### U-238 JENDL-3.3, the `LRF=3`/`LCOMP=1` MF=32 evaluation (added 2026-09-11, `op-gh96`)

| File | Nuclide | Library | MAT | Size | Source | Date accessed |
|---|---|---|---|---|---|---|
| `n-092_U_238-JENDL3.3.endf` | U-238 | JENDL-3.3 (neutron), JAEA (T. Kawano et al., EVAL-Mar00, DIST-MAR02 REV4-FEB02; "Retrieved by E4-util 2007/04/12") | 9237 | 2.7 MB | NJOY2016 upstream test suite, `tests/resources/J33U238` (https://github.com/njoy/NJOY2016, `master`, fetched through raw.githubusercontent.com) | 2026-09-11 |

The `LCOMP=1` (general covariance LIST) case NJOY2016's own tests 15–17
use: MF=2 has ten `LRU=1/LRF=3` Reich-Moore ranges of 1 keV from 1e-5 eV
to 10 keV plus an `LRU=2/LRF=2` URR to 150 keV; MF=32 carries one
`LCOMP=1` short-range block per resolved range (`NSRS=1`, `MPAR=3`,
26–37 resonances each) and an `LRU=2` block, plus MF=31/33/34. It is the
oracle for ERRORR's ERRORJ Reich-Moore branch (`ggrmat`) and `LCOMP=1`
reader (`tests/errorr_mf32_j33u238_lrf3_golden.rs`, decks and tapes in
`../errorr/u238-JENDL3.3-*`). SHA-256
`e1a6fad0d5a55f5580c68f54c3322ea2eb4fb7af7fa967e6396006b9ddbda6fd`,
byte-identical to the upstream resource. JENDL-3.3 is open, published
evaluated data (JAEA Nuclear Data Center), redistributed unchanged.

#### Ar-37 MF=2 L-block-order variant (added 2026-09-10, `op-cjw.1`)

| File | Nuclide | Library | MAT | Size | Source | Date accessed |
|---|---|---|---|---|---|---|
| `n-018_Ar_37-tendl2023-mf2-L1-last.endf` | Ar-37 | TENDL-2023 (neutron), **modified** | 1828 | 3.7 MB | derived from `n-018_Ar_37-tendl2023.endf` (row above) | 2026-09-10 |

**This is not an evaluation — it is `n-018_Ar_37-tendl2023.endf` with one
edit:** in MF=2/MT=151 the resolved range's `L=1` LIST head (which carries
zero resonances) is moved after the `L=2` block, and the line sequence
numbers of MF=2 are renumbered. Nothing else differs (`diff` shows those 96
lines only). It exists because NJOY2016 `ac5adf5`'s ERRORR aborts on the
original tape (`***error in rpxlc12***problem`): its MF=32 resonance search,
`errorr.f90:4363-4380`, only advances the MF=2 pointer inside
`if (ipara.ne.0)`, so an empty L-block leaves it stale and the `L=2`
resonances are never found. MLBW sums over L, so the reorder is
physics-identical; NJOY's own RECONR+BROADR PENDF from the two tapes is
byte-identical. Used only as the ERRORR MF=32 oracle input
(`../errorr/ar37-tendl2023-L1last-*`); every other Ar-37 test reads the
unmodified tape.

#### NJOY2016 upstream test-suite tapes (added 2026-09-10, `op-cjw.1`)

Six evaluations taken from the **NJOY2016 upstream regression suite**,
`tests/resources/`, at commit
[`ac5adf5`](https://github.com/njoy/NJOY2016/commit/ac5adf5f33d893e42f2eed7fb286b0d51c7580da)
(2026-04-06). Same provenance route as `n-017_Cl_035-ENDF7.1.endf` above: the
public data hosts (`www-nds.iaea.org`, `www.nndc.bnl.gov`) answer 403 from this
build environment, but github.com does not, and these are the exact tapes
upstream itself regression-tests against — which makes them the *right* oracle
inputs, not merely the reachable ones. NJOY2016 is distributed under its own
LICENSE (see `upstream_source/NJOY2016/LICENSE`); the evaluations themselves are
open published nuclear data (ENDF/B-VIII.0/8.1, JENDL-3.3), unchanged from
upstream except for the two renames noted below.

| File | Nuclide | Library | MAT | Size | Upstream name | What it covers that nothing else here does |
|---|---|---|---|---|---|---|
| `n-025_Mn_055-ENDF8.0.endf` | Mn-55 | ENDF/B-VIII.0 (neutron) | 2525 | 5.9 MB | `n-025_Mn_055-ENDF8.0.endf` | **MF=32 with `LCOMP=1`** on an `LRU=1/LRF=3` Reich-Moore range — the general (long) resonance-parameter covariance format. `MPAR=3`, `NRB=187` resonances, `NVS=158763` = the full 561×561 lower-triangle plus 6×187 parameter values. Closes the `LCOMP=1` gap in [#171](https://github.com/theodoreOnzGit/outram-park-backend/issues/171). |
| `n-094_Pu_239-JENDL3.3.endf` | Pu-239 | JENDL-3.3 (neutron) | 9437 | 6.1 MB | `J33Pu239` | A second, **fissile** `LCOMP=1` case (three `LRU=1/LRF=3` subranges), so the ERRORR fission-covariance path is exercised too, not only capture/elastic. |
| `n-092_U_238-JENDL3.3.endf` | U-238 | JENDL-3.3 (neutron) | 9237 | 2.7 MB | `J33U238` | `LCOMP=1` across **ten** `LRU=1/LRF=3` subranges — the multi-subrange stitching the single-range Mn-55 case cannot reach. Also a second-library U-238 for the URR work (`op-mzvp`), independent of the ENDF/B-VIII.0 `n-092_U_238.endf` already here. |
| `n-042_Mo_095-ENDF8.0-beta.endf` | Mo-95 | ENDF/B-VIII.0 beta (neutron) | 4234 | 2.3 MB | `n-042_Mo_095-beta.endf` | A **second `LRU=1/LRF=7`** R-matrix-limited evaluation with MF=32 (`LCOMP=2`). Cl-35 was the only one; a single tape cannot distinguish "the port is right" from "the port matches Cl-35". |
| `n-038_Sr_088-ENDF8.1.endf` | Sr-88 | ENDF/B-VIII.1 (neutron) | 3837 | 481 KB | `n-038_Sr_088-ENDF8.1.endf` | A **third** `LRF=7` case, and by far the smallest (481 KB) — cheap enough for a unit test rather than an integration test. |
| `n-026_Fe_058-ENDF8.0-Beta4.endf` | Fe-58 | ENDF/B-VIII.0 beta4 (neutron) | 2637 | 7.9 MB | `n-026_Fe_058-ENDF8.0-Beta4.endf` | The only **`LRU=2/LRF=1`** (energy-**independent** unresolved widths, `LSSF=1`, 3.5e5–3.0e6 eV) tape here. Every other unresolved range in this folder is `LRF=2`, so the UNRESR/PURR `LRF=1` branch has never been run on real data. |

Renamed from upstream: `J33U238` -> `n-092_U_238-JENDL3.3.endf` and `J33Pu239` ->
`n-094_Pu_239-JENDL3.3.endf`, to match this folder's `n-ZZZ_El_AAA-<lib>.endf`
convention. `n-042_Mo_095-beta.endf` -> `n-042_Mo_095-ENDF8.0-beta.endf` for the
same reason. Contents are byte-identical to upstream.

Two things the upstream suite does **not** have, checked by scanning all 68 of
its `tests/resources/` tapes (see "Still wanted"): **no `LRF=4` (Adler-Adler)
anywhere**, and **no tsl with `B(7)=0`** — every `NS>=1` thermal tape upstream
carries (`t322` ENDF/B-III polyethylene, `t404` ENDF/B-IV MAT 1269,
`tsl-HinH2O-ENDF8.0-Beta6`) has `B(7)=1`, free gas.

### Still wanted

`tsl-HinH2O.endf` was the only entry here; it landed 2026-09-10 (see
"Maintainer-supplied evaluations" below). What is still missing is tracked in
GitHub Issues, one issue per tape, each carrying the screening already done so
nobody repeats it:

| Need | Unblocks | Issue | Status |
|---|---|---|---|
| MF=32/MT=151 with `LCOMP=1` | `op-cjw.1` / `op-cjw.4` | [#171](https://github.com/theodoreOnzGit/outram-park-backend/issues/171) | **Landed 2026-09-10** — Mn-55, U-238/JENDL-3.3, Pu-239/JENDL-3.3 (section above) |
| MF=32/MT=151 with `LCOMP=0` | same | [#171](https://github.com/theodoreOnzGit/outram-park-backend/issues/171) | **Still open.** Not present on any of the 68 upstream test tapes either. `LCOMP=0` is the deprecated ENDF-5-carryover layout; `rpxlc0` stays `NotPorted` until a tape exists. |
| Adler-Adler resolved range (`LRU=1`, `LRF=4`) | `op-cjw.5` | [#172](https://github.com/theodoreOnzGit/outram-park-backend/issues/172) | **Still open.** Not on any of the 68 upstream tapes — including the ENDF/B-III (`t322`), ENDF/B-IV (`t404`), ENDF/B-V (`t511`) and ENDF/B-VI (`e6pu241c`, `eni61`) legacy tapes NJOY2016 itself ships. **NJOY2016 has no Adler-Adler regression test of its own.** |
| tsl with `NS>=1` **and** `B(7)=0` (SCT secondary) | `op-cjw.20` | [#173](https://github.com/theodoreOnzGit/outram-park-backend/issues/173) | **Still open.** Not upstream either: every `NS>=1` thermal tape in their suite is `B(7)=1` (free gas). |
| ENDF-102 (ENDF-6 Formats Manual) PDF — a document, not a tape | `op-z1hk` | [#174](https://github.com/theodoreOnzGit/outram-park-backend/issues/174) | **Still open.** `www.nndc.bnl.gov` is 403 from here. |

Screening behind those rows, so it is not re-derived: all 68 tapes in the
NJOY2016 upstream `tests/resources/` were parsed for every MF=2/MT=151 range
header (`LRU`/`LRF`), every MF=32/MT=151 `LCOMP`, and every MF=7/MT=4 `NI`/`NS`/
`B(7)`. Within this folder before that pass, **only `n-017_Cl_035-ENDF7.1.endf`
and `n-018_Ar_37-tendl2023.endf` carried MF=32 at all, and both were `LCOMP=2`**.

The three still-open tape needs are all **legacy or rare-option formats**: the
last two have no oracle upstream either, so they are not simply "we did not look
hard enough" — a tape has to come from an ENDF/B-V-era archive or a
special-purpose evaluation. Treat them as low priority against work that has
data to run on.

**Not** a data need, despite an earlier claim to the contrary: the
charged-particle-elastic `sig=1` branch in GROUPR `getsig` (`op-urh`) needs an
incident-charged-particle evaluation with MF=3/MT=2, and
`a-002_He_004-ENDF8.0.endf` (MAT 228, incident alpha) already has one. That gap
is porting work, not a missing tape.

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
  **2026-09-10 finding:** the committed `n-009_F_019-ENDF8.0.endf` and
  `n-008_O_016-ENDF8.0.endf` carry `LRP=0` / a single `LRU=0` range — no
  resonance parameters at all — so neither is an LRF=7 case. **2026-09-11:**
  the SAMM path is now verified on Cl-35 (ENDF/B-VII.1, row above) instead;
  an evaluation whose eliminated channel is *not* listed first is still wanted
  for the `op-cjw.3` reorder.
- **`n-008_O_016-ENDF8.0.endf`** — O-16, also LRF=7 in VIII.0. A simpler second
  SAMM case, and doubles as a clean elastic-scatter-matrix golden for GROUPR
  (bead **op-3ut**): light nuclide, no unresolved range. (See the F-19 note:
  the committed VIII.0 O-16 file has no resonance parameters.)

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

## Maintainer-supplied evaluations (2026-09-10)

Both public data hosts (`www-nds.iaea.org`, `www.nndc.bnl.gov`) refuse downloads
from the build environment, so these were supplied directly by the maintainer.

| File | Material | Why it was needed |
|---|---|---|
| `tsl-HinH2O.endf` (17.4 MB) | MAT 1, H in H2O; MF=7/MT=4 only (no MT=2) | The canonical THERMR case (`op-cjw.23`), and the **first tsl tape in this folder with a secondary scatterer**: `NS=1`, where all seven pre-existing tsl tapes have `NS=0`. Header: `LAT=1`, `LASYM=0`, `LLN=0`, `NI=12`, `NS=1`, `B(1..12) = 40.8722, 395.26, 0.999167, 10.0001, 0, 2, 1, 3.7939, 15.8575, 0, 0, 1`. |
| `photoat-092_U_000-ENDF8.0.endf` (1.3 MB) | MAT 9200, U (Z=92) | A **real photoatomic evaluation** for GAMINR (`op-iq7`), replacing the synthetic Z=6 tape below as the verification input. MF=23 MT=501/502/504/515/516/517/522 plus 37 discrete photoelectric subshell reactions (MT=534-570); MF=27 MT=502/504/505/506 — i.e. form factors **and** the real/imaginary anomalous-scattering functions, which the synthetic tape does not carry. |

**Caveat on `op-cjw.20` — read before assuming this unblocks it.** That bead wants
the secondary-scatterer **SCT** term for `B(7)=0` evaluations (`teff2`).
`tsl-HinH2O.endf` has `B(7)=1`, i.e. the secondary scatterer (oxygen) is treated
as a **free gas**, not by the short-collision-time approximation. So this tape
exercises the `NS>0` machinery for the first time, but it does **not** reach the
`B(7)=0` SCT path. A `B(7)=0` evaluation is still needed for `op-cjw.20`.

## Synthetic test tape (not an evaluation)

| File | Material | Contents | Provenance |
|---|---|---|---|
| `photoat-synthetic-Z6.endf` (8 KB, 149 lines) | "MAT 600", ZA 6000, Z = 6 | MF=1/451; MF=23 MT=501/502/504/516/522 (lin-lin, 53 energies 1 keV–100 GeV); MF=27 MT=502 form factor, MT=504 scattering function (24 momentum transfers 0–1e9 /Å) | **Synthetic**, generated 2026-09-10 by `photoat-synthetic-Z6.generator.py` (committed next to it) from analytic shapes — coherent `2.4/(1+(E/3e4)^2)`, incoherent `3.99 (1+E/511 keV)^-0.9 (1-0.3 e^{-E/1e4})`, photoelectric `4e3 (E/1 keV)^-3`, pair `0.2 ln(E/1.022 MeV)`, `F(x) = 6/(1+(x/0.6)^2)^2`, `S(x) = 6(1-1/(1+(x/0.5)^2))`. Not evaluated data; exists only so NJOY2016 GAMINR and this crate can be run on the same photoatomic input (both public photoatomic data hosts refuse downloads from the build environment). |
