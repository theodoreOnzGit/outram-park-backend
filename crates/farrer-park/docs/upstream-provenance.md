# Farrer Park — upstream provenance

The record of what was vendored, at which commit, and what its licence actually
says. `vendor/` at the workspace root is **gitignored**, so the trees themselves
are not in this repository — this file is the durable record of what was read.

Vendored 2026-09-11 (bead `op-1zd8`), shallow clones (`--depth 1`):

| Upstream | Path | Commit | Commit date | Size |
|---|---|---|---|---|
| [idaholab/moose](https://github.com/idaholab/moose) | `vendor/moose` | `fe80d24fa4c58f677d6672e5063ec08715953c32` | 2026-09-10 | 1.1 GB |
| [prisms-center/plasticity](https://github.com/prisms-center/plasticity) | `vendor/prisms-plasticity` | `ffdf4eb67b55b84f8b20cbb21407cf310ec3a7e4` | 2026-08-27 | 205 MB |
| [prisms-center/Fatigue](https://github.com/prisms-center/Fatigue) | `vendor/prisms-fatigue` | `2c8fc9a2cc85c0b2b3dcfac987f3a9e603b7fb9f` | 2023-10-13 | 57 MB |

Reproduce with:

```bash
mkdir -p vendor
git clone --depth 1 https://github.com/idaholab/moose.git            vendor/moose
git clone --depth 1 https://github.com/prisms-center/plasticity.git  vendor/prisms-plasticity
git clone --depth 1 https://github.com/prisms-center/Fatigue.git     vendor/prisms-fatigue
```

## Licences, read from each tree's own files

Checked on 2026-09-11 against the vendored sources, not inferred from a badge or
a package index.

**MOOSE — LGPL-2.1 (not "or later").** `vendor/moose/LICENSE` is the verbatim
LGPL-2.1 text, and every source file carries the header
`//* Licensed under LGPL 2.1, please see LICENSE for details`. No "or any later
version" wording appears. Note `COPYRIGHT` at the repo root is a **U.S.
Government rights notice** arising from DOE and NSF contracts (Battelle Energy
Alliance / INL, Triad National Security / LANL) — it is *not* the licence, and
reading it alone gives the wrong answer.

**PRISMS-Plasticity — LGPL-2.1-or-later.** `vendor/prisms-plasticity/LICENSE`
opens with a custom preamble before the LGPL text:

> (c) 2016 The Regents of the University of Michigan, PRISMS Center
>
> This code is a free software; you can use it, redistribute it, and/or modify
> it under the terms of the GNU Lesser General Public License as published by
> the Free Software Foundation; **either version 2.1 of the License, or (at your
> option) any later version.**

This is *more* permissive than the other two, not less.

**PRISMS-Fatigue — LGPL-2.1.** `vendor/prisms-fatigue/LICENSE` is the verbatim
LGPL-2.1 text; the README refers to it without an "or later" grant.

## Why GPL-3.0-only is clean, and one-way

LGPL-2.1 section 3 permits a licensee to opt into the ordinary GPL instead, and
explicitly allows choosing a later GPL version. Quoting the clause the
relicensing rests on, verbatim from `vendor/prisms-plasticity/LICENSE`:

> You may opt to apply the terms of the ordinary GNU General Public License
> instead of this License to a given copy of the Library. To do this, you must
> alter all the notices that refer to this License, so that they refer to the
> ordinary GNU General Public License, version 2, instead of to this License.
> **(If a newer version than version 2 of the ordinary GNU General Public
> License has appeared, then you can specify that version instead if you wish.)**

So GPL-3.0 is explicitly permitted for all three, and Farrer Park exercises that
option — consistent with the rest of this workspace.

**The flow is ONE-WAY.** Code may come from these upstreams into this
GPL-3.0-only crate; code from this crate **cannot** go back to MOOSE or the
PRISMS codes under their LGPL terms. Section 3 also notes the change "is
irreversible for that copy". Same shape as `outram-park-fork-pflotran`
(LGPL-2.1 PFLOTRAN into GPL-3.0) and `raffles` (Apache-2.0 RAVEN into GPL-3.0).

## What is actually relevant to the port

- `vendor/moose/modules/solid_mechanics`, `tensor_mechanics`, `contact` — the
  structural-mechanics modules.
- `vendor/prisms-plasticity/src/{ellipticBVP, materialModels, enrichmentModels,
  userInputParameters, utilityObjects}` — the crystal-plasticity FEM layer
  (bead `op-q75c`).
- `vendor/prisms-fatigue` — FIPs and microstructure-sensitive fatigue
  (bead `op-q1zn`), plus the published case studies that are the parity targets
  (bead `op-9smz`).

## Why this matters operationally

The workspace "Debugging a port: read upstream first" hard rule requires reading
the upstream routine that owns a behaviour **before** hypothesising about a
discrepancy. That rule is unusable without the sources on disk. Any session
debugging a Farrer Park discrepancy should re-clone per the commands above if
`vendor/` is absent — an ephemeral container will not have it.

Per-file attribution headers remain mandatory on anything ported from these
trees; see `NOTICE`.
