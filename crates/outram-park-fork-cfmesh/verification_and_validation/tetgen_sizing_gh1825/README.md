# TetGen port, `-m` mesh sizes, and wall grading of the TetGen tet source (gh:#1825)

> AI-assisted draft, untrusted until human-reviewed (`RESPONSIBLE_USE.md`).
> For education, research and V&V only.

Epic gh:#1789; found by gh:#1796. With `-pq1.414a<V>` TetGen's cells were
finer near the pebble, because its surface triangles are small, but not
toward the pipe or channel walls; the maintainer's reference pictures
(described on gh:#1796) show cells shrinking toward every wall.

## What upstream does (read first: TetGen 1.6.0, `tetgen.cxx`)

- **`.mtr` sizes.** `tetgenio::load_mtr` (`:735`), called by `load_plc` after
  the `.poly`/`.smesh` whether or not `-m` is given, reads one size (or a
  3/6-component tensor) per input point. `transfernodes` (`:10732`) copies
  `size x metric_scale` into each point's `pointmtrindex` word, **also
  without `-m`**: then, without `-q`, that word is the insertion-radius word
  (`initializepools`, `:4882`: `sizeoftensor` is 1 and
  `pointinsradiusindex == pointmtrindex`).
- **`-m` in `parse_commandline`** (`:3341`): `metric = 1`, an optional
  `metric_scale`; no post-processing, and `-m` does not imply `-q`.
- **Refinement under `-m`.** `checktet4split` (`:28391`): after the `-a`
  test, a tetrahedron whose circumradius exceeds the size of any vertex
  that has one is split with `qflag = 1` (the split then does not check
  insertion radii). `check_subface` and `check_enc_segment` have **no**
  size test: surface triangles and segments are refined only when a
  tetrahedron's circumcentre encroaches upon them. `check_tetrahedron`'s
  `-m` block is dead code (its one call is commented out, `:29071`).
- **`insertpoint`** (`:9393`, `:9423`): with `rejflag & 4` (set by
  `split_segment`, `split_subface`, `split_tetrahedron` under `-m`) a point
  closer than **half** a cavity vertex's size is rejected (`ENCVERTEX`);
  with `assignmeshsize` (`= b->metric` at every Steiner-point caller) the
  new point gets `getpointmeshsize` (`:25231`): the P1 interpolation of its
  tetrahedron's (face's, edge's) vertex sizes when **all** are positive,
  else 0. With `INSTAR` (the two callers that collect the cavity
  themselves: `add_steinerpt_in_schoenhardtpoly`,
  `suppressbdrysteinerpoint`) upstream first calls `locate` "for size
  interpolation" and then gets size 0: the `locate` still moves the start
  tetrahedron and consumes `randomnation`, so it is ported.
- **`meshcoarsening`** (`:25740`, under `-m` or `-R`) runs after the
  Steiner-point suppression: `collectremovepoints` lists every vertex that
  is not a ridge, facet, free-facet, free-segment or unused vertex and whose
  shortest edge (`getvertexstar(1, ...)`) is below its size, then every
  input point marked `-1`; `removevertexbyflips` removes them, raising the
  automatic flip link level per round.
- **Output.** `outelements` points every vertex at its last tetrahedron
  (`setpoint2tet`, `:33950`); `outmetrics` (`:33695`) writes `.mtr` (each
  point's size, `" %-16.8e"`) and `.p2t` (each point's tetrahedron).
- **The background mesh** (`-m` with `<file>.b.node/.ele/.mtr`, `bgm`,
  `interpolatemeshsize`, `scout_point`) is only used when that mesh exists;
  it needs `-r`'s `reconstructmesh`. `in->tetunsuitable` is a library
  callback the unmodified binary cannot check against.

**Which `-m` code was absent from `src/tetgen/`:** all of it. Stage 6 listed
`-m sizes` as not ported (`refine.rs`, `steiner.rs`, `carve.rs` comments);
no point carried a size.

## What was ported

- `tetgen/sizing.rs` (new): `transfer_point_mtrs` (the `transfernodes`
  lines, with the insertion-radius quirk), `getpointmeshsize`,
  `collectremovepoints` (size and marker `-1` parts; `-R` not ported),
  `meshcoarsening`.
- `steiner.rs`: `insertpoint`'s `rejflag & 4` and `assignmeshsize`;
  `InsertPointFlags::assignmeshsize`, set at every upstream caller
  (`add_steinerpt_in_schoenhardtpoly` twice, with the `INSTAR` `locate`;
  `add_steinerpt_in_segment`; `add_steinerpt_to_recover_edge`;
  `recoversubfaces` twice). `carve.rs`: `suppressbdrysteinerpoint`'s.
  `refine.rs`: `checktet4split`'s size test, the splits' `rejflag |= 4` and
  `assignmeshsize`.
- `mesh.rs`: `Behavior::metric`, `metric_scale`; `PointRecord::mtr`;
  `TetgenMesh::reachm`. `plc.rs`: `PlcInput::point_mtrs`, `read_mtr`
  (one column; 3 or 6 refused). `tetrahedralize.rs`: `Switches::metric`,
  `m[#]` in `Switches::parse`, `meshcoarsening` in `finish_plc`.
  `output.rs`: `TetgenOutput::point_mtrs`, `point2tet`, `mtr_text`,
  `p2t_text`, C's `%-16.8e`.
- Not ported: the background mesh, anisotropic sizes, `-R`, the `-D` and
  `-i` callers (`delaunizesegments`, `refineregion`,
  `insertconstrainedpoints`), `tetunsuitable`.

## The choice: sizes on the PLC points **and** sized seed points inside

Options: (a) `-m` sizes on the PLC points, (b) a finer surface triangulation
at the walls, (c) both.

**Decided on the merits, before porting, from upstream's code:** (b) alone
cannot grade the volume (only encroachment carries surface sizes inward,
and the radius-edge bound smooths a fine surface over a few cells). (a) is
TetGen's own mechanism, but `getpointmeshsize` interpolates on the
**current** mesh, and a closed surface's first tetrahedra have only surface
vertices: with sizes on the surface points alone, every tetrahedron
spanning wall to wall carries the wall size, so the whole volume is refined
to it. **Predicted, then measured with the upstream binary** on the Mesh
Studio pipe (wall points 0.05 m, cap centres 0.25 m, `-pq1.414a0.00184m`):
mean edge 0.064 m at the wall and 0.068 m on the axis, 541 149 tetrahedra:
uniformly fine, not graded. With twelve sized points on the axis: 0.065 m
at the wall, 0.21 m on the axis, 235 118 tetrahedra.

So `pipeline::tet_source` uses (a) with **seed points**: upstream keeps an
input point that lies on no facet as a mesh vertex, so the PLC gets a
jittered cubic lattice of points inside the volume (spacing the interior
size, at least half a spacing from every surface triangle), and every point
gets `wall_size + (growth - 1) d`, capped at the interior size, `d` its
distance to the nearest wall triangle (`WallGrade`,
`pipeline/tet_source/wall_grade.rs`). The background mesh would carry the
field without adding vertices, but needs `-r` ported; noted for later.

**Predicted look:** cells about 1.25-1.3 x the wall size along the walls
(a size bounds the circumradius), growing inward, the dual's polyhedra
following; strong on the pipe (radius 1 m), weak in the fuel channel's
0.5 m annulus and the pebble's 0.4 m gap, where the distance to a wall
never exceeds 0.25 m.

## Methodology (byte identity)

- **Reference:** the unmodified upstream binary
  (`/home/user/tetgen-upstream/build/tetgen`, libigl/tetgen `e05aca7df74e`,
  x86_64, glibc 2.39), run as `tetgen -<sw> <name>.<poly|smesh>` with
  `<name>.mtr` beside it, without `-Q`. Its `.node`, `.ele`, `.face`,
  `.edge`, `.mtr` and `.p2t` files are the reference.
- **Reach counters:** `sizing_reach.patch` (apply after stage 4's
  `recoverboundary_dump.patch`, stage 5's `stage5_reach.patch` and stage
  6's `stage6_reach.patch`) adds `tgm[32]` (written to `TG_REACHM`): 0
  `getpointmeshsize` calls, 1-4 its interpolating branches (tetrahedron,
  face, edge, vertex), 5 size 0, 6 `ENCVERTEX` by a size, 7 size splits in
  `checktet4split`, 8/9 coarsening candidates / collected, 10 coarsening
  runs, 11 points removed, 12 points not removed, 13 marker `-1` points,
  14 `assignmeshsize`, 15/16 the two `INSTAR` `locate`s, 17 `outmetrics`,
  18 sizes read. The instrumented build writes the same files as the
  unmodified one on every run (checked with `cmp`). The port's `reach5`,
  `reach6` and `reachm` are compared with `tg5`, `tg6`, `tgm`.
- **Inputs (28 runs):** stage 5/6 cases with sizes from three rules (`grad`:
  `diag (0.04 + 0.12 x)` with `x` the point's relative x; `holes`: `grad`
  with every fifth point unsized; `coarse`: a quarter of the box diagonal),
  `skew_points_m1` (every third point marked `-1`); the stage 3 Mesh Studio
  surfaces with `grad`; and the Mesh Studio shapes wall-graded as
  `?grade=on` (`*_graded`: walls h/2, growth 1.2, 2h inside, `-a` the
  regular tetrahedron of 2h rounded; their `.smesh`, with seeds and hole
  points, is written by `tetgen_plc` from the stage 3 surfaces). Switches:
  `-pqmz`, `-pmz` (coarsening only), `-pqm2z` (scale), `-pq1.414a<V>mz`,
  `-pqAmz`, and `-pz` / `-pqz` with a `.mtr` but no `-m`.
- **Pass criterion:** every file equal byte for byte, every `tg5`, `tg6`,
  `tgm` counter equal. No tolerance.
- **Test:** `tests/tetgen_sizing_vs_upstream.rs`, reading committed files
  only (`test_data/tetgen_sizing/`, gzip, 3.8 MB: the `.mtr` inputs, the
  graded `.smesh` inputs, `skew_points_m1.poly`, every run's files and
  counters). `the_comparison_can_fail` raises one wall point's size by 1 %
  and turns `-m` off: both change the `.ele`.
- **Proven able to fail (2026-11-26):** `example_grad.mtr`'s first size
  multiplied by 0.9 in the committed file: 20 of 32 compared files and
  counter sets of the `example_grad` runs differ; restored, all pass. (A
  0.1 % change of a size no tetrahedron is bound by changes only the
  `.mtr`, and 1 % on the pipe's inlet centre changes nothing: sizes that
  never bind do not move the mesh.)

### Regenerating

```bash
S=<scratch>; V=<repo>/crates/outram-park-fork-cfmesh/verification_and_validation
mkdir -p $S/src $S/bin && cp <tetgen-clone>/{tetgen.cxx,tetgen.h,predicates.cxx} $S/src/ && cd $S/src
patch -p1 < $V/tetgen_recoverboundary_gh1793/recoverboundary_dump.patch
patch -p1 < $V/tetgen_plc_gh1794/stage5_reach.patch
patch -p1 < $V/tetgen_quality_gh1795/stage6_reach.patch
patch -p1 < $V/tetgen_sizing_gh1825/sizing_reach.patch
g++ -O0 -c predicates.cxx -o predicates.o
g++ -O2 -ffp-contract=off -o $S/bin/tetgen tetgen.cxx predicates.o -lm
cd <repo>
TETGEN_SIZING_INPUTS=$S/in cargo test --release -p outram-park-fork-cfmesh \
  --test tetgen_sizing_vs_upstream -- --ignored write_sizing_inputs
TETGEN_INSTR=$S/bin $V/tetgen_sizing_gh1825/run_reference.sh $S/in $S/ref
# gzip -n -9 the .mtr and *_graded.smesh inputs, skew_points_m1.poly and
# every $S/ref file into test_data/tetgen_sizing/, then:
cargo test --release -p outram-park-fork-cfmesh --test tetgen_sizing_vs_upstream -- --nocapture
```

## Results (2026-11-26)

**28 of 28 runs identical: 164 of 164 files byte for byte, and every `tg5`,
`tg6` and `tgm` counter equal on every run** (first full run: every file
already equal; one counter differed, `tgm[17]`, the `outmetrics` call the
port had not counted, fixed).

Columns: output nodes, tetrahedra, `.face` triangles, `.edge` segments;
files equal (`.mtr`, `.p2t` only with `-m`); tetrahedra split for size
(`tgm[7]`), points rejected inside half a size (`tgm[6]`), points removed /
listed by `meshcoarsening` (`tgm[11]`, `tgm[9] + tgm[13]`); the port's wall
time (ms, `--release`, the whole `-p` route, files not written, one run).

| input | sw | nodes | tets | faces | edges | files | size splits | ENCVERTEX | coarsened | ms |
|---|---|---|---|---|---|---|---|---|---|---|
| `example_grad` | -pqmz | 573 | 2190 | 908 | 174 | 6/6 | 522 | 84 | 0/0 | 15.5 |
| `box_hole_grad` | -pqmz | 792 | 3276 | 1016 | 124 | 6/6 | 719 | 0 | 0/0 | 22.0 |
| `comb_prism_grad` | -pqmz | 350 | 790 | 672 | 258 | 6/6 | 214 | 95 | 0/0 | 9.4 |
| `slit_box_grad` | -pqmz | 793 | 3411 | 870 | 94 | 6/6 | 779 | 45 | 0/0 | 19.4 |
| `skew_points_grad` | -pqmz | 832 | 3634 | 906 | 128 | 6/6 | 1453 | 1070 | 26/26 | 40.7 |
| `sphere_grad` | -pqmz | 1307 | 4222 | 2208 | 2189 | 6/6 | 101 | 1614 | 0/0 | 130.5 |
| `example_holes` | -pqmz | 401 | 1420 | 704 | 158 | 6/6 | 341 | 69 | 0/0 | 10.3 |
| `fuel_pin_hole_grad` | -pqmz | 640 | 2241 | 1168 | 626 | 6/6 | 2136 | 3359 | 0/0 | 204.3 |
| `skew_points_coarse` | -pmz | 24 | 125 | 12 | 20 | 6/6 | 0 | 0 | 30/30 | 2.7 |
| `skew_points_coarse` | -pqmz | 67 | 298 | 84 | 47 | 6/6 | 104 | 187 | 30/30 | 6.7 |
| `skew_segments_coarse` | -pmz | 18 | 67 | 12 | 17 | 6/6 | 0 | 0 | 20/20 | 3.9 |
| `skew_segments_coarse` | -pqmz | 35 | 129 | 40 | 32 | 6/6 | 43 | 33 | 20/20 | 4.8 |
| `facet_wall_coarse` | -pmz | 12 | 12 | 22 | 20 | 6/6 | 0 | 0 | 20/20 | 1.1 |
| `facet_wall_coarse` | -pqmz | 27 | 48 | 56 | 32 | 6/6 | 13 | 0 | 20/20 | 2.4 |
| `example_grad` | -pmz | 28 | 68 | 50 | 44 | 6/6 | 0 | 0 | 0/0 | 0.4 |
| `example_grad` | -pqm2z | 99 | 329 | 186 | 82 | 6/6 | 330 | 425 | 0/0 | 2.8 |
| `example_grad` | -pq1.414a0.01mz | 688 | 2724 | 1021 | 188 | 6/6 | 266 | 1158 | 0/0 | 19.2 |
| `example_grad` | -pz | 28 | 68 | 50 | 44 | 4/4 | 0 | 0 | 0/0 | 0.4 |
| `example_grad` | -pqz | 122 | 361 | 243 | 95 | 4/4 | 0 | 0 | 0/0 | 3.0 |
| `fuel_pin_regions_grad` | -pqAmz | 575 | 2735 | 1098 | 551 | 6/6 | 2239 | 9412 | 0/0 | 621.5 |
| `skew_points_m1_grad` | -pmz | 27 | 137 | 12 | 20 | 6/6 | 0 | 0 | 27/44 | 2.7 |
| `skew_points_m1_grad` | -pqmz | 833 | 3652 | 898 | 125 | 6/6 | 1529 | 1114 | 27/44 | 38.7 |
| `pipe_grad` | -pq1.414a0.0026mz | 764 | 3569 | 814 | 399 | 6/6 | 319 | 5189 | 0/0 | 76.6 |
| `fuel_channel_grad` | -pq1.414a0.0026mz | 738 | 2761 | 1222 | 654 | 6/6 | 1515 | 5969 | 0/0 | 240.8 |
| `pebble_grad` | -pq1.414a0.0026mz | 1853 | 9277 | 2810 | 2343 | 6/6 | 259 | 8847 | 0/0 | 290.0 |
| `pipe_graded` | -pq1.414a0.0147mz | 5594 | 26963 | 4748 | 1662 | 6/6 | 5425 | 6 | 0/0 | 247.4 |
| `fuel_channel_graded` | -pq1.414a0.00436mz | 13104 | 65097 | 10240 | 3140 | 6/6 | 12129 | 2 | 0/0 | 716.3 |
| `pebble_graded` | -pq1.414a0.00436mz | 9220 | 45321 | 7476 | 3234 | 6/6 | 7906 | 611 | 0/0 | 537.0 |

**Branches reached** (summed `tgm` over the runs): every counter but
`getpointmeshsize`'s `ONVERTEX` branch (`insertpoint` returns before it on
`ONVERTEX`, so it is unreachable from `insertpoint`) and
`suppressbdrysteinerpoint`'s size `locate` (`tgm[16]`; that routine's
successful path is unreached since stage 5). The `INSTAR` `locate` of
`add_steinerpt_in_schoenhardtpoly` is reached twice
(`skew_segments_coarse`), points not removed by coarsening 17 times and
marker `-1` points 18 times (`skew_points_m1_grad`).

