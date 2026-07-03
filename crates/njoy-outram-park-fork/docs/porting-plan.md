# njoy-outram-park-fork — porting plan

Reference material for the Rust port of NJOY2016. Read on demand — the mandatory
license and design rules live in `CLAUDE.md`.

Upstream Fortran source (the golden oracle): **`../../../NJOY2016`** (relative to
this crate), i.e. `/home/teddy0/Documents/research/NJOY2016`, version **2016.79**
(commit `ac5adf5`). ~120,000 lines of Fortran 90 across 39 source files.

---

## 1. Why this port exists

OpenMC does not read ENDF evaluations directly — it reads **ACE** libraries.
NJOY is the canonical tool that processes raw ENDF data (resonance parameters,
covariances, thermal scattering laws) into ACE. `openmc-libs` therefore depends
on NJOY *indirectly*: the data it ingests must first pass through an NJOY
pipeline. Porting NJOY to Rust removes the Fortran/CMake build dependency from
the OUTRAM PARK data-prep chain and brings the same benefits as the rest of the
suite (static navigability, `uom` units, Cargo).

We do **not** need all 39 modules to satisfy OpenMC. The continuous-energy ACE
path is a small subset; everything else is later or never.

---

## 2. The OpenMC ACE pipeline (MVP)

The minimal module chain that yields an OpenMC-ready ACE file:

```
MODER  → RECONR → BROADR → ACER
         (+ optional: HEATR, GASPR, PURR, THERMR before ACER)
```

| Step   | Does | Required for OpenMC? |
|--------|------|----------------------|
| MODER  | ASCII ⇄ NJOY binary tape conversion | plumbing — yes |
| RECONR | reconstruct pointwise σ(E) from resonance params | **yes** |
| BROADR | Doppler-broaden σ(E) to temperature | **yes** |
| HEATR  | heating (KERMA) + damage cross sections | optional (heating) |
| GASPR  | gas-production cross sections | optional |
| PURR   | unresolved-resonance probability tables | optional (self-shielding) |
| THERMR | thermal scattering S(α,β) → cross sections | optional (thermal scatterers) |
| ACER   | write ACE library | **yes** |

---

## 3. Module → Fortran source map

Line counts are approximate (from `wc -l src/*.f90`). Phase column drives order.

### Core infrastructure (Phase 1)

All **39** NJOY2016 source files are accounted for in the four tables below.
Status legend: ✅ done · 🟡 partial · ⏳ scaffolded/stub · ⬜ not started · ➖ subsumed (no standalone module) · ❌ out of scope for OUTRAM PARK.

| Rust module | Fortran file | LOC | Status | Notes |
|---|---|---|---|---|
| `common::phys` | `phys.f90` | ~150 | 🟡 | physical constants → `uom`; ported on demand |
| `common::mathm` | `mathm.f90` | ~1.4k | 🟡 | special functions, small linear algebra; pieces ported as needed |
| `common` (util) | `util.f90` | ~2k | 🟡 | record I/O helpers, interpolation, `error`/`mess` → `Result` |
| `common` (io) | `mainio.f90`, `locale.f90` | ~0.3k | ➖ | unit numbers / formatting → owned config, not literal ports |
| `endf` | `endf.f90` | ~1k | 🟡 | in-memory ENDF tape model + record parsing (tape/records live) |
| `modules::moder` | `moder.f90` | ~0.6k | ⬜ | ASCII ⇄ binary tape conversion |
| `driver` | `main.f90` | ~0.5k | ⬜ | card-input reader → thin Rust `match` over `NjoyModule` |
| — | `vers.f90` | trivial | ➖ | version string; not ported |

### ACE pipeline (Phases 2–4)

