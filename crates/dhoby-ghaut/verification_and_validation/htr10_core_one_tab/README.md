# One whole-core tab for HTR-10, and one k_eff dock for every run (2026-10-25)

> Research, education and V&V only; not for facility operation, licensing or
> safety decisions. AI-assisted change (Claude Code), not yet reviewed by the
> maintainer. This record checks the **page**, not the physics: no number in
> it is a new k_eff result.

## What changed

Maintainer request, 2026-10-25: *"one common interface for the htr10 keff
simulation on web demos ... change the number of stacks and control rod
position under the same tab ... the same keff interface and Shannon entropy
under one collapsible bottom panel or tab. Same as the Godiva keff
simulations too! The top view and control rod views with reference graphs
should be a collapsible side panel or tab."*

- **HTR-10, Watch → "Whole core: layers, rods, live k_eff".** The separate
  "Control rods" tab is gone; `?view=rods` links open this tab.
  - **Controls (left panel):** the bed (Şeker lattice N = 10–20, "stacks",
    or the DEM bed), the ten control rods (synced or one by one), what is
    recorded for that setting, Run k_eff (statistics, Start / Pause / Stop,
    the generation table), worth-curve sweeps, and the neutron animation.
  - **Main view:** the whole core in x-z with the run's neutrons.
  - **Bottom dock "k_eff · entropy":** this run's k per generation, the
    running mean ± σ and the Shannon entropy, against what is recorded for
    the core it ran (record and RMC with every rod out; the recorded rod
    curve with rods in). Its header carries the run's buttons, so a run
    starts and stops with every panel folded.
  - **Right dock "Views":** top slice at bed mid-height, side slice through
    the selected rod, the ten rods' depths, recorded k against rod position,
    recorded k against loading height.
- **Run mode on every rung with a geometry (Godiva and the others):** the
  geometry fills the main view; the same bottom dock holds the k and entropy
  plots, the Run / Pause / Stop buttons and the console.
- **Shared code:** `dhoby_ghaut::web_demo::dock` (library: the dock, with
  layout tests) and `examples/monte_carlo_web/keffplot.rs` (one k + entropy
  drawing for Run mode, serial or pooled, and the HTR-10 core pool).

## Methodology

- **Build:** `crates/dhoby-ghaut/web/monte_carlo/build.sh` (release wasm,
  wasm-bindgen 0.2.129, the published data), later rebuilds of the wasm only;
  served with `python3 -m http.server`.
- **Browser:** Playwright's Chromium, headless, **software rendering**
  (SwiftShader, no GPU) on a 4-core Intel Xeon @ 2.80 GHz container. Frame
  times here are therefore far slower than on a real phone or desktop; they
  are compared with the same harness's earlier record, not with 16 ms.
- **Phone width:** `web/shots.js`, 390 × 844 (touch, mobile) and
  1280 × 900; the HTR-10 tab, the old `?view=rods` link, and Godiva's Run
  mode once its data are processed; on the phone, the Views dock opened and
  closed and the controls opened.
- **No lag:** `web/lagcheck.js`, 1280 × 900, `autostart=1`, 300 × [2 + 6],
  2 workers: frame intervals sampled for 3 s at a time from page load,
  through the pool processing all 36 nuclides, to the end of the run; while
  the data process, the master rod slider is dragged each sample and the time
  for the page title to show the new rods is measured (the rods are locked
  during a run, by design).
- **Pass criteria:** every control reachable at 390 px with the panel folded
  or opened; nothing overlapping a button; the page responds to a drag within
  a few frames while the heaviest work runs.

## Results

Screenshots: `shots/` (phone and desktop, each case above) and
`lagcheck/` (the dock with a live run).

**Phone and desktop.** Both pass after the fixes below. On the phone the
Views dock opens over the whole screen (`Split::covers_main`) and hides
again from its header; the bottom dock stacks k above entropy (HTR-10) and
the console below them (Godiva).

**Lag (first pass, 2026-10-25, before the run-memory fix; frame times
unaffected by it):**

| phase | samples | median frame (median of 3 s medians) | worst p95 | worst frame | title after a rod drag |
|---|---|---|---|---|---|
| data processing (571 s) | 108 | 83 ms | 267 ms | 317 ms | median 86 ms, max 143 ms |
| k_eff run | 12 | 117 ms | 183 ms | 200 ms | (rods locked) |

The rods tab's own record under the same harness
(`../htr10_rod_curve/web/lagcheck_with_pool.txt`) had median frames of
150–400 ms: the merged tab is not slower.

**Lag (re-run after the fix, 2026-10-26, `lagcheck/lagcheck.txt`):**

| phase | samples | median frame (median of 3 s medians) | worst p95 | worst frame | title after a rod drag |
|---|---|---|---|---|---|
| data processing (575 s) | 115 | 83 ms | 200 ms | 200 ms | median 88 ms, max 169 ms |
| k_eff run | 11 | 117 ms | 167 ms | 183 ms | (rods locked) |

The rods the check dragged to 100 % during processing are the rods the run
ran on (the fix in item 5 below): `lagcheck/lagcheck_run_done.png` shows k =
0.85327 ± 0.03762 (300 × [2 + 6], a demonstration size) against the
recorded 0.84624 ± 0.00285 at those rods, +703 ± 3772 pcm. This is a
consistency check of the page's bookkeeping, not a result.

## Found and fixed while checking

1. **The fold arrows drew as boxes** (▾/▴ are not in the page's font):
   ⏷/⏶ instead.
2. **The Views dock started folded on a desktop:** the cut-off is the main
   view's width, and a 1280 px screen with the controls open leaves about
   920 px. Cut-off 1000 → 860 px.
3. **On a phone the open Views dock left a 12 % sliver** in which the bottom
   dock's labels wrapped one letter per line, and "Controls »" covered "Hide
   Views": the dock now covers a phone's whole screen and nothing of the main
   view is drawn under it.
4. **The page's fixed "References" button covered the plots' corner:** a
   bottom dock keeps a 40 px strip free along its bottom
   (`dock::REFS_CLEAR_PX`), and the run's one-line status sits in its left
   part.
5. **A real defect, found by the lag check:** a URL autostart started the run
   on the pool's withdrawn rods while the reader had set 100 %, and when the
   run ended the pool was rebuilt with 100 %, so the dock compared a
   withdrawn-rods k with the 100 %-rods record (a meaningless "+11 247 pcm";
   seen on the first pass's screenshot, which was not kept). A run now records the bed and rods it
   started on (`RunState::layers`, `rods`) and the references follow them;
   autostart goes through the same start path as the button, which rebuilds
   the core with the reader's rods first. Pinned in
   `htr10::rods::tests::a_live_sweep_rebuilds_then_runs`.

## Not checked

- A real phone (only an emulated viewport) and a real GPU.
- The other rungs' Run mode beyond Godiva (they take the same code path).
- The layout at 150 % browser zoom.
