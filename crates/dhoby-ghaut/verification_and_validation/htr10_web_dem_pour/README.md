# Your own DEM pour as the HTR-10 core's bed, in the browser (gh:#1244, 2026-11-05)

> Research, education and V&V only; not for facility operation, licensing or
> safety decisions. AI-assisted change (Claude Code), not yet reviewed by the
> maintainer. No k_eff here is a physics result.

**Request (maintainer, 2026-11-05):** "for web browser demo Dem side, besides the baked in bed (I wanna keep that) I want you to ensure that I can run a pebble bed fill (Dem) based on the ratio of fuel to moderator pebbles, then become MC cross section. Allow the user to fill x number of pebbles up to the maximum of 95% of bed height. Use WGSL from the LIGGGHTS fork if it needs speed up."

## What the page does

In `monte_carlo_web`'s HTR-10 whole core (`?rung=htr10&mode=watch&view=core`), the bed choice has three options:
- **Şeker lattice** (the record's);
- **DEM random bed (baked)**, unchanged;
- **DEM pour (yours)**, new.

**The controls of "DEM pour (yours)":**
- the number of pebbles, from 3 000 up to the cap;
- the fuel fraction above the floor, shown as a fuel : moderator ratio;
- a seed;
- **Pour**, **Pause/Resume**, and **Use this bed for k_eff**.

**The pour.** It is `dem_web`'s engine, included by `#[path]`, so it is the same code as the DEM pour demo: `outram_park_fork_liggghts::htr10_fill` at the gh:#216 settings, on one thread.
- It runs in the page's own worker, the one that draws the slice.
- It goes one chunk of about 250 ms at a time.
- The main view draws the pebbles as they fall, cut by y = 0, with the vessel wall, the floor and the 95 % line.

**What "Use this bed" does** (`htr10/core/pour.rs`, `poured_bed`):
1. **Cut at the 95 % line.** Every pebble whose top stands above 210.727 cm (95 % of the 221.818 cm cavity) is removed, and the page says how many.
2. **Assign fuel.** Fuel flags come from `fuel_assignment_at` at the reader's fraction: the paper's rule, by height from the floor up, with the conus and tube all moderator.
3. **Hand it to every worker.** Each worker gets the bed (`CoreReq::PourBed`), and then the rebuild (`Layers` with the bed code `USER_POUR`).
4. **Build the core.** It is built as the baked DEM bed is: `ParametricStudy` with `StudyBed::Explicit` and random TRISO (gh:#1243). The Monte Carlo then runs on it as on any other bed.

**The default** (`DEFAULT_PEBBLES`, 19 347; maintainer, 2026-11-05: "default to the record's loading, ~16,900 core pebbles"): HTR-10's first-criticality loading of 16 890 core pebbles, plus the conus and tube's 2 457. A fresh pour's own split may differ by a few, and the page prints the core count it got. ~~The slider started at 8 000~~ (changed the same day).

**The cap** (`max_pebbles`, 30 687): the conus and tube's 2 457 (the reference pour's count below the floor), plus a core filled at a fresh pour's measured whole-core φ 0.5954 (`htr10_fill::seed_loose`'s doc) up to the 95 % line. It is a prediction; the cut in step 1 backs it.

**Diagnostics after the pour** (maintainer, 2026-11-05: "Post pour, give diagnostics like packing fraction and rdf graph"). `pour::diagnostics` runs in the worker once a pour settles, or on "Measure". It gives:
- the whole-core φ, gh:#216's instrument against the published 0.61;
- the vessel bulk φ;
- g(r) of the core pebbles by `outram_park_fork_liggghts::rdf`, the same instrument as `bed_rdf_check`: r_max 5 d, 250 bins, the bed's own cylinder. r_max is stepped down to 2.5 d only for a bed too shallow to put 200 centres 5 d from every face.

The panel prints these values beside the reference bed's, and draws g(r) with the reference's √2 d, √3 d and 2 d points marked.

## Methodology

Predictions were written first, in `PREDICTION.md`.

