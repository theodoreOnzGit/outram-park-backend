# ICSBEP benchmark specifications

Committed OpenMC input files for the ICSBEP criticality benchmarks this crate
runs as **external** oracles — cases whose answer does not come from any deck
under test, because an ICSBEP *critical* configuration has benchmark
`k_eff = 1.0000` to within its evaluated experimental uncertainty by
construction.

| Directory | Benchmark | Character | Example |
|---|---|---|---|
| `leu-comp-therm-008/` | LEU-COMP-THERM-008 — B&W critical lattices | thermal, 2.459 w/o UO₂ rods in 1511 ppm borated water; U-238 is 97.5 % of the heavy metal | `examples/lct008_keff.rs` |

Godiva (HEU-MET-FAST-001), Jemima (IEU-MET-FAST-002) and HEU-SOL-THERM-009 are
run by `examples/godiva_keff_endf_local.rs`, `examples/jemima_keff.rs` and
`examples/hst009_keff.rs` from atom densities and radii written into those
files: each is four numbers and three concentric surfaces, small enough to read
in the source. LEU-COMP-THERM-008 is not — 22 distinct 15 × 15 pin lattices
inside a 7 × 7 core lattice — so its specification is **committed here and
parsed at run time** instead of transcribed. The model is then the sourced file,
and a transcription error cannot be introduced by the example.

## Provenance and licence

The files in `leu-comp-therm-008/` are taken verbatim from
[`mit-crpg/benchmarks`](https://github.com/mit-crpg/benchmarks), the OpenMC
developers' collection of ICSBEP models (`icsbep/leu-comp-therm-008/openmc/`),
retrieved 2026-09-11 through `raw.githubusercontent.com`.

That repository carries the **MIT licence, © 2011-2024 Paul Romano and other
contributors** — the notice as written in its own `LICENSE`, copied verbatim to
`leu-comp-therm-008/LICENSE`. MIT is GPL-3-compatible.

This satisfies `DATA_POLICY.md`: properly licensed public benchmark data, from a
public source, traceable to it.

## What is *not* here, and why it does not need to be

The evaluated `k_eff` and its uncertainty are **not** in that repository and the
ICSBEP handbook is not reachable from this environment. They are not needed. A
benchmark model reduced from a delayed-critical experiment is critical by
construction, so `1.0000` is the reference and the only open question is the
width of the band — 0.1–0.6 % for a lattice of this kind, and the examples use
the pessimistic `± 0.006`.

That is reasoning from what an ICSBEP benchmark *is*. It is not a licence to
recall the rest: **atom densities and geometry must come from the file**, never
from memory or a web summary, or the result looks like validation without being
it.
