# LIGGGHTS-PUBLIC reference data

Cross-code reference trajectories for `crates/outram-park-fork-liggghts`,
produced by **building and running upstream LIGGGHTS-PUBLIC**, not by quoting
numbers from its documentation. This closes the evidence gap that crate's
`CLAUDE.md` recorded at its 2026-09-06 maturity declaration:

> There is no cross-code comparison against upstream LIGGGHTS and no
> experimental comparison in this repository.

Same pattern as `reference-data/gsl/` (the `petir` crate): the upstream tool is
compiled from source, run, and its output committed here so the comparison
**regenerates** rather than being trusted.

## Provenance

| | |
|---|---|
| Upstream project | LIGGGHTS-PUBLIC (DCS Computing GmbH / JKU Linz) |
| Source | `https://github.com/CFDEMproject/LIGGGHTS-PUBLIC` |
| Commit | `3d5c00f20519e6bb6eb6756f51f1ad36564e649d` ("Remove logos", 2024-06-07) |
| Internal version string | `LAMMPS_VERSION "23 Nov 2013"` (LIGGGHTS' LAMMPS base) |
| Licence | GNU GPL, **version 2 or later** — used here under the "or later" option as GPL-3.0 |
| Build | `make stubs` then `make serial` (g++ 15.2.1, `-O2 -fPIC`, MPI stubs, no VTK) |
| Date generated | 2026-09-15 |
| Host | x86-64 Linux, 8 cores |

LIGGGHTS® and CFDEM® are registered trademarks of DCS Computing GmbH. This
project is an independent fork and is not affiliated with or endorsed by DCS
Computing GmbH.

## Output-precision patch — READ THIS

Stock LIGGGHTS writes `dump custom` fields with `"%g "`, i.e. **6 significant
figures**. At that precision a comparison saturates at ~`1e-6` relative and
cannot distinguish "our port agrees with LIGGGHTS" from "our port agrees with
LIGGGHTS' printf". Two lines in `src/dump_custom.cpp` were therefore changed to
print full `double` precision:

```diff
-  format_default = new char[3*size_one+1];
+  format_default = new char[8*size_one+1];
   for (int i = 0; i < size_one; i++) {
     if (vtype[i] == INT) strcat(format_default,"%d ");
-    else if (vtype[i] == DOUBLE) strcat(format_default,"%g ");
+    else if (vtype[i] == DOUBLE) strcat(format_default,"%.17g ");
```

The second hunk is the change; the first only enlarges the format buffer so the
longer specifier fits (without it LIGGGHTS segfaults on the first dump — it was
sized for exactly `"%g "` per field).

**This affects only how many digits are printed.** It touches no force model, no
integrator, and no data path — every computed value is the same `double` it was
before. It is recorded here because a reference dataset whose generator was
modified must say so, and because reproducing these files requires the same
patch.

## Cases

All cases: monodisperse spheres, `d = 10 mm`, `ρ = 2500 kg/m³`, `E = 10 MPa`,
`ν = 0.3`, SI units, `fix nve/sphere`, `pair_style gran`.

| File | Input | Physics exercised | `dt` | Steps | Sample |
|---|---|---|---|---|---|
| `headon_hertz.csv` | `in.headon_hertz` | Hertz normal + damping, `e = 0.9` | `1 µs` | 2500 | every 10 |
| `headon_hooke.csv` | `in.headon_hooke` | Hooke normal, `v_char = 2 m/s` | `1 µs` | 2500 | every 10 |
| `oblique_hertz.csv` | `in.oblique_hertz` | tangential **history** spring, Coulomb slip `µ = 0.5`, contact torque / spin-up | `1 µs` | 2500 | every 10 |
| `wall_bounce.csv` | `in.wall_bounce` | primitive `zplane` wall contact + gravity, repeated bounces | `1 µs` | 400 000 | every 200 |
| `rolling_pair.csv` | `in.rolling_pair` | **CDT rolling resistance** (`µ_r = 0.1`), counter-spinning pair | `1 µs` | 2000 | every 10 |
| `oblique_nohist.csv` | `in.oblique_nohist` | `tangential **no_history**` — the model the stateless `contact`+`simulation` path implements | `1 µs` | 2500 | every 10 |
| `pebble_bed_init.csv`, `pebble_bed_settled.csv` | `in.pebble_bed` | bulk settling in a cylinder, 354 pebbles, packing fraction | `5 µs` | 400 000 | first + final state |
| `lift_init.csv`, `lift_heap.csv` | `in.repose_lift` | **angle of repose** by the lifting-cylinder method, 656 pebbles | `5 µs` | 1 700 000 | post-settle + final |
| `lift_cylinder.stl` | — | the cylinder geometry **both codes read**: `R = 0.050 m`, 1280 facets, inward normals | — | — | — |

Column layout of the CSVs: `step`, then per atom id in ascending order,
`x y z vx vy vz omegax omegay omegaz` (SI). Header row names every column.

The two bulk cases (`in.pebble_bed`, `in.repose_lift`) commit a **start and an
end state** rather than a trajectory: their initial condition comes from
LIGGGHTS' own random `fix insert/pack`, and reproducing that RNG stream is not
the point. The Rust side starts from the committed LIGGGHTS state so both codes
integrate the identical configuration, and the comparison is statistical (see
`crates/outram-park-fork-liggghts/docs/verification-and-validation.md`).

`in.repose_lift` is the one case that needs a **mesh** wall. A LIGGGHTS
*primitive* wall cannot move — `fix_wall_gran`'s `shear` imposes a tangential
surface velocity without translating the geometry — so the lifting cylinder is
a triangulated surface driven by `fix move/mesh`, following
`examples/LIGGGHTS/Tutorials_public/movingMeshGran`. `lift_cylinder.stl` is
read by **both** codes: LIGGGHTS via `fix mesh/surface file`, this crate via
`MeshWall::from_ascii_stl`.

## Regenerating

```bash
git clone https://github.com/CFDEMproject/LIGGGHTS-PUBLIC.git
cd LIGGGHTS-PUBLIC && git checkout 3d5c00f2
# apply the two-line precision patch above to src/dump_custom.cpp
cd src/STUBS && make && cd .. && make -j serial
./lmp_serial -in <path-to>/in.headon_hertz      # etc.
```

The dumps are converted to the CSV layout above by the loader in
`crates/outram-park-fork-liggghts/tests/liggghts_cross_code.rs`' sibling
generator; the conversion is a pure reshape (no rounding).

## Data policy

LIGGGHTS-PUBLIC is open-source software under GPL-2-or-later, and these files
are **output we generated ourselves** by running it on synthetic inputs written
for this purpose. No third-party, proprietary, confidential, or operational
data is involved (workspace `DATA_POLICY.md`).
