# Upstream source

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


- **Project:** OpenMC
- **Repository:** <https://github.com/openmc-dev/openmc>
- **License:** MIT — **confirmed 2026-08-06** by reading the upstream `LICENSE`
  file directly (verbatim MIT text, `Copyright (c) 2011-2025 Massachusetts
  Institute of Technology, UChicago Argonne LLC, and OpenMC contributors`).
  Note GitHub's licence detector reports `NOASSERTION`/"Other" for the repo —
  a false alarm caused by the file having no "MIT License" title line, which
  defeats the strict matcher. MIT is GPLv3-compatible one-way; ported files
  must carry the MIT copyright + permission notice.
- **Branch tracked:** `develop` (when cloned — see below)
- **Commit at last sync:** `8f79d9d02483c90e4134f03a9df4b75ce6b062bf` (2026-11-10), a shallow clone in `upstream_source/OpenMC/` made for the surface/lattice differential test (gh:#1386; `verification_and_validation/upstream_openmc_surfaces/`), accessed 2026-11-10. ~~not currently tracked — no persistent local clone~~ **CORRECTED 2026-11-10**
- **Clone command:** `git clone --depth 1 https://github.com/openmc-dev/openmc.git upstream_source/OpenMC`

### History clone and a BUILT OpenMC (2026-11-11, gh:#1386, #1387)

- **`upstream_source/OpenMC-history/`**: `git clone --filter=blob:none --no-checkout
  https://github.com/openmc-dev/openmc.git` (a blob-less clone with full history, ~14 MB
  before blobs), used only to read the diffs of the 23 commits triaged in gh:#1387
  (`git show <sha>` fetches the blobs it needs). Gitignored. Judged against
  `8f79d9d02483c90e4134f03a9df4b75ce6b062bf`.
- **`upstream_source/OpenMC-build/`**: a built OpenMC `8f79d9d0` (`bin/openmc`, `lib/libopenmc.so`),
  gitignored. Recipe (`make -j1`, tens of minutes on one core, 18 MB of products, a few GB of build intermediates):
  `apt-get install -y libhdf5-dev libfmt-dev libpugixml-dev hdf5-tools`, then
  `cmake <OpenMC clone> -DCMAKE_BUILD_TYPE=Release -DOPENMC_USE_OPENMP=OFF
  -DOPENMC_ENABLE_STRICT_FP=ON -DOPENMC_BUILD_TESTS=OFF` and `make -j1 openmc`
  (strict FP = `-ffp-contract=off -fno-builtin`). OpenMC at this commit no longer vendors
  its dependencies as submodules, so the system `fmt` 9.1 and `pugixml` 1.14 are found.
  The Python package and a cross-section library are NOT here (no `openmc` Python, no
  `h5py`), which is what blocks a run: see
  `verification_and_validation/upstream_openmc_samplers/README.md` and the #1386 comment.

### Fork clone: `virtual_lattice`

- **Directory:** `upstream_source/OpenMC-virtual-lattice/` (gitignored via the
  crate `.gitignore` rule `/upstream_source/*/`)
- **Repository:** <https://github.com/liangjg/openmc> (fork of
  `openmc-dev/openmc`)
- **Branch:** `virtual_lattice`
- **Commit at sync:** `be04e2804f9dc563d53429d97368c5d905070978`
  (2025-10-27, *"Merge pull request #2 from cn-skywalker/virtual_lattice_0.15.2"*)
- **Date vendored:** 2026-08-06
- **License:** MIT, inherited from upstream OpenMC — the fork's `LICENSE` is
  the unmodified upstream file (verified against the clone).
- **Clone command:**
  `git clone --depth 1 --branch virtual_lattice --single-branch https://github.com/liangjg/openmc.git upstream_source/OpenMC-virtual-lattice`
- **Why:** the branch adds a *virtual lattice* — a uniform-grid ray-traversal
  accelerator for CSG cells packed with tens of thousands of explicit TRISO
  spheres. It is **not in upstream** `openmc-dev/openmc`: 14 commits ahead of
  `develop`, 17 files changed, contributed by Liang Jingang, Li Ruihan and
  `cn-skywalker`. Recommended to this project by Zhe Chuan.
- **Ported to:** `src/geometry/virtual_lattice/` — see that module's docs for
  the per-function reference map and the scope of what was and was not
  translated.

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
