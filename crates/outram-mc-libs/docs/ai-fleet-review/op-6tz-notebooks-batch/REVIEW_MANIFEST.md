# op-6tz notebooks batch — review manifest

> **⚠️ AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED per `RESPONSIBLE_USE.md`.**
> Everything below (code, tests, V&V notes, measured numbers) is untrusted draft
> material produced by an AI fleet on **2026-07-17**. It has passed
> `cargo build`/`cargo test -p outram-mc-libs --release` but has **not** been
> reviewed by a human, licence-checked line-by-line, or validated against a real
> OpenMC run. Do not describe any of it as validated/trusted until a maintainer
> signs off.

## Scope of this batch

Converted the tractable currently-`#[ignore]`d openmc-notebook tests in
`crates/outram-mc-libs/tests/openmc_notebooks/` into **live** verification tests
where an honest, faithful slice exists against the current API; kept the rest
`#[ignore]` with sharpened, specific reasons + follow-up beads. Touched **only**
`crates/outram-mc-libs/`.

- Notebook reference: openmc-notebooks @ `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (MIT).
- OpenMC C++ reference: `/home/teddy0/Documents/research/openmc/` (cited file:line per port).
- Result gate: `cargo test -p outram-mc-libs --release --test openmc_notebooks`
  → **23 passed; 0 failed; 8 ignored**. Lib suite: **72 passed; 0 failed**.
- Per-notebook CSV comparisons written to
  `verification_and_validation/openmc_notebook_comparisons/` (gitignored,
  reproducible; not committed).

**Important honesty note (applies to every live test in this batch):** the
notebooks' *exact* printed numbers depend on full continuous-energy LWR data we
cannot reproduce with the crate's LOW-tier embedded data (WMP + Watt-collapsed
fast MGXS, free-gas moderation). So — mirroring the existing `flux_spectrum`
precedent ("the notebook's data reference is a *shape*, not a benchmark k") — the
live tests assert **analytic identities, physical bands, or API-response
direction**, not the notebook's absolute values. These verify that the *operation
/ wiring* is correct, which is the real gap each notebook exercises. They are
**not** benchmark-accuracy gates. Benchmark validation against a real OpenMC run
remains open (see op-6tz.26 for the thermal pincell precedent).

---

## LIVE (converted this batch) — 6 notebooks

### 1. `tally_power_normalization` → LIVE (bead op-6tz.22)
- **What:** added `ScoreType::KappaFission` (fission energy deposition) + a
  power-normalization round-trip. Port: `src/tallies/tally_scoring.cpp:1480`
  (`SCORE_KAPPA_FISSION`); recoverable-energy provenance
  `src/nuclide.cpp:335-336` (`fission_q_recov_`).
- **Approximation:** single documented constant Q ≈ 193.4 MeV = `3.0982e-11 J`
  (`Q_FISSION_J`) instead of a per-nuclide Q curve (per-nuclide Q is op-6tz.24).
- **Files:** `src/tally/tally.rs`, `src/tally/scoring.rs`, `src/prelude.rs`,
  `tests/openmc_notebooks/tally_power_normalization.rs`.
- **Measured (2026-07-17, seed-det, 500 part / 20+30 gen):** k_inf = 1.80557 ±
  0.00686; flux_mean = 1.0643e5; fission_mean = 366.73; kappa_mean = 1.1362e-8 J;
  norm factor f = 1.5314e10; **recovered power = 174.000 W (target 174 W, rel err
  0)**; normalized flux = 1.6299e15. Analytic identity `kappa == Q·fission` holds
  to 1.02e-15 rel.
- **Reference:** analytic self-consistency (power round-trip + Q·fission
  identity). Not the notebook's absolute watts.
- **Human-verify:** (a) Q constant choice & units; (b) that
  `kappa_fission = w·d·Σ_f·Q` (track-length) and `w·(Σ_f/Σ_t)·Q` (collision) match
  the C++ intent; (c) power-normalization definition vs the notebook.

### 2. `tally_arithmetic` → LIVE (bead op-6tz.22)
- **What:** new `src/tally/arithmetic.rs` — `DerivedTally{values, std_devs}` with
  elementwise +−×÷, scalar mul, `sum`, `slice`, `from_tally`, first-order
  uncorrelated error propagation. **Provenance:** OpenMC tally arithmetic lives in
  the Python layer (`openmc/tally.py` `__add__/__sub__/__mul__/__div__`,
  `get_slice`, `summation`), **not** C++ — flagged as the sanctioned
  scaffold-new-work path (mirrors documented Python semantics).
- **Files:** `src/tally/arithmetic.rs` (new), `src/tally/mod.rs`,
  `src/prelude.rs`, `tests/openmc_notebooks/tally_arithmetic.rs`.
- **Measured (2026-07-17, 500 part / 20+30 gen, 8 log energy groups, [Flux,Fission]):**
  k_inf = 1.80557 ± 0.00686; sum-of-group-fluxes = 8.4207e3 ± 51.5 matches direct
  sum to rel 0; `(a+b)−b == a` (<1e-9, σ grows); `a*2 == a+a` (means match, σ=2σ
  vs σ√2); `fission/flux` finite/non-negative per group (sample 0.648 cm⁻¹).
- **Reference:** analytic arithmetic identities.
- **Human-verify:** error-propagation formulas (esp. div by near-zero guarding);
  whether `DerivedTally` is the API shape you want long-term.

### 3. `expansion_filters` → LIVE, partial (bead op-6tz.14)
- **What:** `SpatialLegendreFilter{order, axis, min, max}` + `LegendreAxis` +
  Legendre recurrence. Port: `src/tallies/filter_sptl_legendre.cpp:63-88`,
  `src/math_functions.cpp:105-116`. Threaded a `position: Position` field into
  `FilterEvent` and a moment-expansion path into `score_track_length` (a lone
  expansion filter deposits `w·d·Σ_x·P_n(ξ)` into every moment bin — the analogue
  of OpenMC's multi-`(bin,weight)` `get_all_bins`).
- **Gap kept:** Zernike (r-θ) filters NOT ported; expansion filter **combined**
  with other filters not supported. Both remain op-6tz.14.
- **Files:** `src/tally/filter.rs`, `src/tally/scoring.rs`,
  `src/physics/transport_csg.rs`, `src/prelude.rs`,
  `tests/openmc_notebooks/expansion_filters.rs`.
- **Measured (2026-07-17, reflective HEU box, z-Legendre order 4 over [−10,10] cm):**
  k_inf = 1.83164 ± 0.00763; m0 = +3.2133e5 ± 3.17e4 (dominant), m1 = +7.0e1
  (2.2e-4·m0), m2 = −2.24e3 (7.0e-3), m3 = +2.9e1 (8.9e-5), m4 = −3.30e3 (1.0e-2).
  Odd moments statistically zero; even moments ~1% residual.
- **Reference:** analytic symmetry (m0 dominant/positive; |m_n|/m0 small for an
  ~axially-uniform flux).
- **Human-verify:** the `(2n+1)/2` normalization convention (raw moments stored,
  reconstruction-time normalization) vs the C++; the moment-weight scoring path
  is a documented deviation from the single-bin `Filter` contract.

### 4. `post_processing` → LIVE, partial (bead op-6tz.13)
- **What:** new `src/tally/mesh.rs` — `RegularMesh{lower_left, upper_right,
  dimension}` + `MeshFilter{mesh}` (impl `Filter`, bins on segment midpoint).
  Port: `src/mesh.cpp:1473` (`get_index_in_direction`), `:1039`
  (`get_bin_from_indices`), `src/tallies/filter_mesh.cpp:40`.
- **Gap kept:** on-disk StatePoint round-trip NOT ported — the test
  post-processes the in-memory tally directly (op-6tz.22).
- **Files:** `src/tally/mesh.rs` (new), `src/tally/filter.rs`, `src/tally/mod.rs`,
  `src/tally/scoring.rs`, `src/physics/transport_csg.rs`, `src/prelude.rs`,
  `tests/openmc_notebooks/post_processing.rs`.
- **Measured (2026-07-17, 4×4×1 mesh over heterogeneous reflective pincell):**
  k_inf = 1.83164 ± 0.00763; grid max = 2.376e4, min = 1.741e4, total = 3.213e5,
  structure (max−min)/max = 0.267; four corners agree 0.0%, four centres agree
  0.0% (fuel-centred symmetric flux).
- **Reference:** analytic spatial shape + geometric symmetry.
- **Human-verify:** mesh index math vs `mesh.cpp`; that "score on segment
  midpoint" is an acceptable track-length representative.

### 5. `capi` → LIVE, partial (bead op-6tz.20)
- **What:** this crate is a pure in-memory Rust library, not the ctypes C-API. The
  faithful equivalent slice: build geometry+material in memory → `run_keff_csg` →
  read back k±σ **and** a `CellFilter` flux+ν-fission tally (introspection) →
  **edit the material in memory** (×1.5 atom densities) → re-run and assert the
  eigenvalue moves the physically-correct way. Test-only; no `src/` edits.
- **Gap kept:** mid-run batch-stepping / `next_batch` introspection & live edit
  *between* batches (run_keff_csg runs the whole loop internally) — op-6tz.20.
- **Files:** `tests/openmc_notebooks/capi.rs`.
- **Measured (2026-07-17, 500 part / 20+40 gen, finite HEU cylinder R=6 cm, vacuum BC):**
  Run A k = 0.98609 ± 0.00910 (fuel flux 1.30e5 > 0, ν-fis 1.95e4 > 0 read back);
  Run B (1.5× denser) k = 1.31288 ± 0.00984; **Δk = +0.3268 ≈ 24× combined σ**
  (denser fixed-size core ⇒ less leakage ⇒ higher k).
- **Reference:** API-surface + monotonic physical response (no k benchmark).
- **Human-verify:** whether this "in-memory build/run/introspect/edit/rerun"
  framing is an acceptable `capi` analogue, or whether the notebook should stay
  ignored until real batch-stepping exists.

### 6. `candu` → LIVE, partial (beads op-6tz.11 / op-6tz.7)
- **What:** reduced but genuinely-multi-pin CANDU-like cluster: 7-pin concentric
  ring bundle (1 central + 6 ring pins, fuel `ZCylinder` r=0.5 cm) inside a vacuum
  calandria-tube `ZCylinder` (r=2.1 cm), moderator = boolean complement; per-pin
  `CellFilter` flux+ν-fission tally (distribcell analogue). Test-only; no `src/`
  edits.
- **Substitutions (documented honestly):** D₂O → light water H1+O16 (no deuterium
  in CORE WMP); fuel → Godiva HEU densities (natural UO₂ + LOW-tier + light water
  is far subcritical/noisy). Vacuum tube BC (leakage k_eff), not a reflective box
  (keeps histories short).
- **Gaps kept:** `DistribcellFilter` (op-6tz.11) and D₂O data.
- **Files:** `tests/openmc_notebooks/candu.rs`.
- **Measured (2026-07-17, 400 part / 20+40 gen):** k_eff = 0.21486 ± 0.00489
  (small bare cluster, heavy leakage ⇒ deeply subcritical — honest); all 7 pins
  accumulate positive flux (total 2.03e4, central 3.64e3); total fuel ν-fis
  3.52e3 > 0; moderator ν-fis = 0 (fission confined to fuel).
- **Reference:** geometry correctness + physical sanity (no benchmark k — needs
  full CE + D₂O data).
- **Human-verify:** cluster geometry navigates all cells correctly; whether the
  material substitutions are acceptable for a "candu geometry" verification.

---

## STILL IGNORED (honest, this batch's targets) — 4 notebooks

| Notebook | Reason kept ignored | Bead |
|---|---|---|
| `gamma_detector` | **Photon transport is explicitly OUT OF SCOPE** per the crate `CLAUDE.md` (neutron-only; `src/photon.cpp` deferred). Needs a decay-photon source + photon transport that will not be ported here. | op-6tz.19 |
| `shielded_room_weight_window` | Weight-window variance reduction is a substantial unimplemented subsystem, **and** the notebook is absent from openmc-notebooks at the pinned commit (its exact API can't be confirmed). | op-6tz.21 |
| `mg_mode_part_ii` | `run_keff_mg` + `MeshFilter` now exist, but the notebook's **MGXS-from-CE generation** (`mgxs.Library` collapse) is the **njoy track's** responsibility and is unavailable, so the generate-then-run workflow can't be reproduced. | op-6tz.15 / op-6tz.6.3 |
| `mg_mode_part_iii` | Same as ii plus **spatially-varying** MGXS over a lattice + a lattice-aware MG driver with mesh tallies. | op-6tz.15 / op-6tz.6.3 |

**Not this batch's targets (left as-is, per directive):** `cad_based_geometry`
(WON'T PORT — DAGMC), `pandas_dataframes` (ON HOLD), `unstructured_mesh_part_i/ii`
(deferred to op-6tz.32, needs outram-foam mesh).

---

## Known deviations / risks to raise with humans

1. **Trait-object filter system (pre-existing).** `Tally.filters:
   Vec<Box<dyn Filter>>` already uses trait-object dispatch, which the workspace
   `CLAUDE.md` forbids ("no trait objects — use enums"). New filters
   (`SpatialLegendreFilter`, `MeshFilter`) follow the **existing** pattern rather
   than refactoring it. This is a pre-existing deviation now slightly extended —
   a future enum-refactor of the filter system may be warranted (candidate new
   bead). No new `Box<T>` / `dyn` / lifetimes were introduced anywhere else; the
   new modules (`arithmetic.rs`, `mesh.rs`) are `Box`/`dyn`/lifetime-free.
2. **LOW-tier data, not benchmarks.** No live test asserts a notebook's absolute
   number; references are analytic identities / physical bands / response
   direction (see the honesty note above). Benchmark-accuracy validation vs a
   real OpenMC run is out of scope here.
3. **`KappaFission` uses one Q constant** (193.4 MeV), not per-nuclide Q
   (op-6tz.24).
4. **Moment-expansion scoring** deviates from the single-bin `Filter` contract
   (a lone expansion filter deposits into all moment bins); combining an
   expansion filter with other filters is unsupported (op-6tz.14).
5. **`FilterEvent` gained a `position` field** and `score_track_length` gained a
   `position` param; `transport_csg` passes the segment midpoint. `run_keff_csg`'s
   public signature is unchanged, so existing live tests are unaffected.
6. **MAX_EVENTS band-aid (op-6tz.23) untouched** — no new stuck-history guards
   were added.

## Files changed (all under `crates/outram-mc-libs/`)

Source: `src/tally/tally.rs`, `src/tally/scoring.rs`, `src/tally/filter.rs`,
`src/tally/mod.rs`, `src/tally/arithmetic.rs` (new), `src/tally/mesh.rs` (new),
`src/physics/transport_csg.rs`, `src/prelude.rs`.
Tests: `tests/openmc_notebooks.rs` (module map), `tests/openmc_notebooks/{tally_power_normalization,
tally_arithmetic, expansion_filters, post_processing, capi, candu, mg_mode_part_ii,
mg_mode_part_iii}.rs`.
Generated (gitignored, not committed): `verification_and_validation/openmc_notebook_comparisons/*.csv`.
