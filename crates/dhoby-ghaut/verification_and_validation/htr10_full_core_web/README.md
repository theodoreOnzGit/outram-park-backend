# HTR-10 full core in the browser: measured cost and the live k_eff (gh:#786)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** Research, education and V&V only; not
> for any operational use. AI-assisted (Claude Opus 5.5 in Claude Code), not
> yet reviewed by the maintainer.

**Class: verification** (the browser path against the native path and the
recorded runs of the same code). Predictions, committed before any of this
was measured: [`PREDICTION.md`](PREDICTION.md).

## Methodology

- **What runs.** The `htr10` rung's "whole core" view of the Monte Carlo
  demo (`examples/monte_carlo_web/htr10/core/`): the recorded runs' model
  (`assemble_explicit_triso(14, 12, 0)`, `htr10_material_set(..,
  benchmark_default(300.15))`, `Htr10DataConfig::default()`: ENDF/B-VIII.0,
  300.15 K, tolerance 1e-3, URR and DBRC on, every bound thermal law, real
  Ni/Fe rod steel, UO₂ laws by LEAPR), on a pool of Web Workers:
  - **data**, split by nuclide: 36 jobs (31 tapes, 5 laws;
    `nee_soon::htr10_rmc::data_jobs`) dealt longest first; each product
    (`Nuclide::process_evaluation` / `ThermalScattering`, as `f64`s) relayed
    by the page to every other worker; each worker then rebuilds the 38
    nuclides (`Nuclide::from_processed`), the core and the bed majorant;
  - **histories**, split per generation: the page holds
    `transport_csg::distributed::DistributedPowerIteration` and deals 3 chunks
    per worker per generation; results are reduced in history order.
- **Browser.** Headless Chromium (Arch package), `--headless=new`, ANGLE
  SwiftShader (software WebGL), driven over CDP by a Node script; the whole
  browser held to 5 logical CPUs (`taskset -c 11-15`, `nice 10`), so the page,
  the GPU process and every worker share 5 cores. Long tasks from a
  `PerformanceObserver('longtask')` on the page; frames from
  `requestAnimationFrame`; per-worker memory is each worker's own wasm linear
  memory (`WebAssembly.Memory.buffer.byteLength`, reported by the worker).
- **Hardware.** Intel Core i9-13900K (32 logical CPUs), 62.5 GiB RAM, Linux
  7.2.7, CPU only (software rendering). **Shared and loaded**: other agents
  were building and running (load average about 24 on the 16 cores the
  session saw); wall times are upper bounds.
- **Settings of the live run.** 1000 neutrons × [5 inactive + 20 active],
  seed 20260917 (the record's), N = 12.
- **Native reference.** `--bake-htr10-core 10000 2 5` (the pool's own code,
  natively, 5 threads): `logs/bake_native.log`.
- Date: 2026-10-07/08. Code: the commits of gh:#786 on this branch.

## Results

### The native path reproduces the record

At the record's settings (10 000 neutrons, seed 20260917, N = 12), the jobs
path and the distributed power iteration give **generation 0: k = 0.935074,
entropy 5.5797; generation 1: k = 0.983278**, which are the recorded log's
first two generations to every printed digit
(`nee_soon/verification_and_validation/htr10_seker_2026_10_07_10k/logs/run_e8_N12.log`:
`gen 0: k = 0.935074`, `gen 1: k = 0.983278`, entropy trace from 5.580).
That checks the split data processing (every nuclide, law and the majorant)
and the distributed transport against the record's own code path at once.
Native timings (5 threads, loaded): jobs 79 s wall (U-235 58 s, C-in-SiC
57 s, U-238 47 s, graphite 32 s, the LEAPR laws 24–28 s), one worker's
assembly 12 s (nuclides 1.2 s, core 0.16 s, majorant 10.6 s, 251 920
nodes), transport 13–15 ms of CPU per history.

### Browser, desktop (1280 × 800)

| run | workers | data ready | run (25 gen.) | long tasks > 100 ms (max) | frames |
|---|---|---|---|---|---|
| 1 | 4 | 97 s | ≈ 105 s (3.7–6.4 s/gen) | **25 (1367 ms)** | ~29 fps |
| 2 | 2 | 128 s | ≈ 150 s (5.6–6.1 s/gen) | **20 (812 ms)**, 9 of them in the first 30 s | ~33 fps |
| 3 | 4 (profiled) | 67 s | 12 gen. in 45 s | 0 (max 62 ms) | 60 fps |
| 4 | 4 | 73 s | ≈ 75 s (3.0–3.3 s/gen) | 0 (one task, 66 ms) | 60 fps |
| 5 | 4 | 90 s | 8 gen. in 30 s | 1 (289 ms) | ~37 fps |

- **Memory per worker: 544–546 MB** of wasm memory once ready (2 and 4
  workers alike). The page's JS heap peaked at 74 MB while products were
  being relayed (U-235's product is 7.1 M words, 57 MB), then fell to 2–5 MB.
- **Download:** 35 MB for the core's 35 tapes (covariances stripped, zlib),
  of 43 MB for the whole demo.
- **Result (run 1 = run 2 = run 4):** **k = 0.98891 ± 0.01045** (within-run
  σ over 20 active generations). Against the record at N = 12,
  0.995125 ± 0.001055: **−623 ± 1050 pcm (0.6σ)**. Against RMC at equal
  ball count, 0.999419: −1053 ± 1045 pcm (1.0σ).
- **2 workers and 4 workers give the same k in every one of the 25
  generations, bit for bit** (logged with full precision to the console and
  compared). The unit test `the_pool_answer_does_not_depend_on_the_worker_count`
  pins the same for 1 and 3 scripted workers.
- **Browser against native at the browser's settings** (1000 neutrons, same
  seed): generation 0 is 0.988776 in the browser and 0.937925 natively, with
  entropy 5.422 against 5.454. Not the same histories. The wasm build uses
  Rust's libm where native uses glibc, both in transport (source directions,
  free paths) and in the data processing; that the difference is only this
  was **not** tested (the native `deterministic-math` route covers transport
  only). The two are 1.5 σ of one generation's spread apart (the browser's
  generation-to-generation k varies 0.909–1.062).

