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

**No Blender source code is bundled or ported into this crate.** The crate
reuses Blender's mesh/geometry *architecture* — the shape of the BMesh-style
non-manifold mesh representation and the operator model. Copyright covers
expression, not design, so no upstream file is redistributed here today.

The upstream GPL-2 licence text is nevertheless shipped as `LICENSE.blender`
and the lineage recorded in `NOTICE`, so the provenance travels with the
package and the obligations are already in place the moment a literal port
lands.

**If you port a Blender algorithm into this crate**, the ported file must
carry the GPL attribution header block (upstream project, source file,
version/commit, copyright, licence) per the workspace
`RESEARCH_INTEGRITY_AND_PROVENANCE.md`. Do not strip it during refactors.

## Non-affiliation

This crate is **not affiliated with, endorsed by, or sanctioned by the Blender
Foundation**. "Blender" identifies only the upstream project this work derives
from.
