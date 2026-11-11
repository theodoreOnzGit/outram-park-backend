# OpenFOAM-dev 852527377c7e differential (gh:#1364, part of gh:#1336; gh:#1458/#1462, #1459, #1460)

Research, education and V&V only (see `RESPONSIBLE_USE.md`). Status: AI-produced
differential, untrusted until a human has reviewed it.

**What this is.** A partial build of OpenFOAM-dev (OpenFOAM Foundation,
`github.com/OpenFOAM/OpenFOAM-dev`) at commit
`852527377c7e87e33d51b19ca8974b5db60af28b` (2026-11-03), and the first runs of the
port's DEFAULT (dev) flavour against it. Until now the dev-line code paths of
`block_mesh.rs`, `block_edges.rs`, `block_projection.rs` and
`poly_dual_mesh_upstream.rs` were compared only with an OpenFOAM.com v2506 build.
No OpenFOAM source is committed here; the clone lives in
`/home/user/upstream-build/OpenFOAM-dev` (outside the repository).

## Results at a glance

- **polyDualMesh (gh:#1460): 12 of 12 case x variant duals are bit-identical**
  (points, faces, owner, neighbour, patches) to the dev binary, with the port's
  default `UpstreamOptions` (dev flavour). The v2506 flavour is *not* identical on
  the same data (annulus, warped, L-shape differ, worst 2.0 index-level and 1.1e-6
  on the warped hex), so the comparison discriminates.
- **blockMesh: topology (faces, owner, neighbour, patches) identical on all 12
  dicts; points bit-identical on 7 of 12** (cavity, `proj_cyl`, `proj_edges`,
  `proj_faces`, `proj_two`, `sweep_torus_major`, `sweep_torus_minor_wrong`); the
  other five differ by at most 1.8e-15 absolute. All five are attributed below to
  three round-off-level causes, two of them **port defects** (not fixed, by
  instruction).
- **Torus verdict (gh:#1462): the 1.2e-4 is upstream behaviour, not a port
  defect.** The dev `blockMesh` gives 1.156682493910e-4 off the torus for the dev
  tutorial's "exact" minor-sweep case; the port gives 1.1566824939102e-4.

## Build recipe (partial, dummy Pstream, no MPI)

Host: 4 cores shared with other agents (load average 5 to 10 during the build),
`WM_NCOMPPROCS=2`, gcc 13.3, Ubuntu 24.04.

1. `git init OpenFOAM-dev && git remote add origin https://github.com/OpenFOAM/OpenFOAM-dev.git &&
   git fetch --depth 1 --filter=blob:none origin 852527377c7e87e33d51b19ca8974b5db60af28b &&
   git checkout FETCH_HEAD` (1.3 s; the blobless fetch pulls blobs on checkout,
   282 MB).
2. `scripts/of-dev-env.sh` sources `etc/bashrc` with `WM_MPLIB=DUMMY
   WM_COMPILE_OPTION=Opt WM_LABEL_SIZE=32 WM_PRECISION_OPTION=DP` (dev has no
   `WM_MPLIB=dummy`; any value that does not match an MPI name selects
   `FOAM_MPI=dummy`). `scripts/with-of-dev.sh CMD...` runs `CMD` in it, beside the
   existing `with-of.sh` for v2506.
3. `scripts/build-of-dev.sh` (incremental, re-runnable): `wmake/src` tools,
   `Pstream/dummy`, `OSspecific/POSIX` (the `libo` target, linked into
   `libOpenFOAM.so`: `wmake libso` fails with `cannot find libOSspecific.o`),
   `OpenFOAM`, `fileFormats`, `surfMesh`, `triSurface`, `meshTools`,
   `dummyThirdParty` (scotch/metis stubs), `finiteVolume`, `tracking`,
   `lagrangian/basic`, `sampling`, `meshCheck`, `mesh/extrudeModel`,
   `polyTopoChange`, `mesh/blockMesh`, `generic/{genericPatches,genericFields,
   genericFvPatches,genericFvFields}`, then the applications `blockMesh`,
   `checkMesh` and `polyDualMesh`.
   - Dev's `blockMesh` needs `polyTopoChange`, which needs `finiteVolume`,
     `meshCheck`, `sampling`: the whole chain is unavoidable.
   - **`polyDualMesh` is built from a scratch copy** with
     `scripts/polyDualMesh.Make.options` (only `finiteVolume`,
     `genericFvFields`, `meshTools`, `polyTopoChange`): upstream's `Make/options`
     also links `decompositionMethods`, the scotch stubs and `snappyHexMesh`,
     none of which `polyDualMesh.C` / `meshDualiser.C` use. Source unchanged.
   - **`finiteVolume` was finished at `-O0`** (`scripts/build-fv-O0.sh` sets
     `c++OPT = -O0` in `wmake/rules/linux64Gcc/c++Opt` for that one library and
     restores it with `git checkout`): it is only linked, it is not on the
     numeric path of `blockMesh` or `polyDualMesh`. The first ~365 of its 1504
     files were built at `-O3`, the rest at `-O0`. Everything else is `-O3`.
4. **Time:** about 70 minutes wall clock on the loaded host (OpenFOAM core 9 min,
   `fileFormats` to `meshTools` 6 min, `finiteVolume` 43 min, the rest 10 min; a
   few minutes more were lost to two restarts). **Disk:** `platforms/` 788 MB,
   whole clone 1.1 GB (282 MB sources).
5. **`snappyHexMesh` was NOT built**: it needs `decompositionMethods`,
   `parallel` and more libraries, and the requested runs do not use it. This
   leaves the dev `snappyHexMesh` code paths (gh:#1465, #1470) still unverified
   against a dev binary.

## Runs

`writePrecision 17`, one `writePrecision` entry per `controlDict` (the case
`controlDict`s are written by the scripts; no second entry can win).

- `scripts/run-blockmesh.sh`: dev `blockMesh` on the 12 dicts of
  `tests/fixtures/` + `tests/blockMeshDict.cavity` (`FIX`, `CAV`, `DEV_WORK`
  environment variables).
- `scripts/mkcases-dev.sh` then `scripts/rundev.sh`: the four primal cases of
  `../poly_dual_mesh_upstream_gh1460/` (cavity from the dev
  `tutorials/legacy/incompressible/icoFoam/cavity/cavity`, annulus, warped hex,
  L-shape) with dev `blockMesh`, then dev `polyDualMesh 60` with no flag,
  `-splitAllFaces`, `-concaveMultiCells`. Dev **overwrites in place**: the dual is in
  `runs/<case>__<variant>/constant/polyMesh` (v2506 wrote a `0.005/` directory).
  The port-built primals are bit-identical to the dev-built ones on all four.
- `scripts/difftable-dev.sh dev|esi`: `compare_polymesh` over the 12 duals.
- Rust side from a fresh worktree, `CARGO_TARGET_DIR=/home/user/outram-park-backend/target
  CARGO_BUILD_JOBS=1 cargo build --release -p outram-foam-mesh --examples`.
- `scripts/dev_hashes.py` (FNV-1a of a polyMesh dir, the frozen references of the
  pin tests), `scripts/results_table.py`, `scripts/torus_residual.py`.

## Results: blockMesh, port default (dev) flavour against the dev binary

`results/blockMesh_compare_dev_flavour.txt` is the raw output. (`proj_*` and
`sweep_*` are the `tests/fixtures/` dicts; pitzDailySteady is read with `units [mm]`,
which dev understands.)

| case | points bit-identical / total | worst abs diff | topology identical? (faces / owner / neighbour / patches) |
|---|---|---|---|
| cavity | 882 / 882 | 0 | yes (1640/1640, 1640/1640, 760/760, equal) |
| T3A | 8276 / 54496 | 1.78e-15 | yes (107707/107707, 107707/107707, 53213/53213, equal) |
| pitzDailySteady | 1808 / 25012 | 2.64e-16 | yes (49180/49180, 49180/49180, 24170/24170, equal) |
| proj_cyl | 567 / 567 | 0 | yes (1312/1312, 1312/1312, 992/992, equal) |
| proj_edges | 7351 / 7351 | 0 | yes (21300/21300, 21300/21300, 20700/20700, equal) |
| proj_faces | 7351 / 7351 | 0 | yes (21300/21300, 21300/21300, 20700/20700, equal) |
| proj_two | 567 / 567 | 0 | yes (1312/1312, 1312/1312, 992/992, equal) |
| proj_vertex | 56 / 210 | 2.22e-16 | yes (434/434, 434/434, 286/286, equal) |
| sweep_sphere | 1289 / 1331 | 4.44e-16 | yes (3300/3300, 3300/3300, 2700/2700, equal) |
| sweep_torus_major | 637 / 637 | 0 | yes (1476/1476, 1476/1476, 1116/1116, equal) |
| sweep_torus_minor | 633 / 637 | 1.11e-16 | yes (1476/1476, 1476/1476, 1116/1116, equal) |
| sweep_torus_minor_wrong | 637 / 637 | 0 | yes (1476/1476, 1476/1476, 1116/1116, equal) |

The projection fixtures (`project` vertices / edges / faces, the dev face loop
and the dev multi-surface step) are therefore verified against a dev binary for
the first time; so are `sweep` and the dev units path.

### Torus verdict (gh:#1462)

| quantity (measured on the BINARY's `points`, python) | dev binary | port |
|---|---|---|
| sweep torus, minor edges, factor 1: boundary nodes off the torus (> 1e-9) | 270 | 270 |
| same: worst near-torus residual | 1.156682493910e-4 | 1.1566824939102e-4 |
| sweep torus, major edges, factor 0: off the torus / worst | 50 (the two end discs) / 1.8e-15 | 50 / 2.0e-15 |
| sweep torus, minor edges, factor 0 (wrong orientation): off the torus | 270 | 270 |
| sweep sphere block: worst distance from the sphere | 0.1389840342370774 | 0.1389840342370774 |

**The tutorial's "exact" is not what the dev binary produces for this dict
(6 x 6 x 12); #1462's 1.2e-4 is reproduced by upstream to 12 digits, so it is
not a port defect.** The earlier explanation (twist linear in the row angle,
`block_mesh_projection.md` P7b) stands as the mechanism; what changes is that it
is now confirmed against the binary. Pinned by
`dev_sweep_torus_is_not_exact_in_the_binary_either`.

### Attribution of the non-bit-identical points (throwaway experiment, reverted)

Three one-line changes to the port, applied only in a scratch edit and then
reverted (`git status` clean of `src/`), take **all 12 dicts to bit-identical**.
Each was added in turn and the counts re-measured:

| change (port source line) | effect |
|---|---|
| (a) `block_mesh.rs` `line_divide_weights`: `pow(expFact, int)` as integer exponentiation by repeated squaring, as dev's `Scalar.H` `IntegerPow` (the `pow(Scalar, int32_t)` overload) instead of the real `pow` | single graded blocks 66 / 504 -> 504 / 504 (ratio 2.5 x 0.37), `proj_vertex` 56 -> 210 / 210, T3A 8276 -> 54416, pitz 1808 -> 24804 |
| (b) `block_mesh.rs` `graded_positions_multi`: drop the `s[n] = 1.0` clamp (dev keeps `secStart + blockFrac*...` = 1.9999999999999998 on a 2-section edge) | pitz 24804 -> **25012 / 25012** |
| (c) `block_edges.rs` and `block_projection.rs`: platform `cos`, `sin`, `tan`, `acos`, `asin` instead of `petir`'s | T3A 54416 -> **54496 / 54496**, `sweep_torus_minor` 633 -> **637 / 637**, `sweep_sphere` 1289 -> **1331 / 1331** |
| (d) platform `pow` for the real exponent `1/(n-1)` instead of `petir`'s | no change (the two agree on these dicts) |

## Results: polyDualMesh (feature angle 60), default (dev) flavour

`results/polyDualMesh_compare_dev_flavour.txt` is the raw output.

| case | points bit-identical / total | worst abs diff | topology identical? (faces / owner / neighbour / patches) |
|---|---|---|---|
| cavity base | 1832 / 1832 | 0 | yes (3475/3475, 3475/3475, 2121/2121, equal) |
| cavity split | 4321 / 4321 | 0 | yes (6154/6154, 6154/6154, 4800/4800, equal) |
| cavity concave | 1832 / 1832 | 0 | yes (3475/3475, 3475/3475, 2121/2121, equal) |
| annulus base | 924 / 924 | 0 | yes (1670/1670, 1670/1670, 1072/1072, equal) |
| annulus split | 2288 / 2288 | 0 | yes (3478/3478, 3478/3478, 2880/2880, equal) |
| annulus concave | 924 / 924 | 0 | yes (1670/1670, 1670/1670, 1072/1072, equal) |
| warped base | 480 / 480 | 0 | yes (833/833, 833/833, 523/523, equal) |
| warped split | 1133 / 1133 | 0 | yes (1750/1750, 1750/1750, 1440/1440, equal) |
| warped concave | 480 / 480 | 0 | yes (833/833, 833/833, 523/523, equal) |
| lshape base | 522 / 522 | 0 | yes (830/830, 830/830, 466/466, equal) |
| lshape split | 1000 / 1000 | 0 | yes (1516/1516, 1516/1516, 1152/1152, equal) |
| lshape concave | 531 / 531 | 0 | yes (857/857, 857/857, 493/493, equal) |

The dev face-centre formula (projected-area weights) and the dev
`labelHashSet(n)` capacity ordering of the L-shape are therefore confirmed
against the dev binary. The port's v2506 flavour run on the same dev outputs
(`results/polyDualMesh_compare_esi_flavour_vs_dev_binary.txt`) is bit-identical
only on cavity (all variants) and L-shape split, and differs on annulus (670 / 924,
worst 8.9e-16), warped (242 / 480, worst 1.1e-6) and L-shape base / concave
(362 / 522, 371 / 531; the face loops differ from face 0): the pin is not
flavour-blind. Not compared here: the `.obj` feature sets (`cmpfeat.py` of
`../poly_dual_mesh_upstream_gh1460/` was not re-run against the dev dumps).

## Prediction for the fixes of defects 1 and 2 (written 2026-11-11, BEFORE any code change)

Basis: the attribution table above (changes (a), (b), (c) measured in sequence
by the throwaway experiment). Fix (a) = dev integer `pow` in `line_divide_weights`,
fix (b) = no `s[n] = 1.0` clamp in `graded_positions_multi`; petir trig stays.
Reading of the sources before the change: v2506 `lineDivide.C` is the same loop as
dev's (it too overwrites `divisions_[nDiv]` with the computed value, so it has no
clamp either; it differs only in `equal(expRatio, 1)` against `expRatio == 1.0`),
and v2506 `pow(Scalar, label)` is the real `pow`. So the v2506 flavour keeps real
`pow` and (per source) loses the clamp too; the v2506 pins are expected NOT to move.

| dict | predicted points after fixes 1+2 | reason |
|---|---|---|
| pitzDailySteady | **25012 / 25012 bit-identical** | (a)+(b) took it there before (c) was needed |
| proj_vertex | **210 / 210 bit-identical** | (a) alone did it |
| T3A | 54416 / 54496 (80 differ, petir trig) | (c) is what closes the last 80 |
| sweep_sphere | 1289 / 1331 (42 differ, petir trig) | (c) |
| sweep_torus_minor | 633 / 637 (4 differ, petir trig) | (c) |
| the 7 already bit-identical dicts | unchanged (bit-identical) | no graded multi-section edge |
| topology on all 12 | unchanged | |
| polyDualMesh 12 of 12 | unchanged | primals are unaffected |

Predicted worst absolute differences of the three trig dicts: unchanged
(T3A 1.78e-15, sweep_sphere 4.44e-16, sweep_torus_minor 1.11e-16).

## Port defects found (listed, NOT fixed, nothing filed, by instruction)

1. **`line_divide_weights`: integer `pow` (dev) vs real `pow`.** The port comment
   says "`pow(expFact, int)` in C++ is `pow(double, double)`, not repeated
   multiplication: use the real power, as upstream". That is true of ESI v2506;
   **in OpenFOAM-dev `pow(Scalar, int32_t/int64_t)` is `IntegerPow`** (repeated
   squaring, `src/OpenFOAM/primitives/Scalar/Scalar.H` l.395-466), and
   `lineDivide.C` calls it with a `label` exponent. Effect: up to ~1e-15 on every
   graded block (single block 66 / 504 identical). Dev flavour only; the
   v2506 numbers need the real `pow`, so a flavour switch is needed. The doc
   claim is contradicted by the dev code.
2. **`graded_positions_multi`: `s[n] = 1.0` clamp.** Dev keeps the computed last
   division of a multi-section edge (1.9999999999999998 for a 2-section edge
   with a unit-length factor-2 section); the port clamps it to 1. Effect: 208 of
   25012 pitzDailySteady points at 1 ulp (and any multi-section graded block).
3. **Not a defect, by design: `petir` trig against glibc** (`r_sin`, `r_cos`,
   `r_tan`, `r_acos`, `r_asin`) differ from the platform functions by 1 ulp on
   arc and sweep dicts (T3A 80 points, sweep torus minor 4, sweep sphere 42).
   Below the 1e-8 round-off allowance, cause now understood, flagged for the
   maintainer as the rule requires.

Doc claims corrected in the same change (struck through, dated 2026-11-11):
`block_mesh_projection.md` (four places: "no binary differential for `sweep`",
"not verified against any dev binary", "cannot be confirmed without a dev
`blockMesh`", "does not check the dev-only differences"),
`poly_dual_mesh_gh1460.md` ("dev flavour was not run against a dev binary"),
`block_mesh_topology_numbering.md` ("cause is not isolated"). The `src/` doc
comment of `line_divide_weights` (defect 1) is left as it is: this change must not touch port code (kovan review stamps on
that function were not checked).

## Pin tests

`tests/upstream_dev_852527377c7e.rs`, four tests (frozen FNV-1a, same convention
as `tests/block_mesh_projection.rs`): points hash where bit-identical (7 blockMesh
dicts, all 12 duals), topology hash on all 12 + 12 cases, the torus / sphere
residuals of the binary, and one test that the v2506 flavour is *not* equal to the
dev output. The dicts of the three non-cavity duals are in
`tests/fixtures/dual_*.dict`. The five non-bit-identical blockMesh cases (T3A, pitzDailySteady, `proj_vertex`,
`sweep_sphere`, `sweep_torus_minor`) carry the topology pin only (the points differences are the defects above; when they are
fixed, add the points hash from `dev_hashes.py`).

## Reference outputs

`reference/blockMesh/<case>/` and `reference/polyDualMesh/<case>__<variant>/`
hold the dev binary's `points`, `faces`, `owner`, `neighbour`, `boundary`,
gzipped (`gzip -9 -n`), for every case except T3A and pitzDailySteady (1.1 and
0.7 MB each: checksums only). `reference/primal/` holds the four dev-built primal
polyMeshes. `reference/SHA256SUMS-uncompressed.txt` has SHA-256 of every
uncompressed file, T3A and pitz included. `logs/` has the application logs.
Total about 1.6 MB.

## What remains

- `snappyHexMesh` (gh:#1465, #1470) is not built here; its dev paths are still
  checked only against v2506.
- Defects 1 and 2 above are for the maintainer to decide (a flavour switch for
  the two `block_mesh.rs` lines); once fixed, add the points hashes.
- The polyDualMesh feature-set `.obj` comparison and `checkMesh` (built, not run
  on the duals) were not repeated against the dev build.
- `blockMesh` `fastMerge`, `mergePatchPairs` and `#calc` dicts are not covered by
  these fixtures.
