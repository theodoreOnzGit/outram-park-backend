HTR-10 solid / fluid / porous mesh: 177789 cells, 3389435 points, 1351334 faces; tet -> dual at 10 cm, 32 segments; 58.2 s

Region boundaries: by cell centroid (stair-stepped at the cell size).

| cellZone | class | cells | meshed volume [m³] | exact R-Z volume [m³] | difference | faceted (32 segments) volume [m³] | difference |
|---|---|---|---|---|---|---|---|
| `porous_bed` | pebble bed (porous) | 7693 | 3.1544 | 3.1554 | -0.03 % | 3.1244 | +9.59e-3 |
| `fluid_upper_plenum` | fluid | 6728 | 2.7441 | 2.7483 | -0.15 % | 2.7368 | +2.69e-3 |
| `fluid_lower_plenum` | fluid | 1442 | 0.5954 | 0.5972 | -0.29 % | 0.5953 | +2.53e-4 |
| `solid_graphite` | solid | 161926 | 62.2434 | 62.6802 | -0.70 % | 62.2808 | -6.02e-4 |

| patch | faces |
|---|---|
| `bottom` | 12741 |
| `side` | 83652 |
| `top` | 12663 |

Cell kinds: Polyhedron 177789. Max non-orthogonality 69.0 deg, max skewness 2.54.

4 cellZones; 177789 of 177789 cells are in one.

- tet source TetGen -pq1.414/3.5a0.00011785113019775794: 1046314 tetrahedra on 177789 nodes; carve, snap and the Delaunay flips not run (TetGen's constrained Delaunay refinement replaces them)
- tetrahedra: TetGen { radius_edge: 1.414, min_dihedral: 3.5, max_volume: Some(0.00011785113019775794) }; region boundaries: Centroid
- 6282 of 177789 cells are not star-shaped about their centroid (a consumer that splits cells about the centroid must check `decomposition_is_valid`)
