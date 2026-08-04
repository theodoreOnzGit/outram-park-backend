# DWSIM chemistry-model survey — what to port upstream

A complete survey of the **chemistry / thermodynamics** models in the DWSIM
upstream source, cross-referenced against what `outram-park-fork-dwsim-libs`
has already ported, with an OUTRAM PARK relevance ranking to prioritise the
port backlog. This complements [`port-scope.md`](./port-scope.md) (which also
covers the equipment/unit-operation tiers) by enumerating the **full** chemistry
surface — every property package, flash algorithm, and reaction/reactor model —
not just the Tier-1 subset.

> ⚠️ **Unverified until validated.** Survey of upstream scope only; nothing here
> is a validated model. Independent OUTRAM PARK fork, not official DWSIM. Not for
> nuclear facility operation, reactor control, safety-critical, or licensing
> decisions. See the workspace `RESPONSIBLE_USE.md` / `VERIFICATION_AND_VALIDATION.md`.

## Provenance

- **Upstream:** DWSIM — <https://github.com/DanWBR/dwsim>
- **License:** **GPL-3.0** (confirmed against the clone's `COPYING` = GNU GPL v3;
  matches this crate's GPL-3.0).
- **Surveyed commit:** `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766` (2026-07-17),
  shallow clone under `upstream_source/DWSIM/` (gitignored).
- **Language:** VB.NET (thermodynamics + reactors are `.vb`).

## Legend

**Port status** (against `src/thermo/`): ✅ ported · ◐ partial · ⬜ not ported ·
✗ out of scope (external interop / superseded by another OUTRAM crate).

**OUTRAM PARK relevance** (nuclear thermal-hydraulics, molten-salt & coolant
chemistry lens): ★★★ high · ★★ medium · ★ low (petroleum-specific) · — n/a.

---

## 1. Property packages — `DWSIM.Thermodynamics/PropertyPackages/` (30 files)

### 1a. Cubic equations of state

| Model | Source `.vb` | LOC | Status | Rel. | Notes |
|---|---|--:|:--:|:--:|---|
| Peng-Robinson (PR) | `PengRobinson` | 1153 | ✅ | ★★ | Cover-gas / supercritical-CO₂ / hydrocarbon systems. `thermo::cubic_eos`. |
| PR-1978 (corrected ω) | `PengRobinson78` | 1178 | ✅ | ★★ | 1978 α-refit; better for heavy/high-ω species. `thermo::pr1978`. |
| Soave-Redlich-Kwong (SRK) | `SoaveRedlichKwong` | 1199 | ✅ | ★★ | `thermo::cubic_eos`. |
| PRSV2 (Stryjek-Vera) | `PengRobinsonStryjekVera2` | 942 | ✅ | ★★ | Full κ₁/κ₂/κ₃ α-function with Z / fugacity / departure / vapour-pressure surface. `thermo::prsv2_full` (κ₁-only free fn also in `eos_variants`). |
| PRSV2-VL (volume-translated) | `PengRobinsonStryjekVera2VL` | 913 | ◐ | ★★ | Adds Peneloux-style volume translation (partly in `eos_variants`). |
| Lee-Kesler-Plöcker (LKP) | `LeeKeslerPlocker` | 755 | ✅ | ★★ | 3-parameter corresponding-states; accurate densities/enthalpies for light gases. `thermo::lkp`. |
| PR + Lee-Kesler enthalpy | `PengRobinsonLeeKesler` | 409 | ✅ | ★★ | PR K-values with LK caloric departures. `thermo::pr_lee_kesler`. |

### 1b. Activity-coefficient (liquid-phase) models

| Model | Source `.vb` | LOC | Status | Rel. | Notes |
|---|---|--:|:--:|:--:|---|
| NRTL | `NRTL` | 374 | ✅ | ★★ | `thermo::activity`. Non-ideal liquids, aqueous. |
| UNIQUAC | `UNIQUAC` | 396 | ✅ | ★★ | `thermo::activity`. |
| UNIFAC | `UNIFAC` | 143 | ✅ | ★★ | Group contribution. `thermo::unifac`. |
| UNIFAC-LLE | `UNIFACLL` | 142 | ✅ | ★★ | LLE-parameterised UNIFAC. `thermo::unifac_lle`. |
| Modified UNIFAC (Dortmund) | `MODFAC` | 138 | ✅ | ★★ | Temperature-dependent groups; better than base UNIFAC. `thermo::unifac_dortmund`. |
| NIST-Modified UNIFAC | `NISTMFAC` | 151 | ⬜ | ★ | NIST parameter set variant of MODFAC. |
| Wilson | `WilsonPropertyPackage` | 369 | ⬜ | ★ | Cannot do LLE; superseded by NRTL/UNIQUAC for our uses. |
| Ideal / Raoult | `Ideal` | 871 | ✅ | ★★ | `thermo::property_package::Ideal` (Wilson-K estimate). |
| Activity-coefficient base | `ActivityCoefficientBase` | 1063 | ◐ | — | Base machinery (γ→K, γ→H^E); partially mirrored in `thermo::activity`. |

### 1c. Electrolyte / aqueous-ionic models — **coolant & molten-salt chemistry**

| Model | Source `.vb` | LOC | Status | Rel. | Notes |
|---|---|--:|:--:|:--:|---|
| Electrolyte base | `ElectrolyteBase` | 841 | ✅ | ★★★ | Ion speciation substrate. `thermo::electrolyte`. |
| Ideal electrolyte | `ElectrolyteIdeal` | 643 | ✅ | ★★★ | Molality-scale ideal + Debye-Hückel mean-ionic term. `thermo::electrolyte`. |
| LIQUAC | `LIQUAC2PropertyPackage` | 638 | ◐ | ★★★ | Debye-Hückel long-range + middle-range + UNIQUAC short-range activity kernel ported (`thermo::electrolyte`); full package glue not. |
| Sour water | `SourWater` | 298 | ✅ | ★★ | H₂S/NH₃/CO₂ aqueous ionic equilibria — off-gas / coolant degassing. `thermo::sour_water`. |

> **Extended UNIQUAC** (a common electrolyte model) is not a standalone file
> here — DWSIM's electrolyte activity lives in `ElectrolyteBase` +
> `LIQUAC2`. Aqueous-ionic chemistry is the **highest-value gap** for reactor
> coolant modelling (water chemistry, boron/pH control, fission-product
> solubility) and, by analogy, molten-salt ionic melts.

### 1d. Empirical / specialised property packages

| Model | Source `.vb` | LOC | Status | Rel. | Notes |
|---|---|--:|:--:|:--:|---|
| Steam tables (IAPWS-IF97) | `SteamTables` | 1134 | ✗→✓ | ★★★ | **Superseded** by the workspace `tampines-steam-tables` crate — use that. |
| Seawater | `SeaWater` | 857 | ⬜ | ★ | Seawater thermophysics; niche for us. |
| Black oil | `BlackOil` | 815 | ⬜ | ★ | Petroleum reservoir fluids — out of our domain. |
| Chao-Seader | `ChaoSeader` | 851 | ⬜ | ★ | Semi-empirical hydrocarbon K-values (legacy petroleum). |
| Grayson-Streed | `GraysonStreed` | 866 | ⬜ | ★ | Chao-Seader variant for H₂-rich hydrocarbon systems. |

### 1e. External bridges — **out of scope**

| Model | Source `.vb` | LOC | Status | Notes |
|---|---|--:|:--:|---|
| CoolProp | `CoolProp` | 2130 | ✗ | Use the workspace `outram-park-fork-coolprop` crate instead. |
| CoolProp incompressible (mix/pure) | `CoolPropIncompressible*` | 882/756 | ✗ | Same — our coolprop fork. |
| CAPE-OPEN socket | `CAPEOPENSocket` | 1772 | ✗ | Windows COM interop; not portable / not needed. |
| Base class | `PropertyPackage` | 13940 | ◐ | The 14 kLOC base (K-values, phase props, calc orchestration). Its *methods* are the port substrate — mirrored piecewise in `thermo::property_package` + `thermo::property_methods`. |

---

## 2. Flash algorithms — `DWSIM.Thermodynamics/FlashAlgorithms/` (23 files)

The flash is the innermost equilibrium solve. The fork now covers the **2-phase
VLE** core plus the multi-phase, solid, and electrolyte flashes.

| Algorithm | Source `.vb` | LOC | Phases | Status | Rel. | Notes |
|---|---|--:|:--:|:--:|:--:|---|
| Nested Loops (VLE) | `NestedLoops` | 4185 | VL | ✅ | ★★ | Rachford-Rice / nested loops. `thermo::flash`. |
| Boston-Britt Inside-Out | `BostonBrittInsideOut` | 2302 | VL | ✅ | ★★ | Faster inner/outer loop; robust for wide-boiling mixtures. `thermo::flash_insideout`. |
| Nested Loops 3P (VLLE) | `NestedLoops3PV3` | 1853 | VLL | ✅ | ★★ | Three-phase vapour-liquid-liquid. `thermo::flash_vlle`. |
| Inside-Out 3P | `BostonFournierInsideOut3P` | 2144 | VLL | ✅ | ★★ | 3-phase Inside-Out. `thermo::flash_insideout_3p`. |
| Gibbs minimisation 3P | `GibbsMinimization3P` | 1414 | VLL | ✅ | ★★★ | Direct Gibbs-energy minimisation for **speciation**. `thermo::gibbs` / `gibbs_multiphase`. |
| Gibbs minimisation (multi) | `GibbsMinimizationMulti` | 1145 | N-phase | ✅ | ★★★ | Multi-phase Gibbs — molten-salt / fission-product speciation. `thermo::gibbs_multiphase`. |
| **Nested Loops SLE** | `NestedLoopsSLE` | 1830 | SL | ✅ | ★★★ | **Solid-liquid equilibrium** — salt freezing / precipitation (MSR!). `thermo::flash_sle`. |
| Nested Loops SVLLE | `NestedLoopsSVLLE` | 315 | SVLL | ✅ | ★★ | Solid + 3-phase fluid. `thermo::flash_svlle`. |
| Simple LLE | `SimpleLLE` | 1252 | LL | ✅ | ★★ | Liquid-liquid split. `thermo::flash_lle`. |
| Nested Loops immiscible | `NestedLoopsImmiscible` | 293 | VL(+immisc.) | ⬜ | ★ | Immiscible water/hydrocarbon. |
| **Electrolyte SVLE** | `ElectrolyteSVLE` | 1191 | S-V-L ionic | ✅ | ★★★ | Aqueous-ionic solid-vapour-liquid — coolant precipitation chemistry. `thermo::electrolyte_svle`. |
| Single-component flash | `SingleCompFlash` | 454 | any | ✅ | ★★ | Pure-fluid saturation shortcut. `thermo::flash_single_comp`. |
| Forced-phase flash | `ForcedPhaseFlash` | 539 | forced | ⬜ | ★ | Skip equilibrium, force a phase. |
| Universal flash | `UniversalFlash` | 864 | dispatcher | ◐ | ★★ | Picks the right sub-flash by phase count; our `property_package` does a narrower version. |
| Base flash | `BaseFlashAlgorithm` | 1949 | — | ◐ | — | Shared init (Wilson K, stability). Partly in `thermo::stability`. |
| Sour water / Seawater / Black oil / Steam / CoolProp / CAPE-OPEN / UserDefined | — | — | — | ✗/⬜ | — | Package-specific or external flashes; port only alongside their package. |

---

## 3. Reactions & reactors — **now ported (★★★ for reactor chemistry)**

Reaction handling has two layers: the **reaction model** (kinetics/equilibrium
definitions, `crate::reactions`) and the **reactor unit operations**
(`crate::reactors`) that integrate them.

### 3a. Reaction model types — `ReactionType` (`DWSIM.Interfaces/Enums.vb`; definitions in `ThermodynamicsBase.vb`)

| Reaction type | Status | Rel. | Notes |
|---|:--:|:--:|---|
| **Conversion** | ✅ | ★★ | Fixed fractional conversion of a key reactant. `reactions::ReactionKind`. |
| **Equilibrium** | ✅ | ★★★ | K_eq(T) from ΔG — fission-product / corrosion / salt-redox speciation. `reactions`. |
| **Kinetic** | ✅ | ★★★ | Arrhenius power-law forward/reverse rate. `reactions::Reaction`. |
| **Heterogeneous catalytic** | ✅ | ★★ | Langmuir-Hinshelwood surface kinetics. `reactions::LangmuirHinshelwood`. |
| Reaction basis | ✅ | — | Activity / fugacity / molar-conc / mass-conc / molar-frac (`reactions::ReactionBasis`). |

### 3b. Reactor unit operations — `DWSIM.UnitOperations/Reactors/`

| Reactor | Source `.vb` | LOC | Status | Rel. | Notes |
|---|---|--:|:--:|:--:|---|
| Gibbs | `Gibbs` | 3028 | ✅ | ★★★ | Min-Gibbs equilibrium reactor — **speciation without reaction list**. `reactors::GibbsReactor`. |
| Equilibrium | `Equilibrium` | 3798 | ✅ | ★★★ | Simultaneous K_eq reactions. `reactors::EquilibriumReactor`. |
| CSTR | `CSTR` | 1611 | ✅ | ★★ | Continuous stirred tank + kinetics. `reactors::Cstr`. |
| PFR | `PFR` | 2274 | ✅ | ★★ | Plug-flow + kinetics (ODE along length). `reactors::Pfr`. |
| Conversion | `Conversion` | 1374 | ✅ | ★ | Fixed-conversion reactor. `reactors::ConversionReactor`. |
| Base reactor | `BaseReactor` | 335 | ◐ | — | Shared substrate (`reactors::ReactorFeed` / `ReactorOutcome` / `ReactorModel`). |
| Reaktoro-Gibbs | `ReaktoroGibbs` | 693 | ✗ | — | Bridge to external Reaktoro lib — out of scope (built our own Gibbs solver). |

> **Simplifications** versus upstream (honest limitations): reactors hold
> volumetric flow fixed at the feed value and solve isothermally at the feed
> temperature (the heat of reaction is reported but not fed back into an energy
> balance); equilibrium uses ideal activity/fugacity. See `reactors` module docs.

---

## 4. Supporting property-method base classes — `DWSIM.Thermodynamics/BaseClasses/`

Not "models" per se, but the shared machinery every model calls; port as needed
under each model.

| File | Role | Status |
|---|---|:--:|
| `PropertyPackageMethods.vb` | H/S/Cp/Cv/sound-speed/JT from an EOS | ◐ (`thermo::property_methods`, `ideal_props`) |
| `PropertyMethods.vb` | Pure-component correlations (Pvap, μ, k, σ) | ◐ (`thermo::transport`) |
| `ThermodynamicsBase.vb` | Core data types, reaction definitions | ◐ |
| `MichelsenBase.vb` | Stability / TPD, phase-split init | ✅ (`thermo::stability`) |

---

## 4a. Why this matters — HTGR water/steam ingress (priority driver)

**This chemistry port is important for HTGR water-ingress accident analysis.**
When water or steam ingresses into a hot graphite-moderated High-Temperature
Gas-cooled Reactor core, it drives graphite–gas chemistry:

$$C + H_2O \rightarrow CO + H_2 \quad (\text{steam-graphite, endothermic})$$

$$CO + H_2O \rightarrow CO_2 + H_2 \quad (\text{water-gas shift})$$

$$C + CO_2 \rightarrow 2\,CO \quad (\text{Boudouard})$$

This produces combustible CO/H₂, corrodes graphite, and adds reactivity and
pressure. Modelling it needs exactly the models this survey prioritises:

- **Equilibrium + kinetic gas-phase reactions** (`ReactionType::Equilibrium`,
  `::Kinetic`) and the **kinetic reactors** (CSTR/PFR) — the steam-graphite,
  water-gas-shift and Boudouard rate/equilibrium chemistry (bead `op-tts`).
- **Gibbs-minimisation speciation** — equilibrium CO/CO₂/H₂/H₂O/C partitioning
  without hand-listing every reaction (bead `op-4ng`).
- **Multicomponent gas-phase EOS** (LKP / PR-78) for the CO/CO₂/H₂/H₂O/He
  mixture properties (bead `op-b4t`).

The enabling modules for all three are now **ported** (`crate::reactions` /
`crate::reactors`, `thermo::gibbs` / `gibbs_multiphase`, `thermo::lkp` /
`pr1978`) — verification, not benchmark-validation, so the HTGR chemistry itself
still needs to be assembled and validated on these building blocks (beads
`op-tts`, `op-4ng`, `op-b4t`), alongside the molten-salt SLE/electrolyte work.

## 5. Port order for OUTRAM PARK — status

Ranked by reactor-chemistry value (not DWSIM's petroleum-first ordering). The
core VLE tier and the six high-value gaps below are now **ported** (verification,
not benchmark-validation — everything remains untrusted draft until human V&V):

1. ✅ **Solid-liquid equilibrium flash** (`NestedLoopsSLE`) — salt freezing /
   precipitation, the signature molten-salt-reactor need. `thermo::flash_sle`. ★★★
2. ✅ **Gibbs-minimisation flash + Gibbs reactor** (`GibbsMinimization3P` /
   `GibbsMinimizationMulti` + `Reactors/Gibbs`) — equilibrium speciation of
   salts / fission products without an explicit reaction list.
   `thermo::gibbs` / `gibbs_multiphase`, `reactors::GibbsReactor`. ★★★
3. ✅ **Electrolyte tier** (`ElectrolyteBase` → `ElectrolyteIdeal` → `LIQUAC2` +
   `ElectrolyteSVLE` flash) — aqueous coolant chemistry (boron/Li/pH,
   fission-product solubility) and ionic melts. `thermo::electrolyte` /
   `electrolyte_svle` / `sour_water`. ★★★
4. ✅ **Reaction models + kinetic/equilibrium reactors** (`ReactionType` +
   `Reactors/{Equilibrium, CSTR, PFR}`) — time-resolved corrosion / radiolysis
   / fission-product chemistry surrogates. `crate::reactions` / `crate::reactors`. ★★★
5. ✅ **3-phase & Inside-Out flashes** (`NestedLoops3PV3`, `BostonBrittInsideOut`,
   `BostonFournierInsideOut3P`) — off-gas / multiphase robustness.
   `thermo::flash_vlle` / `flash_insideout` / `flash_insideout_3p`. ★★
6. ✅ **Better EOS** (`LeeKeslerPlocker`, `PengRobinson78`, full PRSV2, PR+LK) —
   improved densities/enthalpies for cover gas and near-critical CO₂.
   `thermo::lkp` / `pr1978` / `prsv2_full` / `pr_lee_kesler`. ★★

**Remaining gaps** (not yet ported): the Mathias-Copeman / Twu α-variants and
advanced EOS (PC-SAFT, GERG-2008), the immiscible and forced-phase flashes, and
the LIQUAC full-package glue.

**Do not port** (use the existing OUTRAM crate or drop): steam tables →
`tampines-steam-tables`; CoolProp bridges → `outram-park-fork-coolprop`;
CAPE-OPEN / Reaktoro external interop; petroleum-specific packages (Black oil,
Chao-Seader, Grayson-Streed, Seawater) unless a specific case needs them.

---

## 6. Summary counts

| Category | Upstream models | Ported ✅ | Partial ◐ | Gap ⬜ | Out of scope ✗ |
|---|--:|--:|--:|--:|--:|
| Property packages | 30 | 15 | 4 | 4 | 7 |
| Flash algorithms | 23 | 11 | 3 | 4 | 5 |
| Reaction types | 4 | 4 | 0 | 0 | 0 |
| Reactors | 7 | 5 | 1 | 0 | 1 |

The fork now owns the **2-phase VLE + cubic-EOS + activity** core **plus** the
strategic OUTRAM PARK chemistry: **solid-liquid & Gibbs speciation flashes**, the
**electrolyte tier**, and **reaction/reactor** models — the chemistry a
molten-salt or aqueous-coolant reactor simulation actually exercises. All of it
is **verified, not benchmark-validated**, and untrusted draft until human V&V.
Remaining gaps are the advanced-EOS tier (PC-SAFT / GERG / Twu) and a few
petroleum-specific / external-bridge packages.
