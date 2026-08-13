# Vendored upstream sources (development reference only)

This directory holds the upstream projects that `outram-blender` derives its
design from or takes reference from. **The clones themselves are gitignored**
(see the crate `.gitignore`) — they are present for development/porting
reference only, are **never committed** and **never packaged** (`Cargo.toml`
`exclude`). This README is the small, committed provenance record.

GPLv3-compatible upstreams only (workspace vendor rule).

## Clone commands

```bash
# Blender (GPL-2.0-or-later) — the mesh-authoring architecture this crate's
# mesh/geometry design derives from. Layout of interest:
# source/blender/blenlib (math/geometry), source/blender/bmesh (the BMesh
# non-manifold mesh kernel), source/blender/geometry (mesh operators).
git clone --depth 1 https://github.com/blender/blender.git upstream_source/Blender
```

## Provenance

- **Project:** Blender
- **Repository:** <https://github.com/blender/blender>
- **Vendored:** commit `786af64aad84154047d93ee077e1fdd1d229f32d` (2026-08-04)
- **Licence:** **GPL-2.0-or-later**
- **Copyright:** Blender Authors; NaN Holding BV (2001–2002) on the oldest files

### Licence verification (checked 2026-08-04 against the clone above)

Verified against the actual upstream tree, not assumed:

- Root `COPYING` states Blender uses the GNU GPL and that "Apart from the GNU
  GPL, Blender is not available under other licenses." It points at
  `doc/license/GPL-license.txt`, which contains the **GPL version 2** text.
- SPDX identifiers across `source/blender` (counted by file):

  | SPDX identifier | Files |
  |---|---|
  | `GPL-2.0-or-later` | 5981 |
  | `Apache-2.0` | 167 |
  | `MIT` | 10 |
  | `BSD-3-Clause` | 3 |
  | `Zlib` | 2 |
  | `GPL-3.0-or-later` | 2 |
  | `BSL-1.0` | 2 |

**Conclusion: `GPL-2.0-or-later` is GPLv3-compatible.** The "or later" clause
permits redistribution under GPL-3.0, so this crate's `GPL-3.0-only` licence is
licence-clean with respect to Blender. (Note this is exactly why the "or later"
matters here: Blender also carries Apache-2.0 files, and Apache-2.0 is
compatible with GPLv3 but *not* with GPLv2.)

## What is actually taken from upstream

**One file is a literal port of Blender source:**

| | |
|---|---|
| File | `src/boolean_predicates.rs` |
| From | `source/blender/blenlib/BLI_math_boolean.hh`, `intern/math_boolean.cc` |
| Commit | `96294be75080bbf687fa7f108e344a1063713586` |
| Copyright | `SPDX-FileCopyrightText: 2023 Blender Authors` |
| Licence | `SPDX-License-Identifier: GPL-2.0-or-later` |

Only the `double` (floating-point) predicate API is ported — Blender's
`mpq_class` (GMP rational) overloads are deliberately **not**, because this
crate must stay Android-buildable with no C dependencies and GMP is a C
library. Blender's own `double` predicates are in turn a C++ adaptation of
Jonathan Richard Shewchuk's `predicates.c` (Carnegie Mellon University, May
1996), placed by its author in the **public domain**. The file carries all of
this in its own header block.

**Everything else is an independent reimplementation** of Blender's
mesh/geometry *architecture* — the BMesh-style non-manifold mesh
representation (vert/edge/loop/face) and the modifier/operator model,
including its naming. No Blender code is transcribed in those files;
copyright covers expression, not design.

**Consequence:** because of the ported file, this crate **is a derivative work
of GPL-2.0-or-later code**. Distributing it as `GPL-3.0-only` is permitted
precisely by the "or later" clause — so the compatibility finding above is
load-bearing, not merely precautionary. Upstream's GPL-2 text ships as
`LICENSE.blender` and the lineage is recorded in `NOTICE`.

**If you port further Blender code into this crate**, the ported file must
carry the GPL attribution header block (upstream project, source file,
version/commit, copyright, licence) per the workspace
`RESEARCH_INTEGRITY_AND_PROVENANCE.md`. Do not strip it during refactors.

## Non-affiliation

This crate is **not affiliated with, endorsed by, or sanctioned by the Blender
Foundation**. "Blender" identifies only the upstream project this work derives
from.
