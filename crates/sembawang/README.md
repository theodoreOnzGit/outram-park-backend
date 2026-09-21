# sembawang

**SEMBAWANG** — *Severe-accident Evolution and Melt Behaviour Analysis Workbench
for Advanced Nuclear Geometries* — the source-term end of the offsite chain
(SEMBAWANG -> CHANGI -> REDHILL). "Workbench" because it is a toolkit, not a
single code.

## What exists, and what does not

**Exists:** the fission-product *release* path for TRISO fuel. A caller-prescribed
temperature transient and core inventory go through `boon-lay`'s TRISO-ATOPS fork
(diffusion, release fractions, SiC breakthrough) and come out as a
`changi::activity::source::SourceTerm`.

**Does not exist:** severe-accident *progression* — no melt, relocation, vessel
failure, molten-core-concrete interaction, hydrogen or aerosol physics. The
temperature transient is an **input**; nothing here computes it. Nor is the
core inventory computed (see `src/inventory.rs` for why `fission-yields-data`
must not be used to build one).

**Also exists (2026-09-21): the join to `changi`.** `sembawang::chain::pad_for_dispersion`
lines the release windows up with a `changi` dispersion run, so one run goes from
the prescribed transient to Bq·s/m³ in air and Bq/m² on the ground. No dose is
computed.

## Running

```bash
cargo run --release -p sembawang --example npmhtgr_release   # release only: Bq, Ci, fraction per nuclide
cargo run --release -p sembawang --example npmhtgr_chain     # release -> changi: Bq, Ci, Bq.s/m3, Bq/m2 vs distance
cargo test --release -p sembawang --lib --tests
```

`npmhtgr_chain` records its own time-step convergence study in its doc comment.
From 500 m out the results converge at a 10 s step. **The 100 m and 200 m rows
do not converge** and should not be quoted. `tests/chain_handoff.rs` checks the
join: padding conserves activity, the chain is linear in inventory, and noble
gases deposit exactly zero. These are consistency checks, not verification.

## Limits, stated up front

- Research, education and V&V only. Not for emergency planning or response,
  licensing, or any safety-critical decision.
- The orchestration has **no upstream** and no code-to-code verification. The
  release physics underneath is verified against upstream TRISO-ATOPS in
  `boon-lay`.
- Known upstream behaviours are returned as data (`Caveats`), not logged.
- The three accident-phase failure fractions are required arguments, not
  defaults: they are not in the cited reference case.
- Starting from empty normal-operation pools under-predicts the early release.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
