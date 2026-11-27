# TetGen port, stage 6: Delaunay refinement, `-q` and `-a`, against the unmodified upstream binary (gh:#1795)

> AI-assisted draft, untrusted until human-reviewed (`RESPONSIBLE_USE.md`).
> For education, research and V&V only.

Epic gh:#1789. Stage 6 ports what TetGen 1.6.0's `tetrahedralize()`
(`tetgen.cxx:36120`) runs for `-pq`, `-pq<ratio>/<angle>`, `-pa<V>` and
`-pqa<V>` beyond stage 5: `delaunayrefinement` (`tetgen.cxx:29124`) and
everything it reaches, and the switch post-processing of
`parse_commandline` for `-q` and `-a`.

- **`src/tetgen/refine.rs`** (new): `delaunayrefinement`; the maps
  `makefacetverticesmap`, `create_segment_info_list`,
  `get_min_diahedral_angle`, `get_min_angle_at_ridge_vertex`,
  `create_segment_facet_map`, `ridge_vertices_adjacent`,
  `facet_ridge_vertex_adjacent`, `segfacetadjacent`, `is_sharp_segment`,
  `does_seg_contain_acute_vertex`, `set_ridge_vertex_protecting_ball`; the
  tests `check_encroachment`, `check_enc_segment`, `get_subface_ccent`,
  `check_enc_subface`, `check_subface`, `checktet4split`; the splits
  `get_steiner_on_segment`, `split_segment`, `parallel_shift`,
  `locate_on_surface`, `split_subface`, `locate_point_walk`,
  `split_tetrahedron`; the queues `enqueuesubface`, `enqueue_subface`,
  `top_subface`, `dequeue_subface`; the loops `repairencsegs`,
  `repairencfacs`, `repairbadtets`; `projpt2edge`, `projpt2face` and the
  three-point `circumsphere`.
- **`src/tetgen/quality.rs`** (new): `qualitystatistics` (the `-V` block,
  as text and as numbers, with the radius-edge ratio upstream computes but
  does not print) and C's `%.<p>g`.
- Extended, behaviour unchanged without `-q`: `steiner.rs` (`insertpoint`'s
  `rejflag & 1` / `& 2` rejections with `encseglist` / `encshlist`,
  `check_insert_radius` (`ENCVERTEX`), `smlenflag` / `smlen` /
  `parentpt`, the `chkencflag & 1` / `& 2` queues;
  `create_a_shorter_edge` reads the facet maps when built and still errors
  where upstream reads `NULL`), `flip.rs` (the `chkencflag & 1` / `& 2`
  queues of `flip23`, `flip32`, `flip41`), `surface.rs` (`flip22_chk`,
  `flip22` with the queues), `mesh.rs` (`Behavior`: `quality`, `minratio`,
  `mindihedral`, `fixedvolume`, `maxvolume`, `maxvolume_length`;
  `PointRecord::insradius`; `TetgenMesh::rf`, `reach6`), `delaunay.rs`
  (`LocateResult::{SharpCorner, FensedIn, NonCoplanar}`),
  `tetrahedralize.rs` (`finish_plc` runs `delaunayrefinement` after
  `recoverdelaunay`; `Switches::quality`, `Switches::max_volume`, and
  `Switches::parse` reads `q<ratio>/<angle>` and `a<V>`).

**The switch post-processing.** `parse_commandline` (`tetgen.cxx:3736`)
turns `-a#` into `-q` too, and leaves `optmaxdihedral` at 177 under `-q`
(it becomes 179.9 only without `-q`, stage 5). `finish_plc` applies both,
so a hand-built `Behavior` gives what the command line gives. Nothing in
`improve_mesh` or `smooth_vertices` reads `-q` otherwise; they run as in
stage 5, on the refined mesh.

## Methodology

- **Reference.** The unmodified upstream binary
  (`/home/user/tetgen-upstream/build/tetgen`, libigl/tetgen `e05aca7df74e`,
  x86_64, glibc 2.39), run from `PATH` as `tetgen -<sw> <case>`, **without
  `-Q`**. Its `.node`, `.ele`, `.face` and `.edge` files are the reference.
