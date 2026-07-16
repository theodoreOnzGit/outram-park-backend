# OUTRAM PARK — workspace architecture

The target shape of the simulation stack and the responsibility boundaries
between crates. This is the map; individual crates hold their own `CLAUDE.md`
and `docs/`. Scaffold-level document — several crates below are planned, not yet
built (marked *planned*).

## Responsibility split

```
                         ┌───────────────────────────────────────────┐
                         │  nee-soon  (scaffold) — integration layer   │
                         │  human-readable coupling API; PRKE +        │
                         │  surrogates; exposes CFD-coupling interfaces│
                         └───────────────┬─────────────────────────────┘
             ┌───────────────────────────┼───────────────────────────┐
             ▼                           ▼                           ▼
   ┌──────────────────┐        ┌──────────────────┐        ┌──────────────────┐
   │   outram-mc-libs    │        │  deterministic + │        │   teh-o-prke     │
   │  Monte Carlo:    │        │  TH (GenFOAM)    │        │  point kinetics  │
   │  CSG geometry,   │        │  *planned*, first│        │  + surrogates    │
   │  tracking,       │        │  inside          │        └──────────────────┘
   │  delta-tracking, │        │  openfoam-       │
   │  double-hetero-  │        │  appbuilder-lib  │
   │  geneous media   │        └────────┬─────────┘
   └────────┬─────────┘                 │
            │  pulls cross sections      │  pulls cross sections
            └───────────────┬───────────┘
                            ▼
                 ┌────────────────────────────────┐
                 │   njoy-outram-park-fork         │
                 │   ALL nuclear data:             │
                 │   ENDF processing (RECONR/…),   │
                 │   Faddeeva kernel, WMP eval,    │
                 │   lean-ACE + WMP data blobs,    │
                 │   ν̄/χ secondary data            │
                 └────────────────────────────────┘
```

### `njoy-outram-park-fork` — the nuclear-data crate (single source of truth)

**Everything nuclear-data-related lives here.** No other crate owns cross
sections. Responsibilities:

- ENDF processing (the NJOY2016 port): RECONR, BROADR, THERMR, ACER, … (see its
  `docs/porting-plan.md`).
- The **Faddeeva kernel** `w(z)` (`src/wmp.rs::faddeeva`) — pure Rust, no FFI.
- **Windowed-multipole** evaluation with analytic Doppler (`src/wmp.rs`).
- **Lean ACE** pointwise tables and **WMP data blobs**, embedded in-crate so end
  users download nothing (subject to keeping the footprint small).
- ν̄(E) / χ(E) secondary data (`src/nuclear_data/secondary.rs`).
- The consumer-facing **provider** surface (`src/nuclear_data`): `XsProvider`
  (enum: `Multipole` | `LeanAce`) → `MicroXs`. This is what transport crates call.

Licensing: GPL-3.0-only derivative of NJOY (BSD/LANL) — keep `LICENSE.njoy` +
`NOTICE`. WMP is **separate MIT CRPG** provenance — add `LICENSE-WMP` before
embedding WMP data; never mix the two attributions.

### `outram-mc-libs` — the Monte Carlo transport crate

Pure transport, **data-free**. Pulls cross sections from `njoy-outram-park-fork`
via `XsProvider`. Responsibilities:

- Constructive solid geometry (CSG) + particle tracking.
- Monte Carlo neutron transport, k-eigenvalue via the fission bank.
- **Delta (Woodcock) tracking** and other methods aimed at **doubly
  heterogeneous media** (TRISO / pebble-bed), where surface-tracking every grain
  is intractable.

It keeps no ENDF, HDF5, or WMP-parsing code. When wired, `Nuclide::xs_at_energy`
delegates to njoy's `XsProvider::micro(e, t)`.

### `nee-soon` — integration / coupling layer *(scaffold; the crate exists as `nee_soon`)*

The **human-readable, AI-free-usable** front door to the whole stack. A user
builds and runs simulations through `nee-soon` without needing to understand the
internals of the transport or data crates. Responsibilities:

- Compose the **Monte Carlo** (`outram-mc-libs`), **deterministic + TH** (GenFOAM,
  via `outram-foam-appbuilder-lib`), and **nuclear-data** (`njoy-outram-park-fork`)
  aspects of a simulation.
- Expose interfaces that make **coupling to CFD** straightforward.
- Include **point reactor kinetics** and other **surrogate models** — depends on
  `teh-o-prke`.

Naming (in progress) — `NEE SOON` (a Singapore locality, matching the OUTRAM
PARK / BOON LAY / TAMPINES / TEH-O theme). Backronym so far:

| Letter | Word |
|---|---|
| **N** | **N**eutron |
| **E** | (n**E**utron) |
| **E** | **E**nergy-dependent |
| **S** | **S**imulation |
| **O** | **O**pen-source |
| **O** | *O-?* (candidates: **O**perator-coupled / **O**bject-oriented) |
| **N** | *N-?* (candidates: **N**eutronics / **N**umerics) |

### `outram-foam-appbuilder-lib` — deterministic + TH host (GenFOAM) *(planned)*

GenFOAM (deterministic neutronics + thermal hydraulics, OpenFOAM-based) is to be
ported **inside `outram-foam-appbuilder-lib`** first, rather than as a standalone
crate. It also pulls cross sections / group constants from
`njoy-outram-park-fork`. On hold until the MC + nuclear-data path is further
along.

## Dependency edges (target)

```
nee-soon → { outram-mc-libs, njoy-outram-park-fork, teh-o-prke, outram-foam-appbuilder-lib }
outram-mc-libs → njoy-outram-park-fork          (cross sections)   [declared in root
                                                                  workspace deps;
                                                                  wiring deferred]
outram-foam-appbuilder-lib (GenFOAM) → njoy-outram-park-fork        [planned]
njoy-outram-park-fork → { thiserror, uom }                       (lean; no BLAS)
```

`njoy-outram-park-fork` deliberately stays lean so the crates that depend on it
for data do not inherit heavy build requirements. The WMP HDF5 reader
(`hdf5-pure`) is feature-gated so the embedded-blob path pulls no HDF5.

## Phasing

1. **Now** — Monte Carlo + nuclear data:
   - njoy: Faddeeva `w(z)`, then `WindowedMultipole::evaluate`.
   - Priority 2: U-238 (n,γ) Doppler (WMP vs njoy BROADR vs OpenMC `.h5`).
   - Priority 1: bare-sphere Keff (Godiva U-235, Jezebel/Flattop-23 U-233).
   - See `docs/keff-doppler-roadmap.md`.
2. **Then** — deterministic + TH: port GenFOAM inside `outram-foam-appbuilder-lib`.
3. **Then** — `nee-soon`: the coupling/interface layer over all of the above.
```
