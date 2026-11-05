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

**The cap** (`max_pebbles`, 30 687): the conus and tube's 2 457 (the reference pour's count below the floor), plus a core filled at a fresh pour's measured whole-core φ 0.5954 (`htr10_fill::seed_loose`'s doc) up to the 95 % line. It is a prediction; the cut in step 1 backs it.

## Methodology and results

See below; predictions were written first in `PREDICTION.md`.
