# Upstream source

- **Project:** OpenMC
- **Repository:** <https://github.com/openmc-dev/openmc>
- **License:** MIT (verify against the upstream `LICENSE` file when a clone
  is made — not yet independently confirmed against a local checkout by this
  crate)
- **Branch tracked:** `develop` (when cloned — see below)
- **Commit at last sync:** not currently tracked — no persistent local clone (see note)
- **Clone command:** `git clone --depth 1 https://github.com/openmc-dev/openmc.git upstream_source/OpenMC`

## Provenance

`openmc-libs` is a pure-Rust port of OpenMC's Monte Carlo neutron-transport
kernels — CSG geometry, particle tracking, k-eigenvalue calculations, and
Woodcock (delta) tracking for doubly-heterogeneous media. It is **data-free**:
cross sections are pulled from `njoy-outram-park-fork` at the `XsProvider`
boundary, not read from OpenMC's own data format directly (see the
workspace-root `docs/architecture.md` for the neutronics dependency graph).

**Note:** this crate does not currently maintain a persistent local OpenMC
clone with an automated data-driven codegen pipeline — translation is done by
reading OpenMC's C++/Python source directly. Clone it with the command above
when doing line-by-line porting/verification work (e.g. against the
Godiva/Jezebel k-eff benchmarks tracked in beads).

## Licensing note

OpenMC is (per its public repository) MIT-licensed, compatible with this
crate's own GPL-3.0 (the OUTRAM PARK workspace default). This is an
**independent translation**, not an OpenMC development team release, and is
not endorsed by or affiliated with the OpenMC project or its maintainers.

---

If a clone is added here, it is expected to be **gitignored** — never
committed, present for development only.
