# polyDualMesh differential: recipe (gh:#1472)

Research, education and V&V only (see `RESPONSIBLE_USE.md`). Results:
`../poly_dual_mesh_gh1460.md`.

Needs the partial OpenFOAM.com v2506 build described in
`../../../outram-park-fork-cfmesh/verification_and_validation/upstream_cfmesh_gh1385/`
(dummy Pstream; `blockMesh` built) plus `finiteVolume`, `meshTools`,
`dynamicMesh`, all of which that recipe builds.

1. Build the application. Copy
   `$WM_PROJECT_DIR/applications/utilities/mesh/manipulation/polyDualMesh` to a
   scratch directory, apply `polyDualMeshApp.local.patch` (drops
   `processorMeshes`, which needs the unbuilt `reconstruct` library and only
   acts on parallel cases), replace `Make/options` with
   `polyDualMesh.Make.options` (no `-lreconstruct -lgenericPatchFields`), and
   run `wmake`. About 30 s.
2. `export POLYDUAL_WORK=<scratch dir>`; `bash mkcases.sh` (four `blockMesh`
   primal cases, `writePrecision 17`), then `bash runesi.sh` (v2506
   `polyDualMesh 60` with each option variant; the dual lands in
   `runs/<case>__<variant>/0.005/polyMesh`, the feature sets in `*.obj`).
3. `cargo build --release -p outram-foam-mesh --examples`, then
   `bash difftable.sh esi` (port in v2506 flavour; expect 12 of 12
   byte-identical) and `bash difftable.sh dev` (port in dev flavour).
   `cmpfeat.py` compares the feature sets (toc order) with
   `examples/dump_dual_features.rs` output; `printEdges.C` is the
   `mesh.edges()` probe (build it like the application).
