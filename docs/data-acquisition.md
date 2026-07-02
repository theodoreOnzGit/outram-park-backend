# Data acquisition & fidelity tiers

How OUTRAM PARK gets cross-section data, from the always-offline default up to
full on-device ENDF reprocessing. Companion to `docs/architecture.md` and the
fast-range decision in `docs/keff-doppler-roadmap.md`.

Design principles: the default ships in-crate and needs no network; higher
fidelity is opt-in, downloaded once, and **cached**; every networked path is
pure-Rust and cross-platform (no C build deps); downloaded data is treated as
untrusted until checksum-verified.

## Fidelity tiers

Two tiers: a low-fidelity **offline** tier that ships in-crate, and a
high-fidelity **download** tier.

| Tier | Source | Ships / fetched | Use |
|---|---|---|---|
| **LOW — offline, default** | embedded WMP blob **+ 10-group high-energy augmentation** (125-nuclide CORE) | in-crate `include_bytes!` | complete offline σ(E,T) over the *full* range: WMP CE + analytic Doppler below `e_max`, 10-group multigroup above it |
| **HIGH — opt-in, download** | **pure raw ENDF** | fetched + cached | run RECONR/BROADR/ACER on device → cache the result; authoritative, full continuous-energy fidelity |

**The LOW tier is a single package, not two options.** The WMP blob and the
10-group set are *complementary*, not alternatives: WMP covers 1e-5 eV → `e_max`
(the resonance/thermal range, with analytic Doppler — its strength); the 10-group
set **augments** it above `e_max` (the smooth fast range WMP does not reach; see
"Multigroup augmentation"). Together they give the offline build the full
1e-5 eV → 20 MeV span with zero network.

HIGH tier rationale: ENDF is the authoritative evaluation, the download is KB–MB
(vs ~120 MB/nuclide for pointwise h5), and the njoy pipeline to process it already
lives in this crate. Pay the reconstruction CPU once, cache the output.

## Distribution of the downloadable data — decided

Manifest-driven, supporting both sources per entry so integrity never depends on
where the bytes came from:

- **Canonical pinned mirror: a separate git-lfs repo** (own it, e.g.
  `outram-park-data`). A commit hash pins the *entire* dataset → strong
  reproducibility. git-lfs dedupes and versions the binary tapes. (Cost: lfs
  storage/bandwidth quota; users need git-lfs.)
- **Provenance / fallback source: official sites** (NNDC, IAEA, openmc.org) via
  pinned URLs. Authoritative, no hosting burden, but URLs rot and availability is
  out of our control.
- **Manifest** entry carries `{ nuclide, evaluation, url(s), sha256, size, format }`.
  `sha256` is the source of truth; a mismatch is rejected regardless of origin.
  Default to the pinned git-lfs mirror for reproducibility; official URL as
  documented fallback. ENDF/B is public data, so mirroring is fine — keep the
  evaluation citation in the manifest.

## Networked stack (pure Rust, cross-platform)

- HTTP: **`ureq`** — synchronous, no async runtime infecting a data-prep step.
- TLS: **`rustls`** — pure-Rust; no OpenSSL system dependency (the usual
  cross-platform failure point on Windows/macOS).
- Cache dir: **`directories`** — XDG (Linux) / `AppData` (Windows) /
  `~/Library/Caches` (macOS). Env override `OUTRAM_PARK_DATA_DIR` for HPC/containers.
- Decompress: `flate2`/`miniz_oxide` + `tar` (pure Rust). Prefer `.tar.gz` sources.
- Parse: existing ENDF parser, ACE reader/writer (ACER), `hdf5-pure` for h5.
- All behind a `net-fetch` feature (or a separate companion crate) so the default
  lean build pulls none of it.

## Safety checks — especially for parallel runs

The fetch/cache layer must be correct under multiple processes (parallel sim
runs, HPC array jobs) hitting the same cache concurrently.

1. **Atomic publish, never write the final path directly.** Download to a unique
   temp file `<name>.<pid>.<rand>.part` in the cache dir, `fsync`, then atomic
   `rename()` into the final name. `rename()` is atomic on the same filesystem on
   Linux/macOS/Windows. Readers therefore only ever observe complete files — a
   crashed/killed download leaves an orphan `.part`, never a corrupt "valid" file.
2. **Per-artifact advisory lock (anti-thundering-herd).** Before downloading,
   acquire an exclusive OS file lock (`fs2`/`fd-lock`) on `<name>.lock`. N parallel
   processes wanting the same artifact → one downloads, the rest block on the lock.
3. **Double-checked acquire.** check cache (present + checksum ok? → use) → else
   take lock → **re-check** (a peer may have finished while we waited) → download →
   verify → atomic rename → release lock. The re-check after locking is what makes
   parallel runs do the work exactly once.
