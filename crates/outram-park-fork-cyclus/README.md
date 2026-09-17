# outram-park-fork-cyclus

An independent, `no_std` pure-Rust translation of **CYCLUS**, the agent-based
nuclear fuel-cycle simulator, and **CYCAMORE**, its library of fuel-cycle
facility agents.

> ⚠️ **Unverified until validated.** All code in this workspace is
> **unverified and untrusted** unless a specific verification & validation
> (V&V) case demonstrates otherwise. This crate is an **AI-assisted draft with
> no human V&V**. Not for nuclear facility operation, reactor control,
> safety-critical decision-making, or licensing decisions.

> **Not affiliated with the Cyclus project.** This is an independent fork, not
> endorsed by or supported by the Cyclus developers, CNERG, or the University
> of Wisconsin. See `NOTICE`.

> **Name is provisional.** The maintainer has deferred the naming decision;
> `outram-park-fork-cyclus` follows the workspace's `outram-park-fork-<project>`
> convention in the meantime.

## What a fuel-cycle simulation is, and what makes Cyclus different

A nuclear fuel cycle is a set of facilities — mines, conversion and enrichment
plants, fuel fabricators, reactors, reprocessing plants, repositories — that
pass material between one another over discrete time steps. Most fuel-cycle
codes hard-wire those flows: this mine feeds that enrichment plant, which feeds
that reactor.

Cyclus does not. Each time step it runs a **dynamic resource exchange**:

1. Every consumer posts a **request** for a commodity, with a preference for
   each potential supplier.
2. Every producer answers with **bids**, each carrying its own capacity
   constraints.
3. A solver picks a set of **trades** that respects every constraint and
   maximises preference-weighted flow.

The facilities are agents; the market between them is recomputed from scratch
every step. That exchange is the heart of the code, and the heart of this
translation — see the [`exchange`](src/exchange/) module.

## Why `no_std`, and why every number comes from PETIR

This crate is `#![no_std]`, depending only on `alloc` and on
[`petir`](../petir), the workspace's `no_std` numerics library. There is no
`std` anywhere, and no C dependency of any kind.

Two things follow, and both were the point:

- **It runs where a fuel-cycle code normally cannot.** Embedded in a
  controller, compiled to `wasm32` for a browser, or on Android — with no
  second implementation of anything to keep in sync.
- **Results are bit-identical across platforms.** PETIR's transcendental
  functions are one fixed pure-Rust implementation, where `std` dispatches to
  whatever libm the platform ships and those differ in the last ulp. For a
  code whose output is a mass balance someone will cite in a paper, run-to-run
  and machine-to-machine reproducibility is worth the constraint.

Concretely, PETIR supplies the natural logarithm behind the enrichment value
function

$$V(f) = (1 - 2f) \ln\left(\frac{1}{f} - 1\right)$$

the matrix algebra behind the decay solve

$$\frac{d\vec{N}}{dt} = \mathbf{A}\,\vec{N} \quad (\text{Bateman, in matrix form})$$

and the Kahan-compensated summation behind every composition total.

## Scope: what is NOT ported, and why

This crate is the **simulation kernel only**. The following upstream
subsystems are deliberately absent. Their absence is a design decision, not an
unfinished task:

| Upstream subsystem | Why it is not here |
|---|---|
| `hdf5_back`, `sqlite_back`, `recorder` | Output persistence, via C libraries. Cannot be `no_std`. |
| `xml_file_loader`, `xml_parser`, `infile_tree` | Input decks, via libxml2. Same reason. |
| `prog_solver`, `prog_translator`, `OsiCbcSolverInterface` | The mixed-integer LP exchange solver, via Coin-OR/Cbc — a C++ dependency an order of magnitude larger than this crate. The **greedy solver, which is upstream's default, is ported.** |
| `dynamic_module`, `discovery` | Loading agent archetypes from shared libraries at run time. Rust resolves the equivalent at compile time through an enum. |
| `pyhooks`, `pymodule`, `pyinfile` and the Cython layer | Python bindings. |
| `Decayer`'s bundled decay-chain data | Deprecated upstream in favour of PyNE's decay. More importantly, the workspace rule is that nuclear data lives in `njoy-outram-park-fork`, so decay here runs over a **caller-supplied** chain and this crate ships no nuclear data at all. |
| PyNE's atomic-mass table | Same rule. Conversion between atom and mass bases takes an explicit `AtomicMasses` argument. |

