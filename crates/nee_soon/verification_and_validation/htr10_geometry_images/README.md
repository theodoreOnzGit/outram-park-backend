# HTR-10 explicit-TRISO core — drawn from the assembled geometry

Images required by the geometry-drawing HARD RULE in `crates/nee_soon/CLAUDE.md`:
an axial (R-Z) slice of the whole model, radial slices at the heights that
matter, and zooms at every level of nesting down to the TRISO particle.

## How they were made

```bash
RAYON_NUM_THREADS=8 cargo run --release -p nee_soon --example htr10_geometry_images
```

- **Model:** `nee_soon::htr10_rmc::core_model::assemble_explicit_triso(14, 25, 0)`
  (the reported 14 rings x 25 layers case), as of branch `develop` on
  2026-09-25, after the gh:#316 TRISO-count fix. The geometry was **not**
  modified for these images.
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

Realised sizes printed by the run: 33 497 bed tiles, 20 cells, 5 universes,
bed radius 90.000 cm, bed half-height 61.2372 cm, hex pitch 6.6086 cm, tile
height 4.8990 cm; model z from -319.419 to 290.581 cm, conus floor -98.183 cm,
cavity top 160.581 cm.

## What each image shows (inspected 2026-09-25)

| File | Cut | What is seen |
|---|---|---|
| `htr10_rz_full.png` | x-z, y = 0, 0.4 cm/px | Whole model. Bed x in [-90, 90], z in [-61.2, 61.2]; conus sloping from r = 90 at z = -61.2 to r = 25 at z = -98.2; discharge tube (homogenised dummy pebbles) r = 25 down to the model bottom at -319.4; empty helium cavity from the bed top to 160.6; 130 cm top reflector to 290.6; radial: reflector graphite, helium coolant annulus 140.6-148.6, reflector graphite to 167.8, boronated carbon to 190. Pebbles appear as columns of discs **in contact along z** with no helium between axially adjacent pebbles (gh:#310). **No TRISO appears in this cut**: y = 0 in each pebble frame falls *between* TRISO rows (particle centres sit at y = (k + 1/2) x 0.195 cm), so the fuel zones read as bare matrix graphite. That is the geometry, not a missing fuel zone — see the zooms, which are cut through a TRISO row. |
| `htr10_xy_bed_mid.png` | x-y, z = 0, 0.4 cm/px | Hexagonal pebble array filling the r = 90 cm bed, then the radial reflector zones as above. The isolated TRISO-coloured dots are **aliasing**: at 0.4 cm/px against a 0.195 cm TRISO pitch the pixel centres beat against the particle lattice. Pebbles on the bed wall are cut by the cylinder. |
| `htr10_xy_conus.png` | x-y, z = -79.71 (conus mid-height) | Pebble array inside a circle of radius about 57 cm (the cone at that height), reflector outside; edge pebbles cut by the cone. |
| `htr10_xy_cavity.png` | x-y, z = 110.91 (cavity mid-height) | Helium disc r = 90 cm surrounded by the radial reflector zones; no pebbles. |
| `htr10_xy_bed_wall.png` | x-y, z = 0, 24 cm square at x = 84 | The bed wall at r = 90: pebbles whose tiles straddle it are **truncated by the cylinder**, including their fuel zones, so **partial TRISO particles** exist at the wall (visible in the pebble at x about 91, y = 0). |
| `htr10_xz_conus_wall.png` | x-z, y = 0, 30 cm square on the cone | The same truncation against the conus slope: pebbles cut by the cone surface. |
| `htr10_xz_pebbles.png` | x-z, y = 0.0975 (a TRISO row), 30 cm square round the pebble at the origin | Three pebble columns (x = -11.4, 0, +11.4; the columns between them are offset by half a pitch in y and not cut). Within each column the pebbles **touch and interpenetrate**: consecutive pebbles meet on a waist, with no helium gap (gh:#310). Fuel pebbles (TRISO speckle) and dummy pebbles (plain graphite) alternate irregularly, as the 57:43 split assigns them. |
| `htr10_xz_one_pebble.png` | x-z, y = 0.0975, 7 cm square | The pebble at the origin is **cut flat at z = +/-2.449 cm** (half the 4.899 cm tile height) where the neighbouring pebble takes over, meeting it on a waist of radius about 1.73 cm — the sphere-sphere intersection circle for centres 4.899 cm apart, i.e. the clip of gh:#309 and the contact of gh:#310, in one picture. The fuel zone (r = 2.5) holds a cubic TRISO array whose outermost particles sit at about +/-2.44 cm; the neighbour's TRISO rows begin just above the waist. |
| `htr10_xy_one_pebble.png` | x-y, z = 0, 7 cm square | The same pebble in plan: a full 3.0 cm circle (the x-y cut is not clipped), the square TRISO array inside r = 2.5, and parts of four hex neighbours at the corners (two fuelled, two dummy). |
| `htr10_xz_conus_pebbles.png` | x-z, y = 0.0975 (a TRISO row), 30 cm square at the conus mid-height (z = -79.71) | Three columns of pebbles, 21 cut, **none carrying TRISO** — every conus pebble in this cut is a dummy, as Terry (2005) specifies and as the op-5n34 correction recorded in `crates/outram-mc-libs/verification_and_validation/htr10_rmc/README.md` requires. (The same row cut through the bed, `htr10_xz_pebbles.png`, does show fuelled pebbles, so the absence here is not the cut missing the TRISO.) |
| `htr10_xy_triso.png` | x-y, z = 0, 0.8 cm square round the particle at (0.0975, 0.0975) | TRISO particles on a 0.195 cm square pitch: UO2 kernel (r = 0.025), buffer, IPyC, SiC and OPyC shells out to r = 0.0455, in matrix graphite. |

## What was checked, and what was not

**Checked, by eye against the constants in `core_model.rs` and the realised
sizes the run printed:** the axial stack (bottom reflector, discharge tube,
conus, bed, cavity, top reflector) and its z-coordinates; the radial zone
radii; the bed tile pitch and height; the TRISO pitch and shell sequence; that
the pebble at the origin is a fuel pebble at coordinate depth 3; the conus
pebbles cut by a TRISO row are all dummies.

**Seen, and already tracked:** gh:#309 (pebble shells clipped at the tile
faces) and gh:#310 (axially adjacent pebbles touching / interpenetrating) are
both directly visible in `htr10_xz_one_pebble.png` and `htr10_xz_pebbles.png`.
gh:#316 (TRISO count) is fixed; the images do not by themselves count
particles.

**Seen, not tracked as a defect:** pebbles — and the TRISO particles inside
them — are truncated by the bed cylinder and by the conus cone
(`htr10_xy_bed_wall.png`, `htr10_xz_conus_wall.png`). A lattice clipped by a
region boundary behaves this way by construction. Whether it matters (it
changes the heavy-metal and carbon inventory at the wall by an amount that has
not been measured here) is the maintainer's call.

**Not checked:** anything the eigenvalue depends on quantitatively — volumes,
atom densities, the 57:43 fuel/dummy split, the control-rod band (it is not
enabled in this model, and no slice shows the bored-graphite material).
Images at 0.4 cm/px cannot resolve TRISO; only the zooms can.
