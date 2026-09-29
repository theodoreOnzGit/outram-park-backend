# CLAUDE.md — bishan

**BISHAN — Building Internal Source-term and Hazard Analysis Network.** Level 2
PSA: in-plant severe-accident progression, containment and building response,
in-building aerosol transport and pool scrubbing, release categories.

The workspace root `CLAUDE.md` binds here in full. This file adds only what is
specific to this crate.

## Status: one component (2026-09-29)

~~Placeholder: nothing is implemented.~~ **CHANGED 2026-09-29 (gh:#400):** the
maintainer's source-term plan asked for a lumped reactor-building CV here, and
`src/building.rs` is it (HTR-10 vented confinement; published parameters; exact
stepping; atom-conservation tests). Nothing else in the scope is implemented.
Do not add further code without the maintainer asking for it.

**Deferred (gh:#409, 2026-09-29):** building work waits; `htgr_sim_v1` does not
credit the building by default (conservative) and keeps this CV off its
default path. Leave it unchanged until #409 is taken up.

The building CV is **in-building** physics and so does not touch the open
SEMBAWANG boundary question below, which concerns severe-accident progression.

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
