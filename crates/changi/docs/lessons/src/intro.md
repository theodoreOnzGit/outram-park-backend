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

> **Review status: AI-assisted first draft (2026-10-03; the rung ladder added
> 2026-10-04), not yet reviewed by a human.** Every claim below was checked against the code at the commit this
> page was built from, and each one links to the lines it rests on. If a claim
> and its code disagree, the claim is wrong. Please
> [open an issue](https://github.com/theodoreOnzGit/outram-park-backend/issues/new).

## The problem

Something has been released into the air, say a few hundred terabecquerels of
krypton and iodine from a 30 m stack. The wind is blowing at 2 m/s. **How much
of it is in the air 1 km downwind, how much has settled on the ground, and
what dose would that imply?**

This book follows that question through the code that answers it in OUTRAM
PARK: `buangkok` (the steady Gaussian plume and the dose pathways, ported from
pyDOSEIA), `changi` (the Gaussian puff, decay in transit and deposition) and
`sembawang` (the source term that starts the chain).

## Two kinds of page

- **Core lessons: the rung ladder.** Seven rungs, each adding **one** piece
  of physics, each built as a chain of questions: the question, the shortest
  answer, the formula, an animation (from the demo, once it exists), the
  **code walk** from an entry point down to the lines that do it, the check
  that could have failed, and a prediction that sets up the next question.
  Read them in order.
- **Extended deep dives.** How the code is built and why: where each piece
  was ported from, what upstream gets wrong, the machinery under the
  pathways, and the full V&V record. Read them when a rung sends you there.

**The demo.** One browser app with a rung setting,
[the dispersion demo](../../demos/dispersion/), opens on each rung from its
page and links back with "What's happening here?". It computes everything in
a background worker from `buangkok` and `changi`, except the capstone, whose
recorded results it shows with their provenance.

The [coverage table](./coverage.md) maps every public module of the three
crates to the page that teaches it, its code walk and its V&V record; a module
with no lesson is a visible row, not a silent omission.

## Maturity, stated once

None of the three crates is declared mature, and in every one both
bookkeeping axes (V&V and interface, human-reviewed) are **unchecked**. All of
the checks in this book are **verification** (against upstream code, analytic
results or an independent sum). **Nothing here has been validated against a
measured tracer release or a measured dose.**

## What you need

- The idea of a Gaussian (normal) distribution and its standard deviation.
- Enough Rust to read a function signature. Quantities cross the APIs as
  [`uom`](https://docs.rs/uom) types, so a `Length` is a length and you cannot
  pass metres where kilometres are meant.

## How the pages are built

- **Code is linked, not copied.** Each "lines a–b" link is a permalink to the
  exact commit this site was built from. Snippets are pulled from the real
  source files at build time.
- **Code walks are generated** by `kovan-cli code-walk` (gh:#523) from
  rust-analyzer's call graph, inside `<!-- code-walk -->` blocks that
  `kovan-cli code-walk-check` regenerates and checks. A hop the tool could not
  resolve is marked, and a hop filled in by reading the source is labelled
  **filled by hand**. In this book every concept chain connected without a
  hand-filled hop. The `UNRESOLVED` lines under a walk are calls the tool
  declines to guess: calls to a local closure variable (`closure`; the
  closure's own body is followed as part of its enclosing function) and
  derived `Default::default` (`other`). None is needed to connect a chain.
- **API names** link to the rustdoc pages published beside this book.
- **Numbers are quoted from a record** (file, date, commit) or from a run
  made for this book (settings and timing stated). A V&V number with no
  source file is not quoted.

## The rungs

1. [A steady stack: the Gaussian plume](./rungs/01-plume.md)
2. [How wide? Stability classes and sigma curves](./rungs/02-sigmas.md)
3. [How high? Plume rise, ground reflection, building wake](./rungs/03-rise-wake.md)
4. [Starts, stops and turns: the Gaussian puff](./rungs/04-puffs.md)
5. [Decay in flight, and deposition](./rungs/05-deposition.md)
6. [From air and ground to dose](./rungs/06-dose.md)
7. [Capstone: HTR-10 air ingress, release to dose](./rungs/07-capstone.md)

Side path: [the Lagrangian random walk (coming)](./rungs/side-random-walk.md).