| Rust module | Fortran file | LOC | Phase | Status |
|---|---|---|---|---|
| `modules::reconr` | `reconr.f90` | 5.7k | 2 | ✅ resonance reconstruction |
| `modules::broadr` | `broadr.f90` | 2.0k | 2 | ✅ Doppler broadening (SIGMA1) |
| `modules::heatr` | `heatr.f90` | 6.3k | 3 | ⬜ heating/KERMA + damage (ACE 4e depends on it) |
| `modules::gaspr` | `gaspr.f90` | ~0.6k | 3 | ⬜ gas production |
| `modules::purr` | `purr.f90` | 2.9k | 3 | ⬜ URR probability tables |
| `modules::thermr` | `thermr.f90` | 3.4k | 3 | 🟡 MF=7 reader + coherent/incoherent elastic + inelastic physics; no module driver |
| `modules::unresr` | `unresr.f90` | ~1.8k | 3 | ⬜ URR effective XS (PURR precursor) |

ACER is not one file — it is a family. The Phase-4 sub-blocks (see §4) map to:

| Rust module | Fortran file | LOC | Status |
|---|---|---|---|
| `modules::acer` (driver) | `acer.f90` | ~1k | 🟡 CE cross-section + elastic/discrete angular + partial energy dists |
| `ace` (fast CE) | `acefc.f90` | **19.7k** | 🟡 ESZ/MTR/SIG + LAND/AND + partial LDLW/DLW; the bulk still open |
| `ace::thermal` | `aceth.f90` | ~2k | 🟡 thermal `…t` tables: elastic blocks done; ITXE secondary dists open |
| — | `acecm.f90` | ~1k | 🟡 ACE shared utilities; ported on demand |
| — | `acepn.f90` | 3.8k | ⬜ photonuclear ACE |
| — | `acepa.f90` | ~2k | ⬜ photoatomic ACE |
| — | `acedo.f90` | ~1k | ⬜ dosimetry ACE |
| `wmp` | *(MIT WMP_Library — not NJOY)* | — | 🟡 4g evaluator + Faddeeva + `load_h5` done; `from_blob` TODO (`src/wmp.rs`) |

### Multigroup & covariance (Phase 5 — not needed by OpenMC CE)

| Rust module | Fortran file | LOC | Status |
|---|---|---|---|
| `modules::groupr` | `groupr.f90` | 12.7k | ⬜ multigroup neutron/photon XS |
| `modules::gaminr` | `gaminr.f90` | ~2k | ⬜ multigroup photon interaction |
| `modules::errorr` | `errorr.f90` | 11.2k | ⬜ multigroup covariance matrices |
| `modules::covr` | `covr.f90` | ~3k | ⬜ covariance output/plotting |
| `modules::leapr` | `leapr.f90` | 3.6k | ⬜ *generate* MF=7 S(α,β) (upstream of THERMR) |
| `modules::samm` | `samm.f90` | 7.2k | ⬜ R-matrix (RML); shared by reconr/unresr |

### Formatters, plotting, misc (Phase 6 — lowest priority)

Output formats for codes OUTRAM PARK does not target — port only on demand (all ⬜):
`dtfr.f90`, `ccccr.f90`, `matxsr.f90`, `resxsr.f90`, `powr.f90`, `wimsr.f90`,
`mixr.f90`, `plotr.f90`, `viewr.f90`, `graph.f90` (low-level plotting shared by
`plotr`/`viewr`/`covr`).

> Note: `samm.f90` (Reich–Moore / R-matrix-limited resonance formalism) is shared
> by RECONR and UNRESR. It may need to move earlier than Phase 5 if a target
> evaluation uses the RML format — check the evaluation before Phase 2.

---

## 4. Phased order

- **Phase 0 — scaffold + license compliance.** ✅ this commit: crate skeleton,
  `LICENSE.njoy`, `NOTICE`, module stubs returning `NjoyError::NotPorted`.
- **Phase 1 — infrastructure.** `endf` tape model + `common` (phys, mathm, util,
  io) + `MODER`. Nothing computes physics yet, but tapes round-trip. Gate:
  read a reference ENDF tape and write it back byte-identical.
- **Phase 2 — RECONR + BROADR.** First real cross-section output. Gate: pointwise
  σ(E) at 0 K then broadened, matching upstream within tolerance.
