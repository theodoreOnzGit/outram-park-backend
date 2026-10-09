# Prediction: the shared nuclear-data cache in the browser (gh:#818)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** Research, education and V&V only; not
> for any operational use. AI-assisted (Claude Opus 5.5 in Claude Code), not
> yet reviewed by the maintainer.

Written 2026-10-09, before any browser measurement, and posted the same
hour as a comment on gh:#818. The record is [`README.md`](README.md).

## What is measured

The `htr10` rung's whole-core view (`?rung=htr10&mode=watch&view=core&autostart`),
in headless Chromium held to 4 logical CPUs (`taskset`), each check in a
fresh browser process that is closed after it:

1. **First load**: a new, empty browser profile. Every one of the 36 jobs
   is processed and saved to IndexedDB.
2. **Cached load**: the same profile, browser restarted (as a reload or a
   discarded tab would). Every job should be read back.

Both on a desktop viewport (1280 × 800, pool sized by the page) and an
emulated phone (390 × 844, mobile, pool sized by the page: 2 workers).

## Predictions

| quantity | prediction | reasoning |
|---|---|---|
| first load, desktop, data ready | 90–150 s | gh:#786 measured 67–128 s on 2–4 workers with 5 cores; 4 cores here, plus SHA-256 of every tape (about 150 MB inflated, about 1 s per worker in wasm) and the IndexedDB writes |
| cached load, desktop, data ready | 25–50 s | the products are read, not processed; but every worker still downloads (HTTP cache) and inflates every tape, hashes its own jobs' tapes, rebuilds 38 nuclides (`from_processed`) and the bed majorant (10.6 s natively per worker, likely 2× in wasm). So 2–4× faster, **not** instant |
| first load, phone (2 workers) | 130–220 s | gh:#786's 2-worker desktop run took 128 s; emulation does not slow the CPU |
| cached load, phone | 30–60 s | as the desktop cached load, with 2 workers relaying |
| cache size | 150–300 MB | U-235's product alone is 7.1 M words (57 MB, gh:#786); U-238 and the five laws next; the light nuclides are small |
| every job reported "from this browser's cache" on the cached load | 36 of 36 | same tapes, same settings, same code version |
| long tasks > 100 ms on the page during the cached load | 0–3, the longest < 300 ms | the page only relays (8 MB a frame at most) and reads the cache index; products arrive in a burst, which is the risk |
| k of a live run after a cached load | identical to a first-load run, bit for bit, generation by generation | the cached products are the processed products bit for bit (native tests) |

## What would refute it

- A cached load that processes any job (a key that does not match itself).
- A cached load no faster than a first load (the cost is elsewhere).
- Any difference in k between a first-load and a cached-load run.
- A long task over 1 s on the page during a cached load (the no-lag rule).
