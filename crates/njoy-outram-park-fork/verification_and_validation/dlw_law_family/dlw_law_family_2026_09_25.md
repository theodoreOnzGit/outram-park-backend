# Every DLW law upstream reads, read here — and the census that hid four of them

GitHub **#307**, ACE gap 3. Implemented and measured 2026-09-25.

## The defect, and how a true measurement became a false claim

`acer::ce_laws::decode_energy_law` read ACE DLW laws `{3, 4, 44, 61}` and
refused everything else, citing a census taken over `reference-data/ace`:

> Measured over the whole NJOY2016 reference library in `reference-data/ace`,
> these are the **only** neutron laws present.

That sentence was *true of the files it was measured on* and false as written.
The reference library holds U-234, U-235 and U-238 and nothing else. Running
NJOY2016 over four more tapes **already committed in `reference-data/endf/`**,
with the same 0 K `RECONR + ACER` deck those tables were made with, produces four
laws the decoder refused outright:

| tape | MT | ACE law | from | reason it is there |
|---|---|---|---|---|
| H-2 VIII.0 | 16 | **66** | MF=6 LAW=6 | `n`-body phase space |
| C-12 VIII.0 | 28, 91 | **9** | MF=5 LF=9 | evaporation |
| Na-23 VIII.0 | 16 | **9** | MF=5 LF=9 | evaporation |
| Na-23 VIII.0 | 91 | **9, 9** | MF=5 LF=9, NK=2 | an `LNW` chain |
| Be-9 VIII.0 | 16 | 61 | MF=6 **LAW=7** | already read |

So `Nuclide::from_ace` could not build C-12, Na-23 or H-2 at all — three of the
lightest, most commonly used moderator and coolant nuclides in the workspace.
The actinides worked, which is why nothing failed.

**The lesson is about the wording, not the omission.** "Measured over the
library" was an accurate description of a measurement whose *scope* was three
actinides; read a month later it said something much stronger. The census in the
module doc now names the tapes it was taken over and is struck through rather than
replaced, so the next reader can see which claim was retired.

## What upstream reads, checked rather than assumed

`openmc/data/angle_energy.py::AngleEnergy.from_ace` at OpenMC `afa7a14`
dispatches exactly `{2, 3, 33, 4, 5, 7, 9, 11, 44, 61, 66}` and raises on
anything else. All eleven are now decoded here, with two of them decoded as
refusals for reasons taken from upstream rather than invented:

- **LAW=5** (general evaporation) — upstream *dispatches* it and then raises
  `NotImplementedError` in `GeneralEvaporation.from_ace`
  (`energy_distribution.py:192-194`). NJOY2016 does support ENDF LF=5
  (`groupr.f90:12355`, `acefc.f90:2251`), and the one place LF=5 occurs in
  `reference-data/endf/` — the **MT=455 delayed spectra** of U-234, U-235 (VII.0
  and VIII.0), U-238 (VII.0, VIII.0, JENDL-3.3) and Pu-239 JENDL-3.3 — ACER
  linearises into ACE LAW=4. Measured: the DNED block of every reference table is
  `{LAW4: 6}`. So no held table carries law 5 and a reader for it could not be
  verified against anything.
