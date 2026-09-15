# Upstream source

> ⚠️ **Unverified until validated.** All code in this workspace is unverified
> and untrusted unless a specific V&V case demonstrates otherwise. Not for
> nuclear facility operation, reactor control, safety-critical, or licensing
> decisions.

- **Project:** RAVEN (Risk Analysis Virtual ENvironment)
- **Developer:** Idaho National Laboratory (INL)
- **Repository:** <https://github.com/idaholab/raven>
- **Homepage:** <https://raven.inl.gov>
- **License:** Apache-2.0 (**confirmed 2026-08-06** — `LICENSE.txt` fetched from
  `raw.githubusercontent.com` and preserved verbatim as the crate's
  `LICENSE-APACHE-RAVEN`; blob `9b5e4019df618fc47d429529c369f4903142669b`)
- **Commit referenced:** `01216937967c38ee287859270c035c8eca906dc6` (branch
  `devel`, committed 2026-07-14)
- **Latest release at time of access:** `RAVENv3.2` (2026-03-12)
- **Date accessed:** 2026-08-06
- **Clone command:**
  `git clone --depth 1 https://github.com/idaholab/raven.git upstream_source/raven`

## Provenance

Pure-Rust port/translation of RAVEN's uncertainty-quantification and
risk-analysis core, for the OUTRAM PARK suite. The local clone is
**gitignored** (dev-only, never committed); re-clone with the command above if
absent. This is an **independent translation**, not affiliated with or endorsed
by RAVEN, INL, Battelle Energy Alliance, LLC, or the U.S. Department of Energy.

Nothing has been ported yet — the crate is a scaffold.

## Where the ported material comes from

The modules RAFFLES scaffolds map onto these upstream paths (recorded here so
per-file attribution headers can name a real source file):

| RAFFLES module | Upstream RAVEN path |
|---|---|
| `distributions` | `ravenframework/Distributions.py`, `Distributions1D.py`, `DistributionsND.py` |
| `samplers` | `ravenframework/Samplers/` (`MonteCarlo.py`, `Stratified.py`, `Grid.py`, `Sobol.py`, `FactorialDesign.py`, …) |
| `sensitivity` | `ravenframework/Models/PostProcessors/`, `ravenframework/Metrics/` |
| `surrogate` | `ravenframework/SupervisedLearning/` |

## Licensing note

Upstream is **Apache-2.0**, which is **one-way** compatible with GPLv3: Apache
code may be taken into a GPLv3 work, but **not** the reverse. This crate is
distributed as **GPL-3.0-only** (the OUTRAM PARK workspace default), and its
code therefore **cannot flow back to RAVEN**. Ported files carry the upstream
provenance header block per the workspace provenance rule — see the crate
`NOTICE` and `CLAUDE.md`.

RAVEN also vendors third-party BSD code (AMSC — University of Utah; NGL —
Carlos D. Correa). Files derived from those parts need the **BSD** attribution,
not the Apache-2.0 one. See `NOTICE-RAVEN` for the full text.
