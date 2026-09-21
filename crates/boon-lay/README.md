# boon-lay

**BOON LAY** — ***BO**mbardment of neutrons **O**n **N**uclides with
**L**agrangian transport **a**nd transmutation **Y**ields*.

Radionuclide behaviour in TRISO fuel particles for HTGRs and FHRs: fission,
decay, transmutation and diffusion, all happening at once during reactor
operation.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`.

## Two ways of looking at the same problem

Tracking fission products through a TRISO particle with an **Eulerian**
(control-volume) method means solving a burnup matrix. That matrix is large and
stiff, so it's expensive to compute, and it's hard to visualise. BOON LAY
started from the other side, and now carries both:

| Approach | Module | What it is |
|---|---|---|
| **Lagrangian** | `lagrangian_decay_simulator`, `lagrangian_transmutation_and_fission_simulator` | Tracks representative atoms. Each one walks its decay chain, samples competing decay / `(n,gamma)` / `(n,2n)` / fission clocks, and diffuses through the kernel and coating shells. No burnup matrix: the population emerges from the ensemble. Built for watching radionuclide transport in real time, for outreach and open research. |
| **Eulerian / continuum** | `triso_atops_fork` | A Rust fork of INL's **TRISO-ATOPS** (MIT, upstream commit `de374c8`). Closed-form Booth, breakthrough and attenuation release fractions; normal-operation and accident release; coolant activity and source terms for 84 nuclides. |

The two are complements, not rivals. The Lagrangian model shows *how* atoms
get out. The TRISO-ATOPS fork gives the release fractions that the offsite
chain consumes.

## Where it sits

`boon-lay` supplies the release physics for the offsite chain. `sembawang`
orchestrates it and hands the result to `changi`:

```text
boon-lay (TRISO release)  ->  sembawang (source term)  ->  changi (dispersion, deposition)
```

It depends on `outram-mc-libs` for random numbers. Decay data are ENDF/B-VIII.0,
via the `openmc-endf-8-depletion-lib-b` crate.

## Verification status

- **TRISO-ATOPS fork: verified code-to-code against upstream.** 5 699 cases
  across 32 function groups, all passing (2026-09-15, upstream `de374c8`). Every
  function agrees to between exact equality and 4.2e-10 relative, outside the
  ill-conditioned inputs analysed in the write-up. A second pass on 2026-09-21
  covered the accident path. Methodology, results and the two deliberate
  divergences from upstream are in
  [`docs/triso-atops-code-to-code.md`](docs/triso-atops-code-to-code.md).
- **Lagrangian simulator:** unit-tested. Monte Carlo half-lives are checked
  against ENDF/B-VIII.0, and the release fraction against the IAEA CRP-6
  Case 1a/1b analytical solution. There is no cross-code comparison yet.

Neither is **validated** against measured release data.

## Running

```bash
cargo test --release -p boon-lay --lib --tests
cargo run  --release -p boon-lay --example first_passage_realtime
cargo run  --release -p boon-lay --example triso_simulator
cargo run  --release -p boon-lay --example boon_lay_decay_simulator
```

The egui examples fall under the workspace headless-mode rule; see
`crates/outram-park-digital-twin-engine/CLAUDE.md`.

## Licence

GPL-3.0. The TRISO-ATOPS fork is derived from MIT-licensed code by Battelle
Energy Alliance (INL). Its terms are in [`LICENSE.triso-atops`](LICENSE.triso-atops)
and [`NOTICE.triso-atops`](NOTICE.triso-atops), and the derivation is recorded in
[`TRISO_ATOPS_DERIVATION.md`](TRISO_ATOPS_DERIVATION.md). MIT into GPL-3.0 is one-way.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
