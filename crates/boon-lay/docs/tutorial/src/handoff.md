# Rung 8 — Into the source term, and on to dispersion

> **Research, education and V&V only.** Nothing here is for reactor
> operation, licensing, safety-critical decisions or emergency response
> ([`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md)).
> No page in this track is a source term for any facility, and none is for
> emergency planning.
> **Review status:** AI-assisted draft, 2026-10-04, not yet human-reviewed.

**Core lesson, rung 8 of 8.** This is where `boon-lay` stops.

## Where the chain goes next

```text
boon-lay                          sembawang                       changi / buangkok
(particle failure, release)  ->   (source term: inventory,   ->   (dispersion, deposition,
 rungs 1-7                         failure classes, venting)       the Gaussian plume and puff)
```

- [`sembawang`](https://github.com/theodoreOnzGit/outram-park-backend/tree/@@COMMIT@@/crates/sembawang)
  is the orchestrator: it takes a core inventory, the failure fractions in
  TRISO-ATOPS's classes (rung 5, step 7), runs the release pieces of rung 7
  for every nuclide and node, and adds what upstream does not have (venting
  and flow-through options, each labelled as not upstream).
- [`changi`](https://github.com/theodoreOnzGit/outram-park-backend/tree/@@COMMIT@@/crates/changi)
  carries the release downwind. Its own deep dive is the
  [dispersion track](../dispersion/) (gh:#516).

## What crosses the seam, exactly

| From boon-lay | Type | Used for |
|---|---|---|
| `φ₁`, `φ₂` from an accident history | [`FailureProgress`](../../api/boon_lay/fuel_failure/history/struct.FailureProgress.html) | the `f_inc` (full failure) class, via `with_fuel_failure_incremental` |
| per-nuclide release fractions and activities | `f64` curies / `uom` activity | the release, per nuclide |
| the 84-nuclide TRISO-ATOPS table | [`supported_nuclides`](../../api/boon_lay/triso_atops_fork/nuclide_model/nuclide_database/fn.supported_nuclides.html) | which nuclides the release can carry at all |

What does **not** cross: anything from the random-walk engine (rungs 2–4).
The Lagrangian side is verified against exact solutions, but no source-term
path uses it yet.

## The gaps that travel downstream

A source term inherits every gap above it. From this track:

- **gh:#446** — the accident model releases nothing during an isothermal
  hold, so an air-ingress source term built on it is missing its transport
  term.
- **gh:#441 / #444** — no mechanistic oxidation attack; air oxidation of an
  exposed kernel is missing. `sembawang`'s air-ingress bound adds measured
  KORA failures by hand.
- **gh:#383** — the German fuel-qualification band for `f_hm` is stored and
  unchecked; release of noble gases and halogens is linear in it.
- **gh:#296** — what boon-lay fuel failure can and cannot say for HTR-10.
- **gh:#385** — the Booth truncation floor over-states some metal releases at
  normal-operation temperatures.

## Back to the demo

You can now explain a TRISO particle from the inside: the layers (rung 1),
the clock of each atom (rung 2), its walk (rungs 3–4), when the shell breaks
(rung 5), what air and steam add (rung 6), and how release reaches the
coolant (rung 7). The `triso_atops_web` demo, when built, will put those
rungs side by side; until then, the [TRISO pebble demo](../../demos/triso-pebble/)
shows the particles from the neutron's side.

**Next:** the [extended deep dives](../../deep-dives/triso-atops/architecture.html) for the parts of the
crate the ladder walked past, or the [dispersion track](../dispersion/).
