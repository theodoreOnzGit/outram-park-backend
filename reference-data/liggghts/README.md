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
| Host | x86-64 Linux, Intel Xeon @ 2.80 GHz, **4 cores** (`nproc`, 2026-09-17) |

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

The small deterministic cases (`headon_*`, `oblique_*`, `wall_bounce`,
`rolling_pair`) all use monodisperse spheres, `d = 10 mm`, `ρ = 2500 kg/m³`,
`E = 10 MPa`, `ν = 0.3`. The three bulk cases do not — `pebble_bed` and
`repose_lift` use `d = 10 mm` at their own stiffness, and `htr10` uses the
HTR-10 design point (`d = 60 mm`, `ρ = 1730 kg/m³`, `E = 5e8 Pa`, `ν = 0.2`);
each input file states its own. Common to all: SI units, `fix nve/sphere`,
`pair_style gran`.

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
| `htr10_init.csv`, `htr10_settled.csv` | `in.htr10` | **HTR-10 full core**: 27 000 pebbles at the published reactor geometry (`D/d = 30`), Hertz + history + CDT rolling | `35 µs` | 60 000 insert + 50 000 settle | post-insertion + final |
| `htr10_settled_ours.csv` | — | **this port's own settled state**, not LIGGGHTS' — written by `tests/htr10_pebble_bed.rs` so the two beds can be diffed without re-running either code | — | — | final |
| `htr10_discharge.stl` | — | the HTR-10 **bottom conus + fuel discharge tube**: `R = 0.90 m` inlet, `36.946 cm` conus height, `0.25 m` tube radius, 480 facets, inward normals. Generated by `make_htr10_discharge_stl.sh` beside it, from the published dimensions in `docs/reactor-scoping/htr10-neutronics.md` | — | — | — |
| `htr10_recirculation.csv` | — | **this port's own output**, not LIGGGHTS': per-batch solid fraction during slow defuelling, written by `tests/htr10_recirculation.rs` | — | — | per batch |

**Two column layouts, by case type.** The deterministic trajectory cases
(`headon_*`, `oblique_*`, `wall_bounce`, `rolling_pair`) are one row per
sampled step, laid out wide: `step`, then per atom id in ascending order,
`x y z vx vy vz omegax omegay omegaz` (SI). The snapshot cases
(`pebble_bed_*`, `lift_*`, `htr10_*`) are one row per particle:
`id,x,y,z,vx,vy,vz` (SI, no spin — the settled beds are compared
statistically, not orientation by orientation). Header row names every column
in both.

The three bulk cases (`in.pebble_bed`, `in.repose_lift`, `in.htr10`) commit a **start and an
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

The dumps are converted to the two CSV layouts by the two scripts committed
beside the data:

```bash
./dump2traj.sh dump.headon_hertz  headon_hertz.csv       # wide trajectory layout
./dump2csv.sh  dump.htr10_settled htr10_settled.csv      # per-particle snapshot layout
```

Both are a **pure reshape** — fields are copied exactly as LIGGGHTS printed
them, with no rounding and no reformatting (the precision patch above already
makes the dumps full `double`). `dump2traj.sh` additionally drops the force
columns some dumps carry, which are not compared.

~~The dumps are converted ... by the loader in `tests/liggghts_cross_code.rs`'
sibling generator~~ **CORRECTED 2026-09-17** — no such generator existed. The
conversion had been done ad hoc and was not committed anywhere, so the
"regenerates rather than being trusted" promise at the top of this file did not
actually hold for the CSV step. The two scripts above close it, and were
checked by regenerating **11 of the 12 committed CSVs and diffing: every one
byte-for-byte identical**. The one not re-checked is `htr10_init.csv`, whose
source dump had already been overwritten by a later run at a different
stiffness; it comes from `dump2csv.sh`, which is byte-verified on all five
other snapshot files.

## Data policy

LIGGGHTS-PUBLIC is open-source software under GPL-2-or-later, and these files
are **output we generated ourselves** by running it on synthetic inputs written
for this purpose. No third-party, proprietary, confidential, or operational
data is involved (workspace `DATA_POLICY.md`).
