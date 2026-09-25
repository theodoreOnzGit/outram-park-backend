# Reading the ACE photon-production blocks, and where the scope line is

GitHub **#307**, ACE gap 5. Implemented and measured 2026-09-25.

## The gap, and the honest shape of it

`acer::photon_blocks` **writes** MTRP/LSIGP/SIGP/LANDP/ANDP/LDLWP/DLWP, ported
from the photon half of `acelpp` (`acefc.f90:8214-9014`). Nothing read them back.

Two different things were being reported as one:

- **A data gap in `njoy-outram-park-fork`.** A crate whose job is nuclear data
  could say what it had written and not what it had been given. That is a real
  asymmetry and it is now closed.
- **A scope decision in `outram-mc-libs`.** That crate transports **neutrons
  only** — stated in its own `tally::filter` docs — so a `Nuclide` has nowhere to
  put a photon yield, spectrum or cosine law. Carrying them would be data no
  kernel reads.

Listing the second one beside the genuine omissions (delayed neutrons, URR, DBRC)
made it read as unfinished work. `Nuclide::from_ace`'s scope list now marks it as
a decision and points at the decoder, so the next reader does not re-open it as a
defect — and so whoever *does* add photon transport starts from a decoder rather
than from a format.

## What was implemented

`acer::photon_read::decode_photon_production`, ported from
`openmc/data/reaction.py::_get_photon_products_ace` (OpenMC `afa7a14`).

| block | JXS | holds |
|---|---|---|
| MTRP | 13 | `MT·1000 + k` per photon **subsection** |
| LSIGP | 14 | locator into SIGP |
| SIGP | 15 | `MFTYPE`, then a yield (12/16) or a cross section (13) |
| LANDP | 16 | locator into ANDP, `0` ⇒ isotropic in the laboratory |
| ANDP | 17 | the photon cosine distributions |
| LDLWP | 18 | locator into DLWP |
| DLWP | 19 | the photon energy laws — LAW=2 (a line) or LAW=4 (a spectrum) |

Three things it gets right on purpose:

- **`MFTYPE` decides what the numbers mean.** `13` is a production *cross
  section* on the table's own grid from a 1-based `IE`; `12`/`16` are a *yield*
  to be multiplied by the `MTMULT` reaction's cross section. Reading a yield as a
  cross section understates production by the size of the cross section and reads
  as a plausible small number rather than as an error. `AcePhotonRate` is an enum
  over the two, so a consumer cannot conflate them, and
  `photon_production_xs_at` returns the **count of yield entries it skipped**
  rather than silently summing a partial total.
- **One reader per format, not one per block.** DLWP has the identical
  `[LNW, LAW, IDAT]` + applicability layout as DLW, so it goes through
  `ce_laws::decode_law_chain` — which is why ACE LAW=2 is *decoded* in that module
  rather than refused. ANDP goes through the neutron AND reader, generalised to
  `decode_angular_block(land, and, …)`; upstream reads both pairs through one
  `AngleDistribution.from_ace` for the same reason.
- **An entry is one subsection, not one reaction.** U-235 has 583 entries across
  81 MTs, so `MTRP / 1000` is the neutron MT and the remainder says which
  subsection of it.

## Methodology

Every number is compared against **OpenMC's own Python reader** walking the same
bytes — `openmc.data.ace.get_table` plus the same seven `JXS` blocks — so a
disagreement is between two readers rather than a self-consistency check. U-235
comes from the committed `reference-data/ace` submodule; Na-23, C-12 and H-2 are
regenerated with the NJOY deck in `ace_dlw_law_family_vs_njoy2016.rs` and gated on
`OUTRAM_ACE_LAW_DIR`. They are included because they cover what U-235 does not: an
`MFTYPE = 13` entry, a **non-isotropic** ANDP entry, and a single-line table.

## Results (2026-09-25)

| table | `NTRP` | `MFTYPE` census | DLWP law census | `LANDP = 0` | matches OpenMC |
|---|---|---|---|---|---|
| U-235 293.6 K | 583 | `{12: 577, 16: 6}` | `{2: 576, 4: 7}` | 583/583 | yes |
| Na-23 0 K | 247 | `{12: 246, 13: 1}` | `{2: 246, 4: 1}` | 247/247 | yes |
| C-12 0 K | 5 | `{12: 4, 16: 1}` | `{2: 4, 4: 1}` | **4**/5 | yes |
| H-2 0 K | 1 | `{12: 1}` | `{2: 1}` | 1/1 | yes |

Physical spot checks, which a count census cannot give:

- **U-235's first entry is `MTRP = 18001`** — fission's own photon subsection —
  carrying an MF=12 yield on MT=18 and a LAW=4 continuum spectrum. Its 576
  discrete lines run from **0.0001 to 1.2587 MeV**, so the MeV→eV scaling is right
  (a slip would put them at keV or TeV).
- **H-2's single entry is a discrete line at 6.251 MeV**, which is the
  `n + d → t + γ` capture gamma (6.257 MeV from the mass difference). A wrong `LP`
  read would return the *flag* as the energy, and a MeV/eV slip a factor of a
  million; neither survives that assertion.
- **Na-23's one `MFTYPE = 13` entry** is `MTRP = 3001` — the (n,γ) production
  cross section written as a cross section — 762 points from grid index 3781,
  peaking at **1.9029 b**.
- **C-12's `MTRP = 51001` carries a real ANDP distribution** at 72 incident
  energies, which is the only held case that exercises the LANDP/ANDP path at all.

Gates: `njoy-outram-park-fork`'s `tests/ace_photon_production_read.rs` — 3 passed.

## What this does not claim

- **Nothing transports these photons.** No `k`, no heating, no dose changes. The
  claim is that the blocks are read correctly, and that the scope line is where it
  is by decision.
- **MF=12 `LO=2` cascades and anisotropic MF=14 photons** are still unwritten by
  `photon_blocks::build` (it refuses the whole photon block rather than writing a
  partial one). The reader does not care — it reads what a file contains — but the
  round trip cannot be exercised for those forms from this repository's tapes.
- **`GPD` (`JXS(12)`, the old 30×20 photon-production table)** is not read. It is
  the pre-1980 format, and upstream's continuous-energy reader does not read it
  either: `_get_photon_products_ace` never touches `JXS(12)`, and the only
  `ace.jxs[12]` anywhere in `openmc/data` is the **photoatomic** class's
  electron-count block (`photon.py:237`), which is a different class and a
  different meaning for the same locator. Checked, not assumed.
