# PASIR RIS

**P**robabilistic **A**ssessment of **S**afety **I**n **R**eactors:
**R**isk & Reliability **I**ntegrated **S**tudio.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`. Risk and reliability analysis is
> licensing-adjacent: nothing in this crate supports licensing or
> safety-critical decisions.

The reserved home for an integrated risk and reliability GUI. It is **mostly
a GUI**: an egui studio with a thin library under it, the physics and
probabilistic machinery staying in the crates below. It would sit at
the top of the safety column of the workspace, one studio over the crates that
already hold the pieces. It answers **"how do the safety pieces fit together,
on one screen?"**

```text
                              PASIR RIS (GUI)
    ┌──────────┬───────────┬──────┴─────┬─────────────┬───────────┐
  RAFFLES    BISHAN     SEMBAWANG     CHANGI  ──►  BUANGKOK     REDHILL
  fault      in-plant   source term,  dispersion   dose         ground
  trees, UQ  building   offsite chain                           transport
  ─ Level 1 ─  ──── Level 2 ────────  ──────────── Level 3 ─────────────
```

## Status: placeholder, nothing is implemented

Created 2026-10-06 to reserve the name and state the scope. The crate has no
behaviour. Its only public item is the `SCOPE` string constant.

By your direction the same day, it depends on every crate above, across all
three levels: it will drive their **lower-fidelity** models, and the
high-fidelity solvers stay in their own crates and studios. The edges are
declared before code calls into them, so kovan's code map shows where the
studio sits.

## Naming fence

"Probabilistic" in the backronym does not promote any of this work to
"Level 1/2/3 PSA". `docs/ecosystem-naming.md` records that this wording is a
separate, deliberate decision for the maintainer, and it has not been taken.

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
