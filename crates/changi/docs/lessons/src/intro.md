# How to read this

> **This site always tracks the `develop` branch, never `main`.** It is rebuilt
> on every push to `develop`, and every code link points at the exact `develop`
> commit it was built from,
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@).
> A released version on `main` may differ from what you read here.

> **Research, education and V&V only.** Nothing in this deep dive, or in the
> code it describes, is for emergency planning, emergency response, dose
> assessment for real populations, Level 3 PSA, reactor operation or any
> licensing or safety decision. The crate states this limit as binding
> ([`changi/src/lib.rs`, lines 36–47](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/changi/src/lib.rs#L36-L47)),
> and an earlier naming draft that claimed emergency-response capability was
> corrected for exactly that reason.

> **Review status: AI-assisted first draft (2026-10-03), not yet reviewed by a
> human.** Every claim below was checked against the code at the commit this
> page was built from, and each one links to the lines it rests on. If a claim
> and its code disagree, the claim is wrong. Please
> [open an issue](https://github.com/theodoreOnzGit/outram-park-backend/issues/new).

## The problem

Something has been released into the air, say a few hundred terabecquerels of
krypton and iodine from a 30 m stack. The wind is blowing at 2 m/s. **How much
of it is in the air 1 km downwind, and how much has settled on the ground?**

This deep dive follows that question through the code that answers it in
OUTRAM PARK. It is the "extended" companion to the concept lessons: it covers
how the code is built and why, where each piece was ported from, what upstream
gets wrong, and what has and has not been checked.

## What you need

- The idea of a Gaussian (normal) distribution and its standard deviation.
- Enough Rust to read a function signature. Quantities cross the APIs as
  [`uom`](https://docs.rs/uom) types, so a `Length` is a length and you cannot
  pass metres where kilometres are meant.

## How the pages are built

- **Code is linked, not copied.** Each "lines a–b" link is a permalink to the
  exact commit this site was built from. Snippets are pulled from the real
  source files at build time.
- **API names** link to the rustdoc pages published beside this deep dive.
- **Numbers are quoted only where the repository records them**, with the file
  that records them. A V&V number with no source file is not quoted.

## The chapters

1. [Three crates, two ports, one chain](./architecture.md): which crate does
   what, and where each piece came from.
2. [The steady Gaussian plume](./plume.md): the textbook formula, as
   `buangkok` ports it from pyDOSEIA.
3. [One puff](./puff.md): Pasquill stability classes, the
   Pasquill–Gifford sigmas, and the Gaussian puff kernel.
4. [A train of puffs](./puff-train.md): time-varying release, a wind that
   turns, and two upstream defects the port fixes by default.
5. [From a unit release to becquerels](./activity.md): dilution factors,
   decay in transit, dry deposition, and the end-to-end example.
6. [What has been checked](./vv-and-limits.md): the code-to-code results,
   the independent puff-versus-plume check, and the limits that remain.
