<!--
SPDX-License-Identifier: GPL-3.0-or-later
Copyright (C) 2026 Ong Kay Chen Theodore, National University of Singapore
Part of Outram Park (outram-park-backend).
-->

# HTGR Sim v1 — current threading and dataflow map

Audit deliverable for bead `op-morr`, taken 2026-09-06 on `develop` **before**
any execution change. Companion to `htgr-sim-execution.md`, which describes the
target.

This records **what the code does today**, with file:line, so the refactor can
be checked against it.

---

## 1. The loop, end to end

```
GUI thread (eframe)                    physics thread
-------------------                    --------------
HtgrSimApp::update
  reads  SharedState<HtgrSnapshot> ──┐
  renders                            │  spawn_physics_thread_monitored
  writes control fields ─────────────┤    loop {
                                     ├──►   read commands from the SAME struct
                                     │      plant.step(dt, commands) x N
                                     └───   write outputs back
                                            sleep to pace
                                          }
```

- Spawned at `app/mod.rs:350`, `spawn_physics_thread_monitored("htgr-physics", ..)`.
- Underlying primitive: `app_scaffold/mod.rs:149` `spawn_physics_thread`, which
  is `thread::spawn(move || loop { step(&state) })`.
- `plant: HtgrPlant` is **moved into the closure** and owned solely by it. Good:
  the plant itself is not shared, and nothing else can touch it.
- Shared state is `SharedState<T>(Arc<RwLock<T>>)` — `app_scaffold/mod.rs:74`.

## 2. The finding that matters: `HtgrSnapshot` is not a snapshot

It is documented as *"the only channel to the physics thread"*
(`app/panels.rs:35`, `app/mod.rs:262`), and `PlantCommands` as *"the only
channel into the plant model"*. Both statements are true as far as they go, and
together they give a misleading picture.

**`HtgrSnapshot` is one struct carrying traffic in both directions.**

| direction | fields | evidence |
|---|---|---|
| GUI → physics | `control_rod_insertion_fraction`, `helium_flow_setpoint_kg_per_s`, `feedwater_manual`, `feedwater_manual_flow_kg_per_s`, `feedwater_target_steam_temp_k`, `condenser_pressure_setpoint_kpa`, `rps_enabled`, `trip_reset_requested` | read back out by `plant_commands_from(s)` at `app/mod.rs:356` |
| physics → GUI | `reactor_power_mw`, `prompt_power_mw`, `delayed_power_mw`, `fuel_temperature_k`, `bed_temperature_k`, `trip_reason`, … (66 fields total) | written by `HtgrPlant::write_snapshot` (`physics/mod.rs:960`) |

So it is a **shared mutable blackboard**, not a one-way snapshot. The physics
thread performs read-modify-write on it every tick, including writing *control*
state back (`state.update(|s| s.trip_reset_requested = false)`,
`app/mod.rs:374`).

**Why this blocks the target architecture.** The target requires controls one
way, snapshots the other, and **no shared mutable state between them**. A single
`RwLock`-ed struct serving both directions cannot give atomic snapshot
publication: a GUI reader can observe a struct whose output fields are from
timestep N while a control field it just wrote is already visible. Today that is
harmless because the reader is a human; with worker-parallel reconciliation it
would not be.

**This is the one structural change the kernel extraction must make**, and it is
independent of parallelism — splitting it improves the native build on its own.

## 3. Platform-hostile calls on the physics path

| call | site | why it matters |
|---|---|---|
| `thread::spawn` | `app_scaffold/mod.rs:153, 218` | compiles for wasm, **panics at run time** |
| `Instant::now()` | `app/mod.rs:355` (tick start), `RealTimePacer`, `gui_frame_metrics` | same — panics on wasm; `op-eeqw.3` swaps for `web-time` |
| `std::fs` | `app_scaffold/csv_logging.rs` | returns errors on wasm |

The fast-forward branch (`app/mod.rs:390`) is wall-clock driven:
`while tick_start.elapsed() < PHYSICS_TICK`. **The physics loop's cadence is
therefore a function of the host clock**, which is precisely the coupling the
refactor removes.

## 4. What is already clean

Confirmed by construction rather than by reading: **the headless driver
(`headless.rs`) required no change to `physics/` at all.**

- `HtgrPlant::step` / `step_with_correctors` (`physics/mod.rs:757`, `:775`) take
  `dt` and `commands` explicitly, are deterministic, bounded, and own no thread.
- Per-subsystem `step` functions likewise: `kinetics.rs:328`,
  `primary_loop.rs:727` (+`step_hot_leg:754`), `secondary_loop.rs:971`,
  `turbine_generator.rs:380`.
- No RNG, no clock, no I/O inside `physics/`.

**That is the evidence that kernel extraction is an extraction, not a rewrite** —
which is what the architecture-first sequencing decision was betting on.

## 5. Timestep structure — measured, not assumed

`step_with_correctors` (`physics/mod.rs:775`) is **not** a plain feed-forward
chain:

- protection is evaluated **first and outside** the corrector loop, deliberately,
  on the *previous* step's signals, so a trip cannot be outrun within a timestep
- the cheap lumped subsystems are **rewound and re-advanced `PLANT_OUTER_CORRECTORS`
  times** against improving estimates
- the **steam generator is advanced exactly once**, on the final corrector, with
  converged boundary conditions
- the heat exchanger and turbine shaft sit **outside** the corrector loop

So the DAG has real structure worth mapping (bead `op-zxjc`), but note what it
implies for parallelism: **the expensive element (the SG, ~96% of compute) runs
once, last, alone, after the cheap ones have converged.** That ordering is
deliberate and physically motivated. It also means the obvious
"run subsystems concurrently" win is not available at this level — any
throughput gain has to come from *inside* the SG.

## 6. Reference baseline

`reference/baseline_default_commands.csv`, 3000 steps sampled every 25,
captured 2026-09-06. Asserted bit-exact by
`headless::tests::matches_the_recorded_reference_baseline`.

**It is a startup transient, not a steady state**, despite
`PlantCommands::default()`'s docstring: power reaches ~27.8 MW near 100 s
(~2.8x nominal) before settling near 8.1 MW, with bed temperature still drifting
at 1200 s. A transient exercises far more of the model than a hold, so this is a
better baseline than the documented behaviour would have been — but it must not
be described as steady state, and it is **not** an HTR-10 physics reference.

---

## Summary — what extraction must change

1. **Split the bidirectional blackboard** into one-way controls and one-way
   snapshots. The single structural change, and it stands on its own merits.
2. **Move the loop out of the thread**: a bounded `tick()` the platform drives,
   replacing `spawn_physics_thread`.
3. **Remove clock dependence from the physics path** — cadence is the platform
   layer's business.
4. **Leave `physics/` alone.** It already satisfies the kernel contract.