- **Reach counters.** `stage6_reach.patch` (apply after stage 4's
  `recoverboundary_dump.patch` and stage 5's `stage5_reach.patch`) adds
  `tg6[64]` (written to `TG_REACH6`) at every refinement routine and branch
  below, and three dump hooks (`TG_DUMP6S` after the segment pass,
  `TG_DUMP6F` after the subface pass, `TG_DUMP6Q` after refinement; stage
  4's state dump, then exit). Installed as `tetgen` on `PATH`, the
  instrumented build writes the same files as the unmodified binary on all
  41 runs (checked with `cmp`).
- **Runs (41).** The 21 stage 5 cases under `-pqz`; `-pq1.2z` on
  `example`, `comb_prism`, `pipe`, `sphere`, `skew_points`; `-pq1.414/12z`
  on `example`, `box_hole`, `comb_prism`, `slit_box`, `fuel_channel` (and
  `pebble`, `facet_wall`: README only, see below); `-pqa<V>z` on `example`
  (V = 0.01, 122 → 806 nodes), `unit_box` (0.002, 8 → 237), `box_hole`
  (0.01, 52 → 377), `pipe` (0.005, 666 → 1 507); `-pqAz` on
  `fuel_pin_regions`; and the Mesh Studio pipe, fuel channel and pebble
  (size 2) under `-pq1.414a0.0026z` (V ≈ (2/8)³/6, so they look like real
  meshes: 2 726, 2 705 and 4 547 nodes).
- **Pass criterion.** Every file equal byte for byte and every one of the
  64 `tg6` counters equal the port's `TetgenMesh::reach6`. No tolerance.
- **Test.** `tests/tetgen_quality_vs_upstream.rs`, reading committed files
  only: the inputs (stages 3 to 5) and, in `test_data/tetgen_quality/`
  (gzip, 4.9 MB), the 39 committed runs' files and reach counters and six
  `-V` statistics blocks. `pebble -pq1.414/12z` and
  `facet_wall -pq1.414/12z` (0.9 MB more) were run against references in a
  scratch folder (`TETGEN_QUALITY_REF=<dir> TETGEN_QUALITY_EXTRA=1`) and
  are not committed, to keep the data under ~5 MB.

### Regenerating

```bash
S=<scratch>/tetgen_stage6; mkdir -p $S/src $S/bin
cp <tetgen-clone>/{tetgen.cxx,tetgen.h,predicates.cxx} $S/src/ && cd $S/src
V=<repo>/crates/outram-park-fork-cfmesh/verification_and_validation
patch -p1 < $V/tetgen_recoverboundary_gh1793/recoverboundary_dump.patch
patch -p1 < $V/tetgen_plc_gh1794/stage5_reach.patch
patch -p1 < $V/tetgen_quality_gh1795/stage6_reach.patch
g++ -O0 -c predicates.cxx -o predicates.o
g++ -O2 -ffp-contract=off -o $S/bin/tetgen tetgen.cxx predicates.o -lm
# per run (case c, switches sw, tag t = sw with '/' -> '_'):
PATH=<tetgen-clone>/build:$PATH tetgen -$sw $c.<poly|smesh>   # -> c.1.{node,ele,face,edge} as c.$t.*
TG_REACH6=$c.$t.reach6 PATH=$S/bin:$PATH tetgen -$sw $c.<poly|smesh>
tetgen -${sw}V $c.smesh | sed -n '/^Mesh quality statistics:/,/^Memory usage statistics:/p' | sed '$d' > $c.$sw.quality
for f in *.p*.* ; do gzip -n -9 -c $f > <repo>/crates/outram-park-fork-cfmesh/test_data/tetgen_quality/$f.gz; done
cargo test --release -p outram-park-fork-cfmesh --test tetgen_quality_vs_upstream -- --nocapture
```

## Results (2026-11-26)

**41 of 41 runs identical: 164 of 164 files byte for byte, and all 64
reach counters equal on every run** (39 committed, 156 files; the two
README-only runs checked the same way).

Columns: output nodes, tets, `.face` triangles, `.edge` segments; files
equal; refinement insertions on segments / facets / in the volume
(`tg6[3]`, `[11]`, `[25]`); rejections: segment and subface splits refused
at a sharp segment or corner (`[4]`, `[12]`), tetrahedron circumcentres
rejected for encroaching upon a segment / a subface (`[27]`, `[28]`); wall
time of the port (ms, `--release`, the whole `-p` route, files not
written) and of the binary (ms, process start and file writing included),
one run each, this container.

| case | sw | nodes | tets | faces | edges | files | seg / facet / vol insertions | rejected: SHARPCORNER seg / facet, ENCSEG / ENCSUB tet | port ms | upstream ms |
|---|---|---|---|---|---|---|---|---|---|---|
| `example` | -pqz | 122 | 361 | 243 | 95 | 4/4 | 51 / 43 / 0 | 0 / 6, 10 / 10 | 3.3 | 6 |
| `unit_box` | -pqz | 8 | 6 | 12 | 12 | 4/4 | 0 / 0 / 0 | 0 / 0, 0 / 0 | 0.1 | 3 |
| `box_hole` | -pqz | 52 | 138 | 96 | 36 | 4/4 | 12 / 24 / 0 | 0 / 0, 0 / 0 | 1.5 | 5 |
| `box_edges` | -pqz | 11 | 18 | 16 | 16 | 4/4 | 2 / 0 / 0 | 0 / 0, 0 / 0 | 0.1 | 4 |
| `comb_prism` | -pqz | 327 | 853 | 582 | 254 | 4/4 | 144 / 71 / 34 | 0 / 0, 7 / 32 | 9.4 | 16 |
| `grid_box` | -pqz | 74 | 192 | 144 | 24 | 4/4 | 0 / 0 / 0 | 0 / 0, 0 / 0 | 1.3 | 5 |
| `jitter_soup` | -pqz | 103 | 253 | 192 | 48 | 4/4 | 0 / 0 / 5 | 0 / 0, 0 / 0 | 1.1 | 6 |
| `slit_box` | -pqz | 57 | 137 | 104 | 31 | 4/4 | 19 / 9 / 3 | 0 / 0, 0 / 8 | 1.1 | 6 |
| `fin_box` | -pqz | 10 | 9 | 18 | 17 | 4/4 | 0 / 0 / 0 | 0 / 0, 0 / 0 | 0.2 | 3 |
| `pipe` | -pqz | 666 | 2146 | 1126 | 576 | 4/4 | 480 / 19 / 101 | 0 / 0, 7 / 10 | 75.6 | 74 |
| `fuel_channel` | -pqz | 1669 | 5975 | 2746 | 1388 | 4/4 | 1196 / 49 / 296 | 0 / 0, 8 / 57 | 141.1 | 100 |
| `pebble` | -pqz | 2699 | 15235 | 3228 | 2612 | 4/4 | 542 / 16 / 1081 | 946 / 0, 390 / 64 | 499.2 | 451 |
| `sphere` | -pqz | 2037 | 7038 | 3252 | 2705 | 4/4 | 516 / 5 / 410 | 2745 / 0, 421 / 60 | 238.3 | 228 |
| `schoenhardt` | -pqz | 13 | 24 | 20 | 18 | 4/4 | 6 / 0 / 0 | 0 / 4, 0 / 0 | 0.3 | 3 |
| `thin_slab` | -pqz | 14995 | 45863 | 29986 | 596 | 4/4 | 528 / 14305 / 0 | 0 / 0, 31 / 6964 | 993.5 | 899 |
| `facet_wall` | -pqz | 1883 | 10396 | 2027 | 85 | 4/4 | 65 / 922 / 864 | 0 / 0, 14 / 894 | 55.0 | 67 |
| `facet_wall2` | -pqz | 1349 | 7295 | 1704 | 88 | 4/4 | 68 / 755 / 454 | 0 / 0, 19 / 680 | 40.4 | 50 |
| `skew_segments` | -pqz | 2046 | 11552 | 170 | 355 | 4/4 | 338 / 47 / 1623 | 0 / 0, 275 / 46 | 55.8 | 64 |
| `skew_points` | -pqz | 1181 | 6661 | 160 | 357 | 4/4 | 337 / 38 / 752 | 0 / 0, 143 / 32 | 31.6 | 39 |
| `fuel_pin_hole` | -pqz | 1793 | 6459 | 2932 | 1485 | 4/4 | 1293 / 45 / 327 | 0 / 0, 8 / 55 | 147.9 | 130 |
| `fuel_pin_regions` | -pqz | 1997 | 9824 | 3110 | 1519 | 4/4 | 1327 / 70 / 472 | 0 / 0, 8 / 81 | 263.8 | 185 |
| `example` | -pq1.2z | 391 | 1321 | 714 | 160 | 4/4 | 116 / 213 / 34 | 20 / 11, 54 / 168 | 10.6 | 16 |
| `comb_prism` | -pq1.2z | 1488 | 5229 | 2230 | 507 | 4/4 | 397 / 642 / 371 | 0 / 0, 50 / 434 | 39.8 | 46 |
| `pipe` | -pq1.2z | 1166 | 4744 | 1488 | 725 | 4/4 | 629 / 51 / 420 | 0 / 0, 128 / 65 | 115.2 | 90 |
| `sphere` | -pq1.2z | 3227 | 14263 | 3532 | 2846 | 4/4 | 657 / 5 / 1459 | 4371 / 0, 698 / 136 | 313.6 | 326 |
| `skew_points` | -pq1.2z | 2506 | 14326 | 364 | 433 | 4/4 | 413 / 138 / 1901 | 0 / 0, 379 / 110 | 84.2 | 82 |
| `example` | -pq1.414/12z | 221 | 715 | 429 | 126 | 4/4 | 82 / 105 / 6 | 13 / 13, 45 / 49 | 7.8 | 14 |
| `box_hole` | -pq1.414/12z | 64 | 192 | 120 | 36 | 4/4 | 12 / 36 / 0 | 0 / 0, 0 / 6 | 1.7 | 4 |
| `comb_prism` | -pq1.414/12z | 926 | 3280 | 1398 | 356 | 4/4 | 246 / 377 / 225 | 0 / 0, 27 / 297 | 28.4 | 34 |
| `slit_box` | -pq1.414/12z | 92 | 275 | 152 | 37 | 4/4 | 25 / 27 / 14 | 0 / 0, 4 / 25 | 2.0 | 8 |
| `fuel_channel` | -pq1.414/12z | 2583 | 10422 | 3464 | 1693 | 4/4 | 1501 / 103 / 851 | 0 / 0, 22 / 120 | 175.8 | 148 |
| `example` | -pqa0.01z | 806 | 3253 | 1218 | 206 | 4/4 | 162 / 408 / 208 | 0 / 14, 78 / 387 | 22.0 | 39 |
| `unit_box` | -pqa0.002z | 237 | 900 | 326 | 49 | 4/4 | 37 / 120 / 72 | 0 / 0, 31 / 122 | 6.1 | 10 |
| `box_hole` | -pqa0.01z | 377 | 1404 | 590 | 93 | 4/4 | 69 / 214 / 78 | 0 / 0, 51 / 191 | 11.4 | 17 |
| `pipe` | -pqa0.005z | 1507 | 7009 | 1452 | 691 | 4/4 | 595 / 67 / 779 | 0 / 0, 99 / 106 | 87.8 | 77 |
| `fuel_pin_regions` | -pqAz | 1997 | 9824 | 3110 | 1519 | 4/4 | 1327 / 70 / 472 | 0 / 0, 8 / 81 | 246.8 | 224 |
| `pipe` | -pq1.414a0.0026z | 2726 | 13217 | 2258 | 1037 | 4/4 | 941 / 124 / 1595 | 0 / 0, 425 / 164 | 177.5 | 190 |
| `fuel_channel` | -pq1.414a0.0026z | 2705 | 11176 | 3518 | 1718 | 4/4 | 1526 / 105 / 946 | 0 / 0, 25 / 189 | 178.4 | 199 |
| `pebble` | -pq1.414a0.0026z | 4547 | 26022 | 3980 | 2884 | 4/4 | 814 / 120 / 2553 | 1109 / 0, 667 / 239 | 610.0 | 601 |
| `pebble` (README only) | -pq1.414/12z | 4856 | 28298 | 3770 | 2844 | 4/4 | 774 / 55 / 2967 | 1399 / 0, 639 / 254 | 615.6 | 650 |
| `facet_wall` (README only) | -pq1.414/12z | 4353 | 24694 | 3962 | 117 | 4/4 | 97 / 1847 / 2377 | 0 / 0, 23 / 1661 | 150.0 | 196 |

**Times.** The port runs at the binary's speed: Mesh Studio pebble 610 /
601 ms, pipe 178 / 190, fuel channel 178 / 199, `thin_slab -pqz` 994 /
899 (port / binary). The whole test (39 runs rendered and compared, plus
the statistics, sanity and failure tests) takes about 5 s.

### Reach (the `tg6` counters, summed over the 41 runs; runs that reach each)

| # | counter | total | runs |
|---|---|---|---|
| 0 | delaunayrefinement | 41 | 41 |
| 1 | repairencsegs | 4484 | 41 |
| 2 | split_segment | 28191 | 37 |
| 3 | split_segment inserted | 17344 | 37 |
| 4 | split_segment SHARPCORNER | 10603 | 7 |
| 5 | get_steiner_on_segment: refpt on an adjacent segment | 32 | 7 |
| 6 | ... refpt an adjacent ridge vertex | 1 | 1 |
| 7 | ... at the projection of refpt | 12890 | 35 |
| 8 | ... midpoint (no refpt) | 4665 | 34 |
| 9 | repairencfacs | 15257 | 41 |
| 10 | split_subface | 24915 | 37 |
| 11 | split_subface inserted | 21245 | 35 |
| 12 | split_subface SHARPCORNER | 48 | 5 |
| 13 | ... at the projection of encpt | 7538 | 15 |
| 14 | ... projection too close, circumcentre used | 58 | 9 |
| 15 | locate_on_surface ENCSEGMENT (FENSEDIN) | 7 | 2 |
| 16 | locate_on_surface ONVERTEX / NONCOPLANAR | 764 | 15 |
| 17 | split_subface rejected ENCSEGMENT (segments split) | 2787 | 34 |
| 18 | unsplit subface queued | 318 | 12 |
| 19 | repairbadtets | 53 | 41 |
| 20 | checktet4split true | 58842 | 36 |
| 21 | ... volume (`-a`) | 5424 | 7 |
| 22 | ... radius-edge ratio | 48896 | 33 |
| 23 | ... dihedral angle | 4522 | 31 |
| 24 | split_tetrahedron | 59021 | 36 |
| 25 | split_tetrahedron inserted | 23750 | 32 |
| 26 | ... walk hit a subface / the hull (FENSEDIN) | 686 | 8 |
| 27 | ... rejected ENCSEGMENT | 4794 | 32 |
| 28 | ... rejected ENCSUBFACE | 13797 | 34 |
| 29 | ... no subface holds the projection (fallback) | 101 | 5 |
| 30 | unsplit tetrahedron queued | 1120 | 15 |
| 31 | unsplit tetrahedra retried | 179 | 11 |
| 32 | check_enc_subface: encroaching vertex on the same facet (flippush) | **0** | 0 |
| 33 | insertpoint `rejflag & 1` (ENCSEGMENT) | 7581 | 34 |
| 34 | insertpoint `rejflag & 2` (ENCSUBFACE) | 13797 | 34 |
| 35 | insertpoint `check_insert_radius` (ENCVERTEX) | 16090 | 22 |
| 38 | set_ridge_vertex_protecting_ball sets a radius | 20 | 7 |
| 39 | recoverdelaunay from refinement | 27 | 17 |
| 40 | lawsonflip3d after a refinement insertion | 62339 | 38 |
| 41 | locate_point_walk random choice (`randomnation`) | 407 | 8 |
| 42 | locate_on_surface coin flip (`rand()`) | **0** | 0 |
| 43 | split_subface refused otherwise (BADELEMENT, NULLCAVITY, ENCVERTEX) | 64 | 5 |
| 44 | split_segment insertion refused | 244 | 9 |

**Not reached by any run** (say so if you rely on them): counter 32
(`check_enc_subface`'s same-facet branch that pushes the face on the flip
stack, and with it the `lawsonflip3d` calls after the initial subface pass
and in `repairencfacs`; `lawsonflip3d` itself is reached 62 339 times
after insertions), counter 42 (`locate_on_surface`'s glibc coin flip, both
orientations negative). Not instrumented: the new `create_a_shorter_edge`
branches (facet maps), `split_segment`'s sharp-segment insertion radius
(`is_sharp` with a successful split), the `steinerleft` and `elem_limit`
stops (`-S`, `-r`, not ported switches: unreachable here).

### Quality statistics, ours against upstream's (`-V`)

`TetgenMesh::qualitystatistics` (`quality.rs`) renders upstream's
`qualitystatistics()` block. On the six Mesh Studio runs it equals the
binary's `-V` block **byte for byte** (test
`quality_statistics_match_upstream_dash_v`; `-V` does not change the
output files, checked). The radius-edge ratio is computed by upstream but
not printed; the port's `quality_numbers` returns it.

| shape | sw | volume min / max | edge min / max | aspect ratio min / max | face angle min / max | dihedral min / max | largest radius-edge ratio |
|---|---|---|---|---|---|---|---|
| pipe | -pqz | 4.9323e-4 / 0.10057 | 0.19603 / 1.2367 | 1.2867 / 49.719 | 14.93 / 128.6058 | 1.7748 / 175.1950 | 2.0345 |
| pipe | -pq1.414a0.0026z | 6.0035e-5 / 3.4475e-3 | 0.11603 / 0.42315 | 1.279 / 33.463 | 24.037 / 116.7521 | 2.5646 / 176.0308 | 1.2855 |
| fuel channel | -pqz | 5.9769e-5 / 6.0043e-3 | 0.064399 / 0.43642 | 1.3183 / 38.814 | 15.53 / 137.7436 | 2.053 / 175.6508 | 1.9419 |
| fuel channel | -pq1.414a0.0026z | 2.6144e-5 / 3.4559e-3 | 0.075203 / 0.40321 | 1.296 / 32.111 | 20.753 / 131.9620 | 2.6501 / 174.7706 | 1.5408 |
| pebble | -pqz | 8.7888e-8 / 0.034482 | 0.0072714 / 0.8277 | 1.3488 / 70.217 | 10.743 / 141.9849 | 1.475 / 176.7934 | 2.9607 |
| pebble | -pq1.414a0.0026z | 1.6472e-8 / 3.4805e-3 | 0.0082775 / 0.42315 | 1.2623 / 52.014 | 11.236 / 139.3045 | 2.1126 / 176.5885 | 2.8466 |

(Every printed value is the same in both codes; the full blocks are in
`test_data/tetgen_quality/*.quality.gz`.)

**A pointer comparison is emulated.** Upstream counts each face's angles
once, from the tetrahedron at the lower **address**. With slot order the
pipe's face-angle histogram was off by two (80–90 / 90–100 degrees). Each
8188-tetrahedron pool block is a separate `mmap` and Linux places a new
mapping below the previous one, so a later block is lower; with that
order (`quality.rs`, `tet_address_below`) all six blocks are identical
(an assumption about the allocator, stated where it is used).

### Proving the test can fail

- `the_comparison_can_fail` (committed): the volume bound perturbed from
  0.0026 to 0.0027 changes the Mesh Studio pipe's `.ele`. (The ratio bound
  1.414 → 1.415 does not: every tetrahedron of that mesh is below 1.29.)
