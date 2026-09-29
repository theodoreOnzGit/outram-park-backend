# BISHAN

**B**uilding **I**nternal **S**ource-term and **H**azard **A**nalysis **N**etwork.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`. Level 2 PSA is licensing-adjacent:
> nothing in this crate supports licensing or safety-critical decisions.

The reserved home for **Level 2 PSA**: severe-accident progression inside the
plant, containment and reactor-building response, in-building aerosol
transport and pool scrubbing, and release categories. It answers **"what
leaves the building, how, and how often?"**, between the fuel release and the
offsite chain:

```text
  fuel release  ──►  BISHAN  ──►  CHANGI  ──►  REDHILL
  (boon-lay)         building,     dispersion,   ground
                     containment,  deposition    transport
                     release
                     categories
```

## Status: one component implemented

~~Placeholder, nothing is implemented.~~ Since 2026-09-29 (GitHub #400),
`bishan::building` is a lumped HTR-10 reactor-building control volume: inflow
from the primary circuit, decay, deposition and filtered exhaust to the stack,
from published figures (Jiang et al. 2002; Liu & Cao 2002), stepped exactly.
Everything else in the scope below is **not implemented**.

**Deferred (GitHub #409, maintainer 2026-09-29).** Reactor-building work waits
until next week or next month. `htgr_sim_v1` does **not** credit the building
by default (conservative: the circuit leak goes straight to the stack); this
CV is kept, unchanged, off that default path. Do not develop it further until
#409 is taken up.

The name and backronym come from the roadmap slides (`slides/outram-park.tex`,
2026-09-14), which also say BISHAN depends on RAFFLES for its probabilistic
machinery. That dependency is **deliberately not declared yet**; it is added
when code here calls into it.

**Open question:** SEMBAWANG's scope (`docs/ecosystem-naming.md`) also
includes severe-accident progression. Which crate owns in-plant progression
is not decided yet. See `src/lib.rs`.

**Not dose.** BISHAN ends at what is released.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## License

GPL-3.0. Part of the [OUTRAM PARK](../../README.md) workspace.
