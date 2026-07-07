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
| `moder` | `moder.f90` | 1714 | 🟡 | material selection (`select_materials`) + ASCII write (`endf::tape::Tape::write`, `endf::parse::format_endf_float`) ported; blocked-binary conversion not planned (see `src/moder/README.md`) |
| `driver` | `main.f90` | ~0.5k | ⬜ | card-input reader → thin Rust `match` over `NjoyModule` |
| — | `vers.f90` | trivial | ➖ | version string; not ported |

### ACE pipeline (Phases 2–4)

| Rust module | Fortran file | LOC | Phase | Status |
|---|---|---|---|---|
| `modules::reconr` | `reconr.f90` | 5.7k | 2 | ✅ resonance reconstruction |
| `modules::broadr` | `broadr.f90` | 2.0k | 2 | ✅ Doppler broadening (SIGMA1) |
| `heatr` | `heatr.f90` | 6.3k | 3 | 🟡 kinematic-limit KERMA (H1–H5, wired into ACE ESZ) + damage energy for the two-body recoil channels (H7: elastic + discrete levels) done, `src/heatr/mod.rs`; full photon energy-balance (H6) deferred, H7 anisotropy/continuum/capture channels remaining — see sub-phase table below |
| `gaspr` | `gaspr.f90` | 1.15k | 3 | ✅ gas production (MT=203–207), lumped-channel case only — see `src/gaspr/mod.rs` |
| `purr` | `purr.f90` | 2919 | 3 | ✅ fully ported — ENDF parsing (reuses `unresr::mf2`), `uw2`, `DopplerTable` (`uwtab2`), `Rng`, `generate_ladder`, `infinite_dilution_reference`, `read_heating_cross_sections`, and `probability_table`/`line_shape` (`unrest`, the Monte Carlo core) — see `src/purr/README.md`. Translation-only, **not run even once** — Opus verification pending; PENDF MT=152/153 tape writer not ported (pure plumbing, no physics) |
| `thermr` | `thermr.f90` | 3.4k | 3 | 🟡 MF=7 reader + coherent/incoherent elastic + inelastic physics; no module driver |
| `unresr` | `unresr.f90` | 1665 | 3 | 🟡 physics kernel ported (ENDF LRU=2 parser, Faddeeva/W-function library, `unresolved_cross_sections`) — see `src/unresr/README.md`; PENDF MT=152 output bookkeeping not ported |

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
| `modules::errorr` | `errorr.f90` | 11.2k | ⬜ multigroup covariance matrices; next up after `samm` finishes (2026-07-07) — it is `samm`'s `Want_Partial_Derivs`/`Want_Angular_Dist` caller |
| `modules::covr` | `covr.f90` | ~3k | ⬜ covariance output/plotting |
| `modules::leapr` | `leapr.f90` | 3.6k | ⬜ *generate* MF=7 S(α,β) (upstream of THERMR) |
| `modules::samm` | `samm.f90` | 7.2k | 🟨 Phase 1-4/6 done (ENDF reader, spin/parity/penetrability incl. betset core, Coulomb wave functions, R-matrix inversion); XS formula/derivatives/angular/top-level not started; shared by reconr/unresr |

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
  heating, gas, self-shielding, thermal scattering. **GASPR done** (2026-07-03,
  `src/gaspr.rs`): MT=203–207 gas production as a yield-weighted sum over the
  reconstructed MF=3 sections (MT=11/16/17/22–45/102–117 — the modern
  lumped-channel ENDF representation), using the crate's own `MtReaction`
  particle-content naming instead of NJOY's residual-mass bookkeeping. Not
  ported: the legacy MT=600–849 detailed-breakup fallback (pre-ENDF/B-VI style,
  rare in VII/VIII). 6 unit tests (additivity, multi-particle yields,
  two-species channels, non-gas-reaction exclusion).

  **HEATR (`heatr.f90`, ~6.3k lines) sub-phases.** The full module computes
  MT=301 heating (KERMA) by a **photon energy-balance** method (needs MF=12–15
  / MF=6 photon-production data, momentum conservation for capture recoil) plus
  a separate Lindhard-partition **damage-energy** calculation (MT=444) — too
  large for one pass, unlike GASPR. Broken into small, independently-testable
  phases, each its own commit:

  - **H1 — Elastic kinematic heating (MT=2).** ✅ `src/heatr.rs`. Closed-form
    average recoil energy for isotropic-CM elastic scattering off a mass-`A`
    target: `H(E) = σ_el(E)·E·2A/(A+1)²` (derived from 2-body kinematics
    averaged over isotropic `μ_cm`; independently reproduces the textbook
    result that hydrogen, A=1, loses on average half its energy per elastic
    collision — the test case). No photon data needed.
  - **H2 — Local-deposition reactions (MT=102, 103–117).** ✅ `src/heatr.rs`.
    Reactions with **no escaping neutron** (pure capture, or capture +
    charged-particle(s) that stay local in matter): `H(E) = σ(E)·(E+Q)`. This
    is NJOY's own documented behavior taken to its conclusion — `heatr.f90`'s
    module doc states it "deposits all photon energy locally when \[photon\]
    files are not available"; for a reaction with zero escaping neutrons that
    means *all* of `E+Q` is local.
  - **H3 — Single-escaping-neutron reactions (MT=4, 22, 23, 28, 29, 32–36, 44,
    45, and discrete levels 51–90).** ✅ `src/heatr.rs`. Generalizes H1 to a
    nonzero Q-value via the derived two-body-with-Q kinematics:
    `H(E) = σ(E)·[E·2A/(A+1)² + Q/(A+1)]` (reduces to H1's formula at `Q=0`).
    Reuses each reconstructed section's own `qi`.
  - **H4 — Fission heating (MT=18, 19–21, 38).** ✅ `src/heatr.rs`.
    `H(E) = σ_f(E)·[E + Q_fission − ν̄(E)·⟨E'⟩]`, reusing this session's
    `NuBar` (ν̄) and a new `FissionSpectrum::mean_energy` (⟨E'⟩, the first
    moment of χ — closed-form for the analytic laws, trapezoidal quadrature
    for the tabulated ones).
  - **H5 — Multi-neutron-exit + continuum inelastic (MT=11, 16, 17, 24, 25, 30,
    37, 41, 42, 91).** ✅ `src/heatr.rs`. Ports `nheat`'s neutron energy balance
    `H(E) = σ(E)·[E + Q − ȳ·⟨E'⟩]` (`heatr.f90:1441`, the `mtd<18 .or. mtd>21`
    branch): `ȳ` escaping neutrons each carry the mean energy `⟨E'⟩` of the
    reaction's emitted-neutron spectrum. The multiplicity `ȳ` is fixed by
    the MT ([`neutron_multiplicity`]: (n,2n)-type→2, (n,3n)-type→3, (n,4n)→4,
    continuum inelastic→1), and `⟨E'⟩` comes from the reaction's secondary
    spectrum via the [`EmissionSpectrum`] enum, which carries **either** an ENDF
    **MF=5** law ([`FissionSpectrum::mean_energy`] — LF=7 Maxwell / LF=9
    evaporation / LF=1 tabulated / LF=11 Watt, the same first-moment machinery H4
    uses for χ) **or** an ENDF **MF=6** LAW=1 tabulated emission
    ([`Mf6Neutron::mean_energy`] — the modern (n,2n)/(n,3n) representation; first
    moment `∫E'·f₀ dE'`, exact in the lab frame, a documented approximation in
    the CM frame with the `h6cm` angle transform deferred). Because the mean has
    no closed kinematic form, the spectrum must be *supplied*
    (`Kerma::from_reconr`'s `emission: &[(MtReaction, EmissionSpectrum)]`
    argument); a reaction whose spectrum is absent still contributes 0 (excluded,
    not guessed). Continuum inelastic MT=91 uses its section's own QI as the
    ground-state Q — a documented simplification of NJOY's QM/0 choice, immaterial
    away from the continuum threshold in the kinematic limit.

    **V&V (methodology + results).** Five unit tests in `src/heatr.rs`
    (`cargo test -p njoy-outram-park-fork --lib heatr`, 19/19 green 2026-07-04):
    with a fixed-parameter Watt emission spectrum (closed-form mean
    `⟨E'⟩ = 1.5a + ¼a²b`), (i) MT=16 (n,2n) reproduces `σ·(E+Q−2⟨E'⟩)` to a
    relative `<1e-9` at 10 and 14 MeV; (ii) MT=17/37 subtract 3×/4× the mean
    (multiplicity keyed off the MT); (iii) MT=91 is the yield-1 member,
    `σ·(E+Q−⟨E'⟩)`; (iv) a physical-sanity check on a 14-MeV (n,2n) on A=56
    (Q=−8 MeV, ⟨E'⟩≈1.5 MeV) gives heating `0 < H/σ < 5 MeV` and `< E+Q` — the
    few-MeV recoil left after ~8 MeV threshold + two escaping neutrons; (v) H5
    sums additively with H1–H4 on the shared union grid. A sixth test confirms an
    (n,2n) with **no** supplied spectrum yields an empty grid (contributes 0).
  - **H6 — Full photon energy-balance method.** ⬜ **deferred.** The actual
    `heatr.f90` algorithm proper: MF=12–15 / MF=6 photon-production data,
    momentum-conservation capture recoil, and using H1–H5 as the *kinematic
    check* NJOY itself runs the full method against (`kchk` branch).
  - **H7 — Damage energy (MT=444).** 🟡 two-body neutron-scattering channels
    ported (elastic + discrete inelastic levels), `src/heatr.rs`
    ([`DamageEnergy`]). Ports the Lindhard-Robinson partition `df` (`heatr.f90`)
    — the fraction of a recoil's kinetic energy that goes into atomic
    displacements rather than electronic excitation — plus NJOY's per-element
    default displacement-threshold table [`default_displacement_energy`] (C=31,
    Al=27, Fe/Ti–Cu/Zr/Nb=40, Mo/Ag=60, Ta/W=90, Pb=25, 25 eV fallback). For
    **elastic** (MT=2) and each **discrete inelastic level** (MT=51–90) the
    damage cross section is `σ_r(E)·⟨df⟩`; under **isotropic-CM** scattering the
    recoil energy is uniform on `[E_min, E_max]` (linear in the CM cosine —
    [`two_body_recoil_bounds`]: `C=A/(A+1)²·E`, `g=√(1−E_thr/E)`,
    `E_thr=(A+1)/A·|Q|`; elastic is `Q=0`, `[0, 4C]`), so `⟨df⟩` is the
    composite-Simpson mean of `df` over `[max(E_min,E_d), E_max]`. **Remaining
    H7**: MF=4 angular anisotropy reweighting the recoil (NJOY's 64-point
    Gauss-Legendre `disbar`), and the continuum/(n,xn)/capture-recoil (`capdam`)
    channels — all reuse the same `df`.

    **V&V (methodology + results).** Seven unit tests in `src/heatr.rs`
    (`--lib heatr`, 26/26 green 2026-07-04): the default-`E_d` table matches
    NJOY; `df=0` below `E_d`, `0<df≤E_R` above, and the damage fraction `df/E_R`
    is ≈1 near threshold, `<0.1` at 10 MeV, falling monotonically (the Lindhard
    signature); for Fe (A=56, E_d=40 eV) the elastic MT=444 is 0 at 100 eV
    (E_max≈6.9 eV<E_d), positive/rising above, and per-collision `< the H1
    heating` at every energy; a discrete level (`Q<0`) narrows the recoil window
    (`E_max < 4C`) and adds strictly-positive damage on top of elastic.

    **V&V (methodology + results).** Five unit tests in `src/heatr.rs`
    (`cargo test -p njoy-outram-park-fork --lib heatr`, 24/24 green 2026-07-04):
    (i) the default-`E_d` table reproduces NJOY's values; (ii) `df = 0` below
    `E_d` and `0 < df ≤ E_R` above (partition only *removes* energy); (iii) the
    damage fraction `df/E_R` is ≈1 just above threshold and `< 0.1` at 10 MeV
    (electronic stopping dominates) and falls monotonically — the Lindhard
    signature; (iv) for Fe (A=56, E_d=40 eV) the elastic MT=444 is 0 at 100 eV
    (E_max ≈ 6.9 eV < E_d), positive and monotically rising above, and per
    collision **< the H1 heating** `σ·E·2A/(A+1)²` at every energy (damage is a
    partition of the recoil energy heating counts in full); (v) empty table when
    no elastic section.

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
  - **4e — heating (ESZ column 5).** ✅ `src/ace/build.rs` +
    `src/interface.rs`. `from_reconr_full` takes an optional `&Kerma`; when
    supplied, the ESZ heating column is filled with the ACE heating number
    `H(E) = KERMA(E)/σ_total(E)` \[MeV\] (`acefc`'s `xss(ih+j) = s/emev/σ_t`).
    `write_ace` builds the HEATR H1–H5 KERMA (ν̄ from MF=1/452, χ from MF=5/18,
    and the H5 emission spectra via `heatr::build_emission_spectra`, which reads
    each (n,2n)/(n,3n)/continuum reaction's MF=6 LAW=1 or MF=5 law) and threads
    it in. V&V (`tests/acer.rs::esz_heating_column_is_physical`, 2026-07-04): the
    U-235 heating column is populated, everything in `[0, 200] MeV`, peak ≈ 160
    MeV/collision (thermal-fission dominated — ~185 MeV × fission/total). The
    `write_ace` example prints the same peak. Damage (MT=444) as a separate MTR
    reaction is not yet wired (needs H7 anisotropy + the MTR/SIG slot).
  - **4f — thermal S(α,β) ACE table.** ✅ **done** for the standard case
    (`src/ace/thermal.rs`, `AceTable::thermal_from_mf7`) — writes the `…t`
    thermal-scattering tables (graphite, H₂O, D₂O, ZrH, Al, …) with the full
    thermal NXS/JXS layout: ITIE/ITIX/ITXE inelastic (IFENG=0 equiprobable
    form), ITCE/ITCX coherent-elastic Bragg, and ITCE/ITCX/ITCA **or**
    ITCEI/ITCXI/ITCAI incoherent-elastic (primary-slot vs secondary-slot
    depending on whether coherent-elastic is also present — mirrors
    `aceth.f90::thrlod`'s IDPNC=3/4/5 branching). Both `tests/thermal_ace.rs`
    (Al-27, coherent+inelastic) and `tests/thermal_ace_zrh.rs`
    (H-in-ZrH, incoherent-elastic+inelastic, no coherent) pass, including a
    full write→read round-trip.
    - **THERMR physics** (`src/thermal/`) — done: [`mf7`] parses MT=2
      coherent/incoherent elastic and MT=4 incoherent-inelastic S(α,β);
      [`coherent`] gives σ_coh(E)=S(E)/E + discrete Bragg reflection
      cosines/weights; [`incoherent_elastic`] gives the closed-form
      σ(E,T)=(σ_b/2N)(1−e^{−4EW'})/(2EW') and its equiprobable cosines
      (analytic CDF inversion of the exponential angular law); [`inelastic`]
      gives the double-differential `d²σ/dE'dμ` from S(α,β), the integrated
      `σ(E→E')`/`σ_inel(E)`, and — the piece that closed this out —
      `equiprobable_emission`/`equiprobable_cosines`: numerically inverts the
      (E'-profile, then per-E' angular) CDFs to build the `nieb`×`nang`
      equiprobable emission table the ACE ITXE block needs. Validated on
      Al-27: σ_b≈1.45 b, σ_inel rises from the cold-crystal thermal value
      toward the free-atom limit σ_free≈1.35 b near 1–2 eV.
    **Not ported** (documented in `src/ace/thermal.rs`'s own module doc, not a
    silent gap): the skewed/continuous **IFENG=1/2** inelastic forms (IFENG=0
    equiprobable is what production ACE thermal libraries typically ship), and
    multi-scatterer mixing (`nmix` taken as 1). **LEAPR** (`leapr.f90`, Phase
    5, *generates* MF=7 when an evaluation lacks it) remains unported — not
    needed since the ENDF/B thermal sublibrary ships MF=7 directly.
  The written CE file now carries cross sections + the elastic angular
  distribution. Until 4b/4d finish it is still not a complete CE transport
  library (fission has no NU block; continuum producers are isotropic); 4f
  (thermal S(α,β)) is a separate, now-complete table type layered on top.
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

> ### ⏭️ NEXT TO PORT — Unresolved Resonance Region (URR, LRU=2): `UNRESR` then `PURR`
>
> **Why now (2026-07-06):** RECONR reconstructs only the **resolved** resonance
> region (LRU=1: SLBW/MLBW/Reich-Moore). For U-238 the resolved region ends at
> **20 keV**; from 20 keV to **149 keV** the evaluation is an **unresolved**
> region (MF=2 LRU=2, which the port currently parses *header-only* — see
> `reconr/mf2.rs::parse_lru2_header`). Consequences:
> - Pointwise σ(n,γ) / σ_el can be reconstructed **only up to 20 keV**. The
>   U-238 capture Doppler code-to-code verification vs the OpenMC `.h5`
>   (`tests/u238_doppler_verification/`) is therefore run in the RRR window
>   (≤ 20 keV), which is where all the strong, Doppler-sensitive resonances sit.
> - The 20–149 keV band and self-shielding (probability tables) need the
>   unresolved modules, in this order:
>   1. **`UNRESR`** (`unresr.f90`, ~1.8k LOC) — infinite-dilution average σ in
>      the URR from the LRU=2 parameters. Unblocks the 20–149 keV pointwise band.
>   2. **`PURR`** (`purr.f90`, ~2.9k LOC) — URR probability tables for
>      self-shielding (feeds the ACE `urr` block / OpenMC's `urr` group).
> - Above 149 keV the fast region is pure tabulated MF=3 and needs no resonance
>   reconstruction.
>
> This is the first gap a caller hits going past the resolved region, so it is
> the next porting target after the current Doppler verification lands.
>
> **Update (2026-07-06):** `UNRESR`'s physics kernel is ported — ENDF LRU=2
> parameter reader (`unresr::mf2`, Case A/B/C), the Faddeeva/`w(z)`
> width-fluctuation library (`unresr::wfun`), and the per-energy self-shielded
> cross-section calculator (`unresr::unresolved_cross_sections`, ported from
> `unresl`). Translation only — no tests written yet (Opus verification
> pending; see `src/unresr/README.md`).
>
> **Update (2026-07-06, cont'd):** `PURR`'s scaffolding is ported —
> ENDF parsing (reuses `unresr::mf2`), `purr::wfun::uw2`, `purr::Rng` (`rann`),
> `purr::generate_ladder` (`ladr2`), `purr::infinite_dilution_reference`
> (`unresx`+`gnrx`), `purr::read_heating_cross_sections` (`rdheat`). After
> reading the full `unrest` routine (the Monte Carlo probability-table core,
> ~750 lines) it was clear this is the single most numerically delicate piece
> in this crate's NJOY work — six-tier Doppler line-shape regime branching,
> its own two-table `w(z)` lookup scheme, dynamic histogram binning —
> comparable in *kind* to BROADR's still-open wing bug but larger in scope.
> Checked in with the user before proceeding rather than rushing it.
>
> **Update (2026-07-06, cont'd again):** at the user's direction, `unrest` is
> now ported too — `purr::wfun::DopplerTable` (`uwtab2`, the two-table `w(z)`
> lookup), `purr::line_shape` (the four-tier Doppler line-shape evaluator —
> ported as a **direct per-point classification**, verified equivalent to
> upstream's binary-search index-range chains rather than replicating that
> bookkeeping literally; see `src/purr/README.md` for the equivalence
> argument), and `purr::probability_table` (the full Monte Carlo binning +
> Bondarenko-moment pipeline). `PURR` is now **fully ported** module-for-module
> (only the PENDF MT=152/153 tape writer remains unported, matching every
> other module's driver-vs-typed-API split). Translation only — **the code
> has not been run even once**, only reviewed line-by-line against the Fortran
> source and cross-checked arithmetic (e.g. the dynamic bin-edge index
> mapping, Fortran 1-indexed → Rust 0-indexed, re-verified branch-by-branch by
> hand). Opus verification is the necessary next step before this can be
> trusted; full workspace build + existing test suite pass as a regression
> check (compiles clean, zero warnings, but that only proves the *types*
> check out, not the physics).
>
> **Update (2026-07-07):** at the user's explicit direction ("full port
> including Coulomb + derivatives + angular"), started `samm.f90`
> (R-matrix-limited, LRF=7 — see the note above §5 and `src/samm/README.md`
> for the 6-phase plan). **Phase 1 done**: the ENDF LRF=7/KRM=3 parameter
> reader (`samm::mf2` — `ParticlePair`, `RmlChannel`, `RmlResonance`,
> `SpinGroup`, `RmlSection`, `parse_rml_section`, ported from `rdsammy`'s
> `mode==7` branch). Found and fixed one real bug (an inverted
> `lbk`/`lch` error-guard condition). **One discrepancy is flagged, not
> resolved**: the eliminated-capture-channel reorder step reads
> `gamma(igamma+1)` in the Fortran (`samm.f90:1186`); two independent
> hand-derivations both predicted `igamma-1` instead. Ported literally
> (bounds-checked, returns `NjoyError::EndfParse` rather than panicking)
> with a prominent doc-comment flag — this is the top verification priority
> for Opus once a real LRF=7 evaluation (¹⁶O or ¹⁹F) is available to check
> against. Phases 2–6 (spin/parity/penetrability setup, Coulomb wave
> functions, R-matrix inversion, cross-section + derivatives + angular
> distributions, top-level orchestration) are not started. Full workspace
> build + test suite pass as a regression check only (139/12/7/12/1/20/11/
> 11/4/5/2/3 test groups green, zero regressions from adding `samm::mf2`).
>
> **Update (2026-07-07, cont'd):** found and surfaced a concrete scope
> reduction — RECONR (the only current caller) hardcodes
> `Want_Partial_Derivs=.false.`/`Want_Angular_Dist=.false.` (`reconr.f90:
> 149-150`), so `betset`'s derivative branches and all of `angle`/`lmaxxx`/
> `kclbsch`/`clbsch`/`setleg` are unreachable dead code for this crate's
> actual call graph — only `ERRORR` (not yet in this workspace) enables
> them. User chose to defer those (**not** drop them) until `ERRORR` is
> actually being built. **Phase 2 (spin/parity/penetrability, non-Coulomb
> core) is now done**: `samm::penetrability` (`pf`/`genpsf`/`pgh` — hard-
> sphere penetrability/shift/phase-shift, `l=0..4` closed form + recursion
> for `l>4`) and `samm::context` (`ppdefs`→`apply_particle_pair_defaults`,
> `checkqn`→`check_quantum_numbers`, `fxradi`→`compute_channel_kinematics`).
> Confirmed `findsp`/`rearrange` are dead code for `mode==7` too (resonances
> are already read per spin group, matching Phase 1's data model) — not
> ported. Two more items flagged for Opus: (1) strengthened the eliminated-
> channel-reorder flag in `mf2.rs` with a concrete worked numerical example
> showing `igamma+1` reads past this group's valid channel range — stronger
> circumstantial evidence of a latent upstream bug, but still not
> independently confirmed, so still ported literally; (2) `genpsf`'s `l=4`
> recursion seed reads a local (`dss`) before it's set in the Fortran itself
> — ported with it seeded to `0.0`, flagged, affects only `l>4` (rare in
> practice). `betset` itself is not yet ported — its non-derivative core
> needs `pghcou` (Phase 3, Coulomb) for charged-particle channels before it
> can be done in full; `pgh` alone (done) covers neutral/neutron channels.
> Full workspace build + test suite green (same 139/12/7/12/1/20/11/11/4/5/
> 2/3 counts), zero regressions.
>
> **Update (2026-07-07, cont'd again — scope reversal):** user said "finish
> all phases of samm" — the derivatives/angular deferral above is
> **superseded**: `babb`/`abpart`/`derres`/`derext`/`angle`/`lmaxxx`/
> `kclbsch`/`clbsch`/`setleg` are back in scope, to be ported alongside
> `errorr.f90` (their real caller) right after samm finishes, per further
> instruction to port `errorr` next. Standing rule going forward: when
> porting code with no reachable caller yet, also build (or schedule) the
> consumer, rather than leaving it orphaned.
>
> **Phase 3 (Coulomb wave-function library) is now done**: `samm::coulomb`
> — `jwkb`, `coulfg` (Steed's method / Barnett's CPC "COULFG"), `xsigll`,
> `asymp1`/`asymp2`, `taylor`, `end1`, `getfg`, `bigeta`, `getps`, `coulx`,
> `pspcou`, `pghcou`. Uses a direct 0-indexed-by-`L` `Vec<f64>` convention
> throughout instead of Fortran's 1-indexed `array(L+1)`, checked position-
> by-position against the source rather than re-derived (an indexing slip
> here would be easy to make and hard to notice). One dead local (`paccq`
> in `coulfg`'s CF2 section, write-only in the Fortran) intentionally not
> ported.
>
> **`betset`'s non-derivative core is also done** (`samm::betset::
> compute_resonance_amplitudes`) — the first real consumer of both
> `penetrability::pgh` and `coulomb::pghcou`, converting ENDF reduced
> widths into R-matrix reduced-width amplitudes, their triangular products,
> and the eliminated channel's own amplitude. This closes out Phase 2
> completely (previously blocked on Phase 3 for charged channels). Flagged:
> a stale-`drho` term inherited from upstream for on-threshold resonances,
> only relevant to the not-yet-ported derivative term.
>
> Cleanup during this pass: an initial draft of `pspcou`/`pghcou` used a
> `thread_local!` scratch cell to smuggle an extra derivative value between
> functions — caught on review as needless hidden state and replaced with a
> plain `dshift` field on the shared result struct.
>
> Full workspace build + test suite green (same 139/12/7/12/1/20/11/11/4/5/
> 2/3 counts), zero regressions, zero warnings in this crate.
>
> **Update (2026-07-07, cont'd yet again):** **Phase 4 (R-matrix inversion)
> is done** — `samm::linpack` (the general complex-symmetric packed solver:
> `xspfa` Bunch-Kaufman factorization, `xspsl` solve, `xaxpy`/`xdot`/
> `xswap`/`ixamax` BLAS-1 helpers; only the stride-1 path is ported since
> every `samm.f90` call site passes `incx=incy=1`, and the 1970s manual
> loop-unrolling is dropped as meaningless under a modern optimizer) and
> `samm::rmatrix_invert` (`yinvrs`'s dispatcher, closed-form `onech`/
> `twoch`/`threech` for 1/2/3 channels, `yfour` for 4+ via `linpack`,
> `zeror`). Kept indices numerically identical to the Fortran's 1-indexed
> flat packed offsets rather than translating to 0-indexed, for the same
> line-by-line-checkability reason as `coulomb.rs` — this is the densest,
> most bug-prone numerical code in the port so far (a pivoting-logic
> off-by-one corrupts silently, doesn't panic). `setxqx`/`sectio`/
> `gcphase`/`setqri`/`settri` (the R-matrix *assembly* step, as opposed to
> its inversion) are deferred to Phase 5, since they're `crosss`'s
> responsibility, not `yinvrs`'s.
>
> A few borrow-checker-driven refactors during translation (not physics
> changes): several `s(v, k, g(v, k) * factor)`-style in-place scale/
> unscale calls needed splitting into read-then-write across two
> statements (Rust won't allow a mutable and immutable borrow of the same
> buffer within one function call's argument list) — factored into small
> `scale_at`/`unscale_at`/`swap1` helpers rather than repeating the
> workaround inline.
>
> Full workspace build + test suite green (same 139/12/7/12/1/20/11/11/4/5/
> 2/3 counts), zero regressions, zero warnings in this crate. This is
> genuinely not run against any real R-matrix problem yet — Phase 5 needs
> to wire `crosss`/`setr` in before there's an end-to-end path to test
> against.

- **Priority 2 — U-238 Doppler broadening of capture.** 🟡 The in-crate data is
  **WMP** with analytic broadening via the Faddeeva function — implemented **here
  in njoy** (`src/wmp.rs`), not `openmc-libs`. Real U-238 broadening of the
  6.67 eV capture resonance is confirmed (`tests/wmp_u238.rs`). njoy also holds
  the **independent oracle**: `RECONR` reconstructs the 0 K pointwise U-238
  σ(n,γ); `BROADR` SIGMA1-broadens it to T. The code-to-code gate vs the OpenMC
  ENDF/B-VIII.0 `.h5` is wired up in `tests/u238_doppler_verification/`
  (RECONR+BROADR from the ENDF tape vs OpenMC pointwise, extracted to committed
  reference CSVs so the 115 MB `.h5` need not be tracked).
  - ✅ 0 K reconstruction (thermal 2.68 b, 6.67 eV peak 22.2 kb), peak Doppler
    broadening (~10%), and the smooth above-RRR / MF=3 band (L1 ≈ 1.5%) all agree.
  - 🐛 **OPEN BUG (BROADR/SIGMA1 resonance wings).** The Rust broadened capture
    leaves a spurious ~200 b pedestal several eV past each resolved resonance
    where OpenMC decays to ~1 b (e.g. 103.5 eV @ 900 K: 105 eV → OpenMC 1.8 b vs
    Rust 211 b). RRR L1 ≈ 0.23–0.25. Off-resonance baseline agrees, so it is a
    wing/pedestal fidelity bug in the SIGMA1 kernel (or its interaction with the
    RECONR wing grid), not a global offset. This is the concrete BROADR debugging
    task; the verification test reports it (gate not loosened). See
    `tests/u238_doppler_verification/README.md`.
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
