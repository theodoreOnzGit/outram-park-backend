# Rotational periodic boundary conditions (GitHub #259)

Port of OpenMC's `RotationalPeriodicBC` (`src/boundary_condition.cpp:152-262`,
pairing in `src/surface.cpp:1411-1470`, OpenMC `8f79d9d`) as
`geometry::crossing::{periodic_transform, PeriodicTransform}`; a pair is
declared with `BoundaryType::PeriodicWith(partner)`. Parallel planes give the
translational condition, planes through the origin that share a coordinate axis
give the rotational one. Research and V&V only (`RESPONSIBLE_USE.md`).

## Methodology

1. **Single-particle track tests**
   (`src/geometry/crossing/rotational_periodic_tests.rs`), reference = plane
   geometry only: the angle equals the wedge angle (90, 60, 45, 30 degrees, both
   directions) to within 4e-16 rad; a hand-rotated 90 degree crossing; a 60
   degree wedge with a general plane (radius, z, polar angle and direction
   rotated together, particle inside the cell afterwards); there and back is the
   identity; 200 random lines in each of the 90/60/30 degree wedges, folded
   through all their crossings, keep `|r|`, the folded polar angle and `u . r`
   (1e-6 cm, two 1e-9 cm nudges per crossing); malformed pairs are refused by
   name.
2. **Quarter against full** (`tests/rotational_periodic_quarter_vs_full.rs`): a
   bare HEU cylinder (Godiva composition, CORE data), R = 7 cm, z in +/-7 cm,
   vacuum outside. Full cylinder against the x >= 0, y >= 0 wedge closed by a 90
   degree periodic pair; control: the wedge with vacuum on its two flat faces.
   1000 histories, 10 inactive + 30 active, `run_keff_csg_seq`, seeds 1..N.
   **Prediction written before the first run: quarter - full within 1 % of k on
   the N-seed mean; control more than 5 % below.** The gate is not tuned.

## Results (2026-11-14, release, 4-core shared container, CPU only, hardware not otherwise recorded)

| N seeds | full | quarter periodic | quarter - full | vacuum control |
|---|---|---|---|---|
| 8 | 0.91486 +/- 0.00287 | 0.91570 +/- 0.00256 | +85 pcm (+0.22 sigma, +0.09 %) | 0.54080 +/- 0.00116 (-40.9 %) |
| 64 | 0.91762 +/- 0.00073 | 0.91680 +/- 0.00086 | -83 pcm (-0.73 sigma, -0.09 %) | 0.54098 +/- 0.00043 (-41.1 %) |

(+/- is the standard error over seeds.) The prediction held, with 10x margin on
the 1 % gate. The two differences have opposite sign and are each inside one
sigma: no bias is resolved. The control shows the test can fail: dropping the
periodic pair loses 41 % of k.

**Not done:** a comparison with OpenMC itself on a lattice without mirror
symmetry (the #259 acceptance item). This test only shows the rotation is
self-consistent against the full model; it does not distinguish a rotation from
a reflection on a mirror-symmetric problem, which is what the single-particle
tests do instead.

## Pictures (assembled geometry, `render_material_slice`)

`full_xy_z0.png`, `quarter_xy_z0.png`: x-y slices at z = 0; the wedge is the
x >= 0, y >= 0 quarter of the disc, radius 7 cm (checked against the axes).
`full_xz_y3.png`, `quarter_xz_y3.png`: x-z slices at y = 3 cm; the full model's
half-width there is sqrt(49 - 9) = 6.32 cm, matching the picture; the quarter
slice is x >= 0 only. The plotter does not mark boundary conditions, so the
periodic faces are not visible; that is checked by the track tests.
Regenerate with `OUTRAM_PLOT_OUT=<dir> cargo test --release -p outram-mc-libs
--test rotational_periodic_quarter_vs_full`.

## A defect found on the way

The first set of pictures drew a half-plane, not a cylinder. The region had
been written as bare half-space tokens without `Intersection`, and
`Cell::contains` tests only the **last** token of a stream. Transport did not
notice (k is identical to the printed digits before and after the fix), because tracking takes
distances to the cell's surfaces and `locate` trusts the recorded side after a
crossing. The same omission was in the box of `periodic_tests.rs`
(committed 2026-11-14, `61cf9cbda`); both are fixed. Any other hand-built region
without operators is drawn wrong by the plotter and is worth a look.
