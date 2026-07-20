# Upstream source

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


- **Project:** OpenFOAM
- **Repository:** <https://github.com/OpenFOAM/OpenFOAM-dev>
- **License:** GPL-3.0
- **Branch tracked:** `master` (when cloned — see below)
- **Commit at last sync:** not currently tracked — no persistent local clone (see note)
- **Clone command:** `git clone --depth 1 https://github.com/OpenFOAM/OpenFOAM-dev.git upstream_source/OpenFOAM`

## Provenance

`openfoam-turbulence-lib` is a pure-Rust port of OpenFOAM's turbulence model
library (RAS and LES closures: k-ε, k-ω, k-ω SST, Spalart-Allmaras,
Smagorinsky, …) for use with `openfoam-appbuilder-lib` solver loops.

**Note:** this crate does not currently maintain a persistent local OpenFOAM
clone with an automated data-driven codegen pipeline (unlike
`outram-park-fork-coolprop`) — translation is done by reading OpenFOAM's C++
turbulence-model source directly. Clone it with the command above when doing
line-by-line porting/verification work.

## Licensing note

OpenFOAM is GPL-3.0, matching this crate's own license directly. This is an
**independent translation**, not an OpenFOAM Foundation/ESI release, and is
not endorsed by or affiliated with the OpenFOAM Foundation, ESI Group, or
OpenCFD Ltd.

---

If a clone is added here, it is expected to be **gitignored** — never
committed, present for development only.
