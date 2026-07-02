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

## Multigroup augmentation above `e_max` (part of the LOW tier) — two-spectrum interpolation

This is **not a separate tier** — it is the upper half of the LOW-tier package,
extending the embedded WMP blob past its ceiling so the offline build spans the
full energy range. The high-energy region (above each nuclide's WMP `e_max`;
U-233 600 eV, U-235 2.25 keV, U-238 20 keV) is smooth → **10 groups** per reaction
is enough. Rather
than one spectrum-locked set, embed **two** pre-weighted endpoints per nuclide and
interpolate between them by spectral hardness:

- **HARD endpoint — Watt (fission) weighted.** Unmoderated / fast-reactor extreme.
- **SOFT endpoint — 1/E slowing-down weighted.** Moderated / thermal-reactor
  extreme. (Note: in this band the *soft* shape is 1/E, **not** a Maxwellian — a
  Maxwellian peaks at thermal ~0.025 eV, far below `e_max`. A Maxwellian+1/E+fission
  composite is an option, but the fast-band shape it contributes is essentially
  1/E.) This is the user's "Boltzmann-weighted" endpoint, corrected to the shape
  that is physical in the fast region.

Interpolate group σ for an in-between reactor by a hardness parameter λ ∈ [0,1]:

    σ_g(λ) = (1−λ)·σ_g^soft + λ·σ_g^hard      (linear first cut; log-interp optional)

- **λ from a physical spectral index** — median fission energy, fast/thermal flux
  ratio, or moderator-to-fuel ratio; *or* measured self-consistently from the sim's
  own tally spectrum and the set re-collapsed (iterate to convergence).
- **Caveats.** Group σ is a flux-weighted average, so it is not strictly linear in
  hardness; the two endpoints *bound* the true value but the interior is
  approximate. For 10 smooth fast groups this is a defensible one-parameter model
  for the offline LOW tier — not a replacement for the HIGH tier's on-spectrum
  ENDF reprocessing. URR self-shielding (Bondarenko) still applies just above
  `e_max` regardless.

## Open questions

- λ mapping: expose a single user "spectral hardness" knob, infer from
  moderator-to-fuel ratio, or always iterate from the tallied spectrum?
- Soft endpoint: pure 1/E, or the Maxwellian+1/E+fission composite (matters only
  near the bottom of the fast band)?
- git-lfs mirror hosting cost vs relying on official URLs + local cache only.
