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
  (`examples/common/processed_cache.rs`, `nd-2026-10-09.1`). Each record
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
