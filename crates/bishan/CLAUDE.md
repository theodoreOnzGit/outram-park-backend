# CLAUDE.md — bishan

**BISHAN — Building Internal Source-term and Hazard Analysis Network.** Level 2
PSA: in-plant severe-accident progression, containment and building response,
in-building aerosol transport and pool scrubbing, release categories.

The workspace root `CLAUDE.md` binds here in full. This file adds only what is
specific to this crate.

## Status: placeholder (2026-09-28)

Nothing is implemented. The crate exists to reserve the name and state the
scope. Do not describe it as providing anything, and do not add code here
without the maintainer asking for it.

## Rules for when work starts

- **Settle the boundary with SEMBAWANG first.** Both scopes name
  severe-accident progression (`docs/ecosystem-naming.md` vs the roadmap
  slides). Ask the maintainer which crate owns it before writing any of it.
- **Search before building.** Aerosol, deposition and filtration physics may
  already exist in `changi` (dry deposition), `boon-lay`/`sembawang` (source
  term, venting) and `outram-foam-*`. Read them before porting anything.
- **Port a mature code rather than write one.** The workspace default is to
  translate an established code with its tests (see `docs/melcor-scoping.md`
  for the MELCOR-class options already scoped for SEMBAWANG).
- **Dependencies are declared when code calls into them**, not before
  (RAFFLES for the probabilistic machinery, per the roadmap).
- **No dose.** BISHAN ends at what is released.