- By hand (then restored): the subface queue's bucket factor 64 → 63 in
  `SubfaceQueue::enqueue` made 50 of the 156 committed files and reach
  files differ.

### Upstream-free sanity (`refined_meshes_meet_their_bounds`)

Plain-`f64` circumradius over shortest edge and volume of every output
tetrahedron. **After `-pq1.414`, no tetrahedron is above 1.414 except
tetrahedra upstream's own `.ele` also holds**, and likewise for `-pq1.2`:
TetGen leaves a tetrahedron whose circumcentre would encroach upon a
sharp segment or corner, or is not visible from it (FENSEDIN), and retries
unsplit ones at most three times. Counts above the bound: Mesh Studio pipe
0, fuel channel 5 (worst 1.54), pebble 351 (worst 2.85; 1 109
SHARPCORNER refusals on the pebble's facetted sphere); `-pq1.414/12z`:
`box_hole` 0, `slit_box` 4, `example` 20, `comb_prism` 63,
`fuel_channel` 64.

**Finding: the `-a` volume bound is not a guarantee either.** Every `-a`
run leaves tetrahedra above `V`, all of them upstream's too: Mesh Studio
pipe 91 of 13 217 (largest 3.45e-3 against 2.6e-3), fuel channel 44,
pebble 44; `example -pqa0.01z` 27 (largest 0.01274), `unit_box` 7,
`box_hole` 8, `pipe -pqa0.005z` 61. Those are tetrahedra whose split was
refused (an encroached boundary that could not be split, or a walk that
hit a subface); `-a` in TetGen 1.6.0 is a target, not a bound.

## Faults found while porting (each in the port, fixed)

- **A loop variable's version carried across iterations.** In
  `delaunayrefinement`'s subface pass upstream's `checksh` keeps the
  `shver` `check_subface` left (the shortest edge) when the traversal
  moves to the next subface; with a fresh version per subface,
  `thin_slab` and `facet_wall2 -pqz` gave the same nodes and tetrahedra
  but `.face` in another order. Carried, as stage 3's `mergefacets`.
- **`locate_point_walk` decodes the neighbour pointer without `fsymtbl`**
  (`decode(searchtet->tet[ver & 3], *searchtet)`, not `fsymself`): with
  `fsymself` the located tetrahedron's version differed, and on
  `comb_prism -pq1.2z` the order of `encshlist`, and so the subface split,
  diverged.

## The images (drawing rule)

`pipe_{z,x}_cut.png`, `fuel-channel_{z,x}_cut.png`,
`pebble_{z,x}_cut.png`, from `cargo run --release -p dhoby-ghaut --example
mesh_studio -- --tetgen-render <this folder> pq1.414a0.0026zA` (stage 5's
helper, now taking the switches; `-A` only colours the regions: the counts
equal the `-pq1.414a0.0026z` runs above). Mesh Studio's `reactor_surface`
at size 2, `tetrahedralize_plc`, `to_volume_mesh`, then Mesh Studio's own
section code.

| image | shows |
|---|---|
| `pipe_z_cut.png`, `pipe_x_cut.png` | 13 217 tets on 2 726 nodes; the z-cut crosses 416, the x-cut 1 814 |
| `fuel-channel_z_cut.png`, `fuel-channel_x_cut.png` | the coolant annulus (11 176 tets); the open pin carved out as in stage 5 |
| `pebble_z_cut.png`, `pebble_x_cut.png` | two regions: the pebble (attribute 1, 8 761 tets) and the channel (attribute 2, 17 261) |

**What I checked in them:** the meshes are no longer stage 5's long
wall-to-wall tetrahedra: interior nodes everywhere, irregular tetrahedra of
similar size across the section, finer where the boundary is finer (the
pebble's 32-segment sphere, the fuel channel's thin annulus); the fuel
channel's centre is still empty; the pebble region is closed by its
polygonal sphere and the channel region surrounds it; no cell outside the
outer 32-gon; the cut polygons tile the sections without gaps (Mesh Studio
test `refined_sections_are_drawn` checks the refined cut has more cells
than the `-p` one, and stage 5's area test still holds). **What I could
not:** sections do not show slivers away from the cut (the `-V` dihedral
minimum, 2.1 to 2.7 degrees, says some remain: TetGen's, identical); the
dual of these meshes (the reference look of gh:#1796) is stage 7's.

## Not ported (with why)

`-r` with an `.elem` list (refining a given mesh; no `-r` route here);
`-m` mesh sizes (`rejflag & 4`, the `pointmtrindex` tests: without `-m`
every size is 0, so those branches never fire); `-aa` per-region volume
bounds (`varvolume`, `volumebound`, their copies in the flips and in
`insertpoint`; `Switches::parse` refuses `-a` without a number); `-S#`
and `-r#` limits (the `steinerleft == 0` and `elem_limit` stops are
ported but unreachable); `-c`; `in->tetunsuitable` (a library callback);
the progress printing (`refine_progress_ratio`); `check_tetrahedron` (its
only call is commented out upstream); `segsegadjacent`,
`facetfacetadjacent` (not called).

## What stage 7 (gh:#1796) needs

`to_volume_mesh` already gives one cell per tetrahedron, one patch per
facet marker, closed and upper-triangular. For the dual and the layers it
needs the boundary patch of every boundary face named from the cfMesh
surface's regions (it is, through `SurfaceRegions::names`), and for
Mesh Studio a `Switches` choice exposed in the UI (`quality`,
`max_volume`) with the background worker the no-lag rule needs (the
pebble at `-a0.0026` takes 0.6 s).
