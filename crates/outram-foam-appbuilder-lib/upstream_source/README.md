# Upstream source

- **Project:** OpenFOAM
- **Repository:** <https://github.com/OpenFOAM/OpenFOAM-dev>
- **License:** GPL-3.0
- **Branch tracked:** `master` (when cloned — see below)
- **Commit at last sync:** not currently tracked — no persistent local clone (see note)
- **Clone command:** `git clone --depth 1 https://github.com/OpenFOAM/OpenFOAM-dev.git upstream_source/OpenFOAM`

## Provenance

`openfoam-appbuilder-lib` is the solver-application layer for the workspace's
OpenFOAM-in-Rust stack — solver time loops (PISO/PIMPLE), polyMesh I/O, case
file parsing, and field output, ported from OpenFOAM's application/solver
C++ source, sitting on top of `openfoam-basic-lib` (primitives/FV operators)
and `openfoam-turbulence-lib` (turbulence closures).

**Note:** this crate does not currently maintain a persistent local OpenFOAM
clone with an automated data-driven codegen pipeline — translation is done by
reading OpenFOAM's C++ solver-application source directly. Clone it with the
command above when doing line-by-line porting/verification work.

## Licensing note

OpenFOAM is GPL-3.0, matching this crate's own license directly. This is an
**independent translation**, not an OpenFOAM Foundation/ESI release, and is
not endorsed by or affiliated with the OpenFOAM Foundation, ESI Group, or
OpenCFD Ltd.

---

If a clone is added here, it is expected to be **gitignored** — never
committed, present for development only.
