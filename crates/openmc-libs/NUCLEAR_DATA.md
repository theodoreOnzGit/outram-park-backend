# Nuclear data distribution — design notes

How do we get ENDF-derived neutron cross sections into the Rust OpenMC port,
ideally "hardcoded, no manual ENDF loading", when:

- a full continuous-energy library (e.g. ENDF/B-VIII.0) is **500 MB – several GB**
  depending on the temperature grid, and
- crates.io rejects packages over **~10 MiB** (compressed; the limit can be
  raised by emailing the team, but only modestly, and **not** to hundreds of MB).

This is a real architectural fork. Below is the reasoning and a recommendation.

---

## TL;DR recommendation

1. **Do not put the full library on crates.io.** It is a source registry, not a
   data CDN. Splitting 500 MB into 50+ per-nuclide crates is a maintenance
   nightmare *and* against crates.io policy — they will ask you to stop.
2. **Keep `openmc-libs` data-free** (it already is — good). Data lives in
   separate crates / channels.
3. **Two complementary delivery paths:**
   - **Runtime downloader + cache** for the *full* library (mirrors how OpenMC
     itself ships data) — the default for "I want everything".
   - **Embedded curated *application* subsets** (`include_bytes!` of zstd blobs)
     for "hardcoded, offline, no manual loading" — scoped to ~20–60 nuclides per
     reactor problem, which *does* fit under 10 MB per crate.
4. **You almost certainly do not need to port NJOY** (see below).

---

## First: do you actually need NJOY2016?

Probably not, and this changes the whole problem.

NJOY's job is **raw ENDF → usable cross sections**: resonance reconstruction
(`RECONR`), Doppler broadening (`BROADR`), thermal scattering (`THERMR`),
producing **ACE** files. **OpenMC never reads raw ENDF at run time** — it reads
the *already-processed* ACE/HDF5 library. The Python `openmc.data` layer is what
calls NJOY, offline, to *generate* that library.

So for this port there are two data inputs you could target:

| Input | What you must implement | NJOY needed? |
|---|---|---|
| **Pre-processed ACE / HDF5** (what OpenMC runtime uses) | an ACE (or HDF5) **reader** | **No** |
| **Raw ENDF** | RECONR + BROADR + THERMR (i.e. port NJOY) | Yes |

