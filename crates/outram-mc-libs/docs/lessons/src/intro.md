# How to read this deep dive

> **This site always tracks the `develop` branch, never `main`.** It is rebuilt
> on every push to `develop`, and every code link points at the exact `develop`
> commit it was built from,
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@).
> A released version on `main` may differ from what you read here.

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response. See
> [`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md).

> **Review status: first draft, 2026-10-03, AI-assisted, not yet reviewed by a
> human.** Treat every claim as checkable, not as settled. Each one links to the
> exact lines it is about.

This is the **extended** track for `outram-mc-libs`, the Monte Carlo
neutron-transport crate of OUTRAM PARK. It is written for someone who already
knows roughly what Monte Carlo transport is and wants to know **how this code is
built, why, and where it went wrong**. The concept lessons for students (what a
cross section is, why `1/√N`) will live on the separate teaching site.

## What the crate is

A pure-Rust port of selected [OpenMC](https://openmc.org) kernels: random
numbers, constructive solid geometry, particle tracking, collision physics,
k-eigenvalue and fixed-source drivers, tallies, variance reduction and a
multigroup mode, plus some work OpenMC does not have (delta tracking for pebble
beds). It holds no nuclear data of its own; cross sections come from
`njoy-outram-park-fork`.

It is a **derivative work** of OpenMC (MIT licence, copyright the OpenMC
development team, MIT and Argonne National Laboratory), distributed under
GPL-3.0-only. It is not the official OpenMC and is not affiliated with or
endorsed by MIT or Argonne.

## Live demo

**[Launch the TRISO pebble demo](../../demos/monte-carlo/?rung=triso&mode=watch)**: one neutron at a
time through a TRISO pebble, on real ENDF/B-VIII.0 data processed in your
browser by this workspace's NJOY port and tracked by this crate. It is a
teaching picture, not a benchmark (tracking issue
[#519](https://github.com/theodoreOnzGit/outram-park-backend/issues/519)).

## How each chapter is built

Every chapter starts from a **problem**, shows the **code that answers it**, and
ends with **what was measured** and where that measurement is recorded.

- **Code anchors.** Links of the form `transport_csg.rs#L1474-L1560` point at the
  exact commit this site was built from, so the lines you read are the lines
  that ran. Code shown inline is pulled from the real source file at build
  time, never copied by hand.
- **API links** go to the [rustdoc reference](../../api/outram_mc_libs/index.html)
  published next to this book.
- **Numbers** are quoted only as they are recorded in the repository, with the
  file and the date. A number recorded before a later change is labelled as
  such. Several are.

## When something doesn't tally

If a sentence here disagrees with the code it links to, **the page is wrong**
and that is a defect. Please
[open an issue](https://github.com/theodoreOnzGit/outram-park-backend/issues/new)
naming the chapter and the link. (A structured report form is coming,
[#511](https://github.com/theodoreOnzGit/outram-park-backend/issues/511).)

Tracking issue for this track:
[#514](https://github.com/theodoreOnzGit/outram-park-backend/issues/514).
