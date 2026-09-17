# Golden NJOY2016 HEATR tapes for V&V (repo-tracked, NOT crate-packaged)

Heating (KERMA, MT=301) and damage-energy production (MT=443–446) written by the
upstream Fortran NJOY2016 `HEATR` module, used as a like-for-like oracle for
`crates/njoy-outram-park-fork/src/heatr/` (`tests/heatr_vs_njoy2016.rs`). Like
`../endf/`, `../reconr/`, `../gaspr/`, `../errorr/` and `../leapr/`, they live at
the repository root, outside `crates/`, so they are git-tracked but never part of
a published crate tarball. Read them through
`njoy_outram_park_fork::reference_data::reference_file("heatr", …)`.

Each tape is the deck's `tape24` **trimmed to MF=1 plus MF=3/MT=301, 443, 444,
445, 446**. The `.njoy-input` beside each tape is the verbatim deck that produced
it, so the reference regenerates rather than being trusted.

## `local = 1` — and why it matters

The decks pass `local = 1` on HEATR's card 2, which deposits photon energy
locally instead of transporting it. That is the regime this crate's `Kerma`
models (its H2 arm is defined as `H = sigma*(E + Q)`, everything deposited), so
MT=301 from a `local = 1` run is the correct comparand. A `local = 0` MT=301 is
the energy-balance KERMA and is a different quantity.

MT=445 and MT=446 are unaffected by the flag — verified byte-identical between a
`local = 0` and a `local = 1` run on both nuclides. MT=444 **is** affected, via
its MT=447 disappearance term, which depends on the capture-recoil treatment
(Si-28's MT=444 differs between the two runs; Fe-58's does not).

## Provenance

NJOY2016 upstream `ac5adf5f33d893e42f2eed7fb286b0d51c7580da` (2026-04-06), built
from source and executed 2026-09-17. Inputs are the committed ENDF/B-VIII.0
evaluations in `../endf/`:

| tape | evaluation | MAT | Z | `E_d` |
|---|---|---|---|---|
| `fe58-…` | `n-026_Fe_058-ENDF8.0.endf` | 2637 | 26 | 40 eV |
| `si28-…` | `n-014_Si_028-ENDF8.0.endf` | 1425 | 14 | 25 eV |

Both at `RECONR err = 0.001`, 0 K (no BROADR), `E_d` from HEATR's built-in table.

## Why these two, and why U-238 is absent

Fe-58 and Si-28 are structural materials, which is where damage energy is
actually used, and they bracket the mass range where the isotropic-CM
approximation this crate makes starts to fail. A **U-238** deck is committed
here (`u238-ENDF8.0-0K-local1.njoy-input`) but its tape is **not**: HEATR
tabulates on the full resonance grid and the full output is 144 MB and even trimmed to MT=301/443-446 it is 47 MB, too large
to carry in git for the value it adds. Regenerate it from the deck if needed — the deck was executed on 2026-09-17 to confirm it runs, and produced MT=301, 443, 444, 445 and 446.
U-238's KERMA is already covered by `tests/heatr_kerma_vs_kinematics.rs`, which
checks it against closed-form kinematics at machine precision.

The write-up, including the two measured gaps this comparison quantified, is
`../../crates/njoy-outram-park-fork/verification_and_validation/heatr_vs_njoy2016.md`.
