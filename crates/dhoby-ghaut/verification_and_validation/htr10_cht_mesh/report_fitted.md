HTR-10 solid / fluid / porous mesh: 185798 cells, 3279629 points, 1422484 faces; tet -> dual at 10 cm, 32 segments; 55.8 s

Region boundaries: body-fitted (every interface is a face of the mesh).

| cellZone | class | cells | meshed volume [m³] | exact R-Z volume (analytic) [m³] | difference | faceted (32 segments) volume [m³] | difference |
|---|---|---|---|---|---|---|---|
| `porous_bed` | pebble bed (porous) | 9198 | 3.1439 | 3.1446 | -0.02 % | 3.1244 | +6.21e-3 |
| `fluid_upper_plenum` | fluid | 8753 | 2.7537 | 2.7544 | -0.03 % | 2.7368 | +6.20e-3 |
| `fluid_lower_plenum` | fluid | 2270 | 0.5992 | 0.5991 | +0.01 % | 0.5953 | +6.53e-3 |
| `solid_graphite` | solid | 165577 | 62.6804 | 62.6828 | -0.00 % | 62.2808 | +6.42e-3 |

| interface | faces |
|---|---|
| `fluid_lower_plenum_to_solid_graphite` | 8004 |
| `porous_bed_to_solid_graphite` | 12306 |
| `fluid_upper_plenum_to_solid_graphite` | 18054 |
| `porous_bed_to_fluid_upper_plenum` | 3126 |
| `all interfaces` | 41490 |

| patch | faces |
|---|---|
| `top` | 12765 |
| `bottom` | 12843 |
| `side` | 83922 |

Cell kinds: Polyhedron 185798. Max non-orthogonality 86.4 deg, max skewness 12.47.

4 cellZones; 185798 of 185798 cells are in one.

- TetGen tetrahedra with region attributes (1053587 tetrahedra), each region dualled separately and stitched: 41490 interface faces
- TetGen -pq1.414/3.5a0.00011785113019775794A
- tetrahedra: TetGen { radius_edge: 1.414, min_dihedral: 3.5, max_volume: Some(0.00011785113019775794) }; region boundaries: Fitted
- snapped 64547 surface points radially onto the true surface of revolution (32-facet input), after morphing 616383 points within 10.0 cm of a cylinder
- quality by place: 13840 cells on a region interface, max non-orthogonality 86.4 deg, max skewness 12.47; the other 171958 cells 63.8 deg, 1.42. Worst non-orthogonality at r 89.2, z -62.9 cm; worst skewness at r 89.2, z -62.9 cm
- 8157 of 185798 cells are not star-shaped about their centroid (a consumer that splits cells about the centroid must check `decomposition_is_valid`)