**`-pz` / `-pqz` with a `.mtr` and no `-m`** give the same four files,
apart from the command line, as stage 5's and stage 6's `example -pz` /
`-pqz` without one (checked with `diff`): the insertion-radius quirk
changes nothing on `-pz`.

## Grading measured

Mean edge length of the tetrahedra with a face on a wall, and of interior
tetrahedra (centroid farther than 0.4 m (pipe), 0.15 m (fuel channel),
0.3 m (pebble) from every wall, no boundary vertex), from the `.node`/`.ele`
files: ungraded = the page's default (`-pq1.414a<V>`, V the regular
tetrahedron of the cell size h, no seeds, hole points as the pipeline
gives), graded = `?grade=on`.

| shape | h | ungraded tets | wall / interior edge [m] | interior / wall | graded tets | wall / interior edge [m] | interior / wall |
|---|---|---|---|---|---|---|---|
| pipe | 0.25 | 18 191 | 0.219 / 0.217 | 0.99 | 26 963 | 0.153 / 0.254 | **1.66** |
| fuel channel | 1/6 | 30 910 | 0.143 / 0.148 | 1.04 | 65 097 | 0.103 / 0.122 | **1.18** |
| pebble | 1/6 | 35 533 | 0.106 / 0.145 | 1.37 | 45 321 | 0.095 / 0.176 | **1.85** |

