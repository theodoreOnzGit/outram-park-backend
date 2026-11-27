HTR-10 solid / fluid / porous mesh: 184204 cells, 3297499 points, 1445531 faces; tet -> dual at 10 cm, 32 segments; 72.0 s

Region boundaries: each region meshed on its own, body-fitted and snapped, joined by cyclicAMI interface pairs.

| cellZone | class | cells | meshed volume [m³] | exact R-Z volume (analytic) [m³] | difference | faceted (32 segments) volume [m³] | difference |
|---|---|---|---|---|---|---|---|
| `porous_bed` | pebble bed (porous) | 8822 | 3.1437 | 3.1446 | -0.03 % | 3.1244 | +6.16e-3 |
| `fluid_upper_plenum` | fluid | 8462 | 2.7536 | 2.7544 | -0.03 % | 2.7368 | +6.15e-3 |
| `fluid_lower_plenum` | fluid | 2120 | 0.5991 | 0.5991 | +0.00 % | 0.5953 | +6.48e-3 |
| `solid_graphite` | solid | 164800 | 62.6807 | 62.6828 | -0.00 % | 62.2808 | +6.42e-3 |

| cyclicAMI pair | area a [m²] | area b [m²] | b / a - 1 |
|---|---|---|---|
| `porous_bed_to_fluid_upper_plenum` / `fluid_upper_plenum_to_porous_bed` | 2.543704 | 2.543704 | +4.44e-16 |
| `porous_bed_to_solid_graphite` / `solid_graphite_to_porous_bed` | 9.531077 | 9.531244 | +1.75e-5 |
| `fluid_upper_plenum_to_solid_graphite` / `solid_graphite_to_fluid_upper_plenum` | 13.751625 | 13.751661 | +2.60e-6 |
| `fluid_lower_plenum_to_solid_graphite` / `solid_graphite_to_fluid_lower_plenum` | 6.476752 | 6.476805 | +8.14e-6 |

| patch | faces |
|---|---|
| `porous_bed_to_solid_graphite` | 10617 |
| `porous_bed_to_fluid_upper_plenum` | 2811 |
| `fluid_upper_plenum_to_porous_bed` | 2847 |
| `fluid_upper_plenum_to_solid_graphite` | 16551 |
| `fluid_lower_plenum_to_solid_graphite` | 7182 |
| `bottom` | 12762 |
| `side` | 83817 |
| `solid_graphite_to_fluid_lower_plenum` | 7140 |
| `solid_graphite_to_porous_bed` | 10602 |
| `solid_graphite_to_fluid_upper_plenum` | 15582 |
| `top` | 12531 |

Cell kinds: Polyhedron 184204. Max non-orthogonality 71.0 deg, max skewness 2.50.

4 cellZones; 184204 of 184204 cells are in one.

- porous_bed: TetGen tetrahedra with region attributes (47603 tetrahedra), each region dualled separately and stitched: 0 interface faces
- porous_bed: TetGen -pq1.414/3.5a0.00011785113019775794A
- porous_bed: snapped 4522 surface points radially onto the true surface of revolution (32-facet input), after morphing 37484 points within 10.0 cm of a cylinder
- porous_bed: snapped 54 rim points (edges between two patches) onto their rings; split 0 rim chords to half a facet
- porous_bed: 8822 cells
- fluid_upper_plenum: TetGen tetrahedra with region attributes (42111 tetrahedra), each region dualled separately and stitched: 0 interface faces
- fluid_upper_plenum: TetGen -pq1.414/3.5a0.00011785113019775794A
- fluid_upper_plenum: snapped 4135 surface points radially onto the true surface of revolution (32-facet input), after morphing 34924 points within 10.0 cm of a cylinder
- fluid_upper_plenum: snapped 53 rim points (edges between two patches) onto their rings; split 0 rim chords to half a facet
- fluid_upper_plenum: 8462 cells
- fluid_lower_plenum: TetGen tetrahedra with region attributes (9200 tetrahedra), each region dualled separately and stitched: 0 interface faces
- fluid_lower_plenum: TetGen -pq1.414/3.5a0.00011785113019775794A
- fluid_lower_plenum: snapped 1547 surface points radially onto the true surface of revolution (32-facet input), after morphing 15442 points within 10.0 cm of a cylinder
- fluid_lower_plenum: snapped 0 rim points (edges between two patches) onto their rings; split 0 rim chords to half a facet
- fluid_lower_plenum: 2120 cells
- solid_graphite: TetGen tetrahedra with region attributes (949570 tetrahedra), each region dualled separately and stitched: 0 interface faces
- solid_graphite: TetGen -pq1.414/3.5a0.00011785113019775794A
- solid_graphite: snapped 62974 surface points radially onto the true surface of revolution (32-facet input), after morphing 532003 points within 10.0 cm of a cylinder
- solid_graphite: snapped 242 rim points (edges between two patches) onto their rings; split 0 rim chords to half a facet
- solid_graphite: 164800 cells
- AMI porous_bed_to_fluid_upper_plenum / fluid_upper_plenum_to_porous_bed: areas 2.543704e0 / 2.543704e0 m², mismatch +4.44e-16; weight sums source 0.9907 / 0.999977 / 1.0000, target 0.9917 / 0.999983 / 1.0000 (min / mean / max)
- AMI porous_bed_to_solid_graphite / solid_graphite_to_porous_bed: areas 9.531077e0 / 9.531244e0 m², mismatch +1.75e-5; weight sums source 0.9999 / 1.000057 / 1.0004, target 0.9901 / 1.000022 / 1.0054 (min / mean / max)
- AMI fluid_upper_plenum_to_solid_graphite / solid_graphite_to_fluid_upper_plenum: areas 1.375162e1 / 1.375166e1 m², mismatch +2.60e-6; weight sums source 0.9906 / 1.000016 / 1.0083, target 0.9907 / 1.000030 / 1.0003 (min / mean / max)
- AMI fluid_lower_plenum_to_solid_graphite / solid_graphite_to_fluid_lower_plenum: areas 6.476752e0 / 6.476805e0 m², mismatch +8.14e-6; weight sums source 0.9852 / 1.000063 / 1.0244, target 0.9999 / 1.000048 / 1.0007 (min / mean / max)
- tetrahedra: TetGen { radius_edge: 1.414, min_dihedral: 3.5, max_volume: Some(0.00011785113019775794), wall_grade: None }; region boundaries: Ami
- quality by place: 0 cells on a region interface, max non-orthogonality 0.0 deg, max skewness 0.00; the other 184204 cells 71.0 deg, 2.50. Worst non-orthogonality at r 188.0, z 91.9 cm; worst skewness at r 188.0, z 91.9 cm
- 7962 of 184204 cells are not star-shaped about their centroid (a consumer that splits cells about the centroid must check `decomposition_is_valid`)
