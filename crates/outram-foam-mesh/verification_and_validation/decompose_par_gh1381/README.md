# decomposePar differential against ESI v2506 (gh:#1381)

Research, education and V&V only (`RESPONSIBLE_USE.md`). AI-produced, untrusted until a
human has reviewed it. Date of runs: 2026-11-14, release, x86_64. Predictions, committed
before any run: [`PREDICTIONS.md`](PREDICTIONS.md).

## Result

**1098 of 1098 files byte-identical** (17 variants on 3 meshes, 4 to 12 processors each):
`points`, `faces`, `owner`, `neighbour`, `boundary` (with `processor` patches) and the four
`*ProcAddressing` files of every `processorN/constant/polyMesh`, headers and `note` strings
included. Predictions D1 to D7 all held.

| mesh | variants | files | identical |
|---|---|---|---|
| `cavity` (20x20x1, 2D, `empty` patches, 400 cells) | simple 2x2x1, 4x1x1, 3x2x1 delta 0; hierarchical 2x2x1 xyz, 3x2x1 yxz, 2x3x1 delta 0 | 270 | 270 |
| `box3d` (7x5x4 graded, 140 cells) | simple 2x2x2, 3x2x2, 4x1x2 delta 0; hierarchical 2x2x2 zyx, 3x2x2 xyz, 2x2x3 yzx, 4x1x2 delta 0 | 612 | 612 |
| `lshape` (3 blocks, 96 cells) | simple 2x2x1, 3x1x2 delta 0; hierarchical 2x2x2 xyz, 3x2x1 yxz | 216 | 216 |

**The comparison can fail (ablation).** Replacing the libstdc++ introsort (`decompose::std_sort`)
by Rust's stable sort, nothing else changed, makes 2 of the 3 `delta 0` simple variants differ
(`cavity simple_3_2_1_d0`: 24 of 54 files identical; `box3d simple_4_1_2_d0`: 6 of 72): with
`delta 0` the cell centres of a structured mesh tie, `std::sort` is not stable, and which tied
cell lands first decides its processor. With the default `delta 0.001` the rotation breaks the
ties and any sort passes. (This is why D1 predicted the failure would appear only on `delta 0`.)
The two `delta 0` hierarchical variants and `lshape simple_3_1_2_d0` (whose ties happen to fall on band edges) pass with either sort, as `stable_sort` is what ESI
uses there (`SortableList`).

## Method

- **Reference.** OpenFOAM.com ESI v2506 `decomposePar -force` (dummy Pstream, `-O3`, serial),
  built by the `decompose` stage of `scripts/upstream/build-esi-v2506-and-genfoam.sh`
  (`libfaDecompose`, `libfaReconstruct`, `decomposePar`, `reconstructPar`; `libdecompositionMethods`,
  `libdecompose`, `libreconstruct` come from the `prereq-libs` stage). `scotch`, `metis` and
  `kahip` need external libraries that are not installed: the apps are linked against empty
  dummy libraries, so those methods do not exist in this reference build.
- **Inputs.** `cases/*.blockMeshDict` through ESI `blockMesh` (`writePrecision 17`), then
  `run_upstream.sh <workdir>` (17 variants; each writes `system/decomposeParDict`). Cavity is the
  OpenFOAM tutorial dictionary, `lshape` is the polyDualMesh differential's dictionary, `box3d`
  is new (graded, so cell centres are not symmetric between processors).
- **Port.** `outram_foam_mesh::decompose`: `geom` (`simpleGeomDecomp`, `hierarchGeomDecomp`
  serial unweighted, `geomDecomp::adjustPoints` with the `delta` rotation), `std_sort`
  (libstdc++ `std::sort` statement by statement: `__introsort_loop`, median-of-3 pivot,
  `__final_insertion_sort`, heap fallback), `domain` (`domainDecomposition::decomposeMesh` and the
  polyMesh part of `writeDecomposition`, `polyMesh::initMesh(cellList)`). The cell centres are the
  `check_mesh::esi_geometry` port, already bit-identical to the ESI binary (gh:#1364).
  Writers: `PolyMesh::write_esi_processor` and `write_label_list_esi` in `outram-foam-basic-lib`
  (`IOstream::minPrecision(10)` is applied: precision is `max(writePrecision, 10)`).
- **Comparison.** `compare.sh <workdir> <decompose_par binary>` runs the example
  `decompose_par` on each case and `cmp`s every file of every `processorN` tree. The committed test
  `tests/decompose_par_esi_v2506.rs` freezes the ESI trees as FNV-1a hashes of the file bytes
  (`tests/fixtures/esi_decompose/fnv_<case>_<variant>.txt`, from `examples/fnv_tree.rs`) so it needs
  no ESI install. Pass criterion: every file byte-identical. No tolerance exists to relax.

## Upstream findings recorded (read before porting)

- `Foam::sort` is `std::sort` (`UList.C:313`), `SortableList::sort` is `std::stable_sort`. A port must
  reproduce the first one's tie order.
- `geomDecomp` applies a rotation by default (`delta 0.001`) to the points before sorting, so the
  default result is not the axis-aligned brick one expects; `delta 0` removes it.
- `domainDecompositionMesh.C` calls `sortedOrder(nbrs, order)` and then never uses `order`: the
  processor patches follow `Map<label>::toc()`, ascending only while labels are below the table
  size (128). The port refuses more than 128 processors rather than guess.
- `procBoundary<me>to<nbr>` is created for every pair of processors that share a face; patches of
  size 0 on a processor keep their `startFace`.

## Not ported (child issues filed, see gh:#1381)

`scotch`, `metis`, `kahip`, `ptscotch` (external C libraries; not available here, so also no reference),
`manual`, `random`, `multiLevel`, `structured`; weighted `hierarchical` (`findBinary`); cyclic and
`processorCyclic` patches; point, face and cell zones, sets, `hexRef8` data; decomposition
constraints; field decomposition and `reconstructPar`; `reconstructParMesh`; `redistributePar`;
parallel (`parRun`) branches. Beyond 128 processors the port refuses.