A downstream crate that wants persistence or XML input adds it on top. The
kernel does not need to know.

## How the C++ was translated

The workspace forbids trait objects, `Box<T>` and lifetime parameters. Cyclus
is built from all three, so the translation is not mechanical. The
substitutions are uniform enough to state once:

| Upstream C++ | Here |
|---|---|
| `virtual` dispatch over `Agent*` | an `AgentKind` enum, matched at each call |
| `boost::shared_ptr<ExchangeNode>` | `NodeId`, an index into an arena |
| `Resource::Ptr` | `Resource`, an owned enum |
| `std::map<Nuc, double>` | `CompMap`, a `BTreeMap` — ordered, so results reproduce |
| thrown exceptions | `Result<T>` |
| `boost::math::float_distance` | a hand-rolled `limits::float_distance` |
| a `shared_ptr<Converter>` functor | a `Converter` enum |

Enum dispatch is not a workaround here, it is a better fit: the set of
resource types and agent archetypes is closed, so exhaustiveness becomes a
compile error rather than a runtime string comparison.

Every ported file carries a `PROVENANCE` header block naming the upstream file
and commit, so any routine can be opened next to its source and read line for
line.

## Build and test

```bash
cargo build --release -p outram-park-fork-cyclus
cargo test  --release -p outram-park-fork-cyclus --lib --tests
```

The `no_std` claim is checked by building for targets that have no `std` to
fall back on:

```bash
rustup target add thumbv7em-none-eabihf wasm32-unknown-unknown
cargo build --release -p outram-park-fork-cyclus --target thumbv7em-none-eabihf
cargo build --release -p outram-park-fork-cyclus --target wasm32-unknown-unknown
```

## What is implemented

| Layer | State |
|---|---|
| Kernel: nuclides, compositions, materials, products, resources | complete |
| Resource exchange: graph, portfolios, constraints, translator, greedy solver + preconditioner | complete |
| Toolkit: material queries, enrichment, resource buffers, commodities, symbolic functions, geodesic positions, inventory tracking | complete |
| Decay: Uniform Taylor matrix exponential over a caller-supplied chain | complete — but see the fidelity note below |
| Simulation: clock, six-phase loop, agent hierarchy, recipes, build/decommission scheduling | complete |
| CYCAMORE archetypes: `Source`, `Sink` | complete |
| CYCAMORE archetypes: `Storage`, `Enrichment`, `Reactor`, `Separations`, `FuelFab`, `Mixer`, `DeployInst`, `ManagerInst`, `GrowthRegion` | **not yet ported** |

The unported archetypes are **absent rather than stubbed**. A stub that
silently trades nothing would let a simulation run and produce a plausible,
wrong answer; a missing enum variant is a compile error.

## Verification & validation status

**Nothing in this crate is validated.** What exists is verification against
closed-form results, and it is recorded in the doc comment of each test:

- Decay is checked against the analytic single-species exponential and the
  closed-form two-species Bateman solution.
- Enrichment is checked against the closed-form separative-work expression.
- The exchange solver is checked against hand-worked matching cases.
- Resource operations are checked for exact mass conservation.

**Decay is a known fidelity gap, not merely unvalidated.** This crate ports
upstream's `UniformTaylor` solver, but that solver is only reachable from
`Decayer`, which is deprecated upstream and never instantiated. Upstream's live
decay path is CRAM (`pyne_cram_expm_multiply14`). Decay results here will
therefore **not** reproduce a modern Cyclus run, and the Uniform Taylor
truncation error is one-signed, costing 0.095 % of the atom inventory per year
at upstream's default tolerance. Use the `_with_tol` variants where a mass
balance matters. Full detail and the reuse path — `outram-park-fork-onix`
already has CRAM — are in `docs/port-notes.md`.

There is **no comparison against a Cyclus run**, and that is the single most
valuable thing that could be added. Building upstream Cyclus and diffing a
simple simulation's trade schedule against this port is the natural next step,
and is the same cross-code approach `outram-park-fork-liggghts` and `petir`
took to earn their evidence.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## Licence

GPL-3.0-only. Derived from CYCLUS and CYCAMORE (both BSD-3-Clause) and from
PyNE's `nucname` (BSD-2-Clause). The relicensing is **one-way**. See `NOTICE`
and `upstream_source/README.md`.