- **Phase 3 — HEATR / GASPR / PURR / THERMR (+ UNRESR).** ACE prerequisites for
  heating, gas, self-shielding, thermal scattering.
- **Phase 4 — ACER.** Emit an ACE file OpenMC loads and runs. **This is the
  milestone that satisfies the OpenMC dependency.** Largest single phase
  (acefc.f90 alone is ~20k lines) — split by ACE block (nu, angular, energy
  distributions, photon production, …). Progress:
  - **4a — cross-section core.** ✅ `src/ace/` ports `aceout`/`change` (Type-1
    ASCII format) + the cross-section part of `acelod`: the **ESZ** block (union
    energy grid, total = elastic + Σ partials, disappearance, elastic, heating)
    and the **MTR/LQR/TYR/LSIG/SIG** reaction blocks, from a `ReconrResult`.
    Exposed as `AceTable::from_reconr` / `write_type1` and via
    `NuclearDataLibrary::write_ace`. RECONR now threads each reaction's QI
    through `ReconrSection.qi` for the LQR block. Gate: NXS/JXS/XSS
    self-consistency + Type-1 round-trip (`tests/acer.rs`).
  - **4c — elastic angular distribution (LAND/AND).** ✅ `src/ace/angular.rs`
    ports the MF=4/MT=2 path (`topfil`/`ptleg`/`pttab`, `newfor=1`): parses LTT=1
    (Legendre), 2 (tabulated), 3 (both), converts each incident energy to the ACE
    tabulated-cosine form (`JJ=2`, μ/pdf/cdf), and appends LAND + AND via
    `from_reconr_with_angular`. `write_ace` wires it through the tape. Gate:
    AND-block round-trip + cdf∈[0,1] monotone + ⟨μ⟩ = a₁ physics check
    (`tests/acer.rs`, `angular.rs` unit tests). Only **elastic** for now.
  - **4b — fission ν̄ (NU block).** Needs MF=1/MT=452 (and 455/456). Not started.
  - **4d — energy distributions (LDLW/DLW) + non-elastic angular.** ⏳ in
    progress. `src/ace/energy.rs` ports the **MF=5 LF=1 → ACE Law 4** conversion
    (continuous tabular E', the fission χ(E→E') path; faithful to `acelf5`):
    `parse_mf5_law4` (fission χ). This is a **separate, ACE-file-writer-specific**
    MF=5 parser from `nuclear_data::secondary::FissionSpectrum::from_endf_mf5`
    (Priority 1 above), which is consumed directly by `openmc-libs` and covers
    LF=1/7/9/11 + NK>1 mixtures — `ace/energy.rs` here still only handles LF=1
    (the ACE Law-4 conversion has no equivalent for Law 7/9/11/44 yet). Plus
    **MF=6 LAW=1** (`parse_mf6_law1_neutron`):
    the neutron (ZAP=1) energy pdf `f₀` + yield/frame.
    **DLW block wired** (`build_emissions` + `AceTable::from_reconr_full`): the
    neutron-producing reactions get a **TYR** yield and a **DLW** law — **Law 3**
    for discrete inelastic levels (MT51–90, two-body from Q+AWR) and **Law 4** for
    the continuum / (n,xn) (MT16/17/91/5 from MF=6). **NXS(5)=NR**, **LDLW**,
    **DLW**, and the **LAND** array (NR+1) are all filled; `write_ace` builds them
    from the tape. Gate: DLW-walk validation (IDAT=header+9, cdf 0→1 monotone,
    TYR↔producer) + full-table Type-1 round-trip on U-235 (`tests/acer.rs`). The
    U-235 table now emits with NR=43 (39 Law 3 + 4 Law 4).
    **Discrete-level angular** is also wired: MT51–90 (Law 3) carry their MF=4
    angular distribution into the AND block (LAND locator > 0), via the generalised
    `append_angular_blocks`; U-235 shows many anisotropic producers.
    **Remaining 4d gaps**: fission secondaries (MT18 — needs the ν̄/NU block, 4b);
    **continuum** correlated angle (MF=6 producers still isotropic — LANG=1
    Legendre → Law 61, LANG=2 Kalbach → Law 44); and MF=6 **LAW=2** (two-body) /
    **LAW=6** (phase space), currently skipped gracefully (e.g. H-2 (n,2n)).
  - **4e — heating (ESZ column 5).** Zero until HEATR (Phase 3) lands.
  - **4f — thermal S(α,β) ACE table.** ⏳ **scaffolded** (`src/ace/thermal.rs`,
    stub returning `NotPorted`). Writes the `…t` thermal-scattering tables
    (graphite, H₂O, D₂O, ZrH, …) with the thermal NXS/JXS layout
    (ITIE/ITIX/ITXE inelastic; ITCE/ITCX/ITCA coherent-elastic Bragg;
    ITCEI/ITCXI/ITCAI incoherent-elastic) — ports `aceth.f90` + the thermal
    driver in `acefc.f90`. **Scheduled after 4a–4e** (the CE library) finish.
    Prerequisite: **THERMR** (`thermr.f90`, Phase 3) to turn MF=7 S(α,β) into the
    thermal cross sections / energy-angle distributions this consumes; optionally
    **LEAPR** (`leapr.f90`, Phase 5) to *generate* MF=7 when an evaluation lacks
    it. See the `src/ace/thermal.rs` module docs for the full TODO list.
    **Progress (THERMR now open):**
    - **MF=7 reader** — `src/thermal/mf7.rs` parses MT=2 coherent-elastic Bragg
      `S(E)` and MT=4 incoherent-inelastic `S(α,β)` (B-const + β/α grids), tested
      on the Al-27 ENDF/B-VIII `tsl` fixture.
    - **Coherent-elastic** — `src/thermal/coherent.rs`: σ_coh(E)=S(E)/E (Bragg
      sawtooth) + discrete reflection cosines `μ_i=1−2E_i/E` and weights, tested.
    - **Incoherent-inelastic** — `src/thermal/inelastic.rs`: the double-differential
      `d²σ/dE'dμ = (σ_b/2kT)·√(E'/E)·S̃(α,β)·exp(−β/2)` from S(α,β), and the
      integrated `σ(E→E')` (∫dμ) and `σ_inel(E)` (∫dE' on the table's β grid, the
      physically-dense quadrature). Validated on Al-27: σ_b≈1.45 b, and σ_inel
      rises from the cold-crystal thermal value toward the free-atom limit
      σ_free=B(1)≈1.35 b near 1–2 eV. Deep-downscatter overflow (S̃ below the
      numerical floor while exp(−β/2) grows) is guarded.
    Still needed: the secondary energy-angle **distributions** (equiprobable
    cosines / Legendre for the ACE ITXE block) and the `aceth.f90` thermal-ACE
    writer (ITIE/ITIX/ITXE + ITCE/ITCX/ITCA blocks).
  The written file now carries cross sections + the elastic angular distribution.
  Until 4b/4d/4e exist it is still not a complete transport library (no secondary
  energy distributions, so inelastic/fission collisions can't be followed); 4f
  (thermal S(α,β)) is a separate table type layered on top once 4a–4e are done.
  - **4g — Windowed Multipole (WMP) import.** ✅ **done** (`src/wmp.rs`).
    **Independent MIT CRPG work — NOT NJOY/LANL.** Reads the **MIT** `WMP_Library`
    (<https://github.com/mit-crpg/WMP_Library>, MIT-licensed) HDF5 multipole data:
    complex poles/residues + windows enabling *analytic* on-the-fly Doppler
    broadening (Faddeeva `w(z)`), a parallel alternative to the pointwise
    ACE/PENDF representation. **Done:** `faddeeva` (pure-Rust Weideman, no FFI,
    validated vs scipy `wofz`); `WindowedMultipole::evaluate` (faithful to OpenMC
    `wmp.cpp`); `load_h5` (behind the `wmp-hdf5` feature, pure-Rust `hdf5-pure`);
    real U-238 Doppler confirmed (`tests/wmp_u238.rs`); the **WMPB v1** per-nuclide
    blob codec (`to_blob`/`from_blob`) and the **WMPL v1** multi-nuclide container
    (`WmpLibrary::pack`/`from_blob`) — both round-trip tested, corruption-hardened.
    **CORE set embedded and shipped**: 125 reactor-grade nuclides baked into
    `src/data/wmp_core.wmpl` (4.70 MB deflated) via `examples/bake_wmp.rs`, loaded
    zero-dependency (no HDF5 at runtime) through `WmpLibrary::core()`
    (`OnceLock` + `include_bytes!`, always embedded, no feature gate). 11/11 wmp
    unit tests pass, including `embedded_core_library_loads_and_evaluates`.
    Credit MIT CRPG and Josey/Romano/Forget/Smith: `LICENSE-WMP` (MIT) + a WMP
    entry in `NOTICE`, distinct from the NJOY/BSD notice. **Remaining:** the
    **EXTENDED** set (298 more nuclides, 9.64 MB raw) is not yet packaged — planned
    as a **separate sibling crate** (not embedded here) so the njoy crate stays
    small; no sibling crate exists yet. See the `src/wmp.rs` module docs and
    [`project_wmp_embedding_plan`] in the assistant's memory for the full sizing
    rationale.
- **Phase 5 — multigroup/covariance** (GROUPR, ERRORR, …): only if OUTRAM PARK
  needs deterministic or sensitivity workflows.
- **Phase 6 — formatters/plotting:** on demand only.

---

## 5. Fortran → Rust translation conventions

NJOY is old-style Fortran. Recurring patterns and how they map:

- **`common` blocks / module variables → owned structs.** No global mutable
  state. A module's working set becomes a struct passed by `&mut`. Read-only
  tables shared across threads use `Arc<T>` (per workspace rules).
- **Scratch "tapes" on logical units → typed in-memory data or explicit files.**
  Replace the `nin/nout/nscr` integer-unit indirection with owned `endf::Tape`
  values; only hit disk at the driver boundary.
- **`go to` control flow → structured loops / early `return` / `match`.** Keep
  the numerics line-traceable to the Fortran, but never reproduce a `goto` with a
  labelled loop hack where a `while`/`for` reads clearly.
- **Implicit typing & 1-based, column-major arrays → explicit types + `ndarray`.**
  Watch the index base off-by-one; `ndarray` is row-major, so transpose intent
  carefully for 2-D data, or keep 1-D flat with explicit strides.
- **`real(kr)` (kr = 8) → `f64`.** Single-precision `real` is not used for the
  physics; confirm per-variable when in doubt.
- **`error(...)` (abort) → `Err(NjoyError::...)`.** `mess(...)` (warning) →
  `log`/return-with-warning, never a silent print to a global unit.
- **Units:** put `uom` on public-API energies (eV/MeV), temperatures (K), and
  cross sections (barns) — spell the unit out in the doc comment even though
  `uom` enforces it. Internal hot loops may use raw `f64` for speed, converting
  at the boundary.

Do not "improve" the algorithms during translation. Port faithfully first,
verify against the oracle, *then* refactor — otherwise discrepancies are
impossible to localise.

---

## 6. Verification strategy (golden oracle)

Upstream Fortran NJOY2016 at `../../../NJOY2016` is the reference. For each
ported module:

1. Build upstream once (`cmake -DCMAKE_BUILD_TYPE=Release ..; make`) to get the
   `njoy` executable.
2. Pick a small reference ENDF evaluation (e.g. a light nuclide) and run the
   module(s) under upstream NJOY to capture the golden output tape / ACE file.
3. Run the Rust port over the same input and assert equivalence:
   - ENDF tapes: structural equality (MAT/MF/MT sections, record values within
     a tight float tolerance — not byte equality, since formatting differs).
   - ACE files: parse both and compare arrays numerically; OpenMC must load the
     Rust-generated ACE and reproduce a reference k-eff / reaction rate.
4. Store reference inputs/outputs as test fixtures (mind size — ENDF files are
   large; keep fixtures minimal and consider `exclude` from the package).

Tests are `#[ignore]`-gated where they require the upstream `njoy` binary or
bulky ENDF data not checked in, with a comment explaining how to regenerate.

---

## 7. Open questions to resolve before Phase 2

- Which ENDF evaluation(s) become the canonical test fixtures? (smallest nuclide
  that exercises resonance reconstruction.)
- Does any target evaluation use the RML (`samm.f90`) formalism? If so, pull
  `samm` forward into Phase 2.
- Where do generated ACE files live, and does `openmc-libs` get a loader to
  consume them in an integration test?

---

## 8. Priority tracks — Keff + Doppler (opened 2026-07)

Two near-term OUTRAM PARK goals pull specific njoy modules forward. The full
cross-crate plan (njoy ↔ `openmc-libs`) lives in the workspace-level
**`../../../docs/keff-doppler-roadmap.md`**; njoy's slice of it:

- **Priority 2 — U-238 Doppler broadening of capture.** 🟡 The in-crate data is
  **WMP** with analytic broadening via the Faddeeva function — implemented **here
  in njoy** (`src/wmp.rs`), not `openmc-libs`. Real U-238 broadening of the
  6.67 eV capture resonance is confirmed (`tests/wmp_u238.rs`). njoy also holds
  the **independent oracle**: `RECONR` (✅) reconstructs the 0 K pointwise U-238
  σ(n,γ); `BROADR` (✅) SIGMA1-broadens it to T. The remaining quantitative gate
  compares WMP-analytic vs BROADR-kernel vs the OpenMC pregenerated `.h5`. Uses
  only already-ported modules — no new porting required.
- **Priority 1 — bare critical sphere Keff (U-235 Godiva, U-233 Jezebel-23).** ✅
  **done** — not via the ACER 4b/4d ACE-writer path, but directly:
  `nuclear_data::secondary` reads **ν̄(E)** (MF=1/452, `NuBar::from_endf`, LNU=1
  polynomial and LNU=2 tabulated) and the **fission spectrum χ(E→E')** (MF=5/MT=18,
  `FissionSpectrum::from_endf_mf5`) straight off the ENDF tape and hands them to
  `openmc-libs::Nuclide` — no ACE round-trip needed for this path. χ covers every
  MF=5 law with a real sampling algorithm: **LF=1** (arbitrary tabulated,
  `ContinuousTabular`), **LF=7** (Maxwellian, `Maxwell`), **LF=9** (evaporation,
  `Evaporation`), **LF=11** (energy-dependent Watt, `WattEnergyDependent`), and
  **NK>1** multi-partition mixtures (`Mixture`) recursively wrapping any of the
  above. **LF=5** (general evaporation) is not ported — it has no sampling
  algorithm even in canonical OpenMC (`GeneralEvaporation.to_hdf5` raises
  `NotImplementedError` in `openmc/data/energy_distribution.py`) — and **LF=12**
  (Madland-Nix) is left for later. Godiva V&V: replacing the fixed thermal-Watt
  χ with the real ENDF/B-VII.1 MF=5/LF=1 χ moved HIGH-tier k_eff by **+495 ± 251
  pcm** (0.99872 → 1.00367), see `docs/development-history.md` 2026-07. The
  ACER 4b/4d NU/DLW blocks (below) remain open for the *ACE-file* path, which a
  full transport library still needs for tools other than this workspace's own
  `openmc-libs`.
- **WMP import (`src/wmp.rs`, 4g).** 🟡 Done: `load_h5` reads the MIT
  `WMP_Library` HDF5 (behind the `wmp-hdf5` feature). This is now the data
  ingestion path for njoy's own WMP evaluator (all nuclear data lives in njoy;
  `openmc-libs` pulls via `XsProvider`). Remaining: `from_blob` so a curated set
  ships embedded with no HDF5 dependency in the built crate.
