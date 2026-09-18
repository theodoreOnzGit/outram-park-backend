# Upstream source

> ⚠️ **Unverified until validated.** All code in this workspace is unverified
> and untrusted unless a specific V&V case demonstrates otherwise. Not for
> nuclear facility operation, reactor control, safety-critical, or licensing
> decisions.

This crate is an independent Rust translation of **two** upstream projects,
both from the same organisation and both under the same licence. The local
clones are **gitignored** (dev-only, never committed); re-clone with the
commands below if absent.

## CYCLUS — the simulation kernel

- **Project:** CYCLUS
- **Repository:** <https://github.com/cyclus/cyclus>
- **License:** BSD-3-Clause (**confirmed 2026-09-16** against the cloned
  `LICENSE.rst`, not against the documentation — the distinction matters, see
  the note below)
- **Copyright:** (c) 2010-2016, University of Wisconsin Computational Nuclear
  Engineering Research Group. All rights reserved.
- **Commit at last sync:** `d4faab7ce0566ccb8febfcf50915cdeedee59db0` —
  shallow (`--depth 1`) clone taken 2026-09-16 for the fuel-cycle port
  (epic `op-r830`).
- **Clone command:**
  `git clone --depth 1 https://github.com/cyclus/cyclus.git upstream_source/cyclus`

## CYCAMORE — the additional-modules agent library

- **Project:** CYCAMORE (CYClus Additional MOdules REpository)
- **Repository:** <https://github.com/cyclus/cycamore>
- **License:** BSD-3-Clause (**confirmed 2026-09-16** against the cloned
  licence file)
- **Copyright:** (c) 2010-2016, University of Wisconsin Computational Nuclear
  Engineering Research Group. All rights reserved.
- **Commit at last sync:** `fee8c80190e0b91dafccae6b1a3129dc544441e8` —
  shallow (`--depth 1`) clone taken 2026-09-16.
- **Clone command:**
  `git clone --depth 1 https://github.com/cyclus/cycamore.git upstream_source/cycamore`

## A third upstream, vendored inside Cyclus

Cyclus vendors **PyNE**'s `nucname` and `atomic_mass` machinery as
`src/pyne.{h,cc}`. PyNE is **BSD-2-Clause**, also GPLv3-compatible. Only the
`nucname` identifier arithmetic is translated here (see `src/nuclide.rs`); the
atomic-mass *data* is deliberately not, because this workspace keeps nuclear
data in `njoy-outram-park-fork`.

## Licensing note

Both upstreams are **BSD-3-Clause**, which is GPLv3-compatible. This crate is
distributed as **GPL-3.0-only** (the OUTRAM PARK workspace default). The
combination is permitted, and the flow is **ONE-WAY**: code written here
cannot be contributed back upstream under BSD-3-Clause without the copyright
holder's agreement.

Every ported file carries a `PROVENANCE` header block naming the specific
upstream file and this commit, so any routine can be opened next to its source
and read line for line. **Do not strip those headers during a refactor.**

## How the licence was verified

Per the workspace rule, the licence was read from the cloned repositories'
own licence files at the commits recorded above — not from the project
websites, not from a package index, and not from documentation. Both
`LICENSE.rst` files carry the full BSD-3-Clause text with the University of
Wisconsin CNERG copyright line. Unlike GSL (see `crates/petir/NOTICE` for why
per-file headers can disagree with a repository's `COPYING`), Cyclus's own
source files do not carry per-file licence headers, so the repository-level
licence file is the authority available.
