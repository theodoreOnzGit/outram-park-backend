# Relicensing OUTRAM PARK to AGPL-3.0 — the record (2026-10-20)

**Maintainer direction, 2026-10-20 (gh:#973):** relicense PETIR and all of
OUTRAM PARK to AGPL-3.0. Standing direction the same day, recorded in the root
`CLAUDE.md`: *"AGPLv3 is acceptable also, never ask me about it again."*

**This file is the authoritative per-crate record.** Each crate's own `NOTICE`
carries a dated pointer to it rather than a copy, so there is one statement of
the licence position and not 48 that can drift. The audit the position rests on
is the first comment on gh:#973, posted before anything was changed.

Everything below was measured in the tree at `884ce24c0` or fetched from the
crates.io API on 2026-10-20. Nothing is recalled.

## The rule, and why most files keep their GPL-3.0 header

A translation is a derivative work of its upstream, so the upstream licence
decides whether the translation may be conveyed under AGPL-3.0:

- **Permissive upstream** (MIT, BSD-2/3-Clause, Apache-2.0, Zlib, ISC, public
  domain) — the translation **may** be conveyed under AGPL-3.0, with the
  upstream notice and attribution kept. Such a file's SPDX header may become
  `AGPL-3.0-only`.
- **Copyleft upstream** (GPL-3.0-only, GPL-3.0-or-later, GPL-2.0-or-later,
  LGPL-2.1, LGPL-3.0) — the translation **may not** become AGPL-3.0.
  `-or-later` does not help: AGPLv3 is not a later version of the GPL. The
  route is **GPLv3 §13 / AGPLv3 §13 combination** — the ported files stay
  GPL-3.0, and the *crate* conveys as a combined work carrying AGPL's
  network-interaction obligation. This is the mechanism `crates/kovan/NOTICE`
  has described since 2026-08-21.
- A **crates.io dependency** under GPL-3.0 does not stop a crate's own files
  being AGPL-3.0; §13 covers the combination. The classification below is
  about vendored or ported code *inside* a crate, not its dependency graph.

**SPDX expression style follows the kovan precedent**: the crate's `license`
field is a plain `AGPL-3.0-only`, and the GPL/MIT/BSD parts are named in its
`NOTICE` rather than written into an `AND` expression. Chosen for consistency
with the nine crates already on AGPL; an `AND` expression is the more literally
precise label, and switching to one is a maintainer decision.

**`-only`, not `-or-later`**, matching all nine crates that were already AGPL.

## Licence texts, and where they live

- Root `LICENSE` — the **AGPL-3.0** text. Root `LICENSE.GPL-3.0` — the
  **GPL-3.0** text, for the ported files that still need it.
- Every relicensed crate's own `LICENSE` is the AGPL-3.0 text.
- Every **combine-only** crate additionally ships `LICENSE.GPL-3.0`, because
  GPLv3 §4 requires the GPL text to travel with the covered work and a
  crates.io tarball contains only the crate's own directory.
- Per-crate upstream licence files (`LICENSE.njoy`, `LICENSE.openmc`,
  `LICENSE.blender`, `LICENSE.flexpart`, `LICENSE-APACHE-RAVEN`,
  `src/rustbca/LICENSE`, …) are **untouched**, as are every file's upstream
  provenance header.

## Published versions keep their licence forever

31 of the 48 member crates have versions on crates.io. Relicensing changes
future versions only; anyone depending on a published version keeps the grant
it was published under. The per-crate tables below name what is grandfathered.
**No version was bumped** — bumping is the maintainer's call (gh:#971, #972).

## Already AGPL-3.0-only before this change (9)

`kovan`, `kovan-web`, `kovan-common`, `kovan-literature`, `kovan-discovery`,
`kovan-semantics`, `kovan-metrics`, `kovan-codegen` (the family, since
2026-08-21 and 2026-10-07) and `dhoby-ghaut` (since 2026-10-05). Unchanged by
this pass.

## Fully relicensable — file SPDX headers may become AGPL-3.0 (16)

Their vendored upstreams are permissive or absent.

| Crate | Vendored/ported upstream | Published versions grandfathered |
|---|---|---|
| `njoy-outram-park-fork` | NJOY2016, modified BSD-3-Clause (LANL/DOE); MIT (mit-crpg / WMP) | 0.0.1–0.0.4 GPL-3.0-only |
| `outram-mc-libs` | OpenMC, MIT (MIT + UChicago Argonne) | 0.0.1–0.0.5 GPL-3.0-only |
| `outram-park-fork-coolprop` | CoolProp, MIT | 0.1.0–0.1.2 GPL-3.0-only |
| `outram-park-fork-onix` | ONIX, MIT | none |
| `outram-park-fork-thermochimica` | Thermochimica, BSD-3-Clause (ORNL) | none |
| `kaki-bukit` | CYCLUS / CYCAMORE, BSD-3-Clause + BSD-2-Clause | none |
| `outram-park-mpi` | MPICH subset, BSD-style | none |
| `outram-park-fork-liggghts` | none — an independent implementation; LIGGGHTS-PUBLIC (GPL-2.0-or-later) was read as a reference only | 0.0.1 GPL-3.0-only |
| `buangkok` | BSD-3-Clause, MIT, public domain | none |
| `outram-park-digital-twin-engine` | in-house (colour maps ported from the maintainer's own v1 simulator) | 0.0.1–0.0.2 GPL-3.0-only |
| `nee_soon` | none found — an integration layer | 0.0.1–0.0.3 GPL-3.0-only |
| `dover` | none vendored; consumes `outram-park-fork-dwsim-libs` as a dependency | none |
| `bishan`, `sembawang`, `redhill`, `pasir-ris` | placeholders, no code | none |

## Combine-only — ported files keep GPL-3.0; the crate is an AGPL combined work (18)

Each ships `LICENSE` (AGPL-3.0) and `LICENSE.GPL-3.0`, and **no file SPDX
header changed**.

| Crate | Copyleft-derived content | Published versions grandfathered |
|---|---|---|
| `petir` | GSL, **GPL-3.0-or-later** (`NOTICE` lines 46–49); GNU Octave GPL-3.0-or-later (`transfer_fn`); OpenFOAM verbatim lifts. Its permissive parts (OpenMC MIT, PyClaw BSD-3, `roots` BSD-2, `peroxide` MIT, ARM optimized-routines) would be relicensable; the GSL core is not, and by the crate's own "ported, never invented" rule there are essentially no maintainer-original numerics | 0.0.1 GPL-3.0-only |
| `outram-foam-basic-lib` | OpenFOAM GPL-3.0-or-later | 0.1.0–0.1.4 |
| `outram-foam-appbuilder-lib` | GeN-Foam / OpenFOAM GPL-3.0-or-later (124 ported files) | 0.1.0–0.1.4 |
| `outram-foam-turbulence-lib` | OpenFOAM GPL-3.0-or-later | 0.1.0–0.1.2 |
| `outram-foam-multiphase` | OpenFOAM GPL-3.0-or-later | 0.0.1–0.0.2 |
| `outram-foam-mesh` | OpenFOAM GPL-3.0-or-later | 0.1.0 |
| `outram-foam-cli` | OpenFOAM lineage. One maintainer-owned file is deliberately `GPL-3.0-or-later`: `tests/ci_smoke/sod.rs` | none |
| `outram-park-fork-cfmesh` | cfMesh GPL-3.0; voro++ BSD | 0.0.1–0.0.2 |
| `outram-park-fork-dwsim-libs` | DWSIM GPL-3.0 (20 ported files) | 0.0.1–0.0.3 |
| `outram-park-fork-offbeat` | OFFBEAT GPL-3.0; MIT parts | none |
| `outram-park-fork-pflotran` | PFLOTRAN LGPL-3.0, elected as GPL-3.0 | none |
| `outram-park-fork-moltres` | Moltres LGPL-2.1 → GPL-3.0 (6 ported files) | none |
| `farrer-park` | MOOSE / PRISMS-Plasticity / PRISMS-Fatigue, LGPL-2.1 → GPL-3.0 by LGPL-2.1 §3 | 0.0.1 |
| `changi` | FLEXPART GPL-3.0-or-later (36 ported files); `puff` MIT | none |
| `outram-blender` | Blender GPL-2.0-or-later → GPL-3.0 (73 ported files); MIT / BSD-3 / Apache-2.0 / Zlib / public-domain parts | 0.0.1–0.0.5 |
| `boon-lay` | RustBCA GPL-3.0-or-later, copied in whole as `src/rustbca/`; TRISO-ATOPS MIT (Battelle) | 0.1.3, 0.2.0 |
| `chem-eng-real-time-process-control-simulator` | GNU Octave control package GPL-3.0-or-later (`z_domain`); BSD-3 | 0.0.1–0.1.1 **Apache-2.0**, 0.2.1–0.2.2 GPL-3.0-only |
| `teh-o-prke` | `matrix::SquareMatrix` copied in, itself an OpenFOAM translation (`README.md:48`) | 0.0.1–0.1.6 |

## ~~HELD at GPL-3.0-only pending recorded third-party consent (5)~~ RELICENSED 2026-10-20

**CORRECTED 2026-10-20:** the maintainer directed "All crates AGPLv3", saying of the people
below "they are colleagues" and "i know them". That direction, given in the coordinating
session, stands in place of the per-person recorded consent this section asked for. All five
now inherit `license.workspace = true` (AGPL-3.0-only), each with a dated NOTICE section.
`bedok` ~~and `raffles` are~~ is relicensed outright; ~~(raffles keeps `LICENSE-APACHE-RAVEN`)~~
**CORRECTED 2026-10-20:** `raffles` cannot be: it holds ports of third-party GPL-3.0 code
(SCRAM, Rakhimov, GPL-3.0-or-later; Physics-guided-MPNN, GPL-3.0; pba.r, Ferson, GPL-3.0,
since gh:#987), which only the upstream holders could relicense. It conveys as a combined
work, keeps `LICENSE-APACHE-RAVEN`, and now ships `LICENSE.GPL-3.0`;
`tuas_boussinesq_solver`, `tampines` and `tampines-steam-tables` convey as combined works,
shipping `LICENSE.GPL-3.0`, because they contain files referring to or derived from OpenFOAM
(GPL-3.0-or-later). Treating them as combine-only is the conservative reading, pending a
file-by-file check. The original text of this section follows, kept as the record.


**Maintainer note on gh:#973, 2026-10-20:** colleagues are expected to agree,
but each relicence needs that person's explicit consent **recorded** — a
comment on the issue, an approving commit trailer, or an email the maintainer
notes there — before the crate's licence field changes. Agreeing to open source
in general is not agreeing to the AGPL network clause specifically.

Each of these five carries an explicit `license = "GPL-3.0-only"` field with a
comment naming the reason, so it does **not** follow the workspace default.
Restore `license.workspace = true` once consent is recorded.

| Crate | Person | Evidence in-tree | Published |
|---|---|---|---|
| `bedok` | **Than Yan Ren** (SNRSI), copyright holder of the translated MATLAB; **Xiao SiCong** | `README.md:11,429,444`, `NOTICE:69`, `CLAUDE.md:14,20` | no |
| `raffles` | **Adolphus Lye**, crate owner and copyright holder of all seven source repositories; the current GPL-3.0 is by their direct grant | `README.md:30,137–142` | 0.0.1 |
| `tuas_boussinesq_solver` | **Sicong Xiao**, **Per F. Peterson**, co-authors of the peer-reviewed paper the crate implements (doi:10.1016/j.jandt.2025.03.006) | `README.md:45,54` | 0.0.1–0.1.7 |
| `tampines` | **Ethan Yew Hoe Wong**, **Sicong Xiao**, in the crate's own `authors` field | `Cargo.toml` `authors`; root `README.md:119` (arXiv:2608.17504) | 0.0.1–0.0.2 |
| `tampines-steam-tables` | as `tampines` | same | 0.0.7–0.2.8 |

`tampines` and `tampines-steam-tables` are **not** in the issue's own list;
they were found from the `authors` fields, and whether that entry reflects code
copyright or paper co-authorship is the maintainer's to confirm. They are held
until it is.

The root `[workspace.package] authors` list names Professor Per F. Peterson, UC
Berkeley Thermal Hydraulics Lab, SNRSI and NUS for *every* crate.
`chem-eng-real-time-process-control-simulator`'s own `NOTICE` (2026-08-11)
treated that identical list as attribution and relicensed anyway, keeping the
attribution lines unchanged; that precedent is what the other 43 crates rest on.

## GPL-2.0 audit — no blocker

- All **8** `GPL-2.0-or-later` SPDX lines in the tree are **quoted upstream**
  identifiers, in 7 `outram-blender` files (`boolean_predicates.rs`,
  `planar_faces.rs`, `connect_nonplanar.rs`, `polyfill_beautify.rs`,
  `connect_concave.rs`, `limited_dissolve.rs`, `polyfill.rs`), each naming the
  Blender Authors. `-or-later` → GPL-3.0 → §13.
- **No `GPL-2.0-only` upstream exists anywhere.** The only hits for that string
  are in `outram-park-fork-liggghts`, and all four *correct* an earlier note
  that wrongly called LIGGGHTS-PUBLIC GPL-2.0-only.
- Every LGPL route lands on GPL-3.0, each already reasoned out in the crate's
  own `NOTICE`.
- **Third-party crates.io dependencies: clear.** 1,080 packages, no copyleft
  blocker. The one GPL-2.0-only appearance is `self_cell` 1.3.0, dual
  `Apache-2.0 OR GPL-2.0-only` — the Apache option is taken.
  `fission-yields-data` 0.1.5 is `GPL-3.0`; the 11 `kopitiam-*` crates are
  `AGPL-3.0-only` (the maintainer's own).

## Two instruments that do not work here, recorded so they are not retried

- **`git log --format='%an' -- crates/<c>` cannot find third-party authors in
  this repository.** Its history begins **2026-10-16** with **121** commits and
  three identities (two maintainer machines plus `Claude`); the
  pre-consolidation per-crate history is not here. It returns the maintainer
  for every crate, including `bedok`, which the issue itself names as
  third-party. Authorship evidence comes from the `authors` fields, the
  `NOTICE`s, the `README`s and `docs/ecosystem-naming.md` instead.
- **There is no mechanical way to split `.rs` files into "maintainer-owned" and
  "upstream-derived".** Header conventions differ per crate. A phrase scan for
  `Original licence` / `Upstream project` finds **298** files, but
  `crates/petir/src/specfunc/gamma.rs` — a GSL port by its crate's own
  governing rule — carries a plain GPL-3.0-only boilerplate header reading
  `Copyright (C) 2026 OUTRAM PARK contributors` and names GSL only in the `///`
  docs below, and `njoy-outram-park-fork` (464 files, wholly an NJOY2016
  translation) matches on 1. So the partition is decided per crate from the
  crate's own `NOTICE`, and in the 18 combine-only crates the answer is "change
  no file headers".

## SPDX census over `crates/**/*.rs` at `884ce24c0`

4,863 files, 1,684 carrying a header. Reproduces the issue's survey exactly.

| SPDX | Files |
|---|---|
| `GPL-3.0-only` | 1,112 |
| `GPL-3.0` | 300 |
| `MIT` | 271 |
| `GPL-2.0-or-later` | 8 |
| `AGPL-3.0-only` | 4 |
| `GPL-3.0-or-later` | 3 |

## Not in scope

Porting EGSnrc itself (the alias table, RANMAR/RANLUX and the fast
interpolator wanted in PETIR — a separate issue), and any version bump. The
EGSnrc numerics can be ported into PETIR under the same §13 combination: its
GSL-derived GPL-3.0 files and EGSnrc-derived AGPL-3.0 files coexist in one
crate lawfully.
