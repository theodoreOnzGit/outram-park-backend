# The shared nuclear-data cache in the browser: measured (gh:#818)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** Research, education and V&V only; not
> for any operational use. AI-assisted (Claude Opus 5.5 in Claude Code), not
> yet reviewed by the maintainer.

**Class: verification** (a cached product against the processed product,
and the demo's answer with and without the cache). Predictions, written and
posted on gh:#818 before any of this was measured: [`PREDICTION.md`](PREDICTION.md).

## Methodology

- **What is cached.** The expensive half of each nuclide or law, in the exact
  `f64` encodings of gh:#786 (`ProcessedEvaluation::to_f64s`,
  `ThermalScattering::to_f64s`), in IndexedDB on the site's origin
  (`dhoby_ghaut::web_demo::data_cache`, database `outram-park-nuclear-data`).
  The key is the SHA-256 of the covariance-stripped tape, the material, the
  temperature and tolerances to the bit, PURR's table settings, the law's
  label, and the processing-code version `CODE`
  (`examples/common/processed_cache.rs`, ~~`nd-2026-10-09.1`~~ ~~`nd-2026-10-15.1` since
  2026-10-15, after `50911476b` changed U-234's processed output~~
  `nd-2026-10-18.1` since gh:#932, when the products became njoy
  `process`'s). Each record
  carries a SHA-256 of its words; a record with another key or code, a bad
  checksum or words that do not decode is a visible miss and is processed
  again.
- **Native checks** (release): `processed_cache::tests` (U-234 at both tiers
  the demos use, from the tape, through a store, and from a record offered
  back; corrupt, other-code and junk records refused and reprocessed; the
  Si-in-SiC law; the code-version fingerprint of He-4 and U-234),
  `htr10::core::tests::a_cached_job_is_the_processed_job`, and the engine
  thread test (a rung loaded twice: the second time every product comes from
  the engine's memory and the neutrons are the same).
- **Browser.** Headless Chromium (Arch package, `--headless=new`, ANGLE
  SwiftShader) driven over CDP by a Node script, the whole browser held to 4
  logical CPUs (`taskset -c 12-15`), each check a fresh browser process
  closed at the end, served by `python3 -m http.server` from the
  `build.sh` output. Page `?rung=htr10&mode=watch&view=core&autostart`, which
  sizes its pool from the device and runs 1000 neutrons × [5 + 20] at N = 12,
  seed 20260917. **First load**: a new, empty profile. **Cached load**: the
  same profile, browser restarted (what a reload or a discarded tab does).
  "Data ready" is the page's own clock to the first title with "Data ready".
  Long tasks from a `PerformanceObserver('longtask')` on the page; `k` from
  the console line each generation logs at full precision.
- **Hardware.** Intel Core i9-13900K (32 logical CPUs, 4 used by the browser),
  62.5 GiB RAM, Linux 7.2.7, CPU only (software rendering); the machine was
  lightly loaded (load average about 2). Native tests: the same host,
  release, one thread per test.
- Date: 2026-10-09. Code: this commit.

## Results

### Native

- U-234, `Fast` and `VeryFast`: **identical** in all 106 395 compared
  values each (cross sections at 1 300+ energies, URR samples, bounds,
  fission-energy draws), from the tape, through a store and from a record.
  The three refused records were each noted and reprocessed to the same
  nuclide.
- htr10 core jobs: O-16 (76 849 words) and Si-in-SiC (229 552 words)
  identical, processed and cached, to the path every recorded run took.
- The `triso` rung loaded twice by one engine: 11 of 11 products from memory
  the second time, the same five neutrons as the headless fixture.

### Browser

| run | workers | data ready | from the cache | longest page task (all runs: at page start) | k over 25 generations |
|---|---|---|---|---|---|
| desktop 1280 × 800, first | 3 | **112 s** | 0 of 36 | 315 ms at 84 ms | 0.98891 ± 0.01045 |
| desktop, cached | 3 | **22 s** | **36 of 36** | 296 ms at 101 ms | identical, every generation |
| phone 390 × 844, first | 2 | **141 s** | 0 of 36 | 287 ms at 77 ms | identical to desktop |
| phone, cached | 2 | **16 s** | **36 of 36** | 128 ms at 91 ms | identical |

- **k**: all four runs give the same `k` in all 25 generations to full
  precision, and the same mean as gh:#786's record (0.98891 ± 0.01045).
- **Cache size: 131.6 MB** for the core's 36 products (U-235 56.7 MB, U-238
  24.7 MB, Fe-56 8.5 MB, Fe-54 7.6 MB, every other under 3 MB); 166 MB after
  the Godiva rung's three were added. Chromium reported a quota of 10.8 GB.
- **Long tasks**: the only task over 100 ms in each run is at page start
  (77–101 ms after navigation: compiling the wasm), before any data; none
  while products were read back and relayed. The page's JS heap peaked at
  94 MB during the desktop cached load (products in transit) against 60 MB on
  a first load.
- **Memory per worker** after a cached load: 394–396 MB of wasm memory,
  against 546 MB after processing (the processing peak is not reached).
- **View and rung switches** (desktop, cached profile): switching to the
  geometry view and back, and to the Godiva rung and back, kept the pool:
  the run went on behind the other view (generation 11 after a minute on
  Godiva), the page title still read "Data ready in 22 s", and the browser
  still had 4 workers (the page's engine and the pool's 3). Godiva itself
  loaded 2 of 3 products from the cache the second time and 3 of 3 the
  third.
- **Across pages**: in a fresh profile, the Monte Carlo demo's `triso` rung
  processed its 11 products (43 s); the delta-tracking demo, opened next in
  the same profile, read all 11 back ("Nuclear data: all 11 from this
  browser's cache"), 1.2 s after opening.
- **Quota**: with the site's quota held to 20 MB (CDP
  `Storage.overrideQuotaForOrigin`), 12 products were saved, U-235 and U-238
  were refused with `QuotaExceededError`, the note ("was not cached: this
  browser's storage quota for the site is full") went to the pool log and
  the console, nothing more was saved, and the load finished normally.
- **Clear**: "Clear cached nuclear data" emptied the store (0 entries,
  checked from the page) and showed "Cache: in this browser: empty" and the
  note.

## Predictions against measurements

| quantity | predicted | measured | |
|---|---|---|---|
| first load, desktop | 90–150 s | 112 s | held |
| cached load, desktop | 25–50 s | 22 s | **missed** (faster) |
| first load, phone | 130–220 s | 141 s | held |
| cached load, phone | 30–60 s | 16 s | **missed** (faster) |
| cache size | 150–300 MB | 131.6 MB | **missed** (smaller) |
| jobs from the cache on a reload | 36 of 36 | 36 of 36 | held |
| long tasks > 100 ms, cached load | 0–3, < 300 ms | 1 per run, ≤ 296 ms, all at page start | held |
| k after a cached load | identical | identical | held |

The two speed misses are the same mistake: the per-worker majorant build
in the browser was taken as twice its native 10.6 s; the cached loads put
the whole of assembly, majorant included, at about 10–15 s. The size miss:
U-238's product is 24.7 MB, well under the U-235-sized guess.

## Stage records and the nuclear-data demo (gh:#923, 2026-10-16)

**Change.** The maintainer asked that every web demo share one nuclide cache,
"whether keff or simple demo". The Monte Carlo rungs (Run k_eff, Watch, the
pools) and `delta_tracking_web` already shared it. `nuclear_data_web`
included the module but never called it. Now:

- ~~`Nuclide::process_evaluation` (outram-mc-libs) is three public calls in a
  row: `reconstruct_0k` (RECONR at 0 K), `urr_tables` (PURR at
  `URR_TABLE_SETTINGS` = 20 bins, 16 ladders, 2000 samples), and
  `process_from_stages` (the 0 K elastic slice for DBRC, and BROADR).~~
  **CHANGED 2026-10-18 (gh:#931, #932):** that chain is now
  `Nuclide::process_evaluation_pre_931`; the demos use njoy's `process`
  (see the gh:#932 section below).
- The two slow stages have keys of their own (`reconstruction_key`,
  `urr_key`), under the tape's prefix, so a Monte Carlo load's prefix query
  reads them too. ~~A product missing from the cache is made from whatever
  stages are cached, so a held RECONR leaves only PURR and BROADR to run.~~
  **CHANGED 2026-10-18 (gh:#932):** a product is made from a cached RECONR
  only; PURR runs inside `process::cross_sections_at` (after HEATR), and the
  `urr_key` stage is the PURR rung's alone.
- `nuclear_data_web`'s RECONR rung, its BROADR/GROUPR base (0.001) and its
  PURR rung go through the same store. Its worker reads the keys a step may
  use (`NdEngine::cache_keys`) from IndexedDB before the step and saves what
  the step made. Each rung's note says where its product came from.
- **Who saves what.** ~~Every nuclide leaves three records~~ **CORRECTED
  before commit, on a measurement**: kept for every Monte Carlo nuclide, the
  stages took the TRISO rung's cache from 88 MB (11 products) to 307 MB (31
  records), 3.5x and not the 2x guessed, because RECONR at 0 K is far larger
  than the thinned, broadened product. So:
  - The **Monte Carlo demos save products only**, as before. They read
    stages when present.
  - The **nuclear-data demo saves the stages** its rungs show.
  - Its **PURR rung also reads the tables inside a Monte Carlo product** at
    the same temperature (they are PURR at the same settings), at no extra
    storage.
  - What is *not* shared as a result: the RECONR rung after only Monte Carlo
    rungs have run (the 0 K reconstruction they made was not kept).

**Native checks** (release, i9-13900K, 2026-10-16):

- `processed_cache::tests::a_nuclide_made_from_cached_stages_is_the_processed_nuclide_bit_for_bit`:
  U-234 at `Fast`, stages made by a store that keeps them.
  - A Monte Carlo store offered both stages but not the product: identical
    to `Nuclide::from_tape_with_speed` in all 106 322 compared values, both
    stages read, only the product saved.
  - Offered RECONR alone at 600 K: identical in all 106 503 values, RECONR
    read, PURR processed and not saved.
- `the_code_version_pins_the_processing_output` passes unchanged: the stage
  split leaves the processed output of He-4 and U-234 bit for bit as
  recorded under `nd-2026-10-15.1`, so `CODE` is not bumped.
- `nuclear_data_web` `tests::the_nuclear_data_demo_and_the_monte_carlo_demos_share_their_products`,
  U-238 at 293.6 K, 0.001:
  - The Monte Carlo route saved its product only (one record).
  - This demo's PURR rung (16 x 2000), offered it, read its tables from it:
    3 276 curve values identical to an engine that ran PURR.
  - This demo's RECONR rung saved its stage. A Monte Carlo store offered
    only that stage made the product with RECONR read: 3 092 684 words
    identical to the first product.
- `a_cached_nuclide_is_the_processed_nuclide_bit_for_bit` and the engine
  thread test are unchanged: one record per product for the Monte Carlo
  demos, and 11 products for TRISO.

**Browser** (2026-10-16, headless Chromium 154 over CDP, the i9-13900K
unpinned, both demos built by their `build.sh` and served by
`python3 -m http.server`; each case a fresh browser context, so an empty
IndexedDB; times from the worker log lines `processed_cache` writes):

| case | U-238 product made in | Godiva data ready | IndexedDB after |
|---|---|---|---|
| (a) Godiva alone | 41.3 s | 57.9 s | 3 products (U-234 3.0 MB, U-235 57.1 MB, U-238 24.7 MB) |
| (b) nuclear-data RECONR rung, then Godiva | **15.9 s, RECONR read from this browser's cache** | 57.3 s | the same 3 products, and 6 RECONR stages |
| (c) nuclear-data RECONR rung, twice | | | sweep to 0.001: 73.8 s, then **3.6 s** |

- Godiva's wall time does not move in (b), because its three tapes are
  processed on three workers at once and U-235 (53 s) sets it. The U-238
  saving is real but hidden there. It would show on a rung where U-238 is the
  slowest job.
- **Cost, measured.** The RECONR rung saves a stage at each of its six
  tolerances: 6.2, 7.9, 12.0, 18.7, 31.2 and 50.3 MB, 126 MB in all, saved
  only by a browser that opens that rung. RECONR at 0.001 (50.3 MB) is twice
  U-238's broadened product (24.7 MB). That is why the Monte Carlo demos do
  not save stages.
- **A defect this check caught.** The first build of the worker moved the
  store out of the engine while saving. The sweep's next step, sent the
  moment a result arrived, then ran on an empty stand-in store that saved
  nothing: only the 0.3 stage was kept. Fixed before commit, so the store
  never leaves the engine, and re-measured as above.
- **Not checked in a browser:** the PURR rung reading a Monte Carlo
  product's tables (natively tested above). ~~The rung opens at 8 ladders and
  the Monte Carlo settings are 16, so it reads them only once a reader moves
  the slider to 16.~~ **CHANGED 2026-10-17 (maintainer):** the rung opens at
  16 ladders (`engine::PURR_LADDERS`, tied to `URR_TABLE_SETTINGS`), ~~so after
  Godiva (293.6 K) it opens on Godiva's U-238 tables.~~ **CHANGED 2026-10-18
  (gh:#932):** the Monte Carlo products now hold PURR at 64 ladders x 10 000
  samples, which the rung's slider (2-32 ladders, 2000 samples) cannot reach,
  so it no longer opens on Godiva's tables. `PURR_LADDERS` is now a plain 16
  (UI unchanged); the open question is on gh:#932.
- **CHANGED 2026-10-17 (maintainer): only RECONR at 0.001 is saved.** The
  coarser sweep steps are processed and shown but leave no record
  (`DataStore::keeping_stages(BASE_TOL)`), so the rung's storage is the
  50.3 MB of the 0.001 stage measured above, not 126 MB. Gated natively by
  step 3 of `the_nuclear_data_demo_and_the_monte_carlo_demos_share_their_products`
  (13/13 pass, release); the browser figure is not re-measured.

## The pool on njoy's `process` path (gh:#932, epic #925 E, 2026-10-18)

**Change.** The store's product is `process::reconstruct` then
`process::cross_sections_at` at `demo_settings` (NJOY's defaults: PURR 20
bands / 64 ladders / 10 000 samples after HEATR, the PENDF-rounded 0 K
elastic over the whole range), carried over by
`ProcessedEvaluation::from_cross_sections`. That is route 4
(`Nuclide::process_evaluation`, gh:#931) and the CLI's path. Retired from the
demos: the pre-#931 chain (`process_evaluation_pre_931`: `reconstruct_0k`,
`urr_tables` at 20 / 16 / 2000 without HEATR, `process_from_stages`) and
`pre931_nuclide`. RECONR at 0 K remains a cached stage; PURR is no longer a
product stage. `CODE` is `nd-2026-10-18.1`; the key names the settings
(`|purr=20,64,10000|heatr=1|e0k=1`), so no browser reuses a pre-#932
product.

**Methodology.**

- **Native identity** (release, i9-13900K): `processed_cache::tests::
  the_store_is_route_4_and_process_bit_for_bit`: He-4 and U-234 at 293.6 K,
  tolerance 1e-3, product words from the store, from route 4 and from
  `process::cross_sections`, compared bit for bit; the pre-#931 chain must
  differ. The cache tests (`a_cached_nuclide_…`, `a_nuclide_made_from_cached_stages_…`)
  compare against `Nuclide::from_tape_with_speed` again.
- **Browser against native, every product.** The demo built by
  `web/monte_carlo/build.sh` (wasm-bindgen 0.2.127), served by
  `python3 -m http.server`, page `?rung=htr10&mode=watch&view=core&autostart`
  (the 36 jobs of the whole HTR-10 core, then 1000 × [5 + 20], seed
  20260917), headless Chromium 154 driven by `run_headless_932.mjs`, the
  browser pinned to 3 logical CPUs (`taskset -c 13-15`; the page chose 2
  workers). At the end the script reads every IndexedDB record and
  recomputes its SHA-256 in the page from the stored words. Native: the
  same jobs through `run_job_cached` with a fresh browser store
  (`monte_carlo_web --htr10-product-shas`, `bake::product_records`). Pass:
  every key present on both sides with the same word count and SHA-256.
- **Demo numbers before and after**: the same page built from this commit's
  parent (the pre-#931 chain) and from this commit, each on a fresh profile,
  same settings. Browser and native transport are not bit-identical
  (`htr10_full_core_web` P8: wasm uses Rust's libm), so k is compared
  browser against browser and native against native.
- **No-lag check**: `PerformanceObserver('longtask')` and
  `requestAnimationFrame` gaps on the page; every 2 s the script sends a
  mouse-wheel zoom over the view and times a round trip to the main thread.
  Desktop 1280 × 800 and phone 390 × 844 (device emulation, touch).

**Results.**

| check | result |
|---|---|
| store = route 4 = `process::cross_sections` | He-4 3 228 words, U-234 380 182 words identical; pre-#931 chain another length (2 840, 379 196: its 0 K elastic stops at 25 keV) |
| cached = processed nuclide (U-234, Fast / VeryFast) | 106 654 values identical each; refused records reprocessed |
| HTR-10 core products, browser vs native | **36 of 36 identical** (key, words, SHA-256; stored = recomputed), on the desktop first load, the cached reload and the phone load |
| HTR-10 cached reload | 36 of 36 from the cache, data ready in 27 s, k identical in all 25 generations to the first load |
| phone first load | 36 of 36 identical; first 5 generations identical to the desktop run |

| HTR-10 core, browser, 3 CPUs | before (pre-#931 chain) | after (`process`) |
|---|---|---|
| data ready, first load | 196 s | **337 s** |
| U-235 / U-238 processed in | 68.8 s / 53.4 s | 143.1 s / 82.1 s |
| wasm memory per worker (page's report) | up to 547 MB | up to 752 MB |
| cache size, 36 products | 131.6 MB | 139.7 MB |
| k, 1000 × [5 + 20] | 0.98895 ± 0.01004 | 0.99381 ± 0.00985 |

Native (bake, 1000 × 25 generations, 3 threads, i9-13900K, the machine
shared with other agents' runs): data 68.3 → 97.6 s; k over generations
6–25 0.98980 ± 0.00824 → 1.00193 ± 0.01004. Godiva rung, native
`--headless-keff` defaults: 0.99689 ± 0.00178 → 1.00144 ± 0.00160.

**Interpretation.**

- **The pool's data are the `process` path's, bit for bit**, in the browser
  as natively. This is the acceptance criterion of gh:#932.
- **The k changes are statistics, not physics.** The tables differ (PURR at
  64 × 10 000 instead of 16 × 2000, HEATR's heating column, the 0 K elastic
  over the whole range), so every history decorrelates from generation 0:
  browser +486 ± 1406 pcm (0.3 σ), native HTR-10 +1213 ± 1299 pcm (0.9 σ),
  Godiva +455 ± 239 pcm (1.9 σ, one seed, not paired). gh:#931 predicted
  |Δk| well under 5 pcm from the table change and measured HST-009 at
  −37 ± 32 pcm over 32 seeds; nothing here resolves an effect, and the
  Godiva 1.9 σ single draw should be read against gh:#931's paired-seed
  ensemble, not on its own. **The `process` numbers are the reference**
  (route 4 and the CLI); the old ones came from a chain no other path uses.
- **Cost.** PURR at 64 × 10 000 is about 20 times the Monte Carlo work; the
  first load is 1.7 times longer in the browser (U-235 2.1×) and the
  processing peak 1.4 times the memory. A cached reload is unaffected.
- **Responsiveness.** Compute stays in workers. Main-thread long tasks
  (> 50 ms) and their maximum:

  | run | data phase | run phase | probe round trip, median / max |
  |---|---|---|---|
  | desktop before, first | 1116, max 208 ms | 2840, max 219 ms | 461 / 651 ms (data), 216 / 739 ms (run) |
  | desktop after, first | 474, max 212 ms | 1624, max 288 ms | 146 / 711 ms, 172 / 932 ms |
  | desktop after, cached | 19, max 82 ms | 2368, max 74 ms | 132 / 199 ms, 160 / 231 ms |
  | phone after, first | 2465, max 255 ms | 346, max 190 ms | 328 / 573 ms, 388 / 539 ms |

  No regression from this change: the before and after builds show the same
  pattern. **But neither meets the no-lag rule strictly on this setup**:
  with the browser held to 3 CPUs, 2 workers busy and software rendering
  (SwiftShader), the main thread runs tasks of 150–290 ms throughout,
  where the 2026-10-09 record (4 CPUs) saw one at page start only. The page
  never froze (every probe answered within 0.93 s), but frames of up to
  ~0.25 s are visible lag. Not investigated here (GUI work is the
  maintainer's); noted on gh:#932.
- **Not re-baked:** `htr10/data/core_tracks.bin.z` and the rungs' recorded
  tracks were made on the old data. They are playback only (the recorded
  tracks the view opens on); a re-bake gives other histories.

## Not checked

- A real phone, and a background tab actually discarded by a mobile
  browser: a browser restart on the same profile stands in for both. That a
  mobile browser discards background tabs under memory pressure (forcing a
  full reload, which the cache now survives) is from the browsers' own
  documentation, not measured here.
- A private window (storage that throws): handled by the code (every
  IndexedDB call is inside a promise; a rejection becomes "storage is
  unavailable … processed and not cached"), but not exercised in a browser.
- A corrupt or other-version record in a browser (tested natively only).
- Thermal laws are not in the code-version fingerprint (THERMR/LEAPR take
  tens of seconds each): a change to them must bump `CODE` by hand.