- **LAW=67** (laboratory angle-energy) — upstream refuses it too. Measured:
  ACER converts the ENDF form it comes from (Be-9's MF=6 LAW=7 on MT=16) into ACE
  **law 61**, which this port already read. Law 67 does not appear in any table
  generated from this repository's tapes.

A **stale claim was corrected in the same change**: `nuclear_data::secondary`'s
`parse_mf5_section` said LF=5 was unported "because no evaluation in
`reference-data/endf/` uses it; `mf5_lf_survey` asserts that". Seven tapes do use
it, and the survey walks MT ∈ {18, 16, 91, 5} only, so it could never have
failed. Nothing is silently degraded — the delayed *spectrum* is dropped on both
routes — but the comment said something untrue about the data.

## What was implemented

| law | representation | sampler it reaches |
|---|---|---|
| 2 | `AceEnergyLaw::DiscretePhoton { lp, eg }` | none — photon production is out of this crate's transport scope |
| 3, 33 | `TwoBodyLevel` (unchanged) | analytic two-body kinematics |
| 7, 9, 11 | `Analytic { law, spectrum }` carrying `FissionSpectrum::{Maxwell, Evaporation, WattEnergyDependent}` | `sample_maxwell_lf7` / `sample_evaporation_lf9` / `sample_watt_lf11` — the existing ports of OpenMC's C++ samplers |
| 66 | `PhaseSpace { npsx, apsx }` + `ace_phase_space_chi` | the tabulated continuum sampler |
| `LNW != 0` | `Mixture(Vec<(Tab1, AceEnergyLaw)>)` | `sample_chi`'s existing NK>1 arm |

Three reuse decisions, in preference to new code:

1. **The analytic laws reuse `FissionSpectrum`.** ACE law 7/9/11 and ENDF MF=5
   LF=7/9/11 are the same three laws with the same parameters; only the units
   differ. On a non-fission reaction they go into the *same*
   `UncorrelatedEmission` slot the ENDF route uses, so MT=91/16/17 sample
   through one code path whichever route built the nuclide.
2. **Law 66 reuses two existing pieces**: `law66_shape_table` (the port of
   `acefc.f90`'s `acelf6` grid, so the reader rebuilds the shape ACER would have
   written — ACE stores only `NPSX`/`APSX`) and a newly-shared
   `secondary::phase_space_chi`, factored out of the ENDF MF=6 LAW=6 path so both
   routes use one `E'_max(E)`.
3. **`as_fission_spectrum()` returns `None` for laws 44 and 61 deliberately.**
   Flattening a correlated energy-angle law into an MF=5 spectrum would drop the
   correlation silently; a caller that wants those must match the variant.

`read_tab1_full` keeps the ACE TAB1's **interpolation regions**, which the
previous `read_tab1` discarded. That is load-bearing and not tidiness: Na-23's
MT=91 `θ(E)` is `INT = 5` (log-log) with a breakpoint at point 9. Evaluating it
lin-lin is a different `θ(E)` and nothing downstream would notice.

## Methodology

Three levels, each against an oracle that is not the code under test.

1. **`njoy`'s `tests/ace_dlw_law_family.rs`** — the smallest ACE table that
   contains each law, written word by word, so the whole content of the test is
   the index arithmetic and the unit scaling.
2. **`njoy`'s `tests/ace_dlw_law_family_vs_njoy2016.rs`** — the four real
   NJOY2016 tables, with every value cross-checked against what **OpenMC's own
   Python reader** (`openmc.data.IncidentNeutron.from_ace`, `afa7a14`) reads out
   of the same bytes. Gated on `OUTRAM_ACE_LAW_DIR`; the deck is in the file.
3. **`outram-mc-libs`'s `tests/ace_new_laws_reach_transport.rs`** — build the
   nuclide through `Nuclide::from_ace` and sample what the kernel would sample,
   against a closed-form moment (evaporation) and the law's own kinematic ceiling
   (phase space).

## Results (2026-09-25)

**Reading, against OpenMC's numbers.** All five cases agree exactly — the ACE
words are the same `f64`s in both readers:

| table | MT | law | value |
|---|---|---|---|
| H-2 | 16 | 66 | `NPSX = 3`, `APSX = 2.99862`, `Q = -2.225002` MeV |
| C-12 | 28 | 9 | `U = 15.957` MeV, `θ(E₁) = 0.3` MeV, 4 points |
| C-12 | 91 | 9 | `U = 7.8864` MeV, `θ` flat at 0.3 MeV |
| Na-23 | 16 | 9 | `U = 12.414` MeV, `θ(E₁) = 0.01` MeV |
| Na-23 | 91 | 9, 9 | `U = 6.1` and `0.47` MeV; `p₁ = (1, 0, 0)`, `p₂ = (0, 1, 1)` on `(6.1, 12, 20)` MeV, histogram |
| Be-9 | 16 | 61 | 24 correlated incident rows — no law 67 |

Full law census of the four tables, printed by the test rather than asserted
from a comment: `H2 {66: 1}`, `Be9 {61: 1}`, `C12 {3: 12, 9: 2, 44: 1}`,
`Na23 {3: 18, 9: 1, "9 (×2)": 1}`.

**Transport, against oracles that are not the sampler:**

| case | sampled | oracle | agreement |
|---|---|---|---|
| C-12 MT=91 evaporation, `⟨E'⟩` at 12 MeV, 200 000 histories | `5.99286e5 ± 9.5e2` eV | `5.99937e5` eV = `θ·γ(3,y)/γ(2,y)`, closed form | **0.69 σ** |
| Na-23 MT=91 at 10 MeV, 50 000 histories | `max E' = 3.9000e6` eV | `E − U₁ = 3.900e6` eV | at the bound |
| Na-23 MT=91 at 15 MeV | `max E' = 1.4526e7` eV | `E − U₂ = 1.453e7` eV | the switch fires |
| H-2 MT=16 phase space at 14 MeV | `⟨E'⟩ = 3.9524e6`, `max = 1.1504e7` eV | lab ceiling `(√E'_max,cm + √(E/(A+1)²))² = 1.1727e7` eV | under the ceiling |

**The Na-23 pair is the sharpest result here**, and it is a support test rather
than a moment test: the two links of the chain differ only in their restriction
energy, so if the applicability were ignored — or both links read as one law —
the 15 MeV arm would still be bounded by 8.9 MeV instead of reaching 14.5. It is
a check the chain *switches*, not a check that an average comes out right.

## What this does not claim

- **Laws 7 and 11 are read but unexercised by held data.** No tape in
  `reference-data/endf/` carries MF=5 LF=7 or LF=11, so those two are verified
  against the format (hand-built words, upstream's index arithmetic) and not
  against a file NJOY wrote. Stated here rather than left to be inferred from the
  absence of a row above.
- **Law 2 is decoded and goes nowhere.** Photon production is out of scope for
  `outram-mc-libs`; the variant exists so the DLW and DLWP blocks can share one
  reader.
- **No `k_eff` moves.** None of H-2, C-12, Na-23 or Be-9 appears in this
  workspace's criticality cases. What this closes is a construction that failed
  outright and a claim that was read as broader than it was measured.
- **The four tables are not committed.** They are regenerable in ~40 s from tapes
  that are; the deck is in the test's module doc and the test skips with that
  note when `OUTRAM_ACE_LAW_DIR` is unset.
