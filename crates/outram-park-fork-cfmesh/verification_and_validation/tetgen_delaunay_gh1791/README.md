# TetGen port, stage 2: point-set Delaunay against the upstream binary (gh:#1791)

> AI-assisted draft, untrusted until human-reviewed (`RESPONSIBLE_USE.md`).
> For education, research and V&V only.

Epic gh:#1789. Stage 2 ports what TetGen 1.6.0's `tetrahedralize()` runs
for a plain point set (no `-p`, `-r`, `-q`, `-w`, `-m`): `initializepools`,
`transfernodes` (bounding box, `exactinit` with its extents, `srand(n)`),
`incrementaldelaunay` (glibc `rand()` permutation, `hilbert_init`,
`brio_multiscale_sort`, `hilbert_sort3`, the three independence checks,
`initialdelaunay`), `insert_vertex_bw` per vertex (`randomsample` with
`-b0`, `locate_dt`, `insphere_s`, the three cavity-filling branches),
`jettisonnodes`, and `outnodes` / `outelements` with `-z` numbering.
Code: `src/tetgen/mesh.rs` (data structure), `src/tetgen/delaunay.rs`
(algorithms, `delaunay_tetrahedralize`).

**Not `insertpoint`.** The issue named `insertpoint` with flips; upstream's
`incrementaldelaunay` calls `insert_vertex_bw` (Bowyer-Watson,
`ivf.bowywat = 1`) instead, so that is what is ported. `insertpoint` is
first reached by stage 3 and is not ported yet; `locate` (its walk, with
`randomnation`) is, with a unit test (`locate` finds every vertex).

## Methodology

- **Reference.** The upstream binary `tetgen` built from libigl/tetgen
  `e05aca7df74e` with its `makefile` (g++ 13.3.0, `-O3`, predicates `-O0`,
  x86_64, glibc 2.39), run unmodified. No instrumented copy was needed.
- **Inputs.** `make_cases.py` writes every case as a 0-based `.node` file
  with `%.17g` coordinates (read back exactly by `strtod` and by Rust's
  `parse`). Fixed seeds; regeneration is byte-identical (checked).
- **Command.** `tetgen -zQ <case>.node` (`-Q` only silences the console;
  the `.1.node`, `.1.ele`, `.1.face` files are still written). Two cases
  change a switch: `random_1000_b0` runs `-zQb0` (no BRIO/Hilbert sort,
  each walk starts from `randomsample`, which draws from `randomnation`) and
  `sphere_centre_last` runs `-zQb/1` (input order, `no_sort`).
- **Committed data** (`test_data/tetgen_delaunay/`, 2.5 MB, `gzip -n -9`):
  the inputs, upstream's `.1.ele` for every case, and `.1.node` for
  `duplicates` (the only case whose output points differ from its input).
  The 20 000-point case is committed in full.
- **Test.** `tests/tetgen_delaunay_vs_upstream.rs` reads only the committed
  files (no compiler: runs on Android). Pass criterion, per case: equal
  tetrahedron count, every tetrahedron's four indices equal **in the same
  order within the tetrahedron**, the tetrahedra in the same order, and the
  output points equal bit for bit. No tolerance.
- **Upstream-free check** (`delaunay_properties_hold`, six cases): every
  tetrahedron has `orient3d(a, b, c, d) < 0` (TetGen's positive
  orientation), every interior face is locally Delaunay
  (`insphere(a, b, c, d, e) >= 0` for the opposite vertex `e`, upstream's
  sign: `insphere < 0` means inside), every face seen once is a convex-hull
  face with all points on its inner side, and the tetrahedra's volumes sum
  to the hull volume within 1e-12 relative (measured: 1.7e-16 to 8.4e-15).

### Regenerating

```bash
S=/tmp/tetgen_stage2; mkdir -p $S && cd $S
python3 -I <repo>/crates/outram-park-fork-cfmesh/verification_and_validation/tetgen_delaunay_gh1791/make_cases.py cases
cd cases && cp random_1000.node random_1000_b0.node
for f in *.node; do case $f in
  *_b0.node) fl=-zQb0;; sphere_centre_last.node) fl=-zQb/1;; *) fl=-zQ;; esac
  <tetgen-clone>/build/tetgen $fl $f; done
T=<repo>/crates/outram-park-fork-cfmesh/test_data/tetgen_delaunay
for f in *.1.ele; do gzip -n -9 -c $f > $T/$f.gz; done
for f in *.node; do case $f in *.1.node|*_b0.node) ;; *) gzip -n -9 -c $f > $T/$f.gz;; esac; done
gzip -n -9 -c duplicates.1.node > $T/duplicates.1.node.gz
cargo test --release -p outram-park-fork-cfmesh --test tetgen_delaunay_vs_upstream -- --nocapture
```

