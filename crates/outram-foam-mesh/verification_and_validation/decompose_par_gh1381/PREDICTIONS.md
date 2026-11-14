# Predictions for gh:#1381 decomposePar differential (committed BEFORE any comparison)

Written 2026-11-14, before `decomposePar` was run on any case and before any Rust
was written. Reference: ESI v2506 `decomposePar` (built by the new `decompose`
stage of `scripts/upstream/build-esi-v2506-and-genfoam.sh`; scotch, metis and kahip
are linked as empty dummies, so only `simple`, `hierarchical`, `manual`, `random`,
`none` and `multiLevel` exist upstream here).

Ported (source read, not yet compared): `simpleGeomDecomp`, `hierarchGeomDecomp`
(serial, unweighted), `geomDecomp::adjustPoints` (the `delta` rotation),
`domainDecomposition::decomposeMesh` and the polyMesh part of `writeDecomposition`.

| id | prediction | reasoning |
|---|---|---|
| D1 | The cell-to-processor list of `simple` is identical on every variant (0 cells differ). | The only floating-point step is `csys_.localPosition` = `R^T & p` with `R` from `delta = 0.001`, evaluated left to right; ESI is built `-O3` with no `-march`, so there is no FMA, and Rust does not contract. The sort is `std::sort` (unstable), so ties are resolved by libstdc++ introsort: the port re-implements that algorithm statement by statement. With `delta 0` every row of a structured mesh ties, so those variants test the introsort port, not the rotation. If the introsort port is wrong, D1 fails only on `delta 0` variants. |
| D2 | `hierarchical` identical on every variant. | `SortableList::sort` is `std::stable_sort` (unique result), the bin size in the serial path is integer `size/n`, and the rotation is shared with D1. |
| D3 | Every `processorN/constant/polyMesh/{points,faces,owner,neighbour,boundary}` is byte-identical, header and `note` included, on every variant. | `polyMesh(io, points, faces, cells)` only derives owner/neighbour by the first-visit rule; the face numbering is the `procFaceAddressing` order, a deterministic integer construction. Risk: the header, and the extra entries of `processor` patches (`matchTolerance`, `transform`, `myProcNo`, `neighbProcNo`), which I have read in `processorPolyPatch::write` but not seen in a file yet. |
| D4 | The four addressing files (`point/face/cell/boundaryProcAddressing`) are byte-identical. | pure integer lists; `faceProcAddressing` carries the `-(facei+1)` turning index on the neighbour side. |
| D5 | The processor patch order is ascending neighbour processor on every variant (fewer than 128 processors). | `procNbrToInterPatch[proci].toc()` of a `Map<label>`: the hash of a small non-negative label is the label itself, so the table iterates ascending. NOTE `sortedOrder(nbrs, order)` upstream fills `order` but the loop never uses it, so beyond 128 processors (table wraps) the order would differ from ascending; not tested, recorded as a caveat. |
| D6 | `decomposePar` on a 2D `empty` mesh keeps the `empty` patches with `nFaces` per processor. | read from `decomposeMesh`. |
| D7 | Total: 100 % of compared files byte-identical. If any differ, first suspect the number formatting of points (the `%.17g` path the blockMesh differential already pins, but the processor meshes use `IOstream::minPrecision(10)`), then the header. | |

Not predicted (not ported, reported as such): scotch/metis/kahip/ptscotch (external
libraries), `manual`, cyclic and `processorCyclic` patches, zones, sets, `hexRef8`
data, `-cellDist`, weights, constraints (`preserveFaceZones`, `preservePatches`,
`singleProcessorFaceSets`, `refinementHistory`), `reconstructParMesh`, `redistributePar`.
