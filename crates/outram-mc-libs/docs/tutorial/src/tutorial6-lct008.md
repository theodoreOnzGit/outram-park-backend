# Tutorial 6 — LEU-COMP-THERM-008: a rectangular pin lattice, and a fast way to run it

> **Research, education and V&V only.** Not for reactor operation, licensing,
> safety decisions or emergency response
> ([`RESPONSIBLE_USE.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/RESPONSIBLE_USE.md)).
> **Review status: first draft, 2026-10-18, AI-assisted, not yet reviewed by a
> human.** Built from
> [`@@COMMIT_SHORT@@`](https://github.com/theodoreOnzGit/outram-park-backend/commit/@@COMMIT@@)
> on @@BUILD_DATE@@; every code link points at that commit.

> **Which k to quote.** The default run here is `MCSettings::Smoke`, and
> `--fast` is `MCSettings::Fast`: smoke tests, whose own notes say they are
> not results. The number to compare with the benchmark is the crate's
> record (32 seeds of 10 000 × [40 + 120], route 4): **1.00135 ± 0.00023**.
> A run on this page is judged against that record, not against 1.0000.

[Tutorial 4](tutorial4-triso-particle.md) repeated one TRISO particle on a
grid. This is the lattice of a light-water reactor: **4961 fuel pins in 22
different assemblies of a 7 × 7 core**, cut by a cylinder. Rung 4 of the
older ladder ([LEU-COMP-THERM-008](lct008.md)) tells the physics story of the
same benchmark; this page is about describing it, drawing it, and running it
*fast*.

## The question

**How do I describe 4961 pins without writing 4961 things, how do I know the
lattice I wrote is the one the solver sees, and how do I run a model with 35
nuclides in seconds rather than minutes?**

## The case

ICSBEP **LEU-COMP-THERM-008 case 1** (B&W critical lattices, Core XI), as the
OpenMC model in `mit-crpg/benchmarks` (Paul Romano, MIT licence) gives it,
committed under `verification_and_validation/icsbep/leu-comp-therm-008/`:

| level | what | numbers |
|---|---|---|
| pin | UO2 fuel rod, Al-6061 clad rod, water | radii 0.514858 and 0.602996 cm, pitch 1.63576 cm |
| assembly | 15 × 15 pin tiles, 22 different patterns | pitch 24.5364 cm |
| core | 7 × 7 assembly tiles; 37 hold pins, 12 are water | cut by a cylinder of radius 76.2 cm, height 163.324 cm, vacuum outside |

3 materials, 35 nuclides, 2.459 w/o fuel, 1511 ppm boron in the water, 293.6 K
everywhere (the cards give no temperature, so OpenMC's default applies). The
atom densities, the 22 pin maps and the core map are **read from the cards**,
not retyped, so a transcription error cannot enter.

**The reference.** The benchmark is critical, `k = 1.0000`. Its uncertainty
is *not* in the repository; the crate uses "about 600 pcm" as a stand-in (a
pessimistic ICSBEP band for lattices, not a quoted figure,
`examples/lct008_keff.rs`).

## What you write

A pin is a **rod** in a **rod** in a water box. A rod is a `Cylinder` with no
height: it runs through whatever holds it, which for a pin is its lattice tile.

```rust,ignore
let fuel_rod = Cylinder::radius(cm(R_FUEL)).filled_with(fuel).in_component(fuel_c);
let clad_rod = Cylinder::radius(cm(R_CLAD)).filled_with(clad)
    .in_component(clad_c).containing(fuel_rod);
let fuel_pin  = Cuboid::sides(cm(pitch), cm(pitch), height).filled_with(water.clone())
    .in_component(water_c).containing(clad_rod);
let water_pin = Cuboid::sides(cm(pitch), cm(pitch), height).filled_with(water.clone())
    .in_component(water_c);
```

One `.containing` per statement, inside out, as in tutorials 3 and 4. An
**assembly** is a water box filled with a lattice of pin tiles, and the
**core** is a finite cylinder filled with a lattice of assembly tiles. A tile
may itself hold a lattice:

```rust,ignore
let pins = Lattice::from_rows_top_down(vec![water_pin, fuel_pin], pin_rows);
let assembly = Cuboid::sides(cm(24.5364), cm(24.5364), height).filled_with(water.clone())
    .containing_lattice(pins);
let core_lattice = Lattice::from_rows_top_down(assembly_tiles, core_rows);
let core = Cylinder::radius(cm(76.2)).height(height).filled_with(water)
    .containing_lattice(core_lattice);
model.add(core);                       // vacuum outside: the default
```

The statements are in `examples/common/lct008_tutorial_model.rs`, shared by
the example and the test that pins it
([`Cylinder`](../../code-review/#crates/outram-mc-libs/src/model/shape.rs::Cylinder),
[`Shape::containing_lattice`](../../code-review/#crates/outram-mc-libs/src/model/shape.rs::Shape::containing_lattice),
[`Lattice::from_rows_top_down`](../../code-review/#crates/outram-mc-libs/src/model/lattice.rs::Lattice::from_rows_top_down)).

**A cylinder cuts a lattice.** The core's grid is 171.76 cm wide and the
cylinder 152.4 cm, so the wall cuts the water tiles at the corners; the
lattice must *cover* the cylinder (otherwise part of it holds no tile, and
assembling refuses it with a picture). Only water is cut: all 4961 fuel pins
lie wholly inside r = 76.2 cm (checked from the cards).

**One layer is extruded.** The CSG lattice ignores z when it has one layer, so
its tiles run through the core's whole height. The tile height you give is
then only a label, and the API requires tiles at least as tall as the
container, with the container centred at z = 0. (Writing this page turned up
that a one-layer lattice *shorter* than its box used to be accepted and filled
the whole box, against the doc's promise that the box's material fills what
the lattice does not cover. It is now refused, with a unit test.)

### The trap: which row is the top row

OpenMC's cards list a lattice's **top row first**; outram's CSG lattice starts
at the **lower-left**. The API makes you choose by the constructor's name:
`from_rows_top_down` is the cards' order, `from_natural_order` the other. The
V&V port needed a `reverse_rows` flag for exactly this.

The example builds two wrong models and compares them with the right one at
100 000 random points of the midplane:

| mistake | fuel pins counted | points where the material differs |
|---|---|---|
| every lattice read bottom-row-first | 4961 | **0** |
| only the core's lattice read bottom-row-first | 4961 | **5066** (5.1 %) |

Read that twice. **The census cannot see the mistake**: the pins are all there.
And the first mistake is harmless *here*: reading every lattice upside down
mirrors the whole core in y, and this core is its own mirror image, so nothing
changes (the V&V file reasoned the same, `lct008_model.rs`). The second is
what a half-converted input looks like: assemblies at mirrored places but not
themselves mirrored. Only the picture shows it
(`lct008_xy_wrong_order.png`). This is why the pictures come first.

## Look at it first: the pictures

`cargo run --release -p outram-mc-libs --example tutorial_lct008 -- --geometry-only`
draws from the **assembled geometry** (the cell the solver finds at each
pixel, by the ported OpenMC plotter, not from the constants) and stops before
any data are processed. Files in `target/tutorials/lct008/` (gitignored):

- `lct008_xy.png`: the whole core, x-y through the midplane. Checked: a round
  cylinder of water, a stepped fuel region (a stepped outline, the 37
  pin-holding assemblies), symmetric left to right and top to bottom.
- `lct008_xz.png`: the whole core, x-z through y = 0. Checked: the rods run
  the full height, the cylinder's flat top and bottom at ±81.662 cm, the water
  ring outside the pins.
- `lct008_assembly_xy.png`: the centre assembly, 15 × 15 pins.
- `lct008_pin_xy.png`: one pin and its neighbours. Checked: fuel (blue), clad
  (green) and water, clad thickness 0.088 cm, centre pin at the origin.
- `lct008_wall_xy.png`: the cylinder's wall cutting the water beside a corner
  assembly. Checked: the wall cuts only water, no pin.
- `lct008_xy_wrong_order.png`: the trap above, staggered assemblies.

What I could not check from the images: the pin maps pin by pin (the test
compares 200 000 lookups with the hand-built model instead), and pins under
the legend scale in the whole-core picture (a pin is 3 pixels wide there).

<div class="predict">

**Predict.** The core is a vacuum-bounded cylinder 1.6 m tall. Will `k` be
above or below 1.0 for a run that is only a smoke test, and by how much can a
single Fast run be trusted? Check against the table below.

</div>

## Counting from the geometry

| check | result (2026-10-18) |
|---|---|
| fuel pins | the census counts **4961** copies of the fuel cell from the assembled universe graph, the benchmark's number; 37 assemblies, 22 distinct patterns |
| fuel volume | 4961 × π r² × 163.324 cm = 674 751.8 cm³, equal to 1e-12 relative |
| lookups | 200 000 random points: the same material (by name) in the beginner model and the hand-built one |
| lattice grids | core lattice: same size, pitch and lower-left corner **to the bit** |
| per-generation k | 8 generations on the LOW tier (20 of 35 nuclides): **identical to the last bit**, multi-thread and single-thread |
| atom densities | 22 (nuclide, density) pairs: 17 bit-identical, **5 one ulp off** (O-16 in the water and in the fuel, Si-30, Cr-53 and Fe-54 in the clad). `AtomDensity` stores per m³ and the conversion is not one-to-one on `f64`; tutorial 3 met the same. The k values still agree to the bit: a one-ulp change in a density does not move a sampled history. |

The bit-for-bit result says the lowering adds and loses nothing against the
hand-built lattice; it does not say the model is physically right. All of it
is pinned by `tests/model_lct008_tutorial.rs` (without nuclear data).

The census table counts each placed cell whole: its total volume (4.818e6 cm³)
is the 49 assembly tiles, not the cylinder's 2.98e6 cm³, because the cylinder
cuts water tiles and the census does not clip them.

## The fast path

LCT-008 is slow for two reasons: **processing 35 tapes** (RECONR, BROADR,
PURR, plus LEAPR for H in H2O), then **transport**. Hardware for every number
below: **Intel Core i9-13900K, 8 logical cores used** (`taskset -c 0-7`,
`RAYON_NUM_THREADS=8`), 62.5 GiB RAM, Linux, CPU only, not otherwise idle
(load average about 3 from other jobs). Seed is the preset's own.

| step | wall time |
|---|---|
| **processed now** (the default): 35 nuclides in parallel | **78.8 s** (79.0 s in a second run) |
| transport, `Smoke` (1 000 × 50 active) | 2.0 s |
| transport, `Fast` (5 000 × 120 active) | 13.1 s (processed data) and 13.9 s (ACE data) |
| transport, `Auto` (10 000 × 250 active) | 43.8 s and 46.2 s |
| **build the ACE library once** (`lct008_ace_library.sh`, 8 processes) | **122.6 s**, 2.0 GB (U-235 alone 102.9 s) |
| **read that library** (`--ace-dir`, the cached run) | **4.2 s** (4.1 s to read and parse 76 files, 0.1 s to load nuclides) |

So the first run on the fast path is about 123 s of data plus 14 s of
transport; every later run is **4.2 s plus 14 s**. A `Fast` run from a
built library takes about 18 s start to finish, against 92 s processing the
tapes each time. (The read was from a warm page cache; a cold read of 2 GB
from disk was not measured.)

**Why the ACE route and not a cache.** There is no disk cache of processed
nuclides for the command line: the browser demos have one in IndexedDB (gh
#818), the CLI does not. The ACE route (gh #936) reads finished tables and
processes nothing, but `reference-data/ace` holds only U-234, 235, 236 and 238
(and H in H2O at 283.6 K, the wrong temperature), none of the other 31 nuclides here.
So the library is built once with the existing generator
(`write_ace_library`, the one the five-route study's route 5 uses) and then
*is* the cache. A real processed-nuclide cache for the CLI was **not built
here**; it would make the first run as cheap as the second without the 2 GB.

**It is not free of physics differences.** ACE tables are printed at 12
figures, and the DBRC companion tables come from the library's `0K/` folder.
The five-route study found routes 4 and 5 within statistics on this case
(32 seeds, −26 ± 26 and −25 ± 25 pcm from route 1). One `Fast` run each: 0.99783
(processed) and 0.99795 (ACE), 12 pcm apart, inside the 150 pcm scatter of one
run.

**Not used for "fast".** `SpeedTier::VeryFast` (RECONR and BROADR at 1 %) is
an approximation measured at −65 ± 40 pcm on this very case
(`docs/profiling/speed_tiers_2026_09_27.md`); it is a hidden ablation, not a
speed-up.

## What you get

| run | k | against the record 1.00135 ± 0.00023 |
|---|---|---|
| `Smoke`, ACE | 1.00066 ± 0.00588 | −0.1σ (resolves nothing) |
| `Fast`, processed | 0.99783 ± 0.00154 | −352 pcm, **−2.3σ** |
| `Fast`, ACE | 0.99795 ± 0.00161 | −340 pcm, −1.6σ |
| `--seed 1..4` (10 000 × [40 + 120]), ACE | 1.00016, 1.00097, 0.99980, 1.00181 (each ± 0.0011) | −0.9σ, −0.3σ, −1.0σ, +0.3σ; mean 1.00069 ± 0.00040, −66 ± 46 pcm |
| `Auto`, processed | 1.00158 ± 0.00081 | +23 pcm, +0.3σ |
| `Auto`, ACE | 0.99938 ± 0.00075 | −197 pcm, −2.4σ |

Read them in this order.

1. **The Fast run is a smoke test**, and its σ is 150 pcm. A −2.3σ draw is
   the low tail of one scatter; the four seeds at the record's own run size
   spread over about 190 pcm and centre 66 ± 46 pcm below the record (1.4σ, not
   resolved). Not a defect found, and not shown to be absent.
2. **`Auto` on processed data reproduces the record** (+0.3σ) at its full
   statistics, which is what this page asks of the tutorial's model. The
   `Auto` run on ACE data sits −2.4σ below it; one draw, two data routes,
   different random streams, so the −220 ± 110 pcm between the two `Auto`
   runs is **not resolved as a data difference**. It is recorded rather than
   explained.
3. **Against the benchmark:** the processed `Auto` run is +158 ± 81 pcm from
   1.0000, the record +135 ± 23 pcm, inside the 600 pcm stand-in. That is
   agreement with an experiment whose uncertainty this repository does not
   hold; it is not a claim of 100 pcm accuracy.
4. **Data vintage.** Route 4 changed in gh:#931 (PURR 20 / 64 / 10 000 after
   HEATR). The record and these runs are after it; the older record
   (−26 ± 26 pcm from route 1 before) moved by −25 ± 31 pcm (not resolved).

The Fast and Smoke runs print "SMOKE TEST, not a result to quote". Quote the
record.

## Predictions, and what happened

Posted on #943 before the first run of the new model.

| prediction | outcome |
|---|---|
| census counts 4961; lookups identical; per-generation k bit for bit | **held** (the one-ulp densities are a finding, not a miss) |
| `Fast` within 4σ of the record | **held**, at −2.3σ |
| processing 100 to 250 s | **missed**: 78.8 s (faster) |
| reading a built library under 60 s | **held**: 4.2 s |
| `Fast` transport 30 to 120 s | **missed**: 13 s. I took the crate's old 20.1 s for 11 nuclides on an unrecorded host and scaled up; threads and the total-cross-section fast path took more off than I added for 35 nuclides. |

## Every simplification

| simplification | kind | what it is worth |
|---|---|---|
| Only case 1 (no pyrex rods, no cases 2 and 8) | no data in this tutorial | cases 2 and 8 are in the V&V example; a pin universe other than 1 and 2 is refused with a message |
| 293.6 K everywhere | no data (cards give none) | OpenMC's default; same on every route |
| A one-layer lattice is extruded through the core's height, tile height only a label | negligible | the cards have no axial structure either (z pitch 1 cm, one layer) |
| One water component for all water | negligible | a label for output, no physics |
| Benchmark uncertainty "about 600 pcm" | no data | stand-in for an experiment uncertainty not in the repository; compare with the record |
| Census counts tiles the cylinder cuts whole | negligible | only water is cut; fuel and clad counts exact (4961) |
| `Fast` and `Smoke` statistics | too expensive to do better here | 13 s against about 45 s for `Auto`; the page quotes `Auto` and the record |
| 4 seeds, not 32 | too expensive | the −66 ± 46 pcm is a bound, not a measurement |
| ACE route: 12-figure tables | open | one `Fast` pair agrees to 12 pcm; no paired ensemble run here |
| Fe-54 via `from_folder` | bounding | `workspace_reference()` cannot offer Fe-54 (its header says NLIB = 8), so the tutorial indexes the same folder with `from_folder`; same tapes |

## What we assumed

- That the cards are the benchmark (they are Romano's model, not the handbook).
- That the water's H-1 law (H in H2O from the LEAPR deck) is right at 293.6 K.
- That the record is a fair yardstick: it is the crate's own route 4, so
  agreement with it checks the beginner lowering, not the physics.
- Nothing was tuned, and no input was changed after seeing k.

## Use, modify, create

**Use.** Run `--geometry-only` and open the wall picture; then
`--fast --ace-dir ...` after building the library. **Modify.** In
`lct008_tutorial_model.rs` change `RowOrder::TopDown` for the core only and
look at the picture; what does the census say? **Create.** Describe a 3 × 3
lattice of your own pins in a box and ask for its census.

## Next question

A 15 × 15 grid of identical rods is the easy kind of repetition. The HTR-10's
pebbles are *spheres packed in hexagonal layers*, and a ball may cross from one
tile into its neighbour. Tutorial 7 builds one layer of that, as an infinite
stack.

## Sources

- `examples/tutorial_lct008.rs`, `examples/common/lct008_tutorial_model.rs`,
  `examples/common/lct008_ace_library.sh`; `tests/model_lct008_tutorial.rs`.
- The V&V case it repurposes: `examples/lct008_keff.rs` and
  `examples/common/lct008_model.rs`.
- The record: `verification_and_validation/icsbep/five_route_keff_2026_09_29.md`
  ("Route 4 re-measured 2026-10-18", gh:#931).
- gh:[#943](https://github.com/theodoreOnzGit/outram-park-backend/issues/943)
  (decisions and progress), gh:[#926](https://github.com/theodoreOnzGit/outram-park-backend/issues/926),
  gh:[#936](https://github.com/theodoreOnzGit/outram-park-backend/issues/936) (ACE route).