By distance from the wall (0.1 m bins, mean edge [m]), graded pipe: 0.156,
0.171, 0.193, 0.210, 0.231, 0.252, 0.273, 0.291, 0.304, 0.305 (ungraded:
0.218 to 0.224 throughout); graded pebble: 0.100, 0.118, 0.139, 0.163,
0.182, 0.201, 0.223, 0.242, 0.252.

**Interpretation.** The wall edges come out 1.23-1.25 x the wall size (the
size bounds the circumradius); the sizes ask an interior / wall ratio of
2.6 on the pipe axis and get 1.66 near it (`-a` of 2h and the radius-edge
bound smooth the field). In the fuel channel's 0.5 m annulus the distance
to a wall never exceeds 0.25 m, so the sizes grow only from 0.083 to
0.133 m: the grading there is weak (1.18). The pebble was already finer at
its surface (1.37) and is graded further (1.85), now also toward the
channel wall.

## Images (the drawing rule)

Native, from the assembled TetGen meshes (`mesh_studio --tetgen-render
<dir> graded`, coloured by each tetrahedron's mean edge length in six log
bins): `pipe_graded_{z,x,zoom}_cut.png`, `fuel-channel_graded_*`,
`pebble_graded_*` here. The page's own views, tetrahedra and their
cell-centre dual, z- and x-cuts and a zoom across the walls:
`crates/dhoby-ghaut/verification_and_validation/mesh_studio_gh1771/*_graded_*.png`
(and `pipe_ungraded_dual_wall_zoom.png` beside its graded pair).

Checked in them: the pipe's x-cut runs from the smallest bins at both
walls to the largest on the axis, the inlet and outlet caps (not walls)
keep large cells at their centres; the pebble's finest cells hug the
pebble and the channel walls are finer than mid-gap; the fuel channel's
grading is visible but weak. Not checked: the cells' quality against a
solver (the page's non-orthogonality and skewness are in its report).
