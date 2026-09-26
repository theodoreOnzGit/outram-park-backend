# REDHILL

**R**adionuclide **E**ffluent **D**ispersion solver for **H**ydrogeological
**I**nfiltration and **L**eaching through **L**ayers.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`. Repository assessment and long-term
> environmental transport are licensing-adjacent: nothing in this crate
> supports licensing or safety-critical decisions.

The reserved home for groundwater and geological transport of radionuclides:
subsurface migration, porous-media flow and long-term repository assessment.
It answers **"what happens after deposition and infiltration?"**, the far end
of the workspace's offsite chain:

```text
  SEMBAWANG  ──►  CHANGI  ──►  REDHILL
  what gets       what happens    what happens after
  released?       after release?  deposition + infiltration?
```

## Status: placeholder, nothing is implemented

Created 2026-09-18 to reserve the name and state the scope. The crate has no
dependencies and no behaviour. Its only public item is the `SCOPE` string
constant. Do not cite it as the location of any transport calculation.

The intended engine is `outram-park-fork-pflotran`, a pure-Rust PFLOTRAN fork
already in the workspace, so REDHILL is expected to be mostly an integration
layer over it. That dependency is **deliberately not declared yet**. It gets
added when code here calls into it. That fork is itself a scaffold
with no human V&V. `src/lib.rs` records why REDHILL depends on a separate
engine crate while CHANGI contains its FLEXPART port. The two crates date from
different eras. Neither shape is a mistake, so don't "harmonise" them.

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