1. **Unit tests** (`cargo test --release -p dhoby-ghaut --example monte_carlo_web -- pour random_bed`, 11 tests). They check:
   - the cap from the reference CSV;
   - the cut and the fuel rule;
   - the wire round-trip;
   - the page's chunk protocol;
   - the pool sends each worker the bed before its rebuild;
   - a handed-over pour builds cell for cell as the baked bed does;
   - the review images are written.

   `the_diagnostics_reproduce_the_reference_bed` runs the diagnostics on the reference pour and compares them with its recorded figures.
2. **The cap, natively.** `htr10_fresh_fill --n 30687 --threads 3`, the page's maximum, made into a bed at 57 % fuel and drawn with `monte_carlo_web --render-pour` (`images/`). g(r) is by `bed_rdf_check`.
3. **The browser, end to end.** Headless Chromium 141 with SwiftShader, driven by `nuclear_data_cache_web/run_headless_932.mjs`:
   - `?rung=htr10&mode=watch&view=core&autostart&pour=4000&workers=2`: pour, use, then two workers process the data, build the core on the pour and run k_eff generations;
   - `pour=30687` for 300 s, to sample the browser's cost per step at the cap;
   - `pour=8000` with the diagnostics build.

**Hardware, for every timing:** a cloud container with 4 vCPU (Intel Xeon, model not exposed) and 15 GiB, rustc 1.99.1, release build. **The machine was shared** between the runs: load average 6–11, with the native pour on 3 threads, the browser runs and a build of the other demos. Every time is an upper bound for an idle machine.

## Results (2026-11-05)

**Unit tests: 11 passed.** On the reference pour the diagnostics give:

| quantity | recorded | measured |
|---|---|---|
| whole-core φ | 0.604688 | 0.6047 |
| g(√2 d) | 0.599 | 0.599 |
| g(√3 d) | 1.286 | 1.286 |
| g(2 d) | 1.190 | 1.190 |
| contact peak | 19.56 | 19.56 |

1.3 s natively, one thread.

**A first r_max rule was replaced.** The first version reduced r_max until a third of the core's centres were interior. On the reference bed that chose 4.25 d and gave g(2 d) = 1.120. It was not the reference's instrument, so it was replaced by the rule above.

**The cap pour, natively** (`native_max_pour_30687.log`):

| quantity | predicted | measured |
|---|---|---|
| surface (99th pct + r) | 205–215 cm | **206.37 cm**, under the 210.73 cm line |
| whole-core φ | 0.59–0.60 | **0.6074**, outside the range |
| steps to settle | 25 000–40 000 | **32 000** |
| cost per step, 3 threads | 15–20 ms | **36 ms** on average (shared machine) |
| wall time | 10–20 min | **20.3 min** (1 145 s) |
| pebbles cut at the line | 0–300 | **7** |

- **φ came out above the predicted range.** The pour packs denser than the fresh 16 890-pebble pour the cap assumed (0.5954), so the cap leaves 4 cm spare under the line, rather than overshooting.
- **The bed made from it:** 28 195 core pebbles, 16 071 of them fuel (57.0 %), top 210.6 cm.
- **g(r)** (`bed_rdf_check`): contact peak 19.47, g(√2 d) 0.594, g(√3 d) 1.299, g(2 d) 1.187. This is a random packing, as the reference is.
- **Images, checked by eye** (`images/`):
  - `pour_core_xz.png`: the bed fills the cavity to its top with a helium gap under the top reflector; the conus and the discharge tube are full; the reflector, boronated bricks and bottom structures sit where the baked bed's images have them.
  - `pour_bed_xz.png`: whole pebbles, no overlaps visible, fuel pebbles (with TRISO) mixed through the height.
  - `pour_fuel_pebble_xy.png`: one fuel pebble, TRISO packed at random, all whole inside the fuel zone.
  - **Not checked:** the control-rod channels (no slice cuts a rod).

