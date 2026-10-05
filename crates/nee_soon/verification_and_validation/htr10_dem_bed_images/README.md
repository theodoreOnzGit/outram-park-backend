# HTR-10 core on a DEM-settled bed — geometry drawings (2026-10-05)

The geometry-drawing HARD RULE of `crates/nee_soon/CLAUDE.md`, applied to
`htr10_rmc::explicit_bed::assemble_explicit_triso_from_centres`. AI-drafted;
awaiting human review.

```bash
taskset -c 0-11 cargo run --release -p nee_soon --example htr10_dem_bed_images
```

## Input

`reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv`: 27 554
pebbles settled by this workspace's LIGGGHTS port into the published conus and
a 25 cm discharge tube (provenance: `reference-data/liggghts/README.md`).
Trimmed by height: the 2 457 pebbles centred at or below the bed floor (conus
and tube) and the lowest 16 890 above it (the first-criticality ball count,
IAEA-TECDOC-1382 p. 251). The top is therefore a flat cut through a random
packing, not a poured surface. Identity: `paper_fuel_assignment`, 57:43 above
the floor (9 627 fuel of 16 890 = 0.5700), the conus and tube all dummy.

## Built model (from the example's log)

| quantity | value |
|---|---|
| DEM balls / tube filler balls | 19 347 / 1 721 |
| core balls (centre above floor) | 16 890 |
| bed half-height (highest ball top / 2) | 63.278 cm; p99 surface 125.300 cm above the floor |
| frame | `z_MC = 100 z_DEM - 63.278` cm |
| overlapping pairs split by bisector | 52 356, max overlap 0.04723 cm (1.574 % of r) |
| lens volume fraction (counted once) | 1.27e-5 of the ball volume |
| max wall penetration (clipped by the envelope) | 0.0317 cm |
| DEM column bottom / filler gap | 25.020 cm below the conus floor / 0.620 cm helium |
| tiles / with balls / cells / universes / surfaces | 22 427 / 2 280 / 88 693 / 2 285 / 175 264 |
| assembly time | 0.2 s |
| point check (200 000 random container points) | 0 not located, 0 claimed by two cells |

Every image was rendered with OpenMC's overlap check on (`showing_overlaps`):
**0 overlap pixels in every image.**

## Images

| file | what it shows |
|---|---|
| `dem_rz_full.png` | x-z through the axis, whole 610 cm model |
| `dem_rz_bed.png` | x-z, bed, conus and the tube top at 0.1 cm/px |
| `dem_xy_bed_mid.png` | x-y at half the loading height |
| `dem_xy_conus.png` | x-y through the conus |
| `dem_xy_bed_wall.png` | x-y, 24 cm square at the r = 90 cm wall |
| `dem_xz_conus_wall.png` | x-z, 30 cm square on the conus slope |
| `dem_xy_tube_dem.png`, `dem_xy_tube_filler.png` | x-y through the tube: the DEM balls, then Şeker's filler |
| `dem_xz_tube_junction.png` | x-z at the DEM/filler junction |
| `dem_xz_bed_top.png` | x-z, the top of the trimmed bed |
| `dem_xz_pebbles.png`, `dem_xz_one_pebble.png` | x-z round one fuel pebble near the axis, through a TRISO row |
| `dem_xy_triso.png` | x-y, 0.8 cm round one TRISO particle |
| `dem_lens_cells.png` | the most-overlapping pair in the bed (0.0425 cm), coloured by bed-tile CELL |

## What was checked in them

- The whole model: the bed sits on the conus, the tube runs to the model
  bottom, the cavity, reflector zones, borings and boronated bricks are those
  of `htr10_geometry_images` (the shared `core_shell` code).
- Pebbles are whole spheres at random positions; none is cut by the bed top,
  the side wall or the cone at the drawn resolution; the helium between them
  is continuous; fuel pebbles show the TRISO lattice and dummy pebbles none;
  no fuel below the bed floor (conus and tube all graphite).
- The tube: DEM balls above, an ordered Şeker stack below, a visible
  ~0.6 cm helium gap between them, nothing overlapping.
- The TRISO particle has its five layers inside the matrix.
- The lens: the two balls' cells (a fuel shell and a dummy) meet on a straight
  line, the bisector, with helium wedges at the lens rim; a tile face crosses
  the left of the frame. No red overlap pixel.

## What could not be checked here

- The 0.03 cm wall caps and the 0.05 cm lenses are below the resolution of
  every image but the lens zoom; they are checked by the numbers above and by
  the tests in `src/htr10_rmc/explicit_bed_tests.rs`, not by eye.
- Whether the DEM bed is physical (packing, friction, wall effects) is the
  LIGGGHTS port's claim, not this drawing's.
- The tube filler below the DEM column is the default model's ordered tube,
  not a pour; the 0.62 cm gap and the 25 cm junction are artefacts of a short
  DEM tube.

## Smoke k_eff (NOT a V&V result)

`--keff`, 2026-10-05: ENDF/B-VIII.0 with the correct-physics default data,
500 particles x [10 inactive + 20 active], seed 20 260 917, 12 threads,
i9-13900K pinned to CPUs 0-11: **k = 0.98405 +/- 0.01104** (within-run 1σ,
one seed); 15 000 histories, **0 lost**; Shannon entropy 5.41 -> 5.42 bits
(ceiling 6); transport 38.3 s. The 16 890 balls match Şeker's model at
123.61 cm, where RMC reads 1.00439.

This shows the DEM-built core transports without losing particles. It is
**not** a comparison: 10 inactive cycles do not converge the source, the
1σ is 1100 pcm, and the bed (random packing, top cut flat, ordered tube
filler) is not the reference's model. No conclusion about k on a DEM bed
should be drawn from it.
