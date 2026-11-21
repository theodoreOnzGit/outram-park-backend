# Brief: PUNGGOL phase 1, the Sonnet first port (gh:#1680, #1683)

> Written 2026-11-20 for a Sonnet agent. It covers both PUNGGOL ports. They
> can run as two agents in parallel, one per track, because they touch
> different modules. Phase 2 (differential testing, gh:#1681 / #1684) and
> phase 3 (an Opus review pass, gh:#1682 / #1685) come later and are **not**
> part of this brief.

## Read these first; they bind you

- **Root `CLAUDE.md`.** Release builds only. Search before building. Every
  function you write is reached by a test. No macros. No `dyn`, no `Box`
  (except for a recursive structure), no lifetime parameters. Never tune or
  loosen anything.
- **`crates/punggol/AGENTS.md`.** The particle split, the placement table and
  this crate's rules.
- **`crates/outram-mc-libs/AGENTS.md`.** The `no_std` section, the particle
  split, the porting rule and the RNG section.
- **`crates/petir/AGENTS.md`.** Its EGSnrc and OpenMC ports already exist.
  Maths goes through `Real` (call it as UFCS, `Real::exp(x)`) or
  `RealMath`.
- **`docs/claude-md/rust-design-rules.md`** and
  **`docs/claude-md/accounting-and-historian.md`** (commit trailers).

## Goal

A **first port**, faithful to upstream, routine by routine. It must compile,
and every function must be reached by a test. Do **not** claim the port is
correct: that is phase 2's job. Every new module's doc says
**"Status: first port, not differentially tested (gh:#1680)"** (or #1683).

## Where code goes (maintainer, 2026-11-20)

| Upstream part | Module |
|---|---|
| EGSnrc physics (`HEN_HOUSE/src` core, PEGS4) | `punggol::egsnrc` (the placeholders are already there, one per file) |
| OpenMC photon transport | its own module in `punggol`, named for OpenMC (e.g. `punggol::openmc_photon`), **never** inside `egsnrc` |
| EGSnrc numerics: RANMAR, RANLUX, alias table, interpolator | **already ported** in `petir` (`petir::rng::{ranmar, ranlux, alias_table}`, `petir::interp_egs`). Reuse them, do not re-port. Add a `petir::egsnrc` module that re-exports them. |
| egs++ geometry | `outram_blender::egsnrc`. **Out of scope:** it waits on the port-or-adapter decision on #976. |
| Photon and electron data readers (ENDF, ACE, HDF5) | `njoy-outram-park-fork`, reusing `acer::photoatomic` |
| Neutron-side hooks (secondary-photon birth sites, `ParticleType`) | `outram-mc-libs`, minimal and neutron-agnostic. **`punggol` depends on `outram-mc-libs`, never the reverse.** |

## `no_std` + `alloc`, as far as possible (maintainer)

`punggol` builds `no_std` + `alloc` without its default `std` feature. Keep it
that way in every new module.

- Use `core::` and `alloc::` paths, and `crate::nostd_prelude::*` in `no_std`
  builds. Copy the gated import line from `src/decay_spectrum.rs`.
- Maps come from `hashbrown`, through a `crate::hash` module: never `std`'s,
  never a `BTreeMap` stand-in. `punggol` has no `crate::hash` yet. If you need
  a map, add one by copying `outram-mc-libs`'s `src/hash.rs` pattern.
- Do all maths through `petir::mathf::RealMath` (`r_exp`, `r_ln`, `r_sin`, …)
  or `petir::real::Real` called as UFCS. Never `x.exp()` on an `f64`: under
  `std` that silently calls the platform libm.
- **Only file and data *loading* is `#[cfg(feature = "std")]`**: reading PEGS4
  output, HDF5 or ACE files from disk, and printing. Code that *uses* loaded
  data takes it by value, so it stays available without `std`.
- Tests that read files are `std`-only. All others also build with
  `--no-default-features`.
- Check both builds:
  `cargo check --release -p punggol --lib --no-default-features --target x86_64-unknown-none -j 2`
  and the same for any other crate you touch.

## Track A, EGSnrc (gh:#1680)

- **Upstream:** <https://github.com/nrc-cnrc/EGSnrc> at
  `bd49a47aae3938f49f4100b8e018091861408b19`. Clone it into a scratch
  directory (`git clone --filter=blob:none --sparse`, then
  `sparse-checkout add HEN_HOUSE/src HEN_HOUSE/pegs4`). **Never commit EGSnrc
  source.**
- **Scope:** `egsnrc.mortran` with `egsnrc.macros` (the kernel),
  `egs_utilities`, `nrcaux`, `get_media_inputs`, `pegs4_routines`/`_macros`
  and `pegs4/pegs4.mortran`.
- **Deferred:** ~~the `EEMF_*` field macros~~ **CORRECTED 2026-11-21:** `EEMF_macros` is ported (magnetic field) and compared; `EEMF_macros_beamnrc` is not; `egs_parallel`, and
  `get_inputs`'s user-code input parsing beyond what the media need.
- **Order** (each step reaches the previous ones):
  1. PEGS4 data preparation;
  2. media setup;
  3. the photon samplers: Compton, photoelectric, pair, Rayleigh;
  4. the electron and positron samplers: Møller, Bhabha, bremsstrahlung,
     annihilation;
  5. the condensed-history step and multiple scattering;
  6. the transport loop.
- **Expand Mortran macros by hand** into functions, enums and plain `match`
  arms. The workspace has no-macros rules. A Mortran macro that only names a
  constant becomes a `const`.
- **Common blocks become owned structs, passed explicitly.** No global
  mutable state, and nothing `static mut`.
- **RNG:** use `petir::rng::ranmar` / `ranlux`. They are already
  bit-identical to EGSnrc. Fill the `ranmar_*`/`ranlux_*` placeholders in
  `punggol::egsnrc` with docs that point there, not with code.
- **Attribution header on each file:** EGSnrc, upstream file, commit, NRC
  copyright as the file states it, AGPL-3.0. Cite `file:line` in doc
  comments.

## Track B, OpenMC photon transport (gh:#1683)

- **Upstream:** vendored at `crates/outram-mc-libs/upstream_source/OpenMC/`.
  Record its commit; `8f79d9d0` is used elsewhere in this workspace.
- **Scope:** `src/photon.cpp`, `include/openmc/photon.h`,
  `src/bremsstrahlung.cpp`, and in `src/physics.cpp` the functions
  `sample_photon_reaction`, `sample_electron_reaction`,
  `sample_positron_reaction` and `process_charged_secondary`. Thick-target
  bremsstrahlung (TTB) is the default, as upstream (`settings.cpp:113`).
- **The full spec is the #975 body.** Read it, but take placement from this
  brief.
- **Reuse** `njoy_outram_park_fork::acer::photoatomic` (fluorescence,
  incoherent heating) and `photon_blocks`, `outram_mc_libs`'s
  `ParticleType`, and `punggol::decay_spectrum` for decay-photon sources.
  Check `njoy-outram-park-fork` for a Compton-profile or form-factor reader
  before writing one.
- **Data files:** `compton_profiles.h5`, `density_effect.h5` and `BREMX.DAT`
  have unverified provenance and licence (#975). Do not vendor them. Write
  readers, put them behind `std`, and test them on synthetic inputs.
- **Leave a seam** at `process_charged_secondary` where an electron would go
  to `punggol::egsnrc`. A seam is a named function or enum arm, not a stub
  pretending to work.
- **Correct physics is on by default:** photon production and transport are
  ON, with `without_*` ablations, pinned by a test like
  `outram-mc-libs/tests/correct_physics_is_default.rs`.
- **Attribution header on each file:** OpenMC, file, commit, MIT.

## Rules that bite here

- **Search the workspace before writing anything.** Grep for the domain noun
  (Compton, Klein-Nishina, bremsstrahlung, Moller, Bhabha, stopping power,
  form factor). List in your report what you checked and found.
- **Read upstream, do not recall it.** If you cannot read a routine's source,
  do not write it. Say so.
- **No kovan-reviewed function is edited.** A function is reviewed if its
  folder's `review.md` has a `target = "fn:…"` for it. Extend beside it
  instead.
- **Units:** raw `f64` (cm, eV) inside the transport loop, as
  `outram-mc-libs` does; `uom` at the public, non-hot API.
- **Every new file path is at most 170 characters.**
- **Keep the docs true:** update `crates/punggol/README.md`'s status table
  and `AGENTS.md` status. Correct any stale doc claim the code contradicts,
  with strike-through.

## Gates before you report

Run all in release mode with `-j 2`. If memory is tight, add
`ulimit -v 12582912`.

```bash
cargo check --release -p punggol -p outram-mc-libs -p njoy-outram-park-fork -p petir --lib --tests -j 2
cargo check --release -p punggol --lib --no-default-features --target x86_64-unknown-none -j 2
cargo check --release -p outram-mc-libs --lib --no-default-features --target x86_64-unknown-none -j 2   # if touched
cargo check --release -p njoy-outram-park-fork --lib --no-default-features --target x86_64-unknown-none -j 2   # if touched
touch crates/petir/tests/no_platform_libm.rs; cargo test --release -p petir --test no_platform_libm -j 2
cargo test --release -p punggol --lib --tests -j 2
cargo check --release -p punggol --all-targets --target aarch64-linux-android -j 2   # Android rule
scripts/check-wasm.sh   # if punggol is in its list; add it if not
```

If you touch `petir`, also run `scripts/check-std-shadowing.sh`.

## Report back

- Modules ported, by path and upstream file.
- What you searched for and found existing.
- What you deferred and why.
- The gates run and their results.
- Every branch or routine you could not test, which is phase 2's starting
  list.
- Commit hashes.

Commit on your branch. Do not push to `main`. Do not close issues.