**The browser, end to end** (`headless_pour4000_end_to_end.json`, exit 0):
- **The pour:** 4 000 pebbles settled at 20 722 steps in **242 s** wall, at 9.6–10 ms per step. Bulk φ 0.597. The whole-core φ of 0.443 is low because the bed is only 2.5 diameters deep (the 4 000-pebble case in the DEM demo's record shows the same).
- **The bed:** 1 536 core pebbles, 875 fuel (57.0 %), top 17.4 cm.
- **Processing and assembly:** the two workers processed the 36 jobs with no cache and built the core on the pour. Each took 46 s to assemble, 38 s of it the majorant. Their wasm memory was 446 and 685 MB. Assembly finished 644 s after the pool started.
- **The run:** generations 0–3 gave k = 0.187, 0.175, 0.204, 0.190. A 17 cm bed is far from critical; these numbers show the poured core transports, and are not a physics result.
- **Responsiveness:** the main-thread probe's worst answer was 756 ms, and the worst frame gap 450 ms, with 1 863 frames over 100 ms of 14 661. These were measured on the shared machine with SwiftShader software rendering; the DEM demo's record attributes most of such time to the browser's own rasterising. **This does not demonstrate the no-lag rule.** A clean measurement on an idle machine with a GPU is still owed.

**Diagnostics in the browser** (`headless_pour8000_diagnostics.json`, `pour=8000`, one worker):
- **The pour:** settled at 20 013 steps in 339 s, at 13–24 ms per step.
- **The bed:** 5 528 core pebbles, 3 150 of them fuel; top 45.9 cm.
- **Packing:** whole-core φ 0.5639, bulk φ 0.6022.
- **g(r):** the bed is 44 cm deep, so r_max stepped down to 3.25 d, over 309 interior centres. Contact peak 20.23; g(√2 d) 0.592, g(√3 d) 1.263, g(2 d) 1.136. These are random-packing features, from fewer centres than the reference's 7 071 and a smaller r_max, so they are noisier.
- **Cost:** under 0.05 s in the worker.
- **No pool in this run.** Its URL had no `autostart`, so no pool was started; the page sat idle on the settled pour from 339 s to the 1 200 s timeout.
- **Responsiveness:** the worst frame gap was **2 450 ms**, at 914 s, with the page idle. At that time the machine was compiling the full test suite and the other demos (load average 6–7 on 4 vCPU). Over 100 ms: 947 of 34 779 frames. Contention is the likely cause, but **this was not shown**; the no-lag check on an idle machine is still owed.

**The browser at the cap** (300 s sample, `pour=30687`, one worker; `headless_pour30687_300s.json`):
- 12 006 steps in 255 s;
- 18 ms per step while the pebbles fall loose, rising to 36 ms as they pack;
- the worst probe answer 410 ms and the worst frame gap 233 ms.

~~Assuming the dense phase keeps the native pour's step count (32 000) at about 40 ms, a full-cap pour in this browser takes about **20–25 min**. This is an extrapolation, not a measured settle.~~ **SUPERSEDED the same day:** measured on an idle machine at 17.5 min (above).

**The cap, in the browser, end to end, on an idle machine** (`headless_cap_pour_idle.json`, `?autostart&pour=30687&workers=2`, built from develop `79896c3ad`, load average 1.0 before and 2.6–5.3 during):
- **A measured settle:** 32 614 steps in **1 051 s (17.5 min)**, at 15–41 ms per step. This replaces the 20–25 min extrapolation below.
- **Packing:** surface 206.1 cm, whole-core φ 0.6081, bulk φ 0.6100.
- **The bed:** 28 191 core pebbles, 16 068 fuel (57.0 %), 10 pebbles cut at the 95 % line.
- **g(r), measured in the worker** in 0.74 s, over 8 461 interior centres to 5 d: contact peak 19.49, g(√2 d) 0.587, g(√3 d) 1.297, g(2 d) 1.190. These are random packing values, at the reference's (19.56 / 0.599 / 1.286 / 1.190).
- **The pool:** both workers built the core on it in 24.5 s (majorant 18.4 s), with wasm memory 498 and 685 MB. Generations 0–2 gave k = 1.154, 1.196, 1.147. A 28 000-pebble core is well past the 16 890-pebble critical loading, so k > 1 is expected; this is not a physics result.
- **No lag, measured:**
  - **frame gaps:** over the whole 1 550 s (pour, diagnostics, data processing, assembly, generations), the worst was **233 ms**, and only **5 of 50 388 frames** took over 100 ms;
  - **probe:** the wheel-event probe answered with median 214 ms, 95th percentile 301 ms, worst 338 ms.
  
  This is with SwiftShader software rendering. The DEM demo's record shows most such time is the browser's own rasterising, and no GPU is available here. The UI never froze, so it meets the no-lag rule as far as this machine can measure. The earlier 450 ms and 2.45 s gaps came on the loaded machine.

## The GPU engine (gh:#1250, 2026-11-05)

**What it is.** The maintainer asked for "a wgsl option for LIGGGHTS, within the LIGGGHTS crate, use this as a second option for the pour web demo". So the panel has an **Engine** choice: "CPU (one thread)" (the default) or "GPU (WebGPU)".
- **The GPU engine:** `outram_park_fork_liggghts::gpu::dem`, the same contact law, walls and integrator as WGSL compute shaders, run through `htr10_fill::Htr10GpuFill` in the page's worker.
- **Precision:** f32, with double-f32 positions and overlaps. Not bit-identical to the CPU.
- **Without WebGPU** the page says so, and the CPU pour still works.
- **Timings:** every finished pour goes into a timing table with its engine.

**Verified natively** (`tests/gpu_dem.rs`, on lavapipe, i.e. Mesa's software Vulkan). Tolerances were written before the first run, and all five cases met them on the third run:

| Case | GPU vs CPU |
|---|---|
| Head-on pair | 8.8e-9 |
| Oblique pair with friction and spin | 1.8e-7 |
| Resting ball | 4.8e-13 m |
| Cylinder bounce | 1.65e-6 |
| 4 000-pebble pour, early (median pebble) | 6.8e-7 m |
| 4 000-pebble pour, settled φ | 0.4459 (CPU 0.4465) |

Two compiler behaviours had to be beaten to get there; the test's docs give the history.
- A reassociating shader compiler cancelled the compensated arithmetic. Every ordered partial now goes through `keep`.
- Chrome's compiler refused a NaN bit pattern in a constant, which made every pipeline invalid in the browser.

**Verified in the browser** (headless Chromium 141 with `--enable-unsafe-webgpu`; WebGPU on SwiftShader; `headless_pour4000_{gpu,cpu}_console.log`). 4 000 pebbles on each engine, same seed:

| | GPU (WebGPU) | CPU |
|---|---|---|
| settled after | 22 620 steps | 20 720 steps |
| surface | 15.4 cm | 15.4 cm |
| bulk φ | 0.5984 | 0.5973 |
| bed | 1 526 core pebbles, 869 fuel | 1 536 core pebbles |

The GPU pour was used as the bed, and its diagnostics ran; a 4 000-pebble bed is too shallow for g(r), and the page says so. The page stayed live throughout: the worst frame gap was 67 ms during a GPU pour.

**No timing is claimed here.** This container has no GPU (WebGPU runs on SwiftShader, natively lavapipe), so its times say nothing about a real GPU. The page's timing table shows each device's own.

**Still owed.** The packing fraction and g(r) at the record's loading, GPU against several CPU seeds (`examples/gpu_pour_compare.rs`, the maintainer's acceptance criterion), has not been run yet.

## WGSL

The maintainer said "Use WGSL from the LIGGGHTS fork if it needs speed up". This change **does not add a GPU timestep**:
- **Nothing to reuse.** The fork's only WGSL kernel is the g(r) histogram, which is native-only. Its timestep deliberately has no GPU path: it is kept bit-identical to the scalar reference, which reproduces upstream LIGGGHTS (`outram-park-fork-liggghts/CLAUDE.md`, `compute`'s docs).
- **What one would take.** A GPU timestep would be new physics code with its own verification (shear history on the GPU, agreement with the CPU path to be shown statistically rather than bit for bit).
- **The case for it.** A full core at the cap pours in the browser in **17.5 min** (measured above; the earlier 20–25 min was an extrapolation). 4 000 pebbles take 4 min.
- **Next step.** The decision is gh:#1250, for the maintainer.
- **CHANGED 2026-11-05:** the maintainer chose to keep the CPU pour as the default (the page shows its measured time) **and** to add the GPU engine as a second option; see the section above.

## Not done here

- No pour-to-pour scatter of k.
- No k_eff on a full poured core in the browser.
- ~~The no-lag measurement on an idle machine.~~ Done above; a measurement with a real GPU is not possible in this container.
