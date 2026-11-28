# What the BISON Theory Manual holds for unit testing

> **Research, education and V&V only.** AI-written inventory, 2026-11-28,
> not reviewed by a human.

**Source.** Hales et al. (2013), *BISON Theory Manual*, INL/EXT-13-29930. It is in
the maintainer's private corpus (`propreitrary-literature-theodore`,
`reports/inl-ext-13-29930-bison-theory-manual-2013.pdf`). Every figure and
table caption was listed from the extracted text of all 72 pages on
2026-11-28. Each item was then checked against the workspace for code that
implements the equation it shows.

**The short answer.** The manual has **no figure, table or test data for its
TRISO laws** (Section 10.1–10.2). For those, the only check available is a hand
evaluation of each printed formula, which
`triso_particle_mechanics::tests::bison_manual_section_10_hand_values` does. Its
figures are mostly plots of its own UO2/MOX correlations. A plot of a
correlation can check a transcription of that correlation, which is
verification. None is a measurement.

| Item | Page | What it shows | Usable as | Workspace code | Status |
|---|---|---|---|---|---|
| Fig. 8.1 | 17 | UO2 conductivity, Fink-Lucuta vs MATPRO | A transcription check of both curves (digitise) | `offbeat` `ConductivityModel::MatproUo2`; Fink-Lucuta not found by name | Not done |
| Fig. 8.2 | 18 | Fink-Lucuta vs Halden | As above | `offbeat` Halden IFA-601 fit (conductivity.rs:259) | Not done |
| Fig. 8.3 | 19 | Fink-Lucuta vs NFIR | As above | `offbeat` `ConductivityModel::NfirUo2` | Not done |
| Fig. 8.4 | 21 | Fink-Lucuta vs modified NFI | As above | `offbeat` modified-NFI form | Not done |
| Fig. 8.5, 8.6 | 23 | Unirradiated UO2 and MOX conductivity | As above | `offbeat` `KatoMox`, `LanningBeyerMox`, `MagniMamox` | Not done |
| **Fig. 8.7** | 24 | MATPRO UO2 gaseous and total swelling vs T, 10/20/30 MWd/kgU (Eqs (8.21)–(8.22)) | **The shape of the gaseous curve, independent of density** | `offbeat` `SwellingModel::Uo2Matpro` | **Done:** `outram-park-fork-offbeat/tests/bison_manual_fig87_matpro_swelling.rs`; 5 points within 0.003 of the digitised ratio. The magnitudes cannot be checked because the figure does not state its density or burnup basis |
| Fig. 8.8 | 27 | Activation-energy function of UO2 thermal creep | Transcription check | `offbeat` `CreepModel::Matpro` | Not done |
| Fig. 8.9 | 29 | MOX vs UO2 creep rates | Transcription check | `offbeat` creep models | Not done |
| Fig. 8.10 + Table 8.1 | 37 | Modified Forsberg-Massih FGR; centreline T at 1 % release, against the Vitanza threshold | Code-to-correlation comparison | **None**: `offbeat` has no Forsberg-Massih (fgr/mod.rs:86) | Nothing to test |
| Table 11.1 | 46 | TUBRNP spectrum-averaged cross sections | Constants | **None**: TUBRNP/Lassmann not ported (`offbeat` burnup/mod.rs:87) | Nothing to test |
| Figs. 11.1, 11.2 | 47 | **BISON vs FRAPCON** radial power factor, fresh and at 33 MWd/kgU (20 kW/m, 5 % enriched, 95 % TD) | The manual's only **code-to-code** data | **None**: no TUBRNP | Usable once a radial-power model exists |
| Figs. 14.1, 14.2 | 60–61 | Schematics (heat-transfer regime logic, IAPWS-IF97 regions) | No numbers | — | — |

## Section 10 (TRISO): values worked by hand from the printed formulas

| Equation | Point | Hand value | Test |
|---|---|---|---|
| (10.1) buffer | Φ = 1 (10²⁵ n/m²) | −0.176 e^(−1.75) = −0.0305842 | `bison_manual_section_10_hand_values` |
| (10.2) dense PyC, radial | Φ = 1 | −0.077 e^(−1) + 0.031 = +0.0026733 | same |
| (10.3) dense PyC, tangential | Φ = 1 | −0.036 e^(−2.1) − 0.01 = −0.0144084 | same |
| (10.1)–(10.3) | Φ = 0 | −0.176; −0.046; −0.046 | `dimensional_change_is_the_integral_of_the_printed_rate` |
| (10.5)–(10.6) PyC creep K | 600 °C, ρ = 1.9 | 1.325168e-29 per MPa·n/m² | `bison_manual_section_10_hand_values` |
| (10.5)–(10.6) | 1000 °C, ρ = 1.9 and 1.8 | 2.4708e-29; density factor 1.238 | `pyc_creep_coefficient_transcription` |
| (10.7) SiC creep K | 640 °C, 900 °C | 2e-37, 4e-37 per Pa·n/m² | `bison_manual_section_10_hand_values` |
| ν_c, M_Irr,Creep | — | 0.5, 2 | same |

## Worth knowing before the next step

- **TRISO layer properties already exist in `outram-park-fork-offbeat`**,
  ported from OFFBEAT's PARFUME correlations:
  - Young's modulus: `YoungModulusModel::ParfumeBuffer`, `ParfumePyC`;
  - conductivity: `ConductivityModel::ParfumeBuffer`, `ParfumeSiC`;
  - thermal expansion: `PARFUMEBuffer`, `PARFUMEPyC`, `PARFUMESiC`, `SneadSiC`;
  - heat capacity: `SneadSiC`.

  `triso_particle_mechanics` takes these as inputs. Using them directly would
  need a new dependency, `boon-lay → outram-park-fork-offbeat`, which has not
  been asked for.
- The BISON vs FRAPCON radial-power figures are the manual's only
  code-to-code data. They need a TUBRNP-type radial power model, which nothing
  in the workspace has.
