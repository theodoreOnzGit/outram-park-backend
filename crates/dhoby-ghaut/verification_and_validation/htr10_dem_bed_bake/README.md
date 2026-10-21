# HTR-10 DEM bed for the web: bake, provenance and filling fraction (gh:#787)

AI-drafted, 2026-10-07, awaiting human review. Research, education and V&V
only.

This folder records three things. The first is the physics-grade random
HTR-10 bed that the Monte Carlo demo's `htr10` rung shows beside the lattice
bed (the liberties toggle, `?rung=htr10&mode=watch&view=beds`). The second is
how that bed was baked for the web. The third is what the live DEM pour demo
(`demos/dem/`) costs.

## 1. The bed: provenance

- **Engine:** `outram-park-fork-liggghts`' `GranularSystem`, the LIGGGHTS
  port's engine with Hertz contacts and shear history. It is verified against
  upstream LIGGGHTS on the HTR-10 core (crate V&V § 4.7–4.8). The
  artwork-grade `DemSimulation` and `bake_pebble_packing.rs` are **not** used.
- **Run:** the gh:#216 friction study's cell `mu10_mur00`, from
  `examples/htr10_recirculation_sweep.rs --trace-only`. It starts from
  LIGGGHTS' settled flat-floor bed (`reference-data/liggghts/htr10_settled.csv`,
  27 554 pebbles). That bed is drained into the published conus, a triangle
  mesh, and settled adaptively until the kinetic energy per core pebble falls
  below 1e-3 of a one-radius drop.
- **Parameters:**
  - µ = 0.1, µ_r = 0;
  - E = 5e8 Pa, ν = 0.2, e = 0.5;
  - ρ = 1730 kg/m³, r = 3 cm;
  - dt = 35 µs;
  - core radius 0.90 m, conus 0.36946 m, a 0.25 m stub of the 0.25 m-radius
    tube.

  µ = 0.1 is the literature range for graphite on graphite (0.1–0.2). It was
  ablated, not fitted (V&V § 4.9).
- **File:** `reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv`,
  sha256 `43684afaf4247b04261f85ed5bf8b69e831b3d6573e628fbaa5c061daff1e553`.

### Re-run for this bake: prediction, then result

