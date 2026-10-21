# Tutorial 8 — The full Şeker bed of HTR-10: the tile stacked in a cylinder, cone and tube, whole balls only

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response
> ([`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md)).
> **Review status: first draft, 2026-10-18, AI-assisted, not yet reviewed by a
> human.** Built from
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@)
> on @@BUILD_DATE@@; every code link points at that commit.

> **Which k to quote.** None. The runs here are `MCSettings::Smoke`, which says
> of itself that it is not a result, with a sigma of about 500 pcm, one seed per
> height, **and a reflector that is deliberately simpler than the real one**.
> The full-statistics numbers are the V&V record's:
> `nee_soon/verification_and_validation/htr10_seker_2026_10_07_10k/`
> (10 000 x [5 + 135], the reference paper's own statistics).

[Tutorial 7](tutorial7-seker-layer.md) repeated one layer of Şeker & Çolak's
bed without end. The real bed ends: it sits in a cylinder that narrows through
a cone into a discharge tube, and **a ball that does not fit is not cut, it is
left out**. This page stacks the tile in the plane and in height inside that
cavity, counts what the rejection costs, and puts the result inside the core.

## The question

**How do I fit a lattice into a container that is not a box, without cutting a
single ball, and how many balls does that cost against Şeker's count?**

## The case

| what | value | source |
|---|---|---|
| tile | tutorial 7's: hexagonal prism, pitch 16.392 cm, height 9.798 cm, 13 balls | Şeker & Çolak (2003) pp.266-267 |
| cavity | cylinder r 90 cm; cone 36.946 cm down to a 25 cm tube; tube 221.236 cm to the bottom of the model; **the floor (top of the cone) is z = 0** | TECDOC-1382; Terry et al. (2005) Fig. 2; `nee_soon::htr10_rmc::core_model` |
| loading | `N` layers: the bed is `9.798 N + 6` cm, lowest ball's bottom to highest ball's top | Şeker p.267 |
| rejection | a ball is kept only if wholly inside the cavity; none is cut | Şeker p.267: *"If any ball intersects with the reflector surface, it is rejected"*, *"Balls intersect with cone or discharge tube surface are rejected"* |
| fuel : moderator | 0.57 : 0.43 of the balls above the floor; every ball in the cone and the tube is graphite | Şeker p.267; Terry (2005) s.2 |
| around the cavity | solid graphite (TECDOC Table 4-3 zone 22) to r 167.793 cm, boronated carbon brick (zone 17) to 190 cm, helium above the bed to z = 221.818 cm, 130 cm of graphite above that; 610 cm tall, vacuum outside | Terry (2005) Fig. 2 |
| temperature | 300.15 K | Li, Yu & Wei (2014) |

## What you write

```rust,ignore
// tutorial 7's tile, but each of its 13 balls is the fuel ball (tutorial 5's)
let tile = HexPrism::flat_to_flat(cm(pitch), cm(height)).filled_with(helium.clone())
    .containing(fuel_ball_at(site));               // 13 times, in a loop
let moderator = Sphere::radius(cm(3.0)).filled_with(graphite);
let bed = Lattice::hex_bed(tile, moderator, Ratio::new::<ratio>(0.57), cm(bed_top));

// the core cavity: a cylinder, a cone, a tube
let cavity = Hopper::new(cm(90.0), cm(221.818))
    .conical_floor(cm(36.946), cm(25.0))
    .outlet_tube(cm(221.236))
    .filled_with(helium)
    .containing_lattice(bed);

// from the inside out, one statement each
let reflector = Cylinder::radius(cm(167.793)).height(cm(610.0)).centred_at(cm(0.0), cm(0.0), cm(z_mid))
    .filled_with(graphite).containing(cavity);
let bricks = Cylinder::radius(cm(190.0)).height(cm(610.0)).centred_at(cm(0.0), cm(0.0), cm(z_mid))
    .filled_with(brick).containing(reflector);
model.add(bricks);
```

The statements are in `examples/common/seker_bed_model.rs`, shared by the
example and `tests/model_seker_bed.rs`.
[`Hopper`](../../code-review/#crates/outram-mc-libs/src/model/hopper.rs::Hopper),
[`Lattice::hex_bed`](../../code-review/#crates/outram-mc-libs/src/model/lattice.rs::Lattice::hex_bed),
[`BedCensus`](../../code-review/#crates/outram-mc-libs/src/model/bed.rs::BedCensus).

- **`Hopper`** lowers to a cylinder, a cone and planes; its inside is the union of
  the cylinder part, the cone part and the tube part, as `nee_soon`'s bed cells
  write it. Its one exact test is `contains_ball`: a sphere lies inside a solid of
  revolution exactly when its meridional disc lies inside the profile.
- **`Lattice::hex_bed`** is tutorial 7's tile repeated until it covers the hopper.
  A ball is kept only if wholly inside the hopper filled to `bed_top`. Where a
  ball is rejected its place is the tile's helium.
- The lowest plane of ball centres sits one radius above the floor, so the lowest
  balls rest on it (Şeker's layout).
- **Each kind of ball is lowered once**, to a universe of its own, and every site
  of every tile places that universe. 9 508 fuel balls share **one** TRISO lattice
  and **one** delta-tracked zone; the geometry is 28 600 cells and 1 494 universes
  (one tile universe per distinct state of its 23 pieces).

## Why a ball is rejected, not cut

A pebble cut by the wall is a smaller pebble: less carbon, a zone of fuel with
a flat face. The maintainer's rule (2026-10-01) is that this is wrong physics,
so the geometry never does it; the cost is that **fewer balls fit than Şeker's
count**, and the comparison with the published curves is made at equal ball
count (gh:#472), not at equal height.

At N = 12 (123.576 cm), from the census of the assembled model:

| | count |
|---|---|
| balls kept above the floor (Şeker's Table 3 counts these) | **16 681** (9 508 fuel, 7 173 moderator) |
| Şeker's Table 3, `1346 N + 733` | 16 885 |
| **difference** | **-204 (-1.21 %)**; the same -1.2 % at N = 10, 16 and 20 |
| balls kept in the cone and tube (all graphite) | 4 001 |
| balls **rejected** (centre inside, ball would cross a wall) | **1 899**: 1 080 at the side wall, 441 at the cone, 330 at the tube wall, 48 at the tube bottom, 0 at the top |
| per plane | 721 balls per basal plane, 609 per central plane (Şeker: 733 and 613) |
| height | 123.576 cm, exactly `9.798 N + 6` |
| filling fraction of the bed cylinder | 0.5999 (Şeker: "61 %" before rejection) |

`nee_soon`'s `seker_height_for_balls` note explains the 12 and 4 balls per plane:
they are the next shell out, which would cross the r = 90 cm wall by 0.21 cm and
0.08 cm. Şeker's count keeps them, so his model cuts them or keeps them by a
tolerance. We do neither. The top row never loses a ball: the highest plane of
balls just fits under the bed top.

## Look at it first: the pictures

`cargo run --release -p outram-mc-libs --example tutorial_seker_bed -- --geometry-only`
draws from the **assembled geometry** into `target/tutorials/seker_bed/`
(gitignored; the images are not committed). What I checked in each:

- `core_rz.png`: x-z at y = 0, the whole model. Reflector graphite to r = 168 cm,
  brown boronated brick to 190 cm, the cavity with the bed's top at z = 123.6 cm,
  helium above it to z = 221.8 cm, the cone from r = 90 to r = 25 cm between
  z = 0 and -36.9 cm, the tube to the model's bottom at z = -258 cm, where the
  strip of "outside model" is the vacuum. The black outline is the hopper's profile.
- `bed_rz.png`, `cone_tube_rz.png`: x-z zooms. Fuel balls are a purple zone in a
  grey shell, moderator balls plain grey, helium round them. **Every ball touches
  the black line or leaves a gap, none crosses it**; in the cone the balls follow
  the slope and stop short of it; in the tube a column of grey balls fills the
  middle with gaps at the wall. The title says 4 001 balls below the floor; the
  census agrees.
- `xy_bed_top.png`, `xy_bed_floor.png`: x-y on the top and bottom planes of ball
  centres (z = 120.576 and 3.000 cm, drawn 0.057 cm lower so that the plane cuts
  layers of TRISO particles). Both say 721 balls (411 fuel, 310 moderator at the
  top; 410 and 311 at the floor). The ring of helium along the wall is where the
  rejected balls would have been; the magenta circle is the r = 90 cm wall. I
  could not count 721 balls by eye: the check is the next section.
- `xy_cone.png` (z = -16.596 cm): 289 graphite balls inside the cone's circle
  (radius 60.8 cm there), none touching it. `xy_tube.png` (z = -94.98 cm): 49
  balls in the 25 cm tube, a gap at the right where balls would have crossed.
- `tile_xy.png`: one tile (the black hexagon, drawn for the reader: it is not a
  surface) at the central plane z = 7.899 cm. Title: 4 fuel + 2 moderator balls
  centred in it. In the picture: fuel at (0, 3.46), (-6, 3.46), (3, -1.73) and
  (0, -6.93); moderator at (-3, -1.73) and (6, 3.46). Matches.
- `fuel_ball_xy.png`, `triso_xy.png`: one fuel ball at (0, 3.464, 7.899) cm and one
  TRISO particle in it, as in tutorial 7: particles fill a disc inside the 2.5 cm
  zone, none crosses its edge; five layers in order.
- `rejected_xy_wall.png`: x-y at z = 61.788 cm, the quadrant x, y > 0. **Magenta
  rings are where a rejected ball would have been.** Title: 9 rings here of the 36
  rejected in the plane. Counted: 9 rings. Each ring sits on helium, touching the
  wall circle from inside, and the kept balls next to it do not overlap it.
- `rejected_rz_wall.png`, `rejected_rz_lower.png`: x-z at y = 0 with the cross
  section each rejected ball would have had in the slice (80 of the 1 899 cross the
  plane). Rings line up along the side wall every 9.8 cm (one per layer), along the
  cone, and down both sides of the tube. Every ring is on helium.
- `rejected_meridional.png`: **every** ball by radius and height. Not a geometry
  slice (it is drawn from the census): grey and blue dots are the kept balls, the
  coloured ones the rejected, by the wall nearest: red 1 080 at the side wall, orange
  441 at the cone, green 330 at the tube wall, blue 48 at the tube bottom. The
  legend counts are the census's; the picture is the same lattice seen from the side.

What I could not check from the images: the TRISO lattice particle by particle,
and which of two symmetric balls is the fuel one (the pictures show a pattern, not
a rule). The model is checked by counting, below.

<div class="predict">

**Predict.** The bed keeps every ball whole. Will it hold more or fewer balls than
Şeker's 16 885 at 123.576 cm, and by how much? Which wall rejects the most?
Check above.

</div>

## Counting from the geometry

| check | result (2026-10-18) |
|---|---|
| balls | the census walks the assembled universe graph and counts **9 508 fuel + 11 174 moderator = 20 682** balls, one place each however many tiles draw a piece of it; the bed census (from the lattice) says 16 681 + 4 001 = 20 682 |
| particles | **79 249 180** = 9 508 x 8 335, from the geometry's own packing rows |
| every kept ball is in the geometry | all 20 682 kept centres, located by the transport's own search, find a ball of their kind (9 508 fuel zones, 11 174 graphite shells; 0 wrong) |
| every rejected ball is not | all 1 899 rejected centres find helium |
| against `nee_soon` | `SekerBed::new` run for N = 3, 10, 12, 16 and 20: balls above the floor, fuel, rejected and kept in all **equal in all five** (e.g. N = 12: 16 681, 9 508, 1 899, 20 682); per plane, the number of balls and of fuel balls is equal in all 77 planes at N = 12 |
| hand-built | the beginner model on LOW-tier materials (a 30 cm hopper, 421 balls, 177 rejected) against a hand-built `HexLattice` CSG written from `nee_soon`'s formulas: **300 000 lookups identical**, and **k in every generation identical to the last bit**, multi-thread and single-thread |
| hand-built layout | an independent port of `SekerBed`'s layout in the test gives the same balls, rejected balls and kinds, ball for ball, at N = 3, 10, 12, 16, 20 |
| bed pool | the helium between the balls is one pool: the hopper's volume less the kept balls |

All pinned by `tests/model_seker_bed.rs` (5 tests).

**One difference from `nee_soon`, found by this check.** The fuel rule orders the
balls of a plane by their distance from the axis, then angle. Balls at the same
distance (the six of a flower, every mirror image) tie, and `nee_soon` breaks the tie by
the last bits of `x*x + y*y`; the model's positions have gone through `uom` and
carry different last bits, so the first comparison disagreed on 278 of 4 711
balls at N = 3. Here distances are compared on a 1e-6 cm² grid and ties go by angle
(as tutorial 7's rule does). The **number** of fuel balls, and the fuel count in each
plane, are `nee_soon`'s (77 of 77 planes at N = 12); which of two symmetric balls is the
fuel one differs for 20 to 31 % of balls (1 762 of 8 712 at N = 3, 6 352 of 20 682 at N = 12).
What that does to k is **not measured**.

## Tracking per region

The lattice (pebble boundaries) is **surface tracked**; each fuel zone is a
delta-tracked `DhRegion` (tutorial 5), with one majorant over the zone's six materials.
`nee_soon` delta-tracks the whole bed with one majorant over its materials; a
lattice-level override is not offered in the beginner API (it needs majorants the
beginner run does not carry, #926). Nothing is surface tracked through a TRISO particle.
Both choices are exact; they differ in speed (not compared here).

## Results

`MCSettings::Smoke` (1 000 per cycle, 100 inactive + 50 active, seed 1), ENDF/B-VIII.0,
300.15 K, 15 nuclides. Hardware for every time: **Intel Core i9-13900K, 4 logical
cores used** (`taskset -c 0-3`, `RAYON_NUM_THREADS=4`), 62.5 GiB RAM, Linux, CPU
only; the machine was shared with other jobs. The N = 12 run was first made on 10
threads (transport 96.2 s, data 72.7 s) and gives the same k (the run is the same on any
thread count).

| N | balls | built [cm] | reference at equal balls [cm] | **k (Smoke)** | record k (10 000 x [5+135]) | RMC | MCNP T3 | k - record [pcm] | sigmas |
|---|---|---|---|---|---|---|---|---|---|
| 10 | 14 021 | 103.980 | 102.728 | 0.95810 ± 0.00580 | 0.92514 ± 0.00108 | 0.93170 | 0.92516 | +3296 | +5.6 |
| 12 | 16 681 | 123.576 | 122.091 | 1.03643 ± 0.00565 | 0.99513 ± 0.00106 | 0.99942 | 0.99825 | +4130 | +7.2 |
| 16 | 22 001 | 162.767 | 160.817 | 1.11631 ± 0.00473 | 1.09513 ± 0.00110 | 1.09360 | 1.09936 | +2118 | +4.4 |
| 20 | 27 321 | 201.959 | 199.543 | 1.19770 ± 0.00484 | 1.16023 ± 0.00107 | 1.15861 | 1.16861 | +3747 | +7.6 |

- **SMOKE RUNS, not results to quote.** One seed per height. The Smoke run's own
  batch-means sigma is 0.0099 at N = 12 against 0.0057 naive, so the true scatter is
  larger than the table's.
- **The model is above the record by 2 100 to 4 100 pcm at every height**, and above
  RMC and MCNP at equal ball count. That is the expected sign (see the next table):
  the shell around the bed is simpler than the record's. At Smoke's sigma the excess
  at the four heights is the same within about 1.5 sigma; **a height trend in it is
  not resolved**.
- **The k-against-height slope is close to the record's:** k(16) - k(10) = +15 821 pcm
  against the record's +16 999 (-1 178 pcm, inside 2 sigma of Smoke); k(20) - k(10)
  = +23 960 against +23 509. The curve rises with the bed the way the published ones do.
- **Wall time** (4 threads): nuclear data 96.3 s once (15 nuclides, shared by all
  heights); transport 194, 204, 216 and 211 s. Peak memory 6.5 GB (not decomposed). The
  geometry census counted 79 million particle placements in 17.6 s (10 threads
  free) after the change described under "What changed".

## Predictions, and what happened

Posted on #943 before any transport run.

| prediction | outcome |
|---|---|
| lowering bit for bit, 300 000 lookups | **held** |
| k above the record at every height | **held** (+2118 to +4130 pcm) |
| excess +3500 ± 1500 pcm at N = 12 | **held**: +4130 ± 570 (sigma of the run) |
| excess larger at N = 10 than at N = 20 | **missed**: +3296 and +3747, not resolved either way |
| k(16) - k(10) within ±1500 pcm of +16 999 | **held**: +15 821 (-1 178) |
| nuclear data 80 to 200 s | **held**: 73 s on 10 threads, 96 s on 4 |
| Smoke transport 10 to 40 min | **missed**: 96 s on 10 threads, about 3.5 min on 4. (Surface tracking at the pebble boundary is not slower than I feared; I assumed `nee_soon`'s 4 267 to 8 984 s per point at 10 000 x 140 would scale with histories only.) |
| Smoke k at N = 12 within 3 sigma of record + 3500 | **held** (+4130, +1.1 sigma from it) |

## What changed in the library, and why

- **`Solid::Hopper`, `Lattice::hex_bed`, `BedCensus`** are new (above).
- **Z-planes at one height are one surface.** The reflector is two coaxial cylinders
  of one height and the tube ends where the model ends: two coincident planes a
  neutron on them could be lost between. `Lowering::zplane` reuses an identical
  plane. A test fails without it: two coaxial cylinders have two planes, not four,
  and the outermost's vacuum reaches them.
- **`set_bc` now reaches a cone** (it had no arm for it), so a hopper that is the
  outermost shape has its vacuum on the cone. The bit-for-bit test needs it.
- **The census is faster.** It looked every placement up in all 27 neighbouring grid
  cells, 341 s for the 79 million TRISO placements; it now looks up one (more only
  when a coordinate lies within 1 % of a grid line), and does not remember
  placements of a universe with a single owner (their placements are distinct: one
  per placement of the owner), which a hash of 180 million entries had to. 17 s, 6.5 GB
  peak instead of 10.8 GB. It assumes a universe holding a lattice is not placed
  twice at offsets that differ by a lattice vector (two overlapping copies). The
  random-packing census, where a particle really is reached by two paths, still passes.
- **The beginner plots have 12 colours**, not 8 (this model has 9 materials, and the
  brick took the kernel's colour).
- No transport code was changed.

## Every simplification

| simplification | kind | what it is worth |
|---|---|---|
| **Reflector: solid graphite and boronated brick**; none of the 20 coolant channels, the control-rod borings, the withdrawn rods, the Table 4-3 zone map (82 zones with their own densities) | **open** | each omission makes the reflector better than the real one. `nee_soon`'s own notes record that the explicit reflector moved its k by about -4 000 pcm from its simple one. The excess of +2 100 to +4 100 pcm here is the same sign and size, **but this is not isolated: no run removes the shell's other differences**, and the shell is tutorial 13's job. |
| Whole bed surface tracked between pebbles, delta in the fuel zones | too expensive to build now | exact either way; a lattice-level delta override needs majorants the beginner run lacks (#926). Speed not compared |
| Fuel rule ties broken by angle on a 1e-6 cm² grid | **open** | same number of fuel balls in every plane; which symmetric ball differs for 20 to 31 %; the effect on k is not measured |
| Regular TRISO lattice, 8 335 particles | open | tutorial 5 compared it with random packing, +765 ± 728 pcm, not resolved |
| Bed top is a plane at Şeker's height `9.798 N + 6` | no data | the experiment's first criticality was 123.06 cm with 16 890 balls; Şeker's N = 12 row holds 16 885 |
| Compared at equal ball count, not equal height | bounding | maintainer's choice on gh:#472: the whole-ball bed keeps 1.2 % fewer balls than Şeker's, which is 1.5 cm of bed at N = 12 |
| Tube open to vacuum at the model's bottom | no data | as in `nee_soon`'s model (TECDOC zones 6, 7, 81 hold balls to the model bottom) |
| Helium at 101.33 kPa | negligible, with an estimate | tutorial 7's estimate (0.06 % of graphite's absorption); the pressure is not in the papers (gh:#426) |
| Natural boron as the library's isotopes (B-10 0.199) | negligible, with an estimate | `nee_soon` uses 0.199 too; the difference is below the quoted digits |
| 300.15 K everywhere | no data | the papers' temperature |
| Smoke statistics, one seed per height | too expensive to do better here | sigma about 500 pcm against 105 pcm for the record's size; the comparison says only sign and order of magnitude |

## What we assumed

- That `nee_soon`'s `SekerBed` is Şeker's bed (medium confidence, gh:#472), and that its
  counts are the right reference for the hand-built layout (they are: the five counts
  above were run, not recalled).
- That the lowering is right because it matches a hand-built CSG, not because k
  agrees with a record. **Nothing was tuned and no input changed after seeing k.**

## Use, modify, create

**Use.** Run `--geometry-only`, open `rejected_xy_wall.png`, and count the rings against
the title. **Modify.** `--layers 10,12,16` runs three heights; `--fast` is the 5 000 x
120 smoke test. What does the census say about the rejection at the cone as the bed
grows (hint: nothing changes)? **Create.** Make a hopper of your own, a small cone,
and ask `assemble()` and `bed_census()` how many balls it keeps.

## Next question

The bed here is an ordered lattice. Tutorial 9 fills the same cavity from a DEM
pour (a random bed), reuses this tutorial's fuel and moderator balls, and runs both beds
in vacuum side by side.

## Sources

- `examples/tutorial_seker_bed.rs`, `examples/common/seker_bed_model.rs`;
  `tests/model_seker_bed.rs`; `src/model/{hopper,bed,lower_bed}.rs`.
- Şeker & Çolak (2003), pp. 266-267; `nee_soon::htr10_rmc::{bed, core_model, core_shell,
  reflector}` and `htr10_rmc::keff_vs_height`.
- The record: `nee_soon/verification_and_validation/htr10_seker_2026_10_07_10k/`
  (VIII.0). Published curves: Li, Yu & Wei (2014) Tables 3-4 (RMC); Şeker & Çolak
  Table 3 (MCNP, ENDF/B-VI).
- gh:[#943](https://github.com/theodoreOnzGit/outram-park-backend/issues/943),
  gh:[#926](https://github.com/theodoreOnzGit/outram-park-backend/issues/926),
  gh:[#472](https://github.com/theodoreOnzGit/outram-park-backend/issues/472).
