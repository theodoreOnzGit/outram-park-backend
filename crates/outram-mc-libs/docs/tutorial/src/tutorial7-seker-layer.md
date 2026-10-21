# Tutorial 7 — One Şeker layer of the HTR-10 bed: a hexagonal lattice of pebbles, stacked without end

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response
> ([`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md)).
> **Review status: first draft, 2026-10-18, AI-assisted, not yet reviewed by a
> human.** Built from
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@)
> on @@BUILD_DATE@@; every code link points at that commit.

> **Which k to quote.** The default run here is `MCSettings::Smoke`, a smoke
> test whose own note says it is not a result, and its sigma (about 500 pcm)
> is larger than every difference this page discusses. **Nothing on this page
> is a number to quote.** The comparison is a pilot of the crate's own, also
> not a record (see Results).

[Tutorial 5](pebble_api.md) built one fuel pebble. [Tutorial 6](tutorial6-lct008.md)
repeated pins on a rectangular grid. The HTR-10's bed is **many pebbles,
fuel and moderator, packed**, and Şeker & Çolak (2003) model the packing as a
**hexagonal lattice** of one 13-ball tile. This page builds one layer of it and
stacks it without end, so the answer is the bed's **k-infinity** at Şeker's
packing.

## The question

**How do I describe a packing whose balls cross the boundaries of the tile
that repeats, without cutting a ball, and how do I know the lattice I wrote is
the one the solver sees?**

## The case

Şeker & Çolak (2003), *Nucl. Eng. Des.* 222, 263-270, pp. 266-267, as
`nee_soon::htr10_rmc::bed::SekerCell::from_paper` reads it (gh:#472, medium
confidence: from the text and Fig. 3, nothing fitted). For 6 cm balls:

| what | value |
|---|---|
| tile | hexagonal prism, faces perpendicular to y, pitch 16.392 cm (apothem `d cos 30 + d/2` = 8.196 cm), height 9.798 cm (`2 d sqrt(2/3)`) |
| basal plane | a 7-ball "flower" on the bottom face, centred on it: each flower ball is a hemisphere in this layer and the other half of the *same* ball in the layer below |
| central plane | 3 `Inner` balls at `d/sqrt(3)` and 3 `Outer` balls at `2d/sqrt(3)`; each `Outer` crosses a side face by 1.73 cm |
| **13 balls per tile** | 7 + 6; the tile draws **pieces of 23** (10 come from the neighbours and the layer above) |
| fuel : moderator | the core's rule on the 13: **7 : 6** (0.538); the paper says 0.57 : 0.43 per layer |
| fuel ball | tutorial 5's: 8335 TRISO particles in a 2.5 cm zone, 0.5 cm graphite shell |
| moderator ball | graphite sphere (the shell's graphite) |
| helium | ideal gas at 101.33 kPa (an assumption, gh:#426), 300.15 K |

## What you write

```rust,ignore
let tile = HexPrism::flat_to_flat(cm(pitch), cm(height)).filled_with(helium.clone());
// one ball per site; a fuel ball is tutorial 5's pebble moved to its place
let tile = tile.containing(ball_at(site));      // 13 times, in a loop
let layer = HexPrism::flat_to_flat(cm(pitch), cm(height)).filled_with(helium)
    .containing_lattice(Lattice::hex(tile))
    .boundary(Boundary::Reflective);
model.add(layer);
```

(The loop over 13 sites is the one place this series puts several `.containing`
in one statement; the sites are listed in `examples/common/seker_layer_model.rs`,
shared by the example and the test that pins it.)
[`HexPrism`](../../code-review/#crates/outram-mc-libs/src/model/hex.rs::HexPrism),
[`Lattice::hex`](../../code-review/#crates/outram-mc-libs/src/model/lattice.rs::Lattice::hex).

**A ball may cross the tile's faces.** You list each ball once, in the tile
that holds its centre. The lattice shows the piece that sticks into a
neighbour in that neighbour too: it translates every ball by the six lattice
vectors and by plus and minus one tile height, and keeps the copies that
overlap the tile. Nothing is cut; the pieces are one ball. The model is refused
if pieces overlap, if a ball's centre is outside its tile, or if the tile is
not mirror-symmetric about its top and bottom faces.

**A cut pebble is wrong physics** (maintainer, 2026-10-01). The census counts
a ball once, whole, in the tile that holds its centre; the pieces and the
particles inside them are left out, so the volumes add up to the periodic
cell's inventory and a count of balls is a count of balls.

**The side condition.** Top and bottom are **reflective and exact**: the flower
balls are cut through their centres and the stacking repeats every layer, so
the layer is its own mirror image about both faces. The six side faces are
**reflective and not exact**: a reflected face shows a neutron the mirror
image of the tile where a periodic face would show the neighbour, and Şeker's
central plane is not mirror-symmetric across its faces. The transport has no
periodic boundary (gh:#259), so reflective it is; the error is **open** (see
"Every simplification").

## Look at it first: the pictures

`cargo run --release -p outram-mc-libs --example tutorial_seker_layer -- --geometry-only`
draws from the **assembled geometry** into `target/tutorials/seker_layer/`
(gitignored). What I checked in each, with the counts against their titles:

- `seker_xy_central.png`: x-y at z = -0.0566 cm, the central plane. Title says
  "6 balls own (4 fuel, 2 moderator), 3 enter from the neighbours"; counted in
  the picture: 3 whole balls (`Inner(0)` moderator at the top, `Inner(1)` and
  `Inner(2)` fuel below), 3 balls cut by the faces (`Outer(0)` fuel at the
  bottom, `Outer(1)` moderator at upper right, `Outer(2)` fuel at upper left)
  = 4 fuel + 2 moderator, and 3 pieces entering through the opposite faces
  (top: fuel, lower left: moderator, lower right: fuel). **The pairing is the
  check**: each cut ball has its twin on the opposite face and the same
  material (`Outer(0)` fuel with the top piece, `Outer(1)` moderator with the
  lower-left piece, `Outer(2)` fuel with the lower-right one). The hexagon has
  flat top and bottom (faces perpendicular to y), no ball overlaps another.
- `seker_xy_basal.png`: x-y at z = -4.7604 cm, just above the bottom face. Title
  "7-ball flower (3 fuel, 4 moderator)": counted 7 balls, 3 with fuel zones
  (upper left, right, lower left). Each flower ball is tangent to its
  neighbours and to the faces beside it, the top two touch the top face.
- `seker_xz_y0.png` and `seker_xz_inner.png`: x-z at y = 0 and at
  y = 3.4641 cm. Title says 7 fuel and 6 moderator balls in all. At y = 0 the
  flower balls appear as **hemispheres on both faces at the same x** (-6, 0 and
  6 cm: moderator, moderator, fuel on top and on the bottom), which is the
  stack repeating every layer; the two balls between are the `Inner` balls cut
  1.73 cm off their centres. At y = 3.46 the middle ball is `Inner(0)`, the two
  at the sides are `Outer(2)` (fuel) and `Outer(1)` (moderator) cut by the
  side faces. The windows are square, so the balls are round.
- `seker_fuel_ball_xy.png`: 7 cm round one fuel ball (`Inner(1)`, centre
  (-3.000, -1.732, 0) cm) at a TRISO layer. Checked: the particles fill a disc
  inside the 2.5 cm zone, none crosses its edge, the 0.5 cm shell is
  continuous, and its neighbours touch it at single points.
- `seker_triso_xy.png`: one particle in that ball, 0.12 cm window. Checked: five
  layers in order, kernel (blue), buffer (green), IPyC, SiC (purple), OPyC, in
  matrix graphite.

What I could not check from the images: the TRISO lattice particle by particle
(the test classifies 120 000+ random points against the 23-site table
instead), and the top and bottom faces' mirror images (they are the planes
themselves).

<div class="predict">

**Predict.** A layer of 7 fuel and 6 moderator balls in a reflective hexagon.
Will `k_infinity` be above or below the single pebble's 1.685, and above or
below the finite core's 1.16? Check below.

</div>

## Counting from the geometry

| check | result (2026-10-18) |
|---|---|
| balls | the census counts **7 fuel + 6 moderator = 13** from the assembled universe graph; 23 ball surfaces (13 owned + 10 pieces), 8 container planes |
| particles | **58 345** = 7 x 8335, from the packed zones' own counts; 7 packed zones in the census (the 5 pieces of fuel balls in the other tiles are not counted again), 12 delta-tracked zones in the geometry |
| packing | tile 2280.0601 cm³, helium 809.7947 cm³: ball fraction **0.644836**, which is 13 balls over the tile and Şeker's 0.6448 |
| lookups, hand-built | 300 000 random points of the prism: the same material in the beginner model and in a hand-built `HexLattice` CSG written from nee_soon's 23-site table |
| lookups, real model | 149 868 random points of the TRISO layer classified by the 23-site table alone (which ball, how far from its centre): all agree (30 030 in a fuel zone, 66 788 in graphite, 53 050 in helium) |
| per-generation k | 12 generations on the LOW tier: **identical to the last bit** with the hand-built CSG, multi-thread and single-thread |
| fuel rule | 7 : 6, with the 13 balls in the order the rule names (pinned by a test) |

The bit-for-bit result says the lowering adds and loses nothing against the
hand-built lattice, not that the model is right. All of it is pinned by
`tests/model_seker_layer.rs` (4 tests, no nuclear data processed).

## What went wrong first, and the fix

The first real-data run died: **k = 0.005 in generation 1 and the population
gone by generation 3**, with every picture and count right. The cause was not
the lattice. A fuel zone is *delta-tracked*, and `fly_delta_region` samples
virtual-collision sites along the whole flight through the zone. The flower
balls are cut through their centres by the reflective top and bottom, and the
balls that cross a side face are cut by it, so such a zone extends past the
model; a site there cannot be placed, and the flight scored the history as a
leak. Tutorial 5's pebble sits wholly inside its container, so no earlier model
had a delta zone cut by its boundary. A site past the nearest surface cannot
matter (a collision there is discarded), so it is now treated as void. A LOW-tier
test with the TRISO fuel ball fails without the change (it fails the
population) and passes with it. **This touches the transport**
(`physics/delta_tracking/handoff.rs`), only where the history used to be lost.

## Results

`MCSettings::Smoke` (1000 per cycle, 100 inactive + 50 active, seed 1),
ENDF/B-VIII.0, 300.15 K, 15 nuclides. Hardware for every time: **Intel Core
i9-13900K, 8 logical cores used** (`taskset -c 0-7`, `RAYON_NUM_THREADS=8`),
62.5 GiB RAM, Linux, CPU only; the machine was shared with other jobs.

| run | k | against the pilot 1.74844 ± 0.00309 |
|---|---|---|
| 7 fuel : 6 moderator (the rule) | **1.74096 ± 0.00496** (batch means 0.00565) | -748 pcm, -1.28 sigma |
| 8 : 5 (`--fuel-balls 8`) | 1.74716 ± 0.00518 (0.00543) | -128 pcm, -0.21 sigma |
| 8 : 5 minus 7 : 6 | +620 ± 717 pcm | not resolved (0.9 sigma) |

- **SMOKE RUN, not a result to quote**: sigma is about 500 pcm, and the run's
  own source check says "not converged" at this size (the low-population check
  passed at cycle 55, the production one never).
- **The comparison is a pilot**, not a record: nee_soon's `seker_cluster(1)`,
  the same tile in a reflective hexagonal prism one tile high, 2000 x [10 + 40],
  300.15 K, 4-core Xeon, 1.74844 ± 0.00309
  (`htr10_mgxs_cells/README.md` section 4). Its fuel inventory differs from
  this model's: 13 fuel and 10 moderator *pieces* (0.576 of the ball volume)
  against 7 and 6 *balls* (0.538), and its 23 sites are assigned
  independently, so a flower ball at the top need not match its twin at the
  bottom. Its pilot used different data processing, the 2026-10-13 code, and
  fewer histories.
- **k-infinity is above the lone pebble** (tutorial 5's 1.68517, +5579 pcm: the
  moderator balls help) **and far above the finite core** at 20 layers (1.16023,
  `htr10_seker_2026_10_07_10k`, +58 073 pcm: that core leaks, this stack does
  not). Both orderings are what the physics says.
- **The two splits bracket the paper's 0.57** (0.538 and 0.615) and agree within
  their sigma: the split is not what limits this comparison.
- **Wall time:** data **78.5 s** (79.1 s), transport **297.5 s** (326.4 s on
  8 : 5), on 8 threads.

## Predictions, and what happened

Posted on #943 before the first run of the real model.

| prediction | outcome |
|---|---|
| census 7 + 6 balls, 58 345 particles, 0.644836, 23 + 8 surfaces | **held** |
| lookups agree; per-generation k bit for bit | **held** (first time the test ran) |
| k within 1.755 ± 0.02 | **held**: 1.74096 |
| pilot within 2 sigma | **held** (-1.28 sigma); **but the predicted sign was wrong**: I said +500 pcm above the pilot from "less fuel, more moderation"; it is -748 pcm below, unresolved, and the 8 : 5 arm moved *up* by +620 ± 717 pcm, the other way from my slope. Not resolved either way. |
| above the lone pebble and the finite core | **held** |
| data processing 100 to 200 s | **missed**: 78.5 s (15 nuclides in parallel on 8 threads) |
| Smoke transport 120 to 900 s | **held**: 297.5 s |
| (not predicted) the population survives | **missed**, see "What went wrong first" |

## Every simplification

| simplification | kind | what it is worth |
|---|---|---|
| Reflective side faces, not periodic | **open** | A reflected face shows a neutron the tile's mirror image where Şeker's tile has a neighbour; the tile is not mirror-symmetric across its faces. nee_soon's superlattice cells give the only bound: from 1 to 19 tiles k moved -491 ± 450 pcm, from 7 to 19 tiles -11 ± 470 pcm (pilot sigma, not resolved); not measured here. Periodic needs gh:#259. |
| Top and bottom reflective | negligible, with an estimate | exact for the balls (the layer is its own mirror image, and `assemble()` checks it). Not exact for the TRISO particles in a flower hemisphere: the lattice offset 0.71 means the mirror image of the lower hemisphere is not the real upper one. It rearranges about 4000 particles per hemisphere without changing the count; the regular-against-random arrangement of a whole pebble moved k by +765 ± 728 pcm (tutorial 5, not resolved), a hemisphere is a fraction of that. |
| 7 : 6 fuel split, the paper says 0.57 | bounding | a tile of 13 cannot hold 0.57; 7 : 6 (0.538) and 8 : 5 (0.615) bracket it and differ by +620 ± 717 pcm, not resolved |
| One tile repeated: every layer and every tile identical | no data | the paper's bed is a 13-ball tile; it says nothing of different identities in different layers beyond the 0.57 : 0.43 per layer |
| Regular TRISO lattice, 8335 particles | open | tutorial 5 compared it with random packing, +765 ± 728 pcm, not resolved |
| Dummy ball = the shell's graphite (with boron) | no data | the papers do not give the dummy balls' composition separately; `htr10_rmc` makes the same choice |
| Helium at 101.33 kPa | negligible, with an estimate | He-3 absorption at 2.4e-5 atoms/(b cm) x 1.3e-6 x 5333 b is 1.7e-7 /cm, 0.06 % of graphite's absorption (about 2.8e-4 /cm); neither paper states the pressure (gh:#426) |
| 300.15 K | no data | the temperature of the papers and of the crate's HTR-10 records |
| Smoke statistics | too expensive to do better here | sigma 500 pcm against 85 pcm for the ten-times-larger record size; the page quotes no result |
| The pilot as the yardstick | no data | no record exists for this cell (its record cases were never run) |

## What we assumed

- That nee_soon's `SekerCell` is Şeker's cell (medium confidence, gh:#472).
- That an infinite stack of one layer is what tutorial 7 asks for (maintainer,
  #943).
- That agreement with a pilot checks the lowering and the inventory, not the
  physics; and that a delta-tracked region cut by a boundary is unbiased once
  sites past the surface are ignored (argued above; the 7 : 6 and 8 : 5 runs
  are the only measurement).
- Nothing was tuned, and no input was changed after seeing k.

## Use, modify, create

**Use.** Run `--geometry-only`, open `seker_xy_central.png`, and find each
`Outer` ball and the piece of its twin on the opposite face. **Modify.**
`--fuel-balls 8` makes 8 of the 13 fuel; what does the census say, and which
way does k move? **Create.** Make a tile of your own, a hexagon with one ball
that crosses a face, and ask `assemble()` what it thinks.

## Next question

One layer stacked without end is not a reactor. Tutorial 8 stacks Şeker's tile
in the plane and in height inside HTR-10's core, rejects the balls that would
cross the wall (a ball is never cut), and compares k with the published
curves against loading height.

## Sources

- `examples/tutorial_seker_layer.rs`, `examples/common/seker_layer_model.rs`;
  `tests/model_seker_layer.rs`.
- Şeker & Çolak (2003), pp. 266-267; `nee_soon::htr10_rmc::bed` (`SekerCell`,
  `SekerSite`, `SekerBed::assign`).
- The pilot: `nee_soon/verification_and_validation/htr10_mgxs_cells/README.md`
  section 4 (`Htr10Cell::seker_cluster`).
- The finite-core curve: `htr10_seker_2026_10_07_10k`.
- gh:[#943](https://github.com/theodoreOnzGit/outram-park-backend/issues/943)
  (decisions and progress), gh:[#926](https://github.com/theodoreOnzGit/outram-park-backend/issues/926),
  gh:[#259](https://github.com/theodoreOnzGit/outram-park-backend/issues/259) (periodic boundary).