**Prediction** (posted on #787 before the run): the re-run is byte-identical
to the committed bed and trace, because the parallel backend is bit-identical
for any thread count. It settles at 16 000 steps. Whole-core φ = 0.6047 and
bulk φ = 0.6083. It takes 4–7 min at 15–20 ms per step on 3 threads.

**Command:**
```
cargo run --release -j 3 -p outram-park-fork-liggghts --example htr10_recirculation_sweep -- \
  --label bake787_mu10_mur00 --mu 0.1 --mu-r 0 --threads 3 --trace-only
```

**Result:** run on 2026-10-07 at `8df5cde5fb`; the log is in
`bake_rerun_2026_10_07.log`.

- **Hardware:** i9-13900K; 32 logical CPUs, 16 of them visible to the run, 3
  threads used; 62 GB RAM; Linux 7.2.7; CPU only. The machine was shared with
  other jobs, at load average 9–15.

| quantity | predicted | measured |
|---|---|---|
| bed bytes vs committed | identical | **identical** (`cmp`, same sha256) |
| φ-vs-step trace vs committed | identical | **identical** |
| steps to settle | 16 000 | **16 000** (KE/E_drop 3.58e-4) |
| whole-core φ, `N V / (π R² h)` | 0.6047 | **0.604688** |
| bulk-slab φ | 0.6083 | **0.608293** |
| wall time | 4–7 min | **356 s** |
| cost per step | 15–20 ms | **22 ms** (slower than predicted) |

**Against the published 0.61** (IAEA-TECDOC-1382, quoted from a
specification table, not measured): −0.9 %. This is verification against
LIGGGHTS plus agreement with a quoted figure. It is not validation, because
no measured bed is compared.

The duplicate outputs the re-run wrote under `reference-data/liggghts/` were
deleted, because they are byte-identical to the committed files.

## 2. The bake for the web

- **Command:** `cargo run --release -p dhoby-ghaut --example dem_web -- --bake-beds`
  (0.1 s). It writes `crates/dhoby-ghaut/examples/common/htr10_beds.zz`,
  157 kB.
- **Format:**
  - centres in metres, DEM frame (`z = 0` at the conus inlet);
  - quantised to u16 per axis over each bed's own box, a step under 0.05 mm;
  - sorted by height;
  - zlib level 9 with `miniz_oxide`, the codec of the demo's ENDF tapes.
- **Two beds in the file:**
  - **Lattice:** the ball list of `nee_soon`'s
    `assemble_explicit_triso(14, 12, 0)`, read from the assembled core's
    `bed`. It keeps the balls centred at or above the DEM valve (18 961
    balls, 16 681 above the floor).
  - **DEM:** all 27 554 pebbles.
- **Pinned by** `the_baked_beds_regenerate_from_their_sources` (in
  `examples/common/htr10_beds.rs`, run by both demos' tests). It re-bakes from
  the CSV and the builder, and requires identical bytes. It also requires 27 554
  pebbles, whole-core φ within 1e-4 of 0.6047, quantisation moving φ by under
  1e-4, and every lattice ball inside the DEM vessel's wall to 2 mm.

**Measured on the decoded beds:**

| | lattice, N = 12 | DEM, all | DEM, trimmed to the lattice's 16 681 |
|---|---|---|---|
| balls above the floor | 16 681 | 25 097 | 16 681 |
| surface (p99 + r) | 123.58 cm | 184.46 cm | 123.70 cm |
| whole-core φ | 0.5999 | 0.604694 (full precision 0.604688) | 0.5994 |

Trimming keeps the lowest balls above the floor, as `nee_soon`'s DEM-bed
builder does with 16 890. At equal ball count the two beds stand within
1.2 mm of each other.

## 3. The live pour (`demos/dem/`)

The pour is `htr10_fill::Htr10Fill` at its defaults, which are the gh:#216
settings, with 4 000 pebbles, one thread and seed `0x5EED0010`. It runs in
the page's Web Worker, one chunk per message, with chunks sized to about
120 ms.

**φ instruments.**
- **Whole-core φ.** This is gh:#216's like-for-like measure. A 4 000-pebble
  bed stands only about 15 cm above the conus, so it reads 0.44 there.
- **Bulk φ of the filled vessel.** It excludes 4 r at the valve and at the
  surface and divides by `∫ π R(z)² dz` through the tube, conus and barrel.
  It is the crate's bulk-slab measure widened to the vessel. This measure was
  designed **after** the 0.44 had been seen.
- **Predictions, written before the bulk measure was first run:**
  - 0.600–0.607 on the full-size bed;
  - 0.57–0.60 on the 4 000 pour.
- **Measured** (natively, one thread, same machine and load;
  `browser_pour_native_4000.log`):
  - **0.6061** on the full-size bed;
  - **0.5970** on the 4 000 pour, settled at 22 500 steps;
  - the 4 000 pour took 131 s at 5.8 ms per step;
  - both predictions held.
- **Pinned** by `the_bulk_fraction_of_a_known_packing`, a simple cubic block:
  π/6 over the tube area to 1 %. Also pinned by
  `the_plotted_full_size_figures_are_the_baked_beds`.

**Browser** (headless Chromium, software rendering with swiftshader;
`headless_chromium_check.log`):
- **Settling:** one desktop run, not pinned, settled at **22 406 steps after
  about 5 minutes**, at 6–10 ms per step. Its bulk φ was **0.5961** and its
  whole-core φ **0.4457**.
- **Why that differs from native:** wasm's `sin` and `cos` (the seed
  placement) come from a different libm, so the chaotic pour is a different
  realisation. Native and browser are each deterministic.
- **The settle defect.** The first browser run reported "settled" at step 20.
  The pebbles start at rest, and `Htr10Fill::progress` called a pour settled
  whenever its KE was below the target. That is now fixed: the KE must first
  have risen above the target. It is pinned by
  `a_pour_is_not_settled_before_it_has_fallen`. Earlier callers stepped in
  chunks of 500 or more, so they always saw the fall first, and no recorded
  result changes.

**No lag (the HARD RULE), measured.**
- **Long tasks while pouring.** Main-thread long tasks occur on every paint:
  - 50–240 ms each with Chromium pinned to 3 cores at load average 13–19;
  - 16 in 30 s, up to 84 ms, when not pinned at load 14.
- **Where the time goes.** A CPU profile of the page's main thread over 6 s of
  pouring gives:
  - **~150 ms in JS and wasm** (egui tessellation, the message, the title);
  - **5 750 ms in "(program)"**, the browser's own software rasterisation and
    compositing.
- **The page computes nothing.** The DEM runs in the worker, the page repaints
  only when a chunk arrives, and the remaining cost is software GPU
  emulation on a loaded CPU. The same pattern was recorded on #528 and #549.
- **Frame rate** (rAF) during the pour: 7–30 fps at 390 × 844 DPR 2, and
  9–60 fps on desktop.
- **Zooming** with + and − during the pour responded every time.
- **The beds view** decodes and draws in 4–6 long tasks (100–360 ms) when it
  opens, then idles at 60 fps. Real-phone GPUs: **not checked**.

## Images (`images/`, from headless Chromium, 2026-10-07)

| file | what it shows | checked |
|---|---|---|
| `desktop-beds-side.png`, `phone-beds-side.png` | the liberties toggle: the lattice and the DEM bed cut by y = 0, same 16 681 balls | lattice rows and straight gaps against no order in the DEM bed; both inside the drawn wall; equal heights; the "no k_eff" label; stacked on the phone |
| `desktop-dem-settled.png` | the live 4 000 pour, settled, with φ and KE against time | φ plateaus as KE falls below 1e-3; the reduced count is stated on screen |
| `phone-dem-30s.png` | the pour at 390 px, 30 s in | controls folded, + / − / Reset visible, plots under the vessel |
| `desktop-dem-baked.png` | the full-size baked bed in the DEM demo | 27 554 pebbles, surface ~1.85 m, cut away |

**Not checked by eye:** the 0.05 mm quantisation, the soft-sphere overlaps
(up to 0.5 mm) and wall penetrations. They are below the drawn resolution
and are covered by the tests and numbers above.
