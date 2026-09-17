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

`outram-foam-appbuilder-lib` is the solver-application layer for the
workspace's OpenFOAM-in-Rust stack — solver time loops (PISO/PIMPLE) plus
polyMesh and field **readers**, ported from OpenFOAM's application/solver C++
source, sitting on top of `outram-foam-basic-lib` (primitives/FV operators)
and `outram-foam-turbulence-lib` (turbulence closures).

Note that case-dictionary parsing (`ControlDict::read`, `FvSchemes::read`,
`FvSolution::read`) and field output (`io::output`) are **not implemented** —
every one is `todo!()`. Cases are built by constructing the structs in Rust.

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

# GeN-Foam (reactor multiphysics — the `genfoam` port)

This crate is also the in-workspace home for the Rust port of **GeN-Foam**
(Generalized Nuclear Foam), an OpenFOAM-based reactor-multiphysics solver
(neutronics + thermal-hydraulics + thermo-mechanics). The port lives under
`src/genfoam/`; the upstream C++ is used reference-only.

- **Project:** GeN-Foam
- **Repository:** <https://gitlab.com/foam-for-nuclear/GeN-Foam>
- **Commit at last sync:** `652b3da`
- **License:** GPL-3.0
- **Date accessed:** 2026-07-15
- **Original copyright:** (C) 2015–2022 EPFL (École polytechnique fédérale de
  Lausanne); principal authors incl. Carlo Fiorina, Stefan Radman,
  Thomas Guilbaud. Built on OpenFOAM v2506.
- **Clone command:**
  `git clone https://gitlab.com/foam-for-nuclear/GeN-Foam.git upstream_source/GeN-Foam`
  then `git -C upstream_source/GeN-Foam checkout 652b3da`

GeN-Foam is GPL-3.0, matching this crate. The port is an **independent
translation**, not an EPFL / OpenFOAM Foundation / ESI release, and is not
endorsed by or affiliated with any of them. GeN-Foam and its tutorials are
open-source / public literature and are permitted as reference and benchmark
input under the workspace data policy. Every Rust file ported from GeN-Foam
keeps an attribution header naming the upstream project, source path, commit
`652b3da`, and GPL-3.0. Module map + translation order:
`docs/genfoam-port-plan.md`.

---

If a clone is added here, it is expected to be **gitignored** — never
committed, present for development only.
