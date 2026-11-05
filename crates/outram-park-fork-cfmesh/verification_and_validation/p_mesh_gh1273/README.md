# cfMesh `pMesh` (Voronoi) as upstream: V&V record (gh:#1273)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-written port, not yet reviewed by
a human.

**Upstream:** `github.com/DAFoam/cfmesh` at `e944f6ba695f` (cfMesh v1.1 line).

## Predictions (written 2026-11-05, before any run)

On the **same** `tetCreatorOctree` tets (gh:#1272), extracted to a tet
`VolumeMesh` (sphere R 3 at 0.5 m; pipe 48 at 2 mm with 1 mm boundary
cells):

1. `voronoiMeshExtractor` makes exactly one cell per tet vertex with no
   boundary edge, i.e. per interior vertex of the tet mesh; the mesh is
   closed and every cell has positive volume.
2. Against `DualKind::CellCentre` (`polyDualMesh`, feature
   `cell-centre-dual`): the same interior cells. For every interior vertex
   the two cells have the same number of faces and the same volume to 1e-12
   relative (same corners: tet centroids; OpenFOAM's tet cell centre equals
   the vertex average up to round-off).
3. Against `MedianPerCorner`: the same set of interior vertices has cells,
   but different volumes (edge midpoints and face centroids as corners).
4. The Voronoi total volume is smaller than the tet mesh's (no cells at the
   boundary vertices): the missing band is about one tet layer.
5. The full `pMesh` workflow keeps the mesh closed with no negative cell;
   the volume error is larger than `tetMesh`'s (the boundary is further
   from the surface before mapping, and `preMapVertices` is not ported).

## Results

(Filled in after the runs.)
