# NJOY2016 thermal ACE reference tables and their decks

Reference tables for `crates/njoy-outram-park-fork/tests/acer_thermal_word_parity.rs`
(GitHub #865), and the NJOY2016 decks that made them.

- **Producer:** NJOY2016 2016.79, commit `ac5adf5`, built from source on this
  machine (`cmake`, gfortran), run 2026-10-14.
- **Inputs:** ENDF/B-VIII.0 tapes from `reference-data/endf/` (their provenance
  is in that folder's README): `n-013_Al_027-ENDF8.0.endf` with
  `tsl-013_Al_027-ENDF8.0.endf`, `n-001_H_001-ENDF8.0-Beta6.endf` with
  `tsl-HinH2O.endf`, `n-006_C_012-ENDF8.0.endf` with
  `tsl-crystalline-graphite.endf`.
- **Deck:** `reconr` (0.001) → `broadr` (0.001, the S(α,β) base temperature) →
  `thermr` (16 cosines, `iform = 0`, card-4 `tol`, `emax = 4 eV`) → `acer`
  (`iopt = 2`), every unit positive (text). Tape 20 is the incident tape,
  tape 21 the thermal tape; run as `njoy < deck`.
- **Licence:** ENDF/B-VIII.0 is public; these are NJOY's processed output of it.

| table (in `reference-data/acer/`) | deck | SHA-256 |
|---|---|---|
| `al27_20k_thermal_tol0p05_njoy2016.ace.gz` | `al27_20k_tol0p05.njoy` | `2ea619f4e51ea2c2e6d45145b29642f697d5182374066209349ec86d30a60a35` |
| `al27_20k_thermal_tol0p001_njoy2016.ace.gz` | `al27_20k_tol0p001.njoy` | `ea350f7780bc57a6b4202b57cf14820437c598442ff5f3e6ab7ffb2897e975ef` |
| `al27_20k_thermal_tol0p001_iwt0_njoy2016.ace.gz` | `al27_20k_tol0p001_iwt0.njoy` | `2bb567894584e6788099e042abc8021e78d25375333a74c71a253a657df97aa4` |
| `hh2o_283p6k_thermal_tol0p001_njoy2016.ace.gz` | `hh2o_283p6k_tol0p001.njoy` | `5040012bf051183320a0905135a129e3472fc963f39dc132104f613285559ec0` |
| `graphite_296k_thermal_tol0p001_njoy2016.ace.gz` | `graphite_296k_tol0p001.njoy` | `76c9296f35f151533762551b88d1483a564612def453fc3e8a61ef4f83d96f70` |

The `.gz` files are `gzip -9 -n` of NJOY's `tape30`.

~~**Decks whose tables are not committed**~~ **CORRECTED 2026-10-14:** these
three tables are now in the `reference-data/ace` submodule
(`reference-njoy/endf-b-viii.0/thermal/`, with stamped decks beside them) and
gated by `tests/acer_thermal_word_parity_large.rs`. The decks here differ from
those only in the comment card; the hashes below are of the unstamped runs
(2–7 MB gzipped):

| deck | table SHA-256 (gzipped, as measured) |
|---|---|
| `al27_20k_tol0p001_iwt2.njoy` | `975c77e850ca18de351742c949709cedf5b07a2565df91e0dd8b1dd647c65e59` |
| `hh2o_283p6k_tol0p001_nbint64_iwt0.njoy` | `396cac55a80335de51ea212413fbf1df94029cd0a3ed7813e73a61a029c96c81` |
| `hh2o_283p6k_tol0p001_nbint64_iwt2.njoy` | `73e81426665d37c86fdba5230b096cd1e8934cd724dab42074af34eee3af050f` |

`gzip -n` output is stable, but `tape30`'s header carries NJOY's run date, so
a re-run on another day gives a different hash; the bytes from line 2 on do
not change.
