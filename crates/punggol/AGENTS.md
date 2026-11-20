# AGENTS.md — punggol

**PUNGGOL — Particle transport, UNcertainty, Geometry, Gamma transport,
Optimization Library.** Photons, electrons and positrons: where the gammas and
betas go and where they deposit their energy.

The workspace root `CLAUDE.md` binds here in full. This file adds only what is
specific to this crate.

## Status (2026-11-20)

Decay-photon source spectra (`src/decay_spectrum.rs`, gh:#1391) are the only
code. The EGSnrc port (gh:#976) is **scaffolded, nothing implemented**
(`src/egsnrc/`: one placeholder module per upstream Mortran file, no stub
functions). The OpenMC photon port (gh:#975) is not begun. Do not describe
this crate as transporting anything.

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
