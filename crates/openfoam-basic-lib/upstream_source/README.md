# Upstream source

- **Project:** OpenFOAM
- **Repository:** <https://github.com/OpenFOAM/OpenFOAM-dev>
- **License:** GPL-3.0
- **Branch tracked:** `master` (when cloned — see below)
- **Commit at last sync:** not currently tracked — no persistent local clone (see note)
- **Clone command:** `git clone --depth 1 https://github.com/OpenFOAM/OpenFOAM-dev.git upstream_source/OpenFOAM`

## Provenance

`openfoam-basic-lib` is a pure-Rust translation of OpenFOAM's C++ primitive
and finite-volume library layer — tensor algebra, polynomial/ODE solvers,
interpolation, thermophysics kernels, fields, mesh, FV operators, and
fluid/solid thermodynamics (Layers 1–4 in the workspace's OpenFOAM-in-Rust
stack; see the workspace-root `docs/architecture.md`).

**Note:** unlike `outram-park-fork-coolprop`/`njoy-outram-park-fork`, this
crate does not currently maintain a persistent local OpenFOAM clone with an
automated data-driven codegen pipeline — the translation was done by reading
OpenFOAM's C++ source directly during development. If resuming
codegen-driven or line-by-line verification work against OpenFOAM, clone it
fresh with the command above (it is a large repository).

## Licensing note

OpenFOAM is GPL-3.0, matching this crate's own license (the OUTRAM PARK
workspace default) directly — no relicensing step is needed. This is an
**independent translation**, not an OpenFOAM Foundation/ESI release, and is
not endorsed by or affiliated with the OpenFOAM Foundation, ESI Group, or
OpenCFD Ltd.

---

If a clone is added here, it is expected to be **gitignored** — never
committed, present for development only.