4. **Checksum on both sides.** Verify `sha256` after download *and* before use.
   On mismatch: discard, re-fetch once, then **hard-fail** — never hand a bad file
   to the solver. Guards partial/corrupted/tampered/mirror-rot bytes.
5. **Stale-artifact sweep.** On startup, remove orphan `.part`/`.lock` older than a
   threshold (from crashed runs) so they don't wedge future fetches.
6. **Disk-space precheck.** Manifest carries expected size; verify free space
   before starting a large download; fail early with a clear message.
7. **Resumable large downloads.** HTTP range requests to resume an interrupted
   `.part` (h5 ~120 MB); restart cleanly if the server ignores ranges.
8. **Fetch is a setup-phase gate, not in the hot loop.** Downloading/cache-loading
   happens once in a single-threaded setup phase; the resulting read-only tables
   are shared to worker threads via `Arc<T>` (matches the workspace rule: `Arc<T>`
   for read-only data). Never fetch from inside the timestep/particle loop.
9. **Provenance-keyed cache for processed results.** Cache the ENDF→ACE/blob output
   under a key of `(nuclide, evaluation, temperature, processing-params-hash,
   njoy-code-version)`, with a small sidecar manifest. A njoy version bump or a
   changed processing parameter invalidates stale cached results automatically.
10. **Hardened parsers.** Downloaded ENDF is untrusted input: a malformed record
    must fail cleanly, not runaway-allocate. Bounded allocations + cursor-advance
    checks (this is the same failure mode the crate's 12 GB unit-test cap guards).

## Multigroup augmentation above `e_max` (part of the LOW tier) — single Watt-weighted MGXS

This is **not a separate tier** — it is the upper half of the LOW-tier package,
extending the embedded WMP blob past its ceiling so the offline build spans the
full energy range. The high-energy region (above each nuclide's WMP `e_max`;
U-233 600 eV, U-235 2.25 keV, U-238 20 keV) is smooth, so a **coarse multigroup**
set per reaction is enough.

**Design decision (2026-07-02): keep it deliberately simple — one Watt-spectrum
weight, no Boltzmann.** This is the *low-fidelity, high-speed* fallback; it is not
trying to be accurate, it is trying to be cheap. So:

- **Single weighting: the Watt fission spectrum.** Group constants are collapsed
  once, offline, as σ_g = ∫σ(E)χ(E)dE / ∫χ(E)dE with χ the Watt form. No second
  (1/E / Maxwellian) endpoint, no hardness-parameter interpolation between spectra.
- **No self-shielding.** There is *no* Boltzmann solve, *no* Bondarenko dilution,
  *no* URR probability-table treatment. A caller who needs any of that uses the
  HIGH tier (on-spectrum ENDF reprocessing) instead.
- **Runtime lookup is a piecewise-constant group index** — temperature-independent.
  The fast range where a bare sphere lives is smooth, so a hard-spectrum-weighted
  constant is adequate and fast.

Implemented and baked (`src/nuclear_data/mod.rs`, `examples/bake_mgxs.rs`):

- **Group structure:** `FAST_GROUP_COUNT` = **10 log-spaced groups** from each
  nuclide's WMP `e_max` up to `ENDF_MAX_ENERGY_EV` = **20 MeV** (standard ENDF/B
  neutron sublibrary ceiling). `Mgxs::fast_group_bounds(e_lo, e_hi)` builds them.
- **Source = RECONR pointwise output from the local ENDF/B-VIII.0 library.** The
  `bake_mgxs` example reads the tapes under `ENDF-B-VIII.0/neutrons/`, reconstructs
  the **MF=3 background only** (`reconr::reconr_background` — no MF=2 resonance
  reconstruction, which is irrelevant above `e_max` and lets us skip formats the
  port does not yet handle, e.g. VIII.0's LRF=7 R-Matrix Limited), reads each
  `e_max` from the embedded WMP CORE library, parses ν̄(E) from MF=1/MT=452
  (`NuBar::from_endf`), and Watt-collapses via `Mgxs::collapse_from_reconr`.
- **Container + embed:** packed into **MGXL v1** (`MgxsLibrary`, flat uncompressed
  LE) as `src/data/mgxs_core.mgxl` (~60 KB, 123 nuclides), shipped via the
  always-embedded `MgxsLibrary::core()`. Non-resonant nuclides whose WMP already
  spans 20 MeV (H-1, He-4) have no fast gap and are excluded.
- **Runtime:** `Mgxs::micro(e)` is the O(log n) piecewise-constant group lookup,
  temperature-independent; wired into `XsProvider::Mgxs`.

If a moderated-spectrum use case ever needs it, a softer weight is a *new* baked
set, not added complexity in the runtime path — the collapse stays single-spectrum.

## Open questions

- git-lfs mirror hosting cost vs relying on official URLs + local cache only.
- Whether the transport seam (CE below `e_max` on WMP, MG above) lives in
  `openmc-libs` or is a combined `XsProvider` variant here.
