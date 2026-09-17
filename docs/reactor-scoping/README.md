# Reactor scoping — digital-twin simulator slate

Scoping documents for a set of reactor digital-twin simulators built on
`crates/outram-park-digital-twin-engine`, each with a **coupled secondary loop**
so the full OUTRAM PARK stack is exercised end to end.

> **Intended use.** Education, research, capability building, and V&V only.
> These are offline demonstrations with no connection to any operational system.
> See `RESPONSIBLE_USE.md`.

## Scope of these documents

Each reactor gets a document covering: why it was chosen, plant and secondary-loop
configuration, a **capability audit** partitioned into HAVE / SCAFFOLD / MISSING
against the actual code, open validation data with an access tier, a proposed
work breakdown, and open questions for the maintainer.

**Provenance.** The capability findings come from a six-agent codebase audit run
on 2026-08-06 against commit `ebbde1b`, with claims cited to `file:line` and, where
stated, tests actually executed.

**Validation sources are named at facility and programme level only.** No report
numbers, DOIs, access terms, or numeric benchmark values are asserted anywhere in
these documents — they must be confirmed against the real publications before
being cited in a V&V case, per `RESEARCH_INTEGRITY_AND_PROVENANCE.md`.

## The slate

| Reactor | Type | Coolant | Secondary | Document | Bead |
|---|---|---|---|---|---|
| **HTR-10** | Pebble-bed HTGR | Helium | Helical-coil steam generator | [htr10.md](htr10.md) | `op-wqk.9` (prismatic today) |
| **MSRE** | Molten salt, circulating fuel | Fueled salt | Coolant salt to air radiator | [msre.md](msre.md) | new |
| **iPWR** | Integral PWR SMR | Pressurised water | Once-through steam generator | [ipwr.md](ipwr.md) | `op-wqk.10` |
| **BWR** | Natural-circulation BWR | Boiling water | Direct cycle | [bwr.md](bwr.md) | `op-wqk.11` |
| **FHR** | Pebble-bed FHR | FLiBe | Salt intermediate loop, then Rankine | [fhr.md](fhr.md) | `op-wqk.8` |
| **EBR-II** | Sodium fast reactor | Liquid sodium | Intermediate sodium, then steam | [ebr2.md](ebr2.md) | new |

Between them these cover gas, fueled salt, coolant salt, pressurised water,
boiling water and liquid metal — six distinct coolant regimes and six distinct
secondary-loop topologies.

## Readiness at a glance

Ordered by how close each is to a credible twin.

| Reactor | Readiness | The one thing blocking it |
|---|---|---|
| **FHR** | **Most developed.** Coupled three-loop plant running; the only case with recorded quantitative validation against experiment | Widget migration is half done; the core is not actually a porous-media component |
| **HTR-10** | App shell and secondary reusable; core is a **rewrite** — the existing sim is prismatic, not pebble-bed | Packed-bed closures do not exist: friction is a `todo!()`, bed conductivity is zero code, no graphite properties |
| **MSRE** | Circulating-fuel physics exists and passes its tests | The spatial solver is ~1000x too slow for real time; a reduced circulating-fuel point-kinetics model must be written |
| **BWR** | Rich two-phase closures, plus a **committed benchmark case with data on disk** | Integration, not closures: no heated channel, no separator, no loop closure, no void reactivity in the kinetics path |
| **iPWR** | Steam tables are the workspace's strongest asset, tested at PWR pressures | Pressuriser and helical-coil steam generator do not exist at all; the thermal-hydraulic stack is pressure-blind |
| **EBR-II** | Sodium properties exist and are tested | The SFR expansion feedbacks that *are* EBR-II's famous behaviour are absent, and the pool has no model |

## Cross-cutting findings

These affect several reactors at once and are worth handling as shared work
rather than per-simulator.

### Assets that are better than expected

- **Three capable crates were not on my radar and are wired to nothing.**
  `crates/outram-park-fork-moltres` implements circulating-fuel precursor
  advection and passes 20 tests, with zero dependents. `crates/bedok` carries a
  three-dimensional nodal neutronics code with a **committed BWR benchmark case**.
  `crates/outram-park-fork-coolprop` holds tested **liquid sodium and NaK**
  properties that the thermal-hydraulics stack cannot reach.
- **`crates/outram-foam-multiphase` is not an empty framework.** Its five beads
  read "in progress", but it delivered 67 passing tests of real closures.
- **`examples/htgr_sim_v1/physics/secondary_loop.rs` is the reusable secondary.**
  A working closed Rankine cycle with real flashes and six passing tests. Roughly
  70% of what the BWR and iPWR secondaries need.

### Shared gaps

- **The `crates/tampines` component layer is hollow.** Turbine, condenser, heat
  exchanger, steam generator, pump, valve and cooling tower all return
  not-implemented. Engine widgets wrap those stubs, which is why several render as
  flat rectangles and why the turbine rotor does not spin in one mode. The
  algebra exists in `crates/outram-park-fork-dwsim-libs` — this is wiring.
- **Packed-bed closures are missing workspace-wide.** The friction correlation is
  a `todo!()` carrying seventeen "not putting in ergun equation yet" comments;
  effective bed conductivity is zero code; there are no graphite properties. HTR-10
  and FHR both need all three.
- **Steam-table calls panic rather than returning errors** at roughly forty sites,
  and the two-phase `(p,T)` path reaches a `todo!()` inside the dome. Any reactor
  whose transient overshoots will kill the physics thread. A bounds-checked façade
  would serve the whole slate.
- **No V&V case exists for the kinetics crate** — its verification directory holds
  only a README.

### Correctness defects found during the audit

Worth beads independent of this slate:

1. **Precursor decay constants are identical across nuclides.** The six-group
   constants return the same decay-constant array for all three fissioning
   nuclides and say so in a comment; the half-life set carries a comment doubting
   its provenance. Affects any effective delayed fraction.
2. **A TRISO release "verification" test verifies nothing.** It wraps its
   assertion against the published reference in a catch-unwind, discards the
   result, then pins the code's own output at a value outside that reference
   range. It passes while the model disagrees with its own stated reference.
3. **Decay heat is self-flagged as suspect** by its author's own comment, on unit
   grounds.
4. **A parallel-branch flow calculator carries three live `todo!()` calls** and a
   header stating it does not work.

### Bead record drift

- Three of four `op-wqk.9.*` children describe defects that have since been
  fixed — they read as scaffold when the code is implemented and tested.
- **`op-wqk.8` is *not* a close candidate** despite a comment saying so. Step one
  is done; step two, the widget migration, has not started.
- `op-p6p.7.12` is marked in progress and assigned, but zero lines of the
  two-phase driver are written.
- Genfoam closure module docs **under-report** what exists, which misleads audits
  in the opposite direction.

## Suggested ordering

1. **Finish the FHR widget migration.** It is the immediate unblock and every
   other simulator draws from the same library.
2. **Shared closures next** — packed-bed friction and conductivity, graphite
   properties, the bounds-checked steam-table façade. These serve multiple
   reactors.
3. **Then the cheapest new reactor.** MSRE needs one reduced kinetics model and
   four property functions; BWR needs integration of closures that already exist.
   Both are cheaper than iPWR or EBR-II, which each need a major component built
   from nothing.
4. **Validate against what is already on disk before chasing documents.** The
   committed BWR benchmark case, the CIET data, and the vendored upstream test
   cases are all reachable today.
