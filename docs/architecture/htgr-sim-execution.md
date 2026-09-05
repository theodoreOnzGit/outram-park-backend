<!--
SPDX-License-Identifier: GPL-3.0-or-later
Copyright (C) 2026 Ong Kay Chen Theodore, National University of Singapore
Part of Outram Park (outram-park-backend).
-->

# HTGR Sim v1 — execution architecture

Reference document for implementation agents working on the execution refactor
(bead epic `op-276r`, GH #148). Describes the native and browser execution
models, the timestep dependency DAG, state ownership, the worker protocol and
the invariants that must hold on both platforms.

> **Status: design, not description.** Sections marked **CURRENT** record what
> the code does today (audited 2026-09-06). Everything else is the target. Do
> not read this as an account of working software.

---

## 1. The principle

> **Physics kernels are deterministic, bounded, and single-threaded. Platform
> execution layers decide how independent kernels are scheduled. Native Rust
> owns native parallelism; browser JavaScript owns Web Worker parallelism;
> egui remains Rust and consumes coherent asynchronous snapshots.**

Rust owns all reactor and plant physics on every platform. What differs between
platforms is *only* who schedules the kernels.

```
                         Rust physics
                  deterministic component kernels
                             |
                 +-----------+-----------+
                 |                       |
              NATIVE                    WEB
                 |                       |
          Rust orchestrator        JS orchestrator
                 |                       |
       Rust threads / Rayon        Web Worker pool
                 |                       |
                 +-----------+-----------+
                             |
                      coherent plant state
                             |
                            egui
```

---

## 2. What already exists — CURRENT

The refactor is smaller than it looks. Three of its four pillars are partly
built.

### 2.1 Per-component kernels already exist

`examples/htgr_sim_v1/physics/` already exposes per-subsystem `step`
functions:

| kernel | location |
|---|---|
| kinetics | `kinetics.rs:328` |
| primary loop | `primary_loop.rs:727`, `step_hot_leg` at `:754` |
| steam generator | within `steam_generator.rs` (2226 lines) |
| secondary loop | `secondary_loop.rs:971` |
| turbine/generator | `turbine_generator.rs:380` |
| whole plant | `physics/mod.rs:757`, `step_with_correctors` at `:775` |

**The decomposition the target architecture needs is largely present.** The
work is to make these callable without a surrounding thread, not to invent
them.

### 2.2 The GUI boundary already exists

`app/panels.rs:35` and `app/mod.rs:262` document `HtgrSnapshot` as *"the only
channel to the physics thread"* and `PlantCommands` as *"the only channel into
the plant model"*. That is already the `PlantSnapshot` / `Controls` split the
target architecture requires.

### 2.3 The blocker — CURRENT

`src/app_scaffold/mod.rs:149`:

```rust
pub fn spawn_physics_thread<T, F>(state: SharedState<T>, mut step: F) -> JoinHandle<()>
{
    thread::spawn(move || loop {
        step(&state);
    })
}
```

Two problems, and they are the same problem seen from two sides:

1. **`thread::spawn` compiles on wasm32-unknown-unknown and then panics at run
   time.** `scripts/check-wasm.sh` documents exactly this trap.
2. **The `loop` lives inside the spawned thread**, so the physics owns its own
   cadence. A kernel that never returns cannot be scheduled by anyone else —
   not Rayon, not a Web Worker, not a test harness.

**This single function is the thing standing between the current code and the
target architecture.** It must be replaced by a bounded-work `tick()` that the
platform layer drives.

---

## 3. Kernel contract

Every physics kernel must satisfy all of:

- **deterministic** for equivalent inputs
- **single-threaded** at the kernel boundary
- **owns no OS thread**
- **contains no infinite or background loop**
- **bounded work per invocation** — returns after one timestep of work
- **platform-independent** — no `std::thread`, no `std::fs`, no
  `std::time::Instant` (use `web-time`), no I/O
- callable from native Rust **and** exportable to wasm
- **preserves existing physics behaviour** — this is an architectural refactor,
  not a physics change

**Do not change physical models or correlations to achieve this
architecture.**

---

## 4. Timestep dependency DAG

To be established by measurement, not assumption (bead: DAG extraction). The
expected shape, from the current call structure:

```
            controls / protection
                     |
              reactor kinetics
                     |
              primary helium loop
                     |
              steam generator          <-- ~96% of compute time
                     |
              secondary loop
                     |
             turbine / generator
                     |
              coherent PlantSnapshot
```

**This is close to a chain, which is the central risk to the whole
programme.** A strictly serial DAG parallelises across *nothing*, and the
worker pool would buy only responsiveness, not throughput.

Two consequences follow, and both must be settled before the worker pool is
built:

1. **The dependency map must be measured, not assumed.** `step_with_correctors`
   (`physics/mod.rs:775`) runs outer iterations, which suggests the coupling is
   not a pure feed-forward chain and that some subsystems may be evaluable
   concurrently within an outer iteration.
2. **The steam generator is where the parallelism has to come from.** It is
   documented as ~96% of compute. If the SG offers coarse-grained internal
   independence — axial nodes, tube groups, independent enthalpy marches —
   then the kernel boundary should be drawn so it can later be subdivided.
   **Do not subdivide it prematurely**, but do not draw a boundary that makes
   subdivision impossible.

**The worker pool is built regardless of what the DAG shows** — see §4.1. What
the DAG determines is *how much HTGR itself benefits*, not whether the runtime
exists.

### 4.1 The pool is unconditional — DECIDED (maintainer, 2026-09-06)

**Build the worker pool whether or not the HTGR DAG exercises it.**

The deliverable of this epic is a **reusable execution runtime**, not a faster
HTGR sim. HTGR Sim v1 is the dogfood case — the first consumer — but the
architecture is meant to carry other Outram Park simulators, and a steady-state
**refinery flowsheet is the obvious second consumer**: a sequential-modular
solve over a large complex has genuinely independent unit operations, and
recycle/tear convergence has independent work inside each sweep. That case has
the parallelism HTGR's near-chain may lack.

So the DAG bead's role changes. It is **no longer a go/no-go gate on the
pool**. It answers a narrower and still-useful question: *how much does HTGR
itself gain, and where?*

### The risk this creates, and how to contain it

**An abstraction validated only against a case that does not stress it is an
abstraction that has not been validated.** If HTGR's DAG really is a chain, the
pool's first genuine exercise would be a different simulator entirely — and
whatever is wrong with the design would surface there, late, in someone else's
work.

Two mitigations, both required:

1. **The pool must be exercised by a genuinely parallel case in its own test
   suite**, even if that case is synthetic — independent kernels with known
   results, dispatched concurrently, reconciled into one coherent step. This is
   cheap and it is the only thing that proves the machinery before a second
   real simulator arrives.
2. **Design against the refinery shape, not only the HTGR shape.** A flowsheet
   has many small independent units and a recycle loop; a plant timestep has
   few large dependent ones. A protocol tuned exclusively to the second will
   fit the first badly. Specifically: do not assume a fixed small kernel count,
   do not assume every kernel runs every step, and do not bake the HTGR
   subsystem list into the protocol types.

### Consequence for placement

Generalisation is now a **hard requirement, not an aspiration**. The runtime
interfaces — kernel trait, scheduler, snapshot publication, worker protocol
types — **belong in the `outram-park-digital-twin-engine` library**, not in
`examples/htgr_sim_v1/`. Nothing simulator-specific may leak into them.

`op-eeqw.5` (promote out of `examples/` into a library target) is no longer
merely convenient; it is on the critical path for this epic.

---

## 4.2 Three execution classes, not one — the kernel contract is not enough

**CURRENT §3 describes one class of work. There are three, and conflating them
will break the design.**

The HTGR core and turbine models are **simplified today and will get more
complex**. Planned alongside them: high-fidelity models running *outside* the
real-time loop to periodically **calibrate** it — a Monte Carlo (`outram-mc-libs`)
and LIGGGHTS/DEM pipeline, surfaced in a separate tab, user-triggerable — plus
**periodic burnup** calculations.

None of that fits the §3 kernel contract, which requires *bounded work per
invocation*. A criticality calculation or a DEM settling run is unbounded by
nature and may take minutes. Forcing it into a per-timestep kernel would stall
the plant loop; leaving it undesigned would have it invent its own threading and
reintroduce exactly the problem this epic removes.

### The three classes

| class | cadence | bounded? | carries state? | result feeds back as |
|---|---|---|---|---|
| **A. Real-time kernels** | every timestep | **yes** — frame budget | plant state | the next `PlantSnapshot` |
| **B. Periodic state-carrying jobs** — burnup | every N timesteps | roughly | **yes** — isotopic inventory evolves | updated cross sections / inventory |
| **C. On-demand calibration jobs** — MC, DEM | user-triggered or occasional | **no** — minutes | no (one-shot) | **parameters**, not state |

Class A is what §3 specifies. **B and C need their own contract.**

### What B and C require

- **They must not block a plant timestep.** Class A's frame budget is the
  invariant; a calibration run that stalls the loop is a defect regardless of
  how good its physics is.
- **They must be cancellable.** A user who triggers a manual calibration and
  changes their mind must not wait minutes, and a browser tab close must not
  leak a running worker.
- **Their results arrive asynchronously as a parameter update**, which is a
  *different message class from a snapshot*. A snapshot is the whole plant at
  one instant; a calibration result is a set of coefficients valid until
  superseded. Do not force one into the other's shape.
- **Applying a result must be explicit and visible.** The plant model changing
  underneath the operator because a background job finished is exactly the kind
  of silent reinterpretation the multi-thermo epic (#126) forbids elsewhere.
  Show what was applied, when, and from which run.
- **Class B carries state**, so it is not a pure function of the current
  timestep. Burnup inventory must be owned somewhere explicit and versioned —
  a worker holding it implicitly breaks determinism, exactly as §8 forbids.
- **They must not starve class A.** In the browser this is a scheduling
  requirement on the pool: a long MC run must not consume every worker. Reserve
  capacity, or use a separate pool.

### Consequence for the fidelity growth

Because the core and turbine models **will** grow, the kernel boundary must not
encode today's cost or today's granularity. Specifically: do not assume a kernel
is cheap because it is cheap now, do not assume the subsystem list is fixed, and
do not let the protocol name today's subsystems (§8, and the generalisation
requirement in §4.1 says the same thing for a different reason).

This is the same constraint arriving from two directions — refinery
generalisation and HTGR fidelity growth — which is reason to take it seriously
rather than treat it as speculative.

---

## 5. State ownership

```
UI:      frame frame frame frame frame frame frame
                    ^                 ^
Physics: |--- step N ---|--- step N+1 ---|
                    v                 v
             coherent snapshot  coherent snapshot
```

- Physics owns the authoritative plant state. The GUI never mutates it.
- The GUI renders the **most recently published coherent snapshot** and keeps
  rendering it while the next is computed.
- **A partially reconciled timestep is never visible to egui.** Publication is
  atomic: either the whole timestep's results are visible or none are.
- Controls flow one way (GUI to runtime), snapshots the other. There is no
  shared mutable state between them.

The same semantics apply on both platforms. Native gets this benefit too:
GUI repaint rate decouples from physics timestep duration, which should improve
native smoothness independently of any browser work.

---

## 6. Native execution

Rust owns the scheduler.

```
single-threaded physics kernels
           |
     Rust scheduler          <- dependency-aware
           |
    persistent thread / Rayon pool
```

- Genuine multithreading is retained. This refactor must not degrade the native
  simulator to serve the web build.
- Prefer a **persistent** pool over spawning OS threads per timestep.
- Termux/Android native builds keep native multithreading where the platform
  supports it.

---

## 7. Browser execution

JavaScript owns the top-level scheduler. **Rust physics is unchanged.**

```
Browser main thread
        |
        +-- egui/eframe Wasm  (GUI only)
        |
        +-- JavaScript coordinator
                    |
          +---------+---------+
          v         v         v
       Worker 1  Worker 2  Worker 3
          |         |         |
        Wasm      Wasm      Wasm      <- separate single-threaded instances
          |         |         |
       selected single-threaded Rust physics kernels
```

JavaScript owns: worker lifecycle and pool, dispatch, dependency ordering,
async completion, reconciliation into a coherent timestep, and communication
with the main-thread GUI instance.

### Deliberate non-goals

- **Do not reproduce native Rayon threading inside one Wasm instance** as the
  primary browser architecture.
- **`SharedArrayBuffer` / shared-memory wasm threads are not a prerequisite.**
  Ordinary `postMessage` with transferables is the first implementation. This
  is what makes static hosting on GitHub Pages practical — shared memory needs
  COOP/COEP headers that Pages does not serve.
- **Browser execution must not be a permanently serial fallback.** The existing
  serial wasm work (`op-eeqw` Phase 3, `boon-lay`'s `wasm_par`) is a useful
  reference and debug path. **It does not satisfy this requirement.**

---

## 8. Worker protocol

To be specified in its own bead. Constraints it must satisfy:

- messages carry a **timestep id** so late results can be discarded rather than
  applied out of order
- payloads are transferable where practical; avoid copying large arrays
- a worker is **stateless between dispatches**, or its state is explicitly
  owned and versioned — a worker holding hidden state breaks determinism
- results are reconciled only when **every** kernel in a timestep has reported;
  partial reconciliation is never published
- the protocol must be expressible without `SharedArrayBuffer`

---

## 9. Invariants

Both platforms, all of the time:

1. No kernel owns a thread or loops indefinitely.
2. No partially reconciled timestep is visible to the GUI.
3. Controls are accepted immediately; the GUI never blocks on physics.
4. The same Rust physics source compiles for native and wasm. **The physics is
   not forked.** Platform differences are confined to threading, time and I/O.
5. Single-threaded execution reproduces the pre-refactor reference within the
   documented tolerance.
6. Parallel execution reproduces single-threaded execution within a documented
   floating-point tolerance. **Byte equality is not required where parallel
   reduction order legitimately differs — document the tolerance instead of
   demanding equality.**

---

## 10. Relationship to existing work

| epic | relationship |
|---|---|
| `op-eeqw` / GH #39 — wasm browser demo | **Phases 0-2 are prerequisites** (toolchain, feature-gating the OPC-UA/mDNS/fs stack, `web-time`). **Phase 3's serial fallback becomes a debug path, not the end state.** Phase 4 (promote out of `examples/`) is a shared prerequisite. Phases 5-9 (web shell, pacing, size, deploy) **overlap with the Pages work here and must be reconciled, not duplicated.** |
| `op-jyyp` — HTR-10 pebble-bed retarget | **Same files, different axis.** That epic rewrites the physics content; this one changes how physics is invoked. **Sequencing must be agreed** — see below. |
| `scripts/check-wasm.sh` | Its compiles-vs-runs distinction is preserved and extended. Passing that gate is necessary and **not** sufficient. |

### Sequencing — DECIDED: architecture first (maintainer, 2026-09-06)

`op-jyyp` states the primary-loop model is *"a REWRITE, not a retune"*, which
raised the question of whether to extract kernels from code about to be
rewritten.

**Decision: architecture first.** Establish the kernel contract against the
current prismatic physics, then write the HTR-10 pebble-bed physics into a
shape that already satisfies it.

**Why this is the right way round:** the kernel contract is cheap to establish
now (the `step` functions already exist and extraction should be close to
mechanical) and expensive to retrofit later onto a larger, newer body of code.
Writing the pebble-bed closures directly into the kernel shape costs the
physics work nothing, because a bounded single-threaded `step` is what those
closures would naturally be anyway.

**What this gates, precisely.** Only the five `op-jyyp` children that rewrite
`htgr_sim_v1`'s own physics modules now wait on kernel extraction:

| bead | module it rewrites |
|---|---|
| `op-jyyp.2` | Ergun/KTA packed-bed pressure drop (primary loop) |
| `op-jyyp.6` | graphite/moderator feedback channel (kinetics) |
| `op-jyyp.7` | reflector / core barrel / cavity cooling (primary loop) |
| `op-jyyp.8` | decay heat (kinetics) |
| `op-jyyp.9` | helical-coil once-through steam generator (SG kernel) |

The other thirteen `op-jyyp` children are **library-level work in tampines,
TUAS, boon-lay and the material databases**. They do not touch the simulator's
execution path and are **deliberately not gated**.

**The cost, stated honestly.** `op-jyyp.9` (the helical-coil SG) is
substantial physics work now sitting behind an architectural bead. Kernel
extraction must therefore be treated as **time-critical**, not as leisurely
groundwork — if it stalls, it stalls the SG rewrite. It should be scoped to be
close to mechanical, and if it turns out not to be, that is a signal to
re-examine this decision rather than to let the physics wait.

### A consequence for the reference baseline

The reference recorded before extraction is a baseline of the **current
prismatic model**. Its job is to prove that *extraction changed nothing* — and
that job ends the moment extraction is shown equivalent.

**It is not an HTR-10 reference and must never be cited as one.** Once
`op-jyyp` rewrites the physics, this baseline is intentionally obsolete. The
HTR-10 reference is `op-jyyp.11`'s business (PBMR-400 coupled benchmark, then
HTR-10 criticality and safety demonstration).

---

## 11. Repository placement

There is a documented intention that `htgr_sim_v1` and `fhr_sim_v2` are
application-level simulators belonging in the outer `outram-park` repo under
`simulators/`, with reusable physics and digital-twin infrastructure staying in
`outram-park-backend`.

**Do not perform that move as part of this refactor.** But the architecture
must not make it harder: reusable runtime interfaces (kernel traits, scheduler,
snapshot publication, worker protocol types) belong in
`outram-park-digital-twin-engine`'s **library**, not buried in the example.
`op-eeqw.5` (promote out of `examples/` into a library target) is the natural
vehicle.

---

## 12. Validation

Ordered, and each stage gates the next:

1. **Record the reference.** Identify deterministic cases and capture baseline
   outputs **before** any execution change.
2. **Single-threaded equivalence.** Extracted kernels driven serially reproduce
   the reference.
3. **Native parallel equivalence.** Within documented tolerance.
4. **Browser equivalence.** Same, in a real browser.

Browser CI must eventually cover: wasm compilation; module instantiation;
worker startup; kernel invocation inside a worker; message round trip;
**multiple workers running concurrently**; egui web startup; and a short HTGR
smoke test in a real browser.

**`cargo check --target wasm32-unknown-unknown` satisfies none of these.**
