# How these lessons work

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response. See
> [`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md).

> **This site always tracks the `develop` branch.** It is rebuilt on every
> push, and every code link points at the exact commit it was built from,
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@)
> (@@BUILD_DATE@@).

> **Review status: first draft, 2026-10-04, AI-assisted, not yet reviewed by a
> human.** Every claim links to the code or the record it rests on, so it can
> be checked.

These lessons teach Monte Carlo neutron transport **by asking the questions a
curious reader would ask**, in order, and answering each one with the real code
of OUTRAM PARK's Monte Carlo crate, `outram-mc-libs`. Each step has the same
shape:

1. a question;
2. the shortest answer;
3. the formula;
4. a small animation of exactly that idea (or the demo, at the right setting);
5. the **code walk**: how the program gets from its entry point down to the
   lines that do it, each hop linked to its exact lines;
6. the measured check, where there is one;
7. a **predict** prompt that sets up the next question.

## The ladder

Each lesson is one rung. A rung adds one piece of physics, ends on a check
that can fail, and the problem it leaves open motivates the next rung.

| Rung | System | What it adds | Page |
|---|---|---|---|
| 1 | Godiva, a bare sphere of uranium | free flights, collisions, the surface, leakage, fission generations, k; leakage and "all fission is fast" by counting | [Godiva](godiva.md) |
| 2 | uranium mixed into graphite | slowing down, lethargy, free gas and S(α,β), resonance escape, the four-factor formula, why the homogeneous mixture fails | [Uranium in graphite](ugraphite.md) |
| 3 | lumped uranium in graphite | spatial self-shielding, why lumps help | [Lumping](lumped.md) |
| 4 | LCT-008, a water-moderated rod lattice | hydrogen moderation, lattices | [LCT-008](lct008.md) |
| 5 | TRISO, then the HTR-10 pebble bed | double heterogeneity, delta tracking | planned; the TRISO pebble already runs in the [demo](../../demos/monte-carlo/?rung=triso&mode=watch) |

## The demo

There is **one Monte Carlo demo**, with a rung setting. Each lesson opens it
at its own rung, and the demo's *What's happening here?* link opens the
lesson. It runs entirely in your browser: real ENDF/B-VIII.0 nuclear data are
downloaded and processed on your machine by the workspace's own NJOY port,
and the neutrons are transported by `outram-mc-libs` compiled to WebAssembly.

## Numbers

Every number on these pages is quoted from a record in the repository, with
the file, the date and the commit. When several exist, the page says which
one counts and why. A number's history is shown in a dated box, never as the
current result. A comparison with another code is **verification**; a
comparison with a measured experiment is **validation**.

## When something doesn't tally

If a sentence here disagrees with the code it links to, **the page is wrong**,
and that is a defect.
[Report it](https://github.com/theodoreOnzGit/outram-park-backend/issues/new?title=Tutorial%20doesn%27t%20tally%3A%20&labels=bug)
naming the page and the link (a structured form is coming,
[#511](https://github.com/theodoreOnzGit/outram-park-backend/issues/511)).
Tracking issues: the ladder [#520](https://github.com/theodoreOnzGit/outram-park-backend/issues/520),
this rung [#521](https://github.com/theodoreOnzGit/outram-park-backend/issues/521).
