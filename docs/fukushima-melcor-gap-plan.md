# Fukushima Unit 1 simulator: closing the MELCOR gap — plan

**Status:** plan only; nothing in it is built. **Date:** 2026-10-28.
**Epic:** gh:#1028. **Gap analysis this plan answers:**
[#1028 comment of 2026-10-28](https://github.com/theodoreOnzGit/outram-park-backend/issues/1028#issuecomment-6483771300).

> **Research, education and V&V only** (`RESPONSIBLE_USE.md`). A severe-accident
> reconstruction is not a licensing input, not a PSA, not an emergency-response
> tool, and not authoritative for any facility. AI draft, untrusted until a
> human reviews it.

## 0. What this is for

The Fukushima Unit 1 simulator, ~~`crates/dhoby-ghaut/examples/fukushima_web/`~~
`sembawang::bwr_mark1` (`crates/sembawang/src/bwr_mark1/`, moved 2026-10-29 by
F2, gh:#1033; the `dhoby-ghaut` `fukushima_web` demo draws it), is
a reduced model. Against BSAF Phase II (NEA No. 7525) it misses, uncalibrated:

| | Model | BSAF Phase II, Unit 1 |
|---|---|---|
| Lower-head failure | ≈ 5.5 h | 7.45–17.16 h |
| In-vessel hydrogen | 127 kg | ≈ 400–1 310 kg *(figure-read)* |
| Peak drywell pressure | 0.53 MPa | 0.8–1.0 MPa |
| Wetwell vent, building explosion | never | ≈ 24 h, 24.83 h *(observed)* |
| Cs-137 | not computed | 5.5–43.2 PBq, three units |

This plan closes those gaps one physical mechanism at a time, measuring how much
of each miss each mechanism carries. It does **not** port MELCOR, and nobody on
it may read MELCOR source (`docs/melcor-parity-scoping.md` §1.1). Every model is
written from the public document it cites.

## 1. Rules every item follows

These are the root `CLAUDE.md` rules, restated where they bite hardest here.

1. **The baseline is uncalibrated, and stays so.** No item tunes a coefficient
   toward a BSAF band. The bands are the check; the moment one becomes an input
   the check is gone. An honestly-derived miss is a result.
2. **Prediction before measurement.** Each item states, before it is run, which
   way it should move each Unit 1 metric and roughly how far. A change that
   moves a metric the wrong way means the hypothesis is wrong; stop and re-read
   the source.
3. **One mechanism, one ablation.** Each new mechanism lands with a
   `without_*` switch (default **on**, per the correct-physics-default rule), so
   the harness of F1 can report how much of each miss it carries.
4. **Which BSAF numbers count.** Measured (Tier A) and well-constrained (Tier B)
   quantities in `derived/nea7525-bsaf-fukushima-daiichi-bands.md` §"tiering"
   are checks. Tier C spreads are context only, and Tier D (MCCI erosion to
   500 h) is **never** a target: the report itself calls it unrealistic.
5. **Every input cites its public source** at its definition, with its
   `[P]`/`[U]`/`[A]` tag kept honest (`params.rs` convention).
6. **V&V documentation** carries methodology and measured results with dates,
   in the test's doc comment.
7. **Copyright**: see §4. The access tier comes from the document's own
   statement, never from where it is hosted.

## 2. The items, in dependency order

The issue numbers are in §3. "Expected effect" is the predicted sign on the
Unit 1 metrics; none is measured yet.

| Item | What | Expected effect | Needs |
|---|---|---|---|
| **F1** | Unit 1 comparison harness and ablation switches | none (measures) | BSAF bands (held) |
| **F2** | Move the Unit 1 physics into `sembawang` as library code | none intended; PETIR changes bits, re-bless with a recorded diff | ~~maintainer go-ahead~~ approved 2026-10-29 ("Physics libraries go to sembawang") |
| **F3** | Lower head: heat structure, debris-to-head heat transfer, penetration and creep failure | lower-head failure **later** | L1/L3, L10 |
| **F4** | Core: candling, continued oxidation of relocating Zr, core-plate retention and support failure | H₂ **up**; failure **later** | L1/L3, L12, L13 |
| **F5** | RPV two-phase mixture level (swell) | uncovery **later** | L1/L2, L20 |
| **F6** | RPV wall and internals as heat structures, in-core radiation, main-steam-line creep as a leak path | heat-up **slower**; leak path physical | L1/L3, L4, L10, L21 |
| **F7** | Temperature-dependent fuel and clad properties from OFFBEAT; one cited Zr–steam reaction heat | small, either sign; removes `[A]` constants | L13, L14 |
| **F8** | Multi-non-condensable containment (CO, CO₂, O₂) and MCCI gas generation | drywell pressure **up** after breach | L1/L3, L15 |
| **F9** | Fission products: release from UO₂, transport, pool scrubbing, aerosol settling; wired to `changi` | new output: Cs-137 against BSAF | L1/L3, L16, L17 |
| **F10** | Decay heat: actinide mix, irradiation history, decay heat leaving with fission products | small; decay power **lower** later | L17, L18 |
| **F11** | Hydrogen to the building, and combustion computed (flammability, steam inerting, deflagration) | building explosion becomes reachable | L19 |
| **F12** | Injection that bypasses the core, long-term containment leak, runs to 500 h | debris hotter, drywell pressure **up** | L9 |
| **F13** | Check every `[U]`/`[A]` design value and event time against open sources | corrects inputs; either sign | L4, L5, L6, L7, L9 |
| **F14** | Units 2 and 3 (RCIC, HPCI, ADS, torus stratification) | new units | `sembawang` CV EOS (`melcor-scoping.md` §3.1), L4, L9, L22 |

Order: **F1 first** (everything is measured through it), and **F13 early**,
because it is cheap and an input error can masquerade as a missing mechanism.
F3 and F4 next, as the leading candidates for the two largest misses. F14 waits
on the two-phase multi-component control-volume EOS that `melcor-scoping.md`
§3.1 names as the blocker for everything beyond a reduced model.

F3–F12 can be prototyped in the reduced model first. That says which mechanisms
carry the misses before the long `sembawang` programme starts. ~~If F2 is
approved, they land in `sembawang` directly.~~ F2 was approved 2026-10-29: they
land in `sembawang::bwr_mark1` directly.

## 3. Issues

All are sub-issues of #1028, filed 2026-10-28.

| Item | Issue | Priority |
|---|---|---|
| Literature and copyright tiers (§4) | #1031 | P1 |
| F1 comparison harness and ablations | #1032 | P1 |
| F2 move into `sembawang` (~~needs go-ahead~~ approved 2026-10-29) | #1033 | P2 |
| F3 lower head | #1034 | P1 |
| F4 candling, oxidation, core plate | #1035 | P1 |
| F5 mixture level | #1036 | P2 |
| F6 RPV heat structures, radiation, MSL creep | #1037 | P2 |
| F7 properties from OFFBEAT; reaction heat | #1038 | P2 (`bug` for the 6.45 vs 6.763 MJ/kg disagreement) |
| F8 multi-NCG and MCCI gas | #1039 | P2 |
| F9 fission products to `changi` | #1040 | P2 |
| F10 decay heat | #1041 | P3 |
| F11 hydrogen combustion | #1042 | P2 |
| F12 injection bypass, leakage, 500 h | #1043 | P2 |
| F13 input and event-time check | #1044 | P1 |
| F14 Units 2 and 3 (blocked) | #1045 | P3 |

## 4. Literature needed, and its copyright

**The tiers below are expected, not verified.** A document's tier comes from
its own copyright statement (`DATA_POLICY.md`; `crates/kovan-literature/CLAUDE.md`),
and none of these could be opened here: this environment's network policy
blocks `nrc.gov`, `osti.gov` and `sandia.gov`. Each is checked against its
title page on ingestion, and the tier corrected if it differs.

Tier key:

- **public**: may go into `reactor-literature` (public), with a verified
  redistribution basis recorded in its README row.
- **private**: copyrighted with no redistribution grant. The PDF goes to the
  private repository; facts and numbers are cited with acknowledgement.
- **ruling**: needs a maintainer decision before anything is done with it.

Identifiers marked † are from memory and must be confirmed on retrieval.

| Code | Document | Used by | Expected tier and basis |
|---|---|---|---|
| **L1** | MELCOR Computer Code Manuals Vol. 2, Reference Manual, SAND2017-0876 O (ADAMS ML17040A420) | F3–F9 | **public**: Sandia report for NRC, unlimited release; NRC ADAMS public |
| **L2** | MELCOR Vol. 1, Primer and Users' Guide, SAND2017-0455 O (ML17040A429); later rev. SAND2021-0726 O (ML21042B319) | F5, executive | **public**: as L1 |
| **L3** | MELCOR 1.8.5 Manuals Vol. 2 Rev. 2, NUREG/CR-6119 / SAND2000-2417 (ML010190117), incl. the COR and BH package references (ML010190222) | F3, F4, F6 | **public**: NUREG/CR, US government |
| **L4** | Gauntt et al., *Fukushima Daiichi Accident Study (Status as of April 2012)*, SAND2012-6173 | F6, F13, F14 | **public**: Sandia, unlimited release (to verify) |
| **L5** | INPO 11-005, *Special Report on the Nuclear Accident at the Fukushima Daiichi NPS* (2011) | F13 | **private**: © INPO |
| **L6** | IAEA, *The Fukushima Daiichi Accident*, Technical Volume 1 (2015) | F13 | **private**: © IAEA |
| **L7** | Government of Japan, *Report to the IAEA Ministerial Conference on Nuclear Safety* (June 2011), attachments | F13 | **private** until its statement is checked |
| **L8** | TEPCO accident investigation reports and plant parameter data | F13, F12 | **ruling**: © TEPCO, *and* measured plant data sits close to `DATA_POLICY.md`'s "operational facility data". Until ruled on, the BSAF-published traces (held) stand in |
| **L9** | OECD/NEA BSAF Phase II full report (the summary, NEA No. 7525, is already held, private); Phase I report | F12, F13, F14 | **private**: © OECD |
| **L10** | Rempe et al., *Light Water Reactor Lower Head Failure Analysis*, NUREG/CR-5642† (1993); Chu et al., *Lower Head Failure Experiments and Analyses*, NUREG/CR-5582† (1998/99), incl. SA533B creep data | F3, F6 | **public**: NUREG/CR, US government |
| **L11** | OECD/NEA OLHF programme report (lower-head failure) | F3 (optional) | **private**: © OECD |
| **L12** | CORA (Hagen et al., KfK reports) and QUENCH (FZKA/KIT reports) bundle tests | F4 V&V | **private** until KIT's statement is checked |
| **L13** | Urbanic & Heidrick, *J. Nucl. Mater.* 75 (1978); Cathcart et al., ORNL/NUREG-17 (1977); Baker & Just, ANL-6548 (1962) | F4, F7 | Urbanic–Heidrick **private** (© Elsevier); ORNL/NUREG-17 and ANL-6548 **public** (US government) |
| **L14** | SCDAP/RELAP5 MATPRO, NUREG/CR-6150† Vol. 4 (the manual only, never the code); Fink, *J. Nucl. Mater.* 279 (2000) | F7 | MATPRO **public** (NUREG/CR); Fink **private** (© Elsevier). Check what OFFBEAT already cites first |
| **L15** | CORCON-Mod3 manual, NUREG/CR-5843†; OECD MCCI / CCI final reports; Unit 1 concrete composition (source to find) | F8 | CORCON **public**; OECD **private**; concrete: unknown |
| **L16** | NUREG-0772 (1981); CORSOR-M, NUREG/CR-4173†; MAEROS, NUREG/CR-1391†; SPARC-90, NUREG/CR-5765†; NUREG-1465; PHÉBUS-FP FPT-1 final report | F9 | NUREGs **public**; PHÉBUS **private** (© IRSN/EC) |
| **L17** | A Unit 1 core inventory: Nishihara et al., JAEA-Data/Code 2012-018† — or computed with `outram-mc` depletion, which needs no literature beyond the core design (L4) | F9, F10 | JAEA **private** until its statement is checked |
| **L18** | ANS-5.1 decay heat standard (2014 edition) | F10 | **private**: a purchased standard (`melcor-parity-scoping.md` §1.7); only if the maintainer holds a copy. The 1978 draft already in `teh-o-prke` covers the actinide cases |
| **L19** | Coward & Jones, US Bureau of Mines Bulletin 503 (1952), flammability limits; NEA/CSNI/R(2000)7, *Flame Acceleration and DDT in Nuclear Safety* | F11 | Bulletin 503 **public** (US government); CSNI **private** (© OECD) |
| **L20** | A public drift-flux model for the mixture level: an NRC code's theory manual (manual only); Zuber & Findlay, *J. Heat Transfer* 87 (1965) | F5 | NRC manual **public**; Zuber–Findlay **private** (© ASME) |
| **L21** | Radiation between core components: L1/L3's COR sections; a heat-transfer textbook (e.g. Siegel & Howell) | F6 | L1/L3 **public**; textbook **private** (maintainer's own copy) |
| **L22** | Sandia reports on RCIC (Terry turbine) beyond-design operation† | F14 | **public** (to verify) |

**Most needed first:** L1–L4 (public, but blocked by the network policy), L9
(full BSAF report), L10, L13 and L16. Then L5–L7 for F13.

## 5. What this plan does not do

- No MELCOR, ASTEC, MAAP, RELAP5, SCDAP/RELAP5 or MOOSE-application source, even
  "just to check an equation" (`melcor-parity-scoping.md` §1.1, §8).
- No calibration toward BSAF bands, and nothing gated on Tier C or D quantities.
- No plant data that `DATA_POLICY.md` excludes. L8 waits for a ruling.
