# AGENTS.md — punggol

**PUNGGOL — Particle transport, UNcertainty, Geometry, Gamma transport,
Optimization Library.** Photons, electrons and positrons: where the gammas and
betas go and where they deposit their energy.

The workspace root `CLAUDE.md` binds here in full. This file adds only what is
specific to this crate.

## Status (2026-11-20)

Decay-photon source spectra (`src/decay_spectrum.rs`, gh:#1391) and the
first port of OpenMC's photon samplers (`src/openmc_photon/`, gh:#1683, not
differentially tested) are the code. The EGSnrc port (gh:#976) is **barely begun** (`src/egsnrc/`: one module
per upstream Mortran file; `ANNIH_AT_REST`, `ANNIH`, `COMPT`, `RELAX`, `PAIR`, `PHOTO`, `BHABHA`, `MOLLER`, `BREMS`, `PHOTON`, `SHOWER`, `ELECTR`, `mscat`, `sscat`, `spin_rejection`, `UPHI` (IENTRY 2 and 3), Russian
roulette and the stack and azimuth macros are a first port, not differentially tested, gh:#1680; the
rest are placeholders with no stub functions (`docs/egsnrc_port_remaining.md` lists what is ported and what each remaining item needs); RANMAR/RANLUX are in PETIR). ~~The OpenMC photon port (gh:#975) is not begun.~~ **CORRECTED 2026-11-20** — samplers, TTB and reaction sampling exist (`src/openmc_photon/`). There is no transport loop: do not describe
this crate as tracking particles through geometry.

## The particle split (maintainer, 2026-10-16 and 2026-10-20)

- **Neutrons stay in `outram-mc-libs`.** "outramMC will take care of
  neutrons, PUNGGOL takes care of gammas and other things."
- **PUNGGOL depends on `outram-mc-libs`, never the reverse.** If a change here
  seems to need an edit there, it is neutron-agnostic data access (as the
  chain codec re-export is) or it belongs elsewhere. Never make
  `outram-mc-libs` depend on this crate.
- Coupled neutron-photon transport: `outram-mc-libs` records where secondary
  photons are born in its collision loop; PUNGGOL transports them.
- Ions (fission fragments, alphas, recoil knock-ons) stay in `boon-lay`.
- **General numerics go to `petir`**: alias tables, RANMAR and RANLUX,
  interpolation. Do not grow a second copy here.

## Where each port goes: an `egsnrc` module in every crate it touches (maintainer, 2026-11-20)

Both PUNGGOL ports live in this crate: the OpenMC photon port (gh:#975) and
the EGSnrc port (gh:#976). **Code ported from EGSnrc always lives in a module
named `egsnrc`**, in whichever crate owns that kind of code, so a reader can
tell its upstream from the path:

| EGSnrc part | Goes to |
|---|---|
| Physics: the `HEN_HOUSE/src` core and PEGS4 | `punggol::egsnrc` (scaffolded) |
| egs++ geometry (`HEN_HOUSE/egs++/geometry`) | `outram_blender::egsnrc`, the workspace's geometry crate (see `crates/outram-blender/AGENTS.md`) |
| General numerics (RANMAR, RANLUX, alias table, interpolator) | `petir`. These are **already ported** and pinned against compiled EGSnrc, but they live at `petir::rng::{ranmar, ranlux, alias_table}` and `petir::interp_egs`. Give them the `egsnrc` label by adding a `petir::egsnrc` module that re-exports them, not by moving them: moving the files would edit possibly-reviewed code. Never port them a second time. |
| Any other part, "and such" | an `egsnrc` module in the crate that owns that kind of code |

- **Keep the OpenMC photon port out of `punggol::egsnrc`.** It goes in its
  own module named for OpenMC.
- **Ports are done in three phases** (gh:#1680-#1685):
  1. a Sonnet first port;
  2. differential testing against the compiled upstream;
  3. one Opus pass that reads only what the differential tests did not reach.

## Rules

- **Search before building.** The chain `<source>` codec is
  `outram_mc_libs::depletion::DepletionChainXml` and the univariate samplers are
  `outram_mc_libs::source::distribution`. Reuse them.
- **Porting rule.** Every routine is ported from the upstream (OpenMC under
  `crates/outram-mc-libs/upstream_source/OpenMC/`, EGSnrc from
  <https://github.com/nrc-cnrc/EGSnrc>), cites `file:line`, and keeps the file's
  attribution header block (upstream project, file, commit, copyright, licence).
  EGSnrc is AGPL-3.0 and is **never vendored or committed**; clone it into a
  scratch directory to read it.
- **Code-to-code to bit identity** where OpenMC is the oracle; round-off only
  below 1e-9 relative, with the cause explained (root `CLAUDE.md`). Never
  loosen a gate.
- **V&V records carry methodology and results**, and the OpenMC drivers that
  made the reference are committed under `verification_and_validation/<topic>/openmc_inputs/`.
- **`no_std` + `alloc`** without the default `std` feature. Use `core::` /
  `alloc::` paths and `crate::nostd_prelude::*` in `no_std` builds; transcendental
  functions through `petir::mathf::RealMath`.
- **Geometry is drawn.** If this crate ever builds or changes a geometry (a
  shielding model, a detector), it follows the geometry-drawing rule in
  `crates/outram-mc-libs/AGENTS.md`: render from the assembled geometry and show
  the pictures to the maintainer.
- **No macros** (workspace rule). Mortran macros are expanded by hand into
  functions, enums and plain `match` arms. No `dyn`, no `Box`, no lifetime
  parameters (`docs/claude-md/rust-design-rules.md`).
- **Every function written or changed is reached by a test.**
- **Not a licensing or dose tool.** Research, education and V&V only.

## Avoid writing macros (workspace rule, 2026-11-05)

Maintainer direction: macros make code harder to read and review. Do not
write new `macro_rules!` or proc macros in this crate; use functions,
generics, enums and plain `match` arms. Using std and dependency macros
(`vec!`, `format!`, derives) is fine. Full rule: root `CLAUDE.md`, "Avoid
writing macros".
