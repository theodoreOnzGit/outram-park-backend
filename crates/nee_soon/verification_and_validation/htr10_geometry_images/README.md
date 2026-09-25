# HTR-10 explicit-TRISO core — drawn from the assembled geometry

Images required by the geometry-drawing HARD RULE in `crates/nee_soon/CLAUDE.md`:
an axial (R-Z) slice of the whole model, radial slices at the heights that
matter, and zooms at every level of nesting down to the TRISO particle.

**Regenerated 2026-09-25 for the two-ball prism cell (gh:#309 step 2,
gh:#310).** The previous set showed the one-ball-per-tile bed, whose pebbles
interpenetrated along each column; that set is in the git history of this
folder, and "What changed" below says what it showed.

## How they were made

```bash
RAYON_NUM_THREADS=16 cargo run --release -p nee_soon --example htr10_geometry_images
```

- **Model:** `nee_soon::htr10_rmc::core_model::assemble_explicit_triso(14, 25, 0)`
  (the reported 14 rings x 25 half-layers case), on branch
  `claude/htr10-two-ball-cell` (from `develop` at `fe975296a2`), 2026-09-25.
  The geometry was **not** modified for these images.
- **Renderer:** `outram_mc_libs::geometry::plot` — the Rust port of OpenMC's
  slice plotter. Every pixel is one `Geometry::locate` at the pixel centre on
  the assembled geometry, with OpenMC's tie-break direction. On the 17 cases
  in `crates/outram-mc-libs/verification_and_validation/geometry_plotting/`
  it reproduces `openmc --plot` with 0 differing pixels.
- **Colours:** by material, a fixed palette (`palette()` in the example). The
  legend lists only materials present in that slice. "OUTSIDE MODEL" is a point
  in no cell.
- **Axes:** tick labels in cm, computed from the slice's own origin and width;
  the caption gives the coordinate of the cut plane.

Realised sizes printed by the run: 16 340 bed tiles, 287 cells, 35 universes
(root, TRISO particle, matrix, and all 32 five-ball fuel masks), bed radius
90.000 cm, bed half-height 61.2372 cm, hex pitch 6.6106 cm, tile height
9.7980 cm (one A-B layer pair, two balls per tile); model z from -319.419 to
290.581 cm, conus floor -98.183 cm, cavity top 160.581 cm. A kernel near the
axis locates at coordinate depth 3, lattices `[None, Some(0), Some(1)]`. The
pebble the zooms are centred on is at (22.8999, 0, 0) — an A-layer ball on a
tile FACE, so its lower and upper halves are drawn by two different tiles.

## What each image shows (inspected 2026-09-25)

| File | Cut | What is seen |
|---|---|---|
| `htr10_rz_full.png` | x-z, y = 0, 0.4 cm/px | Whole model. Bed x in [-90, 90], z in [-61.2, 61.2]; conus sloping from r = 90 at z = -61.2 to r = 25 at z = -98.2; discharge tube (homogenised dummy pebbles) r = 25 down to the model bottom at -319.4; empty helium cavity from the bed top to 160.6; 130 cm top reflector to 290.6; radial: reflector graphite, helium coolant annulus 140.6-148.6, reflector graphite to 167.8, boronated carbon to 190. The pebbles now form an **A-B zigzag of separate discs with helium between every pair**, not columns in contact. Discs at the bed top are cut by the bed plane, and discs of the layer above protrude into the bed from above (see "The bed top" below). No TRISO appears in this cut: y = 0 in each pebble frame falls between TRISO rows (centres at y = (k + 1/2) x 0.195 cm). |
| `htr10_xy_bed_mid.png` | x-y, z = 0 (an A layer), 0.4 cm/px | Hexagonal array of whole pebble discs at 6.61 cm pitch filling the r = 90 cm bed, cut by the wall, then the radial reflector zones. The isolated TRISO-coloured dots, in vertical stripes, are **aliasing**: at 0.4 cm/px against a 0.195 cm TRISO pitch the pixel centres beat against each pebble's particle lattice with a phase that depends only on the pebble's x, so whole columns light up together. Fuel/dummy cannot be read at this resolution; see the zooms. |
| `htr10_xy_b_layer.png` | x-y, z = 4.899 (a B layer), 40 cm square, 0.04 cm/px | **New.** The B layer in plan: a hex array of whole discs at 6.61 cm pitch, shifted by pitch/sqrt(3) = 3.82 cm in x from the A layer (a B centre at x = 26.72, y = 0 against the A pebble at 22.90). Each B ball is split across three tiles, and **every disc is either wholly TRISO-speckled or wholly plain graphite: no disc shows sectors of different kind**, which is what a per-tile identity would produce. |
| `htr10_xy_conus.png` | x-y, z = -79.71 (conus mid-height) | Pebble array inside the cone's circle (radius about 57 cm), reflector outside; edge pebbles cut by the cone. **No TRISO speckle at all**, in contrast to the aliased speckle in the bed slice at the same resolution: consistent with an all-dummy conus. |
| `htr10_xy_cavity.png` | x-y, z = 110.91 (cavity mid-height) | Helium disc r = 90 cm surrounded by the radial reflector zones; no pebbles. |
| `htr10_xy_bed_wall.png` | x-y, z = 0, 24 cm square at x = 84 | The bed wall at r = 90: pebbles are **truncated by the cylinder**, including their fuel zones, so **partial TRISO particles** exist at the wall. Balls centred just outside r = 90 (e.g. x about 91.6) contribute their inner piece, and can be fuelled: the 57:43 rule runs over every ball with volume in the bed, so the wall region is not dummy-biased. |
| `htr10_xz_conus_wall.png` | x-z, y = 0, 30 cm square on the cone | Pebbles in the A-B zigzag, truncated by the conus slope. |
| `htr10_xz_pebbles.png` | x-z, y = 0.0975 (a TRISO row), 30 cm square round the pebble at (22.90, 0, 0) | **The two-ball cell in section.** A columns at x = 11.45, 22.90, 34.35 (pebble centres at z = 0, +/-9.80), B balls at x = 15.27, 26.72, 38.17 (z = +/-4.90, +/-14.70): each layer sits in the hollows of the one below. **Every pebble is a whole 3.0 cm circle, with helium between every pair**, including the closest (A-B) pairs, whose centres are 6.21 cm apart. Fuel pebbles (TRISO speckle, whole fuel zone) and dummy pebbles (plain graphite) are mixed. Nothing is cut at the tile faces z = 0, +/-9.80 or at the B vertices. |
| `htr10_xz_one_pebble.png` | x-z, y = 0.0975, 7 cm square | The A pebble at (22.90, 0, 0): **a whole sphere, from z = -3.0 to +3.0**, not cut flat at +/-2.449 cm as the one-ball tile cut it. Its fuel zone (r = 2.5) holds a cubic TRISO array **continuous across z = 0**, which is the tile face: the lower half is drawn by one tile, the upper by the next, with one identity and one TRISO lattice translated to the ball centre. Neighbouring B pebbles appear at the upper and lower right corners, separated by helium. |
| `htr10_xy_one_pebble.png` | x-y, z = 0, 7 cm square | The same pebble in plan: a full 3.0 cm circle, the square TRISO array inside r = 2.5, and parts of its four in-plane A neighbours at the corners (centres 6.61 cm away, 0.61 cm of helium between). |
| `htr10_xz_bed_floor.png` | x-z, y = 0.0975, 30 cm square centred on the bed floor z = -61.24 | **New.** The per-ball conus rule at the floor: balls centred above the floor (the A layer at z = -58.79) may be fuelled — one here is, and it crosses the floor as a whole sphere, its fuel zone by 0.05 cm — while every ball centred below it (the conus B layer at -63.69 and below) is a dummy. |
| `htr10_xz_conus_pebbles.png` | x-z, y = 0.0975 (a TRISO row), 30 cm square at the conus mid-height (z = -79.71) | A-B zigzag of pebbles, **none carrying TRISO** — every conus pebble in this cut is a dummy, as Terry (2005) specifies. The same row cut through the bed (`htr10_xz_pebbles.png`) does show fuelled pebbles, so the absence here is not the cut missing the TRISO. |
| `htr10_xy_triso.png` | x-y, z = 0, 0.8 cm square round the particle at (20.46, 0.0975) | TRISO particles on a 0.195 cm square pitch: UO2 kernel (r = 0.025), buffer, IPyC, SiC and OPyC shells out to r = 0.0455, in matrix graphite. The left of the frame is past the fuel-zone edge (whole-particle rejection), so it is matrix only. |

### The bed top

The bed plane at z = +61.24 clips the top ball layer's upper 0.55 cm, and the
layer centred 2.449 cm above the plane (present in the lattice, fuel/dummy by
the same rule) puts its lower 0.55 cm inside the bed. That exchange keeps the
top slab at the bed's packing: the laterally averaged ball density is periodic
with period 4.899 cm and the bed is a whole number of periods. Visible at the
top edge of the bed in `htr10_rz_full.png`.

## What was checked, and what was not

**Checked, by eye against the constants in `core_model.rs` / `bed.rs` and the
sizes the run printed:** the axial stack (bottom reflector, discharge tube,
conus, bed, cavity, top reflector) and its z-coordinates; the radial zone
radii; the tile pitch (6.61 cm between in-plane centres), the A-B layer
spacing (4.90 cm) and lateral offset (3.82 cm); that **no pebble is cut at a
tile face or vertex** and that **helium separates every pair of pebbles**
(`htr10_xz_pebbles.png`, `htr10_xz_one_pebble.png`, `htr10_xy_one_pebble.png`);
that a pebble shared by two tiles (an A ball) and by three (a B ball) is drawn
whole with one identity and one continuous TRISO array
(`htr10_xz_one_pebble.png`, `htr10_xy_b_layer.png`); that the conus pebbles
are dummies and the floor rule is per ball (`htr10_xz_conus_pebbles.png`,
`htr10_xz_bed_floor.png`, `htr10_xy_conus.png`); the TRISO pitch and shell
sequence; kernel coordinate depth 3.

**Checked quantitatively elsewhere, not by eye:** no two built balls overlap
(minimum centre distance 6.2102 cm over all 29 445 balls of the 14 x 20
lattice) and every ball has one identity in all its tiles —
`htr10_rmc::tests::no_two_balls_of_the_built_bed_overlap` and
`every_piece_of_a_built_ball_has_one_identity`; the sampled volume fractions
(filling 0.6096-0.6097, fuel-ball 0.5693-0.5697, kernel 0.998-1.000 of the
paper-implied value) — `examples/htr10_fuel_fraction.rs`, recorded in
`crates/outram-mc-libs/verification_and_validation/htr10_rmc/README.md`.

**Seen, not tracked as a defect:** pebbles — and the TRISO particles inside
them — are truncated by the bed cylinder, the conus cone and the bed-top plane
(`htr10_xy_bed_wall.png`, `htr10_xz_conus_wall.png`, `htr10_rz_full.png`). A
lattice clipped by a region boundary behaves this way by construction; a real
bed has a wall-packing effect instead, which neither the old nor the new model
represents.

**Not checked:** atom densities; the control-rod band (not enabled in this
model, and no slice shows bored graphite); the fuel/dummy split at resolutions
where TRISO aliases (0.4 cm/px). Images cannot count particles.

## What changed (2026-09-25)

The previous images (the one-ball-per-tile bed, `develop` at `fe975296a2`)
showed pebbles **cut flat at z = +/-2.449 cm**, meeting their axial neighbours
on a 1.73 cm-radius waist with no helium between (gh:#309, #310). Both are gone:
the tile is the paper's two-ball prism and every pebble is whole. Two images
were added (`htr10_xy_b_layer.png`, `htr10_xz_bed_floor.png`) because the new
construction has two new things to check: B balls shared by three tiles, and a
per-ball rather than per-tile conus boundary. The zoom pebble moved from the
origin to (22.90, 0, 0): the search now reads the pebble centre as tile centre
+ fuel-zone translation, since a pebble is no longer at its tile's centre.
