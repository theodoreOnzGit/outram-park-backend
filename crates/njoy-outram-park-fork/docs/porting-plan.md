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

| Rust module | Fortran file | LOC | Notes |
|---|---|---|---|
| `common::phys` | `phys.f90` | ~150 | physical constants → `uom` |
| `common::mathm` | `mathm.f90` | ~1.4k | special functions, small linear algebra |
| `common` (util) | `util.f90` | ~2k | record I/O helpers, interpolation, `error`/`mess` → `Result` |
| `common` (io) | `mainio.f90`, `locale.f90` | ~0.3k | unit numbers / formatting → owned config |
| `endf` | `endf.f90` | ~1k | in-memory ENDF tape model + record parsing |
| `modules::moder` | `moder.f90` | ~0.6k | ASCII ⇄ binary tape conversion |

### ACE pipeline (Phases 2–4)

| Rust module | Fortran file | LOC | Phase |
|---|---|---|---|
| `modules::reconr` | `reconr.f90` | 5.7k | 2 |
| `modules::broadr` | `broadr.f90` | 2.0k | 2 |
| `modules::heatr` | `heatr.f90` | 6.3k | 3 |
| `modules::gaspr` | `gaspr.f90` | ~0.6k | 3 |
| `modules::purr` | `purr.f90` | 2.9k | 3 |
| `modules::thermr` | `thermr.f90` | 3.4k | 3 |
| `modules::unresr` | `unresr.f90` | ~1.8k | 3 (PURR precursor) |
| `modules::acer` | `acer.f90` + `acefc.f90` (19.7k!), `acepn.f90` (3.8k), `acepa.f90`, `aceth.f90`, `acedo.f90`, `acecm.f90` | ~30k total | 4 |

### Multigroup & covariance (Phase 5 — not needed by OpenMC CE)

| Rust module | Fortran file | LOC |
|---|---|---|
| `modules::groupr` | `groupr.f90` | 12.7k |
| `modules::gaminr` | `gaminr.f90` | ~2k |
| `modules::errorr` | `errorr.f90` | 11.2k |
| `modules::covr` | `covr.f90` | ~3k |
| `modules::leapr` | `leapr.f90` | 3.6k |
| `modules::samm` | `samm.f90` | 7.2k (R-matrix; shared by reconr/unresr) |

### Formatters, plotting, misc (Phase 6 — lowest priority)

`dtfr.f90`, `ccccr.f90`, `matxsr.f90`, `resxsr.f90`, `powr.f90`, `wimsr.f90`,
`mixr.f90`, `plotr.f90`, `viewr.f90`, `graph.f90`. Output formats for codes
OUTRAM PARK does not target. Port only on demand.

The driver `main.f90` (NJOY card-input reader sequencing the modules) becomes a
thin Rust `driver` that parses the input deck and `match`es over `NjoyModule`.

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
    `parse_mf5_law4` → per-incident-energy (E_out, pdf, cdf), validated on U-235
    χ (pdf≥0, cdf 0→1 monotone, peaks ~1 MeV). Plus **MF=6 LAW=1**
    (`parse_mf6_law1_neutron`): extracts the neutron (ZAP=1) energy pdf `f₀` +
    yield/frame for TYR, validated on U-235 MT16 (n,2n yield 2, CM) and MT91.
    **Not yet wired into the DLW block**: NXS(5)=NR counts *neutron-producing*
    reactions and every producer needs a valid law — **next**: the angular upgrade
    (MF=6 LANG=1 Legendre → Law 61, LANG=2 Kalbach → Law 44), two-body discrete
    levels → Law 3 (from Q+AWR), then wire LDLW/DLW/TYR. The `energy.rs` module
    doc records the exact DLW/LDLW/TYR/NR layout for wiring.
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
    **Progress:** the **MF=7 reader** is done — `src/thermal/mf7.rs` parses MT=2
    coherent-elastic Bragg `S(E)` and MT=4 incoherent-inelastic `S(α,β)` (B-const
    + β/α grids) into typed data, tested against the Al-27 ENDF/B-VIII `tsl`
    fixture. Still needed: the THERMR computation (S(α,β) → σ + dists) and the
    `aceth.f90` writer.
  The written file now carries cross sections + the elastic angular distribution.
  Until 4b/4d/4e exist it is still not a complete transport library (no secondary
  energy distributions, so inelastic/fission collisions can't be followed); 4f
  (thermal S(α,β)) is a separate table type layered on top once 4a–4e are done.
  - **4g — Windowed Multipole (WMP) import.** ⏳ **scaffolded** (`src/wmp.rs`,
    stub returning `NotPorted`). **Independent MIT CRPG work — NOT NJOY/LANL.**
    Imports the **MIT** `WMP_Library` (<https://github.com/mit-crpg/WMP_Library>,
    MIT-licensed) HDF5 multipole data: complex poles/residues + windows enabling
    *analytic* on-the-fly Doppler broadening (Faddeeva `w(z)`), a parallel
    alternative to the pointwise ACE/PENDF representation. **Scheduled after 4f**
    (thermal S(α,β)). Credit MIT CRPG and Josey/Romano/Forget/Smith; add a
    separate `LICENSE-WMP` (MIT) + NOTICE entry before importing any code/data —
    keep it cleanly separable from the NJOY BSD/LANL provenance. See the
    `src/wmp.rs` module docs.
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
