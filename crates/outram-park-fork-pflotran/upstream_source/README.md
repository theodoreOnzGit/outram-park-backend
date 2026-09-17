# Upstream provenance — PFLOTRAN

This directory records the provenance of the upstream **PFLOTRAN** project that
`outram-park-fork-pflotran` translates. The reference clone under `PFLOTRAN/`
is kept for porting / provenance reference only and is **gitignored — never
committed** (workspace vendor rule; see the crate `.gitignore`).

## Upstream

| Field | Value |
|---|---|
| Project | PFLOTRAN — subsurface flow & reactive transport |
| Home | https://www.pflotran.org |
| Code | https://bitbucket.org/pflotran/pflotran |
| Docs | https://documentation.pflotran.org |
| Stewardship | PNNL (Battelle), SNL (NTESS), LANL, ORNL, LBNL — US DOE national labs |
| Language | Fortran (2003+) on PETSc (MPI) |
| License | **GNU Lesser General Public License v3.0 (LGPL-3.0)** |

## Vendored clone

```bash
git clone --depth 1 https://bitbucket.org/pflotran/pflotran.git upstream_source/PFLOTRAN
```

| Field | Value |
|---|---|
| Commit | `669fbce204af39428020c6b9d74beae363376689` |
| Commit date | 2026-09-09 12:06:57 -0700 |
| Date accessed | 2026-09-10 |
| Default branch | `master` |
| Depth | shallow (`--depth 1`) |
| Size on disk | 203 MB — 3906 files, 533 `.F90` |

## License determination (2026-09-10) — VERIFIED

Superseding the 2026-07-20 documentary determination, which recorded
**LGPL-2.1-or-later** from the PFLOTRAN documentation and OSTI/DOECODE records
without reading the upstream files. That determination was **wrong for current
PFLOTRAN**; it described the v2.X era only (see the table below).

Verified two independent ways on 2026-09-10 — an HTTPS fetch of the raw files
and the shallow clone above — with matching SHA-256 digests:

| File | Bytes | SHA-256 |
|---|---|---|
| `LICENSE` | 43237 (850 lines) | `925e69c826a46d22362381d2742ff6ffaa4365be46646e8658967cf850a5b062` |
| `COPYRIGHT` | 5458 | `2b0798510a503d84dae708461ac6feac2cea9ba608b837473aadba8c76ad4cc6` |

`LICENSE` line 1 reads *"PFLOTRAN is released under the GNU LESSER GENERAL
PUBLIC LICENSE (LGPL)."* It then carries the **LGPL Version 3, 29 June 2007**
text (line 9) followed by the **GPL Version 3** text (line 177) — the GPL is
bundled because LGPLv3 incorporates it by reference, as the file's own preamble
explains. The file states no "or later" election of its own; the "any later
version" strings in it belong to the standard LGPLv3 §14 / GPLv3 §14 boilerplate.

### Copyright holders, by era (from `COPYRIGHT`)

| Era | Copyright holder | Contract | License as stated |
|---|---|---|---|
| v5.X (current) | Battelle Memorial Institute, 2023 (PNNL) | DE-AC05-76RL01830 | LGPL v3.0 |
| v3.X | National Technology & Engineering Solutions of Sandia, LLC (NTESS), 2020 | DE-NA0003525 | LGPL 3.0 |
| v2.X (LA-CC-09-047) | Los Alamos National Security, LLC, 2009 | DE-AC52-06NA25396 | LGPL "version 2.1 of the License, or (at your option) any later version" |

The v5.X Battelle notice additionally grants BSD-3-Clause-style permission to
redistribute in source and binary form, conditioned on retaining the copyright
notice, the list of conditions and the disclaimers, and restricting use of the
Battelle name. Those conditions are reproduced in the crate `NOTICE`.

Per-file Fortran headers do not carry full license text; they point at these two
files, e.g. `src/pflotran/pflotran.F90`:

```
!=======================================================================
! Please see LICENSE and COPYRIGHT files at top of repository.
!=======================================================================
```

### GPL-3.0 compatibility

Distributing derived work under **GPL-3.0-only** is permitted:

1. LGPLv3 is, by its own opening sentence, *"the terms and conditions of version
   3 of the GNU General Public License, supplemented by the additional
   permissions listed below."* GPLv3 §7 ¶4 provides that when conveying a copy
   of a covered work *"you may at your option remove any additional permissions
   from that copy, or from any part of it."* Removing LGPLv3's additional
   permissions yields plain GPLv3.
2. The v2.X-era LANL material is LGPL-2.1-**or-later**, so it may be taken at
   LGPL-3.0 and handled as above; LGPL-2.1 §3 independently permits electing the
   ordinary GPL.
3. Battelle's supplementary permissive grant is one-way compatible with GPL-3.0;
   its retain-notice conditions are carried in the crate `NOTICE`.

Note this crate has **no weaker option**: it links `outram-foam-basic-lib`,
`tampines-steam-tables` and `outram-park-mpi`, all GPL-3.0-only. GPL-3.0-only is
therefore the required result license, not merely a permitted one.

*This is a provenance record, not legal advice.*

## Action status

- [x] Clone upstream PFLOTRAN — done 2026-09-10, commit `669fbce`.
- [x] Read `LICENSE` / `COPYRIGHT` verbatim and record the exact text and
      per-file copyright lines — done; LGPL-3.0, not LGPL-2.1-or-later.
- [x] Confirm GPL-3.0 compatibility of the confirmed license — done, above.
- [x] Capture the clone's commit SHA and date accessed — done, above.
- [ ] **Open (bead op-2nz):** `src/` holds 26,960 lines across 47 files, while
      the crate `NOTICE` still claims no upstream source has been ported. No
      file cites an upstream `.F90`. Audit whether that code is independent work
      (state so precisely) or derived (add a provenance header to each derived
      file naming the upstream file, commit, copyright holder and license, per
      the workspace attribution rule). The clone above makes this comparison
      possible for the first time.

## Recommended citation

Cite the PFLOTRAN user manual / theory guide when using derived functionality,
e.g. Lichtner, P.C., Hammond, G.E., Lu, C., Karra, S., Bisht, G., Andre, B.,
Mills, R.T., Kumar, J. (2015), *PFLOTRAN User Manual*, and the appropriate
methods papers for the specific process models translated. Confirm the current
recommended citation against the upstream `documentation.pflotran.org` before
citing in a publication.
