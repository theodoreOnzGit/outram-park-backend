# Architecture: three models in one crate

`boon-lay` holds three models that answer different parts of the question in
the last chapter. They have different origins, so they also have different
kinds of evidence behind them. Knowing which part of the crate you are in
tells you how far to trust what it says.

| Module | Answers | Where it comes from | Evidence it has |
|---|---|---|---|
| [`fuel_failure`](../../api/boon_lay/fuel_failure/index.html) | *Does the particle break?* | **New code**, written from the equations printed in the PANAMA-I report | reproduces the report's printed tables; partial match to its figures |
| [`triso_atops_fork`](../../api/boon_lay/triso_atops_fork/index.html) | *What leaves the fuel, and where does it go?* (continuum) | **Port** of INL's TRISO-ATOPS (Python, MIT, commit `de374c8`) | code to code against the upstream Python; analytical limits; one published workflow |
| [`lagrangian_decay_simulator`](../../api/boon_lay/lagrangian_decay_simulator/index.html) | *How* do atoms get out? (one atom at a time) | **New code**, this crate's original purpose | closed-form diffusion solutions (CRP-6 Case 1, interface equilibrium) |

The crate root says the same thing in its own words
([`src/lib.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/lib.rs#L1-L8)),
and the
[README](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/README.md)
gives the Lagrangian/Eulerian comparison.

## Lagrangian and Eulerian: two views of one equation

A fission product in the fuel obeys diffusion with decay and a source,

```text
∂C/∂t = D∇²C − λC + B
```

There are two ways to solve it.

- **Eulerian (continuum).** Solve for the concentration field `C`. For
  simple shapes this has closed-form solutions, and TRISO-ATOPS uses them
  (Booth, breakthrough, attenuation). This is cheap, and it gives the release
  fractions the offsite chain needs.
- **Lagrangian (particles).** Follow individual atoms, each doing a random
  walk and carrying its own decay clock. The concentration is what you see
  when you histogram many atoms. This is how the crate began. It shows
  *how* atoms get out, it needs no burnup matrix, and it can be animated.

They are complements. When both are applied to a case with an exact answer,
they should agree with each other and with that answer. The
[V&V chapter](./vv-status.md) shows that they do on CRP-6 Case 1.

## Where the crate sits in the offsite chain

```text
boon-lay (TRISO release)  ->  sembawang (source term)  ->  changi (dispersion, deposition)
```

`boon-lay` supplies the release physics.
[`sembawang`](https://github.com/theodoreOnzGit/outram-park-backend/tree/@@COMMIT@@/crates/sembawang)
orchestrates it into a source term, and
[`changi`](https://github.com/theodoreOnzGit/outram-park-backend/tree/@@COMMIT@@/crates/changi)
carries it downwind. The two failure-and-release models meet at one seam,
covered in [Into the source term](./source-term.md).

## Provenance, per module

- **`triso_atops_fork`** keeps the upstream MIT notice on every ported file
  ([header](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/mod.rs#L1-L13)),
  plus `LICENSE.triso-atops` and `NOTICE.triso-atops` at the crate root. MIT
  code can be combined into a GPL-3.0 work; the reverse is not possible. The
  Tkinter GUI (1432 lines) was deliberately not ported, because `boon-lay` is
  a headless library that must also build for Android. The full
  Python→Rust map is in
  [`docs/triso-atops-fork.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/docs/triso-atops-fork.md).
- **`fuel_failure`** is coded from the PANAMA-I report's equations. The report
  is restricted literature with no reuse licence, so only the equations and
  their constants appear in the code, with page citations, and no prose or
  figure is copied
  ([header](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/fuel_failure/mod.rs#L1-L20)).
- **The Lagrangian engine** is original. Its Walk-on-Spheres core replaced an
  earlier fixed-step Gaussian walk for a reason that is a good case study in
  itself; see [Getting out](./release.md).

## Decay data

Half-lives and decay modes come from ENDF/B-VIII.0 through the
`openmc-endf-8-depletion-lib-b` crate, parsed into a
[`DecayLibrary`](../../api/boon_lay/nuclide_reaction_and_decay_data/decay_library/struct.DecayLibrary.html).
TRISO-ATOPS carries its **own** 84-nuclide table with IAEA Live Chart
half-lives
([`nuclide_database.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/src/triso_atops_fork/nuclide_model/nuclide_database.rs)),
because the port reproduces upstream. So the two halves of the crate do not
share one decay-data source.

## The examples

Three `egui` desktop examples live in
[`examples/`](https://github.com/theodoreOnzGit/outram-park-backend/tree/@@COMMIT@@/crates/boon-lay/examples):

- `first_passage_realtime`: an ensemble of atoms diffusing out of a TRISO
  particle with the Walk-on-Spheres engine. A worker thread owns the ensemble
  and publishes snapshots through an `Arc<RwLock<…>>`. The compute backend can
  be switched between CPU single-thread, CPU multi-thread and a `wgpu` GPU
  kernel
  ([header](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/boon-lay/examples/first_passage_realtime.rs#L1-L15)).
- `triso_simulator` and `boon_lay_decay_simulator`: the original diffusion
  and decay demonstrators.

```bash
cargo run --release -p boon-lay --example first_passage_realtime
```

Two headless examples were added with the core lessons (2026-10-04, gh:#531):

- `triso_cell_slice` draws the assembled `TrisoCell` from region lookups and
  recovers every interface radius from the geometry ([rung 1](../../tutorials/triso-atops/triso.html)).
- `layer_diffusion_table` prints the `D` the random walk uses in each layer
  and the PyC→SiC transmission probability ([rung 4](../../tutorials/triso-atops/layers.html)).

Both are pure `std` and print their result; neither has a GUI.

> **Known gap.** The workspace requires every egui example to have a
> deterministic `--headless` mode with a regression test. At this commit none
> of the three has one (a search of `examples/` for `headless` finds
> nothing), so they are demonstrations, not checked artefacts.