### Browser, phone width (390 × 844, DPR 3, mobile emulation)

The sizing rule gave **2 workers** (narrow screen: at most 2), data ready in
107 s, 4.7 s per generation; 1 long task (71 ms). The panel opens folded;
"Controls »", + / − / Reset and the status lines are on the main view.
(Emulated: the CPU is this desktop's, not a phone's. Real-phone timing is not
measured.)

### Against the predictions

| | predicted | measured | |
|---|---|---|---|
| P1 one worker | 3–6 min | not measured (2 workers: 128 s) | — |
| P2 4 workers | 1.5–3 min | **67–97 s** | faster than predicted: the per-worker rebuild is 1–2 s, not 10–25 % of the serial time |
| P3 memory per worker | 250–600 MB | **544–546 MB** | held |
| P4 per history per worker | 13–35 ms | 12–19 ms (worker busy time / histories) | held, low end |
| P5 1000 × [5 + 20] on 4 workers | 80–220 s | **75–105 s** | held, low end |
| P6 σ and mean | 800–1000 pcm; 0.978–1.012 | **1045 pcm**; 0.98891 | σ slightly above the range; mean inside |
| P7 1 vs N workers bit for bit | identical | identical (2 vs 4, 25 generations) | held |
| P8 browser vs native | not bit-identical | not identical from generation 0 | held; cause not isolated |
| P9 long tasks | a few during relay, none in transport | **none in 3 runs; 20–25 (up to 1.4 s) in 2 runs under heavy machine load**, in the data phase and in transport | partly missed |
| P10 phone | 1–2 workers | 2 workers | held (timing not on a real phone) |

**Decision (rule fixed in the prediction):** data under ~5 min with progress
→ **live**; a 25-generation k under ~5 min → **live k_eff**. The view still
opens on the recorded tracks (the bake), labelled RECORDED, and the reader
starts the live run.

### Interpretation

- The browser runs the record's model, not a stand-in: the native run of the
  same code reproduces the record's logged generations exactly.
- A 1000-neutron run shows the core near critical at N = 12 (0.6σ from the
  record); with σ ≈ 1000 pcm it does not test the record's 105 pcm.
- **The no-lag rule is not demonstrated under load.** In two of five desktop
  runs the page had long tasks of up to 1.4 s, at times when the page itself
  does little (relays, a reduction of 1000 sites). The quiet reruns of the
  same build had none, so CPU starvation of the page (the workers, the
  software GPU process and the page on 5 shared cores of a loaded machine) is
  the likeliest cause, but it was not profiled while it happened. A pool of
  `cores − 1` leaves the page one core only on an otherwise idle machine.
  Follow-up: [gh:#788](https://github.com/theodoreOnzGit/outram-park-backend/issues/788).
