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

REDHILL holds **no implementation yet**. What it records for a reader is a
design decision and its reasons: why subsurface transport is expected to be
an integration layer over the workspace's PFLOTRAN port rather than a new
solver, and why that differs from how CHANGI holds its FLEXPART port
(`src/lib.rs`).

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | None yet. The intended engine, `outram-park-fork-pflotran`, carries the PFLOTRAN provenance. |
| Mathematical equations | None yet. |
| Source code | `src/lib.rs`: the `SCOPE` constant and the module doc recording the scope and the PFLOTRAN decision. |
| Interactive simulations | None yet. |
| Visualisations | None yet. |
| Verification and validation evidence | None yet. |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp. No entry in `docs/human-corrections-to-ai-work.md`. |

**Status:** a placeholder; there is nothing here to verify.

## Status: placeholder, nothing is implemented

Created 2026-09-18 to reserve the name and state the scope. The crate has no
dependencies and no behaviour. Its only public item is the `SCOPE` string
constant. Do not cite it as the location of any transport calculation.

The intended engine is `outram-park-fork-pflotran`, a pure-Rust PFLOTRAN fork
already in the workspace, so REDHILL is expected to be mostly an integration
layer over it. That dependency is **deliberately not declared yet**. It gets
added when code here calls into it. That fork is itself ~~a scaffold~~
**CORRECTED 2026-11-27** — an early v1 (its own README: RICHARDS solves,
verification only) — with no human V&V. `src/lib.rs` records why REDHILL depends on a separate
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

~~GPL-3.0~~ **AGPL-3.0-only** (relicensed 2026-10-20, gh:#973). Part of the [OUTRAM PARK](../../README.md) workspace.
