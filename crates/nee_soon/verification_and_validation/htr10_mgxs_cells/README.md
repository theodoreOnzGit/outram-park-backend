# HTR-10 cells for the MGXS gate (gh:#849)

**Research, education and V&V only** (`RESPONSIBLE_USE.md`). Nothing here is
for reactor operation, licensing or safety decisions.

The cells are `nee_soon::htr10_mgxs::cells::Htr10Cell`. They are built from
the full core's own TRISO lattice (`htr10_rmc::core_shell::TrisoBuild`) and
pebble builder (`htr10_rmc::core_model::build_tile_universes`), which were
extracted from the core builder for this purpose. The extraction is proven to
be a no-op on the core: the `Debug` hash of the assembled geometry is the same
before and after, `a1c325330bec698b` (length 11 422 660) at 14 x 12 and
`a1fe9d4b5e48fb93` at 4 x 3, measured 2026-10-13.

| cell | boundary | balls with a piece inside | ball fraction |
|---|---|---|---|
| `single_pebble()` | reflective 6 cm cube | 1 fuel | 0.5236 (simple cubic) |
| `seker_cluster(1)` | reflective Wigner–Seitz hexagon of the tile lattice, one tile high | 13 fuel + 10 dummy = 23 | 0.6448 (Şeker) |
| `seker_cluster(2)` | the same, superlattice (2, 1): 7 tiles' worth | 100 fuel + 76 dummy = 176 | 0.6448 |
| `seker_cluster(3)` | the same, superlattice (3, 2): 19 tiles' worth | 247 fuel + 187 dummy = 434 | 0.6448 |

Each pebble holds 8335 TRISO particles, the core's count. Fuel and dummy balls
follow the core's `SekerBed` rule at the design's fuel-ball fraction, 0.57.
The cells hold 0.565, 0.568 and 0.569 of their balls as fuel (by count).
Why the side faces are a superlattice hexagon, and what that approximates, is
in the module docs of `cells.rs`.

## 1. Drawings (crate HARD RULE)

Regenerate with:

```text
cargo run --release -p nee_soon --example htr10_mgxs_cell_images
```

Every image is drawn from the assembled geometry with `render_material_slice`
(a cell and material lookup at each pixel).

| file | plane |
|---|---|
| `single_pebble_xy_layer.png`, `single_pebble_xz_layer.png` | x-y at z = 0.1385 cm and x-z at y = -0.1229 cm, each through a TRISO layer near the centre |
| `single_pebble_triso_xy.png` | 0.8 cm round one particle |
| `seker_cluster_r{1,2,3}_xy_basal.png` | x-y, 0.05 cm above the bottom face |
| `seker_cluster_r{1,2,3}_xy_central.png` | x-y at z = 0 |
| `seker_cluster_r{1,2,3}_xz.png` | x-z at y = 0 |
| `seker_cluster_r{1,2,3}_pebble_xy.png` | 7 cm round the fuel pebble nearest the centre, through a TRISO layer |
| `seker_cluster_r{1,2,3}_triso_xy.png` | 0.8 cm round one particle in that pebble |

**What was checked in them (2026-10-13):**

- **The single pebble.** The pebble is tangent to all four cube faces. Helium
  fills the corners. The particles fill a disc of radius about 2.45 cm, inside
  the 2.5 cm fuel zone, and none of them cross the zone boundary.
- **The TRISO particle.** All five layers are drawn in order (kernel, buffer,
  IPyC, SiC, OPyC) inside the matrix, on the core's cubic pitch of 0.195 cm.
- **The rings-1 slices.** The hexagon is the tile. The basal plane holds the
  7-ball flower, centred and wholly inside. The central plane holds the
  B-layer balls, the outer ones cut by the faces. No ball overlaps another.
- **The rings-2 and rings-3 slices.** The prism is rotated against the tiles,
  by 19.1° and 23.4°. The balls run continuously across tile borders, with no
  gaps or overlaps at the seams. Every face cuts balls, as the superlattice
  construction requires.
- **The x-z slices.** The top and bottom planes cut the A-layer balls through
  their centres. Fuel balls show particles, and dummy balls show plain
  graphite.

**What cannot be seen in them:**

- **Planes between particle layers.** The TRISO lattice has an even count per
  axis, so a plane through a pebble's centre runs between particle layers and
  shows plain matrix. The fuel-zone matrix and the shell are the same graphite
  material. This is why the pebble-level images are drawn through a layer.
  The first render was at z = 0 and looked like a pebble with no fuel. It was
  found by looking at the image and redrawn.
- **Volume fractions.** These are measured by sampling instead, in section 2.

## 2. Packing and fuel volume, by sampling

`cells::tests` samples points with a fixed xorshift generator and locates each
one in the assembled geometry. It compares the fraction of points in a ball,
and in a kernel, against the values implied by the construction. The kernel
expectation counts the fuel balls actually present. Pass criterion: within 4σ
(binomial). Measured 2026-10-13:

| cell | ball fraction, sampled / expected | kernel fraction, sampled / expected |
|---|---|---|
| single pebble | 0.52537 / 0.52360 (+1.59σ) | 0.00248 / 0.00255 (−0.61σ) |
| rings 1 | 0.64522 / 0.64484 (+0.29σ) | 0.00204 / 0.00181 (+1.98σ) |
| rings 2 | 0.64593 / 0.64484 (+0.82σ) | 0.00191 / 0.00178 (+1.07σ) |
| rings 3 | 0.64295 / 0.64484 (−1.42σ) | 0.00163 / 0.00177 (−1.18σ) |

A fixed defect showed up here first. A ball only *tangent* to a prism face
(distance from the face = r) was counted as inside, which gave rings 1 a total
of 31 balls instead of 23. Since 2026-10-13 a ball must reach 1 µm past the
face to count.

## 3. The CSG single pebble against `DhUniverse::pebble`

Test: `tests/htr10_single_pebble_vs_dh_universe.rs` (gated by `long-tests`).
The methodology is in its module docs. In short, it uses the same data and
materials in both arms. The CSG arm is the cubic lattice in a reflective cube.
The DH arm is random packing in a reflective equal-volume sphere
(r = 3.7221 cm). Pass criterion: |Δk| < 3σ combined. Prediction, made before
the run: |Δk| < 300 pcm, sign not predicted.

**Result** (2026-10-13; 4000 x [10 + 50], seed 20 261 013; 4-core Xeon @
2.10 GHz; 494 s CSG, 66 s DH):

| arm | k |
|---|---|
| CSG single pebble | 1.68619 ± 0.00223 (0 histories lost) |
| DhUniverse pebble | 1.68517 ± 0.00266 |
| CSG − DH | **+102 ± 347 pcm (+0.29σ): passes** |

The two independently built pebbles agree. **This budget does not resolve
the prediction**, because σ = 347 pcm exceeds the 300 pcm bound. What it does
exclude, at about 3σ, is a gross build defect, such as #316's TRISO-count
error (+353 pcm on the core). A probe at 500 x [2 + 3] gave −12 ± 1864 pcm.

## 4. Cell-size convergence, 1 / 7 / 19 tiles

Recorded cases (`htr10_mgxs::cases`, run on the long-case machine):
`cell_pebble_t300`, `cell_seker1_t300`, `cell_seker7_t300` and
`cell_seker19_t300`, each 10 000 x [20 + 135], seed 20 261 012.

### Pilots on this machine

The pilots are the `pilot_*` cases: 2000 x [10 + 40], seed 20 261 012,
2026-10-13, on a 4-core Xeon @ 2.10 GHz, 925–1579 s each. They are not
records, and their output went to `target/htr10_mgxs_pilots/`. All checks
passed on all four, and no history was lost or leaked.

The table gives one-group flux-weighted constants from the 70-group
condensation. Their σ is propagated in quadrature from `condense`'s per-group
σ, ignoring the correlations between groups.

| cell | k (= k_inf, reflective) | kernel Σa \[1/cm\] | kernel νΣf \[1/cm\] | matrix Σt \[1/cm\] |
|---|---|---|---|---|
| single pebble | 1.68253 ± 0.00423 | 0.5970 ± 0.0025 | 1.0376 ± 0.0047 | 0.38280 ± 0.00030 |
| Şeker, 1 tile | 1.74844 ± 0.00309 | 0.8283 ± 0.0038 | 1.5310 ± 0.0072 | 0.38867 ± 0.00036 |
| Şeker, 7 tiles | 1.74364 ± 0.00334 | 0.8297 ± 0.0038 | 1.5374 ± 0.0072 | 0.38857 ± 0.00038 |
| Şeker, 19 tiles | 1.74353 ± 0.00329 | 0.8322 ± 0.0039 | 1.5367 ± 0.0075 | 0.38864 ± 0.00037 |

**What this shows:**

1. **The side-face approximation is not resolved at this budget.** From 1
   to 19 tiles, k moves −491 ± 450 pcm (1.1σ), and from 7 to 19 tiles it
   moves −11 ± 470 pcm. The kernel constants agree within 1σ across N.
   The record budget (15.5x the histories, σ ≈ 85 pcm per cell) resolves a
   shift of about 250 pcm. Until those records exist, the convergence over N
   is **not measured**, only bounded at about 1300 pcm (3σ).
2. **The single pebble is a different neutronic cell, not a cheaper copy of
   the bed.** Every ball in it is fuel. The Şeker cells carry the core's
   43 % dummy (pure graphite) balls, and helium is 48 % of the pebble cell's
   volume against 36 % in the clusters. So the pebble cell is
   under-moderated. Its spectrum is harder: the kernel's one-group Σa is 28 %
   lower and νΣf 32 % lower. Its k is lower by 6100 ± 536 pcm against
   19 tiles. Constants condensed on the single pebble do **not** describe the
   core's fuel. It remains useful as the simplest MG-against-CE gate (#850),
   where the cell only has to be the same on both sides.
3. **The pilot of the single pebble agrees with section 3's CSG arm**, which
   used a different seed and budget: 1.68253 ± 0.00423 against
   1.68619 ± 0.00223, a difference of −366 ± 478 pcm.