## Results (2026-11-26)

| case | points | tets | identical tets | order identical |
|---|---|---|---|---|
| `random_10` | 10 | 14 | 14 | yes |
| `random_100` | 100 | 529 | 529 | yes |
| `random_1000` | 1 000 | 6 388 | 6 388 | yes |
| `random_1000_b0` (`-b0`) | 1 000 | 6 388 | 6 388 | yes |
| `random_20000` | 20 000 | 133 332 | 133 332 | yes |
| `lattice_5x5x5` (degenerate, cospherical) | 125 | 384 | 384 | yes |
| `sphere_exact` (144 integer points, x²+y²+z² = 125) | 144 | 275 | 275 | yes |
| `sphere_unit` (400 normalised) | 400 | 1 168 | 1 168 | yes |
| `duplicates` (300 + 40 exact copies; 300 out) | 340 | 1 754 | 1 754 | yes |
| `nearly_flat` (z in [0, 1e-4)) | 500 | 2 900 | 2 900 | yes |
| `sphere_centre` (3000 normalised + centre) | 3 001 | 5 996 | 5 996 | yes |
| `sphere_centre_last` (`-b/1`, centre last) | 3 001 | 5 996 | 5 996 | yes |

**12 of 12 cases identical: 165 124 of 165 124 tetrahedra, element order
and vertex order within each element, and every output point bit for bit.**
Duplicates: upstream warns "Point #i is coincident with #j. Ignored!" for 40
points, jettisons them, and renumbers; the port reports the same 40
(`Tetrahedralization::duplicates`) and the same `.node`.

Coverage of the branches that matter: all three cavity-filling branches of
`insert_vertex_bw` are reached (`v_out < 64` throughout; `64 <= v_out <
1024` once in `sphere_centre`; `v_out >= 1024` once in
`sphere_centre_last`; counted with a temporary trace, since removed); the
symbolic perturbation of `insphere_s` in the lattice and sphere cases; the
`ONVERTEX` result in `duplicates`; `randomsample` and `randomnation` in
`random_1000_b0`; `no_sort` in `sphere_centre_last`.

**The comparison can fail.** Shifting the new `recenttet` choice by one
(`(rand() % f_out + 1) % f_out`) makes `random_10` differ from its second
tetrahedron on (tried and reverted).

No difference was found, so there is none to explain.

### Run time (x86_64, release; wall clock)

Upstream: its own "Delaunay seconds" (`tetgen -z`, without `-Q`), three runs.
Port: `delaunay_tetrahedralize_with` in the test, which also builds the
output arrays (`outnodes`, `outelements`); two runs.

| case | upstream (ms) | port (ms) |
|---|---|---|
| `random_1000` | 3.2–3.8 | 2.9–3.1 |
| `random_20000` | 66–80 | 72–97 |
| `sphere_unit` | 16–20 | 21–31 |
| `sphere_centre` | 41–48 | 41–57 |
| `sphere_centre_last` (`-b/1`) | 162–183 | 165–179 |

The port is within the run-to-run scatter of upstream to about 1.3× slower.

## What is not ported (unreachable with default switches)

`-w` (weighted: `orient4d_s`, `NONREGULAR`), `-m` metrics, point and
element attributes, `-r`, `-s` and boundary-marker columns of `outnodes`,
`-o2`, verbose printouts. `terminatetetgen` becomes a `TetgenError`.
Fewer than four points is an error here (upstream reads past its array).

## Platform note

The result depends on the C library's `rand()`. The reference links glibc;
`mesh::GlibcRand` is glibc's `TYPE_3` generator (unit-tested against glibc's
first outputs for four seeds). A TetGen built against macOS's, musl's or
MSVC's `rand()` produces a different (equally valid) tetrahedralisation.