**Consume pre-processed ACE.** ACE is a documented, mostly-flat format (a header
block + `NXS`/`JXS` index arrays + one big `XSS` float array). A pure-Rust ACE
parser is a few hundred lines and needs **no `libhdf5` system dependency** —
prefer it over HDF5. NJOY (RECONR/BROADR) only becomes necessary if you later
choose the *resonance-reconstruction* size-reduction path (§ "Shrinking the
data", option D), which is an advanced, optional optimisation.

---

## Where the 500 MB comes from (and how to shrink it)

The size is dominated by:

1. **Temperature points.** Production libraries ship many broadened temperatures
   (e.g. 250–2500 K). Each temperature is a full pointwise set. *This is the
   single biggest multiplier.*
2. **Union energy grids.** Tens to hundreds of thousands of points per nuclide in
   the resolved-resonance region.
3. **Secondary distributions.** Angular and energy-out distributions for
   scattering, fission spectra, etc. (much bigger than the 1-D XS curves the
   current `Nuclide` struct stores).
4. **Thermal scattering** S(α,β) tables (graphite, H in H₂O, BeO, …) — bulky.
5. **All ~557 nuclides** in the library, most of which any single problem never
   touches.

### Shrinking levers (multiplicative)

- **A. Curate the nuclide set.** A given reactor needs ~20–60 nuclides, not 557.
  This alone is a **~10–25×** cut. (FHR/TRISO working set: U-235/238, Pu chain,
  C-graphite + S(α,β), FLiBe = Li-6/7, Be-9, F-19, structural Fe/Cr/Ni/Mo, plus
  a fission-product set for depletion.)
- **B. Few temperatures.** Ship 1–3 temperatures, Doppler-broaden on the fly for
  the rest (or accept interpolation). **~5–10×** vs a full T grid.
- **C. Compression.** XS arrays are smooth and compress well; zstd gives
  **~3–5×**. Decompress at load into the `Nuclide` arrays.
- **D. Resonance reconstruction.** Store only **resonance parameters** + a
  background grid and reconstruct pointwise XS (and Doppler-broaden) at load —
  this is exactly NJOY's RECONR/BROADR. **~10–50×** on the resolved region, but
  it is the most work (a real physics port) and is the *only* reason you would
  port NJOY.
- **E. Grid thinning.** Re-grid to a fixed relative tolerance (e.g. 0.1 %).
- **F. Drop what the kernel doesn't use yet.** The current `Nuclide` only holds
  total/elastic/fission/absorption/ν. If full secondary distributions aren't
  consumed yet, don't ship them.

Stacking **A + B + C** on a curated single/few-temperature set comfortably puts a
real reactor working set in the **single-digit MB** range — i.e. *embeddable*.

---

## Options, with verdicts

### ❌ Option 1 — Split the full library into many <10 MB crates
`openmc-data-u235`, `openmc-data-u238`, … ×557, or chunked blobs.
- Technically possible, but: crates.io **discourages** registry-as-CDN and will
  intervene; 50–500 crates to version, publish, and keep in lock-step; brutal
  `cargo` resolution and docs.rs build times; users still download GBs.
- **Verdict: no.** This is the option to avoid.

### ❌ Option 2 — `build.rs` downloads the data at build time
- Breaks **docs.rs**, offline builds, vendoring, and reproducible/sandboxed
  builds (no network during `cargo build` in CI/Nix/etc.).
- **Verdict: no** for a published crate. (Downloading at *run time*, cached, is
  fine — that's Option 4.)

### ✅ Option 3 — Embedded **curated application subsets** (the "hardcoded" path)
A small family of data crates, each a curated nuclide set for a problem class,
baked in as compressed blobs:
```
openmc-data-fhr   = include_bytes!("flibe_graphite_set.zst")   // < 10 MB
openmc-data-lwr   = include_bytes!("uo2_water_set.zst")
```
- Exactly the "no manual ENDF loading, offline by default" experience the user
  wants. `let lib = openmc_data_fhr::load();` → `Vec<Nuclide>`.
- Each crate stays under the limit because it is **curated + few-T + zstd**
  (levers A+B+C). Feature-gate per application so users pull only what they need.
- **Verdict: yes — this is the embedded answer**, for bounded problem sets.

### ✅ Option 4 — Runtime **downloader + cache** (the "full library" path)
A small crate/CLI that fetches the official ready-made libraries (openmc.org
hosts ENDF/B-VII.1, VIII.0, JEFF, … as HDF5/ACE tarballs), verifies a checksum,
and caches them in a platform data dir; honour an `OPENMC_CROSS_SECTIONS`-style
env var / config like OpenMC does.
- This is **how OpenMC itself ships data** — the precedent users already expect.
- No registry abuse, no build-time network, full coverage.
- **Verdict: yes — this is the full-coverage answer.**

### ➖ Option 5 — Resonance reconstruction (store params, rebuild at load)
The size-optimal path for *broad* offline coverage, but it means porting RECONR
(Reich-Moore / MLBW / SLBW) + BROADR (Doppler). Large effort, real physics risk.
- **Verdict: future**, only if Option 3's curated subsets prove too limiting and
  you need broad coverage *offline*.

---

## Recommended architecture (phased)

Crate layout (all depend only downward; none ships the full library):

```
openmc-libs            transport kernels — DATA-FREE (exists)
openmc-data-format     ACE reader (+ optional `hdf5` feature) → Nuclide; zstd codec
openmc-data-downloader fetch+cache official libraries (runtime), OPENMC_CROSS_SECTIONS
openmc-data-<set>      embedded curated subsets: openmc-data-fhr, -lwr, … (< 10 MB each)
xtask / data-baker     OFFLINE maintainer tool: (library + nuclide list + T + tol)
                       → curated, thinned, zstd blob baked into an openmc-data-<set> crate
```

- **`openmc-data-format`** — pure-Rust ACE parser (no libhdf5); HDF5 behind an
  optional feature for those who already have the HDF5 libs. Produces the same
  `Nuclide` the kernel consumes.
- **`openmc-data-<set>`** — the "hardcoded" crates. `include_bytes!` a zstd blob,
  decompress on `load()`. One per problem class; small enough for crates.io.
- **`openmc-data-downloader`** — for users who want the whole library; mirrors
  OpenMC's data-download UX. No crates.io data hosting.
- **`xtask`/data-baker** — runs on a maintainer's machine (where the full library
  already exists), emits the curated blobs. **Never** runs at user build time.

### Phasing
1. **Now:** `openmc-data-format` (ACE reader) + wire `Nuclide::xs_at_energy`
   (the `todo!()` in `nuclide.rs`). Validate against one nuclide read from a real
   ACE file on disk. **No data shipped yet.**
2. **Next:** the data-baker + **one** curated set (`openmc-data-fhr`) embedded and
   loadable offline → the user's "hardcoded, no manual loading" goal, for the
   FHR/TRISO problem that motivates boon-lay.
3. **Then:** `openmc-data-downloader` for full-library users.
4. **Later (optional):** resonance reconstruction (the NJOY port) if broad
   *offline* coverage is needed beyond curated sets.

---

## Direct answers to the questions raised

- **"Hardcode data in Rust, no manual ENDF loading"** → yes, via **Option 3**
  embedded curated subsets (`include_bytes!` + zstd). Scope each crate to an
  application so it fits under 10 MB. Don't try to embed the *whole* library.
- **"crates have a 10 MB limit — split the ENDF up?"** → only split into a few
  **application-curated** subsets, never per-nuclide chunks of the full library.
  For full coverage use the **downloader** (Option 4), not crates.io.
- **"Do we need to port NJOY2016?"** → **No**, if you consume pre-processed ACE
  (write an ACE reader instead). NJOY is only needed for the optional
  resonance-reconstruction path (Option 5).

---

## Precedents

- **OpenMC** — data downloaded separately (`openmc.org` libraries; Python
  `openmc-data-downloader`), never bundled with the source.
- **crates.io policy** — explicitly discourages large binary/data blobs; the size
  cap exists to keep the registry a *source* registry.
- **Rust ecosystem** — crates needing large assets either embed a *small* curated
  artifact (`include_bytes!`) or fetch+cache at runtime; none ship hundreds of MB
  through the registry.
